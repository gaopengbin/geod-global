//! Native metadata registry. Secrets live only in independently named vault entries.
use super::{validate_settings, ModelSettings};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::SystemTime,
};
use zeroize::Zeroizing;

const LEGACY_REF: &str = "model-connection";
const MAX_CONNECTIONS: usize = 16;
const MAX_REGISTRY_BYTES: usize = 65536;
pub const PROVIDERS: &[&str] = &["openai", "deepseek", "anthropic", "google", "custom"];
fn valid_provider_protocol(provider: &str, protocol: &str) -> bool {
    match provider {
        "openai" => matches!(protocol, "openai-compatible" | "openai-responses"),
        "deepseek" => protocol == "openai-compatible",
        "anthropic" => protocol == "anthropic-messages",
        "google" => protocol == "google-generative-ai",
        "custom" => true,
        _ => false,
    }
}

#[derive(Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    #[default]
    Save,
    Select,
    Delete,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelRequest {
    #[serde(default)]
    pub action: Action,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default = "custom_provider")]
    pub provider: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub protocol: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub api_key: Zeroizing<String>,
}
fn custom_provider() -> String {
    "custom".into()
}
impl ModelRequest {
    pub fn settings(&self) -> ModelSettings {
        ModelSettings {
            label: self.label.clone(),
            protocol: self.protocol.clone(),
            base_url: self.base_url.clone(),
            model: self.model.clone(),
        }
    }
}

