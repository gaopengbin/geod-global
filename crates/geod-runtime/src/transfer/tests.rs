use super::*;
use crate::{CreateJobRequest, JobManager, JobStatus, ProxySettings};
use std::sync::{Arc, Mutex};
use tokio::{io::AsyncReadExt, net::TcpListener};

// Deliberately only a JPEG signature fixture: these tests verify HTTP recovery,
// durable byte integrity and cancellation, not real imagery or raster science.
const BODY: &[u8] = b"\xff\xd8\xff\xe0transfer-fixture-original-version\xff\xd9";
const CHANGED: &[u8] = b"\xff\xd8\xff\xe0transfer-fixture-replacement-version\xff\xd9";
const PREFIX: usize = 16;

struct Server {
    origin: String,
    requests: Arc<Mutex<Vec<String>>>,
    token: CancellationToken,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.token.cancel();
        self.task.abort();
    }
}

async fn server(behavior: &'static str) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    let token = CancellationToken::new();
    let cancellation = token.clone();
    let task = tokio::spawn(async move {
        loop {
            let accepted = tokio::select! {
                _ = cancellation.cancelled() => break,
                accepted = listener.accept() => accepted,
            };
            let Ok((mut socket, _)) = accepted else {
                break;
            };
            let requests = captured.clone();
            let token = cancellation.clone();
            tokio::spawn(async move {
                let mut request = vec![0; 8192];
                let mut read = 0;
                while read < request.len() && !request[..read].windows(4).any(|w| w == b"\r\n\r\n")
                {
                    let amount = socket.read(&mut request[read..]).await.unwrap();
                    if amount == 0 {
                        return;
                    }
                    read += amount;
                }
                let request = String::from_utf8(request[..read].to_vec())
                    .unwrap()
                    .to_ascii_lowercase();
                let attempt = {
                    let mut list = requests.lock().unwrap();
                    list.push(request.clone());
                    list.len()
                };
                let partial = request.contains("\r\nrange:");
                if behavior == "unavailable" {
                    let _ = socket.write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
                    return;
                }
                let first = attempt == 1;
                if behavior == "stall-headers" {
                    let _ = socket.write_all(b"HTTP/1.1 200 OK\r\n").await;
                    tokio::select! { _ = token.cancelled() => {}, _ = tokio::time::sleep(std::time::Duration::from_secs(20)) => {} }
                    return;
                }
                let weak = behavior == "weak";
                let missing = behavior == "missing";
                let changed = behavior == "changed"
                    || (!first && !partial && matches!(behavior, "bad-range" | "bad-etag" | "416"));
                let body = if changed && !first { CHANGED } else { BODY };
                let status = if behavior == "unexpected" || (partial && behavior != "changed") {
                    "206 Partial Content"
                } else {
                    "200 OK"
                };
                let mut headers = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: image/jpeg\r\nConnection: close\r\n"
                );
                if partial && behavior == "forbidden" && attempt == 2 {
                    let _ = socket.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
                    return;
                }
                if partial && behavior == "416" {
                    let _ = socket.write_all(b"HTTP/1.1 416 Range Not Satisfiable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
                    return;
                }
                if !missing {
                    headers.push_str(if weak {
                        "ETag: W/\"v1\"\r\n"
                    } else if changed && !first || behavior == "bad-etag" && partial {
                        "ETag: \"v2\"\r\n"
                    } else {
                        "ETag: \"v1\"\r\n"
                    });
                }
                let offset = if partial && behavior != "changed" {
                    PREFIX
                } else {
                    0
                };
                if status.starts_with("206") {
                    let from = if behavior == "bad-range" {
                        PREFIX + 1
                    } else {
                        offset
                    };
                    headers.push_str(&format!(
                        "Content-Range: bytes {from}-{}/{}\r\n",
                        body.len() - 1,
                        body.len()
                    ));
                }
                headers.push_str(&format!("Content-Length: {}\r\n\r\n", body.len() - offset));
                if socket.write_all(headers.as_bytes()).await.is_err() {
                    return;
                }
                if behavior == "stall-first-data" {
                    tokio::select! { _ = token.cancelled() => {}, _ = tokio::time::sleep(std::time::Duration::from_secs(20)) => {} }
                    return;
                }
                if behavior == "healthy-slow" {
                    let _ = socket.write_all(&body[..PREFIX]).await;
                    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
                    let _ = socket.write_all(&body[PREFIX..]).await;
                    return;
                }
                if first && behavior != "unexpected" {
                    let _ = socket.write_all(&body[..PREFIX - 4]).await;
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                    let _ = socket.write_all(&body[PREFIX - 4..PREFIX]).await;
                    if behavior == "hold" {
                        tokio::select! { _ = token.cancelled() => {}, _ = tokio::time::sleep(std::time::Duration::from_secs(20)) => {} }
                    }
                } else {
                    let _ = socket.write_all(&body[offset..]).await;
                }
                let _ = socket.shutdown().await;
            });
        }
    });
    Server {
        origin,
        requests,
        token,
        task,
    }
}

async fn manager(dir: &Path, server: &Server) -> JobManager {
    let manager = JobManager::open_inner(dir, Some(server.origin.clone()))
        .await
        .unwrap();
    manager
        .save_proxy_settings(ProxySettings {
            mode: crate::proxy::ProxyMode::Direct,
            url: None,
        })
        .await
        .unwrap();
    manager
}

