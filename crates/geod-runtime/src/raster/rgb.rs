//! Bounded display and exact source-pixel inspection of managed UInt8 RGB files.
use super::*;
use crate::{
    io_error,
    mosaic::{source_raster, SourceRaster},
    thumbnail::sample_preview_with_channels,
};
use tiff::decoder::ChunkType;

fn source(root: &Path, job: &Job) -> Result<SourceRaster> {
    if job.status != JobStatus::Succeeded
        || !matches!(job.asset_key.as_str(), "visual" | "aerial")
        || uuid::Uuid::parse_str(&job.id)
            .ok()
            .map(|id| id.to_string())
            .as_deref()
            != Some(&job.id)
        || extension(&job.media_type)? != "tif"
    {
        return Err("RGB inspection requires a completed managed true-color GeoTIFF".into());
    }
    source_raster(root, job, &job.asset_key, &CancellationToken::new())
}

pub(super) fn inspect(root: &Path, job: &Job) -> Result<RasterInspection> {
    inspect_with_edge(root, job, PREVIEW_EDGE)
}
pub(crate) fn inspect_with_edge(root: &Path, job: &Job, edge: u32) -> Result<RasterInspection> {
    inspect_with_view(root, job, edge, aerial::AerialView::Rgb)
}

pub(crate) fn inspect_with_view(
    root: &Path,
    job: &Job,
    edge: u32,
    view: aerial::AerialView,
) -> Result<RasterInspection> {
    if job.asset_key != "aerial" && view != aerial::AerialView::Rgb {
        return Err("NIR display requires a managed NAIP RGB + NIR raster".into());
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    let raster = source(root, job)?;
    let mut decoder = raster.decoder.into_tiff()?;
    let mut aerial = if job.asset_key == "aerial" {
        Some(crate::raster::aerial::inspection_display(
            &mut decoder,
            job,
        )?)
    } else {
        None
    };
    if let Some(info) = aerial.as_mut() {
        info.display_bands = view.display_bands();
    }
    let mut dimensions = [raster.width, raster.height];
    // Display may use an embedded overview. Grid metadata and pixel sampling
    // always refer to the original image, never to overview pixel values.
    let mut best = 0;
    let mut best_edge = raster.width.max(raster.height);
    for index in 1..=16 {
        if !decoder.more_images() {
            break;
        }
        check_time(deadline)?;
        decoder.next_image().map_err(io_error)?;
        let (width, height) = decoder.dimensions().map_err(io_error)?;
        if width > 0
            && height > 0
            && width <= raster.width
            && height <= raster.height
            && width.max(height) >= edge
            && width.max(height) < best_edge
            && (if job.asset_key == "aerial" {
                crate::raster::aerial::validate_samples(&mut decoder, job).is_ok()
            } else {
                decoder.colortype().map_err(io_error)? == ColorType::RGB(8)
            })
        {
            best = index;
            best_edge = width.max(height);
            dimensions = [width, height];
        }
    }
    decoder.seek_to_image(best).map_err(io_error)?;
    let (preview_width, preview_height, mut rgba) = sample_preview_with_channels(
        &mut decoder,
        dimensions,
        false,
        raster.nodata.map(|value| value as u8),
        edge,
        deadline,
        view.channels(),
    )?;
    if job.asset_key == "aerial" && job.kind == "raster_mosaic" {
        crate::raster::aerial::mask::apply_preview(
            &mut decoder,
            [raster.width, raster.height],
            [preview_width, preview_height],
            &mut rgba,
            deadline,
        )?;
    }
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, preview_width, preview_height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(io_error)?
        .write_image_data(&rgba)
        .map_err(io_error)?;
    check_time(deadline)?;
    Ok(RasterInspection {
        science: None,
        vegetation: None,
        quality: None,
        radar: None,
        width: raster.width,
        height: raster.height,
        band_count: raster.bands as u8,
        data_type: "UInt8".into(),
        crs: raster.crs,
        bounds: raster.bounds,
        pixel_size: raster.pixel_size,
        nodata: raster.nodata,
        preview_data_url: format!("data:image/png;base64,{}", STANDARD.encode(bytes)),
        preview_width,
        preview_height,
        classes: Vec::new(),
        sha256: job.sha256.clone().ok_or("RGB source has no checksum")?,
        reflectance: None,
        elevation: None,
        aerial,
    })
}

pub(crate) fn sample(root: &Path, job: &Job, x: f64, y: f64) -> Result<RasterPixel> {
    let raster = source(root, job)?;
    let mut decoder = raster.decoder.into_tiff()?;
    let [left, bottom, right, top] = raster.bounds;
    if !x.is_finite() || !y.is_finite() || x < left || x >= right || y <= bottom || y > top {
        return Err("The coordinate is outside the source raster pixel grid".into());
    }
    let column = ((x - left) / raster.pixel_size[0]).floor() as u32;
    let row = ((top - y) / raster.pixel_size[1]).floor() as u32;
    let (width, height) = decoder.chunk_dimensions();
    if width == 0 || height == 0 {
        return Err("RGB source chunks are invalid".into());
    }
    let columns = raster.width.div_ceil(width);
    let count = match decoder.get_chunk_type() {
        ChunkType::Tile => decoder.tile_count().map_err(io_error)?,
        ChunkType::Strip => decoder.strip_count().map_err(io_error)?,
    };
    if columns.checked_mul(raster.height.div_ceil(height)) != Some(count) {
        return Err("Planar RGB chunks are unsupported".into());
    }
    let chunk = row / height * columns + column / width;
    let (actual_width, actual_height) = decoder.chunk_data_dimensions(chunk);
    let px = column % width;
    let py = row % height;
    if px >= actual_width || py >= actual_height {
        return Err("RGB sample is outside its chunk".into());
    }
    let data = if raster.bands == 4 {
        crate::raster::aerial::read_chunk(&mut decoder, chunk)?
    } else {
        match decoder.read_chunk(chunk).map_err(io_error)? {
            DecodingResult::U8(values) => values,
            _ => return Err("RGB samples are not unsigned 8-bit values".into()),
        }
    };
    let offset = (py as usize * actual_width as usize + px as usize) * raster.bands;
    let near_infrared = if raster.bands == 4 {
        Some(*data.get(offset + 3).ok_or("NIR sample is missing")?)
    } else {
        None
    };
    let values: [u8; 3] = data
        .get(offset..offset + 3)
        .ok_or("RGB chunk has too few samples")?
        .try_into()
        .map_err(io_error)?;
    let is_no_data = if raster.bands == 4 && job.kind == "raster_mosaic" {
        !crate::raster::aerial::mask::sample(
            &mut decoder,
            [raster.width, raster.height],
            [column, row],
        )?
    } else {
        raster
            .nodata
            .is_some_and(|nodata| values.iter().all(|value| f64::from(*value) == nodata))
    };
    Ok(RasterPixel {
        science: None,
        index_value: None,
        quality: None,
        decibels: None,
        job_id: job.id.clone(),
        sha256: job.sha256.clone().ok_or("RGB source has no checksum")?,
        crs: raster.crs,
        coordinate: [x, y],
        pixel: [column, row],
        center: [
            left + (column as f64 + 0.5) * raster.pixel_size[0],
            top - (row as f64 + 0.5) * raster.pixel_size[1],
        ],
        value: f64::from(values[0]),
        values: Some(values),
        near_infrared,
        reflectance: None,
        label: if raster.bands == 4 {
            "RGB + NIR"
        } else {
            "RGB"
        }
        .into(),
        color: format!("#{:02x}{:02x}{:02x}", values[0], values[1], values[2]),
        is_no_data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiff::encoder::{colortype, TiffEncoder};

    #[test]
    fn rgb_inspection_uses_real_channels_and_rejects_changed_source() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut bytes).unwrap();
            let mut image = encoder.new_image::<colortype::RGB8>(2, 2).unwrap();
            image
                .encoder()
                .write_tag(
                    Tag::GeoKeyDirectoryTag,
                    &[
                        1u16, 1, 0, 4, 1024, 0, 1, 1, 1025, 0, 1, 1, 3072, 0, 1, 32610, 3076, 0, 1,
                        9001,
                    ][..],
                )
                .unwrap();
            image
                .encoder()
                .write_tag(Tag::ModelPixelScaleTag, &[20.0f64, 20.0, 0.0][..])
                .unwrap();
            image
                .encoder()
                .write_tag(
                    Tag::ModelTiepointTag,
                    &[0.0f64, 0.0, 0.0, 500000.0, 4200000.0, 0.0][..],
                )
                .unwrap();
            image.encoder().write_tag(Tag::GdalNodata, "0").unwrap();
            image
                .write_data(&[25, 50, 75, 0, 0, 0, 1, 2, 3, 100, 110, 120])
                .unwrap();
        }
        let mut job = crate::raster::tests::record(&root, &bytes.into_inner());
        job.asset_key = "visual".into();
        let metadata = inspect(&root, &job).unwrap();
        assert_eq!(
            (metadata.width, metadata.height, metadata.band_count),
            (2, 2, 3)
        );
        assert!(metadata.classes.is_empty());
        assert_eq!(metadata.bounds, [500000.0, 4199960.0, 500040.0, 4200000.0]);
        let pixel = sample(&root, &job, 500001.0, 4199999.0).unwrap();
        assert_eq!(pixel.values, Some([25, 50, 75]));
        assert_eq!(pixel.color, "#19324b");
        assert!(!pixel.is_no_data);
        assert!(sample(&root, &job, 500021.0, 4199999.0).unwrap().is_no_data);
        assert_eq!(
            sample(&root, &job, 500021.0, 4199979.0).unwrap().values,
            Some([100, 110, 120])
        );
        assert!(sample(&root, &job, 500040.0, 4199999.0).is_err());
        job.sha256 = Some("0".repeat(64));
        assert!(inspect(&root, &job).unwrap_err().contains("SHA-256"));
        assert!(sample(&root, &job, 500001.0, 4199999.0)
            .unwrap_err()
            .contains("SHA-256"));
    }
}
