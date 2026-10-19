//! Exact numerical originals independently checked for the next public batch.
//! This is a client file index, not publisher STAC or a generated AOI export.
use super::*;
use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewedIndex {
    originals: Vec<ReviewedOriginal>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewedOriginal {
    dataset_id: String,
    layer: String,
    id: String,
    title: String,
    url: String,
    bounds: [f64; 4],
    bytes: u64,
    sha256: Option<String>,
    header_sha256: String,
    dtype: String,
    nodata: Option<f64>,
    scale: f64,
    offset: f64,
}
static INDEX: LazyLock<ReviewedIndex> = LazyLock::new(|| {
    serde_json::from_str(include_str!("next-hydro-geoid-reviewed-index.json"))
        .expect("compile-time reviewed HydroSHEDS accumulation and NOAA geoid originals")
});

pub(super) fn definitions() -> Vec<Dataset> {
    let mut hydro = base(
        "hydrosheds-accumulation",
        "HydroSHEDS v2 · Upstream accumulation",
        "hydrology",
        "https://www.hydrosheds.org/downloads-core-data-v2",
    );
    hydro.description = "HydroSHEDS v2r0 upstream accumulation from conditioned TanDEM-X drainage routing. ACC counts contributing cells; ACA gives the latitude-adjusted contributing area in square kilometres. This one product has two independent scientific layers, not two counted channels, observed river discharge, rainfall, D8 flow directions or river vectors.".into();
    hydro.version =
        "HydroSHEDS v2r0 · 40 ACC / 27 ACA header-reviewed originals · index 20260830".into();
    hydro.nominal_resolution =
        "1 arc-second (about 30 m at the equator); original 36000 x 36000 area grids".into();
    hydro.unit = "ACC: contributing cells; ACA: contributing area in km²".into();
    hydro.license =
        "CC BY 4.0; retain HydroSHEDS attribution and original redistribution request".into();
    hydro.license_url = "https://www.hydrosheds.org/terms-of-use".into();
    hydro.attribution = "HydroSHEDS v2; Lehner, B., A. Roth, M. Huber, M. Anand and M. Thieme (2022), A sharper look at the world's rivers and catchments, Eos 103, doi:10.1029/2022EO220167. Retain release, selected ACC or ACA layer, original tile, HydroSHEDS / DLR / WWF / Confluvio attribution and access date. The v2 technical guide states CC BY 4.0 and asks users to obtain authors' written permission before mirroring whole unchanged originals on other websites; this connector reads the publisher distribution and does not create a hosted mirror.".into();
    hydro.value_interpretation = "Preserve original Float64 values, scale 1, offset 0 and embedded NoData 1.79e308. ACC includes the target cell, so a first upstream cell is 1; cell count is not km² because cell area varies with latitude. ACA includes the target cell's geodesic area and has fractional square-kilometre values. These derived drainage metrics are not measured flow rate or water volume. Original compressed overviews provide display only; raw-pixel inspection uses the unchanged base grid.".into();
    hydro.layers = vec![
        scaled(layer("acc", "Upstream accumulation · contributing cells (ACC)", "cells", "Float64 original upstream cell count including the target cell; scale 1, offset 0, NoData 1.79e308. Do not interpret this as km², convert using a fixed 30 m cell area, cast to Float32 or substitute flow-direction codes."), 1., 0., 1.79e308),
        scaled(layer("aca", "Upstream accumulation · contributing area (ACA)", "km²", "Float64 original latitude-adjusted upstream area including the target cell's own area; scale 1, offset 0, NoData 1.79e308. Preserve fractional square kilometres and do not substitute ACC counts, hectares, measured discharge or rainfall."), 1., 0., 1.79e308),
    ];
    hydro.default_layer = Some("acc".into());
    hydro.warnings = vec![
        "Only 40 ACC and 27 ACA v2r0 originals in the official Americas index are eligible: each real TIFF header, actual grid/footprint and exact byte count was checked under the unchanged 512 MiB limit. The other larger originals, v1, DIR, river vectors and future releases are not silently substituted. Complete downloads and science pixels were independently accepted for n00w050 only; a checked header does not certify every full original or map.".into(),
        "Both originals are 36000 x 36000 Float64 grids (about 10.4 GB uncompressed each). The app keeps existing bounded tile decoding and uses original internal overviews for PNG/map display; it does not decode the full array into memory, clip a small research file, resample a new product or alter original bytes.".into(),
        "The real n00w050 originals are 76,824,474 bytes (ACC) and 79,213,241 bytes (ACA) regardless of AOI. A footprint intersection or grayscale display does not establish valid land pixels everywhere, connected river quality or watershed correctness.".into(),
        "HydroSHEDS v2 is based on conditioned TanDEM-X topography and routing. The 2026 archive publication is not a 2026 observation of discharge or flooding. No date selector applies to these fixed original files.".into(),
    ];

    let mut geoid = base(
        "etopo-geoid",
        "NOAA ETOPO 2022 · Geoid height",
        "elevation",
        "https://www.ncei.noaa.gov/products/etopo-global-relief-model",
    );
    geoid.description = "ETOPO 2022 v1 published EGM2008 geoid-height grid, a vertical separation field used with WGS84 ellipsoidal elevations. This is an independent physical quantity from the connected ETOPO surface elevation: it is not terrain height, ocean depth, a new bathymetric survey or a computed elevation conversion.".into();
    geoid.version = "ETOPO 2022 v1 · 288 header-reviewed 15 arc-second geoid originals".into();
    geoid.nominal_resolution =
        "15 arc-seconds (about 500 m at the equator); original 3600 x 3600 point grid".into();
    geoid.unit = "metres of EGM2008 geoid separation relative to WGS84 ellipsoid".into();
    geoid.license = "CC0 1.0; original NGA public-domain notice and NOAA citation retained".into();
    geoid.license_url = "https://www.ncei.noaa.gov/access/metadata/landing-page/bin/iso?id=gov.noaa.ngdc.mgg.dem%3Aetopo_2022".into();
    geoid.attribution = "NOAA National Centers for Environmental Information (2022), ETOPO 2022 15 Arc-Second Global Relief Model, doi:10.25921/fd45-gt74. The numerical original retains its NGA Public Domain copyright notice and EGM2008 source description. NOAA metadata applies CC0 1.0 worldwide; retain source, release, selected geoid tile and access date.".into();
    geoid.value_interpretation = "Preserve Float32 geoid separation in metres, scale 1, offset 0, Point registration and no embedded NoData tag. The original identifies WGS84 (EPSG:4979) to EGM2008 height (EPSG:3855), derived from egm08_25.gtx, but its horizontal grid is EPSG:4326. Positive and negative values are valid vertical offsets, not land/sea classifications. NOAA documents ETOPO orthometric elevation + this GEOID = WGS84 ellipsoidal elevation; this connector only delivers the geoid values and performs no automatic datum conversion.".into();
    let mut geoid_layer = layer("geoid", "EGM2008 geoid separation · original metres", "m", "Original Float32 geoid separation, scale 1 and offset 0, no embedded NoData. Negative offsets remain valid; no terrain elevation, positive-depth conversion, assumed missing-zero mask or surface/bedrock substitution is applied.");
    geoid_layer.scale = Some(1.);
    geoid_layer.offset = Some(0.);
    geoid.layers = vec![geoid_layer];
    geoid.default_layer = Some("geoid".into());
    geoid.warnings = vec![
        "Only the 288 actual 15 arc-second geoid files in the official NOAA directory are eligible. Each real TIFF header, actual Point grid/footprint, absent NoData and exact byte count was independently checked. Complete download and all samples were accepted for N45E105 only; that does not certify every full file or online map. ETOPO surface/bedrock, SID classes and generated AOI subsets are not substituted.".into(),
        "The 19,941,002-byte original has no internal overviews and no NoData tag; all 12,960,000 checked samples are finite. Preserve the original Point registration, Float32 precision and signed metre values. A PNG or selected AOI does not change the complete-file download or establish survey precision.".into(),
        "This is a fixed EGM2008-based vertical-reference field distributed with ETOPO 2022. Its TIFF source dates (2018/2019) and 2022 publication are not an observation date or recent terrain update; year/month filters and automatic elevation conversions are unavailable.".into(),
        "The publisher excludes use of ETOPO for navigation. A geoid field is not a tide model, chart datum, local surveyed vertical-control network or real-time water level; users must validate vertical conventions before applying any external conversion.".into(),
    ];
    let limits = vec![
        "At most 8 intersecting complete published files per selection; narrow larger areas. Files are downloaded without clipping, reprojection, resampling or a generated subset. Only the exact reviewed product, layer and intersecting original footprint is eligible.".into(),
        "Only the geographic query footprint is intersected with legal longitude/latitude bounds. Tiny original floating-point outer-edge excursions and the original TIFF transforms, values and bytes remain unchanged.".into(),
        "Existing 512 MiB original transfer, axis, grid-pixel, chunk, intermediate-buffer, memory and preview-deadline limits remain unchanged. Available metadata does not certify every online preview or network transfer.".into(),
    ];
    hydro.limits = limits.clone();
    geoid.limits = limits;
    vec![hydro, geoid]
}

fn intersects(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] < b[2] && a[2] > b[0] && a[1] < b[3] && a[3] > b[1]
}
fn physical(bounds: [f64; 4]) -> [f64; 4] {
    [
        bounds[0].max(-180.),
        bounds[1].max(-90.),
        bounds[2].min(180.),
        bounds[3].min(90.),
    ]
}
pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    features::bounds(request.bounds)?;
    if request.year.is_some()
        || request.month.is_some()
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
    {
        return Err("These fixed accumulation/geoid originals have no year, month, country, region or season selector; no alternate release or area is substituted".into());
    }
    if !matches!(
        (request.dataset_id.as_str(), request.layer.as_deref()),
        ("hydrosheds-accumulation", Some("acc" | "aca")) | ("etopo-geoid", Some("geoid"))
    ) {
        return Err("Choose the exact reviewed HydroSHEDS ACC/ACA or ETOPO geoid layer; no flow-direction, elevation, source-ID, alternative version or provider is substituted".into());
    }
    let mut files = Vec::new();
    for item in &INDEX.originals {
        if item.dataset_id != request.dataset_id
            || Some(item.layer.as_str()) != request.layer.as_deref()
            || !intersects(physical(item.bounds), request.bounds)
        {
            continue;
        }
        if item.bytes == 0
            || item.bytes > 512 * 1024 * 1024
            || item.header_sha256.len() != 64
            || item.sha256.as_ref().is_some_and(|v| v.len() != 64)
        {
            return Err("Invalid reviewed original byte length or SHA256 receipt".into());
        }
        if item.scale != 1.
            || item.offset != 0.
            || match request.dataset_id.as_str() {
                "hydrosheds-accumulation" => {
                    item.dtype != "Float64" || item.nodata != Some(1.79e308)
                }
                "etopo-geoid" => item.dtype != "Float32" || item.nodata.is_some(),
                _ => true,
            }
        {
            return Err(
                "Reviewed original calibration, data type or NoData declaration changed".into(),
            );
        }
        let mut file = source(
            item.title.clone(),
            item.url.clone(),
            Some(physical(item.bounds)),
        );
        file.id = item.id.clone();
        files.push(file);
    }
    if files.is_empty() {
        return Err("The map area does not intersect the exact independently checked original for this fixed product and layer; no other tile, demonstration area or generated subset is selected".into());
    }
    if files.len() > MAX_TILES {
        return Err("The map area intersects more than 8 eligible original files; narrow the area instead of silently dropping files".into());
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(id: &str, layer: &str, bounds: [f64; 4]) -> ResolveRequest {
        ResolveRequest {
            dataset_id: id.into(),
            layer: Some(layer.into()),
            bounds,
            year: None,
            month: None,
            country: None,
            region: None,
            season: None,
        }
    }
    #[test]
    fn accumulation_selects_exact_float64_layer_with_actual_separate_footprints() {
        assert_eq!(
            INDEX.originals.iter().filter(|v| v.layer == "acc").count(),
            40
        );
        assert_eq!(
            INDEX.originals.iter().filter(|v| v.layer == "aca").count(),
            27
        );
        assert_eq!(
            INDEX
                .originals
                .iter()
                .filter(|v| v.layer == "geoid")
                .count(),
            288
        );
        for (layer_id, bytes) in [("acc", 76_824_474), ("aca", 79_213_241)] {
            let files = resolve(&request(
                "hydrosheds-accumulation",
                layer_id,
                [-49.9, 0.2, -49.4, 1.],
            ))
            .unwrap();
            assert_eq!(files.len(), 1);
            assert!(files[0]
                .stac
                .as_ref()
                .unwrap()
                .url
                .ends_with(&format!("n00w050_{}_1s_v2r0.tif", layer_id.to_uppercase())));
            let indexed = INDEX
                .originals
                .iter()
                .find(|v| v.layer == layer_id && v.id.starts_with("n00w050_"))
                .unwrap();
            assert_eq!(indexed.bytes, bytes);
            assert_eq!(indexed.dtype, "Float64");
            assert_eq!(indexed.nodata, Some(1.79e308));
            assert_eq!((indexed.scale, indexed.offset), (1., 0.));
            assert_eq!(files[0].bounds, Some(physical(indexed.bounds)));
            assert!(files[0].search.is_none() && files[0].start_datetime.is_none());
            assert!(files[0].stac.as_ref().unwrap().name.len() <= 80);
        }
        let d = definitions()
            .into_iter()
            .find(|v| v.id == "hydrosheds-accumulation")
            .unwrap();
        assert_eq!(d.layers.len(), 2);
        assert_eq!(d.layers[0].unit, "cells");
        assert_eq!(d.layers[1].unit, "km²");
        assert!(d.license.contains("CC BY 4.0"));
    }
    #[test]
    fn geoid_retains_no_nodata_and_vertical_quantity_separate_from_surface() {
        let files = resolve(&request("etopo-geoid", "geoid", [116.3, 39.8, 116.5, 40.])).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].id, "ETOPO_2022_v1_15s_N45E105_geoid");
        assert_eq!(files[0].bounds, Some([105., 30., 120., 45.]));
        let d = definitions()
            .into_iter()
            .find(|v| v.id == "etopo-geoid")
            .unwrap();
        assert_eq!(d.layers[0].nodata, None);
        assert_eq!(d.layers[0].scale, Some(1.));
        assert_eq!(d.layers[0].offset, Some(0.));
        assert!(d
            .value_interpretation
            .contains("no automatic datum conversion"));
        assert!(d.license.contains("CC0"));
        let boundary = resolve(&request(
            "etopo-geoid",
            "geoid",
            [119.999, 39.8, 120.001, 40.],
        ))
        .unwrap();
        assert_eq!(boundary.len(), 2);
        assert_ne!(boundary[0].title, boundary[1].title);
        assert!(boundary
            .iter()
            .any(|v| v.id == "ETOPO_2022_v1_15s_N45E105_geoid"));
        assert!(boundary
            .iter()
            .any(|v| v.id == "ETOPO_2022_v1_15s_N45E120_geoid"));
        assert!(INDEX.originals.iter().all(|v| v.title.len() <= 80));
    }
    #[test]
    fn wrong_layer_release_or_outside_area_fails_without_fallback() {
        for (id, layer, bounds) in [
            ("hydrosheds-accumulation", "acc", [-49.9, 0.2, -49.4, 1.]),
            ("etopo-geoid", "geoid", [116.3, 39.8, 116.5, 40.]),
        ] {
            let original = request(id, layer, bounds);
            for field in 0..6 {
                let mut r = original.clone();
                match field {
                    0 => r.year = Some(2026),
                    1 => r.month = Some(10),
                    2 => r.country = Some("USA".into()),
                    3 => r.region = Some("sample".into()),
                    4 => r.season = Some("summer".into()),
                    _ => r.layer = Some("surface".into()),
                }
                assert!(resolve(&r).is_err());
            }
            let mut outside = original;
            outside.bounds = if id == "hydrosheds-accumulation" {
                [0., 80., 1., 81.]
            } else {
                // The actual NOAA archive covers every legal geographic
                // region. Test invalid latitude rather than an invented gap.
                [0., 90.01, 1., 91.]
            };
            assert!(resolve(&outside).is_err());
        }
        assert!(resolve(&request(
            "hydrosheds-accumulation",
            "aca",
            [-181., 0., -179., 1.]
        ))
        .is_err());
        assert!(resolve(&request("hydrosheds", "acc", [-49.9, 0.2, -49.4, 1.])).is_err());
        assert!(resolve(&request("etopo-geoid", "geoid", [-180., -80., 180., 80.])).is_err());
    }
}
