use super::*;
use crate::{providers::modis, raster::quality};
use tiff::encoder::colortype;

fn value(key: &str, index: usize, newer: bool) -> u32 {
    let p = quality::profile(key).unwrap();
    if newer {
        if index.is_multiple_of(7) {
            p.nodata
        } else {
            (1u32 << (p.bits - 1)) | (index % 4) as u32
        }
    } else {
        match index % 6 {
            0 => p.nodata,
            1 => 0,
            2 => 1u32 << (p.bits - 1),
            3 => p.nodata - 1,
            4 => 7 << 2,
            _ => 3,
        }
    }
}
fn source(root: &Path, id: &str, key: &str, newer: bool) -> Job {
    let p = quality::profile(key).unwrap();
    let (h, v) = modis::identity(id).unwrap();
    let tile = modis::PIXEL * 2400.0;
    let mut bytes = std::io::Cursor::new(Vec::new());
    {
        let mut encoder = TiffEncoder::new(&mut bytes)
            .unwrap()
            .with_compression(tiff::encoder::Compression::Deflate(DeflateLevel::Balanced));
        macro_rules! image {
            ($ty:ty, $sample:ty) => {{
                let mut image = encoder.new_image::<$ty>(2400, 2400).unwrap();
                crate::raster::reflectance::modis::write_crs(image.encoder()).unwrap();
                image
                    .encoder()
                    .write_tag(
                        Tag::ModelPixelScaleTag,
                        &[modis::PIXEL, modis::PIXEL, 0.0][..],
                    )
                    .unwrap();
                image
                    .encoder()
                    .write_tag(
                        Tag::ModelTiepointTag,
                        &[
                            0.0,
                            0.0,
                            0.0,
                            (h as f64 - 18.0) * tile,
                            (9.0 - v as f64) * tile,
                            0.0,
                        ][..],
                    )
                    .unwrap();
                image
                    .encoder()
                    .write_tag(Tag::GdalNodata, p.nodata.to_string().as_str())
                    .unwrap();
                image.rows_per_strip(128).unwrap();
                let pixels: Vec<$sample> = (0..2400 * 2400)
                    .map(|i| value(key, i, newer) as $sample)
                    .collect();
                image.write_data(&pixels).unwrap();
            }};
        }
        if p.bits == 32 {
            image!(colortype::Gray32, u32);
        } else {
            image!(colortype::Gray16, u16);
        }
    }
    let mut job = crate::raster::tests::record(root, bytes.get_ref());
    let parts: Vec<_> = id.split('.').collect();
    job.item_id = id.into();
    job.asset_key = key.into();
    job.href = format!(
        "https://{}/modis-061-cogs/{}/{}/{}/{}/{}{}",
        modis::HOST,
        parts[0],
        &parts[2][1..3],
        &parts[2][4..],
        &parts[1][1..],
        id,
        modis::suffix(key).unwrap()
    );
    job
}
fn project(sources: &[Job], key: &str, polygon: bool) -> Project {
    let mut p = super::tests::project(sources, key);
    p.bounds = [-110.1, 34.96, -109.7, 35.04];
    if polygon {
        p.geometry = Some(crop::PolygonGeometry::Polygon(vec![
            vec![
                [-110.1, 34.96],
                [-109.7, 34.96],
                [-109.7, 35.04],
                [-110.1, 35.04],
                [-110.1, 34.96],
            ],
            vec![
                [-110.01, 34.99],
                [-110.01, 35.01],
                [-109.99, 35.01],
                [-109.99, 34.99],
                [-110.01, 34.99],
            ],
        ]));
    }
    for (scene, job) in p.scenes.iter_mut().zip(sources) {
        scene.item_id = job.item_id.clone();
        scene.date = modis::period(&job.item_id).unwrap()[0].clone();
        scene.crs = Some(modis::CRS.into());
    }
    p
}
fn output_job(sources: &[Job], project: &Project, output: MosaicOutput, id: String) -> Job {
    let mut job = sources[0].clone();
    job.id = id;
    job.kind = "raster_mosaic".into();
    job.item_id = format!("project:{}", project.id);
    job.mosaic = Some(MosaicSpec {
        project_id: project.id.clone(),
        asset_key: job.asset_key.clone(),
        sources: sources
            .iter()
            .map(|s| MosaicSource {
                job_id: s.id.clone(),
                sha256: s.sha256.clone().unwrap(),
            })
            .collect(),

        coverage_sources: Vec::new(),
        vi_selection: None,
    });
    job.output_path = Some(output.path);
    job.bytes_downloaded = output.bytes;
    job.total_bytes = Some(output.bytes);
    job.sha256 = Some(output.sha256);
    job.mosaic_output = Some(output.plan);
    job
}

