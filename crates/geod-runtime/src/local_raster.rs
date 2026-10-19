//! Explicitly selected local TIFF originals. No remote URL or inferred provider.
use crate::{io_error, now, JobManager, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use uuid::Uuid;

pub const MAX_BYTES: usize = 32 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalRaster {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub bytes: u64,
    pub sha256: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedRaster {
    pub asset: LocalRaster,
    pub raster: crate::stac::GenericRasterInspection,
}
fn name(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 240
        || value
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
        || ![".tif", ".tiff"]
            .iter()
            .any(|ext| value.to_ascii_lowercase().ends_with(ext))
    {
        return Err("Choose a local GeoTIFF or COG file with a .tif or .tiff name".into());
    }
    Ok(value.to_owned())
}
fn directory(manager: &JobManager, id: &str) -> Result<PathBuf> {
    if Uuid::parse_str(id).is_err() {
        return Err("Invalid local raster identifier".into());
    }
    let base = manager.inner.root.join("local-rasters");
    let path = base.join(id);
    for checked in [&base, &path] {
        if checked.exists()
            && (std::fs::symlink_metadata(checked)
                .map_err(io_error)?
                .file_type()
                .is_symlink()
                || std::fs::canonicalize(checked).map_err(io_error)? != *checked)
        {
            return Err("Local raster storage cannot be a redirected path".into());
        }
    }
    Ok(path)
}
fn validate(record: &LocalRaster, id: &str) -> Result<()> {
    if record.id != id
        || name(&record.name)? != record.name
        || record.bytes == 0
        || record.bytes > MAX_BYTES as u64
        || record.sha256.len() != 64
        || !record
            .sha256
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
        || chrono::DateTime::parse_from_rfc3339(&record.created_at).is_err()
    {
        return Err("Invalid local raster record".into());
    }
    Ok(())
}
impl JobManager {
    pub async fn list_local_rasters(&self) -> Result<Vec<LocalRaster>> {
        let base = self.inner.root.join("local-rasters");
        if !base.exists() {
            return Ok(Vec::new());
        }
        let mut entries = tokio::fs::read_dir(&base).await.map_err(io_error)?;
        let mut result = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(io_error)? {
            let id = entry.file_name().to_string_lossy().into_owned();
            if Uuid::parse_str(&id).is_err() {
                continue;
            }
            let path = directory(self, &id)?.join("record.json");
            if std::fs::symlink_metadata(&path)
                .map_err(io_error)?
                .file_type()
                .is_symlink()
                || std::fs::canonicalize(&path).map_err(io_error)? != path
            {
                return Err("Local raster record was redirected".into());
            }
            if tokio::fs::metadata(&path).await.map_err(io_error)?.len() > 8192 {
                return Err("Local raster record exceeds its limit".into());
            }
            let record: LocalRaster =
                serde_json::from_slice(&tokio::fs::read(path).await.map_err(io_error)?)
                    .map_err(io_error)?;
            validate(&record, &id)?;
            result.push(record);
            if result.len() > 1000 {
                return Err("Local raster library exceeds 1000 records".into());
            }
        }
        result.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(result)
    }
    pub async fn import_local_raster(
        &self,
        file_name: String,
        bytes: Vec<u8>,
    ) -> Result<ImportedRaster> {
        let file_name = name(&file_name)?;
        if bytes.is_empty() || bytes.len() > MAX_BYTES {
            return Err("Local raster import supports files up to 32 MiB".into());
        }
        self.inner.store.lock().await.accepting_jobs()?;
        let _permit = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            self.inner.raster_permits.clone().acquire_owned(),
        )
        .await
        .map_err(|_| "The raster reader is busy; try again shortly")?
        .map_err(io_error)?;
        if self.list_local_rasters().await?.len() >= 1000 {
            return Err("Local raster library exceeds 1000 records".into());
        }
        let id = Uuid::new_v4().to_string();
        let hash = format!("{:x}", Sha256::digest(&bytes));
        let check_id = id.clone();
        let check_hash = hash.clone();
        let (bytes, raster) = tokio::task::spawn_blocking(move || {
            let raster = crate::stac::raster::inspect_local(&bytes, &check_id, &check_hash)?;
            Ok::<_, String>((bytes, raster))
        })
        .await
        .map_err(io_error)??;
        let asset = LocalRaster {
            id: id.clone(),
            name: file_name,
            created_at: now(),
            bytes: bytes.len() as u64,
            sha256: hash,
        };
        let path = directory(self, &id)?;
        tokio::fs::create_dir_all(path.parent().ok_or("Invalid local raster storage")?)
            .await
            .map_err(io_error)?;
        let path = directory(self, &id)?;
        // Only publish a directory after both original bytes and its record are durable.
        // Failed imports remain outside the UUID library and cannot break reopening.
        let staging = path
            .parent()
            .ok_or("Invalid local raster storage")?
            .join(format!(".pending-{id}"));
        tokio::fs::create_dir(&staging).await.map_err(io_error)?;
        use tokio::io::AsyncWriteExt;
        for (file_name, content) in [
            ("source.tif", bytes),
            (
                "record.json",
                serde_json::to_vec_pretty(&asset).map_err(io_error)?,
            ),
        ] {
            let mut file = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(staging.join(file_name))
                .await
                .map_err(io_error)?;
            file.write_all(&content).await.map_err(io_error)?;
            file.sync_all().await.map_err(io_error)?;
        }
        self.inner.store.lock().await.accepting_jobs()?;
        tokio::fs::rename(staging, path).await.map_err(io_error)?;
        Ok(ImportedRaster { asset, raster })
    }
    pub async fn local_raster_file(&self, id: &str) -> Result<Vec<u8>> {
        let path = directory(self, id)?;
        let record_path = path.join("record.json");
        if std::fs::symlink_metadata(&record_path)
            .map_err(io_error)?
            .file_type()
            .is_symlink()
            || std::fs::canonicalize(&record_path).map_err(io_error)? != record_path
        {
            return Err("Local raster record was redirected".into());
        }
        if tokio::fs::metadata(&record_path)
            .await
            .map_err(io_error)?
            .len()
            > 8192
        {
            return Err("Invalid local raster record".into());
        }
        let record: LocalRaster =
            serde_json::from_slice(&tokio::fs::read(record_path).await.map_err(io_error)?)
                .map_err(io_error)?;
        validate(&record, id)?;
        let file = path.join("source.tif");
        if std::fs::symlink_metadata(&file)
            .map_err(io_error)?
            .file_type()
            .is_symlink()
            || std::fs::canonicalize(&file).map_err(io_error)? != file
            || tokio::fs::metadata(&file).await.map_err(io_error)?.len() != record.bytes
        {
            return Err("Local raster file size or location changed".into());
        }
        let bytes = tokio::fs::read(file).await.map_err(io_error)?;
        if bytes.len() as u64 != record.bytes
            || format!("{:x}", Sha256::digest(&bytes)) != record.sha256
        {
            return Err("Local raster checksum changed".into());
        }
        Ok(bytes)
    }
    pub async fn inspect_local_raster(
        &self,
        id: &str,
    ) -> Result<crate::stac::GenericRasterInspection> {
        let bytes = self.local_raster_file(id).await?;
        let id = id.to_owned();
        let hash = format!("{:x}", Sha256::digest(&bytes));
        let permit = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            self.inner.raster_permits.clone().acquire_owned(),
        )
        .await
        .map_err(|_| "The raster reader is busy; try again shortly")?
        .map_err(io_error)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            crate::stac::raster::inspect_local(&bytes, &id, &hash)
        })
        .await
        .map_err(io_error)?
    }
    pub async fn sample_local_raster(
        &self,
        id: &str,
        column: u32,
        row: u32,
    ) -> Result<crate::stac::GenericRasterPixel> {
        let bytes = self.local_raster_file(id).await?;
        let id = id.to_owned();
        let hash = format!("{:x}", Sha256::digest(&bytes));
        let permit = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            self.inner.raster_permits.clone().acquire_owned(),
        )
        .await
        .map_err(|_| "The raster reader is busy; try again shortly")?
        .map_err(io_error)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            crate::stac::raster::sample_local(&bytes, &id, &hash, column, row)
        })
        .await
        .map_err(io_error)?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn original_bytes_and_pixel_survive_restart_and_tampering_is_rejected() {
        use tiff::encoder::{colortype::Gray16, TiffEncoder};
        use tiff::tags::Tag;
        let temp = tempfile::tempdir().unwrap();
        let mut output = std::io::Cursor::new(Vec::new());
        {
            let mut writer = TiffEncoder::new(&mut output).unwrap();
            let mut image = writer.new_image::<Gray16>(2, 2).unwrap();
            image
                .encoder()
                .write_tag(Tag::ModelPixelScaleTag, &[0.01f64, 0.01, 0.][..])
                .unwrap();
            image
                .encoder()
                .write_tag(Tag::ModelTiepointTag, &[0.0f64, 0., 0., 114., 23., 0.][..])
                .unwrap();
            image
                .encoder()
                .write_tag(
                    Tag::GeoKeyDirectoryTag,
                    &[
                        1u16, 1, 0, 3, 1024, 0, 1, 2, 1025, 0, 1, 1, 2048, 0, 1, 4326,
                    ][..],
                )
                .unwrap();
            image.write_data(&[0, 125, 300, 65535]).unwrap();
        }
        let bytes = output.into_inner();
        let original = bytes.clone();
        let manager = JobManager::open(temp.path()).await.unwrap();
        let imported = manager
            .import_local_raster("own.tif".into(), bytes)
            .await
            .unwrap();
        assert_eq!(
            manager.local_raster_file(&imported.asset.id).await.unwrap(),
            original
        );
        assert_eq!(
            manager
                .sample_local_raster(&imported.asset.id, 1, 0)
                .await
                .unwrap()
                .values,
            vec![serde_json::json!(125)]
        );
        drop(manager);
        let manager = JobManager::open(temp.path()).await.unwrap();
        assert_eq!(
            manager.list_local_rasters().await.unwrap(),
            vec![imported.asset.clone()]
        );
        assert_eq!(
            manager
                .inspect_local_raster(&imported.asset.id)
                .await
                .unwrap()
                .sha256,
            imported.asset.sha256
        );
        let file = directory(&manager, &imported.asset.id)
            .unwrap()
            .join("source.tif");
        let mut changed = original;
        changed[0] ^= 1;
        tokio::fs::write(file, changed).await.unwrap();
        assert!(manager
            .local_raster_file(&imported.asset.id)
            .await
            .unwrap_err()
            .contains("checksum"));
    }
    #[tokio::test]
    async fn malformed_input_and_external_file_names_do_not_create_records() {
        let temp = tempfile::tempdir().unwrap();
        let manager = JobManager::open(temp.path()).await.unwrap();
        for name in ["../file.tif", "C:\\outside.tif", "file.json", "file\ntif"] {
            assert!(manager
                .import_local_raster(name.into(), vec![1, 2, 3])
                .await
                .is_err());
        }
        assert!(manager
            .import_local_raster("bad.tif".into(), vec![1, 2, 3])
            .await
            .is_err());
        assert!(manager.list_local_rasters().await.unwrap().is_empty());
    }
}
