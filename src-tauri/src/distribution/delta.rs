//! A delta only reconstructs an installer. It never patches live application files.
use super::*;
use std::io::Read;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Delta {
    algorithm: String,
    url: String,
    bytes: u64,
    sha256: String,
    base_bytes: u64,
    base_sha256: String,
    target_bytes: u64,
    target_sha256: String,
}
fn hash_valid(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())
}
impl Delta {
    fn valid(&self) -> bool {
        self.algorithm == "zstd-dictionary-v1"
            && trusted_url(&self.url)
            && self.bytes > 0
            && self.bytes < self.target_bytes.saturating_mul(85) / 100
            && self.base_bytes > 0
            && self.target_bytes > 0
            && self.base_bytes <= 512 * 1024 * 1024
            && self.target_bytes <= 512 * 1024 * 1024
            && [&self.sha256, &self.base_sha256, &self.target_sha256]
                .into_iter()
                .all(|h| hash_valid(h))
    }
}
pub(super) fn candidate(update: &Update) -> Option<Delta> {
    let delta: Delta = serde_json::from_value(
        update
            .raw_json
            .get("deltas")?
            .get(&update.current_version)?
            .clone(),
    )
    .ok()?;
    delta.valid().then_some(delta)
}
pub(super) fn verify_installer(
    bytes: &[u8],
    signature: &str,
    key: &str,
    version: &str,
) -> Result<(), String> {
    let decoded = STANDARD
        .decode(signature)
        .map_err(|_| "Update signature is invalid.")?;
    let text = std::str::from_utf8(&decoded).map_err(|_| "Update signature is invalid.")?;
    let signature =
        minisign_verify::Signature::decode(text).map_err(|_| "Update signature is invalid.")?;
    public_key(key)?
        .verify(bytes, &signature, true)
        .map_err(|_| "Update signature is invalid.")?;
    // The version token lives in the cryptographically verified trusted comment.
    let expected = format!("version:{version}");
    if !text
        .lines()
        .nth(2)
        .and_then(|line| line.strip_prefix("trusted comment: "))
        .is_some_and(|comment| comment.split('\t').any(|token| token == expected))
    {
        return Err("Update signature version does not match.".into());
    }
    Ok(())
}
pub(super) fn reconstruct(delta: &Delta, base: &[u8], patch: &[u8]) -> Result<Vec<u8>, String> {
    if !delta.valid()
        || base.len() as u64 != delta.base_bytes
        || patch.len() as u64 != delta.bytes
        || format!("{:x}", Sha256::digest(base)) != delta.base_sha256
        || format!("{:x}", Sha256::digest(patch)) != delta.sha256
    {
        return Err("Delta base or download did not match.".into());
    }
    let mut decoder =
        zstd::stream::read::Decoder::with_dictionary(std::io::Cursor::new(patch), base)
            .map_err(|_| "Could not restore update package.")?;
    decoder
        .window_log_max(29)
        .map_err(|_| "Could not restore update package.")?;
    let mut restored = Vec::new();
    decoder
        .take(delta.target_bytes + 1)
        .read_to_end(&mut restored)
        .map_err(|_| "Could not restore update package.")?;
    if restored.len() as u64 != delta.target_bytes
        || format!("{:x}", Sha256::digest(&restored)) != delta.target_sha256
    {
        return Err("Restored update package did not match.".into());
    }
    Ok(restored)
}
pub(super) async fn download(
    update: &Update,
    root: PathBuf,
    key: String,
    mut progress: impl FnMut(usize, Option<u64>),
) -> Result<Vec<u8>, String> {
    let delta = candidate(update).ok_or("No matching delta package.")?;
    let base_path = root.join("installer-base.exe");
    if fs::metadata(&base_path)
        .map_err(|_| "No cached installer.")?
        .len()
        != delta.base_bytes
    {
        return Err("Delta base did not match.".into());
    }
    // Check before downloading, so a stale/missing base immediately falls back.
    let base = tauri::async_runtime::spawn_blocking(move || fs::read(base_path))
        .await
        .map_err(|_| "Could not read cached installer.")?
        .map_err(|_| "Could not read cached installer.")?;
    if format!("{:x}", Sha256::digest(&base)) != delta.base_sha256 {
        return Err("Delta base did not match.".into());
    }
    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(600))
        .https_only(!cfg!(debug_assertions));
    builder = if update.no_proxy {
        builder.no_proxy()
    } else if let Some(proxy) = &update.proxy {
        builder.proxy(reqwest::Proxy::all(proxy.as_str()).map_err(|_| "Update proxy is invalid.")?)
    } else {
        builder
    };
    let mut response = builder
        .build()
        .map_err(|_| "Could not download delta package.")?
        .get(&delta.url)
        .send()
        .await
        .map_err(|_| "Could not download delta package.")?;
    if !response.status().is_success()
        || response.content_length().is_some_and(|n| n != delta.bytes)
    {
        return Err("Could not download delta package.".into());
    }
    let mut patch = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Could not download delta package.")?
    {
        if patch.len() as u64 + chunk.len() as u64 > delta.bytes {
            return Err("Delta package is too large.".into());
        }
        progress(chunk.len(), Some(delta.bytes));
        patch.extend_from_slice(&chunk);
    }
    let signature = update.signature.clone();
    let version = update.version.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = reconstruct(&delta, &base, &patch)?;
        verify_installer(&bytes, &signature, &key, &version)?;
        Ok(bytes)
    })
    .await
    .map_err(|_| "Could not restore update package.")?
}