async fn create(manager: &JobManager, server: &Server) -> Job {
    manager
        .create(CreateJobRequest {
            item_id: "S2_TRANSFER_TEST".into(),
            asset_key: "thumbnail".into(),
            href: format!("{}/image", server.origin),
            media_type: "image/jpeg".into(),
            title: None,
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn bounded_recovery_resumes_a_broken_body_without_user_retry() {
    let server = server("valid").await;
    let dir = tempfile::tempdir().unwrap();
    let manager = manager(dir.path(), &server).await;
    let job = crate::new_download_job(CreateJobRequest {
        item_id: "S2_TRANSFER_TEST".into(),
        asset_key: "thumbnail".into(),
        href: format!("{}/image", server.origin),
        media_type: "image/jpeg".into(),
        title: None,
    });
    manager
        .inner
        .store
        .lock()
        .await
        .jobs
        .insert(job.id.clone(), job.clone());
    manager
        .download_with_retries(&job.id, &CancellationToken::new(), 2)
        .await
        .unwrap();
    let job = assert_complete(&manager, &job.id, BODY, TransferMode::Resumed).await;
    assert_eq!(job.network_retries, 1);
    assert_eq!(job.attempts, 1);
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].contains("\r\nrange: bytes=16-\r\n"));
    assert!(requests[1].contains("\r\nif-range: \"v1\"\r\n"));
}

#[tokio::test]
async fn first_data_deadline_covers_headers_and_body_and_preserves_cancellation() {
    for behavior in ["stall-headers", "stall-first-data"] {
        let server = server(behavior).await;
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path(), &server).await;
        let job = crate::new_download_job(CreateJobRequest {
            item_id: "S2_TRANSFER_TEST".into(),
            asset_key: "thumbnail".into(),
            href: format!("{}/image", server.origin),
            media_type: "image/jpeg".into(),
            title: None,
        });
        manager
            .inner
            .store
            .lock()
            .await
            .jobs
            .insert(job.id.clone(), job.clone());
        let start = tokio::time::Instant::now();
        let error = manager
            .download_once_with_first_data_timeout(
                &job.id,
                &CancellationToken::new(),
                Some(std::time::Duration::from_millis(250)),
            )
            .await
            .unwrap_err();
        assert_eq!(
            error,
            "Asset transfer timed out before receiving response data"
        );
        assert!(retryable(&error));
        assert!(start.elapsed() < std::time::Duration::from_secs(5));
        let record = manager.get(&job.id).await.unwrap();
        assert_eq!(record.bytes_downloaded, 0);
        assert!(record.output_path.is_none() && record.sha256.is_none());
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert_eq!(
            manager
                .download_once_with_first_data_timeout(
                    &job.id,
                    &cancellation,
                    Some(std::time::Duration::from_millis(250))
                )
                .await
                .unwrap_err(),
            "Transfer cancelled"
        );
    }
}

#[tokio::test]
async fn first_data_deadline_excludes_queue_wait_and_does_not_limit_a_healthy_body() {
    let server = server("healthy-slow").await;
    let dir = tempfile::tempdir().unwrap();
    let manager = manager(dir.path(), &server).await;
    let mut job = crate::new_download_job(CreateJobRequest {
        item_id: "S2_TRANSFER_TEST".into(),
        asset_key: "thumbnail".into(),
        href: format!("{}/image", server.origin),
        media_type: "image/jpeg".into(),
        title: None,
    });
    // Model a previous failed attempt waiting to reacquire a worker slot.
    job.status = JobStatus::Running;
    manager
        .inner
        .store
        .lock()
        .await
        .jobs
        .insert(job.id.clone(), job.clone());
    let permits = manager.inner.permits.acquire_many(2).await.unwrap();
    let token = CancellationToken::new();
    let download = manager.download_once_with_first_data_timeout(
        &job.id,
        &token,
        Some(std::time::Duration::from_millis(250)),
    );
    let release = async {
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        assert_eq!(
            manager.get(&job.id).await.unwrap().status,
            JobStatus::Queued
        );
        drop(permits);
    };
    let (result, ()) = tokio::join!(download, release);
    result.unwrap();
    assert_complete(&manager, &job.id, BODY, TransferMode::Fresh).await;
}

#[tokio::test]
async fn body_failure_keeps_a_safe_reason_and_permanent_errors_are_not_retryable() {
    let server = server("valid").await;
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let error = client
        .get(format!("{}/image?sig=private", server.origin))
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap_err();
    let reason = network_error(error);
    assert_eq!(reason, "Asset network response was incomplete");
    assert!(retryable(&reason));
    assert!(!reason.contains("private"));
    for reason in [
        "Asset server returned HTTP 403 Forbidden",
        "Asset server returned HTTP 404 Not Found",
        "Unexpected partial HTTP response",
        "Asset SHA-256 checksum mismatch",
        "Transfer cancelled",
    ] {
        assert!(!retryable(reason));
    }
    assert!(retryable(
        "Asset server returned HTTP 503 Service Unavailable; partial download was retained"
    ));
}

