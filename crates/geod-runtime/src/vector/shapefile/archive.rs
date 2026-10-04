use super::*;
use std::io::{Cursor, Write};
pub(super) fn safe_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 240
        || name.contains(['\\', ':'])
        || name.chars().any(char::is_control)
        || name
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == ".." || s.ends_with(['.', ' ']))
    {
        return Err("Shapefile ZIP contains an unsafe or unsupported member name".into());
    }
    Ok(())
}
pub(super) fn members(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    if bytes.len() > MAX_BYTES {
        return Err("Shapefile ZIP exceeds 20 MiB".into());
    }
    let mut zip =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| "Invalid Shapefile ZIP archive")?;
    if zip.len() > 256 {
        return Err("Shapefile ZIP exceeds 256 members".into());
    }
    let mut out = BTreeMap::new();
    let mut names = BTreeSet::new();
    let mut size = 0usize;
    for i in 0..zip.len() {
        let mut file = zip
            .by_index(i)
            .map_err(|_| "Shapefile ZIP member is encrypted or unsupported")?;
        let name = file.name().to_string();
        safe_name(if file.is_dir() {
            name.trim_end_matches('/')
        } else {
            &name
        })?;
        if !names.insert(name.trim_end_matches('/').to_lowercase())
            || file.unix_mode().is_some_and(|n| n & 0o170000 == 0o120000)
            || ![
                zip::CompressionMethod::Stored,
                zip::CompressionMethod::Deflated,
            ]
            .contains(&file.compression())
        {
            return Err("Shapefile ZIP has ambiguous, linked or unsupported members".into());
        }
        if file.is_dir() {
            continue;
        }
        let n = usize::try_from(file.size()).map_err(io_error)?;
        size = size.checked_add(n).ok_or("Shapefile ZIP size overflow")?;
        if size > MAX_BYTES {
            return Err("Uncompressed Shapefile members exceed 20 MiB".into());
        }
        let mut data = Vec::with_capacity(n.min(65536));
        (&mut file)
            .take(n as u64 + 1)
            .read_to_end(&mut data)
            .map_err(|_| "Shapefile ZIP CRC or decompression check failed")?;
        if data.len() != n {
            return Err("Shapefile ZIP member size changed".into());
        }
        out.insert(name, data);
    }
    Ok(out)
}
/// Deterministic wrapper. Each source file is unchanged; no producer ZIP is claimed.
pub fn sidecar_bundle(path: &Path) -> Result<Vec<u8>> {
    let path = storage::regular_file(path)?;
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("Shapefile has no Unicode file name")?;
    if !name.to_lowercase().ends_with(".shp") {
        return Err("Choose the .shp member of a Shapefile dataset".into());
    }
    let stem = &name[..name.len() - 4];
    let parent = path.parent().ok_or("Shapefile has no parent directory")?;
    let mut entries = BTreeMap::new();
    let mut folded = BTreeSet::new();
    let mut size = 0usize;
    for (i, item) in std::fs::read_dir(parent).map_err(io_error)?.enumerate() {
        if i >= 10000 {
            return Err("Shapefile folder exceeds the bounded companion scan; place the dataset in its own folder".into());
        }
        let item = item.map_err(io_error)?;
        let Some(n) = item.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let Some(suffix) = n.get(stem.len()..).filter(|_| {
            n.get(..stem.len())
                .is_some_and(|s| s.eq_ignore_ascii_case(stem))
        }) else {
            continue;
        };
        if ![
            ".shp", ".shx", ".dbf", ".prj", ".cpg", ".qix", ".sbn", ".sbx", ".fix", ".shp.xml",
        ]
        .contains(&suffix.to_lowercase().as_str())
        {
            continue;
        }
        safe_name(&n)?;
        if !folded.insert(n.to_lowercase()) {
            return Err("Shapefile folder contains ambiguous companion file names".into());
        }
        let p = storage::regular_file(&item.path())?;
        if p.parent() != Some(parent) {
            return Err("Shapefile companion was redirected outside its source folder".into());
        }
        let data = read(&p)?;
        size += data.len();
        if size > MAX_BYTES {
            return Err("Shapefile companion files exceed 20 MiB".into());
        }
        entries.insert(n, data);
    }
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (n, data) in entries {
        writer
            .start_file(
                n,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .map_err(io_error)?;
        writer.write_all(&data).map_err(io_error)?;
    }
    let bytes = writer.finish().map_err(io_error)?.into_inner();
    if bytes.len() > MAX_BYTES {
        return Err("Shapefile companion bundle exceeds 20 MiB".into());
    }
    Ok(bytes)
}
