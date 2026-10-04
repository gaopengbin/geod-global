//! Managed HDF-EOS -> original Int16 bands. Never resample or apply QA masks.
use super::hdf::{self, ScienceSummary, CRS, EDGE, NODATA, SCALE};
use crate::{active, io_error, now, raster::check_cancel, Job, JobManager, JobStatus, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek, Write},
    path::Path,
};
use tiff::{
    decoder::{Decoder, DecodingResult},
    encoder::{colortype, compression::DeflateLevel, Compression, TiffEncoder},
    tags::Tag,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ViirsSpec {
    pub source_job_id: String,
    pub source_sha256: String,
    pub science: ScienceSummary,
}

pub(crate) fn validate_stored(job: &Job) -> Result<()> {
    let spec = job
        .viirs_prepare
        .as_ref()
        .ok_or("VIIRS preparation has no source pin")?;
    spec.science.validate(&job.item_id, &spec.source_sha256)?;
    let url = crate::providers::asset_url(&job.href)?;
    if job.kind != "raster_prepare"
        || !matches!(job.asset_key.as_str(), "red" | "green" | "blue")
        || crate::extension(&job.media_type)? != "tif"
        || job.safe.is_some()
        || job.safe_output.is_some()
        || job.viirs_science.is_some()
        || job.parent_id.as_deref() != Some(&spec.source_job_id)
        || Uuid::parse_str(&spec.source_job_id)
            .ok()
            .map(|id| id.to_string())
            .as_deref()
            != Some(&spec.source_job_id)
        || !crate::providers::matches_item(&url, &job.item_id, "viirs")
    {
        return Err("VIIRS prepared band has invalid original-product provenance".into());
    }
    Ok(())
}

pub(crate) fn validate_source(job: &Job, jobs: &BTreeMap<String, Job>) -> Result<Job> {
    validate_stored(job)?;
    let spec = job.viirs_prepare.as_ref().unwrap();
    let source = jobs
        .get(&spec.source_job_id)
        .ok_or("Original VIIRS job is missing")?;
    if source.kind != "download"
        || source.status != JobStatus::Succeeded
        || source.asset_key != "viirs"
        || source.item_id != job.item_id
        || source.href != job.href
        || crate::extension(&source.media_type)? != "h5"
        || source.sha256.as_deref() != Some(&spec.source_sha256)
        || source.viirs_science.as_ref() != Some(&spec.science)
    {
        return Err(
            "Original VIIRS source is missing, changed or has no checked science layers".into(),
        );
    }
    Ok(source.clone())
}

pub(crate) fn scene_source<'a>(
    scene: &crate::projects::ProjectScene,
    jobs: &'a BTreeMap<String, Job>,
    key: &str,
) -> Option<&'a Job> {
    let asset = scene.assets.get("viirs")?;
    jobs.values()
        .filter(|j| {
            j.kind == "raster_prepare"
                && j.status == JobStatus::Succeeded
                && j.asset_key == key
                && j.item_id == scene.item_id
                && j.href == asset.href
                && j.sha256.is_some()
                && validate_source(j, jobs).is_ok()
        })
        .max_by_key(|j| &j.updated_at)
}

