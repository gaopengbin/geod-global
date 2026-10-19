//! Whole publisher ZIP transfer + byte-preserving selected numerical TIFF.
//! Only native reviewed dataset provenance can activate this adapter. Ordinary
//! STAC archives, caller filenames and the independent SRTM HGT path cannot.
use super::*;
use std::{
    fs::File,
    io::{Seek, SeekFrom},
    time::Instant,
};
use tokio_util::sync::CancellationToken;

const MAX_MEMBERS: usize = 256;
const MAX_BYTES: u64 = crate::MAX_ASSET_BYTES;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ArchiveSelection {
    pub member: String,
    pub archive_bytes: u64,
    pub archive_sha256: String,
    pub member_bytes: u64,
    pub member_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MemberReceipt {
    pub schema_version: u32,
    pub job_id: String,
    pub source_pin: SourcePin,
    pub source_provenance: crate::datasets::SourceProvenance,
    pub archive_url: String,
    pub archive_file: String,
    pub member_file: String,
    pub selection: ArchiveSelection,
}

impl ArchiveSelection {
    fn validate(&self) -> Result<()> {
        safe_name(&self.member)?;
        let envi = crate::datasets::enmap::is_selection(self);
        if (!self.member.to_ascii_lowercase().ends_with(".tif") && !envi)
            || self.archive_bytes == 0
            || self.archive_bytes > MAX_BYTES
            || self.member_bytes == 0
            || (self.member_bytes > MAX_BYTES && !envi)
            || !digest(&self.archive_sha256)
            || !digest(&self.member_sha256)
        {
            return Err(
                "Reviewed original ZIP/member size, SHA-256 or TIFF identity is invalid".into(),
            );
        }
        Ok(())
    }
}

/// All recipes are compiled reviewed client records, never supplied by an API
/// caller. Revalidation reconstructs the original resolver/provenance first.
pub(crate) fn from_proof(
    proof: &crate::datasets::SourceProvenance,
    url: &str,
) -> Result<Option<ArchiveSelection>> {
    let selection = match proof.dataset.id.as_str() {
        "fabdem" => {
            let p = crate::datasets::fabdem::archive_selection(proof, url)?;
            Some(ArchiveSelection {
                member: p.member.into(),
                archive_bytes: p.archive_bytes,
                archive_sha256: p.archive_sha256.into(),
                member_bytes: p.member_bytes,
                member_sha256: p.member_sha256.into(),
            })
        }
        "glc-fcs10" => Some(crate::datasets::glc_fcs10::archive_selection(proof, url)?),
        "glc-fcs30d" => Some(crate::datasets::glc_fcs30d::archive_selection(proof, url)?),
        "sinolc-1" => Some(crate::datasets::sinolc::archive_selection(proof, url)?),
        "enmap" => Some(crate::datasets::enmap::archive_selection(proof, url)?),
        "gebco-tid" => Some(crate::datasets::gebco_tid::archive_selection(proof, url)?),
        // Added only when its complete author ZIP and member profile exist.
        _ => None,
    };
    if let Some(p) = &selection {
        p.validate()?;
    }
    Ok(selection)
}

pub(crate) fn validate_receipt(
    proof: &crate::datasets::SourceProvenance,
    url: &str,
    raw: &Value,
) -> Result<Option<ArchiveSelection>> {
    let expected = from_proof(proof, url)?;
    let stored = raw
        .get("reviewedArchive")
        .map(|v| serde_json::from_value::<ArchiveSelection>(v.clone()).map_err(io_error))
        .transpose()?;
    if stored != expected {
        return Err(
            "Reviewed publisher ZIP/member recipe changed in the immutable source receipt".into(),
        );
    }
    Ok(expected)
}

fn job_selection(
    root: &Path,
    job: &Job,
) -> Result<Option<(ArchiveSelection, crate::datasets::SourceProvenance)>> {
    let Some(pin) = &job.stac_source else {
        return Ok(None);
    };
    validate_job(root, job)?;
    let (_, snapshot) = record(root, &pin.snapshot_id)?;
    let asset = snapshot
        .assets
        .iter()
        .find(|a| a.key == pin.asset_key)
        .ok_or("Original archive asset is missing")?;
    let Some(raw) = asset.metadata.get("datasetPreset") else {
        return Ok(None);
    };
    let proof: crate::datasets::SourceProvenance =
        serde_json::from_value(raw.clone()).map_err(io_error)?;
    crate::datasets::validate_provenance(&proof, &job.href)?;
    Ok(validate_receipt(&proof, &job.href, &asset.metadata)?.map(|s| (s, proof)))
}

pub(crate) fn reviewed_original(root: &Path, job: &Job) -> Result<Option<ArchiveSelection>> {
    Ok(job_selection(root, job)?.map(|(selection, _)| selection))
}

fn safe_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 240
        || !name.is_ascii()
        || name.contains(['\\', ':'])
        || name.chars().any(char::is_control)
        || name.split('/').any(|s| {
            s.is_empty()
                || s == "."
                || s == ".."
                || s.ends_with(['.', ' '])
                || matches!(
                    s.split('.')
                        .next()
                        .unwrap_or("")
                        .to_ascii_uppercase()
                        .as_str(),
                    "CON"
                        | "PRN"
                        | "AUX"
                        | "NUL"
                        | "COM1"
                        | "COM2"
                        | "COM3"
                        | "COM4"
                        | "COM5"
                        | "COM6"
                        | "COM7"
                        | "COM8"
                        | "COM9"
                        | "LPT1"
                        | "LPT2"
                        | "LPT3"
                        | "LPT4"
                        | "LPT5"
                        | "LPT6"
                        | "LPT7"
                        | "LPT8"
                        | "LPT9"
                )
        })
    {
        return Err("Original ZIP contains an unsafe or unsupported member name".into());
    }
    Ok(())
}

