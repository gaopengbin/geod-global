//! Original Landsat 8/9 C2 QA. Counts and pixel queries always use the base
//! image, never an overview. QA_RADSAT zero is a valid flag value, not NoData.
use super::{
    georeference,
    quality::{QualityDisplay, QualityField, QualityPixel},
    reflectance, validate_geokeys, RasterClass, RasterInspection, RasterPixel, PREVIEW_EDGE,
};
use crate::{io_error, providers, Job, JobStatus, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    io::{Read, Seek},
    path::Path,
    time::{Duration, Instant},
};
use tiff::{
    decoder::{ChunkType, Decoder, DecodingResult},
    tags::Tag,
    ColorType,
};
mod processing;
pub(crate) use processing::{processing_profile, DERIVED_COVERAGE, MOSAIC_POLICY};
pub use processing::{CoverageStatistics, ProcessingProfile};

pub(crate) const KEYS: &[&str] = &["qa_pixel", "qa_radsat"];
pub(crate) const DEFINITION: &str =
    "https://www.usgs.gov/landsat-missions/landsat-collection-2-quality-assessment-bands";
pub(crate) const FILL_RULE: &str =
    "QA_PIXEL bit 0 marks fill; all other unsigned values are retained";
pub(crate) const RADSAT_RULE: &str =
    "QA_RADSAT has no fill flag; zero means no saturation or terrain occlusion flags";
pub(crate) const RADSAT_COVERAGE: &str =
    "QA_RADSAT alone cannot determine image coverage; use the matching QA_PIXEL fill flag";
