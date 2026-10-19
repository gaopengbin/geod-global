//! Public STAC and direct raster sources. Original metadata is immutable and
//! content-addressed; asset selection never trusts a caller-supplied download URL.
use crate::{features, io_error, now, storage, Job, JobManager, ProxySettings, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use url::Url;
use uuid::Uuid;

pub(crate) mod archive_original;
mod catalog;
mod planetary_access;
pub mod preview;
pub mod raster;
mod search;
pub use catalog::CatalogNode;
pub use raster::{GenericRasterInspection, GenericRasterPixel};
pub use search::{MetadataRequest, SearchMethod};
const MAX_DOCUMENT: usize = 8 * 1024 * 1024;
const MAX_SEARCH_BYTES: usize = 20 * 1024 * 1024;
const MAX_ITEMS: usize = 1000;
const MAX_PAGES: usize = 20;
const MAX_REGISTRY: usize = 4 * 1024 * 1024;
const MAX_CONNECTIONS: usize = 128;
const MAX_CATALOG_CONNECTIONS: usize = 24;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourcePin {
    pub snapshot_id: String,
    pub asset_key: String,
}
pub type Selection = SourcePin;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectItem {
    pub snapshot_id: String,
    pub asset_key: String,
    pub title: String,
    pub item_id: String,
    pub collection_id: Option<String>,
    pub service_name: String,
    pub media_type: String,
    pub datetime: Option<String>,
    pub start_datetime: Option<String>,
    pub end_datetime: Option<String>,
    pub bbox: Option<[f64; 4]>,
    pub href: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectRequest {
    pub name: String,
    pub url: String,
    pub kind: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Collection {
    pub id: String,
    pub title: String,
    pub description: String,
    pub license: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Capabilities {
    pub search_get: bool,
    pub search_post: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Connection {
    pub id: String,
    pub name: String,
    pub url: String,
    pub kind: String,
    pub connected_at: String,
    pub collections: Vec<Collection>,
    pub capabilities: Capabilities,
    pub snapshot_ids: Vec<String>,
    pub search_url: Option<String>,
    #[serde(default)]
    pub search_method: SearchMethod,
    pub metadata_sha256: Vec<String>,
    pub metadata_documents: Vec<DocumentReceipt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub catalog_nodes: Vec<CatalogNode>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentReceipt {
    pub url: String,
    pub sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchRequest {
    pub connection_id: String,
    pub collection_id: String,
    pub bounds: [f64; 4],
    pub datetime: Option<String>,
    pub limit: Option<usize>,
    pub cursor: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    pub items: Vec<ItemSnapshot>,
    pub next_cursor: Option<String>,
    pub complete: bool,
    pub limit_reached: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scanned_items: Option<usize>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Asset {
    pub key: String,
    pub title: String,
    pub href: String,
    pub media_type: Option<String>,
    pub roles: Vec<String>,
    pub eligible: bool,
    pub reason: Option<String>,
    pub metadata: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_href: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport_adapter: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetTransport {
    pub asset_key: String,
    pub original_href: String,
    pub href: String,
    pub adapter: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ItemSnapshot {
    pub id: String,
    pub connection_id: String,
    pub collection_id: Option<String>,
    pub item_id: String,
    pub title: String,
    pub datetime: Option<String>,
    pub start_datetime: Option<String>,
    pub end_datetime: Option<String>,
    pub bbox: Option<[f64; 4]>,
    pub geometry: Value,
    pub properties: Value,
    pub assets: Vec<Asset>,
    pub retrieved_at: String,
    pub document_sha256: String,
    pub temporal_status: String,
    pub warnings: Vec<String>,
    pub provenance: Provenance,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Provenance {
    pub document_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_request: Option<MetadataRequest>,
    pub metadata_documents: Vec<DocumentReceipt>,
    pub search: Option<SearchRequest>,
    pub collection: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_dataset: Option<crate::datasets::SourceProvenance>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub asset_transports: Vec<AssetTransport>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Record {
    version: u32,
    connection_id: String,
    service_name: String,
    kind: String,
    document_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    document_request: Option<MetadataRequest>,
    document_sha256: String,
    item_index: Option<usize>,
    retrieved_at: String,
    metadata_documents: Vec<DocumentReceipt>,
    search: Option<SearchRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reviewed_dataset: Option<crate::datasets::SourceProvenance>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    asset_transports: Vec<AssetTransport>,
}
#[derive(Debug, Clone)]
struct Cursor {
    request: SearchRequest,
    original: MetadataRequest,
    page: MetadataRequest,
    visited: BTreeSet<String>,
    identities: BTreeSet<String>,
    pages: usize,
    bytes: usize,
}
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Registry {
    connections: BTreeMap<String, Connection>,
    #[serde(skip)]
    cursors: BTreeMap<String, Cursor>,
    #[serde(skip)]
    catalog_cursors: BTreeMap<String, catalog::CatalogCursor>,
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn text(s: &str, max: usize) -> bool {
    !s.trim().is_empty() && s.len() <= max && !s.chars().any(char::is_control)
}
fn uuid(s: &str) -> bool {
    Uuid::parse_str(s).is_ok_and(|v| v.to_string() == s)
}

/// Public metadata and stable assets only. Temporary signatures and credentials
/// must use a reviewed account adapter, never the persisted custom-source path.
fn public_url(raw: &str) -> Result<Url> {
    let url = features::public_url(raw)?;
    if url.query_pairs().any(|(key, _)| {
        let key = key.to_ascii_lowercase();
        matches!(
            key.as_str(),
            "sig"
                | "signature"
                | "token"
                | "access_token"
                | "api_key"
                | "apikey"
                | "key"
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
    }) {
        return Err("Custom sources require stable public URLs without access tokens or temporary signatures".into());
    }
    Ok(url)
}
fn scoped(base: &Url, raw: &str) -> Result<Url> {
    let url = public_url(base.join(raw).map_err(io_error)?.as_str())?;
    if url.origin() != base.origin() {
        return Err("STAC metadata pagination must stay on the connected origin".into());
    }
    Ok(url)
}
fn directory(root: &Path) -> Result<PathBuf> {
    let dir = root.join("stac");
    if dir.canonicalize().map_err(io_error)? != dir {
        return Err("STAC metadata storage was redirected".into());
    }
    Ok(dir)
}
fn read_file(path: &Path, max: usize) -> Result<Vec<u8>> {
    let file = std::fs::File::open(storage::regular_file(path)?).map_err(io_error)?;
    if file.metadata().map_err(io_error)?.len() > max as u64 {
        return Err("STAC metadata exceeds its size limit".into());
    }
    let mut bytes = Vec::new();
    file.take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() > max {
        return Err("STAC metadata exceeds its size limit".into());
    }
    Ok(bytes)
}
fn immutable(root: &Path, prefix: &str, bytes: &[u8]) -> Result<String> {
    let id = hash(bytes);
    let dir = directory(root)?;
    let path = dir.join(format!("{prefix}-{id}.json"));
    if path.try_exists().map_err(io_error)? {
        if read_file(&path, MAX_DOCUMENT)? != bytes {
            return Err("Saved STAC metadata checksum changed".into());
        }
    } else {
        let mut temp = tempfile::NamedTempFile::new_in(&dir).map_err(io_error)?;
        temp.write_all(bytes).map_err(io_error)?;
        temp.as_file().sync_all().map_err(io_error)?;
        match temp.persist_noclobber(&path) {
            Ok(_) => {}
            Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {
                if read_file(&path, MAX_DOCUMENT)? != bytes {
                    return Err("Saved STAC metadata checksum changed".into());
                }
            }
            Err(e) => return Err(io_error(e)),
        }
    }
    Ok(id)
}
fn document(root: &Path, id: &str) -> Result<Vec<u8>> {
    if !digest(id) {
        return Err("Invalid STAC metadata checksum".into());
    }
    let bytes = read_file(
        &directory(root)?.join(format!("document-{id}.json")),
        MAX_DOCUMENT,
    )?;
    if hash(&bytes) != id {
        return Err("Saved STAC document changed".into());
    }
    Ok(bytes)
}
fn persist(root: &Path, registry: &Registry) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(registry).map_err(io_error)?;
    if bytes.len() > MAX_REGISTRY {
        return Err("STAC connection registry is full".into());
    }
    let mut tmp = tempfile::NamedTempFile::new_in(root).map_err(io_error)?;
    tmp.write_all(&bytes).map_err(io_error)?;
    tmp.as_file().sync_all().map_err(io_error)?;
    tmp.persist(root.join("stac-connections.json"))
        .map_err(io_error)?;
    Ok(())
}
fn json_document(raw: &[u8]) -> Result<Value> {
    let value: Value = serde_json::from_slice(raw).map_err(|_| "STAC returned invalid JSON")?;
    if !value.is_object() {
        return Err("STAC response must be a JSON object".into());
    }
    fn credentials(value: &Value) -> Result<()> {
        match value {
            Value::Object(m) => {
                if m.contains_key("href") {
                    for key in ["body", "headers"] {
                        if let Some(v) = m.get(key) {
                            search::check_public_body(v)?;
                        }
                    }
                }
                for (key, v) in m {
                    if matches!(key.as_str(), "href" | "url") {
                        if let Some(s) = v.as_str() {
                            if let Ok(u) = Url::parse(s).or_else(|_| {
                                Url::parse("https://metadata.invalid/").unwrap().join(s)
                            }) {
                                if !u.username().is_empty()
                                    || u.password().is_some()
                                    || u.query_pairs().any(|(k, _)| {
                                        let k = k.to_ascii_lowercase();
                                        matches!(
                                            k.as_str(),
                                            "sig"
                                                | "signature"
                                                | "token"
                                                | "access_token"
                                                | "api_key"
                                                | "apikey"
                                                | "key"
                                                | "password"
                                                | "authorization"
                                                | "credential"
                                                | "expires"
                                                | "se"
                                                | "sp"
                                                | "sv"
                                                | "sr"
                                        ) || k.starts_with("x-amz-")
                                            || k.starts_with("x-goog-")
                                    })
                                {
                                    return Err("STAC metadata contains credentials or temporary signatures and cannot be persisted as a public source".into());
                                }
                            }
                        }
                    }
                    credentials(v)?;
                }
            }
            Value::Array(a) => {
                for v in a {
                    credentials(v)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    credentials(&value)?;
    Ok(value)
}
async fn fetch(settings: &ProxySettings, url: &Url) -> Result<Vec<u8>> {
    search::fetch(settings, &MetadataRequest::get(url)).await
}
async fn metadata_bytes(response: reqwest::Response) -> Result<Vec<u8>> {
    if !response.status().is_success() || response.status() == reqwest::StatusCode::PARTIAL_CONTENT
    {
        return Err(format!(
            "STAC returned HTTP {}; redirects are not followed",
            response.status()
        ));
    }
    let media = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if !matches!(
        media.as_str(),
        "application/json"
            | "application/geo+json"
            | "text/plain"
            | "application/octet-stream"
            | "binary/octet-stream"
            | ""
    ) {
        return Err("STAC metadata must contain JSON or GeoJSON".into());
    }
    if response
        .content_length()
        .is_some_and(|n| n > MAX_DOCUMENT as u64)
    {
        return Err("STAC metadata exceeds 8 MiB".into());
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "STAC metadata transfer was interrupted")?;
        if bytes.len() + chunk.len() > MAX_DOCUMENT {
            return Err("STAC metadata exceeds 8 MiB".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    if bytes.is_empty() {
        return Err("STAC returned empty metadata".into());
    }
    // Object storage sometimes serves JSON with a generic or absent media
    // type. This is not an HTML fallback: the complete bounded document still
    // must be UTF-8 JSON and a structurally identifiable STAC document.
    if matches!(
        media.as_str(),
        "" | "application/octet-stream" | "binary/octet-stream"
    ) {
        validate_generic_metadata(&bytes)?;
    }
    Ok(bytes)
}

fn validate_generic_metadata(bytes: &[u8]) -> Result<()> {
    std::str::from_utf8(bytes).map_err(|_| "STAC metadata must be valid UTF-8 JSON")?;
    let value = json_document(bytes)?;
    match value["type"].as_str() {
        Some("Feature") if value["id"].is_string() && value["properties"].is_object() && value["assets"].is_object() => version(&value)?,
        Some("Catalog" | "Collection") if value["id"].is_string() && value["links"].is_array() => version(&value)?,
        Some("FeatureCollection") if value["features"].is_array() && value["links"].is_array() => {
            for item in value["features"].as_array().unwrap() {
                version(item)?;
                if item["type"] != "Feature" || !item["properties"].is_object() || !item["assets"].is_object() {
                    return Err("Generic metadata body is not a STAC item page".into());
                }
            }
        }
        None if value["collections"].is_array() && value["links"].is_array() => {
            for collection in value["collections"].as_array().unwrap() {
                version(collection)?;
                if collection["type"] != "Collection" || !collection["id"].is_string() {
                    return Err("Generic metadata body is not a STAC collection page".into());
                }
            }
        }
        _ => return Err("Generic metadata body must be a structured STAC JSON document, not HTML or an arbitrary JSON response".into()),
    }
    Ok(())
}
fn links<'a>(value: &'a Value, rel: &str) -> Result<Vec<&'a Value>> {
    let array = value["links"]
        .as_array()
        .filter(|a| a.len() <= 256)
        .ok_or("STAC links are missing or exceed their limit")?;
    Ok(array.iter().filter(|l| l["rel"] == rel).collect())
}
fn link(value: &Value, rel: &str, base: &Url) -> Result<Option<Url>> {
    let found = links(value, rel)?;
    if found.len() > 1 {
        return Err(format!("STAC has ambiguous {rel} links"));
    }
    found
        .first()
        .map(|l| {
            if l.get("method").is_some_and(|m| m != "GET")
                || l.get("body").is_some()
                || l.get("headers").is_some()
            {
                return Err("This STAC link requires unsupported POST or headers".into());
            }
            scoped(base, l["href"].as_str().ok_or("STAC link has no URL")?)
        })
        .transpose()
}
fn version(value: &Value) -> Result<()> {
    if !value["stac_version"]
        .as_str()
        .is_some_and(|v| matches!(v, "1.0.0" | "1.1.0"))
    {
        return Err("Custom STAC requires a STAC 1.0 or 1.1 document".into());
    }
    Ok(())
}
fn collection(value: &Value) -> Result<Collection> {
    version(value)?;
    if value["type"] != "Collection" {
        return Err("STAC collections response contains a non-Collection document".into());
    }
    let id = value["id"]
        .as_str()
        .filter(|s| text(s, 512))
        .ok_or("STAC collection has no valid ID")?;
    let title = value["title"]
        .as_str()
        .filter(|s| text(s, 512))
        .unwrap_or(id);
    let description = value["description"]
        .as_str()
        .filter(|s| s.len() <= 32768)
        .ok_or("STAC collection description is invalid")?;
    let license = value["license"]
        .as_str()
        .filter(|s| text(s, 2048))
        .ok_or("STAC collection license declaration is missing")?;
    Ok(Collection {
        id: id.into(),
        title: title.into(),
        description: description.into(),
        license: license.into(),
    })
}
fn media_supported(media: Option<&str>) -> bool {
    media.is_some_and(|s| {
        matches!(
            s.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
                .as_str(),
            "image/tiff" | "image/geotiff"
        )
    })
}
fn timestamp(value: Option<&Value>) -> Result<Option<String>> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s))
            if s.len() <= 64 && chrono::DateTime::parse_from_rfc3339(s).is_ok() =>
        {
            Ok(Some(s.clone()))
        }
        _ => Err("STAC datetime must be RFC3339 or null".into()),
    }
}

fn raw_original_policy(
    proof: Option<&crate::datasets::SourceProvenance>,
    href: &str,
) -> Result<Option<crate::datasets::clcd::OriginalPolicy>> {
    proof
        .filter(|p| p.dataset.id == "clcd")
        .map(|p| crate::datasets::clcd::original_policy(p, href))
        .transpose()
}

fn direct_original_size(raw: &Value) -> Result<Option<u64>> {
    if raw["probeStatus"] == 206 {
        let range = raw["contentRange"]
            .as_str()
            .ok_or("Direct raster probe lost its Content-Range")?;
        let total = range
            .strip_prefix("bytes 0-15/")
            .and_then(|s| s.parse::<u64>().ok())
            .filter(|n| *n >= 16)
            .ok_or("Direct raster probe has an invalid complete byte length")?;
        if raw["contentLength"].as_u64().is_some_and(|n| n != 16) {
            return Err("Direct raster probe length differs from its selected header range".into());
        }
        Ok(Some(total))
    } else {
        Ok(raw["contentLength"].as_u64())
    }
}

fn validate_raw_original_receipt(
    proof: Option<&crate::datasets::SourceProvenance>,
    href: &str,
    raw: &Value,
) -> Result<Option<crate::datasets::clcd::OriginalPolicy>> {
    let policy = raw_original_policy(proof, href)?;
    if let Some(p) = &policy {
        if raw["kind"] != "direct-raster"
            || raw.get("reviewedArchive").is_some()
            || raw["reviewedOriginal"] != json!({"bytes":p.bytes,"sha256":p.sha256})
            || file_size_declaration(raw)? != Some(p.bytes)
            || checksum_declaration(raw)?.as_deref() != Some(p.sha256.as_str())
            || direct_original_size(raw)? != Some(p.bytes)
        {
            return Err(
                "CLCD raw original receipt differs from its complete reviewed native source".into(),
            );
        }
    } else if raw.get("reviewedOriginal").is_some() {
        return Err("A raw original allowance requires its exact compiled CLCD provenance".into());
    }
    Ok(policy)
}

fn item(record: &Record, id: &str, raw: &Value) -> Result<ItemSnapshot> {
    if let Some(proof) = &record.reviewed_dataset {
        if record.kind != "item" || record.item_index.is_some() || record.search.is_some() {
            return Err("Reviewed regional item provenance cannot be attached to another connector or search page".into());
        }
        let expected = crate::datasets::reviewed_item_transports(proof, &record.document_url, raw)?;
        if expected != record.asset_transports {
            return Err("Reviewed regional asset transport receipt changed".into());
        }
    } else if !record.asset_transports.is_empty() {
        return Err(
            "Asset transport mapping requires a native reviewed regional item proof".into(),
        );
    }
    if record.kind == "raster" {
        let href = public_url(&record.document_url)?.to_string();
        if raw["href"] != href
            || !matches!(
                raw["kind"].as_str(),
                Some("direct-raster" | "reviewed-raster-archive")
            )
        {
            return Err("Direct raster metadata changed".into());
        }
        let (bbox, properties, mut warnings, raw_policy) = if let Some(value) =
            raw.get("datasetPreset")
        {
            let proof: crate::datasets::SourceProvenance =
                serde_json::from_value(value.clone()).map_err(io_error)?;
            crate::datasets::validate_provenance(&proof, &href)?;
            let raw_policy = validate_raw_original_receipt(Some(&proof), &href, raw)?;
            let archive = archive_original::validate_receipt(&proof, &href, raw)?;
            if (raw["kind"] == "reviewed-raster-archive") != archive.is_some() {
                return Err(
                    "Direct original source kind conflicts with its reviewed ZIP/member provenance"
                        .into(),
                );
            }
            let mut warnings = proof.dataset.warnings.clone();
            warnings.push("Dataset preset metadata is a client declaration based on publisher documentation. It does not prove scientific validity, native-file metadata or valid-data coverage. The release/model epoch is not a scene acquisition time.".into());
            (
                proof.source_bounds,
                json!({"datasetPreset":value}),
                warnings,
                raw_policy,
            )
        } else {
            if raw["kind"] != "direct-raster" || raw.get("reviewedArchive").is_some() {
                return Err(
                    "Publisher ZIP access requires a native reviewed numerical member recipe"
                        .into(),
                );
            }
            validate_raw_original_receipt(None, &href, raw)?;
            (None, json!({}), Vec::new(), None)
        };
        let declared_size = file_size_declaration(raw)?;
        let probe_size = direct_original_size(raw)?;
        if declared_size
            .zip(probe_size)
            .is_some_and(|(declared, probed)| declared != probed)
        {
            return Err(
                "Direct raster declared size conflicts with its original header probe".into(),
            );
        }
        checksum_declaration(raw)?;
        let file_limit = raw_policy
            .as_ref()
            .map_or(crate::MAX_ASSET_BYTES, |p| p.bytes);
        let reason = declared_size
            .or(probe_size)
            .filter(|size| *size > file_limit)
            .map(|_| {
                if file_limit == crate::MAX_ASSET_BYTES {
                    "The declared original exceeds the ordinary 512 MiB native download file limit."
                        .to_string()
                } else {
                    "The declared original exceeds its bounded native download file limit."
                        .to_string()
                }
            });
        warnings.push(
            "Direct raster source has no declared acquisition time; COG layout is not validated."
                .into(),
        );
        return Ok(ItemSnapshot {
            id: id.into(),
            connection_id: record.connection_id.clone(),
            collection_id: None,
            item_id: record.document_sha256.clone(),
            title: record.service_name.clone(),
            datetime: None,
            start_datetime: None,
            end_datetime: None,
            bbox,
            geometry: Value::Null,
            properties,
            assets: vec![Asset {
                key: "raster".into(),
                title: record.service_name.clone(),
                href,
                media_type: Some(
                    if raw["kind"] == "reviewed-raster-archive" {
                        "application/zip"
                    } else {
                        "image/tiff; application=geotiff"
                    }
                    .into(),
                ),
                roles: vec!["data".into()],
                eligible: reason.is_none(),
                reason,
                metadata: raw.clone(),
                original_href: None,
                transport_adapter: None,
            }],
            retrieved_at: record.retrieved_at.clone(),
            document_sha256: record.document_sha256.clone(),
            temporal_status: "missing".into(),
            warnings,
            provenance: Provenance {
                document_url: record.document_url.clone(),
                document_request: None,
                metadata_documents: record.metadata_documents.clone(),
                search: None,
                collection: None,
                search_mode: None,
                reviewed_dataset: None,
                asset_transports: Vec::new(),
            },
        });
    }
    let value = if let Some(index) = record.item_index {
        raw["features"]
            .as_array()
            .and_then(|a| a.get(index))
            .ok_or("STAC source page no longer contains the selected item")?
    } else {
        raw
    };
    version(value)?;
    if value["type"] != "Feature" {
        return Err("STAC item must be a Feature".into());
    }
    let item_id = value["id"]
        .as_str()
        .filter(|s| text(s, 512))
        .ok_or("STAC item has no valid ID")?
        .to_string();
    let properties = value["properties"]
        .as_object()
        .ok_or("STAC item has no properties object")?;
    let datetime = timestamp(properties.get("datetime"))?;
    let annual_dtr = record.reviewed_dataset.as_ref().is_some_and(|proof| {
        matches!(
            proof.dataset.id.as_str(),
            "deafrica-fractional-cover" | "deafrica-wofs-annual"
        )
    });
    let start_datetime = timestamp(properties.get("start_datetime").or_else(|| {
        annual_dtr
            .then(|| properties.get("dtr:start_datetime"))
            .flatten()
    }))?;
    let end_datetime = timestamp(properties.get("end_datetime").or_else(|| {
        annual_dtr
            .then(|| properties.get("dtr:end_datetime"))
            .flatten()
    }))?;
    if start_datetime.is_some() != end_datetime.is_some() {
        return Err("STAC item has an incomplete temporal interval".into());
    }
    if let (Some(start), Some(end)) = (&start_datetime, &end_datetime) {
        if chrono::DateTime::parse_from_rfc3339(start).map_err(io_error)?
            > chrono::DateTime::parse_from_rfc3339(end).map_err(io_error)?
        {
            return Err("STAC item temporal interval is reversed".into());
        }
    }
    let bbox = match value.get("bbox") {
        None | Some(Value::Null) => None,
        Some(v) => {
            let a = v
                .as_array()
                .filter(|a| a.len() == 4)
                .ok_or("Only two-dimensional STAC bounding boxes are supported")?;
            let b = [a[0].as_f64(), a[1].as_f64(), a[2].as_f64(), a[3].as_f64()];
            if b.iter().any(Option::is_none) {
                return Err("STAC bounds must be finite numbers".into());
            }
            let b = b.map(Option::unwrap);
            if !b.iter().all(|v| v.is_finite())
                || b[0] < -180.0
                || b[2] > 180.0
                || b[1] < -90.0
                || b[3] > 90.0
                || b[0] > b[2]
                || b[1] > b[3]
            {
                return Err("STAC item bounds are outside WGS84 or cross the antimeridian".into());
            }
            Some(b)
        }
    };
    let geometry = value
        .get("geometry")
        .cloned()
        .ok_or("STAC item has no geometry member")?;
    if !geometry.is_null() && !geometry.is_object() {
        return Err("STAC item geometry must be an object or null".into());
    }
    let collection_id = match value.get("collection") {
        None => None,
        Some(Value::String(s)) if text(s, 512) => Some(s.clone()),
        _ => return Err("STAC item collection is invalid".into()),
    };
    let base = public_url(&record.document_url)?;
    let originals = value["assets"]
        .as_object()
        .filter(|a| !a.is_empty() && a.len() <= 512)
        .ok_or("STAC item assets are empty or exceed 512 entries")?;
    let mut assets = Vec::new();
    for (key, source) in originals {
        if !text(key, 512) || !source.is_object() {
            return Err("STAC asset key or metadata is invalid".into());
        }
        let raw_href = source["href"]
            .as_str()
            .filter(|s| text(s, 4096))
            .ok_or("STAC asset has no valid href")?;
        let resolved = base.join(raw_href).map_err(io_error)?;
        let transport = record
            .asset_transports
            .iter()
            .find(|value| value.asset_key == *key);
        let href = transport.map_or_else(|| resolved.to_string(), |value| value.href.clone());
        let media_type = source
            .get("type")
            .map(|v| {
                v.as_str()
                    .filter(|s| text(s, 256))
                    .map(str::to_owned)
                    .ok_or("STAC asset media type is invalid")
            })
            .transpose()?;
        let roles = match source.get("roles") {
            None => Vec::new(),
            Some(v) => v
                .as_array()
                .filter(|a| a.len() <= 32)
                .ok_or("STAC asset roles are invalid")?
                .iter()
                .map(|v| {
                    v.as_str()
                        .filter(|s| text(s, 128))
                        .map(str::to_owned)
                        .ok_or_else(|| "STAC asset role is invalid".to_string())
                })
                .collect::<Result<Vec<_>>>()?,
        };
        // item() has already validated this exact reviewed proof and original
        // document above. Keep every publisher asset declaration, while the
        // finite Maui client selection authorizes only its selected analytic
        // key. Ordinary manually connected Maxar items retain generic behavior.
        let reviewed_layer_reason = record.reviewed_dataset.as_ref()
            .filter(|proof| proof.dataset.id == "maxar-open-legacy" || (crate::datasets::next_planetary::is_product(&proof.dataset.id) || crate::datasets::additional_pc::is_product(&proof.dataset.id)))
            .filter(|proof| proof.request.layer.as_deref() != Some(key.as_str()))
            .map(|proof| {
                if proof.dataset.id == "maxar-open-legacy" && key == "visual" {
                    "The reviewed Maui recipe does not support the visual asset: its JPEG/YCbCr layout is not decoded by the current raw-sample reader. Original publisher metadata is retained.".to_string()
                } else if proof.dataset.id == "maxar-open-legacy" {
                    format!("This reviewed Maui source authorizes only its selected {} analytic asset. Choose another supported analytic layer through a new reviewed source selection; original publisher metadata is retained.", proof.layer.id)
                } else {
                    format!("This reviewed source authorizes only its selected {} numerical asset. Choose another supported layer through a new source selection; original publisher metadata is retained.", proof.layer.id)
                }
            });
        let reviewed_visual_reason = record
            .reviewed_dataset
            .as_ref()
            .filter(|p| p.dataset.id == "vantor-open")
            .and_then(|p| {
                crate::datasets::vantor_open::visual_asset(p, &record.document_url, key, source)
                    .err()
            });
        let reason = reviewed_visual_reason.or(reviewed_layer_reason)
            .or_else(|| public_url(&href).err())
            .or_else(|| (!media_supported(media_type.as_deref())).then(|| "Original download and local inspection currently support GeoTIFF/COG assets only".into()))
            .or_else(|| checksum_declaration(source).err())
            .or_else(|| match raw_original_policy(record.reviewed_dataset.as_ref(), &href) {
                Err(reason) => Some(reason),
                Ok(policy) => file_size_declaration(source).ok().flatten()
                    .filter(|size| *size > policy.as_ref().map_or(crate::MAX_ASSET_BYTES, |p| p.bytes))
                    .map(|_| if policy.is_none() {
                        "The declared original exceeds the ordinary 512 MiB native download file limit.".into()
                    } else {
                        "The declared original exceeds its bounded native download file limit.".into()
                    }),
            });
        assets.push(Asset {
            key: key.clone(),
            title: source["title"]
                .as_str()
                .filter(|s| text(s, 512))
                .unwrap_or(key)
                .into(),
            href,
            media_type,
            roles,
            eligible: reason.is_none(),
            reason,
            metadata: source.clone(),
            original_href: transport.map(|value| value.original_href.clone()),
            transport_adapter: transport.map(|value| value.adapter.clone()),
        });
    }
    let temporal_status = if datetime.is_some() {
        "instant"
    } else if start_datetime.is_some() {
        "interval"
    } else {
        "missing"
    };
    let mut warnings = if temporal_status == "missing" {
        vec!["Source item has no acquisition time or interval; its temporal fields do not conform to STAC requirements.".into()]
    } else {
        vec![]
    };
    if record
        .reviewed_dataset
        .as_ref()
        .is_some_and(|proof| proof.dataset.id == "worldcereal")
    {
        warnings.extend([
            "The archived publisher item and original S3 asset metadata are unchanged. A separate native receipt maps only the reviewed public WorldCereal assets to the fixed DE Africa regional HTTPS endpoint.".into(),
            "This standalone publisher item has no collection link. Collection metadata/license was not retrieved; documented preset declarations are separate client provenance, not publisher properties.".into(),
            "Seasonal category 254 is valid no-crop data, not NoData. Classification and confidence assets retain their independent original grid declarations.".into(),
        ]);
    } else if let Some(proof) = &record.reviewed_dataset {
        warnings.extend(proof.dataset.warnings.clone());
        warnings.push("Publisher item properties and original asset declarations remain unchanged. The reviewed client product recipe and any exact public transport mapping are separate provenance. Missing collection metadata or license declarations are not fabricated.".into());
    }
    Ok(ItemSnapshot {
        id: id.into(),
        connection_id: record.connection_id.clone(),
        collection_id,
        item_id: item_id.clone(),
        title: properties
            .get("title")
            .and_then(Value::as_str)
            .filter(|s| text(s, 512))
            .unwrap_or(&item_id)
            .into(),
        datetime,
        start_datetime,
        end_datetime,
        bbox,
        geometry,
        properties: Value::Object(properties.clone()),
        assets,
        retrieved_at: record.retrieved_at.clone(),
        document_sha256: record.document_sha256.clone(),
        temporal_status: temporal_status.into(),
        warnings,
        provenance: Provenance {
            document_url: record.document_url.clone(),
            document_request: record.document_request.clone(),
            metadata_documents: record.metadata_documents.clone(),
            search: record.search.clone(),
            collection: None,
            search_mode: (record.kind == "catalog").then(|| "catalog".into()),
            reviewed_dataset: record.reviewed_dataset.clone(),
            asset_transports: record.asset_transports.clone(),
        },
    })
}

fn file_size_declaration(asset: &Value) -> Result<Option<u64>> {
    let mut declared = None;
    for key in ["file:size", "bdc:size"] {
        if let Some(value) = asset.get(key) {
            let size = value
                .as_u64()
                .filter(|size| *size > 0)
                .ok_or("STAC declared file size is invalid")?;
            if declared.is_some_and(|previous| previous != size) {
                return Err("STAC file:size and bdc:size declarations conflict".into());
            }
            declared = Some(size);
        }
    }
    Ok(declared)
}

fn checksum_declaration(asset: &Value) -> Result<Option<String>> {
    file_size_declaration(asset)?;
    let mut declared: Option<String> = None;
    for key in ["file:checksum", "checksum:multihash"] {
        if let Some(checksum) = asset.get(key) {
            let value = checksum
                .as_str()
                .ok_or("STAC checksum declaration must be a string")?;
            let sha = value.strip_prefix("1220").filter(|sha| digest(sha)).ok_or("Only hexadecimal SHA-256 multihash declarations (1220 plus 64 lowercase hex digits) are supported")?;
            if declared.as_deref().is_some_and(|previous| previous != sha) {
                return Err(
                    "STAC file:checksum and checksum:multihash declarations conflict".into(),
                );
            }
            declared = Some(sha.into());
        }
    }
    Ok(declared)
}
fn record(root: &Path, id: &str) -> Result<(Record, ItemSnapshot)> {
    if !digest(id) {
        return Err("Invalid STAC snapshot identifier".into());
    }
    let bytes = read_file(&directory(root)?.join(format!("snapshot-{id}.json")), 65536)?;
    if hash(&bytes) != id {
        return Err("STAC snapshot record changed".into());
    }
    let record: Record = serde_json::from_slice(&bytes).map_err(io_error)?;
    if record.version != 1
        || !uuid(&record.connection_id)
        || !text(&record.service_name, 80)
        || !matches!(record.kind.as_str(), "api" | "item" | "raster" | "catalog")
        || chrono::DateTime::parse_from_rfc3339(&record.retrieved_at).is_err()
    {
        return Err("Invalid STAC snapshot provenance".into());
    }
    public_url(&record.document_url)?;
    if let Some(request) = &record.document_request {
        let request_url = request.validate()?;
        if request.url != record.document_url
            || record.kind != "api"
            || record.search.is_none()
            || record.item_index.is_none()
            || record.metadata_documents.first().is_some_and(|source| {
                public_url(&source.url).is_ok_and(|url| url.origin() != request_url.origin())
            })
        {
            return Err("STAC saved page request conflicts with its provenance".into());
        }
    }
    if record.metadata_documents.len() > 32 {
        return Err("STAC snapshot references too many metadata documents".into());
    }
    for receipt in &record.metadata_documents {
        public_url(&receipt.url)?;
        document(root, &receipt.sha256)?;
    }
    if let Some(search) = &record.search {
        features::bounds(search.bounds)?;
        if search.cursor.is_some() || search.connection_id != record.connection_id {
            return Err("STAC snapshot query identity changed".into());
        }
        if let Some(datetime) = &search.datetime {
            validate_datetime(datetime)?;
        }
    }
    let value = json_document(&document(root, &record.document_sha256)?)?;
    validate_catalog_raster_receipt(root, &record, &value)?;
    validate_reviewed_collection_receipt(root, &record, &value)?;
    let mut snapshot = item(&record, id, &value)?;
    attach_collection(root, &mut snapshot)?;
    if record.kind == "catalog" {
        catalog::validate_record(root, &record, &snapshot)?;
    }
    Ok((record, snapshot))
}
fn save_snapshot(root: &Path, record: Record, raw: &Value) -> Result<ItemSnapshot> {
    validate_catalog_raster_receipt(root, &record, raw)?;
    validate_reviewed_collection_receipt(root, &record, raw)?;
    let bytes = serde_json::to_vec(&record).map_err(io_error)?;
    if bytes.len() > 65536 {
        return Err("STAC snapshot receipt exceeds 64 KiB".into());
    }
    let id = hash(&bytes);
    let mut snapshot = item(&record, &id, raw)?;
    attach_collection(root, &mut snapshot)?;
    if record.kind == "api" {
        validate_search_item(
            &snapshot,
            record
                .search
                .as_ref()
                .ok_or("STAC API item is missing its reviewed search query")?,
        )?;
    }
    if record.kind == "catalog" {
        catalog::validate_record(root, &record, &snapshot)?;
    }
    immutable(root, "snapshot", &bytes)?;
    Ok(snapshot)
}
fn validate_catalog_raster_receipt(root: &Path, record: &Record, raw: &Value) -> Result<()> {
    let Some(value) = raw.get("datasetPreset") else {
        return Ok(());
    };
    let proof: crate::datasets::SourceProvenance =
        serde_json::from_value(value.clone()).map_err(io_error)?;
    let Some(catalog) = proof.catalog_raster.as_ref() else {
        return Ok(());
    };
    if record.kind != "raster"
        || !matches!(proof.dataset.id.as_str(), "usgs-3dep" | "usgs-3dep-1m")
        || record.metadata_documents
            != vec![DocumentReceipt {
                url: catalog.catalog_url.clone(),
                sha256: catalog.catalog_sha256.clone(),
            }]
        || raw["file:size"] != catalog.record["sizeInBytes"]
    {
        return Err(
            "Direct raster catalog receipt conflicts with its archived publisher record".into(),
        );
    }
    let document = json_document(&document(root, &catalog.catalog_sha256)?)?;
    match proof.dataset.id.as_str() {
        "usgs-3dep" => crate::datasets::usgs_3dep::validate_catalog(&proof, &document),
        "usgs-3dep-1m" => crate::datasets::usgs_3dep_1m::validate_catalog(&proof, &document),
        _ => Err("Unsupported direct raster publisher catalog".into()),
    }
}
fn validate_planetary_collection(
    proof: &crate::datasets::SourceProvenance,
    url: &str,
    raw: &Value,
) -> Result<()> {
    if crate::datasets::additional_pc::is_product(&proof.dataset.id) {
        crate::datasets::additional_pc::validate_collection(proof, url, raw)
    } else {
        crate::datasets::next_planetary::validate_collection(proof, url, raw)
    }
}
fn validate_reviewed_collection_receipt(root: &Path, record: &Record, raw: &Value) -> Result<()> {
    if let Some(proof) = record
        .reviewed_dataset
        .as_ref()
        .filter(|p| p.dataset.id == "vantor-open")
    {
        let base = public_url(&record.document_url)?;
        let url = link(raw, "collection", &base)?
            .ok_or("Reviewed Vantor item lost its event collection link")?;
        if record.kind != "item"
            || record.metadata_documents.len() != 2
            || record.metadata_documents[0]
                != (DocumentReceipt {
                    url: record.document_url.clone(),
                    sha256: record.document_sha256.clone(),
                })
            || record.metadata_documents[1].url != url.as_str()
        {
            return Err("Reviewed Vantor item and event collection receipts conflict".into());
        }
        let collection = json_document(&document(root, &record.metadata_documents[1].sha256)?)?;
        return crate::datasets::vantor_open::validate_collection(proof, url.as_str(), &collection);
    }
    if let Some(proof) = record.reviewed_dataset.as_ref().filter(|p| {
        crate::datasets::next_planetary::is_product(&p.dataset.id)
            || crate::datasets::additional_pc::is_product(&p.dataset.id)
    }) {
        let base = public_url(&record.document_url)?;
        let url = link(raw, "collection", &base)?
            .ok_or("Reviewed Planetary item lost its Collection link")?;
        if record.kind != "item"
            || record.metadata_documents.len() != 2
            || record.metadata_documents[0]
                != (DocumentReceipt {
                    url: record.document_url.clone(),
                    sha256: record.document_sha256.clone(),
                })
            || record.metadata_documents[1].url != url.as_str()
        {
            return Err("Reviewed Planetary item and Collection receipts conflict".into());
        }
        let collection = json_document(&document(root, &record.metadata_documents[1].sha256)?)?;
        return validate_planetary_collection(proof, url.as_str(), &collection);
    }
    if let Some(proof) = record
        .reviewed_dataset
        .as_ref()
        .filter(|proof| proof.dataset.id == "maxar-open-legacy")
    {
        let base = public_url(&record.document_url)?;
        let url = link(raw, "collection", &base)?
            .ok_or("Reviewed Maui item lost its acquisition collection link")?;
        let event_url = crate::datasets::maxar::event_url(proof)?;
        if record.kind != "item"
            || record.metadata_documents.len() != 3
            || record.metadata_documents[0]
                != (DocumentReceipt {
                    url: record.document_url.clone(),
                    sha256: record.document_sha256.clone(),
                })
            || record.metadata_documents[1].url != url.as_str()
            || record.metadata_documents[2].url != event_url
        {
            return Err("Reviewed Maui item, acquisition and event receipts conflict".into());
        }
        let collection = json_document(&document(root, &record.metadata_documents[1].sha256)?)?;
        crate::datasets::maxar::validate_collection(proof, url.as_str(), &collection)?;
        let event = json_document(&document(root, &record.metadata_documents[2].sha256)?)?;
        return crate::datasets::maxar::validate_event(proof, &event_url, &event);
    }
    let Some(proof) = record
        .reviewed_dataset
        .as_ref()
        .filter(|proof| crate::datasets::is_reviewed_jaxa(&proof.dataset.id))
    else {
        return Ok(());
    };
    let base = public_url(&record.document_url)?;
    let url = link(raw, "collection", &base)?
        .ok_or("Reviewed JAXA item lost its original collection link")?;
    if record.kind != "item"
        || record.metadata_documents.len() != 2
        || record.metadata_documents[0]
            != (DocumentReceipt {
                url: record.document_url.clone(),
                sha256: record.document_sha256.clone(),
            })
        || record.metadata_documents[1].url != url.as_str()
    {
        return Err("Reviewed JAXA collection receipts conflict with the original item".into());
    }
    let metadata = json_document(&document(root, &record.metadata_documents[1].sha256)?)?;
    crate::datasets::validate_jaxa_collection(proof, url.as_str(), &metadata)
}
fn attach_collection(root: &Path, snapshot: &mut ItemSnapshot) -> Result<()> {
    let Some(id) = snapshot.collection_id.as_deref() else {
        return Ok(());
    };
    for receipt in &snapshot.provenance.metadata_documents {
        let value = json_document(&document(root, &receipt.sha256)?)?;
        let candidates = if value["type"] == "Collection" {
            vec![&value]
        } else {
            value["collections"]
                .as_array()
                .map(|a| a.iter().collect())
                .unwrap_or_default()
        };
        for candidate in candidates {
            if candidate["id"] == id {
                collection(candidate)?;
                if snapshot
                    .provenance
                    .collection
                    .as_ref()
                    .is_some_and(|old| old != candidate)
                {
                    return Err("Pinned STAC collection declarations disagree".into());
                }
                snapshot.provenance.collection = Some(candidate.clone());
            }
        }
    }
    Ok(())
}
pub fn same_selection(root: &Path, a: &SourcePin, b: &SourcePin) -> Result<bool> {
    let (ar, ai) = record(root, &a.snapshot_id)?;
    let (br, bi) = record(root, &b.snapshot_id)?;
    let aa = ai
        .assets
        .iter()
        .find(|v| v.key == a.asset_key)
        .ok_or("Selected STAC asset is missing")?;
    let ba = bi
        .assets
        .iter()
        .find(|v| v.key == b.asset_key)
        .ok_or("Selected STAC asset is missing")?;
    Ok(ar.connection_id == br.connection_id
        && ar.kind == br.kind
        && public_url(&ar.document_url)?.origin() == public_url(&br.document_url)?.origin()
        && ai.collection_id == bi.collection_id
        && ai.item_id == bi.item_id
        && a.asset_key == b.asset_key
        && aa == ba
        && ai.properties == bi.properties
        && ai.geometry == bi.geometry
        && ai.bbox == bi.bbox
        && ai.provenance.collection == bi.provenance.collection)
}
pub fn resolve(root: &Path, pin: &SourcePin) -> Result<ProjectItem> {
    let (record, snapshot) = record(root, &pin.snapshot_id)?;
    let asset = snapshot
        .assets
        .iter()
        .find(|a| a.key == pin.asset_key)
        .ok_or("Selected STAC asset is absent from its saved item")?;
    if !asset.eligible {
        return Err(asset
            .reason
            .clone()
            .unwrap_or("STAC asset is unsupported".into()));
    }
    Ok(ProjectItem {
        snapshot_id: pin.snapshot_id.clone(),
        asset_key: pin.asset_key.clone(),
        title: format!("{} · {}", snapshot.title, asset.title),
        item_id: snapshot.item_id,
        collection_id: snapshot.collection_id,
        service_name: record.service_name,
        media_type: asset
            .media_type
            .clone()
            .ok_or("STAC asset has no media type")?,
        datetime: snapshot.datetime,
        start_datetime: snapshot.start_datetime,
        end_datetime: snapshot.end_datetime,
        bbox: snapshot.bbox,
        href: asset.href.clone(),
    })
}
pub fn validate_job(root: &Path, job: &Job) -> Result<ProjectItem> {
    let pin = job
        .stac_source
        .as_ref()
        .ok_or("Job has no STAC source pin")?;
    let asset = resolve(root, pin)?;
    if job.kind != "download"
        || job.wcs_source.is_some()
        || job.asset_key != "stac_asset"
        || job.href != asset.href
        || job.item_id != asset.item_id
        || job.media_type != asset.media_type
        || job.title != asset.title
        || job.source != asset.service_name
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
        return Err("STAC job differs from its immutable source asset".into());
    }
    Ok(asset)
}

/// Reconstruct the finite raw-original allowance from an immutable native
/// selection. A job URL, metadata digest or caller checksum cannot activate it.
pub(crate) fn reviewed_raw_original(
    root: &Path,
    job: &Job,
) -> Result<Option<crate::datasets::clcd::OriginalPolicy>> {
    let Some(pin) = &job.stac_source else {
        return Ok(None);
    };
    validate_job(root, job)?;
    let (_, snapshot) = record(root, &pin.snapshot_id)?;
    let asset = snapshot
        .assets
        .iter()
        .find(|a| a.key == pin.asset_key)
        .ok_or("Reviewed original source asset is absent")?;
    let proof = asset
        .metadata
        .get("datasetPreset")
        .map(|raw| {
            serde_json::from_value::<crate::datasets::SourceProvenance>(raw.clone())
                .map_err(io_error)
        })
        .transpose()?;
    validate_raw_original_receipt(proof.as_ref(), &job.href, &asset.metadata)
}

pub(crate) struct ReviewedWholeOriginal {
    pub(crate) bytes: u64,
    pub(crate) sha256: String,
}

/// Shared checkpoint identity, independent of whether the publisher supplied
/// a ZIP or a direct TIFF. Archive-member handling retains its separate policy.
pub(crate) fn reviewed_whole_original(
    root: &Path,
    job: &Job,
) -> Result<Option<ReviewedWholeOriginal>> {
    if let Some(p) = reviewed_raw_original(root, job)? {
        return Ok(Some(ReviewedWholeOriginal {
            bytes: p.bytes,
            sha256: p.sha256,
        }));
    }
    Ok(
        archive_original::reviewed_original(root, job)?.map(|p| ReviewedWholeOriginal {
            bytes: p.archive_bytes,
            sha256: p.archive_sha256,
        }),
    )
}

pub fn verify_transfer(root: &Path, job: &Job, bytes: u64, sha256: &str) -> Result<()> {
    validate_job(root, job)?;
    let pin = job.stac_source.as_ref().unwrap();
    let (source_record, snapshot) = record(root, &pin.snapshot_id)?;
    if let Some(proof) = source_record
        .reviewed_dataset
        .as_ref()
        .filter(|p| p.dataset.id == "vantor-open")
    {
        crate::datasets::vantor_open::validate_original(proof, &job.href, bytes, sha256)?;
    }
    if let Some(proof) = source_record
        .reviewed_dataset
        .as_ref()
        .filter(|p| crate::datasets::additional_pc::is_product(&p.dataset.id))
    {
        // Bind whole-original evidence to the selected scientific asset, even
        // when the unchanged publisher Item declares neither size nor checksum.
        crate::datasets::additional_pc::validate_original(proof, &job.href, bytes, sha256)?;
    }
    let asset = snapshot
        .assets
        .iter()
        .find(|a| a.key == pin.asset_key)
        .unwrap();
    let direct_size = if asset.metadata["kind"] == "direct-raster" {
        if asset.metadata["probeStatus"] == 206 {
            asset.metadata["contentRange"]
                .as_str()
                .and_then(|s| s.rsplit_once('/'))
                .and_then(|(_, n)| n.parse::<u64>().ok())
        } else {
            asset.metadata["contentLength"].as_u64()
        }
    } else {
        None
    };
    if file_size_declaration(&asset.metadata)?.is_some_and(|n| n != bytes)
        || checksum_declaration(&asset.metadata)?.is_some_and(|s| s != sha256)
        || direct_size.is_some_and(|n| n != bytes)
    {
        return Err(
            "Downloaded original differs from its STAC declared file size or SHA-256 checksum"
                .into(),
        );
    }
    if let Some(value) = asset.metadata.get("datasetPreset") {
        let proof: crate::datasets::SourceProvenance =
            serde_json::from_value(value.clone()).map_err(io_error)?;
        if proof.dataset.id == "clcd" {
            validate_raw_original_receipt(Some(&proof), &job.href, &asset.metadata)?;
            crate::datasets::clcd::validate_original(&proof, &job.href, bytes, sha256)?;
        }
        if proof.dataset.id == "worldclim" {
            crate::datasets::worldclim::validate_original(&proof, &job.href, bytes, sha256)?;
        }
        if proof.dataset.id == "merit-hydro" {
            crate::datasets::merit_ihu::validate_original(&proof, &job.href, bytes, sha256)?;
        }
        if proof.dataset.id == "soilgrids" {
            crate::datasets::soilgrids::validate_original(&proof, &job.href, bytes, sha256)?;
        }
        if let Some(selection) =
            archive_original::validate_receipt(&proof, &job.href, &asset.metadata)?
        {
            if bytes != selection.archive_bytes || sha256 != selection.archive_sha256 {
                return Err(
                    "Complete publisher ZIP differs from its reviewed original length or SHA-256"
                        .into(),
                );
            }
        }
    }
    Ok(())
}

/// The recorded query identifies the caller's request, not proof that the
/// publisher honored it. Check actual item declarations before archiving a
/// new API selection; never manufacture dates or a footprint from the query.
fn validate_search_item(snapshot: &ItemSnapshot, request: &SearchRequest) -> Result<()> {
    features::bounds(request.bounds)?;
    if snapshot.connection_id != request.connection_id
        || snapshot.collection_id.as_deref() != Some(&request.collection_id)
    {
        return Err("STAC search returned an item from a different source or collection".into());
    }
    let bounds = snapshot.bbox.ok_or("STAC search item has no declared bounding box; its intersection with the requested area cannot be verified")?;
    if bounds[2] < request.bounds[0]
        || bounds[0] > request.bounds[2]
        || bounds[3] < request.bounds[1]
        || bounds[1] > request.bounds[3]
    {
        return Err("STAC search item does not intersect the requested area".into());
    }
    if let Some(datetime) = &request.datetime {
        validate_datetime(datetime)?;
        let parse = |value: &str| chrono::DateTime::parse_from_rfc3339(value).map_err(io_error);
        let parts = datetime.split('/').collect::<Vec<_>>();
        let wanted_start = (parts[0] != "..").then(|| parse(parts[0])).transpose()?;
        let wanted_end = (parts[parts.len() - 1] != "..")
            .then(|| parse(parts[parts.len() - 1]))
            .transpose()?;
        let (actual_start, actual_end) = if let Some(instant) = &snapshot.datetime {
            let instant = parse(instant)?;
            (instant, instant)
        } else if let (Some(start), Some(end)) = (&snapshot.start_datetime, &snapshot.end_datetime)
        {
            (parse(start)?, parse(end)?)
        } else {
            return Err("STAC search item has no declared observation time or interval; its intersection with the requested period cannot be verified".into());
        };
        if wanted_start.is_some_and(|start| actual_end < start)
            || wanted_end.is_some_and(|end| actual_start > end)
        {
            return Err(
                "STAC search item does not intersect the requested observation period".into(),
            );
        }
    }
    Ok(())
}
fn revalidation_request(saved: &Record, original_item: &Value) -> Result<MetadataRequest> {
    if let Some(proof) = &saved.reviewed_dataset {
        let url =
            crate::datasets::reviewed_item_self_url(proof, &saved.document_url, original_item)?;
        return Ok(MetadataRequest::get(&public_url(&url)?));
    }
    let base = public_url(&saved.document_url)?;
    if let Some(url) = link(original_item, "self", &base)? {
        Ok(MetadataRequest::get(&url))
    } else {
        let request = saved
            .document_request
            .clone()
            .unwrap_or_else(|| MetadataRequest::get(&base));
        request.validate()?;
        Ok(request)
    }
}
pub(crate) async fn asset_head(
    settings: &ProxySettings,
    root: &Path,
    selection: &Selection,
) -> Result<reqwest::Response> {
    let asset = resolve(root, selection)?;
    let url = public_url(&asset.href)?;
    let delivery = planetary_access::delivery_url(settings, root, selection)
        .await?
        .unwrap_or_else(|| url.clone());
    features::client_with_timeout(&url, settings, Duration::from_secs(12))
        .await?
        .head(delivery)
        .header("Accept", "image/tiff, application/octet-stream")
        .send()
        .await
        .map_err(|_| "Could not check source file size. Check your proxy and try again.".into())
}

pub(crate) async fn asset_response(
    settings: &ProxySettings,
    job: &Job,
    root: &Path,
    checkpoint: Option<&crate::transfer::Checkpoint>,
) -> Result<reqwest::Response> {
    asset_response_inner(
        settings,
        job,
        root,
        checkpoint,
        &AssetResponseTransport::default(),
    )
    .await
}

/// Independent closed-range transport for the compiled whole-original policy.
/// It does not change legacy GET/If-Range behavior or obtain a replacement URL.
pub(crate) async fn finite_asset_response(
    settings: &ProxySettings,
    job: &Job,
    root: &Path,
    requested: crate::transfer::FiniteRange,
) -> Result<reqwest::Response> {
    let asset = validate_job(root, job)?;
    if asset.href != job.href
        || job
            .agent_approval
            .as_ref()
            .and_then(|a| a.remote.as_ref())
            .is_some()
    {
        return Err(
            "Reviewed finite transfer differs from its original source or approval path".into(),
        );
    }
    let url = public_url(&asset.href)?;
    let raw = reviewed_raw_original(root, job)?;
    if let Some(policy) = &raw {
        if requested.total != policy.bytes
            || requested.start > requested.end
            || requested.end >= policy.bytes
            || requested.length() > crate::transfer::CLCD_RANGE_BYTES as usize
        {
            return Err("CLCD finite range differs from its exact reviewed whole original".into());
        }
    }
    let response = features::client_with_timeout(&url, settings, Duration::from_secs(300))
        .await?
        .get(url)
        .header(reqwest::header::ACCEPT_ENCODING, "identity")
        .header(
            reqwest::header::ACCEPT,
            if raw.is_some() {
                // Zenodo's API content route negotiates the API endpoint
                // before streaming its TIFF and rejects an image-only Accept
                // with 406. The scoped response still must pass exact 206,
                // length/range, original TIFF media/signature and whole SHA.
                "*/*"
            } else {
                "application/zip, application/octet-stream"
            },
        )
        .header(reqwest::header::RANGE, requested.header())
        .send()
        .await
        .map_err(|error| error.without_url().to_string())?;
    Ok(response)
}

// The production transport has no alternative URL or client. A test-only
// loopback transport exercises the same provenance and response state machine
// without relaxing public HTTPS/DNS policy in the shipped application.
#[derive(Default)]
struct AssetResponseTransport {
    #[cfg(test)]
    fixture_origin: Option<String>,
}

impl AssetResponseTransport {
    async fn fetch(&self, settings: &ProxySettings, request: &MetadataRequest) -> Result<Vec<u8>> {
        #[cfg(test)]
        if let Some(origin) = &self.fixture_origin {
            return transfer_tests::fetch_fixture(settings, request, origin).await;
        }
        search::fetch(settings, request).await
    }

    async fn request(
        &self,
        settings: &ProxySettings,
        url: &Url,
        delivery: &Url,
    ) -> Result<reqwest::RequestBuilder> {
        #[cfg(test)]
        if let Some(origin) = &self.fixture_origin {
            return transfer_tests::fixture_request(settings, url, origin);
        }
        Ok(
            features::client_with_timeout(url, settings, Duration::from_secs(30 * 60))
                .await?
                .get(delivery.clone()),
        )
    }
}

#[cfg(test)]
pub(crate) async fn fixture_asset_response(
    settings: &ProxySettings,
    job: &Job,
    root: &Path,
    checkpoint: Option<&crate::transfer::Checkpoint>,
    origin: &str,
) -> Result<reqwest::Response> {
    asset_response_inner(
        settings,
        job,
        root,
        checkpoint,
        &AssetResponseTransport {
            fixture_origin: Some(origin.into()),
        },
    )
    .await
}

async fn asset_response_inner(
    settings: &ProxySettings,
    job: &Job,
    root: &Path,
    checkpoint: Option<&crate::transfer::Checkpoint>,
    transport: &AssetResponseTransport,
) -> Result<reqwest::Response> {
    let asset = validate_job(root, job)?;
    let pin = job.stac_source.as_ref().unwrap();
    let (saved, snapshot) = record(root, &pin.snapshot_id)?;
    if saved.kind != "raster" {
        let original = json_document(&document(root, &saved.document_sha256)?)?;
        let original_item = if let Some(index) = saved.item_index {
            &original["features"][index]
        } else {
            &original
        };
        let page_request = revalidation_request(&saved, original_item)?;
        let item_url = page_request.validate()?;
        let fresh = json_document(&transport.fetch(settings, &page_request).await?)?;
        let index = if fresh["type"] == "FeatureCollection" {
            let features = fresh["features"]
                .as_array()
                .ok_or("STAC source page has no items")?;
            let matches = features
                .iter()
                .enumerate()
                .filter(|(_, v)| {
                    v["id"] == snapshot.item_id
                        && v.get("collection").and_then(Value::as_str)
                            == snapshot.collection_id.as_deref()
                })
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            if matches.len() != 1 {
                return Err(
                    "Selected STAC item changed or disappeared; search the catalog again".into(),
                );
            }
            Some(matches[0])
        } else {
            None
        };
        let current_record = Record {
            document_url: item_url.to_string(),
            document_request: Some(page_request),
            item_index: index,
            ..saved
        };
        let current = item(&current_record, &pin.snapshot_id, &fresh)?;
        let current_asset = current.assets.iter().find(|a| a.key == pin.asset_key);
        let saved_asset = snapshot.assets.iter().find(|a| a.key == pin.asset_key);
        if current.item_id != snapshot.item_id
            || current.collection_id != snapshot.collection_id
            || current_asset != saved_asset
            || current.properties != snapshot.properties
            || current.geometry != snapshot.geometry
            || current.bbox != snapshot.bbox
        {
            return Err("Selected STAC item or original asset metadata changed; refresh the source before downloading".into());
        }
    }
    let url = public_url(&asset.href)?;
    let delivery = planetary_access::delivery_url(settings, root, pin)
        .await?
        .unwrap_or_else(|| url.clone());
    let mut request = transport.request(settings, &url, &delivery).await?.header(
        "Accept",
        if crate::extension(&job.media_type) == Ok("zip") {
            "application/zip, application/octet-stream"
        } else {
            "image/tiff, application/octet-stream"
        },
    );
    if let Some(pin) = job.agent_approval.as_ref().and_then(|a| a.remote.as_ref()) {
        request = request.header(reqwest::header::IF_MATCH, &pin.etag);
    }
    crate::transfer::request(request, checkpoint)
        .send()
        .await
        .map_err(|_| "Cannot reach the public original raster asset".into())
}

pub(crate) async fn load(root: &Path) -> Result<Registry> {
    tokio::fs::create_dir_all(root.join("stac"))
        .await
        .map_err(io_error)?;
    directory(root)?;
    let path = root.join("stac-connections.json");
    let registry: Registry = if path.try_exists().map_err(io_error)? {
        serde_json::from_slice(&read_file(&path, MAX_REGISTRY)?).map_err(io_error)?
    } else {
        Registry::default()
    };
    validate_registry_capacity(&registry)?;
    for (id, connection) in &registry.connections {
        if !uuid(id)
            || id != &connection.id
            || !text(&connection.name, 80)
            || !matches!(
                connection.kind.as_str(),
                "api" | "item" | "raster" | "catalog"
            )
            || connection.collections.len() > 512
            || connection.snapshot_ids.len() > 1
            || chrono::DateTime::parse_from_rfc3339(&connection.connected_at).is_err()
        {
            return Err("Invalid saved STAC connection".into());
        }
        let url = public_url(&connection.url)?;
        if let Some(search) = &connection.search_url {
            scoped(&url, search)?;
        }
        for digest in &connection.metadata_sha256 {
            document(root, digest)?;
        }
        for snapshot in &connection.snapshot_ids {
            let (_, item) = record(root, snapshot)?;
            if item.connection_id != *id {
                return Err("STAC snapshot belongs to a different connection".into());
            }
        }
        validate_connection_documents(root, connection)?;
    }
    Ok(registry)
}
fn capacity(total: usize, catalogs: usize) -> Result<()> {
    if total > MAX_CONNECTIONS {
        return Err(format!(
            "STAC connection limit is {MAX_CONNECTIONS}, including direct raster files"
        ));
    }
    if catalogs > MAX_CATALOG_CONNECTIONS {
        return Err(format!(
            "STAC API, item and catalog connection limit is {MAX_CATALOG_CONNECTIONS}; direct raster files share the total limit of {MAX_CONNECTIONS}"
        ));
    }
    Ok(())
}
fn validate_registry_capacity(registry: &Registry) -> Result<()> {
    capacity(
        registry.connections.len(),
        registry
            .connections
            .values()
            .filter(|c| c.kind != "raster")
            .count(),
    )
}
fn admit_connection(registry: &Registry, candidate: &Connection) -> Result<()> {
    // Reconnecting a selected source replaces only its registry entry. It never
    // evicts archived metadata, snapshots, project selections or downloaded files.
    let replacing = registry.connections.contains_key(&candidate.id);
    let catalogs = registry
        .connections
        .values()
        .filter(|c| c.id != candidate.id && c.kind != "raster")
        .count()
        + usize::from(candidate.kind != "raster");
    capacity(
        registry.connections.len() + usize::from(!replacing),
        catalogs,
    )
}
fn validate_connection_documents(root: &Path, c: &Connection) -> Result<()> {
    if c.kind == "catalog" {
        return catalog::validate_connection(root, c);
    }
    if !c.catalog_nodes.is_empty() {
        return Err("Non-catalog source has static directory metadata".into());
    }
    let root_url = public_url(&c.url)?;
    if c.metadata_documents.len() > 22 || c.metadata_sha256.len() > 22 {
        return Err("STAC connection metadata exceeds its document limit".into());
    }
    if c.kind == "raster" {
        if c.capabilities.search_get
            || c.capabilities.search_post
            || c.search_method != SearchMethod::Get
            || c.search_url.is_some()
            || !c.collections.is_empty()
            || c.snapshot_ids.len() != 1
            || c.metadata_sha256.len() != 1
        {
            return Err("Direct raster connection has conflicting STAC metadata".into());
        }
        let raw = json_document(&document(root, &c.metadata_sha256[0])?)?;
        if raw["href"] != c.url
            || !matches!(
                raw["kind"].as_str(),
                Some("direct-raster" | "reviewed-raster-archive")
            )
        {
            return Err("Direct raster connection differs from its receipt".into());
        }
        let (saved, _) = record(root, &c.snapshot_ids[0])?;
        if saved.kind != "raster"
            || saved.connection_id != c.id
            || saved.document_url != c.url
            || saved.document_sha256 != c.metadata_sha256[0]
            || saved.metadata_documents != c.metadata_documents
        {
            return Err(
                "Direct raster connection differs from its archived source and catalog receipts"
                    .into(),
            );
        }
        return Ok(());
    }
    let receipts = &c.metadata_documents;
    if receipts.is_empty()
        || receipts[0].url != c.url
        || receipts
            .iter()
            .map(|r| r.sha256.clone())
            .collect::<Vec<_>>()
            != c.metadata_sha256
    {
        return Err("STAC connection metadata receipts changed".into());
    }
    let landing = json_document(&document(root, &receipts[0].sha256)?)?;
    let mut collections = Vec::new();
    if c.kind == "api" {
        if !c.snapshot_ids.is_empty() {
            return Err("STAC API connection has a standalone snapshot".into());
        }
        version(&landing)?;
        let searches = search::endpoints(&landing, &root_url)?;
        let endpoint = match c.search_method {
            SearchMethod::Get => &searches.get,
            SearchMethod::Post => &searches.post,
        };
        if endpoint.is_none()
            || c.search_url.as_deref() != endpoint.as_ref().map(|s| s.url.as_str())
            || c.capabilities.search_get != searches.get.is_some()
            || c.capabilities.search_post != searches.post_advertised
        {
            return Err(
                "Saved STAC search capabilities differ from their original metadata".into(),
            );
        }
        let mut expected =
            link(&landing, "data", &root_url)?.ok_or("STAC collections link missing")?;
        for (index, receipt) in receipts.iter().enumerate().skip(1) {
            if receipt.url != expected.as_str() {
                return Err("Saved STAC collection page URL changed".into());
            }
            let value = json_document(&document(root, &receipt.sha256)?)?;
            for c in value["collections"]
                .as_array()
                .ok_or("STAC collections missing")?
            {
                collections.push(collection(c)?);
            }
            let next = link(&value, "next", &expected)?;
            if index + 1 < receipts.len() {
                expected = next.ok_or("Saved STAC collections include an unrequested page")?;
            } else if next.is_some() {
                return Err("Saved STAC collection discovery is incomplete".into());
            }
        }
        if receipts.len() < 2 {
            return Err("Saved STAC collections have no source documents".into());
        }
    } else {
        if c.snapshot_ids.len() != 1
            || c.search_url.is_some()
            || c.capabilities.search_get
            || c.capabilities.search_post
            || c.search_method != SearchMethod::Get
        {
            return Err("Standalone STAC item has conflicting search metadata".into());
        }
        let collection_url = link(&landing, "collection", &root_url)?;
        let (saved, _) = record(root, &c.snapshot_ids[0])?;
        if saved.kind != "item"
            || saved.connection_id != c.id
            || saved.document_url != c.url
            || saved.document_sha256 != receipts[0].sha256
            || saved.metadata_documents != c.metadata_documents
        {
            return Err(
                "Standalone STAC connection differs from its archived item receipts".into(),
            );
        }
        let jaxa = saved
            .reviewed_dataset
            .as_ref()
            .filter(|proof| crate::datasets::is_reviewed_jaxa(&proof.dataset.id));
        let maxar = saved
            .reviewed_dataset
            .as_ref()
            .filter(|proof| proof.dataset.id == "maxar-open-legacy");
        if receipts.len()
            != if collection_url.is_some() {
                2 + usize::from(maxar.is_some())
            } else {
                1
            }
        {
            return Err("Standalone STAC collection receipt is missing".into());
        }
        if let Some(url) = collection_url {
            if receipts[1].url != url.as_str() {
                return Err("Standalone STAC collection source changed".into());
            }
            let metadata = json_document(&document(root, &receipts[1].sha256)?)?;
            let value = collection(&metadata)?;
            if let Some(proof) = jaxa {
                crate::datasets::validate_jaxa_collection(proof, url.as_str(), &metadata)?;
            }
            if let Some(proof) = maxar {
                crate::datasets::maxar::validate_collection(proof, url.as_str(), &metadata)?;
            }
            if let Some(proof) = saved
                .reviewed_dataset
                .as_ref()
                .filter(|p| p.dataset.id == "vantor-open")
            {
                crate::datasets::vantor_open::validate_collection(proof, url.as_str(), &metadata)?;
            }
            if let Some(proof) = saved.reviewed_dataset.as_ref().filter(|p| {
                crate::datasets::next_planetary::is_product(&p.dataset.id)
                    || crate::datasets::additional_pc::is_product(&p.dataset.id)
            }) {
                validate_planetary_collection(proof, url.as_str(), &metadata)?;
            }
            let mismatch = landing.get("collection").is_some_and(|id| *id != value.id);
            if mismatch && jaxa.is_none() {
                return Err("STAC item's collection link has a different identity".into());
            }
            if !mismatch {
                collections.push(value);
            }
        }
    }
    if collections != c.collections {
        return Err("Saved STAC collections differ from their original metadata".into());
    }
    Ok(())
}

impl JobManager {
    pub(crate) async fn archive_dataset_catalog(
        &self,
        href: &str,
    ) -> Result<(Value, DocumentReceipt)> {
        // The product-specific query recipe is validated before reading this
        // bounded public JSON. Preserve the response bytes, not a fake STAC.
        let url = public_url(href)?;
        let settings = self.proxy_settings().await;
        let bytes = tokio::time::timeout(Duration::from_secs(60), fetch(&settings, &url))
            .await
            .map_err(|_| "Publisher catalog request timed out")??;
        let value = json_document(&bytes)?;
        let sha256 = immutable(&self.inner.root, "document", &bytes)?;
        Ok((
            value,
            DocumentReceipt {
                url: href.into(),
                sha256,
            },
        ))
    }
    pub async fn list_stac_connections(&self) -> Vec<Connection> {
        self.inner
            .stac
            .lock()
            .await
            .connections
            .values()
            .cloned()
            .collect()
    }
    pub async fn stac_snapshot(&self, id: &str) -> Result<ItemSnapshot> {
        record(&self.inner.root, id).map(|(_, s)| s)
    }
    pub async fn stac_selection(&self, pin: &Selection) -> Result<(ProjectItem, SourcePin)> {
        Ok((resolve(&self.inner.root, pin)?, pin.clone()))
    }
    pub async fn forget_stac_connection(&self, id: &str) -> Result<()> {
        self.inner.store.lock().await.accepting_jobs()?;
        let mut registry = self.inner.stac.lock().await;
        let old = registry
            .connections
            .remove(id)
            .ok_or("Unknown STAC connection")?;
        if let Err(error) = persist(&self.inner.root, &registry) {
            registry.connections.insert(id.into(), old);
            return Err(error);
        }
        registry
            .cursors
            .retain(|_, c| c.request.connection_id != id);
        registry
            .catalog_cursors
            .retain(|_, c| c.request.connection_id != id);
        Ok(())
    }
    pub async fn connect_stac(&self, request: ConnectRequest) -> Result<Connection> {
        self.connect_stac_inner(request, None, None).await
    }
    pub(crate) async fn connect_dataset_raster(
        &self,
        request: ConnectRequest,
        provenance: crate::datasets::SourceProvenance,
    ) -> Result<Connection> {
        if request.kind != "raster" {
            return Err(
                "Public dataset presets require the original direct raster connector".into(),
            );
        }
        crate::datasets::validate_provenance(&provenance, &request.url)?;
        self.connect_stac_inner(request, Some(provenance), None)
            .await
    }
    pub(crate) async fn connect_dataset_item(
        &self,
        request: ConnectRequest,
        provenance: crate::datasets::SourceProvenance,
    ) -> Result<Connection> {
        if request.kind != "item" {
            return Err(
                "Reviewed regional discoveries require actual standalone STAC items".into(),
            );
        }
        crate::datasets::validate_reviewed_item(&provenance, &request.url, None)?;
        self.connect_stac_inner(request, None, Some(provenance))
            .await
    }
    async fn connect_stac_inner(
        &self,
        request: ConnectRequest,
        preset: Option<crate::datasets::SourceProvenance>,
        reviewed_item: Option<crate::datasets::SourceProvenance>,
    ) -> Result<Connection> {
        self.inner.store.lock().await.accepting_jobs()?;
        if !text(request.name.trim(), 80)
            || !matches!(request.kind.as_str(), "api" | "item" | "raster" | "catalog")
        {
            return Err("Choose a source type and a name of 1–80 characters".into());
        }
        let url = public_url(request.url.trim())?;
        let settings = self.proxy_settings().await;
        let root = self.inner.root.clone();
        let existing = self
            .inner
            .stac
            .lock()
            .await
            .connections
            .values()
            .find(|c| c.url == url.as_str() && c.kind == request.kind)
            .map(|c| c.id.clone());
        let mut connection = Connection {
            id: existing.unwrap_or_else(|| Uuid::new_v4().to_string()),
            name: request.name.trim().into(),
            url: url.to_string(),
            kind: request.kind.clone(),
            connected_at: now(),
            collections: vec![],
            capabilities: Capabilities {
                search_get: false,
                search_post: false,
            },
            snapshot_ids: vec![],
            search_url: None,
            search_method: SearchMethod::Get,
            metadata_sha256: vec![],
            metadata_documents: vec![],
            catalog_nodes: vec![],
        };
        tokio::time::timeout(Duration::from_secs(if request.kind == "catalog" { 180 } else { 60 }),async {
            if request.kind == "catalog" { return catalog::discover(&root, &settings, &mut connection).await; }
            if request.kind=="raster" {
                let archive = preset.as_ref().map(|proof| archive_original::from_proof(proof, url.as_str())).transpose()?.flatten();
                let raw_original = raw_original_policy(preset.as_ref(),url.as_str())?;
                let client=features::client(&url,&settings).await?;
                let mut probe=client.get(url.clone()).header("Range","bytes=0-15");
                if raw_original.is_some() { probe=probe.header(reqwest::header::ACCEPT_ENCODING,"identity"); }
                let response=probe.send().await.map_err(|_|"Cannot inspect the public raster URL")?;
                if !matches!(response.status(),reqwest::StatusCode::OK|reqwest::StatusCode::PARTIAL_CONTENT){return Err(format!("Raster source returned HTTP {}; redirects are not followed",response.status()));}
                let status=response.status().as_u16();let length=response.content_length();let content_range=response.headers().get("content-range").and_then(|v|v.to_str().ok()).map(str::to_owned);
                if let Some(p) = &raw_original {
                    let headers = response.headers();
                    if headers.get_all(reqwest::header::CONTENT_LENGTH).iter().count() != 1
                        || headers.contains_key(reqwest::header::TRANSFER_ENCODING)
                        || headers.get_all(reqwest::header::CONTENT_ENCODING).iter().count() > 1
                        || headers.get(reqwest::header::CONTENT_ENCODING).is_some_and(|v| v != "identity")
                        || (status == 206 && (headers.get_all(reqwest::header::CONTENT_RANGE).iter().count() != 1 || content_range.as_deref() != Some(format!("bytes 0-15/{}",p.bytes).as_str()) || length != Some(16)))
                        || (status == 200 && (headers.contains_key(reqwest::header::CONTENT_RANGE) || length != Some(p.bytes)))
                    { return Err("CLCD original probe differs from its exact complete representation".into()); }
                }
                if status==206&&!content_range.as_ref().is_some_and(|s|s.starts_with("bytes 0-15/")){return Err("Raster probe returned a different byte range".into());}
                let mut header=Vec::new();let mut stream=response.bytes_stream();while header.len()<16{let chunk=stream.next().await.ok_or("Raster header was truncated")?.map_err(|_|"Raster header was interrupted")?;header.extend(chunk.iter().take(16-header.len()));}
                if archive.is_some() {
                    if !header.starts_with(b"PK\x03\x04") { return Err("The reviewed publisher archive URL did not return an original ZIP signature".into()); }
                } else if !matches!(header.get(..4),Some(b"II*\0"|b"MM\0*"|b"II+\0"|b"MM\0+")){return Err("The public URL did not return a TIFF/BigTIFF header".into());}
                let mut direct=json!({"kind":"direct-raster","href":url.as_str(),"probeStatus":status,"contentLength":length,"contentRange":content_range,"headerHex":header.iter().map(|b|format!("{b:02x}")).collect::<String>(),"cogLayout":"not-validated"});
                if let Some(provenance) = &preset { direct["datasetPreset"] = serde_json::to_value(provenance).map_err(io_error)?; }
                if let Some(p) = &raw_original {
                    direct["reviewedOriginal"] = json!({"bytes":p.bytes,"sha256":p.sha256});
                    direct["file:size"] = json!(p.bytes);
                    direct["file:checksum"] = json!(format!("1220{}",p.sha256));
                }
                if let Some(archive) = &archive {
                    let actual_size = if status == 206 { content_range.as_ref().and_then(|s|s.rsplit_once('/')).and_then(|(_,n)|n.parse::<u64>().ok()) } else { length };
                    if actual_size.is_some_and(|bytes| bytes != archive.archive_bytes) { return Err("Publisher ZIP current byte length differs from its reviewed complete original".into()); }
                    direct["kind"] = json!("reviewed-raster-archive");
                    direct["reviewedArchive"] = serde_json::to_value(archive).map_err(io_error)?;
                    direct["file:size"] = json!(archive.archive_bytes);
                    direct["file:checksum"] = json!(format!("1220{}",archive.archive_sha256));
                    direct["cogLayout"] = json!("original ZIP; selected whole TIFF remains separately checksummed");
                }
                let catalog_receipts = if let Some(catalog) = preset.as_ref().and_then(|p| p.catalog_raster.as_ref()) {
                    direct["file:size"] = catalog.record["sizeInBytes"].clone();
                    let actual_size = if status == 206 { content_range.as_ref().and_then(|s|s.rsplit_once('/')).and_then(|(_,n)|n.parse::<u64>().ok()) } else { length };
                    if actual_size.is_some_and(|size| Some(size) != catalog.record["sizeInBytes"].as_u64()) { return Err("Publisher catalog size disagrees with the current original file".into()); }
                    vec![DocumentReceipt { url: catalog.catalog_url.clone(), sha256: catalog.catalog_sha256.clone() }]
                } else { vec![] };
                let bytes=serde_json::to_vec(&direct).map_err(io_error)?;let digest=immutable(&root,"document",&bytes)?;connection.metadata_sha256.push(digest.clone());
                connection.metadata_documents = catalog_receipts.clone();
                let snapshot=save_snapshot(&root,Record{version:1,connection_id:connection.id.clone(),service_name:connection.name.clone(),kind:"raster".into(),document_url:url.to_string(),document_request:None,document_sha256:digest,item_index:None,retrieved_at:connection.connected_at.clone(),metadata_documents:catalog_receipts,search:None,reviewed_dataset:None,asset_transports:Vec::new()},&direct)?;connection.snapshot_ids.push(snapshot.id);return Ok(());
            }
            let bytes=fetch(&settings,&url).await?;let value=json_document(&bytes)?;version(&value)?;let digest=immutable(&root,"document",&bytes)?;connection.metadata_sha256.push(digest.clone());connection.metadata_documents.push(DocumentReceipt{url:url.to_string(),sha256:digest.clone()});
            if request.kind=="item" {
                let transports = reviewed_item.as_ref().map(|proof|crate::datasets::reviewed_item_transports(proof,url.as_str(),&value)).transpose()?.unwrap_or_default();
                let mut receipts=connection.metadata_documents.clone();
                if let Some(collection_url) = link(&value, "collection", &url)? {
                    let bytes = fetch(&settings, &collection_url).await?;
                    let metadata = json_document(&bytes)?;
                    let c = collection(&metadata)?;
                    let jaxa = reviewed_item.as_ref().filter(|proof| crate::datasets::is_reviewed_jaxa(&proof.dataset.id));
                    if let Some(proof) = jaxa { crate::datasets::validate_jaxa_collection(proof, collection_url.as_str(), &metadata)?; }
                    if let Some(proof) = reviewed_item.as_ref().filter(|proof| proof.dataset.id == "maxar-open-legacy") { crate::datasets::maxar::validate_collection(proof, collection_url.as_str(), &metadata)?; }
                    if let Some(proof) = reviewed_item.as_ref().filter(|proof| proof.dataset.id == "vantor-open") { crate::datasets::vantor_open::validate_collection(proof, collection_url.as_str(), &metadata)?; }
                    if let Some(proof) = reviewed_item.as_ref().filter(|p| crate::datasets::next_planetary::is_product(&p.dataset.id) || crate::datasets::additional_pc::is_product(&p.dataset.id)) { validate_planetary_collection(proof, collection_url.as_str(), &metadata)?; }
                    let mismatch = value.get("collection").is_some_and(|id| *id != c.id);
                    if mismatch && jaxa.is_none() { return Err("STAC item's collection link has a different identity".into()); }
                    // The scoped JAXA adapter preserves both original IDs and
                    // the complete collection document. A generic "collection"
                    // is not aliased to the item's product or marked verified.
                    let sha = immutable(&root, "document", &bytes)?;
                    receipts.push(DocumentReceipt { url: collection_url.to_string(), sha256: sha.clone() });
                    connection.metadata_sha256.push(sha);
                    if !mismatch { connection.collections.push(c); }
                }
                if let Some(proof) = reviewed_item.as_ref().filter(|proof| proof.dataset.id == "maxar-open-legacy") {
                    let event_url = public_url(&crate::datasets::maxar::event_url(proof)?)?;
                    let bytes = fetch(&settings, &event_url).await?;
                    let event = json_document(&bytes)?;
                    crate::datasets::maxar::validate_event(proof, event_url.as_str(), &event)?;
                    let sha = immutable(&root, "document", &bytes)?;
                    receipts.push(DocumentReceipt { url: event_url.to_string(), sha256: sha.clone() });
                    connection.metadata_sha256.push(sha);
                }
                connection.metadata_documents=receipts.clone();let snapshot=save_snapshot(&root,Record{version:1,connection_id:connection.id.clone(),service_name:connection.name.clone(),kind:"item".into(),document_url:url.to_string(),document_request:None,document_sha256:digest,item_index:None,retrieved_at:connection.connected_at.clone(),metadata_documents:receipts,search:None,reviewed_dataset:reviewed_item.clone(),asset_transports:transports},&value)?;connection.snapshot_ids.push(snapshot.id);return Ok(());
            }
            if !matches!(value["type"].as_str(),Some("Catalog"|"Collection")){return Err("The STAC API landing page must be a Catalog or Collection".into());}
            let conforms=value["conformsTo"].as_array().ok_or("STAC API must advertise Item Search conformance; choose Static STAC catalog for directory traversal")?;
            if !conforms.iter().any(|v|v.as_str().is_some_and(|s|matches!(s,"https://api.stacspec.org/v1.0.0/item-search"|"https://api.stacspec.org/v1.0.0-rc.3/item-search"))){return Err("This source does not advertise supported STAC API Item Search; choose Static STAC catalog for directory traversal".into());}
            let searches=search::endpoints(&value,&url)?;
            connection.capabilities.search_get=searches.get.is_some();connection.capabilities.search_post=searches.post_advertised;
            let endpoint=searches.post.or(searches.get).ok_or("STAC search is unavailable")?;
            connection.search_method=endpoint.method;connection.search_url=Some(endpoint.url);
            let mut next=Some(link(&value,"data",&url)?.ok_or("STAC API has no collections link")?);let mut visited=BTreeSet::new();let mut ids=BTreeSet::new();let mut total=bytes.len();
            while let Some(page)=next {
                if visited.len()>=20||!visited.insert(page.to_string()){return Err("STAC collection discovery exceeds its page limit or repeats a page".into());}
                let bytes=fetch(&settings,&page).await?;total+=bytes.len();if total>MAX_SEARCH_BYTES{return Err("STAC collection metadata exceeds 20 MiB".into());}
                let value=json_document(&bytes)?;let sha=immutable(&root,"document",&bytes)?;connection.metadata_sha256.push(sha.clone());connection.metadata_documents.push(DocumentReceipt{url:page.to_string(),sha256:sha});
                for entry in value["collections"].as_array().ok_or("STAC API has no collections array")?{
                    let c=collection(entry)?;if !ids.insert(c.id.clone())||connection.collections.len()>=512{return Err("STAC collections repeat IDs or exceed 512 collections".into());}connection.collections.push(c);
                }next=link(&value,"next",&page)?;
            }Ok(())
        }).await.map_err(|_|"STAC connection discovery timed out")??;
        let mut registry = self.inner.stac.lock().await;
        admit_connection(&registry, &connection)?;
        let old = registry
            .connections
            .insert(connection.id.clone(), connection.clone());
        if let Err(e) = persist(&self.inner.root, &registry) {
            registry.connections.remove(&connection.id);
            if let Some(old) = old {
                registry.connections.insert(old.id.clone(), old);
            }
            return Err(e);
        }
        registry
            .cursors
            .retain(|_, c| c.request.connection_id != connection.id);
        registry
            .catalog_cursors
            .retain(|_, c| c.request.connection_id != connection.id);
        Ok(connection)
    }
    pub async fn search_stac(&self, request: SearchRequest) -> Result<SearchPage> {
        self.inner.store.lock().await.accepting_jobs()?;
        features::bounds(request.bounds)?;
        if self
            .inner
            .stac
            .lock()
            .await
            .connections
            .get(&request.connection_id)
            .is_some_and(|c| c.kind == "catalog")
        {
            return self.search_stac_catalog(request).await;
        }
        let limit = request.limit.unwrap_or(100);
        if !(1..=100).contains(&limit) {
            return Err("STAC page size must be between 1 and 100".into());
        }
        if let Some(datetime) = &request.datetime {
            validate_datetime(datetime)?;
        }
        let (connection, mut cursor) = {
            let registry = self.inner.stac.lock().await;
            let connection = registry
                .connections
                .get(&request.connection_id)
                .cloned()
                .ok_or("Unknown STAC connection")?;
            if connection.kind != "api"
                || !connection
                    .collections
                    .iter()
                    .any(|c| c.id == request.collection_id)
            {
                return Err("Choose a discovered STAC collection".into());
            }
            let cursor = if let Some(id) = &request.cursor {
                let c = registry
                    .cursors
                    .get(id)
                    .cloned()
                    .ok_or("STAC search cursor expired; start this search again")?;
                let mut expected = request.clone();
                expected.cursor = None;
                if c.request != expected {
                    return Err("STAC cursor belongs to different search filters".into());
                }
                c
            } else {
                let landing = json_document(&document(
                    &self.inner.root,
                    &connection.metadata_documents[0].sha256,
                )?)?;
                let endpoints = search::endpoints(&landing, &public_url(&connection.url)?)?;
                let endpoint = match connection.search_method {
                    SearchMethod::Get => endpoints.get,
                    SearchMethod::Post => endpoints.post,
                }
                .ok_or("STAC search is unavailable")?;
                let page = search::initial(endpoint, &request)?;
                let mut original = request.clone();
                original.cursor = None;
                Cursor {
                    request: original,
                    original: page.clone(),
                    page,
                    visited: BTreeSet::new(),
                    identities: BTreeSet::new(),
                    pages: 0,
                    bytes: 0,
                }
            };
            (connection, cursor)
        };
        if cursor.pages >= MAX_PAGES || cursor.identities.len() >= MAX_ITEMS {
            return Err("STAC search limit reached; narrow the region or time interval".into());
        }
        if !cursor.visited.insert(cursor.page.identity()?) {
            return Err("STAC repeated a pagination link".into());
        }
        let url = scoped(&public_url(&connection.url)?, &cursor.page.url)?;
        let settings = self.proxy_settings().await;
        let bytes = tokio::time::timeout(
            Duration::from_secs(45),
            search::fetch(&settings, &cursor.page),
        )
        .await
        .map_err(|_| "STAC search timed out")??;
        cursor.bytes += bytes.len();
        cursor.pages += 1;
        if cursor.bytes > MAX_SEARCH_BYTES {
            return Err("STAC search metadata exceeds 20 MiB; narrow the search".into());
        }
        let value = json_document(&bytes)?;
        if value["type"] != "FeatureCollection" {
            return Err("STAC search must return a FeatureCollection".into());
        }
        let features = value["features"]
            .as_array()
            .filter(|a| a.len() <= limit)
            .ok_or("STAC search returned more than the requested page size")?;
        let digest = immutable(&self.inner.root, "document", &bytes)?;
        let mut items = Vec::new();
        let retrieved = now();
        for index in 0..features.len() {
            let snapshot = save_snapshot(
                &self.inner.root,
                Record {
                    version: 1,
                    connection_id: connection.id.clone(),
                    service_name: connection.name.clone(),
                    kind: "api".into(),
                    document_url: url.to_string(),
                    document_request: Some(cursor.page.clone()),
                    document_sha256: digest.clone(),
                    item_index: Some(index),
                    retrieved_at: retrieved.clone(),
                    metadata_documents: connection.metadata_documents.clone(),
                    search: Some(cursor.request.clone()),
                    reviewed_dataset: None,
                    asset_transports: Vec::new(),
                },
                &value,
            )?;
            if snapshot.collection_id.as_deref() != Some(&request.collection_id)
                || !cursor.identities.insert(snapshot.item_id.clone())
            {
                return Err(
                    "STAC search returned a foreign collection or duplicate item ID".into(),
                );
            }
            items.push(snapshot);
        }
        if value
            .get("numberReturned")
            .is_some_and(|v| v.as_u64() != Some(items.len() as u64))
        {
            return Err("STAC search declared a different returned count".into());
        }
        let next = search::next(&value, &cursor.page, &cursor.original)?;
        let matched = value
            .get("numberMatched")
            .or_else(|| value.get("context").and_then(|v| v.get("matched")));
        if let Some(matched) = matched {
            if let Some(n) = matched.as_u64() {
                if (n as usize) < cursor.identities.len()
                    || next.is_none() && n as usize != cursor.identities.len()
                {
                    return Err("STAC pagination ended before its declared matching count".into());
                }
            }
        }
        let complete = next.is_none();
        let limit_reached = !complete
            && (cursor.pages >= MAX_PAGES
                || cursor.identities.len() >= MAX_ITEMS
                || cursor.bytes >= MAX_SEARCH_BYTES);
        let next_cursor = if !complete && !limit_reached {
            let next = next.unwrap();
            if cursor.visited.contains(&next.identity()?) {
                return Err("STAC repeated a pagination link".into());
            }
            cursor.page = next;
            Some(Uuid::new_v4().to_string())
        } else {
            None
        };
        let mut registry = self.inner.stac.lock().await;
        if registry.connections.get(&connection.id) != Some(&connection) {
            return Err("STAC connection changed while the search was running".into());
        }
        if let Some(previous) = &request.cursor {
            registry.cursors.remove(previous);
        }
        if let Some(id) = &next_cursor {
            if registry.cursors.len() >= 64 {
                registry.cursors.clear();
            }
            registry.cursors.insert(id.clone(), cursor);
        }
        Ok(SearchPage {
            items,
            next_cursor,
            complete,
            limit_reached,
            scanned_items: None,
        })
    }
}
fn validate_datetime(value: &str) -> Result<()> {
    if value.len() > 160 {
        return Err("STAC datetime filter is too long".into());
    }
    let values = value.split('/').collect::<Vec<_>>();
    if values.is_empty()
        || values.len() > 2
        || values
            .iter()
            .any(|v| *v != ".." && chrono::DateTime::parse_from_rfc3339(v).is_err())
        || values.iter().all(|v| *v == "..")
    {
        return Err("STAC datetime must be RFC3339 or an RFC3339 interval".into());
    }
    if values.len() == 2
        && values[0] != ".."
        && values[1] != ".."
        && chrono::DateTime::parse_from_rfc3339(values[0]).map_err(io_error)?
            > chrono::DateTime::parse_from_rfc3339(values[1]).map_err(io_error)?
    {
        return Err("STAC datetime interval is reversed".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod clcd_original_policy_tests {
    use super::*;

    #[test]
    fn arbitrary_direct_sources_cannot_attach_a_raw_original_allowance() {
        let raw = json!({"kind":"direct-raster","href":"https://example.com/original.tif",
            "probeStatus":206,"contentLength":16,"contentRange":"bytes 0-15/791887020",
            "file:size":791887020,"file:checksum":format!("1220{}","a".repeat(64)),
            "reviewedOriginal":{"bytes":791887020,"sha256":"a".repeat(64)}});
        assert!(
            validate_raw_original_receipt(None, "https://example.com/original.tif", &raw).is_err()
        );
        assert!(validate_raw_original_receipt(None, crate::datasets::clcd::URL, &raw).is_err());
    }

    #[test]
    fn direct_probe_keeps_complete_length_separate_from_header_bytes() {
        let mut raw =
            json!({"probeStatus":206,"contentLength":16,"contentRange":"bytes 0-15/791887020"});
        assert_eq!(direct_original_size(&raw).unwrap(), Some(791887020));
        raw["contentLength"] = json!(15);
        assert!(direct_original_size(&raw).is_err());
        raw["contentLength"] = json!(16);
        for range in [
            "bytes 1-16/791887020",
            "bytes 0-15/*",
            "bytes 0-15/0",
            "bytes 0-15/791887020, bytes 0-15/791887020",
        ] {
            raw["contentRange"] = json!(range);
            assert!(direct_original_size(&raw).is_err());
        }
    }

    #[test]
    fn ordinary_direct_large_files_are_ineligible_without_the_exact_native_proof() {
        let href = "https://example.com/large.tif";
        let record = Record {
            version: 1,
            connection_id: Uuid::from_u128(1).to_string(),
            service_name: "Unreviewed TIFF".into(),
            kind: "raster".into(),
            document_url: href.into(),
            document_request: None,
            document_sha256: "b".repeat(64),
            item_index: None,
            retrieved_at: "2026-10-14T00:00:00Z".into(),
            metadata_documents: vec![],
            search: None,
            reviewed_dataset: None,
            asset_transports: vec![],
        };
        let raw = json!({"kind":"direct-raster","href":href,"probeStatus":206,"contentLength":16,
            "contentRange":"bytes 0-15/791887020","headerHex":"49492a00"});
        let snapshot = item(&record, &"c".repeat(64), &raw).unwrap();
        assert!(!snapshot.assets[0].eligible);
        assert!(snapshot.assets[0]
            .reason
            .as_deref()
            .unwrap()
            .contains("limit"));
    }
}
#[cfg(test)]
mod transfer_tests;
#[cfg(test)]
mod vantor_tests;
#[cfg(test)]
pub(crate) fn fixture_snapshot(root: &Path, asset_href: &str) -> SourcePin {
    tests::fixture(root, asset_href)
}
