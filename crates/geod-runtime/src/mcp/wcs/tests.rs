use super::*;
use crate::mcp::{parse_operation, tests::fake_backend, Adapter, READ_TOOLS, WRITE_TOOLS};
use std::collections::HashSet;

const ID: &str = "11111111-1111-4111-8111-111111111111";

#[test]
fn coverage_contracts_reject_unknown_fields_path_injection_and_non_grid_pixels() {
    let hash = "a".repeat(64);
    for (name, args) in [
        ("geod_wcs_plan", json!({"id":"../other.json"})),
        ("geod_wcs_plan", json!({"id":hash.to_uppercase()})),
        ("geod_wcs_inspect", json!({"id":hash})),
        ("geod_wcs_pixel", json!({"id":ID,"column":1.5,"row":0})),
        ("geod_wcs_pixel", json!({"id":ID,"column":0,"row":-1})),
        (
            "geod_wcs_pixel",
            json!({"id":ID,"column":0,"row":4294967296u64}),
        ),
        (
            "geod_wcs_pixel",
            json!({"id":ID,"column":0,"row":0,"band":1}),
        ),
        ("geod_wcs_coverages", json!({"id":ID,"limit":101})),
        (
            "geod_wcs_connect",
            json!({"request":{"name":"user source","url":"https://example.com/wcs","password":"secret"}}),
        ),
        (
            "geod_wcs_prepare",
            json!({"request":{"descriptionId":hash,"bounds":[53.,2.,53.05,2.05],"url":"https://other.example/tif"}}),
        ),
        (
            "geod_wcs_prepare",
            json!({"request":{"descriptionId":hash,"bounds":[-181.,-10.,1.,10.]}}),
        ),
        (
            "geod_wcs_project_save",
            json!({"request":{"bounds":[1.,2.,3.,4.],"selections":[{"planId":hash,"href":"https://other.example/tif"}]}}),
        ),
        (
            "geod_wcs_project_save",
            json!({"request":{"bounds":[1.,2.,3.,4.],"selections":[]}}),
        ),
        (
            "geod_wcs_download",
            json!({"request":{"projectId":ID,"url":"https://other.example/tif"}}),
        ),
        (
            "geod_wcs_download",
            json!({"request":{"projectId":ID,"selections":[]}}),
        ),
    ] {
        assert!(parse_operation(name, args, true).is_err(), "{name}");
    }
    assert!(parse_operation("geod_wcs_plan", json!({"id":hash}), false).is_ok());
    assert!(parse_operation(
        "geod_wcs_prepare",
        json!({"request":{"descriptionId":hash,"bounds":[2.,53.,2.05,53.05]}}),
        false
    )
    .is_err());
    assert!(parse_operation(
        "geod_wcs_download",
        json!({"request":{"projectId":ID}}),
        true
    )
    .is_ok());
}

#[test]
fn tool_names_and_annotations_match_discovery_and_native_side_effects() {
    let all = crate::mcp::tools(true);
    let names: HashSet<_> = all.iter().map(|t| t.name.as_ref()).collect();
    assert_eq!(names.len(), READ_TOOLS.len() + WRITE_TOOLS.len());
    for tool in all {
        let name = tool.name.as_ref();
        let a = tool.annotations.unwrap();
        assert_eq!(a.read_only_hint, Some(READ_TOOLS.contains(&name)));
        if matches!(
            name,
            "geod_wcs_connect" | "geod_wcs_describe" | "geod_wcs_download"
        ) {
            assert_eq!(a.open_world_hint, Some(true));
            assert_eq!(a.read_only_hint, Some(false));
        }
        if matches!(
            name,
            "geod_wcs_prepare" | "geod_wcs_project_save" | "geod_wcs_forget"
        ) {
            assert_eq!(a.open_world_hint, Some(false));
            assert_eq!(a.read_only_hint, Some(false));
        }
        if name == "geod_wcs_forget" {
            assert_eq!(a.destructive_hint, Some(true));
        }
    }
}

