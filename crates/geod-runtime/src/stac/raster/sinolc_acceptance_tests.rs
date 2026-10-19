//! Opt-in core-reader proof using complete independently verified originals.
//! This does not create a software download/job or claim UI acceptance.
use super::*;
use sha2::{Digest, Sha256};
use std::io::Cursor;

fn whole_file_identity(path: &Path, bytes: u64, expected_sha: &str) {
    let mut source = File::open(path).unwrap();
    let mut hash = Sha256::new();
    let mut block = [0u8; 65_536];
    let mut observed = 0u64;
    loop {
        let count = source.read(&mut block).unwrap();
        if count == 0 {
            break;
        }
        observed += count as u64;
        hash.update(&block[..count]);
    }
    assert_eq!(observed, bytes);
    assert_eq!(format!("{:x}", hash.finalize()), expected_sha);
}

#[test]
#[ignore = "requires SINOLC_REVIEWED_ORIGINAL_DIR with immutable whole ZIP/TIFF and independent GDAL preview references; no download"]
fn exact_original_core_preview_and_152_raw_references_match_independent_gdal() {
    let dir = PathBuf::from(std::env::var("SINOLC_REVIEWED_ORIGINAL_DIR").unwrap());
    let index: Value =
        serde_json::from_str(include_str!("../../datasets/sinolc-reviewed-index.json")).unwrap();
    let original = dir.join("East_Shanghai.tif");
    let member_sha = index["member"]["sha256"].as_str().unwrap();
    whole_file_identity(
        &dir.join("East_Shanghai.zip"),
        index["archiveBytes"].as_u64().unwrap(),
        index["archiveSha256"].as_str().unwrap(),
    );
    whole_file_identity(
        &original,
        index["member"]["bytes"].as_u64().unwrap(),
        member_sha,
    );

    // Identity is established by complete streaming reads before Decoder::new.
    let mut raster = decode_identified(
        BufReader::new(File::open(&original).unwrap()),
        "research-original-core-acceptance",
        member_sha,
    )
    .unwrap();
    assert!(raster.supported);
    assert!(crate::datasets::sinolc::budget_preview_profile(
        &raster.metadata
    ));
    assert_eq!(raster.metadata.bands[0].nodata, json!(0));
    assert_eq!(
        raster.metadata.categorical_encoding.as_ref().unwrap()["unlistedCodes"],
        json!([11, 250, 255])
    );
    assert_eq!(
        raster.metadata.categorical_encoding.as_ref().unwrap()["classes"]
            .as_array()
            .unwrap()
            .len(),
        11
    );
    assert!(raster.metadata.sample_encoding.is_none());
    assert_eq!(raster.decoder.tile_count().unwrap(), 230_136);
    assert_eq!(raster.decoder.chunk_dimensions(), (256, 256));
    assert!(!raster.decoder.more_images());
    let unknown = decode_identified(
        BufReader::new(File::open(&original).unwrap()),
        "unknown-identity-negative-case",
        &"0".repeat(64),
    )
    .err()
    .unwrap();
    assert!(
        unknown.contains("131072"),
        "ordinary axis rejection must remain: {unknown}"
    );
    assert!(validate_dimensions(132000, 114000, ChunkType::Tile).is_err());
    assert!(
        chunk_layout(&mut raster.decoder).is_err(),
        "ordinary chunk cap must remain unchanged"
    );

    let reference: Value = serde_json::from_slice(
        &std::fs::read(dir.join("sinolc-core-gdal-reference.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(reference["sourceSha256"], member_sha);
    assert_eq!(reference["sourceBytes"], 427_679_242u64);
    let expected_pixels = reference["pixels"].as_array().unwrap();
    assert_eq!(expected_pixels.len(), 1020);
    let preview_started = Instant::now();
    preview(&mut raster, 768).unwrap();
    let preview_millis = preview_started.elapsed().as_millis();
    assert_eq!(
        [
            raster.metadata.preview_width.unwrap(),
            raster.metadata.preview_height.unwrap()
        ],
        [34, 30]
    );
    assert!(raster
        .metadata
        .limitations
        .iter()
        .any(|v| v.contains("low-detail class-color")));
    let charge = preview_grid_charge_with_limit(
        &mut raster.decoder,
        [132000, 114000],
        1,
        34,
        512 * 1024 * 1024,
        crate::datasets::sinolc::CHUNK_COUNT,
    )
    .unwrap()
    .unwrap();
    assert_eq!(charge, (534_773_760, 1020));
    assert!(preview_grid_charge_with_limit(
        &mut raster.decoder,
        [132000, 114000],
        1,
        35,
        512 * 1024 * 1024,
        crate::datasets::sinolc::CHUNK_COUNT
    )
    .unwrap()
    .is_none());
    let png_bytes = STANDARD
        .decode(
            raster
                .metadata
                .preview_data_url
                .as_ref()
                .unwrap()
                .strip_prefix("data:image/png;base64,")
                .unwrap(),
        )
        .unwrap();
    let mut decoder = png::Decoder::new(Cursor::new(&png_bytes))
        .read_info()
        .unwrap();
    let mut rgba = vec![0u8; decoder.output_buffer_size().unwrap()];
    let info = decoder.next_frame(&mut rgba).unwrap();
    assert_eq!(
        (info.width, info.height, info.color_type, info.bit_depth),
        (34, 30, png::ColorType::Rgba, png::BitDepth::Eight)
    );
    rgba.truncate(info.buffer_size());
    for point in expected_pixels {
        let x = point["x"].as_u64().unwrap() as usize;
        let y = point["y"].as_u64().unwrap() as usize;
        let color: Vec<u8> = point["rgba"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect();
        assert_eq!(
            &rgba[(y * 34 + x) * 4..(y * 34 + x + 1) * 4],
            color,
            "native categorical PNG differs from independent GDAL at {x},{y}"
        );
    }
    assert!(expected_pixels.iter().any(|p| p["value"] == 0));
    assert!(expected_pixels.iter().any(|p| p["value"] == 9));

    let mut samples = Vec::new();
    let mut edge_count = 0;
    for (edge, points) in [(false, "pixels"), (true, "edgePixels")] {
        for point in index["member"][points].as_array().unwrap() {
            let column = point["column"].as_u64().unwrap() as u32;
            let row = point["row"].as_u64().unwrap() as u32;
            let (values, no_data) = pixel(&mut raster, column, row).unwrap();
            assert_eq!(json!(values), point["values"]);
            assert_eq!(json!(no_data), point["noData"]);
            let transform = raster.metadata.transform.unwrap();
            assert_eq!(
                transform[2] + (f64::from(column) + 0.5) * transform[0],
                point["longitude"].as_f64().unwrap()
            );
            assert_eq!(
                transform[5] + (f64::from(row) + 0.5) * transform[4],
                point["latitude"].as_f64().unwrap()
            );
            if edge {
                let (w, h) = raster
                    .decoder
                    .chunk_data_dimensions(row / 256 * 132000u32.div_ceil(256) + column / 256);
                assert!(w == 160 || h == 80);
                edge_count += 1;
            }
            samples.push(json!({"column":column,"row":row,"values":values,"noData":no_data,"partialEdge":edge}));
        }
    }
    assert_eq!((samples.len(), edge_count), (152, 8));
    assert!(samples.iter().any(|p| p["noData"] == json!([true])));
    assert!(pixel(&mut raster, 132000, 0).is_err());
    assert!(pixel(&mut raster, 0, 114000).is_err());

    std::fs::write(dir.join("sinolc-core-native-preview.png"), &png_bytes).unwrap();
    std::fs::write(
        dir.join("sinolc-core-native-inspection.json"),
        serde_json::to_vec_pretty(&raster.metadata).unwrap(),
    )
    .unwrap();
    let receipt = json!({"schema":"geod-sinolc-core-original-acceptance/v1","accepted":true,
        "scope":"Core-reader acceptance on immutable complete research originals; not a fresh software download, job, reopen or GUI acceptance",
        "archiveBytes":index["archiveBytes"],"archiveSha256":index["archiveSha256"],
        "memberBytes":index["member"]["bytes"],"memberSha256":member_sha,"originalGrid":[132000,114000],
        "previewWidth":34,"previewHeight":30,"previewPngBytes":png_bytes.len(),
        "previewPngSha256":format!("{:x}",Sha256::digest(&png_bytes)),"previewMillis":preview_millis,
        "previewCumulativeConservativeChunkBytes":charge.0,"previewChunks":charge.1,
        "previewPixelsIndependentlyCompared":expected_pixels.len(),"rawReferences":samples,
        "partialEdgeReferences":edge_count,"nodata":0,"unlistedCodes":[11,250,255],
        "unknownShaOrdinaryDimensionsRejected":true,"ordinaryChunkLimitUnchanged":true,
        "originalBytesUnmodified":true,"wholeGridDecoded":false,"softwareGuiAcceptance":false});
    std::fs::write(
        dir.join("sinolc-core-native-acceptance.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
    println!("SinoLC core acceptance: native 34×30 categorical PNG matched all 1020 independent GDAL samples; 144+8 raw refs matched; preview charge {} B, {} ms; unknown SHA rejected; software/GUI acceptance remains separate",charge.0,preview_millis);
}
