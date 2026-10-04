use super::*;
use crate::raster::reflectance::tests::{fixture_samples, record};

fn originals(root: &Path, signed: bool, values: [[i32; 6]; 3]) -> [Job; 3] {
    std::array::from_fn(|c| {
        let mut job = record(
            root,
            signed,
            &fixture_samples(
                signed,
                if signed { "-9999" } else { "0" },
                30.0,
                1,
                &values[c],
            ),
        );
        job.asset_key = ["red", "green", "blue"][c].into();
        job.href = if signed {
            job.href
                .replace(".B04.tif", [".B04.tif", ".B03.tif", ".B02.tif"][c])
        } else {
            job.href
                .replace("_SR_B4.TIF", ["_SR_B4.TIF", "_SR_B3.TIF", "_SR_B2.TIF"][c])
        };
        job
    })
}

#[tokio::test]
async fn persisted_unsigned_and_signed_rgb_preserve_each_sample_restart_and_bundle_without_parents()
{
    for signed in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let manager = JobManager::open(&root).await.unwrap();
        let fill = if signed { -9999 } else { 0 };
        let high = if signed { 32767 } else { 65535 };
        let low = if signed { -100 } else { 100 };
        let values = [
            [fill, low, 1000, 20000, high, 40],
            [20, low + 1, 2000, fill, high, 50],
            [30, low + 2, 3000, 10000, high, 60],
        ];
        let jobs = originals(&root, signed, values);
        {
            let mut store = manager.inner.store.lock().await;
            for j in &jobs {
                store.jobs.insert(j.id.clone(), j.clone());
            }
            manager.persist(&store.jobs).await.unwrap();
        }
        let request = RgbRequest {
            job_ids: jobs.clone().map(|j| j.id),
            project_id: None,
            name: Some("科学 RGB 测试".into()),
            quality_mask: None,
        };
        let plan = manager.plan_scientific_rgb(request.clone()).await.unwrap();
        assert_eq!(plan.raw_bytes, 36);
        let queued = manager.run_scientific_rgb(request).await.unwrap();
        let done = manager.wait(&queued.id).await.unwrap();
        assert_eq!(done.status, JobStatus::Succeeded, "{:?}", done.error);
        assert_eq!(
            done.rgb_output.as_ref().unwrap().channel_valid_pixels,
            [5, 5, 6]
        );
        assert_eq!(done.rgb_output.as_ref().unwrap().common_valid_pixels, 4);
        for (c, band) in values.iter().enumerate() {
            let raw = band
                .iter()
                .flat_map(|v| (*v as u16).to_le_bytes())
                .collect::<Vec<_>>();
            assert_eq!(
                done.rgb_output.as_ref().unwrap().samples_sha256[c],
                format!("{:x}", Sha256::digest(raw))
            );
        }
        // Parents are no longer present: the completed RGB's own samples remain usable.
        {
            let mut store = manager.inner.store.lock().await;
            for j in &jobs {
                store.jobs.remove(&j.id);
                std::fs::remove_file(j.output_path.as_ref().unwrap()).unwrap();
            }
            manager.persist(&store.jobs).await.unwrap();
        }
        drop(manager);
        let manager = JobManager::open(&root).await.unwrap();
        let metadata = manager.inspect_scientific_rgb(&done.id).await.unwrap();
        assert_eq!(
            metadata.artifact.unwrap().sha256,
            done.sha256.clone().unwrap()
        );
        assert_eq!(metadata.data_type, if signed { "Int16" } else { "UInt16" });
        for i in 0..6 {
            let point = manager
                .sample_scientific_rgb(
                    &done.id,
                    500015.0 + (i % 3) as f64 * 30.0,
                    4199985.0 - (i / 3) as f64 * 30.0,
                )
                .await
                .unwrap();
            assert_eq!(point.values, values.map(|v| v[i]));
        }
        let first = manager.prepare_artifact(&done.id).await.unwrap();
        let repeated = manager.prepare_artifact(&done.id).await.unwrap();
        assert_eq!(first.sha256, repeated.sha256);
        let (_, bytes) = manager.artifact_bytes(&done.id).await.unwrap();
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert_eq!(zip.len(), 5);
        let mut tiff = Vec::new();
        zip.by_name(&format!("{}.tif", done.id))
            .unwrap()
            .read_to_end(&mut tiff)
            .unwrap();
        assert_eq!(format!("{:x}", Sha256::digest(tiff)), done.sha256.unwrap());
        let mut decoder = Decoder::new(File::open(done.output_path.unwrap()).unwrap()).unwrap();
        assert_eq!(
            decoder.get_tag_u16_vec(Tag::SampleFormat).unwrap(),
            vec![if signed { 2 } else { 1 }; 3]
        );
        assert!(decoder
            .get_tag_ascii_string(Tag::Unknown(42112))
            .unwrap()
            .contains("role=\"scale\""));
    }
}

