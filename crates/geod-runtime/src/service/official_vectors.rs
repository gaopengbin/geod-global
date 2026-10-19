use super::{api_error, ApiError, JobManager};
use crate::vector::official_packages::{self, DownloadRequest, SearchRequest};
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde_json::Value;

async fn providers() -> Json<Value> {
    Json(official_packages::providers())
}
async fn search(Json(request): Json<SearchRequest>) -> Result<Json<Value>, ApiError> {
    official_packages::search(request)
        .map(Json)
        .map_err(api_error)
}
async fn download(
    State(manager): State<JobManager>,
    Json(request): Json<DownloadRequest>,
) -> Result<Json<crate::vector::VectorAsset>, ApiError> {
    manager
        .download_official_vector_package(request)
        .await
        .map(Json)
        .map_err(api_error)
}
pub(super) fn routes() -> Router<JobManager> {
    Router::new()
        .route("/official-vector-packages", get(providers))
        .route("/official-vector-packages/search", post(search))
        .route("/official-vector-packages/download", post(download))
}
