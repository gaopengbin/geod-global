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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raster_band: Option<ReflectanceBand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReflectanceBand {
    pub data_type: String,
    pub scale: f64,
    pub offset: f64,
    pub nodata: f64,
    pub spatial_resolution: f64,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stac_items: Vec<crate::stac::ProjectItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wcs_items: Vec<crate::wcs::ProjectItem>,
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

pub(crate) fn valid_bounds(bounds: [f64; 4]) -> bool {
    bounds.iter().all(|value| value.is_finite())
        && bounds[0] >= -180.0
        && bounds[2] <= 180.0
        && bounds[1] >= -90.0
        && bounds[3] <= 90.0
        && bounds[0] < bounds[2]
        && bounds[1] < bounds[3]
}

pub(crate) fn validate_project_name(name: &str) -> Result<String> {
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
                || scene.assets.len() > crate::providers::SOURCE_ASSET_KEYS.len()
                || scene
                    .assets
                    .keys()
                    .any(|key| !crate::providers::SOURCE_ASSET_KEYS.contains(&key.as_str()))
            {
                return Err("Project scenes require supported source raster assets".into());
            }
            if let Some(period) = crate::providers::vegetation::period(&scene.item_id) {
                let expected =
                    chrono::DateTime::parse_from_rfc3339(&period[0]).map_err(crate::io_error)?;
                let date =
                    chrono::DateTime::parse_from_rfc3339(&scene.date).map_err(crate::io_error)?;
                if date != expected || scene.crs.as_deref() != Some(crate::providers::modis::CRS) {
                    return Err("MODIS vegetation project date or sinusoidal CRS differs from its pinned product".into());
                }
            }
            for (key, asset) in &scene.assets {
                if (crate::providers::modis::QUALITY_KEYS.contains(&key.as_str())
                    || crate::raster::landsat_quality::KEYS.contains(&key.as_str()))
                    && asset.raster_band.is_some()
                {
                    return Err("Quality bit fields must not carry reflectance calibration".into());
                }
                if (matches!(key.as_str(), "red" | "green" | "blue")
                    || crate::providers::vegetation::is_key(key))
                    && asset.raster_band.is_none()
                {
                    return Err(
                        "Reflectance source bands require their original conversion metadata"
                            .into(),
                    );
                }
                if let Some(band) = &asset.raster_band {
                    let hls = url::Url::parse(&asset.href)
                        .is_ok_and(|url| url.host_str() == Some(crate::providers::nasa::HOST));
                    let modis = url::Url::parse(&asset.href)
                        .is_ok_and(|url| url.host_str() == Some(crate::providers::modis::HOST));
                    let vegetation = crate::providers::vegetation::is_key(key);
                    let calibration = if vegetation {
                        let layer = crate::providers::vegetation::layer(key).unwrap();
                        modis
                            && band.data_type == layer.data_type
                            && band.scale == layer.scale
                            && band.offset == 0.0
                            && band.nodata == f64::from(layer.nodata)
                    } else if modis {
                        band.data_type == "int16"
                            && band.scale == 0.0001
                            && band.offset == 0.0
                            && band.nodata == -28672.0
                    } else if hls {
                        band.data_type == "int16"
                            && band.scale == 0.0001
                            && band.offset == 0.0
                            && band.nodata == -9999.0
                    } else {
                        band.data_type == "uint16"
                            && band.scale == 0.0000275
                            && band.offset == -0.2
                            && band.nodata == 0.0
                    };
                    if !(matches!(key.as_str(), "red" | "green" | "blue") || vegetation)
                        || !calibration
                        || band.spatial_resolution
                            != if vegetation {
                                250.0
                            } else if modis {
                                500.0
                            } else {
                                30.0
                            }
                    {
                        return Err("Unsupported reflectance band metadata".into());
                    }
                }
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
            if scene
                .assets
                .keys()
                .any(|key| crate::raster::landsat_quality::KEYS.contains(&key.as_str()))
            {
                let mut directory = None;
                for (key, asset) in &scene.assets {
                    if matches!(
                        key.as_str(),
                        "red" | "green" | "blue" | "qa_pixel" | "qa_radsat"
                    ) {
                        let parent = asset.href.rsplit_once('/').map(|(p, _)| p);
                        if directory.is_some_and(|p| Some(p) != parent) {
                            return Err("Landsat QA and RGB must retain the exact same original processing directory".into());
                        }
                        directory = parent;
                    }
                }
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
        crate::stac_projects::validate_project(root, project)?;
        if !project.scenes.is_empty() {
            CreateProjectRequest {
                name: project.name.clone(),
                bounds: project.bounds,
                geometry: project.geometry.clone(),
                scenes: project.scenes.clone(),
            }
            .validate(fixture_origin)?;
        }
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
            stac_items: Vec::new(),
            wcs_items: Vec::new(),
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
        let mut changed = false;
        for scene in request.scenes {
            // A catalogue refresh must not replace an already pinned source or metadata.
            if ids.insert(scene.item_id.clone()) {
                updated.scenes.push(scene);
                changed = true;
            } else if crate::providers::modis::identity(&scene.item_id).is_some()
                || scene.assets.values().any(|a| {
                    url::Url::parse(&a.href)
                        .is_ok_and(|u| u.host_str() == Some(crate::providers::LANDSAT_HOST))
                })
            {
                // Newly supported quality layers may be added to a legacy RGB
                // scene; its prior assets and acquisition metadata stay pinned.
                if let Some(existing) = updated.scenes.iter_mut().find(|s| {
                    s.item_id == scene.item_id
                        && s.assets.values().any(|asset| {
                            url::Url::parse(&asset.href).is_ok_and(|url| {
                                matches!(
                                    url.host_str(),
                                    Some(crate::providers::modis::HOST)
                                        | Some(crate::providers::LANDSAT_HOST)
                                )
                            })
                        })
                }) {
                    for (key, asset) in scene.assets {
                        if (crate::providers::modis::QUALITY_KEYS.contains(&key.as_str())
                            || crate::raster::landsat_quality::KEYS.contains(&key.as_str()))
                            && !existing.assets.contains_key(&key)
                        {
                            existing.assets.insert(key, asset);
                            changed = true;
                        }
                    }
                }
            }
        }
        crate::stac_projects::validate_project(&self.inner.root, &updated)?;
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
        if !changed {
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

    pub(crate) async fn persist_projects(
        &self,
        projects: &BTreeMap<String, Project>,
    ) -> Result<()> {
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
        if !crate::providers::SOURCE_ASSET_KEYS.contains(&asset_key) {
            return Err("Choose supported source files for the project download".into());
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
                    && (asset_key != "viirs"
                        || job.status != JobStatus::Succeeded
                        || job.viirs_science.is_some())
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
                        raster_band: None,
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
    async fn landsat_conversion_metadata_survives_restart_and_rejects_missing_or_changed_parameters(
    ) {
        let directory = tempfile::tempdir().unwrap();
        let manager = JobManager::open(directory.path()).await.unwrap();
        let mut request = project_request();
        let product = "LC09_L2SP_044034_20250628_20250629_02_T1";
        request.scenes[0].item_id = "LC09_L2SP_044034_20250628_02_T1".into();
        request.scenes[0].assets = [("red",4), ("green",3), ("blue",2)].into_iter().map(|(key, number)| (key.into(), ProjectAsset {
            href: format!("https://{}/landsat-c2/level-2/standard/oli-tirs/2025/044/034/{product}/{product}_SR_B{number}.TIF", crate::providers::LANDSAT_HOST),
            media_type: "image/tiff; application=geotiff; profile=cloud-optimized".into(),
            raster_band: Some(ReflectanceBand { data_type: "uint16".into(), scale: 0.0000275, offset: -0.2, nodata: 0.0, spatial_resolution: 30.0 }),
        })).collect();
        let project = manager.create_project(request.clone()).await.unwrap();
        drop(manager);
        let loaded = load_projects(directory.path(), None).await.unwrap();
        assert_eq!(
            serde_json::to_value(&loaded[&project.id].scenes).unwrap(),
            serde_json::to_value(&project.scenes).unwrap()
        );
        let mut missing = request.clone();
        missing.scenes[0].assets.get_mut("red").unwrap().raster_band = None;
        assert!(missing.validate(None).is_err());
        request.scenes[0]
            .assets
            .get_mut("red")
            .unwrap()
            .raster_band
            .as_mut()
            .unwrap()
            .scale = 1.0;
        assert!(request.validate(None).is_err());
        let legacy = project_request();
        let json = serde_json::to_string(&legacy).unwrap();
        assert!(!json.contains("rasterBand"));
        assert!(serde_json::from_str::<CreateProjectRequest>(&json)
            .unwrap()
            .validate(None)
            .is_ok());
    }

    #[tokio::test]
    async fn landsat_quality_extension_keeps_old_rgb_and_rejects_another_processing_directory() {
        let directory = tempfile::tempdir().unwrap();
        let manager = JobManager::open(directory.path()).await.unwrap();
        let mut request = project_request();
        let product = "LC09_L2SP_044034_20250628_20250629_02_T1";
        request.scenes[0].item_id = "LC09_L2SP_044034_20250628_02_T1".into();
        request.scenes[0].assets=[("red",4),("green",3),("blue",2)].into_iter().map(|(key,b)| (key.into(),ProjectAsset {
            href:format!("https://{}/landsat-c2/level-2/standard/oli-tirs/2025/044/034/{product}/{product}_SR_B{b}.TIF",crate::providers::LANDSAT_HOST),
            media_type:"image/tiff; application=geotiff".into(),raster_band:Some(ReflectanceBand {data_type:"uint16".into(),scale:0.0000275,offset:-0.2,nodata:0.0,spatial_resolution:30.0}) })).collect();
        let original = manager.create_project(request.clone()).await.unwrap();
        let mut incoming = request.scenes.clone();
        for key in crate::raster::landsat_quality::KEYS {
            let href = incoming[0].assets["red"]
                .href
                .replace("_SR_B4.TIF", &format!("_{}.TIF", key.to_uppercase()));
            incoming[0].assets.insert(
                (*key).into(),
                ProjectAsset {
                    href,
                    media_type: "image/tiff; application=geotiff".into(),
                    raster_band: None,
                },
            );
        }
        let mut wrong = incoming.clone();
        wrong[0].assets.get_mut("qa_pixel").unwrap().href = wrong[0].assets["qa_pixel"]
            .href
            .replace("_20250629_", "_20250630_");
        assert!(manager
            .add_project_scenes(&original.id, AddProjectScenesRequest { scenes: wrong })
            .await
            .is_err());
        incoming[0].date = "2020-01-01T00:00:00Z".into();
        incoming[0].assets.get_mut("red").unwrap().href =
            "https://untrusted.example/changed.tif".into();
        let expanded = manager
            .add_project_scenes(&original.id, AddProjectScenesRequest { scenes: incoming })
            .await
            .unwrap();
        assert_eq!(expanded.scenes[0].date, original.scenes[0].date);
        for key in ["red", "green", "blue"] {
            assert_eq!(
                expanded.scenes[0].assets[key].href,
                original.scenes[0].assets[key].href
            );
        }
        assert_eq!(expanded.scenes[0].assets.len(), 5);
        drop(manager);
        let restored = load_projects(directory.path(), None).await.unwrap();
        assert_eq!(restored[&original.id].scenes[0].assets.len(), 5);
    }

    #[tokio::test]
    async fn hls_signed_band_calibration_survives_restart_and_cannot_use_landsat_parameters() {
        let directory = tempfile::tempdir().unwrap();
        let manager = JobManager::open(directory.path()).await.unwrap();
        let mut request = project_request();
        let id = "HLS.L30.T10SEG.2025179T184546.v2.0";
        request.scenes[0].item_id = id.into();
        request.scenes[0].assets = [("red", "B04"), ("green", "B03"), ("blue", "B02")]
            .into_iter()
            .map(|(key, band)| {
                (
                    key.into(),
                    ProjectAsset {
                        href: format!(
                            "https://{}/lp-prod-protected/HLSL30.020/{id}/{id}.{band}.tif",
                            crate::providers::nasa::HOST
                        ),
                        media_type: "image/tiff; application=geotiff".into(),
                        raster_band: Some(ReflectanceBand {
                            data_type: "int16".into(),
                            scale: 0.0001,
                            offset: 0.0,
                            nodata: -9999.0,
                            spatial_resolution: 30.0,
                        }),
                    },
                )
            })
            .collect();
        let project = manager.create_project(request.clone()).await.unwrap();
        drop(manager);
        let loaded = load_projects(directory.path(), None).await.unwrap();
        assert_eq!(
            serde_json::to_value(&loaded[&project.id].scenes).unwrap(),
            serde_json::to_value(&project.scenes).unwrap()
        );
        let band = request.scenes[0]
            .assets
            .get_mut("red")
            .unwrap()
            .raster_band
            .as_mut()
            .unwrap();
        *band = ReflectanceBand {
            data_type: "uint16".into(),
            scale: 0.0000275,
            offset: -0.2,
            nodata: 0.0,
            spatial_resolution: 30.0,
        };
        assert!(request.validate(None).is_err());
        request.scenes[0].assets.get_mut("red").unwrap().raster_band = None;
        assert!(request.validate(None).is_err());
    }

    #[tokio::test]
    async fn modis_five_source_project_and_legacy_quality_extension_keep_existing_pins() {
        let directory = tempfile::tempdir().unwrap();
        let manager = JobManager::open(directory.path()).await.unwrap();
        let mut request = project_request();
        let id = "MYD09A1.A2025177.h08v05.061.2025189031924";
        request.scenes[0].item_id = id.into();
        request.scenes[0].date = "2025-06-26T00:00:00Z".into();
        request.scenes[0].crs = Some(crate::providers::modis::CRS.into());
        request.scenes[0].cloud = None;
        request.scenes[0].assets = ["red", "green", "blue", "modis_qc", "modis_state"]
            .into_iter()
            .map(|key| {
                (
                    key.into(),
                    ProjectAsset {
                        href: format!(
                            "https://{}/modis-061-cogs/MYD09A1/08/05/2025177/{id}{}",
                            crate::providers::modis::HOST,
                            crate::providers::modis::suffix(key).unwrap()
                        ),
                        media_type: "image/tiff; application=geotiff".into(),
                        raster_band: ["red", "green", "blue"].contains(&key).then_some(
                            ReflectanceBand {
                                data_type: "int16".into(),
                                scale: 0.0001,
                                offset: 0.0,
                                nodata: -28672.0,
                                spatial_resolution: 500.0,
                            },
                        ),
                    },
                )
            })
            .collect();
        request.validate(None).unwrap();
        let full = manager.create_project(request.clone()).await.unwrap();
        assert_eq!(full.scenes[0].assets.len(), 5);
        let mut legacy = request.clone();
        legacy.scenes[0]
            .assets
            .retain(|key, _| !crate::providers::modis::QUALITY_KEYS.contains(&key.as_str()));
        let old = manager.create_project(legacy.clone()).await.unwrap();
        let mut incoming = request.scenes.clone();
        incoming[0].date = "2020-01-01T00:00:00Z".into();
        incoming[0].assets.get_mut("red").unwrap().href =
            "https://untrusted.example/changed.tif".into();
        let expanded = manager
            .add_project_scenes(
                &old.id,
                AddProjectScenesRequest {
                    scenes: incoming.clone(),
                },
            )
            .await
            .unwrap();
        assert_eq!(expanded.id, old.id);
        assert_eq!(expanded.created_at, old.created_at);
        assert_eq!(expanded.name, old.name);
        assert_eq!(expanded.bounds, old.bounds);
        assert_eq!(expanded.scenes[0].date, old.scenes[0].date);
        assert_eq!(expanded.scenes[0].assets.len(), 5);
        for (key, asset) in &old.scenes[0].assets {
            assert_eq!(
                serde_json::to_value(&expanded.scenes[0].assets[key]).unwrap(),
                serde_json::to_value(asset).unwrap()
            );
        }
        incoming[0].assets.get_mut("modis_qc").unwrap().href =
            "https://untrusted.example/changed.tif".into();
        assert_eq!(
            manager
                .add_project_scenes(
                    &old.id,
                    AddProjectScenesRequest {
                        scenes: incoming.clone()
                    }
                )
                .await
                .unwrap()
                .scenes[0]
                .assets["modis_qc"]
                .href,
            expanded.scenes[0].assets["modis_qc"].href
        );
        let untouched = manager.create_project(legacy).await.unwrap();
        assert!(manager
            .add_project_scenes(&untouched.id, AddProjectScenesRequest { scenes: incoming })
            .await
            .is_err());
        assert_eq!(
            manager
                .list_projects()
                .await
                .iter()
                .find(|p| p.id == untouched.id)
                .unwrap()
                .scenes[0]
                .assets
                .len(),
            3
        );
        request.scenes[0]
            .assets
            .get_mut("modis_qc")
            .unwrap()
            .raster_band = request.scenes[0].assets["red"].raster_band.clone();
        assert!(request.validate(None).is_err());
        drop(manager);
        let restored = JobManager::open(directory.path()).await.unwrap();
        let projects = restored.list_projects().await;
        for project in [full, expanded] {
            assert_eq!(
                serde_json::to_value(&projects.iter().find(|p| p.id == project.id).unwrap().scenes)
                    .unwrap(),
                serde_json::to_value(&project.scenes).unwrap()
            );
        }
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
