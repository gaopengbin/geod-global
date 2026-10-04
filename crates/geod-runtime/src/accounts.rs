//! Native-only provider authorization. Secrets never enter job/project records,
//! HTTP routes, diagnostics or returned account state. Passwords are one-use.
use crate::{proxy, JobManager, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::Utc;
use reqwest::{header, Client, Response};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::Arc,
    time::Duration,
};
use zeroize::Zeroizing;

const CDSE_TOKEN: &str =
    "https://identity.dataspace.copernicus.eu/auth/realms/CDSE/protocol/openid-connect/token";
const CDSE_USER: &str =
    "https://identity.dataspace.copernicus.eu/auth/realms/CDSE/protocol/openid-connect/userinfo";
// CMR rejects invalid bearer tokens even for this public collection. This proves
// EDL token validity, not entitlement to every NASA product or a file download.
const NASA_VERIFY: &str =
    "https://cmr.earthdata.nasa.gov/search/granules.json?short_name=HLSL30&version=2.0&page_size=1";
const NETWORK_ERROR: &str =
    "Account service could not be reached. Check the network or source download proxy and retry.";
const REJECTED: &str = "Authorization was rejected. Check the credentials, token expiry or two-step verification code.";
const STORAGE_ERROR: &str = "Secure credential storage is unavailable. Authorization was not saved. Check Windows Credential Manager and retry.";
const NOT_CONNECTED: &str =
    "Connect this data source in Settings before downloading protected files.";

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum AccountProvider {
    #[serde(rename = "nasa-earthdata")]
    Nasa,
    #[serde(rename = "copernicus")]
    Copernicus,
}
impl AccountProvider {
    #[cfg(windows)]
    fn id(self) -> &'static str {
        match self {
            Self::Nasa => "nasa-earthdata",
            Self::Copernicus => "copernicus",
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectAccountRequest {
    pub provider: AccountProvider,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: Zeroizing<String>,
    #[serde(default)]
    pub token: Zeroizing<String>,
    #[serde(default)]
    pub totp: Zeroizing<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStatus {
    pub provider: AccountProvider,
    pub status: &'static str,
    pub expires_at: Option<String>,
    pub verified_at: Option<String>,
}

// Only NASA's user token or CDSE's refresh token is stored. Short-lived CDSE
// access tokens stay in memory; passwords/TOTP are never persisted.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredCredential {
    version: u8,
    secret: Zeroizing<String>,
    expires_at: i64,
    verified_at: String,
}
struct Session {
    token: Zeroizing<String>,
    expires_at: i64,
    status: AccountStatus,
}

trait CredentialVault: Send + Sync {
    fn get(&self, provider: AccountProvider) -> Result<Option<StoredCredential>>;
    fn set(&self, provider: AccountProvider, value: &StoredCredential) -> Result<()>;
    fn delete(&self, provider: AccountProvider) -> Result<()>;
    fn supported(&self) -> bool {
        true
    }
}

#[cfg(windows)]
struct WindowsVault {
    service: String,
}
#[cfg(windows)]
impl WindowsVault {
    fn entry(&self, provider: AccountProvider) -> Result<keyring::Entry> {
        keyring::Entry::new(&self.service, provider.id()).map_err(|_| STORAGE_ERROR.into())
    }
}
#[cfg(windows)]
impl CredentialVault for WindowsVault {
    fn get(&self, provider: AccountProvider) -> Result<Option<StoredCredential>> {
        match self.entry(provider)?.get_secret() {
            Ok(bytes) => decode_saved(&Zeroizing::new(bytes)).map(Some),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(STORAGE_ERROR.into()),
        }
    }
    fn set(&self, provider: AccountProvider, value: &StoredCredential) -> Result<()> {
        let bytes = Zeroizing::new(serde_json::to_vec(value).map_err(|_| STORAGE_ERROR)?);
        self.entry(provider)?
            .set_secret(&bytes)
            .map_err(|_| STORAGE_ERROR.into())
    }
    fn delete(&self, provider: AccountProvider) -> Result<()> {
        match self.entry(provider)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(STORAGE_ERROR.into()),
        }
    }
}
#[cfg(all(not(windows), not(test)))]
struct UnsupportedVault;
#[cfg(all(not(windows), not(test)))]
impl CredentialVault for UnsupportedVault {
    fn get(&self, _: AccountProvider) -> Result<Option<StoredCredential>> {
        Ok(None)
    }
    fn set(&self, _: AccountProvider, _: &StoredCredential) -> Result<()> {
        Err(STORAGE_ERROR.into())
    }
    fn delete(&self, _: AccountProvider) -> Result<()> {
        Err(STORAGE_ERROR.into())
    }
    fn supported(&self) -> bool {
        false
    }
}

fn decode_saved(bytes: &[u8]) -> Result<StoredCredential> {
    let value: StoredCredential = serde_json::from_slice(bytes).map_err(|_| STORAGE_ERROR)?;
    if value.version != 1
        || !valid_token(&value.secret)
        || value.expires_at <= 0
        || chrono::DateTime::parse_from_rfc3339(&value.verified_at).is_err()
    {
        return Err(STORAGE_ERROR.into());
    }
    Ok(value)
}
fn valid_token(value: &str) -> bool {
    (16..=8000).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_graphic())
}
fn timestamp(value: i64) -> Option<String> {
    chrono::DateTime::from_timestamp(value, 0).map(|date| date.to_rfc3339())
}
fn empty_status(provider: AccountProvider, status: &'static str) -> AccountStatus {
    AccountStatus {
        provider,
        status,
        expires_at: None,
        verified_at: None,
    }
}

