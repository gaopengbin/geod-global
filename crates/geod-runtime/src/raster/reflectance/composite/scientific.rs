//! Persisted three-band scientific RGB. Display stretches never enter the TIFF.
use super::*;
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

mod coupled;
#[cfg(test)]
mod coupled_tests;
mod encode;
mod landsat_mask;
#[cfg(test)]
mod landsat_mask_tests;
#[cfg(test)]
mod mask_tests;
mod modis_mask;
mod package;
mod quality_mask;
pub(crate) mod reader;
#[cfg(test)]
mod tests;

const SCHEMA: &str = "geod-scientific-rgb/v1";
const MAX_SPEC_BYTES: usize = 2 * 1024 * 1024;
const MAX_RAW_BYTES: u64 = 500 * 1024 * 1024;
const STEPS: u64 = 1000;
pub use coupled::{ModisCoupledResult, ModisCoupledScene, ModisCoupledSpec};
pub use landsat_mask::{LandsatMaskPolicy, LandsatMaskRequest, LandsatMaskSpec};
pub use modis_mask::{ModisMaskPolicy, ModisMaskRequest, ModisMaskResult, ModisMaskSpec};
pub use quality_mask::{QualityMaskRequest, QualityMaskSpec};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RgbRequest {
    pub job_ids: [String; 3],
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality_mask: Option<QualityMaskRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RgbSource {
    #[serde(flatten)]
    pub pin: BandPin,
    pub kind: String,
    pub item_id: String,
    pub href: String,
    pub bytes: u64,
    pub attribution: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RgbSpec {
    pub schema_version: String,
    pub name: String,
    pub sources: [RgbSource; 3],
    pub grid: Grid,
    pub profile: Profile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality_mask: Option<QualityMaskSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RgbOutput {
    /// SHA-256 of row-major, little-endian output samples, per channel.
    pub samples_sha256: [String; 3],
    pub channel_valid_pixels: [u64; 3],
    pub common_valid_pixels: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality_mask: Option<ModisMaskResult>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RgbPlan {
    pub spec: RgbSpec,
    pub raw_bytes: u64,
    pub required_disk_bytes: u64,
}

fn canonical_id(id: &str) -> bool {
    uuid::Uuid::parse_str(id)
        .ok()
        .map(|v| v.to_string())
        .as_deref()
        == Some(id)
}
fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn raw_bytes(grid: &Grid) -> Result<u64> {
    let bytes = u64::from(grid.width)
        .checked_mul(u64::from(grid.height))
        .and_then(|pixels| pixels.checked_mul(6))
        .ok_or("Scientific RGB size overflow")?;
    if bytes == 0 || bytes > MAX_RAW_BYTES {
        return Err("Scientific RGB exceeds the 500 MiB uncompressed sample limit".into());
    }
    Ok(bytes)
}
fn disk_bytes(grid: &Grid, masked: bool) -> Result<u64> {
    let raw = raw_bytes(grid)?;
    Ok(raw * 2 + raw / 100 + 8 * 1024 * 1024 + if masked { raw / 6 * 8 } else { 0 })
}

pub(crate) fn validate_spec(spec: &RgbSpec) -> Result<()> {
    let p = &spec.profile;
    let (signed, scale, offset, nodata) = match p.product.as_str() {
        "landsat-c2-l2" => (false, 0.0000275, -0.2, 0),
        "hls-l30-v2" => (true, 0.0001, 0.0, -9999),
        "modis-09a1-v061" | "viirs-09a1-v002" => (true, 0.0001, 0.0, -28672),
        _ => return Err("Unsupported scientific RGB product".into()),
    };
    let g = &spec.grid;
    if spec.schema_version != SCHEMA
        || spec.name.trim().is_empty()
        || spec.name.len() > 512
        || spec.name.chars().any(char::is_control)
        || p.signed != signed
        || p.scale != scale
        || p.offset != offset
        || p.nodata != nodata
        || g.width == 0
        || g.height == 0
        || g.width > 20000
        || g.height > 20000
        || !g.bounds.iter().chain(&g.pixel_size).all(|v| v.is_finite())
        || g.bounds[0] >= g.bounds[2]
        || g.bounds[1] >= g.bounds[3]
        || g.pixel_size.iter().any(|v| *v <= 0.0)
        || !matches!(
            g.pixel_interpretation.as_str(),
            "PixelIsPoint" | "PixelIsArea"
        )
        || spec
            .project_id
            .as_deref()
            .is_some_and(|id| !canonical_id(id))
        || serde_json::to_vec(spec).map_err(io_error)?.len() > MAX_SPEC_BYTES
    {
        return Err("Invalid scientific RGB parameters, calibration or grid".into());
    }
    let pixel = match p.product.as_str() {
        "modis-09a1-v061" => providers::modis::PIXEL,
        "viirs-09a1-v002" => std::f64::consts::PI * providers::modis::RADIUS / 18.0 / 1200.0,
        _ => 30.0,
    };
    if g.pixel_size.iter().any(|v| (*v - pixel).abs() > 1e-6)
        || [0, 1].into_iter().any(|i| {
            ((g.bounds[i + 2] - g.bounds[i]) / [g.width, g.height][i] as f64 - g.pixel_size[i])
                .abs()
                > 1e-7
        })
        || (p.product == "modis-09a1-v061" && g.crs != providers::modis::CRS)
        || (p.product == "viirs-09a1-v002" && g.crs != providers::viirs::hdf::CRS)
        || (!matches!(p.product.as_str(), "modis-09a1-v061" | "viirs-09a1-v002")
            && !g
                .crs
                .strip_prefix("EPSG:")
                .and_then(|v| v.parse::<u16>().ok())
                .is_some_and(|v| (32601..=32660).contains(&v) || (32701..=32760).contains(&v)))
    {
        return Err("Scientific RGB grid differs from its pinned product".into());
    }
    for (i, source) in spec.sources.iter().enumerate() {
        if !canonical_id(&source.pin.job_id)
            || !digest(&source.pin.sha256)
            || source.pin.band != ["red", "green", "blue"][i]
            || source.kind != spec.sources[0].kind
            || !matches!(
                source.kind.as_str(),
                "download" | "raster_prepare" | "raster_mosaic"
            )
            || source.bytes == 0
            || source.bytes > MAX_ASSET_BYTES
            || source.item_id.is_empty()
            || source.item_id.len() > 512
            || source.attribution.len() > 2048
            || spec.sources[..i]
                .iter()
                .any(|s| s.pin.job_id == source.pin.job_id)
        {
            return Err("Invalid scientific RGB source pins".into());
        }
        providers::asset_url(&source.href)?;
    }
    let expected_pair = pair_key(&spec.sources[0])?;
    for source in &spec.sources[1..] {
        if pair_key(source)? != expected_pair {
            return Err("RGB source selections or processing areas differ".into());
        }
    }
    if let Some(mask) = &spec.quality_mask {
        quality_mask::validate_spec(spec, mask)?;
    }
    raw_bytes(g)?;
    Ok(())
}

pub(crate) fn validate_stored(job: &Job) -> Result<()> {
    let spec = job
        .rgb_spec
        .as_ref()
        .ok_or("Scientific RGB source specification is missing")?;
    validate_spec(spec)?;
    if job.kind != "raster_rgb"
        || job.asset_key != "reflectance_rgb"
        || job.media_type != "image/tiff"
        || !canonical_id(&job.id)
        || job.title != spec.name
        || job.href != spec.sources[0].href
        || job.item_id != spec.sources[0].item_id
        || job.parent_id.is_some()
        || job.recipe.is_some()
        || job.mosaic.is_some()
        || job.safe.is_some()
        || job.viirs_prepare.is_some()
        || job.stac_source.is_some()
        || job.wcs_source.is_some()
    {
        return Err("Stored scientific RGB belongs to a different operation or source".into());
    }
    if job.status == JobStatus::Succeeded {
        let output = job
            .rgb_output
            .as_ref()
            .ok_or("Scientific RGB validation result is missing")?;
        let pixels = u64::from(spec.grid.width) * u64::from(spec.grid.height);
        if !output.samples_sha256.iter().all(|h| digest(h))
            || output.channel_valid_pixels.iter().any(|v| *v > pixels)
            || output.common_valid_pixels > *output.channel_valid_pixels.iter().min().unwrap()
        {
            return Err("Invalid scientific RGB sample validation result".into());
        }
        match (&spec.quality_mask, &output.quality_mask) {
            (None, None) => {}
            (Some(_), Some(mask))
                if mask.examined_pixels == pixels
                    && mask.rejected_pixels <= pixels
                    && mask.input_common_valid_pixels <= pixels
                    && mask.removed_valid_pixels <= mask.rejected_pixels
                    && mask.removed_valid_pixels <= mask.input_common_valid_pixels
                    && mask.input_common_valid_pixels - mask.removed_valid_pixels
                        == output.common_valid_pixels => {}
            _ => {
                return Err(
                    "Scientific RGB quality-mask validation result differs from its specification"
                        .into(),
                )
            }
        }
        coupled::validate_result(spec, output)?;
    }
    Ok(())
}

fn pair_key(source: &RgbSource) -> Result<Value> {
    let url = providers::asset_url(&source.href)?;
    if source.kind == "raster_mosaic" {
        let p = source
            .provenance
            .as_ref()
            .ok_or("Processed RGB band has no provenance")?;
        let inputs = p["sources"]
            .as_array()
            .ok_or("Processed RGB source list is missing")?;
        let mut identities = Vec::with_capacity(inputs.len());
        for input in inputs {
            let href = input["href"]
                .as_str()
                .ok_or("Processed RGB input URL is missing")?;
            let directory = providers::asset_url(href)?.join(".").map_err(io_error)?;
            identities.push(json!([
                input["itemId"],
                directory.as_str(),
                input["compositePeriod"],
                input["acquiredAt"],
                input.pointer("/originalHdf5/sha256")
            ]));
        }
        Ok(json!([
            p.pointer("/project/id"),
            p.pointer("/project/bounds"),
            p.pointer("/project/geometry"),
            identities
        ]))
    } else if source.kind == "raster_prepare" {
        let p = source
            .provenance
            .as_ref()
            .ok_or("Prepared RGB band has no original HDF5 pin")?;
        Ok(json!([
            source.item_id,
            source.href,
            p["sourceJobId"],
            p["sourceSha256"]
        ]))
    } else {
        Ok(json!([
            source.item_id,
            url.join(".").map_err(io_error)?.as_str()
        ]))
    }
}

async fn source_pins(manager: &JobManager, jobs: &[Job; 3]) -> Result<[RgbSource; 3]> {
    let (_, pins) = validate_jobs(jobs)?;
    let mut sources = Vec::with_capacity(3);
    for (job, pin) in jobs.iter().zip(pins) {
        let provenance = if job.kind == "raster_mosaic" {
            Some(
                serde_json::from_slice(&manager.mosaic_metadata_bytes(&job.id).await?.1)
                    .map_err(io_error)?,
            )
        } else if let Some(prepared) = &job.viirs_prepare {
            Some(serde_json::to_value(prepared).map_err(io_error)?)
        } else {
            None
        };
        sources.push(RgbSource {
            pin,
            kind: job.kind.clone(),
            item_id: job.item_id.clone(),
            href: job.href.clone(),
            bytes: job.bytes_downloaded,
            attribution: job.source.clone(),
            provenance,
        });
    }
    let sources: [RgbSource; 3] = sources.try_into().map_err(|_| "RGB requires three bands")?;
    let first = pair_key(&sources[0])?;
    for source in &sources[1..] {
        if pair_key(source)? != first {
            return Err("RGB bands use different scene selections or processing areas".into());
        }
    }
    Ok(sources)
}

pub(super) async fn validate_pairing(manager: &JobManager, jobs: &[Job; 3]) -> Result<()> {
    if jobs.iter().any(|job| job.kind == "raster_mosaic") {
        source_pins(manager, jobs).await?;
    }
    Ok(())
}

pub(crate) fn validate_sources(job: &Job, jobs: &BTreeMap<String, Job>) -> Result<[Job; 3]> {
    validate_stored(job)?;
    let spec = job.rgb_spec.as_ref().unwrap();
    let mut result = Vec::with_capacity(3);
    for pin in &spec.sources {
        let source = jobs
            .get(&pin.pin.job_id)
            .ok_or("Scientific RGB source job was removed")?;
        if source.sha256.as_deref() != Some(pin.pin.sha256.as_str())
            || source.bytes_downloaded != pin.bytes
            || source.kind != pin.kind
            || source.item_id != pin.item_id
            || source.href != pin.href
            || source.asset_key != pin.pin.band
            || source.status != JobStatus::Succeeded
        {
            return Err("Scientific RGB source differs from its pinned completed band".into());
        }
        result.push(source.clone());
    }
    let result = result.try_into().map_err(|_| "RGB requires three bands")?;
    if validate_jobs(&result)?.0 != spec.profile {
        return Err("Scientific RGB source calibration changed".into());
    }
    Ok(result)
}

fn inspect_grid(root: &Path, jobs: &[Job; 3]) -> Result<Grid> {
    let deadline = Instant::now() + Duration::from_secs(55);
    let mut expected = None;
    for job in jobs {
        let source = source_with_deadline(root, job, deadline)?;
        let actual = grid(&source);
        if expected.as_ref().is_some_and(|g| g != &actual) {
            return Err(
                "Local RGB bands have different grids; no implicit resampling is performed".into(),
            );
        }
        expected = Some(actual);
    }
    expected.ok_or("Missing scientific RGB grid".into())
}

impl JobManager {
    pub async fn plan_scientific_rgb(&self, request: RgbRequest) -> Result<RgbPlan> {
        let jobs = self.composite_jobs(request.job_ids).await?;
        let (profile, _) = validate_jobs(&jobs)?;
        let sources = source_pins(self, &jobs).await?;
        let (quality_mask, quality_jobs) = if let Some(mask) = request.quality_mask {
            let (mask, jobs) = quality_mask::plan(self, mask).await?;
            (Some(mask), Some(jobs))
        } else {
            (None, None)
        };
        let coupled_jobs = if let Some(mask) = &quality_mask {
            coupled::original_jobs(self, &sources, mask).await?
        } else {
            None
        };
        let derived_project = jobs[0].mosaic.as_ref().map(|m| m.project_id.clone());
        let project_id = request.project_id.or(derived_project.clone());
        if let Some(id) = &project_id {
            if derived_project
                .as_ref()
                .is_some_and(|original| original != id)
            {
                return Err("Processed RGB bands belong to a different project".into());
            }
            let project = self
                .inner
                .projects
                .lock()
                .await
                .get(id)
                .cloned()
                .ok_or("Unknown RGB project")?;
            if derived_project.is_none()
                && !jobs.iter().all(|job| {
                    project.scenes.iter().any(|scene| {
                        scene.item_id == job.item_id
                            && scene
                                .assets
                                .get(if job.viirs_prepare.is_some() {
                                    "viirs"
                                } else {
                                    &job.asset_key
                                })
                                .is_some_and(|asset| asset.href == job.href)
                    })
                })
            {
                return Err("RGB source bands are not in the selected project".into());
            }
        }
        let name = request
            .name
            .unwrap_or_else(|| {
                format!(
                    "{} · {}",
                    jobs[0].item_id,
                    if quality_mask.is_some() {
                        "quality-masked RGB"
                    } else {
                        "scientific RGB"
                    }
                )
            })
            .trim()
            .to_owned();
        let root = self.inner.root.clone();
        let permit = tokio::time::timeout(
            Duration::from_secs(5),
            self.inner.raster_permits.clone().acquire_owned(),
        )
        .await
        .map_err(|_| "Raster inspection is busy; try again shortly")?
        .map_err(io_error)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let mut spec = RgbSpec { schema_version: SCHEMA.into(), name, sources, grid: inspect_grid(&root, &jobs)?, profile, project_id, quality_mask };
            if let Some(originals) = &coupled_jobs {
                let selection = coupled::pin_spec(&root, &spec, originals)?;
                let mask = spec.quality_mask.as_mut().unwrap();
                mask.set_coupled(selection)?;
            }
            validate_spec(&spec)?;
            if let Some(jobs) = &quality_jobs { quality_mask::inspect_grids(&root, spec.quality_mask.as_ref().unwrap(), &spec.grid, jobs)?; }
            let required_disk_bytes = coupled::disk_bytes(&spec)?;
            if fs2::available_space(root.join("assets")).map_err(io_error)? < required_disk_bytes {
                return Err(format!("Insufficient workspace disk space for scientific RGB: need {required_disk_bytes} bytes"));
            }
            Ok(RgbPlan { raw_bytes: raw_bytes(&spec.grid)?, required_disk_bytes, spec })
        }).await.map_err(io_error)?
    }

    pub async fn run_scientific_rgb(&self, request: RgbRequest) -> Result<Job> {
        let spec = self.plan_scientific_rgb(request).await?.spec;
        let mut store = self.inner.store.lock().await;
        store.accepting_jobs()?;
        if let Some(existing) = store
            .jobs
            .values()
            .find(|job| crate::active(&job.status) && job.rgb_spec.as_deref() == Some(&spec))
        {
            return Ok(existing.clone());
        }
        if store.active.len() >= 64 {
            return Err("The local queue is full (64 jobs)".into());
        }
        let mut job = crate::new_download_job(crate::CreateJobRequest {
            item_id: spec.sources[0].item_id.clone(),
            asset_key: "reflectance_rgb".into(),
            href: spec.sources[0].href.clone(),
            media_type: "image/tiff".into(),
            title: Some(spec.name.clone()),
        });
        job.kind = "raster_rgb".into();
        job.source = "Locally combined pinned original reflectance samples".into();
        job.validation = "Checking pinned RGB bands".into();
        job.rgb_spec = Some(Box::new(spec));
        validate_sources(&job, &store.jobs)?;
        if let Some(mask) = &job.rgb_spec.as_ref().unwrap().quality_mask {
            quality_mask::validate_sources(mask, &store.jobs)?;
        }
        coupled::validate_sources(job.rgb_spec.as_ref().unwrap(), &store.jobs)?;
        store.jobs.insert(job.id.clone(), job.clone());
        if let Err(error) = self.persist(&store.jobs).await {
            store.jobs.remove(&job.id);
            return Err(error);
        }
        let token = CancellationToken::new();
        store.active.insert(job.id.clone(), token.clone());
        self.spawn(job.id.clone(), token);
        Ok(job)
    }

    pub(crate) async fn process_scientific_rgb(
        &self,
        id: &str,
        token: &CancellationToken,
    ) -> Result<()> {
        let permit = tokio::select! { _ = token.cancelled() => return Err("Scientific RGB cancelled".into()),
        permit = self.inner.raster_permits.clone().acquire_owned() => permit.map_err(io_error)?, };
        self.rgb_progress(id, 0, "Checking pinned RGB bands")
            .await?;
        let job = self.get(id).await.ok_or("Unknown scientific RGB job")?;
        let (sources, quality_jobs, coupled_jobs) = {
            let store = self.inner.store.lock().await;
            (
                validate_sources(&job, &store.jobs)?,
                job.rgb_spec
                    .as_ref()
                    .unwrap()
                    .quality_mask
                    .as_ref()
                    .map(|mask| quality_mask::validate_sources(mask, &store.jobs))
                    .transpose()?,
                coupled::validate_sources(job.rgb_spec.as_ref().unwrap(), &store.jobs)?,
            )
        };
        let spec = job.rgb_spec.unwrap();
        if source_pins(self, &sources).await? != spec.sources {
            return Err("RGB source provenance changed after planning".into());
        }
        if let (Some(mask), Some(jobs)) = (&spec.quality_mask, &quality_jobs) {
            if quality_mask::source_pins(self, mask, jobs).await? != *mask.sources() {
                return Err("RGB quality-layer provenance changed after planning".into());
            }
        }
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let root = self.inner.root.clone();
        let output_id = id.to_owned();
        let parameters = spec.clone();
        let cancel = token.clone();
        let worker = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            if let Some(originals) = &coupled_jobs {
                coupled::write(&root, &output_id, &parameters, originals, &cancel, &tx)
            } else {
                encode::write(
                    &root,
                    &output_id,
                    &parameters,
                    &sources,
                    quality_jobs.as_ref(),
                    &cancel,
                    &tx,
                )
            }
        });
        let mut update_error = None;
        while let Some((completed, stage)) = rx.recv().await {
            if let Err(e) = self.rgb_progress(id, completed, stage).await {
                update_error.get_or_insert(e);
            }
        }
        let output = worker.await.map_err(io_error)??;
        if let Some(error) = update_error {
            return Err(error);
        }
        let manifest = json!({"schemaVersion":SCHEMA,"createdAt":crate::now(),"spec":spec,
            "output":{"file":format!("{id}.tif"),"format":"GeoTIFF","bytes":output.bytes,"sha256":output.sha256,"samples":output.samples}});
        let assets = self.inner.root.join("assets");
        let manifest_path = assets.join(format!("{id}.metadata.json"));
        let temporary = assets.join(format!("{id}.metadata.json.tmp"));
        let mut file = tokio::fs::File::create(&temporary)
            .await
            .map_err(io_error)?;
        file.write_all(&serde_json::to_vec_pretty(&manifest).map_err(io_error)?)
            .await
            .map_err(io_error)?;
        file.sync_all().await.map_err(io_error)?;
        drop(file);
        tokio::fs::rename(&temporary, &manifest_path)
            .await
            .map_err(io_error)?;
        let mut store = self.inner.store.lock().await;
        let record = store.jobs.get_mut(id).ok_or("Unknown scientific RGB job")?;
        if token.is_cancelled() || !crate::active(&record.status) {
            return Err("Scientific RGB cancelled".into());
        }
        let before = record.clone();
        record.status = JobStatus::Succeeded;
        record.output_path = Some(output.path);
        record.sha256 = Some(output.sha256);
        record.bytes_downloaded = output.bytes;
        record.total_bytes = Some(output.bytes);
        record.rgb_output = Some(output.samples);
        record.manifest_path = Some(manifest_path.to_string_lossy().into_owned());
        record.updated_at = crate::now();
        record.error = None;
        record.validation = if spec.quality_mask.is_some() {
            "Pinned product-specific quality rules applied; accepted RGB DN retained, rejected pixels set to NoData; every output sample checked"
        } else { "Original 16-bit RGB samples, per-channel NoData, calibration and grid retained; every output sample checked" }.into();
        if let Err(error) = self.persist(&store.jobs).await {
            store.jobs.insert(id.into(), before);
            return Err(error);
        }
        Ok(())
    }

    async fn rgb_progress(&self, id: &str, completed: u64, stage: &str) -> Result<()> {
        let mut store = self.inner.store.lock().await;
        let job = store.jobs.get_mut(id).ok_or("Unknown scientific RGB job")?;
        if !crate::active(&job.status) {
            return Err("Scientific RGB cancelled".into());
        }
        job.status = JobStatus::Running;
        job.bytes_downloaded = completed;
        job.total_bytes = Some(STEPS);
        job.validation = stage.into();
        job.updated_at = crate::now();
        self.persist(&store.jobs).await
    }
}
