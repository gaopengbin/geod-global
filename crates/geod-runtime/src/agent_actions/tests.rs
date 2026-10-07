use super::*;

#[test]
fn map_context_accepts_date_free_dem_without_weakening_temporal_validation() {
    let project_id = Uuid::new_v4().to_string();
    let mut raw = json!({"page":"My Data","provider":"copernicus-dem","bounds":[-123,37,-122,38],"cloudMax":100,"projectId":project_id});
    let context: MapContext = serde_json::from_value(raw.clone()).unwrap();
    context.validate().unwrap();
    let serialized = serde_json::to_value(context).unwrap();
    assert_eq!(serialized["projectId"], project_id);
    assert!(serialized.get("start").is_none());
    raw["provider"] = json!("earth-search");
    assert!(serde_json::from_value::<MapContext>(raw.clone())
        .unwrap()
        .validate()
        .is_err());
    raw["provider"] = json!("copernicus-dem");
    raw["start"] = json!("2026-10-01");
    assert!(serde_json::from_value::<MapContext>(raw.clone())
        .unwrap()
        .validate()
        .is_err());
    raw["end"] = json!("2026-10-06");
    serde_json::from_value::<MapContext>(raw.clone())
        .unwrap()
        .validate()
        .unwrap();
    raw["bounds"] = json!([-122, 37, -123, 38]);
    assert!(serde_json::from_value::<MapContext>(raw)
        .unwrap()
        .validate()
        .is_err());
}

pub(super) async fn local_source(manager: &JobManager) -> Job {
    let job = crate::raster::tests::record(
        &manager.inner.root,
        &crate::raster::tests::fixture(100, 100, &vec![4; 10000], 32610, false),
    );
    let mut store = manager.inner.store.lock().await;
    store.jobs.insert(job.id.clone(), job.clone());
    manager.persist(&store.jobs).await.unwrap();
    job
}
pub(super) async fn crop_plan(manager: &JobManager, session: &str) -> Value {
    let job = local_source(manager).await;
    // Native source-grid plan is an isolated synthetic unit fixture, not remote-data proof.
    let recipe = crate::RasterRecipe {
        schema_version: crate::processing::RECIPE_SCHEMA_VERSION.into(),
        name: "Unit crop".into(),
        source: crate::processing::RecipeSource {
            job_id: job.id,
            sha256: job.sha256.unwrap(),
        },
        operation: crate::processing::ClipOperation {
            operation_type: "clip".into(),
            crs: "source".into(),
            bounds: [500000.0, 4199000.0, 501000.0, 4200000.0],
            geometry: None,
        },
        output: crate::processing::RecipeOutput {
            format: "GeoTIFF".into(),
        },
    };
    let p = manager.plan_recipe(recipe).await.unwrap();
    manager
        .save_agent_plan(
            session,
            Action::Clip {
                recipe: p.recipe,
                output: p.plan,
            },
        )
        .await
        .unwrap()
}

pub(super) async fn project_search_fixture(manager: &JobManager, session: &str) -> SearchReceipt {
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../../prototype/public/samples/earth-search-response.json"
    ))
    .unwrap();
    value["features"] = json!([value["features"][0], value["features"][1]]);
    let query = SearchQuery {
        provider: "earth-search".into(),
        bounds: [-123.001, 37.947, -122.999, 37.949],
        start: "2025-06-01".into(),
        end: "2025-06-30".into(),
        cloud_max: 60.0,
        limit: 5,
    };
    // Synthetic native processing jobs below use this tiny source-grid extent,
    // which is outside the retained catalog scene's real northern edge. Their
    // synthetic coverage must be explicit, never asserted as public STAC proof.
    let mut candidates = normalize_catalog(&value, &query).unwrap();
    for c in &mut candidates {
        c.footprint = Some(footprint::rectangle(query.bounds));
    }
    let receipt = SearchReceipt {
        id: Uuid::new_v4().to_string(),
        session_id: session.into(),
        candidates,
        query,
        retrieved_at: now(),
        document_sha256: digest(value.to_string().as_bytes()),
        more_available: false,
        next: None,
    };
    write_record(&manager.inner.root, "searches", &receipt.id, &receipt)
        .await
        .unwrap();
    receipt
}
pub(super) async fn confirmed_project(
    manager: &JobManager,
    session: &str,
) -> (Value, SearchReceipt) {
    let receipt = project_search_fixture(manager, session).await;
    let plan = manager
        .agent_project_plan(
            session,
            &receipt.id,
            vec![receipt.candidates[0].item_id.clone()],
            Some("Agent project".into()),
            None,
        )
        .await
        .unwrap();
    let committed = manager
        .approve_agent_plan(
            session,
            plan["planId"].as_str().unwrap(),
            plan["planHash"].as_str().unwrap(),
        )
        .await
        .unwrap();
    (committed, receipt)
}
pub(super) async fn source_for_project(manager: &JobManager, project_id: &str) -> Job {
    let project = manager
        .list_projects()
        .await
        .into_iter()
        .find(|p| p.id == project_id)
        .unwrap();
    let scene = &project.scenes[0];
    let mut job = crate::raster::tests::record(
        &manager.inner.root,
        &crate::raster::tests::fixture(100, 100, &vec![4; 10000], 32610, false),
    );
    job.item_id = scene.item_id.clone();
    job.href = scene.assets["scl"].href.clone();
    let mut store = manager.inner.store.lock().await;
    store.jobs.insert(job.id.clone(), job.clone());
    manager.persist(&store.jobs).await.unwrap();
    job
}

