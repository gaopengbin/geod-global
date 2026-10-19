//! Account sessions come from the official website in an isolated desktop webview.
//! Passwords and captchas never pass through GeoD commands, HTTP or Agent tools.
use crate::{JobManager, Result};
use futures_util::StreamExt;
use reqwest::header::COOKIE;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::time::Duration;
use zeroize::Zeroizing;

const ORIGIN: &str = "https://www.gscloud.cn";

// Verify with the publisher instead of guessing a session-cookie name.
fn account_cookies(raw: &str) -> Result<Zeroizing<String>> {
    if raw.len() > 8192 || raw.contains(['\r', '\n']) {
        return Err("Invalid GSCloud website session.".into());
    }
    let mut jar = std::collections::BTreeMap::new();
    for pair in raw.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        let (name, value) = pair
            .split_once('=')
            .ok_or("Invalid GSCloud website session.")?;
        if !(name.starts_with("gsc_") || name == "sessionid") {
            continue;
        }
        if name.len() > 64
            || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
            || value.len() > 4096
            || !value
                .bytes()
                .all(|c| c.is_ascii_graphic() && c != b';' && c != b',')
        {
            return Err("Invalid GSCloud website session.".into());
        }
        if !value.is_empty() {
            jar.insert(name, value);
        }
    }
    if !jar.keys().any(|k| *k != "gsc_csrftoken") {
        return Err("Complete sign-in in the GSCloud website window first.".into());
    }
    Ok(Zeroizing::new(
        jar.into_iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; "),
    ))
}

fn authenticated_header(html: &str) -> bool {
    let Some(start) = html.find("class=\"top3-login\"") else {
        return false;
    };
    // Logout strings in shared scripts or the footer are not login evidence.
    let menu = &html[start..];
    let mut end = menu
        .find("<div class=\"modal")
        .unwrap_or(menu.len())
        .min(8192);
    while !menu.is_char_boundary(end) {
        end -= 1;
    }
    let menu = &menu[..end];
    menu.contains("user-name")
        && !menu.contains("user-name-login")
        && (menu.contains("退出") || menu.contains("/userview/"))
        && !menu.contains("id=\"login-form\"")
}

fn delivery_url(html: &str, product: &str, item: &str) -> Result<Zeroizing<String>> {
    if !super::valid_id(product) || !super::valid_id(item) {
        return Err("Invalid GSCloud original identity.".into());
    }
    let body = html
        .split_once("var download = function")
        .and_then(|(_, s)| s.split_once("};").map(|(s, _)| s))
        .ok_or("GSCloud original delivery has changed; update the adapter.")?;
    let literals = body.split('"').skip(1).step_by(2).collect::<Vec<_>>();
    if literals.len() != 3
        || literals[0] != "https://bjdl.gscloud.cn/sources/download/"
        || literals[1] != "/"
        || !literals[2].starts_with('?')
    {
        return Err("GSCloud original delivery has changed; update the adapter.".into());
    }
    let pairs = url::form_urlencoded::parse(&literals[2].as_bytes()[1..]).collect::<Vec<_>>();
    if pairs.len() != 2
        || pairs[0].0 != "sid"
        || pairs[1].0 != "uid"
        || pairs.iter().any(|(_, v)| {
            v.is_empty()
                || v.len() > 180
                || !v
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        })
    {
        return Err(
            "GSCloud website did not supply a valid delivery session. Sign in again.".into(),
        );
    }
    Ok(Zeroizing::new(format!(
        "{}{product}/{item}{}",
        literals[0], literals[2]
    )))
}

/// Short-lived sid/uid never leave this native request or enter a persisted job.
pub(super) async fn original_response(
    root: &std::path::Path,
    client: &reqwest::Client,
    product: &str,
    item: &str,
) -> Result<reqwest::Response> {
    let cookies = stored(root)?.ok_or("Connect your GSCloud account in the desktop app first.")?;
    let response = client
        .get(format!("{ORIGIN}/userview/mycollect/data_collect/can_dl"))
        .header(COOKIE, cookies.as_str())
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|_| "GSCloud delivery session connection failed.")?;
    if response.status().as_u16() != 200 {
        return Err("GSCloud session expired or delivery access was denied. Sign in again.".into());
    }
    let mut html = Zeroizing::new(Vec::new());
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "GSCloud delivery session transfer failed.")?;
        if html.len() + chunk.len() > 512 * 1024 {
            return Err("GSCloud delivery page exceeds the size limit.".into());
        }
        html.extend_from_slice(&chunk);
    }
    let html = std::str::from_utf8(&html).map_err(|_| "Invalid GSCloud delivery page.")?;
    if !authenticated_header(html) {
        return Err("GSCloud session expired or delivery access was denied. Sign in again.".into());
    }
    let url = delivery_url(html, product, item)?;
    // Keep the transfer client's inactivity timeout. A total page deadline
    // would truncate healthy downloads on slower routes.
    client
        .get(url.as_str())
        .header("Referer", format!("{ORIGIN}/"))
        .send()
        .await
        .map_err(|_| "GSCloud original connection failed.".to_owned())
}

