//! Model requests can open only an inspected managed asset, never a path/URL.
use geod_runtime::{agent_actions, JobManager};
use serde::Deserialize;
use serde_json::{json, Value};
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    kind: String,
    id: String,
}
pub async fn open(manager: JobManager, session: &str, args: Value) -> Result<Value, String> {
    let request: Request = serde_json::from_value(args)
        .map_err(|_| "Choose a managed raster or vector file to open.")?;
    let name = match request.kind.as_str() {
        "raster" => "geod_raster_inspect",
        "vector" => "geod_vector_inspect",
        _ => return Err("Choose a managed raster or vector file to open.".into()),
    };
    // Inspection verifies actual file availability and the persisted checksum.
    // Workspace rendering performs its own native read as well.
    agent_actions::call(manager, session, name, json!({"id":request.id}), None).await?;
    Ok(
        json!({"workspaceView":{"requestId":super::registry::new_id(),"kind":request.kind,"id":request.id,"verified":true,"acknowledged":false}}),
    )
}
pub fn definition() -> Value {
    json!({"name":"geod_workspace_open","description":"Open an actual completed managed raster job or verified vector file in the application's map workspace when the human asks to view the result. Native inspection checks the file first. This requests UI navigation, not an export or proof that the rendered pixels were observed. No file paths, URLs, downloads, browser automation or external programs are accepted.","inputSchema":{"type":"object","properties":{"kind":{"type":"string","enum":["raster","vector"]},"id":{"type":"string"}},"required":["kind","id"],"additionalProperties":false}})
}
