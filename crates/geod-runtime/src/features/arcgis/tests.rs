use super::*;

// All values in this module are synthetic contract fixtures. They are not
// captured provider responses and do not demonstrate live-service acceptance.
const ROOT: &str = "https://example.com/arcgis/rest/services/Synthetic/FeatureServer";
const REGION: [f64; 4] = [0., 0., 10., 10.];

fn layer_json() -> Value {
    json!({
        "id": 0,
        "name": "Synthetic places",
        "type": "Feature Layer",
        "description": "Synthetic metadata fixture",
        "capabilities": "Create, Query, Update",
        "supportedQueryFormats": "JSON, geoJSON, PBF",
        "hasZ": false,
        "hasM": false,
        "hasCurves": false,
        "objectIdField": "OBJECTID",
        "geometryType": "esriGeometryPoint",
        "sourceSpatialReference": {"wkid": 102100, "latestWkid": 3857},
        "extent": {"spatialReference": {"wkid": 4326}},
        "maxRecordCount": 2000,
        "copyrightText": "Synthetic layer attribution",
        "fields": [
            {"name":"OBJECTID", "alias":"Object ID", "type":"esriFieldTypeOID"},
            {"name":"name", "alias":"名称", "type":"esriFieldTypeString"},
            {"name":"height", "alias":"Height", "type":"esriFieldTypeDouble"}
        ]
    })
}

fn collection_fixture() -> Collection {
    parse_layer(
        &root_url(ROOT).unwrap(),
        &layer_json(),
        &"a".repeat(64),
        "Service attribution",
    )
    .unwrap()
}

fn layer_fixture() -> Layer {
    collection_fixture().arcgis.unwrap()
}

fn service_fixture() -> FeatureService {
    FeatureService {
        id: Uuid::new_v4().to_string(),
        name: "Synthetic ArcGIS".into(),
        url: ROOT.into(),
        title: "Synthetic service".into(),
        collections: vec![collection_fixture()],
        connected_at: now(),
        overpass: None,
        wfs: None,
        arcgis: Some(Service {
            current_version: 11.3,
            copyright_text: "Synthetic service attribution".into(),
            metadata_sha256: "b".repeat(64),
            excluded_layers: vec![ExcludedLayer {
                id: "2".into(),
                name: "Synthetic Z layer".into(),
                reason: "Z geometry needs a separate adapter".into(),
            }],
        }),
    }
}

fn feature(id: u64) -> Value {
    json!({"type":"Feature", "id":id,
        "properties":{"OBJECTID":id,"name":"测试地点","height":null},
        "geometry":{"type":"Point","coordinates":[2.5,3.25]}})
}

fn batch(features: Vec<Value>) -> Value {
    json!({"type":"FeatureCollection", "features":features})
}

fn receipt(parameters: Parameters, returned: usize, body: Value) -> PageReceipt {
    let bytes = serde_json::to_vec(&body).unwrap();
    PageReceipt {
        url: format!("{ROOT}/0/query"),
        sha256: hash(&bytes),
        bytes: bytes.len(),
        returned,
        parameters: Some(parameters),
    }
}

fn source_fixture(ids: &[u64]) -> Provenance {
    let id_receipt = receipt(
        selection_parameters(REGION, false),
        ids.len(),
        json!({"objectIdFieldName":"OBJECTID", "objectIds":ids}),
    );
    let count_receipt = receipt(
        selection_parameters(REGION, true),
        ids.len(),
        json!({"count":ids.len()}),
    );
    Provenance {
        service_url: ROOT.into(),
        service_name: "Synthetic ArcGIS".into(),
        collection_id: "0".into(),
        collection_title: "Synthetic places".into(),
        license_links: vec![],
        requested_bounds: REGION,
        area_geometry: None,
        requested_at: now(),
        pages: ids
            .chunks(2)
            .map(|ids| {
                receipt(
                    batch_parameters(ids),
                    ids.len(),
                    batch(ids.iter().map(|id| feature(*id)).collect()),
                )
            })
            .collect(),
        number_matched: Some(ids.len()),
        feature_count: ids.len(),
        selection: "bbox-full-features".into(),
        wfs: None,
        arcgis: Some(Snapshot {
            layer: layer_fixture(),
            object_ids: ids.to_vec(),
            id_receipts: vec![id_receipt.clone(), id_receipt],
            count_receipts: vec![count_receipt.clone(), count_receipt],
        }),
    }
}

