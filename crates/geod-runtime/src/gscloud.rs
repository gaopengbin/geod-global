//! Public GSCloud metadata adapter. Dataset presence is not download entitlement.
//! Account sessions, ordering and original delivery are deliberately not inferred
//! from the public GeoAgent catalogue endpoints.
use crate::{JobManager, Result};
use chrono::NaiveDate;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, time::Duration};

const BASE: &str = "https://www.gscloud.cn/geoagent/api/gscloud";
const MAX_JSON: usize = 2 * 1024 * 1024;
pub mod auth;
pub(crate) mod original;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Product {
    pub id: String,
    pub title: String,
    pub series_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchRequest {
    pub product_id: String,
    pub bounds: [f64; 4],
    pub start: String,
    pub end: String,
    pub cloud_max: Option<f64>,
    #[serde(default = "first_page")]
    pub page: u32,
    #[serde(default = "page_size")]
    pub page_size: u32,
    #[serde(default = "all_presence")]
    pub presence: String,
}
fn first_page() -> u32 {
    1
}
fn page_size() -> u32 {
    20
}
fn all_presence() -> String {
    "all".into()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetailRequest {
    pub product_id: String,
    pub item_id: String,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 180
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-+().".contains(&c))
}
impl SearchRequest {
    pub fn validate(&self) -> Result<()> {
        let date = |s: &str| {
            s.len() == 10
                && NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok_and(|d| d.to_string() == s)
        };
        if !valid_id(&self.product_id)
            || !crate::projects::valid_bounds(self.bounds)
            || !date(&self.start)
            || !date(&self.end)
            || self.start > self.end
            || self.page == 0
            || self.page > 10000
            || !(1..=20).contains(&self.page_size)
            || !["all", "has", "none"].contains(&self.presence.as_str())
            || self
                .cloud_max
                .is_some_and(|c| !c.is_finite() || !(0.0..=100.0).contains(&c))
        {
            return Err(
                "Choose a GSCloud product, valid WGS84 area, dates and page (1–20 results).".into(),
            );
        }
        Ok(())
    }
    fn payload(&self, product: &Product) -> Value {
        let [w, s, e, n] = self.bounds;
        json!({"seriesId":product.series_id,"productId":self.product_id,"startDate":self.start,"endDate":self.end,
            "monthStep":null,"cloudMax":self.cloud_max,"dataPresence":self.presence,
            "aoi":{"type":"Polygon","coordinates":[[[w,s],[e,s],[e,n],[w,n],[w,s]]]},"page":self.page,"pageSize":self.page_size})
    }
}

fn products(value: &Value) -> Result<Vec<Product>> {
    fn visit(nodes: &[Value], series: &str, out: &mut Vec<Product>, depth: usize) -> Result<()> {
        if depth > 6 || nodes.len() > 500 || out.len() > 500 {
            return Err("Invalid GSCloud product directory.".into());
        }
        for node in nodes {
            let id = node["id"].as_str().ok_or("Invalid GSCloud product ID.")?;
            let name = node["name"]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() <= 500)
                .ok_or("Invalid GSCloud product name.")?;
            match node["kind"].as_str() {
                Some("product") if valid_id(id) && !series.is_empty() => out.push(Product {
                    id: id.into(),
                    title: name.into(),
                    series_id: series.into(),
                }),
                Some("series") => visit(
                    node["children"]
                        .as_array()
                        .ok_or("Invalid GSCloud product tree.")?,
                    id,
                    out,
                    depth + 1,
                )?,
                _ => return Err("Unsupported GSCloud product tree.".into()),
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    visit(
        value
            .as_array()
            .ok_or("Invalid GSCloud product directory.")?,
        "",
        &mut out,
        0,
    )?;
    let mut ids = BTreeSet::new();
    if out.is_empty() || out.len() > 500 || out.iter().any(|p| !ids.insert(&p.id)) {
        return Err("Invalid GSCloud product directory.".into());
    }
    Ok(out)
}
async fn document(request: reqwest::RequestBuilder) -> Result<(Value, String)> {
    let response = request
        .timeout(Duration::from_secs(45))
        .send()
        .await
        .map_err(|_| "GSCloud metadata connection failed. Check the source or proxy.")?;
    if !response.status().is_success() {
        return Err(format!(
            "GSCloud metadata returned HTTP {}.",
            response.status().as_u16()
        ));
    }
    if response
        .content_length()
        .is_some_and(|n| n > MAX_JSON as u64)
    {
        return Err("GSCloud metadata exceeds the size limit.".into());
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "GSCloud metadata transfer failed.")?;
        if bytes.len() + chunk.len() > MAX_JSON {
            return Err("GSCloud metadata exceeds the size limit.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let value = serde_json::from_slice(&bytes)
        .map_err(|_| "GSCloud did not return valid JSON metadata.")?;
    Ok((value, format!("{:x}", Sha256::digest(&bytes))))
}
fn string(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| value.as_i64().map(|n| n.to_string()))
}
fn item(value: &Value, product: &str) -> Result<Value> {
    let id = string(&value["dataid"])
        .or_else(|| string(&value["landsat_product_identifier_l2"]))
        .or_else(|| string(&value["keyid"]))
        .ok_or("GSCloud item has no identifier.")?;
    if !valid_id(&id) || string(&value["productid"]).is_some_and(|p| p != product) {
        return Err("GSCloud item does not match its product.".into());
    }
    let date = string(&value["datadate"])
        .filter(|s| s.len() >= 8)
        .and_then(|s| {
            let d = if s.len() == 8 {
                NaiveDate::parse_from_str(&s, "%Y%m%d")
            } else {
                NaiveDate::parse_from_str(s.get(..10)?, "%Y-%m-%d")
            };
            d.ok().map(|d| d.to_string())
        });
    let presence = match value["dataexists"]
        .as_i64()
        .or_else(|| value["dataexists"].as_str().and_then(|s| s.parse().ok()))
    {
        Some(1) => "available",
        Some(0) => "reservation",
        _ => "unknown",
    };
    let cloud = value
        .get("cloud_cover")
        .or_else(|| value.get("land_cloud_cover"))
        .and_then(Value::as_f64)
        .filter(|v| (0.0..=100.0).contains(v));
    Ok(
        json!({"id":id,"productId":product,"datetime":date.map(|d|format!("{d}T00:00:00Z")),"cloudCover":cloud,
        "presence":presence,"downloadStatus":if presence=="reservation"{"reservation-required"}else{"account-required"},
        "path":string(&value["path"]).or_else(||string(&value["wrs_path"])),"row":string(&value["row"]).or_else(||string(&value["wrs_row"])),
        "bytes":value["filesize"].as_f64().filter(|n|n.is_finite()&&*n>0.0),"geometry":null,"bbox":null}),
    )
}

pub(crate) fn agent_definitions() -> Vec<Value> {
    let id = json!({"type":"string","minLength":1,"maxLength":180});
    vec![
        json!({"name":"geod_gscloud_products","description":"Read the actual public GSCloud (地理空间数据云) product directory. When context.gscloud has productId, pass that exact productId; do not guess a name query or use geod_dataset_catalog/STAC. query is optional name text, never proof that a different ID is the selected product. Metadata only, not original entitlement or transfer.","inputSchema":{"type":"object","properties":{"productId":id,"query":{"type":"string","maxLength":100}},"additionalProperties":false}}),
        json!({"name":"geod_gscloud_search","description":"Search one actual public GSCloud page. Use the exact context.gscloud.productId when present; the search validates it against the live directory. Use geod_gscloud_products only when the product ID is unknown. Dates, area and data presence are explicit; elevation may need the historical observation period. Items have no footprint until geod_gscloud_detail. Never claim download/coverage completion from metadata. Desktop controls can download verified public terrain ZIPs (305 SRTM90, 307 slope, 308 TPI, 310 ASTER V1, 421 ASTER V2, aeab8000652a45b38afbb7ff023ddabb ASTER V3); this search returns a session-bound searchId for geod_gscloud_download_plan and native confirmation, never a download itself. Other products and reservations require separate delivery verification. No credential or order mutation.","inputSchema":{"type":"object","properties":{"productId":id,"bounds":{"type":"array","items":{"type":"number"},"minItems":4,"maxItems":4},"start":{"type":"string"},"end":{"type":"string"},"cloudMax":{"type":["number","null"],"minimum":0,"maximum":100},"presence":{"type":"string","enum":["all","has","none"]},"page":{"type":"integer","minimum":1,"maximum":10000},"pageSize":{"type":"integer","minimum":1,"maximum":20}},"required":["productId","bounds","start","end"],"additionalProperties":false}}),
        json!({"name":"geod_gscloud_detail","description":"Read real GSCloud item metadata, declared file size and source footprint for a productId/itemId returned by GSCloud search. Missing geometry remains unknown. Returns metadata SHA-256, not an original-file hash. No download or purchase.","inputSchema":{"type":"object","properties":{"productId":id,"itemId":id},"required":["productId","itemId"],"additionalProperties":false}}),
    ]
}
pub(crate) async fn agent_call(manager: &JobManager, name: &str, args: Value) -> Result<Value> {
    match name {
        "geod_gscloud_products" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase", deny_unknown_fields)]
            struct Filter {
                query: Option<String>,
                product_id: Option<String>,
            }
            let filter: Filter =
                serde_json::from_value(args).map_err(|_| "Invalid GSCloud directory arguments.")?;
            let query = filter.query.unwrap_or_default();
            if query.len() > 100 || filter.product_id.as_ref().is_some_and(|id| !valid_id(id)) {
                return Err("GSCloud product query is too long.".into());
            }
            let mut value = manager.gscloud_products().await?;
            filter_agent_products(&mut value, filter.product_id.as_deref(), &query);
            Ok(value)
        }
        "geod_gscloud_search" => {
            manager
                .gscloud_search(
                    serde_json::from_value(args)
                        .map_err(|_| "Invalid GSCloud search arguments.")?,
                )
                .await
        }
        "geod_gscloud_detail" => {
            manager
                .gscloud_detail(
                    serde_json::from_value(args)
                        .map_err(|_| "Invalid GSCloud detail arguments.")?,
                )
                .await
        }
        _ => Err("Unknown GSCloud tool.".into()),
    }
}
fn filter_agent_products(value: &mut Value, product: Option<&str>, query: &str) {
    let total = value["products"].as_array().map_or(0, Vec::len);
    value["products"].as_array_mut().unwrap().retain(|p| {
        if product.is_some_and(|id| p["id"] != id) {
            return false;
        }
        query.is_empty()
            || format!("{} {} {}", p["id"], p["title"], p["seriesId"])
                .to_lowercase()
                .contains(&query.to_lowercase())
    });
    value["directoryTotal"] = json!(total);
    value["matchedProducts"] = json!(value["products"].as_array().unwrap().len());
    value["appliedFilter"] = json!({"productId":product,"query":query});
}
fn detail(value: &Value, request: &DetailRequest) -> Result<Value> {
    let mut normalized = item(value, &request.product_id)?;
    if normalized["id"] != request.item_id {
        return Err("GSCloud detail does not match the requested item.".into());
    }
    // Use the provider's real polygon; never enlarge a centre point to a footprint.
    let geometry = value
        .get("st_asgeojson(the_geom)")
        .and_then(Value::as_str)
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .or_else(|| value.get("geometry").cloned())
        .or_else(|| value.get("the_geom").filter(|v| v.is_object()).cloned());
    if let Some(geometry) = geometry {
        if let Ok(parsed) = serde_json::from_value::<crate::crop::PolygonGeometry>(geometry.clone())
        {
            if let Ok(bounds) = parsed.bounds() {
                normalized["geometry"] = geometry;
                normalized["bbox"] = json!(bounds);
            }
        }
    }
    Ok(normalized)
}
impl JobManager {
    pub async fn gscloud_products(&self) -> Result<Value> {
        let client = crate::proxy::download_client(&self.proxy_settings().await)?;
        let (value, sha) = document(client.get(format!("{BASE}/datasets"))).await?;
        Ok(
            json!({"products":products(&value)?,"retrievedAt":crate::now(),"metadataSha256":sha,"source":"https://www.gscloud.cn/geoagent","downloadsStarted":false,"downloadCapability":"official-account-required"}),
        )
    }
    pub async fn gscloud_search(&self, request: SearchRequest) -> Result<Value> {
        request.validate()?;
        let catalog = self.gscloud_products().await?;
        let product: Product = serde_json::from_value(
            catalog["products"]
                .as_array()
                .and_then(|v| v.iter().find(|p| p["id"] == request.product_id))
                .cloned()
                .ok_or("This product is not in the current GSCloud directory.")?,
        )
        .map_err(crate::io_error)?;
        let client = crate::proxy::download_client(&self.proxy_settings().await)?;
        let (value, sha) = document(
            client
                .post(format!("{BASE}/search"))
                .header("Content-Type", "application/json")
                .body(serde_json::to_vec(&request.payload(&product)).map_err(crate::io_error)?),
        )
        .await?;
        let items = value["items"]
            .as_array()
            .filter(|v| v.len() <= request.page_size as usize)
            .ok_or("Invalid GSCloud result page.")?;
        let total = value["totalSize"]
            .as_u64()
            .ok_or("Invalid GSCloud total count.")?;
        let pages = value["totalPage"]
            .as_u64()
            .ok_or("Invalid GSCloud page count.")?;
        if value["page"] != request.page
            || value["pageSize"] != request.page_size
            || total < items.len() as u64
        {
            return Err("GSCloud returned a different result page.".into());
        }
        let mut seen = BTreeSet::new();
        let items = items
            .iter()
            .map(|v| item(v, &request.product_id))
            .collect::<Result<Vec<_>>>()?;
        if items
            .iter()
            .any(|v| !seen.insert(v["id"].as_str().unwrap_or_default()))
        {
            return Err("GSCloud returned duplicate item identifiers.".into());
        }
        Ok(
            json!({"request":request,"product":product,"items":items,"total":total,"totalPages":pages,
            "hasMore":u64::from(request.page)<pages,"retrievedAt":crate::now(),"metadataSha256":sha,"downloadsStarted":false,"access":original::download_access(&request.product_id)}),
        )
    }
    pub async fn gscloud_detail(&self, request: DetailRequest) -> Result<Value> {
        if !valid_id(&request.product_id) || !valid_id(&request.item_id) {
            return Err("Invalid GSCloud item identifier.".into());
        }
        let client = crate::proxy::download_client(&self.proxy_settings().await)?;
        let (value, sha) = document(
            client
                .get(format!("{BASE}/detail"))
                .query(&[("pid", &request.product_id), ("dataid", &request.item_id)]),
        )
        .await?;
        Ok(
            json!({"item":detail(&value,&request)?,"retrievedAt":crate::now(),"metadataSha256":sha,"downloadsStarted":false,"access":original::download_access(&request.product_id)}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_scope_and_credentials() {
        let r = SearchRequest {
            product_id: "305".into(),
            bounds: [116.2, 39.8, 116.5, 40.1],
            start: "2000-01-01".into(),
            end: "2025-10-15".into(),
            cloud_max: None,
            page: 1,
            page_size: 20,
            presence: "all".into(),
        };
        assert!(r.validate().is_ok());
        let mut bad = r.clone();
        bad.bounds = [180.0, 0.0, -180.0, 1.0];
        assert!(bad.validate().is_err());
        bad = r.clone();
        bad.start = "2025-02-30".into();
        assert!(bad.validate().is_err());
        bad = r.clone();
        bad.page_size = 21;
        assert!(bad.validate().is_err());
        let mut v = serde_json::to_value(r).unwrap();
        v["password"] = json!("never");
        assert!(serde_json::from_value::<SearchRequest>(v).is_err());
        assert!(!valid_id("../accounts"));
        assert!(!valid_id("x?token=secret"));
    }
    #[test]
    fn keeps_reservation_unknown_and_footprints_honest() {
        let v = json!({"dataid":"tile","productid":"305","dataexists":0,"ct_long":116,"ct_lat":40,"datadate":20000201});
        let r = DetailRequest {
            product_id: "305".into(),
            item_id: "tile".into(),
        };
        let d = detail(&v, &r).unwrap();
        assert_eq!(d["downloadStatus"], "reservation-required");
        assert!(d["bbox"].is_null());
        assert!(d["geometry"].is_null());
        let mut v = v;
        v["dataexists"] = Value::Null;
        assert_eq!(item(&v, "305").unwrap()["presence"], "unknown");
        v["dataid"] = json!("other");
        assert!(detail(&v, &r).is_err());
    }
    #[test]
    fn agent_product_identity_is_exact_and_name_filter_does_not_substitute_ids() {
        let directory = json!({"products":[{"id":"310","title":"ASTER GDEM 30M","seriesId":"DEM"},{"id":"421","title":"GDEMV2 30M","seriesId":"DEM"},{"id":"305","title":"SRTMDEM 90M","seriesId":"DEM"}]});
        let mut exact = directory.clone();
        filter_agent_products(&mut exact, Some("421"), "");
        assert_eq!(exact["products"].as_array().unwrap().len(), 1);
        assert_eq!(exact["products"][0]["id"], "421");
        assert_eq!(exact["directoryTotal"], 3);
        assert_eq!(exact["matchedProducts"], 1);
        assert_eq!(exact["appliedFilter"]["productId"], "421");
        let mut missing = directory.clone();
        filter_agent_products(&mut missing, Some("missing"), "");
        assert_eq!(missing["matchedProducts"], 0);
        let mut incompatible = directory;
        filter_agent_products(&mut incompatible, Some("421"), "ASTER");
        assert_eq!(incompatible["matchedProducts"], 0);
        assert_eq!(original::desktop_product_ids().len(), 6);
        assert_eq!(
            original::download_access("421")["desktopOriginalDownload"],
            true
        );
        assert_eq!(
            original::download_access("309")["desktopOriginalDownload"],
            false
        );
        assert_eq!(original::download_access("421")["agentDownload"], true);
        assert_eq!(original::download_access("309")["agentDownload"], false);
    }
    #[test]
    fn source_context_is_typed_and_account_operations_are_not_model_tools() {
        let request = json!({"productId":"305","bounds":[116.2,39.8,116.5,40.1],"start":"1980-01-01","end":"2026-10-15","cloudMax":null,"presence":"all","page":1,"pageSize":20});
        let context = json!({"page":"Explore","provider":"gscloud","bounds":[116.2,39.8,116.5,40.1],"start":"1980-01-01","end":"2026-10-15","cloudMin":0,"cloudMax":100,"projectId":null,"geometry":null,"catalog":null,"gscloud":request});
        let parsed =
            serde_json::from_value::<crate::agent_actions::MapContext>(context.clone()).unwrap();
        assert!(parsed.validate().is_ok());
        let mut bad = context;
        bad["provider"] = json!("earth-search");
        assert!(
            serde_json::from_value::<crate::agent_actions::MapContext>(bad)
                .unwrap()
                .validate()
                .is_err()
        );
        assert!(agent_definitions()
            .iter()
            .all(|v| !v["name"].as_str().unwrap().contains("login")));
    }
    #[tokio::test]
    #[ignore = "real GSCloud public metadata requests"]
    async fn live_gscloud_landsat_and_dem() {
        let temp = tempfile::tempdir().unwrap();
        let manager = JobManager::open(temp.path()).await.unwrap();
        for (product, start, end) in [
            (
                "2c79f880a0304edebac6c48592d8e088",
                "2025-09-01",
                "2025-10-15",
            ),
            ("305", "1980-01-01", "2026-10-15"),
        ] {
            let request = SearchRequest {
                product_id: product.into(),
                bounds: [116.2, 39.8, 116.5, 40.1],
                start: start.into(),
                end: end.into(),
                cloud_max: None,
                page: 1,
                page_size: 3,
                presence: "all".into(),
            };
            let result = crate::agent_actions::call(
                manager.clone(),
                "a1234567-1234-1234-1234-123456789abc",
                "geod_gscloud_search",
                serde_json::to_value(request).unwrap(),
                None,
            )
            .await
            .unwrap();
            assert!(!result["items"].as_array().unwrap().is_empty());
            let id = result["items"][0]["id"].as_str().unwrap();
            let detail = manager
                .gscloud_detail(DetailRequest {
                    product_id: product.into(),
                    item_id: id.into(),
                })
                .await
                .unwrap();
            assert!(!detail["item"]["geometry"].is_null());
            assert!(!detail["item"]["bbox"].is_null());
            println!(
                "product={product} total={} item={id} geometry={} download={}",
                result["total"],
                detail["item"]["geometry"]["type"],
                detail["item"]["downloadStatus"]
            );
        }
    }
}
