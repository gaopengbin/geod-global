use super::*;
use crate::raster::reflectance::tests::fixture_sized;

const ITEM: &str = "LC09_L2SP_044034_20250628_02_T1";
const PRODUCT: &str = "LC09_L2SP_044034_20250628_20250629_02_T1";
const CLEAR: u32 = 0x5540;
fn flags() -> ([u32; 64], [u32; 64]) {
    let mut pixel = [CLEAR; 64];
    let mut radsat = [0; 64];
    for bit in 0..5 {
        pixel[bit + 1] |= 1 << bit;
    }
    pixel[6] |= 1 << 5;
    pixel[7] |= 1 << 7;
    pixel[8] &= !(3 << 8);
    pixel[9] = (pixel[9] & !(3 << 8)) | (2 << 8);
    pixel[10] |= 3 << 10;
    pixel[11] = (pixel[11] & !(3 << 14)) | (2 << 14);
    pixel[12] &= !(1 << 6);
    radsat[13] = 1;
    radsat[14] = 1 << 1;
    radsat[15] = 1 << 2;
    radsat[16] = 1 << 3;
    radsat[17] = 1 << 11;
    radsat[18] = 1 << 8; // B9 is a non-RGB band, not an unused bit.
    radsat[19] = 1 << 7;
    pixel[22] = u16::MAX as u32;
    pixel[23] &= !(3 << 12); // Snow confidence unset, optional snow rule.
    (pixel, radsat)
}
fn fixtures(root: &Path, point: bool) -> ([Job; 3], [Job; 2], [[i32; 64]; 3]) {
    let mut values = std::array::from_fn::<_, 3, _>(|c| {
        std::array::from_fn(|i| 10000 + c as i32 * 1000 + i as i32)
    });
    for band in &mut values {
        band[0] = 65535;
    }
    values[0][20] = 0;
    values[1][21] = 0;
    let (pixel, radsat) = flags();
    let jobs: Vec<_> = ["red", "green", "blue", "qa_pixel", "qa_radsat"].iter().enumerate().map(|(c, key)| {
        let samples = if c < 3 { values[c].to_vec() } else { if c == 3 { pixel } else { radsat }.map(|v| v as i32).to_vec() };
        let bytes = fixture_sized(false, if c == 3 { "1" } else { "0" }, 30.0, if point { 2 } else { 1 }, &samples, 32, 2);
        let mut job = crate::raster::tests::record(root, &bytes);
        job.asset_key = (*key).into(); job.item_id = ITEM.into();
        let suffix = ["SR_B4", "SR_B3", "SR_B2", "QA_PIXEL", "QA_RADSAT"][c];
        job.href = format!("https://{}/landsat-c2/level-2/standard/oli-tirs/2025/044/034/{PRODUCT}/{PRODUCT}_{suffix}.TIF", providers::LANDSAT_HOST);
        job
    }).collect();
    (
        jobs[..3].to_vec().try_into().unwrap(),
        jobs[3..].to_vec().try_into().unwrap(),
        values,
    )
}
fn request(rgb: &[Job; 3], qa: &[Job; 2], policy: LandsatMaskPolicy, snow: bool) -> RgbRequest {
    RgbRequest {
        job_ids: rgb.clone().map(|j| j.id),
        project_id: None,
        name: Some(format!("Landsat {policy:?} snow {snow}")),
        quality_mask: Some(
            LandsatMaskRequest {
                qa_pixel_job_id: qa[0].id.clone(),
                qa_radsat_job_id: qa[1].id.clone(),
                policy,
                exclude_snow: snow,
            }
            .into(),
        ),
    }
}