impl JobManager {
    pub(crate) async fn prepare_viirs_project(
        &self,
        id: &str,
        key: &str,
    ) -> Result<crate::ProjectDownloads> {
        if !matches!(key, "red" | "green" | "blue") {
            return Err("Choose VIIRS M5, M4 or M3".into());
        }
        let project = self
            .inner
            .projects
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or("Unknown project")?;
        let mut store = self.inner.store.lock().await;
        store.accepting_jobs()?;
        let mut output = Vec::new();
        let mut added = Vec::new();
        // Resolve every checked original before adding any job in this batch.
        for scene in &project.scenes {
            let asset = scene
                .assets
                .get("viirs")
                .ok_or("This scene has no VIIRS original")?;
            let source = store
                .jobs
                .values()
                .filter(|j| {
                    j.kind == "download"
                        && j.status == JobStatus::Succeeded
                        && j.asset_key == "viirs"
                        && j.item_id == scene.item_id
                        && j.href == asset.href
                        && j.viirs_science.is_some()
                })
                .max_by_key(|j| &j.updated_at)
                .ok_or("Download and verify every VIIRS original before preparing bands")?;
            let science = source
                .viirs_science
                .clone()
                .ok_or("VIIRS original lacks science validation; download it again")?;
            let hash = source
                .sha256
                .clone()
                .ok_or("Original VIIRS source has no checksum")?;
            science.validate(&source.item_id, &hash)?;
            if let Some(existing) = store.jobs.values().find(|j| {
                j.kind == "raster_prepare"
                    && j.asset_key == key
                    && j.parent_id.as_deref() == Some(&source.id)
                    && (active(&j.status) || j.status == JobStatus::Succeeded)
                    && validate_source(j, &store.jobs).is_ok()
            }) {
                output.push(existing.clone());
                continue;
            }
            let name = science
                .bands
                .iter()
                .find(|b| b.band == key)
                .unwrap()
                .dataset
                .rsplit('/')
                .next()
                .unwrap();
            let mut job = crate::new_download_job(crate::CreateJobRequest {
                item_id: source.item_id.clone(),
                asset_key: key.into(),
                href: source.href.clone(),
                media_type: "image/tiff".into(),
                title: Some(format!("{} · {name}", source.item_id)),
            });
            job.kind = "raster_prepare".into();
            job.parent_id = Some(source.id.clone());
            job.viirs_prepare = Some(ViirsSpec {
                source_job_id: source.id.clone(),
                source_sha256: hash,
                science,
            });
            job.validation =
                "Pending pinned VIIRS HDF5, original Int16 samples and GeoTIFF grid validation"
                    .into();
            validate_source(&job, &store.jobs)?;
            output.push(job.clone());
            added.push(job);
        }
        if store.active.len() + added.len() > 64 {
            return Err("The local queue is full (64 jobs)".into());
        }
        for job in &added {
            store.jobs.insert(job.id.clone(), job.clone());
        }
        if let Err(error) = self.persist(&store.jobs).await {
            for job in &added {
                store.jobs.remove(&job.id);
            }
            return Err(error);
        }
        for job in added {
            let token = CancellationToken::new();
            store.active.insert(job.id.clone(), token.clone());
            self.spawn(job.id, token);
        }
        Ok(crate::ProjectDownloads {
            project_id: id.into(),
            asset_key: key.into(),
            jobs: output,
        })
    }

    pub(crate) async fn process_viirs(&self, id: &str, token: &CancellationToken) -> Result<()> {
        let permit = tokio::select! { _ = token.cancelled() => return Err("VIIRS preparation cancelled".into()),
        p = self.inner.raster_permits.clone().acquire_owned() => p.map_err(io_error)? };
        let job = self.get(id).await.ok_or("Unknown preparation job")?;
        let source = {
            let store = self.inner.store.lock().await;
            validate_source(&job, &store.jobs)?
        };
        {
            let mut store = self.inner.store.lock().await;
            let record = store.jobs.get_mut(id).ok_or("Unknown preparation job")?;
            check_cancel(Some(token))?;
            record.status = JobStatus::Running;
            record.total_bytes = Some(1000);
            record.validation =
                "Reading pinned VIIRS HDF5 and retaining original scientific DN".into();
            record.updated_at = now();
            self.persist(&store.jobs).await?;
        }
        let root = self.inner.root.clone();
        let cancellation = token.clone();
        let worker_job = job.clone();
        let (bytes, hash) = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            convert(&root, &worker_job, &source, &cancellation)
        })
        .await
        .map_err(io_error)??;
        let mut store = self.inner.store.lock().await;
        let record = store.jobs.get_mut(id).ok_or("Unknown preparation job")?;
        if token.is_cancelled() || !active(&record.status) {
            return Err("VIIRS preparation cancelled".into());
        }
        let before = record.clone();
        record.status = JobStatus::Succeeded;
        record.bytes_downloaded = bytes;
        record.total_bytes = Some(bytes);
        record.sha256 = Some(hash);
        record.output_path = Some(
            self.inner
                .root
                .join("assets")
                .join(format!("{id}.tif"))
                .to_string_lossy()
                .into_owned(),
        );
        record.manifest_path = Some(
            self.inner
                .root
                .join("assets")
                .join(format!("{id}.metadata.json"))
                .to_string_lossy()
                .into_owned(),
        );
        record.updated_at = now();
        record.error = None;
        record.validation = "Original VIIRS SHA-256 and embedded science grid checked; all prepared Int16 DN read back unchanged; reflectance calibration retained; no QA mask applied".into();
        if let Err(error) = self.persist(&store.jobs).await {
            store.jobs.insert(id.into(), before);
            return Err(error);
        }
        Ok(())
    }
}