#[cfg(windows)]
fn vault(root: &std::path::Path) -> Result<keyring::Entry> {
    let scope = format!(
        "GeoD-Global-GSCloud-{:x}",
        Sha256::digest(root.to_string_lossy().as_bytes())
    );
    keyring::Entry::new(&scope, "session")
        .map_err(|_| "Windows could not open the GSCloud credential vault.".into())
}
#[cfg(windows)]
fn stored(root: &std::path::Path) -> Result<Option<Zeroizing<String>>> {
    match vault(root)?.get_password() {
        Ok(v) => Ok(Some(Zeroizing::new(v))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err("Windows could not read the GSCloud session.".into()),
    }
}
#[cfg(not(windows))]
fn stored(_: &std::path::Path) -> Result<Option<Zeroizing<String>>> {
    Ok(None)
}
#[cfg(windows)]
fn save(root: &std::path::Path, cookies: &str) -> Result<()> {
    vault(root)?
        .set_password(cookies)
        .map_err(|_| "Windows could not save the GSCloud session.".into())
}
#[cfg(not(windows))]
fn save(_: &std::path::Path, _: &str) -> Result<()> {
    Err("GSCloud secure account storage requires the Windows desktop.".into())
}

impl JobManager {
    pub async fn gscloud_auth_status(&self) -> Result<Value> {
        let available = stored(&self.inner.root)?.is_some_and(|v| account_cookies(&v).is_ok());
        Ok(
            json!({"status":if available{"saved"}else{"disconnected"},"verified":false,"downloadVerified":false}),
        )
    }

    /// Called only by Rust desktop code reading the website's cookie jar.
    /// Never expose cookie/password arguments to a frontend, HTTP or Agent tool.
    pub async fn gscloud_verify_web_session(&self, raw: Zeroizing<String>) -> Result<Value> {
        let cookies = account_cookies(&raw)?;
        let client = crate::proxy::download_client(&self.proxy_settings().await)?;
        let response = client
            .get(format!("{ORIGIN}/"))
            .header(COOKIE, cookies.as_str())
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .map_err(|_| "GSCloud session verification connection failed.")?;
        if !response.status().is_success() {
            return Err(format!(
                "GSCloud session verification returned HTTP {}.",
                response.status().as_u16()
            ));
        }
        let mut bytes = Zeroizing::new(Vec::new());
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| "GSCloud session verification connection failed.")?;
            if bytes.len() + chunk.len() > 512 * 1024 {
                return Err("Invalid GSCloud account verification response.".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        let html = std::str::from_utf8(&bytes)
            .map_err(|_| "Invalid GSCloud account verification response.")?;
        if !authenticated_header(html) {
            return Err("GSCloud did not confirm an authenticated account. Complete website sign-in and try again.".into());
        }
        save(&self.inner.root, &cookies)?;
        Ok(json!({"status":"connected","verified":true,"downloadVerified":false}))
    }

    pub async fn gscloud_logout(&self) -> Result<Value> {
        #[cfg(windows)]
        match vault(&self.inner.root)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(_) => return Err("Windows could not remove the GSCloud session.".into()),
        }
        Ok(json!({"status":"disconnected","verified":false,"downloadVerified":false}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn website_cookie_scope_and_secret_safe_errors() {
        let cookies =
            account_cookies("tracker=discard; gsc_csrftoken=csrf; gsc_sessionid=secret").unwrap();
        assert_eq!(cookies.as_str(), "gsc_csrftoken=csrf; gsc_sessionid=secret");
        assert!(account_cookies("gsc_session_v2=opaque").is_ok());
        for raw in [
            "",
            "gsc_csrftoken=csrf",
            "gsc_sessionid=",
            "gsc_sessionid=secret\r\nHost: evil",
            "gsc_sessionid=private value",
        ] {
            let err = account_cookies(raw).unwrap_err();
            assert!(!err.contains("private") && !err.contains("secret"));
        }
    }
    #[test]
    fn delivery_templates_pin_origin_identity_and_keep_credentials_native() {
        let page="var download = function (productid, dataid) {window.open(\"https://bjdl.gscloud.cn/sources/download/\" + productid + \"/\" + dataid + \"?sid=private_session&uid=123\");};";
        let url = delivery_url(page, "305", "srtm_60_04").unwrap();
        assert!(url.starts_with("https://bjdl.gscloud.cn/sources/download/305/srtm_60_04?"));
        for bad in [
            page.replace("bjdl.gscloud.cn", "evil.example"),
            page.replace(
                "?sid=private_session&uid=123",
                "?sid=private_session&uid=123&next=evil",
            ),
            page.replace("private_session", ""),
        ] {
            let err = delivery_url(&bad, "305", "srtm_60_04").unwrap_err();
            assert!(!err.contains("private_session"));
        }
        assert!(delivery_url(page, "../accounts", "tile").is_err());
    }
    #[test]
    fn cookie_presence_or_shared_logout_scripts_do_not_prove_login() {
        let anonymous = "<div class=\"top3-login\"><p class=\"user-name user-name-login\">登录/注册</p></div><div class=\"modal\"><script>/accounts/logout 退出</script>";
        assert!(!authenticated_header(anonymous));
        assert!(!authenticated_header("<script>/accounts/logout</script>"));
        assert!(authenticated_header("<div class=\"top3-login\"><p class=\"user-name\">Account</p><a href=\"/userview/index\">用户中心</a><a>退出登录</a></div><div class=\"modal\">"));
    }
    #[tokio::test]
    async fn incomplete_sign_in_never_reaches_transport_or_saves_a_session() {
        let temp = tempfile::tempdir().unwrap();
        let manager = JobManager::open(temp.path()).await.unwrap();
        let error = manager
            .gscloud_verify_web_session(Zeroizing::new("gsc_csrftoken=csrf".into()))
            .await
            .unwrap_err();
        assert!(error.contains("Complete sign-in"));
        assert_eq!(
            manager.gscloud_auth_status().await.unwrap()["status"],
            "disconnected"
        );
    }
}

#[cfg(test)]
mod probe;
