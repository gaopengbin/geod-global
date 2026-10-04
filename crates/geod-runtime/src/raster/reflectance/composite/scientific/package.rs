use super::*;
use crate::artifact::ArtifactPackage;
use std::io::Write;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

pub(super) fn metadata_bytes(root: &Path, job: &Job) -> Result<Vec<u8>> {
    validate_stored(job)?;
    if job.status != JobStatus::Succeeded {
        return Err("Scientific RGB is not complete".into());
    }
    let path = root
        .join("assets")
        .join(format!("{}.metadata.json", job.id));
    storage::exact_file(
        &path,
        Path::new(
            job.manifest_path
                .as_deref()
                .ok_or("Scientific RGB metadata is missing")?,
        ),
    )?;
    let file = File::open(path).map_err(io_error)?;
    let size = file.metadata().map_err(io_error)?.len();
    if size == 0 || size > MAX_SPEC_BYTES as u64 + 65536 {
        return Err("Scientific RGB metadata exceeds its limit".into());
    }
    let mut bytes = Vec::new();
    file.take(size + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() as u64 != size {
        return Err("Scientific RGB metadata changed while reading".into());
    }
    let manifest: Value = serde_json::from_slice(&bytes).map_err(io_error)?;
    let created = manifest["createdAt"]
        .as_str()
        .ok_or("Invalid RGB creation time")?;
    chrono::DateTime::parse_from_rfc3339(created).map_err(io_error)?;
    let expected = json!({"schemaVersion":SCHEMA,"createdAt":created,"spec":job.rgb_spec,
        "output":{"file":format!("{}.tif",job.id),"format":"GeoTIFF","bytes":job.bytes_downloaded,"sha256":job.sha256,"samples":job.rgb_output}});
    if manifest != expected {
        return Err(
            "Scientific RGB metadata differs from its committed specification and samples".into(),
        );
    }
    Ok(bytes)
}

fn hash_file(path: &Path, limit: u64) -> Result<(u64, String)> {
    storage::regular_file(path)?;
    let mut file = File::open(path).map_err(io_error)?;
    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(io_error)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > limit {
            return Err("RGB delivery package exceeds its limit".into());
        }
        hash.update(&buffer[..n]);
    }
    Ok((total, format!("{:x}", hash.finalize())))
}

