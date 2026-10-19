use super::*;

#[test]
fn selected_pc_original_is_bound_at_transfer_and_reopen_without_publisher_checksum_fields() {
    let items: Value = serde_json::from_str(include_str!(
        "../datasets/additional-pc-reviewed-items.json"
    ))
    .unwrap();
    let collections: Value = serde_json::from_str(include_str!(
        "../datasets/additional-pc-reviewed-collections.json"
    ))
    .unwrap();
    let cases = [
        (
            "planetary-more",
            "sst",
            1_096_226,
            "47b19bb75c95cdd302b1372a4d6a5a910c3e7213ccc013e6fee5f82f48646bfa",
        ),
        (
            "modis-more",
            "Burn_Date",
            112_419,
            crate::datasets::additional_pc::BURN_DATE_SHA256,
        ),
        (
            "modis-more",
            "QA",
            811_891,
            crate::datasets::additional_pc::BURN_QA_SHA256,
        ),
    ];
    for (id, key, bytes, sha) in cases {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("stac")).unwrap();
        let plan = crate::datasets::resolve(
            serde_json::from_value(json!({
                "datasetId": id, "layer": key, "bounds":[114.12,34.92,114.14,34.94],
                "year":2020,"month":9
            }))
            .unwrap(),
        )
        .unwrap();
        let source = &plan.sources[0];
        let proof = unit_dataset_proof(&plan, source);
        let entry = items
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["datasetId"] == id)
            .unwrap();
        let collection_entry = collections
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["datasetId"] == id)
            .unwrap();
        let raw = &entry["document"];
        let metadata = &raw["assets"][key];
        assert!(file_size_declaration(metadata).unwrap().is_none());
        assert!(checksum_declaration(metadata).unwrap().is_none());
        let href = metadata["href"].as_str().unwrap();
        let receipts = [entry, collection_entry]
            .into_iter()
            .map(|v| DocumentReceipt {
                url: v["url"].as_str().unwrap().into(),
                // Unit serialization hashes do not claim an HTTP transfer receipt.
                sha256: immutable(
                    &root,
                    "document",
                    &serde_json::to_vec(&v["document"]).unwrap(),
                )
                .unwrap(),
            })
            .collect::<Vec<_>>();
        let saved = Record {
            version: 1,
            connection_id: Uuid::new_v4().to_string(),
            service_name: source.stac.as_ref().unwrap().name.clone(),
            kind: "item".into(),
            document_url: entry["url"].as_str().unwrap().into(),
            document_request: None,
            document_sha256: receipts[0].sha256.clone(),
            item_index: None,
            retrieved_at: "2026-10-15T00:00:00Z".into(),
            metadata_documents: receipts,
            search: None,
            reviewed_dataset: Some(proof),
            asset_transports: vec![AssetTransport {
                asset_key: key.into(),
                original_href: href.into(),
                href: href.into(),
                adapter: "planetary-anonymous-sas".into(),
            }],
        };
        let snapshot = save_snapshot(&root, saved, raw).unwrap();
        let pin = SourcePin {
            snapshot_id: snapshot.id.clone(),
            asset_key: key.into(),
        };
        let asset = resolve(&root, &pin).unwrap();
        let mut job = crate::new_download_job(crate::CreateJobRequest {
            item_id: asset.item_id,
            asset_key: "stac_asset".into(),
            href: asset.href,
            media_type: asset.media_type,
            title: Some(asset.title),
        });
        job.source = asset.service_name;
        job.stac_source = Some(pin);
        // This shared boundary is also used by verified_original on reopening.
        verify_transfer(&root, &job, bytes, sha).unwrap();
        for (_, _, other_bytes, other_sha) in cases {
            if (bytes, sha) != (other_bytes, other_sha) {
                assert!(verify_transfer(&root, &job, other_bytes, other_sha).is_err());
            }
        }
        assert!(verify_transfer(&root, &job, bytes, &"a".repeat(64)).is_err());
        assert!(verify_transfer(&root, &job, bytes + 1, sha).is_err());
        let (_, restored) = record(&root, &snapshot.id).unwrap();
        assert_eq!(
            &restored
                .assets
                .iter()
                .find(|a| a.key == key)
                .unwrap()
                .metadata,
            metadata
        );
    }
}

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
fn oversize_declared_original_is_ineligible_before_selection_and_retains_raw_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::create_dir_all(root.join("stac")).unwrap();
    for declaration in ["file:size", "bdc:size"] {
        let mut value = item_value("https://example.com/large.tif");
        value["assets"]["red"][declaration] = json!(crate::MAX_ASSET_BYTES + 1);
        let original = value["assets"]["red"].clone();
        let bytes = serde_json::to_vec(&value).unwrap();
        let rec = test_record(&bytes);
        let snapshot = item(&rec, &"a".repeat(64), &value).unwrap();
        assert!(!snapshot.assets[0].eligible);
        assert_eq!(snapshot.assets[0].metadata, original);
        assert!(snapshot.assets[0]
            .reason
            .as_deref()
            .unwrap()
            .contains("512 MiB"));
        immutable(&root, "document", &bytes).unwrap();
        let snapshot = save_snapshot(&root, rec, &value).unwrap();
        let pin = SourcePin {
            snapshot_id: snapshot.id,
            asset_key: "red".into(),
        };
        assert!(resolve(&root, &pin).unwrap_err().contains("512 MiB"));
        value["assets"]["red"][declaration] = json!(crate::MAX_ASSET_BYTES);
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(
            item(&test_record(&bytes), &"a".repeat(64), &value)
                .unwrap()
                .assets[0]
                .eligible
        );
    }
}

#[test]
fn inpe_checksum_and_size_aliases_verify_known_original_and_preserve_publisher_fields() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::create_dir_all(root.join("stac")).unwrap();
    // Actual independently verified CBERS-4A MUX BAND5 declaration. The test
    // exercises metadata/transfer verification, not a simulated real download.
    let expected = "d24255698a18e8d751e91e9f7f02423ce685fde85785e2f34504911e6693bc59";
    let length = 32_374_526;
    let mut raw = item_value("https://example.com/cbers-band5.tif");
    raw["assets"]["red"]["checksum:multihash"] = json!(format!("1220{expected}"));
    raw["assets"]["red"]["bdc:size"] = json!(length);
    let bytes = serde_json::to_vec(&raw).unwrap();
    let receipt = test_record(&bytes);
    immutable(&root, "document", &bytes).unwrap();
    let snapshot = save_snapshot(&root, receipt, &raw).unwrap();
    assert_eq!(snapshot.assets[0].metadata, raw["assets"]["red"]);
    assert_eq!(document(&root, &hash(&bytes)).unwrap(), bytes);
    let pin = SourcePin {
        snapshot_id: snapshot.id,
        asset_key: "red".into(),
    };
    let asset = resolve(&root, &pin).unwrap();
    let mut job = crate::new_download_job(crate::CreateJobRequest {
        item_id: asset.item_id,
        asset_key: "stac_asset".into(),
        href: asset.href,
        media_type: asset.media_type,
        title: Some(asset.title),
    });
    job.source = asset.service_name;
    job.stac_source = Some(pin);
    verify_transfer(&root, &job, length, expected).unwrap();
    assert!(verify_transfer(&root, &job, length + 1, expected).is_err());
    assert!(verify_transfer(&root, &job, length, &"a".repeat(64)).is_err());
    let declarations = json!({"file:size":length,"bdc:size":length,"file:checksum":format!("1220{expected}"),"checksum:multihash":format!("1220{expected}")});
    assert_eq!(
        checksum_declaration(&declarations).unwrap().as_deref(),
        Some(expected)
    );
    assert_eq!(file_size_declaration(&declarations).unwrap(), Some(length));
}

