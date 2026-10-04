use super::*;
fn receipt() -> PageReceipt {
    PageReceipt {
        url: "https://example.com/api/collections/lakes/items?f=json&bbox=0,0,10,10&limit=2".into(),
        sha256: "a".repeat(64),
        bytes: 200,
        returned: 0,
        parameters: None,
    }
}
fn feature(id: Value) -> Value {
    json!({"type":"Feature","id":id,"properties":{"name":"Lake","height":3},"geometry":{"type":"Point","coordinates":[1,2,3]}})
}
fn page(features: Vec<Value>, matched: usize) -> Value {
    json!({"type":"FeatureCollection","numberReturned":features.len(),"numberMatched":matched,"features":features})
}
#[test]
fn public_service_policy_and_page_scope() {
    for raw in [
        "http://example.com/api",
        "https://user:secret@example.com/api",
        "https://localhost/a",
        "https://127.0.0.1/a",
        "https://10.0.0.1/a",
        "https://example.local/a",
        "https://example.com:444/api",
    ] {
        assert!(public_url(raw).is_err(), "{raw}");
    }
    for raw in [
        "100.64.0.1",
        "198.18.0.1",
        "192.0.0.1",
        "224.0.0.1",
        "::1",
        "fc00::1",
        "2001:db8::1",
    ] {
        assert!(!public_ip(raw.parse().unwrap()), "{raw}");
    }
    assert!(public_ip("8.8.8.8".parse().unwrap()));
    let root = public_url("https://example.com/api").unwrap();
    let current = Url::parse(&receipt().url).unwrap();
    assert!(next_url(
        &root,
        &current,
        "?f=json&bbox=0,0,10,10&limit=2&offset=2",
        Some([0., 0., 10., 10.])
    )
    .is_ok());
    for raw in [
        "https://other.example/api/collections/lakes/items?bbox=0,0,10,10",
        "/api2/collections/lakes/items?bbox=0,0,10,10",
        "?offset=2",
        "?bbox=0,0,11,10",
        "?bbox=0,0,10,10&token=secret",
        "?bbox=0,0,10,10&bbox=1,1,2,2",
        "?bbox=0,0,10,10&f=html",
    ] {
        assert!(
            next_url(&root, &current, raw, Some([0., 0., 10., 10.])).is_err(),
            "{raw}"
        );
    }
}
#[test]
fn assembly_checks_complete_pagination_and_conflicting_ids() {
    let mut a = Assembly::default();
    let first = feature(json!(1));
    let mut p = page(vec![first.clone()], 2);
    p["links"] =
        json!([{"rel":"next","type":"application/geo+json","href":"?bbox=0,0,10,10&offset=1"}]);
    assert!(a.append(p, receipt()).unwrap().is_some());
    assert!(a.complete().is_err());
    a.append(page(vec![first, feature(json!("1"))], 2), receipt())
        .unwrap();
    a.complete().unwrap();
    assert_eq!(a.features.len(), 2);
    let mut changed = feature(json!(1));
    changed["properties"]["name"] = json!("Changed");
    assert!(a
        .append(page(vec![changed], 2), receipt())
        .unwrap_err()
        .contains("conflicting"));
    assert!(a
        .append(page(vec![], 3), receipt())
        .unwrap_err()
        .contains("changed"));
}
#[test]
fn empty_page_next_is_not_treated_as_completion_and_counts_are_checked() {
    let mut a = Assembly::default();
    let mut empty = page(vec![], 1);
    empty["links"] =
        json!([{"rel":"next","type":"application/geo+json","href":"?bbox=0,0,10,10&offset=1"}]);
    assert!(a.append(empty, receipt()).unwrap().is_some());
    assert!(a.complete().is_err());
    let mut wrong = page(vec![feature(json!(1))], 1);
    wrong["numberReturned"] = json!(2);
    assert!(a.append(wrong, receipt()).is_err());
    let mut big = page(vec![], vector::MAX_FEATURES + 1);
    assert!(a.append(big.clone(), receipt()).is_err());
    big["numberMatched"] = json!("unknown");
    a.append(big, receipt()).unwrap();
}
#[test]
fn collections_follow_advertised_geojson_and_do_not_infer_dataset_license() {
    let root = public_url("https://example.com/api").unwrap();
    let c = json!({"id":"lakes","title":"Lakes","links":[{"rel":"items","type":"application/geo+json","href":"/api/collections/lakes/items?f=json"}]});
    assert!(collection(&root, &c)
        .unwrap()
        .unwrap()
        .license_links
        .is_empty());
    let mut coverage = c.clone();
    coverage["itemType"] = json!("coverage");
    assert!(collection(&root, &coverage).unwrap().is_none());
    let mut invalid = c;
    invalid["links"][0]["href"] = json!("https://other.example/items");
    assert!(collection(&root, &invalid).is_err());
}
#[tokio::test]
async fn snapshot_persists_provenance_and_untrusted_import_cannot_promote_it() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let mut p = receipt();
    p.returned = 1;
    let source = Provenance {
        service_url: "https://example.com/api".into(),
        service_name: "Test service".into(),
        collection_id: "lakes".into(),
        collection_title: "Lakes".into(),
        license_links: vec![],
        requested_bounds: [0., 0., 10., 10.],
        area_geometry: None,
        requested_at: now(),
        pages: vec![p],
        number_matched: Some(1),
        feature_count: 1,
        selection: "bbox-full-features".into(),
        arcgis: None,
        wfs: None,
    };
    let data =
        json!({"type":"FeatureCollection","features":[feature(json!(1))],"geodSource":source});
    let request = vector::ImportVectorRequest {
        name: "Lakes".into(),
        text: serde_json::to_string(&data).unwrap(),
    };
    let untrusted = manager.import_vector(request.clone()).await.unwrap();
    assert!(untrusted.remote_source.is_none());
    let asset = manager
        .import_vector_source(request.clone(), Some(source.clone()))
        .await
        .unwrap();
    assert_eq!(
        manager.inspect_vector(&asset.id).await.unwrap().geojson,
        data
    );
    let mut invalid = source;
    invalid.feature_count = 2;
    assert!(manager
        .import_vector_source(request, Some(invalid))
        .await
        .is_err());
    let id = asset.id;
    drop(manager);
    let manager = JobManager::open(dir.path()).await.unwrap();
    assert!(manager
        .inspect_vector(&id)
        .await
        .unwrap()
        .asset
        .remote_source
        .is_some());
}
#[tokio::test]
async fn registry_and_query_validate_before_network_and_removal_retains_files() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let req: QueryRequest = serde_json::from_value(
        json!({"serviceId":"unknown","collectionId":"lakes","bounds":[10,0,0,10]}),
    )
    .unwrap();
    assert!(manager
        .query_features(req)
        .await
        .unwrap_err()
        .contains("region"));
    let mut service = FeatureService {
        id: Uuid::new_v4().to_string(),
        name: "Demo".into(),
        url: "https://example.com/api".into(),
        title: "Demo".into(),
        collections: vec![],
        connected_at: now(),
        arcgis: None,
        wfs: None,
        overpass: None,
    };
    let records = BTreeMap::from([(service.id.clone(), service.clone())]);
    manager.persist_feature_services(&records).await.unwrap();
    assert_eq!(load(dir.path()).await.unwrap(), records);
    manager.inner.feature_services.lock().await.extend(records);
    manager.forget_feature_service(&service.id).await.unwrap();
    assert!(load(dir.path()).await.unwrap().is_empty());
    service.url = "https://127.0.0.1/api".into();
    manager
        .persist_feature_services(&BTreeMap::from([(service.id.clone(), service)]))
        .await
        .unwrap();
    assert!(load(dir.path()).await.is_err());
}
