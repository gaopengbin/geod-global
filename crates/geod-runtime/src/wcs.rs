//! Public WCS coverage subsets. Source declarations and returned TIFF bytes are
//! separate evidence; a subset is not represented as an original survey asset.
use crate::{features, io_error, now, storage, Job, JobManager, ProxySettings, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use url::Url;
use uuid::Uuid;

mod grid;
#[cfg(test)]
mod tests;
mod verify;
mod xml;
const MAX_XML: usize = 8 * 1024 * 1024;
const MAX_RECORD: usize = 8 * 1024 * 1024;
// Verification streams chunks; this cumulative budget bounds work even for a
// highly compressed raster (up to 4 GiB of 64-bit samples, never held at once).
const MAX_SAMPLES: u64 = 536_870_912;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourcePin {
    pub plan_id: String,
}
pub type Selection = SourcePin;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectItem {
    pub plan_id: String,
    pub title: String,
    pub coverage_id: String,
    pub service_name: String,
    pub media_type: String,
    pub href: String,
    pub bounds: [f64; 4],
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectRequest {
    pub name: String,
    pub url: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Coverage {
    pub id: String,
    pub title: String,
    pub subtype: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Connection {
    pub id: String,
    pub name: String,
    pub url: String,
    pub title: String,
    pub version: String,
    pub connected_at: String,
    pub capabilities_sha256: String,
    pub coverages: Vec<Coverage>,
    pub formats: Vec<String>,
    pub profiles: Vec<String>,
    pub access_constraints: String,
    pub fees: String,
    pub attribution: Option<String>,
    pub describe_url: String,
    pub coverage_url: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DescribeRequest {
    pub connection_id: String,
    pub coverage_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NilValue {
    pub value: String,
    pub reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Field {
    pub name: String,
    pub description: String,
    pub unit: Option<String>,
    pub nil_values: Vec<NilValue>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Description {
    pub id: String,
    pub connection_id: String,
    pub coverage_id: String,
    pub title: String,
    pub service_name: String,
    pub crs: String,
    pub declared_crs: String,
    pub axis_labels: [String; 2],
    pub grid_axis_labels: [String; 2],
    pub width: u32,
    pub height: u32,
    pub transform: [f64; 6],
    pub native_bounds: [f64; 4],
    pub bounds: [f64; 4],
    pub fields: Vec<Field>,
    pub metadata_links: Vec<String>,
    pub capabilities_sha256: String,
    pub description_sha256: String,
    pub description_url: String,
    pub retrieved_at: String,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlanRequest {
    pub description_id: String,
    pub bounds: [f64; 4],
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Plan {
    pub id: String,
    pub description: Description,
    pub requested_bounds: [f64; 4],
    pub bounds: [f64; 4],
    pub native_bounds: [f64; 4],
    pub transform: [f64; 6],
    pub width: u32,
    pub height: u32,
    pub request_url: String,
    pub format: String,
    pub selection: String,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DescriptionRecord {
    version: u32,
    connection: Connection,
    coverage_id: String,
    description_sha256: String,
    description_url: String,
    retrieved_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlanRecord {
    version: u32,
    description_id: String,
    bounds: [f64; 4],
}
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Registry {
    connections: BTreeMap<String, Connection>,
}

#[cfg(test)]
pub(crate) fn fixture_plan(root: &Path) -> SourcePin {
    tests::fixture(root)
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}
fn credential_url(url: &Url) -> bool {
    !url.username().is_empty()
        || url.password().is_some()
        || url.query_pairs().any(|(key, _)| {
            let key = key.to_ascii_lowercase();
            matches!(
                key.as_str(),
                "token"
                    | "access_token"
                    | "key"
                    | "apikey"
                    | "api_key"
                    | "sig"
                    | "signature"
                    | "password"
                    | "authorization"
                    | "credential"
                    | "expires"
                    | "se"
                    | "sp"
                    | "sv"
                    | "sr"
            ) || key.starts_with("x-amz-")
                || key.starts_with("x-goog-")
        })
}
fn public_url(raw: &str) -> Result<Url> {
    let url = features::public_url(raw)?;
    if credential_url(&url) {
        return Err("WCS requires stable public URLs without credentials or signatures".into());
    }
    Ok(url)
}
fn service_url(raw: &str) -> Result<Url> {
    let url = public_url(raw)?;
    if url.query().is_some() {
        return Err("Enter the WCS endpoint without query parameters".into());
    }
    Ok(url)
}
fn endpoint(base: &Url, raw: &str) -> Result<Url> {
    let mut joined = base.join(raw).map_err(io_error)?;
    // Some WCS deployments publish their internal HTTP origin. Retain that
    // declaration in the XML, but only use the already selected HTTPS endpoint.
    if joined.scheme() == "http"
        && joined.host_str() == base.host_str()
        && joined.port().is_none()
        && joined.path() == base.path()
    {
        joined
            .set_scheme("https")
            .map_err(|_| "Invalid WCS endpoint scheme")?;
    }
    let mut url = public_url(joined.as_str())?;
    let directory = base
        .path()
        .rsplit_once('/')
        .map(|(v, _)| format!("{v}/"))
        .unwrap_or_else(|| "/".into());
    if url.origin() != base.origin() || !url.path().starts_with(&directory) {
        return Err(
            "WCS operation endpoints must remain on the connected service origin and directory"
                .into(),
        );
    }
    if url.query_pairs().any(|(k, _)| {
        !matches!(
            k.to_ascii_lowercase().as_str(),
            "service" | "version" | "request"
        )
    }) {
        return Err("WCS operation endpoint contains unsupported parameters".into());
    }
    url.set_query(None);
    Ok(url)
}
fn request_url(base: &str, operation: &str, coverage: Option<&str>) -> Result<Url> {
    let mut url = service_url(base)?;
    let mut query = url.query_pairs_mut();
    query
        .append_pair("service", "WCS")
        .append_pair("version", "2.0.1")
        .append_pair("request", operation);
    if let Some(id) = coverage {
        query.append_pair("coverageId", id);
    }
    drop(query);
    Ok(url)
}
fn directory(root: &Path) -> Result<PathBuf> {
    let dir = root.join("wcs");
    if dir.canonicalize().map_err(io_error)? != dir {
        return Err("WCS metadata storage was redirected".into());
    }
    Ok(dir)
}
fn read_file(path: &Path, max: usize) -> Result<Vec<u8>> {
    let file = std::fs::File::open(storage::regular_file(path)?).map_err(io_error)?;
    if file.metadata().map_err(io_error)?.len() > max as u64 {
        return Err("WCS metadata exceeds its size limit".into());
    }
    let mut bytes = Vec::new();
    file.take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() > max {
        return Err("WCS metadata exceeds its size limit".into());
    }
    Ok(bytes)
}
fn immutable(root: &Path, prefix: &str, extension: &str, bytes: &[u8]) -> Result<String> {
    if bytes.len() > MAX_RECORD {
        return Err("WCS metadata exceeds 8 MiB".into());
    }
    let id = hash(bytes);
    let dir = directory(root)?;
    let path = dir.join(format!("{prefix}-{id}.{extension}"));
    if path.try_exists().map_err(io_error)? {
        if read_file(&path, MAX_RECORD)? != bytes {
            return Err("Saved WCS metadata changed".into());
        }
    } else {
        if std::fs::read_dir(&dir).map_err(io_error)?.count() >= 8192 {
            return Err("WCS metadata storage is full".into());
        }
        let mut tmp = tempfile::NamedTempFile::new_in(&dir).map_err(io_error)?;
        tmp.write_all(bytes).map_err(io_error)?;
        tmp.as_file().sync_all().map_err(io_error)?;
        match tmp.persist_noclobber(&path) {
            Ok(_) => {}
            Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {
                if read_file(&path, MAX_RECORD)? != bytes {
                    return Err("Saved WCS metadata changed".into());
                }
            }
            Err(e) => return Err(io_error(e)),
        }
    }
    Ok(id)
}
fn stored(root: &Path, prefix: &str, extension: &str, id: &str) -> Result<Vec<u8>> {
    if !digest(id) {
        return Err("Invalid WCS metadata checksum".into());
    }
    let bytes = read_file(
        &directory(root)?.join(format!("{prefix}-{id}.{extension}")),
        MAX_RECORD,
    )?;
    if hash(&bytes) != id {
        return Err("Saved WCS metadata changed".into());
    }
    Ok(bytes)
}
fn persist(root: &Path, registry: &Registry) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(registry).map_err(io_error)?;
    if bytes.len() > MAX_RECORD {
        return Err("WCS registry exceeds 8 MiB".into());
    }
    let mut tmp = tempfile::NamedTempFile::new_in(root).map_err(io_error)?;
    tmp.write_all(&bytes).map_err(io_error)?;
    tmp.as_file().sync_all().map_err(io_error)?;
    tmp.persist(root.join("wcs-connections.json"))
        .map_err(io_error)?;
    Ok(())
}
async fn fetch(settings: &ProxySettings, url: &Url) -> Result<Vec<u8>> {
    let response = features::client(url, settings)
        .await?
        .get(url.clone())
        .header("Accept", "application/xml, text/xml")
        .header("Accept-Encoding", "identity")
        .send()
        .await
        .map_err(|_| "Cannot reach public WCS metadata")?;
    if !response.status().is_success() || response.status() == reqwest::StatusCode::PARTIAL_CONTENT
    {
        return Err(format!(
            "WCS metadata returned HTTP {}; redirects are not followed",
            response.status()
        ));
    }
    if response
        .content_length()
        .is_some_and(|n| n > MAX_XML as u64)
    {
        return Err("WCS XML exceeds 8 MiB".into());
    }
    let encoding = response_encoding(&response)?;
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "WCS XML transfer was interrupted")?;
        if bytes.len() + chunk.len() > MAX_XML {
            return Err("WCS XML exceeds 8 MiB".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    decode_xml(bytes, &encoding)
}
fn response_encoding(response: &reqwest::Response) -> Result<String> {
    let mut values = response
        .headers()
        .get_all(reqwest::header::CONTENT_ENCODING)
        .iter();
    let encoding = values
        .next()
        .map(|v| v.to_str().map(str::trim))
        .transpose()
        .map_err(|_| "WCS response has an invalid content encoding")?
        .unwrap_or("identity");
    if values.next().is_some() {
        return Err("WCS response uses unsupported stacked content encodings".into());
    }
    Ok(encoding.to_ascii_lowercase())
}
// The archive retains the exact decoded XML entity, not its HTTP gzip envelope.
// Both compressed input and decoded output are independently bounded.
fn decode_xml(bytes: Vec<u8>, encoding: &str) -> Result<Vec<u8>> {
    if bytes.len() > MAX_XML {
        return Err("WCS XML exceeds 8 MiB".into());
    }
    let decoded = match encoding {
        "identity" => bytes,
        "gzip" => {
            let mut decoded = Vec::new();
            flate2::read::MultiGzDecoder::new(bytes.as_slice())
                .take(MAX_XML as u64 + 1)
                .read_to_end(&mut decoded)
                .map_err(|_| "WCS metadata gzip is invalid or incomplete")?;
            if decoded.len() > MAX_XML {
                return Err("Decoded WCS XML exceeds 8 MiB".into());
            }
            decoded
        }
        _ => return Err("WCS metadata uses unsupported content encoding".into()),
    };
    xml::document(&decoded)?;
    Ok(decoded)
}
fn validated_connection(root: &Path, connection: &Connection) -> Result<()> {
    if !Uuid::parse_str(&connection.id).is_ok_and(|id| id.to_string() == connection.id)
        || !text(&connection.name, 512)
        || chrono::DateTime::parse_from_rfc3339(&connection.connected_at).is_err()
    {
        return Err("Stored WCS connection identity is invalid".into());
    }
    let raw = stored(root, "capabilities", "xml", &connection.capabilities_sha256)?;
    let parsed = xml::capabilities(
        &raw,
        &service_url(&connection.url)?,
        &connection.name,
        &connection.id,
        &connection.connected_at,
    )?;
    if &parsed != connection {
        return Err("Stored WCS connection differs from its capabilities".into());
    }
    Ok(())
}
fn description(root: &Path, id: &str) -> Result<(DescriptionRecord, Description)> {
    let raw = stored(root, "description", "json", id)?;
    let record: DescriptionRecord = serde_json::from_slice(&raw).map_err(io_error)?;
    if record.version != 1 || chrono::DateTime::parse_from_rfc3339(&record.retrieved_at).is_err() {
        return Err("Invalid saved WCS description".into());
    }
    validated_connection(root, &record.connection)?;
    let expected = request_url(
        &record.connection.describe_url,
        "DescribeCoverage",
        Some(&record.coverage_id),
    )?;
    if expected.as_str() != record.description_url {
        return Err("WCS description request differs from its source".into());
    }
    let xml = stored(root, "coverage", "xml", &record.description_sha256)?;
    let value = xml::description(&xml, &record, id)?;
    Ok((record, value))
}
pub(crate) fn plan(root: &Path, id: &str) -> Result<Plan> {
    let record: PlanRecord =
        serde_json::from_slice(&stored(root, "plan", "json", id)?).map_err(io_error)?;
    if record.version != 1 {
        return Err("Unsupported WCS plan version".into());
    }
    let (source, description) = description(root, &record.description_id)?;
    grid::plan(id, description, record.bounds, &source.connection)
}
pub fn resolve(root: &Path, pin: &SourcePin) -> Result<ProjectItem> {
    let plan = plan(root, &pin.plan_id)?;
    Ok(ProjectItem {
        plan_id: plan.id,
        title: format!("{} · WCS subset", plan.description.title),
        coverage_id: plan.description.coverage_id,
        service_name: plan.description.service_name,
        media_type: "image/tiff; application=geotiff".into(),
        href: plan.request_url,
        bounds: plan.bounds,
    })
}
pub fn same_selection(root: &Path, a: &SourcePin, b: &SourcePin) -> Result<bool> {
    let a = plan(root, &a.plan_id)?;
    let b = plan(root, &b.plan_id)?;
    if a.description.connection_id != b.description.connection_id
        || a.request_url != b.request_url
        || a.requested_bounds != b.requested_bounds
        || a.native_bounds != b.native_bounds
    {
        return Ok(false);
    }
    for (prefix, left, right) in [
        (
            "capabilities",
            &a.description.capabilities_sha256,
            &b.description.capabilities_sha256,
        ),
        (
            "coverage",
            &a.description.description_sha256,
            &b.description.description_sha256,
        ),
    ] {
        if xml::fingerprint(&stored(root, prefix, "xml", left)?)?
            != xml::fingerprint(&stored(root, prefix, "xml", right)?)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}
pub fn validate_job(root: &Path, job: &Job) -> Result<ProjectItem> {
    let pin = job.wcs_source.as_ref().ok_or("Job has no WCS source pin")?;
    let selected = resolve(root, pin)?;
    if job.kind != "download"
        || job.asset_key != "wcs_coverage"
        || job.item_id != selected.coverage_id
        || job.href != selected.href
        || job.media_type != selected.media_type
        || job.title != selected.title
        || job.source != selected.service_name
        || job.stac_source.is_some()
        || job.parent_id.is_some()
        || job.recipe.is_some()
        || job.crop.is_some()
        || job.mosaic.is_some()
        || job.mosaic_output.is_some()
        || job.safe.is_some()
        || job.safe_output.is_some()
        || job.viirs_prepare.is_some()
        || job.viirs_science.is_some()
        || job.manifest_path.is_some()
    {
        return Err("WCS job differs from its immutable subset plan".into());
    }
    Ok(selected)
}
pub async fn asset_response(
    settings: &ProxySettings,
    job: &Job,
    root: &Path,
) -> Result<reqwest::Response> {
    let selected = validate_job(root, job)?;
    let plan = plan(root, &job.wcs_source.as_ref().unwrap().plan_id)?;
    let (record, _) = description(root, &plan.description.id)?;
    let fresh_cap = fetch(
        settings,
        &request_url(&record.connection.url, "GetCapabilities", None)?,
    )
    .await?;
    let saved_cap = stored(
        root,
        "capabilities",
        "xml",
        &record.connection.capabilities_sha256,
    )?;
    if xml::fingerprint(&fresh_cap)? != xml::fingerprint(&saved_cap)? {
        return Err("WCS capabilities changed; reconnect and prepare a new subset".into());
    }
    let fresh = fetch(settings, &public_url(&record.description_url)?).await?;
    let saved = stored(root, "coverage", "xml", &record.description_sha256)?;
    if xml::fingerprint(&fresh)? != xml::fingerprint(&saved)? {
        return Err("WCS coverage definition changed; describe it and prepare a new subset".into());
    }
    let url = public_url(&selected.href)?;
    let response = features::client_with_timeout(&url, settings, Duration::from_secs(30 * 60))
        .await?
        .get(url)
        .header("Accept", "image/tiff, application/octet-stream")
        .header("Accept-Encoding", "identity")
        .send()
        .await
        .map_err(|_| "Cannot reach public WCS coverage")?;
    if !response.status().is_success() || response.status() == reqwest::StatusCode::PARTIAL_CONTENT
    {
        return Err(format!(
            "WCS coverage returned HTTP {}; redirects are not followed",
            response.status()
        ));
    }
    if response_encoding(&response)? != "identity" {
        return Err("WCS GeoTIFF uses unsupported HTTP content encoding; uncompressed entity bytes are required".into());
    }
    let media = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim();
    if !matches!(
        media,
        "image/tiff" | "image/geotiff" | "application/octet-stream"
    ) {
        return Err("WCS returned an exception or unsupported response instead of GeoTIFF".into());
    }
    Ok(response)
}
pub fn verify_download(
    root: &Path,
    job: &Job,
    part_path: &Path,
    bytes: u64,
    sha: &str,
    cancel: &tokio_util::sync::CancellationToken,
) -> Result<()> {
    validate_job(root, job)?;
    let plan = plan(root, &job.wcs_source.as_ref().unwrap().plan_id)?;
    verify::download(root, job, part_path, bytes, sha, &plan, cancel)
}
pub async fn load(root: &Path) -> Result<Registry> {
    tokio::fs::create_dir_all(root.join("wcs"))
        .await
        .map_err(io_error)?;
    directory(root)?;
    let path = root.join("wcs-connections.json");
    let registry: Registry = if path.try_exists().map_err(io_error)? {
        serde_json::from_slice(&read_file(&path, MAX_RECORD)?).map_err(io_error)?
    } else {
        Registry::default()
    };
    if registry.connections.len() > 24 {
        return Err("WCS connection registry exceeds 24 services".into());
    }
    for (id, connection) in &registry.connections {
        if id != &connection.id {
            return Err("WCS registry identifier mismatch".into());
        }
        validated_connection(root, connection)?;
    }
    Ok(registry)
}
impl JobManager {
    pub async fn list_wcs_connections(&self) -> Vec<Connection> {
        self.inner
            .wcs
            .lock()
            .await
            .connections
            .values()
            .cloned()
            .collect()
    }
    pub async fn connect_wcs(&self, request: ConnectRequest) -> Result<Connection> {
        if !text(&request.name, 512) {
            return Err("Enter a WCS source name up to 512 characters".into());
        }
        let url = service_url(&request.url)?;
        let settings = self.proxy_settings().await;
        let raw = fetch(
            &settings,
            &request_url(url.as_str(), "GetCapabilities", None)?,
        )
        .await?;
        let value = xml::capabilities(
            &raw,
            &url,
            request.name.trim(),
            &Uuid::new_v4().to_string(),
            &now(),
        )?;
        let mut registry = self.inner.wcs.lock().await;
        if registry.connections.len() >= 24 {
            return Err("WCS supports up to 24 saved connections".into());
        }
        if registry.connections.values().any(|c| c.url == value.url) {
            return Err("This WCS endpoint is already connected".into());
        }
        immutable(&self.inner.root, "capabilities", "xml", &raw)?;
        registry.connections.insert(value.id.clone(), value.clone());
        if let Err(error) = persist(&self.inner.root, &registry) {
            registry.connections.remove(&value.id);
            return Err(error);
        }
        Ok(value)
    }
    pub async fn forget_wcs_connection(&self, id: &str) -> Result<()> {
        let mut registry = self.inner.wcs.lock().await;
        let old = registry
            .connections
            .remove(id)
            .ok_or("Unknown WCS connection")?;
        if let Err(error) = persist(&self.inner.root, &registry) {
            registry.connections.insert(id.into(), old);
            return Err(error);
        }
        Ok(())
    }
    pub async fn describe_wcs(&self, request: DescribeRequest) -> Result<Description> {
        let connection = self
            .inner
            .wcs
            .lock()
            .await
            .connections
            .get(&request.connection_id)
            .cloned()
            .ok_or("Unknown WCS connection")?;
        validated_connection(&self.inner.root, &connection)?;
        if !connection
            .coverages
            .iter()
            .any(|c| c.id == request.coverage_id && c.subtype == "RectifiedGridCoverage")
        {
            return Err("Choose an advertised two-dimensional rectified coverage".into());
        }
        let url = request_url(
            &connection.describe_url,
            "DescribeCoverage",
            Some(&request.coverage_id),
        )?;
        let raw = fetch(&self.proxy_settings().await, &url).await?;
        let record = DescriptionRecord {
            version: 1,
            connection: connection.clone(),
            coverage_id: request.coverage_id,
            description_sha256: hash(&raw),
            description_url: url.to_string(),
            retrieved_at: now(),
        };
        let bytes = serde_json::to_vec(&record).map_err(io_error)?;
        let id = hash(&bytes);
        let value = xml::description(&raw, &record, &id)?;
        let registry = self.inner.wcs.lock().await;
        if registry.connections.get(&connection.id) != Some(&connection) {
            return Err("WCS connection changed during coverage discovery".into());
        }
        immutable(&self.inner.root, "coverage", "xml", &raw)?;
        immutable(&self.inner.root, "description", "json", &bytes)?;
        Ok(value)
    }
    pub async fn plan_wcs(&self, request: PlanRequest) -> Result<Plan> {
        let (record, description) = description(&self.inner.root, &request.description_id)?;
        let stored = PlanRecord {
            version: 1,
            description_id: request.description_id,
            bounds: request.bounds,
        };
        let bytes = serde_json::to_vec(&stored).map_err(io_error)?;
        let id = hash(&bytes);
        let value = grid::plan(&id, description, request.bounds, &record.connection)?;
        immutable(&self.inner.root, "plan", "json", &bytes)?;
        Ok(value)
    }
    pub async fn wcs_plan(&self, id: &str) -> Result<Plan> {
        plan(&self.inner.root, id)
    }
    pub async fn wcs_description(&self, id: &str) -> Result<Description> {
        description(&self.inner.root, id).map(|(_, value)| value)
    }
}
