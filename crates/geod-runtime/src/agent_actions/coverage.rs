//! Reviewed WCS subsets reuse archived grid plans and the native WCS queue.
use super::*;
use crate::wcs::{self, SourcePin};
use crate::wcs_projects::{DownloadRequest, SaveProjectRequest};

fn selections(pins: &[SourcePin]) -> Result<()> {
    let mut seen = BTreeSet::new();
    if pins.is_empty() || pins.len() > MAX_FILES {
        return Err("Choose 1..32 saved WCS requests.".into());
    }
    for pin in pins {
        if pin.plan_id.len() != 64
            || !pin
                .plan_id
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            || !seen.insert(&pin.plan_id)
        {
            return Err("Choose distinct saved WCS requests.".into());
        }
    }
    Ok(())
}

pub(super) fn validate(action: &Action) -> Result<()> {
    let (pins, bounds) = match action {
        Action::WcsProject { request, target } => {
            if request.project_id.as_ref() != target.as_ref().map(|p| &p.id) {
                return Err("Invalid WCS project identity.".into());
            }
            if let Some(pin) = target {
                pin.validate()?;
                if request.name.is_some() {
                    return Err("Rename existing projects separately.".into());
                }
            } else {
                crate::projects::validate_project_name(
                    request.name.as_deref().ok_or("Name the project.")?,
                )?;
            }
            (&request.selections, request.bounds)
        }
        Action::WcsDownload {
            project,
            bounds,
            selections,
        } => {
            project.validate()?;
            (selections, *bounds)
        }
        _ => return Err("Invalid coverage review action.".into()),
    };
    if !valid_bounds(bounds) {
        return Err("Choose an increasing WGS84 project area.".into());
    }
    selections(pins)
}

// A reuse decision verifies original bytes, never infers completion from a status.
async fn reusable(manager: &JobManager, pin: &SourcePin) -> Result<Option<Value>> {
    let candidates = manager
        .inner
        .store
        .lock()
        .await
        .jobs
        .values()
        .filter(|j| {
            j.wcs_source.as_ref() == Some(pin)
                && matches!(
                    j.status,
                    JobStatus::Queued | JobStatus::Running | JobStatus::Succeeded
                )
        })
        .cloned()
        .collect::<Vec<_>>();
    for candidate in candidates {
        wcs::validate_job(&manager.inner.root, &candidate)?;
        let verified = if candidate.status == JobStatus::Succeeded {
            let root = manager.inner.root.clone();
            let copy = candidate.clone();
            tokio::task::spawn_blocking(move || {
                crate::stac::raster::verified_original(&root, &copy)
            })
            .await
            .map_err(io_error)?
            .is_ok()
        } else {
            true
        };
        let store = manager.inner.store.lock().await;
        if let Some(current) = store.jobs.get(&candidate.id) {
            if serde_json::to_value(current).map_err(io_error)?
                == serde_json::to_value(&candidate).map_err(io_error)?
                && verified
            {
                return Ok(Some(job_view(
                    current,
                    !crate::active(&current.status) && !store.active.contains_key(&current.id),
                )));
            }
        }
    }
    Ok(None)
}