#[tokio::test]
async fn automatic_recovery_has_a_finite_budget_and_cancellation_stops_backoff() {
    for cancelled in [false, true] {
        let server = server("unavailable").await;
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path(), &server).await;
        let job = crate::new_download_job(CreateJobRequest {
            item_id: "S2_TRANSFER_TEST".into(),
            asset_key: "thumbnail".into(),
            href: format!("{}/image", server.origin),
            media_type: "image/jpeg".into(),
            title: None,
        });
        manager
            .inner
            .store
            .lock()
            .await
            .jobs
            .insert(job.id.clone(), job.clone());
        let token = CancellationToken::new();
        if cancelled {
            let stopping = token.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                stopping.cancel();
            });
        }
        let error = manager
            .download_with_retries(&job.id, &token, 2)
            .await
            .unwrap_err();
        assert_eq!(
            server.requests.lock().unwrap().len(),
            if cancelled { 1 } else { 3 }
        );
        assert_eq!(
            manager.get(&job.id).await.unwrap().network_retries,
            if cancelled { 0 } else { 2 }
        );
        if cancelled {
            assert_eq!(error, "Transfer cancelled");
        } else {
            assert!(error.contains("HTTP 503"));
        }
        assert!(manager.get(&job.id).await.unwrap().output_path.is_none());
    }
}

async fn settled(manager: &JobManager, id: &str) -> Job {
    tokio::time::timeout(std::time::Duration::from_secs(8), manager.wait(id))
        .await
        .unwrap()
        .unwrap()
}

async fn progressed(manager: &JobManager, id: &str) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while manager.get(id).await.unwrap().bytes_downloaded < PREFIX as u64 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

async fn assert_complete(manager: &JobManager, id: &str, bytes: &[u8], mode: TransferMode) -> Job {
    let job = settled(manager, id).await;
    assert_eq!(job.status, JobStatus::Succeeded, "{:?}", job.error);
    assert_eq!(
        job.sha256.as_deref(),
        Some(format!("{:x}", Sha256::digest(bytes)).as_str())
    );
    assert_eq!(
        tokio::fs::read(job.output_path.as_ref().unwrap())
            .await
            .unwrap(),
        bytes
    );
    assert_eq!(job.bytes_downloaded, bytes.len() as u64);
    assert_eq!(job.transfer.as_ref().unwrap().mode, mode);
    let (part, receipt) = paths(&manager.inner.root, id).unwrap();
    assert!(!part.exists() && !receipt.exists());
    job
}

#[tokio::test]
async fn interrupted_body_resumes_only_exact_validator_and_range_and_hashes_whole_file() {
    let server = server("valid").await;
    let dir = tempfile::tempdir().unwrap();
    let manager = manager(dir.path(), &server).await;
    let job = create(&manager, &server).await;
    let failed = settled(&manager, &job.id).await;
    assert_eq!(failed.status, JobStatus::Failed);
    assert!(failed.output_path.is_none() && failed.sha256.is_none());
    assert!(candidate(
        &manager.inner.root,
        &failed,
        crate::MAX_ASSET_BYTES
    ));
    manager.retry(&job.id).await.unwrap();
    let job = assert_complete(&manager, &job.id, BODY, TransferMode::Resumed).await;
    assert_eq!(job.transfer.unwrap().resumed_bytes, PREFIX as u64);
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(!requests[0].contains("\r\nrange:"));
    assert!(requests[1].contains("\r\nrange: bytes=16-\r\n"));
    assert!(requests[1].contains("\r\nif-range: \"v1\"\r\n"));
    assert!(requests
        .iter()
        .all(|request| request.contains("\r\naccept-encoding: identity\r\n")));
}

#[tokio::test]
async fn shutdown_retains_checkpoint_and_reopen_ignores_uncheckpointed_tail() {
    let server = server("hold").await;
    let dir = tempfile::tempdir().unwrap();
    let first = manager(dir.path(), &server).await;
    let job = create(&first, &server).await;
    progressed(&first, &job.id).await;
    first.shutdown().await.unwrap();
    let interrupted = first.get(&job.id).await.unwrap();
    assert_eq!(interrupted.status, JobStatus::Interrupted);
    let (part, _) = paths(&first.inner.root, &job.id).unwrap();
    assert!(candidate(
        &first.inner.root,
        &interrupted,
        crate::MAX_ASSET_BYTES
    ));
    let mut file = tokio::fs::OpenOptions::new()
        .append(true)
        .open(&part)
        .await
        .unwrap();
    file.write_all(b"uncommitted crash tail").await.unwrap();
    drop(file);
    drop(first);
    let reopened = manager(dir.path(), &server).await;
    assert!(part.exists());
    reopened.retry(&job.id).await.unwrap();
    assert_complete(&reopened, &job.id, BODY, TransferMode::Resumed).await;
}

#[tokio::test]
async fn changed_version_invalid_range_and_416_restart_without_mixing_bytes() {
    for behavior in ["changed", "bad-range", "bad-etag", "416"] {
        let server = server(behavior).await;
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path(), &server).await;
        let job = create(&manager, &server).await;
        assert_eq!(settled(&manager, &job.id).await.status, JobStatus::Failed);
        manager.retry(&job.id).await.unwrap();
        assert_complete(&manager, &job.id, CHANGED, TransferMode::Restarted).await;
        let requests = server.requests.lock().unwrap();
        assert_eq!(requests.len(), if behavior == "changed" { 2 } else { 3 });
        if requests.len() == 3 {
            assert!(!requests[2].contains("\r\nrange:"));
        }
    }
}

