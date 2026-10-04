//! Original Landsat C2 L2 UInt16 and HLS L30 v2 Int16 values. The grayscale
//! stretch is display-only, computed from nearest-neighbour source samples.
use super::*;
use crate::{io_error, providers, storage, MAX_ASSET_BYTES};
use serde::Deserialize;
use std::collections::BTreeMap;
use tiff::decoder::ChunkType;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReflectanceDisplay {
    pub product: String,
    pub band: String,
    pub scale: f64,
    pub offset: f64,
    pub display_range: [i32; 2],
    pub sample_count: u32,
    pub valid_sample_count: u32,
    pub pixel_interpretation: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VegetationDisplay {
    pub product: String,
    pub index: String,
    pub scale: f64,
    pub offset: f64,
    pub valid_range: [i32; 2],
    pub display_range: [i32; 2],
    pub palette: String,
    pub sample_count: u32,
    pub valid_sample_count: u32,
    pub out_of_range_sample_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality_selection: Option<crate::mosaic::vegetation::SelectionResult>,
    pub pixel_interpretation: String,
}

pub(crate) const VEGETATION_PALETTE: &str = "modis-vi-v1";

pub(crate) fn vegetation_color(value: i32) -> [u8; 3] {
    const STOPS: &[(i32, [u8; 3])] = &[
        (-2000, [42, 91, 135]),
        (0, [208, 169, 104]),
        (2000, [235, 223, 160]),
        (4000, [161, 196, 112]),
        (7000, [75, 143, 76]),
        (10000, [24, 86, 51]),
    ];
    let value = value.clamp(-2000, 10000);
    let pair = STOPS.windows(2).find(|p| value <= p[1].0).unwrap();
    let span = pair[1].0 - pair[0].0;
    std::array::from_fn(|i| {
        // Integer interpolation keeps exact half ties stable across decoders.
        let weighted = i32::from(pair[0].1[i]) * (pair[1].0 - value)
            + i32::from(pair[1].1[i]) * (value - pair[0].0);
        ((weighted + span / 2) / span) as u8
    })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Profile {
    pub(crate) product: String,
    pub(crate) signed: bool,
    pub(crate) scale: f64,
    pub(crate) offset: f64,
    pub(crate) nodata: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) sample_bits: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) science_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) calendar_year: Option<i32>,
}
impl Profile {
    pub(crate) fn bits(&self) -> u8 {
        self.sample_bits.unwrap_or(16)
    }
}

