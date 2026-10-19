//! Reviewed public numerical products and STAC scene catalogs. Presets resolve
//! fixed publisher filenames or fixed catalog queries; selections, transfers
//! and raw-pixel inspection use the generic STAC/direct-raster implementation.
//! A filename or a matching item is not proof of useful valid-pixel coverage.
use crate::{features, stac, wcs, JobManager, Result};
use serde::{Deserialize, Serialize};

mod catalogs;
mod chelsa;
mod ghsl;
mod glad_landcover;
pub(crate) mod maxar;
pub(crate) mod vantor_open;
// One exact author-published 2024 original; preserve unresolved calibration.
pub(crate) mod additional_pc;
pub(crate) mod amazonia_wfi;
pub(crate) mod cbers_wpm;
pub(crate) mod cgls;
pub(crate) mod clcd;
pub(crate) mod deafrica_annual;
pub(crate) mod earth_search_more;
pub(crate) mod enmap;
pub(crate) mod fabdem;
pub(crate) mod gebco_tid;
pub(crate) mod glc_fcs10;
pub(crate) mod glc_fcs30d;
mod hydrosheds;
pub(crate) mod jaxa;
pub(crate) mod jaxa_chla;
pub(crate) mod jaxa_extra;
pub(crate) mod jaxa_fnf;
pub(crate) mod jaxa_land_batch;
pub(crate) mod jaxa_more;
pub(crate) mod jaxa_ndvi;
pub(crate) mod jaxa_ocean_batch;
pub(crate) mod jaxa_rainfall;
mod land_use;
pub(crate) mod merit_ihu;
mod next_eth_canopy;
mod next_ghsl;
mod next_hydro_geoid;
pub(crate) mod next_planetary;
mod nightlights;
mod public_batch;
pub(crate) mod settlements_mangrove;
pub(crate) mod sinolc;
pub(crate) mod soilgrids;
#[cfg(test)]
mod tests;
pub(crate) mod usgs_3dep;
pub(crate) mod usgs_3dep_1m;
pub(crate) mod worldcereal;
pub(crate) mod worldclim;

const MAX_TILES: usize = 8;
const CC_BY: &str = "https://creativecommons.org/licenses/by/4.0/";
const FULL_TILE: &str = "The complete published tile is downloaded, not clipped to the requested area. Ocean tiles may be absent or contain only NoData.";

pub(crate) fn is_reviewed_jaxa(id: &str) -> bool {
    matches!(
        id,
        "jaxa-amsr2-soil-moisture"
            | "jaxa-sgli-lst"
            | "jaxa-modis-ndvi"
            | "jaxa-palsar-fnf"
            | "jaxa-sgli-chla"
            | "jaxa-gsmap-rainfall"
            | "jaxa-hrlulc-japan"
            | "jaxa-modis-swr"
            | "jaxa-sgli-sst"
            | "jaxa-sgli-et"
            | "jaxa-sgli-et-index"
            | "jaxa-sgli-aerosol"
            | "jaxa-modis-aerosol"
            | "jaxa-amsr2-wind"
            | "jaxa-amsr2-sst"
            | "jaxa-gsmap-spi"
    )
}

