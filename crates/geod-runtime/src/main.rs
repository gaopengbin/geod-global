use geod_runtime::{CreateJobRequest, Job, JobManager, JobStatus, RasterRecipe};
use serde_json::{json, Value};
use std::{collections::BTreeMap, time::Duration};
mod cli_rgb;
mod cli_three_d;
mod cli_tiles;
mod cli_wcs;

#[tokio::main]
async fn main() {
    // Heap allocation bounds future storage, but not debug poll-frame stack
    // temporaries. Poll the dispatcher on Tokio's worker instead of Windows'
    // smaller entry-thread stack. The spawned block constructs run() there.
    let result = tokio::spawn(async { Box::pin(run()).await })
        .await
        .unwrap_or_else(|error| Err(format!("Command dispatcher failed: {error}")));
    if let Err(error) = result {
        eprintln!("{}", json!({"error":error}));
        std::process::exit(1);
    }
}

fn print_json(value: &impl serde::Serialize) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn options(args: impl Iterator<Item = String>) -> Result<BTreeMap<String, String>, String> {
    let mut args = args;
    let mut options = BTreeMap::new();
    while let Some(key) = args.next() {
        if !key.starts_with("--") {
            return Err(format!("Expected an option, received {key}"));
        }
        let value = args
            .next()
            .filter(|value| !value.starts_with("--"))
            .ok_or_else(|| format!("Missing value for {key}"))?;
        if options.insert(key.clone(), value).is_some() {
            return Err(format!("Repeated option {key}"));
        }
    }
    Ok(options)
}

fn required<'a>(options: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, String> {
    options
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| format!("{key} is required"))
}

fn loopback_server(value: &str) -> Result<String, String> {
    let url = url::Url::parse(value).map_err(|_| "Invalid --server URL")?;
    if url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || url.port_or_known_default() == Some(0)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        return Err("--server must be an HTTP 127.0.0.1 origin with no credentials, path, query or fragment".into());
    }
    Ok(url.origin().ascii_serialization())
}

async fn read_json<T: serde::de::DeserializeOwned>(
    path: &str,
    max_bytes: u64,
) -> Result<T, String> {
    let metadata = tokio::fs::metadata(path)
        .await
        .map_err(|e| format!("Cannot open JSON input: {e}"))?;
    if !metadata.is_file() || metadata.len() > max_bytes {
        return Err(format!(
            "JSON input must be a file no larger than {max_bytes} bytes"
        ));
    }
    let bytes = tokio::fs::read(path).await.map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| format!("Invalid JSON input: {e}"))
}

enum Backend {
    Direct(JobManager),
    Server {
        base: String,
        client: reqwest::Client,
    },
}

impl Backend {
    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value, String> {
        let Self::Server { base, client } = self else {
            return Err("Not a server connection".into());
        };
        let mut request = client
            .request(method, format!("{base}{path}"))
            .header("X-GeoD-Client", "geod-global");
        if let Some(body) = body {
            request = request
                .header("Content-Type", "application/json")
                .body(serde_json::to_vec(&body).map_err(|e| e.to_string())?);
        }
        let response = request
            .send()
            .await
            .map_err(|e| format!("Local runtime request failed: {e}"))?;
        let status = response.status();
        let bytes = response.bytes().await.map_err(|e| e.to_string())?;
        let value: Value = serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| json!({"error":String::from_utf8_lossy(&bytes)}));
        if !status.is_success() {
            return Err(format!(
                "HTTP {status}: {}",
                value.get("error").unwrap_or(&value)
            ));
        }
        Ok(value)
    }

    async fn wait(&self, id: &str) -> Result<Job, String> {
        match self {
            Self::Direct(manager) => manager.wait(id).await,
            Self::Server { .. } => loop {
                let snapshot = self
                    .request(reqwest::Method::GET, &format!("/jobs/{id}"), None)
                    .await?;
                let settled = snapshot.get("settled").and_then(Value::as_bool)
                    .ok_or("The task service does not report worker settlement; restart it with the current runtime version")?;
                let job: Job = serde_json::from_value(snapshot).map_err(|e| e.to_string())?;
                if settled && !matches!(job.status, JobStatus::Queued | JobStatus::Running) {
                    return Ok(job);
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            },
        }
    }

    async fn cancel(&self, id: &str) -> Result<(), String> {
        match self {
            Self::Direct(manager) => {
                manager.cancel(id).await?;
            }
            Self::Server { .. } => {
                self.request(reqwest::Method::POST, &format!("/jobs/{id}/cancel"), None)
                    .await?;
            }
        }
        Ok(())
    }
}

async fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>().into_iter();
    let group = args.next().unwrap_or_else(|| "--help".into());
    if matches!(group.as_str(), "--help" | "help" | "-h") {
        return print_json(&json!({"commands":[
            "serve --data-dir DIR [--port 4318]",
            "scientific-rgb plan|run --request FILE (--data-dir DIR | --server http://127.0.0.1:4318)",
            "scientific-rgb inspect|package --id UUID (--data-dir DIR | --server http://127.0.0.1:4318)",
            "scientific-rgb pixel --id UUID --x X --y Y (--data-dir DIR | --server http://127.0.0.1:4318)",
            "serve-mcp (--data-dir DIR | --server http://127.0.0.1:4318) [--allow-write]",
            "jobs list|status|cancel|retry|inspect [--id UUID] (--data-dir DIR | --server http://127.0.0.1:4318)",
            "jobs download --request FILE (--data-dir DIR | --server http://127.0.0.1:4318)",
            "vectors list|inspect|forget [--id UUID] (--data-dir DIR | --server http://127.0.0.1:4318)",
            "vectors open --file FILE [--mode reference|managed] --data-dir DIR",
            "vectors export --id UUID --out FILE [--format geojson|original] --data-dir DIR",
            "feature-services list|forget [--id UUID] (--data-dir DIR | --server http://127.0.0.1:4318)",
            "feature-services connect|query --request FILE (--data-dir DIR | --server http://127.0.0.1:4318)",
            "stac list|forget|inspect [--id UUID] (--data-dir DIR | --server http://127.0.0.1:4318)",
            "stac snapshot --id SHA256 (--data-dir DIR | --server http://127.0.0.1:4318)",
            "stac connect|search|project|download --request FILE (--data-dir DIR | --server http://127.0.0.1:4318)",
            "stac pixel --id UUID --column N --row N (--data-dir DIR | --server http://127.0.0.1:4318)",
            "wcs connect|describe|plan|project|download --request FILE (--data-dir DIR | --server http://127.0.0.1:4318)",
            "wcs list|forget|inspect [--id UUID] (--data-dir DIR | --server http://127.0.0.1:4318)",
            "wcs description|snapshot --id SHA256 (--data-dir DIR | --server http://127.0.0.1:4318)",
            "wcs pixel --id UUID --column N --row N (--data-dir DIR | --server http://127.0.0.1:4318)",
            "map-services list|forget [--id UUID] (--data-dir DIR | --server http://127.0.0.1:4318)",
            "map-services connect --request FILE (--data-dir DIR | --server http://127.0.0.1:4318)",
            "tile-sources list|forget [--id UUID] (--data-dir DIR | --server http://127.0.0.1:4318)",
            "tile-sources connect --request FILE (--data-dir DIR | --server http://127.0.0.1:4318)",
            "tile-packages list|inspect [--id UUID] (--data-dir DIR | --server http://127.0.0.1:4318)",
            "tile-packages extract|tile --request FILE (--data-dir DIR | --server http://127.0.0.1:4318)",
            "tile-packages export --id UUID --out FILE --data-dir DIR",
            "three-d list|inspect [--id UUID] (--data-dir DIR | --server http://127.0.0.1:4318)",
            "three-d discover|save|resource --request FILE (--data-dir DIR | --server http://127.0.0.1:4318)",
            "three-d open --file FILE --request FILE --data-dir DIR",
            "three-d export --id UUID --out FILE --data-dir DIR",
            "tile-packages open --file FILE --data-dir DIR",
            "map-images list|inspect [--id UUID] (--data-dir DIR | --server http://127.0.0.1:4318)",
            "map-images get --request FILE (--data-dir DIR | --server http://127.0.0.1:4318)",
            "map-images export --id UUID --out FILE --data-dir DIR",
            "recipes list (--data-dir DIR | --server http://127.0.0.1:4318)",
            "recipes plan|save|run --recipe FILE (--data-dir DIR | --server http://127.0.0.1:4318)"
        ],"notes":["download, retry and run wait until terminal; success exits 0, failures exit 1", "Use --server while a service owns storage; direct mode requires exclusive access", "Recipe imports depend on the pinned local source job and do not download it"]}));
    }
    if group == "serve-mcp" {
        // Do not embed another startup state machine in the dispatcher frame:
        // direct MCP also validates persisted, nested coverage records here.
        let options = geod_runtime::mcp::Options::parse(args)?;
        return Box::pin(geod_runtime::mcp::serve(options)).await;
    }
    let command = if group == "serve" {
        "serve".into()
    } else {
        args.next().ok_or("A subcommand is required; use --help")?
    };
    let options = options(args)?;
    let mut allowed = vec!["--data-dir", "--server"];
    match (group.as_str(), command.as_str()) {
        ("serve", "serve") => allowed = vec!["--data-dir", "--port"],
        ("jobs", "list") | ("recipes", "list") | ("vectors", "list") => {}
        ("vectors", "open") => allowed.extend(["--file", "--mode"]),
        ("vectors", "export") => allowed.extend(["--id", "--out", "--format"]),
        ("vectors", "inspect" | "forget") => allowed.push("--id"),
        ("feature-services", "list") => {}
        ("stac", "list") => {}
        ("stac", "forget" | "snapshot" | "inspect") => allowed.push("--id"),
        ("stac", "connect" | "search" | "project" | "download") => allowed.push("--request"),
        ("stac", "pixel") => allowed.extend(["--id", "--column", "--row"]),
        ("wcs", "list") => {}
        ("wcs", "forget" | "inspect" | "description" | "snapshot") => allowed.push("--id"),
        ("wcs", "connect" | "describe" | "plan" | "project" | "download") => {
            allowed.push("--request")
        }
        ("wcs", "pixel") => allowed.extend(["--id", "--column", "--row"]),
        ("feature-services", "forget") => allowed.push("--id"),
        ("feature-services", "connect" | "query") => allowed.push("--request"),
        ("map-services", "list") | ("map-images", "list") => {}
        ("map-services", "forget") | ("map-images", "inspect") => allowed.push("--id"),
        ("map-services", "connect") | ("map-images", "get") => allowed.push("--request"),
        ("map-images", "export") => allowed.extend(["--id", "--out"]),
        ("tile-sources", "list") | ("tile-packages", "list") => {}
        ("tile-sources", "connect") | ("tile-packages", "extract" | "tile") => {
            allowed.push("--request")
        }
        ("tile-sources", "forget") | ("tile-packages", "inspect") => allowed.push("--id"),
        ("tile-packages", "export") => allowed.extend(["--id", "--out"]),
        ("tile-packages", "open") => allowed.push("--file"),
        ("three-d", "list") => {}
        ("scientific-rgb", "plan" | "run") => allowed.push("--request"),
        ("scientific-rgb", "inspect" | "package") => allowed.push("--id"),
        ("scientific-rgb", "pixel") => allowed.extend(["--id", "--x", "--y"]),
        ("three-d", "discover" | "save" | "resource") => allowed.push("--request"),
        ("three-d", "inspect") => allowed.push("--id"),
        ("three-d", "export") => allowed.extend(["--id", "--out"]),
        ("three-d", "open") => allowed.extend(["--file", "--request"]),
        ("jobs", "status" | "cancel" | "retry" | "inspect") => allowed.push("--id"),
        ("jobs", "download") => allowed.push("--request"),
        ("recipes", "plan" | "save" | "run") => allowed.push("--recipe"),
        _ => return Err("Unknown command; use --help".into()),
    }
    for key in options.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("Unknown option for this command: {key}"));
        }
    }
    if group == "serve" {
        let port = options
            .get("--port")
            .map(|value| value.parse::<u16>())
            .transpose()
            .map_err(|_| "Invalid port")?
            .unwrap_or(4318);
        if port == 0 {
            return Err("Choose an explicit nonzero port".into());
        }
        return geod_runtime::service::serve(
            JobManager::open(required(&options, "--data-dir")?).await?,
            port,
        )
        .await;
    }
    let backend = match (options.get("--data-dir"), options.get("--server")) {
        (Some(path), None) => Backend::Direct(JobManager::open(path).await?),
        (None, Some(server)) => Backend::Server {
            base: loopback_server(server)?,
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .no_proxy()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(
                    if matches!(group.as_str(), "tile-packages" | "three-d") {
                        190
                    } else {
                        120
                    },
                ))
                .build()
                .map_err(|e| e.to_string())?,
        },
        _ => return Err("Choose exactly one of --data-dir or --server".into()),
    };
    let id = options.get("--id").map(String::as_str).unwrap_or("");
    if group == "scientific-rgb" {
        return Box::pin(cli_rgb::run(&backend, &command, id, &options)).await;
    }
    if group == "three-d" {
        return cli_three_d::run(&backend, &command, id, &options).await;
    }
    if matches!(group.as_str(), "tile-sources" | "tile-packages") {
        return cli_tiles::run(&backend, &group, &command, id, &options).await;
    }
    if matches!(group.as_str(), "map-services" | "map-images") {
        if matches!(command.as_str(), "inspect" | "forget" | "export")
            && uuid::Uuid::parse_str(id)
                .ok()
                .map(|u| u.to_string())
                .as_deref()
                != Some(id)
        {
            return Err("A canonical UUID is required for --id".into());
        }
        let request = if matches!(command.as_str(), "connect" | "get") {
            Some(read_json::<Value>(required(&options, "--request")?, 2 * 1024 * 1024).await?)
        } else {
            None
        };
        let value = match &backend {
            Backend::Direct(m) => match (group.as_str(), command.as_str()) {
                ("map-services", "list") => serde_json::to_value(m.list_map_services().await),
                ("map-services", "connect") => serde_json::to_value(
                    m.connect_map_service(
                        serde_json::from_value(request.unwrap()).map_err(|e| e.to_string())?,
                    )
                    .await?,
                ),
                ("map-services", "forget") => {
                    m.forget_map_service(id).await?;
                    Ok(json!({"removed":true}))
                }
                ("map-images", "list") => serde_json::to_value(m.list_map_images().await),
                ("map-images", "get") => serde_json::to_value(
                    m.get_map_image(
                        serde_json::from_value(request.unwrap()).map_err(|e| e.to_string())?,
                    )
                    .await?,
                ),
                ("map-images", "inspect") => serde_json::to_value(m.inspect_map_image(id).await?),
                ("map-images", "export") => {
                    m.export_map_image_path(id, required(&options, "--out")?.into())
                        .await?;
                    Ok(json!({"exported":true}))
                }
                _ => return Err("Unknown map service operation".into()),
            }
            .map_err(|e| e.to_string())?,
            Backend::Server { .. } => {
                let (method, path) = match (group.as_str(), command.as_str()) {
                    ("map-services", "list") => (reqwest::Method::GET, "/map-services".into()),
                    ("map-services", "connect") => (reqwest::Method::POST, "/map-services".into()),
                    ("map-services", "forget") => {
                        (reqwest::Method::POST, format!("/map-services/{id}/forget"))
                    }
                    ("map-images", "list") => (reqwest::Method::GET, "/map-images".into()),
                    ("map-images", "get") => (reqwest::Method::POST, "/map-images".into()),
                    ("map-images", "inspect") => {
                        (reqwest::Method::GET, format!("/map-images/{id}"))
                    }
                    _ => return Err(
                        "Map ZIP export requires direct --data-dir mode or the desktop save dialog"
                            .into(),
                    ),
                };
                backend.request(method, &path, request).await?
            }
        };
        return print_json(&value);
    }
    if group == "wcs" {
        return cli_wcs::run(&backend, &command, id, &options).await;
    }
    if group == "stac" {
        if matches!(command.as_str(), "forget" | "inspect" | "pixel")
            && uuid::Uuid::parse_str(id)
                .ok()
                .map(|u| u.to_string())
                .as_deref()
                != Some(id)
        {
            return Err("A canonical UUID is required for --id".into());
        }
        if command == "snapshot"
            && (id.len() != 64
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
        {
            return Err("A snapshot SHA-256 identifier is required for --id".into());
        }
        let request = if matches!(
            command.as_str(),
            "connect" | "search" | "project" | "download"
        ) {
            Some(read_json::<Value>(required(&options, "--request")?, 2 * 1024 * 1024).await?)
        } else {
            None
        };
        let pixel = if command == "pixel" {
            Some((
                required(&options, "--column")?
                    .parse::<u32>()
                    .map_err(|_| "Invalid column")?,
                required(&options, "--row")?
                    .parse::<u32>()
                    .map_err(|_| "Invalid row")?,
            ))
        } else {
            None
        };
        let value = match &backend {
            Backend::Direct(manager) => match command.as_str() {
                "list" => serde_json::to_value(manager.list_stac_connections().await),
                "forget" => {
                    manager.forget_stac_connection(id).await?;
                    Ok(json!({"removed":true}))
                }
                "connect" => serde_json::to_value(
                    manager
                        .connect_stac(
                            serde_json::from_value(request.unwrap()).map_err(|e| e.to_string())?,
                        )
                        .await?,
                ),
                "search" => serde_json::to_value(
                    manager
                        .search_stac(
                            serde_json::from_value(request.unwrap()).map_err(|e| e.to_string())?,
                        )
                        .await?,
                ),
                "snapshot" => serde_json::to_value(manager.stac_snapshot(id).await?),
                "project" => serde_json::to_value(
                    manager
                        .save_stac_project(
                            serde_json::from_value(request.unwrap()).map_err(|e| e.to_string())?,
                        )
                        .await?,
                ),
                "download" => serde_json::to_value(
                    manager
                        .download_stac_project(
                            serde_json::from_value(request.unwrap()).map_err(|e| e.to_string())?,
                        )
                        .await?,
                ),
                "inspect" => serde_json::to_value(manager.inspect_stac_asset(id).await?),
                "pixel" => {
                    let (column, row) = pixel.unwrap();
                    serde_json::to_value(manager.sample_stac_asset(id, column, row).await?)
                }
                _ => return Err("Unknown custom raster operation".into()),
            }
            .map_err(|e| e.to_string())?,
            Backend::Server { .. } => {
                let (method, path) = match command.as_str() {
                    "list" => (reqwest::Method::GET, "/stac/connections".into()),
                    "connect" => (reqwest::Method::POST, "/stac/connections".into()),
                    "forget" => (
                        reqwest::Method::POST,
                        format!("/stac/connections/{id}/forget"),
                    ),
                    "search" => (reqwest::Method::POST, "/stac/search".into()),
                    "snapshot" => (reqwest::Method::GET, format!("/stac/snapshots/{id}")),
                    "project" => (reqwest::Method::POST, "/stac/project".into()),
                    "download" => (reqwest::Method::POST, "/stac/downloads".into()),
                    "inspect" => (reqwest::Method::GET, format!("/stac/jobs/{id}/inspect")),
                    "pixel" => {
                        let (column, row) = pixel.unwrap();
                        (
                            reqwest::Method::GET,
                            format!("/stac/jobs/{id}/pixel?column={column}&row={row}"),
                        )
                    }
                    _ => return Err("Unknown custom raster operation".into()),
                };
                backend.request(method, &path, request).await?
            }
        };
        if command == "download" {
            let ids: Vec<String> = value
                .get("jobs")
                .and_then(Value::as_array)
                .ok_or("Invalid download queue response")?
                .iter()
                .map(|job| {
                    job.get("id")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .ok_or("Invalid job ID")
                })
                .collect::<Result<_, _>>()?;
            let wait_all = async {
                let mut completed = Vec::new();
                for id in &ids {
                    completed.push(backend.wait(id).await?);
                }
                Ok::<_, String>(completed)
            };
            let completed = tokio::select! {
                result = wait_all => result?,
                _ = tokio::signal::ctrl_c() => {
                    for id in &ids { backend.cancel(id).await?; }
                    let mut completed = Vec::new();
                    for id in &ids { completed.push(backend.wait(id).await?); }
                    completed
                }
            };
            let succeeded = completed
                .iter()
                .all(|job| job.status == JobStatus::Succeeded);
            let mut value = value;
            value["jobs"] = serde_json::to_value(completed).map_err(|e| e.to_string())?;
            print_json(&value)?;
            return if succeeded {
                Ok(())
            } else {
                Err("One or more downloads did not complete".into())
            };
        }
        return print_json(&value);
    }
    if group == "feature-services" {
        if command == "forget"
            && uuid::Uuid::parse_str(id)
                .ok()
                .map(|u| u.to_string())
                .as_deref()
                != Some(id)
        {
            return Err("A canonical UUID is required for --id".into());
        }
        let request = if matches!(command.as_str(), "connect" | "query") {
            Some(read_json::<Value>(required(&options, "--request")?, 2 * 1024 * 1024).await?)
        } else {
            None
        };
        let value = match &backend {
            Backend::Direct(manager) => match command.as_str() {
                "list" => serde_json::to_value(manager.list_feature_services().await),
                "forget" => {
                    manager.forget_feature_service(id).await?;
                    Ok(json!({"removed":true}))
                }
                "connect" => serde_json::to_value(
                    manager
                        .connect_feature_service(
                            serde_json::from_value(request.unwrap()).map_err(|e| e.to_string())?,
                        )
                        .await?,
                ),
                "query" => serde_json::to_value(
                    manager
                        .query_features(
                            serde_json::from_value(request.unwrap()).map_err(|e| e.to_string())?,
                        )
                        .await?,
                ),
                _ => return Err("Unknown data service operation".into()),
            }
            .map_err(|e| e.to_string())?,
            Backend::Server { .. } => {
                let (method, path) = match command.as_str() {
                    "list" => (reqwest::Method::GET, "/feature-services".into()),
                    "connect" => (reqwest::Method::POST, "/feature-services".into()),
                    "forget" => (
                        reqwest::Method::POST,
                        format!("/feature-services/{id}/forget"),
                    ),
                    "query" => (reqwest::Method::POST, "/feature-services/query".into()),
                    _ => return Err("Unknown data service operation".into()),
                };
                backend.request(method, &path, request).await?
            }
        };
        return print_json(&value);
    }
    if group == "vectors" {
        if matches!(command.as_str(), "inspect" | "forget" | "export")
            && uuid::Uuid::parse_str(id).is_err()
        {
            return Err("--id must be a vector file UUID".into());
        }
        let value = match &backend {
            Backend::Direct(manager) => match command.as_str() {
                "list" => serde_json::to_value(manager.list_vectors().await),
                "inspect" => serde_json::to_value(manager.inspect_vector(id).await?),
                "forget" => {
                    manager.forget_vector(id).await?;
                    Ok(json!({"removed":true}))
                }
                "open" => {
                    let managed = match options
                        .get("--mode")
                        .map(String::as_str)
                        .unwrap_or("reference")
                    {
                        "reference" => false,
                        "managed" => true,
                        _ => return Err("Vector mode must be reference or managed".into()),
                    };
                    serde_json::to_value(
                        manager
                            .open_vector_path(required(&options, "--file")?.into(), managed)
                            .await?,
                    )
                }
                "export" => {
                    let path = required(&options, "--out")?.into();
                    match options
                        .get("--format")
                        .map(String::as_str)
                        .unwrap_or("geojson")
                    {
                        "geojson" => manager.export_vector_path(id, path).await?,
                        "original" => manager.export_vector_original_path(id, path).await?,
                        _ => return Err("Vector export format must be geojson or original".into()),
                    }
                    Ok(json!({"saved":true}))
                }
                _ => unreachable!(),
            }
            .map_err(|e| e.to_string())?,
            Backend::Server { .. } => {
                let (method, path) = match command.as_str() {
                    "list" => (reqwest::Method::GET, "/vectors".to_string()),
                    "inspect" => (reqwest::Method::GET, format!("/vectors/{id}")),
                    "forget" => (reqwest::Method::POST, format!("/vectors/{id}/forget")),
                    _ => return Err(
                        "Opening and saving native paths require --data-dir with exclusive access"
                            .into(),
                    ),
                };
                backend.request(method, &path, None).await?
            }
        };
        return print_json(&value);
    }
    if group == "jobs"
        && matches!(command.as_str(), "status" | "cancel" | "retry" | "inspect")
        && uuid::Uuid::parse_str(id).is_err()
    {
        return Err("--id must be a job UUID".into());
    }
    let recipe: Option<RasterRecipe> = if group == "recipes" && command != "list" {
        let recipe: RasterRecipe = read_json(required(&options, "--recipe")?, 512_000).await?;
        recipe.validate()?;
        Some(recipe)
    } else {
        None
    };
    let request: Option<CreateJobRequest> = if group == "jobs" && command == "download" {
        Some(read_json(required(&options, "--request")?, 8192).await?)
    } else {
        None
    };
    let value = match &backend {
        Backend::Direct(manager) => match (group.as_str(), command.as_str()) {
            ("jobs", "list") => serde_json::to_value(manager.list().await),
            ("jobs", "status") => serde_json::to_value(manager.get(id).await.ok_or("Unknown job")?),
            ("jobs", "cancel") => serde_json::to_value(manager.cancel(id).await?),
            ("jobs", "retry") => serde_json::to_value(manager.retry(id).await?),
            ("jobs", "inspect") => serde_json::to_value(manager.inspect_raster(id).await?),
            ("jobs", "download") => serde_json::to_value(manager.create(request.unwrap()).await?),
            ("recipes", "list") => serde_json::to_value(manager.list_recipes().await),
            ("recipes", "plan") => {
                serde_json::to_value(manager.plan_recipe(recipe.unwrap()).await?)
            }
            ("recipes", "save") => {
                serde_json::to_value(manager.save_recipe(recipe.unwrap()).await?)
            }
            ("recipes", "run") => serde_json::to_value(manager.run_recipe(recipe.unwrap()).await?),
            _ => unreachable!(),
        }
        .map_err(|e| e.to_string())?,
        Backend::Server { .. } => {
            let (method, path, body) = match (group.as_str(), command.as_str()) {
                ("jobs", "list") => (reqwest::Method::GET, "/jobs".into(), None),
                ("jobs", "status") => (reqwest::Method::GET, format!("/jobs/{id}"), None),
                ("jobs", "inspect") => (reqwest::Method::GET, format!("/jobs/{id}/raster"), None),
                ("jobs", "cancel" | "retry") => {
                    (reqwest::Method::POST, format!("/jobs/{id}/{command}"), None)
                }
                ("jobs", "download") => (
                    reqwest::Method::POST,
                    "/jobs".into(),
                    Some(serde_json::to_value(request.unwrap()).map_err(|e| e.to_string())?),
                ),
                ("recipes", "list") => (reqwest::Method::GET, "/recipes".into(), None),
                ("recipes", "plan" | "run" | "save") => (
                    reqwest::Method::POST,
                    if command == "save" {
                        "/recipes".into()
                    } else {
                        format!("/recipes/{command}")
                    },
                    Some(serde_json::to_value(recipe.unwrap()).map_err(|e| e.to_string())?),
                ),
                _ => unreachable!(),
            };
            backend.request(method, &path, body).await?
        }
    };
    if command == "cancel" {
        let job: Job = serde_json::from_value(value).map_err(|e| e.to_string())?;
        return print_json(&backend.wait(&job.id).await?);
    }
    if matches!(command.as_str(), "run" | "download" | "retry") {
        let job: Job = serde_json::from_value(value).map_err(|e| e.to_string())?;
        let completed = tokio::select! {
            result = backend.wait(&job.id) => result?,
            _ = tokio::signal::ctrl_c() => { backend.cancel(&job.id).await?; backend.wait(&job.id).await? },
        };
        print_json(&completed)?;
        if completed.status != JobStatus::Succeeded {
            return Err(completed
                .error
                .unwrap_or_else(|| "The job did not succeed".into()));
        }
        return Ok(());
    }
    print_json(&value)
}

