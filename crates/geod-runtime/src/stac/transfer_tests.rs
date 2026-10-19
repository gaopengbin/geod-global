//! Local HTTP transport fixtures, not live publisher or raster-source acceptance.
//! Original public URLs and immutable metadata still pass the production checks.
use super::*;
use crate::{transfer, CreateJobRequest, JobManager, JobStatus, ProxySettings};
use std::sync::{Arc, Mutex};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use tokio_util::sync::CancellationToken;

const PUBLIC_ITEM: &str = "https://example.com/item.json";
const PUBLIC_ASSET: &str = "https://example.com/source.tif";
const BODY: &[u8] = include_bytes!("../../fixtures/tiled-rgb.tif");
const PREFIX: usize = 128;

// This seam is cfg(test) only and cannot be selected by a production setting.
fn fixture_url(url: &Url, origin: &str) -> Result<Url> {
    public_url(url.as_str())?;
    let mut target = Url::parse(origin).map_err(io_error)?;
    if target.scheme() != "http"
        || target.host_str() != Some("127.0.0.1")
        || !target.username().is_empty()
        || target.password().is_some()
        || target.query().is_some()
        || target.fragment().is_some()
    {
        return Err("STAC test transport requires its explicit loopback origin".into());
    }
    target.set_path(url.path());
    target.set_query(url.query());
    Ok(target)
}

pub(super) fn fixture_request(
    settings: &ProxySettings,
    url: &Url,
    origin: &str,
) -> Result<reqwest::RequestBuilder> {
    Ok(crate::proxy::download_client(settings)?.get(fixture_url(url, origin)?))
}

pub(super) async fn fetch_fixture(
    settings: &ProxySettings,
    request: &MetadataRequest,
    origin: &str,
) -> Result<Vec<u8>> {
    let url = request.validate()?;
    let client = crate::proxy::download_client(settings)?;
    let mut builder = match request.method {
        search::SearchMethod::Get => client.get(fixture_url(&url, origin)?),
        search::SearchMethod::Post => client.post(fixture_url(&url, origin)?),
    };
    if let Some(body) = &request.body {
        builder = builder
            .header("Content-Type", "application/json")
            .body(serde_json::to_vec(body).map_err(io_error)?);
    }
    let response = builder
        .header("Accept", "application/geo+json, application/json")
        .send()
        .await
        .map_err(io_error)?;
    if response.status() != reqwest::StatusCode::OK {
        return Err("STAC fixture metadata was not HTTP 200".into());
    }
    let bytes = response.bytes().await.map_err(io_error)?;
    if bytes.len() > MAX_DOCUMENT {
        return Err("STAC fixture metadata exceeds the document limit".into());
    }
    Ok(bytes.to_vec())
}

struct Server {
    origin: String,
    requests: Arc<Mutex<Vec<String>>>,
    metadata: Arc<Mutex<Value>>,
    token: CancellationToken,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.token.cancel();
        self.task.abort();
    }
}

fn published_item(integrity: bool) -> Value {
    let mut value = json!({
        "stac_version":"1.0.0", "type":"Feature", "id":"public-transfer-fixture",
        "bbox":[13.,52.,14.,53.],
        "geometry":{"type":"Polygon","coordinates":[[[13.,52.],[14.,52.],[14.,53.],[13.,53.],[13.,52.]]]},
        "properties":{"datetime":"2024-01-01T00:00:00Z"},
        "links":[{"rel":"self","href":PUBLIC_ITEM,"type":"application/geo+json"}],
        "assets":{
            "red":{"href":PUBLIC_ASSET,"type":"image/tiff; application=geotiff","roles":["data"],"raster:bands":[{"scale":0.1,"unit":"source-declared-unit"}]},
            "blue":{"href":"https://example.com/blue.tif","type":"image/tiff; application=geotiff","roles":["data"]}
        }
    });
    if integrity {
        value["assets"]["red"]["file:size"] = json!(BODY.len());
        value["assets"]["red"]["file:checksum"] = json!(format!("1220{}", hash(BODY)));
    }
    value
}

