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
    let partial = dir.path().join("assets").join(format!("{}.part", job.id));
    std::fs::write(&partial, b"interrupted transfer").unwrap();
    drop(manager);
    let reopened = JobManager::open_inner(dir.path(), Some(server.origin.clone()))
        .await
        .unwrap();
    let recovered = reopened.get(&job.id).await.unwrap();
    assert_eq!(recovered.status, JobStatus::Interrupted);
    assert!(recovered.output_path.is_none());
    assert!(recovered.sha256.is_none());
    assert!(
        !partial.exists(),
        "restart must remove the interrupted transfer's partial file"
    );
    reopened.retry(&job.id).await.unwrap();
    assert_eq!(
        settled(&reopened, &job.id).await.status,
        JobStatus::Succeeded
    );
}

#[tokio::test]
async fn shutdown_drains_running_and_queued_downloads_and_preserves_completed_files() {
    let server = fixture().await;
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open_inner(dir.path(), Some(server.origin.clone()))
        .await
        .unwrap();
    let completed = manager
        .create(request(&server.origin, "/ok"))
        .await
        .unwrap();
    let completed = manager.wait(&completed.id).await.unwrap();
    let first = manager
        .create(request(&server.origin, "/slow"))
        .await
        .unwrap();
    let second = manager
        .create(request(&server.origin, "/slow"))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while manager.get(&first.id).await.unwrap().bytes_downloaded == 0
            || manager.get(&second.id).await.unwrap().bytes_downloaded == 0
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let queued = manager
        .create(request(&server.origin, "/slow"))
        .await
        .unwrap();
    assert_eq!(
        manager.get(&queued.id).await.unwrap().status,
        JobStatus::Queued
    );
    tokio::time::timeout(Duration::from_secs(3), manager.shutdown())
        .await
        .unwrap()
        .unwrap();
    assert!(manager.inner.store.lock().await.active.is_empty());
    for job in [&first, &second, &queued] {
        let (record, settled) = manager.get_with_settled(&job.id).await.unwrap();
        assert!(settled);
        assert_eq!(record.status, JobStatus::Interrupted);
        assert!(record.output_path.is_none() && record.sha256.is_none());
        assert!(!dir
            .path()
            .join("assets")
            .join(format!("{}.part", job.id))
            .exists());
    }
    assert_eq!(
        std::fs::read(completed.output_path.as_ref().unwrap()).unwrap(),
        JPEG
    );
    assert_eq!(
        manager.get(&completed.id).await.unwrap().sha256,
        completed.sha256
    );
    assert!(manager
        .create(request(&server.origin, "/ok"))
        .await
        .unwrap_err()
        .contains("shutting down"));
    assert!(manager
        .retry(&first.id)
        .await
        .unwrap_err()
        .contains("shutting down"));
    manager.shutdown().await.unwrap(); // Repeated exit is idempotent.
    drop(manager);
    let reopened = JobManager::open_inner(dir.path(), Some(server.origin.clone()))
        .await
        .unwrap();
    assert_eq!(
        reopened.get(&first.id).await.unwrap().status,
        JobStatus::Interrupted
    );
    assert_eq!(reopened.retry(&queued.id).await.unwrap().attempts, 2);
    reopened.cancel(&queued.id).await.unwrap();
    reopened.wait(&queued.id).await.unwrap();
}

