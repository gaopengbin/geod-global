//! Original SRTMGL1N bytes are categorical source codes, never heights.
use super::*;
use crate::io_error;
use crate::providers::srtm;
const SPACING: f64 = 1.0 / 3600.0;

pub(crate) fn validate_job(job: &Job) -> Result<()> {
    if job.asset_key != "srtm_num" || job.kind != "download" {
        return Err("NUM inspection requires a completed original SRTMGL1N ZIP".into());
    }
    super::srtm::validate_job(job)
}

fn read(root: &Path, job: &Job) -> Result<Vec<u8>> {
    validate_job(job)?;
    super::srtm::open_original(root, job, None).map(|(_, raw)| raw)
}

pub(crate) fn inspect(root: &Path, job: &Job, edge: u32) -> Result<RasterInspection> {
    if edge == 0 || edge > PREVIEW_EDGE {
        return Err("Invalid NUM preview size".into());
    }
    let raw = read(root, job)?;
    let mut counts = [0_u64; 256];
    for code in &raw {
        counts[usize::from(*code)] += 1;
    }
    let mut rgba = Vec::with_capacity((edge * edge * 4) as usize);
    for y in 0..edge {
        for x in 0..edge {
            let code = raw[((y * srtm::EDGE / edge) * srtm::EDGE + x * srtm::EDGE / edge) as usize];
            // Exact code gray, nearest original sample. No continuous terrain ramp.
            rgba.extend_from_slice(&[code, code, code, 255]);
        }
    }
    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, edge, edge);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(io_error)?
        .write_image_data(&rgba)
        .map_err(io_error)?;
    Ok(RasterInspection {
        science: None,
        vegetation: None,
        quality: None,
        radar: None,
        width: srtm::EDGE,
        height: srtm::EDGE,
        band_count: 1,
        data_type: "UInt8".into(),
        crs: "EPSG:4326".into(),
        bounds: super::srtm::bounds(&job.item_id)?,
        pixel_size: [SPACING; 2],
        // No scalar NoData declaration exists in the byte original. Do not
        // discard 0 or 255 based on assumptions about a source-code legend.
        nodata: None,
        preview_data_url: format!("data:image/png;base64,{}", STANDARD.encode(png)),
        preview_width: edge,
        preview_height: edge,
        classes: counts
            .iter()
            .enumerate()
            .filter(|(_, count)| **count > 0)
            .map(|(code, count)| RasterClass {
                value: code as u8,
                label: "SRTM NUM source code".into(),
                color: format!("#{code:02x}{code:02x}{code:02x}"),
                count: *count,
            })
            .collect(),
        sha256: job.sha256.clone().ok_or("Missing NUM original checksum")?,
        reflectance: None,
        aerial: None,
        elevation: None,
    })
}

pub(crate) fn sample(root: &Path, job: &Job, x: f64, y: f64) -> Result<RasterPixel> {
    let [left, bottom, right, top] = super::srtm::bounds(&job.item_id)?;
    if !x.is_finite() || !y.is_finite() || x < left || x >= right || y <= bottom || y > top {
        return Err("The coordinate is outside the original NUM grid".into());
    }
    let col = ((x - left) / SPACING).floor() as u32;
    let row = ((top - y) / SPACING).floor() as u32;
    if col >= srtm::EDGE || row >= srtm::EDGE {
        return Err("NUM coordinate exceeds its source grid".into());
    }
    let raw = read(root, job)?;
    let code = raw[(row * srtm::EDGE + col) as usize];
    Ok(RasterPixel {
        science: None,
        index_value: None,
        quality: None,
        decibels: None,
        job_id: job.id.clone(),
        sha256: job.sha256.clone().ok_or("Missing NUM checksum")?,
        crs: "EPSG:4326".into(),
        coordinate: [x, y],
        pixel: [col, row],
        center: [
            left + (f64::from(col) + 0.5) * SPACING,
            top - (f64::from(row) + 0.5) * SPACING,
        ],
        value: f64::from(code),
        values: None,
        near_infrared: None,
        reflectance: None,
        label: "SRTM NUM source code".into(),
        color: format!("#{code:02x}{code:02x}{code:02x}"),
        is_no_data: false,
    })
}
