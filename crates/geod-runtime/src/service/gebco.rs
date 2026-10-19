use super::{api_error, ApiError, JobManager};
use crate::gebco::{Catalog, PixelRequest, Subset, SubsetRequest, SubsetResult};
use axum::{
    extract::{Path, State},
    http::header,
    routing::get,
    Json, Router,
};
async fn grids(State(m): State<JobManager>) -> Result<Json<Catalog>, ApiError> {
    m.gebco_grids().await.map(Json).map_err(api_error)
}
async fn list(State(m): State<JobManager>) -> Result<Json<Vec<Subset>>, ApiError> {
    m.gebco_list().await.map(Json).map_err(api_error)
}
async fn create(
    State(m): State<JobManager>,
    Json(r): Json<SubsetRequest>,
) -> Result<Json<SubsetResult>, ApiError> {
    m.gebco_create_subset(r).await.map(Json).map_err(api_error)
}
async fn record(
    State(m): State<JobManager>,
    Path(id): Path<String>,
) -> Result<Json<Subset>, ApiError> {
    m.gebco_get(&id).await.map(Json).map_err(api_error)
}
async fn inspect(
    State(m): State<JobManager>,
    Path(id): Path<String>,
) -> Result<Json<crate::stac::GenericRasterInspection>, ApiError> {
    m.gebco_inspect(&id).await.map(Json).map_err(api_error)
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Coordinate {
    column: u32,
    row: u32,
}
async fn pixel(
    State(m): State<JobManager>,
    Path(id): Path<String>,
    Json(p): Json<Coordinate>,
) -> Result<Json<crate::stac::GenericRasterPixel>, ApiError> {
    m.gebco_pixel(PixelRequest {
        id,
        column: p.column,
        row: p.row,
    })
    .await
    .map(Json)
    .map_err(api_error)
}
async fn source(
    State(m): State<JobManager>,
    Path(id): Path<String>,
) -> Result<([(header::HeaderName, &'static str); 2], Vec<u8>), ApiError> {
    Ok((
        [
            (header::CONTENT_TYPE, "image/tiff"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=gebco-2026-numerical-subset.tif",
            ),
        ],
        m.gebco_source(&id).await.map_err(api_error)?,
    ))
}
async fn archive(
    State(m): State<JobManager>,
    Path(id): Path<String>,
) -> Result<([(header::HeaderName, &'static str); 2], Vec<u8>), ApiError> {
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=gebco-2026-official-subset.zip",
            ),
        ],
        m.gebco_archive(&id).await.map_err(api_error)?,
    ))
}
pub(super) fn routes() -> Router<JobManager> {
    Router::new()
        .route("/gebco/grids", get(grids))
        .route("/gebco/subsets", get(list).post(create))
        .route("/gebco/subsets/{id}", get(record))
        .route("/gebco/subsets/{id}/inspect", get(inspect))
        .route("/gebco/subsets/{id}/pixel", axum::routing::post(pixel))
        .route("/gebco/subsets/{id}/source", get(source))
        .route("/gebco/subsets/{id}/archive", get(archive))
}
