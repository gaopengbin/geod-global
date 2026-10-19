use crate::lifecycle::DesktopLifecycle;
use geod_runtime::{
    earth_engine::{DownloadRequest, DownloadResult, Export},
    JobManager,
};
use tauri::State;

#[tauri::command]
pub async fn list_earth_engine_exports(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
) -> Result<Vec<Export>, String> {
    let _command = lifecycle.enter()?;
    manager.list_earth_engine_exports().await
}
#[tauri::command]
pub async fn download_earth_engine_export(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: DownloadRequest,
) -> Result<DownloadResult, String> {
    let _command = lifecycle.enter()?;
    manager.download_earth_engine_export(request).await
}
#[tauri::command]
pub async fn inspect_earth_engine_export(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
) -> Result<geod_runtime::stac::GenericRasterInspection, String> {
    let _command = lifecycle.enter()?;
    manager.inspect_earth_engine_export(&id).await
}
#[tauri::command]
pub async fn export_earth_engine_file(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    window: tauri::WebviewWindow,
    id: String,
) -> Result<bool, String> {
    let _command = lifecycle.enter()?;
    let bytes = manager.earth_engine_file(&id).await?;
    #[cfg(windows)]
    {
        let path = tauri::async_runtime::spawn_blocking(move || {
            rfd::FileDialog::new()
                .set_parent(&window)
                .add_filter("GeoTIFF", &["tif"])
                .set_file_name("gee-export.tif")
                .save_file()
        })
        .await
        .map_err(|_| "Could not open the save dialog")?;
        if let Some(path) = path {
            tokio::fs::write(path, bytes)
                .await
                .map_err(|_| "Could not save the GEE export")?;
            return Ok(true);
        }
        Ok(false)
    }
    #[cfg(not(windows))]
    {
        let _ = (window, bytes);
        Err("Native save dialog is unavailable on this platform".into())
    }
}