#[tokio::test]
async fn saved_native_plan_can_be_appended_idempotently_without_network_or_urls() {
    let directory = tempfile::tempdir().unwrap();
    let manager = crate::JobManager::open(directory.path()).await.unwrap();
    let pin = wcs::fixture_plan(directory.path());
    let adapter = Adapter::new(Backend::Direct(manager.clone()), true);
    let result = adapter
        .call("geod_wcs_plan", json!({"id":pin.plan_id}))
        .await
        .unwrap();
    assert_eq!(result.is_error, Some(false));
    let plan = result.structured_content.unwrap();
    assert_eq!(plan["width"], 2);
    assert_eq!(
        plan["description"]["fields"][0]["unit"],
        "source-declared-unit"
    );
    let request =
        json!({"name":"Agent coverage project","bounds":[13.,52.,15.,54.],"selections":[pin]});
    let saved = adapter
        .call("geod_wcs_project_save", json!({"request":request}))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    let id = saved["id"].as_str().unwrap();
    let again = adapter
        .call(
            "geod_wcs_project_save",
            json!({"request":{"projectId":id,"bounds":[13.1,52.1,14.9,53.9],"selections":[pin]}}),
        )
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(saved, again);
    let before = std::fs::read(directory.path().join("projects.json")).unwrap();
    let readonly = Adapter::new(Backend::Direct(manager.clone()), false);
    let listed = readonly
        .call("geod_projects_list", json!({"limit":1}))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(listed["projects"][0]["wcsItemCount"], 1);
    assert!(listed["projects"][0].get("wcsItems").is_none());
    let found = readonly
        .call("geod_project_get", json!({"id":id}))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(found, saved);
    let description = readonly
        .call(
            "geod_wcs_description",
            json!({"id":plan["description"]["id"]}),
        )
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(description, plan["description"]);
    let denied = readonly
        .call("geod_wcs_project_save", json!({"request":request}))
        .await;
    assert!(denied.is_err());
    assert_eq!(
        before,
        std::fs::read(directory.path().join("projects.json")).unwrap()
    );
    assert!(manager.list().await.is_empty());
    readonly.shutdown().await.unwrap();
    adapter.shutdown().await.unwrap();
}

#[tokio::test]
async fn service_catalog_paging_retains_metadata_and_does_not_hide_next_page() {
    let app = axum::Router::new().route("/wcs/connections",axum::routing::get(||async {
        axum::Json(json!([{"id":ID,"name":"source title is untrusted","url":"https://example.com/wcs","version":"2.0.1",
            "fees":"NONE","accessConstraints":"Unknown license","coverages":[{"id":"one"},{"id":"two"},{"id":"three"}]}]))
    }));
    let (backend, server) = fake_backend(app).await;
    let adapter = Adapter::new(backend, false);
    let connections = adapter
        .call("geod_wcs_connections", json!({"limit":1}))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(connections["connections"][0]["coverageCount"], 3);
    assert!(connections["connections"][0].get("coverages").is_none());
    let page = adapter
        .call("geod_wcs_coverages", json!({"id":ID,"offset":1,"limit":1}))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(page["coverages"], json!([{"id":"two"}]));
    assert_eq!(page["nextOffset"], 2);
    assert_eq!(page["connection"]["accessConstraints"], "Unknown license");
    assert!(page["connection"].get("coverages").is_none());
    let wrong = adapter
        .call(
            "geod_wcs_coverages",
            json!({"id":"22222222-2222-4222-8222-222222222222"}),
        )
        .await
        .unwrap();
    assert_eq!(wrong.is_error, Some(true));
    adapter.shutdown().await.unwrap();
    server.abort();
}

#[tokio::test]
async fn coverage_inspection_omits_images_pixels_remain_raw_and_batch_requires_settlement() {
    let app = axum::Router::new()
        .route("/wcs/jobs/{id}/inspect",axum::routing::get(||async {axum::Json(json!({"width":2,"height":2,"previewDataUrl":"data:image/png;base64,not-tool-content","bands":[{"dataType":"Int64"}]}))}))
        .route("/wcs/jobs/{id}/pixel",axum::routing::get(|axum::extract::Query(p):axum::extract::Query<std::collections::HashMap<String,String>>|async move {
            assert_eq!(p.get("column").unwrap(),"1");assert_eq!(p.get("row").unwrap(),"0");
            axum::Json(json!({"column":1,"row":0,"values":["9223372036854775807","NaN"],"noData":[false,true]}))
        }))
        .route("/wcs/downloads",axum::routing::post(|axum::Json(v):axum::Json<Value>|async move {
            assert_eq!(v["projectId"],ID);assert!(v.get("href").is_none());
            axum::Json(json!({"projectId":ID,"assetKey":"wcs_coverage","jobs":[{"id":ID,"status":"queued"}]}))
        }))
        .route("/jobs/{id}",axum::routing::get(||async {axum::Json(json!({"id":ID,"status":"running","settled":false}))}));
    let (backend, server) = fake_backend(app).await;
    let adapter = Adapter::new(backend, true);
    let inspect = adapter
        .call("geod_wcs_inspect", json!({"id":ID}))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(inspect["previewOmitted"], true);
    assert!(inspect.get("previewDataUrl").is_none());
    let pixel = adapter
        .call("geod_wcs_pixel", json!({"id":ID,"column":1,"row":0}))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(pixel["values"][0], "9223372036854775807");
    assert_eq!(pixel["values"][1], "NaN");
    let batch = adapter
        .call("geod_wcs_download", json!({"request":{"projectId":ID}}))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(batch["jobs"][0]["jobId"], ID);
    assert_eq!(batch["jobs"][0]["job"]["settled"], false);
    assert_eq!(batch["jobs"][0]["poll"]["tool"], "geod_job_status");
    assert!(batch.get("succeeded").is_none());
    adapter.shutdown().await.unwrap();
    server.abort();
}

