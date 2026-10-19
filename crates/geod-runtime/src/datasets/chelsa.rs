//! Fixed CHELSA climate-moisture-index original, not fabricated publisher STAC.
//! The recipe preserves the actual climate normal, signed raw samples and
//! typed NoData. Its legal query/display footprint is explicitly distinguished
//! from the original TIFF's slightly expanded outer pixel edges.
use super::*;

pub(crate) const CMI_MEAN_URL: &str = "https://os.zhdk.cloud.switch.ch/chelsav2/GLOBAL/climatologies/1981-2010/bio/CHELSA_cmi_mean_1981-2010_V.2.1.tif";
pub(crate) const CMI_MEAN_SHA256: &str =
    "5aa9f5f1aa14f8671807a73181c13aa55217ab1a653081111e691fa89c6cd283";
pub(crate) const CMI_MEAN_BYTES: u64 = 244_959_851;
// Intersection of the independently read PixelIsArea outer footprint with
// physical WGS84 longitude/latitude. Original TIFF transform/bytes do not
// change. The west/south original edges are -180.00013888885002 and
// -90.00013888884999; all cell centres remain in the legal physical domain.
const QUERY_BOUNDS: [f64; 4] = [-180., -90., 179.99985967115003, 83.99986041515001];

pub(super) fn definition() -> Dataset {
    let mut dataset = base(
        "chelsa-climate",
        "CHELSA climate moisture index · 1981–2010 normal",
        "climate",
        "https://www.chelsa-climate.org/datasets/chelsa_climatologies",
    );
    dataset.description = "One fixed CHELSA V2.1 climate-moisture-index (CMI) mean original for the 1981–2010 climate normal, published by WSL on SWITCH public object storage. According to the authors' dataset paper, CMI is precipitation minus potential evapotranspiration. This historical modeled surface is not current weather, measured soil moisture, annual rainfall, a forecast or air temperature.".into();
    dataset.version =
        "CHELSA BIOCLIM+ cmi_mean V2.1 · fixed 1981–2010 climate normal · reviewed 2026-10-12"
            .into();
    dataset.license = "CC0 1.0".into();
    dataset.license_url = "https://creativecommons.org/publicdomain/zero/1.0/".into();
    // There is no single observation/reference year or monthly selector.
    // The exact 30-year interval is carried by the source and dataset text.
    dataset.nominal_resolution = "30 arc-seconds (0.0083333333 degree), approximately 1 km at the equator; ground cell distance varies with latitude".into();
    dataset.unit = "original Int16 CMI encoding; kg m^-2 month^-1 after embedded scale".into();
    dataset.attribution = "CHELSA / Swiss Federal Research Institute WSL; Brun, P., Zimmermann, N. E., Hari, C., Pellissier, L., and Karger, D. N. (2022), CHELSA-BIOCLIM+, doi:10.16904/envidat.332; dataset-description paper doi:10.5194/essd-14-5573-2022. Distribution: SWITCH public object storage. Preserve cmi_mean, V2.1, the actual 1981–2010 climate normal, source URL and access date.".into();
    dataset.value_interpretation = "Preserve original signed Int16 raw samples, embedded scale 0.1, offset 0 and representable NoData 32767. In the authors' paper, CMI = precipitation - potential evapotranspiration, in kg m^-2 month^-1; physical values require raw × 0.1, separately from raw inspection. Negative values represent a modeled climatic water deficit and positive values a surplus, not observed soil-water content or current drought severity. No automatic conversion, clipping, calibration or resampling is applied.".into();
    let mut cmi = layer(
        "cmi-mean",
        "Mean climate moisture index · 1981–2010 normal",
        "raw Int16; kg m^-2 month^-1 after declared scale",
        "Original signed CMI mean encoding. Embedded scale 0.1, offset 0, NoData 32767; keep negative samples and the original mask. The authors' paper defines CMI as precipitation minus potential evapotranspiration. It is a 30-year normal, not current soil moisture, rainfall alone, temperature or a single-year observation.",
    );
    cmi.scale = Some(0.1);
    cmi.offset = Some(0.);
    cmi.nodata = Some(32767.);
    dataset.layers = vec![cmi];
    dataset.default_layer = Some("cmi-mean".into());
    dataset.warnings = vec![
        "Only the fixed cmi_mean V2.1 / 1981–2010 original is reviewed. No year or month selection exists. Other climate variables, BIO1 temperature, periods, future scenarios or versions are not substituted; malformed UInt16 files with negative NoData declarations remain unsupported.".into(),
        "The entire published quasi-global original is downloaded without clipping to the requested AOI. A legal WGS84 AOI must intersect the actual distribution. This is a historical downscaled climate normal; it does not establish current conditions, station-level measurement accuracy or exact valid-pixel coverage of the map area.".into(),
        "The original is PixelIsArea, EPSG:4326, 43,200 × 20,880 signed Int16 strip samples with its actual ModelPixelScale/ModelTiepoint. Its outer west/south edges are -180.00013888885002/-90.00013888884999 (a small fraction of a cell beyond the physical world), while all cell centres lie inside. The client query footprint is the intersection with legal physical longitude/latitude; original TIFF bytes, outer extent, transform, grid and click-to-pixel mapping are not rewritten.".into(),
        "Embedded scale is 0.1 and offset is 0. Raw pixel reads remain signed integers: the independently checked Beijing value -323 corresponds to -32.3 kg m^-2 month^-1 only after the documented scale. NoData 32767 is excluded through the original typed declaration, not converted into a physical moisture value. A grayscale thumbnail is not a calibrated climate visualization or a scientific validation.".into(),
        "The author's current climatologies summary describes CMI as a ratio, but its unit is kg m^-2 month^-1 and the original authors' dataset paper explicitly defines precipitation minus potential evapotranspiration. This inconsistency is disclosed. Preserve raw data and consult the authoritative paper (doi:10.5194/essd-14-5573-2022, section 2.2.4) rather than interpreting these samples as a dimensionless ratio.".into(),
        "The independently decoded original includes both valid signed samples and 32767 NoData, including the checked 0-degree ocean position. A numerical value or lack of NoData at a point does not establish scientifically useful land/sea coverage. The original native mask is retained without inventing an ocean mask or a different product's sentinel.".into(),
        "CC0 is a separate current declaration on the author's exact CHELSA-climatologies page. This direct-file recipe is client provenance; no publisher STAC collection, item, license field or arbitrary mirror source is fabricated. Metadata connection and complete original transfer/inspection are separate stages.".into(),
    ];
    dataset.limits = vec![
        "One complete fixed cmi_mean original per selection. Only this single layer/period is supported, not every CHELSA variable or scenario.".into(),
        format!("The independently checked original is {CMI_MEAN_BYTES} bytes with SHA-256 {CMI_MEAN_SHA256}. Existing 512 MiB transfer, 65,536 stripped-axis / 131,072 tiled-axis, 8-billion grid-pixel, chunk/intermediate/preview-budget and deadline guards remain unchanged."),
    ];
    dataset
}

pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    features::bounds(request.bounds)?;
    if request.dataset_id != "chelsa-climate"
        || request.year.is_some()
        || request.month.is_some()
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
        || request.layer.as_deref() != Some("cmi-mean")
    {
        return Err("Choose only the fixed CHELSA cmi_mean V2.1 / 1981–2010 normal, without year, month, country, region or season parameters; no other variable or period is substituted".into());
    }
    let bounds = request.bounds;
    if bounds[0] >= QUERY_BOUNDS[2]
        || bounds[2] <= QUERY_BOUNDS[0]
        || bounds[1] >= QUERY_BOUNDS[3]
        || bounds[3] <= QUERY_BOUNDS[1]
    {
        return Err("The requested map area does not intersect the fixed CHELSA distribution's true physical extent; no northern polar grid, sample area or different period is substituted".into());
    }
    let mut original = source(
        "CHELSA CMI mean V2.1 · 1981–2010 normal".into(),
        CMI_MEAN_URL.into(),
        Some(QUERY_BOUNDS),
    );
    original.start_datetime = Some("1981-01-01T00:00:00Z".into());
    original.end_datetime = Some("2010-12-31T23:59:59Z".into());
    Ok(vec![original])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ResolveRequest {
        ResolveRequest {
            dataset_id: "chelsa-climate".into(),
            bounds: [116.3, 39.8, 116.5, 40.],
            year: None,
            month: None,
            country: None,
            layer: Some("cmi-mean".into()),
            region: None,
            season: None,
        }
    }

    #[test]
    fn cmi_direct_original_retains_normal_period_signed_scale_and_valid_typed_mask() {
        let sources = resolve(&request()).unwrap();
        assert_eq!(sources.len(), 1);
        let selected = &sources[0];
        assert_eq!(selected.id, "CHELSA_cmi_mean_1981-2010_V.2.1");
        assert_eq!(selected.stac.as_ref().unwrap().url, CMI_MEAN_URL);
        assert_eq!(selected.stac.as_ref().unwrap().kind, "raster");
        assert_eq!(selected.bounds, Some(QUERY_BOUNDS));
        assert_eq!(
            selected.start_datetime.as_deref(),
            Some("1981-01-01T00:00:00Z")
        );
        assert_eq!(
            selected.end_datetime.as_deref(),
            Some("2010-12-31T23:59:59Z")
        );
        assert!(selected.region.is_none() && selected.season.is_none());
        let dataset = definition();
        assert!(dataset.discovery.is_none());
        assert!(dataset.years.is_empty() && dataset.default_year.is_none());
        assert!(dataset.months.is_empty() && dataset.default_month.is_none());
        assert_eq!(dataset.layers[0].scale, Some(0.1));
        assert_eq!(dataset.layers[0].offset, Some(0.));
        assert_eq!(dataset.layers[0].nodata, Some(32767.));
        assert_eq!(dataset.license, "CC0 1.0");
        assert!(dataset
            .warnings
            .iter()
            .any(|warning| warning.contains("ratio")));
        assert!(dataset
            .warnings
            .iter()
            .any(|warning| warning.contains("not rewritten")));
    }

    #[test]
    fn fake_year_month_region_country_season_and_other_bio_variables_are_rejected() {
        for year in [1981, 2000, 2010, 2026] {
            let mut selected = request();
            selected.year = Some(year);
            assert!(resolve(&selected).is_err());
        }
        let mut selected = request();
        selected.month = Some(1);
        assert!(resolve(&selected).is_err());
        let mut selected = request();
        selected.country = Some("CHN".into());
        assert!(resolve(&selected).is_err());
        let mut selected = request();
        selected.region = Some("sample".into());
        assert!(resolve(&selected).is_err());
        let mut selected = request();
        selected.season = Some("winter".into());
        assert!(resolve(&selected).is_err());
        for layer in ["bio1", "bio12", "cmi-01", "swb"] {
            let mut selected = request();
            selected.layer = Some(layer.into());
            assert!(resolve(&selected).is_err());
        }
    }

    #[test]
    fn invalid_world_bounds_and_outside_northern_or_eastern_areas_fail() {
        for bounds in [
            [-180.1, -89., -179.9, -88.],
            [0., -90.1, 1., -89.9],
            [0., 85., 1., 86.],
            [179.9999, 39., 180., 40.],
        ] {
            let mut selected = request();
            selected.bounds = bounds;
            assert!(resolve(&selected).is_err());
        }
        let mut selected = request();
        selected.bounds = [-180., -90., -179.9, -89.9];
        assert!(resolve(&selected).is_ok());
    }

    #[test]
    fn ocean_intersection_has_no_fabricated_valid_pixel_coverage_claim() {
        let mut selected = request();
        selected.bounds = [0., 0., 0.1, 0.1];
        assert!(resolve(&selected).is_ok());
        assert!(definition()
            .warnings
            .iter()
            .any(|warning| warning.contains("scientifically useful land/sea coverage")));
    }
}