#[test]
fn discovery_preserves_schema_and_source_crs_without_inventing_a_license() {
    let root = root_url(ROOT).unwrap();
    let collection = collection_fixture();
    assert_eq!(collection.id, "0");
    assert_eq!(collection.items_url, format!("{ROOT}/0/query"));
    assert!(collection.license_links.is_empty());
    let layer = collection.arcgis.unwrap();
    assert_eq!(
        layer.spatial_reference,
        json!({"wkid":102100,"latestWkid":3857})
    );
    assert_eq!(layer.fields[1].alias, "名称");
    assert_eq!(layer.copyright_text, "Synthetic layer attribution");
    assert_eq!(layer.max_record_count, 2000);
    let mut fallback = layer_json();
    fallback
        .as_object_mut()
        .unwrap()
        .remove("sourceSpatialReference");
    fallback["copyrightText"] = json!("");
    let layer = parse_layer(&root, &fallback, &"a".repeat(64), "Service attribution")
        .unwrap()
        .arcgis
        .unwrap();
    assert_eq!(layer.spatial_reference, json!({"wkid":4326}));
    assert_eq!(layer.copyright_text, "Service attribution");
    validate_service(&service_fixture()).unwrap();
}

#[test]
fn unsupported_metadata_is_rejected_before_a_layer_can_be_offered() {
    let root = root_url(ROOT).unwrap();
    for (field, value) in [
        ("hasZ", json!(true)),
        ("hasM", json!(true)),
        ("hasCurves", json!(true)),
        ("hasZ", json!(null)),
        ("hasM", json!("false")),
        ("supportedQueryFormats", json!("JSON, PBF")),
        ("capabilities", json!("Create, Update")),
        ("type", json!("Table")),
        ("geometryType", json!("esriGeometryEnvelope")),
        ("objectIdField", json!("other")),
        ("maxRecordCount", json!(0)),
        ("id", json!(-1)),
        ("id", json!("0")),
        ("id", json!(u64::from(u32::MAX) + 1)),
    ] {
        let mut value_to_parse = layer_json();
        value_to_parse[field] = value.clone();
        assert!(
            parse_layer(&root, &value_to_parse, &"a".repeat(64), "").is_err(),
            "{field}: {value}"
        );
    }
    for unsupported in [
        "esriFieldTypeGeometry",
        "esriFieldTypeBlob",
        "esriFieldTypeRaster",
    ] {
        let mut value = layer_json();
        value["fields"][1]["type"] = json!(unsupported);
        assert!(parse_layer(&root, &value, &"a".repeat(64), "").is_err());
    }
    let mut duplicated = layer_json();
    duplicated["fields"][1]["name"] = json!("OBJECTID");
    assert!(parse_layer(&root, &duplicated, &"a".repeat(64), "").is_err());
    let mut two_ids = layer_json();
    two_ids["fields"][1]["type"] = json!("esriFieldTypeOID");
    assert!(parse_layer(&root, &two_ids, &"a".repeat(64), "").is_err());
}

#[test]
fn discovery_excludes_generated_hash_ids_that_require_unique_id_queries() {
    let root = root_url(ROOT).unwrap();
    let mut metadata = layer_json();
    metadata["uniqueIdInfo"] = json!({
        "type":"simple", "fields":["name"], "OIDFieldContainsHashValue":true
    });
    assert!(parse_layer(&root, &metadata, &"a".repeat(64), "").is_err());
    metadata["uniqueIdInfo"]["OIDFieldContainsHashValue"] = json!(false);
    parse_layer(&root, &metadata, &"a".repeat(64), "").unwrap();
}

