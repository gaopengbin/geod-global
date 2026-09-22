#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use geod_runtime::{CreateJobRequest, Job, JobManager, JobStatus, RuntimeHealth};
use std::path::{Path, PathBuf};
use tauri::{Manager, State, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;

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
fn open_source(url: String, app: tauri::AppHandle) -> Result<(), String> {
    let url = tauri::Url::parse(&url).map_err(|_| "Invalid source URL.")?;
    open_source_url(&app, &url)
}

#[tauri::command]
fn health(manager: State<'_, JobManager>) -> RuntimeHealth {
    manager.health()
}

#[tauri::command]
async fn list_jobs(manager: State<'_, JobManager>) -> Result<Vec<Job>, String> {
    Ok(manager.list().await)
}

#[tauri::command]
async fn create_job(
    request: CreateJobRequest,
    manager: State<'_, JobManager>,
) -> Result<Job, String> {
    manager.create(request).await
}

#[tauri::command]
async fn cancel_job(id: String, manager: State<'_, JobManager>) -> Result<Job, String> {
    manager.cancel(&id).await
}

#[tauri::command]
async fn retry_job(id: String, manager: State<'_, JobManager>) -> Result<Job, String> {
    manager.retry(&id).await
}

fn verified_output(storage_root: &Path, job: &Job) -> Result<PathBuf, String> {
    if job.status != JobStatus::Succeeded {
        return Err("Only completed downloads can be revealed.".to_owned());
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
    id: String,
    manager: State<'_, JobManager>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let job = manager.get(&id).await.ok_or("Job not found.")?;
    let output = verified_output(manager.storage_root(), &job)?;
    app.opener()
        .reveal_item_in_dir(output)
        .map_err(|error| format!("Could not reveal the output in your file explorer: {error}"))
}

fn main() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_opener::Builder::new()
                .open_js_links_on_click(false)
                .build(),
        )
        .setup(|app| {
            let storage_root = app.path().app_local_data_dir()?.join("runtime");
            let manager = tauri::async_runtime::block_on(JobManager::open(storage_root))
                .map_err(std::io::Error::other)?;
            app.manage(manager);
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
            health,
            list_jobs,
            create_job,
            cancel_job,
            retry_job,
            reveal_job,
            open_source
        ])
        .run(tauri::generate_context!())
        .expect("GeoD Global desktop could not start");
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