fn check(cancel: Option<&CancellationToken>, deadline: Instant) -> Result<()> {
    crate::raster::check_cancel(cancel)?;
    if Instant::now() > deadline {
        return Err("Original ZIP verification exceeded its bounded time limit".into());
    }
    Ok(())
}

fn hash_file(
    file: &mut File,
    bytes: u64,
    sha: &str,
    cancel: Option<&CancellationToken>,
    deadline: Instant,
) -> Result<()> {
    if file.metadata().map_err(io_error)?.len() != bytes
        || bytes == 0
        || (bytes > MAX_BYTES && !crate::datasets::enmap::is_member(bytes, sha))
    {
        return Err("Original ZIP/member complete byte count changed or exceeds 512 MiB".into());
    }
    let before = file.metadata().map_err(io_error)?.modified().ok();
    file.seek(SeekFrom::Start(0)).map_err(io_error)?;
    let mut hasher = Sha256::new();
    let mut count = 0u64;
    let mut buf = [0u8; 65536];
    loop {
        check(cancel, deadline)?;
        let n = file.read(&mut buf).map_err(io_error)?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count > bytes {
            return Err("Original grew during verification".into());
        }
        hasher.update(&buf[..n]);
    }
    if count != bytes
        || format!("{:x}", hasher.finalize()) != sha
        || before != file.metadata().map_err(io_error)?.modified().ok()
    {
        return Err("Original ZIP/member complete SHA-256 changed".into());
    }
    file.seek(SeekFrom::Start(0)).map_err(io_error)?;
    Ok(())
}

