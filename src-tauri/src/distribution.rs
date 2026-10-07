//! Product-owned update trust and an anonymous, local-first announcement inbox.
//! No Agent tool, web endpoint override, or dependency on the domestic app.
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{DateTime, Utc};
use geod_runtime::proxy::ProxyMode;
use geod_runtime::JobManager;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::sync::Notify;

const PRODUCT: &str = "xyz.laogao.geod.global";
const MAX_FEED: usize = 512 * 1024;
const MAX_UPDATE: usize = 1024 * 1024 * 1024;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Channel {
    product: String,
    endpoint: String,
    pubkey: String,
    messages_endpoint: Option<String>,
}
fn channel() -> Result<Option<Channel>, String> {
    // Public configuration is frozen into a release. Local HTTP fixtures are
    // available only in debug builds and never become a saved user preference.
    let raw = if cfg!(debug_assertions) {
        match std::env::var_os("GEOD_GLOBAL_DEV_DISTRIBUTION") {
            Some(file) => Some(fs::read_to_string(file).map_err(|_| "Update channel is invalid.")?),
            None => option_env!("GEOD_GLOBAL_DISTRIBUTION").map(str::to_owned),
        }
    } else {
        option_env!("GEOD_GLOBAL_DISTRIBUTION").map(str::to_owned)
    };
    raw.map(|raw| {
        let value: Channel =
            serde_json::from_str(&raw).map_err(|_| "Update channel is invalid.")?;
        value.validate()?;
        Ok(value)
    })
    .transpose()
}
fn trusted_url(raw: &str) -> bool {
    url::Url::parse(raw).is_ok_and(|url| {
        url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none()
            && (url.scheme() == "https"
                || (cfg!(debug_assertions)
                    && url.scheme() == "http"
                    && matches!(url.host_str(), Some("127.0.0.1" | "[::1]"))))
    })
}
impl Channel {
    fn validate(&self) -> Result<(), String> {
        if self.product != PRODUCT
            || !trusted_url(&self.endpoint)
            || self
                .messages_endpoint
                .as_ref()
                .is_some_and(|url| !trusted_url(url))
            || public_key(&self.pubkey).is_err()
        {
            return Err("Update channel is invalid.".into());
        }
        Ok(())
    }
}
fn public_key(encoded: &str) -> Result<minisign_verify::PublicKey, String> {
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| "Update channel is invalid.")?;
    let text = std::str::from_utf8(&bytes).map_err(|_| "Update channel is invalid.")?;
    minisign_verify::PublicKey::decode(text).map_err(|_| "Update channel is invalid.".into())
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Text {
    en: String,
    #[serde(rename = "zh-CN")]
    zh: String,
}
impl Text {
    fn valid(&self, max: usize) -> bool {
        !self.en.trim().is_empty()
            && !self.zh.trim().is_empty()
            && self.en.len() <= max
            && self.zh.len() <= max
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Message {
    id: String,
    revision: u32,
    title: Text,
    body: Text,
    priority: String,
    published_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
    min_version: Option<String>,
    max_version: Option<String>,
    action: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Feed {
    schema_version: u8,
    product: String,
    items: Vec<Message>,
}
impl Feed {
    fn validate(&self) -> Result<(), String> {
        let mut ids = BTreeSet::new();
        if self.schema_version != 1 || self.product != PRODUCT || self.items.len() > 100 {
            return Err("Notification response is invalid.".into());
        }
        for item in &self.items {
            let versions = [item.min_version.as_ref(), item.max_version.as_ref()];
            if item.id.is_empty()
                || item.id.len() > 100
                || !item
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
                || !ids.insert(&item.id)
                || item.revision == 0
                || !item.title.valid(200)
                || !item.body.valid(12000)
                || !["normal", "important"].contains(&item.priority.as_str())
                || item
                    .expires_at
                    .is_some_and(|date| date <= item.published_at)
                || versions
                    .into_iter()
                    .flatten()
                    .any(|v| semver::Version::parse(v).is_err())
                || item
                    .min_version
                    .as_ref()
                    .zip(item.max_version.as_ref())
                    .is_some_and(|(a, b)| {
                        semver::Version::parse(a).unwrap() > semver::Version::parse(b).unwrap()
                    })
                || item
                    .action
                    .as_deref()
                    .is_some_and(|action| !["updates", "sources"].contains(&action))
            {
                return Err("Notification response is invalid.".into());
            }
        }
        Ok(())
    }
}
fn eligible(item: &Message, now: DateTime<Utc>, version: &semver::Version) -> bool {
    item.published_at <= now
        && item.expires_at.is_none_or(|date| now < date)
        && item
            .min_version
            .as_ref()
            .is_none_or(|v| semver::Version::parse(v).is_ok_and(|v| version >= &v))
        && item
            .max_version
            .as_ref()
            .is_none_or(|v| semver::Version::parse(v).is_ok_and(|v| version <= &v))
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CachedUpdate {
    version: String,
    signature: String,
    sha256: String,
    bytes: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Saved {
    cached_update: Option<CachedUpdate>,
    read_update: Option<String>,
    automatic_checks: bool,
    notifications_enabled: bool,
    last_update_check: Option<DateTime<Utc>>,
    last_messages_check: Option<DateTime<Utc>>,
    read: BTreeMap<String, u32>,
    seen: BTreeMap<String, u32>,
    items: Vec<Message>,
}
impl Default for Saved {
    fn default() -> Self {
        Self {
            cached_update: None,
            read_update: None,
            automatic_checks: true,
            notifications_enabled: true,
            last_update_check: None,
            last_messages_check: None,
            read: BTreeMap::new(),
            seen: BTreeMap::new(),
            items: vec![],
        }
    }
}
struct Downloaded {
    update: Update,
    path: PathBuf,
    sha256: String,
}
pub struct Distribution {
    recovered_state: bool,
    root: PathBuf,
    saved: Mutex<Saved>,
    view: Mutex<Value>,
    pending: Mutex<Option<Update>>,
    downloaded: Mutex<Option<Downloaded>>,
    busy: AtomicBool,
    cancel: Notify,
}
impl Distribution {
    pub fn open(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&root).map_err(|_| "Could not save update settings.")?;
        let mut recovered_state = false;
        let saved = match fs::read(root.join("state.json")) {
            Ok(bytes) => {
                let parsed = if bytes.len() <= 2 * MAX_FEED {
                    serde_json::from_slice::<Saved>(&bytes).ok()
                } else {
                    None
                };
                if let Some(saved) = parsed.filter(|saved| {
                    Feed {
                        schema_version: 1,
                        product: PRODUCT.into(),
                        items: saved.items.clone(),
                    }
                    .validate()
                    .is_ok()
                }) {
                    saved
                } else {
                    // This optional inbox must not prevent the main data app
                    // from opening. Preserve the damaged file for recovery.
                    let backup = root.join(format!(
                        "state-damaged-{}.json",
                        Utc::now().timestamp_nanos_opt().unwrap_or_default()
                    ));
                    fs::rename(root.join("state.json"), backup)
                        .map_err(|_| "Could not read update settings.")?;
                    recovered_state = true;
                    Saved::default()
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Saved::default(),
            _ => return Err("Could not read update settings.".into()),
        };
        Feed {
            schema_version: 1,
            product: PRODUCT.into(),
            items: saved.items.clone(),
        }
        .validate()?;
        Ok(Self {
            recovered_state,
            root,
            saved: Mutex::new(saved),
            view: Mutex::new(json!({"state":"idle"})),
            pending: Mutex::new(None),
            downloaded: Mutex::new(None),
            busy: AtomicBool::new(false),
            cancel: Notify::new(),
        })
    }
    fn change(&self, action: impl FnOnce(&mut Saved)) -> Result<(), String> {
        let mut saved = self
            .saved
            .lock()
            .map_err(|_| "Could not save update settings.")?;
        let mut next = saved.clone();
        action(&mut next);
        let mut file = tempfile::NamedTempFile::new_in(&self.root)
            .map_err(|_| "Could not save update settings.")?;
        serde_json::to_writer(file.as_file_mut(), &next)
            .map_err(|_| "Could not save update settings.")?;
        file.as_file()
            .sync_all()
            .map_err(|_| "Could not save update settings.")?;
        file.persist(self.root.join("state.json"))
            .map_err(|_| "Could not save update settings.")?;
        *saved = next;
        Ok(())
    }
    fn phase(&self, app: &AppHandle, value: Value) {
        *self.view.lock().unwrap() = value;
        let _ = app.emit_to("main", "geod-distribution-changed", ());
    }
    fn begin(&self) -> Result<Operation<'_>, String> {
        self.busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "An update operation is already running.")?;
        Ok(Operation(&self.busy))
    }
    fn snapshot(&self, version: &str) -> Result<Value, String> {
        let configured = channel()?;
        let saved = self
            .saved
            .lock()
            .map_err(|_| "Could not read update settings.")?;
        let version_value =
            semver::Version::parse(version).map_err(|_| "Invalid application version.")?;
        let items: Vec<Value> = saved
            .items
            .iter()
            .filter(|item| eligible(item, Utc::now(), &version_value))
            .map(|item| {
                let mut value = serde_json::to_value(item).unwrap();
                value["read"] = json!(saved
                    .read
                    .get(&item.id)
                    .is_some_and(|r| *r >= item.revision));
                value["seen"] = json!(saved
                    .seen
                    .get(&item.id)
                    .is_some_and(|r| *r >= item.revision));
                value
            })
            .collect();
        let update = self.view.lock().unwrap().clone();
        Ok(
            json!({"schemaVersion":1,"product":PRODUCT,"version":version,"development":cfg!(debug_assertions),"recoveredState":self.recovered_state,
            "updateConfigured":configured.is_some(),"messagesConfigured":configured.as_ref().is_some_and(|c|c.messages_endpoint.is_some()),
            "automaticChecks":saved.automatic_checks,"notificationsEnabled":saved.notifications_enabled,
            "lastUpdateCheck":saved.last_update_check,"lastMessagesCheck":saved.last_messages_check,
            "updateRead":update["version"].as_str().is_some_and(|v|saved.read_update.as_deref()==Some(v)),
            "update":update,"busy":self.busy.load(Ordering::Acquire),
            "items":items,"unreadCount":items.iter().filter(|i|i["read"] == false).count()}),
        )
    }
}
struct Operation<'a>(&'a AtomicBool);
impl Drop for Operation<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[tauri::command]
pub fn distribution_snapshot(
    app: AppHandle,
    state: State<'_, Distribution>,
) -> Result<Value, String> {
    state.snapshot(&app.package_info().version.to_string())
}
#[tauri::command]
pub fn distribution_preferences(
    app: AppHandle,
    state: State<'_, Distribution>,
    automatic_checks: bool,
    notifications_enabled: bool,
) -> Result<Value, String> {
    state.change(|s| {
        s.automatic_checks = automatic_checks;
        s.notifications_enabled = notifications_enabled;
    })?;
    distribution_snapshot(app, state)
}
#[tauri::command]
pub fn notifications_read(
    app: AppHandle,
    state: State<'_, Distribution>,
    ids: Vec<String>,
    seen_only: bool,
    update_version: Option<String>,
) -> Result<Value, String> {
    if ids.len() > 100 {
        return Err("Notification response is invalid.".into());
    }
    let current_version = state.view.lock().unwrap()["version"]
        .as_str()
        .map(str::to_owned);
    if update_version
        .as_ref()
        .is_some_and(|version| Some(version) != current_version.as_ref())
    {
        return Err("Update changed. Check again.".into());
    }
    state.change(|s| {
        if !seen_only && update_version.is_some() {
            s.read_update = update_version;
        }
        for item in &s.items {
            if ids.contains(&item.id) {
                if seen_only {
                    s.seen.insert(item.id.clone(), item.revision);
                } else {
                    s.read.insert(item.id.clone(), item.revision);
                }
            }
        }
        // Bound old read markers while retaining all current revisions.
        if s.read.len() > 500 {
            s.read.retain(|id, _| s.items.iter().any(|i| &i.id == id));
        }
        if s.seen.len() > 500 {
            s.seen.retain(|id, _| s.items.iter().any(|i| &i.id == id));
        }
    })?;
    distribution_snapshot(app, state)
}
#[tauri::command]
pub async fn update_check(
    app: AppHandle,
    state: State<'_, Distribution>,
    manager: State<'_, JobManager>,
) -> Result<Value, String> {
    let _operation = state.begin()?;
    let Some(channel) = channel()? else {
        state.phase(&app, json!({"state":"unconfigured"}));
        return Ok(json!({"state":"unconfigured"}));
    };
    state.phase(&app, json!({"state":"checking"}));
    let result = async {
        let mut builder = app.updater_builder().pubkey(channel.pubkey).endpoints(vec![channel.endpoint.parse().unwrap()]).map_err(|_| "Update channel is invalid.")?.timeout(Duration::from_secs(30));
        let proxy = manager.proxy_settings().await;
        builder = match proxy.mode { ProxyMode::Direct => builder.no_proxy(), ProxyMode::Custom => builder.proxy(proxy.url.as_deref().ok_or("Update proxy is invalid.")?.parse().map_err(|_| "Update proxy is invalid.")?), ProxyMode::System => builder };
        let update = builder.build().map_err(|_| "Update channel is invalid.")?.check().await.map_err(|_| "Could not check for updates. Check your network and retry.")?;
        if update.as_ref().is_some_and(|u| !trusted_url(u.download_url.as_str())) {return Err("Update download address is invalid.");}
        if update.as_ref().is_some_and(|u| u.raw_json["product"] != PRODUCT || u.signature.len()>8192 || u.body.as_ref().is_some_and(|notes|notes.len()>12000)) {return Err("Update channel is invalid.");}
        let cached=state.saved.lock().unwrap().cached_update.clone();
        if let (Some(update),Some(cached))=(&update,cached) {
            if update.version==cached.version && update.signature==cached.signature && cached.sha256.len()==64 && cached.sha256.bytes().all(|b|b.is_ascii_hexdigit()) && cached.bytes>0 && cached.bytes<=MAX_UPDATE as u64 {
                let path=state.root.join(format!("{}.update",cached.sha256));
                if fs::metadata(&path).is_ok_and(|m|m.len()==cached.bytes) && fs::read(&path).is_ok_and(|b|format!("{:x}",Sha256::digest(&b))==cached.sha256) {
                    *state.downloaded.lock().unwrap()=Some(Downloaded{update:update.clone(),path,sha256:cached.sha256});
                }
            }
        }
        state.change(|s|s.last_update_check=Some(Utc::now())).map_err(|_| "Could not save update settings.")?;
        let value = if let Some(update) = &update {
            if state.downloaded.lock().unwrap().as_ref().is_some_and(|d| d.update.version == update.version && d.update.signature == update.signature) {json!({"state":"ready","version":update.version,"notes":update.body})}
            else {json!({"state":"available","version":update.version,"notes":update.body,"publishedAt":update.date.map(|d|d.to_string())})}
        } else {json!({"state":"upToDate"})};
        {let mut downloaded=state.downloaded.lock().unwrap();if downloaded.as_ref().is_some_and(|d|update.as_ref().is_none_or(|u|u.version!=d.update.version || u.signature!=d.update.signature)) {*downloaded=None;}}
        *state.pending.lock().unwrap()=update;
        Ok(value)
    }.await;
    match result {
        Ok(value) => {
            state.phase(&app, value.clone());
            Ok(value)
        }
        Err(error) => {
            state.phase(&app, json!({"state":"error","message":error}));
            Err(error.into())
        }
    }
}
#[tauri::command]
pub fn update_cancel(state: State<'_, Distribution>) {
    state.cancel.notify_one();
}
#[tauri::command]
pub async fn update_download(
    app: AppHandle,
    state: State<'_, Distribution>,
    version: String,
) -> Result<Value, String> {
    let _operation = state.begin()?;
    let update = state
        .pending
        .lock()
        .unwrap()
        .clone()
        .filter(|u| u.version == version)
        .ok_or("Update changed. Check again.")?;
    // Drain an old cancel permit before starting this explicitly requested download.
    let _ = tokio::time::timeout(Duration::ZERO, state.cancel.notified()).await;
    state.phase(
        &app,
        json!({"state":"downloading","version":version,"downloaded":0,"total":null}),
    );
    let mut received = 0usize;
    let mut tick = std::time::Instant::now();
    let transfer=update.download(|chunk,total| {
        received=received.saturating_add(chunk);
        if received>MAX_UPDATE || total.is_some_and(|t|t>MAX_UPDATE as u64) {state.cancel.notify_one();}
        if tick.elapsed()>Duration::from_millis(150) {state.phase(&app,json!({"state":"downloading","version":version,"downloaded":received,"total":total}));tick=std::time::Instant::now();}
    },||state.phase(&app,json!({"state":"verifying","version":version})));
    let result = tokio::select! {
        value=transfer => value.map_err(|_| "Update download or signature verification failed."),
        _=state.cancel.notified() => Err("Update download cancelled."),
        _=tokio::time::sleep(Duration::from_secs(3600)) => Err("Update download timed out."),
    };
    let result=result.and_then(|bytes| {
        if bytes.is_empty() || bytes.len()>MAX_UPDATE {return Err("Update package is too large.");}
        let sha256=format!("{:x}",Sha256::digest(&bytes));
        let path=state.root.join(format!("{sha256}.update"));
        let mut file=tempfile::NamedTempFile::new_in(&state.root).map_err(|_| "Could not save update file.")?;
        std::io::Write::write_all(file.as_file_mut(),&bytes).map_err(|_| "Could not save update file.")?;
        file.as_file().sync_all().map_err(|_| "Could not save update file.")?;
        file.persist(&path).map_err(|_| "Could not save update file.")?;
        state.change(|s|s.cached_update=Some(CachedUpdate{version:version.clone(),signature:update.signature.clone(),sha256:sha256.clone(),bytes:bytes.len() as u64})).map_err(|_| "Could not save update settings.")?;
        *state.downloaded.lock().unwrap()=Some(Downloaded { update:update.clone(),path,sha256 });
        Ok(json!({"state":"ready","version":version,"bytes":bytes.len(),"verified":true,"notes":update.body}))
    });
    match result {
        Ok(value) => {
            state.phase(&app, value.clone());
            Ok(value)
        }
        Err(error) => {
            state.phase(
                &app,
                json!({"state":"available","version":version,"notes":update.body,"message":error}),
            );
            Err(error.into())
        }
    }
}
#[tauri::command]
pub async fn update_install(
    app: AppHandle,
    state: State<'_, Distribution>,
    version: String,
) -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Err("Development builds do not install updates.".into());
    }
    let _operation = state.begin()?;
    let lifecycle = app.state::<crate::DesktopLifecycle>().inner().clone();
    let _maintenance = lifecycle.maintenance()?;
    let drain = lifecycle.clone();
    tauri::async_runtime::spawn_blocking(move || drain.drain_commands())
        .await
        .map_err(|_| "Could not prepare update installation.")?;
    let manager = app.state::<JobManager>().inner().clone();
    let agent = app.state::<crate::AgentAvailability>().0.clone().ok();
    if manager.has_active_work().await
        || if let Some(agent) = &agent {
            agent.is_busy().await?
        } else {
            false
        }
    {
        return Err(
            "Wait for downloads, processing and the Agent to finish before installing.".into(),
        );
    }
    let (update, path, sha256) = {
        let saved = state.downloaded.lock().unwrap();
        let d = saved
            .as_ref()
            .filter(|d| d.update.version == version)
            .ok_or("Update changed. Check again.")?;
        (d.update.clone(), d.path.clone(), d.sha256.clone())
    };
    let bytes = fs::read(&path).map_err(|_| "Could not read update file.")?;
    if format!("{:x}", Sha256::digest(&bytes)) != sha256 {
        return Err("Update file changed. Download it again.".into());
    }
    // Download checked the signed version; recheck the original Minisign content
    // too, immediately before handing the bytes to the installer.
    let channel = channel()?.ok_or("Update channel is invalid.")?;
    let signature = STANDARD
        .decode(&update.signature)
        .map_err(|_| "Update signature is invalid.")?;
    let signature = std::str::from_utf8(&signature).map_err(|_| "Update signature is invalid.")?;
    let signature = minisign_verify::Signature::decode(signature)
        .map_err(|_| "Update signature is invalid.")?;
    public_key(&channel.pubkey)?
        .verify(&bytes, &signature, true)
        .map_err(|_| "Update signature is invalid.")?;
    if lifecycle.exiting() {
        return Err("Application maintenance is already running.".into());
    }
    state.phase(&app, json!({"state":"installing","version":version}));
    if let Some(agent) = agent {
        agent.shutdown().await;
    }
    manager.shutdown().await?;
    // Installer only changes the application's directory. User records and
    // credentials live in the separate application-local store and are retained.
    update
        .install(bytes)
        .map_err(|_| "Could not start update installation. Restart the app and retry.".into())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    payload: String,
    signature: String,
}
fn signed_feed(bytes: &[u8], key: &str) -> Result<Feed, String> {
    if bytes.len() > MAX_FEED {
        return Err("Notification response is invalid.".into());
    }
    let envelope: Envelope =
        serde_json::from_slice(bytes).map_err(|_| "Notification response is invalid.")?;
    let payload = STANDARD
        .decode(envelope.payload)
        .map_err(|_| "Notification response is invalid.")?;
    let signature = STANDARD
        .decode(envelope.signature)
        .map_err(|_| "Notification signature is invalid.")?;
    let signature =
        std::str::from_utf8(&signature).map_err(|_| "Notification signature is invalid.")?;
    let signature = minisign_verify::Signature::decode(signature)
        .map_err(|_| "Notification signature is invalid.")?;
    public_key(key)?
        .verify(&payload, &signature, true)
        .map_err(|_| "Notification signature is invalid.")?;
    let feed: Feed =
        serde_json::from_slice(&payload).map_err(|_| "Notification response is invalid.")?;
    feed.validate()?;
    Ok(feed)
}
#[tauri::command]
pub async fn notifications_refresh(
    app: AppHandle,
    state: State<'_, Distribution>,
    manager: State<'_, JobManager>,
) -> Result<Value, String> {
    let Some(channel) = channel()? else {
        return state.snapshot(&app.package_info().version.to_string());
    };
    let Some(endpoint) = channel.messages_endpoint else {
        return state.snapshot(&app.package_info().version.to_string());
    };
    let proxy = manager.proxy_settings().await;
    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none());
    builder = match proxy.mode {
        ProxyMode::Direct => builder.no_proxy(),
        ProxyMode::Custom => builder.proxy(
            reqwest::Proxy::all(proxy.url.ok_or("Update proxy is invalid.")?)
                .map_err(|_| "Update proxy is invalid.")?,
        ),
        ProxyMode::System => builder,
    };
    let mut response = builder
        .build()
        .map_err(|_| "Could not refresh notifications.")?
        .get(endpoint)
        .send()
        .await
        .map_err(|_| "Could not refresh notifications. Check your network and retry.")?;
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|n| n > MAX_FEED as u64)
    {
        return Err("Could not refresh notifications.".to_owned());
    }
    let mut bytes = vec![];
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Could not refresh notifications.")?
    {
        if bytes.len() + chunk.len() > MAX_FEED {
            return Err("Notification response is invalid.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let feed = signed_feed(&bytes, &channel.pubkey)?;
    state.change(|s| {
        s.items = feed.items;
        s.last_messages_check = Some(Utc::now());
    })?;
    let _ = app.emit_to("main", "geod-distribution-changed", ());
    state.snapshot(&app.package_info().version.to_string())
}

#[cfg(test)]
mod tests;
