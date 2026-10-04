use super::*;
use std::{
    fs::File,
    io::{BufReader, Seek, SeekFrom},
    time::Instant,
};
use tiff::{
    decoder::{ChunkType, Decoder, DecodingResult, Limits},
    tags::Tag,
};
use tokio_util::sync::CancellationToken;

struct Guard<'a> {
    cancel: &'a CancellationToken,
    deadline: Instant,
}
impl Guard<'_> {
    fn check(&self) -> Result<()> {
        if self.cancel.is_cancelled() {
            return Err("WCS validation was cancelled".into());
        }
        if Instant::now() > self.deadline {
            return Err("WCS validation exceeded its 60-second budget".into());
        }
        Ok(())
    }
    fn check_io(&self) -> std::io::Result<()> {
        self.check().map_err(std::io::Error::other)
    }
}
struct GuardedReader<'a, 'b, R> {
    inner: R,
    guard: &'a Guard<'b>,
}
impl<R: Read> Read for GuardedReader<'_, '_, R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.guard.check_io()?;
        self.inner.read(buffer)
    }
}
impl<R: Seek> Seek for GuardedReader<'_, '_, R> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.guard.check_io()?;
        self.inner.seek(position)
    }
}

#[derive(Clone, Copy, Debug)]
enum Number {
    U(u64),
    I(i64),
    F(f64),
}
impl Number {
    fn parse(value: &str, format: u16, bits: u16) -> Option<Self> {
        match format {
            1 => value
                .parse::<u64>()
                .ok()
                .filter(|v| bits == 64 || *v < 1u64 << bits)
                .map(Self::U),
            2 => value
                .parse::<i64>()
                .ok()
                .filter(|v| bits == 64 || (-(1i64 << (bits - 1))..(1i64 << (bits - 1))).contains(v))
                .map(Self::I),
            3 if bits == 32 => value.parse::<f32>().ok().map(|v| Self::F(v as f64)),
            3 => value.parse::<f64>().ok().map(Self::F),
            _ => None,
        }
    }
    fn same(self, other: Self) -> bool {
        match (self, other) {
            (Self::U(a), Self::U(b)) => a == b,
            (Self::I(a), Self::I(b)) => a == b,
            (Self::F(a), Self::F(b)) => a == b || a.is_nan() && b.is_nan(),
            _ => false,
        }
    }
    fn nil(value: &str, format: u16, bits: u16) -> Result<Option<Self>> {
        let number = value
            .parse::<f64>()
            .map_err(|_| "WCS nil declaration is not a supported numeric literal")?;
        // Non-finite sentinels cannot occur in an integer sample. Other lexical
        // forms must be compared exactly for the decoded dtype, never dropped.
        let explicit_nonfinite = matches!(
            value.to_ascii_lowercase().as_str(),
            "nan"
                | "+nan"
                | "-nan"
                | "inf"
                | "+inf"
                | "-inf"
                | "infinity"
                | "+infinity"
                | "-infinity"
        );
        if !number.is_finite() && !explicit_nonfinite {
            return Err("WCS nil declaration overflows supported numeric comparison".into());
        }
        if format != 3 && explicit_nonfinite {
            return Ok(None);
        }
        let parsed = Self::parse(value, format, bits)
            .ok_or("WCS nil declaration cannot be compared exactly with the TIFF sample type")?;
        if let Self::F(parsed) = parsed {
            if number.is_finite() && parsed.is_infinite() {
                return Err("WCS nil declaration overflows the TIFF sample type".into());
            }
        }
        Ok(Some(parsed))
    }
}
fn sample(pixels: &DecodingResult, index: usize) -> Option<Number> {
    match pixels {
        DecodingResult::U8(v) => v.get(index).map(|v| Number::U(*v as u64)),
        DecodingResult::U16(v) => v.get(index).map(|v| Number::U(*v as u64)),
        DecodingResult::U32(v) => v.get(index).map(|v| Number::U(*v as u64)),
        DecodingResult::U64(v) => v.get(index).map(|v| Number::U(*v)),
        DecodingResult::I8(v) => v.get(index).map(|v| Number::I(*v as i64)),
        DecodingResult::I16(v) => v.get(index).map(|v| Number::I(*v as i64)),
        DecodingResult::I32(v) => v.get(index).map(|v| Number::I(*v as i64)),
        DecodingResult::I64(v) => v.get(index).map(|v| Number::I(*v)),
        DecodingResult::F16(v) => v.get(index).map(|v| Number::F(v.to_f32() as f64)),
        DecodingResult::F32(v) => v.get(index).map(|v| Number::F(*v as f64)),
        DecodingResult::F64(v) => v.get(index).map(|v| Number::F(*v)),
    }
}
fn len(pixels: &DecodingResult) -> usize {
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
fn decoded_type(pixels: &DecodingResult) -> (u16, u16) {
    match pixels {
        DecodingResult::U8(_) => (1, 8),
        DecodingResult::U16(_) => (1, 16),
        DecodingResult::U32(_) => (1, 32),
        DecodingResult::U64(_) => (1, 64),
        DecodingResult::I8(_) => (2, 8),
        DecodingResult::I16(_) => (2, 16),
        DecodingResult::I32(_) => (2, 32),
        DecodingResult::I64(_) => (2, 64),
        DecodingResult::F16(_) => (3, 16),
        DecodingResult::F32(_) => (3, 32),
        DecodingResult::F64(_) => (3, 64),
    }
}
fn close(a: f64, b: f64, pixel: f64) -> bool {
    a.is_finite() && b.is_finite() && (a - b).abs() <= pixel.abs() * 1e-6 + b.abs() * 1e-12
}
#[cfg(test)]
pub(super) fn tiff<R: Read + Seek>(
    reader: R,
    plan: &Plan,
    cancel: &CancellationToken,
) -> Result<()> {
    tiff_checked(
        reader,
        plan,
        &Guard {
            cancel,
            deadline: Instant::now() + Duration::from_secs(60),
        },
    )
}
fn tiff_checked<R: Read + Seek>(reader: R, plan: &Plan, guard: &Guard<'_>) -> Result<()> {
    guard.check()?;
    let mut limits = Limits::default();
    limits.decoding_buffer_size = 128 * 1024 * 1024;
    limits.intermediate_buffer_size = 32 * 1024 * 1024;
    limits.ifd_value_size = 1024 * 1024;
    let mut decoder = Decoder::new(GuardedReader {
        inner: reader,
        guard,
    })
    .map_err(|_| "WCS result is not a supported TIFF")?
    .with_limits(limits);
    if decoder.dimensions().map_err(io_error)? != (plan.width, plan.height) {
        return Err("WCS TIFF dimensions differ from the native subset plan".into());
    }
    let samples = decoder
        .find_tag_unsigned::<u16>(Tag::SamplesPerPixel)
        .map_err(io_error)?
        .unwrap_or(1);
    if samples as usize != plan.description.fields.len() {
        return Err("WCS TIFF band count differs from its range fields".into());
    }
    if decoder
        .find_tag_unsigned::<u16>(Tag::Orientation)
        .map_err(io_error)?
        .unwrap_or(1)
        != 1
        || decoder
            .find_tag_unsigned::<u16>(Tag::PlanarConfiguration)
            .map_err(io_error)?
            .unwrap_or(1)
            != 1
    {
        return Err("WCS TIFF requires top-left, interleaved samples".into());
    }
    if !matches!(
        decoder
            .get_tag_unsigned::<u16>(Tag::PhotometricInterpretation)
            .map_err(io_error)?,
        1 | 2
    ) {
        return Err(
            "WCS TIFF photometric interpretation could alter raw samples and is unsupported".into(),
        );
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
        || bits.len() != 1 && bits.len() != samples as usize
        || formats.len() != 1 && formats.len() != samples as usize
        || bits.iter().any(|b| *b != bits[0])
        || formats.iter().any(|f| *f != formats[0])
        || !matches!(
            (formats[0], bits[0]),
            (1 | 2, 8 | 16 | 32 | 64) | (3, 16 | 32 | 64)
        )
    {
        return Err("WCS TIFF has unsupported or inconsistent sample type declarations".into());
    }
    let keys = decoder
        .get_tag_u16_vec(Tag::GeoKeyDirectoryTag)
        .map_err(io_error)?;
    if keys.len() < 4 || keys[0] != 1 || keys.len() != 4 + keys[3] as usize * 4 {
        return Err("WCS TIFF geokey directory is invalid".into());
    }
    let mut entries = BTreeMap::new();
    for key in keys[4..].chunks_exact(4) {
        if entries.insert(key[0], (key[1], key[2], key[3])).is_some() {
            return Err("WCS TIFF repeats a geokey".into());
        }
    }
    let direct = |key| -> Result<Option<u16>> {
        match entries.get(&key) {
            None => Ok(None),
            Some((0, 1, v)) => Ok(Some(*v)),
            Some(_) => Err("WCS TIFF uses unsupported indirect CRS keys".into()),
        }
    };
    let model = direct(1024)?.ok_or("WCS TIFF has no coordinate model")?;
    let code = match model {
        1 => direct(3072)?,
        2 => direct(2048)?,
        _ => return Err("WCS TIFF uses an unsupported coordinate model".into()),
    }
    .ok_or("WCS TIFF has no EPSG code")?;
    if format!("EPSG:{code}") != plan.description.crs || (code == 4326) != (model == 2) {
        return Err("WCS TIFF CRS differs from its native coverage".into());
    }
    if model == 1 && direct(3076)?.is_some_and(|u| u != 9001)
        || model == 2 && direct(2054)?.is_some_and(|u| u != 9102)
    {
        return Err("WCS TIFF coordinate units conflict with its EPSG CRS".into());
    }
    let interpretation = direct(1025)?.unwrap_or(1);
    if !matches!(interpretation, 1 | 2) {
        return Err("WCS TIFF has invalid raster interpretation".into());
    }
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
        plan.width,
        plan.height,
        transform.as_deref(),
        scale.as_deref(),
        tiepoint.as_deref(),
    )?;
    if interpretation == 2 {
        bounds[0] -= spacing[0] / 2.;
        bounds[2] -= spacing[0] / 2.;
        bounds[1] += spacing[1] / 2.;
        bounds[3] += spacing[1] / 2.;
    }
    if !close(spacing[0], plan.transform[0], plan.transform[0])
        || !close(spacing[1], -plan.transform[4], plan.transform[4])
        || bounds
            .iter()
            .zip(plan.native_bounds)
            .enumerate()
            .any(|(i, (a, b))| !close(*a, b, spacing[i % 2]))
    {
        return Err(
            "WCS TIFF was shifted, rescaled or clipped differently from the native grid plan"
                .into(),
        );
    }
    let nodata_text = decoder
        .find_tag(Tag::GdalNodata)
        .map_err(io_error)?
        .map(|v| v.into_string())
        .transpose()
        .map_err(io_error)?;
    let nodata = nodata_text
        .as_deref()
        .map(|s| {
            Number::parse(s.trim_matches('\0').trim(), formats[0], bits[0])
                .ok_or("WCS TIFF NoData is incompatible with its sample type")
        })
        .transpose()?;
    let nil_values = plan
        .description
        .fields
        .iter()
        .map(|field| {
            field
                .nil_values
                .iter()
                .map(|n| Number::nil(&n.value, formats[0], bits[0]))
                .collect::<Result<Vec<_>>>()
                .map(|values| values.into_iter().flatten().collect::<Vec<_>>())
        })
        .collect::<Result<Vec<_>>>()?;
    if formats[0] == 3
        && bits[0] == 16
        && (nodata.is_some() || nil_values.iter().any(|v| !v.is_empty()))
    {
        return Err("Float16 WCS nil validation is unsupported".into());
    }
    if let Some(nodata) = nodata {
        for nil in &nil_values {
            if !nil.is_empty() && !nil.iter().any(|n| n.same(nodata)) {
                return Err("WCS TIFF NoData conflicts with its declared nil values".into());
            }
        }
    }
    let (cw, ch) = decoder.chunk_dimensions();
    if cw == 0 || ch == 0 {
        return Err("WCS TIFF has empty chunks".into());
    }
    let count = match decoder.get_chunk_type() {
        ChunkType::Tile => decoder.tile_count().map_err(io_error)?,
        ChunkType::Strip => decoder.strip_count().map_err(io_error)?,
    };
    if plan
        .width
        .div_ceil(cw)
        .checked_mul(plan.height.div_ceil(ch))
        != Some(count)
    {
        return Err("WCS TIFF chunk layout is incomplete".into());
    }
    let mut total = 0u64;
    for index in 0..count {
        guard.check()?;
        let (w, h) = decoder.chunk_data_dimensions(index);
        let expected = u64::from(w)
            .checked_mul(u64::from(h))
            .and_then(|n| n.checked_mul(u64::from(samples)))
            .ok_or("WCS TIFF chunk sample count overflow")?;
        if expected > MAX_SAMPLES || total.checked_add(expected).is_none_or(|n| n > MAX_SAMPLES) {
            return Err("WCS TIFF decoded samples exceed the bounded plan".into());
        }
        let decoded = decoder.read_chunk(index);
        guard.check()?;
        let decoded = decoded.map_err(io_error)?;
        if len(&decoded) as u64 != expected || decoded_type(&decoded) != (formats[0], bits[0]) {
            return Err("WCS TIFF decoded samples differ from the declared layout".into());
        }
        for i in 0..len(&decoded) {
            if i % 65_536 == 0 {
                guard.check()?;
            }
            let value = sample(&decoded, i).ok_or("WCS TIFF sample is missing")?;
            if nil_values[i % samples as usize]
                .iter()
                .any(|n| n.same(value))
                && !nodata.is_some_and(|n| n.same(value))
            {
                return Err(
                    "WCS returned declared nil samples without a matching TIFF NoData tag".into(),
                );
            }
        }
        guard.check()?;
        total += expected;
    }
    if total != plan.width as u64 * plan.height as u64 * samples as u64 {
        return Err("WCS TIFF does not contain every planned sample".into());
    }
    guard.check()
}
pub(super) fn download(
    root: &Path,
    job: &Job,
    path: &Path,
    bytes: u64,
    sha: &str,
    plan: &Plan,
    cancel: &CancellationToken,
) -> Result<()> {
    let guard = Guard {
        cancel,
        deadline: Instant::now() + Duration::from_secs(60),
    };
    guard.check()?;
    let expected = root.join("assets").join(format!("{}.part", job.id));
    let actual = storage::regular_file(path)?;
    if actual != storage::regular_file(&expected)?
        || bytes == 0
        || bytes > crate::MAX_ASSET_BYTES
        || !digest(sha)
    {
        return Err("WCS validation requires the exact managed partial file".into());
    }
    let mut file = File::open(actual).map_err(io_error)?;
    fs2::FileExt::try_lock_shared(&file).map_err(|_| "WCS partial is being modified")?;
    let metadata = file.metadata().map_err(io_error)?;
    if metadata.len() != bytes {
        return Err("WCS partial byte count changed".into());
    }
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut count = 0u64;
    loop {
        guard.check()?;
        let n = file.read(&mut buffer).map_err(io_error)?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count > bytes {
            return Err("WCS partial grew during validation".into());
        }
        hasher.update(&buffer[..n]);
    }
    if count != bytes || format!("{:x}", hasher.finalize()) != sha {
        return Err("WCS partial checksum changed".into());
    }
    file.seek(SeekFrom::Start(0)).map_err(io_error)?;
    tiff_checked(BufReader::new(&mut file), plan, &guard)?;
    if file.metadata().map_err(io_error)?.modified().ok() != metadata.modified().ok() {
        return Err("WCS partial changed during sample validation".into());
    }
    guard.check()
}