/// Extract only the complete reviewed member; never use its name as a local
/// path. Reading through EOF performs ZIP CRC checking before byte acceptance.
fn extract<R: Read + Seek, W: Write>(
    reader: R,
    expected: &ArchiveSelection,
    output: &mut W,
    cancel: Option<&CancellationToken>,
    deadline: Instant,
) -> Result<()> {
    expected.validate()?;
    let envi = crate::datasets::enmap::is_selection(expected);
    let tid = crate::datasets::gebco_tid::is_selection(expected);
    let expanded_limit = if envi {
        crate::datasets::enmap::INDEX.expanded_bytes
    } else if tid {
        crate::datasets::gebco_tid::INDEX.expanded_bytes
    } else {
        MAX_BYTES
    };
    let mut zip =
        zip::ZipArchive::new(reader).map_err(|e| format!("Invalid original ZIP directory: {e}"))?;
    if zip.is_empty() || zip.len() > MAX_MEMBERS {
        return Err("Original ZIP exceeds its bounded 256-member directory".into());
    }
    let mut names = BTreeSet::new();
    let mut expanded = 0u64;
    let mut compressed = 0u64;
    let mut selected = None;
    let mut spans = Vec::new();
    for i in 0..zip.len() {
        check(cancel, deadline)?;
        let entry = zip
            .by_index(i)
            .map_err(|_| "Original ZIP member is encrypted or unsupported")?;
        let name = if entry.is_dir() {
            entry.name().strip_suffix('/').unwrap_or(entry.name())
        } else {
            entry.name()
        };
        safe_name(name)?;
        if !names.insert(name.to_ascii_lowercase())
            || entry.is_symlink()
            || entry.encrypted()
            || entry
                .unix_mode()
                .is_some_and(|v| v & 0o170000 != 0 && !matches!(v & 0o170000, 0o040000 | 0o100000))
            || !matches!(
                entry.compression(),
                zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
            )
        {
            return Err(
                "Original ZIP has duplicate, linked, encrypted or unsupported members".into(),
            );
        }
        expanded = expanded
            .checked_add(entry.size())
            .ok_or("ZIP expansion overflow")?;
        compressed = compressed
            .checked_add(entry.compressed_size())
            .ok_or("ZIP compressed size overflow")?;
        if expanded > expanded_limit || compressed > expected.archive_bytes {
            return Err(
                "Original ZIP expansion exceeds its ordinary or exact reviewed archive-byte budget"
                    .into(),
            );
        }
        if entry.is_dir() && entry.size() != 0 {
            return Err("Original ZIP directory member unexpectedly contains file data".into());
        }
        if !entry.is_dir() {
            let start = entry
                .data_start()
                .ok_or("Original ZIP member has no exact data offset")?;
            let end = start
                .checked_add(entry.compressed_size())
                .ok_or("Original ZIP member byte interval overflow")?;
            if end > expected.archive_bytes {
                return Err("Original ZIP member byte interval exceeds the whole archive".into());
            }
            spans.push((start, end));
        }
        if entry.name() == expected.member {
            if entry.is_dir() || entry.size() != expected.member_bytes {
                return Err(
                    "Selected original TIFF member identity or complete byte length changed".into(),
                );
            }
            selected = Some(i);
        }
    }
    spans.sort_unstable();
    if spans.windows(2).any(|v| v[0].1 > v[1].0) {
        return Err("Original ZIP has overlapping member data intervals".into());
    }
    let selected =
        selected.ok_or("Reviewed numerical TIFF member is absent from the original ZIP")?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    let mut signature = Vec::new();
    let mut header_seen = false;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(io_error)?;
        let is_header = envi && entry.name() == crate::datasets::enmap::INDEX.header.name;
        let mut header_hash = Sha256::new();
        let declared = entry.size();
        let mut count = 0u64;
        loop {
            check(cancel, deadline)?;
            let n = entry
                .read(&mut buf)
                .map_err(|e| format!("Original ZIP member CRC/decompression failed: {e}"))?;
            if n == 0 {
                break;
            }
            count += n as u64;
            if count > declared || count > expanded_limit {
                return Err("Original ZIP member exceeded its declared complete length".into());
            }
            if index == selected {
                if signature.len() < 8 {
                    signature.extend(buf.iter().take(8 - signature.len()));
                }
                hasher.update(&buf[..n]);
                output.write_all(&buf[..n]).map_err(io_error)?;
            }
            if is_header {
                header_hash.update(&buf[..n]);
            }
        }
        if count != declared {
            return Err("Original ZIP member is truncated".into());
        }
        if is_header {
            if count != crate::datasets::enmap::INDEX.header.bytes
                || format!("{:x}", header_hash.finalize())
                    != crate::datasets::enmap::INDEX.header.sha256
            {
                return Err("EnMAP original HDR complete identity changed".into());
            }
            header_seen = true;
        }
    }
    if envi && (!header_seen || expanded != expanded_limit) {
        return Err("EnMAP original header or complete expanded directory changed".into());
    }
    if tid && expanded != expanded_limit {
        return Err("GEBCO TID complete expanded directory changed".into());
    }
    if format!("{:x}", hasher.finalize()) != expected.member_sha256 {
        return Err(
            "Extracted whole original TIFF SHA-256 or length differs from its reviewed member"
                .into(),
        );
    }
    if !envi
        && !matches!(
            signature.get(..4),
            Some(b"II*\0" | b"MM\0*" | b"II+\0" | b"MM\0+")
        )
    {
        return Err("Reviewed original ZIP member does not contain TIFF/BigTIFF bytes".into());
    }
    Ok(())
}