pub(crate) const PIXEL_COVERAGE: &str =
    "Image coverage follows QA_PIXEL bit 0, independently of the TIFF NoData tag";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Histogram {
    pub name: String,
    pub start_bit: u8,
    pub end_bit: u8,
    pub counts: Vec<u64>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Statistics {
    #[serde(rename = "sourceNoData")]
    pub source_nodata: Option<u16>,
    pub fill_rule: String,
    pub coverage: String,
    /// Includes every source sample, including flagged fill and reserved codes.
    pub fields: Vec<Histogram>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage_mask: Option<CoverageStatistics>,
}
type Field = (&'static str, u8, u8, &'static [&'static str]);
const PIXEL_FIELDS: &[Field] = &[
    ("Fill", 0, 0, &["Image data", "Fill data"]),
    (
        "Dilated cloud",
        1,
        1,
        &["Not dilated or no cloud", "Dilated cloud"],
    ),
    (
        "High-confidence cirrus",
        2,
        2,
        &["Cirrus confidence not high", "High-confidence cirrus"],
    ),
    (
        "High-confidence cloud",
        3,
        3,
        &["Cloud confidence not high", "High-confidence cloud"],
    ),
    (
        "High-confidence cloud shadow",
        4,
        4,
        &["Shadow confidence not high", "High-confidence cloud shadow"],
    ),
    (
        "High-confidence snow / ice",
        5,
        5,
        &["Snow confidence not high", "High-confidence snow / ice"],
    ),
    (
        "Clear cloud flag",
        6,
        6,
        &["Clear flag unset", "Cloud and dilated-cloud flags unset"],
    ),
    ("Water", 7, 7, &["Land or cloud", "Water"]),
    (
        "Cloud confidence",
        8,
        9,
        &["Confidence unset", "Low", "Medium", "High"],
    ),
    (
        "Cloud shadow confidence",
        10,
        11,
        &["Confidence unset", "Low", "Reserved code", "High"],
    ),
    (
        "Snow / ice confidence",
        12,
        13,
        &["Confidence unset", "Low", "Reserved code", "High"],
    ),
    (
        "Cirrus confidence",
        14,
        15,
        &["Confidence unset", "Low", "Reserved code", "High"],
    ),
];
const RADSAT_FIELDS: &[Field] = &[
    ("Band 1 saturation", 0, 0, &["Not saturated", "Saturated"]),
    ("Band 2 saturation", 1, 1, &["Not saturated", "Saturated"]),
    ("Band 3 saturation", 2, 2, &["Not saturated", "Saturated"]),
    ("Band 4 saturation", 3, 3, &["Not saturated", "Saturated"]),
    ("Band 5 saturation", 4, 4, &["Not saturated", "Saturated"]),
    ("Band 6 saturation", 5, 5, &["Not saturated", "Saturated"]),
    ("Band 7 saturation", 6, 6, &["Not saturated", "Saturated"]),
    ("Unused bit 7", 7, 7, &["Unset", "Unused bit set"]),
    ("Band 9 saturation", 8, 8, &["Not saturated", "Saturated"]),
    ("Unused bit 9", 9, 9, &["Unset", "Unused bit set"]),
    ("Unused bit 10", 10, 10, &["Unset", "Unused bit set"]),
    (
        "Terrain occlusion",
        11,
        11,
        &["Not terrain-occluded", "Terrain occlusion"],
    ),
    ("Unused bits 12–15", 12, 15, &["Unset"]),
];
fn fields(key: &str) -> Result<&'static [Field]> {
    match key {
        "qa_pixel" => Ok(PIXEL_FIELDS),
        "qa_radsat" => Ok(RADSAT_FIELDS),
        _ => Err("Unsupported Landsat quality layer".into()),
    }
}
pub(crate) fn validate_job(job: &Job) -> Result<()> {
    fields(&job.asset_key)?;
    let url = providers::asset_url(&job.href)?;
    let original_item = url
        .path_segments()
        .and_then(|p| p.rev().nth(1))
        .and_then(|product| {
            let parts = product.split('_').collect::<Vec<_>>();
            (parts.len() == 7).then(|| [&parts[..4], &parts[5..]].concat().join("_"))
        });
    if job.kind == "raster_mosaic" {
        crate::mosaic::validate_stored_mosaic(job)?;
        if job.status != JobStatus::Succeeded
            || crate::extension(&job.media_type)? != "tif"
            || job
                .mosaic_output
                .as_ref()
                .and_then(|p| p.landsat_quality.as_ref())
                != Some(&processing_profile(&job.asset_key)?)
            || job
                .mosaic
                .as_ref()
                .is_none_or(|s| job.item_id != format!("project:{}", s.project_id))
            || original_item
                .as_deref()
                .is_none_or(|id| !providers::matches_item(&url, id, &job.asset_key))
            || uuid::Uuid::parse_str(&job.id)
                .ok()
                .map(|v| v.to_string())
                .as_deref()
                != Some(&job.id)
            || job.rgb_spec.is_some()
        {
            return Err("Landsat quality result requires a verified unsigned mosaic plan".into());
        }
        return Ok(());
    }
    if job.status != JobStatus::Succeeded
        || job.kind != "download"
        || crate::extension(&job.media_type)? != "tif"
        || url.host_str() != Some(providers::LANDSAT_HOST)
        || !providers::matches_item(&url, &job.item_id, &job.asset_key)
        || uuid::Uuid::parse_str(&job.id)
            .ok()
            .map(|v| v.to_string())
            .as_deref()
            != Some(&job.id)
        || job.mosaic.is_some()
        || job.mosaic_output.is_some()
        || job.rgb_spec.is_some()
    {
        return Err(
            "Landsat quality requires a completed, verified original Landsat 8/9 C2 file".into(),
        );
    }
    Ok(())
}
struct Source {
    decoder: Decoder<reflectance::Snapshot>,
    header: reflectance::Header,
    source_nodata: Option<u16>,
    deadline: Instant,
    mask: Option<processing::MaskReader>,
}
fn source(root: &Path, job: &Job) -> Result<Source> {
    validate_job(job)?;
    let deadline = Instant::now() + Duration::from_secs(55);
    let mut decoder = reflectance::verified_decoder(root, job, deadline)?;
    let (header, source_nodata) =
        validate_header(&mut decoder, &job.asset_key, job.mosaic_output.as_ref())?;
    let mask = if job.kind == "raster_mosaic" {
        Some(processing::MaskReader::new(root, job, &header, deadline)?)
    } else {
        None
    };
    Ok(Source {
        decoder,
        header,
        source_nodata,
        deadline,
        mask,
    })
}
pub(crate) fn validate_header<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    key: &str,
    plan: Option<&crate::mosaic::MosaicPlan>,
) -> Result<(reflectance::Header, Option<u16>)> {
    let (width, height) = decoder.dimensions().map_err(io_error)?;
    if width == 0
        || height == 0
        || plan.is_none() && (width > 20000 || height > 20000)
        || decoder.colortype().map_err(io_error)? != ColorType::Gray(16)
        || decoder
            .find_tag_unsigned::<u16>(Tag::SamplesPerPixel)
            .map_err(io_error)?
            .unwrap_or(1)
            != 1
        || decoder
            .find_tag_unsigned::<u16>(Tag::SampleFormat)
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
        return Err("Landsat QA must retain its original unsigned UInt16 base image".into());
    }
    let mut keys = decoder
        .get_tag_u16_vec(Tag::GeoKeyDirectoryTag)
        .map_err(io_error)?;
    let end = 4 + 4 * usize::from(*keys.get(3).ok_or("Missing Landsat QA GeoKey directory")?);
    if keys.len() < end {
        return Err("Malformed Landsat QA GeoKey directory".into());
    }
    let mut pixel_is_point = false;
    for entry in keys[4..end].chunks_exact_mut(4) {
        if entry == [1025, 0, 1, 2] {
            pixel_is_point = true;
            entry[3] = 1;
        }
    }
    let crs = validate_geokeys(&keys)?;
    let read = |d: &mut Decoder<R>, tag| -> Result<Option<Vec<f64>>> {
        d.find_tag(tag)
            .map_err(io_error)?
            .map(|v| v.into_f64_vec())
            .transpose()
            .map_err(io_error)
    };
    let transform = read(decoder, Tag::ModelTransformationTag)?;
    let scale = read(decoder, Tag::ModelPixelScaleTag)?;
    let tiepoint = read(decoder, Tag::ModelTiepointTag)?;
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
    if pixel_size.iter().any(|v| (*v - 30.0).abs() > 1e-6) {
        return Err("Landsat quality requires the original 30 m grid".into());
    }
    let source_nodata = decoder
        .find_tag(Tag::GdalNodata)
        .map_err(io_error)?
        .map(|v| -> Result<u16> {
            v.into_string()
                .map_err(io_error)?
                .trim_matches('\0')
                .trim()
                .parse()
                .map_err(io_error)
        })
        .transpose()?;
    if key == "qa_pixel" && source_nodata.is_some_and(|v| v != 1)
        || key == "qa_radsat" && source_nodata.is_some_and(|v| v != 0)
    {
        return Err("Unexpected Landsat QA TIFF NoData declaration".into());
    }
    if let Some(plan) = plan {
        let profile = processing_profile(key)?;
        let stored: ProcessingProfile = serde_json::from_str(
            decoder
                .get_tag(Tag::ImageDescription)
                .map_err(io_error)?
                .into_string()
                .map_err(io_error)?
                .trim_matches('\0'),
        )
        .map_err(io_error)?;
        if stored != profile
            || plan.landsat_quality.as_ref() != Some(&profile)
            || width != plan.width
            || height != plan.height
            || crs != plan.crs
            || pixel_is_point
            || bounds
                .iter()
                .zip(plan.bounds)
                .any(|(a, b)| (*a - b).abs() > 1e-6)
            || pixel_size != plan.pixel_size
            || source_nodata.is_some()
        {
            return Err(
                "Derived Landsat quality grid, metadata or NoData declaration differs".into(),
            );
        }
    }
    Ok((
        reflectance::Header {
            width,
            height,
            crs,
            bounds,
            pixel_size,
            pixel_is_point,
        },
        source_nodata,
    ))
}
fn chunks(s: &mut Source) -> Result<[u32; 4]> {
    let (w, h) = s.decoder.chunk_dimensions();
    if w == 0 || h == 0 {
        return Err("Invalid Landsat QA chunk grid".into());
    }
    let columns = s.header.width.div_ceil(w);
    let count = match s.decoder.get_chunk_type() {
        ChunkType::Tile => s.decoder.tile_count(),
        ChunkType::Strip => s.decoder.strip_count(),
    }
    .map_err(io_error)?;
    if columns.checked_mul(s.header.height.div_ceil(h)) != Some(count) {
        return Err("Invalid Landsat QA chunk count".into());
    }
    Ok([w, h, columns, count])
}
fn read_chunk(s: &mut Source, index: u32) -> Result<(Vec<u16>, u32, u32)> {
    if Instant::now() > s.deadline {
        return Err("Landsat quality inspection timed out".into());
    }
    let data = match s.decoder.read_chunk(index).map_err(io_error)? {
        DecodingResult::U16(v) => v,
        _ => return Err("Quality decoder did not retain original UInt16".into()),
    };
    let (w, h) = s.decoder.chunk_data_dimensions(index);
    if data.len() != w as usize * h as usize {
        return Err("Invalid Landsat QA decoded sample count".into());
    }
    Ok((data, w, h))
}
pub(crate) fn source_header(root: &Path, job: &Job) -> Result<reflectance::Header> {
    Ok(source(root, job)?.header)
}

