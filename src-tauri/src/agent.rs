//! Managed, optional Agent child. No second runtime store or HTTP GeoD API.
use geod_runtime::agent_actions::{self, MapContext};
use geod_runtime::{mcp, JobManager};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::{Child, Command},
    sync::{mpsc, oneshot, Mutex},
};
#[cfg(all(test, windows))]
use zeroize::Zeroizing;

#[cfg(all(test, windows))]
#[path = "agent_conversation_tests.rs"]
mod conversation_tests;
#[path = "agent_documents.rs"]
mod documents;
#[path = "agent_execution.rs"]
mod execution;
#[cfg(test)]
#[path = "agent_image_acceptance.rs"]
mod image_acceptance;
#[path = "agent_images.rs"]
mod images;
pub use execution::ExecutionMode;
#[path = "agent_registry.rs"]
mod registry;
#[path = "agent_workspace.rs"]
mod workspace;
pub use registry::ModelRequest;

fn definitions() -> Vec<Value> {
    let mut result = mcp::agent_read_definitions();
    result.extend(agent_actions::definitions());
    result.extend(execution::definitions());
    result.push(workspace::definition());
    result
}

const FRAME_LIMIT: usize = 1_000_000;
#[cfg(windows)]
const VAULT_SERVICE: &str = "xyz.laogao.geod.global.agent";

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelSettings {
    pub label: String,
    pub protocol: String,
    pub base_url: String,
    pub model: String,
}

fn validate_settings(value: &ModelSettings) -> Result<(), String> {
    if ![
        "openai-compatible",
        "openai-responses",
        "anthropic-messages",
        "google-generative-ai",
    ]
    .contains(&value.protocol.as_str())
        || value.label.trim().is_empty()
        || value.label.len() > 80
        || value.model.is_empty()
        || value.model.len() > 160
        || !value.model.as_bytes()[0].is_ascii_alphanumeric()
        || value.base_url.len() > 1000
        || !value
            .model
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_./:@+-".contains(&byte))
        || value.protocol == "google-generative-ai"
            && !value
                .model
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_.+-".contains(&byte))
    {
        return Err("Invalid Agent model settings.".into());
    }
    let url = url::Url::parse(&value.base_url).map_err(|_| "Invalid Agent model settings.")?;
    let loopback = matches!(url.host_str(), Some("127.0.0.1" | "[::1]"));
    if (url.scheme() != "https" && !(url.scheme() == "http" && loopback))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Agent model endpoint must use HTTPS or local loopback HTTP.".into());
    }
    Ok(())
}

#[derive(Clone)]
pub struct DesktopAgent(Arc<AgentInner>);
struct AgentInner {
    home: PathBuf,
    runtime: PathBuf,
    manager: JobManager,
    vault: Arc<dyn registry::Vault>,
    state: Mutex<AgentState>,
    notifier: std::sync::RwLock<Option<ChangeNotifier>>,
}
type ChangeNotifier = Arc<dyn Fn(u64) + Send + Sync>;
struct AgentState {
    registry: registry::Registry,
    connection: Option<Arc<Connection>>,
}

