use super::{print_json, read_json, required, Backend};
use serde_json::{json, Value};
use std::collections::BTreeMap;
pub(super) async fn run(
    backend: &Backend,
    group: &str,
    command: &str,
    id: &str,
    options: &BTreeMap<String, String>,
) -> Result<(), String> {
    if command == "open" {
        let Backend::Direct(m) = backend else {
            return Err(
                "Open a local tile package with --data-dir and an explicit --file path".into(),
            );
        };
        return print_json(
            &m.open_tile_path(required(options, "--file")?.into())
                .await?,
        );
    }
    let request = if matches!(command, "connect" | "extract" | "tile") {
        Some(read_json::<Value>(required(options, "--request")?, 8192).await?)
    } else {
        None
    };
    if command == "export" {
        let Backend::Direct(m) = backend else {
            return Err("Export a tile package with --data-dir and an explicit --out path".into());
        };
        m.export_tile_package_path(id, required(options, "--out")?.into())
            .await?;
        return print_json(&json!({"exported":true}));
    }
    let value = match backend {
        Backend::Direct(m) => match (group, command) {
            ("tile-sources", "list") => serde_json::to_value(m.list_tile_sources().await),
            ("tile-sources", "connect") => serde_json::to_value(
                m.connect_tiles(
                    serde_json::from_value(request.unwrap()).map_err(|e| e.to_string())?,
                )
                .await?,
            ),
            ("tile-sources", "forget") => {
                m.forget_tile_source(id).await?;
                Ok(json!({"removed":true}))
            }
            ("tile-packages", "list") => serde_json::to_value(m.list_tile_packages().await),
            ("tile-packages", "extract") => serde_json::to_value(
                m.extract_tiles(
                    serde_json::from_value(request.unwrap()).map_err(|e| e.to_string())?,
                )
                .await?,
            ),
            ("tile-packages", "inspect") => serde_json::to_value(m.inspect_tile_package(id).await?),
            ("tile-packages", "tile") => serde_json::to_value(
                m.read_tile(serde_json::from_value(request.unwrap()).map_err(|e| e.to_string())?)
                    .await?,
            ),
            _ => return Err("Unknown PMTiles command".into()),
        }
        .map_err(|e| e.to_string())?,
        Backend::Server { .. } => {
            use reqwest::Method;
            let (method, path) = match (group, command) {
                ("tile-sources", "list") => (Method::GET, "/tile-sources".into()),
                ("tile-sources", "connect") => (Method::POST, "/tile-sources".into()),
                ("tile-sources", "forget") => (Method::POST, format!("/tile-sources/{id}/forget")),
                ("tile-packages", "list") => (Method::GET, "/tile-packages".into()),
                ("tile-packages", "extract") => (Method::POST, "/tile-packages".into()),
                ("tile-packages", "inspect") => (Method::GET, format!("/tile-packages/{id}")),
                ("tile-packages", "tile") => {
                    let r: geod_runtime::tiles::TileRequest =
                        serde_json::from_value(request.clone().unwrap())
                            .map_err(|e| e.to_string())?;
                    (
                        Method::GET,
                        format!("/tile-packages/{}/tiles/{}/{}/{}", r.id, r.z, r.x, r.y),
                    )
                }
                _ => return Err("Unknown PMTiles command".into()),
            };
            backend.request(method, &path, request).await?
        }
    };
    print_json(&value)
}
