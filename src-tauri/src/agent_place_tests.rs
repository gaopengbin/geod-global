use super::*;

#[tokio::test]
#[ignore = "explicit isolated copy of reported pending-decision failure, actual owned Codex/model and metadata only; no download"]
async fn live_legacy_boundary_question_retires_and_prepares_polygon_review() {
    struct ReadOnlyVault;
    impl registry::Vault for ReadOnlyVault {
        fn read(&self, reference: &str) -> Result<Option<Zeroizing<String>>, String> {
            registry::Vault::read(&registry::NativeVault, reference)
        }
        fn write(&self, _: &str, _: &str) -> Result<(), String> {
            panic!("No credential writes")
        }
        fn delete(&self, _: &str) -> Result<(), String> {
            panic!("No credential deletes")
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap();
    let directory = PathBuf::from(
        std::env::var_os("GEOD_AGENT_PENDING_BOUNDARY_QA").expect("explicit isolated legacy QA"),
    )
    .canonicalize()
    .unwrap();
    assert!(directory.starts_with(root.join(".verification")));
    let home = directory.join("agent");
    let seed: Value =
        serde_json::from_slice(&tokio::fs::read(home.join("sessions.json")).await.unwrap())
            .unwrap();
    let session = seed["selectedId"].as_str().unwrap();
    let count = seed["sessions"][0]["entries"].as_array().unwrap().len();
    let prior = seed["sessions"][0]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["decision"]["status"] == "pending")
        .unwrap()["decision"]
        .clone();
    assert_eq!(prior["questions"][0]["id"], "crop_area");
    let manager = JobManager::open(directory.join("core")).await.unwrap();
    assert!(manager.list().await.is_empty());
    let agent = DesktopAgent::open_with_vault(
        home,
        root.join(".agent-runtime/win32-x64"),
        manager.clone(),
        Arc::new(ReadOnlyVault),
    )
    .await
    .unwrap();
    agent
        .operation("send", json!({"sessionId":session,"text":"再试试"}))
        .await
        .unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(240);
    let snapshot = loop {
        let value = agent.snapshot().await.unwrap();
        if value["busy"] == false {
            break value;
        }
        if tokio::time::Instant::now() >= deadline {
            tokio::fs::write(
                directory.join("legacy-boundary-timeout.json"),
                serde_json::to_vec_pretty(&value).unwrap(),
            )
            .await
            .unwrap();
            agent.shutdown().await;
            manager.shutdown().await.unwrap();
            panic!("bounded legacy acceptance timed out");
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    };
    tokio::fs::write(
        directory.join("legacy-boundary-snapshot.json"),
        serde_json::to_vec_pretty(&snapshot).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(
        snapshot["selected"]["status"], "completed",
        "inspect retained snapshot"
    );
    assert!(snapshot["selected"]["error"].is_null());
    let entries = snapshot["selected"]["entries"].as_array().unwrap();
    let added = &entries[count..];
    let resolved = entries
        .iter()
        .find(|e| e["decision"]["id"] == prior["id"])
        .unwrap();
    assert_eq!(resolved["decision"]["status"], "superseded");
    assert!(resolved["decision"]["answers"].is_null());
    assert_eq!(snapshot["selected"]["executionMode"], "confirm-each");
    assert!(added
        .iter()
        .any(|e| e["name"] == "geod_boundary_read" && e["status"] == "completed"));
    assert!(!entries.iter().any(|e| e["decision"]["status"] == "pending"));
    assert!(!added.iter().any(|e| e["name"] == "geod_plan_execute"));
    let plan_id = added
        .iter()
        .rev()
        .filter(|e| e["name"] == "geod_project_plan" && e["status"] == "completed")
        .find_map(|e| {
            e["references"]
                .as_array()?
                .iter()
                .find(|r| r["kind"] == "plan")?["id"]
                .as_str()
        })
        .expect("actual polygon project review");
    let review = manager.agent_plan_status(session, plan_id).await.unwrap();
    assert_eq!(review["status"], "pending");
    assert_eq!(
        review["polygon"]["sha256"],
        resolved["decision"]["resolution"]["boundary"]["sha256"]
    );
    let preview = manager
        .agent_plan_map_preview(session, plan_id, review["planHash"].as_str().unwrap())
        .await
        .unwrap();
    assert_eq!(preview["geometry"]["type"], "MultiPolygon");
    assert_eq!(
        preview["geometry"]["coordinates"].as_array().unwrap().len(),
        3
    );
    assert_ne!(snapshot["selected"]["goal"]["status"], "complete");
    assert!(manager.list().await.is_empty());
    tokio::fs::write(directory.join("legacy-boundary-verification.json"),serde_json::to_vec_pretty(&json!({
        "status":"passed","engine":"actual owned Codex runtime","model":"actual saved connection","conversation":"isolated exact reported conversation",
        "oldDecision":"superseded without answer","polygonParts":3,"projectReview":review,"downloadJobs":0,"userStoreModified":false,"vaultWrites":0,
        "compactionsBefore":seed["sessions"][0]["contextState"]["count"],"compactionsAfter":snapshot["selected"]["contextState"]["count"],
        "newToolEntries":added.iter().filter(|e|e["type"]=="tool").count(),"scope":"Retry to actual polygon review; no original download or completed crop"
    })).unwrap()).await.unwrap();
    agent.shutdown().await;
    manager.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore = "actual saved model and owned Codex, isolated real archived city boundary/public catalog; no downloads or user-history writes"]
async fn live_source_boundary_prepares_polygon_project_without_upload_question() {
    struct ReadOnlyVault;
    impl registry::Vault for ReadOnlyVault {
        fn read(&self, reference: &str) -> Result<Option<Zeroizing<String>>, String> {
            registry::Vault::read(&registry::NativeVault, reference)
        }
        fn write(&self, _: &str, _: &str) -> Result<(), String> {
            panic!("No credential writes")
        }
        fn delete(&self, _: &str) -> Result<(), String> {
            panic!("No credential deletes")
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap();
    let directory = PathBuf::from(
        std::env::var_os("GEOD_SOURCE_BOUNDARY_QA").expect("explicit isolated source-boundary QA"),
    )
    .canonicalize()
    .unwrap();
    assert!(directory.starts_with(root.join(".verification")));
    let manager = JobManager::open(directory.join("core")).await.unwrap();
    assert!(manager.list().await.is_empty());
    let agent = DesktopAgent::open_with_vault(
        directory.join("agent"),
        root.join(".agent-runtime/win32-x64"),
        manager.clone(),
        Arc::new(ReadOnlyVault),
    )
    .await
    .unwrap();
    agent.operation("send",json!({"text":"按真实纽约市行政区多边形范围，准备最新的哨兵2号真彩色影像裁剪方案。请使用软件可读取的边界，显示待确认任务卡；只准备方案，不执行下载或裁剪。"})).await.unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(180);
    let snapshot = loop {
        let value = agent.snapshot().await.unwrap();
        if value["busy"] == false {
            break value;
        }
        if tokio::time::Instant::now() >= deadline {
            tokio::fs::write(
                directory.join("boundary-model-timeout.json"),
                serde_json::to_vec_pretty(&value).unwrap(),
            )
            .await
            .unwrap();
            agent.shutdown().await;
            manager.shutdown().await.unwrap();
            panic!("bounded boundary model verification timed out");
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    };
    tokio::fs::write(
        directory.join("boundary-model-snapshot.json"),
        serde_json::to_vec_pretty(&snapshot).unwrap(),
    )
    .await
    .unwrap();
    let entries = snapshot["selected"]["entries"].as_array().unwrap();
    assert_eq!(
        snapshot["selected"]["status"], "completed",
        "inspect retained boundary-model snapshot"
    );
    assert!(
        entries
            .iter()
            .any(|e| e["name"] == "geod_boundary_read" && e["status"] == "completed"),
        "no native boundary read"
    );
    assert!(
        !entries.iter().any(|e| e["decision"]["status"] == "pending"),
        "an existing source polygon must not require an upload choice"
    );
    let plan_id = entries
        .iter()
        .rev()
        .filter(|e| e["name"] == "geod_project_plan" && e["status"] == "completed")
        .find_map(|e| {
            e["references"]
                .as_array()?
                .iter()
                .find(|r| r["kind"] == "plan")?["id"]
                .as_str()
        })
        .expect("actual pending project review");
    let session = snapshot["selected"]["id"].as_str().unwrap();
    let review = manager.agent_plan_status(session, plan_id).await.unwrap();
    assert_eq!(review["status"], "pending");
    assert!(review["polygon"]["sha256"].as_str().is_some());
    let preview = manager
        .agent_plan_map_preview(session, plan_id, review["planHash"].as_str().unwrap())
        .await
        .unwrap();
    let geometry = &preview["geometry"];
    assert_eq!(geometry["type"], "MultiPolygon");
    assert_eq!(geometry["coordinates"].as_array().unwrap().len(), 3);
    let positions = geometry["coordinates"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|p| p.as_array().unwrap())
        .map(|r| r.as_array().unwrap().len())
        .sum::<usize>();
    assert_eq!(positions, 1645);
    assert!(manager.list().await.is_empty());
    assert!(!entries.iter().any(|e| e["name"] == "geod_plan_execute"));
    tokio::fs::write(directory.join("boundary-model-verification.json"),serde_json::to_vec_pretty(&json!({
        "status":"passed","engine":"actual owned Codex runtime","model":"actual saved connection",
        "boundary":"genuine retained Census city response, original cache timestamp","newLiveCensusQuery":false,
        "catalog":"actual public Earth Search","polygonParts":3,"positions":1645,"projectReview":review,
        "downloadJobs":0,"userConversationModified":false,"vaultWrites":0,"scope":"Source boundary to pending project and map; no original transfer or completed crop"
    })).unwrap()).await.unwrap();
    agent.shutdown().await;
    manager.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore = "explicit isolated copy of failed conversation, actual saved model connection, readonly metadata/preflight only"]
async fn live_crop_followup_preserves_request_and_returns_decision_without_execution() {
    struct ReadOnlyVault;
    impl registry::Vault for ReadOnlyVault {
        fn read(&self, reference: &str) -> Result<Option<Zeroizing<String>>, String> {
            registry::Vault::read(&registry::NativeVault, reference)
        }
        fn write(&self, _: &str, _: &str) -> Result<(), String> {
            panic!("Acceptance must never modify user credentials")
        }
        fn delete(&self, _: &str) -> Result<(), String> {
            panic!("Acceptance must never remove user credentials")
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap();
    let directory = PathBuf::from(
        std::env::var_os("GEOD_AGENT_CROP_FOLLOWUP_QA").expect("explicit isolated QA copy"),
    )
    .canonicalize()
    .unwrap();
    assert!(directory.starts_with(root.join(".verification")));
    let home = directory.join("agent");
    let seeded: Value =
        serde_json::from_slice(&tokio::fs::read(home.join("sessions.json")).await.unwrap())
            .unwrap();
    let session = seeded["selectedId"].as_str().unwrap();
    let count = seeded["sessions"][0]["entries"].as_array().unwrap().len();
    let manager = JobManager::open(directory.join("core")).await.unwrap();
    assert!(manager.list().await.is_empty());
    let agent = DesktopAgent::open_with_vault(
        home,
        root.join(".agent-runtime/win32-x64"),
        manager.clone(),
        Arc::new(ReadOnlyVault),
    )
    .await
    .unwrap();
    agent
        .operation(
            "send",
            json!({"sessionId":session,"text":"要按纽约市区范围裁剪出来"}),
        )
        .await
        .unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(240);
    let snapshot = loop {
        let value = agent.snapshot().await.unwrap();
        if value["busy"] == false {
            break value;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "bounded crop follow-up timed out"
        );
        tokio::time::sleep(Duration::from_millis(400)).await;
    };
    tokio::fs::write(
        directory.join("snapshot.json"),
        serde_json::to_vec_pretty(&snapshot).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(
        snapshot["selected"]["status"], "completed",
        "inspect retained snapshot"
    );
    assert!(snapshot["selected"]["error"].is_null());
    let entries = snapshot["selected"]["entries"].as_array().unwrap();
    let added = &entries[count..];
    assert!(added
        .iter()
        .any(|e| e["type"] == "assistant"
            && e["text"].as_str().is_some_and(|t| !t.trim().is_empty())));
    // A necessary geometry choice may precede goal redefinition. No dependent
    // project may be prepared until that real human preference is recorded.
    assert_ne!(snapshot["selected"]["goal"]["status"], "complete");
    assert!(added
        .iter()
        .any(|e| e["name"] == "geod_request_decision" && e["decision"]["status"] == "pending"));
    assert!(!added.iter().any(|e| e["name"] == "geod_plan_execute"));
    assert!(
        manager.list().await.is_empty(),
        "A follow-up must not start downloads before confirmation"
    );
    tokio::fs::write(directory.join("verification.json"),serde_json::to_vec_pretty(&json!({
        "status":"passed","engine":"actual owned Codex runtime","provider":"actual saved model connection",
        "conversation":"isolated copy of the reported failure","decision":"pending human crop-area choice",
        "goalRevised":added.iter().any(|e|e["name"]=="geod_goal_define"&&e["status"]=="completed"),"nativeJobsCreated":0,"credentialVaultWrites":0,"userStoreModified":false,
        "verificationScope":"Model/native follow-up and decision; no original download or completed crop"
    })).unwrap()).await.unwrap();
    agent.shutdown().await;
    manager.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore = "explicit pending crop decision in isolated copy, actual model and public metadata only; no downloads"]
async fn live_crop_choice_keeps_latest_request_and_prepares_pending_project() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap();
    let directory = PathBuf::from(
        std::env::var_os("GEOD_AGENT_CROP_FOLLOWUP_QA").expect("explicit isolated QA copy"),
    )
    .canonicalize()
    .unwrap();
    assert!(directory.starts_with(root.join(".verification")));
    let home = directory.join("agent");
    let seeded: Value =
        serde_json::from_slice(&tokio::fs::read(home.join("sessions.json")).await.unwrap())
            .unwrap();
    let session = seeded["selectedId"].as_str().unwrap();
    let entries = seeded["sessions"][0]["entries"].as_array().unwrap();
    let count = entries.len();
    let decision = entries
        .iter()
        .find(|e| e["decision"]["status"] == "pending")
        .unwrap()["decision"]
        .clone();
    let questions = decision["questions"].as_array().unwrap();
    assert_eq!(questions.len(), 1);
    let question = &questions[0];
    let option = question["options"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| {
            o["label"]
                .as_str()
                .is_some_and(|text| text.contains("外接矩形"))
        })
        .unwrap();
    let manager = JobManager::open(directory.join("core")).await.unwrap();
    assert!(manager.list().await.is_empty());
    // Only the isolated QA registry is loaded; ordinary current-format entries
    // need credential reads, never a credential write or migration.
    let agent = DesktopAgent::open(home, root.join(".agent-runtime/win32-x64"), manager.clone())
        .await
        .unwrap();
    agent.operation("send",json!({"sessionId":session,"text":"","decisionAnswer":{"decisionId":decision["id"],"answers":[{"questionId":question["id"],"optionId":option["id"]}]}})).await.unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(240);
    let snapshot = loop {
        let value = agent.snapshot().await.unwrap();
        if value["busy"] == false {
            break value;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "crop choice continuation timed out"
        );
        tokio::time::sleep(Duration::from_millis(400)).await;
    };
    tokio::fs::write(
        directory.join("choice-snapshot.json"),
        serde_json::to_vec_pretty(&snapshot).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(
        snapshot["selected"]["status"], "completed",
        "inspect retained choice snapshot"
    );
    let entries = snapshot["selected"]["entries"].as_array().unwrap();
    let added = &entries[count..];
    assert!(added
        .iter()
        .any(|e| e["name"] == "geod_goal_define" && e["status"] == "completed"));
    assert!(snapshot["selected"]["goal"]["requestText"]
        .as_str()
        .is_some_and(|text| text.starts_with("要按纽约市区范围裁剪出来")));
    let review = snapshot["plans"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["kind"] == "project" && p["status"] == "pending")
        .unwrap();
    assert_eq!(
        review["files"].as_array().unwrap().len(),
        4,
        "keep the four selected latest scenes rather than silently adding an older fifth scene"
    );
    assert!(added
        .iter()
        .filter(|e| e["taskContext"].is_object())
        .any(|e| e["taskContext"]["choices"]
            .as_array()
            .is_some_and(|c| !c.is_empty())));
    assert!(!added.iter().any(|e| e["name"] == "geod_plan_execute"));
    assert!(manager.list().await.is_empty());
    tokio::fs::write(directory.join("choice-verification.json"),serde_json::to_vec_pretty(&json!({"status":"passed","goalRevised":true,"savedChoiceUsed":true,"selectedScenes":4,"projectReview":"pending","nativeJobsCreated":0,"userStoreModified":false,"verificationScope":"Actual model/native choice continuation; no downloaded or cropped raster"})).unwrap()).await.unwrap();
    agent.shutdown().await;
    manager.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore = "explicit native System proxy and stopped Windows user-profile storage probe; no model or credentials"]
async fn live_native_system_proxy_and_profile_storage() {
    let base = PathBuf::from(
        std::env::var_os("GEOD_AGENT_SYSTEM_QA").expect("explicit isolated QA directory"),
    );
    tokio::fs::create_dir_all(&base).await.unwrap();
    // A detached parent-exit test releases this gate only after its launcher
    // has exited. GeoD must validate storage without any live package ancestor.
    if std::env::var_os("GEOD_AGENT_SYSTEM_START_GATE").is_some() {
        let gate = base.join("start-gate");
        let deadline = tokio::time::Instant::now() + Duration::from_secs(45);
        while !gate.exists() {
            assert!(
                tokio::time::Instant::now() < deadline,
                "detached launch gate timed out"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
    let core = std::env::var_os("GEOD_AGENT_SYSTEM_CORE")
        .map(PathBuf::from)
        .unwrap_or_else(|| base.join("core"));
    let manager = JobManager::open(core.clone()).await.unwrap();
    println!(
        "{}",
        json!({"storageRoot":manager.storage_root(),"resolvedRoot":manager.storage_root().canonicalize().unwrap(),
        "recordDirectories":(["agent-places","agent-region-levels","agent-searches"].map(|name| {
            let p=manager.storage_root().join(name);
            #[cfg(windows)]
            { use std::os::windows::fs::MetadataExt;
                json!({"expected":p,"resolved":p.canonicalize().ok(),"attributes":p.symlink_metadata().ok().map(|m|m.file_attributes())}) }
            #[cfg(not(windows))]
            json!({"expected":p,"resolved":p.canonicalize().ok()})
        }))})
    );
    let mut checks = Vec::new();
    for (tool, args) in [
        (
            "geod_place_search",
            json!({"query":"New York","kind":"city","countryCode":"US"}),
        ),
        ("geod_region_levels", json!({"countryCode":"US"})),
        (
            "geod_region_search",
            json!({"query":"Köln","countryCode":"DE","adminLevel":2}),
        ),
        (
            "geod_region_search",
            json!({"query":"Île-de-France","countryCode":"FR","adminLevel":1}),
        ),
        (
            "geod_region_search",
            json!({"query":"Pune","countryCode":"IN","adminLevel":2}),
        ),
        (
            "geod_scene_search",
            json!({"provider":"earth-search","bounds":[-74.258843,40.476578,-73.700233,40.91763],"start":"2026-09-07","end":"2026-10-06","cloudMax":100,"limit":1}),
        ),
    ] {
        let start = std::time::Instant::now();
        let result = agent_actions::call(
            manager.clone(),
            "a3d6b09d-fb3b-4403-af55-25f0b336d9e5",
            tool,
            args,
            None,
        )
        .await;
        println!(
            "{}",
            json!({"tool":tool,"elapsedMs":start.elapsed().as_millis(),"error":result.as_ref().err()})
        );
        checks.push(json!({"tool":tool,"result":result}));
    }
    // Retain successful native results even if restart verification fails.
    tokio::fs::write(
        base.join("native-system-probe.json"),
        serde_json::to_vec_pretty(&json!({"proxy":manager.proxy_settings().await,"checks":checks,"phase":"before-reopen"})).unwrap(),
    ).await.unwrap();
    manager.shutdown().await.unwrap();
    drop(manager);
    let reopened = JobManager::open(core).await.unwrap();
    let mut restored = Vec::new();
    for (tool, args) in [
        (
            "geod_place_search",
            json!({"query":"New York","kind":"city","countryCode":"US"}),
        ),
        (
            "geod_region_search",
            json!({"query":"Pune","countryCode":"IN","adminLevel":2}),
        ),
    ] {
        let result = agent_actions::call(
            reopened.clone(),
            "a3d6b09d-fb3b-4403-af55-25f0b336d9e5",
            tool,
            args,
            None,
        )
        .await;
        restored.push(json!({"tool":tool,"result":result}));
    }
    tokio::fs::write(
        base.join("native-system-probe.json"),
        serde_json::to_vec_pretty(&json!({"proxy":reopened.proxy_settings().await,"checks":checks,"afterReopen":restored}))
            .unwrap(),
    )
    .await
    .unwrap();
    reopened.shutdown().await.unwrap();
    assert!(
        checks.iter().all(|c| c["result"].get("Ok").is_some()),
        "inspect retained isolated native failures"
    );
    assert_eq!(restored[0]["result"]["Ok"]["cached"], true);
    assert_eq!(restored[1]["result"]["Ok"]["provenance"]["cached"], true);
}

#[tokio::test]
#[ignore = "owned live model plus public place/catalog/HEAD requests in isolated review-only store"]
async fn live_named_city_latest_download_review_without_parameters() {
    run_place_review(false).await;
}

#[tokio::test]
#[ignore = "owned model, genuine old Codex thread upgrade and public readonly acquisition preflight"]
async fn live_tool_upgrade_continues_same_conversation() {
    run_place_review(true).await;
}

async fn run_place_review(restore: bool) {
    let secret =
        Zeroizing::new(std::env::var("GEOD_AGENT_TEST_KEY").expect("explicit owned test key"));
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap();
    let base = PathBuf::from(
        std::env::var_os("GEOD_AGENT_PLACE_MODEL_QA").expect("isolated QA directory"),
    );
    let home = base.join("sessions");
    tokio::fs::create_dir_all(&home).await.unwrap();
    let core = std::env::var_os("GEOD_AGENT_PLACE_MODEL_CORE")
        .map(PathBuf::from)
        .unwrap_or_else(|| base.join("core"));
    let actual_profile = std::env::var_os("GEOD_AGENT_PLACE_MODEL_CORE").is_some();
    let manager = JobManager::open(core).await.unwrap();
    let original_jobs: std::collections::BTreeSet<_> =
        manager.list().await.into_iter().map(|job| job.id).collect();
    let original_proxy = manager.proxy_settings().await;
    assert!(
        !actual_profile
            || std::env::var_os("GEOD_AGENT_TEST_DIRECT").is_none()
                && std::env::var_os("GEOD_AGENT_TEST_PROXY").is_none(),
        "actual profile acceptance retains its saved network route"
    );
    if std::env::var("GEOD_AGENT_TEST_DIRECT").as_deref() == Ok("1") {
        manager
            .save_proxy_settings(geod_runtime::ProxySettings {
                mode: geod_runtime::proxy::ProxyMode::Direct,
                url: None,
            })
            .await
            .unwrap();
    } else if let Ok(proxy) = std::env::var("GEOD_AGENT_TEST_PROXY") {
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
    let url = std::env::var("GEOD_AGENT_TEST_BASE_URL").expect("owned loopback model route");
    let model =
        std::env::var("GEOD_AGENT_TEST_MODEL").unwrap_or_else(|_| "deepseek-v4-flash".into());
    let config = json!({"label":"Owned place workflow acceptance","protocol":"openai-compatible","baseUrl":url,"model":model,"apiKey":secret.as_str()});
    let initial_tools = if restore {
        definitions()
            .into_iter()
            .filter(|tool| tool["name"] != "geod_place_search")
            .collect()
    } else {
        definitions()
    };
    connection
        .rpc(
            "configure",
            json!({"config":config,"definitions":initial_tools}),
        )
        .await
        .unwrap();
    let agent = DesktopAgent::open(home.clone(), runtime, manager.clone())
        .await
        .unwrap();
    agent.0.state.lock().await.connection = Some(connection.clone());
    let mut existing_id = None;
    if restore {
        let old=agent.operation("send",json!({"text":"记住：我想下载纽约最新的卫星影像。现在仅记住需求，不要检索或执行。简短回复即可。"})).await.unwrap();
        let id = old["selected"]["id"].as_str().unwrap().to_owned();
        let before = wait_for_turn(&connection).await;
        let old_thread = before["selected"]["threadId"].as_str().unwrap().to_owned();
        let upgraded = connection
            .rpc(
                "configure",
                json!({"config":config,"definitions":definitions()}),
            )
            .await
            .unwrap();
        assert_eq!(upgraded["selected"]["id"], id);
        assert_eq!(upgraded["sessions"][0]["compatible"], true);
        assert!(upgraded["selected"]["threadId"].is_null());
        assert!(
            upgraded["selected"]["entries"].as_array().unwrap().len()
                >= before["selected"]["entries"].as_array().unwrap().len()
        );
        existing_id = Some((id, old_thread));
    }
    // Same human prompt as the reported failure; unrelated San Francisco map.
    agent
        .operation(
            "send",
            json!({"sessionId":existing_id.as_ref().map(|(id,_)|id),"text":if restore {"重试"} else {"我要下载纽约最新的卫星影像"},"context":{
        "page":"Explore","provider":"earth-search","bounds":[-122.55,37.68,-122.32,37.84],
        "start":"2026-09-07","end":"2026-10-06","cloudMax":60,"projectId":null}}),
        )
        .await
        .unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(240);
    let snapshot = loop {
        let value = connection.rpc("snapshot", json!({})).await.unwrap();
        if value["busy"] == false {
            break value;
        }
        if tokio::time::Instant::now() >= deadline {
            tokio::fs::write(
                base.join("timed-out-snapshot.json"),
                serde_json::to_vec_pretty(&value).unwrap(),
            )
            .await
            .unwrap();
            agent.shutdown().await;
            manager.shutdown().await.unwrap();
            panic!("Place workflow exceeded bounded acceptance turn");
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    };
    assert!(!snapshot.to_string().contains(secret.as_str()));
    tokio::fs::write(
        base.join("snapshot.json"),
        serde_json::to_vec_pretty(&snapshot).unwrap(),
    )
    .await
    .unwrap();
    let entries = snapshot["selected"]["entries"].as_array().unwrap();
    for name in [
        "geod_place_search",
        "geod_sources_list",
        "geod_workspace_context",
        "geod_scene_search",
        "geod_download_plan",
    ] {
        assert!(
            entries
                .iter()
                .any(|entry| entry["name"] == name && entry["status"] == "completed"),
            "missing completed native tool {name}; inspect retained snapshot"
        );
    }
    let session = snapshot["selected"]["id"].as_str().unwrap();
    if let Some((id, old_thread)) = &existing_id {
        assert_eq!(session, id);
        assert_ne!(snapshot["selected"]["threadId"], *old_thread);
    }
    let plan_id = entries
        .iter()
        .rev()
        .filter(|v| v["name"] == "geod_download_plan")
        .find_map(|entry| {
            entry["references"]
                .as_array()?
                .iter()
                .find(|r| r["kind"] == "plan")?["id"]
                .as_str()
        })
        .expect("download plan reference");
    let plan = manager.agent_plan_status(session, plan_id).await.unwrap();
    assert_eq!(plan["status"], "pending");
    // Native review has New York bounds, not the unrelated map area.
    let bounds = plan["bounds"].as_array().expect("native search bounds");
    assert!(bounds[0].as_f64().unwrap() > -75.0 && bounds[2].as_f64().unwrap() < -73.0);
    assert_eq!(plan["source"], "Earth Search · Sentinel-2 L2A");
    assert!(plan["files"]
        .as_array()
        .unwrap()
        .iter()
        .all(|file| file["assetKey"] == "visual"));
    let final_jobs: std::collections::BTreeSet<_> =
        manager.list().await.into_iter().map(|job| job.id).collect();
    assert_eq!(
        final_jobs, original_jobs,
        "review mode queues no task and preserves existing jobs"
    );
    if actual_profile {
        assert_eq!(manager.proxy_settings().await.mode, original_proxy.mode);
    }
    let answer = entries
        .iter()
        .rev()
        .find(|entry| entry["type"] == "assistant")
        .unwrap()["text"]
        .as_str()
        .unwrap();
    assert!(
        answer.chars().count() <= 650,
        "simple review response must remain compact"
    );
    assert!(
        !answer.contains("共同覆盖") && !answer.contains("完全覆盖"),
        "catalog intersection is not complete coverage proof"
    );
    assert!(
        !answer.contains("文本回复不能代替确认") && !answer.contains("只能在卡片"),
        "native human chat confirmation remains available"
    );
    let receipt = json!({"schema":"geod-agent-place-model-acceptance/v1","status":"passed",
        "humanMessages":if restore {2} else {1},"sameConversationRestoredAfterToolsUpgrade":restore,"coordinateParametersSupplied":false,"currentMapAreaOverridden":true,
        "realPlaceQuery":true,"realCatalogQuery":true,"nativeDownloadReview":plan,
        "persistedJobCount":final_jobs.len(),"addedJobCount":0,"usedActualProfileStorage":actual_profile,"cardClicks":0,"snapshot":snapshot,"modelRoute":model,
        "upstreamVendorVerified":false,"usedUserDesktop":false,"credentialVaultWritten":false,
        "nativeProxyMode":manager.proxy_settings().await.mode});
    tokio::fs::write(
        base.join("native-acceptance.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .await
    .unwrap();
    agent.shutdown().await;
    manager.shutdown().await.unwrap();
}

async fn wait_for_turn(connection: &Arc<Connection>) -> Value {
    tokio::time::timeout(Duration::from_secs(90), async {
        loop {
            let value = connection.rpc("snapshot", json!({})).await.unwrap();
            if value["busy"] == false {
                assert_eq!(value["selected"]["status"], "completed");
                return value;
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    })
    .await
    .expect("bounded restoration setup turn")
}
