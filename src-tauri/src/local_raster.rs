use crate::lifecycle::DesktopLifecycle;
use geod_runtime::{
    local_raster::{ImportedRaster, LocalRaster, MAX_BYTES},
    JobManager,
};
use tauri::State;

#[tauri::command]
pub async fn list_local_rasters(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
) -> Result<Vec<LocalRaster>, String> {
    let _command = lifecycle.enter()?;
    manager.list_local_rasters().await
}

#[tauri::command]
pub async fn open_local_raster(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    window: tauri::WebviewWindow,
) -> Result<Option<ImportedRaster>, String> {
    let _command = lifecycle.enter()?;
    #[cfg(windows)]
    {
        let selected = tauri::async_runtime::spawn_blocking(move || {
            rfd::FileDialog::new()
                .set_parent(&window)
                .add_filter("GeoTIFF or COG", &["tif", "tiff"])
                .pick_file()
        })
        .await
        .map_err(|e| e.to_string())?;
        let Some(path) = selected else {
            return Ok(None);
        };
        let (name, bytes) = tauri::async_runtime::spawn_blocking(move || {
            use std::io::Read;
            let name = path
                .file_name()
                .and_then(|v| v.to_str())
                .ok_or("The file name must be valid text")?
                .to_owned();
            let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
            let metadata = file.metadata().map_err(|e| e.to_string())?;
            if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_BYTES as u64 {
                return Err("Choose a GeoTIFF or COG file up to 32 MiB".to_owned());
            }
            let mut bytes = Vec::with_capacity(metadata.len() as usize);
            file.take(MAX_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() as u64 != metadata.len() {
                return Err("The selected file changed while it was read".to_owned());
            }
            Ok::<_, String>((name, bytes))
        })
        .await
        .map_err(|e| e.to_string())??;
        manager.import_local_raster(name, bytes).await.map(Some)
    }
    #[cfg(not(windows))]
    {
        let _ = (manager, window);
        Err("Native raster import is not available on this platform".into())
    }
}

#[tauri::command]
pub async fn inspect_local_raster(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
) -> Result<geod_runtime::stac::GenericRasterInspection, String> {
    let _command = lifecycle.enter()?;
    manager.inspect_local_raster(&id).await
}

#[tauri::command]
pub async fn sample_local_raster(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
    column: u32,
    row: u32,
) -> Result<geod_runtime::stac::GenericRasterPixel, String> {
    let _command = lifecycle.enter()?;
    manager.sample_local_raster(&id, column, row).await
}

#[tauri::command]
pub async fn export_local_raster(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
    window: tauri::WebviewWindow,
) -> Result<bool, String> {
    let _command = lifecycle.enter()?;
    #[cfg(windows)]
    {
        let asset = manager
            .list_local_rasters()
            .await?
            .into_iter()
            .find(|v| v.id == id)
            .ok_or("Unknown local raster")?;
        let name = asset.name.clone();
        let selected = tauri::async_runtime::spawn_blocking(move || {
            rfd::FileDialog::new()
                .set_parent(&window)
                .add_filter("Original GeoTIFF", &["tif", "tiff"])
                .set_file_name(name)
                .save_file()
        })
        .await
        .map_err(|e| e.to_string())?;
        let Some(path) = selected else {
            return Ok(false);
        };
        // Verify the managed original after the dialog, immediately before copying it.
        let bytes = manager.local_raster_file(&id).await?;
        tauri::async_runtime::spawn_blocking(move || {
            use std::io::Write;
            let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;
            file.write_all(&bytes).map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())??;
        Ok(true)
    }
    #[cfg(not(windows))]
    {
        let _ = (manager, id, window);
        Err("Native raster export is not available on this platform".into())
    }
}
