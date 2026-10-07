//! Human form edits create a new native review, never an executable model tool.
use super::*;
use std::collections::BTreeSet;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PlanRevision {
    Vector {
        bounds: [f64; 4],
        name: String,
        keep_polygon: bool,
    },
    Clip {
        bounds: [f64; 4],
        name: String,
        keep_polygon: bool,
    },
    Download {
        item_ids: Vec<String>,
    },
    Project {
        item_ids: Vec<String>,
        name: Option<String>,
        bounds: Option<[f64; 4]>,
        keep_polygon: bool,
    },
    Mosaic {
        asset_key: String,
        quality_policy: Option<crate::mosaic::vegetation::Policy>,
    },
    Rgb {
        name: String,
        quality_policy: Option<String>,
        exclude_snow: Option<bool>,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Replacement {
    session_id: String,
    original_hash: String,
    parameters_hash: String,
    pub plan_id: String,
    plan_hash: String,
}

pub(super) async fn replacement(root: &Path, original: &Plan) -> Result<Option<Replacement>> {
    let path = record_dir(root, "revisions")
        .await?
        .join(format!("{}.json", original.id));
    match tokio::fs::symlink_metadata(path).await {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error(error)),
        Ok(_) => (),
    }
    let receipt: Replacement = read_record(root, "revisions", &original.id).await?;
    let next: Plan = read_record(root, "plans", &receipt.plan_id).await?;
    next.validate_reference(&original.session_id, &receipt.plan_id)?;
    if receipt.session_id != original.session_id
        || receipt.original_hash != original.hash
        || receipt.plan_hash != next.hash
        || receipt.plan_id == original.id
        || receipt.parameters_hash.len() != 64
        || !receipt
            .parameters_hash
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err("The revised review record changed. Reload the conversation.".into());
    }
    Ok(Some(receipt))
}

fn selected(ids: Vec<String>, allowed: impl Iterator<Item = String>) -> Result<BTreeSet<String>> {
    let allowed: BTreeSet<_> = allowed.collect();
    let count = ids.len();
    let ids: BTreeSet<_> = ids.into_iter().collect();
    if ids.is_empty() || ids.len() != count || !ids.is_subset(&allowed) {
        return Err("Select distinct scenes from this native review.".into());
    }
    Ok(ids)
}

fn preserve_project_scope(original: &Action, revised: &Action) -> Result<()> {
    let same = match (original, revised) {
        (
            Action::Mosaic {
                project_hash: a, ..
            },
            Action::Mosaic {
                project_hash: b, ..
            },
        ) => a == b,
        (Action::Rgb { project: a, .. }, Action::Rgb { project: b, .. }) => {
            serde_json::to_value(a).map_err(io_error)?
                == serde_json::to_value(b).map_err(io_error)?
        }
        (Action::StacProject { target: a, .. }, Action::StacProject { target: b, .. }) => {
            serde_json::to_value(a).map_err(io_error)?
                == serde_json::to_value(b).map_err(io_error)?
        }
        (Action::StacDownload { project: a, .. }, Action::StacDownload { project: b, .. }) => {
            a.hash == b.hash && a.id == b.id
        }
        (Action::WcsProject { target: a, .. }, Action::WcsProject { target: b, .. }) => {
            serde_json::to_value(a).map_err(io_error)?
                == serde_json::to_value(b).map_err(io_error)?
        }
        (Action::WcsDownload { project: a, .. }, Action::WcsDownload { project: b, .. }) => {
            a.hash == b.hash && a.id == b.id
        }
        _ => true,
    };
    if !same {
        return Err("The saved project changed during review. Create a new plan.".into());
    }
    Ok(())
}

impl JobManager {
    async fn editable_plan(&self, session: &str, id: &str, hash: &str) -> Result<Plan> {
        let summary = self.agent_plan_status(session, id).await?;
        if summary["planHash"].as_str() != Some(hash)
            || !matches!(summary["status"].as_str(), Some("pending" | "expired"))
        {
            return Err("Only this conversation's unsubmitted review can be edited.".into());
        }
        let plan: Plan = read_record(&self.inner.root, "plans", id).await?;
        plan.validate_reference(session, id)?;
        if plan.hash != hash {
            return Err("The review changed. Reload the native plan before editing.".into());
        }
        Ok(plan)
    }

