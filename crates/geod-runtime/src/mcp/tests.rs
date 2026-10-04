use super::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

fn args(values: &[&str]) -> std::vec::IntoIter<String> {
    values
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .into_iter()
}

#[test]
fn startup_requires_one_owner_and_never_expands_remote_access() {
    assert!(
        !Options::parse(args(&["--server", "http://127.0.0.1:4318"]))
            .unwrap()
            .allow_write
    );
    assert!(
        Options::parse(args(&["--data-dir", "state", "--allow-write"]))
            .unwrap()
            .allow_write
    );
    for options in [
        vec![],
        vec!["--data-dir", "state", "--server", "http://127.0.0.1:4318"],
        vec!["--server"],
        vec!["--data-dir", "x", "--allow-write", "--allow-write"],
        vec!["--data-dir", "x", "--data-dir", "y"],
        vec!["--data-dir", "x", "--shell", "cmd.exe"],
    ] {
        assert!(Options::parse(args(&options)).is_err());
    }
    for origin in [
        "https://127.0.0.1:4318",
        "http://localhost:4318",
        "http://127.0.0.1.evil:4318",
        "http://user@127.0.0.1:4318",
        "http://127.0.0.1:4318/private",
        "http://127.0.0.1:4318?token=x",
        "http://127.0.0.1:0",
    ] {
        assert!(loopback_origin(origin).is_err(), "{origin}");
    }
}

#[test]
fn read_only_discovery_and_dispatch_both_deny_writes() {
    assert_eq!(tools(false).len(), READ_TOOLS.len());
    assert_eq!(tools(true).len(), READ_TOOLS.len() + WRITE_TOOLS.len());
    for tool in tools(false) {
        assert_eq!(tool.annotations.unwrap().read_only_hint, Some(true));
    }
    for &tool in WRITE_TOOLS {
        assert!(parse_operation(tool, json!({}), false).is_err());
        assert!(!tools(false).iter().any(|entry| entry.name == tool));
    }
    for (name, arguments) in [
        ("geod_health", json!({"path":"../../secret"})),
        ("geod_job_status", json!({"id":"../health"})),
        (
            "geod_raster_pixel",
            json!({"id":"../health", "x":500000, "y":4000000}),
        ),
        (
            "geod_raster_pixel",
            json!({"id":"11111111-1111-4111-8111-111111111111", "x":"NaN", "y":4000000}),
        ),
        (
            "geod_job_status",
            json!({"id":"FFFFFFFF-FFFF-4FFF-8FFF-FFFFFFFFFFFF"}),
        ),
        ("geod_jobs_list", json!({"limit":101})),
        ("geod_jobs_list", json!({"limit":0})),
        ("geod_jobs_list", json!({"offset":-1})),
        (
            "geod_health",
            json!({"padding":"x".repeat(MAX_ARGUMENT_BYTES)}),
        ),
    ] {
        assert!(parse_operation(name, arguments, true).is_err());
    }
}

#[test]
fn write_inputs_share_runtime_allowlist_and_pinned_recipe_contract() {
    let mut request: Value = serde_json::from_str(include_str!(
        "../../../../examples/sentinel-scl-download.json"
    ))
    .unwrap();
    assert!(parse_operation("geod_download", json!({"request":request}), true).is_ok());
    request["href"] = json!("http://127.0.0.1:9999/private");
    assert!(parse_operation("geod_download", json!({"request":request}), true).is_err());
    let mut recipe: Value = serde_json::from_str(include_str!(
        "../../../../examples/sentinel-scl-clip.recipe.json"
    ))
    .unwrap();
    assert!(parse_operation("geod_recipe_plan", json!({"recipe":recipe}), false).is_ok());
    recipe["output"]["path"] = json!("../../overwritten.tif");
    assert!(parse_operation("geod_recipe_run", json!({"recipe":recipe}), true).is_err());
}

#[tokio::test]
async fn bounded_stdio_stops_oversized_frames_before_json_parsing() {
    let input = [
        b"{}\n".as_slice(),
        vec![b'x'; MAX_FRAME_BYTES + 1].as_slice(),
    ]
    .concat();
    let mut reader = BufReader::new(bounded_reader(io::Cursor::new(input)));
    let mut first = String::new();
    reader.read_line(&mut first).await.unwrap();
    assert_eq!(first, "{}\n");
    assert_eq!(
        reader
            .read_line(&mut String::new())
            .await
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidData
    );
}

