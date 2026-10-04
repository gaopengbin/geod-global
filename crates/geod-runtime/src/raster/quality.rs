//! MOD/MYD09A1 v061 bit fields. Original unsigned samples are never calibrated
//! or treated as reflectance. Interpretation follows NASA's C61 guide, tables
//! 10 and 13; unknown band-quality codes remain unknown, not 'good'.
use super::*;
use crate::{io_error, providers};
use std::collections::BTreeMap;
use tiff::decoder::ChunkType;

pub(crate) const DEFINITION: &str =
    "https://landweb.modaps.eosdis.nasa.gov/data/userguide/MOD09_User_Guide_V61.pdf";

#[derive(Clone, Debug, PartialEq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Profile {
    pub product: String,
    pub band: String,
    pub layer: String,
    pub bits: u8,
    pub nodata: u32,
    pub pixel_interpretation: String,
    pub definition: String,
}
pub(crate) fn profile(key: &str) -> Result<Profile> {
    let (layer, bits, nodata) = layer(key)?;
    Ok(Profile {
        product: "modis-09a1-v061".into(),
        band: key.into(),
        layer: layer.into(),
        bits,
        nodata,
        pixel_interpretation: "PixelIsArea".into(),
        definition: DEFINITION.into(),
    })
}
pub(crate) fn metadata(p: &Profile) -> String {
    format!("<GDALMetadata><Item name=\"PRODUCT\">{}</Item><Item name=\"QUALITY_LAYER\" sample=\"0\">{}</Item><Item name=\"FLAG_BITS\" sample=\"0\">{}</Item><Item name=\"DEFINITION\">{}</Item></GDALMetadata>", p.product, p.layer, p.bits, p.definition)
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityDisplay {
    pub product: String,
    pub band: String,
    pub layer: String,
    pub pixel_interpretation: String,
    pub display_field: String,
    pub sample_count: u64,
    pub valid_sample_count: u64,
    pub counts_full_resolution: bool,
    pub definition: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<super::landsat_quality::Statistics>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityField {
    pub name: String,
    pub start_bit: u8,
    pub end_bit: u8,
    pub value: u32,
    pub label: String,
    pub defined: bool,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityPixel {
    pub layer: String,
    pub binary: String,
    pub hex: String,
    pub fields: Vec<QualityField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub covered: Option<bool>,
}
fn layer(key: &str) -> Result<(&'static str, u8, u32)> {
    match key {
        "modis_qc" => Ok(("sur_refl_qc_500m", 32, u32::MAX)),
        "modis_state" => Ok(("sur_refl_state_500m", 16, u16::MAX.into())),
        _ => Err("Unsupported MODIS quality layer".into()),
    }
}
pub(crate) fn validate_job(job: &Job) -> Result<()> {
    let expected = profile(&job.asset_key)?;
    let url = providers::asset_url(&job.href)?;
    if job.status != JobStatus::Succeeded
        || extension(&job.media_type)? != "tif"
        || url.host_str() != Some(providers::modis::HOST)
        || providers::modis::item_from_path(url.path(), &job.asset_key).is_none()
        || uuid::Uuid::parse_str(&job.id)
            .ok()
            .map(|id| id.to_string())
            .as_deref()
            != Some(&job.id)
    {
        return Err(
            "Quality inspection requires a completed managed MODIS v061 quality COG".into(),
        );
    }
    if job.kind == "raster_mosaic" {
        crate::mosaic::validate_stored_mosaic(job)?;
        let spec = job.mosaic.as_ref().unwrap();
        let plan = job
            .mosaic_output
            .as_ref()
            .ok_or("Quality result grid is missing")?;
        let count = u64::from(plan.width) * u64::from(plan.height);
        if job.item_id != format!("project:{}", spec.project_id)
            || plan.quality.as_ref() != Some(&expected)
            || plan.band_count != 1
            || plan.crs != providers::modis::CRS
            || plan.source_count != spec.sources.len()
            || count == 0
            || plan.covered_pixels == 0
            || plan.covered_pixels > count
            || plan.masked_pixels > count
            || plan.bounds.iter().any(|v| !v.is_finite())
        {
            return Err("Quality result differs from its pinned unsigned profile and grid".into());
        }
    } else if job.kind != "download"
        || !providers::modis::matches(url.path(), &job.item_id, &job.asset_key)
    {
        return Err("Quality source identity differs from the original product".into());
    }
    Ok(())
}
struct Source {
    decoder: Decoder<reflectance::Snapshot>,
    header: reflectance::Header,
    bits: u8,
    nodata: u32,
    deadline: Instant,
}
fn source(root: &Path, job: &Job) -> Result<Source> {
    validate_job(job)?;
    let (_, bits, nodata) = layer(&job.asset_key)?;
    let deadline = Instant::now() + Duration::from_secs(55);
    let mut decoder = reflectance::verified_decoder(root, job, deadline)?;
    let header = validate_header(
        &mut decoder,
        &job.asset_key,
        &job.item_id,
        job.mosaic_output.as_ref(),
    )?;
    Ok(Source {
        decoder,
        header,
        bits,
        nodata,
        deadline,
    })
}
pub(crate) fn validate_header<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    key: &str,
    item_id: &str,
    plan: Option<&crate::mosaic::MosaicPlan>,
) -> Result<reflectance::Header> {
    let expected = profile(key)?;
    let bits = expected.bits;
    let nodata = expected.nodata;
    let (width, height) = decoder.dimensions().map_err(io_error)?;
    if decoder.colortype().map_err(io_error)? != ColorType::Gray(bits)
        || decoder
            .find_tag_unsigned::<u16>(Tag::SamplesPerPixel)
            .map_err(io_error)?
            .unwrap_or(1)
            != 1
        || decoder
            .find_tag_unsigned_vec::<u16>(Tag::SampleFormat)
            .map_err(io_error)?
            .unwrap_or(vec![1])
            != [1]
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
            "MODIS quality samples must retain their original unsigned bit-field type".into(),
        );
    }
    let keys = decoder
        .get_tag_u16_vec(Tag::GeoKeyDirectoryTag)
        .map_err(io_error)?;
    let crs = reflectance::modis::crs(decoder, &keys)?;
    let get = |decoder: &mut Decoder<R>, tag| -> Result<Option<Vec<f64>>> {
        decoder
            .find_tag(tag)
            .map_err(io_error)?
            .map(|v| v.into_f64_vec())
            .transpose()
            .map_err(io_error)
    };
    let transform = get(decoder, Tag::ModelTransformationTag)?;
    let scale = get(decoder, Tag::ModelPixelScaleTag)?;
    let tiepoint = get(decoder, Tag::ModelTiepointTag)?;
    let (bounds, pixel_size) = georeference(
        width,
        height,
        transform.as_deref(),
        scale.as_deref(),
        tiepoint.as_deref(),
    )?;
    let header = reflectance::Header {
        width,
        height,
        crs,
        bounds,
        pixel_size,
        pixel_is_point: false,
    };
    if pixel_size
        .iter()
        .any(|value| (*value - providers::modis::PIXEL).abs() > 1e-6)
        || decoder
            .get_tag(Tag::GdalNodata)
            .map_err(io_error)?
            .into_string()
            .map_err(io_error)?
            .trim_matches('\0')
            .trim()
            .parse::<u32>()
            .map_err(io_error)?
            != nodata
    {
        return Err("MODIS quality grid or fill value differs from the reviewed product".into());
    }
    if let Some(plan) = plan {
        if plan.quality.as_ref() != Some(&expected)
            || plan.width != width
            || plan.height != height
            || plan.band_count != 1
            || plan.crs != header.crs
            || plan.bounds != bounds
            || plan.pixel_size != pixel_size
            || plan.calibration.is_some()
            || plan.elevation.is_some()
            || plan.aerial.is_some()
            || plan.radar.is_some()
            || decoder
                .get_tag(Tag::Unknown(42112))
                .map_err(io_error)?
                .into_string()
                .map_err(io_error)?
                .trim_matches('\0')
                != metadata(&expected)
        {
            return Err(
                "Quality result header differs from its recorded grid or bit-field definition"
                    .into(),
            );
        }
    } else {
        reflectance::modis::grid(&header, item_id)?;
    }
    Ok(header)
}
fn chunks(source: &mut Source) -> Result<[u32; 4]> {
    let (cw, ch) = source.decoder.chunk_dimensions();
    if cw == 0 || ch == 0 {
        return Err("Quality COG has an invalid chunk grid".into());
    }
    let columns = source.header.width.div_ceil(cw);
    let count = match source.decoder.get_chunk_type() {
        ChunkType::Tile => source.decoder.tile_count().map_err(io_error)?,
        ChunkType::Strip => source.decoder.strip_count().map_err(io_error)?,
    };
    if columns.checked_mul(source.header.height.div_ceil(ch)) != Some(count) {
        return Err("Quality COG chunk count differs from the original grid".into());
    }
    Ok([cw, ch, columns, count])
}
fn read_chunk(source: &mut Source, index: u32) -> Result<(Vec<u32>, u32, u32)> {
    check_time(source.deadline)?;
    let data = match source.decoder.read_chunk(index).map_err(io_error)? {
        DecodingResult::U16(values) if source.bits == 16 => {
            values.into_iter().map(u32::from).collect()
        }
        DecodingResult::U32(values) if source.bits == 32 => values,
        _ => return Err("Decoded quality samples have a different unsigned type".into()),
    };
    let (width, height) = source.decoder.chunk_data_dimensions(index);
    if data.len() != width as usize * height as usize {
        return Err("Quality COG has an invalid decoded sample count".into());
    }
    Ok((data, width, height))
}

