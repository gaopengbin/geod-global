use super::*;

const AREA: [f64; 4] = [13.0, 52.0, 14.0, 53.0];
fn request(pins: Vec<Selection>, id: Option<String>) -> SaveProjectRequest {
    SaveProjectRequest {
        name: id.is_none().then(|| "Reviewed custom originals".into()),
        project_id: id,
        bounds: AREA,
        selections: pins,
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
async fn custom_project_requires_native_confirmation_and_preserves_receipt_on_restart() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let pin = stac::fixture_snapshot(manager.storage_root(), "https://example.com/original.tif");
    let plan = manager
        .agent_stac_project_plan(&session, request(vec![pin.clone()], None))
        .await
        .unwrap();
    assert!(manager.list_projects().await.is_empty());
    assert!(manager.list().await.is_empty());
    assert!(!plan.to_string().contains("https://"));
    assert_eq!(plan["files"][0]["snapshotId"], pin.snapshot_id);
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
    let repeated = approve(&manager, &session, &plan).await;
    assert_eq!(saved, repeated);
    assert_eq!(saved["status"], "submitted");
    assert_eq!(plan["project"]["saved"], false);
    assert_eq!(saved["project"]["saved"], true);
    assert!(manager.list().await.is_empty());
    let project = &manager.list_projects().await[0];
    assert_eq!(project.stac_items.len(), 1);
    assert_eq!(project.agent_approvals.len(), 1);
    drop(manager);
    let reopened = JobManager::open(dir.path()).await.unwrap();
    assert_eq!(
        reopened
            .agent_plan_status(&session, plan["planId"].as_str().unwrap())
            .await
            .unwrap(),
        saved
    );
}

#[tokio::test]
async fn custom_review_correction_pins_assets_not_ambiguous_catalog_item_ids() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let a = stac::fixture_snapshot(manager.storage_root(), "https://example.com/first.tif");
    let b = stac::fixture_snapshot(manager.storage_root(), "https://example.com/second.tif");
    let plan = manager
        .agent_stac_project_plan(&session, request(vec![a.clone(), b.clone()], None))
        .await
        .unwrap();
    assert_eq!(plan["files"][0]["itemId"], plan["files"][1]["itemId"]);
    let id = plan["planId"].as_str().unwrap();
    let hash = plan["planHash"].as_str().unwrap();
    let draft = manager
        .agent_plan_revision_draft(&session, id, hash)
        .await
        .unwrap();
    assert_ne!(
        draft["fields"]["items"][0]["id"],
        draft["fields"]["items"][1]["id"]
    );
    let edit = PlanRevision::Project {
        item_ids: vec![selection_id(&b)],
        name: Some("Second asset".into()),
        bounds: Some(AREA),
        keep_polygon: false,
    };
    let next = manager
        .revise_agent_plan(&session, id, hash, edit.clone())
        .await
        .unwrap();
    assert_eq!(
        manager
            .revise_agent_plan(&session, id, hash, edit)
            .await
            .unwrap(),
        next
    );
    assert!(manager
        .approve_agent_plan(&session, id, hash)
        .await
        .is_err());
    assert_eq!(
        manager.agent_plan_status(&session, id).await.unwrap()["status"],
        "superseded"
    );
    approve(&manager, &session, &next).await;
    assert_eq!(
        manager.list_projects().await[0].stac_items[0].snapshot_id,
        b.snapshot_id
    );
}

#[tokio::test]
async fn custom_append_keeps_existing_area_and_rejects_changed_scope() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let a = stac::fixture_snapshot(manager.storage_root(), "https://example.com/first.tif");
    let b = stac::fixture_snapshot(manager.storage_root(), "https://example.com/second.tif");
    let project = manager
        .save_stac_project(request(vec![a], None))
        .await
        .unwrap();
    let mut append = request(vec![b.clone()], Some(project.id.clone()));
    append.bounds = [0.0, 0.0, 1.0, 1.0];
    let plan = manager
        .agent_stac_project_plan(&session, append.clone())
        .await
        .unwrap();
    assert_eq!(plan["bounds"], json!(AREA));
    manager
        .rename_project(&project.id, "Changed independently")
        .await
        .unwrap();
    assert!(manager
        .approve_agent_plan(
            &session,
            plan["planId"].as_str().unwrap(),
            plan["planHash"].as_str().unwrap()
        )
        .await
        .is_err());
    assert_eq!(manager.list_projects().await[0].stac_items.len(), 1);
    let fresh = manager
        .agent_stac_project_plan(&session, append)
        .await
        .unwrap();
    approve(&manager, &session, &fresh).await;
    let project = &manager.list_projects().await[0];
    assert_eq!(project.stac_items.len(), 2);
    assert_eq!(project.bounds, AREA);
}

