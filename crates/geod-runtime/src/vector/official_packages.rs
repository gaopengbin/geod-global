//! Reviewed publisher originals. The caller selects a fixed catalog identity,
//! never an arbitrary remote URL. Whole gzip GeoJSONL and Shapefile ZIP bytes
//! are retained; the map representation is a separately hashed conversion.
use super::{hash, io_error, Result, MAX_BYTES, MAX_FEATURES};
use crate::{features, now, JobManager};
use flate2::read::MultiGzDecoder;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{io::Read, sync::LazyLock, time::Duration};

#[cfg(test)]
mod tests;

const MS_RELEASE: &str = "2026-08-13";
const MS_INDEX: &str = "https://bfppub.blob.core.windows.net/%24web/2026-08-13/dataset-links.csv";
const MS_DOC: &str = "https://github.com/microsoft/GlobalMLBuildingFootprints";
const MS_LICENSE: &str = "https://cdla.dev/permissive-2-0/";
const GEO_DOC: &str = "https://download.geofabrik.de/europe/monaco.html";
const GEO_URL: &str = "https://download.geofabrik.de/europe/monaco-261011-free.shp.zip";
const GEO_HASH: &str = "33611b0f8fd1fbc6562ce6a2c93b35371d26da9e38f3015a889fb9e527e08ccd";

