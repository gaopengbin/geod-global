use super::*;
fn collection() -> Value {
    json!({"type":"FeatureCollection","features":[
        {"type":"Feature","id":"p","properties":{"name":"北京 <script>"},"geometry":{"type":"Point","coordinates":[116.4,39.9,12.5]}},
        {"type":"Feature","properties":null,"geometry":{"type":"Polygon","coordinates":[[[0,0],[4,0],[4,4],[0,4],[0,0]],[[1,1],[1,2],[2,2],[2,1],[1,1]]]}},
        {"type":"Feature","properties":{},"geometry":null}
    ]})
}
#[test]
fn geojson_preserves_id_properties_height_holes_and_computed_bounds() {
    let data = collection();
    let bytes = serde_json::to_vec(&data).unwrap();
    let i = normalize(&bytes, initial("test.geojson", "reference").unwrap()).unwrap();
    assert_eq!(i.geojson, data);
    assert_eq!(i.asset.feature_count, 3);
    assert_eq!(i.asset.coordinate_count, 11);
    assert_eq!(i.asset.bounds, Some([0.0, 0.0, 116.4, 39.9]));
    assert!(i.asset.license_url.is_none());
    for changed in [
        json!({"type":"FeatureCollection","features":[],"crs":{"name":"EPSG:3857"}}),
        json!({"type":"Feature","properties":{},"geometry":{"type":"Point","coordinates":[200,100]}}),
        json!({"type":"Feature","properties":{},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1]]]}}),
    ] {
        assert!(normalize(
            &serde_json::to_vec(&changed).unwrap(),
            initial("bad", "managed").unwrap()
        )
        .is_err());
    }
}
fn overpass() -> Value {
    json!({"generator":"Overpass API test fixture","osm3s":{"timestamp_osm_base":"2025-06-01T00:00:00Z"},"elements":[
        {"type":"node","id":1,"lat":1,"lon":2,"tags":{"amenity":"cafe"}},
        {"type":"way","id":2,"tags":{"highway":"residential"},"geometry":[{"lat":0,"lon":0},{"lat":2,"lon":2}]},
        {"type":"relation","id":3,"tags":{"type":"multipolygon","building":"yes"},"members":[
            {"type":"way","ref":4,"role":"outer","geometry":[{"lat":0,"lon":0},{"lat":0,"lon":4},{"lat":4,"lon":4}]},
            {"type":"way","ref":5,"role":"outer","geometry":[{"lat":0,"lon":0},{"lat":4,"lon":0},{"lat":4,"lon":4}]},
            {"type":"way","ref":6,"role":"inner","geometry":[{"lat":1,"lon":1},{"lat":2,"lon":1},{"lat":2,"lon":2},{"lat":1,"lon":2},{"lat":1,"lon":1}]}
        ]}
    ]})
}
#[test]
fn osm_joins_reversed_members_keeps_hole_tags_identity_and_timestamp() {
    let mut input = overpass();
    let i = normalize(
        &serde_json::to_vec(&input).unwrap(),
        initial("osm.json", "managed").unwrap(),
    )
    .unwrap();
    assert_eq!(i.asset.feature_count, 3);
    assert_eq!(
        i.asset.data_timestamp.as_deref(),
        Some("2025-06-01T00:00:00Z")
    );
    assert_eq!(
        i.geojson["features"][2]["geometry"]["coordinates"][0]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(i.geojson["features"][0]["id"], "node/1");
    assert_eq!(
        i.geojson["features"][0]["properties"]["tags"]["amenity"],
        "cafe"
    );
    input["remark"] = json!("runtime timeout");
    assert!(normalize(
        &serde_json::to_vec(&input).unwrap(),
        initial("partial", "managed").unwrap()
    )
    .is_err());
    input.as_object_mut().unwrap().remove("remark");
    input["elements"][2]["members"][1]["geometry"][2]["lon"] = json!(5);
    assert!(normalize(
        &serde_json::to_vec(&input).unwrap(),
        initial("open", "managed").unwrap()
    )
    .is_err());
}

#[test]
fn osm_legacy_conversion_keeps_existing_registered_content_identical() {
    let source = overpass();
    let old = osm::convert(&source).unwrap();
    assert_eq!(
        old,
        json!({
            "type":"FeatureCollection","features":[
                {"type":"Feature","id":"node/1","properties":{"osm_type":"node","osm_id":1,"tags":{"amenity":"cafe"}},"geometry":{"type":"Point","coordinates":[2,1]}},
                {"type":"Feature","id":"way/2","properties":{"osm_type":"way","osm_id":2,"tags":{"highway":"residential"}},"geometry":{"type":"LineString","coordinates":[[0,0],[2,2]]}},
                {"type":"Feature","id":"relation/3","properties":{"osm_type":"relation","osm_id":3,"tags":{"type":"multipolygon","building":"yes"}},"geometry":{"type":"MultiPolygon","coordinates":[[[[0,0],[0,4],[4,4],[4,0],[0,0]],[[1,1],[1,2],[2,2],[2,1],[1,1]]]]}}
            ],"attribution":"© OpenStreetMap contributors","license":"https://www.openstreetmap.org/copyright","data_timestamp":"2025-06-01T00:00:00Z"
        })
    );
    let current = osm::convert_current(&source).unwrap();
    for (old, current) in old["features"]
        .as_array()
        .unwrap()
        .iter()
        .zip(current["features"].as_array().unwrap())
    {
        assert_eq!(old["geometry"], current["geometry"]);
        assert_eq!(old["properties"]["tags"], current["properties"]["tags"]);
    }
    assert_eq!(
        current["features"][2]["properties"]["osm_members"],
        json!([
            {"type":"way","ref":4,"role":"outer"},
            {"type":"way","ref":5,"role":"outer"},
            {"type":"way","ref":6,"role":"inner"}
        ])
    );
}

#[test]
fn osm_current_way_classification_keeps_open_objects_and_explicit_area_semantics() {
    let mut source = overpass();
    source["elements"] = json!([]);
    let cases = [
        (json!({"building":"yes"}), "Polygon"),
        (json!({"building:part":"roof"}), "Polygon"),
        (json!({"landuse":"forest"}), "Polygon"),
        (json!({"natural":"wetland"}), "Polygon"),
        (json!({"natural":"water"}), "Polygon"),
        (json!({"waterway":"riverbank"}), "Polygon"),
        (json!({"waterway":"river"}), "LineString"),
        (json!({"natural":"coastline"}), "LineString"),
        (json!({"highway":"residential"}), "LineString"),
        (json!({"highway":"services"}), "Polygon"),
        (json!({"shop":"supermarket"}), "Polygon"),
        (json!({"tourism":"museum"}), "Polygon"),
        (json!({"office":"company"}), "Polygon"),
        (json!({"amenity":"cafe","area":"no"}), "LineString"),
        (json!({"highway":"pedestrian","area":"yes"}), "Polygon"),
        (json!({"leisure":"track"}), "Polygon"),
    ];
    for (index, (tags, _)) in cases.iter().enumerate() {
        source["elements"].as_array_mut().unwrap().push(json!({
            "type":"way","id":index + 1,"tags":tags,
            "geometry":[{"lon":0,"lat":0},{"lon":1,"lat":0},{"lon":1,"lat":1},{"lon":0,"lat":0}]
        }));
    }
    let converted = osm::convert_current(&source).unwrap();
    for (index, (_, expected)) in cases.iter().enumerate() {
        assert_eq!(converted["features"][index]["geometry"]["type"], *expected);
    }
    for tags in [
        json!({"amenity":"bench"}),
        json!({"leisure":"slipway"}),
        json!({"building":"yes"}),
    ] {
        source["elements"] = json!([{"type":"way","id":1,"tags":tags,
            "geometry":[{"lon":0,"lat":0},{"lon":1,"lat":1}]}]);
        assert_eq!(
            osm::convert_current(&source).unwrap()["features"][0]["geometry"]["type"],
            "LineString"
        );
    }
    source["elements"][0]["tags"]["area"] = json!("yes");
    assert!(osm::convert_current(&source)
        .unwrap_err()
        .contains("closed ring"));
}

fn dependent_overpass() -> Value {
    json!({"generator":"Overpass API test fixture","osm3s":{"timestamp_osm_base":"2025-06-01T00:00:00Z"},"elements":[
        {"type":"relation","id":10,"version":7,"timestamp":"2025-05-31T20:00:00Z","tags":{"type":"site","amenity":"university","name":"Unicode 校园"},"members":[
            {"type":"node","ref":1,"role":"entrance"},
            {"type":"way","ref":2,"role":"forward"},
            {"type":"relation","ref":11,"role":"campus"}
        ]},
        {"type":"relation","id":11,"tags":{"type":"waterway","waterway":"river"},"members":[
            {"type":"way","ref":2,"role":"main_stream"},
            {"type":"node","ref":1,"role":"spring"}
        ]},
        {"type":"node","id":1,"lat":1,"lon":2,"tags":{"entrance":"main"}},
        {"type":"way","id":2,"nodes":[1,3],"tags":{"waterway":"stream"}},
        {"type":"node","id":3,"lat":2,"lon":3}
    ]})
}

#[test]
fn osm_current_resolves_nested_relations_preserves_order_and_does_not_promote_dependencies() {
    let source = dependent_overpass();
    let converted = osm::convert_current_selected(&source, 1).unwrap();
    assert_eq!(converted["features"].as_array().unwrap().len(), 1);
    let feature = &converted["features"][0];
    assert_eq!(feature["id"], "relation/10");
    assert_eq!(feature["properties"]["tags"], source["elements"][0]["tags"]);
    assert_eq!(
        feature["properties"]["osm_members"],
        source["elements"][0]["members"]
    );
    assert_eq!(
        feature["properties"]["osm_metadata"],
        json!({"version":7,"timestamp":"2025-05-31T20:00:00Z"})
    );
    assert_eq!(
        feature["geometry"],
        json!({"type":"GeometryCollection","geometries":[
            {"type":"Point","coordinates":[2,1]},
            {"type":"LineString","coordinates":[[2,1],[3,2]]},
            {"type":"GeometryCollection","geometries":[
                {"type":"LineString","coordinates":[[2,1],[3,2]]},
                {"type":"Point","coordinates":[2,1]}
            ]}
        ]})
    );
    let all = osm::convert_current(&source).unwrap();
    assert_eq!(all["features"][3]["properties"]["osm_nodes"], json!([1, 3]));
    assert_eq!(
        osm::convert_current_selected(&source, 0).unwrap()["features"],
        json!([])
    );
    assert!(osm::convert_current_selected(&source, 6).is_err());
}

#[test]
fn osm_current_rejects_partial_conflicting_or_cyclic_dependencies_without_dropping_features() {
    let source = dependent_overpass();
    let mut missing = source.clone();
    missing["elements"].as_array_mut().unwrap().remove(1);
    assert!(osm::convert_current_selected(&missing, 1)
        .unwrap_err()
        .contains("missing nested"));
    let mut missing_node = source.clone();
    missing_node["elements"].as_array_mut().unwrap().pop();
    assert!(osm::convert_current_selected(&missing_node, 1)
        .unwrap_err()
        .contains("missing node"));
    let mut cyclic = source.clone();
    cyclic["elements"][1]["members"] = json!([{"type":"relation","ref":10,"role":""}]);
    assert!(osm::convert_current_selected(&cyclic, 1)
        .unwrap_err()
        .contains("cycle"));
    let mut incomplete = source.clone();
    incomplete["elements"][3]["geometry"] = json!([{"lon":2,"lat":1}, null]);
    assert!(osm::convert_current_selected(&incomplete, 1).is_err());
    incomplete["elements"][3]["nodes"] = json!([1, 3, 4]);
    assert!(osm::convert_current_selected(&incomplete, 1)
        .unwrap_err()
        .contains("every referenced"));
    let mut conflicting = source.clone();
    conflicting["elements"][0]["members"][1]["geometry"] =
        json!([{"lon":2,"lat":1},{"lon":4,"lat":2}]);
    assert!(osm::convert_current_selected(&conflicting, 1)
        .unwrap_err()
        .contains("differs"));
    let mut duplicate = source.clone();
    duplicate["elements"]
        .as_array_mut()
        .unwrap()
        .push(source["elements"][2].clone());
    assert!(osm::convert_current_selected(&duplicate, 1)
        .unwrap_err()
        .contains("duplicate"));
    let mut partial = source;
    partial["remark"] = json!("runtime error: timed out");
    assert!(osm::convert_current_selected(&partial, 1)
        .unwrap_err()
        .contains("partial"));
}

#[test]
fn osm_current_accepts_complete_embedded_route_members_and_legacy_empty_outer_roles() {
    let mut source = overpass();
    source["elements"] = json!([{"type":"relation","id":1,"tags":{"type":"route","route":"road"},"members":[
        {"type":"way","ref":2,"role":"backward","geometry":[{"lon":0,"lat":0},{"lon":1,"lat":2}]},
        {"type":"node","ref":3,"role":"stop","lon":1,"lat":2}
    ]}]);
    let converted = osm::convert_current(&source).unwrap();
    assert_eq!(
        converted["features"][0]["geometry"]["geometries"][0]["coordinates"],
        json!([[0, 0], [1, 2]])
    );
    assert_eq!(
        converted["features"][0]["properties"]["osm_members"][0]["role"],
        "backward"
    );
    let mut source = overpass();
    source["elements"][2]["members"][0]["role"] = json!("");
    assert!(osm::convert_current(&source).is_ok());
    assert!(osm::convert(&source).is_err());
}

#[test]
fn osm_multipolygon_joins_node_identities_instead_of_coincident_coordinates() {
    let mut source = overpass();
    source["elements"] = json!([
        {"type":"relation","id":1,"tags":{"type":"multipolygon","building":"yes"},"members":[
            {"type":"way","ref":2,"role":"outer"},
            {"type":"way","ref":3,"role":"outer"}
        ]},
        {"type":"way","id":2,"nodes":[10,11,12],"geometry":[{"lon":0,"lat":0},{"lon":1,"lat":0},{"lon":1,"lat":1}]},
        {"type":"way","id":3,"nodes":[10,13,12],"geometry":[{"lon":0,"lat":0},{"lon":0,"lat":1},{"lon":1,"lat":1}]}
    ]);
    let converted = osm::convert_current_selected(&source, 1).unwrap();
    assert_eq!(
        converted["features"][0]["geometry"]["coordinates"][0][0],
        json!([[0, 0], [0, 1], [1, 1], [1, 0], [0, 0]])
    );
    // Same coordinate, different endpoint identity: do not invent a shared node.
    source["elements"][2]["nodes"][2] = json!(99);
    assert!(osm::convert_current_selected(&source, 1)
        .unwrap_err()
        .contains("connections"));
    source["elements"][2]["nodes"][2] = json!(12);
    source["elements"][2]["geometry"][2]["lon"] = json!(2);
    assert!(osm::convert_current_selected(&source, 1)
        .unwrap_err()
        .contains("inconsistent coordinates"));
    // A single geometrically closed way also needs matching endpoint IDs.
    source["elements"] = json!([
        {"type":"relation","id":1,"tags":{"type":"multipolygon","landuse":"forest"},"members":[{"type":"way","ref":2,"role":"outer"}]},
        {"type":"way","id":2,"nodes":[10,11,12,99],"geometry":[{"lon":0,"lat":0},{"lon":1,"lat":0},{"lon":1,"lat":1},{"lon":0,"lat":0}]}
    ]);
    assert!(osm::convert_current_selected(&source, 1)
        .unwrap_err()
        .contains("connections"));
}

#[test]
fn osm_embedded_way_coordinates_must_agree_with_available_node_dependencies() {
    let mut source = dependent_overpass();
    source["elements"][3]["geometry"] = json!([{"lon":2,"lat":1},{"lon":3.0,"lat":2.0}]);
    assert!(osm::convert_current_selected(&source, 1).is_ok());
    source["elements"][4]["lon"] = json!(4);
    assert!(osm::convert_current_selected(&source, 1)
        .unwrap_err()
        .contains("node dependency"));
}

#[test]
fn osm_current_validates_nonrendered_label_and_admin_centre_dependencies() {
    let mut source = overpass();
    let area = source["elements"][2].clone();
    source["elements"] = json!([area, {"type":"node","id":70,"lon":2,"lat":2}]);
    source["elements"][0]["members"]
        .as_array_mut()
        .unwrap()
        .push(json!({"type":"node","ref":70,"role":"label"}));
    let converted = osm::convert_current_selected(&source, 1).unwrap();
    assert_eq!(converted["features"].as_array().unwrap().len(), 1);
    assert_eq!(converted["features"][0]["geometry"]["type"], "MultiPolygon");
    let mut missing = source.clone();
    missing["elements"][1]
        .as_object_mut()
        .unwrap()
        .remove("lat");
    assert!(osm::convert_current_selected(&missing, 1).is_err());
    let mut invalid = source.clone();
    invalid["elements"][0]["members"][3]["role"] = json!("admin_centre");
    invalid["elements"][1]["lat"] = json!(91);
    assert!(osm::convert_current_selected(&invalid, 1).is_err());
    let mut conflicting = source.clone();
    conflicting["elements"][0]["members"][3]["lon"] = json!(3);
    conflicting["elements"][0]["members"][3]["lat"] = json!(2);
    assert!(osm::convert_current_selected(&conflicting, 1)
        .unwrap_err()
        .contains("differs"));
    // Keep legacy behavior unchanged for registrations using the old converter.
    assert!(osm::convert(&overpass()).is_ok());
}

#[test]
fn osm_current_validates_every_dependency_shape_without_promoting_it_to_a_feature() {
    let base = dependent_overpass();
    let extra = [
        json!({"type":"way","id":90,"nodes":[1,3],"geometry":[{"lon":2,"lat":1},null]}),
        json!({"type":"way","id":90,"nodes":[1,3,4],"geometry":[{"lon":2,"lat":1},{"lon":3,"lat":2}]}),
        json!({"type":"relation","id":90,"members":[{"type":"node","ref":1,"role":null}]}),
        json!({"type":"relation","id":90,"members":[{"type":"way","ref":2,"role":"","geometry":[{"lon":2,"lat":1},{"lon":99,"lat":2}]}]}),
    ];
    for element in extra {
        let mut source = base.clone();
        source["elements"]
            .as_array_mut()
            .unwrap()
            .push(element.clone());
        assert!(
            osm::convert_current_selected(&source, 1).is_err(),
            "{element}"
        );
    }
    assert_eq!(
        osm::convert_current_selected(&base, 1).unwrap()["features"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
#[tokio::test]
async fn reference_managed_source_checksums_and_registration_survive_restart() {
    let d = tempfile::tempdir().unwrap();
    let original = d.path().join("input.geojson");
    let raw = serde_json::to_vec(&collection()).unwrap();
    std::fs::write(&original, &raw).unwrap();
    let root = d.path().join("runtime");
    let manager = JobManager::open(&root).await.unwrap();
    let reference = manager
        .open_vector_path(original.clone(), false)
        .await
        .unwrap();
    let copy = manager
        .open_vector_path(original.clone(), true)
        .await
        .unwrap();
    assert_eq!(reference.storage_mode, "reference");
    assert_eq!(copy.storage_mode, "managed");
    assert_eq!(std::fs::read(&original).unwrap(), raw);
    assert_eq!(
        manager.inspect_vector(&reference.id).await.unwrap().geojson,
        collection()
    );
    let exported = d.path().join("exported.geojson");
    manager
        .export_vector_path(&reference.id, exported.clone())
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&std::fs::read(exported).unwrap()).unwrap(),
        collection()
    );
    assert!(manager
        .export_vector_path(&reference.id, original.clone())
        .await
        .is_err());
    drop(manager);
    let manager = JobManager::open(&root).await.unwrap();
    assert_eq!(manager.list_vectors().await.len(), 2);
    assert_eq!(
        manager.inspect_vector(&copy.id).await.unwrap().geojson,
        collection()
    );
    std::fs::write(&original, b"{}").unwrap();
    assert!(manager
        .inspect_vector(&reference.id)
        .await
        .unwrap_err()
        .contains("changed"));
    assert!(manager.inspect_vector(&copy.id).await.is_ok());
    manager.forget_vector(&reference.id).await.unwrap();
    assert!(original.exists());
    let managed = root.join("vectors").join(format!("{}.json", copy.id));
    std::fs::write(&managed, b"{}").unwrap();
    assert!(manager.inspect_vector(&copy.id).await.is_err());
}
#[test]
fn vector_size_coordinate_and_nested_geometry_limits_are_enforced() {
    assert!(normalize(
        &vec![b' '; MAX_BYTES + 1],
        initial("oversized", "managed").unwrap()
    )
    .is_err());
    let mut geometry = json!({"type":"Point","coordinates":[0,0]});
    for _ in 0..10 {
        geometry = json!({"type":"GeometryCollection","geometries":[geometry]});
    }
    let value = json!({"type":"Feature","properties":{},"geometry":geometry});
    assert!(normalize(
        &serde_json::to_vec(&value).unwrap(),
        initial("nested", "managed").unwrap()
    )
    .is_err());
    assert!(initial("../bad\nname", "managed").is_err());
}

#[test]
fn utf8_bom_keeps_original_checksum_and_all_seven_geometry_types_are_read() {
    let raw = json!({"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{},"geometry":{"type":"Point","coordinates":[0,0]}},
        {"type":"Feature","properties":{},"geometry":{"type":"MultiPoint","coordinates":[[0,0],[1,1]]}},
        {"type":"Feature","properties":{},"geometry":{"type":"LineString","coordinates":[[0,0],[1,1]]}},
        {"type":"Feature","properties":{},"geometry":{"type":"MultiLineString","coordinates":[[[0,0],[1,1]]]}},
        {"type":"Feature","properties":{},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,0]]]}},
        {"type":"Feature","properties":{},"geometry":{"type":"MultiPolygon","coordinates":[[[[0,0],[1,0],[1,1],[0,0]]]]}},
        {"type":"Feature","properties":{},"geometry":{"type":"GeometryCollection","geometries":[{"type":"Point","coordinates":[1,1,2]}]}}
    ]});
    let mut bytes = vec![0xef, 0xbb, 0xbf];
    bytes.extend(serde_json::to_vec(&raw).unwrap());
    let inspection = normalize(&bytes, initial("bom.geojson", "managed").unwrap()).unwrap();
    assert_eq!(inspection.geojson, raw);
    assert_eq!(inspection.asset.source_sha256, hash(&bytes));
    assert_eq!(inspection.asset.geometry_counts.len(), 7);
    assert_eq!(inspection.asset.coordinate_count, 16);
}

