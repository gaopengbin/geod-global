//! Controlled scene-catalog recipes. Declarations remain in the reviewed plan;
//! API item provenance is always the real publisher collection and query.
use super::*;
use chrono::NaiveDate;

const INPE_API: &str = "https://data.inpe.br/bdc/stac/v1/";
const IO_API: &str = "https://api.impactobservatory.com/stac-aws/";

fn catalog_limits() -> Vec<String> {
    vec!["The initial metadata page contains at most 20 items. A next cursor requires loading more pages with the same native query; item count and footprints do not establish valid-pixel or cloud-free coverage.".into(), "Each original file uses the runtime's 512 MiB transfer limit. Only tiled TIFFs support dimensions up to 131,072 pixels per side and at most 8 billion grid pixels; stripped TIFFs remain limited to 65,536 pixels per side. Decoding is bounded by 128 MiB per chunk, 32 MiB per intermediate buffer, 512 MiB preview memory and a 30-second review deadline. Some polar supercells exceed these bounds and fail explicitly; this connector does not guarantee that every global file is reviewable.".into()]
}

pub(super) fn cbers() -> Dataset {
    let mut dataset = base(
        "cbers-sr",
        "INPE CBERS surface reflectance scenes",
        "imagery",
        "https://data.inpe.br/bdc/stac-spatiotemporal-asset-catalog/",
    );
    dataset.description = "Search the official INPE CBERS-4/4A MUX and WFI Level-4 surface-reflectance scene collections by map area and UTC observation month; archive real STAC items and choose individual original numerical band files.".into();
    dataset.version = "INPE CB4/CB4A MUX/WFI L4-SR collections version 1".into();
    dataset.discovery = Some("stac-api".into());
    dataset.years = (2016..=2026).collect();
    dataset.default_year = Some(2026);
    dataset.months = (1..=12).collect();
    dataset.default_month = Some(9);
    dataset.nominal_resolution = "CBERS-4A MUX 16 m, WFI 55 m; CBERS-4 MUX 20 m, WFI 64 m".into();
    dataset.unit = "band-specific raw samples / surface reflectance".into();
    dataset.value_interpretation = "Year/month selects an observation search window, not a monthly composite. Reflectance bands are original Int16 samples; STAC declares reflectance = DN × 0.0001 and missing value -9999. CMASK is a separate categorical cloud mask with its own classes and NoData 255. Native inspection reads original values and embedded tags; no automatic reflectance calibration or cloud masking is inferred.".into();
    dataset.attribution = "National Institute for Space Research (INPE), Brazil Data Cube / BIG; China-Brazil Earth Resources Satellite (CBERS) program. Preserve each official STAC collection's CC BY 4.0 license and source credits, item identity, processing level, observation date and access date.".into();
    dataset.layers = vec![
        layer("cbers4a-mux", "CBERS-4A MUX L4 surface reflectance · 16 m", "band-specific", "Official CB4A-MUX-L4-SR-1 version 1; candidate observations in 2026 only. BAND5 blue, BAND6 green, BAND7 red, BAND8 near infrared; CMASK is the separate cloud mask. This is orthorectified surface reflectance, not WPM 2 m imagery or uncalibrated DN."),
        layer("cbers4-mux", "CBERS-4 MUX L4 surface reflectance · 20 m", "band-specific", "Official CB4-MUX-L4-SR-1 version 1; candidate observations in 2016–2026. BAND5 blue, BAND6 green, BAND7 red, BAND8 near infrared; CMASK is the separate cloud mask. Preserve original numerical samples and the publisher's calibration declaration."),
        layer("cbers4a-wfi", "CBERS-4A WFI L4 surface reflectance · 55 m", "band-specific", "Official CB4A-WFI-L4-SR-1 version 1; candidate observations in 2020–2026. BAND13 blue, BAND14 green, BAND15 red, BAND16 near infrared; CMASK is the separate cloud mask. This is an individual L4-SR scene, not the 64 m eight-day data cube."),
        layer("cbers4-wfi", "CBERS-4 WFI L4 surface reflectance · 64 m", "band-specific", "Official CB4-WFI-L4-SR-1 version 1; candidate observations in 2016–2026. BAND13 blue, BAND14 green, BAND15 red, BAND16 near infrared; CMASK is the separate cloud mask. This is an individual L4-SR scene, not a sixteen-day data cube."),
    ];
    dataset.default_layer = Some("cbers4a-mux".into());
    dataset.warnings = vec!["The archive has received and published scenes, principally over Brazil and parts of South America. Collection extent rectangles are not proof of continuous or global coverage; an empty search is a valid result and no different sensor, level, source or month is substituted.".into(), "The month is a UTC observation filter. The service may publish delayed observations and current/future month files can be absent; candidate year ranges are explicitly bounded to the reviewed archive through 2026. CBERS-4A MUX starts in 2026 and CBERS-4A WFI starts in 2020 for these specific SR collections.".into(), "Downloads preserve complete individual band files rather than clipping to the map area. RGB quicklooks, PCA-fused imagery, WPM PAN/DN products and eight/sixteen-day cubes are different products and are not substituted. Some original files may exceed native transfer or inspection limits.".into(), "Reflectance scale 0.0001 is declared in the official STAC band metadata; verified original TIFFs expose raw samples with TIFF scale 1. Preserve both declarations and use -9999/quality masks appropriately in scientific analysis. This connector does not add synthetic publisher properties to the archived item.".into()];
    dataset.limits = catalog_limits();
    dataset
}

