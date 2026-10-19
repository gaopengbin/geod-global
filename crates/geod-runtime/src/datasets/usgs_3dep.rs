//! TNM's real Current 1-arc-second catalog records, not publisher STAC.
//! Query bytes are archived by the caller. No date is invented for the DEM.
use super::*;
use serde_json::Value;
use std::collections::BTreeSet;
use url::Url;

pub(crate) const DATASET_ID: &str = "usgs-3dep";
const API: &str = "https://tnmaccess.nationalmap.gov/api/v1/products";
const TAG: &str = "National Elevation Dataset (NED) 1 arc-second Current";
const MAX_RECORDS: usize = 100;
const MAX_BYTES: u64 = 512 * 1024 * 1024;

pub(super) fn definition() -> Dataset {
    let mut value = base(
        DATASET_ID,
        "USGS 3DEP · Current 1 arc-second elevation",
        "elevation",
        "https://www.usgs.gov/3d-elevation-program/about-3dep-products-services",
    );
    value.description = "Current standard bare-earth elevation DEM tiles discovered through the public USGS National Map catalog and downloaded from its exact public AWS original-file URL. This is a regional elevation product, not a global optical scene or ERS account download.".into();
    value.version = "TNM Current 1 arc-second DEM · selected record publication retained".into();
    value.license = "Public domain · USGS 3DEP".into();
    value.license_url = value.documentation_url.clone();
    value.attribution = "U.S. Geological Survey, 3D Elevation Program / The National Map. Retain the actual ScienceBase source ID, publication date, vendor metadata, catalog receipt and access date.".into();
    value.nominal_resolution =
        "1 arc-second (approximately 30 m); preserve the original NAD83 geographic grid".into();
    value.unit = "metres; publisher vertical reference".into();
    value.value_interpretation = "Original signed bare-earth elevations. CONUS heights reference NAVD88; the vertical reference can vary elsewhere and must be checked against the selected publisher metadata. Negative or zero elevations are valid unless the original embedded TIFF mask says otherwise. No calibration, datum conversion, resampling or AOI clipping is applied.".into();
    value.layers = vec![layer(
        "elevation",
        "Current bare-earth elevation",
        "m",
        &value.value_interpretation,
    )];
    value.default_layer = Some("elevation".into());
    value.discovery = Some("catalog-raster".into());
    value.warnings = vec![
        "The Current catalog selects published DEM releases, not imagery acquisition years. Publication date remains in the archived raw TNM record; no observation datetime is synthesized. Source surveys may span many decades.".into(),
        "The read-only public TNM query uses the requested map bounds. Every returned record is checked and locally filtered by its actual declared bounding box; catalog geometry intersection is not a guarantee of valid elevation coverage.".into(),
        "Each complete original GeoTIFF tile is downloaded, including its border overlap, without clipping. Bbox match is neither pixel-level coverage nor terrain accuracy. The publisher's vendor XML link and original API record are retained separately from local preset declarations.".into(),
        "Only the exact versioned public prd-tnm AWS 1-arc-second TIFF paths declared by the Current catalog are accepted. No ERS credential, requester-pays bucket, thumbnail, temporary export or guessed alternate URL is used.".into(),
        "The original horizontal CRS is NAD83 geographic (EPSG:4269), not WGS84. Map placement does not modify the original or constitute survey-grade horizontal/vertical datum conversion. Native raster inspection confirms the actual embedded CRS and NoData independently.".into(),
        "The separate project-based 1-m DEM, seamless 1-m DEM and historical releases are not substituted. The tested project 1-m API tag has no Current counterpart in the live catalog.".into(),
    ];
    value.limits = vec![
        "One complete Current query is bounded to 100 records; incomplete/paginated responses are rejected with a request to narrow the area. At most 8 actual intersecting originals may be selected; none are silently truncated.".into(),
        "Each original retains the 512 MiB transfer cap. Native tiled TIFF axes are bounded to 131,072 and 8 billion pixels per original; metadata discovery alone is not proof of successful native decoding.".into(),
    ];
    value
}

