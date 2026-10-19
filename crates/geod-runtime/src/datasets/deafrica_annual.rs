//! Versioned DE Africa annual products on their published EPSG:6933 grid.
//! Coordinates resolve candidate item filenames. Fresh publisher JSON confirms
//! availability, identity, actual interval and grid before any file selection.
use super::*;
use proj4rs::Proj;
use serde_json::Value;

const HTTPS: &str = "https://deafrica-services.s3.af-south-1.amazonaws.com/";
const ORIGIN: [f64; 2] = [-17_376_000., -7_392_000.];
const STEP: f64 = 96_000.;
const AFRICA: [f64; 4] = [-26., -40., 65., 40.];
const YEAR: u16 = 2025;
const START: &str = "2025-01-01T00:00:00.000000Z";
const END: &str = "2025-12-31T23:59:59.999999Z";

fn product(id: &str) -> Result<&'static str> {
    match id {
        "deafrica-fractional-cover" => Ok("fc_ls_summary_annual"),
        "deafrica-wofs-annual" => Ok("wofs_ls_summary_annual"),
        _ => Err("Unknown DE Africa annual product".into()),
    }
}

pub(super) fn definitions() -> Vec<Dataset> {
    let mut fc = base(
        "deafrica-fractional-cover",
        "DE Africa · Annual fractional cover",
        "vegetation",
        "https://docs.digitalearthafrica.org/en/latest/data_specs/Fractional_Cover_specs.html",
    );
    fc.description = "Landsat-derived annual statistics of modelled green vegetation, non-green vegetation and bare soil, distributed as complete original African tiles. Component percentiles are independent summaries, not one simultaneous observation.".into();
    fc.attribution = "Digital Earth Africa / Geoscience Australia; Fractional Cover algorithms by the Joint Remote Sensing Research Program. Retain Scarth et al. (2010), the exact publisher item, component/percentile, year and access date.".into();
    for component in ["pv", "npv", "bs"] {
        let title = match component {
            "pv" => "Green vegetation",
            "npv" => "Non-green vegetation",
            _ => "Bare soil",
        };
        for percentile in [10, 50, 90] {
            let id = format!("{component}_pc_{percentile}");
            fc.layers.push(scaled(layer(&id, &format!("{title} · annual percentile {percentile}"),
                "modelled percentage", "Independent annual component percentile, nominally 0–100; model values can exceed 100. Preserve original values without clipping, calibration or summing different component percentiles to 100%. Not measured crop yield or a crop-type classification."), 1., 0., 255.));
        }
    }
    fc.layers.push(scaled(layer("count_valid", "Clear valid observation count", "observation count",
        "Count of clear valid Landsat observations used by the annual summary, not a percentage or confidence score. Use the original embedded NoData declaration."), 1., 0., 255.));
    fc.layers.push(scaled(layer("qa", "Annual summary quality code", "quality code",
        "Original quality-assurance code 0–2, not a percentage; the publisher says this band is not currently used in DE Africa. NoData is independently read from the TIFF."), 1., 0., 255.));
    fc.default_layer = Some("pv_pc_50".into());
    fc.warnings.push("The Fractional Cover service is provisional in Africa: suitable African field validation was unavailable. Green vegetation, non-green vegetation and bare-soil percentiles are estimated independently and must not be added as a simultaneous cover composition. Values above the nominal range are retained.".into());

    let mut water = base(
        "deafrica-wofs-annual",
        "DE Africa · Annual water observations",
        "hydrology",
        "https://docs.digitalearthafrica.org/en/latest/data_specs/Landsat_WOfS_specs.html",
    );
    water.description = "Annual Landsat Water Observations from Space summaries: wet and clear observation counts and their original water-frequency grid. This is an observed annual frequency model, not present water extent, depth or an automatically detected flood.".into();
    water.attribution = "Digital Earth Africa / Geoscience Australia; Water Observations from Space (WOfS), Mueller et al. (2016), Remote Sensing of Environment 174, 341–352. Retain the publisher item, year, selected measurement and access date.".into();
    water.layers = vec![
        layer("frequency", "Annual water observation frequency", "fraction (0–1)", "Original Float32 frequency of wet observations among clear observations. NaN is embedded NoData, not zero water. A valid zero means clear observations without detected water. No conversion to percent is applied."),
        layer("count_wet", "Wet observation count", "observation count", "Original count of observations classified as water, not water area or depth. Clear and wet counts have their own TIFF NoData declaration."),
        layer("count_clear", "Clear observation count", "observation count", "Original count of clear observations. Zero clear observations provide no frequency estimate; the original frequency file retains NaN rather than a fabricated dry-land value."),
    ];
    water.default_layer = Some("frequency".into());
    water.warnings.push("WOfS frequency and observation counts are distinct measurements. Original NaN/missing-data pixels are retained; lack of clear observations is not evidence of dry land. Classification and cloud/terrain effects limit interpretation; raw pixels do not establish local accuracy or full valid-area coverage.".into());

    for dataset in [&mut fc, &mut water] {
        dataset.version = "DE Africa v1.0.0 · annual 2025 published grid recipe".into();
        dataset.years = vec![YEAR];
        dataset.default_year = Some(YEAR);
        dataset.discovery = Some("stac-items".into());
        dataset.nominal_resolution = "30 m · native EPSG:6933 · 3,200 × 3,200 tile".into();
        dataset.warnings.extend([
            "This connector resolves candidate 2025 annual item filenames on the 96 km published EPSG:6933 grid; file existence and real publisher metadata are checked when connecting. The candidate African envelope is not a guarantee of availability, land coverage or scientific validity. No different tile, year or product is substituted.".into(),
            "The actual calendar-year interval is declared in the original dtr:start_datetime/dtr:end_datetime properties. The standard datetime is a reference instant, not a single-scene acquisition. Both fields and all original publisher properties remain unchanged.".into(),
            "Original asset filenames are relative to the actual HTTPS .stac-item.json directory. The publisher's S3 self link names a different .json suffix; revalidation uses the exact reviewed HTTPS item. This is a scoped filename recipe, not general private S3 access or fabricated collection metadata.".into(),
            "Complete original tiles are retained, with native pixel grids and embedded masks. The requested geographic area selects files without clipping or reprojecting them. A raw grayscale preview applies no palette, scientific calibration or component recombination.".into(),
        ]);
        dataset.limits = vec![
            "Up to 8 candidate 96 km tiles per selection; narrow larger requests. The fixed candidate envelope is 26°W–65°E, 40°S–40°N; only actual published African files can connect. Current recipe is version 1.0.0, year 2025.".into(),
            "Each complete original remains subject to the 512 MiB transfer limit and existing native TIFF decoding budgets. Absent or invalid metadata fails explicitly, without background file jobs or fallback products.".into(),
        ];
    }
    vec![fc, water]
}

