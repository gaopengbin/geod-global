use crate::lifecycle::DesktopLifecycle;
use geod_runtime::{
    gscloud::{DetailRequest, SearchRequest},
    JobManager,
};
use serde_json::json;
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};
use zeroize::Zeroizing;

const LOGIN_WINDOW: &str = "gscloud-sign-in";
const LOGIN_URL: &str = "https://www.gscloud.cn/accounts/login_user";

fn official_navigation(url: &url::Url) -> bool {
    url.scheme() == "https"
        && url.host_str() == Some("www.gscloud.cn")
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
}

fn login_navigation(url: &url::Url) -> bool {
    official_navigation(url)
        || (url.scheme() == "https"
            && matches!(
                url.host_str(),
                Some("passport.escience.cn" | "graph.qq.com")
            )
            && url.port_or_known_default() == Some(443)
            && url.username().is_empty()
            && url.password().is_none())
}

fn close_login(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(LOGIN_WINDOW) {
        window
            .close()
            .map_err(|_| "Could not close the GSCloud sign-in window.")?;
    }
    Ok(())
}
#[tauri::command]
pub async fn gscloud_products(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
) -> Result<serde_json::Value, String> {
    let _command = lifecycle.enter()?;
    manager.gscloud_products().await
}
#[tauri::command]
pub async fn gscloud_search(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: SearchRequest,
) -> Result<serde_json::Value, String> {
    let _command = lifecycle.enter()?;
    manager.gscloud_search(request).await
}
#[tauri::command]
pub async fn gscloud_detail(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: DetailRequest,
) -> Result<serde_json::Value, String> {
    let _command = lifecycle.enter()?;
    manager.gscloud_detail(request).await
}
#[tauri::command]
pub async fn gscloud_inspect_original(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    id: String,
) -> Result<serde_json::Value, String> {
    let _command = lifecycle.enter()?;
    manager.gscloud_inspect_original(&id).await
}
#[tauri::command]
pub async fn gscloud_download_original(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
    request: DetailRequest,
) -> Result<geod_runtime::Job, String> {
    let _command = lifecycle.enter()?;
    manager.gscloud_download_original(request).await
}
#[tauri::command]
pub async fn gscloud_auth_status(
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
) -> Result<serde_json::Value, String> {
    let _command = lifecycle.enter()?;
    manager.gscloud_auth_status().await
}
#[tauri::command]
pub async fn gscloud_login_begin(
    app: AppHandle,
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
) -> Result<serde_json::Value, String> {
    let _command = lifecycle.enter()?;
    if let Some(window) = app.get_webview_window(LOGIN_WINDOW) {
        window
            .show()
            .map_err(|_| "Could not show the GSCloud sign-in window.")?;
        window
            .set_focus()
            .map_err(|_| "Could not show the GSCloud sign-in window.")?;
    } else {
        // Remote website has no desktop ACL. No script or password interception,
        // shared persistent browser profile, or additional windows are allowed.
        let mut builder = WebviewWindowBuilder::new(
            &app,
            LOGIN_WINDOW,
            WebviewUrl::External(LOGIN_URL.parse().unwrap()),
        )
        .title("地理空间数据云 · 官网登录 · www.gscloud.cn")
        .inner_size(1050.0, 800.0)
        .min_inner_size(720.0, 600.0)
        .incognito(true)
        .on_navigation(login_navigation)
        .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
        .on_download(|_, _| false);
        let proxy = manager.proxy_settings().await;
        match proxy.mode {
            geod_runtime::proxy::ProxyMode::Custom => {
                let url = url::Url::parse(proxy.url.as_deref().ok_or("Missing proxy address")?)
                    .map_err(|_| "Invalid proxy address.")?;
                builder = builder.proxy_url(url);
            }
            geod_runtime::proxy::ProxyMode::Direct => {
                #[cfg(windows)]
                {
                    builder = builder.additional_browser_args("--no-proxy-server");
                }
            }
            geod_runtime::proxy::ProxyMode::System => {}
        }
        builder
            .build()
            .map_err(|_| "Could not open the GSCloud sign-in window.")?;
    }
    Ok(json!({"status":"pending","verified":false,"downloadVerified":false}))
}
#[tauri::command]
pub async fn gscloud_login_finish(
    app: AppHandle,
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
) -> Result<serde_json::Value, String> {
    let _command = lifecycle.enter()?;
    let window = app
        .get_webview_window(LOGIN_WINDOW)
        .ok_or("Open the GSCloud website sign-in window first.")?;
    if !official_navigation(
        &window
            .url()
            .map_err(|_| "Could not read the GSCloud sign-in window.")?,
    ) {
        return Err("Invalid GSCloud sign-in window origin.".into());
    }
    // WebView2 cookie queries must run away from the UI thread. Read only the
    // fixed first-party URL; cookie values never return to the local frontend.
    let cookies = tauri::async_runtime::spawn_blocking(move || {
        let cookies = window
            .cookies_for_url("https://www.gscloud.cn/".parse().unwrap())
            .map_err(|_| "Could not read the GSCloud website session.")?;
        if cookies.len() > 100 {
            return Err("Invalid GSCloud website session.");
        }
        Ok::<_, &str>(Zeroizing::new(
            cookies
                .iter()
                .filter(|c| c.name().starts_with("gsc_") || c.name() == "sessionid")
                .map(|c| format!("{}={}", c.name(), c.value()))
                .collect::<Vec<_>>()
                .join("; "),
        ))
    })
    .await
    .map_err(|_| "Could not read the GSCloud website session.")??;
    let status = manager.gscloud_verify_web_session(cookies).await?;
    close_login(&app)?;
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.set_focus();
    }
    Ok(status)
}
#[tauri::command]
pub async fn gscloud_login_cancel(
    app: AppHandle,
    lifecycle: State<'_, DesktopLifecycle>,
) -> Result<serde_json::Value, String> {
    let _command = lifecycle.enter()?;
    close_login(&app)?;
    Ok(json!({"status":"cancelled","verified":false,"downloadVerified":false}))
}
#[tauri::command]
pub async fn gscloud_logout(
    app: AppHandle,
    lifecycle: State<'_, DesktopLifecycle>,
    manager: State<'_, JobManager>,
) -> Result<serde_json::Value, String> {
    let _command = lifecycle.enter()?;
    close_login(&app)?;
    manager.gscloud_logout().await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sign_in_navigation_stays_on_the_exact_publisher_origin() {
        assert!(official_navigation(&LOGIN_URL.parse().unwrap()));
        assert!(login_navigation(
            &"https://passport.escience.cn/oauth2/authorize"
                .parse()
                .unwrap()
        ));
        assert!(!official_navigation(
            &"https://passport.escience.cn/".parse().unwrap()
        ));
        for url in [
            "http://www.gscloud.cn/",
            "https://gscloud.cn/",
            "https://www.gscloud.cn.evil/",
            "https://evil@www.gscloud.cn/",
            "https://www.gscloud.cn:444/",
            "tauri://localhost/",
        ] {
            assert!(!official_navigation(&url.parse().unwrap()));
            assert!(!login_navigation(&url.parse().unwrap()));
        }
    }
}
