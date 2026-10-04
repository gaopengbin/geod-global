//! Bounded, local inspection of managed Sentinel-2 SCL and UInt8 RGB files.
//! Palette reference: https://custom-scripts.sentinel-hub.com/custom-scripts/sentinel-2/scene-classification/
//! GeoTIFF tags are read from the file; no catalog values substitute for missing metadata.
use crate::{extension, Job, JobManager, JobStatus, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Cursor, Read, Seek, SeekFrom},
    path::Path,
    time::{Duration, Instant},
};
use tiff::{
    decoder::{Decoder, DecodingResult, Limits},
    tags::Tag,
    ColorType,
};
use tokio_util::sync::CancellationToken;

const MAX_FILE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_PIXELS: u64 = 64 * 1024 * 1024;
const MAX_DIMENSION: u32 = 16384;
const PREVIEW_EDGE: u32 = 768;
pub use aerial::AerialView;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RasterClass {
    pub value: u8,
    pub label: String,
    pub color: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RasterInspection {
    pub width: u32,
    pub height: u32,
    pub band_count: u8,
    pub data_type: String,
    pub crs: String,
    /// Outer pixel edges in the projected CRS, [minX, minY, maxX, maxY].
    pub bounds: [f64; 4],
    /// Positive x/y spacing in the source CRS units, metres or degrees.
    pub pixel_size: [f64; 2],
    pub nodata: Option<f64>,
    pub preview_data_url: String,
    pub preview_width: u32,
    pub preview_height: u32,
    pub classes: Vec<RasterClass>,
    pub sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reflectance: Option<reflectance::ReflectanceDisplay>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vegetation: Option<reflectance::VegetationDisplay>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub science: Option<science::ScienceDisplay>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elevation: Option<elevation::ElevationDisplay>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aerial: Option<aerial::AerialDisplay>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radar: Option<radar::RadarDisplay>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<quality::QualityDisplay>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RasterPixel {
    pub job_id: String,
    pub sha256: String,
    pub crs: String,
    /// Requested source-CRS coordinate, followed by its exact source pixel and centre.
    pub coordinate: [f64; 2],
    pub pixel: [u32; 2],
    pub center: [f64; 2],
    /// Exact integer DN or Float32 elevation promoted to Float64. NaN NoData is null in JSON.
    #[serde(serialize_with = "serialize_pixel_value")]
    pub value: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<[u8; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub near_infrared: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reflectance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub science: Option<science::SciencePixel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decibels: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<quality::QualityPixel>,
    pub label: String,
    pub color: String,
    pub is_no_data: bool,
}

/// Shared verified pixel snapshot used by inspection and cropping. It is never
/// constructed from catalog metadata or an unchecked filesystem path.
pub(crate) struct DecodedRaster {
    pub width: u32,
    pub height: u32,
    pub crs: String,
    pub bounds: [f64; 4],
    pub pixel_size: [f64; 2],
    pub nodata: Option<u8>,
    pub pixels: Vec<u8>,
    pub sha256: String,
    counts: [u64; 12],
}

// Standard SCL class values and display colors, independently applied to decoded pixels.
pub(crate) const PALETTE: [(&str, [u8; 3]); 12] = [
    ("No data", [0, 0, 0]),
    ("Saturated or defective", [255, 0, 0]),
    ("Terrain shadow", [47, 47, 47]),
    ("Cloud shadow", [100, 50, 0]),
    ("Vegetation", [0, 160, 0]),
    ("Bare/non-vegetated", [255, 230, 90]),
    ("Water", [0, 0, 255]),
    ("Unclassified", [128, 128, 128]),
    ("Medium-probability cloud", [192, 192, 192]),
    ("High-probability cloud", [255, 255, 255]),
    ("Thin cirrus", [100, 200, 255]),
    ("Snow or ice", [255, 150, 255]),
];

impl JobManager {
    pub async fn sample_raster(&self, id: &str, x: f64, y: f64) -> Result<RasterPixel> {
        if !x.is_finite() || !y.is_finite() {
            return Err("Pixel coordinates must be finite source-CRS values".into());
        }
        let job = self.get(id).await.ok_or("Unknown job")?;
        let root = self.inner.root.clone();
        let permit = tokio::time::timeout(
            Duration::from_secs(5),
            self.inner.raster_permits.clone().acquire_owned(),
        )
        .await
        .map_err(|_| "The raster worker is busy; try the pixel query again shortly")?
        .map_err(fail)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            if crate::providers::modis::QUALITY_KEYS.contains(&job.asset_key.as_str()) {
                return quality::sample(&root, &job, x, y);
            }
            if landsat_quality::KEYS.contains(&job.asset_key.as_str()) {
                return landsat_quality::sample(&root, &job, x, y);
            }
            if crate::providers::radar::KEYS.contains(&job.asset_key.as_str()) {
                return radar::sample(&root, &job, x, y);
            }
            if matches!(job.asset_key.as_str(), "visual" | "aerial") {
                return rgb::sample(&root, &job, x, y);
            }
            if matches!(
                job.asset_key.as_str(),
                "red"
                    | "green"
                    | "blue"
                    | "ndvi"
                    | "evi"
                    | "vi_quality"
                    | "vi_reliability"
                    | "vi_doy"
                    | "vi_red"
                    | "vi_nir"
                    | "vi_blue"
                    | "vi_mir"
                    | "vi_view_zenith"
                    | "vi_sun_zenith"
                    | "vi_relative_azimuth"
            ) {
                return reflectance::sample(&root, &job, x, y);
            }
            if job.asset_key == "elevation" {
                return elevation::sample(&root, &job, x, y);
            }
            if job.asset_key == "srtm" {
                return srtm::sample(&root, &job, x, y);
            }
            let raster = load_verified_raster(&root, &job, None)?;
            sample_pixel(raster, &job.id, x, y)
        })
        .await
        .map_err(fail)?
    }

    pub async fn inspect_raster(&self, id: &str) -> Result<RasterInspection> {
        self.inspect_raster_view(id, None).await
    }

    pub async fn inspect_raster_view(
        &self,
        id: &str,
        aerial_view: Option<AerialView>,
    ) -> Result<RasterInspection> {
        let job = self.get(id).await.ok_or("Unknown job")?;
        if aerial_view.is_some() && job.asset_key != "aerial" {
            return Err("NAIP display selection requires a managed RGB + NIR raster".into());
        }
        let root = self.inner.root.clone();
        let permit = tokio::time::timeout(
            Duration::from_secs(5),
            self.inner.raster_permits.clone().acquire_owned(),
        )
        .await
        .map_err(|_| "Raster inspection is busy. Try again when the current inspection finishes.")?
        .map_err(fail)?;
        // Own the permit inside the blocking task: dropping an HTTP request cannot
        // release it while decoding is still running and admit unbounded work.
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            if let Some(view) = aerial_view {
                return rgb::inspect_with_view(&root, &job, PREVIEW_EDGE, view);
            }
            if crate::providers::modis::QUALITY_KEYS.contains(&job.asset_key.as_str()) {
                return quality::inspect(&root, &job, PREVIEW_EDGE);
            }
            if landsat_quality::KEYS.contains(&job.asset_key.as_str()) {
                return landsat_quality::inspect(&root, &job, PREVIEW_EDGE);
            }
            if crate::providers::radar::KEYS.contains(&job.asset_key.as_str()) {
                return radar::inspect(&root, &job, PREVIEW_EDGE);
            }
            if matches!(job.asset_key.as_str(), "visual" | "aerial") {
                return rgb::inspect(&root, &job);
            }
            if matches!(
                job.asset_key.as_str(),
                "red"
                    | "green"
                    | "blue"
                    | "ndvi"
                    | "evi"
                    | "vi_quality"
                    | "vi_reliability"
                    | "vi_doy"
                    | "vi_red"
                    | "vi_nir"
                    | "vi_blue"
                    | "vi_mir"
                    | "vi_view_zenith"
                    | "vi_sun_zenith"
                    | "vi_relative_azimuth"
            ) {
                return reflectance::inspect(&root, &job, PREVIEW_EDGE);
            }
            if job.asset_key == "elevation" {
                return elevation::inspect(&root, &job, PREVIEW_EDGE);
            }
            if job.asset_key == "srtm" {
                return srtm::inspect(&root, &job, PREVIEW_EDGE);
            }
            inspect_download(&root, &job)
        })
        .await
        .map_err(|e| format!("Raster inspection could not finish: {e}"))?
    }
}

