use super::*;
use crate::mcp::{
    agent_read_call, parse_operation, tests::fake_backend, Adapter, READ_TOOLS, WRITE_TOOLS,
};
use std::{collections::HashSet, sync::Arc};
use tokio::sync::Mutex;

const ID: &str = "11111111-1111-4111-8111-111111111111";

#[test]
fn contracts_reject_foreign_transfer_inputs_and_readonly_metadata_mutations() {
    let digest = "a".repeat(64);
    for (name, value) in [
        ("geod_stac_snapshot", json!({"id":"../other"})),
        ("geod_stac_assets", json!({"id":digest.to_uppercase()})),
        ("geod_stac_assets", json!({"id":digest,"limit":101})),
        ("geod_stac_catalog", json!({"id":ID,"offset":1000001})),
        ("geod_stac_inspect", json!({"id":digest})),
        ("geod_stac_pixel", json!({"id":ID,"column":1.5,"row":0})),
        ("geod_stac_pixel", json!({"id":ID,"column":0,"row":-1})),
        (
            "geod_stac_pixel",
            json!({"id":ID,"column":0,"row":4294967296u64}),
        ),
        (
            "geod_stac_pixel",
            json!({"id":ID,"column":0,"row":0,"band":2}),
        ),
        (
            "geod_stac_connect",
            json!({"request":{"name":"source","url":"https://example.com/","kind":"api","headers":{"Authorization":"secret"}}}),
        ),
        (
            "geod_stac_search",
            json!({"request":{"connectionId":ID,"collectionId":"real-id","bounds":[-181,1,3,4]}}),
        ),
        (
            "geod_stac_search",
            json!({"request":{"connectionId":ID,"collectionId":"real-id","bounds":[1,2,3,4],"cursor":"../other"}}),
        ),
        (
            "geod_stac_search",
            json!({"request":{"connectionId":ID,"collectionId":"real-id","bounds":[1,2,3,4],"url":"https://other.example/search"}}),
        ),
        (
            "geod_stac_project_save",
            json!({"request":{"bounds":[1,2,3,4],"selections":[]}}),
        ),
        (
            "geod_stac_project_save",
            json!({"request":{"bounds":[1,2,3,4],"selections":[{"snapshotId":digest,"assetKey":"actual-key","href":"https://other.example/data.tif"}]}}),
        ),
        (
            "geod_stac_download",
            json!({"request":{"projectId":ID,"selections":[]}}),
        ),
        (
            "geod_stac_download",
            json!({"request":{"projectId":ID,"outputPath":"C:/private"}}),
        ),
    ] {
        assert!(parse_operation(name, value, true).is_err(), "{name}");
    }
    for (name, value) in [
        (
            "geod_stac_connect",
            json!({"request":{"name":"source","url":"https://example.com/","kind":"api"}}),
        ),
        (
            "geod_stac_search",
            json!({"request":{"connectionId":ID,"collectionId":"real-id","bounds":[1,2,3,4]}}),
        ),
        (
            "geod_stac_project_save",
            json!({"request":{"name":"project","bounds":[1,2,3,4],"selections":[{"snapshotId":digest,"assetKey":"original"}]}}),
        ),
        ("geod_stac_download", json!({"request":{"projectId":ID}})),
        ("geod_stac_forget", json!({"id":ID})),
    ] {
        assert!(parse_operation(name, value.clone(), true).is_ok(), "{name}");
        assert!(parse_operation(name, value, false).is_err(), "{name}");
    }
}

#[test]
fn metadata_reads_and_search_have_truthful_annotations_and_stable_names() {
    let all = crate::mcp::tools(true);
    let names: HashSet<_> = all.iter().map(|t| t.name.as_ref()).collect();
    assert_eq!(names.len(), READ_TOOLS.len() + WRITE_TOOLS.len());
    for t in all.iter().filter(|t| t.name.starts_with("geod_stac_")) {
        let a = t.annotations.as_ref().unwrap();
        assert_eq!(
            a.read_only_hint,
            Some(READ_TOOLS.contains(&t.name.as_ref()))
        );
        assert_eq!(
            a.open_world_hint,
            Some(matches!(
                t.name.as_ref(),
                "geod_stac_connect" | "geod_stac_search" | "geod_stac_download"
            ))
        );
        assert_eq!(a.destructive_hint, Some(t.name == "geod_stac_forget"));
    }
    let search = all.iter().find(|t| t.name == "geod_stac_search").unwrap();
    assert_eq!(
        search.annotations.as_ref().unwrap().idempotent_hint,
        Some(false)
    );
    let readonly = crate::mcp::tools(false);
    assert_eq!(
        readonly
            .iter()
            .filter(|t| t.name.starts_with("geod_stac_"))
            .count(),
        6
    );
}

