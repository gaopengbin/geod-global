//! User-authorized Earth Engine export links. No Google credential or signed
//! URL is persisted. This is a generated subset, never an original-scene mirror.
use crate::{io_error, now, JobManager, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use url::Url;
use uuid::Uuid;

const MAX_BYTES: usize = 32 * 1024 * 1024;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DownloadRequest {
    pub url: String,
    pub name: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Export {
    pub id: String,
    pub name: String,
    pub project_id: String,
    pub created_at: String,
    pub bytes: u64,
    pub sha256: String,
    pub download_seconds: f64,
    // Unknown is deliberately null, never zero or elapsed network time.
    pub eecu_seconds: Option<f64>,
    pub usage_status: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadResult {
    pub export: Export,
    pub raster: crate::stac::GenericRasterInspection,
}

fn link(value: &str) -> Result<(Url, String)> {
    let bad = || "Use a Google Earth Engine getPixels export link".to_owned();
    if value.len() > 2048 {
        return Err(bad());
    }
    let url = Url::parse(value).map_err(|_| bad())?;
    if url.scheme() != "https"
        || url.host_str() != Some("earthengine.googleapis.com")
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(bad());
    }
    let parts: Vec<_> = url.path().split('/').collect();
    if parts.len() != 6 || parts[1] != "v1" || parts[2] != "projects" || parts[4] != "thumbnails" {
        return Err(bad());
    }
    let project = parts[3];
    if project.len() < 6
        || project.len() > 63
        || !project
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
    {
        return Err(bad());
    }
    let token = parts[5].strip_suffix(":getPixels").ok_or_else(bad)?;
    if token.len() < 16
        || token.len() > 512
        || !token
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err(bad());
    }
    let project = project.to_owned();
    Ok((url, project))
}
fn record_path(manager: &JobManager, id: &str) -> Result<PathBuf> {
    if Uuid::parse_str(id)
        .map_err(|_| "Invalid GEE export identifier")?
        .to_string()
        != id
    {
        return Err("Invalid GEE export identifier".into());
    }
    Ok(manager.inner.root.join("earth-engine").join(id))
}
impl JobManager {
    pub async fn list_earth_engine_exports(&self) -> Result<Vec<Export>> {
        let dir = self.inner.root.join("earth-engine");
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries = tokio::fs::read_dir(dir).await.map_err(io_error)?;
        let mut result = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(io_error)? {
            let id = entry.file_name().to_string_lossy().to_string();
            if Uuid::parse_str(&id).is_err() {
                continue;
            }
            let path = record_path(self, &id)?.join("record.json");
            let bytes = tokio::fs::read(path).await.map_err(io_error)?;
            if bytes.len() > 8192 {
                return Err("Invalid GEE export record".into());
            }
            let record: Export =
                serde_json::from_slice(&bytes).map_err(|_| "Invalid GEE export record")?;
            if record.id != id {
                return Err("GEE export identifier mismatch".into());
            }
            result.push(record);
            if result.len() > 1000 {
                return Err("GEE export library exceeds 1000 records".into());
            }
        }
        result.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(result)
    }
    pub async fn earth_engine_file(&self, id: &str) -> Result<Vec<u8>> {
        let path = record_path(self, id)?;
        let record_bytes = tokio::fs::read(path.join("record.json"))
            .await
            .map_err(io_error)?;
        if record_bytes.len() > 8192 {
            return Err("Invalid GEE export record".into());
        }
        let record: Export =
            serde_json::from_slice(&record_bytes).map_err(|_| "Invalid GEE export record")?;
        if record.id != id || record.bytes == 0 || record.bytes > MAX_BYTES as u64 {
            return Err("Invalid GEE export record".into());
        }
        let file = path.join("export.tif");
        if tokio::fs::metadata(&file).await.map_err(io_error)?.len() != record.bytes {
            return Err("Saved GEE export size changed".into());
        }
        let bytes = tokio::fs::read(file).await.map_err(io_error)?;
        if format!("{:x}", Sha256::digest(&bytes)) != record.sha256 {
            return Err("Saved GEE export checksum changed".into());
        }
        Ok(bytes)
    }
    pub async fn inspect_earth_engine_export(
        &self,
        id: &str,
    ) -> Result<crate::stac::GenericRasterInspection> {
        let bytes = self.earth_engine_file(id).await?;
        let id = id.to_owned();
        let sha = format!("{:x}", Sha256::digest(&bytes));
        let permit = self
            .inner
            .raster_permits
            .clone()
            .acquire_owned()
            .await
            .map_err(io_error)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            crate::stac::raster::inspect_export(&bytes, &id, &sha)
        })
        .await
        .map_err(io_error)?
    }
    pub async fn download_earth_engine_export(
        &self,
        request: DownloadRequest,
    ) -> Result<DownloadResult> {
        let (url, project_id) = link(request.url.trim())?;
        let name = crate::projects::validate_project_name(&request.name)?;
        self.inner.store.lock().await.accepting_jobs()?;
        let _permit = self.inner.permits.acquire().await.map_err(io_error)?;
        let client = crate::proxy::download_builder(&self.proxy_settings().await)?
            .timeout(Duration::from_secs(180))
            .build()
            .map_err(|_| "Could not create GEE download client")?;
        let start = Instant::now();
        let mut response = client.get(url).send().await.map_err(|_| {
            "GEE download connection failed; check network and generate a fresh link"
        })?;
        match response.status().as_u16() {
            200 => (),
            401 | 403 | 404 | 410 => {
                return Err(
                    "GEE link expired or access was refused; generate a new link in your project"
                        .into(),
                )
            }
            429 => return Err("GEE request quota reached; wait before retrying".into()),
            _ => {
                return Err(
                    "GEE download failed; no redirect or automatic retry was performed".into(),
                )
            }
        }
        let size = response.content_length();
        if size.is_some_and(|n| n == 0 || n > MAX_BYTES as u64) {
            return Err("GEE direct export exceeds 32 MiB; export a smaller region".into());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "GEE download was interrupted; generate a fresh link and retry")?
        {
            if bytes.len().saturating_add(chunk.len()) > MAX_BYTES {
                return Err("GEE direct export exceeds 32 MiB; export a smaller region".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.is_empty() || size.is_some_and(|n| n != bytes.len() as u64) {
            return Err("GEE export is incomplete".into());
        }
        let download_seconds = start.elapsed().as_secs_f64();
        let id = Uuid::new_v4().to_string();
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        let inspect_id = id.clone();
        let inspect_hash = sha256.clone();
        let raster_permit = self
            .inner
            .raster_permits
            .clone()
            .acquire_owned()
            .await
            .map_err(io_error)?;
        let (bytes, raster) = tokio::task::spawn_blocking(move || {
            let _permit = raster_permit;
            let raster = crate::stac::raster::inspect_export(&bytes, &inspect_id, &inspect_hash)?;
            Ok::<_, String>((bytes, raster))
        })
        .await
        .map_err(io_error)??;
        let record = Export {
            id: id.clone(),
            name,
            project_id,
            created_at: now(),
            bytes: bytes.len() as u64,
            sha256,
            download_seconds,
            eecu_seconds: None,
            usage_status: "not_measured".into(),
        };
        let dir = self.inner.root.join("earth-engine");
        tokio::fs::create_dir_all(&dir).await.map_err(io_error)?;
        let pending = tempfile::Builder::new()
            .prefix("pending-")
            .tempdir_in(&dir)
            .map_err(io_error)?;
        tokio::fs::write(pending.path().join("export.tif"), bytes)
            .await
            .map_err(io_error)?;
        tokio::fs::write(
            pending.path().join("record.json"),
            serde_json::to_vec_pretty(&record).map_err(io_error)?,
        )
        .await
        .map_err(io_error)?;
        tokio::fs::rename(pending.path(), record_path(self, &id)?)
            .await
            .map_err(io_error)?;
        Ok(DownloadResult {
            export: record,
            raster,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_links_are_restricted_and_project_is_extracted() {
        let good = "https://earthengine.googleapis.com/v1/projects/example-project/thumbnails/0123456789abcdef-0123456789abcdef:getPixels";
        assert_eq!(link(good).unwrap().1, "example-project");
        for bad in [
            good.replace("https:", "http:"),
            good.replace("googleapis.com", "googleapis.com.evil.test"),
            good.replace("/thumbnails/", "/assets/"),
            format!("{good}?token=secret"),
            format!("{good}#secret"),
            good.replace(
                "earthengine.googleapis.com",
                "user:secret@earthengine.googleapis.com",
            ),
            good.replace(
                "earthengine.googleapis.com",
                "earthengine.googleapis.com:444",
            ),
            good.replace("example-project", "..%2fprivate"),
        ] {
            assert!(link(&bad).is_err(), "{bad}");
        }
    }
    #[test]
    fn missing_usage_is_not_zero() {
        let record = Export {
            id: Uuid::new_v4().to_string(),
            name: "Test".into(),
            project_id: "example-project".into(),
            created_at: now(),
            bytes: 4,
            sha256: "a".repeat(64),
            download_seconds: 1.5,
            eecu_seconds: None,
            usage_status: "not_measured".into(),
        };
        let json = serde_json::to_value(record).unwrap();
        assert!(json["eecuSeconds"].is_null());
        assert_eq!(json["usageStatus"], "not_measured");
        assert!(crate::stac::raster::inspect_export(b"<html>login</html>", "id", "hash").is_err());
    }
    #[tokio::test]
    #[ignore = "Use GEOD_GEE_ACCEPTANCE_FILE for a local real GEE export; never downloads in CI"]
    async fn real_export_decodes_with_georeference() {
        let path = std::env::var("GEOD_GEE_ACCEPTANCE_FILE").expect("Set GEOD_GEE_ACCEPTANCE_FILE");
        let bytes = std::fs::read(path).unwrap();
        let sha = format!("{:x}", Sha256::digest(&bytes));
        let raster = crate::stac::raster::inspect_export(&bytes, "acceptance", &sha).unwrap();
        assert!(raster.crs.is_some());
        assert!(raster.preview_data_url.is_some());
    }
}
