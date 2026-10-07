//! Product-specific rules share one checked, bounded flag-staging path.
use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum QualityMaskRequest {
    Modis(ModisMaskRequest),
    Landsat(LandsatMaskRequest),
}
impl From<ModisMaskRequest> for QualityMaskRequest {
    fn from(value: ModisMaskRequest) -> Self {
        Self::Modis(value)
    }
}
impl From<LandsatMaskRequest> for QualityMaskRequest {
    fn from(value: LandsatMaskRequest) -> Self {
        Self::Landsat(value)
    }
}
impl QualityMaskRequest {
    pub fn job_ids(&self) -> [&str; 2] {
        match self {
            Self::Modis(m) => [&m.qc_job_id, &m.state_job_id],
            Self::Landsat(m) => [&m.qa_pixel_job_id, &m.qa_radsat_job_id],
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum QualityMaskSpec {
    Modis(ModisMaskSpec),
    Landsat(LandsatMaskSpec),
}
impl From<ModisMaskSpec> for QualityMaskSpec {
    fn from(value: ModisMaskSpec) -> Self {
        Self::Modis(value)
    }
}
impl QualityMaskSpec {
    pub(crate) fn request(&self) -> QualityMaskRequest {
        match self {
            Self::Modis(m) => ModisMaskRequest {
                qc_job_id: m.sources[0].pin.job_id.clone(),
                state_job_id: m.sources[1].pin.job_id.clone(),
                policy: m.policy,
                exclude_snow: m.exclude_snow,
            }
            .into(),
            Self::Landsat(m) => LandsatMaskRequest {
                qa_pixel_job_id: m.sources[0].pin.job_id.clone(),
                qa_radsat_job_id: m.sources[1].pin.job_id.clone(),
                policy: m.policy,
                exclude_snow: m.exclude_snow,
            }
            .into(),
        }
    }
    pub fn sources(&self) -> &[RgbSource; 2] {
        match self {
            Self::Modis(m) => &m.sources,
            Self::Landsat(m) => &m.sources,
        }
    }
    pub fn sources_mut(&mut self) -> &mut [RgbSource; 2] {
        match self {
            Self::Modis(m) => &mut m.sources,
            Self::Landsat(m) => &mut m.sources,
        }
    }
    pub fn schema_version(&self) -> &str {
        match self {
            Self::Modis(m) => &m.schema_version,
            Self::Landsat(m) => &m.schema_version,
        }
    }
    pub fn coupled(&self) -> Option<&ModisCoupledSpec> {
        match self {
            Self::Modis(m) => m.coupled.as_ref(),
            Self::Landsat(m) => m.coupled.as_ref(),
        }
    }
    pub fn coupled_mut(&mut self) -> Option<&mut ModisCoupledSpec> {
        match self {
            Self::Modis(m) => m.coupled.as_mut(),
            Self::Landsat(m) => m.coupled.as_mut(),
        }
    }
    pub(super) fn set_coupled(&mut self, selection: ModisCoupledSpec) -> Result<()> {
        match self {
            Self::Modis(m) => {
                m.schema_version = "geod-modis-rgb-mask/v2".into();
                m.coupled = Some(selection);
                Ok(())
            }
            Self::Landsat(m) => {
                m.schema_version = "geod-landsat-rgb-mask/v2".into();
                m.coupled = Some(selection);
                Ok(())
            }
        }
    }
    pub(super) fn accepted(&self, a: u32, b: u32) -> bool {
        match self {
            Self::Modis(m) => modis_mask::accepted(m.policy, m.exclude_snow, a, b),
            Self::Landsat(m) => landsat_mask::accepted(m.policy, m.exclude_snow, a, b),
        }
    }
}
pub(super) async fn plan(
    manager: &JobManager,
    request: QualityMaskRequest,
) -> Result<(QualityMaskSpec, [Job; 2])> {
    match request {
        QualityMaskRequest::Modis(r) => {
            let (m, j) = modis_mask::plan(manager, r).await?;
            Ok((m.into(), j))
        }
        QualityMaskRequest::Landsat(r) => {
            let (m, j) = landsat_mask::plan(manager, r).await?;
            Ok((QualityMaskSpec::Landsat(m), j))
        }
    }
}
pub(super) fn validate_spec(spec: &RgbSpec, mask: &QualityMaskSpec) -> Result<()> {
    match mask {
        QualityMaskSpec::Modis(m) => modis_mask::validate_spec(spec, m),
        QualityMaskSpec::Landsat(m) => landsat_mask::validate_spec(spec, m),
    }
}
pub(super) fn validate_sources(
    mask: &QualityMaskSpec,
    jobs: &BTreeMap<String, Job>,
) -> Result<[Job; 2]> {
    match mask {
        QualityMaskSpec::Modis(m) => modis_mask::validate_sources(m, jobs),
        QualityMaskSpec::Landsat(m) => landsat_mask::validate_sources(m, jobs),
    }
}
pub(super) async fn source_pins(
    manager: &JobManager,
    mask: &QualityMaskSpec,
    jobs: &[Job; 2],
) -> Result<[RgbSource; 2]> {
    match mask {
        QualityMaskSpec::Modis(_) => modis_mask::source_pins(manager, jobs).await,
        QualityMaskSpec::Landsat(_) => landsat_mask::source_pins(manager, jobs).await,
    }
}
pub(super) fn inspect_grids(
    root: &Path,
    mask: &QualityMaskSpec,
    grid: &Grid,
    jobs: &[Job; 2],
) -> Result<()> {
    match mask {
        QualityMaskSpec::Modis(_) => modis_mask::inspect_grids(root, grid, jobs),
        QualityMaskSpec::Landsat(_) => landsat_mask::inspect_grids(root, grid, jobs),
    }
}
fn check_grid(
    mask: &QualityMaskSpec,
    grid: &Grid,
    header: &crate::raster::reflectance::Header,
) -> Result<()> {
    match mask {
        QualityMaskSpec::Modis(_) => modis_mask::check_grid(grid, header),
        QualityMaskSpec::Landsat(_) => landsat_mask::check_grid(grid, header),
    }
}
fn visit_chunks(
    root: &Path,
    job: &Job,
    token: &CancellationToken,
    mask: &QualityMaskSpec,
    check: impl FnOnce(&crate::raster::reflectance::Header) -> Result<()>,
    visit: impl FnMut([u32; 5], &[u32]) -> Result<()>,
) -> Result<()> {
    match mask {
        QualityMaskSpec::Modis(_) => {
            crate::raster::quality::visit_chunks(root, job, token, check, visit)
        }
        QualityMaskSpec::Landsat(_) => {
            crate::raster::landsat_quality::visit_chunks(root, job, token, check, visit)
        }
    }
}

use std::io::Write;
use tempfile::NamedTempFile;

struct Window {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    expected: String,
    readback: Sha256,
}
struct Plane {
    file: NamedTempFile,
    windows: Vec<Window>,
}
pub(super) struct Planes {
    planes: [Plane; 2],
    width: u32,
    mask: QualityMaskSpec,
    signed: bool,
    pub counts: ModisMaskResult,
}
impl Planes {
    pub fn stage(
        root: &Path,
        id: &str,
        spec: &RgbSpec,
        jobs: &[Job; 2],
        token: &CancellationToken,
    ) -> Result<Self> {
        let mask = spec
            .quality_mask
            .as_ref()
            .ok_or("Missing RGB mask specification")?;
        let mut planes = Vec::with_capacity(2);
        for job in jobs {
            encode::cancelled(token)?;
            let mut file = encode::staged(root, id)?;
            file.as_file()
                .set_len(u64::from(spec.grid.width) * u64::from(spec.grid.height) * 4)
                .map_err(io_error)?;
            let mut windows = Vec::new();
            let mut covered = 0u64;
            visit_chunks(
                root,
                job,
                token,
                mask,
                |header| check_grid(mask, &spec.grid, header),
                |[x, y, width, height, stride], data| {
                    let mut hash = Sha256::new();
                    for row in 0..height {
                        encode::cancelled(token)?;
                        let bytes: Vec<u8> = data
                            [(row * stride) as usize..(row * stride + width) as usize]
                            .iter()
                            .flat_map(|v| v.to_le_bytes())
                            .collect();
                        hash.update(&bytes);
                        file.as_file_mut()
                            .seek(SeekFrom::Start(
                                ((y + row) as u64 * spec.grid.width as u64 + x as u64) * 4,
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
                    Ok(())
                },
            )?;
            if covered != u64::from(spec.grid.width) * u64::from(spec.grid.height) {
                return Err("RGB quality chunks did not cover RGB grid".into());
            }
            file.as_file().sync_all().map_err(io_error)?;
            file.as_file_mut()
                .seek(SeekFrom::Start(0))
                .map_err(io_error)?;
            planes.push(Plane { file, windows });
        }
        Ok(Self {
            planes: planes
                .try_into()
                .map_err(|_| "Missing RGB quality planes")?,
            width: spec.grid.width,
            mask: mask.clone(),
            signed: spec.profile.signed,
            counts: ModisMaskResult::default(),
        })
    }
    pub fn apply(
        &mut self,
        y: u32,
        height: u32,
        bands: &mut [Vec<u8>; 3],
        nodata: i32,
    ) -> Result<()> {
        let pixels = self.width as usize * height as usize;
        let mut flags: [Vec<u8>; 2] = std::array::from_fn(|_| vec![0; pixels * 4]);
        for (channel, plane) in self.planes.iter_mut().enumerate() {
            plane
                .file
                .as_file_mut()
                .read_exact(&mut flags[channel])
                .map_err(io_error)?;
            for window in &mut plane.windows {
                for row in y.max(window.y)..(y + height).min(window.y + window.height) {
                    let offset = ((row - y) as usize * self.width as usize + window.x as usize) * 4;
                    window
                        .readback
                        .update(&flags[channel][offset..offset + window.width as usize * 4]);
                }
            }
        }
        for index in 0..pixels {
            let input_valid = bands.iter().all(|b| {
                let value = u16::from_le_bytes([b[index * 2], b[index * 2 + 1]]);
                let dn = if self.signed {
                    i32::from(value as i16)
                } else {
                    i32::from(value)
                };
                dn != nodata
            });
            let [qc, state] = flags
                .each_ref()
                .map(|v| u32::from_le_bytes(v[index * 4..index * 4 + 4].try_into().unwrap()));
            let rejected = !self.mask.accepted(qc, state);
            self.counts.examined_pixels += 1;
            self.counts.input_common_valid_pixels += u64::from(input_valid);
            self.counts.rejected_pixels += u64::from(rejected);
            self.counts.removed_valid_pixels += u64::from(input_valid && rejected);
            if rejected {
                for band in bands.iter_mut() {
                    band[index * 2..index * 2 + 2].copy_from_slice(&(nodata as i16).to_le_bytes());
                }
            }
        }
        Ok(())
    }
    pub fn verify(&self) -> Result<()> {
        for plane in &self.planes {
            for window in &plane.windows {
                if format!("{:x}", window.readback.clone().finalize()) != window.expected {
                    return Err("Staged RGB quality flags changed after source verification".into());
                }
            }
        }
        Ok(())
    }
}
