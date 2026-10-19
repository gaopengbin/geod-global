//! Opt-in live renderer acceptance using the actual saved model and native
//! review/job engine. This bridge is compiled only into the test executable.
use super::*;
use tokio::net::{TcpListener, TcpStream};

fn arg<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value[key].as_str().ok_or_else(|| format!("Missing {key}"))
}
async fn dispatch(
    agent: &DesktopAgent,
    manager: &JobManager,
    name: &str,
    args: Value,
) -> Result<Value, String> {
    match name {
        "agent_snapshot" => agent.snapshot().await,
        "qa_tool_errors" => {
            let connection = agent
                .0
                .state
                .lock()
                .await
                .connection
                .clone()
                .ok_or("No model connection")?;
            let diagnostics = connection.diagnostics.lock().await;
            Ok(
                json!({"errors":diagnostics.lines().filter(|line|line.starts_with("tool-error:")).collect::<Vec<_>>()}),
            )
        }
        // Private, test-only diagnostic. It cannot approve, execute or connect
        // accounts; use it only to explain an actual dialogue tool failure.
        "qa_native_read" => {
            let tool = arg(&args, "tool")?;
            if !matches!(
                tool,
                "geod_dataset_catalog" | "geod_dataset_resolve" | "geod_scene_coverage"
            ) {
                return Err("Unsupported diagnostic read".into());
            }
            agent_actions::call(
                manager.clone(),
                arg(&args, "sessionId")?,
                tool,
                args["arguments"].clone(),
                None,
            )
            .await
        }
        "agent_send" => {
            if !args["context"].is_null() {
                serde_json::from_value::<MapContext>(args["context"].clone())
                    .map_err(|_| "Invalid map context")?
                    .validate()?;
            }
            agent.operation("send", args).await
        }
        "agent_select" => agent.operation("select", args).await,
        "agent_acknowledge_view" => agent.operation("acknowledgeView", args).await,
        "agent_goal_control" => agent.operation("goalControl", args).await,
        "agent_approve_plan" => {
            agent
                .approve_plan(
                    arg(&args, "sessionId")?,
                    arg(&args, "planId")?,
                    arg(&args, "planHash")?,
                )
                .await
        }
        "agent_plan_map_preview" => {
            agent
                .plan_map_preview(
                    arg(&args, "sessionId")?,
                    arg(&args, "planId")?,
                    arg(&args, "planHash")?,
                )
                .await
        }
        "health" => serde_json::to_value(manager.health()).map_err(|_| "Health response".into()),
        "list_public_datasets" => serde_json::to_value(geod_runtime::datasets::list())
            .map_err(|_| "Dataset response".into()),
        "resolve_public_dataset" => serde_json::to_value(geod_runtime::datasets::resolve(
            serde_json::from_value(args["request"].clone()).map_err(|_| "Dataset request")?,
        )?)
        .map_err(|_| "Dataset plan response".into()),
        "connect_public_dataset" => serde_json::to_value(
            manager
                .connect_dataset(
                    serde_json::from_value(args["request"].clone())
                        .map_err(|_| "Dataset request")?,
                )
                .await?,
        )
        .map_err(|_| "Dataset connection response".into()),
        "stac_snapshot" => serde_json::to_value(manager.stac_snapshot(arg(&args, "id")?).await?)
            .map_err(|_| "Snapshot response".into()),
        "list_jobs" => {
            serde_json::to_value(manager.list().await).map_err(|_| "Task response".into())
        }
        "list_projects" => serde_json::to_value(manager.list_projects().await)
            .map_err(|_| "Project response".into()),
        "list_provider_accounts" => serde_json::to_value(manager.provider_accounts().await)
            .map_err(|_| "Account response".into()),
        "get_proxy_settings" => serde_json::to_value(manager.proxy_settings().await)
            .map_err(|_| "Proxy response".into()),
        "inspect_raster" => serde_json::to_value(manager.inspect_raster(arg(&args, "id")?).await?)
            .map_err(|_| "Raster response".into()),
        "gscloud_inspect_original" => manager.gscloud_inspect_original(arg(&args, "id")?).await,
        "gscloud_auth_status" => manager.gscloud_auth_status().await,
        "file_thumbnail" => serde_json::to_value(manager.file_thumbnail(arg(&args, "id")?).await?)
            .map_err(|_| "Thumbnail response".into()),
        "list_recipes" => {
            serde_json::to_value(manager.list_recipes().await).map_err(|_| "Recipe response".into())
        }
        "list_vectors" => {
            serde_json::to_value(manager.list_vectors().await).map_err(|_| "Vector response".into())
        }
        "list_stac_connections" => serde_json::to_value(manager.list_stac_connections().await)
            .map_err(|_| "Source response".into()),
        "list_tile_sources" => serde_json::to_value(manager.list_tile_sources().await)
            .map_err(|_| "Tile response".into()),
        "list_wcs_connections" => serde_json::to_value(manager.list_wcs_connections().await)
            .map_err(|_| "Coverage response".into()),
        "list_map_services" => serde_json::to_value(manager.list_map_services().await)
            .map_err(|_| "Service response".into()),
        "list_feature_services" => serde_json::to_value(manager.list_feature_services().await)
            .map_err(|_| "Feature response".into()),
        "set_desktop_locale" => Ok(Value::Null),
        _ => Err(format!("Live acceptance bridge does not implement {name}")),
    }
}

