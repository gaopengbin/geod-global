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

// Preserve actionable transport categories without URLs, SAS queries or OS paths.
pub(crate) fn network_error(error: reqwest::Error) -> String {
    if error.is_timeout() {
        "Asset transfer timed out"
    } else if error.is_connect() {
        "Asset network connection failed"
    } else if error.is_body() || error.is_decode() {
        "Asset network response was incomplete"
    } else {
        "Asset network request failed"
    }
    .into()
}

pub(crate) fn retryable(error: &str) -> bool {
    matches!(
        error,
        "Asset transfer timed out"
            | "Asset network connection failed"
            | "Asset network response was incomplete"
            | "Asset network request failed"
            | "Asset transfer timed out: server stopped sending data for 45 seconds"
    ) || [408, 425, 429, 500, 502, 503, 504].iter().any(|status| {
        error.starts_with(&format!("Asset server returned HTTP {status} "))
            || error.starts_with(&format!("Asset server returned HTTP {status};"))
    })
}

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expected_original_sha256: Option<String>,
}

/// A compiled whole-file identity is independent of HTTP representation tags.
/// It never grants eligibility to a URL or client-provided checksum.
#[derive(Clone, Debug)]
pub(crate) enum Validator {
    StrongEtag {
        etag: String,
        total: u64,
    },
    ReviewedOriginal {
        expected_original_sha256: String,
        total: u64,
    },
}

impl Checkpoint {
    pub(crate) fn validator(&self) -> Validator {
        match self.expected_original_sha256.as_ref() {
            Some(sha256) => Validator::ReviewedOriginal {
                expected_original_sha256: sha256.clone(),
                total: self.total,
            },
            None => Validator::StrongEtag {
                etag: self.etag.clone(),
                total: self.total,
            },
        }
    }
}

const ORIGINAL_KIND: &str = "reviewed-original-sha256";

pub(crate) mod finite_original;

#[cfg(test)]
mod clcd_range_policy_tests {
    use super::*;

    #[test]
    fn large_clcd_ranges_require_the_complete_reviewed_identity() {
        let total = crate::datasets::clcd::ORIGINAL_BYTES;
        assert!(FiniteRange::new(0, total).is_err());
        assert!(FiniteRange::new_clcd(0, total, &"a".repeat(64)).is_err());
        if let Ok(sha) = crate::datasets::clcd::original_sha256() {
            let range = FiniteRange::new_clcd(0, total, sha).unwrap();
            assert_eq!(range.length(), 2 * 1024 * 1024);
            let end = FiniteRange::new_clcd(total - 17, total, sha).unwrap();
            assert_eq!(end.length(), 17);
            assert_eq!(end.end, total - 1);
            assert!(FiniteRange::new_clcd(0, total + 1, sha).is_err());
            assert!(FiniteRange::new_clcd(total, total, sha).is_err());
        }
    }

    fn response(media: &str) -> Response {
        reqwest::Response::from(
            axum::http::Response::builder()
                .status(206)
                .header("Content-Range", "bytes 16-20/21")
                .header("Content-Length", "5")
                .header("Content-Type", media)
                .body(vec![0; 5])
                .unwrap(),
        )
    }

    #[test]
    fn raw_tiff_headers_do_not_relax_the_zip_or_ambiguous_media_contract() {
        let range = FiniteRange {
            start: 16,
            end: 20,
            total: 21,
        };
        assert!(validate_finite_tiff_response(&response("image/tiff, image/tiff"), range).is_ok());
        assert!(validate_finite_response(&response("image/tiff, image/tiff"), range).is_err());
        assert!(validate_finite_tiff_response(
            &response("image/tiff, application/octet-stream"),
            range
        )
        .is_err());
        assert!(validate_finite_tiff_response(&response("image/tiff, text/html"), range).is_err());
        assert!(validate_finite_tiff_response(&response("application/zip"), range).is_err());
        assert!(validate_finite_response(&response("application/zip"), range).is_ok());
    }
}

/// Closed byte intervals used only by the explicitly reviewed original path.
/// Legacy open-ended Range/If-Range behavior remains separate and unchanged.
pub(crate) const CLCD_RANGE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FiniteRange {
    pub(crate) start: u64,
    pub(crate) end: u64,
    pub(crate) total: u64,
}