fn sample_pixel(raster: DecodedRaster, id: &str, x: f64, y: f64) -> Result<RasterPixel> {
    let [left, bottom, right, top] = raster.bounds;
    if !x.is_finite() || !y.is_finite() || x < left || x >= right || y <= bottom || y > top {
        return Err("The coordinate is outside the source raster pixel grid".into());
    }
    let column = ((x - left) / raster.pixel_size[0]).floor() as u32;
    let row = ((top - y) / raster.pixel_size[1]).floor() as u32;
    if column >= raster.width || row >= raster.height {
        return Err("The coordinate is outside the source raster pixel grid".into());
    }
    let value = raster.pixels[row as usize * raster.width as usize + column as usize];
    let (label, rgb) = PALETTE[value as usize];
    Ok(RasterPixel {
        science: None,
        index_value: None,
        quality: None,
        decibels: None,
        job_id: id.into(),
        sha256: raster.sha256,
        crs: raster.crs,
        coordinate: [x, y],
        pixel: [column, row],
        center: [
            left + (column as f64 + 0.5) * raster.pixel_size[0],
            top - (row as f64 + 0.5) * raster.pixel_size[1],
        ],
        value: f64::from(value),
        values: None,
        near_infrared: None,
        reflectance: None,
        label: label.into(),
        color: format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]),
        is_no_data: raster.nodata == Some(value),
    })
}

