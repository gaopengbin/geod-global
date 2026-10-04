use super::*;
use std::io::{BufReader, Write};
use tempfile::NamedTempFile;
use tiff::encoder::{
    compression::{CompressionAlgorithm, Deflate, DeflateLevel},
    TiffEncoder,
};
use tokio::sync::mpsc::UnboundedSender;

pub(super) struct Output {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    pub samples: RgbOutput,
}

pub(super) struct Window {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub expected: String,
    pub readback: Sha256,
}
pub(super) struct Plane {
    pub file: NamedTempFile,
    pub windows: Vec<Window>,
    pub reference: Reference,
}
pub(super) struct Staged {
    pub planes: [Plane; 3],
    pub quality: Option<quality_mask::Planes>,
    pub coupled_result: Option<ModisMaskResult>,
}

// Retain the checked source's CRS keys and citations as well as its numerical
// grid. Recreating a custom CRS can discard datum/ellipsoid names in GDAL.
#[derive(Clone)]
pub(super) struct Reference {
    keys: Vec<u16>,
    doubles: Option<Vec<f64>>,
    ascii: Option<String>,
}
impl Reference {
    fn read<R: Read + Seek>(decoder: &mut Decoder<R>) -> Result<Self> {
        let keys = decoder
            .get_tag_u16_vec(Tag::GeoKeyDirectoryTag)
            .map_err(io_error)?;
        let doubles = decoder
            .find_tag(Tag::GeoDoubleParamsTag)
            .map_err(io_error)?
            .map(|v| v.into_f64_vec())
            .transpose()
            .map_err(io_error)?;
        let ascii = decoder
            .find_tag(Tag::GeoAsciiParamsTag)
            .map_err(io_error)?
            .map(|v| v.into_string())
            .transpose()
            .map_err(io_error)?;
        if keys.len() > 32768
            || doubles
                .as_ref()
                .is_some_and(|v| v.len() > 8192 || v.iter().any(|x| !x.is_finite()))
            || ascii
                .as_ref()
                .is_some_and(|v| v.len() > 65536 || !v.is_ascii())
        {
            return Err("Scientific RGB source CRS tags exceed their bounds".into());
        }
        Ok(Self {
            keys,
            doubles,
            ascii,
        })
    }
}

pub(super) fn cancelled(token: &CancellationToken) -> Result<()> {
    if token.is_cancelled() {
        Err("Scientific RGB cancelled".into())
    } else {
        Ok(())
    }
}
pub(super) fn staged(root: &Path, id: &str) -> Result<NamedTempFile> {
    let expected = root.join("assets");
    if expected.canonicalize().map_err(io_error)? != expected {
        return Err("Managed RGB asset directory was redirected".into());
    }
    tempfile::Builder::new()
        .prefix(&format!("{id}.rgb-"))
        .suffix(".part")
        .tempfile_in(expected)
        .map_err(io_error)
}

