//! ETH's fixed author-published 2020 canopy-height numerical GeoTIFFs.
//! The finite tile footprints and hrefs come from the unchanged FeatureCollection
//! embedded in the authors' tile browser. This is client provenance, not STAC.
use super::*;
use serde::Deserialize;
use std::sync::LazyLock;

pub(crate) const DATASET_ID: &str = "eth-canopy-height";
pub(crate) const TILE_BROWSER: &str =
    "https://langnico.github.io/globalcanopyheight/assets/tile_index.html";
const DOCUMENTATION: &str = "https://langnico.github.io/globalcanopyheight/";
const INDEX_HTML_SHA256: &str = "e140cda0253ab46cd268c27ed6122d39d116dbb0be0010419f3e9d65b4e01903";
const PREFIX: &str =
    "https://libdrive.ethz.ch/index.php/s/cO8or7iOe5dT2Rt/download?path=%2F3deg_cogs&files=";

#[derive(Deserialize)]
struct PublishedIndex {
    #[serde(rename = "type")]
    kind: String,
    features: Vec<PublishedFeature>,
}
#[derive(Deserialize)]
struct PublishedFeature {
    bbox: [f64; 4],
    properties: PublishedProperties,
}
#[derive(Deserialize)]
struct PublishedProperties {
    href_canopy_height: String,
    tile_name: String,
}
static INDEX: LazyLock<PublishedIndex> = LazyLock::new(|| {
    let value: PublishedIndex =
        serde_json::from_str(include_str!("next-eth-canopy-reviewed-index.json"))
            .expect("Reviewed original ETH tile-browser FeatureCollection must parse");
    assert_eq!(value.kind, "FeatureCollection");
    assert_eq!(value.features.len(), 2651);
    for feature in &value.features {
        features::bounds(feature.bbox).expect("ETH published tile bounds must be geographic");
        assert_eq!(
            feature.properties.href_canopy_height,
            format!("{PREFIX}{}", feature.properties.tile_name)
        );
        assert!(feature
            .properties
            .tile_name
            .starts_with("ETH_GlobalCanopyHeight_10m_2020_"));
        assert!(feature.properties.tile_name.ends_with("_Map.tif"));
    }
    value
});

pub(crate) fn definitions() -> Vec<Dataset> {
    let mut product = base(
        DATASET_ID,
        "ETH global canopy height · 2020",
        "vegetation",
        DOCUMENTATION,
    );
    product.description = "Author-published dense canopy top-height predictions for the reference year 2020, estimated from Sentinel-2 imagery by a probabilistic deep-learning ensemble trained with GEDI RH98 canopy top-height supervision. This is the ETH Global Canopy Height 2020 model product, separate from measured GEDI footprints, GEDI L3 mean RH100, forest classification, biomass and ground elevation.".into();
    product.version =
        "ETH Global Canopy Height 2020 v1 · fixed 2,651-file CHM tile-browser index".into();
    product.nominal_resolution = "Nominal 10 m ground sampling distance; published 0.3 arc-second (0.00008333333333333333 degree) EPSG:4326 grid".into();
    product.unit = "metres of predicted canopy top height".into();
    product.license = "CC BY 4.0".into();
    product.license_url = CC_BY.into();
    product.attribution = "Lang, N., Jetz, W., Schindler, K., and Wegner, J.D. (2023), A high-resolution canopy height model of the Earth, Nature Ecology & Evolution, doi:10.1038/s41559-023-02206-6; author-published ETH Global Canopy Height 2020, doi:10.3929/ethz-b-000609802. Retain authors, model reference year, selected original tile, CC BY 4.0, data and article citations, and access date.".into();
    product.value_interpretation = "Preserve the published single-band UInt8 height samples, scale 1 and offset 0, with embedded NoData 255. Zero is a valid stored prediction and must not be masked as missing. These are integer-metre model outputs, not one-metre measurement accuracy, optical reflectance, raw GEDI waveform heights or per-tree surveys.".into();
    product.years = vec![2020];
    product.default_year = Some(2020);
    product.layers = vec![scaled(
        layer("height", "Predicted canopy top height", "m", "Original UInt8 model-predicted canopy top height in metres; scale 1, offset 0, NoData 255. Retain valid zero and the full published geographic grid; do not apply canopy-class masks, infer measurement uncertainty from height, convert RH98 predictions into RH100 measurements, or claim current-year tree heights."),
        1., 0., 255.,
    )];
    product.default_layer = Some("height".into());
    product.warnings = vec![
        "Only the fixed 2020 CHM product is connected. The separately published STD/uncertainty files and other canopy products are not substituted or treated as additional observation dates. A reference year is not the acquisition date of every training or input image.".into(),
        "The author-provided 3-degree tiles are EPSG:4326 with 36,000 × 36,000 pixels, approximately 0.3 arc-second spacing. The nominal 10 m GSD does not make all geographic cells exactly 10 m square, and this delivery is not an untouched UTM Sentinel-2 tile. Original file grids and values remain unchanged.".into(),
        "The model predicts a wall-to-wall canopy top-height field from optical imagery with sparse GEDI supervision. A numerical pixel or grayscale preview does not establish measured laser coverage, individual-tree accuracy, correct water/non-forest classification or biomass. Retain the paper's prediction and uncertainty limitations.".into(),
        FULL_TILE.into(),
        format!("Tile candidates use only the 2,651 actual CHM hrefs and declared geographic footprints extracted from the unchanged author tile browser {TILE_BROWSER}; source HTML SHA-256 {INDEX_HTML_SHA256}. This is an application-reviewed client index, not publisher STAC or proof that every complete original has been accepted."),
        "Downloads go directly to the authors' public ETH libdrive delivery. The original file and embedded metadata are preserved; no Google Earth Engine export, account credential, paid compute, generated AOI subset, mirror or new resampling is used.".into(),
    ];
    product.limits = vec![
        "At most 8 actual published original tiles per selection; larger requests fail instead of silently dropping candidates.".into(),
        "The existing 512 MiB complete-file transfer limit, 131,072-pixel tiled axis limit, 8-billion-pixel grid limit and bounded native decoder remain unchanged. Some published complete tiles may exceed transfer limits; a source listing is not proof of full-file acceptance.".into(),
    ];
    vec![product]
}