#[test]
fn conflicting_or_unsupported_publisher_integrity_declarations_are_not_downloadable() {
    let mut source = json!({"file:size":8,"bdc:size":9});
    assert!(file_size_declaration(&source)
        .unwrap_err()
        .contains("conflict"));
    source = json!({"file:checksum":format!("1220{}","a".repeat(64)),"checksum:multihash":format!("1220{}","b".repeat(64))});
    assert!(checksum_declaration(&source)
        .unwrap_err()
        .contains("conflict"));
    for checksum in [
        json!("md5:abcd"),
        json!("1220abcd"),
        json!(format!("1220{}", "A".repeat(64))),
        json!("sha256:abcd"),
        json!(12),
        Value::Null,
    ] {
        assert!(checksum_declaration(&json!({"checksum:multihash":checksum})).is_err());
    }
    for size in [json!(0), json!(-1), json!(12.5), json!("12"), Value::Null] {
        assert!(file_size_declaration(&json!({"bdc:size":size})).is_err());
    }
    let mut raw = item_value("https://example.com/cbers-band5.tif");
    raw["assets"]["red"]["file:checksum"] = json!(format!("1220{}", "a".repeat(64)));
    raw["assets"]["red"]["checksum:multihash"] = json!(format!("1220{}", "b".repeat(64)));
    let bytes = serde_json::to_vec(&raw).unwrap();
    let snapshot = item(&test_record(&bytes), &hash(&bytes), &raw).unwrap();
    assert!(!snapshot.assets[0].eligible);
    assert!(snapshot.assets[0]
        .reason
        .as_ref()
        .unwrap()
        .contains("conflict"));
    assert_eq!(snapshot.assets[0].metadata, raw["assets"]["red"]);
}

fn api_query() -> SearchRequest {
    SearchRequest {
        connection_id: "00000000-0000-4000-8000-000000000001".into(),
        collection_id: "example".into(),
        bounds: [13., 52., 14., 53.],
        datetime: Some("2024-01-01T00:00:00Z/2024-01-31T23:59:59.999999Z".into()),
        limit: Some(20),
        cursor: None,
    }
}

#[test]
fn api_selection_checks_actual_date_and_aoi_not_only_recorded_query_labels() {
    let mut raw = item_value("https://example.com/source.tif");
    let bytes = serde_json::to_vec(&raw).unwrap();
    let record = test_record(&bytes);
    let query = api_query();
    let accepted = item(&record, &hash(&bytes), &raw).unwrap();
    validate_search_item(&accepted, &query).unwrap();
    raw["properties"]["datetime"] = json!("2023-01-15T00:00:00Z");
    assert!(
        validate_search_item(&item(&record, &hash(&bytes), &raw).unwrap(), &query)
            .unwrap_err()
            .contains("period")
    );
    // The UTC instant is in January despite its local February representation.
    raw["properties"]["datetime"] = json!("2024-02-01T00:00:00+01:00");
    validate_search_item(&item(&record, &hash(&bytes), &raw).unwrap(), &query).unwrap();
    raw["bbox"] = json!([116., 39., 117., 40.]);
    assert!(
        validate_search_item(&item(&record, &hash(&bytes), &raw).unwrap(), &query)
            .unwrap_err()
            .contains("area")
    );
    raw["bbox"] = Value::Null;
    assert!(
        validate_search_item(&item(&record, &hash(&bytes), &raw).unwrap(), &query)
            .unwrap_err()
            .contains("bounding box")
    );
    raw["bbox"] = json!([13., 52., 14., 53.]);
    raw["properties"]["datetime"] = Value::Null;
    assert!(
        validate_search_item(&item(&record, &hash(&bytes), &raw).unwrap(), &query)
            .unwrap_err()
            .contains("time or interval")
    );
    let mut no_time = query;
    no_time.datetime = None;
    validate_search_item(&item(&record, &hash(&bytes), &raw).unwrap(), &no_time).unwrap();
}

#[test]
fn annual_classification_interval_and_open_queries_use_temporal_intersection() {
    let mut raw = item_value("https://example.com/annual-classification.tif");
    raw["properties"]["datetime"] = Value::Null;
    raw["properties"]["start_datetime"] = json!("2024-01-01T00:00:00Z");
    raw["properties"]["end_datetime"] = json!("2025-01-01T00:00:00Z");
    let bytes = serde_json::to_vec(&raw).unwrap();
    let snapshot = item(&test_record(&bytes), &hash(&bytes), &raw).unwrap();
    let mut query = api_query();
    query.datetime = Some("2024-06-01T00:00:00Z/2024-06-30T23:59:59.999999Z".into());
    validate_search_item(&snapshot, &query).unwrap();
    query.datetime = Some("2024-01-01T00:00:00Z/2024-12-31T23:59:59.999999Z".into());
    validate_search_item(&snapshot, &query).unwrap();
    query.datetime = Some("2024-06-01T00:00:00Z".into());
    validate_search_item(&snapshot, &query).unwrap();
    query.datetime = Some("../2023-12-31T23:59:59Z".into());
    assert!(validate_search_item(&snapshot, &query).is_err());
    query.datetime = Some("2025-02-01T00:00:00Z/..".into());
    assert!(validate_search_item(&snapshot, &query).is_err());
    query.datetime = Some("../2024-06-01T00:00:00Z".into());
    validate_search_item(&snapshot, &query).unwrap();
}

#[test]
fn new_api_snapshot_guard_does_not_migrate_or_relabel_existing_source_receipts() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::create_dir_all(root.join("stac")).unwrap();
    let mut original = item_value("https://example.com/source.tif");
    original["properties"]["datetime"] = json!("2023-01-01T00:00:00Z");
    let page = json!({"type":"FeatureCollection","features":[original],"links":[]});
    let bytes = serde_json::to_vec(&page).unwrap();
    let mut receipt = test_record(&bytes);
    receipt.kind = "api".into();
    receipt.item_index = Some(0);
    receipt.search = Some(api_query());
    immutable(&root, "document", &bytes).unwrap();
    let saved = serde_json::to_vec(&receipt).unwrap();
    let id = immutable(&root, "snapshot", &saved).unwrap();
    let (_, reopened) = record(&root, &id).unwrap();
    assert_eq!(reopened.datetime.as_deref(), Some("2023-01-01T00:00:00Z"));
    assert_eq!(reopened.provenance.search.as_ref().unwrap().cursor, None);
    assert!(save_snapshot(&root, receipt, &page)
        .unwrap_err()
        .contains("period"));
    assert_eq!(
        read_file(
            &directory(&root)
                .unwrap()
                .join(format!("snapshot-{id}.json")),
            65536
        )
        .unwrap(),
        saved
    );
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
        reviewed_dataset: None,
        asset_transports: Vec::new(),
    }
}

