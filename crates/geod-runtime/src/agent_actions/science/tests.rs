use super::*;

// Synthetic unit TIFFs prove sample/approval invariants, not remote availability.
async fn bands(manager: &JobManager) -> ([Job; 3], [Job; 2], [[i32; 6]; 3]) {
    let values = [
        [0, 100, 1000, 20000, 65535, 40],
        [20, 101, 2000, 0, 65535, 50],
        [30, 102, 3000, 10000, 65535, 60],
    ];
    let mut jobs = Vec::new();
    for (c, key) in ["red", "green", "blue", "qa_pixel", "qa_radsat"]
        .iter()
        .enumerate()
    {
        let samples = if c < 3 {
            values[c]
        } else if c == 3 {
            [0x5540, 0x5540, 0x5548, 0x5540, 0x5540, 0x5560]
        } else {
            [0, 0, 0, 0, 4, 0]
        };
        let mut job = crate::raster::reflectance::tests::record(
            &manager.inner.root,
            false,
            &crate::raster::reflectance::tests::fixture_samples(
                false,
                if c == 3 { "1" } else { "0" },
                30.0,
                1,
                &samples,
            ),
        );
        job.asset_key = (*key).into();
        job.href = job.href.replace(
            "_SR_B4.TIF",
            [
                "_SR_B4.TIF",
                "_SR_B3.TIF",
                "_SR_B2.TIF",
                "_QA_PIXEL.TIF",
                "_QA_RADSAT.TIF",
            ][c],
        );
        jobs.push(job);
    }
    let mut store = manager.inner.store.lock().await;
    for job in &jobs {
        store.jobs.insert(job.id.clone(), job.clone());
    }
    manager.persist(&store.jobs).await.unwrap();
    (
        jobs[..3].to_vec().try_into().unwrap(),
        jobs[3..].to_vec().try_into().unwrap(),
        values,
    )
}
fn request(rgb: &[Job; 3], qa: &[Job; 2], masked: bool) -> crate::RgbRequest {
    crate::RgbRequest {
        job_ids: rgb.each_ref().map(|j| j.id.clone()),
        project_id: None,
        name: Some("Unit reviewed RGB".into()),
        quality_mask: masked.then(|| {
            crate::LandsatMaskRequest {
                qa_pixel_job_id: qa[0].id.clone(),
                qa_radsat_job_id: qa[1].id.clone(),
                policy: crate::LandsatMaskPolicy::CloudFreeConservative,
                exclude_snow: true,
            }
            .into()
        }),
    }
}
#[tokio::test]
async fn confirmed_rgb_is_atomic_pinned_sample_exact_and_idempotent_after_restart() {
    for masked in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let manager = JobManager::open(&root).await.unwrap();
        let (rgb, qa, values) = bands(&manager).await;
        let session = Uuid::new_v4().to_string();
        let plan = manager
            .agent_scientific_rgb_plan(&session, request(&rgb, &qa, masked))
            .await
            .unwrap();
        assert_eq!(plan["status"], "pending");
        assert_eq!(plan["kind"], "rgb");
        assert_eq!(plan["processing"]["rawBytes"], 36);
        assert_eq!(plan["processing"]["quality"].is_object(), masked);
        assert_eq!(
            plan["notes"][0],
            if masked {
                "Accepted original RGB samples and calibration are retained. Screening replaces rejected pixels with NoData; no reprojection or resampling."
            } else {
                "Original RGB values and calibration are retained; no quality screening or resampling."
            }
        );
        assert_eq!(manager.list().await.len(), 5);
        let id = plan["planId"].as_str().unwrap();
        let hash = plan["planHash"].as_str().unwrap();
        assert!(manager
            .approve_agent_plan(&session, id, &"f".repeat(64))
            .await
            .is_err());
        assert!(call(
            manager.clone(),
            &session,
            "geod_rgb_run",
            json!({"request":request(&rgb,&qa,masked)}),
            None
        )
        .await
        .is_err());
        let (a, b) = tokio::join!(
            manager.approve_agent_plan(&session, id, hash),
            manager.approve_agent_plan(&session, id, hash)
        );
        let a = a.unwrap();
        let b = b.unwrap();
        assert_eq!(a["jobs"][0]["id"], b["jobs"][0]["id"]);
        let job_id = a["jobs"][0]["id"].as_str().unwrap();
        let done = manager.wait(job_id).await.unwrap();
        assert_eq!(done.status, JobStatus::Succeeded, "{:?}", done.error);
        assert_eq!(manager.list().await.len(), 6);
        assert_eq!(done.agent_approval.as_ref().unwrap().plan_hash, hash);
        for (c, band) in values.iter().enumerate() {
            let samples = band
                .iter()
                .enumerate()
                .flat_map(|(i, v)| {
                    if masked && [2, 4, 5].contains(&i) {
                        0u16
                    } else {
                        *v as u16
                    }
                    .to_le_bytes()
                })
                .collect::<Vec<_>>();
            assert_eq!(
                done.rgb_output.as_ref().unwrap().samples_sha256[c],
                digest(&samples)
            );
        }
        let actual = call(
            manager.clone(),
            &session,
            "geod_rgb_inspect",
            json!({"id":job_id}),
            None,
        )
        .await
        .unwrap();
        assert_eq!(actual["artifact"]["sha256"], done.sha256.clone().unwrap());
        assert_eq!(actual["artifact"]["jobId"], job_id);
        assert!(!actual.to_string().contains("https://"));
        manager.shutdown().await.unwrap();
        drop(manager);
        let reopened = JobManager::open(&root).await.unwrap();
        assert_eq!(
            reopened
                .approve_agent_plan(&session, id, hash)
                .await
                .unwrap()["jobs"][0]["id"],
            job_id
        );
        assert_eq!(reopened.list().await.len(), 6);
        reopened.shutdown().await.unwrap();
    }
}
#[tokio::test]
async fn rgb_review_rejects_changed_sources_and_policy_changes_the_hash() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let (rgb, qa, _) = bands(&manager).await;
    let session = Uuid::new_v4().to_string();
    let masked = manager
        .agent_scientific_rgb_plan(&session, request(&rgb, &qa, true))
        .await
        .unwrap();
    let unmasked = manager
        .agent_scientific_rgb_plan(&session, request(&rgb, &qa, false))
        .await
        .unwrap();
    assert_ne!(masked["planHash"], unmasked["planHash"]);
    std::fs::write(qa[0].output_path.as_ref().unwrap(), b"changed quality file").unwrap();
    assert!(manager
        .approve_agent_plan(
            &session,
            masked["planId"].as_str().unwrap(),
            masked["planHash"].as_str().unwrap()
        )
        .await
        .is_err());
    assert_eq!(manager.list().await.len(), 5);
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn rgb_form_rechecks_pinned_bands_and_matched_quality_without_creating_a_job() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let (rgb, qa, _) = bands(&manager).await;
    let session = Uuid::new_v4().to_string();
    let original = manager
        .agent_scientific_rgb_plan(&session, request(&rgb, &qa, true))
        .await
        .unwrap();
    let id = original["planId"].as_str().unwrap();
    let hash = original["planHash"].as_str().unwrap();
    let draft = manager
        .agent_plan_revision_draft(&session, id, hash)
        .await
        .unwrap();
    assert_eq!(
        draft["parameters"]["qualityPolicy"],
        "cloud_free_conservative"
    );
    let revised = manager
        .revise_agent_plan(
            &session,
            id,
            hash,
            PlanRevision::Rgb {
                name: "Revised RGB".into(),
                quality_policy: Some("cloud_free".into()),
                exclude_snow: Some(false),
            },
        )
        .await
        .unwrap();
    assert_ne!(revised["planHash"], original["planHash"]);
    assert_eq!(revised["processing"]["quality"]["policy"], "cloud_free");
    assert_eq!(revised["processing"]["quality"]["excludeSnow"], false);
    assert_eq!(manager.list().await.len(), 5);
    let id = revised["planId"].as_str().unwrap();
    let hash = revised["planHash"].as_str().unwrap();
    assert!(manager
        .revise_agent_plan(
            &session,
            id,
            hash,
            PlanRevision::Rgb {
                name: "Wrong policy".into(),
                quality_policy: Some("clear".into()),
                exclude_snow: Some(false)
            }
        )
        .await
        .is_err());
    assert_eq!(
        manager.agent_plan_status(&session, id).await.unwrap()["status"],
        "pending"
    );
    let submitted = manager
        .approve_agent_plan(&session, id, hash)
        .await
        .unwrap();
    let done = manager
        .wait(submitted["jobs"][0]["id"].as_str().unwrap())
        .await
        .unwrap();
    assert_eq!(done.status, JobStatus::Succeeded);
    assert_eq!(done.title, "Revised RGB");
    manager.shutdown().await.unwrap();
}