pub(crate) struct Accounts {
    vault: Arc<dyn CredentialVault>,
    sessions: BTreeMap<AccountProvider, Session>,
    rejected: BTreeSet<AccountProvider>,
}
impl Accounts {
    pub(crate) fn new(root: &Path) -> Self {
        // Independent workspace identities also keep QA credentials out of the
        // user's real desktop workspace vault. No workstation path is stored.
        let _scope = format!(
            "GeoD-Global.{}",
            &format!("{:x}", Sha256::digest(root.to_string_lossy().as_bytes()))[..24]
        );
        #[cfg(all(windows, not(test)))]
        let vault: Arc<dyn CredentialVault> = Arc::new(WindowsVault { service: _scope });
        #[cfg(all(not(windows), not(test)))]
        let vault: Arc<dyn CredentialVault> = Arc::new(UnsupportedVault);
        #[cfg(test)]
        let vault: Arc<dyn CredentialVault> = Arc::new(tests::MemoryVault::default());
        Self {
            vault,
            sessions: BTreeMap::new(),
            rejected: BTreeSet::new(),
        }
    }
    fn statuses(&self) -> Vec<AccountStatus> {
        [AccountProvider::Nasa, AccountProvider::Copernicus]
            .into_iter()
            .map(|provider| {
                if !self.vault.supported() {
                    return empty_status(provider, "unsupported");
                }
                let saved = match self.vault.get(provider) {
                    Ok(Some(saved)) => saved,
                    Ok(None) => return empty_status(provider, "not-connected"),
                    Err(_) => return empty_status(provider, "storage-error"),
                };
                let mut status = self
                    .sessions
                    .get(&provider)
                    .map(|session| session.status.clone())
                    .unwrap_or(AccountStatus {
                        provider,
                        status: "saved",
                        expires_at: timestamp(saved.expires_at),
                        verified_at: Some(saved.verified_at.clone()),
                    });
                if self
                    .sessions
                    .get(&provider)
                    .is_some_and(|session| session.expires_at <= Utc::now().timestamp())
                    && status.status == "connected"
                {
                    status.status = "saved";
                }
                if saved.expires_at <= Utc::now().timestamp() {
                    status.status = "expired";
                }
                if self.rejected.contains(&provider) {
                    status.status = "expired";
                }
                status
            })
            .collect()
    }
    fn remember(
        &mut self,
        provider: AccountProvider,
        credential: StoredCredential,
        token: Zeroizing<String>,
        expiry: i64,
    ) -> Result<AccountStatus> {
        self.vault.set(provider, &credential)?;
        self.rejected.remove(&provider);
        let status = AccountStatus {
            provider,
            status: "connected",
            expires_at: timestamp(expiry),
            verified_at: Some(credential.verified_at.clone()),
        };
        self.sessions.insert(
            provider,
            Session {
                token,
                expires_at: expiry,
                status: status.clone(),
            },
        );
        Ok(status)
    }
    async fn connect(
        &mut self,
        request: ConnectAccountRequest,
        client: &Client,
        endpoints: &Endpoints,
    ) -> Result<AccountStatus> {
        if !self.vault.supported() {
            return Err(STORAGE_ERROR.into());
        }
        match request.provider {
            AccountProvider::Nasa => {
                if !request.username.is_empty()
                    || !request.password.is_empty()
                    || !request.totp.is_empty()
                    || !valid_token(&request.token)
                {
                    return Err("Enter an Earthdata user token without spaces. Create it on the official Earthdata Login website.".into());
                }
                let token = Zeroizing::new(request.token.trim().to_owned());
                let expiry = nasa_expiry(&token)?;
                verify_nasa(client, endpoints, &token).await?;
                self.remember(
                    request.provider,
                    StoredCredential {
                        version: 1,
                        secret: token.clone(),
                        expires_at: expiry,
                        verified_at: crate::now(),
                    },
                    token,
                    expiry,
                )
            }
            AccountProvider::Copernicus => {
                if request.username.is_empty()
                    || request.username.len() > 254
                    || request.username.chars().any(char::is_control)
                    || request.password.is_empty()
                    || request.password.len() > 1024
                    || !request.token.is_empty()
                    || (!request.totp.is_empty()
                        && (request.totp.len() != 6
                            || !request.totp.bytes().all(|b| b.is_ascii_digit())))
                {
                    return Err("Enter a Copernicus username and password; the optional two-step code must contain six digits.".into());
                }
                let mut form = vec![
                    ("client_id", "cdse-public"),
                    ("grant_type", "password"),
                    ("scope", "openid"),
                    ("username", request.username.as_str()),
                    ("password", request.password.as_str()),
                ];
                if !request.totp.is_empty() {
                    form.push(("totp", request.totp.as_str()));
                }
                let tokens = exchange_cdse(client, endpoints, &form).await?;
                verify_cdse(client, endpoints, &tokens.access_token).await?;
                self.remember(
                    request.provider,
                    StoredCredential {
                        version: 1,
                        secret: tokens.refresh_token,
                        expires_at: Utc::now().timestamp() + tokens.refresh_expires_in,
                        verified_at: crate::now(),
                    },
                    tokens.access_token,
                    Utc::now().timestamp() + tokens.expires_in,
                )
            }
        }
    }
    async fn verify(
        &mut self,
        provider: AccountProvider,
        client: &Client,
        endpoints: &Endpoints,
    ) -> Result<AccountStatus> {
        let saved = self.vault.get(provider)?.ok_or(NOT_CONNECTED)?;
        if saved.expires_at <= Utc::now().timestamp() {
            self.sessions.remove(&provider);
            return Err(REJECTED.into());
        }
        let result = match provider {
            AccountProvider::Nasa => {
                verify_nasa(client, endpoints, &saved.secret).await?;
                let token = saved.secret.clone();
                let expiry = saved.expires_at;
                self.remember(
                    provider,
                    StoredCredential {
                        verified_at: crate::now(),
                        ..saved
                    },
                    token,
                    expiry,
                )
            }
            AccountProvider::Copernicus => {
                if let Some(session) = self
                    .sessions
                    .get_mut(&provider)
                    .filter(|session| session.expires_at > Utc::now().timestamp() + 30)
                {
                    verify_cdse(client, endpoints, &session.token).await?;
                    session.status.verified_at = Some(crate::now());
                    session.status.status = "connected";
                    return Ok(session.status.clone());
                }
                // Refresh after restart and while checking access. A rotating
                // refresh token is replaced atomically by the credential store.
                let tokens = exchange_cdse(
                    client,
                    endpoints,
                    &[
                        ("client_id", "cdse-public"),
                        ("grant_type", "refresh_token"),
                        ("refresh_token", saved.secret.as_str()),
                    ],
                )
                .await?;
                verify_cdse(client, endpoints, &tokens.access_token).await?;
                self.remember(
                    provider,
                    StoredCredential {
                        version: 1,
                        secret: tokens.refresh_token,
                        expires_at: Utc::now().timestamp() + tokens.refresh_expires_in,
                        verified_at: crate::now(),
                    },
                    tokens.access_token,
                    Utc::now().timestamp() + tokens.expires_in,
                )
            }
        };
        result
    }
    async fn download_token(
        &mut self,
        provider: AccountProvider,
        client: &Client,
        endpoints: &Endpoints,
    ) -> Result<Zeroizing<String>> {
        if self.rejected.contains(&provider) {
            return Err(REJECTED.into());
        }
        // Vault state is checked even when there is a cached session, so a
        // deleted, corrupt or expired saved credential cannot grant access.
        let saved = self.vault.get(provider)?.ok_or(NOT_CONNECTED)?;
        if saved.expires_at <= Utc::now().timestamp() + 30 {
            self.sessions.remove(&provider);
            return Err(REJECTED.into());
        }
        if self
            .sessions
            .get(&provider)
            .is_none_or(|session| session.expires_at <= Utc::now().timestamp() + 30)
        {
            if let Err(error) = self.verify(provider, client, endpoints).await {
                if error == REJECTED {
                    self.rejected.insert(provider);
                    self.sessions.remove(&provider);
                }
                return Err(error);
            }
        }
        self.sessions
            .get(&provider)
            .map(|session| session.token.clone())
            .ok_or_else(|| NOT_CONNECTED.into())
    }

