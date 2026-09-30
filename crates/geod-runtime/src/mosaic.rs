//! Bounded, pixel-aligned project mosaics. These are new GeoTIFF files made
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
    encoder::{colortype, compression::DeflateLevel, Compression, TiffEncoder},
    tags::Tag,
    ColorType,
};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const MAX_OUTPUT_PIXELS: u64 = 8_000_000;
const MAX_SOURCE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 128 * 1024 * 1024;

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
}

pub(crate) fn validate_stored_mosaic(job: &Job) -> Result<()> {
    let spec = job
        .mosaic
        .as_ref()
        .ok_or("Stored mosaic job has no source specification")?;
    if job.kind != "raster_mosaic"
        || job.asset_key != spec.asset_key
        || !matches!(spec.asset_key.as_str(), "scl" | "visual")
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
    for source in &spec.sources {
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
    Ok(())
}

pub(crate) fn validate_mosaic_sources(job: &Job, jobs: &BTreeMap<String, Job>) -> Result<Vec<Job>> {
    validate_stored_mosaic(job)?;
    let spec = job.mosaic.as_ref().unwrap();
    let mut sources = Vec::with_capacity(spec.sources.len());
    for pin in &spec.sources {
        let source = jobs
            .get(&pin.job_id)
            .ok_or("A project source job is missing")?;
        if source.kind != "download"
            || source.status != JobStatus::Succeeded
            || source.asset_key != spec.asset_key
            || source.sha256.as_deref() != Some(&pin.sha256)
            || crate::extension(&source.media_type)? != "tif"
        {
            return Err(
                "A project source is missing, changed or not a completed GeoTIFF download".into(),
            );
        }
        sources.push(source.clone());
    }
    Ok(sources)
}

impl JobManager {
    pub async fn run_project_mosaic(&self, id: &str, asset_key: &str) -> Result<Job> {
        if !matches!(asset_key, "scl" | "visual") {
            return Err("Choose SCL or true-color imagery".into());
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
        if store.active.len() >= 64 {
            return Err("The local queue is full (64 jobs)".into());
        }
        let mut pins = Vec::with_capacity(project.scenes.len());
        let mut ordered_scenes = project.scenes.iter().collect::<Vec<_>>();
        ordered_scenes.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.item_id.cmp(&b.item_id)));
        for scene in ordered_scenes {
            let asset = scene
                .assets
                .get(asset_key)
                .ok_or("A selected scene lacks this asset type")?;
            let source = store
                .jobs
                .values()
                .find(|job| {
                    job.kind == "download"
                        && job.item_id == scene.item_id
                        && job.asset_key == asset_key
                        && job.href == asset.href
                        && job.status == JobStatus::Succeeded
                        && job.sha256.is_some()
                })
                .ok_or_else(|| {
                    format!("Download {} for every scene before mosaicking", asset_key)
                })?;
            pins.push(MosaicSource {
                job_id: source.id.clone(),
                sha256: source.sha256.clone().unwrap(),
            });
        }
        let timestamp = now();
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
            }),
            mosaic_output: None,
            manifest_path: None,
            item_id: format!("project:{}", project.id),
            asset_key: asset_key.into(),
            href: project.scenes[0].assets[asset_key].href.clone(),
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
            source: "Earth Search / Copernicus Sentinel-2".into(),
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
        let project = self
            .inner
            .projects
            .lock()
            .await
            .get(&spec.project_id)
            .cloned()
            .ok_or("Project was removed")?;
        let sources = {
            let store = self.inner.store.lock().await;
            validate_mosaic_sources(&job, &store.jobs)?
        };
        let total_steps = sources.len() as u64 * 2 + 1;
        self.mosaic_progress(id, 0, total_steps, "Checking downloaded sources")
            .await?;
        let source_provenance = sources
            .iter()
            .map(|source| {
                let scene = project
                    .scenes
                    .iter()
                    .find(|scene| scene.item_id == source.item_id);
                serde_json::json!({
                    "jobId":source.id,"itemId":source.item_id,"href":source.href,
                    "sha256":source.sha256,"attribution":source.source,
                    "acquiredAt":scene.map(|scene| scene.date.as_str()),
                })
            })
            .collect::<Vec<_>>();
        let project_provenance = serde_json::json!({
            "id":project.id,"name":project.name,"bounds":project.bounds,"geometry":project.geometry,
        });
        let root = self.inner.root.clone();
        let output_id = id.to_owned();
        let asset_key = spec.asset_key.clone();
        let cancellation = token.clone();
        let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel();
        let worker = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            write_mosaic(
                &root,
                &project,
                &sources,
                &asset_key,
                &output_id,
                &cancellation,
                Some(&progress_tx),
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
        let manifest = serde_json::json!({
            "schemaVersion":"geod-project-mosaic/v1", "createdAt":now(),
            "output":{"file":format!("{id}.tif"),"format":"GeoTIFF","bytes":output.bytes,"sha256":output.sha256},
            "project":project_provenance,"assetKey":spec.asset_key,
            "sources":source_provenance,"plan":output.plan,
        });
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
        record.validation = "All local sources SHA-256 verified; same-CRS aligned UInt8 pixels mosaicked and clipped; output decoded and checked; no reprojection or resampling".into();
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
    pub(crate) decoder: Decoder<BufReader<File>>,
    pub(crate) width: u32,
    pub(crate) height: u32,
    crs: String,
    bounds: [f64; 4],
    pixel_size: [f64; 2],
    nodata: Option<u8>,
    bands: usize,
}