fn projections() -> Result<(Proj, Proj)> {
    Ok((
        Proj::from_proj_string("+proj=longlat +datum=WGS84 +no_defs").map_err(crate::io_error)?,
        Proj::from_proj_string(
            "+proj=cea +lat_ts=30 +lon_0=0 +x_0=0 +y_0=0 +datum=WGS84 +units=m +no_defs",
        )
        .map_err(crate::io_error)?,
    ))
}

fn project(lon: f64, lat: f64) -> Result<(f64, f64)> {
    let (wgs, grid) = projections()?;
    let mut p = (lon.to_radians(), lat.to_radians(), 0.);
    proj4rs::transform::transform(&wgs, &grid, &mut p).map_err(crate::io_error)?;
    Ok((p.0, p.1))
}

fn tile_bounds(x: i32, y: i32) -> Result<[f64; 4]> {
    let (wgs, grid) = projections()?;
    let mut sw = (
        ORIGIN[0] + f64::from(x) * STEP,
        ORIGIN[1] + f64::from(y) * STEP,
        0.,
    );
    let mut ne = (sw.0 + STEP, sw.1 + STEP, 0.);
    for p in [&mut sw, &mut ne] {
        let target_y = p.1;
        proj4rs::transform::transform(&grid, &wgs, p).map_err(crate::io_error)?;
        // The CEA inverse uses a latitude approximation. Refine against the
        // forward ellipsoidal transform so exact tile edges round-trip to the
        // original metre coordinates instead of selecting a neighbouring row.
        for _ in 0..4 {
            let mut center = (0., p.1, 0.);
            let mut lower = (0., p.1 - 1e-7, 0.);
            let mut upper = (0., p.1 + 1e-7, 0.);
            proj4rs::transform::transform(&wgs, &grid, &mut center).map_err(crate::io_error)?;
            proj4rs::transform::transform(&wgs, &grid, &mut lower).map_err(crate::io_error)?;
            proj4rs::transform::transform(&wgs, &grid, &mut upper).map_err(crate::io_error)?;
            let derivative = (upper.1 - lower.1) / 2e-7;
            if !derivative.is_finite() || derivative == 0. {
                return Err("Annual grid inverse transform is not finite".into());
            }
            p.1 -= (center.1 - target_y) / derivative;
        }
    }
    Ok([
        sw.0.to_degrees(),
        sw.1.to_degrees(),
        ne.0.to_degrees(),
        ne.1.to_degrees(),
    ])
}

