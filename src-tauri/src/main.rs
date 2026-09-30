#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod lifecycle;

use lifecycle::DesktopLifecycle;

use geod_runtime::{
    AddProjectScenesRequest, CreateJobRequest, CreateProjectRequest, Job, JobManager, JobStatus,
    Project, ProjectDownloads, ProxySettings, ProxyTest, RasterInspection, RasterPixel,
    RasterRecipe, RecipePlan, RuntimeHealth, SavedRecipe,
};
use std::path::{Path, PathBuf};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, RunEvent, State, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_opener::OpenerExt;

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
                | Some("registry.opendata.aws")
        )
}

fn open_source_url(app: &tauri::AppHandle, url: &tauri::Url) -> Result<(), String> {
    if !allowed_source(url) {
        return Err(
            "Only official Earth Search, Sentinel COG, and AWS registry HTTPS links can be opened."
                .to_owned(),
        );
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
async fn mosaic_project(
    lifecycle: State<'_, DesktopLifecycle>,
    id: String,
    asset_key: String,
    manager: State<'_, JobManager>,
) -> Result<Job, String> {
    let _command = lifecycle.enter()?;
    manager.run_project_mosaic(&id, &asset_key).await
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
) -> Result<RasterInspection, String> {
    let _command = lifecycle.enter()?;
    manager.inspect_raster(&id).await
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
    if job.status != JobStatus::Succeeded {
        return Err("Only completed outputs can be revealed.".to_owned());
    }
    let output = job
        .output_path
        .as_ref()
        .ok_or("The completed job has no output file.")?;
    let root = storage_root
        .canonicalize()
        .map_err(|error| format!("The job storage directory is unavailable: {error}"))?;
    let path = Path::new(output)
        .canonicalize()
        .map_err(|error| format!("The output file is unavailable: {error}"))?;
    if !path.starts_with(&root) || !path.is_file() {
        return Err("The output is not a file in this application's job storage.".to_owned());
    }
    Ok(path)
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
        // Claim the application instance before opening its exclusive runtime store.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            restore_window(app);
        }))
        .plugin(
            tauri_plugin_opener::Builder::new()
                .open_js_links_on_click(false)
                .build(),
        )
        .setup(|app| {
            app.manage(DesktopLifecycle::default());
            let storage_root = app.path().app_local_data_dir()?.join("runtime");
            let manager = tauri::async_runtime::block_on(JobManager::open(storage_root))
                .map_err(std::io::Error::other)?;
            app.manage(manager);
            // Install native state before the webview can send its first IPC call.
            create_tray(app)?;
            let config = app.config().app.windows[0].clone();
            let navigation_app = app.handle().clone();
            let new_window_app = app.handle().clone();
            WebviewWindowBuilder::from_config(app, &config)?
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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            set_desktop_locale,
            health,
            diagnostics,
            get_proxy_settings,
            save_proxy_settings,
            test_proxy_settings,
            list_jobs,
            list_projects,
            create_project,
            rename_project,
            add_project_scenes,
            download_project,
            mosaic_project,
            create_job,
            cancel_job,
            retry_job,
            inspect_raster,
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
            "https://sentinel-cogs.s3.us-west-2.amazonaws.com/sentinel-s2-l2a-cogs/item/SCL.tif",
            "https://registry.opendata.aws/sentinel-2-l2a-cogs/",
        ] {
            assert!(allowed_source(&address.parse().unwrap()));
        }
        for address in [
            "http://earth-search.aws.element84.com/",
            "https://earth-search.aws.element84.com.evil.test/",
            "https://earth-search.aws.element84.com:444/",
            "https://user@earth-search.aws.element84.com/",
            "https://registry.opendata.aws@evil.test/",
            "file:///C:/Windows/win.ini",
            "javascript:alert(1)",
        ] {
            assert!(!allowed_source(&address.parse().unwrap()));
        }
    }

    fn completed(path: &Path) -> Job {
        Job {
            id: "test-job".into(),
            kind: "download".into(),
            parent_id: None,
            recipe: None,
            crop: None,
            mosaic: None,
            mosaic_output: None,
            manifest_path: None,
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
        let output = root.join("test.jpg");
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
