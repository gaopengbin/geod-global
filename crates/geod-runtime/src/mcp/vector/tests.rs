use super::*;
use crate::{
    mcp::{agent_read_call, Adapter},
    vector::ImportVectorRequest,
    JobManager,
};
use std::collections::VecDeque;

async fn source(manager: &JobManager) -> crate::vector::VectorAsset {
    let ring = (0..2000)
        .map(|i| {
            json!([
                -100.0 + (i % 20) as f64 * 0.01,
                30.0 + (i / 20) as f64 * 0.01
            ])
        })
        .collect::<Vec<_>>();
    let value = json!({"type":"FeatureCollection","features":[
        {"type":"Feature","id":"native-fixture/1","properties":{"name":"Test lake","a/b~c":{"nested":[1,2,3]},"longText":"湖🛰️".repeat(5000),"password":"private-password","downloadPath":"C:\\private\\file","location":"C:\\private\\other-file","ordinary":{"url":"https://example.com/?token=private-url","safe":2}},"geometry":{"type":"LineString","coordinates":ring}},
        {"type":"Feature","properties":null,"geometry":null}
    ]});
    manager
        .import_vector(ImportVectorRequest {
            name: "Vector read unit fixture".into(),
            text: value.to_string(),
        })
        .await
        .unwrap()
}
async fn call(backend: &Backend, name: &str, args: Value) -> Value {
    execute(backend, parse(name, args).unwrap()).await.unwrap()
}

