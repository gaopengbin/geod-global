use super::*;
use crate::{
    raster::tests::{fixture, record},
    RasterRecipe,
};
use std::collections::BTreeMap;

async fn completed(root: &Path) -> (JobManager, Job) {
    let source = record(root, &fixture(3, 2, &[0, 4, 4, 6, 9, 11], 32610, false));
    std::fs::write(
        root.join("jobs.json"),
        serde_json::to_vec(&BTreeMap::from([(source.id.clone(), source.clone())])).unwrap(),
    )
    .unwrap();
    let manager = JobManager::open(root).await.unwrap();
    let recipe: RasterRecipe = serde_json::from_value(json!({"schemaVersion":"geod-raster-recipe/v1", "name":"Verified sample", "source":{"jobId":source.id,"sha256":source.sha256}, "operation":{"type":"clip","crs":"source","bounds":[500000,4199960,500040,4200000]}, "output":{"format":"GeoTIFF"}})).unwrap();
    let job = manager.run_recipe(recipe).await.unwrap();
    let output = manager.wait(&job.id).await.unwrap();
    assert_eq!(output.status, JobStatus::Succeeded);
    (manager, output)
}

#[tokio::test]
async fn package_is_persisted_repeatable_and_contains_exact_verified_files_without_local_paths() {
    let directory = tempfile::tempdir().unwrap();
    let (manager, job) = completed(directory.path()).await;
    assert!(manager.artifact_bytes(&job.id).await.is_err());
    let first = manager.prepare_artifact(&job.id).await.unwrap();
    let second = manager.prepare_artifact(&job.id).await.unwrap();
    assert_eq!(first.sha256, second.sha256);
    let (metadata, bytes) = manager.artifact_bytes(&job.id).await.unwrap();
    assert_eq!(metadata.sha256, format!("{:x}", Sha256::digest(&bytes)));
    assert_eq!(metadata.bytes, bytes.len() as u64);
    assert_eq!(std::fs::read(&first.path).unwrap(), bytes);
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    assert_eq!(archive.len(), 5);
    let mut tiff = Vec::new();
    archive
        .by_name(&format!("{}.tif", job.id))
        .unwrap()
        .read_to_end(&mut tiff)
        .unwrap();
    assert_eq!(
        tiff,
        std::fs::read(job.output_path.as_ref().unwrap()).unwrap()
    );
    for name in [
        &format!("{}.metadata.json", job.id),
        "recipe.json",
        "README.txt",
    ] {
        let mut text = String::new();
        archive
            .by_name(name)
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert!(!text.contains(&directory.path().to_string_lossy().to_string()));
    }
    drop(manager);
    let restarted = JobManager::open(directory.path()).await.unwrap();
    assert_eq!(
        restarted.artifact_bytes(&job.id).await.unwrap().0.sha256,
        first.sha256
    );
}

#[tokio::test]
async fn changed_tiff_metadata_and_existing_package_never_get_silently_exported_or_overwritten() {
    let directory = tempfile::tempdir().unwrap();
    let (manager, job) = completed(directory.path()).await;
    let package = manager.prepare_artifact(&job.id).await.unwrap();
    let original = std::fs::read(job.output_path.as_ref().unwrap()).unwrap();
    std::fs::write(job.output_path.as_ref().unwrap(), [0u8]).unwrap();
    assert!(manager
        .prepare_artifact(&job.id)
        .await
        .unwrap_err()
        .contains("checksum"));
    std::fs::write(job.output_path.as_ref().unwrap(), original).unwrap();
    let metadata = std::fs::read(job.manifest_path.as_ref().unwrap()).unwrap();
    let mut changed: Value = serde_json::from_slice(&metadata).unwrap();
    changed["privatePath"] = json!("C:/private/should-never-export");
    std::fs::write(
        job.manifest_path.as_ref().unwrap(),
        serde_json::to_vec(&changed).unwrap(),
    )
    .unwrap();
    assert!(manager
        .prepare_artifact(&job.id)
        .await
        .unwrap_err()
        .contains("metadata"));
    std::fs::write(job.manifest_path.as_ref().unwrap(), metadata).unwrap();
    std::fs::write(&package.path, b"unrelated user bytes").unwrap();
    assert!(manager.prepare_artifact(&job.id).await.is_err());
    assert!(manager.artifact_bytes(&job.id).await.is_err());
    assert_eq!(
        std::fs::read(package.path).unwrap(),
        b"unrelated user bytes"
    );
}

#[tokio::test]
async fn package_api_requires_mutation_header_and_download_has_attachment_headers() {
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use tower::ServiceExt;
    let directory = tempfile::tempdir().unwrap();
    let (manager, job) = completed(directory.path()).await;
    let app = crate::service::router(manager);
    let route = format!("/jobs/{}/package", job.id);
    let denied = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&route)
                .header("host", "127.0.0.1:4318")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), 403);
    let prepared = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&route)
                .header("host", "127.0.0.1:4318")
                .header("x-geod-client", "geod-global")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(prepared.status(), 200);
    let response = app
        .oneshot(
            Request::builder()
                .uri(&route)
                .header("host", "127.0.0.1:4318")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(response.headers()["content-disposition"]
        .to_str()
        .unwrap()
        .starts_with("attachment; filename=\"geod-artifact-"));
    assert_eq!(response.headers()["content-type"], "application/zip");
    assert!(to_bytes(response.into_body(), 100000)
        .await
        .unwrap()
        .starts_with(b"PK"));
}