#[tokio::test]
async fn custom_project_persistence_failure_does_not_commit_in_memory() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let pin = stac::fixture_snapshot(manager.storage_root(), "https://example.com/original.tif");
    let plan = manager
        .agent_stac_project_plan(&session, request(vec![pin], None))
        .await
        .unwrap();
    let path = manager.storage_root().join("projects.json");
    if path.exists() {
        std::fs::remove_file(&path).unwrap();
    }
    std::fs::create_dir(&path).unwrap();
    assert!(manager
        .approve_agent_plan(
            &session,
            plan["planId"].as_str().unwrap(),
            plan["planHash"].as_str().unwrap()
        )
        .await
        .is_err());
    assert!(manager.list_projects().await.is_empty());
    assert!(manager.list().await.is_empty());
}

#[tokio::test]
async fn reuse_checks_original_bytes_and_does_not_accept_same_size_corruption() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let pin = stac::fixture_snapshot(manager.storage_root(), "https://example.com/original.tif");
    let project = manager
        .save_stac_project(request(vec![pin.clone()], None))
        .await
        .unwrap();
    let item = stac::resolve(manager.storage_root(), &pin).unwrap();
    let mut task = job(&item, &pin);
    let bytes = b"II*\0unit original bytes";
    let path = manager
        .storage_root()
        .join("assets")
        .join(format!("{}.tif", task.id));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, bytes).unwrap();
    task.status = JobStatus::Succeeded;
    task.sha256 = Some(digest(bytes));
    task.total_bytes = Some(bytes.len() as u64);
    task.bytes_downloaded = bytes.len() as u64;
    task.output_path = Some(path.to_string_lossy().into());
    {
        let mut store = manager.inner.store.lock().await;
        store.jobs.insert(task.id.clone(), task.clone());
        manager.persist(&store.jobs).await.unwrap();
    }
    let reused = manager
        .agent_stac_download_plan(
            &session,
            DownloadRequest {
                project_id: project.id,
                selections: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(reused["needsDownload"], false);
    assert_eq!(reused["jobs"][0]["id"], task.id);
    assert_eq!(manager.list().await.len(), 1);
    let mut changed = bytes.to_vec();
    changed[6] ^= 1;
    std::fs::write(&path, changed).unwrap();
    assert!(reusable(&manager, &pin).await.unwrap().is_none());
}

#[tokio::test]
async fn model_cannot_mutate_directly_or_substitute_urls_in_review_requests() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let pin = stac::fixture_snapshot(manager.storage_root(), "https://example.com/original.tif");
    assert!(call(manager.clone(),&session,"geod_stac_project_plan",json!({"name":"Injected","bounds":AREA,"selections":[pin],"href":"https://example.com/replaced.tif"}),None).await.is_err());
    for name in [
        "geod_stac_connect",
        "geod_stac_project_save",
        "geod_stac_download",
        "geod_stac_forget",
        "geod_plan_approve",
    ] {
        assert!(call(manager.clone(), &session, name, json!({}), None)
            .await
            .is_err());
    }
    assert!(manager.list_projects().await.is_empty());
    assert!(manager.list().await.is_empty());
}