pub(crate) fn profile(job: &Job) -> Result<Profile> {
    if job.status != JobStatus::Succeeded
        || !(matches!(
            job.asset_key.as_str(),
            "red" | "green" | "blue" | "ndvi" | "evi"
        ) || providers::vegetation::SCIENCE_KEYS.contains(&job.asset_key.as_str()))
        || extension(&job.media_type)? != "tif"
        || uuid::Uuid::parse_str(&job.id)
            .ok()
            .map(|id| id.to_string())
            .as_deref()
            != Some(&job.id)
    {
        return Err("Reflectance inspection requires a completed managed source band".into());
    }
    let url = providers::asset_url(&job.href)?;
    if job.kind == "raster_prepare" {
        providers::viirs::prepare::validate_stored(job)?;
        return Ok(providers::viirs::prepare::profile());
    }
    let source_item = if job.kind == "raster_mosaic" {
        if url.host_str() == Some(providers::modis::HOST) {
            providers::modis::item_from_path(url.path(), &job.asset_key)
                .or_else(|| providers::vegetation::item_from_path(url.path(), &job.asset_key))
                .ok_or("Processed MODIS band source product is invalid")?
                .into()
        } else if url.host_str() == Some(providers::nasa::HOST) {
            url.path_segments()
                .and_then(|parts| parts.rev().nth(1))
                .unwrap_or("")
                .to_string()
        } else {
            let product = url
                .path_segments()
                .and_then(|parts| parts.rev().nth(1))
                .unwrap_or("");
            let parts: Vec<_> = product.split('_').collect();
            if parts.len() != 7 {
                return Err("Processed band source product is invalid".into());
            }
            [parts[0], parts[1], parts[2], parts[3], parts[5], parts[6]].join("_")
        }
    } else {
        job.item_id.clone()
    };
    let viirs = url.host_str() == Some(providers::nasa::HOST)
        && providers::viirs::matches(url.path(), &source_item);
    if !providers::matches_item(
        &url,
        &source_item,
        if viirs { "viirs" } else { &job.asset_key },
    ) {
        return Err("Reflectance band does not match its source product and channel".into());
    }
    // These transformations are specific to the reviewed versioned products,
    // never inferred from arbitrary grayscale files or user-supplied metadata.
    let expected = match url.host_str() {
        Some(providers::LANDSAT_HOST) => Ok::<Profile, String>(Profile {
            product: "landsat-c2-l2".into(),
            signed: false,
            scale: 0.0000275,
            offset: -0.2,
            nodata: 0,
            sample_bits: None,
            science_key: None,
            calendar_year: None,
        }),
        Some(providers::modis::HOST)
            if providers::vegetation::SCIENCE_KEYS.contains(&job.asset_key.as_str()) =>
        {
            science::profile(&job.asset_key, &source_item)
        }
        Some(providers::modis::HOST)
            if providers::vegetation::KEYS.contains(&job.asset_key.as_str()) =>
        {
            Ok(Profile {
                product: providers::vegetation::PRODUCT.into(),
                signed: true,
                scale: 0.0001,
                offset: 0.0,
                nodata: -3000,
                sample_bits: None,
                science_key: None,
                calendar_year: None,
            })
        }
        Some(providers::modis::HOST) => Ok(Profile {
            product: "modis-09a1-v061".into(),
            signed: true,
            scale: 0.0001,
            offset: 0.0,
            nodata: -28672,
            sample_bits: None,
            science_key: None,
            calendar_year: None,
        }),
        Some(providers::nasa::HOST) if viirs => Ok(providers::viirs::prepare::profile()),
        Some(providers::nasa::HOST) => Ok(Profile {
            product: "hls-l30-v2".into(),
            signed: true,
            scale: 0.0001,
            offset: 0.0,
            nodata: -9999,
            sample_bits: None,
            science_key: None,
            calendar_year: None,
        }),
        _ => Err("Unsupported reflectance product calibration".into()),
    }?;
    if job.kind == "raster_mosaic" {
        crate::mosaic::validate_stored_mosaic(job)?;
        let spec = job.mosaic.as_ref().unwrap();
        if job.item_id != format!("project:{}", spec.project_id)
            || job
                .mosaic_output
                .as_ref()
                .and_then(|plan| plan.calibration.as_ref())
                != Some(&expected)
        {
            return Err("Processed band calibration differs from its pinned product".into());
        }
    } else if job.kind != "download" {
        return Err("Unsupported reflectance operation".into());
    } else if viirs {
        return Err("VIIRS science bands must be prepared from the checked HDF5".into());
    }
    Ok(expected)
}

pub(crate) struct Snapshot {
    bytes: Cursor<Vec<u8>>,
    deadline: Instant,
}
impl Read for Snapshot {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if Instant::now() > self.deadline {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Reflectance inspection time limit",
            ));
        }
        self.bytes.read(output)
    }
}
impl Seek for Snapshot {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.bytes.seek(position)
    }
}

struct Source {
    decoder: Decoder<Snapshot>,
    width: u32,
    height: u32,
    crs: String,
    bounds: [f64; 4],
    pixel_size: [f64; 2],
    profile: Profile,
    deadline: Instant,
    pixel_is_point: bool,
}

fn source(root: &Path, job: &Job) -> Result<Source> {
    let deadline = Instant::now() + Duration::from_secs(55);
    source_with_deadline(root, job, deadline)
}

fn source_with_deadline(root: &Path, job: &Job, deadline: Instant) -> Result<Source> {
    let profile = profile(job)?;
    let mut decoder = verified_decoder(root, job, deadline)?;
    let Header {
        width,
        height,
        crs,
        bounds,
        pixel_size,
        pixel_is_point,
    } = validate_header(&mut decoder, &profile)?;
    if job.kind == "raster_prepare" {
        providers::viirs::prepare::grid(
            &Header {
                width,
                height,
                crs: crs.clone(),
                bounds,
                pixel_size,
                pixel_is_point,
            },
            job,
        )?;
    }
    if profile.product == "modis-09a1-v061" && job.kind == "download" {
        modis::grid(
            &Header {
                width,
                height,
                crs: crs.clone(),
                bounds,
                pixel_size,
                pixel_is_point,
            },
            &job.item_id,
        )?;
    }
    if profile.product == providers::vegetation::PRODUCT && job.kind == "download" {
        modis::vegetation_grid(
            &Header {
                width,
                height,
                crs: crs.clone(),
                bounds,
                pixel_size,
                pixel_is_point,
            },
            &job.item_id,
        )?;
    }
    if let Some(plan) = job
        .mosaic_output
        .as_ref()
        .filter(|_| job.kind == "raster_mosaic")
    {
        if plan.width != width
            || plan.height != height
            || plan.band_count != 1
            || plan.crs != crs
            || plan.bounds != bounds
            || plan.pixel_size != pixel_size
            || pixel_is_point
        {
            return Err("Processed band geometry differs from its recorded output".into());
        }
    }
    science::validate_metadata(&mut decoder, job, &profile)?;
    crate::mosaic::vegetation::validate_header(&mut decoder, job)?;
    Ok(Source {
        decoder,
        width,
        height,
        crs,
        bounds,
        pixel_size,
        profile,
        deadline,
        pixel_is_point,
    })
}

