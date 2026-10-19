use super::{api_error, ApiError, JobManager};
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde_json::Value;
async fn products(State(manager): State<JobManager>) -> Result<Json<Value>, ApiError> {
    manager
        .gscloud_products()
        .await
        .map(Json)
        .map_err(api_error)
}
async fn search(
    State(manager): State<JobManager>,
    Json(request): Json<crate::gscloud::SearchRequest>,
) -> Result<Json<Value>, ApiError> {
    manager
        .gscloud_search(request)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn detail(
    State(manager): State<JobManager>,
    Json(request): Json<crate::gscloud::DetailRequest>,
) -> Result<Json<Value>, ApiError> {
    manager
        .gscloud_detail(request)
        .await
        .map(Json)
        .map_err(api_error)
}
pub(super) fn routes() -> Router<JobManager> {
    Router::new()
        .route("/gscloud/products", get(products))
        .route("/gscloud/search", post(search))
        .route("/gscloud/detail", post(detail))
}
