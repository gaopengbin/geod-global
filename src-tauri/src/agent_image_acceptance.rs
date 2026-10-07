use super::images;
use base64::{engine::general_purpose::STANDARD, Engine};
use image::ImageFormat;
use sha2::{Digest, Sha256};
use std::{
    io::Cursor,
    path::{Path, PathBuf},
};

#[test]
#[ignore = "writes an explicitly requested owned acceptance fixture"]
fn export_native_image_acceptance_fixture() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let directory = PathBuf::from(std::env::var("GEOD_AGENT_IMAGE_FIXTURE").unwrap());
    assert!(directory.is_absolute() && directory.starts_with(root.join(".verification")));
    assert!(!directory.exists());
    std::fs::create_dir_all(&directory).unwrap();
    let mut image = image::RgbImage::from_pixel(800, 400, image::Rgb([255, 255, 255]));
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        if (i64::from(x) - 200).pow(2) + (i64::from(y) - 200).pow(2) <= 10000 {
            *pixel = image::Rgb([240, 20, 20]);
        }
        if (500..=700).contains(&x) && (100..=300).contains(&y) {
            *pixel = image::Rgb([20, 50, 240]);
        }
    }
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image)
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    let bytes = bytes.into_inner();
    std::fs::write(directory.join("source.png"), &bytes).unwrap();
    let image = images::ingest(&directory, "visual-check.png", &STANDARD.encode(&bytes)).unwrap();
    assert_eq!(images::read(&directory, &image.id).unwrap().0, image);
    std::fs::write(directory.join("ingestion.json"),serde_json::to_vec_pretty(&serde_json::json!({
        "schema":"geod-native-agent-image-ingestion/v1","status":"passed","inputKind":"controlled-test-image",
        "sourceSha256":format!("{:x}",Sha256::digest(&bytes)),"sourceBytes":bytes.len(),"image":image,
        "expectation":{"left":{"shape":"circle","color":"red"},"right":{"shape":"square","color":"blue"}}
    })).unwrap()).unwrap();
}

