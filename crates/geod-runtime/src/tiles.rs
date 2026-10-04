//! Public PMTiles v3 MVT discovery, tile-aligned offline extraction and readback.
use crate::{features, io_error, now, storage, JobManager, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;
pub mod format;
mod http;
mod local;
mod mbtiles;
mod mvt;
const MAX_TILES: usize = 512;
const MAX_PACKAGE: usize = 128 * 1024 * 1024;
const MAX_REGISTRY: usize = 32 * 1024 * 1024;
fn hash(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
fn clean(s: &str, max: usize) -> bool {
    !s.is_empty() && s.trim() == s && s.chars().count() <= max && !s.chars().any(char::is_control)
}
fn uuid(s: &str) -> bool {
    Uuid::parse_str(s).is_ok_and(|u| u.to_string() == s)
}
fn digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Source {
    pub id: String,
    pub name: String,
    pub url: String,
    pub etag: String,
    pub total_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<format::Header>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbtiles: Option<mbtiles::Descriptor>,
    pub metadata: serde_json::Value,
    pub connected_at: String,
    pub ranges: Vec<http::Receipt>,
    pub discovery_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local: Option<LocalSource>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalSource {
    pub file_name: String,
    pub sha256: String,
}
fn fingerprint(s: &Source) -> Result<String> {
    let bytes = serde_json::to_vec(&(
        &s.url,
        &s.etag,
        s.total_bytes,
        &s.header,
        &s.metadata,
        &s.ranges,
    ))
    .map_err(io_error)?;
    Ok(if let Some(mbtiles) = &s.mbtiles {
        hash(&serde_json::to_vec(&(bytes, &s.local, mbtiles)).map_err(io_error)?)
    } else if let Some(local) = &s.local {
        hash(&serde_json::to_vec(&(bytes, local)).map_err(io_error)?)
    } else {
        hash(&bytes)
    })
}
fn valid_source(s: &Source) -> Result<()> {
    if s.mbtiles.is_some() {
        return mbtiles::valid_source(s);
    }
    if s.header.is_none() {
        return Err("Missing PMTiles header".into());
    }
    let h = s.header.as_ref().unwrap();
    if let Some(local) = &s.local {
        local::file_name(&local.file_name)?;
        if !local.file_name.to_ascii_lowercase().ends_with(".pmtiles")
            || !s.url.is_empty()
            || !s.etag.is_empty()
            || !digest(&local.sha256)
            || s.total_bytes > MAX_PACKAGE as u64
        {
            return Err("Invalid local PMTiles source".into());
        }
    } else {
        http::source_url(&s.url)?;
        if !http::strong_etag(&s.etag) {
            return Err("Invalid saved PMTiles source".into());
        }
    }
    h.validate(s.total_bytes)?;
    if !uuid(&s.id)
        || !clean(&s.name, 80)
        || chrono::DateTime::parse_from_rfc3339(&s.connected_at).is_err()
        || s.ranges.len() != 3
        || !s.metadata.is_object()
        || !s.metadata["vector_layers"].is_array()
        || fingerprint(s)? != s.discovery_sha256
    {
        return Err("Invalid saved PMTiles source".into());
    }
    let spans = BTreeSet::from([
        (0, 127),
        (h.root_offset, h.root_length),
        (h.metadata_offset, h.metadata_length),
    ]);
    if s.ranges
        .iter()
        .map(|r| (r.offset, r.bytes as u64))
        .collect::<BTreeSet<_>>()
        != spans
        || s.ranges.iter().any(|r| !digest(&r.sha256))
    {
        return Err("PMTiles discovery receipts do not match its header".into());
    }
    Ok(())
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectRequest {
    pub name: String,
    pub url: String,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtractRequest {
    pub source_id: String,
    pub bounds: [f64; 4],
    pub min_zoom: u8,
    pub max_zoom: u8,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TileReceipt {
    pub coordinate: format::Coordinate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_offset: Option<u64>,
    pub bytes: usize,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_offset: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<mbtiles::Image>,
    pub layers: Vec<mvt::Layer>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Package {
    pub id: String,
    pub name: String,
    pub requested_bounds: [f64; 4],
    pub tile_coverage_bounds: [f64; 4],
    pub min_zoom: u8,
    pub max_zoom: u8,
    pub bytes: usize,
    pub sha256: String,
    pub source: Source,
    pub created_at: String,
    pub ranges: Vec<http::Receipt>,
    pub tiles: Vec<TileReceipt>,
    pub absent: Vec<format::Coordinate>,
    pub selection: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inspection {
    pub asset: Package,
    pub metadata: serde_json::Value,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TileRequest {
    pub id: String,
    pub z: u8,
    pub x: u32,
    pub y: u32,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tile {
    pub coordinate: format::Coordinate,
    pub data_base64: Option<String>,
    pub sha256: Option<String>,
    pub layers: Vec<mvt::Layer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Registry {
    pub sources: BTreeMap<String, Source>,
    pub packages: BTreeMap<String, Package>,
}
fn directory(root: &Path) -> Result<PathBuf> {
    let d = root.join("tiles");
    if d.canonicalize().map_err(io_error)? != d {
        return Err("Tile storage was redirected".into());
    }
    Ok(d)
}
fn file(root: &Path, p: &Package) -> Result<PathBuf> {
    let id = &p.id;
    if !uuid(id) {
        return Err("Invalid tile package identifier".into());
    }
    storage::regular_file(&directory(root)?.join(format!("{id}.{}", extension(p))))
}
fn extension(p: &Package) -> &'static str {
    if p.source.mbtiles.is_some() {
        "mbtiles"
    } else {
        "pmtiles"
    }
}
fn persist(root: &Path, r: &Registry) -> Result<()> {
    let b = serde_json::to_vec_pretty(r).map_err(io_error)?;
    if b.len() > MAX_REGISTRY {
        return Err("Tile registry exceeds its size limit".into());
    }
    let mut f = tempfile::NamedTempFile::new_in(root).map_err(io_error)?;
    f.write_all(&b).map_err(io_error)?;
    f.as_file().sync_all().map_err(io_error)?;
    f.persist(root.join("tiles.json")).map_err(io_error)?;
    Ok(())
}
fn valid_package(p: &Package) -> Result<()> {
    if p.source.mbtiles.is_some() {
        return mbtiles::valid_package(p);
    }
    let h = p.source.header.as_ref().ok_or("Missing PMTiles header")?;
    valid_source(&p.source)?;
    features::bounds(p.tile_coverage_bounds)?;
    features::bounds(p.requested_bounds)?;
    let local = p.source.local.is_some();
    let selected = if local {
        Vec::new()
    } else {
        format::selected(p.requested_bounds, p.min_zoom, p.max_zoom)?
    };
    if !uuid(&p.id)
        || !clean(&p.name, 120)
        || !digest(&p.sha256)
        || p.bytes == 0
        || p.bytes > MAX_PACKAGE
        || p.selection
            != if local {
                "imported-archive"
            } else {
                "tile-aligned-pyramid"
            }
        || p.min_zoom > p.max_zoom
        || p.min_zoom < h.min_zoom
        || p.max_zoom > h.max_zoom
        || chrono::DateTime::parse_from_rfc3339(&p.created_at).is_err()
        || p.tiles.is_empty()
        || p.tiles.len() + p.absent.len() > MAX_TILES
        || (!local && p.tiles.len() + p.absent.len() != selected.len())
        || p.ranges.len() > 2048
    {
        return Err("Invalid saved PMTiles package".into());
    }
    if local
        && (p.bytes as u64 != p.source.total_bytes
            || p.source.local.as_ref().unwrap().sha256 != p.sha256
            || p.min_zoom != h.min_zoom
            || p.max_zoom != h.max_zoom
            || p.requested_bounds != h.bounds
            || (h.addressed_tiles != 0 && h.addressed_tiles != p.tiles.len() as u64)
            || !p.absent.is_empty())
    {
        return Err("Local PMTiles import receipt changed".into());
    }
    let expected = selected
        .iter()
        .map(|c| format::tile_id(c.z, c.x, c.y))
        .collect::<Result<BTreeSet<_>>>()?;
    let mut actual = BTreeSet::new();
    for c in p.tiles.iter().map(|t| &t.coordinate).chain(p.absent.iter()) {
        if c.z < p.min_zoom || c.z > p.max_zoom || !actual.insert(format::tile_id(c.z, c.x, c.y)?) {
            return Err("Duplicate saved tile coordinate".into());
        }
    }
    if (!local && expected != actual)
        || p.ranges.iter().any(|r| {
            !digest(&r.sha256)
                || r.bytes == 0
                || r.offset
                    .checked_add(r.bytes as u64)
                    .is_none_or(|n| n > p.source.total_bytes)
        })
    {
        return Err("PMTiles extraction membership or range receipts changed".into());
    }
    for t in &p.tiles {
        if t.image.is_some()
            || !digest(&t.sha256)
            || t.bytes == 0
            || t.bytes > format::MAX_TILE
            || t.package_offset.is_none_or(|n| n < 127)
            || t.source_offset.is_none_or(|n| n < h.tile_offset)
            || (local && t.package_offset != t.source_offset)
            || t.package_offset
                .and_then(|offset| offset.checked_add(t.bytes as u64))
                .is_none_or(|n| n > p.bytes as u64)
            || t.source_offset
                .and_then(|offset| offset.checked_add(t.bytes as u64))
                .is_none_or(|n| n > h.tile_offset + h.tile_length)
            || !p.ranges.iter().any(|r| {
                Some(r.offset) == t.source_offset && r.bytes == t.bytes && r.sha256 == t.sha256
            })
        {
            return Err("PMTiles tile receipt exceeds its source or package".into());
        }
    }
    if local {
        let mut coverage: [f64; 4] = [180., 90., -180., -90.];
        for t in &p.tiles {
            let b = tile_bounds(&t.coordinate);
            coverage = [
                coverage[0].min(b[0]),
                coverage[1].min(b[1]),
                coverage[2].max(b[2]),
                coverage[3].max(b[3]),
            ];
        }
        if coverage != p.tile_coverage_bounds {
            return Err("Local PMTiles import receipt changed".into());
        }
    }
    Ok(())
}
pub(crate) async fn load(root: &Path) -> Result<Registry> {
    tokio::fs::create_dir_all(root.join("tiles"))
        .await
        .map_err(io_error)?;
    directory(root)?;
    let p = root.join("tiles.json");
    if !p.try_exists().map_err(io_error)? {
        return Ok(Registry::default());
    }
    let p = storage::regular_file(&p)?;
    let f = std::fs::File::open(p).map_err(io_error)?;
    if f.metadata().map_err(io_error)?.len() > MAX_REGISTRY as u64 {
        return Err("Tile registry exceeds its size limit".into());
    }
    let r: Registry = serde_json::from_reader(f).map_err(io_error)?;
    if r.sources.len() > 24 || r.packages.len() > 128 {
        return Err("Tile registry exceeds its record limit".into());
    }
    for (id, s) in &r.sources {
        valid_source(s)?;
        if id != &s.id || s.local.is_some() {
            return Err("Tile source identifier changed".into());
        }
    }
    for (id, p) in &r.packages {
        valid_package(p)?;
        if id != &p.id {
            return Err("Tile package identifier changed".into());
        }
    }
    Ok(r)
}
async fn discover(
    name: &str,
    url: &str,
    settings: &crate::ProxySettings,
) -> Result<(Source, http::Reader)> {
    if !clean(name, 80) {
        return Err("Enter a tile source name of 1–80 characters".into());
    }
    let mut r = http::Reader::open(url, settings).await?;
    let (h, metadata) = r.metadata().await?;
    let mut s = Source {
        id: Uuid::new_v4().to_string(),
        name: name.into(),
        url: url.into(),
        etag: r.etag.clone(),
        total_bytes: r.total,
        header: Some(h),
        mbtiles: None,
        metadata,
        connected_at: now(),
        ranges: r.receipts(),
        discovery_sha256: String::new(),
        local: None,
    };
    s.discovery_sha256 = fingerprint(&s)?;
    valid_source(&s)?;
    Ok((s, r))
}
fn tile_bounds(c: &format::Coordinate) -> [f64; 4] {
    let n = (1u32 << c.z) as f64;
    let lon = |x: f64| x / n * 360. - 180.;
    let lat = |y: f64| {
        (std::f64::consts::PI * (1. - 2. * y / n))
            .sinh()
            .atan()
            .to_degrees()
    };
    [
        lon(f64::from(c.x)),
        lat(f64::from(c.y) + 1.),
        lon(f64::from(c.x) + 1.),
        lat(f64::from(c.y)),
    ]
}
async fn extract(
    source: Source,
    request: ExtractRequest,
    settings: &crate::ProxySettings,
) -> Result<(Package, Vec<u8>)> {
    valid_source(&source)?;
    let h = source
        .header
        .as_ref()
        .ok_or("Not a remote PMTiles source")?;
    if request.min_zoom < h.min_zoom || request.max_zoom > h.max_zoom {
        return Err("Choose levels within the PMTiles source range".into());
    }
    let coordinates = format::selected(request.bounds, request.min_zoom, request.max_zoom)?;
    let (current, mut reader) = discover(&source.name, &source.url, settings).await?;
    if current.discovery_sha256 != source.discovery_sha256 {
        return Err("PMTiles archive changed; refresh the connection before extracting".into());
    }
    let h = source.header.as_ref().ok_or("Missing PMTiles header")?;
    let mut data = Vec::new();
    let mut entries = Vec::new();
    let mut receipts = Vec::new();
    let mut absent = Vec::new();
    let mut coverage: [f64; 4] = [180., 90., -180., -90.];
    let mut dedup: BTreeMap<(u64, u64), (u64, Vec<mvt::Layer>)> = BTreeMap::new();
    for c in coordinates {
        let id = format::tile_id(c.z, c.x, c.y)?;
        let Some(entry) = reader.entry(h, id).await? else {
            absent.push(c);
            continue;
        };
        let raw = reader
            .get(h.tile_offset + entry.offset, entry.length)
            .await?;
        let (offset, layers) = if let Some(v) = dedup.get(&(entry.offset, entry.length)) {
            v.clone()
        } else {
            let decoded = format::decompress(&raw, h.tile_compression, format::MAX_TILE)?;
            let layers = mvt::inspect(&decoded)?;
            if data.len() + raw.len() > MAX_PACKAGE - format::MAX_METADATA - format::MAX_DIRECTORY {
                return Err("The extracted tile package exceeds 128 MiB".into());
            }
            let offset = data.len() as u64;
            data.extend_from_slice(&raw);
            dedup.insert((entry.offset, entry.length), (offset, layers.clone()));
            (offset, layers)
        };
        entries.push(format::Entry {
            id,
            run: 1,
            length: raw.len() as u64,
            offset,
        });
        let b = tile_bounds(&c);
        coverage = [
            coverage[0].min(b[0]),
            coverage[1].min(b[1]),
            coverage[2].max(b[2]),
            coverage[3].max(b[3]),
        ];
        receipts.push(TileReceipt {
            coordinate: c,
            source_offset: Some(h.tile_offset + entry.offset),
            bytes: raw.len(),
            sha256: hash(&raw),
            package_offset: Some(offset),
            image: None,
            layers,
        });
    }
    let root = format::serialize(&entries)?;
    if root.len() > 16257 {
        return Err("Extracted PMTiles root directory exceeds the format limit".into());
    }
    let mut metadata = source.metadata.clone();
    metadata["bounds"] = serde_json::json!(coverage);
    metadata["minzoom"] = serde_json::json!(request.min_zoom);
    metadata["maxzoom"] = serde_json::json!(request.max_zoom);
    metadata["center"] = serde_json::json!([
        (request.bounds[0] + request.bounds[2]) / 2.,
        (request.bounds[1] + request.bounds[3]) / 2.,
        request.min_zoom
    ]);
    let metadata = format::gzip(&serde_json::to_vec(&metadata).map_err(io_error)?)?;
    let out_header = format::Header {
        root_offset: 127,
        root_length: root.len() as u64,
        metadata_offset: 127 + root.len() as u64,
        metadata_length: metadata.len() as u64,
        leaf_offset: 127 + root.len() as u64 + metadata.len() as u64,
        leaf_length: 0,
        tile_offset: 127 + root.len() as u64 + metadata.len() as u64,
        tile_length: data.len() as u64,
        addressed_tiles: receipts.len() as u64,
        tile_entries: entries.len() as u64,
        tile_contents: dedup.len() as u64,
        clustered: true,
        internal_compression: 2,
        tile_compression: h.tile_compression,
        min_zoom: request.min_zoom,
        max_zoom: request.max_zoom,
        bounds: coverage,
        center_zoom: request.min_zoom,
        center: [
            (request.bounds[0] + request.bounds[2]) / 2.,
            (request.bounds[1] + request.bounds[3]) / 2.,
        ],
    };
    let mut out = out_header.encode()?;
    out.extend(root);
    out.extend(metadata);
    out.extend(data);
    for t in &mut receipts {
        t.package_offset = t.package_offset.map(|n| n + out_header.tile_offset);
    }
    let p = Package {
        id: Uuid::new_v4().to_string(),
        name: source.name.clone(),
        requested_bounds: request.bounds,
        tile_coverage_bounds: coverage,
        min_zoom: request.min_zoom,
        max_zoom: request.max_zoom,
        bytes: out.len(),
        sha256: hash(&out),
        source,
        created_at: now(),
        ranges: reader.receipts(),
        tiles: receipts,
        absent,
        selection: "tile-aligned-pyramid".into(),
    };
    valid_package(&p)?;
    Ok((p, out))
}
fn read_range(file: &mut std::fs::File, offset: u64, length: usize) -> Result<Vec<u8>> {
    file.seek(SeekFrom::Start(offset)).map_err(io_error)?;
    let mut b = vec![0; length];
    file.read_exact(&mut b).map_err(io_error)?;
    Ok(b)
}
fn verified(root: &Path, p: &Package) -> Result<Vec<u8>> {
    valid_package(p)?;
    let f = std::fs::File::open(file(root, p)?).map_err(io_error)?;
    if f.metadata().map_err(io_error)?.len() != p.bytes as u64 {
        return Err("Local PMTiles package size changed".into());
    }
    let mut b = Vec::new();
    f.take(MAX_PACKAGE as u64 + 1)
        .read_to_end(&mut b)
        .map_err(io_error)?;
    if b.len() != p.bytes || hash(&b) != p.sha256 {
        return Err("Local PMTiles package checksum changed".into());
    }
    Ok(b)
}
impl JobManager {
    pub async fn list_tile_sources(&self) -> Vec<Source> {
        self.inner
            .tiles
            .lock()
            .await
            .sources
            .values()
            .cloned()
            .collect()
    }
    pub async fn list_tile_packages(&self) -> Vec<Package> {
        self.inner
            .tiles
            .lock()
            .await
            .packages
            .values()
            .cloned()
            .collect()
    }
    pub async fn connect_tiles(&self, r: ConnectRequest) -> Result<Source> {
        self.inner.store.lock().await.accepting_jobs()?;
        let settings = self.proxy_settings().await;
        let (mut s, _) = tokio::time::timeout(
            Duration::from_secs(60),
            discover(r.name.trim(), r.url.trim(), &settings),
        )
        .await
        .map_err(|_| "PMTiles discovery timed out")??;
        let mut guard = self.inner.tiles.lock().await;
        let mut next = guard.clone();
        if let Some(old) = next.sources.values().find(|v| v.url == s.url) {
            s.id = old.id.clone();
        } else if next.sources.len() >= 24 {
            return Err("Remove a tile connection before adding another".into());
        }
        next.sources.insert(s.id.clone(), s.clone());
        persist(&self.inner.root, &next)?;
        *guard = next;
        Ok(s)
    }
    pub async fn forget_tile_source(&self, id: &str) -> Result<()> {
        if !uuid(id) {
            return Err("Invalid tile source identifier".into());
        }
        self.inner.store.lock().await.accepting_jobs()?;
        let mut guard = self.inner.tiles.lock().await;
        let mut next = guard.clone();
        next.sources.remove(id);
        persist(&self.inner.root, &next)?;
        *guard = next;
        Ok(())
    }
    pub async fn extract_tiles(&self, r: ExtractRequest) -> Result<Package> {
        self.inner.store.lock().await.accepting_jobs()?;
        let _permit = self
            .inner
            .raster_permits
            .clone()
            .acquire_owned()
            .await
            .map_err(io_error)?;
        let source = self
            .inner
            .tiles
            .lock()
            .await
            .sources
            .get(&r.source_id)
            .cloned()
            .ok_or("Tile source was not found")?;
        let settings = self.proxy_settings().await;
        let (p, bytes) =
            tokio::time::timeout(Duration::from_secs(180), extract(source, r, &settings))
                .await
                .map_err(|_| {
                    "PMTiles extraction timed out; choose fewer levels or a smaller area"
                })??;
        self.save_tile_package(p, bytes).await
    }
    async fn save_tile_package(&self, p: Package, bytes: Vec<u8>) -> Result<Package> {
        valid_package(&p)?;
        if bytes.len() != p.bytes || hash(&bytes) != p.sha256 {
            return Err("Local PMTiles import receipt changed".into());
        }
        self.inner.store.lock().await.accepting_jobs()?;
        let mut guard = self.inner.tiles.lock().await;
        if let Some(local) = &p.source.local {
            if let Some(existing) = guard.packages.values().find(|old| {
                old.source
                    .local
                    .as_ref()
                    .is_some_and(|original| original.file_name == local.file_name)
                    && old.sha256 == p.sha256
                    && verified(&self.inner.root, old).is_ok()
            }) {
                return Ok(existing.clone());
            }
        }
        if guard.packages.len() >= 128 {
            return Err("The tile package registry is full".into());
        }
        let mut tmp =
            tempfile::NamedTempFile::new_in(directory(&self.inner.root)?).map_err(io_error)?;
        tmp.write_all(&bytes).map_err(io_error)?;
        tmp.as_file().sync_all().map_err(io_error)?;
        let dest = directory(&self.inner.root)?.join(format!("{}.{}", p.id, extension(&p)));
        tmp.persist_noclobber(&dest).map_err(io_error)?;
        let mut next = guard.clone();
        next.packages.insert(p.id.clone(), p.clone());
        if let Err(e) = persist(&self.inner.root, &next) {
            let _ = std::fs::remove_file(dest);
            return Err(e);
        }
        *guard = next;
        Ok(p)
    }
    async fn tile_package(&self, id: &str) -> Result<Package> {
        if !uuid(id) {
            return Err("Invalid tile package identifier".into());
        }
        self.inner
            .tiles
            .lock()
            .await
            .packages
            .get(id)
            .cloned()
            .ok_or("Tile package was not found".into())
    }
    pub async fn inspect_tile_package(&self, id: &str) -> Result<Inspection> {
        let p = self.tile_package(id).await?;
        let _permit = self
            .inner
            .raster_permits
            .clone()
            .acquire_owned()
            .await
            .map_err(io_error)?;
        let root = self.inner.root.clone();
        tokio::task::spawn_blocking(move || {
            let b = verified(&root, &p)?;
            if p.source.mbtiles.is_some() {
                mbtiles::verify(&p, &b)?;
                return Ok(Inspection {
                    metadata: p.source.metadata.clone(),
                    asset: p,
                });
            }
            let h = format::Header::parse(&b[..127], b.len() as u64)?;
            let metadata = format::metadata(
                &b[h.metadata_offset as usize..(h.metadata_offset + h.metadata_length) as usize],
                &h,
            )?;
            Ok(Inspection { asset: p, metadata })
        })
        .await
        .map_err(io_error)?
    }
    pub async fn read_tile(&self, r: TileRequest) -> Result<Tile> {
        let id = format::tile_id(r.z, r.x, r.y)?;
        let p = self.tile_package(&r.id).await?;
        let _permit = if p.source.mbtiles.is_some() {
            Some(
                self.inner
                    .raster_permits
                    .clone()
                    .acquire_owned()
                    .await
                    .map_err(io_error)?,
            )
        } else {
            None
        };
        let root = self.inner.root.clone();
        tokio::task::spawn_blocking(move || {
            valid_package(&p)?;
            let c = format::Coordinate {
                z: r.z,
                x: r.x,
                y: r.y,
            };
            let Some(t) = p.tiles.iter().find(|t| {
                format::tile_id(t.coordinate.z, t.coordinate.x, t.coordinate.y) == Ok(id)
            }) else {
                return Ok(Tile {
                    coordinate: c,
                    data_base64: None,
                    sha256: None,
                    layers: Vec::new(),
                    content_type: None,
                });
            };
            if p.source.mbtiles.is_some() {
                return mbtiles::read(&p, &verified(&root, &p)?, t);
            }
            let mut f = std::fs::File::open(file(&root, &p)?).map_err(io_error)?;
            if f.metadata().map_err(io_error)?.len() != p.bytes as u64 {
                return Err("Local PMTiles package size changed".into());
            }
            let raw = read_range(
                &mut f,
                t.package_offset.ok_or("Missing tile offset")?,
                t.bytes,
            )?;
            if hash(&raw) != t.sha256 {
                return Err("Local PMTiles tile checksum changed".into());
            }
            let decoded = format::decompress(
                &raw,
                p.source
                    .header
                    .as_ref()
                    .ok_or("Missing PMTiles header")?
                    .tile_compression,
                format::MAX_TILE,
            )?;
            let layers = mvt::inspect(&decoded)?;
            if layers != t.layers {
                return Err("PMTiles tile layer receipt changed".into());
            }
            Ok(Tile {
                coordinate: c,
                sha256: Some(hash(&decoded)),
                data_base64: Some(STANDARD.encode(decoded)),
                layers,
                content_type: None,
            })
        })
        .await
        .map_err(io_error)?
    }
    pub async fn tile_export(&self, id: &str) -> Result<Vec<u8>> {
        let p = self.tile_package(id).await?;
        let root = self.inner.root.clone();
        tokio::task::spawn_blocking(move||{
            let bytes=verified(&root,&p)?;let source=serde_json::to_vec_pretty(&p).map_err(io_error)?;let note=if p.source.mbtiles.is_some() { mbtiles::export_note().to_vec() } else if p.source.local.is_some() { b"GeoD Global local PMTiles import\n\nThe complete original archive is copied unchanged; source.json records the file name, SHA-256 and verified tile inventory. No original workstation path is added. Original source metadata is retained, including any producer-declared text. MVT geometry remains quantized, potentially simplified, buffered and repeated across tiles or levels; this is not a full-resolution vector dataset or a polygon clip. GeoD uses a local preview style and does not include source styles, sprites or fonts. Retain attribution and check the dataset reuse terms. PMTiles is a container format and does not grant data rights.\n".to_vec() } else { b"GeoD Global tile-aligned PMTiles extraction\n\nMVT tiles retain exact source bytes, quantized geometry and their original attributes. They may be simplified, clipped at tile edges, repeated across zooms, and buffered outside the requested bounds. This is not an exact polygon clip or a full-resolution vector dataset. No source style, sprite, font or external code was downloaded. GeoD uses a local preview style. Retain source attribution and check the source dataset's reuse terms. PMTiles is a container format and does not grant data rights. No absolute local paths are included.\n".to_vec() };let files=[(if p.source.mbtiles.is_some() {"tiles.mbtiles"} else {"tiles.pmtiles"},bytes),("source.json",source),("README.txt",note)];let sums=files.iter().map(|(name,b)|format!("{}  {name}\n",hash(b))).collect::<String>();let mut zip=zip::ZipWriter::new(Cursor::new(Vec::new()));let options=zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);for(name,b)in files {zip.start_file(name,options).map_err(io_error)?;zip.write_all(&b).map_err(io_error)?;}zip.start_file("checksums.sha256",options).map_err(io_error)?;zip.write_all(sums.as_bytes()).map_err(io_error)?;Ok(zip.finish().map_err(io_error)?.into_inner())
        }).await.map_err(io_error)?
    }
    pub async fn export_tile_package_path(&self, id: &str, path: PathBuf) -> Result<()> {
        let b = self.tile_export(id).await?;
        let parent = path
            .parent()
            .filter(|p| p.is_dir())
            .ok_or("Choose an existing export directory")?;
        let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(io_error)?;
        tmp.write_all(&b).map_err(io_error)?;
        tmp.as_file().sync_all().map_err(io_error)?;
        tmp.persist_noclobber(path)
            .map_err(|_| "Export destination already exists or cannot be written")?;
        Ok(())
    }
}
#[cfg(test)]
mod tests;