impl JobManager {
    pub async fn agent_wcs_project_plan(
        &self,
        session: &str,
        mut request: SaveProjectRequest,
    ) -> Result<Value> {
        if !uuid(session) {
            return Err("Invalid Agent conversation ID.".into());
        }
        selections(&request.selections)?;
        if !valid_bounds(request.bounds) {
            return Err("Choose an increasing WGS84 project area.".into());
        }
        let previous = if let Some(id) = &request.project_id {
            if !uuid(id) || request.name.is_some() {
                return Err("Choose a saved project without renaming it.".into());
            }
            Some(
                self.inner
                    .projects
                    .lock()
                    .await
                    .get(id)
                    .cloned()
                    .ok_or("Unknown project")?,
            )
        } else {
            None
        };
        if let Some(p) = &previous {
            request.bounds = p.bounds;
        }
        let mut additions = Vec::new();
        for pin in request.selections {
            let plan = self.wcs_plan(&pin.plan_id).await?;
            if plan.requested_bounds != request.bounds {
                return Err(
                    "Prepare coverage requests for the saved project area before reviewing.".into(),
                );
            }
            if additions
                .iter()
                .map(|p| wcs::same_selection(&self.inner.root, p, &pin))
                .collect::<Result<Vec<_>>>()?
                .contains(&true)
            {
                return Err("Choose distinct saved WCS requests.".into());
            }
            let already = previous
                .as_ref()
                .map(|p| {
                    p.wcs_items
                        .iter()
                        .map(|i| {
                            wcs::same_selection(
                                &self.inner.root,
                                &SourcePin {
                                    plan_id: i.plan_id.clone(),
                                },
                                &pin,
                            )
                        })
                        .collect::<Result<Vec<_>>>()
                })
                .transpose()?
                .is_some_and(|v| v.contains(&true));
            if !already {
                additions.push(pin);
            }
        }
        request.selections = additions;
        selections(&request.selections)?;
        let mut candidate = if let Some(previous) = previous.clone() {
            previous
        } else {
            crate::Project {
                id: Uuid::new_v4().to_string(),
                name: crate::projects::validate_project_name(
                    request.name.as_deref().unwrap_or(""),
                )?,
                bounds: request.bounds,
                geometry: None,
                scenes: Vec::new(),
                stac_items: Vec::new(),
                wcs_items: Vec::new(),
                created_at: now(),
                updated_at: now(),
                agent_approvals: Vec::new(),
            }
        };
        for pin in &request.selections {
            candidate
                .wcs_items
                .push(wcs::resolve(&self.inner.root, pin)?);
        }
        crate::stac_projects::validate_project(&self.inner.root, &candidate)?;
        let target = previous
            .as_ref()
            .map(ProjectPin::from_project)
            .transpose()?;
        self.save_agent_plan(session, Action::WcsProject { request, target })
            .await
    }

    pub async fn agent_wcs_download_plan(
        &self,
        session: &str,
        request: DownloadRequest,
    ) -> Result<Value> {
        if !uuid(session) || !uuid(&request.project_id) {
            return Err("Choose a saved WCS project.".into());
        }
        let project = self
            .inner
            .projects
            .lock()
            .await
            .get(&request.project_id)
            .cloned()
            .ok_or("Unknown project")?;
        crate::stac_projects::validate_project(&self.inner.root, &project)?;
        let requested = request.selections.unwrap_or_else(|| {
            project
                .wcs_items
                .iter()
                .map(|i| SourcePin {
                    plan_id: i.plan_id.clone(),
                })
                .collect()
        });
        selections(&requested)?;
        let mut pins = Vec::new();
        for requested in requested {
            let mut saved = None;
            for item in &project.wcs_items {
                let pin = SourcePin {
                    plan_id: item.plan_id.clone(),
                };
                if wcs::same_selection(&self.inner.root, &pin, &requested)? {
                    saved = Some(pin);
                    break;
                }
            }
            pins.push(saved.ok_or("Choose coverage requests belonging to this project")?);
        }
        selections(&pins)?;
        let mut missing = Vec::new();
        let mut reused = Vec::new();
        for pin in pins {
            if let Some(job) = reusable(self, &pin).await? {
                reused.push(job);
            } else {
                missing.push(pin);
            }
        }
        let pin = ProjectPin::from_project(&project)?;
        pin.verify(self.inner.projects.lock().await.get(&pin.id))?;
        if missing.is_empty() {
            return Ok(
                json!({"project":{"id":project.id,"name":project.name},"jobs":reused,"needsDownload":false,"note":"All selected coverage requests already have native tasks. Read their actual status."}),
            );
        }
        self.save_agent_plan(
            session,
            Action::WcsDownload {
                project: pin,
                bounds: project.bounds,
                selections: missing,
            },
        )
        .await
    }
}

