use super::*;
use crate::raster::reflectance::tests::{fixture_samples, fixture_with_type, record};

fn trio(root: &Path, signed: bool, values: [[i32; 6]; 3]) -> [Job; 3] {
    std::array::from_fn(|channel| {
        let bytes = fixture_samples(
            signed,
            if signed { "-9999" } else { "0" },
            30.0,
            1,
            &values[channel],
        );
        let mut job = record(root, signed, &bytes);
        job.asset_key = ["red", "green", "blue"][channel].into();
        job.href = if signed {
            job.href
                .replace(".B04.tif", [".B04.tif", ".B03.tif", ".B02.tif"][channel])
        } else {
            job.href.replace(
                "_SR_B4.TIF",
                ["_SR_B4.TIF", "_SR_B3.TIF", "_SR_B2.TIF"][channel],
            )
        };
        job
    })
}

fn pixels(metadata: &CompositeInspection) -> Vec<u8> {
    let bytes = STANDARD
        .decode(metadata.preview_data_url.split(',').nth(1).unwrap())
        .unwrap();
    let mut reader = png::Decoder::new(Cursor::new(bytes)).read_info().unwrap();
    let mut buffer = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buffer).unwrap();
    buffer.truncate(info.buffer_size());
    buffer
}

#[test]
fn original_unsigned_and_signed_dn_survive_rgb_stretch_and_channel_nodata() {
    for signed in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let fill = if signed { -9999 } else { 0 };
        let low = if signed { -100 } else { 1000 };
        let high = if signed { 32767 } else { 65535 };
        let values = [
            [fill, low, 1000, 10000, high, 20000],
            [1000, low + 1, 2000, fill, high, 10000],
            [2000, low + 2, 3000, 30000, high, 5000],
        ];
        let jobs = trio(&root, signed, values);
        let metadata = inspect(&root, &jobs, 768).unwrap();
        assert_eq!(metadata.grid.width, 3);
        assert_eq!(metadata.data_type, if signed { "Int16" } else { "UInt16" });
        assert_eq!(metadata.composite.valid_sample_count, 4);
        let png = pixels(&metadata);
        assert_eq!(&png[..4], &[0, 0, 0, 0]);
        assert_eq!(&png[12..16], &[0, 0, 0, 0]);
        for index in 0..6 {
            let result = sample(
                &root,
                &jobs,
                500015.0 + (index % 3) as f64 * 30.0,
                4199985.0 - (index / 3) as f64 * 30.0,
            )
            .unwrap();
            assert_eq!(result.values, values.map(|band| band[index]));
            assert_eq!(result.pixel, [(index % 3) as u32, (index / 3) as u32]);
            for channel in 0..3 {
                assert_eq!(
                    result.channel_no_data[channel],
                    result.values[channel] == fill
                );
                if result.values[channel] == fill {
                    assert_eq!(result.reflectances[channel], None);
                } else {
                    assert_eq!(
                        result.reflectances[channel],
                        Some(
                            result.values[channel] as f64 * metadata.composite.scale
                                + metadata.composite.offset
                        )
                    );
                }
            }
        }
        if signed {
            assert!(
                sample(&root, &jobs, 500045.0, 4199985.0)
                    .unwrap()
                    .reflectances[0]
                    .unwrap()
                    < 0.0
            );
        }
        assert!(
            sample(&root, &jobs, 500045.0, 4199955.0)
                .unwrap()
                .reflectances[1]
                .unwrap()
                > 1.0
        );
        for (channel, job) in jobs.iter().enumerate() {
            assert_eq!(
                crate::raster::reflectance::sample(&root, job, 500075.0, 4199985.0)
                    .unwrap()
                    .value,
                values[channel][2] as f64
            );
        }
    }
}

#[test]
fn rgb_uses_intersection_of_valid_samples_and_bounded_nearest_neighbour_grid() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let jobs = trio(&root, false, [[0, 10, 20, 30, 40, 50]; 3]);
    let metadata = inspect(&root, &jobs, 2).unwrap();
    assert_eq!([metadata.preview_width, metadata.preview_height], [2, 1]);
    assert_eq!(pixels(&metadata), [0, 0, 0, 0, 128, 128, 128, 255]);
    assert_eq!(metadata.composite.display_ranges, [[10, 10]; 3]);
    let jobs = trio(&root, false, [[0; 6]; 3]);
    let empty = inspect(&root, &jobs, 768).unwrap();
    assert_eq!(empty.composite.valid_sample_count, 0);
    assert!(pixels(&empty).iter().all(|value| *value == 0));
    assert!(inspect(&root, &jobs, 769).is_err());
    assert!(sample(&root, &jobs, 500090.0, 4200000.0).is_err());
    assert!(sample(&root, &jobs, f64::NAN, 4200000.0).is_err());
}

#[test]
fn mismatched_order_scene_grid_processing_version_and_changed_hash_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let jobs = trio(&root, false, [[100; 6]; 3]);
    let mut invalid = jobs.clone();
    invalid.swap(0, 1);
    assert!(inspect(&root, &invalid, 768).is_err());
    let mut invalid = jobs.clone();
    invalid[1].item_id = invalid[1].item_id.replace("20250628", "20250627");
    assert!(inspect(&root, &invalid, 768).is_err());
    let mut invalid = jobs.clone();
    invalid[1].href = invalid[1].href.replace("20250629", "20250630");
    assert!(inspect(&root, &invalid, 768).is_err());
    let mut point = record(&root, false, &fixture_with_type(false, "0", 30.0, 2));
    point.asset_key = "green".into();
    point.href = jobs[1].href.clone();
    assert!(inspect(&root, &[jobs[0].clone(), point, jobs[2].clone()], 768).is_err());
    let mut invalid = jobs.clone();
    invalid[2].sha256 = Some("f".repeat(64));
    assert!(inspect(&root, &invalid, 768).is_err());
    assert!(sample(&root, &invalid, 500015.0, 4199985.0).is_err());
    let mut invalid = jobs.clone();
    invalid[2].output_path = Some(root.join("outside.tif").to_string_lossy().into_owned());
    assert!(inspect(&root, &invalid, 768).is_err());
}