pub(crate) fn profile() -> crate::raster::reflectance::Profile {
    crate::raster::reflectance::Profile {
        product: "viirs-09a1-v002".into(),
        signed: true,
        scale: SCALE,
        offset: 0.0,
        nodata: i32::from(NODATA),
        sample_bits: None,
        science_key: None,
        calendar_year: None,
    }
}

pub(crate) fn grid(header: &crate::raster::reflectance::Header, job: &Job) -> Result<()> {
    validate_stored(job)?;
    let science = &job.viirs_prepare.as_ref().unwrap().science;
    if header.width != science.width
        || header.height != science.height
        || header.crs != CRS
        || header.pixel_is_point
        || header
            .bounds
            .iter()
            .zip(science.bounds)
            .any(|(a, b)| (*a - b).abs() > 1e-8)
        || header.pixel_size != science.pixel_size
    {
        return Err("Prepared VIIRS GeoTIFF differs from its checked HDF-EOS grid".into());
    }
    Ok(())
}

fn convert(
    root: &Path,
    job: &Job,
    source: &Job,
    cancel: &CancellationToken,
) -> Result<(u64, String)> {
    validate_stored(job)?;
    check_cancel(Some(cancel))?;
    let spec = job.viirs_prepare.as_ref().unwrap();
    let path = crate::storage::verified_output_path(root, source)?;
    let assets = root.join("assets").canonicalize().map_err(io_error)?;
    if assets != root.join("assets") {
        return Err("Managed assets directory was redirected".into());
    }
    if fs2::available_space(&assets).map_err(io_error)? < 20 * 1024 * 1024 {
        return Err("Insufficient workspace space for VIIRS preparation".into());
    }
    let mut samples = Vec::new();
    let summary = hdf::read(
        &path,
        &source.item_id,
        source.bytes_downloaded,
        &spec.source_sha256,
        cancel,
        |band, values| {
            if band.band == job.asset_key {
                samples.extend_from_slice(values);
            }
            Ok(())
        },
    )?;
    if summary != spec.science || samples.len() != (EDGE * EDGE) as usize {
        return Err("VIIRS science layers differ from their source pin".into());
    }
    let band = summary
        .bands
        .iter()
        .find(|b| b.band == job.asset_key)
        .unwrap();
    let mut output = tempfile::Builder::new()
        .prefix(&format!("{}.viirs-", job.id))
        .suffix(".part")
        .tempfile_in(&assets)
        .map_err(io_error)?;
    encode(output.as_file_mut(), &summary, &samples)?;
    output.as_file().sync_all().map_err(io_error)?;
    check_cancel(Some(cancel))?;
    let mut decoder =
        Decoder::new(File::open(output.path()).map_err(io_error)?).map_err(io_error)?;
    let header = crate::raster::reflectance::validate_header(&mut decoder, &profile())?;
    grid(&header, job)?;
    let DecodingResult::I16(pixels) = decoder.read_image().map_err(io_error)? else {
        return Err("Prepared VIIRS samples are not Int16".into());
    };
    if pixels.len() != samples.len()
        || pixels != samples
        || sample_hash(&pixels) != band.samples_sha256
    {
        return Err("Prepared VIIRS pixels changed during encoding".into());
    }
    drop(decoder);
    drop(pixels);
    drop(samples);
    let bytes = output.as_file().metadata().map_err(io_error)?.len();
    let mut encoded = Vec::new();
    output.as_file_mut().rewind().map_err(io_error)?;
    output
        .as_file_mut()
        .read_to_end(&mut encoded)
        .map_err(io_error)?;
    let hash = format!("{:x}", Sha256::digest(&encoded));
    let manifest = serde_json::json!({"schemaVersion":"geod-viirs-raster/v1","createdAt":now(),"assetKey":job.asset_key,
        "source":{"jobId":source.id,"itemId":source.item_id,"href":source.href,"sha256":spec.source_sha256,"dataset":band.dataset,"samplesSha256":band.samples_sha256},
        "output":{"file":format!("{}.tif",job.id),"bytes":bytes,"sha256":hash},"science":summary,"qualityMaskApplied":false});
    let mut metadata = tempfile::Builder::new()
        .prefix(&format!("{}.viirs-", job.id))
        .suffix(".part")
        .tempfile_in(&assets)
        .map_err(io_error)?;
    metadata
        .write_all(&serde_json::to_vec_pretty(&manifest).map_err(io_error)?)
        .map_err(io_error)?;
    metadata.as_file().sync_all().map_err(io_error)?;
    check_cancel(Some(cancel))?;
    let manifest_path = assets.join(format!("{}.metadata.json", job.id));
    metadata
        .persist_noclobber(&manifest_path)
        .map_err(|e| io_error(e.error))?;
    if let Err(error) = output.persist_noclobber(assets.join(format!("{}.tif", job.id))) {
        let _ = std::fs::remove_file(manifest_path);
        return Err(io_error(error.error));
    }
    Ok((bytes, hash))
}

