//! Original OSM 0.6 snapshots. XML and raw/zlib PBF are decoded locally;
//! original bytes and explicit source metadata stay separate from map geometry.
use super::{osm, Result, MAX_BYTES, MAX_FEATURES};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
mod pbf;
#[cfg(test)]
mod tests;
mod xml;

const SAFE: i64 = 9_007_199_254_740_991;
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObjectCounts {
    pub node: usize,
    pub way: usize,
    pub relation: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Provenance {
    pub conversion_version: u32,
    pub encoding: String,
    pub schema_version: String,
    pub object_counts: ObjectCounts,
    pub generator: Option<String>,
    pub source: Option<String>,
    pub dataset_timestamp: Option<String>,
    pub declared_bounds: Option<[f64; 4]>,
    pub required_features: Vec<String>,
    pub optional_features: Vec<String>,
    pub replication_sequence: Option<String>,
    pub replication_base_url: Option<String>,
    pub ignored_block_types: Vec<String>,
    pub xml_root_attributes: BTreeMap<String, String>,
}
impl Provenance {
    fn new(encoding: &str) -> Self {
        Self {
            conversion_version: 1,
            encoding: encoding.into(),
            schema_version: "0.6".into(),
            object_counts: ObjectCounts::default(),
            generator: None,
            source: None,
            dataset_timestamp: None,
            declared_bounds: None,
            required_features: Vec::new(),
            optional_features: Vec::new(),
            replication_sequence: None,
            replication_base_url: None,
            ignored_block_types: Vec::new(),
            xml_root_attributes: BTreeMap::new(),
        }
    }
}
pub fn validate(source: &Provenance, count: usize) -> Result<()> {
    let c = &source.object_counts;
    let total = c
        .node
        .checked_add(c.way)
        .and_then(|n| n.checked_add(c.relation));
    if source.conversion_version != 1
        || source.schema_version != "0.6"
        || !["xml", "pbf"].contains(&source.encoding.as_str())
        || total != Some(count)
        || count > MAX_FEATURES
        || source
            .dataset_timestamp
            .as_deref()
            .is_some_and(|t| chrono::DateTime::parse_from_rfc3339(t).is_err())
        || source.declared_bounds.is_some_and(|b| !valid_bounds(b))
        || source.required_features.len() > 64
        || source.optional_features.len() > 64
        || source.ignored_block_types.len() > 64
        || source
            .required_features
            .iter()
            .any(|s| !["OsmSchema-V0.6", "DenseNodes"].contains(&s.as_str()))
        || (source.encoding == "pbf"
            && !source
                .required_features
                .iter()
                .any(|s| s == "OsmSchema-V0.6"))
        || (source.encoding == "xml"
            && (!source.required_features.is_empty()
                || !source.optional_features.is_empty()
                || !source.ignored_block_types.is_empty()
                || source.replication_sequence.is_some()
                || source.replication_base_url.is_some()
                || source.source.is_some()))
        || (source.encoding == "pbf" && !source.xml_root_attributes.is_empty())
        || source
            .replication_sequence
            .as_deref()
            .is_some_and(|s| s.parse::<u64>().is_err())
        || serde_json::to_vec(source)
            .map_err(super::super::io_error)?
            .len()
            > 65_536
    {
        return Err("Invalid local OSM original-file provenance".into());
    }
    Ok(())
}
fn valid_bounds([w, s, e, n]: [f64; 4]) -> bool {
    [w, s, e, n].iter().all(|v| v.is_finite())
        && -180. <= w
        && w <= e
        && e <= 180.
        && -90. <= s
        && s <= n
        && n <= 90.
}
fn integer(n: i64, positive: bool) -> Result<i64> {
    if n < i64::from(positive) || n > SAFE {
        return Err("OSM snapshot has a negative or unsafe object ID / metadata integer".into());
    }
    Ok(n)
}
fn push(elements: &mut Vec<Value>, source: &mut Provenance, value: Value) -> Result<()> {
    if elements.len() >= MAX_FEATURES {
        return Err("OSM snapshot exceeds 50,000 objects; use a smaller complete extract".into());
    }
    match value["type"].as_str() {
        Some("node") => source.object_counts.node += 1,
        Some("way") => source.object_counts.way += 1,
        Some("relation") => source.object_counts.relation += 1,
        _ => return Err("Unsupported OSM snapshot object".into()),
    }
    elements.push(value);
    Ok(())
}
pub(crate) fn is_xml(bytes: &[u8]) -> bool {
    bytes
        .strip_prefix(&[0xef, 0xbb, 0xbf])
        .unwrap_or(bytes)
        .iter()
        .copied()
        .find(|b| !b.is_ascii_whitespace())
        == Some(b'<')
}
pub(crate) fn is_pbf(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[0] == 0 && bytes[1] == 0
}
pub(super) fn decode(bytes: &[u8]) -> Result<(Value, Provenance)> {
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err("OSM snapshot must contain at most 20 MiB".into());
    }
    let (elements, source) = if is_xml(bytes) {
        xml::decode(bytes)?
    } else {
        pbf::decode(bytes)?
    };
    validate(&source, elements.len())?;
    let identities = elements
        .iter()
        .map(|e| (format!("{}/{}", e["type"].as_str().unwrap(), e["id"]), e))
        .collect::<BTreeMap<_, _>>();
    for element in &elements {
        let owner = format!("{}/{}", element["type"].as_str().unwrap(), element["id"]);
        let refs = if element["type"] == "way" && element.get("geometry").is_none() {
            element["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|id| format!("node/{id}"))
                .collect::<Vec<_>>()
        } else if element["type"] == "relation" {
            element["members"]
                .as_array()
                .unwrap()
                .iter()
                .map(|m| format!("{}/{}", m["type"].as_str().unwrap(), m["ref"]))
                .collect()
        } else {
            Vec::new()
        };
        for dependency in refs {
            if !identities.contains_key(&dependency) {
                return Err(format!(
                    "OSM {owner} has missing {dependency}; import a complete extract"
                ));
            }
        }
    }
    let raw = json!({"elements":elements,"osm3s":{"timestamp_osm_base":source.dataset_timestamp}});
    let mut data = osm::convert_elements(
        &raw,
        source.object_counts.node + source.object_counts.way + source.object_counts.relation,
    )?;
    data["geodLocalOsm"] = serde_json::to_value(&source).map_err(super::super::io_error)?;
    if serde_json::to_vec(&data)
        .map_err(super::super::io_error)?
        .len()
        > MAX_BYTES
    {
        return Err(
            "Converted OSM geometry and attributes exceed 20 MiB; use a smaller complete extract"
                .into(),
        );
    }
    Ok((data, source))
}
