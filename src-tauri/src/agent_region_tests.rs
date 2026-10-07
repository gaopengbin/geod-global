use super::*;

#[tokio::test]
#[ignore = "owned model plus real global administrative source queries in an isolated read-only store"]
async fn live_global_administrative_dialogue_without_coordinates() {
    let secret = Zeroizing::new(std::env::var("GEOD_AGENT_TEST_KEY").expect("owned test key"));
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap();
    let base = PathBuf::from(
        std::env::var_os("GEOD_AGENT_REGIONS_MODEL_QA").expect("isolated QA directory"),
    );
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
    let model =
        std::env::var("GEOD_AGENT_TEST_MODEL").unwrap_or_else(|_| "deepseek-v4-flash".into());
    let url = std::env::var("GEOD_AGENT_TEST_BASE_URL").expect("owned loopback model route");
    connection.rpc("configure",json!({"config":{"label":"Owned global administrative acceptance","protocol":"openai-compatible","baseUrl":url,"model":model,"apiKey":secret.as_str()},"definitions":definitions()})).await.unwrap();
    let agent = DesktopAgent::open(home, runtime, manager.clone())
        .await
        .unwrap();
    agent.0.state.lock().await.connection = Some(connection.clone());
    // Human names and administrative types only; no ISO codes, levels, bounds,
    // tool names or desktop interaction supplied to the model.
    agent.operation("send",json!({"text":"查询中国浙江省、德国北莱茵-威斯特法伦州和印度浦那县的行政区范围，简短列出匹配名称、行政层级和边界数据来源。只查询，不下载。"})).await.unwrap();
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
            panic!("global administrative acceptance timed out");
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    };
    tokio::fs::write(
        base.join("snapshot.json"),
        serde_json::to_vec_pretty(&snapshot).unwrap(),
    )
    .await
    .unwrap();
    assert!(!snapshot.to_string().contains(secret.as_str()));
    let entries = snapshot["selected"]["entries"].as_array().unwrap();
    for tool in ["geod_region_search", "geod_region_levels"] {
        assert!(
            entries
                .iter()
                .any(|e| e["name"] == tool && e["status"] == "completed"),
            "missing {tool}; inspect retained snapshot"
        );
    }
    let answer = entries
        .iter()
        .rev()
        .find(|e| e["type"] == "assistant")
        .unwrap()["text"]
        .as_str()
        .unwrap();
    assert!(
        answer.contains("浙江")
            && (answer.contains("浦那") || answer.contains("Pune"))
            && (answer.contains("威斯特") || answer.contains("Westphalia")),
        "inspect retained answer"
    );
    assert!(!answer.contains("请提供坐标") && !answer.contains("没有地名解析"));
    assert!(manager.list().await.is_empty());
    let receipt = json!({"schema":"geod-agent-global-regions-model/v1","status":"passed","humanMessages":1,
        "coordinatesSupplied":false,"isoCodesSupplied":false,"adminLevelsSupplied":false,"realNativeRegionQueries":true,
        "persistedJobCount":0,"modelRoute":model,"upstreamVendorVerified":false,"usedUserDesktop":false,"snapshot":snapshot});
    tokio::fs::write(
        base.join("native-acceptance.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .await
    .unwrap();
    agent.shutdown().await;
    manager.shutdown().await.unwrap();
}
