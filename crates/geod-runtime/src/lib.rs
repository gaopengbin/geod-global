//! Local, persistent asset downloads. Validation checks signatures, size and SHA-256;
//! it does not establish GeoTIFF scientific correctness or source authenticity.

pub mod accounts;
pub mod artifact;
pub mod crop;
pub mod diagnostics;
pub mod features;
pub mod mcp;
pub mod mosaic;
mod prepared;
pub mod preview;
pub mod processing;
pub mod projects;
pub mod providers;
pub mod proxy;
pub mod raster;
pub mod safe;
pub mod service;
pub mod stac;
pub mod stac_projects;
mod storage;
pub mod three_d;
pub mod thumbnail;
pub mod tiles;
mod transfer;
pub mod vector;
pub mod wcs;
pub mod wcs_projects;
pub mod wms;
pub use accounts::{AccountProvider, AccountStatus, ConnectAccountRequest};
pub use processing::{RasterRecipe, RecipePlan, SavedRecipe};
pub use projects::{AddProjectScenesRequest, CreateProjectRequest, Project, ProjectDownloads};
pub use proxy::{ProxySettings, ProxyTest};
pub use raster::reflectance::composite::scientific::{
    LandsatMaskPolicy, LandsatMaskRequest, LandsatMaskSpec, ModisCoupledResult, ModisCoupledScene,
    ModisCoupledSpec, ModisMaskPolicy, ModisMaskRequest, ModisMaskResult, ModisMaskSpec,
    QualityMaskRequest, QualityMaskSpec, RgbOutput, RgbPlan, RgbRequest, RgbSpec,
};
pub use raster::reflectance::composite::{
    CompositeInspection, CompositePixel, CompositePixelRequest, CompositeRequest,
};
pub use raster::{RasterClass, RasterInspection, RasterPixel};
pub use storage::verified_output_path;
pub use transfer::{TransferInfo, TransferMode};

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
    #[serde(default)]
    pub safe: Option<safe::SafeSpec>,
    #[serde(default)]
    pub safe_output: Option<safe::SafeOutput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viirs_science: Option<providers::viirs::hdf::ScienceSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viirs_prepare: Option<providers::viirs::prepare::ViirsSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rgb_spec: Option<Box<RgbSpec>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rgb_output: Option<RgbOutput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stac_source: Option<stac::SourcePin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wcs_source: Option<wcs::SourcePin>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transfer: Option<TransferInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHealth {
    pub status: &'static str,
    pub runtime: &'static str,
    pub version: &'static str,
    pub storage_root: String,
    pub max_asset_bytes: u64,
    pub max_aerial_asset_bytes: u64,
    pub validation: &'static str,
}

#[derive(Default)]
struct Store {
    jobs: BTreeMap<String, Job>,
    active: BTreeMap<String, CancellationToken>,
    closing: bool,
}

impl Store {
    fn accepting_jobs(&self) -> Result<()> {
        if self.closing {
            Err("The runtime is shutting down; reopen the application to start tasks".into())
        } else {
            Ok(())
        }
    }
}

