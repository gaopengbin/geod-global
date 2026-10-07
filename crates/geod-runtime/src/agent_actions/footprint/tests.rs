use super::*;
fn scope(target: Option<PolygonGeometry>, parts: Vec<PolygonGeometry>) -> Scope {
    Scope {
        bounds: [0.0, 0.0, 4.0, 4.0],
        geometry: target,
        scenes: parts
            .into_iter()
            .enumerate()
            .map(|(i, g)| (i.to_string(), Some(g)))
            .collect(),
    }
}
#[test]
fn bounding_box_intersection_and_count_cannot_prove_coverage() {
    let triangle =
        PolygonGeometry::Polygon(vec![vec![[0.0, 0.0], [4.0, 0.0], [0.0, 4.0], [0.0, 0.0]]]);
    let s = scope(None, vec![triangle.clone(), triangle]);
    let result = report(&s).unwrap();
    assert_eq!(result["status"], "partial");
    assert_eq!(result["coveredFraction"], 0.5);
    assert!(complete(&s).is_err());
    let s = scope(
        None,
        vec![
            rectangle([0.0, 0.0, 2.0, 4.0]),
            rectangle([2.0, 0.0, 4.0, 4.0]),
        ],
    );
    assert_eq!(report(&s).unwrap()["status"], "complete");
}
#[test]
fn polygon_holes_and_separate_islands_use_the_actual_target() {
    let PolygonGeometry::Polygon(mut rings) = rectangle([0.0, 0.0, 4.0, 4.0]) else {
        unreachable!()
    };
    let PolygonGeometry::Polygon(hole) = rectangle([1.0, 1.0, 3.0, 3.0]) else {
        unreachable!()
    };
    rings.extend(hole);
    let target = PolygonGeometry::Polygon(rings);
    let pieces = vec![
        rectangle([0.0, 0.0, 4.0, 1.0]),
        rectangle([0.0, 3.0, 4.0, 4.0]),
        rectangle([0.0, 1.0, 1.0, 3.0]),
        rectangle([3.0, 1.0, 4.0, 3.0]),
    ];
    assert_eq!(
        report(&scope(Some(target), pieces.clone())).unwrap()["status"],
        "complete"
    );
    assert_eq!(
        report(&scope(None, pieces)).unwrap()["coveredFraction"],
        0.75
    );
    let PolygonGeometry::Polygon(a) = rectangle([0.0, 0.0, 1.0, 1.0]) else {
        unreachable!()
    };
    let PolygonGeometry::Polygon(b) = rectangle([3.0, 3.0, 4.0, 4.0]) else {
        unreachable!()
    };
    let target = PolygonGeometry::MultiPolygon(vec![a, b]);
    assert_eq!(
        report(&scope(Some(target), vec![rectangle([0.0, 0.0, 1.0, 1.0])])).unwrap()["status"],
        "partial"
    );
}
#[test]
fn missing_or_invalid_footprints_never_become_bounding_box_proof() {
    let mut s = scope(None, vec![rectangle([0.0, 0.0, 2.0, 4.0])]);
    s.scenes.push(("missing".into(), None));
    assert_eq!(report(&s).unwrap()["status"], "unknown");
    assert!(complete(&s).is_err());
    assert!(read_geometry(&json!({"bbox":[0,0,4,4]})).is_none());
    assert!(
        read_geometry(&json!({"bbox":[0,0,1,1],"geometry":rectangle([0.0,0.0,4.0,4.0])})).is_none()
    );
}
#[test]
fn a_query_envelope_cannot_silently_cut_off_the_requested_polygon() {
    let s = scope(
        Some(rectangle([-1.0, 0.0, 4.0, 4.0])),
        vec![rectangle([0.0, 0.0, 4.0, 4.0])],
    );
    assert!(report(&s).unwrap_err().contains("entire selected polygon"));
}
#[test]
fn visible_gaps_are_not_rounded_to_complete_and_overlap_is_not_double_counted() {
    let s = scope(None, vec![rectangle([0.0, 0.0, 3.99996, 4.0])]);
    assert_eq!(report(&s).unwrap()["status"], "partial");
    let s = scope(
        None,
        vec![
            rectangle([0.0, 0.0, 3.0, 4.0]),
            rectangle([1.0, 0.0, 4.0, 4.0]),
        ],
    );
    assert_eq!(report(&s).unwrap()["coveredFraction"], 1.0);
}
#[tokio::test]
async fn partial_selection_cannot_prepare_or_confirm_a_project_even_with_a_matching_bbox() {
    let temp = tempfile::tempdir().unwrap();
    let manager = JobManager::open(temp.path()).await.unwrap();
    let session = Uuid::new_v4().to_string();
    let mut receipt = super::super::tests::project_search_fixture(&manager, &session).await;
    let b = receipt.query.bounds;
    receipt.candidates[0].footprint = Some(rectangle([b[0], b[1], (b[0] + b[2]) / 2.0, b[3]]));
    write_record(&manager.inner.root, "searches", &receipt.id, &receipt)
        .await
        .unwrap();
    let id = receipt.candidates[0].item_id.clone();
    let result = check(
        &manager,
        &session,
        json!({"searchId":receipt.id,"itemIds":[id]}),
        None,
    )
    .await
    .unwrap();
    assert_eq!(result["coverage"]["status"], "partial");
    assert!(manager
        .agent_download_plan(&session, &receipt.id, vec![id.clone()], "visual")
        .await
        .is_err());
    assert!(manager
        .agent_project_plan(
            &session,
            &receipt.id,
            vec![id.clone()],
            Some("Incomplete".into()),
            None
        )
        .await
        .is_err());
    let mut request = crate::projects::CreateProjectRequest {
        name: "Bypass test".into(),
        bounds: b,
        geometry: None,
        scenes: vec![],
    };
    let c = &receipt.candidates[0];
    request.scenes.push(crate::projects::ProjectScene {
        item_id: id,
        date: c.date.clone(),
        cloud: c.cloud,
        crs: c.crs.clone(),
        grid_code: None,
        bbox: c.bounds,
        footprint: c.footprint.clone(),
        assets: c
            .assets
            .iter()
            .map(|a| {
                (
                    a.asset_key.clone(),
                    crate::projects::ProjectAsset {
                        href: a.href.clone(),
                        media_type: a.media_type.clone(),
                        raster_band: None,
                    },
                )
            })
            .collect(),
    });
    let draft = manager
        .save_agent_plan(
            &session,
            Action::Project {
                request,
                target: None,
                metadata_sha256: receipt.document_sha256,
            },
        )
        .await
        .unwrap();
    assert!(manager
        .approve_agent_plan(
            &session,
            draft["planId"].as_str().unwrap(),
            draft["planHash"].as_str().unwrap()
        )
        .await
        .is_err());
    assert!(manager.list_projects().await.is_empty());
    assert!(manager.list().await.is_empty());
}
#[test]
fn pagination_only_accepts_same_native_endpoint_and_never_claims_post_pages_exhausted() {
    let base =
        url::Url::parse("https://earth-search.aws.element84.com/v1/search?limit=20").unwrap();
    assert!(super::super::search_more::next_url(
        &json!({"links":[{"rel":"next","href":"https://evil.example/search"}]}),
        &base
    )
    .is_err());
    assert!(super::super::search_more::next_url(
        &json!({"links":[{"rel":"next","href":"?token=native"}]}),
        &base
    )
    .unwrap()
    .is_some());
    assert!(super::super::search_more::next_url(
        &json!({"links":[{"rel":"next","href":"?token=native","method":"POST"}]}),
        &base
    )
    .unwrap()
    .is_none());
}
#[tokio::test]
#[ignore = "Opt-in isolated Beijing boundary and actual Earth Search footprint coverage; no model, image transfer or user store."]
async fn live_beijing_administrative_area_coverage() {
    let home =
        std::path::PathBuf::from(std::env::var_os("GEOD_AREA_QA").expect("isolated QA directory"));
    std::fs::create_dir_all(&home).unwrap();
    let manager = JobManager::open(home.join("core")).await.unwrap();
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
    let regions = call(
        manager.clone(),
        &session,
        "geod_region_search",
        json!({"query":"北京","countryCode":"CHN","adminLevel":1}),
        None,
    )
    .await
    .unwrap();
    let source = regions["candidates"][0]["boundarySource"].clone();
    assert!(source.is_object());
    let boundary = call(
        manager.clone(),
        &session,
        "geod_boundary_read",
        source,
        None,
    )
    .await
    .unwrap();
    let b: [f64; 4] = serde_json::from_value(boundary["bounds"].clone()).unwrap();
    let end = Utc::now().date_naive();
    let query = SearchQuery {
        provider: "earth-search".into(),
        bounds: b,
        start: (end - chrono::Duration::days(29)).to_string(),
        end: end.to_string(),
        cloud_max: 4.999999999,
        limit: 20,
    };
    let mut catalog = manager.agent_search(&session, query).await.unwrap();
    let mut history = Vec::new();
    let mut result;
    loop {
        result = check(
            &manager,
            &session,
            json!({"searchId":catalog["searchId"],"boundary":boundary["boundary"]}),
            None,
        )
        .await
        .unwrap();
        history.push(result.clone());
        if result["coverage"]["status"] == "complete"
            || result["canContinue"] != true
            || history.len() >= 5
        {
            break;
        }
        catalog = super::super::search_more::more(
            &manager,
            &session,
            json!({"searchId":catalog["searchId"]}),
        )
        .await
        .unwrap();
    }
    let review = if result["coverage"]["status"] == "complete" {
        let plan=call(manager.clone(),&session,"geod_project_plan",json!({"searchId":result["searchId"],"itemIds":result["recommendedItemIds"],"name":"Beijing area coverage QA","boundary":boundary["boundary"]}),None).await.unwrap();
        assert_eq!(plan["areaCoverage"]["status"], "complete");
        assert_eq!(plan["areaCoverage"]["target"], "polygon");
        assert_eq!(plan["polygon"]["sha256"], boundary["boundary"]["sha256"]);
        assert_eq!(plan["status"], "pending");
        Some(plan)
    } else {
        None
    };
    assert!(manager.list_projects().await.is_empty());
    assert!(manager.list().await.is_empty());
    let proof = json!({"status":"passed","scope":"Actual source polygon and online STAC footprints; no model or download; incomplete coverage produces no review.","boundary":boundary,"attempts":history,"review":review,"projectsCreated":0,"jobsCreated":0});
    std::fs::write(
        home.join("result.json"),
        serde_json::to_vec_pretty(&proof).unwrap(),
    )
    .unwrap();
    println!(
        "{}",
        json!({"status":"passed","coverage":result["coverage"],"selected":result["recommendedItemIds"],"output":home.join("result.json")})
    );
    manager.shutdown().await.unwrap();
}
