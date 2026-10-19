//! Real publisher download and native reopen acceptance in an explicit isolated
//! storage directory. Does not open windows or start a network listener.
use geod_runtime::{
    vector::official_packages::{self, DownloadRequest, SearchRequest},
    JobManager, ProxySettings,
};
use serde_json::json;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let root = PathBuf::from(
        args.next()
            .ok_or("explicit isolated storage directory required")?,
    );
    let proxy = args.next();
    let output = args.next().map(PathBuf::from);
    let manager = JobManager::open(&root).await?;
    if let Some(url) = proxy {
        let settings = if url == "direct" {
            ProxySettings {
                mode: geod_runtime::proxy::ProxyMode::Direct,
                url: None,
            }
        } else {
            ProxySettings {
                mode: geod_runtime::proxy::ProxyMode::Custom,
                url: Some(url),
            }
        };
        manager.save_proxy_settings(settings).await?;
    }
    let mut inspections = Vec::new();
    for (dataset, package_id, bounds) in [
        (
            "ms-buildings",
            "Andorra-120222210",
            [1.50, 42.57, 1.53, 42.60],
        ),
        (
            "geofabrik",
            "monaco-261011-free-shp",
            [7.41, 43.72, 7.43, 43.74],
        ),
    ] {
        let search = official_packages::search(SearchRequest {
            dataset_id: dataset.into(),
            bounds,
        })?;
        if !search["items"]
            .as_array()
            .ok_or("search did not return packages")?
            .iter()
            .any(|p| p["id"] == package_id)
        {
            return Err(
                "actual area discovery did not return the selected publisher original".into(),
            );
        }
        let asset = manager
            .download_official_vector_package(DownloadRequest {
                dataset_id: dataset.into(),
                package_id: package_id.into(),
                bounds,
            })
            .await?;
        let inspection = manager.inspect_vector(&asset.id).await?;
        inspections.push(inspection);
    }
    drop(manager);
    let manager = JobManager::open(&root).await?;
    let mut receipts = Vec::new();
    for before in inspections {
        let after = manager.inspect_vector(&before.asset.id).await?;
        if after.asset != before.asset || after.geojson != before.geojson {
            return Err(
                "downloaded original, native geometry or properties changed on reopen".into(),
            );
        }
        receipts.push(
            json!({"asset":after.asset,"reopenedEveryGeometryAndProperty":true,
            "completeOriginalDownloadedFromReviewedPublisher":true,
            "firstGeometryType":after.geojson["features"][0]["geometry"]["type"],
            "firstProperties":after.geojson["features"][0]["properties"]}),
        );
    }
    let report =
        serde_json::to_string_pretty(&json!({"receipts":receipts,"noListenerStarted":true}))?;
    if let Some(output) = output {
        std::fs::write(output, &report)?;
    }
    println!("{report}");
    Ok(())
}
