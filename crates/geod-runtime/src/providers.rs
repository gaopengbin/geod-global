//! Stable source URLs are persisted; short-lived access signatures are not.
use crate::{Result, SOURCE_HOST};
use url::Url;
pub mod copernicus;
pub mod modis;
pub mod nasa;
mod planetary;
pub mod radar;
pub mod srtm;
pub mod vegetation;
pub mod viirs;
pub(crate) use planetary::AccessCache;

pub const PC_HOST: &str = "sentinel2l2a01.blob.core.windows.net";
pub const LANDSAT_HOST: &str = "landsateuwest.blob.core.windows.net";
pub const NAIP_HOST: &str = "naipeuwest.blob.core.windows.net";
pub const MAX_NAIP_BYTES: u64 = 4 * 1024 * 1024 * 1024;
pub(crate) const MAX_NAIP_EDGE: u32 = 40000;
pub const DEM_HOST: &str = "copernicus-dem-30m.s3.eu-central-1.amazonaws.com";
pub const DEM90_HOST: &str = "copernicus-dem-90m.s3.eu-central-1.amazonaws.com";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DemProduct {
    Glo30Public,
    Glo90,
}
impl DemProduct {
    pub(crate) fn host(self) -> &'static str {
        match self {
            Self::Glo30Public => DEM_HOST,
            Self::Glo90 => DEM90_HOST,
        }
    }
    pub(crate) fn product(self) -> &'static str {
        match self {
            Self::Glo30Public => "cop-dem-glo-30-public",
            Self::Glo90 => "cop-dem-glo-90",
        }
    }
    pub(crate) fn height(self) -> u32 {
        match self {
            Self::Glo30Public => 3600,
            Self::Glo90 => 1200,
        }
    }
    pub(crate) fn widths(self) -> &'static [u32] {
        match self {
            Self::Glo30Public => &[3600, 2400, 1800, 1200, 720, 360],
            Self::Glo90 => &[1200, 800, 600, 400, 240, 120],
        }
    }
}
pub(crate) fn dem_product(id: &str) -> Option<DemProduct> {
    dem_cell(id)?;
    match id.split('_').nth(3)? {
        "10" => Some(DemProduct::Glo30Public),
        "30" => Some(DemProduct::Glo90),
        _ => None,
    }
}
pub const SOURCE_ASSET_KEYS: &[&str] = &[
    "visual",
    "scl",
    "red",
    "green",
    "blue",
    "ndvi",
    "evi",
    "vi_quality",
    "vi_reliability",
    "vi_doy",
    "vi_red",
    "vi_nir",
    "vi_blue",
    "vi_mir",
    "vi_view_zenith",
    "vi_sun_zenith",
    "vi_relative_azimuth",
    "modis_qc",
    "modis_state",
    "qa_pixel",
    "qa_radsat",
    "product",
    "elevation",
    "aerial",
    "srtm",
    "viirs",
    "vv",
    "vh",
    "hh",
    "hv",
];

fn naip_date(value: &str) -> bool {
    value.len() == 8
        && value.bytes().all(|c| c.is_ascii_digit())
        && chrono::NaiveDate::parse_from_str(value, "%Y%m%d").is_ok()
}

fn naip_filename_parts(stem: &str) -> Option<Vec<&str>> {
    let s: Vec<_> = stem.split('_').collect();
    if !matches!(s.len(), 6 | 7)
        || s[0] != "m"
        || s[1].len() != 7
        || !s[1].bytes().all(|c| c.is_ascii_digit())
        || !matches!(s[2], "nw" | "ne" | "sw" | "se")
        || s[3].len() != 2
        || !s[3].bytes().all(|c| c.is_ascii_digit())
        || !(1..=23).contains(&s[3].parse::<u16>().ok()?)
        || !matches!(s[4], "030" | "060" | "100" | "1")
        || !s[5..].iter().all(|value| naip_date(value))
    {
        return None;
    }
    Some(s)
}

/// Reviewed NAIP v002 RGB+NIR COG path. Some filenames carry a second date;
/// older 1 m filenames use `1` inside a `100cm` directory. Keep both intact.
pub(crate) fn naip_path(path: &str) -> Option<String> {
    let p: Vec<_> = path.strip_prefix('/')?.split('/').collect();
    if p.len() != 7
        || p[0] != "naip"
        || p[1] != "v002"
        || p[2].len() != 2
        || !p[2].bytes().all(|c| c.is_ascii_lowercase())
    {
        return None;
    }
    let stem = p[6].strip_suffix(".tif")?;
    let s = naip_filename_parts(stem)?;
    let cm = if s[4] == "1" { "100" } else { s[4] };
    if p[3] != &s[5][..4] || p[5] != &s[1][..5] || p[4] != format!("{}_{}cm_{}", p[2], cm, p[3]) {
        return None;
    }
    Some(format!("{}_{stem}", p[2]))
}

