//! Managed Sentinel-2 L2A SAFE -> original-resolution TCI/SCL GeoTIFF.
//! Product/entry identity and geometry come from SAFE XML, never catalog guesses.
use crate::{active, io_error, now, raster::check_cancel, Job, JobManager, JobStatus, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom, Write},
    path::Path,
};
use tiff::{
    decoder::{Decoder, DecodingResult},
    encoder::{
        compression::{CompressionAlgorithm, Deflate, DeflateLevel},
        TiffEncoder,
    },
    tags::Tag,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

mod decode;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SafeSpec {
    pub source_job_id: String,
    pub source_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SafeOutput {
    pub entry: String,
    pub metadata_entry: String,
    pub width: u32,
    pub height: u32,
    pub band_count: u8,
    pub crs: String,
    pub bounds: [f64; 4],
    pub pixel_size: [f64; 2],
}

pub(crate) fn validate_stored(job: &Job) -> Result<()> {
    let spec = job
        .safe
        .as_ref()
        .ok_or("SAFE preparation has no source pin")?;
    crate::validate_request(
        &crate::CreateJobRequest {
            item_id: job.item_id.clone(),
            asset_key: "product".into(),
            href: job.href.clone(),
            media_type: "application/zip".into(),
            title: None,
        },
        None,
    )?;
    if job.kind != "raster_prepare"
        || !matches!(job.asset_key.as_str(), "visual" | "scl")
        || job.parent_id.as_deref() != Some(&spec.source_job_id)
        || Uuid::parse_str(&spec.source_job_id)
            .ok()
            .map(|id| id.to_string())
            .as_deref()
            != Some(&spec.source_job_id)
        || spec.source_sha256.len() != 64
        || !spec
            .source_sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        || !crate::providers::copernicus::valid_item(&job.item_id)
        || !crate::providers::matches_item(
            &url::Url::parse(&job.href).map_err(io_error)?,
            &job.item_id,
            "product",
        )
        || crate::extension(&job.media_type)? != "tif"
    {
        return Err("SAFE preparation provenance is invalid".into());
    }
    Ok(())
}

pub(crate) fn validate_source(job: &Job, jobs: &BTreeMap<String, Job>) -> Result<Job> {
    validate_stored(job)?;
    let spec = job.safe.as_ref().unwrap();
    let source = jobs
        .get(&spec.source_job_id)
        .ok_or("Original SAFE source job is missing")?;
    if source.kind != "download"
        || source.status != JobStatus::Succeeded
        || source.asset_key != "product"
        || source.item_id != job.item_id
        || source.href != job.href
        || crate::extension(&source.media_type)? != "zip"
        || source.sha256.as_deref() != Some(&spec.source_sha256)
    {
        return Err("Original SAFE source is missing, changed or incomplete".into());
    }
    Ok(source.clone())
}

pub(crate) fn scene_source<'a>(
    scene: &crate::projects::ProjectScene,
    jobs: &'a BTreeMap<String, Job>,
    key: &str,
) -> Option<&'a Job> {
    if let Some(asset) = scene.assets.get(key) {
        return jobs.values().find(|j| {
            j.kind == "download"
                && j.status == JobStatus::Succeeded
                && j.item_id == scene.item_id
                && j.asset_key == key
                && j.href == asset.href
                && j.sha256.is_some()
        });
    }
    let asset = scene.assets.get("product")?;
    jobs.values()
        .filter(|j| {
            j.kind == "raster_prepare"
                && j.status == JobStatus::Succeeded
                && j.item_id == scene.item_id
                && j.asset_key == key
                && j.href == asset.href
                && j.safe_output.is_some()
                && j.sha256.is_some()
                && validate_source(j, jobs).is_ok()
        })
        .max_by_key(|j| &j.updated_at)
}