#[tokio::test]
async fn project_confirmation_is_atomic_idempotent_and_does_not_download() {
    let directory = tempfile::tempdir().unwrap();
    let manager = JobManager::open(directory.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let receipt = project_search_fixture(&manager, &session).await;
    let plan = manager
        .agent_project_plan(
            &session,
            &receipt.id,
            vec![receipt.candidates[0].item_id.clone()],
            Some("Review first".into()),
            None,
        )
        .await
        .unwrap();
    assert!(manager.list_projects().await.is_empty());
    assert!(manager.list().await.is_empty());
    let id = plan["planId"].as_str().unwrap();
    let hash = plan["planHash"].as_str().unwrap();
    assert!(manager
        .approve_agent_plan(&Uuid::new_v4().to_string(), id, hash)
        .await
        .is_err());
    let temporary = manager.inner.root.join("projects.json.tmp");
    std::fs::create_dir(&temporary).unwrap();
    assert!(manager
        .approve_agent_plan(&session, id, hash)
        .await
        .is_err());
    assert!(manager.list_projects().await.is_empty());
    std::fs::remove_dir(temporary).unwrap();
    let (a, b) = tokio::join!(
        manager.approve_agent_plan(&session, id, hash),
        manager.approve_agent_plan(&session, id, hash)
    );
    let a = a.unwrap();
    assert_eq!(a["project"]["id"], b.unwrap()["project"]["id"]);
    assert_eq!(a["status"], "submitted");
    let project_id = a["project"]["id"].as_str().unwrap();
    assert!(manager.list().await.is_empty());
    manager
        .rename_project(project_id, "Renamed after approval")
        .await
        .unwrap();
    let repeat = manager
        .approve_agent_plan(&session, id, hash)
        .await
        .unwrap();
    assert_eq!(repeat["project"]["id"], project_id);
    assert_eq!(
        manager.list_projects().await[0].name,
        "Renamed after approval"
    );
    manager.shutdown().await.unwrap();
    drop(manager);
    let manager = JobManager::open(directory.path()).await.unwrap();
    assert_eq!(
        manager
            .approve_agent_plan(&session, id, hash)
            .await
            .unwrap()["status"],
        "submitted"
    );
    assert_eq!(manager.list_projects().await.len(), 1);
    assert_eq!(manager.list_projects().await[0].agent_approvals.len(), 1);
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn append_plan_rechecks_project_and_preserves_original_area_and_assets() {
    let directory = tempfile::tempdir().unwrap();
    let manager = JobManager::open(directory.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let (created, receipt) = confirmed_project(&manager, &session).await;
    let project_id = created["project"]["id"].as_str().unwrap();
    let original = manager.list_projects().await[0].clone();
    let plan = manager
        .agent_project_plan(
            &session,
            &receipt.id,
            vec![receipt.candidates[1].item_id.clone()],
            None,
            Some(project_id.into()),
        )
        .await
        .unwrap();
    assert_eq!(manager.list_projects().await[0].scenes.len(), 1);
    manager
        .rename_project(project_id, "Another edit")
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
    manager
        .rename_project(project_id, &original.name)
        .await
        .unwrap();
    manager
        .approve_agent_plan(
            &session,
            plan["planId"].as_str().unwrap(),
            plan["planHash"].as_str().unwrap(),
        )
        .await
        .unwrap();
    let saved = manager.list_projects().await[0].clone();
    assert_eq!(saved.bounds, original.bounds);
    assert_eq!(saved.scenes.len(), 2);
    let pinned = saved
        .scenes
        .iter()
        .find(|s| s.item_id == original.scenes[0].item_id)
        .unwrap();
    assert_eq!(
        serde_json::to_value(pinned).unwrap(),
        serde_json::to_value(&original.scenes[0]).unwrap()
    );
    assert!(manager.list().await.is_empty());
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn project_downloads_reuse_existing_tasks_and_processing_pins_the_project() {
    let directory = tempfile::tempdir().unwrap();
    let manager = JobManager::open(directory.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let (created, _) = confirmed_project(&manager, &session).await;
    let project_id = created["project"]["id"].as_str().unwrap();
    let source = source_for_project(&manager, project_id).await;
    let reuse = manager
        .agent_project_download_plan(&session, project_id, "scl", None)
        .await
        .unwrap();
    assert_eq!(reuse["needsDownload"], false);
    assert_eq!(reuse["jobs"][0]["id"], source.id);
    assert_eq!(manager.list().await.len(), 1);
    let plan = manager
        .agent_project_mosaic_plan(&session, project_id, "scl")
        .await
        .unwrap();
    assert_eq!(plan["kind"], "mosaic");
    assert_eq!(manager.list().await.len(), 1);
    assert!(plan["files"][0]["width"].as_u64().unwrap() > 0);
    let id = plan["planId"].as_str().unwrap();
    let hash = plan["planHash"].as_str().unwrap();
    manager
        .rename_project(project_id, "Changed before approval")
        .await
        .unwrap();
    assert!(manager
        .approve_agent_plan(&session, id, hash)
        .await
        .is_err());
    manager
        .rename_project(project_id, "Agent project")
        .await
        .unwrap();
    let a = manager
        .approve_agent_plan(&session, id, hash)
        .await
        .unwrap();
    let job_id = a["jobs"][0]["id"].as_str().unwrap();
    manager
        .rename_project(project_id, "Changed after approval")
        .await
        .unwrap();
    let job = manager.wait(job_id).await.unwrap();
    assert_eq!(job.status, JobStatus::Succeeded, "{:?}", job.error);
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(job.manifest_path.unwrap()).unwrap()).unwrap();
    assert_eq!(manifest["project"]["name"], "Agent project");
    assert_eq!(manifest["sources"][0]["sha256"], source.sha256.unwrap());
    assert!(manager.inspect_raster(job_id).await.is_ok());
    assert_eq!(
        manager
            .approve_agent_plan(&session, id, hash)
            .await
            .unwrap()["jobs"][0]["id"],
        job_id
    );
    assert_eq!(manager.list().await.len(), 2);
    manager.shutdown().await.unwrap();
    drop(manager);
    let manager = JobManager::open(directory.path()).await.unwrap();
    assert_eq!(
        manager
            .approve_agent_plan(&session, id, hash)
            .await
            .unwrap()["jobs"][0]["id"],
        job_id
    );
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn model_cannot_approve_and_hash_and_conversation_are_bound() {
    let directory = tempfile::tempdir().unwrap();
    let manager = JobManager::open(directory.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let p = crop_plan(&manager, &session).await;
    let id = p["planId"].as_str().unwrap();
    let hash = p["planHash"].as_str().unwrap();
    assert_eq!(manager.list().await.len(), 1);
    assert!(call(
        manager.clone(),
        &session,
        "geod_approve_plan",
        json!({"planId":id,"planHash":hash}),
        None
    )
    .await
    .is_err());
    assert!(
        call(manager.clone(), &session, "geod_download", json!({}), None)
            .await
            .is_err()
    );
    assert!(manager
        .approve_agent_plan(&session, id, &"0".repeat(64))
        .await
        .is_err());
    assert!(manager
        .approve_agent_plan(&Uuid::new_v4().to_string(), id, hash)
        .await
        .is_err());
    assert_eq!(manager.list().await.len(), 1);
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn atomic_approval_double_click_and_restart_keep_same_job() {
    let directory = tempfile::tempdir().unwrap();
    let manager = JobManager::open(directory.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let p = crop_plan(&manager, &session).await;
    let id = p["planId"].as_str().unwrap();
    let hash = p["planHash"].as_str().unwrap();
    let (a, b) = tokio::join!(
        manager.approve_agent_plan(&session, id, hash),
        manager.approve_agent_plan(&session, id, hash)
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a["jobs"][0]["id"], b["jobs"][0]["id"]);
    let job_id = a["jobs"][0]["id"].as_str().unwrap();
    let job = manager.wait(job_id).await.unwrap();
    assert_eq!(job.status, JobStatus::Succeeded);
    assert!(manager.inspect_raster(job_id).await.is_ok());
    assert_eq!(manager.list().await.len(), 2);
    // The expiry deadline applies to an unsubmitted review. An already
    // committed approval must still return its original job after a long
    // transfer or a later restart, without silently running it again.
    let mut approved: Plan = read_record(&manager.inner.root, "plans", id).await.unwrap();
    approved.expires_at = (Utc::now() - chrono::Duration::minutes(1)).to_rfc3339();
    write_record(&manager.inner.root, "plans", id, &approved)
        .await
        .unwrap();
    let expired_approval = manager
        .approve_agent_plan(&session, id, hash)
        .await
        .unwrap();
    assert_eq!(expired_approval["jobs"][0]["id"], job_id);
    assert_eq!(manager.list().await.len(), 2);
    manager.shutdown().await.unwrap();
    drop(manager);
    let manager = JobManager::open(directory.path()).await.unwrap();
    let same = manager
        .approve_agent_plan(&session, id, hash)
        .await
        .unwrap();
    assert_eq!(same["jobs"][0]["id"], job_id);
    assert_eq!(manager.list().await.len(), 2);
    let receipt = manager.get(job_id).await.unwrap().agent_approval.unwrap();
    assert_eq!(receipt.plan_hash, hash);
    assert_eq!(receipt.session_id, session);
    assert!(!same.to_string().contains("outputPath"));
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn expiry_mutation_and_changed_file_do_not_submit() {
    let directory = tempfile::tempdir().unwrap();
    let manager = JobManager::open(directory.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let p = crop_plan(&manager, &session).await;
    let id = p["planId"].as_str().unwrap();
    let hash = p["planHash"].as_str().unwrap();
    let mut plan: Plan = read_record(&manager.inner.root, "plans", id).await.unwrap();
    plan.expires_at = (Utc::now() - chrono::Duration::minutes(1)).to_rfc3339();
    write_record(&manager.inner.root, "plans", id, &plan)
        .await
        .unwrap();
    assert_eq!(
        manager.agent_plan_status(&session, id).await.unwrap()["status"],
        "expired"
    );
    assert!(manager
        .approve_agent_plan(&session, id, hash)
        .await
        .is_err());
    plan.expires_at = (Utc::now() + chrono::Duration::minutes(10)).to_rfc3339();
    if let Action::Clip { recipe, .. } = &mut plan.action {
        recipe.operation.bounds[2] -= 20.0;
    }
    write_record(&manager.inner.root, "plans", id, &plan)
        .await
        .unwrap();
    assert!(manager
        .approve_agent_plan(&session, id, hash)
        .await
        .is_err());
    assert_eq!(manager.list().await.len(), 1);
    let p = crop_plan(&manager, &session).await;
    let mut source = manager
        .list()
        .await
        .into_iter()
        .find(|j| j.kind == "download")
        .unwrap();
    let plan: Plan = read_record(&manager.inner.root, "plans", p["planId"].as_str().unwrap())
        .await
        .unwrap();
    if let Action::Clip { recipe, .. } = plan.action {
        source = manager.get(&recipe.source.job_id).await.unwrap();
    }
    tokio::fs::write(source.output_path.unwrap(), b"altered")
        .await
        .unwrap();
    assert!(manager
        .approve_agent_plan(
            &session,
            p["planId"].as_str().unwrap(),
            p["planHash"].as_str().unwrap()
        )
        .await
        .is_err());
    manager.shutdown().await.unwrap();
}

#[test]
fn normalized_plan_hash_ignores_time_and_ids_but_tracks_scope_and_policy() {
    let query = SearchQuery {
        provider: "earth-search".into(),
        bounds: [-122.55, 37.68, -122.32, 37.84],
        start: "2025-06-01".into(),
        end: "2025-06-30".into(),
        cloud_max: 60.0,
        limit: 5,
    };
    let mut p = Plan {
        id: Uuid::new_v4().to_string(),
        session_id: Uuid::new_v4().to_string(),
        policy: POLICY.into(),
        hash: String::new(),
        created_at: now(),
        expires_at: now(),
        action: Action::Download {
            acquisition: None,
            query,
            metadata_sha256: "abc".into(),
            files: vec![],
            project: None,
        },
    };
    let hash = p.effective_hash().unwrap();
    p.id = Uuid::new_v4().to_string();
    p.created_at = "different".into();
    assert_eq!(p.effective_hash().unwrap(), hash);
    if let Action::Download { query, .. } = &mut p.action {
        query.bounds[0] += 0.01;
    }
    assert_ne!(p.effective_hash().unwrap(), hash);
    let hash = p.effective_hash().unwrap();
    p.policy = "changed-policy".into();
    assert_ne!(p.effective_hash().unwrap(), hash);
}

#[test]
fn strict_context_and_catalog_reject_forged_assets_or_wrong_scope() {
    let query = SearchQuery {
        provider: "earth-search".into(),
        bounds: [-122.55, 37.68, -122.32, 37.84],
        start: "2025-06-01".into(),
        end: "2025-06-30".into(),
        cloud_max: 60.0,
        limit: 5,
    };
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../../prototype/public/samples/earth-search-response.json"
    ))
    .unwrap();
    // Search fixture supplies a larger page; only the requested bounds/dates pass.
    value["features"] = json!([value["features"][0]]);
    assert!(normalize_catalog(&value, &query).is_ok());
    value["features"][0]["assets"]["scl"]["href"] = json!("https://example.com/SCL.tif");
    assert!(normalize_catalog(&value, &query).is_err());
    assert!(serde_json::from_value::<SearchQuery>(json!({"provider":"earth-search","bounds":query.bounds,"start":query.start,"end":query.end,"url":"http://127.0.0.1"})).is_err());
    let mut bad = query;
    bad.provider = "nasa".into();
    assert!(bad.validate().is_err());
}

#[test]
fn public_catalog_adapters_preserve_native_identity_calibration_and_static_dates() {
    let fixtures = [
        (
            "planetary-computer",
            include_str!("../../../../prototype/public/samples/planetary-computer-response.json"),
        ),
        (
            "planetary-landsat",
            include_str!("../../../../prototype/public/samples/landsat-quality-response.json"),
        ),
        (
            "planetary-vegetation",
            include_str!("../../../../prototype/public/samples/modis-vegetation-response.json"),
        ),
        (
            "planetary-naip",
            include_str!("../../../../prototype/public/samples/naip-response.json"),
        ),
        (
            "copernicus-dem",
            include_str!("../../../../prototype/public/samples/cop-dem-response.json"),
        ),
        (
            "copernicus-dem-90",
            include_str!("../../../../prototype/public/samples/cop-dem-90-response.json"),
        ),
    ];
    for (provider, raw) in fixtures {
        let query = SearchQuery {
            provider: provider.into(),
            bounds: [-180.0, -90.0, 180.0, 90.0],
            start: "2000-01-01".into(),
            end: "2026-10-04".into(),
            cloud_max: 100.0,
            limit: 20,
        };
        let value: Value = serde_json::from_str(raw).unwrap();
        let candidates =
            normalize_catalog(&value, &query).unwrap_or_else(|e| panic!("{provider}: {e}"));
        assert!(!candidates.is_empty());
        let scenes = candidates
            .iter()
            .map(|c| crate::projects::ProjectScene {
                footprint: c.footprint.clone(),
                item_id: c.item_id.clone(),
                date: c.date.clone(),
                cloud: c.cloud,
                crs: c.crs.clone(),
                grid_code: None,
                bbox: c.bounds,
                assets: c
                    .assets
                    .iter()
                    .map(|a| {
                        (
                            a.asset_key.clone(),
                            crate::projects::ProjectAsset {
                                href: a.href.clone(),
                                media_type: a.media_type.clone(),
                                raster_band: c.bands.get(&a.asset_key).cloned(),
                            },
                        )
                    })
                    .collect(),
            })
            .collect();
        crate::CreateProjectRequest {
            name: "Public fixture".into(),
            bounds: query.bounds,
            geometry: None,
            scenes,
        }
        .validate(None)
        .unwrap();
        if provider.starts_with("copernicus-dem") {
            assert!(candidates
                .iter()
                .all(|c| c.cloud.is_none() && c.date_role.is_some()));
            let static_query = SearchQuery {
                start: String::new(),
                end: String::new(),
                ..query
            };
            static_query.validate().unwrap();
            let url = catalog::url(&static_query).unwrap();
            assert!(url
                .query_pairs()
                .all(|(key, _)| key != "datetime" && key != "query"));
            assert!(normalize_catalog(&value, &static_query).is_ok());
        }
    }
    assert_eq!(catalog::sources()["sources"].as_array().unwrap().len(), 15);
    assert!(catalog::source("copernicus").is_ok());
    assert!(catalog::source("unreviewed-provider").is_err());
    assert!(catalog::source("http://127.0.0.1").is_err());
}

#[tokio::test]
#[ignore = "Explicit GEOD_AGENT_LIVE_DATA=1; actual original transfers for nine public adapters in an isolated store"]
async fn live_public_provider_agent_matrix() {
    assert_eq!(std::env::var("GEOD_AGENT_LIVE_DATA").as_deref(), Ok("1"));
    let directory =
        PathBuf::from(std::env::var("GEOD_AGENT_TEST_STORE").expect("explicit isolated store"));
    let manager = JobManager::open(&directory).await.unwrap();
    if let Ok(proxy) = std::env::var("GEOD_AGENT_TEST_PROXY") {
        manager
            .save_proxy_settings(crate::ProxySettings {
                mode: crate::proxy::ProxyMode::Custom,
                url: Some(proxy),
            })
            .await
            .unwrap();
    }
    let session = Uuid::new_v4().to_string();
    let mut rows = Vec::new();
    let selected = std::env::var("GEOD_AGENT_TEST_PROVIDERS")
        .ok()
        .map(|value| value.split(',').map(str::to_owned).collect::<BTreeSet<_>>());
    if let Some(selected) = &selected {
        assert!(
            !selected.is_empty()
                && selected
                    .iter()
                    .all(|id| catalog::IDS.contains(&id.as_str()))
        );
    }
    let wait_seconds = std::env::var("GEOD_AGENT_TEST_WAIT_SECONDS")
        .ok()
        .map(|value| {
            value
                .parse::<u64>()
                .expect("explicit transfer wait seconds")
        })
        .unwrap_or(420);
    assert!((30..=7200).contains(&wait_seconds));
    for provider in catalog::IDS {
        if selected
            .as_ref()
            .is_some_and(|ids| !ids.contains(*provider))
        {
            continue;
        }
        let started = std::time::Instant::now();
        let result = async {
            let search = manager.agent_search(&session,SearchQuery {provider:(*provider).into(),bounds:[-122.46,37.76,-122.45,37.77],start:if *provider == "planetary-naip" {"2010-01-01"} else {"2025-06-01"}.into(),end:"2025-06-30".into(),cloud_max:100.0,limit:1}).await?;
            let scene = search["scenes"][0]["itemId"].as_str().ok_or("No actual matching scenes.")?;
            let key = match *provider { "earth-search"|"planetary-computer"=>"scl", "planetary-landsat"=>"qa_pixel", "planetary-modis"=>"modis_state", "planetary-vegetation"=>"vi_reliability", "planetary-radar"=>"vv", "planetary-naip"=>"aerial", _=>"elevation" };
            let count = manager.list().await.len();
            let project_plan = manager.agent_project_plan(&session,search["searchId"].as_str().unwrap(),vec![scene.into()],Some(format!("Agent public check · {provider}")),None).await?;
            assert_eq!(manager.list().await.len(),count);
            let saved = manager.approve_agent_plan(&session,project_plan["planId"].as_str().unwrap(),project_plan["planHash"].as_str().unwrap()).await?;
            let project_id = saved["project"]["id"].as_str().unwrap();
            let plan = manager.agent_project_download_plan(&session,project_id,key,None).await?;
            assert_eq!(manager.list().await.len(),count);
            // The explicit acceptance budget does not change the product limit.
            // Larger originals remain clearly marked as preflight-only.
            if let Ok(budget) = std::env::var("GEOD_AGENT_TEST_FILE_BUDGET") {
                let budget: u64 = budget.parse().map_err(|_|"Invalid explicit test byte budget.")?;
                if plan["expectedBytes"].as_u64().ok_or("Missing preflight bytes.")? > budget {
                    return Ok::<_,String>(json!({"provider":provider,"search":search,"projectPlan":project_plan,"project":saved,"downloadPlan":plan,"preflightOnly":true,"downloaded":false,"reason":"File exceeds explicit acceptance byte budget; product download limit is unchanged.","modelCalls":0,"fixture":false}));
                }
            }
            let submitted = manager.approve_agent_plan(&session,plan["planId"].as_str().unwrap(),plan["planHash"].as_str().unwrap()).await?;
            let id = submitted["jobs"][0]["id"].as_str().unwrap();
            let job = tokio::time::timeout(Duration::from_secs(wait_seconds),manager.wait(id)).await.map_err(|_|"Actual transfer observation deadline reached; inspect the same live task before restarting.")??;
            if job.status != JobStatus::Succeeded { return Err(job.error.unwrap_or("Transfer failed.".into())); }
            let inspected = manager.inspect_raster(id).await?;
            let again = manager.approve_agent_plan(&session,plan["planId"].as_str().unwrap(),plan["planHash"].as_str().unwrap()).await?;
            assert_eq!(again["jobs"][0]["id"],id);
            if job.href.contains('?') { return Err("Access signature persisted in job.".into()); }
            Ok::<_,String>(json!({"provider":provider,"search":search,"projectPlan":project_plan,"project":saved,"downloadPlan":plan,"jobId":id,"bytes":job.bytes_downloaded,"sha256":job.sha256,"raster":inspected,"seconds":started.elapsed().as_secs_f64(),"modelCalls":0,"fixture":false}))
        }.await;
        let row = result.unwrap_or_else(|error|json!({"provider":provider,"error":error,"fixture":false,"seconds":started.elapsed().as_secs_f64()}));
        eprintln!(
            "Agent source {}: {}",
            provider,
            if row.get("error").is_some() {
                "FAILED"
            } else if row["preflightOnly"] == true {
                "preflight only"
            } else {
                "download verified"
            }
        );
        rows.push(row);
        tokio::fs::write(
            directory
                .parent()
                .unwrap()
                .join("public-agent-acceptance.json"),
            serde_json::to_vec_pretty(&rows).unwrap(),
        )
        .await
        .unwrap();
    }
    manager.shutdown().await.unwrap();
    assert!(
        rows.iter().all(|r| r.get("error").is_none()),
        "{:?}",
        rows.iter()
            .filter(|r| r.get("error").is_some())
            .collect::<Vec<_>>()
    );
    drop(manager);
    let reopened = JobManager::open(&directory).await.unwrap();
    let expected_jobs = rows.iter().filter(|r| r["preflightOnly"] != true).count();
    for row in rows {
        if row["preflightOnly"] == true {
            continue;
        }
        let plan = &row["downloadPlan"];
        let confirmed = reopened
            .approve_agent_plan(
                &session,
                plan["planId"].as_str().unwrap(),
                plan["planHash"].as_str().unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(confirmed["jobs"][0]["id"], row["jobId"]);
    }
    assert_eq!(reopened.list().await.len(), expected_jobs);
    reopened.shutdown().await.unwrap();
}

#[tokio::test]
async fn reviewed_polygon_recipe_pins_geometry_and_needs_native_confirmation() {
    let directory = tempfile::tempdir().unwrap();
    let manager = JobManager::open(directory.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let source = local_source(&manager).await;
    let geometry = json!({"type":"Polygon","coordinates":[[[-123.0,37.947],[-122.999,37.947],[-122.999,37.948],[-123.0,37.948],[-123.0,37.947]]]});
    let request = json!({"recipe":{"schemaVersion":"geod-raster-recipe/v2","name":"Reviewed polygon","source":{"jobId":source.id,"sha256":source.sha256},"operation":{"type":"clip","crs":"EPSG:4326","bounds":[-123.0,37.947,-122.999,37.948],"geometry":geometry},"output":{"format":"GeoTIFF"}}});
    let plan = call(
        manager.clone(),
        &session,
        "geod_recipe_review_plan",
        request.clone(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(plan["status"], "pending");
    assert_eq!(manager.list().await.len(), 1);
    let mut forged = request.clone();
    forged["recipe"]["source"]["sha256"] = json!("a".repeat(64));
    assert!(call(
        manager.clone(),
        &session,
        "geod_recipe_review_plan",
        forged,
        None
    )
    .await
    .is_err());
    let mut other = request;
    other["recipe"]["operation"]["geometry"]["coordinates"][0][1][0] = json!(-122.9995);
    let revised = call(
        manager.clone(),
        &session,
        "geod_recipe_review_plan",
        other,
        None,
    )
    .await
    .unwrap();
    assert_ne!(revised["planHash"], plan["planHash"]);
    let queued = manager
        .approve_agent_plan(
            &session,
            plan["planId"].as_str().unwrap(),
            plan["planHash"].as_str().unwrap(),
        )
        .await
        .unwrap();
    let id = queued["jobs"][0]["id"].as_str().unwrap();
    let output = manager.wait(id).await.unwrap();
    assert_eq!(output.status, JobStatus::Succeeded, "{:?}", output.error);
    assert!(output.sha256.is_some());
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn attached_polygon_keeps_vertices_local_and_binds_native_crop_plan() {
    let directory = tempfile::tempdir().unwrap();
    let manager = JobManager::open(directory.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let source = local_source(&manager).await;
    let geometry = json!({"type":"Polygon","coordinates":[[[-123.0,37.947],[-122.999,37.947],[-122.999,37.948],[-123.0,37.948],[-123.0,37.947]]]});
    let context: MapContext = serde_json::from_value(json!({"page":"Explore","provider":"earth-search","bounds":[-123.0,37.947,-122.999,37.948],"start":"2025-06-01","end":"2025-06-30","cloudMax":60,"projectId":null,"geometry":geometry})).unwrap();
    let summary = call(
        manager.clone(),
        &session,
        "geod_workspace_context",
        json!({}),
        Some(context.clone()),
    )
    .await
    .unwrap();
    assert!(summary["context"]["geometry"].is_null());
    assert!(summary["attachedPolygon"].get("coordinates").is_none());
    assert!(summary["attachedPolygon"].get("geometry").is_none());
    assert_eq!(
        summary["attachedPolygon"]["sha256"],
        digest(&serde_json::to_vec(&context.geometry).unwrap())
    );
    let args = json!({"jobId":source.id,"bounds":context.bounds,"name":"Actual attached polygon","useAttachedPolygon":true});
    assert!(call(
        manager.clone(),
        &session,
        "geod_clip_plan",
        args.clone(),
        None
    )
    .await
    .is_err());
    let plan = call(
        manager.clone(),
        &session,
        "geod_clip_plan",
        args,
        Some(context),
    )
    .await
    .unwrap();
    assert_eq!(
        plan["polygon"]["sha256"],
        summary["attachedPolygon"]["sha256"]
    );
    assert_eq!(manager.list().await.len(), 1);
    manager.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore = "Explicit GEOD_AGENT_LIVE_DATA=1; real public catalog and original SCL transfer into isolated store"]
async fn live_search_review_download_crop_restart() {
    assert_eq!(std::env::var("GEOD_AGENT_LIVE_DATA").as_deref(), Ok("1"));
    let directory =
        PathBuf::from(std::env::var("GEOD_AGENT_TEST_STORE").expect("explicit isolated store"));
    let manager = JobManager::open(&directory).await.unwrap();
    if let Ok(proxy) = std::env::var("GEOD_AGENT_TEST_PROXY") {
        manager
            .save_proxy_settings(crate::ProxySettings {
                mode: crate::proxy::ProxyMode::Custom,
                url: Some(proxy),
            })
            .await
            .unwrap();
    }
    let session = Uuid::new_v4().to_string();
    let search = manager
        .agent_search(
            &session,
            SearchQuery {
                provider: "earth-search".into(),
                bounds: [-122.55, 37.68, -122.32, 37.84],
                start: "2025-06-01".into(),
                end: "2025-06-30".into(),
                cloud_max: 60.0,
                limit: 3,
            },
        )
        .await
        .unwrap();
    let scene = search["scenes"][0]["itemId"]
        .as_str()
        .expect("actual scene");
    let plan = manager
        .agent_download_plan(
            &session,
            search["searchId"].as_str().unwrap(),
            vec![scene.into()],
            "scl",
        )
        .await
        .unwrap();
    assert_eq!(plan["status"], "pending");
    assert!(manager.list().await.is_empty());
    let submitted = manager
        .approve_agent_plan(
            &session,
            plan["planId"].as_str().unwrap(),
            plan["planHash"].as_str().unwrap(),
        )
        .await
        .unwrap();
    let download_id = submitted["jobs"][0]["id"].as_str().unwrap();
    let job = tokio::time::timeout(Duration::from_secs(120), manager.wait(download_id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job.status, JobStatus::Succeeded, "{:?}", job.error);
    let raster = manager.inspect_raster(download_id).await.unwrap();
    assert_eq!(raster.sha256, job.sha256.clone().unwrap());
    let crop = manager
        .agent_clip_plan(
            &session,
            download_id,
            [-122.46, 37.76, -122.45, 37.77],
            "Agent live SCL crop",
        )
        .await
        .unwrap();
    let committed = manager
        .approve_agent_plan(
            &session,
            crop["planId"].as_str().unwrap(),
            crop["planHash"].as_str().unwrap(),
        )
        .await
        .unwrap();
    let crop_id = committed["jobs"][0]["id"].as_str().unwrap();
    let output = manager.wait(crop_id).await.unwrap();
    assert_eq!(output.status, JobStatus::Succeeded, "{:?}", output.error);
    let inspected = manager.inspect_raster(crop_id).await.unwrap();
    assert!(inspected.width > 0 && inspected.height > 0);
    let receipt = json!({"schemaVersion":"geod-agent-acquisition-acceptance/v1","search":search,"plan":plan,"downloadId":download_id,"downloadSha256":job.sha256,"downloadBytes":job.bytes_downloaded,"cropPlan":crop,"cropId":crop_id,"cropSha256":output.sha256,"cropBytes":output.bytes_downloaded,"modelCalls":0,"fixture":false});
    tokio::fs::write(
        directory.parent().unwrap().join("native-acquisition.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .await
    .unwrap();
    manager.shutdown().await.unwrap();
    drop(manager);
    let manager = JobManager::open(&directory).await.unwrap();
    let again = manager
        .approve_agent_plan(
            &session,
            plan["planId"].as_str().unwrap(),
            plan["planHash"].as_str().unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(again["jobs"][0]["id"], download_id);
    assert_eq!(manager.list().await.len(), 2);
    manager.shutdown().await.unwrap();
}
