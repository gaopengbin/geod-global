//! Read verified original HGT ZIPs; never manufacture TIFF metadata or heights.
use super::*;
use crate::{
    io_error,
    providers::{self, srtm},
    storage,
};

const SPACING: f64 = 1.0 / 3600.0;

pub(crate) fn profile() -> elevation::Profile {
    elevation::Profile {
        product: "srtmgl1-v003".into(),
        height_unit: "metre".into(),
        coordinate_unit: "degree".into(),
        vertical_reference: "EPSG:5773".into(),
        pixel_interpretation: "PixelIsPoint".into(),
    }
}

pub(crate) fn signed(profile: Option<&elevation::Profile>) -> bool {
    profile.is_some_and(|p| p == &self::profile())
}

pub(crate) fn validate_job(job: &Job) -> Result<()> {
    if job.kind == "raster_mosaic" && job.asset_key == "srtm" {
        return elevation::validate_job(job);
    }
    let url = providers::asset_url(&job.href)?;
    if job.status != JobStatus::Succeeded
        || job.kind != "download"
        || job.asset_key != "srtm"
        || extension(&job.media_type)? != "zip"
        || url.host_str() != Some(providers::nasa::HOST)
        || !providers::matches_item(&url, &job.item_id, "srtm")
    {
        return Err("SRTM inspection requires a completed NASA SRTMGL1 original HGT ZIP".into());
    }
    Ok(())
}

fn read_source(root: &Path, job: &Job) -> Result<Vec<u8>> {
    open_original(root, job, None).map(|(_, bytes)| bytes)
}