pub(crate) fn source_raster(
    root: &Path,
    job: &Job,
    key: &str,
    cancel: &CancellationToken,
) -> Result<SourceRaster> {
    check_cancel(Some(cancel))?;
    let assets = root.join("assets").canonicalize().map_err(io_error)?;
    if assets != root.join("assets") {
        return Err("Managed source directory was redirected".into());
    }
    let expected = assets.join(format!("{}.tif", job.id));
    let actual = Path::new(
        job.output_path
            .as_deref()
            .ok_or("Source has no output file")?,
    )
    .canonicalize()
    .map_err(io_error)?;
    if actual != expected {
        return Err("Mosaic source is not its managed GeoTIFF file".into());
    }
    let mut file = File::open(&actual).map_err(io_error)?;
    let size = file.metadata().map_err(io_error)?.len();
    if size == 0
        || size > MAX_SOURCE_BYTES
        || size != job.bytes_downloaded
        || job.total_bytes.is_some_and(|total| total != size)
    {
        return Err("Mosaic source byte count is invalid or exceeds 512 MiB".into());
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
    if width == 0 || height == 0 || width > 20000 || height > 20000 {
        return Err("Source GeoTIFF dimensions are unsupported".into());
    }
    let bands = if key == "scl" { 1 } else { 3 };
    let color = decoder.colortype().map_err(io_error)?;
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
    let crs = validate_geokeys(&keys)?;
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
    Ok(SourceRaster {
        decoder,
        width,
        height,
        crs,
        bounds,
        pixel_size,
        nodata,
        bands,
    })
}

fn aligned(value: f64) -> Result<i64> {
    let rounded = value.round();
    if !value.is_finite() || (value - rounded).abs() > 1e-5 || rounded.abs() > 1_000_000.0 {
        return Err(
            "Source grids are not pixel-aligned; choose scenes on the same UTM grid and resolution"
                .into(),
        );
    }
    Ok(rounded as i64)
}

struct MosaicOutput {
    path: String,
    bytes: u64,
    sha256: String,
    plan: MosaicPlan,
}

fn write_mosaic(
    root: &Path,
    project: &Project,
    sources: &[Job],
    key: &str,
    output_id: &str,
    cancel: &CancellationToken,
    progress: Option<&UnboundedSender<(u64, &'static str)>>,
) -> Result<MosaicOutput> {
    if sources.is_empty() || sources.len() != project.scenes.len() {
        return Err("Project source count changed".into());
    }
    let mut rasters = Vec::with_capacity(sources.len());
    let report = |completed, stage| {
        if let Some(progress) = progress {
            let _ = progress.send((completed, stage));
        }
    };
    for (index, source) in sources.iter().enumerate() {
        rasters.push(source_raster(root, source, key, cancel)?);
        report(index as u64 + 1, "Checking downloaded sources");
    }
    let first = &rasters[0];
    let crs = first.crs.clone();
    let [dx, dy] = first.pixel_size;
    let bands = first.bands;
    let mut union = first.bounds;
    for raster in &rasters {
        if raster.crs != crs
            || raster.bands != bands
            || (raster.pixel_size[0] - dx).abs() > 1e-8
            || (raster.pixel_size[1] - dy).abs() > 1e-8
        {
            return Err("Sources use different UTM zones, pixel sizes or band types; select one aligned grid per mosaic".into());
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
    let x0 = ((clipped[0] - first.bounds[0]) / dx).floor() as i64;
    let x1 = ((clipped[2] - first.bounds[0]) / dx).ceil() as i64;
    let y0 = ((first.bounds[3] - clipped[3]) / dy).floor() as i64;
    let y1 = ((first.bounds[3] - clipped[1]) / dy).ceil() as i64;
    let width = u32::try_from(x1 - x0).map_err(io_error)?;
    let height = u32::try_from(y1 - y0).map_err(io_error)?;
    let count = width as u64 * height as u64;
    if count == 0 || count > MAX_OUTPUT_PIXELS {
        return Err("Project output exceeds 8 million pixels; choose a smaller area or process smaller groups".into());
    }
    let left = first.bounds[0] + x0 as f64 * dx;
    let top = first.bounds[3] - y0 as f64 * dy;
    let bounds = [
        left,
        top - height as f64 * dy,
        left + width as f64 * dx,
        top,
    ];
    let mut pixels = vec![0u8; count as usize * bands];
    let mut covered = vec![false; count as usize];
    let deadline = Instant::now() + Duration::from_secs(180);
    for (index, raster) in rasters.iter_mut().enumerate() {
        copy_source(
            raster,
            left,
            top,
            width,
            height,
            dx,
            dy,
            &mut pixels,
            &mut covered,
            cancel,
            deadline,
        )?;
        report(
            sources.len() as u64 + index as u64 + 1,
            "Combining scene pixels",
        );
    }
    let mut masked_pixels = 0u64;
    if let Some(geometry) = &project.geometry {
        let inside =
            crop::polygon_coverage(geometry, &crs, bounds, [dx, dy], width, height, cancel)?;
        for (index, included) in inside.iter().enumerate() {
            if !included {
                masked_pixels += 1;
                covered[index] = false;
                pixels[index * bands..(index + 1) * bands].fill(0);
            }
        }
    }
    let covered_pixels = covered.iter().filter(|value| **value).count() as u64;
    if covered_pixels == 0 {
        return Err("No valid source pixels remain inside this project area".into());
    }
    let plan = MosaicPlan {
        width,
        height,
        band_count: bands as u8,
        crs,
        bounds,
        pixel_size: [dx, dy],
        source_count: sources.len(),
        masked_pixels,
        covered_pixels,
        overlap_policy: "newest non-nodata scene wins; unfilled pixels are zero".into(),
    };
    report(sources.len() as u64 * 2, "Writing and checking GeoTIFF");
    let output = encode_mosaic(root, output_id, &plan, &pixels, cancel)?;
    report(sources.len() as u64 * 2 + 1, "Writing and checking GeoTIFF");
    Ok(output)
}

fn copy_source(
    raster: &mut SourceRaster,
    left: f64,
    top: f64,
    width: u32,
    height: u32,
    dx: f64,
    dy: f64,
    output: &mut [u8],
    covered: &mut [bool],
    cancel: &CancellationToken,
    deadline: Instant,
) -> Result<()> {
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
    for cy in 0..rows {
        check_cancel(Some(cancel))?;
        if Instant::now() > deadline {
            return Err("Mosaic decoding exceeded its time limit".into());
        }
        for cx in 0..columns {
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
            let decoded = match raster.decoder.read_chunk(index).map_err(io_error)? {
                DecodingResult::U8(values) => values,
                _ => return Err("Source chunk is not UInt8".into()),
            };
            let expected = actual_w as usize * actual_h as usize * raster.bands;
            if decoded.len() < expected {
                return Err("Decoded source chunk has too few samples".into());
            }
            for row in 0..actual_h {
                let target_y = out_y + row as i64;
                if target_y < 0 || target_y >= height as i64 {
                    continue;
                }
                for col in 0..actual_w {
                    let target_x = out_x + col as i64;
                    if target_x < 0 || target_x >= width as i64 {
                        continue;
                    }
                    let source_index =
                        (row as usize * actual_w as usize + col as usize) * raster.bands;
                    let sample = &decoded[source_index..source_index + raster.bands];
                    if raster.bands == 1 && sample[0] > 11 {
                        return Err(
                            "SCL source contains a value outside the 0–11 classification range"
                                .into(),
                        );
                    }
                    if raster
                        .nodata
                        .is_some_and(|nodata| sample.iter().all(|value| *value == nodata))
                        || (raster.bands == 1 && sample[0] == 0)
                    {
                        continue;
                    }
                    let target = target_y as usize * width as usize + target_x as usize;
                    output[target * raster.bands..(target + 1) * raster.bands]
                        .copy_from_slice(sample);
                    covered[target] = true;
                }
            }
        }
    }
    Ok(())
}

fn add_geo_tags<W: Write + Seek, C: tiff::encoder::colortype::ColorType>(
    image: &mut tiff::encoder::ImageEncoder<'_, W, C, tiff::encoder::TiffKindStandard>,
    plan: &MosaicPlan,
) -> Result<()> {
    let epsg = plan
        .crs
        .strip_prefix("EPSG:")
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or("Invalid mosaic CRS")?;
    let keys = [
        1u16, 1, 0, 4, 1024, 0, 1, 1, 1025, 0, 1, 1, 3072, 0, 1, epsg, 3076, 0, 1, 9001,
    ];
    image
        .encoder()
        .write_tag(Tag::GeoKeyDirectoryTag, &keys[..])
        .map_err(io_error)?;
    image
        .encoder()
        .write_tag(
            Tag::ModelPixelScaleTag,
            &[plan.pixel_size[0], plan.pixel_size[1], 0.0][..],
        )
        .map_err(io_error)?;
    image
        .encoder()
        .write_tag(
            Tag::ModelTiepointTag,
            &[0.0, 0.0, 0.0, plan.bounds[0], plan.bounds[3], 0.0][..],
        )
        .map_err(io_error)?;
    image
        .encoder()
        .write_tag(Tag::GdalNodata, "0")
        .map_err(io_error)?;
    Ok(())
}

fn encode_mosaic(
    root: &Path,
    id: &str,
    plan: &MosaicPlan,
    pixels: &[u8],
    cancel: &CancellationToken,
) -> Result<MosaicOutput> {
    let assets = root.join("assets").canonicalize().map_err(io_error)?;
    if assets != root.join("assets") {
        return Err("Managed output directory was redirected".into());
    }
    let final_path = assets.join(format!("{id}.tif"));
    if final_path.exists() {
        return Err("Mosaic output already exists".into());
    }
    let mut temporary = tempfile::Builder::new()
        .prefix(&format!("{id}.mosaic-"))
        .suffix(".part")
        .tempfile_in(&assets)
        .map_err(io_error)?;
    {
        let mut encoder = TiffEncoder::new(temporary.as_file_mut())
            .map_err(io_error)?
            .with_compression(Compression::Deflate(DeflateLevel::Balanced));
        if plan.band_count == 1 {
            let mut image = encoder
                .new_image::<colortype::Gray8>(plan.width, plan.height)
                .map_err(io_error)?;
            image
                .rows_per_strip(plan.height.min(128))
                .map_err(io_error)?;
            add_geo_tags(&mut image, plan)?;
            image.write_data(pixels).map_err(io_error)?;
        } else {
            let mut image = encoder
                .new_image::<colortype::RGB8>(plan.width, plan.height)
                .map_err(io_error)?;
            image
                .rows_per_strip(plan.height.min(128))
                .map_err(io_error)?;
            add_geo_tags(&mut image, plan)?;
            image.write_data(pixels).map_err(io_error)?;
        }
    }
    temporary.as_file().sync_all().map_err(io_error)?;
    check_cancel(Some(cancel))?;
    let bytes = std::fs::read(temporary.path()).map_err(io_error)?;
    if bytes.len() > MAX_OUTPUT_BYTES {
        return Err("Mosaic output exceeds 128 MiB".into());
    }
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let mut decoded = Decoder::new(std::io::Cursor::new(&bytes)).map_err(io_error)?;
    if decoded.dimensions().map_err(io_error)? != (plan.width, plan.height)
        || decoded.colortype().map_err(io_error)?
            != if plan.band_count == 1 {
                ColorType::Gray(8)
            } else {
                ColorType::RGB(8)
            }
    {
        return Err("Mosaic output read-back dimensions or bands differ".into());
    }
    let keys = decoded
        .get_tag_u16_vec(Tag::GeoKeyDirectoryTag)
        .map_err(io_error)?;
    if validate_geokeys(&keys)? != plan.crs {
        return Err("Mosaic output CRS read-back differs".into());
    }
    let scale = decoded
        .get_tag_f64_vec(Tag::ModelPixelScaleTag)
        .map_err(io_error)?;
    let tiepoint = decoded
        .get_tag_f64_vec(Tag::ModelTiepointTag)
        .map_err(io_error)?;
    let (bounds, pixel_size) =
        georeference(plan.width, plan.height, None, Some(&scale), Some(&tiepoint))?;
    if bounds != plan.bounds || pixel_size != plan.pixel_size {
        return Err("Mosaic output georeferencing read-back differs".into());
    }
    let readback = match decoded.read_image().map_err(io_error)? {
        DecodingResult::U8(values) => values,
        _ => return Err("Mosaic output is not UInt8".into()),
    };
    if readback != pixels {
        return Err("Mosaic output pixels failed read-back validation".into());
    }
    check_cancel(Some(cancel))?;
    temporary
        .persist_noclobber(&final_path)
        .map_err(|error| format!("Cannot commit mosaic: {}", error.error))?;
    Ok(MosaicOutput {
        path: final_path.to_string_lossy().into_owned(),
        bytes: bytes.len() as u64,
        sha256: hash,
        plan: plan.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::{CreateProjectRequest, ProjectAsset, ProjectScene};
    use std::io::Cursor;

    fn project(sources: &[Job], asset_key: &str) -> Project {
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
            created_at: now(),
            updated_at: now(),
        }
    }

    fn rgb_fixture(pixels: &[u8]) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut buffer).unwrap();
            let mut image = encoder.new_image::<colortype::RGB8>(2, 2).unwrap();
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
        assert_eq!(progress.first(), Some(&(1, "Checking downloaded sources")));
        assert_eq!(progress.last(), Some(&(5, "Writing and checking GeoTIFF")));
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
