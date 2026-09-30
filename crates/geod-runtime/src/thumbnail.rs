//! Small previews from managed, checksum-verified local GeoTIFFs.
//! RGB previews may use a file's own overview; SCL always samples original classes.
use crate::{io_error, mosaic::source_raster, raster::PALETTE, Job, JobManager, JobStatus, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    io::{Read, Seek},
    path::Path,
    time::{Duration, Instant},
};
use tiff::{
    decoder::{ChunkType, Decoder, DecodingResult},
    ColorType,
};
use tokio_util::sync::CancellationToken;

const EDGE: u32 = 160;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileThumbnail {
    pub job_id: String,
    pub sha256: String,
    pub width: u32,
    pub height: u32,
    pub data_url: String,
}

impl JobManager {
    pub async fn file_thumbnail(&self, id: &str) -> Result<FileThumbnail> {
        let job = self.get(id).await.ok_or("Unknown job")?;
        if job.status != JobStatus::Succeeded
            || !matches!(job.asset_key.as_str(), "scl" | "visual")
            || uuid::Uuid::parse_str(id)
                .ok()
                .map(|value| value.to_string())
                .as_deref()
                != Some(id)
        {
            return Err("Preview requires a completed managed SCL or RGB GeoTIFF".into());
        }
        let root = self.inner.root.clone();
        // Independent from interactive pixel/clip workers; one bounded preview at a time.
        let permit = self
            .inner
            .thumbnail_permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| "Preview worker is busy. Try again shortly.")?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            thumbnail(&root, &job)
        })
        .await
        .map_err(io_error)?
    }
}

fn thumbnail(root: &Path, job: &Job) -> Result<FileThumbnail> {
    let deadline = Instant::now() + Duration::from_secs(60);
    let raster = source_raster(root, job, &job.asset_key, &CancellationToken::new())?;
    let mut decoder = raster.decoder;
    let mut width = raster.width;
    let mut height = raster.height;
    if job.asset_key == "visual" {
        let mut best = 0;
        let mut best_edge = width.max(height);
        for index in 1..=16 {
            if !decoder.more_images() {
                break;
            }
            decoder.next_image().map_err(io_error)?;
            let (w, h) = decoder.dimensions().map_err(io_error)?;
            if w > 0
                && h > 0
                && w <= width
                && h <= height
                && w.max(h) >= EDGE
                && w.max(h) < best_edge
                && decoder.colortype().map_err(io_error)? == ColorType::RGB(8)
            {
                best = index;
                best_edge = w.max(h);
            }
        }
        decoder.seek_to_image(best).map_err(io_error)?;
        (width, height) = decoder.dimensions().map_err(io_error)?;
    }
    let (preview_width, preview_height, rgba) = sample_preview(
        &mut decoder,
        width,
        height,
        job.asset_key == "scl",
        raster.nodata,
        deadline,
    )?;
    let mut png_bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, preview_width, preview_height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .map_err(io_error)?
            .write_image_data(&rgba)
            .map_err(io_error)?;
    }
    Ok(FileThumbnail {
        job_id: job.id.clone(),
        sha256: job.sha256.clone().ok_or("Preview has no source checksum")?,
        width: preview_width,
        height: preview_height,
        data_url: format!("data:image/png;base64,{}", STANDARD.encode(png_bytes)),
    })
}

fn sample_preview<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    width: u32,
    height: u32,
    scl: bool,
    nodata: Option<u8>,
    deadline: Instant,
) -> Result<(u32, u32, Vec<u8>)> {
    sample_preview_with_edge(decoder, [width, height], scl, nodata, EDGE, deadline)
}