impl FiniteRange {
    pub(crate) fn new(start: u64, total: u64) -> Result<Self> {
        if start >= total || total > crate::MAX_ASSET_BYTES {
            return Err("Reviewed byte range lies outside its complete original".into());
        }
        Ok(Self {
            start,
            end: start.saturating_add(8 * 1024 * 1024 - 1).min(total - 1),
            total,
        })
    }
    pub(crate) fn new_clcd(start: u64, total: u64, sha256: &str) -> Result<Self> {
        let expected = crate::datasets::clcd::original_sha256()?;
        if sha256 != expected
            || total != crate::datasets::clcd::ORIGINAL_BYTES
            || total > crate::datasets::clcd::MAX_ORIGINAL_BYTES
            || start >= total
        {
            return Err(
                "CLCD byte range requires its exact independently reviewed complete original"
                    .into(),
            );
        }
        Ok(Self {
            start,
            // Measured slow CLCD intervals exceed an 8 MiB/300 s attempt.
            // Smaller exact-source windows retain all time and SHA guards and
            // reduce buffered payload; ordinary SinoLC remains at 8 MiB.
            end: start.saturating_add(CLCD_RANGE_BYTES - 1).min(total - 1),
            total,
        })
    }
    pub(crate) fn length(self) -> usize {
        (self.end - self.start + 1) as usize
    }
    pub(crate) fn header(self) -> String {
        format!("bytes={}-{}", self.start, self.end)
    }
}

pub(crate) fn validate_finite_response(response: &Response, requested: FiniteRange) -> Result<()> {
    validate_finite_response_inner(response, requested, false)
}

pub(crate) fn validate_finite_tiff_response(
    response: &Response,
    requested: FiniteRange,
) -> Result<()> {
    validate_finite_response_inner(response, requested, true)
}