impl JobManager {
    pub async fn prepare_project(&self, id: &str, key: &str) -> Result<crate::ProjectDownloads> {
        if matches!(key, "red" | "green" | "blue") {
            return self.prepare_viirs_project(id, key).await;
        }
        if !matches!(key, "visual" | "scl") {
            return Err("Choose SAFE true-color imagery or SCL".into());
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
        // Validate the entire batch before queuing any work.
        let mut output = Vec::new();
        let mut added = Vec::new();
        for scene in &project.scenes {
            let asset = scene
                .assets
                .get("product")
                .ok_or("This scene has no SAFE original product")?;
            let source = store
                .jobs
                .values()
                .find(|j| {
                    j.kind == "download"
                        && j.status == JobStatus::Succeeded
                        && j.item_id == scene.item_id
                        && j.asset_key == "product"
                        && j.href == asset.href
                })
                .ok_or("Download every original SAFE product before preparing rasters")?;
            let hash = source
                .sha256
                .clone()
                .ok_or("Original SAFE source has no checksum")?;
            if let Some(existing) = store.jobs.values().find(|j| {
                j.kind == "raster_prepare"
                    && j.asset_key == key
                    && j.parent_id.as_deref() == Some(&source.id)
                    && j.safe.as_ref().is_some_and(|s| s.source_sha256 == hash)
                    && (active(&j.status) || j.status == JobStatus::Succeeded)
                    && validate_source(j, &store.jobs).is_ok()
            }) {
                output.push(existing.clone());
                continue;
            }
            let timestamp = now();
            let mut job = crate::new_download_job(crate::CreateJobRequest {
                item_id: source.item_id.clone(),
                asset_key: key.into(),
                href: source.href.clone(),
                media_type: "image/tiff".into(),
                title: Some(format!(
                    "{} · {}",
                    source.item_id,
                    if key == "visual" { "TCI" } else { "SCL" }
                )),
            });
            job.kind = "raster_prepare".into();
            job.parent_id = Some(source.id.clone());
            job.safe = Some(SafeSpec {
                source_job_id: source.id.clone(),
                source_sha256: hash,
            });
            job.created_at = timestamp.clone();
            job.updated_at = timestamp;
            job.validation =
                "Pending original SAFE checksum, XML geometry and JP2 pixel validation".into();
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

    pub(crate) async fn process_safe(&self, id: &str, token: &CancellationToken) -> Result<()> {
        let permit = tokio::select! { _ = token.cancelled() => return Err("SAFE preparation cancelled".into()),
        p = self.inner.raster_permits.clone().acquire_owned() => p.map_err(io_error)? };
        let job = self.get(id).await.ok_or("Unknown preparation job")?;
        let source = {
            let store = self.inner.store.lock().await;
            validate_source(&job, &store.jobs)?
        };
        let root = self.inner.root.clone();
        let cancellation = token.clone();
        let worker_job = job.clone();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let worker = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            convert(&root, &worker_job, &source, &cancellation, Some(&tx))
        });
        let mut update_error = None;
        while let Some(progress) = rx.recv().await {
            let mut store = self.inner.store.lock().await;
            let record = store.jobs.get_mut(id).ok_or("Unknown preparation job")?;
            if active(&record.status) {
                record.status = JobStatus::Running;
                record.bytes_downloaded = progress;
                record.total_bytes = Some(1000);
                record.validation = "Decoding original SAFE JP2 pixels and checking GeoTIFF".into();
                record.updated_at = now();
                if let Err(error) = self.persist(&store.jobs).await {
                    update_error.get_or_insert(error);
                    token.cancel();
                }
            }
        }
        let (output, bytes, hash) = worker.await.map_err(io_error)??;
        if let Some(error) = update_error {
            return Err(error);
        }
        let mut store = self.inner.store.lock().await;
        let record = store.jobs.get_mut(id).ok_or("Unknown preparation job")?;
        if token.is_cancelled() || !active(&record.status) {
            return Err("SAFE preparation cancelled".into());
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
        record.safe_output = Some(output);
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
        record.validation = "Original SAFE SHA-256 and ZIP CRC checked; product and XML grid verified; full-resolution JP2 samples retained; GeoTIFF pixels decoded and checked".into();
        if let Err(error) = self.persist(&store.jobs).await {
            store.jobs.insert(id.into(), before);
            return Err(error);
        }
        Ok(())
    }
}

fn locked_file(path: &Path) -> Result<File> {
    let mut options = File::options();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1);
    }
    options.open(path).map_err(io_error)
}

fn hash_file(file: &mut File, cancel: &CancellationToken) -> Result<String> {
    file.seek(SeekFrom::Start(0)).map_err(io_error)?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        check_cancel(Some(cancel))?;
        let n = file.read(&mut buffer).map_err(io_error)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    file.seek(SeekFrom::Start(0)).map_err(io_error)?;
    Ok(format!("{:x}", hash.finalize()))
}

