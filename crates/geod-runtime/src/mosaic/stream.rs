//! Bounded strip buffers, streamed GeoTIFF encoding and streamed read-back.
use super::*;
use crate::crop::PolygonGeometry;
use tiff::encoder::{
    compression::{CompressionAlgorithm, Deflate},
    TiffKind,
};

const STRIP_BUFFER_BYTES: usize = 64 * 1024 * 1024;
const MAX_STRIP_ROWS: u32 = 512;

// A synchronous test observer makes cancellation at a completed block
// deterministic without changing release code or relying on thread scheduling.
#[cfg(test)]
type BlockObserver = Box<dyn Fn(u32)>;
#[cfg(test)]
thread_local! {
    static BLOCK_OBSERVER: std::cell::RefCell<Option<BlockObserver>> = const { std::cell::RefCell::new(None) };
}
#[cfg(test)]
pub(super) struct BlockObservation;
#[cfg(test)]
impl Drop for BlockObservation {
    fn drop(&mut self) {
        BLOCK_OBSERVER.with(|observer| *observer.borrow_mut() = None);
    }
}
#[cfg(test)]
pub(super) fn observe_blocks(observer: BlockObserver) -> BlockObservation {
    BLOCK_OBSERVER.with(|slot| *slot.borrow_mut() = Some(observer));
    BlockObservation
}

fn sample_bytes(plan: &MosaicPlan) -> usize {
    if plan.landsat_quality.is_some() {
        2
    } else if let Some(profile) = &plan.quality {
        usize::from(profile.bits / 8)
    } else if crate::raster::srtm::signed(plan.elevation.as_ref()) {
        2
    } else if plan.elevation.is_some() || plan.radar.is_some() {
        4
    } else if let Some(profile) = &plan.calibration {
        usize::from(profile.bits() / 8)
    } else {
        1
    }
}

fn strip_rows(plan: &MosaicPlan, polygon: bool) -> Result<u32> {
    // Include the worst-case compressed strip as well as pixel/coverage masks.
    let buffers_per_pixel = plan.band_count as usize * sample_bytes(plan) * 2
        + 1
        + usize::from(polygon)
        + if plan.vi_quality.is_some() {
            25
        } else if plan.landsat_quality.is_some() {
            6
        } else if plan.aerial.is_some() {
            2
        } else {
            0
        };
    let row_bytes = (plan.width as usize)
        .checked_mul(buffers_per_pixel)
        .ok_or("Mosaic row size overflow")?;
    if row_bytes == 0 || row_bytes > STRIP_BUFFER_BYTES {
        return Err("Mosaic row exceeds the block memory budget".into());
    }
    let mut rows = (STRIP_BUFFER_BYTES / row_bytes) as u32;
    if polygon {
        rows = rows.min((8_000_000 / plan.width as u64) as u32);
    }
    Ok(rows.clamp(1, MAX_STRIP_ROWS).min(plan.height))
}

pub(super) fn preflight_budget(root: &Path, plan: &MosaicPlan, polygon: bool) -> Result<u64> {
    strip_rows(plan, polygon)?;
    let assets = root.join("assets").canonicalize().map_err(io_error)?;
    if assets != root.join("assets") {
        return Err("Managed output directory was redirected".into());
    }
    let raw_bytes = (plan.width as u64)
        .checked_mul(plan.height as u64)
        .and_then(|count| count.checked_mul(plan.band_count as u64 * sample_bytes(plan) as u64))
        .ok_or("Mosaic output byte count overflow")?;
    // Compressed size is unknown until written. Reserve a conservative upper
    // estimate so an incomplete file cannot silently exhaust the workspace.
    let mask_bytes = if plan.aerial.is_some() || plan.landsat_quality.is_some() {
        u64::from(plan.width.div_ceil(8)) * u64::from(plan.height)
    } else {
        0
    };
    let required = raw_bytes
        .checked_add(mask_bytes * 2)
        .ok_or("Mosaic mask disk estimate overflow")?
        .checked_add(raw_bytes / 100 + 1024 * 1024)
        .ok_or("Mosaic disk estimate overflow")?;
    let available = fs2::available_space(&assets).map_err(io_error)?;
    if available < required {
        return Err(format!("Insufficient workspace disk space for mosaic: need {required} bytes, available {available} bytes"));
    }
    Ok(required)
}