#[test]
fn landsat_rules_preserve_water_and_non_rgb_saturation_and_reject_unknown_conservative_flags() {
    use landsat_mask::accepted;
    use LandsatMaskPolicy::*;
    for policy in [CloudFree, CloudFreeConservative] {
        assert!(accepted(policy, false, CLEAR, 0));
        assert!(accepted(policy, true, CLEAR | 128, 0));
        for bit in [0, 4, 5, 6, 8] {
            assert!(accepted(policy, false, CLEAR, 1 << bit));
        }
        for bit in [1, 2, 3, 11] {
            assert!(!accepted(policy, false, CLEAR, 1 << bit));
        }
        for bit in 0..5 {
            assert!(!accepted(policy, false, CLEAR | (1 << bit), 0));
        }
        assert!(accepted(policy, false, CLEAR | (1 << 5), 0));
        assert!(!accepted(policy, true, CLEAR | (1 << 5), 0));
        assert!(!accepted(policy, false, u32::MAX, 0));
        assert!(!accepted(policy, false, CLEAR, u32::MAX));
    }
    for shift in [8, 10, 14] {
        for code in [0, 2, 3] {
            let value = (CLEAR & !(3 << shift)) | (code << shift);
            assert!(accepted(CloudFree, false, value, 0));
            assert!(!accepted(CloudFreeConservative, false, value, 0));
        }
    }
    for bit in [7, 9, 10, 12, 13, 14, 15] {
        assert!(accepted(CloudFree, false, CLEAR, 1 << bit));
        assert!(!accepted(CloudFreeConservative, false, CLEAR, 1 << bit));
    }
    assert!(!accepted(CloudFreeConservative, false, CLEAR & !64, 0));
    assert!(accepted(
        CloudFreeConservative,
        false,
        CLEAR & !(3 << 12),
        0
    ));
    assert!(!accepted(
        CloudFreeConservative,
        true,
        CLEAR & !(3 << 12),
        0
    ));
}

