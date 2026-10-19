//! Closed-range recovery for exact compiled SinoLC ZIP and CLCD TIFF originals. Each response
//! is a complete bounded interval; only contiguous complete intervals become
//! durable prefixes. Ordinary transfers retain their existing transport.
use super::{FiniteRange, Prepared, TransferInfo, TransferMode, Validator};
use crate::{io_error, CompletedDownload, Job, JobManager, ProxySettings, Result};
use futures_util::{stream, Stream, StreamExt};
use sha2::Digest;
use std::{future::Future, path::Path, pin::Pin, time::Duration};
use tokio::io::AsyncWriteExt;
use tokio_util::{bytes::Bytes, sync::CancellationToken};

const URL: &str = "https://zenodo.org/records/7709370/files/East_Shanghai.zip";
const TOTAL: u64 = 365_818_022;
const SHA: &str = "f128cae66f38c546a850af767b42b85d29fd0c4d71ee3bb325e30b962d58d268";
const CONCURRENCY: usize = 3;
const MAX_FRAGMENTS: usize = 8192;
const ATTEMPTS: usize = 3;

struct Source {
    url: String,
    total: u64,
    sha: String,
}

fn verify_source_signature(source: &Source, signature: &[u8]) -> Result<()> {
    // Source admission already binds CLCD to its compiled proof and whole SHA.
    // Its first complete interval is a TIFF, while SinoLC retains a ZIP.
    let media = if source.url == crate::datasets::clcd::URL {
        "image/tiff"
    } else {
        "application/zip"
    };
    crate::verify_signature(signature, media)
}

fn source(root: &Path, job: &Job) -> Result<Option<Source>> {
    if job.href == crate::datasets::clcd::URL {
        if job.asset_key != "stac_asset"
            || job
                .stac_source
                .as_ref()
                .is_none_or(|pin| pin.asset_key != "raster")
            || job.wcs_source.is_some()
            || job.agent_approval.is_some()
            || crate::extension(&job.media_type)? != "tif"
        {
            return Ok(None);
        }
        let policy = crate::stac::reviewed_raw_original(root, job)?
            .ok_or("CLCD finite transfer requires its exact compiled native source")?;
        return Ok(Some(Source {
            url: job.href.clone(),
            total: policy.bytes,
            sha: policy.sha256,
        }));
    }
    if job.href != URL
        || job.media_type != "application/zip"
        || job.asset_key != "stac_asset"
        || job
            .stac_source
            .as_ref()
            .is_none_or(|pin| pin.asset_key != "raster")
        || job.wcs_source.is_some()
        || job.agent_approval.is_some()
    {
        return Ok(None);
    }
    let Some(reviewed) = crate::stac::archive_original::reviewed_original(root, job)? else {
        return Ok(None);
    };
    if reviewed.archive_bytes != TOTAL
        || reviewed.archive_sha256 != SHA
        || reviewed.member != "East_Shanghai.tif"
        || reviewed.member_bytes != 427_679_242
        || reviewed.member_sha256 != crate::datasets::sinolc::ORIGINAL_MEMBER_SHA256
    {
        return Ok(None);
    }
    Ok(Some(Source {
        url: job.href.clone(),
        total: TOTAL,
        sha: SHA.into(),
    }))
}

type ResponseFuture<'a> = Pin<Box<dyn Future<Output = Result<reqwest::Response>> + Send + 'a>>;
trait Transport: Sync {
    fn request(&self, range: FiniteRange) -> ResponseFuture<'_>;
}
struct NativeTransport<'a> {
    settings: &'a ProxySettings,
    job: &'a Job,
    root: &'a Path,
}
impl Transport for NativeTransport<'_> {
    fn request(&self, range: FiniteRange) -> ResponseFuture<'_> {
        Box::pin(crate::stac::finite_asset_response(
            self.settings,
            self.job,
            self.root,
            range,
        ))
    }
}

type CommitFuture<'a> = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;
trait Journal {
    fn commit<'a>(&'a mut self, state: &'a CompletedDownload) -> CommitFuture<'a>;
}
struct NativeJournal<'a> {
    manager: &'a JobManager,
    job: &'a Job,
}
impl Journal for NativeJournal<'_> {
    fn commit<'a>(&'a mut self, state: &'a CompletedDownload) -> CommitFuture<'a> {
        Box::pin(async move {
            super::checkpoint(
                &self.manager.inner.root,
                self.job,
                &state.file,
                state.validator.as_ref(),
                state.bytes,
                &state.hasher,
                &state.header,
            )
            .await?;
            self.manager
                .progress(&self.job.id, state.bytes, state.total)
                .await
        })
    }
}

enum Failure {
    Network(String),
    Fatal(String),
}
impl Failure {
    fn message(self) -> String {
        match self {
            Self::Network(v) | Self::Fatal(v) => v,
        }
    }
}
struct Segment {
    range: FiniteRange,
    fragments: Vec<Bytes>,
}