pub(super) async fn status(manager: &JobManager, plan: &Plan) -> Result<Value> {
    let projects = manager.inner.projects.lock().await;
    let (kind, id, name, bounds, mode, pins, committed) = match &plan.action {
        Action::WcsProject { request, target } => {
            let id = target
                .as_ref()
                .map(|p| p.id.clone())
                .unwrap_or_else(|| project::created_project_id(plan));
            let committed = project::receipt_exists(plan, projects.get(&id))?;
            (
                "project",
                id,
                request
                    .name
                    .clone()
                    .unwrap_or_else(|| target.as_ref().unwrap().name.clone()),
                request.bounds,
                if target.is_some() { "append" } else { "create" },
                &request.selections,
                committed,
            )
        }
        Action::WcsDownload {
            project,
            bounds,
            selections,
        } => (
            "download",
            project.id.clone(),
            project.name.clone(),
            *bounds,
            "existing",
            selections,
            false,
        ),
        _ => return Err("Invalid coverage review action.".into()),
    };
    let store = manager.inner.store.lock().await;
    let saved = projects.contains_key(&id);
    let jobs = existing(plan, &store.jobs)?;
    // Verify the queue receipt's source identity as well as its review identity.
    if let Some(jobs) = &jobs {
        for (job, pin) in jobs.iter().zip(pins) {
            if job.wcs_source.as_ref() != Some(pin) {
                return Err("Coverage approval source differs from its review.".into());
            }
            wcs::validate_job(&manager.inner.root, job)?;
        }
    }
    let replacement = revision::replacement(&manager.inner.root, plan).await?;
    let state = if committed || jobs.is_some() {
        "submitted"
    } else if replacement.is_some() {
        "superseded"
    } else if plan.expired() {
        "expired"
    } else {
        "pending"
    };
    let files=pins.iter().map(|pin|{
        let value=wcs::plan(&manager.inner.root,&pin.plan_id)?;
        Ok::<_,String>(json!({"itemId":value.description.coverage_id,"assetKey":if kind=="project"{"scene"}else{"wcs_coverage"},"referenceId":pin.plan_id,"coveragePlanId":pin.plan_id,"bytes":null,"width":value.width,"height":value.height,"crs":value.description.crs,"requestedBounds":value.requested_bounds,"alignedBounds":value.bounds,"selection":value.selection}))
    }).collect::<Result<Vec<_>>>()?;
    Ok(
        json!({"planId":plan.id,"planHash":plan.hash,"kind":kind,"status":state,"source":"Selected WCS coverage subsets","bounds":bounds,"boundsCrs":"EPSG:4326","polygon":null,"start":null,"end":null,"files":files,"replacedBy":replacement.map(|r|r.plan_id),"expectedBytes":null,"format":if kind=="project"{"Project"}else{"GeoTIFF"},"output":"Managed workspace files","expiresAt":plan.expires_at,"approvalRequired":true,"project":{"id":id,"name":name,"sceneCount":pins.len(),"mode":mode,"committed":committed,"saved":saved},"processing":null,"notes":[if kind=="project"{"Saves native coverage requests; no files are downloaded."}else{"Service-generated subset on the declared native grid; not an original scene. Encoded size is unknown before download; the native 512 MiB limit applies. Grid edges align outward and intersect the coverage; no resampling or polygon clipping."}],"jobs":jobs.unwrap_or_default().iter().map(|j|job_view(j,!crate::active(&j.status)&&!store.active.contains_key(&j.id))).collect::<Vec<_>>() }),
    )
}

