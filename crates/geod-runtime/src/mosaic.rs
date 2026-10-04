//! Streamed, pixel-aligned project mosaics. These are new GeoTIFF files made
//! from checksum-pinned local sources, never a browser overlay or remote VRT.
use crate::{
    active, crop, io_error, now,
    projects::Project,
    raster::{check_cancel, georeference, validate_geokeys},
    Job, JobManager, JobStatus, Result,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom, Write},
    path::Path,
    time::{Duration, Instant},
};
use tiff::{
    decoder::{ChunkType, Decoder, DecodingResult, Limits},
    encoder::{compression::DeflateLevel, TiffEncoder},
    tags::Tag,
    ColorType,
};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const MOSAIC_PROGRESS_TOTAL: u64 = 1000;

#[cfg(test)]
mod bands_tests;
#[cfg(test)]
mod elevation_tests;
mod landsat;
#[cfg(test)]
mod landsat_tests;
#[cfg(test)]
mod modis_tests;
#[cfg(test)]
mod quality_tests;
#[cfg(test)]
mod radar_tests;
mod source;
#[cfg(test)]
mod srtm_tests;
mod stream;
pub mod vegetation;
use source::SourceDecoder;

/// Storage is exclusively locked before this is called. Recover only engine
/// HGT staging names; never sweep arbitrary partial files or source assets.
pub(crate) async fn cleanup_staged_hgt(root: &Path) -> Result<()> {
    let assets = root.join("assets");
    if tokio::fs::canonicalize(&assets).await.map_err(io_error)? != assets {
        return Err("Managed HGT staging directory was redirected".into());
    }
    let mut entries = tokio::fs::read_dir(&assets).await.map_err(io_error)?;
    while let Some(entry) = entries.next_entry().await.map_err(io_error)? {
        let name = entry.file_name();
        let managed = name
            .to_str()
            .and_then(|s| s.strip_prefix("srtm-grid-"))
            .and_then(|s| s.strip_suffix(".part"))
            .is_some_and(|s| s.len() == 6 && s.bytes().all(|b| b.is_ascii_alphanumeric()));
        if managed && entry.file_type().await.map_err(io_error)?.is_file() {
            let path = entry.path();
            let actual = crate::storage::exact_file(&path, &path)?;
            tokio::fs::remove_file(actual).await.map_err(io_error)?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MosaicSource {
    pub job_id: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MosaicSpec {
    pub project_id: String,
    pub asset_key: String,
    pub sources: Vec<MosaicSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coverage_sources: Vec<MosaicSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vi_selection: Option<vegetation::Spec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MosaicPlan {
    pub width: u32,
    pub height: u32,
    pub band_count: u8,
    pub crs: String,
    pub bounds: [f64; 4],
    pub pixel_size: [f64; 2],
    pub source_count: usize,
    pub masked_pixels: u64,
    pub covered_pixels: u64,
    pub overlap_policy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibration: Option<crate::raster::reflectance::Profile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elevation: Option<crate::raster::elevation::Profile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aerial: Option<crate::raster::aerial::AerialDisplay>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radar: Option<crate::raster::radar::Profile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<crate::raster::quality::Profile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub landsat_quality: Option<crate::raster::landsat_quality::ProcessingProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vi_quality: Option<vegetation::SelectionResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vi_index: Option<String>,
}

pub(crate) fn validate_stored_mosaic(job: &Job) -> Result<()> {
    let spec = job
        .mosaic
        .as_ref()
        .ok_or("Stored mosaic job has no source specification")?;
    if job.kind != "raster_mosaic"
        || job.asset_key != spec.asset_key
        || !matches!(
            spec.asset_key.as_str(),
            "scl"
                | "visual"
                | "red"
                | "green"
                | "blue"
                | "elevation"
                | "srtm"
                | "aerial"
                | "vv"
                | "vh"
                | "hh"
                | "hv"
                | "ndvi"
                | "evi"
                | "vi_quality"
                | "vi_reliability"
                | "vi_doy"
                | "vi_red"
                | "vi_nir"
                | "vi_blue"
                | "vi_mir"
                | "vi_view_zenith"
                | "vi_sun_zenith"
                | "vi_relative_azimuth"
                | "modis_qc"
                | "modis_state"
                | "qa_pixel"
                | "qa_radsat"
        )
        || Uuid::parse_str(&spec.project_id)
            .ok()
            .map(|id| id.to_string())
            .as_deref()
            != Some(&spec.project_id)
        || spec.sources.is_empty()
        || spec.sources.len() > crate::projects::MAX_PROJECT_SCENES
    {
        return Err("Stored mosaic specification is invalid".into());
    }
    if spec.asset_key == "qa_radsat" && spec.coverage_sources.len() != spec.sources.len()
        || spec.asset_key != "qa_radsat" && !spec.coverage_sources.is_empty()
    {
        return Err(
            "Landsat saturation mosaics require one pinned pixel-quality source per scene".into(),
        );
    }
    for source in spec.sources.iter().chain(&spec.coverage_sources) {
        if Uuid::parse_str(&source.job_id)
            .ok()
            .map(|id| id.to_string())
            .as_deref()
            != Some(&source.job_id)
            || source.sha256.len() != 64
            || !source
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err("Stored mosaic source pin is invalid".into());
        }
    }
    if let Some(selection) = &spec.vi_selection {
        vegetation::validate_spec(&spec.asset_key, &spec.sources, selection)?;
    }
    if let Some(plan) = &job.mosaic_output {
        if let Some(selection) = &spec.vi_selection {
            vegetation::validate_result(selection, &spec.asset_key, plan)?;
        } else if plan.vi_quality.is_some() || plan.vi_index.is_some() {
            return Err("Vegetation quality counts need their pinned selection rules".into());
        }
        landsat::validate_plan(&spec.asset_key, plan)?;
        if plan.landsat_quality.is_some()
            && (plan.source_count != spec.sources.len()
                || spec
                    .sources
                    .iter()
                    .chain(&spec.coverage_sources)
                    .map(|source| &source.job_id)
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != spec.sources.len() + spec.coverage_sources.len())
        {
            return Err(
                "Landsat quality source count or distinct coverage pins differ from the plan"
                    .into(),
            );
        }
        let quality = crate::providers::modis::QUALITY_KEYS.contains(&spec.asset_key.as_str());
        if quality
            && (plan.quality.as_ref() != Some(&crate::raster::quality::profile(&spec.asset_key)?)
                || plan.calibration.is_some()
                || plan.elevation.is_some()
                || plan.aerial.is_some()
                || plan.radar.is_some())
            || !quality && plan.quality.is_some()
        {
            return Err("Stored mosaic mixes quality bit fields with other raster profiles".into());
        }
        let radar = crate::providers::radar::KEYS.contains(&spec.asset_key.as_str());
        if radar
            && (plan.radar.as_ref() != Some(&crate::raster::radar::profile(&spec.asset_key)?)
                || plan.calibration.is_some()
                || plan.elevation.is_some()
                || plan.aerial.is_some())
            || !radar && plan.radar.is_some()
        {
            return Err(
                "Stored mosaic mixes incompatible radar and optical or elevation profiles".into(),
            );
        }
    }
    Ok(())
}

pub(crate) fn validate_mosaic_sources(job: &Job, jobs: &BTreeMap<String, Job>) -> Result<Vec<Job>> {
    validate_stored_mosaic(job)?;
    let spec = job.mosaic.as_ref().unwrap();
    if let Some(selection) = &spec.vi_selection {
        return vegetation::sources(selection, &spec.asset_key, jobs);
    }
    let mut sources = Vec::with_capacity(spec.sources.len());
    for pin in &spec.sources {
        let source = jobs
            .get(&pin.job_id)
            .ok_or("A project source job is missing")?;
        if !matches!(source.kind.as_str(), "download" | "raster_prepare")
            || source.status != JobStatus::Succeeded
            || source.asset_key != spec.asset_key
            || source.sha256.as_deref() != Some(&pin.sha256)
            || crate::extension(&source.media_type)?
                != if spec.asset_key == "srtm" {
                    "zip"
                } else {
                    "tif"
                }
        {
            return Err(
                "A project source is missing, changed or not a completed GeoTIFF download".into(),
            );
        }
        if source.kind == "raster_prepare" {
            crate::prepared::validate_source(source, jobs)?;
        }
        sources.push(source.clone());
    }
    if spec.asset_key == "qa_radsat" {
        for (index, pin) in spec.coverage_sources.iter().enumerate() {
            let source = jobs
                .get(&pin.job_id)
                .ok_or("A pinned Landsat coverage source is missing")?;
            if source.sha256.as_deref() != Some(&pin.sha256) {
                return Err("A pinned Landsat pixel-quality source changed".into());
            }
            landsat::validate_pair(&sources[index], source)?;
            sources.push(source.clone());
        }
    }
    Ok(sources)
}

impl JobManager {
    pub async fn run_project_mosaic(&self, id: &str, asset_key: &str) -> Result<Job> {
        self.run_project_mosaic_with_selection(id, asset_key, None)
            .await
    }

    pub async fn run_project_mosaic_with_selection(
        &self,
        id: &str,
        asset_key: &str,
        selection: Option<vegetation::Request>,
    ) -> Result<Job> {
        if selection.is_some() && !crate::providers::vegetation::KEYS.contains(&asset_key) {
            return Err("Vegetation quality selection is only available for NDVI or EVI".into());
        }
        if !matches!(
            asset_key,
            "scl"
                | "visual"
                | "red"
                | "green"
                | "blue"
                | "elevation"
                | "srtm"
                | "aerial"
                | "vv"
                | "vh"
                | "hh"
                | "hv"
                | "ndvi"
                | "evi"
                | "vi_quality"
                | "vi_reliability"
                | "vi_doy"
                | "vi_red"
                | "vi_nir"
                | "vi_blue"
                | "vi_mir"
                | "vi_view_zenith"
                | "vi_sun_zenith"
                | "vi_relative_azimuth"
                | "modis_qc"
                | "modis_state"
                | "qa_pixel"
                | "qa_radsat"
        ) {
            return Err(
                "Choose SCL, true-color imagery, NAIP imagery, a reflectance band, MODIS or Landsat quality, elevation or an RTC polarization"
                    .into(),
            );
        }
        let project = self
            .inner
            .projects
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or("Unknown project")?;
        let known_crs = project
            .scenes
            .iter()
            .filter_map(|scene| scene.crs.as_deref())
            .collect::<std::collections::HashSet<_>>();
        if known_crs.len() > 1 {
            return Err("Selected scenes span multiple UTM zones; split them into one project per CRS before mosaicking".into());
        }
        let mut store = self.inner.store.lock().await;
        store.accepting_jobs()?;
        if store.active.len() >= 64 {
            return Err("The local queue is full (64 jobs)".into());
        }
        let mut pins = Vec::with_capacity(project.scenes.len());
        let mut coverage_sources = Vec::new();
        let mut ordered_scenes = project.scenes.iter().collect::<Vec<_>>();
        ordered_scenes.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.item_id.cmp(&b.item_id)));
        for scene in ordered_scenes {
            let source =
                crate::prepared::scene_source(scene, &store.jobs, asset_key).ok_or_else(|| {
                    format!(
                        "Download or prepare {} for every scene before mosaicking",
                        asset_key
                    )
                })?;
            pins.push(MosaicSource {
                job_id: source.id.clone(),
                sha256: source.sha256.clone().unwrap(),
            });
            if asset_key == "qa_radsat" {
                let coverage = crate::prepared::scene_source(scene, &store.jobs, "qa_pixel")
                    .ok_or("Download matching QA_PIXEL files for every scene before processing saturation flags")?;
                landsat::validate_pair(source, coverage)?;
                coverage_sources.push(MosaicSource {
                    job_id: coverage.id.clone(),
                    sha256: coverage.sha256.clone().unwrap(),
                });
            }
        }
        let timestamp = now();
        let vi_selection = selection
            .map(|request| vegetation::create(&project, &store.jobs, request.policy))
            .transpose()?;
        if let Some(selection) = &vi_selection {
            vegetation::validate_spec(asset_key, &pins, selection)?;
        }
        let job = Job {
            id: Uuid::new_v4().to_string(),
            kind: "raster_mosaic".into(),
            parent_id: None,
            recipe: None,
            crop: None,
            mosaic: Some(MosaicSpec {
                project_id: project.id.clone(),
                asset_key: asset_key.into(),
                sources: pins,
                coverage_sources,
                vi_selection,
            }),
            mosaic_output: None,
            manifest_path: None,
            safe: None,
            safe_output: None,
            viirs_science: None,
            transfer: None,
            viirs_prepare: None,
            stac_source: None,
            wcs_source: None,
            rgb_spec: None,
            rgb_output: None,
            item_id: format!("project:{}", project.id),
            asset_key: asset_key.into(),
            href: crate::prepared::scene_source(&project.scenes[0], &store.jobs, asset_key)
                .unwrap()
                .href
                .clone(),
            media_type: "image/tiff".into(),
            title: format!("{} · {} mosaic", project.name, asset_key.to_uppercase()),
            status: JobStatus::Queued,
            bytes_downloaded: 0,
            total_bytes: None,
            sha256: None,
            output_path: None,
            error: None,
            created_at: timestamp.clone(),
            updated_at: timestamp,
            source: "Project source rasters / pinned original products".into(),
            validation: "Pending source checksum validation and pixel-aligned mosaic/clip".into(),
            attempts: 1,
        };
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

    pub(crate) async fn process_mosaic(&self, id: &str, token: &CancellationToken) -> Result<()> {
        let permit = tokio::select! {
            _ = token.cancelled() => return Err("Mosaic cancelled".into()),
            permit = self.inner.raster_permits.clone().acquire_owned() => permit.map_err(io_error)?,
        };
        let job = self.get(id).await.ok_or("Unknown mosaic job")?;
        let spec = job
            .mosaic
            .as_ref()
            .ok_or("Mosaic specification is missing")?;
        let mut project = self
            .inner
            .projects
            .lock()
            .await
            .get(&spec.project_id)
            .cloned()
            .ok_or("Project was removed")?;
        if let Some(selection) = &spec.vi_selection {
            // The submitted area and scene set remain stable while a user adds
            // more scenes or renames the project for a later download.
            project.bounds = selection.bounds;
            project.geometry = selection.geometry.clone();
            project
                .scenes
                .retain(|s| selection.scenes.iter().any(|p| p.item_id == s.item_id));
            if project.scenes.len() != selection.scenes.len() {
                return Err("A submitted vegetation scene was removed from the project".into());
            }
        }
        let sources = {
            let store = self.inner.store.lock().await;
            validate_mosaic_sources(&job, &store.jobs)?
        };
        let total_steps = MOSAIC_PROGRESS_TOTAL;
        self.mosaic_progress(id, 0, total_steps, "Checking downloaded sources")
            .await?;
        let source_provenance = sources
            .iter()
            .map(|source| {
                let scene = project
                    .scenes
                    .iter()
                    .find(|scene| scene.item_id == source.item_id);
                let mut provenance = serde_json::json!({
                    "jobId":source.id,"itemId":source.item_id,"href":source.href,
                    "sha256":source.sha256,"attribution":source.source,
                });
                let vegetation_period = crate::providers::vegetation::period(&source.item_id);
                let modis_period = crate::providers::modis::period(&source.item_id).or_else(|| vegetation_period.clone());
                let viirs_period = crate::providers::viirs::identity(&source.item_id);
                let date_key = if modis_period.is_some() || viirs_period.is_some() {
                    "compositeStart"
                } else if matches!(source.asset_key.as_str(), "elevation" | "srtm") {
                    "catalogReferenceDate"
                } else {
                    "acquiredAt"
                };
                provenance[date_key] = serde_json::json!(scene.map(|scene| scene.date.as_str()));
                if let Some(period) = modis_period {
                    provenance["compositePeriod"] =
                        serde_json::json!({"start":period[0],"end":period[1]});
                    provenance["distribution"] = serde_json::json!(
                        if vegetation_period.is_some() { "Planetary Computer converted MOD13Q1/MYD13Q1 v061 science COG; not NASA original HDF" } else { "Planetary Computer converted MOD/MYD09A1 v061 COG; not NASA original HDF" }
                    );
                    provenance["licence"] = serde_json::json!(
                        "https://lpdaac.usgs.gov/data/data-citation-and-policies/"
                    );
                }
                if let (Some(period), Some(prepared)) = (viirs_period, &source.viirs_prepare) {
                    let band = &prepared.science.bands[match source.asset_key.as_str() {
                        "red" => 0,
                        "green" => 1,
                        _ => 2,
                    }];
                    provenance["compositePeriod"] = serde_json::json!({
                        "start":period.start.to_string(),"end":period.end.to_string(),
                    });
                    provenance["originalHdf5"] = serde_json::json!({
                        "jobId":prepared.source_job_id,"sha256":prepared.source_sha256,
                        "href":source.href,"dataset":band.dataset,
                        "samplesSha256":band.samples_sha256,
                    });
                    provenance["qualityMaskApplied"] = serde_json::json!(false);
                    provenance["distribution"] = serde_json::json!(
                        "Locally prepared original VIIRS 09A1 v002 Int16 samples; no rescaling or QA mask"
                    );
                    provenance["licence"] = serde_json::json!(
                        "https://lpdaac.usgs.gov/data/data-citation-and-policies/"
                    );
                }
                if source.asset_key == "elevation" {
                    provenance["licence"] =
                        serde_json::json!("https://registry.opendata.aws/copernicus-dem/");
                } else if source.asset_key == "srtm" {
                    provenance["product"] = serde_json::json!("SRTMGL1 v003");
                    provenance["productReference"] =
                        serde_json::json!("https://doi.org/10.5067/MEASURES/SRTM/SRTMGL1.003");
                } else if crate::providers::radar::KEYS.contains(&source.asset_key.as_str()) {
                    provenance["radar"] = serde_json::json!(crate::raster::radar::profile(&source.asset_key).ok());
                    provenance["distribution"] = serde_json::json!("Catalyst radiometrically terrain corrected Sentinel-1 IW gamma0 COG, hosted by Microsoft Planetary Computer");
                    provenance["licence"] = serde_json::json!("https://creativecommons.org/licenses/by/4.0/");
                    provenance["additionalCalibrationApplied"] = serde_json::json!(false);
                    provenance["speckleFilteringApplied"] = serde_json::json!(false);
                }
                provenance
            })
            .collect::<Vec<_>>();
        let project_provenance = serde_json::json!({
            "id":project.id,"name":project.name,"bounds":project.bounds,"geometry":project.geometry,
        });
        let root = self.inner.root.clone();
        let output_id = id.to_owned();
        let asset_key = spec.asset_key.clone();
        let cancellation = token.clone();
        let selection = spec.vi_selection.clone();
        let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel();
        let worker = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            write_mosaic_with_selection(
                &root,
                &project,
                &sources,
                &asset_key,
                &output_id,
                &cancellation,
                Some(&progress_tx),
                selection.as_ref(),
            )
        });
        let mut update_error = None;
        while let Some((completed, stage)) = progress_rx.recv().await {
            if let Err(error) = self
                .mosaic_progress(id, completed, total_steps, stage)
                .await
            {
                update_error.get_or_insert(error);
            }
        }
        let output = worker.await.map_err(io_error)??;
        if let Some(error) = update_error {
            let _ = tokio::fs::remove_file(&output.path).await;
            return Err(error);
        }
        let manifest_path = self
            .inner
            .root
            .join("assets")
            .join(format!("{id}.metadata.json"));
        let temporary = self
            .inner
            .root
            .join("assets")
            .join(format!("{id}.metadata.json.tmp"));
        let mut manifest = serde_json::json!({
            "schemaVersion":"geod-project-mosaic/v1", "createdAt":now(),
            "output":{"file":format!("{id}.tif"),"format":"GeoTIFF","bytes":output.bytes,"sha256":output.sha256},
            "project":project_provenance,"assetKey":spec.asset_key,
            "sources":source_provenance[..spec.sources.len()],"plan":output.plan,
        });
        if !spec.coverage_sources.is_empty() {
            manifest["coverageSources"] =
                serde_json::json!(&source_provenance[spec.sources.len()..]);
        }
        if let Some(selection) = &spec.vi_selection {
            manifest["viSelection"] = serde_json::to_value(selection).map_err(io_error)?;
        }
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
        let record = store.jobs.get_mut(id).ok_or("Unknown mosaic job")?;
        if token.is_cancelled() || !active(&record.status) {
            let _ = tokio::fs::remove_file(&output.path).await;
            return Err("Mosaic cancelled".into());
        }
        let before = record.clone();
        record.status = JobStatus::Succeeded;
        record.bytes_downloaded = output.bytes;
        record.total_bytes = Some(output.bytes);
        record.sha256 = Some(output.sha256);
        record.output_path = Some(output.path);
        record.mosaic_output = Some(output.plan);
        record.manifest_path = Some(manifest_path.to_string_lossy().into_owned());
        record.updated_at = now();
        record.error = None;
        record.validation = if spec.vi_selection.is_some() {
            "All four MOD13 originals per scene SHA-256 checked; complete same-observation NDVI/EVI selected by pinned quality rules; original Int16 DN, scale and source grid retained; every output sample checked; full-resolution selection and contribution counts retained"
        } else if crate::raster::landsat_quality::KEYS.contains(&spec.asset_key.as_str()) {
            "Original Landsat quality hashes checked; aligned UInt16 flags and independent internal coverage mask retained; QA_PIXEL bit 0 determines coverage; saturation zero remains valid; no bit merging, averaging, cloud filtering or resampling"
        } else if crate::providers::modis::QUALITY_KEYS.contains(&spec.asset_key.as_str()) {
            "Original MODIS quality source hashes checked; aligned unsigned bit fields retained; product fill, grid, embedded definition and every output sample checked; no averaging, bit merging, quality selection, cloud masking or resampling"
        } else if crate::providers::radar::KEYS.contains(&spec.asset_key.as_str()) {
            "Original RTC source hashes checked; aligned Float32 linear gamma0 retained; output geometry, polarization, NoData and every output sample checked; no resampling, averaging, calibration or speckle filtering"
        } else if spec.asset_key == "aerial" {
            "All local sources SHA-256 verified; aligned RGB+NIR original samples mosaicked and clipped; independent internal coverage mask checked; no reprojection or resampling"
        } else {
            "All local sources SHA-256 verified; same-CRS aligned original samples mosaicked and clipped; NoData and calibration retained; output decoded and checked; no reprojection or resampling"
        }.into();
        if let Err(error) = self.persist(&store.jobs).await {
            store.jobs.insert(id.into(), before);
            return Err(error);
        }
        Ok(())
    }

    async fn mosaic_progress(
        &self,
        id: &str,
        completed: u64,
        total: u64,
        stage: &str,
    ) -> Result<()> {
        let mut store = self.inner.store.lock().await;
        let job = store.jobs.get_mut(id).ok_or("Unknown mosaic job")?;
        if !active(&job.status) {
            return Err("Mosaic cancelled".into());
        }
        job.status = JobStatus::Running;
        job.bytes_downloaded = completed;
        job.total_bytes = Some(total);
        job.validation = stage.into();
        job.updated_at = now();
        self.persist(&store.jobs).await
    }
}

pub(crate) struct SourceRaster {
    pub(crate) decoder: SourceDecoder,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) crs: String,
    pub(crate) bounds: [f64; 4],
    pub(crate) pixel_size: [f64; 2],
    pub(crate) nodata: Option<f64>,
    elevation: Option<crate::raster::elevation::Profile>,
    calibration: Option<crate::raster::reflectance::Profile>,
    radar: Option<crate::raster::radar::Profile>,
    quality: Option<crate::raster::quality::Profile>,
    landsat_quality: Option<crate::raster::landsat_quality::ProcessingProfile>,
    coverage_source: Option<Box<SourceRaster>>,
    pub(crate) bands: usize,
}

