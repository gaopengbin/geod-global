//! Public WMS rendered map snapshots. PNG pixels are not scientific source bands.
use crate::{features, io_error, now, storage, JobManager, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use futures_util::StreamExt;
use roxmltree::Node;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use url::Url;
use uuid::Uuid;
pub mod arcgis;
#[cfg(test)]
mod tests;
pub mod wmts;
pub mod xyz;

const MAX_XML: usize = 8 * 1024 * 1024;
const MAX_PNG: usize = 16 * 1024 * 1024;
const MAX_REGISTRY: usize = 32 * 1024 * 1024;
const MAX_EDGE: u32 = 2048;
const SERVICES: &str = "map-services.json";
const IMAGES: &str = "map-images.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimeDimension {
    pub values: String,
    pub default: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MapLayer {
    pub name: String,
    pub title: String,
    pub description: String,
    pub crs: String,
    pub styles: Vec<String>,
    pub time: Option<TimeDimension>,
    pub bounds: Option<[f64; 4]>,
    pub attribution: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wmts: Option<wmts::Layer>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MapService {
    pub id: String,
    pub name: String,
    pub url: String,
    pub title: String,
    pub version: String,
    pub map_url: String,
    pub layers: Vec<MapLayer>,
    pub access_constraints: String,
    pub max_width: u32,
    pub max_height: u32,
    pub capabilities_sha256: String,
    pub connected_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wmts: Option<wmts::Capabilities>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arcgis: Option<arcgis::Capabilities>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xyz: Option<xyz::Configuration>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectRequest {
    pub name: String,
    pub url: String,
    #[serde(default = "default_protocol")]
    pub protocol: String,
    #[serde(default)]
    pub tile_config: Option<xyz::GridOptions>,
    #[serde(default)]
    pub wmts_document: bool,
}
fn default_protocol() -> String {
    "WMS".into()
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MapRequest {
    pub service_id: String,
    pub layer_name: String,
    pub style: String,
    pub time: Option<String>,
    pub bounds: [f64; 4],
    pub width: u32,
    pub height: u32,
    pub area_geometry: Option<crate::crop::PolygonGeometry>,
    pub tile_matrix_set: Option<String>,
    pub tile_matrix: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MapSource {
    pub service_url: String,
    pub service_name: String,
    pub service_title: String,
    pub version: String,
    pub map_endpoint: String,
    pub capabilities_sha256: String,
    pub layer_name: String,
    pub layer_title: String,
    pub style: String,
    pub time: Option<String>,
    pub request_crs: String,
    pub request_url: String,
    pub requested_at: String,
    pub access_constraints: String,
    pub attribution: Option<String>,
    pub area_geometry: Option<crate::crop::PolygonGeometry>,
    pub selection: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wmts: Option<wmts::Snapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arcgis: Option<arcgis::Snapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xyz: Option<xyz::Snapshot>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MapImage {
    pub id: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub bounds: [f64; 4],
    pub bytes: usize,
    pub sha256: String,
    pub crs: String,
    pub source: MapSource,
    /// Native CRS pixel-corner bounds. Older WMS records use `bounds`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_extent: Option<[f64; 4]>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapInspection {
    pub asset: MapImage,
    pub image_url: String,
}
fn hash(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
fn text_valid(s: &str, max: usize) -> bool {
    !s.trim().is_empty()
        && s.trim() == s
        && s.chars().count() <= max
        && !s.chars().any(char::is_control)
}
fn digest_valid(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}
fn uuid_valid(s: &str) -> bool {
    Uuid::parse_str(s).ok().map(|u| u.to_string()).as_deref() == Some(s)
}
fn child<'a, 'input>(n: Node<'a, 'input>, tag: &str) -> Option<Node<'a, 'input>> {
    n.children().find(|c| {
        c.is_element()
            && c.tag_name().name() == tag
            && c.tag_name().namespace() == n.tag_name().namespace()
    })
}
fn children<'a, 'input>(n: Node<'a, 'input>, tag: &str) -> impl Iterator<Item = Node<'a, 'input>> {
    let tag = tag.to_string();
    n.children().filter(move |c| {
        c.is_element()
            && c.tag_name().name() == tag
            && c.tag_name().namespace() == n.tag_name().namespace()
    })
}
fn text(n: Node<'_, '_>, tag: &str, max: usize) -> Result<String> {
    let s = child(n, tag).and_then(|n| n.text()).unwrap_or("").trim();
    if s.len() > max {
        return Err("WMS metadata field is too large".into());
    }
    Ok(s.into())
}
fn endpoint(root: &Url, raw: &str) -> Result<Url> {
    let u = features::public_url(root.join(raw).map_err(io_error)?.as_str())?;
    let parent = root
        .path()
        .rsplit_once('/')
        .map(|(p, _)| format!("{p}/"))
        .unwrap_or_else(|| "/".into());
    if u.origin() != root.origin() || !u.path().starts_with(&parent) {
        return Err("WMS request links must stay within the connected service directory".into());
    }
    if u.query_pairs().any(|(k, _)| {
        !matches!(
            k.to_ascii_lowercase().as_str(),
            "service" | "version" | "request"
        )
    }) {
        return Err("WMS service URLs cannot contain credentials or custom parameters".into());
    }
    let mut u = u;
    u.set_query(None);
    Ok(u)
}
fn service_url(raw: &str) -> Result<Url> {
    let u = features::public_url(raw)?;
    if u.query().is_some() {
        return Err("Enter the WMS endpoint without query parameters".into());
    }
    Ok(u)
}
#[derive(Clone, Default)]
struct Inherited {
    crs: BTreeSet<String>,
    styles: BTreeSet<String>,
    time: Option<TimeDimension>,
    bounds: Option<[f64; 4]>,
    attribution: Option<String>,
    unsupported_dimension: bool,
    restricted: bool,
}
fn layers(
    n: Node<'_, '_>,
    inherited: &Inherited,
    out: &mut Vec<MapLayer>,
    depth: usize,
) -> Result<()> {
    if depth > 32 || out.len() > 4096 {
        return Err("WMS layer tree exceeds the connection limit".into());
    }
    let mut v = inherited.clone();
    for c in children(n, "CRS").chain(children(n, "SRS")) {
        for s in c.text().unwrap_or("").split_whitespace() {
            v.crs.insert(s.into());
        }
    }
    for s in children(n, "Style") {
        let name = text(s, "Name", 256)?;
        if !name.is_empty() {
            v.styles.insert(name);
        }
    }
    for d in children(n, "Dimension").chain(children(n, "Extent")) {
        if d.attribute("name")
            .is_some_and(|s| s.eq_ignore_ascii_case("time"))
        {
            let values = d.text().unwrap_or("").trim();
            if values.len() > 65536 {
                return Err("WMS time metadata is too large".into());
            }
            if !values.is_empty() {
                v.time = Some(TimeDimension {
                    values: values.into(),
                    default: d.attribute("default").map(str::to_owned),
                });
            }
        } else {
            v.unsupported_dimension = true;
        }
    }
    v.restricted |= n.attribute("noSubsets") == Some("1")
        || n.attribute("fixedWidth").is_some_and(|s| s != "0")
        || n.attribute("fixedHeight").is_some_and(|s| s != "0");
    if let Some(b) = child(n, "EX_GeographicBoundingBox") {
        let values = [
            "westBoundLongitude",
            "southBoundLatitude",
            "eastBoundLongitude",
            "northBoundLatitude",
        ]
        .map(|t| text(b, t, 64)?.parse::<f64>().map_err(io_error));
        let mut parsed = [0.; 4];
        for (i, x) in values.into_iter().enumerate() {
            parsed[i] = x?;
        }
        features::bounds(parsed)?;
        v.bounds = Some(parsed);
    } else if let Some(b) = child(n, "LatLonBoundingBox") {
        let mut parsed = [0.; 4];
        for (i, k) in ["minx", "miny", "maxx", "maxy"].iter().enumerate() {
            parsed[i] = b
                .attribute(*k)
                .ok_or("Invalid WMS geographic bounds")?
                .parse()
                .map_err(io_error)?;
        }
        features::bounds(parsed)?;
        v.bounds = Some(parsed);
    }
    if let Some(a) = child(n, "Attribution") {
        let s = text(a, "Title", 1024)?;
        if !s.is_empty() {
            v.attribution = Some(s);
        }
    }
    let name = text(n, "Name", 256)?;
    let crs = if v.crs.contains("CRS:84") {
        Some("CRS:84")
    } else if v.crs.contains("EPSG:4326") {
        Some("EPSG:4326")
    } else {
        None
    };
    if !name.is_empty() && !v.unsupported_dimension && !v.restricted {
        if let Some(crs) = crs {
            let title = text(n, "Title", 1024)?;
            out.push(MapLayer {
                wmts: None,
                name: name.clone(),
                title: if title.is_empty() { name } else { title },
                description: text(n, "Abstract", 16384)?,
                crs: crs.into(),
                styles: v.styles.iter().cloned().collect(),
                time: v.time.clone(),
                bounds: v.bounds,
                attribution: v.attribution.clone(),
            });
        }
    }
    for c in children(n, "Layer") {
        layers(c, &v, out, depth + 1)?;
    }
    Ok(())
}
fn parse_capabilities(bytes: &[u8], root: &Url, name: &str) -> Result<MapService> {
    let xml = std::str::from_utf8(bytes).map_err(|_| "WMS capabilities must use UTF-8")?;
    // External DTD identifiers are accepted for WMS 1.1.1, but never fetched.
    // Entity declarations are rejected, including local entity expansion.
    if xml.contains("<!ENTITY") {
        return Err("WMS XML entity declarations are not supported".into());
    }
    let doc = roxmltree::Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            nodes_limit: 200_000,
            ..Default::default()
        },
    )
    .map_err(|_| "Invalid WMS capabilities XML")?;
    let n = doc.root_element();
    let version = n.attribute("version").ok_or("WMS version is missing")?;
    if !matches!(
        (n.tag_name().name(), n.tag_name().namespace(), version),
        (
            "WMS_Capabilities",
            Some("http://www.opengis.net/wms"),
            "1.3.0"
        ) | ("WMT_MS_Capabilities", None, "1.1.1")
    ) {
        return Err("Only WMS 1.3.0 and 1.1.1 capabilities are supported".into());
    }
    let service = child(n, "Service").ok_or("WMS service metadata is missing")?;
    let cap = child(n, "Capability").ok_or("WMS capabilities are missing")?;
    let request = child(cap, "Request")
        .and_then(|r| child(r, "GetMap"))
        .ok_or("WMS GetMap is not advertised")?;
    if !children(request, "Format").any(|n| n.text() == Some("image/png")) {
        return Err("WMS service must advertise PNG map images".into());
    }
    let raw = children(request, "DCPType")
        .filter_map(|d| child(d, "HTTP"))
        .filter_map(|d| child(d, "Get"))
        .filter_map(|d| child(d, "OnlineResource"))
        .find_map(|r| r.attribute(("http://www.w3.org/1999/xlink", "href")))
        .ok_or("WMS has no HTTP GET map endpoint")?;
    let map_url = endpoint(root, raw)?;
    let mut list = Vec::new();
    for l in children(cap, "Layer") {
        layers(l, &Inherited::default(), &mut list, 0)?;
    }
    let mut names = BTreeSet::new();
    if list.is_empty() || list.len() > 4096 || list.iter().any(|l| !names.insert(&l.name)) {
        return Err("WMS has no compatible unique WGS84 layers".into());
    }
    let size = |tag| -> Result<u32> {
        let v = text(service, tag, 16)?;
        if v.is_empty() {
            Ok(MAX_EDGE)
        } else {
            let n = v.parse::<u32>().map_err(io_error)?;
            if n == 0 {
                return Err("Invalid WMS image dimension limit".into());
            }
            Ok(n.min(MAX_EDGE))
        }
    };
    let title = text(service, "Title", 1024)?;
    let out = MapService {
        xyz: None,
        arcgis: None,
        wmts: None,
        id: Uuid::new_v4().to_string(),
        name: name.into(),
        url: root.to_string(),
        title: if title.is_empty() { name.into() } else { title },
        version: version.into(),
        map_url: map_url.to_string(),
        layers: list,
        access_constraints: text(service, "AccessConstraints", 16384)?,
        max_width: size("MaxWidth")?,
        max_height: size("MaxHeight")?,
        capabilities_sha256: hash(bytes),
        connected_at: now(),
    };
    validate_service(&out)?;
    Ok(out)
}
fn instant(raw: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .ok()
        .filter(|d| d.to_string() == raw)
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|d| d.and_utc())
        .or_else(|| {
            chrono::DateTime::parse_from_rfc3339(raw)
                .ok()
                .map(|d| d.with_timezone(&chrono::Utc))
        })
}
fn time_supported(dim: &TimeDimension, raw: &str) -> bool {
    let Some(value) = instant(raw) else {
        return false;
    };
    dim.values.split(',').any(|entry| {
        let p: Vec<_> = entry.trim().split('/').collect();
        if p.len() == 1 {
            return instant(p[0]) == Some(value);
        }
        if p.len() != 3 {
            return false;
        }
        let (Some(start), Some(end)) = (instant(p[0]), instant(p[1])) else {
            return false;
        };
        if value < start || value > end {
            return false;
        }
        let period = p[2];
        let timed = period.starts_with("PT");
        let prefix = if timed { 2 } else { 1 };
        if !period.is_ascii() || !period.starts_with('P') || period.len() <= prefix + 1 {
            return false;
        }
        if !period.as_bytes()[prefix..period.len() - 1]
            .iter()
            .all(u8::is_ascii_digit)
        {
            return false;
        }
        let Ok(n) = period[prefix..period.len() - 1].parse::<i64>() else {
            return false;
        };
        if n <= 0 {
            return false;
        }
        let unit = period.as_bytes()[period.len() - 1];
        let seconds = match (timed, unit) {
            (false, b'D') => n.checked_mul(86400),
            (false, b'W') => n.checked_mul(604800),
            (true, b'H') => n.checked_mul(3600),
            (true, b'M') => n.checked_mul(60),
            (true, b'S') => Some(n),
            _ => None,
        };
        if let Some(seconds) = seconds {
            return (value - start).num_seconds() % seconds == 0
                && value.timestamp_subsec_nanos() == start.timestamp_subsec_nanos();
        }
        if !timed && matches!(unit, b'M' | b'Y') {
            use chrono::Datelike;
            let months = i64::from(value.year() - start.year()) * 12 + i64::from(value.month())
                - i64::from(start.month());
            let Some(step) = (if unit == b'Y' {
                n.checked_mul(12)
            } else {
                Some(n)
            }) else {
                return false;
            };
            return months >= 0
                && months % step == 0
                && u32::try_from(months)
                    .ok()
                    .and_then(|m| start.checked_add_months(chrono::Months::new(m)))
                    == Some(value);
        }
        false
    })
}
#[allow(clippy::too_many_arguments)] // Explicit WMS request dimensions, serialized into one URL.
fn map_url(
    endpoint: &str,
    version: &str,
    crs: &str,
    layer: &str,
    style: &str,
    time: Option<&str>,
    b: [f64; 4],
    size: [u32; 2],
) -> Result<Url> {
    let mut u = features::public_url(endpoint)?;
    let axis = if version == "1.3.0" && crs == "EPSG:4326" {
        [b[1], b[0], b[3], b[2]]
    } else {
        b
    };
    let bbox = axis
        .iter()
        .map(f64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    u.query_pairs_mut()
        .clear()
        .append_pair("SERVICE", "WMS")
        .append_pair("VERSION", version)
        .append_pair("REQUEST", "GetMap")
        .append_pair("LAYERS", layer)
        .append_pair("STYLES", style)
        .append_pair(if version == "1.3.0" { "CRS" } else { "SRS" }, crs)
        .append_pair("BBOX", &bbox)
        .append_pair("WIDTH", &size[0].to_string())
        .append_pair("HEIGHT", &size[1].to_string())
        .append_pair("FORMAT", "image/png")
        .append_pair("TRANSPARENT", "TRUE");
    if let Some(t) = time {
        u.query_pairs_mut().append_pair("TIME", t);
    }
    Ok(u)
}
async fn fetch(c: &reqwest::Client, u: &Url, max: usize, png: bool) -> Result<Vec<u8>> {
    let r = c
        .get(u.clone())
        .header(
            "Accept",
            if png {
                "image/png"
            } else {
                "application/xml,text/xml"
            },
        )
        .send()
        .await
        .map_err(|_| "Cannot reach WMS service; check the service and proxy settings")?;
    if !r.status().is_success() {
        return Err(format!("WMS service returned HTTP {}", r.status().as_u16()));
    }
    let media = r
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if if png {
        media != "image/png"
    } else {
        !matches!(
            media.as_str(),
            "application/xml" | "text/xml" | "application/vnd.ogc.wms_xml"
        )
    } {
        return Err("WMS returned an unexpected response type or service exception".into());
    }
    if r.content_length().is_some_and(|n| n > max as u64) {
        return Err("WMS response exceeds the size limit".into());
    }
    let mut stream = r.bytes_stream();
    let mut out = Vec::new();
    while let Some(chunk) = stream.next().await {
        let b = chunk.map_err(|_| "WMS response was interrupted")?;
        if b.len() > max - out.len() {
            return Err("WMS response exceeds the size limit".into());
        }
        out.extend_from_slice(&b);
    }
    Ok(out)
}
fn validate_png(b: &[u8], w: u32, h: u32) -> Result<()> {
    if b.len() > MAX_PNG || w == 0 || h == 0 || w > MAX_EDGE || h > MAX_EDGE {
        return Err("Map image exceeds 2048 pixels per edge or 16 MiB".into());
    }
    let mut decoder = png::Decoder::new(Cursor::new(b));
    decoder.set_limits(png::Limits {
        bytes: 32 * 1024 * 1024,
    });
    let mut reader = decoder
        .read_info()
        .map_err(|_| "WMS did not return a valid PNG image")?;
    if reader.info().width != w
        || reader.info().height != h
        || reader.info().animation_control.is_some()
    {
        return Err("WMS returned different image dimensions or an animated image".into());
    }
    let len = reader
        .output_buffer_size()
        .filter(|n| *n <= 32 * 1024 * 1024)
        .ok_or("Map decoding exceeds the memory limit")?;
    reader
        .next_frame(&mut vec![0; len])
        .map_err(|_| "WMS PNG pixels are incomplete or corrupt")?;
    reader
        .finish()
        .map_err(|_| "WMS PNG file is incomplete or corrupt")?;
    Ok(())
}
fn validate_service(s: &MapService) -> Result<()> {
    if [s.xyz.is_some(), s.arcgis.is_some(), s.wmts.is_some()]
        .iter()
        .filter(|v| **v)
        .count()
        > 1
    {
        return Err("Saved map service contains conflicting protocols".into());
    }
    if s.xyz.is_some() {
        return xyz::validate_service(s);
    }
    if s.arcgis.is_some() {
        return arcgis::validate_service(s);
    }
    if s.wmts.is_some() {
        return wmts::validate_service(s);
    }
    let root = service_url(&s.url)?;
    endpoint(&root, &s.map_url)?;
    if !uuid_valid(&s.id)
        || !text_valid(&s.name, 80)
        || !text_valid(&s.title, 1024)
        || !matches!(s.version.as_str(), "1.3.0" | "1.1.1")
        || s.layers.is_empty()
        || s.layers.len() > 4096
        || s.max_width == 0
        || s.max_height == 0
        || s.max_width > MAX_EDGE
        || s.max_height > MAX_EDGE
        || !digest_valid(&s.capabilities_sha256)
        || instant(&s.connected_at).is_none()
        || s.access_constraints.len() > 16384
    {
        return Err("Invalid saved WMS service".into());
    }
    let mut names = BTreeSet::new();
    for l in &s.layers {
        if !names.insert(&l.name)
            || !text_valid(&l.name, 256)
            || !text_valid(&l.title, 1024)
            || l.description.len() > 16384
            || !matches!(l.crs.as_str(), "CRS:84" | "EPSG:4326")
            || l.styles.len() > 256
            || l.styles.iter().any(|s| !text_valid(s, 256))
            || l.time.as_ref().is_some_and(|t| {
                t.values.is_empty()
                    || t.values.len() > 65536
                    || t.default.as_ref().is_some_and(|d| d.len() > 128)
            })
            || l.attribution.as_ref().is_some_and(|s| !text_valid(s, 1024))
        {
            return Err("Invalid saved WMS layer".into());
        }
        if let Some(b) = l.bounds {
            features::bounds(b)?;
        }
    }
    Ok(())
}
fn validate_asset(a: &MapImage) -> Result<()> {
    if [
        a.source.xyz.is_some(),
        a.source.arcgis.is_some(),
        a.source.wmts.is_some(),
    ]
    .iter()
    .filter(|v| **v)
    .count()
        > 1
    {
        return Err("Saved map image contains conflicting protocols".into());
    }
    if a.source.xyz.is_some() {
        return xyz::validate_asset(a);
    }
    if a.source.arcgis.is_some() {
        return arcgis::validate_asset(a);
    }
    if a.source.wmts.is_some() {
        return wmts::validate_asset(a);
    }
    if a.image_extent.is_some() {
        return Err("Unexpected WMS image extent".into());
    }
    features::bounds(a.bounds)?;
    let s = &a.source;
    let root = service_url(&s.service_url)?;
    endpoint(&root, &s.map_endpoint)?;
    if !uuid_valid(&a.id)
        || !text_valid(&a.name, 120)
        || a.width == 0
        || a.height == 0
        || a.width > MAX_EDGE
        || a.height > MAX_EDGE
        || a.bytes == 0
        || a.bytes > MAX_PNG
        || !digest_valid(&a.sha256)
        || a.crs != "EPSG:4326"
        || !matches!(s.version.as_str(), "1.3.0" | "1.1.1")
        || !matches!(s.request_crs.as_str(), "EPSG:4326" | "CRS:84")
        || !digest_valid(&s.capabilities_sha256)
        || !text_valid(&s.service_name, 80)
        || !text_valid(&s.service_title, 1024)
        || !text_valid(&s.layer_name, 256)
        || !text_valid(&s.layer_title, 1024)
        || s.style.len() > 256
        || s.time.as_ref().is_some_and(|t| instant(t).is_none())
        || instant(&s.requested_at).is_none()
        || s.selection != "bbox-rendered-map"
        || s.access_constraints.len() > 16384
        || s.attribution.as_ref().is_some_and(|t| !text_valid(t, 1024))
    {
        return Err("Invalid saved rendered map metadata".into());
    }
    if s.request_url
        != map_url(
            &s.map_endpoint,
            &s.version,
            &s.request_crs,
            &s.layer_name,
            &s.style,
            s.time.as_deref(),
            a.bounds,
            [a.width, a.height],
        )?
        .to_string()
    {
        return Err("Map request does not match the saved image grid".into());
    }
    if let Some(area) = &s.area_geometry {
        let b = area.bounds()?;
        if b[0] < a.bounds[0] || b[1] < a.bounds[1] || b[2] > a.bounds[2] || b[3] > a.bounds[3] {
            return Err("Map bounds must include the selected polygon".into());
        }
    }
    Ok(())
}
fn directory(root: &Path) -> Result<PathBuf> {
    let d = root.join("map-images");
    if d.canonicalize().map_err(io_error)? != d {
        return Err("Map image storage was redirected".into());
    }
    Ok(d)
}
fn read_file(path: &Path, max: usize) -> Result<Vec<u8>> {
    let path = storage::regular_file(path)?;
    let file = std::fs::File::open(path).map_err(io_error)?;
    if file.metadata().map_err(io_error)?.len() > max as u64 {
        return Err("Map file exceeds the size limit".into());
    }
    let mut b = Vec::new();
    file.take(max as u64 + 1)
        .read_to_end(&mut b)
        .map_err(io_error)?;
    if b.len() > max {
        return Err("Map file exceeds the size limit".into());
    }
    Ok(b)
}
fn persist<T: Serialize>(root: &Path, name: &str, data: &T) -> Result<()> {
    let b = serde_json::to_vec_pretty(data).map_err(io_error)?;
    if b.len() > MAX_REGISTRY {
        return Err("WMS registry exceeds the size limit".into());
    }
    let mut tmp = tempfile::NamedTempFile::new_in(root).map_err(io_error)?;
    tmp.write_all(&b).map_err(io_error)?;
    tmp.as_file().sync_all().map_err(io_error)?;
    tmp.persist(root.join(name)).map_err(io_error)?;
    Ok(())
}
pub(crate) async fn load(
    root: &Path,
) -> Result<(BTreeMap<String, MapService>, BTreeMap<String, MapImage>)> {
    tokio::fs::create_dir_all(root.join("map-images"))
        .await
        .map_err(io_error)?;
    directory(root)?;
    fn registry<T: serde::de::DeserializeOwned>(
        root: &Path,
        name: &str,
    ) -> Result<BTreeMap<String, T>> {
        let p = root.join(name);
        if !p.try_exists().map_err(io_error)? {
            return Ok(BTreeMap::new());
        }
        serde_json::from_slice(&read_file(&p, MAX_REGISTRY)?).map_err(io_error)
    }
    let services: BTreeMap<String, MapService> = registry(root, SERVICES)?;
    let images: BTreeMap<String, MapImage> = registry(root, IMAGES)?;
    if services.len() > 24 || images.len() > 512 {
        return Err("WMS registry exceeds the record limit".into());
    }
    for (id, s) in &services {
        validate_service(s)?;
        if id != &s.id {
            return Err("WMS service identifier changed".into());
        }
    }
    for (id, a) in &images {
        validate_asset(a)?;
        if id != &a.id {
            return Err("Map image identifier changed".into());
        }
    }
    Ok((services, images))
}
impl JobManager {
    pub async fn list_map_services(&self) -> Vec<MapService> {
        self.inner
            .map_services
            .lock()
            .await
            .values()
            .cloned()
            .collect()
    }
    pub async fn list_map_images(&self) -> Vec<MapImage> {
        self.inner
            .map_images
            .lock()
            .await
            .values()
            .cloned()
            .collect()
    }
    pub async fn connect_map_service(&self, r: ConnectRequest) -> Result<MapService> {
        self.inner.store.lock().await.accepting_jobs()?;
        let name = r.name.trim();
        if !text_valid(name, 80) {
            return Err("Enter a WMS service name of 1–80 characters".into());
        }
        if r.wmts_document && r.protocol != "WMTS" {
            return Err("A capabilities document requires a WMTS connection".into());
        }
        if matches!(r.protocol.as_str(), "XYZ" | "TMS") {
            let service = xyz::connect(
                name,
                r.url.trim(),
                &r.protocol,
                r.tile_config.ok_or("Configure the XYZ/TMS tile grid")?,
            )?;
            return self.register_map_service(service).await;
        }
        if r.tile_config.is_some() {
            return Err("Tile grid settings require an XYZ or TMS connection".into());
        }
        let root = service_url(r.url.trim())?;
        if !matches!(r.protocol.as_str(), "WMS" | "WMTS" | "ArcGIS") {
            return Err("Choose WMS, WMTS or ArcGIS".into());
        }
        let is_wmts = r.protocol == "WMTS";
        let settings = self.proxy_settings().await;
        let service = if r.protocol == "ArcGIS" {
            arcgis::connect(&root, name, &settings).await?
        } else {
            let bytes = tokio::time::timeout(Duration::from_secs(60), async {
                let c = features::client(&root, &settings).await?;
                let mut u = root.clone();
                if !r.wmts_document {
                    u.query_pairs_mut()
                        .append_pair("SERVICE", if is_wmts { "WMTS" } else { "WMS" })
                        .append_pair("REQUEST", "GetCapabilities")
                        .append_pair("VERSION", if is_wmts { "1.0.0" } else { "1.3.0" });
                }
                fetch(&c, &u, MAX_XML, false).await
            })
            .await
            .map_err(|_| "WMS service discovery timed out")??;
            let root_clone = root.clone();
            let name = name.to_string();
            let service = tokio::task::spawn_blocking(move || {
                if is_wmts {
                    let mut service = wmts::parse_capabilities(&bytes, &root_clone, &name)?;
                    service.wmts.as_mut().unwrap().capabilities_document = r.wmts_document;
                    Ok(service)
                } else {
                    parse_capabilities(&bytes, &root_clone, &name)
                }
            })
            .await
            .map_err(io_error)??;
            service
        };
        self.register_map_service(service).await
    }
    async fn register_map_service(&self, mut service: MapService) -> Result<MapService> {
        let mut records = self.inner.map_services.lock().await;
        let old = records
            .values()
            .find(|s| s.url == service.url && s.xyz == service.xyz)
            .cloned();
        if let Some(old) = &old {
            service.id = old.id.clone();
        } else if records.len() >= 24 {
            return Err("WMS service registry is full (24 connections)".into());
        }
        records.insert(service.id.clone(), service.clone());
        if let Err(e) = persist(&self.inner.root, SERVICES, &*records) {
            records.remove(&service.id);
            if let Some(old) = old {
                records.insert(old.id.clone(), old);
            }
            return Err(e);
        }
        Ok(service)
    }
    pub async fn forget_map_service(&self, id: &str) -> Result<()> {
        self.inner.store.lock().await.accepting_jobs()?;
        let mut records = self.inner.map_services.lock().await;
        let old = records.remove(id).ok_or("Unknown WMS service")?;
        if let Err(e) = persist(&self.inner.root, SERVICES, &*records) {
            records.insert(id.into(), old);
            return Err(e);
        }
        Ok(())
    }
    pub async fn get_map_image(&self, r: MapRequest) -> Result<MapImage> {
        self.inner.store.lock().await.accepting_jobs()?;
        features::bounds(r.bounds)?;
        let service = self
            .inner
            .map_services
            .lock()
            .await
            .get(&r.service_id)
            .cloned()
            .ok_or("Unknown WMS service")?;
        if service.xyz.is_some() {
            return xyz::get(self, r, service).await;
        }
        if service.arcgis.is_some() {
            return arcgis::get(self, r, service).await;
        }
        if service.wmts.is_some() {
            return wmts::get(self, r, service).await;
        }
        if r.tile_matrix.is_some() || r.tile_matrix_set.is_some() {
            return Err("WMS requests cannot use a tile matrix".into());
        }
        let layer = service
            .layers
            .iter()
            .find(|l| l.name == r.layer_name)
            .ok_or("Unknown WMS layer")?;
        if r.width == 0
            || r.height == 0
            || r.width > service.max_width
            || r.height > service.max_height
        {
            return Err(
                "Choose image dimensions within the advertised limit, up to 2048 per edge".into(),
            );
        }
        if !r.style.is_empty() && !layer.styles.contains(&r.style) {
            return Err("Choose an advertised WMS style".into());
        }
        match (&layer.time, &r.time) {
            (Some(d), Some(t)) if time_supported(d, t) => {}
            (None, None) => {}
            _ => return Err("Choose an explicit time supported by the WMS layer".into()),
        }
        let u = map_url(
            &service.map_url,
            &service.version,
            &layer.crs,
            &layer.name,
            &r.style,
            r.time.as_deref(),
            r.bounds,
            [r.width, r.height],
        )?;
        let source = MapSource {
            xyz: None,
            arcgis: None,
            wmts: None,
            service_url: service.url.clone(),
            service_name: service.name.clone(),
            service_title: service.title.clone(),
            version: service.version.clone(),
            map_endpoint: service.map_url.clone(),
            capabilities_sha256: service.capabilities_sha256.clone(),
            layer_name: layer.name.clone(),
            layer_title: layer.title.clone(),
            style: r.style,
            time: r.time,
            request_crs: layer.crs.clone(),
            request_url: u.to_string(),
            requested_at: now(),
            access_constraints: service.access_constraints.clone(),
            attribution: layer.attribution.clone(),
            area_geometry: r.area_geometry,
            selection: "bbox-rendered-map".into(),
        };
        let mut a = MapImage {
            image_extent: None,
            id: Uuid::new_v4().to_string(),
            name: format!(
                "{} · WMS",
                layer.title.chars().take(112).collect::<String>()
            ),
            width: r.width,
            height: r.height,
            bounds: r.bounds,
            bytes: 1,
            sha256: "0".repeat(64),
            crs: "EPSG:4326".into(),
            source,
        };
        validate_asset(&a)?;
        let _permit = self
            .inner
            .thumbnail_permits
            .acquire()
            .await
            .map_err(io_error)?;
        let settings = self.proxy_settings().await;
        let b = tokio::time::timeout(Duration::from_secs(60), async {
            let c = features::client(&service_url(&service.url)?, &settings).await?;
            fetch(&c, &u, MAX_PNG, true).await
        })
        .await
        .map_err(|_| "WMS map request timed out; no image was registered")??;
        let (a, b) = tokio::task::spawn_blocking(move || {
            validate_png(&b, a.width, a.height)?;
            a.bytes = b.len();
            a.sha256 = hash(&b);
            Ok::<_, String>((a, b))
        })
        .await
        .map_err(io_error)??;
        // Finish persistence outside the network deadline, with rollback on failure.
        self.save_map_image(a, b, None).await
    }
    async fn save_map_image(
        &self,
        a: MapImage,
        b: Vec<u8>,
        tiles: Option<Vec<u8>>,
    ) -> Result<MapImage> {
        validate_asset(&a)?;
        let mut records = self.inner.map_images.lock().await;
        if records.len() >= 512 {
            return Err("Map image registry is full (512 images)".into());
        }
        let d = directory(&self.inner.root)?;
        let p = d.join(format!("{}.png", a.id));
        let mut tmp = tempfile::NamedTempFile::new_in(d).map_err(io_error)?;
        tmp.write_all(&b).map_err(io_error)?;
        tmp.as_file().sync_all().map_err(io_error)?;
        tmp.persist_noclobber(&p).map_err(io_error)?;
        let tile_path = p.with_extension("tiles.zip");
        if let Some(tiles) = &tiles {
            let saved = (|| {
                let mut tmp =
                    tempfile::NamedTempFile::new_in(p.parent().unwrap()).map_err(io_error)?;
                tmp.write_all(tiles).map_err(io_error)?;
                tmp.as_file().sync_all().map_err(io_error)?;
                tmp.persist_noclobber(&tile_path).map_err(io_error)?;
                Ok::<_, String>(())
            })();
            if let Err(e) = saved {
                let _ = std::fs::remove_file(&p);
                return Err(e);
            }
        }
        records.insert(a.id.clone(), a.clone());
        if let Err(e) = persist(&self.inner.root, IMAGES, &*records) {
            records.remove(&a.id);
            let _ = std::fs::remove_file(p);
            let _ = std::fs::remove_file(tile_path);
            return Err(e);
        }
        Ok(a)
    }
    async fn map_bytes(&self, id: &str) -> Result<(MapImage, Vec<u8>)> {
        let a = self
            .inner
            .map_images
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or("Unknown map image")?;
        validate_asset(&a)?;
        let p = directory(&self.inner.root)?.join(format!("{}.png", a.id));
        tokio::task::spawn_blocking(move || {
            let b = read_file(&p, MAX_PNG)?;
            if b.len() != a.bytes || hash(&b) != a.sha256 {
                return Err("Saved map image changed; retrieve it again".into());
            }
            validate_png(&b, a.width, a.height)?;
            Ok((a, b))
        })
        .await
        .map_err(io_error)?
    }
    pub async fn inspect_map_image(&self, id: &str) -> Result<MapInspection> {
        let (asset, b) = self.map_bytes(id).await?;
        Ok(MapInspection {
            asset,
            image_url: format!("data:image/png;base64,{}", STANDARD.encode(b)),
        })
    }
    pub async fn export_map_image(&self, id: &str) -> Result<Vec<u8>> {
        let (a, b) = self.map_bytes(id).await?;
        let tile_path = directory(&self.inner.root)?.join(format!("{}.tiles.zip", a.id));
        tokio::task::spawn_blocking(move || {
            let tiles = if let Some(snapshot) = &a.source.wmts {
                Some(wmts::read_archive(&tile_path, snapshot)?)
            } else if let Some(snapshot) = &a.source.xyz {
                Some(wmts::read_archive(
                    &tile_path,
                    &xyz::tile_snapshot(snapshot)?,
                )?)
            } else {
                None
            };
            package_with_tiles(&a, b, tiles)
        })
        .await
        .map_err(io_error)?
    }
    pub async fn export_map_image_path(&self, id: &str, path: PathBuf) -> Result<()> {
        if path
            .extension()
            .and_then(|s| s.to_str())
            .is_none_or(|s| !s.eq_ignore_ascii_case("zip"))
        {
            return Err("Rendered map exports require a .zip filename".into());
        }
        let parent = path
            .parent()
            .ok_or("Choose an export directory")?
            .canonicalize()
            .map_err(io_error)?;
        let root = self.inner.root.canonicalize().map_err(io_error)?;
        if parent.starts_with(root) {
            return Err("Export outside the managed GeoD storage directory".into());
        }
        let path = parent.join(path.file_name().ok_or("Choose an export filename")?);
        if path.exists() {
            return Err("Export destination already exists; choose another filename".into());
        }
        let b = self.export_map_image(id).await?;
        tokio::task::spawn_blocking(move || {
            let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(io_error)?;
            tmp.write_all(&b).map_err(io_error)?;
            tmp.as_file().sync_all().map_err(io_error)?;
            tmp.persist_noclobber(path).map_err(io_error)?;
            Ok(())
        })
        .await
        .map_err(io_error)?
    }
}
#[cfg(test)]
fn package(a: &MapImage, b: Vec<u8>) -> Result<Vec<u8>> {
    package_with_tiles(a, b, None)
}
fn package_with_tiles(a: &MapImage, b: Vec<u8>, tiles: Option<Vec<u8>>) -> Result<Vec<u8>> {
    let [w, s, e, n] = a.image_extent.unwrap_or(a.bounds);
    let dx = (e - w) / f64::from(a.width);
    let dy = -(n - s) / f64::from(a.height);
    let world = format!(
        "{dx:.17}\n0.0\n0.0\n{dy:.17}\n{:.17}\n{:.17}\n",
        w + dx / 2.,
        n + dy / 2.
    );
    let projection=b"GEOGCS[\"WGS 84\",DATUM[\"WGS_1984\",SPHEROID[\"WGS 84\",6378137,298.257223563]],PRIMEM[\"Greenwich\",0],UNIT[\"degree\",0.0174532925199433],AUTHORITY[\"EPSG\",\"4326\"]]";
    // PNG drivers do not consistently consume a .prj sidecar. GDAL PAM records
    // both CRS and the corner transform, while PGW remains pixel-center based.
    let pam=format!("<PAMDataset><SRS dataAxisToSRSAxisMapping=\"{}\">{}</SRS><GeoTransform>{w:.17}, {dx:.17}, 0, {n:.17}, 0, {dy:.17}</GeoTransform></PAMDataset>", if a.crs == "EPSG:3857" { "1,2" } else { "2,1" }, a.crs);
    let mut files=vec![("map.png",b),("map.pgw",world.into_bytes()),("map.prj",projection.to_vec()),("map.png.aux.xml",pam.into_bytes()),("source.json",serde_json::to_vec_pretty(a).map_err(io_error)?),("README.txt",b"GeoD Global WMS rendered map image\n\nmap.png is the exact PNG returned by the named WMS service, verified with SHA-256. map.pgw records the WGS84 longitude/latitude output grid with pixel-center coordinates; map.prj declares EPSG:4326; map.png.aux.xml provides GDAL-compatible CRS and transform metadata. The requested image dimensions are a rendering grid, not sensor resolution. Pixels are rendered display colors, not original scientific bands or calibrated measurements. The image covers the requested bounding rectangle and is not clipped to a recorded polygon. The service time is the requested visualization time, not an inferred scene acquisition timestamp. Source metadata includes request and capabilities receipts and declared access constraints; those declarations are not a grant of all dataset reuse rights. No absolute local paths are included.\n".to_vec())];
    if a.source.wmts.is_some() {
        files.retain(|(name, _)| !matches!(*name, "map.prj" | "README.txt"));
        files.push(("README.txt", b"GeoD Global WMTS rendered map image\n\nmap.png contains a pixel-aligned rectangular window assembled from declared WMTS tiles without resampling. PNG tiles are expanded to RGBA; JPEG tiles are decoded as rendered RGB with opaque alpha. These display colors are not scientific source bands. map.pgw and map.png.aux.xml use the native CRS and pixel grid recorded in source.json. The requested WGS84 rectangle is expanded only to whole pixels. Recorded polygons are not masked. source-tiles.zip contains the exact tile responses, with hashes and request URLs in source.json. Service time is the requested visualization time. Service access declarations do not grant all dataset reuse rights.\n".to_vec()));
        if let Some(tiles) = &tiles {
            files.push(("source-tiles.zip", tiles.clone()));
        }
    }
    if a.source.xyz.is_some() {
        files.retain(|(name, _)| !matches!(*name, "README.txt" | "map.prj"));
        files.push(("README.txt", b"GeoD Global XYZ/TMS rendered tile window\n\nmap.png is a pixel-aligned rectangle copied from the configured EPSG:3857 grid without resampling. PNG tiles are expanded to RGBA; JPEG tiles are decoded as RGB with opaque alpha. The tile grid, row direction, logical zoom, URL zoom offset, requested bounds and original tile receipts are in source.json. Source archive filenames use top-origin logical row/column even for bottom-origin TMS requests. Grid metadata is user configuration, not inferred service capabilities. map.pgw records pixel centers and map.png.aux.xml records native CRS/corner transform. Recorded polygons are not masked. Colors are rendered visualization, not scientific bands. source-tiles.zip contains exact original responses. Public access does not grant all dataset reuse rights.\n".to_vec()));
        if let Some(tiles) = &tiles {
            files.push(("source-tiles.zip", tiles.clone()));
        }
    }
    if let Some(snapshot) = &a.source.arcgis {
        files.retain(|(name, _)| *name != "README.txt");
        files.push((
            "service-metadata.json",
            snapshot.capabilities.metadata.as_bytes().to_vec(),
        ));
        if let Some(layers) = &snapshot.capabilities.layers_metadata {
            files.push(("layers-metadata.json", layers.as_bytes().to_vec()));
        }
        files.push((
            "export-response.json",
            snapshot.export_metadata.as_bytes().to_vec(),
        ));
        files.push(("README.txt", b"GeoD Global ArcGIS rendered map image\n\nmap.png is the exact server PNG, verified with SHA-256. Its grid uses the actual WGS84 extent returned by export/exportImage, which may differ from the requested rectangle. map.pgw uses pixel centers; map.png.aux.xml and map.prj record EPSG:4326. Source metadata preserves the requested bounds, explicit time if any, exact service and export responses, default service rendering and declared attribution. This is a rendered visualization, not original scientific bands, calibrated values or a clipped polygon. Public access and copyright declarations do not grant all dataset reuse rights.\n".to_vec()));
    }
    let sums = files
        .iter()
        .map(|(name, b)| format!("{}  {name}\n", hash(b)))
        .collect::<String>();
    files.push(("checksums.sha256", sums.into_bytes()));
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o644);
    for (name, b) in files {
        zip.start_file(name, opts).map_err(io_error)?;
        zip.write_all(&b).map_err(io_error)?;
    }
    Ok(zip.finish().map_err(io_error)?.into_inner())
}