#[test]
fn layer_recheck_rejects_changed_identity_schema_and_geometry_declarations() {
    type Mutation = (&'static str, fn(&mut Value));
    let mutations: &[Mutation] = &[
        ("layer ID", |v| v["id"] = json!(1)),
        ("layer title", |v| v["name"] = json!("Republished places")),
        ("field name", |v| v["fields"][1]["name"] = json!("label")),
        ("field alias", |v| {
            v["fields"][1]["alias"] = json!("Updated label")
        }),
        ("field type", |v| {
            v["fields"][2]["type"] = json!("esriFieldTypeString")
        }),
        ("removed field", |v| {
            v["fields"].as_array_mut().unwrap().pop();
        }),
        ("added field", |v| {
            v["fields"].as_array_mut().unwrap().push(json!({
                "name":"status", "alias":"Status", "type":"esriFieldTypeString"
            }));
        }),
        ("source spatial reference", |v| {
            v["sourceSpatialReference"] = json!({"wkid":4326})
        }),
        ("geometry type", |v| {
            v["geometryType"] = json!("esriGeometryPolyline")
        }),
        ("object ID field", |v| {
            v["objectIdField"] = json!("NEW_OBJECTID");
            v["fields"][0]["name"] = json!("NEW_OBJECTID");
        }),
        ("query limit", |v| v["maxRecordCount"] = json!(1000)),
        ("attribution", |v| {
            v["copyrightText"] = json!("Updated synthetic attribution")
        }),
    ];
    let root = root_url(ROOT).unwrap();
    let before = collection_fixture();
    ensure_same_layer(&before, &before).unwrap();
    for (name, mutate) in mutations {
        let mut metadata = layer_json();
        mutate(&mut metadata);
        let after = parse_layer(&root, &metadata, &"b".repeat(64), "").unwrap();
        assert!(ensure_same_layer(&before, &after).is_err(), "{name}");
    }
    // Raw metadata bytes can change without changing the saved layer contract.
    let mut hash_only = before.clone();
    hash_only.arcgis.as_mut().unwrap().metadata_sha256 = "b".repeat(64);
    ensure_same_layer(&before, &hash_only).unwrap();
}

#[test]
fn feature_server_urls_and_layer_identifiers_are_canonical_and_scoped() {
    let root = root_url(&format!("{ROOT}/")).unwrap();
    assert_eq!(root.as_str(), ROOT);
    assert_eq!(
        metadata(&root, "layers").unwrap().as_str(),
        format!("{ROOT}/layers?f=json")
    );
    assert_eq!(
        query_url(&root, "4294967295").unwrap().as_str(),
        format!("{ROOT}/4294967295/query")
    );
    for id in [
        "",
        "00",
        "+1",
        "-1",
        "1.0",
        "4294967296",
        "../0",
        "0/query",
        "0?token=x",
    ] {
        assert!(query_url(&root, id).is_err(), "{id}");
    }
    for raw in [
        format!("{ROOT}?f=json"),
        format!("{ROOT}/0"),
        format!("{ROOT}Extra"),
        ROOT.replace("FeatureServer", "MapServer"),
        ROOT.replace("https:", "http:"),
        ROOT.replace("example.com", "127.0.0.1"),
        ROOT.replace("example.com", "user:secret@example.com"),
    ] {
        assert!(root_url(&raw).is_err(), "{raw}");
    }
    let mut service = service_fixture();
    service.url.push('/');
    assert!(validate_service(&service).is_err());
    let mut service = service_fixture();
    service.collections[0].items_url = "https://other.example/0/query".into();
    assert!(validate_service(&service).is_err());
    let mut service = service_fixture();
    service.arcgis.as_mut().unwrap().excluded_layers[0].id = "0".into();
    assert!(validate_service(&service).is_err());
}

#[test]
fn http_success_with_arcgis_error_or_truncation_is_not_successful_data() {
    for code in [400, 401, 403, 498, 499, 500] {
        assert!(
            complete_response(&json!({"error":{"code":code,"message":"Synthetic error"}})).is_err()
        );
    }
    for value in [json!([]), json!(null), json!("not an object")] {
        assert!(response_ok(&value).is_err());
    }
    for flag in [json!(true), json!("false"), json!(0), json!(null)] {
        assert!(complete_response(&json!({"exceededTransferLimit":flag})).is_err());
        assert!(complete_response(&json!({"properties":{"exceededTransferLimit":flag}})).is_err());
        assert!(complete_response(&json!({
            "exceededTransferLimit":false,
            "properties":{"exceededTransferLimit":flag}
        }))
        .is_err());
    }
    complete_response(&json!({"features":[]})).unwrap();
    complete_response(&json!({"exceededTransferLimit":false})).unwrap();
    complete_response(&json!({"properties":{"exceededTransferLimit":false}})).unwrap();
}

#[test]
fn membership_uses_independent_count_and_rejects_unsafe_or_incomplete_object_ids() {
    let layer = layer_fixture();
    let response = |ids| json!({"objectIdFieldName":"OBJECTID", "objectIds":ids});
    assert_eq!(
        parse_ids(&response(json!([7, 0, 2])), &layer, 3).unwrap(),
        vec![0, 2, 7]
    );
    assert!(parse_ids(&response(json!([])), &layer, 0)
        .unwrap()
        .is_empty());
    // ArcGIS can explicitly return null for a zero-match object ID query.
    // An independently verified zero count is required; omission is not null.
    assert!(parse_ids(&response(json!(null)), &layer, 0)
        .unwrap()
        .is_empty());
    assert!(parse_ids(&response(json!(null)), &layer, 1).is_err());
    let missing_ids = json!({"objectIdFieldName":"OBJECTID"});
    assert!(parse_ids(&missing_ids, &layer, 0).is_err());
    assert!(parse_ids(&missing_ids, &layer, 1).is_err());
    assert!(parse_ids(&response(json!([1])), &layer, 2).is_err());
    assert!(parse_ids(&response(json!([1, 2])), &layer, 1).is_err());
    for ids in [
        json!([1, 1]),
        json!([-1]),
        json!([1.5]),
        json!(["1"]),
        json!([SAFE_ID + 1]),
        json!(null),
    ] {
        assert!(parse_ids(&response(ids), &layer, 1).is_err());
    }
    let mut changed = response(json!([1]));
    changed["objectIdFieldName"] = json!("OBJECTID2");
    assert!(parse_ids(&changed, &layer, 1).is_err());
    changed["objectIdFieldName"] = json!("OBJECTID");
    changed["exceededTransferLimit"] = json!(true);
    assert!(parse_ids(&changed, &layer, 1).is_err());
    assert!(parse_ids(&response(json!(vec![1; MAX_IDS + 1])), &layer, MAX_IDS + 1).is_err());
}

#[test]
fn batches_retain_values_and_full_geometry_and_restore_object_id_order() {
    let mut polygon_layer = layer_fixture();
    polygon_layer.geometry_type = "esriGeometryPolygon".into();
    let mut polygon = feature(2);
    // Geometry crosses the selection bounds; extraction must not clip it.
    polygon["geometry"] =
        json!({"type":"Polygon","coordinates":[[[-5.,-5.],[15.,-5.],[15.,15.],[-5.,-5.]]]});
    let mut empty_geometry = feature(7);
    empty_geometry["geometry"] = Value::Null;
    empty_geometry["properties"]["name"] = json!("  Preserve spacing and Unicode 中文  ");
    let result = append_batch(
        batch(vec![empty_geometry.clone(), polygon.clone()]),
        &[2, 7],
        &polygon_layer,
    )
    .unwrap();
    assert_eq!(result, vec![polygon, empty_geometry]);
    assert_eq!(batch_parameters(&[2, 7])["outFields"], "*");
    assert_eq!(batch_parameters(&[2, 7])["returnGeometry"], "true");
    assert!(!batch_parameters(&[2, 7]).contains_key("geometry"));
    assert!(append_batch(batch(vec![]), &[], &layer_fixture())
        .unwrap()
        .is_empty());
}

#[test]
fn two_dimensional_point_multipoint_line_and_polygon_variants_are_supported() {
    for (layer_type, geometry) in [
        (
            "esriGeometryPoint",
            json!({"type":"Point","coordinates":[1,2]}),
        ),
        (
            "esriGeometryMultipoint",
            json!({"type":"MultiPoint","coordinates":[[1,2],[3,4]]}),
        ),
        (
            "esriGeometryPolyline",
            json!({"type":"LineString","coordinates":[[1,2],[3,4]]}),
        ),
        (
            "esriGeometryPolyline",
            json!({"type":"MultiLineString","coordinates":[[[1,2],[3,4]]]}),
        ),
        (
            "esriGeometryPolygon",
            json!({"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,0]]]}),
        ),
        (
            "esriGeometryPolygon",
            json!({"type":"MultiPolygon","coordinates":[[[[0,0],[1,0],[1,1],[0,0]]]]}),
        ),
    ] {
        let mut layer = layer_fixture();
        layer.geometry_type = layer_type.into();
        let mut f = feature(1);
        f["geometry"] = geometry;
        assert_eq!(
            append_batch(batch(vec![f.clone()]), &[1], &layer).unwrap(),
            vec![f]
        );
    }
}

#[test]
fn batches_reject_missing_attributes_changed_fields_ids_and_geometry() {
    let layer = layer_fixture();
    assert!(append_batch(batch(vec![feature(1)]), &[1, 2], &layer).is_err());
    assert!(append_batch(batch(vec![feature(1), feature(1)]), &[1, 2], &layer).is_err());
    let mut no_properties = feature(1);
    no_properties.as_object_mut().unwrap().remove("properties");
    let mut no_geometry = feature(1);
    no_geometry.as_object_mut().unwrap().remove("geometry");
    let mut missing_field = feature(1);
    missing_field["properties"]
        .as_object_mut()
        .unwrap()
        .remove("height");
    let mut extra_field = feature(1);
    extra_field["properties"]["unrequested"] = json!(1);
    let mut wrong_id = feature(1);
    wrong_id["id"] = json!("1");
    let mut wrong_property_id = feature(1);
    wrong_property_id["properties"]["OBJECTID"] = json!(2);
    for f in [
        no_properties,
        no_geometry,
        missing_field,
        extra_field,
        wrong_id,
        wrong_property_id,
        feature(2),
    ] {
        assert!(
            append_batch(batch(vec![f.clone()]), &[1], &layer).is_err(),
            "{f}"
        );
    }
    for geometry in [
        json!({"type":"Point","coordinates":[1,2,3]}),
        json!({"type":"Point","coordinates":[]}),
        json!({"type":"Point","coordinates":[1,null]}),
        json!({"type":"Point","coordinates":[1,"2"]}),
        json!({"type":"MultiPoint","coordinates":[[1,2]]}),
        json!({"type":"GeometryCollection","geometries":[]}),
    ] {
        let mut f = feature(1);
        f["geometry"] = geometry;
        assert!(append_batch(batch(vec![f]), &[1], &layer).is_err());
    }
    let mut truncated = batch(vec![feature(1)]);
    truncated["exceededTransferLimit"] = json!(true);
    assert!(append_batch(truncated, &[1], &layer).is_err());
    // Even exact ID/count coverage must not override a service truncation flag.
    let mut nested_truncation = batch(vec![feature(1)]);
    nested_truncation["properties"] = json!({"exceededTransferLimit":true});
    assert!(append_batch(nested_truncation.clone(), &[1], &layer).is_err());
    nested_truncation["properties"]["exceededTransferLimit"] = json!(false);
    assert_eq!(
        append_batch(nested_truncation, &[1], &layer).unwrap(),
        vec![feature(1)]
    );
    let mut other_crs = batch(vec![feature(1)]);
    other_crs["crs"] = json!({"type":"name","properties":{"name":"EPSG:3857"}});
    assert!(append_batch(other_crs, &[1], &layer).is_err());
}

#[test]
fn post_receipts_distinguish_identical_urls_by_exact_parameters_and_cover_every_id() {
    let source = source_fixture(&[1, 2, 9]);
    source.validate(3).unwrap();
    assert_eq!(source.pages[0].url, source.pages[1].url);
    assert_ne!(source.pages[0].parameters, source.pages[1].parameters);
    let snapshot = source.arcgis.as_ref().unwrap();
    assert_eq!(snapshot.id_receipts.len(), 2);
    assert_eq!(snapshot.count_receipts.len(), 2);
    assert_eq!(snapshot.id_receipts[0].url, snapshot.count_receipts[0].url);
    assert_ne!(
        snapshot.id_receipts[0].parameters,
        snapshot.count_receipts[0].parameters
    );
    assert_ne!(
        snapshot.id_receipts[0].sha256,
        snapshot.count_receipts[0].sha256
    );
    let encoded = serde_json::to_value(&source).unwrap();
    assert_eq!(encoded["pages"][0]["parameters"]["objectIds"], "1,2");
    assert_eq!(encoded["pages"][1]["parameters"]["objectIds"], "9");
    let decoded: Provenance = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, source);
    decoded.validate(3).unwrap();
}

#[test]
fn zero_matches_requires_both_count_and_id_receipts_but_no_feature_batches() {
    let source = source_fixture(&[]);
    source.validate(0).unwrap();
    assert!(source.pages.is_empty());
    assert!(source.arcgis.as_ref().unwrap().object_ids.is_empty());
    let mut missing = source.clone();
    missing.arcgis.as_mut().unwrap().count_receipts.clear();
    assert!(missing.validate(0).is_err());
    let mut unexpected_batch = source;
    unexpected_batch
        .pages
        .push(receipt(batch_parameters(&[]), 0, batch(vec![])));
    assert!(unexpected_batch.validate(0).is_err());
}

#[test]
fn provenance_rejects_truncated_duplicated_or_tampered_query_evidence() {
    type Mutation = (&'static str, fn(&mut Provenance));
    let mutations: &[Mutation] = &[
        ("missing last batch", |p| {
            p.pages.pop();
        }),
        ("duplicated batch", |p| {
            p.pages[1] = p.pages[0].clone();
        }),
        ("batch order", |p| p.pages.swap(0, 1)),
        ("wrong object IDs", |p| {
            p.pages[0]
                .parameters
                .as_mut()
                .unwrap()
                .insert("objectIds".into(), "1,9".into());
        }),
        ("extra token", |p| {
            p.pages[0]
                .parameters
                .as_mut()
                .unwrap()
                .insert("token".into(), "synthetic".into());
        }),
        ("lossy fields", |p| {
            p.pages[0]
                .parameters
                .as_mut()
                .unwrap()
                .insert("outFields".into(), "OBJECTID".into());
        }),
        ("wrong target CRS", |p| {
            p.pages[0]
                .parameters
                .as_mut()
                .unwrap()
                .insert("outSR".into(), "3857".into());
        }),
        ("unrecorded POST body", |p| p.pages[0].parameters = None),
        ("changed endpoint", |p| {
            p.pages[0].url = "https://other.example/query".into()
        }),
        ("invalid receipt digest", |p| {
            p.pages[0].sha256 = "g".repeat(64)
        }),
        ("empty receipt body", |p| p.pages[0].bytes = 0),
        ("oversized batch", |p| p.pages[0].returned = PAGE_SIZE + 1),
        ("layer batch limit", |p| {
            p.arcgis.as_mut().unwrap().layer.max_record_count = 1
        }),
        ("missing second ID check", |p| {
            p.arcgis.as_mut().unwrap().id_receipts.pop();
        }),
        ("missing second count check", |p| {
            p.arcgis.as_mut().unwrap().count_receipts.pop();
        }),
        ("count substituted for IDs", |p| {
            let snapshot = p.arcgis.as_mut().unwrap();
            snapshot.id_receipts[0] = snapshot.count_receipts[0].clone();
        }),
        ("different count after query", |p| {
            p.arcgis.as_mut().unwrap().count_receipts[1].returned = 4
        }),
        ("different ID count after query", |p| {
            p.arcgis.as_mut().unwrap().id_receipts[1].returned = 4
        }),
        ("changed membership region", |p| {
            p.arcgis.as_mut().unwrap().id_receipts[1]
                .parameters
                .as_mut()
                .unwrap()
                .insert("geometry".into(), "0,0,20,20".into());
        }),
        ("duplicate persisted IDs", |p| {
            p.arcgis.as_mut().unwrap().object_ids[1] = 1
        }),
        ("unsorted persisted IDs", |p| {
            p.arcgis.as_mut().unwrap().object_ids.swap(0, 1)
        }),
        ("unsafe persisted ID", |p| {
            p.arcgis.as_mut().unwrap().object_ids[2] = SAFE_ID + 1
        }),
        ("missing total", |p| p.number_matched = None),
        ("changed total", |p| p.number_matched = Some(2)),
        ("invented license", |p| {
            p.license_links.push("https://example.com/license".into())
        }),
        ("invalid timestamp", |p| {
            p.requested_at = "not a timestamp".into()
        }),
    ];
    let source = source_fixture(&[1, 2, 9]);
    source.validate(3).unwrap();
    for (name, mutate) in mutations {
        let mut changed = source.clone();
        mutate(&mut changed);
        assert!(changed.validate(3).is_err(), "{name}");
    }
    let mut excess_total = source;
    for page in &mut excess_total.pages {
        page.bytes = vector::MAX_BYTES / 2;
    }
    assert!(excess_total.validate(3).is_err());
}

#[tokio::test]
async fn arcgis_snapshot_reopens_with_full_provenance_and_untrusted_import_stays_untrusted() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let source = source_fixture(&[1, 2, 9]);
    let mut null_geometry = feature(9);
    null_geometry["geometry"] = Value::Null;
    let data = json!({"type":"FeatureCollection", "features":[feature(1),feature(2),null_geometry], "geodSource":source});
    let request = vector::ImportVectorRequest {
        name: "Synthetic ArcGIS snapshot".into(),
        text: serde_json::to_string(&data).unwrap(),
    };
    let untrusted = manager.import_vector(request.clone()).await.unwrap();
    assert!(untrusted.remote_source.is_none());
    let saved = manager
        .import_vector_source(request.clone(), Some(source.clone()))
        .await
        .unwrap();
    assert_eq!(saved.feature_count, 3);
    assert_eq!(saved.coordinate_count, 2);
    assert_eq!(saved.crs, "EPSG:4326");
    assert_eq!(
        manager.inspect_vector(&saved.id).await.unwrap().geojson,
        data
    );
    let mut changed = source.clone();
    changed.collection_title = "Changed after content serialization".into();
    assert!(manager
        .import_vector_source(request, Some(changed))
        .await
        .is_err());
    let service = service_fixture();
    let records = BTreeMap::from([(service.id.clone(), service.clone())]);
    manager.persist_feature_services(&records).await.unwrap();
    drop(manager);

    let manager = JobManager::open(dir.path()).await.unwrap();
    assert_eq!(manager.list_feature_services().await, vec![service.clone()]);
    let inspection = manager.inspect_vector(&saved.id).await.unwrap();
    assert_eq!(inspection.geojson, data);
    assert_eq!(inspection.asset.remote_source, Some(source));
    manager.forget_feature_service(&service.id).await.unwrap();
    assert!(manager.list_feature_services().await.is_empty());
    assert_eq!(
        manager.inspect_vector(&saved.id).await.unwrap().geojson,
        data
    );
}

#[tokio::test]
async fn legacy_ogc_registry_and_get_receipts_reopen_beside_arcgis_without_migration() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let old_id = Uuid::new_v4().to_string();
    let old_service_json = json!({
        "id":old_id,"name":"Synthetic legacy OGC","url":"https://example.com/api",
        "title":"Legacy OGC", "collections":[{
            "id":"lakes","title":"Lakes","description":"Synthetic legacy fixture",
            "itemsUrl":"https://example.com/api/collections/lakes/items?f=json","licenseLinks":[]
        }],"connectedAt":now()
    });
    let old_service: FeatureService = serde_json::from_value(old_service_json.clone()).unwrap();
    assert!(old_service.arcgis.is_none());
    assert!(old_service.collections[0].arcgis.is_none());
    assert_eq!(
        serde_json::to_value(&old_service).unwrap(),
        old_service_json
    );
    let old_source_json = json!({
        "serviceUrl":"https://example.com/api","serviceName":"Synthetic legacy OGC",
        "collectionId":"lakes","collectionTitle":"Lakes","licenseLinks":[],
        "requestedBounds":REGION,"areaGeometry":null,"requestedAt":now(),
        "pages":[{"url":"https://example.com/api/collections/lakes/items?f=json&bbox=0,0,10,10&limit=2",
                  "sha256":"c".repeat(64),"bytes":200,"returned":1}],
        "numberMatched":1,"featureCount":1,"selection":"bbox-full-features"
    });
    let old_source: Provenance = serde_json::from_value(old_source_json.clone()).unwrap();
    old_source.validate(1).unwrap();
    assert!(old_source.pages[0].parameters.is_none());
    assert_eq!(serde_json::to_value(&old_source).unwrap(), old_source_json);
    let old_data =
        json!({"type":"FeatureCollection","features":[feature(1)],"geodSource":old_source});
    let old_asset = manager
        .import_vector_source(
            vector::ImportVectorRequest {
                name: "Synthetic legacy snapshot".into(),
                text: serde_json::to_string(&old_data).unwrap(),
            },
            Some(old_source),
        )
        .await
        .unwrap();
    let arcgis_service = service_fixture();
    let records = BTreeMap::from([
        (old_id, old_service),
        (arcgis_service.id.clone(), arcgis_service),
    ]);
    manager.persist_feature_services(&records).await.unwrap();
    drop(manager);
    let manager = JobManager::open(dir.path()).await.unwrap();
    let restored: BTreeMap<_, _> = manager
        .list_feature_services()
        .await
        .into_iter()
        .map(|s| (s.id.clone(), s))
        .collect();
    assert_eq!(restored, records);
    assert_eq!(
        manager.inspect_vector(&old_asset.id).await.unwrap().geojson,
        old_data
    );
}

#[tokio::test]
async fn malformed_coordinate_structure_is_rejected_before_snapshot_persistence() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let source = source_fixture(&[1]);
    for geometry in [
        json!({"type":"Point","coordinates":[[1,2]]}),
        json!({"type":"Point","coordinates":[181,2]}),
        json!({"type":"LineString","coordinates":[[1,2]]}),
        json!({"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1]]]}),
    ] {
        let mut f = feature(1);
        f["geometry"] = geometry;
        let data = json!({"type":"FeatureCollection","features":[f],"geodSource":source});
        assert!(manager
            .import_vector_source(
                vector::ImportVectorRequest {
                    name: "Invalid synthetic snapshot".into(),
                    text: serde_json::to_string(&data).unwrap(),
                },
                Some(source.clone())
            )
            .await
            .is_err());
    }
}
