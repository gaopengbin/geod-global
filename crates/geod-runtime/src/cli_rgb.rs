use super::{print_json, read_json, required, Backend};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub(super) async fn run(
    backend: &Backend,
    command: &str,
    id: &str,
    options: &BTreeMap<String, String>,
) -> Result<(), String> {
    let request = if matches!(command, "plan" | "run") {
        Some(read_json::<geod_runtime::RgbRequest>(required(options, "--request")?, 8192).await?)
    } else {
        None
    };
    if matches!(command, "inspect" | "pixel" | "package")
        && uuid::Uuid::parse_str(id)
            .ok()
            .map(|u| u.to_string())
            .as_deref()
            != Some(id)
    {
        return Err("--id must be a canonical local RGB job UUID".into());
    }
    let point = if command == "pixel" {
        Some([
            required(options, "--x")?
                .parse::<f64>()
                .map_err(|_| "Invalid --x")?,
            required(options, "--y")?
                .parse::<f64>()
                .map_err(|_| "Invalid --y")?,
        ])
    } else {
        None
    };
    if point.is_some_and(|p| p.iter().any(|v| !v.is_finite())) {
        return Err("RGB pixel coordinates must be finite".into());
    }
    let value = match backend {
        Backend::Direct(manager) => Box::pin(direct(manager, command, id, request, point)).await?,
        Backend::Server { .. } => Box::pin(server(backend, command, id, request, point)).await?,
    };
    let mut value: Value = value;
    if command == "inspect" {
        value
            .as_object_mut()
            .ok_or("Invalid RGB response")?
            .remove("previewDataUrl");
        value["previewOmitted"] = json!(true);
    }
    print_json(&value)
}

// Keep mutually exclusive native and HTTP operations in separate poll frames.
// A small future alone does not bound debug poll-frame stack temporaries.
async fn direct(
    manager: &geod_runtime::JobManager,
    command: &str,
    id: &str,
    request: Option<geod_runtime::RgbRequest>,
    point: Option<[f64; 2]>,
) -> Result<Value, String> {
    match command {
        "plan" => {
            serde_json::to_value(Box::pin(manager.plan_scientific_rgb(request.unwrap())).await?)
        }
        "run" => {
            let queued = Box::pin(manager.run_scientific_rgb(request.unwrap())).await?;
            let done = manager.wait(&queued.id).await?;
            if done.status != geod_runtime::JobStatus::Succeeded {
                return Err(done
                    .error
                    .unwrap_or("Scientific RGB processing failed".into()));
            }
            serde_json::to_value(done)
        }
        "inspect" => serde_json::to_value(Box::pin(manager.inspect_scientific_rgb(id)).await?),
        "pixel" => {
            let [x, y] = point.unwrap();
            serde_json::to_value(Box::pin(manager.sample_scientific_rgb(id, x, y)).await?)
        }
        "package" => serde_json::to_value(Box::pin(manager.prepare_artifact(id)).await?),
        _ => return Err("Unknown scientific RGB command".into()),
    }
    .map_err(|e| e.to_string())
}
async fn server(
    backend: &Backend,
    command: &str,
    id: &str,
    request: Option<geod_runtime::RgbRequest>,
    point: Option<[f64; 2]>,
) -> Result<Value, String> {
    Ok({
        use reqwest::Method;
        let (method, path) = match command {
            "plan" => (Method::POST, "/rasters/rgb/plan".into()),
            "run" => (Method::POST, "/rasters/rgb".into()),
            "inspect" => (Method::GET, format!("/jobs/{id}/rgb")),
            "pixel" => {
                let [x, y] = point.unwrap();
                (Method::GET, format!("/jobs/{id}/rgb/pixel?x={x}&y={y}"))
            }
            "package" => (Method::POST, format!("/jobs/{id}/package")),
            _ => return Err("Unknown scientific RGB command".into()),
        };
        let body = request.map(|r| json!(r));
        let response = backend.request(method, &path, body);
        let value = Box::pin(response).await?;
        if command == "run" {
            let id = value["id"].as_str().ok_or("No RGB job ID returned")?;
            let done = backend.wait(id).await?;
            if done.status != geod_runtime::JobStatus::Succeeded {
                return Err(done
                    .error
                    .unwrap_or("Scientific RGB processing failed".into()));
            }
            serde_json::to_value(done).map_err(|e| e.to_string())?
        } else {
            value
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_cli_dispatch_keeps_a_bounded_future_frame() {
        let backend = Backend::Server {
            base: "http://127.0.0.1:4318".into(),
            client: reqwest::Client::new(),
        };
        let options = BTreeMap::new();
        let future = run(&backend, "plan", "", &options);
        let bytes = std::mem::size_of_val(&future);
        eprintln!(
            "Scientific RGB CLI future: {bytes} bytes; Job: {} bytes",
            std::mem::size_of::<geod_runtime::Job>()
        );
        assert!(
            bytes < 128 * 1024,
            "Avoid unboxed nested native futures on the Windows entry stack: {bytes}"
        );
    }
}