#[tokio::test]
async fn shutdown_preserves_user_cancel_and_drains_even_when_state_cannot_be_written() {
    let server = fixture().await;
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open_inner(dir.path(), Some(server.origin.clone()))
        .await
        .unwrap();
    let cancelled = manager
        .create(request(&server.origin, "/slow"))
        .await
        .unwrap();
    manager.cancel(&cancelled.id).await.unwrap();
    manager.wait(&cancelled.id).await.unwrap();
    let interrupted = manager
        .create(request(&server.origin, "/slow"))
        .await
        .unwrap();
    // A directory at the atomic-write path simulates a real filesystem failure.
    std::fs::create_dir(dir.path().join("jobs.json.tmp")).unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(3), manager.shutdown())
            .await
            .unwrap()
            .is_err()
    );
    assert!(manager.inner.store.lock().await.active.is_empty());
    assert_eq!(
        manager.get(&cancelled.id).await.unwrap().status,
        JobStatus::Cancelled
    );
    assert_eq!(
        manager.get(&interrupted.id).await.unwrap().status,
        JobStatus::Interrupted
    );
    assert!(!dir
        .path()
        .join("assets")
        .join(format!("{}.part", interrupted.id))
        .exists());
    std::fs::remove_dir(dir.path().join("jobs.json.tmp")).unwrap();
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn shutdown_interrupts_queued_clip_without_removing_its_original_source() {
    let (directory, manager, source) = raster_manager().await;
    let original = std::fs::read(source.output_path.as_ref().unwrap()).unwrap();
    let permit = manager
        .inner
        .raster_permits
        .clone()
        .acquire_owned()
        .await
        .unwrap();
    let clip = manager.run_recipe(clip_recipe(&source)).await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), manager.shutdown())
        .await
        .unwrap()
        .unwrap();
    let clip = manager.wait(&clip.id).await.unwrap();
    assert_eq!(clip.status, JobStatus::Interrupted);
    assert!(clip.crop.is_none() && clip.manifest_path.is_none());
    assert!(!directory
        .path()
        .join("assets")
        .join(format!("{}.tif", clip.id))
        .exists());
    assert_eq!(
        std::fs::read(source.output_path.as_ref().unwrap()).unwrap(),
        original
    );
    assert!(manager
        .run_recipe(clip_recipe(&source))
        .await
        .unwrap_err()
        .contains("shutting down"));
    drop(permit);
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

#[tokio::test]
async fn job_snapshot_exposes_terminal_but_unsettled_cancellation() {
    let server = fixture().await;
    let directory = tempfile::tempdir().unwrap();
    let manager = JobManager::open_inner(directory.path(), Some(server.origin.clone()))
        .await
        .unwrap();
    let job = manager
        .create(request(&server.origin, "/ok"))
        .await
        .unwrap();
    settled(&manager, &job.id).await;
    // Represent the real cancellation interval: the terminal record has been
    // persisted, while a worker still owns its cleanup registration.
    {
        let mut store = manager.inner.store.lock().await;
        store.jobs.get_mut(&job.id).unwrap().status = JobStatus::Cancelled;
        store
            .active
            .insert(job.id.clone(), CancellationToken::new());
    }
    let app = service::router(manager.clone());
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/jobs/{}", job.id))
                .header("Host", "127.0.0.1:4318")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 16384)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(value["status"], "cancelled");
    assert_eq!(value["settled"], false);
    manager.inner.store.lock().await.active.remove(&job.id);
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/jobs/{}", job.id))
                .header("Host", "127.0.0.1:4318")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 16384)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(value["settled"], true);
}

async fn raster_manager() -> (tempfile::TempDir, JobManager, Job) {
    let directory = tempfile::tempdir().unwrap();
    let manager = JobManager::open(directory.path()).await.unwrap();
    let pixels: Vec<_> = (0..20).map(|value| (value % 12) as u8).collect();
    let source = raster::tests::record(
        manager.storage_root(),
        &raster::tests::fixture(5, 4, &pixels, 32610, false),
    );
    {
        let mut store = manager.inner.store.lock().await;
        store.jobs.insert(source.id.clone(), source.clone());
        manager.persist(&store.jobs).await.unwrap();
    }
    (directory, manager, source)
}

fn clip_recipe(source: &Job) -> RasterRecipe {
    serde_json::from_value(serde_json::json!({
        "schemaVersion":"geod-raster-recipe/v1", "name":"Exact local clip",
        "source":{"jobId":source.id,"sha256":source.sha256},
        "operation":{"type":"clip","crs":"source","bounds":[500020.0,4199940.0,500080.0,4199980.0]},
        "output":{"format":"GeoTIFF"}
    }))
    .unwrap()
}

