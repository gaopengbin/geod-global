use super::*;

#[tokio::test]
#[ignore = "owned model test key; one actual public SCL download and crop in an isolated store"]
async fn live_natural_language_download_crop_and_open_without_card_clicks() {
    let secret =
        Zeroizing::new(std::env::var("GEOD_AGENT_TEST_KEY").expect("explicit owned test key"));
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap();
    let base = root
        .join(".verification")
        .join(format!("agent-conversation-{}", registry::new_id()));
    let home = base.join("sessions");
    tokio::fs::create_dir_all(&home).await.unwrap();
    let manager = JobManager::open(base.join("core")).await.unwrap();
    manager
        .save_proxy_settings(geod_runtime::ProxySettings {
            mode: geod_runtime::proxy::ProxyMode::Custom,
            url: Some("http://127.0.0.1:7890".into()),
        })
        .await
        .unwrap();
    let runtime = root.join(".agent-runtime/win32-x64");
    verify_runtime(&runtime).await.unwrap();
    let connection = Connection::spawn(&runtime, &home, manager.clone())
        .await
        .unwrap();
    connection.rpc("configure",json!({"config":{"label":"Owned natural-language acceptance","protocol":"openai-compatible","baseUrl":"http://127.0.0.1:19094/v1","model":"deepseek-v4-flash","apiKey":secret.as_str()},"definitions":definitions()})).await.unwrap();
    let agent = DesktopAgent::open(home.clone(), runtime, manager.clone())
        .await
        .unwrap();
    agent.0.state.lock().await.connection = Some(connection.clone());
    agent
        .execution_mode(None, ExecutionMode::FullAccess)
        .await
        .unwrap();
    let first=agent.operation("send",json!({"text":"请完成这个任务：在当前地图范围和日期内，从 Earth Search 找一景 SCL 影像，保存为新的工程并下载 SCL 原文件。下载完成后按当前地图范围裁剪，检查实际成果，然后在地图工作区打开裁剪结果。工程叫 自然语言流程验收。只要一景，只做这项任务，直接完成。","context":{"page":"Explore","provider":"earth-search","bounds":[-122.46,37.76,-122.45,37.77],"start":"2025-06-01","end":"2025-06-30","cloudMax":60,"projectId":null}})).await.unwrap();
    let session = first["selected"]["id"].as_str().unwrap().to_owned();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(300);
    let final_value = loop {
        let value = connection.rpc("snapshot", json!({})).await.unwrap();
        if value["selected"]["workflow"]["status"] == "completed"
            && value["busy"] == false
            && value["selected"]["workspaceView"]["verified"] == true
        {
            break value;
        }
        if tokio::time::Instant::now() > deadline
            || value["selected"]["workflow"]["status"] == "failed"
            || value["selected"]["status"] == "failed"
        {
            tokio::fs::write(
                base.join("failed-snapshot.json"),
                serde_json::to_vec_pretty(&value).unwrap(),
            )
            .await
            .unwrap();
            agent.shutdown().await;
            manager.shutdown().await.unwrap();
            panic!(
                "Natural language flow did not finish; inspect isolated receipt {}",
                base.display()
            );
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    };
    assert_eq!(final_value["selected"]["id"], session);
    assert!(!final_value["selected"]["threadId"].is_null());
    assert!(
        final_value["selected"]["workflow"]["continuations"]
            .as_u64()
            .unwrap()
            >= 1
    );
    let jobs = manager.list().await;
    assert_eq!(jobs.len(), 2, "one original and one crop only");
    assert!(jobs
        .iter()
        .all(|job| job.status == geod_runtime::JobStatus::Succeeded));
    let output_id = final_value["selected"]["workspaceView"]["id"]
        .as_str()
        .unwrap();
    let raster = agent_actions::call(
        manager.clone(),
        &session,
        "geod_raster_inspect",
        json!({"id":output_id}),
        None,
    )
    .await
    .unwrap();
    let projects = manager.list_projects().await;
    assert_eq!(projects.len(), 1);
    assert!(raster["width"].as_u64().unwrap() < 100 && raster["height"].as_u64().unwrap() < 100);
    let snapshot = agent.snapshot().await.unwrap();
    assert!(!snapshot.to_string().contains(secret.as_str()));
    let receipt = json!({"schema":"geod-natural-language-workflow-acceptance/v1","status":"passed","modelRoute":"deepseek-v4-flash","upstreamVendorVerified":false,"nativeTasks":jobs,"raster":raster,"snapshot":snapshot,"humanMessages":1,"cardClicks":0,"automaticContinuation":true,"workspaceOpenRequestVerified":true,"nativeMapRenderObserved":false,"usedUserDesktop":false,"credentialVaultWritten":false});
    tokio::fs::write(
        base.join("acceptance.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .await
    .unwrap();
    tokio::fs::write(
        root.join(".verification/agent-conversation-latest.json"),
        serde_json::to_vec_pretty(&json!({"directory":base})).unwrap(),
    )
    .await
    .unwrap();
    agent.shutdown().await;
    manager.shutdown().await.unwrap();
    println!("Natural language acceptance passed: one prompt, actual SCL download, project crop, file verification and managed map-open request.");
}
