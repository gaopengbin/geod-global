#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod elevation_preview;
mod lifecycle;

use lifecycle::DesktopLifecycle;

use geod_runtime::{
    AccountProvider, AccountStatus, AddProjectScenesRequest, ConnectAccountRequest,
    CreateJobRequest, CreateProjectRequest, Job, JobManager, Project, ProjectDownloads,
    ProxySettings, ProxyTest, RasterInspection, RasterPixel, RasterRecipe, RecipePlan,
    RuntimeHealth, SavedRecipe,
};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, RunEvent, State, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_decoration::WebviewWindowExt;
use tauri_plugin_opener::OpenerExt;

#[derive(Clone, Default)]
struct DesktopFrameReady(Arc<AtomicBool>);

async fn restore_desktop_frame(window: &tauri::WebviewWindow) -> Result<&'static str, String> {
    // A minimized WebView retains its old viewport while the native client
    // rectangle is empty. Leave it alone until the user restores the window.
    if window.is_minimized().map_err(|error| error.to_string())? {
        return Ok("deferred");
    }
    let restore = window.restore_decoration().await.err();
    let show = window.show().err();
    if restore.is_some() || show.is_some() {
        return Err(format!(
            "Desktop frame fallback: restore={restore:?}, reveal={show:?}"
        ));
    }
    Ok("native")
}

#[tauri::command]
async fn activate_desktop_frame(
    lifecycle: State<'_, DesktopLifecycle>,
    ready: State<'_, DesktopFrameReady>,
    window: tauri::WebviewWindow,
) -> Result<&'static str, String> {
    let _command = lifecycle.enter()?;
    if window.label() != "main" {
        return Err("Unsupported desktop window".into());
    }
    if window.is_minimized().map_err(|error| error.to_string())?
        || (ready.0.load(Ordering::Acquire)
            && !window.is_visible().map_err(|error| error.to_string())?)
    {
        // The frontend is alive; startup fallback must not reveal a window the
        // user deliberately minimized or hid to the tray. Initial hidden startup
        // still activates normally. Its focus/restore handler retries later.
        ready.0.store(true, Ordering::Release);
        return Ok("deferred");
    }
    let result = match window.activate_decoration().await {
        Ok(()) => match window.show() {
            Ok(()) => Ok("custom"),
            Err(_) => restore_desktop_frame(&window).await,
        },
        Err(error) => {
            eprintln!("Custom desktop frame unavailable: {error}");
            restore_desktop_frame(&window).await
        }
    };
    ready.0.store(true, Ordering::Release);
    result
}

