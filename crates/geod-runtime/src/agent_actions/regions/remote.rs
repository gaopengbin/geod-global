use super::*;
use tokio::sync::Mutex;
const API: &str = "https://www.geoboundaries.org/api/current/gbOpen";
const MAX_BYTES: usize = 64 * 1024 * 1024;
const MAX_UNITS: usize = 100_000;
const MAX_CATALOG_UNITS: usize = 1_000_000;
static GATE: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Metadata {
    boundary_id: String,
    country_code: String,
    level: u8,
    level_name: String,
    year: String,
    original_source: String,
    license: String,
    license_source: String,
    build_date: String,
    units: usize,
    geometry_url: String,
}
impl Metadata {
    fn parse(value: &Value, iso: &str) -> Result<Self> {
        let s = |key| {
            value[key]
                .as_str()
                .map(str::trim)
                .filter(|s| text(s, 1000))
                .map(String::from)
                .ok_or(INVALID)
        };
        let kind = s("boundaryType")?;
        let level = kind
            .strip_prefix("ADM")
            .filter(|s| s.len() == 1)
            .and_then(|s| s.parse::<u8>().ok())
            .filter(|l| *l <= 5)
            .ok_or(INVALID)?;
        if value["boundaryISO"] != iso {
            return Err(INVALID.into());
        }
        let units = value["admUnitCount"]
            .as_str()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|n| *n > 0 && *n <= MAX_CATALOG_UNITS)
            .ok_or(INVALID)?;
        let supplied = s("simplifiedGeometryGeoJSON")?;
        let geometry_url = reviewed_url(&supplied, iso, level)?.to_string();
        let result = Self {
            boundary_id: s("boundaryID")?,
            country_code: iso.into(),
            level,
            level_name: value["boundaryCanonical"]
                .as_str()
                .filter(|s| text(s, 200) && *s != "nan")
                .unwrap_or(&kind)
                .into(),
            year: s("boundaryYearRepresented")?,
            original_source: s("boundarySource")?,
            license: s("boundaryLicense")?,
            license_source: value["licenseSource"]
                .as_str()
                .filter(|s| s.chars().count() <= 1000 && !s.chars().any(char::is_control))
                .unwrap_or("")
                .into(),
            build_date: s("buildDate")?,
            units,
            geometry_url,
        };
        if !text(&result.boundary_id, 120)
            || !result
                .boundary_id
                .starts_with(&format!("{iso}-ADM{level}-"))
        {
            return Err(INVALID.into());
        }
        Ok(result)
    }
    fn provenance(&self, hash: &str, checked: &str, cached: bool, geometry_units: usize) -> Value {
        json!({"provider":"geoBoundaries gbOpen","source":format!("{API}/{}/ADM{}/",self.country_code,self.level),
            "boundaryId":self.boundary_id,"countryCode":self.country_code,"adminLevel":self.level,"levelName":self.level_name,
            "dataYear":self.year,"buildDate":self.build_date,"declaredUnits":self.units,"geometryUnits":geometry_units,"unitCountMatchesMetadata":self.units==geometry_units,"originalSource":self.original_source,
            "license":self.license,"licenseSource":self.license_source,
            "attribution":"geoBoundaries and the listed original boundary source; retain the original license attribution.",
            "geometrySource":self.geometry_url,"sha256":hash,"checkedAt":checked,"cached":cached,"cacheDays":CACHE_DAYS,
            "geometry":"Source-provided simplified WGS84 boundaries; bounds are derived from every real geometry. No legal-boundary or current-year guarantee."})
    }
}
fn reviewed_url(s: &str, iso: &str, level: u8) -> Result<url::Url> {
    let url = url::Url::parse(s).map_err(|_| INVALID)?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(INVALID.into());
    }
    let path = url
        .path()
        .split('/')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    let parts = match url.host_str() {
        Some("github.com") if path.get(..3) == Some(&["wmgeolab", "geoBoundaries", "raw"]) => {
            &path[3..]
        }
        Some("media.githubusercontent.com")
            if path.get(..3) == Some(&["media", "wmgeolab", "geoBoundaries"]) =>
        {
            &path[3..]
        }
        _ => return Err(INVALID.into()),
    };
    let expected = format!("geoBoundaries-{iso}-ADM{level}_simplified.geojson");
    if parts.len() != 6
        || !(7..=40).contains(&parts[0].len())
        || !parts[0].bytes().all(|b| b.is_ascii_hexdigit())
        || parts[1] != "releaseData"
        || parts[2] != "gbOpen"
        || parts[3] != iso
        || parts[4] != format!("ADM{level}")
        || parts[5] != expected
    {
        return Err(INVALID.into());
    }
    url::Url::parse(&format!("https://media.githubusercontent.com/media/wmgeolab/geoBoundaries/{}/releaseData/gbOpen/{iso}/ADM{level}/{expected}",parts[0])).map_err(|_|INVALID.into())
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LevelCache {
    iso: String,
    checked_at: String,
    levels: Vec<Metadata>,
    #[serde(default)]
    issues: Vec<LevelIssue>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LevelIssue {
    declared_level: Option<u8>,
    reason: String,
}
fn fresh(date: &str) -> bool {
    DateTime::parse_from_rfc3339(date).is_ok_and(|date| {
        let age = Utc::now().signed_duration_since(date);
        age >= chrono::Duration::zero() && age < chrono::Duration::days(CACHE_DAYS)
    })
}
fn metadata_valid(m: &Metadata, iso: &str) -> bool {
    m.country_code == iso
        && m.level <= 5
        && m.units > 0
        && m.units <= MAX_CATALOG_UNITS
        && text(&m.boundary_id, 120)
        && m.boundary_id.starts_with(&format!("{iso}-ADM{}-", m.level))
        && [
            &m.level_name,
            &m.year,
            &m.original_source,
            &m.license,
            &m.build_date,
        ]
        .iter()
        .all(|s| text(s, 1000))
        && reviewed_url(&m.geometry_url, iso, m.level).is_ok()
}
async fn load_levels(manager: &JobManager, iso: &str) -> Result<LevelCache> {
    let id = stable_id(&format!("gbOpen-levels-v1:{iso}"));
    if let Ok(cache) = read_record::<LevelCache>(&manager.inner.root, "region-levels", &id).await {
        if cache.iso == iso
            && fresh(&cache.checked_at)
            && cache.levels.len() <= 6
            && cache.levels.len() + cache.issues.len() <= 6
            && cache.issues.iter().all(|i| {
                i.declared_level.is_none_or(|l| l <= 5)
                    && i.reason == "inconsistent-source-metadata"
            })
            && cache.levels.iter().all(|m| metadata_valid(m, iso))
            && cache
                .levels
                .iter()
                .map(|m| m.level)
                .collect::<BTreeSet<_>>()
                .len()
                == cache.levels.len()
        {
            return Ok(cache);
        }
    }
    let bytes = fetch(manager, &format!("{API}/{iso}/ALL/"), 128 * 1024).await?;
    let doc: Value = serde_json::from_slice(&bytes).map_err(|_| INVALID)?;
    let entries = doc.as_array().filter(|e| e.len() <= 6).ok_or(INVALID)?;
    // A malformed row must not disable other valid levels for that country.
    // Keep an explicit coverage issue; never repair mismatched identities by
    // guessing which boundary file the source meant to reference.
    let (mut levels, issues) = parse_levels(entries, iso);
    levels.sort_by_key(|m| m.level);
    let cache = LevelCache {
        iso: iso.into(),
        checked_at: now(),
        levels,
        issues,
    };
    write_record(&manager.inner.root, "region-levels", &id, &cache).await?;
    Ok(cache)
}
fn parse_levels(entries: &[Value], iso: &str) -> (Vec<Metadata>, Vec<LevelIssue>) {
    let mut levels = Vec::new();
    let mut issues = Vec::new();
    let mut seen = BTreeSet::new();
    for value in entries {
        match Metadata::parse(value, iso) {
            Ok(m) if seen.insert(m.level) => levels.push(m),
            _ => issues.push(LevelIssue {
                declared_level: value["boundaryType"]
                    .as_str()
                    .and_then(|s| s.strip_prefix("ADM"))
                    .and_then(|s| s.parse::<u8>().ok())
                    .filter(|l| *l <= 5),
                reason: "inconsistent-source-metadata".into(),
            }),
        }
    }
    (levels, issues)
}
pub(super) async fn levels(manager: &JobManager, iso: &str) -> Result<Value> {
    tokio::time::timeout(Duration::from_secs(45),async {
        let _guard=GATE.get_or_init(||Mutex::new(())).lock().await;
        let cache=load_levels(manager,iso).await?;
        Ok(json!({"countryCode":iso,"countryName":bundled::country_name(iso),"checkedAt":cache.checked_at,
            "provider":"geoBoundaries gbOpen","source":format!("{API}/{iso}/ALL/"),
            "coverageIssues":cache.issues,"metadataComplete":cache.issues.is_empty(),
            "availableLevels":cache.levels.iter().map(|m|json!({"adminLevel":m.level,"levelName":m.level_name,
                "units":m.units,"withinDatasetLimit":m.units<=MAX_UNITS,"dataYear":m.year,"boundaryId":m.boundary_id,"license":m.license})).collect::<Vec<_>>(),
            "next":"Query geod_region_search with the requested administrative name, this countryCode and an available adminLevel. City and county numbering differ across countries; use levelName. Missing levels are not covered by this source, not a permission failure. Use the established English or local name if the dataset has no translated aliases."}))
    }).await.map_err(|_|"Administrative lookup timed out.")?
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Page {
    id: String,
    sha256: String,
    count: usize,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Index {
    metadata: Metadata,
    checked_at: String,
    geometry_sha256: String,
    geometry_units: usize,
    pages: Vec<Page>,
}
fn sha_valid(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
async fn read_index(manager: &JobManager, iso: &str, level: u8) -> Result<(Index, Vec<Region>)> {
    let id = stable_id(&format!("gbOpen-index-v2:{iso}:{level}"));
    let index = read_record::<Index>(&manager.inner.root, "region-indexes", &id).await?;
    if !metadata_valid(&index.metadata, iso)
        || index.metadata.level != level
        || !fresh(&index.checked_at)
        || !sha_valid(&index.geometry_sha256)
        || index.geometry_units == 0
        || index.geometry_units > MAX_UNITS
        || index.pages.is_empty()
        || index.pages.len() > 200
    {
        return Err(INVALID.into());
    }
    let mut regions = Vec::new();
    let mut ids = BTreeSet::new();
    let mut page_ids = BTreeSet::new();
    for page in &index.pages {
        if !uuid(&page.id)
            || !page_ids.insert(&page.id)
            || !sha_valid(&page.sha256)
            || page.count == 0
            || page.count > 500
        {
            return Err(INVALID.into());
        }
        let entries =
            read_record::<Vec<Region>>(&manager.inner.root, "region-pages", &page.id).await?;
        let bytes = serde_json::to_vec(&entries).map_err(io_error)?;
        if format!("{:x}", Sha256::digest(&bytes)) != page.sha256 || entries.len() != page.count {
            return Err(INVALID.into());
        }
        for r in &entries {
            if !r.valid()
                || r.country_code != iso
                || r.admin_level != level
                || !ids.insert(r.id.clone())
            {
                return Err(INVALID.into());
            }
        }
        regions.extend(entries);
    }
    if regions.len() != index.geometry_units {
        return Err(INVALID.into());
    }
    Ok((index, regions))
}
async fn save_index(
    manager: &JobManager,
    metadata: Metadata,
    regions: &[Region],
    hash: String,
) -> Result<Index> {
    let id = stable_id(&format!(
        "gbOpen-index-v2:{}:{}",
        metadata.country_code, metadata.level
    ));
    let mut pages = Vec::new();
    // Fresh page IDs and manifest-last publication ensure interrupted refreshes
    // cannot pair old metadata with partially overwritten boundary pages.
    for entries in regions.chunks(500) {
        let id = Uuid::new_v4().to_string();
        let bytes = serde_json::to_vec(entries).map_err(io_error)?;
        write_record(&manager.inner.root, "region-pages", &id, &entries).await?;
        pages.push(Page {
            id,
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            count: entries.len(),
        });
    }
    let index = Index {
        metadata,
        checked_at: now(),
        geometry_sha256: hash,
        geometry_units: regions.len(),
        pages,
    };
    write_record(&manager.inner.root, "region-indexes", &id, &index).await?;
    Ok(index)
}
pub(super) async fn search(manager: &JobManager, query: Query, iso: &str) -> Result<Value> {
    tokio::time::timeout(Duration::from_secs(45),async {
        let _guard=GATE.get_or_init(||Mutex::new(())).lock().await;
        let level=query.admin_level.ok_or(INVALID)?;
        if let Ok((index,regions))=read_index(manager,iso,level).await {
            let hits=matches(&regions,&query,Some(iso));
            return Ok(result(&query,hits,index.metadata.provenance(&index.geometry_sha256,&index.checked_at,true,index.geometry_units)));
        }
        let levels=load_levels(manager,iso).await?;
        let Some(metadata)=levels.levels.into_iter().find(|m|m.level==level) else {
            return Ok(json!({"query":query.query,"countryCode":iso,"adminLevel":level,"candidates":[],"matchCount":0,
                "coverage":if levels.issues.is_empty() {"unavailable-level"} else {"unavailable-or-invalid-metadata"},"coverageIssues":levels.issues,"provider":"geoBoundaries gbOpen",
                "next":"The requested level has no usable source metadata for this country. Read geod_region_levels and its coverageIssues; preserve the requested administrative type, and do not substitute a province for a city. A separate geod_place_search city lookup may be available."}));
        };
        if metadata.units>MAX_UNITS { return Err("Administrative dataset exceeds the bounded download limit.".into()); }
        let bytes=fetch(manager,&metadata.geometry_url,MAX_BYTES).await?;
        let hash=format!("{:x}",Sha256::digest(&bytes));
        let m=metadata.clone();
        let regions=tokio::task::spawn_blocking(move || normalize(&bytes,&m)).await.map_err(|_|INVALID)??;
        let hits=matches(&regions,&query,Some(iso));
        let index=save_index(manager,metadata,&regions,hash).await?;
        Ok(result(&query,hits,index.metadata.provenance(&index.geometry_sha256,&index.checked_at,false,index.geometry_units)))
    }).await.map_err(|_|"Administrative lookup timed out.")?
}
fn normalize(bytes: &[u8], metadata: &Metadata) -> Result<Vec<Region>> {
    let doc: Value = serde_json::from_slice(bytes).map_err(|_| INVALID)?;
    if doc["type"] != "FeatureCollection"
        || doc.get("crs").is_some_and(|c| {
            c["type"] != "name"
                || ![
                    "urn:ogc:def:crs:OGC:1.3:CRS84",
                    "urn:ogc:def:crs:EPSG::4326",
                    "EPSG:4326",
                ]
                .contains(&c["properties"]["name"].as_str().unwrap_or(""))
        })
    {
        return Err(INVALID.into());
    }
    let features = doc["features"]
        .as_array()
        .filter(|f| !f.is_empty() && f.len() <= MAX_UNITS)
        .ok_or(INVALID)?;
    let mut regions = Vec::new();
    let mut ids = BTreeSet::new();
    for f in features {
        let p = &f["properties"];
        if f["type"] != "Feature"
            || p["shapeGroup"] != metadata.country_code
            || p["shapeType"] != format!("ADM{}", metadata.level)
        {
            return Err(INVALID.into());
        }
        // Some public source labels contain a trailing non-breaking space.
        // Normalize label whitespace, retaining the exact geometry byte hash.
        let name = p["shapeName"].as_str().ok_or(INVALID)?.trim().to_owned();
        let shape_id = p["shapeID"]
            .as_str()
            .filter(|s| text(s, 100))
            .ok_or(INVALID)?;
        let bounds = envelope(&f["geometry"])?;
        let r = Region {
            id: format!("gb:{shape_id}"),
            name,
            aliases: Vec::new(),
            country_code: metadata.country_code.clone(),
            country_name: bundled::country_name(&metadata.country_code),
            admin_level: metadata.level,
            bounds,
            limitation: extent_limitation(bounds),
        };
        if !r.valid() || !ids.insert(r.id.clone()) {
            return Err(INVALID.into());
        }
        regions.push(r);
    }
    Ok(regions)
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BoundaryCache {
    candidate_id: String,
    source_sha256: String,
    geometry_sha256: String,
    geometry: crate::crop::PolygonGeometry,
}
pub(super) async fn boundary(
    manager: &JobManager,
    id: &str,
    iso: &str,
    level: u8,
) -> Result<(String, crate::crop::PolygonGeometry, Value)> {
    tokio::time::timeout(Duration::from_secs(45),async {
        let _guard=GATE.get_or_init(||Mutex::new(())).lock().await;
        let (index,regions)=read_index(manager,iso,level).await.map_err(|_|"Read a fresh administrative candidate before reading its boundary.")?;
        let region=regions.iter().find(|r|r.id==id).ok_or("Use the exact administrative candidate identity.")?;
        let cache_id=stable_id(&format!("gb-boundary:{iso}:{level}:{id}:{}",index.geometry_sha256));
        let cached=read_record::<BoundaryCache>(&manager.inner.root,"region-boundaries",&cache_id).await.ok()
            .filter(|c|c.candidate_id==id && c.source_sha256==index.geometry_sha256
                && serde_json::to_vec(&c.geometry).is_ok_and(|bytes|digest(&bytes)==c.geometry_sha256)
                && c.geometry.bounds().ok()==Some(region.bounds));
        let geometry=if let Some(cache)=cached { cache.geometry } else {
            let bytes=fetch(manager,&index.metadata.geometry_url,MAX_BYTES).await?;
            if digest(&bytes)!=index.geometry_sha256 { return Err("Administrative source changed. Search the area again before selecting a boundary.".into()); }
            let doc: Value=serde_json::from_slice(&bytes).map_err(|_|INVALID)?;
            let feature=doc["features"].as_array().ok_or(INVALID)?.iter()
                .find(|f|f["properties"]["shapeID"].as_str().is_some_and(|v|format!("gb:{v}")==id)).ok_or(INVALID)?;
            if feature["properties"]["shapeGroup"]!=iso || feature["properties"]["shapeType"]!=format!("ADM{level}") { return Err(INVALID.into()); }
            let geometry: crate::crop::PolygonGeometry=serde_json::from_value(feature["geometry"].clone()).map_err(|_|INVALID)?;
            if geometry.bounds()?!=region.bounds { return Err(INVALID.into()); }
            let geometry_sha256=digest(&serde_json::to_vec(&geometry).map_err(io_error)?);
            write_record(&manager.inner.root,"region-boundaries",&cache_id,&BoundaryCache{candidate_id:id.into(),source_sha256:index.geometry_sha256.clone(),geometry_sha256,geometry:geometry.clone()}).await?;
            geometry
        };
        Ok((region.name.clone(),geometry,index.metadata.provenance(&index.geometry_sha256,&index.checked_at,true,index.geometry_units)))
    }).await.map_err(|_|"Administrative boundary lookup timed out.")?
}
async fn fetch(manager: &JobManager, address: &str, max: usize) -> Result<Vec<u8>> {
    let url = url::Url::parse(address).map_err(|_| INVALID)?;
    let client = crate::features::client_with_timeout(
        &url,
        &manager.proxy_settings().await,
        Duration::from_secs(35),
    )
    .await
    .map_err(|_| "Administrative lookup is temporarily unreachable.")?;
    let response=client.get(url).header("Accept","application/geo+json, application/json")
        .header("User-Agent","GeoD-Global/0.1.0 (user-requested administrative lookup; https://github.com/gaopengbin/geod-global)")
        .send().await.map_err(|e| if e.is_timeout() { "Administrative lookup timed out." } else { "Administrative lookup is temporarily unreachable." })?;
    if response.status().as_u16() == 404 {
        return Err("Administrative source has no dataset for this country or level.".into());
    }
    if !response.status().is_success() {
        return Err(
            if response.status().is_client_error() && response.status().as_u16() != 408 {
                "Administrative source denied the request."
            } else {
                "Administrative lookup is temporarily unreachable."
            }
            .into(),
        );
    }
    if response.content_length().is_some_and(|l| l > max as u64) {
        return Err("Administrative dataset exceeds the bounded download limit.".into());
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "Administrative lookup is temporarily unreachable.")?;
        if bytes.len() + chunk.len() > max {
            return Err("Administrative dataset exceeds the bounded download limit.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn selected_boundary_cache_survives_restart_and_rejects_wrong_candidate_scope() {
        let root = tempfile::tempdir().unwrap();
        let manager = JobManager::open(root.path()).await.unwrap();
        let doc = fixture();
        let bytes = serde_json::to_vec(&doc).unwrap();
        let hash = digest(&bytes);
        let regions = normalize(&bytes, &metadata()).unwrap();
        save_index(&manager, metadata(), &regions, hash.clone())
            .await
            .unwrap();
        let geometry: crate::crop::PolygonGeometry =
            serde_json::from_value(doc["features"][0]["geometry"].clone()).unwrap();
        let id = stable_id(&format!("gb-boundary:DEU:2:gb:test:{hash}"));
        write_record(
            &manager.inner.root,
            "region-boundaries",
            &id,
            &BoundaryCache {
                candidate_id: "gb:test".into(),
                source_sha256: hash,
                geometry_sha256: digest(&serde_json::to_vec(&geometry).unwrap()),
                geometry: geometry.clone(),
            },
        )
        .await
        .unwrap();
        manager.shutdown().await.unwrap();
        drop(manager);
        let manager = JobManager::open(root.path()).await.unwrap();
        let (name, shape, provenance) = boundary(&manager, "gb:test", "DEU", 2).await.unwrap();
        assert_eq!(name, "Köln");
        assert_eq!(shape, geometry);
        assert_eq!(provenance["license"], "Fixture");
        assert!(boundary(&manager, "gb:invented", "DEU", 2).await.is_err());
        assert!(boundary(&manager, "gb:test", "DEU", 3).await.is_err());
        assert!(manager.list().await.is_empty());
        manager.shutdown().await.unwrap();
    }
    fn metadata() -> Metadata {
        Metadata { boundary_id:"DEU-ADM2-test".into(),country_code:"DEU".into(),level:2,level_name:"Government District".into(),year:"2021".into(),original_source:"Test fixture".into(),license:"Fixture".into(),license_source:"".into(),build_date:"2023".into(),units:1,
        geometry_url:"https://media.githubusercontent.com/media/wmgeolab/geoBoundaries/9469f09/releaseData/gbOpen/DEU/ADM2/geoBoundaries-DEU-ADM2_simplified.geojson".into() }
    }
    #[test]
    fn provider_metadata_and_github_paths_follow_the_documented_schema() {
        let doc = json!({"boundaryID":"DEU-ADM2-9070358","boundaryISO":"DEU","boundaryType":"ADM2",
            "admUnitCount":"38","boundaryCanonical":"Government District","boundaryYearRepresented":"2021",
            "boundarySource":"Federal Agency for Cartography and Geodesy","boundaryLicense":"Data license Germany - Attribution - Version 2.0",
            "licenseSource":"www.govdata.de/dl-de/by-2-0","buildDate":"Dec 12, 2023",
            "simplifiedGeometryGeoJSON":"https://github.com/wmgeolab/geoBoundaries/raw/9469f09/releaseData/gbOpen/DEU/ADM2/geoBoundaries-DEU-ADM2_simplified.geojson"});
        let m = Metadata::parse(&doc, "DEU").unwrap();
        assert!(metadata_valid(&m, "DEU"));
        assert_eq!(m.units, 38);
        let mut bad = doc.clone();
        bad["boundaryID"] = json!("DEU-ADM0-19620994");
        let mut large = doc.clone();
        large["admUnitCount"] = json!("649771");
        let (levels, issues) = parse_levels(&[bad, doc], "DEU");
        assert_eq!(levels.len(), 1);
        assert_eq!(issues.len(), 1);
        assert_eq!(levels[0].level, 2);
        assert!(Metadata::parse(&large, "DEU").unwrap().units > MAX_UNITS);
    }
    fn fixture() -> Value {
        json!({"type":"FeatureCollection","features":[{"type":"Feature","properties":{"shapeGroup":"DEU","shapeType":"ADM2","shapeID":"test","shapeName":"Köln"},"geometry":{"type":"Polygon","coordinates":[[[6,50],[7,50],[7,51],[6,50]]]}}]})
    }
    #[test]
    fn rejects_wrong_identity_crs_count_geometry_and_download_origins() {
        let m = metadata();
        let f = fixture();
        assert!(normalize(&serde_json::to_vec(&f).unwrap(), &m).is_ok());
        let mut mismatched = m.clone();
        mismatched.units = 2;
        let parsed = normalize(&serde_json::to_vec(&f).unwrap(), &mismatched).unwrap();
        assert_eq!(
            mismatched.provenance(&"a".repeat(64), &now(), false, parsed.len())
                ["unitCountMatchesMetadata"],
            false
        );
        let mut spaced = f.clone();
        spaced["features"][0]["properties"]["shapeName"] = json!("Köln\u{a0}");
        assert_eq!(
            normalize(&serde_json::to_vec(&spaced).unwrap(), &m).unwrap()[0].name,
            "Köln"
        );
        for (key, value) in [
            ("shapeGroup", json!("USA")),
            ("shapeType", json!("ADM3")),
            ("shapeID", json!("")),
        ] {
            let mut bad = f.clone();
            bad["features"][0]["properties"][key] = value;
            assert!(normalize(&serde_json::to_vec(&bad).unwrap(), &m).is_err());
        }
        let mut bad = f.clone();
        bad["crs"] = json!({"type":"name","properties":{"name":"EPSG:3857"}});
        assert!(normalize(&serde_json::to_vec(&bad).unwrap(), &m).is_err());
        let mut bad = f.clone();
        bad["features"][0]["geometry"]["coordinates"][0][0][0] = json!(200);
        assert!(normalize(&serde_json::to_vec(&bad).unwrap(), &m).is_err());
        let mut bad = f.clone();
        bad["features"] = json!([]);
        assert!(normalize(&serde_json::to_vec(&bad).unwrap(), &m).is_err());
        for bad in [
            m.geometry_url
                .replace("media.githubusercontent.com", "localhost"),
            m.geometry_url.replace("/DEU/", "/USA/"),
            m.geometry_url.replace("/gbOpen/", "/gbAuthoritative/"),
            format!("{}?token=x", m.geometry_url),
        ] {
            assert!(reviewed_url(&bad, "DEU", 2).is_err());
        }
    }
    #[tokio::test]
    async fn persistent_pages_survive_reopen_and_reject_tampering_or_partial_refresh() {
        let home = tempfile::tempdir().unwrap();
        let m = JobManager::open(home.path()).await.unwrap();
        let metadata = metadata();
        let regions = normalize(&serde_json::to_vec(&fixture()).unwrap(), &metadata).unwrap();
        let index = save_index(&m, metadata, &regions, "a".repeat(64))
            .await
            .unwrap();
        m.shutdown().await.unwrap();
        drop(m);
        let reopened = JobManager::open(home.path()).await.unwrap();
        assert_eq!(
            read_index(&reopened, "DEU", 2).await.unwrap().1[0].name,
            "Köln"
        );
        let mut tampered = regions;
        tampered[0].bounds = [0., 0., 1., 1.];
        write_record(
            &reopened.inner.root,
            "region-pages",
            &index.pages[0].id,
            &tampered,
        )
        .await
        .unwrap();
        assert!(read_index(&reopened, "DEU", 2).await.is_err());
        tokio::fs::remove_file(
            reopened
                .inner
                .root
                .join("agent-region-pages")
                .join(format!("{}.json", index.pages[0].id)),
        )
        .await
        .unwrap();
        assert!(read_index(&reopened, "DEU", 2).await.is_err());
    }
}
