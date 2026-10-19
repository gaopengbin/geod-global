use crate::lifecycle::DesktopLifecycle;
use geod_runtime::{
    gebco::{Catalog, PixelRequest, Subset, SubsetRequest, SubsetResult},
    JobManager,
};
use tauri::State;
#[tauri::command]
pub async fn gebco_grids(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
) -> Result<Catalog, String> {
    let _command = lifecycle.enter()?;
    manager.gebco_grids().await
}
#[tauri::command]
pub async fn gebco_list(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
) -> Result<Vec<Subset>, String> {
    let _command = lifecycle.enter()?;
    manager.gebco_list().await
}
#[tauri::command]
pub async fn gebco_get(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
) -> Result<Subset, String> {
    let _command = lifecycle.enter()?;
    manager.gebco_get(&id).await
}
#[tauri::command]
pub async fn gebco_create_subset(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: SubsetRequest,
) -> Result<SubsetResult, String> {
    let _command = lifecycle.enter()?;
    manager.gebco_create_subset(request).await
}
#[tauri::command]
pub async fn gebco_inspect(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
) -> Result<geod_runtime::stac::GenericRasterInspection, String> {
    let _command = lifecycle.enter()?;
    manager.gebco_inspect(&id).await
}
#[tauri::command]
pub async fn gebco_pixel(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: PixelRequest,
) -> Result<geod_runtime::stac::GenericRasterPixel, String> {
    let _command = lifecycle.enter()?;
    manager.gebco_pixel(request).await
}
async fn save(
    manager: &JobManager,
    window: tauri::WebviewWindow,
    id: String,
    archive: bool,
) -> Result<bool, String> {
    let bytes = if archive {
        manager.gebco_archive(&id).await?
    } else {
        manager.gebco_source(&id).await?
    };
    #[cfg(windows)]
    {
        let path = tauri::async_runtime::spawn_blocking(move || {
            rfd::FileDialog::new()
                .set_parent(&window)
                .add_filter(
                    if archive { "ZIP" } else { "GeoTIFF" },
                    if archive { &["zip"] } else { &["tif"] },
                )
                .set_file_name(if archive {
                    "gebco-2026-official-subset.zip"
                } else {
                    "gebco-2026-numerical-subset.tif"
                })
                .save_file()
        })
        .await
        .map_err(|_| "Could not open the save dialog")?;
        if let Some(path) = path {
            tokio::fs::write(path, bytes)
                .await
                .map_err(|_| "Could not save the verified GEBCO subset")?;
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
#[tauri::command]
pub async fn gebco_export_source(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    window: tauri::WebviewWindow,
    id: String,
) -> Result<bool, String> {
    let _command = lifecycle.enter()?;
    save(&manager, window, id, false).await
}
#[tauri::command]
pub async fn gebco_export_archive(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    window: tauri::WebviewWindow,
    id: String,
) -> Result<bool, String> {
    let _command = lifecycle.enter()?;
    save(&manager, window, id, true).await
}
