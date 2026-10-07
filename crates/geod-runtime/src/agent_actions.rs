//! Review-first Agent actions. Plans are immutable native records; only the
//! desktop confirmation command can atomically commit approval with its project
//! metadata or queued jobs in the existing native store.
use crate::{io_error, now, projects::valid_bounds, Job, JobManager, JobStatus, Result};
use chrono::{DateTime, NaiveDate, Utc};
use futures_util::StreamExt;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const POLICY: &str = "geod-agent-review/v2";
const LEGACY_POLICY: &str = "geod-agent-review/v1";
const CATALOG: &str = "https://earth-search.aws.element84.com/v1/search";
const MAX_DOCUMENT: usize = 5 * 1024 * 1024;
const MAX_RECORD: usize = 4 * 1024 * 1024;
const MAX_FILES: usize = 32;
const TTL_MINUTES: i64 = 30;

mod boundary;
mod catalog;
mod coverage;
mod custom;
mod footprint;
mod places;
mod preview;
mod project;
mod protected;
mod regions;
mod revision;
mod science;
mod search_more;
mod vector;
use project::ProjectPin;
pub use project::ProjectScope;
pub use revision::PlanRevision;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MapContext {
    pub page: String,
    pub provider: String,
    pub bounds: [f64; 4],
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub start: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub end: String,
    pub cloud_max: f64,
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<crate::crop::PolygonGeometry>,
}
impl MapContext {
    pub fn validate(&self) -> Result<()> {
        if ![
            "Explore",
            "Workspace",
            "My Data",
            "Tasks",
            "Settings",
            "Help",
        ]
        .contains(&self.page.as_str())
            || self.provider.len() > 80
            || !self
                .provider
                .bytes()
                .all(|v| v.is_ascii_alphanumeric() || v == b'-')
            || self.project_id.as_ref().is_some_and(|id| !uuid(id))
        {
            return Err("Invalid Agent map context.".into());
        }
        // Reference elevation products do not have an acquisition date filter.
        // Keep their actual area/project context without inventing dates. All
        // temporal sources (and partial date pairs) retain strict validation.
        if self.start.is_empty()
            && self.end.is_empty()
            && catalog::source(&self.provider).is_ok_and(|source| !source.temporal)
        {
            if !valid_bounds(self.bounds)
                || !self.cloud_max.is_finite()
                || !(0.0..=100.0).contains(&self.cloud_max)
            {
                return Err("Invalid Agent map context.".into());
            }
        } else {
            validate_area_dates(self.bounds, &self.start, &self.end, self.cloud_max)?;
        }
        if let Some(geometry) = &self.geometry {
            let extent = geometry.bounds()?;
            if extent[0] >= self.bounds[2]
                || extent[2] <= self.bounds[0]
                || extent[1] >= self.bounds[3]
                || extent[3] <= self.bounds[1]
            {
                return Err("Attached polygon does not overlap the map area.".into());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchQuery {
    pub provider: String,
    pub bounds: [f64; 4],
    #[serde(default)]
    pub start: String,
    #[serde(default)]
    pub end: String,
    #[serde(default = "default_cloud")]
    pub cloud_max: f64,
    #[serde(default = "default_limit")]
    pub limit: usize,
}
fn default_cloud() -> f64 {
    60.0
}
fn default_limit() -> usize {
    5
}
fn uuid(id: &str) -> bool {
    Uuid::parse_str(id).is_ok_and(|v| v.to_string() == id)
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn validate_area_dates(bounds: [f64; 4], start: &str, end: &str, cloud: f64) -> Result<()> {
    let parse = |v: &str| {
        NaiveDate::parse_from_str(v, "%Y-%m-%d")
            .ok()
            .filter(|d| d.to_string() == v)
    };
    if !valid_bounds(bounds)
        || !cloud.is_finite()
        || !(0.0..=100.0).contains(&cloud)
        || parse(start).is_none()
        || parse(end).is_none()
        || start > end
    {
        return Err(
            "Use ordered WGS84 bounds, valid YYYY-MM-DD dates and cloud cover 0..100.".into(),
        );
    }
    Ok(())
}
impl SearchQuery {
    fn validate(&self) -> Result<()> {
        let source = catalog::source(&self.provider)?;
        if !(1..=20).contains(&self.limit) {
            return Err("Agent search limit must be 1..20.".into());
        }
        if !source.temporal {
            if !valid_bounds(self.bounds) {
                return Err("Use ordered WGS84 bounds.".into());
            }
            return Ok(());
        }
        validate_area_dates(self.bounds, &self.start, &self.end, self.cloud_max)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Candidate {
    item_id: String,
    date: String,
    cloud: Option<f64>,
    bounds: [f64; 4],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    footprint: Option<crate::crop::PolygonGeometry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    crs: Option<String>,
    assets: Vec<crate::CreateJobRequest>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    bands: std::collections::BTreeMap<String, crate::projects::ReflectanceBand>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    end_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    date_role: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SearchReceipt {
    id: String,
    session_id: String,
    query: SearchQuery,
    retrieved_at: String,
    document_sha256: String,
    candidates: Vec<Candidate>,
    more_available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    next: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemotePin {
    pub bytes: u64,
    pub etag: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DownloadFile {
    request: crate::CreateJobRequest,
    date: String,
    pin: Option<RemotePin>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum Action {
    WcsProject {
        request: crate::wcs_projects::SaveProjectRequest,
        target: Option<ProjectPin>,
    },
    WcsDownload {
        project: ProjectPin,
        bounds: [f64; 4],
        selections: Vec<crate::wcs::SourcePin>,
    },
    Vector {
        scope: vector::Scope,
    },
    StacProject {
        request: crate::stac_projects::SaveProjectRequest,
        target: Option<ProjectPin>,
    },
    StacDownload {
        project: ProjectPin,
        bounds: [f64; 4],
        files: Vec<custom::FilePin>,
    },
    Download {
        query: SearchQuery,
        metadata_sha256: String,
        files: Vec<DownloadFile>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<ProjectPin>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        acquisition: Option<footprint::Scope>,
    },
    Clip {
        recipe: crate::RasterRecipe,
        output: crate::crop::CropPlan,
    },
    Project {
        request: crate::CreateProjectRequest,
        target: Option<ProjectPin>,
        metadata_sha256: String,
    },
    Mosaic {
        project: ProjectScope,
        project_hash: String,
        spec: crate::mosaic::MosaicSpec,
        output: crate::mosaic::MosaicPlan,
    },
    Rgb {
        spec: crate::RgbSpec,
        raw_bytes: u64,
        required_disk_bytes: u64,
        project: Option<ProjectPin>,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Plan {
    id: String,
    session_id: String,
    policy: String,
    hash: String,
    created_at: String,
    expires_at: String,
    action: Action,
}
impl Plan {
    fn validate_reference(&self, session: &str, id: &str) -> Result<()> {
        self.validate(session)?;
        if self.id != id {
            return Err("The stored review identity differs from its reference.".into());
        }
        Ok(())
    }
    fn effective_hash(&self) -> Result<String> {
        // No approval, timestamps, random plan IDs or output filenames in the hash.
        Ok(digest(
            &serde_json::to_vec(&json!({"policy":self.policy,"action":self.action}))
                .map_err(io_error)?,
        ))
    }
    fn validate(&self, session: &str) -> Result<()> {
        if !uuid(&self.id)
            || !uuid(session)
            || self.session_id != session
            || ![POLICY, LEGACY_POLICY].contains(&self.policy.as_str())
            || self.hash != self.effective_hash()?
        {
            return Err(
                "Agent plan changed or belongs to another conversation. Create a new plan.".into(),
            );
        }
        match &self.action {
            Action::WcsProject { .. } | Action::WcsDownload { .. } => {
                if self.policy != POLICY {
                    return Err("Coverage requests need a current review policy.".into());
                }
                coverage::validate(&self.action)?;
            }
            Action::Vector { .. } => {
                if self.policy != POLICY {
                    return Err("Vector extraction needs a current review policy".into());
                }
                vector::validate(&self.action)?;
            }
            Action::StacProject { .. } | Action::StacDownload { .. } => {
                if self.policy != POLICY {
                    return Err("This custom source needs a current review policy.".into());
                }
                custom::validate(&self.action)?;
            }
            Action::Download {
                query,
                files,
                project,
                ..
            } => {
                query.validate()?;
                if let Some(pin) = project {
                    if self.policy != POLICY {
                        return Err("This project download plan uses an older policy.".into());
                    }
                    pin.validate()?;
                }
                if files.is_empty() || files.len() > MAX_FILES {
                    return Err("Invalid Agent plan file count.".into());
                }
                for file in files {
                    protected::validate_pin(&file.request, &query.provider, file.pin.as_ref())?;
                    crate::validate_request(&file.request, None)?;
                    catalog::validate_source(&file.request, &query.provider)?;
                    if self.policy == LEGACY_POLICY && query.provider != "earth-search" {
                        return Err("This provider needs a current review policy.".into());
                    }
                }
            }
            Action::Clip { recipe, .. } => recipe.validate()?,
            Action::Project {
                request, target, ..
            } => {
                if self.policy != POLICY {
                    return Err("This project plan uses an older policy.".into());
                }
                request.validate(None)?;
                if let Some(pin) = target {
                    pin.validate()?;
                }
            }
            Action::Mosaic {
                project,
                project_hash,
                spec,
                ..
            } => {
                if self.policy != POLICY
                    || project.fingerprint()? != *project_hash
                    || project.id != spec.project_id
                {
                    return Err("Mosaic project scope changed. Create a new plan.".into());
                }
                project.validate()?;
            }
            Action::Rgb {
                spec,
                raw_bytes,
                required_disk_bytes,
                project,
            } => {
                if self.policy != POLICY {
                    return Err("Scientific RGB needs a current review policy.".into());
                }
                science::validate(spec, *raw_bytes, *required_disk_bytes, project)?;
            }
        }
        Ok(())
    }
    fn expired(&self) -> bool {
        DateTime::parse_from_rfc3339(&self.expires_at).map_or(true, |date| date <= Utc::now())
    }
    fn job_ids(&self) -> Vec<String> {
        let count = match &self.action {
            Action::Download { files, .. } => files.len(),
            Action::StacDownload { files, .. } => files.len(),
            Action::WcsDownload { selections, .. } => selections.len(),
            Action::Clip { .. } => 1,
            Action::Mosaic { .. } => 1,
            Action::Rgb { .. } => 1,
            Action::Project { .. }
            | Action::StacProject { .. }
            | Action::WcsProject { .. }
            | Action::Vector { .. } => 0,
        };
        (0..count)
            .map(|index| {
                let hash = Sha256::digest(format!("{}:{}:{index}", self.policy, self.id));
                let mut bytes: [u8; 16] = hash[..16].try_into().unwrap();
                bytes[6] = (bytes[6] & 0x0f) | 0x40;
                bytes[8] = (bytes[8] & 0x3f) | 0x80;
                Uuid::from_bytes(bytes).to_string()
            })
            .collect()
    }
}

/// Persisted in the same atomic record as each queued job. There is no gap
/// between a separate approval ledger and queue submission to recover/replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalReceipt {
    pub plan_id: String,
    pub plan_hash: String,
    pub session_id: String,
    pub approved_at: String,
    pub policy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<RemotePin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_scope: Option<ProjectScope>,
}

async fn record_dir(root: &Path, kind: &str) -> Result<PathBuf> {
    let directory = root.join(format!("agent-{kind}"));
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(io_error)?;
    crate::storage::managed_directory(root, &format!("agent-{kind}"))
}
async fn read_record<T: DeserializeOwned>(root: &Path, kind: &str, id: &str) -> Result<T> {
    if !uuid(id) {
        return Err("Invalid Agent record ID.".into());
    }
    let path = record_dir(root, kind).await?.join(format!("{id}.json"));
    crate::storage::regular_file(&path)?;
    let mut file = tokio::fs::File::open(path).await.map_err(io_error)?;
    use tokio::io::AsyncReadExt;
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_RECORD as u64 + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(io_error)?;
    if bytes.len() > MAX_RECORD {
        return Err("Agent record exceeds its storage limit.".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "Agent record could not be read.".into())
}
async fn write_record<T: Serialize>(root: &Path, kind: &str, id: &str, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec(value).map_err(io_error)?;
    if bytes.len() > MAX_RECORD {
        return Err("Agent record exceeds its storage limit.".into());
    }
    let directory = record_dir(root, kind).await?;
    let mut entries = tokio::fs::read_dir(&directory).await.map_err(io_error)?;
    let mut count = 0;
    while entries.next_entry().await.map_err(io_error)?.is_some() {
        count += 1;
        if count >= 2000 {
            return Err("Agent plan storage limit reached.".into());
        }
    }
    let temporary = directory.join(format!("{id}.{}.tmp", Uuid::new_v4()));
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .await
        .map_err(io_error)?;
    file.write_all(&bytes).await.map_err(io_error)?;
    file.sync_all().await.map_err(io_error)?;
    drop(file);
    tokio::fs::rename(temporary, directory.join(format!("{id}.json")))
        .await
        .map_err(io_error)
}

fn normalize_catalog(value: &Value, query: &SearchQuery) -> Result<Vec<Candidate>> {
    catalog::normalize(value, query)
}
fn validate_remote(pin: &RemotePin, request: &crate::CreateJobRequest) -> Result<()> {
    let limit = crate::source_transfer_limit(&crate::new_download_job(request.clone()));
    if pin.bytes == 0 || pin.bytes > limit || !crate::transfer::strong_etag(&pin.etag) {
        return Err(
            "Source requires a stable ETag and a file within its native product size limit.".into(),
        );
    }
    Ok(())
}
async fn head(manager: &JobManager, request: &crate::CreateJobRequest) -> Result<RemotePin> {
    crate::validate_request(request, None)?;
    let settings = manager.proxy_settings().await;
    let client = crate::proxy::download_builder(&settings)?
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(io_error)?;
    let href = manager
        .inner
        .planetary_access
        .resolve(&client, &request.href, &request.item_id, &request.asset_key)
        .await?;
    let response = client
        .head(href)
        .send()
        .await
        .map_err(|_| "Could not check source file size. Check your proxy and try again.")?;
    if !response.status().is_success() {
        return Err("Source file preflight failed. Try searching again.".into());
    }
    let pin = RemotePin {
        bytes: response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .ok_or("Source file size is unavailable.")?,
        etag: response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .ok_or("Source file fingerprint is unavailable.")?
            .into(),
    };
    validate_remote(&pin, request)?;
    Ok(pin)
}

async fn download_preflight(
    manager: &JobManager,
    request: &crate::CreateJobRequest,
) -> Result<Option<RemotePin>> {
    let provider = catalog::provider(request)?;
    if protected::account(provider).is_some() {
        // Public metadata does not establish original-product entitlement. No
        // protected file request, credential refresh or guessed ETag here.
        return Ok(None);
    }
    head(manager, request).await.map(Some)
}

impl JobManager {
    pub async fn agent_search(&self, session: &str, query: SearchQuery) -> Result<Value> {
        if !uuid(session) {
            return Err("Invalid Agent conversation ID.".into());
        }
        query.validate()?;
        let url = catalog::url(&query)?;
        let settings = self.proxy_settings().await;
        let client =
            crate::features::client_with_timeout(&url, &settings, Duration::from_secs(25)).await?;
        let response = client
            .get(url)
            .header("Accept", "application/geo+json")
            .send()
            .await
            .map_err(|_| "Agent catalog search failed. Check your proxy and try again.")?;
        if !response.status().is_success() {
            return Err("Agent catalog search failed. Try again later.".into());
        }
        if response
            .content_length()
            .is_some_and(|len| len > MAX_DOCUMENT as u64)
        {
            return Err("Catalog response exceeds 5 MiB.".into());
        }
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| "Catalog response interrupted.")?;
            if bytes.len() + chunk.len() > MAX_DOCUMENT {
                return Err("Catalog response exceeds 5 MiB.".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|_| "Catalog response is invalid JSON.")?;
        let mut candidates = normalize_catalog(&value, &query)?;
        if query.provider == "copernicus" {
            protected::resolve_copernicus_candidates(self, &mut candidates).await?;
        }
        if query.provider.starts_with("planetary-") {
            self.inner
                .planetary_access
                .observe_catalogue(&value, catalog::source(&query.provider)?.collection)
                .await?;
        }
        let next = search_more::next_url(&value, &catalog::url(&query)?)?;
        let receipt = SearchReceipt {
            id: Uuid::new_v4().to_string(),
            session_id: session.into(),
            query,
            retrieved_at: now(),
            document_sha256: digest(&bytes),
            candidates,
            next,
            more_available: value["links"]
                .as_array()
                .is_some_and(|links| links.iter().any(|link| link["rel"] == "next")),
        };
        write_record(&self.inner.root, "searches", &receipt.id, &receipt).await?;
        Ok(
            json!({"searchId":receipt.id,"query":receipt.query,"retrievedAt":receipt.retrieved_at,"metadataSha256":receipt.document_sha256,
            "moreAvailable":receipt.more_available,"complete":!receipt.more_available,"scenes":receipt.candidates.iter().map(|c|json!({"itemId":c.item_id,"date":c.date,"cloud":c.cloud,"endDate":c.end_date,"dateMeaning":c.date_role,"bounds":c.bounds,"assets":c.assets.iter().map(|a|a.asset_key.clone()).collect::<Vec<_>>()})).collect::<Vec<_>>(),
            "canContinue":receipt.next.is_some(),"capabilities":catalog::capabilities(&receipt.query.provider)?,"note":"A bounded catalog page is not full coverage or a downloaded file. Read the source administrative polygon, use geod_scene_coverage, and continue with geod_scene_search_more if incomplete."}),
        )
    }
    pub async fn agent_download_plan(
        &self,
        session: &str,
        search_id: &str,
        mut item_ids: Vec<String>,
        asset_key: &str,
    ) -> Result<Value> {
        if !crate::providers::SOURCE_ASSET_KEYS.contains(&asset_key)
            || item_ids.is_empty()
            || item_ids.len() > MAX_FILES
        {
            return Err("Choose 1..32 scenes and a reviewed source asset key.".into());
        }
        item_ids.sort();
        item_ids.dedup();
        let receipt: SearchReceipt = read_record(&self.inner.root, "searches", search_id).await?;
        if receipt.session_id != session
            || !uuid(session)
            || DateTime::parse_from_rfc3339(&receipt.retrieved_at).map_or(true, |date| {
                Utc::now() - date.with_timezone(&Utc) > chrono::Duration::minutes(TTL_MINUTES)
            })
        {
            return Err(
                "Scene search is stale or belongs to another conversation. Search again.".into(),
            );
        }
        receipt.query.validate()?;
        footprint::selection(self, session, search_id, &item_ids, None).await?;
        let mut requests = Vec::new();
        for id in item_ids {
            let candidate = receipt
                .candidates
                .iter()
                .find(|c| c.item_id == id)
                .ok_or("Choose scene IDs returned by this native search.")?;
            let request = candidate
                .assets
                .iter()
                .find(|a| a.asset_key == asset_key)
                .ok_or("Selected scene has no requested asset.")?
                .clone();
            requests.push((request, candidate.date.clone()));
        }
        let files = futures_util::future::try_join_all(requests.into_iter().map(
            |(request, date)| async move {
                Ok::<_, String>(DownloadFile {
                    pin: download_preflight(self, &request).await?,
                    request,
                    date,
                })
            },
        ))
        .await?;
        self.save_agent_plan(
            session,
            Action::Download {
                acquisition: Some(footprint::Scope {
                    bounds: receipt.query.bounds,
                    geometry: None,
                    scenes: receipt
                        .candidates
                        .iter()
                        .filter(|c| files.iter().any(|f| f.request.item_id == c.item_id))
                        .map(|c| (c.item_id.clone(), c.footprint.clone()))
                        .collect(),
                }),
                query: receipt.query,
                metadata_sha256: receipt.document_sha256,
                files,
                project: None,
            },
        )
        .await
    }
    pub async fn agent_clip_plan(
        &self,
        session: &str,
        job_id: &str,
        bounds: [f64; 4],
        name: &str,
    ) -> Result<Value> {
        self.agent_clip_polygon_plan(session, job_id, bounds, name, None)
            .await
    }
    async fn agent_clip_polygon_plan(
        &self,
        session: &str,
        job_id: &str,
        bounds: [f64; 4],
        name: &str,
        geometry: Option<crate::crop::PolygonGeometry>,
    ) -> Result<Value> {
        let (source, settled) = self
            .get_with_settled(job_id)
            .await
            .ok_or("Unknown source task.")?;
        if !settled || source.status != JobStatus::Succeeded || source.asset_key != "scl" {
            return Err("Choose a settled, completed local SCL file.".into());
        }
        let recipe = crate::RasterRecipe {
            schema_version: if geometry.is_some() {
                crate::processing::POLYGON_RECIPE_SCHEMA_VERSION
            } else {
                crate::processing::RECIPE_SCHEMA_VERSION
            }
            .into(),
            name: name.trim().into(),
            source: crate::processing::RecipeSource {
                job_id: source.id,
                sha256: source.sha256.ok_or("Source checksum is missing.")?,
            },
            operation: crate::processing::ClipOperation {
                operation_type: "clip".into(),
                crs: "EPSG:4326".into(),
                bounds,
                geometry,
            },
            output: crate::processing::RecipeOutput {
                format: "GeoTIFF".into(),
            },
        };
        self.agent_recipe_review_plan(session, recipe).await
    }
    pub async fn agent_recipe_review_plan(
        &self,
        session: &str,
        recipe: crate::RasterRecipe,
    ) -> Result<Value> {
        recipe.validate()?;
        let (source, settled) = self
            .get_with_settled(&recipe.source.job_id)
            .await
            .ok_or("Unknown source task.")?;
        if !settled || source.status != JobStatus::Succeeded {
            return Err("Choose a settled, completed local SCL file.".into());
        }
        let preflight = self.plan_recipe(recipe).await?;
        self.save_agent_plan(
            session,
            Action::Clip {
                recipe: preflight.recipe,
                output: preflight.plan,
            },
        )
        .await
    }
    async fn save_agent_plan(&self, session: &str, action: Action) -> Result<Value> {
        if !uuid(session) {
            return Err("Invalid Agent conversation ID.".into());
        }
        let created = Utc::now();
        let mut plan = Plan {
            id: Uuid::new_v4().to_string(),
            session_id: session.into(),
            policy: POLICY.into(),
            hash: String::new(),
            created_at: created.to_rfc3339(),
            expires_at: (created + chrono::Duration::minutes(TTL_MINUTES)).to_rfc3339(),
            action,
        };
        plan.hash = plan.effective_hash()?;
        plan.validate(session)?;
        write_record(&self.inner.root, "plans", &plan.id, &plan).await?;
        self.agent_plan_status(session, &plan.id).await
    }
    pub async fn agent_plan_status(&self, session: &str, id: &str) -> Result<Value> {
        let plan: Plan = read_record(&self.inner.root, "plans", id).await?;
        plan.validate_reference(session, id)?;
        if matches!(plan.action, Action::Vector { .. }) {
            return vector::status(self, &plan).await;
        }
        if matches!(
            plan.action,
            Action::StacProject { .. } | Action::StacDownload { .. }
        ) {
            return custom::status(self, &plan).await;
        }
        if matches!(
            plan.action,
            Action::WcsProject { .. } | Action::WcsDownload { .. }
        ) {
            return coverage::status(self, &plan).await;
        }
        let authorization = match &plan.action {
            Action::Download { query, .. } => protected::authorization(self, &query.provider).await,
            _ => None,
        };
        let projects = self.inner.projects.lock().await;
        let project = project::view(&plan, &projects)?;
        let project_committed = project.as_ref().is_some_and(|p| p["committed"] == true);
        let store = self.inner.store.lock().await;
        let jobs = existing(&plan, &store.jobs)?;
        let replacement = revision::replacement(&self.inner.root, &plan).await?;
        let status = if jobs.is_some() || project_committed {
            "submitted"
        } else if replacement.is_some() {
            "superseded"
        } else if plan.expired() {
            "expired"
        } else {
            "pending"
        };
        let files=match &plan.action {
            Action::Download { files,.. } => files.iter().map(|f|json!({"itemId":f.request.item_id,"assetKey":f.request.asset_key,"date":f.date,"bytes":f.pin.as_ref().map(|p|p.bytes)})).collect::<Vec<_>>(),
            Action::Clip { recipe,output } => vec![json!({"itemId":recipe.source.job_id,"assetKey":"scl","bytes":null,"width":output.width,"height":output.height})],
            Action::Project {request,..} => request.scenes.iter().map(|s|json!({"itemId":s.item_id,"assetKey":"scene","date":s.date,"bytes":null})).collect(),
            Action::Mosaic {spec,output,..} => spec.sources.iter().map(|s|json!({"itemId":store.jobs.get(&s.job_id).map(|j|j.item_id.as_str()).unwrap_or(&s.job_id),"assetKey":spec.asset_key,"bytes":null,"width":output.width,"height":output.height})).collect(),
            Action::Rgb {spec,..} => spec.sources.iter().map(|s|json!({"itemId":s.item_id,"assetKey":s.pin.band,"bytes":s.bytes,"width":spec.grid.width,"height":spec.grid.height})).collect(),
            Action::WcsProject { .. } | Action::WcsDownload { .. } | Action::StacProject { .. } | Action::StacDownload { .. } | Action::Vector { .. } => unreachable!(),
        };
        let (kind, source, bounds, start, end) = match &plan.action {
            Action::Download { query, .. } => (
                "download",
                catalog::source(&query.provider)?.name,
                query.bounds,
                catalog::source(&query.provider)?
                    .temporal
                    .then(|| query.start.clone()),
                catalog::source(&query.provider)?
                    .temporal
                    .then(|| query.end.clone()),
            ),
            Action::Clip { recipe, .. } => (
                "clip",
                "Verified local SCL",
                recipe.operation.bounds,
                None,
                None,
            ),
            Action::Project { request, .. } => (
                "project",
                "Selected catalog scenes",
                request.bounds,
                request
                    .scenes
                    .iter()
                    .filter(|s| catalog::scene_temporal(s))
                    .map(|s| s.date[..10].to_string())
                    .min(),
                request
                    .scenes
                    .iter()
                    .filter(|s| catalog::scene_temporal(s))
                    .map(|s| s.date[..10].to_string())
                    .max(),
            ),
            Action::Mosaic { project, .. } => (
                "mosaic",
                "Verified local project rasters",
                project.bounds,
                None,
                None,
            ),
            Action::Rgb { spec, .. } => (
                "rgb",
                "Verified local reflectance bands",
                spec.grid.bounds,
                None,
                None,
            ),
            Action::WcsProject { .. }
            | Action::WcsDownload { .. }
            | Action::StacProject { .. }
            | Action::StacDownload { .. }
            | Action::Vector { .. } => {
                unreachable!()
            }
        };
        let bounds_crs = match &plan.action {
            Action::Clip { recipe, output } if recipe.operation.crs == "source" => {
                output.crs.as_str()
            }
            Action::Rgb { spec, .. } => spec.grid.crs.as_str(),
            _ => "EPSG:4326",
        };
        let polygon = match &plan.action {
            Action::Project { request, .. } => request.geometry.as_ref(),
            Action::Clip { recipe, .. } => recipe.operation.geometry.as_ref(),
            Action::Mosaic { project, .. } => project.geometry.as_ref(),
            _ => None,
        }.map(|geometry| Ok::<_,String>(json!({"bounds":geometry.bounds()?,"sha256":digest(&serde_json::to_vec(geometry).map_err(io_error)?)}))).transpose()?;
        let format = match &plan.action {
            Action::Project { .. } => "Project",
            Action::Download { query, .. } => protected::format(&query.provider),
            _ => "GeoTIFF",
        };
        let area_coverage = footprint::action_scope(&plan.action)
            .map(|s| footprint::report(&s))
            .transpose()?;
        let polygon = if polygon.is_none() {
            match &plan.action {
            Action::Download { acquisition:Some(scope), .. } => scope.geometry.as_ref().map(|g| Ok::<_,String>(json!({"bounds":g.bounds()?,"sha256":digest(&serde_json::to_vec(g).map_err(io_error)?)}))).transpose()?,
            _ => None,
        }
        } else {
            polygon
        };
        Ok(
            json!({"planId":plan.id,"planHash":plan.hash,"kind":kind,"status":status,"source":source,"bounds":bounds,"boundsCrs":bounds_crs,"polygon":polygon,"areaCoverage":area_coverage,"start":start,"end":end,"files":files,
            "replacedBy":replacement.map(|r|r.plan_id),
            "expectedBytes":match &plan.action { Action::Download{files,..}=>files.iter().map(|f|f.pin.as_ref().map(|p|p.bytes)).sum::<Option<u64>>(),_=>None },
            "format":format,"authorization":authorization,"output":"Managed workspace files","expiresAt":plan.expires_at,"approvalRequired":true,"project":project,"processing":science::summary(&plan.action)?,
            "notes":match &plan.action { Action::Download{query,..}=>protected::download_notes(&query.provider),Action::Clip{..}=>vec!["Exact local SCL pixel window; no reprojection or resampling."],Action::Project{target,..}=>vec![if target.is_some(){"Append the listed scenes to the existing project; preserve its saved area and source assets. No file transfer."}else{"Save the listed catalog scenes and search area as a project. No file transfer."}],Action::Mosaic{..}=>vec!["Verified local sources, aligned original pixel grid and saved project area. No reprojection or resampling."],Action::Rgb{spec,..}=>vec![if spec.quality_mask.is_some(){"Accepted original RGB samples and calibration are retained. Screening replaces rejected pixels with NoData; no reprojection or resampling."}else{"Original RGB values and calibration are retained; no quality screening or resampling."}],Action::WcsProject{..}|Action::WcsDownload{..}|Action::StacProject{..}|Action::StacDownload{..}|Action::Vector{..}=>unreachable!() },
            "jobs":jobs.unwrap_or_default().iter().map(|job|job_view(job,!crate::active(&job.status)&&!store.active.contains_key(&job.id))).collect::<Vec<_>>() }),
        )
    }
    /// Called by a scoped desktop confirmation, never by the dynamic tool router.
    pub async fn approve_agent_plan(
        &self,
        session: &str,
        id: &str,
        expected_hash: &str,
    ) -> Result<Value> {
        let _commit = self.inner.agent_commits.lock().await;
        let plan: Plan = read_record(&self.inner.root, "plans", id).await?;
        plan.validate_reference(session, id)?;
        if expected_hash != plan.hash {
            return Err("Agent plan changed. Review the new plan before confirming.".into());
        }
        if revision::replacement(&self.inner.root, &plan)
            .await?
            .is_some()
        {
            return Err("This review was corrected. Confirm its replacement instead.".into());
        }
        if matches!(plan.action, Action::Project { .. }) {
            return self.commit_project_plan(&plan).await;
        }
        if matches!(plan.action, Action::Vector { .. }) {
            return vector::commit(self, &plan).await;
        }
        if matches!(
            plan.action,
            Action::StacProject { .. } | Action::StacDownload { .. }
        ) {
            return custom::commit(self, &plan).await;
        }
        if matches!(
            plan.action,
            Action::WcsProject { .. } | Action::WcsDownload { .. }
        ) {
            return coverage::commit(self, &plan).await;
        }
        {
            let store = self.inner.store.lock().await;
            if existing(&plan, &store.jobs)?.is_some() {
                drop(store);
                return self.agent_plan_status(session, id).await;
            }
        }
        // Replaying a committed receipt creates no new transfer. New approvals
        // must pass coverage, including legacy reviews with no footprints.
        if let Some(scope) = footprint::action_scope(&plan.action) {
            footprint::complete(&scope)?;
        }
        if plan.expired() {
            return Err("Agent plan expired. Create a new plan before confirming.".into());
        }
        match &plan.action {
            Action::Download { files, query, .. } => {
                protected::require_authorization(self, &query.provider).await?;
                if protected::account(&query.provider).is_some() {
                    protected::recheck(self, &query.provider, files).await?;
                }
                futures_util::future::try_join_all(files.iter().map(|file| async move {
                    if download_preflight(self, &file.request).await? != file.pin {
                        return Err("Source file changed. Create and review a new plan.".into());
                    }
                    Ok::<_, String>(())
                }))
                .await?;
            }
            Action::Clip { recipe, output } => {
                let fresh = self.plan_recipe(recipe.clone()).await?;
                if serde_json::to_value(fresh.plan).map_err(io_error)?
                    != serde_json::to_value(output).map_err(io_error)?
                {
                    return Err("Crop source or output changed. Create a new plan.".into());
                }
            }
            Action::Mosaic {
                project,
                project_hash,
                spec,
                output,
            } => {
                let (fresh, job) = self
                    .mosaic_review_job_with_selection(
                        &project.id,
                        &spec.asset_key,
                        science::selection(spec),
                    )
                    .await?;
                if ProjectScope::from_project(&fresh).fingerprint()? != *project_hash
                    || serde_json::to_value(&job.mosaic).map_err(io_error)?
                        != serde_json::to_value(Some(spec)).map_err(io_error)?
                    || serde_json::to_value(self.mosaic_preflight(&fresh, &job).await?)
                        .map_err(io_error)?
                        != serde_json::to_value(output).map_err(io_error)?
                {
                    return Err("The project, source files or output grid changed. Create a new processing plan.".into());
                }
            }
            Action::Rgb {
                spec,
                raw_bytes,
                required_disk_bytes,
                ..
            } => {
                let fresh = self.plan_scientific_rgb(spec.request()).await?;
                if &fresh.spec != spec
                    || fresh.raw_bytes != *raw_bytes
                    || fresh.required_disk_bytes != *required_disk_bytes
                {
                    return Err("Scientific RGB sources, quality policy or output grid changed. Create a new plan.".into());
                }
            }
            Action::Project { .. }
            | Action::StacProject { .. }
            | Action::StacDownload { .. }
            | Action::WcsProject { .. }
            | Action::WcsDownload { .. }
            | Action::Vector { .. } => {
                unreachable!()
            }
        }
        let protected_checked = match &plan.action {
            Action::Download { query, files, .. }
                if protected::account(&query.provider).is_some() =>
            {
                protected::verified_candidates(self, files).await?
            }
            _ => Vec::new(),
        };
        // Keep the pinned project stable until the queue commit. After admission,
        // mosaic workers use the immutable approved scope stored with their task.
        let projects = self.inner.projects.lock().await;
        match &plan.action {
            Action::Download {
                project: Some(pin), ..
            }
            | Action::Rgb {
                project: Some(pin), ..
            } => pin.verify(projects.get(&pin.id))?,
            Action::Mosaic {
                project,
                project_hash,
                ..
            } => {
                let current = projects
                    .get(&project.id)
                    .ok_or("The target project was removed. Create a new plan.")?;
                if ProjectScope::from_project(current).fingerprint()? != *project_hash {
                    return Err("The project changed. Create a new plan.".into());
                }
            }
            _ => (),
        }
        // Authorization removal and admission share this native account lock.
        // It is released before workers obtain/refresh credentials themselves.
        let authorization_guard = if let Action::Download { query, .. } = &plan.action {
            if let Some(provider) = protected::account(&query.provider) {
                let accounts = self.inner.accounts.lock().await;
                accounts.require_agent_download(provider)?;
                Some(accounts)
            } else {
                None
            }
        } else {
            None
        };
        let mut store = self.inner.store.lock().await;
        store.accepting_jobs()?;
        if existing(&plan, &store.jobs)?.is_some() {
            drop(store);
            drop(projects);
            drop(authorization_guard);
            return self.agent_plan_status(session, id).await;
        }
        if plan.expired() {
            return Err("Agent plan expired. Create a new plan.".into());
        }
        let mut retired = Vec::new();
        if let Action::Download { query, files, .. } = &plan.action {
            if protected::account(&query.provider).is_some() {
                for current in store.jobs.values().filter(|j| {
                    files.iter().any(|f| {
                        f.request.item_id == j.item_id
                            && f.request.asset_key == j.asset_key
                            && f.request.href == j.href
                    })
                }) {
                    if crate::active(&current.status) || store.active.contains_key(&current.id) {
                        return Err("An original product task arrived during review. Prepare a fresh download plan.".into());
                    }
                    if current.status == JobStatus::Succeeded {
                        let (_,verified)=protected_checked.iter().find(|(checked,_)|protected::same_receipt(checked,current)).ok_or("An original product receipt changed during verification. Prepare a fresh plan.")?;
                        if verified.is_ok() {
                            return Err("A verified original product arrived during review. Prepare a fresh download plan.".into());
                        }
                        retired.push(current.clone());
                    }
                }
            }
        }
        let ids = plan.job_ids();
        if store.active.len() + ids.len() > 64 {
            return Err("The task queue is full. Try confirming later.".into());
        }
        let approved_at = now();
        let mut created = Vec::new();
        match &plan.action {
            Action::Download { files, .. } => {
                for (file, id) in files.iter().zip(&ids) {
                    let mut job = crate::new_download_job(file.request.clone());
                    job.id = id.clone();
                    job.agent_approval = Some(approval(&plan, &approved_at, file.pin.clone()));
                    created.push(job);
                }
            }
            Action::Clip { recipe, .. } => {
                let source = crate::processing::validate_source(
                    recipe,
                    store.jobs.get(&recipe.source.job_id),
                )?;
                let mut job = crate::processing::clip_job(recipe.clone(), source);
                job.id = ids[0].clone();
                job.agent_approval = Some(approval(&plan, &approved_at, None));
                created.push(job);
            }
            Action::Mosaic { project, spec, .. } => {
                let snapshot = project.to_project(&approved_at);
                let mut job = crate::mosaic::mosaic_job(
                    &snapshot,
                    &store.jobs,
                    &spec.asset_key,
                    science::selection(spec),
                )?;
                if serde_json::to_value(&job.mosaic).map_err(io_error)?
                    != serde_json::to_value(Some(spec)).map_err(io_error)?
                {
                    return Err("Processing sources changed. Create a new plan.".into());
                }
                job.id = ids[0].clone();
                let mut receipt = approval(&plan, &approved_at, None);
                receipt.project_scope = Some(project.clone());
                job.agent_approval = Some(receipt);
                created.push(job);
            }
            Action::Rgb { spec, .. } => {
                if spec
                    .source_job_ids()
                    .iter()
                    .any(|id| store.active.contains_key(*id))
                {
                    return Err(
                        "Wait for every source task to settle before planning processing".into(),
                    );
                }
                let mut job = crate::raster::reflectance::composite::scientific::rgb_job(
                    spec.clone(),
                    &store.jobs,
                )?;
                job.id = ids[0].clone();
                job.agent_approval = Some(approval(&plan, &approved_at, None));
                created.push(job);
            }
            Action::Project { .. }
            | Action::StacProject { .. }
            | Action::StacDownload { .. }
            | Action::WcsProject { .. }
            | Action::WcsDownload { .. }
            | Action::Vector { .. } => {
                unreachable!()
            }
        }
        for original in &retired {
            let invalid = store.jobs.get_mut(&original.id).unwrap();
            invalid.status = JobStatus::Failed;
            invalid.error=Some("Previously completed original product failed its size/SHA-256 recheck; bytes retained, replacement approved.".into());
            invalid.updated_at = approved_at.clone();
            invalid.sha256 = None;
        }
        for job in &created {
            store.jobs.insert(job.id.clone(), job.clone());
        }
        if let Err(error) = self.persist(&store.jobs).await {
            for id in &ids {
                store.jobs.remove(id);
            }
            for original in retired {
                store.jobs.insert(original.id.clone(), original);
            }
            return Err(error);
        }
        for job in created {
            let token = CancellationToken::new();
            store.active.insert(job.id.clone(), token.clone());
            self.spawn(job.id, token);
        }
        drop(store);
        drop(projects);
        drop(authorization_guard);
        self.agent_plan_status(session, id).await
    }
}
fn approval(plan: &Plan, at: &str, remote: Option<RemotePin>) -> ApprovalReceipt {
    ApprovalReceipt {
        plan_id: plan.id.clone(),
        plan_hash: plan.hash.clone(),
        session_id: plan.session_id.clone(),
        approved_at: at.into(),
        policy: plan.policy.clone(),
        remote,
        project_scope: None,
    }
}
fn existing(
    plan: &Plan,
    jobs: &std::collections::BTreeMap<String, Job>,
) -> Result<Option<Vec<Job>>> {
    let ids = plan.job_ids();
    let found = ids
        .iter()
        .filter_map(|id| jobs.get(id).cloned())
        .collect::<Vec<_>>();
    if found.is_empty() {
        return Ok(None);
    }
    if found.len() != ids.len()
        || found.iter().any(|job| {
            job.agent_approval.as_ref().is_none_or(|a| {
                a.plan_id != plan.id
                    || a.plan_hash != plan.hash
                    || a.session_id != plan.session_id
                    || a.policy != plan.policy
            })
        })
    {
        return Err("Agent approval receipt conflicts with the task queue.".into());
    }
    Ok(Some(found))
}
fn job_view(job: &Job, settled: bool) -> Value {
    json!({"id":job.id,"title":job.title,"status":job.status,"settled":settled,"bytesDownloaded":job.bytes_downloaded,"totalBytes":job.total_bytes,"sha256":job.sha256})
}

pub fn definitions() -> Vec<Value> {
    let uuid = json!({"type":"string","format":"uuid"});
    let bounds = json!({"type":"array","items":{"type":"number"},"minItems":4,"maxItems":4});
    let schema = |properties: Value, required: Value| json!({"type":"object","properties":properties,"required":required,"additionalProperties":false});
    let item_ids = json!({"type":"array","items":{"type":"string"},"minItems":1,"maxItems":32});
    let boundary_reference = schema(
        json!({"id":uuid,"sha256":{"type":"string","pattern":"^[a-f0-9]{64}$"}}),
        json!(["id", "sha256"]),
    );
    let processing_keys = crate::providers::SOURCE_ASSET_KEYS
        .iter()
        .filter(|key| !matches!(**key, "product" | "viirs"))
        .collect::<Vec<_>>();
    let mut recipe_v1: Value = serde_json::from_str(include_str!(
        "../../../schemas/raster-recipe-v1.schema.json"
    ))
    .unwrap();
    let mut recipe_v2: Value = serde_json::from_str(include_str!(
        "../../../schemas/raster-recipe-v2.schema.json"
    ))
    .unwrap();
    for recipe in [&mut recipe_v1, &mut recipe_v2] {
        recipe.as_object_mut().unwrap().remove("$id");
        recipe.as_object_mut().unwrap().remove("$schema");
    }
    let download_keys = crate::providers::SOURCE_ASSET_KEYS
        .iter()
        .collect::<Vec<_>>();
    let mut definitions = vec![
        json!({"name":"geod_sources_list","description":"Read reviewed public adapters, product/date semantics, current native NASA/Copernicus authorization status and app account setup entry points. Read-only: no login, refresh or account verification. Saved/connected authorization is not original-product entitlement. Protected search and reviews are available; native confirmation requires saved authorization, and file access is unverified until the native task succeeds.","inputSchema":schema(json!({}),json!([]))}),
        json!({"name":"geod_workspace_context","description":"Read the attached desktop area/dates/source (may be null) and native current UTC date plus latest-imagery search defaults. An explicit named place overrides an unrelated current map area: use geod_region_search for administrative names or geod_place_search for a city gazetteer. Never guess coordinates.","inputSchema":schema(json!({}),json!([]))}),
        json!({"name":"geod_region_search","description":"Resolve global administrative names using actual source boundaries. Read-only. Bundled multilingual ADM0 country/region groupings and ADM1 provinces/states work offline. Detailed ADM2 through ADM5 use country-specific geoBoundaries gbOpen data with seven-day persistent cache, original license and geometry checksum. For detailed levels supply countryCode and adminLevel from geod_region_levels. Omitted level searches bundled ADM0/1 only. Match exact English, Chinese or local aliases where supplied; remote levels may require the established local or English name. ISO2 or ISO3 country filters; no guessed coordinates or universal city/county numbering. Returns real WGS84 envelopes and ambiguity, not a polygon, current legal status or complete imagery coverage.","inputSchema":schema(json!({"query":{"type":"string","minLength":1,"maxLength":120},"countryCode":{"type":"string","pattern":"^[A-Z]{2,3}$"},"adminLevel":{"type":"integer","minimum":0,"maximum":5},"limit":{"type":"integer","minimum":1,"maximum":10,"default":5}}),json!(["query"]))}),
        json!({"name":"geod_region_levels","description":"Read actual available geoBoundaries gbOpen administrative levels for an ISO2/ISO3 country, including each level's native name, unit count and source year. Read-only, cached. Use this before detailed city/county/district search when the country's numbering is uncertain. Missing levels are unavailable data, not permission errors. Do not ask the human to know administrative level numbers.","inputSchema":schema(json!({"countryCode":{"type":"string","pattern":"^[A-Z]{2,3}$"}}),json!(["countryCode"]))}),
        json!({"name":"geod_place_search","description":"Resolve a place explicitly named by the human using an actual bounded Photon/OpenStreetMap lookup, with a seven-day persistent cache and attribution. Read-only; no project, download or map mutation. kind=city excludes buildings/POIs (use for New York city); region selects county/state/country. Returns actual WGS84 extents; null bounds must not be invented. Prefer an exact relevant name over partial matches; ask only when plausible matches remain ambiguous. If a translated place has no match, retry its established English/local spelling. No arbitrary URLs or bulk lookup.","inputSchema":schema(json!({"query":{"type":"string","minLength":1,"maxLength":120},"kind":{"type":"string","enum":["city","region","place"],"default":"place"},"countryCode":{"type":"string","pattern":"^[A-Z]{2}$"}}),json!(["query"]))}),
        json!({"name":"geod_scene_search","description":"Search a bounded actual public catalog page from a reviewed provider. Read geod_sources_list first; DEM ignores dates/clouds, MODIS uses composite periods, radar has polarization-specific assets. Returns native searchId and validated scene IDs/assets; no source URLs. Not full coverage or download proof. Use context or user-specified WGS84 bounds, valid dates and cloud max.","inputSchema":schema(json!({"provider":{"type":"string","enum":catalog::IDS},"bounds":bounds,"start":{"type":"string"},"end":{"type":"string"},"cloudMax":{"type":"number","minimum":0,"maximum":100,"default":60},"limit":{"type":"integer","minimum":1,"maximum":20,"default":5}}),json!(["provider","bounds"]))}),
        json!({"name":"geod_scene_search_more","description":"Continue this session's native fixed-provider search using its pinned next page. Retains validated footprints across pages (up to 200 scenes). Pass only the latest returned searchId; no URL or changed filters. Then call geod_scene_coverage again. A missing GET continuation requires another date interval within the user's constraints, not silent cloud/date relaxation.","inputSchema":schema(json!({"searchId":uuid}),json!(["searchId"]))}),
        json!({"name":"geod_scene_coverage","description":"Read-only native union coverage of actual STAC scene footprints against the requested polygon or rectangle. Pass the source administrative boundary or useAttachedPolygon=true by default for polygon areas. Omit itemIds to obtain a newest-first covering selection up to 32 scenes; supply IDs to check that exact selection. Returns complete/partial/unknown and gap envelopes, never bbox-based proof, files or approval. Partial/unknown cannot become downloads. Continue native pages/dates within the human's constraints; ask a decision card before relaxing them. Catalog coverage is separate from valid pixels and local cloud conditions.","inputSchema":schema(json!({"searchId":uuid,"itemIds":item_ids,"boundary":boundary_reference,"useAttachedPolygon":{"type":"boolean","default":false}}),json!(["searchId"]))}),
        json!({"name":"geod_download_plan","description":"Create an immutable reviewable download plan for scene IDs from this conversation's native search. Public originals preflight sizes and ETags; protected originals use native authorization and transfer validation with encoded size unknown. Creates no download and no project; prefer project_plan then project_download_plan when the user wants project ownership. Text approval is insufficient.","inputSchema":schema(json!({"searchId":uuid,"itemIds":item_ids,"assetKey":{"type":"string","enum":download_keys}}),json!(["searchId","itemIds","assetKey"]))}),
        json!({"name":"geod_boundary_read","description":"Read the actual source polygon for a boundarySource returned by geod_place_search or geod_region_search. Census cities retain original TIGERweb rings including water areas; Natural Earth ADM0/ADM1 are offline reference boundaries; geoBoundaries detailed levels retain the source-provided simplified geometry and license. Return a session-scoped hash reference and provenance, never raw coordinates to the model. A failed request does not mean no boundary exists. Read-only: no project, image download or crop. Pass the exact returned boundary to project_plan/clip_plan for masking, preserving source precision and year.","inputSchema":schema(json!({"provider":{"type":"string","enum":["census","natural-earth","geoboundaries"]},"candidateId":{"type":"string","minLength":1,"maxLength":120},"lookupId":uuid,"countryCode":{"type":"string","pattern":"^[A-Z]{2,3}$"},"adminLevel":{"type":"integer","minimum":0,"maximum":5}}),json!(["provider","candidateId"]))}),
        json!({"name":"geod_project_plan","description":"Prepare a project review card from native search IDs. Supply name to create a project, OR projectId to append scenes. For a source administrative polygon, pass boundary exactly as returned by geod_boundary_read. Alternatively useAttachedPolygon=true preserves an attached native polygon. Choose one geometry source; do not invent vertices. Appending preserves the existing project's area and assets. No project or download is created until the user confirms the card.","inputSchema":schema(json!({"searchId":uuid,"itemIds":item_ids,"name":{"type":"string","maxLength":120},"projectId":uuid,"useAttachedPolygon":{"type":"boolean","default":false},"boundary":boundary_reference}),json!(["searchId","itemIds"]))}),
        json!({"name":"geod_project_download_plan","description":"Prepare downloads of a reviewed source asset for selected scenes in a confirmed saved project. Each plan uses one provider; select itemIds explicitly for a mixed project. Native active/completed matching tasks are reused; plan includes only missing files. Returns existing tasks if no files are missing. Requires a new card confirmation; does not execute.","inputSchema":schema(json!({"projectId":uuid,"assetKey":{"type":"string","enum":download_keys},"itemIds":item_ids}),json!(["projectId","assetKey"]))}),
        json!({"name":"geod_project_mosaic_plan","description":"Preflight saved-project mosaic/crop from settled local source files. Optional viQuality with good/usable policy selects complete NDVI/EVI observations using matched VI quality/reliability layers, never independent QA mosaics. All four files per scene must be local. Reads checksums, original grids, area and disk budget. No output until native confirmation. No reprojection/resampling.","inputSchema":schema(json!({"projectId":uuid,"assetKey":{"type":"string","enum":processing_keys},"viQuality":{"type":"object","properties":{"policy":{"type":"string","enum":["good","usable"]}},"required":["policy"],"additionalProperties":false}}),json!(["projectId","assetKey"]))}),
        json!({"name":"geod_scientific_rgb_plan","description":"Prepare a native review card for three completed local red, green, blue reflectance jobs, in that order. Optional matched MODIS QC/state or Landsat QA_PIXEL/QA_RADSAT qualityMask. Native preflight pins all original checksums, policy, calibration, exact output grid and disk budget, including coherent multi-scene observations. Int16/UInt16 DN retained; rejected pixels become NoData. No resampling, display stretch or output until card confirmation. Use geod_rgb_inspect/pixel for completed RGB results.","inputSchema":crate::mcp::agent_rgb_plan_schema()}),
        json!({"name":"geod_clip_plan","description":"Preflight a WGS84 crop of a settled local SCL file and create a reviewable plan. Pass boundary exactly as returned by geod_boundary_read for a source polygon, OR useAttachedPolygon=true for the original attached polygon. Exact source grid, no resampling. No file is created until the user confirms the native plan card.","inputSchema":schema(json!({"jobId":uuid,"bounds":bounds,"name":{"type":"string","maxLength":120},"useAttachedPolygon":{"type":"boolean","default":false},"boundary":boundary_reference}),json!(["jobId","bounds","name"]))}),
        json!({"name":"geod_recipe_review_plan","description":"Prepare a native review card for a versioned SHA-256-pinned local SCL crop recipe. Supports source/WGS84 rectangular windows (v1) and explicit WGS84 polygon masks (v2). Never invent a polygon; use the attached/user-specified geometry. Reuses native checksum, grid and mask preflight; writes no output until card confirmation.","inputSchema":schema(json!({"recipe":{"oneOf":[recipe_v1,recipe_v2]}}),json!(["recipe"]))}),
        json!({"name":"geod_plan_status","description":"Read this conversation's persisted plan and real results. Raster jobs complete only with succeeded AND settled=true. A submitted vector plan with vector.verified=true is a saved extraction result, with no raster jobs; verify that file through geod_vector_inspect. Never executes, retries or approves anything.","inputSchema":schema(json!({"planId":uuid}),json!(["planId"]))}),
    ];
    definitions.push(crate::mcp::agent_stac_search_definition());
    definitions.extend(custom::definitions());
    definitions.extend(coverage::definitions());
    definitions.extend(crate::mcp::agent_wcs_metadata_definitions());
    definitions.push(vector::definition());
    definitions
}
/// Keep the growing native tool dispatcher off Windows caller coroutine stacks.
/// Tool semantics remain in the shared native implementations.
pub fn call<'a>(
    manager: JobManager,
    session: &'a str,
    name: &'a str,
    args: Value,
    context: Option<MapContext>,
) -> futures_util::future::BoxFuture<'a, Result<Value>> {
    Box::pin(call_inner(manager, session, name, args, context))
}

async fn call_inner(
    manager: JobManager,
    session: &str,
    name: &str,
    args: Value,
    context: Option<MapContext>,
) -> Result<Value> {
    if !uuid(session) {
        return Err("Invalid Agent conversation ID.".into());
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct DownloadArgs {
        search_id: String,
        item_ids: Vec<String>,
        asset_key: String,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct ClipArgs {
        job_id: String,
        bounds: [f64; 4],
        name: String,
        #[serde(default)]
        use_attached_polygon: bool,
        boundary: Option<boundary::Reference>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct StatusArgs {
        plan_id: String,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct ProjectArgs {
        search_id: String,
        item_ids: Vec<String>,
        name: Option<String>,
        project_id: Option<String>,
        #[serde(default)]
        use_attached_polygon: bool,
        boundary: Option<boundary::Reference>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct ProjectDownloadArgs {
        project_id: String,
        asset_key: String,
        item_ids: Option<Vec<String>>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct MosaicArgs {
        project_id: String,
        asset_key: String,
        #[serde(default)]
        vi_quality: Option<crate::mosaic::vegetation::Request>,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RecipeArgs {
        recipe: crate::RasterRecipe,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RgbArgs {
        request: crate::RgbRequest,
    }
    let invalid = |_| "Invalid Agent tool arguments.".to_string();
    let mut result = match name {
        "geod_sources_list" => {
            if args != json!({}) {
                return Err("Sources tool takes no arguments.".into());
            }
            catalog::sources_with_accounts(&manager).await
        }
        "geod_workspace_context" => {
            if args != json!({}) {
                return Err("Context tool takes no arguments.".into());
            }
            if let Some(c) = &context {
                c.validate()?;
            }
            let attached_polygon = context.as_ref().and_then(|c|c.geometry.as_ref()).map(|geometry| {
                Ok::<_,String>(json!({"bounds":geometry.bounds()?,"sha256":digest(&serde_json::to_vec(geometry).map_err(io_error)?),"use":"Set useAttachedPolygon=true on geod_clip_plan. Original coordinates stay in the local context."}))
            }).transpose()?;
            let mut summary = context;
            if let Some(c) = &mut summary {
                c.geometry = None;
            }
            json!({"context":summary,"attachedPolygon":attached_polygon,"acquisitionProviders":catalog::IDS,"searchDefaults":places::latest_defaults(Utc::now().date_naive()),"approval":"Use the native plan card confirmation; model tools cannot approve."})
        }
        "geod_place_search" => {
            places::search(&manager, serde_json::from_value(args).map_err(invalid)?).await?
        }
        "geod_region_search" => {
            regions::search(&manager, serde_json::from_value(args).map_err(invalid)?).await?
        }
        "geod_region_levels" => {
            regions::levels(&manager, serde_json::from_value(args).map_err(invalid)?).await?
        }
        "geod_boundary_read" => {
            boundary::read(
                &manager,
                session,
                serde_json::from_value(args).map_err(invalid)?,
            )
            .await?
        }
        "geod_scene_search" => {
            manager
                .agent_search(session, serde_json::from_value(args).map_err(invalid)?)
                .await?
        }
        "geod_scene_search_more" => search_more::more(&manager, session, args).await?,
        "geod_scene_coverage" => footprint::check(&manager, session, args, context).await?,
        "geod_wcs_describe" | "geod_wcs_prepare" => {
            crate::mcp::agent_wcs_metadata(manager.clone(), name, args).await?
        }
        "geod_wcs_project_plan" => {
            manager
                .agent_wcs_project_plan(session, serde_json::from_value(args).map_err(invalid)?)
                .await?
        }
        "geod_wcs_download_plan" => {
            manager
                .agent_wcs_download_plan(session, serde_json::from_value(args).map_err(invalid)?)
                .await?
        }
        "geod_stac_search" => crate::mcp::agent_stac_search(manager.clone(), args).await?,
        "geod_vector_extract_plan" => {
            vector::prepare(
                &manager,
                session,
                serde_json::from_value(args).map_err(invalid)?,
                context,
            )
            .await?
        }
        "geod_stac_project_plan" => {
            manager
                .agent_stac_project_plan(session, serde_json::from_value(args).map_err(invalid)?)
                .await?
        }
        "geod_stac_download_plan" => {
            manager
                .agent_stac_download_plan(session, serde_json::from_value(args).map_err(invalid)?)
                .await?
        }
        "geod_download_plan" => {
            let a: DownloadArgs = serde_json::from_value(args).map_err(invalid)?;
            manager
                .agent_download_plan(session, &a.search_id, a.item_ids, &a.asset_key)
                .await?
        }
        "geod_clip_plan" => {
            let a: ClipArgs = serde_json::from_value(args).map_err(invalid)?;
            let geometry = source_geometry(
                &manager,
                session,
                context,
                a.use_attached_polygon,
                a.boundary,
            )
            .await?;
            manager
                .agent_clip_polygon_plan(session, &a.job_id, a.bounds, &a.name, geometry)
                .await?
        }
        "geod_recipe_review_plan" => {
            let a: RecipeArgs = serde_json::from_value(args).map_err(invalid)?;
            manager.agent_recipe_review_plan(session, a.recipe).await?
        }
        "geod_project_plan" => {
            let a: ProjectArgs = serde_json::from_value(args).map_err(invalid)?;
            let geometry = source_geometry(
                &manager,
                session,
                context,
                a.use_attached_polygon,
                a.boundary,
            )
            .await?;
            manager
                .agent_project_polygon_plan(
                    session,
                    &a.search_id,
                    a.item_ids,
                    a.name,
                    a.project_id,
                    geometry,
                )
                .await?
        }
        "geod_project_download_plan" => {
            let a: ProjectDownloadArgs = serde_json::from_value(args).map_err(invalid)?;
            manager
                .agent_project_download_plan(session, &a.project_id, &a.asset_key, a.item_ids)
                .await?
        }
        "geod_project_mosaic_plan" => {
            let a: MosaicArgs = serde_json::from_value(args).map_err(invalid)?;
            manager
                .agent_project_mosaic_plan_with_selection(
                    session,
                    &a.project_id,
                    &a.asset_key,
                    a.vi_quality,
                )
                .await?
        }
        "geod_scientific_rgb_plan" => {
            let a: RgbArgs = serde_json::from_value(args).map_err(invalid)?;
            manager
                .agent_scientific_rgb_plan(session, a.request)
                .await?
        }
        "geod_plan_status" => {
            let a: StatusArgs = serde_json::from_value(args).map_err(invalid)?;
            manager.agent_plan_status(session, &a.plan_id).await?
        }
        _ => crate::mcp::agent_read_call(manager, name, args).await?,
    };
    if matches!(
        name,
        "geod_wcs_project_plan"
            | "geod_wcs_download_plan"
            | "geod_stac_project_plan"
            | "geod_stac_download_plan"
            | "geod_plan_status"
            | "geod_vector_extract_plan"
    ) {
        crate::mcp::sanitize_agent_value(&mut result);
    }
    if result.to_string().len() > 32768 {
        return Err("Agent result exceeds 32 KiB.".into());
    }
    Ok(result)
}

async fn source_geometry(
    manager: &JobManager,
    session: &str,
    context: Option<MapContext>,
    attached: bool,
    boundary: Option<boundary::Reference>,
) -> Result<Option<crate::crop::PolygonGeometry>> {
    if let Some(reference) = boundary {
        if attached {
            return Err("Choose one polygon source for the review.".into());
        }
        return boundary::resolve(manager, session, reference)
            .await
            .map(Some);
    }
    attached_geometry(context, attached)
}
fn attached_geometry(
    context: Option<MapContext>,
    requested: bool,
) -> Result<Option<crate::crop::PolygonGeometry>> {
    if !requested {
        return Ok(None);
    }
    let context = context.ok_or("No attached polygon is available.")?;
    context.validate()?;
    Ok(Some(
        context
            .geometry
            .ok_or("No attached polygon is available.")?,
    ))
}

#[cfg(test)]
mod tests;