// Inspection and scientific RGB both decode this immutable, checksum-verified
// snapshot. They do not race a later disk read against an earlier checksum.
pub(crate) fn verified_decoder(
    root: &Path,
    job: &Job,
    deadline: Instant,
) -> Result<Decoder<Snapshot>> {
    let path = storage::verified_output_path(root, job)?;
    let mut file = File::open(path).map_err(io_error)?;
    let size = file.metadata().map_err(io_error)?.len();
    if size == 0
        || size > MAX_ASSET_BYTES
        || size != job.bytes_downloaded
        || job.total_bytes.is_some_and(|total| total != size)
    {
        return Err("Reflectance source byte count is invalid or exceeds 512 MiB".into());
    }
    let mut bytes = Vec::with_capacity(size as usize);
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        check_time(deadline)?;
        let count = file.read(&mut buffer).map_err(io_error)?;
        if count == 0 {
            break;
        }
        if bytes.len() as u64 + count as u64 > size {
            return Err("Reflectance source changed while reading".into());
        }
        hash.update(&buffer[..count]);
        bytes.extend_from_slice(&buffer[..count]);
    }
    if bytes.len() as u64 != size
        || Some(format!("{:x}", hash.finalize())).as_ref() != job.sha256.as_ref()
    {
        return Err("Reflectance source SHA-256 or size changed after download".into());
    }
    // All subsequent metadata and samples come from the exact verified bytes.
    let mut limits = Limits::default();
    limits.decoding_buffer_size = 32 * 1024 * 1024;
    limits.intermediate_buffer_size = 32 * 1024 * 1024;
    limits.ifd_value_size = 1024 * 1024;
    let decoder = Decoder::new(Snapshot {
        bytes: Cursor::new(bytes),
        deadline,
    })
    .map_err(io_error)?
    .with_limits(limits);
    Ok(decoder)
}

pub(crate) struct Header {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) crs: String,
    pub(crate) bounds: [f64; 4],
    pub(crate) pixel_size: [f64; 2],
    pub(crate) pixel_is_point: bool,
}

pub(crate) fn validate_header<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    profile: &Profile,
) -> Result<Header> {
    validate_sample_header(decoder, profile, 1)
}