#[test]
fn generic_binary_metadata_requires_complete_utf8_and_structured_stac_json() {
    validate_generic_metadata(
        &serde_json::to_vec(&item_value("https://example.com/original.tif")).unwrap(),
    )
    .unwrap();
    validate_generic_metadata(br#"{"type":"FeatureCollection","features":[],"links":[]}"#).unwrap();
    for bytes in [
        b"<html>Internal error</html>".as_slice(),
        b"{\"message\":\"ok\"}".as_slice(),
        b"{\"type\":\"Feature\",\"id\":\"fake\"}".as_slice(),
        b"\xff{}".as_slice(),
        b"{".as_slice(),
    ] {
        assert!(validate_generic_metadata(bytes).is_err());
    }
}

#[test]
fn reviewed_worldcereal_receipts_reopen_unchanged_documents_and_bind_transport_and_season() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::create_dir_all(root.join("stac")).unwrap();
    for layer in crate::datasets::list()
        .into_iter()
        .find(|dataset| dataset.id == "worldcereal")
        .unwrap()
        .layers
    {
        let plan = crate::datasets::resolve(
            serde_json::from_value(
                json!({"datasetId":"worldcereal","bounds":[57.5,-20.1,57.6,-20.],"layer":layer.id}),
            )
            .unwrap(),
        )
        .unwrap();
        let source = &plan.sources[0];
        let proof=crate::datasets::SourceProvenance{catalog_raster:None,schema_version:2,dataset:plan.dataset.clone(),request:plan.request.clone(),layer:layer.clone(),source_id:source.id.clone(),source_title:source.title.clone(),source_bounds:source.bounds,declaration_kind:"reviewed-client-preset; publisher file bytes and embedded metadata verified separately".into()};
        let (url, raw) = crate::datasets::worldcereal::reviewed_document(&layer.id);
        let bytes = serde_json::to_vec(&raw).unwrap();
        let sha = immutable(&root, "document", &bytes).unwrap();
        let mut receipt = test_record(&bytes);
        receipt.document_url = url.clone();
        receipt.metadata_documents = vec![DocumentReceipt {
            url: url.clone(),
            sha256: sha,
        }];
        let ordinary = item(&receipt, "ordinary", &raw).unwrap();
        assert!(ordinary
            .assets
            .iter()
            .all(|asset| !asset.eligible && asset.original_href.is_none()));
        receipt.asset_transports =
            crate::datasets::worldcereal::transports(&proof, &url, &raw).unwrap();
        receipt.reviewed_dataset = Some(proof.clone());
        let saved = save_snapshot(&root, receipt.clone(), &raw).unwrap();
        let (reopened_receipt, reopened) = record(&root, &saved.id).unwrap();
        assert_eq!(saved, reopened);
        assert_eq!(reopened.properties, raw["properties"]);
        assert_eq!(reopened.geometry, raw["geometry"]);
        assert_eq!(reopened.provenance.reviewed_dataset, Some(proof));
        assert!(reopened.provenance.collection.is_none());
        assert_eq!(
            document(&root, &reopened_receipt.document_sha256).unwrap(),
            bytes
        );
        for asset in &reopened.assets {
            assert!(asset.eligible);
            assert_eq!(asset.metadata, raw["assets"][&asset.key]);
            assert_eq!(
                asset.original_href.as_deref(),
                asset.metadata["href"].as_str()
            );
            assert_eq!(
                asset.transport_adapter.as_deref(),
                Some(crate::datasets::worldcereal::ADAPTER)
            );
            let selected = resolve(
                &root,
                &SourcePin {
                    snapshot_id: saved.id.clone(),
                    asset_key: asset.key.clone(),
                },
            )
            .unwrap();
            assert_eq!(selected.href, asset.href);
        }
        assert_eq!(revalidation_request(&receipt, &raw).unwrap().url, url);
        let mut corrupted = receipt.clone();
        corrupted.asset_transports[0].href = "https://unreviewed.test/original.tif".into();
        assert!(save_snapshot(&root, corrupted, &raw).is_err());
        let mut corrupted = receipt;
        corrupted.reviewed_dataset = None;
        assert!(save_snapshot(&root, corrupted, &raw).is_err());
    }
}

#[test]
fn new_annual_and_cgls_receipts_reopen_original_documents_and_separate_product_contracts() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::create_dir_all(root.join("stac")).unwrap();
    for id in [
        "deafrica-fractional-cover",
        "deafrica-wofs-annual",
        "cgls-landcover",
    ] {
        let bounds = if id == "cgls-landcover" {
            json!([32.85, -0.45, 32.95, -0.35])
        } else {
            json!([36.84, -1.30, 36.86, -1.28])
        };
        let plan = crate::datasets::resolve(
            serde_json::from_value(json!({"datasetId":id,"bounds":bounds})).unwrap(),
        )
        .unwrap();
        let source = &plan.sources[0];
        let proof = crate::datasets::SourceProvenance { schema_version:2,dataset:plan.dataset.clone(),request:plan.request.clone(),
            layer:plan.dataset.layers.iter().find(|l| Some(&l.id) == plan.request.layer.as_ref()).unwrap().clone(),
            source_id:source.id.clone(),source_title:source.title.clone(),source_bounds:source.bounds,
            declaration_kind:"reviewed-client-preset; publisher file bytes and embedded metadata verified separately".into(),catalog_raster:None };
        let url = source.stac.as_ref().unwrap().url.clone();
        let raw = if id == "cgls-landcover" {
            crate::datasets::cgls::reviewed_document(2019).1
        } else {
            crate::datasets::deafrica_annual::reviewed_document(id)
        };
        let bytes = serde_json::to_vec(&raw).unwrap();
        let sha = immutable(&root, "document", &bytes).unwrap();
        let mut receipt = test_record(&bytes);
        receipt.document_url = url.clone();
        receipt.metadata_documents = vec![DocumentReceipt {
            url: url.clone(),
            sha256: sha,
        }];
        receipt.asset_transports =
            crate::datasets::reviewed_item_transports(&proof, &url, &raw).unwrap();
        receipt.reviewed_dataset = Some(proof.clone());
        let saved = save_snapshot(&root, receipt.clone(), &raw).unwrap();
        let (reopened_receipt, reopened) = record(&root, &saved.id).unwrap();
        assert_eq!(saved, reopened);
        assert_eq!(reopened.properties, raw["properties"]);
        assert_eq!(reopened.geometry, raw["geometry"]);
        assert_eq!(reopened.provenance.reviewed_dataset, Some(proof));
        assert_eq!(
            document(&root, &reopened_receipt.document_sha256).unwrap(),
            bytes
        );
        assert_eq!(reopened.start_datetime, source.start_datetime);
        assert_eq!(reopened.end_datetime, source.end_datetime);
        assert_eq!(revalidation_request(&receipt, &raw).unwrap().url, url);
        assert!(!reopened
            .warnings
            .iter()
            .any(|w| w.contains("Seasonal category 254")));
        let eligible = reopened
            .assets
            .iter()
            .filter(|a| a.eligible)
            .collect::<Vec<_>>();
        if id == "cgls-landcover" {
            assert_eq!(eligible.len(), 1);
            assert_eq!(eligible[0].key, "permanentwater_cover_fraction");
        } else {
            assert_eq!(eligible.len(), plan.dataset.layers.len());
            assert!(receipt.asset_transports.is_empty());
            assert!(eligible
                .iter()
                .all(|a| a.original_href.is_none() && a.transport_adapter.is_none()));
        }
        for asset in eligible {
            assert_eq!(asset.metadata, raw["assets"][&asset.key]);
            let selected = resolve(
                &root,
                &SourcePin {
                    snapshot_id: saved.id.clone(),
                    asset_key: asset.key.clone(),
                },
            )
            .unwrap();
            assert_eq!(selected.href, asset.href);
        }
        let mut corrupted = receipt;
        corrupted.reviewed_dataset.as_mut().unwrap().source_title = "unreviewed replacement".into();
        assert!(save_snapshot(&root, corrupted, &raw).is_err());
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
        catalog_nodes: vec![],
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

#[test]
fn dataset_direct_receipt_preserves_release_license_units_and_true_tile_on_reopen() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::create_dir_all(root.join("stac")).unwrap();
    let plan = crate::datasets::resolve(crate::datasets::ResolveRequest {
        dataset_id: "worldcover-composites".into(),
        bounds: [10.1, 78.1, 10.2, 78.2],
        year: Some(2021),
        month: None,
        country: None,
        layer: Some("swir".into()),
        region: None,
        season: None,
    })
    .unwrap();
    let source = &plan.sources[0];
    let href = &source.stac.as_ref().unwrap().url;
    let proof = json!({"schemaVersion":1,"dataset":plan.dataset,"request":plan.request,"layer":plan.dataset.layers.iter().find(|l|l.id=="swir").unwrap(),"sourceId":source.id,"sourceTitle":source.title,"sourceBounds":source.bounds,"declarationKind":"reviewed-client-preset; publisher file bytes and embedded metadata verified separately"});
    let raw = json!({"kind":"direct-raster","href":href,"probeStatus":206,"contentLength":16,"contentRange":"bytes 0-15/2829765","headerHex":"49492a00","cogLayout":"not-validated","datasetPreset":proof});
    let bytes = serde_json::to_vec(&raw).unwrap();
    let mut saved = test_record(&bytes);
    saved.kind = "raster".into();
    saved.document_url = href.clone();
    immutable(&root, "document", &bytes).unwrap();
    let snapshot = save_snapshot(&root, saved, &raw).unwrap();
    let (_, reloaded) = record(&root, &snapshot.id).unwrap();
    assert_eq!(reloaded.bbox, Some([10., 78., 11., 79.]));
    assert_eq!(
        reloaded.properties["datasetPreset"]["layer"]["scale"],
        json!(0.004)
    );
    assert_eq!(
        reloaded.properties["datasetPreset"]["layer"]["nodata"],
        json!(255.)
    );
    assert_eq!(
        reloaded.properties["datasetPreset"]["dataset"]["license"],
        json!("CC BY 4.0")
    );
    assert_eq!(reloaded.datetime, None);
    assert_eq!(reloaded.temporal_status, "missing");
    let path = directory(&root)
        .unwrap()
        .join(format!("document-{}.json", reloaded.document_sha256));
    std::fs::write(path, b"{}").unwrap();
    assert!(record(&root, &snapshot.id).is_err());
}