#[tokio::test]
async fn landsat_mask_retains_full_unsigned_dn_point_centres_and_standalone_restart_package() {
    for point in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let manager = JobManager::open(&root).await.unwrap();
        let (rgb, qa, values) = fixtures(&root, point);
        {
            let mut store = manager.inner.store.lock().await;
            for job in rgb.iter().chain(&qa) {
                store.jobs.insert(job.id.clone(), job.clone());
            }
            manager.persist(&store.jobs).await.unwrap();
        }
        let mut outputs = Vec::new();
        for policy in [
            LandsatMaskPolicy::CloudFree,
            LandsatMaskPolicy::CloudFreeConservative,
        ] {
            for snow in [false, true] {
                // Independently listed expectations, not a second call to the predicate.
                let mut rejected = vec![1, 2, 3, 4, 5, 14, 15, 16, 17, 22];
                if snow {
                    rejected.push(6);
                }
                if policy == LandsatMaskPolicy::CloudFreeConservative {
                    rejected.extend([8, 9, 10, 11, 12, 19]);
                    if snow {
                        rejected.push(23);
                    }
                }
                let r = request(&rgb, &qa, policy, snow);
                let plan = manager.plan_scientific_rgb(r.clone()).await.unwrap();
                assert_eq!(
                    plan.spec.quality_mask.as_ref().unwrap().schema_version(),
                    landsat_mask::SCHEMA
                );
                assert_eq!(
                    plan.spec.grid.pixel_interpretation,
                    if point { "PixelIsPoint" } else { "PixelIsArea" }
                );
                let done = manager
                    .wait(&manager.run_scientific_rgb(r).await.unwrap().id)
                    .await
                    .unwrap();
                assert_eq!(done.status, JobStatus::Succeeded, "{:?}", done.error);
                let output = done.rgb_output.as_ref().unwrap();
                let count = output.quality_mask.as_ref().unwrap();
                assert_eq!(count.examined_pixels, 64);
                assert_eq!(count.rejected_pixels, rejected.len() as u64);
                assert_eq!(count.input_common_valid_pixels, 62);
                assert_eq!(count.removed_valid_pixels, rejected.len() as u64);
                assert_eq!(output.common_valid_pixels, 62 - rejected.len() as u64);
                for (channel, band) in values.iter().enumerate() {
                    let expected: Vec<_> = band
                        .iter()
                        .enumerate()
                        .flat_map(|(index, v)| {
                            if rejected.contains(&index) {
                                0u16
                            } else {
                                *v as u16
                            }
                            .to_le_bytes()
                        })
                        .collect();
                    assert_eq!(
                        output.samples_sha256[channel],
                        format!("{:x}", Sha256::digest(expected))
                    );
                }
                let g = &done.rgb_spec.as_ref().unwrap().grid;
                let pixel = manager
                    .sample_scientific_rgb(&done.id, g.bounds[0] + 15., g.bounds[3] - 15.)
                    .await
                    .unwrap();
                assert_eq!(pixel.values, [65535; 3]);
                assert_eq!(pixel.channel_no_data, [false; 3]);
                assert_eq!(pixel.reflectances, [Some(65535. * 0.0000275 - 0.2); 3]);
                outputs.push(done);
            }
        }
        {
            let mut store = manager.inner.store.lock().await;
            for job in rgb.iter().chain(&qa) {
                store.jobs.remove(&job.id);
                std::fs::remove_file(job.output_path.as_ref().unwrap()).unwrap();
            }
            manager.persist(&store.jobs).await.unwrap();
        }
        drop(manager);
        let manager = JobManager::open(&root).await.unwrap();
        for done in outputs {
            let info = manager.inspect_scientific_rgb(&done.id).await.unwrap();
            assert_eq!(info.artifact.unwrap().sha256, done.sha256.clone().unwrap());
            let bundle = manager.prepare_artifact(&done.id).await.unwrap();
            assert_eq!(
                manager.prepare_artifact(&done.id).await.unwrap().sha256,
                bundle.sha256
            );
            let mut zip = zip::ZipArchive::new(File::open(bundle.path).unwrap()).unwrap();
            let mut metadata = String::new();
            zip.by_name(&format!("{}.metadata.json", done.id))
                .unwrap()
                .read_to_string(&mut metadata)
                .unwrap();
            let v: Value = serde_json::from_str(&metadata).unwrap();
            assert_eq!(
                v["spec"]["qualityMask"]["sources"][0]["sha256"],
                qa[0].sha256.clone().unwrap()
            );
            std::fs::write(done.output_path.as_ref().unwrap(), b"corrupt").unwrap();
            assert!(manager.inspect_scientific_rgb(&done.id).await.is_err());
            assert!(manager.prepare_artifact(&done.id).await.is_err());
        }
    }
}

#[tokio::test]
async fn landsat_mask_rejects_missing_mismatched_duplicate_and_changed_quality_before_queueing() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let manager = JobManager::open(&root).await.unwrap();
    let (rgb, qa, _) = fixtures(&root, true);
    {
        let mut store = manager.inner.store.lock().await;
        for job in rgb.iter().chain(&qa) {
            store.jobs.insert(job.id.clone(), job.clone());
        }
    }
    let r = request(&rgb, &qa, LandsatMaskPolicy::CloudFree, false);
    let spec = manager.plan_scientific_rgb(r.clone()).await.unwrap().spec;
    let mut wrong = spec.clone();
    wrong.quality_mask.as_mut().unwrap().sources_mut()[0].href =
        qa[0].href.replace("20250629", "20250630");
    assert!(validate_spec(&wrong).is_err());
    wrong = spec.clone();
    wrong.quality_mask.as_mut().unwrap().sources_mut()[0]
        .pin
        .job_id = rgb[0].id.clone();
    assert!(validate_spec(&wrong).is_err());
    let mut missing = r.clone();
    if let Some(QualityMaskRequest::Landsat(m)) = &mut missing.quality_mask {
        m.qa_pixel_job_id = uuid::Uuid::new_v4().to_string();
    }
    assert!(manager.run_scientific_rgb(missing).await.is_err());
    let cancel = CancellationToken::new();
    cancel.cancel();
    let id = uuid::Uuid::new_v4().to_string();
    let (tx, _) = tokio::sync::mpsc::unbounded_channel();
    assert!(encode::write(&root, &id, &spec, &rgb, Some(&qa), &cancel, &tx).is_err());
    assert!(!root.join("assets").join(format!("{id}.tif")).exists());
    // Complete matching source identity but a different Point/Area grid fails.
    let bytes = fixture_sized(false, "1", 30., 1, &[0x5540; 64], 32, 2);
    let mut changed = qa[0].clone();
    std::fs::write(changed.output_path.as_ref().unwrap(), &bytes).unwrap();
    changed.sha256 = Some(format!("{:x}", Sha256::digest(&bytes)));
    changed.bytes_downloaded = bytes.len() as u64;
    {
        manager
            .inner
            .store
            .lock()
            .await
            .jobs
            .insert(changed.id.clone(), changed);
    }
    assert!(manager
        .run_scientific_rgb(r)
        .await
        .unwrap_err()
        .contains("grid differs"));
    assert_eq!(manager.inner.store.lock().await.jobs.len(), 5);
}