/// Checksum-verified, bounded flags. Uncovered derived samples are represented
/// outside the UInt16 range so a valid QA_RADSAT zero can never mean NoData.
pub(crate) fn visit_chunks(
    root: &Path,
    job: &Job,
    token: &tokio_util::sync::CancellationToken,
    check: impl FnOnce(&reflectance::Header) -> Result<()>,
    mut visit: impl FnMut([u32; 5], &[u32]) -> Result<()>,
) -> Result<()> {
    let mut s = source(root, job)?;
    check(&s.header)?;
    let [cw, ch, columns, count] = chunks(&mut s)?;
    for index in 0..count {
        if token.is_cancelled() {
            return Err("Landsat quality-mask processing cancelled".into());
        }
        let (values, stride, decoded_height) = read_chunk(&mut s, index)?;
        let x = index % columns * cw;
        let y = index / columns * ch;
        let width = cw.min(s.header.width - x);
        let height = ch.min(s.header.height - y);
        if width > stride || height > decoded_height {
            return Err("Invalid Landsat quality-mask chunk window".into());
        }
        let mut values: Vec<u32> = values.into_iter().map(u32::from).collect();
        if let Some(mask) = &mut s.mask {
            for row in 0..height {
                if token.is_cancelled() {
                    return Err("Landsat quality-mask processing cancelled".into());
                }
                for column in 0..width {
                    if !mask.at(x + column, y + row, s.deadline)? {
                        values[(row * stride + column) as usize] = u32::MAX;
                    }
                }
            }
        }
        visit([x, y, width, height, stride], &values)?;
    }
    Ok(())
}
fn is_fill(key: &str, value: u16) -> bool {
    key == "qa_pixel" && value & 1 != 0
}
pub(crate) fn class(key: &str, v: u16) -> usize {
    if key == "qa_pixel" {
        for (mask, index) in [(8, 3), (2, 2), (4, 4), (16, 5), (32, 6), (128, 7), (64, 1)] {
            if v & mask != 0 {
                return index;
            }
        }
    } else {
        if v & 0xf680 != 0 {
            return 4;
        } // All unused bits, including 7, 9, 10, 12–15.
        if v & 2048 != 0 {
            return 3;
        }
        if v & 14 != 0 {
            return 1;
        } // RGB B4 / B3 / B2, not bands 1 / 2 / 3.
        if v & 0x0171 != 0 {
            return 2;
        }
    }
    0
}
pub(crate) fn palette(key: &str) -> Vec<(&'static str, [u8; 3])> {
    if key == "qa_pixel" {
        vec![
            ("Clear flag unset", [107, 114, 128]),
            ("Clear cloud flag", [37, 99, 235]),
            ("Dilated cloud", [234, 179, 8]),
            ("High-confidence cloud", [226, 232, 240]),
            ("High-confidence cirrus", [125, 211, 252]),
            ("High-confidence cloud shadow", [100, 50, 0]),
            ("High-confidence snow / ice", [255, 150, 255]),
            ("Water", [0, 160, 190]),
        ]
    } else {
        vec![
            ("No saturation or terrain occlusion flags", [37, 99, 235]),
            ("RGB band saturation", [239, 68, 68]),
            ("Other band saturation", [234, 179, 8]),
            ("Terrain occlusion", [100, 50, 0]),
            ("Unused bits set", [107, 114, 128]),
        ]
    }
}
pub(crate) fn decode(key: &str, value: u16) -> Result<QualityPixel> {
    Ok(QualityPixel {
        layer: if key == "qa_pixel" {
            "QA_PIXEL"
        } else {
            "QA_RADSAT"
        }
        .into(),
        binary: format!("{value:016b}"),
        hex: format!("0x{value:04X}"),
        covered: None,
        fields: fields(key)?
            .iter()
            .map(|&(name, start, end, labels)| {
                let raw = (u32::from(value) >> start) & ((1 << (end - start + 1)) - 1);
                let label = labels
                    .get(raw as usize)
                    .copied()
                    .unwrap_or("Unused bits set");
                QualityField {
                    name: name.into(),
                    start_bit: start,
                    end_bit: end,
                    value: raw,
                    label: label.into(),
                    defined: labels.get(raw as usize).is_some()
                        && label != "Reserved code"
                        && !(name.starts_with("Unused") && raw != 0),
                }
            })
            .collect(),
    })
}
pub(crate) fn inspect(root: &Path, job: &Job, edge: u32) -> Result<RasterInspection> {
    if edge == 0 || edge > PREVIEW_EDGE {
        return Err("Invalid Landsat QA preview size".into());
    }
    let mut s = source(root, job)?;
    let longest = s.header.width.max(s.header.height);
    let pw = (u64::from(s.header.width) * u64::from(edge.min(longest)) / u64::from(longest)).max(1)
        as u32;
    let ph = (u64::from(s.header.height) * u64::from(edge.min(longest)) / u64::from(longest)).max(1)
        as u32;
    let [cw, ch, columns, count] = chunks(&mut s)?;
    let mut targets: BTreeMap<u32, Vec<(usize, u32, u32)>> = BTreeMap::new();
    for y in 0..ph {
        for x in 0..pw {
            let sx = u64::from(x) * u64::from(s.header.width) / u64::from(pw);
            let sy = u64::from(y) * u64::from(s.header.height) / u64::from(ph);
            targets
                .entry(sy as u32 / ch * columns + sx as u32 / cw)
                .or_default()
                .push(((y * pw + x) as usize, sx as u32 % cw, sy as u32 % ch));
        }
    }
    let colors = palette(&job.asset_key);
    let mut counts = vec![0; colors.len()];
    let mut values = vec![0u64; 65536];
    let mut covered_values = s.mask.as_ref().map(|_| vec![0u64; 65536]);
    let mut histograms: Vec<_> = fields(&job.asset_key)?
        .iter()
        .map(|&(name, start, end, _)| Histogram {
            name: name.into(),
            start_bit: start,
            end_bit: end,
            counts: vec![0; 1 << (end - start + 1)],
        })
        .collect();
    let mut rgba = vec![0; pw as usize * ph as usize * 4];
    for index in 0..count {
        let (data, w, h) = read_chunk(&mut s, index)?;
        let valid_w = w.min(s.header.width - index % columns * cw);
        let valid_h = h.min(s.header.height - index / columns * ch);
        for y in 0..valid_h {
            for x in 0..valid_w {
                let v = data[(y * w + x) as usize];
                values[v as usize] += 1;
                if let (Some(mask), Some(valid)) = (&mut s.mask, &mut covered_values) {
                    let inside = mask.at(
                        index % columns * cw + x,
                        index / columns * ch + y,
                        s.deadline,
                    )?;
                    if job.asset_key == "qa_pixel" && inside == is_fill(&job.asset_key, v) {
                        return Err(
                            "QA_PIXEL fill bit differs from the internal coverage mask".into()
                        );
                    }
                    if inside {
                        valid[v as usize] += 1;
                    }
                }
            }
        }
        if let Some(points) = targets.remove(&index) {
            for (target, x, y) in points {
                let v = *data
                    .get((y * w + x) as usize)
                    .ok_or("Invalid Landsat quality preview sample")?;
                let inside = if let Some(mask) = &mut s.mask {
                    mask.at(
                        index % columns * cw + x,
                        index / columns * ch + y,
                        s.deadline,
                    )?
                } else {
                    !is_fill(&job.asset_key, v)
                };
                if inside {
                    rgba[target * 4..target * 4 + 3]
                        .copy_from_slice(&colors[class(&job.asset_key, v)].1);
                    rgba[target * 4 + 3] = 255;
                }
            }
        }
    }
    if !targets.is_empty() {
        return Err("Incomplete Landsat QA preview".into());
    }
    for (v, count) in values
        .into_iter()
        .enumerate()
        .filter(|(_, count)| *count > 0)
    {
        let inside = covered_values.as_ref().map_or_else(
            || {
                if is_fill(&job.asset_key, v as u16) {
                    0
                } else {
                    count
                }
            },
            |values| values[v],
        );
        counts[class(&job.asset_key, v as u16)] += inside;
        for histogram in &mut histograms {
            let bin = (v >> histogram.start_bit)
                & ((1 << (histogram.end_bit - histogram.start_bit + 1)) - 1);
            histogram.counts[bin] += count;
        }
    }
    let mut png_data = Vec::new();
    let mut png = png::Encoder::new(&mut png_data, pw, ph);
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.write_header()
        .map_err(io_error)?
        .write_image_data(&rgba)
        .map_err(io_error)?;
    let h = s.header;
    let total = u64::from(h.width) * u64::from(h.height);
    if histograms
        .iter()
        .any(|f| f.counts.iter().sum::<u64>() != total)
    {
        return Err("Incomplete Landsat QA base-image counts".into());
    }
    let valid = counts.iter().sum();
    if job
        .mosaic_output
        .as_ref()
        .is_some_and(|p| p.covered_pixels != valid)
    {
        return Err("Landsat quality result coverage differs".into());
    }
    Ok(RasterInspection {
        science: None,
        vegetation: None,
        width: h.width,
        height: h.height,
        band_count: 1,
        data_type: "UInt16".into(),
        crs: h.crs,
        bounds: h.bounds,
        pixel_size: h.pixel_size,
        nodata: None,
        preview_data_url: format!("data:image/png;base64,{}", STANDARD.encode(png_data)),
        preview_width: pw,
        preview_height: ph,
        classes: colors
            .into_iter()
            .enumerate()
            .map(|(i, (label, color))| RasterClass {
                value: i as u8,
                label: label.into(),
                color: format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2]),
                count: counts[i],
            })
            .collect(),
        sha256: job.sha256.clone().ok_or("Missing QA checksum")?,
        reflectance: None,
        elevation: None,
        aerial: None,
        radar: None,
        quality: Some(QualityDisplay {
            product: "landsat-c2-l2".into(),
            band: job.asset_key.clone(),
            layer: if job.asset_key == "qa_pixel" {
                "QA_PIXEL"
            } else {
                "QA_RADSAT"
            }
            .into(),
            pixel_interpretation: if h.pixel_is_point {
                "PixelIsPoint"
            } else {
                "PixelIsArea"
            }
            .into(),
            display_field: if job.asset_key == "qa_pixel" {
                "Pixel quality flags"
            } else {
                "Saturation and terrain flags"
            }
            .into(),
            sample_count: total,
            valid_sample_count: valid,
            counts_full_resolution: true,
            definition: DEFINITION.into(),
            flags: Some(Statistics {
                source_nodata: s.source_nodata,
                fill_rule: if job.asset_key == "qa_pixel" {
                    FILL_RULE
                } else {
                    RADSAT_RULE
                }
                .into(),
                coverage: if s.mask.is_some() {
                    DERIVED_COVERAGE
                } else if job.asset_key == "qa_pixel" {
                    PIXEL_COVERAGE
                } else {
                    RADSAT_COVERAGE
                }
                .into(),
                fields: histograms,
                coverage_mask: s.mask.as_ref().map(|_| CoverageStatistics {
                    kind: "internal-1bit".into(),
                    covered_pixels: valid,
                    uncovered_pixels: total - valid,
                }),
            }),
        }),
    })
}
pub(crate) fn sample(root: &Path, job: &Job, x: f64, y: f64) -> Result<RasterPixel> {
    let mut s = source(root, job)?;
    let [left, bottom, right, top] = s.header.bounds;
    let [dx, dy] = s.header.pixel_size;
    if !x.is_finite() || !y.is_finite() || x < left || x >= right || y <= bottom || y > top {
        return Err("Coordinate outside the Landsat QA pixel grid".into());
    }
    let col = ((x - left) / dx).floor() as u32;
    let row = ((top - y) / dy).floor() as u32;
    if col >= s.header.width || row >= s.header.height {
        return Err("Coordinate outside the Landsat QA pixel grid".into());
    }
    let [cw, ch, columns, _] = chunks(&mut s)?;
    let (data, w, _) = read_chunk(&mut s, row / ch * columns + col / cw)?;
    let value = *data
        .get((row % ch * w + col % cw) as usize)
        .ok_or("Invalid QA sample")?;
    let (label, color) = palette(&job.asset_key)[class(&job.asset_key, value)];
    let covered = s
        .mask
        .as_mut()
        .map(|m| m.at(col, row, s.deadline))
        .transpose()?;
    let fill = covered.map_or_else(|| is_fill(&job.asset_key, value), |v| !v);
    let mut quality = decode(&job.asset_key, value)?;
    quality.covered = covered;
    Ok(RasterPixel {
        science: None,
        index_value: None,
        job_id: job.id.clone(),
        sha256: job.sha256.clone().ok_or("Missing QA checksum")?,
        crs: s.header.crs,
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
        quality: Some(quality),
        label: if fill {
            if covered.is_some() {
                "Outside source coverage"
            } else {
                "Fill data"
            }
        } else {
            label
        }
        .into(),
        color: if fill {
            "#000000".into()
        } else {
            format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2])
        },
        is_no_data: fill,
    })
}

#[cfg(test)]
mod tests;
