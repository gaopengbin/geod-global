//! Complete, fixed GeoParquet originals are stored independently of the bounded
//! GeoJSON preview. AOI selection never rewrites the original container.
use super::{hash, io_error, Result, MAX_BYTES, MAX_FEATURES};
use crate::{features, now, storage, JobManager};
use futures_util::StreamExt;
use parquet::{
    file::reader::{FileReader, SerializedFileReader},
    record::{Field, Row},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::Duration,
};

#[cfg(test)]
mod tests;

pub const ORIGINAL_LIMIT: u64 = 512 * 1024 * 1024;
pub const ROW_GROUP_LIMIT: i64 = 32 * 1024 * 1024;
const FOOTER_LIMIT: u64 = 4 * 1024 * 1024;
const ROW_PAGE_LIMIT: usize = 22 * 1024; // reserve 2 KiB for the response envelope
const ORIGINAL_BYTES: u64 = 52_018_294;
const ORIGINAL_ROWS: usize = 407_277;
const ORIGINAL_HASH: &str = "cfaf02a15f70713a52ed2f4367858d1b3d3be0dadba2a9aa4ee7faf0267ea23d";
const URL: &str = "https://overturemaps-us-west-2.s3.us-west-2.amazonaws.com/release/2026-09-23.1/theme=base/type=bathymetry/part-00000-304001d2-9e92-5aeb-8db5-60be5bd09d96-c000.zstd.parquet";
const ID: &str = "base-bathymetry-2026-09-23.1-part-00000";
const LICENSE: &str = "https://opendatacommons.org/licenses/odbl/1-0/";
const ATTRIBUTION: &str = "Overture Maps Foundation, base/bathymetry 2026-09-23.1; ODbL 1.0 theme with source-specific ETOPO/GLOBathy notices retained in every original row. Retain original source attribution and Overture attribution/ licensing documentation.";
const SELECTION: &str = "GeoJSON preview of complete features whose publisher per-row bbox intersects the AOI; no geometry clipping; full GeoParquet original retained separately";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub dataset_id: String,
    pub package_id: Option<String>,
    pub bounds: [f64; 4],
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Provenance {
    pub conversion: u32,
    pub dataset_id: String,
    pub package_id: String,
    pub original_url: String,
    pub original_sha256: String,
    pub original_bytes: u64,
    pub original_rows: usize,
    pub row_groups: usize,
    pub geo_metadata: Value,
    pub schema: String,
    pub max_row_group_uncompressed_bytes: i64,
    pub requested_bounds: [f64; 4],
    pub preview_feature_count: usize,
    pub selection: String,
    pub fetched_at: String,
    pub attribution: String,
    pub license_url: String,
}
impl Provenance {
    pub fn validate(&self) -> Result<()> {
        features::bounds(self.requested_bounds)?;
        validate_geo(&self.geo_metadata)?;
        if self.conversion != 1
            || self.dataset_id != "overture-maps"
            || self.package_id != ID
            || self.original_url != URL
            || self.original_sha256 != ORIGINAL_HASH
            || self.original_bytes != ORIGINAL_BYTES
            || self.original_rows != ORIGINAL_ROWS
            || self.row_groups != 16
            || self.preview_feature_count > MAX_FEATURES
            || !(1..=ROW_GROUP_LIMIT).contains(&self.max_row_group_uncompressed_bytes)
            || self.schema.len() > 64 * 1024
            || self.selection != SELECTION
            || self.attribution != ATTRIBUTION
            || self.license_url != LICENSE
            || chrono::DateTime::parse_from_rfc3339(&self.fetched_at).is_err()
        {
            return Err("Invalid saved complete GeoParquet original/preview provenance".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rows {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}
pub fn providers() -> Value {
    json!([{"id":"overture-maps","title":"Overture Maps · base bathymetry GeoParquet",
        "version":"2026-09-23.1","discovery":"official-parquet-original","packageCount":1,
        "formats":["GeoParquet WKB / bounded AOI GeoJSON preview"],"license":"ODbL 1.0; source-specific notices",
        "licenseUrl":LICENSE,"documentationUrl":"https://docs.overturemaps.org/schema/reference/base/bathymetry/",
        "warnings":["Only this exact complete official base/bathymetry part is connected. Buildings, places, transport and other partitions are not implied.",
        "The 52,018,294-byte, 407,277-row original is retained as GeoParquet. AOI output is a bounded preview subset, never a full original GeoJSON export or a server-generated subset download.",
        "Depth is the source feature depth below surface in metres. These ETOPO/GLOBathy-derived polygons are thematic underwater representations, not a new seabed measurement or a pointwise depth grid.",
        "Original container cap is 512 MiB; per-row-group uncompressed cap is 32 MiB. GeoJSON previews still have the existing 20 MiB / 50,000-feature / 500,000-coordinate limits and can be refused for large AOIs.",
        "Selection uses the publisher per-row bounding box intersection, not exact polygon intersection. Complete source geometry, IDs, nulls, version, cartography and nested source attributes are retained for matched rows; no clipping, reprojection, rounding or topology repair."]}])
}
pub fn search(request: Request) -> Result<Value> {
    validate_request(&request)?;
    Ok(
        json!({"datasetId":"overture-maps","bounds":request.bounds,"complete":true,
        "items":[{"datasetId":"overture-maps","id":ID,"title":"Overture base/bathymetry · 2026-09-23.1 · whole GeoParquet part",
            "format":"geoparquet","fileName":"Overture-2026-09-23.1-base-bathymetry-part-00000.parquet",
            "url":URL,"bounds":[-180.,-90.,180.,90.],"release":"2026-09-23.1",
            "originalBytes":ORIGINAL_BYTES,"originalRows":ORIGINAL_ROWS,"expectedSha256":ORIGINAL_HASH,
            "sourceIndexUrl":"https://stac.overturemaps.org/2026-09-23.1/base/bathymetry/00000/00000.json",
            "licenseUrl":LICENSE,"attribution":ATTRIBUTION,"selection":SELECTION}],
        "originalContainerLimitBytes":ORIGINAL_LIMIT,"rowGroupUncompressedLimitBytes":ROW_GROUP_LIMIT,
        "previewLimitBytes":MAX_BYTES,"previewFeatureLimit":MAX_FEATURES,"previewCoordinateLimit":super::MAX_COORDINATES}),
    )
}
fn validate_request(r: &Request) -> Result<()> {
    features::bounds(r.bounds)?;
    if r.dataset_id != "overture-maps" || r.package_id.as_deref().is_some_and(|v| v != ID) {
        return Err("Choose the reviewed Overture base/bathymetry original part".into());
    }
    Ok(())
}
fn validate_geo(geo: &Value) -> Result<()> {
    let g = &geo["columns"]["geometry"];
    let crs = g.get("crs");
    if geo["version"] != "1.1.0"
        || geo["primary_column"] != "geometry"
        || g["encoding"] != "WKB"
        || g.get("edges").is_some_and(|v| v != "planar")
        || g.get("epoch").is_some()
        || crs.is_some_and(|c| {
            !((c["id"]["authority"] == "OGC" && c["id"]["code"] == "CRS84")
                || (c["id"]["authority"] == "EPSG" && c["id"]["code"] == 4326))
        })
        || g["covering"]["bbox"]
            != json!({"xmin":["bbox","xmin"],"ymin":["bbox","ymin"],"xmax":["bbox","xmax"],"ymax":["bbox","ymax"]})
    {
        return Err("GeoParquet requires explicit WKB, planar WGS84/CRS84 metadata and the reviewed bbox covering; a null or unknown CRS is not guessed".into());
    }
    Ok(())
}
pub(crate) fn digest_file(path: &Path) -> Result<(u64, String)> {
    let mut file = File::open(path).map_err(io_error)?;
    let mut buffer = [0u8; 64 * 1024];
    let mut sha = Sha256::new();
    let mut size = 0u64;
    loop {
        let n = file.read(&mut buffer).map_err(io_error)?;
        if n == 0 {
            break;
        }
        size = size
            .checked_add(n as u64)
            .ok_or("GeoParquet size overflow")?;
        if size > ORIGINAL_LIMIT {
            return Err("Original GeoParquet exceeds its distinct 512 MiB container limit".into());
        }
        sha.update(&buffer[..n]);
    }
    Ok((size, format!("{:x}", sha.finalize())))
}
fn stored(root: &Path) -> Result<PathBuf> {
    let directory = root.join("parquet-originals");
    if directory.canonicalize().map_err(io_error)? != directory {
        return Err("GeoParquet original storage was redirected".into());
    }
    let expected = directory.join(format!("{ORIGINAL_HASH}.parquet"));
    let actual = storage::regular_file(&expected)?;
    if actual != expected {
        return Err("GeoParquet original file was redirected".into());
    }
    Ok(actual)
}
pub(crate) fn verify_saved_original(root: &Path, p: &Provenance) -> Result<PathBuf> {
    p.validate()?;
    let path = stored(root)?;
    let (bytes, sha) = digest_file(&path)?;
    if bytes != p.original_bytes || sha != p.original_sha256 {
        return Err("Complete GeoParquet original changed after saving".into());
    }
    let source = reader(&path)?;
    let (geo, schema, max) = source_metadata(&source)?;
    if geo != p.geo_metadata
        || schema != p.schema
        || max != p.max_row_group_uncompressed_bytes
        || source.metadata().num_row_groups() != p.row_groups
    {
        return Err(
            "Retained GeoParquet original metadata differs from the saved provenance".into(),
        );
    }
    Ok(path)
}
fn reader(path: &Path) -> Result<SerializedFileReader<File>> {
    let mut file = File::open(path).map_err(io_error)?;
    let size = file.metadata().map_err(io_error)?.len();
    if !(12..=ORIGINAL_LIMIT).contains(&size) {
        return Err("Invalid bounded GeoParquet original length".into());
    }
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic).map_err(io_error)?;
    file.seek(SeekFrom::End(-8)).map_err(io_error)?;
    let mut footer = [0u8; 8];
    file.read_exact(&mut footer).map_err(io_error)?;
    let footer_size = u32::from_le_bytes(footer[..4].try_into().unwrap()) as u64;
    if magic != *b"PAR1"
        || footer[4..] != *b"PAR1"
        || footer_size > FOOTER_LIMIT
        || footer_size > size - 12
    {
        return Err("GeoParquet footer is invalid or exceeds 4 MiB".into());
    }
    let r = SerializedFileReader::new(file)
        .map_err(|e| format!("Cannot read GeoParquet metadata: {e}"))?;
    let metadata = r.metadata();
    if metadata.num_row_groups() > 128
        || metadata.file_metadata().schema_descr().num_columns() > 128
        || metadata.file_metadata().num_rows() != ORIGINAL_ROWS as i64
    {
        return Err("GeoParquet row count/schema exceeds the reviewed container budget".into());
    }
    for group in metadata.row_groups() {
        if group.num_rows() <= 0
            || group.num_rows() > 100_000
            || !(1..=ROW_GROUP_LIMIT).contains(&group.total_byte_size())
            || group.columns().iter().any(|c| {
                c.compressed_size() < 0
                    || c.compressed_size() > ROW_GROUP_LIMIT
                    || c.uncompressed_size() < 0
                    || c.uncompressed_size() > ROW_GROUP_LIMIT
            })
        {
            return Err(
                "GeoParquet row group exceeds the distinct 32 MiB decoded-group/100,000-row budget"
                    .into(),
            );
        }
    }
    Ok(r)
}
fn source_metadata(r: &SerializedFileReader<File>) -> Result<(Value, String, i64)> {
    let metadata = r.metadata();
    let entries = metadata
        .file_metadata()
        .key_value_metadata()
        .ok_or("GeoParquet has no geo metadata")?;
    let values = entries
        .iter()
        .filter(|e| e.key == "geo")
        .collect::<Vec<_>>();
    if values.len() != 1 {
        return Err("GeoParquet geo metadata is absent or ambiguous".into());
    }
    let geo: Value = serde_json::from_str(
        values[0]
            .value
            .as_deref()
            .ok_or("Empty GeoParquet geo metadata")?,
    )
    .map_err(io_error)?;
    validate_geo(&geo)?;
    let schema = format!(
        "{:?}",
        metadata.file_metadata().schema_descr().root_schema()
    );
    if schema.len() > 64 * 1024 {
        return Err("GeoParquet schema exceeds its bounded metadata read".into());
    }
    let max = metadata
        .row_groups()
        .iter()
        .map(|g| g.total_byte_size())
        .max()
        .unwrap_or(0);
    Ok((geo, schema, max))
}
fn fields(row: Row) -> Result<(Value, Vec<u8>)> {
    let mut properties = serde_json::Map::new();
    let mut geometry = None;
    for (name, field) in row.into_columns() {
        if name == "geometry" {
            if let Field::Bytes(b) = field {
                geometry = Some(b.data().to_vec());
            } else {
                return Err("Original GeoParquet geometry is not a non-null WKB byte array".into());
            }
        } else {
            properties.insert(name, field.to_json_value());
        }
    }
    let properties = Value::Object(properties);
    super::validate_integer_precision(&properties)?;
    Ok((properties, geometry.ok_or("Missing original WKB geometry")?))
}
fn matches(properties: &Value, bounds: [f64; 4]) -> Result<bool> {
    let b = &properties["bbox"];
    let x = b["xmin"].as_f64().ok_or("Missing publisher bbox.xmin")?;
    let y = b["ymin"].as_f64().ok_or("Missing publisher bbox.ymin")?;
    let e = b["xmax"].as_f64().ok_or("Missing publisher bbox.xmax")?;
    let n = b["ymax"].as_f64().ok_or("Missing publisher bbox.ymax")?;
    if ![x, y, e, n].iter().all(|v| v.is_finite()) || x > e || y > n {
        return Err("Invalid original per-row bbox; selection is not guessed".into());
    }
    Ok(x <= bounds[2] && e >= bounds[0] && y <= bounds[3] && n >= bounds[1])
}
fn preview(path: &Path, bounds: [f64; 4], fetched_at: String) -> Result<(Vec<u8>, Provenance)> {
    features::bounds(bounds)?;
    let (bytes, sha) = digest_file(path)?;
    if bytes != ORIGINAL_BYTES || sha != ORIGINAL_HASH {
        return Err(
            "Overture original does not match the complete reviewed part hash/length".into(),
        );
    }
    let reader = reader(path)?;
    let (geo, schema, max) = source_metadata(&reader)?;
    let transform = proj_wkt::transform_from_crs_strings_horizontal("EPSG:4326", "EPSG:4326")
        .map_err(io_error)?;
    let mut features = Vec::new();
    let mut row_index = 0usize;
    let mut coordinates = 0usize;
    let mut output_size = 0usize;
    for i in 0..reader.metadata().num_row_groups() {
        let group = reader.get_row_group(i).map_err(io_error)?;
        for row in group.get_row_iter(None).map_err(io_error)? {
            let (properties, wkb) = fields(row.map_err(io_error)?)?;
            if matches(&properties, bounds)? {
                if features.len() >= MAX_FEATURES {
                    return Err(
                        "Selected GeoParquet preview exceeds the existing 50,000-feature limit"
                            .into(),
                    );
                }
                let geometry =
                    super::geopackage::decode_wgs84_wkb(&wkb, &transform, &mut coordinates)?;
                let f = json!({"type":"Feature","id":properties["id"],"properties":properties,"geometry":geometry,
                    "geodOriginalRow":row_index,"geodOriginalWkbSha256":hash(&wkb)});
                output_size = output_size
                    .checked_add(serde_json::to_vec(&f).map_err(io_error)?.len() + 1)
                    .ok_or("Preview size overflow")?;
                if output_size > MAX_BYTES {
                    return Err("Selected GeoParquet GeoJSON preview exceeds the unchanged 20 MiB output limit".into());
                }
                features.push(f);
            }
            row_index += 1;
        }
    }
    if row_index != ORIGINAL_ROWS {
        return Err("Original GeoParquet row scan is incomplete; no preview was registered".into());
    }
    let p = Provenance {
        conversion: 1,
        dataset_id: "overture-maps".into(),
        package_id: ID.into(),
        original_url: URL.into(),
        original_sha256: sha,
        original_bytes: bytes,
        original_rows: row_index,
        row_groups: reader.metadata().num_row_groups(),
        geo_metadata: geo,
        schema,
        max_row_group_uncompressed_bytes: max,
        requested_bounds: bounds,
        preview_feature_count: features.len(),
        selection: SELECTION.into(),
        fetched_at,
        attribution: ATTRIBUTION.into(),
        license_url: LICENSE.into(),
    };
    p.validate()?;
    let data = json!({"type":"FeatureCollection","features":features});
    let text = serde_json::to_vec(&data).map_err(io_error)?;
    if text.len() > MAX_BYTES {
        return Err("GeoParquet preview exceeds the unchanged 20 MiB vector output limit".into());
    }
    Ok((text, p))
}
impl JobManager {
    pub async fn parquet_original_verified_file(&self, id: &str) -> Result<PathBuf> {
        let asset = self.inspect_vector(id).await?.asset;
        let source = asset
            .geo_parquet
            .ok_or("This vector has no complete retained GeoParquet original")?;
        verify_saved_original(&self.inner.root, &source)
    }
    pub async fn download_official_parquet(&self, request: Request) -> Result<super::VectorAsset> {
        validate_request(&request)?;
        if request.package_id.is_none() {
            return Err("Choose the exact reviewed GeoParquet package identity".into());
        }
        let directory = self.inner.root.join("parquet-originals");
        tokio::fs::create_dir_all(&directory)
            .await
            .map_err(io_error)?;
        if directory.canonicalize().map_err(io_error)? != directory {
            return Err("Original GeoParquet storage was redirected".into());
        }
        let output = directory.join(format!("{ORIGINAL_HASH}.parquet"));
        if !output.exists() {
            let url = features::public_url(URL)?;
            let client = features::client_with_timeout(
                &url,
                &self.proxy_settings().await,
                Duration::from_secs(300),
            )
            .await?;
            let response = client
                .get(url)
                .header("Accept-Encoding", "identity")
                .send()
                .await
                .map_err(|_| "Cannot reach the official Overture original; check proxy settings")?;
            if !response.status().is_success() {
                return Err(format!(
                    "Overture original returned HTTP {}",
                    response.status().as_u16()
                ));
            }
            if response
                .content_length()
                .is_some_and(|n| n != ORIGINAL_BYTES || n > ORIGINAL_LIMIT)
            {
                return Err(
                    "Official GeoParquet original length differs from the pinned complete part"
                        .into(),
                );
            }
            let mut temporary = tempfile::NamedTempFile::new_in(&directory).map_err(io_error)?;
            let mut sha = Sha256::new();
            let mut size = 0u64;
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk =
                    chunk.map_err(|_| "Complete GeoParquet original transfer was interrupted")?;
                size += chunk.len() as u64;
                if size > ORIGINAL_LIMIT || size > ORIGINAL_BYTES {
                    return Err(
                        "Original GeoParquet exceeds its distinct container budget/pinned length"
                            .into(),
                    );
                }
                temporary.write_all(&chunk).map_err(io_error)?;
                sha.update(&chunk);
            }
            if size != ORIGINAL_BYTES || format!("{:x}", sha.finalize()) != ORIGINAL_HASH {
                return Err("Complete original GeoParquet hash or length differs; no preview was registered".into());
            }
            temporary.as_file().sync_all().map_err(io_error)?;
            match temporary.persist_noclobber(&output) {
                Ok(_) => (),
                Err(e) if output.exists() => drop(e),
                Err(e) => return Err(io_error(e)),
            }
        }
        let path = stored(&self.inner.root)?;
        let bounds = request.bounds;
        let permit = self
            .inner
            .raster_permits
            .clone()
            .acquire_owned()
            .await
            .map_err(io_error)?;
        let (text, p) = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            preview(&path, bounds, now())
        })
        .await
        .map_err(io_error)??;
        let mut title = super::initial(
            "Overture bathymetry AOI preview · original GeoParquet retained",
            "managed",
        )?;
        title.geo_parquet = Some(p);
        self.import_vector_bytes(text, title).await
    }
    pub async fn parquet_original_metadata(&self, id: &str) -> Result<Value> {
        let asset = self.inspect_vector(id).await?.asset;
        let p = asset
            .geo_parquet
            .as_ref()
            .ok_or("This vector has no retained GeoParquet original")?;
        Ok(
            json!({"assetId":id,"originalVerified":true,"original":p,"previewSubset":true,
            "previewBytes":asset.bytes,"previewSourceSha256":asset.source_sha256,"previewGeojsonSha256":asset.geojson_sha256,
            "originalContainerLimitBytes":ORIGINAL_LIMIT,"rowGroupUncompressedLimitBytes":ROW_GROUP_LIMIT,
            "previewLimitBytes":MAX_BYTES,"originalRowsTool":"/vectors/{id}/parquet/rows"}),
        )
    }
    pub async fn parquet_original_rows(&self, id: &str, args: Rows) -> Result<Value> {
        let offset = args.offset.unwrap_or(0);
        let limit = args.limit.unwrap_or(5);
        if !(1..=20).contains(&limit) || offset > ORIGINAL_ROWS {
            return Err(
                "Original Parquet rows require limit 1..20 and a valid original row offset".into(),
            );
        }
        let asset = self.inspect_vector(id).await?.asset;
        let p = asset
            .geo_parquet
            .ok_or("This vector has no retained GeoParquet original")?;
        let path = verify_saved_original(&self.inner.root, &p)?;
        let id = id.to_string();
        let permit = self
            .inner
            .raster_permits
            .clone()
            .acquire_owned()
            .await
            .map_err(io_error)?;
        tokio::task::spawn_blocking(move || {
            let _permit=permit;let reader=reader(&path)?;let mut start=0usize;let mut rows=Vec::new();let mut size=0usize;let mut stopped=false;
            for i in 0..reader.metadata().num_row_groups() {
                let count=reader.metadata().row_group(i).num_rows() as usize;
                if start+count<=offset { start+=count;continue; }
                let group=reader.get_row_group(i).map_err(io_error)?;
                for (local,row) in group.get_row_iter(None).map_err(io_error)?.enumerate() {
                    let index=start+local;if index<offset { continue; }
                    let (properties,wkb)=fields(row.map_err(io_error)?)?;
                    let r=json!({"originalRow":index,"properties":properties,"geometryEncoding":"WKB",
                        "geometryBytes":wkb.len(),"geometryWkbSha256":hash(&wkb),"geometryOmitted":true});
                    let n=serde_json::to_vec(&r).map_err(io_error)?.len();
                    if size+n>ROW_PAGE_LIMIT { if rows.is_empty() { return Err("One original property record exceeds 24 KiB; use the retained original container".into()); }stopped=true;break; }
                    rows.push(r);size+=n;if rows.len()==limit { break; }
                }
                if rows.len()==limit || stopped || size>=ROW_PAGE_LIMIT { break; }start+=count;
            }
            let end=offset+rows.len();
            Ok(json!({"assetId":id,"originalSha256":p.original_sha256,"originalVerified":true,
                "originalRows":p.original_rows,"offset":offset,"rows":rows,"nextOffset":(end<p.original_rows).then_some(end),
                "complete":end>=p.original_rows,"previewSubset":true,
                "representation":"complete original nested attributes; geometry retained as raw WKB in the independently hashed original container"}))
        }).await.map_err(io_error)?
    }
}
