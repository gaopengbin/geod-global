//! Pixel-aligned unsigned QA copying. Coverage is independent of saturation DN.
use super::*;
use crate::raster::landsat_quality as qa;

pub(super) fn validate_plan(key: &str, plan: &MosaicPlan) -> Result<()> {
    if qa::KEYS.contains(&key) {
        if plan.landsat_quality.as_ref() != Some(&qa::processing_profile(key)?)
            || plan.band_count != 1
            || plan.calibration.is_some()
            || plan.elevation.is_some()
            || plan.aerial.is_some()
            || plan.radar.is_some()
            || plan.quality.is_some()
            || plan.width == 0
            || plan.height == 0
            || plan.source_count == 0
            || plan.source_count > crate::projects::MAX_PROJECT_SCENES
            || plan.bounds.iter().any(|v| !v.is_finite())
            || ((plan.bounds[2] - plan.bounds[0]) - f64::from(plan.width) * 30.0).abs() > 1e-6
            || ((plan.bounds[3] - plan.bounds[1]) - f64::from(plan.height) * 30.0).abs() > 1e-6
            || plan
                .pixel_size
                .iter()
                .any(|v| !v.is_finite() || (*v - 30.0).abs() > 1e-6)
            || plan.covered_pixels > u64::from(plan.width) * u64::from(plan.height)
            || plan.masked_pixels > u64::from(plan.width) * u64::from(plan.height)
            || plan.covered_pixels + plan.masked_pixels
                > u64::from(plan.width) * u64::from(plan.height)
            || plan.overlap_policy != qa::MOSAIC_POLICY
        {
            return Err("Stored Landsat quality mosaic has a different profile or grid".into());
        }
    } else if plan.landsat_quality.is_some() {
        return Err("Landsat quality cannot be mixed with another raster profile".into());
    }
    Ok(())
}

pub(super) fn validate_pair(saturation: &Job, pixel: &Job) -> Result<()> {
    qa::validate_job(saturation)?;
    qa::validate_job(pixel)?;
    if saturation.kind != "download"
        || pixel.kind != "download"
        || saturation.asset_key != "qa_radsat"
        || pixel.asset_key != "qa_pixel"
        || saturation.item_id != pixel.item_id
        || saturation.href.rsplit_once('/').map(|v| v.0) != pixel.href.rsplit_once('/').map(|v| v.0)
    {
        return Err("Saturation coverage requires matching original QA_PIXEL from the same Landsat processing directory".into());
    }
    Ok(())
}

pub(super) fn validate_grid_pair(source: &SourceRaster, pixel: &SourceRaster) -> Result<()> {
    if source.width != pixel.width
        || source.height != pixel.height
        || source.crs != pixel.crs
        || source
            .bounds
            .iter()
            .zip(pixel.bounds)
            .any(|(a, b)| (*a - b).abs() > 1e-6)
        || source.pixel_size != pixel.pixel_size
    {
        return Err("Landsat saturation and pixel-quality grids differ".into());
    }
    Ok(())
}

fn window(
    source: &mut SourceRaster,
    extent: [u32; 4],
    cancel: &CancellationToken,
    deadline: Instant,
) -> Result<Vec<u16>> {
    let [x, y, width, height] = extent;
    if width == 0
        || height == 0
        || x.checked_add(width).is_none_or(|end| end > source.width)
        || y.checked_add(height).is_none_or(|end| end > source.height)
    {
        return Err("Landsat quality source window is invalid".into());
    }
    let (cw, ch) = source.decoder.chunk_dimensions();
    if cw == 0 || ch == 0 {
        return Err("Invalid Landsat quality chunk grid".into());
    }
    let columns = source.width.div_ceil(cw);
    let chunks = match source.decoder.get_chunk_type() {
        ChunkType::Tile => source.decoder.tile_count()?,
        ChunkType::Strip => source.decoder.strip_count()?,
    };
    if columns.checked_mul(source.height.div_ceil(ch)) != Some(chunks) {
        return Err("Invalid Landsat quality chunk count".into());
    }
    let mut output = vec![0; width as usize * height as usize];
    for cy in y / ch..(y + height).div_ceil(ch) {
        for cx in x / cw..(x + width).div_ceil(cw) {
            check_cancel(Some(cancel))?;
            if Instant::now() > deadline {
                return Err("Landsat quality block timed out".into());
            }
            let index = cy * columns + cx;
            let (aw, ah) = source.decoder.chunk_data_dimensions(index);
            let data = match source.decoder.read_chunk(index)? {
                DecodingResult::U16(data) => data,
                _ => return Err("Landsat quality chunks must remain unsigned UInt16".into()),
            };
            if data.len() != aw as usize * ah as usize {
                return Err("Incomplete Landsat quality chunk".into());
            }
            let left = x.max(cx * cw);
            let right = (x + width).min(cx * cw + aw);
            let top = y.max(cy * ch);
            let bottom = (y + height).min(cy * ch + ah);
            for row in top..bottom {
                check_cancel(Some(cancel))?;
                let start = ((row - cy * ch) * aw + left - cx * cw) as usize;
                let target = ((row - y) * width + left - x) as usize;
                let count = (right - left) as usize;
                output[target..target + count].copy_from_slice(&data[start..start + count]);
            }
        }
    }
    Ok(output)
}

pub(super) fn copy_source(
    source: &mut SourceRaster,
    grid: &MosaicGrid,
    output: &mut [u8],
    covered: &mut [bool],
    cancel: &CancellationToken,
    deadline: Instant,
) -> Result<()> {
    let ox = aligned((source.bounds[0] - grid.origin[0]) / grid.pixel_size[0])?;
    let oy = aligned((grid.origin[1] - source.bounds[3]) / grid.pixel_size[1])?;
    let [width, height] = grid.dimensions;
    let left = ox.max(0);
    let right = (ox + i64::from(source.width)).min(i64::from(width));
    let top = oy.max(0);
    let bottom = (oy + i64::from(source.height)).min(i64::from(height));
    if right <= left || bottom <= top {
        return Ok(());
    }
    let extent = [
        (left - ox) as u32,
        (top - oy) as u32,
        (right - left) as u32,
        (bottom - top) as u32,
    ];
    let data = window(source, extent, cancel, deadline)?;
    let validity = if source
        .landsat_quality
        .as_ref()
        .is_some_and(|p| p.band == "qa_radsat")
    {
        let paired = source
            .coverage_source
            .as_mut()
            .ok_or("Matching QA_PIXEL coverage is missing")?;
        Some(window(paired, extent, cancel, deadline)?)
    } else {
        None
    };
    for row in 0..extent[3] {
        check_cancel(Some(cancel))?;
        for col in 0..extent[2] {
            let index = (row * extent[2] + col) as usize;
            let valid = validity.as_ref().map_or(data[index], |v| v[index]) & 1 == 0;
            let target =
                (top as usize + row as usize) * width as usize + left as usize + col as usize;
            // Preserve a filled source's complete bit field when there is no
            // valid older sample. An invalid newer scene never erases valid data.
            if valid || !covered[target] {
                output[target * 2..target * 2 + 2].copy_from_slice(&data[index].to_ne_bytes());
                covered[target] = valid;
            }
        }
    }
    Ok(())
}

pub(super) fn decoded_bytes(result: DecodingResult) -> Result<Vec<u8>> {
    match result {
        DecodingResult::U16(data) => Ok(data.into_iter().flat_map(u16::to_ne_bytes).collect()),
        _ => Err("Landsat quality output must retain UInt16 flags".into()),
    }
}
