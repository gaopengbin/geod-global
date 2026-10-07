//! Custom public assets use archived native selections and the same review ledger.
//! Model tools prepare plans only; native confirmation is the sole commit path.
use super::*;
use crate::stac::{self, Selection};
use crate::stac_projects::{DownloadRequest, SaveProjectRequest};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct FilePin {
    pub selection: Selection,
    pub item: stac::ProjectItem,
    pub remote: RemotePin,
}

pub(super) fn selection_id(pin: &Selection) -> String {
    digest(&serde_json::to_vec(pin).expect("selection serialization"))
}

fn selections(pins: &[Selection]) -> Result<()> {
    let mut seen = BTreeSet::new();
    if pins.is_empty() || pins.len() > MAX_FILES {
        return Err("Choose 1..32 archived custom raster assets.".into());
    }
    for pin in pins {
        if pin.snapshot_id.len() != 64
            || !pin
                .snapshot_id
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            || pin.asset_key.trim().is_empty()
            || pin.asset_key.len() > 512
            || pin.asset_key.chars().any(char::is_control)
            || !seen.insert(selection_id(pin))
        {
            return Err("Choose unique archived custom raster assets.".into());
        }
    }
    Ok(())
}

pub(super) fn validate(action: &Action) -> Result<()> {
    match action {
        Action::StacProject { request, target } => {
            selections(&request.selections)?;
            if !valid_bounds(request.bounds)
                || request.project_id.as_ref() != target.as_ref().map(|p| &p.id)
            {
                return Err("Invalid custom project review.".into());
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
        }
        Action::StacDownload {
            project,
            bounds,
            files,
        } => {
            project.validate()?;
            if !valid_bounds(*bounds) {
                return Err("Invalid custom project area.".into());
            }
            selections(
                &files
                    .iter()
                    .map(|f| f.selection.clone())
                    .collect::<Vec<_>>(),
            )?;
            for file in files {
                if file.selection.snapshot_id != file.item.snapshot_id
                    || file.selection.asset_key != file.item.asset_key
                {
                    return Err("Custom asset identity changed.".into());
                }
                remote_valid(&file.remote)?;
            }
        }
        _ => return Err("Invalid custom review action.".into()),
    }
    Ok(())
}

fn remote_valid(pin: &RemotePin) -> Result<()> {
    if pin.bytes == 0
        || pin.bytes > crate::MAX_ASSET_BYTES
        || !crate::transfer::strong_etag(&pin.etag)
    {
        return Err(
            "Source requires a stable ETag and a file within its native product size limit.".into(),
        );
    }
    Ok(())
}

async fn head(manager: &JobManager, selection: &Selection) -> Result<RemotePin> {
    let response = stac::asset_head(
        &manager.proxy_settings().await,
        &manager.inner.root,
        selection,
    )
    .await?;
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
            .ok_or("Source ETag is unavailable.")?
            .into(),
    };
    remote_valid(&pin)?;
    Ok(pin)
}

fn job(item: &stac::ProjectItem, selection: &Selection) -> Job {
    let mut result = crate::new_download_job(crate::CreateJobRequest {
        item_id: item.item_id.clone(),
        asset_key: "stac_asset".into(),
        href: item.href.clone(),
        media_type: item.media_type.clone(),
        title: Some(item.title.clone()),
    });
    result.source = item.service_name.clone();
    result.stac_source = Some(selection.clone());
    result
}

