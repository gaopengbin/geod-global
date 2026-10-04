//! Shareable diagnostics deliberately exclude paths, coordinates, source URLs and user text.
use crate::{JobManager, JobStatus};
use serde_json::{json, Value};

impl JobManager {
    pub async fn diagnostics(&self) -> Value {
        let jobs = self.list().await;
        let mut counts = serde_json::Map::new();
        for (status, label) in [
            (JobStatus::Queued, "queued"),
            (JobStatus::Running, "running"),
            (JobStatus::Succeeded, "succeeded"),
            (JobStatus::Failed, "failed"),
            (JobStatus::Cancelled, "cancelled"),
            (JobStatus::Interrupted, "interrupted"),
        ] {
            counts.insert(
                label.into(),
                json!(jobs.iter().filter(|job| job.status == status).count()),
            );
        }
        json!({
            "schemaVersion":"geod-support-diagnostics/v1",
            "runtime":"geod-runtime", "version":env!("CARGO_PKG_VERSION"),
            "platform":std::env::consts::OS,"architecture":std::env::consts::ARCH,
            "capabilities":["sentinel-public-download","scl-inspection","scl-pixel-query","scl-rectangle-clip","pinned-local-recipes","verified-artifact-package","naip-original-download","naip-rgb-inspection","naip-nir-pixel-query","persistent-thumbnail-cache"],
            "limits":{"downloadBytes":crate::MAX_ASSET_BYTES,"aerialDownloadBytes":crate::providers::MAX_NAIP_BYTES,"rasterLimitScope":"single-band-scl","rasterFileBytes":128*1024*1024,"rasterPixels":64*1024*1024,"rasterWorkers":1,"downloadWorkers":2,"queuedJobs":64,"deliveryInputBytes":32*1024*1024},
            "jobCounts":counts,"savedRecipeCount":self.list_recipes().await.len(),
            "privacy":{"includesPaths":false,"includesCoordinates":false,"includesSourceUrls":false,"includesUserText":false,"uploaded":false},
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn support_report_has_only_allowlisted_aggregate_fields() {
        let directory = tempfile::tempdir().unwrap();
        let manager = JobManager::open(directory.path()).await.unwrap();
        let result = manager.diagnostics().await;
        let object = result.as_object().unwrap();
        let keys: std::collections::BTreeSet<_> = object.keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            std::collections::BTreeSet::from([
                "schemaVersion",
                "runtime",
                "version",
                "platform",
                "architecture",
                "capabilities",
                "limits",
                "jobCounts",
                "savedRecipeCount",
                "privacy",
            ])
        );
        assert_eq!(result["schemaVersion"], "geod-support-diagnostics/v1");
        assert_eq!(result["jobCounts"]["succeeded"], 0);
        assert_eq!(result["privacy"]["includesPaths"], false);
        assert!(!result
            .to_string()
            .contains(directory.path().to_str().unwrap()));
        assert!(!result.to_string().contains("storageRoot"));
    }
}
