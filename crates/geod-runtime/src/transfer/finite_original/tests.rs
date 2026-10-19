//! Loopback payloads below exercise byte recovery, not real ZIP/raster science.
use super::*;
use crate::{storage, CreateJobRequest};
use sha2::Sha256;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use tokio::{io::AsyncReadExt, net::TcpListener};

#[test]
fn first_clcd_interval_requires_tiff_and_keeps_sinolc_zip_separate() {
    // This exercises first-interval format selection, not source admission or
    // completion: the synthetic SHA cannot grant a CLCD transfer capability.
    let source = Source {
        url: crate::datasets::clcd::URL.into(),
        total: crate::datasets::clcd::ORIGINAL_BYTES,
        sha: "a".repeat(64),
    };
    let tiff = b"II\x2a\0\x08\0\0\0";
    let zip = b"PK\x03\x04\0\0\0\0";
    assert!(verify_source_signature(&source, tiff).is_ok());
    assert!(verify_source_signature(&source, zip).is_err());
    let sino = Source {
        url: URL.into(),
        total: TOTAL,
        sha: SHA.into(),
    };
    assert!(verify_source_signature(&sino, zip).is_ok());
    assert!(verify_source_signature(&sino, tiff).is_err());
    let alias = Source {
        url: format!("{}?alias=1", source.url),
        ..source
    };
    assert!(verify_source_signature(&alias, tiff).is_err());
}