pub(super) fn io_land_cover() -> Dataset {
    let mut dataset = base(
        "io-lulc",
        "Impact Observatory annual land cover",
        "land-cover",
        "https://registry.opendata.aws/io-lulc/",
    );
    dataset.description = "Search the official Impact Observatory STAC API for original annual 10 m land-use/land-cover classification supercells, produced with Microsoft and Esri from Sentinel-2 imagery. The complete categorical original is retained rather than an RGB tile or an AOI export.".into();
    dataset.version = "io-10m-annual-lulc annual archive 2017–2024; updated time series".into();
    dataset.discovery = Some("stac-api".into());
    dataset.years = (2017..=2024).collect();
    dataset.default_year = Some(2024);
    dataset.nominal_resolution = "10 m categorical classification grid".into();
    dataset.unit = "land-use/land-cover class code".into();
    dataset.value_interpretation = "One annual model-derived representative classification, with nine classes. It is not optical reflectance, a monthly composite, a daily acquisition or an observed change map. Preserve the original numerical class codes, embedded NoData and publisher legend; no automatic reclassification or scientific calibration is applied.".into();
    dataset.attribution = "Impact Observatory, Microsoft and Esri, 10m Annual Land Use Land Cover (9-class), accessed from the Registry of Open Data on AWS; contains ESA Sentinel-2 data. Cite the selected year, collection, item identity and access date.".into();
    dataset.layers = vec![layer("land-cover", "Annual nine-class land cover", "class code", "Model-derived annual land-use/land-cover labels from Sentinel-2 imagery; the fixed query selects the full calendar year. Original class codes and palette are retained. Zero is not assigned a synthetic missing-data meaning by this preset.")];
    dataset.default_layer = Some("land-cover".into());
    dataset.warnings = vec!["The official AWS registry declares CC BY 4.0 and the collection description also mentions CC BY 4.0, but the original STAC collection's license field says proprietary. This connector records the registry declaration and this conflict while preserving the publisher collection document unchanged; it does not silently rewrite the STAC license.".into(), "The supported annual archive is 2017–2024. Collection temporal extent ending at 2025-01-01 is the exclusive end of 2024, not a 2025 classification. A listed year and a completed search page do not guarantee valid-pixel coverage for an area.".into(), "The complete published supercell is downloaded, not clipped to the selected map area. A supercell can be much larger than the area and some polar files exceed native transfer or inspection limits. No PNG preview, different year or sensor product is substituted.".into(), "Annual classification differences can include model errors and algorithm changes. A raw numerical/palette preview is not validation of map accuracy or a statistically valid land-change estimate.".into()];
    dataset.limits = catalog_limits();
    dataset
}

pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    match request.dataset_id.as_str() {
        "cbers-sr" => resolve_cbers(request),
        "io-lulc" => {
            let year = request.year.ok_or("Choose an annual classification year")?;
            if !(2017..=2024).contains(&year)
                || request.month.is_some()
                || request.layer.as_deref() != Some("land-cover")
            {
                return Err(
                    "Choose an IO annual land-cover year from 2017–2024 without a month parameter"
                        .into(),
                );
            }
            Ok(vec![api_source(
                request,
                "io-10m-annual-lulc",
                format!("Impact Observatory annual land cover · {year}"),
                "Impact Observatory STAC catalog",
                IO_API,
                format!("{year}-01-01T00:00:00Z/{year}-12-31T23:59:59.999999Z"),
            )])
        }
        _ => Err("This product has no reviewed STAC API recipe".into()),
    }
}

fn api_source(
    request: &ResolveRequest,
    collection: &str,
    title: String,
    name: &str,
    api: &str,
    datetime: String,
) -> DatasetSource {
    DatasetSource {
        id: collection.into(),
        title,
        bounds: None,
        stac: Some(stac::ConnectRequest {
            name: name.into(),
            url: api.into(),
            kind: "api".into(),
        }),
        wcs: None,
        coverage_id: None,
        search: Some(DatasetSearch {
            collection_id: collection.into(),
            datetime,
            bounds: request.bounds,
            limit: 20,
        }),
        region: None,
        season: None,
        start_datetime: None,
        end_datetime: None,
        catalog_raster: None,
    }
}

fn resolve_cbers(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    let (collection, first_year) = match request.layer.as_deref() {
        Some("cbers4a-mux") => ("CB4A-MUX-L4-SR-1", 2026),
        Some("cbers4-mux") => ("CB4-MUX-L4-SR-1", 2016),
        Some("cbers4a-wfi") => ("CB4A-WFI-L4-SR-1", 2020),
        Some("cbers4-wfi") => ("CB4-WFI-L4-SR-1", 2016),
        _ => {
            return Err(
                "Choose one of the reviewed CBERS L4 surface-reflectance collections".into(),
            )
        }
    };
    let year = request.year.ok_or("Choose an observation year")?;
    let month = request.month.ok_or("Choose an observation month")?;
    if !(first_year..=2026).contains(&year) {
        return Err(format!("This CBERS surface-reflectance collection supports candidate observation years {first_year}–2026"));
    }
    let start = NaiveDate::from_ymd_opt(i32::from(year), u32::from(month), 1)
        .ok_or("Choose a valid observation month")?;
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let end = NaiveDate::from_ymd_opt(i32::from(next_year), u32::from(next_month), 1)
        .and_then(|value| value.pred_opt())
        .ok_or("Choose a valid observation month")?;
    Ok(vec![api_source(
        request,
        collection,
        format!("INPE {collection} · observations {year}-{month:02}"),
        "INPE STAC scene catalog",
        INPE_API,
        format!("{start}T00:00:00Z/{end}T23:59:59.999999Z"),
    )])
}