fn snap(value: f64) -> f64 {
    // Stabilize exact projected grid edges after round-trip degree conversion.
    // This tolerance is under 10 micrometres and does not expand an AOI.
    if (value - value.round()).abs() < 1e-10 {
        value.round()
    } else {
        value
    }
}

pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    let name = product(&request.dataset_id)?;
    if request.year != Some(YEAR)
        || request.month.is_some()
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
    {
        return Err("DE Africa annual recipes use year 2025 and the current geographic area, not month, country, region or season filters".into());
    }
    let b = request.bounds;
    if b[0] < AFRICA[0] || b[1] < AFRICA[1] || b[2] > AFRICA[2] || b[3] > AFRICA[3] {
        return Err("The requested area is outside the African annual-product candidate envelope; no sample region or alternate source is substituted".into());
    }
    let (west, south) = project(b[0], b[1])?;
    let (east, north) = project(b[2], b[3])?;
    let x0 = snap((west - ORIGIN[0]) / STEP).floor() as i32;
    let x1 = snap((east - ORIGIN[0]) / STEP).ceil() as i32;
    let y0 = snap((south - ORIGIN[1]) / STEP).floor() as i32;
    let y1 = snap((north - ORIGIN[1]) / STEP).ceil() as i32;
    let count = i64::from(x1 - x0) * i64::from(y1 - y0);
    if count < 1 || count > MAX_TILES as i64 {
        return Err(format!(
            "This area selects {count} candidate annual tiles; narrow it to at most {MAX_TILES}"
        ));
    }
    let mut result = Vec::new();
    for x in x0..x1 {
        for y in y0..y1 {
            let region = format!("x{x:03}y{y:03}");
            let stem = format!("{name}_{region}_{YEAR}--P1Y");
            let path = format!("{name}/1-0-0/x{x:03}/y{y:03}/{YEAR}--P1Y/{stem}.stac-item.json");
            let title = format!("{name} · {region} · {YEAR}");
            result.push(DatasetSource {
                id: stem,
                title: title.clone(),
                bounds: Some(tile_bounds(x, y)?),
                stac: Some(stac::ConnectRequest {
                    name: title,
                    kind: "item".into(),
                    url: format!("{HTTPS}{path}"),
                }),
                wcs: None,
                coverage_id: None,
                search: None,
                region: Some(region),
                season: None,
                start_datetime: Some(START.into()),
                end_datetime: Some(END.into()),
                catalog_raster: None,
            });
        }
    }
    Ok(result)
}