fn normalized(request: &ResolveRequest) -> Result<ResolveRequest> {
    features::bounds(request.bounds)?;
    if request.dataset_id != DATASET_ID
        || request.year.is_some()
        || request.month.is_some()
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
    {
        return Err("USGS 3DEP Current uses geographic bounds and an elevation layer, not year, month, country, region or season filters".into());
    }
    if request
        .layer
        .as_deref()
        .is_some_and(|layer| layer != "elevation")
    {
        return Err("Choose the Current 1-arc-second elevation layer".into());
    }
    let mut value = request.clone();
    value.layer = Some("elevation".into());
    Ok(value)
}

pub(crate) fn query(request: &ResolveRequest) -> Result<String> {
    let request = normalized(request)?;
    let mut url = Url::parse(API).map_err(crate::io_error)?;
    let bounds = request.bounds.map(|value| value.to_string()).join(",");
    url.query_pairs_mut()
        .append_pair("datasets", TAG)
        .append_pair("bbox", &bounds)
        .append_pair("max", &MAX_RECORDS.to_string())
        .append_pair("offset", "0")
        .append_pair("outputFormat", "JSON");
    Ok(url.into())
}

fn intersects(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] < b[2] && a[2] > b[0] && a[1] < b[3] && a[3] > b[1]
}

fn bounds(record: &Value) -> Result<[f64; 4]> {
    let value = &record["boundingBox"];
    let mut result = [0.; 4];
    for (index, key) in ["minX", "minY", "maxX", "maxY"].into_iter().enumerate() {
        result[index] = value[key]
            .as_f64()
            .ok_or("TNM record has no numerical geographic bbox")?;
    }
    features::bounds(result)?;
    Ok(result)
}

fn text<'a>(record: &'a Value, key: &str, limit: usize) -> Result<&'a str> {
    record[key]
        .as_str()
        .filter(|value| {
            !value.is_empty() && value.len() <= limit && !value.chars().any(char::is_control)
        })
        .ok_or_else(|| format!("TNM record {key} is missing or invalid"))
}

pub(crate) fn original_size(record: &Value) -> Result<u64> {
    record["sizeInBytes"].as_u64().filter(|value| *value > 0 && *value <= MAX_BYTES)
        .ok_or_else(|| "TNM Current original size is missing, zero, non-integer or exceeds 512 MiB; do not guess or substitute historical sizes".into())
}

fn validate_publisher_query(request: &ResolveRequest, raw: &Value) -> Result<()> {
    let value = raw["sciencebaseQuery"]
        .as_str()
        .ok_or("TNM response has no original upstream query provenance")?;
    let url = Url::parse(value).map_err(crate::io_error)?;
    if url.scheme() != "https"
        || url.host_str() != Some("www.sciencebase.gov")
        || url.path() != "/catalog/items"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "TNM upstream query provenance is outside the official read-only catalog".into(),
        );
    }
    let pairs = url.query_pairs().collect::<Vec<_>>();
    let filters = pairs
        .iter()
        .filter(|(key, _)| key == "filter")
        .map(|(_, value)| value.as_ref())
        .collect::<Vec<_>>();
    let tag = format!(
        "tagslq=tags.scheme:\"The National Map Collection Thesaurus\" AND (tags.name: \"{TAG}\")"
    );
    if filters.len() != 2 || !filters.contains(&tag.as_str()) {
        return Err("TNM upstream query does not use the exact Current 1-arc-second tag".into());
    }
    for (name, expected) in [("max", "100"), ("offset", "0"), ("format", "json")] {
        let values = pairs
            .iter()
            .filter(|(key, _)| key == name)
            .map(|(_, value)| value.as_ref())
            .collect::<Vec<_>>();
        if values != vec![expected] {
            return Err("TNM upstream query is paginated or has conflicting result bounds".into());
        }
    }
    let spatial = filters
        .iter()
        .find_map(|value| value.strip_prefix("spatialQuery="))
        .ok_or("TNM upstream spatial filter is missing")?;
    let spatial: Value = serde_json::from_str(spatial).map_err(crate::io_error)?;
    if spatial["relation"] != "intersects" {
        return Err("TNM upstream spatial relation is not the reviewed intersection query".into());
    }
    let polygon = spatial["wkt"]
        .as_str()
        .and_then(|value| value.strip_prefix("POLYGON(("))
        .and_then(|value| value.strip_suffix("))"))
        .ok_or("TNM upstream AOI polygon is invalid")?;
    let points = polygon
        .split(',')
        .map(|point| {
            let numbers = point
                .split_whitespace()
                .map(|value| value.parse::<f64>().map_err(crate::io_error))
                .collect::<Result<Vec<_>>>()?;
            if numbers.len() != 2 {
                return Err("TNM upstream AOI point is invalid".into());
            }
            Ok([numbers[0], numbers[1]])
        })
        .collect::<Result<Vec<_>>>()?;
    let [west, south, east, north] = request.bounds;
    if points
        != vec![
            [west, south],
            [east, south],
            [east, north],
            [west, north],
            [west, south],
        ]
    {
        return Err("TNM response upstream AOI differs from the exact requested map bounds".into());
    }
    Ok(())
}