pub(crate) mod aerial;
pub(crate) mod elevation;
pub(crate) mod landsat_quality;
pub(crate) mod quality;
pub(crate) mod radar;
pub(crate) mod reflectance;
pub(crate) mod rgb;
pub(crate) mod science;
pub(crate) mod srtm;

fn serialize_pixel_value<S: serde::Serializer>(
    value: &f64,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error> {
    if !value.is_finite() {
        serializer.serialize_none()
    } else if value.fract() == 0.0 && value.abs() <= 9_007_199_254_740_991.0 {
        serializer.serialize_i64(*value as i64)
    } else {
        serializer.serialize_f64(*value)
    }
}

fn fail(error: impl std::fmt::Display) -> String {
    format!("Cannot inspect this GeoTIFF: {error}")
}

fn check_time(deadline: Instant) -> Result<()> {
    if Instant::now() > deadline {
        Err("Raster inspection exceeded its processing time limit".into())
    } else {
        Ok(())
    }
}

struct TimedReader<'a> {
    bytes: Cursor<&'a [u8]>,
    deadline: Instant,
    cancel: Option<&'a CancellationToken>,
}
impl Read for TimedReader<'_> {
    fn read(&mut self, target: &mut [u8]) -> std::io::Result<usize> {
        if self.cancel.is_some_and(CancellationToken::is_cancelled) {
            // `read_exact` retries Interrupted, so cancellation must be terminal.
            return Err(std::io::Error::other("Raster operation cancelled"));
        }
        if Instant::now() > self.deadline {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Raster inspection time limit",
            ));
        }
        self.bytes.read(target)
    }
}
impl Seek for TimedReader<'_> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.bytes.seek(position)
    }
}

fn inspect_download(root: &Path, job: &Job) -> Result<RasterInspection> {
    let raster = load_verified_raster(root, job, None)?;
    preview(raster, Instant::now() + Duration::from_secs(60))
}

