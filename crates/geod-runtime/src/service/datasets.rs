//! Reviewed public dataset recipes use the ordinary persisted STAC/WCS paths.
use super::{api_error, ApiError, JobManager};
use crate::datasets;
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};

async fn list() -> Json<Vec<datasets::Dataset>> {
    Json(datasets::list())
}
async fn resolve(
    Json(request): Json<datasets::ResolveRequest>,
) -> Result<Json<datasets::DatasetPlan>, ApiError> {
    datasets::resolve(request).map(Json).map_err(api_error)
}
async fn connect(
    State(manager): State<JobManager>,
    Json(request): Json<datasets::ResolveRequest>,
) -> Result<Json<datasets::DatasetConnection>, ApiError> {
    manager
        .connect_dataset(request)
        .await
        .map(Json)
        .map_err(api_error)
}
pub(super) fn routes() -> Router<JobManager> {
    Router::new()
        .route("/datasets", get(list))
        .route("/datasets/resolve", post(resolve))
        .route("/datasets/connect", post(connect))
}