fn read_entry(
    archive: &mut zip::ZipArchive<File>,
    name: &str,
    max: u64,
    cancel: &CancellationToken,
) -> Result<Vec<u8>> {
    let mut entry = archive.by_name(name).map_err(io_error)?;
    if entry.size() == 0 || entry.size() > max {
        return Err("SAFE metadata entry size is unsupported".into());
    }
    let size = entry.size();
    let mut bytes = Vec::with_capacity(size as usize);
    let mut buffer = [0u8; 65536];
    loop {
        check_cancel(Some(cancel))?;
        let n = entry
            .read(&mut buffer)
            .map_err(|e| format!("SAFE ZIP CRC/decompression failed: {e}"))?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..n]);
        if bytes.len() as u64 > size {
            return Err("SAFE entry exceeded declared size".into());
        }
    }
    if bytes.len() as u64 != size {
        return Err("SAFE metadata entry is truncated".into());
    }
    Ok(bytes)
}

fn xml(bytes: &[u8]) -> Result<roxmltree::Document<'_>> {
    roxmltree::Document::parse_with_options(
        std::str::from_utf8(bytes).map_err(io_error)?,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 100_000,
            ..Default::default()
        },
    )
    .map_err(|e| format!("Invalid SAFE XML: {e}"))
}
fn unique<'a, 'input>(
    parent: roxmltree::Node<'a, 'input>,
    name: &str,
) -> Result<roxmltree::Node<'a, 'input>> {
    let mut nodes = parent
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == name);
    let node = nodes
        .next()
        .ok_or_else(|| format!("SAFE metadata lacks {name}"))?;
    if nodes.next().is_some() {
        return Err(format!("SAFE metadata repeats {name}"));
    }
    Ok(node)
}
fn text<'a, 'input>(parent: roxmltree::Node<'a, 'input>, name: &str) -> Result<&'a str> {
    unique(parent, name)?
        .text()
        .map(str::trim)
        .ok_or_else(|| format!("SAFE metadata has empty {name}"))
}
fn resolution<'a, 'input>(
    parent: roxmltree::Node<'a, 'input>,
    name: &str,
    res: &str,
) -> Result<roxmltree::Node<'a, 'input>> {
    let mut nodes = parent.children().filter(|n| {
        n.is_element() && n.tag_name().name() == name && n.attribute("resolution") == Some(res)
    });
    let node = nodes
        .next()
        .ok_or("SAFE metadata lacks the selected resolution")?;
    if nodes.next().is_some() {
        return Err("SAFE metadata repeats the selected resolution".into());
    }
    Ok(node)
}