#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Capabilities {
    text: bool,
    function_calls: bool,
    plaintext_reasoning: bool,
    images: bool,
    encrypted_reasoning: bool,
    context_compaction: bool,
}
impl Default for Capabilities {
    fn default() -> Self {
        Self {
            text: true,
            function_calls: true,
            plaintext_reasoning: true,
            images: true,
            encrypted_reasoning: false,
            context_compaction: true,
        }
    }
}
impl Capabilities {
    fn for_protocol(protocol: &str) -> Self {
        Self {
            encrypted_reasoning: protocol == "openai-responses",
            ..Self::default()
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub provider: String,
    pub settings: ModelSettings,
    pub credential_ref: String,
    pub capabilities: Capabilities,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    legacy_identity: Option<String>,
}
impl Entry {
    pub fn public(&self) -> Value {
        let mut value = serde_json::to_value(&self.settings).unwrap();
        value["id"] = json!(self.id);
        value["provider"] = json!(self.provider);
        value["capabilities"] = json!(self.capabilities);
        // Protocol capability is not evidence that a particular model route works.
        value["verification"] = json!("not-verified");
        value
    }
    pub fn identity(&self) -> Value {
        json!({"id":self.id,"provider":self.provider,"legacyIdentity":self.legacy_identity.as_ref().filter(|id| id.as_str() == legacy_identity(&self.settings))})
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Registry {
    pub version: u8,
    pub selected_id: Option<String>,
    pub connections: Vec<Entry>,
}
impl Default for Registry {
    fn default() -> Self {
        Self {
            version: 1,
            selected_id: None,
            connections: vec![],
        }
    }
}
impl Registry {
    pub fn selected(&self) -> Option<&Entry> {
        self.connections
            .iter()
            .find(|entry| Some(&entry.id) == self.selected_id.as_ref())
    }
    pub fn public(&self) -> Value {
        json!({"version":1,"selectedId":self.selected_id,"connections":self.connections.iter().map(Entry::public).collect::<Vec<_>>()})
    }
    fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || self.connections.len() > MAX_CONNECTIONS
            || self.selected_id.is_some() && self.selected().is_none()
        {
            return Err("Invalid Agent connection registry.".into());
        }
        let mut ids = std::collections::HashSet::new();
        let mut refs = std::collections::HashSet::new();
        for entry in &self.connections {
            if !valid_uuid(&entry.id)
                || !ids.insert(&entry.id)
                || !PROVIDERS.contains(&entry.provider.as_str())
                || !valid_provider_protocol(&entry.provider, &entry.settings.protocol)
                || !entry
                    .credential_ref
                    .strip_prefix("connection-")
                    .is_some_and(valid_uuid)
                || !refs.insert(&entry.credential_ref)
                || entry.capabilities != Capabilities::for_protocol(&entry.settings.protocol)
                || entry.legacy_identity.as_ref().is_some_and(|hash| {
                    hash.len() != 64
                        || !hash
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                })
            {
                return Err("Invalid Agent connection registry.".into());
            }
            validate_settings(&entry.settings)?;
        }
        Ok(())
    }
}

pub trait Vault: Send + Sync {
    fn read(&self, reference: &str) -> Result<Option<Zeroizing<String>>, String>;
    fn write(&self, reference: &str, secret: &str) -> Result<(), String>;
    fn delete(&self, reference: &str) -> Result<(), String>;
}
pub struct NativeVault;
#[cfg(windows)]
fn credential(reference: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(super::VAULT_SERVICE, reference)
        .map_err(|_| "Agent secure credential storage is unavailable.".into())
}
#[cfg(windows)]
impl Vault for NativeVault {
    fn read(&self, reference: &str) -> Result<Option<Zeroizing<String>>, String> {
        match credential(reference)?.get_password() {
            Ok(value) => Ok(Some(Zeroizing::new(value))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err("Agent secure credential storage is unavailable.".into()),
        }
    }
    fn write(&self, reference: &str, secret: &str) -> Result<(), String> {
        credential(reference)?
            .set_password(secret)
            .map_err(|_| "Agent secure credential storage is unavailable.".into())
    }
    fn delete(&self, reference: &str) -> Result<(), String> {
        match credential(reference)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err("Agent secure credential storage is unavailable.".into()),
        }
    }
}
#[cfg(not(windows))]
impl Vault for NativeVault {
    fn read(&self, _: &str) -> Result<Option<Zeroizing<String>>, String> {
        Ok(None)
    }
    fn write(&self, _: &str, _: &str) -> Result<(), String> {
        Err("Agent secure credential storage is unavailable.".into())
    }
    fn delete(&self, _: &str) -> Result<(), String> {
        Err("Agent secure credential storage is unavailable.".into())
    }
}

fn valid_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, byte)| {
            if [8, 13, 18, 23].contains(&i) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
}
pub(super) fn new_id() -> String {
    // Local identifiers, not credentials or authentication tokens. A process-wide
    // sequence also disambiguates simultaneous writes with the same clock value.
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let nonce = format!(
        "{:?}:{}:{}",
        SystemTime::now(),
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let hash = format!("{:x}", Sha256::digest(nonce.as_bytes()));
    format!(
        "{}-{}-{}-{}-{}",
        &hash[..8],
        &hash[8..12],
        &hash[12..16],
        &hash[16..20],
        &hash[20..32]
    )
}
fn normalized(mut settings: ModelSettings) -> Result<ModelSettings, String> {
    validate_settings(&settings)?;
    settings.label = settings.label.trim().into();
    settings.base_url = url::Url::parse(&settings.base_url)
        .map_err(|_| "Invalid Agent model settings.")?
        .to_string();
    Ok(settings)
}
fn legacy_identity(settings: &ModelSettings) -> String {
    format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&json!([
                settings.protocol,
                settings
                    .base_url
                    .strip_suffix('/')
                    .unwrap_or(&settings.base_url),
                settings.model
            ]))
            .unwrap()
        )
    )
}

async fn persist(home: &Path, registry: &Registry) -> Result<(), String> {
    registry.validate()?;
    let bytes = serde_json::to_vec_pretty(registry)
        .map_err(|_| "Agent connection registry could not be saved.")?;
    if bytes.len() > MAX_REGISTRY_BYTES {
        return Err("Agent connection registry is full.".into());
    }
    let temporary = home.join(format!("registry-{}.tmp", new_id()));
    let result = async {
        tokio::fs::write(&temporary, bytes).await?;
        tokio::fs::rename(&temporary, home.join("registry.json")).await
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(temporary).await;
        return Err("Agent connection registry could not be saved.".into());
    }
    Ok(())
}