#[test]
fn mask_json_keeps_modis_compatible_and_rejects_cross_product_policy_or_mixed_fields() {
    let a = uuid::Uuid::new_v4().to_string();
    let b = uuid::Uuid::new_v4().to_string();
    let modis = json!({"qcJobId":a,"stateJobId":b,"policy":"clear"});
    let landsat = json!({"qaPixelJobId":a,"qaRadsatJobId":b,"policy":"cloud_free"});
    assert!(matches!(
        serde_json::from_value::<QualityMaskRequest>(modis.clone()).unwrap(),
        QualityMaskRequest::Modis(_)
    ));
    assert!(matches!(
        serde_json::from_value::<QualityMaskRequest>(landsat.clone()).unwrap(),
        QualityMaskRequest::Landsat(_)
    ));
    for mut bad in [modis.clone(), landsat.clone()] {
        bad["policy"] = json!("clear_best_unrecognized");
        assert!(serde_json::from_value::<QualityMaskRequest>(bad).is_err());
    }
    let mut bad = landsat;
    bad["qcJobId"] = json!(a);
    assert!(serde_json::from_value::<QualityMaskRequest>(bad).is_err());
    bad = modis;
    bad["policy"] = json!("cloud_free");
    assert!(serde_json::from_value::<QualityMaskRequest>(bad).is_err());
}

