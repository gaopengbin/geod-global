use super::{api_error, ApiError, JobManager};
use crate::{stac, stac_projects, Project, ProjectDownloads};
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

async fn list(State(manager): State<JobManager>) -> Json<Vec<stac::Connection>> {
    Json(manager.list_stac_connections().await)
}
async fn connect(
    State(manager): State<JobManager>,
    Json(request): Json<stac::ConnectRequest>,
) -> Result<Json<stac::Connection>, ApiError> {
    manager
        .connect_stac(request)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn forget(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    manager
        .forget_stac_connection(&id)
        .await
        .map_err(api_error)?;
    Ok(Json(serde_json::json!({"removed": true})))
}
async fn search(
    State(manager): State<JobManager>,
    Json(request): Json<stac::SearchRequest>,
) -> Result<Json<stac::SearchPage>, ApiError> {
    manager
        .search_stac(request)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn snapshot(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
) -> Result<Json<stac::ItemSnapshot>, ApiError> {
    manager
        .stac_snapshot(&id)
        .await
        .map(Json)
        .map_err(api_error)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PreviewQuery {
    asset_key: String,
}
async fn preview(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
    Query(query): Query<PreviewQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if headers.get_all(header::RANGE).iter().count() != 1 {
        return Err(api_error(
            "A single bounded Range header is required".into(),
        ));
    }
    let range = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| api_error("A bounded Range header is required".into()))?;
    manager
        .read_stac_preview(&id, &query.asset_key, range)
        .await
        .map(|data| data.into_response().map(axum::body::Body::from))
        .map_err(api_error)
}
async fn save_project(
    State(manager): State<JobManager>,
    Json(request): Json<stac_projects::SaveProjectRequest>,
) -> Result<Json<Project>, ApiError> {
    manager
        .save_stac_project(request)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn download(
    State(manager): State<JobManager>,
    Json(request): Json<stac_projects::DownloadRequest>,
) -> Result<Json<ProjectDownloads>, ApiError> {
    manager
        .download_stac_project(request)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn inspect(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
    Query(query): Query<InspectionQuery>,
) -> Result<Json<stac::raster::GenericRasterInspection>, ApiError> {
    manager
        .inspect_stac_asset_band(&id, query.display_band)
        .await
        .map(Json)
        .map_err(api_error)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InspectionQuery {
    display_band: Option<u16>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pixel {
    column: u32,
    row: u32,
}
async fn pixel(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
    Query(p): Query<Pixel>,
) -> Result<Json<stac::raster::GenericRasterPixel>, ApiError> {
    manager
        .sample_stac_asset(&id, p.column, p.row)
        .await
        .map(Json)
        .map_err(api_error)
}
pub(super) fn routes() -> Router<JobManager> {
    Router::new()
        .route("/stac/connections", get(list).post(connect))
        .route("/stac/connections/{id}/forget", post(forget))
        .route("/stac/search", post(search))
        .route("/stac/snapshots/{id}", get(snapshot))
        .route("/stac/snapshots/{id}/preview", get(preview))
        .route("/stac/project", post(save_project))
        .route("/stac/downloads", post(download))
        .route("/stac/jobs/{id}/inspect", get(inspect))
        .route("/stac/jobs/{id}/pixel", get(pixel))
}
