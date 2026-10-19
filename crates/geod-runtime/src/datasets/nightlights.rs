//! Fixed author-published annual VNL-derived numerical original.
//! This is client provenance for one direct file, not fabricated publisher
//! STAC or an assertion that the processed DN are calibrated radiance.
use super::*;

// The record's file URL serves the same published bytes with the raster Accept
// contract. Its API /content route requires application/json even for a TIFF
// transfer and returns HTTP 406 with the native raster request headers.
pub(crate) const NIGHTLIGHTS_2024_URL: &str = "https://zenodo.org/records/17294744/files/nightlights.average_viirs.v21_m_500m_s_20240101_20241231_go_epsg4326_v20250904.tif";
pub(crate) const NIGHTLIGHTS_2024_BYTES: u64 = 63_713_625;
pub(crate) const NIGHTLIGHTS_2024_MD5: &str = "0b976892d40c2d0d99177c727289eaf2";
pub(crate) const NIGHTLIGHTS_2024_SHA256: &str =
    "e6edde68952052f8c53050e2ce7082bab08ddd0e15a696f999215e2d57194c53";
const SOURCE_ID: &str =
    "nightlights.average_viirs.v21_m_500m_s_20240101_20241231_go_epsg4326_v20250904";
// The actual PixelIsArea east edge is 180.00000000000028, a floating-point
// fraction beyond legal longitude. Only client query/display bounds intersect
// the physical domain; original outer edges, grid, transform and bytes remain.
const QUERY_BOUNDS: [f64; 4] = [-180., -62.000833333333446, 180., 87.37];