pub(super) async fn commit(manager: &JobManager, plan: &Plan) -> Result<Value> {
    match &plan.action {
        Action::WcsProject { request, target } => {
            let id = target
                .as_ref()
                .map(|p| p.id.clone())
                .unwrap_or_else(|| project::created_project_id(plan));
            let mut projects = manager.inner.projects.lock().await;
            let previous = projects.get(&id).cloned();
            if project::receipt_exists(plan, previous.as_ref())? {
                drop(projects);
                return status(manager, plan).await;
            }
            if plan.expired() {
                return Err("Agent plan expired. Create a new plan before confirming.".into());
            }
            manager.inner.store.lock().await.accepting_jobs()?;
            if let Some(pin) = target {
                pin.verify(previous.as_ref())?;
            } else if previous.is_some() {
                return Err("Project identifier conflicts with an existing project.".into());
            }
            let mut current = previous.clone().unwrap_or_else(|| crate::Project {
                id: id.clone(),
                name: request.name.clone().unwrap(),
                bounds: request.bounds,
                geometry: None,
                scenes: Vec::new(),
                stac_items: Vec::new(),
                wcs_items: Vec::new(),
                created_at: now(),
                updated_at: now(),
                agent_approvals: Vec::new(),
            });
            for pin in &request.selections {
                current
                    .wcs_items
                    .push(wcs::resolve(&manager.inner.root, pin)?);
            }
            crate::stac_projects::validate_project(&manager.inner.root, &current)?;
            if current.agent_approvals.len() >= 500 {
                return Err("Project approval history limit reached. Use a new project.".into());
            }
            current.updated_at = now();
            current
                .agent_approvals
                .push(approval(plan, &current.updated_at, None));
            projects.insert(id.clone(), current);
            if let Err(error) = manager.persist_projects(&projects).await {
                if let Some(old) = previous {
                    projects.insert(id, old);
                } else {
                    projects.remove(&id);
                }
                return Err(error);
            }
            drop(projects);
        }
        Action::WcsDownload {
            project,
            selections,
            ..
        } => {
            if existing(plan, &manager.inner.store.lock().await.jobs)?.is_some() {
                return status(manager, plan).await;
            }
            if plan.expired() {
                return Err("Agent plan expired. Create a new plan before confirming.".into());
            }
            let receipt = approval(plan, &now(), None);
            manager
                .download_wcs_project_reviewed(
                    DownloadRequest {
                        project_id: project.id.clone(),
                        selections: Some(selections.clone()),
                    },
                    crate::wcs_projects::ReviewedDownload {
                        project_hash: project.hash.clone(),
                        expires_at: plan.expires_at.clone(),
                        jobs: plan
                            .job_ids()
                            .into_iter()
                            .map(|id| (id, receipt.clone()))
                            .collect(),
                    },
                )
                .await?;
        }
        _ => return Err("Invalid coverage review action.".into()),
    }
    status(manager, plan).await
}