fn capacity_fixture(root: &Path, index: u128, kind: &str) -> Connection {
    let id = Uuid::from_u128(index + 1).to_string();
    let href = format!("https://example.com/source-{index}.tif");
    let url = if kind == "raster" {
        href.clone()
    } else {
        format!("https://example.com/items/{index}.json")
    };
    let raw = if kind == "raster" {
        json!({"kind":"direct-raster","href":href,"probeStatus":206,"contentLength":16,"contentRange":"bytes 0-15/1024","headerHex":"49492a00","cogLayout":"not-validated"})
    } else {
        item_value(&href)
    };
    let bytes = serde_json::to_vec(&raw).unwrap();
    let sha = immutable(root, "document", &bytes).unwrap();
    let receipts = if kind == "raster" {
        vec![]
    } else {
        vec![DocumentReceipt {
            url: url.clone(),
            sha256: sha.clone(),
        }]
    };
    let record = Record {
        version: 1,
        connection_id: id.clone(),
        service_name: format!("Source {index}"),
        kind: kind.into(),
        document_url: url.clone(),
        document_request: None,
        document_sha256: sha.clone(),
        item_index: None,
        retrieved_at: "2026-10-12T00:00:00Z".into(),
        metadata_documents: receipts.clone(),
        search: None,
        reviewed_dataset: None,
        asset_transports: Vec::new(),
    };
    let snapshot = save_snapshot(root, record, &raw).unwrap();
    Connection {
        id,
        name: format!("Source {index}"),
        url,
        kind: kind.into(),
        connected_at: "2026-10-12T00:00:00Z".into(),
        collections: vec![],
        capabilities: Capabilities {
            search_get: false,
            search_post: false,
        },
        snapshot_ids: vec![snapshot.id],
        search_url: None,
        search_method: SearchMethod::Get,
        metadata_sha256: vec![sha],
        metadata_documents: receipts,
        catalog_nodes: vec![],
    }
}

#[tokio::test]
async fn connection_capacity_reopens_128_originals_and_rejects_129_without_mutation() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::create_dir_all(root.join("stac")).unwrap();
    let mut registry = Registry::default();
    for index in 0..128 {
        let connection = capacity_fixture(&root, index, "raster");
        admit_connection(&registry, &connection).unwrap();
        registry
            .connections
            .insert(connection.id.clone(), connection);
    }
    persist(&root, &registry).unwrap();
    let saved = std::fs::read(root.join("stac-connections.json")).unwrap();
    let reopened = load(&root).await.unwrap();
    assert_eq!(reopened.connections.len(), 128);
    let replacement = reopened.connections.values().next().unwrap().clone();
    admit_connection(&reopened, &replacement).unwrap();
    let candidate = capacity_fixture(&root, 128, "raster");
    assert!(admit_connection(&reopened, &candidate)
        .unwrap_err()
        .contains("128"));
    assert_eq!(
        std::fs::read(root.join("stac-connections.json")).unwrap(),
        saved
    );
    let pin = SourcePin {
        snapshot_id: replacement.snapshot_ids[0].clone(),
        asset_key: "raster".into(),
    };
    resolve(&root, &pin).unwrap();
    registry.connections.insert(candidate.id.clone(), candidate);
    persist(&root, &registry).unwrap();
    assert!(load(&root).await.unwrap_err().contains("128"));
    // Even an over-capacity registry does not erase the immutable selected file.
    resolve(&root, &pin).unwrap();
}

#[tokio::test]
async fn legacy_24_sources_keep_their_receipts_and_non_raster_cap_stays_24() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::create_dir_all(root.join("stac")).unwrap();
    let mut registry = Registry::default();
    for index in 0..24 {
        let connection = capacity_fixture(&root, index, "item");
        admit_connection(&registry, &connection).unwrap();
        registry
            .connections
            .insert(connection.id.clone(), connection);
    }
    persist(&root, &registry).unwrap();
    let saved = std::fs::read(root.join("stac-connections.json")).unwrap();
    let reopened = load(&root).await.unwrap();
    assert_eq!(reopened.connections.len(), 24);
    assert_eq!(
        std::fs::read(root.join("stac-connections.json")).unwrap(),
        saved
    );
    let candidate = capacity_fixture(&root, 24, "item");
    assert!(admit_connection(&reopened, &candidate)
        .unwrap_err()
        .contains("24"));
    let raster = capacity_fixture(&root, 25, "raster");
    admit_connection(&reopened, &raster).unwrap();
    registry.connections.insert(raster.id.clone(), raster);
    persist(&root, &registry).unwrap();
    assert_eq!(load(&root).await.unwrap().connections.len(), 25);
    registry.connections.insert(candidate.id.clone(), candidate);
    persist(&root, &registry).unwrap();
    assert!(load(&root).await.unwrap_err().contains("24"));
}

fn unit_dataset_proof(
    plan: &crate::datasets::DatasetPlan,
    source: &crate::datasets::DatasetSource,
) -> crate::datasets::SourceProvenance {
    serde_json::from_value(json!({
        "schemaVersion":2,"dataset":plan.dataset,"request":plan.request,
        "layer":plan.dataset.layers.iter().find(|layer|Some(&layer.id)==plan.request.layer.as_ref()).unwrap(),
        "sourceId":source.id,"sourceTitle":source.title,"sourceBounds":source.bounds,
        "declarationKind":if source.catalog_raster.is_some(){
            "reviewed-client-catalog-raster; exact publisher catalog record archived separately"
        }else{
            "reviewed-client-preset; publisher file bytes and embedded metadata verified separately"
        },
        "catalogRaster":source.catalog_raster,
    })).unwrap()
}

fn persist_unit_connection(root: &Path, connection: &Connection) -> Vec<u8> {
    let mut registry = Registry::default();
    registry
        .connections
        .insert(connection.id.clone(), connection.clone());
    persist(root, &registry).unwrap();
    std::fs::read(root.join("stac-connections.json")).unwrap()
}