pub(crate) fn load_verified_raster(
    root: &Path,
    job: &Job,
    cancel: Option<&CancellationToken>,
) -> Result<DecodedRaster> {
    let deadline = Instant::now() + Duration::from_secs(60);
    check_cancel(cancel)?;
    if job.status != JobStatus::Succeeded {
        return Err("Raster inspection requires a completed download".into());
    }
    if job.asset_key != "scl" || extension(&job.media_type)? != "tif" {
        return Err("Raster inspection currently supports only single-band UInt8 Sentinel-2 SCL GeoTIFF assets; JPEG and RGB assets are unsupported".into());
    }
    if uuid::Uuid::parse_str(&job.id).is_err() {
        return Err("The stored job identifier is invalid".into());
    }
    let root = root.canonicalize().map_err(fail)?;
    let assets = root.join("assets").canonicalize().map_err(fail)?;
    if assets != root.join("assets") {
        return Err(
            "The managed assets directory must not redirect outside its original location".into(),
        );
    }
    let expected = assets.join(format!("{}.tif", job.id));
    let output = crate::storage::exact_file(
        &expected,
        Path::new(
            job.output_path
                .as_deref()
                .ok_or("The completed job has no output file")?,
        ),
    )
    .map_err(|error| {
        format!("Raster inspection is restricted to this job's managed asset file: {error}")
    })?;
    let mut file = File::open(output).map_err(fail)?;
    let size = file.metadata().map_err(fail)?.len();
    if size == 0 || size > MAX_FILE_BYTES {
        return Err("Raster inspection accepts files up to 128 MiB".into());
    }
    if size != job.bytes_downloaded || job.total_bytes.is_some_and(|total| total != size) {
        return Err("The local file size no longer matches its download record".into());
    }
    let expected_hash = job
        .sha256
        .as_deref()
        .ok_or("The download has no recorded SHA-256")?;
    let mut bytes = Vec::with_capacity(size as usize);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        check_cancel(cancel)?;
        check_time(deadline)?;
        let count = file.read(&mut buffer).map_err(fail)?;
        if count == 0 {
            break;
        }
        if bytes.len() as u64 + count as u64 > MAX_FILE_BYTES {
            return Err("The local asset grew beyond the inspection size limit".into());
        }
        hasher.update(&buffer[..count]);
        bytes.extend_from_slice(&buffer[..count]);
    }
    if bytes.len() as u64 != size {
        return Err("The local asset changed while it was being read".into());
    }
    let sha256 = format!("{:x}", hasher.finalize());
    if sha256 != expected_hash {
        return Err(
            "SHA-256 mismatch: the local asset no longer matches the completed download".into(),
        );
    }
    // Decode this exact hash-verified snapshot; never reopen the file for pixels.
    let raster = decode_raster(&bytes, sha256, deadline, cancel)?;
    if job.kind == "raster_prepare" {
        crate::safe::validate_stored(job)?;
        let grid = job
            .safe_output
            .as_ref()
            .ok_or("Prepared SAFE grid is missing")?;
        if grid.width != raster.width
            || grid.height != raster.height
            || grid.band_count != 1
            || grid.crs != raster.crs
            || grid.bounds != raster.bounds
            || grid.pixel_size != raster.pixel_size
            || raster.nodata != Some(0)
        {
            return Err("Prepared SAFE raster differs from its recorded grid".into());
        }
    }
    Ok(raster)
}

pub(crate) fn check_cancel(cancel: Option<&CancellationToken>) -> Result<()> {
    if cancel.is_some_and(CancellationToken::is_cancelled) {
        Err("Raster operation cancelled".into())
    } else {
        Ok(())
    }
}