#[test]
fn polygon_recipe_deserializes_with_exact_geojson_and_rejects_v1_geometry() {
    let value = serde_json::json!({
        "schemaVersion":"geod-raster-recipe/v2", "name":"Region polygon clip",
        "source":{"jobId":"48bb6e18-3657-48ed-b62c-72472fb39d88","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
        "operation":{"type":"clip","crs":"EPSG:4326","bounds":[-123.0,37.0,-122.0,38.0],
            "geometry":{"type":"Polygon","coordinates":[[[-123.0,37.0],[-122.0,37.0],[-122.0,38.0],[-123.0,37.0]]]}},
        "output":{"format":"GeoTIFF"}
    });
    let recipe: RasterRecipe = serde_json::from_value(value.clone()).unwrap();
    recipe.validate().unwrap();
    assert_eq!(serde_json::to_value(recipe).unwrap(), value);
    let mut invalid = value;
    invalid["schemaVersion"] = serde_json::json!("geod-raster-recipe/v1");
    assert!(serde_json::from_value::<RasterRecipe>(invalid)
        .unwrap()
        .validate()
        .is_err());
}

#[test]
fn persisted_recipe_coordinates_roundtrip_without_one_ulp_drift() {
    // Observed during real WGS84 CLI crop QA: default JSON float parsing shifted
    // the final northing by one ULP on each open/save, separating job and sidecar metadata.
    let bounds = [
        539594.5842039796,
        4170406.570469174,
        559961.0330107023,
        4188280.7439028444,
    ];
    let mut coordinates = bounds;
    for _ in 0..10 {
        coordinates = serde_json::from_slice(&serde_json::to_vec(&coordinates).unwrap()).unwrap();
        assert_eq!(coordinates.map(f64::to_bits), bounds.map(f64::to_bits));
    }
}

#[tokio::test]
async fn old_job_records_default_to_download_without_migration_loss() {
    let (directory, manager, source) = raster_manager().await;
    let mut legacy = serde_json::to_value(&source).unwrap();
    for key in ["kind", "parentId", "recipe", "crop", "manifestPath"] {
        legacy.as_object_mut().unwrap().remove(key);
    }
    std::fs::write(
        directory.path().join("jobs.json"),
        serde_json::to_vec(&serde_json::json!({source.id.clone():legacy})).unwrap(),
    )
    .unwrap();
    drop(manager);
    let reopened = JobManager::open(directory.path()).await.unwrap();
    let recovered = reopened.get(&source.id).await.unwrap();
    assert_eq!(recovered.kind, "download");
    assert_eq!(recovered.sha256, source.sha256);
    assert_eq!(recovered.status, JobStatus::Succeeded);
    assert!(
        recovered.recipe.is_none()
            && recovered.parent_id.is_none()
            && recovered.crop.is_none()
            && recovered.manifest_path.is_none()
    );
}

#[tokio::test]
async fn recipe_validation_rejects_unknown_fields_versions_operations_and_wrong_pins() {
    let (_directory, manager, source) = raster_manager().await;
    let recipe = clip_recipe(&source);
    for (pointer, replacement) in [
        ("/schemaVersion", serde_json::json!("geod-raster-recipe/v2")),
        ("/operation/type", serde_json::json!("shell")),
        ("/operation/crs", serde_json::json!("EPSG:3857")),
        ("/output/format", serde_json::json!("COG")),
        ("/source/sha256", serde_json::json!("A".repeat(64))),
        ("/source/jobId", serde_json::json!("../file")),
        ("/name", serde_json::json!(" ")),
        ("/name", serde_json::json!("a".repeat(121))),
    ] {
        let mut value = serde_json::to_value(&recipe).unwrap();
        *value.pointer_mut(pointer).unwrap() = replacement;
        let invalid: RasterRecipe = serde_json::from_value(value).unwrap();
        assert!(invalid.validate().is_err(), "accepted {pointer}");
    }
    for pointer in ["", "/source", "/operation", "/output"] {
        let mut value = serde_json::to_value(&recipe).unwrap();
        value
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("command".into(), serde_json::json!("untrusted"));
        assert!(serde_json::from_value::<RasterRecipe>(value).is_err());
    }
    let mut bad_pin = recipe.clone();
    bad_pin.source.sha256 = "0".repeat(64);
    assert!(manager
        .plan_recipe(bad_pin.clone())
        .await
        .unwrap_err()
        .contains("SHA-256"));
    assert!(manager.run_recipe(bad_pin.clone()).await.is_err());
    assert!(manager.save_recipe(bad_pin).await.is_err());
    let mut invalid_wgs84 = recipe;
    invalid_wgs84.operation.crs = "EPSG:4326".into();
    for bounds in [
        [-1.0, -81.0, 1.0, 20.0],
        [-1.0, 0.0, 1.0, 85.0],
        [-179.0, 0.0, 179.0, 1.0],
    ] {
        invalid_wgs84.operation.bounds = bounds;
        assert!(invalid_wgs84.validate().is_err());
    }
    assert_eq!(manager.list().await.len(), 1);
}

#[tokio::test]
async fn recipe_plan_is_read_only_and_saved_recipe_survives_restart() {
    let (directory, manager, source) = raster_manager().await;
    let before = std::fs::read(directory.path().join("jobs.json")).unwrap();
    let recipe = clip_recipe(&source);
    let plan = manager.plan_recipe(recipe.clone()).await.unwrap();
    assert_eq!(plan.plan.window, [1, 1, 3, 2]);
    assert_eq!(plan.plan.source_sha256, source.sha256.unwrap());
    assert_eq!(
        std::fs::read_dir(directory.path().join("assets"))
            .unwrap()
            .count(),
        1
    );
    assert_eq!(
        std::fs::read(directory.path().join("jobs.json")).unwrap(),
        before
    );
    assert!(!directory.path().join("recipes.json").exists());
    let saved = manager.save_recipe(recipe.clone()).await.unwrap();
    assert_eq!(manager.list_recipes().await.len(), 1);
    drop(manager);
    let reopened = JobManager::open(directory.path()).await.unwrap();
    let records = reopened.list_recipes().await;
    assert_eq!(records[0].id, saved.id);
    assert_eq!(
        serde_json::to_value(&records[0].recipe).unwrap(),
        serde_json::to_value(recipe).unwrap()
    );
}

#[tokio::test]
async fn recipe_runs_commit_real_geotiff_sidecar_and_deterministic_inspectable_output() {
    let (directory, manager, source) = raster_manager().await;
    let original = std::fs::read(source.output_path.as_ref().unwrap()).unwrap();
    let recipe = clip_recipe(&source);
    let first = manager.run_recipe(recipe.clone()).await.unwrap();
    let first = manager.wait(&first.id).await.unwrap();
    assert_eq!(first.status, JobStatus::Succeeded, "{:?}", first.error);
    assert_eq!(first.kind, "raster_clip");
    assert_eq!(first.parent_id.as_deref(), Some(source.id.as_str()));
    assert_eq!(first.href, source.href);
    assert_eq!(first.crop.as_ref().unwrap().window, [1, 1, 3, 2]);
    let inspection = manager.inspect_raster(&first.id).await.unwrap();
    assert_eq!((inspection.width, inspection.height), (3, 2));
    let metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(first.manifest_path.as_ref().unwrap()).unwrap())
            .unwrap();
    assert_eq!(metadata["schemaVersion"], "geod-raster-artifact/v1");
    assert_eq!(metadata["output"]["file"], format!("{}.tif", first.id));
    assert_eq!(
        metadata["output"]["sha256"],
        first.sha256.as_deref().unwrap()
    );
    assert_eq!(
        metadata["source"]["sha256"],
        source.sha256.as_deref().unwrap()
    );
    assert_eq!(metadata["recipe"], serde_json::to_value(&recipe).unwrap());
    let second = manager.run_recipe(recipe).await.unwrap();
    let second = manager.wait(&second.id).await.unwrap();
    assert_eq!(second.status, JobStatus::Succeeded);
    assert_ne!(first.id, second.id);
    assert_eq!(first.sha256, second.sha256);
    assert_eq!(
        std::fs::read(source.output_path.as_ref().unwrap()).unwrap(),
        original
    );
    drop(manager);
    let reopened = JobManager::open(directory.path()).await.unwrap();
    assert_eq!(
        reopened.get(&first.id).await.unwrap().status,
        JobStatus::Succeeded
    );
    assert_eq!(
        reopened.get(&first.id).await.unwrap().manifest_path,
        first.manifest_path
    );
}