// Check original bytes outside the store lock; recheck the effective record after
// awaiting. This is a reuse decision, never proof that a queued file is complete.
async fn reusable(manager: &JobManager, pin: &Selection) -> Result<Option<Value>> {
    let candidates = manager
        .inner
        .store
        .lock()
        .await
        .jobs
        .values()
        .filter(|j| {
            j.stac_source.as_ref() == Some(pin)
                && matches!(
                    j.status,
                    JobStatus::Queued | JobStatus::Running | JobStatus::Succeeded
                )
        })
        .cloned()
        .collect::<Vec<_>>();
    for candidate in candidates {
        stac::validate_job(&manager.inner.root, &candidate)?;
        let verified = if candidate.status == JobStatus::Succeeded {
            let root = manager.inner.root.clone();
            let copy = candidate.clone();
            Some(
                tokio::task::spawn_blocking(move || stac::raster::verified_original(&root, &copy))
                    .await
                    .map_err(io_error)?,
            )
        } else {
            None
        };
        let store = manager.inner.store.lock().await;
        if let Some(current) = store.jobs.get(&candidate.id) {
            if current.stac_source == candidate.stac_source
                && current.status == candidate.status
                && current.sha256 == candidate.sha256
                && current.output_path == candidate.output_path
                && current.bytes_downloaded == candidate.bytes_downloaded
                && current.total_bytes == candidate.total_bytes
                && current.updated_at == candidate.updated_at
                && (crate::active(&current.status) || verified.as_ref().is_some_and(|v| v.is_ok()))
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
    pub async fn agent_stac_project_plan(
        &self,
        session: &str,
        mut request: SaveProjectRequest,
    ) -> Result<Value> {
        selections(&request.selections)?;
        if !valid_bounds(request.bounds) {
            return Err("Choose an increasing WGS84 project area".into());
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
        let mut additions = Vec::new();
        for pin in request.selections {
            stac::resolve(&self.inner.root, &pin)?;
            let already = previous
                .as_ref()
                .map(|p| {
                    p.stac_items
                        .iter()
                        .map(|i| Selection {
                            snapshot_id: i.snapshot_id.clone(),
                            asset_key: i.asset_key.clone(),
                        })
                        .map(|saved| stac::same_selection(&self.inner.root, &saved, &pin))
                        .collect::<Result<Vec<_>>>()
                })
                .transpose()?
                .is_some_and(|v| v.contains(&true));
            if additions
                .iter()
                .map(|saved| stac::same_selection(&self.inner.root, saved, &pin))
                .collect::<Result<Vec<_>>>()?
                .contains(&true)
            {
                return Err("Choose unique archived custom raster assets.".into());
            }
            if !already {
                additions.push(pin);
            }
        }
        request.selections = additions;
        if let Some(p) = &previous {
            request.bounds = p.bounds;
        }
        let mut candidate = if let Some(p) = &previous {
            p.clone()
        } else {
            crate::Project {
                id: Uuid::new_v4().to_string(),
                name: request.name.clone().ok_or("Name the project.")?,
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
                .stac_items
                .push(stac::resolve(&self.inner.root, pin)?);
        }
        crate::stac_projects::validate_project(&self.inner.root, &candidate)?;
        let target = previous
            .as_ref()
            .map(ProjectPin::from_project)
            .transpose()?;
        self.save_agent_plan(session, Action::StacProject { request, target })
            .await
    }

    pub async fn agent_stac_download_plan(
        &self,
        session: &str,
        request: DownloadRequest,
    ) -> Result<Value> {
        if !uuid(session) || !uuid(&request.project_id) {
            return Err("Choose a saved custom-source project.".into());
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
                .stac_items
                .iter()
                .map(|i| Selection {
                    snapshot_id: i.snapshot_id.clone(),
                    asset_key: i.asset_key.clone(),
                })
                .collect()
        });
        selections(&requested)?;
        let mut pins = Vec::new();
        for selection in requested {
            let mut saved = None;
            for item in &project.stac_items {
                let pin = Selection {
                    snapshot_id: item.snapshot_id.clone(),
                    asset_key: item.asset_key.clone(),
                };
                if stac::same_selection(&self.inner.root, &pin, &selection)? {
                    saved = Some(pin);
                    break;
                }
            }
            pins.push(saved.ok_or("Choose source assets belonging to this project")?);
        }
        selections(&pins)?;
        let mut files = Vec::new();
        let mut reused = Vec::new();
        for selection in pins {
            if let Some(value) = reusable(self, &selection).await? {
                reused.push(value);
                continue;
            }
            let item = stac::resolve(&self.inner.root, &selection)?;
            files.push(FilePin {
                remote: head(self, &selection).await?,
                selection,
                item,
            });
        }
        let pin = ProjectPin::from_project(&project)?;
        pin.verify(self.inner.projects.lock().await.get(&pin.id))?;
        if files.is_empty() {
            return Ok(
                json!({"project":{"id":project.id,"name":project.name},"jobs":reused,"needsDownload":false,"note":"All requested files already have native tasks. Read their status or prepare a processing plan."}),
            );
        }
        self.save_agent_plan(
            session,
            Action::StacDownload {
                project: pin,
                bounds: project.bounds,
                files,
            },
        )
        .await
    }
}

pub(super) async fn status(manager: &JobManager, plan: &Plan) -> Result<Value> {
    let projects = manager.inner.projects.lock().await;
    let (kind, id, name, bounds, mode, selections, committed, expected) = match &plan.action {
        Action::StacProject { request, target } => {
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
                request.selections.clone(),
                committed,
                None,
            )
        }
        Action::StacDownload {
            project,
            bounds,
            files,
        } => (
            "download",
            project.id.clone(),
            project.name.clone(),
            *bounds,
            "existing",
            files.iter().map(|f| f.selection.clone()).collect(),
            false,
            Some(files.iter().map(|f| f.remote.bytes).sum::<u64>()),
        ),
        _ => return Err("Invalid custom review action.".into()),
    };
    let store = manager.inner.store.lock().await;
    let saved = projects.contains_key(&id);
    let jobs = existing(plan, &store.jobs)?;
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
    let files = selections.iter().enumerate().map(|(index,pin)| {
        let item = stac::resolve(&manager.inner.root, pin)?;
        let bytes = match &plan.action {Action::StacDownload {files,..}=>Some(files[index].remote.bytes),_=>None};
        Ok::<_,String>(json!({"itemId":item.item_id,"assetKey":if kind=="project"{"scene"}else{"stac_asset"},
            "originalAssetKey":pin.asset_key,"snapshotId":pin.snapshot_id,"referenceId":selection_id(pin),"date":item.datetime.or(item.start_datetime),"bytes":bytes}))
    }).collect::<Result<Vec<_>>>()?;
    let polygon = projects.get(&id).and_then(|p|p.geometry.as_ref()).map(|geometry| Ok::<_,String>(json!({"bounds":geometry.bounds()?,"sha256":digest(&serde_json::to_vec(geometry).map_err(io_error)?)}))).transpose()?;
    Ok(
        json!({"planId":plan.id,"planHash":plan.hash,"kind":kind,"status":state,"source":"Selected custom raster assets",
        "bounds":bounds,"boundsCrs":"EPSG:4326","polygon":polygon,"start":null,"end":null,"files":files,
        "replacedBy":replacement.map(|r|r.plan_id),"expectedBytes":expected,"format":if kind=="project"{"Project"}else{"GeoTIFF"},
        "output":"Managed workspace files","expiresAt":plan.expires_at,"approvalRequired":true,
        "project":{"id":id,"name":name,"sceneCount":selections.len(),"mode":mode,"committed":committed,"saved":saved},"processing":null,
        "notes":[if kind=="project"{"Saves archived original asset selections; no files are downloaded."}else{"Downloads retain complete original rasters; no scientific type is inferred from an asset name."}],
        "jobs":jobs.unwrap_or_default().iter().map(|j|job_view(j,!crate::active(&j.status)&&!store.active.contains_key(&j.id))).collect::<Vec<_>>() }),
    )
}

pub(super) async fn commit(manager: &JobManager, plan: &Plan) -> Result<Value> {
    match &plan.action {
        Action::StacProject { request, target } => {
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
            for selection in &request.selections {
                current
                    .stac_items
                    .push(stac::resolve(&manager.inner.root, selection)?);
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
        Action::StacDownload { project, files, .. } => {
            if existing(plan, &manager.inner.store.lock().await.jobs)?.is_some() {
                return status(manager, plan).await;
            }
            if plan.expired() {
                return Err("Agent plan expired. Create a new plan before confirming.".into());
            }
            for file in files {
                if stac::resolve(&manager.inner.root, &file.selection)? != file.item
                    || head(manager, &file.selection).await? != file.remote
                {
                    return Err("Source file changed. Create and review a new plan.".into());
                }
                if reusable(manager, &file.selection).await?.is_some() {
                    return Err("A selected original already has a reusable task. Prepare a new download plan.".into());
                }
            }
            let projects = manager.inner.projects.lock().await;
            project.verify(projects.get(&project.id))?;
            let mut store = manager.inner.store.lock().await;
            store.accepting_jobs()?;
            if existing(plan, &store.jobs)?.is_some() {
                drop(store);
                drop(projects);
                return status(manager, plan).await;
            }
            if plan.expired() {
                return Err("Agent plan expired. Create a new plan before confirming.".into());
            }
            // A concurrent app/MCP transfer may arrive while HEAD is running.
            if files.iter().any(|f| {
                store.jobs.values().any(|j| {
                    j.stac_source.as_ref() == Some(&f.selection)
                        && matches!(j.status, JobStatus::Queued | JobStatus::Running)
                })
            }) {
                return Err(
                    "A selected original already has a reusable task. Prepare a new download plan."
                        .into(),
                );
            }
            // Recheck newly settled files while admission is locked. A valid
            // original cannot become a second transfer between reuse and commit.
            let mut invalid_originals = Vec::new();
            for file in files {
                for existing in store.jobs.values().filter(|j| {
                    j.stac_source.as_ref() == Some(&file.selection)
                        && j.status == JobStatus::Succeeded
                }) {
                    let root = manager.inner.root.clone();
                    let copy = existing.clone();
                    if tokio::task::spawn_blocking(move || {
                        stac::raster::verified_original(&root, &copy)
                    })
                    .await
                    .map_err(io_error)?
                    .is_ok()
                    {
                        return Err("A selected original already has a reusable task. Prepare a new download plan.".into());
                    }
                    invalid_originals.push(existing.clone());
                }
            }
            if store.active.len() + files.len() > 64 {
                return Err("The task queue is full. Try confirming later.".into());
            }
            if plan.expired() {
                return Err("Agent plan expired. Create a new plan before confirming.".into());
            }
            let ids = plan.job_ids();
            let at = now();
            let mut created = Vec::new();
            for (file, id) in files.iter().zip(&ids) {
                let mut task = job(&file.item, &file.selection);
                task.id = id.clone();
                task.agent_approval = Some(approval(plan, &at, Some(file.remote.clone())));
                stac::validate_job(&manager.inner.root, &task)?;
                created.push(task);
            }
            // Retire invalid receipts in the same durable commit as their
            // replacements, so app/MCP reuse cannot pick the corrupt file again.
            for original in &invalid_originals {
                let mut invalid = original.clone();
                invalid.status = JobStatus::Failed;
                invalid.error = Some(
                    "The completed local original is missing or failed size / SHA-256 verification"
                        .into(),
                );
                invalid.output_path = None;
                invalid.sha256 = None;
                invalid.updated_at = at.clone();
                store.jobs.insert(invalid.id.clone(), invalid);
            }
            for task in &created {
                store.jobs.insert(task.id.clone(), task.clone());
            }
            if let Err(error) = manager.persist(&store.jobs).await {
                for id in ids {
                    store.jobs.remove(&id);
                }
                for original in invalid_originals {
                    store.jobs.insert(original.id.clone(), original);
                }
                return Err(error);
            }
            for task in created {
                let token = CancellationToken::new();
                store.active.insert(task.id.clone(), token.clone());
                manager.spawn(task.id, token);
            }
            drop(store);
            drop(projects);
        }
        _ => return Err("Invalid custom review action.".into()),
    }
    status(manager, plan).await
}

pub(super) async fn revision_draft(manager: &JobManager, plan: &Plan) -> Result<Value> {
    let (kind, pins, name, bounds, project_id, polygon) = match &plan.action {
        Action::StacProject { request, target } => {
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
        Action::StacDownload { project, files, .. } => {
            project.verify(manager.inner.projects.lock().await.get(&project.id))?;
            (
                "download",
                files.iter().map(|f| f.selection.clone()).collect(),
                None,
                None,
                Some(project.id.clone()),
                false,
            )
        }
        _ => return Err("Invalid custom review action.".into()),
    };
    let items=pins.iter().map(|pin|{
        let item=stac::resolve(&manager.inner.root,pin)?;
        Ok::<_,String>(json!({"id":selection_id(pin),"label":format!("{} · {}",item.item_id,pin.asset_key),"date":item.datetime.or(item.start_datetime),"locked":false}))
    }).collect::<Result<Vec<_>>>()?;
    let mut parameters =
        json!({"kind":kind,"itemIds":pins.iter().map(selection_id).collect::<Vec<_>>()});
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

fn selected(ids: Vec<String>, pins: &[Selection]) -> Result<Vec<Selection>> {
    let count = ids.len();
    let ids = ids.into_iter().collect::<BTreeSet<_>>();
    if ids.is_empty()
        || ids.len() != count
        || ids
            .iter()
            .any(|id| !pins.iter().any(|p| selection_id(p) == *id))
    {
        return Err("Select distinct scenes from this native review.".into());
    }
    Ok(pins
        .iter()
        .filter(|p| ids.contains(&selection_id(p)))
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
            Action::StacProject {
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
                    return Err("Keep the saved project area and scenes; select at least one scene to append.".into());
                }
            } else {
                if keep_polygon {
                    return Err("This review has no saved polygon.".into());
                }
                request.name = Some(name.ok_or("Name the project.")?);
                request.bounds = bounds.ok_or("Set the project area.")?;
            }
            manager.agent_stac_project_plan(session, request).await
        }
        (Action::StacDownload { project, files, .. }, PlanRevision::Download { item_ids }) => {
            project.verify(manager.inner.projects.lock().await.get(&project.id))?;
            let pins = files
                .iter()
                .map(|f| f.selection.clone())
                .collect::<Vec<_>>();
            let next = manager
                .agent_stac_download_plan(
                    session,
                    DownloadRequest {
                        project_id: project.id,
                        selections: Some(selected(item_ids, &pins)?),
                    },
                )
                .await?;
            if next.get("planId").is_none() {
                return Err(
                    "A selected original already has a reusable task. Prepare a new download plan."
                        .into(),
                );
            }
            Ok(next)
        }
        _ => Err("The form belongs to a different review kind.".into()),
    }
}

pub(super) fn definitions() -> Vec<Value> {
    let selection = json!({"type":"object","properties":{"snapshotId":{"type":"string","pattern":"^[a-f0-9]{64}$"},"assetKey":{"type":"string","minLength":1,"maxLength":512}},"required":["snapshotId","assetKey"],"additionalProperties":false});
    let selections = json!({"type":"array","items":selection,"minItems":1,"maxItems":32});
    let uuid = json!({"type":"string","format":"uuid"});
    vec![
        json!({"name":"geod_stac_project_plan","description":"Prepare a native project review from archived custom STAC snapshotId/original assetKey selections. Supply name for a new project or projectId to append, plus explicit WGS84 bounds. Appending preserves the saved area and all existing sources. No project or file transfer until card confirmation. No href, local path or credentials accepted.","inputSchema":{"type":"object","properties":{"name":{"type":"string","maxLength":120},"projectId":uuid,"bounds":{"type":"array","items":{"type":"number"},"minItems":4,"maxItems":4},"selections":selections},"required":["bounds","selections"],"additionalProperties":false}}),
        json!({"name":"geod_stac_download_plan","description":"Prepare downloads of archived original assets belonging to a confirmed custom-source project. Optional selections chooses a subset; otherwise all project custom assets. Verifies completed files before reuse; reads remote sizes/strong ETags for missing files. Native confirmation required. Original complete rasters only, at most 32 files and 512 MiB per file. No calibration/type inference or direct model approval.","inputSchema":{"type":"object","properties":{"projectId":uuid,"selections":selections},"required":["projectId"],"additionalProperties":false}}),
    ]
}

#[cfg(test)]
mod tests;
