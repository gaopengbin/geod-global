//! Conditional byte-range recovery. A partial is never a published asset.
use crate::{io_error, storage, verify_signature, Job, Result};
use reqwest::{header, RequestBuilder, Response, StatusCode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TransferMode {
    Fresh,
    Resumed,
    Restarted,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransferInfo {
    pub mode: TransferMode,
    pub resumed_bytes: u64,
}

impl TransferInfo {
    pub(crate) fn validate(&self, job: &Job) -> Result<()> {
        if job.kind != "download"
            || (self.mode == TransferMode::Resumed) != (self.resumed_bytes > 0)
            || self.resumed_bytes > job.bytes_downloaded
        {
            return Err("Stored transfer mode differs from its download progress".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Checkpoint {
    schema: String,
    binding: String,
    pub(crate) etag: String,
    pub(crate) bytes: u64,
    pub(crate) total: u64,
    sha256: String,
}

pub(crate) struct Prepared {
    pub(crate) checkpoint: Option<Checkpoint>,
    pub(crate) hasher: Sha256,
    pub(crate) header: Vec<u8>,
    pub(crate) bytes: u64,
    pub(crate) restarted: bool,
    pub(crate) file: Option<tokio::fs::File>,
}

struct VerifiedPrefix {
    hasher: Sha256,
    header: Vec<u8>,
    file: std::fs::File,
}

fn binding(job: &Job) -> Result<String> {
    let bytes = serde_json::to_vec(&(
        &job.id,
        &job.kind,
        &job.item_id,
        &job.asset_key,
        &job.href,
        &job.media_type,
        &job.stac_source,
        &job.wcs_source,
    ))
    .map_err(io_error)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn paths(root: &Path, id: &str) -> Result<(PathBuf, PathBuf)> {
    if Uuid::parse_str(id).ok().map(|id| id.to_string()).as_deref() != Some(id) {
        return Err("Invalid partial transfer identifier".into());
    }
    let assets = root.join("assets");
    if assets.canonicalize().map_err(io_error)? != assets {
        return Err("Managed partial directory was redirected".into());
    }
    Ok((
        assets.join(format!("{id}.part")),
        assets.join(format!("{id}.part.resume.json")),
    ))
}

pub(crate) async fn discard(root: &Path, id: &str) -> Result<()> {
    let (partial, receipt) = paths(root, id)?;
    for path in [partial, receipt] {
        match tokio::fs::remove_file(path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(error)),
        }
    }
    Ok(())
}

pub(crate) async fn discard_checkpoint(root: &Path, id: &str) -> Result<()> {
    let (_, receipt) = paths(root, id)?;
    match tokio::fs::remove_file(receipt).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(error)),
    }
}

pub(crate) async fn cleanup_staging(root: &Path) -> Result<()> {
    let assets = root.join("assets");
    if assets.canonicalize().map_err(io_error)? != assets {
        return Err("Managed partial directory was redirected".into());
    }
    let mut entries = tokio::fs::read_dir(assets).await.map_err(io_error)?;
    while let Some(entry) = entries.next_entry().await.map_err(io_error)? {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(ids) = name
            .strip_suffix(".resume-write.part")
            .or_else(|| name.strip_suffix(".resume.json.tmp"))
        else {
            continue;
        };
        let Some((job, temporary)) = ids.split_once('.') else {
            continue;
        };
        if [job, temporary].iter().all(|value| {
            Uuid::parse_str(value)
                .ok()
                .map(|id| id.to_string())
                .as_deref()
                == Some(value)
        }) {
            storage::regular_file(&entry.path())?;
            tokio::fs::remove_file(entry.path())
                .await
                .map_err(io_error)?;
        }
    }
    Ok(())
}

fn strong_etag(value: &str) -> bool {
    let bytes = value.as_bytes();
    (2..=512).contains(&bytes.len())
        && bytes[0] == b'"'
        && bytes[bytes.len() - 1] == b'"'
        && bytes[1..bytes.len() - 1]
            .iter()
            .all(|b| *b == 0x21 || (0x23..=0x7e).contains(b))
}

fn load(root: &Path, job: &Job, limit: u64) -> Result<Checkpoint> {
    let (partial, receipt) = paths(root, &job.id)?;
    storage::regular_file(&receipt)?;
    if std::fs::metadata(&receipt).map_err(io_error)?.len() > 4096 {
        return Err("Partial transfer receipt is too large".into());
    }
    let receipt: Checkpoint = serde_json::from_slice(&std::fs::read(receipt).map_err(io_error)?)
        .map_err(|_| "Invalid partial transfer receipt")?;
    storage::regular_file(&partial)?;
    let size = std::fs::metadata(&partial).map_err(io_error)?.len();
    if receipt.schema != "geod-partial-transfer/v1"
        || receipt.binding != binding(job)?
        || !strong_etag(&receipt.etag)
        || receipt.bytes == 0
        || receipt.bytes >= receipt.total
        || receipt.total > limit
        || size < receipt.bytes
        || size > limit
        || receipt.sha256.len() != 64
        || !receipt
            .sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("Partial transfer no longer matches the source or byte limits".into());
    }
    Ok(receipt)
}

// Startup only identifies a candidate. Its entire prefix is rehashed before a
// retry sends Range; neither this probe nor a saved digest claims it has resumed.
pub(crate) fn candidate(root: &Path, job: &Job, limit: u64) -> bool {
    load(root, job, limit).is_ok()
}

pub(crate) async fn prepare(
    root: &Path,
    job: &Job,
    limit: u64,
    token: &CancellationToken,
) -> Result<Prepared> {
    let (partial, _) = paths(root, &job.id)?;
    let existed = tokio::fs::symlink_metadata(&partial).await.is_ok();
    let checkpoint = load(root, job, limit).ok();
    if let Some(checkpoint) = checkpoint {
        if fs2::available_space(root).map_err(io_error)?
            < checkpoint.bytes.saturating_add(64 * 1024 * 1024)
        {
            return Err("Insufficient workspace disk space to verify the partial download".into());
        }
        let staging =
            partial.with_file_name(format!("{}.{}.resume-write.part", job.id, Uuid::new_v4()));
        let source = partial.clone();
        let destination = staging.clone();
        let pin = checkpoint.clone();
        let cancellation = token.clone();
        // Copy to an exclusively created file. Never append to a pre-existing
        // inode/hard link, and ignore any uncheckpointed tail after a crash.
        let result = tokio::task::spawn_blocking(move || -> Result<Option<VerifiedPrefix>> {
            storage::regular_file(&source)?;
            let mut source = std::fs::File::open(source).map_err(io_error)?;
            let mut target = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(destination)
                .map_err(io_error)?;
            let mut remaining = pin.bytes;
            let mut hasher = Sha256::new();
            let mut header = Vec::new();
            let mut buffer = vec![0; 1024 * 1024];
            while remaining > 0 {
                if cancellation.is_cancelled() {
                    return Err("Transfer cancelled".into());
                }
                let amount = buffer.len().min(remaining as usize);
                source.read_exact(&mut buffer[..amount]).map_err(io_error)?;
                header.extend(
                    buffer[..amount]
                        .iter()
                        .take(16usize.saturating_sub(header.len())),
                );
                hasher.update(&buffer[..amount]);
                target.write_all(&buffer[..amount]).map_err(io_error)?;
                remaining -= amount as u64;
            }
            if format!("{:x}", hasher.clone().finalize()) != pin.sha256 {
                return Ok(None);
            }
            target.sync_all().map_err(io_error)?;
            Ok(Some(VerifiedPrefix {
                hasher,
                header,
                file: target,
            }))
        })
        .await
        .map_err(io_error)
        .and_then(|result| result);
        match result {
            Ok(Some(prefix)) if verify_signature(&prefix.header, &job.media_type).is_ok() => {
                let VerifiedPrefix {
                    hasher,
                    header,
                    file,
                } = prefix;
                if let Err(error) = tokio::fs::rename(&staging, &partial).await {
                    drop(file);
                    let _ = tokio::fs::remove_file(&staging).await;
                    return Err(io_error(error));
                }
                return Ok(Prepared {
                    bytes: checkpoint.bytes,
                    checkpoint: Some(checkpoint),
                    hasher,
                    header,
                    restarted: false,
                    file: Some(tokio::fs::File::from_std(file)),
                });
            }
            Err(error) => {
                let _ = tokio::fs::remove_file(staging).await;
                return Err(error);
            }
            _ => {
                let _ = tokio::fs::remove_file(staging).await;
            }
        }
        if token.is_cancelled() {
            return Err("Transfer cancelled".into());
        }
    }
    discard(root, &job.id).await?;
    Ok(Prepared {
        checkpoint: None,
        hasher: Sha256::new(),
        header: Vec::new(),
        bytes: 0,
        restarted: existed,
        file: None,
    })
}

pub(crate) fn request(builder: RequestBuilder, checkpoint: Option<&Checkpoint>) -> RequestBuilder {
    let builder = builder.header(header::ACCEPT_ENCODING, "identity");
    match checkpoint {
        Some(pin) => builder
            .header(header::RANGE, format!("bytes={}-", pin.bytes))
            .header(header::IF_RANGE, &pin.etag),
        None => builder,
    }
}

fn number(text: &str) -> Option<u64> {
    (!text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()))
        .then(|| text.parse().ok())
        .flatten()
}

fn range(text: &str) -> Option<(u64, u64, u64)> {
    let (span, total) = text.strip_prefix("bytes ")?.split_once('/')?;
    let (from, to) = span.split_once('-')?;
    Some((number(from)?, number(to)?, number(total)?))
}

pub(crate) fn accepts(response: &Response, checkpoint: &Checkpoint) -> bool {
    let headers = response.headers();
    response.status() == StatusCode::PARTIAL_CONTENT
        && headers.get_all(header::ETAG).iter().count() == 1
        && headers.get_all(header::CONTENT_RANGE).iter().count() == 1
        && headers.get(header::ETAG).and_then(|v| v.to_str().ok()) == Some(checkpoint.etag.as_str())
        && headers
            .get(header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .and_then(range)
            == Some((checkpoint.bytes, checkpoint.total - 1, checkpoint.total))
        && response
            .content_length()
            .is_none_or(|length| length == checkpoint.total - checkpoint.bytes)
        && headers
            .get(header::CONTENT_ENCODING)
            .is_none_or(|v| v == "identity")
        && headers
            .get(header::CONTENT_TYPE)
            .is_none_or(|v| !v.as_bytes().starts_with(b"multipart/"))
}

pub(crate) fn validator(response: &Response, supported: bool) -> Option<(String, u64)> {
    if !supported
        || response
            .headers()
            .get(header::CONTENT_ENCODING)
            .is_some_and(|v| v != "identity")
    {
        return None;
    }
    if response.headers().get_all(header::ETAG).iter().count() != 1 {
        return None;
    }
    let etag = response.headers().get(header::ETAG)?.to_str().ok()?;
    let total = response.content_length()?;
    (strong_etag(etag) && total > 0).then(|| (etag.to_owned(), total))
}

pub(crate) async fn checkpoint(
    root: &Path,
    job: &Job,
    file: &tokio::fs::File,
    validator: Option<&(String, u64)>,
    bytes: u64,
    hasher: &Sha256,
    header_bytes: &[u8],
) -> Result<()> {
    let Some((etag, total)) = validator else {
        return Ok(());
    };
    if bytes == 0 || bytes >= *total || verify_signature(header_bytes, &job.media_type).is_err() {
        return Ok(());
    }
    file.sync_all().await.map_err(io_error)?;
    let receipt = Checkpoint {
        schema: "geod-partial-transfer/v1".into(),
        binding: binding(job)?,
        etag: etag.clone(),
        bytes,
        total: *total,
        sha256: format!("{:x}", hasher.clone().finalize()),
    };
    let (_, destination) = paths(root, &job.id)?;
    let pending =
        destination.with_file_name(format!("{}.{}.resume.json.tmp", job.id, Uuid::new_v4()));
    let result = async {
        let mut output = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&pending)
            .await
            .map_err(io_error)?;
        output
            .write_all(&serde_json::to_vec(&receipt).map_err(io_error)?)
            .await
            .map_err(io_error)?;
        output.sync_all().await.map_err(io_error)?;
        drop(output);
        tokio::fs::rename(&pending, destination)
            .await
            .map_err(io_error)
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(pending).await;
    }
    result
}

#[cfg(test)]
mod tests;