pub(crate) fn naip_matches_item(path: &str, item_id: &str) -> bool {
    let Some(identity) = naip_path(path) else {
        return false;
    };
    identity == item_id
        // A catalogue ID may retain a second date omitted from its filename.
        // The subsequent official item lookup must still match the exact ID
        // and unsigned asset URL; this does not invent an alternate product.
        || (identity.split('_').count() == 7
            && item_id
                .strip_prefix(&format!("{identity}_"))
                .is_some_and(naip_date))
}

pub(crate) fn naip_pixel_size(item_id: &str) -> Option<f64> {
    let (state, stem) = item_id.split_once('_')?;
    if state.len() != 2 || !state.bytes().all(|c| c.is_ascii_lowercase()) {
        return None;
    }
    match naip_filename_parts(stem)?[4] {
        "030" => Some(0.3),
        "060" => Some(0.6),
        "100" | "1" => Some(1.0),
        _ => None,
    }
}

/// Public Copernicus geocell name: 10/30 means 1/3 arc seconds, not metres.
pub(crate) fn dem_cell(id: &str) -> Option<[i32; 2]> {
    let p: Vec<_> = id.split('_').collect();
    if p.len() != 9
        || p[..3] != ["Copernicus", "DSM", "COG"]
        || !matches!(p[3], "10" | "30")
        || p[5] != "00"
        || p[7] != "00"
        || p[8] != "DEM"
    {
        return None;
    }
    let signed = |s: &str, positive: char, negative: char, digits: usize| -> Option<i32> {
        if s.len() != digits + 1 || !s[1..].bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let value: i32 = s[1..].parse().ok()?;
        match s.chars().next()? {
            c if c == positive => Some(value),
            c if c == negative && value != 0 => Some(-value),
            _ => None,
        }
    };
    if !id.is_ascii() {
        return None;
    }
    let lat = signed(p[4], 'N', 'S', 2)?;
    let lon = signed(p[6], 'E', 'W', 3)?;
    ((-90..90).contains(&lat) && (-180..180).contains(&lon)).then_some([lon, lat])
}

pub fn asset_url(href: &str) -> Result<Url> {
    let url = Url::parse(href).map_err(|_| "Invalid asset URL")?;
    let source = match url.host_str() {
        Some(SOURCE_HOST) => url.path().starts_with("/sentinel-s2-l2a-cogs/"),
        Some(PC_HOST) => {
            url.path().starts_with("/sentinel2-l2/")
                && (url.path().ends_with("_TCI_10m.tif") || url.path().ends_with("_SCL_20m.tif"))
        }
        Some(LANDSAT_HOST) => {
            url.path()
                .starts_with("/landsat-c2/level-2/standard/oli-tirs/")
                && [
                    "_SR_B2.TIF",
                    "_SR_B3.TIF",
                    "_SR_B4.TIF",
                    "_QA_PIXEL.TIF",
                    "_QA_RADSAT.TIF",
                ]
                .iter()
                .any(|suffix| url.path().ends_with(suffix))
        }
        Some(modis::HOST) => modis::asset_path(url.path()) || vegetation::asset_path(url.path()),
        Some(radar::HOST) => radar::identity(url.path()).is_some(),
        Some(NAIP_HOST) => naip_path(url.path()).is_some(),
        Some(DEM_HOST | DEM90_HOST) => {
            let parts: Vec<_> = url.path().trim_start_matches('/').split('/').collect();
            parts.len() == 2
                && dem_product(parts[0])
                    .is_some_and(|product| Some(product.host()) == url.host_str())
                && parts[1] == format!("{}.tif", parts[0])
        }
        Some(nasa::HOST) => nasa::asset_path(url.path()),
        Some(copernicus::HOST) => copernicus::product_id(url.path()).is_some(),
        _ => false,
    };
    if !source
        || url.scheme() != "https"
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path().contains('%')
    {
        return Err("Asset URL must be an unsigned HTTPS raster URL on an approved source".into());
    }
    Ok(url)
}