pub async fn load(home: &Path, vault: &dyn Vault) -> Result<Registry, String> {
    match tokio::fs::read(home.join("registry.json")).await {
        Ok(bytes) => {
            if bytes.len() > MAX_REGISTRY_BYTES {
                return Err(
                    "Agent connection registry could not be read. Existing data was retained."
                        .into(),
                );
            }
            let mut registry: Registry = serde_json::from_slice(&bytes).map_err(|_| {
                "Agent connection registry could not be read. Existing data was retained."
            })?;
            // Registry v1 previously advertised text-only adapters. Normalize
            // this one known capability; all other fields still validate exactly.
            for entry in &mut registry.connections {
                entry.capabilities.images = true;
                entry.capabilities.context_compaction = true;
            }
            registry.validate()?;
            return Ok(registry);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => {
            return Err(
                "Agent connection registry could not be read. Existing data was retained.".into(),
            )
        }
    }
    let bytes = match tokio::fs::read(home.join("model.json")).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Registry::default())
        }
        Err(_) => return Err("Agent model settings could not be read.".into()),
    };
    if bytes.len() > MAX_REGISTRY_BYTES {
        return Err("Agent model settings could not be read.".into());
    }
    let settings = normalized(
        serde_json::from_slice(&bytes).map_err(|_| "Agent model settings could not be read.")?,
    )?;
    let provider = match url::Url::parse(&settings.base_url)
        .ok()
        .and_then(|url| url.host_str().map(String::from))
        .as_deref()
    {
        Some("api.openai.com") => "openai",
        Some("api.deepseek.com") => "deepseek",
        _ => "custom",
    };
    let entry = Entry {
        id: new_id(),
        provider: provider.into(),
        credential_ref: format!("connection-{}", new_id()),
        capabilities: Capabilities::default(),
        legacy_identity: Some(legacy_identity(&settings)),
        settings,
    };
    let secret = vault.read(LEGACY_REF)?;
    if let Some(secret) = &secret {
        vault.write(&entry.credential_ref, secret)?;
    }
    let registry = Registry {
        version: 1,
        selected_id: Some(entry.id.clone()),
        connections: vec![entry.clone()],
    };
    if let Err(error) = persist(home, &registry).await {
        if secret.is_some() {
            let _ = vault.delete(&entry.credential_ref);
        }
        return Err(error);
    }
    // Never delete the old key before the new metadata and key are committed.
    if secret.is_some() {
        let _ = vault.delete(LEGACY_REF);
    }
    // The old secret-free model.json is retained as migration evidence, never used
    // again while registry.json exists. This also avoids overwriting user history.
    Ok(registry)
}

// Resolve an unsaved diagnostic form without changing registry, vault or model
// selection. A saved key may only leave for its unchanged provider/endpoint.
pub fn test_config(
    vault: &dyn Vault,
    current: &Registry,
    request: ModelRequest,
) -> Result<Value, String> {
    current.validate()?;
    if request.action != Action::Save {
        return Err("Connection testing accepts model settings only.".into());
    }
    let settings = normalized(request.settings())?;
    if request.api_key.len() > 4096
        || !PROVIDERS.contains(&request.provider.as_str())
        || !valid_provider_protocol(&request.provider, &settings.protocol)
    {
        return Err("Invalid Agent model settings.".into());
    }
    let old = match request.id.as_ref() {
        Some(id) => Some(
            current
                .connections
                .iter()
                .find(|entry| &entry.id == id)
                .ok_or("Unknown Agent model connection.")?,
        ),
        None => None,
    };
    let secret = if request.api_key.trim().is_empty() {
        let entry = old.filter(|entry| entry.settings.base_url == settings.base_url
            && entry.settings.protocol == settings.protocol && entry.provider == request.provider)
            .ok_or("Enter a new API key when creating a connection or changing its endpoint or provider.")?;
        vault
            .read(&entry.credential_ref)?
            .ok_or("Agent saved credential is missing.")?
    } else {
        request.api_key
    };
    if secret.trim().is_empty() {
        return Err("Agent saved credential is missing.".into());
    }
    let mut config = serde_json::to_value(settings).unwrap();
    config["apiKey"] = json!(secret.as_str());
    Ok(config)
}