impl DesktopAgent {
    pub async fn open(
        home: PathBuf,
        runtime: PathBuf,
        manager: JobManager,
    ) -> Result<Self, String> {
        Self::open_with_vault(home, runtime, manager, Arc::new(registry::NativeVault)).await
    }
    async fn open_with_vault(
        home: PathBuf,
        runtime: PathBuf,
        manager: JobManager,
        vault: Arc<dyn registry::Vault>,
    ) -> Result<Self, String> {
        tokio::fs::create_dir_all(&home)
            .await
            .map_err(|_| "Agent session folder could not be opened.")?;
        let registry = registry::load(&home, vault.as_ref()).await?;
        Ok(Self(Arc::new(AgentInner {
            home,
            runtime,
            manager,
            vault,
            notifier: std::sync::RwLock::new(None),
            state: Mutex::new(AgentState {
                registry,
                connection: None,
            }),
        })))
    }
    pub fn on_change(&self, callback: impl Fn(u64) + Send + Sync + 'static) {
        if let Ok(mut notifier) = self.0.notifier.write() {
            *notifier = Some(Arc::new(callback));
        }
    }
    async fn connection(&self, state: &mut AgentState) -> Result<Arc<Connection>, String> {
        if let Some(connection) = &state.connection {
            if !connection.closed.load(Ordering::Acquire) {
                return Ok(connection.clone());
            }
            connection.close().await;
            state.connection = None;
        }
        verify_runtime(&self.0.runtime).await?;
        let binding = state
            .registry
            .selected()
            .map(|entry| entry.id.clone())
            .unwrap_or_else(registry::new_id);
        let notifier = self.0.notifier.read().ok().and_then(|value| value.clone());
        let connection = Connection::spawn_notifying(
            &self.0.runtime,
            &self.0.home,
            self.0.manager.clone(),
            binding,
            notifier,
        )
        .await?;
        if let Some(entry) = state.registry.selected() {
            if let Some(secret) = self.0.vault.read(&entry.credential_ref)? {
                let mut config = serde_json::to_value(&entry.settings)
                    .map_err(|_| "Invalid Agent model settings.")?;
                config["apiKey"] = json!(secret.as_str());
                config["connection"] = entry.identity();
                if let Err(error) = connection
                    .rpc(
                        "configure",
                        json!({"config":config,"definitions":definitions()}),
                    )
                    .await
                {
                    connection.close().await;
                    return Err(error);
                }
            }
        }
        state.connection = Some(connection.clone());
        Ok(connection)
    }
    pub async fn snapshot(&self) -> Result<Value, String> {
        let mut state = self.0.state.lock().await;
        // Missing optional runtime is a normal, explicit setup state. Ordinary
        // data workflows remain usable and never trigger a silent download.
        if !self.0.runtime.join("manifest.json").is_file() {
            return Ok(
                json!({"version":1,"revision":0,"runtimeAvailable":false,"mode":"review-first","configured":false,
                "model":state.registry.selected().map(registry::Entry::public),"registry":state.registry.public(),"busy":false,"sessions":[],"selected":null,"plans":[]}),
            );
        }
        let connection = self.connection(&mut state).await?;
        self.snapshot_value(&connection, &state).await
    }
    /// An update idle check must not start a model/runtime or restore history.
    pub async fn is_busy(&self) -> Result<bool, String> {
        let state = self.0.state.lock().await;
        if let Some(connection) = &state.connection {
            let value = connection.rpc("snapshot", json!({})).await?;
            return value["busy"]
                .as_bool()
                .ok_or_else(|| "Cannot check Agent activity.".into());
        }
        Ok(false)
    }
    async fn snapshot_value(
        &self,
        connection: &Arc<Connection>,
        state: &AgentState,
    ) -> Result<Value, String> {
        let mut value = connection.rpc("snapshot", json!({})).await?;
        // Recover a committed native replacement after a crash or failed
        // transcript write. Only follow receipts from completed native refs.
        for _ in 0..10 {
            if value["busy"] == true {
                break;
            }
            let session = value["selected"]["id"].as_str().unwrap_or("");
            if value["sessions"].as_array().is_some_and(|sessions| {
                sessions
                    .iter()
                    .any(|s| s["id"] == session && s["compatible"] == false)
            }) {
                break;
            }
            let refs = plan_references(&value);
            let mut recovered = None;
            for id in &refs {
                if let Ok(old) = self.0.manager.agent_plan_status(session, id).await {
                    if old["status"] == "superseded" {
                        if let Some(next) = old["replacedBy"]
                            .as_str()
                            .filter(|next| !refs.iter().any(|id| id == next))
                        {
                            let plan = self.0.manager.agent_plan_status(session, next).await?;
                            recovered = Some(
                                json!({"sessionId":session,"originalPlanId":id,"planId":next,"kind":plan["kind"]}),
                            );
                            break;
                        }
                    }
                }
            }
            let Some(params) = recovered else {
                break;
            };
            value = connection.rpc("recordPlanRevision", params).await?;
        }
        value["runtimeAvailable"] = json!(true);
        value["registry"] = state.registry.public();
        if !value["model"].is_null() {
            value["model"] = state
                .registry
                .selected()
                .map(registry::Entry::public)
                .unwrap_or(Value::Null);
        }
        self.attach_plans(&mut value).await;
        value["execution"] = connection
            .policy
            .public(value["selected"]["id"].as_str(), &connection.binding)
            .await;
        Ok(value)
    }
    pub async fn operation(&self, method: &str, params: Value) -> Result<Value, String> {
        if !matches!(
            method,
            "send" | "select" | "interrupt" | "compact" | "acknowledgeView" | "goalControl"
        ) {
            return Err("Unknown Agent operation.".into());
        }
        let mut state = self.0.state.lock().await;
        let connection = self.connection(&mut state).await?;
        if method == "send" {
            self.send_or_control(&mut state, &connection, params)
                .await?;
        } else {
            connection.rpc(method, params).await?;
        }
        self.snapshot_value(&connection, &state).await
    }
    async fn send_or_control(
        &self,
        state: &mut AgentState,
        connection: &Arc<Connection>,
        mut params: Value,
    ) -> Result<(), String> {
        let snapshot = connection.rpc("snapshot", json!({})).await?;
        if snapshot["configured"] != true {
            return Err("Configure an Agent model connection first.".into());
        }
        if snapshot["busy"] == true {
            return Err("Wait for the Agent response to finish before changing execution.".into());
        }
        let text = params["text"]
            .as_str()
            .filter(|text| text.len() <= 32_000)
            .ok_or("Invalid Agent message.")?
            .to_owned();
        let previous = params["sessionId"].as_str().map(str::to_owned);
        if !params["decisionAnswer"].is_null()
            && (previous.as_deref() != snapshot["selected"]["id"].as_str()
                || !text.is_empty()
                || ["images", "documents"].iter().any(|key| {
                    params[*key]
                        .as_array()
                        .is_some_and(|items| !items.is_empty())
                }))
        {
            return Err("Answer the selected conversation's decision without additional text or attachments.".into());
        }
        if let Some(id) = &previous {
            if !snapshot["sessions"].as_array().is_some_and(|sessions| {
                sessions
                    .iter()
                    .any(|session| session["id"] == *id && session["compatible"] == true)
            }) {
                return Err(
                    "Unknown or incompatible Agent conversation. Start a new conversation.".into(),
                );
            }
        }
        let session = previous.clone().unwrap_or_else(registry::new_id);
        let mut mode = connection
            .policy
            .mode(previous.as_deref(), &connection.binding)
            .await;
        let mut control = execution::chat_control(&text);
        // A normal follow-up can also say "continue" before any background work
        // exists. In that case let Codex reason from the existing conversation.
        if matches!(control, Some(execution::ChatControl::Resume))
            && (snapshot["selected"]["workflow"].is_null()
                || snapshot["selected"]["workflow"]["status"] == "completed")
        {
            control = None;
        }
        let mut plans = Vec::new();
        if let Some(action) = control {
            if ["images", "documents"].iter().any(|key| {
                params[*key]
                    .as_array()
                    .is_some_and(|items| !items.is_empty())
            }) {
                return Err("Send execution controls without attachments.".into());
            }
            if let execution::ChatControl::Mode(next) = action {
                mode = next;
            }
            if let execution::ChatControl::Confirm(all) = action {
                if previous.is_none() || snapshot["selected"]["id"] != session {
                    return Err(
                        "Select the conversation containing the plan before confirming.".into(),
                    );
                }
                let value = self.snapshot_value(connection, state).await?;
                let pending = value["plans"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|plan| plan["status"] == "pending")
                    .cloned()
                    .collect::<Vec<_>>();
                if pending.is_empty() {
                    return Err("There is no pending plan in this conversation.".into());
                }
                if !all && pending.len() != 1 {
                    return Err("Several plans need confirmation. Say confirm all plans or choose a specific plan card.".into());
                }
                for plan in &pending {
                    execution::check_decision_confirmation(
                        &value,
                        plan["planId"].as_str().unwrap(),
                    )?;
                    self.checked_review(
                        state,
                        &session,
                        plan["planId"].as_str().unwrap(),
                        plan["planHash"].as_str().unwrap(),
                    )
                    .await?;
                }
                for plan in pending {
                    match self
                        .0
                        .manager
                        .approve_agent_plan(
                            &session,
                            plan["planId"].as_str().unwrap(),
                            plan["planHash"].as_str().unwrap(),
                        )
                        .await
                    {
                        Ok(value) => plans.push(value),
                        Err(error) => {
                            if !plans.is_empty() {
                                connection.rpc("recordControl",json!({"sessionId":session,"text":text,"action":"confirm","plans":plans,"executionMode":mode,"executionBinding":connection.binding})).await?;
                            }
                            return Err(error);
                        }
                    }
                }
            }
            connection
                .policy
                .grant(Some(&session), &connection.binding, mode)
                .await?;
            let action = match action {
                execution::ChatControl::Mode(_) => "mode",
                execution::ChatControl::Confirm(_) => "confirm",
                execution::ChatControl::Resume => "resume",
                execution::ChatControl::Pause => "pause",
            };
            connection.rpc("recordControl",json!({"sessionId":previous,"newSessionId":session,"text":text,"action":action,"plans":plans,"executionMode":mode,"executionBinding":connection.binding})).await?;
        } else {
            connection
                .policy
                .grant(Some(&session), &connection.binding, mode)
                .await?;
            let request = connection
                .policy
                .begin_human_request(&session, &connection.binding, &text)
                .await;
            params["newSessionId"] = json!(session);
            params["executionMode"] = json!(mode);
            params["executionBinding"] = json!(connection.binding);
            params["requestId"] = json!(request);
            connection.rpc("send", params).await?;
        }
        Ok(())
    }
    pub async fn execution_mode(
        &self,
        session: Option<String>,
        mode: ExecutionMode,
    ) -> Result<Value, String> {
        let mut state = self.0.state.lock().await;
        let connection = self.connection(&mut state).await?;
        let snapshot = connection.rpc("snapshot", json!({})).await?;
        if snapshot["busy"] == true {
            return Err("Wait for the Agent response to finish before changing execution.".into());
        }
        if let Some(id) = &session {
            if snapshot["configured"] != true
                || snapshot["selected"]["id"] != *id
                || !snapshot["sessions"].as_array().is_some_and(|sessions| {
                    sessions
                        .iter()
                        .any(|session| session["id"] == *id && session["compatible"] == true)
                })
            {
                return Err("Select a compatible conversation before changing execution.".into());
            }
        }
        connection
            .policy
            .grant(session.as_deref(), &connection.binding, mode)
            .await?;
        if let Some(id) = session {
            connection.rpc("recordControl",json!({"sessionId":id,"text":"","action":"mode","executionMode":mode,"executionBinding":connection.binding})).await?;
        }
        self.snapshot_value(&connection, &state).await
    }
    pub async fn attach_image(&self, name: String, encoded: String) -> Result<Value, String> {
        // Serialize owned image writes with model/settings operations. The
        // decoder runs off the async thread and never receives an external path.
        let _state = self.0.state.lock().await;
        let home = self.0.home.clone();
        let image = tokio::task::spawn_blocking(move || images::ingest(&home, &name, &encoded))
            .await
            .map_err(|_| "Agent image preview could not be created.")??;
        serde_json::to_value(image).map_err(|_| "Agent image preview could not be created.".into())
    }
    pub async fn image_preview(&self, id: String) -> Result<Value, String> {
        let home = self.0.home.clone();
        tokio::task::spawn_blocking(move || images::preview(&home, &id))
            .await
            .map_err(|_| "Agent image could not be read.")?
    }
    pub async fn attach_document(&self, name: String, encoded: String) -> Result<Value, String> {
        let _state = self.0.state.lock().await;
        let home = self.0.home.clone();
        let document =
            tokio::task::spawn_blocking(move || documents::ingest(&home, &name, &encoded))
                .await
                .map_err(|_| "Agent document could not be saved.")??;
        serde_json::to_value(document).map_err(|_| "Agent document could not be saved.".into())
    }
    pub async fn document_preview(&self, id: String) -> Result<Value, String> {
        let home = self.0.home.clone();
        tokio::task::spawn_blocking(move || documents::preview(&home, &id))
            .await
            .map_err(|_| "Agent document could not be read.")?
    }
    pub async fn attachment_storage(
        &self,
        protected_images: Vec<String>,
        protected_documents: Vec<String>,
        cleanup: bool,
    ) -> Result<Value, String> {
        if protected_images.len() + protected_documents.len() > 3
            || protected_images
                .iter()
                .chain(&protected_documents)
                .any(|id| !images::valid_id(id))
        {
            return Err("Invalid Agent attachment reference.".into());
        }
        let mut state = self.0.state.lock().await;
        let connection = self.connection(&mut state).await?;
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct References {
            images: Vec<String>,
            documents: Vec<String>,
        }
        let references: References =
            serde_json::from_value(connection.rpc("attachmentReferences", json!({})).await?)
                .map_err(|_| "Agent attachment history could not be checked.")?;
        if references.images.len() > 300
            || references.documents.len() > 400
            || references
                .images
                .iter()
                .chain(&references.documents)
                .any(|id| !images::valid_id(id))
        {
            return Err("Agent attachment history could not be checked.".into());
        }
        let kept_images = references
            .images
            .into_iter()
            .chain(protected_images)
            .collect();
        let kept_documents = references
            .documents
            .into_iter()
            .chain(protected_documents)
            .collect();
        let home = self.0.home.clone();
        tokio::task::spawn_blocking(move || {
            // Check both directories before starting cleanup, retaining all
            // connections and drafts under the same native state lock.
            let images = images::storage(&home, &kept_images, false)?;
            let documents = documents::storage(&home, &kept_documents, false)?;
            if !cleanup { return Ok(json!({"images":images,"documents":documents})); }
            Ok(json!({"images":images::storage(&home, &kept_images, true)?,"documents":documents::storage(&home, &kept_documents, true)?}))
        }).await.map_err(|_| "Agent attachment storage is unavailable.")?
    }
    pub async fn image_storage(
        &self,
        protected_images: Vec<String>,
        cleanup: bool,
    ) -> Result<Value, String> {
        if protected_images.len() > 3 || protected_images.iter().any(|id| !images::valid_id(id)) {
            return Err("Invalid Agent image reference.".into());
        }
        let mut state = self.0.state.lock().await;
        let connection = self.connection(&mut state).await?;
        let references = connection.rpc("imageReferences", json!({})).await?;
        let references: Vec<String> = serde_json::from_value(references)
            .map_err(|_| "Agent image history could not be checked.")?;
        if references.len() > 300 || references.iter().any(|id| !images::valid_id(id)) {
            return Err("Agent image history could not be checked.".into());
        }
        let keep = references.into_iter().chain(protected_images).collect();
        let home = self.0.home.clone();
        let result = tokio::task::spawn_blocking(move || images::storage(&home, &keep, cleanup))
            .await
            .map_err(|_| "Agent image storage is unavailable.")??;
        serde_json::to_value(result).map_err(|_| "Agent image storage is unavailable.".into())
    }
    async fn attach_plans(&self, snapshot: &mut Value) {
        let session = snapshot["selected"]["id"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let mut ids = Vec::new();
        if let Some(entries) = snapshot["selected"]["entries"].as_array() {
            for entry in entries
                .iter()
                .rev()
                .filter(|e| e["type"] == "tool" && e["status"] == "completed")
            {
                if let Some(references) = entry["references"].as_array() {
                    for reference in references.iter().filter(|r| r["kind"] == "plan") {
                        if let Some(id) = reference["id"].as_str() {
                            if !ids.iter().any(|seen| seen == id) && ids.len() < 10 {
                                ids.push(id.to_string());
                            }
                        }
                    }
                }
            }
        }
        let mut plans = Vec::new();
        for id in ids {
            match self.0.manager.agent_plan_status(&session, &id).await {
                Ok(plan) => plans.push(plan),
                Err(_) => plans.push(json!({"planId":id,"status":"unavailable"})),
            }
        }
        snapshot["plans"] = json!(plans);
    }
    pub async fn approve_plan(&self, session: &str, id: &str, hash: &str) -> Result<Value, String> {
        // A model cannot reach this path. Require the selected native transcript
        // to contain this exact review card before honoring the desktop click.
        let mut state = self.0.state.lock().await;
        let connection = self.checked_review(&mut state, session, id, hash).await?;
        execution::check_decision_confirmation(&connection.rpc("snapshot", json!({})).await?, id)?;
        let plan = self.0.manager.approve_agent_plan(session, id, hash).await?;
        let mode = connection
            .policy
            .mode(Some(session), &connection.binding)
            .await;
        connection
            .policy
            .grant(Some(session), &connection.binding, mode)
            .await?;
        connection.rpc("recordControl",json!({"sessionId":session,"text":"","action":"confirm","plans":[plan],"executionMode":mode,"executionBinding":connection.binding})).await?;
        self.snapshot_value(&connection, &state).await
    }
    async fn checked_review(
        &self,
        state: &mut AgentState,
        session: &str,
        id: &str,
        hash: &str,
    ) -> Result<Arc<Connection>, String> {
        let connection = self.connection(state).await?;
        let snapshot = connection.rpc("snapshot", json!({})).await?;
        if snapshot["busy"] == true {
            return Err("Wait for the Agent response to finish before confirming.".into());
        }
        if snapshot["selected"]["id"] != session
            || snapshot["sessions"].as_array().is_some_and(|sessions| {
                sessions
                    .iter()
                    .any(|s| s["id"] == session && s["compatible"] == false)
            })
            || !plan_references(&snapshot).iter().any(|value| value == id)
        {
            return Err("Select and review this conversation's plan before confirming.".into());
        }
        let plan = self.0.manager.agent_plan_status(session, id).await?;
        if plan["planHash"] != hash {
            return Err("The review changed. Reload the native plan before editing.".into());
        }
        Ok(connection)
    }
    pub async fn plan_map_preview(
        &self,
        session: &str,
        id: &str,
        hash: &str,
    ) -> Result<Value, String> {
        let mut state = self.0.state.lock().await;
        self.checked_review(&mut state, session, id, hash).await?;
        self.0
            .manager
            .agent_plan_map_preview(session, id, hash)
            .await
    }
    pub async fn plan_revision_draft(
        &self,
        session: &str,
        id: &str,
        hash: &str,
    ) -> Result<Value, String> {
        let mut state = self.0.state.lock().await;
        self.checked_review(&mut state, session, id, hash).await?;
        self.0
            .manager
            .agent_plan_revision_draft(session, id, hash)
            .await
    }
    pub async fn revise_plan(
        &self,
        session: &str,
        id: &str,
        hash: &str,
        revision: agent_actions::PlanRevision,
    ) -> Result<Value, String> {
        let mut state = self.0.state.lock().await;
        let connection = self.checked_review(&mut state, session, id, hash).await?;
        execution::check_no_pending_decisions(&connection.rpc("snapshot", json!({})).await?)?;
        let next = self
            .0
            .manager
            .revise_agent_plan(session, id, hash, revision)
            .await?;
        connection.rpc("recordPlanRevision",json!({"sessionId":session,"originalPlanId":id,"planId":next["planId"],"kind":next["kind"]})).await?;
        self.snapshot_value(&connection, &state).await
    }
    pub async fn test_model(&self, request: ModelRequest) -> Result<Value, String> {
        let mut state = self.0.state.lock().await;
        if let Some(connection) = &state.connection {
            if connection.rpc("snapshot", json!({})).await?["busy"] == true {
                return Err("Stop the Agent response before testing a connection.".into());
            }
        }
        let config = registry::test_config(self.0.vault.as_ref(), &state.registry, request)?;
        let connection = self.connection(&mut state).await?;
        let result = connection
            .rpc("testModel", json!({"config":config}))
            .await?;
        let checked: ConnectionTestResult = serde_json::from_value(result)
            .map_err(|_| "Agent returned invalid connection test data.")?;
        if checked.version != 1
            || checked.latency_ms > 30_000
            || checked.checked_at.len() != 24
            || !checked
                .checked_at
                .bytes()
                .enumerate()
                .all(|(index, byte)| match index {
                    4 | 7 => byte == b'-',
                    10 => byte == b'T',
                    13 | 16 => byte == b':',
                    19 => byte == b'.',
                    23 => byte == b'Z',
                    _ => byte.is_ascii_digit(),
                })
            || !matches!(checked.status.as_str(), "passed" | "failed")
            || (checked.status == "passed"
                && (!checked.text || !checked.function_calls || checked.message.is_some()))
            || (checked.status == "failed"
                && (checked.function_calls
                    || checked
                        .message
                        .as_ref()
                        .is_none_or(|message| message.is_empty() || message.len() > 240)))
        {
            return Err("Agent returned invalid connection test data.".into());
        }
        Ok(serde_json::to_value(checked).unwrap())
    }
    pub async fn save_model(&self, request: ModelRequest) -> Result<Value, String> {
        let mut state = self.0.state.lock().await;
        // This guard precedes all vault reads/writes and registry persistence.
        if let Some(connection) = &state.connection {
            if connection
                .rpc("snapshot", json!({}))
                .await?
                .get("busy")
                .and_then(Value::as_bool)
                == Some(true)
            {
                return Err("Stop the Agent response before changing models.".into());
            }
        }
        let next = registry::mutate(
            &self.0.home,
            self.0.vault.as_ref(),
            &state.registry,
            request,
        )
        .await?;
        state.registry = next;
        if let Some(connection) = state.connection.take() {
            connection.close().await;
        }
        drop(state);
        self.snapshot().await
    }
    pub async fn shutdown(&self) {
        let mut state = self.0.state.lock().await;
        if let Some(connection) = state.connection.take() {
            connection.close().await;
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConnectionTestResult {
    version: u8,
    status: String,
    text: bool,
    function_calls: bool,
    latency_ms: u64,
    checked_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

fn plan_references(snapshot: &Value) -> Vec<String> {
    let mut ids = Vec::new();
    if let Some(entries) = snapshot["selected"]["entries"].as_array() {
        for entry in entries
            .iter()
            .rev()
            .filter(|entry| entry["type"] == "tool" && entry["status"] == "completed")
        {
            if let Some(refs) = entry["references"].as_array() {
                for reference in refs.iter().filter(|r| r["kind"] == "plan") {
                    if let Some(id) = reference["id"].as_str() {
                        if !ids.iter().any(|known| known == id) && ids.len() < 10 {
                            ids.push(id.to_owned());
                        }
                    }
                }
            }
        }
    }
    ids
}

async fn verify_runtime(root: &std::path::Path) -> Result<(), String> {
    let bytes = tokio::fs::read(root.join("manifest.json"))
        .await
        .map_err(|_| "Agent runtime is unavailable. Prepare the development runtime first.")?;
    let manifest: Value =
        serde_json::from_slice(&bytes).map_err(|_| "Agent runtime verification failed.")?;
    if manifest["version"] != 1
        || manifest["platform"] != "win32-x64"
        || manifest["codexVersion"] != "0.159.2"
        || manifest["nodeVersion"] != "24.14.0"
    {
        return Err("Agent runtime verification failed.".into());
    }
    let root = root.to_owned();
    // Hash incrementally off the async worker; avoid a 325 MiB allocation and
    // blocking unrelated desktop IPC while checking the exact same binaries.
    tokio::task::spawn_blocking(move || {
        use std::io::Read;
        let mut buffer = vec![0u8; 256 * 1024];
        for file in ["node.exe", "codex.exe", "agent.mjs"] {
            let expected = manifest["files"][file]["sha256"]
                .as_str()
                .filter(|v| v.len() == 64)
                .ok_or("Agent runtime verification failed.")?;
            let mut reader = std::fs::File::open(root.join(file))
                .map_err(|_| "Agent runtime verification failed.")?;
            let mut digest = Sha256::new();
            loop {
                let length = reader
                    .read(&mut buffer)
                    .map_err(|_| "Agent runtime verification failed.")?;
                if length == 0 {
                    break;
                }
                digest.update(&buffer[..length]);
            }
            if format!("{:x}", digest.finalize()) != expected {
                return Err("Agent runtime verification failed.".to_string());
            }
        }
        Ok(())
    })
    .await
    .map_err(|_| "Agent runtime verification failed.".to_string())?
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>;
fn change_revision(value: &Value) -> Option<u64> {
    if value.as_object()?.len() != 2
        || value["method"] != "geod.changed"
        || value["params"].as_object()?.len() != 1
    {
        return None;
    }
    value["params"]["revision"]
        .as_u64()
        .filter(|revision| *revision <= 9_007_199_254_740_991)
}
enum Write {
    Json(Value),
    Close,
}
struct Connection {
    binding: String,
    policy: Arc<execution::ExecutionPolicy>,
    writer: mpsc::Sender<Write>,
    pending: Pending,
    sequence: AtomicU64,
    closed: Arc<AtomicBool>,
    child: Mutex<Child>,
    #[cfg(windows)]
    _job: ProcessJob,
    #[cfg(all(test, windows))]
    diagnostics: Arc<Mutex<String>>,
}
impl Connection {
    #[cfg(test)]
    async fn spawn(
        runtime: &std::path::Path,
        home: &std::path::Path,
        manager: JobManager,
    ) -> Result<Arc<Self>, String> {
        Self::spawn_notifying(runtime, home, manager, registry::new_id(), None).await
    }
    async fn spawn_notifying(
        runtime: &std::path::Path,
        home: &std::path::Path,
        manager: JobManager,
        binding: String,
        notifier: Option<ChangeNotifier>,
    ) -> Result<Arc<Self>, String> {
        // Node's entry-point resolver rejects Windows canonical \\?\ drive
        // paths (EISDIR on G:). Keep native verification canonical, but pass
        // equivalent ordinary paths to this owned child runtime.
        let runtime = child_path(runtime);
        let home = child_path(home);
        let policy = Arc::new(execution::ExecutionPolicy::open(home.clone()).await?);
        let mut command = Command::new(runtime.join("node.exe"));
        command
            .arg(runtime.join("agent.mjs"))
            .arg(&home)
            .arg(runtime.join("codex.exe"))
            .current_dir(&home)
            .env_clear()
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        for name in [
            "SystemRoot",
            "SYSTEMROOT",
            "WINDIR",
            "TEMP",
            "TMP",
            "HTTPS_PROXY",
            "HTTP_PROXY",
            "NO_PROXY",
        ] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        command.env("NODE_USE_ENV_PROXY", "1");
        #[cfg(all(test, windows))]
        command.env("GEOD_AGENT_TEST_TRACE", "1");
        #[cfg(windows)]
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        let mut child = command
            .spawn()
            .map_err(|_| "Agent runtime could not start.")?;
        #[cfg(windows)]
        let job = ProcessJob::attach(&child)?;
        let mut input = child.stdin.take().ok_or("Agent runtime could not start.")?;
        let mut output = child
            .stdout
            .take()
            .ok_or("Agent runtime could not start.")?;
        let mut diagnostic = child
            .stderr
            .take()
            .ok_or("Agent runtime could not start.")?;
        let (writer, mut receiver) = mpsc::channel(32);
        let pending: Pending = Arc::default();
        let closed = Arc::new(AtomicBool::new(false));
        #[cfg(all(test, windows))]
        let diagnostics = Arc::new(Mutex::new(String::new()));
        let connection = Arc::new(Self {
            binding: binding.clone(),
            policy: policy.clone(),
            writer: writer.clone(),
            pending: pending.clone(),
            sequence: AtomicU64::new(0),
            closed: closed.clone(),
            child: Mutex::new(child),
            #[cfg(windows)]
            _job: job,
            #[cfg(all(test, windows))]
            diagnostics: diagnostics.clone(),
        });
        tokio::spawn(async move {
            while let Some(message) = receiver.recv().await {
                let Write::Json(value) = message else {
                    break;
                };
                let Ok(mut bytes) = serde_json::to_vec(&value) else {
                    break;
                };
                bytes.push(b'\n');
                if input.write_all(&bytes).await.is_err() {
                    break;
                }
            }
        });
        tokio::spawn(async move {
            let mut buffer = [0; 4096];
            while let Ok(count) = diagnostic.read(&mut buffer).await {
                if count == 0 {
                    break;
                }
                #[cfg(all(test, windows))]
                {
                    let secret = std::env::var("GEOD_AGENT_TEST_KEY").unwrap_or_default();
                    let text = String::from_utf8_lossy(&buffer[..count]);
                    let text = if secret.is_empty() {
                        text.to_string()
                    } else {
                        text.replace(&secret, "[redacted]")
                    };
                    let mut value = diagnostics.lock().await;
                    if value.len() < 4000 {
                        value.push_str(&text);
                    }
                }
            }
        });
        tokio::spawn(async move {
            let mut buffer = Vec::new();
            let mut chunk = [0; 8192];
            let permits = Arc::new(tokio::sync::Semaphore::new(4));
            loop {
                let Ok(count) = output.read(&mut chunk).await else {
                    break;
                };
                if count == 0 {
                    break;
                }
                buffer.extend_from_slice(&chunk[..count]);
                if buffer.len() > FRAME_LIMIT {
                    break;
                }
                while let Some(end) = buffer.iter().position(|byte| *byte == b'\n') {
                    let Ok(value) = serde_json::from_slice::<Value>(&buffer[..end]) else {
                        closed.store(true, Ordering::Release);
                        break;
                    };
                    buffer.drain(..=end);
                    if value["method"] == "geod.changed" && value.get("id").is_none() {
                        // The webview only receives a change signal, never raw
                        // model text, tool arguments, credentials or Node frames.
                        if let Some(revision) = change_revision(&value) {
                            if let Some(callback) = &notifier {
                                callback(revision);
                            }
                        }
                    } else if value["method"] == "geod.tool" {
                        let writer = writer.clone();
                        let manager = manager.clone();
                        let policy = policy.clone();
                        let binding = binding.clone();
                        let permit = permits.clone().try_acquire_owned();
                        tokio::spawn(async move {
                            let response = match (
                                permit,
                                value["params"]["name"].as_str(),
                                value["params"].get("arguments"),
                            ) {
                                (Ok(_permit), Some(name), Some(args)) => {
                                    let session =
                                        value["params"]["sessionId"].as_str().unwrap_or("");
                                    let context = value["params"]
                                        .get("context")
                                        .filter(|v| !v.is_null())
                                        .map(|v| serde_json::from_value::<MapContext>(v.clone()))
                                        .transpose();
                                    match context {
                                        Ok(context) => {
                                            if [
                                                "geod_execution_policy",
                                                "geod_plan_execute",
                                                "geod_job_control",
                                            ]
                                            .contains(&name)
                                            {
                                                policy
                                                    .call(
                                                        manager,
                                                        session,
                                                        &binding,
                                                        name,
                                                        args.clone(),
                                                        execution::TurnScope {
                                                            request_id: value["params"]
                                                                ["requestId"]
                                                                .as_str(),
                                                            read_only: value["params"]["readOnly"]
                                                                == true,
                                                        },
                                                    )
                                                    .await
                                            } else if name == "geod_workspace_open" {
                                                workspace::open(manager, session, args.clone())
                                                    .await
                                            } else {
                                                agent_actions::call(
                                                    manager,
                                                    session,
                                                    name,
                                                    args.clone(),
                                                    context,
                                                )
                                                .await
                                            }
                                        }
                                        Err(_) => Err("Invalid Agent map context.".into()),
                                    }
                                }
                                _ => Err("Invalid or concurrent GeoD tool arguments".into()),
                            };
                            let value = match response {
                                Ok(result) => json!({"id":value["id"],"result":result}),
                                Err(error) => {
                                    let message = if error.len() <= 240
                                        && !error.contains(['\\', '/', '\n', '\r'])
                                        && !error.contains(":")
                                    {
                                        error
                                    } else {
                                        "GeoD tool could not finish. Check the source, area and local file status.".into()
                                    };
                                    json!({"id":value["id"],"error":{"message":message}})
                                }
                            };
                            let _ = writer.send(Write::Json(value)).await;
                        });
                    } else if let Some(id) = value["id"].as_u64() {
                        if let Some(sender) = pending.lock().await.remove(&id) {
                            let response = if value.get("error").is_some() {
                                Err(value["error"]["message"]
                                    .as_str()
                                    .unwrap_or("Agent operation failed.")
                                    .to_owned())
                            } else {
                                Ok(value["result"].clone())
                            };
                            let _ = sender.send(response);
                        }
                    }
                }
                if closed.load(Ordering::Acquire) {
                    break;
                }
            }
            closed.store(true, Ordering::Release);
            for (_, sender) in pending.lock().await.drain() {
                let _ = sender.send(Err("Agent runtime stopped.".into()));
            }
        });
        Ok(connection)
    }
    async fn rpc(&self, method: &str, params: Value) -> Result<Value, String> {
        if self.closed.load(Ordering::Acquire) {
            return Err("Agent runtime stopped.".into());
        }
        let id = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let (sender, receiver) = oneshot::channel();
        self.pending.lock().await.insert(id, sender);
        if self
            .writer
            .send(Write::Json(
                json!({"id":id,"method":method,"params":params}),
            ))
            .await
            .is_err()
        {
            self.pending.lock().await.remove(&id);
            return Err("Agent runtime stopped.".into());
        }
        let response = tokio::time::timeout(Duration::from_secs(30), receiver).await;
        self.pending.lock().await.remove(&id);
        response
            .map_err(|_| "Agent runtime request timed out.")?
            .map_err(|_| "Agent runtime stopped.")?
    }
    async fn close(&self) {
        let _ = self.writer.send(Write::Close).await;
        let mut child = self.child.lock().await;
        if tokio::time::timeout(Duration::from_secs(5), child.wait())
            .await
            .is_err()
        {
            let _ = child.kill().await;
        }
        self.closed.store(true, Ordering::Release);
    }
}

fn child_path(path: &std::path::Path) -> PathBuf {
    #[cfg(windows)]
    if let Some(value) = path.to_str() {
        if let Some(unc) = value.strip_prefix("\\\\?\\UNC\\") {
            return PathBuf::from(format!("\\\\{unc}"));
        }
        if let Some(drive) = value.strip_prefix("\\\\?\\") {
            return PathBuf::from(drive);
        }
    }
    path.to_owned()
}

#[cfg(windows)]
struct ProcessJob(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
unsafe impl Send for ProcessJob {}
#[cfg(windows)]
unsafe impl Sync for ProcessJob {}
#[cfg(windows)]
impl ProcessJob {
    fn attach(child: &Child) -> Result<Self, String> {
        use windows_sys::Win32::System::JobObjects::*;
        let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if job.is_null() {
            return Err("Agent process isolation could not start.".into());
        }
        let handle = Self(job);
        let mut information: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        information.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let success = unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &information as *const _ as *const _,
                std::mem::size_of_val(&information) as u32,
            )
        };
        if success == 0
            || unsafe {
                AssignProcessToJobObject(
                    job,
                    child
                        .raw_handle()
                        .ok_or("Agent process isolation could not start.")?
                        as _,
                )
            } == 0
        {
            return Err("Agent process isolation could not start.".into());
        }
        Ok(handle)
    }
}
#[cfg(windows)]
impl Drop for ProcessJob {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(all(test, windows))]
#[path = "agent_revision_tests.rs"]
mod revision_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renderer_notifications_accept_only_a_bounded_revision() {
        assert_eq!(
            change_revision(&json!({"method":"geod.changed","params":{"revision":42}})),
            Some(42)
        );
        for invalid in [
            json!({"id":1,"method":"geod.changed","params":{"revision":42}}),
            json!({"method":"geod.changed","params":{"revision":-1}}),
            json!({"method":"geod.changed","params":{"revision":9007199254740992u64}}),
            json!({"method":"geod.changed","params":{"revision":42,"text":"private transcript"}}),
            json!({"method":"geod.tool","params":{"revision":42}}),
        ] {
            assert_eq!(change_revision(&invalid), None);
        }
    }
    #[test]
    fn endpoints_and_model_settings_are_bounded() {
        let mut value = ModelSettings {
            label: "Test".into(),
            protocol: "openai-compatible".into(),
            base_url: "https://api.example.com/v1".into(),
            model: "test-model".into(),
        };
        assert!(validate_settings(&value).is_ok());
        for invalid in [
            "http://api.example.com/v1",
            "https://user:secret@api.example.com/v1",
            "https://api.example.com/v1?key=secret",
            "file:///tmp/api",
            "http://localhost/v1",
        ] {
            value.base_url = invalid.into();
            assert!(validate_settings(&value).is_err());
        }
        value.base_url = "http://127.0.0.1:19094/v1".into();
        assert!(validate_settings(&value).is_ok());
        value.model = "model\n[provider]".into();
        assert!(validate_settings(&value).is_err());
    }
    #[test]
    fn settings_never_serialize_secret() {
        let value: ModelRequest = serde_json::from_value(json!({"label":"Test","protocol":"openai-compatible","baseUrl":"https://api.example.com/v1","model":"test","apiKey":"private-secret"})).unwrap();
        assert!(!serde_json::to_string(&value.settings())
            .unwrap()
            .contains("private-secret"));
        assert!(serde_json::from_value::<ModelSettings>(json!({"label":"Test","protocol":"openai-compatible","baseUrl":"https://api.example.com/v1","model":"test","apiKey":"private-secret"})).is_err());
    }

    #[tokio::test]
    async fn desktop_registry_persists_private_connections_and_public_selection_without_model_calls(
    ) {
        #[derive(Default)]
        struct MarkerVault(std::sync::Mutex<HashMap<String, String>>);
        impl registry::Vault for MarkerVault {
            fn read(&self, id: &str) -> Result<Option<zeroize::Zeroizing<String>>, String> {
                Ok(self
                    .0
                    .lock()
                    .unwrap()
                    .get(id)
                    .cloned()
                    .map(zeroize::Zeroizing::new))
            }
            fn write(&self, id: &str, key: &str) -> Result<(), String> {
                self.0.lock().unwrap().insert(id.into(), key.into());
                Ok(())
            }
            fn delete(&self, id: &str) -> Result<(), String> {
                self.0.lock().unwrap().remove(id);
                Ok(())
            }
        }
        let directory = tempfile::tempdir().unwrap();
        let manager = JobManager::open(directory.path().join("core"))
            .await
            .unwrap();
        let vault = Arc::new(MarkerVault::default());
        let home = directory.path().join("agent");
        let runtime = directory.path().join("missing-optional-runtime");
        let agent = DesktopAgent::open_with_vault(
            home.clone(),
            runtime.clone(),
            manager.clone(),
            vault.clone(),
        )
        .await
        .unwrap();
        let save = |label: &str, key: &str| {
            serde_json::from_value(json!({"provider":"custom","label":label,"protocol":"openai-compatible","baseUrl":"https://example.test/v1","model":"test-model","apiKey":key})).unwrap()
        };
        let first = agent
            .save_model(save("First", "synthetic-first-marker"))
            .await
            .unwrap();
        let id = first["registry"]["selectedId"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(first["runtimeAvailable"], false);
        assert_eq!(first["configured"], false);
        assert_eq!(first["model"]["verification"], "not-verified");
        let second = agent
            .save_model(save("Second", "synthetic-second-marker"))
            .await
            .unwrap();
        assert_eq!(
            second["registry"]["connections"].as_array().unwrap().len(),
            2
        );
        assert_eq!(vault.0.lock().unwrap().len(), 2);
        let selected = agent
            .save_model(serde_json::from_value(json!({"action":"select","id":id})).unwrap())
            .await
            .unwrap();
        assert_eq!(selected["model"]["id"], id);
        assert_eq!(selected["model"]["label"], "First");
        assert!(!selected.to_string().contains("synthetic-"));
        assert!(!selected.to_string().contains("credentialRef"));
        agent.shutdown().await;
        let reopened = DesktopAgent::open_with_vault(home, runtime, manager.clone(), vault.clone())
            .await
            .unwrap();
        assert_eq!(
            reopened.snapshot().await.unwrap()["registry"]["selectedId"],
            id
        );
        let after = reopened
            .save_model(serde_json::from_value(json!({"action":"delete","id":id})).unwrap())
            .await
            .unwrap();
        assert_eq!(after["model"]["label"], "Second");
        assert_eq!(vault.0.lock().unwrap().len(), 1);
        assert!(manager.list().await.is_empty());
        reopened.shutdown().await;
        manager.shutdown().await.unwrap();
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn source_account_status_reads_the_open_desktop_core_without_authorizing() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = std::env::var_os("GEOD_AGENT_ACCOUNT_TEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| temporary.path().to_owned());
        let manager = JobManager::open(directory.join("core")).await.unwrap();
        let before = serde_json::to_value(manager.provider_accounts().await).unwrap();
        let session = "a1234567-1234-1234-1234-123456789abc".to_string();
        let result = agent_actions::call(
            manager.clone(),
            &session,
            "geod_sources_list",
            json!({}),
            None,
        )
        .await
        .unwrap();
        assert_eq!(result["sources"].as_array().unwrap().len(), 15);
        for account in result["accountSources"].as_array().unwrap() {
            assert_eq!(account["authorization"]["status"], "not-connected");
            assert_eq!(account["agentDownload"], true);
            assert_eq!(account["downloadEnabled"], false);
            assert_eq!(account["entitlement"], "not-checked");
        }
        for tool in [
            "geod_connect_account",
            "geod_verify_account",
            "geod_disconnect_account",
        ] {
            assert!(
                agent_actions::call(manager.clone(), &session, tool, json!({}), None)
                    .await
                    .is_err()
            );
        }
        assert!(manager.list().await.is_empty());
        assert_eq!(
            serde_json::to_value(manager.provider_accounts().await).unwrap(),
            before
        );
        if std::env::var_os("GEOD_AGENT_ACCOUNT_TEST_DIR").is_some() {
            let receipt = json!({"schema":"geod-agent-native-account-acceptance/v1","status":"passed","result":result,"modelCalls":0,"usedUserDesktop":false,"credentialVaultWritten":false,"accountMutationsRefused":true,"jobsCreated":0});
            tokio::fs::write(
                directory.join("native-acceptance.json"),
                serde_json::to_vec_pretty(&receipt).unwrap(),
            )
            .await
            .unwrap();
        }
        manager.shutdown().await.unwrap();
    }

    // Explicit, paid/live development acceptance. Never part of ordinary CI,
    // never touches the user's desktop, data directory or saved model account.
    #[cfg(windows)]
    #[tokio::test]
    #[ignore = "requires owned Agent runtime, isolated real file and explicit test model credentials"]
    async fn live_agent_reads_native_data_and_resumes_after_process_restart() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .canonicalize()
            .unwrap();
        let runtime = root.join(".agent-runtime/win32-x64");
        verify_runtime(&runtime).await.unwrap();
        let core = root.join(".verification/agent-native-20261004/core");
        let home = root.join(".verification/agent-native-20261004/sessions");
        tokio::fs::create_dir_all(&home).await.unwrap();
        let manager = JobManager::open(core).await.unwrap();
        let nonce = format!("Agent acceptance {}", uuid_for_test());
        let request = json!({"name":nonce,"bounds":[-123.0,37.0,-122.0,38.0],"scenes":[{"itemId":"S2C_TEST","date":"2026-09-28T00:00:00Z","cloud":0,"crs":"EPSG:32610","bbox":[-123.0,37.0,-122.0,38.0],"assets":{"scl":{"href":"https://sentinel-cogs.s3.us-west-2.amazonaws.com/sentinel-s2-l2a-cogs/10/S/EG/2026/9/S2C_TEST/SCL.tif","mediaType":"image/tiff; application=geotiff"}}}]});
        let project = manager
            .create_project(serde_json::from_value(request).unwrap())
            .await
            .unwrap();
        let job = manager
            .list()
            .await
            .into_iter()
            .find(|job| job.asset_key == "scl" && job.status == geod_runtime::JobStatus::Succeeded)
            .unwrap();
        let secret = Zeroizing::new(
            std::env::var("GEOD_AGENT_TEST_KEY").expect("Explicit test credential required"),
        );
        let config = json!({"label":"Isolated development verification","protocol":"openai-compatible","baseUrl":"http://127.0.0.1:19094/v1","model":"deepseek-v4-flash","apiKey":secret.as_str()});
        let mut connection = Connection::spawn(&runtime, &home, manager.clone())
            .await
            .unwrap();
        if let Err(error) = connection
            .rpc(
                "configure",
                json!({"config":config,"definitions":mcp::agent_read_definitions()}),
            )
            .await
        {
            panic!(
                "{error}; test-only redacted launch diagnostic: {}",
                connection.diagnostics.lock().await
            );
        }
        let first = connection.rpc("send", json!({"text":format!("Call geod_project_get with id {} to read the saved project name, then geod_job_status and geod_raster_inspect with id {}. Reply with ONLY the saved project name, the exact file SHA-256, and whether status=succeeded AND settled=true. Do not invent values or run other tools.",project.id,job.id)})).await.unwrap();
        let id = first["selected"]["id"].as_str().unwrap().to_owned();
        let first = completed(&connection).await;
        assert_eq!(
            first["selected"]["status"],
            "completed",
            "{}; test-only diagnostic: {}",
            first["selected"]["error"],
            connection.diagnostics.lock().await
        );
        let text = assistant_text(&first);
        assert!(text.contains(&nonce));
        assert!(text.contains(job.sha256.as_deref().unwrap()));
        let names: Vec<_> = first["selected"]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|entry| entry["name"].as_str())
            .collect();
        for tool in ["geod_project_get", "geod_job_status", "geod_raster_inspect"] {
            assert!(names.contains(&tool));
        }
        let thread_id = first["selected"]["threadId"].clone();
        connection.close().await;
        drop(connection);
        manager
            .rename_project(&project.id, "Agent native restart receipt")
            .await
            .unwrap();
        connection = Connection::spawn(&runtime, &home, manager.clone())
            .await
            .unwrap();
        connection
            .rpc(
                "configure",
                json!({"config":config,"definitions":mcp::agent_read_definitions()}),
            )
            .await
            .unwrap();
        connection.rpc("send", json!({"sessionId":id,"text":"The saved project name has changed. Use the same project ID from our previous conversation and call geod_project_get again. Reply with ONLY its current saved name. Do not reuse the old name."})).await.unwrap();
        let second = completed(&connection).await;
        assert_eq!(second["selected"]["status"], "completed");
        assert_eq!(second["selected"]["threadId"], thread_id);
        assert!(assistant_text(&second).ends_with("Agent native restart receipt"));
        assert!(!second.to_string().contains(secret.as_str()));
        connection.close().await;
        drop(connection);
        let receipt = json!({"schema":"geod-agent-native-acceptance/v1","modelRoute":"deepseek-v4-flash","upstreamVendorVerified":false,
            "runtimeVersions":{"codex":"0.159.2","node":"24.14.0","aiSdk":"7.0.127"},"modelCalls":"bounded live calls through local SSH tunnel",
            "projectMetadata":"locally created acceptance project, not a downloaded scene","raster":"existing real Sentinel SCL copy verified by native SHA-256 inspection",
            "firstTurn":"completed","nativeTools":names,"jobId":job.id,"sha256":job.sha256,
            "processRestartAndThreadResume":true,"freshNativeResultAfterRestart":true,"usedUserDesktop":false,"installedWebViewTested":false,
            "credentialVaultWritten":false,"readonly":true,"status":"passed"});
        tokio::fs::write(
            home.parent().unwrap().join("native-acceptance.json"),
            serde_json::to_vec_pretty(&receipt).unwrap(),
        )
        .await
        .unwrap();
        println!("Agent native acceptance passed: actual file inspection, live model tool chain, process restart and fresh saved-project result.");
        manager.shutdown().await.unwrap();
    }
    #[cfg(windows)]
    pub(super) fn uuid_for_test() -> String {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string()
    }
    #[cfg(windows)]
    #[tokio::test]
    #[ignore = "explicit model key, owned runtime and live public acquisition in isolated store"]
    async fn live_agent_search_plan_confirm_and_inspect() {
        let secret =
            Zeroizing::new(std::env::var("GEOD_AGENT_TEST_KEY").expect("explicit test key"));
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .canonicalize()
            .unwrap();
        let base = root
            .join(".verification")
            .join(format!("agent-workflow-{}", uuid_for_test()));
        let home = base.join("sessions");
        tokio::fs::create_dir_all(&home).await.unwrap();
        let manager = JobManager::open(base.join("core")).await.unwrap();
        if let Ok(proxy) = std::env::var("GEOD_AGENT_TEST_PROXY") {
            manager
                .save_proxy_settings(geod_runtime::ProxySettings {
                    mode: geod_runtime::proxy::ProxyMode::Custom,
                    url: Some(proxy),
                })
                .await
                .unwrap();
        }
        let runtime = root.join(".agent-runtime/win32-x64");
        verify_runtime(&runtime).await.unwrap();
        let connection = Connection::spawn(&runtime, &home, manager.clone())
            .await
            .unwrap();
        connection.rpc("configure",json!({"config":{"label":"Isolated workflow acceptance","protocol":"openai-compatible","baseUrl":"http://127.0.0.1:19094/v1","model":"deepseek-v4-flash","apiKey":secret.as_str()},"definitions":definitions()})).await.unwrap();
        let agent = DesktopAgent::open(home.clone(), runtime, manager.clone())
            .await
            .unwrap();
        agent.0.state.lock().await.connection = Some(connection.clone());
        let first = agent.operation("send",json!({"text":"请读取当前地图区域，按当前区域和日期从 Earth Search 查询最多 3 景，然后选择其中一景 SCL 生成下载计划。不要下载或修改文件。回答里给出计划编号和预估文件大小，等待我在计划卡片确认。","context":{"page":"Explore","provider":"earth-search","bounds":[-122.55,37.68,-122.32,37.84],"start":"2025-06-01","end":"2025-06-30","cloudMax":60,"projectId":null}})).await.unwrap();
        let session = first["selected"]["id"].as_str().unwrap().to_string();
        let finished = completed(&connection).await;
        assert_eq!(
            finished["selected"]["status"],
            "completed",
            "{}; diagnostic: {}",
            finished["selected"]["error"],
            connection.diagnostics.lock().await
        );
        let reviewed = agent.snapshot().await.unwrap();
        let plan = reviewed["plans"][0].clone();
        assert_eq!(
            plan["status"],
            "pending",
            "No native plan: {}",
            assistant_text(&finished)
        );
        assert_eq!(plan["files"].as_array().unwrap().len(), 1);
        assert!(manager.list().await.is_empty());
        let plan_id = plan["planId"].as_str().unwrap();
        let plan_hash = plan["planHash"].as_str().unwrap();
        assert!(agent
            .approve_plan(&session, plan_id, &"0".repeat(64))
            .await
            .is_err());
        let accepted = agent
            .approve_plan(&session, plan_id, plan_hash)
            .await
            .unwrap();
        let download_id = accepted["plans"][0]["jobs"][0]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let output = tokio::time::timeout(Duration::from_secs(120), manager.wait(&download_id))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            output.status,
            geod_runtime::JobStatus::Succeeded,
            "{:?}",
            output.error
        );
        let sha = output.sha256.clone().unwrap();
        manager.inspect_raster(&download_id).await.unwrap();
        // Repeating the exact native confirmation returns the same queue receipt.
        let duplicate = agent
            .approve_plan(&session, plan_id, plan_hash)
            .await
            .unwrap();
        assert_eq!(duplicate["plans"][0]["jobs"][0]["id"], download_id);
        assert_eq!(manager.list().await.len(), 1);
        agent.operation("send",json!({"sessionId":session,"text":format!("请调用 geod_plan_status 查看计划 {plan_id} 的真实任务结果，然后 geod_raster_inspect 检查完成的任务。只回答实际任务编号、是否 settled、SHA-256 和文件大小。不要再生成计划或提交任务。"),"context":null})).await.unwrap();
        let final_turn = completed(&connection).await;
        assert_eq!(
            final_turn["selected"]["status"], "completed",
            "{}",
            final_turn["selected"]["error"]
        );
        assert!(assistant_text(&final_turn).contains(&sha));
        assert_eq!(manager.list().await.len(), 1);
        let mut final_snapshot = agent.snapshot().await.unwrap();
        final_snapshot["configured"] = json!(false);
        final_snapshot["model"] = Value::Null;
        assert!(!final_snapshot.to_string().contains(secret.as_str()));
        let receipt = json!({"schema":"geod-agent-workflow-acceptance/v1","status":"passed","modelRoute":"deepseek-v4-flash","upstreamVendorVerified":false,"nativeTools":["geod_workspace_context","geod_scene_search","geod_download_plan","geod_plan_status","geod_raster_inspect"],"plan":plan,"pendingBeforeExplicitConfirmation":true,"duplicateConfirmationReusesJob":true,"jobId":download_id,"sha256":sha,"bytes":output.bytes_downloaded,"usedUserDesktop":false,"credentialVaultWritten":false,"snapshot":final_snapshot});
        tokio::fs::write(
            base.join("native-acceptance.json"),
            serde_json::to_vec_pretty(&receipt).unwrap(),
        )
        .await
        .unwrap();
        tokio::fs::write(
            root.join(".verification/agent-workflow-latest.json"),
            serde_json::to_vec_pretty(&json!({"directory":base})).unwrap(),
        )
        .await
        .unwrap();
        agent.shutdown().await;
        drop(connection);
        manager.shutdown().await.unwrap();
        println!("Agent workflow acceptance passed: live catalog, model-created native plan, explicit confirmation, real SCL checksum, no duplicate submission.");
    }
    #[cfg(windows)]
    #[tokio::test]
    #[ignore = "explicit model key; real project/search/download/processing in an isolated owned runtime"]
    async fn live_agent_project_download_process_and_resume() {
        let secret =
            Zeroizing::new(std::env::var("GEOD_AGENT_TEST_KEY").expect("explicit test key"));
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .canonicalize()
            .unwrap();
        let base = root
            .join(".verification")
            .join(format!("agent-project-{}", uuid_for_test()));
        let home = base.join("sessions");
        tokio::fs::create_dir_all(&home).await.unwrap();
        let manager = JobManager::open(base.join("core")).await.unwrap();
        if let Ok(proxy) = std::env::var("GEOD_AGENT_TEST_PROXY") {
            manager
                .save_proxy_settings(geod_runtime::ProxySettings {
                    mode: geod_runtime::proxy::ProxyMode::Custom,
                    url: Some(proxy),
                })
                .await
                .unwrap();
        }
        let runtime = root.join(".agent-runtime/win32-x64");
        verify_runtime(&runtime).await.unwrap();
        let connection = Connection::spawn(&runtime, &home, manager.clone())
            .await
            .unwrap();
        connection.rpc("configure",json!({"config":{"label":"Isolated project acceptance","protocol":"openai-compatible","baseUrl":"http://127.0.0.1:19094/v1","model":"deepseek-v4-flash","apiKey":secret.as_str()},"definitions":definitions()})).await.unwrap();
        let agent = DesktopAgent::open(home.clone(), runtime.clone(), manager.clone())
            .await
            .unwrap();
        agent.0.state.lock().await.connection = Some(connection.clone());
        let source = std::env::var("GEOD_AGENT_TEST_SOURCE").unwrap_or("earth-search".into());
        assert!(["earth-search", "planetary-computer"].contains(&source.as_str()));
        let context = json!({"page":"Explore","provider":source,"bounds":[-122.46,37.76,-122.45,37.77],"start":"2025-06-01","end":"2025-06-30","cloudMax":60,"projectId":null});
        let initial=agent.operation("send",json!({"text":format!("请读取当前地图区域与日期，先查询数据源能力，再搜索 {source} 最多 3 景，选择一景，生成名为 Agent 实际工程验收 的新工程选景计划。只调用原生计划工具，不下载、不创建工程，等我确认工程卡片。"),"context":context})).await.unwrap();
        let session = initial["selected"]["id"].as_str().unwrap().to_string();
        let first = completed(&connection).await;
        assert_eq!(
            first["selected"]["status"],
            "completed",
            "{}",
            assistant_text(&first)
        );
        let pending_project = agent.snapshot().await.unwrap();
        let project_plan = pending_project["plans"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["kind"] == "project")
            .expect("actual model project plan")
            .clone();
        assert_eq!(project_plan["status"], "pending");
        assert!(manager.list_projects().await.is_empty());
        assert!(manager.list().await.is_empty());
        let confirmed = agent
            .approve_plan(
                &session,
                project_plan["planId"].as_str().unwrap(),
                project_plan["planHash"].as_str().unwrap(),
            )
            .await
            .unwrap();
        let project_id = confirmed["plans"][0]["project"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(manager.list_projects().await.len(), 1);
        assert!(manager.list().await.is_empty());
        agent.operation("send",json!({"sessionId":session,"text":format!("工程 {project_id} 已通过卡片确认保存。请用 geod_project_get 查看真实工程，再用 geod_project_download_plan 为它生成 SCL 下载计划。只生成计划，等待确认。"),"context":null})).await.unwrap();
        let second = completed(&connection).await;
        assert_eq!(
            second["selected"]["status"],
            "completed",
            "{}",
            assistant_text(&second)
        );
        let pending_download = agent.snapshot().await.unwrap();
        let download_plan = pending_download["plans"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["kind"] == "download")
            .expect("actual project download plan")
            .clone();
        assert_eq!(download_plan["project"]["id"], project_id);
        assert!(manager.list().await.is_empty());
        let submitted = agent
            .approve_plan(
                &session,
                download_plan["planId"].as_str().unwrap(),
                download_plan["planHash"].as_str().unwrap(),
            )
            .await
            .unwrap();
        let submitted_plan = submitted["plans"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["planId"] == download_plan["planId"])
            .unwrap();
        let download_id = submitted_plan["jobs"][0]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let source = tokio::time::timeout(Duration::from_secs(120), manager.wait(&download_id))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            source.status,
            geod_runtime::JobStatus::Succeeded,
            "{:?}",
            source.error
        );
        manager.inspect_raster(&download_id).await.unwrap();
        agent.operation("send",json!({"sessionId":session,"text":format!("请读取工程 {project_id} 的实际任务结果；完成后，用 geod_project_mosaic_plan 为这个工程的 SCL 生成按工程区域处理的计划。不要创建重复下载，等待处理卡片确认。"),"context":null})).await.unwrap();
        let third = completed(&connection).await;
        assert_eq!(
            third["selected"]["status"],
            "completed",
            "{}",
            assistant_text(&third)
        );
        let pending_processing = agent.snapshot().await.unwrap();
        let processing_plan = pending_processing["plans"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["kind"] == "mosaic")
            .expect("actual model processing plan")
            .clone();
        assert_eq!(processing_plan["status"], "pending");
        assert_eq!(manager.list().await.len(), 1);
        let processed = agent
            .approve_plan(
                &session,
                processing_plan["planId"].as_str().unwrap(),
                processing_plan["planHash"].as_str().unwrap(),
            )
            .await
            .unwrap();
        let processed_plan = processed["plans"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["planId"] == processing_plan["planId"])
            .unwrap();
        let output_id = processed_plan["jobs"][0]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let output = tokio::time::timeout(Duration::from_secs(120), manager.wait(&output_id))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            output.status,
            geod_runtime::JobStatus::Succeeded,
            "{:?}",
            output.error
        );
        let raster = manager.inspect_raster(&output_id).await.unwrap();
        assert_eq!(
            raster.width as u64,
            processing_plan["files"][0]["width"].as_u64().unwrap()
        );
        assert_eq!(
            raster.height as u64,
            processing_plan["files"][0]["height"].as_u64().unwrap()
        );
        agent.operation("send",json!({"sessionId":session,"text":format!("请读取处理计划 {} 的 geod_plan_status，再用 geod_raster_inspect 检查处理输出任务 {output_id}。报告原生任务是否 settled、SHA-256 和输出尺寸；不要生成新计划或任务。",processing_plan["planId"].as_str().unwrap()),"context":null})).await.unwrap();
        let fourth = completed(&connection).await;
        assert_eq!(fourth["selected"]["status"], "completed");
        assert!(assistant_text(&fourth).contains(output.sha256.as_ref().unwrap()));
        let final_snapshot = agent.snapshot().await.unwrap();
        let receipt = json!({"schema":"geod-agent-project-acceptance/v1","status":"passed","modelRoute":"deepseek-v4-flash","upstreamVendorVerified":false,"pendingProject":pending_project,"pendingDownload":pending_download,"pendingProcessing":pending_processing,"snapshot":final_snapshot,"projectId":project_id,"downloadId":download_id,"downloadSha256":source.sha256,"downloadBytes":source.bytes_downloaded,"outputId":output_id,"outputSha256":output.sha256,"outputBytes":output.bytes_downloaded,"width":raster.width,"height":raster.height,"scope":"actual public catalog and original file; native confirmations and output inspection; not an installed WebView check","usedUserDesktop":false,"credentialVaultWritten":false});
        assert!(!receipt.to_string().contains(secret.as_str()));
        agent.shutdown().await;
        drop(connection);
        drop(agent);
        manager.shutdown().await.unwrap();
        drop(manager);
        let manager = JobManager::open(base.join("core")).await.unwrap();
        for plan in [&project_plan, &download_plan, &processing_plan] {
            manager
                .approve_agent_plan(
                    &session,
                    plan["planId"].as_str().unwrap(),
                    plan["planHash"].as_str().unwrap(),
                )
                .await
                .unwrap();
        }
        assert_eq!(manager.list_projects().await.len(), 1);
        assert_eq!(manager.list().await.len(), 2);
        manager.shutdown().await.unwrap();
        tokio::fs::write(
            base.join("native-acceptance.json"),
            serde_json::to_vec_pretty(&receipt).unwrap(),
        )
        .await
        .unwrap();
        tokio::fs::write(
            root.join(".verification/agent-project-latest.json"),
            serde_json::to_vec_pretty(&json!({"directory":base})).unwrap(),
        )
        .await
        .unwrap();
        println!("Agent project acceptance passed: actual model search, project confirmation, original SCL download, native project processing, checksum report and restart idempotency.");
    }

    #[cfg(windows)]
    #[tokio::test]
    #[ignore = "explicit model key; copied real source cohorts, isolated native approvals and scientific outputs"]
    async fn live_agent_scientific_rgb_and_coupled_vi() {
        let secret =
            Zeroizing::new(std::env::var("GEOD_AGENT_TEST_KEY").expect("explicit test key"));
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .canonicalize()
            .unwrap();
        let base = PathBuf::from(
            std::env::var("GEOD_AGENT_SCIENCE_DIR").expect("prepared isolated source store"),
        )
        .canonicalize()
        .unwrap();
        assert_eq!(base.parent().unwrap(), root.join(".verification"));
        assert!(base
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("agent-science-"));
        let inputs: Value =
            serde_json::from_slice(&tokio::fs::read(base.join("inputs.json")).await.unwrap())
                .unwrap();
        let manager = JobManager::open(base.join("core")).await.unwrap();
        let initial_count = manager.list().await.len();
        let home = base.join("sessions");
        tokio::fs::create_dir_all(&home).await.unwrap();
        let runtime = root.join(".agent-runtime/win32-x64");
        verify_runtime(&runtime).await.unwrap();
        let connection = Connection::spawn(&runtime, &home, manager.clone())
            .await
            .unwrap();
        connection.rpc("configure",json!({"config":{"label":"Isolated scientific acceptance","protocol":"openai-compatible","baseUrl":"http://127.0.0.1:19094/v1","model":"deepseek-v4-flash","apiKey":secret.as_str()},"definitions":definitions()})).await.unwrap();
        let agent = DesktopAgent::open(home.clone(), runtime.clone(), manager.clone())
            .await
            .unwrap();
        agent.0.state.lock().await.connection = Some(connection.clone());
        let sent=agent.operation("send",json!({"text":format!("使用已保存的真实本地文件，先查询任务状态，再生成 3 个待确认计划：1. 按指定参数 {} 生成科学 RGB；2. 为工程 {} 的 ndvi 和 evi 分别生成处理计划，viQuality policy 选 good。必须使用原生 geod_scientific_rgb_plan 和 geod_project_mosaic_plan。只生成计划，不执行，不下载任何文件，等我确认卡片。",inputs["rgbRequest"],inputs["viProjectId"].as_str().unwrap()),"context":null})).await.unwrap();
        let session = sent["selected"]["id"].as_str().unwrap().to_owned();
        let first = completed(&connection).await;
        assert_eq!(
            first["selected"]["status"],
            "completed",
            "{}",
            assistant_text(&first)
        );
        let pending = agent.snapshot().await.unwrap();
        let plans = pending["plans"].as_array().unwrap().clone();
        assert_eq!(plans.len(), 3, "{pending}");
        assert_eq!(manager.list().await.len(), initial_count);
        assert_eq!(plans.iter().filter(|p| p["kind"] == "rgb").count(), 1);
        for plan in &plans {
            assert_eq!(plan["status"], "pending");
            assert!(plan["processing"]["quality"].is_object());
            if plan["kind"] == "rgb" {
                assert_eq!(
                    plan["processing"]["quality"]["policy"],
                    "cloud_free_conservative"
                );
            } else {
                assert_eq!(plan["processing"]["quality"]["policy"], "good");
            }
        }
        let mut output_ids = Vec::new();
        for plan in &plans {
            let submitted = agent
                .approve_plan(
                    &session,
                    plan["planId"].as_str().unwrap(),
                    plan["planHash"].as_str().unwrap(),
                )
                .await
                .unwrap();
            let current = submitted["plans"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["planId"] == plan["planId"])
                .unwrap();
            let id = current["jobs"][0]["id"].as_str().unwrap().to_owned();
            let job = tokio::time::timeout(Duration::from_secs(180), manager.wait(&id))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                job.status,
                geod_runtime::JobStatus::Succeeded,
                "{:?}",
                job.error
            );
            output_ids.push(id);
        }
        assert_eq!(manager.list().await.len(), initial_count + 3);
        let rgb = manager
            .get(&output_ids[plans.iter().position(|p| p["kind"] == "rgb").unwrap()])
            .await
            .unwrap();
        let rgb_inspection = manager.inspect_scientific_rgb(&rgb.id).await.unwrap();
        assert_eq!(
            rgb_inspection.artifact.unwrap().sha256,
            rgb.sha256.clone().unwrap()
        );
        let mut selection = Vec::new();
        for id in &output_ids {
            let j = manager.get(id).await.unwrap();
            if let Some(output) = &j.mosaic_output {
                selection.push(output.vi_quality.as_ref().unwrap().selection_sha256.clone());
            }
        }
        assert_eq!(selection.len(), 2);
        assert_eq!(selection[0], selection[1]);
        let output_plans = plans
            .iter()
            .zip(&output_ids)
            .map(|(plan, job)| json!({"planId":plan["planId"],"jobId":job}))
            .collect::<Vec<_>>();
        agent.operation("send",json!({"sessionId":session,"text":format!("三个卡片已确认，计划及任务 {}。请用 planId 调用 geod_plan_status 查询实际完成状态，用 jobId 对科学 RGB 调用 geod_rgb_inspect，对 NDVI/EVI 调用 geod_raster_inspect。报告完成任务、文件 SHA-256、原始尺寸、质量规则和同一观测筛选结果，不生成新计划。",json!(output_plans)),"context":null})).await.unwrap();
        let second = completed(&connection).await;
        assert_eq!(
            second["selected"]["status"],
            "completed",
            "{}",
            assistant_text(&second)
        );
        let snapshot = agent.snapshot().await.unwrap();
        assert!(snapshot["selected"]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["name"] == "geod_rgb_inspect" && e["status"] == "completed"));
        assert!(assistant_text(&snapshot).contains(rgb.sha256.as_ref().unwrap()));
        let receipt = json!({"schema":"geod-agent-science-acceptance/v1","status":"passed","modelRoute":"deepseek-v4-flash","upstreamVendorVerified":false,"pending":pending,"snapshot":snapshot,"outputIds":output_ids,"initialJobs":initial_count,"viSelectionSha256":selection[0],"rgbId":rgb.id,"rgbSha256":rgb.sha256,"scope":"Actual previously downloaded originals; isolated native/model processing and confirmations; no new transfers","usedUserDesktop":false,"credentialVaultWritten":false});
        assert!(!receipt.to_string().contains(secret.as_str()));
        agent.shutdown().await;
        drop(connection);
        drop(agent);
        manager.shutdown().await.unwrap();
        drop(manager);
        let reopened = JobManager::open(base.join("core")).await.unwrap();
        for p in &plans {
            reopened
                .approve_agent_plan(
                    &session,
                    p["planId"].as_str().unwrap(),
                    p["planHash"].as_str().unwrap(),
                )
                .await
                .unwrap();
        }
        assert_eq!(reopened.list().await.len(), initial_count + 3);
        reopened.shutdown().await.unwrap();
        tokio::fs::write(
            base.join("native-acceptance.json"),
            serde_json::to_vec_pretty(&receipt).unwrap(),
        )
        .await
        .unwrap();
        tokio::fs::write(
            root.join(".verification/agent-science-latest.json"),
            serde_json::to_vec_pretty(&json!({"directory":base})).unwrap(),
        )
        .await
        .unwrap();
        println!("Scientific Agent acceptance passed: model-created RGB/NDVI/EVI cards, real source QA, native outputs and restart idempotency.");
    }