async fn collect<S>(
    stream: S,
    range: FiniteRange,
    token: &CancellationToken,
) -> std::result::Result<Segment, Failure>
where
    S: Stream<Item = std::result::Result<Bytes, reqwest::Error>>,
{
    if token.is_cancelled() {
        return Err(Failure::Fatal("Transfer cancelled".into()));
    }
    futures_util::pin_mut!(stream);
    let mut fragments = Vec::new();
    let mut bytes = 0usize;
    loop {
        let next = tokio::select! {
            _ = token.cancelled() => return Err(Failure::Fatal("Transfer cancelled".into())),
            next = tokio::time::timeout(Duration::from_secs(45), stream.next()) =>
                next.map_err(|_| Failure::Network("Original segment server stopped sending data for 45 seconds".into()))?,
        };
        let Some(next) = next else {
            break;
        };
        let chunk = next.map_err(|e| Failure::Network(e.without_url().to_string()))?;
        if chunk.len() > range.length().saturating_sub(bytes) {
            return Err(Failure::Fatal(
                "Original segment exceeded its exact bounded length".into(),
            ));
        }
        if chunk.is_empty() {
            continue;
        }
        if fragments.len() >= MAX_FRAGMENTS {
            return Err(Failure::Fatal(
                "Original segment exceeded its bounded fragment index".into(),
            ));
        }
        bytes += chunk.len();
        // Keep HTTP Bytes without a second assembled 8 MiB allocation. The
        // three-future window covers active and ready buffers until committed.
        fragments.push(chunk);
    }
    if token.is_cancelled() {
        return Err(Failure::Fatal("Transfer cancelled".into()));
    }
    if bytes != range.length() {
        return Err(Failure::Fatal("Original segment body is truncated".into()));
    }
    Ok(Segment { range, fragments })
}

async fn fetch<T: Transport>(
    transport: &T,
    source: &Source,
    range: FiniteRange,
    token: &CancellationToken,
) -> Result<Segment> {
    for attempt in 0..ATTEMPTS {
        let outcome = async {
            let response = tokio::select! {
                _ = token.cancelled() => return Err(Failure::Fatal("Transfer cancelled".into())),
                response = tokio::time::timeout(Duration::from_secs(30), transport.request(range)) =>
                    response.map_err(|_| Failure::Network("Original segment request timed out".into()))?
                        .map_err(Failure::Network)?,
            };
            if response.url().as_str() != source.url {
                return Err(Failure::Fatal(
                    "Original segment changed its immutable source URL".into(),
                ));
            }
            if matches!(response.status().as_u16(), 408 | 429 | 503) {
                // An error page is never ZIP data. Drop it without reading its
                // body, then use the same bounded retry budget as network errors.
                return Err(Failure::Network(format!(
                    "Original segment source temporarily returned HTTP {}",
                    response.status().as_u16()
                )));
            }
            if source.url == crate::datasets::clcd::URL {
                super::validate_finite_tiff_response(&response, range)
            } else {
                super::validate_finite_response(&response, range)
            }
            .map_err(Failure::Fatal)?;
            collect(response.bytes_stream(), range, token).await
        };
        let result = tokio::select! {
            _ = token.cancelled() => return Err("Transfer cancelled".into()),
            result = tokio::time::timeout(Duration::from_secs(300), outcome) =>
                result.unwrap_or_else(|_| Err(Failure::Network("Original segment transfer timed out".into()))),
        };
        match result {
            Ok(segment) => return Ok(segment),
            Err(Failure::Network(_)) if attempt + 1 < ATTEMPTS => {
                tokio::select! { _ = token.cancelled() => return Err("Transfer cancelled".into()),
                _ = tokio::time::sleep(Duration::from_secs((attempt + 1) as u64)) => {} }
            }
            Err(error) => return Err(error.message()),
        }
    }
    unreachable!()
}

fn ranges(start: u64, total: u64) -> Result<Vec<FiniteRange>> {
    let mut result = Vec::new();
    let mut offset = start;
    while offset < total {
        let range = FiniteRange::new(offset, total)?;
        offset = range.end + 1;
        result.push(range);
    }
    Ok(result)
}

fn source_ranges(start: u64, source: &Source) -> Result<Vec<FiniteRange>> {
    if source.url != crate::datasets::clcd::URL {
        return ranges(start, source.total);
    }
    let mut result = Vec::new();
    let mut offset = start;
    while offset < source.total {
        let range = FiniteRange::new_clcd(offset, source.total, &source.sha)?;
        offset = range.end + 1;
        result.push(range);
    }
    Ok(result)
}