pub(crate) fn source_raster(
    root: &Path,
    job: &Job,
    key: &str,
    cancel: &CancellationToken,
) -> Result<SourceRaster> {
    check_cancel(Some(cancel))?;
    if key == "srtm" && job.kind == "download" {
        return source::hgt_source(root, job, cancel);
    }
    if job.kind == "raster_prepare" {
        crate::prepared::validate_stored(job)?;
    }
    let assets = root.join("assets").canonicalize().map_err(io_error)?;
    if assets != root.join("assets") {
        return Err("Managed source directory was redirected".into());
    }
    let expected = assets.join(format!("{}.tif", job.id));
    let actual = crate::storage::exact_file(
        &expected,
        Path::new(
            job.output_path
                .as_deref()
                .ok_or("Source has no output file")?,
        ),
    )
    .map_err(|error| format!("Mosaic source is not its managed GeoTIFF file: {error}"))?;
    let mut options = File::options();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Hold the same file against writers/replacement during hash and decoding.
        options.share_mode(1); // FILE_SHARE_READ
    }
    let mut file = options.open(&actual).map_err(io_error)?;
    let size = file.metadata().map_err(io_error)?.len();
    if size == 0
        || (job.kind != "raster_mosaic" && size > crate::source_transfer_limit(job))
        || size != job.bytes_downloaded
        || job.total_bytes.is_some_and(|total| total != size)
    {
        return Err(
            "Mosaic source byte count is invalid or exceeds the reviewed product limit".into(),
        );
    }
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        check_cancel(Some(cancel))?;
        let read = file.read(&mut buffer).map_err(io_error)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    if format!("{:x}", hasher.finalize()) != job.sha256.as_deref().ok_or("Source has no SHA-256")? {
        return Err("Mosaic source SHA-256 changed after download".into());
    }
    file.seek(SeekFrom::Start(0)).map_err(io_error)?;
    let mut limits = Limits::default();
    limits.decoding_buffer_size = 64 * 1024 * 1024;
    limits.intermediate_buffer_size = 32 * 1024 * 1024;
    limits.ifd_value_size = 1024 * 1024;
    let mut decoder = Decoder::new(BufReader::new(file))
        .map_err(io_error)?
        .with_limits(limits);
    let (width, height) = decoder.dimensions().map_err(io_error)?;
    if width == 0
        || height == 0
        || (job.kind != "raster_mosaic"
            && (width
                > if crate::providers::radar::KEYS.contains(&key) {
                    40000
                } else if key == "aerial" {
                    crate::providers::MAX_NAIP_EDGE
                } else {
                    20000
                }
                || height
                    > if crate::providers::radar::KEYS.contains(&key) {
                        40000
                    } else if key == "aerial" {
                        crate::providers::MAX_NAIP_EDGE
                    } else {
                        20000
                    }))
    {
        return Err("Source GeoTIFF dimensions are unsupported".into());
    }
    if matches!(key, "elevation" | "srtm") {
        crate::raster::elevation::validate_job(job)?;
        let header = crate::raster::elevation::validate_header(
            &mut decoder,
            &job.item_id,
            job.mosaic_output.as_ref(),
        )?;
        return Ok(SourceRaster {
            decoder: SourceDecoder::Tiff(Box::new(decoder)),
            width: header.width,
            height: header.height,
            crs: "EPSG:4326".into(),
            bounds: header.bounds,
            pixel_size: header.pixel_size,
            nodata: header.nodata.map(f64::from),
            elevation: Some(crate::raster::elevation::job_profile(job)?),
            bands: 1,
            calibration: None,
            radar: None,
            quality: None,

            landsat_quality: None,
            coverage_source: None,
        });
    }
    if crate::providers::radar::KEYS.contains(&key) {
        crate::raster::radar::validate_job(job)?;
        if job.asset_key != key {
            return Err("Radar source polarization changed".into());
        }
        let header =
            crate::raster::radar::validate_header(&mut decoder, job.mosaic_output.as_ref())?;
        return Ok(SourceRaster {
            decoder: SourceDecoder::Tiff(Box::new(decoder)),
            width: header.width,
            height: header.height,
            crs: header.crs,
            bounds: header.bounds,
            pixel_size: header.pixel_size,
            nodata: Some(-32768.0),
            elevation: None,
            calibration: None,
            radar: Some(crate::raster::radar::profile(key)?),
            quality: None,
            bands: 1,

            landsat_quality: None,
            coverage_source: None,
        });
    }
    if crate::raster::landsat_quality::KEYS.contains(&key) {
        crate::raster::landsat_quality::validate_job(job)?;
        if job.asset_key != key {
            return Err("Landsat quality source layer changed".into());
        }
        let (header, _) = crate::raster::landsat_quality::validate_header(
            &mut decoder,
            key,
            job.mosaic_output.as_ref(),
        )?;
        return Ok(SourceRaster {
            decoder: SourceDecoder::Tiff(Box::new(decoder)),
            width: header.width,
            height: header.height,
            crs: header.crs,
            bounds: header.bounds,
            pixel_size: header.pixel_size,
            nodata: None,
            elevation: None,
            calibration: None,
            radar: None,
            quality: None,
            landsat_quality: Some(crate::raster::landsat_quality::processing_profile(key)?),
            coverage_source: None,
            bands: 1,
        });
    }
    if crate::providers::modis::QUALITY_KEYS.contains(&key) {
        crate::raster::quality::validate_job(job)?;
        if job.asset_key != key {
            return Err("Quality source layer changed".into());
        }
        let header = crate::raster::quality::validate_header(
            &mut decoder,
            key,
            &job.item_id,
            job.mosaic_output.as_ref(),
        )?;
        let profile = crate::raster::quality::profile(key)?;
        return Ok(SourceRaster {
            decoder: SourceDecoder::Tiff(Box::new(decoder)),
            width: header.width,
            height: header.height,
            crs: header.crs,
            bounds: header.bounds,
            pixel_size: header.pixel_size,
            nodata: Some(f64::from(profile.nodata)),
            elevation: None,
            calibration: None,
            radar: None,
            quality: Some(profile),
            bands: 1,

            landsat_quality: None,
            coverage_source: None,
        });
    }
    if matches!(key, "red" | "green" | "blue" | "ndvi" | "evi")
        || crate::providers::vegetation::SCIENCE_KEYS.contains(&key)
    {
        let calibration = crate::raster::reflectance::profile(job)?;
        let header = crate::raster::reflectance::validate_header(&mut decoder, &calibration)?;
        if job.kind == "raster_prepare" {
            crate::providers::viirs::prepare::grid(&header, job)?;
        }
        if calibration.product == "modis-09a1-v061" && job.kind == "download" {
            crate::raster::reflectance::modis::grid(&header, &job.item_id)?;
        }
        if calibration.product == crate::providers::vegetation::PRODUCT && job.kind == "download" {
            crate::raster::reflectance::modis::vegetation_grid(&header, &job.item_id)?;
        }
        crate::raster::science::validate_metadata(&mut decoder, job, &calibration)?;
        vegetation::validate_header(&mut decoder, job)?;
        return Ok(SourceRaster {
            decoder: SourceDecoder::Tiff(Box::new(decoder)),
            width: header.width,
            height: header.height,
            crs: header.crs,
            bounds: header.bounds,
            pixel_size: header.pixel_size,
            nodata: Some(f64::from(calibration.nodata)),
            elevation: None,
            bands: 1,
            calibration: Some(calibration),
            radar: None,
            quality: None,

            landsat_quality: None,
            coverage_source: None,
        });
    }
    let naip = key == "aerial";
    let naip_grid = if naip {
        let grid = crate::raster::aerial::validate_job(job)?;
        crate::raster::aerial::validate_samples(&mut decoder, job)?;
        Some(grid)
    } else {
        None
    };
    let bands = if naip {
        4
    } else if key == "scl" {
        1
    } else {
        3
    };
    let color = decoder.colortype().map_err(io_error)?;
    if decoder
        .find_tag_unsigned::<u16>(Tag::SamplesPerPixel)
        .map_err(io_error)?
        .unwrap_or(1)
        != bands as u16
    {
        return Err("Source sample count differs from its product bands".into());
    }
    if (bands == 1 && color != ColorType::Gray(8)) || (bands == 3 && color != ColorType::RGB(8)) {
        return Err(format!(
            "The {} source must be an unsigned 8-bit {} GeoTIFF",
            key,
            if bands == 1 { "single-band" } else { "RGB" }
        ));
    }
    if decoder
        .find_tag_unsigned::<u16>(Tag::Orientation)
        .map_err(io_error)?
        .unwrap_or(1)
        != 1
        || decoder
            .find_tag_unsigned::<u16>(Tag::PlanarConfiguration)
            .map_err(io_error)?
            .unwrap_or(1)
            != 1
        || decoder
            .find_tag_unsigned_vec::<u16>(Tag::SampleFormat)
            .map_err(io_error)?
            .is_some_and(|samples| samples.iter().any(|sample| *sample != 1))
    {
        return Err("Only top-left, interleaved UInt8 GeoTIFF sources are supported".into());
    }
    if bands == 1
        && decoder
            .find_tag_unsigned::<u16>(Tag::PhotometricInterpretation)
            .map_err(io_error)?
            != Some(1)
    {
        return Err("SCL mosaic requires unmodified grayscale class values".into());
    }
    let keys = decoder
        .get_tag_u16_vec(Tag::GeoKeyDirectoryTag)
        .map_err(|_| "Source GeoTIFF projection keys are missing")?;
    let crs = if let Some((epsg, _)) = naip_grid {
        // Permit NAD83 only for the reviewed aerial product; existing satellite
        // validators retain their WGS84-only contract.
        let crs = crate::raster::validate_projected_geokeys(&keys, true)?;
        if crs != format!("EPSG:{epsg}") {
            return Err("NAIP NAD83 UTM zone differs from its source identity".into());
        }
        crs
    } else {
        validate_geokeys(&keys)?
    };
    let transform = decoder
        .find_tag(Tag::ModelTransformationTag)
        .map_err(io_error)?
        .map(|value| value.into_f64_vec())
        .transpose()
        .map_err(io_error)?;
    let scale = decoder
        .find_tag(Tag::ModelPixelScaleTag)
        .map_err(io_error)?
        .map(|value| value.into_f64_vec())
        .transpose()
        .map_err(io_error)?;
    let tiepoint = decoder
        .find_tag(Tag::ModelTiepointTag)
        .map_err(io_error)?
        .map(|value| value.into_f64_vec())
        .transpose()
        .map_err(io_error)?;
    let (bounds, pixel_size) = georeference(
        width,
        height,
        transform.as_deref(),
        scale.as_deref(),
        tiepoint.as_deref(),
    )?;
    if naip_grid.is_some_and(|(_, spacing)| pixel_size.iter().any(|v| (v - spacing).abs() > 1e-8)) {
        return Err("NAIP pixel spacing differs from its source resolution".into());
    }
    let nodata = decoder
        .find_tag(Tag::GdalNodata)
        .map_err(io_error)?
        .map(|value| {
            value.into_string().map_err(io_error).and_then(|text| {
                text.trim_matches('\0')
                    .trim()
                    .parse::<u8>()
                    .map_err(io_error)
            })
        })
        .transpose()?;
    if nodata.is_some_and(|value| value != 0) {
        return Err("Project mosaics currently require zero-valued NoData samples".into());
    }
    if naip && job.kind == "raster_mosaic" {
        let plan = job
            .mosaic_output
            .as_ref()
            .ok_or("NAIP result grid is missing")?;
        if plan.width != width
            || plan.height != height
            || plan.crs != crs
            || plan.bounds != bounds
            || plan.pixel_size != pixel_size
            || nodata.is_some()
        {
            return Err("NAIP result differs from its recorded grid".into());
        }
        crate::raster::aerial::mask::select(&mut decoder, width, height)?;
        decoder.seek_to_image(0).map_err(io_error)?;
    }
    if job.kind == "raster_prepare" {
        let grid = job
            .safe_output
            .as_ref()
            .ok_or("Prepared SAFE grid is missing")?;
        if grid.width != width
            || grid.height != height
            || grid.band_count as usize != bands
            || grid.crs != crs
            || grid.bounds != bounds
            || grid.pixel_size != pixel_size
            || nodata != Some(0)
        {
            return Err("Prepared SAFE raster differs from its recorded grid".into());
        }
    }
    Ok(SourceRaster {
        decoder: SourceDecoder::Tiff(Box::new(decoder)),
        width,
        height,
        crs,
        bounds,
        pixel_size,
        nodata: nodata.map(f64::from),
        elevation: None,
        bands,
        calibration: None,
        radar: None,
        quality: None,

        landsat_quality: None,
        coverage_source: None,
    })
}