#[tokio::test]
async fn weak_or_missing_etag_never_retains_or_sends_a_range() {
    for behavior in ["weak", "missing"] {
        let server = server(behavior).await;
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path(), &server).await;
        let job = create(&manager, &server).await;
        settled(&manager, &job.id).await;
        let (part, receipt) = paths(&manager.inner.root, &job.id).unwrap();
        assert!(!part.exists() && !receipt.exists());
        manager.retry(&job.id).await.unwrap();
        assert_complete(&manager, &job.id, BODY, TransferMode::Fresh).await;
        assert!(server
            .requests
            .lock()
            .unwrap()
            .iter()
            .all(|request| !request.contains("\r\nrange:")));
    }
}

#[tokio::test]
async fn changed_local_prefix_restarts_without_trusting_saved_digest() {
    let server = server("valid").await;
    let dir = tempfile::tempdir().unwrap();
    let manager = manager(dir.path(), &server).await;
    let job = create(&manager, &server).await;
    settled(&manager, &job.id).await;
    let (part, _) = paths(&manager.inner.root, &job.id).unwrap();
    let mut bytes = tokio::fs::read(&part).await.unwrap();
    bytes[PREFIX - 1] ^= 1;
    tokio::fs::write(&part, bytes).await.unwrap();
    manager.retry(&job.id).await.unwrap();
    assert_complete(&manager, &job.id, BODY, TransferMode::Restarted).await;
    assert!(!server.requests.lock().unwrap()[1].contains("\r\nrange:"));
}

#[tokio::test]
async fn explicit_cancel_purges_checkpoint_and_retries_from_start() {
    let server = server("hold").await;
    let dir = tempfile::tempdir().unwrap();
    let manager = manager(dir.path(), &server).await;
    let job = create(&manager, &server).await;
    progressed(&manager, &job.id).await;
    manager.cancel(&job.id).await.unwrap();
    assert_eq!(
        settled(&manager, &job.id).await.status,
        JobStatus::Cancelled
    );
    let (part, receipt) = paths(&manager.inner.root, &job.id).unwrap();
    assert!(!part.exists() && !receipt.exists());
    manager.retry(&job.id).await.unwrap();
    assert_complete(&manager, &job.id, BODY, TransferMode::Fresh).await;
    assert!(!server.requests.lock().unwrap()[1].contains("\r\nrange:"));
}

#[tokio::test]
async fn access_denial_keeps_candidate_without_retrying_or_bypassing_authorization() {
    let server = server("forbidden").await;
    let dir = tempfile::tempdir().unwrap();
    let manager = manager(dir.path(), &server).await;
    let job = create(&manager, &server).await;
    settled(&manager, &job.id).await;
    manager.retry(&job.id).await.unwrap();
    let failed = settled(&manager, &job.id).await;
    assert_eq!(failed.status, JobStatus::Failed);
    assert!(failed.error.unwrap().contains("403"));
    assert_eq!(server.requests.lock().unwrap().len(), 2);
    assert!(candidate(&manager.inner.root, &job, crate::MAX_ASSET_BYTES));
    manager.retry(&job.id).await.unwrap();
    assert_complete(&manager, &job.id, BODY, TransferMode::Resumed).await;
}

#[tokio::test]
async fn unsolicited_partial_response_is_never_committed() {
    let server = server("unexpected").await;
    let dir = tempfile::tempdir().unwrap();
    let manager = manager(dir.path(), &server).await;
    let job = settled(&manager, &create(&manager, &server).await.id).await;
    assert_eq!(job.status, JobStatus::Failed);
    assert!(job.sha256.is_none() && job.output_path.is_none());
}

#[test]
fn strong_validators_and_content_ranges_are_strict() {
    assert!(strong_etag("\"opaque-v1\""));
    for value in ["W/\"v1\"", "v1", "\"bad\r\n\"", "\"embedded\"quote\""] {
        assert!(!strong_etag(value));
    }
    assert_eq!(range("bytes 16-99/100"), Some((16, 99, 100)));
    for value in [
        "bytes +16-99/100",
        "bytes 16-99/*",
        "bytes=16-99/100",
        "bytes 16-99/100,0-15/100",
    ] {
        assert!(range(value).is_none());
    }
}

#[tokio::test]
async fn receipt_is_bound_to_the_job_and_size_limit_and_contains_no_access_url() {
    let server = server("valid").await;
    let dir = tempfile::tempdir().unwrap();
    let manager = manager(dir.path(), &server).await;
    let job = create(&manager, &server).await;
    settled(&manager, &job.id).await;
    let (_, path) = paths(&manager.inner.root, &job.id).unwrap();
    let receipt = tokio::fs::read(&path).await.unwrap();
    assert!(!String::from_utf8_lossy(&receipt).contains(&server.origin));
    let mut changed = job.clone();
    changed.asset_key = "visual".into();
    assert!(!candidate(
        &manager.inner.root,
        &changed,
        crate::MAX_ASSET_BYTES
    ));
    changed = job.clone();
    changed.href.push_str("/changed");
    assert!(!candidate(
        &manager.inner.root,
        &changed,
        crate::MAX_ASSET_BYTES
    ));
    assert!(!candidate(&manager.inner.root, &job, PREFIX as u64));
    tokio::fs::write(&path, vec![b' '; 4097]).await.unwrap();
    assert!(!candidate(
        &manager.inner.root,
        &job,
        crate::MAX_ASSET_BYTES
    ));
    tokio::fs::write(&path, receipt).await.unwrap();
    assert!(candidate(&manager.inner.root, &job, crate::MAX_ASSET_BYTES));
    assert!(TransferInfo {
        mode: TransferMode::Resumed,
        resumed_bytes: 1
    }
    .validate(&job)
    .is_err());
    assert!(TransferInfo {
        mode: TransferMode::Fresh,
        resumed_bytes: 1
    }
    .validate(&job)
    .is_err());
}

