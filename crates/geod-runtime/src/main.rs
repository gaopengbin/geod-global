use geod_runtime::{CreateJobRequest, Job, JobManager, JobStatus, RasterRecipe};
use serde_json::{json, Value};
use std::{collections::BTreeMap, time::Duration};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
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
    let mut args = std::env::args().skip(1);
    let group = args.next().unwrap_or_else(|| "--help".into());
    if matches!(group.as_str(), "--help" | "help" | "-h") {
        return print_json(&json!({"commands":[
            "serve --data-dir DIR [--port 4318]",
            "serve-mcp (--data-dir DIR | --server http://127.0.0.1:4318) [--allow-write]",
            "jobs list|status|cancel|retry|inspect [--id UUID] (--data-dir DIR | --server http://127.0.0.1:4318)",
            "jobs download --request FILE (--data-dir DIR | --server http://127.0.0.1:4318)",
            "recipes list (--data-dir DIR | --server http://127.0.0.1:4318)",
            "recipes plan|save|run --recipe FILE (--data-dir DIR | --server http://127.0.0.1:4318)"
        ],"notes":["download, retry and run wait until terminal; success exits 0, failures exit 1", "Use --server while a service owns storage; direct mode requires exclusive access", "Recipe imports depend on the pinned local source job and do not download it"]}));
    }
    if group == "serve-mcp" {
        return geod_runtime::mcp::serve(geod_runtime::mcp::Options::parse(args)?).await;
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
        ("jobs", "list") | ("recipes", "list") => {}
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
                .timeout(Duration::from_secs(120))
                .build()
                .map_err(|e| e.to_string())?,
        },
        _ => return Err("Choose exactly one of --data-dir or --server".into()),
    };
    let id = options.get("--id").map(String::as_str).unwrap_or("");
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