#[tokio::test]
async fn queued_clip_cancel_retry_uses_same_operation_without_downloading() {
    let (_directory, manager, source) = raster_manager().await;
    let permit = manager
        .inner
        .raster_permits
        .clone()
        .acquire_owned()
        .await
        .unwrap();
    let job = manager.run_recipe(clip_recipe(&source)).await.unwrap();
    assert!(manager
        .plan_recipe(clip_recipe(&source))
        .await
        .unwrap_err()
        .contains("busy"));
    assert!(manager.inspect_raster(&source.id).await.is_err());
    manager.cancel(&job.id).await.unwrap();
    let cancelled = settled(&manager, &job.id).await;
    assert_eq!(cancelled.status, JobStatus::Cancelled);
    assert!(cancelled.output_path.is_none());
    let retry = manager.retry(&job.id).await.unwrap();
    assert_eq!(retry.kind, "raster_clip");
    assert_eq!(retry.attempts, 2);
    drop(permit);
    let completed = settled(&manager, &job.id).await;
    assert_eq!(
        completed.status,
        JobStatus::Succeeded,
        "{:?}",
        completed.error
    );
    assert!(completed.manifest_path.is_some());
}

#[tokio::test]
async fn clip_restart_discards_uncommitted_output_and_retry_reexecutes() {
    let (directory, manager, source) = raster_manager().await;
    let job = manager.run_recipe(clip_recipe(&source)).await.unwrap();
    let completed = settled(&manager, &job.id).await;
    assert_eq!(completed.status, JobStatus::Succeeded);
    {
        let mut store = manager.inner.store.lock().await;
        store.jobs.get_mut(&job.id).unwrap().status = JobStatus::Running;
        manager.persist(&store.jobs).await.unwrap();
    }
    let partial = directory
        .path()
        .join("assets")
        .join(format!("{}.crop-crash.part", job.id));
    std::fs::write(&partial, "partial").unwrap();
    drop(manager);
    let reopened = JobManager::open(directory.path()).await.unwrap();
    let recovered = reopened.get(&job.id).await.unwrap();
    assert_eq!(recovered.status, JobStatus::Interrupted);
    assert!(
        recovered.output_path.is_none()
            && recovered.manifest_path.is_none()
            && recovered.crop.is_none()
    );
    assert!(!Path::new(completed.output_path.as_ref().unwrap()).exists());
    assert!(!Path::new(completed.manifest_path.as_ref().unwrap()).exists());
    assert!(!partial.exists());
    reopened.retry(&job.id).await.unwrap();
    let retried = settled(&reopened, &job.id).await;
    assert_eq!(retried.status, JobStatus::Succeeded, "{:?}", retried.error);
    assert_eq!(retried.sha256, completed.sha256);
}

