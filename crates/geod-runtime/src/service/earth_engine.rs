use super::{api_error, ApiError, JobManager};
use crate::earth_engine::{DownloadRequest, DownloadResult, Export};
use axum::{
    extract::{Path, State},
    http::header,
    routing::get,
    Json, Router,
};

async fn list(State(manager): State<JobManager>) -> Result<Json<Vec<Export>>, ApiError> {
    manager
        .list_earth_engine_exports()
        .await
        .map(Json)
        .map_err(api_error)
}
async fn download(
    State(manager): State<JobManager>,
    Json(request): Json<DownloadRequest>,
) -> Result<Json<DownloadResult>, ApiError> {
    manager
        .download_earth_engine_export(request)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn inspect(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
) -> Result<Json<crate::stac::GenericRasterInspection>, ApiError> {
    manager
        .inspect_earth_engine_export(&id)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn file(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
) -> Result<([(header::HeaderName, &'static str); 2], Vec<u8>), ApiError> {
    let bytes = manager.earth_engine_file(&id).await.map_err(api_error)?;
    Ok((
        [
            (header::CONTENT_TYPE, "image/tiff"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=gee-export.tif",
            ),
        ],
        bytes,
    ))
}
pub(super) fn routes() -> Router<JobManager> {
    Router::new()
        .route("/earth-engine/exports", get(list).post(download))
        .route("/earth-engine/exports/{id}/inspect", get(inspect))
        .route("/earth-engine/exports/{id}/file", get(file))
}