fn source(record: &Value) -> Result<DatasetSource> {
    let id = text(record, "sourceId", 24)?;
    if id.len() != 24
        || !id
            .bytes()
            .all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
        || record["sourceName"] != "ScienceBase"
        || record["format"] != "GeoTIFF"
    {
        return Err(
            "TNM record must be a ScienceBase Current GeoTIFF with its exact source identity"
                .into(),
        );
    }
    let title = text(record, "title", 512)?;
    let publication = text(record, "publicationDate", 10)?;
    let date =
        chrono::NaiveDate::parse_from_str(publication, "%Y-%m-%d").map_err(crate::io_error)?;
    if date.format("%Y-%m-%d").to_string() != publication {
        return Err("TNM publication date is not the original ISO calendar date".into());
    }
    let href = text(record, "downloadURL", 2048)?;
    let parsed = Url::parse(href).map_err(crate::io_error)?;
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("prd-tnm.s3.amazonaws.com")
        || parsed.port().is_some()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || href.contains('%')
        || href.contains('\\')
    {
        return Err("Only the exact public USGS AWS original URL without credentials, port, query or escape is accepted".into());
    }
    let path = parsed
        .path()
        .strip_prefix("/StagedProducts/Elevation/1/TIFF/historical/")
        .ok_or("Current record must declare a versioned 1-arc-second original TIFF path")?;
    let (tile, filename) = path
        .split_once('/')
        .ok_or("TNM original tile directory is missing")?;
    let bytes = tile.as_bytes();
    if bytes.len() != 7
        || !matches!(bytes[0], b'n' | b's')
        || !matches!(bytes[3], b'e' | b'w')
        || !bytes[1..3]
            .iter()
            .chain(&bytes[4..7])
            .all(u8::is_ascii_digit)
    {
        return Err("TNM original tile identifier is invalid".into());
    }
    let lat =
        tile[1..3].parse::<i32>().map_err(crate::io_error)? * if bytes[0] == b'n' { 1 } else { -1 };
    let lon =
        tile[4..7].parse::<i32>().map_err(crate::io_error)? * if bytes[3] == b'e' { 1 } else { -1 };
    if !(-89..=90).contains(&lat)
        || !(-180..=179).contains(&lon)
        || filename != format!("USGS_1_{tile}_{}.tif", date.format("%Y%m%d"))
        || record["urls"]["TIFF"] != href
    {
        return Err(
            "TNM source tile, publication date and exact original filename disagree".into(),
        );
    }
    let area = bounds(record)?;
    let nominal = [
        f64::from(lon),
        f64::from(lat - 1),
        f64::from(lon + 1),
        f64::from(lat),
    ];
    // Standard tiles contain a six-pixel border. This validates the declared
    // index, never changes the actual bbox or treats it as full valid coverage.
    if !area
        .iter()
        .zip(nominal)
        .all(|(actual, nominal)| (actual - nominal).abs() <= 0.002)
    {
        return Err("TNM original filename grid differs from its actual declared bbox".into());
    }
    original_size(record)?;
    let body = text(record, "body", 32768)?.to_ascii_lowercase();
    if !body.contains("1 arc second")
        || !body.contains("north american datum of 1983")
        || !body.contains("all elevation values are in meters")
        || !body.contains("all 3dep products are public domain")
    {
        return Err("TNM record does not declare the reviewed 1-arc-second NAD83 elevation product and public-domain provenance".into());
    }
    let metadata = text(record, "metaUrl", 2048)?;
    if metadata != format!("https://www.sciencebase.gov/catalog/item/{id}") {
        return Err("TNM ScienceBase metadata identity differs from the selected source".into());
    }
    // Retain the vendor URL in the raw record. It is not followed here and does
    // not authorize arbitrary network destinations or a substituted original.
    text(record, "vendorMetaUrl", 2048)?;
    Ok(DatasetSource {
        id: id.into(),
        title: title.into(),
        bounds: Some(area),
        stac: Some(stac::ConnectRequest {
            name: title.into(),
            url: href.into(),
            kind: "raster".into(),
        }),
        wcs: None,
        coverage_id: None,
        search: None,
        region: None,
        season: None,
        start_datetime: None,
        end_datetime: None,
        catalog_raster: None,
    })
}