    fn disconnect(&mut self, provider: AccountProvider) -> Result<AccountStatus> {
        self.vault.delete(provider)?;
        self.sessions.remove(&provider);
        self.rejected.remove(&provider);
        Ok(empty_status(provider, "not-connected"))
    }
}

struct Endpoints {
    nasa_verify: String,
    cdse_token: String,
    cdse_user: String,
}
impl Default for Endpoints {
    fn default() -> Self {
        Self {
            nasa_verify: NASA_VERIFY.into(),
            cdse_token: CDSE_TOKEN.into(),
            cdse_user: CDSE_USER.into(),
        }
    }
}
pub(crate) fn bearer(token: &str) -> Result<header::HeaderValue> {
    let mut value =
        header::HeaderValue::from_str(&format!("Bearer {token}")).map_err(|_| REJECTED)?;
    value.set_sensitive(true);
    Ok(value)
}
async fn bounded_json(response: Response) -> Result<serde_json::Value> {
    let status = response.status();
    if matches!(status.as_u16(), 400 | 401 | 403) {
        return Err(REJECTED.into());
    }
    if status.as_u16() == 429 {
        return Err("Too many authorization attempts. Wait a moment before retrying.".into());
    }
    if !status.is_success() {
        return Err(NETWORK_ERROR.into());
    }
    let mut bytes = Zeroizing::new(Vec::new());
    let mut response = response;
    while let Some(chunk) = response.chunk().await.map_err(|_| NETWORK_ERROR)? {
        if bytes.len() + chunk.len() > 128 * 1024 {
            return Err("Account service returned an invalid response. Retry later.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| "Account service returned an invalid response. Retry later.".into())
}
async fn verify_nasa(client: &Client, endpoints: &Endpoints, token: &str) -> Result<()> {
    let response = client
        .get(&endpoints.nasa_verify)
        .header(header::AUTHORIZATION, bearer(token)?)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|_| NETWORK_ERROR)?;
    let value = bounded_json(response).await?;
    if !value
        .get("feed")
        .is_some_and(|feed| feed.get("entry").is_some_and(serde_json::Value::is_array))
    {
        return Err("Account service returned an invalid response. Retry later.".into());
    }
    Ok(())
}
async fn verify_cdse(client: &Client, endpoints: &Endpoints, token: &str) -> Result<()> {
    let response = client
        .get(&endpoints.cdse_user)
        .header(header::AUTHORIZATION, bearer(token)?)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|_| NETWORK_ERROR)?;
    let value = bounded_json(response).await?;
    if !value
        .get("sub")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|id| !id.is_empty() && id.len() < 512)
    {
        return Err("Account service returned an invalid response. Retry later.".into());
    }
    Ok(())
}
#[derive(Deserialize)]
struct CdseTokens {
    access_token: Zeroizing<String>,
    refresh_token: Zeroizing<String>,
    token_type: String,
    expires_in: i64,
    refresh_expires_in: i64,
}
async fn exchange_cdse(
    client: &Client,
    endpoints: &Endpoints,
    form: &[(&str, &str)],
) -> Result<CdseTokens> {
    let response = client
        .post(&endpoints.cdse_token)
        .form(form)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|_| NETWORK_ERROR)?;
    let mut value = bounded_json(response).await?;
    // Move secret JSON strings out of Value instead of retaining extra copies.
    let tokens: CdseTokens = serde_json::from_value(value.take())
        .map_err(|_| "Account service returned an invalid response. Retry later.")?;
    if !tokens.token_type.eq_ignore_ascii_case("bearer")
        || !valid_token(&tokens.access_token)
        || !valid_token(&tokens.refresh_token)
        || !(30..=86400).contains(&tokens.expires_in)
        || !(30..=366 * 86400).contains(&tokens.refresh_expires_in)
    {
        return Err("Account service returned an invalid response. Retry later.".into());
    }
    Ok(tokens)
}
fn nasa_expiry(token: &str) -> Result<i64> {
    let invalid = "The Earthdata token is invalid or expired. Create a new user token on the official website.";
    let parts: Vec<_> = token.split('.').collect();
    if parts.len() != 3 || parts.iter().any(|part| part.is_empty()) {
        return Err(invalid.into());
    }
    // Expiry is only a local rejection/display hint; acceptance ALWAYS requires
    // server verification, never trusting an unsigned local JWT decode.
    let bytes = Zeroizing::new(URL_SAFE_NO_PAD.decode(parts[1]).map_err(|_| invalid)?);
    let payload: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| invalid)?;
    let expiry = payload
        .get("exp")
        .and_then(serde_json::Value::as_i64)
        .ok_or(invalid)?;
    if expiry <= Utc::now().timestamp() + 30 || timestamp(expiry).is_none() {
        return Err(invalid.into());
    }
    Ok(expiry)
}

