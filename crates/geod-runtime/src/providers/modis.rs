//! Fixed MOD/MYD09A1 Collection 6.1 PC-converted reflectance COG identities.
use chrono::{Datelike, NaiveDate};
pub const HOST: &str = "modiseuwest.blob.core.windows.net";
pub const CRS: &str = "MODIS:Sinusoidal";
pub const PIXEL: f64 = 463.312716527778;
pub const RADIUS: f64 = 6371007.181;

/// MODLAND's fixed spherical sinusoidal projection. This intentionally has no
/// ellipsoidal datum shift, matching the product's specified sphere and lon_0.
pub(crate) fn forward([lon, lat]: [f64; 2]) -> crate::Result<[f64; 2]> {
    if !lon.is_finite()
        || !lat.is_finite()
        || !(-180.0..=180.0).contains(&lon)
        || !(-90.0..=90.0).contains(&lat)
    {
        return Err("MODIS project coordinates must be valid WGS84 longitude and latitude".into());
    }
    let phi = lat.to_radians();
    Ok([
        if lat.abs() == 90.0 {
            0.0
        } else {
            RADIUS * lon.to_radians() * phi.cos()
        },
        RADIUS * phi,
    ])
}

/// Rectangular MODIS tiles contain cells outside the projected globe. Those
/// have no WGS84 pixel centre and must remain outside a polygon mask.
pub(crate) fn inverse([x, y]: [f64; 2]) -> Option<[f64; 2]> {
    use std::f64::consts::{FRAC_PI_2, PI};
    let phi = y / RADIUS;
    if !x.is_finite() || !phi.is_finite() || phi.abs() > FRAC_PI_2 + 1e-12 {
        return None;
    }
    let phi = phi.clamp(-FRAC_PI_2, FRAC_PI_2);
    let cos = phi.cos();
    let lon = if cos.abs() < 1e-12 {
        if x.abs() > 1e-7 {
            return None;
        }
        0.0
    } else {
        x / (RADIUS * cos)
    };
    if lon.abs() > PI + 1e-12 {
        return None;
    }
    Some([lon.clamp(-PI, PI).to_degrees(), phi.to_degrees()])
}

pub(crate) const QUALITY_KEYS: &[&str] = &["modis_qc", "modis_state"];
pub(crate) fn suffix(key: &str) -> Option<&'static str> {
    match key {
        "red" => Some("_sur_refl_b01.tif"),
        "green" => Some("_sur_refl_b04.tif"),
        "blue" => Some("_sur_refl_b03.tif"),
        "modis_qc" => Some("_sur_refl_qc_500m.tif"),
        "modis_state" => Some("_sur_refl_state_500m.tif"),
        _ => None,
    }
}
pub(crate) fn item_from_path<'a>(path: &'a str, key: &str) -> Option<&'a str> {
    let id = path.rsplit('/').next()?.strip_suffix(suffix(key)?)?;
    matches(path, id, key).then_some(id)
}

pub(crate) fn period(id: &str) -> Option<[String; 2]> {
    identity(id)?;
    let p: Vec<_> = id.split('.').collect();
    let year = p[1][1..5].parse().ok()?;
    let start = NaiveDate::from_yo_opt(year, p[1][5..].parse().ok()?)?;
    let end = (start + chrono::Duration::days(7)).min(NaiveDate::from_ymd_opt(year, 12, 31)?);
    Some([format!("{start}T00:00:00Z"), format!("{end}T23:59:59Z")])
}
pub fn identity(id: &str) -> Option<(u32, u32)> {
    if !id.is_ascii() {
        return None;
    }
    let p: Vec<_> = id.split('.').collect();
    if p.len() != 5
        || !matches!(p[0], "MOD09A1" | "MYD09A1")
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
    let date = NaiveDate::from_yo_opt(year, doy)?;
    let production = NaiveDate::from_yo_opt(p[4][..4].parse().ok()?, p[4][4..7].parse().ok()?)?;
    let h: u32 = p[2][1..3].parse().ok()?;
    let v: u32 = p[2][4..].parse().ok()?;
    ((2000..=9998).contains(&date.year())
        && doy % 8 == 1
        && production >= date
        && h < 36
        && v < 18
        && p[4][7..9].parse::<u32>().ok()? < 24
        && p[4][9..11].parse::<u32>().ok()? < 60
        && p[4][11..].parse::<u32>().ok()? < 60)
        .then_some((h, v))
}
pub fn matches(path: &str, id: &str, key: &str) -> bool {
    if identity(id).is_none() {
        return false;
    }
    let p: Vec<_> = id.split('.').collect();
    let band = match suffix(key) {
        Some(value) => value,
        _ => return false,
    };
    path == format!(
        "/modis-061-cogs/{}/{}/{}/{}/{}{}",
        p[0],
        &p[2][1..3],
        &p[2][4..],
        &p[1][1..],
        id,
        band
    )
}
pub fn asset_path(path: &str) -> bool {
    ["red", "green", "blue", "modis_qc", "modis_state"]
        .iter()
        .any(|key| {
            path.rsplit('/')
                .next()
                .and_then(|s| s.strip_suffix(suffix(key)?))
                .is_some_and(|id| matches(path, id, key))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn modis_period_tile_version_and_band_paths_are_bound() {
        let id = "MYD09A1.A2025177.h08v05.061.2025189031924";
        let href =
            format!("https://{HOST}/modis-061-cogs/MYD09A1/08/05/2025177/{id}_sur_refl_b01.tif");
        let url = crate::providers::asset_url(&href).unwrap();
        assert!(crate::providers::matches_item(&url, id, "red"));
        assert!(!crate::providers::matches_item(&url, id, "green"));
        assert!(!crate::providers::matches_item(
            &url,
            &id.replace("MYD", "MOD"),
            "red"
        ));
        for bad in [
            format!("{href}?sig=SECRET"),
            href.replace("/08/", "/09/"),
            href.replace(".061.", ".060."),
            href.replace("2025177", "2025178"),
            href.replace("h08v05", "h36v05"),
            href.replace("031924", "251924"),
        ] {
            assert!(crate::providers::asset_url(&bad).is_err());
        }
        assert!(identity("MOD09A1.A2025361.h08v05.061.2026001031924").is_some());
        assert!(identity("MOD09A1.A2025366.h08v05.061.2026001031924").is_none());
    }
}
