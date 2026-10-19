//! One reviewed project-based 1 m DEM. Real TNM records, not fabricated STAC.
//! This narrow adapter does not enable every 1 m project or the seamless S1M.
use super::*;
use serde_json::Value;
use std::collections::BTreeSet;
use url::Url;

pub(crate) const DATASET_ID: &str = "usgs-3dep-1m";
const API: &str = "https://tnmaccess.nationalmap.gov/api/v1/products";
const TAG: &str = "Digital Elevation Model (DEM) 1 meter";
const SOURCE_ID: &str = "6aac9d8b1ba49b6a17badefa";
const ORIGINAL: &str = "https://prd-tnm.s3.amazonaws.com/StagedProducts/Elevation/1m/Projects/NJ_NE6County_B23/TIFF/USGS_1M_18_x58y451_NJ_NE6County_B23.tif";
const MAX_RECORDS: usize = 100;
const MAX_BYTES: u64 = 512 * 1024 * 1024;

pub(super) fn definition() -> Dataset {
    let mut value = base(
        DATASET_ID,
        "USGS 3DEP · NJ 1 m DEM · reviewed tile",
        "elevation",
        "https://www.usgs.gov/3d-elevation-program/about-3dep-products-services",
    );
    value.description = "One complete reviewed project-based NJ_NE6County_B23 bare-earth 1 m tile from the real public USGS National Map catalog. Only requests intersecting its published geographic extent return this original. This is not all US 1 m coverage, seamless S1M, a satellite image, LiDAR points, or a Current 1-arc-second product.".into();
    value.version = "NJ_NE6County_B23 · USGS_1M_18_x58y451 · publication 2026-09-17".into();
    value.license = "Public domain · USGS 3DEP".into();
    value.license_url = value.documentation_url.clone();
    value.attribution = "U.S. Geological Survey, 3D Elevation Program / The National Map. Retain ScienceBase 6aac9d8b1ba49b6a17badefa, the exact TNM record, vendor XML and access receipt.".into();
    value.nominal_resolution = "1 m original NAD83 / UTM zone 18N grid (EPSG:26918); retain the complete 10012 × 10012 tile and border overlap".into();
    value.unit = "metres; NAVD88 per actual publisher record and XML abstract".into();
    value.value_interpretation = "Original Float32 bare-earth elevations in metres, referenced to NAVD88 according to the exact TNM record body and original vendor XML abstract. Source period in that XML is 2023-04-13 through 2025-04-09; 2026-09-17 is the publication date. The TIFF has no explicit vertical EPSG key. Its actual embedded NoData is -999999; zero and negative elevations are valid. No datum conversion, calibration, clipping or resampling is applied.".into();
    value.layers = vec![layer(
        "elevation",
        "NJ project bare-earth elevation · reviewed tile",
        "m",
        &value.value_interpretation,
    )];
    value.default_layer = Some("elevation".into());
    value.discovery = Some("catalog-raster".into());
    value.warnings = vec![
        "Only the reviewed NJ_NE6County_B23 tile is connected. Other project originals and the separate seamless 1 m product remain outside this connector, even if the official API also lists them.".into(),
        "The read-only TNM query uses your exact map area. Only an exact reviewed record actually returned by that query and whose declared geographic bbox intersects the area can be selected. No demonstration area, alternative project, historical file or guessed URL is substituted.".into(),
        "The published record bbox and the embedded NAD83 UTM grid bounds are different declarations and are preserved separately. Neither bbox intersection nor an available file proves valid terrain coverage at every pixel; the tested complete original contains substantial NoData.".into(),
        "The actual original is a 10 km tile with a six-pixel border, not a 1 km tile. Downloading retains all 10012 × 10012 original samples without AOI clipping.".into(),
        "Publication is not a single acquisition instant. The original XML source period 2023-04-13 through 2025-04-09 remains distinct from the publication date and no observation datetime is synthesized.".into(),
        "NAD83 / UTM zone 18N is EPSG:26918, not WGS84 UTM 32618. Map placement does not transform original elevation values or establish survey-grade horizontal or vertical datum conversion.".into(),
    ];
    value.limits = vec![
        "One complete public TNM query is bounded to 100 records; incomplete or paginated responses require a narrower area. Only the single exact reviewed original may be selected; unknown projects are not silently claimed as supported.".into(),
        "The exact original size is 45,328,865 bytes. Downloading keeps strict source provenance, original size and the generic 512 MiB cap. A catalog or metadata success alone does not establish native download/inspection acceptance.".into(),
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
        || request
            .layer
            .as_deref()
            .is_some_and(|layer| layer != "elevation")
    {
        return Err("The reviewed USGS NJ 1 m DEM uses geographic map bounds and an elevation layer, not year, month, country, region or season filters".into());
    }
    let mut value = request.clone();
    value.layer = Some("elevation".into());
    Ok(value)
}

pub(crate) fn query(request: &ResolveRequest) -> Result<String> {
    let request = normalized(request)?;
    let mut url = Url::parse(API).map_err(crate::io_error)?;
    url.query_pairs_mut()
        .append_pair("datasets", TAG)
        .append_pair("bbox", &request.bounds.map(|v| v.to_string()).join(","))
        .append_pair("max", "100")
        .append_pair("offset", "0")
        .append_pair("outputFormat", "JSON");
    Ok(url.into())
}

fn reviewed_record() -> Result<Value> {
    serde_json::from_str(include_str!("usgs-3dep-1m-reviewed-record.json")).map_err(crate::io_error)
}

fn intersects(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] < b[2] && a[2] > b[0] && a[1] < b[3] && a[3] > b[1]
}