pub(crate) fn parse(
    request: &ResolveRequest,
    catalog_url: &str,
    raw: &Value,
) -> Result<Vec<(DatasetSource, Value)>> {
    let request = normalized(request)?;
    if catalog_url != query(&request)? {
        return Err(
            "TNM catalog URL differs from the reviewed Current/bbox/max100 read-only query".into(),
        );
    }
    if raw["errors"]
        .as_array()
        .is_none_or(|errors| !errors.is_empty())
        || raw.get("error").is_some_and(|error| !error.is_null())
    {
        return Err("TNM catalog reported an error; no original selection was produced".into());
    }
    validate_publisher_query(&request, raw)?;
    let total = raw["total"]
        .as_u64()
        .ok_or("TNM catalog result count is missing")?;
    let items = raw["items"]
        .as_array()
        .ok_or("TNM catalog items are missing")?;
    if total > MAX_RECORDS as u64 || total != items.len() as u64 {
        return Err("TNM query exceeds 100 records or is paginated/incomplete; narrow the area before selecting any originals".into());
    }
    if raw
        .get("filteredOut")
        .is_some_and(|value| value.as_u64() != Some(0))
    {
        return Err("TNM response filtered out records; no partial catalog is accepted".into());
    }
    let mut result = Vec::new();
    let mut ids = BTreeSet::new();
    let mut urls = BTreeSet::new();
    for record in items {
        let candidate = source(record)?;
        if !ids.insert(candidate.id.clone())
            || !urls.insert(candidate.stac.as_ref().unwrap().url.clone())
        {
            return Err("TNM Current response repeats a source identity or original URL".into());
        }
        if intersects(candidate.bounds.unwrap(), request.bounds) {
            result.push((candidate, record.clone()));
        }
    }
    if result.is_empty() {
        return Err("No reviewed Current 1-arc-second original declared bbox intersects the requested area; no historical tile or sample area is substituted".into());
    }
    if result.len() > MAX_TILES {
        return Err("More than 8 actual Current elevation originals intersect this area; narrow it instead of truncating results".into());
    }
    Ok(result)
}

pub(crate) fn validate(proof: &SourceProvenance, href: &str) -> Result<()> {
    let request = normalized(&proof.request)?;
    let evidence = proof
        .catalog_raster
        .as_ref()
        .ok_or("TNM source has no archived raw catalog evidence")?;
    if proof.schema_version != 2
        || proof.dataset != definition()
        || proof.request != request
        || proof.layer != definition().layers[0]
        || evidence.catalog_url != query(&request)?
        || evidence.catalog_sha256.len() != 64
        || !evidence
            .catalog_sha256
            .bytes()
            .all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
        || proof.declaration_kind
            != "reviewed-client-catalog-raster; exact publisher catalog record archived separately"
    {
        return Err("TNM provenance differs from its reviewed Current product, normalized request or exact archived catalog receipt".into());
    }
    let candidate = source(&evidence.record)?;
    if candidate.id != proof.source_id
        || candidate.title != proof.source_title
        || candidate.bounds != proof.source_bounds
        || candidate.stac.as_ref().unwrap().url != href
        || !intersects(candidate.bounds.unwrap(), request.bounds)
    {
        return Err("TNM original URL, source identity, title, size or bbox differs from its archived publisher record".into());
    }
    Ok(())
}