pub(crate) fn sample_preview_with_edge<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    dimensions: [u32; 2],
    scl: bool,
    nodata: Option<u8>,
    edge: u32,
    deadline: Instant,
) -> Result<(u32, u32, Vec<u8>)> {
    let [width, height] = dimensions;
    if width == 0 || height == 0 || width > 20000 || height > 20000 {
        return Err("Preview raster dimensions are unsupported".into());
    }
    let longest = width.max(height);
    let pw = (width as u64 * edge.min(longest) as u64 / longest as u64).max(1) as u32;
    let ph = (height as u64 * edge.min(longest) as u64 / longest as u64).max(1) as u32;
    let (cw, ch) = decoder.chunk_dimensions();
    if cw == 0 || ch == 0 {
        return Err("Preview chunks are invalid".into());
    }
    let cols = width.div_ceil(cw);
    let count = match decoder.get_chunk_type() {
        ChunkType::Tile => decoder.tile_count().map_err(io_error)?,
        ChunkType::Strip => decoder.strip_count().map_err(io_error)?,
    };
    if cols.checked_mul(height.div_ceil(ch)) != Some(count) {
        return Err("Planar preview chunks are unsupported".into());
    }
    let bands = if scl { 1 } else { 3 };
    let mut samples: BTreeMap<u32, Vec<(usize, u32, u32)>> = BTreeMap::new();
    for y in 0..ph {
        for x in 0..pw {
            let sx = x as u64 * width as u64 / pw as u64;
            let sy = y as u64 * height as u64 / ph as u64;
            let chunk = sy as u32 / ch * cols + sx as u32 / cw;
            samples.entry(chunk).or_default().push((
                (y * pw + x) as usize,
                sx as u32 % cw,
                sy as u32 % ch,
            ));
        }
    }
    let mut rgba = vec![0; (pw * ph * 4) as usize];
    for (chunk, targets) in samples {
        if Instant::now() > deadline {
            return Err("Local preview exceeded its time limit".into());
        }
        let (actual_w, actual_h) = decoder.chunk_data_dimensions(chunk);
        let values = match decoder.read_chunk(chunk).map_err(io_error)? {
            DecodingResult::U8(values) => values,
            _ => return Err("Preview only supports UInt8 pixels".into()),
        };
        for (target, x, y) in targets {
            if x >= actual_w || y >= actual_h {
                return Err("Preview sample is outside its chunk".into());
            }
            let offset = (y as usize * actual_w as usize + x as usize) * bands;
            let sample = values
                .get(offset..offset + bands)
                .ok_or("Preview chunk has too few samples")?;
            let color = if scl {
                PALETTE
                    .get(sample[0] as usize)
                    .ok_or("Invalid SCL class in preview")?
                    .1
            } else {
                [sample[0], sample[1], sample[2]]
            };
            rgba[target * 4..target * 4 + 3].copy_from_slice(&color);
            rgba[target * 4 + 3] =
                if nodata.is_some_and(|value| sample.iter().all(|sample| *sample == value)) {
                    0
                } else {
                    255
                };
        }
    }
    Ok((pw, ph, rgba))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tiff::encoder::{colortype, TiffEncoder};

    #[test]
    fn managed_preview_rejects_changed_hash_and_unmanaged_paths() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let bytes = crate::raster::tests::fixture(2, 1, &[4, 6], 32610, false);
        let mut job = crate::raster::tests::record(&root, &bytes);
        let data = thumbnail(&root, &job).unwrap();
        assert_eq!((data.width, data.height), (2, 1));
        assert_eq!(data.job_id, job.id);
        assert_eq!(Some(&data.sha256), job.sha256.as_ref());
        job.sha256 = Some("0".repeat(64));
        assert!(thumbnail(&root, &job).unwrap_err().contains("SHA-256"));
        let outside = root.join("unmanaged.tif");
        std::fs::write(&outside, bytes).unwrap();
        job.output_path = Some(outside.to_string_lossy().into_owned());
        assert!(thumbnail(&root, &job)
            .unwrap_err()
            .contains("managed GeoTIFF"));
    }

    #[test]
    fn preview_is_bounded_and_preserves_non_square_aspect_ratio() {
        let mut bytes = Cursor::new(Vec::new());
        TiffEncoder::new(&mut bytes)
            .unwrap()
            .write_image::<colortype::Gray8>(321, 81, &vec![4; 321 * 81])
            .unwrap();
        let mut decoder = Decoder::new(Cursor::new(bytes.into_inner())).unwrap();
        let (w, h, rgba) = sample_preview(
            &mut decoder,
            321,
            81,
            true,
            Some(0),
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!((w, h), (160, 40));
        assert!(rgba.chunks_exact(4).all(|pixel| pixel == [0, 160, 0, 255]));
    }

    #[test]
    fn rgb_preview_preserves_pixels_and_transparent_nodata() {
        let mut bytes = Cursor::new(Vec::new());
        TiffEncoder::new(&mut bytes)
            .unwrap()
            .write_image::<colortype::RGB8>(2, 1, &[25, 50, 75, 0, 0, 0])
            .unwrap();
        let mut decoder = Decoder::new(Cursor::new(bytes.into_inner())).unwrap();
        let (w, h, pixels) = sample_preview(
            &mut decoder,
            2,
            1,
            false,
            Some(0),
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!((w, h), (2, 1));
        assert_eq!(pixels, [25, 50, 75, 255, 0, 0, 0, 0]);
    }

    #[test]
    fn scl_preview_uses_class_palette_and_rejects_unknown_values() {
        for (value, expected) in [
            (4, Some([0, 160, 0, 255])),
            (6, Some([0, 0, 255, 255])),
            (12, None),
        ] {
            let mut bytes = Cursor::new(Vec::new());
            TiffEncoder::new(&mut bytes)
                .unwrap()
                .write_image::<colortype::Gray8>(1, 1, &[value])
                .unwrap();
            let mut decoder = Decoder::new(Cursor::new(bytes.into_inner())).unwrap();
            let result = sample_preview(
                &mut decoder,
                1,
                1,
                true,
                Some(0),
                Instant::now() + Duration::from_secs(1),
            );
            if let Some(expected) = expected {
                assert_eq!(result.unwrap().2, expected);
            } else {
                assert!(result.is_err());
            }
        }
    }

    #[test]
    fn valid_black_rgb_pixels_remain_opaque_without_nodata() {
        let mut bytes = Cursor::new(Vec::new());
        TiffEncoder::new(&mut bytes)
            .unwrap()
            .write_image::<colortype::RGB8>(1, 1, &[0, 0, 0])
            .unwrap();
        let mut decoder = Decoder::new(Cursor::new(bytes.into_inner())).unwrap();
        let (_, _, pixels) = sample_preview(
            &mut decoder,
            1,
            1,
            false,
            None,
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(pixels, [0, 0, 0, 255]);
    }
}
