//! Disposable, bounded disk cache of previews generated from verified originals.
//! A hit checks managed file identity/metadata, without rehashing the whole TIFF.
use super::{FileThumbnail, EDGE};
use crate::{io_error, storage, Job, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

// Bump when the palette, sampling or rendering behavior changes.
const VERSION: u32 = 1;
const MAX_ENTRY_BYTES: u64 = 192 * 1024;
const MAX_CACHE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_CACHE_ENTRIES: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceFingerprint {
    path: String,
    bytes: u64,
    modified: String,
    created: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    version: u32,
    asset_key: String,
    edge: u32,
    source: SourceFingerprint,
    preview: FileThumbnail,
    png_sha256: String,
}

fn timestamp(time: SystemTime) -> Option<String> {
    let duration = time.duration_since(UNIX_EPOCH).ok()?;
    Some(format!(
        "{}:{}",
        duration.as_secs(),
        duration.subsec_nanos()
    ))
}

fn source_fingerprint(root: &Path, job: &Job) -> Result<SourceFingerprint> {
    let checksum = job
        .sha256
        .as_deref()
        .ok_or("Preview has no source checksum")?;
    if checksum.len() != 64 || !checksum.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Preview source checksum is invalid".into());
    }
    if crate::extension(&job.media_type)? != "tif"
        && !(job.asset_key == "srtm" && crate::extension(&job.media_type)? == "zip")
    {
        return Err("Preview requires a completed managed SCL or RGB GeoTIFF".into());
    }
    let path = storage::verified_output_path(root, job)?;
    let metadata = fs::metadata(&path).map_err(io_error)?;
    let bytes = metadata.len();
    if bytes == 0
        || (job.kind != "raster_mosaic" && bytes > crate::source_transfer_limit(job))
        || bytes != job.bytes_downloaded
        || job.total_bytes.is_some_and(|total| total != bytes)
    {
        return Err("Preview source size no longer matches its completed job".into());
    }
    Ok(SourceFingerprint {
        path: path.to_string_lossy().into_owned(),
        bytes,
        modified: metadata
            .modified()
            .ok()
            .and_then(timestamp)
            .ok_or("Cannot read preview source modification time")?,
        created: metadata.created().ok().and_then(timestamp),
    })
}

fn directory(root: &Path) -> Result<PathBuf> {
    let mut path = root.to_path_buf();
    for component in ["cache", "thumbnails", "v1"] {
        path.push(component);
        match fs::create_dir(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(io_error(error)),
        }
        let metadata = fs::symlink_metadata(&path).map_err(io_error)?;
        let redirected = metadata.file_type().is_symlink();
        #[cfg(windows)]
        let redirected = {
            use std::os::windows::fs::MetadataExt;
            redirected || metadata.file_attributes() & 0x400 != 0
        };
        if !metadata.is_dir() || redirected {
            return Err("Thumbnail cache directory must not redirect to another location".into());
        }
    }
    // Windows package virtualization may resolve a normal directory into
    // LocalCache. Each fixed path component was checked for redirected links.
    path.canonicalize().map_err(io_error)
}

fn key(job: &Job) -> String {
    let mut recipe = if matches!(job.asset_key.as_str(), "ndvi" | "evi") {
        format!("\0{}", crate::raster::reflectance::VEGETATION_PALETTE)
    } else if crate::providers::vegetation::SCIENCE_KEYS.contains(&job.asset_key.as_str()) {
        format!("\0{}", crate::raster::science::PALETTE)
    } else {
        String::new()
    };
    if job
        .mosaic
        .as_ref()
        .is_some_and(|m| m.vi_selection.is_some())
    {
        // A rules/count change must miss the disk cache and check the TIFF's
        // pinned selection metadata, even after all originals are removed.
        let binding = serde_json::to_vec(&(&job.mosaic, &job.mosaic_output)).unwrap_or_default();
        recipe.push_str(&format!("\0{:x}", Sha256::digest(binding)));
    }
    format!(
        "{:x}",
        Sha256::digest(format!(
            "thumbnail/{VERSION}\0{}\0{}\0{}\0{EDGE}{recipe}",
            job.id,
            job.sha256.as_deref().unwrap_or(""),
            job.asset_key
        ))
    )
}

fn entry_path(directory: &Path, job: &Job) -> PathBuf {
    directory.join(format!("{}.json", key(job)))
}