#[tokio::test]
#[ignore = "Explicit opt-in actual public request in a fresh isolated QA store; no model or desktop operation"]
async fn live_custom_agent_review_download_and_restart() {
    let root = std::env::var("GEOD_AGENT_STAC_REVIEW_QA")
        .expect("fresh GEOD_AGENT_STAC_REVIEW_QA directory");
    assert!(!Path::new(&root).exists());
    let manager = JobManager::open(&root).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let connection=manager.connect_stac(stac::ConnectRequest {name:"Public GLO-90 review acceptance".into(),kind:"raster".into(),url:"https://copernicus-dem-90m.s3.eu-central-1.amazonaws.com/Copernicus_DSM_COG_30_N52_00_E013_00_DEM/Copernicus_DSM_COG_30_N52_00_E013_00_DEM.tif".into()}).await.unwrap();
    let snapshot = manager
        .stac_snapshot(&connection.snapshot_ids[0])
        .await
        .unwrap();
    let asset = snapshot.assets.iter().find(|a| a.eligible).unwrap();
    let pin = Selection {
        snapshot_id: snapshot.id.clone(),
        asset_key: asset.key.clone(),
    };
    let project_plan = call(
        manager.clone(),
        &session,
        "geod_stac_project_plan",
        serde_json::to_value(request(vec![pin.clone()], None)).unwrap(),
        None,
    )
    .await
    .unwrap();
    assert!(manager.list_projects().await.is_empty());
    assert!(manager.list().await.is_empty());
    let project = approve(&manager, &session, &project_plan).await;
    let project_id = project["project"]["id"].as_str().unwrap();
    let plan = call(
        manager.clone(),
        &session,
        "geod_stac_download_plan",
        json!({"projectId":project_id}),
        None,
    )
    .await
    .unwrap();
    assert!(manager.list().await.is_empty());
    let revised = manager
        .revise_agent_plan(
            &session,
            plan["planId"].as_str().unwrap(),
            plan["planHash"].as_str().unwrap(),
            PlanRevision::Download {
                item_ids: vec![selection_id(&pin)],
            },
        )
        .await
        .unwrap();
    assert!(manager
        .approve_agent_plan(
            &session,
            plan["planId"].as_str().unwrap(),
            plan["planHash"].as_str().unwrap()
        )
        .await
        .is_err());
    let submitted = approve(&manager, &session, &revised).await;
    assert_eq!(submitted["jobs"].as_array().unwrap().len(), 1);
    let id = submitted["jobs"][0]["id"].as_str().unwrap();
    let completed = tokio::time::timeout(Duration::from_secs(180), manager.wait(id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        completed.status,
        JobStatus::Succeeded,
        "{:?}",
        completed.error
    );
    assert_eq!(completed.bytes_downloaded, 3_765_647);
    let inspection = manager.inspect_stac_asset(id).await.unwrap();
    let pixel = manager.sample_stac_asset(id, 400, 600).await.unwrap();
    let repeated = approve(&manager, &session, &revised).await;
    assert_eq!(repeated["jobs"][0]["id"], id);
    assert_eq!(repeated["jobs"][0]["settled"], true);
    let reused = manager
        .agent_stac_download_plan(
            &session,
            DownloadRequest {
                project_id: project_id.into(),
                selections: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(reused["needsDownload"], false);

    // Change one byte without changing size in this isolated QA file. A fresh
    // review must repair the receipt, not repeatedly choose the old success.
    let original_path = Path::new(completed.output_path.as_ref().unwrap());
    let mut corrupted = std::fs::read(original_path).unwrap();
    corrupted[1024] ^= 1;
    std::fs::write(original_path, &corrupted).unwrap();
    assert!(manager.inspect_stac_asset(id).await.is_err());
    let replacement_plan = manager
        .agent_stac_download_plan(
            &session,
            DownloadRequest {
                project_id: project_id.into(),
                selections: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(replacement_plan["kind"], "download");

    // Inject a durable-write failure before approval. No repair record or new
    // task may survive in memory if the shared task-store commit fails.
    let blocked_write = manager.storage_root().join("jobs.json.tmp");
    std::fs::create_dir(&blocked_write).unwrap();
    assert!(manager
        .approve_agent_plan(
            &session,
            replacement_plan["planId"].as_str().unwrap(),
            replacement_plan["planHash"].as_str().unwrap(),
        )
        .await
        .is_err());
    assert_eq!(manager.list().await.len(), 1);
    assert_eq!(
        serde_json::to_value(manager.get(id).await.unwrap()).unwrap(),
        serde_json::to_value(&completed).unwrap()
    );
    assert!(manager.inner.store.lock().await.active.is_empty());
    std::fs::remove_dir(&blocked_write).unwrap();

    let replacement = approve(&manager, &session, &replacement_plan).await;
    let replacement_id = replacement["jobs"][0]["id"].as_str().unwrap();
    assert_ne!(replacement_id, id);
    let retired = manager.get(id).await.unwrap();
    assert_eq!(retired.status, JobStatus::Failed);
    assert!(retired.error.as_ref().unwrap().contains("SHA-256"));
    assert!(retired.sha256.is_none());
    assert!(retired.output_path.is_none());
    assert_eq!(std::fs::read(original_path).unwrap(), corrupted);
    let repaired = tokio::time::timeout(Duration::from_secs(180), manager.wait(replacement_id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        repaired.status,
        JobStatus::Succeeded,
        "{:?}",
        repaired.error
    );
    assert_eq!(repaired.bytes_downloaded, completed.bytes_downloaded);
    assert_eq!(repaired.sha256, completed.sha256);
    let repaired_inspection = manager.inspect_stac_asset(replacement_id).await.unwrap();
    let repaired_pixel = manager
        .sample_stac_asset(replacement_id, 400, 600)
        .await
        .unwrap();
    assert_eq!(repaired_pixel.values, pixel.values);
    assert_eq!(repaired_pixel.no_data, pixel.no_data);
    let app_reused = manager
        .download_stac_project(DownloadRequest {
            project_id: project_id.into(),
            selections: None,
        })
        .await
        .unwrap();
    assert_eq!(app_reused.jobs.len(), 1);
    assert_eq!(app_reused.jobs[0].id, replacement_id);
    assert_eq!(manager.list().await.len(), 2);
    let replacement_repeated = approve(&manager, &session, &replacement_plan).await;
    assert_eq!(replacement_repeated["jobs"][0]["id"], replacement_id);
    assert_eq!(replacement_repeated["jobs"][0]["settled"], true);
    manager.shutdown().await.unwrap();
    drop(manager);
    let reopened = JobManager::open(&root).await.unwrap();
    assert_eq!(
        reopened
            .agent_plan_status(&session, revised["planId"].as_str().unwrap())
            .await
            .unwrap()["jobs"][0]["id"],
        id
    );
    assert_eq!(reopened.get(id).await.unwrap().status, JobStatus::Failed);
    assert_eq!(
        reopened
            .agent_plan_status(&session, replacement_plan["planId"].as_str().unwrap())
            .await
            .unwrap()["jobs"][0]["id"],
        replacement_id
    );
    assert_eq!(
        serde_json::to_value(
            reopened
                .sample_stac_asset(replacement_id, 400, 600)
                .await
                .unwrap()
        )
        .unwrap(),
        serde_json::to_value(&repaired_pixel).unwrap()
    );
    let reopened_reused = reopened
        .agent_stac_download_plan(
            &session,
            DownloadRequest {
                project_id: project_id.into(),
                selections: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(reopened_reused["needsDownload"], false);
    assert_eq!(reopened_reused["jobs"][0]["id"], replacement_id);
    assert_eq!(reopened.list().await.len(), 2);
    let receipt = json!({"status":"passed","modelCalled":false,"userDesktopOperated":false,"sessionId":session,"snapshotId":pin.snapshot_id,"projectPlan":project_plan,"downloadPlan":revised,"firstTransfer":{"submitted":repeated,"sha256":completed.sha256,"bytes":completed.bytes_downloaded,"inspection":inspection,"pixel":pixel,"reused":reused},"cacheRepair":{"sameSizeCorruptionDetected":true,"failedCommitRolledBack":true,"damagedFilePreserved":true,"retiredJob":retired,"replacementPlan":replacement_plan,"submitted":replacement_repeated,"appReusedJobId":app_reused.jobs[0].id,"persistedJobCount":2},"sha256":repaired.sha256,"bytes":repaired.bytes_downloaded,"inspection":repaired_inspection,"pixel":repaired_pixel,"reusedAfterRestart":reopened_reused});
    reopened.shutdown().await.unwrap();
    std::fs::write(
        Path::new(&root).join("acceptance.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
}
