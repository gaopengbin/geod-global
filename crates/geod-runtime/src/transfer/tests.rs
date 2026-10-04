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
                let first = attempt == 1;
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
