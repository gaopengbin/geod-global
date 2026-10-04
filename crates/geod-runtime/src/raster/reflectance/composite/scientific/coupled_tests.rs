use super::*;
use crate::projects::{CreateProjectRequest, ProjectAsset, ProjectScene, ReflectanceBand};

#[tokio::test]
async fn coupled_scene_selection_falls_back_as_a_triplet_and_survives_parent_removal() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let manager = JobManager::open(&root).await.unwrap();
    let old = mask_tests::fixtures_for(&root, "MYD09A1.A2025169.h08v05.061.2025178155736", true);
    let new = mask_tests::fixtures_for(&root, "MYD09A1.A2025177.h08v05.061.2025189031924", false);
    let groups: Vec<[Job; 5]> = [old, new]
        .into_iter()
        .map(|(rgb, qa)| {
            rgb.into_iter()
                .chain(qa)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap()
        })
        .collect();
    {
        let mut store = manager.inner.store.lock().await;
        for job in groups.iter().flatten() {
            store.jobs.insert(job.id.clone(), job.clone());
        }
        manager.persist(&store.jobs).await.unwrap();
    }
    let size = providers::modis::PIXEL;
    let left = -10.0 * size * 2400.0;
    let top = 4.0 * size * 2400.0;
    let corners = [
        [left, top],
        [left + 50.0 * size, top],
        [left + 50.0 * size, top - 2.0 * size],
        [left, top - 2.0 * size],
    ]
    .map(|p| providers::modis::inverse(p).unwrap());
    let bounds = [
        corners.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min),
        corners.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min),
        corners
            .iter()
            .map(|p| p[0])
            .fold(f64::NEG_INFINITY, f64::max),
        corners
            .iter()
            .map(|p| p[1])
            .fold(f64::NEG_INFINITY, f64::max),
    ];
    let scenes = groups
        .iter()
        .map(|jobs| ProjectScene {
            item_id: jobs[0].item_id.clone(),
            date: providers::modis::period(&jobs[0].item_id).unwrap()[0].clone(),
            cloud: None,
            crs: Some(providers::modis::CRS.into()),
            grid_code: None,
            bbox: bounds,
            assets: jobs
                .iter()
                .map(|job| {
                    (
                        job.asset_key.clone(),
                        ProjectAsset {
                            href: job.href.clone(),
                            media_type: job.media_type.clone(),
                            raster_band: ["red", "green", "blue"]
                                .contains(&job.asset_key.as_str())
                                .then_some(ReflectanceBand {
                                    data_type: "int16".into(),
                                    scale: 0.0001,
                                    offset: 0.0,
                                    nodata: -28672.0,
                                    spatial_resolution: 500.0,
                                }),
                        },
                    )
                })
                .collect(),
        })
        .collect();
    let project = manager
        .create_project(CreateProjectRequest {
            name: "Synthetic coupled MODIS control".into(),
            bounds,
            geometry: None,
            scenes,
        })
        .await
        .unwrap();
    let mut derived = Vec::new();
    for key in ["red", "green", "blue", "modis_qc", "modis_state"] {
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
        derived.push(job);
    }
    let mut outputs = Vec::new();
    for (policy, snow) in [
        (ModisMaskPolicy::Clear, false),
        (ModisMaskPolicy::ClearBest, false),
        (ModisMaskPolicy::ClearBest, true),
    ] {
        let request = RgbRequest {
            job_ids: derived[..3]
                .iter()
                .map(|j| j.id.clone())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
            project_id: Some(project.id.clone()),
            name: Some(format!("Synthetic coupled {policy:?} snow {snow}")),
            quality_mask: Some(
                ModisMaskRequest {
                    qc_job_id: derived[3].id.clone(),
                    state_job_id: derived[4].id.clone(),
                    policy,
                    exclude_snow: snow,
                }
                .into(),
            ),
        };
        let plan = manager.plan_scientific_rgb(request.clone()).await.unwrap();
        assert_eq!(
            plan.spec.quality_mask.as_ref().unwrap().schema_version(),
            "geod-modis-rgb-mask/v2"
        );
        assert_eq!(
            plan.spec
                .quality_mask
                .as_ref()
                .unwrap()
                .coupled()
                .unwrap()
                .scenes
                .len(),
            2
        );
        let mut changed = plan.spec.clone();
        changed
            .quality_mask
            .as_mut()
            .unwrap()
            .coupled_mut()
            .unwrap()
            .scenes[0]
            .sources[3]
            .pin
            .sha256 = "0".repeat(64);
        assert!(validate_spec(&changed).is_err());
        changed = plan.spec.clone();
        changed
            .quality_mask
            .as_mut()
            .unwrap()
            .coupled_mut()
            .unwrap()
            .scenes
            .reverse();
        assert!(validate_spec(&changed).is_err());
        let done = manager
            .wait(&manager.run_scientific_rgb(request).await.unwrap().id)
            .await
            .unwrap();
        assert_eq!(done.status, JobStatus::Succeeded, "{:?}", done.error);
        let counts = done
            .rgb_output
            .as_ref()
            .unwrap()
            .quality_mask
            .as_ref()
            .unwrap();
        assert!(counts.coupled.as_ref().unwrap().fallback_pixels > 0);
        assert!(counts
            .coupled
            .as_ref()
            .unwrap()
            .scene_valid_pixels
            .iter()
            .all(|n| *n > 0));
        for index in [0, 1, 3, 8, 9, 11, 14, 23, 24] {
            let expected =
                if index == 3 || index == 23 || index == 14 && policy == ModisMaskPolicy::ClearBest
                {
                    vec![-28672; 3]
                } else if index == 1
                    || index == 24
                    || index == 11 && policy == ModisMaskPolicy::ClearBest
                    || index == 9 && snow
                {
                    vec![1100 + index, 1200 + index, 1300 + index]
                } else {
                    vec![-100 + index, 20000 + index, index]
                };
            let pixel = manager
                .sample_scientific_rgb(
                    &done.id,
                    left + (index as f64 + 0.5) * size,
                    top - 0.5 * size,
                )
                .await
                .unwrap();
            assert_eq!(
                pixel.values.as_slice(),
                expected.as_slice(),
                "{policy:?} snow {snow} column {index}"
            );
        }
        // Independently mosaicked red/blue still contain the newer values at
        // column 24; the exported RGB must replace ALL channels together.
        let legacy = crate::raster::reflectance::sample(
            &root,
            &derived[0],
            left + 24.5 * size,
            top - 0.5 * size,
        )
        .unwrap();
        assert_eq!(legacy.value, -76.0);
        let token = CancellationToken::new();
        token.cancel();
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        assert!(coupled::write(&root, &id, &plan.spec, &groups, &token, &tx).is_err());
        assert!(!root.join("assets").join(format!("{id}.tif")).exists());
        outputs.push(done);
    }
    {
        let mut store = manager.inner.store.lock().await;
        let missing = store.jobs.remove(&groups[0][3].id).unwrap();
        manager.persist(&store.jobs).await.unwrap();
        assert!(
            coupled::validate_sources(outputs[0].rgb_spec.as_ref().unwrap(), &store.jobs).is_err()
        );
        store.jobs.insert(missing.id.clone(), missing);
        for job in groups.iter().flatten().chain(&derived) {
            store.jobs.remove(&job.id);
            std::fs::remove_file(job.output_path.as_ref().unwrap()).unwrap();
        }
        manager.persist(&store.jobs).await.unwrap();
    }
    drop(manager);
    let manager = JobManager::open(&root).await.unwrap();
    for done in outputs {
        assert!(manager.inspect_scientific_rgb(&done.id).await.is_ok());
        manager.prepare_artifact(&done.id).await.unwrap();
        assert!(manager.artifact_bytes(&done.id).await.is_ok());
        let mut changed = done.clone();
        changed
            .rgb_output
            .as_mut()
            .unwrap()
            .quality_mask
            .as_mut()
            .unwrap()
            .coupled
            .as_mut()
            .unwrap()
            .scene_valid_pixels[0] += 1;
        assert!(validate_stored(&changed).is_err());
    }
}
