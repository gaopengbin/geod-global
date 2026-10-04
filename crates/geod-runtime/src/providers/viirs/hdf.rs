//! Bounded read-only 09A1 v002 HDF-EOS science validation. No GDAL, Python,
//! network access, external HDF5 files, reprojection or cloud mask at runtime.
use super::identity;
use crate::{io_error, raster::check_cancel, Result, MAX_ASSET_BYTES};
use hdf5_reader::{Datatype, Hdf5File, OpenOptions};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    io::Read,
    path::Path,
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

mod odl;
#[cfg(test)]
mod tests;

pub const EDGE: u32 = 1200;
pub const NODATA: i16 = -28672;
pub const SCALE: f64 = 0.0001;
pub const CRS: &str = "VIIRS:Sinusoidal";
const MAX_METADATA: usize = 256 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BandSummary {
    pub band: String,
    pub dataset: String,
    pub data_type: String,
    pub scale: f64,
    pub offset: f64,
    pub nodata: i16,
    pub valid_range: [i16; 2],
    pub sample_count: u32,
    pub no_data_count: u32,
    pub outside_valid_range_count: u32,
    pub minimum: Option<i16>,
    pub maximum: Option<i16>,
    /// Row-major original signed DN, canonical little-endian encoding.
    pub samples_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScienceSummary {
    pub schema_version: String,
    pub source_sha256: String,
    pub item_id: String,
    pub start_date: String,
    pub end_date: String,
    pub grid_name: String,
    pub width: u32,
    pub height: u32,
    pub crs: String,
    pub bounds: [f64; 4],
    pub pixel_size: [f64; 2],
    pub bands: [BandSummary; 3],
    pub quality_mask_applied: bool,
}

impl ScienceSummary {
    pub(crate) fn validate(&self, item: &str, hash: &str) -> Result<()> {
        let product = identity(item).ok_or("VIIRS summary has an invalid product")?;
        let digest = |value: &str| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        };
        let size = std::f64::consts::PI * crate::providers::modis::RADIUS / 18.0;
        let expected = [
            (f64::from(product.horizontal_tile) - 18.0) * size,
            (8.0 - f64::from(product.vertical_tile)) * size,
            (f64::from(product.horizontal_tile) - 17.0) * size,
            (9.0 - f64::from(product.vertical_tile)) * size,
        ];
        if self.schema_version != "geod-viirs-science/v1"
            || self.item_id != item
            || !digest(hash)
            || self.source_sha256 != hash
            || self.crs != CRS
            || self.width != EDGE
            || self.height != EDGE
            || self.quality_mask_applied
            || self.start_date != product.start.to_string()
            || self.end_date != product.end.to_string()
            || self.grid_name.is_empty()
            || self.grid_name.len() > 80
            || !self
                .grid_name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || self
                .bounds
                .iter()
                .zip(expected)
                .any(|(a, b)| !a.is_finite() || (*a - b).abs() > 0.02)
            || self
                .pixel_size
                .iter()
                .any(|v| !v.is_finite() || (*v - size / 1200.0).abs() > 1e-5)
        {
            return Err("VIIRS summary source identity or grid differs".into());
        }
        for (index, band) in self.bands.iter().enumerate() {
            let (key, name) = [
                ("red", "SurfReflect_M5"),
                ("green", "SurfReflect_M4"),
                ("blue", "SurfReflect_M3"),
            ][index];
            if band.band != key
                || band.dataset != format!("/HDFEOS/GRIDS/{}/Data Fields/{name}", self.grid_name)
                || band.data_type != "Int16"
                || band.scale != SCALE
                || band.offset != 0.0
                || band.nodata != NODATA
                || band.valid_range != [-100, 16000]
                || band.sample_count != EDGE * EDGE
                || !digest(&band.samples_sha256)
                || band.no_data_count > band.sample_count
                || band.outside_valid_range_count > band.sample_count - band.no_data_count
                || (band.no_data_count == band.sample_count)
                    != (band.minimum.is_none() && band.maximum.is_none())
                || band
                    .minimum
                    .zip(band.maximum)
                    .is_some_and(|(lo, hi)| lo > hi || lo == NODATA || hi == NODATA)
                || band.minimum.is_some() != band.maximum.is_some()
            {
                return Err("VIIRS summary science band differs".into());
            }
        }
        Ok(())
    }
}

fn guard(cancel: &CancellationToken, deadline: Instant) -> Result<()> {
    check_cancel(Some(cancel))?;
    if Instant::now() > deadline {
        return Err("VIIRS science validation exceeded 55 seconds".into());
    }
    Ok(())
}

