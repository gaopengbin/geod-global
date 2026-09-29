//! Persisted scene sets. A project records selected catalog assets and an area;
//! downloads remain ordinary verified jobs and are never mistaken for a mosaic.
use crate::{
    crop::PolygonGeometry, io_error, now, validate_request, CreateJobRequest, Job, JobManager,
    JobStatus, Result,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub const MAX_PROJECT_SCENES: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectAsset {
    pub href: String,
    pub media_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectScene {
    pub item_id: String,
    pub date: String,
    pub cloud: Option<f64>,
    pub crs: Option<String>,
    pub grid_code: Option<String>,
    pub bbox: [f64; 4],
    pub assets: BTreeMap<String, ProjectAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateProjectRequest {
    pub name: String,
    pub bounds: [f64; 4],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<PolygonGeometry>,
    pub scenes: Vec<ProjectScene>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub bounds: [f64; 4],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<PolygonGeometry>,
    pub scenes: Vec<ProjectScene>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDownloads {
    pub project_id: String,
    pub asset_key: String,
    pub jobs: Vec<Job>,
}

fn valid_bounds(bounds: [f64; 4]) -> bool {
    bounds.iter().all(|value| value.is_finite())
        && bounds[0] >= -180.0
        && bounds[2] <= 180.0
        && bounds[1] >= -90.0
        && bounds[3] <= 90.0
        && bounds[0] < bounds[2]
        && bounds[1] < bounds[3]
}

impl CreateProjectRequest {
    fn validate(&self, fixture_origin: Option<&str>) -> Result<()> {
        if self.name.trim().is_empty()
            || self.name.chars().count() > 120
            || self.name.chars().any(char::is_control)
        {
            return Err("Project name must contain 1 to 120 printable characters".into());
        }
        if !valid_bounds(self.bounds) {
            return Err(
                "Project bounds must be an increasing WGS84 west/south/east/north rectangle".into(),
            );
        }
        if let Some(geometry) = &self.geometry {
            let polygon_bounds = geometry.bounds()?;
            if polygon_bounds[0] >= self.bounds[2]
                || polygon_bounds[2] <= self.bounds[0]
                || polygon_bounds[1] >= self.bounds[3]
                || polygon_bounds[3] <= self.bounds[1]
            {
                return Err("Project polygon does not intersect its saved area".into());
            }
        }
        if self.scenes.is_empty() || self.scenes.len() > MAX_PROJECT_SCENES {
            return Err(format!(
                "Select 1 to {MAX_PROJECT_SCENES} scenes for a project"
            ));
        }
        let mut ids = HashSet::new();
        for scene in &self.scenes {
            if !ids.insert(&scene.item_id) {
                return Err("A project cannot contain the same scene twice".into());
            }
            if scene.date.len() < 10
                || scene.date.len() > 64
                || !scene.date.is_ascii()
                || chrono::DateTime::parse_from_rfc3339(&scene.date).is_err()
                || !valid_bounds(scene.bbox)
                || scene
                    .cloud
                    .is_some_and(|cloud| !cloud.is_finite() || !(0.0..=100.0).contains(&cloud))
                || scene.crs.as_ref().is_some_and(|crs| crs.len() > 32)
                || scene.grid_code.as_ref().is_some_and(|code| code.len() > 80)
            {
                return Err("Project scene metadata is invalid".into());
            }
            if scene.assets.is_empty()
                || scene.assets.len() > 2
                || scene
                    .assets
                    .keys()
                    .any(|key| key != "scl" && key != "visual")
            {
                return Err("Project scenes require SCL and/or true-color assets".into());
            }
            for (key, asset) in &scene.assets {
                validate_request(
                    &CreateJobRequest {
                        item_id: scene.item_id.clone(),
                        asset_key: key.clone(),
                        href: asset.href.clone(),
                        media_type: asset.media_type.clone(),
                        title: None,
                    },
                    fixture_origin,
                )?;
            }
        }
        Ok(())
    }
}

pub(crate) async fn load_projects(
    root: &std::path::Path,
    fixture_origin: Option<&str>,
) -> Result<BTreeMap<String, Project>> {
    let path = root.join("projects.json");
    let projects: BTreeMap<String, Project> = match tokio::fs::read(path).await {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|e| format!("Cannot read saved projects: {e}"))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
        Err(error) => return Err(io_error(error)),
    };
    for (id, project) in &projects {
        if id != &project.id
            || Uuid::parse_str(id)
                .ok()
                .map(|value| value.to_string())
                .as_deref()
                != Some(id)
        {
            return Err("Saved project has an invalid identifier".into());
        }
        CreateProjectRequest {
            name: project.name.clone(),
            bounds: project.bounds,
            geometry: project.geometry.clone(),
            scenes: project.scenes.clone(),
        }
        .validate(fixture_origin)?;
    }
    Ok(projects)
}

impl JobManager {
    pub async fn list_projects(&self) -> Vec<Project> {
        let mut projects: Vec<_> = self.inner.projects.lock().await.values().cloned().collect();
        projects.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        projects
    }

    pub async fn create_project(&self, request: CreateProjectRequest) -> Result<Project> {
        #[cfg(test)]
        let origin = self.inner.fixture_origin.as_deref();
        #[cfg(not(test))]
        let origin = None;
        request.validate(origin)?;
        let timestamp = now();
        let project = Project {
            id: Uuid::new_v4().to_string(),
            name: request.name,
            bounds: request.bounds,
            geometry: request.geometry,
            scenes: request.scenes,
            created_at: timestamp.clone(),
            updated_at: timestamp,
        };
        let mut projects = self.inner.projects.lock().await;
        projects.insert(project.id.clone(), project.clone());
        if let Err(error) = self.persist_projects(&projects).await {
            projects.remove(&project.id);
            return Err(error);
        }
        Ok(project)
    }

    async fn persist_projects(&self, projects: &BTreeMap<String, Project>) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(projects).map_err(io_error)?;
        let temporary = self.inner.root.join("projects.json.tmp");
        let mut file = tokio::fs::File::create(&temporary)
            .await
            .map_err(io_error)?;
        file.write_all(&bytes).await.map_err(io_error)?;
        file.sync_all().await.map_err(io_error)?;
        drop(file);
        tokio::fs::rename(temporary, self.inner.root.join("projects.json"))
            .await
            .map_err(io_error)
    }

    pub async fn enqueue_project(&self, id: &str, asset_key: &str) -> Result<ProjectDownloads> {
        if asset_key != "scl" && asset_key != "visual" {
            return Err("Choose SCL or true-color imagery for the project download".into());
        }
        let project = self
            .inner
            .projects
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or("Unknown project")?;
        let mut store = self.inner.store.lock().await;
        let mut jobs = Vec::with_capacity(project.scenes.len());
        let mut created = Vec::new();
        for scene in &project.scenes {
            let asset = scene.assets.get(asset_key).ok_or_else(|| {
                format!(
                    "Scene {} does not provide a {} asset",
                    scene.item_id, asset_key
                )
            })?;
            if let Some(existing) = store.jobs.values().find(|job| {
                job.item_id == scene.item_id
                    && job.asset_key == asset_key
                    && job.href == asset.href
                    && matches!(
                        job.status,
                        JobStatus::Succeeded | JobStatus::Queued | JobStatus::Running
                    )
            }) {
                jobs.push(existing.clone());
                continue;
            }
            let job = crate::new_download_job(CreateJobRequest {
                item_id: scene.item_id.clone(),
                asset_key: asset_key.into(),
                href: asset.href.clone(),
                media_type: asset.media_type.clone(),
                title: Some(format!("{} · {}", scene.item_id, asset_key.to_uppercase())),
            });
            jobs.push(job.clone());
            created.push(job);
        }
        if store.active.len() + created.len() > 64 {
            return Err("The local download queue has room for fewer scenes; wait for current tasks to finish".into());
        }
        for job in &created {
            store.jobs.insert(job.id.clone(), job.clone());
        }
        if let Err(error) = self.persist(&store.jobs).await {
            for job in &created {
                store.jobs.remove(&job.id);
            }
            return Err(error);
        }
        for job in created {
            let token = CancellationToken::new();
            store.active.insert(job.id.clone(), token.clone());
            self.spawn(job.id, token);
        }
        Ok(ProjectDownloads {
            project_id: project.id,
            asset_key: asset_key.into(),
            jobs,
        })
    }
}
