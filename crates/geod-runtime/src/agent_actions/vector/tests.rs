//! Explicit synthetic native fixtures. Live model/public-service proof is separate.
use super::*;
use crate::features::{Collection, FeatureService, PageReceipt, Provenance, Snapshot};

async fn setup() -> (tempfile::TempDir, JobManager, String, String) {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let id = Uuid::new_v4().to_string();
    let session = Uuid::new_v4().to_string();
    let service = FeatureService {
        id: id.clone(),
        name: "Synthetic review fixture".into(),
        url: "https://example.com/api".into(),
        title: "Fixture".into(),
        connected_at: now(),
        arcgis: None,
        overpass: None,
        wfs: None,
        collections: vec![Collection {
            id: "lakes".into(),
            title: "Lakes".into(),
            description: "Explicit synthetic collection".into(),
            items_url: "https://example.com/api/collections/lakes/items?f=json".into(),
            license_links: vec![],
            arcgis: None,
            wfs: None,
        }],
    };
    manager
        .inner
        .feature_services
        .lock()
        .await
        .insert(id.clone(), service);
    tokio::fs::write(
        manager.inner.root.join("feature-services.json"),
        serde_json::to_vec(&*manager.inner.feature_services.lock().await).unwrap(),
    )
    .await
    .unwrap();
    (dir, manager, id, session)
}
async fn review(manager: &JobManager, service: &str, session: &str) -> Value {
    call(manager.clone(), session, "geod_vector_extract_plan", json!({"serviceId":service,"collectionId":"lakes","bounds":[0,0,1,1],"name":"Reviewed lakes","pageSize":2}), None).await.unwrap()
}
async fn native_plan(manager: &JobManager, session: &str, value: &Value) -> Plan {
    let plan: Plan = read_record(
        &manager.inner.root,
        "plans",
        value["planId"].as_str().unwrap(),
    )
    .await
    .unwrap();
    plan.validate(session).unwrap();
    plan
}
fn snapshot(plan: &Plan) -> Snapshot {
    let Action::Vector { scope } = &plan.action else {
        panic!()
    };
    let feature = json!({"type":"Feature","id":"fixture-1","properties":{"label":"Lakes","height":3},"geometry":{"type":"Point","coordinates":[0.2,0.3,4]}});
    let page = json!({"type":"FeatureCollection","numberMatched":1,"numberReturned":1,"features":[feature.clone()]});
    let bytes = serde_json::to_vec(&page).unwrap();
    let source = Provenance {
        service_url: scope.service_url.clone(),
        service_name: scope.service_name.clone(),
        collection_id: scope.request.collection_id.clone(),
        collection_title: scope.collection_title.clone(),
        license_links: vec![],
        requested_bounds: scope.request.bounds,
        area_geometry: scope.request.area_geometry.clone(),
        requested_at: now(),
        number_matched: Some(1),
        feature_count: 1,
        selection: "bbox-full-features".into(),
        arcgis: None,
        wfs: None,
        pages: vec![PageReceipt {
            url: "https://example.com/api/collections/lakes/items?f=json&bbox=0,0,1,1&limit=2"
                .into(),
            sha256: digest(&bytes),
            bytes: bytes.len(),
            returned: 1,
            parameters: None,
        }],
    };
    Snapshot::Features(
        crate::vector::ImportVectorRequest {
            name: scope.name.clone(),
            text: serde_json::to_string(
                &json!({"type":"FeatureCollection","features":[feature],"geodSource":source}),
            )
            .unwrap(),
        },
        source,
    )
}
async fn publish_fixture(manager: &JobManager, plan: &Plan) -> crate::vector::VectorAsset {
    let Action::Vector { scope } = &plan.action else {
        panic!()
    };
    manager
        .import_feature_snapshot(
            snapshot(plan),
            Some((plan.vector_id(), plan.vector_approval(), scope.name.clone())),
        )
        .await
        .unwrap()
}
#[tokio::test]
async fn review_is_local_bound_to_session_and_hash_and_connection_changes_deny_commit() {
    let (_dir, manager, service, session) = setup().await;
    let value = review(&manager, &service, &session).await;
    assert_eq!(value["kind"], "vector");
    assert_eq!(value["status"], "pending");
    assert!(value["vector"].is_null());
    assert!(value["expectedBytes"].is_null());
    assert_eq!(value["jobs"], json!([]));
    assert_eq!(value["vectorReview"]["clipped"], false);
    assert!(manager.list_vectors().await.is_empty() && manager.list().await.is_empty());
    let id = value["planId"].as_str().unwrap();
    let hash = value["planHash"].as_str().unwrap();
    assert!(manager
        .approve_agent_plan(&Uuid::new_v4().to_string(), id, hash)
        .await
        .is_err());
    assert!(manager
        .approve_agent_plan(&session, id, &"0".repeat(64))
        .await
        .is_err());
    for name in [
        "geod_vector_extract",
        "geod_vector_approve",
        "geod_query_features",
    ] {
        assert!(call(manager.clone(), &session, name, json!({}), None)
            .await
            .is_err());
    }
    manager
        .inner
        .feature_services
        .lock()
        .await
        .get_mut(&service)
        .unwrap()
        .collections[0]
        .description
        .push_str(" changed");
    assert!(manager
        .approve_agent_plan(&session, id, hash)
        .await
        .unwrap_err()
        .contains("service changed"));
    assert!(manager.list_vectors().await.is_empty());
    let mut args = json!({"serviceId":service,"collectionId":"lakes","bounds":[0,0,1,1],"url":"https://other.example/api"});
    assert!(call(
        manager.clone(),
        &session,
        "geod_vector_extract_plan",
        args.clone(),
        None
    )
    .await
    .is_err());
    args.as_object_mut().unwrap().remove("url");
    args["bounds"] = json!([1, 0, 0, 1]);
    assert!(call(
        manager.clone(),
        &session,
        "geod_vector_extract_plan",
        args,
        None
    )
    .await
    .is_err());
}
#[tokio::test]
async fn confirmation_receipt_and_result_are_atomic_reused_and_resumed_without_jobs() {
    let (dir, manager, service, session) = setup().await;
    let value = review(&manager, &service, &session).await;
    let plan = native_plan(&manager, &session, &value).await;
    let asset = publish_fixture(&manager, &plan).await;
    assert_eq!(asset.id, plan.vector_id());
    assert_eq!(asset.agent_approval.as_ref().unwrap().plan_hash, plan.hash);
    let registry: Value = serde_json::from_slice(
        &tokio::fs::read(manager.inner.root.join("vectors.json"))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        registry[&asset.id]["asset"]["agentApproval"]["planId"],
        plan.id
    );
    manager.forget_feature_service(&service).await.unwrap(); // Confirmed results do not depend on a live service.
    for _ in 0..2 {
        let view = manager
            .approve_agent_plan(&session, &plan.id, &plan.hash)
            .await
            .unwrap();
        assert_eq!(view["status"], "submitted");
        assert_eq!(view["vector"]["id"], asset.id);
        assert_eq!(view["jobs"], json!([]));
    }
    manager.shutdown().await.unwrap();
    drop(manager);
    let manager = JobManager::open(dir.path()).await.unwrap();
    let view = manager
        .approve_agent_plan(&session, &plan.id, &plan.hash)
        .await
        .unwrap();
    assert_eq!(view["vector"]["sourceSha256"], asset.source_sha256);
    assert_eq!(manager.list_vectors().await.len(), 1);
    assert!(manager.list().await.is_empty());
    tokio::fs::write(
        manager
            .inner
            .root
            .join("vectors")
            .join(format!("{}.json", asset.id)),
        b"changed",
    )
    .await
    .unwrap();
    assert!(manager
        .approve_agent_plan(&session, &plan.id, &plan.hash)
        .await
        .unwrap_err()
        .contains("changed"));
}
#[tokio::test]
async fn published_original_before_registry_commit_is_recovered_only_from_exact_receipt() {
    let (dir, manager, service, session) = setup().await;
    let value = review(&manager, &service, &session).await;
    let plan = native_plan(&manager, &session, &value).await;
    let asset = publish_fixture(&manager, &plan).await;
    let record = manager
        .inner
        .vectors
        .lock()
        .await
        .remove(&asset.id)
        .unwrap();
    tokio::fs::write(manager.inner.root.join("vectors.json"), b"{}")
        .await
        .unwrap();
    let pending = manager
        .inner
        .root
        .join("agent-vector-pending")
        .join(format!("{}.json", asset.id));
    tokio::fs::write(&pending, serde_json::to_vec(&record).unwrap())
        .await
        .unwrap();
    manager.shutdown().await.unwrap();
    drop(manager);
    let manager = JobManager::open(dir.path()).await.unwrap();
    assert_eq!(
        manager.agent_plan_status(&session, &plan.id).await.unwrap()["status"],
        "pending"
    );
    assert!(manager.list_vectors().await.is_empty()); // Status never promotes an uncommitted file.
    let view = manager
        .approve_agent_plan(&session, &plan.id, &plan.hash)
        .await
        .unwrap();
    assert_eq!(view["vector"]["id"], asset.id);
    assert!(!pending.exists());
    assert_eq!(manager.list_vectors().await.len(), 1);
}
#[tokio::test]
async fn failed_registry_commit_rolls_back_its_file_and_approval() {
    let (_dir, manager, service, session) = setup().await;
    let value = review(&manager, &service, &session).await;
    let plan = native_plan(&manager, &session, &value).await;
    tokio::fs::create_dir(manager.inner.root.join("vectors.json"))
        .await
        .unwrap();
    let result = manager
        .import_feature_snapshot(
            snapshot(&plan),
            Some((
                plan.vector_id(),
                plan.vector_approval(),
                "Reviewed lakes".into(),
            )),
        )
        .await;
    assert!(result.is_err());
    assert!(manager.list_vectors().await.is_empty());
    assert!(!manager
        .inner
        .root
        .join("vectors")
        .join(format!("{}.json", plan.vector_id()))
        .exists());
    assert!(!manager
        .inner
        .root
        .join("agent-vector-pending")
        .join(format!("{}.json", plan.vector_id()))
        .exists());
}
#[tokio::test]
async fn human_edits_replace_the_review_and_preserve_the_native_service_pin() {
    let (_dir, manager, service, session) = setup().await;
    let value = review(&manager, &service, &session).await;
    let plan = native_plan(&manager, &session, &value).await;
    let form = manager
        .agent_plan_revision_draft(&session, &plan.id, &plan.hash)
        .await
        .unwrap();
    assert_eq!(form["kind"], "vector");
    let parameters = PlanRevision::Vector {
        bounds: [0.1, 0.1, 0.9, 0.9],
        name: "New lakes".into(),
        keep_polygon: false,
    };
    let replacement = manager
        .revise_agent_plan(&session, &plan.id, &plan.hash, parameters.clone())
        .await
        .unwrap();
    assert_eq!(replacement["vectorReview"]["serviceId"], service);
    assert_eq!(
        replacement["vectorReview"]["serviceSha256"],
        value["vectorReview"]["serviceSha256"]
    );
    assert_eq!(replacement["vectorReview"]["name"], "New lakes");
    assert_eq!(replacement["bounds"], json!([0.1, 0.1, 0.9, 0.9]));
    assert_ne!(replacement["planHash"], value["planHash"]);
    assert_eq!(
        manager
            .revise_agent_plan(&session, &plan.id, &plan.hash, parameters)
            .await
            .unwrap()["planId"],
        replacement["planId"]
    );
    assert!(manager
        .approve_agent_plan(&session, &plan.id, &plan.hash)
        .await
        .unwrap_err()
        .contains("corrected"));
    assert!(manager.list_vectors().await.is_empty());
}