    #[cfg(windows)]
    pub(super) fn assistant_text(snapshot: &Value) -> String {
        snapshot["selected"]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| entry["type"] == "assistant")
            .filter_map(|entry| entry["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
    #[cfg(windows)]
    pub(super) async fn completed(connection: &Connection) -> Value {
        tokio::time::timeout(Duration::from_secs(220), async {
            loop {
                let snapshot = connection.rpc("snapshot", json!({})).await.unwrap();
                if snapshot["busy"] == false {
                    return snapshot;
                }
                tokio::time::sleep(Duration::from_millis(350)).await;
            }
        })
        .await
        .expect("Bounded Agent acceptance turn timed out")
    }
}

#[cfg(all(test, windows))]
#[path = "agent_stac_tests.rs"]
mod stac_model_tests;

#[cfg(all(test, windows))]
#[path = "agent_vector_tests.rs"]
mod vector_model_tests;

#[cfg(test)]
#[path = "agent_wcs_tests.rs"]
mod wcs_model_tests;

#[cfg(all(test, windows))]
#[path = "agent_protected_tests.rs"]
mod protected_model_tests;

#[cfg(all(test, windows))]
#[path = "agent_connection_tests.rs"]
mod connection_model_tests;

#[cfg(all(test, windows))]
#[path = "agent_task_status_tests.rs"]
mod task_status_model_tests;

#[cfg(all(test, windows))]
#[path = "agent_place_tests.rs"]
mod place_model_tests;

#[cfg(all(test, windows))]
#[path = "agent_region_tests.rs"]
mod region_model_tests;

#[cfg(all(test, windows))]
#[path = "agent_restore_tests.rs"]
mod restore_tests;
