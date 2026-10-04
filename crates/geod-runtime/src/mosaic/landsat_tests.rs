use super::*;
use crate::raster::landsat_quality as qa;
use crate::{crop::PolygonGeometry, providers};
use std::io::Cursor;
use tiff::encoder::colortype;

#[allow(clippy::too_many_arguments)]
fn source(
    root: &Path,
    key: &str,
    date: &str,
    path: &str,
    width: u32,
    height: u32,
    x: f64,
    point: bool,
    values: &[u16],
) -> Job {
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut encoder = TiffEncoder::new(&mut bytes)
            .unwrap()
            .with_compression(tiff::encoder::Compression::Deflate(DeflateLevel::Balanced));
        let mut image = encoder
            .new_image::<colortype::Gray16>(width, height)
            .unwrap();
        image
            .encoder()
            .write_tag(
                Tag::GeoKeyDirectoryTag,
                &[
                    1u16,
                    1,
                    0,
                    4,
                    1024,
                    0,
                    1,
                    1,
                    1025,
                    0,
                    1,
                    if point { 2 } else { 1 },
                    3072,
                    0,
                    1,
                    32610,
                    3076,
                    0,
                    1,
                    9001,
                ][..],
            )
            .unwrap();
        image
            .encoder()
            .write_tag(Tag::ModelPixelScaleTag, &[30.0, 30.0, 0.0][..])
            .unwrap();
        let shift = if point { 15.0 } else { 0.0 };
        image
            .encoder()
            .write_tag(
                Tag::ModelTiepointTag,
                &[0.0, 0.0, 0.0, x + shift, 4200000.0 - shift, 0.0][..],
            )
            .unwrap();
        image.rows_per_strip(128).unwrap();
        image.write_data(values).unwrap();
    }
    let mut job = crate::raster::tests::record(root, bytes.get_ref());
    job.asset_key = key.into();
    job.item_id = format!("LC09_L2SP_{path}034_{date}_02_T1");
    let product = format!("LC09_L2SP_{path}034_{date}_20250629_02_T1");
    job.href = format!("https://{}/landsat-c2/level-2/standard/oli-tirs/2025/{path}/034/{product}/{product}_{}.TIF", providers::LANDSAT_HOST,key.to_uppercase());
    job
}
fn project(sources: &[Job], key: &str) -> Project {
    let mut p = super::tests::project(sources, key);
    p.bounds = [-123.1, 37.0, -121.8, 38.1];
    for (scene, job) in p.scenes.iter_mut().zip(sources) {
        scene.item_id = job.item_id.clone();
        let date = job.item_id.split('_').nth(3).unwrap();
        scene.date = format!("{}-{}-{}T00:00:00Z", &date[..4], &date[4..6], &date[6..8]);
    }
    p
}
fn result(sources: &[Job], p: &Project, output: MosaicOutput, id: String) -> Job {
    let count = p.scenes.len();
    let pins = |slice: &[Job]| {
        slice
            .iter()
            .map(|job| MosaicSource {
                job_id: job.id.clone(),
                sha256: job.sha256.clone().unwrap(),
            })
            .collect()
    };
    let mut job = sources[0].clone();
    job.id = id;
    job.kind = "raster_mosaic".into();
    job.item_id = format!("project:{}", p.id);
    job.mosaic = Some(MosaicSpec {
        project_id: p.id.clone(),
        asset_key: job.asset_key.clone(),
        sources: pins(&sources[..count]),
        coverage_sources: pins(&sources[count..]),
        vi_selection: None,
    });
    job.output_path = Some(output.path);
    job.bytes_downloaded = output.bytes;
    job.total_bytes = Some(output.bytes);
    job.sha256 = Some(output.sha256);
    job.mosaic_output = Some(output.plan);
    job
}
fn read(root: &Path, job: &Job) -> (Vec<u16>, Vec<bool>) {
    let plan = job.mosaic_output.as_ref().unwrap();
    let mut decoder = Decoder::new(File::open(job.output_path.as_ref().unwrap()).unwrap()).unwrap();
    assert!(decoder.get_tag(Tag::GdalNodata).is_err());
    let values = match decoder.read_image().unwrap() {
        DecodingResult::U16(v) => v,
        _ => panic!("QA was not UInt16"),
    };
    let profile: qa::ProcessingProfile =
        serde_json::from_str(&decoder.get_tag_ascii_string(Tag::ImageDescription).unwrap())
            .unwrap();
    assert_eq!(Some(&profile), plan.landsat_quality.as_ref());
    crate::raster::aerial::mask::select(&mut decoder, plan.width, plan.height).unwrap();
    let mut covered = Vec::new();
    for index in 0..decoder.strip_count().unwrap() {
        let data = crate::raster::aerial::mask::strip(&mut decoder, index).unwrap();
        for row in data.chunks_exact(plan.width.div_ceil(8) as usize) {
            for x in 0..plan.width {
                covered.push(row[x as usize / 8] & (0x80 >> (x % 8)) != 0);
            }
            if !plan.width.is_multiple_of(8) {
                assert_eq!(row.last().unwrap() & ((1 << (8 - plan.width % 8)) - 1), 0);
            }
        }
    }
    assert_eq!(covered.len(), values.len());
    assert!(qa::inspect(root, job, 96).is_ok());
    (values, covered)
}

