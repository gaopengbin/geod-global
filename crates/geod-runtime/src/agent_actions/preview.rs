//! Private desktop map preview. Reads a pinned review; never approves or starts work.
use super::*;
use std::collections::BTreeMap;

impl JobManager {
    pub async fn agent_plan_map_preview(
        &self,
        session: &str,
        id: &str,
        hash: &str,
    ) -> Result<Value> {
        let plan: Plan = read_record(&self.inner.root, "plans", id).await?;
        plan.validate_reference(session, id)?;
        if plan.hash != hash
            || revision::replacement(&self.inner.root, &plan)
                .await?
                .is_some()
        {
            return Err("The review changed. Reload the native plan before previewing.".into());
        }
        let mut selections: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        let (provider, bounds, geometry) = match &plan.action {
            Action::Download {
                query,
                files,
                project,
                ..
            } => {
                for file in files {
                    selections
                        .entry(file.request.item_id.clone())
                        .or_default()
                        .insert(file.request.asset_key.clone(), file.request.href.clone());
                }
                let geometry = if let Some(pin) = project {
                    let projects = self.inner.projects.lock().await;
                    pin.verify(projects.get(&pin.id))?;
                    projects[&pin.id].geometry.clone()
                } else {
                    None
                };
                (query.provider.clone(), query.bounds, geometry)
            }
            Action::Project { request, .. } => {
                let mut provider = None;
                for scene in &request.scenes {
                    for (key, asset) in &scene.assets {
                        let source = catalog::provider(&crate::CreateJobRequest {
                            item_id: scene.item_id.clone(),
                            asset_key: key.clone(),
                            href: asset.href.clone(),
                            media_type: asset.media_type.clone(),
                            title: None,
                        })?;
                        if provider.as_ref().is_some_and(|previous| previous != source) {
                            return Err("Map preview requires scenes from one data source.".into());
                        }
                        provider = Some(source.to_string());
                        selections
                            .entry(scene.item_id.clone())
                            .or_default()
                            .insert(key.clone(), asset.href.clone());
                    }
                }
                (
                    provider.ok_or("This review has no imagery to preview.")?,
                    request.bounds,
                    request.geometry.clone(),
                )
            }
            _ => return Err("This review has no online imagery preview.".into()),
        };
        catalog::source(&provider)?;
        Ok(
            json!({"planId":plan.id,"planHash":plan.hash,"provider":provider,"bounds":bounds,"geometry":geometry,
            "selections":selections.into_iter().map(|(item_id,assets)|json!({"itemId":item_id,"assets":assets})).collect::<Vec<_>>()}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn preview_reads_exact_pinned_files_without_approving_or_transferring() {
        let root = std::env::temp_dir().join(format!("geod-plan-preview-{}", Uuid::new_v4()));
        let manager = JobManager::open(root.clone()).await.unwrap();
        let session = Uuid::new_v4().to_string();
        let review = manager.save_agent_plan(&session, Action::Download {
            acquisition: None,
            query: SearchQuery { provider:"earth-search".into(), bounds:[-74.3,40.4,-73.7,41.0], start:"2026-10-01".into(),end:"2026-10-07".into(),cloud_max:100.0,limit:5 },
            metadata_sha256:"a".repeat(64),project:None,
            files:vec![DownloadFile { request:crate::CreateJobRequest {
                item_id:"S2A_10SEG_20250605_0_L2A".into(),asset_key:"visual".into(),
                href:"https://sentinel-cogs.s3.us-west-2.amazonaws.com/sentinel-s2-l2a-cogs/10/S/EG/2025/6/S2A_10SEG_20250605_0_L2A/TCI.tif".into(),
                media_type:"image/tiff; application=geotiff; profile=cloud-optimized".into(),title:None },date:"2025-06-05T00:00:00Z".into(),pin:Some(RemotePin {bytes:100,etag:"\"source\"".into()}) }],
        }).await.unwrap();
        let id = review["planId"].as_str().unwrap();
        let hash = review["planHash"].as_str().unwrap();
        let view = manager
            .agent_plan_map_preview(&session, id, hash)
            .await
            .unwrap();
        assert_eq!(view["bounds"], review["bounds"]);
        assert_eq!(view["selections"][0]["itemId"], "S2A_10SEG_20250605_0_L2A");
        assert!(view["selections"][0]["assets"]["visual"]
            .as_str()
            .unwrap()
            .ends_with("/TCI.tif"));
        assert!(manager
            .agent_plan_map_preview(&Uuid::new_v4().to_string(), id, hash)
            .await
            .is_err());
        assert!(manager
            .agent_plan_map_preview(&session, id, &"0".repeat(64))
            .await
            .is_err());
        assert_eq!(
            manager.agent_plan_status(&session, id).await.unwrap()["status"],
            "pending"
        );
        assert!(manager.inner.store.lock().await.jobs.is_empty());
        manager.shutdown().await.unwrap();
        drop(manager);
        assert!(
            root.starts_with(std::env::temp_dir())
                && root
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("geod-plan-preview-")
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