/// The caller verifies the exact immutable document bytes against its SHA-256
/// before passing their decoded Value; membership cannot be asserted by a URL.
pub(crate) fn validate_catalog(proof: &SourceProvenance, full_raw: &Value) -> Result<()> {
    let evidence = proof
        .catalog_raster
        .as_ref()
        .ok_or("TNM archived catalog evidence is missing")?;
    let selected = parse(&proof.request, &evidence.catalog_url, full_raw)?;
    let (candidate, record) = selected
        .iter()
        .find(|(source, _)| source.id == proof.source_id)
        .ok_or("TNM archived catalog does not contain the selected original source")?;
    if *record != evidence.record {
        return Err(
            "TNM archived source record differs from the exact full publisher response".into(),
        );
    }
    validate(proof, &candidate.stac.as_ref().unwrap().url)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ResolveRequest {
        serde_json::from_value(serde_json::json!({"datasetId":DATASET_ID,"bounds":[-73.994,40.71,-73.99,40.714],"layer":"elevation"})).unwrap()
    }
    fn raw() -> Value {
        let document: Value = serde_json::from_str(include_str!(
            "../../../../docs/research/next-channels-20261012-usgs-3dep.json"
        ))
        .unwrap();
        document["nativeCase"]["document"].clone()
    }

    #[test]
    fn actual_catalog_filters_coarse_geometry_by_raw_bbox_without_fabricating_dates() {
        let request = request();
        let raw = raw();
        assert_eq!(raw["total"], 2);
        let sources = parse(&request, &query(&request).unwrap(), &raw).unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].0.id, "6aac9d7f1ba49b6a17baded8");
        assert!(sources[0].0.start_datetime.is_none() && sources[0].0.end_datetime.is_none());
        assert_eq!(original_size(&sources[0].1).unwrap(), 20_582_281);
    }

    #[test]
    fn actual_record_rejects_url_scope_filename_date_size_bbox_and_identity_changes() {
        let request = request();
        let url = query(&request).unwrap();
        for (key,value) in [
            ("sourceId",serde_json::json!("../../../escape")),
            ("publicationDate",serde_json::json!("2025-09-17")),
            ("sizeInBytes",Value::Null), ("sizeInBytes",serde_json::json!(0)),
            ("sizeInBytes",serde_json::json!(MAX_BYTES+1)),
            ("downloadURL",serde_json::json!("https://prd-tnm.s3.amazonaws.com.evil.example/StagedProducts/Elevation/1/TIFF/historical/n41w074/USGS_1_n41w074_20260917.tif")),
            ("downloadURL",serde_json::json!("https://prd-tnm.s3.amazonaws.com/StagedProducts/Elevation/1/TIFF/historical/n41w074/%2e%2e/USGS_1_n41w074_20260917.tif")),
            ("format",serde_json::json!("JPEG")), ("sourceName",serde_json::json!("Unverified")),
            ("body",serde_json::json!("Calibrated optical reflectance")),
        ] {
            let mut changed=raw(); changed["items"][0][key]=value;
            assert!(parse(&request,&url,&changed).is_err(),"accepted {key}");
        }
        let mut changed = raw();
        changed["items"][0]["boundingBox"]["minX"] = serde_json::json!(-100.);
        assert!(parse(&request, &url, &changed).is_err());
        let mut changed = raw();
        changed["total"] = serde_json::json!(101);
        assert!(parse(&request, &url, &changed).is_err());
        let mut changed = raw();
        changed["items"][1] = changed["items"][0].clone();
        assert!(parse(&request, &url, &changed).is_err());
        assert!(parse(&request, &url.replace("Current", "Historical"), &raw()).is_err());
    }

    #[test]
    fn unsupported_filters_are_rejected_before_a_catalog_request() {
        for value in [
            serde_json::json!({"year":2026}),
            serde_json::json!({"month":9}),
            serde_json::json!({"country":"USA"}),
            serde_json::json!({"region":"NY"}),
            serde_json::json!({"season":"summer"}),
            serde_json::json!({"layer":"one-meter"}),
            serde_json::json!({"bounds":[0.,0.,0.,1.]}),
        ] {
            let mut original = serde_json::to_value(request()).unwrap();
            for (key, value) in value.as_object().unwrap() {
                original[key] = value.clone();
            }
            let changed: ResolveRequest = serde_json::from_value(original).unwrap();
            assert!(query(&changed).is_err());
        }
    }

    fn proof() -> SourceProvenance {
        let request = request();
        let raw = raw();
        let catalog_url = query(&request).unwrap();
        let (candidate, record) = parse(&request, &catalog_url, &raw).unwrap().remove(0);
        let document: Value = serde_json::from_str(include_str!(
            "../../../../docs/research/next-channels-20261012-usgs-3dep.json"
        ))
        .unwrap();
        SourceProvenance {
            schema_version: 2,
            dataset: definition(),
            request,
            layer: definition().layers.remove(0),
            source_id: candidate.id,
            source_title: candidate.title,
            source_bounds: candidate.bounds,
            declaration_kind:
                "reviewed-client-catalog-raster; exact publisher catalog record archived separately"
                    .into(),
            catalog_raster: Some(CatalogRasterEvidence {
                catalog_url,
                catalog_sha256: document["nativeCase"]["receipt"]["sha256"]
                    .as_str()
                    .unwrap()
                    .into(),
                record,
            }),
        }
    }

    #[test]
    fn archived_proof_binds_definition_record_identity_request_url_and_full_catalog_membership() {
        let original = proof();
        let href = original.catalog_raster.as_ref().unwrap().record["downloadURL"]
            .as_str()
            .unwrap()
            .to_string();
        validate(&original, &href).unwrap();
        validate_catalog(&original, &raw()).unwrap();
        let mut changed = original.clone();
        changed.catalog_raster = None;
        assert!(validate(&changed, &href).is_err());
        let mut changed = original.clone();
        changed.catalog_raster.as_mut().unwrap().catalog_sha256 = "INVALID".into();
        assert!(validate(&changed, &href).is_err());
        let mut changed = original.clone();
        changed.source_title = "Unreviewed title".into();
        assert!(validate(&changed, &href).is_err());
        let mut changed = original.clone();
        changed.source_bounds = Some([-75., 40., -74., 41.]);
        assert!(validate(&changed, &href).is_err());
        let mut changed = original.clone();
        changed.request.bounds = [-73.99, 40.71, -73.98, 40.714];
        assert!(validate(&changed, &href).is_err());
        // Even rewriting the query URL cannot turn this archived original
        // upstream AOI into evidence for a different request.
        changed.catalog_raster.as_mut().unwrap().catalog_url = query(&changed.request).unwrap();
        assert!(validate_catalog(&changed, &raw()).is_err());
        let mut changed = original.clone();
        changed.dataset.layers[0].unit = "ellipsoid height".into();
        assert!(validate(&changed, &href).is_err());
        let mut changed = original.clone();
        changed.catalog_raster.as_mut().unwrap().record["sizeInBytes"] =
            serde_json::json!(20_000_000);
        assert!(validate_catalog(&changed, &raw()).is_err());
        let mut changed = raw();
        changed["items"][0]["lastUpdated"] = serde_json::json!("2026-09-18T00:00:00Z");
        assert!(validate_catalog(&original, &changed).is_err());
        assert!(validate(&original, &href.replace("20260917", "20260918")).is_err());
        // A well-shaped but different SHA requires the caller's immutable
        // byte-hash validation; this pure Value layer does not assert it.
    }
}
