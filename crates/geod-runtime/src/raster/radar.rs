//! Linear gamma0 Float32 RTC originals. dB stretching is display-only.
use super::*;
use crate::{io_error, providers, storage};
use std::{collections::BTreeMap, io::BufReader};
use tiff::decoder::ChunkType;

#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Profile {
    pub product: String,
    pub polarization: String,
    pub quantity: String,
    pub unit: String,
}
pub(crate) fn profile(key: &str) -> Result<Profile> {
    if !providers::radar::KEYS.contains(&key) {
        return Err("Unsupported radar polarization".into());
    }
    Ok(Profile {
        product: "sentinel-1-iw-rtc".into(),
        polarization: key.to_uppercase(),
        quantity: "gamma0".into(),
        unit: "linear".into(),
    })
}
pub(crate) fn metadata(profile: &Profile) -> String {
    format!("<GDALMetadata><Item name=\"PRODUCT\">sentinel-1-iw-rtc</Item><Item name=\"POLARIZATION\" sample=\"0\">{}</Item><Item name=\"UNITTYPE\" sample=\"0\" role=\"unittype\">gamma0 (linear)</Item></GDALMetadata>", profile.polarization)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RadarDisplay {
    pub product: String,
    pub polarization: String,
    pub quantity: String,
    pub unit: String,
    pub display_unit: String,
    pub display_range: [f64; 2],
    pub sample_count: u32,
    pub valid_sample_count: u32,
    pub overview: bool,
}
pub(crate) struct Grid {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) crs: String,
    pub(crate) bounds: [f64; 4],
    pub(crate) pixel_size: [f64; 2],
}
type Reader = Decoder<BufReader<File>>;
#[cfg(test)]
mod tests;
fn optional_double_tag<R: Read + Seek>(d: &mut Decoder<R>, tag: Tag) -> Result<Option<Vec<f64>>> {
    d.find_tag(tag)
        .map_err(io_error)?
        .map(|v| v.into_f64_vec())
        .transpose()
        .map_err(io_error)
}