pub(super) fn definition() -> Dataset {
    let mut dataset = base(
        "openlandmap-nightlights",
        "OpenLandMap annual nighttime-light DN · 2024",
        "nightlights",
        "https://zenodo.org/records/17294744",
    );
    dataset.description = "One author-published Hengl / OpenGeoHub v0.4 annual processed nighttime-light numerical original for reference year 2024, derived from EOG VIIRS VNL annual inputs and distributed on Zenodo. This is processed DN, not an unmodified EOG radiance original, NASA Black Marble, an optical image, monthly observation or real-time light measurement. The separate backward-modeled 2000–2011 files are excluded.".into();
    dataset.version = "Hengl / OpenGeoHub Zenodo v0.4 · file v20250904 · fixed 2024 annual reference bin · reviewed 2026-10-12".into();
    dataset.license = "CC BY 4.0".into();
    dataset.license_url = "https://creativecommons.org/licenses/by/4.0/".into();
    dataset.attribution = "Hengl, T. / OpenGeoHub (2025), Annual time series of global VIIRS nighttime lights for 2000-2024 at 500-m spatial resolution extrapolated using logistic regression, v0.4, Zenodo, doi:10.5281/zenodo.17294744; processed annual VIIRS VNL source attributed to Earth Observation Group / Payne Institute. Preserve the actual author record, reference year 2024, file version v20250904, source URL, access date and CC BY 4.0 attribution.".into();
    dataset.nominal_resolution = "15 arc-seconds (0.0041666667 degree), nominally 500 m; ground cell distance varies with latitude".into();
    dataset.unit = "original processed Int16 DN; physical radiance calibration unresolved".into();
    dataset.value_interpretation = "Keep original signed Int16 processed DN, embedded TIFF scale 10, offset 0 and typed NoData -32768. The author record describes converting input 0–200 to 0–2000, while the delivered TIFF declares scale 10 and no unit. Physical radiance calibration remains unresolved. This conflict is not resolved by applying a guessed inverse or importing the older STAC collection's proportion unit. The software does not convert DN to calibrated nW/cm²/sr. Non-NoData is a storage-mask distinction, not a scientific validity guarantee; negative DN and 32767 remain original values rather than invented masks.".into();
    dataset.years = vec![2024];
    dataset.default_year = Some(2024);
    let mut annual = layer(
        "annual-dn",
        "Annual processed nighttime-light DN · 2024",
        "original processed DN; calibration unresolved",
        "One annual VNL-derived processed surface for 2024. Preserve signed Int16 samples, actual embedded scale 10 / offset 0 and NoData -32768. The author record's 0–200 to 0–2000 rescaling and the file's scale 10 are disclosed separately; neither is silently rewritten. Zero, negative non-NoData samples and 32767 are not automatically treated as missing observations, physical radiance or a normalized proportion. No observation-count or quality layer is substituted.",
    );
    annual.scale = Some(10.);
    annual.offset = Some(0.);
    annual.nodata = Some(-32768.);
    dataset.layers = vec![annual];
    dataset.default_layer = Some("annual-dn".into());
    dataset.warnings = vec![
        "Only the exact 2024 annual-dn file in the published Zenodo v0.4 record is reviewed. Years 2012–2023, older OpenLandMap STAC files, differences and principal-component layers are not substituted. The 2000–2011 backward logistic-model extrapolations are not observation years and are excluded.".into(),
        "Year 2024 is an annual reference interval, not a single acquisition timestamp. The record was published in 2025 and the filename retains v20250904; publication time is not used as the observation year. The original TIFF input-name tag identifies VNL_npp_2024_global_vcmslcfg_v2_c202502261200.average_masked.dat.tif.".into(),
        "Scientific declarations conflict: the author describes rescaling 0–200 to 0–2000, but the actual delivered TIFF embeds scale 10, offset 0 and no unit. Preserve both declarations and the raw samples; multiplying by 10 or a guessed 0.1 does not independently establish calibrated radiance. The older STAC unit proportion is not assigned to this different author release.".into(),
        "Independent all-pixel decoding found 3,299 negative non-NoData samples and 3,032 samples above 2000, with non-NoData DN ranging from -11 to 32767. Their scientific meaning is not established. The sole embedded scalar NoData is -32768; no negative-value filter, saturation mask, 32767 sentinel or new quality layer is invented.".into(),
        "The complete quasi-global original is downloaded without clipping or resampling to the selected AOI. A legal WGS84 AOI must intersect the actual distribution; overlap is not a claim that every requested pixel is observed or scientifically useful. Original zero remains non-NoData and is not equivalent to zero observations or certified absence of lights.".into(),
        "The actual original has EPSG:4326, PixelIsArea, 86,400 × 35,849 signed Int16 samples in 512 × 512 tiles, with 3,097,353,600 grid pixels. The actual east outer edge is 180.00000000000028; the client query footprint intersects legal longitude at 180 without rewriting original bounds, transform, bytes or pixel mapping.".into(),
        "Zenodo's exact public content endpoint produced transient 504/502/503 responses and a premature EOF during independent research. The complete hash-verified reference required official byte-range recovery and is not proof of a reliable single whole GET. Native fresh transfer, preview, raw-value and reopen acceptance remain separate; no alternative host or authentication bypass is used.".into(),
        "CC BY 4.0 is retained from the exact published author record. This is a reviewed direct-file client recipe; no publisher STAC collection, item, checksum field or arbitrary mirror is fabricated. Grayscale preview is a numerical quicklook, not a calibrated nighttime-light map or validation of sensing accuracy.".into(),
    ];
    dataset.limits = vec![
        "One complete fixed author-processed annual file per selection, with year 2024 and layer annual-dn only. No month, country, region, season, sensor, scene or quality selector is supported.".into(),
        format!("The independently reconstructed original has {NIGHTLIGHTS_2024_BYTES} bytes, publisher MD5 {NIGHTLIGHTS_2024_MD5}, and local SHA-256 {NIGHTLIGHTS_2024_SHA256}. The research checksum receipt is separate from any native transfer receipt; it is not a fabricated native checksum-verification field. Existing 512 MiB transfer, 131,072 tiled-axis / 65,536 stripped-axis, 8-billion grid-pixel, memory and preview deadline guards remain unchanged."),
    ];
    dataset
}

pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    features::bounds(request.bounds)?;
    if request.dataset_id != "openlandmap-nightlights"
        || request.year != Some(2024)
        || request.month.is_some()
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
        || request.layer.as_deref() != Some("annual-dn")
    {
        return Err("Choose only the fixed OpenLandMap / Hengl Zenodo v0.4 year 2024 annual-dn original, without month, country, region or season parameters; no older STAC, modeled year, radiance or quality layer is substituted".into());
    }
    let bounds = request.bounds;
    if bounds[0] >= QUERY_BOUNDS[2]
        || bounds[2] <= QUERY_BOUNDS[0]
        || bounds[1] >= QUERY_BOUNDS[3]
        || bounds[3] <= QUERY_BOUNDS[1]
    {
        return Err("The requested map area does not intersect the fixed 2024 nighttime-light file's actual extent; no other polar grid, year or region is substituted".into());
    }
    let mut original = source(
        "Hengl / OpenGeoHub annual nighttime-light DN · 2024 · v0.4".into(),
        NIGHTLIGHTS_2024_URL.into(),
        Some(QUERY_BOUNDS),
    );
    original.id = SOURCE_ID.into();
    // The selected reference year is carried by the reviewed product proof.
    // A direct TIFF does not publish a scene acquisition interval.
    Ok(vec![original])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ResolveRequest {
        ResolveRequest {
            dataset_id: "openlandmap-nightlights".into(),
            bounds: [116.3, 39.8, 116.5, 40.],
            year: Some(2024),
            month: None,
            country: None,
            layer: Some("annual-dn".into()),
            region: None,
            season: None,
        }
    }

    #[test]
    fn exact_direct_file_has_unique_id_annual_reference_and_no_fabricated_stac() {
        let sources = resolve(&request()).unwrap();
        assert_eq!(sources.len(), 1);
        let original = &sources[0];
        assert_eq!(original.id, SOURCE_ID);
        assert_ne!(original.id, "content");
        assert_eq!(original.stac.as_ref().unwrap().url, NIGHTLIGHTS_2024_URL);
        assert_eq!(original.stac.as_ref().unwrap().kind, "raster");
        assert_eq!(original.bounds, Some(QUERY_BOUNDS));
        assert!(original.start_datetime.is_none() && original.end_datetime.is_none());
        assert!(original.catalog_raster.is_none() && original.search.is_none());
        let dataset = definition();
        assert_eq!(dataset.years, vec![2024]);
        assert!(dataset.discovery.is_none());
        assert_eq!(dataset.layers[0].scale, Some(10.));
        assert_eq!(dataset.layers[0].nodata, Some(-32768.));
        assert!(dataset.value_interpretation.contains("calibration"));
        assert!(dataset
            .warnings
            .iter()
            .any(|warning| warning.contains("3,299")));
        assert!(dataset
            .warnings
            .iter()
            .any(|warning| warning.contains("0–2000")));
    }

    #[test]
    fn modeled_and_unreviewed_years_or_other_layers_are_never_substituted() {
        for year in [2000, 2011, 2012, 2021, 2023, 2025, 2026] {
            let mut selected = request();
            selected.year = Some(year);
            assert!(resolve(&selected).is_err());
        }
        let mut selected = request();
        selected.year = None;
        assert!(resolve(&selected).is_err());
        for layer in ["radiance", "proportion", "difference", "observation-count"] {
            let mut selected = request();
            selected.layer = Some(layer.into());
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
    }

    #[test]
    fn invalid_and_nonintersecting_areas_fail_without_claiming_ocean_validity() {
        for bounds in [
            [0., 88., 1., 89.],
            [0., -70., 1., -65.],
            [0., 87.37, 1., 88.],
            [-181., 30., -179., 31.],
        ] {
            let mut selected = request();
            selected.bounds = bounds;
            assert!(resolve(&selected).is_err());
        }
        let mut selected = request();
        selected.bounds = [-30., 0., -29.9, 0.1];
        assert!(resolve(&selected).is_ok());
        assert!(definition()
            .warnings
            .iter()
            .any(|warning| warning.contains("not equivalent to zero observations")));
    }
}
