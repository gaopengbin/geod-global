//! Draft preparation and read-only access only; models cannot pay or authorize monitoring.
use crate::identity::DesktopIdentity;
use geod_runtime::{
    agent_actions::{self, MapContext},
    JobManager,
};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn definitions() -> Vec<Value> {
    let read = |name: &str, description: &str| json!({"name":name,"description":description,"inputSchema":{"type":"object","properties":{},"additionalProperties":false}});
    vec![
        read("geod_official_services", "Read actual official aggregation capability and current account service entitlement. Never infer Pro from client settings. Official service availability is separate from local tools. No payment or execution."),
        read("geod_monitor_list", "Read this signed-in account's real monitors and recent catalog results. Monitoring results are not downloaded files, full coverage or regional cloud proof."),
        read("geod_monitor_notifications", "Read this signed-in account's deduplicated new-imagery notifications. Never claim an alert means imagery has been downloaded."),
        json!({"name":"geod_monitor_quote","description":"Prepare a complete, priced official monitoring review for an explicit human request to monitor an area. Use an actual geod_boundary_read boundary reference, or useAttachedPolygon=true for the actual attached area. Never invent geometry. Requires server-side Pro. Sends the polygon to the official service. A review card appears in this conversation and Imagery monitoring for human confirmation; this tool cannot accept, start, pay or download.","inputSchema":{"type":"object","properties":{
            "name":{"type":"string"},"sourceId":{"type":"string","enum":["earth-search","planetary-landsat"]},
            "boundary":{"type":"object","properties":{"id":{"type":"string"},"sha256":{"type":"string"}},"required":["id","sha256"],"additionalProperties":false},
            "useAttachedPolygon":{"type":"boolean"},"cloudMax":{"type":"integer","minimum":0,"maximum":100},
            "lookbackDays":{"type":"integer","minimum":1,"maximum":90},"intervalHours":{"type":"integer","minimum":1,"maximum":168},
            "dailyCreditLimit":{"type":"integer","minimum":1,"maximum":100},"totalCreditLimit":{"type":"integer","minimum":1,"maximum":10000}
        },"required":["name","sourceId","cloudMax","lookbackDays","intervalHours","dailyCreditLimit","totalCreditLimit"],"additionalProperties":false}}),
    ]
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Quote {
    name: String,
    source_id: String,
    cloud_max: u32,
    lookback_days: u32,
    interval_hours: u32,
    daily_credit_limit: u32,
    total_credit_limit: u32,
    boundary: Option<Value>,
    #[serde(default)]
    use_attached_polygon: bool,
}

pub async fn call(
    identity: Option<DesktopIdentity>,
    manager: JobManager,
    session: &str,
    name: &str,
    args: Value,
    context: Option<MapContext>,
    read_only: bool,
) -> Result<Value, String> {
    let identity = identity.ok_or("Official services are unavailable in this runtime.")?;
    let mut result = match name {
        "geod_official_services" if args == json!({}) => {
            let capabilities = identity.official("capabilities", json!({})).await?;
            let account = identity.official("account", json!({})).await?;
            json!({"capabilities":capabilities,"account":account.get("account").map(|value|json!({"plan":value["plan"],"proExpiresAt":value["proExpiresAt"],"mode":value["mode"],"buckets":value["buckets"]})),"accountError":account.get("error")})
        }
        "geod_monitor_list" if args == json!({}) => identity.official("monitors", args).await?,
        "geod_monitor_notifications" if args == json!({}) => {
            identity.official("notifications", args).await?
        }
        "geod_monitor_quote" if !read_only => {
            let quote: Quote =
                serde_json::from_value(args).map_err(|_| "Invalid monitoring review arguments.")?;
            let geometry = agent_actions::official_monitor_geometry(
                &manager,
                session,
                context,
                quote.use_attached_polygon,
                quote.boundary,
            )
            .await?;
            identity.official("quoteMonitor",json!({"name":quote.name,"sourceId":quote.source_id,"geometry":geometry,"cloudMax":quote.cloud_max,"lookbackDays":quote.lookback_days,"intervalHours":quote.interval_hours,"dailyCreditLimit":quote.daily_credit_limit,"totalCreditLimit":quote.total_credit_limit,"conversationId":session})).await?
        }
        _ => return Err("This official service operation is not permitted.".into()),
    };
    if name == "geod_monitor_quote" && result.get("quote").is_some() {
        result["reviewPage"] = json!("Imagery monitoring");
        result["requiresHumanConfirmation"] = json!(true);
        result["downloadsStarted"] = json!(false);
    }
    // Polygon coordinates stay outside model history. The official UI reads the server review.
    fn strip(value: &mut Value) {
        match value {
            Value::Object(map) => {
                map.remove("geometry");
                for child in map.values_mut() {
                    strip(child);
                }
            }
            Value::Array(items) => {
                for item in items {
                    strip(item);
                }
            }
            _ => {}
        }
    }
    strip(&mut result);
    if result.to_string().len() > 32768 {
        return Err("Official service result exceeds the conversation limit. Open Imagery monitoring for the full review.".into());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_tool_set_has_no_execution_payment_or_account_mutation() {
        let tools = definitions();
        assert_eq!(tools.len(), 4);
        assert!(tools
            .iter()
            .all(|v| !v["name"].as_str().unwrap().contains("accept")
                && !v["name"].as_str().unwrap().contains("checkout")));
        assert!(serde_json::from_value::<Quote>(json!({"name":"fake","sourceId":"earth-search","cloudMax":10,"lookbackDays":30,"intervalHours":24,"dailyCreditLimit":5,"totalCreditLimit":100,"geometry":{}})).is_err());
    }
}