#[tokio::test]
async fn recovery_copies_the_prefix_without_appending_to_a_preexisting_hard_link() {
    let server = server("valid").await;
    let dir = tempfile::tempdir().unwrap();
    let manager = manager(dir.path(), &server).await;
    let job = create(&manager, &server).await;
    settled(&manager, &job.id).await;
    let (part, _) = paths(&manager.inner.root, &job.id).unwrap();
    let outside = tempfile::tempdir().unwrap();
    let linked = outside.path().join("retained-original-part");
    std::fs::hard_link(part, &linked).unwrap();
    manager.retry(&job.id).await.unwrap();
    assert_complete(&manager, &job.id, BODY, TransferMode::Resumed).await;
    assert_eq!(tokio::fs::read(linked).await.unwrap(), &BODY[..PREFIX]);
}

#[tokio::test]
async fn crash_staging_cleanup_only_removes_canonical_managed_temporary_files() {
    let server = server("valid").await;
    let dir = tempfile::tempdir().unwrap();
    let manager = manager(dir.path(), &server).await;
    let id = Uuid::new_v4();
    let temporary = Uuid::new_v4();
    let assets = manager.inner.root.join("assets");
    let staged = assets.join(format!("{id}.{temporary}.resume-write.part"));
    let receipt = assets.join(format!("{id}.{temporary}.resume.json.tmp"));
    let unrelated = assets.join("ordinary.resume-write.part");
    let partial = assets.join(format!("{id}.part"));
    for path in [&staged, &receipt, &unrelated, &partial] {
        tokio::fs::write(path, b"test").await.unwrap();
    }
    cleanup_staging(&manager.inner.root).await.unwrap();
    assert!(!staged.exists() && !receipt.exists());
    assert!(unrelated.exists() && partial.exists());
}

// Offline native preset receipts here exercise eligibility and checkpoint
// security. They do not assert an HTTP download or validate fixture pixels.
fn reviewed_archive_job(root: &Path) -> (Job, crate::stac::archive_original::ArchiveSelection) {
    use serde_json::json;
    std::fs::create_dir_all(root.join("assets")).unwrap();
    std::fs::create_dir_all(root.join("stac")).unwrap();
    let plan = crate::datasets::resolve(crate::datasets::ResolveRequest {
        dataset_id: "glc-fcs10".into(),
        bounds: [-174.5, 54.5, -174.4, 54.6],
        year: Some(2023),
        month: None,
        country: None,
        layer: Some("classes".into()),
        region: None,
        season: None,
    })
    .unwrap();
    let source = &plan.sources[0];
    let href = source.stac.as_ref().unwrap().url.clone();
    let proof = crate::datasets::SourceProvenance {
        schema_version: 2,
        dataset: plan.dataset.clone(),
        request: plan.request.clone(),
        layer: plan.dataset.layers[0].clone(),
        source_id: source.id.clone(),
        source_title: source.title.clone(),
        source_bounds: source.bounds,
        declaration_kind:
            "reviewed-client-preset; publisher file bytes and embedded metadata verified separately"
                .into(),
        catalog_raster: None,
    };
    let original = crate::stac::archive_original::from_proof(&proof, &href)
        .unwrap()
        .unwrap();
    let raw = json!({"kind":"reviewed-raster-archive","href":href,"probeStatus":206,
        "contentRange":format!("bytes 0-15/{}",original.archive_bytes),"contentLength":16,
        "headerHex":"504b0304","reviewedArchive":original,"datasetPreset":proof,
        "file:size":original.archive_bytes,"file:checksum":format!("1220{}",original.archive_sha256)});
    let bytes = serde_json::to_vec(&raw).unwrap();
    let document_sha = format!("{:x}", Sha256::digest(&bytes));
    std::fs::write(
        root.join("stac")
            .join(format!("document-{document_sha}.json")),
        bytes,
    )
    .unwrap();
    let record = json!({"version":1,"connectionId":Uuid::new_v4().to_string(),
        "serviceName":source.title,"kind":"raster","documentUrl":href,
        "documentSha256":document_sha,"itemIndex":null,"retrievedAt":crate::now(),
        "metadataDocuments":[],"search":null});
    let bytes = serde_json::to_vec(&record).unwrap();
    let snapshot = format!("{:x}", Sha256::digest(&bytes));
    std::fs::write(
        root.join("stac").join(format!("snapshot-{snapshot}.json")),
        bytes,
    )
    .unwrap();
    let pin = crate::stac::SourcePin {
        snapshot_id: snapshot,
        asset_key: "raster".into(),
    };
    let item = crate::stac::resolve(root, &pin).unwrap();
    let mut job = crate::new_download_job(CreateJobRequest {
        item_id: item.item_id,
        asset_key: "stac_asset".into(),
        href: item.href,
        media_type: item.media_type,
        title: Some(item.title),
    });
    job.source = item.service_name;
    job.stac_source = Some(pin);
    assert_eq!(
        crate::stac::archive_original::reviewed_original(root, &job).unwrap(),
        Some(original.clone())
    );
    (job, original)
}

