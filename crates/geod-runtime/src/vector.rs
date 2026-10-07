//! Bounded local vector sources. Referenced files are never copied implicitly.
use crate::{io_error, now, storage, JobManager, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
};
use uuid::Uuid;
pub mod geopackage;
pub mod local_osm;
mod osm;
pub mod reads;
mod review;
pub use review::VectorApproval;
pub mod shapefile;
#[cfg(test)]
mod tests;

pub const MAX_BYTES: usize = 20 * 1024 * 1024;
pub const MAX_FEATURES: usize = 50_000;
pub const MAX_COORDINATES: usize = 500_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VectorAsset {
    pub id: String,
    pub name: String,
    pub format: String,
    pub storage_mode: String,
    pub crs: String,
    pub source_sha256: String,
    pub geojson_sha256: String,
    pub bytes: usize,
    pub feature_count: usize,
    pub coordinate_count: usize,
    pub geometry_counts: BTreeMap<String, usize>,
    pub bounds: Option<[f64; 4]>,
    pub created_at: String,
    pub attribution: Option<String>,
    pub license_url: Option<String>,
    pub data_timestamp: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_source: Option<crate::features::Provenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub osm_source: Option<crate::features::overpass::Provenance>,
    /// Missing on legacy records whose exact converted geometry/hash is retained.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub osm_conversion: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geo_package: Option<geopackage::Provenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shapefile: Option<shapefile::Provenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_osm: Option<local_osm::Provenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_approval: Option<VectorApproval>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Record {
    pub asset: VectorAsset,
    pub path: String,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorInspection {
    pub asset: VectorAsset,
    pub geojson: Value,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportVectorRequest {
    pub name: String,
    pub text: String,
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub(crate) fn validate_integer_precision(value: &Value) -> Result<()> {
    // JSON is also consumed by WebView JavaScript. Reject integer literals
    // that would silently change during response parsing or browser export.
    const SAFE: i64 = 9_007_199_254_740_991;
    match value {
        Value::Number(n) if n.as_u64().is_some_and(|n| n > SAFE as u64)
            || n.as_i64().is_some_and(|n| n < -SAFE) => {
            Err("GeoJSON integers beyond the safe range must be encoded as strings to preserve their values".into())
        }
        Value::Array(values) => values.iter().try_for_each(validate_integer_precision),
        Value::Object(values) => values.values().try_for_each(validate_integer_precision),
        _ => Ok(()),
    }
}
fn name(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 120 || value.chars().any(char::is_control) {
        return Err("Use a vector file name of 1–120 characters without control characters".into());
    }
    Ok(value.into())
}
#[derive(Default)]
struct Stats {
    coordinates: usize,
    bounds: Option<[f64; 4]>,
    geometries: BTreeMap<String, usize>,
}
fn position(value: &Value, stats: &mut Stats) -> Result<()> {
    let p = value
        .as_array()
        .ok_or("Vector positions must be coordinate arrays")?;
    if p.is_empty() {
        return Ok(());
    }
    if !(2..=3).contains(&p.len()) || p.iter().any(|v| v.as_f64().is_none_or(|x| !x.is_finite())) {
        return Err("Vector positions require two or three finite coordinates".into());
    }
    let x = p[0].as_f64().unwrap();
    let y = p[1].as_f64().unwrap();
    if !(-180.0..=180.0).contains(&x) || !(-90.0..=90.0).contains(&y) {
        return Err(
            "GeoJSON requires WGS84 longitude and latitude; another CRS is not guessed".into(),
        );
    }
    stats.coordinates += 1;
    if stats.coordinates > MAX_COORDINATES {
        return Err("Vector file exceeds 500,000 coordinates".into());
    }
    let b = stats.bounds.get_or_insert([x, y, x, y]);
    b[0] = b[0].min(x);
    b[1] = b[1].min(y);
    b[2] = b[2].max(x);
    b[3] = b[3].max(y);
    Ok(())
}
fn list(value: &Value, min: usize) -> Result<&Vec<Value>> {
    value
        .as_array()
        .filter(|v| v.len() >= min)
        .ok_or("Vector geometry has missing or insufficient coordinates".into())
}
fn line(value: &Value, stats: &mut Stats, ring: bool) -> Result<()> {
    if value.as_array().is_some_and(|p| p.is_empty()) {
        return Ok(());
    }
    let p = list(value, if ring { 4 } else { 2 })?;
    if ring && p.first() != p.last() {
        return Err("Polygon rings must be closed".into());
    }
    for p in p {
        position(p, stats)?;
    }
    Ok(())
}
fn polygon(value: &Value, stats: &mut Stats) -> Result<()> {
    if value.as_array().is_some_and(|p| p.is_empty()) {
        return Ok(());
    }
    for ring in list(value, 1)? {
        line(ring, stats, true)?;
    }
    Ok(())
}
fn geometry(value: &Value, stats: &mut Stats, depth: usize) -> Result<()> {
    if value.is_null() {
        return Ok(());
    }
    if depth > 8 || !value.is_object() || value.get("crs").is_some() {
        return Err("Unsupported vector geometry or nested CRS".into());
    }
    let kind = value["type"]
        .as_str()
        .ok_or("Vector geometry requires a type")?;
    *stats.geometries.entry(kind.into()).or_default() += 1;
    let c = &value["coordinates"];
    if c.as_array().is_some_and(|p| p.is_empty()) {
        return Ok(());
    }
    match kind {
        "Point" => position(c, stats),
        "MultiPoint" => {
            for p in list(c, 1)? {
                position(p, stats)?;
            }
            Ok(())
        }
        "LineString" => line(c, stats, false),
        "MultiLineString" => {
            for p in list(c, 1)? {
                line(p, stats, false)?;
            }
            Ok(())
        }
        "Polygon" => polygon(c, stats),
        "MultiPolygon" => {
            for p in list(c, 1)? {
                polygon(p, stats)?;
            }
            Ok(())
        }
        "GeometryCollection" => {
            if value["geometries"].as_array().is_some_and(|p| p.is_empty()) {
                return Ok(());
            }
            for g in list(&value["geometries"], 1)? {
                geometry(g, stats, depth + 1)?;
            }
            Ok(())
        }
        _ => Err("Unsupported GeoJSON geometry type".into()),
    }
}
fn normalize(bytes: &[u8], asset: VectorAsset) -> Result<VectorInspection> {
    normalize_kind(bytes, asset, None)
}
fn normalize_kind(
    bytes: &[u8],
    mut asset: VectorAsset,
    container: Option<&str>,
) -> Result<VectorInspection> {
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err("Vector file must contain at most 20 MiB".into());
    }
    let raw: Value = if local_osm::is_xml(bytes) || local_osm::is_pbf(bytes) {
        if asset.remote_source.is_some()
            || asset.osm_source.is_some()
            || asset.osm_conversion.is_some()
            || asset.geo_package.is_some()
            || asset.shapefile.is_some()
            || container.is_some()
        {
            return Err("Local OSM snapshots require distinct original-file provenance".into());
        }
        let (data, source) = local_osm::decode(bytes)?;
        if asset
            .local_osm
            .as_ref()
            .is_some_and(|saved| saved != &source)
        {
            return Err(
                "OSM snapshot content differs from its recorded conversion metadata".into(),
            );
        }
        asset.local_osm = Some(source);
        data
    } else if bytes.starts_with(b"SQLite format 3\0") {
        if asset.remote_source.is_some()
            || asset.osm_source.is_some()
            || asset.osm_conversion.is_some()
            || asset.shapefile.is_some()
            || asset.local_osm.is_some()
        {
            return Err("GeoPackage files require distinct original-file provenance".into());
        }
        let (data, source) = geopackage::decode(bytes)?;
        if asset
            .geo_package
            .as_ref()
            .is_some_and(|saved| saved != &source)
        {
            return Err("GeoPackage content differs from its recorded conversion metadata".into());
        }
        asset.geo_package = Some(source);
        data
    } else if bytes.starts_with(b"PK\x03\x04") {
        if asset.remote_source.is_some()
            || asset.osm_source.is_some()
            || asset.osm_conversion.is_some()
            || asset.geo_package.is_some()
            || asset.local_osm.is_some()
        {
            return Err("Shapefile bundles require distinct original-file provenance".into());
        }
        let kind = container
            .or_else(|| asset.shapefile.as_ref().map(|s| s.container.as_str()))
            .unwrap_or("zip");
        let (data, source) = shapefile::decode(bytes, kind)?;
        if asset
            .shapefile
            .as_ref()
            .is_some_and(|saved| saved != &source)
        {
            return Err("Shapefile content differs from its recorded conversion metadata".into());
        }
        asset.shapefile = Some(source);
        data
    } else {
        if asset.geo_package.is_some()
            || asset.shapefile.is_some()
            || asset.local_osm.is_some()
            || container.is_some()
        {
            return Err("Saved binary vector source no longer contains its original file".into());
        }
        serde_json::from_slice(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes))
            .map_err(|e| format!("Invalid vector JSON: {e}"))?
    };
    validate_integer_precision(&raw)?;
    let (mut data, format, timestamp) = if let Some(source) = &asset.local_osm {
        let format = if source.encoding == "xml" {
            "osm-xml"
        } else {
            "osm-pbf"
        };
        (raw, format, source.dataset_timestamp.clone())
    } else if asset.geo_package.is_some() {
        (raw, "geopackage", None)
    } else if asset.shapefile.is_some() {
        (raw, "shapefile", None)
    } else if let Some(source) = asset.remote_source.as_ref().filter(|s| s.wfs.is_some()) {
        if asset.storage_mode != "managed"
            || asset.osm_source.is_some()
            || asset.osm_conversion.is_some()
        {
            return Err(
                "WFS snapshots require managed original responses and distinct provenance".into(),
            );
        }
        (
            crate::features::wfs::decode_bundle(&raw, source)?,
            "wfs-snapshot",
            None,
        )
    } else if raw.get("elements").is_some() {
        let timestamp = raw["osm3s"]["timestamp_osm_base"]
            .as_str()
            .ok_or("OSM extract has no dataset timestamp")?;
        chrono::DateTime::parse_from_rfc3339(timestamp)
            .map_err(|_| "OSM extract has an invalid dataset timestamp")?;
        let converted = if let Some(source) = &asset.osm_source {
            if asset.remote_source.is_some() || asset.osm_conversion != Some(2) {
                return Err(
                    "OSM extraction requires its current conversion and distinct provenance".into(),
                );
            }
            let elements = source.validate_raw(bytes, &raw)?;
            osm::convert_current_selected(&elements, source.element_counts.total)?
        } else if asset.osm_conversion == Some(2) || asset.geojson_sha256.is_empty() {
            asset.osm_conversion = Some(2);
            osm::convert_current(&raw)?
        } else {
            osm::convert(&raw)?
        };
        (converted, "overpass-json", Some(timestamp.to_string()))
    } else {
        let value = if raw["type"] == "Feature" {
            json!({"type":"FeatureCollection","features":[raw]})
        } else {
            raw
        };
        (value, "geojson", None)
    };
    if data["type"] != "FeatureCollection" || data.get("crs").is_some() {
        return Err(
            "Open RFC 7946 GeoJSON in WGS84; legacy CRS declarations are not guessed".into(),
        );
    }
    if let Some(source) = &asset.osm_source {
        if format != "overpass-json" || asset.storage_mode != "managed" {
            return Err("OSM query snapshots must contain the managed original response".into());
        }
        data["geodOsmSource"] = serde_json::to_value(source).map_err(io_error)?;
    }
    let features = data["features"]
        .as_array()
        .ok_or("GeoJSON requires a features array")?;
    if let Some(source) = &asset.osm_source {
        source.validate(features.len())?;
    }
    if let Some(source) = &asset.remote_source {
        source.validate(features.len())?;
        let expected = if source.wfs.is_some() {
            "wfs-snapshot"
        } else {
            "geojson"
        };
        if format != expected
            || data.get("geodSource") != Some(&serde_json::to_value(source).map_err(io_error)?)
        {
            return Err("Feature query content differs from its recorded provenance".into());
        }
    }
    if features.len() > MAX_FEATURES {
        return Err("Vector file exceeds 50,000 features".into());
    }
    let mut stats = Stats::default();
    for f in features {
        if f["type"] != "Feature"
            || !f
                .get("properties")
                .is_some_and(|v| v.is_object() || v.is_null())
            || f.get("geometry").is_none()
            || f.get("crs").is_some()
            || f.get("id")
                .is_some_and(|v| !v.is_string() && !v.is_number())
        {
            return Err(
                "GeoJSON features require geometry, properties and valid optional identity".into(),
            );
        }
        geometry(&f["geometry"], &mut stats, 0)?;
    }
    asset.format = format.into();
    asset.crs = "EPSG:4326".into();
    asset.source_sha256 = hash(bytes);
    asset.geojson_sha256 = hash(&serde_json::to_vec(&data).map_err(io_error)?);
    asset.bytes = bytes.len();
    asset.feature_count = features.len();
    asset.coordinate_count = stats.coordinates;
    asset.geometry_counts = stats.geometries;
    asset.bounds = stats.bounds;
    asset.data_timestamp = timestamp;
    if ["overpass-json", "osm-xml", "osm-pbf"].contains(&format) {
        asset.attribution = Some("© OpenStreetMap contributors".into());
        asset.license_url = Some("https://www.openstreetmap.org/copyright".into());
    } else {
        asset.attribution = None;
        asset.license_url = None;
    }
    Ok(VectorInspection {
        asset,
        geojson: data,
    })
}
fn initial(title: &str, mode: &str) -> Result<VectorAsset> {
    Ok(VectorAsset {
        id: Uuid::new_v4().to_string(),
        name: name(title)?,
        format: String::new(),
        storage_mode: mode.into(),
        crs: String::new(),
        source_sha256: String::new(),
        geojson_sha256: String::new(),
        bytes: 0,
        feature_count: 0,
        coordinate_count: 0,
        geometry_counts: BTreeMap::new(),
        bounds: None,
        created_at: now(),
        remote_source: None,
        osm_source: None,
        osm_conversion: None,
        geo_package: None,
        shapefile: None,
        local_osm: None,
        agent_approval: None,
        attribution: None,
        license_url: None,
        data_timestamp: None,
    })
}
fn source(root: &Path, record: &Record) -> Result<PathBuf> {
    let p = Path::new(&record.path);
    if record.asset.storage_mode == "managed" {
        let directory = root.join("vectors");
        if directory.canonicalize().map_err(io_error)? != directory {
            return Err("Vector storage was redirected".into());
        }
        storage::exact_file(
            &root.join("vectors").join(format!(
                "{}.{}",
                record.asset.id,
                source_extension(&record.asset)
            )),
            p,
        )
    } else {
        storage::regular_file(p)
    }
}
fn read(path: &Path) -> Result<Vec<u8>> {
    let mut file = std::fs::File::open(path).map_err(io_error)?;
    if file.metadata().map_err(io_error)?.len() > MAX_BYTES as u64 {
        return Err("Vector file exceeds 20 MiB".into());
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() > MAX_BYTES {
        return Err("Vector file exceeds 20 MiB".into());
    }
    Ok(bytes)
}
fn source_extension(asset: &VectorAsset) -> &'static str {
    if asset.geo_package.is_some() {
        "gpkg"
    } else if asset.shapefile.is_some() {
        "zip"
    } else if let Some(source) = &asset.local_osm {
        if source.encoding == "xml" {
            "osm"
        } else {
            "pbf"
        }
    } else {
        "json"
    }
}
fn read_record_source(root: &Path, record: &Record) -> Result<Vec<u8>> {
    let path = source(root, record)?;
    if record.asset.storage_mode == "reference"
        && record
            .asset
            .shapefile
            .as_ref()
            .is_some_and(|s| s.container == "sidecars")
    {
        shapefile::sidecar_bundle(&path)
    } else {
        read(&path)
    }
}
pub(crate) async fn load(root: &Path) -> Result<BTreeMap<String, Record>> {
    let directory = root.join("vectors");
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(io_error)?;
    if directory.canonicalize().map_err(io_error)? != directory {
        return Err("Vector storage cannot be redirected".into());
    }
    let records: BTreeMap<String, Record> = match tokio::fs::read(root.join("vectors.json")).await {
        Ok(b) => {
            serde_json::from_slice(&b).map_err(|e| format!("Cannot read vector registry: {e}"))?
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
        Err(e) => return Err(io_error(e)),
    };
    for (id, r) in &records {
        if let Some(approval) = &r.asset.agent_approval {
            approval.validate()?;
            if r.asset.storage_mode != "managed"
                || r.asset.remote_source.is_none() && r.asset.osm_source.is_none()
            {
                return Err(
                    "Agent vector approval requires an original managed service snapshot".into(),
                );
            }
        }
        if let Some(source) = &r.asset.geo_package {
            geopackage::validate(source, r.asset.feature_count)?;
            if r.asset.format != "geopackage"
                || r.asset.remote_source.is_some()
                || r.asset.osm_source.is_some()
                || r.asset.osm_conversion.is_some()
                || r.asset.shapefile.is_some()
                || r.asset.local_osm.is_some()
            {
                return Err("Invalid saved GeoPackage storage or provenance".into());
            }
        }
        if let Some(source) = &r.asset.shapefile {
            shapefile::validate(source, r.asset.feature_count)?;
            if r.asset.format != "shapefile"
                || r.asset.remote_source.is_some()
                || r.asset.osm_source.is_some()
                || r.asset.osm_conversion.is_some()
                || r.asset.geo_package.is_some()
                || r.asset.local_osm.is_some()
            {
                return Err("Invalid saved Shapefile storage or provenance".into());
            }
            if r.asset.storage_mode == "reference"
                && source.container == "sidecars"
                && !r.path.to_lowercase().ends_with(".shp")
            {
                return Err("Referenced Shapefile sidecars require their original SHP path".into());
            }
        }
        if let Some(source) = &r.asset.local_osm {
            local_osm::validate(source, r.asset.feature_count)?;
            if r.asset.remote_source.is_some()
                || r.asset.osm_source.is_some()
                || r.asset.osm_conversion.is_some()
                || r.asset.geo_package.is_some()
                || r.asset.shapefile.is_some()
                || r.asset.format
                    != if source.encoding == "xml" {
                        "osm-xml"
                    } else {
                        "osm-pbf"
                    }
                || r.asset.data_timestamp != source.dataset_timestamp
            {
                return Err("Invalid saved local OSM storage or provenance".into());
            }
        }
        if let Some(source) = &r.asset.osm_source {
            source.validate(r.asset.feature_count)?;
            if r.asset.remote_source.is_some()
                || r.asset.storage_mode != "managed"
                || r.asset.format != "overpass-json"
                || r.asset.osm_conversion != Some(2)
                || r.asset.source_sha256 != source.response_sha256
                || r.asset.bytes != source.bytes
                || r.asset.data_timestamp.as_ref() != Some(&source.data_timestamp)
            {
                return Err("Invalid saved OSM query metadata".into());
            }
        }
        if r.asset
            .osm_conversion
            .is_some_and(|version| version != 2 || r.asset.format != "overpass-json")
        {
            return Err("Unsupported OSM conversion version".into());
        }
        if let Some(source) = &r.asset.remote_source {
            source.validate(r.asset.feature_count)?;
            let expected = if source.wfs.is_some() {
                "wfs-snapshot"
            } else {
                "geojson"
            };
            if r.asset.storage_mode != "managed" || r.asset.format != expected {
                return Err(
                    "Feature query snapshots must retain their managed source encoding".into(),
                );
            }
        }
        if r.asset.format == "wfs-snapshot"
            && r.asset
                .remote_source
                .as_ref()
                .is_none_or(|s| s.wfs.is_none())
        {
            return Err("WFS snapshots require recorded WFS provenance".into());
        }
        let bounds_valid = match r.asset.bounds {
            Some([w, s, e, n]) => {
                r.asset.coordinate_count > 0
                    && [w, s, e, n].iter().all(|v| v.is_finite())
                    && -180.0 <= w
                    && w <= e
                    && e <= 180.0
                    && -90.0 <= s
                    && s <= n
                    && n <= 90.0
            }
            None => r.asset.coordinate_count == 0,
        };
        let osm_metadata = if ["overpass-json", "osm-xml", "osm-pbf"]
            .contains(&r.asset.format.as_str())
        {
            r.asset.attribution.as_deref() == Some("© OpenStreetMap contributors")
                && r.asset.license_url.as_deref() == Some("https://www.openstreetmap.org/copyright")
                && (r.asset.local_osm.is_some()
                    || r.asset
                        .data_timestamp
                        .as_deref()
                        .is_some_and(|t| chrono::DateTime::parse_from_rfc3339(t).is_ok()))
        } else {
            r.asset.attribution.is_none()
                && r.asset.license_url.is_none()
                && r.asset.data_timestamp.is_none()
        };
        if Uuid::parse_str(id).ok().map(|id| id.to_string()).as_ref() != Some(id)
            || r.asset.id != *id
            || name(&r.asset.name)? != r.asset.name
            || !["managed", "reference"].contains(&r.asset.storage_mode.as_str())
            || ![
                "geojson",
                "overpass-json",
                "wfs-snapshot",
                "geopackage",
                "shapefile",
                "osm-xml",
                "osm-pbf",
            ]
            .contains(&r.asset.format.as_str())
            || (r.asset.format == "geopackage") != r.asset.geo_package.is_some()
            || (r.asset.format == "shapefile") != r.asset.shapefile.is_some()
            || ["osm-xml", "osm-pbf"].contains(&r.asset.format.as_str())
                != r.asset.local_osm.is_some()
            || r.asset.crs != "EPSG:4326"
            || r.asset.bytes == 0
            || r.asset.bytes > MAX_BYTES
            || r.asset.feature_count > MAX_FEATURES
            || r.asset.coordinate_count > MAX_COORDINATES
            || records.len() > 1024
            || !bounds_valid
            || !osm_metadata
            || chrono::DateTime::parse_from_rfc3339(&r.asset.created_at).is_err()
            || r.asset.geometry_counts.iter().any(|(kind, count)| {
                *count == 0
                    || *count > MAX_COORDINATES
                    || ![
                        "Point",
                        "MultiPoint",
                        "LineString",
                        "MultiLineString",
                        "Polygon",
                        "MultiPolygon",
                        "GeometryCollection",
                    ]
                    .contains(&kind.as_str())
            })
            || [&r.asset.source_sha256, &r.asset.geojson_sha256]
                .iter()
                .any(|v| {
                    v.len() != 64
                        || !v
                            .bytes()
                            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                })
        {
            return Err("Stored vector metadata is invalid".into());
        }
    }
    Ok(records)
}
impl JobManager {
    pub async fn list_vectors(&self) -> Vec<VectorAsset> {
        self.inner
            .vectors
            .lock()
            .await
            .values()
            .map(|r| r.asset.clone())
            .collect()
    }
    async fn persist_vectors(&self, records: &BTreeMap<String, Record>) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(records).map_err(io_error)?;
        let path = self.inner.root.join("vectors.json.tmp");
        let mut file = tokio::fs::File::create(&path).await.map_err(io_error)?;
        use tokio::io::AsyncWriteExt;
        file.write_all(&bytes).await.map_err(io_error)?;
        file.sync_all().await.map_err(io_error)?;
        drop(file);
        tokio::fs::rename(path, self.inner.root.join("vectors.json"))
            .await
            .map_err(io_error)
    }
    async fn register_vector(
        &self,
        inspection: VectorInspection,
        path: PathBuf,
    ) -> Result<VectorAsset> {
        self.inner.store.lock().await.accepting_jobs()?;
        let asset = inspection.asset;
        let mut records = self.inner.vectors.lock().await;
        if records.contains_key(&asset.id) {
            return Err("Vector identity is already registered".into());
        }
        if records.len() >= 1024 {
            return Err("Vector registry is full (1,024 files)".into());
        }
        records.insert(
            asset.id.clone(),
            Record {
                asset: asset.clone(),
                path: path.to_string_lossy().into_owned(),
            },
        );
        if let Err(error) = self.persist_vectors(&records).await {
            records.remove(&asset.id);
            return Err(error);
        }
        Ok(asset)
    }
    pub async fn import_vector(&self, request: ImportVectorRequest) -> Result<VectorAsset> {
        self.import_vector_source(request, None).await
    }
    /// Content upload only. Loopback callers never provide a native file path.
    pub async fn import_vector_file_bytes(
        &self,
        file_name: String,
        bytes: Vec<u8>,
    ) -> Result<VectorAsset> {
        if Path::new(&file_name).components().count() != 1
            || file_name.contains(['/', '\\', ':'])
            || !["gpkg", "zip", "osm", "xml", "pbf"]
                .iter()
                .any(|ext| file_name.to_lowercase().ends_with(&format!(".{ext}")))
        {
            return Err(
                "Choose a GeoPackage, Shapefile ZIP or OSM XML / PBF name without a directory path"
                    .into(),
            );
        }
        if file_name.to_lowercase().ends_with(".gpkg") && !bytes.starts_with(b"SQLite format 3\0")
            || file_name.to_lowercase().ends_with(".zip") && !bytes.starts_with(b"PK\x03\x04")
            || [".osm", ".xml"]
                .iter()
                .any(|ext| file_name.to_lowercase().ends_with(ext))
                && !local_osm::is_xml(&bytes)
            || file_name.to_lowercase().ends_with(".pbf") && !local_osm::is_pbf(&bytes)
        {
            return Err(
                "The selected file does not match its GeoPackage, Shapefile ZIP or OSM XML / PBF format".into(),
            );
        }
        self.import_vector_bytes(bytes, initial(&file_name, "managed")?)
            .await
    }
    pub(crate) async fn import_vector_source(
        &self,
        request: ImportVectorRequest,
        source: Option<crate::features::Provenance>,
    ) -> Result<VectorAsset> {
        let mut title = initial(&request.name, "managed")?;
        title.remote_source = source;
        let bytes = request.text.into_bytes();
        self.import_vector_bytes(bytes, title).await
    }
    pub(crate) async fn import_wfs_source(
        &self,
        bytes: Vec<u8>,
        source: crate::features::Provenance,
    ) -> Result<VectorAsset> {
        if source.wfs.is_none() {
            return Err("Missing WFS snapshot provenance".into());
        }
        let name = format!(
            "{} · WFS",
            source
                .collection_title
                .chars()
                .take(110)
                .collect::<String>()
        );
        let mut asset = initial(&name, "managed")?;
        asset.remote_source = Some(source);
        self.import_vector_bytes(bytes, asset).await
    }
    pub(crate) async fn import_osm_source(
        &self,
        bytes: Vec<u8>,
        source: crate::features::overpass::Provenance,
    ) -> Result<VectorAsset> {
        let mut title = initial(&format!("{} · OSM", source.preset_title), "managed")?;
        title.osm_source = Some(source);
        title.osm_conversion = Some(2);
        self.import_vector_bytes(bytes, title).await
    }
    async fn import_vector_bytes(&self, bytes: Vec<u8>, title: VectorAsset) -> Result<VectorAsset> {
        self.import_vector_bytes_as(bytes, title, None).await
    }
    async fn import_vector_bytes_as(
        &self,
        bytes: Vec<u8>,
        title: VectorAsset,
        container: Option<String>,
    ) -> Result<VectorAsset> {
        let _permit = self
            .inner
            .raster_permits
            .clone()
            .acquire_owned()
            .await
            .map_err(io_error)?;
        let inspection = tokio::task::spawn_blocking(move || {
            normalize_kind(&bytes, title, container.as_deref()).map(|i| (i, bytes))
        })
        .await
        .map_err(io_error)??;
        let (inspection, bytes) = inspection;
        let path = self.inner.root.join("vectors").join(format!(
            "{}.{}",
            inspection.asset.id,
            source_extension(&inspection.asset)
        ));
        let directory = self.inner.root.join("vectors");
        if directory.canonicalize().map_err(io_error)? != directory {
            return Err("Vector storage was redirected".into());
        }
        let output = path.clone();
        tokio::task::spawn_blocking(move || {
            use std::io::Write;
            let mut temporary = tempfile::NamedTempFile::new_in(directory).map_err(io_error)?;
            temporary.write_all(&bytes).map_err(io_error)?;
            temporary.as_file().sync_all().map_err(io_error)?;
            temporary.persist_noclobber(output).map_err(io_error)?;
            Ok::<_, String>(())
        })
        .await
        .map_err(io_error)??;
        let result = self.register_vector(inspection, path.clone()).await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(path).await;
        }
        result
    }
    /// Desktop-only caller obtains the path from a user-operated native chooser.
    pub async fn open_vector_path(&self, path: PathBuf, managed: bool) -> Result<VectorAsset> {
        let path = storage::regular_file(&path)?;
        let title = path
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or("Vector file has no Unicode name")?
            .to_string();
        let read_path = path.clone();
        let container = path
            .extension()
            .and_then(|s| s.to_str())
            .filter(|s| s.eq_ignore_ascii_case("shp"))
            .map(|_| "sidecars".to_string());
        let sidecars = container.is_some();
        let bytes = tokio::task::spawn_blocking(move || {
            if sidecars {
                shapefile::sidecar_bundle(&read_path)
            } else {
                read(&read_path)
            }
        })
        .await
        .map_err(io_error)??;
        if managed {
            return self
                .import_vector_bytes_as(bytes, initial(&title, "managed")?, container)
                .await;
        }
        let asset = initial(&title, "reference")?;
        let _permit = self
            .inner
            .raster_permits
            .clone()
            .acquire_owned()
            .await
            .map_err(io_error)?;
        let inspection = tokio::task::spawn_blocking(move || {
            normalize_kind(&bytes, asset, container.as_deref())
        })
        .await
        .map_err(io_error)??;
        self.register_vector(inspection, path).await
    }
    pub async fn inspect_vector(&self, id: &str) -> Result<VectorInspection> {
        let _permit = self
            .inner
            .raster_permits
            .clone()
            .acquire_owned()
            .await
            .map_err(io_error)?;
        let record = self
            .inner
            .vectors
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or("Unknown vector file")?;
        let root = self.inner.root.clone();
        tokio::task::spawn_blocking(move || {
            let bytes = read_record_source(&root, &record)?;
            if hash(&bytes) != record.asset.source_sha256 {
                return Err(
                    "Vector source changed after opening; open the changed file again".into(),
                );
            }
            let inspection = normalize(&bytes, record.asset.clone())?;
            if inspection.asset != record.asset {
                return Err(
                    "Vector content differs from its recorded geometry or source metadata".into(),
                );
            }
            Ok(inspection)
        })
        .await
        .map_err(io_error)?
    }
    pub async fn forget_vector(&self, id: &str) -> Result<()> {
        self.inner.store.lock().await.accepting_jobs()?;
        let mut records = self.inner.vectors.lock().await;
        let old = records.remove(id).ok_or("Unknown vector file")?;
        if let Err(e) = self.persist_vectors(&records).await {
            records.insert(id.into(), old);
            return Err(e);
        }
        // Removing a registration never deletes a user's source file.
        Ok(())
    }
    /// Destination is chosen by the desktop user, never accepted by loopback HTTP.
    pub async fn export_vector_path(&self, id: &str, path: PathBuf) -> Result<()> {
        let data = self.inspect_vector(id).await?;
        let bytes = serde_json::to_vec(&data.geojson).map_err(io_error)?;
        self.save_vector_export(path, bytes).await
    }
    pub async fn vector_original_bytes(&self, id: &str) -> Result<Vec<u8>> {
        let inspection = self.inspect_vector(id).await?;
        let record = self
            .inner
            .vectors
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or("Unknown vector file")?;
        let root = self.inner.root.clone();
        tokio::task::spawn_blocking(move || {
            let bytes = read_record_source(&root, &record)?;
            if hash(&bytes) != inspection.asset.source_sha256 {
                return Err(
                    "Vector source changed after opening; open the changed file again".into(),
                );
            }
            Ok(bytes)
        })
        .await
        .map_err(io_error)?
    }
    pub async fn export_vector_original_path(&self, id: &str, path: PathBuf) -> Result<()> {
        let bytes = self.vector_original_bytes(id).await?;
        self.save_vector_export(path, bytes).await
    }
    async fn save_vector_export(&self, path: PathBuf, bytes: Vec<u8>) -> Result<()> {
        let parent = path
            .parent()
            .ok_or("Export destination has no parent folder")?
            .canonicalize()
            .map_err(io_error)?;
        let destination = parent.join(path.file_name().ok_or("Export destination has no name")?);
        if destination.starts_with(&self.inner.root) {
            return Err("Export outside the managed runtime storage".into());
        }
        if destination.exists() {
            let target = storage::regular_file(&destination)?;
            let records = self.inner.vectors.lock().await;
            if records.values().any(|r| {
                if storage::regular_file(Path::new(&r.path)).is_ok_and(|p| p == target) {
                    return true;
                }
                r.asset.storage_mode == "reference"
                    && r.asset
                        .shapefile
                        .as_ref()
                        .filter(|s| s.container == "sidecars")
                        .is_some_and(|s| {
                            Path::new(&r.path).parent().is_some_and(|parent| {
                                s.files.iter().any(|f| {
                                    storage::regular_file(&parent.join(&f.name))
                                        .is_ok_and(|p| p == target)
                                })
                            })
                        })
            }) {
                return Err("Export must not overwrite a registered vector source".into());
            }
        }
        tokio::task::spawn_blocking(move || {
            use std::io::Write;
            let mut file = tempfile::NamedTempFile::new_in(parent).map_err(io_error)?;
            file.write_all(&bytes).map_err(io_error)?;
            file.as_file().sync_all().map_err(io_error)?;
            file.persist(destination).map_err(io_error)?;
            Ok(())
        })
        .await
        .map_err(io_error)?
    }
}