pub fn matches_item(url: &Url, item_id: &str, asset_key: &str) -> bool {
    if vegetation::is_key(asset_key) {
        return url.host_str() == Some(modis::HOST)
            && vegetation::matches(url.path(), item_id, asset_key);
    }
    if modis::QUALITY_KEYS.contains(&asset_key) {
        return url.host_str() == Some(modis::HOST)
            && modis::matches(url.path(), item_id, asset_key);
    }
    if url.host_str() == Some(radar::HOST) {
        return radar::matches(url.path(), item_id, asset_key);
    }
    if asset_key == "viirs" {
        return url.host_str() == Some(nasa::HOST) && viirs::matches(url.path(), item_id);
    }
    if asset_key == "srtm" {
        return url.host_str() == Some(nasa::HOST) && nasa::matches_item(url, item_id, asset_key);
    }
    if url.host_str() == Some(modis::HOST) {
        return modis::matches(url.path(), item_id, asset_key);
    }
    if url.host_str() == Some(NAIP_HOST) {
        return asset_key == "aerial" && naip_matches_item(url.path(), item_id);
    }
    if matches!(url.host_str(), Some(DEM_HOST | DEM90_HOST)) {
        return dem_product(item_id).is_some_and(|product| Some(product.host()) == url.host_str())
            && asset_key == "elevation"
            && url.path() == format!("/{item_id}/{item_id}.tif");
    }
    if url.host_str() == Some(copernicus::HOST) {
        return copernicus::valid_item(item_id)
            && asset_key == "product"
            && copernicus::product_id(url.path()).is_some();
    }
    if url.host_str() == Some(nasa::HOST) {
        return nasa::matches_item(url, item_id, asset_key);
    }
    if url.host_str() == Some(LANDSAT_HOST) {
        let product = url.path_segments().and_then(|parts| parts.rev().nth(1));
        let suffix = match asset_key {
            "red" => "_SR_B4.TIF",
            "green" => "_SR_B3.TIF",
            "blue" => "_SR_B2.TIF",
            "qa_pixel" => "_QA_PIXEL.TIF",
            "qa_radsat" => "_QA_RADSAT.TIF",
            _ => return false,
        };
        return product.is_some_and(|product| {
            let parts: Vec<_> = product.split('_').collect();
            parts.len() == 7
                && matches!(parts[0], "LC08" | "LC09")
                && (!matches!(asset_key, "qa_pixel" | "qa_radsat") || {
                    let path: Vec<_> = url.path().split('/').collect();
                    matches!(parts[1], "L2SP" | "L2SR")
                        && parts[5] == "02"
                        && matches!(parts[6], "T1" | "T2" | "RT")
                        && parts[2].len() == 6
                        && parts[2].bytes().all(|b| b.is_ascii_digit())
                        && parts[3].len() == 8
                        && parts[3].bytes().all(|b| b.is_ascii_digit())
                        && parts[4].len() == 8
                        && parts[4].bytes().all(|b| b.is_ascii_digit())
                        && path.len() == 10
                        && path[5] == &parts[3][..4]
                        && path[6] == &parts[2][..3]
                        && path[7] == &parts[2][3..]
                })
                && [parts[0], parts[1], parts[2], parts[3], parts[5], parts[6]].join("_") == item_id
                && url.path().ends_with(&format!("/{product}{suffix}"))
        });
    }
    if url.host_str() != Some(PC_HOST) {
        return !matches!(
            asset_key,
            "red" | "green" | "blue" | "aerial" | "qa_pixel" | "qa_radsat"
        ) && url
            .path_segments()
            .is_some_and(|mut parts| parts.any(|part| part == item_id));
    }
    let product = url
        .path_segments()
        .and_then(|mut parts| parts.find(|part| part.ends_with(".SAFE")));
    let matched = product.is_some_and(|part| {
        let id = part
            .trim_end_matches(".SAFE")
            .split('_')
            .filter(|part| {
                !(part.len() == 5
                    && part.starts_with('N')
                    && part[1..].bytes().all(|c| c.is_ascii_digit()))
            })
            .collect::<Vec<_>>()
            .join("_");
        id == item_id
    });
    matched
        && match asset_key {
            "visual" => url.path().ends_with("_TCI_10m.tif"),
            "scl" => url.path().ends_with("_SCL_20m.tif"),
            _ => false,
        }
}