fn metadata(
    archive: &mut zip::ZipArchive<File>,
    item: &str,
    key: &str,
    cancel: &CancellationToken,
) -> Result<SafeOutput> {
    let root = format!("{item}.SAFE/");
    let bytes = read_entry(
        archive,
        &format!("{root}MTD_MSIL2A.xml"),
        4 * 1024 * 1024,
        cancel,
    )?;
    let doc = xml(&bytes)?;
    let general = unique(doc.root_element(), "General_Info")?;
    let info = unique(general, "Product_Info")?;
    if text(info, "PRODUCT_URI")? != format!("{item}.SAFE")
        || text(info, "PROCESSING_LEVEL")? != "Level-2A"
    {
        return Err("SAFE XML product identity differs from the downloaded product".into());
    }
    let res = if key == "visual" { "10" } else { "20" };
    let suffix = if key == "visual" {
        "_TCI_10m"
    } else {
        "_SCL_20m"
    };
    let images = info
        .descendants()
        .filter(|n| n.is_element() && matches!(n.tag_name().name(), "IMAGE_FILE" | "IMAGE_FILE_2A"))
        .filter_map(|n| n.text())
        .filter(|name| name.ends_with(suffix))
        .collect::<Vec<_>>();
    if images.len() != 1 {
        return Err("SAFE product must identify exactly one selected TCI/SCL entry".into());
    }
    let reference = images[0];
    let parts = reference.split('/').collect::<Vec<_>>();
    let tile = item.split('_').nth(5).ok_or("Invalid SAFE tile")?;
    if parts.len() != 5
        || parts[0] != "GRANULE"
        || parts[2] != "IMG_DATA"
        || parts[3] != format!("R{res}m")
        || !parts[4].starts_with(&format!("{tile}_"))
        || parts
            .iter()
            .any(|p| p.is_empty() || *p == "." || *p == ".." || p.contains('\\'))
    {
        return Err("SAFE product image reference is not the selected tile/resolution".into());
    }
    let entry = format!("{root}{reference}.jp2");
    let metadata_entry = format!("{root}GRANULE/{}/MTD_TL.xml", parts[1]);
    let bytes = read_entry(archive, &metadata_entry, 4 * 1024 * 1024, cancel)?;
    let doc = xml(&bytes)?;
    let general = unique(doc.root_element(), "General_Info")?;
    if !text(general, "TILE_ID")?
        .split('_')
        .any(|part| part == tile)
    {
        return Err("SAFE tile metadata identifies another MGRS tile".into());
    }
    let geo = unique(
        unique(doc.root_element(), "Geometric_Info")?,
        "Tile_Geocoding",
    )?;
    let crs = text(geo, "HORIZONTAL_CS_CODE")?.to_owned();
    let zone: u16 = tile[1..3].parse().map_err(io_error)?;
    let epsg = if tile.as_bytes()[3] >= b'N' {
        32600
    } else {
        32700
    } + zone;
    if !(1..=60).contains(&zone) || crs != format!("EPSG:{epsg}") {
        return Err("SAFE tile CRS is not its MGRS UTM zone/hemisphere".into());
    }
    let size = resolution(geo, "Size", res)?;
    let pos = resolution(geo, "Geoposition", res)?;
    let width: u32 = text(size, "NCOLS")?.parse().map_err(io_error)?;
    let height: u32 = text(size, "NROWS")?.parse().map_err(io_error)?;
    let number = |name| -> Result<f64> { text(pos, name)?.parse().map_err(io_error) };
    let left = number("ULX")?;
    let top = number("ULY")?;
    let dx = number("XDIM")?;
    let dy = number("YDIM")?;
    let expected = if key == "visual" { 10.0 } else { 20.0 };
    if width == 0
        || height == 0
        || width > 10980
        || height > 10980
        || dx != expected
        || dy != -expected
        || !left.is_finite()
        || !top.is_finite()
        || !(-100_000.0..=1_000_000.0).contains(&left)
        || !(0.0..=10_000_000.0).contains(&top)
    {
        return Err("SAFE tile dimensions or georeferencing are unsupported".into());
    }
    Ok(SafeOutput {
        entry,
        metadata_entry,
        width,
        height,
        band_count: if key == "visual" { 3 } else { 1 },
        crs,
        bounds: [
            left,
            top - height as f64 * expected,
            left + width as f64 * expected,
            top,
        ],
        pixel_size: [expected; 2],
    })
}

