//! Inherited MSIX AppData virtualization outlives the process that launched us.
//! Bind the write namespace to a fresh managed file's OS handle. Package-looking
//! paths and process/environment package labels alone never grant an exemption.
use std::{
    ffi::OsString,
    fs::File,
    os::windows::{ffi::OsStringExt, io::AsRawHandle},
    path::{Component, Path, PathBuf},
};
use windows_sys::Win32::Storage::FileSystem::{
    GetFinalPathNameByHandleW, FILE_NAME_NORMALIZED, VOLUME_NAME_DOS,
};

pub(super) fn matches(root: &Path, expected: &Path, actual: &Path, local: &Path) -> bool {
    let Some(mapped) = write_root(root, local) else {
        return false;
    };
    let Ok(relative) = expected.strip_prefix(root) else {
        return false;
    };
    actual == mapped.join(relative)
}

fn write_root(root: &Path, local: &Path) -> Option<PathBuf> {
    // Create-new with a random name prevents accepting a pre-existing witness.
    // NamedTempFile removes only this owned, empty file on every return path.
    let witness = tempfile::Builder::new()
        .prefix(".geod-namespace-")
        .rand_bytes(16)
        .tempfile_in(root)
        .ok()?;
    let final_file = final_path(witness.as_file())?;
    if super::regular_file(witness.path()).ok()? != final_file {
        return None;
    }
    let mapped = namespace_root(root, witness.path(), &final_file, local)?;
    if super::directory(root).ok()? != root || super::directory(&mapped).ok()? != mapped {
        return None;
    }
    Some(mapped)
}

fn namespace_root(
    root: &Path,
    expected_file: &Path,
    actual_file: &Path,
    local: &Path,
) -> Option<PathBuf> {
    if expected_file.parent()? != root {
        return None;
    }
    let relative = actual_file.strip_prefix(local).ok()?;
    let mut parts = relative.components();
    if parts.next()? != Component::Normal("Packages".as_ref()) {
        return None;
    }
    let Component::Normal(family) = parts.next()? else {
        return None;
    };
    let family = family.to_str()?;
    if family.is_empty()
        || !family
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || [b'.', b'_', b'-'].contains(&b))
    {
        return None;
    }
    if !super::package_cache_matches(expected_file, actual_file, local, family) {
        return None;
    }
    Some(actual_file.parent()?.to_path_buf())
}

fn final_path(file: &File) -> Option<PathBuf> {
    let handle = file.as_raw_handle();
    let flags = FILE_NAME_NORMALIZED | VOLUME_NAME_DOS;
    let needed = unsafe { GetFinalPathNameByHandleW(handle, std::ptr::null_mut(), 0, flags) };
    if !(2..=32768).contains(&needed) {
        return None;
    }
    let mut path = vec![0u16; needed as usize];
    let length = unsafe { GetFinalPathNameByHandleW(handle, path.as_mut_ptr(), needed, flags) };
    if length == 0 || length >= needed {
        return None;
    }
    Some(PathBuf::from(OsString::from_wide(&path[..length as usize])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_witness_binds_only_its_exact_managed_root_and_file_leaf() {
        let local = Path::new(r"C:\Users\Example\AppData\Local");
        let root = local.join("GeoD/runtime");
        let witness = root.join(".geod-namespace-fresh");
        let mapped = local.join("Packages/Example.Package_abc/LocalCache/Local/GeoD/runtime");
        let actual = mapped.join(".geod-namespace-fresh");
        assert_eq!(
            namespace_root(&root, &witness, &actual, local),
            Some(mapped)
        );
        for wrong in [
            actual.with_file_name("other"),
            local.join("Packages/Example.Package_abc/LocalCache/Local/Other/.geod-namespace-fresh"),
            root.join(".geod-namespace-fresh"),
        ] {
            assert!(namespace_root(&root, &witness, &wrong, local).is_none());
        }
        assert!(namespace_root(&root, &witness, &actual, Path::new(r"D:\OtherProfile")).is_none());
        assert!(namespace_root(
            &root,
            &root.join("child/.geod-namespace-fresh"),
            &actual,
            local
        )
        .is_none());
    }

    #[test]
    fn a_live_file_handle_resolves_the_created_file_and_cleanup_leaves_no_witness() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let witness = tempfile::Builder::new()
            .prefix(".geod-namespace-")
            .tempfile_in(&root)
            .unwrap();
        let expected = witness.path().canonicalize().unwrap();
        assert_eq!(final_path(witness.as_file()).unwrap(), expected);
        drop(witness);
        assert!(!expected.exists());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    }
}
