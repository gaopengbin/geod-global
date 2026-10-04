use super::{print_json, read_json, required, Backend};
use serde_json::{json, Value};
use std::collections::BTreeMap;
pub(super) async fn run(
    backend: &Backend,
    command: &str,
    id: &str,
    options: &BTreeMap<String, String>,
) -> Result<(), String> {
    let request = if matches!(command, "discover" | "save" | "open" | "resource") {
        Some(read_json::<Value>(required(options, "--request")?, 8192).await?)
    } else {
        None
    };
    if command == "open" {
        let Backend::Direct(m) = backend else {
            return Err("Open local 3D assets with --data-dir and an explicit --file path".into());
        };
        return print_json(
            &m.open_three_d_path(
                required(options, "--file")?.into(),
                serde_json::from_value(request.unwrap()).map_err(|e| e.to_string())?,
            )
            .await?,
        );
    }
    if command == "export" {
        let Backend::Direct(m) = backend else {
            return Err("Export 3D assets with --data-dir and an explicit --out path".into());
        };
        m.export_three_d_path(id, required(options, "--out")?.into())
            .await?;
        return print_json(&json!({"exported":true}));
    }
    let value = match backend {
        Backend::Direct(m) => match command {
            "list" => serde_json::to_value(m.list_three_d().await),
            "discover" => serde_json::to_value(
                m.discover_three_d(
                    serde_json::from_value(request.unwrap()).map_err(|e| e.to_string())?,
                )
                .await?,
            ),
            "save" => serde_json::to_value(
                m.acquire_three_d(
                    serde_json::from_value(request.unwrap()).map_err(|e| e.to_string())?,
                )
                .await?,
            ),
            "inspect" => serde_json::to_value(m.inspect_three_d(id).await?),
            "resource" => serde_json::to_value(
                m.read_three_d_resource(
                    serde_json::from_value(request.unwrap()).map_err(|e| e.to_string())?,
                )
                .await?,
            ),
            _ => return Err("Unknown 3D asset command".into()),
        }
        .map_err(|e| e.to_string())?,
        Backend::Server { .. } => {
            use reqwest::Method;
            let (method, path) = match command {
                "list" => (Method::GET, "/three-d/packages".into()),
                "discover" => (Method::POST, "/three-d/discover".into()),
                "save" => (Method::POST, "/three-d/packages".into()),
                "inspect" => (Method::GET, format!("/three-d/packages/{id}")),
                "resource" => {
                    let r: geod_runtime::three_d::ResourceRequest =
                        serde_json::from_value(request.clone().unwrap())
                            .map_err(|e| e.to_string())?;
                    (
                        Method::GET,
                        format!("/three-d/packages/{}/resources/{}", r.id, r.resource_id),
                    )
                }
                _ => return Err("Unknown 3D asset command".into()),
            };
            backend.request(method, &path, request).await?
        }
    };
    print_json(&value)
}