#[test]
fn large_integer_id_or_attribute_cannot_silently_round_in_the_webview() {
    let mut raw = collection();
    raw["features"][0]["id"] = json!(9_007_199_254_740_993_u64);
    assert!(normalize(
        &serde_json::to_vec(&raw).unwrap(),
        initial("large-id", "reference").unwrap()
    )
    .unwrap_err()
    .contains("encoded as strings"));
    raw["features"][0]["id"] = json!("9007199254740993");
    raw["features"][0]["properties"]["nested"] = json!({"values":[-9_007_199_254_740_993_i64]});
    assert!(normalize(
        &serde_json::to_vec(&raw).unwrap(),
        initial("large-attribute", "reference").unwrap()
    )
    .is_err());
    raw["features"][0]["properties"]["nested"] =
        json!({"values":["-9007199254740993",9_007_199_254_740_991_u64]});
    let inspection = normalize(
        &serde_json::to_vec(&raw).unwrap(),
        initial("string-identity", "reference").unwrap(),
    )
    .unwrap();
    assert_eq!(inspection.geojson, raw);
}

#[tokio::test]
async fn forged_registry_bounds_or_license_are_rejected_before_listing() {
    let d = tempfile::tempdir().unwrap();
    let manager = JobManager::open(d.path()).await.unwrap();
    let asset = manager
        .import_vector(ImportVectorRequest {
            name: "registry.geojson".into(),
            text: collection().to_string(),
        })
        .await
        .unwrap();
    let path = d.path().join("vectors.json");
    drop(manager);
    let original: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    for (key, value) in [
        ("bounds", json!([1, 2, 0, 1])),
        ("licenseUrl", json!("https://evil.example")),
        ("geometryCounts", json!({"Point":0})),
    ] {
        let mut changed = original.clone();
        changed[&asset.id]["asset"][key] = value;
        std::fs::write(&path, serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(JobManager::open(d.path())
            .await
            .err()
            .unwrap()
            .contains("metadata"));
    }
    std::fs::write(path, serde_json::to_vec(&original).unwrap()).unwrap();
    assert!(JobManager::open(d.path()).await.is_ok());
}

#[tokio::test]
async fn browser_vector_adapter_accepts_bounded_content_but_never_native_file_paths() {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let d = tempfile::tempdir().unwrap();
    let manager = JobManager::open(d.path()).await.unwrap();
    let app = crate::service::router(manager.clone());
    let request = |body: Value, client: bool| {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/vectors")
            .header("Host", "127.0.0.1:4318")
            .header("Origin", crate::service::ALLOWED_ORIGIN)
            .header("Content-Type", "application/json");
        if client {
            builder = builder.header("X-GeoD-Client", "geod-global");
        }
        builder.body(Body::from(body.to_string())).unwrap()
    };
    let denied = app
        .clone()
        .oneshot(request(
            json!({"name":"x","text":collection().to_string()}),
            false,
        ))
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let path = app
        .clone()
        .oneshot(request(
            json!({"name":"x","text":collection().to_string(),"path":"C:/private.geojson"}),
            true,
        ))
        .await
        .unwrap();
    assert_eq!(path.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(manager.list_vectors().await.is_empty());
    let mut data = collection();
    data["features"][0]["properties"]["long"] = json!("a".repeat(2 * 1024 * 1024));
    let response = app
        .oneshot(request(
            json!({"name":"large.geojson","text":data.to_string()}),
            true,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let asset: VectorAsset =
        serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap()).unwrap();
    assert_eq!(asset.storage_mode, "managed");
    assert_eq!(
        manager.inspect_vector(&asset.id).await.unwrap().geojson,
        data
    );
}
