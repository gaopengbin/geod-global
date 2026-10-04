use super::{api_error, ApiError, JobManager};
use crate::{wcs, wcs_projects, Project, ProjectDownloads};
use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

async fn list(State(m): State<JobManager>) -> Json<Vec<wcs::Connection>> {
    Json(m.list_wcs_connections().await)
}
async fn connect(
    State(m): State<JobManager>,
    Json(r): Json<wcs::ConnectRequest>,
) -> Result<Json<wcs::Connection>, ApiError> {
    m.connect_wcs(r).await.map(Json).map_err(api_error)
}
async fn forget(
    State(m): State<JobManager>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    m.forget_wcs_connection(&id).await.map_err(api_error)?;
    Ok(Json(serde_json::json!({"removed":true})))
}
async fn describe(
    State(m): State<JobManager>,
    Json(r): Json<wcs::DescribeRequest>,
) -> Result<Json<wcs::Description>, ApiError> {
    m.describe_wcs(r).await.map(Json).map_err(api_error)
}
async fn plan(
    State(m): State<JobManager>,
    Json(r): Json<wcs::PlanRequest>,
) -> Result<Json<wcs::Plan>, ApiError> {
    m.plan_wcs(r).await.map(Json).map_err(api_error)
}
async fn snapshot(
    State(m): State<JobManager>,
    Path(id): Path<String>,
) -> Result<Json<wcs::Plan>, ApiError> {
    m.wcs_plan(&id).await.map(Json).map_err(api_error)
}
async fn description(
    State(m): State<JobManager>,
    Path(id): Path<String>,
) -> Result<Json<wcs::Description>, ApiError> {
    m.wcs_description(&id).await.map(Json).map_err(api_error)
}
async fn project(
    State(m): State<JobManager>,
    Json(r): Json<wcs_projects::SaveProjectRequest>,
) -> Result<Json<Project>, ApiError> {
    m.save_wcs_project(r).await.map(Json).map_err(api_error)
}
async fn download(
    State(m): State<JobManager>,
    Json(r): Json<wcs_projects::DownloadRequest>,
) -> Result<Json<ProjectDownloads>, ApiError> {
    m.download_wcs_project(r).await.map(Json).map_err(api_error)
}
async fn inspect(
    State(m): State<JobManager>,
    Path(id): Path<String>,
) -> Result<Json<crate::stac::raster::GenericRasterInspection>, ApiError> {
    m.inspect_wcs_asset(&id).await.map(Json).map_err(api_error)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pixel {
    column: u32,
    row: u32,
}
async fn pixel(
    State(m): State<JobManager>,
    Path(id): Path<String>,
    Query(p): Query<Pixel>,
) -> Result<Json<crate::stac::raster::GenericRasterPixel>, ApiError> {
    m.sample_wcs_asset(&id, p.column, p.row)
        .await
        .map(Json)
        .map_err(api_error)
}
pub(super) fn routes() -> Router<JobManager> {
    Router::new()
        .route("/wcs/connections", get(list).post(connect))
        .route("/wcs/connections/{id}/forget", post(forget))
        .route("/wcs/describe", post(describe))
        .route("/wcs/descriptions/{id}", get(description))
        .route("/wcs/plan", post(plan))
        .route("/wcs/plans/{id}", get(snapshot))
        .route("/wcs/project", post(project))
        .route("/wcs/downloads", post(download))
        .route("/wcs/jobs/{id}/inspect", get(inspect))
        .route("/wcs/jobs/{id}/pixel", get(pixel))
}
