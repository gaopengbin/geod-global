use crate::lifecycle::DesktopLifecycle;
use geod_runtime::{
    vector::{
        official_packages::{self, DownloadRequest, SearchRequest},
        VectorAsset,
    },
    JobManager,
};
use tauri::State;

#[tauri::command]
pub fn list_official_vector_packages(
    lifecycle: State<'_, DesktopLifecycle>,
) -> Result<serde_json::Value, String> {
    let _command = lifecycle.enter()?;
    Ok(official_packages::providers())
}
#[tauri::command]
pub fn search_official_vector_packages(
    lifecycle: State<'_, DesktopLifecycle>,
    request: SearchRequest,
) -> Result<serde_json::Value, String> {
    let _command = lifecycle.enter()?;
    official_packages::search(request)
}
#[tauri::command]
pub async fn download_official_vector_package(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: DownloadRequest,
) -> Result<VectorAsset, String> {
    let _command = lifecycle.enter()?;
    manager.download_official_vector_package(request).await
}

#[tauri::command]
pub fn list_official_parquet(
    lifecycle: State<'_, DesktopLifecycle>,
) -> Result<serde_json::Value, String> {
    let _command = lifecycle.enter()?;
    Ok(geod_runtime::vector::parquet_original::providers())
}
#[tauri::command]
pub fn search_official_parquet(
    lifecycle: State<'_, DesktopLifecycle>,
    request: geod_runtime::vector::parquet_original::Request,
) -> Result<serde_json::Value, String> {
    let _command = lifecycle.enter()?;
    geod_runtime::vector::parquet_original::search(request)
}
#[tauri::command]
pub async fn download_official_parquet(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: geod_runtime::vector::parquet_original::Request,
) -> Result<VectorAsset, String> {
    let _command = lifecycle.enter()?;
    manager.download_official_parquet(request).await
}
#[tauri::command]
pub async fn parquet_original_metadata(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
) -> Result<serde_json::Value, String> {
    let _command = lifecycle.enter()?;
    manager.parquet_original_metadata(&id).await
}
#[tauri::command]
pub async fn parquet_original_rows(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
    args: geod_runtime::vector::parquet_original::Rows,
) -> Result<serde_json::Value, String> {
    let _command = lifecycle.enter()?;
    manager.parquet_original_rows(&id, args).await
}
