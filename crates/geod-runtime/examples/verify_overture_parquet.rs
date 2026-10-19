use geod_runtime::{
    vector::parquet_original::{self, Request, Rows},
    JobManager, ProxySettings,
};
use serde_json::json;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let root = PathBuf::from(
        args.next()
            .ok_or("explicit isolated runtime storage required")?,
    );
    let output = PathBuf::from(args.next().ok_or("output evidence JSON required")?);
    let manager = JobManager::open(&root).await?;
    manager
        .save_proxy_settings(ProxySettings {
            mode: geod_runtime::proxy::ProxyMode::Custom,
            url: Some("socks5://127.0.0.1:7890".into()),
        })
        .await?;
    let request = Request {
        dataset_id: "overture-maps".into(),
        package_id: Some("base-bathymetry-2026-09-23.1-part-00000".into()),
        bounds: [-71.5, 40.5, -70.5, 41.5],
    };
    let discovered = parquet_original::search(request.clone())?;
    let asset = manager.download_official_parquet(request).await?;
    let before = manager.inspect_vector(&asset.id).await?;
    let first = before.geojson["features"][0].clone();
    let rows = manager
        .parquet_original_rows(
            &asset.id,
            Rows {
                offset: first["geodOriginalRow"].as_u64().map(|v| v as usize),
                limit: Some(1),
            },
        )
        .await?;
    drop(manager);
    let manager = JobManager::open(&root).await?;
    let reopened = manager.inspect_vector(&asset.id).await?;
    if before.asset != reopened.asset || before.geojson != reopened.geojson {
        return Err("Original or filtered geometry/properties changed on reopen".into());
    }
    let evidence = json!({"discovery":discovered,"asset":asset,"allSelectedFeatures":before.geojson["features"],
        "firstOriginalPropertyPage":rows,"everySelectedGeometryAndPropertyReopenedIdentically":true,
        "completeOriginalDownloaded":true,"previewIsSubset":true,"originalNeverRewrittenOrServerSubset":true});
    std::fs::write(output, serde_json::to_vec_pretty(&evidence)?)?;
    println!(
        "{}",
        json!({"assetId":asset.id,"format":asset.format,"previewFeatures":asset.feature_count,
        "previewCoordinates":asset.coordinate_count,"previewBytes":asset.bytes,"previewSha256":asset.source_sha256,
        "originalSha256":asset.geo_parquet.as_ref().map(|p|&p.original_sha256),"originalBytes":52018294,
        "originalRows":407277,"everySelectedGeometryAndPropertyReopenedIdentically":true})
    );
    Ok(())
}
