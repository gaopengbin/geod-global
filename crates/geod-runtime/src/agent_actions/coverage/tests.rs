use super::*;

const AREA: [f64; 4] = [13., 52., 15., 54.];
fn request(pin: &SourcePin) -> SaveProjectRequest {
    SaveProjectRequest {
        name: Some("Reviewed coverage".into()),
        project_id: None,
        bounds: AREA,
        selections: vec![pin.clone()],
    }
}
async fn approve(manager: &JobManager, session: &str, plan: &Value) -> Value {
    manager
        .approve_agent_plan(
            session,
            plan["planId"].as_str().unwrap(),
            plan["planHash"].as_str().unwrap(),
        )
        .await
        .unwrap()
}
#[tokio::test]
async fn coverage_review_requires_native_confirmation_and_recovers_project_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let pin = wcs::fixture_plan(manager.storage_root());
    let plan = manager
        .agent_wcs_project_plan(&session, request(&pin))
        .await
        .unwrap();
    assert!(manager.list().await.is_empty());
    assert!(manager.list_projects().await.is_empty());
    assert_eq!(plan["files"][0]["coveragePlanId"], pin.plan_id);
    assert_eq!(plan["files"][0]["width"], 2);
    assert!(manager
        .approve_agent_plan(&session, plan["planId"].as_str().unwrap(), &"0".repeat(64))
        .await
        .is_err());
    assert!(manager
        .approve_agent_plan(
            &Uuid::new_v4().to_string(),
            plan["planId"].as_str().unwrap(),
            plan["planHash"].as_str().unwrap()
        )
        .await
        .is_err());
    let saved = approve(&manager, &session, &plan).await;
    assert_eq!(plan["project"]["saved"], false);
    assert_eq!(saved["project"]["saved"], true);
    assert_eq!(saved, approve(&manager, &session, &plan).await);
    assert!(manager.list().await.is_empty());
    assert_eq!(manager.list_projects().await[0].agent_approvals.len(), 1);
    drop(manager);
    let reopened = JobManager::open(dir.path()).await.unwrap();
    assert_eq!(
        saved,
        reopened
            .agent_plan_status(&session, plan["planId"].as_str().unwrap())
            .await
            .unwrap()
    );
}
#[tokio::test]
async fn coverage_area_correction_rederives_grid_plan_and_supersedes_old_review() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let pin = wcs::fixture_plan(manager.storage_root());
    let plan = manager
        .agent_wcs_project_plan(&session, request(&pin))
        .await
        .unwrap();
    let id = plan["planId"].as_str().unwrap();
    let hash = plan["planHash"].as_str().unwrap();
    let params = PlanRevision::Project {
        item_ids: vec![pin.plan_id.clone()],
        name: Some("Corrected subset".into()),
        bounds: Some([13., 53., 14., 54.]),
        keep_polygon: false,
    };
    let revised = manager
        .revise_agent_plan(&session, id, hash, params.clone())
        .await
        .unwrap();
    assert_eq!(
        revised,
        manager
            .revise_agent_plan(&session, id, hash, params)
            .await
            .unwrap()
    );
    assert_ne!(revised["planHash"], plan["planHash"]);
    assert_ne!(revised["files"][0]["coveragePlanId"], pin.plan_id);
    assert_eq!(revised["files"][0]["width"], 1);
    assert_eq!(revised["files"][0]["height"], 1);
    assert_eq!(
        revised["files"][0]["requestedBounds"],
        json!([13., 53., 14., 54.])
    );
    assert!(manager
        .approve_agent_plan(&session, id, hash)
        .await
        .is_err());
    approve(&manager, &session, &revised).await;
    assert_eq!(
        manager.list_projects().await[0].bounds,
        [13., 53., 14., 54.]
    );
}
#[tokio::test]
async fn coverage_queue_receipt_rolls_back_with_failed_durable_commit_and_replays_once() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let pin = wcs::fixture_plan(manager.storage_root());
    let project = approve(
        &manager,
        &session,
        &manager
            .agent_wcs_project_plan(&session, request(&pin))
            .await
            .unwrap(),
    )
    .await;
    let download = manager
        .agent_wcs_download_plan(
            &session,
            DownloadRequest {
                project_id: project["project"]["id"].as_str().unwrap().into(),
                selections: None,
            },
        )
        .await
        .unwrap();
    assert!(manager.list().await.is_empty());
    let path = manager.storage_root().join("jobs.json");
    assert_eq!(download["project"]["saved"], true);
    assert_eq!(download["project"]["committed"], false);
    assert_eq!(download["status"], "pending");
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(manager
        .approve_agent_plan(
            &session,
            download["planId"].as_str().unwrap(),
            download["planHash"].as_str().unwrap()
        )
        .await
        .is_err());
    assert!(manager.list().await.is_empty());
    assert!(manager.inner.store.lock().await.active.is_empty());
    std::fs::remove_dir(&path).unwrap();
    let submitted = approve(&manager, &session, &download).await;
    let id = submitted["jobs"][0]["id"].as_str().unwrap();
    manager.wait(id).await.unwrap(); // Synthetic connection is absent: no upstream request.
    let repeated = approve(&manager, &session, &download).await;
    assert_eq!(repeated["jobs"].as_array().unwrap().len(), 1);
    assert_eq!(repeated["jobs"][0]["id"], id);
    let tasks = manager.list().await;
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].wcs_source, Some(pin));
    assert_eq!(
        tasks[0].agent_approval.as_ref().unwrap().plan_hash,
        download["planHash"].as_str().unwrap()
    );
    drop(manager);
    let restarted = JobManager::open(dir.path()).await.unwrap();
    assert_eq!(
        approve(&restarted, &session, &download).await["jobs"][0]["id"],
        id
    );
    assert_eq!(restarted.list().await.len(), 1);
}
#[tokio::test]
async fn coverage_download_rejects_project_drift_and_nonmember_or_duplicate_selections() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let pin = wcs::fixture_plan(manager.storage_root());
    let mut duplicate = request(&pin);
    duplicate.selections.push(pin.clone());
    assert!(manager
        .agent_wcs_project_plan(&session, duplicate)
        .await
        .is_err());
    let mut wrong = request(&pin);
    wrong.bounds = [13., 53., 14., 54.];
    assert!(manager
        .agent_wcs_project_plan(&session, wrong)
        .await
        .is_err());
    let saved = approve(
        &manager,
        &session,
        &manager
            .agent_wcs_project_plan(&session, request(&pin))
            .await
            .unwrap(),
    )
    .await;
    let id = saved["project"]["id"].as_str().unwrap();
    assert!(manager
        .agent_wcs_download_plan(
            &session,
            DownloadRequest {
                project_id: id.into(),
                selections: Some(vec![SourcePin {
                    plan_id: "f".repeat(64)
                }])
            }
        )
        .await
        .is_err());
    let download = manager
        .agent_wcs_download_plan(
            &session,
            DownloadRequest {
                project_id: id.into(),
                selections: None,
            },
        )
        .await
        .unwrap();
    manager
        .rename_project(id, "Changed outside review")
        .await
        .unwrap();
    assert!(manager
        .approve_agent_plan(
            &session,
            download["planId"].as_str().unwrap(),
            download["planHash"].as_str().unwrap()
        )
        .await
        .is_err());
    assert!(manager.list().await.is_empty());
}
#[tokio::test]
async fn coverage_metadata_adapter_cannot_inherit_write_enabled_mcp_router() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    for name in [
        "geod_wcs_connect",
        "geod_wcs_download",
        "geod_wcs_project_save",
        "geod_wcs_forget",
    ] {
        assert!(
            crate::mcp::agent_wcs_metadata(manager.clone(), name, json!({}))
                .await
                .is_err()
        );
        assert!(
            crate::mcp::agent_read_call(manager.clone(), name, json!({}))
                .await
                .is_err()
        );
    }
    assert!(crate::mcp::agent_wcs_metadata(manager.clone(),"geod_wcs_prepare",json!({"request":{"descriptionId":"a".repeat(64),"bounds":AREA,"url":"https://example.com"}})).await.is_err());
    assert!(manager.list().await.is_empty());
    assert!(manager.list_projects().await.is_empty());
}