fn bundle(root: &Path, job: &Job, require_existing: bool) -> Result<ArtifactPackage> {
    let metadata = metadata_bytes(root, job)?;
    let preview = reader::inspect(root, job, 160)?;
    let png = STANDARD
        .decode(
            preview
                .preview_data_url
                .split(',')
                .nth(1)
                .ok_or("Missing RGB preview")?,
        )
        .map_err(io_error)?;
    let exports = root.join("exports");
    std::fs::create_dir_all(&exports).map_err(io_error)?;
    if exports.canonicalize().map_err(io_error)? != exports {
        return Err("Managed RGB export directory was redirected".into());
    }
    if fs2::available_space(&exports).map_err(io_error)? < job.bytes_downloaded + 4 * 1024 * 1024 {
        return Err("Insufficient disk space for RGB delivery package".into());
    }
    let mut temporary = tempfile::NamedTempFile::new_in(&exports).map_err(io_error)?;
    let mut zip = ZipWriter::new(temporary.as_file_mut());
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .unix_permissions(0o644);
    let tiff_name = format!("{}.tif", job.id);
    zip.start_file(&tiff_name, options).map_err(io_error)?;
    let mut source = File::open(storage::verified_output_path(root, job)?).map_err(io_error)?;
    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0; 65536];
    loop {
        let n = source.read(&mut buffer).map_err(io_error)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > MAX_ASSET_BYTES {
            return Err("Scientific RGB exceeds its export limit".into());
        }
        hash.update(&buffer[..n]);
        zip.write_all(&buffer[..n]).map_err(io_error)?;
    }
    let hash = format!("{:x}", hash.finalize());
    if total != job.bytes_downloaded || job.sha256.as_deref() != Some(&hash) {
        return Err("Scientific RGB file changed during delivery packaging".into());
    }
    let mut checksums = format!("{hash}  {tiff_name}\n");
    let metadata_name = format!("{}.metadata.json", job.id);
    let sample_note = if job
        .rgb_spec
        .as_ref()
        .unwrap()
        .quality_mask
        .as_ref()
        .and_then(|m| m.coupled())
        .is_some()
    {
        if job.rgb_spec.as_ref().unwrap().profile.product == "landsat-c2-l2" {
            "Every retained RGB triplet comes from one complete, qualified original Landsat 8/9 scene. Overlaps choose the newest acquisition date, then item ID; a qualified older scene fills flagged or incomplete newer pixels. Pixels without any qualified complete candidate are NoData in all three channels. Independently mosaicked parent values are not used. metadata.json includes every original five-layer scene pin, selection rule, per-scene retained counts and quality fallback count. Input-valid counts refer to availability of a complete same-scene RGB candidate before quality selection; rejected counts include NoData and uncovered pixels. These explicit rules do not guarantee atmospheric accuracy."
        } else {
            "Every retained RGB triplet comes from one complete, qualified original MODIS scene. Overlaps choose the newest composite start, then item ID; a qualified older scene fills flagged or incomplete newer pixels. Pixels without any qualified complete candidate are NoData in all three channels. Independently mosaicked parent values are not used. metadata.json includes every original five-layer scene pin, selection rule, per-scene retained counts and quality fallback count. Input-valid counts refer to availability of a complete same-scene RGB candidate before quality selection; rejected counts include NoData and uncovered pixels. These explicit rules do not guarantee atmospheric accuracy."
        }
    } else if job.rgb_spec.as_ref().unwrap().quality_mask.is_some() {
        "Accepted pixels retain original 16-bit DN. Pixels rejected by the recorded product-specific quality rules are set to NoData in all three channels. metadata.json includes the quality-layer pins, policy, snow option and examined/rejected/removed-valid counts. These are explicit flag-selection rules, not a guarantee of atmospheric accuracy."
    } else {
        "All original 16-bit DN values remain unchanged, including where another channel is NoData."
    };
    let readme = if job.rgb_spec.as_ref().unwrap().quality_mask.is_none() {
        "GeoD Global scientific RGB\n\nThis three-band GeoTIFF retains original 16-bit DN values in red, green, blue order; per-channel scale and offset, shared NoData and the original spatial grid are stored in TIFF tags. Values remain unchanged even where another channel is NoData. No reprojection or resampling was performed. This is a strip-based GeoTIFF, not a COG.\n\npreview.png is display-only (nearest-neighbour samples, independent 2-98 percent stretches, transparent where any channel is NoData). Use the TIFF for analysis. metadata.json records all source URLs, attribution, pinned source checksums and processing provenance. Source files are not included. This package does not grant additional data rights.\n\nchecksums.sha256 covers each content file. No absolute local file paths are included.\n".to_owned()
    } else {
        format!("GeoD Global scientific RGB\n\n{sample_note}\n\nThe channels are red, green, blue; per-channel scale and offset, shared NoData and the original spatial grid are stored in TIFF tags. No reprojection or resampling was performed. This is a strip-based GeoTIFF, not a COG.\n\npreview.png is display-only (nearest-neighbour samples, independent 2-98 percent stretches, transparent where any channel is NoData). Use the TIFF for analysis. metadata.json records source URLs, attribution, pinned checksums and processing provenance. Source files are not included. This package does not grant additional data rights.\n\nchecksums.sha256 covers each content file. No absolute local file paths are included.\n")
    };
    let mut files = vec![tiff_name];
    for (name, bytes) in [
        (metadata_name, metadata),
        ("preview.png".into(), png),
        ("README.txt".into(), readme.into_bytes()),
    ] {
        checksums.push_str(&format!("{:x}  {name}\n", Sha256::digest(&bytes)));
        zip.start_file(&name, options).map_err(io_error)?;
        zip.write_all(&bytes).map_err(io_error)?;
        files.push(name);
    }
    zip.start_file("checksums.sha256", options)
        .map_err(io_error)?;
    zip.write_all(checksums.as_bytes()).map_err(io_error)?;
    files.push("checksums.sha256".into());
    zip.finish().map_err(io_error)?;
    temporary.as_file().sync_all().map_err(io_error)?;
    let (bytes, sha256) = hash_file(temporary.path(), MAX_ASSET_BYTES + 4 * 1024 * 1024)?;
    let filename = format!("geod-rgb-{}.zip", job.id);
    let path = exports.join(&filename);
    if path.exists() {
        if hash_file(&path, MAX_ASSET_BYTES + 4 * 1024 * 1024)? != (bytes, sha256.clone()) {
            return Err(
                "The existing RGB delivery package changed; it will not be overwritten".into(),
            );
        }
    } else if require_existing {
        return Err("Prepare the RGB delivery package before downloading it".into());
    } else {
        temporary.persist_noclobber(&path).map_err(io_error)?;
    }
    Ok(ArtifactPackage {
        job_id: job.id.clone(),
        filename,
        path: path.to_string_lossy().into_owned(),
        bytes,
        sha256,
        files,
    })
}

impl JobManager {
    pub async fn scientific_rgb_metadata_bytes(&self, id: &str) -> Result<(String, Vec<u8>)> {
        let job = self.get(id).await.ok_or("Unknown scientific RGB job")?;
        let root = self.inner.root.clone();
        tokio::task::spawn_blocking(move || {
            Ok((
                format!("{}.metadata.json", job.id),
                metadata_bytes(&root, &job)?,
            ))
        })
        .await
        .map_err(io_error)?
    }
    pub(crate) async fn scientific_rgb_package(
        &self,
        id: &str,
        existing: bool,
    ) -> Result<ArtifactPackage> {
        let job = self.get(id).await.ok_or("Unknown scientific RGB job")?;
        let root = self.inner.root.clone();
        let permit = self
            .inner
            .raster_permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| "The raster worker is busy; retry packaging shortly")?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            bundle(&root, &job, existing)
        })
        .await
        .map_err(io_error)?
    }
}