async fn hash_checkpoint(
    root: &Path,
    job: &Job,
    original: &crate::stac::archive_original::ArchiveSelection,
    prefix: &[u8],
) {
    let (part, _) = paths(root, &job.id).unwrap();
    let mut file = tokio::fs::File::create(&part).await.unwrap();
    file.write_all(prefix).await.unwrap();
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    let pin = Validator::ReviewedOriginal {
        expected_original_sha256: original.archive_sha256.clone(),
        total: original.archive_bytes,
    };
    checkpoint(
        root,
        job,
        &file,
        Some(&pin),
        prefix.len() as u64,
        &hasher,
        &prefix[..prefix.len().min(16)],
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn compiled_original_checkpoint_is_bound_to_native_source_and_full_identity() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let (job, original) = reviewed_archive_job(&root);
    let prefix = b"PK\x03\x04offline-security-prefix-fixture";
    hash_checkpoint(&root, &job, &original, prefix).await;
    assert!(candidate(&root, &job, crate::MAX_ASSET_BYTES));
    let (_, receipt) = paths(&root, &job.id).unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&receipt).unwrap()).unwrap();
    assert_eq!(value["schema"], "geod-partial-transfer/v2");
    assert_eq!(value["kind"], ORIGINAL_KIND);
    assert_eq!(value["etag"], "");
    assert_eq!(value["expectedOriginalSha256"], original.archive_sha256);
    assert!(!String::from_utf8(serde_json::to_vec(&value).unwrap())
        .unwrap()
        .contains(&job.href));
    for n in 0..8 {
        let mut forged = value.clone();
        match n {
            0 => forged["expectedOriginalSha256"] = "0".repeat(64).into(),
            1 => forged["total"] = (original.archive_bytes + 1).into(),
            2 => forged["schema"] = "geod-partial-transfer/v1".into(),
            3 => forged["etag"] = "\"not-a-sha-etag\"".into(),
            4 => forged["kind"] = "client-sha256".into(),
            5 => forged["bytes"] = 0.into(),
            6 => forged["binding"] = "0".repeat(64).into(),
            _ => forged["unexpectedOverride"] = true.into(),
        };
        std::fs::write(&receipt, serde_json::to_vec(&forged).unwrap()).unwrap();
        assert!(
            !candidate(&root, &job, crate::MAX_ASSET_BYTES),
            "Forged checkpoint {n}"
        );
    }
    std::fs::write(&receipt, serde_json::to_vec(&value).unwrap()).unwrap();
    let mut forged = job.clone();
    forged.href.push_str("?alternate=1");
    assert!(!candidate(&root, &forged, crate::MAX_ASSET_BYTES));
    forged = job.clone();
    forged.stac_source = None;
    assert!(!candidate(&root, &forged, crate::MAX_ASSET_BYTES));
    assert!(!candidate(&root, &job, original.archive_bytes - 1));
    let unreviewed = Validator::ReviewedOriginal {
        expected_original_sha256: "0".repeat(64),
        total: original.archive_bytes,
    };
    let file = tokio::fs::OpenOptions::new()
        .write(true)
        .open(paths(&root, &job.id).unwrap().0)
        .await
        .unwrap();
    assert!(checkpoint(
        &root,
        &job,
        &file,
        Some(&unreviewed),
        1,
        &Sha256::new(),
        prefix
    )
    .await
    .is_err());
}

