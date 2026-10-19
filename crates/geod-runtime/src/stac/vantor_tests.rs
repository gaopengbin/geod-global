use super::*;
fn archive(root: &Path) -> (Record, Value, Value, ItemSnapshot) {
    std::fs::create_dir_all(root.join("stac")).unwrap();
    let index: Value =
        serde_json::from_str(include_str!("../datasets/vantor-reviewed-index.json")).unwrap();
    let item = index["document"].clone();
    let c = index["collection"].clone();
    let p=crate::datasets::resolve(serde_json::from_value(json!({"datasetId":"vantor-open","bounds":[85.4,28.2,85.45,28.25],"year":2026,"month":9,"layer":"visual"})).unwrap()).unwrap();
    let s = &p.sources[0];
    let proof = crate::datasets::SourceProvenance {
        schema_version: 2,
        dataset: p.dataset.clone(),
        request: p.request.clone(),
        layer: p.dataset.layers[0].clone(),
        source_id: s.id.clone(),
        source_title: s.title.clone(),
        source_bounds: s.bounds,
        declaration_kind:
            "reviewed-client-preset; publisher file bytes and embedded metadata verified separately"
                .into(),
        catalog_raster: None,
    };
    let mut docs = vec![];
    for (url, v) in [
        (index["itemUrl"].as_str().unwrap(), &item),
        (index["collectionUrl"].as_str().unwrap(), &c),
    ] {
        docs.push(DocumentReceipt {
            url: url.into(),
            sha256: immutable(root, "document", &serde_json::to_vec(v).unwrap()).unwrap(),
        });
    }
    let r = Record {
        version: 1,
        connection_id: Uuid::from_u128(20261014).to_string(),
        service_name: s.title.clone(),
        kind: "item".into(),
        document_url: index["itemUrl"].as_str().unwrap().into(),
        document_request: None,
        document_sha256: docs[0].sha256.clone(),
        item_index: None,
        retrieved_at: "2026-10-14T00:00:00Z".into(),
        metadata_documents: docs,
        search: None,
        reviewed_dataset: Some(proof),
        asset_transports: vec![],
    };
    let snap = save_snapshot(root, r.clone(), &item).unwrap();
    (r, item, c, snap)
}
#[test]
fn finite_visual_source_keeps_original_metadata_and_verified_collection_receipt() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let (r, item, c, snap) = archive(&root);
    let (reopened, again) = record(&root, &snap.id).unwrap();
    assert_eq!(
        again.datetime.as_deref(),
        Some("2026-09-07T05:07:53.249593Z")
    );
    assert_eq!(again.provenance.collection, Some(c));
    assert_eq!(again.properties, item["properties"]);
    let visual = again.assets.iter().find(|v| v.key == "visual").unwrap();
    assert!(visual.eligible);
    assert_eq!(visual.metadata, item["assets"]["visual"]);
    assert_eq!(visual.roles, vec!["visual"]);
    assert_eq!(reopened.metadata_documents, r.metadata_documents);
    let pin = SourcePin {
        snapshot_id: snap.id,
        asset_key: "visual".into(),
    };
    let selected = resolve(&root, &pin).unwrap();
    let mut job = crate::new_download_job(crate::CreateJobRequest {
        item_id: selected.item_id,
        asset_key: "stac_asset".into(),
        href: selected.href,
        media_type: selected.media_type,
        title: Some(selected.title),
    });
    job.source = selected.service_name;
    job.stac_source = Some(pin);
    verify_transfer(
        &root,
        &job,
        29_534_465,
        crate::datasets::vantor_open::ORIGINAL_SHA256,
    )
    .unwrap();
    assert!(verify_transfer(
        &root,
        &job,
        29_534_464,
        crate::datasets::vantor_open::ORIGINAL_SHA256
    )
    .is_err());
    assert!(verify_transfer(&root, &job, 29_534_465, &"0".repeat(64)).is_err());
}
#[test]
fn visual_profile_permission_cannot_be_forged_or_drop_the_original_licence_collection() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let (r, item, _, snap) = archive(&root);
    let mut changed = r.clone();
    changed.metadata_documents.truncate(1);
    assert!(save_snapshot(&root, changed, &item).is_err());
    let mut changed = r.clone();
    changed.reviewed_dataset.as_mut().unwrap().layer.unit = "reflectance".into();
    assert!(save_snapshot(&root, changed, &item).is_err());
    let mut item_changed = item.clone();
    item_changed["assets"]["visual"]["roles"] = json!(["data"]);
    assert!(save_snapshot(&root, r.clone(), &item_changed).is_err());
    let mut c: Value =
        serde_json::from_str::<Value>(include_str!("../datasets/vantor-reviewed-index.json"))
            .unwrap()["collection"]
            .clone();
    c["license"] = json!("CC-BY-4.0");
    let mut changed = r.clone();
    changed.metadata_documents[1].sha256 =
        immutable(&root, "document", &serde_json::to_vec(&c).unwrap()).unwrap();
    assert!(save_snapshot(&root, changed, &item).is_err());
    record(&root, &snap.id).unwrap();
}