async fn request(
    mut socket: TcpStream,
    agent: DesktopAgent,
    manager: JobManager,
    nonce: String,
    folder: PathBuf,
) -> Result<(), String> {
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 8192];
    let mut split = None;
    let mut length = 0usize;
    loop {
        let count = socket
            .read(&mut buffer)
            .await
            .map_err(|_| "Request interrupted")?;
        if count == 0 {
            return Ok(());
        }
        bytes.extend_from_slice(&buffer[..count]);
        if bytes.len() > 1_000_000 {
            return Err("Request exceeds bound".into());
        }
        if split.is_none() {
            if let Some(index) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                split = Some(index + 4);
                let headers = String::from_utf8_lossy(&bytes[..index]);
                length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse().ok())
                    })
                    .unwrap_or(0);
            }
        }
        if let Some(index) = split {
            if bytes.len() >= index + length {
                break;
            }
        }
    }
    let split = split.unwrap();
    let headers = String::from_utf8_lossy(&bytes[..split]);
    let first = headers.lines().next().unwrap_or("");
    let valid = headers
        .lines()
        .any(|line| line.eq_ignore_ascii_case(&format!("x-geod-qa: {nonce}")));
    let origin = headers
        .lines()
        .find(|line| line.to_ascii_lowercase().starts_with("origin:"));
    let allowed = origin.is_none_or(|line| {
        line.trim()
            .eq_ignore_ascii_case("origin: http://127.0.0.1:4317")
    });
    let (status, value) = if !allowed {
        ("403 Forbidden", json!({"error":"Wrong QA origin"}))
    } else if first.starts_with("OPTIONS ") {
        ("200 OK", Value::Null)
    } else if !valid {
        ("403 Forbidden", json!({"error":"QA session missing"}))
    } else if first.starts_with("POST /rpc ") {
        let input: Value =
            serde_json::from_slice(&bytes[split..split + length]).map_err(|_| "Invalid RPC")?;
        let name = arg(&input, "method")?;
        let args = input["args"].clone();
        match dispatch(&agent, &manager, name, args).await {
            Ok(value) => ("200 OK", value),
            Err(error) => ("400 Bad Request", json!({"error":error})),
        }
    } else if first.starts_with("POST /finish ") {
        tokio::fs::write(folder.join("finish"), b"Finished")
            .await
            .map_err(|_| "Finish signal")?;
        ("200 OK", json!({"finishing":true}))
    } else {
        ("404 Not Found", json!({"error":"Unknown QA route"}))
    };
    let body = serde_json::to_vec(&value).map_err(|_| "RPC serialization")?;
    let header=format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: http://127.0.0.1:4317\r\nAccess-Control-Allow-Headers: Content-Type, X-GeoD-QA\r\nAccess-Control-Allow-Methods: POST, OPTIONS\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",body.len());
    socket
        .write_all(header.as_bytes())
        .await
        .map_err(|_| "RPC header")?;
    socket.write_all(&body).await.map_err(|_| "RPC body")?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Explicit live model/download/renderer acceptance; private loopback test bridge"]
