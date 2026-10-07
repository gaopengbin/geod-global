//! Reviewed project mutations and processing use the existing project/job stores.
use super::*;
use crate::projects::{CreateProjectRequest, Project, ProjectAsset, ProjectScene};
use std::collections::BTreeMap;

/// Effective project scope; runtime timestamps and approval history are excluded.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectScope {
    pub id: String,
    pub name: String,
    pub bounds: [f64; 4],
    pub geometry: Option<crate::crop::PolygonGeometry>,
    pub scenes: Vec<ProjectScene>,
    pub stac_items: Vec<crate::stac::ProjectItem>,
    pub wcs_items: Vec<crate::wcs::ProjectItem>,
}
impl ProjectScope {
    pub(crate) fn from_project(project: &Project) -> Self {
        Self {
            id: project.id.clone(),
            name: project.name.clone(),
            bounds: project.bounds,
            geometry: project.geometry.clone(),
            scenes: project.scenes.clone(),
            stac_items: project.stac_items.clone(),
            wcs_items: project.wcs_items.clone(),
        }
    }
    pub(crate) fn fingerprint(&self) -> Result<String> {
        let mut normalized = self.clone();
        normalized.scenes.sort_by(|a, b| a.item_id.cmp(&b.item_id));
        Ok(digest(&serde_json::to_vec(&normalized).map_err(io_error)?))
    }
    pub(crate) fn validate(&self) -> Result<()> {
        if !uuid(&self.id) {
            return Err("Invalid project scope identifier.".into());
        }
        self.request().validate(None)
    }
    fn request(&self) -> CreateProjectRequest {
        CreateProjectRequest {
            name: self.name.clone(),
            bounds: self.bounds,
            geometry: self.geometry.clone(),
            scenes: self.scenes.clone(),
        }
    }
    pub(crate) fn to_project(&self, timestamp: &str) -> Project {
        Project {
            id: self.id.clone(),
            name: self.name.clone(),
            bounds: self.bounds,
            geometry: self.geometry.clone(),
            scenes: self.scenes.clone(),
            stac_items: self.stac_items.clone(),
            wcs_items: self.wcs_items.clone(),
            created_at: timestamp.into(),
            updated_at: timestamp.into(),
            agent_approvals: Vec::new(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ProjectPin {
    pub id: String,
    pub name: String,
    pub hash: String,
}
impl ProjectPin {
    pub fn validate(&self) -> Result<()> {
        if !uuid(&self.id)
            || self.hash.len() != 64
            || !self.hash.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err("Invalid pinned project.".into());
        }
        crate::projects::validate_project_name(&self.name)?;
        Ok(())
    }
    pub(super) fn from_project(project: &Project) -> Result<Self> {
        Ok(Self {
            id: project.id.clone(),
            name: project.name.clone(),
            hash: ProjectScope::from_project(project).fingerprint()?,
        })
    }
    pub fn verify(&self, project: Option<&Project>) -> Result<()> {
        self.validate()?;
        let project = project.ok_or("The target project was removed. Create a new plan.")?;
        if ProjectScope::from_project(project).fingerprint()? != self.hash {
            return Err("The project changed. Create and review a new plan.".into());
        }
        Ok(())
    }
}
pub(super) fn created_project_id(plan: &Plan) -> String {
    let hash = Sha256::digest(format!("{}:{}:project", plan.policy, plan.id));
    let mut bytes: [u8; 16] = hash[..16].try_into().unwrap();
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes).to_string()
}
pub(super) fn receipt_exists(plan: &Plan, project: Option<&Project>) -> Result<bool> {
    if let Some(receipt) =
        project.and_then(|p| p.agent_approvals.iter().find(|a| a.plan_id == plan.id))
    {
        if receipt.plan_hash != plan.hash
            || receipt.session_id != plan.session_id
            || receipt.policy != plan.policy
        {
            return Err("Agent approval receipt conflicts with the project.".into());
        }
        return Ok(true);
    }
    Ok(false)
}
pub(super) fn view(plan: &Plan, projects: &BTreeMap<String, Project>) -> Result<Option<Value>> {
    let (id, name, scenes, mode) = match &plan.action {
        Action::Project {
            request, target, ..
        } => (
            target
                .as_ref()
                .map(|t| t.id.clone())
                .unwrap_or_else(|| created_project_id(plan)),
            request.name.clone(),
            request.scenes.len(),
            if target.is_some() { "append" } else { "create" },
        ),
        Action::Download {
            project: Some(pin), ..
        }
        | Action::Rgb {
            project: Some(pin), ..
        } => (
            pin.id.clone(),
            pin.name.clone(),
            projects.get(&pin.id).map_or(0, |p| p.scenes.len()),
            "existing",
        ),
        Action::Mosaic { project, .. } => (
            project.id.clone(),
            project.name.clone(),
            project.scenes.len(),
            "existing",
        ),
        _ => return Ok(None),
    };
    let committed =
        matches!(plan.action, Action::Project { .. }) && receipt_exists(plan, projects.get(&id))?;
    // Metadata persistence and confirmation of this review are separate facts.
    // An existing project must not make an unapproved download look submitted.
    let saved = projects.contains_key(&id);
    Ok(Some(
        json!({"id":id,"name":name,"sceneCount":scenes,"mode":mode,"committed":committed,"saved":saved}),
    ))
}

impl JobManager {
    pub async fn agent_project_plan(
        &self,
        session: &str,
        search_id: &str,
        item_ids: Vec<String>,
        name: Option<String>,
        target_id: Option<String>,
    ) -> Result<Value> {
        self.agent_project_polygon_plan(session, search_id, item_ids, name, target_id, None)
            .await
    }
    pub(super) async fn agent_project_polygon_plan(
        &self,
        session: &str,
        search_id: &str,
        mut item_ids: Vec<String>,
        name: Option<String>,
        target_id: Option<String>,
        geometry: Option<crate::crop::PolygonGeometry>,
    ) -> Result<Value> {
        if target_id.is_some() && geometry.is_some() {
            return Err("Appending scenes preserves the existing project polygon. Use a new project to change the area.".into());
        }
        if !uuid(session)
            || item_ids.is_empty()
            || item_ids.len() > crate::projects::MAX_PROJECT_SCENES
        {
            return Err("Choose 1..32 scene IDs from this conversation's search.".into());
        }
        item_ids.sort();
        item_ids.dedup();
        let receipt: SearchReceipt = read_record(&self.inner.root, "searches", search_id).await?;
        if receipt.session_id != session
            || DateTime::parse_from_rfc3339(&receipt.retrieved_at).map_or(true, |d| {
                Utc::now() - d.with_timezone(&Utc) > chrono::Duration::minutes(TTL_MINUTES)
            })
        {
            return Err(
                "Scene search is stale or belongs to another conversation. Search again.".into(),
            );
        }
        receipt.query.validate()?;
        let previous = if let Some(id) = target_id {
            if !uuid(&id) || name.is_some() {
                return Err("Choose a project ID to append, or a name for a new project.".into());
            }
            Some(
                self.inner
                    .projects
                    .lock()
                    .await
                    .get(&id)
                    .cloned()
                    .ok_or("Unknown project.")?,
            )
        } else {
            None
        };
        let mut scenes = previous
            .as_ref()
            .map(|p| p.scenes.clone())
            .unwrap_or_default();
        let before = scenes.len();
        for id in item_ids {
            let candidate = receipt
                .candidates
                .iter()
                .find(|c| c.item_id == id)
                .ok_or("Choose scene IDs returned by this native search.")?;
            let scene = ProjectScene {
                footprint: candidate.footprint.clone(),
                item_id: candidate.item_id.clone(),
                date: candidate.date.clone(),
                cloud: candidate.cloud,
                crs: candidate.crs.clone(),
                grid_code: None,
                bbox: candidate.bounds,
                assets: candidate
                    .assets
                    .iter()
                    .map(|a| {
                        (
                            a.asset_key.clone(),
                            ProjectAsset {
                                href: a.href.clone(),
                                media_type: a.media_type.clone(),
                                raster_band: candidate.bands.get(&a.asset_key).cloned(),
                            },
                        )
                    })
                    .collect(),
            };
            if let Some(existing) = scenes.iter().find(|s| s.item_id == id) {
                // Never replace a previously selected source with refreshed catalogue data.
                if serde_json::to_value(&existing.assets).map_err(io_error)?
                    != serde_json::to_value(&scene.assets).map_err(io_error)?
                {
                    return Err("A selected scene's pinned assets differ from the catalog. Keep the existing source or use a new project.".into());
                }
            } else {
                scenes.push(scene);
            }
        }
        if previous.is_some() && scenes.len() == before {
            return Err(
                "These scenes are already in the project. Use its download or processing plan."
                    .into(),
            );
        }
        scenes.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.item_id.cmp(&b.item_id)));
        let request = CreateProjectRequest {
            name: match &previous {
                Some(p) => p.name.clone(),
                None => crate::projects::validate_project_name(
                    name.as_deref().ok_or("Name the new project.")?,
                )?,
            },
            bounds: previous.as_ref().map_or(receipt.query.bounds, |p| p.bounds),
            geometry: previous
                .as_ref()
                .and_then(|p| p.geometry.clone())
                .or(geometry),
            scenes,
        };
        request.validate(None)?;
        footprint::complete(&footprint::project_scope(&request))?;
        let target = previous
            .as_ref()
            .map(ProjectPin::from_project)
            .transpose()?;
        self.save_agent_plan(
            session,
            Action::Project {
                request,
                target,
                metadata_sha256: receipt.document_sha256,
            },
        )
        .await
    }
    pub(super) async fn commit_project_plan(&self, plan: &Plan) -> Result<Value> {
        let Action::Project {
            request, target, ..
        } = &plan.action
        else {
            return Err("Invalid project plan.".into());
        };
        let id = target
            .as_ref()
            .map(|p| p.id.clone())
            .unwrap_or_else(|| created_project_id(plan));
        let mut projects = self.inner.projects.lock().await;
        let previous = projects.get(&id).cloned();
        if receipt_exists(plan, previous.as_ref())? {
            drop(projects);
            return self.agent_plan_status(&plan.session_id, &plan.id).await;
        }
        footprint::complete(&footprint::project_scope(request))?;
        if plan.expired() {
            return Err("Agent plan expired. Create a new plan before confirming.".into());
        }
        self.inner.store.lock().await.accepting_jobs()?;
        if let Some(pin) = target {
            pin.verify(previous.as_ref())?;
        } else if previous.is_some() {
            return Err("Project identifier conflicts with an existing project.".into());
        }
        let mut project = previous.clone().unwrap_or(Project {
            id: id.clone(),
            name: request.name.clone(),
            bounds: request.bounds,
            geometry: request.geometry.clone(),
            scenes: Vec::new(),
            stac_items: Vec::new(),
            wcs_items: Vec::new(),
            created_at: now(),
            updated_at: now(),
            agent_approvals: Vec::new(),
        });
        project.scenes = request.scenes.clone();
        project.updated_at = now();
        if project.agent_approvals.len() >= 500 {
            return Err("Project approval history limit reached. Use a new project.".into());
        }
        request.validate(None)?;
        crate::stac_projects::validate_project(&self.inner.root, &project)?;
        project
            .agent_approvals
            .push(approval(plan, &project.updated_at, None));
        projects.insert(id.clone(), project);
        if let Err(error) = self.persist_projects(&projects).await {
            if let Some(old) = previous {
                projects.insert(id, old);
            } else {
                projects.remove(&id);
            }
            return Err(error);
        }
        drop(projects);
        self.agent_plan_status(&plan.session_id, &plan.id).await
    }
    pub async fn agent_project_download_plan(
        &self,
        session: &str,
        project_id: &str,
        asset_key: &str,
        item_ids: Option<Vec<String>>,
    ) -> Result<Value> {
        if !uuid(session)
            || !uuid(project_id)
            || !crate::providers::SOURCE_ASSET_KEYS.contains(&asset_key)
        {
            return Err("Choose a saved project and a reviewed public source asset.".into());
        }
        let project = self
            .inner
            .projects
            .lock()
            .await
            .get(project_id)
            .cloned()
            .ok_or("Unknown project.")?;
        ProjectScope::from_project(&project).validate()?;
        let selected =
            item_ids.unwrap_or_else(|| project.scenes.iter().map(|s| s.item_id.clone()).collect());
        if selected.is_empty()
            || selected.len() > MAX_FILES
            || selected
                .iter()
                .any(|id| !project.scenes.iter().any(|s| s.item_id == *id))
        {
            return Err("Choose 1..32 scene IDs belonging to this project.".into());
        }
        let selected = selected.into_iter().collect::<BTreeSet<_>>();
        footprint::complete(&footprint::Scope {
            bounds: project.bounds,
            geometry: project.geometry.clone(),
            scenes: project
                .scenes
                .iter()
                .filter(|s| selected.contains(&s.item_id))
                .map(|s| (s.item_id.clone(), s.footprint.clone()))
                .collect(),
        })?;
        let providers = project
            .scenes
            .iter()
            .filter(|s| selected.contains(&s.item_id))
            .map(|s| {
                let a = s
                    .assets
                    .get(asset_key)
                    .ok_or("A selected scene has no requested asset.")?;
                catalog::provider(&crate::CreateJobRequest {
                    item_id: s.item_id.clone(),
                    asset_key: asset_key.into(),
                    href: a.href.clone(),
                    media_type: a.media_type.clone(),
                    title: None,
                })
            })
            .collect::<Result<BTreeSet<_>>>()?;
        if providers.len() != 1 {
            return Err("Select scenes from one provider for each download plan.".into());
        }
        let provider = *providers.first().unwrap();
        let mut requests = Vec::new();
        let mut reused = Vec::new();
        let protected_checks = if protected::account(provider).is_some() {
            let files = project
                .scenes
                .iter()
                .filter(|s| selected.contains(&s.item_id))
                .map(|s| {
                    let a = s
                        .assets
                        .get(asset_key)
                        .ok_or("A selected scene has no requested asset.")?;
                    Ok(DownloadFile {
                        request: crate::CreateJobRequest {
                            item_id: s.item_id.clone(),
                            asset_key: asset_key.into(),
                            href: a.href.clone(),
                            media_type: a.media_type.clone(),
                            title: None,
                        },
                        date: s.date.clone(),
                        pin: None,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            protected::verified_candidates(self, &files).await?
        } else {
            Vec::new()
        };
        {
            let store = self.inner.store.lock().await;
            for scene in project
                .scenes
                .iter()
                .filter(|s| selected.contains(&s.item_id))
            {
                let asset = scene
                    .assets
                    .get(asset_key)
                    .ok_or("A selected scene has no requested asset.")?;
                let request = crate::CreateJobRequest {
                    item_id: scene.item_id.clone(),
                    asset_key: asset_key.into(),
                    href: asset.href.clone(),
                    media_type: asset.media_type.clone(),
                    title: Some(format!("{} · {}", scene.item_id, asset_key.to_uppercase())),
                };
                crate::validate_request(&request, None)?;
                catalog::provider(&request)?;
                if let Some(job) = store.jobs.values().find(|j| {
                    j.item_id == scene.item_id
                        && j.asset_key == asset_key
                        && j.href == asset.href
                        && matches!(
                            j.status,
                            JobStatus::Succeeded | JobStatus::Queued | JobStatus::Running
                        )
                        && (j.status != JobStatus::Succeeded
                            || protected::account(provider).is_none()
                            || !store.active.contains_key(&j.id)
                                && protected_checks.iter().any(|(checked, result)| {
                                    protected::same_receipt(checked, j) && result.is_ok()
                                }))
                }) {
                    reused.push(job_view(
                        job,
                        !crate::active(&job.status) && !store.active.contains_key(&job.id),
                    ));
                } else {
                    requests.push((request, scene.date.clone()));
                }
            }
        }
        if requests.is_empty() {
            return Ok(
                json!({"project":{"id":project.id,"name":project.name},"jobs":reused,"needsDownload":false,"note":"All requested files already have native tasks. Read their status or prepare a processing plan."}),
            );
        }
        requests.sort_by(|a, b| a.0.item_id.cmp(&b.0.item_id));
        let start = requests
            .iter()
            .map(|(_, date)| date[..10].to_string())
            .min()
            .unwrap();
        let end = requests
            .iter()
            .map(|(_, date)| date[..10].to_string())
            .max()
            .unwrap();
        let query = SearchQuery {
            provider: provider.into(),
            bounds: project.bounds,
            start,
            end,
            cloud_max: 100.0,
            limit: 20,
        };
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
                    bounds: project.bounds,
                    geometry: project.geometry.clone(),
                    scenes: project
                        .scenes
                        .iter()
                        .filter(|s| selected.contains(&s.item_id))
                        .map(|s| (s.item_id.clone(), s.footprint.clone()))
                        .collect(),
                }),
                query,
                metadata_sha256: ProjectScope::from_project(&project).fingerprint()?,
                files,
                project: Some(ProjectPin::from_project(&project)?),
            },
        )
        .await
    }
    pub async fn agent_project_mosaic_plan(
        &self,
        session: &str,
        project_id: &str,
        asset_key: &str,
    ) -> Result<Value> {
        self.agent_project_mosaic_plan_with_selection(session, project_id, asset_key, None)
            .await
    }
    pub async fn agent_project_mosaic_plan_with_selection(
        &self,
        session: &str,
        project_id: &str,
        asset_key: &str,
        selection: Option<crate::mosaic::vegetation::Request>,
    ) -> Result<Value> {
        let (project, job) = self
            .mosaic_review_job_with_selection(project_id, asset_key, selection)
            .await?;
        let output = self.mosaic_preflight(&project, &job).await?;
        let scope = ProjectScope::from_project(&project);
        scope.validate()?;
        self.save_agent_plan(
            session,
            Action::Mosaic {
                project_hash: scope.fingerprint()?,
                project: scope,
                spec: job.mosaic.unwrap(),
                output,
            },
        )
        .await
    }
}