#[tokio::test]
async fn a_forged_pending_scope_is_rejected_before_registering_the_original() {
    let (_dir, manager, service, session) = setup().await;
    let value = review(&manager, &service, &session).await;
    let plan = native_plan(&manager, &session, &value).await;
    let asset = publish_fixture(&manager, &plan).await;
    let mut record = manager
        .inner
        .vectors
        .lock()
        .await
        .remove(&asset.id)
        .unwrap();
    tokio::fs::write(manager.inner.root.join("vectors.json"), b"{}")
        .await
        .unwrap();
    let marker = manager
        .inner
        .root
        .join("agent-vector-pending")
        .join(format!("{}.json", asset.id));
    record.asset.name = "Unreviewed replacement".into();
    tokio::fs::write(&marker, serde_json::to_vec(&record).unwrap())
        .await
        .unwrap();
    let error = manager
        .approve_agent_plan(&session, &plan.id, &plan.hash)
        .await
        .unwrap_err();
    assert!(error.contains("confirmed review"), "{error}");
    assert!(manager.list_vectors().await.is_empty());
    assert_eq!(
        tokio::fs::read(manager.inner.root.join("vectors.json"))
            .await
            .unwrap(),
        b"{}"
    );
    assert!(marker.exists());
    record.asset.name = "Reviewed lakes".into();
    tokio::fs::write(&marker, serde_json::to_vec(&record).unwrap())
        .await
        .unwrap();
    let view = manager
        .approve_agent_plan(&session, &plan.id, &plan.hash)
        .await
        .unwrap();
    assert_eq!(view["vector"]["id"], asset.id);
    assert!(!marker.exists());
}
#[tokio::test]
async fn expired_review_has_no_network_or_registration_and_protocol_is_pinned() {
    let (_dir, manager, service, session) = setup().await;
    let value = review(&manager, &service, &session).await;
    let mut plan = native_plan(&manager, &session, &value).await;
    plan.created_at = (Utc::now() - chrono::Duration::minutes(60)).to_rfc3339();
    plan.expires_at = (Utc::now() - chrono::Duration::minutes(30)).to_rfc3339();
    plan.hash = plan.effective_hash().unwrap();
    write_record(&manager.inner.root, "plans", &plan.id, &plan)
        .await
        .unwrap();
    assert_eq!(
        manager.agent_plan_status(&session, &plan.id).await.unwrap()["status"],
        "expired"
    );
    manager.inner.feature_services.lock().await.clear();
    assert!(manager
        .approve_agent_plan(&session, &plan.id, &plan.hash)
        .await
        .unwrap_err()
        .contains("expired"));
    assert!(manager.list_vectors().await.is_empty());
    plan.created_at = now();
    plan.expires_at = (Utc::now() + chrono::Duration::minutes(TTL_MINUTES)).to_rfc3339();
    let asset = publish_fixture(&manager, &plan).await;
    let Action::Vector { scope } = &mut plan.action else {
        panic!()
    };
    scope.protocol = "ArcGIS".into();
    assert!(validate_result(&plan, &asset)
        .unwrap_err()
        .contains("protocol"));
}

