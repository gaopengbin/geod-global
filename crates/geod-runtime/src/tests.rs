use super::*;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::{io::AsyncReadExt, net::TcpListener};
use tower::ServiceExt;

const JPEG: &[u8] = b"\xff\xd8\xff\xe0fixture-jpeg-signature-only\xff\xd9";

struct Fixture {
    origin: String,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn fixture() -> Fixture {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let attempts = Arc::new(AtomicUsize::new(0));
    let task = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let attempts = attempts.clone();
            tokio::spawn(async move {
                let mut buffer = [0u8; 8192];
                let read = socket.read(&mut buffer).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buffer[..read]);
                let path = request.split_whitespace().nth(1).unwrap_or("/");
                match path {
                    "/truncated" => {
                        let _ = socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\nConnection: close\r\n\r\n\xff\xd8\xffshort").await;
                    }
                    "/bad" => {
                        let _ = socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\n<html>").await;
                    }
                    "/large" => {
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            MAX_ASSET_BYTES + 1
                        );
                        let _ = socket.write_all(response.as_bytes()).await;
                    }
                    "/redirect" => {
                        let _ = socket.write_all(b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/private\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
                    }
                    "/slow" => {
                        let _ = socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100000\r\nConnection: close\r\n\r\n\xff\xd8\xff").await;
                        for _ in 0..100 {
                            if socket.write_all(&[0; 999]).await.is_err() {
                                break;
                            }
                            tokio::time::sleep(Duration::from_millis(50)).await;
                        }
                    }
                    "/retry" if attempts.fetch_add(1, Ordering::SeqCst) == 0 => {
                        let _ = socket.write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
                    }
                    _ => {
                        let head = format!("HTTP/1.1 200 OK\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", JPEG.len());
                        let _ = socket.write_all(head.as_bytes()).await;
                        let _ = socket.write_all(JPEG).await;
                    }
                }
                let _ = socket.shutdown().await;
            });
        }
    });
    Fixture { origin, task }
}

fn request(origin: &str, path: &str) -> CreateJobRequest {
    CreateJobRequest {
        item_id: "S2_TEST_ITEM".into(),
        asset_key: "thumbnail".into(),
        href: format!("{origin}{path}"),
        media_type: "image/jpeg".into(),
        title: Some("Fixture".into()),
    }
}

async fn settled(manager: &JobManager, id: &str) -> Job {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let store = manager.inner.store.lock().await;
            let job = store.jobs.get(id).unwrap().clone();
            if !active(&job.status) && !store.active.contains_key(id) {
                return job;
            }
            drop(store);
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("job did not settle")
}

#[test]
fn url_guard_rejects_other_hosts_credentials_ports_and_path_mismatch() {
    let valid = format!(
        "https://{SOURCE_HOST}/sentinel-s2-l2a-cogs/10/S/EG/2025/6/S2_TEST_ITEM/preview.jpg"
    );
    assert!(validate_asset_url(&valid).is_ok());
    assert!(validate_request(&request(&valid, ""), None).is_ok());
    for invalid in [
        "http://127.0.0.1/asset.jpg".to_string(),
        valid.replace("https:", "http:"),
        valid.replace(SOURCE_HOST, "evil.example"),
        valid.replace(SOURCE_HOST, &format!("{SOURCE_HOST}.evil.example")),
        valid.replace(SOURCE_HOST, &format!("user@{SOURCE_HOST}")),
        valid.replace(SOURCE_HOST, &format!("{SOURCE_HOST}:444")),
        format!("{valid}?secret=token"),
        format!("{valid}#fragment"),
        valid.replace("sentinel-s2-l2a-cogs", "other-bucket"),
    ] {
        assert!(validate_asset_url(&invalid).is_err(), "accepted {invalid}");
    }
    let mut mismatch = request(&valid, "");
    mismatch.item_id = "OTHER".into();
    assert!(validate_request(&mismatch, None).is_err());
    mismatch.item_id = "../escape".into();
    assert!(validate_request(&mismatch, None).is_err());
    let mut wrong_type = request(&valid, "");
    wrong_type.media_type = "image/tiff".into();
    assert!(validate_request(&wrong_type, None).is_err());
}

#[test]
fn signatures_are_format_specific_and_not_scientific_validation() {
    assert!(verify_signature(JPEG, "image/jpeg").is_ok());
    assert!(verify_signature(b"II\x2a\0\x08\0\0\0", "image/tiff; application=geotiff").is_ok());
    assert!(verify_signature(b"MM\0\x2b\0\x08\0\0", "image/tiff").is_ok());
    assert!(verify_signature(b"II\x2b\0oops", "image/tiff").is_err());
    assert!(verify_signature(JPEG, "image/tiff").is_err());
    assert!(verify_signature(b"<html>error", "image/jpeg").is_err());
}

#[tokio::test]
async fn download_commits_bytes_hash_and_persistent_record() {
    let server = fixture().await;
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open_inner(dir.path(), Some(server.origin.clone()))
        .await
        .unwrap();
    assert!(
        JobManager::open(dir.path()).await.is_err(),
        "storage lock must exclude a second manager"
    );
    let job = manager
        .create(request(&server.origin, "/ok"))
        .await
        .unwrap();
    let completed = settled(&manager, &job.id).await;
    assert_eq!(completed.status, JobStatus::Succeeded);
    assert_eq!(completed.bytes_downloaded, JPEG.len() as u64);
    assert_eq!(
        completed.sha256,
        Some(format!("{:x}", Sha256::digest(JPEG)))
    );
    assert_eq!(
        tokio::fs::read(completed.output_path.as_ref().unwrap())
            .await
            .unwrap(),
        JPEG
    );
    assert!(!dir
        .path()
        .join("assets")
        .join(format!("{}.part", job.id))
        .exists());
    assert!(completed.validation.contains("no scientific"));
    drop(manager);
    let reopened = JobManager::open(dir.path()).await.unwrap();
    assert_eq!(
        reopened.get(&job.id).await.unwrap().sha256,
        completed.sha256
    );
}

