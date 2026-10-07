use super::*;
use crate::agent_actions::tests::{
    confirmed_project, crop_plan, project_search_fixture, source_for_project,
};

#[tokio::test]
async fn crop_form_repreflights_same_source_and_retires_original_across_restart() {
    let directory = tempfile::tempdir().unwrap();
    let manager = JobManager::open(directory.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let original = crop_plan(&manager, &session).await;
    let id = original["planId"].as_str().unwrap();
    let hash = original["planHash"].as_str().unwrap();
    let draft = manager
        .agent_plan_revision_draft(&session, id, hash)
        .await
        .unwrap();
    assert_eq!(draft["boundsCrs"], "EPSG:32610");
    assert_eq!(draft["parameters"]["bounds"][0], 500000.0);
    assert!(!draft.to_string().contains("sha256"));
    let edits = PlanRevision::Clip {
        bounds: [500100.0, 4199100.0, 500500.0, 4199500.0],
        name: "Revised crop".into(),
        keep_polygon: false,
    };
    let (a, b) = tokio::join!(
        manager.revise_agent_plan(&session, id, hash, edits.clone()),
        manager.revise_agent_plan(&session, id, hash, edits.clone())
    );
    let revised = a.unwrap();
    assert_eq!(revised, b.unwrap());
    assert_ne!(revised["planId"], original["planId"]);
    assert_ne!(revised["planHash"], original["planHash"]);
    // The source fixture has 20 m cells: a 400 m window is 20 by 20 pixels.
    assert_eq!(revised["files"][0]["width"], 20);
    assert_eq!(revised["files"][0]["height"], 20);
    assert_eq!(manager.list().await.len(), 1);
    assert_eq!(
        manager.agent_plan_status(&session, id).await.unwrap()["status"],
        "superseded"
    );
    assert!(manager
        .approve_agent_plan(&session, id, hash)
        .await
        .is_err());
    manager.shutdown().await.unwrap();
    drop(manager);
    let manager = JobManager::open(directory.path()).await.unwrap();
    assert_eq!(
        manager
            .revise_agent_plan(&session, id, hash, edits)
            .await
            .unwrap()["planId"],
        revised["planId"]
    );
    let submitted = manager
        .approve_agent_plan(
            &session,
            revised["planId"].as_str().unwrap(),
            revised["planHash"].as_str().unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        manager
            .wait(submitted["jobs"][0]["id"].as_str().unwrap())
            .await
            .unwrap()
            .status,
        JobStatus::Succeeded
    );
    assert_eq!(manager.list().await.len(), 2);
    assert!(manager
        .agent_plan_revision_draft(
            &session,
            revised["planId"].as_str().unwrap(),
            revised["planHash"].as_str().unwrap()
        )
        .await
        .is_err());
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn form_rejects_foreign_hash_invalid_grid_and_fabricated_polygon_without_retiring_review() {
    let directory = tempfile::tempdir().unwrap();
    let manager = JobManager::open(directory.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let original = crop_plan(&manager, &session).await;
    let id = original["planId"].as_str().unwrap();
    let hash = original["planHash"].as_str().unwrap();
    let edits = PlanRevision::Clip {
        bounds: [500000.0, 4199000.0, 501000.0, 4200000.0],
        name: "Edited".into(),
        keep_polygon: false,
    };
    assert!(manager
        .revise_agent_plan(&Uuid::new_v4().to_string(), id, hash, edits.clone())
        .await
        .is_err());
    assert!(manager
        .revise_agent_plan(&session, id, &"0".repeat(64), edits.clone())
        .await
        .is_err());
    assert!(manager
        .revise_agent_plan(
            &session,
            id,
            hash,
            PlanRevision::Clip {
                bounds: [1.0, 0.0, 0.0, 1.0],
                name: "Invalid".into(),
                keep_polygon: false
            }
        )
        .await
        .is_err());
    assert!(manager
        .revise_agent_plan(
            &session,
            id,
            hash,
            PlanRevision::Clip {
                bounds: [500000.0, 4199000.0, 501000.0, 4200000.0],
                name: "Invalid".into(),
                keep_polygon: true
            }
        )
        .await
        .is_err());
    assert!(manager
        .revise_agent_plan(
            &session,
            id,
            hash,
            PlanRevision::Download {
                item_ids: vec!["foreign".into()]
            }
        )
        .await
        .is_err());
    assert_eq!(
        manager.agent_plan_status(&session, id).await.unwrap()["status"],
        "pending"
    );
    let mut expired: Plan = read_record(&manager.inner.root, "plans", id).await.unwrap();
    expired.expires_at = (Utc::now() - chrono::Duration::minutes(1)).to_rfc3339();
    write_record(&manager.inner.root, "plans", id, &expired)
        .await
        .unwrap();
    assert_eq!(
        manager
            .revise_agent_plan(&session, id, hash, edits)
            .await
            .unwrap()["status"],
        "pending"
    );
    assert_eq!(manager.list().await.len(), 1);
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn project_form_selects_native_scenes_and_preserves_pinned_append_scope() {
    let directory = tempfile::tempdir().unwrap();
    let manager = JobManager::open(directory.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let receipt = project_search_fixture(&manager, &session).await;
    let original = manager
        .agent_project_plan(
            &session,
            &receipt.id,
            receipt
                .candidates
                .iter()
                .map(|c| c.item_id.clone())
                .collect(),
            Some("Original".into()),
            None,
        )
        .await
        .unwrap();
    let id = original["planId"].as_str().unwrap();
    let hash = original["planHash"].as_str().unwrap();
    let mut form = manager
        .agent_plan_revision_draft(&session, id, hash)
        .await
        .unwrap()["parameters"]
        .clone();
    form["itemIds"] = json!([receipt.candidates[0].item_id]);
    form["name"] = json!("Corrected");
    let revised = manager
        .revise_agent_plan(&session, id, hash, serde_json::from_value(form).unwrap())
        .await
        .unwrap();
    assert!(manager.list_projects().await.is_empty());
    assert_eq!(revised["files"].as_array().unwrap().len(), 1);
    let submitted = manager
        .approve_agent_plan(
            &session,
            revised["planId"].as_str().unwrap(),
            revised["planHash"].as_str().unwrap(),
        )
        .await
        .unwrap();
    let project_id = submitted["project"]["id"].as_str().unwrap();
    let append = manager
        .agent_project_plan(
            &session,
            &receipt.id,
            vec![receipt.candidates[1].item_id.clone()],
            None,
            Some(project_id.into()),
        )
        .await
        .unwrap();
    let id = append["planId"].as_str().unwrap();
    let hash = append["planHash"].as_str().unwrap();
    let draft = manager
        .agent_plan_revision_draft(&session, id, hash)
        .await
        .unwrap();
    assert_eq!(draft["fields"]["name"], false);
    assert_eq!(
        draft["fields"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["locked"] == true)
            .count(),
        1
    );
    let mut invalid = draft["parameters"].clone();
    invalid["itemIds"] = json!([receipt.candidates[1].item_id]);
    assert!(manager
        .revise_agent_plan(&session, id, hash, serde_json::from_value(invalid).unwrap())
        .await
        .is_err());
    let fresh = manager
        .revise_agent_plan(
            &session,
            id,
            hash,
            serde_json::from_value(draft["parameters"].clone()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fresh["files"].as_array().unwrap().len(), 2);
    assert_eq!(manager.list_projects().await[0].scenes.len(), 1);
    assert!(manager.list().await.is_empty());
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn mosaic_form_rechecks_sources_and_cannot_substitute_quality() {
    let directory = tempfile::tempdir().unwrap();
    let manager = JobManager::open(directory.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let (project, _) = confirmed_project(&manager, &session).await;
    let project_id = project["project"]["id"].as_str().unwrap();
    source_for_project(&manager, project_id).await;
    let plan = manager
        .agent_project_mosaic_plan(&session, project_id, "scl")
        .await
        .unwrap();
    let id = plan["planId"].as_str().unwrap();
    let hash = plan["planHash"].as_str().unwrap();
    let draft = manager
        .agent_plan_revision_draft(&session, id, hash)
        .await
        .unwrap();
    assert_eq!(draft["parameters"]["assetKey"], "scl");
    assert!(manager
        .revise_agent_plan(
            &session,
            id,
            hash,
            PlanRevision::Mosaic {
                asset_key: "scl".into(),
                quality_policy: Some(crate::mosaic::vegetation::Policy::Good)
            }
        )
        .await
        .is_err());
    let fresh = manager
        .revise_agent_plan(
            &session,
            id,
            hash,
            serde_json::from_value(draft["parameters"].clone()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fresh["status"], "pending");
    assert_eq!(manager.list().await.len(), 1);
    let stored: Plan = read_record(&manager.inner.root, "plans", id).await.unwrap();
    let mut changed = stored.action.clone();
    if let Action::Mosaic { project_hash, .. } = &mut changed {
        *project_hash = "0".repeat(64);
    }
    assert!(preserve_project_scope(&stored.action, &changed).is_err());
    manager.shutdown().await.unwrap();
}

#[test]
fn forms_reject_injected_paths_unknown_fields_and_outside_selections() {
    assert!(serde_json::from_value::<PlanRevision>(
        json!({"kind":"download","itemIds":["a"],"href":"https://example.com"})
    )
    .is_err());
    assert!(selected(vec!["x".into()], vec!["a".into()].into_iter()).is_err());
    assert!(selected(vec!["a".into(), "a".into()], vec!["a".into()].into_iter()).is_err());
}

#[tokio::test]
async fn replacement_rejects_same_hash_record_under_another_internal_id() {
    let directory = tempfile::tempdir().unwrap();
    let manager = JobManager::open(directory.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let original = crop_plan(&manager, &session).await;
    let id = original["planId"].as_str().unwrap();
    let hash = original["planHash"].as_str().unwrap();
    let draft = manager
        .agent_plan_revision_draft(&session, id, hash)
        .await
        .unwrap();
    let next = manager
        .revise_agent_plan(
            &session,
            id,
            hash,
            serde_json::from_value(draft["parameters"].clone()).unwrap(),
        )
        .await
        .unwrap();
    // Effective hashes deliberately omit random IDs. File identity must still
    // reject a same-hash record copied under the replacement's filename.
    assert_eq!(next["planHash"], original["planHash"]);
    let record: Plan = read_record(&manager.inner.root, "plans", id).await.unwrap();
    write_record(
        &manager.inner.root,
        "plans",
        next["planId"].as_str().unwrap(),
        &record,
    )
    .await
    .unwrap();
    assert!(manager.agent_plan_status(&session, id).await.is_err());
    assert!(manager
        .agent_plan_status(&session, next["planId"].as_str().unwrap())
        .await
        .is_err());
    assert!(manager
        .approve_agent_plan(&session, id, hash)
        .await
        .is_err());
    assert_eq!(manager.list().await.len(), 1);
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn concurrent_approval_and_correction_admit_only_one_version() {
    let directory = tempfile::tempdir().unwrap();
    let manager = JobManager::open(directory.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let original = crop_plan(&manager, &session).await;
    let id = original["planId"].as_str().unwrap();
    let hash = original["planHash"].as_str().unwrap();
    let edits = PlanRevision::Clip {
        bounds: [500100.0, 4199100.0, 500500.0, 4199500.0],
        name: "Corrected".into(),
        keep_polygon: false,
    };
    let (approval, revision) = tokio::join!(
        manager.approve_agent_plan(&session, id, hash),
        manager.revise_agent_plan(&session, id, hash, edits)
    );
    assert_ne!(approval.is_ok(), revision.is_ok());
    if let Ok(approved) = approval {
        manager
            .wait(approved["jobs"][0]["id"].as_str().unwrap())
            .await
            .unwrap();
        assert_eq!(manager.list().await.len(), 2);
    } else {
        assert_eq!(manager.list().await.len(), 1);
    }
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn polygon_form_preserves_local_vertices_or_explicitly_removes_the_mask() {
    let directory = tempfile::tempdir().unwrap();
    let manager = JobManager::open(directory.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let source = crate::agent_actions::tests::local_source(&manager).await;
    let geometry = json!({"type":"Polygon","coordinates":[[[-123.0,37.947],[-122.999,37.947],[-122.999,37.948],[-123.0,37.948],[-123.0,37.947]]]});
    let recipe=serde_json::from_value(json!({"schemaVersion":"geod-raster-recipe/v2","name":"Polygon","source":{"jobId":source.id,"sha256":source.sha256},"operation":{"type":"clip","crs":"EPSG:4326","bounds":[-123.0,37.947,-122.999,37.948],"geometry":geometry},"output":{"format":"GeoTIFF"}})).unwrap();
    let original = manager
        .agent_recipe_review_plan(&session, recipe)
        .await
        .unwrap();
    let id = original["planId"].as_str().unwrap();
    let hash = original["planHash"].as_str().unwrap();
    let draft = manager
        .agent_plan_revision_draft(&session, id, hash)
        .await
        .unwrap();
    assert_eq!(draft["fields"]["polygon"], true);
    assert!(!draft.to_string().contains("coordinates"));
    let edits = PlanRevision::Clip {
        bounds: [-123.0, 37.947, -122.999, 37.9478],
        name: "Retained mask".into(),
        keep_polygon: true,
    };
    let masked = manager
        .revise_agent_plan(&session, id, hash, edits)
        .await
        .unwrap();
    assert_eq!(masked["polygon"]["sha256"], original["polygon"]["sha256"]);
    let rectangle = manager
        .revise_agent_plan(
            &session,
            masked["planId"].as_str().unwrap(),
            masked["planHash"].as_str().unwrap(),
            PlanRevision::Clip {
                bounds: [-123.0, 37.947, -122.999, 37.9478],
                name: "Rectangle".into(),
                keep_polygon: false,
            },
        )
        .await
        .unwrap();
    assert!(rectangle["polygon"].is_null());
    assert_eq!(manager.list().await.len(), 1);
    // A source modified after review must fail preflight and leave the card pending.
    std::fs::write(source.output_path.as_ref().unwrap(), b"changed source").unwrap();
    let draft = manager
        .agent_plan_revision_draft(
            &session,
            rectangle["planId"].as_str().unwrap(),
            rectangle["planHash"].as_str().unwrap(),
        )
        .await
        .unwrap();
    assert!(manager
        .revise_agent_plan(
            &session,
            rectangle["planId"].as_str().unwrap(),
            rectangle["planHash"].as_str().unwrap(),
            serde_json::from_value(draft["parameters"].clone()).unwrap()
        )
        .await
        .is_err());
    assert_eq!(
        manager
            .agent_plan_status(&session, rectangle["planId"].as_str().unwrap())
            .await
            .unwrap()["status"],
        "pending"
    );
    manager.shutdown().await.unwrap();
}