/// One tuple per complete row in the publisher's pinned CSV. The part name is
/// retained verbatim; no quadkey URL or absent country is invented.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MicrosoftIndex {
    source_sha256: String,
    rows: Vec<(String, String, String, String)>,
}
static MS: LazyLock<MicrosoftIndex> = LazyLock::new(|| {
    serde_json::from_str(include_str!(
        "official_packages/ms-buildings-reviewed-index.json"
    ))
    .expect("reviewed complete Microsoft 2026-08-13 publisher index")
});

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Package {
    pub dataset_id: String,
    pub id: String,
    pub title: String,
    pub format: String,
    pub url: String,
    pub file_name: String,
    pub release: String,
    pub bounds: [f64; 4],
    pub source_index_url: String,
    pub source_index_sha256: Option<String>,
    pub publisher_size: String,
    pub expected_sha256: Option<String>,
    pub documentation_url: String,
    pub attribution: String,
    pub license_url: String,
    pub data_timestamp: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Provenance {
    pub conversion: u32,
    pub package: Package,
    pub requested_bounds: [f64; 4],
    pub fetched_at: String,
    pub bytes: usize,
    pub sha256: String,
    pub selection: String,
}
impl Provenance {
    pub fn validate(&self) -> Result<()> {
        features::bounds(self.requested_bounds)?;
        if self.conversion != 1
            || package(&self.package.dataset_id, &self.package.id)? != self.package
            || !intersects(self.package.bounds, self.requested_bounds)
            || self.bytes == 0
            || self.bytes > MAX_BYTES
            || self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|v| v.is_ascii_hexdigit() && !v.is_ascii_uppercase())
            || self
                .package
                .expected_sha256
                .as_ref()
                .is_some_and(|h| h != &self.sha256)
            || chrono::DateTime::parse_from_rfc3339(&self.fetched_at).is_err()
            || self.selection
                != "complete publisher original; area selects a package, not a geometry clip"
        {
            return Err("Invalid official vector package provenance".into());
        }
        Ok(())
    }
    pub(super) fn validate_original(&self, bytes: &[u8]) -> Result<()> {
        self.validate()?;
        if self.bytes != bytes.len() || self.sha256 != hash(bytes) {
            return Err("Official vector original differs from its retained receipt".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchRequest {
    pub dataset_id: String,
    pub bounds: [f64; 4],
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DownloadRequest {
    pub dataset_id: String,
    pub package_id: String,
    pub bounds: [f64; 4],
}

fn intersects(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] < b[2] && a[2] > b[0] && a[1] < b[3] && a[3] > b[1]
}
fn quad_bounds(key: &str) -> Result<[f64; 4]> {
    if key.len() != 9 || !key.bytes().all(|c| matches!(c, b'0'..=b'3')) {
        return Err("Invalid publisher Microsoft LOD9 quadkey".into());
    }
    let (mut x, mut y) = (0u32, 0u32);
    for c in key.bytes() {
        x = x * 2 + u32::from(c == b'1' || c == b'3');
        y = y * 2 + u32::from(c == b'2' || c == b'3');
    }
    let lat = |y: u32| {
        (std::f64::consts::PI * (1. - 2. * y as f64 / 512.))
            .sinh()
            .atan()
            .to_degrees()
    };
    Ok([
        x as f64 / 512. * 360. - 180.,
        lat(y + 1),
        (x + 1) as f64 / 512. * 360. - 180.,
        lat(y),
    ])
}
fn microsoft(row: &(String, String, String, String)) -> Result<Package> {
    let (country, quadkey, part, size) = row;
    if country.is_empty()
        || !country
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        || !part.starts_with("part-")
        || !part.ends_with(".c000.csv.gz")
        || !part
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
    {
        return Err("Invalid reviewed Microsoft publisher row".into());
    }
    Ok(Package {
        dataset_id: "ms-buildings".into(), id: format!("{country}-{quadkey}"),
        title: format!("Microsoft buildings · {country} · {quadkey}"),
        format: "geojsonl-gzip".into(),
        url: format!("https://bfppub.z5.web.core.windows.net/{MS_RELEASE}/global-buildings.geojsonl/RegionName={country}/quadkey={quadkey}/{part}"),
        file_name: format!("Microsoft-{country}-{quadkey}-{MS_RELEASE}.csv.gz"),
        release: MS_RELEASE.into(), bounds: quad_bounds(quadkey)?,
        source_index_url: MS_INDEX.into(), source_index_sha256: Some(MS.source_sha256.clone()),
        publisher_size: size.clone(), expected_sha256: None,
        documentation_url: MS_DOC.into(), attribution: "Microsoft Global ML Building Footprints; retain Microsoft, the selected country, LOD9 quadkey, original URL, pinned 2026-08-13 index and CDLA Permissive 2.0 notice.".into(),
        license_url: MS_LICENSE.into(), data_timestamp: None,
        warnings: vec![
            "This is a complete country/LOD9 gzip GeoJSONL partition. The selected area is reference only; no footprint is clipped, simplified, filtered or repaired. A quadkey bounds match does not prove a building exists in the selected area.".into(),
            "2026-08-13 is the fixed publisher distribution/index version, not the acquisition date of every building. Input imagery dates, coverage and model quality vary; these footprints are not legal cadastral boundaries or proof of current buildings.".into(),
            "Height is a model estimate in metres above ground; -1 means no height estimate. Confidence describes the footprint and -1 is the publisher's absent/legacy score. Original numbers, including these sentinels, are preserved without converting them to measured height or replacing them with zero.".into(),
            "Publisher Size is a rounded download estimate. Each full original and decompressed/converted data remain limited to 20 MiB, 50,000 features and 500,000 coordinates; large intersecting partitions can fail without registration or a partial substitute.".into(),
        ],
    })
}
fn geofabrik() -> Package {
    Package {
        dataset_id: "geofabrik".into(), id: "monaco-261011-free-shp".into(),
        title: "Geofabrik Monaco · complete free GIS Shapefile bundle · 2026-10-11".into(),
        format: "shapefile".into(), url: GEO_URL.into(), file_name: "monaco-261011-free.shp.zip".into(),
        release: "2026-10-11".into(), bounds: [7.4016855,43.5165358,7.5329917,43.7543687],
        source_index_url: GEO_DOC.into(), source_index_sha256: None,
        publisher_size: "746384 bytes".into(), expected_sha256: Some(GEO_HASH.into()),
        documentation_url: "https://download.geofabrik.de/osm-data-in-gis-formats-free.pdf".into(),
        attribution: "© OpenStreetMap contributors; Geofabrik GmbH regional free GIS-format extract. Retain ODbL 1.0, original bundle README, source timestamp and Geofabrik format documentation.".into(),
        license_url: "https://opendatacommons.org/licenses/odbl/1-0/".into(),
        data_timestamp: Some("2026-10-11T20:21:06Z".into()),
        warnings: vec![
            "Only the pinned complete Monaco free GIS Shapefile ZIP is connected. Its 20 published thematic layers are retained with original members and DBF properties; this is Geofabrik's derived free GIS bundle, not raw OSM PBF, all OSM tags, extended paid shapefiles or a national download guarantee.".into(),
            "Original layer geometries can extend beyond Monaco and the selected area. The union bounds are the checked original feature bounds, not a legal country border, and the package is never clipped to the selected map area.".into(),
            "The complete Monaco raw PBF was tested and rejected at relation/8654's 1,024-member native bound. Existing parser limits remain unchanged; the Shapefile format is an explicit selection rather than a silent PBF fallback.".into(),
        ],
    }
}
pub fn package(dataset_id: &str, id: &str) -> Result<Package> {
    match dataset_id {
        "ms-buildings" => MS
            .rows
            .iter()
            .find(|r| format!("{}-{}", r.0, r.1) == id)
            .ok_or_else(|| "Unknown package in the pinned Microsoft publisher index".to_string())
            .and_then(microsoft),
        "geofabrik" if id == geofabrik().id => Ok(geofabrik()),
        _ => Err("Unknown reviewed official vector package".into()),
    }
}
pub fn providers() -> Value {
    json!([
        {"id":"ms-buildings","title":"Microsoft Global Building Footprints","version":MS_RELEASE,
         "documentationUrl":MS_DOC,"license":"CDLA Permissive 2.0","licenseUrl":MS_LICENSE,
         "discovery":"official-vector-packages","formats":["geojsonl-gzip"],"packageCount":MS.rows.len(),
         "warnings":microsoft(&MS.rows[0]).expect("reviewed index").warnings},
        {"id":"geofabrik","title":"Geofabrik · Monaco regional free GIS bundle","version":"2026-10-11",
         "documentationUrl":GEO_DOC,"license":"ODbL 1.0","licenseUrl":geofabrik().license_url,
         "discovery":"official-vector-packages","formats":["shapefile"],"packageCount":1,
         "warnings":geofabrik().warnings}
    ])
}
pub fn search(request: SearchRequest) -> Result<Value> {
    features::bounds(request.bounds)?;
    let candidates = match request.dataset_id.as_str() {
        "ms-buildings" => MS.rows.iter().map(microsoft).collect::<Result<Vec<_>>>()?,
        "geofabrik" => vec![geofabrik()],
        _ => return Err("Choose a reviewed official vector dataset".into()),
    };
    let items = candidates
        .into_iter()
        .filter(|p| intersects(p.bounds, request.bounds))
        .collect::<Vec<_>>();
    if items.len() > 8 {
        return Err(
            "This area intersects more than 8 complete vector packages; narrow the map area".into(),
        );
    }
    Ok(
        json!({"datasetId":request.dataset_id,"bounds":request.bounds,"items":items,
        "complete":true,"reviewedIndexOnly":true,"liveAvailabilityChecked":false,
        "selection":"complete publisher original; area selects a package, not a geometry clip"}),
    )
}
pub(super) fn decode_gzip_jsonl(bytes: &[u8]) -> Result<Value> {
    if bytes.is_empty() || bytes.len() > MAX_BYTES || !bytes.starts_with(&[0x1f, 0x8b]) {
        return Err("Choose a complete gzip GeoJSONL original of at most 20 MiB".into());
    }
    let mut expanded = Vec::new();
    MultiGzDecoder::new(bytes)
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut expanded)
        .map_err(|_| "Original gzip GeoJSONL failed CRC, length or decompression verification")?;
    if expanded.len() > MAX_BYTES {
        return Err("Decompressed gzip GeoJSONL exceeds 20 MiB".into());
    }
    let text = std::str::from_utf8(&expanded).map_err(|_| "Original GeoJSONL must be UTF-8")?;
    let mut features = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() || features.len() >= MAX_FEATURES {
            return Err("GeoJSONL has an empty record or exceeds 50,000 complete features".into());
        }
        let feature: Value = serde_json::from_str(line)
            .map_err(|e| format!("Invalid original GeoJSONL record: {e}"))?;
        if feature["type"] != "Feature" {
            return Err("Every original GeoJSONL record must be a GeoJSON Feature".into());
        }
        features.push(feature);
    }
    if features.is_empty() {
        return Err("The gzip GeoJSONL original contains no complete features".into());
    }
    let data = json!({"type":"FeatureCollection","features":features});
    if serde_json::to_vec(&data).map_err(io_error)?.len() > MAX_BYTES {
        return Err("Converted complete GeoJSONL exceeds the native 20 MiB vector limit".into());
    }
    Ok(data)
}

impl JobManager {
    pub async fn download_official_vector_package(
        &self,
        request: DownloadRequest,
    ) -> Result<super::VectorAsset> {
        features::bounds(request.bounds)?;
        let source = package(&request.dataset_id, &request.package_id)?;
        if !intersects(source.bounds, request.bounds) {
            return Err("The selected package does not intersect the current map area".into());
        }
        let url = features::public_url(&source.url)?;
        let client = features::client_with_timeout(
            &url,
            &self.proxy_settings().await,
            Duration::from_secs(120),
        )
        .await?;
        let work = async {
            let response = client
                .get(url)
                .header("Accept-Encoding", "identity")
                .send()
                .await
                .map_err(|_| {
                    "Cannot reach the official vector publisher; check source and proxy settings"
                })?;
            if !response.status().is_success() {
                return Err(format!(
                    "Official vector publisher returned HTTP {}",
                    response.status().as_u16()
                ));
            }
            if response
                .content_length()
                .is_some_and(|n| n > MAX_BYTES as u64)
            {
                return Err(
                    "Complete official vector package exceeds 20 MiB; no file was registered"
                        .into(),
                );
            }
            let mut bytes = Vec::new();
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk =
                    chunk.map_err(|_| "Official vector original transfer was interrupted")?;
                if bytes.len().saturating_add(chunk.len()) > MAX_BYTES {
                    return Err(
                        "Complete official vector package exceeds 20 MiB; no file was registered"
                            .into(),
                    );
                }
                bytes.extend_from_slice(&chunk);
            }
            if source
                .expected_sha256
                .as_ref()
                .is_some_and(|h| *h != hash(&bytes))
            {
                return Err(
                    "Pinned official vector original hash changed; no file was registered".into(),
                );
            }
            let receipt = Provenance {
                conversion: 1,
                package: source.clone(),
                requested_bounds: request.bounds,
                fetched_at: now(),
                bytes: bytes.len(),
                sha256: hash(&bytes),
                selection:
                    "complete publisher original; area selects a package, not a geometry clip"
                        .into(),
            };
            receipt.validate_original(&bytes)?;
            let mut title = super::initial(&source.file_name, "managed")?;
            title.official_package = Some(receipt);
            self.import_vector_bytes(bytes, title).await
        };
        tokio::time::timeout(Duration::from_secs(150), work)
            .await
            .map_err(|_| "Official vector package timed out; retry the complete original")?
    }
}