#[tokio::test]
async fn coverage_append_preserves_mixed_project_and_its_saved_area() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let pin = wcs::fixture_plan(manager.storage_root());
    let stac =
        crate::stac::fixture_snapshot(manager.storage_root(), "https://example.com/original.tif");
    let previous = manager
        .save_stac_project(crate::stac_projects::SaveProjectRequest {
            project_id: None,
            name: Some("Mixed sources".into()),
            bounds: AREA,
            selections: vec![stac],
        })
        .await
        .unwrap();
    let mut append = request(&pin);
    append.project_id = Some(previous.id.clone());
    append.name = None;
    append.bounds = [13., 53., 14., 54.];
    let plan = manager
        .agent_wcs_project_plan(&session, append.clone())
        .await
        .unwrap();
    assert_eq!(plan["bounds"], json!(AREA));
    assert_eq!(plan["project"]["mode"], "append");
    approve(&manager, &session, &plan).await;
    let current = &manager.list_projects().await[0];
    assert_eq!(current.bounds, previous.bounds);
    assert_eq!(current.stac_items, previous.stac_items);
    assert_eq!(
        serde_json::to_value(&current.scenes).unwrap(),
        serde_json::to_value(&previous.scenes).unwrap()
    );
    assert_eq!(current.name, previous.name);
    assert_eq!(current.wcs_items.len(), 1);
    assert_eq!(current.agent_approvals.len(), 1);
    assert!(manager
        .agent_wcs_project_plan(&session, append)
        .await
        .is_err());
    assert!(manager.list().await.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Explicit isolated public WCS acceptance only; performs bounded real network requests"]
async fn public_wcs_review_download_and_restart() {
    eprintln!("WCS acceptance: opening isolated store");
    let root =
        PathBuf::from(std::env::var("GEOD_AGENT_WCS_QA_ROOT").expect("explicit isolated root"));
    assert!(!root.exists(), "Use a fresh acceptance directory");
    let source: Value = serde_json::from_str(
        &std::env::var("GEOD_AGENT_WCS_SOURCE_QA").expect("explicit public source and bounds"),
    )
    .unwrap();
    let manager = JobManager::open(&root).await.unwrap();
    let session = Uuid::new_v4().to_string();
    manager
        .save_proxy_settings(crate::ProxySettings {
            mode: crate::proxy::ProxyMode::Direct,
            url: None,
        })
        .await
        .unwrap();
    eprintln!("WCS acceptance: reading public service metadata");
    let connection = Box::pin(manager.connect_wcs(wcs::ConnectRequest {
        name: source["name"].as_str().unwrap().into(),
        url: source["url"].as_str().unwrap().into(),
    }))
    .await
    .unwrap();
    eprintln!("WCS acceptance: Agent metadata and native grid plan");
    let description = Box::pin(call(
        manager.clone(),
        &session,
        "geod_wcs_describe",
        json!({"request":{"connectionId":connection.id,"coverageId":source["coverageId"]}}),
        None,
    ))
    .await
    .unwrap();
    let grid = call(
        manager.clone(),
        &session,
        "geod_wcs_prepare",
        json!({"request":{"descriptionId":description["id"],"bounds":source["bounds"]}}),
        None,
    )
    .await
    .unwrap();
    let project=call(manager.clone(),&session,"geod_wcs_project_plan",json!({"name":"WCS Agent public acceptance","bounds":source["bounds"],"selections":[{"planId":grid["id"]}]}),None).await.unwrap();
    assert!(manager.list_projects().await.is_empty());
    assert!(manager.list().await.is_empty());
    let revised = manager
        .revise_agent_plan(
            &session,
            project["planId"].as_str().unwrap(),
            project["planHash"].as_str().unwrap(),
            PlanRevision::Project {
                item_ids: vec![grid["id"].as_str().unwrap().into()],
                name: Some("WCS confirmed native subset".into()),
                bounds: Some(serde_json::from_value(source["bounds"].clone()).unwrap()),
                keep_polygon: false,
            },
        )
        .await
        .unwrap();
    assert!(manager
        .approve_agent_plan(
            &session,
            project["planId"].as_str().unwrap(),
            project["planHash"].as_str().unwrap()
        )
        .await
        .is_err());
    let saved = approve(&manager, &session, &revised).await;
    let download = call(
        manager.clone(),
        &session,
        "geod_wcs_download_plan",
        json!({"projectId":saved["project"]["id"]}),
        None,
    )
    .await
    .unwrap();
    assert!(manager.list().await.is_empty());
    let queued = approve(&manager, &session, &download).await;
    let id = queued["jobs"][0]["id"].as_str().unwrap();
    let job = manager.wait(id).await.unwrap();
    assert_eq!(job.status, JobStatus::Succeeded, "{:?}", job.error);
    let inspected = call(
        manager.clone(),
        &session,
        "geod_wcs_inspect",
        json!({"id":id}),
        None,
    )
    .await
    .unwrap();
    let pixel = call(
        manager.clone(),
        &session,
        "geod_wcs_pixel",
        json!({"id":id,"column":0,"row":0}),
        None,
    )
    .await
    .unwrap();
    let status = manager
        .agent_plan_status(&session, download["planId"].as_str().unwrap())
        .await
        .unwrap();
    assert_eq!(status["jobs"][0]["settled"], true);
    assert_eq!(status["jobs"][0]["status"], "succeeded");
    assert_eq!(approve(&manager, &session, &download).await, status);
    assert_eq!(manager.list().await.len(), 1);
    assert_eq!(manager.list_projects().await.len(), 1);
    manager.shutdown().await.unwrap();
    drop(manager);
    let restarted = JobManager::open(&root).await.unwrap();
    let restored = restarted
        .agent_plan_status(&session, download["planId"].as_str().unwrap())
        .await
        .unwrap();
    assert_eq!(restored, status);
    assert_eq!(approve(&restarted, &session, &download).await, status);
    let reuse = restarted
        .agent_wcs_download_plan(
            &session,
            DownloadRequest {
                project_id: saved["project"]["id"].as_str().unwrap().into(),
                selections: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(reuse["needsDownload"], false);
    assert_eq!(restarted.list().await.len(), 1);
    let record = json!({"synthetic":false,"modelUsed":false,"usedUserDesktop":false,"sessionId":session,"source":source,"connection":connection,"description":description,"grid":grid,"originalProjectReview":project,"revisedProjectReview":revised,"savedProject":saved,"downloadReview":download,"queued":queued,"completed":status,"inspect":inspected,"pixel":pixel,"job":job,"restored":restored,"reused":reuse});
    std::fs::write(
        root.join("agent-wcs-acceptance.json"),
        serde_json::to_vec_pretty(&record).unwrap(),
    )
    .unwrap();
    restarted.shutdown().await.unwrap();
}