#[tokio::test]
async fn mismatched_grid_source_checksum_cancellation_and_tampered_outputs_fail_without_commits() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let manager = JobManager::open(&root).await.unwrap();
    let jobs = originals(&root, false, [[100; 6]; 3]);
    {
        let mut store = manager.inner.store.lock().await;
        for j in &jobs {
            store.jobs.insert(j.id.clone(), j.clone());
        }
    }
    let request = RgbRequest {
        job_ids: jobs.clone().map(|j| j.id),
        project_id: None,
        name: None,
        quality_mask: None,
    };
    let plan = manager.plan_scientific_rgb(request.clone()).await.unwrap();
    let token = CancellationToken::new();
    token.cancel();
    let id = uuid::Uuid::new_v4().to_string();
    let (tx, _) = tokio::sync::mpsc::unbounded_channel();
    assert!(encode::write(&root, &id, &plan.spec, &jobs, None, &token, &tx).is_err());
    assert!(!root.join("assets").join(format!("{id}.tif")).exists());
    let mut invalid = plan.spec.clone();
    invalid.grid.pixel_size[0] = 15.0;
    assert!(validate_spec(&invalid).is_err());
    let queued = manager.run_scientific_rgb(request.clone()).await.unwrap();
    let done = manager.wait(&queued.id).await.unwrap();
    assert_eq!(done.status, JobStatus::Succeeded);
    std::fs::write(done.output_path.as_ref().unwrap(), b"corrupt").unwrap();
    assert!(manager.inspect_scientific_rgb(&done.id).await.is_err());
    assert!(manager.prepare_artifact(&done.id).await.is_err());
    std::fs::write(jobs[1].output_path.as_ref().unwrap(), b"corrupt").unwrap();
    assert!(manager.plan_scientific_rgb(request).await.is_err());
}

#[tokio::test]
async fn crashed_rgb_is_interrupted_cleans_staging_and_retries_the_pinned_operation() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let manager = JobManager::open(&root).await.unwrap();
    let jobs = originals(&root, false, [[100; 6]; 3]);
    {
        let mut store = manager.inner.store.lock().await;
        for job in &jobs {
            store.jobs.insert(job.id.clone(), job.clone());
        }
    }
    let plan = manager
        .plan_scientific_rgb(RgbRequest {
            job_ids: jobs.map(|j| j.id),
            project_id: None,
            name: Some("Crash recovery".into()),
            quality_mask: None,
        })
        .await
        .unwrap();
    let mut record = crate::new_download_job(crate::CreateJobRequest {
        item_id: plan.spec.sources[0].item_id.clone(),
        asset_key: "reflectance_rgb".into(),
        href: plan.spec.sources[0].href.clone(),
        media_type: "image/tiff".into(),
        title: Some(plan.spec.name.clone()),
    });
    record.kind = "raster_rgb".into();
    record.rgb_spec = Some(Box::new(plan.spec));
    record.status = JobStatus::Running;
    let stage = root
        .join("assets")
        .join(format!("{}.rgb-crash.part", record.id));
    std::fs::write(&stage, b"partial").unwrap();
    std::fs::write(
        root.join("assets").join(format!("{}.tif", record.id)),
        b"uncommitted output",
    )
    .unwrap();
    {
        let mut store = manager.inner.store.lock().await;
        store.jobs.insert(record.id.clone(), record.clone());
        manager.persist(&store.jobs).await.unwrap();
    }
    drop(manager);
    let manager = JobManager::open(&root).await.unwrap();
    let interrupted = manager.get(&record.id).await.unwrap();
    assert_eq!(interrupted.status, JobStatus::Interrupted);
    assert!(interrupted.rgb_output.is_none());
    assert_eq!(interrupted.rgb_spec, record.rgb_spec);
    assert!(!stage.exists());
    let queued = manager.retry(&record.id).await.unwrap();
    assert_eq!(queued.id, record.id);
    assert_eq!(queued.attempts, 2);
    let done = manager.wait(&record.id).await.unwrap();
    assert_eq!(done.status, JobStatus::Succeeded, "{:?}", done.error);
}