#[cfg(test)]
mod tests {

    use super::*;
    #[test]
    fn server_adapter_rejects_non_loopback_and_credential_urls() {
        assert_eq!(
            loopback_server("http://127.0.0.1:4318/").unwrap(),
            "http://127.0.0.1:4318"
        );
        for value in [
            "https://127.0.0.1:4318",
            "http://localhost:4318",
            "http://127.0.0.1.evil:4318",
            "http://user@127.0.0.1:4318",
            "http://127.0.0.1:4318/private",
            "http://127.0.0.1:4318?key=x",
        ] {
            assert!(loopback_server(value).is_err());
        }
    }
    #[test]
    fn options_reject_duplicate_and_missing_values() {
        assert!(options(["--id", "x", "--id", "y"].into_iter().map(String::from)).is_err());
        assert!(options(["--id", "--data-dir"].into_iter().map(String::from)).is_err());
    }

    #[tokio::test]
    async fn server_wait_does_not_return_cancelled_until_worker_settles() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        let polls = Arc::new(AtomicUsize::new(0));
        let observed = polls.clone();
        let app = axum::Router::new().route("/jobs/{id}", axum::routing::get(move || {
            let calls = observed.clone();
            async move {
                let settled = calls.fetch_add(1, Ordering::SeqCst) > 0;
                axum::Json(json!({
                    "id":"11111111-1111-4111-8111-111111111111", "itemId":"S2_TEST", "assetKey":"scl",
                    "href":"https://example.invalid/SCL.tif", "mediaType":"image/tiff", "title":"test",
                    "status":"cancelled", "bytesDownloaded":0, "totalBytes":null, "sha256":null,
                    "outputPath":null, "error":"Cancelled", "createdAt":"2026-09-22T00:00:00Z",
                    "updatedAt":"2026-09-22T00:00:00Z", "source":"fixture", "validation":"pending", "attempts":1,
                    "settled":settled
                }))
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let service = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let backend = Backend::Server {
            base,
            client: reqwest::Client::builder().no_proxy().build().unwrap(),
        };
        let result = tokio::time::timeout(
            Duration::from_secs(3),
            backend.wait("11111111-1111-4111-8111-111111111111"),
        )
        .await
        .unwrap()
        .unwrap();
        service.abort();
        assert_eq!(result.status, JobStatus::Cancelled);
        assert_eq!(
            polls.load(Ordering::SeqCst),
            2,
            "terminal status alone is not a completed cancellation"
        );
    }
}
