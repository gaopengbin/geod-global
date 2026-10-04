use super::*;

fn item_value(href: &str) -> Value {
    json!({"stac_version":"1.0.0","type":"Feature","id":"arbitrary:item/一","collection":"example","bbox":[13.0,52.0,14.0,53.0],"geometry":{"type":"Polygon","coordinates":[[[13.0,52.0],[14.0,52.0],[14.0,53.0],[13.0,53.0],[13.0,52.0]]]},"properties":{"datetime":"2024-01-01T00:00:00Z"},"links":[],"assets":{"red":{"href":href,"type":"image/tiff; application=geotiff; profile=cloud-optimized","roles":["data"],"raster:bands":[{"scale":0.1,"unit":"source-declared-unit"}]}}})
}

#[test]
fn immutable_source_retains_arbitrary_identity_and_unknown_science_without_aliasing() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let pin = fixture_snapshot(&root, "https://example.com/source.tif");
    let selected = resolve(&root, &pin).unwrap();
    assert_eq!(selected.item_id, "arbitrary:item/一");
    assert_eq!(selected.asset_key, "red");
    assert_eq!(
        selected.media_type,
        "image/tiff; application=geotiff; profile=cloud-optimized"
    );
    let (_, snapshot) = record(&root, &pin.snapshot_id).unwrap();
    assert_eq!(snapshot.assets[0].metadata["raster:bands"][0]["scale"], 0.1);
    assert_eq!(snapshot.temporal_status, "instant");
    let mut job = crate::new_download_job(crate::CreateJobRequest {
        item_id: selected.item_id,
        asset_key: "stac_asset".into(),
        href: selected.href,
        media_type: selected.media_type,
        title: Some(selected.title),
    });
    job.source = selected.service_name;
    job.stac_source = Some(pin.clone());
    validate_job(&root, &job).unwrap();
    job.asset_key = "red".into();
    assert!(validate_job(&root, &job).is_err());
    let path = directory(&root)
        .unwrap()
        .join(format!("document-{}.json", snapshot.document_sha256));
    std::fs::write(path, b"{}").unwrap();
    assert!(resolve(&root, &pin).is_err());
}
#[test]
fn public_source_policy_rejects_private_credentialed_signed_urls_and_keeps_s3_unsupported() {
    for url in [
        "http://example.com/a.tif",
        "https://127.0.0.1/a.tif",
        "https://example.com/a.tif?X-Amz-Signature=secret",
        "https://user:password@example.com/a.tif",
        "https://example.com/a.tif?sig=secret",
    ] {
        assert!(public_url(url).is_err(), "{url}");
    }
    let value = item_value("s3://public-bucket/data.tif");
    let rec = test_record(&serde_json::to_vec(&value).unwrap());
    let snapshot = item(&rec, &"a".repeat(64), &value).unwrap();
    assert!(!snapshot.assets[0].eligible);
    assert_eq!(snapshot.assets[0].href, "s3://public-bucket/data.tif");
    let signed = item_value("https://example.com/a.tif?sig=secret");
    assert!(json_document(&serde_json::to_vec(&signed).unwrap()).is_err());
}
#[test]
fn missing_dates_are_explicit_and_partial_intervals_fail() {
    let mut value = item_value("https://example.com/a.tif");
    value["properties"]["datetime"] = Value::Null;
    let rec = test_record(&serde_json::to_vec(&value).unwrap());
    let snapshot = item(&rec, &"a".repeat(64), &value).unwrap();
    assert_eq!(snapshot.temporal_status, "missing");
    assert!(snapshot.datetime.is_none());
    assert!(!snapshot.warnings.is_empty());
    value["properties"]["start_datetime"] = json!("2024-01-01T00:00:00Z");
    assert!(item(&rec, &"a".repeat(64), &value).is_err());
}
#[test]
fn checksum_claims_never_pass_as_transfer_verification_without_matching_bytes() {
    assert_eq!(
        checksum_declaration(&json!({"file:checksum":format!("1220{}","a".repeat(64))})).unwrap(),
        Some("a".repeat(64))
    );
    assert!(checksum_declaration(&json!({"file:checksum":"md5:abcd"})).is_err());
    assert!(checksum_declaration(&json!({"file:size":0})).is_err());
}
#[test]
fn pagination_rejects_foreign_origin_post_links_and_invalid_temporal_bounds() {
    let base = Url::parse("https://example.com/stac/search").unwrap();
    assert!(link(
        &json!({"links":[{"rel":"next","href":"https://elsewhere.example/search"}]}),
        "next",
        &base
    )
    .is_err());
    assert!(link(
        &json!({"links":[{"rel":"next","href":"?page=2","method":"POST","body":{}}]}),
        "next",
        &base
    )
    .is_err());
    assert_eq!(
        link(
            &json!({"links":[{"rel":"next","href":"?page=2"}]}),
            "next",
            &base
        )
        .unwrap()
        .unwrap()
        .as_str(),
        "https://example.com/stac/search?page=2"
    );
    assert!(validate_datetime("2024-01-02T00:00:00Z/2024-01-01T00:00:00Z").is_err());
    assert!(validate_datetime("../..").is_err());
    assert!(validate_datetime("2024-01-01T00:00:00Z/..").is_ok());
}
fn test_record(bytes: &[u8]) -> Record {
    Record {
        version: 1,
        connection_id: "00000000-0000-4000-8000-000000000001".into(),
        service_name: "Custom fixture".into(),
        kind: "item".into(),
        document_url: "https://example.com/stac/item.json".into(),
        document_request: None,
        document_sha256: hash(bytes),
        item_index: None,
        retrieved_at: "2026-10-02T00:00:00Z".into(),
        metadata_documents: vec![],
        search: None,
    }
}