#[tauri::command]
fn set_desktop_appearance(
    lifecycle: State<'_, DesktopLifecycle>,
    theme: String,
    window: tauri::WebviewWindow,
) -> Result<(), String> {
    let _command = lifecycle.enter()?;
    let theme = match theme.as_str() {
        "light" => tauri::Theme::Light,
        "dark" => tauri::Theme::Dark,
        _ => return Err("Unsupported desktop theme".into()),
    };
    window
        .set_theme(Some(theme))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn show_desktop_window_menu(
    lifecycle: State<'_, DesktopLifecycle>,
    window: tauri::WebviewWindow,
) -> Result<(), String> {
    let _command = lifecycle.enter()?;
    #[cfg(windows)]
    {
        let handle = window.hwnd().map_err(|error| error.to_string())?.0 as usize;
        window
            .run_on_main_thread(move || {
                use windows_sys::Win32::{
                    Foundation::POINT,
                    UI::WindowsAndMessaging::{
                        GetCursorPos, GetSystemMenu, PostMessageW, TrackPopupMenu, TPM_RETURNCMD,
                        TPM_RIGHTBUTTON, WM_SYSCOMMAND,
                    },
                };
                // Only our own main window's OS menu; no arbitrary handles or actions.
                unsafe {
                    let hwnd = handle as windows_sys::Win32::Foundation::HWND;
                    let mut cursor = POINT { x: 0, y: 0 };
                    if GetCursorPos(&mut cursor) == 0 {
                        return;
                    }
                    let menu = GetSystemMenu(hwnd, 0);
                    if menu.is_null() {
                        return;
                    }
                    let action = TrackPopupMenu(
                        menu,
                        TPM_RETURNCMD | TPM_RIGHTBUTTON,
                        cursor.x,
                        cursor.y,
                        0,
                        hwnd,
                        std::ptr::null(),
                    );
                    if action != 0 {
                        PostMessageW(hwnd, WM_SYSCOMMAND, action as usize, 0);
                    }
                }
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

struct DesktopTray {
    open: MenuItem<tauri::Wry>,
    tasks: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
}

#[tauri::command]
fn set_desktop_locale(
    lifecycle: State<'_, DesktopLifecycle>,
    locale: String,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let _command = lifecycle.enter()?;
    let labels = match locale.as_str() {
        "zh-CN" => (
            "打开 GeoD Global",
            "查看任务",
            "退出并停止任务",
            "GeoD Global · 关闭窗口后继续下载和处理",
        ),
        "en" => (
            "Open GeoD Global",
            "View tasks",
            "Quit and stop tasks",
            "GeoD Global · Downloads and processing continue when the window is closed",
        ),
        _ => return Err("Unsupported desktop language".into()),
    };
    let tray = app.state::<DesktopTray>();
    tray.open
        .set_text(labels.0)
        .map_err(|error| error.to_string())?;
    tray.tasks
        .set_text(labels.1)
        .map_err(|error| error.to_string())?;
    tray.quit
        .set_text(labels.2)
        .map_err(|error| error.to_string())?;
    app.tray_by_id("geod-global")
        .ok_or("The desktop tray is unavailable")?
        .set_tooltip(Some(labels.3))
        .map_err(|error| error.to_string())
}

fn restore_window(app: &tauri::AppHandle) {
    if app
        .try_state::<DesktopLifecycle>()
        .is_some_and(|state| state.exiting())
    {
        return;
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn request_exit(app: &tauri::AppHandle) {
    let lifecycle = app.state::<DesktopLifecycle>().inner().clone();
    if !lifecycle.begin_exit() {
        return;
    }
    let tray = app.state::<DesktopTray>();
    let _ = tray.open.set_enabled(false);
    let _ = tray.tasks.set_enabled(false);
    let _ = tray.quit.set_enabled(false);
    let manager = app.state::<JobManager>().inner().clone();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // Finish in-flight IPC writes before interrupting background jobs.
        let drain = lifecycle.clone();
        if let Err(error) =
            tauri::async_runtime::spawn_blocking(move || drain.drain_commands()).await
        {
            eprintln!("Could not drain desktop commands: {error}");
        }
        let result = manager.shutdown().await;
        if let Err(error) = &result {
            eprintln!("Could not save desktop shutdown state: {error}");
        }
        lifecycle.finish();
        app.exit(if result.is_ok() { 0 } else { 1 });
    });
}

#[tauri::command]
fn hide_desktop_window(
    lifecycle: State<'_, DesktopLifecycle>,
    window: tauri::WebviewWindow,
) -> Result<(), String> {
    let _command = lifecycle.enter()?;
    if window.label() != "main" {
        return Err("Unsupported desktop window".into());
    }
    window.hide().map_err(|error| error.to_string())
}

#[tauri::command]
fn quit_desktop(app: tauri::AppHandle, window: tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Unsupported desktop window".into());
    }
    // Do not hold a command guard: request_exit drains the existing IPC guards.
    request_exit(&app);
    Ok(())
}

fn create_tray(app: &tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open GeoD Global", true, None::<&str>)?;
    let tasks = MenuItem::with_id(app, "tasks", "View tasks", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit and stop tasks", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &tasks, &separator, &quit])?;
    let icon = app
        .default_window_icon()
        .ok_or_else(|| tauri::Error::AssetNotFound("Application icon".into()))?
        .clone();
    TrayIconBuilder::with_id("geod-global")
        .icon(icon)
        .tooltip("GeoD Global")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => restore_window(app),
            "tasks" => {
                if !app.state::<DesktopLifecycle>().exiting() {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.eval("window.location.hash = '#Tasks'");
                    }
                    restore_window(app);
                }
            }
            "quit" => request_exit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                restore_window(tray.app_handle());
            }
        })
        .build(app)?;
    app.manage(DesktopTray { open, tasks, quit });
    Ok(())
}

fn allowed_navigation(url: &tauri::Url) -> bool {
    match (url.scheme(), url.host_str(), url.port_or_known_default()) {
        ("tauri", Some("localhost"), None) => true,
        ("http", Some("tauri.localhost"), Some(80)) => true,
        ("https", Some("tauri.localhost"), Some(443)) => true,
        ("http", Some("127.0.0.1"), Some(4317)) => !cfg!(feature = "custom-protocol"),
        _ => false,
    }
}

fn allowed_source(url: &tauri::Url) -> bool {
    url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(
            url.host_str(),
            Some("earth-search.aws.element84.com")
                | Some("sentinel-cogs.s3.us-west-2.amazonaws.com")
                | Some("copernicus-dem-30m.s3.eu-central-1.amazonaws.com")
                | Some("copernicus-dem-90m.s3.eu-central-1.amazonaws.com")
                | Some("registry.opendata.aws")
                | Some("planetarycomputer.microsoft.com")
                | Some("sentinel2l2a01.blob.core.windows.net")
                | Some("landsateuwest.blob.core.windows.net")
                | Some("modiseuwest.blob.core.windows.net")
                | Some("sentinel1euwestrtc.blob.core.windows.net")
                | Some("lpdaac.usgs.gov")
                | Some("cmr.earthdata.nasa.gov")
                | Some("www.earthdata.nasa.gov")
                | Some("search.earthdata.nasa.gov")
                | Some("urs.earthdata.nasa.gov")
                | Some("identity.dataspace.copernicus.eu")
                | Some("stac.dataspace.copernicus.eu")
                | Some("dataspace.copernicus.eu")
                | Some("browser.dataspace.copernicus.eu")
                | Some("www.openstreetmap.org")
        )
}

fn open_source_url(app: &tauri::AppHandle, url: &tauri::Url) -> Result<(), String> {
    if !allowed_source(url) {
        return Err("Only reviewed official data-source HTTPS links can be opened.".to_owned());
    }
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|error| format!("Could not open the source in your browser: {error}"))
}

#[tauri::command]
fn open_source(
    lifecycle: State<'_, DesktopLifecycle>,
    url: String,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let _command = lifecycle.enter()?;
    let url = tauri::Url::parse(&url).map_err(|_| "Invalid source URL.")?;
    open_source_url(&app, &url)
}

#[tauri::command]
fn health(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
) -> Result<RuntimeHealth, String> {
    let _command = lifecycle.enter()?;
    Ok(manager.health())
}

#[tauri::command]
async fn diagnostics(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
) -> Result<serde_json::Value, String> {
    let _command = lifecycle.enter()?;
    Ok(manager.diagnostics().await)
}

#[tauri::command]
async fn get_proxy_settings(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
) -> Result<ProxySettings, String> {
    let _command = lifecycle.enter()?;
    Ok(manager.proxy_settings().await)
}

#[tauri::command]
async fn list_provider_accounts(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
) -> Result<Vec<AccountStatus>, String> {
    let _command = lifecycle.enter()?;
    Ok(manager.provider_accounts().await)
}

#[tauri::command]
async fn resolve_copernicus_products(
    lifecycle: State<'_, DesktopLifecycle>,
    request: geod_runtime::providers::copernicus::ResolveProductsRequest,
    manager: State<'_, JobManager>,
) -> Result<Vec<geod_runtime::providers::copernicus::ProductAsset>, String> {
    let _command = lifecycle.enter()?;
    manager.resolve_copernicus_products(request).await
}

#[tauri::command]
async fn connect_provider_account(
    lifecycle: State<'_, DesktopLifecycle>,
    request: ConnectAccountRequest,
    manager: State<'_, JobManager>,
) -> Result<AccountStatus, String> {
    let _command = lifecycle.enter()?;
    manager.connect_provider_account(request).await
}

#[tauri::command]
async fn verify_provider_account(
    lifecycle: State<'_, DesktopLifecycle>,
    provider: AccountProvider,
    manager: State<'_, JobManager>,
) -> Result<AccountStatus, String> {
    let _command = lifecycle.enter()?;
    manager.verify_provider_account(provider).await
}