    /// Paths, URLs, pins and polygon vertices stay native; the form cannot
    /// substitute another source or credential.
    pub async fn agent_plan_revision_draft(
        &self,
        session: &str,
        id: &str,
        hash: &str,
    ) -> Result<Value> {
        let plan = self.editable_plan(session, id, hash).await?;
        if matches!(plan.action, Action::Vector { .. }) {
            return vector::draft(&plan);
        }
        if matches!(
            plan.action,
            Action::StacProject { .. } | Action::StacDownload { .. }
        ) {
            return custom::revision_draft(self, &plan).await;
        }
        if matches!(
            plan.action,
            Action::WcsProject { .. } | Action::WcsDownload { .. }
        ) {
            return coverage::revision_draft(self, &plan).await;
        }
        let mut draft = json!({"planId":id,"planHash":hash,"fields":{}});
        match &plan.action {
            Action::Clip { recipe, output } => {
                draft["parameters"] = json!({"kind":"clip","bounds":recipe.operation.bounds,"name":recipe.name,"keepPolygon":recipe.operation.geometry.is_some()});
                draft["fields"] = json!({"name":true,"bounds":true,"polygon":recipe.operation.geometry.is_some()});
                draft["boundsCrs"] = json!(if recipe.operation.crs == "source" {
                    &output.crs
                } else {
                    &recipe.operation.crs
                });
            }
            Action::Download { files, project, .. } => {
                draft["parameters"] = json!({"kind":"download","itemIds":files.iter().map(|f| &f.request.item_id).collect::<Vec<_>>()});
                draft["fields"]["items"] = json!(files.iter().map(|f|json!({"id":f.request.item_id,"date":f.date,"assetKey":f.request.asset_key,"locked":false})).collect::<Vec<_>>());
                draft["projectId"] = json!(project.as_ref().map(|p| &p.id));
            }
            Action::Project {
                request, target, ..
            } => {
                let projects = self.inner.projects.lock().await;
                let locked: BTreeSet<_> = if let Some(pin) = target {
                    pin.verify(projects.get(&pin.id))?;
                    projects[&pin.id]
                        .scenes
                        .iter()
                        .map(|s| s.item_id.as_str())
                        .collect()
                } else {
                    BTreeSet::new()
                };
                draft["parameters"] = json!({"kind":"project","itemIds":request.scenes.iter().map(|s| &s.item_id).collect::<Vec<_>>(),"name":target.is_none().then_some(&request.name),"bounds":target.is_none().then_some(request.bounds),"keepPolygon":request.geometry.is_some()});
                draft["fields"] = json!({"name":target.is_none(),"bounds":target.is_none(),"polygon":target.is_none() && request.geometry.is_some(),"items":request.scenes.iter().map(|s|json!({"id":s.item_id,"date":s.date,"locked":locked.contains(s.item_id.as_str())})).collect::<Vec<_>>()});
                draft["boundsCrs"] = json!("EPSG:4326");
                draft["projectId"] = json!(target.as_ref().map(|p| &p.id));
            }
            Action::Mosaic { project, spec, .. } => {
                let keys: BTreeSet<_> = project
                    .scenes
                    .iter()
                    .flat_map(|s| s.assets.keys())
                    .filter(|k| crate::providers::SOURCE_ASSET_KEYS.contains(&k.as_str()))
                    .filter(|k| spec.vi_selection.is_none() || matches!(k.as_str(), "ndvi" | "evi"))
                    .collect();
                draft["parameters"] = json!({"kind":"mosaic","assetKey":spec.asset_key,"qualityPolicy":spec.vi_selection.as_ref().map(|s|s.policy)});
                draft["fields"]["assetKeys"] = json!(keys);
                if spec.vi_selection.is_some() {
                    draft["fields"]["qualityPolicies"] = json!(["good", "usable"]);
                }
                draft["projectId"] = json!(project.id);
            }
            Action::Rgb { spec, .. } => {
                let quality = spec
                    .request()
                    .quality_mask
                    .map(serde_json::to_value)
                    .transpose()
                    .map_err(io_error)?;
                draft["parameters"] = json!({"kind":"rgb","name":spec.name,"qualityPolicy":quality.as_ref().map(|q|&q["policy"]),"excludeSnow":quality.as_ref().map(|q|&q["excludeSnow"])});
                draft["fields"] = json!({"name":true,"snow":quality.is_some()});
                if quality.is_some() {
                    draft["fields"]["qualityPolicies"] = if spec.profile.product == "landsat-c2-l2"
                    {
                        json!(["cloud_free", "cloud_free_conservative"])
                    } else {
                        json!(["clear", "clear_best"])
                    };
                }
                draft["projectId"] = json!(spec.project_id);
            }
            Action::WcsProject { .. }
            | Action::WcsDownload { .. }
            | Action::StacProject { .. }
            | Action::StacDownload { .. }
            | Action::Vector { .. } => {
                unreachable!()
            }
        }
        draft["kind"] = draft["parameters"]["kind"].clone();
        Ok(draft)
    }