/// Shared by the desktop command and the explicitly opted-in installation probe.
pub(super) async fn package(
    update: &Update,
    root: PathBuf,
    key: String,
    mut progress: impl FnMut(usize, Option<u64>, &'static str, bool),
    verify: impl FnOnce(),
) -> Result<(Vec<u8>, &'static str, bool), String> {
    let mut fallback = false;
    if candidate(update).is_some() {
        let result = download(update, root, key, |chunk, total| {
            progress(chunk, total, "delta", false)
        })
        .await;
        if let Ok(bytes) = result {
            return Ok((bytes, "delta", false));
        }
        fallback = true;
        // Zero resets the full-package counter after a failed delta attempt.
        progress(0, None, "full", true);
    }
    update
        .download(
            |chunk, total| progress(chunk, total, "full", fallback),
            verify,
        )
        .await
        .map(|bytes| (bytes, "full", fallback))
        .map_err(|error| sdk_failure(error, "update_download"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_checks_base_patch_output_and_size_bounds() {
        let base = b"Old application installer: retained runtime runtime runtime".repeat(1000);
        let target = [base.clone(), b"new application content".to_vec()].concat();
        let mut compressor = zstd::bulk::Compressor::with_dictionary(8, &base).unwrap();
        let patch = compressor.compress(&target).unwrap();
        let mut delta = Delta {
            algorithm: "zstd-dictionary-v1".into(),
            url: "https://example.test/delta".into(),
            bytes: patch.len() as u64,
            sha256: format!("{:x}", Sha256::digest(&patch)),
            base_bytes: base.len() as u64,
            base_sha256: format!("{:x}", Sha256::digest(&base)),
            target_bytes: target.len() as u64,
            target_sha256: format!("{:x}", Sha256::digest(&target)),
        };
        assert_eq!(reconstruct(&delta, &base, &patch).unwrap(), target);
        let mut bad = patch.clone();
        bad[0] ^= 1;
        assert!(reconstruct(&delta, &base, &bad).is_err());
        assert!(reconstruct(&delta, b"wrong base", &patch).is_err());
        delta.target_sha256 = "0".repeat(64);
        assert!(reconstruct(&delta, &base, &patch).is_err());
        delta.target_bytes = 1;
        assert!(reconstruct(&delta, &base, &patch).is_err());
    }
    #[test]
    fn reconstructed_installer_still_requires_version_bound_signature() {
        let bytes = include_bytes!("fixtures/update.bin");
        let signature = include_str!("fixtures/update.bin.sig").trim();
        let key = include_str!("fixtures/key.pub").trim();
        assert!(verify_installer(bytes, signature, key, "0.1.1").is_ok());
        assert!(verify_installer(bytes, signature, key, "0.2.0").is_err());
        assert!(verify_installer(b"tampered", signature, key, "0.1.1").is_err());
    }
}