fn validate_finite_response_inner(
    response: &Response,
    requested: FiniteRange,
    raw_tiff: bool,
) -> Result<()> {
    let h = response.headers();
    if response.status() != StatusCode::PARTIAL_CONTENT {
        return Err(format!(
            "Reviewed original segment returned HTTP {} instead of 206",
            response.status().as_u16()
        ));
    }
    if h.get_all(header::CONTENT_RANGE).iter().count() != 1
        || h.get(header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .and_then(range)
            != Some((requested.start, requested.end, requested.total))
    {
        return Err(
            "Reviewed original segment has a missing, ambiguous or changed Content-Range".into(),
        );
    }
    if h.get_all(header::CONTENT_LENGTH).iter().count() != 1
        || h.get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(number)
            != Some(requested.length() as u64)
        || response.content_length() != Some(requested.length() as u64)
    {
        return Err(
            "Reviewed original segment has a missing, ambiguous or changed Content-Length".into(),
        );
    }
    if h.contains_key(header::TRANSFER_ENCODING)
        || h.get_all(header::CONTENT_ENCODING).iter().count() > 1
        || h.get(header::CONTENT_ENCODING)
            .is_some_and(|v| v != "identity")
    {
        return Err(
            "Reviewed original segment has an unsupported or ambiguous transfer/content encoding"
                .into(),
        );
    }
    let types = h.get_all(header::CONTENT_TYPE).iter().collect::<Vec<_>>();
    if raw_tiff {
        // The exact Zenodo TIFF repeats image/tiff, sometimes within a single
        // comma-combined header. All repetitions must agree; mixed types,
        // multipart, HTML and missing declarations remain rejected.
        let mut media: Option<&str> = None;
        for value in types {
            let value = value
                .to_str()
                .map_err(|_| "CLCD segment Content-Type is invalid")?;
            for part in value.split(',').map(str::trim) {
                if !matches!(
                    part,
                    "image/tiff" | "image/geotiff" | "application/octet-stream"
                ) || media.is_some_and(|previous| previous != part)
                {
                    return Err(
                        "CLCD segment has a missing, conflicting or non-TIFF Content-Type".into(),
                    );
                }
                media = Some(part);
            }
        }
        if media.is_none() {
            return Err("CLCD segment has no original TIFF Content-Type".into());
        }
        return Ok(());
    }
    // The native Zenodo response repeats application/octet-stream verbatim.
    // Conflicting declarations, HTML, multipart and absent types remain invalid.
    let media = types.first().and_then(|v| v.to_str().ok()).map(str::trim);
    if !matches!(media, Some("application/zip" | "application/octet-stream"))
        || types
            .iter()
            .any(|v| v.to_str().ok().map(str::trim) != media)
    {
        return Err(
            "Reviewed original segment has a missing, conflicting or non-ZIP Content-Type".into(),
        );
    }
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
}

fn original_binding(job: &Job, sha256: &str, total: u64) -> Result<String> {
    let bytes =
        serde_json::to_vec(&(ORIGINAL_KIND, binding(job)?, sha256, total)).map_err(io_error)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
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
        &job.agent_approval,
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

pub(crate) fn strong_etag(value: &str) -> bool {
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
    let representation_matches = match (
        receipt.schema.as_str(),
        receipt.kind.as_deref(),
        receipt.expected_original_sha256.as_deref(),
    ) {
        ("geod-partial-transfer/v1", None, None) => {
            receipt.binding == binding(job)? && strong_etag(&receipt.etag)
        }
        ("geod-partial-transfer/v2", Some(ORIGINAL_KIND), Some(expected))
            if receipt.etag.is_empty() && valid_sha256(expected) =>
        {
            let reviewed = crate::stac::reviewed_whole_original(root, job)?
                .ok_or("A whole-original checkpoint requires a compiled reviewed source")?;
            reviewed.bytes == receipt.total
                && reviewed.sha256 == expected
                && receipt.binding == original_binding(job, expected, receipt.total)?
        }
        _ => false,
    };
    if !representation_matches
        || receipt.bytes == 0
        || receipt.bytes >= receipt.total
        || receipt.total > limit
        || size < receipt.bytes
        || size > limit
        || !valid_sha256(&receipt.sha256)
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
        Some(pin) if pin.expected_original_sha256.is_some() => {
            builder.header(header::RANGE, format!("bytes={}-", pin.bytes))
        }
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
    let validator_matches = if checkpoint.expected_original_sha256.is_some() {
        // SHA-bound recovery has no If-Range semantics. Length and range must
        // be explicit; final whole-file validation decides source identity.
        headers.get_all(header::CONTENT_LENGTH).iter().count() == 1
            && headers
                .get(header::CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(number)
                == Some(checkpoint.total - checkpoint.bytes)
    } else {
        headers.get_all(header::ETAG).iter().count() == 1
            && headers.get(header::ETAG).and_then(|v| v.to_str().ok())
                == Some(checkpoint.etag.as_str())
    };
    response.status() == StatusCode::PARTIAL_CONTENT
        && validator_matches
        && headers.get_all(header::CONTENT_RANGE).iter().count() == 1
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

pub(crate) fn validator(
    response: &Response,
    supported: bool,
    root: &Path,
    job: &Job,
) -> Result<Option<Validator>> {
    if !supported
        || response
            .headers()
            .get(header::CONTENT_ENCODING)
            .is_some_and(|v| v != "identity")
    {
        return Ok(None);
    }
    if let Some(reviewed) = crate::stac::reviewed_whole_original(root, job)? {
        if !valid_sha256(&reviewed.sha256)
            || reviewed.bytes == 0
            || response.status() != StatusCode::OK
            || response.headers().contains_key(header::CONTENT_RANGE)
            || response
                .headers()
                .get_all(header::CONTENT_LENGTH)
                .iter()
                .count()
                != 1
            || response.content_length() != Some(reviewed.bytes)
        {
            return Err("Reviewed whole-original response differs from its compiled byte length or complete representation".into());
        }
        return Ok(Some(Validator::ReviewedOriginal {
            expected_original_sha256: reviewed.sha256,
            total: reviewed.bytes,
        }));
    }
    if response.headers().get_all(header::ETAG).iter().count() != 1 {
        return Ok(None);
    }
    let Some(etag) = response
        .headers()
        .get(header::ETAG)
        .and_then(|v| v.to_str().ok())
    else {
        return Ok(None);
    };
    let Some(total) = response.content_length() else {
        return Ok(None);
    };
    Ok(
        (strong_etag(etag) && total > 0).then(|| Validator::StrongEtag {
            etag: etag.to_owned(),
            total,
        }),
    )
}

pub(crate) async fn checkpoint(
    root: &Path,
    job: &Job,
    file: &tokio::fs::File,
    validator: Option<&Validator>,
    bytes: u64,
    hasher: &Sha256,
    header_bytes: &[u8],
) -> Result<()> {
    let Some(validator) = validator else {
        return Ok(());
    };
    let (schema, kind, expected_original_sha256, etag, total, source_binding) = match validator {
        Validator::StrongEtag { etag, total } => {
            if !strong_etag(etag) {
                return Err("Invalid strong ETag checkpoint validator".into());
            }
            (
                "geod-partial-transfer/v1",
                None,
                None,
                etag.clone(),
                *total,
                binding(job)?,
            )
        }
        Validator::ReviewedOriginal {
            expected_original_sha256,
            total,
        } => {
            let reviewed = crate::stac::reviewed_whole_original(root, job)?
                .ok_or("A whole-original checkpoint requires a compiled reviewed source")?;
            if reviewed.bytes != *total
                || reviewed.sha256 != *expected_original_sha256
                || !valid_sha256(expected_original_sha256)
            {
                return Err(
                    "Whole-original checkpoint differs from its compiled native source".into(),
                );
            }
            (
                "geod-partial-transfer/v2",
                Some(ORIGINAL_KIND.into()),
                Some(expected_original_sha256.clone()),
                String::new(),
                *total,
                original_binding(job, expected_original_sha256, *total)?,
            )
        }
    };
    if bytes == 0 || bytes >= total || verify_signature(header_bytes, &job.media_type).is_err() {
        return Ok(());
    }
    file.sync_all().await.map_err(io_error)?;
    let receipt = Checkpoint {
        schema: schema.into(),
        binding: source_binding,
        etag,
        bytes,
        total,
        sha256: format!("{:x}", hasher.clone().finalize()),
        kind,
        expected_original_sha256,
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