fn receipt(
    root: &Path,
    job: &Job,
    selection: ArchiveSelection,
    proof: crate::datasets::SourceProvenance,
) -> Result<MemberReceipt> {
    if Uuid::parse_str(&job.id)
        .ok()
        .map(|id| id.to_string())
        .as_deref()
        != Some(job.id.as_str())
    {
        return Err("Invalid original archive job identifier".into());
    }
    let _assets = storage::managed_directory(root, "assets")?;
    Ok(MemberReceipt {
        schema_version: 1,
        job_id: job.id.clone(),
        source_pin: job
            .stac_source
            .clone()
            .ok_or("Archive source pin is missing")?,
        source_provenance: proof,
        archive_url: job.href.clone(),
        archive_file: format!("{}.zip", job.id),
        member_file: format!(
            "{}.member.{}",
            job.id,
            if crate::datasets::enmap::is_selection(&selection) {
                "bsq"
            } else {
                "tif"
            }
        ),
        selection,
    })
}

fn prepare(
    root: &Path,
    job: &Job,
    path: &Path,
    cancel: &CancellationToken,
    selection: ArchiveSelection,
    proof: crate::datasets::SourceProvenance,
) -> Result<()> {
    let assets = storage::managed_directory(root, "assets")?;
    let manifest = receipt(root, job, selection.clone(), proof)?;
    let deadline = Instant::now() + Duration::from_secs(60);
    let path = storage::regular_file(path)?;
    if path.parent() != Some(assets.as_path())
        || path.file_name().and_then(|v| v.to_str()) != Some(format!("{}.part", job.id).as_str())
    {
        return Err(
            "Original archive preparation requires its exact managed partial-transfer file".into(),
        );
    }
    let mut file = File::open(path).map_err(io_error)?;
    fs2::FileExt::try_lock_shared(&file).map_err(|_| "Original archive is being modified")?;
    hash_file(
        &mut file,
        selection.archive_bytes,
        &selection.archive_sha256,
        Some(cancel),
        deadline,
    )?;
    let destination = assets.join(&manifest.member_file);
    if destination.exists() {
        extract(
            file,
            &selection,
            &mut std::io::sink(),
            Some(cancel),
            deadline,
        )?;
        let mut existing =
            File::open(storage::exact_file(&destination, &destination)?).map_err(io_error)?;
        fs2::FileExt::try_lock_shared(&existing)
            .map_err(|_| "Original TIFF member is being modified")?;
        hash_file(
            &mut existing,
            selection.member_bytes,
            &selection.member_sha256,
            Some(cancel),
            deadline,
        )?;
    } else {
        if fs2::available_space(&assets).map_err(io_error)? < selection.member_bytes + 65536 {
            return Err("Insufficient workspace space for the whole original TIFF member".into());
        }
        let mut output = tempfile::Builder::new()
            .prefix(&format!("{}.archive-", job.id))
            .suffix(".part")
            .tempfile_in(&assets)
            .map_err(io_error)?;
        extract(file, &selection, &mut output, Some(cancel), deadline)?;
        output.as_file().sync_all().map_err(io_error)?;
        check(Some(cancel), deadline)?;
        output
            .persist_noclobber(&destination)
            .map_err(|e| io_error(e.error))?;
    }
    let manifest_path = assets.join(format!("{}.archive-member.json", job.id));
    let bytes = serde_json::to_vec_pretty(&manifest).map_err(io_error)?;
    if manifest_path.exists() {
        let saved: MemberReceipt =
            serde_json::from_slice(&read_file(&manifest_path, 65536)?).map_err(io_error)?;
        if saved != manifest {
            return Err("Original archive/member receipt changed".into());
        }
    } else {
        let mut output = tempfile::Builder::new()
            .prefix(&format!("{}.receipt-", job.id))
            .suffix(".part")
            .tempfile_in(&assets)
            .map_err(io_error)?;
        output.write_all(&bytes).map_err(io_error)?;
        output.as_file().sync_all().map_err(io_error)?;
        check(Some(cancel), deadline)?;
        output
            .persist_noclobber(&manifest_path)
            .map_err(|e| io_error(e.error))?;
    }
    Ok(())
}