fn changed_body() -> Vec<u8> {
    // TIFF retains the complete original directory/pixels; the representation's
    // extra trailing bytes and ETag differ so it must be a new complete GET.
    let mut bytes = BODY.to_vec();
    bytes.extend_from_slice(b"new-publisher-representation");
    bytes
}

async fn server(behavior: &'static str, integrity: bool) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let metadata = Arc::new(Mutex::new(published_item(integrity)));
    let captured = requests.clone();
    let published = metadata.clone();
    let token = CancellationToken::new();
    let cancellation = token.clone();
    let task = tokio::spawn(async move {
        loop {
            let accepted = tokio::select! { _ = cancellation.cancelled() => break, accepted = listener.accept() => accepted };
            let Ok((mut socket, _)) = accepted else {
                break;
            };
            let captured = captured.clone();
            let published = published.clone();
            let token = cancellation.clone();
            tokio::spawn(async move {
                let mut bytes = vec![0; 8192];
                let mut read = 0;
                while read < bytes.len() && !bytes[..read].windows(4).any(|w| w == b"\r\n\r\n") {
                    let n = socket.read(&mut bytes[read..]).await.unwrap();
                    if n == 0 {
                        return;
                    }
                    read += n;
                }
                let request = String::from_utf8(bytes[..read].to_vec())
                    .unwrap()
                    .to_ascii_lowercase();
                let path = request.split_whitespace().nth(1).unwrap().to_owned();
                let attempt = {
                    let mut list = captured.lock().unwrap();
                    list.push(request.clone());
                    list.iter()
                        .filter(|v| v.starts_with("get /source.tif "))
                        .count()
                };
                if path == "/item.json" {
                    let body = serde_json::to_vec(&*published.lock().unwrap()).unwrap();
                    let headers = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/geo+json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    socket.write_all(headers.as_bytes()).await.unwrap();
                    socket.write_all(&body).await.unwrap();
                    let _ = socket.shutdown().await;
                    return;
                }
                assert_eq!(
                    path, "/source.tif",
                    "unreviewed asset must never be requested"
                );
                let first = attempt == 1;
                let partial = request.contains("\r\nrange:");
                if partial && matches!(behavior, "412" | "416") {
                    let status = if behavior == "412" {
                        "412 Precondition Failed"
                    } else {
                        "416 Range Not Satisfiable"
                    };
                    let _=socket.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).await;
                    return;
                }
                let changed = !first
                    && (behavior == "changed"
                        || (!partial
                            && matches!(behavior, "bad-range" | "bad-etag" | "412" | "416")));
                let body = if changed {
                    changed_body()
                } else {
                    BODY.to_vec()
                };
                let offset = if partial && behavior != "changed" {
                    PREFIX
                } else {
                    0
                };
                let status = if offset > 0 {
                    "206 Partial Content"
                } else {
                    "200 OK"
                };
                let mut headers = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: image/tiff\r\nContent-Length: {}\r\nConnection: close\r\n",
                    body.len() - offset
                );
                if behavior != "missing" {
                    let etag = if behavior == "weak" {
                        "W/\"v1\""
                    } else if changed || (partial && behavior == "bad-etag") {
                        "\"v2\""
                    } else {
                        "\"v1\""
                    };
                    headers.push_str(&format!("ETag: {etag}\r\n"));
                }
                if offset > 0 {
                    let from = if behavior == "bad-range" {
                        offset + 1
                    } else {
                        offset
                    };
                    headers.push_str(&format!(
                        "Content-Range: bytes {from}-{}/{}\r\n",
                        body.len() - 1,
                        body.len()
                    ));
                }
                headers.push_str("\r\n");
                if socket.write_all(headers.as_bytes()).await.is_err() {
                    return;
                }
                if first {
                    let _ = socket.write_all(&body[..PREFIX / 2]).await;
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    let _ = socket.write_all(&body[PREFIX / 2..PREFIX]).await;
                    if behavior == "hold" {
                        tokio::select! {_=token.cancelled()=>{},_=tokio::time::sleep(Duration::from_secs(20))=>{}}
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
        metadata,
        token,
        task,
    }
}

async fn manager(path: &Path, server: &Server) -> JobManager {
    let manager = JobManager::open_inner(path, Some(server.origin.clone()))
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
    let value = server.metadata.lock().unwrap().clone();
    let bytes = serde_json::to_vec(&value).unwrap();
    let root = &manager.inner.root;
    immutable(root, "document", &bytes).unwrap();
    let record = Record {
        version: 1,
        connection_id: "00000000-0000-4000-8000-000000000001".into(),
        service_name: "Public STAC HTTP fixture".into(),
        kind: "item".into(),
        document_url: PUBLIC_ITEM.into(),
        document_request: None,
        document_sha256: hash(&bytes),
        item_index: None,
        retrieved_at: "2026-10-12T00:00:00Z".into(),
        metadata_documents: vec![],
        search: None,
        reviewed_dataset: None,
        asset_transports: vec![],
    };
    let snapshot = save_snapshot(root, record, &value).unwrap();
    let pin = SourcePin {
        snapshot_id: snapshot.id,
        asset_key: "red".into(),
    };
    let asset = resolve(root, &pin).unwrap();
    let mut job = crate::new_download_job(CreateJobRequest {
        item_id: asset.item_id,
        asset_key: "stac_asset".into(),
        href: asset.href,
        media_type: asset.media_type,
        title: Some(asset.title),
    });
    job.source = asset.service_name;
    job.stac_source = Some(pin);
    validate_job(root, &job).unwrap();
    let mut store = manager.inner.store.lock().await;
    store.jobs.insert(job.id.clone(), job.clone());
    manager.persist(&store.jobs).await.unwrap();
    let token = CancellationToken::new();
    store.active.insert(job.id.clone(), token.clone());
    manager.spawn(job.id.clone(), token);
    job
}

async fn settled(manager: &JobManager, id: &str) -> Job {
    tokio::time::timeout(Duration::from_secs(8), manager.wait(id))
        .await
        .unwrap()
        .unwrap()
}

async fn assert_complete(
    manager: &JobManager,
    id: &str,
    body: &[u8],
    mode: transfer::TransferMode,
) -> Job {
    let job = settled(manager, id).await;
    assert_eq!(job.status, JobStatus::Succeeded, "{:?}", job.error);
    assert_eq!(job.sha256.as_deref(), Some(hash(body).as_str()));
    assert_eq!(
        tokio::fs::read(job.output_path.as_ref().unwrap())
            .await
            .unwrap(),
        body
    );
    assert_eq!(job.bytes_downloaded, body.len() as u64);
    assert_eq!(job.transfer.as_ref().unwrap().mode, mode);
    assert!(!manager
        .inner
        .root
        .join("assets")
        .join(format!("{id}.part"))
        .exists());
    assert!(!manager
        .inner
        .root
        .join("assets")
        .join(format!("{id}.part.resume.json"))
        .exists());
    let mut original = tiff::decoder::Decoder::new(std::io::Cursor::new(BODY)).unwrap();
    let mut result = tiff::decoder::Decoder::new(std::io::Cursor::new(body)).unwrap();
    assert_eq!(result.dimensions().unwrap(), original.dimensions().unwrap());
    match (original.read_image().unwrap(), result.read_image().unwrap()) {
        (tiff::decoder::DecodingResult::U8(a), tiff::decoder::DecodingResult::U8(b)) => {
            assert_eq!(a, b)
        }
        _ => panic!("fixture raw UInt8 samples were not preserved"),
    }
    job
}

#[tokio::test]
async fn public_stac_interruption_revalidates_then_resumes_exact_etag_and_original_sha() {
    let server = server("valid", true).await;
    let dir = tempfile::tempdir().unwrap();
    let manager = manager(dir.path(), &server).await;
    let job = create(&manager, &server).await;
    let failed = settled(&manager, &job.id).await;
    assert_eq!(failed.status, JobStatus::Failed);
    assert!(failed.sha256.is_none() && failed.output_path.is_none());
    assert!(transfer::candidate(
        &manager.inner.root,
        &failed,
        crate::MAX_ASSET_BYTES
    ));
    manager.retry(&job.id).await.unwrap();
    let complete = assert_complete(&manager, &job.id, BODY, transfer::TransferMode::Resumed).await;
    assert_eq!(complete.transfer.unwrap().resumed_bytes, PREFIX as u64);
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 4);
    assert!(
        requests[0].starts_with("get /item.json ") && requests[2].starts_with("get /item.json ")
    );
    assert!(!requests[1].contains("\r\nrange:"));
    assert!(requests[3].contains("\r\nrange: bytes=128-\r\n"));
    assert!(requests[3].contains("\r\nif-range: \"v1\"\r\n"));
    for request in [&requests[1], &requests[3]] {
        assert!(request.contains("\r\naccept: image/tiff, application/octet-stream\r\n"));
        assert!(request.contains("\r\naccept-encoding: identity\r\n"));
        assert!(!request.contains("authorization:") && !request.contains("cookie:"));
    }
}

#[tokio::test]
async fn public_stac_shutdown_reopen_rehashes_prefix_and_revalidates_before_range() {
    let server = server("hold", true).await;
    let dir = tempfile::tempdir().unwrap();
    let first = manager(dir.path(), &server).await;
    let job = create(&first, &server).await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while first.get(&job.id).await.unwrap().bytes_downloaded < PREFIX as u64 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    first.shutdown().await.unwrap();
    assert_eq!(
        first.get(&job.id).await.unwrap().status,
        JobStatus::Interrupted
    );
    let part = first
        .inner
        .root
        .join("assets")
        .join(format!("{}.part", job.id));
    let mut file = tokio::fs::OpenOptions::new()
        .append(true)
        .open(&part)
        .await
        .unwrap();
    file.write_all(b"uncheckpointed crash tail").await.unwrap();
    drop(file);
    drop(first);
    let reopened = manager(dir.path(), &server).await;
    assert!(part.exists());
    reopened.retry(&job.id).await.unwrap();
    assert_complete(&reopened, &job.id, BODY, transfer::TransferMode::Resumed).await;
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 4);
    assert!(requests[2].starts_with("get /item.json "));
    assert!(requests[3].contains("\r\nrange: bytes=128-\r\n"));
}

#[tokio::test]
async fn public_stac_changed_etag_invalid_range_and_preconditions_never_append_old_bytes() {
    for behavior in ["changed", "bad-range", "bad-etag", "412", "416"] {
        let server = server(behavior, false).await;
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path(), &server).await;
        let job = create(&manager, &server).await;
        assert_eq!(settled(&manager, &job.id).await.status, JobStatus::Failed);
        manager.retry(&job.id).await.unwrap();
        let job = assert_complete(
            &manager,
            &job.id,
            &changed_body(),
            transfer::TransferMode::Restarted,
        )
        .await;
        assert_eq!(job.transfer.unwrap().resumed_bytes, 0);
        let requests = server.requests.lock().unwrap();
        let assets = requests
            .iter()
            .filter(|v| v.starts_with("get /source.tif "))
            .collect::<Vec<_>>();
        assert_eq!(assets.len(), if behavior == "changed" { 2 } else { 3 });
        assert!(assets[1].contains("\r\nrange: bytes=128-\r\n"));
        if assets.len() == 3 {
            assert!(!assets[2].contains("\r\nrange:"));
        }
        assert_eq!(
            requests
                .iter()
                .filter(|v| v.starts_with("get /item.json "))
                .count(),
            assets.len()
        );
    }
}

#[tokio::test]
async fn public_stac_missing_or_weak_etag_discards_partial_and_starts_fresh() {
    for behavior in ["missing", "weak"] {
        let server = server(behavior, true).await;
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path(), &server).await;
        let job = create(&manager, &server).await;
        assert_eq!(settled(&manager, &job.id).await.status, JobStatus::Failed);
        assert!(!transfer::candidate(
            &manager.inner.root,
            &job,
            crate::MAX_ASSET_BYTES
        ));
        assert!(!manager
            .inner
            .root
            .join("assets")
            .join(format!("{}.part", job.id))
            .exists());
        manager.retry(&job.id).await.unwrap();
        assert_complete(&manager, &job.id, BODY, transfer::TransferMode::Fresh).await;
        assert!(server
            .requests
            .lock()
            .unwrap()
            .iter()
            .all(|v| !v.contains("\r\nrange:")));
    }
}