#[tokio::test]
async fn landsat_coupled_preserves_unsigned_triplets_and_point_centres_after_parent_removal() {
    use crate::projects::{CreateProjectRequest, ProjectAsset, ProjectScene, ReflectanceBand};
    for point in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let manager = JobManager::open(&root).await.unwrap();
        let (rgb, qa, newest_values) = fixtures(&root, point);
        let newer: [Job; 5] = rgb
            .into_iter()
            .chain(qa)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let (rgb, qa, _) = fixtures(&root, point);
        let mut older: [Job; 5] = rgb
            .into_iter()
            .chain(qa)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let old_values: [[i32; 64]; 3] = std::array::from_fn(|c| {
            std::array::from_fn(|i| {
                if i == 21 {
                    0
                } else {
                    20000 + c as i32 * 1000 + i as i32
                }
            })
        });
        for (c, job) in older.iter_mut().enumerate() {
            job.item_id = ITEM.replace("20250628", "20250612");
            job.href = job
                .href
                .replace("20250628", "20250612")
                .replace("20250629", "20250613");
            let values = if c < 3 {
                old_values[c].to_vec()
            } else if c == 3 {
                (0..64)
                    .map(|i| if i == 5 { CLEAR | 8 } else { CLEAR })
                    .map(|v| v as i32)
                    .collect()
            } else {
                vec![0; 64]
            };
            let bytes = fixture_sized(
                false,
                if c == 3 { "1" } else { "0" },
                30.,
                if point { 2 } else { 1 },
                &values,
                32,
                2,
            );
            std::fs::write(job.output_path.as_ref().unwrap(), &bytes).unwrap();
            job.bytes_downloaded = bytes.len() as u64;
            job.total_bytes = Some(job.bytes_downloaded);
            job.sha256 = Some(format!("{:x}", Sha256::digest(bytes)));
        }
        let originals = vec![older, newer];
        {
            let mut store = manager.inner.store.lock().await;
            for job in originals.iter().flatten() {
                store.jobs.insert(job.id.clone(), job.clone());
            }
            manager.persist(&store.jobs).await.unwrap();
        }
        let bounds = [-123.1, 37., -121.8, 38.1];
        let scenes = originals
            .iter()
            .enumerate()
            .map(|(i, jobs)| ProjectScene {
                footprint: None,
                item_id: jobs[0].item_id.clone(),
                date: if i == 0 {
                    "2025-06-12T00:00:00Z"
                } else {
                    "2025-06-28T00:00:00Z"
                }
                .into(),
                cloud: None,
                crs: Some("EPSG:32610".into()),
                grid_code: None,
                bbox: bounds,
                assets: jobs
                    .iter()
                    .enumerate()
                    .map(|(c, j)| {
                        (
                            j.asset_key.clone(),
                            ProjectAsset {
                                href: j.href.clone(),
                                media_type: j.media_type.clone(),
                                raster_band: (c < 3).then_some(ReflectanceBand {
                                    data_type: "uint16".into(),
                                    scale: 0.0000275,
                                    offset: -0.2,
                                    nodata: 0.,
                                    spatial_resolution: 30.,
                                }),
                            },
                        )
                    })
                    .collect(),
            })
            .collect();
        let project = manager
            .create_project(CreateProjectRequest {
                name: "Synthetic Landsat coherent selection".into(),
                bounds,
                geometry: None,
                scenes,
            })
            .await
            .unwrap();
        let mut parents = Vec::new();
        for key in ["red", "green", "blue", "qa_pixel", "qa_radsat"] {
            let job = manager
                .wait(
                    &manager
                        .run_project_mosaic(&project.id, key)
                        .await
                        .unwrap()
                        .id,
                )
                .await
                .unwrap();
            assert_eq!(job.status, JobStatus::Succeeded, "{:?}", job.error);
            parents.push(job);
        }
        let rgb = parents[..3].to_vec().try_into().unwrap();
        let qa = parents[3..].to_vec().try_into().unwrap();
        let mut outputs = Vec::new();
        for policy in [
            LandsatMaskPolicy::CloudFree,
            LandsatMaskPolicy::CloudFreeConservative,
        ] {
            for snow in [false, true] {
                let r = request(&rgb, &qa, policy, snow);
                let plan = manager.plan_scientific_rgb(r.clone()).await.unwrap();
                let mask = plan.spec.quality_mask.as_ref().unwrap();
                assert_eq!(mask.schema_version(), "geod-landsat-rgb-mask/v2");
                assert_eq!(
                    mask.coupled().unwrap().scenes[0].grid.pixel_interpretation,
                    if point { "PixelIsPoint" } else { "PixelIsArea" }
                );
                assert!(plan.required_disk_bytes >= 64 * 14);
                let mut bad = plan.spec.clone();
                bad.quality_mask
                    .as_mut()
                    .unwrap()
                    .coupled_mut()
                    .unwrap()
                    .scenes
                    .reverse();
                assert!(validate_spec(&bad).is_err());
                bad = plan.spec.clone();
                bad.quality_mask
                    .as_mut()
                    .unwrap()
                    .coupled_mut()
                    .unwrap()
                    .scenes[0]
                    .sources[3]
                    .pin
                    .sha256 = "0".repeat(64);
                assert!(validate_spec(&bad).is_err());
                let done = manager
                    .wait(&manager.run_scientific_rgb(r).await.unwrap().id)
                    .await
                    .unwrap();
                assert_eq!(done.status, JobStatus::Succeeded, "{:?}", done.error);
                let mut fallback = vec![1, 2, 3, 4, 14, 15, 16, 17, 22];
                if policy == LandsatMaskPolicy::CloudFreeConservative {
                    fallback.extend([8, 9, 10, 11, 12, 19]);
                    if snow {
                        fallback.push(23);
                    }
                }
                if snow {
                    fallback.push(6);
                }
                let g = &done.rgb_spec.as_ref().unwrap().grid;
                assert_eq!((g.width, g.height), (32, 2));
                let mut expected = [Vec::new(), Vec::new(), Vec::new()];
                for i in 0..64 {
                    let values = if i == 5 || i == 21 {
                        [0; 3]
                    } else if i == 20 || fallback.contains(&i) {
                        old_values.each_ref().map(|v| v[i])
                    } else {
                        newest_values.each_ref().map(|v| v[i])
                    };
                    for c in 0..3 {
                        expected[c].extend_from_slice(&(values[c] as u16).to_le_bytes());
                    }
                    let pixel = manager
                        .sample_scientific_rgb(
                            &done.id,
                            g.bounds[0] + (i % 32) as f64 * 30. + 15.,
                            g.bounds[3] - (i / 32) as f64 * 30. - 15.,
                        )
                        .await
                        .unwrap();
                    assert_eq!(pixel.values, values, "{policy:?} snow {snow} pixel {i}");
                }
                let result = done.rgb_output.as_ref().unwrap();
                assert_eq!(
                    result.samples_sha256,
                    expected.map(|v| format!("{:x}", Sha256::digest(v)))
                );
                assert_eq!(result.common_valid_pixels, 62);
                let counts = result.quality_mask.as_ref().unwrap();
                assert_eq!(
                    counts.coupled.as_ref().unwrap().fallback_pixels,
                    fallback.len() as u64
                );
                assert_eq!(
                    counts.coupled.as_ref().unwrap().scene_valid_pixels,
                    vec![fallback.len() as u64 + 1, 61 - fallback.len() as u64]
                );
                let cancel = CancellationToken::new();
                cancel.cancel();
                let (tx, _) = tokio::sync::mpsc::unbounded_channel();
                assert!(coupled::write(
                    &root,
                    &uuid::Uuid::new_v4().to_string(),
                    &plan.spec,
                    &originals,
                    &cancel,
                    &tx
                )
                .is_err());
                outputs.push(done);
            }
        }
        {
            let mut store = manager.inner.store.lock().await;
            store.jobs.remove(&originals[0][3].id);
            assert!(
                coupled::validate_sources(outputs[0].rgb_spec.as_ref().unwrap(), &store.jobs)
                    .is_err()
            );
            for j in originals.iter().flatten().chain(&parents) {
                store.jobs.remove(&j.id);
                std::fs::remove_file(j.output_path.as_ref().unwrap()).unwrap();
            }
            manager.persist(&store.jobs).await.unwrap();
        }
        drop(manager);
        let manager = JobManager::open(&root).await.unwrap();
        for done in outputs {
            assert!(manager.inspect_scientific_rgb(&done.id).await.is_ok());
            let package = manager.prepare_artifact(&done.id).await.unwrap();
            let mut zip = zip::ZipArchive::new(File::open(package.path).unwrap()).unwrap();
            let mut note = String::new();
            zip.by_name("README.txt")
                .unwrap()
                .read_to_string(&mut note)
                .unwrap();
            assert!(
                note.contains("original Landsat 8/9 scene")
                    && !note.contains("original MODIS scene")
            );
        }
    }
}