struct Server {
    url: String,
    requests: Arc<Mutex<Vec<FiniteRange>>>,
    max_active: Arc<AtomicUsize>,
    interrupt_later: Arc<AtomicBool>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn server(body: Vec<u8>, behavior: &'static str) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/original", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    let body = Arc::new(body);
    let active = Arc::new(AtomicUsize::new(0));
    let max_active = Arc::new(AtomicUsize::new(0));
    let maximum = max_active.clone();
    let attempt = Arc::new(AtomicUsize::new(0));
    let interrupt_later = Arc::new(AtomicBool::new(behavior == "interrupt-later"));
    let later = interrupt_later.clone();
    let task = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let captured = captured.clone();
            let body = body.clone();
            let active = active.clone();
            let maximum = maximum.clone();
            let attempt = attempt.clone();
            let later = later.clone();
            tokio::spawn(async move {
                let mut request = vec![0; 8192];
                let mut size = 0;
                while !request[..size].windows(4).any(|b| b == b"\r\n\r\n") {
                    let n = socket.read(&mut request[size..]).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    size += n;
                }
                let request = String::from_utf8(request[..size].to_vec())
                    .unwrap()
                    .to_ascii_lowercase();
                assert!(
                    request.contains("accept-encoding: identity") && !request.contains("if-range:")
                );
                let text = request
                    .lines()
                    .find_map(|s| s.strip_prefix("range: bytes="))
                    .unwrap();
                let (start, end) = text.split_once('-').unwrap();
                assert!(!end.is_empty());
                let r = FiniteRange {
                    start: start.parse().unwrap(),
                    end: end.parse().unwrap(),
                    total: body.len() as u64,
                };
                captured.lock().unwrap().push(r);
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                maximum.fetch_max(current, Ordering::SeqCst);
                let n = attempt.fetch_add(1, Ordering::SeqCst);
                let transient = match behavior {
                    "408-once" if n == 0 => Some("408 Request Timeout"),
                    "429-once" if n == 0 => Some("429 Too Many Requests"),
                    "503-once" if n == 0 => Some("503 Service Unavailable"),
                    "408-always" => Some("408 Request Timeout"),
                    "429-always" => Some("429 Too Many Requests"),
                    "503-always" => Some("503 Service Unavailable"),
                    _ => None,
                };
                if let Some(status) = transient {
                    // Deliberately not a valid ZIP representation. A retryable
                    // status must discard this body before type/range checks.
                    let html = b"<html>retry</html>";
                    let headers = format!("HTTP/1.1 {status}\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", html.len());
                    let _ = socket.write_all(headers.as_bytes()).await;
                    let _ = socket.write_all(html).await;
                    let _ = socket.shutdown().await;
                    active.fetch_sub(1, Ordering::SeqCst);
                    return;
                }
                let status = match behavior {
                    "wrong-status" => "200 OK",
                    "redirect" => "302 Found",
                    "unauthorized" => "401 Unauthorized",
                    "forbidden" => "403 Forbidden",
                    "precondition" => "412 Precondition Failed",
                    "range-unsatisfied" => "416 Range Not Satisfiable",
                    _ => "206 Partial Content",
                };
                let media = if behavior == "html" {
                    "text/html"
                } else {
                    "application/zip"
                };
                let total = if behavior == "wrong-total" {
                    r.total + 1
                } else {
                    r.total
                };
                let headers=format!("HTTP/1.1 {status}\r\nContent-Type: {media}\r\nContent-Range: bytes {}-{}/{total}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",r.start,r.end,r.length());
                let _ = socket.write_all(headers.as_bytes()).await;
                if behavior == "hold" {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                } else if (behavior == "interrupt-once" && n == 0)
                    || (later.load(Ordering::SeqCst) && r.start >= 8 * 1024 * 1024)
                {
                    let _ = socket
                        .write_all(&body[r.start as usize..r.start as usize + 13.min(r.length())])
                        .await;
                } else {
                    if behavior == "out-of-order" && r.start == 0 {
                        tokio::time::sleep(Duration::from_millis(80)).await;
                    }
                    let _ = socket
                        .write_all(&body[r.start as usize..=r.end as usize])
                        .await;
                }
                let _ = socket.shutdown().await;
                active.fetch_sub(1, Ordering::SeqCst);
            });
        }
    });
    Server {
        url,
        requests,
        max_active,
        interrupt_later,
        task,
    }
}
struct Http {
    client: reqwest::Client,
    url: String,
}
impl Transport for Http {
    fn request(&self, r: FiniteRange) -> ResponseFuture<'_> {
        Box::pin(async move {
            self.client
                .get(&self.url)
                .header("Range", r.header())
                .header("Accept-Encoding", "identity")
                .send()
                .await
                .map_err(io_error)
        })
    }
}
fn payload(bytes: usize) -> Vec<u8> {
    let mut v = (0..bytes).map(|n| (n % 251) as u8).collect::<Vec<_>>();
    v[..4].copy_from_slice(b"PK\x03\x04");
    v
}
fn fixture_job(url: &str) -> Job {
    crate::new_download_job(CreateJobRequest {
        item_id: "finite-range-transport-fixture".into(),
        asset_key: "stac_asset".into(),
        href: url.into(),
        media_type: "application/zip".into(),
        title: None,
    })
}
struct Log {
    root: std::path::PathBuf,
    job: Job,
    commits: Vec<u64>,
}
impl Journal for Log {
    fn commit<'a>(&'a mut self, state: &'a CompletedDownload) -> CommitFuture<'a> {
        Box::pin(async move {
            super::super::checkpoint(
                &self.root,
                &self.job,
                &state.file,
                state.validator.as_ref(),
                state.bytes,
                &state.hasher,
                &state.header,
            )
            .await?;
            self.commits.push(state.bytes);
            Ok(())
        })
    }
}
async fn state(root: &Path, job: &Job, total: u64, prefix: &[u8]) -> CompletedDownload {
    std::fs::create_dir_all(root.join("assets")).unwrap();
    let file = tokio::fs::File::create(root.join("assets").join(format!("{}.part", job.id)))
        .await
        .unwrap();
    let mut state = CompletedDownload {
        file,
        hasher: Sha256::new(),
        bytes: 0,
        header: vec![],
        total: Some(total),
        validator: Some(Validator::StrongEtag {
            etag: "\"fixture-v1\"".into(),
            total,
        }),
    };
    if !prefix.is_empty() {
        state.file.write_all(prefix).await.unwrap();
        state.hasher.update(prefix);
        state.bytes = prefix.len() as u64;
        state.header = prefix[..16.min(prefix.len())].to_vec();
        state.file.sync_all().await.unwrap();
    }
    state
}
fn http(server: &Server) -> Http {
    Http {
        url: server.url.clone(),
        client: reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap(),
    }
}
fn fixture_source(server: &Server, body: &[u8]) -> Source {
    Source {
        url: server.url.clone(),
        total: body.len() as u64,
        sha: format!("{:x}", Sha256::digest(body)),
    }
}