pub(super) async fn revision_draft(manager: &JobManager, plan: &Plan) -> Result<Value> {
    let (kind, pins, name, bounds, project_id, polygon) = match &plan.action {
        Action::WcsProject { request, target } => {
            let projects = manager.inner.projects.lock().await;
            let polygon = if let Some(pin) = target {
                pin.verify(projects.get(&pin.id))?;
                projects[&pin.id].geometry.is_some()
            } else {
                false
            };
            (
                "project",
                request.selections.clone(),
                request.name.clone(),
                target.is_none().then_some(request.bounds),
                target.as_ref().map(|p| p.id.clone()),
                polygon,
            )
        }
        Action::WcsDownload {
            project,
            selections,
            ..
        } => {
            project.verify(manager.inner.projects.lock().await.get(&project.id))?;
            (
                "download",
                selections.clone(),
                None,
                None,
                Some(project.id.clone()),
                false,
            )
        }
        _ => return Err("Invalid coverage review action.".into()),
    };
    let items = pins
        .iter()
        .map(|pin| {
            let item = wcs::resolve(&manager.inner.root, pin)?;
            Ok::<_, String>(json!({"id":pin.plan_id,"label":item.title,"date":null,"locked":false}))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut parameters =
        json!({"kind":kind,"itemIds":pins.iter().map(|p|&p.plan_id).collect::<Vec<_>>()});
    let mut fields = json!({"items":items});
    if kind == "project" {
        parameters["name"] = json!(name);
        parameters["bounds"] = json!(bounds);
        parameters["keepPolygon"] = json!(polygon);
        fields["name"] = json!(project_id.is_none());
        fields["bounds"] = json!(project_id.is_none());
        fields["polygon"] = json!(false);
    }
    Ok(
        json!({"planId":plan.id,"planHash":plan.hash,"kind":kind,"parameters":parameters,"fields":fields,"projectId":project_id,"boundsCrs":"EPSG:4326"}),
    )
}

fn selected(ids: Vec<String>, pins: &[SourcePin]) -> Result<Vec<SourcePin>> {
    let count = ids.len();
    let ids = ids.into_iter().collect::<BTreeSet<_>>();
    if ids.is_empty()
        || ids.len() != count
        || ids.iter().any(|id| !pins.iter().any(|p| p.plan_id == *id))
    {
        return Err("Select distinct coverage requests from this native review.".into());
    }
    Ok(pins
        .iter()
        .filter(|p| ids.contains(&p.plan_id))
        .cloned()
        .collect())
}
pub(super) async fn revise(
    manager: &JobManager,
    session: &str,
    action: Action,
    parameters: PlanRevision,
) -> Result<Value> {
    match (action, parameters) {
        (
            Action::WcsProject {
                mut request,
                target,
            },
            PlanRevision::Project {
                item_ids,
                name,
                bounds,
                keep_polygon,
            },
        ) => {
            request.selections = selected(item_ids, &request.selections)?;
            if let Some(pin) = target {
                let projects = manager.inner.projects.lock().await;
                pin.verify(projects.get(&pin.id))?;
                if name.is_some()
                    || bounds.is_some()
                    || keep_polygon != projects[&pin.id].geometry.is_some()
                {
                    return Err(
                        "Keep the saved project area; select coverage requests to append.".into(),
                    );
                }
            } else {
                if keep_polygon {
                    return Err("WCS subsets do not apply a polygon mask.".into());
                }
                request.name = Some(name.ok_or("Name the project.")?);
                request.bounds = bounds.ok_or("Set the project area.")?;
                for pin in &mut request.selections {
                    let saved = manager.wcs_plan(&pin.plan_id).await?;
                    pin.plan_id = manager
                        .plan_wcs(wcs::PlanRequest {
                            description_id: saved.description.id,
                            bounds: request.bounds,
                        })
                        .await?
                        .id;
                }
            }
            manager.agent_wcs_project_plan(session, request).await
        }
        (
            Action::WcsDownload {
                project,
                selections,
                ..
            },
            PlanRevision::Download { item_ids },
        ) => {
            project.verify(manager.inner.projects.lock().await.get(&project.id))?;
            let next = manager
                .agent_wcs_download_plan(
                    session,
                    DownloadRequest {
                        project_id: project.id,
                        selections: Some(selected(item_ids, &selections)?),
                    },
                )
                .await?;
            if next.get("planId").is_none() {
                return Err(
                    "A selected coverage already has a reusable task. Prepare a new download plan."
                        .into(),
                );
            }
            Ok(next)
        }
        _ => Err("The form belongs to a different review kind.".into()),
    }
}

pub(super) fn definitions() -> Vec<Value> {
    let pins = json!({"type":"array","items":{"type":"object","properties":{"planId":{"type":"string","pattern":"^[a-f0-9]{64}$"}},"required":["planId"],"additionalProperties":false},"minItems":1,"maxItems":32});
    vec![
        json!({"name":"geod_wcs_project_plan","description":"Prepare a project review from saved native WCS grid plans. Read saved connections/coverages, describe the actual dataset, then geod_wcs_prepare for the explicit user WGS84 bounds first. Supply name for a new project or projectId to append. For append prepare subsets for the saved project area. Native confirmation saves project metadata only; no download. Preserves all existing source selections. No URL, path, credentials or polygon clipping.","inputSchema":{"type":"object","properties":{"name":{"type":"string","maxLength":120},"projectId":{"type":"string","format":"uuid"},"bounds":{"type":"array","items":{"type":"number"},"minItems":4,"maxItems":4},"selections":pins},"required":["bounds","selections"],"additionalProperties":false}}),
        json!({"name":"geod_wcs_download_plan","description":"Prepare download review for a saved project WCS subset, optionally select native planIds. Completed reuse verifies complete bytes/SHA-256. Native confirmation returns real queued task IDs immediately; wait for actual succeeded AND settled. Server-generated native-grid subsets, not original scenes. Rechecks remote coverage declarations at transfer; unknown encoded size, native 512 MiB cap. No resampling or polygon mask.","inputSchema":{"type":"object","properties":{"projectId":{"type":"string","format":"uuid"},"selections":pins},"required":["projectId"],"additionalProperties":false}}),
    ]
}

#[cfg(test)]
mod tests;