#[tokio::test]
async fn truncation_bad_format_redirect_and_size_limit_never_commit() {
    let server = fixture().await;
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open_inner(dir.path(), Some(server.origin.clone()))
        .await
        .unwrap();
    for path in ["/truncated", "/bad", "/redirect", "/large"] {
        let job = manager.create(request(&server.origin, path)).await.unwrap();
        let failed = settled(&manager, &job.id).await;
        assert_eq!(failed.status, JobStatus::Failed, "{path}");
        assert!(failed.error.is_some());
        assert!(failed.output_path.is_none());
        assert!(!dir
            .path()
            .join("assets")
            .join(format!("{}.jpg", job.id))
            .exists());
        assert!(!dir
            .path()
            .join("assets")
            .join(format!("{}.part", job.id))
            .exists());
    }
}

#[tokio::test]
async fn cancel_stops_transfer_and_retry_is_explicit_from_start() {
    let server = fixture().await;
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open_inner(dir.path(), Some(server.origin.clone()))
        .await
        .unwrap();
    let job = manager
        .create(request(&server.origin, "/slow"))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(manager.get(&job.id).await.unwrap().bytes_downloaded > 0);
    assert_eq!(
        manager.cancel(&job.id).await.unwrap().status,
        JobStatus::Cancelled
    );
    let cancelled = settled(&manager, &job.id).await;
    assert_eq!(cancelled.status, JobStatus::Cancelled);
    assert!(cancelled.output_path.is_none());
    let retried = manager.retry(&job.id).await.unwrap();
    assert_eq!(retried.bytes_downloaded, 0);
    assert_eq!(retried.attempts, 2);
    manager.cancel(&job.id).await.unwrap();
    settled(&manager, &job.id).await;
    let job = manager
        .create(request(&server.origin, "/retry"))
        .await
        .unwrap();
    assert_eq!(settled(&manager, &job.id).await.status, JobStatus::Failed);
    manager.retry(&job.id).await.unwrap();
    assert_eq!(
        settled(&manager, &job.id).await.status,
        JobStatus::Succeeded
    );
    assert!(manager.retry(&job.id).await.is_err());
}

#[tokio::test]
async fn restart_marks_unfinished_jobs_interrupted_without_resuming() {
    let server = fixture().await;
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open_inner(dir.path(), Some(server.origin.clone()))
        .await
        .unwrap();
    let job = manager
        .create(request(&server.origin, "/ok"))
        .await
        .unwrap();
    settled(&manager, &job.id).await;
    {
        let mut store = manager.inner.store.lock().await;
        store.jobs.get_mut(&job.id).unwrap().status = JobStatus::Running;
        manager.persist(&store.jobs).await.unwrap();
    }
    drop(manager);
    let reopened = JobManager::open_inner(dir.path(), Some(server.origin.clone()))
        .await
        .unwrap();
    let recovered = reopened.get(&job.id).await.unwrap();
    assert_eq!(recovered.status, JobStatus::Interrupted);
    assert!(recovered.output_path.is_none());
    assert!(recovered.sha256.is_none());
    reopened.retry(&job.id).await.unwrap();
    assert_eq!(
        settled(&reopened, &job.id).await.status,
        JobStatus::Succeeded
    );
}

#[tokio::test]
async fn concurrent_creates_persist_without_lost_updates() {
    let server = fixture().await;
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open_inner(dir.path(), Some(server.origin.clone()))
        .await
        .unwrap();
    let (a, b, c) = tokio::join!(
        manager.create(request(&server.origin, "/ok")),
        manager.create(request(&server.origin, "/ok")),
        manager.create(request(&server.origin, "/ok"))
    );
    for job in [a.unwrap(), b.unwrap(), c.unwrap()] {
        assert_eq!(
            settled(&manager, &job.id).await.status,
            JobStatus::Succeeded
        );
    }
    let records: BTreeMap<String, Job> =
        serde_json::from_slice(&tokio::fs::read(dir.path().join("jobs.json")).await.unwrap())
            .unwrap();
    assert_eq!(records.len(), 3);
}

#[tokio::test]
async fn loopback_api_enforces_origin_host_and_mutation_header() {
    let dir = tempfile::tempdir().unwrap();
    let app = service::router(JobManager::open(dir.path()).await.unwrap());
    for (host, origin, status) in [
        ("127.0.0.1:4318", service::ALLOWED_ORIGIN, StatusCode::OK),
        (
            "127.0.0.1:4318",
            "https://evil.example",
            StatusCode::FORBIDDEN,
        ),
        (
            "evil.example:4318",
            service::ALLOWED_ORIGIN,
            StatusCode::FORBIDDEN,
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .header("Host", host)
                    .header("Origin", origin)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        if status == StatusCode::OK {
            assert_eq!(
                response.headers()["Access-Control-Allow-Origin"],
                service::ALLOWED_ORIGIN
            );
        }
    }
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/jobs")
                .header("Host", "127.0.0.1:4318")
                .header("Content-Type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let response = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/jobs")
                .header("Host", "127.0.0.1:4318")
                .header("Origin", service::ALLOWED_ORIGIN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}