pub(crate) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    features::bounds(request.bounds)?;
    if request.dataset_id != DATASET_ID
        || request.layer.as_deref() != Some("height")
        || request.year.is_some_and(|year| year != 2020)
        || request.month.is_some()
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
    {
        return Err("Choose only ETH's fixed 2020 predicted canopy-height layer; monthly scenes, STD uncertainty, other canopy metrics, other years and named-country exports are not substituted".into());
    }
    let [west, south, east, north] = request.bounds;
    let mut sources = Vec::new();
    for feature in &INDEX.features {
        let [w, s, e, n] = feature.bbox;
        if west < e && east > w && south < n && north > s {
            let name = &feature.properties.tile_name;
            let id = name
                .strip_prefix("ETH_GlobalCanopyHeight_10m_2020_")
                .and_then(|v| v.strip_suffix("_Map.tif"))
                .ok_or("The reviewed ETH published CHM identity is invalid")?;
            let mut selected = source(
                format!("ETH canopy height 2020 {id} · original CHM"),
                feature.properties.href_canopy_height.clone(),
                Some(feature.bbox),
            );
            selected.id = id.into();
            sources.push(selected);
        }
    }
    if sources.is_empty() {
        return Err("The current map area does not intersect an actual listed ETH 2020 CHM original; no different region, source, prediction year or generated subset is substituted".into());
    }
    if sources.len() > MAX_TILES {
        return Err("The map area intersects more than 8 published ETH CHM originals; narrow the area instead of silently dropping tiles".into());
    }
    Ok(sources)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(bounds: [f64; 4]) -> ResolveRequest {
        ResolveRequest {
            dataset_id: DATASET_ID.into(),
            bounds,
            year: Some(2020),
            month: None,
            country: None,
            layer: Some("height".into()),
            region: None,
            season: None,
        }
    }
    #[test]
    fn actual_published_index_selects_exact_originals_and_keeps_reference_year() {
        let result = resolve(&request([4.5, 52., 4.8, 52.3])).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "N51E003");
        assert_eq!(result[0].bounds, Some([3., 51., 6., 54.]));
        assert_eq!(
            result[0].stac.as_ref().unwrap().url,
            format!("{PREFIX}ETH_GlobalCanopyHeight_10m_2020_N51E003_Map.tif")
        );
        assert_eq!(result[0].stac.as_ref().unwrap().kind, "raster");
        assert!(result[0].stac.as_ref().unwrap().name.len() <= 80);
        assert!(result[0].search.is_none() && result[0].catalog_raster.is_none());
        assert!(result[0].start_datetime.is_none() && result[0].end_datetime.is_none());
        let boundary = resolve(&request([5.9, 51.2, 6.1, 51.4])).unwrap();
        assert_eq!(boundary.len(), 2);
        assert!(boundary.iter().any(|v| v.id == "N51E003"));
        assert!(boundary.iter().any(|v| v.id == "N51E006"));
        assert_eq!(INDEX.features.len(), 2651);
    }
    #[test]
    fn exact_product_units_grid_and_nodata_do_not_masquerade_as_measurements() {
        let product = definitions().remove(0);
        assert_eq!(product.years, vec![2020]);
        assert_eq!(product.default_year, Some(2020));
        assert_eq!(product.layers.len(), 1);
        assert_eq!(product.layers[0].nodata, Some(255.));
        assert_eq!(product.layers[0].scale, Some(1.));
        assert_eq!(product.layers[0].offset, Some(0.));
        assert_eq!(product.license, "CC BY 4.0");
        assert!(product.nominal_resolution.contains("0.3 arc-second"));
        assert!(product.description.contains("predictions"));
        assert!(product.description.contains("RH98"));
        assert!(product.value_interpretation.contains("Zero is a valid"));
        assert!(product.discovery.is_none());
    }
    #[test]
    fn wrong_dates_layers_excessive_and_absent_areas_fail_without_fallback() {
        let original = request([4.5, 52., 4.8, 52.3]);
        for bad in ["std", "rh100-mean", "biomass", "height-2026"] {
            let mut changed = original.clone();
            changed.layer = Some(bad.into());
            assert!(resolve(&changed).is_err());
        }
        let mut changed = original.clone();
        changed.year = Some(2021);
        assert!(resolve(&changed).is_err());
        changed = original.clone();
        changed.month = Some(1);
        assert!(resolve(&changed).is_err());
        changed = original.clone();
        changed.country = Some("NLD".into());
        assert!(resolve(&changed).is_err());
        assert!(resolve(&request([10., 87., 11., 88.])).is_err());
        assert!(resolve(&request([-180., -60., 180., 84.])).is_err());
        assert!(resolve(&request([4., 52., 3., 53.])).is_err());
    }
}
