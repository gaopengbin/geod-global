//! Reviewed NASA/NOAA 09A1 v002 eight-day surface reflectance HDF5 originals.
use chrono::{Datelike, Duration, NaiveDate};

pub mod hdf;
pub mod prepare;

pub struct Identity {
    pub product: &'static str,
    pub collection: &'static str,
    pub production: String,
    pub start: NaiveDate,
    pub end: NaiveDate,
    pub horizontal_tile: u32,
    pub vertical_tile: u32,
}
pub fn identity(id: &str) -> Option<Identity> {
    if !id.is_ascii() {
        return None;
    }
    let p: Vec<_> = id.split('.').collect();
    if p.len() != 5 || p[3] != "002" {
        return None;
    }
    let (product, collection) = match p[0] {
        "VNP09A1" => ("VNP09A1", "VNP09A1_002"),
        "VJ109A1" => ("VJ109A1", "VJ109A1_002"),
        "VJ209A1" => ("VJ209A1", "VJ209A1_002"),
        _ => return None,
    };
    if p[1].len() != 8
        || !p[1].starts_with('A')
        || !p[1][1..].bytes().all(|b| b.is_ascii_digit())
        || p[2].len() != 6
        || !p[2].starts_with('h')
        || p[2].as_bytes()[3] != b'v'
        || !p[2][1..3]
            .bytes()
            .chain(p[2][4..].bytes())
            .all(|b| b.is_ascii_digit())
        || p[2][1..3].parse::<u32>().ok()? > 35
        || p[2][4..].parse::<u32>().ok()? > 17
        || p[4].len() != 13
        || !p[4].bytes().all(|b| b.is_ascii_digit())
        || p[4][7..9].parse::<u32>().ok()? > 23
        || p[4][9..11].parse::<u32>().ok()? > 59
        || p[4][11..].parse::<u32>().ok()? > 59
    {
        return None;
    }
    let year = p[1][1..5].parse::<i32>().ok()?;
    let doy = p[1][5..].parse::<u32>().ok()?;
    if !(2012..=9998).contains(&year) || doy % 8 != 1 {
        return None;
    }
    let start = NaiveDate::from_yo_opt(year, doy)?;
    let production = NaiveDate::from_yo_opt(p[4][..4].parse().ok()?, p[4][4..7].parse().ok()?)?;
    if production < start {
        return None;
    }
    let end = (start + Duration::days(7)).min(NaiveDate::from_ymd_opt(start.year(), 12, 31)?);
    Some(Identity {
        product,
        collection,
        production: p[4].into(),
        start,
        end,
        horizontal_tile: p[2][1..3].parse().ok()?,
        vertical_tile: p[2][4..].parse().ok()?,
    })
}
pub fn matches(path: &str, id: &str) -> bool {
    identity(id)
        .is_some_and(|p| path == format!("/lp-prod-protected/{}.002/{id}/{id}.h5", p.product))
}
pub fn asset_path(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .and_then(|s| s.strip_suffix(".h5"))
        .is_some_and(|id| matches(path, id))
}
pub fn period_matches(item: &serde_json::Value, id: &str) -> bool {
    let Some(p) = identity(id) else {
        return false;
    };
    [
        ("start_datetime", format!("{}T00:00:00Z", p.start)),
        ("end_datetime", format!("{}T23:59:59Z", p.end)),
    ]
    .iter()
    .all(|(key, expected)| {
        item["properties"][key]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            == chrono::DateTime::parse_from_rfc3339(expected).ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn viirs_platform_version_period_and_original_path_are_bound() {
        for product in ["VNP09A1", "VJ109A1", "VJ209A1"] {
            let id = format!("{product}.A2025177.h08v05.002.2025333224010");
            let p = identity(&id).unwrap();
            assert_eq!(p.collection, format!("{product}_002"));
            assert_eq!(p.start.to_string(), "2025-06-26");
            assert_eq!(p.end.to_string(), "2025-07-03");
            let path = format!("/lp-prod-protected/{product}.002/{id}/{id}.h5");
            assert!(asset_path(&path));
            assert!(!matches(&path.replace(".h5", ".tif"), &id));
            assert!(!matches(&path.replace("protected", "public"), &id));
        }
        let id = "VNP09A1.A2025361.h08v05.002.2026001224010";
        assert_eq!(identity(id).unwrap().end.to_string(), "2025-12-31");
        for bad in [
            "VNP09A1.A2025366.h08v05.002.2026001224010",
            "VNP09A1.A2025178.h08v05.002.2025333224010",
            "VNP09A1.A2025177.h36v05.002.2025333224010",
            "VNP09A1.A2025177.h08v18.002.2025333224010",
            "VNP09A1.A2025177.h08v05.001.2025333224010",
            "VNP09A1.A2025177.h08v05.002.2025333244010",
            "VNP09A1.A2025177.h08v05.002.2025001224010",
            "VNP09H1.A2025177.h08v05.002.2025333224010",
            "你好.A2025177.h08v05.002.2025333224010",
        ] {
            assert!(identity(bad).is_none(), "{bad}");
        }
    }
}