/// Parse only immutable bytes already bound to the downloaded source checksum.
/// The resulting summaries do not confer scientific or terrain accuracy.
pub(crate) fn verify(
    path: &Path,
    item: &str,
    expected_size: u64,
    expected_hash: &str,
    cancel: &CancellationToken,
) -> Result<ScienceSummary> {
    read(path, item, expected_size, expected_hash, cancel, |_, _| {
        Ok(())
    })
}

/// Decode unchanged scientific DN from the same immutable, checksum-pinned
/// snapshot as validation. Callers must not publish output before this returns.
pub(crate) fn read(
    path: &Path,
    item: &str,
    expected_size: u64,
    expected_hash: &str,
    cancel: &CancellationToken,
    mut on_band: impl FnMut(&BandSummary, &[i16]) -> Result<()>,
) -> Result<ScienceSummary> {
    let deadline = Instant::now() + Duration::from_secs(55);
    guard(cancel, deadline)?;
    identity(item).ok_or("VIIRS science validation requires a reviewed 09A1 v002 identity")?;
    let mut options = File::options();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1);
    }
    let mut file = options.open(path).map_err(io_error)?;
    if expected_size == 0
        || expected_size > MAX_ASSET_BYTES
        || file.metadata().map_err(io_error)?.len() != expected_size
    {
        return Err("VIIRS HDF5 source byte count differs or exceeds 512 MiB".into());
    }
    let mut bytes = Vec::with_capacity(expected_size as usize);
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        guard(cancel, deadline)?;
        let count = file.read(&mut buffer).map_err(io_error)?;
        if count == 0 {
            break;
        }
        if bytes.len() as u64 + count as u64 > expected_size {
            return Err("VIIRS HDF5 source changed during validation".into());
        }
        hash.update(&buffer[..count]);
        bytes.extend_from_slice(&buffer[..count]);
    }
    if bytes.len() as u64 != expected_size || format!("{:x}", hash.finalize()) != expected_hash {
        return Err("VIIRS HDF5 source checksum differs from the downloaded bytes".into());
    }
    let file = Hdf5File::from_vec_with_options(
        bytes,
        OpenOptions {
            chunk_cache_bytes: 8 * 1024 * 1024,
            chunk_cache_slots: 31,
            ..Default::default()
        },
    )
    .map_err(|e| format!("Invalid VIIRS HDF5: {e}"))?;
    let mut summary = metadata(&file, item)?;
    summary.source_sha256 = expected_hash.into();
    let mut bands = Vec::new();
    for (key, name) in [
        ("red", "SurfReflect_M5"),
        ("green", "SurfReflect_M4"),
        ("blue", "SurfReflect_M3"),
    ] {
        guard(cancel, deadline)?;
        let path = format!("/HDFEOS/GRIDS/{}/Data Fields/{name}", summary.grid_name);
        let data = file
            .dataset(&path)
            .map_err(|e| format!("VIIRS source lacks {name}: {e}"))?;
        if data.shape() != [u64::from(EDGE), u64::from(EDGE)]
            || !matches!(
                data.dtype(),
                Datatype::FixedPoint {
                    size: 2,
                    signed: true,
                    ..
                }
            )
            || data
                .chunks()
                .is_some_and(|shape| shape.len() != 2 || shape.iter().any(|v| *v == 0 || *v > EDGE))
        {
            return Err(format!(
                "VIIRS {name} must contain a bounded 1200 × 1200 Int16 grid"
            ));
        }
        let fill = data
            .attribute("_FillValue")
            .map_err(io_error)?
            .read_scalar::<i16>()
            .map_err(io_error)?;
        let range = data
            .attribute("valid_range")
            .map_err(io_error)?
            .read_1d::<i16>()
            .map_err(io_error)?;
        let scale = data
            .attribute("scale_factor")
            .map_err(io_error)?
            .read_scalar::<f64>()
            .map_err(io_error)?;
        let offset = data
            .attribute("add_offset")
            .map_err(io_error)?
            .read_scalar::<f64>()
            .map_err(io_error)?;
        if fill != NODATA
            || range != [-100, 16000]
            || (scale - SCALE).abs() > 1e-12
            || offset != 0.0
            || !scale.is_finite()
        {
            return Err(format!(
                "VIIRS {name} calibration differs from the reviewed v002 product"
            ));
        }
        let mut pixels = vec![0i16; (EDGE * EDGE) as usize];
        data.read_into(&mut pixels)
            .map_err(|e| format!("VIIRS {name} pixel decoding failed: {e}"))?;
        guard(cancel, deadline)?;
        let mut hash = Sha256::new();
        let mut no_data_count = 0;
        let mut outside_valid_range_count = 0;
        let mut minimum = None;
        let mut maximum = None;
        for row in pixels.chunks(EDGE as usize) {
            guard(cancel, deadline)?;
            for &value in row {
                hash.update(value.to_le_bytes());
                if value == NODATA {
                    no_data_count += 1;
                    continue;
                }
                if !(-100..=16000).contains(&value) {
                    outside_valid_range_count += 1;
                }
                minimum = Some(minimum.map_or(value, |previous: i16| previous.min(value)));
                maximum = Some(maximum.map_or(value, |previous: i16| previous.max(value)));
            }
        }
        let band = BandSummary {
            band: key.into(),
            dataset: path,
            data_type: "Int16".into(),
            scale: SCALE,
            offset: 0.0,
            nodata: fill,
            valid_range: [-100, 16000],
            sample_count: EDGE * EDGE,
            no_data_count,
            outside_valid_range_count,
            minimum,
            maximum,
            samples_sha256: format!("{:x}", hash.finalize()),
        };
        on_band(&band, &pixels)?;
        bands.push(band);
    }
    summary.bands = bands
        .try_into()
        .map_err(|_| "VIIRS RGB layer set is incomplete")?;
    summary.validate(item, expected_hash)?;
    Ok(summary)
}

