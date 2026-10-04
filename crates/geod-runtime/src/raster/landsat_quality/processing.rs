//! A derived QA raster carries a separate mask, never a saturation NoData code.
use super::*;
use serde::Deserialize;

pub(crate) const MOSAIC_POLICY: &str = "newest scene with QA_PIXEL bit 0 unset wins; complete UInt16 flags retained; filled newer scenes do not erase covered older samples; independent internal mask; no quality ranking or bit merging";
pub(crate) const DERIVED_COVERAGE: &str = "Coverage follows the independent internal mask, derived from matching QA_PIXEL bit 0 and the project geometry";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProcessingProfile {
    pub schema_version: String,
    pub product: String,
    pub band: String,
    pub bits: u8,
    pub definition: String,
    pub coverage: String,
}
pub(crate) fn processing_profile(key: &str) -> Result<ProcessingProfile> {
    fields(key)?;
    Ok(ProcessingProfile {
        schema_version: "geod-landsat-quality-mosaic/v1".into(),
        product: "landsat-c2-l2".into(),
        band: key.into(),
        bits: 16,
        definition: DEFINITION.into(),
        coverage: DERIVED_COVERAGE.into(),
    })
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageStatistics {
    pub kind: String,
    pub covered_pixels: u64,
    pub uncovered_pixels: u64,
}

pub(super) struct MaskReader {
    decoder: Decoder<reflectance::Snapshot>,
    width: u32,
    rows: u32,
    last: u32,
    data: Vec<u8>,
}
impl MaskReader {
    pub(super) fn new(
        root: &Path,
        job: &Job,
        header: &reflectance::Header,
        deadline: Instant,
    ) -> Result<Self> {
        let plan = job
            .mosaic_output
            .as_ref()
            .ok_or("Missing Landsat quality processing plan")?;
        let mut decoder = reflectance::verified_decoder(root, job, deadline)?;
        super::super::aerial::mask::select(&mut decoder, header.width, header.height)?;
        let rows = decoder.chunk_dimensions().1;
        let stride = header.width.div_ceil(8) as usize;
        let mut covered = 0u64;
        for index in 0..decoder.strip_count().map_err(io_error)? {
            super::super::check_time(deadline)?;
            let data = super::super::aerial::mask::strip(&mut decoder, index)?;
            for row in data.chunks_exact(stride) {
                covered += row.iter().map(|v| u64::from(v.count_ones())).sum::<u64>();
                if !header.width.is_multiple_of(8)
                    && row[stride - 1] & ((1 << (8 - header.width % 8)) - 1) != 0
                {
                    return Err("Landsat quality coverage mask padding differs".into());
                }
            }
        }
        if covered != plan.covered_pixels {
            return Err("Landsat quality coverage count differs from its plan".into());
        }
        Ok(Self {
            decoder,
            width: header.width,
            rows,
            last: u32::MAX,
            data: Vec::new(),
        })
    }
    pub(super) fn at(&mut self, x: u32, y: u32, deadline: Instant) -> Result<bool> {
        super::super::check_time(deadline)?;
        let index = y / self.rows;
        if self.last != index {
            self.data = super::super::aerial::mask::strip(&mut self.decoder, index)?;
            self.last = index;
        }
        let offset = (y % self.rows) as usize * self.width.div_ceil(8) as usize + x as usize / 8;
        let byte = self
            .data
            .get(offset)
            .ok_or("Missing Landsat quality coverage sample")?;
        Ok(byte & (0x80 >> (x % 8)) != 0)
    }
}
