//! Owned, detached acceptance driver for real Agent RGB-band acquisitions.
//! This is test orchestration, not another runtime API or model execution tool.
use geod_runtime::{agent_actions::SearchQuery, JobManager, JobStatus, ProxySettings};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;

fn save(root: &Path, receipt: &Value) -> geod_runtime::Result<()> {
    let temporary = root.join("native-acceptance.tmp");
    std::fs::write(&temporary, serde_json::to_vec_pretty(receipt).unwrap())
        .map_err(|e| e.to_string())?;
    std::fs::rename(temporary, root.join("native-acceptance.json")).map_err(|e| e.to_string())
}

async fn acquire(
    manager: &JobManager,
    root: &Path,
    receipt: &mut Value,
) -> geod_runtime::Result<()> {
    let session = Uuid::new_v4().to_string();
    receipt["sessionId"] = json!(session);
    for (provider, keys) in [
        (
            "planetary-landsat",
            ["red", "green", "blue", "qa_pixel", "qa_radsat"],
        ),
        (
            "planetary-modis",
            ["red", "green", "blue", "modis_qc", "modis_state"],
        ),
    ] {
        receipt["stage"] = json!(format!("search:{provider}"));
        save(root, receipt)?;
        let search = manager
            .agent_search(
                &session,
                SearchQuery {
                    provider: provider.into(),
                    bounds: [-122.46, 37.76, -122.45, 37.77],
                    start: "2025-06-01".into(),
                    end: "2025-06-30".into(),
                    cloud_max: 100.0,
                    limit: 1,
                },
            )
            .await?;
        let scene = search["scenes"][0]["itemId"]
            .as_str()
            .ok_or("No real scene returned.")?
            .to_owned();
        let project_plan = manager
            .agent_project_plan(
                &session,
                search["searchId"].as_str().unwrap(),
                vec![scene.clone()],
                Some(format!("Agent RGB original acceptance · {provider}")),
                None,
            )
            .await?;
        assert_eq!(project_plan["status"], "pending");
        let count = manager.list().await.len();
        let saved = manager
            .approve_agent_plan(
                &session,
                project_plan["planId"].as_str().unwrap(),
                project_plan["planHash"].as_str().unwrap(),
            )
            .await?;
        assert_eq!(manager.list().await.len(), count);
        let project_id = saved["project"]["id"].as_str().unwrap().to_owned();
        let index = receipt["providers"].as_array().unwrap().len();
        receipt["providers"]
            .as_array_mut()
            .unwrap()
            .push(json!({"provider":provider,"itemId":scene,
            "search":search,"projectPlan":project_plan,"project":saved,"files":[]}));
        save(root, receipt)?;
        // Preflight all same-observation bands before approving any transfer.
        for key in keys {
            let plan = manager
                .agent_project_download_plan(&session, &project_id, key, None)
                .await?;
            assert_eq!(plan["status"], "pending");
            assert_eq!(manager.list().await.len(), count);
            assert!(plan["expectedBytes"].as_u64().is_some_and(|n| n > 0));
            receipt["providers"][index]["files"]
                .as_array_mut()
                .unwrap()
                .push(json!({
                    "assetKey":key,"downloadPlan":plan,"preflightOnly":false,"fixture":false,
                }));
            save(root, receipt)?;
        }
        for file_index in 0..keys.len() {
            let plan = receipt["providers"][index]["files"][file_index]["downloadPlan"].clone();
            let approved = manager
                .approve_agent_plan(
                    &session,
                    plan["planId"].as_str().unwrap(),
                    plan["planHash"].as_str().unwrap(),
                )
                .await?;
            let id = approved["jobs"][0]["id"]
                .as_str()
                .ok_or("No native approved job.")?
                .to_owned();
            receipt["providers"][index]["files"][file_index]["jobId"] = json!(id);
            receipt["providers"][index]["files"][file_index]["approval"] = approved;
            save(root, receipt)?;
        }
    }
    receipt["stage"] = json!("waiting-for-native-settlement");
    save(root, receipt)?;
    // The process owns JobManager and survives observation-script exit. No test
    // deadline cancels a transfer or starts a replacement task.
    loop {
        let mut unfinished = false;
        for provider in receipt["providers"].as_array_mut().unwrap() {
            for file in provider["files"].as_array_mut().unwrap() {
                let job = manager
                    .get(file["jobId"].as_str().unwrap())
                    .await
                    .ok_or("Missing approved job")?;
                file["status"] = json!(job.status);
                file["bytesDownloaded"] = json!(job.bytes_downloaded);
                file["totalBytes"] = json!(job.total_bytes);
                unfinished |= matches!(job.status, JobStatus::Queued | JobStatus::Running);
            }
        }
        save(root, receipt)?;
        if !unfinished {
            break;
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
    receipt["stage"] = json!("inspecting-complete-originals");
    save(root, receipt)?;
    for provider_index in 0..receipt["providers"].as_array().unwrap().len() {
        for file_index in 0..5 {
            let file = receipt["providers"][provider_index]["files"][file_index].clone();
            let id = file["jobId"].as_str().unwrap();
            let job = manager.wait(id).await?;
            if job.status != JobStatus::Succeeded {
                return Err(format!(
                    "{}: {}",
                    file["assetKey"],
                    job.error.unwrap_or("Original transfer failed.".into())
                ));
            }
            assert_eq!(job.total_bytes, Some(job.bytes_downloaded));
            assert_eq!(
                file["downloadPlan"]["expectedBytes"],
                json!(job.bytes_downloaded)
            );
            assert!(
                job.sha256.is_some() && job.agent_approval.is_some() && !job.href.contains('?')
            );
            let raster = manager.inspect_raster(id).await?;
            assert_eq!(raster.sha256, job.sha256.clone().unwrap());
            let mut inspection = serde_json::to_value(&raster).unwrap();
            inspection.as_object_mut().unwrap().remove("previewDataUrl");
            let mut samples = Vec::new();
            let points = BTreeSet::from([
                (0, 0),
                (raster.width - 1, 0),
                (0, raster.height - 1),
                (raster.width - 1, raster.height - 1),
                (raster.width / 2, raster.height / 2),
                (raster.width / 4, raster.height / 4),
                (raster.width * 3 / 4, raster.height * 3 / 4),
            ]);
            for (column, row) in points {
                let x = raster.bounds[0] + (f64::from(column) + 0.5) * raster.pixel_size[0];
                let y = raster.bounds[3] - (f64::from(row) + 0.5) * raster.pixel_size[1];
                samples.push(manager.sample_raster(id, x, y).await?);
            }
            let plan = &file["downloadPlan"];
            let again = manager
                .approve_agent_plan(
                    &session,
                    plan["planId"].as_str().unwrap(),
                    plan["planHash"].as_str().unwrap(),
                )
                .await?;
            assert_eq!(again["jobs"][0]["id"], id);
            receipt["providers"][provider_index]["files"][file_index]["sha256"] = json!(job.sha256);
            receipt["providers"][provider_index]["files"][file_index]["raster"] = inspection;
            receipt["providers"][provider_index]["files"][file_index]["pixels"] = json!(samples);
            receipt["providers"][provider_index]["files"][file_index]["settled"] = json!(true);
            receipt["providers"][provider_index]["files"][file_index]["reconfirmedSameJob"] =
                json!(true);
            save(root, receipt)?;
        }
    }
    assert_eq!(manager.list().await.len(), 10);
    manager.shutdown().await?;
    receipt["stage"] = json!("reopening-for-idempotency");
    save(root, receipt)?;
    Ok(())
}

async fn run() {
    let args = std::env::args().collect::<Vec<_>>();
    let root = PathBuf::from(&args[1]);
    let mut receipt = json!({"schema":"geod-agent-rgb-original-acquisition/v1","status":"running",
        "scope":"Real native Agent search, project review, exact-hash confirmation, complete RGB and QA original transfers; not a model turn",
        "modelCalls":0,"fixture":false,"usedUserDesktop":false,"credentialsWritten":false,
        "pid":std::process::id(),"providers":[]});
    save(&root, &receipt).unwrap();
    let manager = JobManager::open(&root.join("core")).await.unwrap();
    assert!(
        manager.list().await.is_empty(),
        "Fresh isolated core required; never replace existing transfers."
    );
    manager
        .save_proxy_settings(ProxySettings {
            mode: geod_runtime::proxy::ProxyMode::Custom,
            url: Some(args[2].clone()),
        })
        .await
        .unwrap();
    let result = acquire(&manager, &root, &mut receipt).await;
    if let Err(error) = result {
        receipt["status"] = json!("failed");
        receipt["error"] = json!(error);
        save(&root, &receipt).unwrap();
        manager.shutdown().await.unwrap();
        std::process::exit(1);
    }
    drop(manager);
    let reopened = JobManager::open(&root.join("core")).await.unwrap();
    let session = receipt["sessionId"].as_str().unwrap();
    for provider in receipt["providers"].as_array().unwrap() {
        for file in provider["files"].as_array().unwrap() {
            let plan = &file["downloadPlan"];
            let result = reopened
                .approve_agent_plan(
                    session,
                    plan["planId"].as_str().unwrap(),
                    plan["planHash"].as_str().unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(result["jobs"][0]["id"], file["jobId"]);
        }
    }
    assert_eq!(reopened.list().await.len(), 10);
    reopened.shutdown().await.unwrap();
    receipt["status"] = json!("passed");
    receipt["stage"] = json!("complete");
    receipt["reopenedSameJobs"] = json!(true);
    save(&root, &receipt).unwrap();
    println!("Real Agent acquisition verified: 2 providers, 6 complete RGB bands and 4 matched QA originals.");
}

fn main() {
    // Windows executables use a smaller main-thread stack than the Rust test
    // harness. Keep this acceptance driver equivalent to the native harness;
    // do not change the production worker or its data/resource limits.
    std::thread::Builder::new()
        .name("agent-band-acceptance".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_stack_size(8 * 1024 * 1024)
                .build()
                .unwrap()
                .block_on(Box::pin(run()))
        })
        .unwrap()
        .join()
        .unwrap();
}
