//! Independent Global account. OAuth stays in the system browser and server.
use crate::lifecycle::DesktopLifecycle;
use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{sync::Arc, time::Duration};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::{watch, Mutex},
};
use zeroize::Zeroizing;

const PRODUCT: &str = "xyz.laogao.geod.global";
const PUBLIC_ORIGIN: &str = "https://geod-global.laogao.xyz";
const MAX_JSON: usize = 65536;
const LOGIN_TIMEOUT: u64 = 300;

fn opaque(v: &str) -> bool {
    v.len() == 43
        && v.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}
fn random() -> Result<Zeroizing<String>, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| "callback-failed")?;
    Ok(Zeroizing::new(URL_SAFE_NO_PAD.encode(bytes)))
}
fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}
fn same_state(a: &str, b: &str) -> bool {
    opaque(a) && opaque(b) && a.bytes().zip(b.bytes()).fold(0u8, |n, (x, y)| n | (x ^ y)) == 0
}
fn origin() -> Result<String, String> {
    let raw = if cfg!(debug_assertions) {
        std::env::var("GEOD_GLOBAL_DEV_AUTH_ORIGIN").unwrap_or_else(|_| PUBLIC_ORIGIN.into())
    } else {
        PUBLIC_ORIGIN.into()
    };
    validate_origin(&raw)?;
    Ok(raw)
}
fn validate_origin(raw: &str) -> Result<(), String> {
    let u = url::Url::parse(raw).map_err(|_| "Invalid Global account origin.")?;
    if !u.username().is_empty()
        || u.password().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
        || u.path() != "/"
        || (u.scheme() != "https"
            && !(cfg!(debug_assertions)
                && u.scheme() == "http"
                && u.host_str() == Some("127.0.0.1")))
    {
        return Err("Invalid Global account origin.".into());
    }
    Ok(())
}
trait Vault: Send + Sync {
    fn get(&self, origin: &str) -> Result<Option<String>, String>;
    fn set(&self, origin: &str, value: &str) -> Result<(), String>;
    fn forget(&self, origin: &str) -> Result<(), String>;
}
struct NativeVault;
#[cfg(windows)]
fn vault_entry(origin: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(
        "xyz.laogao.geod.global.account",
        &format!("{:x}", Sha256::digest(origin.as_bytes())),
    )
    .map_err(|_| "storage-unavailable".into())
}
impl Vault for NativeVault {
    fn get(&self, origin: &str) -> Result<Option<String>, String> {
        #[cfg(windows)]
        {
            match vault_entry(origin)?.get_password() {
                Ok(v) => Ok(Some(v)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(_) => Err("storage-unavailable".into()),
            }
        }
        #[cfg(not(windows))]
        {
            let _ = origin;
            Ok(None)
        }
    }
    fn set(&self, origin: &str, value: &str) -> Result<(), String> {
        #[cfg(windows)]
        {
            vault_entry(origin)?
                .set_password(value)
                .map_err(|_| "storage-unavailable".into())
        }
        #[cfg(not(windows))]
        {
            let _ = (origin, value);
            Err("storage-unavailable".into())
        }
    }
    fn forget(&self, origin: &str) -> Result<(), String> {
        #[cfg(windows)]
        {
            match vault_entry(origin)?.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(_) => Err("storage-unavailable".into()),
            }
        }
        #[cfg(not(windows))]
        {
            let _ = origin;
            Ok(())
        }
    }
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct User {
    id: String,
    provider: String,
    name: String,
    email: Option<String>,
    email_verified: bool,
    avatar: Option<String>,
    expires_at: i64,
}
impl User {
    fn valid(&self) -> bool {
        self.id.len() == 32
            && self.id.bytes().all(|b| b.is_ascii_hexdigit())
            && ["email", "google", "github"].contains(&self.provider.as_str())
            && (self.provider != "email"
                || self.email_verified && self.email.as_ref().is_some_and(|s| !s.is_empty()))
            && self.name.len() <= 500
            && self
                .email
                .as_ref()
                .is_none_or(|s| s.len() <= 254 && !s.chars().any(char::is_control))
            && self
                .avatar
                .as_ref()
                .is_none_or(|s| s == "/api/desktop-auth/avatar")
            && self.expires_at > chrono::Utc::now().timestamp()
    }
}
#[derive(Clone, Deserialize, Serialize)]
struct Provider {
    id: String,
    available: bool,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Saved {
    token: Zeroizing<String>,
    expires_at: i64,
}
struct AccountState {
    checked: bool,
    ready: bool,
    providers: Vec<Provider>,
    user: Option<User>,
    token: Option<Zeroizing<String>>,
    expires: i64,
    remembered: bool,
    error: Option<String>,
    pending: Option<watch::Sender<bool>>,
    epoch: u64,
}
#[derive(Clone)]
pub struct DesktopIdentity(Arc<Inner>);
struct Inner {
    origin: String,
    http: reqwest::Client,
    vault: Arc<dyn Vault>,
    state: Mutex<AccountState>,
}
pub struct Prepared {
    url: String,
    listener: TcpListener,
    state: Zeroizing<String>,
    verifier: Zeroizing<String>,
    transaction: String,
    redirect_uri: String,
    remember: bool,
    epoch: u64,
    cancel: watch::Receiver<bool>,
}
impl DesktopIdentity {
    pub fn open() -> Result<Self, String> {
        Self::with_vault(origin()?, Arc::new(NativeVault))
    }
    fn with_vault(origin: String, vault: Arc<dyn Vault>) -> Result<Self, String> {
        validate_origin(&origin)?;
        let mut state = AccountState {
            checked: false,
            ready: false,
            providers: vec![],
            user: None,
            token: None,
            expires: 0,
            remembered: false,
            error: None,
            pending: None,
            epoch: 0,
        };
        match vault.get(&origin) {
            Ok(Some(raw)) => {
                let raw = Zeroizing::new(raw);
                if let Ok(saved) = serde_json::from_str::<Saved>(&raw) {
                    if opaque(&saved.token) && saved.expires_at > chrono::Utc::now().timestamp() {
                        state.token = Some(saved.token);
                        state.expires = saved.expires_at;
                        state.remembered = true;
                    } else {
                        let _ = vault.forget(&origin);
                        state.error = Some("expired".into());
                    }
                } else {
                    let _ = vault.forget(&origin);
                    state.error = Some("expired".into());
                }
            }
            Ok(None) => {}
            Err(_) => state.error = Some("storage-unavailable".into()),
        }
        let mut builder = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("GeoD-Global-Desktop/0.1");
        if origin.starts_with("http://127.0.0.1:") {
            builder = builder.no_proxy();
        }
        let http = builder
            .build()
            .map_err(|_| "Account client could not start.")?;
        Ok(Self(Arc::new(Inner {
            origin: origin.trim_end_matches('/').into(),
            http,
            vault,
            state: Mutex::new(state),
        })))
    }
    async fn json(
        &self,
        method: reqwest::Method,
        path: &str,
        data: Option<Value>,
        token: Option<&str>,
    ) -> Result<Value, String> {
        let mut request = self
            .0
            .http
            .request(method, format!("{}{}", self.0.origin, path))
            .header("Accept", "application/json");
        if let Some(data) = data {
            request = request
                .header("Content-Type", "application/json")
                .body(data.to_string());
        }
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        let mut response = request.send().await.map_err(|_| "service-unavailable")?;
        match response.status().as_u16() {
            401 => return Err("expired".into()),
            404 => return Err("desktop-not-ready".into()),
            200 | 201 => {}
            _ => return Err("service-unavailable".into()),
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| "service-unavailable")? {
            if bytes.len() + chunk.len() > MAX_JSON {
                return Err("callback-failed".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| "callback-failed".into())
    }
    fn snapshot_locked(state: &AccountState) -> Value {
        let status = if state.pending.is_some() {
            "waiting"
        } else if state.user.is_some() {
            "signed-in"
        } else if state.error.as_deref() == Some("expired") {
            "expired"
        } else {
            "signed-out"
        };
        json!({"configured":true,"ready":state.ready,"status":status,"user":state.user,"providers":state.providers,
            "busy":state.pending.is_some(),"remembered":state.remembered,"error":state.error})
    }
    pub async fn snapshot(&self, refresh: bool) -> Value {
        let mut state = self.0.state.lock().await;
        if state.pending.is_some() {
            return Self::snapshot_locked(&state);
        }
        if !state.checked || refresh {
            state.checked = true;
            match self
                .json(
                    reqwest::Method::GET,
                    "/api/desktop-auth/providers",
                    None,
                    None,
                )
                .await
            {
                Ok(v) if v["product"] == PRODUCT && v["version"] == 1 => {
                    let providers = serde_json::from_value::<Vec<Provider>>(v["providers"].clone())
                        .ok()
                        .filter(|p| {
                            (2..=3).contains(&p.len())
                                && p.iter()
                                    .all(|x| ["email", "google", "github"].contains(&x.id.as_str()))
                                && p.iter()
                                    .enumerate()
                                    .all(|(i, x)| !p[..i].iter().any(|other| other.id == x.id))
                        });
                    state.ready = providers.is_some();
                    state.providers = providers.unwrap_or_default();
                    state.error = if state.ready {
                        None
                    } else {
                        Some("desktop-not-ready".into())
                    };
                }
                Ok(_) => {
                    state.ready = false;
                    state.error = Some("desktop-not-ready".into());
                }
                Err(error) => {
                    state.ready = false;
                    state.error = Some(error);
                }
            }
        }
        if state.token.is_some() && (state.user.is_none() || refresh) {
            let token = state.token.as_ref().unwrap();
            match self
                .json(
                    reqwest::Method::GET,
                    "/api/desktop-auth/me",
                    None,
                    Some(token),
                )
                .await
            {
                Ok(v) if v["product"] == PRODUCT && v["version"] == 1 => {
                    match serde_json::from_value::<User>(v["user"].clone()) {
                        Ok(mut user) if user.valid() => {
                            user.avatar = self.avatar(token, user.avatar.is_some()).await;
                            state.expires = user.expires_at;
                            state.user = Some(user);
                            state.error = None;
                        }
                        _ => {
                            state.error = Some("expired".into());
                            state.user = None;
                            state.token = None;
                            state.remembered = false;
                            let _ = self.0.vault.forget(&self.0.origin);
                        }
                    }
                }
                Err(error) => {
                    state.error = Some(error.clone());
                    state.user = None;
                    if error == "expired" {
                        state.token = None;
                        state.remembered = false;
                        let _ = self.0.vault.forget(&self.0.origin);
                    }
                }
                _ => state.error = Some("callback-failed".into()),
            }
        }
        if state.token.is_some() && state.expires <= chrono::Utc::now().timestamp() {
            state.token = None;
            state.user = None;
            state.remembered = false;
            state.error = Some("expired".into());
            let _ = self.0.vault.forget(&self.0.origin);
        }
        Self::snapshot_locked(&state)
    }
    async fn avatar(&self, token: &str, present: bool) -> Option<String> {
        if !present {
            return None;
        }
        let mut response = self
            .0
            .http
            .get(format!("{}/api/desktop-auth/avatar", self.0.origin))
            .bearer_auth(token)
            .send()
            .await
            .ok()?;
        if !response.status().is_success() {
            return None;
        }
        let kind = response
            .headers()
            .get("content-type")?
            .to_str()
            .ok()?
            .split(';')
            .next()?
            .to_owned();
        let format = match kind.as_str() {
            "image/png" => image::ImageFormat::Png,
            "image/jpeg" => image::ImageFormat::Jpeg,
            "image/webp" => image::ImageFormat::WebP,
            _ => return None,
        };
        let mut data = Vec::new();
        while let Some(chunk) = response.chunk().await.ok()? {
            if data.len() + chunk.len() > 512 * 1024 {
                return None;
            }
            data.extend_from_slice(&chunk);
        }
        let mut reader = image::ImageReader::with_format(std::io::Cursor::new(&data), format);
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(512);
        limits.max_image_height = Some(512);
        limits.max_alloc = Some(4 * 1024 * 1024);
        reader.limits(limits);
        reader.decode().ok()?;
        Some(format!("data:{};base64,{}", kind, STANDARD.encode(data)))
    }
    pub async fn prepare(
        &self,
        provider: String,
        locale: String,
        remember: bool,
    ) -> Result<Prepared, String> {
        if !["email", "google", "github"].contains(&provider.as_str())
            || !["en", "zh-CN"].contains(&locale.as_str())
        {
            return Err("Invalid account login request.".into());
        }
        let mut state = self.0.state.lock().await;
        if state.pending.is_some() {
            return Err("Account sign-in is already pending.".into());
        }
        if state.user.is_some() {
            return Err("Sign out before signing in with another account.".into());
        }
        if !state.ready
            || !state
                .providers
                .iter()
                .any(|p| p.id == provider && p.available)
        {
            return Err("Account service is unavailable. Try again.".into());
        }
        if remember {
            // Probe without retaining account secrets or destroying an old valid session.
            if self.0.vault.get(&self.0.origin).is_err() {
                return Err("Secure account storage is unavailable. Clear the checkbox to sign in for this session.".into());
            }
        }
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|_| "The sign-in callback could not be verified. Start a new sign-in.")?;
        let redirect_uri = format!(
            "http://127.0.0.1:{}/auth/callback",
            listener.local_addr().map_err(|_| "callback-failed")?.port()
        );
        let client_state = random()?;
        let verifier = random()?;
        let value=self.json(reqwest::Method::POST,"/api/desktop-auth/transactions",Some(json!({
            "provider":provider,"locale":locale,"redirectUri":redirect_uri,"state":client_state.as_str(),
            "codeChallenge":challenge(&verifier),"codeChallengeMethod":"S256"
        })),None).await.map_err(|_| "Account service is unavailable. Try again.")?;
        let transaction = value["transactionId"]
            .as_str()
            .filter(|s| opaque(s))
            .ok_or("Invalid desktop sign-in response.")?
            .to_owned();
        let url = value["authorizeUrl"]
            .as_str()
            .ok_or("Invalid desktop sign-in response.")?
            .to_owned();
        let parsed = url::Url::parse(&url).map_err(|_| "Invalid desktop sign-in response.")?;
        let base =
            url::Url::parse(&self.0.origin).map_err(|_| "Invalid desktop sign-in response.")?;
        let query = parsed.query_pairs().collect::<Vec<_>>();
        if value["product"] != PRODUCT
            || value["version"] != 1
            || parsed.origin() != base.origin()
            || parsed.path() != "/api/desktop-auth/authorize"
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.fragment().is_some()
            || query.len() != 1
            || query[0].0 != "transaction"
            || query[0].1 != transaction
            || value["expiresAt"].as_i64().is_none_or(|n| {
                n <= chrono::Utc::now().timestamp()
                    || n > chrono::Utc::now().timestamp() + LOGIN_TIMEOUT as i64 + 10
            })
        {
            return Err("Invalid desktop sign-in response.".into());
        }
        state.epoch += 1;
        state.error = None;
        let epoch = state.epoch;
        let (cancel, receiver) = watch::channel(false);
        state.pending = Some(cancel);
        Ok(Prepared {
            url,
            listener,
            state: client_state,
            verifier,
            transaction,
            redirect_uri,
            remember,
            epoch,
            cancel: receiver,
        })
    }
    pub fn start(&self, prepared: Prepared) {
        let identity = self.clone();
        tauri::async_runtime::spawn(async move {
            let epoch = prepared.epoch;
            let result = identity.flow(prepared).await;
            if let Err(error) = result {
                let mut state = identity.0.state.lock().await;
                if state.epoch == epoch && state.pending.is_some() {
                    state.pending = None;
                    state.error = Some(error);
                }
            }
        });
    }
    async fn flow(&self, mut flow: Prepared) -> Result<(), String> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(LOGIN_TIMEOUT);
        let code = loop {
            let (mut socket, _) = tokio::select! {
                result=flow.listener.accept()=>result.map_err(|_| "callback-failed")?,
                _=flow.cancel.changed()=>return Ok(()),
                _=tokio::time::sleep_until(deadline)=>return Err("timeout".into()),
            };
            let mut header = Vec::new();
            let read = async {
                loop {
                    let mut bytes = [0u8; 1024];
                    let n = socket.read(&mut bytes).await.map_err(|_| ())?;
                    if n == 0 || header.len() + n > 8192 {
                        return Err(());
                    }
                    header.extend_from_slice(&bytes[..n]);
                    if header.windows(4).any(|s| s == b"\r\n\r\n") {
                        return Ok(());
                    }
                }
            };
            let parsed = if tokio::time::timeout(Duration::from_secs(2), read)
                .await
                .is_ok_and(|v| v.is_ok())
            {
                parse_callback(&header, &flow.redirect_uri, &flow.state)
            } else {
                Err("callback-failed".into())
            };
            let valid = parsed.is_ok() || parsed.as_ref().err().is_some_and(|s| s == "denied");
            let (status, message) = if valid {
                (
                    "200 OK",
                    "Return to GeoD Global. Sign-in is being verified.",
                )
            } else {
                (
                    "400 Bad Request",
                    "This login request could not be verified.",
                )
            };
            let response=format!("HTTP/1.1 {}\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",status,message.len(),message);
            let _ = tokio::time::timeout(
                Duration::from_secs(2),
                socket.write_all(response.as_bytes()),
            )
            .await;
            if valid {
                break parsed?;
            }
        };
        let value=self.json(reqwest::Method::POST,"/api/desktop-auth/exchange",Some(json!({
            "transactionId":flow.transaction,"code":code,"verifier":flow.verifier.as_str(),"redirectUri":flow.redirect_uri
        })),None).await?;
        let token = Zeroizing::new(
            value["accessToken"]
                .as_str()
                .filter(|s| opaque(s))
                .ok_or("callback-failed")?
                .to_owned(),
        );
        let mut user =
            serde_json::from_value::<User>(value["user"].clone()).map_err(|_| "callback-failed")?;
        if value["product"] != PRODUCT
            || value["version"] != 1
            || !user.valid()
            || value["expiresAt"].as_i64() != Some(user.expires_at)
        {
            return Err("callback-failed".into());
        }
        user.avatar = self.avatar(&token, user.avatar.is_some()).await;
        let mut state = self.0.state.lock().await;
        if state.epoch != flow.epoch || state.pending.is_none() {
            drop(state);
            let _ = self.revoke(&token).await;
            return Ok(());
        }
        if flow.remember {
            let secret = Zeroizing::new(
                serde_json::to_string(&Saved {
                    token: token.clone(),
                    expires_at: user.expires_at,
                })
                .map_err(|_| "storage-unavailable")?,
            );
            if self.0.vault.set(&self.0.origin, &secret).is_err() {
                drop(state);
                let _ = self.revoke(&token).await;
                return Err("storage-unavailable".into());
            }
        } else if self.0.vault.forget(&self.0.origin).is_err() {
            drop(state);
            let _ = self.revoke(&token).await;
            return Err("storage-unavailable".into());
        }
        if let Some(old) = state.token.take() {
            let _ = self.revoke(&old).await;
        }
        state.expires = user.expires_at;
        state.token = Some(token);
        state.user = Some(user);
        state.remembered = flow.remember;
        state.pending = None;
        state.error = None;
        Ok(())
    }
    pub async fn cancel(&self, error: Option<&str>) -> Value {
        let mut state = self.0.state.lock().await;
        state.epoch += 1;
        if let Some(cancel) = state.pending.take() {
            let _ = cancel.send(true);
        }
        state.error = error.map(str::to_owned);
        Self::snapshot_locked(&state)
    }
    async fn revoke(&self, token: &str) -> Result<(), String> {
        let result = self
            .json(
                reqwest::Method::POST,
                "/api/desktop-auth/logout",
                Some(json!({})),
                Some(token),
            )
            .await?;
        if result["ok"] != true {
            return Err("service-unavailable".into());
        }
        Ok(())
    }
    pub async fn logout(&self) -> Result<Value, String> {
        let mut state = self.0.state.lock().await;
        self.0
            .vault
            .forget(&self.0.origin)
            .map_err(|_| "Secure account storage could not be cleared.")?;
        state.epoch += 1;
        if let Some(cancel) = state.pending.take() {
            let _ = cancel.send(true);
        }
        let revoked = if let Some(token) = state.token.take() {
            self.revoke(&token).await.is_ok()
        } else {
            true
        };
        state.user = None;
        state.remembered = false;
        state.expires = 0;
        state.error = if revoked {
            None
        } else {
            Some("logout-not-confirmed".into())
        };
        Ok(Self::snapshot_locked(&state))
    }
}
fn parse_callback(header: &[u8], redirect: &str, state: &str) -> Result<String, String> {
    let text = std::str::from_utf8(header).map_err(|_| "callback-failed")?;
    let mut lines = text.split("\r\n");
    let mut request = lines.next().unwrap_or("").split(' ');
    if request.next() != Some("GET") {
        return Err("callback-failed".into());
    }
    let target = request.next().ok_or("callback-failed")?;
    if !target.starts_with("/auth/callback?")
        || request.next() != Some("HTTP/1.1")
        || request.next().is_some()
    {
        return Err("callback-failed".into());
    }
    let base = url::Url::parse(redirect).map_err(|_| "callback-failed")?;
    let hosts = lines
        .filter_map(|s| s.split_once(':'))
        .filter(|(k, _)| k.eq_ignore_ascii_case("host"))
        .map(|(_, v)| v.trim())
        .collect::<Vec<_>>();
    let expected = format!("127.0.0.1:{}", base.port().ok_or("callback-failed")?);
    if hosts.len() != 1 || hosts[0] != expected {
        return Err("callback-failed".into());
    }
    let url = base.join(target).map_err(|_| "callback-failed")?;
    let query = url.query_pairs().collect::<Vec<_>>();
    if query.len() != 2 || url.path() != "/auth/callback" || url.fragment().is_some() {
        return Err("callback-failed".into());
    }
    let values = query
        .iter()
        .filter(|(k, _)| k == "state")
        .collect::<Vec<_>>();
    if values.len() != 1 || !same_state(&values[0].1, state) {
        return Err("callback-failed".into());
    }
    if query
        .iter()
        .any(|(k, v)| k == "error" && v == "access_denied")
    {
        return Err("denied".into());
    }
    query
        .iter()
        .find(|(k, v)| k == "code" && opaque(v))
        .map(|(_, v)| v.to_string())
        .ok_or("callback-failed".into())
}
#[tauri::command]
pub async fn identity_snapshot(
    lifecycle: State<'_, DesktopLifecycle>,
    identity: State<'_, DesktopIdentity>,
    refresh: Option<bool>,
) -> Result<Value, String> {
    let _guard = lifecycle.enter()?;
    Ok(identity.snapshot(refresh.unwrap_or(false)).await)
}
#[tauri::command]
pub async fn identity_begin(
    lifecycle: State<'_, DesktopLifecycle>,
    identity: State<'_, DesktopIdentity>,
    app: AppHandle,
    provider: String,
    locale: String,
    remember: bool,
) -> Result<Value, String> {
    let _guard = lifecycle.enter()?;
    let prepared = identity.prepare(provider, locale, remember).await?;
    if app.opener().open_url(&prepared.url, None::<&str>).is_err() {
        return Ok(identity.cancel(Some("browser-failed")).await);
    }
    identity.start(prepared);
    Ok(identity.snapshot(false).await)
}
#[tauri::command]
pub async fn identity_cancel(
    lifecycle: State<'_, DesktopLifecycle>,
    identity: State<'_, DesktopIdentity>,
) -> Result<Value, String> {
    let _guard = lifecycle.enter()?;
    Ok(identity.cancel(None).await)
}
#[tauri::command]
pub async fn identity_logout(
    lifecycle: State<'_, DesktopLifecycle>,
    identity: State<'_, DesktopIdentity>,
) -> Result<Value, String> {
    let _guard = lifecycle.enter()?;
    identity.logout().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Mutex as StdMutex,
    };
    use tokio::sync::Notify;

