//! Read-only access to saved services and verified native vector representations.
use super::{arguments, validate_id, Backend, ErrorData, IdArgs};
use crate::vector::reads::{Node, Page};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdPage {
    id: String,
    offset: Option<usize>,
    limit: Option<usize>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdNode {
    id: String,
    feature: usize,
    #[serde(default)]
    pointer: String,
    offset: Option<usize>,
    limit: Option<usize>,
}
pub(super) enum Operation {
    Services(Page),
    Collections(String, Page),
    Vectors(Page),
    Inspect(String),
    Features(String, Page),
    Node(String, Node),
}
pub(super) fn parse(name: &str, value: Value) -> Result<Operation, ErrorData> {
    let invalid = |e| ErrorData::invalid_params(e, None);
    Ok(match name {
        "geod_feature_services" | "geod_vectors_list" => {
            let p: Page = arguments(value)?;
            p.validate().map_err(invalid)?;
            if name == "geod_feature_services" {
                Operation::Services(p)
            } else {
                Operation::Vectors(p)
            }
        }
        "geod_feature_collections" | "geod_vector_features" => {
            let p: IdPage = arguments(value)?;
            validate_id(&p.id)?;
            let page = Page {
                offset: p.offset,
                limit: p.limit,
            };
            page.validate().map_err(invalid)?;
            if name == "geod_feature_collections" {
                Operation::Collections(p.id, page)
            } else {
                Operation::Features(p.id, page)
            }
        }
        "geod_vector_inspect" => {
            let p: IdArgs = arguments(value)?;
            validate_id(&p.id)?;
            Operation::Inspect(p.id)
        }
        "geod_vector_node" => {
            let p: IdNode = arguments(value)?;
            validate_id(&p.id)?;
            let node = Node {
                feature: p.feature,
                pointer: p.pointer,
                offset: p.offset,
                limit: p.limit,
            };
            node.validate().map_err(invalid)?;
            Operation::Node(p.id, node)
        }
        _ => return Err(invalid("Unknown vector read tool".into())),
    })
}
fn query(page: &Page) -> String {
    format!(
        "offset={}&limit={}",
        page.offset.unwrap_or(0),
        page.limit.unwrap_or(20)
    )
}
pub(super) async fn execute(backend: &Backend, operation: Operation) -> crate::Result<Value> {
    match backend {
        Backend::Direct(manager) => match operation {
            Operation::Services(p) => manager.feature_service_metadata(p).await,
            Operation::Collections(id, p) => manager.feature_collection_metadata(&id, p).await,
            Operation::Vectors(p) => manager.vector_metadata_list(p).await,
            Operation::Inspect(id) => manager.vector_metadata(&id).await,
            Operation::Features(id, p) => manager.vector_features(&id, p).await,
            Operation::Node(id, p) => manager.vector_node(&id, p).await,
        },
        Backend::Server { .. } => {
            let path = match operation {
                Operation::Services(p) => format!("/feature-services/metadata?{}", query(&p)),
                Operation::Collections(id, p) => {
                    format!("/feature-services/{id}/collections?{}", query(&p))
                }
                Operation::Vectors(p) => format!("/vectors/metadata?{}", query(&p)),
                Operation::Inspect(id) => format!("/vectors/{id}/metadata"),
                Operation::Features(id, p) => format!("/vectors/{id}/features?{}", query(&p)),
                Operation::Node(id, p) => {
                    let mut url = url::Url::parse("http://127.0.0.1").unwrap();
                    url.query_pairs_mut()
                        .append_pair("feature", &p.feature.to_string())
                        .append_pair("pointer", &p.pointer)
                        .append_pair("offset", &p.offset.unwrap_or(0).to_string())
                        .append_pair("limit", &p.limit.unwrap_or(20).to_string());
                    format!("/vectors/{id}/node?{}", url.query().unwrap())
                }
            };
            backend.http(reqwest::Method::GET, &path, None).await
        }
    }
}
pub(super) fn tools(id: &Value) -> Vec<(&'static str, &'static str, Value)> {
    let page = json!({"type":"object","properties":{"offset":{"type":"integer","minimum":0,"maximum":1000000,"default":0},"limit":{"type":"integer","minimum":1,"maximum":100,"default":20}},"additionalProperties":false});
    let mut id_page = page.clone();
    id_page["properties"]["id"] = id["properties"]["id"].clone();
    id_page["required"] = json!(["id"]);
    let mut node = id_page.clone();
    node["properties"]["offset"]["maximum"] = json!(crate::vector::MAX_BYTES);
    node["properties"]["feature"] = json!({"type":"integer","minimum":0,"maximum":49999,"description":"Zero-based native feature index from geod_vector_features"});
    node["properties"]["pointer"] = json!({"type":"string","maxLength":4096,"default":"","description":"JSON pointer within that feature, empty means the feature root. Follow returned read references for omitted values; never a file path."});
    node["required"] = json!(["id", "feature"]);
    vec![
        ("geod_feature_services","Read paginated metadata of vector services already saved in the app (OGC API Features, ArcGIS, WFS 2 or Overpass). No external request, connection or extraction; saved metadata does not establish current server availability.",page.clone()),
        ("geod_feature_collections","Read original saved collection declarations for an actual vector service ID; follow nextOffset. No service query or file creation.",id_page.clone()),
        ("geod_vectors_list","List registered local vector file summaries. Registry metadata is not verified content; use inspect/features/node to recheck original bytes and the native conversion.",page),
        ("geod_vector_inspect","Verify an actual registered vector original and its native WGS84 conversion; return metadata, source/conversion hashes and provenance. No full geometry dump, analysis, extraction or scientific accuracy claim.",id.clone()),
        ("geod_vector_features","Verify a registered vector and page feature identities, original indices, geometry types and hashes. Attribute/geometry details are explicitly omitted with geod_vector_node references. Follow nextOffset; never infer counts or analysis from a partial page.",id_page),
        ("geod_vector_node","Verify and read a node inside a feature's native GeoJSON representation. Objects/arrays are paged with original keys/indices; large child values return read references, never truncated values. Follow page.nextOffset and references. Strings use Unicode scalar offsets and up to 4096 scalars per chunk with nextOffset. No coordinates are simplified, transformed or rounded by this tool; binary sources use the existing declared native conversion. Agent secrets/paths are explicitly redacted; native MCP original data is unchanged.",node),
    ]
}

fn member_allowed(key: &str) -> bool {
    let mut v = json!({key:true});
    super::sanitize_agent_value(&mut v);
    v.get(key).is_some()
}
fn decode(token: &str) -> String {
    token.replace("~1", "/").replace("~0", "~")
}
/// Preserve trusted structural JSON pointers while redacting untrusted data.
/// Attribute keys still use the same credential/path policy as other Agent reads.
pub(super) fn sanitize_agent_result(name: &str, value: &mut Value) -> crate::Result<()> {
    if name != "geod_vector_node" {
        let before = value.clone();
        super::sanitize_agent_value(value);
        if name == "geod_vector_features" {
            if let (Some(original), Some(rows)) = (
                before["features"].as_array(),
                value["features"].as_array_mut(),
            ) {
                for (original, row) in original.iter().zip(rows) {
                    // These references are built by the native reader, not
                    // source properties. A JSON pointer is not a file path.
                    if original["identity"].get("read").is_some() {
                        row["identity"]["read"] = original["identity"]["read"].clone();
                    }
                }
            }
        }
        value["agentRedactionApplied"] = json!(*value != before);
        return Ok(());
    }
    let pointer = value["pointer"]
        .as_str()
        .ok_or("Invalid vector node pointer")?
        .to_string();
    let mut changed = false;
    if value["agentTextSensitive"] == true
        || pointer
            .split('/')
            .skip(1)
            .any(|token| !member_allowed(&decode(token)))
    {
        value.as_object_mut().unwrap().remove("page");
        value.as_object_mut().unwrap().remove("text");
        value.as_object_mut().unwrap().remove("value");
        value["valueOmitted"] = json!(true);
        value["redacted"] = json!(true);
        value["complete"] = json!(false);
        changed = true;
    } else if let Some(entries) = value
        .get_mut("page")
        .and_then(|v| v.get_mut("entries"))
        .and_then(Value::as_array_mut)
    {
        for entry in entries {
            if entry
                .get("key")
                .and_then(Value::as_str)
                .is_some_and(|key| !member_allowed(key))
            {
                entry["data"] = json!({"valueOmitted":true,"redacted":true});
                changed = true;
            } else if let Some(data) = entry.get_mut("data").and_then(|v| v.get_mut("inline")) {
                let before = data.clone();
                super::sanitize_agent_value(data);
                if *data != before {
                    entry["data"]["redacted"] = json!(true);
                    changed = true;
                }
            }
        }
    } else {
        for field in ["text", "value"] {
            if let Some(data) = value.get_mut(field) {
                let before = data.clone();
                super::sanitize_agent_value(data);
                if *data != before {
                    changed = true;
                }
            }
        }
    }
    // Source contains only trusted verification metadata, never native paths.
    if let Some(source) = value.get_mut("source") {
        super::sanitize_agent_value(source);
    }
    value["agentRedactionApplied"] = json!(changed);
    value["redactionNote"] = json!("Credential/path fields and transfer URLs are not available to the model. Redacted data is explicitly marked and remains in the original local file; do not claim the Agent view is complete.");
    Ok(())
}

#[cfg(test)]
mod tests;
