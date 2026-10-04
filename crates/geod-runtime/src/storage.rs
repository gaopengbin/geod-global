//! Compare the exact managed file after Windows resolves its merged AppData view.
//! Resolving a file is different from resolving its parent under MSIX virtualization.
use crate::{extension, io_error, Job, JobStatus, Result};
use std::path::{Path, PathBuf};

pub(crate) fn regular_file(path: &Path) -> Result<PathBuf> {
    let metadata = std::fs::symlink_metadata(path).map_err(io_error)?;
    let redirected = metadata.file_type().is_symlink();
    #[cfg(windows)]
    let redirected = {
        use std::os::windows::fs::MetadataExt;
        redirected || metadata.file_attributes() & 0x400 != 0 // FILE_ATTRIBUTE_REPARSE_POINT
    };
    if redirected || !metadata.is_file() {
        return Err("Managed files must be regular files without redirected links".into());
    }
    path.canonicalize().map_err(io_error)
}

pub(crate) fn exact_file(expected: &Path, recorded: &Path) -> Result<PathBuf> {
    // Keep the caller's managed-directory check. Canonicalize both FILES, not
    // only the record: Windows can map an AppData file into LocalCache while its
    // existing parent directory still resolves to the original logical path.
    let expected = regular_file(expected)?;
    let actual = regular_file(recorded)?;
    if actual != expected {
        return Err("The output is not its exact managed job file".into());
    }
    Ok(actual)
}

/// Resolve only this successful job's managed asset for native file revealing.
pub fn verified_output_path(root: &Path, job: &Job) -> Result<PathBuf> {
    if job.stac_source.is_some() {
        crate::stac::validate_job(root, job)?;
    } else if job.wcs_source.is_some() {
        crate::wcs::validate_job(root, job)?;
    }
    if job.status != JobStatus::Succeeded {
        return Err("Only completed outputs can be revealed.".into());
    }
    if uuid::Uuid::parse_str(&job.id)
        .ok()
        .map(|id| id.to_string())
        .as_deref()
        != Some(job.id.as_str())
    {
        return Err("Invalid managed job identifier".into());
    }
    let root = root.canonicalize().map_err(io_error)?;
    let assets = root.join("assets");
    if assets.canonicalize().map_err(io_error)? != assets {
        return Err("Managed source directory was redirected".into());
    }
    exact_file(
        &assets.join(format!("{}.{}", job.id, extension(&job.media_type)?)),
        Path::new(
            job.output_path
                .as_deref()
                .ok_or("The completed job has no output file.")?,
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_file_resolves_both_file_paths_and_rejects_other_files() {
        let root = tempfile::tempdir().unwrap();
        let expected = root.path().join("source.tif");
        std::fs::write(&expected, [1, 2, 3]).unwrap();
        assert_eq!(
            exact_file(&expected, &expected.canonicalize().unwrap()).unwrap(),
            expected.canonicalize().unwrap()
        );
        let other = root.path().join("other.tif");
        std::fs::write(&other, [1, 2, 3]).unwrap();
        assert!(exact_file(&expected, &other).is_err());
        assert!(exact_file(&expected, root.path()).is_err());
        assert!(exact_file(&expected, &root.path().join("missing.tif")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn exact_file_rejects_redirected_managed_files_and_recorded_aliases() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = root.path().join("outside.tif");
        std::fs::write(&outside, [1]).unwrap();
        let link = root.path().join("source.tif");
        symlink(&outside, &link).unwrap();
        assert!(exact_file(&link, &outside).is_err());
        assert!(exact_file(&outside, &link).is_err());
    }
}
