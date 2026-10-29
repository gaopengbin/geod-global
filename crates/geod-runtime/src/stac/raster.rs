//! Generic raw-sample inspection. Asset names never select a sensor model,
//! calibration, elevation unit, classification palette, or radar conversion.

#[cfg(test)]
mod clcd_complete_reference_regression {
    use super::*;

    #[test]
    #[ignore = "requires the independently downloaded complete CLCD original and reference JSON"]
    fn complete_clcd_original_decodes_palette_preview_and_all_reference_pixels_with_bounded_reads()
    {
        let path = std::path::PathBuf::from(
            std::env::var_os("GEOD_CLCD_RESEARCH_FILE").expect("complete research original"),
        );
        let output = std::path::PathBuf::from(
            std::env::var_os("GEOD_CLCD_NATIVE_OUTPUT_DIR").expect("private audit output"),
        );
        assert!(!output.exists(), "preserve earlier evidence");
        let original_size = std::fs::metadata(&path).unwrap().len();
        assert_eq!(original_size, crate::datasets::clcd::ORIGINAL_BYTES);
        let mut original = std::fs::File::open(&path).unwrap();
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 65536];
        loop {
            let n = original.read(&mut buffer).unwrap();
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }
        let sha = format!("{:x}", hasher.finalize());
        assert_eq!(sha, crate::datasets::clcd::original_sha256().unwrap());
        original.seek(SeekFrom::Start(0)).unwrap();
        let mut raster = decode_identified(
            std::io::BufReader::new(original),
            "independent-reference-only",
            &sha,
        )
        .unwrap();
        assert!(raster.supported);
        assert_eq!(
            (raster.metadata.width, raster.metadata.height),
            (161378, 135079)
        );
        assert_eq!(raster.metadata.crs.as_deref(), Some("CLCD:AlbersWGS84"));
        assert_eq!(raster.metadata.bands[0].nodata, serde_json::json!(0));
        preview(&mut raster, 768).unwrap();
        let encoding = raster.metadata.categorical_encoding.as_ref().unwrap();
        assert_eq!(encoding["classes"].as_array().unwrap().len(), 9);
        assert_eq!(encoding["palette"].as_array().unwrap().len(), 256);
        let mut count = 0;
        for name in [
            "clcd-2004-raw-reference.json",
            "clcd-2004-class-reference.json",
        ] {
            let reference: Value =
                serde_json::from_slice(&std::fs::read(path.parent().unwrap().join(name)).unwrap())
                    .unwrap();
            assert_eq!(reference["original"]["sha256"], sha);
            for point in reference["points"].as_array().unwrap() {
                let (values, no_data) = pixel(
                    &mut raster,
                    point["column"].as_u64().unwrap() as u32,
                    point["row"].as_u64().unwrap() as u32,
                )
                .unwrap();
                assert_eq!(serde_json::to_value(values).unwrap(), point["values"]);
                assert_eq!(serde_json::to_value(no_data).unwrap(), point["noData"]);
                count += 1;
            }
        }
        assert_eq!(count, 161);
        assert_eq!(std::fs::metadata(&path).unwrap().len(), original_size);
        std::fs::create_dir_all(&output).unwrap();
        std::fs::write(
            output.join("inspection.json"),
            serde_json::to_vec_pretty(&raster.metadata).unwrap(),
        )
        .unwrap();
        std::fs::write(
            output.join("summary.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "referenceDecodeOnly":true,"softwareTransferAcceptance":false,"sha256":sha,
                "bytes":original_size,"originalPixelsMatch":count,"wholeOriginalStreamHashed":true,
                "wholeOriginalLoadedIntoMemory":false,"sourceModified":false
            }))
            .unwrap(),
        )
        .unwrap();
    }
}
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
mod enmap;
mod palette;
use palette::RawPaletteReader;

#[cfg(test)]
#[path = "raster/sinolc_acceptance_tests.rs"]
mod sinolc_acceptance_tests;

const MAX_DECODED: usize = 128 * 1024 * 1024;
// Local files are already capped at 32 MiB and 10,000 pixels per side. Full
// validation may stream more than 128 MiB of samples across many small chunks;
// keep each chunk's memory cap unchanged and bound cumulative work separately.
const MAX_LOCAL_DECODE_WORK: usize = 1024 * 1024 * 1024;
const MAX_TILED_AXIS: u32 = 131_072;
const MAX_STRIPPED_AXIS: u32 = 65_536;
const MAX_GRID_PIXELS: u64 = 8_000_000_000;
const MAX_CHUNKS: u32 = 131_072;
// Exact independently transferred SRTM15+ V2.5.5 02_10 original. Full reads
// established a valid base and factor-2 image, but entirely NaN factor-4/8/16
// images. This byte identity is not a product-wide rule or NoData heuristic.
const SRTM15PLUS_02_10_SHA256: &str =
    "2d60d606f2c0cfeaee5c70e5b2bd50ed70f0c95361c3fa3e0ede87ddc2ad2b8d";