#[tokio::test]
async fn public_stac_metadata_or_selected_layer_changes_reject_before_a_resumed_asset_get() {
    for change in ["href", "unit", "roles", "observation"] {
        let server = server("valid", true).await;
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path(), &server).await;
        let job = create(&manager, &server).await;
        assert_eq!(settled(&manager, &job.id).await.status, JobStatus::Failed);
        {
            let mut raw = server.metadata.lock().unwrap();
            match change {
                "href" => raw["assets"]["red"]["href"] = json!("https://example.com/other.tif"),
                "unit" => raw["assets"]["red"]["raster:bands"][0]["unit"] = json!("changed-unit"),
                "roles" => raw["assets"]["red"]["roles"] = json!(["visual"]),
                _ => raw["properties"]["datetime"] = json!("2025-01-01T00:00:00Z"),
            }
        }
        manager.retry(&job.id).await.unwrap();
        let failed = settled(&manager, &job.id).await;
        assert_eq!(failed.status, JobStatus::Failed);
        assert!(failed.error.unwrap().contains("changed"));
        assert!(failed.output_path.is_none() && failed.sha256.is_none());
        let requests = server.requests.lock().unwrap();
        assert_eq!(requests.len(), 3);
        assert!(requests[2].starts_with("get /item.json "));
        assert_eq!(
            requests
                .iter()
                .filter(|v| v.starts_with("get /source.tif "))
                .count(),
            1
        );
    }
}

