//! Explicit MOD09/MYD09 C6.1 RGB screening. Flags and DN must belong to the
//! same scene. Independently mosaicked bands cannot establish that invariant.
use super::*;
use crate::raster::quality;

const MASK_SCHEMA: &str = "geod-modis-rgb-mask/v1";
const KEYS: [&str; 2] = ["modis_qc", "modis_state"];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ModisMaskPolicy {
    Clear,
    ClearBest,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModisMaskRequest {
    pub qc_job_id: String,
    pub state_job_id: String,
    pub policy: ModisMaskPolicy,
    #[serde(default)]
    pub exclude_snow: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModisMaskSpec {
    pub schema_version: String,
    pub policy: ModisMaskPolicy,
    pub exclude_snow: bool,
    pub sources: [RgbSource; 2],
    pub definition: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coupled: Option<ModisCoupledSpec>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModisMaskResult {
    pub examined_pixels: u64,
    pub rejected_pixels: u64,
    pub input_common_valid_pixels: u64,
    pub removed_valid_pixels: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coupled: Option<ModisCoupledResult>,
}

fn single_scene(source: &RgbSource) -> bool {
    source.kind == "download"
        || (source.kind == "raster_mosaic"
            && source
                .provenance
                .as_ref()
                .and_then(|p| p["sources"].as_array())
                .is_some_and(|s| s.len() == 1))
}
pub(super) fn validate_spec(spec: &RgbSpec, mask: &ModisMaskSpec) -> Result<()> {
    if spec.profile.product != "modis-09a1-v061"
        || mask.schema_version
            != if mask.coupled.is_some() {
                "geod-modis-rgb-mask/v2"
            } else {
                MASK_SCHEMA
            }
        || mask.definition != quality::DEFINITION
        || (mask.coupled.is_none() && !spec.sources.iter().all(single_scene))
    {
        return Err("MODIS quality screening requires same-scene RGB; independently mosaicked sources need coupled quality-aware processing".into());
    }
    let expected_pair = pair_key(&spec.sources[0])?;
    for (i, source) in mask.sources.iter().enumerate() {
        if !canonical_id(&source.pin.job_id)
            || !digest(&source.pin.sha256)
            || source.pin.band != KEYS[i]
            || source.kind != spec.sources[0].kind
            || source.bytes == 0
            || source.bytes > MAX_ASSET_BYTES
            || source.attribution.len() > 2048
            || (mask.coupled.is_none() && !single_scene(source))
            || spec
                .sources
                .iter()
                .any(|s| s.pin.job_id == source.pin.job_id)
            || mask.sources[..i]
                .iter()
                .any(|s| s.pin.job_id == source.pin.job_id)
            || pair_key(source)? != expected_pair
        {
            return Err("MODIS QC and state must match the pinned RGB scene, processing area and source selection".into());
        }
        let url = providers::asset_url(&source.href)?;
        if url.host_str() != Some(providers::modis::HOST)
            || providers::modis::item_from_path(url.path(), KEYS[i]).is_none()
            || (source.kind == "download"
                && !providers::modis::matches(url.path(), &source.item_id, KEYS[i]))
        {
            return Err("Invalid MODIS quality-layer source identity".into());
        }
    }
    if let Some(selection) = &mask.coupled {
        coupled::validate_spec(spec, selection)?;
    }
    Ok(())
}

pub(super) fn validate_sources(
    mask: &ModisMaskSpec,
    jobs: &BTreeMap<String, Job>,
) -> Result<[Job; 2]> {
    let mut selected = Vec::with_capacity(2);
    for source in &mask.sources {
        let job = jobs
            .get(&source.pin.job_id)
            .ok_or("Pinned MODIS quality-layer job was removed")?;
        quality::validate_job(job)?;
        if job.asset_key != source.pin.band
            || job.kind != source.kind
            || job.item_id != source.item_id
            || job.href != source.href
            || job.sha256.as_deref() != Some(&source.pin.sha256)
            || job.bytes_downloaded != source.bytes
        {
            return Err("MODIS quality layer changed after planning".into());
        }
        selected.push(job.clone());
    }
    selected
        .try_into()
        .map_err(|_| "Two MODIS quality layers are required".into())
}
pub(super) async fn source_pins(manager: &JobManager, jobs: &[Job; 2]) -> Result<[RgbSource; 2]> {
    let mut sources = Vec::with_capacity(2);
    for (i, job) in jobs.iter().enumerate() {
        quality::validate_job(job)?;
        if job.asset_key != KEYS[i] {
            return Err("MODIS QC and state order differs".into());
        }
        let provenance = if job.kind == "raster_mosaic" {
            Some(
                serde_json::from_slice(&manager.mosaic_metadata_bytes(&job.id).await?.1)
                    .map_err(io_error)?,
            )
        } else {
            None
        };
        sources.push(RgbSource {
            pin: BandPin {
                job_id: job.id.clone(),
                band: job.asset_key.clone(),
                sha256: job.sha256.clone().ok_or("Quality checksum is missing")?,
            },
            kind: job.kind.clone(),
            item_id: job.item_id.clone(),
            href: job.href.clone(),
            bytes: job.bytes_downloaded,
            attribution: job.source.clone(),
            provenance,
        });
    }
    sources
        .try_into()
        .map_err(|_| "Two MODIS quality layers are required".into())
}
pub(super) async fn plan(
    manager: &JobManager,
    request: ModisMaskRequest,
) -> Result<(ModisMaskSpec, [Job; 2])> {
    let jobs: [Result<Job>; 2] = {
        let store = manager.inner.store.lock().await;
        [request.qc_job_id, request.state_job_id].map(|id| {
            if !canonical_id(&id) {
                return Err("Invalid MODIS quality-layer job identifier".into());
            }
            store
                .jobs
                .get(&id)
                .cloned()
                .ok_or("Unknown MODIS quality-layer job".into())
        })
    };
    let [qc, state] = jobs;
    let jobs = [qc?, state?];
    Ok((
        ModisMaskSpec {
            schema_version: MASK_SCHEMA.into(),
            policy: request.policy,
            exclude_snow: request.exclude_snow,
            sources: source_pins(manager, &jobs).await?,
            definition: quality::DEFINITION.into(),
            coupled: None,
        },
        jobs,
    ))
}
pub(super) fn check_grid(grid: &Grid, header: &crate::raster::reflectance::Header) -> Result<()> {
    if grid.width != header.width
        || grid.height != header.height
        || grid.crs != header.crs
        || grid.bounds != header.bounds
        || grid.pixel_size != header.pixel_size
        || grid.pixel_interpretation != "PixelIsArea"
        || header.pixel_is_point
    {
        return Err("MODIS quality-layer grid differs from RGB; no resampling is performed".into());
    }
    Ok(())
}
pub(super) fn inspect_grids(root: &Path, grid: &Grid, jobs: &[Job; 2]) -> Result<()> {
    for job in jobs {
        check_grid(grid, &quality::source_header(root, job)?)?;
    }
    Ok(())
}

pub(super) fn accepted(policy: ModisMaskPolicy, exclude_snow: bool, qc: u32, state: u32) -> bool {
    if qc == u32::MAX
        || state == u16::MAX as u32
        || state & (3 | (1 << 2) | (3 << 8) | (1 << 10) | (1 << 13)) != 0
        || (exclude_snow && state & ((1 << 12) | (1 << 15)) != 0)
    {
        return false;
    }
    policy != ModisMaskPolicy::ClearBest || qc & (3 | (15 << 2) | (15 << 10) | (15 << 14)) == 0
}
