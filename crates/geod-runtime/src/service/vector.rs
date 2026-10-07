use super::{api_error, ApiError, JobManager};
use crate::vector::reads::{Node, Page};
use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde_json::Value;

async fn services(
    State(m): State<JobManager>,
    Query(p): Query<Page>,
) -> Result<Json<Value>, ApiError> {
    m.feature_service_metadata(p)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn collections(
    State(m): State<JobManager>,
    Path(id): Path<String>,
    Query(p): Query<Page>,
) -> Result<Json<Value>, ApiError> {
    m.feature_collection_metadata(&id, p)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn vectors(
    State(m): State<JobManager>,
    Query(p): Query<Page>,
) -> Result<Json<Value>, ApiError> {
    m.vector_metadata_list(p).await.map(Json).map_err(api_error)
}
async fn inspect(
    State(m): State<JobManager>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    m.vector_metadata(&id).await.map(Json).map_err(api_error)
}
async fn features(
    State(m): State<JobManager>,
    Path(id): Path<String>,
    Query(p): Query<Page>,
) -> Result<Json<Value>, ApiError> {
    m.vector_features(&id, p).await.map(Json).map_err(api_error)
}
async fn node(
    State(m): State<JobManager>,
    Path(id): Path<String>,
    Query(p): Query<Node>,
) -> Result<Json<Value>, ApiError> {
    m.vector_node(&id, p).await.map(Json).map_err(api_error)
}
pub(super) fn routes() -> Router<JobManager> {
    Router::new()
        .route("/feature-services/metadata", get(services))
        .route("/feature-services/{id}/collections", get(collections))
        .route("/vectors/metadata", get(vectors))
        .route("/vectors/{id}/metadata", get(inspect))
        .route("/vectors/{id}/features", get(features))
        .route("/vectors/{id}/node", get(node))
}
