//! Bounded, verified delivery bundles. Only managed derived outputs are eligible.
use crate::{io_error, Job, JobManager, JobStatus, Result};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

const MAX_PACKAGE_INPUT: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactPackage {
    pub job_id: String,
    pub filename: String,
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    pub files: Vec<String>,
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file = File::open(path).map_err(io_error)?;
    if file.metadata().map_err(io_error)?.len() > limit {
        return Err("Delivery package exceeds the 32 MiB input limit".into());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() as u64 > limit {
        return Err("Delivery package input grew beyond its limit".into());
    }
    Ok(bytes)
}

fn managed(root: &Path, directory: &str) -> Result<PathBuf> {
    let root = root.canonicalize().map_err(io_error)?;
    let expected = root.join(directory);
    let actual = expected.canonicalize().map_err(io_error)?;
    if actual != expected {
        return Err("Managed artifact directory was redirected".into());
    }
    Ok(actual)
}

fn verified_bundle(root: &Path, job: &Job, source: &Job) -> Result<(ArtifactPackage, Vec<u8>)> {
    if job.status != JobStatus::Succeeded || job.kind != "raster_clip" {
        return Err("Only completed derived GeoTIFF outputs can be packaged".into());
    }
    if uuid::Uuid::parse_str(&job.id)
        .ok()
        .map(|value| value.to_string())
        .as_deref()
        != Some(job.id.as_str())
    {
        return Err("Invalid artifact identifier".into());
    }
    let recipe = job
        .recipe
        .as_ref()
        .ok_or("The artifact recipe is missing")?;
    recipe.validate()?;
    if recipe.source.job_id != source.id
        || source.sha256.as_deref() != Some(recipe.source.sha256.as_str())
    {
        return Err("The artifact source record does not match its recipe".into());
    }
    let crop = job
        .crop
        .as_ref()
        .ok_or("The artifact crop plan is missing")?;
    let assets = managed(root, "assets")?;
    let tiff_name = format!("{}.tif", job.id);
    let metadata_name = format!("{}.metadata.json", job.id);
    let tiff_path = assets.join(&tiff_name);
    let metadata_path = assets.join(&metadata_name);
    for (recorded, expected) in [
        (job.output_path.as_deref(), &tiff_path),
        (job.manifest_path.as_deref(), &metadata_path),
    ] {
        crate::storage::exact_file(
            expected,
            Path::new(recorded.ok_or("The artifact file record is missing")?),
        )
        .map_err(|error| {
            format!("Artifact export is restricted to the exact managed job files: {error}")
        })?;
    }
    let tiff = read_bounded(&tiff_path, MAX_PACKAGE_INPUT)?;
    let output_hash = format!("{:x}", Sha256::digest(&tiff));
    if job.sha256.as_deref() != Some(output_hash.as_str())
        || job.bytes_downloaded != tiff.len() as u64
    {
        return Err(
            "Artifact checksum or size changed; the delivery package was not created".into(),
        );
    }
    let metadata = read_bounded(&metadata_path, 65536)?;
    let manifest: Value = serde_json::from_slice(&metadata).map_err(io_error)?;
    let created = manifest
        .get("createdAt")
        .and_then(Value::as_str)
        .ok_or("Invalid artifact creation timestamp")?;
    chrono::DateTime::parse_from_rfc3339(created).map_err(io_error)?;
    let expected = json!({
        "schemaVersion":"geod-raster-artifact/v1", "createdAt":created,
        "output":{"file":tiff_name,"format":"GeoTIFF","bytes":tiff.len(),"sha256":output_hash},
        "source":{"jobId":source.id,"itemId":source.item_id,"href":source.href,"attribution":source.source,"sha256":source.sha256},
        "recipe":recipe,"crop":crop,
    });
    if manifest != expected {
        return Err(
            "Artifact metadata no longer matches the committed source, recipe and crop plan".into(),
        );
    }
    let recipe_bytes = serde_json::to_vec_pretty(recipe).map_err(io_error)?;
    let readme = b"GeoD Global verified raster delivery\n\nThe GeoTIFF contains source CRS, transform and nodata tags. Its accompanying metadata records source attribution, checksums and the exact crop recipe. No pixels were reprojected or resampled.\n\nrecipe.json is pinned to the original local source job ID. It does not include the source raster, auto-download data or execute commands. Rebind the source ID and its verified SHA-256 when using a different storage directory.\n\nchecksums.sha256 covers all content files. This package contains no absolute local paths. It includes the user-defined recipe name and spatial bounds: review those before sharing.\n";
    let mut files = vec![
        (tiff_name, tiff),
        (metadata_name, metadata),
        ("recipe.json".into(), recipe_bytes),
        ("README.txt".into(), readme.to_vec()),
    ];
    let checksums = files
        .iter()
        .map(|(name, bytes)| format!("{:x}  {name}\n", Sha256::digest(bytes)))
        .collect::<String>();
    files.push(("checksums.sha256".into(), checksums.into_bytes()));
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .unix_permissions(0o644);
    for (name, bytes) in &files {
        zip.start_file(name, options).map_err(io_error)?;
        zip.write_all(bytes).map_err(io_error)?;
    }
    let bytes = zip.finish().map_err(io_error)?.into_inner();
    let filename = format!("geod-artifact-{}.zip", job.id);
    let package = ArtifactPackage {
        job_id: job.id.clone(),
        filename: filename.clone(),
        path: root
            .join("exports")
            .join(filename)
            .to_string_lossy()
            .into_owned(),
        bytes: bytes.len() as u64,
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        files: files.into_iter().map(|(name, _)| name).collect(),
    };
    Ok((package, bytes))
}

impl JobManager {
    async fn bundle_input(&self, id: &str) -> Result<(Job, Job)> {
        let job = self.get(id).await.ok_or("Unknown job")?;
        let source_id = job
            .recipe
            .as_ref()
            .ok_or("The artifact recipe is missing")?
            .source
            .job_id
            .clone();
        let source = self
            .get(&source_id)
            .await
            .ok_or("The original source record is missing")?;
        Ok((job, source))
    }

    pub async fn prepare_artifact(&self, id: &str) -> Result<ArtifactPackage> {
        if self
            .get(id)
            .await
            .is_some_and(|job| job.kind == "raster_rgb")
        {
            return self.scientific_rgb_package(id, false).await;
        }
        let (job, source) = self.bundle_input(id).await?;
        let root = self.storage_root().to_owned();
        let permit = self
            .inner
            .raster_permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| "The raster worker is busy; retry packaging shortly")?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let (mut package, bytes) = verified_bundle(&root, &job, &source)?;
            std::fs::create_dir_all(root.join("exports")).map_err(io_error)?;
            let exports = managed(&root, "exports")?;
            let target = exports.join(&package.filename);
            if target.exists() {
                if crate::storage::regular_file(&target).is_err()
                    || read_bounded(&target, MAX_PACKAGE_INPUT + 1024 * 1024)? != bytes
                {
                    return Err(
                        "An existing delivery package has changed; it will not be overwritten"
                            .into(),
                    );
                }
            } else {
                let mut temporary = tempfile::NamedTempFile::new_in(&exports).map_err(io_error)?;
                temporary.write_all(&bytes).map_err(io_error)?;
                temporary.as_file().sync_all().map_err(io_error)?;
                temporary.persist_noclobber(&target).map_err(io_error)?;
            }
            package.path = crate::storage::regular_file(&target)?
                .to_string_lossy()
                .into_owned();
            Ok(package)
        })
        .await
        .map_err(io_error)?
    }

    /// GET only reads a previously prepared package, revalidating its exact inputs.
    pub async fn artifact_bytes(&self, id: &str) -> Result<(ArtifactPackage, Vec<u8>)> {
        if self
            .get(id)
            .await
            .is_some_and(|job| job.kind == "raster_rgb")
        {
            let package = self.scientific_rgb_package(id, true).await?;
            let bytes = tokio::fs::read(&package.path).await.map_err(io_error)?;
            if bytes.len() as u64 != package.bytes
                || format!("{:x}", Sha256::digest(&bytes)) != package.sha256
            {
                return Err("The RGB delivery package changed during reading".into());
            }
            return Ok((package, bytes));
        }
        let (job, source) = self.bundle_input(id).await?;
        let root = self.storage_root().to_owned();
        let permit = self
            .inner
            .raster_permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| "The raster worker is busy; retry the package download shortly")?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let (mut package, expected) = verified_bundle(&root, &job, &source)?;
            let target = managed(&root, "exports")?.join(&package.filename);
            if crate::storage::regular_file(&target).is_err()
                || read_bounded(&target, MAX_PACKAGE_INPUT + 1024 * 1024)? != expected
            {
                return Err("The prepared delivery package has changed".into());
            }
            package.path = crate::storage::regular_file(&target)?
                .to_string_lossy()
                .into_owned();
            Ok((package, expected))
        })
        .await
        .map_err(io_error)?
    }
}

#[cfg(test)]
mod tests;