#[tauri::command]
async fn disconnect_provider_account(
    lifecycle: State<'_, DesktopLifecycle>,
    provider: AccountProvider,
    manager: State<'_, JobManager>,
) -> Result<AccountStatus, String> {
    let _command = lifecycle.enter()?;
    manager.disconnect_provider_account(provider).await
}

#[tauri::command]
async fn save_proxy_settings(
    lifecycle: State<'_, DesktopLifecycle>,
    settings: ProxySettings,
    manager: State<'_, JobManager>,
) -> Result<ProxySettings, String> {
    let _command = lifecycle.enter()?;
    manager.save_proxy_settings(settings).await
}

#[tauri::command]
async fn test_proxy_settings(
    lifecycle: State<'_, DesktopLifecycle>,
    settings: ProxySettings,
    manager: State<'_, JobManager>,
) -> Result<ProxyTest, String> {
    let _command = lifecycle.enter()?;
    manager.test_proxy_settings(settings).await
}

#[tauri::command]
async fn list_vectors(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
) -> Result<Vec<geod_runtime::vector::VectorAsset>, String> {
    let _command = lifecycle.enter()?;
    Ok(manager.list_vectors().await)
}
#[tauri::command]
async fn list_stac_connections(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
) -> Result<Vec<geod_runtime::stac::Connection>, String> {
    let _command = lifecycle.enter()?;
    Ok(manager.list_stac_connections().await)
}

#[tauri::command]
async fn connect_stac(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: geod_runtime::stac::ConnectRequest,
) -> Result<geod_runtime::stac::Connection, String> {
    let _command = lifecycle.enter()?;
    manager.connect_stac(request).await
}

#[tauri::command]
async fn forget_stac_connection(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
) -> Result<(), String> {
    let _command = lifecycle.enter()?;
    manager.forget_stac_connection(&id).await
}

#[tauri::command]
async fn search_stac(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: geod_runtime::stac::SearchRequest,
) -> Result<geod_runtime::stac::SearchPage, String> {
    let _command = lifecycle.enter()?;
    manager.search_stac(request).await
}

#[tauri::command]
async fn stac_snapshot(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
) -> Result<geod_runtime::stac::ItemSnapshot, String> {
    let _command = lifecycle.enter()?;
    manager.stac_snapshot(&id).await
}

#[tauri::command]
async fn inspect_stac_asset(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
) -> Result<geod_runtime::stac::raster::GenericRasterInspection, String> {
    let _command = lifecycle.enter()?;
    manager.inspect_stac_asset(&id).await
}

#[tauri::command]
async fn sample_stac_asset(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
    column: u32,
    row: u32,
) -> Result<geod_runtime::stac::raster::GenericRasterPixel, String> {
    let _command = lifecycle.enter()?;
    manager.sample_stac_asset(&id, column, row).await
}

#[tauri::command]
async fn save_stac_project(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: geod_runtime::stac_projects::SaveProjectRequest,
) -> Result<geod_runtime::Project, String> {
    let _command = lifecycle.enter()?;
    manager.save_stac_project(request).await
}

#[tauri::command]
async fn download_stac_project(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: geod_runtime::stac_projects::DownloadRequest,
) -> Result<geod_runtime::ProjectDownloads, String> {
    let _command = lifecycle.enter()?;
    manager.download_stac_project(request).await
}

#[tauri::command]
async fn list_wcs_connections(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
) -> Result<Vec<geod_runtime::wcs::Connection>, String> {
    let _command = lifecycle.enter()?;
    Ok(manager.list_wcs_connections().await)
}

#[tauri::command]
async fn connect_wcs(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: geod_runtime::wcs::ConnectRequest,
) -> Result<geod_runtime::wcs::Connection, String> {
    let _command = lifecycle.enter()?;
    manager.connect_wcs(request).await
}

#[tauri::command]
async fn forget_wcs_connection(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
) -> Result<(), String> {
    let _command = lifecycle.enter()?;
    manager.forget_wcs_connection(&id).await
}

#[tauri::command]
async fn describe_wcs(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: geod_runtime::wcs::DescribeRequest,
) -> Result<geod_runtime::wcs::Description, String> {
    let _command = lifecycle.enter()?;
    manager.describe_wcs(request).await
}

#[tauri::command]
async fn wcs_plan(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
) -> Result<geod_runtime::wcs::Plan, String> {
    let _command = lifecycle.enter()?;
    manager.wcs_plan(&id).await
}

#[tauri::command]
async fn inspect_wcs_asset(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
) -> Result<geod_runtime::stac::raster::GenericRasterInspection, String> {
    let _command = lifecycle.enter()?;
    manager.inspect_wcs_asset(&id).await
}

#[tauri::command]
async fn sample_wcs_asset(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
    column: u32,
    row: u32,
) -> Result<geod_runtime::stac::raster::GenericRasterPixel, String> {
    let _command = lifecycle.enter()?;
    manager.sample_wcs_asset(&id, column, row).await
}

#[tauri::command]
async fn save_wcs_project(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: geod_runtime::wcs_projects::SaveProjectRequest,
) -> Result<geod_runtime::Project, String> {
    let _command = lifecycle.enter()?;
    manager.save_wcs_project(request).await
}

#[tauri::command]
async fn download_wcs_project(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: geod_runtime::wcs_projects::DownloadRequest,
) -> Result<geod_runtime::ProjectDownloads, String> {
    let _command = lifecycle.enter()?;
    manager.download_wcs_project(request).await
}

#[tauri::command]
async fn wcs_description(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
) -> Result<geod_runtime::wcs::Description, String> {
    let _command = lifecycle.enter()?;
    manager.wcs_description(&id).await
}

#[tauri::command]
async fn plan_wcs(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: geod_runtime::wcs::PlanRequest,
) -> Result<geod_runtime::wcs::Plan, String> {
    let _command = lifecycle.enter()?;
    manager.plan_wcs(request).await
}

