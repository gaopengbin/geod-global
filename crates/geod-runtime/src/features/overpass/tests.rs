use super::*;

// These are synthetic contract fixtures, not captured OSM provider data.
// The default test suite never contacts a public Overpass endpoint.
const ROOT: &str = "https://example.com/api/interpreter";
const REGION: [f64; 4] = [13.3773, 52.5167, 13.3783, 52.5174];
const TIMESTAMP: &str = "2026-10-02T13:00:00Z";
const COPYRIGHT: &str = "Synthetic fixture for data from www.openstreetmap.org under ODbL.";

fn node(id: u64, lon: f64, lat: f64) -> Value {
    json!({"type":"node","id":id,"lat":lat,"lon":lon,
        "version":2,"timestamp":TIMESTAMP,"changeset":9,
        "uid":999,"user":"Synthetic mapper"})
}

fn marker(nodes: usize, ways: usize, relations: usize) -> Value {
    json!({"type":"count","id":0,"tags":{
        "nodes":nodes.to_string(),"ways":ways.to_string(),
        "relations":relations.to_string(),"total":(nodes+ways+relations).to_string()}})
}

fn envelope(elements: Vec<Value>) -> Value {
    json!({"version":0.6,"generator":"Overpass API synthetic test fixture",
        "osm3s":{"timestamp_osm_base":TIMESTAMP,"copyright":COPYRIGHT},
        "elements":elements})
}

fn response_fixture() -> Value {
    let mut selected_node = node(7, 13.3776, 52.5170);
    selected_node["tags"] = json!({"building":"hut","name":"Synthetic 小屋"});
    let points = [
        (11, 13.3775, 52.5168),
        (12, 13.3785, 52.5168),
        (13, 13.3785, 52.5172),
        (14, 13.3775, 52.5172),
    ];
    let mut geometry: Vec<Value> = points
        .iter()
        .map(|(_, lon, lat)| json!({"lat":lat,"lon":lon}))
        .collect();
    geometry.push(geometry[0].clone());
    // A node and way may legitimately share the numeric ID 7.
    let selected_way = json!({"type":"way","id":7,"nodes":[11,12,13,14,11],
        "geometry":geometry,"tags":{"building":"yes","name":"Synthetic courtyard"},
        "version":3,"timestamp":TIMESTAMP,"changeset":10,
        "uid":999,"user":"Synthetic mapper"});
    let mut elements = vec![selected_node, selected_way, marker(1, 1, 0)];
    elements.extend(points.iter().map(|(id, lon, lat)| node(*id, *lon, *lat)));
    elements.push(marker(4, 0, 0));
    envelope(elements)
}

fn provenance_fixture(raw: &Value, bytes: &[u8]) -> Provenance {
    let (_, selected, dependencies) = split_response(raw).unwrap();
    let (generator, api_version, data_timestamp, copyright_text) = identity(raw).unwrap();
    Provenance {
        service_url: ROOT.into(),
        service_name: "Synthetic OSM endpoint".into(),
        preset: "buildings".into(),
        preset_title: "Buildings".into(),
        requested_bounds: REGION,
        area_geometry: None,
        requested_at: TIMESTAMP.into(),
        query: query_text("buildings", REGION).unwrap(),
        response_sha256: hash(bytes),
        bytes: bytes.len(),
        element_counts: selected,
        dependency_counts: dependencies,
        data_timestamp,
        generator,
        api_version,
        copyright_text,
        selection: "overpass-bbox-full-geometry".into(),
    }
}

fn service_fixture() -> FeatureService {
    FeatureService {
        id: Uuid::new_v4().to_string(),
        name: "Synthetic OSM endpoint".into(),
        title: "Synthetic OSM endpoint".into(),
        url: ROOT.into(),
        collections: collections(&endpoint(ROOT).unwrap()),
        connected_at: TIMESTAMP.into(),
        arcgis: None,
        wfs: None,
        overpass: Some(Service {
            generator: "Overpass API synthetic test fixture".into(),
            api_version: 0.6,
            metadata_sha256: "a".repeat(64),
            copyright_text: COPYRIGHT.into(),
        }),
    }
}