#[test]
fn landsat_quality_mosaics_preserve_unsigned_overlap_fallback_fill_and_gaps() {
    for &key in qa::KEYS {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let (w, h) = (13, 513);
        let n = (w * h) as usize;
        let pixel_old: Vec<u16> = (0..n)
            .map(|i| if i % 7 == 0 { 65 } else { 0x8040 })
            .collect();
        let pixel_new: Vec<u16> = (0..n)
            .map(|i| if i % 5 == 0 { 65535 } else { 0x2040 })
            .collect();
        let pixel_adj = vec![64; n];
        let raw_old: Vec<u16> = (0..n)
            .map(|i| if i % 2 == 0 { 0 } else { 0x8000 })
            .collect();
        let raw_new: Vec<u16> = (0..n).map(|i| if i % 3 == 0 { 0 } else { 65535 }).collect();
        let raw_adj = vec![2048; n];
        let primary = if key == "qa_pixel" {
            [&pixel_old, &pixel_new, &pixel_adj]
        } else {
            [&raw_old, &raw_new, &raw_adj]
        };
        let mut sources = vec![
            source(
                &root, key, "20250612", "044", w, h, 500000.0, true, primary[0],
            ),
            source(
                &root, key, "20250628", "044", w, h, 500000.0, false, primary[1],
            ),
            source(
                &root, key, "20250628", "045", w, h, 500420.0, true, primary[2],
            ),
        ];
        let p = project(&sources, key);
        if key == "qa_radsat" {
            sources.extend([
                source(
                    &root, "qa_pixel", "20250612", "044", w, h, 500000.0, true, &pixel_old,
                ),
                source(
                    &root, "qa_pixel", "20250628", "044", w, h, 500000.0, false, &pixel_new,
                ),
                source(
                    &root, "qa_pixel", "20250628", "045", w, h, 500420.0, true, &pixel_adj,
                ),
            ]);
        }
        let id = Uuid::new_v4().to_string();
        let output = write_mosaic(
            &root,
            &p,
            &sources,
            key,
            &id,
            &CancellationToken::new(),
            None,
        )
        .unwrap();
        assert_eq!((output.plan.width, output.plan.height), (27, h));
        assert_eq!(
            output.plan.bounds,
            [500000.0, 4200000.0 - h as f64 * 30.0, 500810.0, 4200000.0]
        );
        let job = result(&sources, &p, output, id);
        let (values, covered) = read(&root, &job);
        for y in 0..h {
            for x in 0..27 {
                let target = (y * 27 + x) as usize;
                let (expected, valid) = if x < 13 {
                    let i = (y * 13 + x) as usize;
                    if pixel_new[i] & 1 == 0 {
                        (primary[1][i], true)
                    } else if pixel_old[i] & 1 == 0 {
                        (primary[0][i], true)
                    } else {
                        (primary[1][i], false)
                    }
                } else if x == 13 {
                    (if key == "qa_pixel" { 1 } else { 0 }, false)
                } else {
                    (primary[2][(y * 13 + x - 14) as usize], true)
                };
                assert_eq!(
                    (values[target], covered[target]),
                    (expected, valid),
                    "{key} at {x},{y}"
                );
            }
        }
        let metadata = qa::inspect(&root, &job, 96).unwrap();
        assert_eq!(
            metadata.quality.as_ref().unwrap().valid_sample_count,
            covered.iter().filter(|v| **v).count() as u64
        );
        assert_eq!(
            metadata
                .quality
                .as_ref()
                .unwrap()
                .flags
                .as_ref()
                .unwrap()
                .coverage_mask
                .as_ref()
                .unwrap()
                .uncovered_pixels,
            covered.iter().filter(|v| !**v).count() as u64
        );
        for index in [0, 1, 13, 14, 27 * 127 + 5, 27 * 128 + 6] {
            let pixel = qa::sample(
                &root,
                &job,
                500000.0 + (index % 27) as f64 * 30.0 + 15.0,
                4200000.0 - (index / 27) as f64 * 30.0 - 15.0,
            )
            .unwrap();
            assert_eq!(pixel.value, values[index] as f64);
            assert_eq!(pixel.is_no_data, !covered[index]);
            assert_eq!(pixel.quality.unwrap().covered, Some(covered[index]));
        }
        let mut changed = job.clone();
        changed.mosaic_output.as_mut().unwrap().covered_pixels += 1;
        assert!(qa::inspect(&root, &changed, 96).is_err());
        let mut changed = job.clone();
        changed
            .mosaic_output
            .as_mut()
            .unwrap()
            .landsat_quality
            .as_mut()
            .unwrap()
            .bits = 8;
        assert!(qa::inspect(&root, &changed, 96).is_err());
        let jobs = sources.iter().map(|s| (s.id.clone(), s.clone())).collect();
        assert_eq!(
            validate_mosaic_sources(&job, &jobs).unwrap().len(),
            sources.len()
        );
        if key == "qa_radsat" {
            let mut changed = jobs;
            changed.get_mut(&sources[3].id).unwrap().sha256 = Some("a".repeat(64));
            assert!(validate_mosaic_sources(&job, &changed).is_err());
        }
    }
}

