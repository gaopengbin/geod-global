//! Public Copernicus GLO-30/GLO-90 originals and aligned Float32 outputs. Display never changes values.
use super::*;
use crate::{io_error, providers};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::BufReader;
use tiff::decoder::ChunkType;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElevationDisplay {
    pub product: String,
    pub height_unit: String,
    pub coordinate_unit: String,
    pub vertical_reference: String,
    pub pixel_interpretation: String,
    pub display_range: [f64; 2],
    pub sample_count: u32,
    pub valid_sample_count: u32,
    pub nodata_is_nan: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Profile {
    pub product: String,
    pub height_unit: String,
    pub coordinate_unit: String,
    pub vertical_reference: String,
    pub pixel_interpretation: String,
}
#[cfg(test)]
pub(crate) fn profile() -> Profile {
    cop_dem_profile(providers::DemProduct::Glo30Public)
}
fn cop_dem_profile(product: providers::DemProduct) -> Profile {
    Profile {
        product: product.product().into(),
        height_unit: "metre".into(),
        coordinate_unit: "degree".into(),
        vertical_reference: "EPSG:3855".into(),
        pixel_interpretation: "PixelIsPoint".into(),
    }
}
pub(crate) struct Header {
    pub width: u32,
    pub height: u32,
    pub bounds: [f64; 4],
    pub pixel_size: [f64; 2],
    pub nodata: Option<f32>,
}
struct Source<R: Read + Seek> {
    decoder: Decoder<R>,
    width: u32,
    height: u32,
    bounds: [f64; 4],
    pixel_size: [f64; 2],
    nodata: Option<f32>,
    deadline: Instant,
    signed_height: bool,
}

pub(crate) fn job_profile(job: &Job) -> Result<Profile> {
    if job.asset_key == "srtm" {
        Ok(super::srtm::profile())
    } else {
        let url = providers::asset_url(&job.href)?;
        let item = url
            .path_segments()
            .and_then(|mut v| v.next())
            .ok_or("Missing elevation geocell")?;
        let product = providers::dem_product(item).ok_or("Unsupported Copernicus DEM product")?;
        if url.host_str() != Some(product.host()) {
            return Err("Elevation product and bucket differ".into());
        }
        Ok(cop_dem_profile(product))
    }
}

pub(crate) fn validate_job(job: &Job) -> Result<()> {
    if job.status != JobStatus::Succeeded
        || !matches!(job.kind.as_str(), "download" | "raster_mosaic")
        || !matches!(job.asset_key.as_str(), "elevation" | "srtm")
        || (job.asset_key == "srtm" && job.kind != "raster_mosaic")
        || extension(&job.media_type)? != "tif"
    {
        return Err(
            "Elevation inspection requires a completed Copernicus DEM original or project output"
                .into(),
        );
    }
    if job.kind == "raster_mosaic" {
        crate::mosaic::validate_stored_mosaic(job)?;
        let plan = job
            .mosaic_output
            .as_ref()
            .ok_or("Elevation output grid is missing")?;
        let expected = job_profile(job)?;
        let product = if job.asset_key == "srtm" {
            providers::DemProduct::Glo30Public
        } else if expected.product == providers::DemProduct::Glo90.product() {
            providers::DemProduct::Glo90
        } else {
            providers::DemProduct::Glo30Public
        };
        if plan.elevation.as_ref() != Some(&expected)
            || plan.calibration.is_some()
            || plan.aerial.is_some()
            || plan.crs != "EPSG:4326"
            || plan.band_count != 1
            || plan.width == 0
            || plan.height == 0
            || plan.bounds.iter().any(|v| !v.is_finite())
            || plan.pixel_size.iter().any(|v| !v.is_finite() || *v <= 0.0)
            || (job.asset_key == "srtm" && (plan.pixel_size[0] - 1.0 / 3600.0).abs() > 1e-12)
            || !product
                .widths()
                .iter()
                .any(|width| (plan.pixel_size[0] - 1.0 / f64::from(*width)).abs() < 1e-12)
            || (plan.pixel_size[1] - 1.0 / f64::from(product.height())).abs() > 1e-12
            || ((plan.bounds[2] - plan.bounds[0]) / plan.pixel_size[0] - f64::from(plan.width))
                .abs()
                > 1e-5
            || ((plan.bounds[3] - plan.bounds[1]) / plan.pixel_size[1] - f64::from(plan.height))
                .abs()
                > 1e-5
            || plan.bounds[0] >= plan.bounds[2]
            || plan.bounds[1] >= plan.bounds[3]
            || job.item_id != format!("project:{}", job.mosaic.as_ref().unwrap().project_id)
        {
            return Err("Elevation output does not match its pinned project grid".into());
        }
        return Ok(());
    }
    let href = providers::asset_url(&job.href)?;
    if !matches!(
        href.host_str(),
        Some(providers::DEM_HOST | providers::DEM90_HOST)
    ) || !providers::matches_item(&href, &job.item_id, "elevation")
    {
        return Err("Elevation file does not match its public Copernicus DEM geocell".into());
    }
    Ok(())
}

pub(crate) fn validate_header<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    item: &str,
    output: Option<&crate::mosaic::MosaicPlan>,
) -> Result<Header> {
    let signed_height = super::srtm::signed(output.and_then(|plan| plan.elevation.as_ref()));
    let product = if signed_height {
        providers::DemProduct::Glo30Public
    } else if let Some(plan) = output {
        match plan.elevation.as_ref().map(|p| p.product.as_str()) {
            Some("cop-dem-glo-30-public") => providers::DemProduct::Glo30Public,
            Some("cop-dem-glo-90") => providers::DemProduct::Glo90,
            _ => return Err("Unknown elevation output product".into()),
        }
    } else {
        providers::dem_product(item).ok_or("Invalid Copernicus DEM product")?
    };
    let expected_profile = if signed_height {
        super::srtm::profile()
    } else {
        cop_dem_profile(product)
    };
    let (width, height) = decoder.dimensions().map_err(io_error)?;
    if width == 0
        || height == 0
        || (output.is_none() && (!product.widths().contains(&width) || height != product.height()))
        || decoder.colortype().map_err(io_error)?
            != ColorType::Gray(if signed_height { 16 } else { 32 })
        || decoder
            .find_tag_unsigned_vec::<u16>(Tag::SampleFormat)
            .map_err(io_error)?
            != Some(vec![if signed_height { 2 } else { 3 }])
        || decoder
            .find_tag_unsigned::<u16>(Tag::SamplesPerPixel)
            .map_err(io_error)?
            .unwrap_or(1)
            != 1
        || decoder
            .find_tag_unsigned::<u16>(Tag::Orientation)
            .map_err(io_error)?
            .unwrap_or(1)
            != 1
        || decoder
            .find_tag_unsigned::<u16>(Tag::PhotometricInterpretation)
            .map_err(io_error)?
            != Some(1)
        || decoder
            .find_tag_unsigned::<u16>(Tag::PlanarConfiguration)
            .map_err(io_error)?
            .unwrap_or(1)
            != 1
    {
        return Err(
            "Elevation sample type differs from its product: Copernicus Float32 or SRTM Int16"
                .into(),
        );
    }
    let keys = decoder
        .get_tag_u16_vec(Tag::GeoKeyDirectoryTag)
        .map_err(io_error)?;
    if keys.len() < 4
        || keys[..3] != [1, 1, u16::from(signed_height)]
        || keys.len() < 4 + keys[3] as usize * 4
    {
        return Err("Malformed elevation GeoTIFF key directory".into());
    }
    let mut entries = BTreeMap::new();
    for e in keys[4..4 + keys[3] as usize * 4].chunks_exact(4) {
        if entries.insert(e[0], [e[1], e[2], e[3]]).is_some() {
            return Err("Duplicated elevation GeoTIFF key".into());
        }
    }
    for (key, value) in [(1024, 2), (1025, 2), (2048, 4326), (2054, 9102)] {
        if entries.get(&key) != Some(&[0, 1, value]) {
            return Err(
                "Elevation source requires WGS84 degree-based PixelIsPoint coordinates".into(),
            );
        }
    }
    if entries.contains_key(&3072)
        || decoder
            .find_tag(Tag::ModelTransformationTag)
            .map_err(io_error)?
            .is_some()
    {
        return Err(
            "Elevation projection or affine transform conflicts with the original product".into(),
        );
    }
    if entries
        .get(&4096)
        .is_some_and(|v| v != &[0, 1, if signed_height { 5773 } else { 3855 }])
        || entries.get(&4099).is_some_and(|v| v != &[0, 1, 9001])
    {
        return Err(
            "Elevation vertical reference or height unit conflicts with its product".into(),
        );
    }
    let scale = decoder
        .get_tag_f64_vec(Tag::ModelPixelScaleTag)
        .map_err(io_error)?;
    let tie = decoder
        .get_tag_f64_vec(Tag::ModelTiepointTag)
        .map_err(io_error)?;
    let (mut bounds, pixel_size) = georeference(width, height, None, Some(&scale), Some(&tie))?;
    // PixelIsPoint tiepoints describe pixel centres; public bounds use the outer edges.
    bounds[0] -= pixel_size[0] / 2.0;
    bounds[2] -= pixel_size[0] / 2.0;
    bounds[1] += pixel_size[1] / 2.0;
    bounds[3] += pixel_size[1] / 2.0;
    if let Some(plan) = output {
        if plan.elevation.as_ref() != Some(&expected_profile)
            || plan.calibration.is_some()
            || plan.aerial.is_some()
            || (signed_height && pixel_size.iter().any(|v| (*v - 1.0 / 3600.0).abs() > 1e-12))
            || plan.crs != "EPSG:4326"
            || plan.band_count != 1
            || plan.width != width
            || plan.height != height
            || bounds
                .iter()
                .zip(plan.bounds)
                .any(|(a, b)| (*a - b).abs() > 1e-10)
            || pixel_size
                .iter()
                .zip(plan.pixel_size)
                .any(|(a, b)| (*a - b).abs() > 1e-12)
            || entries.get(&4096) != Some(&[0, 1, if signed_height { 5773 } else { 3855 }])
            || entries.get(&4099) != Some(&[0, 1, 9001])
        {
            return Err("Elevation output GeoTIFF differs from its recorded Point grid or vertical reference".into());
        }
    } else {
        let cell = providers::dem_cell(item).ok_or("Invalid elevation geocell")?;
        let expected = [1.0 / width as f64, 1.0 / f64::from(product.height())];
        if pixel_size
            .iter()
            .zip(expected)
            .any(|(a, b)| (*a - b).abs() > 1e-12)
            || (bounds[0] + pixel_size[0] / 2.0 - cell[0] as f64).abs() > 1e-10
            || (bounds[3] - pixel_size[1] / 2.0 - (cell[1] + 1) as f64).abs() > 1e-10
        {
            return Err("Elevation geometry does not match its original named geocell".into());
        }
    }
    let nodata = decoder
        .find_tag(Tag::GdalNodata)
        .map_err(io_error)?
        .map(|v| v.into_string().map_err(io_error))
        .transpose()?
        .map(|s| {
            s.trim_matches('\0')
                .trim()
                .parse::<f32>()
                .map_err(|_| "Invalid elevation NoData".to_string())
        })
        .transpose()?;
    if nodata.is_some_and(|n| n.is_infinite()) {
        return Err("Elevation NoData must be finite or NaN".into());
    }
    if output.is_some()
        && if signed_height {
            nodata != Some(-32768.0)
        } else {
            !nodata.is_some_and(f32::is_nan)
        }
    {
        return Err(
            "Elevation output requires product-specific NoData: SRTM -32768 or Copernicus NaN"
                .into(),
        );
    }
    Ok(Header {
        width,
        height,
        bounds,
        pixel_size,
        nodata,
    })
}