#[tokio::test]
async fn mutations_use_native_routes_and_trim_catalog_without_changing_declarations() {
    let digest = "a".repeat(64);
    let app = axum::Router::new()
        .route("/wcs/connections",axum::routing::post(|axum::Json(v):axum::Json<Value>|async move {
            assert_eq!(v,json!({"name":"selected source","url":"https://example.com/wcs"}));
            axum::Json(json!({"id":ID,"fees":"NONE","accessConstraints":"Unconfirmed","coverages":[{"id":"source:coverage"}]}))
        }))
        .route("/wcs/describe",axum::routing::post(|axum::Json(v):axum::Json<Value>|async move {
            assert_eq!(v,json!({"connectionId":ID,"coverageId":"source:coverage"}));
            axum::Json(json!({"id":"a".repeat(64),"fields":[{"name":"Depth","unit":"source-unit","nilValues":[{"value":"NaN"}]}]}))
        }))
        .route("/wcs/plan",axum::routing::post(|axum::Json(v):axum::Json<Value>|async move {
            assert_eq!(v,json!({"descriptionId":"a".repeat(64),"bounds":[2.,53.,2.05,53.05]}));
            axum::Json(json!({"id":"b".repeat(64),"width":48,"height":48}))
        }))
        .route("/wcs/project",axum::routing::post(|axum::Json(v):axum::Json<Value>|async move {
            assert_eq!(v["name"],"Agent project");assert_eq!(v["selections"],json!([{"planId":"b".repeat(64)}]));
            axum::Json(json!({"id":ID,"name":"Agent project"}))
        }))
        .route("/wcs/connections/{id}/forget",axum::routing::post(|axum::extract::Path(id):axum::extract::Path<String>|async move {
            assert_eq!(id,ID);axum::Json(json!({"removed":true}))
        }));
    let (backend, server) = fake_backend(app).await;
    let adapter = Adapter::new(backend, true);
    let connected = adapter
        .call(
            "geod_wcs_connect",
            json!({"request":{"name":"selected source","url":"https://example.com/wcs"}}),
        )
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(connected["coverageCount"], 1);
    assert!(connected.get("coverages").is_none());
    assert_eq!(connected["accessConstraints"], "Unconfirmed");
    assert_eq!(connected["coveragesTool"]["arguments"]["id"], ID);
    let described = adapter
        .call(
            "geod_wcs_describe",
            json!({"request":{"connectionId":ID,"coverageId":"source:coverage"}}),
        )
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(described["fields"][0]["unit"], "source-unit");
    let prepared = adapter
        .call(
            "geod_wcs_prepare",
            json!({"request":{"descriptionId":digest,"bounds":[2.,53.,2.05,53.05]}}),
        )
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(prepared["width"], 48);
    let saved=adapter.call("geod_wcs_project_save",json!({"request":{"name":"Agent project","bounds":[2.,53.,2.05,53.05],"selections":[{"planId":prepared["id"]}]}})).await.unwrap().structured_content.unwrap();
    assert_eq!(saved["id"], ID);
    assert_eq!(
        adapter
            .call("geod_wcs_forget", json!({"id":ID}))
            .await
            .unwrap()
            .structured_content
            .unwrap()["removed"],
        true
    );
    adapter.shutdown().await.unwrap();
    server.abort();
}
