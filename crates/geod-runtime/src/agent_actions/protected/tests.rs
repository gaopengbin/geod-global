use super::*;

fn query(provider: &str) -> SearchQuery {
    SearchQuery {
        provider: provider.into(),
        bounds: [-180.0, -90.0, 180.0, 90.0],
        start: "2000-01-01".into(),
        end: "2026-10-05".into(),
        cloud_max: 100.0,
        limit: 20,
    }
}
fn hls() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../prototype/public/samples/nasa-response.json"
    ))
    .unwrap()
}
async fn receipt(
    manager: &JobManager,
    session: &str,
    mut query: SearchQuery,
    mut candidates: Vec<Candidate>,
) -> String {
    // Isolated transfer/reuse fixtures describe one synthetic complete area,
    // not a global imagery acquisition. Production normalization stays strict.
    query.bounds = candidates[0].bounds;
    for c in &mut candidates {
        c.footprint = Some(super::super::footprint::rectangle(c.bounds));
    }
    let record = SearchReceipt {
        id: Uuid::new_v4().to_string(),
        session_id: session.into(),
        query,
        retrieved_at: now(),
        document_sha256: digest(b"explicit original catalog fixture"),
        candidates,
        more_available: false,
        next: None,
    };
    write_record(&manager.inner.root, "searches", &record.id, &record)
        .await
        .unwrap();
    record.id
}
#[test]
fn hls_original_band_identity_and_calibration_are_checked() {
    let q = query("nasa-earthdata");
    let value = hls();
    let candidates = normalize(&value, &q).unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].assets.len(), 3);
    for a in &candidates[0].assets {
        assert_eq!(catalog::provider(a).unwrap(), "nasa-earthdata");
        assert!(validate_pin(a, "nasa-earthdata", None).is_ok());
        assert!(validate_pin(a, "earth-search", None).is_err());
        assert!(validate_pin(
            a,
            "nasa-earthdata",
            Some(&RemotePin {
                bytes: 123,
                etag: "\"fake\"".into()
            })
        )
        .is_err());
    }
    assert_eq!(candidates[0].bands["red"].nodata, -9999.0);
    let mut wrong = value.clone();
    wrong["features"][0]["assets"]["B04"]["href"] = json!("https://unreviewed.example/file.tif");
    assert!(normalize(&wrong, &q).is_err());
    wrong = value.clone();
    wrong["features"][0]["collection"] = json!("HLSS30_2.0");
    assert!(normalize(&wrong, &q).is_err());
    let mut cloudy = q;
    cloudy.cloud_max = 0.0;
    assert!(normalize(&value, &cloudy).unwrap().is_empty());
}
#[test]
fn viirs_identity_period_and_three_platforms_stay_distinct() {
    for (provider, product) in [
        ("nasa-viirs-suomi", "VNP09A1"),
        ("nasa-viirs-noaa20", "VJ109A1"),
        ("nasa-viirs-noaa21", "VJ209A1"),
    ] {
        let id = format!("{product}.A2025177.h08v05.002.2025333224010");
        let item = json!({"id":id,"collection":format!("{product}_002"),"bbox":[-122.55,37.65,-122.45,37.85],"properties":{"datetime":null,"start_datetime":"2025-06-26T00:00:00Z","end_datetime":"2025-07-03T23:59:59Z"},"assets":{"2025333224010":{"href":format!("https://{}/lp-prod-protected/{product}.002/{id}/{id}.h5",p::nasa::HOST)}}});
        let value = json!({"type":"FeatureCollection","features":[item]});
        let c = normalize(&value, &query(provider)).unwrap();
        assert_eq!(catalog::provider(&c[0].assets[0]).unwrap(), provider);
        assert_eq!(c[0].end_date.as_deref(), Some("2025-07-03T23:59:59Z"));
        assert_eq!(c[0].crs.as_deref(), Some("VIIRS:Sinusoidal"));
        let mut wrong = value.clone();
        wrong["features"][0]["properties"]["end_datetime"] = json!("2025-07-04T23:59:59Z");
        assert!(normalize(&wrong, &query(provider)).is_err());
    }
}
#[test]
fn srtm_catalog_envelope_is_retained_without_replacing_the_native_grid() {
    // The bounded CMR public envelope is a metadata regression fixture. This
    // test does not download a protected HGT original or assert its pixels.
    let bounds = [-123.0002778, 36.9997222, -121.9997222, 38.0002778];
    let value = json!({"type":"FeatureCollection","features":[{
        "id":"N37W123.SRTMGL1.hgt", "collection":"SRTMGL1_003", "bbox":bounds,
        "properties":{"datetime":"2000-02-11T00:00:00.000Z"},
        "assets":{"hgt":{"href":"https://data.lpdaac.earthdatacloud.nasa.gov/lp-prod-protected/SRTMGL1.003/N37W123.SRTMGL1.hgt/N37W123.SRTMGL1.hgt.zip"}}
    }]});
    let mut q = query("nasa-srtm");
    q.start = "2025-06-01".into();
    q.end = "2025-06-30".into();
    let candidates = normalize(&value, &q).unwrap();
    assert_eq!(candidates[0].bounds, bounds);
    assert_eq!(
        candidates[0].date_role.as_deref(),
        Some("reference; not acquisition")
    );
    assert_eq!(
        catalog::provider(&candidates[0].assets[0]).unwrap(),
        "nasa-srtm"
    );
    let mut wrong = value;
    wrong["features"][0]["bbox"] = json!([-124.0, 37.0, -123.0, 38.0]);
    assert!(normalize(&wrong, &q).is_err());
}
#[test]
fn account_availability_is_not_product_entitlement() {
    let mut status = AccountStatus {
        provider: AccountProvider::Nasa,
        status: "connected",
        expires_at: Some((Utc::now() + chrono::Duration::minutes(30)).to_rfc3339()),
        verified_at: Some(now()),
    };
    assert!(usable(&status));
    status.status = "not-connected";
    assert!(!usable(&status));
    status.status = "saved";
    assert!(usable(&status));
    status.expires_at = Some((Utc::now() - chrono::Duration::minutes(1)).to_rfc3339());
    assert!(!usable(&status));
    status.expires_at = Some((Utc::now() + chrono::Duration::minutes(30)).to_rfc3339());
    status.verified_at = None;
    assert!(!usable(&status));
}
#[tokio::test]
async fn protected_completed_file_reuse_rehashes_without_requesting_authorization() {
    // Synthetic managed bytes test receipt integrity, not original-file
    // entitlement, TIFF decoding or an upstream protected transfer.
    let tmp = tempfile::TempDir::new().unwrap();
    let manager = JobManager::open(tmp.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let candidates = normalize(&hls(), &query("nasa-earthdata")).unwrap();
    let item = candidates[0].item_id.clone();
    let request = candidates[0]
        .assets
        .iter()
        .find(|a| a.asset_key == "red")
        .unwrap()
        .clone();
    let search = receipt(&manager, &session, query("nasa-earthdata"), candidates).await;
    let review = manager
        .agent_project_plan(
            &session,
            &search,
            vec![item],
            Some("Synthetic HLS reuse".into()),
            None,
        )
        .await
        .unwrap();
    let saved = manager
        .approve_agent_plan(
            &session,
            review["planId"].as_str().unwrap(),
            review["planHash"].as_str().unwrap(),
        )
        .await
        .unwrap();
    let project_id = saved["project"]["id"].as_str().unwrap();
    let bytes = b"synthetic original receipt integrity bytes";
    let mut job = crate::new_download_job(request);
    let assets = tmp.path().join("assets");
    std::fs::create_dir_all(&assets).unwrap();
    let path = assets.join(format!("{}.tif", job.id));
    std::fs::write(&path, bytes).unwrap();
    job.status = JobStatus::Succeeded;
    job.bytes_downloaded = bytes.len() as u64;
    job.total_bytes = Some(job.bytes_downloaded);
    job.sha256 = Some(digest(bytes));
    job.output_path = Some(path.to_string_lossy().into());
    {
        let mut store = manager.inner.store.lock().await;
        store.jobs.insert(job.id.clone(), job.clone());
        manager.persist(&store.jobs).await.unwrap();
    }
    assert!(verified_original(tmp.path(), &job).is_ok());
    let reused = manager
        .agent_project_download_plan(&session, project_id, "red", None)
        .await
        .unwrap();
    assert_eq!(reused["needsDownload"], false);
    assert_eq!(reused["jobs"][0]["id"], job.id);
    assert!(
        !authorization(&manager, "nasa-earthdata").await.unwrap()["downloadEnabled"]
            .as_bool()
            .unwrap()
    );
    let mut corrupted = *bytes;
    corrupted[7] ^= 1;
    std::fs::write(&path, corrupted).unwrap();
    assert!(verified_original(tmp.path(), &job).is_err());
    let missing = manager
        .agent_project_download_plan(&session, project_id, "red", None)
        .await
        .unwrap();
    assert_eq!(missing["kind"], "download");
    assert_eq!(missing["authorization"]["downloadEnabled"], false);
    assert!(manager
        .approve_agent_plan(
            &session,
            missing["planId"].as_str().unwrap(),
            missing["planHash"].as_str().unwrap()
        )
        .await
        .is_err());
    assert_eq!(manager.list().await.len(), 1);
    assert_eq!(std::fs::read(&path).unwrap(), corrupted);
}
#[test]
fn protected_integrity_check_accepts_native_zip_and_hdf_paths_and_rejects_wrong_receipts() {
    // Synthetic bytes intentionally do not claim ZIP/HDF product validity.
    // The native original worker verifies that format before Succeeded.
    for request in [
        crate::CreateJobRequest {item_id:"N37W123.SRTMGL1.hgt".into(),asset_key:"srtm".into(),href:"https://data.lpdaac.earthdatacloud.nasa.gov/lp-prod-protected/SRTMGL1.003/N37W123.SRTMGL1.hgt/N37W123.SRTMGL1.hgt.zip".into(),media_type:"application/zip".into(),title:None},
        crate::CreateJobRequest {item_id:"VNP09A1.A2025177.h08v05.002.2025333224010".into(),asset_key:"viirs".into(),href:"https://data.lpdaac.earthdatacloud.nasa.gov/lp-prod-protected/VNP09A1.002/VNP09A1.A2025177.h08v05.002.2025333224010/VNP09A1.A2025177.h08v05.002.2025333224010.h5".into(),media_type:"application/x-hdf5".into(),title:None},
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let mut job = crate::new_download_job(request);
        let assets = tmp.path().join("assets");
        std::fs::create_dir(&assets).unwrap();
        let path = assets.join(format!("{}.{}",job.id,crate::extension(&job.media_type).unwrap()));
        let bytes = b"synthetic native archive integrity bytes";
        std::fs::write(&path, bytes).unwrap();
        job.status = JobStatus::Succeeded;
        job.bytes_downloaded = bytes.len() as u64;
        job.total_bytes = Some(job.bytes_downloaded);
        job.sha256 = Some(digest(bytes));
        job.output_path = Some(path.to_string_lossy().into());
        assert!(verified_original(tmp.path(), &job).is_ok());
        let mut wrong = job.clone();
        wrong.kind = "raster_mosaic".into();
        assert!(verified_original(tmp.path(), &wrong).is_err());
        wrong = job.clone();
        wrong.total_bytes = Some(job.bytes_downloaded + 1);
        assert!(verified_original(tmp.path(), &wrong).is_err());
        wrong = job.clone();
        wrong.sha256 = Some("0".repeat(64));
        assert!(verified_original(tmp.path(), &wrong).is_err());
        let other = tmp.path().join("outside.bin");
        std::fs::write(&other,bytes).unwrap();
        wrong = job;
        wrong.output_path = Some(other.to_string_lossy().into());
        assert!(verified_original(tmp.path(), &wrong).is_err());
    }
}
#[tokio::test]
async fn project_review_and_edit_need_no_account_but_download_confirmation_is_blocked() {
    let tmp = tempfile::TempDir::new().unwrap();
    let manager = JobManager::open(tmp.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let candidates = normalize(&hls(), &query("nasa-earthdata")).unwrap();
    let ids = candidates
        .iter()
        .map(|c| c.item_id.clone())
        .collect::<Vec<_>>();
    let search = receipt(&manager, &session, query("nasa-earthdata"), candidates).await;
    let plan = manager
        .agent_project_plan(
            &session,
            &search,
            ids.clone(),
            Some("HLS test review".into()),
            None,
        )
        .await
        .unwrap();
    assert!(manager.list_projects().await.is_empty());
    assert_eq!(plan["project"]["saved"], false);
    assert_eq!(plan["project"]["committed"], false);
    let replacement = manager
        .revise_agent_plan(
            &session,
            plan["planId"].as_str().unwrap(),
            plan["planHash"].as_str().unwrap(),
            PlanRevision::Project {
                item_ids: ids.clone(),
                name: Some("HLS edited project".into()),
                bounds: Some([-122.55, 37.65, -122.45, 37.85]),
                keep_polygon: false,
            },
        )
        .await
        .unwrap();
    let id = replacement["planId"].as_str().unwrap();
    let hash = replacement["planHash"].as_str().unwrap();
    let committed = manager
        .approve_agent_plan(&session, id, hash)
        .await
        .unwrap();
    let project = committed["project"]["id"].as_str().unwrap();
    assert_eq!(committed["project"]["saved"], true);
    assert_eq!(committed["project"]["committed"], true);
    let download = manager
        .agent_project_download_plan(&session, project, "red", None)
        .await
        .unwrap();
    assert_eq!(download["authorization"]["status"], "not-connected");
    assert_eq!(download["project"]["saved"], true);
    assert_eq!(download["project"]["committed"], false);
    assert_eq!(download["status"], "pending");
    assert_eq!(download["authorization"]["downloadEnabled"], false);
    assert_eq!(download["authorization"]["entitlement"], "not-checked");
    assert!(download["expectedBytes"].is_null());
    let down_id = download["planId"].as_str().unwrap();
    let down_hash = download["planHash"].as_str().unwrap();
    assert!(manager
        .approve_agent_plan(&session, down_id, down_hash)
        .await
        .unwrap_err()
        .contains("Settings"));
    assert!(manager.list().await.is_empty());
    let original = tokio::fs::read(tmp.path().join("projects.json"))
        .await
        .unwrap();
    let accounts = serde_json::to_value(manager.provider_accounts().await).unwrap();
    drop(manager);
    let restored = JobManager::open(tmp.path()).await.unwrap();
    let recovered = restored.agent_plan_status(&session, down_id).await.unwrap();
    assert_eq!(recovered["project"]["saved"], true);
    assert_eq!(recovered["project"]["committed"], false);
    assert_eq!(recovered["status"], "pending");
    assert_eq!(recovered["planHash"], download["planHash"]);
    assert_eq!(recovered["authorization"]["downloadEnabled"], false);
    assert!(restored
        .approve_agent_plan(&session, down_id, down_hash)
        .await
        .is_err());
    assert!(restored.list().await.is_empty());
    assert_eq!(
        tokio::fs::read(tmp.path().join("projects.json"))
            .await
            .unwrap(),
        original
    );
    assert_eq!(
        serde_json::to_value(restored.provider_accounts().await).unwrap(),
        accounts
    );
}

#[tokio::test]
#[ignore = "Explicit fresh QA root and six bounded public catalog queries; no credential or protected file requests"]
async fn live_public_protected_catalog_reviews_and_authorization_gate() {
    let root = std::env::var("GEOD_AGENT_PROTECTED_CATALOG_QA").expect("Explicit isolated QA root");
    let input =
        std::env::var("GEOD_AGENT_PROTECTED_CATALOG_INPUT").expect("Explicit bounded query JSON");
    let queries: Vec<SearchQuery> =
        serde_json::from_str(&std::fs::read_to_string(input).unwrap()).unwrap();
    assert_eq!(queries.len(), 6);
    assert_eq!(
        queries
            .iter()
            .map(|q| q.provider.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "copernicus",
            "nasa-earthdata",
            "nasa-srtm",
            "nasa-viirs-suomi",
            "nasa-viirs-noaa20",
            "nasa-viirs-noaa21"
        ])
    );
    let root = PathBuf::from(root);
    assert!(!root.exists());
    std::fs::create_dir_all(&root).unwrap();
    let manager = JobManager::open(root.join("core")).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let accounts = serde_json::to_value(manager.provider_accounts().await).unwrap();
    let inventory = call(
        manager.clone(),
        &session,
        "geod_sources_list",
        json!({}),
        None,
    )
    .await
    .unwrap();
    assert_eq!(inventory["sources"].as_array().unwrap().len(), 15);
    let mut cases = Vec::new();
    for query in queries {
        let search = call(
            manager.clone(),
            &session,
            "geod_scene_search",
            serde_json::to_value(&query).unwrap(),
            None,
        )
        .await
        .unwrap();
        let id = search["scenes"][0]["itemId"]
            .as_str()
            .expect("Explicit QA area has an actual catalog item");
        let project=call(manager.clone(),&session,"geod_project_plan",json!({"searchId":search["searchId"],"itemIds":[id],"name":format!("Explicit {} metadata acceptance",query.provider)}),None).await.unwrap();
        assert_eq!(project["status"], "pending");
        let saved = manager
            .approve_agent_plan(
                &session,
                project["planId"].as_str().unwrap(),
                project["planHash"].as_str().unwrap(),
            )
            .await
            .unwrap();
        let asset = catalog::source(&query.provider).unwrap().keys[0];
        let download = call(
            manager.clone(),
            &session,
            "geod_project_download_plan",
            json!({"projectId":saved["project"]["id"],"assetKey":asset,"itemIds":[id]}),
            None,
        )
        .await
        .unwrap();
        assert_eq!(download["authorization"]["status"], "not-connected");
        assert_eq!(download["authorization"]["downloadEnabled"], false);
        assert!(download["expectedBytes"].is_null());
        assert_eq!(download["format"], format(&query.provider));
        let refused = manager
            .approve_agent_plan(
                &session,
                download["planId"].as_str().unwrap(),
                download["planHash"].as_str().unwrap(),
            )
            .await
            .unwrap_err();
        assert!(refused.contains("Settings"));
        assert!(manager.list().await.is_empty());
        cases.push(json!({"provider":query.provider,"query":query,"search":search,"projectReview":project,"savedProject":saved,"downloadReview":download,"nativeConfirmationRefused":true,"originalFileTransferred":false}));
    }
    assert_eq!(manager.list_projects().await.len(), 6);
    assert_eq!(
        serde_json::to_value(manager.provider_accounts().await).unwrap(),
        accounts
    );
    let before = std::fs::read(root.join("core/projects.json")).unwrap();
    drop(manager);
    let restored = JobManager::open(root.join("core")).await.unwrap();
    for case in &cases {
        let download = &case["downloadReview"];
        let recovered = restored
            .agent_plan_status(&session, download["planId"].as_str().unwrap())
            .await
            .unwrap();
        assert_eq!(recovered["planHash"], download["planHash"]);
        assert_eq!(recovered["authorization"]["downloadEnabled"], false);
        assert!(restored
            .approve_agent_plan(
                &session,
                download["planId"].as_str().unwrap(),
                download["planHash"].as_str().unwrap()
            )
            .await
            .is_err());
    }
    assert!(restored.list().await.is_empty());
    assert_eq!(
        std::fs::read(root.join("core/projects.json")).unwrap(),
        before
    );
    let report = json!({"schema":"geod-agent-protected-catalog-acceptance/v1","status":"passed","sessionId":session,"inventory":inventory,"cases":cases,"jobCount":0,"projectCount":6,"restarted":true,"publicMetadataUsed":true,"syntheticCatalog":false,"modelUsed":false,"protectedOriginalDownloaded":false,"credentialVaultWritten":false,"usedUserDesktop":false,"published":false});
    std::fs::write(
        root.join("native-acceptance.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    restored.shutdown().await.unwrap();
}
