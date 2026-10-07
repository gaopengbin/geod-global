//! Explicit live-model acceptance of the custom-source workflow. The model
//! creates the native reviews; only this isolated harness confirms their cards.
use super::tests::{assistant_text, completed};
use super::*;

fn tool_names(snapshot: &Value) -> Vec<String> {
    snapshot["selected"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["type"] == "tool" && entry["status"] == "completed")
        .filter_map(|entry| entry["name"].as_str().map(str::to_owned))
        .collect()
}

async fn finish(connection: &Connection, stage: &str) -> Value {
    let snapshot = completed(connection).await;
    assert_eq!(
        snapshot["selected"]["status"],
        "completed",
        "{stage}: {}; diagnostics: {}",
        snapshot["selected"]["error"],
        connection.diagnostics.lock().await
    );
    snapshot
}

fn pending(snapshot: &Value, kind: &str) -> Value {
    let plans: Vec<_> = snapshot["plans"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|plan| plan["status"] == "pending" && plan["kind"] == kind)
        .cloned()
        .collect();
    assert_eq!(plans.len(), 1, "Expected one model-created {kind} review");
    plans[0].clone()
}

#[tokio::test]
#[ignore = "Explicit model credential, owned Agent runtime, fresh isolated store and actual public SCL download"]
async fn live_custom_source_model_project_download_and_resume() {
    let secret = Zeroizing::new(std::env::var("GEOD_AGENT_TEST_KEY").expect("explicit test key"));
    let endpoint =
        std::env::var("GEOD_AGENT_TEST_BASE_URL").unwrap_or("http://127.0.0.1:19094/v1".into());
    let model = std::env::var("GEOD_AGENT_TEST_MODEL").unwrap_or("deepseek-v4-flash".into());
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap();
    let requested = PathBuf::from(
        std::env::var("GEOD_AGENT_STAC_MODEL_QA").expect("fresh isolated QA directory"),
    );
    assert!(requested.is_absolute());
    let parent = requested.parent().unwrap().canonicalize().unwrap();
    assert!(parent.starts_with(root.join(".verification").canonicalize().unwrap()));
    let base = parent.join(requested.file_name().unwrap());
    assert!(!base.exists());
    tokio::fs::create_dir_all(&base).await.unwrap();
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
    let source = manager
        .connect_stac(geod_runtime::stac::ConnectRequest {
            name: "Live Agent custom Earth Search acceptance".into(),
            kind: "api".into(),
            url: "https://earth-search.aws.element84.com/v1/".into(),
        })
        .await
        .unwrap();
    let runtime = root.join(".agent-runtime/win32-x64");
    verify_runtime(&runtime).await.unwrap();
    let config = json!({"label":"Isolated custom-source model acceptance","protocol":"openai-compatible","baseUrl":endpoint,"model":model,"apiKey":secret.as_str()});
    let connection = Connection::spawn(&runtime, &home, manager.clone())
        .await
        .unwrap();
    connection
        .rpc(
            "configure",
            json!({"config":config,"definitions":definitions()}),
        )
        .await
        .unwrap();
    let agent = DesktopAgent::open(home.clone(), runtime.clone(), manager.clone())
        .await
        .unwrap();
    agent.0.state.lock().await.connection = Some(connection.clone());
    let first = agent.operation("send",json!({
        "text":format!("请用已保存的自定义数据源 {}（Live Agent custom Earth Search acceptance），不要用固定供应商搜索。读取当前区域与日期，查看来源和目录，选择实际 sentinel-2-l2a 集合，用 geod_stac_search 最多查询 3 条。选其中一条，读取它的快照与资产页，必要时跟进 nextOffset，找 eligible 的原始 scl 文件。仅为这一个文件准备名为 Custom SCL model acceptance 的 geod_stac_project_plan，工程范围用当前区域。不要创建工程或下载，等待我确认工程卡片。",source.id),
        "context":{"page":"Explore","provider":"earth-search","bounds":[-122.46,37.76,-122.45,37.77],"start":"2025-06-01","end":"2025-06-30","cloudMax":60,"projectId":null}
    })).await.unwrap();
    let session = first["selected"]["id"].as_str().unwrap().to_owned();
    let project_turn = finish(&connection, "custom metadata and project review").await;
    let initial_names = tool_names(&project_turn);
    for tool in [
        "geod_workspace_context",
        "geod_stac_connections",
        "geod_stac_catalog",
        "geod_stac_search",
        "geod_stac_snapshot",
        "geod_stac_assets",
        "geod_stac_project_plan",
    ] {
        assert!(
            initial_names.iter().any(|name| name == tool),
            "Missing actual model call: {tool}"
        );
    }
    assert!(!initial_names.iter().any(|name| name == "geod_scene_search"));
    let project_plan = pending(&agent.snapshot().await.unwrap(), "project");
    assert!(manager.list_projects().await.is_empty());
    assert!(manager.list().await.is_empty());
    let accepted = agent
        .approve_plan(
            &session,
            project_plan["planId"].as_str().unwrap(),
            project_plan["planHash"].as_str().unwrap(),
        )
        .await
        .unwrap();
    let confirmed = accepted["plans"]
        .as_array()
        .unwrap()
        .iter()
        .find(|plan| plan["planId"] == project_plan["planId"])
        .unwrap();
    let project_id = confirmed["project"]["id"].as_str().unwrap().to_owned();
    let projects = manager.list_projects().await;
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].stac_items.len(), 1);
    assert_eq!(projects[0].stac_items[0].asset_key, "scl");
    assert!(manager.list().await.is_empty());
    agent.operation("send", json!({"sessionId":session,"text":format!("工程卡片已确认。读取实际工程 {project_id}，为其中的原始 scl 准备 geod_stac_download_plan。只准备下载卡片，不执行，告诉我实际预估字节数。"),"context":null})).await.unwrap();
    finish(&connection, "custom original download review").await;
    let download_plan = pending(&agent.snapshot().await.unwrap(), "download");
    assert_eq!(download_plan["files"].as_array().unwrap().len(), 1);
    assert!(download_plan["expectedBytes"].as_u64().unwrap() <= 16 * 1024 * 1024);
    assert!(manager.list().await.is_empty());
    let accepted = agent
        .approve_plan(
            &session,
            download_plan["planId"].as_str().unwrap(),
            download_plan["planHash"].as_str().unwrap(),
        )
        .await
        .unwrap();
    let submitted = accepted["plans"]
        .as_array()
        .unwrap()
        .iter()
        .find(|plan| plan["planId"] == download_plan["planId"])
        .unwrap();
    let job_id = submitted["jobs"][0]["id"].as_str().unwrap().to_owned();
    let completed_job = tokio::time::timeout(Duration::from_secs(180), manager.wait(&job_id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        completed_job.status,
        geod_runtime::JobStatus::Succeeded,
        "{:?}",
        completed_job.error
    );
    let inspection = manager.inspect_stac_asset(&job_id).await.unwrap();
    let pixel = manager.sample_stac_asset(&job_id, 0, 0).await.unwrap();
    let sha = completed_job.sha256.clone().unwrap();
    agent.operation("send",json!({"sessionId":session,"text":format!("下载卡片已确认。用 geod_plan_status 读取计划 {} 的实际结果，然后 geod_stac_inspect 检查已完成的原文件，geod_stac_pixel 读取 column=0,row=0。只报告实际任务编号、settled、SHA-256、字节数和原始像元值。最后再调用 geod_stac_download_plan 检查同一工程是否能直接复用，不创建或确认新任务。",download_plan["planId"].as_str().unwrap()),"context":null})).await.unwrap();
    let read_turn = finish(&connection, "completed custom file and original sample").await;
    assert!(assistant_text(&read_turn).contains(&sha));
    for tool in [
        "geod_stac_download_plan",
        "geod_plan_status",
        "geod_stac_inspect",
        "geod_stac_pixel",
    ] {
        assert!(
            tool_names(&read_turn).iter().any(|name| name == tool),
            "Missing actual model call: {tool}"
        );
    }
    assert_eq!(manager.list().await.len(), 1);
    assert!(!agent.snapshot().await.unwrap()["plans"]
        .as_array()
        .unwrap()
        .iter()
        .any(|plan| plan["status"] == "pending"));
    let duplicate = agent
        .approve_plan(
            &session,
            download_plan["planId"].as_str().unwrap(),
            download_plan["planHash"].as_str().unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        duplicate["plans"]
            .as_array()
            .unwrap()
            .iter()
            .find(|plan| plan["planId"] == download_plan["planId"])
            .unwrap()["jobs"][0]["id"],
        job_id
    );
    let thread_id = read_turn["selected"]["threadId"].clone();
    agent.shutdown().await;
    drop(agent);
    drop(connection);
    manager.shutdown().await.unwrap();
    drop(manager);
    let reopened = JobManager::open(base.join("core")).await.unwrap();
    let connection = Connection::spawn(&runtime, &home, reopened.clone())
        .await
        .unwrap();
    connection
        .rpc(
            "configure",
            json!({"config":config,"definitions":definitions()}),
        )
        .await
        .unwrap();
    let agent = DesktopAgent::open(home, runtime, reopened.clone())
        .await
        .unwrap();
    agent.0.state.lock().await.connection = Some(connection.clone());
    agent.operation("send",json!({"sessionId":session,"text":"软件已重启。请使用先前对话中的同一工程和任务编号，重新调用 geod_project_get、geod_job_status、geod_stac_inspect、geod_stac_pixel(column=0,row=0)，再用 geod_stac_download_plan 检查复用。报告当前工程名、当前任务 settled 和原文件 SHA-256，不用记忆代替工具结果，不创建新任务。","context":null})).await.unwrap();
    let resumed = finish(&connection, "process restart and fresh custom-file reads").await;
    assert_eq!(resumed["selected"]["threadId"], thread_id);
    assert!(assistant_text(&resumed).contains(&sha));
    let names = tool_names(&resumed);
    assert!(
        names
            .iter()
            .filter(|name| *name == "geod_stac_inspect")
            .count()
            >= 2
    );
    assert!(
        names
            .iter()
            .filter(|name| *name == "geod_stac_pixel")
            .count()
            >= 2
    );
    assert!(
        names
            .iter()
            .filter(|name| *name == "geod_stac_download_plan")
            .count()
            >= 3
    );
    assert_eq!(reopened.list().await.len(), 1);
    assert_eq!(reopened.list_projects().await[0].id, project_id);
    assert_eq!(
        reopened
            .sample_stac_asset(&job_id, 0, 0)
            .await
            .unwrap()
            .values,
        pixel.values
    );
    let mut final_snapshot = agent.snapshot().await.unwrap();
    final_snapshot["configured"] = json!(false);
    final_snapshot["model"] = Value::Null;
    let receipt = json!({"schema":"geod-agent-custom-model-acceptance/v1","status":"passed","modelRoute":model,"protocol":"OpenAI-compatible Chat Completions through owned Responses bridge / Codex App Server","upstreamVendorVerified":false,"connectionId":source.id,"projectPlan":project_plan,"downloadPlan":download_plan,"projectId":project_id,"jobId":job_id,"sha256":sha,"bytes":completed_job.bytes_downloaded,"inspection":inspection,"pixel":pixel,"nativeTools":names,"pendingBeforeExplicitConfirmations":true,"duplicateConfirmationReusesJob":true,"processRestartAndThreadResume":true,"freshNativeResultsAfterRestart":true,"persistedJobCount":1,"usedUserDesktop":false,"credentialVaultWritten":false,"nativeWindowsWindowTested":false,"snapshot":final_snapshot});
    assert!(!receipt.to_string().contains(secret.as_str()));
    tokio::fs::write(
        base.join("native-acceptance.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .await
    .unwrap();
    agent.shutdown().await;
    drop(connection);
    reopened.shutdown().await.unwrap();
    println!("Custom-source live Agent acceptance passed: model-created project and download reviews, real original, fresh reads and restart reuse.");
}
