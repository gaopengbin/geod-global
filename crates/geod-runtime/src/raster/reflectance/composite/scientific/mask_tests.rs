use super::*;
use tiff::encoder::{colortype, compression::DeflateLevel, Compression, TiffEncoder};

const ITEM: &str = "MYD09A1.A2025177.h08v05.061.2025189031924";
const PIXELS: usize = 2400 * 2400;
const FILL: i16 = -28672;

fn fixtures(root: &Path) -> ([Job; 3], [Job; 2]) {
    fixtures_for(root, ITEM, false)
}
pub(super) fn fixtures_for(root: &Path, item: &str, older: bool) -> ([Job; 3], [Job; 2]) {
    let records: Vec<Job> = ["red", "green", "blue", "modis_qc", "modis_state"]
        .iter()
        .enumerate()
        .map(|(i, key)| {
            let mut bytes = Cursor::new(Vec::new());
            let tile = std::f64::consts::PI * providers::modis::RADIUS / 18.0;
            let mut encoder = TiffEncoder::new(&mut bytes)
                .unwrap()
                .with_compression(Compression::Deflate(DeflateLevel::Balanced));
            macro_rules! write {
                ($ty:ty,$values:expr,$fill:expr) => {{
                    let mut image = encoder.new_image::<$ty>(2400, 2400).unwrap();
                    crate::raster::reflectance::modis::write_crs(image.encoder()).unwrap();
                    image
                        .encoder()
                        .write_tag(
                            Tag::ModelPixelScaleTag,
                            &[providers::modis::PIXEL, providers::modis::PIXEL, 0.0][..],
                        )
                        .unwrap();
                    image
                        .encoder()
                        .write_tag(
                            Tag::ModelTiepointTag,
                            &[0.0, 0.0, 0.0, -10.0 * tile, 4.0 * tile, 0.0][..],
                        )
                        .unwrap();
                    image.encoder().write_tag(Tag::GdalNodata, $fill).unwrap();
                    image.write_data(&$values).unwrap();
                }};
            }
            if i < 3 {
                let mut values = vec![
                    if older {
                        [1100, 1200, 1300][i]
                    } else {
                        [-100, 20000, 0][i]
                    };
                    PIXELS
                ];
                for (index, v) in values.iter_mut().enumerate().take(25) {
                    *v += index as i16;
                }
                if i == 0 {
                    values[23] = FILL;
                } else if i == 1 && !older {
                    values[24] = FILL;
                }
                write!(colortype::GrayI16, values, "-28672");
            } else if i == 3 {
                let mut values = vec![0xC000_0000u32; PIXELS];
                for (index, flag) in if older {
                    vec![(14, 1 << 14)]
                } else {
                    vec![
                        (11, 1),
                        (12, 7 << 2),
                        (13, 11 << 10),
                        (14, 1 << 14),
                        (15, 7 << 6),
                        (20, 7 << 18),
                        (21, 7 << 22),
                    ]
                } {
                    values[index] |= flag;
                }
                if !older {
                    values[16] = u32::MAX;
                }
                write!(colortype::Gray32, values, "4294967295");
            } else {
                let mut values = vec![8u16; PIXELS];
                for (index, flag) in if older {
                    vec![(3, 1)]
                } else {
                    vec![
                        (1, 1),
                        (2, 2),
                        (3, 3),
                        (4, 1 << 2),
                        (5, 1 << 8),
                        (6, 1 << 10),
                        (7, 1 << 13),
                        (8, 1 << 14),
                        (9, 1 << 12),
                        (10, 1 << 15),
                        (18, 1 << 11),
                        (19, 3 << 6),
                        (23, 1),
                    ]
                } {
                    values[index] |= flag;
                }
                if !older {
                    values[17] = u16::MAX;
                }
                write!(colortype::Gray16, values, "65535");
            }
            let mut job = crate::raster::tests::record(root, &bytes.into_inner());
            job.asset_key = (*key).into();
            job.item_id = item.into();
            job.href = format!(
                "https://{}/modis-061-cogs/MYD09A1/08/05/{}/{item}{}",
                providers::modis::HOST,
                item.split('.').nth(1).unwrap().trim_start_matches('A'),
                providers::modis::suffix(key).unwrap()
            );
            job
        })
        .collect();
    (
        records[..3].to_vec().try_into().unwrap(),
        records[3..].to_vec().try_into().unwrap(),
    )
}
fn request(rgb: &[Job; 3], qa: &[Job; 2], policy: ModisMaskPolicy, snow: bool) -> RgbRequest {
    RgbRequest {
        job_ids: rgb.clone().map(|j| j.id),
        project_id: None,
        name: Some(format!("{policy:?} snow {snow}")),
        quality_mask: Some(
            ModisMaskRequest {
                qc_job_id: qa[0].id.clone(),
                state_job_id: qa[1].id.clone(),
                policy,
                exclude_snow: snow,
            }
            .into(),
        ),
    }
}

