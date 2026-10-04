//! Original HGT samples are staged as raw bytes and read in bounded strips.
//! No resampling or intermediate image encoding changes their signed values.
use super::*;

pub(crate) enum SourceDecoder {
    Tiff(Box<Decoder<BufReader<File>>>),
    Hgt {
        raw: tempfile::NamedTempFile,
        _pin: File,
    },
}

const HGT_ROWS: u32 = 128;
const EDGE: u32 = crate::providers::srtm::EDGE;

impl SourceDecoder {
    pub(crate) fn into_tiff(self) -> Result<Decoder<BufReader<File>>> {
        match self {
            Self::Tiff(decoder) => Ok(*decoder),
            Self::Hgt { .. } => Err("Original HGT has no TIFF decoder".into()),
        }
    }

    pub(super) fn as_tiff_mut(&mut self) -> Result<&mut Decoder<BufReader<File>>> {
        match self {
            Self::Tiff(decoder) => Ok(decoder),
            Self::Hgt { .. } => Err("Original HGT is not an aerial TIFF".into()),
        }
    }

    pub(super) fn chunk_dimensions(&self) -> (u32, u32) {
        match self {
            Self::Tiff(decoder) => decoder.chunk_dimensions(),
            Self::Hgt { .. } => (EDGE, HGT_ROWS),
        }
    }

    pub(super) fn get_chunk_type(&self) -> ChunkType {
        match self {
            Self::Tiff(decoder) => decoder.get_chunk_type(),
            Self::Hgt { .. } => ChunkType::Strip,
        }
    }

    pub(super) fn tile_count(&mut self) -> Result<u32> {
        match self {
            Self::Tiff(decoder) => decoder.tile_count().map_err(io_error),
            Self::Hgt { .. } => Err("HGT uses strips".into()),
        }
    }

    pub(super) fn strip_count(&mut self) -> Result<u32> {
        match self {
            Self::Tiff(decoder) => decoder.strip_count().map_err(io_error),
            Self::Hgt { .. } => Ok(EDGE.div_ceil(HGT_ROWS)),
        }
    }

    pub(super) fn chunk_data_dimensions(&self, index: u32) -> (u32, u32) {
        match self {
            Self::Tiff(decoder) => decoder.chunk_data_dimensions(index),
            Self::Hgt { .. } => (EDGE, HGT_ROWS.min(EDGE.saturating_sub(index * HGT_ROWS))),
        }
    }

    pub(super) fn read_chunk(&mut self, index: u32) -> Result<DecodingResult> {
        match self {
            Self::Tiff(decoder) => decoder.read_chunk(index).map_err(io_error),
            Self::Hgt { raw, .. } => {
                if index >= EDGE.div_ceil(HGT_ROWS) {
                    return Err("HGT strip index is outside its grid".into());
                }
                let rows = HGT_ROWS.min(EDGE - index * HGT_ROWS);
                let mut bytes = vec![0; EDGE as usize * rows as usize * 2];
                let file = raw.as_file_mut();
                file.seek(SeekFrom::Start(
                    u64::from(index * HGT_ROWS) * u64::from(EDGE) * 2,
                ))
                .map_err(io_error)?;
                file.read_exact(&mut bytes).map_err(io_error)?;
                Ok(DecodingResult::I16(
                    bytes
                        .chunks_exact(2)
                        .map(|p| i16::from_be_bytes([p[0], p[1]]))
                        .collect(),
                ))
            }
        }
    }
}

pub(super) fn hgt_source(
    root: &Path,
    job: &Job,
    cancel: &CancellationToken,
) -> Result<SourceRaster> {
    let assets = root.join("assets").canonicalize().map_err(io_error)?;
    if assets != root.join("assets") {
        return Err("Managed HGT source directory was redirected".into());
    }
    if fs2::available_space(&assets).map_err(io_error)? < crate::providers::srtm::HGT_BYTES {
        return Err("Insufficient workspace space to stage original HGT samples".into());
    }
    let (pin, bytes) = crate::raster::srtm::open_original(root, job, Some(cancel))?;
    let mut raw = tempfile::Builder::new()
        .prefix("srtm-grid-")
        .rand_bytes(6)
        .suffix(".part")
        .tempfile_in(&assets)
        .map_err(io_error)?;
    for chunk in bytes.chunks(65536) {
        check_cancel(Some(cancel))?;
        raw.write_all(chunk).map_err(io_error)?;
    }
    Ok(SourceRaster {
        decoder: SourceDecoder::Hgt { raw, _pin: pin },
        width: EDGE,
        height: EDGE,
        crs: "EPSG:4326".into(),
        bounds: crate::raster::srtm::bounds(&job.item_id)?,
        pixel_size: [1.0 / 3600.0; 2],
        nodata: Some(-32768.0),
        elevation: Some(crate::raster::srtm::profile()),
        calibration: None,
        radar: None,
        quality: None,
        bands: 1,

        landsat_quality: None,
        coverage_source: None,
    })
}
