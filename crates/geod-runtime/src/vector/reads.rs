//! Bounded, verified local vector reads shared by MCP, Agent and loopback.
//! Reading a saved service never queries it or creates an extraction.
use crate::{io_error, JobManager, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const PAGE_BYTES: usize = 24 * 1024;
const INLINE_BYTES: usize = 1024;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}
impl Page {
    pub fn validate(&self) -> Result<()> {
        if self.limit.is_some_and(|v| v == 0 || v > 100)
            || self.offset.is_some_and(|v| v > 1_000_000)
        {
            return Err("Vector read limit must be 1..100 and offset 0..1000000".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub feature: usize,
    #[serde(default)]
    pub pointer: String,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}
impl Node {
    pub fn validate(&self) -> Result<()> {
        Page {
            offset: None,
            limit: self.limit,
        }
        .validate()?;
        if self.feature >= super::MAX_FEATURES
            || self.offset.is_some_and(|v| v > super::MAX_BYTES)
            || self.pointer.len() > 4096
            || (!self.pointer.is_empty() && !self.pointer.starts_with('/'))
        {
            return Err("Choose a valid vector feature index and JSON pointer".into());
        }
        let mut chars = self.pointer.chars();
        while let Some(c) = chars.next() {
            if c == '~' && !matches!(chars.next(), Some('0' | '1')) {
                return Err("JSON pointer escapes must be ~0 or ~1".into());
            }
        }
        Ok(())
    }
}

fn hash(value: &Value) -> Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).map_err(io_error)?)
    ))
}
fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
fn escape(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}
fn reference(id: &str, feature: usize, pointer: &str) -> Value {
    json!({"tool":"geod_vector_node","arguments":{"id":id,"feature":feature,"pointer":pointer,"offset":0,"limit":5}})
}
fn child(value: &Value, id: &str, feature: usize, pointer: &str) -> Value {
    if value.to_string().len() <= INLINE_BYTES {
        json!({"nodeType":kind(value),"inline":value})
    } else {
        json!({"nodeType":kind(value),"valueOmitted":true,"read":reference(id,feature,pointer)})
    }
}
fn page(values: &[Value], args: Page, key: &str) -> Result<Value> {
    args.validate()?;
    let offset = args.offset.unwrap_or(0);
    let limit = args.limit.unwrap_or(20);
    let mut rows = Vec::new();
    let mut bytes = 0;
    for value in values.iter().skip(offset).take(limit) {
        let size = value.to_string().len();
        if bytes + size > PAGE_BYTES {
            if rows.is_empty() {
                return Err("One vector metadata entry exceeds the bounded read limit".into());
            }
            break;
        }
        bytes += size;
        rows.push(value.clone());
    }
    let end = offset.saturating_add(rows.len());
    Ok(
        json!({key:rows,"total":values.len(),"offset":offset,"nextOffset":(end<values.len()).then_some(end),"complete":end>=values.len()}),
    )
}
fn asset_summary(asset: &super::VectorAsset) -> Value {
    json!({"id":asset.id,"name":asset.name,"format":asset.format,"storageMode":asset.storage_mode,
        "crs":asset.crs,"sourceSha256":asset.source_sha256,"geojsonSha256":asset.geojson_sha256,
        "bytes":asset.bytes,"featureCount":asset.feature_count,"coordinateCount":asset.coordinate_count,
        "geometryCounts":asset.geometry_counts,"bounds":asset.bounds,"createdAt":asset.created_at,
        "dataTimestamp":asset.data_timestamp,"attribution":asset.attribution,"licenseUrl":asset.license_url})
}
fn verified(inspection: &super::VectorInspection) -> Value {
    json!({"assetId":inspection.asset.id,"sourceSha256":inspection.asset.source_sha256,
        "geojsonSha256":inspection.asset.geojson_sha256,"verified":true,
        "representation":"native normalized WGS84 GeoJSON; original file and converted representation have separate hashes"})
}
fn protocol(service: &crate::features::FeatureService) -> &'static str {
    if service.wfs.is_some() {
        "WFS2"
    } else if service.overpass.is_some() {
        "Overpass"
    } else if service.arcgis.is_some() {
        "ArcGIS"
    } else {
        "OGC"
    }
}