#[tokio::test]
async fn wfs_formats_and_overpass_region_limits_are_checked_before_review_or_network() {
    let (_dir, manager, service, session) = setup().await;
    {
        let mut services = manager.inner.feature_services.lock().await;
        let source = services.get_mut(&service).unwrap();
        source.wfs = Some(crate::features::wfs::Service {
            version: "2.0.0".into(),
            capabilities_sha256: "a".repeat(64),
            fees: "NONE".into(),
            access_constraints: "NONE".into(),
            paging_supported: true,
            excluded_layers: vec![],
        });
        source.collections[0].wfs = Some(crate::features::wfs::Layer {
            type_name: "lakes".into(),
            namespace: "https://example.com/lakes".into(),
            default_crs: "urn:ogc:def:crs:EPSG::4326".into(),
            other_crs: vec![],
            formats: vec![crate::features::wfs::Format {
                id: "application/json".into(),
                mime: "application/json".into(),
            }],
            default_format: "application/json".into(),
        });
    }
    let value = review(&manager, &service, &session).await;
    assert_eq!(value["vectorReview"]["protocol"], "WFS 2");
    assert_eq!(value["vectorReview"]["responseFormat"], "application/json");
    assert_eq!(value["vectorReview"]["selection"], "wfs-bbox-full-features");
    let error=call(manager.clone(),&session,"geod_vector_extract_plan",json!({"serviceId":service,"collectionId":"lakes","bounds":[0,0,1,1],"responseFormat":"unsupported-format"}),None).await.unwrap_err();
    assert!(error.contains("advertised WFS"));
    {
        let mut services = manager.inner.feature_services.lock().await;
        let source = services.get_mut(&service).unwrap();
        source.wfs = None;
        source.collections[0].wfs = None;
        source.collections[0].id = "roads".into();
        source.collections[0].title = "Roads".into();
        source.overpass = Some(crate::features::overpass::Service {
            generator: "Synthetic Overpass fixture".into(),
            api_version: 0.6,
            metadata_sha256: "b".repeat(64),
            copyright_text: "OpenStreetMap contributors".into(),
        });
    }
    let args = json!({"serviceId":service,"collectionId":"roads","bounds":[0,0,0.01,0.01]});
    let value = call(
        manager.clone(),
        &session,
        "geod_vector_extract_plan",
        args.clone(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(value["vectorReview"]["protocol"], "Overpass");
    assert!(value["vectorReview"]["pageSize"].is_null());
    let plan = native_plan(&manager, &session, &value).await;
    let Action::Vector { scope } = &plan.action else {
        panic!()
    };
    assert!(scope.request.page_size.is_none());
    assert_eq!(
        value["vectorReview"]["selection"],
        "overpass-bbox-full-geometry"
    );
    let mut invalid = args.clone();
    invalid["pageSize"] = json!(2);
    assert!(call(
        manager.clone(),
        &session,
        "geod_vector_extract_plan",
        invalid,
        None
    )
    .await
    .unwrap_err()
    .contains("does not use page sizes"));
    let mut invalid = args.clone();
    invalid["bounds"] = json!([0, 0, 1, 1]);
    assert!(call(
        manager.clone(),
        &session,
        "geod_vector_extract_plan",
        invalid,
        None
    )
    .await
    .is_err());
    let mut invalid = args;
    invalid["responseFormat"] = json!("application/json");
    assert!(call(
        manager.clone(),
        &session,
        "geod_vector_extract_plan",
        invalid,
        None
    )
    .await
    .is_err());
    assert!(manager.list_vectors().await.is_empty() && manager.list().await.is_empty());
}