// Exact independently downloaded HREA Hong Kong 2019 lightscore original:
// base 54/54 chunks decode, but factor-2 chunk 8 and factor-8 chunk 2 lack
// an LZW end code. This is an original-bound exception, not tolerant decoding.
const HREA_HK_2019_SHA256: &str =
    "c3a8d91858e609e06dfb701901bfb9582b7d472b173f1ad4e3c413ee84237718";

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spectral_encoding: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categorical_encoding: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_encoding: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive_original: Option<Value>,
    pub job_id: String,
    pub sha256: String,
    pub width: u32,
    pub height: u32,
    pub storage_layout: String,
    pub bands: Vec<Band>,
    pub crs: Option<String>,
    /// `crs` remains the horizontal code for existing clients; a declared
    /// vertical coordinate system is reported independently, never discarded
    /// or replaced by an inferred compound authority code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub horizontal_crs: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertical_crs: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_model_pixel_scale: Option<[f64; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_model_tiepoint: Option<[f64; 6]>,
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
    // Keep the declared decimal for metadata, compare at the stored precision.
    Float32(f64),
}
impl NoData {
    fn parse(text: &str, format: u16, bits: u16) -> Result<Self> {
        // GEE writes e.g. `0.0` on UInt16 TIFFs. Accept a decimal suffix only
        // when it is exactly zero, without f64 rounding of 64-bit integers.
        let text = if format == 1 || format == 2 {
            match text.split_once('.') {
                Some((integer, fraction))
                    if !fraction.is_empty() && fraction.bytes().all(|c| c == b'0') =>
                {
                    integer
                }
                _ => text,
            }
        } else {
            text
        };
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
            (3, 32) => {
                let value = text
                    .parse::<f64>()
                    .map_err(|_| "TIFF NoData declaration is unsupported")?;
                if value.is_finite() && !(value as f32).is_finite() {
                    return Err("TIFF NoData is outside its Float32 sample range".into());
                }
                Ok(Self::Float32(value))
            }
            (3, 16 | 64) => Ok(Self::Float(
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
            Self::Float(value) | Self::Float32(value) => finite_json(value),
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
            Self::Float32(declared) => {
                let expected = declared as f32 as f64;
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
    if crate::datasets::additional_pc::is_modis_original_sha(&metadata.sha256) {
        metadata.crs = Some(crate::raster::reflectance::modis::crs(decoder, &tags)?);
        metadata.horizontal_crs = metadata.crs.clone();
    } else if crate::datasets::clcd::large_grid_identity(
        &metadata.sha256,
        metadata.width,
        metadata.height,
        metadata.storage_layout == "tile",
    ) {
        let doubles = decoder
            .get_tag_f64_vec(Tag::GeoDoubleParamsTag)
            .map_err(io_error)?;
        let ascii = decoder
            .get_tag_ascii_string(Tag::GeoAsciiParamsTag)
            .map_err(io_error)?;
        crate::datasets::clcd::validate_geotiff_crs(&metadata.sha256, &tags, &doubles, &ascii)?;
        metadata.crs = Some(crate::datasets::clcd::CRS.into());
        metadata.horizontal_crs = metadata.crs.clone();
    } else {
        let crs = match model {
            Some(1) => direct(3072)?,
            Some(2) => direct(2048)?,
            _ => {
                return Err("User-defined or geocentric coordinate reference is unsupported".into())
            }
        };
        if let Some(crs) = crs.filter(|code| *code != 32767 && *code > 0) {
            metadata.crs = Some(format!("EPSG:{crs}"));
            metadata.horizontal_crs = metadata.crs.clone();
        } else {
            return Err("GeoTIFF does not declare a supported EPSG code".into());
        }
    }
    match direct(4096) {
        Ok(Some(code)) if code > 0 && code != 32767 => {
            metadata.vertical_crs = Some(format!("EPSG:{code}"));
            metadata.limitations.push("The source declares a separate vertical CRS. Map bounds and CRS describe the horizontal footprint only; no vertical datum transformation or height calibration is applied.".into());
        }
        Ok(None) => (),
        _ => metadata.limitations.push("A source vertical CRS key is present but is not a direct EPSG code; its original declaration remains in the original TIFF.".into()),
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
    metadata.source_model_pixel_scale = scale
        .as_ref()
        .filter(|v| v.len() == 3 && v.iter().all(|n| n.is_finite()))
        .map(|v| [v[0], v[1], v[2]]);
    metadata.source_model_tiepoint = tiepoint
        .as_ref()
        .filter(|v| v.len() == 6 && v.iter().all(|n| n.is_finite()))
        .map(|v| [v[0], v[1], v[2], v[3], v[4], v[5]]);
    // Scale/tiepoint tags encode independent X/Y/Z axes. A nonzero Z scale
    // (ETOPO uses 1) does not rotate or change X/Y. Retain all source values,
    // then pass only the horizontal view to the strict 2D footprint helper.
    // Matrix transforms, conflicting tags, rotation, nonfinite values and
    // malformed arrays retain the existing rejection path.
    let horizontal_scale = metadata.source_model_pixel_scale.map(|mut value| {
        value[2] = 0.;
        value
    });
    let horizontal_tiepoint = metadata.source_model_tiepoint.map(|mut value| {
        value[2] = 0.;
        value[5] = 0.;
        value
    });
    if metadata
        .source_model_pixel_scale
        .is_some_and(|v| v[2] != 0.)
        || metadata
            .source_model_tiepoint
            .is_some_and(|v| v[2] != 0. || v[5] != 0.)
    {
        metadata.limitations.push("Original ModelPixelScale/ModelTiepoint Z components are retained in inspection metadata. The footprint uses only their independent horizontal components; original height samples are unchanged.".into());
    }
    let (mut bounds, spacing) = crate::raster::georeference(
        metadata.width,
        metadata.height,
        transform.as_deref(),
        horizontal_scale
            .as_ref()
            .map(|v| v.as_slice())
            .or(scale.as_deref()),
        horizontal_tiepoint
            .as_ref()
            .map(|v| v.as_slice())
            .or(tiepoint.as_deref()),
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
#[cfg(test)]
fn decode<R: Read + Seek>(reader: R, job: &Job) -> Result<Raster<RawPaletteReader<R>>> {
    decode_identified(
        reader,
        &job.id,
        job.sha256.as_deref().ok_or("Source job has no SHA-256")?,
    )
}
fn decode_identified<R: Read + Seek>(
    mut reader: R,
    id: &str,
    sha256: &str,
) -> Result<Raster<RawPaletteReader<R>>> {
    // Palette expansion would lose the original class values. The TIFF decoder
    // currently rejects palettes, so expose their index stream as grayscale in
    // a read-only view; the immutable source, checksum and pixel indices stay
    // unchanged. This never applies to white-is-zero, YCbCr or other encodings.
    if crate::datasets::clcd::large_grid_identity(sha256, 161378, 135079, true) {
        crate::datasets::clcd::validate_original_metadata(&mut reader, sha256)?;
    }
    let reader = RawPaletteReader::new(reader)?;
    let raw_palette = reader.has_palette();
    let mut decoder = Decoder::new(reader)
        .map_err(io_error)?
        .with_limits(limits());
    let (width, height) = decoder.dimensions().map_err(io_error)?;
    let layout = decoder.get_chunk_type();
    let reviewed_sinolc = crate::datasets::sinolc::large_grid_identity(
        sha256,
        width,
        height,
        layout == ChunkType::Tile,
    );
    let reviewed_clcd = crate::datasets::clcd::large_grid_identity(
        sha256,
        width,
        height,
        layout == ChunkType::Tile,
    );
    if !reviewed_sinolc && !reviewed_clcd {
        validate_dimensions(width, height, layout)?;
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
    let mut metadata=GenericRasterInspection{spectral_encoding:None,categorical_encoding:None,sample_encoding:None,archive_original:None,job_id:id.to_owned(),sha256:sha256.to_owned(),width,height,storage_layout:match layout { ChunkType::Tile=>"tile",ChunkType::Strip=>"strip" }.into(),bands,crs:None,horizontal_crs:None,vertical_crs:None,source_model_pixel_scale:None,source_model_tiepoint:None,transform:None,bounds:None,pixel_interpretation:None,preview_data_url:None,preview_width:None,preview_height:None,display_band:1,display_range:None,limitations:vec!["Preview is a first-band grayscale stretch of raw samples; no sensor calibration or scientific units are inferred.".into(),"COG layout conformance has not been validated.".into()]};
    if raw_palette {
        metadata.limitations.push("The source contains a TIFF colour palette. Raw pixel inspection preserves palette indices/class codes; the preview stretches those indices in grayscale and does not apply the palette.".into());
    }
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
    if crate::datasets::additional_pc::is_modis_original_sha(sha256) {
        crate::datasets::additional_pc::validate_modis_grid(&metadata)?;
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
    let planar = decoder
        .find_tag_unsigned::<u16>(Tag::PlanarConfiguration)
        .map_err(io_error)?
        .unwrap_or(1);
    // A one-band separate-planar file has exactly one sample plane. It needs
    // no band rearrangement; multi-band separate planes remain unsupported.
    if planar != 1 && !(planar == 2 && samples == 1) {
        metadata
            .limitations
            .push("Separate-planar TIFF samples are not decoded by this reader.".into());
        supported = false;
    }
    let photo = decoder
        .find_tag_unsigned::<u16>(Tag::PhotometricInterpretation)
        .map_err(io_error)?
        .unwrap_or(1);
    if raw_palette && (samples != 1 || formats[0] != 1 || !matches!(bits[0], 8 | 16)) {
        metadata
            .limitations
            .push("Palette index inspection requires one UInt8 or UInt16 sample per pixel.".into());
        supported = false;
    }
    let reviewed_visual = photo == 6
        && crate::datasets::vantor_open::visual_profile(&metadata)
        && planar == 1 && samples == 3
        && decoder.find_tag_unsigned::<u16>(Tag::Compression).map_err(io_error)? == Some(7)
        && decoder.get_tag_u16_vec(Tag::ChromaSubsampling).is_ok_and(|v| v == [2,2])
        && decoder.find_tag(Tag::Unknown(532)).is_ok_and(|v| matches!(v,Some(tiff::decoder::ifd::Value::List(v)) if v == [0u32,255,128,255,128,255].map(|n| tiff::decoder::ifd::Value::Rational(n,1))))
        && decoder.get_tag_u32(Tag::TileWidth).is_ok_and(|v| v == 512)
        && decoder.get_tag_u32(Tag::TileLength).is_ok_and(|v| v == 512);
    if reviewed_visual {
        metadata.sample_encoding = Some(
            json!({"kind":"jpeg-ycbcr-components","components":["Y","Cb","Cr"],"codec":"JPEG","chromaSubsampling":[2,2],"chromaUpsampling":"triangular fancy interpolation to the original pixel grid","previewComponent":"Y","scientificCalibration":false}),
        );
        metadata.limitations.push("This exact reviewed visual original stores lossy JPEG Y/Cb/Cr with 2×2 chroma subsampling. Pixel values are JPEG-decoded Y, Cb and Cr with codec chroma upsampling, not original subsampled chroma planes, scientific RGB, reflectance or lossless sensor samples. Decoder rounding may differ. Preview uses Y luminance only; original bytes and publisher eo:bands display declarations are unchanged.".into());
    }
    if !matches!(photo, 1 | 2) && !reviewed_visual {
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
    if reviewed_sinolc {
        if !supported
            || !crate::datasets::sinolc::budget_preview_profile(&metadata)
            || decoder.chunk_dimensions() != (256, 256)
            || decoder.tile_count().map_err(io_error)? != crate::datasets::sinolc::CHUNK_COUNT
            || decoder.more_images()
            || photo != 1
            || planar != 2
            || decoder
                .find_tag_unsigned::<u16>(Tag::Compression)
                .map_err(io_error)?
                != Some(5)
            || decoder
                .find_tag_unsigned::<u16>(Tag::Predictor)
                .map_err(io_error)?
                .unwrap_or(1)
                != 1
            || decoder.get_tag_u16_vec(Tag::ColorMap).map_err(io_error)?
                != crate::datasets::sinolc::original_color_map()
        {
            return Err(
                "The exact reviewed large SinoLC original differs from its bounded tile profile"
                    .into(),
            );
        }
        metadata.limitations.push("Only this exact SHA-256 and complete profile-reviewed SinoLC Shanghai original is admitted with its fixed 132000 × 114000 grid and 230136 indexed tiles. Ordinary axis, 8-billion-pixel and chunk ceilings remain unchanged. Reading uses individual 256 × 256 UInt8 tiles; no whole-grid array is allocated. The original grid and classification values are retained, with the existing 128 MiB per-chunk, 512 MiB cumulative preview and timeout limits.".into());
        metadata.categorical_encoding = crate::datasets::sinolc::categorical_encoding(&metadata);
        metadata.limitations.retain(|v| v != "Preview is a first-band grayscale stretch of raw samples; no sensor calibration or scientific units are inferred.");
        metadata.limitations.push("The SinoLC preview maps unchanged class codes through this original's exact ColorMap. It displays categorical colors, not RGB sensor bands or a numerical grayscale calibration. Embedded NoData 0 is transparent; unlisted codes retain their original color without an invented class meaning.".into());
    }
    if reviewed_clcd {
        if !supported || !crate::datasets::clcd::budget_preview_profile(&metadata) {
            return Err("The reviewed CLCD original differs from its physical grid".into());
        }
        crate::datasets::clcd::validate_decoder_profile(&mut decoder, sha256, raw_palette)?;
        metadata.categorical_encoding = crate::datasets::clcd::categorical_encoding(&metadata);
        metadata
            .limitations
            .retain(|v| !v.contains("grayscale") && !v.contains("does not apply the palette"));
        metadata.limitations.push("This exact complete CLCD original preserves its Albers Area grid and classification codes. Display uses its original ColorMap and embedded overviews with bounded tile decoding; no whole-grid array is allocated. NoData 0 is transparent.".into());
    }
    Ok(Raster {
        metadata,
        decoder,
        nodata,
        supported,
    })
}

fn validate_dimensions(width: u32, height: u32, layout: ChunkType) -> Result<()> {
    let axis = match layout {
        ChunkType::Tile => MAX_TILED_AXIS,
        ChunkType::Strip => MAX_STRIPPED_AXIS,
    };
    if width == 0 || height == 0 || width > axis || height > axis {
        return Err(format!(
            "Generic {:?} raster dimensions exceed {axis} pixels per side",
            layout
        ));
    }
    if u64::from(width) * u64::from(height) > MAX_GRID_PIXELS {
        return Err("Generic raster grid exceeds 8 billion pixels; tiled access does not remove the grid limit".into());
    }
    Ok(())
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
    let file_limit = crate::source_transfer_limit_at(root, job)?;
    if size == 0
        || size > file_limit
        || size != job.bytes_downloaded
        || job.total_bytes.is_some_and(|n| n != size)
    {
        return Err(
            "Local original raster size changed or exceeds its approved source limit".into(),
        );
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
    let sha256 = format!("{:x}", hasher.finalize());
    if count != size
        || job.sha256.as_deref() != Some(sha256.as_str())
        || file.metadata().map_err(io_error)?.modified().ok() != metadata.modified().ok()
    {
        return Err("Local original raster size or SHA-256 changed".into());
    }
    if job.stac_source.is_some() {
        verify_transfer(root, job, count, &sha256)?;
    }
    file.seek(SeekFrom::Start(0)).map_err(io_error)?;
    Ok(file)
}
fn read(root: &Path, job: &Job) -> Result<Raster<RawPaletteReader<BufReader<File>>>> {
    let original = verified_original(root, job)?;
    let (file, sha256) = super::archive_original::raster_original(root, job, original)?;
    let mut raster = decode_identified(BufReader::new(file), &job.id, &sha256)?;
    raster.metadata.archive_original = super::archive_original::native_receipt(root, job)?
        .map(|value| serde_json::to_value(value).map_err(io_error))
        .transpose()?;
    Ok(raster)
}
fn chunk_layout<R: Read + Seek>(decoder: &mut Decoder<R>) -> Result<(u32, u32, u32)> {
    chunk_layout_with_limit(decoder, MAX_CHUNKS)
}
fn reviewed_chunk_limit(metadata: &GenericRasterInspection) -> u32 {
    if crate::datasets::sinolc::budget_preview_profile(metadata) {
        crate::datasets::sinolc::CHUNK_COUNT
    } else {
        MAX_CHUNKS
    }
}
fn chunk_layout_with_limit<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    limit: u32,
) -> Result<(u32, u32, u32)> {
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
    if count > limit {
        return Err("TIFF layout exceeds the bounded 131,072 chunk index".into());
    }
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
    let samples = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|value| value.checked_mul(bands as u64))
        .ok_or("TIFF chunk sample dimensions overflow")?;
    let bytes = samples
        .checked_mul(8)
        .ok_or("TIFF chunk byte size overflows")?;
    if bytes > MAX_DECODED as u64 {
        return Err("A TIFF chunk exceeds the 128 MiB raw inspection limit".into());
    }
    let pixels = decoder.read_chunk(index).map_err(io_error)?;
    if result_len(&pixels)
        != usize::try_from(samples).map_err(|_| "TIFF chunk cannot be indexed on this platform")?
    {
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
    let limit = reviewed_chunk_limit(&raster.metadata);
    let (cw, ch, columns) = chunk_layout_with_limit(&mut raster.decoder, limit)?;
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
fn reviewed_preview_minimum_dimensions(metadata: &GenericRasterInspection) -> Option<[u32; 2]> {
    if metadata.sha256 == HREA_HK_2019_SHA256
        && metadata.width == 2183
        && metadata.height == 1469
        && metadata.storage_layout == "tile"
        && metadata.crs.as_deref() == Some("EPSG:4326")
        && metadata.pixel_interpretation.as_deref() == Some("area")
        && metadata.bands.len() == 1
        && metadata.bands[0].data_type == "Float32"
        && metadata.bands[0].nodata == json!(-3.4e38)
    {
        return Some([2183, 1469]);
    }
    (metadata.sha256 == SRTM15PLUS_02_10_SHA256
        && metadata.width == 7200
        && metadata.height == 7200
        && metadata.storage_layout == "tile"
        && metadata.crs.as_deref() == Some("EPSG:4326")
        && metadata.pixel_interpretation.as_deref() == Some("area")
        && metadata.bands.len() == 1
        && metadata.bands[0].data_type == "Float32"
        && metadata.bands[0].nodata == "NaN")
        .then_some([3600, 3600])
}

fn preview_grid(dimensions: [u32; 2], edge: u32) -> [u32; 2] {
    let scale = (dimensions[0].max(dimensions[1]) as f64 / edge as f64).max(1.0);
    [
        (dimensions[0] as f64 / scale).ceil() as u32,
        (dimensions[1] as f64 / scale).ceil() as u32,
    ]
}

/// Preflight the real Cartesian set of source chunks needed by this display
/// grid. It uses the same raw-sample charge as the decoder, including all bands.
/// A rejected candidate stops charging after the unchanged limit is exceeded.
#[cfg(test)]
fn preview_grid_charge<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    dimensions: [u32; 2],
    bands: usize,
    edge: u32,
    budget: usize,
) -> Result<Option<(usize, usize)>> {
    preview_grid_charge_with_limit(decoder, dimensions, bands, edge, budget, MAX_CHUNKS)
}
fn preview_grid_charge_with_limit<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    dimensions: [u32; 2],
    bands: usize,
    edge: u32,
    budget: usize,
    chunk_limit: u32,
) -> Result<Option<(usize, usize)>> {
    let [width, height] = preview_grid(dimensions, edge);
    let (cw, ch, columns) = chunk_layout_with_limit(decoder, chunk_limit)?;
    let row_chunks: std::collections::BTreeSet<u32> = (0..height)
        .map(|y| ((u64::from(y) * u64::from(dimensions[1]) / u64::from(height)) as u32) / ch)
        .collect();
    let col_chunks: std::collections::BTreeSet<u32> = (0..width)
        .map(|x| ((u64::from(x) * u64::from(dimensions[0]) / u64::from(width)) as u32) / cw)
        .collect();
    let mut decoded = 0usize;
    let mut count = 0usize;
    for row in row_chunks {
        for col in &col_chunks {
            let (w, h) = decoder.chunk_data_dimensions(row * columns + col);
            let cost = (w as usize)
                .checked_mul(h as usize)
                .and_then(|n| n.checked_mul(bands))
                .and_then(|n| n.checked_mul(8))
                .ok_or("TIFF preview chunk cost overflows")?;
            if cost > MAX_DECODED {
                return Ok(None);
            }
            decoded = decoded
                .checked_add(cost)
                .ok_or("TIFF cumulative preview cost overflows")?;
            if decoded > budget {
                return Ok(None);
            }
            count += 1;
        }
    }
    Ok(Some((decoded, count)))
}

#[cfg(test)]
fn largest_budget_preview_edge<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    dimensions: [u32; 2],
    bands: usize,
    requested: u32,
    deadline: Instant,
) -> Result<(u32, usize, usize)> {
    largest_budget_preview_edge_with_limit(
        decoder, dimensions, bands, requested, deadline, MAX_CHUNKS,
    )
}
fn largest_budget_preview_edge_with_limit<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    dimensions: [u32; 2],
    bands: usize,
    requested: u32,
    deadline: Instant,
    chunk_limit: u32,
) -> Result<(u32, usize, usize)> {
    // Scan downwards rather than assuming that rounded sample/chunk sets are
    // monotonic. The first accepted candidate is the largest eligible grid.
    for edge in (1..=requested).rev() {
        if Instant::now() > deadline {
            return Err("Generic raster preview budget preflight timed out".into());
        }
        if let Some((decoded, chunks)) = preview_grid_charge_with_limit(
            decoder,
            dimensions,
            bands,
            edge,
            512 * 1024 * 1024,
            chunk_limit,
        )? {
            return Ok((edge, decoded, chunks));
        }
    }
    Err("No display grid fits the unchanged generic raster preview decoding budget".into())
}
/// Sample a verified transparency-mask IFD without turning its validity bits
/// into scalar NoData or changing numerical pixel values. The TIFF library
/// returns packed MSB-first one-bit rows; no whole-grid expansion is needed.
fn preview_mask<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    image: usize,
    dimensions: [u32; 2],
    width: u32,
    height: u32,
    deadline: Instant,
    decoded: &mut usize,
) -> Result<Vec<bool>> {
    decoder.seek_to_image(image).map_err(io_error)?;
    if decoder.dimensions().map_err(io_error)? != (dimensions[0], dimensions[1])
        || decoder
            .find_tag_unsigned::<u32>(Tag::NewSubfileType)
            .map_err(io_error)?
            .unwrap_or(0)
            & 4
            == 0
        || decoder
            .get_tag_u16_vec(Tag::BitsPerSample)
            .map_err(io_error)?
            != vec![1]
        || decoder
            .find_tag_unsigned::<u16>(Tag::SamplesPerPixel)
            .map_err(io_error)?
            .unwrap_or(1)
            != 1
        || decoder
            .find_tag_unsigned::<u16>(Tag::PhotometricInterpretation)
            .map_err(io_error)?
            .unwrap_or(0)
            != 1
        || decoder
            .find_tag_unsigned::<u16>(Tag::FillOrder)
            .map_err(io_error)?
            .unwrap_or(1)
            != 1
        || decoder
            .find_tag_unsigned::<u16>(Tag::Orientation)
            .map_err(io_error)?
            .unwrap_or(1)
            != 1
        || decoder
            .find_tag_unsigned::<u16>(Tag::Predictor)
            .map_err(io_error)?
            .unwrap_or(1)
            != 1
    {
        return Err("TIFF validity mask has an unsupported or mismatched one-bit layout".into());
    }
    let (cw, ch, columns) = chunk_layout(decoder)?;
    let mut requests: BTreeMap<u32, Vec<(usize, u32, u32)>> = BTreeMap::new();
    for y in 0..height {
        for x in 0..width {
            let row = (u64::from(y) * u64::from(dimensions[1]) / u64::from(height)) as u32;
            let col = (u64::from(x) * u64::from(dimensions[0]) / u64::from(width)) as u32;
            requests
                .entry(row / ch * columns + col / cw)
                .or_default()
                .push(((y * width + x) as usize, col % cw, row % ch));
        }
    }
    let mut validity = vec![false; (width * height) as usize];
    for (index, points) in requests {
        if Instant::now() > deadline {
            return Err("Generic raster validity-mask preview timed out".into());
        }
        let (w, h) = decoder.chunk_data_dimensions(index);
        let charge = (w as usize)
            .checked_mul(h as usize)
            .and_then(|n| n.checked_mul(8))
            .ok_or("TIFF mask chunk size overflows")?;
        if charge > MAX_DECODED {
            return Err("A TIFF mask chunk exceeds the 128 MiB raw inspection limit".into());
        }
        *decoded = decoded.saturating_add(charge);
        if *decoded > 512 * 1024 * 1024 {
            return Err(
                "Preview validity mask exceeds the unchanged 512 MiB cumulative chunk budget"
                    .into(),
            );
        }
        let bytes = match decoder.read_chunk(index).map_err(io_error)? {
            DecodingResult::U8(v) => v,
            _ => {
                return Err("TIFF validity-mask decoder did not return packed unsigned bits".into())
            }
        };
        let stride = w.div_ceil(8) as usize;
        if bytes.len()
            != stride
                .checked_mul(h as usize)
                .ok_or("TIFF mask row size overflows")?
        {
            return Err("TIFF validity-mask byte count differs from its packed rows".into());
        }
        for (position, col, row) in points {
            if col >= w || row >= h {
                return Err("TIFF mask sample lies outside its chunk".into());
            }
            validity[position] =
                bytes[row as usize * stride + col as usize / 8] & (0x80 >> (col % 8)) != 0;
        }
    }
    Ok(validity)
}
fn preview<R: Read + Seek>(raster: &mut Raster<R>, edge: u32) -> Result<()> {
    if !raster.supported {
        return Ok(());
    }
    let deadline = Instant::now() + Duration::from_secs(30);
    let chunk_limit = reviewed_chunk_limit(&raster.metadata);
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
        let samples = decoder
            .find_tag_unsigned::<u16>(Tag::SamplesPerPixel)
            .map_err(io_error)?
            .unwrap_or(1);
        let planar = decoder
            .find_tag_unsigned::<u16>(Tag::PlanarConfiguration)
            .map_err(io_error)?
            .unwrap_or(1);
        // Both layouts contain the same single plane when SamplesPerPixel=1.
        // IO originals declare separate planes while their overviews declare
        // interleaving; compare semantics without modifying the TIFF bytes.
        let planar = if samples == 1 && planar == 2 {
            1
        } else {
            planar
        };
        Ok(
            json!({"samples":samples,"bits":decoder.get_tag_u16_vec(Tag::BitsPerSample).map_err(io_error)?,"format":optional(decoder,Tag::SampleFormat)?,"orientation":decoder.find_tag_unsigned::<u16>(Tag::Orientation).map_err(io_error)?.unwrap_or(1),"planar":planar,"photo":decoder.find_tag_unsigned::<u16>(Tag::PhotometricInterpretation).map_err(io_error)?.unwrap_or(1),"nodata":optional(decoder,Tag::GdalNodata)?,"extraSamples":optional(decoder,Tag::ExtraSamples)?,"visualCodec": if decoder.find_tag_unsigned::<u16>(Tag::PhotometricInterpretation).map_err(io_error)? == Some(6) {json!({"compression":optional(decoder,Tag::Compression)?,"chromaSubsampling":optional(decoder,Tag::ChromaSubsampling)?,"referenceBlackWhite":optional(decoder,Tag::Unknown(532))?})} else {Value::Null}}),
        )
    }
    let original_tags = sample_tags(decoder)?;
    let mut image_levels = vec![(0, [raster.metadata.width, raster.metadata.height])];
    let mut mask_levels: BTreeMap<[u32; 2], usize> = BTreeMap::new();
    let reviewed_minimum = reviewed_preview_minimum_dimensions(&raster.metadata);
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
        if reduced & 4 != 0 {
            validate_dimensions(w, h, decoder.get_chunk_type())?;
            if decoder
                .get_tag_u16_vec(Tag::BitsPerSample)
                .map_err(io_error)?
                != vec![1]
                || decoder
                    .find_tag_unsigned::<u16>(Tag::SamplesPerPixel)
                    .map_err(io_error)?
                    .unwrap_or(1)
                    != 1
                || decoder
                    .find_tag_unsigned::<u16>(Tag::PhotometricInterpretation)
                    .map_err(io_error)?
                    .unwrap_or(0)
                    != 1
                || mask_levels.insert([w, h], image).is_some()
            {
                return Err("TIFF validity masks are ambiguous, duplicated or unsupported".into());
            }
            continue;
        }
        let factor = if w > 0 && h > 0 {
            (raster.metadata.width / w)
                .max(raster.metadata.height / h)
                .max(2)
        } else {
            2
        };
        let proportional = (factor.saturating_sub(1).max(2)..=factor.saturating_add(1)).any(|f| {
            // Publishers use both floor and ceil dimensions for an integer
            // reduction (INPE/GDAL floors). Retain the same common factor,
            // reduced-image flag and matching sample/NoData declarations.
            (raster.metadata.width / f == w || raster.metadata.width.div_ceil(f) == w)
                && (raster.metadata.height / f == h || raster.metadata.height.div_ceil(f) == h)
        });
        let matching_image = w > 0
            && h > 0
            && w <= raster.metadata.width
            && h <= raster.metadata.height
            && decoder.colortype().is_ok_and(|c| c == color)
            && reduced & 1 == 1
            && proportional
            && sample_tags(decoder).is_ok_and(|tags| tags == original_tags);
        if matching_image {
            image_levels.push((image, [w, h]));
        }
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
            && reviewed_minimum.is_none_or(|minimum| w >= minimum[0] && h >= minimum[1])
        {
            selected_image = image;
            dimensions = [w, h];
        }
    }
    if !mask_levels.is_empty() {
        if decoder.more_images() {
            return Err("TIFF validity-mask matching exceeds the existing sixteen-overview inspection limit".into());
        }
        if image_levels.len() != mask_levels.len()
            || image_levels
                .iter()
                .any(|(_, dims)| !mask_levels.contains_key(dims))
        {
            return Err(
                "TIFF validity masks do not match every accepted image level exactly".into(),
            );
        }
    }
    if reviewed_minimum.is_some_and(|minimum| dimensions != minimum) {
        return Err("The exact reviewed original cannot select its independently checked preview image under the existing TIFF layout safeguards; original pixel inspection remains available".into());
    }
    decoder.seek_to_image(selected_image).map_err(io_error)?;
    if selected_image > 0 {
        raster.metadata.limitations.push("Preview samples an embedded overview; raw pixel inspection always reads the original image.".into());
    }
    if reviewed_minimum.is_some() && raster.metadata.sha256 == SRTM15PLUS_02_10_SHA256 {
        raster.metadata.limitations.push(
            "This exact SHA-256-verified SRTM15+ V2.5.5 02_10 original has independently confirmed all-NaN factor-4/8/16 overviews. Preview skips only those known broken reduced images and samples the existing valid factor-2 overview within unchanged decoding budgets. Original bytes, raw pixels and NoData are unchanged; this is not a rule for other SRTM15+ tiles or legitimate all-NoData rasters.".into(),
        );
    }
    if reviewed_minimum.is_some() && raster.metadata.sha256 == HREA_HK_2019_SHA256 {
        raster.metadata.limitations.push("This exact SHA-256-verified HREA Hong Kong 2019 lightscore original has independently confirmed missing LZW end codes in factor-2 and factor-8 overview chunks. Preview samples the fully decoded original base image within unchanged budgets; damaged overviews and original bytes are preserved. The literal TIFF NoData declaration -3.39999999999999996e+38 (Float64 declaration -3.4e38) is distinct from its stored Float32 fill -3.3999999521443642e38; raw scalar matching retains that stored precision. This is not a general fallback for unknown rasters or other HREA files.".into());
    }
    let edge = if selected_image == 0
        && mask_levels.is_empty()
        && (crate::datasets::glc_fcs10::budget_preview_profile(&raster.metadata)
            || crate::datasets::gebco_tid::budget_preview_profile(&raster.metadata)
            || crate::datasets::glc_fcs30d::budget_preview_profile(&raster.metadata)
            || crate::datasets::sinolc::budget_preview_profile(&raster.metadata))
    {
        let (chosen, _, _) = largest_budget_preview_edge_with_limit(
            decoder,
            dimensions,
            raster.metadata.bands.len(),
            edge,
            deadline,
            chunk_limit,
        )?;
        if chosen < edge {
            let style = if raster.metadata.categorical_encoding.is_some() {
                "class-color"
            } else {
                "grayscale"
            };
            raster.metadata.limitations.push(format!("This exact SHA-256 and profile-reviewed original has no embedded overview. Its display preview uses the largest pixel grid that fits the unchanged 512 MiB cumulative chunk budget. This low-detail {style} preview is for browsing only; original bytes, full-resolution raw pixel values, class codes and georeference are unchanged."));
        }
        chosen
    } else {
        edge
    };
    let metadata = &mut raster.metadata;
    let [width, height] = preview_grid(dimensions, edge);
    let bands = metadata.bands.len();
    let (cw, ch, columns) = chunk_layout_with_limit(decoder, chunk_limit)?;
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
            selected[position] = value;
        }
    }
    if let Some(mask_image) = mask_levels.get(&dimensions) {
        let validity = preview_mask(
            decoder,
            *mask_image,
            dimensions,
            width,
            height,
            deadline,
            &mut decoded,
        )?;
        for (sample, valid) in selected.iter_mut().zip(validity) {
            if !valid {
                *sample = None;
            }
        }
        metadata.limitations.push("Preview alpha applies the original internal one-bit validity mask from the exact matching image level. Scalar NoData and raw pixel values remain separate and unchanged; a finite value under a zero validity bit is not a scientifically valid observation.".into());
    }
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for value in selected.iter().flatten() {
        min = min.min(*value);
        max = max.max(*value);
    }
    let range = (min.is_finite() && max.is_finite()).then_some([min, max]);
    let mut rgba = Vec::with_capacity(selected.len() * 4);
    for value in selected {
        if let Some(value) = value {
            if let Some(color) = crate::datasets::sinolc::categorical_color(metadata, value)
                .or_else(|| crate::datasets::clcd::categorical_color(metadata, value))
            {
                rgba.extend_from_slice(&color);
                continue;
            }
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

/// A bounded server-generated export, separate from original STAC assets.
/// Decode every chunk before accepting it; never infer sensor provenance from TIFF tags.
pub(crate) fn inspect_export(
    bytes: &[u8],
    id: &str,
    sha256: &str,
) -> Result<GenericRasterInspection> {
    inspect_bounded(bytes, id, sha256, "GEE export")
}

/// A complete numerical file generated by a reviewed regional data service.
/// This verifies its samples and grid, never publisher whole-grid identity.
pub(crate) fn inspect_service_export(
    bytes: &[u8],
    id: &str,
    sha256: &str,
) -> Result<GenericRasterInspection> {
    if bytes.is_empty() || bytes.len() > 32 * 1024 * 1024 {
        return Err("Numerical service subset exceeds the 32 MiB export budget".into());
    }
    inspect_bounded(bytes, id, sha256, "Numerical service subset")
}

/// A local unchanged source file. Registry ownership, original byte budgets
/// and file SHA checks belong to the caller; sensor provenance is never inferred.
pub(crate) fn inspect_local(
    bytes: &[u8],
    id: &str,
    sha256: &str,
) -> Result<GenericRasterInspection> {
    if bytes.is_empty() || bytes.len() > 32 * 1024 * 1024 {
        return Err("Local raster source exceeds the 32 MiB import budget".into());
    }
    inspect_bounded(bytes, id, sha256, "Local raster")
}

fn inspect_bounded(
    bytes: &[u8],
    id: &str,
    sha256: &str,
    context: &str,
) -> Result<GenericRasterInspection> {
    let mut raster = decode_identified(std::io::Cursor::new(bytes), id, sha256)?;
    if !raster.supported
        || raster.metadata.crs.is_none()
        || raster.metadata.transform.is_none()
        || raster.metadata.width > 10000
        || raster.metadata.height > 10000
    {
        return Err(format!(
            "{context} requires a supported georeferenced TIFF up to 10000 pixels per side"
        ));
    }
    let bands = raster.metadata.bands.len();
    let (_, ch, columns) = chunk_layout(&mut raster.decoder)?;
    let count = columns * raster.metadata.height.div_ceil(ch);
    let mut decoded = 0usize;
    let deadline = Instant::now() + Duration::from_secs(30);
    let work_limit = if context == "Local raster" {
        MAX_LOCAL_DECODE_WORK
    } else {
        MAX_DECODED
    };
    for index in 0..count {
        let (w, h) = raster.decoder.chunk_data_dimensions(index);
        decoded = decoded.saturating_add(w as usize * h as usize * bands * 8);
        if decoded > work_limit || Instant::now() > deadline {
            return Err(if context == "GEE export" {
                "GEE export exceeds the local decoding budget; export a smaller region".into()
            } else {
                "Local raster exceeds the unchanged native decoding budget".into()
            });
        }
        chunk(&mut raster.decoder, index, bands)?;
    }
    preview(&mut raster, 768)?;
    Ok(raster.metadata)
}

pub(crate) fn sample_local(
    bytes: &[u8],
    id: &str,
    sha256: &str,
    column: u32,
    row: u32,
) -> Result<GenericRasterPixel> {
    sample_bounded(bytes, id, sha256, column, row, "Local raster")
}

pub(crate) fn sample_service_export(
    bytes: &[u8],
    id: &str,
    sha256: &str,
    column: u32,
    row: u32,
) -> Result<GenericRasterPixel> {
    sample_bounded(bytes, id, sha256, column, row, "Numerical service subset")
}

fn sample_bounded(
    bytes: &[u8],
    id: &str,
    sha256: &str,
    column: u32,
    row: u32,
    context: &str,
) -> Result<GenericRasterPixel> {
    if bytes.is_empty() || bytes.len() > 32 * 1024 * 1024 {
        let budget = if context == "Local raster" {
            "import"
        } else {
            "export"
        };
        return Err(format!(
            "{context} source exceeds the 32 MiB {budget} budget"
        ));
    }
    let mut raster = decode_identified(std::io::Cursor::new(bytes), id, sha256)?;
    if !raster.supported
        || raster.metadata.crs.is_none()
        || raster.metadata.transform.is_none()
        || raster.metadata.width > 10000
        || raster.metadata.height > 10000
    {
        return Err(format!(
            "{context} requires a supported georeferenced TIFF up to 10000 pixels per side"
        ));
    }
    let (values, no_data) = pixel(&mut raster, column, row)?;
    Ok(GenericRasterPixel {
        job_id: id.into(),
        sha256: sha256.into(),
        column,
        row,
        values,
        no_data,
    })
}

#[cfg(test)]
mod local_source_tests {
    use super::*;
    use std::io::Cursor;
    use tiff::encoder::{colortype, TiffEncoder};
    #[test]
    fn real_gebco_numerical_subset_retains_every_signed_reference_sample_and_grid() {
        let bytes = include_bytes!("../../fixtures/gebco-region/gebco-2026-24x24.tif");
        let reference: Value =
            serde_json::from_str(include_str!("../../fixtures/gebco-region/reference.json"))
                .unwrap();
        let sha = format!("{:x}", Sha256::digest(bytes));
        assert_eq!(sha, reference["sha256"].as_str().unwrap());
        assert_eq!(bytes.len(), 8992);
        assert_eq!(reference["publisherStaticWholeOriginal"], false);
        let info = inspect_service_export(bytes, "official-region", &sha).unwrap();
        assert_eq!((info.width, info.height), (24, 24));
        assert_eq!(info.crs.as_deref(), Some("EPSG:4326"));
        assert_eq!(info.pixel_interpretation.as_deref(), Some("area"));
        assert_eq!(info.bands.len(), 1);
        assert_eq!(info.bands[0].data_type, "Int16");
        assert_eq!(info.bands[0].nodata, json!(-32767));
        assert!(info.archive_original.is_none() && info.sample_encoding.is_none());
        assert!(info.preview_data_url.is_some());
        for (actual, expected) in info.bounds.unwrap().iter().zip([-30.1, 40., -30., 40.1]) {
            assert!((actual - expected).abs() < 1e-12);
        }
        for row in 0..24 {
            for column in 0..24 {
                let pixel =
                    sample_service_export(bytes, "official-region", &sha, column, row).unwrap();
                assert_eq!(pixel.sha256, sha);
                assert_eq!(
                    pixel.values,
                    vec![reference["pixels"][row as usize][column as usize].clone()]
                );
                assert_eq!(
                    pixel.no_data,
                    vec![false],
                    "Negative bathymetry must not become NoData"
                );
            }
        }
        assert!(sample_service_export(bytes, "official-region", &sha, 24, 0).is_err());
        assert_eq!(format!("{:x}", Sha256::digest(bytes)), sha);
    }

    #[test]
    fn numerical_subset_keeps_local_decoder_budgets_and_does_not_claim_gee_identity() {
        let bytes = original(false);
        let error = inspect_service_export(&bytes, "region", "sha").unwrap_err();
        assert!(error.starts_with("Numerical service subset") && !error.contains("GEE"));
        let oversized = vec![0; 32 * 1024 * 1024 + 1];
        assert!(inspect_service_export(&oversized, "region", "sha")
            .unwrap_err()
            .contains("32 MiB"));
        assert!(sample_service_export(&oversized, "region", "sha", 0, 0)
            .unwrap_err()
            .contains("32 MiB"));
    }

    fn original(georeferenced: bool) -> Vec<u8> {
        let mut output = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut output).unwrap();
            let mut image = encoder.new_image::<colortype::Gray32Float>(3, 2).unwrap();
            image.encoder().write_tag(Tag::GdalNodata, "-9999").unwrap();
            if georeferenced {
                image
                    .encoder()
                    .write_tag(
                        Tag::GeoKeyDirectoryTag,
                        &[
                            1u16, 1, 0, 3, 1024, 0, 1, 2, 1025, 0, 1, 1, 2048, 0, 1, 4326,
                        ][..],
                    )
                    .unwrap();
                image
                    .encoder()
                    .write_tag(Tag::ModelPixelScaleTag, &[0.1f64, 0.1, 0.][..])
                    .unwrap();
                image
                    .encoder()
                    .write_tag(Tag::ModelTiepointTag, &[0f64, 0., 0., 10., 20., 0.][..])
                    .unwrap();
            }
            image
                .write_data(&[-5f32, 0., 12.5, -9999., 1.25, 100.])
                .unwrap();
        }
        output.into_inner()
    }
    #[test]
    fn local_original_pixels_keep_signed_float_zero_and_embedded_nodata() {
        let bytes = original(true);
        let sha = format!("{:x}", Sha256::digest(&bytes));
        let inspection = inspect_local(&bytes, "local-original", &sha).unwrap();
        assert_eq!((inspection.width, inspection.height), (3, 2));
        assert_eq!(inspection.crs.as_deref(), Some("EPSG:4326"));
        assert_eq!(inspection.sha256, sha);
        assert_eq!(inspection.bands[0].nodata, json!(-9999.));
        assert!(inspection.preview_data_url.is_some());
        for (column, row, value, missing) in
            [(0, 0, -5., false), (1, 0, 0., false), (0, 1, -9999., true)]
        {
            let p = sample_local(&bytes, "local-original", &sha, column, row).unwrap();
            assert_eq!(p.values, vec![json!(value)]);
            assert_eq!(p.no_data, vec![missing]);
        }
        assert!(sample_local(&bytes, "local-original", &sha, 3, 0).is_err());
    }
    #[test]
    fn local_rejects_nonspatial_or_oversize_input_without_claiming_gee_export() {
        let bytes = original(false);
        let error = inspect_local(&bytes, "local", "sha").unwrap_err();
        assert!(error.starts_with("Local raster") && !error.contains("GEE"));
        assert!(sample_local(&bytes, "local", "sha", 0, 0).is_err());
        assert!(
            inspect_local(&vec![0; 32 * 1024 * 1024 + 1], "local", "sha")
                .unwrap_err()
                .contains("32 MiB")
        );
    }

    #[test]
    fn local_streamed_validation_can_exceed_old_cumulative_work_without_raising_chunk_or_service_limits(
    ) {
        fn large_source(rows: u32) -> Vec<u8> {
            let mut output = Cursor::new(Vec::new());
            {
                let mut encoder = TiffEncoder::new(&mut output).unwrap();
                let mut image = encoder.new_image::<colortype::Gray8>(5000, 4000).unwrap();
                image.rows_per_strip(rows).unwrap();
                image
                    .encoder()
                    .write_tag(
                        Tag::GeoKeyDirectoryTag,
                        &[
                            1u16, 1, 0, 3, 1024, 0, 1, 2, 1025, 0, 1, 1, 2048, 0, 1, 4326,
                        ][..],
                    )
                    .unwrap();
                image
                    .encoder()
                    .write_tag(Tag::ModelPixelScaleTag, &[0.0001f64, 0.0001, 0.][..])
                    .unwrap();
                image
                    .encoder()
                    .write_tag(Tag::ModelTiepointTag, &[0f64, 0., 0., 116., 40., 0.][..])
                    .unwrap();
                image.write_data(&vec![17; 20_000_000]).unwrap();
            }
            output.into_inner()
        }
        let bytes = large_source(64);
        let sha = format!("{:x}", Sha256::digest(&bytes));
        let inspected = inspect_local(&bytes, "local", &sha).unwrap();
        assert_eq!((inspected.width, inspected.height), (5000, 4000));
        assert_eq!(
            sample_local(&bytes, "local", &sha, 4999, 3999)
                .unwrap()
                .values,
            vec![json!(17)]
        );
        assert!(inspect_service_export(&bytes, "service", &sha).is_err());
        assert!(inspect_local(&large_source(4000), "local-strip", &sha).is_err());
    }
}
impl JobManager {
    pub async fn inspect_stac_asset(&self, id: &str) -> Result<GenericRasterInspection> {
        self.inspect_stac_asset_band(id, None).await
    }
    pub async fn inspect_stac_asset_band(
        &self,
        id: &str,
        display_band: Option<u16>,
    ) -> Result<GenericRasterInspection> {
        let job = self.get(id).await.ok_or("Unknown job")?;
        if job.stac_source.is_none() {
            return Err("The task has no STAC source".into());
        }
        self.inspect_generic_raster_asset_band(job, display_band)
            .await
    }
    pub async fn inspect_wcs_asset(&self, id: &str) -> Result<GenericRasterInspection> {
        let job = self.get(id).await.ok_or("Unknown job")?;
        if job.wcs_source.is_none() {
            return Err("The task has no coverage request".into());
        }
        self.inspect_generic_raster_asset(job).await
    }
    async fn inspect_generic_raster_asset(&self, job: Job) -> Result<GenericRasterInspection> {
        self.inspect_generic_raster_asset_band(job, None).await
    }
    async fn inspect_generic_raster_asset_band(
        &self,
        job: Job,
        display_band: Option<u16>,
    ) -> Result<GenericRasterInspection> {
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
            if job.stac_source.is_some() && enmap::applies(&root, &job)? {
                return enmap::inspect(&root, &job, display_band)?
                    .ok_or("EnMAP original is missing".into());
            }
            if display_band.is_some_and(|b| b != 1) {
                return Err("Band selection is only available for the reviewed EnMAP cube".into());
            }
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
            if job.stac_source.is_some() && enmap::applies(&root, &job)? {
                return enmap::sample(&root, &job, column, row)?
                    .ok_or("EnMAP original is missing".into());
            }
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
mod reviewed_budget_preview_tests {
    use super::*;

    #[test]
    #[ignore = "requires GEOD_VANTOR_ORIGINAL_PATH with the complete independently reviewed original"]
    fn exact_vantor_original_decodes_ycbcr_components_not_scientific_rgb_and_native_luma_png() {
        use sha2::{Digest, Sha256};
        use std::fs;
        let path = PathBuf::from(std::env::var("GEOD_VANTOR_ORIGINAL_PATH").unwrap());
        let bytes = fs::read(path).unwrap();
        assert_eq!(bytes.len(), 29_534_465);
        let sha = format!("{:x}", Sha256::digest(&bytes));
        assert_eq!(sha, crate::datasets::vantor_open::ORIGINAL_SHA256);
        let mut r = decode_identified(std::io::Cursor::new(&bytes), "reviewed-vantor-visual", &sha)
            .unwrap();
        assert!(
            r.supported,
            "{:?}; profile {:?}; chroma {:?}; ref {:?}; compression {:?}",
            r.metadata,
            crate::datasets::vantor_open::visual_profile(&r.metadata),
            r.decoder.find_tag(Tag::ChromaSubsampling),
            r.decoder.find_tag(Tag::Unknown(532)),
            r.decoder.find_tag(Tag::Compression)
        );
        assert_eq!(
            r.metadata.sample_encoding.as_ref().unwrap()["components"],
            json!(["Y", "Cb", "Cr"])
        );
        for n in 0..8 {
            let mut changed = r.metadata.clone();
            match n {
                0 => changed.sha256 = "0".repeat(64),
                1 => changed.width += 1,
                2 => changed.bands[0].data_type = "Float32".into(),
                3 => changed.bands[0].nodata = json!(0),
                4 => changed.crs = Some("EPSG:3857".into()),
                5 => changed.pixel_interpretation = Some("point".into()),
                6 => changed.source_model_pixel_scale.as_mut().unwrap()[0] += 1e-8,
                _ => changed.source_model_tiepoint.as_mut().unwrap()[3] += 1.,
            }
            assert!(!crate::datasets::vantor_open::visual_profile(&changed));
        }
        let index: Value =
            serde_json::from_str(include_str!("../datasets/vantor-reviewed-index.json")).unwrap();
        let mut samples = vec![];
        for p in index["original"]["pixels"].as_array().unwrap() {
            let sample = pixel(
                &mut r,
                p["column"].as_u64().unwrap() as u32,
                p["row"].as_u64().unwrap() as u32,
            )
            .unwrap();
            assert_eq!(
                serde_json::to_value(&sample.0).unwrap(),
                p["values"],
                "independent libjpeg YCbCr reference at {p}"
            );
            assert_eq!(sample.1, vec![false; 3]);
            samples.push(
                json!({"column":p["column"],"row":p["row"],"values":sample.0,"noData":sample.1}),
            );
        }
        preview(&mut r, 768).unwrap();
        assert!(r.metadata.preview_width.unwrap() > 500);
        let png = STANDARD
            .decode(
                r.metadata
                    .preview_data_url
                    .as_ref()
                    .unwrap()
                    .split_once(',')
                    .unwrap()
                    .1,
            )
            .unwrap();
        let mut reader = png::Decoder::new(std::io::Cursor::new(&png))
            .read_info()
            .unwrap();
        let mut out = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut out).unwrap();
        let pixels = &out[..info.buffer_size()];
        assert!(pixels.chunks_exact(4).all(|p| p[3] == 255));
        assert!(
            pixels
                .chunks_exact(4)
                .map(|p| p[0])
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                > 16
        );
        let dir = PathBuf::from(std::env::var("GEOD_VANTOR_NATIVE_EVIDENCE_DIR").unwrap());
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("B120001101250A10-native-y-preview.png"), png).unwrap();
        fs::write(
            dir.join("B120001101250A10-native-inspection.json"),
            serde_json::to_vec_pretty(&r.metadata).unwrap(),
        )
        .unwrap();
        fs::write(
            dir.join("B120001101250A10-native-ycbcr-pixels.json"),
            serde_json::to_vec_pretty(&samples).unwrap(),
        )
        .unwrap();
        let mut rejected =
            decode_identified(std::io::Cursor::new(&bytes), "unreviewed", &"0".repeat(64)).unwrap();
        assert!(!rejected.supported);
        assert!(rejected.metadata.sample_encoding.is_none());
        assert!(pixel(&mut rejected, 10000, 10000).is_err());
    }

    #[test]
    fn preview_grid_preserves_aspect_and_never_exceeds_requested_edge() {
        assert_eq!(preview_grid([55_661, 55_662], 32), [32, 32]);
        assert_eq!(preview_grid([100, 200], 32), [16, 32]);
        assert_eq!(preview_grid([16, 12], 768), [16, 12]);
    }

    #[test]
    #[ignore = "requires GEOD_GLC_FCS10_ARCHIVE_DIR with complete reviewed originals"]
    fn exact_glc_fcs10_originals_get_budgeted_native_png_and_unchanged_raw_pixels() {
        use sha2::{Digest, Sha256};
        use std::io::Cursor;
        let root = PathBuf::from(std::env::var("GEOD_GLC_FCS10_ARCHIVE_DIR").unwrap());
        let index: Value =
            serde_json::from_str(include_str!("../datasets/glc-fcs10-reviewed-index.json"))
                .unwrap();
        let output = root.join("native-budget-preview");
        std::fs::create_dir_all(&output).unwrap();
        for tile in index["tiles"].as_array().unwrap().iter().filter(|v| {
            matches!(
                v["name"].as_str(),
                Some("GLC_FCS10_2023_W175N70.tif" | "GLC_FCS10_2023_W175S10.tif")
            )
        }) {
            let name = tile["name"].as_str().unwrap();
            let expected = tile["sha256"].as_str().unwrap();
            let bytes = std::fs::read(root.join("original-members").join(name)).unwrap();
            assert_eq!(format!("{:x}", Sha256::digest(&bytes)), expected);
            let mut raster = decode_identified(Cursor::new(bytes), name, expected).unwrap();
            assert!(crate::datasets::glc_fcs10::budget_preview_profile(
                &raster.metadata
            ));
            for mutation in 0..8 {
                let mut changed = raster.metadata.clone();
                match mutation {
                    0 => changed.sha256 = "0".repeat(64),
                    1 => changed.width += 1,
                    2 => changed.crs = Some("EPSG:3857".into()),
                    3 => changed.bands[0].nodata = json!(0),
                    4 => changed.bands[0].data_type = "UInt16".into(),
                    5 => changed.bounds.as_mut().unwrap()[0] += 1.,
                    6 => changed.pixel_interpretation = Some("point".into()),
                    _ => changed.source_model_pixel_scale.as_mut().unwrap()[0] *= 2.,
                };
                assert!(!crate::datasets::glc_fcs10::budget_preview_profile(
                    &changed
                ));
            }
            let dims = [raster.metadata.width, raster.metadata.height];
            let (edge, cost, chunks) = largest_budget_preview_edge(
                &mut raster.decoder,
                dims,
                1,
                768,
                Instant::now() + Duration::from_secs(30),
            )
            .unwrap();
            assert_eq!(edge, 32);
            assert_eq!(chunks, 1024);
            assert!(cost <= 512 * 1024 * 1024);
            assert!(
                preview_grid_charge(&mut raster.decoder, dims, 1, edge + 1, 512 * 1024 * 1024)
                    .unwrap()
                    .is_none()
            );
            let original_bounds = raster.metadata.bounds;
            let original_transform = raster.metadata.transform;
            preview(&mut raster, 768).unwrap();
            assert_eq!(raster.metadata.preview_width, Some(32));
            assert_eq!(raster.metadata.preview_height, Some(32));
            assert_eq!(raster.metadata.bounds, original_bounds);
            assert_eq!(raster.metadata.transform, original_transform);
            assert!(raster
                .metadata
                .limitations
                .iter()
                .any(|v| v.contains("low-detail grayscale preview")));
            let encoded = raster
                .metadata
                .preview_data_url
                .as_ref()
                .unwrap()
                .strip_prefix("data:image/png;base64,")
                .unwrap();
            let png = STANDARD.decode(encoded).unwrap();
            let mut reader = png::Decoder::new(Cursor::new(&png)).read_info().unwrap();
            let mut image = vec![0; reader.output_buffer_size().unwrap()];
            let frame = reader.next_frame(&mut image).unwrap();
            let samples = &image[..frame.buffer_size()];
            assert!(samples.chunks_exact(4).all(|v| v[3] == 255));
            let distinct: std::collections::BTreeSet<u8> =
                samples.chunks_exact(4).map(|v| v[0]).collect();
            assert!(
                distinct.len() > 1,
                "coarse original preview should retain meaningful variation"
            );
            let mut pixels = Vec::new();
            for sample in tile["pixels"].as_array().unwrap() {
                let col = sample["column"].as_u64().unwrap() as u32;
                let row = sample["row"].as_u64().unwrap() as u32;
                let (values, no_data) = pixel(&mut raster, col, row).unwrap();
                assert_eq!(values, sample["values"].as_array().unwrap().clone());
                assert_eq!(no_data, vec![false]);
                pixels.push(json!({"column":col,"row":row,"values":values,"noData":no_data}));
            }
            std::fs::write(output.join(format!("{name}.png")), png).unwrap();
            std::fs::write(
                output.join(format!("{name}.inspection.json")),
                serde_json::to_vec_pretty(&raster.metadata).unwrap(),
            )
            .unwrap();
            std::fs::write(
                output.join(format!("{name}.pixels.json")),
                serde_json::to_vec_pretty(&pixels).unwrap(),
            )
            .unwrap();
            std::fs::write(output.join(format!("{name}.budget.json")),serde_json::to_vec_pretty(&json!({"edge":edge,"previewWidth":32,"previewHeight":32,"sourceChunks":chunks,"chargedDecodedBytes":cost,"cumulativeBudget":512*1024*1024u64,"nextEdgeRejected":33,"distinctGrayValues":distinct})).unwrap()).unwrap();
        }
    }

    #[test]
    #[ignore = "requires GEOD_GLC_FCS30D_ARCHIVE_DIR with complete reviewed originals"]
    fn exact_glc_fcs30d_originals_keep_three_class_years_and_native_budgeted_preview() {
        use sha2::{Digest, Sha256};
        use std::io::{Cursor, Read};
        let root = PathBuf::from(std::env::var("GEOD_GLC_FCS30D_ARCHIVE_DIR").unwrap());
        let index: Value =
            serde_json::from_str(include_str!("../datasets/glc-fcs30d-reviewed-index.json"))
                .unwrap();
        let archive_bytes =
            std::fs::read(root.join("GLC_FCS30D_19852022maps_W175-W180.zip")).unwrap();
        assert_eq!(archive_bytes.len() as u64, 220928112);
        assert_eq!(
            format!("{:x}", Sha256::digest(&archive_bytes)),
            index["archiveSha256"].as_str().unwrap()
        );
        let mut archive = zip::ZipArchive::new(Cursor::new(archive_bytes)).unwrap();
        assert_eq!(archive.len(), 16);
        let output = root.join("native-budget-preview");
        std::fs::create_dir_all(&output).unwrap();
        for tile in index["tiles"].as_array().unwrap() {
            let name = tile["name"].as_str().unwrap();
            let expected = tile["sha256"].as_str().unwrap();
            let mut raw = Vec::new();
            archive
                .by_name(name)
                .unwrap()
                .read_to_end(&mut raw)
                .unwrap();
            assert_eq!(format!("{:x}", Sha256::digest(&raw)), expected);
            let mut raster = decode_identified(Cursor::new(raw), name, expected).unwrap();
            assert!(raster.supported);
            assert!(crate::datasets::glc_fcs30d::budget_preview_profile(
                &raster.metadata
            ));
            assert_eq!(raster.metadata.bands.len(), 3);
            let mut pixels = Vec::new();
            for sample in tile["pixels"].as_array().unwrap() {
                let col = sample["column"].as_u64().unwrap() as u32;
                let row = sample["row"].as_u64().unwrap() as u32;
                let (values, no_data) = pixel(&mut raster, col, row).unwrap();
                assert_eq!(values, sample["values"].as_array().unwrap().clone());
                assert_eq!(no_data, vec![false; 3]);
                pixels.push(json!({"column":col,"row":row,"values":values,"noData":no_data,"sampleYears":[1985,1990,1995]}));
            }
            if !matches!(
                name,
                "GLC_FCS30_19852000_5yearsMap_V1_W175N70.tif"
                    | "GLC_FCS30_19852000_5yearsMap_V1_W180N70.tif"
            ) {
                continue;
            }
            let dims = [raster.metadata.width, raster.metadata.height];
            let (edge, cost, chunks) = largest_budget_preview_edge(
                &mut raster.decoder,
                dims,
                3,
                768,
                Instant::now() + Duration::from_secs(30),
            )
            .unwrap();
            assert_eq!(edge, 18);
            assert_eq!(chunks, 324);
            assert_eq!(cost, 509607936);
            assert!(
                preview_grid_charge(&mut raster.decoder, dims, 3, 19, 512 * 1024 * 1024)
                    .unwrap()
                    .is_none()
            );
            let bounds = raster.metadata.bounds;
            preview(&mut raster, 768).unwrap();
            assert_eq!(raster.metadata.bounds, bounds);
            assert_eq!(raster.metadata.preview_width, Some(18));
            assert_eq!(raster.metadata.preview_height, Some(18));
            let encoded = raster
                .metadata
                .preview_data_url
                .as_ref()
                .unwrap()
                .strip_prefix("data:image/png;base64,")
                .unwrap();
            let png = STANDARD.decode(encoded).unwrap();
            let mut reader = png::Decoder::new(Cursor::new(&png)).read_info().unwrap();
            let mut image = vec![0; reader.output_buffer_size().unwrap()];
            let frame = reader.next_frame(&mut image).unwrap();
            let samples = &image[..frame.buffer_size()];
            assert!(samples.chunks_exact(4).all(|v| v[3] == 255));
            let distinct: std::collections::BTreeSet<u8> =
                samples.chunks_exact(4).map(|v| v[0]).collect();
            assert!(distinct.len() > 1);
            std::fs::write(output.join(format!("{name}.png")), png).unwrap();
            std::fs::write(
                output.join(format!("{name}.inspection.json")),
                serde_json::to_vec_pretty(&raster.metadata).unwrap(),
            )
            .unwrap();
            std::fs::write(
                output.join(format!("{name}.pixels.json")),
                serde_json::to_vec_pretty(&pixels).unwrap(),
            )
            .unwrap();
            std::fs::write(output.join(format!("{name}.budget.json")),serde_json::to_vec_pretty(&json!({"edge":edge,"sourceChunks":chunks,"chargedDecodedBytes":cost,"sampleYears":[1985,1990,1995],"displayYear":1985,"distinctGrayValues":distinct})).unwrap()).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    fn validity_fixture(mask_width: u32, duplicate: bool, mask_bits: u16) -> Vec<u8> {
        use std::io::Cursor;
        use tiff::encoder::{colortype, TiffEncoder};
        let mut output = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut output).unwrap();
            let mut image = encoder.new_image::<colortype::Gray8>(9, 2).unwrap();
            image.encoder().write_tag(Tag::GdalNodata, "255").unwrap();
            image
                .write_data(&[
                    10, 0, 250, 255, 40, 251, 60, 70, 80, 20, 30, 40, 50, 254, 60, 70, 80, 90,
                ])
                .unwrap();
            for _ in 0..if duplicate { 2 } else { 1 } {
                // TIFF encoder's source data is packed mask bytes. Its byte
                // width is replaced by the actual one-bit pixel width before
                // finishing this declared mask IFD; no product TIFF is edited.
                let mut mask = encoder
                    .new_image::<colortype::Gray8>(mask_width.div_ceil(8), 2)
                    .unwrap();
                mask.encoder()
                    .write_tag(Tag::ImageWidth, mask_width)
                    .unwrap();
                mask.encoder()
                    .write_tag(Tag::BitsPerSample, &[mask_bits][..])
                    .unwrap();
                mask.encoder()
                    .write_tag(Tag::PhotometricInterpretation, 4u16)
                    .unwrap();
                mask.encoder().write_tag(Tag::NewSubfileType, 4u32).unwrap();
                mask.rows_per_strip(1).unwrap();
                mask.write_data(&[0b11011011, 0b10000000, 0b11110111, 0b10000000])
                    .unwrap();
            }
        }
        let mut bytes = output.into_inner();
        // The generic Gray8 fixture encoder writes its compile-time sample
        // declaration on finish. Adjust only synthetic mask BitsPerSample to
        // the actual packed-bit fixture layout, retaining its byte row payload.
        let mut ifd = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        while ifd != 0 {
            let count = u16::from_le_bytes(bytes[ifd..ifd + 2].try_into().unwrap()) as usize;
            let entries = ifd + 2..ifd + 2 + count * 12;
            let is_mask = bytes[entries.clone()].chunks_exact(12).any(|e| {
                u16::from_le_bytes(e[..2].try_into().unwrap()) == 254
                    && u32::from_le_bytes(e[8..12].try_into().unwrap()) & 4 != 0
            });
            if is_mask {
                for entry in bytes[entries.clone()].chunks_exact_mut(12) {
                    if u16::from_le_bytes(entry[..2].try_into().unwrap()) == 258 {
                        entry[8..10].copy_from_slice(&mask_bits.to_le_bytes());
                    }
                }
            }
            ifd = u32::from_le_bytes(bytes[entries.end..entries.end + 4].try_into().unwrap())
                as usize;
        }
        bytes
    }
    #[test]
    fn original_internal_mask_controls_only_png_alpha_with_packed_row_padding() {
        use super::*;
        use std::io::Cursor;
        let bytes = validity_fixture(9, false, 1);
        let digest = hash(&bytes);
        let mut raster = decode_identified(Cursor::new(&bytes), "synthetic-mask", &digest).unwrap();
        assert_eq!(
            pixel(&mut raster, 2, 0).unwrap(),
            (vec![json!(250)], vec![false])
        );
        assert_eq!(
            pixel(&mut raster, 3, 0).unwrap(),
            (vec![json!(255)], vec![true])
        );
        assert_eq!(
            pixel(&mut raster, 1, 0).unwrap(),
            (vec![json!(0)], vec![false])
        );
        preview(&mut raster, 9).unwrap();
        assert_eq!(raster.metadata.display_range, Some([0., 90.]));
        let png = STANDARD
            .decode(
                raster
                    .metadata
                    .preview_data_url
                    .as_ref()
                    .unwrap()
                    .split_once(',')
                    .unwrap()
                    .1,
            )
            .unwrap();
        let mut reader = png::Decoder::new(Cursor::new(png)).read_info().unwrap();
        let mut rgba = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut rgba).unwrap();
        assert_eq!((info.width, info.height), (9, 2));
        for index in [2, 3, 5, 13] {
            assert_eq!(rgba[index * 4 + 3], 0);
        }
        for index in [0, 1, 4, 6, 7, 8, 9, 10, 11, 12, 14, 15, 16, 17] {
            assert_eq!(rgba[index * 4 + 3], 255);
        }
        assert_eq!(
            pixel(&mut raster, 2, 0).unwrap(),
            (vec![json!(250)], vec![false])
        );
        assert_eq!(hash(&bytes), digest);
        assert!(raster
            .metadata
            .limitations
            .iter()
            .any(|x| x.contains("one-bit validity mask")));
        assert!(!raster
            .metadata
            .limitations
            .iter()
            .any(|x| x.contains("colour palette")));
    }
    #[test]
    fn mismatched_duplicate_or_non_one_bit_masks_fail_closed() {
        use super::*;
        use std::io::Cursor;
        for (width, duplicate, bits) in [(10, false, 1), (9, true, 1), (9, false, 8)] {
            let bytes = validity_fixture(width, duplicate, bits);
            let digest = hash(&bytes);
            match decode_identified(Cursor::new(&bytes), "synthetic-bad-mask", &digest) {
                Ok(mut r) => {
                    assert!(preview(&mut r, 9).is_err());
                    assert!(r.metadata.preview_data_url.is_none());
                    assert_eq!(pixel(&mut r, 2, 0).unwrap().0, vec![json!(250)]);
                }
                Err(_) => assert_eq!(bits, 8),
            }
        }
    }
    #[test]
    #[ignore = "requires exact independently downloaded BII original via GEOD_BII_MASK_ORIGINAL"]
    fn independently_downloaded_bii_original_applies_validity_alpha_preserving_raw_values() {
        use super::*;
        let file = std::env::var("GEOD_BII_MASK_ORIGINAL").unwrap();
        let bytes = std::fs::read(file).unwrap();
        let digest = hash(&bytes);
        assert_eq!(
            digest,
            "a76ee54f166d68ce8ad6d61b6a58f262e04460e0dc9618a9a723404d55d5e843"
        );
        let mut raster = decode_identified(
            std::io::Cursor::new(&bytes),
            "independent-bii-mask",
            &digest,
        )
        .unwrap();
        assert_eq!(
            pixel(&mut raster, 3996, 3996).unwrap(),
            (vec![json!(0.6769765019416809)], vec![false])
        );
        assert_eq!(
            pixel(&mut raster, 2278, 2253).unwrap(),
            (vec![json!(0.3093675374984741)], vec![false])
        );
        preview(&mut raster, 768).unwrap();
        let png = STANDARD
            .decode(
                raster
                    .metadata
                    .preview_data_url
                    .as_ref()
                    .unwrap()
                    .split_once(',')
                    .unwrap()
                    .1,
            )
            .unwrap();
        if let Ok(path) = std::env::var("GEOD_BII_MASK_PREVIEW") {
            std::fs::write(path, png).unwrap();
        }
        assert!(raster
            .metadata
            .limitations
            .iter()
            .any(|x| x.contains("one-bit validity mask")));
        assert_eq!(
            pixel(&mut raster, 3996, 3996).unwrap(),
            (vec![json!(0.6769765019416809)], vec![false])
        );
        assert_eq!(hash(&bytes), digest);
    }
    #[test]
    #[ignore = "requires independently downloaded HREA original via GEOD_HREA_ORIGINAL"]
    fn independently_downloaded_hrea_audit_all_image_chunks() {
        use super::*;
        let bytes = std::fs::read(std::env::var("GEOD_HREA_ORIGINAL").unwrap()).unwrap();
        let digest = hash(&bytes);
        assert_eq!(
            digest,
            "c3a8d91858e609e06dfb701901bfb9582b7d472b173f1ad4e3c413ee84237718"
        );
        let mut raster =
            decode_identified(std::io::Cursor::new(&bytes), "hrea-ifd-audit", &digest).unwrap();
        let mut results = Vec::new();
        for image in 0..=16 {
            if image > 0 {
                if !raster.decoder.more_images() {
                    break;
                }
                raster.decoder.next_image().unwrap();
            }
            let dims = raster.decoder.dimensions().unwrap();
            let (_, ch, columns) = chunk_layout(&mut raster.decoder).unwrap();
            let mut failed = Vec::new();
            let mut good = 0;
            for index in 0..columns * dims.1.div_ceil(ch) {
                match chunk(&mut raster.decoder, index, 1) {
                    Ok(_) => good += 1,
                    Err(e) => failed.push(json!({"index":index,"error":e})),
                }
            }
            results.push(json!({"image":image,"dimensions":[dims.0,dims.1],"goodChunks":good,"failedChunks":failed}));
        }
        println!("{}", serde_json::to_string_pretty(&results).unwrap());
        if let Ok(path) = std::env::var("GEOD_HREA_AUDIT_OUTPUT") {
            std::fs::write(path, serde_json::to_vec_pretty(&results).unwrap()).unwrap();
        }
    }
    #[test]
    #[ignore = "requires exact independently downloaded HREA original via GEOD_HREA_ORIGINAL"]
    fn independently_downloaded_hrea_preview_uses_intact_base_and_keeps_declared_nodata() {
        use super::*;
        let bytes = std::fs::read(std::env::var("GEOD_HREA_ORIGINAL").unwrap()).unwrap();
        let digest = hash(&bytes);
        assert_eq!(digest, HREA_HK_2019_SHA256);
        let mut raster =
            decode_identified(std::io::Cursor::new(&bytes), "hrea-base-preview", &digest).unwrap();
        assert_eq!(raster.metadata.bands[0].nodata, json!(-3.4e38));
        assert_eq!(
            pixel(&mut raster, 1741, 545).unwrap(),
            (vec![json!(0.9527872204780579)], vec![false])
        );
        assert_eq!(
            pixel(&mut raster, 0, 0).unwrap(),
            (vec![json!(-3.3999999521443642e38)], vec![true])
        );
        for edge in [768, 160] {
            preview(&mut raster, edge).unwrap();
            assert!(raster
                .metadata
                .limitations
                .iter()
                .any(|x| x.contains("intact") || x.contains("original base image")));
            let png = STANDARD
                .decode(
                    raster
                        .metadata
                        .preview_data_url
                        .as_ref()
                        .unwrap()
                        .split_once(',')
                        .unwrap()
                        .1,
                )
                .unwrap();
            if let Ok(prefix) = std::env::var("GEOD_HREA_PREVIEW_OUTPUT") {
                std::fs::write(format!("{prefix}-{edge}.png"), png).unwrap();
            }
        }
        assert_eq!(
            pixel(&mut raster, 1741, 545).unwrap(),
            (vec![json!(0.9527872204780579)], vec![false])
        );
        assert_eq!(hash(&bytes), digest);
    }
    #[test]
    fn hrea_overview_exception_requires_exact_original_and_declaration() {
        use super::*;
        use std::io::Cursor;
        use tiff::encoder::{colortype, TiffEncoder};
        let mut out = Cursor::new(Vec::new());
        {
            let mut e = TiffEncoder::new(&mut out).unwrap();
            let mut im = e.new_image::<colortype::Gray32Float>(2, 2).unwrap();
            im.encoder()
                .write_tag(Tag::GdalNodata, "-3.39999999999999996e+38")
                .unwrap();
            im.write_data(&[0., 1., 0.5, 0.9]).unwrap();
        }
        let bytes = out.into_inner();
        let mut metadata =
            decode_identified(Cursor::new(&bytes), "shape-policy", HREA_HK_2019_SHA256)
                .unwrap()
                .metadata;
        metadata.width = 2183;
        metadata.height = 1469;
        metadata.storage_layout = "tile".into();
        metadata.crs = Some("EPSG:4326".into());
        metadata.pixel_interpretation = Some("area".into());
        assert_eq!(
            reviewed_preview_minimum_dimensions(&metadata),
            Some([2183, 1469])
        );
        for field in [0, 1, 2, 3, 4] {
            let mut changed = metadata.clone();
            match field {
                0 => changed.sha256 = "unknown".into(),
                1 => changed.width = 2182,
                2 => changed.bands[0].nodata = json!(-3.3999999521443642e38),
                3 => changed.bands[0].data_type = "Float64".into(),
                _ => changed.storage_layout = "strip".into(),
            };
            assert_eq!(reviewed_preview_minimum_dimensions(&changed), None);
        }
    }
    #[test]
    fn tiled_original_above_old_axis_limit_reads_the_last_pixel_without_full_grid_allocation() {
        use super::*;
        use std::io::Cursor;
        use tiff::encoder::{colortype, TiffEncoder};
        const WIDTH: u32 = 16;
        const HEIGHT: u32 = 65_792;
        let data: Vec<u8> = (0..HEIGHT)
            .flat_map(|row| [((row % 251) as u8); WIDTH as usize])
            .collect();
        let mut output = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut output).unwrap();
            let mut image = encoder
                .new_image::<colortype::Gray8>(WIDTH, HEIGHT)
                .unwrap();
            image.rows_per_strip(256).unwrap();
            image.encoder().write_tag(Tag::TileWidth, WIDTH).unwrap();
            image.encoder().write_tag(Tag::TileLength, 256u32).unwrap();
            image.write_data(&data).unwrap();
        }
        // A single tile column has exactly the same full-height chunk bytes as
        // these 256-row strips. Relabel the synthetic fixture's offset/count
        // entries, retaining every sample and never allocating a large grid.
        let mut bytes = output.into_inner();
        assert_eq!(&bytes[..2], b"II");
        let ifd = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let entries = u16::from_le_bytes(bytes[ifd..ifd + 2].try_into().unwrap()) as usize;
        let range = ifd + 2..ifd + 2 + entries * 12;
        let mut records: Vec<Vec<u8>> = bytes[range.clone()]
            .chunks_exact(12)
            .map(|v| v.to_vec())
            .collect();
        for record in &mut records {
            let tag = u16::from_le_bytes(record[..2].try_into().unwrap());
            let replacement = match tag {
                273 => Some(324u16),
                279 => Some(325u16),
                _ => None,
            };
            if let Some(tag) = replacement {
                record[..2].copy_from_slice(&tag.to_le_bytes());
            }
        }
        records.sort_by_key(|record| u16::from_le_bytes(record[..2].try_into().unwrap()));
        bytes[range].copy_from_slice(&records.concat());
        let original_hash = hash(&bytes);
        let mut raster =
            decode_identified(Cursor::new(&bytes), "synthetic-tall-tile", &original_hash).unwrap();
        assert_eq!(raster.metadata.storage_layout, "tile");
        assert_eq!(
            pixel(&mut raster, WIDTH - 1, HEIGHT - 1).unwrap().0,
            vec![json!((HEIGHT - 1) % 251)]
        );
        assert!(pixel(&mut raster, WIDTH, HEIGHT - 1).is_err());
        assert!(pixel(&mut raster, WIDTH - 1, HEIGHT).is_err());
        preview(&mut raster, 64).unwrap();
        assert!(raster.metadata.preview_data_url.is_some());
        assert_eq!(original_hash, hash(&bytes));
    }

