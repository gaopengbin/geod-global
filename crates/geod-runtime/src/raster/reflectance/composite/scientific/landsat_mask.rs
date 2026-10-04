//! Landsat 8/9 Collection 2 Level-2 screening of a matched five-layer scene.
use super::*;
use crate::raster::landsat_quality;

pub(super) const SCHEMA: &str = "geod-landsat-rgb-mask/v1";
const KEYS: [&str; 2] = ["qa_pixel", "qa_radsat"];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LandsatMaskPolicy {
    CloudFree,
    CloudFreeConservative,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LandsatMaskRequest {
    pub qa_pixel_job_id: String,
    pub qa_radsat_job_id: String,
    pub policy: LandsatMaskPolicy,
    #[serde(default)]
    pub exclude_snow: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LandsatMaskSpec {
    pub schema_version: String,
    pub policy: LandsatMaskPolicy,
    pub exclude_snow: bool,
    pub sources: [RgbSource; 2],
    pub definition: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coupled: Option<ModisCoupledSpec>,
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
pub(super) fn validate_spec(spec: &RgbSpec, mask: &LandsatMaskSpec) -> Result<()> {
    if spec.profile.product != "landsat-c2-l2"
        || mask.schema_version
            != if mask.coupled.is_some() {
                "geod-landsat-rgb-mask/v2"
            } else {
                SCHEMA
            }
        || mask.definition != landsat_quality::DEFINITION
        || (mask.coupled.is_none() && !spec.sources.iter().all(single_scene))
    {
        return Err("Landsat quality screening requires matched same-scene RGB; independent multi-scene mosaics cannot establish a common pixel source".into());
    }
    let expected = pair_key(&spec.sources[0])?;
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
            || pair_key(source)? != expected
        {
            return Err("Landsat QA_PIXEL and QA_RADSAT must match the pinned RGB scene, processing directory and area".into());
        }
        let url = providers::asset_url(&source.href)?;
        let item = url
            .path_segments()
            .and_then(|p| p.rev().nth(1))
            .and_then(|p| {
                let parts: Vec<_> = p.split('_').collect();
                (parts.len() == 7).then(|| [&parts[..4], &parts[5..]].concat().join("_"))
            })
            .ok_or("Invalid Landsat quality processing directory")?;
        if url.host_str() != Some(providers::LANDSAT_HOST)
            || !providers::matches_item(&url, &item, KEYS[i])
            || (source.kind == "download" && source.item_id != item)
        {
            return Err("Invalid Landsat 8/9 Collection 2 quality-layer identity".into());
        }
    }
    if let Some(selection) = &mask.coupled {
        coupled::validate_spec(spec, selection)?;
    }
    Ok(())
}
pub(super) fn validate_sources(
    mask: &LandsatMaskSpec,
    jobs: &BTreeMap<String, Job>,
) -> Result<[Job; 2]> {
    let mut selected = Vec::with_capacity(2);
    for source in &mask.sources {
        let job = jobs
            .get(&source.pin.job_id)
            .ok_or("Pinned Landsat quality-layer job was removed")?;
        landsat_quality::validate_job(job)?;
        if job.asset_key != source.pin.band
            || job.kind != source.kind
            || job.item_id != source.item_id
            || job.href != source.href
            || job.sha256.as_deref() != Some(&source.pin.sha256)
            || job.bytes_downloaded != source.bytes
        {
            return Err("Landsat quality layer changed after planning".into());
        }
        selected.push(job.clone());
    }
    selected
        .try_into()
        .map_err(|_| "Two Landsat quality layers are required".into())
}
pub(super) async fn source_pins(manager: &JobManager, jobs: &[Job; 2]) -> Result<[RgbSource; 2]> {
    let mut sources = Vec::with_capacity(2);
    for (i, job) in jobs.iter().enumerate() {
        landsat_quality::validate_job(job)?;
        if job.asset_key != KEYS[i] {
            return Err("Landsat quality-layer order differs".into());
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
                sha256: job
                    .sha256
                    .clone()
                    .ok_or("Missing Landsat quality checksum")?,
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
        .map_err(|_| "Two Landsat quality layers are required".into())
}
pub(super) async fn plan(
    manager: &JobManager,
    request: LandsatMaskRequest,
) -> Result<(LandsatMaskSpec, [Job; 2])> {
    let jobs: [Result<Job>; 2] = {
        let store = manager.inner.store.lock().await;
        [request.qa_pixel_job_id, request.qa_radsat_job_id].map(|id| {
            if !canonical_id(&id) {
                return Err("Invalid Landsat quality-layer job identifier".into());
            }
            store
                .jobs
                .get(&id)
                .cloned()
                .ok_or("Unknown Landsat quality-layer job".into())
        })
    };
    let [pixel, radsat] = jobs;
    let jobs = [pixel?, radsat?];
    Ok((
        LandsatMaskSpec {
            schema_version: SCHEMA.into(),
            policy: request.policy,
            exclude_snow: request.exclude_snow,
            sources: source_pins(manager, &jobs).await?,
            definition: landsat_quality::DEFINITION.into(),
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
        || (grid.pixel_interpretation == "PixelIsPoint") != header.pixel_is_point
    {
        return Err(
            "Landsat quality-layer grid differs from RGB; no resampling is performed".into(),
        );
    }
    Ok(())
}
pub(super) fn inspect_grids(root: &Path, grid: &Grid, jobs: &[Job; 2]) -> Result<()> {
    for job in jobs {
        check_grid(grid, &landsat_quality::source_header(root, job)?)?;
    }
    Ok(())
}
/// Flags are product-specific: RGB is B4/B3/B2 (RADSAT bits 3/2/1).
/// Water and non-RGB-band saturation alone do not reject an RGB pixel.
pub(super) fn accepted(
    policy: LandsatMaskPolicy,
    exclude_snow: bool,
    pixel: u32,
    radsat: u32,
) -> bool {
    if pixel > u16::MAX as u32
        || radsat > u16::MAX as u32
        || pixel & 0x1f != 0
        || radsat & ((1 << 1) | (1 << 2) | (1 << 3) | (1 << 11)) != 0
        || (exclude_snow && pixel & (1 << 5) != 0)
    {
        return false;
    }
    if policy == LandsatMaskPolicy::CloudFreeConservative {
        // Confidence zero means unset, not an observed low confidence.
        if pixel & (1 << 6) == 0
            || [8, 10, 14]
                .into_iter()
                .any(|shift| (pixel >> shift) & 3 != 1)
            || (exclude_snow && (pixel >> 12) & 3 != 1)
            || radsat & ((1 << 7) | (1 << 9) | (1 << 10) | (15 << 12)) != 0
        {
            return false;
        }
    }
    true
}
