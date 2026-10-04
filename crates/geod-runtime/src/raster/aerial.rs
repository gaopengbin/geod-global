//! NAIP RGB+NIR source semantics. A fourth sample is never an alpha channel.
use crate::{providers, Job, Result};
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, SeekFrom};
use tiff::{
    decoder::{ChunkType, Decoder},
    tags::Tag,
    ColorType,
};

pub(crate) mod mask;

/// Display channel selection only; source values, coverage and processing
/// profiles are unchanged. Channel four is always NIR, never display alpha.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AerialView {
    #[default]
    Rgb,
    Cir,
    Nir,
}
impl AerialView {
    pub(crate) fn display_bands(self) -> [u8; 3] {
        match self {
            Self::Rgb => [1, 2, 3],
            Self::Cir => [4, 1, 2],
            Self::Nir => [4, 4, 4],
        }
    }
    pub(crate) fn channels(self) -> [usize; 3] {
        self.display_bands().map(|band| usize::from(band - 1))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AerialDisplay {
    pub product: String,
    pub bands: [String; 4],
    pub display_bands: [u8; 3],
    pub pixel_interpretation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage_mask: Option<String>,
    /// Reviewed legacy 1 m and `h` 0.6 m originals label NIR as unassociated alpha.
    /// Preserve that source tag in inspection metadata, never as display alpha.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_extra_sample: Option<u16>,
}
pub fn display() -> AerialDisplay {
    AerialDisplay {
        product: "naip".into(),
        bands: ["red", "green", "blue", "nir"].map(String::from),
        display_bands: [1, 2, 3],
        pixel_interpretation: "PixelIsArea".into(),
        coverage_mask: None,
        source_extra_sample: None,
    }
}
pub(crate) fn masked_display() -> AerialDisplay {
    AerialDisplay {
        coverage_mask: Some("internal-1bit".into()),
        ..display()
    }
}
pub(crate) fn validate_job(job: &Job) -> Result<(u16, f64)> {
    if job.kind == "raster_mosaic" {
        crate::mosaic::validate_stored_mosaic(job)?;
        let plan = job
            .mosaic_output
            .as_ref()
            .ok_or("NAIP result grid is missing")?;
        let epsg = plan
            .crs
            .strip_prefix("EPSG:")
            .and_then(|v| v.parse::<u16>().ok())
            .filter(|v| (26901..=26923).contains(v))
            .ok_or("NAIP result must retain its NAD83 UTM grid")?;
        if job.asset_key != "aerial"
            || plan.band_count != 4
            || plan.aerial.as_ref() != Some(&masked_display())
            || !plan.pixel_size[0].is_finite()
            || plan.pixel_size[0] <= 0.0
            || plan.pixel_size[0] != plan.pixel_size[1]
            || plan.calibration.is_some()
            || plan.elevation.is_some()
            || plan.source_count != job.mosaic.as_ref().unwrap().sources.len()
            || plan.width == 0
            || plan.height == 0
            || plan.covered_pixels == 0
            || plan.covered_pixels > u64::from(plan.width) * u64::from(plan.height)
            || plan.masked_pixels > u64::from(plan.width) * u64::from(plan.height)
        {
            return Err("NAIP result profile is invalid".into());
        }
        return Ok((epsg, plan.pixel_size[0]));
    }
    let url = providers::asset_url(&job.href)?;
    if job.kind != "download"
        || job.asset_key != "aerial"
        || url.host_str() != Some(providers::NAIP_HOST)
        || !providers::matches_item(&url, &job.item_id, "aerial")
    {
        return Err("NAIP inspection requires its reviewed original four-band COG".into());
    }
    let parts: Vec<_> = job.item_id.split('_').collect();
    let zone = parts[4].parse::<u16>().map_err(crate::io_error)?;
    let spacing =
        providers::naip_pixel_size(&job.item_id).ok_or("NAIP source resolution is unsupported")?;
    Ok((26900 + zone, spacing))
}
/// Physical layout validation for already source-validated NAIP reads. Band
/// roles are checked separately against the reviewed job and official catalogue.
pub(crate) fn validate_layout<R: Read + Seek>(decoder: &mut Decoder<R>) -> Result<u16> {
    let invalid = "NAIP requires interleaved UInt8 RGB + NIR; unsupported extra sample layout";
    let extra = decoder
        .find_tag_unsigned_vec::<u16>(Tag::ExtraSamples)
        .map_err(crate::io_error)?;
    let code = match extra.as_deref() {
        Some([0]) => 0,
        Some([2]) => 2,
        _ => return Err(invalid.into()),
    };
    if decoder.colortype().map_err(crate::io_error)?
        != if code == 2 {
            ColorType::RGBA(8)
        } else {
            ColorType::RGB(8)
        }
        || decoder
            .find_tag_unsigned::<u16>(Tag::SamplesPerPixel)
            .map_err(crate::io_error)?
            != Some(4)
        || decoder
            .find_tag_unsigned::<u16>(Tag::PhotometricInterpretation)
            .map_err(crate::io_error)?
            != Some(2)
        || decoder
            .find_tag_unsigned_vec::<u16>(Tag::SampleFormat)
            .map_err(crate::io_error)?
            .is_some_and(|v| v != [1, 1, 1, 1])
        || decoder
            .find_tag_unsigned::<u16>(Tag::PlanarConfiguration)
            .map_err(crate::io_error)?
            .unwrap_or(1)
            != 1
        || decoder
            .find_tag_unsigned::<u16>(Tag::Orientation)
            .map_err(crate::io_error)?
            .unwrap_or(1)
            != 1
        || decoder
            .find_tag(Tag::GdalNodata)
            .map_err(crate::io_error)?
            .is_some()
    {
        return Err(invalid.into());
    }
    // Reviewed NAIP originals have no NoData tag: black and zero NIR are valid.
    Ok(code)
}

pub(crate) fn validate_samples<R: Read + Seek>(decoder: &mut Decoder<R>, job: &Job) -> Result<u16> {
    let extra = validate_layout(decoder)?;
    if extra == 2
        && !(job.kind == "download"
            && job.asset_key == "aerial"
            && providers::asset_url(&job.href).is_ok_and(|url| {
                url.host_str() == Some(providers::NAIP_HOST)
                    && providers::naip_legacy_nir(url.path(), &job.item_id)
            }))
    {
        return Err(
            "Unassociated alpha is allowed only for the reviewed legacy NAIP NIR layouts".into(),
        );
    }
    Ok(extra)
}

pub(crate) fn inspection_display<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    job: &Job,
) -> Result<AerialDisplay> {
    let extra = validate_samples(decoder, job)?;
    Ok(if job.kind == "raster_mosaic" {
        masked_display()
    } else {
        AerialDisplay {
            source_extra_sample: (extra == 2).then_some(extra),
            ..display()
        }
    })
}

/// tiff 0.11.3 discards unspecified extra samples from RGB readout. Decode
/// reviewed 4×UInt8 interleaved blocks directly from the same locked file,
/// with bounded Deflate/uncompressed buffers and exact horizontal prediction.
/// Do not patch source tags or pretend NIR is alpha to coerce the library.
pub(crate) fn read_chunk<R: Read + Seek>(decoder: &mut Decoder<R>, index: u32) -> Result<Vec<u8>> {
    validate_layout(decoder)?;
    let tiled = decoder.get_chunk_type() == ChunkType::Tile;
    let (width, height) = if tiled {
        decoder.chunk_dimensions()
    } else {
        decoder.chunk_data_dimensions(index)
    };
    let expected = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|v| v.checked_mul(4))
        .filter(|v| *v > 0 && *v <= 64 * 1024 * 1024)
        .ok_or("NAIP decoded block exceeds 64 MiB")?;
    let predictor = decoder
        .find_tag_unsigned::<u16>(Tag::Predictor)
        .map_err(crate::io_error)?
        .unwrap_or(1);
    if !matches!(predictor, 1 | 2) {
        return Err("NAIP supports only horizontal prediction or unpredicted blocks".into());
    }
    let mut values = decoded_block(decoder, index, expected)?;
    if predictor == 2 {
        for row in values.chunks_exact_mut(width as usize * 4) {
            for i in 4..row.len() {
                row[i] = row[i].wrapping_add(row[i - 4]);
            }
        }
    }
    let (actual_width, actual_height) = decoder.chunk_data_dimensions(index);
    if actual_width > width || actual_height > height {
        return Err("NAIP block dimensions are invalid".into());
    }
    if actual_width != width || actual_height != height {
        let mut cropped = Vec::with_capacity(actual_width as usize * actual_height as usize * 4);
        for row in values
            .chunks_exact(width as usize * 4)
            .take(actual_height as usize)
        {
            cropped.extend_from_slice(&row[..actual_width as usize * 4]);
        }
        values = cropped;
    }
    Ok(values)
}

