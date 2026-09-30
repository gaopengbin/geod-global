//! Local, persistent asset downloads. Validation checks signatures, size and SHA-256;
//! it does not establish GeoTIFF scientific correctness or source authenticity.

pub mod artifact;
pub mod crop;
pub mod diagnostics;
pub mod mcp;
pub mod mosaic;
pub mod processing;
pub mod projects;
pub mod proxy;
pub mod raster;
pub mod service;
pub mod thumbnail;
pub use processing::{RasterRecipe, RecipePlan, SavedRecipe};
pub use projects::{AddProjectScenesRequest, CreateProjectRequest, Project, ProjectDownloads};
pub use proxy::{ProxySettings, ProxyTest};
pub use raster::{RasterClass, RasterInspection, RasterPixel};

use chrono::Utc;
use fs2::FileExt;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::AsyncWriteExt,
    sync::{Mutex, Semaphore},
};
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, String>;
pub const MAX_ASSET_BYTES: u64 = 512 * 1024 * 1024;
pub const SOURCE_HOST: &str = "sentinel-cogs.s3.us-west-2.amazonaws.com";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateJobRequest {
    pub item_id: String,
    pub asset_key: String,
    pub href: String,
    pub media_type: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    #[serde(default = "default_job_kind")]
    pub kind: String,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub recipe: Option<RasterRecipe>,
    #[serde(default)]
    pub crop: Option<crop::CropPlan>,
    #[serde(default)]
    pub mosaic: Option<mosaic::MosaicSpec>,
    #[serde(default)]
    pub mosaic_output: Option<mosaic::MosaicPlan>,
    #[serde(default)]
    pub manifest_path: Option<String>,
    pub item_id: String,
    pub asset_key: String,
    pub href: String,
    pub media_type: String,
    pub title: String,
    pub status: JobStatus,
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub sha256: Option<String>,
    pub output_path: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub source: String,
    pub validation: String,
    pub attempts: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHealth {
    pub status: &'static str,
    pub runtime: &'static str,
    pub version: &'static str,
    pub storage_root: String,
    pub max_asset_bytes: u64,
    pub validation: &'static str,
}

#[derive(Default)]
struct Store {
    jobs: BTreeMap<String, Job>,
    active: BTreeMap<String, CancellationToken>,
}

struct Inner {
    root: PathBuf,
    store: Mutex<Store>,
    recipes: Mutex<BTreeMap<String, SavedRecipe>>,
    projects: Mutex<BTreeMap<String, Project>>,
    proxy_settings: Mutex<ProxySettings>,
    permits: Semaphore,
    raster_permits: Arc<Semaphore>,
    thumbnail_permits: Arc<Semaphore>,
    _directory_lock: std::fs::File,
    #[cfg(test)]
    fixture_origin: Option<String>,
}

#[derive(Clone)]
pub struct JobManager {
    inner: Arc<Inner>,
}

fn now() -> String {
    Utc::now().to_rfc3339()
}
fn io_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
fn active(status: &JobStatus) -> bool {
    matches!(status, JobStatus::Queued | JobStatus::Running)
}

fn default_job_kind() -> String {
    "download".into()
}

fn new_download_job(request: CreateJobRequest) -> Job {
    let timestamp = now();
    Job {
        id: Uuid::new_v4().to_string(),
        kind: default_job_kind(),
        parent_id: None,
        recipe: None,
        crop: None,
        mosaic: None,
        mosaic_output: None,
        manifest_path: None,
        item_id: request.item_id,
        asset_key: request.asset_key,
        href: request.href,
        media_type: request.media_type,
        title: request.title.unwrap_or_else(|| "Sentinel-2 asset".into()),
        status: JobStatus::Queued,
        bytes_downloaded: 0,
        total_bytes: None,
        sha256: None,
        output_path: None,
        error: None,
        created_at: timestamp.clone(),
        updated_at: timestamp,
        source: "Earth Search / Element 84; Copernicus Sentinel-2 L2A".into(),
        validation: "Pending file signature, byte count and SHA-256 checks".into(),
        attempts: 1,
    }
}

fn extension(media_type: &str) -> Result<&'static str> {
    match media_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "image/tiff" | "image/geotiff" => Ok("tif"),
        "image/jpeg" => Ok("jpg"),
        _ => Err("Only TIFF and JPEG assets are supported".into()),
    }
}