fn validate_sample_header<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    profile: &Profile,
    bands: u16,
) -> Result<Header> {
    let (width, height) = decoder.dimensions().map_err(io_error)?;
    if width == 0 || height == 0 || width > 20000 || height > 20000 {
        return Err("Reflectance source dimensions exceed the 20000-pixel edge limit".into());
    }
    if !matches!(bands, 1 | 3)
        || decoder.colortype().map_err(io_error)?
            != if bands == 1 {
                ColorType::Gray(profile.bits())
            } else {
                ColorType::RGB(profile.bits())
            }
        || decoder
            .find_tag_unsigned::<u16>(Tag::SamplesPerPixel)
            .map_err(io_error)?
            .unwrap_or(1)
            != bands
        || decoder
            .find_tag_unsigned_vec::<u16>(Tag::SampleFormat)
            .map_err(io_error)?
            .unwrap_or(vec![1; bands as usize])
            != vec![if profile.signed { 2 } else { 1 }; bands as usize]
        || decoder
            .find_tag_unsigned::<u16>(Tag::Orientation)
            .map_err(io_error)?
            .unwrap_or(1)
            != 1
        || decoder
            .find_tag_unsigned::<u16>(Tag::PhotometricInterpretation)
            .map_err(io_error)?
            != Some(if bands == 1 { 1 } else { 2 })
        || decoder
            .find_tag_unsigned::<u16>(Tag::PlanarConfiguration)
            .map_err(io_error)?
            .unwrap_or(1)
            != 1
    {
        return Err(format!(
            "GeoTIFF samples do not match the product's original {}-bit band type",
            profile.bits()
        ));
    }
    let mut keys = decoder
        .get_tag_u16_vec(Tag::GeoKeyDirectoryTag)
        .map_err(io_error)?;
    let entry_end =
        4 + 4 * usize::from(*keys.get(3).ok_or("Missing GeoTIFF key directory header")?);
    if keys.len() < entry_end {
        return Err("Malformed GeoTIFF key directory".into());
    }
    let mut pixel_is_point = false;
    for entry in keys[4..entry_end].chunks_exact_mut(4) {
        if entry == [1025, 0, 1, 2] {
            pixel_is_point = true;
            entry[3] = 1;
        }
    }
    // Retain all existing CRS/unit/duplicate validation. Point coordinates are
    // converted below, rather than silently treating their centres as edges.
    let crs = if matches!(
        profile.product.as_str(),
        "modis-09a1-v061" | "modis-13q1-v061" | "viirs-09a1-v002"
    ) {
        if pixel_is_point {
            return Err("MODIS COG must use its original Area grid".into());
        }
        let actual = modis::crs(decoder, &keys)?;
        if profile.product == "viirs-09a1-v002" {
            providers::viirs::hdf::CRS.into()
        } else {
            actual
        }
    } else {
        validate_geokeys(&keys)?
    };
    let transform = decoder
        .find_tag(Tag::ModelTransformationTag)
        .map_err(io_error)?
        .map(|value| value.into_f64_vec())
        .transpose()
        .map_err(io_error)?;
    let scale = decoder
        .find_tag(Tag::ModelPixelScaleTag)
        .map_err(io_error)?
        .map(|value| value.into_f64_vec())
        .transpose()
        .map_err(io_error)?;
    let tiepoint = decoder
        .find_tag(Tag::ModelTiepointTag)
        .map_err(io_error)?
        .map(|value| value.into_f64_vec())
        .transpose()
        .map_err(io_error)?;
    let (mut bounds, pixel_size) = georeference(
        width,
        height,
        transform.as_deref(),
        scale.as_deref(),
        tiepoint.as_deref(),
    )?;
    if pixel_is_point {
        bounds[0] -= pixel_size[0] / 2.0;
        bounds[2] -= pixel_size[0] / 2.0;
        bounds[1] += pixel_size[1] / 2.0;
        bounds[3] += pixel_size[1] / 2.0;
    }
    let expected_size = if profile.product == providers::vegetation::PRODUCT {
        providers::vegetation::PIXEL
    } else if profile.product == "modis-09a1-v061" {
        providers::modis::PIXEL
    } else if profile.product == "viirs-09a1-v002" {
        std::f64::consts::PI * providers::modis::RADIUS / 18.0 / 1200.0
    } else {
        30.0
    };
    if pixel_size
        .iter()
        .any(|size| (*size - expected_size).abs() > 1e-6)
    {
        return Err("Reflectance source does not have its expected product grid".into());
    }
    let nodata = decoder
        .get_tag(Tag::GdalNodata)
        .map_err(io_error)?
        .into_string()
        .map_err(io_error)?;
    if nodata
        .trim_matches('\0')
        .trim()
        .parse::<i32>()
        .map_err(io_error)?
        != profile.nodata
    {
        return Err("GeoTIFF NoData does not match the reflectance product".into());
    }
    Ok(Header {
        width,
        height,
        crs,
        bounds,
        pixel_size,
        pixel_is_point,
    })
}

fn chunk_grid(source: &mut Source) -> Result<[u32; 3]> {
    let (cw, ch) = source.decoder.chunk_dimensions();
    if cw == 0 || ch == 0 {
        return Err("Reflectance chunks are invalid".into());
    }
    let columns = source.width.div_ceil(cw);
    let count = match source.decoder.get_chunk_type() {
        ChunkType::Tile => source.decoder.tile_count().map_err(io_error)?,
        ChunkType::Strip => source.decoder.strip_count().map_err(io_error)?,
    };
    if columns.checked_mul(source.height.div_ceil(ch)) != Some(count) {
        return Err("Reflectance chunks do not match the source grid".into());
    }
    Ok([cw, ch, columns])
}