pub(crate) fn validate(proof: &SourceProvenance, url: &str, raw: Option<&Value>) -> Result<()> {
    let name = product(&proof.dataset.id)?;
    if proof.schema_version != 2 {
        return Err("Unsupported annual publisher-item provenance".into());
    }
    let plan = super::resolve(proof.request.clone())?;
    let source = plan
        .sources
        .iter()
        .find(|s| s.id == proof.source_id && s.stac.as_ref().is_some_and(|r| r.url == url))
        .ok_or("Annual item does not match the fixed product/grid/year recipe")?;
    if super::provenance(&plan, source) != *proof {
        return Err("Annual product provenance differs from its immutable preset".into());
    }
    let Some(raw) = raw else {
        return Ok(());
    };
    let p = &raw["properties"];
    if raw["type"] != "Feature"
        || raw["id"]
            .as_str()
            .is_none_or(|id| uuid::Uuid::parse_str(id).is_err())
        || p["odc:product"] != name
        || p["odc:dataset_version"] != "1.0.0"
        || p["odc:region_code"].as_str() != source.region.as_deref()
        || p["dtr:start_datetime"] != START
        || p["dtr:end_datetime"] != END
        || p["datetime"] != "2025-01-01T00:00:00Z"
        || p["proj:epsg"] != 6933
        || p["proj:shape"] != serde_json::json!([3200, 3200])
    {
        return Err("Publisher annual item identity, product, interval or native grid differs from the fixed recipe".into());
    }
    for (standard, extended) in [
        ("start_datetime", "dtr:start_datetime"),
        ("end_datetime", "dtr:end_datetime"),
    ] {
        if let Some(value) = p.get(standard) {
            let actual = value
                .as_str()
                .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok());
            let expected = p[extended]
                .as_str()
                .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok());
            if actual.is_none() || actual != expected {
                return Err("Publisher standard temporal interval conflicts with the reviewed dtr annual period".into());
            }
        }
    }
    let b: [f64; 4] = serde_json::from_value(raw["bbox"].clone()).map_err(crate::io_error)?;
    if !b
        .iter()
        .zip(source.bounds.unwrap())
        .all(|(a, b)| (a - b).abs() <= 1e-8)
    {
        return Err(
            "Publisher annual item bounds conflict with the resolved EPSG:6933 tile".into(),
        );
    }
    let region = source.region.as_deref().unwrap();
    let x = region[1..4].parse::<i32>().map_err(crate::io_error)?;
    let y = region[5..8].parse::<i32>().map_err(crate::io_error)?;
    let expected = [
        30.,
        0.,
        ORIGIN[0] + f64::from(x) * STEP,
        0.,
        -30.,
        ORIGIN[1] + f64::from(y + 1) * STEP,
        0.,
        0.,
        1.,
    ];
    let actual: [f64; 9] =
        serde_json::from_value(p["proj:transform"].clone()).map_err(crate::io_error)?;
    if actual != expected {
        return Err("Publisher annual item transform conflicts with its tile index".into());
    }
    let assets = raw["assets"]
        .as_object()
        .ok_or("Annual publisher item has no assets")?;
    if assets.len() != proof.dataset.layers.len() {
        return Err("Annual publisher asset list changed".into());
    }
    for layer in &proof.dataset.layers {
        let a = assets
            .get(&layer.id)
            .ok_or("Selected annual product measurement is missing")?;
        if a["href"] != format!("{}_{}.tif", source.id, layer.id)
            || a["roles"] != serde_json::json!(["data"])
            || a["type"]
                .as_str()
                .is_none_or(|t| !t.starts_with("image/tiff"))
        {
            return Err(
                "Annual assets must use the exact publisher-relative TIFF filename and data role"
                    .into(),
            );
        }
    }
    let self_href = format!(
        "s3://deafrica-services/{}",
        url.strip_prefix(HTTPS)
            .ok_or("Invalid annual endpoint")?
            .replace(".stac-item.json", ".json")
    );
    let links = raw["links"]
        .as_array()
        .ok_or("Annual publisher item has no links")?;
    let self_links = links
        .iter()
        .filter(|l| l["rel"] == "self")
        .collect::<Vec<_>>();
    if self_links.len() != 1 || self_links[0]["href"] != self_href {
        return Err("Annual publisher self link changed or is ambiguous".into());
    }
    Ok(())
}

pub(crate) fn transports(
    proof: &SourceProvenance,
    url: &str,
    raw: &Value,
) -> Result<Vec<stac::AssetTransport>> {
    validate(proof, url, Some(raw))?;
    // Relative HTTP assets already resolve against the exact archived item URL.
    // No S3 transport exception or rewritten publisher metadata is needed.
    Ok(Vec::new())
}