/// Called after complete resumable ZIP transfer validation, before success is
/// committed. Partial network transfers keep the existing native state machine.
pub(crate) async fn prepare_download(
    root: &Path,
    job: &Job,
    path: &Path,
    cancel: &CancellationToken,
) -> Result<bool> {
    let Some((selection, proof)) = job_selection(root, job)? else {
        return Ok(false);
    };
    let root = root.to_owned();
    let job = job.clone();
    let path = path.to_owned();
    let cancel = cancel.clone();
    tokio::task::spawn_blocking(move || prepare(&root, &job, &path, &cancel, selection, proof))
        .await
        .map_err(io_error)??;
    Ok(true)
}

/// The input remains the independently verified original ZIP. Returned raster
/// bytes and its SHA identify the separately retained unchanged TIFF member.
pub(crate) fn raster_original(root: &Path, job: &Job, original: File) -> Result<(File, String)> {
    let Some((selection, proof)) = job_selection(root, job)? else {
        return Ok((
            original,
            job.sha256.clone().ok_or("Original source SHA is missing")?,
        ));
    };
    if job.sha256.as_deref() != Some(selection.archive_sha256.as_str())
        || job.bytes_downloaded != selection.archive_bytes
    {
        return Err("Original ZIP job differs from its immutable complete archive receipt".into());
    }
    let manifest = receipt(root, job, selection.clone(), proof)?;
    let assets = storage::managed_directory(root, "assets")?;
    let saved: MemberReceipt = serde_json::from_slice(&read_file(
        &assets.join(format!("{}.archive-member.json", job.id)),
        65536,
    )?)
    .map_err(io_error)?;
    if saved != manifest {
        return Err(
            "Extracted original TIFF receipt differs from its source pin or dual hashes".into(),
        );
    }
    let path = assets.join(&manifest.member_file);
    let mut file = File::open(storage::exact_file(&path, &path)?).map_err(io_error)?;
    fs2::FileExt::try_lock_shared(&file).map_err(|_| "Original TIFF member is being modified")?;
    hash_file(
        &mut file,
        selection.member_bytes,
        &selection.member_sha256,
        None,
        Instant::now() + Duration::from_secs(60),
    )?;
    drop(original);
    Ok((file, selection.member_sha256))
}