pub async fn mutate(
    home: &Path,
    vault: &dyn Vault,
    current: &Registry,
    request: ModelRequest,
) -> Result<Registry, String> {
    current.validate()?;
    let mut next = current.clone();
    let mut new_reference = None;
    let mut obsolete_reference = None;
    if request.action != Action::Save {
        if !request.api_key.is_empty()
            || !request.label.is_empty()
            || !request.protocol.is_empty()
            || !request.base_url.is_empty()
            || !request.model.is_empty()
            || request.provider != "custom"
        {
            return Err("Connection selection and deletion accept only a connection ID.".into());
        }
        let id = request
            .id
            .as_ref()
            .ok_or("Unknown Agent model connection.")?;
        let index = next
            .connections
            .iter()
            .position(|entry| &entry.id == id)
            .ok_or("Unknown Agent model connection.")?;
        if request.action == Action::Select {
            next.selected_id = Some(id.clone());
        } else {
            obsolete_reference = Some(next.connections.remove(index).credential_ref);
            if next.selected_id.as_ref() == Some(id) {
                next.selected_id = next.connections.first().map(|entry| entry.id.clone());
            }
        }
    } else {
        let settings = normalized(request.settings())?;
        if request.api_key.len() > 4096
            || !PROVIDERS.contains(&request.provider.as_str())
            || !valid_provider_protocol(&request.provider, &settings.protocol)
        {
            return Err("Invalid Agent model settings.".into());
        }
        let existing = match &request.id {
            Some(id) => Some(
                next.connections
                    .iter()
                    .position(|entry| &entry.id == id)
                    .ok_or("Unknown Agent model connection.")?,
            ),
            None => None,
        };
        if existing.is_none() && next.connections.len() >= MAX_CONNECTIONS {
            return Err("Agent model connection limit reached.".into());
        }
        let old = existing.map(|index| next.connections[index].clone());
        if request.api_key.trim().is_empty()
            && old.as_ref().is_none_or(|entry| {
                entry.settings.base_url != settings.base_url
                    || entry.settings.protocol != settings.protocol
                    || entry.provider != request.provider
            })
        {
            return Err("Enter a new API key when creating a connection or changing its endpoint or provider.".into());
        }
        let reference = if request.api_key.trim().is_empty() {
            old.as_ref().unwrap().credential_ref.clone()
        } else {
            let reference = format!("connection-{}", new_id());
            vault.write(&reference, request.api_key.as_str())?;
            new_reference = Some(reference.clone());
            obsolete_reference = old.as_ref().map(|entry| entry.credential_ref.clone());
            reference
        };
        let capabilities = Capabilities::for_protocol(&settings.protocol);
        let entry = Entry {
            id: old
                .as_ref()
                .map(|entry| entry.id.clone())
                .unwrap_or_else(new_id),
            provider: request.provider,
            settings,
            credential_ref: reference,
            capabilities,
            legacy_identity: old.and_then(|entry| entry.legacy_identity),
        };
        next.selected_id = Some(entry.id.clone());
        if let Some(index) = existing {
            next.connections[index] = entry;
        } else {
            next.connections.push(entry);
        }
    }
    if let Err(error) = persist(home, &next).await {
        if let Some(reference) = new_reference {
            let _ = vault.delete(&reference);
        }
        return Err(error);
    }
    // A cleanup failure can leave an unreachable vault entry, never a broken or
    // overwritten active credential. No secret or credential reference is public.
    if let Some(reference) = obsolete_reference {
        let _ = vault.delete(&reference);
    }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashMap, sync::Mutex};
    #[derive(Default)]
    struct FakeVault {
        secrets: Mutex<HashMap<String, String>>,
        fail_write: std::sync::atomic::AtomicBool,
    }
    impl Vault for FakeVault {
        fn read(&self, id: &str) -> Result<Option<Zeroizing<String>>, String> {
            Ok(self
                .secrets
                .lock()
                .unwrap()
                .get(id)
                .cloned()
                .map(Zeroizing::new))
        }
        fn write(&self, id: &str, secret: &str) -> Result<(), String> {
            if self.fail_write.load(Ordering::Relaxed) {
                return Err("Synthetic vault failure".into());
            }
            self.secrets
                .lock()
                .unwrap()
                .insert(id.into(), secret.into());
            Ok(())
        }
        fn delete(&self, id: &str) -> Result<(), String> {
            self.secrets.lock().unwrap().remove(id);
            Ok(())
        }
    }
    fn request(value: Value) -> ModelRequest {
        serde_json::from_value(value).unwrap()
    }
    fn save(label: &str, key: &str) -> Value {
        json!({"action":"save","provider":"custom","label":label,"protocol":"openai-compatible","baseUrl":"https://example.test/v1","model":"test-model","apiKey":key})
    }

    #[tokio::test]
    async fn connection_test_uses_current_form_without_saving_or_leaking_a_key_to_changed_endpoint()
    {
        let directory = tempfile::tempdir().unwrap();
        let vault = FakeVault::default();
        let registry = mutate(
            directory.path(),
            &vault,
            &Registry::default(),
            request(save("Saved", "synthetic-saved")),
        )
        .await
        .unwrap();
        let before = tokio::fs::read(directory.path().join("registry.json"))
            .await
            .unwrap();
        let entry = registry.selected().unwrap();
        let form = json!({"id":entry.id,"provider":"custom","label":"Unsaved name","protocol":"openai-compatible","baseUrl":entry.settings.base_url,"model":"different-model"});
        let config = test_config(&vault, &registry, request(form.clone())).unwrap();
        assert_eq!(config["apiKey"], "synthetic-saved");
        assert_eq!(config["model"], "different-model");
        assert_eq!(config["label"], "Unsaved name");
        let mut changed = form.clone();
        changed["baseUrl"] = json!("https://other.test/v1");
        assert!(test_config(&vault, &registry, request(changed.clone())).is_err());
        changed["apiKey"] = json!("synthetic-new-key");
        assert_eq!(
            test_config(&vault, &registry, request(changed)).unwrap()["apiKey"],
            "synthetic-new-key"
        );
        let mut different_provider = form;
        different_provider["provider"] = json!("openai");
        assert!(test_config(&vault, &registry, request(different_provider)).is_err());
        assert_eq!(
            before,
            tokio::fs::read(directory.path().join("registry.json"))
                .await
                .unwrap()
        );
        assert_eq!(registry.selected().unwrap().settings.model, "test-model");
        assert_eq!(vault.secrets.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn registry_keys_selection_and_deletion_are_connection_scoped() {
        let directory = tempfile::tempdir().unwrap();
        let vault = FakeVault::default();
        let first = mutate(
            directory.path(),
            &vault,
            &Registry::default(),
            request(save("First", "synthetic-first")),
        )
        .await
        .unwrap();
        let second = mutate(
            directory.path(),
            &vault,
            &first,
            request(save("Second", "synthetic-second")),
        )
        .await
        .unwrap();
        assert_ne!(second.connections[0].id, second.connections[1].id);
        assert_ne!(
            second.connections[0].credential_ref,
            second.connections[1].credential_ref
        );
        let selected = mutate(
            directory.path(),
            &vault,
            &second,
            request(json!({"action":"select","id":first.selected_id})),
        )
        .await
        .unwrap();
        assert_eq!(selected.selected_id, first.selected_id);
        assert_eq!(vault.secrets.lock().unwrap().len(), 2);
        let reopened = load(directory.path(), &vault).await.unwrap();
        assert_eq!(reopened.selected_id, selected.selected_id);
        let after = mutate(
            directory.path(),
            &vault,
            &reopened,
            request(json!({"action":"delete","id":first.selected_id})),
        )
        .await
        .unwrap();
        assert_eq!(after.connections.len(), 1);
        assert_eq!(
            vault
                .read(&after.connections[0].credential_ref)
                .unwrap()
                .as_deref()
                .map(String::as_str),
            Some("synthetic-second")
        );
        let saved = tokio::fs::read_to_string(directory.path().join("registry.json"))
            .await
            .unwrap();
        for secret in ["synthetic-first", "synthetic-second"] {
            assert!(!saved.contains(secret));
            assert!(!after.public().to_string().contains(secret));
        }
        assert!(!after.public().to_string().contains("credentialRef"));
    }

    #[tokio::test]
    async fn openai_responses_capability_is_protocol_scoped_and_legacy_connections_stay_unchanged()
    {
        let directory = tempfile::tempdir().unwrap();
        let vault = FakeVault::default();
        let mut legacy_request = save("Legacy OpenAI", "synthetic-legacy");
        legacy_request["provider"] = json!("openai");
        let legacy = mutate(
            directory.path(),
            &vault,
            &Registry::default(),
            request(legacy_request),
        )
        .await
        .unwrap();
        let legacy_identity = legacy.connections[0].identity();
        let mut native_request = save("Native OpenAI", "synthetic-native");
        native_request["provider"] = json!("openai");
        native_request["protocol"] = json!("openai-responses");
        let native = mutate(directory.path(), &vault, &legacy, request(native_request))
            .await
            .unwrap();
        assert!(!native.connections[0].capabilities.encrypted_reasoning);
        assert!(native.connections[1].capabilities.encrypted_reasoning);
        let reopened = load(directory.path(), &vault).await.unwrap();
        assert_eq!(reopened.connections[0].identity(), legacy_identity);
        assert_eq!(
            reopened.connections[0].settings.protocol,
            "openai-compatible"
        );
        assert_eq!(
            reopened.connections[1].settings.protocol,
            "openai-responses"
        );
        let mut no_key_change = save("Changed protocol", "");
        no_key_change["id"] = json!(reopened.connections[0].id);
        no_key_change["provider"] = json!("openai");
        no_key_change["protocol"] = json!("openai-responses");
        assert!(
            mutate(directory.path(), &vault, &reopened, request(no_key_change))
                .await
                .is_err()
        );
        assert_eq!(vault.secrets.lock().unwrap().len(), 2);
        let bytes = tokio::fs::read(directory.path().join("registry.json"))
            .await
            .unwrap();
        let mut forged: Value = serde_json::from_slice(&bytes).unwrap();
        forged["connections"][1]["capabilities"]["encryptedReasoning"] = json!(false);
        tokio::fs::write(
            directory.path().join("registry.json"),
            serde_json::to_vec(&forged).unwrap(),
        )
        .await
        .unwrap();
        assert!(load(directory.path(), &vault).await.is_err());
        assert_eq!(vault.secrets.lock().unwrap().len(), 2);
    }
    #[tokio::test]
    async fn endpoint_or_provider_changes_require_a_new_key_and_vault_failure_preserves_the_old_key(
    ) {
        let directory = tempfile::tempdir().unwrap();
        let vault = FakeVault::default();
        let registry = mutate(
            directory.path(),
            &vault,
            &Registry::default(),
            request(save("Old", "synthetic-old")),
        )
        .await
        .unwrap();
        let before = tokio::fs::read(directory.path().join("registry.json"))
            .await
            .unwrap();
        for field in ["baseUrl", "provider"] {
            let mut change = save("Changed", "");
            change["id"] = json!(registry.selected_id);
            change[field] = json!(if field == "provider" {
                "openai"
            } else {
                "https://changed.test/v1"
            });
            assert!(mutate(directory.path(), &vault, &registry, request(change))
                .await
                .is_err());
        }
        let mut change = save("Changed", "synthetic-replacement");
        change["id"] = json!(registry.selected_id);
        vault.fail_write.store(true, Ordering::Relaxed);
        assert!(mutate(directory.path(), &vault, &registry, request(change))
            .await
            .is_err());
        assert_eq!(
            tokio::fs::read(directory.path().join("registry.json"))
                .await
                .unwrap(),
            before
        );
        assert_eq!(
            vault
                .read(&registry.connections[0].credential_ref)
                .unwrap()
                .as_deref()
                .map(String::as_str),
            Some("synthetic-old")
        );
    }
    #[cfg(windows)]
    #[tokio::test]
    async fn failed_atomic_commit_keeps_old_registry_and_key_and_removes_only_the_staged_key() {
        use std::os::windows::fs::OpenOptionsExt;
        let directory = tempfile::tempdir().unwrap();
        let vault = FakeVault::default();
        let registry = mutate(
            directory.path(),
            &vault,
            &Registry::default(),
            request(save("Old", "synthetic-old")),
        )
        .await
        .unwrap();
        let before = tokio::fs::read(directory.path().join("registry.json"))
            .await
            .unwrap();
        // Allow reads/writes but deny deletion/replacement of this exact test file.
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(3)
            .open(directory.path().join("registry.json"))
            .unwrap();
        let mut change = save("Changed", "synthetic-replacement");
        change["id"] = json!(registry.selected_id);
        assert!(mutate(directory.path(), &vault, &registry, request(change))
            .await
            .is_err());
        assert!(mutate(
            directory.path(),
            &vault,
            &registry,
            request(json!({"action":"delete","id":registry.selected_id}))
        )
        .await
        .is_err());
        assert_eq!(
            tokio::fs::read(directory.path().join("registry.json"))
                .await
                .unwrap(),
            before
        );
        assert_eq!(vault.secrets.lock().unwrap().len(), 1);
        assert_eq!(
            vault
                .read(&registry.connections[0].credential_ref)
                .unwrap()
                .as_deref()
                .map(String::as_str),
            Some("synthetic-old")
        );
        drop(lock);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }
    #[tokio::test]
    async fn legacy_migration_is_idempotent_and_binds_its_original_history_identity() {
        let directory = tempfile::tempdir().unwrap();
        let vault = FakeVault::default();
        let settings = ModelSettings {
            label: "Legacy".into(),
            protocol: "openai-compatible".into(),
            base_url: "https://api.deepseek.com".into(),
            model: "legacy-model".into(),
        };
        tokio::fs::write(
            directory.path().join("model.json"),
            serde_json::to_vec(&settings).unwrap(),
        )
        .await
        .unwrap();
        vault.write(LEGACY_REF, "synthetic-legacy").unwrap();
        let migrated = load(directory.path(), &vault).await.unwrap();
        let entry = migrated.selected().unwrap();
        assert_eq!(entry.provider, "deepseek");
        assert_eq!(
            entry.identity()["legacyIdentity"],
            legacy_identity(&normalized(settings).unwrap())
        );
        assert!(vault.read(LEGACY_REF).unwrap().is_none());
        assert_eq!(
            vault
                .read(&entry.credential_ref)
                .unwrap()
                .as_deref()
                .map(String::as_str),
            Some("synthetic-legacy")
        );
        assert_eq!(
            load(directory.path(), &vault).await.unwrap().selected_id,
            migrated.selected_id
        );
        let mut changed = save("Legacy", "");
        changed["id"] = json!(entry.id);
        changed["provider"] = json!(entry.provider);
        changed["baseUrl"] = json!(entry.settings.base_url);
        changed["model"] = json!("changed-model");
        let changed = mutate(directory.path(), &vault, &migrated, request(changed))
            .await
            .unwrap();
        assert!(changed.selected().unwrap().identity()["legacyIdentity"].is_null());
    }
    #[tokio::test]
    async fn corrupt_registry_is_retained_and_unsupported_capabilities_are_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let vault = FakeVault::default();
        tokio::fs::write(directory.path().join("registry.json"), b"broken")
            .await
            .unwrap();
        assert!(load(directory.path(), &vault).await.is_err());
        assert_eq!(
            tokio::fs::read(directory.path().join("registry.json"))
                .await
                .unwrap(),
            b"broken"
        );
        assert!(serde_json::from_value::<ModelRequest>(
            json!({"action":"select","id":new_id(),"credentialRef":"model-connection"})
        )
        .is_err());
        let registry = mutate(
            directory.path(),
            &vault,
            &Registry::default(),
            request(save("Valid", "synthetic")),
        )
        .await
        .unwrap();
        let mut forged = serde_json::to_value(&registry).unwrap();
        forged["connections"][0]["capabilities"]["encryptedReasoning"] = json!(true);
        assert!(serde_json::from_value::<Registry>(forged)
            .unwrap()
            .validate()
            .is_err());
        for provider in ["anthropic", "google"] {
            let mut value = save("Wrong", "synthetic");
            value["provider"] = json!(provider);
            assert!(mutate(directory.path(), &vault, &registry, request(value))
                .await
                .is_err());
        }
    }
    #[tokio::test]
    async fn native_protocols_keep_independent_credentials_and_reject_substitution() {
        let directory = tempfile::tempdir().unwrap();
        let vault = FakeVault::default();
        let mut registry = Registry::default();
        for (provider, protocol, endpoint, model) in [
            (
                "anthropic",
                "anthropic-messages",
                "https://api.anthropic.com/v1",
                "claude-haiku-4-5",
            ),
            (
                "google",
                "google-generative-ai",
                "https://generativelanguage.googleapis.com/v1beta",
                "gemini-3-flash-preview",
            ),
        ] {
            let value = json!({"provider":provider,"protocol":protocol,"label":provider,"baseUrl":endpoint,"model":model,"apiKey":"synthetic-native-key"});
            registry = mutate(directory.path(), &vault, &registry, request(value.clone()))
                .await
                .unwrap();
            let entry = registry.selected().unwrap();
            assert_eq!(entry.settings.protocol, protocol);
            assert_eq!(entry.public()["verification"], "not-verified");
            assert!(!entry.public().to_string().contains("synthetic-native-key"));
            let mut mismatched = value.clone();
            mismatched["protocol"] = json!("openai-compatible");
            assert!(
                mutate(directory.path(), &vault, &registry, request(mismatched))
                    .await
                    .is_err()
            );
            let mut changed = value;
            changed["id"] = json!(entry.id);
            changed["provider"] = json!("custom");
            changed["apiKey"] = json!("");
            assert!(
                mutate(directory.path(), &vault, &registry, request(changed))
                    .await
                    .is_err()
            );
        }
        assert_ne!(
            registry.connections[0].credential_ref,
            registry.connections[1].credential_ref
        );
        let reopened = load(directory.path(), &vault).await.unwrap();
        assert_eq!(reopened.connections.len(), 2);
        let bad = json!({"provider":"google","protocol":"google-generative-ai","label":"Bad path","baseUrl":"https://example.test/v1beta","model":"models/../../messages","apiKey":"synthetic"});
        assert!(mutate(directory.path(), &vault, &registry, request(bad))
            .await
            .is_err());
    }
    #[cfg(windows)]
    #[test]
    fn native_vault_round_trip_uses_only_its_own_marker_and_cleans_it() {
        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = NativeVault.delete(&self.0);
            }
        }
        let reference = format!("connection-{}", new_id());
        let cleanup = Cleanup(reference.clone());
        let marker = format!("geod-registry-test-marker-{}", new_id());
        NativeVault.write(&reference, &marker).unwrap();
        assert_eq!(
            NativeVault
                .read(&reference)
                .unwrap()
                .as_deref()
                .map(String::as_str),
            Some(marker.as_str())
        );
        NativeVault.delete(&reference).unwrap();
        assert!(NativeVault.read(&reference).unwrap().is_none());
        drop(cleanup);
    }
    #[tokio::test]
    async fn legacy_capabilities_load_images_and_context_without_replacing_credentials() {
        let directory = tempfile::tempdir().unwrap();
        let vault = FakeVault::default();
        let original = mutate(
            directory.path(),
            &vault,
            &Registry::default(),
            request(save("Legacy", "synthetic-legacy")),
        )
        .await
        .unwrap();
        let mut data = serde_json::to_value(&original).unwrap();
        data["connections"][0]["capabilities"]["images"] = json!(false);
        data["connections"][0]["capabilities"]["contextCompaction"] = json!(false);
        tokio::fs::write(
            directory.path().join("registry.json"),
            serde_json::to_vec(&data).unwrap(),
        )
        .await
        .unwrap();
        let reopened = load(directory.path(), &vault).await.unwrap();
        assert_eq!(
            reopened.connections[0].credential_ref,
            original.connections[0].credential_ref
        );
        assert_eq!(
            reopened.public()["connections"][0]["capabilities"]["images"],
            true
        );
        assert_eq!(
            reopened.public()["connections"][0]["capabilities"]["contextCompaction"],
            true
        );
        assert_eq!(vault.secrets.lock().unwrap().len(), 1);
        data["connections"][0]["capabilities"]["encryptedReasoning"] = json!(true);
        tokio::fs::write(
            directory.path().join("registry.json"),
            serde_json::to_vec(&data).unwrap(),
        )
        .await
        .unwrap();
        assert!(load(directory.path(), &vault).await.is_err());
    }
}
