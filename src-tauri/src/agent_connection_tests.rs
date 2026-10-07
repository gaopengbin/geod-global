//! Opt-in real model switching and Windows credential persistence.
//! Every native vault reference is newly owned by this isolated acceptance.
use super::registry::{NativeVault, Vault};
use super::tests::{assistant_text, completed};
use super::*;
use std::collections::BTreeSet;

#[derive(Default)]
pub(super) struct OwnedNativeVault {
    references: std::sync::Mutex<BTreeSet<String>>,
    writes: AtomicU64,
}
impl Vault for OwnedNativeVault {
    fn read(&self, reference: &str) -> Result<Option<Zeroizing<String>>, String> {
        if !self.references.lock().unwrap().contains(reference) {
            return Err("Test cannot read an unowned credential reference.".into());
        }
        NativeVault.read(reference)
    }
    fn write(&self, reference: &str, secret: &str) -> Result<(), String> {
        if !reference.strip_prefix("connection-").is_some_and(|id| {
            id.len() == 36 && id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
        }) {
            return Err("Test cannot write an unscoped credential reference.".into());
        }
        self.references.lock().unwrap().insert(reference.into());
        NativeVault.write(reference, secret)?;
        self.writes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn delete(&self, reference: &str) -> Result<(), String> {
        if !self.references.lock().unwrap().contains(reference) {
            return Err("Test cannot delete an unowned credential reference.".into());
        }
        NativeVault.delete(reference)
    }
}
impl Drop for OwnedNativeVault {
    fn drop(&mut self) {
        for reference in self.references.get_mut().unwrap().iter() {
            let _ = NativeVault.delete(reference);
        }
    }
}
fn calls(snapshot: &Value, tool: &str) -> usize {
    snapshot["selected"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| {
            entry["type"] == "tool" && entry["name"] == tool && entry["status"] == "completed"
        })
        .count()
}
pub(super) async fn checked_turn(connection: &Connection, base: &std::path::Path) -> Value {
    let snapshot = completed(connection).await;
    if snapshot["selected"]["status"] != "completed" {
        let diagnostics = connection.diagnostics.lock().await;
        let known = [
            "Encrypted reasoning is not supported by this connection.",
            "Unsupported Agent reasoning.",
            "This Agent connection supports text only.",
            "Unsupported Agent model request.",
            "Unsupported Agent message.",
            "Responses input must be a text/function array.",
            "Agent model output was incomplete.",
            "Agent reasoning limit reached.",
            "Unsupported Agent tool.",
            "Invalid Agent tool declarations.",
            "Unknown model",
        ];
        let matched: Vec<&str> = known
            .into_iter()
            .filter(|message| diagnostics.contains(message))
            .collect();
        tokio::fs::write(
            base.join("failure-classification.json"),
            serde_json::to_vec_pretty(&json!({"schema":"geod-agent-model-test-failure/v1",
                "model":snapshot["selected"]["modelId"],"knownLocalDiagnostics":matched,
                "rawDiagnosticsSaved":false}))
            .unwrap(),
        )
        .await
        .unwrap();
    }
    snapshot
}
fn verify_turn(snapshot: &Value, marker: &str, previous: Option<&Value>, natural: bool) {
    assert_eq!(
        snapshot["selected"]["status"], "completed",
        "{}",
        snapshot["selected"]["error"]
    );
    assert!(!snapshot["selected"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["type"] == "tool" && entry["status"] == "failed"));
    assert!(assistant_text(snapshot).contains(marker));
    let tools = if natural {
        vec![
            "geod_workspace_context",
            "geod_project_get",
            "geod_jobs_list",
        ]
    } else {
        vec!["geod_health", "geod_project_get"]
    };
    for tool in tools {
        assert!(
            calls(snapshot, tool) > previous.map_or(0, |value| calls(value, tool)),
            "Missing actual call: {tool}"
        );
    }
    if natural {
        let entries = snapshot["selected"]["entries"].as_array().unwrap();
        assert!(
            entries
                .iter()
                .filter(|entry| entry["type"] == "tool")
                .all(|entry| {
                    [
                        "geod_workspace_context",
                        "geod_project_get",
                        "geod_jobs_list",
                        "geod_health",
                        "geod_projects_list",
                    ]
                    .contains(&entry["name"].as_str().unwrap())
                }),
            "Status request attempted an unrelated search/review or external action"
        );
        let answer = entries
            .iter()
            .rev()
            .find(|entry| entry["type"] == "assistant")
            .unwrap()["text"]
            .as_str()
            .unwrap();
        assert!(
            answer.chars().count() <= 400,
            "Simple status answer exceeded the requested concise scope"
        );
        let jobs = entries
            .iter()
            .rev()
            .find(|entry| entry["type"] == "tool" && entry["name"] == "geod_jobs_list")
            .unwrap();
        assert_eq!(jobs["summary"]["count"], 0);
        assert_eq!(jobs["summary"]["total"], 0);
    }
}
fn action(action: &str, id: &str) -> ModelRequest {
    serde_json::from_value(json!({"action":action,"id":id})).unwrap()
}

#[tokio::test]
#[ignore = "Explicit model test key, two live routes and fresh QA store; creates and deletes only newly owned Windows credentials"]
async fn live_model_switching_native_vault_and_history_isolation() {
    assert_eq!(std::env::var("GEOD_AGENT_TEST_NATIVE_VAULT").unwrap(), "1");
    let secret = Zeroizing::new(std::env::var("GEOD_AGENT_TEST_KEY").unwrap());
    let endpoint = std::env::var("GEOD_AGENT_TEST_BASE_URL").unwrap();
    let model_a = std::env::var("GEOD_AGENT_TEST_MODEL").unwrap();
    let model_b = std::env::var("GEOD_AGENT_TEST_SECOND_MODEL").unwrap();
    let provider_b = std::env::var("GEOD_AGENT_TEST_SECOND_PROVIDER").unwrap();
    assert!(["openai", "deepseek", "custom"].contains(&provider_b.as_str()));
    assert_ne!(model_a, model_b);
    let natural = std::env::var("GEOD_AGENT_TEST_NATURAL_REQUESTS")
        .ok()
        .as_deref()
        == Some("1");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap();
    let requested = PathBuf::from(std::env::var("GEOD_AGENT_CONNECTION_QA").unwrap());
    assert!(requested.is_absolute() && !requested.exists());
    let parent = requested.parent().unwrap().canonicalize().unwrap();
    assert!(parent.starts_with(root.join(".verification").canonicalize().unwrap()));
    let base = parent.join(requested.file_name().unwrap());
    tokio::fs::create_dir(&base).await.unwrap();
    let public_source = PathBuf::from(std::env::var("GEOD_AGENT_CONNECTION_PROJECTS").unwrap());
    assert!(public_source
        .canonicalize()
        .unwrap()
        .starts_with(root.join(".verification")));
    let public_bytes = tokio::fs::read(public_source).await.unwrap();
    let projects: HashMap<String, geod_runtime::Project> =
        serde_json::from_slice(&public_bytes).unwrap();
    let source = projects
        .values()
        .find(|p| {
            p.scenes
                .first()
                .is_some_and(|s| s.item_id.starts_with("HLS.L30."))
        })
        .unwrap();
    let manager = JobManager::open(base.join("core")).await.unwrap();
    let accounts_before = serde_json::to_value(manager.provider_accounts().await).unwrap();
    let marker = format!("Native registry {}", super::tests::uuid_for_test());
    let project = manager
        .create_project(geod_runtime::CreateProjectRequest {
            name: marker.clone(),
            bounds: source.bounds,
            geometry: source.geometry.clone(),
            scenes: source.scenes.clone(),
        })
        .await
        .unwrap();
    let project_id = project.id;
    let attached_context = json!({"page":"My Data","provider":"nasa-earthdata",
        "bounds":source.bounds,"start":"2025-06-01","end":"2025-06-30",
        "cloudMax":100,"projectId":project_id});
    let home = base.join("agent");
    let runtime = root.join(".agent-runtime/win32-x64");
    verify_runtime(&runtime).await.unwrap();
    let vault = Arc::new(OwnedNativeVault::default());
    let agent = DesktopAgent::open_with_vault(
        home.clone(),
        runtime.clone(),
        manager.clone(),
        vault.clone(),
    )
    .await
    .unwrap();
    let save = |provider: &str, label: &str, model: &str| {
        serde_json::from_value(json!({
        "provider":provider,"label":label,"protocol":"openai-compatible","baseUrl":endpoint,"model":model,"apiKey":secret.as_str()
    })).unwrap()
    };
    let first = agent
        .save_model(save("deepseek", "Owned route A", &model_a))
        .await
        .unwrap();
    let id_a = first["registry"]["selectedId"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(first["model"]["verification"], "not-verified");
    assert_eq!(vault.writes.load(Ordering::SeqCst), 1);
    let sent = agent.operation("send",json!({"text":if natural {"看看这个工程现在叫什么名字，下载完成了吗？直接查本地状态，用两句话回答，不准备方案或开始下载。".to_string()} else {format!("请调用 geod_health 和 geod_project_get(id={project_id}) 读取真实本地状态。只报告当前保存的工程名和任务是否存在，不查询外部数据、不准备方案、不下载。")},"context":if natural {attached_context.clone()} else {Value::Null}})).await.unwrap();
    assert_eq!(sent["busy"], true);
    let session_a = sent["selected"]["id"].as_str().unwrap().to_string();
    let registry_before = tokio::fs::read(home.join("registry.json")).await.unwrap();
    assert!(agent
        .save_model(save(&provider_b, "Forbidden during turn", &model_b))
        .await
        .unwrap_err()
        .contains("Stop"));
    assert_eq!(vault.writes.load(Ordering::SeqCst), 1);
    assert_eq!(
        tokio::fs::read(home.join("registry.json")).await.unwrap(),
        registry_before
    );
    let connection = agent.0.state.lock().await.connection.clone().unwrap();
    let turn_a = checked_turn(&connection, &base).await;
    verify_turn(&turn_a, &marker, None, natural);
    let thread_a = turn_a["selected"]["threadId"].clone();
    drop(connection);
    let second = agent
        .save_model(save(&provider_b, "Owned route B", &model_b))
        .await
        .unwrap();
    let id_b = second["registry"]["selectedId"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(id_a, id_b);
    assert_eq!(
        second["registry"]["connections"].as_array().unwrap().len(),
        2
    );
    assert!(second["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["id"] == session_a && s["compatible"] == false));
    let sessions_before = tokio::fs::read(home.join("sessions.json")).await.unwrap();
    assert!(agent
        .operation(
            "send",
            json!({"sessionId":session_a,"text":"Try an incompatible connection"})
        )
        .await
        .is_err());
    assert_eq!(
        tokio::fs::read(home.join("sessions.json")).await.unwrap(),
        sessions_before
    );
    let marker_b = format!("Native second {}", super::tests::uuid_for_test());
    manager
        .rename_project(&project_id, &marker_b)
        .await
        .unwrap();
    let sent = agent.operation("send",json!({"text":if natural {"这个工程当前叫什么，下载到哪一步了？直接查最新状态，两句话就够，不准备方案或下载。".to_string()} else {format!("请重新调用 geod_health 和 geod_project_get(id={project_id}) 读取真实本地状态。只报告当前工程名和任务是否存在，不准备方案或下载。")},"context":if natural {attached_context.clone()} else {Value::Null}})).await.unwrap();
    let session_b = sent["selected"]["id"].as_str().unwrap().to_string();
    let connection = agent.0.state.lock().await.connection.clone().unwrap();
    let turn_b = checked_turn(&connection, &base).await;
    verify_turn(&turn_b, &marker_b, None, natural);
    assert_ne!(session_a, session_b);
    assert_ne!(thread_a, turn_b["selected"]["threadId"]);
    drop(connection);
    let third = agent
        .save_model(save("deepseek", "Owned identical route A", &model_a))
        .await
        .unwrap();
    let id_c = third["registry"]["selectedId"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(id_a, id_c);
    assert!(third["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|s| s["compatible"] == false));
    assert!(agent
        .operation(
            "send",
            json!({"sessionId":session_a,"text":"Do not alias the identical endpoint and model"})
        )
        .await
        .is_err());
    let marker_c = format!("Native identical {}", super::tests::uuid_for_test());
    manager
        .rename_project(&project_id, &marker_c)
        .await
        .unwrap();
    let sent = agent.operation("send",json!({"text":if natural {"再看看这个工程现在的名称，已经有下载任务了吗？查清楚后用两句话回答，不开始下载。".to_string()} else {format!("请调用 geod_health 和 geod_project_get(id={project_id})，只报告本地工程当前名称与任务状态，不下载或准备方案。")},"context":if natural {attached_context.clone()} else {Value::Null}})).await.unwrap();
    let session_c = sent["selected"]["id"].as_str().unwrap().to_string();
    let connection = agent.0.state.lock().await.connection.clone().unwrap();
    let turn_c = checked_turn(&connection, &base).await;
    verify_turn(&turn_c, &marker_c, None, natural);
    assert_ne!(session_a, session_c);
    drop(connection);
    agent.shutdown().await;
    drop(agent);
    let agent =
        DesktopAgent::open_with_vault(home.clone(), runtime, manager.clone(), vault.clone())
            .await
            .unwrap();
    let after_restart = agent.snapshot().await.unwrap();
    assert_eq!(after_restart["model"]["id"], id_c);
    assert_eq!(after_restart["configured"], true);
    assert_eq!(after_restart["sessions"].as_array().unwrap().len(), 3);
    assert_eq!(after_restart["model"]["verification"], "not-verified");
    let switched_back = agent.save_model(action("select", &id_a)).await.unwrap();
    assert!(switched_back["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["id"] == session_a && s["compatible"] == true));
    let marker_restored = format!("Native resumed {}", super::tests::uuid_for_test());
    manager
        .rename_project(&project_id, &marker_restored)
        .await
        .unwrap();
    agent
        .operation("select", json!({"id":session_a}))
        .await
        .unwrap();
    agent.operation("send",json!({"sessionId":session_a,"text":if natural {"软件刚重启，重新检查这个工程的名称和下载状态，不能照抄旧回复。用两句话回答，不开始下载。"} else {"软件已经重启且工程名称已更改。请使用先前的同一工程 ID，重新调用 geod_health 和 geod_project_get，只报告新的当前名称与任务状态，不能照抄旧回复。"},"context":if natural {attached_context} else {Value::Null}})).await.unwrap();
    let connection = agent.0.state.lock().await.connection.clone().unwrap();
    let resumed_a = checked_turn(&connection, &base).await;
    verify_turn(&resumed_a, &marker_restored, Some(&turn_a), natural);
    assert_eq!(resumed_a["selected"]["threadId"], thread_a);
    assert_eq!(resumed_a["selected"]["id"], session_a);
    drop(connection);
    let references = vault.references.lock().unwrap().clone();
    assert_eq!(references.len(), 3);
    for reference in &references {
        assert!(NativeVault
            .read(reference)
            .unwrap()
            .is_some_and(|key| key.as_str() == secret.as_str()));
    }
    for id in [&id_a, &id_b, &id_c] {
        agent.save_model(action("delete", id)).await.unwrap();
    }
    let final_snapshot = agent.snapshot().await.unwrap();
    assert_eq!(final_snapshot["configured"], false);
    assert_eq!(final_snapshot["registry"]["connections"], json!([]));
    assert_eq!(final_snapshot["sessions"].as_array().unwrap().len(), 3);
    for reference in &references {
        assert!(NativeVault.read(reference).unwrap().is_none());
    }
    assert!(manager.list().await.is_empty());
    assert_eq!(
        serde_json::to_value(manager.provider_accounts().await).unwrap(),
        accounts_before
    );
    assert!(!tokio::fs::read(home.join("registry.json"))
        .await
        .unwrap()
        .windows(secret.len())
        .any(|bytes| bytes == secret.as_bytes()));
    assert!(!tokio::fs::read(home.join("sessions.json"))
        .await
        .unwrap()
        .windows(secret.len())
        .any(|bytes| bytes == secret.as_bytes()));
    let receipt = json!({"schema":"geod-agent-connection-model-acceptance/v1","status":"passed",
        "actualModelRoutes":[model_a,model_b],"registryGroups":["deepseek",provider_b],"protocol":"openai-compatible","upstreamVendorVerified":false,
        "modelTurns":4,"naturalUserRequestsWithoutToolNames":natural,"freshAttachedProjectAndTaskReadsChecked":natural,
        "projectMetadata":"Actual previously verified HLS catalog metadata; a locally named QA project, no original file",
        "sourceMetadataSha256":format!("{:x}",Sha256::digest(public_bytes)),"projectCount":1,"jobCount":0,
        "modelSwitchWhileBusyRefusedBeforeVaultMutation":true,"incompatibleHistoryCannotResume":true,
        "identicalEndpointModelConnectionsRemainDistinct":true,"resumedOriginalThreadAfterRestart":true,
        "freshNativeNamesChecked":true,"nativeWindowsVaultTested":true,"ownedVaultEntriesWritten":3,
        "ownedVaultEntriesDeletedAndReadAbsent":true,"providerAccountsUnchanged":true,
        "userModelPreferencesChanged":false,"usedUserDesktop":false,"published":false,
        "turnA":turn_a,"turnB":turn_b,"turnC":turn_c,"resumedA":resumed_a,"finalSnapshot":final_snapshot});
    tokio::fs::write(
        base.join("native-acceptance.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .await
    .unwrap();
    agent.shutdown().await;
    manager.shutdown().await.unwrap();
}