fn open_source(root: &Path, job: &Job) -> Result<Source<BufReader<File>>> {
    validate_job(job)?;
    let canonical = root.canonicalize().map_err(io_error)?;
    let raster =
        crate::mosaic::source_raster(&canonical, job, &job.asset_key, &CancellationToken::new())?;
    Ok(Source {
        decoder: raster.decoder.into_tiff()?,
        width: raster.width,
        height: raster.height,
        bounds: raster.bounds,
        pixel_size: raster.pixel_size,
        nodata: raster.nodata.map(|v| v as f32),
        deadline: Instant::now() + Duration::from_secs(55),
        signed_height: job.asset_key == "srtm",
    })
}
#[cfg(test)]
fn source<'a>(bytes: &'a [u8], item: &str) -> Result<Source<TimedReader<'a>>> {
    let deadline = Instant::now() + Duration::from_secs(55);
    let mut decoder = Decoder::new(TimedReader {
        bytes: Cursor::new(bytes),
        deadline,
        cancel: None,
    })
    .map_err(io_error)?;
    let h = validate_header(&mut decoder, item, None)?;
    Ok(Source {
        decoder,
        width: h.width,
        height: h.height,
        bounds: h.bounds,
        pixel_size: h.pixel_size,
        nodata: h.nodata,
        deadline,
        signed_height: false,
    })
}