async fn run<T: Transport, J: Journal>(
    source: &Source,
    state: &mut CompletedDownload,
    transport: &T,
    journal: &mut J,
    token: &CancellationToken,
) -> Result<()> {
    let spans = source_ranges(state.bytes, source)?;
    let mut pending = stream::iter(
        spans
            .into_iter()
            .map(|r| fetch(transport, source, r, token)),
    )
    .buffered(CONCURRENCY);
    while let Some(result) = tokio::select! {
        _ = token.cancelled() => return Err("Transfer cancelled".into()),
        next = pending.next() => next,
    } {
        let segment = result?;
        if segment.range.start != state.bytes {
            return Err("Original segment is not the next contiguous prefix".into());
        }
        if state.bytes == 0 {
            let signature = segment
                .fragments
                .iter()
                .flat_map(|b| b.iter().copied())
                .take(16)
                .collect::<Vec<_>>();
            verify_source_signature(source, &signature)?;
        }
        for fragment in segment.fragments {
            if token.is_cancelled() {
                return Err("Transfer cancelled".into());
            }
            state.file.write_all(&fragment).await.map_err(io_error)?;
            state.header.extend(
                fragment
                    .iter()
                    .take(16usize.saturating_sub(state.header.len())),
            );
            state.hasher.update(&fragment);
            state.bytes = state
                .bytes
                .checked_add(fragment.len() as u64)
                .ok_or("Original segment byte arithmetic overflow")?;
        }
        if state.bytes != segment.range.end + 1 {
            return Err("Original segment commit length changed".into());
        }
        state.file.sync_all().await.map_err(io_error)?;
        journal.commit(state).await?;
        // The yielded segment is consumed before polling pending again, so
        // completed+active payload never exceeds three 8 MiB intervals.
    }
    if token.is_cancelled() {
        return Err("Transfer cancelled".into());
    }
    if state.bytes != source.total || format!("{:x}", state.hasher.clone().finalize()) != source.sha
    {
        return Err("Complete bounded original length or SHA-256 differs from its immutable reviewed source".into());
    }
    Ok(())
}

async fn discard_invalid_complete_checkpoint(
    root: &Path,
    job: &Job,
    source: &Source,
    state: &CompletedDownload,
) -> Result<()> {
    if state.bytes == source.total && format!("{:x}", state.hasher.clone().finalize()) != source.sha
    {
        super::discard_checkpoint(root, &job.id).await?;
    }
    Ok(())
}

pub(crate) async fn download(
    manager: &JobManager,
    settings: &ProxySettings,
    job: &Job,
    prepared: &mut Prepared,
    token: &CancellationToken,
) -> Result<Option<CompletedDownload>> {
    let Some(source) = source(&manager.inner.root, job)? else {
        return Ok(None);
    };
    if token.is_cancelled() {
        return Err("Transfer cancelled".into());
    }
    if source.total > crate::source_transfer_limit_at(&manager.inner.root, job)?
        || prepared.bytes > source.total
    {
        return Err("Reviewed finite original exceeds its unchanged source budget".into());
    }
    if let Some(pin) = &prepared.checkpoint {
        if !matches!(pin.validator(), Validator::ReviewedOriginal { expected_original_sha256, total }
            if expected_original_sha256 == source.sha && total == source.total)
        {
            return Err("Reviewed finite original checkpoint changed its source identity".into());
        }
    }
    let needed = source.total - prepared.bytes + 64 * 1024 * 1024;
    if fs2::available_space(&manager.inner.root).map_err(io_error)? < needed {
        return Err("Insufficient workspace disk space for this source download".into());
    }
    manager
        .transfer_progress(
            &job.id,
            prepared.bytes,
            Some(source.total),
            TransferInfo {
                mode: if prepared.bytes > 0 {
                    TransferMode::Resumed
                } else if prepared.restarted {
                    TransferMode::Restarted
                } else {
                    TransferMode::Fresh
                },
                resumed_bytes: prepared.bytes,
            },
        )
        .await?;
    let file = match prepared.file.take() {
        Some(file) => file,
        None => tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(
                manager
                    .inner
                    .root
                    .join("assets")
                    .join(format!("{}.part", job.id)),
            )
            .await
            .map_err(io_error)?,
    };
    let mut state = CompletedDownload {
        file,
        bytes: prepared.bytes,
        header: prepared.header.clone(),
        hasher: prepared.hasher.clone(),
        total: Some(source.total),
        validator: Some(Validator::ReviewedOriginal {
            expected_original_sha256: source.sha.clone(),
            total: source.total,
        }),
    };
    let transport = NativeTransport {
        settings,
        job,
        root: &manager.inner.root,
    };
    let mut journal = NativeJournal { manager, job };
    let result = run(&source, &mut state, &transport, &mut journal, token).await;
    if let Err(error) = result {
        // A complete wrong hash is never a resumable candidate. Other failures
        // retain only the last complete synced contiguous-prefix checkpoint.
        discard_invalid_complete_checkpoint(&manager.inner.root, job, &source, &state).await?;
        return Err(error);
    }
    Ok(Some(state))
}

#[cfg(test)]
mod tests;