#[test]
fn quality_mosaics_keep_unsigned_bits_zero_fill_fallback_and_polygon_holes() {
    for key in modis::QUALITY_KEYS {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let profile = quality::profile(key).unwrap();
        let sources = [
            source(
                &root,
                "MYD09A1.A2025177.h08v05.061.2025189031924",
                key,
                false,
            ),
            source(
                &root,
                "MYD09A1.A2025177.h09v05.061.2025189031924",
                key,
                false,
            ),
            source(
                &root,
                "MYD09A1.A2025185.h08v05.061.2025199031924",
                key,
                true,
            ),
        ];
        for polygon in [false, true] {
            let project = project(&sources, key, polygon);
            let id = Uuid::new_v4().to_string();
            let output = write_mosaic(
                &root,
                &project,
                &sources,
                key,
                &id,
                &CancellationToken::new(),
                None,
            )
            .unwrap();
            let mut decoder = Decoder::new(File::open(&output.path).unwrap()).unwrap();
            let actual: Vec<u32> = match decoder.read_image().unwrap() {
                DecodingResult::U32(v) if profile.bits == 32 => v,
                DecodingResult::U16(v) if profile.bits == 16 => {
                    v.into_iter().map(u32::from).collect()
                }
                _ => panic!("Original unsigned quality type was lost"),
            };
            assert_eq!(output.plan.quality.as_ref(), Some(&profile));
            assert!(output.plan.calibration.is_none());
            let plan = &output.plan;
            let seam = -9.0 * modis::PIXEL * 2400.0;
            assert!(plan.bounds[0] < seam && plan.bounds[2] > seam);
            let mut valid = 0;
            let mut masked = 0;
            let mut classes = [0u64; 4];
            let mut seen_zero = false;
            let mut seen_high = false;
            let mut seen_fallback = false;
            for row in 0..plan.height {
                for col in 0..plan.width {
                    let x = plan.bounds[0] + (col as f64 + 0.5) * modis::PIXEL;
                    let y = plan.bounds[3] - (row as f64 + 0.5) * modis::PIXEL;
                    let ll = modis::inverse([x, y]).unwrap();
                    let inside = ll[0] > -110.1
                        && ll[0] < -109.7
                        && ll[1] > 34.96
                        && ll[1] < 35.04
                        && !(ll[0] > -110.01 && ll[0] < -109.99 && ll[1] > 34.99 && ll[1] < 35.01);
                    let left = x < seam;
                    let tile_left = if left { -10.0 } else { -9.0 } * modis::PIXEL * 2400.0;
                    let sx = ((x - tile_left) / modis::PIXEL).floor() as usize;
                    let sy = ((4.0 * modis::PIXEL * 2400.0 - y) / modis::PIXEL).floor() as usize;
                    let index = sy * 2400 + sx;
                    let older = value(key, index, false);
                    let newer = value(key, index, true);
                    let mut expected = if left && newer != profile.nodata {
                        newer
                    } else {
                        older
                    };
                    if polygon && !inside {
                        expected = profile.nodata;
                        masked += 1;
                    }
                    assert_eq!(actual[(row * plan.width + col) as usize], expected);
                    if expected != profile.nodata {
                        valid += 1;
                        classes[(expected & 3) as usize] += 1;
                        seen_zero |= expected == 0;
                        seen_high |= expected & (1u32 << (profile.bits - 1)) != 0;
                        seen_fallback |= left && newer == profile.nodata;
                    }
                }
            }
            assert!(seen_zero && seen_high && seen_fallback);
            assert_eq!(plan.covered_pixels, valid);
            assert_eq!(plan.masked_pixels, masked);
            if polygon {
                assert!(masked > 0);
            }
            let result = output_job(&sources, &project, output, id);
            let metadata = quality::inspect(&root, &result, 160).unwrap();
            assert_eq!(
                metadata.quality.as_ref().unwrap().sample_count,
                actual.len() as u64
            );
            assert_eq!(
                metadata.classes.iter().map(|c| c.count).collect::<Vec<_>>(),
                classes
            );
            assert_eq!(metadata.nodata, Some(f64::from(profile.nodata)));
            assert!(metadata.reflectance.is_none());
            let row = metadata.height / 2;
            let col = metadata.width / 2;
            let pixel = quality::sample(
                &root,
                &result,
                metadata.bounds[0] + (col as f64 + 0.5) * modis::PIXEL,
                metadata.bounds[3] - (row as f64 + 0.5) * modis::PIXEL,
            )
            .unwrap();
            assert_eq!(
                pixel.value,
                f64::from(actual[(row * metadata.width + col) as usize])
            );
            let mut wrong = result.clone();
            wrong
                .mosaic_output
                .as_mut()
                .unwrap()
                .quality
                .as_mut()
                .unwrap()
                .bits = 8;
            assert!(quality::inspect(&root, &wrong, 160).is_err());
            let mut wrong = result.clone();
            wrong.mosaic_output.as_mut().unwrap().bounds[0] += modis::PIXEL;
            assert!(quality::inspect(&root, &wrong, 160).is_err());
            let mut wrong = result.clone();
            wrong.asset_key = "red".into();
            wrong.mosaic.as_mut().unwrap().asset_key = "red".into();
            assert!(validate_stored_mosaic(&wrong).is_err());
        }
    }
}