pub fn validate_asset_url(href: &str) -> Result<Url> {
    let url = Url::parse(href).map_err(|_| "Invalid asset URL")?;
    if url.scheme() != "https"
        || url.host_str() != Some(SOURCE_HOST)
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.path().starts_with("/sentinel-s2-l2a-cogs/")
        || url.path().contains('%')
    {
        return Err("Asset URL must be an unsigned HTTPS Sentinel COG URL on the approved Earth Search bucket".into());
    }
    Ok(url)
}

fn validate_request(request: &CreateJobRequest, fixture_origin: Option<&str>) -> Result<()> {
    for (value, limit) in [(&request.item_id, 200), (&request.asset_key, 80)] {
        if value.is_empty()
            || value.len() > limit
            || !value
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
        {
            return Err("Invalid itemId or assetKey".into());
        }
    }
    if request
        .title
        .as_ref()
        .is_some_and(|title| title.len() > 240)
    {
        return Err("Asset title is too long".into());
    }
    let ext = extension(&request.media_type)?;
    let url = match validate_asset_url(&request.href) {
        Ok(url) => url,
        Err(error) => {
            #[cfg(test)]
            if let Some(origin) = fixture_origin {
                let url = Url::parse(&request.href).map_err(io_error)?;
                if url.origin().ascii_serialization() == origin {
                    return Ok(());
                }
            }
            let _ = fixture_origin;
            return Err(error);
        }
    };
    if !url
        .path_segments()
        .is_some_and(|mut parts| parts.any(|part| part == request.item_id))
    {
        return Err("The asset URL does not match itemId".into());
    }
    let path = url.path().to_ascii_lowercase();
    if !(path.ends_with(&format!(".{ext}")) || (ext == "jpg" && path.ends_with(".jpeg"))) {
        return Err("Asset URL extension does not match mediaType".into());
    }
    Ok(())
}

impl JobManager {
    pub async fn open(data_dir: impl AsRef<Path>) -> Result<Self> {
        Self::open_inner(data_dir.as_ref(), None).await
    }

