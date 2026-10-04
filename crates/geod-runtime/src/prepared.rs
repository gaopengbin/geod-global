//! Provenance dispatch for managed original-resolution raster preparation.
use crate::{projects::ProjectScene, Job, Result};
use std::collections::BTreeMap;

pub(crate) fn validate_stored(job: &Job) -> Result<()> {
    if job.viirs_prepare.is_some() {
        crate::providers::viirs::prepare::validate_stored(job)
    } else {
        crate::safe::validate_stored(job)
    }
}
pub(crate) fn validate_source(job: &Job, jobs: &BTreeMap<String, Job>) -> Result<Job> {
    if job.viirs_prepare.is_some() {
        crate::providers::viirs::prepare::validate_source(job, jobs)
    } else {
        crate::safe::validate_source(job, jobs)
    }
}
pub(crate) fn scene_source<'a>(
    scene: &ProjectScene,
    jobs: &'a BTreeMap<String, Job>,
    key: &str,
) -> Option<&'a Job> {
    if scene.assets.contains_key("viirs") && matches!(key, "red" | "green" | "blue") {
        crate::providers::viirs::prepare::scene_source(scene, jobs, key)
    } else {
        crate::safe::scene_source(scene, jobs, key)
    }
}