pub(crate) fn self_url(proof: &SourceProvenance, url: &str, raw: &Value) -> Result<String> {
    validate(proof, url, Some(raw))?;
    Ok(url.into())
}

#[cfg(test)]
pub(crate) fn reviewed_document(id: &str) -> Value {
    let documents: Value =
        serde_json::from_str(include_str!("deafrica-annual-test-items.json")).unwrap();
    documents[id].clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_nairobi_grid_and_half_open_edges_resolve_without_a_demo_aoi() {
        let mut request = ResolveRequest {
            dataset_id: "deafrica-fractional-cover".into(),
            bounds: [36.84, -1.30, 36.86, -1.28],
            year: Some(2025),
            layer: Some("pv_pc_50".into()),
            month: None,
            country: None,
            region: None,
            season: None,
        };
        let sources = resolve(&request).unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].region.as_deref(), Some("x218y075"));
        assert!(sources[0]
            .stac
            .as_ref()
            .unwrap()
            .url
            .ends_with("fc_ls_summary_annual_x218y075_2025--P1Y.stac-item.json"));
        request.bounds = tile_bounds(218, 75).unwrap();
        assert_eq!(resolve(&request).unwrap().len(), 1);
        request.bounds = [116., 39., 117., 40.];
        assert!(resolve(&request).is_err());
        request.bounds = [0., -20., 40., 20.];
        assert!(resolve(&request).is_err());
        request.bounds = [36.84, -1.30, 36.86, -1.28];
        request.year = Some(2024);
        assert!(resolve(&request).is_err());
    }

    #[test]
    fn real_annual_documents_bind_relative_assets_native_grid_and_actual_dtr_period() {
        for id in ["deafrica-fractional-cover", "deafrica-wofs-annual"] {
            let plan = super::super::resolve(
                serde_json::from_value(serde_json::json!({
                    "datasetId":id,"bounds":[36.84,-1.30,36.86,-1.28]
                }))
                .unwrap(),
            )
            .unwrap();
            let source = &plan.sources[0];
            let proof = super::super::provenance(&plan, source);
            let url = &source.stac.as_ref().unwrap().url;
            let raw = reviewed_document(id);
            validate(&proof, url, Some(&raw)).unwrap();
            assert!(transports(&proof, url, &raw).unwrap().is_empty());
            assert_eq!(self_url(&proof, url, &raw).unwrap(), *url);
            assert_ne!(source.id, raw["id"].as_str().unwrap());
            for (key, value) in [
                ("proj:epsg", serde_json::json!(4326)),
                ("odc:region_code", serde_json::json!("x219y075")),
                (
                    "dtr:end_datetime",
                    serde_json::json!("2024-12-31T23:59:59Z"),
                ),
                ("start_datetime", serde_json::json!("2019-01-01T00:00:00Z")),
                ("start_datetime", Value::Null),
            ] {
                let mut changed = raw.clone();
                changed["properties"][key] = value;
                assert!(
                    validate(&proof, url, Some(&changed)).is_err(),
                    "accepted {key}"
                );
            }
            let mut changed = raw.clone();
            changed["properties"]["start_datetime"] =
                serde_json::json!("2025-01-01T01:00:00+01:00");
            changed["properties"]["end_datetime"] = serde_json::json!(END);
            validate(&proof, url, Some(&changed)).unwrap();
            let asset = proof.dataset.default_layer.as_ref().unwrap();
            changed = raw.clone();
            changed["assets"][asset]["href"] = serde_json::json!("../../unrelated.tif");
            assert!(validate(&proof, url, Some(&changed)).is_err());
            changed = raw.clone();
            changed["links"][1]["href"] = serde_json::json!("https://unreviewed.test/item.json");
            assert!(validate(&proof, url, Some(&changed)).is_err());
            let mut wrong_proof = proof.clone();
            wrong_proof.dataset.layers[0].unit = "calibrated yield".into();
            assert!(validate(&wrong_proof, url, Some(&raw)).is_err());
            assert!(validate(&proof, "https://unreviewed.test/item.json", Some(&raw)).is_err());
        }
    }
}
