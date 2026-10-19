use super::{api_error, ApiError, JobManager};
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::header,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
#[derive(Deserialize)]
struct ImportQuery {
    name: String,
}
#[derive(Deserialize)]
struct PixelQuery {
    column: u32,
    row: u32,
}
async fn list(
    State(m): State<JobManager>,
) -> Result<Json<Vec<crate::local_raster::LocalRaster>>, ApiError> {
    m.list_local_rasters().await.map(Json).map_err(api_error)
}
async fn import(
    State(m): State<JobManager>,
    Query(q): Query<ImportQuery>,
    body: Bytes,
) -> Result<Json<crate::local_raster::ImportedRaster>, ApiError> {
    m.import_local_raster(q.name, body.to_vec())
        .await
        .map(Json)
        .map_err(api_error)
}
async fn inspect(
    State(m): State<JobManager>,
    Path(id): Path<String>,
) -> Result<Json<crate::stac::GenericRasterInspection>, ApiError> {
    m.inspect_local_raster(&id)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn pixel(
    State(m): State<JobManager>,
    Path(id): Path<String>,
    Query(q): Query<PixelQuery>,
) -> Result<Json<crate::stac::GenericRasterPixel>, ApiError> {
    m.sample_local_raster(&id, q.column, q.row)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn source(
    State(m): State<JobManager>,
    Path(id): Path<String>,
) -> Result<([(header::HeaderName, &'static str); 1], Vec<u8>), ApiError> {
    m.local_raster_file(&id)
        .await
        .map(|b| ([(header::CONTENT_TYPE, "image/tiff")], b))
        .map_err(api_error)
}
pub(super) fn routes() -> Router<JobManager> {
    Router::new()
        .route("/local-rasters", get(list))
        .route(
            "/local-rasters/import-file",
            post(import).layer(DefaultBodyLimit::max(crate::local_raster::MAX_BYTES)),
        )
        .route("/local-rasters/{id}/inspect", get(inspect))
        .route("/local-rasters/{id}/pixel", get(pixel))
        .route("/local-rasters/{id}/source", get(source))
}