pub(super) fn encode_mosaic(
    root: &Path,
    id: &str,
    mut plan: MosaicPlan,
    rasters: &mut [SourceRaster],
    geometry: Option<&PolygonGeometry>,
    cancel: &CancellationToken,
    progress: Option<&UnboundedSender<(u64, &'static str)>>,
) -> Result<MosaicOutput> {
    check_cancel(Some(cancel))?;
    let assets = root.join("assets").canonicalize().map_err(io_error)?;
    let final_path = assets.join(format!("{id}.tif"));
    if final_path.exists() {
        return Err("Mosaic output already exists".into());
    }
    let required = preflight_budget(root, &plan, geometry.is_some())?;
    let mut temporary = tempfile::Builder::new()
        .prefix(&format!("{id}.mosaic-"))
        .suffix(".part")
        .tempfile_in(&assets)
        .map_err(io_error)?;
    let pixel_hash = if required >= u32::MAX as u64 {
        let mut encoder = TiffEncoder::new_big(temporary.as_file_mut()).map_err(io_error)?;
        encode_strips(
            &mut encoder,
            &mut plan,
            rasters,
            geometry,
            cancel,
            progress,
            &assets,
        )?
    } else {
        let mut encoder = TiffEncoder::new(temporary.as_file_mut()).map_err(io_error)?;
        encode_strips(
            &mut encoder,
            &mut plan,
            rasters,
            geometry,
            cancel,
            progress,
            &assets,
        )?
    };
    if plan.covered_pixels == 0 && plan.vi_quality.is_none() {
        return Err("No valid source pixels remain inside this project area".into());
    }
    temporary.as_file().sync_all().map_err(io_error)?;
    validate_output(temporary.path(), &plan, &pixel_hash, cancel, progress)?;
    let mut file = File::open(temporary.path()).map_err(io_error)?;
    let bytes = file.metadata().map_err(io_error)?.len();
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut read_bytes = 0;
    loop {
        check_cancel(Some(cancel))?;
        let count = file.read(&mut buffer).map_err(io_error)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        read_bytes += count as u64;
    }
    if read_bytes != bytes {
        return Err("Mosaic output changed during checksum validation".into());
    }
    let hash = format!("{:x}", hasher.finalize());
    drop(file);
    check_cancel(Some(cancel))?;
    temporary
        .persist_noclobber(&final_path)
        .map_err(|error| format!("Cannot commit mosaic: {}", error.error))?;
    Ok(MosaicOutput {
        path: final_path.to_string_lossy().into_owned(),
        bytes,
        sha256: hash,
        plan,
    })
}

fn encode_strips<K: TiffKind>(
    encoder: &mut TiffEncoder<&mut File, K>,
    plan: &mut MosaicPlan,
    rasters: &mut [SourceRaster],
    geometry: Option<&PolygonGeometry>,
    cancel: &CancellationToken,
    progress: Option<&UnboundedSender<(u64, &'static str)>>,
    assets: &Path,
) -> Result<String> {
    let rows = strip_rows(plan, geometry.is_some())?;
    let mut mask = if plan.aerial.is_some() || plan.landsat_quality.is_some() {
        Some(crate::raster::aerial::mask::Spool::new(assets)?)
    } else {
        None
    };
    // tiff 0.11.3's ImageEncoder::write_strip does not activate its compressor
    // (only write_data does). Use the public directory/compression APIs so
    // independently encoded Deflate strips match their declared TIFF tags.
    let mut image = encoder.image_directory().map_err(io_error)?;
    let bits = vec![(sample_bytes(plan) * 8) as u16; plan.band_count as usize];
    let formats = vec![
        if crate::raster::srtm::signed(plan.elevation.as_ref()) {
            2u16
        } else if plan.elevation.is_some() || plan.radar.is_some() {
            3u16
        } else if plan.calibration.as_ref().is_some_and(|p| p.signed) {
            2u16
        } else {
            1
        };
        plan.band_count as usize
    ];
    image
        .write_tag(Tag::ImageWidth, plan.width)
        .map_err(io_error)?;
    image
        .write_tag(Tag::ImageLength, plan.height)
        .map_err(io_error)?;
    image
        .write_tag(Tag::BitsPerSample, &bits[..])
        .map_err(io_error)?;
    image
        .write_tag(Tag::SamplesPerPixel, plan.band_count as u16)
        .map_err(io_error)?;
    image
        .write_tag(Tag::SampleFormat, &formats[..])
        .map_err(io_error)?;
    image
        .write_tag(
            Tag::PhotometricInterpretation,
            if plan.band_count == 1 { 1u16 } else { 2u16 },
        )
        .map_err(io_error)?;
    image.write_tag(Tag::Compression, 8u16).map_err(io_error)?;
    image
        .write_tag(Tag::PlanarConfiguration, 1u16)
        .map_err(io_error)?;
    image.write_tag(Tag::Orientation, 1u16).map_err(io_error)?;
    image.write_tag(Tag::RowsPerStrip, rows).map_err(io_error)?;
    add_geo_tags(&mut image, plan)?;
    let bands = plan.band_count as usize * sample_bytes(plan);
    let nodata = if let Some(profile) = &plan.landsat_quality {
        (if profile.band == "qa_pixel" { 1u16 } else { 0 })
            .to_ne_bytes()
            .to_vec()
    } else if let Some(profile) = &plan.quality {
        if profile.bits == 32 {
            profile.nodata.to_ne_bytes().to_vec()
        } else {
            (profile.nodata as u16).to_ne_bytes().to_vec()
        }
    } else if crate::raster::srtm::signed(plan.elevation.as_ref()) {
        i16::MIN.to_ne_bytes().to_vec()
    } else if plan.radar.is_some() {
        (-32768.0f32).to_ne_bytes().to_vec()
    } else if plan.elevation.is_some() {
        f32::NAN.to_ne_bytes().to_vec()
    } else {
        plan.calibration.as_ref().map_or_else(
            || vec![0; bands],
            |p| {
                if p.bits() == 8 {
                    vec![p.nodata as i8 as u8]
                } else {
                    (p.nodata as i16).to_ne_bytes().to_vec()
                }
            },
        )
    };
    let mut hasher = Sha256::new();
    let mut vi_counter = plan.vi_quality.clone().map(vegetation::Counter::new);
    let mut offsets = Vec::new();
    let mut byte_counts = Vec::new();
    let mut y = 0;
    while y < plan.height {
        check_cancel(Some(cancel))?;
        let height = rows.min(plan.height - y);
        let count = plan.width as usize * height as usize;
        let mut pixels = nodata.repeat(count);
        let mut covered = vec![false; count];
        let top = plan.bounds[3] - y as f64 * plan.pixel_size[1];
        let grid = MosaicGrid {
            origin: [plan.bounds[0], top],
            dimensions: [plan.width, height],
            pixel_size: plan.pixel_size,
        };
        // The time guard is per bounded block. A large healthy job is not
        // rejected simply because the entire project takes over three minutes.
        let deadline = Instant::now() + Duration::from_secs(180);
        let inside = if let Some(geometry) = geometry {
            let bounds = [
                plan.bounds[0],
                top - height as f64 * plan.pixel_size[1],
                plan.bounds[2],
                top,
            ];
            let inside = crop::polygon_coverage(
                geometry,
                &plan.crs,
                bounds,
                plan.pixel_size,
                plan.width,
                height,
                cancel,
            )?;
            Some(inside)
        } else {
            None
        };
        if let Some(counter) = &mut vi_counter {
            let key = if rasters[0].nodata == Some(-3000.0)
                && rasters[0]
                    .calibration
                    .as_ref()
                    .is_some_and(|p| p.product == crate::providers::vegetation::PRODUCT)
            {
                // The caller places the requested index first; the value is
                // supplied below by the output's explicit index identity.
                plan.vi_index
                    .as_deref()
                    .ok_or("Vegetation output index is missing")?
            } else {
                return Err("Vegetation output profile differs".into());
            };
            vegetation::copy_block(
                rasters,
                key,
                &grid,
                &mut pixels,
                &mut covered,
                inside.as_deref(),
                counter,
                cancel,
                deadline,
            )?;
        } else {
            for raster in rasters.iter_mut() {
                copy_source(raster, &grid, &mut pixels, &mut covered, cancel, deadline)?;
            }
        }
        if let Some(inside) = &inside {
            for (index, included) in inside.iter().enumerate() {
                if !included {
                    plan.masked_pixels += 1;
                    covered[index] = false;
                    pixels[index * bands..(index + 1) * bands].copy_from_slice(&nodata);
                }
            }
        }
        plan.covered_pixels += covered.iter().filter(|value| **value).count() as u64;
        if let Some(mask) = &mut mask {
            mask.strip(&covered, plan.width, cancel)?;
        }
        hasher.update(&pixels);
        let mut compressed = Vec::new();
        Deflate::with_level(DeflateLevel::Balanced)
            .write_to(&mut compressed, &pixels)
            .map_err(io_error)?;
        check_cancel(Some(cancel))?;
        offsets.push(
            K::convert_offset(image.write_data(&compressed[..]).map_err(io_error)?)
                .map_err(io_error)?,
        );
        byte_counts.push(K::convert_offset(compressed.len() as u64).map_err(io_error)?);
        y += height;
        #[cfg(test)]
        BLOCK_OBSERVER.with(|observer| {
            if let Some(observer) = observer.borrow().as_ref() {
                observer(y);
            }
        });
        if let Some(progress) = progress {
            let _ = progress.send((
                100 + 700 * y as u64 / plan.height as u64,
                "Combining scene pixels",
            ));
        }
    }
    if let Some(counter) = vi_counter {
        plan.vi_quality = Some(counter.finish(
            u64::from(plan.width) * u64::from(plan.height),
            plan.covered_pixels,
        ));
        image
            .write_tag(
                Tag::ImageDescription,
                serde_json::to_string(plan.vi_quality.as_ref().unwrap())
                    .map_err(io_error)?
                    .as_str(),
            )
            .map_err(io_error)?;
    }
    image
        .write_tag(Tag::StripOffsets, K::convert_slice(&offsets))
        .map_err(io_error)?;
    image
        .write_tag(Tag::StripByteCounts, K::convert_slice(&byte_counts))
        .map_err(io_error)?;
    image.finish().map_err(io_error)?;
    if let Some(mask) = &mut mask {
        hasher.update(b"internal-1bit:");
        hasher.update(mask.append(encoder, plan.width, plan.height, rows, cancel)?);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn validate_output(
    path: &Path,
    plan: &MosaicPlan,
    pixel_hash: &str,
    cancel: &CancellationToken,
    progress: Option<&UnboundedSender<(u64, &'static str)>>,
) -> Result<()> {
    let mut limits = Limits::default();
    limits.decoding_buffer_size = 64 * 1024 * 1024;
    limits.intermediate_buffer_size = 32 * 1024 * 1024;
    limits.ifd_value_size = 1024 * 1024;
    let mut decoded = Decoder::new(BufReader::new(File::open(path).map_err(io_error)?))
        .map_err(io_error)?
        .with_limits(limits);
    if decoded.dimensions().map_err(io_error)? != (plan.width, plan.height)
        || decoded.colortype().map_err(io_error)?
            != if plan.landsat_quality.is_some() {
                ColorType::Gray(16)
            } else if let Some(profile) = &plan.quality {
                ColorType::Gray(profile.bits)
            } else if crate::raster::srtm::signed(plan.elevation.as_ref()) {
                ColorType::Gray(16)
            } else if plan.elevation.is_some() || plan.radar.is_some() {
                ColorType::Gray(32)
            } else if let Some(profile) = &plan.calibration {
                ColorType::Gray(profile.bits())
            } else if plan.band_count == 1 {
                ColorType::Gray(8)
            } else {
                ColorType::RGB(8)
            }
    {
        return Err("Mosaic output read-back dimensions or bands differ".into());
    }
    if let Some(profile) = &plan.landsat_quality {
        crate::raster::landsat_quality::validate_header(&mut decoded, &profile.band, Some(plan))?;
    } else if let Some(profile) = &plan.quality {
        crate::raster::quality::validate_header(&mut decoded, &profile.band, "", Some(plan))?;
    } else if plan.elevation.is_some() {
        crate::raster::elevation::validate_header(&mut decoded, "", Some(plan))?;
    } else {
        let keys = decoded
            .get_tag_u16_vec(Tag::GeoKeyDirectoryTag)
            .map_err(io_error)?;
        let actual_crs = if matches!(
            plan.crs.as_str(),
            crate::providers::modis::CRS | crate::providers::viirs::hdf::CRS
        ) {
            let crs = crate::raster::reflectance::modis::crs(&mut decoded, &keys)?;
            if plan.crs == crate::providers::viirs::hdf::CRS {
                plan.crs.clone()
            } else {
                crs
            }
        } else {
            crate::raster::validate_projected_geokeys(&keys, plan.aerial.is_some())?
        };
        if actual_crs != plan.crs {
            return Err("Mosaic output CRS read-back differs".into());
        }
        let scale = decoded
            .get_tag_f64_vec(Tag::ModelPixelScaleTag)
            .map_err(io_error)?;
        let tiepoint = decoded
            .get_tag_f64_vec(Tag::ModelTiepointTag)
            .map_err(io_error)?;
        let (bounds, pixel_size) =
            georeference(plan.width, plan.height, None, Some(&scale), Some(&tiepoint))?;
        if bounds != plan.bounds || pixel_size != plan.pixel_size {
            return Err("Mosaic output georeferencing read-back differs".into());
        }
        if plan.radar.is_some() {
            crate::raster::radar::validate_header(&mut decoded, Some(plan))?;
        } else if plan.aerial.is_some() {
            if crate::raster::aerial::validate_layout(&mut decoded)? != 0 {
                return Err("NAIP output must store NIR as an unspecified extra sample, with a separate mask".into());
            }
        } else {
            let nodata = decoded
                .get_tag(Tag::GdalNodata)
                .map_err(io_error)?
                .into_string()
                .map_err(io_error)?;
            if nodata.trim_matches('\0').parse::<i32>().map_err(io_error)?
                != plan.calibration.as_ref().map_or(0, |p| p.nodata)
            {
                return Err("Mosaic output NoData read-back differs".into());
            }
        }
    }
    if let Some(profile) = &plan.calibration {
        crate::raster::reflectance::validate_header(&mut decoded, profile)?;
        let metadata = decoded
            .get_tag(Tag::Unknown(42112))
            .map_err(io_error)?
            .into_string()
            .map_err(io_error)?;
        if !metadata.contains(&format!("role=\"scale\">{}<", profile.scale))
            || !metadata.contains(&format!("role=\"offset\">{}<", profile.offset))
        {
            return Err("Mosaic output calibration read-back differs".into());
        }
    }
    if let Some(result) = &plan.vi_quality {
        let actual = decoded
            .get_tag_ascii_string(Tag::ImageDescription)
            .map_err(io_error)?;
        if serde_json::from_str::<vegetation::SelectionResult>(actual.trim_matches('\0'))
            .map_err(io_error)?
            != *result
        {
            return Err("Vegetation selection read-back metadata differs".into());
        }
    }
    let strips = decoded.strip_count().map_err(io_error)?;
    let mut hasher = Sha256::new();
    let mut samples = 0u64;
    for index in 0..strips {
        check_cancel(Some(cancel))?;
        let readback = if plan.landsat_quality.is_some() {
            landsat::decoded_bytes(decoded.read_chunk(index).map_err(io_error)?)?
        } else if plan.aerial.is_some() {
            crate::raster::aerial::read_chunk(&mut decoded, index)?
        } else {
            decoded_bytes(
                decoded.read_chunk(index).map_err(io_error)?,
                plan.calibration.as_ref(),
                plan.elevation.as_ref(),
                plan.radar.is_some(),
                plan.quality.as_ref(),
            )?
        };
        samples += readback.len() as u64 / sample_bytes(plan) as u64;
        hasher.update(&readback);
        if let Some(progress) = progress {
            let _ = progress.send((
                800 + 150 * (index as u64 + 1) / strips as u64,
                "Writing and checking GeoTIFF",
            ));
        }
    }
    if plan.aerial.is_some() || plan.landsat_quality.is_some() {
        crate::raster::aerial::mask::select(&mut decoded, plan.width, plan.height)?;
        let mut mask_hash = Sha256::new();
        let mut valid = 0u64;
        let stride = plan.width.div_ceil(8) as usize;
        for index in 0..decoded.strip_count().map_err(io_error)? {
            check_cancel(Some(cancel))?;
            let packed = crate::raster::aerial::mask::strip(&mut decoded, index)?;
            for row in packed.chunks_exact(stride) {
                valid += row
                    .iter()
                    .map(|byte| u64::from(byte.count_ones()))
                    .sum::<u64>();
                if !plan.width.is_multiple_of(8)
                    && row[stride - 1] & ((1 << (8 - plan.width % 8)) - 1) != 0
                {
                    return Err("NAIP mask contains nonzero row padding".into());
                }
            }
            mask_hash.update(&packed);
        }
        if valid != plan.covered_pixels {
            return Err("NAIP mask coverage read-back differs".into());
        }
        hasher.update(b"internal-1bit:");
        hasher.update(format!("{:x}", mask_hash.finalize()));
    }
    if samples != plan.width as u64 * plan.height as u64 * plan.band_count as u64
        || format!("{:x}", hasher.finalize()) != pixel_hash
    {
        return Err("Mosaic output pixels failed read-back validation".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aerial_source(root: &Path, x: f64, value: [u8; 4]) -> Job {
        use std::io::Cursor;
        use tiff::encoder::colortype;
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut buffer).unwrap();
            let mut image = encoder.new_image::<colortype::RGBA8>(9, 513).unwrap();
            image
                .encoder()
                .write_tag(Tag::ExtraSamples, &[0u16][..])
                .unwrap();
            image
                .encoder()
                .write_tag(
                    Tag::GeoKeyDirectoryTag,
                    &[
                        1u16, 1, 0, 4, 1024, 0, 1, 1, 1025, 0, 1, 1, 3072, 0, 1, 26910, 3076, 0, 1,
                        9001,
                    ][..],
                )
                .unwrap();
            image
                .encoder()
                .write_tag(Tag::ModelPixelScaleTag, &[0.6f64, 0.6, 0.0][..])
                .unwrap();
            image
                .encoder()
                .write_tag(
                    Tag::ModelTiepointTag,
                    &[0.0f64, 0.0, 0.0, x, 4178430.0, 0.0][..],
                )
                .unwrap();
            image.write_data(&value.repeat(9 * 513)).unwrap();
        }
        let mut source = crate::raster::tests::record(root, &buffer.into_inner());
        source.asset_key = "aerial".into();
        source.item_id = "ca_m_3712221_nw_10_060_20220518".into();
        source.href = "https://naipeuwest.blob.core.windows.net/naip/v002/ca/2022/ca_060cm_2022/37122/m_3712221_nw_10_060_20220518.tif".into();
        source
    }

    #[test]
    fn aerial_stream_preserves_nir_zero_black_overlap_gaps_and_packed_mask_rows() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let sources = [
            aerial_source(&root, 543846.0, [0, 0, 0, 0]),
            aerial_source(&root, 543847.2, [3, 2, 1, 0]),
            aerial_source(&root, 543853.8, [4, 3, 2, 5]),
        ];
        let cancel = CancellationToken::new();
        let mut rasters = sources
            .iter()
            .map(|s| source_raster(&root, s, "aerial", &cancel).unwrap())
            .collect::<Vec<_>>();
        let plan = MosaicPlan {
            width: 22,
            height: 513,
            band_count: 4,
            crs: "EPSG:26910".into(),
            bounds: [
                543846.0,
                4178430.0 - 513.0 * 0.6,
                543846.0 + 22.0 * 0.6,
                4178430.0,
            ],
            pixel_size: [0.6, 0.6],
            source_count: 3,
            masked_pixels: 0,
            covered_pixels: 0,
            overlap_policy: "test".into(),
            calibration: None,
            elevation: None,
            aerial: Some(crate::raster::aerial::masked_display()),
            radar: None,
            quality: None,

            landsat_quality: None,
            vi_quality: None,
            vi_index: None,
        };
        let id = Uuid::new_v4().to_string();
        let mut big_plan = plan.clone();
        let output = encode_mosaic(&root, &id, plan, &mut rasters, None, &cancel, None).unwrap();
        assert_eq!(output.plan.covered_pixels, 20 * 513);
        let mut big_rasters = sources
            .iter()
            .map(|s| source_raster(&root, s, "aerial", &cancel).unwrap())
            .collect::<Vec<_>>();
        let mut big_file = tempfile::NamedTempFile::new_in(root.join("assets")).unwrap();
        let big_hash = {
            let mut encoder = TiffEncoder::new_big(big_file.as_file_mut()).unwrap();
            encode_strips(
                &mut encoder,
                &mut big_plan,
                &mut big_rasters,
                None,
                &cancel,
                None,
                &root.join("assets"),
            )
            .unwrap()
        };
        validate_output(big_file.path(), &big_plan, &big_hash, &cancel, None).unwrap();
        assert_eq!(big_plan.covered_pixels, 20 * 513);
        let mut result = sources[0].clone();
        result.id = id;
        result.kind = "raster_mosaic".into();
        result.output_path = Some(output.path);
        result.sha256 = Some(output.sha256);
        result.bytes_downloaded = output.bytes;
        result.total_bytes = Some(output.bytes);
        result.mosaic = Some(MosaicSpec {
            project_id: Uuid::new_v4().to_string(),
            asset_key: "aerial".into(),
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
        result.mosaic_output = Some(output.plan);
        for row in [0, 511, 512] {
            for (col, rgb, nir, invalid) in [
                (0, [0, 0, 0], 0, false),
                (2, [3, 2, 1], 0, false),
                (10, [3, 2, 1], 0, false),
                (11, [0, 0, 0], 0, true),
                (12, [0, 0, 0], 0, true),
                (13, [4, 3, 2], 5, false),
                (21, [4, 3, 2], 5, false),
            ] {
                let pixel = crate::raster::rgb::sample(
                    &root,
                    &result,
                    543846.0 + (f64::from(col) + 0.5) * 0.6,
                    4178430.0 - (f64::from(row) + 0.5) * 0.6,
                )
                .unwrap();
                assert_eq!(pixel.values, Some(rgb));
                assert_eq!(pixel.near_infrared, Some(nir));
                assert_eq!(pixel.is_no_data, invalid);
            }
        }
        let inspection = crate::raster::rgb::inspect_with_edge(&root, &result, 768).unwrap();
        assert_eq!(
            inspection.aerial.unwrap().coverage_mask.as_deref(),
            Some("internal-1bit")
        );
        let bytes = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            inspection.preview_data_url.split(',').nth(1).unwrap(),
        )
        .unwrap();
        let mut png = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .unwrap();
        let mut rgba = vec![0; png.output_buffer_size().unwrap()];
        png.next_frame(&mut rgba).unwrap();
        assert_eq!(rgba[3], 255);
        assert_eq!(rgba[11 * 4 + 3], 0);
        assert_eq!(rgba[13 * 4 + 3], 255);
        for (view, covered) in [
            (crate::raster::AerialView::Cir, [5, 4, 3, 255]),
            (crate::raster::AerialView::Nir, [5, 5, 5, 255]),
        ] {
            let inspection =
                crate::raster::rgb::inspect_with_view(&root, &result, 768, view).unwrap();
            assert_eq!(
                inspection.aerial.unwrap().coverage_mask.as_deref(),
                Some("internal-1bit")
            );
            let bytes = base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                inspection.preview_data_url.split(',').nth(1).unwrap(),
            )
            .unwrap();
            let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes))
                .read_info()
                .unwrap();
            let mut rgba = vec![0; decoder.output_buffer_size().unwrap()];
            decoder.next_frame(&mut rgba).unwrap();
            assert_eq!(&rgba[..4], &[0, 0, 0, 255]); // Valid black with zero NIR.
            assert_eq!(&rgba[11 * 4..12 * 4], &[0, 0, 0, 0]); // Coverage gap.
            assert_eq!(&rgba[13 * 4..14 * 4], &covered);
        }
        assert_eq!(
            result
                .mosaic_output
                .as_ref()
                .unwrap()
                .aerial
                .as_ref()
                .unwrap()
                .display_bands,
            [1, 2, 3]
        );
        result.mosaic_output.as_mut().unwrap().width += 1;
        assert!(crate::raster::rgb::inspect_with_edge(&root, &result, 768).is_err());
        assert!(!std::fs::read_dir(root.join("assets")).unwrap().any(|e| e
            .unwrap()
            .path()
            .extension()
            .is_some_and(|e| e == "part")));
    }

    #[test]
    fn streamed_bigtiff_is_readable_with_exact_pixels_and_georeferencing() {
        let directory = tempfile::tempdir().unwrap();
        let source = crate::raster::tests::record(
            directory.path(),
            &crate::raster::tests::fixture(2, 2, &[1, 2, 3, 4], 32610, false),
        );
        let root = directory.path().canonicalize().unwrap();
        let cancel = CancellationToken::new();
        let raster = source_raster(&root, &source, "scl", &cancel).unwrap();
        let mut plan = MosaicPlan {
            width: raster.width,
            height: raster.height,
            band_count: 1,
            crs: raster.crs.clone(),
            bounds: raster.bounds,
            pixel_size: raster.pixel_size,
            source_count: 1,
            covered_pixels: 0,
            masked_pixels: 0,
            overlap_policy: "test".into(),
            calibration: None,
            elevation: None,
            aerial: None,
            radar: None,
            quality: None,

            landsat_quality: None,
            vi_quality: None,
            vi_index: None,
        };
        let mut file = tempfile::NamedTempFile::new_in(&root).unwrap();
        let hash = {
            let mut encoder = TiffEncoder::new_big(file.as_file_mut()).unwrap();
            encode_strips(
                &mut encoder,
                &mut plan,
                &mut [raster],
                None,
                &cancel,
                None,
                &root,
            )
            .unwrap()
        };
        validate_output(file.path(), &plan, &hash, &cancel, None).unwrap();
        assert_eq!(plan.covered_pixels, 4);
        let bytes = std::fs::read(file.path()).unwrap();
        assert_eq!(&bytes[..4], &[b'I', b'I', 43, 0]);
        let mut decoder = Decoder::new(std::io::Cursor::new(bytes)).unwrap();
        assert_eq!(
            decoder.read_image().unwrap().as_buffer(0).as_bytes(),
            &[1, 2, 3, 4]
        );
    }
}
