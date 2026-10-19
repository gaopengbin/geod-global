//! Bounded random access to the exact, independently verified 224-band BSQ.
//! No TIFF decoder limit is raised, and no quicklook replaces numerical data.
use super::*;
use crate::datasets::enmap::{is_selection, INDEX};

fn open(root: &Path, job: &Job) -> Result<Option<(File, Value)>> {
    let Some(selection) = super::super::archive_original::reviewed_original(root, job)? else {
        return Ok(None);
    };
    if !is_selection(&selection) {
        return Ok(None);
    }
    let original = verified_original(root, job)?;
    let (file, sha) = super::super::archive_original::raster_original(root, job, original)?;
    if sha != INDEX.main.sha256 {
        return Err("EnMAP spectral member identity changed".into());
    }
    let receipt = super::super::archive_original::native_receipt(root, job)?
        .ok_or("EnMAP receipt missing")?;
    Ok(Some((
        file,
        serde_json::to_value(receipt).map_err(io_error)?,
    )))
}
fn band_bytes() -> usize {
    INDEX.profile.width as usize * INDEX.profile.height as usize * 2
}
fn offset(column: u32, row: u32, band: u16) -> Result<u64> {
    let p = &INDEX.profile;
    if column >= p.width || row >= p.height || band == 0 || band > p.bands {
        return Err(
            "EnMAP pixel or display band is outside its original 1286 × 1197 × 224 cube".into(),
        );
    }
    Ok(
        ((u64::from(band - 1) * u64::from(p.height) + u64::from(row)) * u64::from(p.width)
            + u64::from(column))
            * 2,
    )
}
pub(super) fn encoding() -> Value {
    json!({"kind":"enmap-reviewed-envi-bsq/v1","originalMemberSha256":INDEX.main.sha256,
        "headerSha256":INDEX.header.sha256,"interleave":"bsq","byteOrder":"little-endian",
        "wavelengthUnit":"nm","wavelengths":INDEX.profile.wavelengths,"fwhm":INDEX.profile.fwhm,
        "gain":0.0001,"offset":0,"nodata":-32768,"quantity":"surface reflectance","processingMode":"land"})
}
pub(super) fn inspect(
    root: &Path,
    job: &Job,
    requested: Option<u16>,
) -> Result<Option<GenericRasterInspection>> {
    if requested.is_some_and(|b| b == 0 || b > 224) {
        return Err("Choose an EnMAP display band from 1 to 224".into());
    }
    let Some((mut file, receipt)) = open(root, job)? else {
        return Ok(None);
    };
    let band = requested.unwrap_or(50);
    offset(0, 0, band)?;
    let p = &INDEX.profile;
    // One exact original band is 3078684 bytes; never allocate the cube.
    let mut raw = vec![0u8; band_bytes()];
    file.seek(SeekFrom::Start(offset(0, 0, band)?))
        .map_err(io_error)?;
    file.read_exact(&mut raw).map_err(io_error)?;
    let factor = f64::from(p.width.max(p.height)) / 768.;
    let width = (f64::from(p.width) / factor).round() as u32;
    let height = (f64::from(p.height) / factor).round() as u32;
    let values: Vec<i16> = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .map(|(x, y)| {
            let sx = (u64::from(x) * u64::from(p.width) / u64::from(width)) as usize;
            let sy = (u64::from(y) * u64::from(p.height) / u64::from(height)) as usize;
            let i = (sy * p.width as usize + sx) * 2;
            i16::from_le_bytes([raw[i], raw[i + 1]])
        })
        .collect();
    let range = values
        .iter()
        .filter(|&&v| v != -32768)
        .fold(None, |range: Option<[i16; 2]>, &v| {
            Some(range.map_or([v, v], |r| [r[0].min(v), r[1].max(v)]))
        });
    let mut rgba = Vec::with_capacity(values.len() * 4);
    for value in values {
        let gray = range.map_or(0, |r| {
            if r[0] == r[1] {
                127
            } else {
                ((f64::from(value) - f64::from(r[0])) * 255. / (f64::from(r[1]) - f64::from(r[0])))
                    .round()
                    .clamp(0., 255.) as u8
            }
        });
        rgba.extend_from_slice(&[gray, gray, gray, if value == -32768 { 0 } else { 255 }]);
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
    Ok(Some(GenericRasterInspection {spectral_encoding:Some(encoding()),categorical_encoding:None,sample_encoding:None,
        archive_original:Some(receipt),job_id:job.id.clone(),sha256:INDEX.main.sha256.clone(),width:p.width,height:p.height,
        storage_layout:"bsq".into(),bands:(1..=p.bands).map(|index|Band{index,data_type:"Int16".into(),nodata:json!(-32768)}).collect(),
        crs:Some("EPSG:32611".into()),horizontal_crs:Some("EPSG:32611".into()),vertical_crs:None,
        source_model_pixel_scale:None,source_model_tiepoint:None,transform:Some(p.transform),bounds:Some(p.bounds),
        pixel_interpretation:Some("area".into()),preview_data_url:Some(format!("data:image/png;base64,{}",STANDARD.encode(bytes))),
        preview_width:Some(width),preview_height:Some(height),display_band:band,display_range:range.map(|r|[f64::from(r[0]),f64::from(r[1])]),
        limitations:vec!["Single-band nearest-grid grayscale display only. Preview stretching does not alter the stored Int16 cube or apply reflectance scaling. All 224 raw samples remain available at any original pixel; NoData -32768 is separate from valid negative and above-one reflectance.".into(),
            "EPSG:32611 identifies the original HDR WGS84 UTM zone 11 North parameters. Original HDR/WKT, wavelength and FWHM arrays are preserved in the complete ZIP. Preview is lower-detail; the original 30 m grid is unchanged.".into(),
            "Quality masks remain in the original ZIP. Scalar NoData alone does not certify cloud-free or scientifically useful pixels. EnMAP data ©DLR 2023 All rights reserved; original redistribution is restricted.".into()]}))
}
pub(super) fn sample(
    root: &Path,
    job: &Job,
    column: u32,
    row: u32,
) -> Result<Option<GenericRasterPixel>> {
    offset(column, row, 1)?;
    let Some((mut file, _)) = open(root, job)? else {
        return Ok(None);
    };
    let mut values = Vec::with_capacity(224);
    let mut no_data = Vec::with_capacity(224);
    for band in 1..=224 {
        file.seek(SeekFrom::Start(offset(column, row, band)?))
            .map_err(io_error)?;
        let mut pair = [0u8; 2];
        file.read_exact(&mut pair).map_err(io_error)?;
        let v = i16::from_le_bytes(pair);
        values.push(json!(v));
        no_data.push(v == -32768);
    }
    Ok(Some(GenericRasterPixel {
        job_id: job.id.clone(),
        sha256: INDEX.main.sha256.clone(),
        column,
        row,
        values,
        no_data,
    }))
}
pub(super) fn applies(root: &Path, job: &Job) -> Result<bool> {
    Ok(
        super::super::archive_original::reviewed_original(root, job)?
            .as_ref()
            .is_some_and(is_selection),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bsq_offsets_preserve_all_bands_and_reject_outside_positions() {
        assert_eq!(offset(0, 0, 1).unwrap(), 0);
        assert_eq!(offset(0, 0, 2).unwrap(), band_bytes() as u64);
        assert_eq!(offset(1285, 1196, 224).unwrap(), INDEX.main.bytes - 2);
        for p in [(1286, 0, 1), (0, 1197, 1), (0, 0, 0), (0, 0, 225)] {
            assert!(offset(p.0, p.1, p.2).is_err());
        }
        assert_eq!(encoding()["wavelengths"].as_array().unwrap().len(), 224);
        assert_eq!(encoding()["fwhm"].as_array().unwrap().len(), 224);
    }
}
