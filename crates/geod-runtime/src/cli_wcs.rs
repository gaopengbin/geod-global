use super::{print_json, read_json, required, Backend, JobStatus};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub(super) async fn run(
    backend: &Backend,
    command: &str,
    id: &str,
    options: &BTreeMap<String, String>,
) -> Result<(), String> {
    if matches!(command, "forget" | "inspect" | "pixel")
        && uuid::Uuid::parse_str(id)
            .ok()
            .map(|v| v.to_string())
            .as_deref()
            != Some(id)
    {
        return Err("A canonical UUID is required for --id".into());
    }
    if matches!(command, "snapshot" | "description")
        && (id.len() != 64
            || !id
                .bytes()
                .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v)))
    {
        return Err("A saved coverage SHA-256 identifier is required for --id".into());
    }
    let request = if matches!(
        command,
        "connect" | "describe" | "plan" | "project" | "download"
    ) {
        Some(read_json::<Value>(required(options, "--request")?, 2 * 1024 * 1024).await?)
    } else {
        None
    };
    let pixel = if command == "pixel" {
        Some((
            required(options, "--column")?
                .parse::<u32>()
                .map_err(|_| "Invalid column")?,
            required(options, "--row")?
                .parse::<u32>()
                .map_err(|_| "Invalid row")?,
        ))
    } else {
        None
    };
    let mut value = match backend {
        Backend::Direct(m) => match command {
            "list" => serde_json::to_value(m.list_wcs_connections().await),
            "forget" => {
                m.forget_wcs_connection(id).await?;
                Ok(json!({"removed":true}))
            }
            "connect" => serde_json::to_value(
                m.connect_wcs(serde_json::from_value(request.unwrap()).map_err(|e| e.to_string())?)
                    .await?,
            ),
            "describe" => serde_json::to_value(
                m.describe_wcs(
                    serde_json::from_value(request.unwrap()).map_err(|e| e.to_string())?,
                )
                .await?,
            ),
            "plan" => serde_json::to_value(
                m.plan_wcs(serde_json::from_value(request.unwrap()).map_err(|e| e.to_string())?)
                    .await?,
            ),
            "description" => serde_json::to_value(m.wcs_description(id).await?),
            "snapshot" => serde_json::to_value(m.wcs_plan(id).await?),
            "project" => serde_json::to_value(
                m.save_wcs_project(
                    serde_json::from_value(request.unwrap()).map_err(|e| e.to_string())?,
                )
                .await?,
            ),
            "download" => serde_json::to_value(
                m.download_wcs_project(
                    serde_json::from_value(request.unwrap()).map_err(|e| e.to_string())?,
                )
                .await?,
            ),
            "inspect" => serde_json::to_value(m.inspect_wcs_asset(id).await?),
            "pixel" => {
                let (column, row) = pixel.unwrap();
                serde_json::to_value(m.sample_wcs_asset(id, column, row).await?)
            }
            _ => return Err("Unknown coverage operation".into()),
        }
        .map_err(|e| e.to_string())?,
        Backend::Server { .. } => {
            use reqwest::Method;
            let (method, path) = match command {
                "list" => (Method::GET, "/wcs/connections".into()),
                "connect" => (Method::POST, "/wcs/connections".into()),
                "forget" => (Method::POST, format!("/wcs/connections/{id}/forget")),
                "describe" => (Method::POST, "/wcs/describe".into()),
                "plan" => (Method::POST, "/wcs/plan".into()),
                "description" => (Method::GET, format!("/wcs/descriptions/{id}")),
                "snapshot" => (Method::GET, format!("/wcs/plans/{id}")),
                "project" => (Method::POST, "/wcs/project".into()),
                "download" => (Method::POST, "/wcs/downloads".into()),
                "inspect" => (Method::GET, format!("/wcs/jobs/{id}/inspect")),
                "pixel" => {
                    let (column, row) = pixel.unwrap();
                    (
                        Method::GET,
                        format!("/wcs/jobs/{id}/pixel?column={column}&row={row}"),
                    )
                }
                _ => return Err("Unknown coverage operation".into()),
            };
            backend.request(method, &path, request).await?
        }
    };
    if command == "download" {
        let ids = value["jobs"]
            .as_array()
            .ok_or("Invalid coverage queue response")?
            .iter()
            .map(|j| j["id"].as_str().map(str::to_owned).ok_or("Invalid job ID"))
            .collect::<Result<Vec<_>, _>>()?;
        let wait = async {
            let mut jobs = Vec::new();
            for id in &ids {
                jobs.push(backend.wait(id).await?);
            }
            Ok::<_, String>(jobs)
        };
        let completed = tokio::select! {
            result = wait => result?,
            _ = tokio::signal::ctrl_c() => {
                for id in &ids { backend.cancel(id).await?; }
                let mut jobs=Vec::new(); for id in &ids { jobs.push(backend.wait(id).await?); } jobs
            }
        };
        let succeeded = completed.iter().all(|j| j.status == JobStatus::Succeeded);
        value["jobs"] = serde_json::to_value(completed).map_err(|e| e.to_string())?;
        print_json(&value)?;
        if !succeeded {
            return Err("One or more coverage requests did not complete".into());
        }
        return Ok(());
    }
    print_json(&value)
}