fn bounds(record: &Value) -> Result<[f64; 4]> {
    let mut area = [0.; 4];
    for (index, key) in ["minX", "minY", "maxX", "maxY"].into_iter().enumerate() {
        area[index] = record["boundingBox"][key]
            .as_f64()
            .ok_or("TNM 1 m record has no numerical geographic bbox")?;
    }
    features::bounds(area)?;
    Ok(area)
}

pub(crate) fn original_size(record: &Value) -> Result<u64> {
    record["sizeInBytes"]
        .as_u64()
        .filter(|size| *size == 45_328_865 && *size <= MAX_BYTES)
        .ok_or_else(|| {
            "Reviewed TNM 1 m original size differs from the exact verified publisher declaration"
                .into()
        })
}

fn source(record: &Value) -> Result<DatasetSource> {
    if *record != reviewed_record()?
        || record["sourceId"] != SOURCE_ID
        || record["downloadURL"] != ORIGINAL
        || record["urls"]["TIFF"] != ORIGINAL
        || record["format"] != "GeoTIFF"
        || record["sourceName"] != "ScienceBase"
    {
        return Err("TNM 1 m source differs from the exact reviewed original publisher record; other project files are not accepted".into());
    }
    let body = record["body"]
        .as_str()
        .ok_or("TNM 1 m scientific declaration is missing")?;
    if !body.contains("standard one-meter resolution")
        || !body.contains("North American Datum of 1983 (NAD83)")
        || !body.contains("North American Vertical Datum of 1988 (NAVD88)")
        || !body.contains("All 3DEP products are public domain")
    {
        return Err("TNM 1 m publisher scientific/datum/license declarations differ from the reviewed product".into());
    }
    original_size(record)?;
    let title = record["title"]
        .as_str()
        .ok_or("TNM 1 m source title is missing")?;
    Ok(DatasetSource {
        id: SOURCE_ID.into(),
        title: title.into(),
        bounds: Some(bounds(record)?),
        stac: Some(stac::ConnectRequest {
            name: title.into(),
            url: ORIGINAL.into(),
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

fn validate_publisher_query(request: &ResolveRequest, raw: &Value) -> Result<()> {
    let upstream = raw["sciencebaseQuery"]
        .as_str()
        .ok_or("TNM 1 m upstream query provenance is missing")?;
    let url = Url::parse(upstream).map_err(crate::io_error)?;
    if url.scheme() != "https"
        || url.host_str() != Some("www.sciencebase.gov")
        || url.path() != "/catalog/items"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
    {
        return Err("TNM 1 m upstream query is outside the official read-only catalog".into());
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
        return Err(
            "TNM upstream query does not use the exact project-based 1 meter product tag".into(),
        );
    }
    for (key, expected) in [("max", "100"), ("offset", "0"), ("format", "json")] {
        if pairs
            .iter()
            .filter(|(name, _)| name == key)
            .map(|(_, value)| value.as_ref())
            .collect::<Vec<_>>()
            != vec![expected]
        {
            return Err("TNM 1 m upstream query is paginated or has conflicting limits".into());
        }
    }
    let spatial: Value = serde_json::from_str(
        filters
            .iter()
            .find_map(|v| v.strip_prefix("spatialQuery="))
            .ok_or("TNM 1 m upstream AOI is missing")?,
    )
    .map_err(crate::io_error)?;
    if spatial["relation"] != "intersects" {
        return Err("TNM 1 m spatial relation differs from the reviewed query".into());
    }
    let polygon = spatial["wkt"]
        .as_str()
        .and_then(|v| v.strip_prefix("POLYGON(("))
        .and_then(|v| v.strip_suffix("))"))
        .ok_or("TNM 1 m upstream polygon is invalid")?;
    let points = polygon
        .split(',')
        .map(|point| {
            let numbers = point
                .split_whitespace()
                .map(|v| v.parse::<f64>().map_err(crate::io_error))
                .collect::<Result<Vec<_>>>()?;
            if numbers.len() != 2 {
                return Err("TNM 1 m AOI point is invalid".into());
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
        return Err("TNM 1 m response AOI differs from the exact map request".into());
    }
    Ok(())
}

pub(crate) fn parse(
    request: &ResolveRequest,
    catalog_url: &str,
    raw: &Value,
) -> Result<Vec<(DatasetSource, Value)>> {
    let request = normalized(request)?;
    if catalog_url != query(&request)? {
        return Err(
            "TNM 1 m catalog URL differs from the exact reviewed product/bbox query".into(),
        );
    }
    if raw["errors"]
        .as_array()
        .is_none_or(|errors| !errors.is_empty())
        || raw.get("error").is_some_and(|v| !v.is_null())
        || raw
            .get("filteredOut")
            .is_some_and(|v| v.as_u64() != Some(0))
    {
        return Err("TNM 1 m query has errors or omitted records".into());
    }
    validate_publisher_query(&request, raw)?;
    let total = raw["total"]
        .as_u64()
        .ok_or("TNM 1 m result count is missing")?;
    let items = raw["items"].as_array().ok_or("TNM 1 m items are missing")?;
    if total > MAX_RECORDS as u64 || total != items.len() as u64 {
        return Err("TNM 1 m query exceeds 100 records or is incomplete; narrow the area".into());
    }
    let mut ids = BTreeSet::new();
    let mut result = Vec::new();
    for record in items {
        let id = record["sourceId"]
            .as_str()
            .ok_or("TNM 1 m source identity is missing")?;
        if !ids.insert(id) {
            return Err("TNM 1 m catalog repeats a source identity".into());
        }
        if id == SOURCE_ID {
            let candidate = source(record)?;
            if intersects(candidate.bounds.unwrap(), request.bounds) {
                result.push((candidate, record.clone()));
            }
        }
    }
    if result.is_empty() {
        return Err("No exact reviewed NJ 1 m tile returned by this catalog intersects your area; other projects or a demonstration region are not substituted".into());
    }
    Ok(result)
}

pub(crate) fn validate(proof: &SourceProvenance, href: &str) -> Result<()> {
    let request = normalized(&proof.request)?;
    let evidence = proof
        .catalog_raster
        .as_ref()
        .ok_or("TNM 1 m provenance has no archived original catalog evidence")?;
    if proof.schema_version != 2
        || proof.dataset != definition()
        || proof.request != request
        || proof.layer != definition().layers[0]
        || evidence.catalog_url != query(&request)?
        || evidence.catalog_sha256.len() != 64
        || !evidence
            .catalog_sha256
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
        || proof.declaration_kind
            != "reviewed-client-catalog-raster; exact publisher catalog record archived separately"
    {
        return Err(
            "TNM 1 m provenance differs from the reviewed product/request/catalog receipt".into(),
        );
    }
    let candidate = source(&evidence.record)?;
    if candidate.id != proof.source_id
        || candidate.title != proof.source_title
        || candidate.bounds != proof.source_bounds
        || href != ORIGINAL
        || !intersects(candidate.bounds.unwrap(), request.bounds)
    {
        return Err(
            "TNM 1 m original or source fields differ from the archived reviewed record".into(),
        );
    }
    Ok(())
}

pub(crate) fn validate_catalog(proof: &SourceProvenance, full_raw: &Value) -> Result<()> {
    let evidence = proof
        .catalog_raster
        .as_ref()
        .ok_or("TNM 1 m archived catalog is missing")?;
    let selected = parse(&proof.request, &evidence.catalog_url, full_raw)?;
    let (candidate, record) = selected
        .iter()
        .find(|(source, _)| source.id == proof.source_id)
        .ok_or("TNM 1 m full catalog does not contain the selected source")?;
    if *record != evidence.record {
        return Err(
            "TNM 1 m selected record differs from the original full catalog response".into(),
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
        serde_json::from_str(include_str!("usgs-3dep-1m-reviewed-query.json")).unwrap()
    }
    #[test]
    fn selects_only_exact_reviewed_project_from_complete_real_catalog() {
        let request = request();
        let raw = raw();
        assert_eq!(raw["total"], 3);
        let selected = parse(&request, &query(&request).unwrap(), &raw).unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].0.id, SOURCE_ID);
        assert_eq!(original_size(&selected[0].1).unwrap(), 45_328_865);
        assert!(selected[0].0.start_datetime.is_none() && selected[0].0.end_datetime.is_none());
    }
    #[test]
    fn does_not_substitute_nonintersecting_area_or_wrong_product() {
        let mut request = request();
        request.bounds = [116., 39., 117., 40.];
        let mut raw = raw();
        // Retain a correctly matching upstream query before testing local bbox filtering.
        raw["sciencebaseQuery"] = Value::String(raw["sciencebaseQuery"].as_str().unwrap().replace(
            "-73.994+40.71%2C-73.99+40.71%2C-73.99+40.714%2C-73.994+40.714%2C-73.994+40.71",
            "116+39%2C117+39%2C117+40%2C116+40%2C116+39",
        ));
        assert!(validate_publisher_query(&request, &raw).is_ok());
        assert!(parse(&request, &query(&request).unwrap(), &raw).is_err());
        request.dataset_id = "usgs-3dep".into();
        assert!(query(&request).is_err());
    }
    #[test]
    fn rejects_changed_record_or_incomplete_publisher_catalog() {
        let request = request();
        let url = query(&request).unwrap();
        for (key, value) in [
            ("sizeInBytes", serde_json::json!(45_328_864)),
            (
                "downloadURL",
                serde_json::json!("https://evil.example/original.tif"),
            ),
            ("publicationDate", serde_json::json!("2025-09-17")),
            ("body", serde_json::json!("WGS84 heights")),
        ] {
            let mut changed = raw();
            changed["items"][0][key] = value;
            assert!(parse(&request, &url, &changed).is_err());
        }
        let mut changed = raw();
        changed["total"] = serde_json::json!(101);
        assert!(parse(&request, &url, &changed).is_err());
        let mut changed = raw();
        changed["items"][1] = changed["items"][0].clone();
        assert!(parse(&request, &url, &changed).is_err());
        let mut changed = raw();
        changed["items"].as_array_mut().unwrap().remove(0);
        changed["total"] = serde_json::json!(2);
        assert!(parse(&request, &url, &changed).is_err());
    }
}
