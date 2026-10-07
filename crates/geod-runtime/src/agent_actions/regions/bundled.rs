use super::*;
const COUNTRIES: &str = include_str!(
    "../../../../../prototype/public/basemaps/natural-earth-50m-admin-0-countries.geojson"
);
const SUBDIVISIONS: &str =
    include_str!("../../../../../prototype/public/basemaps/admin1-10m/index.json");
static DATA: OnceLock<Result<Bundle>> = OnceLock::new();
const ALIASES: &str =
    include_str!("../../../../../prototype/public/basemaps/admin1-10m/agent-aliases.json");
const SHAPES: &[u8] = include_bytes!("../../../fixtures/boundaries/admin1.negb");
pub(super) fn geometry(region: &Region) -> Result<crate::crop::PolygonGeometry> {
    let geometry = if region.admin_level == 0 {
        let doc: Value = serde_json::from_str(COUNTRIES).map_err(|_| INVALID)?;
        doc["features"]
            .as_array()
            .ok_or(INVALID)?
            .iter()
            .find(|f| f["properties"]["ADM0_A3"] == region.country_code)
            .ok_or(INVALID)?["geometry"]
            .clone()
    } else if region.admin_level == 1 {
        use std::io::Read;
        if SHAPES.get(..6) != Some(b"NEGB1\0") {
            return Err(INVALID.into());
        }
        let size = u32::from_le_bytes(
            SHAPES
                .get(6..10)
                .ok_or(INVALID)?
                .try_into()
                .map_err(|_| INVALID)?,
        ) as usize;
        let index: Value = serde_json::from_slice(SHAPES.get(10..10 + size).ok_or(INVALID)?)
            .map_err(|_| INVALID)?;
        let entry = &index[&region.country_code];
        let offset = entry["offset"].as_u64().ok_or(INVALID)? as usize;
        let count = entry["bytes"].as_u64().ok_or(INVALID)? as usize;
        let start = 10usize
            .checked_add(size)
            .and_then(|n| n.checked_add(offset))
            .ok_or(INVALID)?;
        let end = start.checked_add(count).ok_or(INVALID)?;
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(SHAPES.get(start..end).ok_or(INVALID)?)
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(io_error)?;
        if bytes.len() > 16 * 1024 * 1024
            || digest(&bytes) != entry["sha256"].as_str().ok_or(INVALID)?
        {
            return Err(INVALID.into());
        }
        let doc: Value = serde_json::from_slice(&bytes).map_err(|_| INVALID)?;
        let code = region.id.strip_prefix("ne:ADM1:").ok_or(INVALID)?;
        doc["features"]
            .as_array()
            .ok_or(INVALID)?
            .iter()
            .find(|f| {
                f["properties"]["adm1_code"] == code
                    && f["properties"]["adm0_a3"] == region.country_code
            })
            .ok_or(INVALID)?["geometry"]
            .clone()
    } else {
        return Err(INVALID.into());
    };
    let geometry: crate::crop::PolygonGeometry =
        serde_json::from_value(geometry).map_err(|_| INVALID)?;
    geometry.bounds()?;
    Ok(geometry)
}
struct Bundle {
    regions: Vec<Region>,
    codes: std::collections::BTreeMap<String, String>,
}
fn data() -> Result<&'static Bundle> {
    DATA.get_or_init(load).as_ref().map_err(Clone::clone)
}
pub(super) fn regions() -> Result<&'static [Region]> {
    Ok(&data()?.regions)
}
pub(super) fn iso3(code: &str) -> Result<String> {
    // gbOpen accepts ISO3 country codes even where this reference ADM0 omits
    // a small territory. Validation cannot turn a country code into a URL.
    if code.len() == 3 && country_code(code) {
        return Ok(code.to_owned());
    }
    data()?.codes.get(code).cloned().ok_or_else(|| "Choose a known ISO country code; use a three-letter code for a territory absent from the reference layer.".into())
}
pub(super) fn country_name(iso: &str) -> String {
    regions()
        .ok()
        .and_then(|r| {
            r.iter()
                .find(|r| r.admin_level == 0 && r.country_code == iso)
        })
        .map(|r| r.name.clone())
        .unwrap_or_else(|| iso.into())
}
pub(super) fn provenance() -> Value {
    json!({"provider":"Natural Earth","license":"Public domain","source":"https://www.naturalearthdata.com/",
        "aliasesSha256":format!("{:x}",Sha256::digest(ALIASES.as_bytes())),
        "datasets":[{"level":0,"scale":"1:50m","units":242,"sha256":format!("{:x}",Sha256::digest(COUNTRIES.as_bytes()))},
            {"level":1,"scale":"1:10m","version":"5.1.1","units":4596,"sha256":format!("{:x}",Sha256::digest(SUBDIVISIONS.as_bytes()))}],
        "cached":true,"offline":true,"coverage":"Bundled ADM0 and ADM1 only; detailed levels use country-specific geoBoundaries coverage. Reference geometry is not a legal boundary determination."})
}
fn special(code: &str, name: &mut String, aliases: &mut Vec<String>) {
    match code {
        "CHN" => aliases.extend(["中国", "中國", "China"].map(String::from)),
        "KOR" => aliases.extend(["韩国", "韓國", "Korea"].map(String::from)),
        "PRK" => aliases.extend(["朝鲜", "朝鮮"].map(String::from)),
        "HKG" => {
            *name = "Hong Kong".into();
            aliases.extend(
                ["香港", "香港特别行政区", "香港特別行政區", "Hong Kong SAR"].map(String::from),
            );
        }
        "MAC" => {
            *name = "Macau".into();
            aliases.extend(
                ["澳门", "澳門", "澳门特别行政区", "澳門特別行政區", "Macao"].map(String::from),
            );
        }
        "TWN" => {
            *name = "Taiwan".into();
            aliases.extend(["台湾", "台灣", "臺灣", "台湾地区", "台灣地區"].map(String::from));
        }
        _ => (),
    }
}
fn load() -> Result<Bundle> {
    let document: Value = serde_json::from_str(COUNTRIES).map_err(|_| INVALID)?;
    let features = document["features"]
        .as_array()
        .filter(|f| f.len() == 242)
        .ok_or(INVALID)?;
    let mut regions = Vec::new();
    let mut codes = std::collections::BTreeMap::new();
    for feature in features {
        let p = feature["properties"].as_object().ok_or(INVALID)?;
        let iso = p.get("ADM0_A3").and_then(Value::as_str).ok_or(INVALID)?;
        let mut name = p
            .get("NAME_EN")
            .and_then(Value::as_str)
            .ok_or(INVALID)?
            .to_owned();
        let mut aliases = p
            .iter()
            .filter(|(k, _)| k.starts_with("NAME"))
            .filter_map(|(_, v)| v.as_str())
            .filter(|v| text(v, 200))
            .map(String::from)
            .collect::<Vec<_>>();
        special(iso, &mut name, &mut aliases);
        aliases.sort();
        aliases.dedup();
        for key in ["ISO_A2", "ISO_A2_EH", "ISO_A3", "ISO_A3_EH", "ADM0_A3"] {
            if let Some(code) = p
                .get(key)
                .and_then(Value::as_str)
                .filter(|c| country_code(c))
            {
                // A dependent territory can reuse its sovereign ISO code in
                // Natural Earth fallback fields. Never overwrite the actual
                // ISO country's mapping with that source-specific grouping.
                if p.get("ISO_A3").and_then(Value::as_str) == Some(iso) {
                    codes.insert(code.into(), iso.into());
                } else {
                    codes.entry(code.into()).or_insert_with(|| iso.into());
                }
            }
        }
        let bounds = envelope(&feature["geometry"])?;
        regions.push(Region {
            id: format!("ne:ADM0:{iso}"),
            name: name.clone(),
            aliases,
            country_code: iso.into(),
            country_name: name,
            admin_level: 0,
            bounds,
            limitation: extent_limitation(bounds),
        });
    }
    let index: Value = serde_json::from_str(SUBDIVISIONS).map_err(|_| INVALID)?;
    let areas = index["areas"]
        .as_array()
        .filter(|a| a.len() == 4596)
        .ok_or(INVALID)?;
    let alias_doc: Value = serde_json::from_str(ALIASES).map_err(|_| INVALID)?;
    let alias_index = alias_doc["aliases"]
        .as_object()
        .filter(|a| a.len() == 4596)
        .ok_or(INVALID)?;
    if alias_doc["sourceSha256"]
        != "efc59726337323058f9446210adc96673179cd344e053666ee3d28cb58ba2b05"
        || alias_doc["version"] != "5.1.1"
    {
        return Err(INVALID.into());
    }
    for a in areas {
        let s = |key| a[key].as_str().ok_or(INVALID);
        let iso = s("parentCode")?;
        let name = s("nameEn")?.to_owned();
        let mut aliases: Vec<String> =
            serde_json::from_value(alias_index.get(s("code")?).ok_or(INVALID)?.clone())
                .map_err(|_| INVALID)?;
        aliases.extend(
            [s("nameZh")?, s("nameLocal")?]
                .into_iter()
                .filter(|s| text(s, 200))
                .map(String::from),
        );
        aliases.sort();
        aliases.dedup();
        let bounds =
            serde_json::from_value::<[f64; 4]>(a["bounds"].clone()).map_err(|_| INVALID)?;
        let country_name = regions
            .iter()
            .find(|r| r.admin_level == 0 && r.country_code == iso)
            .map(|r| r.name.clone())
            .unwrap_or_else(|| iso.into());
        let mut limitation = s("clipLimitation")?.to_owned();
        if limitation.is_empty() {
            limitation = extent_limitation(bounds);
        }
        regions.push(Region {
            id: format!("ne:ADM1:{}", s("code")?),
            name,
            aliases,
            country_code: iso.into(),
            country_name,
            admin_level: 1,
            bounds,
            limitation,
        });
    }
    if regions.iter().any(|r| !r.valid()) {
        return Err(INVALID.into());
    }
    Ok(Bundle { regions, codes })
}