async fn live_saved_model_scene_ui_server() {
    assert_eq!(
        std::env::var("GEOD_SCENE_UI_ACCEPTANCE").as_deref(),
        Ok("1")
    );
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap();
    let base = PathBuf::from(std::env::var("GEOD_SCENE_UI_OUTPUT").unwrap())
        .canonicalize()
        .unwrap();
    assert!(base.starts_with(root.join(".verification")));
    tokio::fs::create_dir_all(base.join("agent")).await.unwrap();
    let profile = PathBuf::from(std::env::var("GEOD_SCENE_UI_PROFILE").unwrap());
    let original = tokio::fs::read(profile.join("agent/registry.json"))
        .await
        .unwrap();
    let mut registry: registry::Registry = serde_json::from_slice(&original).unwrap();
    let selected = registry.selected().unwrap().clone();
    registry.connections = vec![selected.clone()];
    assert!(
        registry::Vault::read(&registry::NativeVault, &selected.credential_ref)
            .unwrap()
            .is_some()
    );
    tokio::fs::write(
        base.join("agent/registry.json"),
        serde_json::to_vec_pretty(&registry).unwrap(),
    )
    .await
    .unwrap();
    // Explicit opt-in uses the already-authorized desktop account in its own
    // vault namespace. Never copy website sessions into a test credential store.
    let runtime = if std::env::var("GEOD_SCENE_UI_DESKTOP_RUNTIME").as_deref() == Ok("1") {
        profile.join("runtime")
    } else {
        base.join("core")
    };
    let manager = JobManager::open(&runtime).await.unwrap();
    let agent = DesktopAgent::open(
        base.join("agent"),
        root.join(".agent-runtime/win32-x64"),
        manager.clone(),
    )
    .await
    .unwrap();
    assert_eq!(agent.snapshot().await.unwrap()["configured"], true);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let nonce = format!("{}{}", registry::new_id(), registry::new_id());
    let ready = json!({"url":format!("http://127.0.0.1:{port}"),"nonce":nonce,"directory":base,"runtimeDirectory":runtime,"model":selected.settings,"pid":std::process::id(),"nativeWindowTested":false,"fixtureReplies":false,"credentialVaultWritten":false});
    tokio::fs::write(
        base.join("ready.json"),
        serde_json::to_vec_pretty(&ready).unwrap(),
    )
    .await
    .unwrap();
    println!("Live saved-model UI bridge ready; no synthetic model responses.");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5400);
    while tokio::time::Instant::now() < deadline && !base.join("finish").exists() {
        if let Ok(Ok((socket, _))) =
            tokio::time::timeout(Duration::from_secs(1), listener.accept()).await
        {
            let agent = agent.clone();
            let manager = manager.clone();
            let nonce = nonce.clone();
            let folder = base.clone();
            tokio::spawn(async move {
                let _ = request(socket, agent, manager, nonce, folder).await;
            });
        }
    }
    let snapshot = agent.snapshot().await.unwrap();
    let jobs = manager.list().await;
    tokio::fs::write(
        base.join("final-snapshot.json"),
        serde_json::to_vec_pretty(&snapshot).unwrap(),
    )
    .await
    .unwrap();
    tokio::fs::write(
        base.join("final-jobs.json"),
        serde_json::to_vec_pretty(&jobs).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(
        tokio::fs::read(profile.join("agent/registry.json"))
            .await
            .unwrap(),
        original
    );
    agent.shutdown().await;
    manager.shutdown().await.unwrap();
}