impl JobManager {
    // Only the native worker can read tokens. The IPC/API returns status, never
    // a token, and jobs persist only their unsigned source URL.
    pub(crate) async fn download_token(
        &self,
        provider: AccountProvider,
        client: &Client,
    ) -> Result<Zeroizing<String>> {
        let mut accounts = self.inner.accounts.lock().await;
        accounts
            .download_token(provider, client, &Endpoints::default())
            .await
    }

    pub async fn provider_accounts(&self) -> Vec<AccountStatus> {
        self.inner.accounts.lock().await.statuses()
    }
    pub async fn connect_provider_account(
        &self,
        request: ConnectAccountRequest,
    ) -> Result<AccountStatus> {
        let client = proxy::download_client(&self.proxy_settings().await)?;
        self.inner
            .accounts
            .lock()
            .await
            .connect(request, &client, &Endpoints::default())
            .await
    }
    pub async fn verify_provider_account(
        &self,
        provider: AccountProvider,
    ) -> Result<AccountStatus> {
        let client = proxy::download_client(&self.proxy_settings().await)?;
        let mut accounts = self.inner.accounts.lock().await;
        let result = accounts
            .verify(provider, &client, &Endpoints::default())
            .await;
        if result.as_ref().is_err_and(|error| error == REJECTED) {
            accounts.rejected.insert(provider);
            if let Some(session) = accounts.sessions.get_mut(&provider) {
                session.status.status = "expired";
            }
        }
        result
    }
    pub async fn disconnect_provider_account(
        &self,
        provider: AccountProvider,
    ) -> Result<AccountStatus> {
        self.inner.accounts.lock().await.disconnect(provider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    pub(super) struct MemoryVault {
        entries: Mutex<BTreeMap<AccountProvider, Vec<u8>>>,
    }
    impl CredentialVault for MemoryVault {
        fn get(&self, provider: AccountProvider) -> Result<Option<StoredCredential>> {
            self.entries
                .lock()
                .unwrap()
                .get(&provider)
                .map(|bytes| decode_saved(bytes))
                .transpose()
        }
        fn set(&self, provider: AccountProvider, value: &StoredCredential) -> Result<()> {
            self.entries
                .lock()
                .unwrap()
                .insert(provider, serde_json::to_vec(value).unwrap());
            Ok(())
        }
        fn delete(&self, provider: AccountProvider) -> Result<()> {
            self.entries.lock().unwrap().remove(&provider);
            Ok(())
        }
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "writes and removes an isolated synthetic Windows Credential Manager entry"]
    fn windows_vault_roundtrip_is_persistent_and_isolated() {
        let service = format!("GeoD-Global.QA.{}", uuid::Uuid::new_v4());
        let first = WindowsVault {
            service: service.clone(),
        };
        let credential = StoredCredential {
            version: 1,
            secret: Zeroizing::new("SYNTHETIC-NOT-A-PROVIDER-TOKEN".into()),
            expires_at: Utc::now().timestamp() + 600,
            verified_at: crate::now(),
        };
        first.set(AccountProvider::Nasa, &credential).unwrap();
        let reopened = WindowsVault { service };
        let loaded = reopened.get(AccountProvider::Nasa);
        // Always remove the synthetic credential before asserting the read.
        reopened.delete(AccountProvider::Nasa).unwrap();
        assert_eq!(
            loaded.unwrap().unwrap().secret.as_str(),
            credential.secret.as_str()
        );
        assert!(reopened.get(AccountProvider::Nasa).unwrap().is_none());
        assert!(first.get(AccountProvider::Copernicus).unwrap().is_none());
    }

    async fn identity_server() -> (Endpoints, tokio::task::JoinHandle<()>) {
        use axum::{
            http::{HeaderMap, StatusCode},
            routing::{get, post},
            Form, Json, Router,
        };
        async fn token(
            Form(form): Form<BTreeMap<String, String>>,
        ) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
            if form.get("client_id").map(String::as_str) != Some("cdse-public") {
                return Err(StatusCode::BAD_REQUEST);
            }
            let grant = form.get("grant_type").map(String::as_str);
            if grant == Some("password") {
                if form.get("password").map(String::as_str) != Some("PRIVATE-PASSWORD")
                    || form.get("totp").map(String::as_str) != Some("123456")
                {
                    return Err(StatusCode::UNAUTHORIZED);
                }
            } else if grant == Some("refresh_token") {
                if form.get("refresh_token").map(String::as_str)
                    != Some("PRIVATE-REFRESH-TOKEN-ONE")
                {
                    return Err(StatusCode::UNAUTHORIZED);
                }
                assert!(!form.contains_key("password"));
                assert!(!form.contains_key("totp"));
            } else {
                return Err(StatusCode::BAD_REQUEST);
            }
            Ok(Json(
                serde_json::json!({ "access_token": "PRIVATE-ACCESS-TOKEN", "refresh_token": if grant == Some("password") {"PRIVATE-REFRESH-TOKEN-ONE"} else {"PRIVATE-REFRESH-TOKEN-TWO"}, "token_type":"Bearer", "expires_in":300, "refresh_expires_in":1800 }),
            ))
        }
        async fn user(
            headers: HeaderMap,
        ) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
            if headers
                .get(header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                != Some("Bearer PRIVATE-ACCESS-TOKEN")
            {
                return Err(StatusCode::UNAUTHORIZED);
            }
            Ok(Json(
                serde_json::json!({"sub":"test-identity","email":"never-return-this@example.test"}),
            ))
        }
        async fn nasa(
            headers: HeaderMap,
        ) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
            let token = headers
                .get(header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("");
            if !token.ends_with(".accepted") {
                return Err(StatusCode::UNAUTHORIZED);
            }
            Ok(Json(serde_json::json!({"feed":{"entry":[]}})))
        }
        let app = Router::new()
            .route("/token", post(token))
            .route("/user", get(user))
            .route("/nasa", get(nasa))
            .route(
                "/redirect",
                post(|| async {
                    (
                        StatusCode::TEMPORARY_REDIRECT,
                        [(header::LOCATION, "http://127.0.0.1:1/credential-theft")],
                    )
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (
            Endpoints {
                nasa_verify: format!("{origin}/nasa"),
                cdse_token: format!("{origin}/token"),
                cdse_user: format!("{origin}/user"),
            },
            task,
        )
    }

    #[tokio::test]
    async fn cdse_verifies_identity_and_restores_by_refresh_without_storing_password() {
        let (endpoints, server) = identity_server().await;
        let client = proxy::download_client(&crate::ProxySettings {
            mode: crate::proxy::ProxyMode::Direct,
            url: None,
        })
        .unwrap();
        let vault = Arc::new(MemoryVault::default());
        let mut accounts = Accounts {
            vault: vault.clone(),
            sessions: BTreeMap::new(),
            rejected: BTreeSet::new(),
        };
        let request = serde_json::from_value(serde_json::json!({"provider":"copernicus","username":"tester","password":"PRIVATE-PASSWORD","totp":"123456"})).unwrap();
        let state = accounts
            .connect(request, &client, &endpoints)
            .await
            .unwrap();
        assert_eq!(state.status, "connected");
        let bytes = vault
            .entries
            .lock()
            .unwrap()
            .get(&AccountProvider::Copernicus)
            .unwrap()
            .clone();
        let saved = String::from_utf8(bytes).unwrap();
        assert!(saved.contains("PRIVATE-REFRESH-TOKEN-ONE"));
        for prohibited in [
            "PRIVATE-PASSWORD",
            "PRIVATE-ACCESS-TOKEN",
            "123456",
            "email",
        ] {
            assert!(!saved.contains(prohibited));
        }
        let mut reopened = Accounts {
            vault: vault.clone(),
            sessions: BTreeMap::new(),
            rejected: BTreeSet::new(),
        };
        assert_eq!(reopened.statuses()[1].status, "saved");
        assert_eq!(
            reopened
                .verify(AccountProvider::Copernicus, &client, &endpoints)
                .await
                .unwrap()
                .status,
            "connected"
        );
        assert_eq!(
            vault
                .get(AccountProvider::Copernicus)
                .unwrap()
                .unwrap()
                .secret
                .as_str(),
            "PRIVATE-REFRESH-TOKEN-TWO"
        );
        reopened.disconnect(AccountProvider::Copernicus).unwrap();
        assert!(vault.get(AccountProvider::Copernicus).unwrap().is_none());
        server.abort();
    }

    #[tokio::test]
    async fn worker_refreshes_expired_sessions_after_restart_and_respects_vault_deletion() {
        let (endpoints, server) = identity_server().await;
        let client = proxy::download_client(&crate::ProxySettings {
            mode: crate::proxy::ProxyMode::Direct,
            url: None,
        })
        .unwrap();
        let vault = Arc::new(MemoryVault::default());
        let mut accounts = Accounts {
            vault: vault.clone(),
            sessions: BTreeMap::new(),
            rejected: BTreeSet::new(),
        };
        let request = serde_json::from_value(serde_json::json!({"provider":"copernicus","username":"tester","password":"PRIVATE-PASSWORD","totp":"123456"})).unwrap();
        accounts
            .connect(request, &client, &endpoints)
            .await
            .unwrap();
        assert_eq!(
            accounts
                .download_token(AccountProvider::Copernicus, &client, &endpoints)
                .await
                .unwrap()
                .as_str(),
            "PRIVATE-ACCESS-TOKEN"
        );
        // Simulate restart: only the stored refresh token survives.
        accounts.sessions.clear();
        assert_eq!(
            accounts
                .download_token(AccountProvider::Copernicus, &client, &endpoints)
                .await
                .unwrap()
                .as_str(),
            "PRIVATE-ACCESS-TOKEN"
        );
        assert_eq!(
            vault
                .get(AccountProvider::Copernicus)
                .unwrap()
                .unwrap()
                .secret
                .as_str(),
            "PRIVATE-REFRESH-TOKEN-TWO"
        );
        vault.delete(AccountProvider::Copernicus).unwrap();
        assert!(accounts
            .download_token(AccountProvider::Copernicus, &client, &endpoints)
            .await
            .is_err());
        let statuses = serde_json::to_string(&accounts.statuses()).unwrap();
        assert!(!statuses.contains("PRIVATE"));
        server.abort();
    }

    #[tokio::test]
    async fn locally_valid_nasa_token_requires_server_acceptance_before_save() {
        let (endpoints, server) = identity_server().await;
        let client = proxy::download_client(&crate::ProxySettings {
            mode: crate::proxy::ProxyMode::Direct,
            url: None,
        })
        .unwrap();
        let mut accounts = Accounts::new(Path::new("nasa-test"));
        let expiry = Utc::now().timestamp() + 600;
        let payload = URL_SAFE_NO_PAD.encode(format!("{{\"exp\":{expiry}}}"));
        let request = |suffix| {
            serde_json::from_value(serde_json::json!({"provider":"nasa-earthdata","token":format!("header.{payload}.{suffix}")})).unwrap()
        };
        let failure = accounts
            .connect(request("rejected"), &client, &endpoints)
            .await
            .err()
            .unwrap();
        assert_eq!(failure, REJECTED);
        assert!(!failure.contains(&payload));
        assert_eq!(accounts.statuses()[0].status, "not-connected");
        assert_eq!(
            accounts
                .connect(request("accepted"), &client, &endpoints)
                .await
                .unwrap()
                .status,
            "connected"
        );
        let before = accounts
            .vault
            .get(AccountProvider::Nasa)
            .unwrap()
            .unwrap()
            .secret
            .clone();
        assert!(accounts
            .connect(request("rejected"), &client, &endpoints)
            .await
            .is_err());
        assert_eq!(
            accounts
                .vault
                .get(AccountProvider::Nasa)
                .unwrap()
                .unwrap()
                .secret
                .as_str(),
            before.as_str()
        );
        server.abort();
    }

    #[tokio::test]
    async fn credential_exchange_never_follows_redirects() {
        let (mut endpoints, server) = identity_server().await;
        endpoints.cdse_token = endpoints.cdse_token.replace("/token", "/redirect");
        let client = proxy::download_client(&crate::ProxySettings {
            mode: crate::proxy::ProxyMode::Direct,
            url: None,
        })
        .unwrap();
        let error = exchange_cdse(&client, &endpoints, &[("password", "DO-NOT-ECHO-THIS")])
            .await
            .err()
            .unwrap();
        assert_eq!(error, NETWORK_ERROR);
        assert!(!error.contains("DO-NOT-ECHO"));
        server.abort();
    }

    #[tokio::test]
    async fn browser_adapter_exposes_status_but_has_no_credential_mutation_routes() {
        use axum::{
            body::Body,
            http::{Method, Request, StatusCode},
        };
        use tower::ServiceExt;
        let directory = tempfile::tempdir().unwrap();
        let manager = JobManager::open(directory.path()).await.unwrap();
        let app = crate::service::router(manager);
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/accounts")
                    .header("Host", "127.0.0.1:4318")
                    .header("Origin", crate::service::ALLOWED_ORIGIN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value.as_array().unwrap().len(), 2);
        assert!(!String::from_utf8_lossy(&bytes).contains("secret"));
        for route in [
            "/accounts",
            "/accounts/connect",
            "/accounts/verify",
            "/accounts/disconnect",
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(Method::POST)
                        .uri(route)
                        .header("Host", "127.0.0.1:4318")
                        .header("Origin", crate::service::ALLOWED_ORIGIN)
                        .header("X-GeoD-Client", "geod-global")
                        .header("Content-Type", "application/json")
                        .body(Body::from("{\"token\":\"NEVER-HANDLE-THIS\"}"))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert!(matches!(
                response.status(),
                StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
            ));
        }
    }
    #[test]
    fn no_secret_can_be_serialized_in_account_status() {
        let vault = Arc::new(MemoryVault::default());
        let mut accounts = Accounts {
            vault: vault.clone(),
            sessions: BTreeMap::new(),
            rejected: BTreeSet::new(),
        };
        let expiry = Utc::now().timestamp() + 600;
        accounts
            .remember(
                AccountProvider::Nasa,
                StoredCredential {
                    version: 1,
                    secret: Zeroizing::new("PRIVATE-USER-TOKEN-SENTINEL".into()),
                    expires_at: expiry,
                    verified_at: crate::now(),
                },
                Zeroizing::new("PRIVATE-USER-TOKEN-SENTINEL".into()),
                expiry,
            )
            .unwrap();
        let result = serde_json::to_string(&accounts.statuses()).unwrap();
        assert!(!result.contains("PRIVATE"));
        assert!(!result.contains("token"));
        let restarted = Accounts {
            vault,
            sessions: BTreeMap::new(),
            rejected: BTreeSet::new(),
        };
        assert_eq!(restarted.statuses()[0].status, "saved");
        accounts.disconnect(AccountProvider::Nasa).unwrap();
        assert_eq!(restarted.statuses()[0].status, "not-connected");
    }
    #[test]
    fn jwt_expiry_never_establishes_authorization() {
        let payload =
            URL_SAFE_NO_PAD.encode(format!("{{\"exp\":{}}}", Utc::now().timestamp() + 600));
        assert!(nasa_expiry(&format!("header.{payload}.unsigned")).is_ok());
        assert!(nasa_expiry("invalid-token").is_err());
        assert!(nasa_expiry("header.eyJleHAiOjF9.signature").is_err());
        let accounts = Accounts::new(Path::new("isolated-test"));
        assert_eq!(accounts.statuses()[0].status, "not-connected");
    }
    #[tokio::test]
    async fn corrupted_storage_and_missing_connection_are_explicit() {
        let vault = Arc::new(MemoryVault::default());
        vault.entries.lock().unwrap().insert(
            AccountProvider::Nasa,
            b"{\"secret\":\"DO-NOT-ECHO\"}".to_vec(),
        );
        let mut accounts = Accounts {
            vault,
            sessions: BTreeMap::new(),
            rejected: BTreeSet::new(),
        };
        assert_eq!(accounts.statuses()[0].status, "storage-error");
        let client = Client::new();
        assert_eq!(
            accounts
                .verify(AccountProvider::Copernicus, &client, &Endpoints::default())
                .await
                .err()
                .as_deref(),
            Some(NOT_CONNECTED)
        );
    }
}