#[tokio::test]
async fn agent_search_uses_only_explicit_metadata_router_with_bounded_registered_schema() {
    let directory = tempfile::tempdir().unwrap();
    let manager = crate::JobManager::open(directory.path()).await.unwrap();
    let definition = crate::mcp::agent_stac_search_definition();
    assert_eq!(definition["name"], "geod_stac_search");
    assert_eq!(
        definition["inputSchema"]["properties"]["request"]["properties"]["limit"]["maximum"],
        20
    );
    assert!(crate::agent_actions::definitions()
        .iter()
        .any(|d| d == &definition));
    let session = uuid::Uuid::new_v4().to_string();
    for request in [
        json!({"connectionId":ID,"collectionId":"real-id","bounds":[1,2,3,4],"limit":21}),
        json!({"connectionId":ID,"collectionId":"real-id","bounds":[1,2,3,4],"href":"https://other.example/file"}),
        json!({"connectionId":ID,"collectionId":"real-id","bounds":[1,2,3,4]}),
    ] {
        assert!(crate::agent_actions::call(
            manager.clone(),
            &session,
            "geod_stac_search",
            json!({"request":request}),
            None
        )
        .await
        .is_err());
    }
    for name in [
        "geod_stac_connect",
        "geod_stac_project_save",
        "geod_stac_download",
        "geod_stac_forget",
    ] {
        assert!(
            crate::agent_actions::call(manager.clone(), &session, name, json!({}), None)
                .await
                .is_err()
        );
        assert!(!crate::agent_actions::definitions()
            .iter()
            .any(|d| d["name"] == name));
    }
    assert!(manager.list().await.is_empty());
    assert!(manager.list_projects().await.is_empty());
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn native_snapshot_paging_project_append_and_agent_reads_keep_exact_pins() {
    let directory = tempfile::tempdir().unwrap();
    let manager = crate::JobManager::open(directory.path()).await.unwrap();
    let pin = crate::stac::fixture_snapshot(directory.path(), "https://example.com/original.tif");
    let adapter = Adapter::new(Backend::Direct(manager.clone()), true);
    let snapshot = adapter
        .call("geod_stac_snapshot", json!({"id":pin.snapshot_id}))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(
        snapshot["assets"][0]["metadata"]["raster:bands"][0]["scale"],
        0.1
    );
    let assets = adapter
        .call("geod_stac_assets", json!({"id":pin.snapshot_id,"limit":1}))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(assets["assets"], snapshot["assets"]);
    assert_eq!(assets["documentSha256"], snapshot["documentSha256"]);
    let request = json!({"name":"STAC MCP project","bounds":[13,52,15,54],"selections":[pin]});
    let saved = adapter
        .call("geod_stac_project_save", json!({"request":request}))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    let again=adapter.call("geod_stac_project_save",json!({"request":{"projectId":saved["id"],"bounds":[13.1,52.1,14.9,53.9],"selections":[pin]}})).await.unwrap().structured_content.unwrap();
    assert_eq!(saved, again);
    let before = std::fs::read(directory.path().join("projects.json")).unwrap();
    let readonly = Adapter::new(Backend::Direct(manager.clone()), false);
    assert!(readonly
        .call("geod_stac_project_save", json!({"request":request}))
        .await
        .is_err());
    let agent = agent_read_call(
        manager.clone(),
        "geod_stac_snapshot",
        json!({"id":pin.snapshot_id}),
    )
    .await
    .unwrap();
    assert!(agent.get("assets").is_none());
    assert_eq!(agent["assetsOmitted"], true);
    assert_eq!(agent["assetCount"], 1);
    assert_eq!(agent["eligibleAssetCount"], 1);
    let declarations = agent_read_call(
        manager.clone(),
        "geod_stac_assets",
        agent["assetsTool"]["arguments"].clone(),
    )
    .await
    .unwrap();
    assert_eq!(declarations["assets"][0]["key"], pin.asset_key);
    assert!(declarations["assets"][0].get("href").is_none());
    assert_eq!(agent["documentSha256"], snapshot["documentSha256"]);
    assert!(agent_read_call(
        manager.clone(),
        "geod_stac_download",
        json!({"request":{"projectId":saved["id"]}})
    )
    .await
    .is_err());
    assert!(agent_read_call(
        manager.clone(),
        "geod_stac_assets",
        json!({"id":pin.snapshot_id,"limit":21})
    )
    .await
    .is_err());
    assert_eq!(
        before,
        std::fs::read(directory.path().join("projects.json")).unwrap()
    );
    assert!(manager.list().await.is_empty());
    adapter.shutdown().await.unwrap();
    readonly.shutdown().await.unwrap();
    drop(adapter);
    drop(readonly);
    drop(manager);
    let reopened = crate::JobManager::open(directory.path()).await.unwrap();
    let restored = agent_read_call(
        reopened.clone(),
        "geod_stac_snapshot",
        json!({"id":pin.snapshot_id}),
    )
    .await
    .unwrap();
    assert_eq!(restored, agent);
    reopened.shutdown().await.unwrap();
}

#[test]
fn agent_snapshot_avoids_duplicate_large_assets_without_losing_pinned_metadata() {
    // Synthetic size/control regression, not a provider-download acceptance.
    let assets: Vec<_> = (0..38)
        .map(|index| json!({"key":format!("original-{index}"),"eligible":index%2==0,"metadata":{"description":"x".repeat(2000),"raster:bands":[{"data_type":"uint16","scale":0.0001}]}}))
        .collect();
    let original = json!({"id":"a".repeat(64),"properties":{"proj:epsg":32610,"eo:cloud_cover":2.7},"geometry":{"type":"Point","coordinates":[1,2]},"assets":assets,"provenance":{"collection":{"id":"actual-collection","license":"proprietary","item_assets":{"raw":{"unit":"m"}}}},"documentSha256":"b".repeat(64)});
    assert!(original.to_string().len() > 32768);
    let mut projected = original.clone();
    agent_snapshot(&mut projected).unwrap();
    assert!(projected.to_string().len() < 32768);
    assert_eq!(projected["properties"], original["properties"]);
    assert_eq!(projected["geometry"], original["geometry"]);
    assert_eq!(projected["provenance"], original["provenance"]);
    assert_eq!(projected["documentSha256"], original["documentSha256"]);
    assert_eq!(projected["assetCount"], 38);
    assert_eq!(projected["eligibleAssetCount"], 19);
    assert_eq!(projected["assetsTool"]["arguments"]["id"], original["id"]);
    assert_eq!(original["assets"].as_array().unwrap().len(), 38);
}

#[tokio::test]
async fn catalog_pages_distinguish_real_collections_static_keys_and_standalone_snapshots() {
    let static_id = "22222222-2222-4222-8222-222222222222";
    let item_id = "33333333-3333-4333-8333-333333333333";
    let app=axum::Router::new().route("/stac/connections",axum::routing::get(move ||async move {
        axum::Json(json!([
            {"id":ID,"kind":"api","collections":[{"id":"real-one","license":"proprietary"},{"id":"real-two","license":"CC-BY-4.0"}],"snapshotIds":[],"capabilities":{"searchGet":false,"searchPost":true}},
            {"id":static_id,"kind":"catalog","collections":[],"catalogNodes":[{"key":"a".repeat(64),"id":"actual-directory","kind":"Catalog","parentKey":null},{"key":"b".repeat(64),"id":"actual-leaf","kind":"Collection","parentKey":"a".repeat(64)}],"snapshotIds":[],"capabilities":{"searchGet":false,"searchPost":false}},
            {"id":item_id,"kind":"item","collections":[],"snapshotIds":["c".repeat(64)]}
        ]))
    }));
    let (backend, server) = fake_backend(app).await;
    let adapter = Adapter::new(backend, false);
    let page = adapter
        .call("geod_stac_catalog", json!({"id":ID,"limit":1}))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(page["entryKind"], "collection");
    assert_eq!(page["entries"][0]["license"], "proprietary");
    assert_eq!(page["nextOffset"], 1);
    let page = adapter
        .call(
            "geod_stac_catalog",
            json!({"id":static_id,"offset":1,"limit":1}),
        )
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(page["entryKind"], "directory");
    assert_eq!(page["entries"][0]["id"], "actual-leaf");
    assert_eq!(page["entries"][0]["key"], "b".repeat(64));
    assert_eq!(page["connection"]["collectionCount"], 0);
    let page = adapter
        .call("geod_stac_catalog", json!({"id":item_id}))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(page["entryKind"], "snapshot");
    assert_eq!(page["entries"][0]["snapshotId"], "c".repeat(64));
    adapter.shutdown().await.unwrap();
    server.abort();
}

#[tokio::test]
async fn search_preserves_empty_page_progress_and_every_large_item_identity() {
    let seen = Arc::new(Mutex::new(Vec::<Value>::new()));
    let capture = seen.clone();
    let app=axum::Router::new().route("/stac/search",axum::routing::post(move |axum::Json(r):axum::Json<Value>| {
        let capture=capture.clone();
        async move {
            capture.lock().await.push(r.clone());
            if r["cursor"].is_null() {
                axum::Json(json!({"items":[],"nextCursor":ID,"complete":false,"limitReached":false,"scannedItems":32}))
            } else {
                axum::Json(json!({"items":[{"id":"a".repeat(64),"connectionId":ID,"itemId":"actual-item","collectionId":"actual-collection","properties":{"description":"x".repeat(600000)},"geometry":null,"assets":[{"key":"original","eligible":true}],"documentSha256":"b".repeat(64)}],"nextCursor":null,"complete":false,"limitReached":true,"scannedItems":1000}))
            }
        }
    }));
    let (backend, server) = fake_backend(app).await;
    let adapter = Adapter::new(backend, true);
    let request = json!({"connectionId":ID,"collectionId":"directory-key","bounds":[1,2,3,4]});
    let first = adapter
        .call("geod_stac_search", json!({"request":request}))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(first["items"], json!([]));
    assert_eq!(first["scannedItems"], 32);
    assert_eq!(first["nextCursor"], ID);
    let mut next = request.clone();
    next["cursor"] = json!(ID);
    let second = adapter
        .call("geod_stac_search", json!({"request":next}))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(second["items"][0]["itemId"], "actual-item");
    assert_eq!(second["items"][0]["collectionId"], "actual-collection");
    assert_eq!(second["items"][0]["eligibleAssetCount"], 1);
    assert_eq!(second["items"][0]["detailsOmitted"], true);
    assert_eq!(second["complete"], false);
    assert_eq!(second["limitReached"], true);
    assert_eq!(second["scannedItems"], 1000);
    assert!(second.to_string().len() < 3000);
    assert_eq!(seen.lock().await[0]["limit"], 20);
    adapter.shutdown().await.unwrap();
    server.abort();
}

#[tokio::test]
async fn server_transfer_returns_recoverable_polling_without_claiming_completion() {
    let app=axum::Router::new()
        .route("/stac/downloads",axum::routing::post(||async { axum::Json(json!({"projectId":ID,"assetKey":"stac_asset","jobs":[{"id":ID,"status":"queued"}]})) }))
        .route("/jobs/{id}",axum::routing::get(||async { axum::Json(json!({"id":ID,"status":"queued","settled":false})) }))
        .route("/stac/jobs/{id}/inspect",axum::routing::get(||async { axum::Json(json!({"sha256":"a".repeat(64),"previewDataUrl":"data:image/png;base64,example","bandCount":1})) }))
        .route("/stac/jobs/{id}/pixel",axum::routing::get(|axum::extract::Query(r):axum::extract::Query<std::collections::HashMap<String,String>>|async move { axum::Json(json!({"column":r["column"],"row":r["row"],"samples":["18446744073709551615","nan"],"nodata":null})) }));
    let (backend, server) = fake_backend(app).await;
    let adapter = Adapter::new(backend, true);
    let submitted = adapter
        .call("geod_stac_download", json!({"request":{"projectId":ID}}))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(submitted["jobs"][0]["jobId"], ID);
    assert_eq!(submitted["jobs"][0]["job"]["settled"], false);
    assert_eq!(submitted["jobs"][0]["poll"]["tool"], "geod_job_status");
    let inspected = adapter
        .call("geod_stac_inspect", json!({"id":ID}))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert!(inspected.get("previewDataUrl").is_none());
    assert_eq!(inspected["previewOmitted"], true);
    let pixel = adapter
        .call("geod_stac_pixel", json!({"id":ID,"column":1,"row":2}))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(pixel["samples"], json!(["18446744073709551615", "nan"]));
    adapter.shutdown().await.unwrap();
    server.abort();
}

#[tokio::test]
#[ignore = "explicit real public metadata acceptance; fresh QA directory required"]
async fn live_agent_custom_stac_reads_and_searches_native_metadata_only() {
    let root = std::path::PathBuf::from(
        std::env::var("GEOD_AGENT_STAC_QA").expect("Set a new directory under .verification"),
    );
    assert!(!root.exists() && root.components().any(|c| c.as_os_str() == ".verification"));
    let manager = crate::JobManager::open(&root).await.unwrap();
    let connection = manager
        .connect_stac(crate::stac::ConnectRequest {
            name: "Agent actual custom STAC".into(),
            url: "https://earth-search.aws.element84.com/v1/".into(),
            kind: "api".into(),
        })
        .await
        .unwrap();
    let session = uuid::Uuid::new_v4().to_string();
    let inventory = crate::agent_actions::call(
        manager.clone(),
        &session,
        "geod_sources_list",
        json!({}),
        None,
    )
    .await
    .unwrap();
    let source = inventory["customSources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == connection.id)
        .unwrap();
    assert_eq!(source["agentSearch"], true);
    assert_eq!(source["agentDownload"], true);
    assert_eq!(source["approvalRequired"], true);
    let catalog = agent_read_call(
        manager.clone(),
        "geod_stac_catalog",
        json!({"id":connection.id,"limit":20}),
    )
    .await
    .unwrap();
    assert!(catalog["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["id"] == "sentinel-2-l2a"));
    let mut request = json!({"connectionId":connection.id,"collectionId":"sentinel-2-l2a","bounds":[13.4,52.5,13.41,52.51],"datetime":"2024-01-01T00:00:00Z/2024-01-10T23:59:59Z","limit":4});
    let mut pages = Vec::new();
    let mut snapshots = Vec::new();
    loop {
        let page = crate::agent_actions::call(
            manager.clone(),
            &session,
            "geod_stac_search",
            json!({"request":request}),
            None,
        )
        .await
        .unwrap();
        for summary in page["items"].as_array().unwrap() {
            let id = summary["id"].as_str().unwrap();
            let native = manager.stac_snapshot(id).await.unwrap();
            assert_eq!(summary["itemId"], native.item_id);
            assert_eq!(summary["documentSha256"], native.document_sha256);
            let mut all_assets = Vec::new();
            let mut offset = 0;
            loop {
                let assets = agent_read_call(
                    manager.clone(),
                    "geod_stac_assets",
                    json!({"id":id,"offset":offset,"limit":20}),
                )
                .await
                .unwrap();
                all_assets.extend(assets["assets"].as_array().unwrap().iter().cloned());
                let Some(next) = assets["nextOffset"].as_u64() else {
                    break;
                };
                assert!(next > offset && next < 512);
                offset = next;
            }
            assert_eq!(all_assets.len(), native.assets.len());
            assert!(all_assets
                .iter()
                .any(|v| v["key"] == "scl" && v["eligible"] == true));
            assert!(all_assets.iter().all(|v| v.get("href").is_none()));
            snapshots.push(
                json!({"id":id,"itemId":native.item_id,"documentSha256":native.document_sha256}),
            );
        }
        let complete = page["complete"] == true;
        request["cursor"] = page["nextCursor"].clone();
        pages.push(page);
        if complete {
            break;
        }
        assert!(pages.len() < 5 && request["cursor"].is_string());
    }
    assert_eq!(snapshots.len(), 8);
    assert!(manager.list().await.is_empty() && manager.list_projects().await.is_empty());
    let report = json!({"schema":"geod-agent-custom-stac-metadata/v1","status":"passed","connectionId":connection.id,
        "pages":pages,"snapshots":snapshots,"source":source,"projectsCreated":0,"originalDownloads":0,"modelCalled":false,"usedUserDesktop":false});
    std::fs::write(
        root.join("acceptance.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    manager.shutdown().await.unwrap();
}
