//! Opt-in live models read a real verified SCL copy in an owned store.
//! Eight foreign FAILED records are metadata distractors, never real downloads.
use super::connection_model_tests::{checked_turn, OwnedNativeVault};
use super::registry::Vault;
use super::tests::assistant_text;
use super::*;

#[path = "agent_task_status_tests/live_transfer.rs"]
mod live_transfer;

fn assert_turn(value: &Value, project_id: &str, name: &str, previous: Option<&Value>) {
    assert_eq!(value["selected"]["status"], "completed");
    let entries = value["selected"]["entries"].as_array().unwrap();
    let after = previous.map_or(0, |old| {
        old["selected"]["entries"].as_array().unwrap().len()
    });
    let fresh = &entries[after..];
    for tool in [
        "geod_workspace_context",
        "geod_project_get",
        "geod_jobs_list",
    ] {
        assert!(
            fresh.iter().any(|entry| entry["type"] == "tool"
                && entry["name"] == tool
                && entry["status"] == "completed"),
            "Missing fresh {tool}"
        );
    }
    assert!(
        fresh
            .iter()
            .filter(|entry| entry["type"] == "tool")
            .all(|entry| entry["status"] == "completed"
                && [
                    "geod_workspace_context",
                    "geod_project_get",
                    "geod_jobs_list",
                    "geod_job_status",
                    "geod_health",
                    "geod_projects_list"
                ]
                .contains(&entry["name"].as_str().unwrap())),
        "Status read attempted an unrelated action"
    );
    let jobs = fresh
        .iter()
        .rev()
        .find(|entry| entry["name"] == "geod_jobs_list")
        .unwrap();
    assert_eq!(jobs["summary"]["projectId"], project_id);
    assert_eq!(jobs["summary"]["total"], 1);
    assert_eq!(jobs["summary"]["count"], 1);
    assert_eq!(jobs["summary"]["pageStatuses"], json!({"succeeded":1}));
    assert_eq!(jobs["summary"]["pageSettledCount"], 1);
    let answer = fresh
        .iter()
        .rev()
        .find(|entry| entry["type"] == "assistant")
        .unwrap()["text"]
        .as_str()
        .unwrap();
    assert!(answer.contains(name));
    assert!(answer.contains("完成") || answer.contains("成功"));
    assert!(answer.chars().count() <= 400);
    assert!(!answer.contains("8 个失败") && !answer.contains("8个失败"));
}

