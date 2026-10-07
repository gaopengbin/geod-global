//! Review cards for the existing pinned scientific processing engine.
use super::*;
use crate::raster::reflectance::composite::scientific;
#[cfg(test)]
mod tests;

pub(super) fn validate(
    spec: &crate::RgbSpec,
    raw: u64,
    disk: u64,
    project: &Option<ProjectPin>,
) -> Result<()> {
    scientific::validate_spec(spec)?;
    let expected = u64::from(spec.grid.width) * u64::from(spec.grid.height) * 6;
    if raw != expected || disk < raw || project.as_ref().map(|p| &p.id) != spec.project_id.as_ref()
    {
        return Err("Scientific RGB output or project pin changed. Create a new plan.".into());
    }
    if let Some(pin) = project {
        pin.validate()?;
    }
    Ok(())
}
pub(super) fn selection(
    spec: &crate::mosaic::MosaicSpec,
) -> Option<crate::mosaic::vegetation::Request> {
    spec.vi_selection
        .as_ref()
        .map(|s| crate::mosaic::vegetation::Request { policy: s.policy })
}

pub(super) fn summary(action: &Action) -> Result<Option<Value>> {
    let result = match action {
        Action::Rgb {
            spec,
            raw_bytes,
            required_disk_bytes,
            ..
        } => {
            let quality = spec.quality_mask.as_ref().map(|mask| {
                let (product, policy, snow) = match mask {
                    crate::QualityMaskSpec::Modis(m) => ("modis-09a1-v061", serde_json::to_value(m.policy).map_err(io_error)?, m.exclude_snow),
                    crate::QualityMaskSpec::Landsat(m) => ("landsat-c2-l2", serde_json::to_value(m.policy).map_err(io_error)?, m.exclude_snow),
                };
                let source_pins = mask.sources().iter().map(|s| &s.pin).collect::<Vec<_>>();
                Ok::<_, String>(json!({"product":product,"policy":policy,"excludeSnow":snow,
                    "coupled":mask.coupled().is_some(),"sceneCount":mask.coupled().map_or(1,|c|c.scenes.len()),
                    "sourceCount":spec.source_job_ids().len(),
                    "sourceSha256":digest(&serde_json::to_vec(&(source_pins,mask.coupled())).map_err(io_error)?)}))
            }).transpose()?;
            json!({"product":spec.profile.product,"dataType":if spec.profile.signed {"Int16"}else{"UInt16"},
                "channels":3,"rawBytes":raw_bytes,"requiredDiskBytes":required_disk_bytes,"quality":quality})
        }
        Action::Mosaic { spec, output, .. } if spec.vi_selection.is_some() => {
            let selection = spec.vi_selection.as_ref().unwrap();
            json!({"product":selection.product,"dataType":"Int16","channels":1,
                "rawBytes":u64::from(output.width)*u64::from(output.height)*2,
                "requiredDiskBytes":null,
                "quality":{"product":selection.product,"policy":selection.policy,"excludeSnow":true,"coupled":true,
                    "sceneCount":selection.scenes.len(),"sourceCount":selection.scenes.len()*4,
                    "sourceSha256":crate::mosaic::vegetation::spec_hash(selection)?}})
        }
        _ => return Ok(None),
    };
    Ok(Some(result))
}

impl JobManager {
    pub async fn agent_scientific_rgb_plan(
        &self,
        session: &str,
        request: crate::RgbRequest,
    ) -> Result<Value> {
        if !uuid(session) {
            return Err("Invalid Agent conversation ID.".into());
        }
        let output = self.plan_scientific_rgb(request).await?;
        {
            let store = self.inner.store.lock().await;
            if output
                .spec
                .source_job_ids()
                .iter()
                .any(|id| store.active.contains_key(*id))
            {
                return Err(
                    "Wait for every source task to settle before planning processing".into(),
                );
            }
        }
        let project = if let Some(id) = &output.spec.project_id {
            let projects = self.inner.projects.lock().await;
            Some(ProjectPin::from_project(
                projects.get(id).ok_or("Unknown RGB project")?,
            )?)
        } else {
            None
        };
        self.save_agent_plan(
            session,
            Action::Rgb {
                spec: output.spec,
                raw_bytes: output.raw_bytes,
                required_disk_bytes: output.required_disk_bytes,
                project,
            },
        )
        .await
    }
}