    #[derive(Default)]
    struct MemoryVault {
        value: StdMutex<Option<String>>,
        broken: AtomicBool,
    }
    impl Vault for MemoryVault {
        fn get(&self, _: &str) -> Result<Option<String>, String> {
            if self.broken.load(Ordering::SeqCst) {
                return Err("storage-unavailable".into());
            }
            Ok(self.value.lock().unwrap().clone())
        }
        fn set(&self, _: &str, value: &str) -> Result<(), String> {
            if self.broken.load(Ordering::SeqCst) {
                return Err("storage-unavailable".into());
            }
            *self.value.lock().unwrap() = Some(value.into());
            Ok(())
        }
        fn forget(&self, _: &str) -> Result<(), String> {
            if self.broken.load(Ordering::SeqCst) {
                return Err("storage-unavailable".into());
            }
            *self.value.lock().unwrap() = None;
            Ok(())
        }
    }
    struct Fixture {
        origin: String,
        requests: Arc<StdMutex<Vec<(String, Value, String)>>>,
        signed_in: Arc<AtomicBool>,
        unavailable: Arc<AtomicBool>,
        blocked: Arc<AtomicBool>,
        exchange_started: Arc<Notify>,
        release_exchange: Arc<Notify>,
        task: tokio::task::JoinHandle<()>,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            self.task.abort();
        }
    }
    fn user() -> Value {
        json!({"id":"a".repeat(32),"provider":"google","name":"Synthetic Global Account","email":"fixture@example.invalid",
            "emailVerified":true,"avatar":null,"expiresAt":chrono::Utc::now().timestamp()+3600})
    }
    impl Fixture {
        async fn open() -> Self {
            let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
            let requests = Arc::new(StdMutex::new(Vec::new()));
            let signed_in = Arc::new(AtomicBool::new(false));
            let unavailable = Arc::new(AtomicBool::new(false));
            let blocked = Arc::new(AtomicBool::new(false));
            let exchange_started = Arc::new(Notify::new());
            let release_exchange = Arc::new(Notify::new());
            let (log, active, absent, hold, started, release, base) = (
                requests.clone(),
                signed_in.clone(),
                unavailable.clone(),
                blocked.clone(),
                exchange_started.clone(),
                release_exchange.clone(),
                origin.clone(),
            );
            let task = tokio::spawn(async move {
                while let Ok((mut stream, _)) = listener.accept().await {
                    let (log, active, absent, hold, started, release, base) = (
                        log.clone(),
                        active.clone(),
                        absent.clone(),
                        hold.clone(),
                        started.clone(),
                        release.clone(),
                        base.clone(),
                    );
                    tokio::spawn(async move {
                        let mut bytes = Vec::new();
                        let end = loop {
                            let mut buffer = [0u8; 2048];
                            let n = stream.read(&mut buffer).await.unwrap();
                            if n == 0 {
                                return;
                            }
                            bytes.extend_from_slice(&buffer[..n]);
                            if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                                break end + 4;
                            }
                        };
                        let header = String::from_utf8(bytes[..end].to_vec()).unwrap();
                        let length = header
                            .lines()
                            .filter_map(|v| v.split_once(':'))
                            .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                            .map(|(_, v)| v.trim().parse::<usize>().unwrap())
                            .unwrap_or(0);
                        while bytes.len() < end + length {
                            let mut buffer = [0u8; 2048];
                            let n = stream.read(&mut buffer).await.unwrap();
                            if n == 0 {
                                return;
                            }
                            bytes.extend_from_slice(&buffer[..n]);
                        }
                        let path = header
                            .lines()
                            .next()
                            .unwrap()
                            .split_whitespace()
                            .nth(1)
                            .unwrap()
                            .to_owned();
                        let data = serde_json::from_slice::<Value>(&bytes[end..end + length])
                            .unwrap_or(Value::Null);
                        log.lock()
                            .unwrap()
                            .push((path.clone(), data, header.clone()));
                        let (status, value) = match path.as_str() {
                            "/api/desktop-auth/providers" if absent.load(Ordering::SeqCst) => {
                                ("404 Not Found", json!({"error":"not_ready"}))
                            }
                            "/api/desktop-auth/providers" => (
                                "200 OK",
                                json!({"product":PRODUCT,"version":1,"providers":[{"id":"email","available":true},{"id":"google","available":true},{"id":"github","available":true}]}),
                            ),
                            "/api/desktop-auth/transactions" => (
                                "201 Created",
                                json!({"product":PRODUCT,"version":1,"transactionId":"t".repeat(43),"authorizeUrl":format!("{}/api/desktop-auth/authorize?transaction={}",base,"t".repeat(43)),"expiresAt":chrono::Utc::now().timestamp()+300}),
                            ),
                            "/api/desktop-auth/exchange" => {
                                active.store(true, Ordering::SeqCst);
                                started.notify_one();
                                if hold.load(Ordering::SeqCst) {
                                    release.notified().await;
                                }
                                let mut account = user();
                                account["provider"] = log
                                    .lock()
                                    .unwrap()
                                    .iter()
                                    .rev()
                                    .find(|(path, _, _)| path == "/api/desktop-auth/transactions")
                                    .map(|(_, data, _)| data["provider"].clone())
                                    .unwrap();
                                (
                                    "200 OK",
                                    json!({"product":PRODUCT,"version":1,"accessToken":"s".repeat(43),"expiresAt":account["expiresAt"],"user":account}),
                                )
                            }
                            "/api/desktop-auth/me"
                                if active.load(Ordering::SeqCst)
                                    && header.contains(&format!("Bearer {}", "s".repeat(43))) =>
                            {
                                let mut account = user();
                                account["provider"] = log
                                    .lock()
                                    .unwrap()
                                    .iter()
                                    .rev()
                                    .find(|(path, _, _)| path == "/api/desktop-auth/transactions")
                                    .map(|(_, data, _)| data["provider"].clone())
                                    .unwrap();
                                (
                                    "200 OK",
                                    json!({"product":PRODUCT,"version":1,"user":account}),
                                )
                            }
                            "/api/desktop-auth/me" => ("401 Unauthorized", json!({"user":null})),
                            "/api/desktop-auth/logout" => {
                                active.store(false, Ordering::SeqCst);
                                ("200 OK", json!({"ok":true}))
                            }
                            _ => ("404 Not Found", json!({})),
                        };
                        let body = value.to_string();
                        let response=format!("HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",status,body.len(),body);
                        let _ = stream.write_all(response.as_bytes()).await;
                    });
                }
            });
            Self {
                origin,
                requests,
                signed_in,
                unavailable,
                blocked,
                exchange_started,
                release_exchange,
                task,
            }
        }
        fn identity(&self, vault: Arc<MemoryVault>) -> DesktopIdentity {
            DesktopIdentity::with_vault(self.origin.clone(), vault).unwrap()
        }
        async fn login(&self, identity: &DesktopIdentity, remember: bool) -> Result<(), String> {
            self.login_as(identity, remember, "google").await
        }
        async fn login_as(
            &self,
            identity: &DesktopIdentity,
            remember: bool,
            provider: &str,
        ) -> Result<(), String> {
            assert_eq!(identity.snapshot(false).await["ready"], true);
            let flow = identity
                .prepare(provider.into(), "en".into(), remember)
                .await
                .unwrap();
            let url = format!(
                "{}?state={}&code={}",
                flow.redirect_uri,
                flow.state.as_str(),
                "c".repeat(43)
            );
            let copy = identity.clone();
            let task = tokio::spawn(async move { copy.flow(flow).await });
            assert_eq!(identity.0.http.get(url).send().await.unwrap().status(), 200);
            task.await.unwrap()
        }
    }

    #[tokio::test]
    async fn email_uses_verified_native_handoff_and_secure_session_restore() {
        let server = Fixture::open().await;
        let vault = Arc::new(MemoryVault::default());
        let identity = server.identity(vault.clone());
        server.login_as(&identity, true, "email").await.unwrap();
        let snapshot = identity.snapshot(false).await;
        assert_eq!(snapshot["status"], "signed-in");
        assert_eq!(snapshot["user"]["provider"], "email");
        assert_eq!(snapshot["user"]["emailVerified"], true);
        assert!(snapshot.get("accessToken").is_none());
        assert!(!snapshot.to_string().contains(&"s".repeat(43)));
        let restored = server.identity(vault);
        assert_eq!(restored.snapshot(false).await["user"]["provider"], "email");
        let mut invalid: User = serde_json::from_value(user()).unwrap();
        invalid.provider = "email".into();
        invalid.email_verified = false;
        assert!(!invalid.valid());
        identity.logout().await.unwrap();
    }

    #[test]
    fn callback_requires_state_and_exact_host_path_method_and_fields() {
        let state = "k".repeat(43);
        let code = "c".repeat(43);
        let redirect = "http://127.0.0.1:12345/auth/callback";
        let request =
            |target: &str, host: &str| format!("GET {} HTTP/1.1\r\nHost: {}\r\n\r\n", target, host);
        let target = format!("/auth/callback?code={}&state={}", code, state);
        assert_eq!(
            parse_callback(
                request(&target, "127.0.0.1:12345").as_bytes(),
                redirect,
                &state
            )
            .unwrap(),
            code
        );
        for invalid in [
            target.replace(&state, &"x".repeat(43)),
            format!("{}&state={}", target, state),
            format!("{}&extra=1", target),
            target.replace("/auth/callback", "/other"),
            format!("{}#fragment", target),
        ] {
            assert!(parse_callback(
                request(&invalid, "127.0.0.1:12345").as_bytes(),
                redirect,
                &state
            )
            .is_err());
        }
        assert!(parse_callback(
            request(&target, "attacker.invalid").as_bytes(),
            redirect,
            &state
        )
        .is_err());
        assert!(parse_callback(
            request(&target, "127.0.0.1:12345\r\nHost: 127.0.0.1:12345").as_bytes(),
            redirect,
            &state
        )
        .is_err());
        assert!(parse_callback(
            request(&target, "127.0.0.1:12345")
                .replace("GET ", "POST ")
                .as_bytes(),
            redirect,
            &state
        )
        .is_err());
        assert_eq!(
            parse_callback(
                request(
                    &format!("/auth/callback?error=access_denied&state={}", state),
                    "127.0.0.1:12345"
                )
                .as_bytes(),
                redirect,
                &state
            ),
            Err("denied".into())
        );
    }
    #[test]
    fn account_origin_excludes_credentials_paths_fragments_and_remote_plain_http() {
        assert!(validate_origin(PUBLIC_ORIGIN).is_ok());
        assert!(validate_origin("http://127.0.0.1:12345").is_ok());
        for raw in [
            "http://example.invalid",
            "https://secret@example.invalid",
            "https://example.invalid/api",
            "https://example.invalid?x=1",
            "https://example.invalid#fragment",
            "file:///C:/private",
        ] {
            assert!(validate_origin(raw).is_err());
        }
    }
    #[tokio::test]
    async fn loopback_pkce_secure_restore_and_logout_work_without_frontend_tokens() {
        let server = Fixture::open().await;
        let vault = Arc::new(MemoryVault::default());
        let identity = server.identity(vault.clone());
        server.login(&identity, true).await.unwrap();
        let snapshot = identity.snapshot(false).await;
        assert_eq!(snapshot["status"], "signed-in");
        assert_eq!(snapshot["user"]["provider"], "google");
        assert_eq!(snapshot["remembered"], true);
        assert!(!snapshot.to_string().contains(&"s".repeat(43)));
        assert!(snapshot.get("accessToken").is_none());
        let requests = server.requests.lock().unwrap().clone();
        let creation = &requests
            .iter()
            .find(|r| r.0.ends_with("/transactions"))
            .unwrap()
            .1;
        let exchange = &requests
            .iter()
            .find(|r| r.0.ends_with("/exchange"))
            .unwrap()
            .1;
        assert_eq!(creation["codeChallengeMethod"], "S256");
        assert_eq!(
            creation["codeChallenge"],
            challenge(exchange["verifier"].as_str().unwrap())
        );
        assert_eq!(creation["redirectUri"], exchange["redirectUri"]);
        drop(requests);
        let restored = server.identity(vault.clone());
        assert_eq!(restored.snapshot(false).await["status"], "signed-in");
        assert_eq!(restored.logout().await.unwrap()["status"], "signed-out");
        assert!(vault.value.lock().unwrap().is_none());
        assert!(!server.signed_in.load(Ordering::SeqCst));
    }
    #[tokio::test]
    async fn session_only_login_is_not_restored_and_expired_credentials_are_removed() {
        let server = Fixture::open().await;
        let vault = Arc::new(MemoryVault::default());
        let identity = server.identity(vault.clone());
        server.login(&identity, false).await.unwrap();
        assert_eq!(identity.snapshot(false).await["remembered"], false);
        assert!(vault.value.lock().unwrap().is_none());
        assert_eq!(
            server.identity(vault.clone()).snapshot(false).await["status"],
            "signed-out"
        );
        vault
            .set(
                &server.origin,
                &json!({"token":"s".repeat(43),"expiresAt":chrono::Utc::now().timestamp()-1})
                    .to_string(),
            )
            .unwrap();
        let expired = server.identity(vault.clone());
        assert!(vault.value.lock().unwrap().is_none());
        assert!(expired.0.state.lock().await.token.is_none());
    }
    #[tokio::test]
    async fn cancel_closes_callback_and_prevents_a_late_exchange_from_restoring_login() {
        let server = Fixture::open().await;
        server.blocked.store(true, Ordering::SeqCst);
        let vault = Arc::new(MemoryVault::default());
        let identity = server.identity(vault.clone());
        identity.snapshot(false).await;
        let flow = identity
            .prepare("google".into(), "en".into(), true)
            .await
            .unwrap();
        let url = format!(
            "{}?state={}&code={}",
            flow.redirect_uri,
            flow.state.as_str(),
            "c".repeat(43)
        );
        let copy = identity.clone();
        let task = tokio::spawn(async move { copy.flow(flow).await });
        identity.0.http.get(url).send().await.unwrap();
        server.exchange_started.notified().await;
        assert_eq!(identity.cancel(None).await["status"], "signed-out");
        server.release_exchange.notify_one();
        task.await.unwrap().unwrap();
        assert_eq!(identity.snapshot(false).await["status"], "signed-out");
        assert!(vault.value.lock().unwrap().is_none());
        assert!(!server.signed_in.load(Ordering::SeqCst));
        let pending = identity
            .prepare("google".into(), "en".into(), false)
            .await
            .unwrap();
        let callback = pending.redirect_uri.clone();
        let copy = identity.clone();
        let task = tokio::spawn(async move { copy.flow(pending).await });
        identity.cancel(None).await;
        task.await.unwrap().unwrap();
        assert!(identity.0.http.get(callback).send().await.is_err());
    }
    #[tokio::test]
    async fn unavailable_server_and_credential_store_do_not_prevent_guest_startup() {
        let server = Fixture::open().await;
        server.unavailable.store(true, Ordering::SeqCst);
        let vault = Arc::new(MemoryVault::default());
        vault.broken.store(true, Ordering::SeqCst);
        let identity = server.identity(vault.clone());
        let snapshot = identity.snapshot(false).await;
        assert_eq!(snapshot["ready"], false);
        assert_eq!(snapshot["status"], "signed-out");
        assert_eq!(snapshot["error"], "desktop-not-ready");
        server.unavailable.store(false, Ordering::SeqCst);
        identity.snapshot(true).await;
        assert!(identity
            .prepare("google".into(), "en".into(), true)
            .await
            .is_err());
        let pending = identity
            .prepare("google".into(), "en".into(), false)
            .await
            .unwrap();
        identity.cancel(None).await;
        drop(pending);
    }
    #[tokio::test]
    async fn failed_secure_save_revokes_the_new_server_session() {
        let server = Fixture::open().await;
        let vault = Arc::new(MemoryVault::default());
        let identity = server.identity(vault.clone());
        identity.snapshot(false).await;
        let flow = identity
            .prepare("google".into(), "en".into(), true)
            .await
            .unwrap();
        vault.broken.store(true, Ordering::SeqCst);
        let url = format!(
            "{}?state={}&code={}",
            flow.redirect_uri,
            flow.state.as_str(),
            "c".repeat(43)
        );
        let copy = identity.clone();
        let task = tokio::spawn(async move { copy.flow(flow).await });
        identity.0.http.get(url).send().await.unwrap();
        assert_eq!(task.await.unwrap(), Err("storage-unavailable".into()));
        assert!(identity.0.state.lock().await.user.is_none());
        assert!(!server.signed_in.load(Ordering::SeqCst));
    }
}