pub(crate) fn native_receipt(root: &Path, job: &Job) -> Result<Option<MemberReceipt>> {
    let Some((selection, proof)) = job_selection(root, job)? else {
        return Ok(None);
    };
    let expected = receipt(root, job, selection, proof)?;
    let assets = storage::managed_directory(root, "assets")?;
    let saved: MemberReceipt = serde_json::from_slice(&read_file(
        &assets.join(format!("{}.archive-member.json", job.id)),
        65536,
    )?)
    .map_err(io_error)?;
    if saved != expected {
        return Err(
            "Original ZIP/member local receipt differs from its immutable source pin and recipe"
                .into(),
        );
    }
    Ok(Some(saved))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    fn zip_file(names: &[&str], data: &[u8]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for name in names {
            writer
                .start_file(
                    *name,
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Stored),
                )
                .unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
    fn profile(bytes: &[u8], member: &str, data: &[u8]) -> ArchiveSelection {
        ArchiveSelection {
            member: member.into(),
            archive_bytes: bytes.len() as u64,
            archive_sha256: format!("{:x}", Sha256::digest(bytes)),
            member_bytes: data.len() as u64,
            member_sha256: format!("{:x}", Sha256::digest(data)),
        }
    }
    #[test]
    fn selected_whole_bytes_crc_and_dual_identity_are_preserved() {
        let data = b"II*\0original numerical member";
        let bytes = zip_file(&["a.tif", "b.tif"], data);
        let p = profile(&bytes, "b.tif", data);
        let mut out = Vec::new();
        extract(
            Cursor::new(&bytes),
            &p,
            &mut out,
            None,
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(out, data);
        let mut wrong = p.clone();
        wrong.member_sha256 = "0".repeat(64);
        assert!(extract(
            Cursor::new(&bytes),
            &wrong,
            &mut Vec::new(),
            None,
            Instant::now() + Duration::from_secs(1)
        )
        .is_err());
        let mut damaged = bytes.clone();
        let offset = damaged.windows(data.len()).position(|v| v == data).unwrap();
        damaged[offset + 5] ^= 1;
        let p = profile(&damaged, "a.tif", data);
        assert!(extract(
            Cursor::new(damaged),
            &p,
            &mut Vec::new(),
            None,
            Instant::now() + Duration::from_secs(1)
        )
        .unwrap_err()
        .contains("CRC"));
    }
    #[test]
    fn zip_slip_alias_duplicate_expansion_and_cancel_are_rejected() {
        let data = b"II*\0original";
        for name in [
            "../a.tif", "/a.tif", "C:/a.tif", "a\\b.tif", "a./b.tif", "a /b.tif", "CON.tif",
        ] {
            assert!(safe_name(name).is_err());
        }
        let bytes = zip_file(&["a.tif", "A.TIF"], data);
        let p = profile(&bytes, "a.tif", data);
        assert!(extract(
            Cursor::new(bytes),
            &p,
            &mut Vec::new(),
            None,
            Instant::now() + Duration::from_secs(1)
        )
        .is_err());
        let bytes = zip_file(&["a.tif"], data);
        let mut p = profile(&bytes, "a.tif", data);
        p.member_bytes = MAX_BYTES + 1;
        assert!(extract(
            Cursor::new(&bytes),
            &p,
            &mut Vec::new(),
            None,
            Instant::now() + Duration::from_secs(1)
        )
        .is_err());
        let p = profile(&bytes, "a.tif", data);
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(extract(
            Cursor::new(bytes),
            &p,
            &mut Vec::new(),
            Some(&cancel),
            Instant::now() + Duration::from_secs(1)
        )
        .is_err());
    }

    #[test]
    fn symlink_encrypted_members_and_forged_expansion_are_rejected() {
        let data = b"II*\0numerical";
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("a.tif", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(data).unwrap();
        writer
            .add_symlink(
                "linked.tif",
                "../outside",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let p = profile(&bytes, "a.tif", data);
        assert!(extract(
            Cursor::new(bytes),
            &p,
            &mut Vec::new(),
            None,
            Instant::now() + Duration::from_secs(1)
        )
        .is_err());
        let mut encrypted = zip_file(&["a.tif"], data);
        let central = encrypted
            .windows(4)
            .position(|v| v == b"PK\x01\x02")
            .unwrap();
        encrypted[6] |= 1;
        encrypted[central + 8] |= 1;
        let p = profile(&encrypted, "a.tif", data);
        assert!(extract(
            Cursor::new(encrypted),
            &p,
            &mut Vec::new(),
            None,
            Instant::now() + Duration::from_secs(1)
        )
        .is_err());
        let mut expanded = zip_file(&["a.tif", "padding.dat"], data);
        let central = expanded
            .windows(4)
            .enumerate()
            .filter(|(_, v)| *v == b"PK\x01\x02")
            .next_back()
            .unwrap()
            .0;
        expanded[central + 24..central + 28].copy_from_slice(&(MAX_BYTES as u32 + 1).to_le_bytes());
        let p = profile(&expanded, "a.tif", data);
        assert!(extract(
            Cursor::new(expanded),
            &p,
            &mut Vec::new(),
            None,
            Instant::now() + Duration::from_secs(1)
        )
        .is_err());
    }

    #[tokio::test]
    #[ignore = "requires GEOD_CLIMATE_ORIGINAL_DIR containing the complete publisher FABDEM ZIP"]
    async fn original_fabdem_native_archive_member_receipt_and_raster_readback() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        std::fs::create_dir(root.join("assets")).unwrap();
        std::fs::create_dir(root.join("stac")).unwrap();
        let plan = crate::datasets::resolve(crate::datasets::ResolveRequest {
            dataset_id: "fabdem".into(),
            bounds: [50.1, 9.1, 50.2, 9.2],
            year: None,
            month: None,
            country: None,
            layer: Some("elevation".into()),
            region: None,
            season: None,
        })
        .unwrap();
        let source = &plan.sources[0];
        let href = source.stac.as_ref().unwrap().url.clone();
        let proof=crate::datasets::SourceProvenance{schema_version:2,dataset:plan.dataset.clone(),request:plan.request.clone(),layer:plan.dataset.layers[0].clone(),source_id:source.id.clone(),source_title:source.title.clone(),source_bounds:source.bounds,declaration_kind:"reviewed-client-preset; publisher file bytes and embedded metadata verified separately".into(),catalog_raster:None};
        let selection = from_proof(&proof, &href).unwrap().unwrap();
        let raw = json!({"kind":"reviewed-raster-archive","href":href,"probeStatus":206,"contentRange":format!("bytes 0-15/{}",selection.archive_bytes),"contentLength":16,"headerHex":"504b0304","reviewedArchive":selection,"datasetPreset":proof,"file:size":selection.archive_bytes,"file:checksum":format!("1220{}",selection.archive_sha256)});
        let bytes = serde_json::to_vec(&raw).unwrap();
        let digest = immutable(&root, "document", &bytes).unwrap();
        let snapshot = save_snapshot(
            &root,
            Record {
                version: 1,
                connection_id: Uuid::new_v4().to_string(),
                service_name: source.title.clone(),
                kind: "raster".into(),
                document_url: href,
                document_request: None,
                document_sha256: digest,
                item_index: None,
                retrieved_at: now(),
                metadata_documents: Vec::new(),
                search: None,
                reviewed_dataset: None,
                asset_transports: Vec::new(),
            },
            &raw,
        )
        .unwrap();
        let pin = SourcePin {
            snapshot_id: snapshot.id,
            asset_key: "raster".into(),
        };
        let selected = resolve(&root, &pin).unwrap();
        assert_eq!(selected.media_type, "application/zip");
        let mut job = crate::new_download_job(crate::CreateJobRequest {
            item_id: selected.item_id,
            asset_key: "stac_asset".into(),
            href: selected.href,
            media_type: selected.media_type,
            title: Some(selected.title),
        });
        job.source = selected.service_name;
        job.stac_source = Some(pin);
        let partial = root.join("assets").join(format!("{}.part", job.id));
        let path = PathBuf::from(std::env::var("GEOD_CLIMATE_ORIGINAL_DIR").unwrap())
            .join("N00E050-N10E060_FABDEM_V1-2.zip");
        std::fs::copy(path, &partial).unwrap();
        verify_transfer(
            &root,
            &job,
            selection.archive_bytes,
            &selection.archive_sha256,
        )
        .unwrap();
        assert!(
            prepare_download(&root, &job, &partial, &CancellationToken::new())
                .await
                .unwrap()
        );
        let final_path = root.join("assets").join(format!("{}.zip", job.id));
        std::fs::rename(&partial, &final_path).unwrap();
        job.status = crate::JobStatus::Succeeded;
        job.bytes_downloaded = selection.archive_bytes;
        job.total_bytes = Some(selection.archive_bytes);
        job.sha256 = Some(selection.archive_sha256.clone());
        job.output_path = Some(final_path.to_string_lossy().into_owned());
        let archive = super::super::raster::verified_original(&root, &job).unwrap();
        let (mut member, member_sha) = raster_original(&root, &job, archive).unwrap();
        assert_eq!(member_sha, selection.member_sha256);
        let mut bytes = Vec::new();
        member.read_to_end(&mut bytes).unwrap();
        let inspection = super::super::raster::inspect_local(&bytes, &job.id, &member_sha).unwrap();
        assert_eq!((inspection.width, inspection.height), (3600, 3600));
        assert_eq!(inspection.bounds, source.bounds);
        assert!(inspection.preview_data_url.is_some());
        let value =
            super::super::raster::sample_local(&bytes, &job.id, &member_sha, 1800, 1800).unwrap();
        assert_eq!(value.values, vec![json!(275.6700134277344)]);
        assert_eq!(value.no_data, vec![false]);
        let receipt_path = root
            .join("assets")
            .join(format!("{}.archive-member.json", job.id));
        let saved: MemberReceipt =
            serde_json::from_slice(&std::fs::read(&receipt_path).unwrap()).unwrap();
        assert_eq!(saved.archive_url, job.href);
        assert_eq!(saved.selection.member_sha256, member_sha);
        assert_eq!(
            Some(saved.selection.archive_sha256.as_str()),
            job.sha256.as_deref()
        );
        let mut bad = saved.clone();
        bad.selection.member_sha256 = "0".repeat(64);
        std::fs::write(&receipt_path, serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(raster_original(&root, &job, File::open(final_path).unwrap()).is_err());
    }
}