#[test]
fn quality_mosaic_rejects_changed_layer_checksum_and_cancellation() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let source = source(
        &root,
        "MYD09A1.A2025177.h08v05.061.2025189031924",
        "modis_qc",
        false,
    );
    assert!(source_raster(&root, &source, "modis_state", &CancellationToken::new()).is_err());
    let mut changed = source.clone();
    changed.sha256 = Some("a".repeat(64));
    assert!(source_raster(&root, &changed, "modis_qc", &CancellationToken::new()).is_err());
    let sources = [source];
    let p = project(&sources, "modis_qc", false);
    let cancel = CancellationToken::new();
    cancel.cancel();
    let id = Uuid::new_v4().to_string();
    assert!(write_mosaic(&root, &p, &sources, "modis_qc", &id, &cancel, None).is_err());
    assert!(!root.join("assets").join(format!("{id}.tif")).exists());
}

#[test]
fn quality_chunks_cannot_fall_back_to_byte_float_or_signed_science_decoding() {
    let qc = quality::profile("modis_qc").unwrap();
    let state = quality::profile("modis_state").unwrap();
    assert_eq!(
        decoded_bytes(
            DecodingResult::U32(vec![0, 0x8000_0000, u32::MAX]),
            None,
            None,
            false,
            Some(&qc)
        )
        .unwrap(),
        [0u32, 0x8000_0000, u32::MAX]
            .into_iter()
            .flat_map(u32::to_ne_bytes)
            .collect::<Vec<_>>()
    );
    for result in [
        DecodingResult::U8(vec![0]),
        DecodingResult::U16(vec![0]),
        DecodingResult::I32(vec![-1]),
        DecodingResult::F32(vec![0.0]),
    ] {
        assert!(decoded_bytes(result, None, None, false, Some(&qc)).is_err());
    }
    assert!(decoded_bytes(
        DecodingResult::U32(vec![0]),
        None,
        None,
        false,
        Some(&state)
    )
    .is_err());
    assert!(decoded_bytes(DecodingResult::U16(vec![0]), None, None, true, Some(&state)).is_err());
}