#[tauri::command]
async fn list_feature_services(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
) -> Result<Vec<geod_runtime::features::FeatureService>, String> {
    let _command = lifecycle.enter()?;
    Ok(manager.list_feature_services().await)
}
#[tauri::command]
async fn connect_feature_service(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: geod_runtime::features::ConnectRequest,
) -> Result<geod_runtime::features::FeatureService, String> {
    let _command = lifecycle.enter()?;
    manager.connect_feature_service(request).await
}
#[tauri::command]
async fn forget_feature_service(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
) -> Result<(), String> {
    let _command = lifecycle.enter()?;
    manager.forget_feature_service(&id).await
}
#[tauri::command]
async fn query_features(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: geod_runtime::features::QueryRequest,
) -> Result<geod_runtime::vector::VectorAsset, String> {
    let _command = lifecycle.enter()?;
    manager.query_features(request).await
}
#[tauri::command]
async fn import_vector(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: geod_runtime::vector::ImportVectorRequest,
) -> Result<geod_runtime::vector::VectorAsset, String> {
    let _command = lifecycle.enter()?;
    manager.import_vector(request).await
}
#[tauri::command]
async fn list_map_services(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
) -> Result<Vec<geod_runtime::wms::MapService>, String> {
    let _command = lifecycle.enter()?;
    Ok(manager.list_map_services().await)
}
#[tauri::command]
async fn list_tile_sources(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
) -> Result<Vec<geod_runtime::tiles::Source>, String> {
    let _command = lifecycle.enter()?;
    Ok(manager.list_tile_sources().await)
}
#[tauri::command]
async fn list_three_d(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
) -> Result<Vec<geod_runtime::three_d::Package>, String> {
    let _command = lifecycle.enter()?;
    Ok(manager.list_three_d().await)
}
#[tauri::command]
async fn discover_three_d(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: geod_runtime::three_d::DiscoverRequest,
) -> Result<geod_runtime::three_d::Discovery, String> {
    let _command = lifecycle.enter()?;
    manager.discover_three_d(request).await
}
#[tauri::command]
async fn acquire_three_d(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: geod_runtime::three_d::AcquireRequest,
) -> Result<geod_runtime::three_d::Package, String> {
    let _command = lifecycle.enter()?;
    manager.acquire_three_d(request).await
}
#[tauri::command]
async fn inspect_three_d(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
) -> Result<geod_runtime::three_d::Package, String> {
    let _command = lifecycle.enter()?;
    manager.inspect_three_d(&id).await
}
#[tauri::command]
async fn read_three_d_resource(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: geod_runtime::three_d::ResourceRequest,
) -> Result<geod_runtime::three_d::ResourceData, String> {
    let _command = lifecycle.enter()?;
    manager.read_three_d_resource(request).await
}
#[tauri::command]
async fn open_three_d(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    window: tauri::WebviewWindow,
    request: geod_runtime::three_d::LocalRequest,
) -> Result<Option<geod_runtime::three_d::Package>, String> {
    let _command = lifecycle.enter()?;
    #[cfg(windows)]
    {
        let path = tauri::async_runtime::spawn_blocking(move || {
            rfd::FileDialog::new()
                .set_parent(&window)
                .add_filter("3D Tiles or glTF scene", &["json", "gltf", "glb", "zip"])
                .pick_file()
        })
        .await
        .map_err(|e| e.to_string())?;
        match path {
            Some(path) => manager.open_three_d_path(path, request).await.map(Some),
            None => Ok(None),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (manager, window, request);
        Err("Native 3D file selection is not available on this platform".into())
    }
}
#[tauri::command]
async fn export_three_d(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    window: tauri::WebviewWindow,
    id: String,
) -> Result<bool, String> {
    let _command = lifecycle.enter()?;
    #[cfg(windows)]
    {
        let name = format!("geod-3d-{id}.zip");
        let path = tauri::async_runtime::spawn_blocking(move || {
            rfd::FileDialog::new()
                .set_parent(&window)
                .add_filter("Offline 3D package", &["zip"])
                .set_file_name(name)
                .save_file()
        })
        .await
        .map_err(|e| e.to_string())?;
        if let Some(path) = path {
            manager.export_three_d_path(&id, path).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (manager, window, id);
        Err("Native 3D export is not available on this platform".into())
    }
}
#[tauri::command]
async fn connect_tiles(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: geod_runtime::tiles::ConnectRequest,
) -> Result<geod_runtime::tiles::Source, String> {
    let _command = lifecycle.enter()?;
    manager.connect_tiles(request).await
}
#[tauri::command]
async fn forget_tile_source(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
) -> Result<(), String> {
    let _command = lifecycle.enter()?;
    manager.forget_tile_source(&id).await
}
#[tauri::command]
async fn list_tile_packages(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
) -> Result<Vec<geod_runtime::tiles::Package>, String> {
    let _command = lifecycle.enter()?;
    Ok(manager.list_tile_packages().await)
}
#[tauri::command]
async fn open_tile_package(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    window: tauri::WebviewWindow,
) -> Result<Option<geod_runtime::tiles::Package>, String> {
    let _command = lifecycle.enter()?;
    #[cfg(windows)]
    {
        let path = tauri::async_runtime::spawn_blocking(move || {
            rfd::FileDialog::new()
                .set_parent(&window)
                .add_filter("Offline tile archive", &["pmtiles", "mbtiles"])
                .pick_file()
        })
        .await
        .map_err(|e| e.to_string())?;
        match path {
            Some(path) => manager.open_tile_path(path).await.map(Some),
            None => Ok(None),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (manager, window);
        Err("Native tile file selection is not available on this platform".into())
    }
}
#[tauri::command]
async fn extract_tiles(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: geod_runtime::tiles::ExtractRequest,
) -> Result<geod_runtime::tiles::Package, String> {
    let _command = lifecycle.enter()?;
    manager.extract_tiles(request).await
}
#[tauri::command]
async fn inspect_tile_package(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
) -> Result<geod_runtime::tiles::Inspection, String> {
    let _command = lifecycle.enter()?;
    manager.inspect_tile_package(&id).await
}
#[tauri::command]
async fn read_tile(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: geod_runtime::tiles::TileRequest,
) -> Result<geod_runtime::tiles::Tile, String> {
    let _command = lifecycle.enter()?;
    manager.read_tile(request).await
}
#[tauri::command]
async fn export_tile_package(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
    window: tauri::WebviewWindow,
) -> Result<bool, String> {
    let _command = lifecycle.enter()?;
    #[cfg(windows)]
    {
        let name = format!("geod-tiles-{id}.zip");
        let path = tauri::async_runtime::spawn_blocking(move || {
            rfd::FileDialog::new()
                .set_parent(&window)
                .add_filter("Offline tile package", &["zip"])
                .set_file_name(name)
                .save_file()
        })
        .await
        .map_err(|e| e.to_string())?;
        if let Some(path) = path {
            manager.export_tile_package_path(&id, path).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (manager, id, window);
        Err("Native tile export is not available on this platform".into())
    }
}
#[tauri::command]
async fn connect_map_service(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: geod_runtime::wms::ConnectRequest,
) -> Result<geod_runtime::wms::MapService, String> {
    let _command = lifecycle.enter()?;
    manager.connect_map_service(request).await
}
#[tauri::command]
async fn forget_map_service(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
) -> Result<(), String> {
    let _command = lifecycle.enter()?;
    manager.forget_map_service(&id).await
}
#[tauri::command]
async fn list_map_images(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
) -> Result<Vec<geod_runtime::wms::MapImage>, String> {
    let _command = lifecycle.enter()?;
    Ok(manager.list_map_images().await)
}
#[tauri::command]
async fn get_map_image(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: geod_runtime::wms::MapRequest,
) -> Result<geod_runtime::wms::MapImage, String> {
    let _command = lifecycle.enter()?;
    manager.get_map_image(request).await
}
#[tauri::command]
async fn inspect_map_image(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
) -> Result<geod_runtime::wms::MapInspection, String> {
    let _command = lifecycle.enter()?;
    manager.inspect_map_image(&id).await
}
#[tauri::command]
async fn export_map_image(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
    window: tauri::WebviewWindow,
) -> Result<bool, String> {
    let _command = lifecycle.enter()?;
    #[cfg(windows)]
    {
        let name = format!("geod-wms-{id}.zip");
        let path = tauri::async_runtime::spawn_blocking(move || {
            rfd::FileDialog::new()
                .set_parent(&window)
                .add_filter("Georeferenced map package", &["zip"])
                .set_file_name(name)
                .save_file()
        })
        .await
        .map_err(|e| e.to_string())?;
        if let Some(path) = path {
            // ID comes from the verified asset registry, never a path from JavaScript.
            manager.export_map_image_path(&id, path).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (manager, id, window);
        Err("Native map export is not available on this platform".into())
    }
}
#[tauri::command]
async fn open_vector(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    managed: bool,
    window: tauri::WebviewWindow,
) -> Result<Option<geod_runtime::vector::VectorAsset>, String> {
    let _command = lifecycle.enter()?;
    #[cfg(windows)]
    {
        let path = tauri::async_runtime::spawn_blocking(move || {
            rfd::FileDialog::new()
                .set_parent(&window)
                .add_filter(
                    "GeoJSON / OSM XML, PBF, JSON / GeoPackage / Shapefile",
                    &["geojson", "json", "gpkg", "shp", "zip", "osm", "xml", "pbf"],
                )
                .pick_file()
        })
        .await
        .map_err(|e| e.to_string())?;
        match path {
            Some(path) => manager.open_vector_path(path, managed).await.map(Some),
            None => Ok(None),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (manager, managed, window);
        Err("Native vector file selection is not available on this platform".into())
    }
}
#[tauri::command]
async fn inspect_vector(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
) -> Result<geod_runtime::vector::VectorInspection, String> {
    let _command = lifecycle.enter()?;
    manager.inspect_vector(&id).await
}
#[tauri::command]
async fn forget_vector(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
) -> Result<(), String> {
    let _command = lifecycle.enter()?;
    manager.forget_vector(&id).await
}
#[tauri::command]
async fn export_vector(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
    original: Option<bool>,
    window: tauri::WebviewWindow,
) -> Result<bool, String> {
    let _command = lifecycle.enter()?;
    #[cfg(windows)]
    {
        let asset = manager.inspect_vector(&id).await?.asset;
        let original = original.unwrap_or(false);
        let extension = if original && asset.geo_package.is_some() {
            "gpkg"
        } else if original && asset.shapefile.is_some() {
            "zip"
        } else if original && asset.local_osm.is_some() {
            if asset.local_osm.as_ref().unwrap().encoding == "xml" {
                "osm"
            } else {
                "pbf"
            }
        } else if original {
            "json"
        } else {
            "geojson"
        };
        let name = format!(
            "{}.{}",
            std::path::Path::new(&asset.name)
                .file_stem()
                .and_then(|v| v.to_str())
                .unwrap_or("vector"),
            extension
        );
        let path = tauri::async_runtime::spawn_blocking(move || {
            rfd::FileDialog::new()
                .set_parent(&window)
                .add_filter(
                    if original {
                        "Original vector file"
                    } else {
                        "GeoJSON"
                    },
                    &[extension],
                )
                .set_file_name(name)
                .save_file()
        })
        .await
        .map_err(|e| e.to_string())?;
        match path {
            Some(path) => {
                if original {
                    manager.export_vector_original_path(&id, path).await?;
                } else {
                    manager.export_vector_path(&id, path).await?;
                }
                Ok(true)
            }
            None => Ok(false),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (manager, id, original, window);
        Err("Native vector export is not available on this platform".into())
    }
}
#[tauri::command]
async fn list_jobs(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
) -> Result<Vec<Job>, String> {
    let _command = lifecycle.enter()?;
    Ok(manager.list().await)
}

#[tauri::command]
async fn list_projects(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
) -> Result<Vec<Project>, String> {
    let _command = lifecycle.enter()?;
    Ok(manager.list_projects().await)
}

#[tauri::command]
async fn create_project(
    lifecycle: State<'_, DesktopLifecycle>,
    request: CreateProjectRequest,
    manager: State<'_, JobManager>,
) -> Result<Project, String> {
    let _command = lifecycle.enter()?;
    manager.create_project(request).await
}

#[tauri::command]
async fn rename_project(
    lifecycle: State<'_, DesktopLifecycle>,
    id: String,
    name: String,
    manager: State<'_, JobManager>,
) -> Result<Project, String> {
    let _command = lifecycle.enter()?;
    manager.rename_project(&id, &name).await
}

#[tauri::command]
async fn add_project_scenes(
    lifecycle: State<'_, DesktopLifecycle>,
    id: String,
    request: AddProjectScenesRequest,
    manager: State<'_, JobManager>,
) -> Result<Project, String> {
    let _command = lifecycle.enter()?;
    manager.add_project_scenes(&id, request).await
}

#[tauri::command]
async fn download_project(
    lifecycle: State<'_, DesktopLifecycle>,
    id: String,
    asset_key: String,
    item_ids: Option<Vec<String>>,
    manager: State<'_, JobManager>,
) -> Result<ProjectDownloads, String> {
    let _command = lifecycle.enter()?;
    manager
        .enqueue_project_selection(&id, &asset_key, item_ids)
        .await
}

#[tauri::command]
async fn prepare_project(
    lifecycle: State<'_, DesktopLifecycle>,
    id: String,
    asset_key: String,
    manager: State<'_, JobManager>,
) -> Result<ProjectDownloads, String> {
    let _command = lifecycle.enter()?;
    manager.prepare_project(&id, &asset_key).await
}

#[tauri::command]
async fn mosaic_project(
    lifecycle: State<'_, DesktopLifecycle>,
    id: String,
    asset_key: String,
    vi_quality: Option<geod_runtime::mosaic::vegetation::Request>,
    manager: State<'_, JobManager>,
) -> Result<Job, String> {
    let _command = lifecycle.enter()?;
    manager
        .run_project_mosaic_with_selection(&id, &asset_key, vi_quality)
        .await
}

#[tauri::command]
async fn create_job(
    lifecycle: State<'_, DesktopLifecycle>,
    request: CreateJobRequest,
    manager: State<'_, JobManager>,
) -> Result<Job, String> {
    let _command = lifecycle.enter()?;
    manager.create(request).await
}

#[tauri::command]
async fn cancel_job(
    lifecycle: State<'_, DesktopLifecycle>,
    id: String,
    manager: State<'_, JobManager>,
) -> Result<Job, String> {
    let _command = lifecycle.enter()?;
    manager.cancel(&id).await
}

#[tauri::command]
async fn retry_job(
    lifecycle: State<'_, DesktopLifecycle>,
    id: String,
    manager: State<'_, JobManager>,
) -> Result<Job, String> {
    let _command = lifecycle.enter()?;
    manager.retry(&id).await
}

#[tauri::command]
async fn inspect_raster(
    lifecycle: State<'_, DesktopLifecycle>,
    id: String,
    manager: State<'_, JobManager>,
    aerial_view: Option<geod_runtime::raster::AerialView>,
) -> Result<RasterInspection, String> {
    let _command = lifecycle.enter()?;
    manager.inspect_raster_view(&id, aerial_view).await
}

#[tauri::command]
async fn file_thumbnail(
    lifecycle: State<'_, DesktopLifecycle>,
    id: String,
    manager: State<'_, JobManager>,
) -> Result<geod_runtime::thumbnail::FileThumbnail, String> {
    let _command = lifecycle.enter()?;
    manager.file_thumbnail(&id).await
}

#[tauri::command]
async fn inspect_composite(
    lifecycle: State<'_, DesktopLifecycle>,
    request: geod_runtime::CompositeRequest,
    manager: State<'_, JobManager>,
) -> Result<geod_runtime::CompositeInspection, String> {
    let _command = lifecycle.enter()?;
    manager.inspect_composite(request).await
}

#[tauri::command]
async fn sample_composite(
    lifecycle: State<'_, DesktopLifecycle>,
    request: geod_runtime::CompositePixelRequest,
    manager: State<'_, JobManager>,
) -> Result<geod_runtime::CompositePixel, String> {
    let _command = lifecycle.enter()?;
    manager.sample_composite(request).await
}

#[tauri::command]
async fn sample_raster(
    lifecycle: State<'_, DesktopLifecycle>,
    id: String,
    x: f64,
    y: f64,
    manager: State<'_, JobManager>,
) -> Result<RasterPixel, String> {
    let _command = lifecycle.enter()?;
    manager.sample_raster(&id, x, y).await
}

#[tauri::command]
async fn plan_scientific_rgb(
    lifecycle: State<'_, DesktopLifecycle>,
    request: geod_runtime::RgbRequest,
    manager: State<'_, JobManager>,
) -> Result<geod_runtime::RgbPlan, String> {
    let _command = lifecycle.enter()?;
    manager.plan_scientific_rgb(request).await
}
#[tauri::command]
async fn run_scientific_rgb(
    lifecycle: State<'_, DesktopLifecycle>,
    request: geod_runtime::RgbRequest,
    manager: State<'_, JobManager>,
) -> Result<Job, String> {
    let _command = lifecycle.enter()?;
    manager.run_scientific_rgb(request).await
}
#[tauri::command]
async fn inspect_scientific_rgb(
    lifecycle: State<'_, DesktopLifecycle>,
    id: String,
    manager: State<'_, JobManager>,
) -> Result<geod_runtime::CompositeInspection, String> {
    let _command = lifecycle.enter()?;
    manager.inspect_scientific_rgb(&id).await
}
#[tauri::command]
async fn sample_scientific_rgb(
    lifecycle: State<'_, DesktopLifecycle>,
    id: String,
    x: f64,
    y: f64,
    manager: State<'_, JobManager>,
) -> Result<geod_runtime::CompositePixel, String> {
    let _command = lifecycle.enter()?;
    manager.sample_scientific_rgb(&id, x, y).await
}

#[tauri::command]
async fn prepare_artifact(
    lifecycle: State<'_, DesktopLifecycle>,
    id: String,
    manager: State<'_, JobManager>,
) -> Result<geod_runtime::artifact::ArtifactPackage, String> {
    let _command = lifecycle.enter()?;
    manager.prepare_artifact(&id).await
}

#[tauri::command]
async fn reveal_artifact(
    lifecycle: State<'_, DesktopLifecycle>,
    id: String,
    manager: State<'_, JobManager>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let _command = lifecycle.enter()?;
    let (package, _) = manager.artifact_bytes(&id).await?;
    app.opener()
        .reveal_item_in_dir(package.path)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn list_recipes(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
) -> Result<Vec<SavedRecipe>, String> {
    let _command = lifecycle.enter()?;
    Ok(manager.list_recipes().await)
}

#[tauri::command]
async fn plan_recipe(
    lifecycle: State<'_, DesktopLifecycle>,
    recipe: RasterRecipe,
    manager: State<'_, JobManager>,
) -> Result<RecipePlan, String> {
    let _command = lifecycle.enter()?;
    manager.plan_recipe(recipe).await
}

#[tauri::command]
async fn save_recipe(
    lifecycle: State<'_, DesktopLifecycle>,
    recipe: RasterRecipe,
    manager: State<'_, JobManager>,
) -> Result<SavedRecipe, String> {
    let _command = lifecycle.enter()?;
    manager.save_recipe(recipe).await
}

#[tauri::command]
async fn run_recipe(
    lifecycle: State<'_, DesktopLifecycle>,
    recipe: RasterRecipe,
    manager: State<'_, JobManager>,
) -> Result<Job, String> {
    let _command = lifecycle.enter()?;
    manager.run_recipe(recipe).await
}

fn verified_output(storage_root: &Path, job: &Job) -> Result<PathBuf, String> {
    geod_runtime::verified_output_path(storage_root, job)
}

#[tauri::command]
async fn reveal_job(
    lifecycle: State<'_, DesktopLifecycle>,
    id: String,
    manager: State<'_, JobManager>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let _command = lifecycle.enter()?;
    let job = manager.get(&id).await.ok_or("Job not found.")?;
    let output = verified_output(manager.storage_root(), &job)?;
    app.opener()
        .reveal_item_in_dir(output)
        .map_err(|error| format!("Could not reveal the output in your file explorer: {error}"))
}

fn main() {
    tauri::Builder::default()
        .register_asynchronous_uri_scheme_protocol("geod-elevation", elevation_preview::handle)
        // Claim the application instance before opening its exclusive runtime store.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            restore_window(app);
        }))
        .plugin(tauri_plugin_decoration::init())
        .plugin(
            tauri_plugin_opener::Builder::new()
                .open_js_links_on_click(false)
                .build(),
        )
        .setup(|app| {
            app.manage(DesktopLifecycle::default());
            app.manage(DesktopFrameReady::default());
            let storage_root = app.path().app_local_data_dir()?.join("runtime");
            let manager = tauri::async_runtime::block_on(JobManager::open(storage_root))
                .map_err(std::io::Error::other)?;
            app.manage(manager);
            // Install native state before the webview can send its first IPC call.
            create_tray(app)?;
            let config = app.config().app.windows[0].clone();
            let navigation_app = app.handle().clone();
            let new_window_app = app.handle().clone();
            let window = WebviewWindowBuilder::from_config(app, &config)?
                .on_navigation(move |url| {
                    if allowed_navigation(url) {
                        return true;
                    }
                    if allowed_source(url) {
                        if let Err(error) = open_source_url(&navigation_app, url) {
                            eprintln!("{error}");
                        }
                    }
                    false
                })
                .on_new_window(move |url, _| {
                    if allowed_source(&url) {
                        if let Err(error) = open_source_url(&new_window_app, &url) {
                            eprintln!("{error}");
                        }
                    }
                    tauri::webview::NewWindowResponse::Deny
                })
                .build()?;
            // A broken frontend must never leave an invisible, unmanageable app.
            let ready = app.state::<DesktopFrameReady>().inner().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(8)).await;
                if !ready.0.swap(true, Ordering::AcqRel) {
                    if let Err(error) = restore_desktop_frame(&window).await {
                        eprintln!("Could not reveal the native frame fallback: {error}");
                        let _ = window.show();
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_three_d,
            discover_three_d,
            acquire_three_d,
            inspect_three_d,
            read_three_d_resource,
            open_three_d,
            export_three_d,
            activate_desktop_frame,
            set_desktop_appearance,
            show_desktop_window_menu,
            hide_desktop_window,
            quit_desktop,
            set_desktop_locale,
            health,
            diagnostics,
            get_proxy_settings,
            save_proxy_settings,
            test_proxy_settings,
            list_provider_accounts,
            resolve_copernicus_products,
            connect_provider_account,
            verify_provider_account,
            disconnect_provider_account,
            list_jobs,
            list_vectors,
            list_feature_services,
            wcs_description,
            list_wcs_connections,
            connect_wcs,
            forget_wcs_connection,
            describe_wcs,
            plan_wcs,
            wcs_plan,
            inspect_wcs_asset,
            sample_wcs_asset,
            save_wcs_project,
            download_wcs_project,
            list_stac_connections,
            connect_stac,
            forget_stac_connection,
            search_stac,
            stac_snapshot,
            inspect_stac_asset,
            sample_stac_asset,
            save_stac_project,
            download_stac_project,
            connect_feature_service,
            forget_feature_service,
            query_features,
            list_map_services,
            list_tile_sources,
            connect_tiles,
            forget_tile_source,
            list_tile_packages,
            extract_tiles,
            open_tile_package,
            inspect_tile_package,
            read_tile,
            export_tile_package,
            connect_map_service,
            forget_map_service,
            list_map_images,
            get_map_image,
            inspect_map_image,
            export_map_image,
            import_vector,
            open_vector,
            inspect_vector,
            forget_vector,
            export_vector,
            list_projects,
            create_project,
            rename_project,
            add_project_scenes,
            download_project,
            mosaic_project,
            prepare_project,
            create_job,
            cancel_job,
            retry_job,
            inspect_raster,
            inspect_composite,
            plan_scientific_rgb,
            run_scientific_rgb,
            inspect_scientific_rgb,
            sample_scientific_rgb,
            sample_composite,
            file_thumbnail,
            sample_raster,
            prepare_artifact,
            reveal_artifact,
            list_recipes,
            plan_recipe,
            save_recipe,
            run_recipe,
            reveal_job,
            open_source
        ])
        .build(tauri::generate_context!())
        .expect("GeoD Global desktop could not start")
        .run(|app, event| match event {
            RunEvent::WindowEvent {
                label,
                event: WindowEvent::CloseRequested { api, .. },
                ..
            } if label == "main" => {
                api.prevent_close();
                // The tray was created successfully before this event loop starts.
                if !app.state::<DesktopLifecycle>().exiting() {
                    if let Some(window) = app.get_webview_window("main") {
                        if let Err(error) = window.hide() {
                            eprintln!("Could not hide the desktop window: {error}");
                        }
                    }
                }
            }
            RunEvent::ExitRequested { api, .. } => {
                if !app.state::<DesktopLifecycle>().finished() {
                    api.prevent_exit();
                    request_exit(app);
                }
            }
            _ => {}
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use geod_runtime::JobStatus;

    #[test]
    fn window_rejects_remote_or_file_navigation() {
        for address in ["tauri://localhost/index.html", "http://tauri.localhost/"] {
            assert!(allowed_navigation(&address.parse().unwrap()));
        }
        for address in [
            "https://example.com/",
            "http://tauri.localhost.evil.test/",
            "file:///C:/Windows/win.ini",
            "http://127.0.0.1:4318/",
        ] {
            assert!(!allowed_navigation(&address.parse().unwrap()));
        }
    }

    #[test]
    fn source_links_allow_only_official_https_hosts() {
        for address in [
            "https://earth-search.aws.element84.com/v1/collections/sentinel-2-l2a/items/item",
            "https://copernicus-dem-30m.s3.eu-central-1.amazonaws.com/Copernicus_DSM_COG_10_N37_00_W123_00_DEM/Copernicus_DSM_COG_10_N37_00_W123_00_DEM.tif",
            "https://copernicus-dem-90m.s3.eu-central-1.amazonaws.com/Copernicus_DSM_COG_30_N51_00_W001_00_DEM/Copernicus_DSM_COG_30_N51_00_W001_00_DEM.tif",
            "https://sentinel-cogs.s3.us-west-2.amazonaws.com/sentinel-s2-l2a-cogs/item/SCL.tif",
            "https://registry.opendata.aws/sentinel-2-l2a-cogs/",
            "https://planetarycomputer.microsoft.com/dataset/sentinel-2-l2a",
            "https://planetarycomputer.microsoft.com/dataset/sentinel-1-rtc",
            "https://sentinel1euwestrtc.blob.core.windows.net/sentinel1-grd-rtc/GRD/2025/6/30/IW/DV/S1C_IW_GRDH_1SDV_20250630T140654_20250630T140719_003013_00622B_A30F/measurement/iw-vv.rtc.tiff",
            "https://sentinel2l2a01.blob.core.windows.net/sentinel2-l2/example.tif",
            "https://stac.dataspace.copernicus.eu/v1/collections/sentinel-2-l2a",
            "https://browser.dataspace.copernicus.eu/",
            "https://landsateuwest.blob.core.windows.net/landsat-c2/example.TIF",
            "https://modiseuwest.blob.core.windows.net/modis-061-cogs/MYD09A1/08/05/2025177/MYD09A1.A2025177.h08v05.061.2025189031924_sur_refl_b01.tif",
            "https://lpdaac.usgs.gov/data/data-citation-and-policies/",
            "https://cmr.earthdata.nasa.gov/stac/LPCLOUD/collections/HLSL30_2.0",
            "https://www.earthdata.nasa.gov/data/catalog/lpcloud-hlsl30-2.0",
            "https://search.earthdata.nasa.gov/",
            "https://www.openstreetmap.org/copyright",
        ] {
            assert!(allowed_source(&address.parse().unwrap()));
        }
        for address in [
            "http://earth-search.aws.element84.com/",
            "https://earth-search.aws.element84.com.evil.test/",
            "https://earth-search.aws.element84.com:444/",
            "https://user@earth-search.aws.element84.com/",
            "https://registry.opendata.aws@evil.test/",
            "https://unreviewed.blob.core.windows.net/example.tif",
            "https://planetarycomputer.microsoft.com.evil.test/",
            "https://modiseuwest.blob.core.windows.net.evil.test/example.tif",
            "https://sentinel1euwestrtc.blob.core.windows.net.evil.test/example.tiff",
            "https://lpdaac.usgs.gov@evil.test/",
            "https://stac.dataspace.copernicus.eu:444/",
            "https://cmr.earthdata.nasa.gov.evil.test/",
            "https://search.earthdata.nasa.gov:444/",
            "https://www.openstreetmap.org.evil.test/copyright",
            "file:///C:/Windows/win.ini",
            "javascript:alert(1)",
        ] {
            assert!(!allowed_source(&address.parse().unwrap()));
        }
    }

    fn completed(path: &Path) -> Job {
        Job {
            id: "11111111-1111-4111-8111-111111111111".into(),
            kind: "download".into(),
            parent_id: None,
            recipe: None,
            crop: None,
            mosaic: None,
            mosaic_output: None,
            manifest_path: None,
            safe: None,
            safe_output: None,
            viirs_science: None,
            transfer: None,
            viirs_prepare: None,
            stac_source: None,
            wcs_source: None,
            rgb_spec: None,
            rgb_output: None,
            item_id: "S2_TEST".into(),
            asset_key: "thumbnail".into(),
            href: "https://sentinel-cogs.s3.us-west-2.amazonaws.com/example.jpg".into(),
            media_type: "image/jpeg".into(),
            title: "Test".into(),
            status: JobStatus::Succeeded,
            bytes_downloaded: 1,
            total_bytes: Some(1),
            sha256: None,
            output_path: Some(path.to_string_lossy().into_owned()),
            error: None,
            created_at: "2026-09-22T00:00:00Z".into(),
            updated_at: "2026-09-22T00:00:00Z".into(),
            source: "test".into(),
            validation: "test".into(),
            attempts: 1,
        }
    }

    #[test]
    fn reveal_requires_existing_successful_output_within_storage() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("runtime");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(root.join("assets")).unwrap();
        let output = root.join("assets/11111111-1111-4111-8111-111111111111.jpg");
        std::fs::write(&output, [0u8]).unwrap();
        let mut job = completed(&output);
        assert_eq!(
            verified_output(&root, &job).unwrap(),
            output.canonicalize().unwrap()
        );

        job.status = JobStatus::Running;
        assert!(verified_output(&root, &job).is_err());
        job.status = JobStatus::Succeeded;
        job.output_path = Some(root.join("missing.jpg").to_string_lossy().into_owned());
        assert!(verified_output(&root, &job).is_err());

        let outside = directory.path().join("outside.jpg");
        std::fs::write(&outside, [0u8]).unwrap();
        job.output_path = Some(outside.to_string_lossy().into_owned());
        assert!(verified_output(&root, &job).is_err());
        job.output_path = Some(root.to_string_lossy().into_owned());
        assert!(verified_output(&root, &job).is_err());
    }
}