fn convert(
    root: &Path,
    job: &Job,
    source: &Job,
    cancel: &CancellationToken,
    progress: Option<&tokio::sync::mpsc::UnboundedSender<u64>>,
) -> Result<(SafeOutput, u64, String)> {
    validate_stored(job)?;
    check_cancel(Some(cancel))?;
    let assets = root.join("assets").canonicalize().map_err(io_error)?;
    if assets != root.join("assets") {
        return Err("Managed assets directory was redirected".into());
    }
    let expected = assets.join(format!("{}.zip", source.id));
    let path = crate::storage::exact_file(
        &expected,
        Path::new(
            source
                .output_path
                .as_deref()
                .ok_or("SAFE source has no local file")?,
        ),
    )?;
    let mut file = locked_file(&path)?;
    if file.metadata().map_err(io_error)?.len() != source.bytes_downloaded
        || source.bytes_downloaded == 0
        || source.bytes_downloaded > crate::providers::copernicus::MAX_PRODUCT_BYTES
    {
        return Err("SAFE source byte count differs or exceeds the product limit".into());
    }
    if hash_file(&mut file, cancel)? != job.safe.as_ref().unwrap().source_sha256 {
        return Err("SAFE source SHA-256 differs from its pin".into());
    }
    crate::providers::copernicus::verify_safe(&path, &job.item_id)?;
    let mut archive = zip::ZipArchive::new(file).map_err(io_error)?;
    let plan = metadata(&mut archive, &job.item_id, &job.asset_key, cancel)?;
    let mut jp2 = tempfile::Builder::new()
        .prefix(&format!("{}.safe-", job.id))
        .suffix(".jp2")
        .tempfile_in(&assets)
        .map_err(io_error)?;
    let mut entry = archive.by_name(&plan.entry).map_err(io_error)?;
    let entry_size = entry.size();
    if entry_size == 0 || entry_size > 512 * 1024 * 1024 {
        return Err("SAFE JP2 entry exceeds 512 MiB".into());
    }
    let required = entry_size
        + plan.width as u64 * plan.height as u64 * plan.band_count as u64
        + 4 * 1024 * 1024;
    if fs2::available_space(&assets).map_err(io_error)? < required {
        return Err("Insufficient workspace space for SAFE preparation".into());
    }
    let mut buffer = [0u8; 65536];
    let mut copied = 0;
    loop {
        check_cancel(Some(cancel))?;
        let n = entry
            .read(&mut buffer)
            .map_err(|e| format!("SAFE JP2 ZIP CRC/decompression failed: {e}"))?;
        if n == 0 {
            break;
        }
        copied += n as u64;
        if copied > entry_size {
            return Err("SAFE JP2 entry exceeded its declared size".into());
        }
        jp2.write_all(&buffer[..n]).map_err(io_error)?;
    }
    if copied != entry_size {
        return Err("SAFE JP2 entry is truncated".into());
    }
    jp2.flush().map_err(io_error)?;
    drop(entry);
    drop(archive);
    decode::preflight(jp2.path(), &plan)?;
    if let Some(tx) = progress {
        let _ = tx.send(100);
    }
    let mut output = tempfile::Builder::new()
        .prefix(&format!("{}.safe-", job.id))
        .suffix(".part")
        .tempfile_in(&assets)
        .map_err(io_error)?;
    let pixel_hash = encode(output.as_file_mut(), jp2.path(), &plan, cancel, progress)?;
    output.as_file().sync_all().map_err(io_error)?;
    readback(output.path(), &plan, &pixel_hash, cancel)?;
    let bytes = output.as_file().metadata().map_err(io_error)?.len();
    let hash = hash_file(output.as_file_mut(), cancel)?;
    let manifest = serde_json::json!({ "schemaVersion":"geod-safe-raster/v1", "createdAt":now(), "assetKey":job.asset_key,
        "source":{"jobId":source.id,"itemId":source.item_id,"href":source.href,"sha256":source.sha256},
        "output":{"file":format!("{}.tif", job.id),"bytes":bytes,"sha256":hash}, "grid":plan });
    let mut metadata_file = tempfile::Builder::new()
        .prefix(&format!("{}.safe-", job.id))
        .suffix(".part")
        .tempfile_in(&assets)
        .map_err(io_error)?;
    metadata_file
        .write_all(&serde_json::to_vec_pretty(&manifest).map_err(io_error)?)
        .map_err(io_error)?;
    metadata_file.as_file().sync_all().map_err(io_error)?;
    check_cancel(Some(cancel))?;
    metadata_file
        .persist_noclobber(assets.join(format!("{}.metadata.json", job.id)))
        .map_err(|e| io_error(e.error))?;
    output
        .persist_noclobber(assets.join(format!("{}.tif", job.id)))
        .map_err(|e| io_error(e.error))?;
    Ok((plan, bytes, hash))
}