pub(super) fn stage(
    root: &Path,
    id: &str,
    spec: &RgbSpec,
    job: &Job,
    token: &CancellationToken,
    progress: &UnboundedSender<(u64, &'static str)>,
    channel: u64,
) -> Result<Plane> {
    cancelled(token)?;
    let mut source = source_with_deadline(root, job, Instant::now() + Duration::from_secs(180))?;
    if grid(&source) != spec.grid || source.profile != spec.profile {
        return Err("RGB source grid or calibration changed after planning".into());
    }
    let reference = Reference::read(&mut source.decoder)?;
    let mut file = staged(root, id)?;
    file.as_file()
        .set_len(raw_bytes(&spec.grid)? / 3)
        .map_err(io_error)?;
    let [cw, ch, columns] = chunk_grid(&mut source)?;
    let count = columns * source.height.div_ceil(ch);
    let mut windows = Vec::with_capacity(count as usize);
    let mut covered = 0u64;
    for index in 0..count {
        cancelled(token)?;
        let values = read_chunk(&mut source, index)?;
        let (decoded_width, decoded_height) = source.decoder.chunk_data_dimensions(index);
        let x = index % columns * cw;
        let y = index / columns * ch;
        let width = cw.min(source.width - x);
        let height = ch.min(source.height - y);
        if width > decoded_width || height > decoded_height {
            return Err("Invalid RGB chunk window".into());
        }
        let mut hash = Sha256::new();
        for row in 0..height {
            cancelled(token)?;
            let mut bytes = Vec::with_capacity(width as usize * 2);
            for col in 0..width {
                let value = values
                    .get((row * decoded_width + col) as usize)
                    .ok_or("Missing RGB source sample")?;
                bytes.extend_from_slice(&(value as u16).to_le_bytes());
            }
            hash.update(&bytes);
            file.as_file_mut()
                .seek(SeekFrom::Start(
                    ((y + row) as u64 * source.width as u64 + x as u64) * 2,
                ))
                .map_err(io_error)?;
            file.as_file_mut().write_all(&bytes).map_err(io_error)?;
        }
        covered += u64::from(width) * u64::from(height);
        windows.push(Window {
            x,
            y,
            width,
            height,
            expected: format!("{:x}", hash.finalize()),
            readback: Sha256::new(),
        });
        let _ = progress.send((
            channel * 200 + 200 * (index as u64 + 1) / count as u64,
            "Reading original RGB samples",
        ));
    }
    if covered != u64::from(source.width) * u64::from(source.height) {
        return Err("RGB source chunks did not cover the grid".into());
    }
    file.as_file().sync_all().map_err(io_error)?;
    file.as_file_mut()
        .seek(SeekFrom::Start(0))
        .map_err(io_error)?;
    Ok(Plane {
        file,
        windows,
        reference,
    })
}

pub(super) fn metadata(spec: &RgbSpec) -> Result<String> {
    let hash = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(spec).map_err(io_error)?)
    );
    let values = if spec.quality_mask.is_some() {
        "accepted original DN; quality-rejected pixels are NoData"
    } else {
        "original DN"
    };
    let mut xml = format!("<GDALMetadata><Item name=\"GEOD_SCHEMA\">{SCHEMA}</Item><Item name=\"GEOD_SPEC_SHA256\">{hash}</Item><Item name=\"GEOD_SAMPLE_VALUES\">{values}</Item>");
    for (sample, role) in ["red", "green", "blue"].into_iter().enumerate() {
        xml.push_str(&format!("<Item name=\"SCALE\" sample=\"{sample}\" role=\"scale\">{}</Item><Item name=\"OFFSET\" sample=\"{sample}\" role=\"offset\">{}</Item><Item name=\"DESCRIPTION\" sample=\"{sample}\" role=\"description\">{role}</Item><Item name=\"UNITTYPE\" sample=\"{sample}\" role=\"unittype\">1</Item>", spec.profile.scale, spec.profile.offset));
    }
    xml.push_str("</GDALMetadata>");
    Ok(xml)
}