fn attribute_text(group: &hdf5_reader::group::Group, name: &str) -> Result<String> {
    let attribute = group
        .attribute(name)
        .map_err(|e| format!("VIIRS metadata lacks {name}: {e}"))?;
    if attribute.num_elements().map_err(io_error)? != 1 || attribute.raw_data.len() > MAX_METADATA {
        return Err(format!("VIIRS {name} metadata is oversized or not scalar"));
    }
    let value = attribute.read_string().map_err(io_error)?;
    if value.len() > MAX_METADATA || value.contains('\0') {
        return Err(format!("VIIRS {name} metadata string is invalid"));
    }
    Ok(value.trim().into())
}

fn metadata(file: &Hdf5File, item: &str) -> Result<ScienceSummary> {
    let product = identity(item).ok_or("Invalid VIIRS v002 product")?;
    // HDF-EOS metadata lives in FILE_ATTRIBUTES; require that exact group.
    let attrs = file
        .group("/HDFEOS/ADDITIONAL/FILE_ATTRIBUTES")
        .map_err(io_error)?;
    for (name, expected) in [
        ("LocalGranuleID", format!("{item}.h5")),
        ("ShortName", product.product.into()),
        ("RangeBeginningDate", product.start.to_string()),
        ("RangeEndingDate", product.end.to_string()),
        ("RangeBeginningTime", "00:00:00.000".into()),
        ("RangeEndingTime", "23:59:59.000".into()),
        ("VersionID", "002".into()),
        ("SensorShortname", "VIIRS".into()),
    ] {
        if attribute_text(&attrs, name)? != expected {
            return Err(format!("VIIRS {name} differs from the pinned product"));
        }
    }
    for (name, expected) in [
        ("HorizontalTileNumber", product.horizontal_tile),
        ("VerticalTileNumber", product.vertical_tile),
    ] {
        if attribute_text(&attrs, name)?
            .parse::<u32>()
            .map_err(io_error)?
            != expected
        {
            return Err(format!("VIIRS {name} differs from the pinned tile"));
        }
    }
    let metadata = file
        .dataset("/HDFEOS INFORMATION/StructMetadata.0")
        .map_err(io_error)?;
    if !metadata.shape().is_empty()
        || !matches!(metadata.dtype(), Datatype::String { size: hdf5_reader::StringSize::Fixed(size), .. } if *size as usize <= MAX_METADATA)
    {
        return Err("VIIRS structural metadata must be one bounded fixed string".into());
    }
    let text = metadata.read_string().map_err(io_error)?;
    if text.len() > MAX_METADATA {
        return Err("VIIRS structural metadata is oversized".into());
    }
    let grid = odl::grid(&text, product.horizontal_tile, product.vertical_tile)?;
    let empty = BandSummary {
        band: String::new(),
        dataset: String::new(),
        data_type: String::new(),
        scale: SCALE,
        offset: 0.0,
        nodata: NODATA,
        valid_range: [-100, 16000],
        sample_count: 0,
        no_data_count: 0,
        outside_valid_range_count: 0,
        minimum: None,
        maximum: None,
        samples_sha256: String::new(),
    };
    Ok(ScienceSummary {
        schema_version: "geod-viirs-science/v1".into(),
        source_sha256: String::new(),
        item_id: item.into(),
        start_date: product.start.to_string(),
        end_date: product.end.to_string(),
        grid_name: grid.name,
        width: EDGE,
        height: EDGE,
        crs: CRS.into(),
        bounds: grid.bounds,
        pixel_size: grid.pixel_size,
        bands: std::array::from_fn(|_| empty.clone()),
        quality_mask_applied: false,
    })
}