#[tokio::test]
#[ignore = "Explicit model test key and fresh owned store; real SCL copy plus clearly labelled foreign metadata fixtures"]
async fn live_project_task_status_is_scoped_read_only_and_resumes() {
    assert_eq!(std::env::var("GEOD_AGENT_TEST_NATIVE_VAULT").unwrap(), "1");
    let secret = Zeroizing::new(std::env::var("GEOD_AGENT_TEST_KEY").unwrap());
    let endpoint = std::env::var("GEOD_AGENT_TEST_BASE_URL").unwrap();
    let models = [
        std::env::var("GEOD_AGENT_TEST_MODEL").unwrap(),
        std::env::var("GEOD_AGENT_TEST_SECOND_MODEL").unwrap(),
    ];
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap();
    let requested = PathBuf::from(std::env::var("GEOD_AGENT_CONNECTION_QA").unwrap());
    assert!(requested.is_absolute() && !requested.exists());
    let parent = requested.parent().unwrap().canonicalize().unwrap();
    assert!(parent.starts_with(root.join(".verification").canonicalize().unwrap()));
    let base = parent.join(requested.file_name().unwrap());
    let source = PathBuf::from(std::env::var("GEOD_AGENT_TASK_JOBS").unwrap())
        .canonicalize()
        .unwrap();
    assert!(source.starts_with(root.join(".verification")));
    let source_bytes = tokio::fs::read(source).await.unwrap();
    let records: HashMap<String, geod_runtime::Job> =
        serde_json::from_slice(&source_bytes).unwrap();
    let mut original = records
        .values()
        .find(|job| {
            job.kind == "download"
                && job.asset_key == "scl"
                && job.status == geod_runtime::JobStatus::Succeeded
        })
        .unwrap()
        .clone();
    let old_file = PathBuf::from(original.output_path.as_ref().unwrap())
        .canonicalize()
        .unwrap();
    assert!(old_file.starts_with(root.join(".verification")));
    let file = tokio::fs::read(old_file).await.unwrap();
    assert_eq!(
        Some(format!("{:x}", Sha256::digest(&file))),
        original.sha256
    );
    let core = base.join("core");
    tokio::fs::create_dir_all(core.join("assets"))
        .await
        .unwrap();
    let target = core.join("assets").join(format!("{}.tif", original.id));
    tokio::fs::write(&target, &file).await.unwrap();
    original.output_path = Some(target.to_string_lossy().into());
    let mut jobs = HashMap::from([(original.id.clone(), original.clone())]);
    for i in 0..8 {
        let mut foreign = original.clone();
        foreign.id = format!("aaaaaaaa-aaaa-4aaa-8aaa-{i:012x}");
        foreign.asset_key = "visual".into();
        foreign.href = foreign.href.replace("SCL.tif", "TCI.tif");
        assert_ne!(foreign.href, original.href);
        foreign.status = geod_runtime::JobStatus::Failed;
        foreign.output_path = None;
        foreign.sha256 = None;
        foreign.bytes_downloaded = 0;
        foreign.total_bytes = None;
        foreign.transfer = None;
        foreign.created_at = "2026-10-05T00:00:00Z".into();
        foreign.error = Some("Controlled metadata distractor, not an actual download".into());
        jobs.insert(foreign.id.clone(), foreign);
    }
    tokio::fs::write(
        core.join("jobs.json"),
        serde_json::to_vec_pretty(&jobs).unwrap(),
    )
    .await
    .unwrap();
    let manager = JobManager::open(&core).await.unwrap();
    let accounts_before = serde_json::to_value(manager.provider_accounts().await).unwrap();
    let job_before = serde_json::to_value(manager.list().await).unwrap();
    assert_eq!(manager.list().await.len(), 9);
    assert!(manager
        .list()
        .await
        .iter()
        .take(5)
        .all(|job| job.id != original.id));
    let inspection = manager.inspect_raster(&original.id).await.unwrap();
    assert_eq!(inspection.sha256, original.sha256.clone().unwrap());
    let marker = format!("Task scope {}", super::tests::uuid_for_test());
    let bounds = [-122.55, 37.68, -122.32, 37.84];
    let scene = geod_runtime::projects::ProjectScene {
        footprint: None,
        item_id: original.item_id.clone(),
        date: "2025-07-07T00:00:00Z".into(),
        cloud: None,
        crs: Some("EPSG:32610".into()),
        grid_code: Some("10SEG".into()),
        bbox: bounds,
        assets: std::collections::BTreeMap::from([(
            "scl".into(),
            geod_runtime::projects::ProjectAsset {
                href: original.href.clone(),
                media_type: original.media_type.clone(),
                raster_band: None,
            },
        )]),
    };
    let project = manager
        .create_project(geod_runtime::CreateProjectRequest {
            name: marker.clone(),
            bounds,
            geometry: None,
            scenes: vec![scene],
        })
        .await
        .unwrap();
    let context = json!({"page":"My Data","provider":"earth-search","bounds":bounds,"start":"2025-07-01","end":"2025-07-31","cloudMax":100,"projectId":project.id});
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
    let mut ids = Vec::new();
    let mut turns = Vec::new();
    for (i, model) in models.iter().enumerate() {
        let saved=agent.save_model(serde_json::from_value(json!({"provider":if i==0 {"deepseek"} else {"custom"},"label":format!("Owned task route {i}"),"protocol":"openai-compatible","baseUrl":endpoint,"model":model,"apiKey":secret.as_str()})).unwrap()).await.unwrap();
        ids.push(
            saved["registry"]["selectedId"]
                .as_str()
                .unwrap()
                .to_string(),
        );
        let name = format!("{marker} route {i}");
        manager.rename_project(&project.id, &name).await.unwrap();
        agent.operation("send",json!({"text":"看看这个工程现在叫什么，下载完成了吗？直接查本地状态，用两句话回答，不准备方案或开始下载。","context":context})).await.unwrap();
        let connection = agent.0.state.lock().await.connection.clone().unwrap();
        let turn = checked_turn(&connection, &base).await;
        assert_turn(&turn, &project.id, &name, None);
        turns.push(turn);
        drop(connection);
    }
    agent.shutdown().await;
    drop(agent);
    let agent =
        DesktopAgent::open_with_vault(home.clone(), runtime, manager.clone(), vault.clone())
            .await
            .unwrap();
    agent
        .save_model(serde_json::from_value(json!({"action":"select","id":ids[0]})).unwrap())
        .await
        .unwrap();
    let name = format!("{marker} resumed");
    manager.rename_project(&project.id, &name).await.unwrap();
    let session = turns[0]["selected"]["id"].clone();
    agent.operation("send",json!({"sessionId":session,"text":"软件刚重启，重新检查这个工程的名称和下载状态，不能照抄旧回复。用两句话回答，不开始下载。","context":context})).await.unwrap();
    let connection = agent.0.state.lock().await.connection.clone().unwrap();
    let resumed = checked_turn(&connection, &base).await;
    assert_turn(&resumed, &project.id, &name, Some(&turns[0]));
    assert_eq!(
        resumed["selected"]["threadId"],
        turns[0]["selected"]["threadId"]
    );
    drop(connection);
    assert_eq!(
        serde_json::to_value(manager.list().await).unwrap(),
        job_before
    );
    assert_eq!(
        manager.inspect_raster(&original.id).await.unwrap().sha256,
        inspection.sha256
    );
    assert_eq!(
        serde_json::to_value(manager.provider_accounts().await).unwrap(),
        accounts_before
    );
    let registry: Value =
        serde_json::from_slice(&tokio::fs::read(home.join("registry.json")).await.unwrap())
            .unwrap();
    for id in &ids {
        let reference = registry["connections"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == *id)
            .unwrap()["credentialRef"]
            .as_str()
            .unwrap();
        assert!(vault
            .read(reference)
            .unwrap()
            .is_some_and(|key| key.as_str() == secret.as_str()));
        agent
            .save_model(serde_json::from_value(json!({"action":"delete","id":id})).unwrap())
            .await
            .unwrap();
        assert!(vault.read(reference).unwrap().is_none());
    }
    let receipt = json!({"schema":"geod-agent-project-task-model/v1","status":"passed","models":models,"upstreamVendorVerified":false,
        "modelTurns":3,"completedFile":{"jobId":original.id,"sha256":inspection.sha256,"bytes":file.len(),"scope":"Copy of prior real Sentinel SCL download, native checksum inspection before and after model reads; no new source download"},
        "foreignRecords":{"count":8,"scope":"Controlled failed metadata distractors only, not actual failed downloads"},
        "nativeProjectTotal":1,"globalTaskTotal":9,"projectFilteringBeforePaginationChecked":true,
        "naturalRequestsWithoutToolNames":true,"freshNativeNameAndScopedStateChecked":true,
        "sameThreadResumedAfterRestart":true,"taskRecordsUnchanged":true,"providerAccountsUnchanged":true,
        "ownedVaultEntriesWritten":2,"ownedVaultEntriesDeletedAndReadAbsent":true,"usedUserDesktop":false,"published":false,
        "sourceJobsSha256":format!("{:x}",Sha256::digest(source_bytes)),"turns":turns,"resumed":resumed,
        "answers":turns.iter().map(assistant_text).collect::<Vec<_>>()});
    tokio::fs::write(
        base.join("native-acceptance.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .await
    .unwrap();
    agent.shutdown().await;
    manager.shutdown().await.unwrap();
}