enum Samples {
    Unsigned(Vec<u16>),
    Signed(Vec<i16>),
    Signed8(Vec<i8>),
}
impl Samples {
    fn get(&self, index: usize) -> Option<i32> {
        match self {
            Self::Unsigned(data) => data.get(index).copied().map(i32::from),
            Self::Signed(data) => data.get(index).copied().map(i32::from),
            Self::Signed8(data) => data.get(index).copied().map(i32::from),
        }
    }
    fn len(&self) -> usize {
        match self {
            Self::Unsigned(data) => data.len(),
            Self::Signed(data) => data.len(),
            Self::Signed8(data) => data.len(),
        }
    }
}
fn read_chunk(source: &mut Source, index: u32) -> Result<Samples> {
    check_time(source.deadline)?;
    let values = match source.decoder.read_chunk(index).map_err(io_error)? {
        DecodingResult::U16(values) if !source.profile.signed && source.profile.bits() == 16 => {
            Samples::Unsigned(values)
        }
        DecodingResult::I16(values) if source.profile.signed && source.profile.bits() == 16 => {
            Samples::Signed(values)
        }
        DecodingResult::I8(values) if source.profile.signed && source.profile.bits() == 8 => {
            Samples::Signed8(values)
        }
        _ => return Err("Decoded samples differ from the original band type".into()),
    };
    let (width, height) = source.decoder.chunk_data_dimensions(index);
    if values.len() != width as usize * height as usize {
        return Err("Reflectance chunk sample count is inconsistent".into());
    }
    Ok(values)
}

pub(crate) fn inspect(root: &Path, job: &Job, edge: u32) -> Result<RasterInspection> {
    if edge == 0 || edge > PREVIEW_EDGE {
        return Err("Invalid reflectance preview size".into());
    }
    let mut source = source(root, job)?;
    let vegetation = providers::vegetation::KEYS.contains(&job.asset_key.as_str());
    let scientific = providers::vegetation::SCIENCE_KEYS.contains(&job.asset_key.as_str());
    let longest = source.width.max(source.height);
    let pw = (source.width as u64 * edge.min(longest) as u64 / longest as u64).max(1) as u32;
    let ph = (source.height as u64 * edge.min(longest) as u64 / longest as u64).max(1) as u32;
    let [cw, ch, columns] = chunk_grid(&mut source)?;
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
    let mut preview = vec![source.profile.nodata; (pw * ph) as usize];
    for (chunk, pixels) in targets {
        let values = read_chunk(&mut source, chunk)?;
        let (width, height) = source.decoder.chunk_data_dimensions(chunk);
        for (index, x, y) in pixels {
            if x >= width || y >= height {
                return Err("Reflectance preview is outside its source chunk".into());
            }
            preview[index] = values
                .get((y * width + x) as usize)
                .ok_or("Missing reflectance sample")?;
        }
    }
    let mut valid: Vec<_> = preview
        .iter()
        .copied()
        .filter(|value| *value != source.profile.nodata)
        .collect();
    valid.sort_unstable();
    let display_range = if vegetation {
        [-2000, 10000]
    } else if scientific {
        providers::vegetation::layer(&job.asset_key).unwrap().range
    } else if valid.is_empty() {
        [0, 0]
    } else {
        [
            valid[(valid.len() - 1) * 2 / 100],
            valid[(valid.len() - 1) * 98 / 100],
        ]
    };
    let rgba: Vec<u8> = preview
        .iter()
        .flat_map(|value| {
            let alpha = if *value == source.profile.nodata {
                0
            } else {
                255
            };
            let gray = if display_range[0] == display_range[1] {
                128
            } else {
                (((*value - display_range[0]) as f64
                    / (display_range[1] - display_range[0]) as f64)
                    .clamp(0.0, 1.0)
                    * 255.0)
                    .round() as u8
            };
            let [r, g, b] = if vegetation {
                vegetation_color(*value)
            } else if scientific {
                science::color(&job.asset_key, *value)
            } else {
                [gray; 3]
            };
            [r, g, b, alpha]
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
        science: scientific.then(|| science::display(&job.asset_key, &source.profile, &preview)),
        vegetation: vegetation.then(|| VegetationDisplay {
            product: source.profile.product.clone(),
            index: job.asset_key.clone(),
            scale: source.profile.scale,
            offset: source.profile.offset,
            valid_range: [-2000, 10000],
            display_range,
            palette: VEGETATION_PALETTE.into(),
            sample_count: pw * ph,
            valid_sample_count: valid.len() as u32,
            out_of_range_sample_count: valid
                .iter()
                .filter(|v| !(-2000..=10000).contains(*v))
                .count() as u32,
            pixel_interpretation: "PixelIsArea".into(),
            quality_selection: job
                .mosaic_output
                .as_ref()
                .and_then(|p| p.vi_quality.clone()),
        }),
        quality: None,
        radar: None,
        aerial: None,
        width: source.width,
        height: source.height,
        band_count: 1,
        data_type: if source.profile.bits() == 8 {
            "Int8"
        } else if source.profile.signed {
            "Int16"
        } else {
            "UInt16"
        }
        .into(),
        crs: source.crs,
        bounds: source.bounds,
        pixel_size: source.pixel_size,
        nodata: Some(f64::from(source.profile.nodata)),
        preview_width: pw,
        preview_height: ph,
        preview_data_url: format!("data:image/png;base64,{}", STANDARD.encode(png)),
        classes: if scientific {
            science::classes(&job.asset_key, &preview)
        } else {
            Vec::new()
        },
        sha256: job.sha256.clone().ok_or("Missing reflectance checksum")?,
        elevation: None,
        reflectance: (!vegetation && !scientific).then(|| ReflectanceDisplay {
            product: source.profile.product,
            band: job.asset_key.clone(),
            scale: source.profile.scale,
            offset: source.profile.offset,
            display_range,
            sample_count: pw * ph,
            valid_sample_count: valid.len() as u32,
            pixel_interpretation: if source.pixel_is_point {
                "PixelIsPoint"
            } else {
                "PixelIsArea"
            }
            .into(),
        }),
    })
}