    async fn open_inner(data_dir: &Path, fixture_origin: Option<String>) -> Result<Self> {
        tokio::fs::create_dir_all(data_dir)
            .await
            .map_err(io_error)?;
        let root = tokio::fs::canonicalize(data_dir).await.map_err(io_error)?;
        let directory_lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join("runtime.lock"))
            .map_err(io_error)?;
        directory_lock.try_lock_exclusive().map_err(|_| {
            "This storage directory is already in use by another GeoD runtime".to_string()
        })?;
        tokio::fs::create_dir_all(root.join("assets"))
            .await
            .map_err(io_error)?;
        if tokio::fs::canonicalize(root.join("assets"))
            .await
            .map_err(io_error)?
            != root.join("assets")
        {
            return Err(
                "The managed assets directory cannot be a redirected filesystem path".into(),
            );
        }
        let records = root.join("jobs.json");
        let mut jobs: BTreeMap<String, Job> = match tokio::fs::read(&records).await {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| format!("Cannot read stored jobs: {e}"))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(error) => return Err(io_error(error)),
        };
        for (id, job) in &mut jobs {
            if Uuid::parse_str(id).is_err() || id != &job.id {
                return Err("Stored job has an invalid identifier".into());
            }
            match job.kind.as_str() {
                "download" => {}
                "raster_clip" => {
                    let recipe = job.recipe.as_ref().ok_or("Stored clip job has no recipe")?;
                    recipe.validate()?;
                    if job.parent_id.as_deref() != Some(&recipe.source.job_id) {
                        return Err("Stored clip job has inconsistent source provenance".into());
                    }
                }
                "raster_mosaic" => {
                    mosaic::validate_stored_mosaic(job)?;
                }
                _ => return Err("Stored job has an unsupported kind".into()),
            }
            if active(&job.status) {
                job.status = JobStatus::Interrupted;
                job.updated_at = now();
                job.error = Some("The runtime stopped before this operation completed. Retry restarts the operation.".into());
                job.output_path = None;
                job.sha256 = None;
                if job.kind == "raster_clip" || job.kind == "raster_mosaic" {
                    // A crash after the engine's file commit but before the job commit is not success.
                    job.crop = None;
                    job.manifest_path = None;
                    job.mosaic_output = None;
                }
            }
            if job.status == JobStatus::Succeeded {
                let expected =
                    root.join("assets")
                        .join(format!("{}.{}", job.id, extension(&job.media_type)?));
                let manifest = root.join("assets").join(format!("{id}.metadata.json"));
                if !expected.is_file()
                    || ((job.kind == "raster_clip" || job.kind == "raster_mosaic")
                        && !manifest.is_file())
                {
                    job.status = JobStatus::Failed;
                    job.error = Some(
                        "The completed local asset or its required metadata is missing".into(),
                    );
                    job.output_path = None;
                    job.sha256 = None;
                } else {
                    job.output_path = Some(expected.to_string_lossy().into_owned());
                    if job.kind == "raster_clip" || job.kind == "raster_mosaic" {
                        job.manifest_path = Some(manifest.to_string_lossy().into_owned());
                    }
                }
            }
            if (job.kind == "raster_clip" || job.kind == "raster_mosaic")
                && job.status != JobStatus::Succeeded
            {
                processing::cleanup_clip(&root, id).await;
                job.crop = None;
                job.manifest_path = None;
                job.mosaic_output = None;
            }
        }
        let recipes = processing::load_recipes(&root).await?;
        let projects = projects::load_projects(&root, fixture_origin.as_deref()).await?;
        let proxy_settings = proxy::load(&root).await?;
        proxy::download_client(&proxy_settings)?;
        let manager = Self {
            inner: Arc::new(Inner {
                root,
                store: Mutex::new(Store {
                    jobs,
                    active: BTreeMap::new(),
                }),
                recipes: Mutex::new(recipes),
                projects: Mutex::new(projects),
                proxy_settings: Mutex::new(proxy_settings),
                permits: Semaphore::new(2),
                raster_permits: Arc::new(Semaphore::new(1)),
                thumbnail_permits: Arc::new(Semaphore::new(1)),
                _directory_lock: directory_lock,
                #[cfg(test)]
                fixture_origin,
            }),
        };
        #[cfg(not(test))]
        let _ = fixture_origin;
        manager
            .persist(&manager.inner.store.lock().await.jobs)
            .await?;
        Ok(manager)
    }

    pub fn storage_root(&self) -> &Path {
        &self.inner.root
    }
    pub fn health(&self) -> RuntimeHealth {
        RuntimeHealth {
            status: "ok",
            runtime: "geod-runtime",
            version: env!("CARGO_PKG_VERSION"),
            storage_root: self.inner.root.to_string_lossy().into_owned(),
            max_asset_bytes: MAX_ASSET_BYTES,
            validation:
                "File signature, byte count and SHA-256 only; no scientific raster validation",
        }
    }
    pub async fn list(&self) -> Vec<Job> {
        let mut jobs: Vec<_> = self
            .inner
            .store
            .lock()
            .await
            .jobs
            .values()
            .cloned()
            .collect();
        jobs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        jobs
    }
    pub async fn get(&self, id: &str) -> Option<Job> {
        self.inner.store.lock().await.jobs.get(id).cloned()
    }

    /// Export a completed derived GeoTIFF only after rechecking the managed
    /// path, byte count and recorded digest. Browser downloads use this path.
    pub async fn derived_bytes(&self, id: &str) -> Result<(String, Vec<u8>)> {
        let job = self.get(id).await.ok_or("Unknown job")?;
        let root = self.inner.root.clone();
        tokio::task::spawn_blocking(move || {
            if job.status != JobStatus::Succeeded
                || !matches!(job.kind.as_str(), "raster_clip" | "raster_mosaic")
            {
                return Err("Only completed derived GeoTIFF files can be exported".into());
            }
            let assets = root.join("assets").canonicalize().map_err(io_error)?;
            if assets != root.join("assets") {
                return Err("Managed asset directory was redirected".into());
            }
            let expected = assets.join(format!("{}.tif", job.id));
            let output = Path::new(
                job.output_path
                    .as_deref()
                    .ok_or("The completed job has no file")?,
            )
            .canonicalize()
            .map_err(io_error)?;
            if output != expected {
                return Err("Output is not this job's managed GeoTIFF".into());
            }
            let file = std::fs::File::open(output).map_err(io_error)?;
            let size = file.metadata().map_err(io_error)?.len();
            if size == 0 || size > 128 * 1024 * 1024 || size != job.bytes_downloaded {
                return Err("Derived file byte count is invalid".into());
            }
            let mut bytes = Vec::with_capacity(size as usize);
            std::io::Read::take(file, size + 1)
                .read_to_end(&mut bytes)
                .map_err(io_error)?;
            let digest = format!("{:x}", Sha256::digest(&bytes));
            if bytes.len() as u64 != size || job.sha256.as_deref() != Some(digest.as_str()) {
                return Err("Derived file checksum changed after processing".into());
            }
            Ok((format!("{}.tif", job.id), bytes))
        })
        .await
        .map_err(io_error)?
    }

    pub async fn mosaic_metadata_bytes(&self, id: &str) -> Result<(String, Vec<u8>)> {
        let job = self.get(id).await.ok_or("Unknown job")?;
        let root = self.inner.root.clone();
        tokio::task::spawn_blocking(move || {
            if job.status != JobStatus::Succeeded || job.kind != "raster_mosaic" {
                return Err("Only completed project mosaics have this metadata export".into());
            }
            let spec = job
                .mosaic
                .as_ref()
                .ok_or("Mosaic source pins are missing")?;
            let plan = job
                .mosaic_output
                .as_ref()
                .ok_or("Mosaic output plan is missing")?;
            let assets = root.join("assets").canonicalize().map_err(io_error)?;
            if assets != root.join("assets") {
                return Err("Managed asset directory was redirected".into());
            }
            let filename = format!("{}.metadata.json", job.id);
            let expected = assets.join(&filename);
            let recorded = Path::new(
                job.manifest_path
                    .as_deref()
                    .ok_or("Mosaic metadata path is missing")?,
            )
            .canonicalize()
            .map_err(io_error)?;
            if recorded != expected {
                return Err("Mosaic metadata is not its managed sidecar".into());
            }
            let file = std::fs::File::open(&expected).map_err(io_error)?;
            let size = file.metadata().map_err(io_error)?.len();
            if size == 0 || size > 2 * 1024 * 1024 {
                return Err("Mosaic metadata exceeds 2 MiB".into());
            }
            let mut bytes = Vec::with_capacity(size as usize);
            file.take(size + 1)
                .read_to_end(&mut bytes)
                .map_err(io_error)?;
            if bytes.len() as u64 != size {
                return Err("Mosaic metadata changed while reading".into());
            }
            let manifest: serde_json::Value = serde_json::from_slice(&bytes).map_err(io_error)?;
            if manifest
                .get("schemaVersion")
                .and_then(|value| value.as_str())
                != Some("geod-project-mosaic/v1")
                || manifest
                    .pointer("/project/id")
                    .and_then(|value| value.as_str())
                    != Some(spec.project_id.as_str())
                || manifest.get("assetKey").and_then(|value| value.as_str())
                    != Some(spec.asset_key.as_str())
                || manifest
                    .pointer("/output/file")
                    .and_then(|value| value.as_str())
                    != Some(format!("{}.tif", job.id).as_str())
                || manifest
                    .pointer("/output/bytes")
                    .and_then(|value| value.as_u64())
                    != Some(job.bytes_downloaded)
                || manifest
                    .pointer("/output/sha256")
                    .and_then(|value| value.as_str())
                    != job.sha256.as_deref()
                || manifest.get("plan") != Some(&serde_json::to_value(plan).map_err(io_error)?)
            {
                return Err("Mosaic metadata no longer matches its completed result".into());
            }
            let sources = manifest
                .get("sources")
                .and_then(|value| value.as_array())
                .ok_or("Mosaic metadata source list is missing")?;
            if sources.len() != spec.sources.len()
                || !sources.iter().zip(&spec.sources).all(|(source, pin)| {
                    source.get("jobId").and_then(|value| value.as_str())
                        == Some(pin.job_id.as_str())
                        && source.get("sha256").and_then(|value| value.as_str())
                            == Some(pin.sha256.as_str())
                })
            {
                return Err("Mosaic metadata source pins no longer match the job".into());
            }
            Ok((filename, bytes))
        })
        .await
        .map_err(io_error)?
    }

    /// A terminal status can precede cancellation cleanup. Read the record and
    /// worker state under one lock so clients can wait for actual settlement.
    pub async fn get_with_settled(&self, id: &str) -> Option<(Job, bool)> {
        let store = self.inner.store.lock().await;
        let job = store.jobs.get(id)?;
        let settled = !active(&job.status) && !store.active.contains_key(id);
        Some((job.clone(), settled))
    }

    fn validate(&self, request: &CreateJobRequest) -> Result<()> {
        #[cfg(test)]
        let origin = self.inner.fixture_origin.as_deref();
        #[cfg(not(test))]
        let origin = None;
        validate_request(request, origin)
    }

    pub async fn create(&self, request: CreateJobRequest) -> Result<Job> {
        self.validate(&request)?;
        let mut store = self.inner.store.lock().await;
        if store.active.len() >= 64 {
            return Err("The local queue is full (64 jobs)".into());
        }
        let job = new_download_job(request);
        store.jobs.insert(job.id.clone(), job.clone());
        if let Err(error) = self.persist(&store.jobs).await {
            store.jobs.remove(&job.id);
            return Err(error);
        }
        let token = CancellationToken::new();
        store.active.insert(job.id.clone(), token.clone());
        self.spawn(job.id.clone(), token);
        Ok(job)
    }

    pub async fn cancel(&self, id: &str) -> Result<Job> {
        let mut store = self.inner.store.lock().await;
        let old = store.jobs.get(id).cloned().ok_or("Unknown job")?;
        if !active(&old.status) {
            return Ok(old);
        }
        let mut job = old.clone();
        job.status = JobStatus::Cancelled;
        job.updated_at = now();
        job.error = Some("Cancelled by user. Retry restarts the operation.".into());
        store.jobs.insert(id.into(), job.clone());
        if let Err(error) = self.persist(&store.jobs).await {
            store.jobs.insert(id.into(), old);
            return Err(error);
        }
        if let Some(token) = store.active.get(id) {
            token.cancel();
        }
        Ok(job)
    }

    pub async fn retry(&self, id: &str) -> Result<Job> {
        let mut store = self.inner.store.lock().await;
        if store.active.contains_key(id) {
            return Err("This operation is still finishing; retry shortly".into());
        }
        if store.active.len() >= 64 {
            return Err("The local queue is full (64 jobs)".into());
        }
        let old = store.jobs.get(id).cloned().ok_or("Unknown job")?;
        if !matches!(
            old.status,
            JobStatus::Failed | JobStatus::Cancelled | JobStatus::Interrupted
        ) {
            return Err("Only failed, cancelled or interrupted jobs can be retried".into());
        }
        if old.kind == "raster_clip" {
            let recipe = old.recipe.as_ref().ok_or("The clip job has no recipe")?;
            recipe.validate()?;
            processing::validate_source(recipe, store.jobs.get(&recipe.source.job_id))?;
        } else if old.kind == "raster_mosaic" {
            mosaic::validate_mosaic_sources(&old, &store.jobs)?;
        } else {
            self.validate(&CreateJobRequest {
                item_id: old.item_id.clone(),
                asset_key: old.asset_key.clone(),
                href: old.href.clone(),
                media_type: old.media_type.clone(),
                title: Some(old.title.clone()),
            })?;
        }
        let mut job = old.clone();
        job.status = JobStatus::Queued;
        job.bytes_downloaded = 0;
        job.total_bytes = None;
        job.sha256 = None;
        job.output_path = None;
        job.error = None;
        job.crop = None;
        job.mosaic_output = None;
        job.manifest_path = None;
        job.updated_at = now();
        job.validation = if job.kind == "raster_clip" {
            "Pending pinned source validation and exact pixel-window clip".into()
        } else if job.kind == "raster_mosaic" {
            "Pending source checksum validation and pixel-aligned mosaic/clip".into()
        } else {
            "Pending file signature, byte count and SHA-256 checks".into()
        };
        job.attempts += 1;
        store.jobs.insert(id.into(), job.clone());
        if let Err(error) = self.persist(&store.jobs).await {
            store.jobs.insert(id.into(), old);
            return Err(error);
        }
        let token = CancellationToken::new();
        store.active.insert(id.into(), token.clone());
        self.spawn(id.into(), token);
        Ok(job)
    }

    async fn persist(&self, jobs: &BTreeMap<String, Job>) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(jobs).map_err(io_error)?;
        let temporary = self.inner.root.join("jobs.json.tmp");
        let mut file = tokio::fs::File::create(&temporary)
            .await
            .map_err(io_error)?;
        file.write_all(&bytes).await.map_err(io_error)?;
        file.sync_all().await.map_err(io_error)?;
        drop(file);
        tokio::fs::rename(temporary, self.inner.root.join("jobs.json"))
            .await
            .map_err(io_error)
    }

    fn spawn(&self, id: String, token: CancellationToken) {
        let manager = self.clone();
        tokio::spawn(async move {
            let result = match manager.get(&id).await.as_ref().map(|job| job.kind.as_str()) {
                Some("raster_clip") => manager.process_clip(&id, &token).await,
                Some("raster_mosaic") => manager.process_mosaic(&id, &token).await,
                Some("download") => manager.download(&id, &token).await,
                _ => Err("Unknown job kind".into()),
            };
            let mut store = manager.inner.store.lock().await;
            if let Err(error) = result {
                if let Some(job) = store.jobs.get_mut(&id) {
                    if job.status != JobStatus::Cancelled {
                        job.status = JobStatus::Failed;
                        job.error = Some(error);
                        job.updated_at = now();
                    }
                    job.output_path = None;
                    job.sha256 = None;
                    job.crop = None;
                    job.mosaic_output = None;
                    job.manifest_path = None;
                    if job.kind == "raster_clip" || job.kind == "raster_mosaic" {
                        processing::cleanup_clip(&manager.inner.root, &id).await;
                    }
                }
                let _ = tokio::fs::remove_file(
                    manager.inner.root.join("assets").join(format!("{id}.part")),
                )
                .await;
                if let Err(error) = manager.persist(&store.jobs).await {
                    eprintln!("Cannot persist failed job: {error}");
                }
            }
            store.active.remove(&id);
        });
    }

    /// Wait for a terminal record and worker cleanup, so CLI process exit cannot kill the operation.
    pub async fn wait(&self, id: &str) -> Result<Job> {
        loop {
            {
                let store = self.inner.store.lock().await;
                let job = store.jobs.get(id).ok_or("Unknown job")?;
                if !active(&job.status) && !store.active.contains_key(id) {
                    return Ok(job.clone());
                }
            }
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
    }

    async fn progress(&self, id: &str, bytes: u64, total: Option<u64>) -> Result<()> {
        let mut store = self.inner.store.lock().await;
        let job = store.jobs.get_mut(id).ok_or("Unknown job")?;
        if !active(&job.status) {
            return Err("Transfer cancelled".into());
        }
        job.status = JobStatus::Running;
        job.bytes_downloaded = bytes;
        job.total_bytes = total;
        job.updated_at = now();
        self.persist(&store.jobs).await
    }

    async fn download(&self, id: &str, token: &CancellationToken) -> Result<()> {
        let _permit = tokio::select! {
            _ = token.cancelled() => return Err("Transfer cancelled".into()),
            permit = self.inner.permits.acquire() => permit.map_err(io_error)?,
        };
        self.progress(id, 0, None).await?;
        let job = self.get(id).await.ok_or("Unknown job")?;
        let proxy_settings = self.proxy_settings().await;
        let client = proxy::download_client(&proxy_settings)?;
        let response = tokio::select! {
            _ = token.cancelled() => return Err("Transfer cancelled".into()),
            response = client.get(&job.href).send() => response.map_err(io_error)?,
        };
        if !response.status().is_success() {
            return Err(format!(
                "Asset server returned HTTP {}; redirects are not followed",
                response.status()
            ));
        }
        if response.status() == reqwest::StatusCode::PARTIAL_CONTENT {
            return Err("Unexpected partial HTTP response".into());
        }
        let total = response.content_length();
        if total.is_some_and(|size| size > MAX_ASSET_BYTES) {
            return Err("Asset exceeds the 512 MiB local transfer limit".into());
        }
        self.progress(id, 0, total).await?;
        let partial = self.inner.root.join("assets").join(format!("{id}.part"));
        let final_path =
            self.inner
                .root
                .join("assets")
                .join(format!("{}.{}", id, extension(&job.media_type)?));
        let mut file = tokio::fs::File::create(&partial).await.map_err(io_error)?;
        let mut stream = response.bytes_stream();
        let mut hasher = Sha256::new();
        let mut bytes = 0u64;
        let mut header = Vec::new();
        let mut last_progress = std::time::Instant::now();
        loop {
            let chunk = tokio::select! {
                _ = token.cancelled() => return Err("Transfer cancelled".into()),
                chunk = tokio::time::timeout(Duration::from_secs(45), stream.next()) => chunk.map_err(|_| "Asset server stopped sending data for 45 seconds")?,
            };
            let Some(chunk) = chunk else {
                break;
            };
            let chunk = chunk.map_err(io_error)?;
            bytes += chunk.len() as u64;
            if bytes > MAX_ASSET_BYTES {
                return Err("Asset exceeds the 512 MiB local transfer limit".into());
            }
            header.extend(chunk.iter().take(16usize.saturating_sub(header.len())));
            file.write_all(&chunk).await.map_err(io_error)?;
            hasher.update(&chunk);
            if last_progress.elapsed() >= Duration::from_millis(200) {
                self.progress(id, bytes, total).await?;
                last_progress = std::time::Instant::now();
            }
        }
        if bytes == 0 || total.is_some_and(|expected| expected != bytes) {
            return Err("Asset transfer was empty or truncated".into());
        }
        verify_signature(&header, &job.media_type)?;
        file.sync_all().await.map_err(io_error)?;
        drop(file);
        let sha256 = format!("{:x}", hasher.finalize());
        let mut store = self.inner.store.lock().await;
        let record = store.jobs.get_mut(id).ok_or("Unknown job")?;
        if token.is_cancelled() || record.status == JobStatus::Cancelled {
            return Err("Transfer cancelled".into());
        }
        let before_commit = record.clone();
        tokio::fs::rename(&partial, &final_path)
            .await
            .map_err(io_error)?;
        record.status = JobStatus::Succeeded;
        record.bytes_downloaded = bytes;
        record.total_bytes = total.or(Some(bytes));
        record.sha256 = Some(sha256);
        record.output_path = Some(final_path.to_string_lossy().into_owned());
        record.updated_at = now();
        record.error = None;
        record.validation =
            "Passed file signature, byte count and SHA-256 checks; no scientific raster validation"
                .into();
        if let Err(error) = self.persist(&store.jobs).await {
            store.jobs.insert(id.to_owned(), before_commit);
            let _ = tokio::fs::remove_file(&final_path).await;
            return Err(error);
        }
        Ok(())
    }
}

fn verify_signature(header: &[u8], media_type: &str) -> Result<()> {
    let valid = match extension(media_type)? {
        "jpg" => header.len() >= 3 && header[..3] == [0xff, 0xd8, 0xff],
        "tif" => {
            header.starts_with(b"II\x2a\0")
                || header.starts_with(b"MM\0\x2a")
                || header.starts_with(b"II\x2b\0\x08\0\0\0")
                || header.starts_with(b"MM\0\x2b\0\x08\0\0")
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err("File signature does not match the requested TIFF/JPEG media type".into())
    }
}

#[cfg(test)]
mod tests;
