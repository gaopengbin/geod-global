//! Explicit live-model WCS workflow using the actual desktop confirmation boundary.
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
fn review(snapshot: &Value, kind: &str) -> Value {
    snapshot["plans"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["kind"] == kind && p["status"] == "pending")
        .unwrap()
        .clone()
}

#[tokio::test]
#[ignore = "Explicit model credential, owned runtime, isolated WCS source and native confirmations"]
async fn live_wcs_model_project_download_and_resume() {
    let source: Value = serde_json::from_str(
        &std::env::var("GEOD_AGENT_WCS_SOURCE_QA").expect("explicit WCS QA source"),
    )
    .unwrap();
    let secret =
        Zeroizing::new(std::env::var("GEOD_AGENT_TEST_KEY").expect("explicit downstream test key"));
    let endpoint = std::env::var("GEOD_AGENT_TEST_BASE_URL").expect("explicit test model endpoint");
    let model = std::env::var("GEOD_AGENT_TEST_MODEL").expect("explicit model route");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap();
    let requested = PathBuf::from(
        std::env::var("GEOD_AGENT_WCS_MODEL_QA").expect("fresh isolated QA directory"),
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
    // Explicit user setup connects this source; the model has no connection tool.
    let service = Box::pin(manager.connect_wcs(geod_runtime::wcs::ConnectRequest {
        name: source["name"].as_str().unwrap().into(),
        url: source["url"].as_str().unwrap().into(),
    }))
    .await
    .unwrap();
    let runtime = root.join(".agent-runtime/win32-x64");
    verify_runtime(&runtime).await.unwrap();
    let config = json!({"label":"Isolated WCS acceptance","protocol":"openai-compatible","baseUrl":endpoint,"model":model,"apiKey":secret.as_str()});
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
    let sent=agent.operation("send",json!({"text":format!("先调用 geod_wcs_connections 和 geod_wcs_coverages(id={}) 核查软件已连接的 WCS 服务。选择真实 coverageId={}，调用 geod_wcs_describe 读取定义，再用 geod_wcs_prepare 根据用户指定的 WGS84 范围 {} 准备原生网格。然后调用 geod_wcs_project_plan，名称为 WCS model review，bounds 使用上述范围、selections 使用刚返回的真实 planId。只创建待确认工程方案，不保存工程、不下载。报告实际区域和网格；不要猜文件字节数，不要称为整景原文件。",json!(service.id),json!(source["coverageId"]),source["bounds"]),"context":null})).await.unwrap();
    let session = sent["selected"]["id"].as_str().unwrap().to_string();
    let prepared = completed(&connection).await;
    assert_eq!(
        prepared["selected"]["status"], "completed",
        "{}",
        prepared["selected"]["error"]
    );
    for tool in [
        "geod_wcs_connections",
        "geod_wcs_coverages",
        "geod_wcs_describe",
        "geod_wcs_prepare",
        "geod_wcs_project_plan",
    ] {
        assert!(
            calls(&prepared, tool) > 0,
            "Missing actual model call: {tool}"
        );
    }
    assert!(manager.list().await.is_empty() && manager.list_projects().await.is_empty());
    let original = review(&agent.snapshot().await.unwrap(), "project");
    assert_eq!(original["files"][0]["width"], 48);
    assert_eq!(original["files"][0]["height"], 48);
    assert_eq!(original["expectedBytes"], Value::Null);
    let old_id = original["planId"].as_str().unwrap();
    let old_hash = original["planHash"].as_str().unwrap();
    let draft = agent
        .plan_revision_draft(&session, old_id, old_hash)
        .await
        .unwrap();
    let edited = agent
        .revise_plan(
            &session,
            old_id,
            old_hash,
            agent_actions::PlanRevision::Project {
                item_ids: serde_json::from_value(draft["parameters"]["itemIds"].clone()).unwrap(),
                name: Some("WCS confirmed model subset".into()),
                bounds: Some(serde_json::from_value(source["bounds"].clone()).unwrap()),
                keep_polygon: false,
            },
        )
        .await
        .unwrap();
    let project_review = review(&edited, "project");
    let project_plan_id = project_review["planId"].as_str().unwrap();
    let project_hash = project_review["planHash"].as_str().unwrap();
    assert_ne!(project_hash, old_hash);
    assert!(agent
        .approve_plan(&session, old_id, old_hash)
        .await
        .is_err());
    let saved = agent
        .approve_plan(&session, project_plan_id, project_hash)
        .await
        .unwrap();
    assert!(manager.list().await.is_empty());
    assert_eq!(manager.list_projects().await.len(), 1);
    let project_id = project_review["project"]["id"].as_str().unwrap();
    agent.operation("send",json!({"sessionId":session,"text":format!("用户已在原生软件内确认工程 {}。调用 geod_project_get(id={}) 检查工程，再调用 geod_wcs_download_plan(projectId={}) 准备下载审核卡；不要执行下载。准确报告服务生成的区域栅格、真实像元尺寸、大小未知、需单独确认。",project_id,json!(project_id),json!(project_id)),"context":null})).await.unwrap();
    let planned = completed(&connection).await;
    assert_eq!(
        planned["selected"]["status"], "completed",
        "{}",
        planned["selected"]["error"]
    );
    for tool in ["geod_project_get", "geod_wcs_download_plan"] {
        assert!(calls(&planned, tool) > calls(&prepared, tool));
    }
    assert!(manager.list().await.is_empty());
    let download = review(&agent.snapshot().await.unwrap(), "download");
    let plan_id = download["planId"].as_str().unwrap();
    let plan_hash = download["planHash"].as_str().unwrap();
    assert!(agent
        .approve_plan(&session, plan_id, &"0".repeat(64))
        .await
        .is_err());
    let queued = agent
        .approve_plan(&session, plan_id, plan_hash)
        .await
        .unwrap();
    let current = queued["plans"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["planId"] == plan_id)
        .unwrap();
    let job_id = current["jobs"][0]["id"].as_str().unwrap().to_string();
    let job = manager.wait(&job_id).await.unwrap();
    assert_eq!(
        job.status,
        geod_runtime::JobStatus::Succeeded,
        "{:?}",
        job.error
    );
    let status = manager.agent_plan_status(&session, plan_id).await.unwrap();
    let bytes_before = tokio::fs::read(base.join("core/jobs.json")).await.unwrap();
    agent
        .approve_plan(&session, plan_id, plan_hash)
        .await
        .unwrap();
    assert_eq!(
        tokio::fs::read(base.join("core/jobs.json")).await.unwrap(),
        bytes_before
    );
    assert_eq!(manager.list().await.len(), 1);
    agent.operation("send",json!({"sessionId":session,"text":format!("用户已确认下载审核 {}，真实任务 {} 已返回。现在调用 geod_plan_status(planId={})、geod_wcs_inspect(id={}) 和 geod_wcs_pixel(id={},column=0,row=0)。报告实际状态、settled、原文件 SHA-256、尺寸和原始像元；区分服务声明单位与实际文件标签，不推测科学校准。不要重复下载或创建方案。",plan_id,job_id,json!(plan_id),json!(job_id),json!(job_id)),"context":null})).await.unwrap();
    let read = completed(&connection).await;
    assert_eq!(
        read["selected"]["status"], "completed",
        "{}",
        read["selected"]["error"]
    );
    for tool in ["geod_plan_status", "geod_wcs_inspect", "geod_wcs_pixel"] {
        assert!(calls(&read, tool) > calls(&planned, tool));
    }
    assert!(assistant_text(&read).contains(job.sha256.as_ref().unwrap()));
    let thread = read["selected"]["threadId"].clone();
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
    agent.operation("send",json!({"sessionId":session,"text":format!("软件重启，请重新调用 geod_plan_status(planId={})、geod_wcs_inspect(id={})、geod_wcs_pixel(id={},column=0,row=0)。只核查恢复后的相同任务和原文件，报告当前完整 SHA-256 和原始像元，不依赖旧回复，不执行下载。",json!(plan_id),json!(job_id),json!(job_id)),"context":null})).await.unwrap();
    let resumed = completed(&connection).await;
    assert_eq!(
        resumed["selected"]["status"], "completed",
        "{}",
        resumed["selected"]["error"]
    );
    assert_eq!(resumed["selected"]["id"], session);
    assert_eq!(resumed["selected"]["threadId"], thread);
    for tool in ["geod_plan_status", "geod_wcs_inspect", "geod_wcs_pixel"] {
        assert!(calls(&resumed, tool) > calls(&read, tool));
    }
    assert!(assistant_text(&resumed).contains(job.sha256.as_ref().unwrap()));
    agent
        .approve_plan(&session, plan_id, plan_hash)
        .await
        .unwrap();
    assert_eq!(reopened.list().await.len(), 1);
    assert_eq!(reopened.list_projects().await.len(), 1);
    assert_eq!(
        tokio::fs::read(base.join("core/jobs.json")).await.unwrap(),
        bytes_before
    );
    let reuse = reopened
        .agent_wcs_download_plan(
            &session,
            geod_runtime::wcs_projects::DownloadRequest {
                project_id: project_id.into(),
                selections: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(reuse["needsDownload"], false);
    let grid = reopened
        .wcs_plan(&job.wcs_source.as_ref().unwrap().plan_id)
        .await
        .unwrap();
    let receipt = json!({"schema":"geod-agent-wcs-model-acceptance/v1","status":"passed","synthetic":false,"modelUsed":true,"usedUserDesktop":false,"credentialVaultWritten":false,"published":false,"upstreamVendorVerified":false,"modelRoute":model,"source":source,"sessionId":session,"connection":service,"description":grid.description,"grid":grid,"originalProjectReview":original,"revisedProjectReview":project_review,"savedProject":saved,"downloadReview":download,"queued":current,"completed":status,"job":job,"restored":reopened.agent_plan_status(&session,plan_id).await.unwrap(),"reused":reuse,"persistedJobCount":1,"persistedVectorCount":0,"resumedSameConversation":true,"resumedSameModelThread":true,"humanRevisionRequiredSeparateConfirmation":true,"beforeProjectConfirmation":prepared,"beforeDownloadConfirmation":planned,"beforeRestart":read,"afterRestart":resumed});
    tokio::fs::write(
        base.join("native-acceptance.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .await
    .unwrap();
    tokio::fs::write(
        base.join("core/agent-wcs-acceptance.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .await
    .unwrap();
    agent.shutdown().await;
    reopened.shutdown().await.unwrap();
}