#[test]
fn saved_post_page_without_item_self_link_replays_exact_body_and_get_self_takes_precedence() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::create_dir_all(root.join("stac")).unwrap();
    let raw = json!({"type":"FeatureCollection","features":[item_value("https://example.com/source.tif")],"links":[]});
    let bytes = serde_json::to_vec(&raw).unwrap();
    let mut saved = test_record(&bytes);
    saved.kind = "api".into();
    saved.document_url = "https://example.com/search".into();
    saved.item_index = Some(0);
    saved.search = Some(SearchRequest {
        connection_id: saved.connection_id.clone(),
        collection_id: "example".into(),
        bounds: [13., 52., 14., 53.],
        datetime: None,
        limit: Some(2),
        cursor: None,
    });
    let request = MetadataRequest {
        url: saved.document_url.clone(),
        method: SearchMethod::Post,
        body: Some(
            json!({"bbox":[13.,52.,14.,53.],"collections":["example"],"limit":2,"token":"page-two"}),
        ),
    };
    saved.document_request = Some(request.clone());
    immutable(&root, "document", &bytes).unwrap();
    let snapshot = save_snapshot(&root, saved, &raw).unwrap();
    let (reopened, result) = record(&root, &snapshot.id).unwrap();
    assert_eq!(result.provenance.document_request, Some(request.clone()));
    assert_eq!(
        revalidation_request(&reopened, &raw["features"][0]).unwrap(),
        request
    );
    let mut self_item = raw["features"][0].clone();
    self_item["links"] = json!([{"rel":"self","href":"/items/one"}]);
    let request = revalidation_request(&reopened, &self_item).unwrap();
    assert_eq!(request.method, SearchMethod::Get);
    assert!(request.body.is_none());
    assert_eq!(request.url, "https://example.com/items/one");
    let mut legacy = reopened.clone();
    legacy.document_request = None;
    assert_eq!(
        revalidation_request(&legacy, &raw["features"][0]).unwrap(),
        MetadataRequest::get(&Url::parse(&legacy.document_url).unwrap())
    );
    let old_json = serde_json::to_value(&legacy).unwrap();
    assert!(old_json.get("documentRequest").is_none());
    assert!(serde_json::from_value::<Record>(old_json)
        .unwrap()
        .document_request
        .is_none());
}

#[tokio::test]
async fn legacy_get_registry_reopens_unchanged_and_rejects_forged_selected_method() {
    let temp = tempfile::tempdir().unwrap();
    let manager = JobManager::open(temp.path()).await.unwrap();
    let root = manager.storage_root().to_path_buf();
    let landing = json!({"stac_version":"1.0.0","type":"Catalog","links":[
        {"rel":"search","href":"/search"},{"rel":"search","href":"/search","method":"POST"},{"rel":"data","href":"/collections"}]});
    let collections = json!({"collections":[{"stac_version":"1.0.0","type":"Collection","id":"example","title":"Example","description":"Public originals","license":"CC0-1.0"}],"links":[]});
    let first = immutable(&root, "document", &serde_json::to_vec(&landing).unwrap()).unwrap();
    let second = immutable(
        &root,
        "document",
        &serde_json::to_vec(&collections).unwrap(),
    )
    .unwrap();
    let id = "00000000-0000-4000-8000-000000000001";
    let connection = Connection {
        id: id.into(),
        name: "Legacy GET".into(),
        url: "https://example.com/".into(),
        kind: "api".into(),
        connected_at: now(),
        collections: vec![collection(&collections["collections"][0]).unwrap()],
        capabilities: Capabilities {
            search_get: true,
            search_post: true,
        },
        snapshot_ids: vec![],
        search_url: Some("https://example.com/search".into()),
        search_method: SearchMethod::Get,
        metadata_sha256: vec![first.clone(), second.clone()],
        metadata_documents: vec![
            DocumentReceipt {
                url: "https://example.com/".into(),
                sha256: first,
            },
            DocumentReceipt {
                url: "https://example.com/collections".into(),
                sha256: second,
            },
        ],
    };
    let mut legacy = serde_json::to_value(&connection).unwrap();
    legacy.as_object_mut().unwrap().remove("searchMethod");
    let saved = serde_json::to_vec(&json!({"connections":{id:legacy}})).unwrap();
    let path = root.join("stac-connections.json");
    std::fs::write(&path, &saved).unwrap();
    drop(manager);
    let reopened = JobManager::open(&root).await.unwrap();
    assert_eq!(
        reopened.list_stac_connections().await,
        vec![connection.clone()]
    );
    assert_eq!(std::fs::read(&path).unwrap(), saved);
    let mut forged = connection;
    forged.search_url = Some("https://example.com/forged".into());
    assert!(validate_connection_documents(&root, &forged).is_err());
}
pub(super) fn fixture(root: &Path, href: &str) -> SourcePin {
    let canonical = root.canonicalize().unwrap();
    let root = canonical.as_path();
    std::fs::create_dir_all(root.join("stac")).unwrap();
    let value = item_value(href);
    let bytes = serde_json::to_vec(&value).unwrap();
    let rec = test_record(&bytes);
    immutable(root, "document", &bytes).unwrap();
    let snapshot = save_snapshot(root, rec, &value).unwrap();
    SourcePin {
        snapshot_id: snapshot.id,
        asset_key: "red".into(),
    }
}