fn chunks<R: Read + Seek>(source: &mut Source<R>) -> Result<[u32; 3]> {
    let (cw, ch) = source.decoder.chunk_dimensions();
    if cw == 0 || ch == 0 {
        return Err("Invalid elevation chunk grid".into());
    }
    let columns = source.width.div_ceil(cw);
    let count = match source.decoder.get_chunk_type() {
        ChunkType::Tile => source.decoder.tile_count().map_err(io_error)?,
        ChunkType::Strip => source.decoder.strip_count().map_err(io_error)?,
    };
    if columns.checked_mul(source.height.div_ceil(ch)) != Some(count) {
        return Err("Elevation chunks differ from its pixel grid".into());
    }
    Ok([cw, ch, columns])
}
fn read_chunk<R: Read + Seek>(source: &mut Source<R>, chunk: u32) -> Result<Vec<f32>> {
    check_time(source.deadline)?;
    let data = match source.decoder.read_chunk(chunk).map_err(io_error)? {
        DecodingResult::F32(values) if !source.signed_height => values,
        // Every Int16 value is exactly representable in f32. This conversion
        // only serves display/sampling; the file remains signed Int16.
        DecodingResult::I16(values) if source.signed_height => {
            values.into_iter().map(f32::from).collect()
        }
        _ => return Err("Elevation samples differ from their pinned product type".into()),
    };
    let (w, h) = source.decoder.chunk_data_dimensions(chunk);
    if data.len() != w as usize * h as usize
        || data
            .iter()
            .any(|v| v.is_infinite() || (v.is_nan() && !source.nodata.is_some_and(f32::is_nan)))
    {
        return Err("Elevation chunk has invalid sample values or dimensions".into());
    }
    Ok(data)
}
pub(crate) fn is_no_data(value: f32, nodata: Option<f32>) -> bool {
    nodata.is_some_and(|n| {
        if n.is_nan() {
            value.is_nan()
        } else {
            value == n
        }
    })
}
fn shade(value: f32, range: [f64; 2]) -> [u8; 3] {
    let gray = if range[0] == range[1] {
        128
    } else {
        ((f64::from(value) - range[0]) / (range[1] - range[0]))
            .clamp(0.0, 1.0)
            .mul_add(255.0, 0.0)
            .round() as u8
    };
    [gray, gray, gray]
}