struct Inner {
    root: PathBuf,
    store: Mutex<Store>,
    recipes: Mutex<BTreeMap<String, SavedRecipe>>,
    projects: Mutex<BTreeMap<String, Project>>,
    vectors: Mutex<BTreeMap<String, vector::Record>>,
    tiles: Mutex<tiles::Registry>,
    three_d: Mutex<three_d::Registry>,
    feature_services: Mutex<BTreeMap<String, features::FeatureService>>,
    map_services: Mutex<BTreeMap<String, wms::MapService>>,
    map_images: Mutex<BTreeMap<String, wms::MapImage>>,
    stac: Mutex<stac::Registry>,
    wcs: Mutex<wcs::Registry>,
    proxy_settings: Mutex<ProxySettings>,
    accounts: Mutex<accounts::Accounts>,
    planetary_access: providers::AccessCache,
    permits: Semaphore,
    preview_permits: Semaphore,
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
    let source = providers::source_name(&request.href).to_string();
    let title = request.title.unwrap_or_else(|| request.item_id.clone());
    Job {
        id: Uuid::new_v4().to_string(),
        kind: default_job_kind(),
        parent_id: None,
        recipe: None,
        crop: None,
        mosaic: None,
        mosaic_output: None,
        manifest_path: None,
        safe: None,
        safe_output: None,
        viirs_science: None,
        viirs_prepare: None,
        stac_source: None,
        wcs_source: None,
        rgb_spec: None,
        rgb_output: None,
        item_id: request.item_id,
        asset_key: request.asset_key,
        href: request.href,
        media_type: request.media_type,
        title,
        status: JobStatus::Queued,
        bytes_downloaded: 0,
        total_bytes: None,
        sha256: None,
        output_path: None,
        error: None,
        created_at: timestamp.clone(),
        updated_at: timestamp,
        source,
        validation: "Pending file signature, byte count and SHA-256 checks".into(),
        attempts: 1,
        transfer: None,
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
        "application/zip" => Ok("zip"),
        "application/x-hdf5" => Ok("h5"),
        _ => Err("Only TIFF, JPEG and reviewed original ZIP or HDF5 assets are supported".into()),
    }
}

fn source_transfer_limit(job: &Job) -> u64 {
    if job.asset_key == "srtm" {
        providers::srtm::MAX_ZIP_BYTES
    } else if job.asset_key == "product" && extension(&job.media_type) == Ok("zip") {
        providers::copernicus::MAX_PRODUCT_BYTES
    } else if providers::radar::KEYS.contains(&job.asset_key.as_str()) {
        providers::radar::MAX_BYTES
    } else if job.asset_key == "aerial" {
        providers::MAX_NAIP_BYTES
    } else {
        MAX_ASSET_BYTES
    }
}

// Protected and server-generated adapters have their own response contracts.
// Until those contracts accept conditional ranges, their retry is a fresh GET.
fn supports_partial_transfer(job: &Job) -> bool {
    job.stac_source.is_none()
        && job.wcs_source.is_none()
        && Url::parse(&job.href).is_ok_and(|url| {
            !matches!(
                url.host_str(),
                Some(providers::nasa::HOST | providers::copernicus::HOST)
            )
        })
}

pub fn validate_asset_url(href: &str) -> Result<Url> {
    providers::asset_url(href)
}