#[test]
fn landsat_quality_polygon_masks_and_missing_pairs_do_not_confuse_zero_with_fill() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let (w, h) = (100, 513);
    let n = (w * h) as usize;
    let saturation = source(
        &root,
        "qa_radsat",
        "20250628",
        "044",
        w,
        h,
        500000.0,
        true,
        &vec![0; n],
    );
    let pixel = source(
        &root,
        "qa_pixel",
        "20250628",
        "044",
        w,
        h,
        500000.0,
        true,
        &vec![64; n],
    );
    let mut p = project(std::slice::from_ref(&saturation), "qa_radsat");
    p.geometry = Some(PolygonGeometry::Polygon(vec![
        vec![
            [-123.1, 37.0],
            [-121.8, 37.0],
            [-121.8, 38.1],
            [-123.1, 38.1],
            [-123.1, 37.0],
        ],
        vec![
            [-122.999, 37.88],
            [-122.999, 37.90],
            [-122.98, 37.90],
            [-122.98, 37.88],
            [-122.999, 37.88],
        ],
    ]));
    let id = Uuid::new_v4().to_string();
    assert!(write_mosaic(
        &root,
        &p,
        std::slice::from_ref(&saturation),
        "qa_radsat",
        &id,
        &CancellationToken::new(),
        None
    )
    .is_err());
    let mut wrong = pixel.clone();
    wrong.href = wrong.href.replace("_20250629_", "_20250630_");
    assert!(landsat::validate_pair(&saturation, &wrong).is_err());
    let shifted = source(
        &root,
        "qa_pixel",
        "20250628",
        "044",
        w,
        h,
        500030.0,
        true,
        &vec![64; n],
    );
    assert!(write_mosaic(
        &root,
        &p,
        &[saturation.clone(), shifted],
        "qa_radsat",
        &id,
        &CancellationToken::new(),
        None
    )
    .is_err());
    let sources = [saturation, pixel];
    let output = write_mosaic(
        &root,
        &p,
        &sources,
        "qa_radsat",
        &id,
        &CancellationToken::new(),
        None,
    )
    .unwrap();
    assert!(output.plan.masked_pixels > 0 && output.plan.covered_pixels > 0);
    let job = result(&sources, &p, output, id);
    let (values, covered) = read(&root, &job);
    assert!(values.iter().all(|v| *v == 0));
    for state in [false, true] {
        let i = covered.iter().position(|v| *v == state).unwrap();
        let raw = qa::sample(
            &root,
            &job,
            500015.0 + (i % w as usize) as f64 * 30.0,
            4199985.0 - (i / w as usize) as f64 * 30.0,
        )
        .unwrap();
        assert_eq!(raw.value, 0.0);
        assert_eq!(raw.is_no_data, !state);
        assert_eq!(raw.quality.unwrap().covered, Some(state));
    }
}

#[test]
fn landsat_quality_streams_over_eight_million_pixels_and_cancels_without_commit() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let source = source(
        &root,
        "qa_pixel",
        "20250628",
        "044",
        3000,
        3000,
        500000.0,
        true,
        &vec![0x8040; 9_000_000],
    );
    let sources = [source];
    let p = project(&sources, "qa_pixel");
    let id = Uuid::new_v4().to_string();
    let output = write_mosaic(
        &root,
        &p,
        &sources,
        "qa_pixel",
        &id,
        &CancellationToken::new(),
        None,
    )
    .unwrap();
    assert_eq!(output.plan.covered_pixels, 9_000_000);
    assert_eq!(output.plan.width, 3000);
    assert_eq!(output.plan.height, 3000);
    let job = result(&sources, &p, output, id);
    let (values, covered) = read(&root, &job);
    assert!(values.iter().all(|v| *v == 0x8040) && covered.iter().all(|v| *v));
    let cancel = CancellationToken::new();
    let observation = stream::observe_blocks(Box::new({
        let token = cancel.clone();
        move |_| token.cancel()
    }));
    let id = Uuid::new_v4().to_string();
    let failed = write_mosaic(&root, &p, &sources, "qa_pixel", &id, &cancel, None);
    drop(observation);
    assert!(failed.is_err());
    assert!(!root.join("assets").join(format!("{id}.tif")).exists());
}

#[test]
fn landsat_quality_output_cannot_decode_signed_float_or_byte_values() {
    assert_eq!(
        landsat::decoded_bytes(DecodingResult::U16(vec![0, 65535, 32768])).unwrap(),
        [0u16, 65535, 32768]
            .into_iter()
            .flat_map(u16::to_ne_bytes)
            .collect::<Vec<_>>()
    );
    for decoded in [
        DecodingResult::U8(vec![0]),
        DecodingResult::I16(vec![-1]),
        DecodingResult::U32(vec![65535]),
        DecodingResult::F32(vec![0.0]),
    ] {
        assert!(landsat::decoded_bytes(decoded).is_err());
    }
}