#[tokio::test]
async fn public_stac_saved_selection_change_cannot_reuse_another_assets_checkpoint() {
    let server = server("valid", true).await;
    let dir = tempfile::tempdir().unwrap();
    let manager = manager(dir.path(), &server).await;
    let job = create(&manager, &server).await;
    settled(&manager, &job.id).await;
    let mut changed = manager.get(&job.id).await.unwrap();
    changed.stac_source.as_mut().unwrap().asset_key = "blue".into();
    assert!(!transfer::candidate(
        &manager.inner.root,
        &changed,
        crate::MAX_ASSET_BYTES
    ));
    manager
        .inner
        .store
        .lock()
        .await
        .jobs
        .insert(job.id.clone(), changed);
    assert!(manager
        .retry(&job.id)
        .await
        .unwrap_err()
        .contains("differs"));
    assert_eq!(server.requests.lock().unwrap().len(), 2);
}

#[test]
fn public_stac_policy_keeps_protected_and_generated_adapters_fresh() {
    let mut job = crate::new_download_job(CreateJobRequest {
        item_id: "fixture".into(),
        asset_key: "stac_asset".into(),
        href: PUBLIC_ASSET.into(),
        media_type: "image/tiff".into(),
        title: None,
    });
    job.stac_source = Some(SourcePin {
        snapshot_id: "a".repeat(64),
        asset_key: "red".into(),
    });
    assert!(crate::supports_partial_transfer(&job));
    for host in [
        crate::providers::nasa::HOST,
        crate::providers::copernicus::HOST,
    ] {
        job.href = format!("https://{host}/source.tif");
        assert!(!crate::supports_partial_transfer(&job));
    }
    job.href = PUBLIC_ASSET.into();
    job.wcs_source = Some(crate::wcs::SourcePin {
        plan_id: "a".repeat(64),
    });
    assert!(!crate::supports_partial_transfer(&job));
}