fn sample_hash(pixels: &[i16]) -> String {
    let mut hash = Sha256::new();
    for value in pixels {
        hash.update(value.to_le_bytes());
    }
    format!("{:x}", hash.finalize())
}
fn encode<W: Write + Seek>(file: W, science: &ScienceSummary, samples: &[i16]) -> Result<()> {
    let mut encoder = TiffEncoder::new(file)
        .map_err(io_error)?
        .with_compression(Compression::Deflate(DeflateLevel::Balanced));
    let mut image = encoder
        .new_image::<colortype::GrayI16>(EDGE, EDGE)
        .map_err(io_error)?;
    // VIIRS and MODIS use the same reviewed sphere/projection, with different
    // product identities, grid dimensions and spacing validated separately.
    crate::raster::reflectance::modis::write_crs(image.encoder())?;
    image
        .encoder()
        .write_tag(
            Tag::ModelPixelScaleTag,
            &[science.pixel_size[0], science.pixel_size[1], 0.0][..],
        )
        .map_err(io_error)?;
    image
        .encoder()
        .write_tag(
            Tag::ModelTiepointTag,
            &[0.0, 0.0, 0.0, science.bounds[0], science.bounds[3], 0.0][..],
        )
        .map_err(io_error)?;
    image
        .encoder()
        .write_tag(Tag::GdalNodata, "-28672")
        .map_err(io_error)?;
    image.encoder().write_tag(Tag::Unknown(42112), "<GDALMetadata><Item name=\"SCALE\" sample=\"0\" role=\"scale\">0.0001</Item><Item name=\"OFFSET\" sample=\"0\" role=\"offset\">0</Item><Item name=\"PRODUCT\">viirs-09a1-v002</Item></GDALMetadata>").map_err(io_error)?;
    image.write_data(samples).map_err(io_error)
}