pub fn source_name(href: &str) -> &'static str {
    if Url::parse(href).is_ok_and(|url| url.host_str() == Some(radar::HOST)) {
        return "ESA Sentinel-1 IW; Catalyst radiometric terrain correction; Microsoft Planetary Computer RTC gamma0 COG distribution; CC-BY-4.0";
    }
    if Url::parse(href).is_ok_and(|url| url.host_str() == Some(modis::HOST)) {
        if Url::parse(href).is_ok_and(|url| vegetation::asset_path(url.path())) {
            return "NASA MODIS Terra/Aqua MOD13Q1/MYD13Q1 v061 16-day nominal 250 m NDVI/EVI; Microsoft Planetary Computer converted COG; not original NASA HDF; LP DAAC citation policies";
        }
        return "NASA MODIS Terra/Aqua MOD/MYD09A1 v061 8-day 500 m surface reflectance; Microsoft Planetary Computer converted COG, not original NASA HDF";
    }
    if Url::parse(href).is_ok_and(|url| url.host_str() == Some(NAIP_HOST)) {
        return "USDA NAIP aerial imagery; RGB + near-infrared original COG; Microsoft Planetary Computer distribution";
    }
    if Url::parse(href).is_ok_and(|url| url.host_str() == Some(DEM_HOST)) {
        return "Copernicus DEM GLO-30 Public COG; DLR / Airbus, European Union and ESA; AWS public distribution";
    }
    if Url::parse(href).is_ok_and(|url| url.host_str() == Some(DEM90_HOST)) {
        return "Copernicus DEM GLO-90 COG; DLR / Airbus, European Union and ESA; AWS public distribution";
    }
    if Url::parse(href).is_ok_and(|url| url.host_str() == Some(copernicus::HOST)) {
        return "Copernicus Data Space; ESA Sentinel-2 L2A SAFE original product";
    }
    if Url::parse(href).is_ok_and(|url| url.host_str() == Some(nasa::HOST)) {
        if Url::parse(href).is_ok_and(|url| viirs::asset_path(url.path())) {
            return "NASA/NOAA VIIRS VNP09A1 / VJ109A1 / VJ209A1 v002 8-day 1 km surface reflectance; original HDF5";
        }
        if Url::parse(href).is_ok_and(|url| srtm::asset_path(url.path())) {
            return "NASA LP DAAC; SRTMGL1 v003 original HGT; WGS84 / EGM96 elevation in metres";
        }
        return "NASA LP DAAC; Harmonized Landsat Sentinel-2 L30 v2.0 Surface Reflectance";
    }
    if Url::parse(href).is_ok_and(|url| url.host_str() == Some(LANDSAT_HOST)) {
        if href.ends_with("_QA_PIXEL.TIF") || href.ends_with("_QA_RADSAT.TIF") {
            return "Microsoft Planetary Computer; USGS Landsat 8/9 Collection 2 original QA_PIXEL / QA_RADSAT unsigned quality flags";
        }
        return "Microsoft Planetary Computer; USGS Landsat Collection 2 Level-2 Surface Reflectance";
    }
    if Url::parse(href).is_ok_and(|url| url.host_str() == Some(PC_HOST)) {
        "Microsoft Planetary Computer; Copernicus Sentinel-2 L2A"
    } else {
        "Earth Search / Element 84; Copernicus Sentinel-2 L2A"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn naip_live_filename_variants_preserve_catalogue_id_and_metre_spacing() {
        for (id, path, spacing) in [
            (
                "me_m_4506963_se_19_030_20231115_20240103",
                "/naip/v002/me/2023/me_030cm_2023/45069/m_4506963_se_19_030_20231115_20240103.tif",
                0.3,
            ),
            (
                "fl_m_2808060_se_17_1_20171211_20180201",
                "/naip/v002/fl/2017/fl_100cm_2017/28080/m_2808060_se_17_1_20171211.tif",
                1.0,
            ),
            (
                "ca_m_3712221_nw_10_060_20220518",
                "/naip/v002/ca/2022/ca_060cm_2022/37122/m_3712221_nw_10_060_20220518.tif",
                0.6,
            ),
        ] {
            let url = asset_url(&format!("https://{NAIP_HOST}{path}")).unwrap();
            assert!(matches_item(&url, id, "aerial"));
            assert_eq!(naip_pixel_size(id), Some(spacing));
            assert!(!matches_item(
                &url,
                &format!("{id}_20240201_20240301"),
                "aerial"
            ));
            assert!(!matches_item(&url, &id.replace("_m_", "_x_"), "aerial"));
            assert!(!matches_item(
                &url,
                &id.replace("_se_", "_sw_").replace("_nw_", "_ne_"),
                "aerial"
            ));
        }
        let path = "/naip/v002/fl/2017/fl_100cm_2017/28080/m_2808060_se_17_1_20171211.tif";
        for bad in [
            path.replace("100cm", "1cm"),
            path.replace("/28080/", "/28081/"),
            path.replace("_17_", "_+1_"),
            path.replace("_20171211.tif", "_20171211_20180230.tif"),
            path.replace("_20171211.tif", "_20171211_20180201_20190101.tif"),
        ] {
            assert!(asset_url(&format!("https://{NAIP_HOST}{bad}")).is_err());
        }
        for bad in [
            "fl_m_2808060_se_17_1_20171211_20180230",
            "fl_m_2808060_se_17_001_20171211_20180201",
            "fl_m_2808060_se_00_1_20171211_20180201",
        ] {
            assert_eq!(naip_pixel_size(bad), None);
            assert!(!naip_matches_item(path, bad));
        }
        assert!(!naip_matches_item(
            path,
            "fl_m_2808060_se_17_1_20171212_20180201"
        ));
        let two_dates =
            "/naip/v002/me/2023/me_030cm_2023/45069/m_4506963_se_19_030_20231115_20240103.tif";
        assert!(!naip_matches_item(
            two_dates,
            "me_m_4506963_se_19_030_20231115"
        ));
        assert!(!naip_matches_item(
            two_dates,
            "me_m_4506963_se_19_030_20231115_20240104"
        ));
    }
    #[test]
    fn naip_binds_state_date_resolution_and_nir_original_to_item() {
        let href="https://naipeuwest.blob.core.windows.net/naip/v002/ca/2022/ca_060cm_2022/37122/m_3712221_nw_10_060_20220518.tif";
        let id = "ca_m_3712221_nw_10_060_20220518";
        let url = asset_url(href).unwrap();
        assert!(matches_item(&url, id, "aerial"));
        assert!(!matches_item(&url, id, "visual"));
        assert!(!matches_item(&url, &id.replace("nw", "ne"), "aerial"));
        for bad in [
            format!("{href}?sig=PRIVATE"),
            href.replace("naipeuwest", "other"),
            href.replace("060cm", "100cm"),
            href.replace("/2022/", "/2021/"),
            href.replace("20220518.tif", "20220230.tif"),
        ] {
            assert!(asset_url(&bad).is_err());
        }
    }
    const ID: &str = "S2C_MSIL2A_20250627T184941_R113_T10SEG_20250627T234511";
    fn href() -> String {
        format!("https://{PC_HOST}/sentinel2-l2/10/S/EG/2025/06/27/S2C_MSIL2A_20250627T184941_N0511_R113_T10SEG_20250627T234511.SAFE/GRANULE/L2A_T10SEG_A004230_20250627T185915/IMG_DATA/R20m/T10SEG_20250627T184941_SCL_20m.tif")
    }
    #[test]
    fn source_paths_are_pinned_to_host_product_and_asset_type() {
        let good = href();
        let url = asset_url(&good).unwrap();
        assert!(matches_item(&url, ID, "scl"));
        assert!(!matches_item(&url, "other_item", "scl"));
        assert!(!matches_item(&url, ID, "visual"));
        for bad in [
            format!("{good}?sig=secret"),
            good.replace(PC_HOST, "evil.blob.core.windows.net"),
            good.replace("https:", "http:"),
            good.replace("_SCL_20m", "_B04_10m"),
            good.replace("/sentinel2-l2/", "/private/"),
        ] {
            assert!(asset_url(&bad).is_err());
        }
    }
    #[test]
    fn landsat_original_band_binding_preserves_product_processing_date_and_channel() {
        let id = "LC09_L2SP_044034_20250628_02_T1";
        let product = "LC09_L2SP_044034_20250628_20250629_02_T1";
        for (key, band) in [("red", 4), ("green", 3), ("blue", 2)] {
            let href = format!("https://{LANDSAT_HOST}/landsat-c2/level-2/standard/oli-tirs/2025/044/034/{product}/{product}_SR_B{band}.TIF");
            let url = asset_url(&href).unwrap();
            assert!(matches_item(&url, id, key));
            assert!(!matches_item(&url, id, "visual"));
            assert!(!matches_item(&url, "LC09_L2SP_044034_20250620_02_T1", key));
            assert!(!matches_item(
                &url,
                id,
                if key == "red" { "green" } else { "red" }
            ));
            for bad in [
                format!("{href}?sig=secret"),
                href.replace(LANDSAT_HOST, "private.blob.core.windows.net"),
                href.replace("_SR_B", "_ST_B"),
                href.replace("/oli-tirs/", "/tm/"),
            ] {
                assert!(asset_url(&bad).is_err());
            }
        }
    }
}