#[tokio::test]
async fn tnm_current_archive_connection_reopens_and_binds_catalog_direct_document_and_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::create_dir_all(root.join("stac")).unwrap();
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../docs/research/next-channels-20261012-usgs-3dep.json"
    ))
    .unwrap();
    let catalog = fixture["nativeCase"]["document"].clone();
    let request: crate::datasets::ResolveRequest =
        serde_json::from_value(fixture["nativeCase"]["request"].clone()).unwrap();
    let mut plan = crate::datasets::resolve(request).unwrap();
    let catalog_url = crate::datasets::usgs_3dep::query(&plan.request).unwrap();
    // These are unit-serialized bytes of the real publisher Value. Their hash
    // is deliberately distinct from the original HTTP response-byte receipt;
    // no unit fixture is reported as a live metadata request or download.
    let catalog_bytes = serde_json::to_vec(&catalog).unwrap();
    let catalog_sha = immutable(&root, "document", &catalog_bytes).unwrap();
    assert_ne!(
        json!(catalog_sha),
        fixture["nativeCase"]["receipt"]["sha256"]
    );
    let (mut source, publisher_record) =
        crate::datasets::usgs_3dep::parse(&plan.request, &catalog_url, &catalog)
            .unwrap()
            .remove(0);
    let length = crate::datasets::usgs_3dep::original_size(&publisher_record).unwrap();
    assert_eq!(length, 20_582_281);
    source.catalog_raster = Some(crate::datasets::CatalogRasterEvidence {
        catalog_url: catalog_url.clone(),
        catalog_sha256: catalog_sha.clone(),
        record: publisher_record,
    });
    plan.sources = vec![source.clone()];
    let proof = unit_dataset_proof(&plan, &source);
    let href = source.stac.as_ref().unwrap().url.clone();
    let raw = json!({"kind":"direct-raster","href":href,"probeStatus":206,"contentLength":16,
        "contentRange":format!("bytes 0-15/{length}"),"headerHex":"49492a00","cogLayout":"not-validated",
        "file:size":length,"datasetPreset":proof});
    let direct_bytes = serde_json::to_vec(&raw).unwrap();
    let direct_sha = immutable(&root, "document", &direct_bytes).unwrap();
    let receipts = vec![DocumentReceipt {
        url: catalog_url.clone(),
        sha256: catalog_sha.clone(),
    }];
    let connection_id = Uuid::from_u128(501).to_string();
    let saved = Record {
        version: 1,
        connection_id: connection_id.clone(),
        service_name: "TNM unit current elevation".into(),
        kind: "raster".into(),
        document_url: href.clone(),
        document_request: None,
        document_sha256: direct_sha.clone(),
        item_index: None,
        retrieved_at: "2026-10-12T00:00:00Z".into(),
        metadata_documents: receipts.clone(),
        search: None,
        reviewed_dataset: None,
        asset_transports: Vec::new(),
    };
    let snapshot = save_snapshot(&root, saved.clone(), &raw).unwrap();
    let connection = Connection {
        id: connection_id,
        name: saved.service_name.clone(),
        url: href.clone(),
        kind: "raster".into(),
        connected_at: saved.retrieved_at.clone(),
        collections: vec![],
        capabilities: Capabilities {
            search_get: false,
            search_post: false,
        },
        snapshot_ids: vec![snapshot.id.clone()],
        search_url: None,
        search_method: SearchMethod::Get,
        metadata_sha256: vec![direct_sha.clone()],
        metadata_documents: receipts.clone(),
        catalog_nodes: vec![],
    };
    validate_connection_documents(&root, &connection).unwrap();
    let registry_bytes = persist_unit_connection(&root, &connection);
    let reopened = load(&root).await.unwrap();
    assert_eq!(reopened.connections.get(&connection.id), Some(&connection));
    assert_eq!(
        std::fs::read(root.join("stac-connections.json")).unwrap(),
        registry_bytes
    );
    assert_eq!(document(&root, &catalog_sha).unwrap(), catalog_bytes);
    let (_, restored) = record(&root, &snapshot.id).unwrap();
    assert_eq!(restored.document_sha256, direct_sha);
    assert_eq!(restored.provenance.metadata_documents, receipts);
    assert!(
        restored.datetime.is_none()
            && restored.start_datetime.is_none()
            && restored.end_datetime.is_none()
    );
    assert_eq!(restored.assets[0].metadata["file:size"], json!(length));
    assert_eq!(
        restored.properties["datasetPreset"]["catalogRaster"]["record"],
        catalog["items"][0]
    );

    // Another intact, hash-verified direct receipt for the same URL must not
    // be paired with this snapshot just because its href and kind agree.
    let mut other_raw = raw.clone();
    other_raw["unitReceiptVariant"] = json!(2);
    let other_sha = immutable(&root, "document", &serde_json::to_vec(&other_raw).unwrap()).unwrap();
    let mut other_record = saved.clone();
    other_record.document_sha256 = other_sha.clone();
    let other_snapshot = save_snapshot(&root, other_record, &other_raw).unwrap();
    let mut variants = Vec::new();
    let mut changed = connection.clone();
    changed.metadata_documents.clear();
    variants.push(changed);
    let mut changed = connection.clone();
    changed.metadata_documents[0].url =
        "https://tnmaccess.nationalmap.gov/api/v1/products?datasets=historical".into();
    variants.push(changed);
    let mut changed = connection.clone();
    changed.metadata_documents[0].sha256 = other_sha.clone();
    variants.push(changed);
    let mut changed = connection.clone();
    changed.metadata_sha256 = vec![other_sha];
    variants.push(changed);
    let mut changed = connection.clone();
    changed.snapshot_ids.clear();
    variants.push(changed);
    let mut changed = connection.clone();
    changed.snapshot_ids = vec![other_snapshot.id];
    variants.push(changed);
    for changed in variants {
        assert!(validate_connection_documents(&root, &changed).is_err());
        let invalid_registry_bytes = persist_unit_connection(&root, &changed);
        assert!(load(&root).await.is_err());
        assert_eq!(
            std::fs::read(root.join("stac-connections.json")).unwrap(),
            invalid_registry_bytes
        );
        // Rejecting a changed registry does not rewrite or erase the original
        // immutable snapshot, so the valid pinned selection stays readable.
        record(&root, &snapshot.id).unwrap();
    }
    persist_unit_connection(&root, &connection);
    load(&root).await.unwrap();
    let pin = SourcePin {
        snapshot_id: snapshot.id.clone(),
        asset_key: "raster".into(),
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
    verify_transfer(&root, &job, length, &"a".repeat(64)).unwrap();
    assert!(verify_transfer(&root, &job, length + 1, &"a".repeat(64)).is_err());
    let catalog_path = directory(&root)
        .unwrap()
        .join(format!("document-{catalog_sha}.json"));
    std::fs::write(catalog_path, b"{}").unwrap();
    assert!(record(&root, &snapshot.id).is_err());
    assert!(load(&root).await.is_err());
}