#[tokio::test]
async fn modis_mask_uses_unsigned_flags_preserves_accepted_dn_and_survives_restart_without_parents()
{
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let manager = JobManager::open(&root).await.unwrap();
    let (rgb, qa) = fixtures(&root);
    {
        let mut store = manager.inner.store.lock().await;
        for job in rgb.iter().chain(&qa) {
            store.jobs.insert(job.id.clone(), job.clone());
        }
        manager.persist(&store.jobs).await.unwrap();
    }
    let mut outputs = Vec::new();
    for (policy, snow, extra) in [
        (ModisMaskPolicy::Clear, false, vec![]),
        (ModisMaskPolicy::ClearBest, false, vec![11, 12, 13, 14]),
        (
            ModisMaskPolicy::ClearBest,
            true,
            vec![9, 10, 11, 12, 13, 14],
        ),
    ] {
        let mut rejected = vec![1, 2, 3, 4, 5, 6, 7, 16, 17, 23];
        rejected.extend(extra);
        let r = request(&rgb, &qa, policy, snow);
        let plan = manager.plan_scientific_rgb(r.clone()).await.unwrap();
        assert_eq!(
            plan.spec.quality_mask.as_ref().unwrap().sources()[0]
                .pin
                .sha256,
            qa[0].sha256.clone().unwrap()
        );
        let done = manager
            .wait(&manager.run_scientific_rgb(r).await.unwrap().id)
            .await
            .unwrap();
        assert_eq!(done.status, JobStatus::Succeeded, "{:?}", done.error);
        let result = done.rgb_output.as_ref().unwrap();
        let counts = result.quality_mask.as_ref().unwrap();
        assert_eq!(counts.examined_pixels, PIXELS as u64);
        assert_eq!(counts.rejected_pixels, rejected.len() as u64);
        assert_eq!(counts.input_common_valid_pixels, PIXELS as u64 - 2);
        assert_eq!(counts.removed_valid_pixels, rejected.len() as u64 - 1);
        assert_eq!(
            result.common_valid_pixels,
            PIXELS as u64 - rejected.len() as u64 - 1
        );
        for channel in 0..3 {
            let mut expected = vec![[-100i16, 20000, 0][channel]; PIXELS];
            for (i, v) in expected.iter_mut().enumerate().take(25) {
                *v += i as i16;
            }
            if channel == 0 {
                expected[23] = FILL;
            } else if channel == 1 {
                expected[24] = FILL;
            }
            for index in &rejected {
                expected[*index] = FILL;
            }
            assert_eq!(
                result.samples_sha256[channel],
                format!(
                    "{:x}",
                    Sha256::digest(
                        expected
                            .into_iter()
                            .flat_map(i16::to_le_bytes)
                            .collect::<Vec<_>>()
                    )
                )
            );
        }
        let spec = done.rgb_spec.as_ref().unwrap();
        // Salt-pan and unsigned bit 31 are not snow or cloud. Negative, zero,
        // above-one reflectance and non-RGB quality flags are not DN cutoffs.
        for index in [0, 8, 15, 18, 19, 20, 21, 22] {
            let point = manager
                .sample_scientific_rgb(
                    &done.id,
                    spec.grid.bounds[0] + (index as f64 + 0.5) * providers::modis::PIXEL,
                    spec.grid.bounds[3] - 0.5 * providers::modis::PIXEL,
                )
                .await
                .unwrap();
            assert_eq!(point.values, [-100 + index, 20000 + index, index]);
        }
        outputs.push(done);
    }
    let r = request(&rgb, &qa, ModisMaskPolicy::Clear, false);
    let spec = manager.plan_scientific_rgb(r.clone()).await.unwrap().spec;
    let mut other = spec.clone();
    other.quality_mask.as_mut().unwrap().sources_mut()[0].item_id =
        ITEM.replace("A2025177", "A2025185");
    assert!(validate_spec(&other).is_err());
    other = spec.clone();
    other.sources[0].kind = "raster_mosaic".into();
    other.sources[0].provenance = Some(json!({"sources":[{},{}]}));
    assert!(validate_spec(&other).is_err());
    let token = CancellationToken::new();
    token.cancel();
    let id = uuid::Uuid::new_v4().to_string();
    let (tx, _) = tokio::sync::mpsc::unbounded_channel();
    assert!(encode::write(&root, &id, &spec, &rgb, Some(&qa), &token, &tx).is_err());
    assert!(!root.join("assets").join(format!("{id}.tif")).exists());
    {
        let mut store = manager.inner.store.lock().await;
        for job in rgb.iter().chain(&qa) {
            assert_eq!(
                format!(
                    "{:x}",
                    Sha256::digest(std::fs::read(job.output_path.as_ref().unwrap()).unwrap())
                ),
                job.sha256.clone().unwrap()
            );
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
        let (_, bytes) = manager.artifact_bytes(&done.id).await.unwrap();
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut readme = String::new();
        zip.by_name("README.txt")
            .unwrap()
            .read_to_string(&mut readme)
            .unwrap();
        assert!(readme.contains("quality-layer pins"));
        assert!(!readme.contains("All original 16-bit DN values remain unchanged"));
        let mut manifest = String::new();
        zip.by_name(&format!("{}.metadata.json", done.id))
            .unwrap()
            .read_to_string(&mut manifest)
            .unwrap();
        let json: Value = serde_json::from_str(&manifest).unwrap();
        assert_eq!(
            json["spec"]["qualityMask"]["definition"],
            crate::raster::quality::DEFINITION
        );
        let mut changed = done.clone();
        changed
            .rgb_output
            .as_mut()
            .unwrap()
            .quality_mask
            .as_mut()
            .unwrap()
            .removed_valid_pixels += 1;
        assert!(validate_stored(&changed).is_err());
    }
}

#[test]
fn clear_policy_rejects_each_cloud_state_and_each_rgb_quality_nibble_without_using_signed_bits() {
    use modis_mask::accepted;
    for state in [1, 2, 3, 4, 256, 512, 768, 1024, 8192, 65535] {
        assert!(!accepted(ModisMaskPolicy::Clear, false, 0xC000_0000, state));
    }
    for qc in [
        1,
        2,
        3,
        1 << 2,
        15 << 2,
        1 << 10,
        15 << 10,
        1 << 14,
        15 << 14,
        u32::MAX,
    ] {
        assert!(!accepted(ModisMaskPolicy::ClearBest, false, qc, 0));
    }
    assert!(accepted(
        ModisMaskPolicy::ClearBest,
        true,
        0xC000_0000,
        1 << 14
    ));
    for bit in [12, 15] {
        assert!(accepted(ModisMaskPolicy::ClearBest, false, 0, 1 << bit));
        assert!(!accepted(ModisMaskPolicy::ClearBest, true, 0, 1 << bit));
    }
    assert!(accepted(ModisMaskPolicy::Clear, false, 7 << 2, 0));
}