#[tokio::test]
async fn original_prefix_is_rehashed_and_copied_and_tampering_forces_fresh_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let (job, original) = reviewed_archive_job(&root);
    let prefix = b"PK\x03\x04offline-security-prefix-fixture";
    hash_checkpoint(&root, &job, &original, prefix).await;
    let (part, _) = paths(&root, &job.id).unwrap();
    let outside = tempfile::tempdir().unwrap();
    let linked = outside.path().join("original-part");
    std::fs::hard_link(&part, &linked).unwrap();
    let mut tail = std::fs::OpenOptions::new()
        .append(true)
        .open(&part)
        .unwrap();
    tail.write_all(b"crash-tail").unwrap();
    drop(tail);
    let prepared = prepare(
        &root,
        &job,
        crate::MAX_ASSET_BYTES,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(prepared.bytes, prefix.len() as u64);
    assert!(prepared.checkpoint.is_some());
    assert_eq!(
        format!("{:x}", prepared.hasher.clone().finalize()),
        format!("{:x}", Sha256::digest(prefix))
    );
    assert_eq!(std::fs::read(&part).unwrap(), prefix);
    assert_eq!(
        std::fs::read(&linked).unwrap(),
        [prefix.as_slice(), b"crash-tail"].concat()
    );
    drop(prepared);
    let mut tampered = prefix.to_vec();
    tampered[6] ^= 1;
    std::fs::write(&part, tampered).unwrap();
    let prepared = prepare(
        &root,
        &job,
        crate::MAX_ASSET_BYTES,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(prepared.bytes, 0);
    assert!(prepared.checkpoint.is_none() && prepared.restarted);
    assert!(!part.exists());
    assert!(!candidate(&root, &job, crate::MAX_ASSET_BYTES));
}

#[tokio::test]
async fn cancelling_prefix_verification_preserves_candidate_without_publishing_it() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let (job, original) = reviewed_archive_job(&root);
    hash_checkpoint(
        &root,
        &job,
        &original,
        b"PK\x03\x04offline-security-prefix-fixture",
    )
    .await;
    let token = CancellationToken::new();
    token.cancel();
    assert!(prepare(&root, &job, crate::MAX_ASSET_BYTES, &token)
        .await
        .is_err());
    assert!(candidate(&root, &job, crate::MAX_ASSET_BYTES));
    let files = std::fs::read_dir(root.join("assets"))
        .unwrap()
        .map(|v| v.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(!files.iter().any(|v| v.ends_with("resume-write.part")));
    assert!(job.output_path.is_none() && job.sha256.is_none());
}

fn hash_response(status: u16, headers: &[(&str, &str)]) -> Response {
    let mut builder = axum::http::Response::builder().status(status);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    Response::from(builder.body(vec![0u8; 5]).unwrap())
}

#[test]
fn original_recovery_requires_explicit_exact_range_and_has_no_if_range_or_etag_requirement() {
    let cp = Checkpoint {
        schema: "geod-partial-transfer/v2".into(),
        binding: "0".repeat(64),
        etag: String::new(),
        bytes: 16,
        total: 21,
        sha256: "0".repeat(64),
        kind: Some(ORIGINAL_KIND.into()),
        expected_original_sha256: Some("1".repeat(64)),
    };
    let good = [
        ("Content-Range", "bytes 16-20/21"),
        ("Content-Length", "5"),
        ("ETag", "W/\"weak\""),
    ];
    assert!(accepts(&hash_response(206, &good), &cp));
    assert!(accepts(&hash_response(206, &good[..2]), &cp));
    for n in 0..9 {
        let mut headers = good.to_vec();
        let mut status = 206;
        match n {
            0 => status = 200,
            1 => headers[0].1 = "bytes 17-20/21",
            2 => headers[0].1 = "bytes 16-19/21",
            3 => headers[0].1 = "bytes 16-20/22",
            4 => headers[1].1 = "6",
            5 => {
                headers.remove(1);
            }
            6 => headers.push(("Content-Range", "bytes 16-20/21")),
            7 => headers.push(("Content-Encoding", "gzip")),
            _ => headers.push(("Content-Type", "multipart/byteranges")),
        };
        assert!(
            !accepts(&hash_response(status, &headers), &cp),
            "Malformed original response {n}"
        );
    }
    let built = request(
        reqwest::Client::new().get("https://example.com/fixture"),
        Some(&cp),
    )
    .build()
    .unwrap();
    assert_eq!(built.headers()[header::RANGE], "bytes=16-");
    assert!(!built.headers().contains_key(header::IF_RANGE));
    let legacy = Checkpoint {
        schema: "geod-partial-transfer/v1".into(),
        binding: "0".repeat(64),
        etag: "\"v1\"".into(),
        bytes: 16,
        total: 21,
        sha256: "0".repeat(64),
        kind: None,
        expected_original_sha256: None,
    };
    let json = serde_json::to_value(&legacy).unwrap();
    assert_eq!(json.as_object().unwrap().len(), 6);
    assert!(
        !json.as_object().unwrap().contains_key("kind")
            && !json
                .as_object()
                .unwrap()
                .contains_key("expectedOriginalSha256")
    );
    assert!(matches!(
        serde_json::from_value::<Checkpoint>(json)
            .unwrap()
            .validator(),
        Validator::StrongEtag { .. }
    ));
}

#[tokio::test]
async fn full_original_identity_failure_is_not_a_resumable_checkpoint() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let (job, original) = reviewed_archive_job(&root);
    hash_checkpoint(
        &root,
        &job,
        &original,
        b"PK\x03\x04offline-security-prefix-fixture",
    )
    .await;
    // This is the existing completion order in JobManager::download: remove
    // recovery state before whole-file source/ZIP/member validation.
    discard_checkpoint(&root, &job.id).await.unwrap();
    assert!(
        crate::stac::verify_transfer(&root, &job, original.archive_bytes, &"0".repeat(64)).is_err()
    );
    assert!(crate::stac::verify_transfer(
        &root,
        &job,
        original.archive_bytes - 1,
        &original.archive_sha256
    )
    .is_err());
    assert!(!candidate(&root, &job, crate::MAX_ASSET_BYTES));
}

