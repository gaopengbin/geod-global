//! Deterministic reviewed snapshots. Approval lives in the atomic vector registry.
//! A bounded pending record permits checksum-verified recovery after file publication.
use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VectorApproval {
    pub plan_id: String,
    pub plan_hash: String,
    pub session_id: String,
    pub approved_at: String,
    pub policy: String,
}
impl VectorApproval {
    pub(crate) fn validate(&self) -> Result<()> {
        let uuid = |s: &String| Uuid::parse_str(s).is_ok_and(|id| id.to_string() == *s);
        if !uuid(&self.plan_id)
            || !uuid(&self.session_id)
            || self.plan_hash.len() != 64
            || !self
                .plan_hash
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            || self.policy != "geod-agent-review/v2"
            || chrono::DateTime::parse_from_rfc3339(&self.approved_at).is_err()
        {
            return Err("Invalid vector approval receipt".into());
        }
        Ok(())
    }
    fn same_review(&self, expected: &Self) -> bool {
        self.plan_id == expected.plan_id
            && self.plan_hash == expected.plan_hash
            && self.session_id == expected.session_id
            && self.policy == expected.policy
    }
}
fn pending(root: &Path, id: &str) -> PathBuf {
    root.join("agent-vector-pending").join(format!("{id}.json"))
}
fn original(root: &Path, id: &str) -> PathBuf {
    root.join("vectors").join(format!("{id}.json"))
}
fn check_pending(root: &Path) -> Result<()> {
    let directory = root.join("agent-vector-pending");
    if directory.canonicalize().map_err(io_error)? != directory {
        return Err("Vector approval recovery storage was redirected".into());
    }
    Ok(())
}
impl JobManager {
    pub(crate) async fn import_feature_snapshot(
        &self,
        snapshot: crate::features::Snapshot,
        review: Option<(String, VectorApproval, String)>,
    ) -> Result<VectorAsset> {
        let Some((id, approval, title)) = review else {
            return match snapshot {
                crate::features::Snapshot::Features(request, source) => {
                    self.import_vector_source(request, Some(source)).await
                }
                crate::features::Snapshot::Wfs(bytes, source) => {
                    self.import_wfs_source(bytes, source).await
                }
                crate::features::Snapshot::Osm(bytes, source) => {
                    self.import_osm_source(bytes, source).await
                }
            };
        };
        let (bytes, mut asset) = match snapshot {
            crate::features::Snapshot::Features(request, source) => {
                let mut asset = initial(&request.name, "managed")?;
                asset.remote_source = Some(source);
                (request.text.into_bytes(), asset)
            }
            crate::features::Snapshot::Wfs(bytes, source) => {
                let mut asset = initial(
                    &format!(
                        "{} · WFS",
                        source
                            .collection_title
                            .chars()
                            .take(110)
                            .collect::<String>()
                    ),
                    "managed",
                )?;
                asset.remote_source = Some(source);
                (bytes, asset)
            }
            crate::features::Snapshot::Osm(bytes, source) => {
                let mut asset = initial(&format!("{} · OSM", source.preset_title), "managed")?;
                asset.osm_source = Some(source);
                asset.osm_conversion = Some(2);
                (bytes, asset)
            }
        };
        approval.validate()?;
        if Uuid::parse_str(&id).is_err() {
            return Err("Invalid reviewed vector ID".into());
        }
        asset.id = id.clone();
        asset.name = name(&title)?;
        asset.agent_approval = Some(approval);
        let _permit = self
            .inner
            .raster_permits
            .clone()
            .acquire_owned()
            .await
            .map_err(io_error)?;
        let (inspection, bytes) = tokio::task::spawn_blocking(move || {
            normalize(&bytes, asset).map(|value| (value, bytes))
        })
        .await
        .map_err(io_error)??;
        let path = original(&self.inner.root, &id);
        let record = Record {
            asset: inspection.asset.clone(),
            path: path.to_string_lossy().into_owned(),
        };
        let record_bytes = serde_json::to_vec(&record).map_err(io_error)?;
        if record_bytes.len() > 4 * 1024 * 1024 {
            return Err("Vector approval record exceeds its storage limit".into());
        }
        let root = self.inner.root.clone();
        let owned_id = id.clone();
        tokio::task::spawn_blocking(move || {
            use std::io::Write;
            let directory = root.join("vectors");
            if directory.canonicalize().map_err(io_error)? != directory {
                return Err("Vector storage was redirected".into());
            }
            let output = original(&root, &owned_id);
            if output.try_exists().map_err(io_error)? {
                return Err(
                    "Reviewed vector already has an unregistered source; recover its receipt first"
                        .into(),
                );
            }
            std::fs::create_dir_all(root.join("agent-vector-pending")).map_err(io_error)?;
            check_pending(&root)?;
            let marker = pending(&root, &owned_id);
            if marker.try_exists().map_err(io_error)? {
                storage::regular_file(&marker)?;
            }
            let mut receipt =
                tempfile::NamedTempFile::new_in(marker.parent().unwrap()).map_err(io_error)?;
            receipt.write_all(&record_bytes).map_err(io_error)?;
            receipt.as_file().sync_all().map_err(io_error)?;
            receipt.persist(&marker).map_err(io_error)?;
            let mut source = tempfile::NamedTempFile::new_in(directory).map_err(io_error)?;
            source.write_all(&bytes).map_err(io_error)?;
            source.as_file().sync_all().map_err(io_error)?;
            source.persist_noclobber(&output).map_err(io_error)?;
            Ok::<_, String>(())
        })
        .await
        .map_err(io_error)??;
        let result = self.register_vector(inspection, path.clone()).await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&path).await;
        }
        let _ = tokio::fs::remove_file(pending(&self.inner.root, &id)).await;
        result
    }
    /// Existing registry receipts win. Published unregistered originals can be
    /// recovered only with their exact matching review and native content hashes.
    pub(crate) async fn recover_reviewed_vector<F: Fn(&VectorAsset) -> Result<()>>(
        &self,
        id: &str,
        approval: &VectorApproval,
        verify: F,
    ) -> Result<Option<VectorAsset>> {
        if let Some(asset) = self.existing_reviewed_vector(id, approval).await? {
            verify(&asset)?;
            return Ok(Some(asset));
        }
        let root = self.inner.root.clone();
        let id = id.to_string();
        let expected = approval.clone();
        let ready = tokio::task::spawn_blocking(move || {
            let output = original(&root, &id);
            if !output.try_exists().map_err(io_error)? {
                return Ok(None);
            }
            check_pending(&root)?;
            let marker = pending(&root, &id);
            storage::regular_file(&marker)?;
            let bytes = read(&marker)?;
            if bytes.len() > 4 * 1024 * 1024 {
                return Err("Vector recovery record exceeds its storage limit".into());
            }
            let record: Record = serde_json::from_slice(&bytes).map_err(io_error)?;
            let a = record
                .asset
                .agent_approval
                .as_ref()
                .ok_or("Missing vector recovery approval")?;
            a.validate()?;
            if record.asset.id != id
                || !a.same_review(&expected)
                || record.asset.storage_mode != "managed"
            {
                return Err("Vector recovery identity or review changed".into());
            }
            let bytes = read_record_source(&root, &record)?;
            let inspection = normalize(&bytes, record.asset.clone())?;
            if inspection.asset != record.asset {
                return Err("Vector recovery original or conversion changed".into());
            }
            Ok::<_, String>(Some((inspection, output)))
        })
        .await
        .map_err(io_error)??;
        let Some((inspection, output)) = ready else {
            return Ok(None);
        };
        verify(&inspection.asset)?;
        let asset = self.register_vector(inspection, output).await?;
        let _ = tokio::fs::remove_file(pending(&self.inner.root, &asset.id)).await;
        Ok(Some(asset))
    }
    pub(crate) async fn existing_reviewed_vector(
        &self,
        id: &str,
        approval: &VectorApproval,
    ) -> Result<Option<VectorAsset>> {
        let asset = self
            .inner
            .vectors
            .lock()
            .await
            .get(id)
            .map(|r| r.asset.clone());
        if let Some(asset) = asset {
            if !asset
                .agent_approval
                .as_ref()
                .is_some_and(|a| a.same_review(approval))
            {
                return Err("Reviewed vector identity belongs to another approval".into());
            }
            return Ok(Some(self.inspect_vector(id).await?.asset));
        }
        Ok(None)
    }
}
