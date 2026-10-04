//! Explicitly configured OSM Overpass endpoints; never a default public backend.
use super::*;
#[cfg(test)]
mod tests;

const LICENSE: &str = "https://www.openstreetmap.org/copyright";
const PROBE: &str = "[out:json][timeout:5][maxsize:1048576];out count;";
static EXTRACTION: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Service {
    pub generator: String,
    pub api_version: f64,
    pub metadata_sha256: String,
    pub copyright_text: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Counts {
    pub nodes: usize,
    pub ways: usize,
    pub relations: usize,
    pub total: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Provenance {
    pub service_url: String,
    pub service_name: String,
    pub preset: String,
    pub preset_title: String,
    pub requested_bounds: [f64; 4],
    pub area_geometry: Option<crate::crop::PolygonGeometry>,
    pub requested_at: String,
    pub query: String,
    pub response_sha256: String,
    pub bytes: usize,
    pub element_counts: Counts,
    pub dependency_counts: Counts,
    pub data_timestamp: String,
    pub generator: String,
    pub api_version: f64,
    pub copyright_text: String,
    pub selection: String,
}
fn digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn endpoint(raw: &str) -> Result<Url> {
    let url = public_url(raw)?;
    if url.query().is_some() || url.as_str() != raw {
        return Err("Use a public HTTPS Overpass interpreter URL without query parameters".into());
    }
    Ok(url)
}
fn preset(id: &str) -> Result<(&'static str, &'static [&'static str])> {
    match id {
        "buildings" => Ok((
            "Buildings",
            &[
                "[building][building!=no]",
                "[\"building:part\"][\"building:part\"!=no]",
            ],
        )),
        "roads" => Ok(("Roads", &["[highway][highway!=no]"])),
        "water" => Ok((
            "Water",
            &[
                "[natural=water]",
                "[waterway][waterway!=no]",
                "[landuse=reservoir]",
                "[landuse=basin]",
            ],
        )),
        "landuse" => Ok(("Land use", &["[landuse][landuse!=no]"])),
        "pois" => Ok((
            "Points of interest",
            &[
                "[amenity][amenity!=no]",
                "[shop][shop!=no]",
                "[tourism][tourism!=no]",
                "[leisure][leisure!=no]",
            ],
        )),
        _ => Err("Unknown OSM preset".into()),
    }
}
pub(super) fn query_bounds(b: [f64; 4]) -> Result<()> {
    bounds(b)?;
    let area = 6371.0088_f64.powi(2)
        * (b[2] - b[0]).to_radians()
        * (b[3].to_radians().sin() - b[1].to_radians().sin());
    if b[2] - b[0] > 1. || b[3] - b[1] > 1. || area > 100. {
        return Err("OSM extraction supports regions up to 100 square kilometres and one degree per side; use a regional extract for larger areas".into());
    }
    Ok(())
}
fn query_text(id: &str, b: [f64; 4]) -> Result<String> {
    query_bounds(b)?;
    let (_, filters) = preset(id)?;
    let bbox = [b[1], b[0], b[3], b[2]]
        .iter()
        .map(f64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let selectors = filters
        .iter()
        .map(|filter| format!("nwr{filter}({bbox});"))
        .collect::<String>();
    Ok(format!("[out:json][timeout:25][maxsize:16777216];({selectors})->.selected;.selected out meta geom;.selected out count;.selected >> ->.dependencies;(.dependencies; - .selected;)->.dependencies;.dependencies out meta geom;.dependencies out count;"))
}
fn collections(url: &Url) -> Vec<Collection> {
    ["buildings", "roads", "water", "landuse", "pois"].into_iter().map(|id| Collection {
        id: id.into(), title: preset(id).unwrap().0.into(),
        description: "OSM bounding-box selection with complete geometry and recursive members; not an exact polygon clip.".into(),
        items_url: url.to_string(), license_links: vec![LICENSE.into()], arcgis: None, wfs: None,
    }).collect()
}
fn identity(raw: &Value) -> Result<(String, f64, String, String)> {
    if !raw.is_object()
        || raw.get("error").is_some()
        || raw.get("remark").is_some_and(|v| v.as_str() != Some(""))
    {
        return Err(
            "Overpass reported incomplete or failed processing; no partial file was saved".into(),
        );
    }
    let generator = raw["generator"]
        .as_str()
        .filter(|s| clean(s, 256) && s.starts_with("Overpass API"))
        .ok_or("The endpoint did not return an Overpass response")?;
    let version = raw["version"]
        .as_f64()
        .filter(|v| *v == 0.6)
        .ok_or("Unsupported Overpass API format version")?;
    let at = raw["osm3s"]["timestamp_osm_base"]
        .as_str()
        .ok_or("Overpass response has no dataset timestamp")?;
    chrono::DateTime::parse_from_rfc3339(at).map_err(|_| "Invalid Overpass dataset timestamp")?;
    let copyright = raw["osm3s"]["copyright"]
        .as_str()
        .filter(|s| s.len() <= 16384 && s.contains("openstreetmap.org") && s.contains("ODbL"))
        .ok_or("The endpoint did not declare OpenStreetMap data and ODbL attribution")?;
    Ok((generator.into(), version, at.into(), copyright.into()))
}
fn count_value(v: &Value) -> Result<usize> {
    v.as_str()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|n| *n <= vector::MAX_FEATURES)
        .ok_or("Invalid Overpass completion count".into())
}
fn counts(value: &Value) -> Result<Counts> {
    if value["type"] != "count"
        || value["id"] != 0
        || value["tags"].as_object().is_none_or(|v| v.len() != 4)
    {
        return Err("Overpass response is missing its completion count".into());
    }
    let t = &value["tags"];
    let result = Counts {
        nodes: count_value(&t["nodes"])?,
        ways: count_value(&t["ways"])?,
        relations: count_value(&t["relations"])?,
        total: count_value(&t["total"])?,
    };
    validate_counts(&result)?;
    Ok(result)
}
fn validate_counts(c: &Counts) -> Result<()> {
    if [c.nodes, c.ways, c.relations, c.total]
        .iter()
        .any(|n| *n > vector::MAX_FEATURES)
        || c.nodes + c.ways + c.relations != c.total
    {
        return Err("Overpass element counts do not match".into());
    }
    Ok(())
}
fn actual_counts(elements: &[Value], seen: &mut BTreeSet<String>) -> Result<Counts> {
    let mut result = Counts::default();
    for element in elements {
        let kind = element["type"]
            .as_str()
            .ok_or("Invalid Overpass element type")?;
        let id = element["id"]
            .as_u64()
            .filter(|id| *id > 0 && *id <= 9_007_199_254_740_991)
            .ok_or("Invalid Overpass object ID")?;
        if !seen.insert(format!("{kind}/{id}")) {
            return Err("Overpass repeated an element identity".into());
        }
        match kind {
            "node" => result.nodes += 1,
            "way" => result.ways += 1,
            "relation" => result.relations += 1,
            _ => return Err("Invalid Overpass element type".into()),
        }
        result.total += 1;
    }
    validate_counts(&result)?;
    Ok(result)
}
fn split_response(raw: &Value) -> Result<(Value, Counts, Counts)> {
    identity(raw)?;
    let elements = raw["elements"]
        .as_array()
        .filter(|a| a.len() <= vector::MAX_FEATURES + 2)
        .ok_or("Overpass response exceeds 50,000 elements")?;
    let markers = elements
        .iter()
        .enumerate()
        .filter_map(|(i, e)| (e["type"] == "count").then_some(i))
        .collect::<Vec<_>>();
    if markers.len() != 2 || markers[1] + 1 != elements.len() {
        return Err("Overpass response is incomplete; both completion counts are required".into());
    }
    let selected = counts(&elements[markers[0]])?;
    let dependencies = counts(&elements[markers[1]])?;
    let mut seen = BTreeSet::new();
    if actual_counts(&elements[..markers[0]], &mut seen)? != selected
        || actual_counts(&elements[markers[0] + 1..markers[1]], &mut seen)? != dependencies
    {
        return Err("Overpass response contains incomplete element groups".into());
    }
    // The request explicitly fetched recursive dependencies, so their absence
    // cannot be interpreted as an untagged or dispensable map object.
    for element in elements {
        let refs: Vec<(String, &Value)> = match element["type"].as_str() {
            Some("way") => element["nodes"]
                .as_array()
                .ok_or("Overpass way has no node references")?
                .iter()
                .map(|id| ("node".into(), id))
                .collect(),
            Some("relation") => element["members"]
                .as_array()
                .ok_or("Overpass relation has no member references")?
                .iter()
                .map(|m| (m["type"].as_str().unwrap_or("").into(), &m["ref"]))
                .collect(),
            _ => Vec::new(),
        };
        for (kind, value) in refs {
            let id = value
                .as_u64()
                .filter(|id| *id > 0 && *id <= 9_007_199_254_740_991)
                .ok_or("Invalid Overpass dependency ID")?;
            if !seen.contains(&format!("{kind}/{id}")) {
                return Err(
                    "Overpass omitted a recursive member dependency; no partial file was saved"
                        .into(),
                );
            }
        }
    }
    let mut normalized = raw.clone();
    normalized["elements"] = Value::Array(
        elements
            .iter()
            .filter(|e| e["type"] != "count")
            .cloned()
            .collect(),
    );
    Ok((normalized, selected, dependencies))
}
async fn request(
    url: &Url,
    settings: &crate::ProxySettings,
    query: &str,
    limit: usize,
) -> Result<Vec<u8>> {
    let client = client(url, settings).await?;
    let response = client
        .post(url.clone())
        .header("Accept", "application/json")
        .header(
            "User-Agent",
            "GeoD-Global/0.1 (user-configured OSM extraction)",
        )
        .form(&[("data", query)])
        .send()
        .await
        .map_err(|_| "Cannot reach Overpass; check the endpoint and proxy settings")?;
    if !response.status().is_success() {
        return Err(match response.status().as_u16() {
            429 => "Overpass is busy or rate limited; wait before trying again. No automatic retry was started.".into(),
            504 => "Overpass could not finish this query; use a smaller area or retry later.".into(),
            status => format!("Overpass returned HTTP {status}"),
        });
    }
    if response
        .headers()
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        != Some("application/json")
    {
        return Err("Overpass did not return JSON".into());
    }
    if response.content_length().is_some_and(|n| n > limit as u64) {
        return Err("Overpass response exceeds the extraction limit".into());
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "Overpass response was interrupted")?;
        if bytes.len() + chunk.len() > limit {
            return Err("Overpass response exceeds the extraction limit".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
pub(super) async fn discover(
    requested: ConnectRequest,
    settings: crate::ProxySettings,
) -> Result<FeatureService> {
    let name = requested.name.trim();
    if !clean(name, 80) {
        return Err("Enter a data service name of 1–80 characters".into());
    }
    let url = endpoint(requested.url.trim())?;
    let bytes = request(&url, &settings, PROBE, 1024 * 1024).await?;
    let raw: Value = serde_json::from_slice(&bytes).map_err(|_| "Invalid Overpass JSON")?;
    let (generator, api_version, _, copyright_text) = identity(&raw)?;
    let elements = raw["elements"]
        .as_array()
        .filter(|a| a.len() == 1)
        .ok_or("Invalid Overpass connection probe")?;
    if counts(&elements[0])? != Counts::default() {
        return Err("Invalid Overpass connection probe".into());
    }
    Ok(FeatureService {
        id: Uuid::new_v4().to_string(),
        name: name.into(),
        title: name.into(),
        url: url.to_string(),
        collections: collections(&url),
        connected_at: now(),
        arcgis: None,
        wfs: None,
        overpass: Some(Service {
            generator,
            api_version,
            metadata_sha256: hash(&bytes),
            copyright_text,
        }),
    })
}
pub(super) fn validate_service(service: &FeatureService) -> Result<()> {
    let url = endpoint(&service.url)?;
    let meta = service
        .overpass
        .as_ref()
        .ok_or("Missing Overpass connection metadata")?;
    if service.wfs.is_some()
        || service.arcgis.is_some()
        || service.collections != collections(&url)
        || !clean(&meta.generator, 256)
        || !meta.generator.starts_with("Overpass API")
        || meta.api_version != 0.6
        || !digest(&meta.metadata_sha256)
        || meta.copyright_text.len() > 16384
        || !meta.copyright_text.contains("openstreetmap.org")
        || !meta.copyright_text.contains("ODbL")
    {
        return Err("Invalid saved Overpass connection".into());
    }
    Ok(())
}
impl Provenance {
    pub(crate) fn validate(&self, count: usize) -> Result<()> {
        endpoint(&self.service_url)?;
        query_bounds(self.requested_bounds)?;
        validate_counts(&self.element_counts)?;
        validate_counts(&self.dependency_counts)?;
        if !clean(&self.service_name, 80)
            || self.preset_title != preset(&self.preset)?.0
            || self.query != query_text(&self.preset, self.requested_bounds)?
            || !digest(&self.response_sha256)
            || self.bytes == 0
            || self.bytes > vector::MAX_BYTES
            || self.element_counts.total != count
            || self.element_counts.total + self.dependency_counts.total > vector::MAX_FEATURES
            || self.selection != "overpass-bbox-full-geometry"
            || self.api_version != 0.6
            || !clean(&self.generator, 256)
            || !self.generator.starts_with("Overpass API")
            || self.copyright_text.len() > 16384
            || !self.copyright_text.contains("openstreetmap.org")
            || !self.copyright_text.contains("ODbL")
            || chrono::DateTime::parse_from_rfc3339(&self.requested_at).is_err()
            || chrono::DateTime::parse_from_rfc3339(&self.data_timestamp).is_err()
        {
            return Err("Invalid OSM query provenance".into());
        }
        if let Some(area) = &self.area_geometry {
            validate_area(area, self.requested_bounds)?;
        }
        Ok(())
    }
    pub(crate) fn validate_raw(&self, bytes: &[u8], raw: &Value) -> Result<Value> {
        self.validate(self.element_counts.total)?;
        let (normalized, selected, dependencies) = split_response(raw)?;
        let (generator, version, timestamp, copyright) = identity(raw)?;
        if bytes.len() != self.bytes
            || hash(bytes) != self.response_sha256
            || selected != self.element_counts
            || dependencies != self.dependency_counts
            || generator != self.generator
            || version != self.api_version
            || timestamp != self.data_timestamp
            || copyright != self.copyright_text
        {
            return Err("OSM content differs from its recorded query provenance".into());
        }
        Ok(normalized)
    }
}
pub(super) async fn query(
    service: FeatureService,
    requested: QueryRequest,
    settings: crate::ProxySettings,
) -> Result<(Vec<u8>, Provenance)> {
    let _permit = EXTRACTION
        .try_lock()
        .map_err(|_| "An OSM extraction is already running; wait for it to finish")?;
    validate_service(&service)?;
    let url = endpoint(&service.url)?;
    if requested.page_size.is_some() {
        return Err("Overpass extraction does not use page sizes".into());
    }
    let query = query_text(&requested.collection_id, requested.bounds)?;
    let at = now();
    let bytes = request(&url, &settings, &query, vector::MAX_BYTES).await?;
    let raw: Value = serde_json::from_slice(&bytes).map_err(|_| "Invalid Overpass JSON")?;
    let (_, selected, dependencies) = split_response(&raw)?;
    let (generator, api_version, data_timestamp, copyright_text) = identity(&raw)?;
    let source = Provenance {
        service_url: service.url,
        service_name: service.name,
        preset: requested.collection_id.clone(),
        preset_title: preset(&requested.collection_id)?.0.into(),
        requested_bounds: requested.bounds,
        area_geometry: requested.area_geometry,
        requested_at: at,
        query,
        response_sha256: hash(&bytes),
        bytes: bytes.len(),
        element_counts: selected,
        dependency_counts: dependencies,
        data_timestamp,
        generator,
        api_version,
        copyright_text,
        selection: "overpass-bbox-full-geometry".into(),
    };
    source.validate(source.element_counts.total)?;
    Ok((bytes, source))
}