/// Decode verified in-memory bytes, also used to read back a new partial output
/// before it becomes a visible completed asset.
pub(crate) fn decode_raster(
    bytes: &[u8],
    sha256: String,
    deadline: Instant,
    cancel: Option<&CancellationToken>,
) -> Result<DecodedRaster> {
    check_cancel(cancel)?;
    let mut limits = Limits::default();
    limits.decoding_buffer_size = MAX_PIXELS as usize;
    limits.intermediate_buffer_size = 16 * 1024 * 1024;
    limits.ifd_value_size = 1024 * 1024;
    let mut decoder = Decoder::new(TimedReader {
        bytes: Cursor::new(bytes),
        deadline,
        cancel,
    })
    .map_err(fail)?
    .with_limits(limits);
    let (width, height) = decoder.dimensions().map_err(fail)?;
    let pixel_count = width as u64 * height as u64;
    if width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || pixel_count > MAX_PIXELS
    {
        return Err(
            "Raster dimensions exceed the 64-megapixel / 16384-pixel edge inspection limit".into(),
        );
    }
    if decoder.colortype().map_err(fail)? != ColorType::Gray(8)
        || decoder
            .find_tag_unsigned::<u16>(Tag::SamplesPerPixel)
            .map_err(fail)?
            .unwrap_or(1)
            != 1
        || decoder
            .find_tag_unsigned_vec::<u16>(Tag::SampleFormat)
            .map_err(fail)?
            .is_some_and(|v| v != [1])
    {
        return Err("Only single-band unsigned 8-bit SCL rasters are currently supported".into());
    }
    if decoder
        .find_tag_unsigned::<u16>(Tag::Orientation)
        .map_err(fail)?
        .unwrap_or(1)
        != 1
    {
        return Err("Only top-left raster orientation is currently supported".into());
    }
    if decoder
        .find_tag_unsigned::<u16>(Tag::PhotometricInterpretation)
        .map_err(fail)?
        != Some(1)
    {
        return Err("Only unmodified grayscale SCL sample values are supported".into());
    }
    let keys = decoder
        .get_tag_u16_vec(Tag::GeoKeyDirectoryTag)
        .map_err(|_| "GeoTIFF projection keys are missing or invalid")?;
    let crs = validate_geokeys(&keys)?;
    let transform = decoder
        .find_tag(Tag::ModelTransformationTag)
        .map_err(fail)?
        .map(|value| value.into_f64_vec())
        .transpose()
        .map_err(fail)?;
    let scale = decoder
        .find_tag(Tag::ModelPixelScaleTag)
        .map_err(fail)?
        .map(|value| value.into_f64_vec())
        .transpose()
        .map_err(fail)?;
    let tiepoint = decoder
        .find_tag(Tag::ModelTiepointTag)
        .map_err(fail)?
        .map(|value| value.into_f64_vec())
        .transpose()
        .map_err(fail)?;
    let (bounds, pixel_size) = georeference(
        width,
        height,
        transform.as_deref(),
        scale.as_deref(),
        tiepoint.as_deref(),
    )?;
    let nodata = decoder
        .find_tag(Tag::GdalNodata)
        .map_err(fail)?
        .map(|value| {
            value.into_string().map_err(fail).and_then(|value| {
                let value = value
                    .trim_matches('\0')
                    .trim()
                    .parse::<u8>()
                    .map_err(|_| "Unsupported or invalid GeoTIFF nodata value")?;
                if value != 0 {
                    return Err("Only the standard SCL nodata value 0 is supported".into());
                }
                Ok(value)
            })
        })
        .transpose()?;
    let pixels = match decoder.read_image().map_err(fail)? {
        DecodingResult::U8(values) => values,
        _ => return Err("Decoded raster samples are not unsigned 8-bit values".into()),
    };
    if pixels.len() as u64 != pixel_count {
        return Err("Decoded sample count does not match the raster dimensions".into());
    }
    let mut counts = [0u64; 12];
    for chunk in pixels.chunks(1024 * 1024) {
        check_cancel(cancel)?;
        check_time(deadline)?;
        for value in chunk {
            let count = counts
                .get_mut(*value as usize)
                .ok_or("The raster contains values outside the Sentinel-2 SCL classes 0-11")?;
            *count += 1;
        }
    }
    Ok(DecodedRaster {
        width,
        height,
        crs,
        bounds,
        pixel_size,
        nodata,
        pixels,
        sha256,
        counts,
    })
}

fn preview(raster: DecodedRaster, deadline: Instant) -> Result<RasterInspection> {
    let DecodedRaster {
        width,
        height,
        crs,
        bounds,
        pixel_size,
        nodata,
        pixels,
        sha256,
        counts,
    } = raster;
    let longest = width.max(height);
    let preview_width = if longest <= PREVIEW_EDGE {
        width
    } else {
        ((width as u64 * PREVIEW_EDGE as u64) / longest as u64).max(1) as u32
    };
    let preview_height = if longest <= PREVIEW_EDGE {
        height
    } else {
        ((height as u64 * PREVIEW_EDGE as u64) / longest as u64).max(1) as u32
    };
    let mut rgba = Vec::with_capacity((preview_width * preview_height * 4) as usize);
    for y in 0..preview_height {
        check_time(deadline)?;
        let source_y = y as u64 * height as u64 / preview_height as u64;
        for x in 0..preview_width {
            let source_x = x as u64 * width as u64 / preview_width as u64;
            let value = pixels[(source_y * width as u64 + source_x) as usize];
            rgba.extend_from_slice(&PALETTE[value as usize].1);
            rgba.push(if nodata == Some(value) { 0 } else { 255 });
        }
    }
    let mut png_data = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_data, preview_width, preview_height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(fail)?;
        writer.write_image_data(&rgba).map_err(fail)?;
    }
    check_time(deadline)?;
    Ok(RasterInspection {
        science: None,
        vegetation: None,
        quality: None,
        radar: None,
        width,
        height,
        band_count: 1,
        data_type: "UInt8".into(),
        crs,
        bounds,
        pixel_size,
        nodata: nodata.map(f64::from),
        preview_data_url: format!("data:image/png;base64,{}", STANDARD.encode(png_data)),
        preview_width,
        preview_height,
        sha256,
        reflectance: None,
        elevation: None,
        aerial: None,
        classes: PALETTE
            .iter()
            .enumerate()
            .map(|(value, (label, rgb))| RasterClass {
                value: value as u8,
                label: (*label).into(),
                color: format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]),
                count: counts[value],
            })
            .collect(),
    })
}

