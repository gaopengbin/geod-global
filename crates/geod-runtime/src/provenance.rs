//! Portable, data-only raster lineage. Export never executes a recorded recipe.
use crate::{io_error, Job, JobManager, JobStatus, Result};
use futures_util::{stream, StreamExt};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use url::Url;

const MAX_JSON: usize = 16 * 1024 * 1024;
const MAX_NODES: usize = 256;
const MAX_ITEM: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceFile {
    pub job_id: String,
    pub filename: String,
    pub bytes: u64,
    pub sha256: String,
    pub source_count: usize,
    pub captured_stac_items: usize,
    pub missing_stac_items: usize,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn valid_id(id: &str) -> Result<()> {
    if uuid::Uuid::parse_str(id)
        .ok()
        .map(|v| v.to_string())
        .as_deref()
        != Some(id)
    {
        return Err("Invalid raster provenance job identifier".into());
    }
    Ok(())
}
fn checked_output(root: &Path, job: &Job) -> Result<PathBuf> {
    valid_id(&job.id)?;
    if job.status != JobStatus::Succeeded || crate::extension(&job.media_type)? != "tif" {
        return Err("Only completed managed GeoTIFF files support source-context export".into());
    }
    let expected = root.join("assets").join(format!("{}.tif", job.id));
    crate::storage::exact_file(
        &expected,
        Path::new(
            job.output_path
                .as_deref()
                .ok_or("Raster output is missing")?,
        ),
    )?;
    Ok(expected)
}
fn check_bytes(path: &Path, job: &Job) -> Result<()> {
    let mut f = File::open(path).map_err(io_error)?;
    if f.metadata().map_err(io_error)?.len() != job.bytes_downloaded {
        return Err("Raster output size changed before provenance export".into());
    }
    let mut hash = Sha256::new();
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        let n = f.read(&mut buffer).map_err(io_error)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    if job.sha256.as_deref() != Some(format!("{:x}", hash.finalize()).as_str()) {
        return Err("Raster output checksum changed before provenance export".into());
    }
    Ok(())
}
fn refs(value: &Value, found: &mut BTreeMap<String, String>) -> Result<()> {
    match value {
        Value::Object(m) => {
            for (key, hash_key) in [("jobId", "sha256"), ("sourceJobId", "sourceSha256")] {
                if let Some(id) = m.get(key).and_then(Value::as_str) {
                    valid_id(id)?;
                    let hash = m
                        .get(hash_key)
                        .and_then(Value::as_str)
                        .ok_or("A processing source has no pinned checksum")?;
                    if let Some(previous) = found.insert(id.into(), hash.into()) {
                        if previous != hash {
                            return Err("Conflicting processing source checksums".into());
                        }
                    }
                }
            }
            for v in m.values() {
                refs(v, found)?;
            }
        }
        Value::Array(a) => {
            for v in a {
                refs(v, found)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn parameters(job: &Job) -> Result<Value> {
    Ok(
        json!({"recipe":job.recipe,"crop":job.crop,"mosaic":job.mosaic,"mosaicOutput":job.mosaic_output,
        "rgb":job.rgb_spec,"rgbOutput":job.rgb_output,"safe":job.safe,"safeOutput":job.safe_output,
        "viirsPrepare":job.viirs_prepare}),
    )
}
fn ordered_graph(all: &BTreeMap<String, Job>, id: &str) -> Result<Vec<Job>> {
    fn visit(
        all: &BTreeMap<String, Job>,
        id: &str,
        stack: &mut BTreeSet<String>,
        done: &mut BTreeSet<String>,
        out: &mut Vec<Job>,
    ) -> Result<()> {
        if done.contains(id) {
            return Ok(());
        }
        valid_id(id)?;
        if stack.len() + done.len() >= MAX_NODES || !stack.insert(id.into()) {
            return Err("Raster provenance graph is cyclic or exceeds its limit".into());
        }
        let job = all.get(id).ok_or("A recorded raster source is missing")?;
        if job.status != JobStatus::Succeeded || job.sha256.is_none() {
            return Err("Raster provenance requires completed, checksum-pinned sources".into());
        }
        let mut inputs = BTreeMap::new();
        refs(&parameters(job)?, &mut inputs)?;
        if let Some(parent) = &job.parent_id {
            inputs.entry(parent.clone()).or_insert_with(|| {
                all.get(parent)
                    .and_then(|j| j.sha256.clone())
                    .unwrap_or_default()
            });
        }
        for (source, hash) in inputs {
            if all.get(&source).and_then(|j| j.sha256.as_deref()) != Some(hash.as_str()) {
                return Err(
                    "Raster provenance source checksum differs from its committed operation".into(),
                );
            }
            visit(all, &source, stack, done, out)?;
        }
        stack.remove(id);
        done.insert(id.into());
        out.push(job.clone());
        Ok(())
    }
    let mut out = Vec::new();
    visit(
        all,
        id,
        &mut BTreeSet::new(),
        &mut BTreeSet::new(),
        &mut out,
    )?;
    Ok(out)
}

// Context stays useful for public items while ephemeral credentials and native
// paths never travel with a shared file. Redactions are explicitly disclosed.
fn sanitize(value: &mut Value, redactions: &mut usize) {
    match value {
        Value::Object(m) => {
            let remove = m
                .keys()
                .filter(|k| {
                    matches!(
                        k.to_ascii_lowercase().as_str(),
                        "authorization"
                            | "cookie"
                            | "password"
                            | "token"
                            | "secret"
                            | "client_secret"
                            | "sig"
                            | "access_token"
                            | "refresh_token"
                            | "api_key"
                            | "apikey"
                            | "outputpath"
                            | "manifestpath"
                            | "storageroot"
                    )
                })
                .cloned()
                .collect::<Vec<_>>();
            for k in remove {
                m.remove(&k);
                *redactions += 1;
            }
            for v in m.values_mut() {
                sanitize(v, redactions);
            }
        }
        Value::Array(a) => {
            for v in a {
                sanitize(v, redactions);
            }
        }
        Value::String(s) => {
            if let Ok(mut u) = Url::parse(s) {
                if matches!(u.scheme(), "http" | "https" | "s3") {
                    let secret = u.query_pairs().any(|(k, _)| {
                        matches!(
                            k.to_ascii_lowercase().as_str(),
                            "sig"
                                | "token"
                                | "access_token"
                                | "api_key"
                                | "apikey"
                                | "password"
                                | "credential"
                                | "signature"
                        ) || k.to_ascii_lowercase().starts_with("x-amz-")
                            || k.to_ascii_lowercase().starts_with("x-goog-")
                    });
                    if secret || !u.username().is_empty() || u.password().is_some() {
                        u.set_query(None);
                        let _ = u.set_username("");
                        let _ = u.set_password(None);
                        *s = u.to_string();
                        *redactions += 1;
                    }
                }
            }
        }
        _ => {}
    }
}
fn fixed_item(job: &Job) -> Option<(Url, String)> {
    let u = crate::providers::asset_url(&job.href).ok()?;
    if !crate::providers::matches_item(&u, &job.item_id, &job.asset_key) {
        return None;
    }
    let (api, collection, key) = match u.host_str()? {
        "sentinel-cogs.s3.us-west-2.amazonaws.com" => (
            "https://earth-search.aws.element84.com/v1",
            "sentinel-2-l2a",
            job.asset_key.clone(),
        ),
        crate::providers::PC_HOST => (
            "https://planetarycomputer.microsoft.com/api/stac/v1",
            "sentinel-2-l2a",
            if job.asset_key == "scl" {
                "SCL".into()
            } else {
                job.asset_key.clone()
            },
        ),
        crate::providers::LANDSAT_HOST => (
            "https://planetarycomputer.microsoft.com/api/stac/v1",
            "landsat-c2-l2",
            job.asset_key.clone(),
        ),
        crate::providers::NAIP_HOST => (
            "https://planetarycomputer.microsoft.com/api/stac/v1",
            "naip",
            "image".into(),
        ),
        crate::providers::radar::HOST => (
            "https://planetarycomputer.microsoft.com/api/stac/v1",
            "sentinel-1-rtc",
            job.asset_key.clone(),
        ),
        crate::providers::modis::HOST => {
            let vegetation = crate::providers::vegetation::asset_path(u.path());
            let key = if vegetation {
                crate::providers::vegetation::ASSETS
                    .iter()
                    .find(|(k, _)| *k == job.asset_key)
                    .map(|(_, v)| (*v).to_owned())?
            } else {
                match job.asset_key.as_str() {
                    "red" => "sur_refl_b01",
                    "green" => "sur_refl_b04",
                    "blue" => "sur_refl_b03",
                    "modis_qc" => "sur_refl_qc_500m",
                    "modis_state" => "sur_refl_state_500m",
                    _ => return None,
                }
                .into()
            };
            (
                "https://planetarycomputer.microsoft.com/api/stac/v1",
                if vegetation {
                    "modis-13Q1-061"
                } else {
                    "modis-09A1-061"
                },
                key,
            )
        }
        _ => return None,
    };
    let mut url = Url::parse(api).ok()?;
    url.path_segments_mut()
        .ok()?
        .extend(["collections", collection, "items", &job.item_id]);
    Some((url, key))
}
fn bind_item(job: &Job, key: &str, value: &Value) -> Result<()> {
    if value["type"] != "Feature"
        || value["id"].as_str() != Some(job.item_id.as_str())
        || value["assets"][key]["href"].as_str() != Some(job.href.as_str())
    {
        return Err("Catalog item no longer matches the selected original asset".into());
    }
    Ok(())
}
struct ContextFailure {
    reason: &'static str,
    status: Option<u16>,
    retryable: bool,
}
impl ContextFailure {
    fn permanent(reason: &'static str) -> Self {
        Self {
            reason,
            status: None,
            retryable: false,
        }
    }
    fn network(reason: &'static str) -> Self {
        Self {
            reason,
            status: None,
            retryable: true,
        }
    }
}
async fn read_fixed_context(
    manager: &JobManager,
    job: &Job,
    url: &Url,
    key: &str,
) -> std::result::Result<Value, ContextFailure> {
    let settings = manager.proxy_settings().await;
    #[cfg(test)]
    let request = if let Some(origin) = &manager.inner.fixture_origin {
        let mut local =
            Url::parse(origin).map_err(|_| ContextFailure::permanent("invalid_fixture_origin"))?;
        local.set_path(url.path());
        reqwest::Client::builder()
            .no_proxy()
            .build()
            .map_err(|_| ContextFailure::permanent("catalog_client_unavailable"))?
            .get(local)
    } else {
        crate::features::client_with_timeout(url, &settings, Duration::from_secs(20))
            .await
            .map_err(|_| ContextFailure::network("catalog_connection_failed"))?
            .get(url.clone())
    };
    #[cfg(not(test))]
    let request = crate::features::client_with_timeout(url, &settings, Duration::from_secs(20))
        .await
        .map_err(|_| ContextFailure::network("catalog_connection_failed"))?
        .get(url.clone());
    let response = request
        .header("Accept", "application/geo+json, application/json")
        .send()
        .await
        .map_err(|e| {
            ContextFailure::network(if e.is_timeout() {
                "catalog_timeout"
            } else {
                "catalog_connection_failed"
            })
        })?;
    if !response.status().is_success() {
        let status = response.status().as_u16();
        return Err(ContextFailure {
            reason: "catalog_http_error",
            status: Some(status),
            retryable: matches!(status, 408 | 425 | 500 | 502 | 503 | 504),
        });
    }
    let mut bytes = Vec::new();
    let mut body = response.bytes_stream();
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(|e| {
            ContextFailure::network(if e.is_timeout() {
                "catalog_timeout"
            } else {
                "catalog_incomplete_response"
            })
        })?;
        if bytes.len() + chunk.len() > MAX_ITEM {
            return Err(ContextFailure::permanent("catalog_metadata_limit"));
        }
        bytes.extend_from_slice(&chunk);
    }
    let item: Value = serde_json::from_slice(&bytes)
        .map_err(|_| ContextFailure::permanent("catalog_decode_failed"))?;
    bind_item(job, key, &item)
        .map_err(|_| ContextFailure::permanent("catalog_identity_mismatch"))?;
    Ok(
        json!({"status":"captured","captureStage":"export","retrievedAt":crate::now(),"documentUrl":url.as_str(),"documentSha256":digest(&bytes),"assetKey":key,"item":item}),
    )
}
async fn source_context(manager: JobManager, job: Job, deadline: tokio::time::Instant) -> Value {
    let base = json!({"jobId":job.id,"itemId":job.item_id,"assetKey":job.asset_key,"href":job.href,"sha256":job.sha256,"bytes":job.bytes_downloaded,"attribution":job.source});
    let mut result = base;
    if let Some(pin) = &job.stac_source {
        result["catalog"] = match crate::stac::portable_item_context(&manager.inner.root, pin) {
            Ok(v)
                if v["status"] != "captured"
                    || v["downloadHref"].as_str() == Some(job.href.as_str()) =>
            {
                v
            }
            Ok(_) => json!({"status":"unavailable","reason":"saved_selection_mismatch"}),
            Err(_) => json!({"status":"unavailable","reason":"saved_snapshot_unavailable"}),
        };
        return result;
    }
    let Some((url, key)) = fixed_item(&job) else {
        result["catalog"] =
            json!({"status":"unavailable","reason":"no_reviewed_stac_item_adapter"});
        return result;
    };
    let uri = url.to_string();
    for attempt in 0..3 {
        let failure =
            match tokio::time::timeout_at(deadline, read_fixed_context(&manager, &job, &url, &key))
                .await
            {
                Ok(Ok(mut value)) => {
                    value["retries"] = json!(attempt);
                    result["catalog"] = value;
                    return result;
                }
                Ok(Err(error)) => error,
                Err(_) => ContextFailure::permanent("catalog_timeout"),
            };
        if failure.retryable && attempt < 2 && tokio::time::Instant::now() < deadline {
            let delay = Duration::from_millis(if attempt == 0 { 250 } else { 750 });
            if tokio::time::timeout_at(deadline, tokio::time::sleep(delay))
                .await
                .is_ok()
            {
                continue;
            }
        }
        let mut missing = json!({"status":"unavailable","reason":failure.reason,"stage":"catalog_context_request","retries":attempt,"documentUrl":uri});
        if let Some(status) = failure.status {
            missing["httpStatus"] = json!(status);
        }
        result["catalog"] = missing;
        return result;
    }
    result
}
fn step(job: &Job) -> Result<Value> {
    let operation = match job.kind.as_str() {
        "download" => "download",
        "raster_clip" => "clip",
        "raster_mosaic" => "mosaic_clip",
        "raster_rgb" => "combine_rgb",
        "raster_prepare" => "decode_original",
        _ => return Err("Unsupported raster processing history".into()),
    };
    Ok(
        json!({"jobId":job.id,"operation":operation,"createdAt":job.created_at,"completedAt":job.updated_at,"outputSha256":job.sha256,
        "parameters":if job.kind=="download"{json!({"assetKey":job.asset_key,"href":job.href,"stacSelection":job.stac_source,"coverageSelection":job.wcs_source})}else{parameters(job)?},
        "localResampling":{"applied":false,"method":null,"gridPolicy":"source grid retained; no local reprojection or resampling"}}),
    )
}
fn file_info(id: &str, bytes: &[u8], doc: &Value) -> ProvenanceFile {
    let sources = doc["sources"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let captured = sources
        .iter()
        .filter(|s| s["catalog"]["status"] == "captured")
        .count();
    ProvenanceFile {
        job_id: id.into(),
        filename: format!("{id}.tif.provenance.json"),
        bytes: bytes.len() as u64,
        sha256: digest(bytes),
        source_count: sources.len(),
        captured_stac_items: captured,
        missing_stac_items: sources.len() - captured,
    }
}
fn paths(root: &Path, id: &str) -> Result<(PathBuf, PathBuf)> {
    valid_id(id)?;
    let root = root.canonicalize().map_err(io_error)?;
    let dir = root.join("exports");
    if dir.canonicalize().map_err(io_error)? != dir {
        return Err("Provenance export directory was redirected".into());
    }
    Ok((
        dir.join(format!("{id}.tif.provenance.json")),
        dir.join(format!("{id}.provenance.sha256")),
    ))
}
fn read_saved(root: &Path, job: &Job, graph_hash: &str) -> Result<(ProvenanceFile, Vec<u8>)> {
    let (path, receipt) = paths(root, &job.id)?;
    crate::storage::exact_file(&path, &path)?;
    crate::storage::exact_file(&receipt, &receipt)?;
    let bytes = read_limited(&path, MAX_JSON)?;
    if read_limited(&receipt, 64)? != digest(&bytes).as_bytes() {
        return Err("Prepared provenance checksum changed".into());
    }
    let doc: Value = serde_json::from_slice(&bytes).map_err(io_error)?;
    if doc["graphSha256"] != graph_hash
        || doc["output"]["sha256"].as_str() != job.sha256.as_deref()
        || doc["output"]["bytes"].as_u64() != Some(job.bytes_downloaded)
    {
        return Err("Prepared provenance no longer matches the completed raster".into());
    }
    Ok((file_info(&job.id, &bytes, &doc), bytes))
}
fn read_limited(path: &Path, max: usize) -> Result<Vec<u8>> {
    let file = File::open(path).map_err(io_error)?;
    if file.metadata().map_err(io_error)?.len() > max as u64 {
        return Err("Provenance metadata exceeds its limit".into());
    }
    let mut bytes = Vec::new();
    file.take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() > max {
        return Err("Provenance metadata exceeds its limit".into());
    }
    Ok(bytes)
}
fn graph_hash(graph: &[Job]) -> Result<String> {
    let values=graph.iter().map(|j|Ok(json!({"id":j.id,"kind":j.kind,"sha256":j.sha256,"bytes":j.bytes_downloaded,"itemId":j.item_id,"assetKey":j.asset_key,"href":j.href,"stacSelection":j.stac_source,"coverageSelection":j.wcs_source,"parameters":parameters(j)?}))).collect::<Result<Vec<_>>>()?;
    Ok(digest(&serde_json::to_vec(&values).map_err(io_error)?))
}
impl JobManager {
    async fn provenance_graph(&self, id: &str) -> Result<Vec<Job>> {
        valid_id(id)?;
        let store = self.inner.store.lock().await;
        ordered_graph(&store.jobs, id)
    }
    pub async fn prepare_raster_provenance(&self, id: &str) -> Result<ProvenanceFile> {
        let _guard = self.inner.provenance_commits.lock().await;
        let graph = self.provenance_graph(id).await?;
        let job = graph.last().ok_or("Raster is missing")?.clone();
        let output = checked_output(&self.inner.root, &job)?;
        let root = self.inner.root.clone();
        let check = job.clone();
        let permit = self
            .inner
            .raster_permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| "The raster worker is busy; retry provenance export shortly")?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            check_bytes(&output, &check)
        })
        .await
        .map_err(io_error)??;
        let hash = graph_hash(&graph)?;
        std::fs::create_dir_all(root.join("exports")).map_err(io_error)?;
        let (target, receipt) = paths(&root, id)?;
        let previous = if target.exists() || receipt.exists() {
            let (info, bytes) = read_saved(&root, &job, &hash)?;
            if info.missing_stac_items == 0 {
                return Ok(info);
            }
            Some(serde_json::from_slice::<Value>(&bytes).map_err(io_error)?)
        } else {
            None
        };
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        let originals = graph
            .iter()
            .filter(|j| j.kind == "download")
            .cloned()
            .collect::<Vec<_>>();
        let cached = previous
            .as_ref()
            .and_then(|v| v["sources"].as_array())
            .cloned()
            .unwrap_or_default();
        let futures = originals
            .into_iter()
            .map(|j| {
                let manager = self.clone();
                let prior = cached
                    .iter()
                    .find(|v| v["jobId"] == j.id && v["catalog"]["status"] == "captured")
                    .cloned();
                async move {
                    if let Some(value) = prior {
                        value
                    } else {
                        source_context(manager, j, deadline).await
                    }
                }
            })
            .collect::<Vec<_>>();
        let mut sources = stream::iter(futures)
            .buffer_unordered(4)
            .collect::<Vec<_>>()
            .await;
        sources.sort_by(|a, b| a["jobId"].as_str().cmp(&b["jobId"].as_str()));
        let steps = graph.iter().map(step).collect::<Result<Vec<_>>>()?;
        let mut doc = json!({"schemaVersion":"geod-raster-provenance/v1","createdAt":crate::now(),"generator":{"product":"GeoD Global","version":env!("CARGO_PKG_VERSION")},
            "graphSha256":hash,"output":{"file":format!("{}.tif",job.id),"format":"GeoTIFF","bytes":job.bytes_downloaded,"sha256":job.sha256},"sources":sources,"processingSteps":steps,
            "limits":["Catalog captured at export is not a historical metadata snapshot at acquisition or download time.","Unavailable original catalog context is explicit; source identities, checksums and committed local operations remain recorded.","Display stretches and map previews are not scientific resampling steps."]});
        let mut redactions = 0;
        sanitize(&mut doc, &mut redactions);
        doc["credentialFieldsRedacted"] = json!(
            redactions as u64
                + previous
                    .as_ref()
                    .and_then(|v| v["credentialFieldsRedacted"].as_u64())
                    .unwrap_or(0)
        );
        let bytes = serde_json::to_vec_pretty(&doc).map_err(io_error)?;
        if bytes.len() > MAX_JSON {
            return Err("Provenance JSON exceeds its limit".into());
        }
        let mut file =
            tempfile::NamedTempFile::new_in(target.parent().unwrap()).map_err(io_error)?;
        file.write_all(&bytes).map_err(io_error)?;
        file.as_file().sync_all().map_err(io_error)?;
        let mut pin =
            tempfile::NamedTempFile::new_in(receipt.parent().unwrap()).map_err(io_error)?;
        pin.write_all(digest(&bytes).as_bytes()).map_err(io_error)?;
        pin.as_file().sync_all().map_err(io_error)?;
        if previous.is_some() {
            read_saved(&root, &job, &hash)?;
            file.persist(target).map_err(io_error)?;
            pin.persist(receipt).map_err(io_error)?;
        } else {
            file.persist_noclobber(target).map_err(io_error)?;
            pin.persist_noclobber(receipt).map_err(io_error)?;
        }
        Ok(file_info(id, &bytes, &doc))
    }
    pub async fn raster_provenance_bytes(&self, id: &str) -> Result<(ProvenanceFile, Vec<u8>)> {
        let _guard = self.inner.provenance_commits.lock().await;
        let graph = self.provenance_graph(id).await?;
        let job = graph.last().ok_or("Raster is missing")?;
        checked_output(&self.inner.root, job)?;
        read_saved(&self.inner.root, job, &graph_hash(&graph)?)
    }
    pub async fn export_raster_with_provenance(&self, id: &str, target: PathBuf) -> Result<()> {
        self.prepare_raster_provenance(id).await?;
        let (_, bytes) = self.raster_provenance_bytes(id).await?;
        let job = self.get(id).await.ok_or("Unknown raster")?;
        let source = checked_output(&self.inner.root, &job)?;
        let permit = self
            .inner
            .raster_permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| "The raster worker is busy; retry export shortly")?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            export_pair(&source, &job, bytes, &target)
        })
        .await
        .map_err(io_error)?
    }
}
fn export_pair(source: &Path, job: &Job, bytes: Vec<u8>, target: &Path) -> Result<()> {
    if !target
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| matches!(s.to_ascii_lowercase().as_str(), "tif" | "tiff"))
    {
        return Err("Choose a .tif or .tiff export filename".into());
    }
    let filename = target
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("Choose a valid raster filename")?;
    let parent = target
        .parent()
        .ok_or("Choose a raster export directory")?
        .canonicalize()
        .map_err(io_error)?;
    let tiff = parent.join(filename);
    let sidecar = parent.join(format!("{filename}.provenance.json"));
    if tiff.try_exists().map_err(io_error)? || sidecar.try_exists().map_err(io_error)? {
        return Err("A raster or provenance sidecar already exists; choose a new filename".into());
    }
    if fs2::available_space(&parent).map_err(io_error)?
        < job.bytes_downloaded.saturating_add(64 * 1024 * 1024)
    {
        return Err("Insufficient disk space for the raster and provenance sidecar".into());
    }
    let mut original = File::open(source).map_err(io_error)?;
    let mut copy = tempfile::NamedTempFile::new_in(&parent).map_err(io_error)?;
    let mut hash = Sha256::new();
    let mut count = 0;
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        let n = original.read(&mut buffer).map_err(io_error)?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count > job.bytes_downloaded {
            return Err("Raster changed during export".into());
        }
        hash.update(&buffer[..n]);
        copy.write_all(&buffer[..n]).map_err(io_error)?;
    }
    if count != job.bytes_downloaded
        || job.sha256.as_deref() != Some(format!("{:x}", hash.finalize()).as_str())
    {
        return Err("Raster changed during export".into());
    }
    copy.as_file().sync_all().map_err(io_error)?;
    let mut doc: Value = serde_json::from_slice(&bytes).map_err(io_error)?;
    doc["output"]["file"] = json!(filename);
    let content = serde_json::to_vec_pretty(&doc).map_err(io_error)?;
    let mut metadata = tempfile::NamedTempFile::new_in(&parent).map_err(io_error)?;
    metadata.write_all(&content).map_err(io_error)?;
    metadata.as_file().sync_all().map_err(io_error)?;
    // Neither existing output may be overwritten. If the second publication
    // loses a race, remove only the just-created, byte-identical JSON member.
    metadata.persist_noclobber(&sidecar).map_err(io_error)?;
    if let Err(error) = copy.persist_noclobber(&tiff) {
        if read_limited(&sidecar, MAX_JSON).ok().as_deref() == Some(content.as_slice()) {
            let _ = std::fs::remove_file(&sidecar);
        }
        return Err(io_error(error));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