fn tags<W: Write + Seek, K: tiff::encoder::TiffKind>(
    image: &mut tiff::encoder::DirectoryEncoder<'_, W, K>,
    spec: &RgbSpec,
    reference: &Reference,
) -> Result<()> {
    let g = &spec.grid;
    for (tag, value) in [
        (Tag::ImageWidth, g.width),
        (Tag::ImageLength, g.height),
        (Tag::RowsPerStrip, 64),
    ] {
        image.write_tag(tag, value).map_err(io_error)?;
    }
    for (tag, value) in [
        (Tag::SamplesPerPixel, 3u16),
        (Tag::Compression, 8),
        (Tag::PhotometricInterpretation, 2),
        (Tag::PlanarConfiguration, 1),
        (Tag::Orientation, 1),
    ] {
        image.write_tag(tag, value).map_err(io_error)?;
    }
    image
        .write_tag(Tag::BitsPerSample, &[16u16; 3][..])
        .map_err(io_error)?;
    image
        .write_tag(
            Tag::SampleFormat,
            &[if spec.profile.signed { 2u16 } else { 1 }; 3][..],
        )
        .map_err(io_error)?;
    // Coupled output uses the pinned project area grid. Originals may declare
    // Point centres; preserve the CRS keys but write the output interpretation.
    let mut keys = reference.keys.clone();
    let raster_type = keys
        .get_mut(4..)
        .ok_or("Invalid RGB CRS key directory")?
        .chunks_exact_mut(4)
        .find(|key| key[0] == 1025)
        .ok_or("Missing RGB pixel interpretation key")?;
    if raster_type[1] != 0 || raster_type[2] != 1 {
        return Err("Invalid RGB pixel interpretation key".into());
    }
    raster_type[3] = if g.pixel_interpretation == "PixelIsPoint" {
        2
    } else {
        1
    };
    image
        .write_tag(Tag::GeoKeyDirectoryTag, &keys[..])
        .map_err(io_error)?;
    if let Some(doubles) = &reference.doubles {
        image
            .write_tag(Tag::GeoDoubleParamsTag, &doubles[..])
            .map_err(io_error)?;
    }
    if let Some(ascii) = &reference.ascii {
        image
            .write_tag(Tag::GeoAsciiParamsTag, ascii.trim_end_matches('\0'))
            .map_err(io_error)?;
    }
    let point = if g.pixel_interpretation == "PixelIsPoint" {
        0.5
    } else {
        0.0
    };
    image
        .write_tag(
            Tag::ModelPixelScaleTag,
            &[g.pixel_size[0], g.pixel_size[1], 0.0][..],
        )
        .map_err(io_error)?;
    image
        .write_tag(
            Tag::ModelTiepointTag,
            &[
                0.0,
                0.0,
                0.0,
                g.bounds[0] + point * g.pixel_size[0],
                g.bounds[3] - point * g.pixel_size[1],
                0.0,
            ][..],
        )
        .map_err(io_error)?;
    image
        .write_tag(Tag::GdalNodata, spec.profile.nodata.to_string().as_str())
        .map_err(io_error)?;
    image
        .write_tag(Tag::Unknown(42112), metadata(spec)?.as_str())
        .map_err(io_error)?;
    Ok(())
}

pub(super) fn read_bands(
    planes: &mut [Plane; 3],
    y: u32,
    height: u32,
    width: usize,
) -> Result<[Vec<u8>; 3]> {
    let mut bands: [Vec<u8>; 3] = std::array::from_fn(|_| vec![0; width * height as usize * 2]);
    for (channel, plane) in planes.iter_mut().enumerate() {
        plane
            .file
            .as_file_mut()
            .read_exact(&mut bands[channel])
            .map_err(io_error)?;
        for window in &mut plane.windows {
            for row in y.max(window.y)..(y + height).min(window.y + window.height) {
                let offset = ((row - y) as usize * width + window.x as usize) * 2;
                window
                    .readback
                    .update(&bands[channel][offset..offset + window.width as usize * 2]);
            }
        }
    }
    Ok(bands)
}
pub(super) fn verify_planes(planes: &[Plane; 3]) -> Result<()> {
    for plane in planes {
        for window in &plane.windows {
            if format!("{:x}", window.readback.clone().finalize()) != window.expected {
                return Err(
                    "Staged RGB samples changed after original checksum verification".into(),
                );
            }
        }
    }
    Ok(())
}