#[tokio::test]
#[ignore = "owned runtime and controlled loopback provider; no real model or user credentials"]
async fn desktop_images_cross_private_ipc_and_resume_same_thread() {
    use super::{registry, DesktopAgent};
    use geod_runtime::JobManager;
    use serde_json::json;
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
        time::Duration,
    };
    #[derive(Default)]
    struct MarkerVault(Mutex<HashMap<String, String>>);
    impl registry::Vault for MarkerVault {
        fn read(&self, id: &str) -> Result<Option<zeroize::Zeroizing<String>>, String> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .get(id)
                .cloned()
                .map(zeroize::Zeroizing::new))
        }
        fn write(&self, id: &str, secret: &str) -> Result<(), String> {
            self.0.lock().unwrap().insert(id.into(), secret.into());
            Ok(())
        }
        fn delete(&self, id: &str) -> Result<(), String> {
            self.0.lock().unwrap().remove(id);
            Ok(())
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let output = PathBuf::from(std::env::var("GEOD_AGENT_IMAGE_NATIVE_OUTPUT").unwrap());
    let compact = std::env::var("GEOD_AGENT_CONTEXT_TEST").ok().as_deref() == Some("1");
    let responses = std::env::var("GEOD_AGENT_RESPONSES_TEST").ok().as_deref() == Some("1");
    let automatic = std::env::var("GEOD_AGENT_AUTOMATIC_CONTEXT_TEST")
        .ok()
        .as_deref()
        == Some("1");
    assert!(!automatic || compact);
    assert!(
        output.is_absolute()
            && output.starts_with(root.join(".verification"))
            && (!output.exists()
                || compact
                    && output.join("core/jobs.json").is_file()
                    && !output.join("agent").exists())
    );
    let source = PathBuf::from(std::env::var("GEOD_AGENT_IMAGE_FIXTURE").unwrap());
    assert!(source.starts_with(root.join(".verification")));
    let base = std::env::var("GEOD_AGENT_IMAGE_BASE_URL").unwrap();
    assert!(base.starts_with("http://127.0.0.1:"));
    let manager = JobManager::open(output.join("core")).await.unwrap();
    let vault = Arc::new(MarkerVault::default());
    let home = output.join("agent");
    let runtime = root.join(".agent-runtime/win32-x64");
    let mut agent = DesktopAgent::open_with_vault(
        home.clone(),
        runtime.clone(),
        manager.clone(),
        vault.clone(),
    )
    .await
    .unwrap();
    let protocol = if responses {
        "openai-responses"
    } else {
        "openai-compatible"
    };
    let model = if responses {
        "gpt-5"
    } else {
        "controlled-vision"
    };
    agent.save_model(serde_json::from_value(json!({"provider":"custom","label":"Owned image wire","protocol":protocol,"baseUrl":base,"model":model,"apiKey":"synthetic-owned-key"})).unwrap()).await.unwrap();
    let connected = agent.snapshot().await.unwrap();
    assert_eq!(connected["model"]["protocol"], protocol);
    assert_eq!(
        connected["model"]["capabilities"]["encryptedReasoning"],
        responses
    );
    let uploaded = agent
        .attach_image(
            "visual-check.png".into(),
            STANDARD.encode(std::fs::read(source.join("source.png")).unwrap()),
        )
        .await
        .unwrap();
    let id = uploaded["id"].as_str().unwrap().to_string();
    assert_eq!(
        agent.image_preview(id.clone()).await.unwrap()["image"],
        uploaded
    );
    let mut session = None;
    let mut original_thread = None;
    let mut snapshots = Vec::new();
    let mut review: Option<serde_json::Value> = None;
    for turn in 0..2 {
        let before = agent.snapshot().await.unwrap();
        let reads = before["selected"]["entries"]
            .as_array()
            .map(|entries| {
                entries
                    .iter()
                    .filter(|entry| entry["name"] == "geod_projects_list")
                    .count()
            })
            .unwrap_or(0);
        let first=agent.operation("send",json!({"sessionId":session,"text":"Read saved projects using the native tool. No writes.","images":if turn==0{vec![id.clone()]}else{vec![]}})).await.unwrap();
        session = Some(first["selected"]["id"].as_str().unwrap().to_string());
        let completed = tokio::time::timeout(Duration::from_secs(45), async {
            loop {
                let snapshot = agent.snapshot().await.unwrap();
                if !snapshot["busy"].as_bool().unwrap() {
                    break snapshot;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(completed["selected"]["status"], "completed");
        assert_eq!(
            completed["selected"]["entries"]
                .as_array()
                .unwrap()
                .iter()
                .filter(
                    |entry| entry["name"] == "geod_projects_list" && entry["status"] == "completed"
                )
                .count(),
            reads + if automatic { 2 } else { 1 }
        );
        if automatic {
            assert_eq!(
                completed["selected"]["contextState"]["count"],
                if turn == 0 { 1 } else { 3 }
            );
        }
        if let Some(pending) = &review {
            assert_eq!(completed["plans"][0]["planId"], pending["planId"]);
            assert_eq!(completed["plans"][0]["planHash"], pending["planHash"]);
            assert_eq!(completed["plans"][0]["status"], "pending");
            assert_eq!(manager.list().await.len(), 1);
        }
        assert_eq!(completed["selected"]["entries"][0]["images"][0]["id"], id);
        let history: serde_json::Value =
            serde_json::from_slice(&tokio::fs::read(home.join("sessions.json")).await.unwrap())
                .unwrap();
        let thread = history["sessions"][0]["threadId"].clone();
        if turn == 0 {
            original_thread = Some(thread);
        } else {
            assert_eq!(original_thread.as_ref(), Some(&thread));
        }
        snapshots.push(completed);
        agent.shutdown().await;
        if turn == 0 {
            if compact {
                let jobs = manager.list().await;
                assert_eq!(jobs.len(), 1);
                let source = &jobs[0];
                assert_eq!(
                    source.sha256.as_deref(),
                    Some("ede35bce788bbafd2c0dbda4bca8c0b56c30fbf1027b37db63d8c5ee92e8b1d8")
                );
                let raster = manager.inspect_raster(&source.id).await.unwrap();
                let bounds = [
                    raster.bounds[0],
                    raster.bounds[3] - 800.0,
                    raster.bounds[0] + 800.0,
                    raster.bounds[3],
                ];
                let pending=manager.agent_recipe_review_plan(session.as_ref().unwrap(),serde_json::from_value(json!({"schemaVersion":"geod-raster-recipe/v1","name":"Seeded native context review","source":{"jobId":source.id,"sha256":source.sha256},"operation":{"type":"clip","crs":"source","bounds":bounds},"output":{"format":"GeoTIFF"}})).unwrap()).await.unwrap();
                assert_eq!(pending["status"], "pending");
                review = Some(pending.clone());
                // The plan is genuinely preflighted; only its transcript link is
                // seeded. This is ledger preservation, not model plan generation.
                let path = home.join("sessions.json");
                let mut history: serde_json::Value =
                    serde_json::from_slice(&tokio::fs::read(&path).await.unwrap()).unwrap();
                history["sessions"][0]["entries"].as_array_mut().unwrap().push(json!({"id":"a1234567-1234-4234-8234-123456789abc","type":"tool","name":"geod_recipe_review_plan","status":"completed","references":[{"kind":"plan","id":pending["planId"],"label":"Seeded actual native review"}]}));
                tokio::fs::write(path, serde_json::to_vec(&history).unwrap())
                    .await
                    .unwrap();
            }
            agent = DesktopAgent::open_with_vault(
                home.clone(),
                runtime.clone(),
                manager.clone(),
                vault.clone(),
            )
            .await
            .unwrap();
            if compact {
                let before = agent.snapshot().await.unwrap();
                assert_eq!(before["plans"][0]["status"], "pending");
                agent
                    .operation("compact", json!({"sessionId":session}))
                    .await
                    .unwrap();
                let organized = tokio::time::timeout(Duration::from_secs(45), async {
                    loop {
                        let value = agent.snapshot().await.unwrap();
                        if !value["busy"].as_bool().unwrap() {
                            break value;
                        }
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                })
                .await
                .unwrap();
                assert_eq!(organized["selected"]["status"], "completed");
                assert_eq!(
                    organized["selected"]["contextState"]["count"],
                    if automatic { 2 } else { 1 }
                );
                assert_eq!(
                    organized["selected"]["entries"],
                    before["selected"]["entries"]
                );
                assert_eq!(
                    organized["plans"][0]["planId"],
                    review.as_ref().unwrap()["planId"]
                );
                assert_eq!(
                    organized["plans"][0]["planHash"],
                    review.as_ref().unwrap()["planHash"]
                );
                assert_eq!(organized["plans"][0]["status"], "pending");
                assert_eq!(manager.list().await.len(), 1);
                assert_eq!(
                    manager
                        .inspect_raster(&manager.list().await[0].id)
                        .await
                        .unwrap()
                        .sha256,
                    "ede35bce788bbafd2c0dbda4bca8c0b56c30fbf1027b37db63d8c5ee92e8b1d8"
                );
                agent.shutdown().await;
                agent = DesktopAgent::open_with_vault(
                    home.clone(),
                    runtime.clone(),
                    manager.clone(),
                    vault.clone(),
                )
                .await
                .unwrap();
                let reopened = agent.snapshot().await.unwrap();
                assert_eq!(
                    reopened["plans"][0]["planId"],
                    review.as_ref().unwrap()["planId"]
                );
                assert_eq!(reopened["plans"][0]["status"], "pending");
            }
        }
    }
    let raw = std::fs::read_to_string(home.join("sessions.json")).unwrap();
    assert!(!raw.contains("controlled-opaque-") && !raw.contains("controlled-incomplete-"));
    assert!(!raw.contains("data:image") && !raw.contains("synthetic-owned-key"));
    let receipt = json!({"schema":"geod-agent-desktop-image-acceptance/v1","status":"passed","model":"controlled-provider-not-live-generation","vault":"RAM marker fixture","nativeReadCalls":if automatic {4}else{2},"nativeToolDeclarations":super::definitions().len(),"restartedSameThread":true,"nativeImage":uploaded,"snapshots":snapshots,"contextOrganized":compact,"automaticContextOrganization":automatic,"review":review,"reviewTranscriptSeeded":compact,"usedUserDesktop":false});
    std::fs::write(
        output.join("native-acceptance.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
    manager.shutdown().await.unwrap();
}