#[tokio::test]
async fn jaxa_generic_collection_identity_is_archived_without_aliasing_or_verified_attachment() {
    let fixtures: Value =
        serde_json::from_str(include_str!("../datasets/jaxa-reviewed-collections.json")).unwrap();
    for (dataset_id, layer) in [
        ("jaxa-amsr2-soil-moisture", "smc"),
        ("jaxa-sgli-lst", "lst"),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("stac")).unwrap();
        let request:crate::datasets::ResolveRequest=serde_json::from_value(json!({
            "datasetId":dataset_id,"bounds":[116.,39.,117.,40.],"year":2025,"month":12,"layer":layer})).unwrap();
        let plan = crate::datasets::resolve(request).unwrap();
        let source = &plan.sources[0];
        let proof = unit_dataset_proof(&plan, source);
        let (item_url, raw) =
            crate::datasets::jaxa::reviewed_document(dataset_id, source.bounds.unwrap());
        let entry = fixtures
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["datasetId"] == dataset_id)
            .unwrap();
        let collection_url = entry["url"].as_str().unwrap().to_string();
        let original_collection = entry["document"].clone();
        assert_eq!(original_collection["id"], "collection");
        assert_ne!(original_collection["id"], raw["collection"]);
        crate::datasets::jaxa::validate_collection(&proof, &collection_url, &original_collection)
            .unwrap();
        // Unit serialization SHA receipts are not the original fetched byte
        // hashes. Both actual publisher Values remain unmodified here.
        let item_bytes = serde_json::to_vec(&raw).unwrap();
        let collection_bytes = serde_json::to_vec(&original_collection).unwrap();
        let item_sha = immutable(&root, "document", &item_bytes).unwrap();
        let collection_sha = immutable(&root, "document", &collection_bytes).unwrap();
        let receipts = vec![
            DocumentReceipt {
                url: item_url.clone(),
                sha256: item_sha.clone(),
            },
            DocumentReceipt {
                url: collection_url.clone(),
                sha256: collection_sha.clone(),
            },
        ];
        let connection_id = Uuid::from_u128(if layer == "smc" { 601 } else { 602 }).to_string();
        let saved = Record {
            version: 1,
            connection_id: connection_id.clone(),
            service_name: source.title.clone(),
            kind: "item".into(),
            document_url: item_url.clone(),
            document_request: None,
            document_sha256: item_sha.clone(),
            item_index: None,
            retrieved_at: "2026-10-12T00:00:00Z".into(),
            metadata_documents: receipts.clone(),
            search: None,
            reviewed_dataset: Some(proof.clone()),
            asset_transports: Vec::new(),
        };
        let snapshot = save_snapshot(&root, saved.clone(), &raw).unwrap();
        assert_eq!(snapshot.item_id, raw["id"].as_str().unwrap());
        assert_eq!(
            snapshot.collection_id.as_deref(),
            raw["collection"].as_str()
        );
        assert!(snapshot.provenance.collection.is_none());
        assert_eq!(snapshot.provenance.metadata_documents, receipts);
        let connection = Connection {
            id: connection_id,
            name: source.title.clone(),
            url: item_url.clone(),
            kind: "item".into(),
            connected_at: "2026-10-12T00:00:00Z".into(),
            collections: vec![],
            capabilities: Capabilities {
                search_get: false,
                search_post: false,
            },
            snapshot_ids: vec![snapshot.id.clone()],
            search_url: None,
            search_method: SearchMethod::Get,
            metadata_sha256: vec![item_sha.clone(), collection_sha.clone()],
            metadata_documents: receipts.clone(),
            catalog_nodes: vec![],
        };
        validate_connection_documents(&root, &connection).unwrap();
        let registry_bytes = persist_unit_connection(&root, &connection);
        let reopened = load(&root).await.unwrap();
        assert_eq!(reopened.connections.get(&connection.id), Some(&connection));
        assert_eq!(
            std::fs::read(root.join("stac-connections.json")).unwrap(),
            registry_bytes
        );
        assert_eq!(document(&root, &item_sha).unwrap(), item_bytes);
        assert_eq!(document(&root, &collection_sha).unwrap(), collection_bytes);
        let (_, restored) = record(&root, &snapshot.id).unwrap();
        assert_eq!(restored.item_id, snapshot.item_id);
        assert_eq!(restored.collection_id, snapshot.collection_id);
        assert_eq!(restored.properties, raw["properties"]);
        assert_eq!(restored.provenance.metadata_documents, receipts);
        assert!(restored.provenance.collection.is_none());
        assert_eq!(restored.provenance.reviewed_dataset, Some(proof.clone()));
        let key = if layer == "smc" { "SMC" } else { "LST" };
        assert_eq!(restored.assets[0].metadata, raw["assets"][key]);

        // Remove only this temporary unit registry. A saved pin must enforce
        // the collection archive contract even after its connection is gone.
        std::fs::remove_file(root.join("stac-connections.json")).unwrap();
        assert!(!root.join("stac-connections.json").exists());
        record(&root, &snapshot.id).unwrap();
        let mut incomplete = saved.clone();
        incomplete.metadata_documents.pop();
        assert!(save_snapshot(&root, incomplete.clone(), &raw).is_err());
        let incomplete_id =
            immutable(&root, "snapshot", &serde_json::to_vec(&incomplete).unwrap()).unwrap();
        assert!(record(&root, &incomplete_id).is_err());

        // A syntactically valid STAC Collection with changed scientific terms
        // or an alias still cannot replace the exact reviewed JAXA original.
        for (field, value) in [
            ("license", json!("CC-BY-4.0")),
            ("id", raw["collection"].clone()),
            (
                "description",
                json!("changed but valid collection description"),
            ),
        ] {
            let mut changed = original_collection.clone();
            changed[field] = value;
            collection(&changed).unwrap();
            assert!(
                crate::datasets::jaxa::validate_collection(&proof, &collection_url, &changed)
                    .is_err(),
                "accepted {field}"
            );
            let changed_bytes = serde_json::to_vec(&changed).unwrap();
            let changed_sha = immutable(&root, "document", &changed_bytes).unwrap();
            assert_eq!(document(&root, &changed_sha).unwrap(), changed_bytes);
            let mut changed_receipt = saved.clone();
            changed_receipt.metadata_documents[1].sha256 = changed_sha;
            // The replacement bytes and their SHA are valid, but the scoped
            // JAXA scientific declaration has changed and cannot be saved.
            assert!(save_snapshot(&root, changed_receipt.clone(), &raw).is_err());
            // Simulate an externally written, correctly checksummed receipt:
            // standalone pin loading must reject it without any connection.
            let changed_id = immutable(
                &root,
                "snapshot",
                &serde_json::to_vec(&changed_receipt).unwrap(),
            )
            .unwrap();
            assert!(record(&root, &changed_id).is_err());
            record(&root, &snapshot.id).unwrap();
        }
        let mut changed = connection.clone();
        changed.collections = vec![collection(&original_collection).unwrap()];
        assert!(validate_connection_documents(&root, &changed).is_err());
        persist_unit_connection(&root, &changed);
        assert!(load(&root).await.is_err());
        let mut changed = connection.clone();
        changed.metadata_documents.pop();
        changed.metadata_sha256.pop();
        assert!(validate_connection_documents(&root, &changed).is_err());
        persist_unit_connection(&root, &changed);
        assert!(load(&root).await.is_err());
        persist_unit_connection(&root, &connection);
        load(&root).await.unwrap();
        // Reading the valid pin remains possible independently of registry
        // mutation, and it never upgrades the generic collection to an alias.
        let pin = SourcePin {
            snapshot_id: snapshot.id.clone(),
            asset_key: key.into(),
        };
        let selected = resolve(&root, &pin).unwrap();
        assert_eq!(selected.item_id, raw["id"].as_str().unwrap());
        assert_eq!(
            selected.collection_id.as_deref(),
            raw["collection"].as_str()
        );
    }
}

struct MauiUnitArchive {
    item: Value,
    collection: Value,
    event: Value,
    saved: Record,
    snapshot: ItemSnapshot,
    connection: Connection,
}