fn validate_request(request: &CreateJobRequest, fixture_origin: Option<&str>) -> Result<()> {
    if matches!(request.asset_key.as_str(), "stac_asset" | "wcs_coverage") {
        return Err("Custom raster downloads require a saved source selection".into());
    }
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
    if !providers::matches_item(&url, &request.item_id, &request.asset_key) {
        return Err("The asset URL does not match itemId".into());
    }
    let path = url.path().to_ascii_lowercase();
    if (request.asset_key == "viirs") != (ext == "h5") {
        return Err("VIIRS original products require HDF5 mediaType".into());
    }
    if request.asset_key == "srtm" && ext != "zip" {
        return Err("SRTMGL1 original files require ZIP mediaType".into());
    }
    if url.host_str() == Some(providers::copernicus::HOST) {
        return if ext == "zip" {
            Ok(())
        } else {
            Err("Copernicus original products require ZIP mediaType".into())
        };
    }
    if !(path.ends_with(&format!(".{ext}"))
        || (ext == "jpg" && path.ends_with(".jpeg"))
        || (ext == "tif"
            && url.host_str() == Some(providers::radar::HOST)
            && path.ends_with(".rtc.tiff")))
    {
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
            if let Some(info) = &job.transfer {
                info.validate(job)?;
            }
            if job.stac_source.is_some() {
                stac::validate_job(&root, job)?;
            } else if job.wcs_source.is_some() {
                wcs::validate_job(&root, job)?;
            } else if matches!(job.asset_key.as_str(), "stac_asset" | "wcs_coverage") {
                return Err("Custom raster job has no saved source metadata".into());
            }
            match job.kind.as_str() {
                "download" => {}
                "raster_prepare" => prepared::validate_stored(job)?,
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
                "raster_rgb" => raster::reflectance::composite::scientific::validate_stored(job)?,
                _ => return Err("Stored job has an unsupported kind".into()),
            }
            if (job.rgb_spec.is_some() || job.rgb_output.is_some()) && job.kind != "raster_rgb" {
                return Err("Stored RGB specification belongs to a different operation".into());
            }
            if job.viirs_prepare.is_some() && job.kind != "raster_prepare" {
                return Err("Stored VIIRS preparation pin belongs to a different operation".into());
            }
            if let Some(science) = &job.viirs_science {
                if job.kind != "download"
                    || job.asset_key != "viirs"
                    || job.status != JobStatus::Succeeded
                {
                    return Err(
                        "Stored VIIRS science summary belongs to an incomplete or different job"
                            .into(),
                    );
                }
                science.validate(&job.item_id, job.sha256.as_deref().unwrap_or(""))?;
            }
            if active(&job.status) {
                job.status = JobStatus::Interrupted;
                job.updated_at = now();
                job.error = Some(if job.kind == "download" {
                    "The runtime stopped before this download completed. Retry checks saved bytes and resumes when supported."
                } else { "The runtime stopped before this operation completed. Retry restarts the operation." }.into());
                job.output_path = None;
                job.sha256 = None;
                job.viirs_science = None;
                if matches!(
                    job.kind.as_str(),
                    "raster_clip" | "raster_mosaic" | "raster_prepare" | "raster_rgb"
                ) {
                    // A crash after the engine's file commit but before the job commit is not success.
                    job.crop = None;
                    job.manifest_path = None;
                    job.mosaic_output = None;
                    job.safe_output = None;
                    job.rgb_output = None;
                }
            }
            if job.status == JobStatus::Succeeded {
                let expected =
                    root.join("assets")
                        .join(format!("{}.{}", job.id, extension(&job.media_type)?));
                let manifest = root.join("assets").join(format!("{id}.metadata.json"));
                if !expected.is_file()
                    || ((matches!(
                        job.kind.as_str(),
                        "raster_clip" | "raster_mosaic" | "raster_prepare" | "raster_rgb"
                    )) && !manifest.is_file())
                {
                    job.status = JobStatus::Failed;
                    job.error = Some(
                        "The completed local asset or its required metadata is missing".into(),
                    );
                    job.output_path = None;
                    job.sha256 = None;
                    job.viirs_science = None;
                } else {
                    job.output_path = Some(expected.to_string_lossy().into_owned());
                    if matches!(
                        job.kind.as_str(),
                        "raster_clip" | "raster_mosaic" | "raster_prepare" | "raster_rgb"
                    ) {
                        job.manifest_path = Some(manifest.to_string_lossy().into_owned());
                    }
                }
            }
            if (matches!(
                job.kind.as_str(),
                "raster_clip" | "raster_mosaic" | "raster_prepare" | "raster_rgb"
            )) && job.status != JobStatus::Succeeded
            {
                processing::cleanup_clip(&root, id).await;
                job.crop = None;
                job.manifest_path = None;
                job.mosaic_output = None;
                job.safe_output = None;
                job.rgb_output = None;
            }
            if job.kind == "download"
                && job.status != JobStatus::Succeeded
                && (job.status == JobStatus::Cancelled
                    || !supports_partial_transfer(job)
                    || !transfer::candidate(&root, job, source_transfer_limit(job)))
            {
                transfer::discard(&root, id).await?;
            }
        }
        transfer::cleanup_staging(&root).await?;
        mosaic::cleanup_staged_hgt(&root).await?;
        let recipes = processing::load_recipes(&root).await?;
        let projects = projects::load_projects(&root, fixture_origin.as_deref()).await?;
        let vectors = vector::load(&root).await?;
        let tiles = tiles::load(&root).await?;
        let three_d = three_d::load(&root).await?;
        let feature_services = features::load(&root).await?;
        let (map_services, map_images) = wms::load(&root).await?;
        let stac = stac::load(&root).await?;
        let wcs = wcs::load(&root).await?;
        let proxy_settings = proxy::load(&root).await?;
        // Mock download servers must remain local even when the developer's
        // workstation has a system proxy. Production defaults remain System.
        #[cfg(test)]
        let proxy_settings =
            if fixture_origin.is_some() && proxy_settings == ProxySettings::default() {
                ProxySettings {
                    mode: proxy::ProxyMode::Direct,
                    url: None,
                }
            } else {
                proxy_settings
            };
        proxy::download_client(&proxy_settings)?;
        let manager = Self {
            inner: Arc::new(Inner {
                accounts: Mutex::new(accounts::Accounts::new(&root)),
                planetary_access: providers::AccessCache::default(),
                root,
                store: Mutex::new(Store {
                    jobs,
                    active: BTreeMap::new(),
                    closing: false,
                }),
                recipes: Mutex::new(recipes),
                projects: Mutex::new(projects),
                vectors: Mutex::new(vectors),
                tiles: Mutex::new(tiles),
                three_d: Mutex::new(three_d),
                feature_services: Mutex::new(feature_services),
                map_services: Mutex::new(map_services),
                map_images: Mutex::new(map_images),
                stac: Mutex::new(stac),
                wcs: Mutex::new(wcs),
                proxy_settings: Mutex::new(proxy_settings),
                permits: Semaphore::new(2),
                preview_permits: Semaphore::new(4),
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
            max_aerial_asset_bytes: providers::MAX_NAIP_BYTES,
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
                || !matches!(
                    job.kind.as_str(),
                    "raster_clip" | "raster_mosaic" | "raster_prepare" | "raster_rgb"
                )
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
            if size == 0
                || size
                    > if job.kind == "raster_rgb" {
                        MAX_ASSET_BYTES
                    } else {
                        128 * 1024 * 1024
                    }
                || size != job.bytes_downloaded
            {
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
                || manifest.get("viSelection")
                    != spec
                        .vi_selection
                        .as_ref()
                        .map(serde_json::to_value)
                        .transpose()
                        .map_err(io_error)?
                        .as_ref()
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
        store.accepting_jobs()?;
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
        store.accepting_jobs()?;
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
        } else if old.kind == "raster_prepare" {
            prepared::validate_source(&old, &store.jobs)?;
        } else if old.kind == "raster_mosaic" {
            mosaic::validate_mosaic_sources(&old, &store.jobs)?;
        } else if old.kind == "raster_rgb" {
            raster::reflectance::composite::scientific::validate_sources(&old, &store.jobs)?;
        } else if old.stac_source.is_some() {
            stac::validate_job(&self.inner.root, &old)?;
            if store.jobs.values().any(|job| {
                job.id != old.id
                    && job.stac_source == old.stac_source
                    && matches!(
                        job.status,
                        JobStatus::Queued | JobStatus::Running | JobStatus::Succeeded
                    )
            }) {
                return Err("This source already has a queued, running or completed task; use that task or download it from its project".into());
            }
        } else if old.wcs_source.is_some() {
            wcs::validate_job(&self.inner.root, &old)?;
            if store.jobs.values().any(|job| {
                job.id != old.id
                    && job.wcs_source == old.wcs_source
                    && matches!(
                        job.status,
                        JobStatus::Queued | JobStatus::Running | JobStatus::Succeeded
                    )
            }) {
                return Err("This coverage request already has a queued, running or completed task; use that task or download it from its project".into());
            }
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
        job.transfer = None;
        job.bytes_downloaded = 0;
        job.total_bytes = None;
        job.sha256 = None;
        job.output_path = None;
        job.error = None;
        job.crop = None;
        job.mosaic_output = None;
        job.safe_output = None;
        job.rgb_output = None;
        job.viirs_science = None;
        job.manifest_path = None;
        job.updated_at = now();
        job.validation = if job.kind == "raster_clip" {
            "Pending pinned source validation and exact pixel-window clip".into()
        } else if job.kind == "raster_prepare" {
            if job.viirs_prepare.is_some() {
                "Pending pinned VIIRS HDF5, original Int16 samples and GeoTIFF grid validation"
                    .into()
            } else {
                "Pending original SAFE checksum, XML geometry and JP2 pixel validation".into()
            }
        } else if job.kind == "raster_rgb" {
            "Pending original RGB sample, calibration and grid validation".into()
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
                Some("raster_rgb") => manager.process_scientific_rgb(&id, &token).await,
                Some("raster_clip") => manager.process_clip(&id, &token).await,
                Some("raster_mosaic") => manager.process_mosaic(&id, &token).await,
                Some("raster_prepare") => {
                    if manager
                        .get(&id)
                        .await
                        .is_some_and(|j| j.viirs_prepare.is_some())
                    {
                        manager.process_viirs(&id, &token).await
                    } else {
                        manager.process_safe(&id, &token).await
                    }
                }
                Some("download") => manager.download(&id, &token).await,
                _ => Err("Unknown job kind".into()),
            };
            let mut store = manager.inner.store.lock().await;
            if let Err(error) = result {
                if let Some(job) = store.jobs.get_mut(&id) {
                    if active(&job.status) {
                        job.status = JobStatus::Failed;
                        job.error = Some(error);
                        job.updated_at = now();
                    }
                    job.output_path = None;
                    job.sha256 = None;
                    job.crop = None;
                    job.mosaic_output = None;
                    job.safe_output = None;
                    job.rgb_output = None;
                    job.viirs_science = None;
                    job.manifest_path = None;
                    if matches!(
                        job.kind.as_str(),
                        "raster_clip" | "raster_mosaic" | "raster_prepare" | "raster_rgb"
                    ) {
                        processing::cleanup_clip(&manager.inner.root, &id).await;
                    }
                }
                let retain = store.jobs.get(&id).is_some_and(|job| {
                    job.kind == "download"
                        && job.status != JobStatus::Cancelled
                        && supports_partial_transfer(job)
                        && transfer::candidate(&manager.inner.root, job, source_transfer_limit(job))
                });
                if !retain {
                    let _ = transfer::discard(&manager.inner.root, &id).await;
                }
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

    /// Stop accepting jobs, interrupt unfinished work and wait for its actual
    /// cleanup. Closing a window to the tray must never call this method.
    /// Reopening the store makes interrupted jobs available for explicit retry.
    pub async fn shutdown(&self) -> Result<()> {
        let ids = {
            let mut store = self.inner.store.lock().await;
            store.closing = true;
            for job in store.jobs.values_mut() {
                if active(&job.status) {
                    job.status = JobStatus::Interrupted;
                    job.error = Some(if job.kind == "download" {
                        "The application exited before this download completed. Retry checks saved bytes and resumes when supported."
                    } else { "The application exited before this operation completed. Retry restarts the operation." }.into());
                    job.updated_at = now();
                }
            }
            // Cancel even workers whose user-cancelled record is already terminal.
            for token in store.active.values() {
                token.cancel();
            }
            if let Err(error) = self.persist(&store.jobs).await {
                // Still drain workers and retry the durable write after cleanup.
                eprintln!("Cannot save initial shutdown state: {error}");
            }
            store.active.keys().cloned().collect::<Vec<_>>()
        };
        for id in ids {
            self.wait(&id).await?;
        }
        self.persist(&self.inner.store.lock().await.jobs).await
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

    async fn transfer_progress(
        &self,
        id: &str,
        bytes: u64,
        total: Option<u64>,
        info: TransferInfo,
    ) -> Result<()> {
        let mut store = self.inner.store.lock().await;
        let job = store.jobs.get_mut(id).ok_or("Unknown job")?;
        if !active(&job.status) {
            return Err("Transfer cancelled".into());
        }
        job.status = JobStatus::Running;
        job.bytes_downloaded = bytes;
        job.total_bytes = total;
        info.validate(job)?;
        job.transfer = Some(info);
        job.updated_at = now();
        self.persist(&store.jobs).await
    }

    async fn download_response(
        &self,
        client: &reqwest::Client,
        settings: &ProxySettings,
        job: &Job,
        checkpoint: Option<&transfer::Checkpoint>,
    ) -> Result<reqwest::Response> {
        if job.stac_source.is_some() {
            stac::asset_response(settings, job, &self.inner.root).await
        } else if job.wcs_source.is_some() {
            wcs::asset_response(settings, job, &self.inner.root).await
        } else if Url::parse(&job.href)
            .is_ok_and(|url| url.host_str() == Some(providers::nasa::HOST))
        {
            self.nasa_response(client, job).await
        } else if Url::parse(&job.href)
            .is_ok_and(|url| url.host_str() == Some(providers::copernicus::HOST))
        {
            self.copernicus_response(client, job).await
        } else {
            let href = self
                .inner
                .planetary_access
                .resolve(client, &job.href, &job.item_id, &job.asset_key)
                .await?;
            transfer::request(client.get(href), checkpoint)
                .send()
                .await
                .map_err(|error| error.without_url().to_string())
        }
    }

    async fn download(&self, id: &str, token: &CancellationToken) -> Result<()> {
        let _permit = tokio::select! {
            _ = token.cancelled() => return Err("Transfer cancelled".into()),
            permit = self.inner.permits.acquire() => permit.map_err(io_error)?,
        };
        self.progress(id, 0, None).await?;
        let job = self.get(id).await.ok_or("Unknown job")?;
        let limit = source_transfer_limit(&job);
        let resumable = supports_partial_transfer(&job);
        let mut prepared = if resumable {
            transfer::prepare(&self.inner.root, &job, limit, token).await?
        } else {
            transfer::discard(&self.inner.root, id).await?;
            transfer::prepare(&self.inner.root, &job, limit, token).await?
        };
        let proxy_settings = self.proxy_settings().await;
        let client = proxy::download_client(&proxy_settings)?;
        let mut response = tokio::select! {
            _ = token.cancelled() => return Err("Transfer cancelled".into()),
            response = self.download_response(&client, &proxy_settings, &job, prepared.checkpoint.as_ref()) => response?,
        };
        if prepared
            .checkpoint
            .as_ref()
            .is_some_and(|pin| !transfer::accepts(&response, pin))
        {
            // If-Range can legitimately return a new complete representation.
            // Invalid ranges and 412/416 get one fresh request, never appended.
            match response.status() {
                reqwest::StatusCode::OK
                | reqwest::StatusCode::PARTIAL_CONTENT
                | reqwest::StatusCode::PRECONDITION_FAILED
                | reqwest::StatusCode::RANGE_NOT_SATISFIABLE => {}
                status => {
                    return Err(format!(
                        "Asset server returned HTTP {status}; partial download was retained"
                    ))
                }
            }
            drop(prepared.file.take());
            transfer::discard(&self.inner.root, id).await?;
            prepared = transfer::prepare(&self.inner.root, &job, limit, token).await?;
            prepared.restarted = true;
            if response.status() != reqwest::StatusCode::OK {
                drop(response);
                response = tokio::select! {
                    _ = token.cancelled() => return Err("Transfer cancelled".into()),
                    response = self.download_response(&client, &proxy_settings, &job, None) => response?,
                };
            }
        }
        if !response.status().is_success() {
            return Err(format!(
                "Asset server returned HTTP {}; redirects are not followed",
                response.status()
            ));
        }
        if prepared.checkpoint.is_none()
            && (response.status() != reqwest::StatusCode::OK
                || response
                    .headers()
                    .contains_key(reqwest::header::CONTENT_RANGE))
        {
            return Err("Unexpected partial HTTP response".into());
        }
        let total = prepared
            .checkpoint
            .as_ref()
            .map(|pin| pin.total)
            .or_else(|| response.content_length());
        let validator = prepared
            .checkpoint
            .as_ref()
            .map(|pin| (pin.etag.clone(), pin.total))
            .or_else(|| transfer::validator(&response, resumable));
        let archive = job.asset_key == "product" && extension(&job.media_type)? == "zip";
        let srtm = job.asset_key == "srtm";
        let radar = providers::radar::KEYS.contains(&job.asset_key.as_str());
        let limit_error = if srtm {
            "SRTM ZIP exceeds the 64 MiB local transfer limit"
        } else if archive {
            "Product exceeds the 4 GiB local transfer limit"
        } else if radar {
            "Radar COG exceeds the 4 GiB local transfer limit"
        } else if job.asset_key == "aerial" {
            "NAIP COG exceeds the 4 GiB local transfer limit"
        } else {
            "Asset exceeds the 512 MiB local transfer limit"
        };
        if total.is_some_and(|size| size > limit) {
            return Err(limit_error.into());
        }
        let needed = total
            .unwrap_or(limit)
            .saturating_sub(prepared.bytes)
            .saturating_add(64 * 1024 * 1024);
        if fs2::available_space(&self.inner.root).map_err(io_error)? < needed {
            return Err("Insufficient workspace disk space for this source download".into());
        }
        let info = TransferInfo {
            mode: if prepared.bytes > 0 {
                TransferMode::Resumed
            } else if prepared.restarted {
                TransferMode::Restarted
            } else {
                TransferMode::Fresh
            },
            resumed_bytes: prepared.bytes,
        };
        self.transfer_progress(id, prepared.bytes, total, info)
            .await?;
        let partial = self.inner.root.join("assets").join(format!("{id}.part"));
        let final_path =
            self.inner
                .root
                .join("assets")
                .join(format!("{}.{}", id, extension(&job.media_type)?));
        let mut file = match prepared.file.take() {
            Some(file) => file,
            None => tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&partial)
                .await
                .map_err(io_error)?,
        };
        let mut stream = response.bytes_stream();
        let mut hasher = prepared.hasher;
        let mut bytes = prepared.bytes;
        let mut header = prepared.header;
        let mut last_progress = std::time::Instant::now();
        let mut last_checkpoint = std::time::Instant::now();
        let mut saved_bytes = bytes;
        let mut invalid_body = false;
        let transfer_result: Result<()> = async { loop {
            let chunk = tokio::select! {
                _ = token.cancelled() => return Err("Transfer cancelled".into()),
                chunk = tokio::time::timeout(Duration::from_secs(45), stream.next()) => chunk.map_err(|_| "Asset server stopped sending data for 45 seconds")?,
            };
            let Some(chunk) = chunk else {
                break;
            };
            let chunk = chunk.map_err(|error| error.without_url().to_string())?;
            bytes += chunk.len() as u64;
            if bytes > limit || total.is_some_and(|size| bytes > size) {
                invalid_body = true;
                return Err(if bytes > limit { limit_error } else { "Asset transfer exceeded its declared byte count" }.into());
            }
            header.extend(chunk.iter().take(16usize.saturating_sub(header.len())));
            file.write_all(&chunk).await.map_err(io_error)?;
            hasher.update(&chunk);
            if last_checkpoint.elapsed() >= Duration::from_secs(5) || bytes.saturating_sub(saved_bytes) >= 32 * 1024 * 1024 {
                transfer::checkpoint(&self.inner.root, &job, &file, validator.as_ref(), bytes, &hasher, &header).await?;
                saved_bytes = bytes;
                last_checkpoint = std::time::Instant::now();
            }
            if last_progress.elapsed() >= Duration::from_millis(200) {
                self.progress(id, bytes, total).await?;
                last_progress = std::time::Instant::now();
            }
        } Ok(()) }.await;
        if let Err(error) = transfer_result {
            if invalid_body {
                transfer::discard_checkpoint(&self.inner.root, id).await?;
            } else {
                transfer::checkpoint(
                    &self.inner.root,
                    &job,
                    &file,
                    validator.as_ref(),
                    bytes,
                    &hasher,
                    &header,
                )
                .await?;
                let _ = self.progress(id, bytes, total).await;
            }
            return Err(error);
        }
        if bytes == 0 || total.is_some_and(|expected| expected != bytes) {
            transfer::checkpoint(
                &self.inner.root,
                &job,
                &file,
                validator.as_ref(),
                bytes,
                &hasher,
                &header,
            )
            .await?;
            return Err("Asset transfer was empty or truncated".into());
        }
        // Full-file validation failures must not become resumable candidates.
        transfer::discard_checkpoint(&self.inner.root, id).await?;
        verify_signature(&header, &job.media_type)?;
        file.sync_all().await.map_err(io_error)?;
        drop(file);
        if archive {
            let path = partial.clone();
            let item = job.item_id.clone();
            tokio::task::spawn_blocking(move || providers::copernicus::verify_safe(&path, &item))
                .await
                .map_err(io_error)??;
        }
        if srtm {
            let path = partial.clone();
            let item = job.item_id.clone();
            tokio::task::spawn_blocking(move || providers::srtm::verify_hgt(&path, &item))
                .await
                .map_err(io_error)??;
        }
        let sha256 = format!("{:x}", hasher.finalize());
        if job.stac_source.is_some() {
            stac::verify_transfer(&self.inner.root, &job, bytes, &sha256)?;
        }
        if job.wcs_source.is_some() {
            let root = self.inner.root.clone();
            let check = job.clone();
            let path = partial.clone();
            let hash = sha256.clone();
            let cancellation = token.clone();
            let permit = tokio::select! {
                _ = token.cancelled() => return Err("Transfer cancelled".into()),
                permit = self.inner.raster_permits.clone().acquire_owned() => permit.map_err(io_error)?,
            };
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                wcs::verify_download(&root, &check, &path, bytes, &hash, &cancellation)
            })
            .await
            .map_err(io_error)??;
        }
        let viirs_science = if job.asset_key == "viirs" {
            let path = partial.clone();
            let item = job.item_id.clone();
            let hash = sha256.clone();
            let cancellation = token.clone();
            let permit = tokio::select! {
                _ = token.cancelled() => return Err("Transfer cancelled".into()),
                permit = self.inner.raster_permits.clone().acquire_owned() => permit.map_err(io_error)?,
            };
            Some(
                tokio::task::spawn_blocking(move || {
                    let _permit = permit;
                    providers::viirs::hdf::verify(&path, &item, bytes, &hash, &cancellation)
                })
                .await
                .map_err(io_error)??,
            )
        } else {
            None
        };
        let mut store = self.inner.store.lock().await;
        let record = store.jobs.get_mut(id).ok_or("Unknown job")?;
        if token.is_cancelled() || !active(&record.status) {
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
        record.viirs_science = viirs_science;
        record.output_path = Some(final_path.to_string_lossy().into_owned());
        record.updated_at = now();
        record.error = None;
        record.validation = if job.wcs_source.is_some() {
            "Passed coverage response byte count, SHA-256, declared grid and raster layout checks; retained server-generated subset without scientific calibration"
        } else if srtm {
            "Passed official SRTMGL1 identity, ZIP member CRC, 3601 × 3601 Int16 HGT byte count and SHA-256 checks; terrain accuracy was not assessed"
        } else if job.asset_key == "viirs" {
            "Passed VIIRS v002 embedded identity, period, sinusoidal tile geometry and calibration; all M5/M4/M3 Int16 samples decoded and checksummed; QA masks and other science layers were not decoded"
        } else if archive {
            "Passed official product identity, SAFE ZIP directory, byte count and SHA-256 checks; JP2 pixels were not decoded"
        } else { "Passed file signature, byte count and SHA-256 checks; no scientific raster validation" }.into();
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
        "zip" => header.starts_with(b"PK\x03\x04"),
        "h5" => header.starts_with(b"\x89HDF\r\n\x1a\n"),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err("File signature does not match the requested media type".into())
    }
}

#[cfg(test)]
mod tests;