pub(crate) fn validate_jaxa_collection(
    proof: &SourceProvenance,
    url: &str,
    raw: &serde_json::Value,
) -> Result<()> {
    match proof.dataset.id.as_str() {
        "jaxa-modis-ndvi" => jaxa_ndvi::validate_collection(proof, url, raw),
        "jaxa-palsar-fnf" => jaxa_fnf::validate_collection(proof, url, raw),
        "jaxa-sgli-chla" => jaxa_chla::validate_collection(proof, url, raw),
        "jaxa-gsmap-rainfall" => jaxa_rainfall::validate_collection(proof, url, raw),
        "jaxa-hrlulc-japan" | "jaxa-modis-swr" => jaxa_extra::validate_collection(proof, url, raw),
        "jaxa-sgli-sst" => jaxa_more::validate_collection(proof, url, raw),
        "jaxa-sgli-et" | "jaxa-sgli-et-index" | "jaxa-sgli-aerosol" | "jaxa-modis-aerosol" => {
            jaxa_land_batch::validate_collection(proof, url, raw)
        }
        "jaxa-amsr2-wind" | "jaxa-amsr2-sst" | "jaxa-gsmap-spi" => {
            jaxa_ocean_batch::validate_collection(proof, url, raw)
        }
        _ => jaxa::validate_collection(proof, url, raw),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatasetOption {
    pub id: String,
    pub title: String,
    pub unit: String,
    pub value_interpretation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nodata: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatasetRegion {
    pub id: String,
    pub title: String,
    pub bounds: [f64; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatasetSeason {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Dataset {
    pub id: String,
    pub title: String,
    pub category: String,
    pub description: String,
    pub version: String,
    pub license: String,
    pub license_url: String,
    pub attribution: String,
    pub documentation_url: String,
    pub nominal_resolution: String,
    pub unit: String,
    pub value_interpretation: String,
    pub years: Vec<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_year: Option<u16>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub months: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_month: Option<u8>,
    pub requires_country: bool,
    pub layers: Vec<DatasetOption>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_layer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovery: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regions: Vec<DatasetRegion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_region: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seasons: Vec<DatasetSeason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_season: Option<String>,
    pub warnings: Vec<String>,
    pub limits: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolveRequest {
    pub dataset_id: String,
    pub bounds: [f64; 4],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub month: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub season: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatasetSource {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<[f64; 4]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stac: Option<stac::ConnectRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wcs: Option<wcs::ConnectRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<DatasetSearch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub season: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_datetime: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_datetime: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_raster: Option<CatalogRasterEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatasetSearch {
    pub collection_id: String,
    pub datetime: String,
    pub bounds: [f64; 4],
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetSearchResult {
    pub source_id: String,
    pub connection_id: String,
    pub page: stac::SearchPage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatasetPlan {
    pub dataset: Dataset,
    pub request: ResolveRequest,
    pub sources: Vec<DatasetSource>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceFailure {
    pub source_id: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetConnection {
    pub plan: DatasetPlan,
    pub stac_connections: Vec<stac::Connection>,
    pub wcs_connections: Vec<wcs::Connection>,
    pub failures: Vec<SourceFailure>,
    pub complete: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub searches: Vec<DatasetSearchResult>,
}

/// Local preset declarations are deliberately distinguished from publisher
/// metadata. The caller cannot supply this through ConnectRequest or a URL.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceProvenance {
    pub schema_version: u32,
    pub dataset: Dataset,
    pub request: ResolveRequest,
    pub layer: DatasetOption,
    pub source_id: String,
    pub source_title: String,
    pub source_bounds: Option<[f64; 4]>,
    pub declaration_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_raster: Option<CatalogRasterEvidence>,
}

/// A selected publisher catalog record and the exact archived response hash.
/// This is a direct-raster client receipt, never a fabricated publisher STAC item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogRasterEvidence {
    pub catalog_url: String,
    pub catalog_sha256: String,
    pub record: serde_json::Value,
}

fn layer(id: &str, title: &str, unit: &str, interpretation: &str) -> DatasetOption {
    DatasetOption {
        id: id.into(),
        title: title.into(),
        unit: unit.into(),
        value_interpretation: interpretation.into(),
        scale: None,
        offset: None,
        nodata: None,
        seasons: Vec::new(),
    }
}

fn scaled(mut value: DatasetOption, scale: f64, offset: f64, nodata: f64) -> DatasetOption {
    value.scale = Some(scale);
    value.offset = Some(offset);
    value.nodata = Some(nodata);
    value
}

fn base(id: &str, title: &str, category: &str, documentation: &str) -> Dataset {
    Dataset {
        id: id.into(), title: title.into(), category: category.into(), description: String::new(),
        version: String::new(), license: "CC BY 4.0".into(), license_url: CC_BY.into(),
        attribution: String::new(), documentation_url: documentation.into(),
        nominal_resolution: String::new(), unit: "layer-specific".into(),
        value_interpretation: "Original numerical samples; inspect the selected layer metadata. No automatic scientific conversion is applied.".into(),
        years: Vec::new(), default_year: None, months: Vec::new(), default_month: None, requires_country: false, layers: Vec::new(), default_layer: None, discovery: None,
        regions: Vec::new(), default_region: None, seasons: Vec::new(), default_season: None,
        warnings: Vec::new(),
        limits: vec![format!("Up to {MAX_TILES} tiles per selection; narrow larger areas."), "Each file uses the runtime's 512 MiB transfer limit. Native inspection supports up to 65,536 pixels per side and bounded decoding; some published files exceed those limits.".into()],
    }
}

pub fn list() -> Vec<Dataset> {
    let mut cover = base(
        "worldcover",
        "ESA WorldCover",
        "land-cover",
        "https://esa-worldcover.org/en/data-access",
    );
    cover.description =
        "Annual global land-cover classification published as original 3° GeoTIFF tiles.".into();
    cover.version = "2020 v100 / 2021 v200".into();
    cover.years = vec![2020, 2021];
    cover.default_year = Some(2021);
    cover.nominal_resolution = "10 m (1/12000 degree grid)".into();
    cover.unit = "land-cover class code".into();
    cover.attribution =
        "ESA WorldCover consortium; contains modified Copernicus Sentinel data (selected year)."
            .into();
    cover.layers = vec![scaled(layer("map", "Land-cover map", "class code", "11 classes: 10 tree cover, 20 shrubland, 30 grassland, 40 cropland, 50 built-up, 60 bare/sparse vegetation, 70 snow/ice, 80 permanent water, 90 herbaceous wetland, 95 mangroves, 100 moss/lichen; 0 NoData."), 1., 0., 0.)];
    cover.default_layer = Some("map".into());
    cover.warnings = vec![FULL_TILE.into(), "2020 and 2021 use different classification algorithms; differences are not necessarily real land-cover change. Classification labels are not optical reflectance.".into()];

    let mut composites = base(
        "worldcover-composites",
        "ESA WorldCover annual composites",
        "optical-radar",
        "https://esa-worldcover.org/en/data-access",
    );
    composites.description = "Published annual Sentinel-1/2 numerical composites, with 1° tile selection; these are processed annual products rather than single satellite scenes.".into();
    composites.version = "2020 v100 / 2021 v200".into();
    composites.years = vec![2020, 2021];
    composites.default_year = Some(2021);
    composites.nominal_resolution = "10 m; SWIR 20 m".into();
    composites.attribution =
        "ESA WorldCover consortium; contains modified Copernicus Sentinel data (selected year)."
            .into();
    composites.layers = vec![
        scaled(layer("rgbnir", "Sentinel-2 RGB + NIR", "reflectance", "Four scaled UInt16 bands: red, green, blue and near infrared. Physical value = DN × 0.0001; DN 0 is NoData."), 0.0001, 0., 0.),
        scaled(layer("swir", "Sentinel-2 SWIR", "reflectance", "Two scaled UInt8 short-wave infrared bands at 20 m. Physical value = DN × 0.004; DN 255 is NoData."), 0.004, 0., 255.),
        scaled(layer("ndvi", "Sentinel-2 NDVI composites", "NDVI", "Three annual median/percentile composite bands, scaled UInt8. Physical value = DN × 0.008 − 1; DN 25 is NoData. Preserve the publisher's band descriptions."), 0.008, -1., 25.),
        scaled(layer("gamma0", "Sentinel-1 gamma0 composites", "dB / publisher band-specific", "Three processed VV, VH and VV/VH ratio composite bands. Published table declares UInt16, scale 0.001 and offset −45; the ratio band's interpretation must be checked against the product manual. No radar conversion is inferred."), 0.001, -45., 0.),
    ];
    composites.default_layer = Some("swir".into());
    composites.warnings = vec![FULL_TILE.into(), "These annual composites are not raw Sentinel-1 GRD or individual Sentinel-2 acquisitions. Scale/offset declarations are retained, not automatically applied.".into()];

    let mut pop = base(
        "worldpop",
        "WorldPop Global 1 population",
        "population",
        "https://hub.worldpop.org/project/categories?id=3",
    );
    pop.description = "Historical Global 1 unconstrained estimated population counts, one original national GeoTIFF for the selected ISO3 country and year.".into();
    pop.version = "Global 1 (2000–2020 archive)".into();
    pop.years = (2000..=2020).collect();
    pop.default_year = Some(2020);
    pop.requires_country = true;
    pop.nominal_resolution = "approximately 100 m (3 arc-second grid)".into();
    pop.unit = "estimated persons per grid cell".into();
    pop.attribution = "WorldPop, University of Southampton; retain the selected country's release citation and DOI.".into();
    pop.layers = vec![layer("population", "Unconstrained population count", "persons/cell", "Modeled population count per pixel, not population density, census observation or Global 2 future projections.")];
    pop.default_layer = Some("population".into());
    pop.warnings = vec!["The entire selected country's file is downloaded; the map area does not clip or identify that country. Country/year availability and file size are checked when connecting/downloading.".into(), "This connector selects the historical Global 1 archive, not current Global 2 or country-specific constrained products. Preserve country-specific metadata/citations.".into()];

    let mut water = base(
        "jrc-water",
        "JRC Global Surface Water",
        "hydrology",
        "https://global-surface-water.appspot.com/download",
    );
    water.description = "EC/JRC v1.5 numerical water-history indicators, published in 10° GeoTIFF tiles through the CloudFerro distribution.".into();
    water.version = "GSW v1.5, release through 2024".into();
    water.nominal_resolution = "approximately 30 m (0.00025 degree grid)".into();
    water.license = "Copernicus free use with acknowledgement".into();
    water.license_url = "https://global-surface-water.appspot.com/download".into();
    water.attribution =
        "Source: EC JRC/Google; Pekel et al., Nature 540, 418–422 (2016), doi:10.1038/nature20584."
            .into();
    water.layers = vec![
        layer("occurrence", "Water occurrence", "percent", "Frequency of valid water observations over 1984–2024, not the current water surface. NoData and source masks must be respected."),
        layer("change", "Occurrence change intensity", "encoded change index", "Change indicator between observation epochs; the original encoded values require the publisher's legend and are not converted to signed percentages."),
        layer("seasonality", "Water seasonality", "months / encoded mask", "Number of months with water for the 2022–2024 release period, with publisher masks; not a full 1984–2024 annual series."),
        layer("recurrence", "Water recurrence", "percent", "Inter-annual recurrence indicator over 1984–2024; original masks are retained."),
        layer("transitions", "Water transitions", "class code", "Categorical change in water state over 1984–2024; classes are not continuous reflectance or water depth."),
        layer("extent", "Maximum water extent", "class / mask", "Maximum observed water extent for the distributed 2022–2024 release; not current inundation."),
    ];
    water.default_layer = Some("occurrence".into());
    water.warnings = vec![FULL_TILE.into(), "Combining Landsat Collections 1 and 2 introduces spatially varying registration offsets. Seasonality/extent and long-term occurrence have different temporal scopes; inspect the selected layer.".into(), "The publisher identifies outstanding monthly recurrence issues; this connector does not provide that monthly layer.".into()];

    let mut forest = base(
        "glad-forest",
        "GLAD Hansen Global Forest Change",
        "vegetation",
        "https://storage.googleapis.com/earthenginepartners-hansen/GFC-2025-v1.13/download.html",
    );
    forest.description = "Published UMD/GLAD numerical forest-cover and loss indicators in original 10° tiles; direct public storage access does not use Earth Engine computation.".into();
    forest.version = "GFC 2025 v1.13".into();
    forest.nominal_resolution = "approximately 30 m (0.00025 degree grid)".into();
    forest.attribution =
        "Source: Hansen/UMD/Google/USGS/NASA; Hansen et al. (2013), Science 342, 850–853.".into();
    forest.layers = vec![
        layer("treecover2000", "Tree canopy cover in 2000", "percent", "Tree canopy closure in 2000 for vegetation taller than 5 m, 0–100. Zero can be valid absence of tree cover; consult datamask."),
        layer("lossyear", "Forest loss year through 2025", "year code", "0 means no detected loss; codes 1–25 identify 2001–2025 stand-replacement disturbance. Forest loss is not necessarily deforestation; consult datamask."),
        layer("gain", "Forest gain 2000–2012", "binary class", "1 identifies forest gain over 2000–2012, 0 no mapped gain. This gain layer was not extended to 2025."),
        layer("datamask", "Land/water/data mask", "class code", "0 no data; 1 mapped land surface; 2 persistent water bodies, based on 2000–2012."),
    ];
    forest.default_layer = Some("lossyear".into());
    forest.warnings = vec![FULL_TILE.into(), "Algorithm and sensor changes affect inter-annual comparisons. Pixel counts alone do not establish statistically valid area estimates. Loss, baseline tree cover and gain refer to different periods.".into(), "Zero in tree cover, loss or gain is not automatically NoData. Download the separate datamask when analytical validity masks are needed.".into()];

    let mut relief = base(
        "etopo",
        "NOAA ETOPO 2022 relief",
        "elevation",
        "https://www.ncei.noaa.gov/products/etopo-global-relief-model",
    );
    relief.description = "NOAA's original 15 arc-second ice-surface elevation tiles, integrating topography and ocean bathymetry.".into();
    relief.version = "ETOPO 2022 v1".into();
    relief.nominal_resolution = "15 arc-seconds (about 0.5 km at the equator)".into();
    relief.unit = "metres relative to EGM2008 geoid".into();
    relief.license = "CC0 1.0".into();
    relief.license_url = "https://creativecommons.org/publicdomain/zero/1.0/".into();
    relief.attribution = "NOAA National Centers for Environmental Information (2022), ETOPO 2022 15 Arc-Second Global Relief Model, doi:10.25921/fd45-gt74.".into();
    relief.layers = vec![layer("surface", "Ice surface elevation", "m (EGM2008)", "Signed land elevation and ocean bathymetry, relative to EGM2008; this is ice surface elevation, not bedrock under ice or a 10 m terrain model.")];
    relief.default_layer = Some("surface".into());
    relief.warnings = vec![FULL_TILE.into(), "The 15° tile grid is a fused relief model, not a single satellite acquisition. No automatic vertical datum transformation is applied.".into()];
    let mut rain = base(
        "chirps",
        "CHIRPS v3 monthly precipitation",
        "climate",
        "https://chc.ucsb.edu/data/chirps3",
    );
    rain.description = "UCSB Climate Hazards Center final monthly precipitation estimates over land, combining satellite infrared information with station observations; one original quasi-global GeoTIFF per month.".into();
    rain.version = "CHIRPS v3.0 monthly final".into();
    rain.years = (1981..=2026).collect();
    rain.default_year = Some(2024);
    rain.months = (1..=12).collect();
    rain.default_month = Some(1);
    rain.nominal_resolution = "0.05 degree grid (about 5.6 km at the equator)".into();
    rain.unit = "mm/month".into();
    rain.attribution = "Climate Hazards Center, UC Santa Barbara; CHIRPS3 Data Repository (2025), doi:10.15780/G2JQ0P; Funk et al. (2026), Scientific Data 13, 718, doi:10.1038/s41597-026-07096-4. Include the access date.".into();
    rain.layers = vec![scaled(layer("monthly-precipitation", "Monthly precipitation total", "mm/month", "Float32 estimated accumulated precipitation for the selected calendar month, not a daily rainfall rate, forecast, anomaly or satellite scene. Values are already millimetres (scale 1, offset 0). The publisher declares -9999 as a bad/missing value; this product declaration is distinct from an embedded TIFF NoData tag."), 1., 0., -9999.)];
    rain.default_layer = Some("monthly-precipitation".into());
    rain.warnings = vec!["The whole quasi-global monthly file (180°W–180°E, 60°S–60°N) is downloaded; the selected map area does not crop it. Estimates are over land and ocean/missing pixels must be masked.".into(), "The publisher's FAQ declares -9999 as bad/missing data. The verified 2024-01 original has no embedded TIFF NoData tag; generic raw inspection does not automatically apply preset declarations as masks. Treat -9999 as missing before scientific analysis, and do not interpret a raw grayscale preview or unmasked statistics as a precipitation map.".into(), "This fixed recipe selects the final monthly product only. Final data is typically published in the third week of the following month; listed year/month options are candidates and do not guarantee that a file is released. Preliminary data is never substituted.".into(), "The official page simultaneously describes CHIRPS3 as public domain with a copyright waiver and links CC BY 4.0. This connector conservatively retains attribution and the CC BY 4.0 notice rather than interpreting the waiver as removal of attribution.".into()];
    rain.limits = vec!["One complete published monthly file per selection; latitude must remain within 60°S–60°N. The supported candidate archive is explicitly bounded to 1981–2026.".into(), "Each file uses the runtime's 512 MiB transfer limit. Native inspection supports up to 65,536 pixels per side and bounded decoding; some published files exceed those limits.".into()];
    let mut nasadem = base(
        "nasadem",
        "NASADEM public COG archive",
        "elevation",
        "https://portal.opentopography.org/datasetMetadata?otCollectionID=OT.032021.4326.2",
    );
    nasadem.description = "NASA's reprocessed SRTM-based elevation model, distributed as public 1° COG tiles by OpenTopography; this connector uses static files rather than the OpenTopography compute API.".into();
    nasadem.version = "NASADEM_HGT V001; OpenTopography April 2021 COG archive".into();
    nasadem.nominal_resolution =
        "1 arc-second (about 30 m at the equator), point-sampled grid".into();
    nasadem.unit = "metres relative to EGM96 geoid".into();
    nasadem.license = "CC0 1.0 (NASA LP DAAC data policy)".into();
    nasadem.license_url =
        "https://www.earthdata.nasa.gov/engage/open-data-services-software/data-use-policy".into();
    nasadem.attribution = "NASA JPL (2020), NASADEM Merged DEM Global 1 arc second V001, NASA EOSDIS LP DAAC, doi:10.5067/MEaSUREs/NASADEM/NASADEM_HGT.001; distributed by OpenTopography, doi:10.5069/G93T9FD9. Include the access date.".into();
    nasadem.layers = vec![scaled(layer("elevation", "NASADEM elevation", "m (EGM96)", "Signed Int16 elevation samples from NASADEM_HGT V001, with void filling and adjustments; not new satellite observations or an ellipsoidal elevation layer. NoData -32768; scale 1 and offset 0. The EGM96 vertical datum is a publisher declaration, not inferred from horizontal WGS84 GeoKeys."), 1., 0., -32768.)];
    nasadem.default_layer = Some("elevation".into());
    nasadem.warnings = vec![FULL_TILE.into(), "The 1° filename extent describes point sample centres. Verified files are PixelIsPoint with 3,601 × 3,601 samples; GDAL may expose outer bounds expanded by half a pixel. Preserve point registration and do not silently resample to an area grid.".into(), "This is the fixed public OpenTopography COG archive of V001, not a live NASA service or a guarantee of the newest archive. No OpenTopography API key is used. Vertical heights remain EGM96; no automatic datum transformation is applied.".into()];

    let mut srtm = base(
        "srtm-gl1",
        "SRTM GL1 public COG archive",
        "elevation",
        "https://portal.opentopography.org/datasetMetadata?otCollectionID=OT.042013.4326.1",
    );
    srtm.description = "NASA SRTM Global 1 arc-second void-filled elevation, openly distributed in 1° COG tiles by OpenTopography; independent of the app's authenticated NASA/USGS adapter.".into();
    srtm.version =
        "SRTM GL1 V3; OpenTopography static COG archive (COG revision not declared)".into();
    srtm.nominal_resolution = "1 arc-second (about 30 m at the equator), point-sampled grid".into();
    srtm.unit = "metres relative to EGM96 geoid".into();
    srtm.license = "CC0 1.0 (NASA LP DAAC data policy)".into();
    srtm.license_url =
        "https://www.earthdata.nasa.gov/engage/open-data-services-software/data-use-policy".into();
    srtm.attribution = "NASA Shuttle Radar Topography Mission (SRTM), NGA/NASA; SRTM GL1 V3, NASA EOSDIS LP DAAC. Distributed by OpenTopography, doi:10.5069/G9445JDF; Farr et al. (2007), doi:10.1029/2005RG000183. Include the access date.".into();
    srtm.layers = vec![scaled(layer("elevation", "SRTM GL1 elevation", "m (EGM96)", "Signed Int16 void-filled SRTM elevation based on the February 2000 mission, with auxiliary elevations; not current terrain, a bare-earth guarantee or SRTM_GL1_Ellip. NoData -32768; scale 1 and offset 0. EGM96 is retained as a publisher vertical-datum declaration."), 1., 0., -32768.)];
    srtm.default_layer = Some("elevation".into());
    srtm.warnings = vec![FULL_TILE.into(), "The 1° filename extent describes point sample centres. Verified files are PixelIsPoint with 3,601 × 3,601 samples; GDAL may expose outer bounds expanded by half a pixel. Preserve point registration and do not silently resample to an area grid.".into(), "This connector uses anonymous static OpenTopography files, not the personal-key Global DEM API or the existing authenticated NASA/USGS channel. COG archive revision is not declared. No automatic vertical datum transformation is applied.".into()];

    let mut alos = base(
        "alos-aw3d30",
        "ALOS AW3D30 public COG archive",
        "elevation",
        "https://portal.opentopography.org/datasetMetadata?otCollectionID=OT.112016.4326.2",
    );
    alos.description = "JAXA's ALOS PRISM-derived digital surface model, distributed by OpenTopography in 1° COG tiles; surface heights can include vegetation and buildings.".into();
    alos.version = "AW3D30 V3.2/3.1, January 2021; OpenTopography May 2021 COG archive".into();
    alos.nominal_resolution =
        "approximately 30 m (1 arc-second; longitude spacing varies at high latitudes)".into();
    alos.unit = "metres relative to EGM96 geoid".into();
    alos.license = "JAXA Terms of Use of Research Data".into();
    alos.license_url = "https://earth.jaxa.jp/en/data/policy/".into();
    alos.attribution = "JAXA ALOS World 3D 30 m (AW3D30); OpenTopography V3.2 January 2021 distribution, doi:10.5069/G94M92HB; Tadono et al. (2014) and Takaku et al. (2014). Preserve JAXA and any named supplementary data source credits.".into();
    alos.layers = vec![layer("surface", "AW3D30 digital surface height", "m (EGM96)", "Signed Int16 mean downsampled digital surface heights based on ALOS PRISM imagery from 2006–2011, with auxiliary void-filling sources. This is DSM surface height, not a bare-earth DTM, latest JAXA release, median downsampling or AW3D30_E ellipsoidal product.")];
    alos.default_layer = Some("surface".into());
    alos.warnings = vec![FULL_TILE.into(), "OpenTopography documents the January 2021 V3.2/3.1 average archive. It does not expose all original JAXA quality/mask/stack files and is not the newest JAXA product release. High-latitude longitude pixel spacing may differ; use embedded georeferencing.".into(), "The verified low-latitude COG is PixelIsArea, 3,600 × 3,600 Int16 samples, with no embedded TIFF NoData tag. Missing-data interpretation requires the product manual/quality masks; zero must not automatically be discarded.".into(), "JAXA requires source credits and credits for named third-party data. Commercial use is free of charge under its research-data terms but requires notifying JAXA in advance; retaining these terms is separate from connecting or testing a public file.".into(), "This is anonymous static bulk storage, not the OpenTopography API. EGM96 is retained as a publisher declaration; no automatic vertical datum transformation is applied.".into()];

    let mut hydro = base(
        "hydrosheds",
        "HydroSHEDS v2 flow directions",
        "hydrology",
        "https://www.hydrosheds.org/downloads-core-data-v2",
    );
    hydro.description = "Published HydroSHEDS v2r0 1 arc-second D8 drainage-direction COG tiles for the released Americas regions. The fixed native recipe contains only the 100 DIR tiles in the reviewed publisher index.".into();
    hydro.version = "HydroSHEDS v2r0 DIR 1s; published index 20260830 (100 tiles)".into();
    hydro.nominal_resolution = "1 arc-second (about 30 m at the equator)".into();
    hydro.unit = "D8 direction code".into();
    hydro.attribution = "HydroSHEDS v2; Lehner, Roth, Huber, Anand and Thieme (2022), A sharper look at the world's rivers and catchments, Eos 103, doi:10.1029/2022EO220167. Retain the HydroSHEDS data-license attribution.".into();
    hydro.layers = vec![scaled(layer("flow-direction", "D8 drainage direction", "D8 direction code", "UInt8 categorical drainage directions: 1 east, 2 southeast, 4 south, 8 southwest, 16 west, 32 northwest, 64 north, 128 northeast; 0 sink (no direction), 255 NoData. These are routing codes, not elevation, water depth, flow accumulation, river vectors or observed water motion."), 1., 0., 255.)];
    hydro.default_layer = Some("flow-direction".into());
    hydro.warnings = vec![FULL_TILE.into(), "Only the 100 DIR tiles in the publisher's 20260830 v2r0 index are supported; current published coverage is the Americas, not global. Any requested tile outside that finite list fails explicitly. This connector does not substitute v1, ACC/ACA, river vectors or a different release.".into(), "Some published DIR tiles exceed the current 512 MiB original transfer limit (the publisher lists files up to about 627 MB). Connecting or resolving a filename does not imply that the full original is downloadable within the runtime limit.".into(), "The original D8 codes are inspected as numerical samples. A raw grayscale thumbnail is not a scientific drainage visualization or a computed watershed.".into()];
    let mut result = vec![
        cover, composites, pop, water, forest, relief, rain, nasadem, srtm, alos, hydro,
    ];
    result.push(catalogs::cbers());
    result.push(cbers_wpm::definition());
    result.push(amazonia_wfi::definition());
    result.push(catalogs::io_land_cover());
    result.push(land_use::hilda());
    result.push(worldcereal::definition());
    result.push(cgls::definition());
    result.extend(deafrica_annual::definitions());
    result.extend(settlements_mangrove::definitions());
    result.push(usgs_3dep::definition());
    result.push(usgs_3dep_1m::definition());
    result.push(chelsa::definition());
    result.extend(jaxa::definitions());
    result.extend(jaxa_ndvi::definitions());
    result.extend(jaxa_fnf::definitions());
    result.extend(jaxa_chla::definitions());
    result.extend(jaxa_rainfall::definitions());
    result.extend(jaxa_extra::definitions());
    result.extend(jaxa_more::definitions());
    result.extend(jaxa_land_batch::definitions());
    result.extend(jaxa_ocean_batch::definitions());
    result.extend(public_batch::definitions());
    result.push(ghsl::definition());
    result.push(glad_landcover::definition());
    result.push(maxar::definition());
    result.extend(vantor_open::definitions());
    result.push(nightlights::definition());
    result.extend(next_planetary::definitions());
    result.extend(additional_pc::definitions());
    result.extend(clcd::definitions());
    result.extend(merit_ihu::definitions());
    result.extend(enmap::definitions());
    result.extend(gebco_tid::definitions());
    result.extend(next_ghsl::definitions());
    result.extend(next_hydro_geoid::definitions());
    result.extend(next_eth_canopy::definitions());
    result.extend(worldclim::definitions());
    result.extend(soilgrids::definitions());
    result.extend(fabdem::definitions());
    result.extend(glc_fcs10::definitions());
    result.extend(glc_fcs30d::definitions());
    result.extend(sinolc::definitions());
    result.extend(earth_search_more::definitions());
    result
}

fn coordinate(value: i32, positive: char, negative: char, digits: usize) -> String {
    format!(
        "{}{:0digits$}",
        if value < 0 { negative } else { positive },
        value.abs()
    )
}

/// Half-open area/grid intersection: exact eastern/northern edges must not add
/// a neighbouring tile. Use integer grid cells instead of floating epsilon.
fn tiles(bounds: [f64; 4], step: i32, min_lat: i32, max_lat: i32) -> Result<Vec<[i32; 4]>> {
    if bounds[1] < min_lat as f64 || bounds[3] > max_lat as f64 {
        return Err(format!("This distribution supports latitude {min_lat} to {max_lat}; choose an area entirely within that extent"));
    }
    let west = (bounds[0] / step as f64).floor() as i32 * step;
    let east = (bounds[2] / step as f64).ceil() as i32 * step;
    let south = (bounds[1] / step as f64).floor() as i32 * step;
    let north = (bounds[3] / step as f64).ceil() as i32 * step;
    let count = ((east - west) / step) as usize * ((north - south) / step) as usize;
    if count == 0 || count > MAX_TILES {
        return Err(format!("This area selects {count} published tiles; narrow the area to at most {MAX_TILES} tiles"));
    }
    let mut result = Vec::with_capacity(count);
    for y in (south..north).step_by(step as usize) {
        for x in (west..east).step_by(step as usize) {
            result.push([x, y, x + step, y + step]);
        }
    }
    Ok(result)
}

fn source(title: String, url: String, bounds: Option<[f64; 4]>) -> DatasetSource {
    let id = url
        .rsplit('/')
        .next()
        .unwrap_or("")
        .trim_end_matches(".tif")
        .to_string();
    DatasetSource {
        id,
        title: title.clone(),
        bounds,
        stac: Some(stac::ConnectRequest {
            name: title,
            url,
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
    }
}

pub fn resolve(mut request: ResolveRequest) -> Result<DatasetPlan> {
    features::bounds(request.bounds)?;
    let dataset = list()
        .into_iter()
        .find(|d| d.id == request.dataset_id)
        .ok_or("Choose an available public dataset preset")?;
    if !dataset.requires_country && request.country.is_some() {
        return Err("This dataset does not use a country parameter".into());
    }
    if dataset.regions.is_empty() && request.region.is_some() {
        return Err("This dataset does not use a region parameter".into());
    }
    if dataset.seasons.is_empty() && request.season.is_some() {
        return Err("This dataset does not use a season parameter".into());
    }
    request.region = request.region.or(dataset.default_region.clone());
    if request
        .region
        .as_ref()
        .is_some_and(|id| !dataset.regions.iter().any(|region| &region.id == id))
    {
        return Err("Choose a reviewed published region listed by this dataset preset".into());
    }
    let year = request.year.or(dataset.default_year);
    if year.is_some_and(|year| !dataset.years.contains(&year)) {
        return Err("Choose a published year listed by this dataset preset".into());
    }
    request.year = year;
    let month = request.month.or(dataset.default_month);
    if month.is_some_and(|month| !dataset.months.contains(&month)) {
        return Err("Choose a published month listed by this dataset preset".into());
    }
    request.month = month;
    let selected = request
        .layer
        .as_deref()
        .or(dataset.default_layer.as_deref())
        .and_then(|id| dataset.layers.iter().find(|layer| layer.id == id))
        .ok_or("Choose an available layer for this dataset")?;
    let layer = selected.id.clone();
    request.layer = Some(layer.clone());
    request.season = request
        .season
        .or(dataset.default_season.clone())
        .or_else(|| (selected.seasons.len() == 1).then(|| selected.seasons[0].clone()));
    if request.season.as_ref().is_some_and(|id| {
        !dataset.seasons.iter().any(|season| &season.id == id) || !selected.seasons.contains(id)
    }) {
        return Err("Choose a published season supported by the selected thematic layer".into());
    }
    let mut sources = Vec::new();
    match dataset.id.as_str() {
        "cbers-sr" | "io-lulc" => sources = catalogs::resolve(&request)?,
        "cbers-wpm" => sources = cbers_wpm::resolve(&request)?,
        "amazonia-wfi-dn" => sources = amazonia_wfi::resolve(&request)?,
        "hilda-land-use" => sources = land_use::resolve(&request)?,
        "chelsa-climate" => sources = chelsa::resolve(&request)?,
        "ghsl-smod" => sources = ghsl::resolve(&request)?,
        "ghsl-built-surface" | "ghsl-built-volume" | "ghsl-population" => {
            sources = next_ghsl::resolve(&request)?
        }
        "pc-hrea" | "pc-biodiversity" | "noaa-ohc" | "noaa-nclimgrid" => {
            sources = next_planetary::resolve(&request)?
        }
        "planetary-more" | "modis-more" => sources = additional_pc::resolve(&request)?,
        "clcd" => sources = clcd::resolve(&request)?,
        "merit-hydro" => sources = merit_ihu::resolve(&request)?,
        "enmap" => sources = enmap::resolve(&request)?,
        "gebco-tid" => sources = gebco_tid::resolve(&request)?,
        "hydrosheds-accumulation" | "etopo-geoid" => sources = next_hydro_geoid::resolve(&request)?,
        "eth-canopy-height" => sources = next_eth_canopy::resolve(&request)?,
        "worldclim" => sources = worldclim::resolve(&request)?,
        "soilgrids" => sources = soilgrids::resolve(&request)?,
        "fabdem" => sources = fabdem::resolve(&request)?,
        "glc-fcs10" => sources = glc_fcs10::resolve(&request)?,
        "glc-fcs30d" => sources = glc_fcs30d::resolve(&request)?,
        "sinolc-1" => sources = sinolc::resolve(&request)?,
        "earth-search-more" => sources = earth_search_more::resolve(&request)?,
        "glad-landcover-2020" => sources = glad_landcover::resolve(&request)?,
        "maxar-open-legacy" => sources = maxar::resolve(&request)?,
        "vantor-open" => sources = vantor_open::resolve(&request)?,
        "openlandmap-nightlights" => sources = nightlights::resolve(&request)?,
        "jaxa-amsr2-soil-moisture" | "jaxa-sgli-lst" => sources = jaxa::resolve(&request)?,
        "jaxa-modis-ndvi" => sources = jaxa_ndvi::resolve(&request)?,
        "jaxa-palsar-fnf" => sources = jaxa_fnf::resolve(&request)?,
        "jaxa-sgli-chla" => sources = jaxa_chla::resolve(&request)?,
        "jaxa-gsmap-rainfall" => sources = jaxa_rainfall::resolve(&request)?,
        "jaxa-hrlulc-japan" | "jaxa-modis-swr" => sources = jaxa_extra::resolve(&request)?,
        "jaxa-sgli-sst" => sources = jaxa_more::resolve(&request)?,
        "jaxa-sgli-et" | "jaxa-sgli-et-index" | "jaxa-sgli-aerosol" | "jaxa-modis-aerosol" => {
            sources = jaxa_land_batch::resolve(&request)?
        }
        "jaxa-amsr2-wind" | "jaxa-amsr2-sst" | "jaxa-gsmap-spi" => {
            sources = jaxa_ocean_batch::resolve(&request)?
        }
        "srtm-gl3" | "gedi-l3-canopy" | "srtm15plus-bathymetry" => {
            sources = public_batch::resolve(&request)?
        }
        "worldcereal" => sources = worldcereal::resolve(&request)?,
        "cgls-landcover" => sources = cgls::resolve(&request)?,
        "deafrica-fractional-cover" | "deafrica-wofs-annual" => {
            sources = deafrica_annual::resolve(&request)?
        }
        "wsf-settlements" | "gmw-mangroves" => sources = settlements_mangrove::resolve(&request)?,
        "usgs-3dep" => {
            let mut candidate = source(
                "USGS 3DEP · published catalog query".into(),
                usgs_3dep::query(&request)?,
                None,
            );
            candidate.id = "usgs-3dep-catalog".into();
            sources.push(candidate);
        }
        "usgs-3dep-1m" => {
            let mut candidate = source(
                "USGS 3DEP 1 m · published catalog query".into(),
                usgs_3dep_1m::query(&request)?,
                None,
            );
            candidate.id = "usgs-3dep-1m-catalog".into();
            sources.push(candidate);
        }
        "nasadem" | "srtm-gl1" | "alos-aw3d30" => {
            let (min_lat, max_lat) = match dataset.id.as_str() {
                "nasadem" => (-56, 61),
                "srtm-gl1" => (-56, 60),
                _ => (-84, 84),
            };
            for tile in tiles(request.bounds, 1, min_lat, max_lat)? {
                let lat = coordinate(
                    tile[1],
                    'N',
                    'S',
                    if dataset.id == "alos-aw3d30" { 3 } else { 2 },
                );
                let lon = coordinate(tile[0], 'E', 'W', 3);
                let id = format!("{lat}{lon}");
                let path = match dataset.id.as_str() {
                    "nasadem" => format!(
                        "NASADEM/NASADEM_be/NASADEM_HGT_{}.tif",
                        id.to_ascii_lowercase()
                    ),
                    "srtm-gl1" => format!("SRTM_GL1/SRTM_GL1_srtm/{id}.tif"),
                    _ => format!("AW3D30/AW3D30_global/ALPSMLC30_{id}_DSM.tif"),
                };
                sources.push(source(
                    format!("{} {id}", dataset.title),
                    format!("https://opentopography.s3.sdsc.edu/raster/{path}"),
                    Some(tile.map(|v| v as f64)),
                ));
            }
        }
        "hydrosheds" => {
            for tile in tiles(request.bounds, 10, -90, 90)? {
                let id = format!(
                    "{}{}",
                    coordinate(tile[1], 'N', 'S', 2),
                    coordinate(tile[0], 'E', 'W', 3)
                )
                .to_ascii_lowercase();
                if !hydrosheds::FLOW_DIRECTION_TILES.contains(&id.as_str()) {
                    return Err(format!("HydroSHEDS v2r0 DIR tile {id} is not in the reviewed 100-tile publisher index. Current released coverage is limited to the Americas; no alternate version or partial tile selection is substituted"));
                }
                sources.push(source(format!("HydroSHEDS v2r0 flow direction {id}"), format!("https://data.hydrosheds.org/file/hydrosheds-v2/DIR/1s/{id}_DIR_1s_v2r0.tif"), Some(tile.map(|v| v as f64))));
            }
        }
        "chirps" => {
            if request.bounds[1] < -60. || request.bounds[3] > 60. {
                return Err("CHIRPS v3 supports latitude -60 to 60; choose an area entirely within that extent".into());
            }
            let year = year.unwrap();
            let month = month.unwrap();
            sources.push(source(format!("CHIRPS v3 final monthly precipitation {year}-{month:02}"), format!("https://data.chc.ucsb.edu/products/CHIRPS/v3.0/monthly/global/tifs/chirps-v3.0.{year}.{month:02}.tif"), Some([-180., -60., 180., 60.])));
        }
        "worldpop" => {
            let country = request
                .country
                .as_deref()
                .ok_or("Enter an ISO 3166-1 alpha-3 country code")?
                .trim()
                .to_ascii_uppercase();
            if country.len() != 3 || !country.bytes().all(|b| b.is_ascii_uppercase()) {
                return Err(
                    "Enter a three-letter ISO country code, for example NIU, CHN or USA".into(),
                );
            }
            request.country = Some(country.clone());
            let year = year.unwrap();
            sources.push(source(format!("WorldPop {country} {year}"), format!("https://data.worldpop.org/GIS/Population/Global_2000_2020/{year}/{country}/{}_ppp_{year}.tif", country.to_ascii_lowercase()), None));
        }
        "worldcover" | "worldcover-composites" => {
            let year = year.unwrap();
            let version = if year == 2020 { "v100" } else { "v200" };
            let step = if dataset.id == "worldcover" { 3 } else { 1 };
            for tile in tiles(request.bounds, step, -90, 90)? {
                let lat = coordinate(tile[1], 'N', 'S', 2);
                let lon = coordinate(tile[0], 'E', 'W', 3);
                let id = format!("{lat}{lon}");
                let url = if dataset.id == "worldcover" {
                    format!("https://esa-worldcover.s3.eu-central-1.amazonaws.com/{version}/{year}/map/ESA_WorldCover_10m_{year}_{version}_{id}_Map.tif")
                } else {
                    let (bucket, path, suffix) = match layer.as_str() {
                        "rgbnir" => ("s2", "rgbnir", "S2RGBNIR"),
                        "swir" => ("s2", "swir", "SWIR"),
                        "ndvi" => ("s2", "ndvi", "NDVI"),
                        "gamma0" => ("s1", "vvvhratio", "S1VVVHratio"),
                        _ => unreachable!(),
                    };
                    format!("https://esa-worldcover-{bucket}.s3.eu-central-1.amazonaws.com/{path}/{year}/{lat}/ESA_WorldCover_10m_{year}_{version}_{id}_{suffix}.tif")
                };
                sources.push(source(
                    format!("WorldCover {year} {layer} {id}"),
                    url,
                    Some(tile.map(|v| v as f64)),
                ));
            }
        }
        "jrc-water" => {
            for tile in tiles(request.bounds, 10, -60, 80)? {
                // JRC uses longitude magnitude + hemisphere, latitude NORTH edge.
                let lon = format!("{}{}", tile[0].abs(), if tile[0] < 0 { 'W' } else { 'E' });
                let lat = format!("{}{}", tile[3].abs(), if tile[3] < 0 { 'S' } else { 'N' });
                sources.push(source(format!("GSW v1.5 {layer} {lon} {lat}"), format!("https://s3.waw4-1.cloudferro.com/swift/v1/global-surface-water/download2024/Aggregated/VER1-5/{layer}/{layer}_{lon}_{lat}_v1_5_2024.tif"), Some(tile.map(|v| v as f64))));
            }
        }
        "glad-forest" => {
            for tile in tiles(request.bounds, 10, -60, 80)? {
                let lat = format!(
                    "{:02}{}",
                    tile[3].abs(),
                    if tile[3] < 0 { 'S' } else { 'N' }
                );
                let lon = format!(
                    "{:03}{}",
                    tile[0].abs(),
                    if tile[0] < 0 { 'W' } else { 'E' }
                );
                sources.push(source(format!("GLAD GFC2025 {layer} {lat} {lon}"), format!("https://storage.googleapis.com/earthenginepartners-hansen/GFC-2025-v1.13/Hansen_GFC-2025-v1.13_{layer}_{lat}_{lon}.tif"), Some(tile.map(|v| v as f64))));
            }
        }
        "etopo" => {
            for tile in tiles(request.bounds, 15, -90, 90)? {
                let lat = coordinate(tile[3], 'N', 'S', 2);
                let lon = coordinate(tile[0], 'E', 'W', 3);
                sources.push(source(format!("ETOPO 2022 {lat}{lon} surface"), format!("https://www.ngdc.noaa.gov/mgg/global/relief/ETOPO2022/data/15s/15s_surface_elev_gtif/ETOPO_2022_v1_15s_{lat}{lon}_surface.tif"), Some(tile.map(|v| v as f64))));
            }
        }
        _ => return Err("This preset has no reviewed native resolver".into()),
    }
    let mut warnings = dataset.warnings.clone();
    warnings.extend(dataset.limits.clone());
    if dataset.discovery.as_deref() == Some("catalog-raster") {
        warnings.push("Resolving prepares a bounded publisher catalog query, not a file or coverage claim. Connecting reads and archives the complete response, checks actual record footprints, and probes selected original GeoTIFFs. Publication dates remain publisher record fields and are not observation timestamps. No download, account or paid computation is started.".into());
    } else if dataset.discovery.as_deref() == Some("stac-api") {
        warnings.push("The fixed UTC interval filters actual publisher STAC items. CBERS uses an observation month; IO uses the selected annual classification period. Connecting archives the real STAC response and at most 20 items on the first page; page completion and a next cursor do not establish valid-pixel coverage. No original-file job, crop, paid service or Earth Engine computation is started.".into());
    } else if dataset.id == "worldcereal" {
        warnings.push("The reviewed finite regional index selects actual standalone publisher STAC items whose declared bounding boxes intersect the map area. Actual AEZ season dates are retained; 2021 is a reference year rather than a calendar-month scene filter. A bounding-box match does not establish valid-crop coverage. Connecting archives metadata only, with no original-file job or paid computation.".into());
    } else if dataset.discovery.as_deref() == Some("stac-items") {
        warnings.push("This product selects actual standalone publisher items using a fixed release index or a versioned projected-grid filename recipe. Connecting confirms fresh metadata and preserves its actual reference interval; a candidate address or bounding-box match is not proof of file availability, valid-area coverage or scientific accuracy. No original-file job or paid computation is started.".into());
    } else {
        warnings.push("Resolved filenames are candidates. Connecting checks public TIFF headers; successful transfer, raw pixel decoding and useful coverage are separate checks. No account, paid service or Earth Engine compute API is used.".into());
    }
    Ok(DatasetPlan {
        dataset,
        request,
        sources,
        warnings,
    })
}

fn provenance(plan: &DatasetPlan, source: &DatasetSource) -> SourceProvenance {
    SourceProvenance {
        schema_version: 2,
        dataset: plan.dataset.clone(),
        request: plan.request.clone(),
        layer: plan
            .dataset
            .layers
            .iter()
            .find(|v| Some(&v.id) == plan.request.layer.as_ref())
            .unwrap()
            .clone(),
        source_id: source.id.clone(),
        source_title: source.title.clone(),
        source_bounds: source.bounds,
        declaration_kind: if source.catalog_raster.is_some() {
            "reviewed-client-catalog-raster; exact publisher catalog record archived separately"
        } else {
            "reviewed-client-preset; publisher file bytes and embedded metadata verified separately"
        }
        .into(),
        catalog_raster: source.catalog_raster.clone(),
    }
}

pub(crate) fn validate_provenance(proof: &SourceProvenance, href: &str) -> Result<()> {
    if proof.dataset.id == "usgs-3dep" {
        return usgs_3dep::validate(proof, href);
    }
    if proof.dataset.id == "usgs-3dep-1m" {
        return usgs_3dep_1m::validate(proof, href);
    }
    if proof.catalog_raster.is_some() {
        return Err("Publisher catalog evidence cannot be attached to another product".into());
    }
    if !matches!(proof.schema_version, 1 | 2) {
        return Err("Unsupported public dataset provenance version".into());
    }
    // Preset definitions are part of the immutable source contract. Changing a
    // release, URL recipe, unit declaration or warning in a future application
    // version requires retaining the old versioned recipe or an explicit,
    // verified receipt migration; never silently relabel saved originals.
    let plan = resolve(proof.request.clone())?;
    if plan.dataset.discovery.is_some() {
        return Err("STAC API discoveries use their actual publisher collection, query and item metadata; they cannot carry a synthetic direct-raster dataset proof".into());
    }
    let selected = plan
        .sources
        .iter()
        .find(|s| s.id == proof.source_id && s.stac.as_ref().is_some_and(|r| r.url == href))
        .ok_or("Public dataset provenance does not match the reviewed published file")?;
    let mut expected = provenance(&plan, selected);
    if proof.schema_version == 1 {
        // v1 persisted the publisher's rounded resolution wording. Retain that
        // historical declaration exactly when reading the old immutable proof;
        // new v2 proofs use the actual 0.00025 degree grid. No file, snapshot or
        // stored receipt is rewritten, and all other fields remain exact.
        expected.schema_version = 1;
        match plan.dataset.id.as_str() {
            "jrc-water" => expected.dataset.nominal_resolution = "30 m (1 arc-second grid)".into(),
            "glad-forest" => {
                expected.dataset.nominal_resolution =
                    "approximately 30 m (1 arc-second grid)".into()
            }
            _ => (),
        }
    }
    if expected != *proof {
        return Err("Public dataset provenance differs from its reviewed preset".into());
    }
    Ok(())
}

/// Product-specific reviewed item adapters never authorize arbitrary S3 URLs.
/// Old WorldCereal receipts retain their original exact validation contract.
pub(crate) fn validate_reviewed_item(
    proof: &SourceProvenance,
    url: &str,
    raw: Option<&serde_json::Value>,
) -> Result<()> {
    match proof.dataset.id.as_str() {
        "pc-hrea" | "pc-biodiversity" | "noaa-ohc" | "noaa-nclimgrid" => {
            next_planetary::validate(proof, url, raw)
        }
        "planetary-more" | "modis-more" => additional_pc::validate(proof, url, raw),
        "worldcereal" => worldcereal::validate(proof, url, raw),
        "cgls-landcover" => cgls::validate(proof, url, raw),
        "deafrica-fractional-cover" | "deafrica-wofs-annual" => {
            deafrica_annual::validate(proof, url, raw)
        }
        "wsf-settlements" | "gmw-mangroves" => settlements_mangrove::validate(proof, url, raw),
        "jaxa-amsr2-soil-moisture" | "jaxa-sgli-lst" => jaxa::validate(proof, url, raw),
        "jaxa-modis-ndvi" => jaxa_ndvi::validate(proof, url, raw),
        "jaxa-palsar-fnf" => jaxa_fnf::validate(proof, url, raw),
        "jaxa-sgli-chla" => jaxa_chla::validate(proof, url, raw),
        "jaxa-gsmap-rainfall" => jaxa_rainfall::validate(proof, url, raw),
        "jaxa-hrlulc-japan" | "jaxa-modis-swr" => jaxa_extra::validate(proof, url, raw),
        "jaxa-sgli-sst" => jaxa_more::validate(proof, url, raw),
        "jaxa-sgli-et" | "jaxa-sgli-et-index" | "jaxa-sgli-aerosol" | "jaxa-modis-aerosol" => {
            jaxa_land_batch::validate(proof, url, raw)
        }
        "jaxa-amsr2-wind" | "jaxa-amsr2-sst" | "jaxa-gsmap-spi" => {
            jaxa_ocean_batch::validate(proof, url, raw)
        }
        "maxar-open-legacy" => maxar::validate(proof, url, raw),
        "vantor-open" => vantor_open::validate(proof, url, raw),
        _ => Err("Unsupported reviewed publisher-item product".into()),
    }
}

pub(crate) fn reviewed_item_transports(
    proof: &SourceProvenance,
    url: &str,
    raw: &serde_json::Value,
) -> Result<Vec<stac::AssetTransport>> {
    match proof.dataset.id.as_str() {
        "pc-hrea" | "pc-biodiversity" | "noaa-ohc" | "noaa-nclimgrid" => {
            next_planetary::transports(proof, url, raw)
        }
        "planetary-more" | "modis-more" => additional_pc::transports(proof, url, raw),
        "worldcereal" => worldcereal::transports(proof, url, raw),
        "cgls-landcover" => cgls::transports(proof, url, raw),
        "deafrica-fractional-cover" | "deafrica-wofs-annual" => {
            deafrica_annual::transports(proof, url, raw)
        }
        "wsf-settlements" | "gmw-mangroves" => settlements_mangrove::transports(proof, url, raw),
        "jaxa-amsr2-soil-moisture" | "jaxa-sgli-lst" => jaxa::transports(proof, url, raw),
        "jaxa-modis-ndvi" => jaxa_ndvi::transports(proof, url, raw),
        "jaxa-palsar-fnf" => jaxa_fnf::transports(proof, url, raw),
        "jaxa-sgli-chla" => jaxa_chla::transports(proof, url, raw),
        "jaxa-gsmap-rainfall" => jaxa_rainfall::transports(proof, url, raw),
        "jaxa-hrlulc-japan" | "jaxa-modis-swr" => jaxa_extra::transports(proof, url, raw),
        "jaxa-sgli-sst" => jaxa_more::transports(proof, url, raw),
        "jaxa-sgli-et" | "jaxa-sgli-et-index" | "jaxa-sgli-aerosol" | "jaxa-modis-aerosol" => {
            jaxa_land_batch::transports(proof, url, raw)
        }
        "jaxa-amsr2-wind" | "jaxa-amsr2-sst" | "jaxa-gsmap-spi" => {
            jaxa_ocean_batch::transports(proof, url, raw)
        }
        "maxar-open-legacy" => maxar::transports(proof, url, raw),
        "vantor-open" => vantor_open::transports(proof, url, raw),
        _ => Err("Unsupported reviewed publisher-item transport".into()),
    }
}

pub(crate) fn reviewed_item_self_url(
    proof: &SourceProvenance,
    url: &str,
    raw: &serde_json::Value,
) -> Result<String> {
    match proof.dataset.id.as_str() {
        "pc-hrea" | "pc-biodiversity" | "noaa-ohc" | "noaa-nclimgrid" => {
            next_planetary::self_url(proof, url, raw)
        }
        "planetary-more" | "modis-more" => additional_pc::self_url(proof, url, raw),
        "worldcereal" => worldcereal::self_url(proof, url, raw),
        "cgls-landcover" => cgls::self_url(proof, url, raw),
        "deafrica-fractional-cover" | "deafrica-wofs-annual" => {
            deafrica_annual::self_url(proof, url, raw)
        }
        "wsf-settlements" | "gmw-mangroves" => settlements_mangrove::self_url(proof, url, raw),
        "jaxa-amsr2-soil-moisture" | "jaxa-sgli-lst" => jaxa::self_url(proof, url, raw),
        "jaxa-modis-ndvi" => jaxa_ndvi::self_url(proof, url, raw),
        "jaxa-palsar-fnf" => jaxa_fnf::self_url(proof, url, raw),
        "jaxa-sgli-chla" => jaxa_chla::self_url(proof, url, raw),
        "jaxa-gsmap-rainfall" => jaxa_rainfall::self_url(proof, url, raw),
        "jaxa-hrlulc-japan" | "jaxa-modis-swr" => jaxa_extra::self_url(proof, url, raw),
        "jaxa-sgli-sst" => jaxa_more::self_url(proof, url, raw),
        "jaxa-sgli-et" | "jaxa-sgli-et-index" | "jaxa-sgli-aerosol" | "jaxa-modis-aerosol" => {
            jaxa_land_batch::self_url(proof, url, raw)
        }
        "jaxa-amsr2-wind" | "jaxa-amsr2-sst" | "jaxa-gsmap-spi" => {
            jaxa_ocean_batch::self_url(proof, url, raw)
        }
        "maxar-open-legacy" => maxar::self_url(proof, url, raw),
        "vantor-open" => vantor_open::self_url(proof, url, raw),
        _ => Err("Unsupported reviewed publisher-item revalidation".into()),
    }
}

impl JobManager {
    pub async fn connect_dataset(&self, request: ResolveRequest) -> Result<DatasetConnection> {
        self.inner.store.lock().await.accepting_jobs()?;
        let mut plan = resolve(request)?;
        if matches!(plan.dataset.id.as_str(), "usgs-3dep" | "usgs-3dep-1m") {
            let catalog_url = if plan.dataset.id == "usgs-3dep" {
                usgs_3dep::query(&plan.request)?
            } else {
                usgs_3dep_1m::query(&plan.request)?
            };
            let (raw, receipt) = self.archive_dataset_catalog(&catalog_url).await?;
            let records = if plan.dataset.id == "usgs-3dep" {
                usgs_3dep::parse(&plan.request, &catalog_url, &raw)?
            } else {
                usgs_3dep_1m::parse(&plan.request, &catalog_url, &raw)?
            };
            plan.sources = records
                .into_iter()
                .map(|(mut source, record)| {
                    source.catalog_raster = Some(CatalogRasterEvidence {
                        catalog_url: catalog_url.clone(),
                        catalog_sha256: receipt.sha256.clone(),
                        record,
                    });
                    source
                })
                .collect();
        }
        let mut stac_connections = Vec::new();
        let wcs_connections = Vec::new();
        let mut failures = Vec::new();
        let mut searches = Vec::new();
        for source in &plan.sources {
            let request = source
                .stac
                .clone()
                .ok_or("This preset needs an unsupported connector")?;
            let result = if let Some(search) = &source.search {
                match self.connect_stac(request).await {
                    Ok(connection) => {
                        let query = stac::SearchRequest {
                            connection_id: connection.id.clone(),
                            collection_id: search.collection_id.clone(),
                            bounds: search.bounds,
                            datetime: Some(search.datetime.clone()),
                            limit: Some(search.limit),
                            cursor: None,
                        };
                        match self.search_stac(query).await {
                            Ok(page) => {
                                // Response selections belong to this query only.
                                // The persisted connection and all old snapshots
                                // remain intact; never return its accumulated IDs
                                // as results of the current area/month.
                                let connection = query_connection(&connection, &page);
                                searches.push(DatasetSearchResult {
                                    source_id: source.id.clone(),
                                    connection_id: connection.id.clone(),
                                    page,
                                });
                                Ok(connection)
                            }
                            Err(error) => Err(error),
                        }
                    }
                    Err(error) => Err(error),
                }
            } else if plan.dataset.discovery.as_deref() == Some("stac-items") {
                self.connect_dataset_item(request, provenance(&plan, source))
                    .await
            } else {
                self.connect_dataset_raster(request, provenance(&plan, source))
                    .await
            };
            match result {
                Ok(connection) => stac_connections.push(connection),
                Err(error) => failures.push(SourceFailure {
                    source_id: source.id.clone(),
                    error,
                }),
            }
        }
        let complete = failures.is_empty();
        Ok(DatasetConnection {
            plan,
            stac_connections,
            wcs_connections,
            failures,
            complete,
            searches,
        })
    }
}

fn query_connection(connection: &stac::Connection, page: &stac::SearchPage) -> stac::Connection {
    let mut current = connection.clone();
    current.snapshot_ids = page.items.iter().map(|item| item.id.clone()).collect();
    current
}