fn maui_unit_archive(root: &Path, asset: &str) -> MauiUnitArchive {
    std::fs::create_dir_all(root.join("stac")).unwrap();
    let items: Value =
        serde_json::from_str(include_str!("../datasets/maxar-reviewed-items.json")).unwrap();
    let collections: Value =
        serde_json::from_str(include_str!("../datasets/maxar-reviewed-collections.json")).unwrap();
    let events: Value =
        serde_json::from_str(include_str!("../datasets/maxar-reviewed-events.json")).unwrap();
    let item = items[0]["document"].clone();
    let collection_value = collections[0]["document"].clone();
    let event = events[0]["document"].clone();
    let item_url = items[0]["url"].as_str().unwrap().to_string();
    let request = serde_json::from_value(json!({"datasetId":"maxar-open-legacy",
        "bounds":[-156.671,20.965,-156.667,20.968],"year":2023,"month":8,"layer":asset}))
    .unwrap();
    let plan = crate::datasets::resolve(request).unwrap();
    let proof = unit_dataset_proof(&plan, &plan.sources[0]);
    // Hashes here cover unit serialization of unchanged publisher Values.
    // They are not receipts for original HTTP bytes or live file transfers.
    let mut receipts = Vec::new();
    for (url, value) in [
        (item_url.clone(), &item),
        (
            collections[0]["url"].as_str().unwrap().into(),
            &collection_value,
        ),
        (events[0]["url"].as_str().unwrap().into(), &event),
    ] {
        receipts.push(DocumentReceipt {
            url,
            sha256: immutable(root, "document", &serde_json::to_vec(value).unwrap()).unwrap(),
        });
    }
    let connection_id = Uuid::from_u128(if asset == "ms_analytic" { 701 } else { 702 }).to_string();
    let saved = Record {
        version: 1,
        connection_id: connection_id.clone(),
        service_name: plan.sources[0].title.clone(),
        kind: "item".into(),
        document_url: item_url.clone(),
        document_request: None,
        document_sha256: receipts[0].sha256.clone(),
        item_index: None,
        retrieved_at: "2026-10-12T00:00:00Z".into(),
        metadata_documents: receipts.clone(),
        search: None,
        reviewed_dataset: Some(proof),
        asset_transports: Vec::new(),
    };
    let snapshot = save_snapshot(root, saved.clone(), &item).unwrap();
    let connection = Connection {
        id: connection_id,
        name: saved.service_name.clone(),
        url: item_url,
        kind: "item".into(),
        connected_at: saved.retrieved_at.clone(),
        collections: vec![collection(&collection_value).unwrap()],
        capabilities: Capabilities {
            search_get: false,
            search_post: false,
        },
        snapshot_ids: vec![snapshot.id.clone()],
        search_url: None,
        search_method: SearchMethod::Get,
        metadata_sha256: receipts
            .iter()
            .map(|receipt| receipt.sha256.clone())
            .collect(),
        metadata_documents: receipts,
        catalog_nodes: vec![],
    };
    MauiUnitArchive {
        item,
        collection: collection_value,
        event,
        saved,
        snapshot,
        connection,
    }
}

#[tokio::test]
async fn maui_exact_three_document_archive_reopens_original_collection_and_instant() {
    for asset in ["ms_analytic", "pan_analytic"] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let original = maui_unit_archive(&root, asset);
        validate_connection_documents(&root, &original.connection).unwrap();
        let registry_bytes = persist_unit_connection(&root, &original.connection);
        let reopened = load(&root).await.unwrap();
        assert_eq!(
            reopened.connections.get(&original.connection.id),
            Some(&original.connection)
        );
        assert_eq!(
            std::fs::read(root.join("stac-connections.json")).unwrap(),
            registry_bytes
        );
        let (saved, restored) = record(&root, &original.snapshot.id).unwrap();
        assert_eq!(saved.metadata_documents, original.saved.metadata_documents);
        assert_eq!(restored.item_id, "4/122000312222/10300100EB15FF00");
        assert_eq!(restored.collection_id.as_deref(), Some("10300100EB15FF00"));
        assert_eq!(restored.datetime.as_deref(), Some("2023-08-12 21:12:49Z"));
        assert_eq!(restored.temporal_status, "instant");
        assert!(restored.start_datetime.is_none() && restored.end_datetime.is_none());
        assert_eq!(restored.properties, original.item["properties"]);
        assert_eq!(
            restored.provenance.collection,
            Some(original.collection.clone())
        );
        assert_eq!(
            restored.provenance.reviewed_dataset,
            original.saved.reviewed_dataset
        );
        assert!(restored.provenance.asset_transports.is_empty());
        for declared in &restored.assets {
            assert_eq!(declared.metadata, original.item["assets"][&declared.key]);
        }
        for receipt in &saved.metadata_documents {
            let value = json_document(&document(&root, &receipt.sha256).unwrap()).unwrap();
            assert!(
                value == original.item || value == original.collection || value == original.event
            );
        }
        let pin = SourcePin {
            snapshot_id: restored.id.clone(),
            asset_key: asset.into(),
        };
        let selected = resolve(&root, &pin).unwrap();
        assert_eq!(selected.item_id, restored.item_id);
        assert_eq!(selected.asset_key, asset);
        // Standalone pins must retain the same three-document contract even
        // after the saved connection registry is no longer present.
        std::fs::remove_file(root.join("stac-connections.json")).unwrap();
        record(&root, &original.snapshot.id).unwrap();
        resolve(&root, &pin).unwrap();
    }
}

#[tokio::test]
async fn maui_rehashed_collection_and_event_mutations_cannot_replace_standalone_receipts() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let original = maui_unit_archive(&root, "ms_analytic");
    let mut variants = Vec::new();
    for index in [1usize, 2] {
        let value = if index == 1 {
            &original.collection
        } else {
            &original.event
        };
        for field in ["license", "id", "linked-member"] {
            let mut changed = value.clone();
            match field {
                "license" => changed["license"] = json!("proprietary"),
                "id" => changed["id"] = json!("other-valid-collection"),
                _ => {
                    let relation = if index == 1 { "item" } else { "child" };
                    let link = changed["links"]
                        .as_array_mut()
                        .unwrap()
                        .iter_mut()
                        .find(|link| link["rel"] == relation)
                        .unwrap();
                    link["href"] = json!("https://example.com/other-member.json");
                }
            }
            // STAC syntax and its newly computed immutable hash are both
            // valid; rejection must come from the exact reviewed declaration.
            collection(&changed).unwrap();
            let bytes = serde_json::to_vec(&changed).unwrap();
            let sha = immutable(&root, "document", &bytes).unwrap();
            assert_eq!(document(&root, &sha).unwrap(), bytes);
            let mut record = original.saved.clone();
            record.metadata_documents[index].sha256 = sha;
            variants.push(record);
        }
    }
    let mut missing = original.saved.clone();
    missing.metadata_documents.pop();
    variants.push(missing);
    let mut reordered = original.saved.clone();
    reordered.metadata_documents.swap(1, 2);
    variants.push(reordered);
    let mut duplicate = original.saved.clone();
    duplicate.metadata_documents[2] = duplicate.metadata_documents[1].clone();
    variants.push(duplicate);
    for changed in variants {
        assert!(save_snapshot(&root, changed.clone(), &original.item).is_err());
        // A correctly hashed receipt written without the save helper cannot
        // bypass reload validation when no Connection is present.
        assert!(!root.join("stac-connections.json").exists());
        let invalid_id =
            immutable(&root, "snapshot", &serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(record(&root, &invalid_id).is_err());
        let mut connection = original.connection.clone();
        connection.snapshot_ids = vec![invalid_id];
        connection.metadata_documents = changed.metadata_documents.clone();
        connection.metadata_sha256 = changed
            .metadata_documents
            .iter()
            .map(|r| r.sha256.clone())
            .collect();
        assert!(validate_connection_documents(&root, &connection).is_err());
        let invalid_registry = persist_unit_connection(&root, &connection);
        assert!(load(&root).await.is_err());
        assert_eq!(
            std::fs::read(root.join("stac-connections.json")).unwrap(),
            invalid_registry
        );
        std::fs::remove_file(root.join("stac-connections.json")).unwrap();
        record(&root, &original.snapshot.id).unwrap();
    }
    persist_unit_connection(&root, &original.connection);
    load(&root).await.unwrap();
}