#[test]
fn selected_roots_and_closure_dependencies_have_separate_typed_completion_counts() {
    let raw = response_fixture();
    let (normalized, selected, dependencies) = split_response(&raw).unwrap();
    assert_eq!(
        selected,
        Counts {
            nodes: 1,
            ways: 1,
            relations: 0,
            total: 2
        }
    );
    assert_eq!(
        dependencies,
        Counts {
            nodes: 4,
            ways: 0,
            relations: 0,
            total: 4
        }
    );
    assert_eq!(normalized["elements"].as_array().unwrap().len(), 6);
    assert_eq!(normalized["elements"][0], raw["elements"][0]);
    assert_eq!(normalized["elements"][1], raw["elements"][1]);
    assert_eq!(normalized["elements"][2], raw["elements"][3]);
    assert_eq!(normalized["osm3s"], raw["osm3s"]);
    assert_eq!(normalized["elements"][1]["geometry"][1]["lon"], 13.3785);
    assert!(
        normalized["elements"][1]["geometry"][1]["lon"]
            .as_f64()
            .unwrap()
            > REGION[2],
        "full outside-AOI geometry must be retained"
    );
    // Normalization is in-memory only: original completion markers are retained.
    assert_eq!(raw["elements"][2]["type"], "count");
    assert_eq!(raw["elements"][7]["type"], "count");
}

#[test]
fn complete_empty_result_requires_both_zero_sentinels() {
    let raw = envelope(vec![marker(0, 0, 0), marker(0, 0, 0)]);
    let (normalized, selected, dependencies) = split_response(&raw).unwrap();
    assert_eq!(selected, Counts::default());
    assert_eq!(dependencies, Counts::default());
    assert_eq!(normalized["elements"], json!([]));
    for incomplete in [envelope(vec![]), envelope(vec![marker(0, 0, 0)])] {
        assert!(split_response(&incomplete).is_err());
    }
    let bytes = serde_json::to_vec(&raw).unwrap();
    provenance_fixture(&raw, &bytes)
        .validate_raw(&bytes, &raw)
        .unwrap();
}

#[test]
fn matching_group_counts_do_not_excuse_missing_way_or_nested_relation_dependencies() {
    let mut missing_node = response_fixture();
    missing_node["elements"].as_array_mut().unwrap().remove(3);
    missing_node["elements"][6] = marker(3, 0, 0);
    assert!(split_response(&missing_node)
        .unwrap_err()
        .contains("dependency"));
    let relation = json!({"type":"relation","id":20,"tags":{"type":"site","building":"yes"},
        "members":[{"type":"relation","ref":21,"role":"part"}]});
    let missing_relation = envelope(vec![relation, marker(0, 0, 1), marker(0, 0, 0)]);
    assert!(split_response(&missing_relation)
        .unwrap_err()
        .contains("dependency"));
}

