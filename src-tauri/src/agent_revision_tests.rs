//! Owned Node/native IPC acceptance. The transcript is explicitly seeded; this
//! checks human review, crash recovery and execution, not a model-generated turn.
use super::*;

struct NoVault;
impl registry::Vault for NoVault {
    fn read(&self, _: &str) -> Result<Option<Zeroizing<String>>, String> {
        panic!("This isolated review acceptance must not access credentials")
    }
    fn write(&self, _: &str, _: &str) -> Result<(), String> {
        panic!("This isolated review acceptance must not write credentials")
    }
    fn delete(&self, _: &str) -> Result<(), String> {
        panic!("This isolated review acceptance must not delete credentials")
    }
}

#[tokio::test]
#[ignore = "requires owned Agent runtime and an isolated copied real SCL store"]
async fn owned_runtime_revision_recovers_and_confirms_the_exact_native_review() {
    let directory = PathBuf::from(
        std::env::var("GEOD_AGENT_REVISION_TEST_DIR").expect("isolated acceptance directory"),
    )
    .canonicalize()
    .unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    assert!(directory.starts_with(root.join(".verification").canonicalize().unwrap()));
    assert!(!directory.join("native-acceptance.json").exists());
    let runtime = root.join(".agent-runtime/win32-x64");
    verify_runtime(&runtime).await.unwrap();
    let manager = JobManager::open(directory.join("core")).await.unwrap();
    let jobs = manager.list().await;
    assert_eq!(
        jobs.len(),
        1,
        "Only the independently verified copied SCL original is allowed"
    );
    let source = &jobs[0];
    assert_eq!(source.asset_key, "scl");
    assert_eq!(source.status, geod_runtime::JobStatus::Succeeded);
    let raster = manager.inspect_raster(&source.id).await.unwrap();
    let bounds = [
        raster.bounds[0],
        raster.bounds[3] - 800.0,
        raster.bounds[0] + 800.0,
        raster.bounds[3],
    ];
    let session = "d1234567-1234-4234-8234-123456789abc".to_string();
    let original = manager
        .agent_recipe_review_plan(
            &session,
            serde_json::from_value(json!({
                "schemaVersion":"geod-raster-recipe/v1", "name":"Original local review",
                "source":{"jobId":source.id,"sha256":source.sha256},
                "operation":{"type":"clip","crs":"source","bounds":bounds},
                "output":{"format":"GeoTIFF"}
            }))
            .unwrap(),
        )
        .await
        .unwrap();
    let id = original["planId"].as_str().unwrap();
    let hash = original["planHash"].as_str().unwrap();
    let home = directory.join("sessions");
    tokio::fs::create_dir_all(&home).await.unwrap();
    let seeded = json!({"version":1,"selectedId":session,"sessions":[{
        "id":session,"title":"Native revision IPC acceptance","status":"completed",
        "entries":[{"id":"e1234567-1234-4234-8234-123456789abc","type":"tool","name":"geod_recipe_review_plan",
            "status":"completed","references":[{"kind":"plan","id":id,"label":"Seeded native review"}]}]
    }]});
    tokio::fs::write(
        home.join("sessions.json"),
        serde_json::to_vec_pretty(&seeded).unwrap(),
    )
    .await
    .unwrap();
    let agent = DesktopAgent::open_with_vault(
        home.clone(),
        runtime.clone(),
        manager.clone(),
        Arc::new(NoVault),
    )
    .await
    .unwrap();
    assert_eq!(agent.snapshot().await.unwrap()["plans"][0]["planId"], id);
    assert!(agent
        .plan_revision_draft("f1234567-1234-4234-8234-123456789abc", id, hash)
        .await
        .is_err());
    assert!(agent
        .plan_revision_draft(&session, id, &"0".repeat(64))
        .await
        .is_err());
    let draft = agent.plan_revision_draft(&session, id, hash).await.unwrap();
    assert_eq!(draft["boundsCrs"], raster.crs);
    assert_eq!(manager.list().await.len(), 1);
    let corrected_bounds = [
        bounds[0] + 200.0,
        bounds[1] + 200.0,
        bounds[2] - 200.0,
        bounds[3] - 200.0,
    ];
    let revision = agent_actions::PlanRevision::Clip {
        bounds: corrected_bounds,
        name: "Corrected local review".into(),
        keep_polygon: false,
    };
    let revised = agent
        .revise_plan(&session, id, hash, revision.clone())
        .await
        .unwrap();
    let next = revised["plans"]
        .as_array()
        .unwrap()
        .iter()
        .find(|plan| plan["status"] == "pending")
        .unwrap()
        .clone();
    let next_id = next["planId"].as_str().unwrap();
    let next_hash = next["planHash"].as_str().unwrap();
    assert_ne!(next_id, id);
    assert_ne!(next_hash, hash);
    assert_eq!(
        manager.list().await.len(),
        1,
        "Saving a form must not execute processing"
    );
    assert!(agent.approve_plan(&session, id, hash).await.is_err());
    let repeated = agent
        .revise_plan(&session, id, hash, revision)
        .await
        .unwrap();
    assert_eq!(
        repeated["plans"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["planId"] == next_id)
            .count(),
        1
    );
    agent.shutdown().await;
    // Model a crash after the native supersession persisted but before the new
    // reference entered the transcript. The original native receipt is retained.
    tokio::fs::write(
        home.join("sessions.json"),
        serde_json::to_vec_pretty(&seeded).unwrap(),
    )
    .await
    .unwrap();
    let reopened = DesktopAgent::open_with_vault(home, runtime, manager.clone(), Arc::new(NoVault))
        .await
        .unwrap();
    let recovered = reopened.snapshot().await.unwrap();
    assert_eq!(
        recovered["plans"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["planId"] == next_id)
            .count(),
        1
    );
    assert_eq!(manager.list().await.len(), 1);
    let confirmed = reopened
        .approve_plan(&session, next_id, next_hash)
        .await
        .unwrap();
    let committed = confirmed["plans"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["planId"] == next_id)
        .unwrap();
    let output_id = committed["jobs"][0]["id"].as_str().unwrap();
    let output = manager.wait(output_id).await.unwrap();
    assert_eq!(output.status, geod_runtime::JobStatus::Succeeded);
    let inspected = manager.inspect_raster(output_id).await.unwrap();
    assert_eq!((inspected.width, inspected.height), (20, 20));
    assert_eq!(inspected.crs, raster.crs);
    assert_eq!(inspected.bounds, corrected_bounds);
    reopened
        .approve_plan(&session, next_id, next_hash)
        .await
        .unwrap();
    assert_eq!(manager.list().await.len(), 2);
    assert_eq!(
        manager.inspect_raster(&source.id).await.unwrap().sha256,
        raster.sha256
    );
    reopened.shutdown().await;
    manager.shutdown().await.unwrap();
    let receipt = json!({"schema":"geod-agent-native-revision-ipc/v1","status":"passed",
        "transcriptEvidence":"Seeded completed native plan reference, not a model turn",
        "usedUserDesktop":false,"modelCalls":0,"newProviderRequests":0,"credentialVaultAccessed":false,
        "originalPlanId":id,"replacementPlanId":next_id,"jobId":output_id,"sha256":inspected.sha256,
        "sourceSha256":raster.sha256,"crs":inspected.crs,"bounds":inspected.bounds,"width":inspected.width,"height":inspected.height,
        "draftCreatedNoJob":true,"revisionCreatedNoJob":true,"oldConfirmationRefused":true,
        "missingTranscriptReferenceRecovered":true,"sameConfirmationReusedJob":true,"sourceUnchanged":true});
    tokio::fs::write(
        directory.join("native-acceptance.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .await
    .unwrap();
}