pub(crate) fn decoded_block<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    index: u32,
    expected: u64,
) -> Result<Vec<u8>> {
    if expected == 0 || expected > 64 * 1024 * 1024 {
        return Err("NAIP decoded block exceeds 64 MiB".into());
    }
    let tiled = decoder.get_chunk_type() == ChunkType::Tile;
    let offsets = decoder
        .get_tag_u64_vec(if tiled {
            Tag::TileOffsets
        } else {
            Tag::StripOffsets
        })
        .map_err(crate::io_error)?;
    let counts = decoder
        .get_tag_u64_vec(if tiled {
            Tag::TileByteCounts
        } else {
            Tag::StripByteCounts
        })
        .map_err(crate::io_error)?;
    let offset = *offsets
        .get(index as usize)
        .ok_or("NAIP block offset is missing")?;
    let size = *counts
        .get(index as usize)
        .filter(|n| **n > 0 && **n <= 64 * 1024 * 1024)
        .ok_or("NAIP compressed block exceeds 64 MiB")?;
    offset
        .checked_add(size)
        .ok_or("NAIP block range overflow")?;
    let compression = decoder
        .get_tag_unsigned::<u16>(Tag::Compression)
        .map_err(crate::io_error)?;
    if !matches!(compression, 1 | 8 | 32946) {
        return Err(
            "NAIP supports only uncompressed or Deflate UInt8 blocks with predictor 1 or 2".into(),
        );
    }
    let mut encoded = vec![0; size as usize];
    decoder
        .inner()
        .seek(SeekFrom::Start(offset))
        .map_err(crate::io_error)?;
    decoder
        .inner()
        .read_exact(&mut encoded)
        .map_err(crate::io_error)?;
    let mut values = Vec::with_capacity(expected as usize);
    if compression == 1 {
        values = encoded;
    } else {
        flate2::read::ZlibDecoder::new(encoded.as_slice())
            .take(expected + 1)
            .read_to_end(&mut values)
            .map_err(crate::io_error)?;
    }
    if values.len() as u64 != expected {
        return Err("NAIP block sample count is inconsistent".into());
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tiff::encoder::{colortype, TiffEncoder};
    const ID: &str = "ca_m_3712221_nw_10_060_20220518";
    const HREF: &str = "https://naipeuwest.blob.core.windows.net/naip/v002/ca/2022/ca_060cm_2022/37122/m_3712221_nw_10_060_20220518.tif";
    fn fixture(extra: u16, epsg: u16, spacing: f64) -> Vec<u8> {
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut bytes).unwrap();
            let mut image = encoder.new_image::<colortype::RGBA8>(2, 1).unwrap();
            image
                .encoder()
                .write_tag(Tag::ExtraSamples, &[extra][..])
                .unwrap();
            image
                .encoder()
                .write_tag(
                    Tag::GeoKeyDirectoryTag,
                    &[
                        1u16, 1, 0, 4, 1024, 0, 1, 1, 1025, 0, 1, 1, 3072, 0, 1, epsg, 3076, 0, 1,
                        9001,
                    ][..],
                )
                .unwrap();
            image
                .encoder()
                .write_tag(Tag::ModelPixelScaleTag, &[spacing, spacing, 0.0][..])
                .unwrap();
            image
                .encoder()
                .write_tag(
                    Tag::ModelTiepointTag,
                    &[0.0, 0.0, 0.0, 543846.0, 4178430.0, 0.0][..],
                )
                .unwrap();
            image.write_data(&[126, 138, 122, 0, 0, 0, 0, 99]).unwrap();
        }
        bytes.into_inner()
    }
    fn job(root: &std::path::Path, bytes: &[u8]) -> Job {
        let mut job = crate::raster::tests::record(root, bytes);
        job.asset_key = "aerial".into();
        job.item_id = ID.into();
        job.href = HREF.into();
        job
    }
    #[test]
    fn near_infrared_is_not_alpha_and_zero_rgb_remains_valid() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let mut record = job(&root, &fixture(0, 26910, 0.6));
        let result = crate::raster::rgb::inspect(&root, &record).unwrap();
        assert_eq!(result.band_count, 4);
        assert_eq!(result.crs, "EPSG:26910");
        assert_eq!(result.nodata, None);
        assert_eq!(
            result.aerial.unwrap().bands,
            ["red", "green", "blue", "nir"]
        );
        let first = crate::raster::rgb::sample(&root, &record, 543846.1, 4178429.9).unwrap();
        assert_eq!(first.values, Some([126, 138, 122]));
        assert_eq!(first.near_infrared, Some(0));
        assert!(!first.is_no_data);
        let black = crate::raster::rgb::sample(&root, &record, 543846.7, 4178429.9).unwrap();
        assert_eq!(black.values, Some([0, 0, 0]));
        assert_eq!(black.near_infrared, Some(99));
        assert!(!black.is_no_data);
        let png = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            result.preview_data_url.split(',').nth(1).unwrap(),
        )
        .unwrap();
        let mut png = png::Decoder::new(Cursor::new(png)).read_info().unwrap();
        let mut pixels = vec![0; png.output_buffer_size().unwrap()];
        png.next_frame(&mut pixels).unwrap();
        assert_eq!(pixels, [126, 138, 122, 255, 0, 0, 0, 255]);
        record.sha256 = Some("0".repeat(64));
        assert!(
            crate::raster::rgb::sample(&root, &record, 543846.1, 4178429.9)
                .unwrap_err()
                .contains("SHA-256")
        );
    }

    #[test]
    fn aerial_views_keep_original_values_zero_nir_and_source_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let original = fixture(0, 26910, 0.6);
        let record = job(&root, &original);
        for (view, bands, expected) in [
            (
                AerialView::Rgb,
                [1, 2, 3],
                [126, 138, 122, 255, 0, 0, 0, 255],
            ),
            (
                AerialView::Cir,
                [4, 1, 2],
                [0, 126, 138, 255, 99, 0, 0, 255],
            ),
            (AerialView::Nir, [4, 4, 4], [0, 0, 0, 255, 99, 99, 99, 255]),
        ] {
            let result = crate::raster::rgb::inspect_with_view(&root, &record, 768, view).unwrap();
            assert_eq!(result.aerial.unwrap().display_bands, bands);
            assert_eq!((result.width, result.height, result.band_count), (2, 1, 4));
            assert_eq!(result.sha256, record.sha256.clone().unwrap());
            let bytes = base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                result.preview_data_url.split(',').nth(1).unwrap(),
            )
            .unwrap();
            let mut decoder = png::Decoder::new(Cursor::new(bytes)).read_info().unwrap();
            let mut rgba = vec![0; decoder.output_buffer_size().unwrap()];
            decoder.next_frame(&mut rgba).unwrap();
            assert_eq!(rgba, expected);
            let pixel = crate::raster::rgb::sample(&root, &record, 543846.1, 4178429.9).unwrap();
            assert_eq!(pixel.values, Some([126, 138, 122]));
            assert_eq!(pixel.near_infrared, Some(0));
            assert!(!pixel.is_no_data);
        }
        assert_eq!(
            std::fs::read(record.output_path.unwrap()).unwrap(),
            original
        );
        assert!(serde_json::from_str::<AerialView>("\"ndvi\"").is_err());
    }
    #[test]
    fn reviewed_legacy_one_metre_tag_keeps_zero_nir_and_opaque_rgb_without_changing_source() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let original = fixture(2, 26917, 1.0);
        let mut record = job(&root, &original);
        record.item_id = "fl_m_2808060_se_17_1_20171211_20180201".into();
        record.href = "https://naipeuwest.blob.core.windows.net/naip/v002/fl/2017/fl_100cm_2017/28080/m_2808060_se_17_1_20171211.tif".into();
        let result = crate::raster::rgb::inspect(&root, &record).unwrap();
        assert_eq!(result.pixel_size, [1.0, 1.0]);
        assert_eq!(result.aerial.unwrap().source_extra_sample, Some(2));
        let first = crate::raster::rgb::sample(&root, &record, 543846.5, 4178429.5).unwrap();
        assert_eq!(first.values, Some([126, 138, 122]));
        assert_eq!(first.near_infrared, Some(0));
        assert!(!first.is_no_data);
        let black = crate::raster::rgb::sample(&root, &record, 543847.5, 4178429.5).unwrap();
        assert_eq!(black.values, Some([0, 0, 0]));
        assert_eq!(black.near_infrared, Some(99));
        assert!(!black.is_no_data);
        let png = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            result.preview_data_url.split(',').nth(1).unwrap(),
        )
        .unwrap();
        let mut decoder = png::Decoder::new(Cursor::new(png)).read_info().unwrap();
        let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
        decoder.next_frame(&mut pixels).unwrap();
        assert_eq!(pixels, [126, 138, 122, 255, 0, 0, 0, 255]);
        let infrared =
            crate::raster::rgb::inspect_with_view(&root, &record, 768, AerialView::Cir).unwrap();
        assert_eq!(infrared.aerial.unwrap().source_extra_sample, Some(2));
        let bytes = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            infrared.preview_data_url.split(',').nth(1).unwrap(),
        )
        .unwrap();
        let mut decoder = png::Decoder::new(Cursor::new(bytes)).read_info().unwrap();
        let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
        decoder.next_frame(&mut pixels).unwrap();
        assert_eq!(pixels, [0, 126, 138, 255, 99, 0, 0, 255]);
        assert_eq!(
            std::fs::read(record.output_path.as_ref().unwrap()).unwrap(),
            original
        );
        record.item_id = record.item_id.replace("_1_", "_100_");
        record.href = record.href.replace("_1_", "_100_");
        assert!(crate::raster::rgb::inspect(&root, &record).is_err());
    }

    #[test]
    fn aerial_rejects_alpha_wrong_datum_resolution_and_identity() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        for (extra, epsg, spacing) in [
            (1, 26910, 0.6),
            (2, 26910, 0.6),
            (0, 32610, 0.6),
            (0, 26911, 0.6),
            (0, 26910, 1.0),
        ] {
            assert!(crate::raster::rgb::inspect(
                &root,
                &job(&root, &fixture(extra, epsg, spacing))
            )
            .is_err());
        }
        let mut record = job(&root, &fixture(0, 26910, 0.6));
        record.item_id = ID.replace("nw", "ne");
        assert!(crate::raster::rgb::inspect(&root, &record).is_err());
    }

    #[test]
    fn reviewed_legacy_half_metre_nir_tag_preserves_zero_nir_and_source_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let original = fixture(2, 26910, 0.6);
        let mut record = job(&root, &original);
        record.item_id = "ca_m_3712221_sw_10_.6_20160625_20161004".into();
        record.href = "https://naipeuwest.blob.core.windows.net/naip/v002/ca/2016/ca_060cm_2016/37122/m_3712221_sw_10_h_20160625.tif".into();
        let result = crate::raster::rgb::inspect(&root, &record).unwrap();
        assert_eq!(result.pixel_size, [0.6, 0.6]);
        assert_eq!(result.aerial.unwrap().source_extra_sample, Some(2));
        let first = crate::raster::rgb::sample(&root, &record, 543846.1, 4178429.9).unwrap();
        assert_eq!(first.values, Some([126, 138, 122]));
        assert_eq!(first.near_infrared, Some(0));
        assert!(!first.is_no_data);
        let black = crate::raster::rgb::sample(&root, &record, 543846.7, 4178429.9).unwrap();
        assert_eq!(black.near_infrared, Some(99));
        assert!(!black.is_no_data);
        assert_eq!(
            std::fs::read(record.output_path.as_ref().unwrap()).unwrap(),
            original
        );
        record.item_id = record.item_id.replace("_.6_", "_060_");
        assert!(crate::raster::rgb::inspect(&root, &record).is_err());
        record.item_id = record.item_id.replace("_060_", "_.6_");
        record.href = record.href.replace("_h_", "_060_");
        assert!(crate::raster::rgb::inspect(&root, &record).is_err());
    }
}