#[test]
fn partial_groups_and_repeated_or_malformed_sentinels_never_count_as_completion() {
    type Mutation = (&'static str, fn(&mut Value));
    let cases: &[Mutation] = &[
        ("missing final count", |v| {
            v["elements"].as_array_mut().unwrap().pop();
        }),
        ("missing selected count", |v| {
            v["elements"].as_array_mut().unwrap().remove(2);
        }),
        ("extra count", |v| {
            v["elements"].as_array_mut().unwrap().push(marker(0, 0, 0));
        }),
        ("content after final count", |v| {
            v["elements"].as_array_mut().unwrap().push(node(99, 1., 2.));
        }),
        ("missing selected object", |v| {
            v["elements"].as_array_mut().unwrap().remove(0);
        }),
        ("missing dependency", |v| {
            v["elements"].as_array_mut().unwrap().remove(3);
        }),
        ("wrong type tally with same total", |v| {
            v["elements"][2] = marker(2, 0, 0);
        }),
        ("wrong total", |v| {
            v["elements"][2]["tags"]["total"] = json!("3");
        }),
        ("numeric count instead of string", |v| {
            v["elements"][2]["tags"]["nodes"] = json!(1);
        }),
        ("negative count", |v| {
            v["elements"][2]["tags"]["nodes"] = json!("-1");
        }),
        ("oversized count", |v| {
            v["elements"][2]["tags"]["nodes"] = json!((vector::MAX_FEATURES + 1).to_string());
        }),
        ("nonzero count identity", |v| {
            v["elements"][2]["id"] = json!(8);
        }),
        ("unknown count field", |v| {
            v["elements"][2]["tags"]["areas"] = json!("0");
        }),
        ("same numeric ID and kind across groups", |v| {
            v["elements"][3]["id"] = json!(7);
        }),
        ("repeated dependency identity", |v| {
            v["elements"][4]["id"] = json!(11);
        }),
        ("zero object identity", |v| {
            v["elements"][0]["id"] = json!(0);
        }),
        ("unsafe object identity", |v| {
            v["elements"][0]["id"] = json!(9_007_199_254_740_992_u64);
        }),
        ("derived area instead of OSM element", |v| {
            v["elements"][0]["type"] = json!("area");
        }),
        ("timeout remark despite matching counts", |v| {
            v["remark"] = json!("runtime error: Query timed out");
        }),
        ("non-string failure remark", |v| {
            v["remark"] = json!({"error":"failure"});
        }),
        ("error field", |v| {
            v["error"] = json!("failed");
        }),
    ];
    for (label, mutate) in cases {
        let mut raw = response_fixture();
        mutate(&mut raw);
        assert!(split_response(&raw).is_err(), "{label}");
    }
    let mut empty_remark = response_fixture();
    empty_remark["remark"] = json!("");
    split_response(&empty_remark).unwrap();
    let bytes = serde_json::to_vec(&response_fixture()).unwrap();
    assert!(serde_json::from_slice::<Value>(&bytes[..bytes.len() - 1]).is_err());
}

#[test]
fn provider_identity_requires_supported_version_dataset_time_and_osm_attribution() {
    let mut raw = response_fixture();
    let actual = identity(&raw).unwrap();
    assert_eq!(actual.1, 0.6);
    assert_eq!(actual.2, TIMESTAMP);
    for (key, value) in [
        ("generator", json!("Unrelated API")),
        ("generator", json!("Overpass API\ncontrol")),
        ("version", json!(0.7)),
        ("version", json!("0.6")),
    ] {
        let mut changed = raw.clone();
        changed[key] = value;
        assert!(identity(&changed).is_err(), "{key}");
    }
    for key in ["timestamp_osm_base", "copyright"] {
        let mut changed = raw.clone();
        changed["osm3s"].as_object_mut().unwrap().remove(key);
        assert!(identity(&changed).is_err(), "missing {key}");
    }
    raw["osm3s"]["timestamp_osm_base"] = json!("not a timestamp");
    assert!(identity(&raw).is_err());
    raw = response_fixture();
    raw["osm3s"]["copyright"] = json!("Unspecified map content");
    assert!(identity(&raw).is_err());
}

#[test]
fn all_presets_use_bounded_full_geometry_and_independently_counted_member_closure() {
    let endpoint = endpoint(ROOT).unwrap();
    let available = collections(&endpoint);
    assert_eq!(
        available.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        vec!["buildings", "roads", "water", "landuse", "pois"]
    );
    for collection in available {
        assert_eq!(collection.items_url, ROOT);
        assert_eq!(collection.license_links, vec![LICENSE]);
        assert!(collection.arcgis.is_none());
        let query = query_text(&collection.id, REGION).unwrap();
        assert!(query.starts_with("[out:json][timeout:25][maxsize:16777216];("));
        assert!(query.contains("(52.5167,13.3773,52.5174,13.3783)"));
        assert!(query.ends_with(".selected out meta geom;.selected out count;.selected >> ->.dependencies;(.dependencies; - .selected;)->.dependencies;.dependencies out meta geom;.dependencies out count;"));
        assert!(!query.contains("geom("), "output must not crop geometry");
        assert!(
            !query.contains("center"),
            "output must not replace geometry with centers"
        );
    }
    let buildings = query_text("buildings", REGION).unwrap();
    assert!(buildings.contains("nwr[building][building!=no]"));
    assert!(buildings.contains("nwr[\"building:part\"][\"building:part\"!=no]"));
    assert!(query_text("custom;out;", REGION).is_err());
    assert_eq!(PROBE, "[out:json][timeout:5][maxsize:1048576];out count;");
}

#[test]
fn query_limits_and_endpoint_policy_reject_unsafe_or_unbounded_inputs() {
    query_bounds(REGION).unwrap();
    for region in [
        [0., 0., 1., 1.],
        [0., 80., 1.1, 80.1],
        [0., 0., 0., 0.1],
        [170., 0., -170., 1.],
        [181., 0., 182., 0.01],
        [0., 90., 0.01, 91.],
        [f64::NAN, 0., 0.01, 0.01],
        [0., 0., f64::INFINITY, 0.01],
    ] {
        assert!(query_bounds(region).is_err(), "{region:?}");
    }
    endpoint(ROOT).unwrap();
    for url in [
        "http://example.com/api/interpreter",
        "https://example.com/api/interpreter?token=synthetic",
        "https://user:synthetic@example.com/api/interpreter",
        "https://127.0.0.1/api/interpreter",
        "https://localhost/api/interpreter",
        "https://example.com/api/interpreter#fragment",
    ] {
        assert!(endpoint(url).is_err(), "{url}");
    }
}

#[test]
fn saved_service_cannot_change_preset_scope_endpoint_or_provider_identity() {
    let source = service_fixture();
    validate_service(&source).unwrap();
    type Mutation = (&'static str, fn(&mut FeatureService));
    let cases: &[Mutation] = &[
        ("missing provider", |s| {
            s.overpass = None;
        }),
        ("missing preset", |s| {
            s.collections.pop();
        }),
        ("different preset endpoint", |s| {
            s.collections[0].items_url = "https://other.example/interpreter".into();
        }),
        ("unrecorded preset", |s| {
            s.collections[0].id = "custom".into();
        }),
        ("invented license", |s| {
            s.collections[0].license_links = vec!["https://example.com/license".into()];
        }),
        ("non-OSM generator", |s| {
            s.overpass.as_mut().unwrap().generator = "Other provider".into();
        }),
        ("unsupported API", |s| {
            s.overpass.as_mut().unwrap().api_version = 0.7;
        }),
        ("invalid probe digest", |s| {
            s.overpass.as_mut().unwrap().metadata_sha256 = "g".repeat(64);
        }),
        ("missing ODbL attribution", |s| {
            s.overpass.as_mut().unwrap().copyright_text = "Copyright example.com".into();
        }),
    ];
    for (label, mutate) in cases {
        let mut changed = source.clone();
        mutate(&mut changed);
        assert!(validate_service(&changed).is_err(), "{label}");
    }
}

#[test]
fn exact_original_bytes_and_provenance_survive_roundtrip_and_tampering_is_rejected() {
    let raw = response_fixture();
    let bytes = serde_json::to_vec_pretty(&raw).unwrap();
    let source = provenance_fixture(&raw, &bytes);
    source.validate_raw(&bytes, &raw).unwrap();
    assert_eq!(source.element_counts.total, 2);
    assert_eq!(source.dependency_counts.total, 4);
    let encoded = serde_json::to_value(&source).unwrap();
    assert_eq!(encoded["requestedBounds"], json!(REGION));
    assert_eq!(encoded["responseSha256"], hash(&bytes));
    assert_eq!(encoded["elementCounts"]["total"], 2);
    assert_eq!(encoded["dependencyCounts"]["total"], 4);
    let decoded: Provenance = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(decoded, source);
    let mut unknown = encoded;
    unknown["accessToken"] = json!("synthetic");
    assert!(serde_json::from_value::<Provenance>(unknown).is_err());
    let compact = serde_json::to_vec(&raw).unwrap();
    assert!(
        source.validate_raw(&compact, &raw).is_err(),
        "equivalent reserialization is not the raw source"
    );
    let mut changed_data = raw.clone();
    changed_data["elements"][0]["tags"]["name"] = json!("Changed");
    assert!(source
        .validate_raw(
            &serde_json::to_vec_pretty(&changed_data).unwrap(),
            &changed_data
        )
        .is_err());
    assert!(
        source.validate(6).is_err(),
        "dependencies must not inflate selected feature count"
    );
    type Mutation = (&'static str, fn(&mut Provenance));
    let cases: &[Mutation] = &[
        ("unrecorded query change", |p| {
            p.query.push_str("out;");
        }),
        ("different selection bounds", |p| {
            p.requested_bounds[0] -= 0.0001;
        }),
        ("different preset", |p| {
            p.preset = "roads".into();
        }),
        ("different preset title", |p| {
            p.preset_title = "Changed".into();
        }),
        ("different digest", |p| {
            p.response_sha256 = "b".repeat(64);
        }),
        ("invalid digest", |p| {
            p.response_sha256 = "g".repeat(64);
        }),
        ("different byte count", |p| {
            p.bytes += 1;
        }),
        ("changed source total", |p| {
            p.element_counts.nodes += 1;
            p.element_counts.total += 1;
        }),
        ("changed dependency total", |p| {
            p.dependency_counts.nodes -= 1;
            p.dependency_counts.total -= 1;
        }),
        ("invalid request time", |p| {
            p.requested_at = "yesterday".into();
        }),
        ("changed dataset time", |p| {
            p.data_timestamp = "2026-10-02T12:00:00Z".into();
        }),
        ("changed generator", |p| {
            p.generator = "Overpass API different build".into();
        }),
        ("unsupported API", |p| {
            p.api_version = 0.7;
        }),
        ("changed attribution", |p| {
            p.copyright_text = "Different www.openstreetmap.org ODbL declaration".into();
        }),
        ("claim of clipping", |p| {
            p.selection = "polygon-clip".into();
        }),
        ("credential URL", |p| {
            p.service_url = "https://example.com/api/interpreter?token=synthetic".into();
        }),
    ];
    for (label, mutate) in cases {
        let mut changed = source.clone();
        mutate(&mut changed);
        assert!(changed.validate_raw(&bytes, &raw).is_err(), "{label}");
    }
}

#[tokio::test]
async fn original_osm_response_reopens_with_roots_only_and_exports_after_connection_removal() {
    let storage = tempfile::tempdir().unwrap();
    let destination = tempfile::tempdir().unwrap();
    let manager = JobManager::open(storage.path()).await.unwrap();
    let raw = response_fixture();
    let bytes = serde_json::to_vec_pretty(&raw).unwrap();
    let source = provenance_fixture(&raw, &bytes);
    let asset = manager
        .import_osm_source(bytes.clone(), source.clone())
        .await
        .unwrap();
    assert_eq!(
        asset.feature_count, 2,
        "four member nodes are dependencies, not selected features"
    );
    assert_eq!(asset.coordinate_count, 6);
    assert_eq!(asset.storage_mode, "managed");
    assert_eq!(asset.format, "overpass-json");
    assert_eq!(asset.osm_conversion, Some(2));
    assert_eq!(asset.source_sha256, hash(&bytes));
    assert_eq!(asset.osm_source, Some(source.clone()));
    assert!(asset.remote_source.is_none());
    let original_path = storage
        .path()
        .join("vectors")
        .join(format!("{}.json", asset.id));
    assert_eq!(tokio::fs::read(&original_path).await.unwrap(), bytes);
    let first = manager.inspect_vector(&asset.id).await.unwrap().geojson;
    assert_eq!(first["features"].as_array().unwrap().len(), 2);
    assert_eq!(first["features"][0]["id"], "node/7");
    assert_eq!(first["features"][1]["id"], "way/7");
    assert_eq!(
        first["features"][0]["properties"]["tags"],
        raw["elements"][0]["tags"]
    );
    assert_eq!(
        first["features"][1]["properties"]["osm_nodes"],
        raw["elements"][1]["nodes"]
    );
    assert_eq!(
        first["features"][1]["properties"]["osm_metadata"]["version"],
        3
    );
    assert_eq!(
        first["geodOsmSource"],
        serde_json::to_value(&source).unwrap()
    );
    let service = service_fixture();
    manager
        .persist_feature_services(&BTreeMap::from([(service.id.clone(), service.clone())]))
        .await
        .unwrap();
    drop(manager);

    let manager = JobManager::open(storage.path()).await.unwrap();
    assert_eq!(manager.list_feature_services().await, vec![service.clone()]);
    assert_eq!(
        manager.inspect_vector(&asset.id).await.unwrap().geojson,
        first
    );
    manager.forget_feature_service(&service.id).await.unwrap();
    assert!(manager.list_feature_services().await.is_empty());
    let out = destination.path().join("synthetic-osm.geojson");
    manager
        .export_vector_path(&asset.id, out.clone())
        .await
        .unwrap();
    let exported: Value = serde_json::from_slice(&tokio::fs::read(&out).await.unwrap()).unwrap();
    assert_eq!(exported, first);
    assert_eq!(tokio::fs::read(&original_path).await.unwrap(), bytes);
    // User-imported GeoJSON retains its fields without gaining trusted provenance.
    let untrusted = manager
        .import_vector(vector::ImportVectorRequest {
            name: "Synthetic reimport".into(),
            text: serde_json::to_string(&exported).unwrap(),
        })
        .await
        .unwrap();
    assert!(untrusted.osm_source.is_none());
    assert!(untrusted.remote_source.is_none());
    assert_eq!(
        manager.inspect_vector(&untrusted.id).await.unwrap().geojson,
        exported
    );
    let mut changed_bytes = bytes;
    changed_bytes.push(b'\n');
    tokio::fs::write(&original_path, changed_bytes)
        .await
        .unwrap();
    assert!(
        manager.inspect_vector(&asset.id).await.is_err(),
        "changed raw bytes require a fresh acquisition"
    );
}

#[tokio::test]
async fn incomplete_recursive_response_never_creates_a_vector_record_or_source_file() {
    let storage = tempfile::tempdir().unwrap();
    let manager = JobManager::open(storage.path()).await.unwrap();
    let valid = response_fixture();
    let valid_bytes = serde_json::to_vec(&valid).unwrap();
    let mut source = provenance_fixture(&valid, &valid_bytes);
    let mut missing = valid;
    missing["elements"].as_array_mut().unwrap().remove(3);
    missing["elements"][6] = marker(3, 0, 0);
    let bytes = serde_json::to_vec(&missing).unwrap();
    source.bytes = bytes.len();
    source.response_sha256 = hash(&bytes);
    source.dependency_counts.nodes = 3;
    source.dependency_counts.total = 3;
    assert!(manager.import_osm_source(bytes, source).await.is_err());
    assert!(manager.list_vectors().await.is_empty());
    let mut files = tokio::fs::read_dir(storage.path().join("vectors"))
        .await
        .unwrap();
    assert!(files.next_entry().await.unwrap().is_none());
}