#[test]
fn maui_visual_client_proof_cannot_authorize_an_unreviewed_layer() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let original = maui_unit_archive(&root, "pan_analytic");
    let mut changed = original.saved.clone();
    let proof = changed.reviewed_dataset.as_mut().unwrap();
    proof.request.layer = Some("visual".into());
    proof.layer.id = "visual".into();
    assert!(save_snapshot(&root, changed.clone(), &original.item).is_err());
    let invalid_id = immutable(&root, "snapshot", &serde_json::to_vec(&changed).unwrap()).unwrap();
    assert!(record(&root, &invalid_id).is_err());
    record(&root, &original.snapshot.id).unwrap();
}

#[tokio::test]
async fn ordinary_stac_items_do_not_inherit_maui_three_document_permission() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let maui = maui_unit_archive(&root, "pan_analytic");
    let raw = item_value("https://example.com/source.tif");
    let bytes = serde_json::to_vec(&raw).unwrap();
    let item_sha = immutable(&root, "document", &bytes).unwrap();
    let item_url = "https://example.com/item.json".to_string();
    let mut saved = maui.saved.clone();
    saved.connection_id = Uuid::from_u128(703).to_string();
    saved.document_url = item_url.clone();
    saved.document_sha256 = item_sha.clone();
    saved.reviewed_dataset = None;
    saved.metadata_documents[0] = DocumentReceipt {
        url: item_url.clone(),
        sha256: item_sha,
    };
    let snapshot = save_snapshot(&root, saved.clone(), &raw).unwrap();
    assert!(snapshot.provenance.reviewed_dataset.is_none());
    let mut connection = maui.connection.clone();
    connection.id = saved.connection_id;
    connection.url = item_url;
    connection.collections.clear();
    connection.snapshot_ids = vec![snapshot.id];
    connection.metadata_documents = saved.metadata_documents;
    connection.metadata_sha256 = connection
        .metadata_documents
        .iter()
        .map(|r| r.sha256.clone())
        .collect();
    assert!(validate_connection_documents(&root, &connection).is_err());
    persist_unit_connection(&root, &connection);
    assert!(load(&root).await.is_err());
}

#[tokio::test]
async fn reviewed_maui_eligibility_authorizes_only_exact_selected_analytic_asset() {
    for selected_key in ["ms_analytic", "pan_analytic"] {
        let temp = tempfile::tempdir().unwrap();
        let manager = JobManager::open(temp.path()).await.unwrap();
        let root = manager.storage_root();
        let original = maui_unit_archive(root, selected_key);
        let snapshot_path = directory(root)
            .unwrap()
            .join(format!("snapshot-{}.json", original.snapshot.id));
        let receipt_before = std::fs::read(&snapshot_path).unwrap();
        let (_, restored) = record(root, &original.snapshot.id).unwrap();
        assert_eq!(
            restored.provenance.reviewed_dataset,
            original.saved.reviewed_dataset
        );
        assert_eq!(std::fs::read(&snapshot_path).unwrap(), receipt_before);
        for asset in &restored.assets {
            assert_eq!(asset.metadata, original.item["assets"][&asset.key]);
            assert_eq!(asset.eligible, asset.key == selected_key);
            if asset.key != selected_key {
                let reason = asset.reason.as_deref().unwrap();
                assert!(reason.contains("reviewed Maui"));
                assert!(
                    reason.contains("Original publisher metadata")
                        || reason.contains("original publisher metadata")
                );
                if asset.key == "visual" {
                    assert!(reason.contains("JPEG/YCbCr"));
                } else {
                    assert!(reason.contains(selected_key));
                }
            }
        }
        let selected = SourcePin {
            snapshot_id: restored.id.clone(),
            asset_key: selected_key.into(),
        };
        let project = manager
            .save_stac_project(crate::stac_projects::SaveProjectRequest {
                project_id: None,
                name: Some("Reviewed Maui selected original".into()),
                bounds: original
                    .saved
                    .reviewed_dataset
                    .as_ref()
                    .unwrap()
                    .request
                    .bounds,
                selections: vec![selected.clone()],
            })
            .await
            .unwrap();
        crate::stac_projects::validate_project(root, &project).unwrap();
        resolve(root, &selected).unwrap();
        for key in ["ms_analytic", "pan_analytic", "visual"] {
            if key == selected_key {
                continue;
            }
            let pin = SourcePin {
                snapshot_id: restored.id.clone(),
                asset_key: key.into(),
            };
            assert!(resolve(root, &pin).is_err());
            assert!(manager
                .save_stac_project(crate::stac_projects::SaveProjectRequest {
                    project_id: Some(project.id.clone()),
                    name: None,
                    bounds: project.bounds,
                    selections: vec![pin.clone()],
                })
                .await
                .is_err());
            assert!(manager
                .download_stac_project(crate::stac_projects::DownloadRequest {
                    project_id: project.id.clone(),
                    selections: Some(vec![pin.clone()]),
                })
                .await
                .is_err());
            let asset = restored
                .assets
                .iter()
                .find(|asset| asset.key == key)
                .unwrap();
            let mut job = crate::new_download_job(crate::CreateJobRequest {
                item_id: restored.item_id.clone(),
                asset_key: "stac_asset".into(),
                href: asset.href.clone(),
                media_type: asset.media_type.clone().unwrap(),
                title: Some(format!("{} · {}", restored.title, asset.title)),
            });
            job.source = original.saved.service_name.clone();
            job.stac_source = Some(pin);
            assert!(validate_job(root, &job).is_err());
            let mut forged_project = project.clone();
            forged_project.stac_items[0].asset_key = key.into();
            forged_project.stac_items[0].href = asset.href.clone();
            forged_project.stac_items[0].media_type = asset.media_type.clone().unwrap();
            forged_project.stac_items[0].title = format!("{} · {}", restored.title, asset.title);
            assert!(crate::stac_projects::validate_project(root, &forged_project).is_err());
        }
        assert!(manager.inner.store.lock().await.jobs.is_empty());
        record(root, &original.snapshot.id).unwrap();
        assert_eq!(std::fs::read(snapshot_path).unwrap(), receipt_before);
    }
}

#[test]
fn ordinary_maui_item_does_not_receive_reviewed_layer_restriction_or_accept_forged_proof() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let original = maui_unit_archive(&root, "ms_analytic");
    let mut generic = original.saved.clone();
    generic.reviewed_dataset = None;
    generic.metadata_documents.pop();
    let snapshot = save_snapshot(&root, generic.clone(), &original.item).unwrap();
    assert!(snapshot.provenance.reviewed_dataset.is_none());
    for key in ["ms_analytic", "pan_analytic", "visual"] {
        let asset = snapshot
            .assets
            .iter()
            .find(|asset| asset.key == key)
            .unwrap();
        assert!(asset.eligible);
        assert_eq!(asset.metadata, original.item["assets"][key]);
        resolve(
            &root,
            &SourcePin {
                snapshot_id: snapshot.id.clone(),
                asset_key: key.into(),
            },
        )
        .unwrap();
    }
    let mut connection = original.connection.clone();
    connection.snapshot_ids = vec![snapshot.id];
    connection.metadata_documents = generic.metadata_documents;
    connection.metadata_sha256 = connection
        .metadata_documents
        .iter()
        .map(|r| r.sha256.clone())
        .collect();
    validate_connection_documents(&root, &connection).unwrap();
    // Mere product-ID text is not sufficient: item() validates the exact
    // native reviewed proof before applying its narrow asset eligibility rule.
    let mut forged = original.saved;
    forged.reviewed_dataset.as_mut().unwrap().request.year = Some(2024);
    assert!(save_snapshot(&root, forged.clone(), &original.item).is_err());
    let forged_id = immutable(&root, "snapshot", &serde_json::to_vec(&forged).unwrap()).unwrap();
    assert!(record(&root, &forged_id).is_err());
}
