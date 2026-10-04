use super::*;
use crate::providers::modis;
use tiff::encoder::colortype;

fn source(root: &Path, id: &str, newer: bool, shifted: bool) -> Job {
    let (h, v) = modis::identity(id).unwrap();
    let size = modis::PIXEL * 2400.0;
    let mut bytes = std::io::Cursor::new(Vec::new());
    let values: Vec<i16> = (0..2400 * 2400)
        .map(|i| {
            if newer {
                if i % 7 == 0 {
                    -28672
                } else {
                    12000
                }
            } else {
                match i % 5 {
                    0 => -28672,
                    1 => -100,
                    2 => 0,
                    3 => 10000,
                    _ => 32767,
                }
            }
        })
        .collect();
    {
        let mut tiff = TiffEncoder::new(&mut bytes)
            .unwrap()
            .with_compression(tiff::encoder::Compression::Deflate(DeflateLevel::Balanced));
        let mut image = tiff.new_image::<colortype::GrayI16>(2400, 2400).unwrap();
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
                    (f64::from(h) - 18.0) * size + if shifted { 0.25 * modis::PIXEL } else { 0.0 },
                    (9.0 - f64::from(v)) * size,
                    0.0,
                ][..],
            )
            .unwrap();
        image
            .encoder()
            .write_tag(Tag::GdalNodata, "-28672")
            .unwrap();
        image.rows_per_strip(128).unwrap();
        image.write_data(&values).unwrap();
    }
    let mut job = crate::raster::tests::record(root, bytes.get_ref());
    let p: Vec<_> = id.split('.').collect();
    job.item_id = id.into();
    job.asset_key = "red".into();
    job.href = format!(
        "https://{}/modis-061-cogs/{}/{}/{}/{}/{}_sur_refl_b01.tif",
        modis::HOST,
        p[0],
        &p[2][1..3],
        &p[2][4..],
        &p[1][1..],
        id
    );
    job
}

fn project(sources: &[Job]) -> Project {
    let mut project = super::tests::project(sources, "red");
    project.bounds = [-110.1, 34.96, -109.7, 35.04];
    for (scene, job) in project.scenes.iter_mut().zip(sources) {
        scene.item_id = job.item_id.clone();
        scene.date = modis::period(&job.item_id).unwrap()[0].clone();
        scene.crs = Some(modis::CRS.into());
    }
    project
}

#[test]
fn adjacent_modis_tiles_and_newer_nodata_keep_signed_original_values() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let sources = [
        source(
            &root,
            "MYD09A1.A2025177.h08v05.061.2025189031924",
            false,
            false,
        ),
        source(
            &root,
            "MYD09A1.A2025177.h09v05.061.2025189031924",
            false,
            false,
        ),
        source(
            &root,
            "MYD09A1.A2025185.h08v05.061.2025199031924",
            true,
            false,
        ),
    ];
    let project = project(&sources);
    let id = Uuid::new_v4().to_string();
    let output = write_mosaic(
        &root,
        &project,
        &sources,
        "red",
        &id,
        &CancellationToken::new(),
        None,
    )
    .unwrap();
    assert_eq!(output.plan.crs, modis::CRS);
    assert_eq!(output.plan.pixel_size, [modis::PIXEL; 2]);
    let mut decoded = Decoder::new(File::open(&output.path).unwrap()).unwrap();
    let DecodingResult::I16(actual) = decoded.read_image().unwrap() else {
        panic!("MODIS result lost Int16 samples")
    };
    let plan = &output.plan;
    let seam = -9.0 * modis::PIXEL * 2400.0;
    assert!(plan.bounds[0] < seam && plan.bounds[2] > seam);
    let mut valid = 0;
    let mut seen_left = false;
    let mut seen_right = false;
    for row in 0..plan.height {
        for col in 0..plan.width {
            let x = plan.bounds[0] + (f64::from(col) + 0.5) * modis::PIXEL;
            let y = plan.bounds[3] - (f64::from(row) + 0.5) * modis::PIXEL;
            let left = x < seam;
            seen_left |= left;
            seen_right |= !left;
            let tile_west = if left { -10.0 } else { -9.0 } * modis::PIXEL * 2400.0;
            let sx = ((x - tile_west) / modis::PIXEL).floor() as usize;
            let sy = ((4.0 * modis::PIXEL * 2400.0 - y) / modis::PIXEL).floor() as usize;
            let index = sy * 2400 + sx;
            let mut expected = match index % 5 {
                0 => -28672,
                1 => -100,
                2 => 0,
                3 => 10000,
                _ => 32767,
            };
            if left && !index.is_multiple_of(7) {
                expected = 12000;
            }
            assert_eq!(actual[(row * plan.width + col) as usize], expected);
            valid += u64::from(expected != -28672);
        }
    }
    assert!(seen_left && seen_right);
    assert_eq!(plan.covered_pixels, valid);
    let result = super::bands_tests::output_job(&root, &sources[0], &project, output, id);
    let metadata = crate::raster::reflectance::inspect(&root, &result, 160).unwrap();
    assert_eq!(metadata.crs, modis::CRS);
    assert_eq!(metadata.reflectance.unwrap().scale, 0.0001);
    let mut wrong = result.clone();
    wrong.mosaic_output.as_mut().unwrap().bounds[0] += modis::PIXEL;
    assert!(crate::raster::reflectance::inspect(&root, &wrong, 160).is_err());
}

#[test]
fn modis_original_tile_cannot_be_shifted_into_a_plausible_adjacent_grid() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let job = source(
        &root,
        "MYD09A1.A2025177.h08v05.061.2025189031924",
        false,
        true,
    );
    assert!(source_raster(&root, &job, "red", &CancellationToken::new())
        .err()
        .unwrap()
        .contains("tile identity"));
}

#[test]
fn modis_exact_envelopes_and_polygon_centres_include_equator_and_exclude_globe_gaps() {
    let extent = crop::projected_envelope([-100.0, -20.0, 50.0, 30.0], modis::CRS).unwrap();
    assert_eq!(extent[0], -100.0f64.to_radians() * modis::RADIUS);
    assert_eq!(extent[2], 50.0f64.to_radians() * modis::RADIUS);
    assert!(modis::inverse([2.0 * std::f64::consts::PI * modis::RADIUS, 0.0]).is_none());
    let point = modis::forward([-110.0, 35.0]).unwrap();
    let restored = modis::inverse(point).unwrap();
    assert!((restored[0] + 110.0).abs() < 1e-12);
    assert!((restored[1] - 35.0).abs() < 1e-12);
    assert_eq!(
        modis::inverse(modis::forward([180.0, 90.0]).unwrap()).unwrap(),
        [0.0, 90.0]
    );
    let geometry = crop::PolygonGeometry::Polygon(vec![vec![
        [-110.01, 34.99],
        [-109.99, 34.99],
        [-109.99, 35.01],
        [-110.01, 35.01],
        [-110.01, 34.99],
    ]]);
    let dx = modis::PIXEL;
    let mask = crop::polygon_coverage(
        &geometry,
        modis::CRS,
        [
            point[0] - dx / 2.0,
            point[1] - dx / 2.0,
            point[0] + dx * 3.5,
            point[1] + dx / 2.0,
        ],
        [dx; 2],
        4,
        1,
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(mask, [true, true, false, false]);
}