impl JobManager {
    pub async fn feature_service_metadata(&self, args: Page) -> Result<Value> {
        let services = self.list_feature_services().await;
        let rows = services.iter().map(|s| json!({"id":s.id,"name":s.name,"title":s.title,
            "url":s.url,"protocol":protocol(s),"connectedAt":s.connected_at,"collectionCount":s.collections.len(),
            "collectionsTool":{"tool":"geod_feature_collections","arguments":{"id":s.id,"limit":5}},
            "savedMetadataOnly":true,"liveAvailabilityChecked":false})).collect::<Vec<_>>();
        page(&rows, args, "services")
    }
    pub async fn feature_collection_metadata(&self, id: &str, args: Page) -> Result<Value> {
        let services = self.list_feature_services().await;
        let service = services
            .iter()
            .find(|s| s.id == id)
            .ok_or("Unknown data service")?;
        let rows = service
            .collections
            .iter()
            .map(serde_json::to_value)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(io_error)?;
        let mut result = page(&rows, args, "collections")?;
        result["serviceId"] = json!(id);
        result["protocol"] = json!(protocol(service));
        result["savedMetadataOnly"] = json!(true);
        result["liveAvailabilityChecked"] = json!(false);
        Ok(result)
    }
    pub async fn vector_metadata_list(&self, args: Page) -> Result<Value> {
        let rows = self
            .list_vectors()
            .await
            .iter()
            .map(asset_summary)
            .collect::<Vec<_>>();
        let mut result = page(&rows, args, "vectors")?;
        result["verified"] = json!(false);
        result["note"] = json!("Saved registry metadata only. Inspect or read a feature to verify the current original bytes and native conversion.");
        Ok(result)
    }
    pub async fn vector_metadata(&self, id: &str) -> Result<Value> {
        let inspection = self.inspect_vector(id).await?;
        let mut result = verified(&inspection);
        result["asset"] = serde_json::to_value(&inspection.asset).map_err(io_error)?;
        result["geojsonOmitted"] = json!(true);
        result["featuresTool"] =
            json!({"tool":"geod_vector_features","arguments":{"id":id,"offset":0,"limit":5}});
        Ok(result)
    }
    pub async fn vector_features(&self, id: &str, args: Page) -> Result<Value> {
        args.validate()?;
        let inspection = self.inspect_vector(id).await?;
        let features = inspection.geojson["features"]
            .as_array()
            .ok_or("Invalid vector features")?;
        let offset = args.offset.unwrap_or(0);
        let mut rows = Vec::new();
        for (index, feature) in features
            .iter()
            .enumerate()
            .skip(offset)
            .take(args.limit.unwrap_or(20))
        {
            rows.push(json!({"index":index,"identity":feature.get("id").map(|v|child(v,id,index,"/id")),
                "geometryType":feature["geometry"].get("type"),"propertiesCount":feature["properties"].as_object().map_or(0,|o|o.len()),
                "featureSha256":hash(feature)?,"detailsOmitted":true,"read":reference(id,index,"")}));
        }
        // The page already starts at the requested offset; preserve the total native count.
        let mut result = page(
            &rows,
            Page {
                offset: Some(0),
                limit: args.limit,
            },
            "features",
        )?;
        let end = offset + result["features"].as_array().unwrap().len();
        result["offset"] = json!(offset);
        result["total"] = json!(features.len());
        result["nextOffset"] = json!((end < features.len()).then_some(end));
        result["complete"] = json!(end >= features.len());
        result["source"] = verified(&inspection);
        Ok(result)
    }
    pub async fn vector_node(&self, id: &str, args: Node) -> Result<Value> {
        args.validate()?;
        let inspection = self.inspect_vector(id).await?;
        let feature = inspection.geojson["features"]
            .as_array()
            .and_then(|v| v.get(args.feature))
            .ok_or("Unknown vector feature index")?;
        let node = feature
            .pointer(&args.pointer)
            .ok_or("Unknown feature JSON pointer")?;
        let offset = args.offset.unwrap_or(0);
        let mut result = json!({"source":verified(&inspection),"feature":args.feature,"featureSha256":hash(feature)?,
            "pointer":args.pointer,"nodeType":kind(node),"nodeSha256":hash(node)?});
        match node {
            Value::Array(values) => {
                let entries = values.iter().enumerate().skip(offset).take(args.limit.unwrap_or(20)).map(|(index,v)|
                    json!({"index":index,"data":child(v,id,args.feature,&format!("{}/{index}",args.pointer))})).collect::<Vec<_>>();
                result["page"] = page(
                    &entries,
                    Page {
                        offset: Some(0),
                        limit: args.limit,
                    },
                    "entries",
                )?;
                result["page"]["total"] = json!(values.len());
            }
            Value::Object(values) => {
                let entries = values.iter().skip(offset).take(args.limit.unwrap_or(20)).map(|(key,v)|
                    json!({"key":key,"data":child(v,id,args.feature,&format!("{}/{}",args.pointer,escape(key)))})).collect::<Vec<_>>();
                result["page"] = page(
                    &entries,
                    Page {
                        offset: Some(0),
                        limit: args.limit,
                    },
                    "entries",
                )?;
                result["page"]["total"] = json!(values.len());
            }
            Value::String(text) => {
                // Classify the complete original before slicing. Otherwise an
                // offset into a URL/path could bypass the Agent prefix policy.
                let mut classified = node.clone();
                crate::mcp::sanitize_agent_value(&mut classified);
                result["agentTextSensitive"] = json!(classified != *node);
                let chunk = text.chars().skip(offset).take(4096).collect::<String>();
                let total = text.chars().count();
                let end = offset.saturating_add(chunk.chars().count());
                result["text"] = json!(chunk);
                result["total"] = json!(total);
                result["offset"] = json!(offset);
                result["nextOffset"] = json!((end < total).then_some(end));
                result["complete"] = json!(end >= total);
                result["offsetUnit"] = json!(
                    "Unicode scalar values; each chunk preserves the original string exactly"
                );
            }
            _ => {
                result["value"] = node.clone();
                result["complete"] = json!(true);
            }
        }
        if let Some(p) = result.get_mut("page") {
            let end = offset + p["entries"].as_array().unwrap().len();
            let total = p["total"].as_u64().unwrap() as usize;
            p["offset"] = json!(offset);
            p["nextOffset"] = json!((end < total).then_some(end));
            p["complete"] = json!(end >= total);
        }
        Ok(result)
    }
}