#[test]
fn closed_ranges_cover_fixed_original_and_non_aligned_prefix_exactly() {
    let full = ranges(0, TOTAL).unwrap();
    assert_eq!(full.len(), 44);
    assert_eq!(
        full[43],
        FiniteRange {
            start: 360710144,
            end: 365818021,
            total: TOTAL
        }
    );
    assert_eq!(full[43].length(), 5107878);
    assert!(full
        .iter()
        .all(|r| r.length() <= 8 * 1024 * 1024 && !r.header().ends_with('-')));
    let resumed = ranges(216825356, TOTAL).unwrap();
    assert_eq!(resumed[0].start, 216825356);
    assert!(resumed.windows(2).all(|r| r[0].end + 1 == r[1].start));
    assert_eq!(resumed.last().unwrap().end, TOTAL - 1);
}
fn response(status: u16, headers: &[(&str, &str)]) -> reqwest::Response {
    let mut b = axum::http::Response::builder().status(status);
    for (n, v) in headers {
        b = b.header(*n, *v);
    }
    reqwest::Response::from(b.body(vec![0; 5]).unwrap())
}
#[test]
fn finite_headers_reject_changed_ranges_lengths_html_encodings_and_ambiguous_representation() {
    let r = FiniteRange {
        start: 16,
        end: 20,
        total: 21,
    };
    let good = [
        ("Content-Range", "bytes 16-20/21"),
        ("Content-Length", "5"),
        ("Content-Type", "application/zip"),
    ];
    assert!(super::super::validate_finite_response(&response(206, &good), r).is_ok());
    let mut duplicate_type = good.to_vec();
    duplicate_type.push(("Content-Type", "application/zip"));
    assert!(super::super::validate_finite_response(&response(206, &duplicate_type), r).is_ok());
    let mut native_duplicate_type = good.to_vec();
    native_duplicate_type[2].1 = "application/octet-stream";
    native_duplicate_type.push(("Content-Type", "application/octet-stream"));
    assert!(
        super::super::validate_finite_response(&response(206, &native_duplicate_type), r).is_ok()
    );
    for n in 0..15 {
        let mut h = good.to_vec();
        let mut s = 206;
        match n {
            0 => s = 200,
            1 => s = 302,
            2 => s = 412,
            3 => s = 416,
            4 => h[0].1 = "bytes 17-20/21",
            5 => h[0].1 = "bytes 16-19/21",
            6 => h[0].1 = "bytes 16-20/22",
            7 => h[1].1 = "6",
            8 => {
                h.remove(1);
            }
            9 => h.push(("Content-Range", "bytes 16-20/21")),
            10 => h.push(("Content-Length", "5")),
            11 => h[2].1 = "text/html",
            12 => h.push(("Content-Encoding", "gzip")),
            13 => h.push(("Transfer-Encoding", "chunked")),
            _ => h.push(("Content-Type", "application/octet-stream")),
        }
        assert!(
            super::super::validate_finite_response(&response(s, &h), r).is_err(),
            "invalid header {n}"
        );
    }
    let mut h = good.to_vec();
    h.push(("Content-Encoding", "identity"));
    h.push(("Content-Encoding", "identity"));
    assert!(super::super::validate_finite_response(&response(206, &h), r).is_err());
}
#[tokio::test]
async fn complete_loopback_ranges_commit_in_order_with_at_most_three_requests_and_whole_sha() {
    let body = payload(25 * 1024 * 1024 + 19);
    let server = server(body.clone(), "out-of-order").await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let job = fixture_job(&server.url);
    let mut log = Log {
        root: root.clone(),
        job: job.clone(),
        commits: vec![],
    };
    let mut state = state(&root, &job, body.len() as u64, &[]).await;
    run(
        &fixture_source(&server, &body),
        &mut state,
        &http(&server),
        &mut log,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(
        log.commits,
        vec![
            8 * 1024 * 1024,
            16 * 1024 * 1024,
            24 * 1024 * 1024,
            body.len() as u64
        ]
    );
    assert!(server.max_active.load(Ordering::SeqCst) <= 3);
    assert_eq!(
        tokio::fs::read(root.join("assets").join(format!("{}.part", job.id)))
            .await
            .unwrap(),
        body
    );
    assert_eq!(
        format!("{:x}", state.hasher.finalize()),
        format!("{:x}", Sha256::digest(body))
    );
    assert_eq!(server.requests.lock().unwrap().len(), 4);
}
#[tokio::test]
async fn interrupted_segment_retries_without_committing_partial_bytes() {
    let body = payload(2048);
    let server = server(body.clone(), "interrupt-once").await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let job = fixture_job(&server.url);
    let mut log = Log {
        root: root.clone(),
        job: job.clone(),
        commits: vec![],
    };
    let mut s = state(&root, &job, body.len() as u64, &[]).await;
    run(
        &fixture_source(&server, &body),
        &mut s,
        &http(&server),
        &mut log,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(log.commits, [body.len() as u64]);
    assert_eq!(server.requests.lock().unwrap().len(), 2);
}
#[tokio::test]
async fn transient_http_error_pages_retry_then_commit_only_valid_zip_bytes() {
    for behavior in ["408-once", "429-once", "503-once"] {
        let body = payload(128);
        let server = server(body.clone(), behavior).await;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let job = fixture_job(&server.url);
        let mut log = Log {
            root: root.clone(),
            job: job.clone(),
            commits: vec![],
        };
        let mut s = state(&root, &job, body.len() as u64, &body[..17]).await;
        run(
            &fixture_source(&server, &body),
            &mut s,
            &http(&server),
            &mut log,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(log.commits, [body.len() as u64]);
        assert_eq!(server.requests.lock().unwrap().len(), 2);
        assert_eq!(
            tokio::fs::read(root.join("assets").join(format!("{}.part", job.id)))
                .await
                .unwrap(),
            body
        );
    }
}
#[tokio::test]
async fn transient_http_errors_exhaust_exactly_three_attempts_without_touching_verified_prefix() {
    for behavior in ["408-always", "429-always", "503-always"] {
        let body = payload(128);
        let server = server(body.clone(), behavior).await;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let job = fixture_job(&server.url);
        let mut log = Log {
            root: root.clone(),
            job: job.clone(),
            commits: vec![],
        };
        let mut s = state(&root, &job, body.len() as u64, &body[..17]).await;
        log.commit(&s).await.unwrap();
        let error = run(
            &fixture_source(&server, &body),
            &mut s,
            &http(&server),
            &mut log,
            &CancellationToken::new(),
        )
        .await
        .unwrap_err();
        assert!(error.contains(&behavior[..3]), "{error}");
        assert_eq!(server.requests.lock().unwrap().len(), ATTEMPTS);
        assert_eq!(s.bytes, 17);
        assert_eq!(log.commits, [17]);
        assert_eq!(
            tokio::fs::read(root.join("assets").join(format!("{}.part", job.id)))
                .await
                .unwrap(),
            body[..17]
        );
        assert!(super::super::candidate(&root, &job, crate::MAX_ASSET_BYTES));
    }
}
#[tokio::test]
async fn cancel_during_transient_backoff_stops_before_a_second_request() {
    let body = payload(128);
    let server = server(body.clone(), "429-always").await;
    let token = CancellationToken::new();
    let cancel = token.clone();
    let seen = server.requests.clone();
    tokio::spawn(async move {
        while seen.lock().unwrap().is_empty() {
            tokio::task::yield_now().await;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        cancel.cancel();
    });
    let result = tokio::time::timeout(
        Duration::from_millis(700),
        fetch(
            &http(&server),
            &fixture_source(&server, &body),
            FiniteRange::new(0, body.len() as u64).unwrap(),
            &token,
        ),
    )
    .await
    .unwrap();
    assert!(matches!(result, Err(error) if error == "Transfer cancelled"));
    assert_eq!(server.requests.lock().unwrap().len(), 1);
}
#[tokio::test]
async fn final_journal_failure_preserves_and_rehashes_the_last_durable_prefix() {
    struct FailedJournal;
    impl Journal for FailedJournal {
        fn commit<'a>(&'a mut self, _: &'a CompletedDownload) -> CommitFuture<'a> {
            Box::pin(async { Err("Injected final journal write failure".into()) })
        }
    }
    let body = payload(128);
    let server = server(body.clone(), "valid").await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let job = fixture_job(&server.url);
    let source = fixture_source(&server, &body);
    let mut log = Log {
        root: root.clone(),
        job: job.clone(),
        commits: vec![],
    };
    let mut s = state(&root, &job, body.len() as u64, &body[..17]).await;
    log.commit(&s).await.unwrap();
    let error = run(
        &source,
        &mut s,
        &http(&server),
        &mut FailedJournal,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(error, "Injected final journal write failure");
    assert_eq!(s.bytes, body.len() as u64);
    assert_eq!(format!("{:x}", s.hasher.clone().finalize()), source.sha);
    discard_invalid_complete_checkpoint(&root, &job, &source, &s)
        .await
        .unwrap();
    drop(s);
    assert!(super::super::candidate(&root, &job, crate::MAX_ASSET_BYTES));
    let p = super::super::prepare(
        &root,
        &job,
        crate::MAX_ASSET_BYTES,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(p.bytes, 17);
    assert_eq!(
        tokio::fs::read(root.join("assets").join(format!("{}.part", job.id)))
            .await
            .unwrap(),
        body[..17]
    );
}
#[tokio::test]
async fn existing_non_aligned_checkpoint_is_rehashed_and_resumes_same_prefix() {
    let body = payload(2 * 1024 * 1024 + 29);
    let server = server(body.clone(), "valid").await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let job = fixture_job(&server.url);
    let prefix = 1031;
    let s = state(&root, &job, body.len() as u64, &body[..prefix]).await;
    let mut log = Log {
        root: root.clone(),
        job: job.clone(),
        commits: vec![],
    };
    log.commit(&s).await.unwrap();
    drop(s);
    let mut p = super::super::prepare(
        &root,
        &job,
        crate::MAX_ASSET_BYTES,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(p.bytes, prefix as u64);
    let mut s = CompletedDownload {
        file: p.file.take().unwrap(),
        hasher: p.hasher,
        bytes: p.bytes,
        header: p.header,
        total: Some(body.len() as u64),
        validator: Some(p.checkpoint.unwrap().validator()),
    };
    run(
        &fixture_source(&server, &body),
        &mut s,
        &http(&server),
        &mut log,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(server.requests.lock().unwrap()[0].start, prefix as u64);
    assert_eq!(
        format!("{:x}", s.hasher.finalize()),
        format!("{:x}", Sha256::digest(body))
    );
}
#[tokio::test]
async fn wrong_headers_and_changed_source_do_not_commit_or_retry() {
    for behavior in [
        "wrong-status",
        "redirect",
        "unauthorized",
        "forbidden",
        "precondition",
        "range-unsatisfied",
        "wrong-total",
        "html",
    ] {
        let body = payload(128);
        let server = server(body.clone(), behavior).await;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let job = fixture_job(&server.url);
        let mut log = Log {
            root: root.clone(),
            job: job.clone(),
            commits: vec![],
        };
        let mut s = state(&root, &job, 128, &body[..17]).await;
        let source = fixture_source(&server, &body);
        assert!(run(
            &source,
            &mut s,
            &http(&server),
            &mut log,
            &CancellationToken::new()
        )
        .await
        .is_err());
        assert_eq!(s.bytes, 17);
        assert!(log.commits.is_empty());
        assert_eq!(server.requests.lock().unwrap().len(), 1);
    }
    let body = payload(128);
    let server = server(body.clone(), "valid").await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let job = fixture_job(&server.url);
    let mut s = state(&root, &job, 128, &[]).await;
    let mut source = fixture_source(&server, &body);
    source.url.push_str("?replacement=1");
    let mut log = Log {
        root,
        job,
        commits: vec![],
    };
    assert!(run(
        &source,
        &mut s,
        &http(&server),
        &mut log,
        &CancellationToken::new()
    )
    .await
    .is_err());
    assert_eq!(s.bytes, 0);
}
#[tokio::test]
async fn collector_rejects_oversized_and_short_streams_before_buffering_or_commit() {
    let r = FiniteRange {
        start: 0,
        end: 4,
        total: 5,
    };
    let token = CancellationToken::new();
    assert!(matches!(
        collect(stream::iter([Ok(Bytes::from_static(b"123456"))]), r, &token).await,
        Err(Failure::Fatal(_))
    ));
    assert!(matches!(
        collect(stream::iter([Ok(Bytes::from_static(b"1234"))]), r, &token).await,
        Err(Failure::Fatal(_))
    ));
    token.cancel();
    assert!(
        collect(stream::iter([Ok(Bytes::from_static(b"12345"))]), r, &token)
            .await
            .is_err()
    );
}
#[tokio::test]
async fn cancel_mid_segment_retains_only_preexisting_verified_checkpoint() {
    let body = payload(2048);
    let server = server(body.clone(), "hold").await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let job = fixture_job(&server.url);
    let mut s = state(&root, &job, 2048, &body[..17]).await;
    let mut log = Log {
        root: root.clone(),
        job: job.clone(),
        commits: vec![],
    };
    log.commit(&s).await.unwrap();
    let token = CancellationToken::new();
    let cancel = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(60)).await;
        cancel.cancel();
    });
    assert!(run(
        &fixture_source(&server, &body),
        &mut s,
        &http(&server),
        &mut log,
        &token
    )
    .await
    .is_err());
    assert_eq!(log.commits, [17]);
    assert_eq!(s.bytes, 17);
    assert!(super::super::candidate(&root, &job, crate::MAX_ASSET_BYTES));
}
#[tokio::test]
async fn complete_prefix_needs_no_requests_and_bad_whole_sha_is_rejected() {
    let body = payload(128);
    let server = server(body.clone(), "valid").await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let job = fixture_job(&server.url);
    let mut s = state(&root, &job, 128, &body).await;
    let mut source = fixture_source(&server, &body);
    let mut log = Log {
        root,
        job,
        commits: vec![],
    };
    run(
        &source,
        &mut s,
        &http(&server),
        &mut log,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(server.requests.lock().unwrap().is_empty());
    source.sha = "0".repeat(64);
    assert!(run(
        &source,
        &mut s,
        &http(&server),
        &mut log,
        &CancellationToken::new()
    )
    .await
    .is_err());
    assert!(server.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn exhausted_network_segment_keeps_completed_prefix_and_retry_resumes_same_source() {
    let body = payload(17 * 1024 * 1024 + 41);
    let server = server(body.clone(), "interrupt-later").await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let job = fixture_job(&server.url);
    let mut log = Log {
        root: root.clone(),
        job: job.clone(),
        commits: vec![],
    };
    let mut s = state(&root, &job, body.len() as u64, &[]).await;
    assert!(run(
        &fixture_source(&server, &body),
        &mut s,
        &http(&server),
        &mut log,
        &CancellationToken::new()
    )
    .await
    .is_err());
    assert_eq!(log.commits, [8 * 1024 * 1024]);
    assert_eq!(s.bytes, 8 * 1024 * 1024);
    drop(s);
    assert!(super::super::candidate(&root, &job, crate::MAX_ASSET_BYTES));
    server.interrupt_later.store(false, Ordering::SeqCst);
    let mut p = super::super::prepare(
        &root,
        &job,
        crate::MAX_ASSET_BYTES,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(p.bytes, 8 * 1024 * 1024);
    let before = server.requests.lock().unwrap().len();
    let mut s = CompletedDownload {
        file: p.file.take().unwrap(),
        hasher: p.hasher,
        bytes: p.bytes,
        header: p.header,
        total: Some(body.len() as u64),
        validator: Some(p.checkpoint.unwrap().validator()),
    };
    run(
        &fixture_source(&server, &body),
        &mut s,
        &http(&server),
        &mut log,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(server.requests.lock().unwrap()[before..]
        .iter()
        .all(|r| r.start >= 8 * 1024 * 1024));
    assert_eq!(
        format!("{:x}", s.hasher.finalize()),
        format!("{:x}", Sha256::digest(body))
    );
}

#[tokio::test]
async fn native_complete_wrong_sha_discards_checkpoint_and_cannot_become_a_retry_candidate() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let manager = JobManager::open(&root).await.unwrap();
    let mut job = native_job(&root);
    job.status = crate::JobStatus::Running;
    manager
        .inner
        .store
        .lock()
        .await
        .jobs
        .insert(job.id.clone(), job.clone());
    let prefix = b"PK\x03\x04explicit-invalid-final-hash-fixture";
    let mut s = state(&root, &job, TOTAL, prefix).await;
    s.validator = Some(Validator::ReviewedOriginal {
        expected_original_sha256: SHA.into(),
        total: TOTAL,
    });
    super::super::checkpoint(
        &root,
        &job,
        &s.file,
        s.validator.as_ref(),
        s.bytes,
        &s.hasher,
        &s.header,
    )
    .await
    .unwrap();
    drop(s);
    assert!(super::super::candidate(&root, &job, crate::MAX_ASSET_BYTES));
    let mut p = super::super::prepare(
        &root,
        &job,
        crate::MAX_ASSET_BYTES,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    // Explicitly invalid caller state: reported complete, but the checked prefix
    // hash cannot equal the real compiled original. No asset request is needed.
    p.bytes = TOTAL;
    let result = download(
        &manager,
        &ProxySettings {
            mode: crate::proxy::ProxyMode::Direct,
            url: None,
        },
        &job,
        &mut p,
        &CancellationToken::new(),
    )
    .await;
    assert!(result.is_err());
    assert!(!super::super::candidate(
        &root,
        &job,
        crate::MAX_ASSET_BYTES
    ));
    assert!(manager.get(&job.id).await.unwrap().output_path.is_none());
}

// An offline native snapshot/proof exercises the standard software job shape;
// no real source bytes or network response are fabricated by this helper.
fn native_job(root: &Path) -> Job {
    use serde_json::json;
    use uuid::Uuid;
    std::fs::create_dir_all(root.join("stac")).unwrap();
    std::fs::create_dir_all(root.join("assets")).unwrap();
    let plan = crate::datasets::resolve(crate::datasets::ResolveRequest {
        dataset_id: "sinolc-1".into(),
        bounds: [121.4, 31.1, 121.6, 31.3],
        year: None,
        month: None,
        country: None,
        layer: Some("classes".into()),
        region: None,
        season: None,
    })
    .unwrap();
    let source = &plan.sources[0];
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
    let original = crate::stac::archive_original::from_proof(&proof, URL)
        .unwrap()
        .unwrap();
    let raw = json!({"kind":"reviewed-raster-archive","href":URL,"probeStatus":206,"contentRange":format!("bytes 0-15/{TOTAL}"),"contentLength":16,"headerHex":"504b0304","reviewedArchive":original,"datasetPreset":proof,"file:size":TOTAL,"file:checksum":format!("1220{SHA}")});
    let bytes = serde_json::to_vec(&raw).unwrap();
    let document_sha = format!("{:x}", Sha256::digest(&bytes));
    std::fs::write(
        root.join("stac")
            .join(format!("document-{document_sha}.json")),
        bytes,
    )
    .unwrap();
    let record = json!({"version":1,"connectionId":Uuid::new_v4().to_string(),"serviceName":source.title,"kind":"raster","documentUrl":URL,"documentSha256":document_sha,"itemIndex":null,"retrievedAt":crate::now(),"metadataDocuments":[],"search":null});
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
    let mut job = fixture_job(&item.href);
    job.item_id = item.item_id;
    job.title = item.title;
    job.media_type = item.media_type;
    job.source = item.service_name;
    job.stac_source = Some(pin);
    job
}
#[test]
fn standard_stac_asset_job_with_native_sino_proof_is_the_only_eligible_source() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let job = native_job(&root);
    assert_eq!(job.asset_key, "stac_asset");
    assert_eq!(job.stac_source.as_ref().unwrap().asset_key, "raster");
    assert!(source(&root, &job).unwrap().is_some());
    for n in 0..5 {
        let mut changed = job.clone();
        match n {
            0 => changed.stac_source = None,
            1 => changed.href = "https://example.com/East_Shanghai.zip".into(),
            2 => changed.asset_key = "raster".into(),
            3 => changed.stac_source.as_mut().unwrap().asset_key = "other".into(),
            _ => changed.media_type = "image/tiff".into(),
        };
        assert!(source(&root, &changed).unwrap().is_none());
    }
    let mut forged = job;
    forged.stac_source.as_mut().unwrap().snapshot_id = "0".repeat(64);
    assert!(source(&root, &forged).is_err());
}
#[test]
#[ignore = "read-only current QA native Sino source gate; requires GEOD_SINO_GATE_JOB_JSON, GEOD_SINO_GATE_STORE, GEOD_SINO_GATE_RECEIPT"]
fn actual_failed_qa_job_enters_the_compiled_finite_gate_without_asset_request() {
    let job_path = std::env::var("GEOD_SINO_GATE_JOB_JSON").unwrap();
    let root = std::path::PathBuf::from(std::env::var("GEOD_SINO_GATE_STORE").unwrap())
        .canonicalize()
        .unwrap();
    let receipt = std::env::var("GEOD_SINO_GATE_RECEIPT").unwrap();
    let job: Job = serde_json::from_slice(&std::fs::read(job_path).unwrap()).unwrap();
    let selected = source(&root, &job)
        .unwrap()
        .expect("Real standard STAC job must enter Some");
    assert_eq!(job.id, "5126af30-4d4e-49b4-a255-7fbb91e25727");
    assert_eq!(selected.total, TOTAL);
    assert_eq!(selected.sha, SHA);
    let raw = serde_json::json!({"jobId":job.id,"assetKey":job.asset_key,"sourcePin":job.stac_source,"finiteGate":"Some","sourceUrl":selected.url,"originalBytes":selected.total,"originalSha256":selected.sha,"networkRequests":0,"runtimeTouched":false});
    std::fs::write(receipt, serde_json::to_vec_pretty(&raw).unwrap()).unwrap();
    assert!(storage::regular_file(&root.join("assets").join(format!("{}.part", job.id))).is_ok());
}

#[tokio::test]
#[ignore = "real bounded GET headers only with current stored proxy; GEOD_SINO_GATE_JOB_JSON/STORE and GEOD_SINO_HEADER_REPORT"]
async fn native_current_proxy_closed_range_header_diagnosis() {
    let root = std::path::PathBuf::from(std::env::var("GEOD_SINO_GATE_STORE").unwrap())
        .canonicalize()
        .unwrap();
    let job: Job = serde_json::from_slice(
        &std::fs::read(std::env::var("GEOD_SINO_GATE_JOB_JSON").unwrap()).unwrap(),
    )
    .unwrap();
    let settings: ProxySettings =
        serde_json::from_slice(&std::fs::read(root.join("proxy-settings.json")).unwrap()).unwrap();
    let range = FiniteRange::new(216825356, TOTAL).unwrap();
    let response = tokio::time::timeout(
        Duration::from_secs(35),
        crate::stac::finite_asset_response(&settings, &job, &root, range),
    )
    .await
    .unwrap()
    .unwrap();
    let names = [
        "content-range",
        "content-length",
        "content-type",
        "content-encoding",
        "transfer-encoding",
        "cache-control",
        "age",
        "vary",
        "x-cache",
        "x-webcache-source",
        "server",
    ];
    let headers = names
        .iter()
        .map(|name| {
            (
                (*name).to_string(),
                response
                    .headers()
                    .get_all(*name)
                    .iter()
                    .map(|v| v.to_str().unwrap_or("non-UTF8").to_string())
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut checks = std::collections::BTreeMap::new();
    let h = response.headers();
    checks.insert(
        "status206",
        response.status() == reqwest::StatusCode::PARTIAL_CONTENT,
    );
    checks.insert("sourceUrlExact", response.url().as_str() == URL);
    checks.insert(
        "singleContentRange",
        h.get_all("content-range").iter().count() == 1,
    );
    checks.insert(
        "rangeExact",
        h.get("content-range")
            .and_then(|v| v.to_str().ok())
            .and_then(super::super::range)
            == Some((range.start, range.end, range.total)),
    );
    checks.insert(
        "singleContentLength",
        h.get_all("content-length").iter().count() == 1,
    );
    checks.insert(
        "explicitLengthExact",
        h.get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(super::super::number)
            == Some(range.length() as u64),
    );
    checks.insert(
        "responseLengthExact",
        response.content_length() == Some(range.length() as u64),
    );
    checks.insert("noTransferEncoding", !h.contains_key("transfer-encoding"));
    checks.insert(
        "encodingIdentity",
        h.get_all("content-encoding").iter().count() <= 1
            && h.get("content-encoding").is_none_or(|v| v == "identity"),
    );
    let verdict = super::super::validate_finite_response(&response, range);
    let result = serde_json::json!({"sourceUrl":URL,"requestedRange":range.header(),"total":TOTAL,"status":response.status().as_u16(),"headers":headers,"checks":checks,"guardAccepted":verdict.is_ok(),"guardError":verdict.err(),"bodyReadBytes":0,"proxyMode":settings.mode,"proxyCredentialsDisclosed":false});
    drop(response);
    std::fs::write(
        std::env::var("GEOD_SINO_HEADER_REPORT").unwrap(),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    println!("{}", serde_json::to_string(&result).unwrap());
}
