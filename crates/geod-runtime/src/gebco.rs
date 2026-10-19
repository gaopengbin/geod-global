//! Official GEBCO numerical regional subsets. These service-generated exports
//! are kept separate from static publisher originals, STAC scenes and jobs.
use crate::{io_error, now, JobManager, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::{Cursor, Read, Seek, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;

const ORIGIN: &str = "https://download.gebco.net";
const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_METADATA: usize = 64 * 1024;
const GET_ATTEMPTS: usize = 4;
const KIND: &str = "service-generated-regional-numerical-subset";
const GRID_STEP: f64 = 1.0 / 240.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubsetRequest {
    pub name: String,
    pub bounds: [f64; 4],
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PixelRequest {
    pub id: String,
    pub column: u32,
    pub row: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Stage {
    pub stage: String,
    pub at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Subset {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub bounds: [f64; 4],
    pub grid_name: String,
    pub grid_id: u8,
    pub data_source_name: String,
    pub data_source_id: u8,
    pub format_id: u8,
    pub resolution_arc_seconds: u8,
    pub provenance_kind: String,
    pub publisher_static_whole_original: bool,
    pub generation_status: String,
    pub verification_status: String,
    pub basket_id: String,
    pub archive_bytes: u64,
    pub archive_sha256: String,
    pub member_name: String,
    pub bytes: u64,
    pub sha256: String,
    pub request_sha256: String,
    pub grids_sha256: String,
    pub formats_sha256: String,
    pub official_request_sha256: String,
    pub queue_response_sha256: String,
    pub stages: Vec<Stage>,
    pub attribution: String,
    pub limitations: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub fetched_at: String,
    pub grid: Value,
    pub format: Value,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubsetResult {
    pub record: Subset,
    pub raster: crate::stac::GenericRasterInspection,
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
fn validate_request(req: &SubsetRequest) -> Result<()> {
    if crate::projects::validate_project_name(&req.name)? != req.name {
        return Err("Enter a valid GEBCO subset name".into());
    }
    crate::features::bounds(req.bounds)?;
    let [w, s, e, n] = req.bounds;
    let columns = ((e - w) / GRID_STEP).ceil();
    let rows = ((n - s) / GRID_STEP).ceil();
    if columns < 1.
        || rows < 1.
        || columns > 10000.
        || rows > 10000.
        || (columns + 2.) * (rows + 2.) * 2. > (MAX_BYTES - 1024 * 1024) as f64
    {
        return Err("Choose a smaller GEBCO region; this numerical subset exceeds the bounded 32 MiB or 10000-axis budget".into());
    }
    Ok(())
}
fn basket_id(value: &str) -> Result<()> {
    if value.len() < 20
        || value.len() > 100
        || !value.starts_with("GEBCO_")
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err("GEBCO returned an invalid basket identifier".into());
    }
    Ok(())
}
fn record_path(manager: &JobManager, id: &str) -> Result<PathBuf> {
    if Uuid::parse_str(id)
        .map_err(|_| "Invalid GEBCO subset identifier")?
        .to_string()
        != id
    {
        return Err("Invalid GEBCO subset identifier".into());
    }
    Ok(manager.inner.root.join("gebco-subsets").join(id))
}
fn checked_path(path: &Path, directory: bool) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path).map_err(io_error)?;
    if metadata.file_type().is_symlink()
        || (if directory {
            !metadata.is_dir()
        } else {
            !metadata.is_file()
        })
        || std::fs::canonicalize(path).map_err(io_error)? != path
    {
        return Err("GEBCO managed storage path was redirected".into());
    }
    Ok(())
}
fn read_file(dir: &Path, name: &str, max: usize) -> Result<Vec<u8>> {
    checked_path(dir, true)?;
    let path = dir.join(name);
    checked_path(&path, false)?;
    let size = std::fs::metadata(&path).map_err(io_error)?.len();
    if size == 0 || size > max as u64 {
        return Err("Invalid saved GEBCO file size".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(io_error)?
        .take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() as u64 != size {
        return Err("Saved GEBCO file changed during reading".into());
    }
    Ok(bytes)
}
fn selected_catalog(grids: &[u8], formats: &[u8]) -> Result<(Value, Value)> {
    let g: Value =
        serde_json::from_slice(grids).map_err(|_| "Invalid official GEBCO grid metadata")?;
    let f: Value =
        serde_json::from_slice(formats).map_err(|_| "Invalid official GEBCO format metadata")?;
    let grid = g
        .as_array()
        .and_then(|a| {
            a.iter()
                .find(|v| v["id"] == 1 && v["name"] == "gebco_2026_global")
        })
        .ok_or("Official GEBCO 2026 numerical grid is unavailable")?;
    let source = grid["data_sources"]
        .as_array()
        .and_then(|a| a.iter().find(|v| v["id"] == 1 && v["name"] == "gebco_2026"))
        .ok_or("Official GEBCO 2026 bathymetry layer is unavailable")?;
    if grid["projection"] != "EPSG:4326"
        || source["z_name"] != "elevation"
        || source["x_name"] != "lon"
        || source["y_name"] != "lat"
        || source["display_name"] != "Bathymetry"
    {
        return Err("Official GEBCO numerical grid semantics changed".into());
    }
    let fmt = f
        .as_array()
        .and_then(|a| {
            a.iter()
                .find(|v| v["id"] == 2 && v["name"] == "geotiff" && v["category"] == "Data")
        })
        .ok_or("Official GEBCO numerical GeoTIFF format is unavailable")?;
    Ok((grid.clone(), fmt.clone()))
}
struct ReadFailure {
    message: String,
    retryable: bool,
}
impl ReadFailure {
    fn new(message: impl Into<String>, retryable: bool) -> Self {
        Self {
            message: message.into(),
            retryable,
        }
    }
}
async fn read_response(
    mut response: reqwest::Response,
    expected: u16,
    max: usize,
) -> std::result::Result<Vec<u8>, ReadFailure> {
    if response.status().as_u16() != expected {
        let status = response.status().as_u16();
        return Err(ReadFailure::new(
            format!("GEBCO official service returned HTTP {status}; redirects were not followed"),
            status == 408 || status == 429 || (500..600).contains(&status),
        ));
    }
    let size = response.content_length();
    if size.is_some_and(|n| n > max as u64) {
        return Err(ReadFailure::new(
            "GEBCO response exceeds its bounded size budget",
            false,
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ReadFailure::new("GEBCO official response was interrupted", true))?
    {
        if bytes.len().saturating_add(chunk.len()) > max {
            return Err(ReadFailure::new(
                "GEBCO response exceeds its bounded size budget",
                false,
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    if bytes.is_empty() || size.is_some_and(|n| n != bytes.len() as u64) {
        return Err(ReadFailure::new(
            "GEBCO official response is incomplete",
            true,
        ));
    }
    Ok(bytes)
}
async fn response_bytes(response: reqwest::Response, expected: u16, max: usize) -> Result<Vec<u8>> {
    // POST submission is never retried, including an interrupted response: the
    // publisher may already have created a basket even when no ID was read.
    read_response(response, expected, max)
        .await
        .map_err(|e| e.message)
}
#[derive(Clone, Copy)]
enum GetPhase {
    Grid,
    Format,
    Status,
    Download,
}
impl GetPhase {
    fn exhausted(self) -> &'static str {
        match self {
            Self::Grid=>"GEBCO grid metadata read failed after four GET attempts",
            Self::Format=>"GEBCO format metadata read failed after four GET attempts",
            Self::Status=>"GEBCO official queue status failed after four GET attempts; the same basket was used and no queue POST was repeated",
            Self::Download=>"GEBCO official subset download failed after four GET attempts; the same basket was used and no queue POST was repeated",
        }
    }
}
async fn get_bytes(
    client: &reqwest::Client,
    url: &str,
    max: usize,
    phase: GetPhase,
) -> Result<Vec<u8>> {
    // A retry is only another read of this exact metadata URL or server-created
    // basket. Partial bodies are discarded; size limits apply to every attempt.
    for attempt in 0..GET_ATTEMPTS {
        let mut delay = Duration::from_millis(500u64 << attempt);
        let result = match client
            .get(url)
            .header("Accept-Encoding", "identity")
            .send()
            .await
        {
            Ok(response) => {
                if let Some(seconds) = response
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                {
                    delay = Duration::from_secs(seconds.clamp(1, 8));
                }
                read_response(response, 200, max).await
            }
            Err(_) => Err(ReadFailure::new(
                "GEBCO official GET connection failed",
                true,
            )),
        };
        match result {
            Ok(bytes) => return Ok(bytes),
            Err(error) if !error.retryable => return Err(error.message),
            Err(_error) if attempt + 1 == GET_ATTEMPTS => return Err(phase.exhausted().into()),
            Err(_) => tokio::time::sleep(delay).await,
        }
    }
    Err(phase.exhausted().into())
}
async fn catalog_bytes(client: &reqwest::Client, origin: &str) -> Result<(Vec<u8>, Vec<u8>)> {
    let grids = get_bytes(
        client,
        &format!("{origin}/api/grids"),
        MAX_METADATA,
        GetPhase::Grid,
    )
    .await?;
    let formats = get_bytes(
        client,
        &format!("{origin}/api/formats"),
        MAX_METADATA,
        GetPhase::Format,
    )
    .await?;
    selected_catalog(&grids, &formats)?;
    Ok((grids, formats))
}
fn safe_member(name: &str) -> bool {
    let base = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    !name.is_empty()
        && name.len() <= 160
        && name.is_ascii()
        && !name.contains(['/', '\\', ':'])
        && !name.chars().any(char::is_control)
        && !name.ends_with(['.', ' '])
        && !matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        && !["COM", "LPT"].iter().any(|p| {
            base.strip_prefix(p)
                .is_some_and(|s| s.len() == 1 && matches!(s.as_bytes()[0], b'1'..=b'9'))
        })
}
fn member_request(name: &str, bounds: [f64; 4]) -> Result<()> {
    let fields = name
        .strip_prefix("gebco_2026_")
        .and_then(|s| s.strip_suffix("_geotiff.tif"))
        .ok_or("GEBCO subset numerical member identity changed")?
        .split('_')
        .collect::<Vec<_>>();
    if fields.len() != 4 {
        return Err("GEBCO subset member has invalid region metadata".into());
    }
    for (field, prefix, expected) in [
        (fields[0], "n", bounds[3]),
        (fields[1], "s", bounds[1]),
        (fields[2], "w", bounds[0]),
        (fields[3], "e", bounds[2]),
    ] {
        let value = field
            .strip_prefix(prefix)
            .and_then(|v| v.parse::<f64>().ok())
            .ok_or("GEBCO subset member has invalid region metadata")?;
        if !value.is_finite() || (value - expected).abs() > 1e-8 {
            return Err("GEBCO subset member does not match the requested region".into());
        }
    }
    Ok(())
}
fn archive_member<R: Read + Seek>(
    reader: R,
    bounds: [f64; 4],
    archive_bytes: u64,
    capture: bool,
) -> Result<(String, String, Vec<u8>)> {
    let mut archive =
        zip::ZipArchive::new(reader).map_err(|_| "GEBCO download is not a complete ZIP archive")?;
    if archive.len() != 3 {
        return Err("GEBCO subset ZIP must contain one numerical TIFF and the official documentation and terms".into());
    }
    let mut total = 0u64;
    let mut compressed = 0u64;
    let central = archive.central_directory_start();
    let mut spans = Vec::new();
    let mut names = std::collections::BTreeSet::new();
    let mut selected = None;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|_| "GEBCO ZIP member is unreadable or encrypted")?;
        let name = entry.name().to_owned();
        if !safe_member(&name)
            || entry.is_dir()
            || entry.is_symlink()
            || entry.encrypted()
            || !names.insert(name.clone())
            || entry
                .unix_mode()
                .is_some_and(|v| v & 0o170000 != 0 && v & 0o170000 != 0o100000)
            || !matches!(
                entry.compression(),
                zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
            )
        {
            return Err("GEBCO ZIP contains an unsafe, duplicate or unsupported member".into());
        }
        total = total
            .checked_add(entry.size())
            .ok_or("GEBCO ZIP expansion overflow")?;
        if entry.size() == 0 || total > MAX_BYTES as u64 {
            return Err("GEBCO ZIP cumulative expansion exceeds 32 MiB".into());
        }
        compressed = compressed
            .checked_add(entry.compressed_size())
            .ok_or("GEBCO ZIP compressed size overflow")?;
        let start = entry
            .data_start()
            .ok_or("GEBCO ZIP has no exact member byte interval")?;
        let end = start
            .checked_add(entry.compressed_size())
            .ok_or("GEBCO ZIP member byte interval overflow")?;
        if entry.header_start() >= start
            || end > central
            || central > archive_bytes
            || compressed > archive_bytes
        {
            return Err("GEBCO ZIP member byte interval exceeds the complete archive".into());
        }
        spans.push((entry.header_start(), end));
        if name.ends_with("_geotiff.tif") {
            member_request(&name, bounds)?;
            if selected.replace((index, name)).is_some() {
                return Err("GEBCO ZIP contains multiple numerical TIFFs".into());
            }
        } else if name != "GEBCO_Grid_documentation.pdf" && name != "GEBCO_Grid_terms_of_use.pdf" {
            return Err("GEBCO ZIP contains an unexpected product".into());
        }
    }
    if !names.contains("GEBCO_Grid_documentation.pdf")
        || !names.contains("GEBCO_Grid_terms_of_use.pdf")
    {
        return Err("GEBCO official documentation or terms are missing".into());
    }
    spans.sort_unstable();
    if spans.windows(2).any(|w| w[0].1 > w[1].0) {
        return Err("GEBCO ZIP contains overlapping member byte intervals".into());
    }
    let (selected, name) = selected.ok_or("GEBCO ZIP has no numerical GeoTIFF")?;
    let mut sha = Sha256::new();
    let mut raw = Vec::new();
    let mut buffer = [0u8; 65536];
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|_| "GEBCO ZIP member is unreadable or encrypted")?;
        let size = entry.size();
        let mut count = 0u64;
        loop {
            let n = entry
                .read(&mut buffer)
                .map_err(|_| "GEBCO ZIP member CRC or stream verification failed")?;
            if n == 0 {
                break;
            }
            count = count
                .checked_add(n as u64)
                .ok_or("GEBCO ZIP expansion overflow")?;
            if count > size {
                return Err("GEBCO ZIP member expansion size changed".into());
            }
            if index == selected {
                sha.update(&buffer[..n]);
                if capture {
                    raw.extend_from_slice(&buffer[..n]);
                }
            }
        }
        if count != size {
            return Err("GEBCO ZIP member expansion size changed".into());
        }
    }
    Ok((name, format!("{:x}", sha.finalize()), raw))
}
fn unzip(bytes: &[u8], bounds: [f64; 4]) -> Result<(String, Vec<u8>)> {
    let (name, _, raw) = archive_member(Cursor::new(bytes), bounds, bytes.len() as u64, true)?;
    Ok((name, raw))
}
fn hashed_file(
    dir: &Path,
    name: &str,
    expected_bytes: u64,
    expected_sha: &str,
) -> Result<std::fs::File> {
    let path = dir.join(name);
    checked_path(dir, true)?;
    checked_path(&path, false)?;
    let mut file = std::fs::File::open(path).map_err(io_error)?;
    if file.metadata().map_err(io_error)?.len() != expected_bytes
        || expected_bytes == 0
        || expected_bytes > MAX_BYTES as u64
    {
        return Err("Saved GEBCO ZIP or numerical TIFF size changed".into());
    }
    let mut sha = Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut count = 0u64;
    loop {
        let n = file.read(&mut buffer).map_err(io_error)?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count > expected_bytes {
            return Err("Saved GEBCO file changed during reading".into());
        }
        sha.update(&buffer[..n]);
    }
    if count != expected_bytes || format!("{:x}", sha.finalize()) != expected_sha {
        return Err("Saved GEBCO ZIP or numerical TIFF checksum changed".into());
    }
    file.rewind().map_err(io_error)?;
    Ok(file)
}
fn validate_raster(r: &crate::stac::GenericRasterInspection, bounds: [f64; 4]) -> Result<()> {
    if r.crs.as_deref() != Some("EPSG:4326")
        || r.bands.len() != 1
        || r.bands[0].data_type != "Int16"
        || r.bands[0].nodata != json!(-32767)
        || r.pixel_interpretation.as_deref() != Some("area")
    {
        return Err("GEBCO subset is not the declared signed 15 arc-second elevation grid".into());
    }
    let t = r
        .transform
        .ok_or("GEBCO numerical subset has no affine grid")?;
    if (t[0] - GRID_STEP).abs() > 1e-10
        || (t[4] + GRID_STEP).abs() > 1e-10
        || t[1] != 0.
        || t[3] != 0.
    {
        return Err("GEBCO numerical subset resolution or axis order changed".into());
    }
    let b = r.bounds.ok_or("GEBCO numerical subset has no bounds")?;
    if b.iter()
        .zip(bounds)
        .any(|(actual, requested)| (*actual - requested).abs() > GRID_STEP + 1e-9)
    {
        return Err("GEBCO numerical grid does not match the requested region".into());
    }
    Ok(())
}
fn validate_record(r: &Subset) -> Result<()> {
    validate_request(&SubsetRequest {
        name: r.name.clone(),
        bounds: r.bounds,
    })?;
    basket_id(&r.basket_id)?;
    member_request(&r.member_name, r.bounds)?;
    if r.grid_id != 1
        || r.grid_name != "gebco_2026_global"
        || r.data_source_id != 1
        || r.data_source_name != "gebco_2026"
        || r.format_id != 2
        || r.resolution_arc_seconds != 15
        || r.provenance_kind != KIND
        || r.publisher_static_whole_original
        || r.generation_status != "finished"
        || r.verification_status != "verified"
        || r.archive_bytes == 0
        || r.archive_bytes > MAX_BYTES as u64
        || r.bytes == 0
        || r.bytes > MAX_BYTES as u64
        || [
            &r.sha256,
            &r.archive_sha256,
            &r.request_sha256,
            &r.grids_sha256,
            &r.formats_sha256,
            &r.official_request_sha256,
            &r.queue_response_sha256,
        ]
        .iter()
        .any(|s| !digest(s))
        || chrono::DateTime::parse_from_rfc3339(&r.created_at).is_err()
        || r.stages.is_empty()
        || r.stages.len() > 40
        || r.stages.iter().any(|s| {
            chrono::DateTime::parse_from_rfc3339(&s.at).is_err()
                || !matches!(
                    s.stage.as_str(),
                    "official_metadata_verified"
                        | "official_queue_submitted"
                        | "official_new"
                        | "official_processing"
                        | "official_finished"
                        | "whole_zip_downloaded"
                        | "whole_zip_crc_and_numerical_tiff_verified"
                )
        })
    {
        return Err("Invalid saved GEBCO subset provenance".into());
    }
    Ok(())
}
fn verified(dir: &Path, id: &str) -> Result<Subset> {
    checked_path(dir.parent().ok_or("Invalid GEBCO managed path")?, true)?;
    let r: Subset = serde_json::from_slice(&read_file(dir, "record.json", MAX_METADATA)?)
        .map_err(|_| "Invalid saved GEBCO subset record")?;
    validate_record(&r)?;
    if r.id != id {
        return Err("GEBCO subset identifier mismatch".into());
    }
    let req = read_file(dir, "request.json", MAX_METADATA)?;
    let grids = read_file(dir, "grids.json", MAX_METADATA)?;
    let formats = read_file(dir, "formats.json", MAX_METADATA)?;
    if hash(&req) != r.request_sha256
        || hash(&grids) != r.grids_sha256
        || hash(&formats) != r.formats_sha256
    {
        return Err("Saved GEBCO request or official metadata checksum changed".into());
    }
    selected_catalog(&grids, &formats)?;
    let request: SubsetRequest =
        serde_json::from_slice(&req).map_err(|_| "Invalid saved GEBCO request")?;
    if request.name != r.name || request.bounds != r.bounds {
        return Err("Saved GEBCO request identity changed".into());
    }
    let submission = read_file(dir, "submission.json", MAX_METADATA)?;
    let queue = read_file(dir, "queue.json", MAX_METADATA)?;
    if hash(&submission) != r.official_request_sha256 || hash(&queue) != r.queue_response_sha256 {
        return Err("Saved GEBCO official request or queue response checksum changed".into());
    }
    let submitted: Value =
        serde_json::from_slice(&submission).map_err(|_| "Invalid saved GEBCO official request")?;
    let queue: Value = serde_json::from_slice(&queue)
        .map_err(|_| "Invalid saved GEBCO official queue response")?;
    let [w, s, e, n] = r.bounds;
    let item = json!({"id":0,"grid_id":1,"data_source_ids":[1],"formats":[2],"left":w,"right":e,"top":n,"bottom":s});
    if submitted["email"] != Value::Null
        || submitted["items"] != json!([item])
        || submitted["processing_status"] != "new"
        || queue["basketId"] != r.basket_id
    {
        return Err("Saved GEBCO official generation request identity changed".into());
    }
    let archive = hashed_file(dir, "archive.zip", r.archive_bytes, &r.archive_sha256)?;
    let _source = hashed_file(dir, "subset.tif", r.bytes, &r.sha256)?;
    let (name, sha, _) = archive_member(archive, r.bounds, r.archive_bytes, false)?;
    if name != r.member_name || sha != r.sha256 {
        return Err("Saved GEBCO TIFF does not match its complete official ZIP member".into());
    }
    Ok(r)
}

impl JobManager {
    async fn gebco_client(&self) -> Result<reqwest::Client> {
        crate::proxy::download_builder(&self.proxy_settings().await)?
            .timeout(Duration::from_secs(180))
            .build()
            .map_err(|_| "Could not create GEBCO official client".into())
    }
    pub async fn gebco_grids(&self) -> Result<Catalog> {
        let client = self.gebco_client().await?;
        let (g, f) = tokio::time::timeout(Duration::from_secs(180), catalog_bytes(&client, ORIGIN))
            .await
            .map_err(|_| "GEBCO metadata request exceeded three minutes")??;
        let (grid, format) = selected_catalog(&g, &f)?;
        Ok(Catalog {
            fetched_at: now(),
            grid,
            format,
        })
    }
    async fn gebco_verified(&self, id: &str) -> Result<Subset> {
        let dir = record_path(self, id)?;
        let id = id.to_owned();
        tokio::task::spawn_blocking(move || verified(&dir, &id))
            .await
            .map_err(io_error)?
    }
    pub async fn gebco_list(&self) -> Result<Vec<Subset>> {
        let base = self.inner.root.join("gebco-subsets");
        if !base.exists() {
            return Ok(Vec::new());
        }
        checked_path(&base, true)?;
        let mut entries = tokio::fs::read_dir(base).await.map_err(io_error)?;
        let mut out = Vec::new();
        while let Some(e) = entries.next_entry().await.map_err(io_error)? {
            let id = e.file_name().to_string_lossy().to_string();
            if id.starts_with("pending-") {
                continue;
            }
            record_path(self, &id)?;
            out.push(self.gebco_get(&id).await?);
            if out.len() > 1000 {
                return Err("GEBCO subset library exceeds 1000 records".into());
            }
        }
        out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(out)
    }
    pub async fn gebco_get(&self, id: &str) -> Result<Subset> {
        self.gebco_verified(id).await
    }
    async fn gebco_payload(&self, id: &str, archive: bool) -> Result<(Subset, Vec<u8>)> {
        let dir = record_path(self, id)?;
        let id = id.to_owned();
        tokio::task::spawn_blocking(move || {
            let r = verified(&dir, &id)?;
            let bytes = read_file(
                &dir,
                if archive { "archive.zip" } else { "subset.tif" },
                MAX_BYTES,
            )?;
            if hash(&bytes)
                != if archive {
                    r.archive_sha256.clone()
                } else {
                    r.sha256.clone()
                }
            {
                return Err("Saved GEBCO ZIP or numerical TIFF checksum changed".into());
            }
            Ok((r, bytes))
        })
        .await
        .map_err(io_error)?
    }
    pub async fn gebco_source(&self, id: &str) -> Result<Vec<u8>> {
        Ok(self.gebco_payload(id, false).await?.1)
    }
    pub async fn gebco_archive(&self, id: &str) -> Result<Vec<u8>> {
        Ok(self.gebco_payload(id, true).await?.1)
    }
    pub async fn gebco_inspect(&self, id: &str) -> Result<crate::stac::GenericRasterInspection> {
        let (r, source) = self.gebco_payload(id, false).await?;
        let permit = self
            .inner
            .raster_permits
            .clone()
            .acquire_owned()
            .await
            .map_err(io_error)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let mut i = crate::stac::raster::inspect_service_export(&source, &r.id, &r.sha256)?;
            validate_raster(&i, r.bounds)?;
            i.limitations.extend(r.limitations);
            Ok(i)
        })
        .await
        .map_err(io_error)?
    }
    pub async fn gebco_pixel(&self, req: PixelRequest) -> Result<crate::stac::GenericRasterPixel> {
        let (r, source) = self.gebco_payload(&req.id, false).await?;
        let permit = self
            .inner
            .raster_permits
            .clone()
            .acquire_owned()
            .await
            .map_err(io_error)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            crate::stac::raster::sample_service_export(
                &source, &r.id, &r.sha256, req.column, req.row,
            )
        })
        .await
        .map_err(io_error)?
    }
    pub async fn gebco_create_subset(&self, req: SubsetRequest) -> Result<SubsetResult> {
        validate_request(&req)?;
        self.inner.store.lock().await.accepting_jobs()?;
        let _permit = self.inner.permits.acquire().await.map_err(io_error)?;
        let client = self.gebco_client().await?;
        tokio::time::timeout(Duration::from_secs(180),self.gebco_create_with_client(req,client,ORIGIN)).await.map_err(|_|"GEBCO generation, transfer and verification exceeded three minutes; no incomplete subset was saved")?
    }
    async fn gebco_create_with_client(
        &self,
        req: SubsetRequest,
        client: reqwest::Client,
        origin: &str,
    ) -> Result<SubsetResult> {
        validate_request(&req)?;
        let (grids, formats) = catalog_bytes(&client, origin).await?;
        let mut stages = vec![Stage {
            stage: "official_metadata_verified".into(),
            at: now(),
        }];
        let [w, s, e, n] = req.bounds;
        let submission = json!({"id":"0","email":null,"submission_date":now(),"processing_status":"new","items":[{"id":0,"grid_id":1,"data_source_ids":[1],"formats":[2],"left":w,"right":e,"top":n,"bottom":s}]});
        let body = serde_json::to_vec(&submission).map_err(io_error)?;
        let raw = response_bytes(
            client
                .post(format!("{origin}/api/queue"))
                .header("Content-Type", "application/json")
                .body(body.clone())
                .send()
                .await
                .map_err(|_| "GEBCO official subset submission failed; no automatic retry")?,
            201,
            MAX_METADATA,
        )
        .await?;
        let response: Value =
            serde_json::from_slice(&raw).map_err(|_| "Invalid GEBCO official queue response")?;
        let basket = response["basketId"]
            .as_str()
            .ok_or("GEBCO returned no basket identifier")?
            .to_owned();
        basket_id(&basket)?;
        stages.push(Stage {
            stage: "official_queue_submitted".into(),
            at: now(),
        });
        let mut last = String::new();
        loop {
            let bytes = get_bytes(
                &client,
                &format!("{origin}/api/queue/status/{basket}"),
                MAX_METADATA,
                GetPhase::Status,
            )
            .await?;
            let v: Value = serde_json::from_slice(&bytes)
                .map_err(|_| "Invalid GEBCO official queue status")?;
            let status = v["status"]
                .as_str()
                .ok_or("GEBCO official queue returned no process status")?;
            if !matches!(status, "new" | "processing" | "finished") {
                return Err(
                    "GEBCO official generation failed or returned an unknown status".into(),
                );
            }
            if status != last {
                stages.push(Stage {
                    stage: format!("official_{status}"),
                    at: now(),
                });
                last = status.into();
            }
            if stages.len() > 36 {
                return Err("GEBCO official generation status changed unexpectedly".into());
            }
            if status == "finished" {
                break;
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
        let archive = get_bytes(
            &client,
            &format!("{origin}/api/queue/download/{basket}"),
            MAX_BYTES,
            GetPhase::Download,
        )
        .await?;
        stages.push(Stage {
            stage: "whole_zip_downloaded".into(),
            at: now(),
        });
        let id = Uuid::new_v4().to_string();
        let inspect_id = id.clone();
        let bounds = req.bounds;
        let permit = self
            .inner
            .raster_permits
            .clone()
            .acquire_owned()
            .await
            .map_err(io_error)?;
        let (archive, name, source, raster) = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let (name, source) = unzip(&archive, bounds)?;
            let sha = hash(&source);
            let raster = crate::stac::raster::inspect_service_export(&source, &inspect_id, &sha)?;
            validate_raster(&raster, bounds)?;
            Ok::<_, String>((archive, name, source, raster))
        })
        .await
        .map_err(io_error)??;
        stages.push(Stage {
            stage: "whole_zip_crc_and_numerical_tiff_verified".into(),
            at: now(),
        });
        let request_bytes = serde_json::to_vec_pretty(&req).map_err(io_error)?;
        let r=Subset{id:id.clone(),name:req.name,created_at:now(),bounds:req.bounds,grid_name:"gebco_2026_global".into(),grid_id:1,data_source_name:"gebco_2026".into(),data_source_id:1,format_id:2,resolution_arc_seconds:15,
            provenance_kind:KIND.into(),publisher_static_whole_original:false,generation_status:"finished".into(),verification_status:"verified".into(),basket_id:basket,archive_bytes:archive.len() as u64,archive_sha256:hash(&archive),member_name:name,bytes:source.len() as u64,sha256:hash(&source),request_sha256:hash(&request_bytes),grids_sha256:hash(&grids),formats_sha256:hash(&formats),official_request_sha256:hash(&body),queue_response_sha256:hash(&raw),stages,
            attribution:"GEBCO Bathymetric Compilation Group 2026 (2026), GEBCO_2026 Grid, doi:10.5285/4f68d5c7-45eb-f999-e063-7086abc036fa; official GEBCO regional download service. Public domain; retain the official terms and documentation included in the complete ZIP.".into(),
            limitations:vec!["Official GEBCO service-generated regional numerical subset; this is not a static whole publisher original and does not complete the GEBCO original-file backlog item.".into(),"Signed Int16 elevations in meters; negative values indicate bathymetric depths and positive values indicate land elevations. Raw values, the 15 arc-second WGS84 grid, NoData -32767 and the complete official download are retained without reprojection or a TID substitution.".into(),"GEBCO compiles heterogeneous data generally assumed to refer to mean sea level; some shallow-water inputs use another vertical datum. A preview or numerical transfer does not verify local survey accuracy or a uniform vertical datum.".into(),"Regional selection is generated by the official service. Actual pixel-center coordinates and TIFF outer bounds are authoritative; requested coordinates may round to the 15 arc-second grid. First-band grayscale is for browsing.".into()]};
        validate_record(&r)?;
        let base = self.inner.root.join("gebco-subsets");
        tokio::fs::create_dir_all(&base).await.map_err(io_error)?;
        checked_path(&base, true)?;
        let pending = tempfile::Builder::new()
            .prefix("pending-")
            .tempdir_in(&base)
            .map_err(io_error)?;
        for (file, bytes) in [
            ("request.json", request_bytes),
            ("grids.json", grids),
            ("formats.json", formats),
            ("submission.json", body),
            ("queue.json", raw),
            ("archive.zip", archive),
            ("subset.tif", source),
            (
                "record.json",
                serde_json::to_vec_pretty(&r).map_err(io_error)?,
            ),
        ] {
            let path = pending.path().join(file);
            tokio::task::spawn_blocking(move || {
                let mut f = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .map_err(io_error)?;
                f.write_all(&bytes).map_err(io_error)?;
                f.sync_all().map_err(io_error)
            })
            .await
            .map_err(io_error)??;
        }
        tokio::fs::rename(pending.path(), record_path(self, &id)?)
            .await
            .map_err(io_error)?;
        let mut raster = raster;
        raster.limitations.extend(r.limitations.clone());
        Ok(SubsetResult { record: r, raster })
    }
}

#[cfg(test)]
mod tests;