/// Keep the original file pinned against replacement until processing finishes.
pub(crate) fn open_original(
    root: &Path,
    job: &Job,
    cancel: Option<&CancellationToken>,
) -> Result<(File, Vec<u8>)> {
    if job.kind != "download" {
        return Err("SRTM HGT source must be an original download".into());
    }
    validate_job(job)?;
    let path = storage::verified_output_path(root, job)?;
    let mut options = File::options();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1); // FILE_SHARE_READ
    }
    let mut file = options.open(path).map_err(io_error)?;
    let size = file.metadata().map_err(io_error)?.len();
    if size == 0
        || size > srtm::MAX_ZIP_BYTES
        || size != job.bytes_downloaded
        || job.total_bytes.is_some_and(|total| total != size)
    {
        return Err("SRTM ZIP size no longer matches its completed job".into());
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut bytes = Vec::with_capacity(size as usize);
    let mut buffer = [0; 65536];
    loop {
        check_cancel(cancel)?;
        check_time(deadline)?;
        let count = file.read(&mut buffer).map_err(io_error)?;
        if count == 0 {
            break;
        }
        if bytes.len() as u64 + count as u64 > size {
            return Err("SRTM ZIP changed during reading".into());
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    if bytes.len() as u64 != size
        || job.sha256.as_deref() != Some(format!("{:x}", Sha256::digest(&bytes)).as_str())
    {
        return Err("SHA-256 mismatch: SRTM ZIP no longer matches its completed download".into());
    }
    let hgt = srtm::read_hgt_cancelled(Cursor::new(bytes), &job.item_id, deadline, cancel)?;
    Ok((file, hgt))
}

pub(crate) fn bounds(id: &str) -> Result<[f64; 4]> {
    let [lon, lat] = srtm::cell(id).ok_or("Invalid SRTM tile")?.map(f64::from);
    // Both endpoints are posted samples. Neighbours share one row/column.
    Ok([
        lon - SPACING / 2.0,
        lat - SPACING / 2.0,
        lon + 1.0 + SPACING / 2.0,
        lat + 1.0 + SPACING / 2.0,
    ])
}

fn value(bytes: &[u8], x: u32, y: u32) -> i16 {
    let offset = (y as usize * srtm::EDGE as usize + x as usize) * 2;
    i16::from_be_bytes([bytes[offset], bytes[offset + 1]])
}

pub(crate) fn inspect(root: &Path, job: &Job, edge: u32) -> Result<RasterInspection> {
    if job.kind == "raster_mosaic" {
        return elevation::inspect(root, job, edge);
    }
    if edge == 0 || edge > PREVIEW_EDGE {
        return Err("Invalid SRTM preview size".into());
    }
    let bytes = read_source(root, job)?;
    let samples: Vec<_> = (0..edge)
        .flat_map(|y| (0..edge).map(move |x| (x, y)))
        .map(|(x, y)| value(&bytes, x * srtm::EDGE / edge, y * srtm::EDGE / edge))
        .collect();
    let mut valid: Vec<_> = samples.iter().copied().filter(|v| *v != i16::MIN).collect();
    valid.sort_unstable();
    let range = if valid.is_empty() {
        [0.0, 0.0]
    } else {
        [
            f64::from(valid[(valid.len() - 1) * 2 / 100]),
            f64::from(valid[(valid.len() - 1) * 98 / 100]),
        ]
    };
    let rgba: Vec<_> = samples
        .iter()
        .flat_map(|v| {
            if *v == i16::MIN {
                [0, 0, 0, 0]
            } else {
                let gray = if range[0] == range[1] {
                    128
                } else {
                    ((f64::from(*v) - range[0]) / (range[1] - range[0]) * 255.0)
                        .clamp(0.0, 255.0)
                        .round() as u8
                };
                [gray, gray, gray, 255]
            }
        })
        .collect();
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
        data_type: "Int16".into(),
        crs: "EPSG:4326".into(),
        bounds: bounds(&job.item_id)?,
        pixel_size: [SPACING; 2],
        nodata: Some(-32768.0),
        preview_data_url: format!("data:image/png;base64,{}", STANDARD.encode(png)),
        preview_width: edge,
        preview_height: edge,
        classes: Vec::new(),
        sha256: job.sha256.clone().ok_or("Missing SRTM checksum")?,
        reflectance: None,
        aerial: None,
        elevation: Some(elevation::ElevationDisplay {
            product: "srtmgl1-v003".into(),
            height_unit: "metre".into(),
            coordinate_unit: "degree".into(),
            vertical_reference: "EPSG:5773".into(),
            pixel_interpretation: "PixelIsPoint".into(),
            display_range: range,
            sample_count: edge * edge,
            valid_sample_count: valid.len() as u32,
            nodata_is_nan: false,
        }),
    })
}

pub(crate) fn sample(root: &Path, job: &Job, x: f64, y: f64) -> Result<RasterPixel> {
    if job.kind == "raster_mosaic" {
        return elevation::sample(root, job, x, y);
    }
    let [left, bottom, right, top] = bounds(&job.item_id)?;
    if !x.is_finite() || !y.is_finite() || x < left || x >= right || y <= bottom || y > top {
        return Err("The coordinate is outside the source raster pixel grid".into());
    }
    let col = ((x - left) / SPACING).floor() as u32;
    let row = ((top - y) / SPACING).floor() as u32;
    if col >= srtm::EDGE || row >= srtm::EDGE {
        return Err("SRTM coordinate exceeds its source grid".into());
    }
    let bytes = read_source(root, job)?;
    let height = value(&bytes, col, row);
    Ok(RasterPixel {
        science: None,
        index_value: None,
        quality: None,
        decibels: None,
        job_id: job.id.clone(),
        sha256: job.sha256.clone().ok_or("Missing SRTM checksum")?,
        crs: "EPSG:4326".into(),
        coordinate: [x, y],
        pixel: [col, row],
        center: [
            left + (col as f64 + 0.5) * SPACING,
            top - (row as f64 + 0.5) * SPACING,
        ],
        value: f64::from(height),
        values: None,
        near_infrared: None,
        reflectance: None,
        label: "Elevation".into(),
        color: "#808080".into(),
        is_no_data: height == i16::MIN,
    })
}

#[cfg(test)]
mod tests;