pub(crate) fn sample(root: &Path, job: &Job, x: f64, y: f64) -> Result<RasterPixel> {
    let mut source = source(root, job)?;
    let vegetation = providers::vegetation::KEYS.contains(&job.asset_key.as_str());
    let scientific = providers::vegetation::SCIENCE_KEYS.contains(&job.asset_key.as_str());
    let [left, bottom, right, top] = source.bounds;
    if !x.is_finite() || !y.is_finite() || x < left || x >= right || y <= bottom || y > top {
        return Err("The coordinate is outside the source raster pixel grid".into());
    }
    let col = ((x - left) / source.pixel_size[0]).floor() as u32;
    let row = ((top - y) / source.pixel_size[1]).floor() as u32;
    if col >= source.width || row >= source.height {
        return Err("Pixel is outside the source grid".into());
    }
    let [cw, ch, columns] = chunk_grid(&mut source)?;
    let index = row / ch * columns + col / cw;
    let (width, height) = source.decoder.chunk_data_dimensions(index);
    if col % cw >= width || row % ch >= height {
        return Err("Pixel is outside its source chunk".into());
    }
    let values = read_chunk(&mut source, index)?;
    let value = values
        .get(((row % ch) * width + col % cw) as usize)
        .ok_or("Missing original DN")?;
    let is_no_data = value == source.profile.nodata;
    let reflectance =
        (!is_no_data).then_some(value as f64 * source.profile.scale + source.profile.offset);
    let gray = (reflectance.unwrap_or(0.0).clamp(0.0, 1.0) * 255.0).round() as u8;
    let [r, g, b] = if vegetation {
        vegetation_color(value)
    } else if scientific {
        science::color(&job.asset_key, value)
    } else {
        [gray; 3]
    };
    Ok(RasterPixel {
        science: scientific
            .then(|| science::pixel(&job.asset_key, value, source.profile.calendar_year)),
        index_value: if vegetation { reflectance } else { None },
        quality: None,
        decibels: None,
        near_infrared: None,
        job_id: job.id.clone(),
        sha256: job.sha256.clone().ok_or("Missing reflectance checksum")?,
        crs: source.crs,
        coordinate: [x, y],
        pixel: [col, row],
        center: [
            left + (col as f64 + 0.5) * source.pixel_size[0],
            top - (row as f64 + 0.5) * source.pixel_size[1],
        ],
        value: f64::from(value),
        values: None,
        reflectance: if vegetation || scientific {
            None
        } else {
            reflectance
        },
        label: if vegetation {
            format!("{} vegetation index", job.asset_key.to_uppercase())
        } else if scientific {
            if job.asset_key == "vi_reliability" {
                ["Good", "Marginal", "Snow or ice", "Cloudy"]
                    .get(value as usize)
                    .unwrap_or(&"Fill or unspecified rank")
                    .to_string()
            } else {
                job.asset_key.clone()
            }
        } else {
            format!("{} reflectance band", job.asset_key)
        },
        color: format!("#{r:02x}{g:02x}{b:02x}"),
        is_no_data,
    })
}

#[cfg(test)]
pub(crate) mod tests;

pub(crate) mod composite;
pub(crate) mod modis;
