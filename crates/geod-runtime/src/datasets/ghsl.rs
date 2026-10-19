//! One independently decoded JRC GHS-SMOD V2 / 2020 WGS84 original.
//! This is a direct-file client recipe, not invented publisher STAC.
//! Query/display longitude bounds are distinguished from the unchanged
//! original expanded grid and its two outer longitude columns.
use super::*;

pub(crate) const SMOD_2020_URL: &str = "https://jrc-ghsl.s3.eu-central-1.amazonaws.com/ghs-smod/r2023a/4326/30asec/2020/GHS_SMOD_E2020_GLOBE_R2023A_4326_30ss_V2_0.tif";
pub(crate) const SMOD_2020_SHA256: &str =
    "20661ac0f98166bd2253ce1155e131d589b0100fc2d5eed9dc7c632228071416";
pub(crate) const SMOD_2020_BYTES: u64 = 20_566_541;
// Physical WGS84 intersection, used only to organize a legal AOI/query.
// Original PixelIsArea bounds remain:
// [-180.00791593130032, -89.10041610517152,
//   180.00874930942342,  89.0995831776456].
pub(crate) const QUERY_BOUNDS: [f64; 4] = [-180., -89.10041610517152, 180., 89.0995831776456];

pub(super) fn definition() -> Dataset {
    let mut dataset = base(
        "ghsl-smod",
        "GHSL GHS-SMOD settlement classes · 2020",
        "population",
        "https://human-settlement.emergency.copernicus.eu/ghs_smod2023.php",
    );
    dataset.description = "One complete JRC GHS-SMOD R2023A V2 settlement-classification original for reference epoch 2020, openly distributed by JRC through its public AWS archive. The Degree of Urbanisation stage I model combines GHS-BUILT-S R2023 and GHS-POP R2023 / GPW v4.11. This is a modeled settlement typology, not a satellite scene, general land-cover map, building footprint, measured city boundary or population count.".into();
    dataset.version =
        "GHS-SMOD R2023A V2.0 · EPSG:4326 30ss · reference epoch 2020 · reviewed 2026-10-12".into();
    dataset.license = "CC BY 4.0".into();
    dataset.license_url = CC_BY.into();
    dataset.nominal_resolution = "30 arc-seconds (approximately 0.0083333333 degree), about 1 km at the equator; ground cell distance varies with latitude".into();
    dataset.unit = "settlement typology class code (Degree of Urbanisation level 2)".into();
    dataset.years = vec![2020];
    dataset.default_year = Some(2020);
    dataset.attribution = "European Commission, Joint Research Centre (JRC); Schiavina M., Melchiorri M., Pesaresi M. (2023), GHS-SMOD R2023A, doi:10.2905/A0DF7A6F-49DE-46EA-9BDE-563437A6E2BA. Reviewed original: V2.0, WGS84 30ss, reference epoch 2020. JRC-managed distribution: https://registry.opendata.aws/ghs-smod/. Retain the license, source, access date and an indication of any changes.".into();
    dataset.value_interpretation = "Preserve the original signed Int16 level-2 class codes: 10 water grid cell; 11 very low density rural; 12 low density rural; 13 rural cluster; 21 suburban or peri-urban; 22 semi-dense urban cluster; 23 dense urban cluster; 30 urban centre. Original typed NoData is -200. The reviewed original also contains code 0, which is not explained by the publisher's legend: preserve it as an unclassified/undocumented raw code, without assigning a settlement class or replacing its mask. These categories are not continuous quantities or population counts.".into();
    let mut classification = layer(
        "settlement-class",
        "Settlement classes · level 2 · reference epoch 2020",
        "Degree of Urbanisation class code",
        "Original Int16 class codes 10, 11, 12, 13, 21, 22, 23 and 30; typed NoData -200. Code 0 occurs in the actual original but has no documented class in the publisher's legend and is not the embedded NoData. No automatic class aggregation, palette invention or population conversion is applied.",
    );
    classification.scale = Some(1.);
    classification.offset = Some(0.);
    classification.nodata = Some(-200.);
    dataset.layers = vec![classification];
    dataset.default_layer = Some("settlement-class".into());
    dataset.warnings = vec![
        "Only the independently decoded reference epoch 2020, GHS-SMOD R2023A V2.0 WGS84 30ss original is supported. Other epochs, version 1, Mollweide grids, urban-centre ID layers and other GHSL products are not substituted. The publisher's 2025/2030 epochs are outside this historical connector.".into(),
        "2020 identifies a modeled reference epoch, not the acquisition date of one satellite observation. The whole published global original is downloaded, without AOI clipping or resampling. A legal selected map area must intersect the distribution; this does not guarantee useful valid-pixel settlement coverage.".into(),
        "The independently read original is PixelIsArea, EPSG:4326, 43,202 × 21,384 signed Int16 samples. Its exact outer bounds are [-180.00791593130032, -89.10041610517152, 180.00874930942342, 89.0995831776456]. Its two outer longitude columns have cell centres outside ±180 degrees. The client recipe's query/display footprint is the intersection with physical WGS84 bounds; original bytes, full extent, transform, grid, column samples, values and NoData remain unchanged. Map display fitting is not scientific cropping or a new publisher extent.".into(),
        "The publisher's machine-readable product table describes int8, while this actual WGS84 V2 original is signed Int16 with representable -200 NoData. The physical TIFF is authoritative for raw storage and mask inspection. No in-file palette is present; a grayscale preview is not the publisher's categorical legend.".into(),
        "The full decoded original contains 10,479 raw code-0 pixels, which are not documented in the publisher's level-2 legend. They are not automatically treated as NoData, water or any rural/urban class. Embedded NoData -200 and valid water code 10 remain separate.".into(),
        "CC BY 4.0 is declared in the JRC-managed AWS registry and the publisher's copyright notice. The direct-file recipe is client provenance, not a fabricated publisher STAC collection/item. The S3 ETag is multipart and is not a whole-file MD5 checksum; the independently measured SHA-256 is recorded separately.".into(),
    ];
    dataset.limits = vec![
        "One complete reviewed WGS84 V2.0 / reference epoch 2020 original per selection; no country, region, month or seasonal parameter is supported.".into(),
        format!("The independently decoded original is {SMOD_2020_BYTES} bytes with SHA-256 {SMOD_2020_SHA256}; all 923,831,568 original pixels were independently decoded. Existing 512 MiB transfer, native-axis/grid, chunk/intermediate/preview-budget and deadline guards remain unchanged."),
    ];
    dataset
}

pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    features::bounds(request.bounds)?;
    if request.dataset_id != "ghsl-smod"
        || request.year != Some(2020)
        || request.month.is_some()
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
        || request.layer.as_deref() != Some("settlement-class")
    {
        return Err("Choose only GHS-SMOD R2023A V2.0 WGS84 settlement-class / reference epoch 2020, without month, country, region or season parameters; other GHSL products and prediction epochs are not substituted".into());
    }
    let b = request.bounds;
    if b[0] >= QUERY_BOUNDS[2]
        || b[2] <= QUERY_BOUNDS[0]
        || b[1] >= QUERY_BOUNDS[3]
        || b[3] <= QUERY_BOUNDS[1]
    {
        return Err("The requested map area does not intersect the physical footprint of the reviewed GHS-SMOD WGS84 original; no polar sample area or different epoch is substituted".into());
    }
    let original = source(
        "JRC GHS-SMOD V2.0 · level 2 · reference epoch 2020".into(),
        SMOD_2020_URL.into(),
        Some(QUERY_BOUNDS),
    );
    // A reference epoch is not a publisher acquisition interval. The recipe
    // retains it through request.year and definition text, without fabricating
    // start/end acquisition timestamps for the direct original.
    Ok(vec![original])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ResolveRequest {
        ResolveRequest {
            dataset_id: "ghsl-smod".into(),
            bounds: [116.3, 39.8, 116.5, 40.],
            year: Some(2020),
            month: None,
            country: None,
            layer: Some("settlement-class".into()),
            region: None,
            season: None,
        }
    }

    #[test]
    fn exact_smod_direct_recipe_preserves_2020_classification_and_typed_mask() {
        let r = request();
        let sources = resolve(&r).unwrap();
        assert_eq!(sources.len(), 1);
        let s = &sources[0];
        assert_eq!(s.id, "GHS_SMOD_E2020_GLOBE_R2023A_4326_30ss_V2_0");
        assert_eq!(s.stac.as_ref().unwrap().kind, "raster");
        assert_eq!(s.stac.as_ref().unwrap().url, SMOD_2020_URL);
        assert_eq!(s.bounds, Some(QUERY_BOUNDS));
        assert_eq!(r.bounds, [116.3, 39.8, 116.5, 40.]);
        assert!(s.start_datetime.is_none() && s.end_datetime.is_none());
        let d = definition();
        assert_eq!(d.years, vec![2020]);
        assert!(d.months.is_empty() && d.regions.is_empty() && d.seasons.is_empty());
        assert!(d.discovery.is_none());
        assert_eq!(d.layers[0].nodata, Some(-200.));
        assert_eq!(d.layers[0].scale, Some(1.));
        assert_eq!(d.layers[0].offset, Some(0.));
        assert!(d.warnings.iter().any(|w| w.contains("10,479")));
        assert!(d.warnings.iter().any(|w| w.contains("int8")));
        assert!(d.warnings.iter().any(|w| w.contains("remain unchanged")));
        assert!(d.limits.iter().any(|l| l.contains(SMOD_2020_SHA256)));
        assert_eq!(d.license, "CC BY 4.0");
    }

    #[test]
    fn other_epochs_projections_and_temporal_spatial_parameters_fail() {
        for year in [1975, 2000, 2015, 2025, 2030] {
            let mut r = request();
            r.year = Some(year);
            assert!(resolve(&r).is_err());
        }
        let mut r = request();
        r.year = None;
        assert!(resolve(&r).is_err());
        let mut r = request();
        r.month = Some(1);
        assert!(resolve(&r).is_err());
        let mut r = request();
        r.country = Some("CHN".into());
        assert!(resolve(&r).is_err());
        let mut r = request();
        r.region = Some("globe".into());
        assert!(resolve(&r).is_err());
        let mut r = request();
        r.season = Some("annual".into());
        assert!(resolve(&r).is_err());
        for layer in ["population", "built-up", "settlement-id", "54009", "v1"] {
            let mut r = request();
            r.layer = Some(layer.into());
            assert!(resolve(&r).is_err());
        }
    }

    #[test]
    fn invalid_or_outside_physical_aoi_is_rejected_without_replacement() {
        for bounds in [
            [-181., 0., -179., 1.],
            [179.9, 0., 180.1, 1.],
            [0., 89.2, 1., 90.],
            [0., -90., 1., -89.2],
            [1., 1., 0., 2.],
        ] {
            let mut r = request();
            r.bounds = bounds;
            assert!(resolve(&r).is_err());
        }
        let mut r = request();
        r.bounds = [-180., 0., -179.9, 0.1];
        assert!(resolve(&r).is_ok());
        r.bounds = [179.9, 0., 180., 0.1];
        assert!(resolve(&r).is_ok());
    }

    #[test]
    fn ocean_selection_keeps_water_class_and_does_not_claim_city_coverage() {
        let mut r = request();
        r.bounds = [0., 0., 0.1, 0.1];
        assert!(resolve(&r).is_ok());
        let d = definition();
        assert!(d.value_interpretation.contains("10 water"));
        assert!(d.value_interpretation.contains("not explained"));
        assert!(d.warnings.iter().any(|w| w.contains("does not guarantee")));
    }
}
