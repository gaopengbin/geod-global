//! Generic raw-sample inspection. Asset names never select a sensor model,
//! calibration, elevation unit, classification palette, or radar conversion.
use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    time::Instant,
};
use tiff::{
    decoder::{ChunkType, Decoder, DecodingResult, Limits},
    tags::Tag,
};

const MAX_DECODED: usize = 128 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Band {
    pub index: u16,
    pub data_type: String,
    pub nodata: Value,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenericRasterInspection {
    pub job_id: String,
    pub sha256: String,
    pub width: u32,
    pub height: u32,
    pub bands: Vec<Band>,
    pub crs: Option<String>,
    pub transform: Option<[f64; 6]>,
    pub bounds: Option<[f64; 4]>,
    pub pixel_interpretation: Option<String>,
    pub preview_data_url: Option<String>,
    pub preview_width: Option<u32>,
    pub preview_height: Option<u32>,
    pub display_band: u16,
    pub display_range: Option<[f64; 2]>,
    pub limitations: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenericRasterPixel {
    pub job_id: String,
    pub sha256: String,
    pub column: u32,
    pub row: u32,
    /// Integers outside JavaScript's safe range and non-finite floats retain
    /// their exact lexical representation instead of losing precision.
    pub values: Vec<Value>,
    pub no_data: Vec<bool>,
}
struct Raster<R: Read + Seek> {
    metadata: GenericRasterInspection,
    decoder: Decoder<R>,
    nodata: Vec<Option<NoData>>,
    supported: bool,
}
#[derive(Clone, Copy)]
enum NoData {
    Unsigned(u64),
    Signed(i64),
    Float(f64),
}
impl NoData {
    fn parse(text: &str, format: u16, bits: u16) -> Result<Self> {
        match (format, bits) {
            (1, 8 | 16 | 32 | 64) => {
                let value = text
                    .parse::<u64>()
                    .map_err(|_| "Integer TIFF NoData must be an exact unsigned integer")?;
                if bits < 64 && value >= 1u64 << bits {
                    return Err("TIFF NoData is outside its unsigned sample range".into());
                }
                Ok(Self::Unsigned(value))
            }
            (2, 8 | 16 | 32 | 64) => {
                let value = text
                    .parse::<i64>()
                    .map_err(|_| "Integer TIFF NoData must be an exact signed integer")?;
                if bits < 64 && !(-(1i64 << (bits - 1))..(1i64 << (bits - 1))).contains(&value) {
                    return Err("TIFF NoData is outside its signed sample range".into());
                }
                Ok(Self::Signed(value))
            }
            (3, 16 | 32 | 64) => Ok(Self::Float(
                text.parse::<f64>()
                    .map_err(|_| "TIFF NoData declaration is unsupported")?,
            )),
            _ => Err("TIFF NoData has an unsupported sample type".into()),
        }
    }
    fn json(self) -> Value {
        match self {
            Self::Unsigned(value) if value > 9_007_199_254_740_991 => json!(value.to_string()),
            Self::Signed(value) if value.unsigned_abs() > 9_007_199_254_740_991 => {
                json!(value.to_string())
            }
            Self::Unsigned(value) => json!(value),
            Self::Signed(value) => json!(value),
            Self::Float(value) => finite_json(value),
        }
    }
    fn matches(self, value: &Value) -> bool {
        match self {
            Self::Unsigned(expected) => {
                value
                    .as_u64()
                    .or_else(|| value.as_str().and_then(|v| v.parse::<u64>().ok()))
                    == Some(expected)
            }
            Self::Signed(expected) => {
                value
                    .as_i64()
                    .or_else(|| value.as_str().and_then(|v| v.parse::<i64>().ok()))
                    == Some(expected)
            }
            Self::Float(expected) => {
                numeric(value).is_some_and(|v| v == expected || v.is_nan() && expected.is_nan())
            }
        }
    }
}
fn finite_json(value: f64) -> Value {
    if value.is_nan() {
        json!("NaN")
    } else if value == f64::INFINITY {
        json!("Infinity")
    } else if value == f64::NEG_INFINITY {
        json!("-Infinity")
    } else {
        json!(value)
    }
}
fn numeric(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
}
fn value_at(pixels: &DecodingResult, index: usize) -> Option<Value> {
    macro_rules! numeric {
        ($v:expr) => {
            $v.get(index).map(|v| json!(*v))
        };
    }
    match pixels {
        DecodingResult::U8(v) => numeric!(v),
        DecodingResult::U16(v) => numeric!(v),
        DecodingResult::U32(v) => numeric!(v),
        DecodingResult::I8(v) => numeric!(v),
        DecodingResult::I16(v) => numeric!(v),
        DecodingResult::I32(v) => numeric!(v),
        DecodingResult::U64(v) => v.get(index).map(|v| {
            if *v <= 9_007_199_254_740_991 {
                json!(*v)
            } else {
                json!(v.to_string())
            }
        }),
        DecodingResult::I64(v) => v.get(index).map(|v| {
            if v.unsigned_abs() <= 9_007_199_254_740_991 {
                json!(*v)
            } else {
                json!(v.to_string())
            }
        }),
        DecodingResult::F16(v) => v.get(index).map(|v| finite_json(v.to_f32() as f64)),
        DecodingResult::F32(v) => v.get(index).map(|v| finite_json(*v as f64)),
        DecodingResult::F64(v) => v.get(index).map(|v| finite_json(*v)),
    }
}
fn result_len(pixels: &DecodingResult) -> usize {
    match pixels {
        DecodingResult::U8(v) => v.len(),
        DecodingResult::U16(v) => v.len(),
        DecodingResult::U32(v) => v.len(),
        DecodingResult::U64(v) => v.len(),
        DecodingResult::I8(v) => v.len(),
        DecodingResult::I16(v) => v.len(),
        DecodingResult::I32(v) => v.len(),
        DecodingResult::I64(v) => v.len(),
        DecodingResult::F16(v) => v.len(),
        DecodingResult::F32(v) => v.len(),
        DecodingResult::F64(v) => v.len(),
    }
}
fn is_nodata(value: &Value, nodata: Option<NoData>) -> bool {
    nodata.is_some_and(|n| n.matches(value))
}
fn limits() -> Limits {
    let mut limits = Limits::default();
    limits.decoding_buffer_size = MAX_DECODED;
    limits.intermediate_buffer_size = 32 * 1024 * 1024;
    limits.ifd_value_size = 1024 * 1024;
    limits
}
fn georeference<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    metadata: &mut GenericRasterInspection,
) -> Result<()> {
    let tags = decoder
        .get_tag_u16_vec(Tag::GeoKeyDirectoryTag)
        .map_err(|_| "GeoTIFF key directory is missing or unsupported")?;
    if tags.len() < 4 || tags[0] != 1 || tags.len() != 4 + tags[3] as usize * 4 {
        return Err("GeoTIFF key directory is malformed".into());
    }
    let mut keys = BTreeMap::new();
    for entry in tags[4..].chunks_exact(4) {
        if keys
            .insert(entry[0], (entry[1], entry[2], entry[3]))
            .is_some()
        {
            return Err("GeoTIFF key directory repeats a key".into());
        }
    }
    let direct = |key| -> Result<Option<u16>> {
        match keys.get(&key) {
            None => Ok(None),
            Some((0, 1, value)) => Ok(Some(*value)),
            Some(_) => {
                Err("Indirect coordinate-system keys cannot be interpreted by this reader".into())
            }
        }
    };
    let model = direct(1024)?;
    let crs = match model {
        Some(1) => direct(3072)?,
        Some(2) => direct(2048)?,
        _ => return Err("User-defined or geocentric coordinate reference is unsupported".into()),
    };
    if let Some(crs) = crs.filter(|code| *code != 32767 && *code > 0) {
        metadata.crs = Some(format!("EPSG:{crs}"));
    } else {
        return Err("GeoTIFF does not declare a supported EPSG code".into());
    }
    let interpretation = direct(1025)?.unwrap_or(1);
    metadata.pixel_interpretation = Some(
        match interpretation {
            1 => "area",
            2 => "point",
            _ => return Err("GeoTIFF pixel interpretation is invalid".into()),
        }
        .into(),
    );
    let transform = decoder
        .find_tag(Tag::ModelTransformationTag)
        .map_err(io_error)?
        .map(|v| v.into_f64_vec())
        .transpose()
        .map_err(io_error)?;
    let scale = decoder
        .find_tag(Tag::ModelPixelScaleTag)
        .map_err(io_error)?
        .map(|v| v.into_f64_vec())
        .transpose()
        .map_err(io_error)?;
    let tiepoint = decoder
        .find_tag(Tag::ModelTiepointTag)
        .map_err(io_error)?
        .map(|v| v.into_f64_vec())
        .transpose()
        .map_err(io_error)?;
    let (mut bounds, spacing) = crate::raster::georeference(
        metadata.width,
        metadata.height,
        transform.as_deref(),
        scale.as_deref(),
        tiepoint.as_deref(),
    )?;
    if interpretation == 2 {
        bounds[0] -= spacing[0] * 0.5;
        bounds[2] -= spacing[0] * 0.5;
        bounds[1] += spacing[1] * 0.5;
        bounds[3] += spacing[1] * 0.5;
    }
    metadata.bounds = Some(bounds);
    metadata.transform = Some([spacing[0], 0.0, bounds[0], 0.0, -spacing[1], bounds[3]]);
    Ok(())
}
fn decode<R: Read + Seek>(reader: R, job: &Job) -> Result<Raster<R>> {
    let mut decoder = Decoder::new(reader)
        .map_err(io_error)?
        .with_limits(limits());
    let (width, height) = decoder.dimensions().map_err(io_error)?;
    if width == 0 || height == 0 || width > 65536 || height > 65536 {
        return Err("Generic raster dimensions exceed 65,536 pixels per side".into());
    }
    let samples = decoder
        .find_tag_unsigned::<u16>(Tag::SamplesPerPixel)
        .map_err(io_error)?
        .unwrap_or(1);
    if samples == 0 || samples > 16 {
        return Err("Generic raster inspection supports 1–16 samples per pixel".into());
    }
    let bits = decoder
        .get_tag_u16_vec(Tag::BitsPerSample)
        .map_err(io_error)?;
    let formats = decoder
        .find_tag(Tag::SampleFormat)
        .map_err(io_error)?
        .map(|v| v.into_u16_vec())
        .transpose()
        .map_err(io_error)?
        .unwrap_or(vec![1]);
    if bits.is_empty()
        || formats.is_empty()
        || !matches!(bits.len(), 1) && bits.len() != samples as usize
        || formats.len() != 1 && formats.len() != samples as usize
    {
        return Err("TIFF sample declarations do not match its band count".into());
    }
    let bands = (0..samples)
        .map(|index| {
            let bit = bits[usize::from(index).min(bits.len() - 1)];
            let format = formats[usize::from(index).min(formats.len() - 1)];
            let dtype = match (format, bit) {
                (1, 8 | 16 | 32 | 64) => format!("UInt{bit}"),
                (2, 8 | 16 | 32 | 64) => format!("Int{bit}"),
                (3, 16 | 32 | 64) => format!("Float{bit}"),
                _ => format!("Unsupported({format},{bit})"),
            };
            Band {
                index: index + 1,
                data_type: dtype,
                nodata: Value::Null,
            }
        })
        .collect();
    let mut metadata=GenericRasterInspection{job_id:job.id.clone(),sha256:job.sha256.clone().ok_or("Source job has no SHA-256")?,width,height,bands,crs:None,transform:None,bounds:None,pixel_interpretation:None,preview_data_url:None,preview_width:None,preview_height:None,display_band:1,display_range:None,limitations:vec!["Preview is a first-band grayscale stretch of raw samples; no sensor calibration or scientific units are inferred.".into(),"COG layout conformance has not been validated.".into()]};
    if let Err(e) = georeference(&mut decoder, &mut metadata) {
        metadata.transform = None;
        metadata.bounds = None;
        metadata.limitations.push(e);
    }
    let nodata_text = match decoder.find_tag(Tag::GdalNodata).map_err(io_error)? {
        None => None,
        Some(v) => {
            let value = v.into_string().map_err(io_error)?;
            Some(value.trim_matches('\0').trim().to_string())
        }
    };
    let mut nodata = Vec::with_capacity(samples as usize);
    for (index, band) in metadata.bands.iter_mut().enumerate() {
        let value = nodata_text
            .as_deref()
            .map(|text| {
                NoData::parse(
                    text,
                    formats[index.min(formats.len() - 1)],
                    bits[index.min(bits.len() - 1)],
                )
            })
            .transpose()?;
        band.nodata = value.map(NoData::json).unwrap_or(Value::Null);
        nodata.push(value);
    }
    let mut supported = true;
    if decoder
        .find_tag_unsigned::<u16>(Tag::Orientation)
        .map_err(io_error)?
        .unwrap_or(1)
        != 1
    {
        metadata
            .limitations
            .push("Non-top-left TIFF orientation is unsupported for raw inspection.".into());
        supported = false;
    }
    if decoder
        .find_tag_unsigned::<u16>(Tag::PlanarConfiguration)
        .map_err(io_error)?
        .unwrap_or(1)
        != 1
    {
        metadata
            .limitations
            .push("Separate-planar TIFF samples are not decoded by this reader.".into());
        supported = false;
    }
    let photo = decoder
        .find_tag_unsigned::<u16>(Tag::PhotometricInterpretation)
        .map_err(io_error)?
        .unwrap_or(1);
    if !matches!(photo, 1 | 2) {
        metadata.limitations.push(
            "This photometric interpretation could transform raw samples and is not decoded."
                .into(),
        );
        supported = false;
    }
    if metadata
        .bands
        .iter()
        .any(|b| b.data_type.starts_with("Unsupported"))
        || bits.iter().any(|b| *b != bits[0])
        || formats.iter().any(|f| *f != formats[0])
    {
        metadata
            .limitations
            .push("Unsupported or mixed TIFF sample types are not decoded.".into());
        supported = false;
    }
    Ok(Raster {
        metadata,
        decoder,
        nodata,
        supported,
    })
}
/// Verify the immutable source and original bytes without requiring TIFF support.
/// The returned file remains shared-locked and is positioned at its beginning.
pub(crate) fn verified_original(root: &Path, job: &Job) -> Result<File> {
    if job.wcs_source.is_some() {
        crate::wcs::validate_job(root, job)?;
    } else {
        validate_job(root, job)?;
    }
    let path = storage::verified_output_path(root, job)?;
    let mut file = File::open(path).map_err(io_error)?;
    fs2::FileExt::try_lock_shared(&file).map_err(|_| "Original raster is being modified")?;
    let metadata = file.metadata().map_err(io_error)?;
    let size = metadata.len();
    if size == 0
        || size > crate::MAX_ASSET_BYTES
        || size != job.bytes_downloaded
        || job.total_bytes.is_some_and(|n| n != size)
    {
        return Err("Local original raster size changed or exceeds 512 MiB".into());
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut hasher = Sha256::new();
    let mut count = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        if Instant::now() > deadline {
            return Err("Original raster checksum verification timed out".into());
        }
        let n = file.read(&mut buffer).map_err(io_error)?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count > size {
            return Err("Original raster grew during verification".into());
        }
        hasher.update(&buffer[..n]);
    }
    if count != size
        || job.sha256.as_deref() != Some(format!("{:x}", hasher.finalize()).as_str())
        || file.metadata().map_err(io_error)?.modified().ok() != metadata.modified().ok()
    {
        return Err("Local original raster size or SHA-256 changed".into());
    }
    file.seek(SeekFrom::Start(0)).map_err(io_error)?;
    Ok(file)
}
fn read(root: &Path, job: &Job) -> Result<Raster<BufReader<File>>> {
    decode(BufReader::new(verified_original(root, job)?), job)
}
fn chunk_layout<R: Read + Seek>(decoder: &mut Decoder<R>) -> Result<(u32, u32, u32)> {
    let (width, height) = decoder.dimensions().map_err(io_error)?;
    let (cw, ch) = decoder.chunk_dimensions();
    if cw == 0 || ch == 0 {
        return Err("TIFF chunks have zero dimensions".into());
    }
    let columns = width.div_ceil(cw);
    let count = match decoder.get_chunk_type() {
        ChunkType::Tile => decoder.tile_count().map_err(io_error)?,
        ChunkType::Strip => decoder.strip_count().map_err(io_error)?,
    };
    if columns.checked_mul(height.div_ceil(ch)) != Some(count) {
        return Err("TIFF chunk layout is not a complete interleaved raster".into());
    }
    Ok((cw, ch, columns))
}
fn chunk<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    index: u32,
    bands: usize,
) -> Result<DecodingResult> {
    let (width, height) = decoder.chunk_data_dimensions(index);
    let bytes = u64::from(width) * u64::from(height) * bands as u64 * 8;
    if bytes > MAX_DECODED as u64 {
        return Err("A TIFF chunk exceeds the 128 MiB raw inspection limit".into());
    }
    let pixels = decoder.read_chunk(index).map_err(io_error)?;
    if result_len(&pixels) != width as usize * height as usize * bands {
        return Err("TIFF chunk sample count differs from its dimensions".into());
    }
    Ok(pixels)
}
fn pixel<R: Read + Seek>(
    raster: &mut Raster<R>,
    column: u32,
    row: u32,
) -> Result<(Vec<Value>, Vec<bool>)> {
    if !raster.supported {
        return Err("This TIFF sample layout cannot be decoded".into());
    }
    if column >= raster.metadata.width || row >= raster.metadata.height {
        return Err("Pixel column or row is outside the original raster".into());
    }
    raster.decoder.seek_to_image(0).map_err(io_error)?;
    let (cw, ch, columns) = chunk_layout(&mut raster.decoder)?;
    let index = row / ch * columns + column / cw;
    let bands = raster.metadata.bands.len();
    let pixels = chunk(&mut raster.decoder, index, bands)?;
    let (width, _) = raster.decoder.chunk_data_dimensions(index);
    let offset = ((row % ch) as usize * width as usize + (column % cw) as usize) * bands;
    let values = (0..bands)
        .map(|band| {
            value_at(&pixels, offset + band).ok_or_else(|| "TIFF pixel is incomplete".to_string())
        })
        .collect::<Result<Vec<_>>>()?;
    let no_data = values
        .iter()
        .enumerate()
        .map(|(band, v)| is_nodata(v, raster.nodata[band]))
        .collect();
    Ok((values, no_data))
}
fn preview<R: Read + Seek>(raster: &mut Raster<R>, edge: u32) -> Result<()> {
    if !raster.supported {
        return Ok(());
    }
    let deadline = Instant::now() + Duration::from_secs(30);
    let decoder = &mut raster.decoder;
    decoder.seek_to_image(0).map_err(io_error)?;
    let mut selected_image = 0;
    let mut dimensions = [raster.metadata.width, raster.metadata.height];
    let color = decoder.colortype().map_err(io_error)?;
    fn sample_tags<R: Read + Seek>(decoder: &mut Decoder<R>) -> Result<Value> {
        let optional = |decoder: &mut Decoder<R>, tag| -> Result<Value> {
            Ok(match decoder.find_tag(tag).map_err(io_error)? {
                None => Value::Null,
                Some(value) => json!(format!("{value:?}")),
            })
        };
        Ok(
            json!({"samples":decoder.find_tag_unsigned::<u16>(Tag::SamplesPerPixel).map_err(io_error)?.unwrap_or(1),"bits":decoder.get_tag_u16_vec(Tag::BitsPerSample).map_err(io_error)?,"format":optional(decoder,Tag::SampleFormat)?,"orientation":decoder.find_tag_unsigned::<u16>(Tag::Orientation).map_err(io_error)?.unwrap_or(1),"planar":decoder.find_tag_unsigned::<u16>(Tag::PlanarConfiguration).map_err(io_error)?.unwrap_or(1),"photo":decoder.find_tag_unsigned::<u16>(Tag::PhotometricInterpretation).map_err(io_error)?.unwrap_or(1),"nodata":optional(decoder,Tag::GdalNodata)?,"extraSamples":optional(decoder,Tag::ExtraSamples)?}),
        )
    }
    let original_tags = sample_tags(decoder)?;
    for image in 1..=16 {
        if !decoder.more_images() {
            break;
        }
        if Instant::now() > deadline {
            return Err("Generic overview inspection timed out".into());
        }
        decoder.next_image().map_err(io_error)?;
        let (w, h) = decoder.dimensions().map_err(io_error)?;
        let reduced = match decoder.find_tag_unsigned::<u32>(Tag::NewSubfileType) {
            Ok(Some(value)) => value,
            Ok(None) | Err(_) => continue,
        };
        let factor = if w > 0 && h > 0 {
            (raster.metadata.width / w)
                .max(raster.metadata.height / h)
                .max(2)
        } else {
            2
        };
        let proportional = (factor.saturating_sub(1).max(2)..=factor.saturating_add(1)).any(|f| {
            raster.metadata.width.div_ceil(f) == w && raster.metadata.height.div_ceil(f) == h
        });
        if w > 0
            && h > 0
            && w <= raster.metadata.width
            && h <= raster.metadata.height
            && w.max(h) >= edge
            && w.max(h) < dimensions[0].max(dimensions[1])
            && decoder.colortype().is_ok_and(|c| c == color)
            && reduced & 1 == 1
            && reduced & 4 == 0
            && proportional
            && sample_tags(decoder).is_ok_and(|tags| tags == original_tags)
        {
            selected_image = image;
            dimensions = [w, h];
        }
    }
    decoder.seek_to_image(selected_image).map_err(io_error)?;
    if selected_image > 0 {
        raster.metadata.limitations.push("Preview samples an embedded overview; raw pixel inspection always reads the original image.".into());
    }
    let metadata = &mut raster.metadata;
    let scale = (dimensions[0].max(dimensions[1]) as f64 / edge as f64).max(1.0);
    let width = (dimensions[0] as f64 / scale).ceil() as u32;
    let height = (dimensions[1] as f64 / scale).ceil() as u32;
    let bands = metadata.bands.len();
    let (cw, ch, columns) = chunk_layout(decoder)?;
    let mut requested: BTreeMap<u32, Vec<(usize, u32, u32)>> = BTreeMap::new();
    for y in 0..height {
        for x in 0..width {
            let row = (u64::from(y) * u64::from(dimensions[1]) / u64::from(height)) as u32;
            let col = (u64::from(x) * u64::from(dimensions[0]) / u64::from(width)) as u32;
            requested
                .entry(row / ch * columns + col / cw)
                .or_default()
                .push(((y * width + x) as usize, col % cw, row % ch));
        }
    }
    let mut selected = vec![None; (width * height) as usize];
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    let mut decoded = 0usize;
    for (index, requests) in requested {
        if Instant::now() > deadline {
            return Err("Generic raster preview timed out".into());
        }
        let (chunk_width, chunk_height) = decoder.chunk_data_dimensions(index);
        decoded = decoded.saturating_add(chunk_width as usize * chunk_height as usize * bands * 8);
        if decoded > 512 * 1024 * 1024 {
            return Err("Preview exceeds its 512 MiB cumulative chunk budget; original pixel inspection remains available".into());
        }
        let pixels = chunk(decoder, index, bands)?;
        for (position, col, row) in requests {
            let value = value_at(
                &pixels,
                (row as usize * chunk_width as usize + col as usize) * bands,
            )
            .filter(|v| !is_nodata(v, raster.nodata[0]))
            .and_then(|v| numeric(&v))
            .filter(|v| v.is_finite());
            if let Some(value) = value {
                min = min.min(value);
                max = max.max(value);
            }
            selected[position] = value;
        }
    }
    let range = (min.is_finite() && max.is_finite()).then_some([min, max]);
    let mut rgba = Vec::with_capacity(selected.len() * 4);
    for value in selected {
        if let Some(value) = value {
            let gray = if max == min {
                128
            } else {
                ((value - min) / (max - min) * 255.0)
                    .round()
                    .clamp(0.0, 255.0) as u8
            };
            rgba.extend_from_slice(&[gray, gray, gray, 255]);
        } else {
            rgba.extend_from_slice(&[0, 0, 0, 0]);
        }
    }
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(io_error)?
        .write_image_data(&rgba)
        .map_err(io_error)?;
    metadata.preview_data_url = Some(format!("data:image/png;base64,{}", STANDARD.encode(bytes)));
    metadata.preview_width = Some(width);
    metadata.preview_height = Some(height);
    metadata.display_range = range;
    Ok(())
}
pub fn thumbnail(root: &Path, job: &Job) -> Result<crate::thumbnail::FileThumbnail> {
    let mut raster = read(root, job)?;
    preview(&mut raster, 160)?;
    let m = raster.metadata;
    Ok(crate::thumbnail::FileThumbnail {
        job_id: job.id.clone(),
        sha256: m.sha256,
        width: m
            .preview_width
            .ok_or("This TIFF cannot produce a local preview")?,
        height: m
            .preview_height
            .ok_or("This TIFF cannot produce a local preview")?,
        data_url: m
            .preview_data_url
            .ok_or("This TIFF cannot produce a local preview")?,
    })
}
impl JobManager {
    pub async fn inspect_stac_asset(&self, id: &str) -> Result<GenericRasterInspection> {
        let job = self.get(id).await.ok_or("Unknown job")?;
        if job.stac_source.is_none() {
            return Err("The task has no STAC source".into());
        }
        self.inspect_generic_raster_asset(job).await
    }
    pub async fn inspect_wcs_asset(&self, id: &str) -> Result<GenericRasterInspection> {
        let job = self.get(id).await.ok_or("Unknown job")?;
        if job.wcs_source.is_none() {
            return Err("The task has no coverage request".into());
        }
        self.inspect_generic_raster_asset(job).await
    }
    async fn inspect_generic_raster_asset(&self, job: Job) -> Result<GenericRasterInspection> {
        let root = self.inner.root.clone();
        let permit = tokio::time::timeout(
            Duration::from_secs(5),
            self.inner.raster_permits.clone().acquire_owned(),
        )
        .await
        .map_err(|_| "Raster worker is busy")?
        .map_err(io_error)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let mut raster = read(&root, &job)?;
            if let Err(error) = preview(&mut raster, 768) {
                raster.metadata.limitations.push(error);
            }
            Ok(raster.metadata)
        })
        .await
        .map_err(io_error)?
    }
    pub async fn sample_stac_asset(
        &self,
        id: &str,
        column: u32,
        row: u32,
    ) -> Result<GenericRasterPixel> {
        let job = self.get(id).await.ok_or("Unknown job")?;
        if job.stac_source.is_none() {
            return Err("The task has no STAC source".into());
        }
        self.sample_generic_raster_asset(job, column, row).await
    }
    pub async fn sample_wcs_asset(
        &self,
        id: &str,
        column: u32,
        row: u32,
    ) -> Result<GenericRasterPixel> {
        let job = self.get(id).await.ok_or("Unknown job")?;
        if job.wcs_source.is_none() {
            return Err("The task has no coverage request".into());
        }
        self.sample_generic_raster_asset(job, column, row).await
    }
    async fn sample_generic_raster_asset(
        &self,
        job: Job,
        column: u32,
        row: u32,
    ) -> Result<GenericRasterPixel> {
        let root = self.inner.root.clone();
        let permit = tokio::time::timeout(
            Duration::from_secs(5),
            self.inner.raster_permits.clone().acquire_owned(),
        )
        .await
        .map_err(|_| "Raster worker is busy")?
        .map_err(io_error)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let mut raster = read(&root, &job)?;
            let (values, no_data) = pixel(&mut raster, column, row)?;
            Ok(GenericRasterPixel {
                job_id: job.id,
                sha256: raster.metadata.sha256,
                column,
                row,
                values,
                no_data,
            })
        })
        .await
        .map_err(io_error)?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tiff::encoder::{colortype, TiffEncoder};
    fn job() -> Job {
        let mut job = crate::new_download_job(crate::CreateJobRequest {
            item_id: "fixture".into(),
            asset_key: "stac_asset".into(),
            href: "https://example.com/source.tif".into(),
            media_type: "image/tiff".into(),
            title: None,
        });
        job.sha256 = Some("a".repeat(64));
        job
    }
    #[test]
    fn raw_float_point_grid_uses_pixel_edges_and_preserves_no_nodata() {
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut bytes).unwrap();
            let mut image = encoder.new_image::<colortype::Gray32Float>(2, 2).unwrap();
            image
                .encoder()
                .write_tag(
                    Tag::GeoKeyDirectoryTag,
                    &[
                        1u16, 1, 0, 3, 1024, 0, 1, 2, 1025, 0, 1, 2, 2048, 0, 1, 4326,
                    ][..],
                )
                .unwrap();
            image
                .encoder()
                .write_tag(Tag::ModelPixelScaleTag, &[0.5f64, 0.5, 0.0][..])
                .unwrap();
            image
                .encoder()
                .write_tag(
                    Tag::ModelTiepointTag,
                    &[0.0f64, 0.0, 0.0, 13.0, 53.0, 0.0][..],
                )
                .unwrap();
            image.write_data(&[-5.0f32, 0.0, 12.5, 100.0]).unwrap();
        }
        bytes.set_position(0);
        let mut raster = decode(bytes, &job()).unwrap();
        assert_eq!(raster.metadata.crs.as_deref(), Some("EPSG:4326"));
        assert_eq!(raster.metadata.bounds, Some([12.75, 52.25, 13.75, 53.25]));
        assert_eq!(raster.metadata.bands[0].nodata, Value::Null);
        assert_eq!(raster.metadata.bands[0].data_type, "Float32");
        assert_eq!(pixel(&mut raster, 0, 0).unwrap().0, vec![json!(-5.0)]);
        preview(&mut raster, 160).unwrap();
        assert_eq!(raster.metadata.display_range, Some([-5.0, 100.0]));
        assert!(raster.metadata.preview_data_url.is_some());
    }
    #[test]
    fn exact_large_integer_and_nonfinite_values_do_not_silently_round() {
        assert_eq!(
            value_at(&DecodingResult::U64(vec![9007199254740993]), 0),
            Some(json!("9007199254740993"))
        );
        assert_eq!(
            value_at(&DecodingResult::F32(vec![f32::NAN]), 0),
            Some(json!("NaN"))
        );
        assert!(!is_nodata(&json!(0), None));
        assert!(is_nodata(&json!("NaN"), Some(NoData::Float(f64::NAN))));
    }
    #[test]
    fn unsigned_large_integer_nodata_does_not_mask_its_adjacent_value() {
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut bytes).unwrap();
            let mut image = encoder.new_image::<colortype::Gray64>(2, 1).unwrap();
            image
                .encoder()
                .write_tag(Tag::GdalNodata, "9007199254740992")
                .unwrap();
            image
                .write_data(&[9_007_199_254_740_992, 9_007_199_254_740_993])
                .unwrap();
        }
        bytes.set_position(0);
        let mut raster = decode(bytes, &job()).unwrap();
        assert_eq!(raster.metadata.bands[0].nodata, json!("9007199254740992"));
        assert_eq!(
            pixel(&mut raster, 0, 0).unwrap(),
            (vec![json!("9007199254740992")], vec![true])
        );
        assert_eq!(
            pixel(&mut raster, 1, 0).unwrap(),
            (vec![json!("9007199254740993")], vec![false])
        );
        preview(&mut raster, 160).unwrap();
        assert!(raster.metadata.display_range.is_some());
        assert!(NoData::parse("18446744073709551616", 1, 64).is_err());
        assert!(NoData::parse("256", 1, 8).is_err());
    }
    #[test]
    fn signed_large_integer_nodata_does_not_mask_its_adjacent_value() {
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut bytes).unwrap();
            let mut image = encoder.new_image::<colortype::GrayI64>(2, 1).unwrap();
            image
                .encoder()
                .write_tag(Tag::GdalNodata, "-9007199254740992")
                .unwrap();
            image
                .write_data(&[-9_007_199_254_740_992, -9_007_199_254_740_993])
                .unwrap();
        }
        bytes.set_position(0);
        let mut raster = decode(bytes, &job()).unwrap();
        assert_eq!(raster.metadata.bands[0].nodata, json!("-9007199254740992"));
        assert_eq!(pixel(&mut raster, 0, 0).unwrap().1, vec![true]);
        assert_eq!(pixel(&mut raster, 1, 0).unwrap().1, vec![false]);
        preview(&mut raster, 160).unwrap();
        assert!(raster.metadata.display_range.is_some());
        assert!(NoData::parse("-9223372036854775809", 2, 64).is_err());
        assert!(NoData::parse("-129", 2, 8).is_err());
    }
    #[test]
    fn missing_georeference_remains_unknown_and_does_not_create_scl_classes() {
        let mut bytes = Cursor::new(Vec::new());
        {
            TiffEncoder::new(&mut bytes)
                .unwrap()
                .write_image::<colortype::Gray8>(2, 1, &[0, 11])
                .unwrap();
        }
        bytes.set_position(0);
        let mut raster = decode(bytes, &job()).unwrap();
        assert_eq!(raster.metadata.crs, None);
        assert_eq!(raster.metadata.bounds, None);
        preview(&mut raster, 160).unwrap();
        assert_eq!(raster.metadata.display_range, Some([0.0, 11.0]));
    }
    #[test]
    fn smaller_unrelated_tiff_page_is_not_an_overview() {
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut bytes).unwrap();
            encoder
                .write_image::<colortype::Gray8>(4, 4, &(0u8..16).collect::<Vec<_>>())
                .unwrap();
            encoder
                .write_image::<colortype::Gray8>(2, 2, &[200; 4])
                .unwrap();
        }
        bytes.set_position(0);
        let mut raster = decode(bytes, &job()).unwrap();
        preview(&mut raster, 2).unwrap();
        assert_eq!(raster.metadata.display_range, Some([0.0, 10.0]));
        assert!(!raster
            .metadata
            .limitations
            .iter()
            .any(|s| s.contains("embedded overview")));
    }
    #[test]
    fn malformed_overview_declarations_do_not_receive_default_values() {
        for malformed_reduced_type in [true, false] {
            let mut bytes = Cursor::new(Vec::new());
            {
                let mut encoder = TiffEncoder::new(&mut bytes).unwrap();
                encoder
                    .write_image::<colortype::Gray8>(4, 4, &(0u8..16).collect::<Vec<_>>())
                    .unwrap();
                let mut image = encoder.new_image::<colortype::Gray8>(2, 2).unwrap();
                if malformed_reduced_type {
                    image.encoder().write_tag(Tag::NewSubfileType, "1").unwrap();
                } else {
                    image
                        .encoder()
                        .write_tag(Tag::NewSubfileType, 1u32)
                        .unwrap();
                    image
                        .encoder()
                        .write_tag(Tag::Orientation, 65_536u32)
                        .unwrap();
                }
                image.write_data(&[200; 4]).unwrap();
            }
            bytes.set_position(0);
            let mut raster = decode(bytes, &job()).unwrap();
            preview(&mut raster, 2).unwrap();
            assert_eq!(raster.metadata.display_range, Some([0.0, 10.0]));
            assert!(!raster
                .metadata
                .limitations
                .iter()
                .any(|s| s.contains("embedded overview")));
        }
    }
}