#[tokio::test]
async fn clip_sidecar_failure_never_exposes_success_and_can_retry() {
    let (directory, manager, source) = raster_manager().await;
    let permit = manager
        .inner
        .raster_permits
        .clone()
        .acquire_owned()
        .await
        .unwrap();
    let job = manager.run_recipe(clip_recipe(&source)).await.unwrap();
    // A directory at the exact generated temporary sidecar path forces metadata failure after TIFF encoding.
    let obstruction = directory
        .path()
        .join("assets")
        .join(format!("{}.metadata.json.tmp", job.id));
    std::fs::create_dir(&obstruction).unwrap();
    drop(permit);
    let failed = settled(&manager, &job.id).await;
    assert_eq!(failed.status, JobStatus::Failed);
    assert!(
        failed.sha256.is_none() && failed.output_path.is_none() && failed.manifest_path.is_none()
    );
    assert!(!directory
        .path()
        .join("assets")
        .join(format!("{}.tif", job.id))
        .exists());
    std::fs::remove_dir(obstruction).unwrap();
    manager.retry(&job.id).await.unwrap();
    assert_eq!(
        settled(&manager, &job.id).await.status,
        JobStatus::Succeeded
    );
}

#[tokio::test]
async fn recipe_api_obeys_same_boundary_and_returns_real_plan() {
    let (_directory, manager, source) = raster_manager().await;
    let app = service::router(manager.clone());
    let body = serde_json::to_vec(&clip_recipe(&source)).unwrap();
    for route in ["/recipes", "/recipes/plan", "/recipes/run"] {
        for (origin, client_header) in [
            ("https://evil.example", true),
            (service::ALLOWED_ORIGIN, false),
        ] {
            let mut request = Request::builder()
                .method("POST")
                .uri(route)
                .header("Host", "127.0.0.1:4318")
                .header("Origin", origin)
                .header("Content-Type", "application/json");
            if client_header {
                request = request.header("X-GeoD-Client", "geod-global");
            }
            let response = app
                .clone()
                .oneshot(request.body(Body::from(body.clone())).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }
    }
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/recipes/plan")
                .header("Host", "127.0.0.1:4318")
                .header("X-GeoD-Client", "geod-global")
                .header("Content-Type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    let plan: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(plan["plan"]["window"], serde_json::json!([1, 1, 3, 2]));
    assert_eq!(manager.list().await.len(), 1);
    assert!(manager.list_recipes().await.is_empty());
}
