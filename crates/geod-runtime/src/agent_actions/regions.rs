//! Global administrative lookup: bundled multilingual ADM0/1 and verified,
//! country-specific geoBoundaries gbOpen levels. No coordinate/name guesses.
use super::*;
use std::sync::OnceLock;
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};
mod bundled;
mod remote;
#[cfg(test)]
mod tests;

const CACHE_DAYS: i64 = 7;
const INVALID: &str = "Administrative source returned invalid boundary data.";

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Query {
    query: String,
    country_code: Option<String>,
    admin_level: Option<u8>,
    #[serde(default = "default_limit")]
    limit: usize,
}
fn default_limit() -> usize {
    5
}
impl Query {
    fn validate(&self) -> Result<()> {
        if !text(&self.query, 120)
            || fold(&self.query).is_empty()
            || self.query.contains("://")
            || self.query.contains(['/', '\\'])
            || !(1..=10).contains(&self.limit)
            || self.admin_level.is_some_and(|l| l > 5)
            || self.country_code.as_ref().is_some_and(|c| !country_code(c))
        {
            return Err("Use an administrative name with an ISO country code and a level from zero to five.".into());
        }
        if self.admin_level.is_some_and(|l| l >= 2) && self.country_code.is_none() {
            return Err("Choose a country before querying detailed administrative levels.".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct LevelsQuery {
    country_code: String,
}
fn text(v: &str, max: usize) -> bool {
    !v.is_empty() && v.trim() == v && v.chars().count() <= max && !v.chars().any(char::is_control)
}
fn country_code(v: &str) -> bool {
    (v.len() == 2 || v.len() == 3) && v.bytes().all(|c| c.is_ascii_uppercase()) && v != "ALL"
}
fn fold(v: &str) -> String {
    v.nfkd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
fn stable_id(v: &str) -> String {
    let hash = Sha256::digest(v.as_bytes());
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&hash[..16]);
    Uuid::from_bytes(bytes).to_string()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Region {
    id: String,
    name: String,
    aliases: Vec<String>,
    country_code: String,
    country_name: String,
    admin_level: u8,
    bounds: [f64; 4],
    #[serde(default, skip_serializing_if = "String::is_empty")]
    limitation: String,
}
impl Region {
    fn valid(&self) -> bool {
        text(&self.id, 120)
            && text(&self.name, 200)
            && text(&self.country_name, 200)
            && self.country_code.len() == 3
            && country_code(&self.country_code)
            && self.admin_level <= 5
            && valid_bounds(self.bounds)
            && self.aliases.len() <= 40
            && self.aliases.iter().all(|a| text(a, 200))
            && self.limitation.chars().count() <= 300
            && !self.limitation.chars().any(char::is_control)
    }
    fn rank(&self, query: &str) -> Option<u8> {
        std::iter::once(&self.name)
            .chain(self.aliases.iter())
            .filter_map(|name| {
                let name = fold(name);
                if name == query {
                    Some(0)
                } else if name.starts_with(query) {
                    Some(1)
                } else if name.contains(query) {
                    Some(2)
                } else {
                    None
                }
            })
            .min()
    }
}
fn matches(regions: &[Region], query: &Query, iso: Option<&str>) -> Vec<Region> {
    let name = fold(&query.query);
    let mut hits = regions
        .iter()
        .filter(|r| {
            iso.is_none_or(|c| c == r.country_code)
                && query.admin_level.is_none_or(|l| l == r.admin_level)
        })
        .filter_map(|r| r.rank(&name).map(|rank| (rank, r)))
        .collect::<Vec<_>>();
    hits.sort_by(|(a, x), (b, y)| {
        (a, x.admin_level, &x.country_code, &x.name, &x.id).cmp(&(
            b,
            y.admin_level,
            &y.country_code,
            &y.name,
            &y.id,
        ))
    });
    hits.into_iter().map(|(_, r)| r.clone()).collect()
}
fn result(query: &Query, mut hits: Vec<Region>, source: Value) -> Value {
    let count = hits.len();
    hits.truncate(query.limit);
    let candidates=hits.iter().map(|region| {
        let mut value=serde_json::to_value(region).unwrap();
        value["boundarySource"]=json!({"provider":if region.id.starts_with("ne:") {"natural-earth"} else {"geoboundaries"},"candidateId":region.id,"countryCode":region.country_code,"adminLevel":region.admin_level});
        value
    }).collect::<Vec<_>>();
    json!({"query":query.query,"countryCode":query.country_code,"adminLevel":query.admin_level,
        "candidates":candidates,"matchCount":count,"truncated":count>query.limit,"provenance":source,
        "boundsMeaning":"Actual source geometry envelope in WGS84 west south east north; a search rectangle, not the polygon or proof of imagery coverage.",
        "hierarchyMeaning":"ADM0 is a source country or region grouping. ADM1 and deeper levels follow each source country's hierarchy, not a universal city or county label. Intermediate parents are not inferred from a bounding box.",
        "next":"Use the exact relevant candidate bounds in geod_scene_search. For a polygon crop, read boundarySource with geod_boundary_read; search envelopes do not mean source polygons are absent. Respect level, precision and country; ask one short clarification for genuinely ambiguous matches. For a missing city or district, read geod_region_levels for the known country and search its available detailed levels; geod_place_search is a separate city gazetteer fallback. Never substitute a same-named province for a requested city or invent coordinates."})
}
pub(super) async fn boundary(
    manager: &JobManager,
    provider: &str,
    id: &str,
    country: &str,
    level: u8,
) -> Result<(String, crate::crop::PolygonGeometry, Value)> {
    if !country_code(country) || level > 5 {
        return Err(INVALID.into());
    }
    let iso = bundled::iso3(country)?;
    if provider == "natural-earth" {
        let region = bundled::regions()?
            .iter()
            .find(|r| r.id == id && r.country_code == iso && r.admin_level == level)
            .ok_or("Use the exact administrative candidate reference.")?;
        let geometry = bundled::geometry(region)?;
        return Ok((region.name.clone(), geometry, bundled::provenance()));
    }
    if provider != "geoboundaries" || !id.starts_with("gb:") {
        return Err(INVALID.into());
    }
    remote::boundary(manager, id, &iso, level).await
}
pub(super) async fn search(manager: &JobManager, query: Query) -> Result<Value> {
    query.validate()?;
    let iso = query
        .country_code
        .as_deref()
        .map(bundled::iso3)
        .transpose()?;
    if query.admin_level.is_none_or(|l| l < 2) {
        let hits = matches(bundled::regions()?, &query, iso.as_deref());
        if !hits.is_empty() || query.admin_level.is_none() || iso.is_none() {
            return Ok(result(&query, hits, bundled::provenance()));
        }
        // Reference subdivisions do not provide every current country's ADM1
        // structure. A country-scoped explicit level can use gbOpen coverage.
    }
    let iso = iso.ok_or("Choose a country before querying detailed administrative levels.")?;
    remote::search(manager, query, &iso).await
}
pub(super) async fn levels(manager: &JobManager, query: LevelsQuery) -> Result<Value> {
    if !country_code(&query.country_code) {
        return Err("Choose a valid ISO country code.".into());
    }
    let iso = bundled::iso3(&query.country_code)?;
    remote::levels(manager, &iso).await
}

// Walk only Polygon/MultiPolygon rings, with explicit limits and CRS checks.
// Unknown or malformed geometry aborts the dataset instead of dropping units.
fn envelope(geometry: &Value) -> Result<[f64; 4]> {
    let polygons: Vec<&Value> = match geometry["type"].as_str() {
        Some("Polygon") => vec![&geometry["coordinates"]],
        Some("MultiPolygon") => geometry["coordinates"]
            .as_array()
            .filter(|p| !p.is_empty())
            .ok_or(INVALID)?
            .iter()
            .collect(),
        _ => return Err(INVALID.into()),
    };
    let mut extent = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    let mut count = 0usize;
    for polygon in polygons {
        let rings = polygon
            .as_array()
            .filter(|r| !r.is_empty())
            .ok_or(INVALID)?;
        for ring in rings {
            let positions = ring
                .as_array()
                .filter(|r| r.len() >= 4 && r.first() == r.last())
                .ok_or(INVALID)?;
            for position in positions {
                count += 1;
                let p = position
                    .as_array()
                    .filter(|p| p.len() == 2)
                    .ok_or(INVALID)?;
                let (x, y) = p[0].as_f64().zip(p[1].as_f64()).ok_or(INVALID)?;
                if count > 2_000_000
                    || !x.is_finite()
                    || !y.is_finite()
                    || !(-180.0..=180.0).contains(&x)
                    || !(-90.0..=90.0).contains(&y)
                {
                    return Err(INVALID.into());
                }
                extent[0] = extent[0].min(x);
                extent[1] = extent[1].min(y);
                extent[2] = extent[2].max(x);
                extent[3] = extent[3].max(y);
            }
        }
    }
    if valid_bounds(extent) {
        Ok(extent)
    } else {
        Err(INVALID.into())
    }
}
fn extent_limitation(bounds: [f64; 4]) -> String {
    if bounds[2] - bounds[0] > 180.0 {
        "Wide or antimeridian extent; split the search area before imagery acquisition.".into()
    } else if bounds[1] < -85.0 || bounds[3] > 85.0 {
        "Polar extent; verify the selected imagery provider latitude limits.".into()
    } else {
        String::new()
    }
}