    #[test]
    fn large_raster_budget_does_not_extend_strips_or_unbounded_tiled_grids() {
        use super::*;
        assert!(validate_dimensions(29_338, 89_448, ChunkType::Tile).is_ok());
        assert!(validate_dimensions(66_794, 88_550, ChunkType::Tile).is_ok());
        assert!(validate_dimensions(29_338, 89_448, ChunkType::Strip).is_err());
        assert!(validate_dimensions(41_326, 134_279, ChunkType::Tile).is_err());
        assert!(validate_dimensions(131_072, 131_072, ChunkType::Tile).is_err());
        assert!(validate_dimensions(0, 128, ChunkType::Tile).is_err());
    }

    #[test]
    fn single_palette_plane_and_interleaved_overview_preserve_original_indices() {
        let mut output = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut output).unwrap();
            for (width, height, planar, reduced, data) in [
                (9, 7, 2u16, 0u32, vec![11u8; 63]),
                (4, 3, 1u16, 1u32, vec![8u8; 12]),
            ] {
                let mut image = encoder
                    .new_image::<colortype::Gray8>(width, height)
                    .unwrap();
                image
                    .encoder()
                    .write_tag(Tag::PhotometricInterpretation, 3u16)
                    .unwrap();
                image
                    .encoder()
                    .write_tag(Tag::PlanarConfiguration, planar)
                    .unwrap();
                image
                    .encoder()
                    .write_tag(Tag::NewSubfileType, reduced)
                    .unwrap();
                image.encoder().write_tag(Tag::GdalNodata, "0").unwrap();
                image
                    .encoder()
                    .write_tag(Tag::ColorMap, &vec![65535u16; 3 * 256][..])
                    .unwrap();
                image.write_data(&data).unwrap();
            }
        }
        let bytes = output.into_inner();
        let sha = hash(&bytes);
        let mut raster =
            decode_identified(Cursor::new(&bytes), "synthetic-single-plane", &sha).unwrap();
        assert!(raster.supported);
        preview(&mut raster, 3).unwrap();
        assert_eq!(raster.metadata.display_range, Some([8., 8.]));
        assert!(raster
            .metadata
            .limitations
            .iter()
            .any(|s| s.contains("embedded overview")));
        assert_eq!(
            pixel(&mut raster, 8, 6).unwrap(),
            (vec![json!(11)], vec![false])
        );
        assert_eq!(hash(&bytes), sha);
    }

    #[test]
    fn multiple_separate_planes_remain_unsupported() {
        let mut output = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut output).unwrap();
            let mut image = encoder.new_image::<colortype::RGB8>(1, 1).unwrap();
            image
                .encoder()
                .write_tag(Tag::PlanarConfiguration, 2u16)
                .unwrap();
            image.write_data(&[1, 2, 3]).unwrap();
        }
        // Make the synthetic RGB file's three one-sample planes explicit.
        // The encoder only emits one interleaved strip by default.
        let mut bytes = output.into_inner();
        let directory = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let count =
            u16::from_le_bytes(bytes[directory..directory + 2].try_into().unwrap()) as usize;
        let entries: Vec<(u16, usize)> = (0..count)
            .map(|index| {
                let entry = directory + 2 + index * 12;
                (
                    u16::from_le_bytes(bytes[entry..entry + 2].try_into().unwrap()),
                    entry,
                )
            })
            .collect();
        for (tag, entry) in entries {
            if matches!(tag, 273 | 279) {
                let old = u32::from_le_bytes(bytes[entry + 8..entry + 12].try_into().unwrap());
                let values = if tag == 273 {
                    [old, old + 1, old + 2]
                } else {
                    [1, 1, 1]
                };
                let offset = bytes.len() as u32;
                bytes.extend(values.into_iter().flat_map(u32::to_le_bytes));
                bytes[entry + 4..entry + 8].copy_from_slice(&3u32.to_le_bytes());
                bytes[entry + 8..entry + 12].copy_from_slice(&offset.to_le_bytes());
            }
        }
        let mut independent = Decoder::new(Cursor::new(&bytes)).unwrap();
        assert_eq!(independent.strip_count().unwrap(), 3);
        assert!(independent.read_chunk(0).is_ok());
        let mut raster = decode(Cursor::new(bytes), &job()).unwrap();
        assert!(!raster.supported);
        assert!(pixel(&mut raster, 0, 0).is_err());
        preview(&mut raster, 3).unwrap();
        assert!(raster.metadata.preview_data_url.is_none());
    }

    #[test]
    fn independent_vertical_tags_do_not_remove_etopo_horizontal_grid_or_change_heights() {
        use super::*;
        use std::io::Cursor;
        use tiff::encoder::{colortype, TiffEncoder};
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut bytes).unwrap();
            let mut image = encoder.new_image::<colortype::Gray32Float>(2, 2).unwrap();
            image
                .encoder()
                .write_tag(
                    Tag::GeoKeyDirectoryTag,
                    &[
                        1u16, 1, 0, 4, 1024, 0, 1, 2, 1025, 0, 1, 1, 2048, 0, 1, 4326, 4096, 0, 1,
                        3855,
                    ][..],
                )
                .unwrap();
            image
                .encoder()
                .write_tag(Tag::ModelPixelScaleTag, &[1. / 240., 1. / 240., 1.][..])
                .unwrap();
            image
                .encoder()
                .write_tag(Tag::ModelTiepointTag, &[0., 0., 0., 0., 45., 0.][..])
                .unwrap();
            image.write_data(&[-8321.5f32, 0., 154.25, 8848.]).unwrap();
        }
        let original = bytes.into_inner();
        let sha = hash(&original);
        let mut raster = decode(Cursor::new(original.clone()), &job()).unwrap();
        assert_eq!(raster.metadata.crs.as_deref(), Some("EPSG:4326"));
        assert_eq!(raster.metadata.horizontal_crs.as_deref(), Some("EPSG:4326"));
        assert_eq!(raster.metadata.vertical_crs.as_deref(), Some("EPSG:3855"));
        assert_eq!(
            raster.metadata.source_model_pixel_scale,
            Some([1. / 240., 1. / 240., 1.])
        );
        assert_eq!(
            raster.metadata.source_model_tiepoint,
            Some([0., 0., 0., 0., 45., 0.])
        );
        assert_eq!(
            raster.metadata.bounds,
            Some([0., 45. - 2. / 240., 2. / 240., 45.])
        );
        assert_eq!(
            pixel(&mut raster, 0, 0).unwrap(),
            (vec![json!(-8321.5)], vec![false])
        );
        assert_eq!(pixel(&mut raster, 0, 1).unwrap().0, vec![json!(154.25)]);
        preview(&mut raster, 160).unwrap();
        assert!(raster.metadata.preview_data_url.is_some());
        assert_eq!(hash(&original), sha);
        assert!(raster
            .metadata
            .limitations
            .iter()
            .any(|s| s.contains("no vertical datum transformation")));
    }

    #[test]
    fn zstd_palette_raster_preserves_raw_class_indices_and_original_checksum() {
        use super::*;
        use std::io::Cursor;
        use tiff::encoder::{colortype, TiffEncoder};
        // Independently produced Zstandard frame for [0,10,50,80] repeated
        // 64 times; Python zstandard decoding verified all 256 original bytes.
        // TIFF's encoder has no Zstd mode, so only this fixture's strip tags
        // point at the complete compressed frame appended below.
        let frame = [
            40u8, 181, 47, 253, 96, 0, 0, 93, 0, 0, 32, 0, 10, 50, 80, 1, 0, 249, 41, 71, 4,
        ];
        let mut original = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut original).unwrap();
            let mut image = encoder.new_image::<colortype::Gray8>(256, 1).unwrap();
            image
                .encoder()
                .write_tag(Tag::PhotometricInterpretation, 3u16)
                .unwrap();
            image
                .encoder()
                .write_tag(Tag::ColorMap, &vec![65535u16; 768][..])
                .unwrap();
            image.encoder().write_tag(Tag::GdalNodata, "0").unwrap();
            image.write_data(&[0u8, 10, 50, 80].repeat(64)).unwrap();
        }
        let mut bytes = original.into_inner();
        assert_eq!(&bytes[..2], b"II");
        let frame_offset = bytes.len() as u32;
        bytes.extend(frame);
        let directory = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let count =
            u16::from_le_bytes(bytes[directory..directory + 2].try_into().unwrap()) as usize;
        let mut changed = 0;
        for index in 0..count {
            let entry = directory + 2 + index * 12;
            let tag = u16::from_le_bytes(bytes[entry..entry + 2].try_into().unwrap());
            match tag {
                259 => {
                    bytes[entry + 8..entry + 10].copy_from_slice(&50000u16.to_le_bytes());
                    changed += 1;
                }
                273 => {
                    bytes[entry + 8..entry + 12].copy_from_slice(&frame_offset.to_le_bytes());
                    changed += 1;
                }
                279 => {
                    bytes[entry + 8..entry + 12]
                        .copy_from_slice(&(frame.len() as u32).to_le_bytes());
                    changed += 1;
                }
                _ => (),
            }
        }
        assert_eq!(changed, 3);
        let original_hash = hash(&bytes);
        let mut raster = decode(Cursor::new(bytes.clone()), &job()).unwrap();
        for column in 0..256 {
            let (values, no_data) = pixel(&mut raster, column, 0).unwrap();
            let expected = [0, 10, 50, 80][column as usize % 4];
            assert_eq!(values, vec![json!(expected)]);
            assert_eq!(no_data, vec![expected == 0]);
        }
        preview(&mut raster, 160).unwrap();
        assert!(raster.metadata.preview_data_url.is_some());
        assert_eq!(hash(&bytes), original_hash);
        assert!(raster
            .metadata
            .limitations
            .iter()
            .any(|s| s.contains("palette indices/class codes")));
    }

    #[test]
    fn palette_indices_and_nodata_are_exact_for_u8_and_u16_without_file_mutation() {
        use super::*;
        use std::io::Cursor;
        use tiff::encoder::{colortype, TiffEncoder};
        for sixteen in [false, true] {
            let mut original = Cursor::new(Vec::new());
            {
                let mut encoder = TiffEncoder::new(&mut original).unwrap();
                if sixteen {
                    let mut image = encoder.new_image::<colortype::Gray16>(3, 1).unwrap();
                    image
                        .encoder()
                        .write_tag(Tag::PhotometricInterpretation, 3u16)
                        .unwrap();
                    image.encoder().write_tag(Tag::GdalNodata, "0").unwrap();
                    image
                        .encoder()
                        .write_tag(Tag::ColorMap, &vec![0u16; 3 * 65536][..])
                        .unwrap();
                    image.write_data(&[0, 50, 65535]).unwrap();
                } else {
                    let mut image = encoder.new_image::<colortype::Gray8>(3, 1).unwrap();
                    image
                        .encoder()
                        .write_tag(Tag::PhotometricInterpretation, 3u16)
                        .unwrap();
                    image.encoder().write_tag(Tag::GdalNodata, "0").unwrap();
                    image
                        .encoder()
                        .write_tag(Tag::ColorMap, &vec![65535u16; 3 * 256][..])
                        .unwrap();
                    image.write_data(&[0, 50, 100]).unwrap();
                }
            }
            let bytes = original.into_inner();
            let before_hash = hash(&bytes);
            let mut decoder = Decoder::new(Cursor::new(bytes.clone())).unwrap();
            assert_eq!(
                decoder
                    .get_tag_u16_vec(Tag::PhotometricInterpretation)
                    .unwrap(),
                vec![3]
            );
            assert!(decoder.read_image().is_err());
            let mut raster = decode(Cursor::new(bytes.clone()), &job()).unwrap();
            assert!(raster.supported);
            assert_eq!(
                pixel(&mut raster, 0, 0).unwrap(),
                (vec![json!(0)], vec![true])
            );
            assert_eq!(
                pixel(&mut raster, 1, 0).unwrap(),
                (vec![json!(50)], vec![false])
            );
            assert_eq!(
                pixel(&mut raster, 2, 0).unwrap().0,
                vec![json!(if sixteen { 65535 } else { 100 })]
            );
            preview(&mut raster, 160).unwrap();
            assert!(raster.metadata.preview_data_url.is_some());
            assert!(raster
                .metadata
                .limitations
                .iter()
                .any(|s| s.contains("palette indices/class codes")));
            assert_eq!(hash(&bytes), before_hash);
        }
    }

    #[test]
    fn unsupported_low_bit_palette_is_a_clean_layout_error() {
        use super::*;
        use std::io::Cursor;
        use tiff::encoder::{colortype, TiffEncoder};
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut bytes).unwrap();
            let mut image = encoder.new_image::<colortype::Gray8>(2, 1).unwrap();
            image
                .encoder()
                .write_tag(Tag::PhotometricInterpretation, 3u16)
                .unwrap();
            image.write_data(&[0, 10]).unwrap();
        }
        let mut bytes = bytes.into_inner();
        assert_eq!(&bytes[..2], b"II");
        let directory = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let count =
            u16::from_le_bytes(bytes[directory..directory + 2].try_into().unwrap()) as usize;
        for index in 0..count {
            let entry = directory + 2 + index * 12;
            if u16::from_le_bytes(bytes[entry..entry + 2].try_into().unwrap()) == 258 {
                bytes[entry + 8..entry + 10].copy_from_slice(&4u16.to_le_bytes());
            }
        }
        let mut raster = decode(Cursor::new(bytes), &job()).unwrap();
        assert!(!raster.supported);
        assert!(pixel(&mut raster, 0, 0).unwrap_err().contains("layout"));
        assert!(raster
            .metadata
            .limitations
            .iter()
            .any(|s| s.contains("UInt8 or UInt16")));
    }

    #[test]
    fn gee_integer_nodata_decimal_suffix_is_exact() {
        assert_eq!(
            super::NoData::parse("0.0", 1, 16).unwrap().json(),
            serde_json::json!(0)
        );
        assert_eq!(
            super::NoData::parse("-9999.000", 2, 16).unwrap().json(),
            serde_json::json!(-9999)
        );
        assert_eq!(
            super::NoData::parse("18446744073709551615.0", 1, 64)
                .unwrap()
                .json(),
            serde_json::json!("18446744073709551615")
        );
        assert!(super::NoData::parse("0.1", 1, 16).is_err());
        assert!(super::NoData::parse("65536.0", 1, 16).is_err());
        assert!(super::NoData::parse("1.000000000000000001", 1, 16).is_err());
    }
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
    fn decimal_float32_nodata_preserves_declaration_and_only_masks_its_exact_stored_sample() {
        let declared = -999.9f64;
        let stored = declared as f32;
        let adjacent = f32::from_bits(stored.to_bits() - 1);
        let values = [stored, adjacent, 0.0f32, 13.0f32];
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut bytes).unwrap();
            let mut image = encoder.new_image::<colortype::Gray32Float>(4, 1).unwrap();
            image
                .encoder()
                .write_tag(Tag::GdalNodata, "-999.9")
                .unwrap();
            image.write_data(&values).unwrap();
        }
        bytes.set_position(0);
        let mut raster = decode(bytes, &job()).unwrap();
        assert_eq!(raster.metadata.bands[0].nodata, json!(declared));
        for (column, value) in values.iter().enumerate() {
            let (raw, no_data) = pixel(&mut raster, column as u32, 0).unwrap();
            assert_eq!(raw, vec![json!(*value as f64)]);
            assert_eq!(no_data, vec![column == 0]);
        }
        preview(&mut raster, 160).unwrap();
        assert_eq!(raster.metadata.display_range, Some([adjacent as f64, 13.0]));
        assert!(NoData::parse("1e99", 3, 32).is_err());
        let double = NoData::parse("-999.9", 3, 64).unwrap();
        assert!(is_nodata(&json!(declared), Some(double)));
        assert!(!is_nodata(&json!(stored as f64), Some(double)));
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
    #[test]
    fn reviewed_bad_overview_policy_requires_exact_byte_identity_and_original_grid() {
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut bytes).unwrap();
            let mut image = encoder.new_image::<colortype::Gray32Float>(2, 2).unwrap();
            image.encoder().write_tag(Tag::GdalNodata, "nan").unwrap();
            image.write_data(&[0.0, -0.5, 7.0, f32::NAN]).unwrap();
        }
        bytes.set_position(0);
        let mut metadata = decode(bytes, &job()).unwrap().metadata;
        // Exercise policy boundaries only. The production read path verifies
        // original size/SHA before decoding; this synthetic TIFF is not a
        // downloaded/publisher-accepted original.
        assert_eq!(reviewed_preview_minimum_dimensions(&metadata), None);
        metadata.sha256 = SRTM15PLUS_02_10_SHA256.into();
        metadata.width = 7200;
        metadata.height = 7200;
        metadata.storage_layout = "tile".into();
        metadata.crs = Some("EPSG:4326".into());
        metadata.pixel_interpretation = Some("area".into());
        assert_eq!(
            reviewed_preview_minimum_dimensions(&metadata),
            Some([3600, 3600])
        );
        for field in [
            "sha", "width", "height", "layout", "crs", "area", "type", "nodata", "bands",
        ] {
            let mut changed = metadata.clone();
            match field {
                "sha" => changed.sha256 = "another-actual-tile".into(),
                "width" => changed.width = 7199,
                "height" => changed.height = 7199,
                "layout" => changed.storage_layout = "strip".into(),
                "crs" => changed.crs = None,
                "area" => changed.pixel_interpretation = Some("point".into()),
                "type" => changed.bands[0].data_type = "Int16".into(),
                "nodata" => changed.bands[0].nodata = json!(-9999),
                _ => changed.bands.push(changed.bands[0].clone()),
            }
            assert_eq!(
                reviewed_preview_minimum_dimensions(&changed),
                None,
                "{field}"
            );
        }
    }

    #[test]
    fn unreviewed_or_legitimate_all_nodata_overviews_are_not_reclassified_as_broken() {
        for original_has_data in [true, false] {
            let mut bytes = Cursor::new(Vec::new());
            {
                let mut encoder = TiffEncoder::new(&mut bytes).unwrap();
                let original = if original_has_data {
                    (0..256).map(|x| x as f32).collect::<Vec<_>>()
                } else {
                    vec![f32::NAN; 256]
                };
                let mut image = encoder.new_image::<colortype::Gray32Float>(16, 16).unwrap();
                image.encoder().write_tag(Tag::GdalNodata, "nan").unwrap();
                image.write_data(&original).unwrap();
                for dimension in [8u32, 4, 2] {
                    let mut image = encoder
                        .new_image::<colortype::Gray32Float>(dimension, dimension)
                        .unwrap();
                    image.encoder().write_tag(Tag::GdalNodata, "nan").unwrap();
                    image
                        .encoder()
                        .write_tag(Tag::NewSubfileType, 1u32)
                        .unwrap();
                    image
                        .write_data(&vec![f32::NAN; (dimension * dimension) as usize])
                        .unwrap();
                }
            }
            let original_bytes = bytes.into_inner();
            let sha = hash(&original_bytes);
            let mut raster = decode_identified(
                Cursor::new(&original_bytes),
                "synthetic-unknown-overviews",
                &sha,
            )
            .unwrap();
            preview(&mut raster, 2).unwrap();
            assert_eq!(raster.metadata.display_range, None);
            assert!(raster
                .metadata
                .limitations
                .iter()
                .any(|s| s.contains("embedded overview")));
            assert!(!raster
                .metadata
                .limitations
                .iter()
                .any(|s| s.contains("known broken")));
            let (values, nodata) = pixel(&mut raster, 0, 0).unwrap();
            assert_eq!(nodata, vec![!original_has_data]);
            assert_eq!(
                values,
                if original_has_data {
                    vec![json!(0.0)]
                } else {
                    vec![json!("NaN")]
                }
            );
            assert_eq!(hash(&original_bytes), sha);
        }
    }

    /// Optional local-original rendering regression. It does not create a
    /// native transfer/store or provide a source file to any download job.
    #[test]
    #[ignore = "requires the complete SHA-verified SRTM15+ 02_10 original in SRTM15PLUS_REVIEWED_TEST_FILE"]
    fn srtm15plus_reviewed_original_uses_valid_factor_two_without_changing_raw_pixels() {
        use std::path::PathBuf;
        let path = PathBuf::from(
            std::env::var("SRTM15PLUS_REVIEWED_TEST_FILE").expect("explicit local original path"),
        );
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(hash(&bytes), SRTM15PLUS_02_10_SHA256);
        let mut raster = decode_identified(
            Cursor::new(&bytes),
            "local-render-regression-only",
            SRTM15PLUS_02_10_SHA256,
        )
        .unwrap();
        let before = [(0, 0), (3600, 3600), (7199, 7199)]
            .map(|(column, row)| pixel(&mut raster, column, row).unwrap());
        for edge in [768, 160] {
            preview(&mut raster, edge).unwrap();
            assert!(raster
                .metadata
                .display_range
                .is_some_and(|[min, max]| min.is_finite() && max.is_finite() && max > min));
            assert!(raster
                .metadata
                .limitations
                .iter()
                .any(|s| s.contains("known broken")));
            assert!(raster
                .metadata
                .limitations
                .iter()
                .any(|s| s.contains("embedded overview")));
            let png_bytes = STANDARD
                .decode(
                    raster
                        .metadata
                        .preview_data_url
                        .as_ref()
                        .unwrap()
                        .split_once(',')
                        .unwrap()
                        .1,
                )
                .unwrap();
            let mut reader = png::Decoder::new(Cursor::new(&png_bytes))
                .read_info()
                .unwrap();
            let mut decoded = vec![0; reader.output_buffer_size().unwrap()];
            let frame = reader.next_frame(&mut decoded).unwrap();
            let rgba = &decoded[..frame.buffer_size()];
            assert_eq!((frame.width, frame.height), (edge, edge));
            assert!(rgba.chunks_exact(4).all(|p| p[3] == 255));
            assert!(rgba.chunks_exact(4).any(|p| p[0] == 0));
            assert!(rgba.chunks_exact(4).any(|p| p[0] == 255));
            if let Ok(out) = std::env::var("SRTM15PLUS_REVIEWED_TEST_OUTPUT") {
                std::fs::write(
                    PathBuf::from(out).join(format!("srtm15plus-native-{edge}.png")),
                    &png_bytes,
                )
                .unwrap();
            }
            for (index, (column, row)) in
                [(0, 0), (3600, 3600), (7199, 7199)].into_iter().enumerate()
            {
                assert_eq!(pixel(&mut raster, column, row).unwrap(), before[index]);
            }
        }
        assert_eq!(hash(&bytes), SRTM15PLUS_02_10_SHA256);
    }
}