pub(crate) fn validate_geokeys(keys: &[u16]) -> Result<String> {
    validate_projected_geokeys(keys, false)
}
pub(crate) fn validate_projected_geokeys(keys: &[u16], nad83: bool) -> Result<String> {
    if keys.len() < 4
        || keys[0] != 1
        || keys[1] != 1
        || keys[2] > 1
        || keys.len() < 4 + 4 * keys[3] as usize
    {
        return Err("Unsupported or malformed GeoTIFF key directory".into());
    }
    let entries = &keys[4..4 + 4 * keys[3] as usize];
    let get = |key: u16| -> Result<Option<u16>> {
        let mut matches = entries.chunks_exact(4).filter(|entry| entry[0] == key);
        let value = matches.next();
        if matches.next().is_some() {
            return Err("Duplicated GeoTIFF projection key".into());
        }
        match value {
            Some(entry) if entry[1] == 0 && entry[2] == 1 => Ok(Some(entry[3])),
            Some(_) => Err("Unsupported indirect GeoTIFF projection key".into()),
            None => Ok(None),
        }
    };
    if get(1024)? != Some(1) || get(1025)? != Some(1) {
        return Err(
            "Only projected PixelIsArea GeoTIFF coordinates are currently supported".into(),
        );
    }
    if get(3076)?.is_some_and(|units| units != 9001) {
        return Err("Only metre-based projected coordinates are supported".into());
    }
    let epsg = get(3072)?.ok_or("Projected CRS EPSG code is missing from the GeoTIFF")?;
    if if nad83 {
        !(26901..=26923).contains(&epsg)
    } else {
        !(32601..=32660).contains(&epsg) && !(32701..=32760).contains(&epsg)
    } {
        return Err(if nad83 {
            "NAIP requires NAD83 UTM EPSG 26901-26923"
        } else {
            "Only WGS84 UTM north/south EPSG 32601-32660 and 32701-32760 are currently supported"
        }
        .into());
    }
    Ok(format!("EPSG:{epsg}"))
}

pub(crate) fn georeference(
    width: u32,
    height: u32,
    transform: Option<&[f64]>,
    scale: Option<&[f64]>,
    tiepoint: Option<&[f64]>,
) -> Result<([f64; 4], [f64; 2])> {
    let (x, y, dx, dy) = if let Some(matrix) = transform {
        if scale.is_some() || tiepoint.is_some() {
            return Err("Conflicting GeoTIFF transformation tags are unsupported".into());
        }
        if matrix.len() != 16
            || !matrix.iter().all(|v| v.is_finite())
            || matrix[0] <= 0.0
            || matrix[5] >= 0.0
            || [1, 2, 4, 6, 8, 9, 11, 12, 13, 14]
                .iter()
                .any(|&i| matrix[i] != 0.0)
            || matrix[15] != 1.0
        {
            return Err("Only a finite, north-up, unrotated GeoTIFF transform is supported".into());
        }
        (matrix[3], matrix[7], matrix[0], -matrix[5])
    } else {
        let scale = scale.ok_or("GeoTIFF pixel scale is missing")?;
        let tiepoint = tiepoint.ok_or("GeoTIFF tiepoint is missing")?;
        if scale.len() != 3
            || tiepoint.len() != 6
            || !scale.iter().chain(tiepoint).all(|v| v.is_finite())
            || scale[0] <= 0.0
            || scale[1] <= 0.0
            || scale[2] != 0.0
            || tiepoint[2] != 0.0
            || tiepoint[5] != 0.0
        {
            return Err("Invalid or unsupported GeoTIFF pixel scale / tiepoint".into());
        }
        (
            tiepoint[3] - tiepoint[0] * scale[0],
            tiepoint[4] + tiepoint[1] * scale[1],
            scale[0],
            scale[1],
        )
    };
    let bounds = [x, y - height as f64 * dy, x + width as f64 * dx, y];
    if !bounds.iter().all(|v| v.is_finite()) || bounds[0] >= bounds[2] || bounds[1] >= bounds[3] {
        return Err("GeoTIFF bounds are invalid".into());
    }
    Ok((bounds, [dx, dy]))
}

#[cfg(test)]
pub(crate) mod tests;
