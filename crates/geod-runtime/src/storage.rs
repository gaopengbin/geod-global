//! Compare the exact managed file after Windows resolves its merged AppData view.
//! Resolving a file is different from resolving its parent under MSIX virtualization.
use crate::{extension, io_error, Job, JobStatus, Result};
use std::path::{Path, PathBuf};
#[cfg(windows)]
mod msix;

/// Resolve a single managed child directory. A packaged Windows parent can
/// give a child process a merged AppData view: its existing root resolves to
/// the logical directory, while a newly created child resolves to this same
/// package's LocalCache. Bind that namespace to an exclusively created file's
/// OS handle; a detached launcher need not remain alive.
pub(crate) fn managed_directory(root: &Path, name: &str) -> Result<PathBuf> {
    if name.is_empty()
        || name.len() > 100
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err("Invalid managed directory name".into());
    }
    if directory(root)? != root {
        return Err("Managed storage root changed".into());
    }
    let expected = root.join(name);
    let actual = directory(&expected)?;
    if actual == expected {
        return Ok(actual);
    }
    #[cfg(windows)]
    if merged_appdata_directory(root, &expected, &actual) {
        return Ok(actual);
    }
    Err("Agent record directory was redirected.".into())
}
fn directory(path: &Path) -> Result<PathBuf> {
    let metadata = std::fs::symlink_metadata(path).map_err(io_error)?;
    let redirected = metadata.file_type().is_symlink();
    #[cfg(windows)]
    let redirected = {
        use std::os::windows::fs::MetadataExt;
        redirected || metadata.file_attributes() & 0x400 != 0
    };
    if redirected || !metadata.is_dir() {
        return Err("Agent record directory was redirected.".into());
    }
    path.canonicalize().map_err(io_error)
}
#[cfg(windows)]
fn merged_appdata_directory(root: &Path, expected: &Path, actual: &Path) -> bool {
    let Some(local) = std::env::var_os("LOCALAPPDATA") else {
        return false;
    };
    let Ok(local) = PathBuf::from(local).canonicalize() else {
        return false;
    };
    msix::matches(root, expected, actual, &local)
}
#[cfg(windows)]
fn package_cache_matches(expected: &Path, actual: &Path, local: &Path, family: &str) -> bool {
    use std::path::Component;
    let Ok(relative) = expected.strip_prefix(local) else {
        return false;
    };
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        || relative
            .components()
            .next()
            .is_some_and(|c| c.as_os_str().eq_ignore_ascii_case("Packages"))
    {
        return false;
    }
    actual
        == local
            .join("Packages")
            .join(family)
            .join("LocalCache")
            .join("Local")
            .join(relative)
}

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
    fn managed_directory_keeps_exact_scope_and_rejects_files_or_path_injection() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        std::fs::create_dir(root.join("agent-places")).unwrap();
        assert_eq!(
            managed_directory(&root, "agent-places").unwrap(),
            root.join("agent-places").canonicalize().unwrap()
        );
        std::fs::write(root.join("agent-searches"), []).unwrap();
        for name in ["agent-searches", "../outside", "a/b", "a\\b", "", ".."] {
            assert!(managed_directory(&root, name).is_err());
        }
    }
    #[cfg(windows)]
    #[test]
    fn msix_correspondence_requires_exact_profile_package_and_relative_path() {
        let local = Path::new(r"C:\Users\Example\AppData\Local");
        let expected = local.join("GeoD").join("runtime").join("agent-places");
        let actual = local
            .join("Packages")
            .join("Example.Package_abc")
            .join("LocalCache")
            .join("Local")
            .join("GeoD/runtime/agent-places");
        assert!(package_cache_matches(
            &expected,
            &actual,
            local,
            "Example.Package_abc"
        ));
        assert!(!package_cache_matches(
            &expected,
            &actual,
            local,
            "Other.Package_abc"
        ));
        assert!(!package_cache_matches(
            &expected,
            &actual.with_file_name("agent-searches"),
            local,
            "Example.Package_abc"
        ));
        assert!(!package_cache_matches(
            &expected,
            &actual,
            Path::new(r"D:\OtherProfile"),
            "Example.Package_abc"
        ));
        assert!(!package_cache_matches(
            &local.join("../outside"),
            &actual,
            local,
            "Example.Package_abc"
        ));
    }
    #[cfg(unix)]
    #[test]
    fn managed_directory_rejects_links_even_inside_the_root() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        std::fs::create_dir(root.join("other")).unwrap();
        std::os::unix::fs::symlink(root.join("other"), root.join("agent-places")).unwrap();
        assert!(managed_directory(&root, "agent-places").is_err());
    }

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