fn png_bytes(preview: &FileThumbnail) -> Option<Vec<u8>> {
    if preview.width == 0 || preview.height == 0 || preview.width > EDGE || preview.height > EDGE {
        return None;
    }
    let encoded = preview.data_url.strip_prefix("data:image/png;base64,")?;
    if encoded.len() > MAX_ENTRY_BYTES as usize {
        return None;
    }
    let bytes = STANDARD.decode(encoded).ok()?;
    let decoder = png::Decoder::new_with_limits(
        Cursor::new(&bytes),
        png::Limits {
            bytes: MAX_ENTRY_BYTES as usize,
        },
    );
    let mut reader = decoder.read_info().ok()?;
    let info = reader.info();
    if info.width != preview.width
        || info.height != preview.height
        || info.color_type != png::ColorType::Rgba
        || info.bit_depth != png::BitDepth::Eight
        || info.animation_control.is_some()
    {
        return None;
    }
    let mut decoded = vec![0; reader.output_buffer_size()?];
    reader.next_frame(&mut decoded).ok()?;
    drop(reader);
    Some(bytes)
}

fn read(directory: &Path, job: &Job, source: &SourceFingerprint) -> Option<FileThumbnail> {
    let path = entry_path(directory, job);
    let path = storage::regular_file(&path).ok()?;
    let mut file = File::open(&path).ok()?;
    let size = file.metadata().ok()?.len();
    if size == 0 || size > MAX_ENTRY_BYTES {
        return None;
    }
    let mut bytes = Vec::with_capacity(size as usize);
    Read::by_ref(&mut file)
        .take(MAX_ENTRY_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 != size {
        return None;
    }
    let entry: Entry = serde_json::from_slice(&bytes).ok()?;
    if entry.version != VERSION
        || entry.edge != EDGE
        || entry.asset_key != job.asset_key
        || &entry.source != source
        || entry.preview.job_id != job.id
        || Some(&entry.preview.sha256) != job.sha256.as_ref()
    {
        return None;
    }
    if format!("{:x}", Sha256::digest(png_bytes(&entry.preview)?)) != entry.png_sha256 {
        return None;
    }
    // Last use drives eviction, including hits after restarting the app.
    if let Ok(file) = OpenOptions::new().write(true).open(&path) {
        let _ = file.set_modified(SystemTime::now());
    }
    Some(entry.preview)
}

fn write(
    directory: &Path,
    job: &Job,
    source: SourceFingerprint,
    preview: &FileThumbnail,
) -> Result<()> {
    let png = png_bytes(preview).ok_or("Generated thumbnail PNG is invalid")?;
    let entry = Entry {
        version: VERSION,
        asset_key: job.asset_key.clone(),
        edge: EDGE,
        source,
        preview: preview.clone(),
        png_sha256: format!("{:x}", Sha256::digest(png)),
    };
    let bytes = serde_json::to_vec(&entry).map_err(io_error)?;
    if bytes.len() as u64 > MAX_ENTRY_BYTES {
        return Err("Thumbnail cache entry is too large".into());
    }
    let mut temporary = tempfile::Builder::new()
        .prefix("thumbnail-")
        .suffix(".part")
        .tempfile_in(directory)
        .map_err(io_error)?;
    temporary.write_all(&bytes).map_err(io_error)?;
    temporary.as_file().sync_all().map_err(io_error)?;
    temporary
        .persist(entry_path(directory, job))
        .map_err(|error| io_error(error.error))?;
    Ok(())
}

fn cache_file_name(name: &str) -> bool {
    name.strip_suffix(".json")
        .is_some_and(|key| key.len() == 64 && key.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn prune(directory: &Path, max_bytes: u64, max_entries: usize) -> Result<()> {
    let mut entries = Vec::new();
    let mut total = 0u64;
    for entry in fs::read_dir(directory).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let path = match storage::regular_file(&entry.path()) {
            Ok(path) => path,
            Err(_) => continue,
        };
        // Only this worker writes in the cache directory. Orphaned unique
        // temporary previews can be discarded after an interrupted write.
        if name.starts_with("thumbnail-") && name.ends_with(".part") {
            let _ = fs::remove_file(path);
            continue;
        }
        if !cache_file_name(&name) {
            continue;
        }
        let metadata = fs::metadata(&path).map_err(io_error)?;
        total = total.saturating_add(metadata.len());
        entries.push((
            metadata.modified().unwrap_or(UNIX_EPOCH),
            path,
            metadata.len(),
        ));
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    let mut count = entries.len();
    for (_, path, bytes) in entries {
        if total <= max_bytes && count <= max_entries {
            break;
        }
        fs::remove_file(&path).map_err(io_error)?;
        total = total.saturating_sub(bytes);
        count -= 1;
    }
    Ok(())
}

pub(super) fn get_or_generate(
    root: &Path,
    job: &Job,
    generate: impl FnOnce() -> Result<FileThumbnail>,
) -> Result<FileThumbnail> {
    let before = source_fingerprint(root, job)?;
    let directory = directory(root).ok();
    if let Some(directory) = &directory {
        if let Some(preview) = read(directory, job, &before) {
            let _ = prune(directory, MAX_CACHE_BYTES, MAX_CACHE_ENTRIES);
            return Ok(preview);
        }
    }
    let preview = generate()?;
    let after = source_fingerprint(root, job)?;
    if before != after {
        return Err("Preview source changed while generating the thumbnail".into());
    }
    if let Some(directory) = &directory {
        // Cache failure must not hide a successfully generated file preview.
        let _ = write(directory, job, after, &preview);
        let _ = prune(directory, MAX_CACHE_BYTES, MAX_CACHE_ENTRIES);
    }
    Ok(preview)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        raster::tests::{fixture, record},
        JobManager,
    };
    use std::{collections::BTreeMap, time::Duration};

    fn generate(root: &Path, job: &Job) -> FileThumbnail {
        get_or_generate(root, job, || super::super::thumbnail(root, job)).unwrap()
    }

    #[tokio::test]
    async fn a_new_manager_reuses_the_persisted_preview_without_redecoding_the_source() {
        let temp = tempfile::tempdir().unwrap();
        let manager = JobManager::open(temp.path()).await.unwrap();
        let root = manager.storage_root().to_path_buf();
        let job = record(&root, &fixture(2, 1, &[4, 6], 32610, false));
        let records = BTreeMap::from([(job.id.clone(), job.clone())]);
        manager.inner.store.lock().await.jobs = records.clone();
        manager.persist(&records).await.unwrap();
        let first = manager.file_thumbnail(&job.id).await.unwrap();
        let cache_path = entry_path(&directory(&root).unwrap(), &job);
        assert!(cache_path.is_file());
        drop(manager);
        let restarted = JobManager::open(temp.path()).await.unwrap();
        let second = restarted.file_thumbnail(&job.id).await.unwrap();
        assert_eq!(first.data_url, second.data_url);
        let cached = get_or_generate(restarted.storage_root(), &job, || {
            panic!("a disk hit must not decode or rehash the TIFF")
        })
        .unwrap();
        assert_eq!(second.data_url, cached.data_url);
    }

    #[test]
    fn source_timestamps_and_recorded_checksums_invalidate_previews() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut job = record(&root, &fixture(1, 1, &[4], 32610, false));
        let first = generate(&root, &job);
        let source = Path::new(job.output_path.as_ref().unwrap());
        File::options()
            .write(true)
            .open(source)
            .unwrap()
            .set_modified(UNIX_EPOCH + Duration::from_secs(1_000_000))
            .unwrap();
        let mut regenerated = false;
        get_or_generate(&root, &job, || {
            regenerated = true;
            super::super::thumbnail(&root, &job)
        })
        .unwrap();
        assert!(regenerated);
        let bytes = fixture(1, 1, &[6], 32610, false);
        fs::write(source, &bytes).unwrap();
        job.sha256 = Some(format!("{:x}", Sha256::digest(&bytes)));
        job.bytes_downloaded = bytes.len() as u64;
        job.total_bytes = Some(bytes.len() as u64);
        let next = generate(&root, &job);
        assert_ne!(first.data_url, next.data_url);
        assert_eq!(next.sha256, job.sha256.as_ref().unwrap().as_str());
    }

    #[test]
    fn cache_cannot_hide_missing_changed_or_unmanaged_source_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut job = record(&root, &fixture(1, 1, &[4], 32610, false));
        generate(&root, &job);
        let source = PathBuf::from(job.output_path.as_ref().unwrap());
        fs::write(&source, fixture(1, 1, &[6], 32610, false)).unwrap();
        File::options()
            .write(true)
            .open(&source)
            .unwrap()
            .set_modified(UNIX_EPOCH + Duration::from_secs(2_000_000))
            .unwrap();
        assert!(
            get_or_generate(&root, &job, || super::super::thumbnail(&root, &job))
                .unwrap_err()
                .contains("SHA-256")
        );
        let outside = root.join("outside.tif");
        fs::copy(&source, &outside).unwrap();
        job.output_path = Some(outside.to_string_lossy().into_owned());
        assert!(get_or_generate(&root, &job, || panic!(
            "unmanaged file must fail before generation"
        ))
        .is_err());
        job.output_path = Some(source.to_string_lossy().into_owned());
        fs::remove_file(source).unwrap();
        assert!(get_or_generate(&root, &job, || panic!(
            "missing file must fail before generation"
        ))
        .is_err());
    }

    #[test]
    fn corrupt_oversized_and_old_version_entries_are_rebuilt() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let job = record(&root, &fixture(2, 1, &[4, 6], 32610, false));
        let first = generate(&root, &job);
        let path = entry_path(&directory(&root).unwrap(), &job);
        for corrupt in [
            b"unfinished JSON".to_vec(),
            vec![0; MAX_ENTRY_BYTES as usize + 1],
        ] {
            fs::write(&path, corrupt).unwrap();
            assert_eq!(generate(&root, &job).data_url, first.data_url);
        }
        for field in ["version", "pngSha256", "dataUrl"] {
            let mut entry: Entry = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            match field {
                "version" => entry.version += 1,
                "pngSha256" => entry.png_sha256 = "0".repeat(64),
                _ => {
                    entry.preview.data_url = "data:image/png;base64,AAAA".into();
                    entry.png_sha256 = format!("{:x}", Sha256::digest([0, 0, 0]));
                }
            }
            fs::write(&path, serde_json::to_vec(&entry).unwrap()).unwrap();
            assert_eq!(generate(&root, &job).data_url, first.data_url);
        }
    }

    #[test]
    fn changed_source_during_generation_is_never_committed_and_cache_failure_keeps_the_preview_usable(
    ) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let job = record(&root, &fixture(1, 1, &[4], 32610, false));
        let result = get_or_generate(&root, &job, || {
            let preview = super::super::thumbnail(&root, &job)?;
            File::options()
                .write(true)
                .open(job.output_path.as_ref().unwrap())
                .unwrap()
                .set_modified(UNIX_EPOCH + Duration::from_secs(3_000_000))
                .unwrap();
            Ok(preview)
        });
        assert!(result.unwrap_err().contains("changed while generating"));
        assert!(!entry_path(&directory(&root).unwrap(), &job).exists());
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let job = record(&root, &fixture(1, 1, &[4], 32610, false));
        fs::write(root.join("cache"), b"cache unavailable").unwrap();
        assert_eq!(generate(&root, &job).width, 1);
    }

    #[test]
    fn capacity_is_enforced_on_writes_and_hits_using_last_use_without_deleting_sources() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut jobs = Vec::new();
        for index in 0..3 {
            let job = record(&root, &fixture(1, 1, &[4], 32610, false));
            generate(&root, &job);
            let cache = directory(&root).unwrap();
            File::options()
                .write(true)
                .open(entry_path(&cache, &job))
                .unwrap()
                .set_modified(UNIX_EPOCH + Duration::from_secs(100 + index))
                .unwrap();
            jobs.push(job);
        }
        let cache = directory(&root).unwrap();
        fs::write(cache.join("keep.txt"), b"unrelated").unwrap();
        fs::write(cache.join("thumbnail-orphan.part"), b"unfinished").unwrap();
        get_or_generate(&root, &jobs[0], || panic!("hit must update last use")).unwrap();
        assert!(!cache.join("thumbnail-orphan.part").exists());
        prune(&cache, u64::MAX, 2).unwrap();
        assert!(entry_path(&cache, &jobs[0]).exists());
        assert!(!entry_path(&cache, &jobs[1]).exists());
        assert!(entry_path(&cache, &jobs[2]).exists());
        prune(&cache, 1, usize::MAX).unwrap();
        assert!(jobs.iter().all(|job| !entry_path(&cache, job).exists()));
        assert!(jobs
            .iter()
            .all(|job| Path::new(job.output_path.as_ref().unwrap()).is_file()));
        assert!(cache.join("keep.txt").is_file());
    }
}