fn encode(
    output: &mut File,
    spec: &RgbSpec,
    planes: &mut [Plane; 3],
    mut quality: Option<&mut quality_mask::Planes>,
    token: &CancellationToken,
    progress: &UnboundedSender<(u64, &'static str)>,
) -> Result<(RgbOutput, String)> {
    let mut encoder = TiffEncoder::new(output).map_err(io_error)?;
    let mut image = encoder.image_directory().map_err(io_error)?;
    tags(&mut image, spec, &planes[0].reference)?;
    let mut offsets = Vec::<u32>::new();
    let mut lengths = Vec::<u32>::new();
    let mut hashes: [Sha256; 3] = std::array::from_fn(|_| Sha256::new());
    let mut pixel_hash = Sha256::new();
    let mut valid = [0u64; 3];
    let mut common = 0u64;
    let width = spec.grid.width as usize;
    for y in (0..spec.grid.height).step_by(64) {
        cancelled(token)?;
        let height = 64.min(spec.grid.height - y);
        let mut bands = read_bands(planes, y, height, width)?;
        if let Some(mask) = quality.as_mut() {
            mask.apply(y, height, &mut bands, spec.profile.nodata)?;
        }
        for (hash, band) in hashes.iter_mut().zip(&bands) {
            hash.update(band);
        }
        let mut interleaved = Vec::with_capacity(width * height as usize * 6);
        for index in 0..width * height as usize {
            let mut joint = true;
            for channel in 0..3 {
                let value =
                    u16::from_le_bytes([bands[channel][index * 2], bands[channel][index * 2 + 1]]);
                let dn = if spec.profile.signed {
                    i32::from(value as i16)
                } else {
                    i32::from(value)
                };
                if dn != spec.profile.nodata {
                    valid[channel] += 1;
                } else {
                    joint = false;
                }
                interleaved.extend_from_slice(&value.to_ne_bytes());
            }
            common += u64::from(joint);
        }
        // Store a sequence of strip digests for bounded validation without holding all samples.
        if cfg!(target_endian = "little") {
            pixel_hash.update(Sha256::digest(&interleaved));
        } else {
            let canonical: Vec<u8> = interleaved
                .chunks_exact(2)
                .flat_map(|v| u16::from_ne_bytes([v[0], v[1]]).to_le_bytes())
                .collect();
            pixel_hash.update(Sha256::digest(canonical));
        }
        let mut compressed = Vec::new();
        Deflate::with_level(DeflateLevel::Balanced)
            .write_to(&mut compressed, &interleaved)
            .map_err(io_error)?;
        cancelled(token)?;
        offsets.push(
            u32::try_from(image.write_data(&compressed[..]).map_err(io_error)?)
                .map_err(io_error)?,
        );
        lengths.push(u32::try_from(compressed.len()).map_err(io_error)?);
        let _ = progress.send((
            600 + 250 * u64::from(y + height) / u64::from(spec.grid.height),
            "Writing scientific RGB GeoTIFF",
        ));
    }
    verify_planes(planes)?;
    if let Some(mask) = &quality {
        mask.verify()?;
    }
    image
        .write_tag(Tag::StripOffsets, &offsets[..])
        .map_err(io_error)?;
    image
        .write_tag(Tag::StripByteCounts, &lengths[..])
        .map_err(io_error)?;
    image.finish().map_err(io_error)?;
    Ok((
        RgbOutput {
            samples_sha256: hashes.map(|h| format!("{:x}", h.finalize())),
            channel_valid_pixels: valid,
            common_valid_pixels: common,
            quality_mask: quality.map(|mask| mask.counts.clone()),
        },
        format!("{:x}", pixel_hash.finalize()),
    ))
}

fn validate(path: &Path, spec: &RgbSpec, expected: &str, token: &CancellationToken) -> Result<()> {
    let mut limits = Limits::default();
    limits.decoding_buffer_size = 32 * 1024 * 1024;
    limits.intermediate_buffer_size = 32 * 1024 * 1024;
    let mut decoder = Decoder::new(BufReader::new(File::open(path).map_err(io_error)?))
        .map_err(io_error)?
        .with_limits(limits);
    reader::check_header(&mut decoder, spec)?;
    if decoder.get_chunk_type() != ChunkType::Strip
        || decoder.chunk_dimensions() != (spec.grid.width, 64)
    {
        return Err("Scientific RGB output strips are invalid".into());
    }
    let mut hash = Sha256::new();
    for index in 0..spec.grid.height.div_ceil(64) {
        cancelled(token)?;
        let values = reader::chunk(&mut decoder, &spec.profile, index)?;
        let canonical: Vec<u8> = match values {
            Samples::Unsigned(v) => v.into_iter().flat_map(u16::to_le_bytes).collect(),
            Samples::Signed(v) => v.into_iter().flat_map(i16::to_le_bytes).collect(),
            Samples::Signed8(_) => {
                return Err("Scientific RGB requires reviewed 16-bit reflectance channels".into())
            }
        };
        hash.update(Sha256::digest(canonical));
    }
    if format!("{:x}", hash.finalize()) != expected {
        return Err("Scientific RGB output differs from verified processed samples".into());
    }
    Ok(())
}

pub(super) fn write(
    root: &Path,
    id: &str,
    spec: &RgbSpec,
    jobs: &[Job; 3],
    quality_jobs: Option<&[Job; 2]>,
    token: &CancellationToken,
    progress: &UnboundedSender<(u64, &'static str)>,
) -> Result<Output> {
    validate_spec(spec)?;
    if !canonical_id(id) {
        return Err("Invalid RGB output identifier".into());
    }
    if fs2::available_space(root.join("assets")).map_err(io_error)?
        < disk_bytes(&spec.grid, spec.quality_mask.is_some())?
    {
        return Err("Insufficient workspace disk space for scientific RGB".into());
    }
    let mut planes = Vec::with_capacity(3);
    for (channel, job) in jobs.iter().enumerate() {
        planes.push(stage(root, id, spec, job, token, progress, channel as u64)?);
    }
    let planes: [Plane; 3] = planes
        .try_into()
        .map_err(|_| "RGB requires three source planes")?;
    let quality = match (&spec.quality_mask, quality_jobs) {
        (None, None) => None,
        (Some(_), Some(jobs)) => {
            let _ = progress.send((600, "Reading pinned product-specific quality flags"));
            Some(quality_mask::Planes::stage(root, id, spec, jobs, token)?)
        }
        _ => return Err("Quality sources differ from the RGB specification".into()),
    };
    finish(
        root,
        id,
        spec,
        Staged {
            planes,
            quality,
            coupled_result: None,
        },
        token,
        progress,
    )
}

pub(super) fn finish(
    root: &Path,
    id: &str,
    spec: &RgbSpec,
    mut inputs: Staged,
    token: &CancellationToken,
    progress: &UnboundedSender<(u64, &'static str)>,
) -> Result<Output> {
    let mut output = staged(root, id)?;
    let (mut samples, pixel_hash) = encode(
        output.as_file_mut(),
        spec,
        &mut inputs.planes,
        inputs.quality.as_mut(),
        token,
        progress,
    )?;
    if let Some(result) = inputs.coupled_result {
        samples.quality_mask = Some(result);
    }
    coupled::validate_result(spec, &samples)?;
    output.as_file().sync_all().map_err(io_error)?;
    let _ = progress.send((850, "Checking every scientific RGB output sample"));
    validate(output.path(), spec, &pixel_hash, token)?;
    let bytes = output.as_file().metadata().map_err(io_error)?.len();
    if bytes == 0 || bytes > MAX_ASSET_BYTES {
        return Err("Scientific RGB output exceeds 512 MiB".into());
    }
    output
        .as_file_mut()
        .seek(SeekFrom::Start(0))
        .map_err(io_error)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        cancelled(token)?;
        let n = output.as_file_mut().read(&mut buffer).map_err(io_error)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    let sha256 = format!("{:x}", hash.finalize());
    cancelled(token)?;
    let path = root.join("assets").join(format!("{id}.tif"));
    output.persist_noclobber(&path).map_err(io_error)?;
    Ok(Output {
        path: path.to_string_lossy().into_owned(),
        bytes,
        sha256,
        samples,
    })
}
