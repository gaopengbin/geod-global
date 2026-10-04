//! Reviewed Planetary Computer Sentinel-1 IW RTC polarization COGs.
pub const HOST: &str = "sentinel1euwestrtc.blob.core.windows.net";
pub const MAX_BYTES: u64 = 4 * 1024 * 1024 * 1024;
pub const KEYS: &[&str] = &["vv", "vh", "hh", "hv"];

pub(crate) fn identity(path: &str) -> Option<(String, String)> {
    let p: Vec<_> = path.strip_prefix('/')?.split('/').collect();
    if p.len() != 10
        || p[0] != "sentinel1-grd-rtc"
        || p[1] != "GRD"
        || p[5] != "IW"
        || p[8] != "measurement"
    {
        return None;
    }
    let s: Vec<_> = p[7].split('_').collect();
    if s.len() != 9
        || !matches!(s[0], "S1A" | "S1B" | "S1C" | "S1D")
        || s[1] != "IW"
        || s[2] != "GRDH"
        || !matches!(s[3], "1SDV" | "1SDH" | "1SSV" | "1SSH")
        || s[6].len() != 6
        || !s[6].bytes().all(|v| v.is_ascii_digit())
        || s[7].len() != 6
        || s[8].len() != 4
        || !s[7..].iter().all(|v| {
            v.bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_lowercase())
        })
    {
        return None;
    }
    let start = chrono::NaiveDateTime::parse_from_str(s[4], "%Y%m%dT%H%M%S").ok()?;
    let end = chrono::NaiveDateTime::parse_from_str(s[5], "%Y%m%dT%H%M%S").ok()?;
    if s[4].len() != 15
        || s[5].len() != 15
        || end < start
        || end - start > chrono::Duration::minutes(5)
        || p[2] != start.format("%Y").to_string()
        || p[3] != start.format("%-m").to_string()
        || p[4] != start.format("%-d").to_string()
        || p[6] != &s[3][2..]
    {
        return None;
    }
    let key = p[9].strip_prefix("iw-")?.strip_suffix(".rtc.tiff")?;
    let allowed: &[&str] = match p[6] {
        "DV" => &["vv", "vh"],
        "DH" => &["hh", "hv"],
        "SV" => &["vv"],
        "SH" => &["hh"],
        _ => return None,
    };
    allowed.contains(&key).then(|| {
        (
            format!("{}_rtc", s[..if s[0] == "S1C" { 7 } else { 8 }].join("_")),
            key.into(),
        )
    })
}
pub(crate) fn matches(path: &str, id: &str, key: &str) -> bool {
    // Two reviewed catalogue generations differ by the datatake segment.
    // AccessCache still requires the exact official item/asset href pair.
    identity(path).is_some_and(|(item, band)| {
        let product = path.split('/').nth(8).unwrap_or("");
        let parts: Vec<_> = product.split('_').collect();
        band == key
            && (item == id
                || id == format!("{}_rtc", parts[..7].join("_"))
                || id == format!("{}_rtc", parts[..8].join("_")))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rtc_paths_bind_date_product_mode_and_polarization() {
        let path = "/sentinel1-grd-rtc/GRD/2025/6/30/IW/DV/S1C_IW_GRDH_1SDV_20250630T140654_20250630T140719_003013_00622B_A30F/measurement/iw-vv.rtc.tiff";
        let short = "S1C_IW_GRDH_1SDV_20250630T140654_20250630T140719_003013_rtc";
        let long = "S1C_IW_GRDH_1SDV_20250630T140654_20250630T140719_003013_00622B_rtc";
        assert!(matches(path, short, "vv"));
        assert!(matches(path, long, "vv"));
        assert!(matches(&path.replace("iw-vv", "iw-vh"), short, "vh"));
        assert!(!matches(path, short, "vh"));
        for invalid in [
            path.replace("/2025/6/30/", "/2025/6/29/"),
            path.replace("/IW/", "/EW/"),
            path.replace("iw-vv", "iw-hh"),
            path.replace("140719", "150719"),
            path.replace("140719", "140000"),
            path.replace("00622B", "00622b"),
            format!("{path}/extra"),
            path.replace(".rtc.tiff", ".tif"),
        ] {
            assert!(identity(&invalid).is_none(), "{invalid}");
        }
    }

    #[test]
    fn sentinel_1d_keeps_exact_scene_and_polarization_binding() {
        let path = "/sentinel1-grd-rtc/GRD/2026/9/30/IW/DV/S1D_IW_GRDH_1SDV_20260930T140731_20260930T140756_004808_00905F_1D03/measurement/iw-vv.rtc.tiff";
        let id = "S1D_IW_GRDH_1SDV_20260930T140731_20260930T140756_004808_00905F_rtc";
        assert_eq!(identity(path), Some((id.into(), "vv".into())));
        assert!(matches(path, id, "vv"));
        assert!(matches(&path.replace("iw-vv", "iw-vh"), id, "vh"));
        assert!(!matches(path, id, "vh"));
        assert!(!matches(path, &id.replace("00905F", "00905E"), "vv"));
        assert!(!matches(path, &id.replace("S1D", "S1C"), "vv"));
        for bad in [
            path.replace("S1D", "S1E"),
            path.replace("S1D", "S1d"),
            path.replace("/2026/9/30/", "/2026/9/29/"),
            path.replace("/DV/", "/DH/"),
            path.replace("iw-vv", "iw-hh"),
        ] {
            assert!(identity(&bad).is_none(), "{bad}");
        }
    }
}