pub(crate) fn inspect(root: &Path, job: &Job, edge: u32) -> Result<RasterInspection> {
    if edge == 0 || edge > PREVIEW_EDGE {
        return Err("Invalid elevation preview size".into());
    }
    let mut source = open_source(root, job)?;
    let max_edge = source.width.max(source.height);
    let pw = (source.width as u64 * edge.min(max_edge) as u64 / max_edge as u64).max(1) as u32;
    let ph = (source.height as u64 * edge.min(max_edge) as u64 / max_edge as u64).max(1) as u32;
    let [cw, ch, columns] = chunks(&mut source)?;
    let mut targets: BTreeMap<u32, Vec<(usize, u32, u32)>> = BTreeMap::new();
    for y in 0..ph {
        for x in 0..pw {
            let sx = (x as u64 * source.width as u64 / pw as u64) as u32;
            let sy = (y as u64 * source.height as u64 / ph as u64) as u32;
            targets
                .entry(sy / ch * columns + sx / cw)
                .or_default()
                .push(((y * pw + x) as usize, sx % cw, sy % ch));
        }
    }
    let mut samples = vec![0.0; (pw * ph) as usize];
    for (chunk, pixels) in targets {
        let values = read_chunk(&mut source, chunk)?;
        let (w, h) = source.decoder.chunk_data_dimensions(chunk);
        for (index, x, y) in pixels {
            if x >= w || y >= h {
                return Err("Elevation preview exceeds its chunk".into());
            }
            samples[index] = *values
                .get((y * w + x) as usize)
                .ok_or("Missing elevation sample")?;
        }
    }
    let mut valid: Vec<_> = samples
        .iter()
        .copied()
        .filter(|v| !is_no_data(*v, source.nodata))
        .collect();
    valid.sort_unstable_by(f32::total_cmp);
    let range = if valid.is_empty() {
        [0.0, 0.0]
    } else {
        [
            f64::from(valid[(valid.len() - 1) * 2 / 100]),
            f64::from(valid[(valid.len() - 1) * 98 / 100]),
        ]
    };
    let rgba: Vec<u8> = samples
        .iter()
        .flat_map(|v| {
            if is_no_data(*v, source.nodata) {
                [0, 0, 0, 0]
            } else {
                let c = shade(*v, range);
                [c[0], c[1], c[2], 255]
            }
        })
        .collect();
    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, pw, ph);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(io_error)?
        .write_image_data(&rgba)
        .map_err(io_error)?;
    check_time(source.deadline)?;
    Ok(RasterInspection {
        science: None,
        vegetation: None,
        quality: None,
        radar: None,
        aerial: None,
        width: source.width,
        height: source.height,
        band_count: 1,
        data_type: if source.signed_height {
            "Int16"
        } else {
            "Float32"
        }
        .into(),
        crs: "EPSG:4326".into(),
        bounds: source.bounds,
        pixel_size: source.pixel_size,
        nodata: source.nodata.filter(|v| v.is_finite()).map(f64::from),
        preview_data_url: format!("data:image/png;base64,{}", STANDARD.encode(png)),
        preview_width: pw,
        preview_height: ph,
        classes: Vec::new(),
        sha256: job.sha256.clone().ok_or("Missing elevation checksum")?,
        reflectance: None,
        elevation: Some(ElevationDisplay {
            product: job_profile(job)?.product,
            height_unit: "metre".into(),
            coordinate_unit: "degree".into(),
            vertical_reference: job_profile(job)?.vertical_reference,
            pixel_interpretation: "PixelIsPoint".into(),
            display_range: range,
            sample_count: pw * ph,
            valid_sample_count: valid.len() as u32,
            nodata_is_nan: source.nodata.is_some_and(f32::is_nan),
        }),
    })
}
pub(crate) fn sample(root: &Path, job: &Job, x: f64, y: f64) -> Result<RasterPixel> {
    let mut source = open_source(root, job)?;
    let [left, bottom, right, top] = source.bounds;
    if !x.is_finite() || !y.is_finite() || x < left || x >= right || y <= bottom || y > top {
        return Err("The coordinate is outside the source raster pixel grid".into());
    }
    let col = ((x - left) / source.pixel_size[0]).floor() as u32;
    let row = ((top - y) / source.pixel_size[1]).floor() as u32;
    if col >= source.width || row >= source.height {
        return Err("Elevation coordinate exceeds its source grid".into());
    }
    let [cw, ch, columns] = chunks(&mut source)?;
    let chunk = row / ch * columns + col / cw;
    let values = read_chunk(&mut source, chunk)?;
    let (w, h) = source.decoder.chunk_data_dimensions(chunk);
    if col % cw >= w || row % ch >= h {
        return Err("Elevation coordinate exceeds its source chunk".into());
    }
    let value = *values
        .get(((row % ch) * w + col % cw) as usize)
        .ok_or("Missing original elevation sample")?;
    Ok(RasterPixel {
        science: None,
        index_value: None,
        quality: None,
        decibels: None,
        near_infrared: None,
        job_id: job.id.clone(),
        sha256: job.sha256.clone().ok_or("Missing elevation checksum")?,
        crs: "EPSG:4326".into(),
        coordinate: [x, y],
        pixel: [col, row],
        center: [
            left + (col as f64 + 0.5) * source.pixel_size[0],
            top - (row as f64 + 0.5) * source.pixel_size[1],
        ],
        value: f64::from(value),
        values: None,
        reflectance: None,
        label: "Elevation".into(),
        color: "#808080".into(),
        is_no_data: is_no_data(value, source.nodata),
    })
}

#[cfg(test)]
mod tests;
