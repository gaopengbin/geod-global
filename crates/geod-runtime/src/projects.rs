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
pub struct AddProjectScenesRequest {
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

fn validate_project_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 120 || name.chars().any(char::is_control) {
        return Err("Project name must contain 1 to 120 printable characters".into());
    }
    Ok(name.to_owned())
}

impl CreateProjectRequest {
    fn validate(&self, fixture_origin: Option<&str>) -> Result<()> {
        validate_project_name(&self.name)?;
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
            name: validate_project_name(&request.name)?,
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

    pub async fn rename_project(&self, id: &str, name: &str) -> Result<Project> {
        let name = validate_project_name(name)?;
        let mut projects = self.inner.projects.lock().await;
        let previous = projects.get(id).cloned().ok_or("Unknown project")?;
        let mut renamed = previous.clone();
        renamed.name = name;
        renamed.updated_at = now();
        projects.insert(id.to_owned(), renamed.clone());
        if let Err(error) = self.persist_projects(&projects).await {
            projects.insert(id.to_owned(), previous);
            return Err(error);
        }
        Ok(renamed)
    }

    pub async fn add_project_scenes(
        &self,
        id: &str,
        request: AddProjectScenesRequest,
    ) -> Result<Project> {
        if request.scenes.is_empty() || request.scenes.len() > MAX_PROJECT_SCENES {
            return Err(format!(
                "Select 1 to {MAX_PROJECT_SCENES} scenes to add to the project"
            ));
        }
        let mut projects = self.inner.projects.lock().await;
        let previous = projects.get(id).cloned().ok_or("Unknown project")?;
        let mut updated = previous.clone();
        let mut ids: HashSet<_> = updated
            .scenes
            .iter()
            .map(|scene| scene.item_id.clone())
            .collect();
        for scene in request.scenes {
            // A catalogue refresh must not replace an already pinned source or metadata.
            if ids.insert(scene.item_id.clone()) {
                updated.scenes.push(scene);
            }
        }
        #[cfg(test)]
        let origin = self.inner.fixture_origin.as_deref();
        #[cfg(not(test))]
        let origin = None;
        CreateProjectRequest {
            name: updated.name.clone(),
            bounds: updated.bounds,
            geometry: updated.geometry.clone(),
            scenes: updated.scenes.clone(),
        }
        .validate(origin)?;
        if updated.scenes.len() == previous.scenes.len() {
            return Ok(previous);
        }
        updated.updated_at = now();
        projects.insert(id.to_owned(), updated.clone());
        if let Err(error) = self.persist_projects(&projects).await {
            projects.insert(id.to_owned(), previous);
            return Err(error);
        }
        Ok(updated)
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
        self.enqueue_project_selection(id, asset_key, None).await
    }

    pub async fn enqueue_project_selection(
        &self,
        id: &str,
        asset_key: &str,
        item_ids: Option<Vec<String>>,
    ) -> Result<ProjectDownloads> {
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
        let selected = item_ids.map(|ids| ids.into_iter().collect::<HashSet<_>>());
        if let Some(ids) = &selected {
            if ids.is_empty()
                || ids
                    .iter()
                    .any(|id| !project.scenes.iter().any(|scene| &scene.item_id == id))
            {
                return Err("Choose scene IDs belonging to this project".into());
            }
        }
        let mut store = self.inner.store.lock().await;
        store.accepting_jobs()?;
        let mut jobs = Vec::with_capacity(project.scenes.len());
        let mut created = Vec::new();
        for scene in &project.scenes {
            if selected
                .as_ref()
                .is_some_and(|ids| !ids.contains(&scene.item_id))
            {
                continue;
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    fn project_request() -> CreateProjectRequest {
        CreateProjectRequest {
            name: "  Original project  ".into(),
            bounds: [-123.0, 37.0, -122.0, 38.0],
            geometry: None,
            scenes: vec![ProjectScene {
                item_id: "S2C_TEST".into(),
                date: "2026-09-28T00:00:00Z".into(),
                cloud: Some(0.0),
                crs: Some("EPSG:32610".into()),
                grid_code: None,
                bbox: [-123.0, 37.0, -122.0, 38.0],
                assets: BTreeMap::from([(
                    "scl".into(),
                    ProjectAsset {
                        href: format!(
                            "https://{}/sentinel-s2-l2a-cogs/10/S/EG/2026/9/S2C_TEST/SCL.tif",
                            crate::SOURCE_HOST
                        ),
                        media_type: "image/tiff; application=geotiff".into(),
                    },
                )]),
            }],
        }
    }

    fn scene_with_id(original: &ProjectScene, id: &str) -> ProjectScene {
        let mut scene = original.clone();
        scene.item_id = id.into();
        for asset in scene.assets.values_mut() {
            asset.href = asset.href.replace(&original.item_id, id);
        }
        scene
    }

    #[tokio::test]
    async fn adding_scenes_preserves_identity_area_and_pinned_sources_and_is_idempotent() {
        let directory = tempfile::tempdir().unwrap();
        let manager = JobManager::open(directory.path()).await.unwrap();
        let original = manager.create_project(project_request()).await.unwrap();
        let mut existing = original.scenes[0].clone();
        existing.assets.get_mut("scl").unwrap().href =
            "https://untrusted.example/changed.tif".into();
        let added = scene_with_id(&original.scenes[0], "S2_ADDED");
        let request = AddProjectScenesRequest {
            scenes: vec![existing, added.clone(), added],
        };
        let updated = manager
            .add_project_scenes(&original.id, request.clone())
            .await
            .unwrap();
        assert_eq!(updated.id, original.id);
        assert_eq!(updated.name, original.name);
        assert_eq!(updated.created_at, original.created_at);
        assert_eq!(updated.bounds, original.bounds);
        assert_eq!(
            serde_json::to_value(&updated.geometry).unwrap(),
            serde_json::to_value(&original.geometry).unwrap()
        );
        assert_eq!(updated.scenes.len(), 2);
        assert_eq!(
            updated.scenes[0].assets["scl"].href,
            original.scenes[0].assets["scl"].href
        );
        assert_eq!(
            manager
                .add_project_scenes(&original.id, request)
                .await
                .unwrap()
                .scenes
                .len(),
            2
        );
        drop(manager);
        assert_eq!(
            JobManager::open(directory.path())
                .await
                .unwrap()
                .list_projects()
                .await[0]
                .scenes
                .len(),
            2
        );
    }

    #[tokio::test]
    async fn invalid_or_excessive_additions_do_not_modify_the_saved_project() {
        let directory = tempfile::tempdir().unwrap();
        let manager = JobManager::open(directory.path()).await.unwrap();
        let original = manager.create_project(project_request()).await.unwrap();
        let mut invalid = original.scenes[0].clone();
        invalid.item_id = "S2_INVALID".into();
        invalid.assets.get_mut("scl").unwrap().href = "https://untrusted.example/source.tif".into();
        assert!(manager
            .add_project_scenes(
                &original.id,
                AddProjectScenesRequest {
                    scenes: vec![invalid]
                }
            )
            .await
            .is_err());
        let scenes = (0..MAX_PROJECT_SCENES)
            .map(|index| scene_with_id(&original.scenes[0], &format!("S2_NEW_{index}")))
            .collect();
        assert!(manager
            .add_project_scenes(&original.id, AddProjectScenesRequest { scenes })
            .await
            .is_err());
        assert_eq!(manager.list_projects().await[0].scenes.len(), 1);
        assert!(manager
            .add_project_scenes(
                "missing",
                AddProjectScenesRequest {
                    scenes: original.scenes.clone()
                }
            )
            .await
            .is_err());
    }

    #[tokio::test]
    async fn selected_download_reuses_only_selected_sources_and_rejects_unknown_ids() {
        let directory = tempfile::tempdir().unwrap();
        let manager = JobManager::open(directory.path()).await.unwrap();
        let mut request = project_request();
        let added = scene_with_id(&request.scenes[0], "S2_OTHER");
        request.scenes.push(added);
        let project = manager.create_project(request).await.unwrap();
        let bytes = crate::raster::tests::fixture(2, 2, &[1, 2, 3, 4], 32610, false);
        let mut source = crate::raster::tests::record(manager.storage_root(), &bytes);
        source.item_id = project.scenes[0].item_id.clone();
        source.asset_key = "scl".into();
        source.href = project.scenes[0].assets["scl"].href.clone();
        manager
            .inner
            .store
            .lock()
            .await
            .jobs
            .insert(source.id.clone(), source.clone());
        for ids in [vec![], vec!["not-in-project".into()]] {
            assert!(manager
                .enqueue_project_selection(&project.id, "scl", Some(ids))
                .await
                .is_err());
        }
        let result = manager
            .enqueue_project_selection(&project.id, "scl", Some(vec![source.item_id.clone()]))
            .await
            .unwrap();
        assert_eq!(result.jobs.len(), 1);
        assert_eq!(result.jobs[0].id, source.id);
        assert_eq!(manager.list().await.len(), 1);
    }

    #[tokio::test]
    async fn add_scenes_http_route_checks_origin_and_client_and_keeps_project_id() {
        let directory = tempfile::tempdir().unwrap();
        let manager = JobManager::open(directory.path()).await.unwrap();
        let project = manager.create_project(project_request()).await.unwrap();
        let added = scene_with_id(&project.scenes[0], "S2_HTTP_ADDED");
        let payload = serde_json::to_string(&AddProjectScenesRequest {
            scenes: vec![added],
        })
        .unwrap();
        let app = crate::service::router(manager);
        for (origin, client, expected) in [
            ("https://evil.example", true, StatusCode::FORBIDDEN),
            (crate::service::ALLOWED_ORIGIN, false, StatusCode::FORBIDDEN),
            (crate::service::ALLOWED_ORIGIN, true, StatusCode::OK),
        ] {
            let mut request = Request::builder()
                .method("POST")
                .uri(format!("/projects/{}/scenes", project.id))
                .header("Host", "127.0.0.1:4318")
                .header("Origin", origin)
                .header("Content-Type", "application/json");
            if client {
                request = request.header("X-GeoD-Client", "geod-global");
            }
            let response = app
                .clone()
                .oneshot(request.body(Body::from(payload.clone())).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), expected);
            if expected == StatusCode::OK {
                let body = to_bytes(response.into_body(), 100_000).await.unwrap();
                let saved: Project = serde_json::from_slice(&body).unwrap();
                assert_eq!(saved.id, project.id);
                assert_eq!(saved.scenes.len(), 2);
            }
        }
    }

    #[tokio::test]
    async fn rename_is_persisted_and_preserves_project_identity_and_sources() {
        let directory = tempfile::tempdir().unwrap();
        let manager = JobManager::open(directory.path()).await.unwrap();
        let project = manager.create_project(project_request()).await.unwrap();
        assert_eq!(project.name, "Original project");
        for invalid in [
            " ".to_owned(),
            "x".repeat(121),
            "name\nwith control".to_owned(),
        ] {
            assert!(manager.rename_project(&project.id, &invalid).await.is_err());
            assert_eq!(manager.list_projects().await[0].name, project.name);
        }
        assert!(manager
            .rename_project("unknown", "Valid name")
            .await
            .is_err());
        let renamed = manager
            .rename_project(&project.id, "  湾区影像工程  ")
            .await
            .unwrap();
        assert_eq!(renamed.id, project.id);
        assert_eq!(renamed.created_at, project.created_at);
        assert_eq!(renamed.bounds, project.bounds);
        assert_eq!(
            renamed.scenes[0].assets["scl"].href,
            project.scenes[0].assets["scl"].href
        );
        drop(manager);
        let reopened = JobManager::open(directory.path()).await.unwrap();
        assert_eq!(reopened.list_projects().await[0].name, "湾区影像工程");
    }

    #[tokio::test]
    async fn rename_http_route_requires_mutation_guard_and_returns_saved_project() {
        let directory = tempfile::tempdir().unwrap();
        let manager = JobManager::open(directory.path()).await.unwrap();
        let project = manager.create_project(project_request()).await.unwrap();
        let app = crate::service::router(manager);
        for (origin, client, expected) in [
            ("https://evil.example", true, StatusCode::FORBIDDEN),
            (crate::service::ALLOWED_ORIGIN, false, StatusCode::FORBIDDEN),
            (crate::service::ALLOWED_ORIGIN, true, StatusCode::OK),
        ] {
            let mut request = Request::builder()
                .method("POST")
                .uri(format!("/projects/{}/rename", project.id))
                .header("Host", "127.0.0.1:4318")
                .header("Origin", origin)
                .header("Content-Type", "application/json");
            if client {
                request = request.header("X-GeoD-Client", "geod-global");
            }
            let response = app
                .clone()
                .oneshot(
                    request
                        .body(Body::from(r#"{"name":"HTTP renamed"}"#))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), expected);
            if expected == StatusCode::OK {
                let body = to_bytes(response.into_body(), 100_000).await.unwrap();
                let saved: Project = serde_json::from_slice(&body).unwrap();
                assert_eq!(saved.name, "HTTP renamed");
                assert_eq!(saved.id, project.id);
            }
        }
    }
}
