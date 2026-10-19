//! Explicit, local acceptance harness; no network or production storage access.
use geod_runtime::JobManager;
use serde_json::json;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let input =
        PathBuf::from(args.next().ok_or("original vector path required")?).canonicalize()?;
    let root = PathBuf::from(args.next().ok_or("isolated acceptance storage required")?);
    let manager = JobManager::open(&root).await?;
    let asset = manager.open_vector_path(input, true).await?;
    let first = manager.inspect_vector(&asset.id).await?;
    let id = asset.id.clone();
    drop(manager);
    let manager = JobManager::open(&root).await?;
    let reopened = manager.inspect_vector(&id).await?;
    if reopened.asset != first.asset || reopened.geojson != first.geojson {
        return Err("original hash, geometry or attributes changed on reopen".into());
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({"asset":asset,"reopenedVerified":true,
        "firstGeometryType":first.geojson["features"][0]["geometry"]["type"],
        "firstProperties":first.geojson["features"][0]["properties"]}))?
    );
    Ok(())
}
