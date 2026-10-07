//! Coverage selections retain a native request plan; client URLs never start transfers.
use crate::{
    projects::{valid_bounds, validate_project_name, MAX_PROJECT_SCENES},
    stac_projects::validate_project,
    wcs::{self, SourcePin},
    CreateJobRequest, Job, JobManager, JobStatus, Project, ProjectDownloads, Result,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    path::Path,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveProjectRequest {
    pub project_id: Option<String>,
    pub name: Option<String>,
    pub bounds: [f64; 4],
    pub selections: Vec<SourcePin>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DownloadRequest {
    pub project_id: String,
    pub selections: Option<Vec<SourcePin>>,
}

/// Internal admission contract for a native human-confirmed Agent review.
/// Not deserializable from desktop/MCP download requests.
pub(crate) struct ReviewedDownload {
    pub project_hash: String,
    pub expires_at: String,
    pub jobs: Vec<(String, crate::agent_actions::ApprovalReceipt)>,
}

fn matching_pin(root: &Path, project: &Project, pin: &SourcePin) -> Result<Option<SourcePin>> {
    for item in &project.wcs_items {
        let saved = SourcePin {
            plan_id: item.plan_id.clone(),
        };
        if wcs::same_selection(root, &saved, pin)? {
            return Ok(Some(saved));
        }
    }
    Ok(None)
}

fn same_completed(a: &Job, b: &Job) -> bool {
    a.id == b.id
        && a.status == JobStatus::Succeeded
        && b.status == JobStatus::Succeeded
        && a.wcs_source == b.wcs_source
        && a.sha256 == b.sha256
        && a.output_path == b.output_path
        && a.bytes_downloaded == b.bytes_downloaded
        && a.total_bytes == b.total_bytes
        && a.updated_at == b.updated_at
}

impl JobManager {
    pub async fn save_wcs_project(&self, request: SaveProjectRequest) -> Result<Project> {
        self.inner.store.lock().await.accepting_jobs()?;
        if request.selections.is_empty() || request.selections.len() > MAX_PROJECT_SCENES {
            return Err(format!(
                "Select 1 to {MAX_PROJECT_SCENES} coverage requests"
            ));
        }
        if !valid_bounds(request.bounds) {
            return Err("Choose an increasing WGS84 project area".into());
        }
        let mut seen = HashSet::new();
        let mut additions = Vec::new();
        for pin in &request.selections {
            if !seen.insert(pin.plan_id.clone()) {
                return Err("The same coverage request was selected twice".into());
            }
            additions.push(wcs::resolve(&self.inner.root, pin)?);
        }
        let mut projects = self.inner.projects.lock().await;
        let previous = match &request.project_id {
            Some(id) => Some(projects.get(id).cloned().ok_or("Unknown project")?),
            None => None,
        };
        if previous.is_some() && request.name.is_some() {
            return Err("Rename existing projects separately".into());
        }
        let timestamp = crate::now();
        let mut project = if let Some(project) = &previous {
            project.clone()
        } else {
            Project {
                id: Uuid::new_v4().to_string(),
                name: validate_project_name(
                    request.name.as_deref().ok_or("Enter a project name")?,
                )?,
                bounds: request.bounds,
                geometry: None,
                scenes: Vec::new(),
                stac_items: Vec::new(),
                wcs_items: Vec::new(),
                created_at: timestamp.clone(),
                updated_at: timestamp.clone(),
                agent_approvals: Vec::new(),
            }
        };
        for item in additions {
            if matching_pin(
                &self.inner.root,
                &project,
                &SourcePin {
                    plan_id: item.plan_id.clone(),
                },
            )?
            .is_none()
            {
                project.wcs_items.push(item);
            }
        }
        validate_project(&self.inner.root, &project)?;
        if previous
            .as_ref()
            .is_some_and(|old| old.wcs_items == project.wcs_items)
        {
            return Ok(project);
        }
        project.updated_at = timestamp;
        projects.insert(project.id.clone(), project.clone());
        if let Err(error) = self.persist_projects(&projects).await {
            if let Some(old) = previous {
                projects.insert(old.id.clone(), old);
            } else {
                projects.remove(&project.id);
            }
            return Err(error);
        }
        Ok(project)
    }

    pub async fn download_wcs_project(&self, request: DownloadRequest) -> Result<ProjectDownloads> {
        self.download_wcs_project_inner(request, None).await
    }

    pub(crate) async fn download_wcs_project_reviewed(
        &self,
        request: DownloadRequest,
        review: ReviewedDownload,
    ) -> Result<ProjectDownloads> {
        self.download_wcs_project_inner(request, Some(review)).await
    }

    async fn download_wcs_project_inner(
        &self,
        request: DownloadRequest,
        review: Option<ReviewedDownload>,
    ) -> Result<ProjectDownloads> {
        let project = self
            .inner
            .projects
            .lock()
            .await
            .get(&request.project_id)
            .cloned()
            .ok_or("Unknown project")?;
        validate_project(&self.inner.root, &project)?;
        let pins = request.selections.unwrap_or_else(|| {
            project
                .wcs_items
                .iter()
                .map(|item| SourcePin {
                    plan_id: item.plan_id.clone(),
                })
                .collect()
        });
        if pins.is_empty() || pins.len() > MAX_PROJECT_SCENES {
            return Err("Select coverage requests belonging to this project".into());
        }
        let mut seen = HashSet::new();
        let mut resolved = Vec::new();
        for pin in pins {
            let pin = matching_pin(&self.inner.root, &project, &pin)?
                .ok_or("Choose coverage requests belonging to this project")?;
            if !seen.insert(pin.plan_id.clone()) {
                return Err("Choose unique coverage requests belonging to this project".into());
            }
            resolved.push((wcs::resolve(&self.inner.root, &pin)?, pin));
        }
        if review
            .as_ref()
            .is_some_and(|r| r.jobs.len() != resolved.len())
        {
            return Err("Coverage review task count changed".into());
        }
        let mut verified: HashMap<String, (Job, Result<File>)> = HashMap::new();
        let mut store = self.inner.store.lock().await;
        store.accepting_jobs()?;
        // Avoid holding the task-store lock during whole-file checks. Keep verified
        // files shared-locked through reuse and recheck records after each await.
        loop {
            let unchecked = store
                .jobs
                .values()
                .filter(|job| {
                    job.status == JobStatus::Succeeded
                        && resolved
                            .iter()
                            .any(|(_, p)| job.wcs_source.as_ref() == Some(p))
                        && !verified
                            .get(&job.id)
                            .is_some_and(|(checked, _)| same_completed(checked, job))
                })
                .cloned()
                .collect::<Vec<_>>();
            if unchecked.is_empty() {
                break;
            }
            drop(store);
            for job in unchecked {
                let root = self.inner.root.clone();
                let check = job.clone();
                let file = tokio::task::spawn_blocking(move || {
                    crate::stac::raster::verified_original(&root, &check)
                })
                .await
                .map_err(crate::io_error)?;
                verified.insert(job.id.clone(), (job, file));
            }
            store = self.inner.store.lock().await;
            store.accepting_jobs()?;
        }
        // Recheck the entire mixed project scope at admission and hold it through
        // the atomic queue commit. Release the store first to preserve lock order.
        drop(store);
        let projects = self.inner.projects.lock().await;
        if let Some(review) = &review {
            let current = projects
                .get(&project.id)
                .ok_or("The target project was removed")?;
            if crate::agent_actions::ProjectScope::from_project(current).fingerprint()?
                != review.project_hash
            {
                return Err("The project changed. Create and review a new plan.".into());
            }
        }
        store = self.inner.store.lock().await;
        store.accepting_jobs()?;
        // Completed records can change while taking the project lock. Never
        // reuse a file checked for another receipt or retire an unchecked file.
        if store.jobs.values().any(|job| {
            job.status == JobStatus::Succeeded
                && resolved
                    .iter()
                    .any(|(_, p)| job.wcs_source.as_ref() == Some(p))
                && !verified
                    .get(&job.id)
                    .is_some_and(|(checked, _)| same_completed(checked, job))
        }) {
            return Err("Coverage tasks changed during admission; prepare a fresh request".into());
        }
        if let Some(review) = &review {
            if chrono::DateTime::parse_from_rfc3339(&review.expires_at)
                .map_or(true, |date| date <= chrono::Utc::now())
            {
                return Err("Agent plan expired. Create a new plan before confirming.".into());
            }
            if review
                .jobs
                .iter()
                .any(|(id, _)| store.jobs.contains_key(id))
            {
                return Err(
                    "Coverage review task identifier conflicts with an existing task".into(),
                );
            }
        }
        let mut jobs = Vec::new();
        let mut created = Vec::new();
        let mut invalid = Vec::new();
        for (index, (item, pin)) in resolved.into_iter().enumerate() {
            if let Some(existing) = store.jobs.values().find(|job| {
                job.wcs_source.as_ref() == Some(&pin)
                    && matches!(
                        job.status,
                        JobStatus::Succeeded | JobStatus::Queued | JobStatus::Running
                    )
            }) {
                wcs::validate_job(&self.inner.root, existing)?;
                if existing.status != JobStatus::Succeeded
                    || verified.get(&existing.id).is_some_and(|(_, f)| f.is_ok())
                {
                    if review.is_some() {
                        return Err("A selected coverage already has a reusable task. Prepare a new download plan.".into());
                    }
                    jobs.push(existing.clone());
                    continue;
                }
                invalid.push(existing.clone());
            }
            let mut job = crate::new_download_job(CreateJobRequest {
                item_id: item.coverage_id,
                asset_key: "wcs_coverage".into(),
                href: item.href,
                media_type: item.media_type,
                title: Some(item.title),
            });
            job.source = item.service_name;
            job.wcs_source = Some(pin);
            if let Some(review) = &review {
                job.id = review.jobs[index].0.clone();
                job.agent_approval = Some(review.jobs[index].1.clone());
            }
            job.validation =
                "Pending coverage response size, checksum and declared native grid validation"
                    .into();
            wcs::validate_job(&self.inner.root, &job)?;
            jobs.push(job.clone());
            created.push(job);
        }
        if store.active.len() + created.len() > 64 {
            return Err("The local queue is full; wait for current tasks to finish".into());
        }
        for old in &invalid {
            let mut failed = old.clone();
            failed.status = JobStatus::Failed;
            failed.error = Some(
                "The local coverage file is missing or failed size / SHA-256 verification".into(),
            );
            failed.validation = "Local coverage file failed integrity verification".into();
            failed.output_path = None;
            failed.sha256 = None;
            failed.updated_at = crate::now();
            store.jobs.insert(failed.id.clone(), failed);
        }
        for job in &created {
            store.jobs.insert(job.id.clone(), job.clone());
        }
        if let Err(error) = self.persist(&store.jobs).await {
            for job in &created {
                store.jobs.remove(&job.id);
            }
            for old in invalid {
                store.jobs.insert(old.id.clone(), old);
            }
            return Err(error);
        }
        for job in created {
            let token = CancellationToken::new();
            store.active.insert(job.id.clone(), token.clone());
            self.spawn(job.id, token);
        }
        drop(projects);
        Ok(ProjectDownloads {
            project_id: project.id,
            asset_key: "wcs_coverage".into(),
            jobs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(pin: &SourcePin, project_id: Option<String>) -> SaveProjectRequest {
        SaveProjectRequest {
            name: project_id.is_none().then(|| "Coverage test".into()),
            project_id,
            bounds: [2.0, 53.0, 2.05, 53.05],
            selections: vec![pin.clone()],
        }
    }
    async fn queued(manager: &JobManager, project: &Project) -> Job {
        manager
            .download_wcs_project(DownloadRequest {
                project_id: project.id.clone(),
                selections: None,
            })
            .await
            .unwrap()
            .jobs
            .remove(0)
    }
    #[tokio::test]
    async fn project_persists_coverage_requests_alongside_stac_assets() {
        let dir = tempfile::tempdir().unwrap();
        let manager = JobManager::open(dir.path()).await.unwrap();
        let stac = crate::stac::fixture_snapshot(
            manager.storage_root(),
            "https://example.com/original.tif",
        );
        let project = manager
            .save_stac_project(crate::stac_projects::SaveProjectRequest {
                name: Some("Mixed sources".into()),
                project_id: None,
                bounds: [1.0, 52.0, 3.0, 54.0],
                selections: vec![stac],
            })
            .await
            .unwrap();
        let pin = wcs::fixture_plan(manager.storage_root());
        let updated = manager
            .save_wcs_project(request(&pin, Some(project.id.clone())))
            .await
            .unwrap();
        assert_eq!(updated.stac_items, project.stac_items);
        assert_eq!(updated.bounds, project.bounds);
        assert_eq!(updated.wcs_items.len(), 1);
        let before = std::fs::read(manager.storage_root().join("projects.json")).unwrap();
        manager
            .save_wcs_project(request(&pin, Some(project.id.clone())))
            .await
            .unwrap();
        assert_eq!(
            before,
            std::fs::read(manager.storage_root().join("projects.json")).unwrap()
        );
        manager
            .rename_project(&project.id, "Renamed mixed project")
            .await
            .unwrap();
        drop(manager);
        let reopened = JobManager::open(dir.path()).await.unwrap();
        let restored = reopened.list_projects().await;
        assert_eq!(restored[0].wcs_items, updated.wcs_items);
        assert_eq!(restored[0].name, "Renamed mixed project");
    }
    #[tokio::test]
    async fn queue_retry_and_replacement_do_not_duplicate_the_same_request() {
        let dir = tempfile::tempdir().unwrap();
        let m = JobManager::open(dir.path()).await.unwrap();
        let pin = wcs::fixture_plan(m.storage_root());
        let p = m.save_wcs_project(request(&pin, None)).await.unwrap();
        let permit = m.inner.permits.acquire_many(2).await.unwrap();
        let first = queued(&m, &p).await;
        assert_eq!(queued(&m, &p).await.id, first.id);
        m.cancel(&first.id).await.unwrap();
        m.wait(&first.id).await.unwrap();
        let retry = m.retry(&first.id).await.unwrap();
        assert_eq!(retry.wcs_source, Some(pin.clone()));
        assert_eq!(retry.attempts, 2);
        m.cancel(&first.id).await.unwrap();
        m.wait(&first.id).await.unwrap();
        let replacement = queued(&m, &p).await;
        assert_ne!(replacement.id, first.id);
        assert!(m
            .retry(&first.id)
            .await
            .unwrap_err()
            .contains("already has"));
        assert_eq!(m.inner.store.lock().await.active.len(), 1);
        m.cancel(&replacement.id).await.unwrap();
        m.wait(&replacement.id).await.unwrap();
        drop(permit);
    }
    #[tokio::test]
    async fn failed_reference_and_ordinary_url_endpoint_cannot_create_coverage_jobs() {
        let dir = tempfile::tempdir().unwrap();
        let m = JobManager::open(dir.path()).await.unwrap();
        let pin = wcs::fixture_plan(m.storage_root());
        let p = m.save_wcs_project(request(&pin, None)).await.unwrap();
        let before = std::fs::read(m.storage_root().join("projects.json")).unwrap();
        let unknown = SourcePin {
            plan_id: "f".repeat(64),
        };
        assert!(m
            .save_wcs_project(request(&unknown, Some(p.id.clone())))
            .await
            .is_err());
        assert!(m
            .download_wcs_project(DownloadRequest {
                project_id: p.id.clone(),
                selections: Some(vec![unknown])
            })
            .await
            .is_err());
        assert!(m
            .download_wcs_project(DownloadRequest {
                project_id: p.id,
                selections: Some(vec![pin.clone(), pin])
            })
            .await
            .is_err());
        assert_eq!(
            before,
            std::fs::read(m.storage_root().join("projects.json")).unwrap()
        );
        assert!(m.list().await.is_empty());
        assert!(crate::validate_request(&CreateJobRequest {
            item_id: "S2A_32UQD_20240108_0_L2A".into(), asset_key: "wcs_coverage".into(),
            href: format!("https://{}/sentinel-s2-l2a-cogs/32/U/QD/2024/1/S2A_32UQD_20240108_0_L2A/SCL.tif", crate::SOURCE_HOST),
            media_type: "image/tiff".into(), title: None,
        }, None).is_err());
    }
    #[tokio::test]
    async fn integrity_check_reuses_bytes_and_recovers_same_size_changed_subset() {
        use sha2::{Digest, Sha256};
        let dir = tempfile::tempdir().unwrap();
        let m = JobManager::open(dir.path()).await.unwrap();
        let pin = wcs::fixture_plan(m.storage_root());
        let p = m.save_wcs_project(request(&pin, None)).await.unwrap();
        let permit = m.inner.permits.acquire_many(2).await.unwrap();
        let first = queued(&m, &p).await;
        m.cancel(&first.id).await.unwrap();
        m.wait(&first.id).await.unwrap();
        let path = m
            .storage_root()
            .join("assets")
            .join(format!("{}.tif", first.id));
        let bytes = b"II*\0managed coverage test bytes";
        std::fs::write(&path, bytes).unwrap();
        {
            let mut store = m.inner.store.lock().await;
            let j = store.jobs.get_mut(&first.id).unwrap();
            j.status = JobStatus::Succeeded;
            j.bytes_downloaded = bytes.len() as u64;
            j.total_bytes = Some(bytes.len() as u64);
            j.output_path = Some(path.to_string_lossy().into_owned());
            j.sha256 = Some(format!("{:x}", Sha256::digest(bytes)));
            m.persist(&store.jobs).await.unwrap();
        }
        assert_eq!(queued(&m, &p).await.id, first.id);
        let mut changed = bytes.to_vec();
        changed[6] ^= 1;
        std::fs::write(&path, changed).unwrap();
        let replacement = queued(&m, &p).await;
        assert_ne!(replacement.id, first.id);
        assert_eq!(m.get(&first.id).await.unwrap().status, JobStatus::Failed);
        m.cancel(&replacement.id).await.unwrap();
        m.wait(&replacement.id).await.unwrap();
        drop(permit);
    }
    #[tokio::test]
    async fn restart_interrupts_incomplete_transfer_and_keeps_request_identity() {
        let dir = tempfile::tempdir().unwrap();
        let m = JobManager::open(dir.path()).await.unwrap();
        let pin = wcs::fixture_plan(m.storage_root());
        let p = m.save_wcs_project(request(&pin, None)).await.unwrap();
        let permit = m.inner.permits.acquire_many(2).await.unwrap();
        let first = queued(&m, &p).await;
        m.cancel(&first.id).await.unwrap();
        m.wait(&first.id).await.unwrap();
        drop(permit);
        {
            let mut store = m.inner.store.lock().await;
            store.jobs.get_mut(&first.id).unwrap().status = JobStatus::Running;
            m.persist(&store.jobs).await.unwrap();
        }
        let partial = m
            .storage_root()
            .join("assets")
            .join(format!("{}.part", first.id));
        std::fs::write(&partial, b"partial").unwrap();
        drop(m);
        let reopened = JobManager::open(dir.path()).await.unwrap();
        let restored = reopened.get(&first.id).await.unwrap();
        assert_eq!(restored.status, JobStatus::Interrupted);
        assert_eq!(restored.wcs_source, Some(pin));
        assert!(!partial.exists());
    }
}