#[tokio::test]
async fn verified_nodes_reconstruct_large_features_without_rounding_or_dropped_attributes() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let asset = source(&manager).await;
    let backend = Backend::Direct(manager.clone());
    let inspect = call(&backend, "geod_vector_inspect", json!({"id":asset.id})).await;
    assert_eq!(inspect["verified"], true);
    assert_eq!(inspect["sourceSha256"], asset.source_sha256);
    assert!(inspect.get("geojson").is_none());
    let page1 = call(
        &backend,
        "geod_vector_features",
        json!({"id":asset.id,"limit":1}),
    )
    .await;
    assert_eq!(page1["nextOffset"], 1);
    let page2 = call(
        &backend,
        "geod_vector_features",
        json!({"id":asset.id,"offset":1,"limit":1}),
    )
    .await;
    assert_eq!(page2["features"][0]["index"], 1);
    assert!(page2["features"][0]["geometryType"].is_null());
    assert!(page2["nextOffset"].is_null());
    let original = manager.inspect_vector(&asset.id).await.unwrap().geojson["features"][0].clone();
    let mut reconstructed = Value::Null;
    let mut todo = VecDeque::from([(String::new(), 0usize)]);
    let mut strings = std::collections::BTreeMap::<String, String>::new();
    while let Some((pointer, offset)) = todo.pop_front() {
        let node = call(
            &backend,
            "geod_vector_node",
            json!({"id":asset.id,"feature":0,"pointer":pointer,"offset":offset,"limit":100}),
        )
        .await;
        assert!(node.to_string().len() < 32768);
        assert_eq!(node["source"]["geojsonSha256"], asset.geojson_sha256);
        if offset == 0 {
            let target = if pointer.is_empty() {
                &mut reconstructed
            } else {
                reconstructed.pointer_mut(&pointer).unwrap()
            };
            *target = match node["nodeType"].as_str().unwrap() {
                "array" => json!(vec![
                    Value::Null;
                    node["page"]["total"].as_u64().unwrap() as usize
                ]),
                "object" => json!({}),
                "string" => json!(""),
                _ => node["value"].clone(),
            };
        }
        if let Some(text) = node["text"].as_str() {
            let assembled = strings.entry(pointer.clone()).or_default();
            assembled.push_str(text);
            *reconstructed.pointer_mut(&pointer).unwrap() = json!(assembled);
            if let Some(next) = node["nextOffset"].as_u64() {
                todo.push_back((pointer.clone(), next as usize));
            }
        }
        if let Some(entries) = node["page"]["entries"].as_array() {
            for entry in entries {
                let key = entry["key"]
                    .as_str()
                    .map(|key| key.replace('~', "~0").replace('/', "~1"))
                    .unwrap_or_else(|| entry["index"].to_string());
                let child_pointer = format!("{pointer}/{key}");
                let target = reconstructed.pointer_mut(&pointer).unwrap();
                let data = entry["data"].get("inline").cloned().unwrap_or(Value::Null);
                if let Some(key) = entry["key"].as_str() {
                    target.as_object_mut().unwrap().insert(key.into(), data);
                } else {
                    target[entry["index"].as_u64().unwrap() as usize] = data;
                }
                if entry["data"]["valueOmitted"] == true {
                    todo.push_back((child_pointer, 0));
                }
            }
            if let Some(next) = node["page"]["nextOffset"].as_u64() {
                todo.push_back((pointer.clone(), next as usize));
            }
        }
    }
    assert_eq!(reconstructed, original);
    assert!(manager.list().await.is_empty());
    assert!(manager.list_projects().await.is_empty());
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn agent_keeps_structural_pointers_but_explicitly_redacts_credentials_and_paths() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let asset = source(&manager).await;
    let node = agent_read_call(
        manager.clone(),
        "geod_vector_node",
        json!({"id":asset.id,"feature":0,"pointer":"/properties","limit":20}),
    )
    .await
    .unwrap();
    assert_eq!(node["pointer"], "/properties");
    assert_eq!(node["agentRedactionApplied"], true);
    for entry in node["page"]["entries"].as_array().unwrap() {
        if entry["key"] == "longText" {
            assert_eq!(
                entry["data"]["read"]["arguments"]["pointer"],
                "/properties/longText"
            );
        }
    }
    for pointer in [
        "/properties/password",
        "/properties/downloadPath",
        "/properties/ordinary/url",
    ] {
        let secret = agent_read_call(
            manager.clone(),
            "geod_vector_node",
            json!({"id":asset.id,"feature":0,"pointer":pointer}),
        )
        .await
        .unwrap();
        assert_eq!(secret["agentRedactionApplied"], true);
        assert!(!secret.to_string().contains("private-"));
        assert!(!secret.to_string().contains("C:\\private"));
    }
    assert!(!node.to_string().contains("private-password"));
    assert!(!node.to_string().contains("private-url"));
    for offset in [0, 1, 3, 5] {
        let secret = agent_read_call(
            manager.clone(),
            "geod_vector_node",
            json!({"id":asset.id,"feature":0,"pointer":"/properties/location","offset":offset}),
        )
        .await
        .unwrap();
        assert_eq!(secret["agentRedactionApplied"], true);
        assert!(secret.get("text").is_none());
    }
    let escaped = agent_read_call(
        manager.clone(),
        "geod_vector_node",
        json!({"id":asset.id,"feature":0,"pointer":"/properties/a~1b~0c"}),
    )
    .await
    .unwrap();
    assert_eq!(
        escaped["page"]["entries"][0]["data"]["inline"],
        json!([1, 2, 3])
    );
    assert_eq!(escaped["agentRedactionApplied"], false);
    assert!(manager.list().await.is_empty());
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn byte_bound_keeps_continuation_and_long_string_offsets_reach_every_original_character() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let properties = (0..100)
        .map(|n| (format!("field-{n:03}"), json!("x".repeat(900))))
        .collect::<serde_json::Map<_, _>>();
    let data = json!({"type":"Feature","geometry":null,"properties":{"wide":properties,"long":"a".repeat(1_100_000)}});
    let asset = manager
        .import_vector(ImportVectorRequest {
            name: "Bounded read fixture".into(),
            text: data.to_string(),
        })
        .await
        .unwrap();
    let mut offset = 0;
    let mut count = 0;
    loop {
        let page = manager
            .vector_node(
                &asset.id,
                Node {
                    feature: 0,
                    pointer: "/properties/wide".into(),
                    offset: Some(offset),
                    limit: Some(100),
                },
            )
            .await
            .unwrap();
        assert!(page.to_string().len() < 32768);
        count += page["page"]["entries"].as_array().unwrap().len();
        if let Some(next) = page["page"]["nextOffset"].as_u64() {
            assert!(next as usize > offset);
            offset = next as usize;
        } else {
            break;
        }
    }
    assert_eq!(count, 100);
    let tail = manager
        .vector_node(
            &asset.id,
            Node {
                feature: 0,
                pointer: "/properties/long".into(),
                offset: Some(1_099_999),
                limit: Some(1),
            },
        )
        .await
        .unwrap();
    assert_eq!(tail["text"], "a");
    assert_eq!(tail["complete"], true);
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn direct_and_loopback_reads_match_and_reopen_checks_changed_originals() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let asset = source(&manager).await;
    let service_id = uuid::Uuid::new_v4().to_string();
    // Explicit saved-metadata fixture; no assertion of live service availability.
    manager.inner.feature_services.lock().await.insert(
        service_id.clone(),
        crate::features::FeatureService {
            id: service_id.clone(),
            name: "Saved unit-test connection".into(),
            url: "https://demo.pygeoapi.io/stable".into(),
            title: "Saved test metadata".into(),
            connected_at: crate::now(),
            arcgis: None,
            overpass: None,
            wfs: None,
            collections: vec![crate::features::Collection {
                id: "lakes".into(),
                title: "Test collection".into(),
                description: "Saved declaration only".into(),
                items_url: "https://demo.pygeoapi.io/stable/collections/lakes/items".into(),
                license_links: vec![],
                arcgis: None,
                wfs: None,
            }],
        },
    );
    let direct = Backend::Direct(manager.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server_manager = manager.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, crate::service::router(server_manager))
            .await
            .unwrap();
    });
    let http = Backend::Server {
        base,
        client: reqwest::Client::builder().no_proxy().build().unwrap(),
    };
    for (name, args) in [
        ("geod_feature_services", json!({})),
        ("geod_vectors_list", json!({"limit":1})),
        (
            "geod_feature_collections",
            json!({"id":service_id,"limit":1}),
        ),
        ("geod_vector_inspect", json!({"id":asset.id})),
        (
            "geod_vector_features",
            json!({"id":asset.id,"offset":1,"limit":1}),
        ),
        (
            "geod_vector_node",
            json!({"id":asset.id,"feature":0,"pointer":"/properties/a~1b~0c"}),
        ),
        (
            "geod_vector_node",
            json!({"id":asset.id,"feature":0,"pointer":"/geometry/coordinates","offset":100,"limit":100}),
        ),
        (
            "geod_vector_node",
            json!({"id":asset.id,"feature":0,"pointer":"/properties/longText","offset":4096}),
        ),
    ] {
        assert_eq!(
            call(&direct, name, args.clone()).await,
            call(&http, name, args).await
        );
    }
    let adapter = Adapter::new(direct, false);
    assert!(adapter.call("geod_feature_query", json!({})).await.is_err());
    server.abort();
    let _ = server.await;
    adapter.shutdown().await.unwrap();
    drop(adapter);
    drop(manager);
    let manager = JobManager::open(dir.path()).await.unwrap();
    assert_eq!(
        manager.vector_metadata(&asset.id).await.unwrap()["verified"],
        true
    );
    let registry: Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("vectors.json")).unwrap()).unwrap();
    let path = registry[&asset.id]["path"].as_str().unwrap();
    let mut bytes = std::fs::read(path).unwrap();
    let index = bytes.iter().position(|b| *b == b'T').unwrap();
    bytes[index] = b'X';
    std::fs::write(path, bytes).unwrap();
    assert_eq!(
        manager.vector_metadata_list(Page::default()).await.unwrap()["verified"],
        false
    );
    for name in [
        "geod_vector_inspect",
        "geod_vector_features",
        "geod_vector_node",
    ] {
        let mut args = json!({"id":asset.id});
        if name == "geod_vector_node" {
            args["feature"] = json!(0);
        }
        assert!(agent_read_call(manager.clone(), name, args)
            .await
            .unwrap_err()
            .contains("changed after opening"));
    }
    manager.shutdown().await.unwrap();
}

#[test]
fn read_arguments_never_accept_paths_mutations_or_unbounded_pages() {
    let id = "11111111-1111-4111-8111-111111111111";
    for (name, args) in [
        ("geod_vector_inspect", json!({"id":"../secret"})),
        ("geod_vectors_list", json!({"limit":101})),
        ("geod_vector_features", json!({"id":id,"offset":1000001})),
        ("geod_vector_node", json!({"id":id,"feature":50000})),
        (
            "geod_vector_node",
            json!({"id":id,"feature":0,"pointer":"https://example.com"}),
        ),
        (
            "geod_vector_node",
            json!({"id":id,"feature":0,"pointer":"/bad~2escape"}),
        ),
        (
            "geod_vector_node",
            json!({"id":id,"feature":0,"path":"C:/private"}),
        ),
        ("geod_feature_collections", json!({"id":id,"query":true})),
    ] {
        assert!(parse(name, args).is_err());
    }
}