pub(crate) fn validate_job(job: &Job) -> Result<()> {
    let expected = profile(&job.asset_key)?;
    let url = providers::asset_url(&job.href)?;
    if url.host_str() != Some(providers::radar::HOST)
        || providers::radar::identity(url.path()).is_none_or(|(_, key)| key != job.asset_key)
    {
        return Err("Radar source provenance differs from its reviewed polarization URL".into());
    }
    if job.kind == "raster_mosaic" {
        crate::mosaic::validate_stored_mosaic(job)?;
        let spec = job.mosaic.as_ref().unwrap();
        let p = job
            .mosaic_output
            .as_ref()
            .ok_or("Radar result grid is missing")?;
        let count = u64::from(p.width) * u64::from(p.height);
        if job.status != JobStatus::Succeeded
            || extension(&job.media_type)? != "tif"
            || job.item_id != format!("project:{}", spec.project_id)
            || p.radar.as_ref() != Some(&expected)
            || p.calibration.is_some()
            || p.elevation.is_some()
            || p.aerial.is_some()
            || p.band_count != 1
            || p.source_count != spec.sources.len()
            || p.pixel_size != [10.0, 10.0]
            || count == 0
            || p.covered_pixels == 0
            || p.covered_pixels > count
            || p.masked_pixels > count
            || p.bounds.iter().any(|v| !v.is_finite())
            || p.bounds[0] >= p.bounds[2]
            || p.bounds[1] >= p.bounds[3]
            || p.bounds[2] - p.bounds[0] != f64::from(p.width) * 10.0
            || p.bounds[3] - p.bounds[1] != f64::from(p.height) * 10.0
        {
            return Err(
                "Radar result differs from its pinned linear gamma0 profile and grid".into(),
            );
        }
        return Ok(());
    }
    if job.kind != "download"
        || job.status != JobStatus::Succeeded
        || !providers::radar::KEYS.contains(&job.asset_key.as_str())
        || url.host_str() != Some(providers::radar::HOST)
        || !providers::matches_item(&url, &job.item_id, &job.asset_key)
        || extension(&job.media_type)? != "tif"
    {
        return Err(
            "Radar inspection requires a completed managed Sentinel-1 IW RTC original".into(),
        );
    }
    Ok(())
}
fn samples<R: Read + Seek>(d: &mut Decoder<R>) -> Result<()> {
    if d.colortype().map_err(io_error)? != ColorType::Gray(32)
        || d.get_tag_u16_vec(Tag::SampleFormat).map_err(io_error)? != [3]
        || d.find_tag(Tag::GdalNodata)
            .map_err(io_error)?
            .ok_or("RTC NoData is missing")?
            .into_string()
            .map_err(io_error)?
            .trim_matches('\0')
            .trim()
            != "-32768"
    {
        return Err("RTC requires one Float32 gamma0 band with -32768 NoData".into());
    }
    Ok(())
}
pub(crate) fn validate_header<R: Read + Seek>(
    d: &mut Decoder<R>,
    plan: Option<&crate::mosaic::MosaicPlan>,
) -> Result<Grid> {
    samples(d)?;
    let (width, height) = d.dimensions().map_err(io_error)?;
    if width == 0 || height == 0 || plan.is_none() && (width > 40000 || height > 40000) {
        return Err("RTC grid exceeds the supported IW dimensions".into());
    }
    let keys = d
        .get_tag_u16_vec(Tag::GeoKeyDirectoryTag)
        .map_err(io_error)?;
    let crs = validate_geokeys(&keys)?;
    let transform = optional_double_tag(d, Tag::ModelTransformationTag)?;
    let scale = optional_double_tag(d, Tag::ModelPixelScaleTag)?;
    let tiepoint = optional_double_tag(d, Tag::ModelTiepointTag)?;
    let (bounds, pixel_size) = georeference(
        width,
        height,
        transform.as_deref(),
        scale.as_deref(),
        tiepoint.as_deref(),
    )?;
    if pixel_size != [10.0, 10.0] {
        return Err("RTC original must use a 10 metre grid".into());
    }
    if let Some(p) = plan {
        let expected = p
            .radar
            .as_ref()
            .ok_or("Radar result has no polarization profile")?;
        let embedded = d
            .get_tag(Tag::Unknown(42112))
            .map_err(io_error)?
            .into_string()
            .map_err(io_error)?;
        if p.width != width
            || p.height != height
            || p.crs != crs
            || p.bounds != bounds
            || p.pixel_size != pixel_size
            || embedded.trim_matches('\0') != metadata(expected)
        {
            return Err(
                "Radar result header differs from its recorded grid or polarization".into(),
            );
        }
    }
    Ok(Grid {
        width,
        height,
        crs,
        bounds,
        pixel_size,
    })
}
fn open(root: &Path, job: &Job) -> Result<(Reader, Grid, Instant)> {
    validate_job(job)?;
    let deadline = Instant::now() + Duration::from_secs(60);
    let path = storage::verified_output_path(root, job)?;
    let mut options = File::options();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1);
    }
    let mut file = options.open(path).map_err(io_error)?;
    let size = file.metadata().map_err(io_error)?.len();
    if size == 0
        || job.kind != "raster_mosaic" && size > providers::radar::MAX_BYTES
        || size != job.bytes_downloaded
        || job.total_bytes.is_some_and(|v| v != size)
    {
        return Err("RTC source byte count no longer matches its job".into());
    }
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        check_time(deadline)?;
        let count = file.read(&mut buffer).map_err(io_error)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    if Some(format!("{:x}", hash.finalize()).as_str()) != job.sha256.as_deref() {
        return Err("RTC source SHA-256 changed after download".into());
    }
    file.seek(SeekFrom::Start(0)).map_err(io_error)?;
    let mut limits = Limits::default();
    limits.decoding_buffer_size = 32 * 1024 * 1024;
    limits.intermediate_buffer_size = 32 * 1024 * 1024;
    limits.ifd_value_size = 1024 * 1024;
    let mut decoder = Decoder::new(BufReader::new(file))
        .map_err(io_error)?
        .with_limits(limits);
    let grid = validate_header(&mut decoder, job.mosaic_output.as_ref())?;
    Ok((decoder, grid, deadline))
}
fn chunks<R: Read + Seek>(d: &mut Decoder<R>, width: u32, height: u32) -> Result<[u32; 3]> {
    let (cw, ch) = d.chunk_dimensions();
    if cw == 0 || ch == 0 || u64::from(cw) * u64::from(ch) > 8 * 1024 * 1024 {
        return Err("RTC chunk exceeds the bounded decoder".into());
    }
    let count = match d.get_chunk_type() {
        ChunkType::Tile => d.tile_count(),
        ChunkType::Strip => d.strip_count(),
    }
    .map_err(io_error)?;
    let columns = width.div_ceil(cw);
    if columns.checked_mul(height.div_ceil(ch)) != Some(count) {
        return Err("RTC chunk layout differs from its grid".into());
    }
    Ok([cw, ch, columns])
}
fn read<R: Read + Seek>(d: &mut Decoder<R>, index: u32, deadline: Instant) -> Result<Vec<f32>> {
    check_time(deadline)?;
    let DecodingResult::F32(values) = d.read_chunk(index).map_err(io_error)? else {
        return Err("RTC decoded samples are not Float32".into());
    };
    let (w, h) = d.chunk_data_dimensions(index);
    if values.len() != w as usize * h as usize
        || values
            .iter()
            .any(|v| !v.is_finite() || *v < 0.0 && *v != -32768.0)
    {
        return Err("RTC contains invalid gamma0 samples".into());
    }
    Ok(values)
}
fn decibels(value: f32) -> Option<f64> {
    (value > 0.0).then(|| 10.0 * f64::from(value).log10())
}
fn shade(value: f32, range: [f64; 2]) -> u8 {
    let Some(db) = decibels(value) else { return 0 };
    if range[0] == range[1] {
        128
    } else {
        (((db - range[0]) / (range[1] - range[0])).clamp(0.0, 1.0) * 255.0).round() as u8
    }
}
pub(crate) fn inspect(root: &Path, job: &Job, edge: u32) -> Result<RasterInspection> {
    if edge == 0 || edge > PREVIEW_EDGE {
        return Err("Invalid radar preview size".into());
    }
    let (mut d, g, deadline) = open(root, job)?;
    let mut best = 0;
    let mut dims = [g.width, g.height];
    let mut best_edge = g.width.max(g.height);
    for index in 1..=16 {
        if !d.more_images() {
            break;
        }
        check_time(deadline)?;
        d.next_image().map_err(io_error)?;
        let (w, h) = d.dimensions().map_err(io_error)?;
        if w > 0
            && h > 0
            && w <= g.width
            && h <= g.height
            && w.max(h) >= edge
            && w.max(h) < best_edge
            && samples(&mut d).is_ok()
            && (f64::from(w) / f64::from(h) - f64::from(g.width) / f64::from(g.height)).abs() < 0.01
        {
            best = index;
            dims = [w, h];
            best_edge = w.max(h);
        }
    }
    d.seek_to_image(best).map_err(io_error)?;
    samples(&mut d)?;
    let [w, h] = dims;
    let max_edge = w.max(h);
    let pw = (u64::from(w) * u64::from(edge.min(max_edge)) / u64::from(max_edge)).max(1) as u32;
    let ph = (u64::from(h) * u64::from(edge.min(max_edge)) / u64::from(max_edge)).max(1) as u32;
    let [cw, ch, columns] = chunks(&mut d, w, h)?;
    let mut targets: BTreeMap<u32, Vec<(usize, u32, u32)>> = BTreeMap::new();
    for y in 0..ph {
        for x in 0..pw {
            let sx = (u64::from(x) * u64::from(w) / u64::from(pw)) as u32;
            let sy = (u64::from(y) * u64::from(h) / u64::from(ph)) as u32;
            targets
                .entry(sy / ch * columns + sx / cw)
                .or_default()
                .push(((y * pw + x) as usize, sx % cw, sy % ch));
        }
    }
    let mut values = vec![-32768.0; (pw * ph) as usize];
    for (index, pixels) in targets {
        let data = read(&mut d, index, deadline)?;
        let (width, _) = d.chunk_data_dimensions(index);
        for (i, x, y) in pixels {
            values[i] = *data
                .get((y * width + x) as usize)
                .ok_or("RTC preview exceeds its chunk")?;
        }
    }
    let mut positive: Vec<_> = values.iter().filter_map(|v| decibels(*v)).collect();
    positive.sort_unstable_by(f64::total_cmp);
    let range = if positive.is_empty() {
        [0.0, 0.0]
    } else {
        [
            positive[(positive.len() - 1) * 2 / 100],
            positive[(positive.len() - 1) * 98 / 100],
        ]
    };
    let rgba: Vec<_> = values
        .iter()
        .flat_map(|v| {
            let c = shade(*v, range);
            [c, c, c, if *v == -32768.0 { 0 } else { 255 }]
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
    check_time(deadline)?;
    Ok(RasterInspection {
        science: None,
        vegetation: None,
        width: g.width,
        height: g.height,
        band_count: 1,
        data_type: "Float32".into(),
        crs: g.crs,
        bounds: g.bounds,
        pixel_size: g.pixel_size,
        nodata: Some(-32768.0),
        preview_data_url: format!("data:image/png;base64,{}", STANDARD.encode(png)),
        preview_width: pw,
        preview_height: ph,
        classes: Vec::new(),
        sha256: job.sha256.clone().ok_or("Missing radar checksum")?,
        reflectance: None,
        elevation: None,
        aerial: None,
        quality: None,
        radar: Some(RadarDisplay {
            product: "sentinel-1-iw-rtc".into(),
            polarization: job.asset_key.to_uppercase(),
            quantity: "gamma0".into(),
            unit: "linear".into(),
            display_unit: "dB".into(),
            display_range: range,
            sample_count: pw * ph,
            valid_sample_count: values.iter().filter(|v| **v != -32768.0).count() as u32,
            overview: best != 0,
        }),
    })
}
pub(crate) fn sample(root: &Path, job: &Job, x: f64, y: f64) -> Result<RasterPixel> {
    let (mut d, g, deadline) = open(root, job)?;
    let [left, bottom, right, top] = g.bounds;
    if !x.is_finite() || !y.is_finite() || x < left || x >= right || y <= bottom || y > top {
        return Err("The coordinate is outside the source raster pixel grid".into());
    }
    let col = ((x - left) / g.pixel_size[0]).floor() as u32;
    let row = ((top - y) / g.pixel_size[1]).floor() as u32;
    let [cw, ch, columns] = chunks(&mut d, g.width, g.height)?;
    let index = row / ch * columns + col / cw;
    let values = read(&mut d, index, deadline)?;
    let (width, _) = d.chunk_data_dimensions(index);
    let value = *values
        .get(((row % ch) * width + col % cw) as usize)
        .ok_or("RTC pixel exceeds its chunk")?;
    let no_data = value == -32768.0;
    Ok(RasterPixel {
        science: None,
        index_value: None,
        job_id: job.id.clone(),
        sha256: job.sha256.clone().ok_or("Missing radar checksum")?,
        crs: g.crs,
        coordinate: [x, y],
        pixel: [col, row],
        center: [
            left + (f64::from(col) + 0.5) * g.pixel_size[0],
            top - (f64::from(row) + 0.5) * g.pixel_size[1],
        ],
        value: f64::from(value),
        values: None,
        near_infrared: None,
        reflectance: None,
        decibels: decibels(value),
        quality: None,
        label: if no_data {
            "No data".into()
        } else {
            format!("Gamma0 · {}", job.asset_key.to_uppercase())
        },
        color: if no_data { "#000000" } else { "#808080" }.into(),
        is_no_data: no_data,
    })
}