#[tokio::test]
#[ignore = "requires GEOD_REVIEWED_TRANSFER_ORIGINAL, real official HTTP and optional GEOD_TRANSFER_PROXY / GEOD_REVIEWED_TRANSFER_REPORT"]
async fn real_compiled_glc_original_range_recovery_retains_checkpoint_and_verifies_whole_source() {
    use futures_util::StreamExt;
    let original_path = PathBuf::from(std::env::var("GEOD_REVIEWED_TRANSFER_ORIGINAL").unwrap());
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let (job, original) = reviewed_archive_job(&root);
    // Seed only from an independently downloaded COMPLETE original, whose
    // whole hash is checked before creating a simulated interrupted prefix.
    // The actual HTTP operation below downloads the exact remaining bytes.
    let mut reference = std::fs::File::open(&original_path).unwrap();
    assert_eq!(reference.metadata().unwrap().len(), original.archive_bytes);
    let mut reference_hash = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let n = reference.read(&mut buffer).unwrap();
        if n == 0 {
            break;
        }
        reference_hash.update(&buffer[..n]);
    }
    assert_eq!(
        format!("{:x}", reference_hash.finalize()),
        original.archive_sha256
    );
    let prefix_bytes = original.archive_bytes - 8 * 1024 * 1024;
    let (part, _) = paths(&root, &job.id).unwrap();
    let mut source = std::fs::File::open(&original_path).unwrap();
    let mut prefix = std::fs::File::create(&part).unwrap();
    let mut left = prefix_bytes;
    let mut hasher = Sha256::new();
    let mut initial_header = Vec::new();
    while left > 0 {
        let n = buffer.len().min(left as usize);
        source.read_exact(&mut buffer[..n]).unwrap();
        prefix.write_all(&buffer[..n]).unwrap();
        hasher.update(&buffer[..n]);
        initial_header.extend(
            buffer[..n]
                .iter()
                .take(16usize.saturating_sub(initial_header.len())),
        );
        left -= n as u64;
    }
    prefix.sync_all().unwrap();
    drop(prefix);
    let validator = Validator::ReviewedOriginal {
        expected_original_sha256: original.archive_sha256.clone(),
        total: original.archive_bytes,
    };
    let file = tokio::fs::OpenOptions::new()
        .write(true)
        .open(&part)
        .await
        .unwrap();
    checkpoint(
        &root,
        &job,
        &file,
        Some(&validator),
        prefix_bytes,
        &hasher,
        &initial_header,
    )
    .await
    .unwrap();
    drop(file);
    assert!(candidate(&root, &job, crate::MAX_ASSET_BYTES));
    let proxy = std::env::var("GEOD_TRANSFER_PROXY").ok();
    let settings = ProxySettings {
        mode: if proxy.is_some() {
            crate::proxy::ProxyMode::Custom
        } else {
            crate::proxy::ProxyMode::Direct
        },
        url: proxy,
    };
    let client = crate::proxy::download_client(&settings).unwrap();
    let mut attempts = Vec::new();
    let mut completed = None;
    for attempt in 0..3 {
        let mut prepared = prepare(
            &root,
            &job,
            crate::MAX_ASSET_BYTES,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        let cp = prepared.checkpoint.as_ref().unwrap();
        let built = request(client.get(&job.href), Some(cp)).build().unwrap();
        let sent_range = built.headers()[header::RANGE].to_str().unwrap().to_string();
        assert!(!built.headers().contains_key(header::IF_RANGE));
        let response = client.execute(built).await.unwrap();
        let accepted = accepts(&response, cp);
        attempts.push(serde_json::json!({"attempt":attempt+1,"range":sent_range,"ifRangeSent":false,
            "status":response.status().as_u16(),"contentRange":response.headers().get(header::CONTENT_RANGE).and_then(|v|v.to_str().ok()),
            "contentLength":response.headers().get(header::CONTENT_LENGTH).and_then(|v|v.to_str().ok()),"accepted":accepted,
            "verifiedPrefixBytes":prepared.bytes}));
        assert!(
            accepted,
            "Official source did not return the exact bounded byte range: {:?}",
            attempts
        );
        let mut stream = response.bytes_stream();
        let mut file = prepared.file.take().unwrap();
        let mut bytes = prepared.bytes;
        let mut hasher = prepared.hasher;
        let mut failed = false;
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(chunk) => {
                    bytes += chunk.len() as u64;
                    if bytes > original.archive_bytes {
                        discard_checkpoint(&root, &job.id).await.unwrap();
                        panic!("Range body exceeded complete original");
                    }
                    file.write_all(&chunk).await.unwrap();
                    hasher.update(&chunk);
                }
                Err(_) => {
                    failed = true;
                    break;
                }
            }
        }
        if failed || bytes != original.archive_bytes {
            checkpoint(
                &root,
                &job,
                &file,
                Some(&validator),
                bytes,
                &hasher,
                &initial_header,
            )
            .await
            .unwrap();
            drop(file);
            assert!(candidate(&root, &job, crate::MAX_ASSET_BYTES));
            continue;
        }
        discard_checkpoint(&root, &job.id).await.unwrap();
        file.sync_all().await.unwrap();
        drop(file);
        let sha = format!("{:x}", hasher.finalize());
        assert_eq!(sha, original.archive_sha256);
        crate::stac::verify_transfer(&root, &job, bytes, &sha).unwrap();
        assert!(crate::stac::archive_original::prepare_download(
            &root,
            &job,
            &part,
            &CancellationToken::new()
        )
        .await
        .unwrap());
        assert!(!candidate(&root, &job, crate::MAX_ASSET_BYTES));
        completed = Some(
            serde_json::json!({"datasetId":"glc-fcs10","originalUrl":job.href,
            "originalBytes":bytes,"originalSha256":sha,"wholeOriginalVerified":true,
            "recoveredPrefixBytes":prefix_bytes,"initialPrefixOrigin":"independent complete original, whole SHA verified before seeding interrupted checkpoint",
            "actualRemainingHttpBytes":bytes-prefix_bytes,"nativeRangeAttempts":attempts,
            "noIfRange":true,"nativeMemberVerified":true,"selectedMember":original.member,
            "memberBytes":original.member_bytes,"memberSha256":original.member_sha256,
            "checkpointRemovedAfterWholeValidation":true,"newServerListener":false}),
        );
        break;
    }
    let report =
        completed.expect("Official range transfer did not complete after three bounded retries");
    if let Ok(path) = std::env::var("GEOD_REVIEWED_TRANSFER_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
