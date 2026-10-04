//! Reviewed MOD13Q1/MYD13Q1 v061 science COGs. This is not the original HDF.
use chrono::{Datelike, NaiveDate};
pub const COLLECTION: &str = "modis-13Q1-061";
pub const PRODUCT: &str = "modis-13q1-v061";
pub const KEYS: &[&str] = &["ndvi", "evi"];
pub const SCIENCE_KEYS: &[&str] = &[
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
];
pub const ASSETS: &[(&str, &str)] = &[
    ("ndvi", "250m_16_days_NDVI"),
    ("evi", "250m_16_days_EVI"),
    ("vi_quality", "250m_16_days_VI_Quality"),
    ("vi_reliability", "250m_16_days_pixel_reliability"),
    ("vi_doy", "250m_16_days_composite_day_of_the_year"),
    ("vi_red", "250m_16_days_red_reflectance"),
    ("vi_nir", "250m_16_days_NIR_reflectance"),
    ("vi_blue", "250m_16_days_blue_reflectance"),
    ("vi_mir", "250m_16_days_MIR_reflectance"),
    ("vi_view_zenith", "250m_16_days_view_zenith_angle"),
    ("vi_sun_zenith", "250m_16_days_sun_zenith_angle"),
    ("vi_relative_azimuth", "250m_16_days_relative_azimuth_angle"),
];
pub fn is_key(key: &str) -> bool {
    KEYS.contains(&key) || SCIENCE_KEYS.contains(&key)
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Layer {
    pub key: &'static str,
    pub data_type: &'static str,
    pub bits: u8,
    pub signed: bool,
    pub scale: f64,
    pub nodata: i32,
    pub range: [i32; 2],
    pub kind: &'static str,
    pub unit: &'static str,
    pub catalog_unit: Option<&'static str>,
}
pub(crate) fn layer(key: &str) -> Option<Layer> {
    let (key, data_type, bits, signed, scale, nodata, range, kind, unit, catalog_unit) = match key {
        "ndvi" => (
            "ndvi",
            "int16",
            16,
            true,
            0.0001,
            -3000,
            [-2000, 10000],
            "index",
            "NDVI",
            Some("NDVI"),
        ),
        "evi" => (
            "evi",
            "int16",
            16,
            true,
            0.0001,
            -3000,
            [-2000, 10000],
            "index",
            "EVI",
            Some("EVI"),
        ),
        "vi_quality" => (
            "vi_quality",
            "uint16",
            16,
            false,
            1.0,
            65535,
            [0, 65534],
            "flags",
            "bit field",
            None,
        ),
        "vi_reliability" => (
            "vi_reliability",
            "int8",
            8,
            true,
            1.0,
            -1,
            [0, 3],
            "rank",
            "rank",
            Some("Rank"),
        ),
        "vi_doy" => (
            "vi_doy",
            "int16",
            16,
            true,
            1.0,
            -1,
            [1, 366],
            "date",
            "day of year",
            Some("JulianDay"),
        ),
        "vi_red" | "vi_nir" | "vi_blue" | "vi_mir" => (
            match key {
                "vi_red" => "vi_red",
                "vi_nir" => "vi_nir",
                "vi_blue" => "vi_blue",
                _ => "vi_mir",
            },
            "int16",
            16,
            true,
            0.0001,
            -1000,
            [0, 10000],
            "reflectance",
            "reflectance",
            None,
        ),
        "vi_view_zenith" | "vi_sun_zenith" => (
            if key == "vi_view_zenith" {
                "vi_view_zenith"
            } else {
                "vi_sun_zenith"
            },
            "int16",
            16,
            true,
            0.01,
            -10000,
            [0, 18000],
            "angle",
            "degrees",
            Some("Degree"),
        ),
        "vi_relative_azimuth" => (
            "vi_relative_azimuth",
            "int16",
            16,
            true,
            0.01,
            -4000,
            [-18000, 18000],
            "angle",
            "degrees",
            Some("Degree"),
        ),
        _ => return None,
    };
    Some(Layer {
        key,
        data_type,
        bits,
        signed,
        scale,
        nodata,
        range,
        kind,
        unit,
        catalog_unit,
    })
}
pub(crate) fn validate_band(band: &serde_json::Value, key: &str) -> bool {
    let Some(layer) = layer(key) else {
        return false;
    };
    band["data_type"].as_str() == Some(layer.data_type)
        && band["spatial_resolution"].as_f64() == Some(250.0)
        && band
            .get("scale")
            .map_or(layer.scale == 1.0, |v| v.as_f64() == Some(layer.scale))
        && band.get("offset").is_none_or(|v| v.as_f64() == Some(0.0))
        && band
            .get("nodata")
            .is_none_or(|v| v.as_i64() == Some(i64::from(layer.nodata)))
        && band.get("unit").map_or(layer.catalog_unit.is_none(), |v| {
            if key == "vi_doy" {
                matches!(v.as_str(), Some("JulianDay" | "Julian Day"))
            } else {
                v.as_str() == layer.catalog_unit
            }
        })
}
pub const PIXEL: f64 = super::modis::PIXEL / 2.0;
pub fn identity(id: &str) -> Option<(u32, u32)> {
    if !id.is_ascii() {
        return None;
    }
    let p: Vec<_> = id.split('.').collect();
    if p.len() != 5
        || !matches!(p[0], "MOD13Q1" | "MYD13Q1")
        || p[1].len() != 8
        || !p[1].starts_with('A')
        || p[2].len() != 6
        || !p[2].starts_with('h')
        || p[2].as_bytes()[3] != b'v'
        || p[3] != "061"
        || p[4].len() != 13
        || !p[1][1..]
            .bytes()
            .chain(p[2][1..3].bytes())
            .chain(p[2][4..].bytes())
            .chain(p[4].bytes())
            .all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let year: i32 = p[1][1..5].parse().ok()?;
    let doy: u32 = p[1][5..].parse().ok()?;
    let start = NaiveDate::from_yo_opt(year, doy)?;
    let production = NaiveDate::from_yo_opt(p[4][..4].parse().ok()?, p[4][4..7].parse().ok()?)?;
    let h: u32 = p[2][1..3].parse().ok()?;
    let v: u32 = p[2][4..].parse().ok()?;
    ((2000..=9998).contains(&start.year())
        && doy % 16 == if p[0] == "MOD13Q1" { 1 } else { 9 }
        && production >= start
        && h < 36
        && v < 18
        && p[4][7..9].parse::<u32>().ok()? < 24
        && p[4][9..11].parse::<u32>().ok()? < 60
        && p[4][11..].parse::<u32>().ok()? < 60)
        .then_some((h, v))
}
pub fn period(id: &str) -> Option<[String; 2]> {
    identity(id)?;
    let year = id[9..13].parse().ok()?;
    let start = NaiveDate::from_yo_opt(year, id[13..16].parse().ok()?)?;
    let end = (start + chrono::Duration::days(15)).min(NaiveDate::from_ymd_opt(year, 12, 31)?);
    Some([format!("{start}T00:00:00Z"), format!("{end}T23:59:59Z")])
}
pub fn suffix(key: &str) -> Option<&'static str> {
    match key {
        "ndvi" => Some("_250m_16_days_NDVI.tif"),
        "evi" => Some("_250m_16_days_EVI.tif"),
        "vi_quality" => Some("_250m_16_days_VI_Quality.tif"),
        "vi_reliability" => Some("_250m_16_days_pixel_reliability.tif"),
        "vi_doy" => Some("_250m_16_days_composite_day_of_the_year.tif"),
        "vi_red" => Some("_250m_16_days_red_reflectance.tif"),
        "vi_nir" => Some("_250m_16_days_NIR_reflectance.tif"),
        "vi_blue" => Some("_250m_16_days_blue_reflectance.tif"),
        "vi_mir" => Some("_250m_16_days_MIR_reflectance.tif"),
        "vi_view_zenith" => Some("_250m_16_days_view_zenith_angle.tif"),
        "vi_sun_zenith" => Some("_250m_16_days_sun_zenith_angle.tif"),
        "vi_relative_azimuth" => Some("_250m_16_days_relative_azimuth_angle.tif"),
        _ => None,
    }
}
pub fn matches(path: &str, id: &str, key: &str) -> bool {
    if identity(id).is_none() {
        return false;
    }
    let Some(suffix) = suffix(key) else {
        return false;
    };
    let p: Vec<_> = id.split('.').collect();
    path == format!(
        "/modis-061-cogs/{}/{}/{}/{}/{}{}",
        p[0],
        &p[2][1..3],
        &p[2][4..],
        &p[1][1..],
        id,
        suffix
    )
}
pub fn item_from_path<'a>(path: &'a str, key: &str) -> Option<&'a str> {
    let id = path.rsplit('/').next()?.strip_suffix(suffix(key)?)?;
    matches(path, id, key).then_some(id)
}
pub fn asset_path(path: &str) -> bool {
    ASSETS
        .iter()
        .any(|(key, _)| item_from_path(path, key).is_some())
}
pub(crate) fn validate_catalogue(value: &serde_json::Value, id: &str) -> crate::Result<()> {
    let invalid = "MODIS vegetation catalogue product, period, tile or index calibration differs";
    let (h, v) = identity(id).ok_or(invalid)?;
    let period = period(id).ok_or(invalid)?;
    let p = &value["properties"];
    let time = |s: Option<&str>| {
        s.and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.timestamp_millis())
    };
    let platform = if id.starts_with("MOD") {
        "terra"
    } else {
        "aqua"
    };
    if value["id"].as_str() != Some(id)
        || value["collection"].as_str() != Some(COLLECTION)
        || time(p["start_datetime"].as_str()) != time(Some(&period[0]))
        || time(p["end_datetime"].as_str()) != time(Some(&period[1]))
        || p["modis:horizontal-tile"].as_u64() != Some(h.into())
        || p["modis:vertical-tile"].as_u64() != Some(v.into())
        || !(p["platform"].is_null()
            || p["platform"]
                .as_str()
                .is_some_and(|s| s.is_empty() || s == platform))
    {
        return Err(invalid.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vegetation_platform_shift_period_version_and_channel_are_pinned() {
        let terra = "MOD13Q1.A2025177.h08v05.061.2025195142416";
        let aqua = "MYD13Q1.A2025169.h08v05.061.2025189102903";
        assert_eq!(
            period(terra).unwrap(),
            ["2025-06-26T00:00:00Z", "2025-07-11T23:59:59Z"]
        );
        assert_eq!(
            period(aqua).unwrap(),
            ["2025-06-18T00:00:00Z", "2025-07-03T23:59:59Z"]
        );
        assert_eq!(
            period("MOD13Q1.A2025353.h08v05.061.2026001142416").unwrap()[1],
            "2025-12-31T23:59:59Z"
        );
        assert_eq!(
            period("MYD13Q1.A2025361.h08v05.061.2026001142416").unwrap()[1],
            "2025-12-31T23:59:59Z"
        );
        for (id, key) in [(terra, "ndvi"), (aqua, "evi")] {
            let path = format!(
                "/modis-061-cogs/{}/08/05/{}/{id}{}",
                &id[..7],
                &id[9..16],
                suffix(key).unwrap()
            );
            let href = format!("https://{}{path}", super::super::modis::HOST);
            let url = crate::providers::asset_url(&href).unwrap();
            assert!(crate::providers::matches_item(&url, id, key));
            assert!(!crate::providers::matches_item(&url, id, "red"));
            assert!(!crate::providers::matches_item(
                &url,
                id,
                if key == "ndvi" { "evi" } else { "ndvi" }
            ));
            for bad in [
                format!("{href}?sig=private"),
                href.replace("/08/", "/09/"),
                href.replace(".061.", ".060."),
                href.replace("h08v05", "h36v05"),
            ] {
                assert!(crate::providers::asset_url(&bad).is_err());
            }
        }
        assert!(identity(&terra.replace("MOD13Q1", "MYD13Q1")).is_none());
        assert!(identity(&aqua.replace("MYD13Q1", "MOD13Q1")).is_none());
        assert!(identity("MOD13Q1.A2025366.h08v05.061.2026001142416").is_none());
    }
}