    /// Serialized with approval. A duplicate save returns the same new review,
    /// including after restart; the predecessor can no longer be submitted.
    pub async fn revise_agent_plan(
        &self,
        session: &str,
        id: &str,
        hash: &str,
        parameters: PlanRevision,
    ) -> Result<Value> {
        let _commit = self.inner.agent_commits.lock().await;
        let original: Plan = read_record(&self.inner.root, "plans", id).await?;
        original.validate_reference(session, id)?;
        if original.hash != hash {
            return Err("The native review hash changed.".into());
        }
        let parameters_hash = digest(&serde_json::to_vec(&parameters).map_err(io_error)?);
        if let Some(previous) = replacement(&self.inner.root, &original).await? {
            if previous.parameters_hash != parameters_hash {
                return Err("This review was already corrected. Edit its replacement.".into());
            }
            return self.agent_plan_status(session, &previous.plan_id).await;
        }
        let plan = self.editable_plan(session, id, hash).await?;
        let next = match (plan.action, parameters) {
            (action @ (Action::WcsProject { .. } | Action::WcsDownload { .. }), parameters) => {
                coverage::revise(self, session, action, parameters).await?
            }
            (
                Action::Vector { scope },
                PlanRevision::Vector {
                    bounds,
                    name,
                    keep_polygon,
                },
            ) => vector::revise(self, session, scope, bounds, name, keep_polygon).await?,
            (action @ (Action::StacProject { .. } | Action::StacDownload { .. }), parameters) => {
                custom::revise(self, session, action, parameters).await?
            }
            (
                Action::Clip { mut recipe, .. },
                PlanRevision::Clip {
                    bounds,
                    name,
                    keep_polygon,
                },
            ) => {
                recipe.name = name.trim().into();
                recipe.operation.bounds = bounds;
                if keep_polygon && recipe.operation.geometry.is_none() {
                    return Err("The review has no saved polygon.".into());
                }
                if !keep_polygon {
                    recipe.operation.geometry = None;
                    recipe.schema_version = crate::processing::RECIPE_SCHEMA_VERSION.into();
                }
                self.agent_recipe_review_plan(session, recipe).await?
            }
            (
                Action::Download {
                    query,
                    metadata_sha256,
                    files,
                    project,
                    mut acquisition,
                },
                PlanRevision::Download { item_ids },
            ) => {
                let ids = selected(item_ids, files.iter().map(|f| f.request.item_id.clone()))?;
                if let Some(pin) = &project {
                    pin.verify(self.inner.projects.lock().await.get(&pin.id))?;
                }
                let mut files: Vec<_> = files
                    .into_iter()
                    .filter(|f| ids.contains(&f.request.item_id))
                    .collect();
                if let Some(scope) = &mut acquisition {
                    scope.scenes.retain(|s| ids.contains(&s.0));
                    footprint::complete(scope)?;
                }
                for file in &mut files {
                    file.pin = download_preflight(self, &file.request).await?;
                }
                self.save_agent_plan(
                    session,
                    Action::Download {
                        acquisition,
                        query,
                        metadata_sha256,
                        files,
                        project,
                    },
                )
                .await?
            }
            (
                Action::Project {
                    mut request,
                    target,
                    metadata_sha256,
                },
                PlanRevision::Project {
                    item_ids,
                    name,
                    bounds,
                    keep_polygon,
                },
            ) => {
                let ids = selected(item_ids, request.scenes.iter().map(|s| s.item_id.clone()))?;
                if let Some(pin) = &target {
                    let projects = self.inner.projects.lock().await;
                    pin.verify(projects.get(&pin.id))?;
                    let previous = &projects[&pin.id];
                    if name.is_some()
                        || bounds.is_some()
                        || keep_polygon != request.geometry.is_some()
                        || previous.scenes.iter().any(|s| !ids.contains(&s.item_id))
                        || ids.len() <= previous.scenes.len()
                    {
                        return Err("Keep the saved project area and scenes; select at least one scene to append.".into());
                    }
                } else {
                    request.name = crate::projects::validate_project_name(
                        name.as_deref().ok_or("Name the project.")?,
                    )?;
                    request.bounds = bounds.ok_or("Set the project area.")?;
                    if keep_polygon && request.geometry.is_none() {
                        return Err("The review has no saved polygon.".into());
                    }
                    if !keep_polygon {
                        request.geometry = None;
                    }
                }
                request.scenes.retain(|s| ids.contains(&s.item_id));
                request.validate(None)?;
                footprint::complete(&footprint::project_scope(&request))?;
                self.save_agent_plan(
                    session,
                    Action::Project {
                        request,
                        target,
                        metadata_sha256,
                    },
                )
                .await?
            }
            (
                Action::Mosaic {
                    project,
                    project_hash,
                    spec,
                    ..
                },
                PlanRevision::Mosaic {
                    asset_key,
                    quality_policy,
                },
            ) => {
                let current = self
                    .inner
                    .projects
                    .lock()
                    .await
                    .get(&project.id)
                    .cloned()
                    .ok_or("The project was removed.")?;
                if ProjectScope::from_project(&current).fingerprint()? != project_hash {
                    return Err("The saved project changed. Create a new plan.".into());
                }
                if quality_policy.is_some() && spec.vi_selection.is_none() {
                    return Err("This review has no vegetation quality selection.".into());
                }
                let selection =
                    quality_policy.map(|policy| crate::mosaic::vegetation::Request { policy });
                self.agent_project_mosaic_plan_with_selection(
                    session,
                    &project.id,
                    &asset_key,
                    selection,
                )
                .await?
            }
            (
                Action::Rgb { spec, project, .. },
                PlanRevision::Rgb {
                    name,
                    quality_policy,
                    exclude_snow,
                },
            ) => {
                if let Some(pin) = &project {
                    pin.verify(self.inner.projects.lock().await.get(&pin.id))?;
                }
                let mut request = spec.request();
                request.name = Some(name.trim().into());
                if let Some(mask) = request.quality_mask.take() {
                    let mut value = serde_json::to_value(mask).map_err(io_error)?;
                    value["policy"] = json!(quality_policy.ok_or("Choose a quality policy.")?);
                    value["excludeSnow"] = json!(exclude_snow.ok_or("Choose the snow policy.")?);
                    request.quality_mask = Some(
                        serde_json::from_value(value).map_err(|_| "Unsupported quality policy.")?,
                    );
                } else if quality_policy.is_some() || exclude_snow.is_some() {
                    return Err("This review has no matched quality source files.".into());
                }
                self.agent_scientific_rgb_plan(session, request).await?
            }
            _ => return Err("The form belongs to a different review kind.".into()),
        };
        // Ordinary project edits do not hold agent_commits. Re-check the
        // immutable result too: a preflight must not silently re-pin a project
        // modified between our initial check and the core's subsequent read.
        let saved: Plan = read_record(
            &self.inner.root,
            "plans",
            next["planId"].as_str().ok_or("Missing revised plan ID")?,
        )
        .await?;
        saved.validate_reference(
            session,
            next["planId"].as_str().ok_or("Missing revised plan ID")?,
        )?;
        preserve_project_scope(&original.action, &saved.action)?;
        let receipt = Replacement {
            session_id: session.into(),
            original_hash: hash.into(),
            parameters_hash,
            plan_id: next["planId"]
                .as_str()
                .ok_or("Missing revised plan ID")?
                .into(),
            plan_hash: next["planHash"]
                .as_str()
                .ok_or("Missing revised plan hash")?
                .into(),
        };
        write_record(&self.inner.root, "revisions", id, &receipt).await?;
        Ok(next)
    }
}