#[tokio::test]
async fn operational_failure_is_tool_error_not_protocol_success() {
    let directory = tempfile::tempdir().unwrap();
    let adapter = Adapter::new(
        Backend::Direct(JobManager::open(directory.path()).await.unwrap()),
        false,
    );
    let error = adapter
        .call(
            "geod_job_status",
            json!({"id":"11111111-1111-4111-8111-111111111111"}),
        )
        .await
        .unwrap();
    assert_eq!(error.is_error, Some(true));
    assert!(error.structured_content.unwrap()["error"]
        .as_str()
        .unwrap()
        .contains("Unknown job"));
    assert!(adapter
        .call("geod_job_status", json!({"id":"not-a-job"}))
        .await
        .is_err());
    adapter.shutdown().await.unwrap();
}

#[tokio::test]
async fn exclusive_shutdown_cancels_active_download_and_waits_for_cleanup() {
    let begun = Arc::new(Notify::new());
    let endpoint = begun.clone();
    let app = axum::Router::new().route(
        "/slow.tif",
        axum::routing::get(move || {
            let begun = endpoint.clone();
            async move {
                begun.notify_one();
                tokio::time::sleep(Duration::from_secs(5)).await;
                (
                    [("content-type", "image/tiff")],
                    vec![b'I', b'I', 42, 0, 0, 0, 0, 0],
                )
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let directory = tempfile::tempdir().unwrap();
    let manager = JobManager::open_inner(directory.path(), Some(origin.clone()))
        .await
        .unwrap();
    let job = manager
        .create(CreateJobRequest {
            item_id: "TEST".into(),
            asset_key: "scl".into(),
            href: format!("{origin}/slow.tif"),
            media_type: "image/tiff".into(),
            title: None,
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(8), begun.notified())
        .await
        .expect("mock download server was not reached");
    Backend::Direct(manager.clone()).shutdown().await.unwrap();
    let (job, settled) = manager.get_with_settled(&job.id).await.unwrap();
    assert_eq!(job.status, JobStatus::Cancelled);
    assert!(settled);
    assert!(job.output_path.is_none());
    assert_eq!(
        std::fs::read_dir(directory.path().join("assets"))
            .unwrap()
            .count(),
        0
    );
    server.abort();
}

pub(super) async fn fake_backend(app: axum::Router) -> (Backend, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let backend = Backend::Server {
        base: format!("http://{}", listener.local_addr().unwrap()),
        client: reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap(),
    };
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (backend, handle)
}

#[tokio::test]
async fn remote_inspection_omits_png_and_old_status_cannot_claim_settlement() {
    let app = axum::Router::new()
        .route("/jobs/{id}/raster", axum::routing::get(|| async { axum::Json(json!({"width":2,"height":2,"previewDataUrl":"data:image/png;base64,do-not-send","classes":[{"value":6,"count":4}]})) }))
        .route("/jobs/{id}", axum::routing::get(|| async { axum::Json(json!({"id":"11111111-1111-4111-8111-111111111111","status":"succeeded"})) }));
    let (backend, server) = fake_backend(app).await;
    let result = backend
        .execute(Operation::Inspect(
            "11111111-1111-4111-8111-111111111111".into(),
        ))
        .await
        .unwrap();
    assert!(result.get("previewDataUrl").is_none());
    assert_eq!(result["previewOmitted"], true);
    assert_eq!(result["classes"][0]["count"], 4);
    assert!(backend
        .status("11111111-1111-4111-8111-111111111111")
        .await
        .unwrap_err()
        .contains("settlement"));
    server.abort();
}

#[tokio::test]
async fn cancelled_call_response_does_not_abort_in_flight_write_and_shutdown_drains_it() {
    let begun = Arc::new(Notify::new());
    let finished = Arc::new(AtomicBool::new(false));
    let endpoint_begun = begun.clone();
    let endpoint_finished = finished.clone();
    let app = axum::Router::new().route(
        "/recipes",
        axum::routing::post(move || {
            let begun = endpoint_begun.clone();
            let finished = endpoint_finished.clone();
            async move {
                begun.notify_one();
                tokio::time::sleep(Duration::from_millis(150)).await;
                finished.store(true, Ordering::SeqCst);
                axum::Json(json!({"id":"saved"}))
            }
        }),
    );
    let (backend, server) = fake_backend(app).await;
    let adapter = Adapter::new(backend, true);
    let caller = adapter.clone();
    let recipe: Value = serde_json::from_str(include_str!(
        "../../../../examples/sentinel-scl-clip.recipe.json"
    ))
    .unwrap();
    let request = tokio::spawn(async move {
        caller
            .call("geod_recipe_save", json!({"recipe":recipe}))
            .await
    });
    begun.notified().await;
    request.abort();
    adapter.shutdown().await.unwrap();
    assert!(finished.load(Ordering::SeqCst));
    assert_eq!(adapter.calls.count.load(Ordering::SeqCst), 0);
    server.abort();
}

async fn rpc_send(writer: &mut (impl AsyncWriteExt + Unpin), value: Value) {
    writer
        .write_all(format!("{value}\n").as_bytes())
        .await
        .unwrap();
    writer.flush().await.unwrap();
}
async fn rpc_read(reader: &mut (impl AsyncBufReadExt + Unpin)) -> Value {
    let mut line = String::new();
    tokio::time::timeout(Duration::from_secs(5), reader.read_line(&mut line))
        .await
        .unwrap()
        .unwrap();
    serde_json::from_str(&line).unwrap()
}

#[tokio::test]
async fn sdk_protocol_initializes_lists_calls_and_returns_meaningful_errors() {
    let directory = tempfile::tempdir().unwrap();
    let adapter = Adapter::new(
        Backend::Direct(JobManager::open(directory.path()).await.unwrap()),
        false,
    );
    let (client, server) = tokio::io::duplex(128 * 1024);
    let task = tokio::spawn(async move {
        let (read, write) = tokio::io::split(server);
        adapter
            .clone()
            .serve((read, write))
            .await
            .unwrap()
            .waiting()
            .await
            .unwrap();
        adapter.shutdown().await.unwrap();
    });
    let (read, mut write) = tokio::io::split(client);
    let mut read = BufReader::new(read);
    rpc_send(&mut write, json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"geod-test","version":"1"}}})).await;
    let initialized = rpc_read(&mut read).await;
    assert_eq!(initialized["result"]["serverInfo"]["name"], "geod-global");
    assert!(initialized["result"]["capabilities"]["tools"].is_object());
    rpc_send(
        &mut write,
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    )
    .await;
    rpc_send(
        &mut write,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
    )
    .await;
    let listed = rpc_read(&mut read).await;
    assert_eq!(
        listed["result"]["tools"].as_array().unwrap().len(),
        READ_TOOLS.len()
    );
    rpc_send(&mut write, json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"geod_health","arguments":{}}})).await;
    let health = rpc_read(&mut read).await;
    assert_eq!(health["result"]["isError"], false);
    assert_eq!(
        health["result"]["structuredContent"]["runtime"]["status"],
        "ok"
    );
    rpc_send(&mut write, json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"geod_recipe_run","arguments":{}}})).await;
    assert_eq!(rpc_read(&mut read).await["error"]["code"], -32602);
    rpc_send(
        &mut write,
        json!({"jsonrpc":"2.0","id":5,"method":"not/a-method"}),
    )
    .await;
    assert_eq!(rpc_read(&mut read).await["error"]["code"], -32601);
    write.shutdown().await.unwrap();
    drop(write);
    drop(read);
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap();
}

#[test]
fn list_pagination_does_not_hide_next_page() {
    let value = paginate(
        json!([1, 2, 3]),
        PageArgs {
            offset: Some(1),
            limit: Some(1),
        },
        "jobs",
    )
    .unwrap();
    assert_eq!(
        value,
        json!({"jobs":[2],"total":3,"offset":1,"nextOffset":2})
    );
    let end = paginate(
        json!([1, 2, 3]),
        PageArgs {
            offset: Some(8),
            limit: Some(2),
        },
        "jobs",
    )
    .unwrap();
    assert_eq!(end["nextOffset"], Value::Null);
}
