//! Fixed OpenTopography published numerical files, separate from its keyed
//! subset API. The finite file index is client provenance, not publisher STAC.
use super::*;
use serde::Deserialize;
use std::sync::LazyLock;

const HTTPS: &str = "https://opentopography.s3.sdsc.edu/raster/";
const GEDI_PATH: &str = "GEDI_L3/GEDI_L3_vh/GEDI03_rh100_mean_2019108_2022019_002_03_EPSG4326.tif";
const GEDI_QUERY_BOUNDS: [f64; 4] = [
    -180.,
    -85.04076567350337,
    179.99597795336211,
    85.04456640737938,
];
const GEDI_BYTES: u64 = 407_711_565;
const GEDI_POLICY: &str = "https://www.earthdata.nasa.gov/engage/open-data-services-and-software/data-and-information-policy";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PublishedIndex {
    gl3_paths: Vec<String>,
    srtm15_tiles: Vec<PublishedTile>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PublishedTile {
    path: String,
    bounds: [f64; 4],
    bytes: u64,
}
static INDEX: LazyLock<PublishedIndex> = LazyLock::new(|| {
    serde_json::from_str(include_str!("public-batch-reviewed-index.json"))
        .expect("compile-time reviewed OpenTopography original file index")
});

fn limits() -> Vec<String> {
    vec![
        "At most 8 intersecting published original files per selection; narrow larger areas. Complete files are downloaded without clipping, reprojection, resampling or a generated subset.".into(),
        "Each original remains limited to 512 MiB. Existing axis, grid-pixel, chunk, intermediate-buffer, memory and preview-deadline limits remain unchanged; an available source or a successful header query is not a guarantee of every full transfer or online map preview.".into(),
    ]
}

pub(super) fn definitions() -> Vec<Dataset> {
    let mut gl3 = base(
        "srtm-gl3",
        "SRTM GL3 V3 · 3 arc-second elevation",
        "elevation",
        "https://portal.opentopography.org/datasetMetadata?otCollectionID=OT.042013.4326.1",
    );
    gl3.description = "NASA SRTMGL3 Version 3 void-filled 3 arc-second elevation, accessed through OpenTopography's published one-degree numerical GeoTIFF archive. This is the distinct coarser GL3 product, not GL1, NASADEM, an API-generated subset, satellite imagery or a new terrain observation.".into();
    gl3.version =
        "SRTMGL3 V3 · OpenTopography static COG archive · reviewed file index 2026-10-14".into();
    gl3.nominal_resolution =
        "3 arc-seconds (0.0008333333333 degree), about 90 m at the equator".into();
    gl3.unit = "metres relative to EGM96 geoid".into();
    gl3.license =
        "OpenTopography collection: Not Provided; NASA SRTM data-use attribution retained".into();
    gl3.license_url = gl3.documentation_url.clone();
    gl3.attribution = "NASA / NGA Shuttle Radar Topography Mission, SRTMGL3 Version 3; Farr et al. (2007), doi:10.1029/2005RG000183. Distributed by OpenTopography, doi:10.5069/G9445JDF. Retain source, version, tile, access date and OpenTopography acknowledgement. Its collection does not provide a license identifier; no Creative Commons label is invented.".into();
    gl3.value_interpretation = "Original signed Int16 elevation, scale 1 and offset 0; embedded NoData -32768. The reviewed N35E139 original has valid zero and negative elevation and no missing pixels. Point registration is retained: 1201 x 1201 samples include both one-degree endpoints, and outer GDAL bounds extend by half of a 3 arc-second cell. Elevation is not canopy-free terrain everywhere or a guaranteed vertical accuracy.".into();
    gl3.layers = vec![scaled(layer("elevation", "SRTMGL3 V3 elevation · 3 arc-seconds", "m (EGM96)", "Original Int16 elevation samples, not optical imagery or GL1. NoData -32768; valid zero and negative heights are preserved. Keep PixelIsPoint registration and original 1201 x 1201 samples without cropping or resampling."), 1., 0., -32768.)];
    gl3.default_layer = Some("elevation".into());
    gl3.warnings = vec![
        "Only 14,280 actual original files in the reviewed OpenTopography SRTM_GL3_srtm.vrt are eligible. Nominal coverage is 56 degrees south to 60 degrees north; unlisted ocean tiles are not generated, and a map area with no listed tile fails without choosing a demonstration area or another elevation product.".into(),
        "Point sample centres lie on one-degree boundaries. Adjacent files share endpoint samples and their outer pixel-edge footprints overlap by one cell; a boundary-area query can select more than one tile. The original grid and overlap are retained, not silently combined into an area-registered seamless raster.".into(),
        "The SRTM mission observations are from February 2000; a 2013 publication or the 2026 index review is not a new acquisition. No year, month, region, country or seasonal observation selector applies to this fixed product archive.".into(),
        "OpenTopography's collection page states Use License: Not Provided and requires source acknowledgement. Public anonymous download does not establish a reviewed commercial redistribution entitlement; preserve that unresolved license instead of assigning CC BY or CC0.".into(),
    ];
    gl3.limits = limits();

    let mut gedi = base(
        "gedi-l3-canopy",
        "GEDI L3 V2 · Mean RH100 canopy height",
        "vegetation",
        "https://portal.opentopography.org/datasetMetadata?otCollectionID=OT.032022.4326.1",
    );
    gedi.description = "Mean canopy height from GEDI L3 Version 2 laser-footprint RH100 metrics accumulated from 2019-04-18 to 2022-01-19, distributed as one complete OpenTopography-reprojected numerical COG. This published EPSG:4326 derivative is not NASA's original EASE-Grid file, mission waveform/HDF, an individual-tree height measurement, biomass or a current 10 m canopy map.".into();
    gedi.version =
        "GEDI03 rh100_mean 2019108_2022019_002_03 · OpenTopography EPSG4326 derivative".into();
    gedi.nominal_resolution =
        "Source L3 nominal 1 km cells; delivered 0.010573500688852589 degree EPSG:4326 grid".into();
    gedi.unit = "mean RH100 height above ground in metres".into();
    gedi.license =
        "NASA EOSDIS Data Use Policy; retain NASA / UMD / ORNL DAAC / OpenTopography citation"
            .into();
    gedi.license_url = GEDI_POLICY.into();
    gedi.attribution = "Dubayah, R.O., S.B. Luthcke, T.J. Sabaka, J.B. Nicholas, S. Preaux and M.A. Hofton (2021), GEDI L3 Gridded Land Surface Metrics, Version 2, NASA / UMD / ORNL DAAC, doi:10.3334/ORNLDAAC/1952. OpenTopography EPSG:4326 distribution, doi:10.5069/G9V12301. Retain selected metric, actual accumulation interval, reprojection provenance and access date.".into();
    gedi.value_interpretation = "The Float32 original contains mean RH100 canopy heights above ground of valid GEDI footprints aggregated into L3 cells. Preserve scale 1, offset 0 and embedded NoData -9999. The reprojected original grid is 34047 x 16086 and extends about 85 degrees north/south, while the actual observed latitude range is approximately 52 degrees north/south and missing coverage remains NoData. A non-missing value is not a ground-truth or individual-tree accuracy guarantee.".into();
    gedi.layers = vec![scaled(layer("rh100-mean", "Mean GEDI RH100 canopy height · 2019-2022", "m above ground", "Fixed V2 2019108_2022019_002_03 OpenTopography-reprojected L3 Float32 original. Original scale 1, offset 0, NoData -9999. No automatic masking by footprint count, uncertainty estimation or conversion to biomass; these separate metrics are not included."), 1., 0., -9999.)];
    gedi.default_layer = Some("rh100-mean".into());
    gedi.warnings = vec![
        "This exact delivered file has been reprojected by OpenTopography from the NASA L3 source. It is not an untouched mission original or a new software export. The app downloads and inspects those exact published bytes without further reprojection, resampling or clipping.".into(),
        "Only mean RH100 for the cumulative 2019-04-18 to 2022-01-19 interval is connected. Ground elevation, standard deviation, footprint counts, newer accumulation periods and GEDI Version 3 are distinct metrics/products and are not substituted. A reference publication date is not an acquisition instant.".into(),
        "The broad EPSG:4326 file footprint includes large missing polar and ocean areas. Valid footprints are limited approximately to 52 degrees north/south and are spatially incomplete; a map intersection, populated pixel or grayscale preview does not establish continuous observed coverage or tree-level measurement.".into(),
        "The complete published file is 407,711,565 bytes regardless of the selected AOI. Existing transfer and inspection limits remain unchanged. Keep raw Float32 precision and -9999 NoData; do not fill gaps, extrapolate polar canopy, apply a new mask or claim 10 m accuracy.".into(),
        "The actual west outer edge is -180.000000000002. Only the legal client query footprint intersects longitude at -180; the original transform, outer edges, sample mapping and bytes remain unchanged.".into(),
    ];
    gedi.limits = limits();

    let mut relief = base(
        "srtm15plus-bathymetry",
        "SRTM15+ V2.5.5 · Land and ocean elevation",
        "elevation",
        "https://portal.opentopography.org/datasetMetadata?otCollectionID=OT.122019.4326.1",
    );
    relief.description = "Published SRTM15+ Version 2.5.5 global topography and bathymetry numerical tiles, combining land elevation with ship soundings and satellite-derived predicted ocean depths. This is the Scripps SRTM15+ product distributed by OpenTopography, distinct from ETOPO, GEBCO, SRTM GL3, a newly measured seabed survey or an API-generated AOI export.".into();
    relief.version = "SRTM15+ V2.5.5 · OpenTopography published 91-file original tile index".into();
    relief.nominal_resolution =
        "15 arc-seconds (0.004166666666666667 degree), about 500 m at the equator".into();
    relief.unit = "metres, publisher WGS84 / EGM96 vertical-reference declaration".into();
    relief.license =
        "OpenTopography collection: Not Provided; SRTM15+ source citation required".into();
    relief.license_url = relief.documentation_url.clone();
    relief.attribution = "Tozer, B., D.T. Sandwell, W.H.F. Smith, C. Olson, J.R. Beale and P. Wessel (2019), Global bathymetry and topography at 15 arc sec: SRTM15+, Earth and Space Science, doi:10.1029/2019EA000658. Version 2.5.5 distributed by OpenTopography, doi:10.5069/G92R3PT9. Retain release, actual original tile, mixed data provenance and access date; no undeclared Creative Commons license is assigned.".into();
    relief.value_interpretation = "Preserve the published Float32 elevation grid with scale 1, offset 0 and embedded NaN NoData. Ocean elevations can be negative, land elevations can also be below sea level, and zero is a valid sample. The checked 02_10 original has valid fractional negative values and no NaN pixels. Published tile edges include non-integer offsets; actual TIFF transforms, not nominal 30-degree filename rectangles, govern the source grid.".into();
    relief.layers = vec![layer("surface", "SRTM15+ V2.5.5 elevation and bathymetry", "m", "Original Float32 elevation samples, scale 1 and offset 0, embedded NaN NoData. Do not cast to integer, negate all values into positive water depth, mask negative land or zero, or substitute GEBCO/ETOPO/SID source-identification classes. Scientific vertical reference is retained from the publisher, not inferred from horizontal EPSG:4326.")];
    relief.default_layer = Some("surface".into());
    relief.warnings = vec![
        "Only the 91 actual SRTM15_V2.5.5 files in the publisher index are eligible. Each file's original TIFF header and physical footprint was independently checked. Some boundary strips are smaller than typical 7200 x 7200 tiles, and tile edges do not lie on integer 30-degree boundaries; no nominal footprint replaces the actual original grid.".into(),
        "This combines historical land DEMs, ship soundings and predicted ocean depths from satellite altimetry. A 15 arc-second grid is not a claim that every seafloor cell is directly surveyed or accurate at 500 m. Negative elevation alone does not distinguish ocean, inland water and below-sea-level land.".into(),
        "The publisher enabled Version 2.5.5 in November 2023 and describes source surveys spanning 2000-2023. The selected release is fixed; month/year filters do not create current bathymetric observations. No V2.1, GEBCO, ETOPO or generated subset is used as a fallback.".into(),
        "OpenTopography states Use License: Not Provided and requires dataset/source acknowledgement. Public availability alone does not establish a reviewed commercial redistribution entitlement. Preserve that unknown license and the SRTM15+ citation; do not label the product CC BY or CC0.".into(),
        "Only the legal query footprint is intersected with geographic longitude/latitude bounds. The original edge strips, bounds, Float32 fractional values, NaN declaration and actual file bytes are preserved unchanged. Online display and valid scientific coverage are separate acceptance steps.".into(),
    ];
    relief.limits = limits();
    vec![gl3, gedi, relief]
}

fn physical(bounds: [f64; 4]) -> [f64; 4] {
    [
        bounds[0].max(-180.),
        bounds[1].max(-90.),
        bounds[2].min(180.),
        bounds[3].min(90.),
    ]
}
fn intersects(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] < b[2] && a[2] > b[0] && a[1] < b[3] && a[3] > b[1]
}
fn named_tile(path: &str) -> Result<(&str, [f64; 4])> {
    let stem = path
        .rsplit('/')
        .next()
        .and_then(|v| v.strip_suffix(".tif"))
        .ok_or("Invalid reviewed SRTMGL3 filename")?;
    let b = stem.as_bytes();
    if b.len() != 7
        || !matches!(b[0], b'N' | b'S')
        || !matches!(b[3], b'E' | b'W')
        || !b[1..3].iter().all(u8::is_ascii_digit)
        || !b[4..7].iter().all(u8::is_ascii_digit)
    {
        return Err("Invalid reviewed SRTMGL3 tile coordinate".into());
    }
    let lat = stem[1..3]
        .parse::<i32>()
        .map_err(|_| "Invalid reviewed tile latitude")?
        * if b[0] == b'S' { -1 } else { 1 };
    let lon = stem[4..7]
        .parse::<i32>()
        .map_err(|_| "Invalid reviewed tile longitude")?
        * if b[3] == b'W' { -1 } else { 1 };
    let half = 1. / 2400.;
    Ok((
        stem,
        physical([
            f64::from(lon) - half,
            f64::from(lat) - half,
            f64::from(lon + 1) + half,
            f64::from(lat + 1) + half,
        ]),
    ))
}
fn original(id: &str, title: String, path: &str, bounds: [f64; 4]) -> DatasetSource {
    let mut file = source(title, format!("{HTTPS}{path}"), Some(physical(bounds)));
    file.id = id.into();
    file
}

pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    features::bounds(request.bounds)?;
    if request.year.is_some()
        || request.month.is_some()
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
    {
        return Err("These fixed published products have no year, month, country, named region or season selector; no alternate observation period or generated subset is substituted".into());
    }
    let mut files = Vec::new();
    match (request.dataset_id.as_str(), request.layer.as_deref()) {
        ("srtm-gl3", Some("elevation")) => {
            for path in &INDEX.gl3_paths {
                let (tile, bounds) = named_tile(path)?;
                if intersects(bounds, request.bounds) {
                    files.push(original(tile, format!("SRTMGL3 V3 {tile} · 3 arc-second original"), path, bounds));
                }
            }
        }
        ("gedi-l3-canopy", Some("rh100-mean")) => {
            if intersects(GEDI_QUERY_BOUNDS, request.bounds) {
                files.push(original("GEDI03_rh100_mean_2019108_2022019_002_03_EPSG4326", "GEDI L3 V2 mean RH100 · OpenTopography EPSG:4326 published derivative".into(), GEDI_PATH, GEDI_QUERY_BOUNDS));
            }
            if GEDI_BYTES > 512 * 1024 * 1024 {
                return Err("The reviewed GEDI original exceeds the existing transfer limit".into());
            }
        }
        ("srtm15plus-bathymetry", Some("surface")) => {
            for item in &INDEX.srtm15_tiles {
                let bounds = physical(item.bounds);
                if intersects(bounds, request.bounds) {
                    if item.bytes == 0 || item.bytes > 512 * 1024 * 1024 {
                        return Err("A reviewed SRTM15+ original exceeds the existing complete-file transfer limit".into());
                    }
                    let id = item.path.rsplit('/').next().and_then(|v| v.strip_suffix(".tif")).ok_or("Invalid reviewed SRTM15+ filename")?;
                    files.push(original(id, format!("SRTM15+ V2.5.5 {id} · original land and ocean grid"), &item.path, bounds));
                }
            }
        }
        _ => return Err("Choose the exact reviewed SRTMGL3 elevation, GEDI mean RH100 or SRTM15+ surface layer; no quality band, ground-height metric, new release or other provider is substituted".into()),
    }
    if files.is_empty() {
        return Err("The map area does not intersect an actual listed published file for this fixed product; no different area, sensor, level or fallback product is selected".into());
    }
    if files.len() > MAX_TILES {
        return Err("The map area intersects more than 8 published original files; narrow the area instead of silently dropping tiles".into());
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
    fn finite_archive_and_point_registration_preserve_exact_public_paths() {
        assert_eq!(INDEX.gl3_paths.len(), 14280);
        assert_eq!(INDEX.srtm15_tiles.len(), 91);
        let plan = resolve(&request(
            "srtm-gl3",
            "elevation",
            [139.3, 35.4, 139.5, 35.6],
        ))
        .unwrap();
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].id, "N35E139");
        assert_eq!(
            plan[0].stac.as_ref().unwrap().url,
            format!("{HTTPS}SRTM_GL3/SRTM_GL3_srtm/North/North_30_60/N35E139.tif")
        );
        assert_eq!(
            plan[0].bounds,
            Some([
                139. - 1. / 2400.,
                35. - 1. / 2400.,
                140. + 1. / 2400.,
                36. + 1. / 2400.
            ])
        );
        assert!(plan[0].search.is_none() && plan[0].catalog_raster.is_none());
        let boundary = resolve(&request(
            "srtm-gl3",
            "elevation",
            [139.2, 35.9998, 139.4, 36.0002],
        ))
        .unwrap();
        assert!(
            boundary.iter().any(|v| v.id == "N35E139")
                && boundary.iter().any(|v| v.id == "N36E139")
        );
    }
    #[test]
    fn gedi_is_one_fixed_published_derivative_and_does_not_extrapolate() {
        let plan = resolve(&request(
            "gedi-l3-canopy",
            "rh100-mean",
            [116.3, 39.8, 116.5, 40.],
        ))
        .unwrap();
        assert_eq!(plan.len(), 1);
        assert_eq!(
            plan[0].stac.as_ref().unwrap().url,
            format!("{HTTPS}{GEDI_PATH}")
        );
        assert_eq!(plan[0].bounds, Some(GEDI_QUERY_BOUNDS));
        assert!(plan[0].search.is_none() && plan[0].start_datetime.is_none());
        assert!(resolve(&request(
            "gedi-l3-canopy",
            "rh100-mean",
            [10., 88., 11., 89.]
        ))
        .is_err());
        let dataset = definitions()
            .into_iter()
            .find(|v| v.id == "gedi-l3-canopy")
            .unwrap();
        assert_eq!(dataset.layers[0].nodata, Some(-9999.));
        assert!(dataset.description.contains("reprojected"));
    }
    #[test]
    fn srtm15_uses_non_nominal_actual_bounds_and_retains_float_nan_declaration() {
        let plan = resolve(&request(
            "srtm15plus-bathymetry",
            "surface",
            [116.3, 39.8, 116.5, 40.],
        ))
        .unwrap();
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].id, "SRTM15_V2.5.5_02_10");
        let bounds = plan[0].bounds.unwrap();
        assert_ne!(bounds, [90., 30., 120., 60.]);
        assert!(bounds[0] < 90. && bounds[2] < 120.);
        let dataset = definitions()
            .into_iter()
            .find(|v| v.id == "srtm15plus-bathymetry")
            .unwrap();
        assert!(dataset.layers[0].nodata.is_none());
        assert!(dataset.value_interpretation.contains("NaN"));
        assert!(dataset.license.contains("Not Provided"));
    }
    #[test]
    fn forbidden_selectors_wrong_layers_invalid_and_excessive_areas_fail() {
        for (id, layer) in [
            ("srtm-gl3", "elevation"),
            ("gedi-l3-canopy", "rh100-mean"),
            ("srtm15plus-bathymetry", "surface"),
        ] {
            let original = request(id, layer, [116.3, 39.8, 116.5, 40.]);
            let mut selected = original.clone();
            selected.year = Some(2026);
            assert!(resolve(&selected).is_err());
            selected = original.clone();
            selected.month = Some(10);
            assert!(resolve(&selected).is_err());
            selected = original.clone();
            selected.country = Some("CHN".into());
            assert!(resolve(&selected).is_err());
            selected = original.clone();
            selected.region = Some("sample".into());
            assert!(resolve(&selected).is_err());
            selected = original.clone();
            selected.season = Some("summer".into());
            assert!(resolve(&selected).is_err());
            selected = original.clone();
            selected.layer = Some("quality".into());
            assert!(resolve(&selected).is_err());
            selected = original;
            selected.bounds = [-181., 0., -179., 1.];
            assert!(resolve(&selected).is_err());
        }
        assert!(resolve(&request("srtm-gl3", "elevation", [-100., 30., -80., 40.])).is_err());
        assert!(resolve(&request(
            "srtm15plus-bathymetry",
            "surface",
            [-180., -80., 180., 80.]
        ))
        .is_err());
        assert!(resolve(&request("srtm-gl3", "elevation", [0., 88., 1., 89.])).is_err());
    }
}