fn aligned(value: f64) -> Result<i64> {
    let rounded = value.round();
    if !value.is_finite() || (value - rounded).abs() > 1e-5 || rounded.abs() > 1_000_000.0 {
        return Err(
            "Source grids are not pixel-aligned; choose rasters on the same coordinate system and resolution"
                .into(),
        );
    }
    Ok(rounded as i64)
}

#[derive(Debug)]
struct MosaicOutput {
    path: String,
    bytes: u64,
    sha256: String,
    plan: MosaicPlan,
}

#[cfg(test)]
fn write_mosaic(
    root: &Path,
    project: &Project,
    sources: &[Job],
    key: &str,
    output_id: &str,
    cancel: &CancellationToken,
    progress: Option<&UnboundedSender<(u64, &'static str)>>,
) -> Result<MosaicOutput> {
    write_mosaic_with_selection(
        root, project, sources, key, output_id, cancel, progress, None,
    )
}

#[allow(clippy::too_many_arguments)]
fn write_mosaic_with_selection(
    root: &Path,
    project: &Project,
    sources: &[Job],
    key: &str,
    output_id: &str,
    cancel: &CancellationToken,
    progress: Option<&UnboundedSender<(u64, &'static str)>>,
    selection: Option<&vegetation::Spec>,
) -> Result<MosaicOutput> {
    let primary_count = project.scenes.len();
    if sources.is_empty()
        || sources.len()
            != primary_count
                * if selection.is_some() {
                    4
                } else if key == "qa_radsat" {
                    2
                } else {
                    1
                }
    {
        return Err("Project source count changed".into());
    }
    let mut rasters = Vec::with_capacity(primary_count);
    let report = |completed, stage| {
        if let Some(progress) = progress {
            let _ = progress.send((completed, stage));
        }
    };
    for (index, source) in sources[..primary_count].iter().enumerate() {
        let mut raster = source_raster(root, source, key, cancel)?;
        if key == "qa_radsat" {
            let paired_job = &sources[primary_count + index];
            landsat::validate_pair(source, paired_job)?;
            let paired = source_raster(root, paired_job, "qa_pixel", cancel)?;
            landsat::validate_grid_pair(&raster, &paired)?;
            raster.coverage_source = Some(Box::new(paired));
        }
        rasters.push(raster);
        report(
            (index as u64 + 1) * 100 / sources.len() as u64,
            "Checking downloaded sources",
        );
    }
    let first = &rasters[0];
    let crs = first.crs.clone();
    let [dx, dy] = first.pixel_size;
    let bands = first.bands;
    let mut union = first.bounds;
    for raster in &rasters {
        if raster.crs != crs
            || raster.bands != bands
            || raster.calibration != first.calibration
            || raster.elevation != first.elevation
            || raster.radar != first.radar
            || raster.quality != first.quality
            || raster.landsat_quality != first.landsat_quality
            || (raster.pixel_size[0] - dx).abs() > 1e-8
            || (raster.pixel_size[1] - dy).abs() > 1e-8
        {
            return Err("Sources use different coordinate systems, pixel sizes or band types; select one aligned grid per mosaic".into());
        }
        aligned((raster.bounds[0] - first.bounds[0]) / dx)?;
        aligned((first.bounds[3] - raster.bounds[3]) / dy)?;
        union[0] = union[0].min(raster.bounds[0]);
        union[1] = union[1].min(raster.bounds[1]);
        union[2] = union[2].max(raster.bounds[2]);
        union[3] = union[3].max(raster.bounds[3]);
    }
    let projected = crop::projected_envelope(project.bounds, &crs)?;
    let clipped = [
        projected[0].max(union[0]),
        projected[1].max(union[1]),
        projected[2].min(union[2]),
        projected[3].min(union[3]),
    ];
    if clipped[0] >= clipped[2] || clipped[1] >= clipped[3] {
        return Err("Project area does not intersect the downloaded source rasters".into());
    }
    // Floating subtraction of degree coordinates must not add an empty row or
    // column at an exactly aligned shared edge. Snap only round-off, then round out.
    let snap = |v: f64| {
        if (v - v.round()).abs() < 1e-7 {
            v.round()
        } else {
            v
        }
    };
    let x0 = snap((clipped[0] - first.bounds[0]) / dx).floor() as i64;
    let x1 = snap((clipped[2] - first.bounds[0]) / dx).ceil() as i64;
    let y0 = snap((first.bounds[3] - clipped[3]) / dy).floor() as i64;
    let y1 = snap((first.bounds[3] - clipped[1]) / dy).ceil() as i64;
    let width = u32::try_from(x1 - x0).map_err(io_error)?;
    let height = u32::try_from(y1 - y0).map_err(io_error)?;
    let count = width as u64 * height as u64;
    if count == 0 {
        return Err("Project output has no pixels".into());
    }
    let left = first.bounds[0] + x0 as f64 * dx;
    let top = first.bounds[3] - y0 as f64 * dy;
    let bounds = [
        left,
        top - height as f64 * dy,
        left + width as f64 * dx,
        top,
    ];
    let plan = MosaicPlan {
        width,
        height,
        band_count: bands as u8,
        crs,
        bounds,
        pixel_size: [dx, dy],
        source_count: primary_count,
        masked_pixels: 0,
        covered_pixels: 0,
        overlap_policy: if first.landsat_quality.is_some() { crate::raster::landsat_quality::MOSAIC_POLICY } else if first.quality.is_some() { "newest non-fill composite wins; whole unsigned bit field retained; zero and highest bit are valid; gaps and polygon masks retain product fill; no quality ranking or bit merging" } else if key == "srtm" { "last ordered valid tile wins; shared edges occupy one row or column; gaps and polygon masks are -32768; zero and negative heights are valid" } else if key == "elevation" { "last ordered valid tile wins; gaps and polygon masks are NaN NoData; zero heights are valid" } else if key == "aerial" { "newest ordered scene wins; independent internal coverage mask; zero RGB and NIR are valid" } else { "newest non-nodata scene wins; unfilled pixels retain product NoData" }.into(),
        calibration: first.calibration.clone(),
        elevation: first.elevation.clone(),
        aerial: (key == "aerial").then(crate::raster::aerial::masked_display),
        radar: first.radar.clone(),
        quality: first.quality.clone(),
        landsat_quality: first.landsat_quality.clone(),
        vi_quality: selection.map(vegetation::empty).transpose()?,
        vi_index: selection.map(|_| key.into()),
    };
    let mut plan = plan;
    if selection.is_some() {
        plan.overlap_policy = vegetation::SELECTION.into();
        for group in 1..4 {
            for i in 0..primary_count {
                let job = &sources[group * primary_count + i];
                let paired = source_raster(root, job, &job.asset_key, cancel)?;
                landsat::validate_grid_pair(&rasters[i], &paired)?;
                rasters.push(paired);
            }
        }
    }
    let output = stream::encode_mosaic(
        root,
        output_id,
        plan,
        &mut rasters,
        project.geometry.as_ref(),
        cancel,
        progress,
    )?;
    if let Some(selection) = selection {
        vegetation::validate_result(selection, key, &output.plan)?;
    }
    report(MOSAIC_PROGRESS_TOTAL, "Writing and checking GeoTIFF");
    Ok(output)
}

#[derive(Clone, Copy)]
struct MosaicGrid {
    origin: [f64; 2],
    dimensions: [u32; 2],
    pixel_size: [f64; 2],
}

fn copy_source(
    raster: &mut SourceRaster,
    grid: &MosaicGrid,
    output: &mut [u8],
    covered: &mut [bool],
    cancel: &CancellationToken,
    deadline: Instant,
) -> Result<()> {
    if raster.landsat_quality.is_some() {
        return landsat::copy_source(raster, grid, output, covered, cancel, deadline);
    }
    let MosaicGrid {
        origin: [left, top],
        dimensions: [width, height],
        pixel_size: [dx, dy],
    } = *grid;
    let offset_x = aligned((raster.bounds[0] - left) / dx)?;
    let offset_y = aligned((top - raster.bounds[3]) / dy)?;
    let (chunk_w, chunk_h) = raster.decoder.chunk_dimensions();
    if chunk_w == 0 || chunk_h == 0 {
        return Err("Source GeoTIFF has invalid chunk dimensions".into());
    }
    let columns = raster.width.div_ceil(chunk_w);
    let rows = raster.height.div_ceil(chunk_h);
    let chunks = match raster.decoder.get_chunk_type() {
        ChunkType::Tile => raster.decoder.tile_count().map_err(io_error)?,
        ChunkType::Strip => raster.decoder.strip_count().map_err(io_error)?,
    };
    if columns.checked_mul(rows) != Some(chunks) {
        return Err("Planar or inconsistent GeoTIFF chunks are unsupported".into());
    }
    let first_column = (-offset_x).max(0).min(raster.width as i64) as u32 / chunk_w;
    let last_column = (width as i64 - offset_x).clamp(0, raster.width as i64) as u32;
    let first_row = (-offset_y).max(0).min(raster.height as i64) as u32 / chunk_h;
    let last_row = (height as i64 - offset_y).clamp(0, raster.height as i64) as u32;
    for cy in first_row..last_row.div_ceil(chunk_h) {
        check_cancel(Some(cancel))?;
        if Instant::now() > deadline {
            return Err("Mosaic decoding exceeded its time limit".into());
        }
        for cx in first_column..last_column.div_ceil(chunk_w) {
            check_cancel(Some(cancel))?;
            let chunk_x = cx * chunk_w;
            let chunk_y = cy * chunk_h;
            let index = cy * columns + cx;
            let (actual_w, actual_h) = raster.decoder.chunk_data_dimensions(index);
            let out_x = offset_x + chunk_x as i64;
            let out_y = offset_y + chunk_y as i64;
            if out_x >= width as i64
                || out_y >= height as i64
                || out_x + actual_w as i64 <= 0
                || out_y + actual_h as i64 <= 0
            {
                continue;
            }
            let decoded = if raster.bands == 4 {
                crate::raster::aerial::read_chunk(raster.decoder.as_tiff_mut()?, index)?
            } else {
                decoded_bytes(
                    raster.decoder.read_chunk(index).map_err(io_error)?,
                    raster.calibration.as_ref(),
                    raster.elevation.as_ref(),
                    raster.radar.is_some(),
                    raster.quality.as_ref(),
                )?
            };
            let signed_height = crate::raster::srtm::signed(raster.elevation.as_ref());
            let bytes_per_sample = if let Some(profile) = &raster.quality {
                usize::from(profile.bits / 8)
            } else if signed_height {
                2
            } else if raster.elevation.is_some() || raster.radar.is_some() {
                4
            } else if let Some(profile) = &raster.calibration {
                usize::from(profile.bits() / 8)
            } else {
                1
            };
            let expected = actual_w as usize * actual_h as usize * raster.bands * bytes_per_sample;
            if decoded.len() != expected {
                return Err("Decoded source chunk has too few samples".into());
            }
            for row in 0..actual_h {
                if row % 16 == 0 {
                    check_cancel(Some(cancel))?;
                }
                let target_y = out_y + row as i64;
                if target_y < 0 || target_y >= height as i64 {
                    continue;
                }
                for col in 0..actual_w {
                    let target_x = out_x + col as i64;
                    if target_x < 0 || target_x >= width as i64 {
                        continue;
                    }
                    let source_index = (row as usize * actual_w as usize + col as usize)
                        * raster.bands
                        * bytes_per_sample;
                    let sample =
                        &decoded[source_index..source_index + raster.bands * bytes_per_sample];
                    if let Some(profile) = &raster.quality {
                        let value = if profile.bits == 32 {
                            u32::from_ne_bytes(sample.try_into().map_err(io_error)?)
                        } else {
                            u32::from(u16::from_ne_bytes(sample.try_into().map_err(io_error)?))
                        };
                        if value != profile.nodata {
                            let target = target_y as usize * width as usize + target_x as usize;
                            output[target * bytes_per_sample..(target + 1) * bytes_per_sample]
                                .copy_from_slice(sample);
                            covered[target] = true;
                        }
                        continue;
                    }
                    if signed_height {
                        let value = i16::from_ne_bytes([sample[0], sample[1]]);
                        if value != i16::MIN {
                            let target = target_y as usize * width as usize + target_x as usize;
                            output[target * 2..(target + 1) * 2].copy_from_slice(sample);
                            covered[target] = true;
                        }
                        continue;
                    }
                    if raster.elevation.is_some() {
                        let value = f32::from_ne_bytes(sample.try_into().map_err(io_error)?);
                        if value.is_infinite()
                            || value.is_nan() && !raster.nodata.is_some_and(f64::is_nan)
                        {
                            return Err(
                                "Elevation source contains an undefined nonfinite sample".into()
                            );
                        }
                        if crate::raster::elevation::is_no_data(
                            value,
                            raster.nodata.map(|v| v as f32),
                        ) {
                            continue;
                        }
                        let target = target_y as usize * width as usize + target_x as usize;
                        output[target * 4..(target + 1) * 4].copy_from_slice(sample);
                        covered[target] = true;
                        continue;
                    }
                    if raster.radar.is_some() {
                        let value = f32::from_ne_bytes(sample.try_into().map_err(io_error)?);
                        if !value.is_finite() || value < 0.0 && value != -32768.0 {
                            return Err("RTC source contains invalid gamma0 samples".into());
                        }
                        if value == -32768.0 {
                            continue;
                        }
                        let target = target_y as usize * width as usize + target_x as usize;
                        output[target * 4..(target + 1) * 4].copy_from_slice(sample);
                        covered[target] = true;
                        continue;
                    }
                    let value = if let Some(profile) = &raster.calibration {
                        if profile.bits() == 8 {
                            i32::from(sample[0] as i8)
                        } else if profile.signed {
                            i16::from_ne_bytes([sample[0], sample[1]]) as i32
                        } else {
                            u16::from_ne_bytes([sample[0], sample[1]]) as i32
                        }
                    } else {
                        sample[0] as i32
                    };
                    if raster.calibration.is_none() && raster.bands == 1 && value > 11 {
                        return Err(
                            "SCL source contains a value outside the 0–11 classification range"
                                .into(),
                        );
                    }
                    if raster.nodata.is_some_and(|nodata| {
                        if raster.calibration.is_some() {
                            f64::from(value) == nodata
                        } else {
                            sample.iter().all(|v| f64::from(*v) == nodata)
                        }
                    }) || (raster.calibration.is_none() && raster.bands == 1 && value == 0)
                    {
                        continue;
                    }
                    let target = target_y as usize * width as usize + target_x as usize;
                    output[target * raster.bands * bytes_per_sample
                        ..(target + 1) * raster.bands * bytes_per_sample]
                        .copy_from_slice(sample);
                    covered[target] = true;
                }
            }
        }
    }
    Ok(())
}

fn decoded_bytes(
    result: DecodingResult,
    calibration: Option<&crate::raster::reflectance::Profile>,
    elevation: Option<&crate::raster::elevation::Profile>,
    radar: bool,
    quality: Option<&crate::raster::quality::Profile>,
) -> Result<Vec<u8>> {
    // TiffEncoder writes the host's byte order; Deflate receives those exact bytes.
    if let Some(profile) = quality {
        if calibration.is_some() || elevation.is_some() || radar {
            return Err(
                "Quality samples cannot use a reflectance, elevation or radar profile".into(),
            );
        }
        return match result {
            DecodingResult::U32(data) if profile.bits == 32 => {
                Ok(data.into_iter().flat_map(u32::to_ne_bytes).collect())
            }
            DecodingResult::U16(data) if profile.bits == 16 => {
                Ok(data.into_iter().flat_map(u16::to_ne_bytes).collect())
            }
            _ => Err(
                "Decoded quality samples differ from their original unsigned bit-field type".into(),
            ),
        };
    }
    match result {
        DecodingResult::F32(data)
            if (elevation.is_some() || radar)
                && !crate::raster::srtm::signed(elevation)
                && calibration.is_none() =>
        {
            Ok(data.into_iter().flat_map(f32::to_ne_bytes).collect())
        }
        DecodingResult::U8(data) if elevation.is_none() && calibration.is_none() && !radar => {
            Ok(data)
        }
        DecodingResult::I8(data) if calibration.is_some_and(|p| p.signed && p.bits() == 8) => {
            Ok(data.into_iter().map(|v| v as u8).collect())
        }
        DecodingResult::U16(data) if calibration.is_some_and(|p| !p.signed && p.bits() == 16) => {
            Ok(data.into_iter().flat_map(u16::to_ne_bytes).collect())
        }
        DecodingResult::I16(data)
            if calibration.is_some_and(|p| p.signed && p.bits() == 16)
                || crate::raster::srtm::signed(elevation) =>
        {
            Ok(data.into_iter().flat_map(i16::to_ne_bytes).collect())
        }
        _ => Err("Decoded samples differ from the pinned original band type".into()),
    }
}

fn add_geo_tags<W: Write + Seek, K: tiff::encoder::TiffKind>(
    image: &mut tiff::encoder::DirectoryEncoder<'_, W, K>,
    plan: &MosaicPlan,
) -> Result<()> {
    if matches!(
        plan.crs.as_str(),
        crate::providers::modis::CRS | crate::providers::viirs::hdf::CRS
    ) {
        crate::raster::reflectance::modis::write_crs(image)?;
    } else {
        let epsg = plan
            .crs
            .strip_prefix("EPSG:")
            .and_then(|value| value.parse::<u16>().ok())
            .ok_or("Invalid mosaic CRS")?;
        let keys = if plan.elevation.is_some() {
            let signed_height = crate::raster::srtm::signed(plan.elevation.as_ref());
            let vertical = if signed_height { 5773 } else { 3855 };
            vec![
                1u16,
                1,
                u16::from(signed_height),
                6,
                1024,
                0,
                1,
                2,
                1025,
                0,
                1,
                2,
                2048,
                0,
                1,
                4326,
                2054,
                0,
                1,
                9102,
                4096,
                0,
                1,
                vertical,
                4099,
                0,
                1,
                9001,
            ]
        } else {
            vec![
                1u16, 1, 0, 4, 1024, 0, 1, 1, 1025, 0, 1, 1, 3072, 0, 1, epsg, 3076, 0, 1, 9001,
            ]
        };
        image
            .write_tag(Tag::GeoKeyDirectoryTag, &keys[..])
            .map_err(io_error)?;
    }
    image
        .write_tag(
            Tag::ModelPixelScaleTag,
            &[plan.pixel_size[0], plan.pixel_size[1], 0.0][..],
        )
        .map_err(io_error)?;
    image
        .write_tag(
            Tag::ModelTiepointTag,
            &[
                0.0,
                0.0,
                0.0,
                plan.bounds[0]
                    + if plan.elevation.is_some() {
                        plan.pixel_size[0] / 2.0
                    } else {
                        0.0
                    },
                plan.bounds[3]
                    - if plan.elevation.is_some() {
                        plan.pixel_size[1] / 2.0
                    } else {
                        0.0
                    },
                0.0,
            ][..],
        )
        .map_err(io_error)?;
    let nodata = if let Some(profile) = &plan.quality {
        profile.nodata.to_string()
    } else if crate::raster::srtm::signed(plan.elevation.as_ref()) || plan.radar.is_some() {
        "-32768".into()
    } else if plan.elevation.is_some() {
        "nan".into()
    } else {
        plan.calibration
            .as_ref()
            .map_or(0, |profile| profile.nodata)
            .to_string()
    };
    if plan.aerial.is_some() {
        image
            .write_tag(Tag::ExtraSamples, &[0u16][..])
            .map_err(io_error)?;
    } else if plan.landsat_quality.is_none() {
        image
            .write_tag(Tag::GdalNodata, nodata.as_str())
            .map_err(io_error)?;
    }
    if let Some(profile) = &plan.landsat_quality {
        image
            .write_tag(
                Tag::ImageDescription,
                serde_json::to_string(profile).map_err(io_error)?.as_str(),
            )
            .map_err(io_error)?;
    }
    if let Some(profile) = &plan.radar {
        image
            .write_tag(
                Tag::Unknown(42112),
                crate::raster::radar::metadata(profile).as_str(),
            )
            .map_err(io_error)?;
    }
    if let Some(profile) = &plan.quality {
        image
            .write_tag(
                Tag::Unknown(42112),
                crate::raster::quality::metadata(profile).as_str(),
            )
            .map_err(io_error)?;
    }
    if let Some(profile) = &plan.calibration {
        let metadata = if profile.science_key.is_some() {
            crate::raster::science::metadata(profile)?
        } else {
            format!("<GDALMetadata><Item name=\"SCALE\" sample=\"0\" role=\"scale\">{}</Item><Item name=\"OFFSET\" sample=\"0\" role=\"offset\">{}</Item><Item name=\"PRODUCT\">{}</Item></GDALMetadata>", profile.scale, profile.offset, profile.product)
        };
        image
            .write_tag(Tag::Unknown(42112), metadata.as_str())
            .map_err(io_error)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::{CreateProjectRequest, ProjectAsset, ProjectScene};
    use std::{fs, io::Cursor};
    use tiff::encoder::colortype;

    pub(super) fn project(sources: &[Job], asset_key: &str) -> Project {
        let scenes = sources
            .iter()
            .enumerate()
            .map(|(index, job)| ProjectScene {
                item_id: format!("SCENE_{index}"),
                date: format!("2026-09-0{}T00:00:00Z", index + 1),
                cloud: Some(0.0),
                crs: Some("EPSG:32610".into()),
                grid_code: Some("10SEG".into()),
                bbox: [-123.001, 37.947, -122.999, 37.949],
                assets: BTreeMap::from([(
                    asset_key.into(),
                    ProjectAsset {
                        href: job.href.clone(),
                        media_type: job.media_type.clone(),
                        raster_band: None,
                    },
                )]),
            })
            .collect();
        Project {
            id: Uuid::new_v4().to_string(),
            name: "test".into(),
            bounds: [-123.001, 37.947, -122.999, 37.949],
            geometry: None,
            scenes,
            stac_items: Vec::new(),
            wcs_items: Vec::new(),
            created_at: now(),
            updated_at: now(),
        }
    }

    fn rgb_fixture(pixels: &[u8]) -> Vec<u8> {
        rgb_fixture_size(2, 2, pixels)
    }

    fn rgb_fixture_size(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut buffer)
                .unwrap()
                .with_compression(tiff::encoder::Compression::Deflate(DeflateLevel::Balanced));
            let mut image = encoder.new_image::<colortype::RGB8>(width, height).unwrap();
            let keys = [
                1u16, 1, 0, 4, 1024, 0, 1, 1, 1025, 0, 1, 1, 3072, 0, 1, 32610, 3076, 0, 1, 9001,
            ];
            image
                .encoder()
                .write_tag(Tag::GeoKeyDirectoryTag, &keys[..])
                .unwrap();
            image
                .encoder()
                .write_tag(Tag::ModelPixelScaleTag, &[20.0f64, 20.0, 0.0][..])
                .unwrap();
            image
                .encoder()
                .write_tag(
                    Tag::ModelTiepointTag,
                    &[0.0f64, 0.0, 0.0, 500000.0, 4200000.0, 0.0][..],
                )
                .unwrap();
            image.encoder().write_tag(Tag::GdalNodata, "0").unwrap();
            image.write_data(pixels).unwrap();
        }
        buffer.into_inner()
    }

    #[test]
    fn mosaics_more_than_eight_million_rgb_pixels_across_strip_boundaries() {
        let directory = tempfile::tempdir().unwrap();
        let original = [10, 20, 30].repeat(3000 * 3000);
        let mut newer = vec![0; original.len()];
        newer[512 * 3000 * 3..].copy_from_slice(&[1, 2, 3].repeat((3000 - 512) * 3000));
        let mut first = crate::raster::tests::record(
            directory.path(),
            &rgb_fixture_size(3000, 3000, &original),
        );
        let mut second =
            crate::raster::tests::record(directory.path(), &rgb_fixture_size(3000, 3000, &newer));
        first.asset_key = "visual".into();
        second.asset_key = "visual".into();
        let sources = [first, second];
        let mut project = project(&sources, "visual");
        project.bounds = [-123.1, 37.0, -122.0, 38.0];
        let root = directory.path().canonicalize().unwrap();
        let output = write_mosaic(
            &root,
            &project,
            &sources,
            "visual",
            &Uuid::new_v4().to_string(),
            &CancellationToken::new(),
            None,
        )
        .unwrap();
        assert_eq!(
            (
                output.plan.width,
                output.plan.height,
                output.plan.band_count
            ),
            (3000, 3000, 3)
        );
        assert_eq!(output.plan.pixel_size, [20.0, 20.0]);
        assert_eq!(output.plan.covered_pixels, 9_000_000);
        let mut decoded = Decoder::new(File::open(&output.path).unwrap()).unwrap();
        assert!(decoded.strip_count().unwrap() > 1);
        let mut pixels = decoded.read_image().unwrap();
        let buffer = pixels.as_buffer(0);
        let pixels = buffer.as_bytes();
        for y in [0, 511, 512, 1023, 1024, 2999] {
            let sample = &pixels[y * 3000 * 3..y * 3000 * 3 + 3];
            assert_eq!(sample, if y < 512 { &[10, 20, 30] } else { &[1, 2, 3] });
        }
        assert_eq!(
            output.sha256,
            format!("{:x}", Sha256::digest(fs::read(output.path).unwrap()))
        );
    }

    #[test]
    fn mosaics_more_than_eight_million_scl_pixels_without_changing_classes() {
        let directory = tempfile::tempdir().unwrap();
        let source = crate::raster::tests::record(
            directory.path(),
            &crate::raster::tests::fixture(3000, 3000, &vec![9; 9_000_000], 32610, false),
        );
        let sources = [source];
        let mut project = project(&sources, "scl");
        project.bounds = [-123.1, 37.0, -122.0, 38.0];
        let root = directory.path().canonicalize().unwrap();
        let output = write_mosaic(
            &root,
            &project,
            &sources,
            "scl",
            &Uuid::new_v4().to_string(),
            &CancellationToken::new(),
            None,
        )
        .unwrap();
        assert_eq!(
            (
                output.plan.width,
                output.plan.height,
                output.plan.band_count
            ),
            (3000, 3000, 1)
        );
        assert_eq!(output.plan.covered_pixels, 9_000_000);
        let mut decoder = Decoder::new(File::open(output.path).unwrap()).unwrap();
        for index in 0..decoder.strip_count().unwrap() {
            assert!(decoder
                .read_chunk(index)
                .unwrap()
                .as_buffer(0)
                .as_bytes()
                .iter()
                .all(|value| *value == 9));
        }
    }

    #[test]
    fn cancellation_between_mosaic_blocks_removes_unfinished_output() {
        let directory = tempfile::tempdir().unwrap();
        let source = crate::raster::tests::record(
            directory.path(),
            &crate::raster::tests::fixture(64, 4096, &vec![5; 64 * 4096], 32610, false),
        );
        let sources = [source];
        let mut project = project(&sources, "scl");
        project.bounds = [-123.1, 37.0, -122.0, 38.0];
        let root = directory.path().canonicalize().unwrap();
        let id = Uuid::new_v4().to_string();
        let cancel = CancellationToken::new();
        let token = cancel.clone();
        let observed = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let rows = observed.clone();
        let _observation = stream::observe_blocks(Box::new(move |completed| {
            rows.store(completed, std::sync::atomic::Ordering::SeqCst);
            token.cancel();
        }));
        assert!(
            write_mosaic(&root, &project, &sources, "scl", &id, &cancel, None)
                .err()
                .unwrap()
                .contains("cancel")
        );
        assert_eq!(observed.load(std::sync::atomic::Ordering::SeqCst), 512);
        assert!(!root.join("assets").join(format!("{id}.tif")).exists());
        assert!(fs::read_dir(root.join("assets"))
            .unwrap()
            .all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".part")));
    }

    #[test]
    fn polygon_mosaic_allows_fully_masked_blocks_before_covered_blocks() {
        let directory = tempfile::tempdir().unwrap();
        let source = crate::raster::tests::record(
            directory.path(),
            &crate::raster::tests::fixture(64, 1536, &vec![5; 64 * 1536], 32610, false),
        );
        let sources = [source];
        let mut project = project(&sources, "scl");
        project.bounds = [-123.1, 37.0, -122.0, 38.0];
        project.geometry = Some(crop::PolygonGeometry::Polygon(vec![vec![
            [-123.01, 37.68],
            [-122.97, 37.68],
            [-122.97, 37.73],
            [-123.01, 37.73],
            [-123.01, 37.68],
        ]]));
        let root = directory.path().canonicalize().unwrap();
        let output = write_mosaic(
            &root,
            &project,
            &sources,
            "scl",
            &Uuid::new_v4().to_string(),
            &CancellationToken::new(),
            None,
        )
        .unwrap();
        assert!(output.plan.masked_pixels > 0);
        assert!(output.plan.covered_pixels > 0);
        let mut decoder = Decoder::new(File::open(output.path).unwrap()).unwrap();
        assert!(decoder
            .read_chunk(0)
            .unwrap()
            .as_buffer(0)
            .as_bytes()
            .iter()
            .all(|value| *value == 0));
        assert!((1..decoder.strip_count().unwrap()).any(|index| decoder
            .read_chunk(index)
            .unwrap()
            .as_buffer(0)
            .as_bytes()
            .contains(&5)));
    }

    #[test]
    fn mosaics_scl_pixels_and_preserves_older_values_under_newer_nodata() {
        let directory = tempfile::tempdir().unwrap();
        let first = crate::raster::tests::record(
            directory.path(),
            &crate::raster::tests::fixture(2, 2, &[1, 2, 3, 4], 32610, false),
        );
        let second = crate::raster::tests::record(
            directory.path(),
            &crate::raster::tests::fixture(2, 2, &[0, 5, 0, 6], 32610, false),
        );
        let sources = [first, second];
        let root = directory.path().canonicalize().unwrap();
        let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel();
        let output = write_mosaic(
            &root,
            &project(&sources, "scl"),
            &sources,
            "scl",
            &Uuid::new_v4().to_string(),
            &CancellationToken::new(),
            Some(&progress_tx),
        )
        .unwrap();
        let mut progress = Vec::new();
        while let Ok(update) = progress_rx.try_recv() {
            progress.push(update);
        }
        assert_eq!(progress.first(), Some(&(50, "Checking downloaded sources")));
        assert_eq!(
            progress.last(),
            Some(&(1000, "Writing and checking GeoTIFF"))
        );
        assert_eq!(output.plan.band_count, 1);
        assert_eq!((output.plan.width, output.plan.height), (2, 2));
        assert_eq!(output.plan.covered_pixels, 4);
        let mut decoded = Decoder::new(File::open(&output.path).unwrap()).unwrap();
        assert_eq!(
            decoded.read_image().unwrap().as_buffer(0).as_bytes(),
            &[1, 5, 3, 6]
        );
    }

    #[test]
    fn mosaics_true_color_pixels_without_interpreting_rgb_as_scl() {
        let directory = tempfile::tempdir().unwrap();
        let mut first = crate::raster::tests::record(
            directory.path(),
            &rgb_fixture(&[10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120]),
        );
        let mut second = crate::raster::tests::record(
            directory.path(),
            &rgb_fixture(&[0, 0, 0, 1, 2, 3, 0, 0, 0, 4, 5, 6]),
        );
        first.asset_key = "visual".into();
        second.asset_key = "visual".into();
        let sources = [first, second];
        let root = directory.path().canonicalize().unwrap();
        let output = write_mosaic(
            &root,
            &project(&sources, "visual"),
            &sources,
            "visual",
            &Uuid::new_v4().to_string(),
            &CancellationToken::new(),
            None,
        )
        .unwrap();
        assert_eq!(output.plan.band_count, 3);
        let mut decoded = Decoder::new(File::open(&output.path).unwrap()).unwrap();
        assert_eq!(
            decoded.read_image().unwrap().as_buffer(0).as_bytes(),
            &[10, 20, 30, 1, 2, 3, 70, 80, 90, 4, 5, 6]
        );
    }

    #[test]
    fn mosaics_tiled_deflate_rgb_like_a_real_sentinel_cog() {
        let directory = tempfile::tempdir().unwrap();
        let mut source = crate::raster::tests::record(
            directory.path(),
            include_bytes!("../fixtures/tiled-rgb.tif"),
        );
        source.asset_key = "visual".into();
        let mut project = project(&[source.clone()], "visual");
        project.bounds = [-123.01, 37.94, -122.99, 37.95];
        let root = directory.path().canonicalize().unwrap();
        let output = write_mosaic(
            &root,
            &project,
            &[source],
            "visual",
            &Uuid::new_v4().to_string(),
            &CancellationToken::new(),
            None,
        )
        .unwrap();
        assert_eq!(
            (
                output.plan.width,
                output.plan.height,
                output.plan.band_count
            ),
            (32, 32, 3)
        );
        let mut original = Decoder::new(Cursor::new(include_bytes!("../fixtures/tiled-rgb.tif")))
            .unwrap()
            .read_image()
            .unwrap();
        let mut derived = Decoder::new(File::open(&output.path).unwrap())
            .unwrap()
            .read_image()
            .unwrap();
        assert_eq!(
            original.as_buffer(0).as_bytes(),
            derived.as_buffer(0).as_bytes()
        );
    }

    #[test]
    fn rgb_project_polygon_masks_pixels_outside_the_saved_boundary() {
        let directory = tempfile::tempdir().unwrap();
        let mut source = crate::raster::tests::record(
            directory.path(),
            include_bytes!("../fixtures/tiled-rgb.tif"),
        );
        source.asset_key = "visual".into();
        let mut project = project(&[source.clone()], "visual");
        project.bounds = [-123.01, 37.94, -122.99, 37.95];
        project.geometry = Some(crop::PolygonGeometry::Polygon(vec![vec![
            [-123.001, 37.944],
            [-122.998, 37.944],
            [-122.998, 37.949],
            [-123.001, 37.949],
            [-123.001, 37.944],
        ]]));
        let root = directory.path().canonicalize().unwrap();
        let output = write_mosaic(
            &root,
            &project,
            &[source],
            "visual",
            &Uuid::new_v4().to_string(),
            &CancellationToken::new(),
            None,
        )
        .unwrap();
        assert!(output.plan.masked_pixels > 0);
        assert!(output.plan.covered_pixels > 0);
        let mut decoder = Decoder::new(File::open(output.path).unwrap()).unwrap();
        let mut pixels = decoder.read_image().unwrap();
        let buffer = pixels.as_buffer(0);
        let bytes = buffer.as_bytes();
        assert_eq!(&bytes[0..3], &[1, 2, 3]);
        assert_eq!(&bytes[31 * 3..32 * 3], &[0, 0, 0]);
    }

    #[tokio::test]
    async fn shutdown_drains_queued_mosaic_and_blocks_project_enqueues() {
        let directory = tempfile::tempdir().unwrap();
        let manager = JobManager::open(directory.path()).await.unwrap();
        let bytes = crate::raster::tests::fixture(2, 2, &[1, 2, 3, 4], 32610, false);
        let mut source = crate::raster::tests::record(manager.storage_root(), &bytes);
        source.item_id = "SCENE_0".into();
        source.href = format!(
            "https://{}/sentinel-s2-l2a-cogs/10/S/EG/2026/9/SCENE_0/SCL.tif",
            crate::SOURCE_HOST
        );
        let request = project(&[source.clone()], "scl");
        {
            let mut store = manager.inner.store.lock().await;
            store.jobs.insert(source.id.clone(), source.clone());
            manager.persist(&store.jobs).await.unwrap();
        }
        let project = manager
            .create_project(CreateProjectRequest {
                name: request.name,
                bounds: request.bounds,
                geometry: None,
                scenes: request.scenes,
            })
            .await
            .unwrap();
        let permit = manager
            .inner
            .raster_permits
            .clone()
            .acquire_owned()
            .await
            .unwrap();
        let mosaic = manager
            .run_project_mosaic(&project.id, "scl")
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(3), manager.shutdown())
            .await
            .unwrap()
            .unwrap();
        let mosaic = manager.wait(&mosaic.id).await.unwrap();
        assert_eq!(mosaic.status, JobStatus::Interrupted);
        assert!(mosaic.mosaic_output.is_none() && mosaic.manifest_path.is_none());
        assert!(!directory
            .path()
            .join("assets")
            .join(format!("{}.tif", mosaic.id))
            .exists());
        assert_eq!(std::fs::read(source.output_path.unwrap()).unwrap(), bytes);
        assert!(manager
            .run_project_mosaic(&project.id, "scl")
            .await
            .unwrap_err()
            .contains("shutting down"));
        assert!(manager
            .enqueue_project(&project.id, "scl")
            .await
            .unwrap_err()
            .contains("shutting down"));
        drop(permit);
    }

    #[tokio::test]
    async fn saved_project_reuses_both_download_types_and_persists_both_mosaics() {
        let directory = tempfile::tempdir().unwrap();
        let manager = JobManager::open(directory.path()).await.unwrap();
        let scl_pixels = [&[1u8, 2, 3, 4][..], &[0u8, 5, 0, 6][..]];
        let rgb_pixels = [
            &[10u8, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120][..],
            &[0u8, 0, 0, 1, 2, 3, 0, 0, 0, 4, 5, 6][..],
        ];
        let mut jobs = Vec::new();
        let mut scenes = Vec::new();
        for index in 0..2 {
            let item_id = format!("S2_TEST_{index}");
            let mut assets = BTreeMap::new();
            for key in ["scl", "visual"] {
                let bytes = if key == "scl" {
                    crate::raster::tests::fixture(2, 2, scl_pixels[index], 32610, false)
                } else {
                    rgb_fixture(rgb_pixels[index])
                };
                let mut job = crate::raster::tests::record(manager.storage_root(), &bytes);
                job.item_id = item_id.clone();
                job.asset_key = key.into();
                job.href = format!(
                    "https://{}/sentinel-s2-l2a-cogs/10/S/EG/2026/9/{}/{}.tif",
                    crate::SOURCE_HOST,
                    item_id,
                    if key == "scl" { "SCL" } else { "TCI" }
                );
                assets.insert(
                    key.into(),
                    ProjectAsset {
                        href: job.href.clone(),
                        media_type: job.media_type.clone(),
                        raster_band: None,
                    },
                );
                jobs.push(job);
            }
            scenes.push(ProjectScene {
                item_id,
                date: format!("2026-09-0{}T00:00:00Z", index + 1),
                cloud: Some(0.0),
                crs: Some("EPSG:32610".into()),
                grid_code: None,
                bbox: [-123.001, 37.947, -122.999, 37.949],
                assets,
            });
        }
        {
            let mut store = manager.inner.store.lock().await;
            for job in jobs {
                store.jobs.insert(job.id.clone(), job);
            }
            manager.persist(&store.jobs).await.unwrap();
        }
        let project = manager
            .create_project(CreateProjectRequest {
                name: "Two source types".into(),
                bounds: [-123.001, 37.947, -122.999, 37.949],
                geometry: None,
                scenes,
            })
            .await
            .unwrap();
        assert_eq!(
            manager
                .enqueue_project(&project.id, "scl")
                .await
                .unwrap()
                .jobs
                .len(),
            2
        );
        assert_eq!(
            manager
                .enqueue_project(&project.id, "visual")
                .await
                .unwrap()
                .jobs
                .len(),
            2
        );
        assert!(manager.inner.store.lock().await.active.is_empty());
        let scl = manager
            .run_project_mosaic(&project.id, "scl")
            .await
            .unwrap();
        let visual = manager
            .run_project_mosaic(&project.id, "visual")
            .await
            .unwrap();
        assert_eq!(
            manager.wait(&scl.id).await.unwrap().status,
            JobStatus::Succeeded
        );
        assert_eq!(
            manager.wait(&visual.id).await.unwrap().status,
            JobStatus::Succeeded
        );
        assert_eq!(
            manager.derived_bytes(&visual.id).await.unwrap().1[0..4],
            [73, 73, 42, 0]
        );
        let (_, metadata) = manager.mosaic_metadata_bytes(&visual.id).await.unwrap();
        let manifest: serde_json::Value = serde_json::from_slice(&metadata).unwrap();
        assert_eq!(manifest["sources"].as_array().unwrap().len(), 2);
        assert_eq!(manifest["project"]["id"], project.id);
        drop(manager);
        let reopened = JobManager::open(directory.path()).await.unwrap();
        assert_eq!(reopened.list_projects().await.len(), 1);
        assert_eq!(
            reopened.get(&scl.id).await.unwrap().status,
            JobStatus::Succeeded
        );
        assert_eq!(
            reopened.get(&visual.id).await.unwrap().status,
            JobStatus::Succeeded
        );
    }
}
