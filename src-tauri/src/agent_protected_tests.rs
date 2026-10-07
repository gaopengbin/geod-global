//! Live-model public discovery and review of account-backed originals.
//! No provider account, protected-file transfer or user desktop operation.
use super::tests::{assistant_text, completed};
use super::*;

fn calls(snapshot: &Value, name: &str) -> usize {
    snapshot["selected"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == "tool" && e["status"] == "completed" && e["name"] == name)
        .count()
}
fn pending(snapshot: &Value, kind: &str, project: Option<&str>) -> Value {
    snapshot["plans"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .find(|p| {
            p["kind"] == kind
                && p["status"] == "pending"
                && project.is_none_or(|id| p["project"]["id"] == id)
        })
        .unwrap()
        .clone()
}
fn successful(snapshot: &Value) {
    assert_eq!(
        snapshot["selected"]["status"], "completed",
        "{}",
        snapshot["selected"]["error"]
    );
    assert!(!snapshot["selected"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["type"] == "tool" && e["status"] == "failed"));
}

#[tokio::test]
#[ignore = "Explicit model key, owned Agent runtime, six real public catalogs, fresh QA store; no protected originals or provider credentials"]
async fn live_protected_sources_model_reviews_and_resume_without_accounts() {
    let secret =
        Zeroizing::new(std::env::var("GEOD_AGENT_TEST_KEY").expect("explicit model test key"));
    let endpoint = std::env::var("GEOD_AGENT_TEST_BASE_URL").expect("explicit model endpoint");
    let model = std::env::var("GEOD_AGENT_TEST_MODEL").expect("explicit model route");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap();
    let requested = PathBuf::from(
        std::env::var("GEOD_AGENT_PROTECTED_MODEL_QA").expect("fresh isolated QA store"),
    );
    assert!(requested.is_absolute());
    let parent = requested.parent().unwrap().canonicalize().unwrap();
    assert!(parent.starts_with(root.join(".verification").canonicalize().unwrap()));
    let base = parent.join(requested.file_name().unwrap());
    assert!(!base.exists());
    tokio::fs::create_dir_all(base.join("sessions"))
        .await
        .unwrap();
    let home = base.join("sessions");
    let manager = JobManager::open(base.join("core")).await.unwrap();
    let accounts_before = serde_json::to_value(manager.provider_accounts().await).unwrap();
    assert!(accounts_before
        .as_array()
        .unwrap()
        .iter()
        .all(|a| a["status"] == "not-connected"));
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
    let config = json!({"label":"Isolated protected-source public acceptance","protocol":"openai-compatible","baseUrl":endpoint,"model":model,"apiKey":secret.as_str()});
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
    let inputs: Value = serde_json::from_slice(
        &tokio::fs::read(root.join("scripts/acceptance/protected-catalog-public.json"))
            .await
            .unwrap(),
    )
    .unwrap();
    let cases = inputs.as_array().unwrap();
    assert_eq!(cases.len(), 6);
    let mut session: Option<String> = None;
    let mut previous_calls = json!({"selected":{"entries":[]}});
    let mut evidence = Vec::new();
    for (index, query) in cases.iter().enumerate() {
        let provider = query["provider"].as_str().unwrap();
        let (asset, format) = match provider {
            "copernicus" => ("product", "SAFE ZIP"),
            "nasa-earthdata" => ("red", "GeoTIFF"),
            "nasa-srtm" => ("srtm", "HGT ZIP"),
            "nasa-viirs-suomi" | "nasa-viirs-noaa20" | "nasa-viirs-noaa21" => ("viirs", "HDF5"),
            _ => panic!("Unreviewed protected QA source"),
        };
        let sent=agent.operation("send",json!({"sessionId":session,"text":format!("请在软件中核对数据源能力，然后按以下明确条件搜索公开目录，只选返回的第一景，准备一个名为 Protected model {provider} 的工程审核方案。查询条件：{query}。我现在没有 NASA 或 Copernicus 账号；公开检索和保存工程不需要登录。只准备工程方案，等待软件卡片确认，不下载文件，不接收密码或令牌。保留此数据源真实的时间含义。"),"context":null})).await.unwrap();
        if session.is_none() {
            session = Some(sent["selected"]["id"].as_str().unwrap().into());
        }
        let session_id = session.as_ref().unwrap();
        let prepared = completed(&connection).await;
        successful(&prepared);
        // Search returns the selected source's native capability declaration.
        // A fresh inventory call on every turn is not needed to establish it.
        assert!(calls(&prepared, "geod_sources_list") >= 1);
        for tool in ["geod_scene_search", "geod_project_plan"] {
            assert!(
                calls(&prepared, tool) > calls(&previous_calls, tool),
                "Missing actual model call: {provider} {tool}"
            );
        }
        assert!(manager.list().await.is_empty());
        assert_eq!(manager.list_projects().await.len(), index);
        let original = pending(&agent.snapshot().await.unwrap(), "project", None);
        assert_eq!(original["project"]["sceneCount"], 1);
        let scene_id = original["files"][0]["itemId"].as_str().unwrap();
        let identity_prefix = match provider {
            "copernicus" => "S2",
            "nasa-earthdata" => "HLS.L30.",
            "nasa-srtm" => "N37W123.SRTMGL1.hgt",
            "nasa-viirs-suomi" => "VNP09A1.",
            "nasa-viirs-noaa20" => "VJ109A1.",
            "nasa-viirs-noaa21" => "VJ209A1.",
            _ => unreachable!(),
        };
        assert!(scene_id.starts_with(identity_prefix));
        let confirmed_review = if index == 0 {
            let old_id = original["planId"].as_str().unwrap();
            let old_hash = original["planHash"].as_str().unwrap();
            let draft = agent
                .plan_revision_draft(session_id, old_id, old_hash)
                .await
                .unwrap();
            let changed = agent
                .revise_plan(
                    session_id,
                    old_id,
                    old_hash,
                    agent_actions::PlanRevision::Project {
                        item_ids: serde_json::from_value(draft["parameters"]["itemIds"].clone())
                            .unwrap(),
                        name: Some("Confirmed protected model project".into()),
                        bounds: Some(serde_json::from_value(query["bounds"].clone()).unwrap()),
                        keep_polygon: false,
                    },
                )
                .await
                .unwrap();
            let revised = pending(&changed, "project", None);
            assert_ne!(revised["planHash"], original["planHash"]);
            assert!(agent
                .approve_plan(session_id, old_id, old_hash)
                .await
                .is_err());
            revised
        } else {
            original.clone()
        };
        let project_id = confirmed_review["project"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        agent
            .approve_plan(
                session_id,
                confirmed_review["planId"].as_str().unwrap(),
                confirmed_review["planHash"].as_str().unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(manager.list_projects().await.len(), index + 1);
        assert!(manager.list().await.is_empty());
        agent.operation("send",json!({"sessionId":session_id,"text":format!("工程 {project_id} 已由我在软件卡片中确认保存。请重新读取真实工程，并为它准备 {asset} 原文件下载审核方案。仅生成方案，不执行下载。准确说明原文件格式、下载前编码大小是否已知，以及我尚未连接数据账号时下一步应在哪里授权。不要把已保存工程或公开目录结果当作文件下载成功。"),"context":null})).await.unwrap();
        let planned = completed(&connection).await;
        successful(&planned);
        for tool in ["geod_project_get", "geod_project_download_plan"] {
            assert!(
                calls(&planned, tool) > calls(&prepared, tool),
                "Missing actual model call: {provider} {tool}"
            );
        }
        let review = pending(
            &agent.snapshot().await.unwrap(),
            "download",
            Some(&project_id),
        );
        assert_eq!(review["project"]["id"], project_id);
        assert_eq!(review["project"]["saved"], true);
        assert_eq!(review["project"]["committed"], false);
        assert_eq!(review["status"], "pending");
        assert_eq!(review["format"], format);
        assert!(review["expectedBytes"].is_null());
        assert_eq!(review["files"][0]["assetKey"], asset);
        assert_eq!(review["authorization"]["downloadEnabled"], false);
        assert_eq!(review["authorization"]["entitlement"], "not-checked");
        assert!(review["jobs"].as_array().unwrap().is_empty());
        for _ in 0..2 {
            assert!(agent
                .approve_plan(
                    session_id,
                    review["planId"].as_str().unwrap(),
                    review["planHash"].as_str().unwrap()
                )
                .await
                .unwrap_err()
                .contains("Settings"));
        }
        assert!(manager.list().await.is_empty());
        assert_eq!(
            serde_json::to_value(manager.provider_accounts().await).unwrap(),
            accounts_before
        );
        evidence.push(json!({"provider":provider,"query":query,"originalProjectReview":original,"confirmedProjectReview":confirmed_review,"downloadReview":review,"nativeDownloadConfirmationRefused":true,"modelPreparedProject":prepared,"modelPreparedDownload":planned,"assistantText":assistant_text(&planned)}));
        previous_calls = planned;
        tokio::fs::write(base.join("progress.json"),serde_json::to_vec_pretty(&json!({"status":"running","completedProviders":index+1,"lastProvider":provider,"jobCount":0})).unwrap()).await.unwrap();
        println!("Completed actual protected-source model review: {provider}");
    }
    let session_id = session.unwrap();
    let thread = previous_calls["selected"]["threadId"].clone();
    let projects_before = tokio::fs::read(base.join("core/projects.json"))
        .await
        .unwrap();
    let latest = evidence.last().unwrap()["downloadReview"].clone();
    agent.shutdown().await;
    manager.shutdown().await.unwrap();
    drop(agent);
    drop(connection);
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
    agent.operation("send",json!({"sessionId":session_id,"text":format!("软件已经重启，请用新的 geod_sources_list、geod_plan_status(planId={}) 和 geod_project_get(id={}) 结果核查刚才 NOAA-21 的工程和下载方案。确认尚未下载、大小未知、HDF5 原件需要在设置中授权。只核对，不执行下载，也不依赖旧回复作为当前状态。",latest["planId"],latest["project"]["id"]),"context":null})).await.unwrap();
    let resumed = completed(&connection).await;
    successful(&resumed);
    assert_eq!(resumed["selected"]["id"], session_id);
    assert_eq!(resumed["selected"]["threadId"], thread);
    for tool in ["geod_sources_list", "geod_plan_status", "geod_project_get"] {
        assert!(calls(&resumed, tool) > calls(&previous_calls, tool));
    }
    for case in &evidence {
        let review = &case["downloadReview"];
        let restored = reopened
            .agent_plan_status(&session_id, review["planId"].as_str().unwrap())
            .await
            .unwrap();
        assert_eq!(restored["planHash"], review["planHash"]);
        assert_eq!(restored["authorization"]["downloadEnabled"], false);
        assert!(reopened
            .approve_agent_plan(
                &session_id,
                review["planId"].as_str().unwrap(),
                review["planHash"].as_str().unwrap()
            )
            .await
            .is_err());
    }
    assert!(reopened.list().await.is_empty());
    assert_eq!(reopened.list_projects().await.len(), 6);
    assert_eq!(
        tokio::fs::read(base.join("core/projects.json"))
            .await
            .unwrap(),
        projects_before
    );
    assert_eq!(
        serde_json::to_value(reopened.provider_accounts().await).unwrap(),
        accounts_before
    );
    let receipt = json!({"schema":"geod-agent-protected-model-acceptance/v1","status":"passed","publicMetadataUsed":true,"syntheticCatalog":false,"modelUsed":true,"modelRoute":model,"upstreamVendorVerified":false,"usedUserDesktop":false,"providerCredentialVaultWritten":false,"modelCredentialVaultWritten":false,"published":false,"protectedOriginalDownloaded":false,"jobCount":0,"projectCount":6,"sessionId":session_id,"resumedSameConversation":true,"resumedSameModelThread":true,"cases":evidence,"afterRestart":resumed});
    tokio::fs::write(
        base.join("native-acceptance.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .await
    .unwrap();
    agent.shutdown().await;
    reopened.shutdown().await.unwrap();
}
