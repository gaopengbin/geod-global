//! Custom source selections share the ordinary project and transfer lifecycle.
//! The UI supplies snapshot references; URLs and display metadata are resolved natively.
use crate::{
    projects::{valid_bounds, validate_project_name, MAX_PROJECT_SCENES},
    stac::{self, Selection},
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
    pub selections: Vec<Selection>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DownloadRequest {
    pub project_id: String,
    pub selections: Option<Vec<Selection>>,
}

fn identity(selection: &Selection) -> (String, String) {
    (selection.snapshot_id.clone(), selection.asset_key.clone())
}

fn same_completed_record(a: &Job, b: &Job) -> bool {
    a.id == b.id
        && a.status == JobStatus::Succeeded
        && b.status == JobStatus::Succeeded
        && a.stac_source == b.stac_source
        && a.sha256 == b.sha256
        && a.output_path == b.output_path
        && a.bytes_downloaded == b.bytes_downloaded
        && a.total_bytes == b.total_bytes
        && a.updated_at == b.updated_at
}

fn item_pin(item: &stac::ProjectItem) -> Selection {
    Selection {
        snapshot_id: item.snapshot_id.clone(),
        asset_key: item.asset_key.clone(),
    }
}

fn matching_pin(
    root: &Path,
    project: &Project,
    selection: &Selection,
) -> Result<Option<Selection>> {
    for item in &project.stac_items {
        let saved = item_pin(item);
        if stac::same_selection(root, &saved, selection)? {
            return Ok(Some(saved));
        }
    }
    Ok(None)
}

pub(crate) fn validate_project(root: &Path, project: &Project) -> Result<()> {
    validate_project_name(&project.name)?;
    if !valid_bounds(project.bounds) {
        return Err("Project requires increasing WGS84 bounds".into());
    }
    if let Some(geometry) = &project.geometry {
        let b = geometry.bounds()?;
        if b[0] >= project.bounds[2]
            || b[2] <= project.bounds[0]
            || b[1] >= project.bounds[3]
            || b[3] <= project.bounds[1]
        {
            return Err("Project polygon does not intersect its saved area".into());
        }
    }
    let count = project.scenes.len() + project.stac_items.len() + project.wcs_items.len();
    if count == 0 || count > MAX_PROJECT_SCENES {
        return Err(format!(
            "A project supports 1 to {MAX_PROJECT_SCENES} source selections"
        ));
    }
    let mut seen = HashSet::new();
    for item in &project.stac_items {
        let pin = Selection {
            snapshot_id: item.snapshot_id.clone(),
            asset_key: item.asset_key.clone(),
        };
        if !seen.insert(identity(&pin)) || stac::resolve(root, &pin)? != *item {
            return Err("Project custom source metadata does not match its saved snapshot".into());
        }
    }
    let mut coverage_ids = HashSet::new();
    for item in &project.wcs_items {
        let pin = crate::wcs::SourcePin {
            plan_id: item.plan_id.clone(),
        };
        if !coverage_ids.insert(pin.plan_id.clone()) || crate::wcs::resolve(root, &pin)? != *item {
            return Err("Project coverage metadata does not match its saved request".into());
        }
    }
    Ok(())
}

impl JobManager {
    pub async fn save_stac_project(&self, request: SaveProjectRequest) -> Result<Project> {
        if request.selections.is_empty() || request.selections.len() > MAX_PROJECT_SCENES {
            return Err(format!("Select 1 to {MAX_PROJECT_SCENES} original assets"));
        }
        if !valid_bounds(request.bounds) {
            return Err("Choose an increasing WGS84 project area".into());
        }
        let mut additions = Vec::new();
        let mut seen = HashSet::new();
        for selection in &request.selections {
            if !seen.insert(identity(selection)) {
                return Err("The same original asset was selected twice".into());
            }
            additions.push(stac::resolve(&self.inner.root, selection)?);
        }
        let mut projects = self.inner.projects.lock().await;
        let previous = if let Some(id) = &request.project_id {
            Some(projects.get(id).cloned().ok_or("Unknown project")?)
        } else {
            None
        };
        if previous.is_some() && request.name.is_some() {
            return Err("Rename existing projects separately".into());
        }
        let timestamp = crate::now();
        let mut project = match &previous {
            Some(project) => project.clone(),
            None => Project {
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
            },
        };
        // Appending a new source never silently replaces the existing project's AOI.
        for item in additions {
            if matching_pin(&self.inner.root, &project, &item_pin(&item))?.is_none() {
                project.stac_items.push(item);
            }
        }
        validate_project(&self.inner.root, &project)?;
        if previous
            .as_ref()
            .is_some_and(|old| old.stac_items == project.stac_items)
        {
            return Ok(project);
        }
        project.updated_at = timestamp;
        projects.insert(project.id.clone(), project.clone());
        if let Err(error) = self.persist_projects(&projects).await {
            if let Some(previous) = previous {
                projects.insert(previous.id.clone(), previous);
            } else {
                projects.remove(&project.id);
            }
            return Err(error);
        }
        Ok(project)
    }

    pub async fn download_stac_project(
        &self,
        request: DownloadRequest,
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
        let selections = request.selections.unwrap_or_else(|| {
            project
                .stac_items
                .iter()
                .map(|item| Selection {
                    snapshot_id: item.snapshot_id.clone(),
                    asset_key: item.asset_key.clone(),
                })
                .collect()
        });
        if selections.is_empty() || selections.len() > MAX_PROJECT_SCENES {
            return Err("Select original assets belonging to this project".into());
        }
        let mut seen = HashSet::new();
        let mut resolved = Vec::new();
        for selection in selections {
            let selection = matching_pin(&self.inner.root, &project, &selection)?
                .ok_or("Choose source assets belonging to this project")?;
            if !seen.insert(identity(&selection)) {
                return Err("Choose unique source assets belonging to this project".into());
            }
            resolved.push((stac::resolve(&self.inner.root, &selection)?, selection));
        }
        let mut store = self.inner.store.lock().await;
        store.accepting_jobs()?;
        // Hash completed originals outside the task-store lock. Keep successful
        // file locks until reuse is committed, and recheck records after awaiting.
        let mut verified: HashMap<String, (Job, Result<File>)> = HashMap::new();
        loop {
            let unchecked = store
                .jobs
                .values()
                .filter(|job| {
                    job.status == JobStatus::Succeeded
                        && resolved
                            .iter()
                            .any(|(_, pin)| job.stac_source.as_ref() == Some(pin))
                        && !verified
                            .get(&job.id)
                            .is_some_and(|(checked, _)| same_completed_record(checked, job))
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
                    stac::raster::verified_original(&root, &check)
                })
                .await
                .map_err(crate::io_error)?;
                verified.insert(job.id.clone(), (job, file));
            }
            store = self.inner.store.lock().await;
            store.accepting_jobs()?;
        }
        let mut jobs = Vec::new();
        let mut created = Vec::new();
        let mut missing = Vec::new();
        for (item, pin) in resolved {
            if let Some(existing) = store.jobs.values().find(|job| {
                job.stac_source.as_ref() == Some(&pin)
                    && matches!(
                        job.status,
                        JobStatus::Succeeded | JobStatus::Queued | JobStatus::Running
                    )
            }) {
                stac::validate_job(&self.inner.root, existing)?;
                let present = existing.status != JobStatus::Succeeded
                    || verified
                        .get(&existing.id)
                        .is_some_and(|(_, file)| file.is_ok());
                if present {
                    jobs.push(existing.clone());
                    continue;
                }
                missing.push(existing.clone());
            }
            let mut job = crate::new_download_job(CreateJobRequest {
                item_id: item.item_id,
                asset_key: "stac_asset".into(),
                href: item.href,
                media_type: item.media_type,
                title: Some(item.title),
            });
            job.source = item.service_name;
            job.stac_source = Some(pin);
            stac::validate_job(&self.inner.root, &job)?;
            jobs.push(job.clone());
            created.push(job);
        }
        if store.active.len() + created.len() > 64 {
            return Err("The local queue is full; wait for current tasks to finish".into());
        }
        for old in &missing {
            let mut failed = old.clone();
            failed.status = JobStatus::Failed;
            failed.error = Some(
                "The completed local original is missing or failed size / SHA-256 verification"
                    .into(),
            );
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
            for old in missing {
                store.jobs.insert(old.id.clone(), old);
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
            asset_key: "stac_asset".into(),
            jobs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const AREA: [f64; 4] = [13.0, 52.0, 14.0, 53.0];

    fn save(pin: &Selection, project_id: Option<String>) -> SaveProjectRequest {
        SaveProjectRequest {
            name: if project_id.is_none() {
                Some("Public raster project".into())
            } else {
                None
            },
            project_id,
            bounds: AREA,
            selections: vec![pin.clone()],
        }
    }

    #[tokio::test]
    async fn custom_project_pins_original_identity_and_reopens_without_catalog() {
        let dir = tempfile::tempdir().unwrap();
        let manager = JobManager::open(dir.path()).await.unwrap();
        let pin =
            stac::fixture_snapshot(manager.storage_root(), "https://example.com/original.tif");
        let project = manager.save_stac_project(save(&pin, None)).await.unwrap();
        assert!(project.scenes.is_empty());
        assert_eq!(
            project.stac_items[0],
            stac::resolve(manager.storage_root(), &pin).unwrap()
        );
        let saved = std::fs::read(manager.storage_root().join("projects.json")).unwrap();
        manager
            .save_stac_project(save(&pin, Some(project.id.clone())))
            .await
            .unwrap();
        assert_eq!(
            saved,
            std::fs::read(manager.storage_root().join("projects.json")).unwrap()
        );
        manager
            .rename_project(&project.id, "Renamed")
            .await
            .unwrap();
        drop(manager);
        let reopened = JobManager::open(dir.path()).await.unwrap();
        let projects = reopened.list_projects().await;
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "Renamed");
        assert_eq!(projects[0].stac_items, project.stac_items);
        assert!(reopened.list_stac_connections().await.is_empty());
    }

    #[tokio::test]
    async fn rejects_foreign_duplicate_and_forged_selections_before_mutation() {
        let dir = tempfile::tempdir().unwrap();
        let manager = JobManager::open(dir.path()).await.unwrap();
        let pin = stac::fixture_snapshot(manager.storage_root(), "https://example.com/one.tif");
        let other = stac::fixture_snapshot(manager.storage_root(), "https://example.com/two.tif");
        let project = manager.save_stac_project(save(&pin, None)).await.unwrap();
        let saved = std::fs::read(manager.storage_root().join("projects.json")).unwrap();
        let jobs = std::fs::read(manager.storage_root().join("jobs.json")).unwrap();
        let mut duplicate = save(&pin, None);
        duplicate.selections.push(pin.clone());
        assert!(manager.save_stac_project(duplicate).await.is_err());
        let mut forged = pin.clone();
        forged.asset_key = "missing".into();
        assert!(manager
            .save_stac_project(save(&forged, Some(project.id.clone())))
            .await
            .is_err());
        assert!(manager
            .download_stac_project(DownloadRequest {
                project_id: project.id.clone(),
                selections: Some(vec![other])
            })
            .await
            .is_err());
        assert_eq!(
            saved,
            std::fs::read(manager.storage_root().join("projects.json")).unwrap()
        );
        assert_eq!(
            jobs,
            std::fs::read(manager.storage_root().join("jobs.json")).unwrap()
        );
    }

    #[tokio::test]
    async fn cancellation_retry_and_queue_reuse_keep_the_same_source_pin() {
        let dir = tempfile::tempdir().unwrap();
        let manager = JobManager::open(dir.path()).await.unwrap();
        let pin =
            stac::fixture_snapshot(manager.storage_root(), "https://example.com/original.tif");
        let project = manager.save_stac_project(save(&pin, None)).await.unwrap();
        // Hold both transfer slots: lifecycle is exercised without making network requests.
        let permit = manager.inner.permits.acquire_many(2).await.unwrap();
        let first = manager
            .download_stac_project(DownloadRequest {
                project_id: project.id.clone(),
                selections: None,
            })
            .await
            .unwrap();
        let job = &first.jobs[0];
        assert_eq!(job.asset_key, "stac_asset");
        assert_eq!(job.stac_source.as_ref(), Some(&pin));
        let second = manager
            .download_stac_project(DownloadRequest {
                project_id: project.id,
                selections: None,
            })
            .await
            .unwrap();
        assert_eq!(second.jobs[0].id, job.id);
        manager.cancel(&job.id).await.unwrap();
        assert_eq!(
            manager.wait(&job.id).await.unwrap().status,
            JobStatus::Cancelled
        );
        let retry = manager.retry(&job.id).await.unwrap();
        assert_eq!(retry.attempts, 2);
        assert_eq!(retry.stac_source.as_ref(), Some(&pin));
        manager.cancel(&job.id).await.unwrap();
        manager.wait(&job.id).await.unwrap();
        drop(permit);
        drop(manager);
        let reopened = JobManager::open(dir.path()).await.unwrap();
        let job = reopened.get(&job.id).await.unwrap();
        assert_eq!(job.stac_source, Some(pin));
        assert_eq!(job.status, JobStatus::Cancelled);
        assert!(job.output_path.is_none());
    }

    #[tokio::test]
    async fn changed_saved_project_metadata_is_not_accepted_on_restart() {
        let dir = tempfile::tempdir().unwrap();
        let manager = JobManager::open(dir.path()).await.unwrap();
        let pin =
            stac::fixture_snapshot(manager.storage_root(), "https://example.com/original.tif");
        let project = manager.save_stac_project(save(&pin, None)).await.unwrap();
        let path = manager.storage_root().join("projects.json");
        drop(manager);
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        value[&project.id]["stacItems"][0]["title"] = serde_json::json!("Changed title");
        std::fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(JobManager::open(dir.path()).await.is_err());
    }

    #[tokio::test]
    async fn repeated_catalog_ids_from_distinct_snapshots_do_not_reuse_wrong_download() {
        let dir = tempfile::tempdir().unwrap();
        let manager = JobManager::open(dir.path()).await.unwrap();
        let first = stac::fixture_snapshot(manager.storage_root(), "https://example.com/first.tif");
        let second =
            stac::fixture_snapshot(manager.storage_root(), "https://example.com/second.tif");
        let mut request = save(&first, None);
        request.selections.push(second.clone());
        let project = manager.save_stac_project(request).await.unwrap();
        assert_eq!(project.stac_items[0].item_id, project.stac_items[1].item_id);
        let permit = manager.inner.permits.acquire_many(2).await.unwrap();
        let result = manager
            .download_stac_project(DownloadRequest {
                project_id: project.id,
                selections: None,
            })
            .await
            .unwrap();
        assert_eq!(result.jobs.len(), 2);
        assert_ne!(result.jobs[0].id, result.jobs[1].id);
        assert_ne!(result.jobs[0].href, result.jobs[1].href);
        for job in &result.jobs {
            manager.cancel(&job.id).await.unwrap();
            manager.wait(&job.id).await.unwrap();
        }
        drop(permit);
        // The ordinary provider endpoint must not become an arbitrary-URL shortcut.
        assert!(manager
            .create(CreateJobRequest {
                item_id: "custom".into(),
                asset_key: "stac_asset".into(),
                href: "https://example.com/first.tif".into(),
                media_type: "image/tiff".into(),
                title: None
            })
            .await
            .is_err());
        assert!(crate::validate_request(&CreateJobRequest {
            item_id: "S2A_32UQD_20240108_0_L2A".into(),
            asset_key: "stac_asset".into(),
            href: format!("https://{}/sentinel-s2-l2a-cogs/32/U/QD/2024/1/S2A_32UQD_20240108_0_L2A/SCL.tif", crate::SOURCE_HOST),
            media_type: "image/tiff".into(), title: None,
        },None).is_err());
    }

    #[tokio::test]
    async fn missing_completed_original_is_requeued_without_restarting_runtime() {
        let dir = tempfile::tempdir().unwrap();
        let manager = JobManager::open(dir.path()).await.unwrap();
        let pin =
            stac::fixture_snapshot(manager.storage_root(), "https://example.com/original.tif");
        let project = manager.save_stac_project(save(&pin, None)).await.unwrap();
        let permit = manager.inner.permits.acquire_many(2).await.unwrap();
        let request = || DownloadRequest {
            project_id: project.id.clone(),
            selections: None,
        };
        let queued = manager.download_stac_project(request()).await.unwrap();
        let id = queued.jobs[0].id.clone();
        manager.cancel(&id).await.unwrap();
        manager.wait(&id).await.unwrap();
        {
            let mut store = manager.inner.store.lock().await;
            let job = store.jobs.get_mut(&id).unwrap();
            job.status = JobStatus::Succeeded;
            job.output_path = Some(
                manager
                    .storage_root()
                    .join("assets")
                    .join(format!("{id}.tif"))
                    .to_string_lossy()
                    .into_owned(),
            );
            job.sha256 = Some("a".repeat(64));
            manager.persist(&store.jobs).await.unwrap();
        }
        let replacement = manager.download_stac_project(request()).await.unwrap();
        assert_ne!(replacement.jobs[0].id, id);
        assert_eq!(manager.get(&id).await.unwrap().status, JobStatus::Failed);
        manager.cancel(&replacement.jobs[0].id).await.unwrap();
        manager.wait(&replacement.jobs[0].id).await.unwrap();
        drop(permit);
    }

    #[tokio::test]
    async fn retry_does_not_duplicate_a_replacement_transfer() {
        let dir = tempfile::tempdir().unwrap();
        let manager = JobManager::open(dir.path()).await.unwrap();
        let pin =
            stac::fixture_snapshot(manager.storage_root(), "https://example.com/original.tif");
        let project = manager.save_stac_project(save(&pin, None)).await.unwrap();
        let permit = manager.inner.permits.acquire_many(2).await.unwrap();
        let request = || DownloadRequest {
            project_id: project.id.clone(),
            selections: None,
        };
        let first = manager
            .download_stac_project(request())
            .await
            .unwrap()
            .jobs
            .remove(0);
        manager.cancel(&first.id).await.unwrap();
        manager.wait(&first.id).await.unwrap();
        let next = manager
            .download_stac_project(request())
            .await
            .unwrap()
            .jobs
            .remove(0);
        let error = manager.retry(&first.id).await.unwrap_err();
        assert!(error.contains("already has"));
        assert_eq!(manager.get(&first.id).await.unwrap().attempts, 1);
        assert_eq!(
            manager.get(&first.id).await.unwrap().status,
            JobStatus::Cancelled
        );
        assert_eq!(manager.inner.store.lock().await.active.len(), 1);
        manager.cancel(&next.id).await.unwrap();
        manager.wait(&next.id).await.unwrap();
        drop(permit);
    }

    #[tokio::test]
    async fn reuse_checks_original_sha_and_recovers_same_size_corruption() {
        use sha2::{Digest, Sha256};
        let dir = tempfile::tempdir().unwrap();
        let manager = JobManager::open(dir.path()).await.unwrap();
        let pin =
            stac::fixture_snapshot(manager.storage_root(), "https://example.com/original.tif");
        let project = manager.save_stac_project(save(&pin, None)).await.unwrap();
        let permit = manager.inner.permits.acquire_many(2).await.unwrap();
        let request = || DownloadRequest {
            project_id: project.id.clone(),
            selections: None,
        };
        let first = manager
            .download_stac_project(request())
            .await
            .unwrap()
            .jobs
            .remove(0);
        manager.cancel(&first.id).await.unwrap();
        manager.wait(&first.id).await.unwrap();
        let bytes = b"II*\0original transfer bytes";
        let path = manager
            .storage_root()
            .join("assets")
            .join(format!("{}.tif", first.id));
        std::fs::write(&path, bytes).unwrap();
        {
            let mut store = manager.inner.store.lock().await;
            let job = store.jobs.get_mut(&first.id).unwrap();
            job.status = JobStatus::Succeeded;
            job.bytes_downloaded = bytes.len() as u64;
            job.total_bytes = Some(bytes.len() as u64);
            job.output_path = Some(path.to_string_lossy().into_owned());
            job.sha256 = Some(format!("{:x}", Sha256::digest(bytes)));
            manager.persist(&store.jobs).await.unwrap();
        }
        // Transfer reuse checks bytes, not whether a generic TIFF decoder supports the source.
        let reused = manager.download_stac_project(request()).await.unwrap();
        assert_eq!(reused.jobs[0].id, first.id);
        let mut changed = bytes.to_vec();
        changed[6] ^= 1;
        std::fs::write(&path, changed).unwrap();
        let replacement = manager.download_stac_project(request()).await.unwrap();
        assert_ne!(replacement.jobs[0].id, first.id);
        assert_eq!(
            manager.get(&first.id).await.unwrap().status,
            JobStatus::Failed
        );
        assert!(manager
            .get(&first.id)
            .await
            .unwrap()
            .error
            .unwrap()
            .contains("SHA-256"));
        manager.cancel(&replacement.jobs[0].id).await.unwrap();
        manager.wait(&replacement.jobs[0].id).await.unwrap();
        drop(permit);
    }

    #[tokio::test]
    async fn catalog_refresh_keeps_existing_pin_and_queues_that_same_original() {
        use sha2::{Digest, Sha256};
        let dir = tempfile::tempdir().unwrap();
        let manager = JobManager::open(dir.path()).await.unwrap();
        let pin =
            stac::fixture_snapshot(manager.storage_root(), "https://example.com/original.tif");
        let path = manager
            .storage_root()
            .join("stac")
            .join(format!("snapshot-{}.json", pin.snapshot_id));
        let mut record: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        record["retrievedAt"] = serde_json::json!("2026-10-03T00:00:00Z");
        let bytes = serde_json::to_vec(&record).unwrap();
        let refreshed = Selection {
            snapshot_id: format!("{:x}", Sha256::digest(&bytes)),
            asset_key: pin.asset_key.clone(),
        };
        std::fs::write(
            manager
                .storage_root()
                .join("stac")
                .join(format!("snapshot-{}.json", refreshed.snapshot_id)),
            bytes,
        )
        .unwrap();
        assert_ne!(pin.snapshot_id, refreshed.snapshot_id);
        let project = manager.save_stac_project(save(&pin, None)).await.unwrap();
        let updated = manager
            .save_stac_project(save(&refreshed, Some(project.id.clone())))
            .await
            .unwrap();
        assert_eq!(updated.stac_items, project.stac_items);
        let permit = manager.inner.permits.acquire_many(2).await.unwrap();
        let downloads = manager
            .download_stac_project(DownloadRequest {
                project_id: project.id,
                selections: Some(vec![refreshed]),
            })
            .await
            .unwrap();
        assert_eq!(downloads.jobs[0].stac_source.as_ref(), Some(&pin));
        manager.cancel(&downloads.jobs[0].id).await.unwrap();
        manager.wait(&downloads.jobs[0].id).await.unwrap();
        drop(permit);
    }
}
