//! Native project/task association, before source addresses are redacted.
//! Saved selection metadata is not a claim that its files have been acquired.
use super::Project;
use crate::{Job, JobStatus};
use std::collections::HashSet;

pub(crate) fn related_ids(project: &Project, jobs: &[Job]) -> HashSet<String> {
    let sources: HashSet<String> = jobs
        .iter()
        .filter(|job| {
            job.kind == "download"
                && (project.scenes.iter().any(|scene| {
                    scene.item_id == job.item_id
                        && scene
                            .assets
                            .get(&job.asset_key)
                            .is_some_and(|asset| asset.href == job.href)
                }) || job.asset_key == "stac_asset"
                    && project.stac_items.iter().any(|item| {
                        job.stac_source.as_ref().is_some_and(|pin| {
                            pin.snapshot_id == item.snapshot_id
                                && pin.asset_key == item.asset_key
                                && job.href == item.href
                        })
                    })
                    || job.asset_key == "wcs_coverage"
                        && project.wcs_items.iter().any(|item| {
                            job.wcs_source.as_ref().is_some_and(|pin| {
                                pin.plan_id == item.plan_id && job.href == item.href
                            })
                        }))
        })
        .map(|job| job.id.clone())
        .collect();
    let mut outputs: HashSet<String> = jobs
        .iter()
        .filter(|job| {
            job.kind == "raster_mosaic"
                && job
                    .mosaic
                    .as_ref()
                    .is_some_and(|spec| spec.project_id == project.id)
                || job.kind == "raster_rgb"
                    && job
                        .rgb_spec
                        .as_ref()
                        .is_some_and(|spec| spec.project_id.as_deref() == Some(&project.id))
        })
        .map(|job| job.id.clone())
        .collect();
    loop {
        let mut changed = false;
        for job in jobs {
            let parent = job
                .parent_id
                .as_ref()
                .or_else(|| job.recipe.as_ref().map(|recipe| &recipe.source.job_id));
            let Some(parent) = parent else { continue };
            let prepared = job.kind == "raster_prepare"
                && sources.contains(parent)
                && jobs.iter().any(|source| {
                    source.id == *parent
                        && source.item_id == job.item_id
                        && source.href == job.href
                        && source.status == JobStatus::Succeeded
                        && (source.asset_key == "product"
                            && job.safe.as_ref().is_some_and(|spec| {
                                spec.source_job_id == source.id
                                    && Some(&spec.source_sha256) == source.sha256.as_ref()
                            })
                            || source.asset_key == "viirs"
                                && job.viirs_prepare.as_ref().is_some_and(|spec| {
                                    spec.source_job_id == source.id
                                        && Some(&spec.source_sha256) == source.sha256.as_ref()
                                }))
                });
            if !outputs.contains(&job.id) && (outputs.contains(parent) || prepared) {
                outputs.insert(job.id.clone());
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    outputs.extend(sources);
    outputs
}

#[cfg(test)]
mod tests;