pub(crate) fn source_header(root: &Path, job: &Job) -> Result<reflectance::Header> {
    Ok(source(root, job)?.header)
}

/// Read checksum-verified unsigned flags in bounded source chunks. The caller
/// checks the header before any samples are written; no alignment is inferred.
pub(crate) fn visit_chunks(
    root: &Path,
    job: &Job,
    token: &tokio_util::sync::CancellationToken,
    check: impl FnOnce(&reflectance::Header) -> Result<()>,
    mut visit: impl FnMut([u32; 5], &[u32]) -> Result<()>,
) -> Result<()> {
    let mut source = source(root, job)?;
    check(&source.header)?;
    let [cw, ch, columns, count] = chunks(&mut source)?;
    for index in 0..count {
        if token.is_cancelled() {
            return Err("Quality-mask processing cancelled".into());
        }
        let (values, stride, decoded_height) = read_chunk(&mut source, index)?;
        let x = index % columns * cw;
        let y = index / columns * ch;
        let width = cw.min(source.header.width - x);
        let height = ch.min(source.header.height - y);
        if width > stride || height > decoded_height {
            return Err("Invalid quality-mask chunk window".into());
        }
        visit([x, y, width, height, stride], &values)?;
    }
    Ok(())
}
fn palette(key: &str) -> [(&'static str, [u8; 3]); 4] {
    if key == "modis_qc" {
        [
            ("Ideal MODLAND quality", [37, 99, 235]),
            ("Reduced MODLAND quality", [234, 179, 8]),
            ("Not produced: cloud effects", [239, 68, 68]),
            ("Not produced: other reasons", [107, 114, 128]),
        ]
    } else {
        [
            ("Clear cloud flag", [37, 99, 235]),
            ("Cloudy cloud flag", [226, 232, 240]),
            ("Mixed cloud flag", [234, 179, 8]),
            ("Cloud flag unset; product assumes clear", [107, 114, 128]),
        ]
    }
}
pub(crate) fn inspect(root: &Path, job: &Job, edge: u32) -> Result<RasterInspection> {
    if edge == 0 || edge > PREVIEW_EDGE {
        return Err("Invalid quality preview size".into());
    }
    let mut source = source(root, job)?;
    let longest = source.header.width.max(source.header.height);
    let pw = (u64::from(source.header.width) * u64::from(edge.min(longest)) / u64::from(longest))
        .max(1) as u32;
    let ph = (u64::from(source.header.height) * u64::from(edge.min(longest)) / u64::from(longest))
        .max(1) as u32;
    let [cw, ch, columns, count] = chunks(&mut source)?;
    let mut targets: BTreeMap<u32, Vec<(usize, u32, u32)>> = BTreeMap::new();
    for y in 0..ph {
        for x in 0..pw {
            let sx = u64::from(x) * u64::from(source.header.width) / u64::from(pw);
            let sy = u64::from(y) * u64::from(source.header.height) / u64::from(ph);
            targets
                .entry(sy as u32 / ch * columns + sx as u32 / cw)
                .or_default()
                .push(((y * pw + x) as usize, sx as u32 % cw, sy as u32 % ch));
        }
    }
    let colors = palette(&job.asset_key);
    let mut rgba = vec![0; pw as usize * ph as usize * 4];
    let mut counts = [0u64; 4];
    for index in 0..count {
        let (data, width, height) = read_chunk(&mut source, index)?;
        let start_x = index % columns * cw;
        let start_y = index / columns * ch;
        let valid_w = width.min(source.header.width - start_x);
        let valid_h = height.min(source.header.height - start_y);
        for row in 0..valid_h {
            check_time(source.deadline)?;
            for col in 0..valid_w {
                let value = data[(row * width + col) as usize];
                if value != source.nodata {
                    counts[(value & 3) as usize] += 1;
                }
            }
        }
        if let Some(points) = targets.remove(&index) {
            for (target, x, y) in points {
                let value = *data
                    .get((y * width + x) as usize)
                    .ok_or("Invalid quality preview sample")?;
                if value != source.nodata {
                    rgba[target * 4..target * 4 + 3]
                        .copy_from_slice(&colors[(value & 3) as usize].1);
                    rgba[target * 4 + 3] = 255;
                }
            }
        }
    }
    if !targets.is_empty() {
        return Err("Quality preview was not fully decoded".into());
    }
    if job
        .mosaic_output
        .as_ref()
        .is_some_and(|plan| plan.covered_pixels != counts.iter().sum::<u64>())
    {
        return Err(
            "Quality result coverage count differs from the decoded original bit fields".into(),
        );
    }
    let mut bytes = Vec::new();
    let mut png = png::Encoder::new(&mut bytes, pw, ph);
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.write_header()
        .map_err(io_error)?
        .write_image_data(&rgba)
        .map_err(io_error)?;
    let (name, _, _) = layer(&job.asset_key)?;
    let header = source.header;
    Ok(RasterInspection {
        science: None,
        vegetation: None,
        width: header.width,
        height: header.height,
        band_count: 1,
        data_type: format!("UInt{}", source.bits),
        crs: header.crs,
        bounds: header.bounds,
        pixel_size: header.pixel_size,
        nodata: Some(f64::from(source.nodata)),
        preview_data_url: format!("data:image/png;base64,{}", STANDARD.encode(bytes)),
        preview_width: pw,
        preview_height: ph,
        classes: colors
            .into_iter()
            .enumerate()
            .map(|(index, (label, color))| RasterClass {
                value: index as u8,
                label: label.into(),
                color: format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2]),
                count: counts[index],
            })
            .collect(),
        sha256: job.sha256.clone().ok_or("Missing quality checksum")?,
        reflectance: None,
        elevation: None,
        aerial: None,
        radar: None,
        quality: Some(QualityDisplay {
            product: "modis-09a1-v061".into(),
            band: job.asset_key.clone(),
            layer: name.into(),
            pixel_interpretation: "PixelIsArea".into(),
            display_field: if source.bits == 32 {
                "MODLAND quality"
            } else {
                "Cloud state"
            }
            .into(),
            sample_count: u64::from(header.width) * u64::from(header.height),
            valid_sample_count: counts.iter().sum(),
            counts_full_resolution: true,
            definition: DEFINITION.into(),
            flags: None,
        }),
    })
}
fn field(raw: u32, name: &str, start: u8, end: u8, labels: &[&str]) -> QualityField {
    let value = (raw >> start) & ((1u32 << (end - start + 1)) - 1);
    QualityField {
        name: name.into(),
        start_bit: start,
        end_bit: end,
        value,
        label: labels
            .get(value as usize)
            .copied()
            .filter(|v| !v.is_empty())
            .unwrap_or("Undocumented quality code")
            .into(),
        defined: labels.get(value as usize).is_some_and(|v| !v.is_empty()),
    }
}
fn decode(key: &str, value: u32) -> Result<QualityPixel> {
    let (name, bits, fill) = layer(key)?;
    let mut fields = Vec::new();
    if value != fill {
        let yes_no = ["No", "Yes"];
        if bits == 32 {
            fields.push(field(
                value,
                "MODLAND quality",
                0,
                1,
                &[
                    "Ideal",
                    "Reduced quality",
                    "Cloud prevented production",
                    "Other production failure",
                ],
            ));
            let band_labels = [
                "Best quality",
                "",
                "",
                "",
                "",
                "",
                "",
                "Detector noise",
                "Detector interpolated",
                "Solar zenith at least 86°",
                "Solar zenith from 85° to 86°",
                "Missing input",
                "Atmospheric input substituted",
                "Correction clipped",
                "Faulty Level-1 input",
                "Ocean or cloud: not processed",
            ];
            for band in 0..7 {
                fields.push(field(
                    value,
                    &format!("Band {} quality", band + 1),
                    2 + band * 4,
                    5 + band * 4,
                    &band_labels,
                ));
            }
            fields.push(field(value, "Atmospheric correction", 30, 30, &yes_no));
            fields.push(field(value, "Adjacency correction", 31, 31, &yes_no));
        } else {
            fields.push(field(
                value,
                "Cloud state",
                0,
                1,
                &["Clear", "Cloudy", "Mixed", "Unset; product assumes clear"],
            ));
            fields.push(field(value, "Cloud shadow", 2, 2, &yes_no));
            fields.push(field(
                value,
                "Land / water",
                3,
                5,
                &[
                    "Shallow sea",
                    "Land",
                    "Coast or lake shore",
                    "Shallow inland water",
                    "Temporary water",
                    "Deep inland water",
                    "Shelf or moderate ocean",
                    "Deep ocean",
                ],
            ));
            fields.push(field(
                value,
                "Aerosol correction uncertainty",
                6,
                7,
                &["Climatology used", "Low", "Medium", "High"],
            ));
            fields.push(field(
                value,
                "Cirrus",
                8,
                9,
                &["None", "Low", "Medium", "High"],
            ));
            for (bit, name) in [
                (10, "Internal cloud"),
                (11, "Internal fire"),
                (12, "MOD35 snow / ice"),
                (13, "Adjacent to cloud"),
                (14, "Salt pan"),
                (15, "Internal snow"),
            ] {
                fields.push(field(value, name, bit, bit, &yes_no));
            }
        }
    }
    Ok(QualityPixel {
        layer: name.into(),
        binary: format!("{value:0width$b}", width = bits as usize),
        hex: format!("0x{value:0width$X}", width = bits as usize / 4),
        fields,
        covered: None,
    })
}
pub(crate) fn sample(root: &Path, job: &Job, x: f64, y: f64) -> Result<RasterPixel> {
    let mut source = source(root, job)?;
    let [left, bottom, right, top] = source.header.bounds;
    if !x.is_finite() || !y.is_finite() || x < left || x >= right || y <= bottom || y > top {
        return Err("The coordinate is outside the quality raster pixel grid".into());
    }
    let [dx, dy] = source.header.pixel_size;
    let col = ((x - left) / dx).floor() as u32;
    let row = ((top - y) / dy).floor() as u32;
    if col >= source.header.width || row >= source.header.height {
        return Err("The coordinate is outside the quality raster pixel grid".into());
    }
    let [cw, ch, columns, _] = chunks(&mut source)?;
    let (data, width, _) = read_chunk(&mut source, row / ch * columns + col / cw)?;
    let value = *data
        .get((row % ch * width + col % cw) as usize)
        .ok_or("Invalid quality sample")?;
    let (label, color) = palette(&job.asset_key)[(value & 3) as usize];
    let no_data = value == source.nodata;
    Ok(RasterPixel {
        science: None,
        index_value: None,
        job_id: job.id.clone(),
        sha256: job.sha256.clone().ok_or("Missing quality checksum")?,
        crs: source.header.crs,
        coordinate: [x, y],
        pixel: [col, row],
        center: [
            left + (f64::from(col) + 0.5) * dx,
            top - (f64::from(row) + 0.5) * dy,
        ],
        value: f64::from(value),
        values: None,
        near_infrared: None,
        reflectance: None,
        decibels: None,
        quality: Some(decode(&job.asset_key, value)?),
        label: if no_data { "NoData" } else { label }.into(),
        color: if no_data {
            "#000000".into()
        } else {
            format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2])
        },
        is_no_data: no_data,
    })
}

#[cfg(test)]
mod tests;