fn encode(
    file: &mut File,
    jp2: &Path,
    plan: &SafeOutput,
    cancel: &CancellationToken,
    progress: Option<&tokio::sync::mpsc::UnboundedSender<u64>>,
) -> Result<String> {
    let mut encoder = TiffEncoder::new(file).map_err(io_error)?;
    let mut image = encoder.image_directory().map_err(io_error)?;
    image
        .write_tag(Tag::ImageWidth, plan.width)
        .map_err(io_error)?;
    image
        .write_tag(Tag::ImageLength, plan.height)
        .map_err(io_error)?;
    image
        .write_tag(
            Tag::BitsPerSample,
            &vec![8u16; plan.band_count as usize][..],
        )
        .map_err(io_error)?;
    image
        .write_tag(Tag::SamplesPerPixel, plan.band_count as u16)
        .map_err(io_error)?;
    image
        .write_tag(Tag::SampleFormat, &vec![1u16; plan.band_count as usize][..])
        .map_err(io_error)?;
    image
        .write_tag(
            Tag::PhotometricInterpretation,
            if plan.band_count == 3 { 2u16 } else { 1u16 },
        )
        .map_err(io_error)?;
    image.write_tag(Tag::Compression, 8u16).map_err(io_error)?;
    image
        .write_tag(Tag::PlanarConfiguration, 1u16)
        .map_err(io_error)?;
    image.write_tag(Tag::Orientation, 1u16).map_err(io_error)?;
    let rows = 256u32.min(plan.height);
    image.write_tag(Tag::RowsPerStrip, rows).map_err(io_error)?;
    let epsg: u16 = plan.crs[5..].parse().map_err(io_error)?;
    image
        .write_tag(
            Tag::GeoKeyDirectoryTag,
            &[
                1u16, 1, 0, 4, 1024, 0, 1, 1, 1025, 0, 1, 1, 3072, 0, 1, epsg, 3076, 0, 1, 9001,
            ][..],
        )
        .map_err(io_error)?;
    image
        .write_tag(
            Tag::ModelPixelScaleTag,
            &[plan.pixel_size[0], plan.pixel_size[1], 0.0][..],
        )
        .map_err(io_error)?;
    image
        .write_tag(
            Tag::ModelTiepointTag,
            &[0.0, 0.0, 0.0, plan.bounds[0], plan.bounds[3], 0.0][..],
        )
        .map_err(io_error)?;
    image.write_tag(Tag::GdalNodata, "0").map_err(io_error)?;
    let mut offsets = Vec::new();
    let mut counts = Vec::new();
    let mut hash = Sha256::new();
    let mut y = 0;
    while y < plan.height {
        check_cancel(Some(cancel))?;
        let height = rows.min(plan.height - y);
        let pixels = decode::strip(jp2, plan, y, height)?;
        check_cancel(Some(cancel))?;
        hash.update(&pixels);
        let mut compressed = Vec::new();
        Deflate::with_level(DeflateLevel::Balanced)
            .write_to(&mut compressed, &pixels)
            .map_err(io_error)?;
        offsets.push(
            u32::try_from(image.write_data(&compressed[..]).map_err(io_error)?)
                .map_err(io_error)?,
        );
        counts.push(u32::try_from(compressed.len()).map_err(io_error)?);
        y += height;
        if let Some(tx) = progress {
            let _ = tx.send(100 + 700 * y as u64 / plan.height as u64);
        }
    }
    image
        .write_tag(Tag::StripOffsets, &offsets[..])
        .map_err(io_error)?;
    image
        .write_tag(Tag::StripByteCounts, &counts[..])
        .map_err(io_error)?;
    image.finish().map_err(io_error)?;
    Ok(format!("{:x}", hash.finalize()))
}

fn readback(
    path: &Path,
    plan: &SafeOutput,
    pixels_hash: &str,
    cancel: &CancellationToken,
) -> Result<()> {
    let mut decoder =
        Decoder::new(BufReader::new(File::open(path).map_err(io_error)?)).map_err(io_error)?;
    if decoder.dimensions().map_err(io_error)? != (plan.width, plan.height)
        || decoder.colortype().map_err(io_error)?
            != if plan.band_count == 3 {
                tiff::ColorType::RGB(8)
            } else {
                tiff::ColorType::Gray(8)
            }
        || crate::raster::validate_geokeys(
            &decoder
                .get_tag_u16_vec(Tag::GeoKeyDirectoryTag)
                .map_err(io_error)?,
        )? != plan.crs
    {
        return Err("Prepared SAFE GeoTIFF dimensions/bands/CRS failed read-back".into());
    }
    let scale = decoder
        .get_tag_f64_vec(Tag::ModelPixelScaleTag)
        .map_err(io_error)?;
    let tie = decoder
        .get_tag_f64_vec(Tag::ModelTiepointTag)
        .map_err(io_error)?;
    if crate::raster::georeference(plan.width, plan.height, None, Some(&scale), Some(&tie))?
        != (plan.bounds, plan.pixel_size)
    {
        return Err("Prepared SAFE GeoTIFF grid failed read-back".into());
    }
    let mut hash = Sha256::new();
    let mut count = 0u64;
    for index in 0..decoder.strip_count().map_err(io_error)? {
        check_cancel(Some(cancel))?;
        let DecodingResult::U8(pixels) = decoder.read_chunk(index).map_err(io_error)? else {
            return Err("Prepared SAFE pixels are not UInt8".into());
        };
        count += pixels.len() as u64;
        hash.update(pixels);
    }
    if count != plan.width as u64 * plan.height as u64 * plan.band_count as u64
        || format!("{:x}", hash.finalize()) != pixels_hash
    {
        return Err("Prepared SAFE GeoTIFF samples failed read-back".into());
    }
    Ok(())
}
