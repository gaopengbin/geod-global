use super::{api_error, ApiError, JobManager};
use crate::vector::parquet_original::{self, Request, Rows};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use serde_json::Value;
use tokio::io::AsyncReadExt;

async fn providers() -> Json<Value> {
    Json(parquet_original::providers())
}
async fn search(Json(request): Json<Request>) -> Result<Json<Value>, ApiError> {
    parquet_original::search(request)
        .map(Json)
        .map_err(api_error)
}
async fn download(
    State(manager): State<JobManager>,
    Json(request): Json<Request>,
) -> Result<Json<crate::vector::VectorAsset>, ApiError> {
    manager
        .download_official_parquet(request)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn metadata(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    manager
        .parquet_original_metadata(&id)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn rows(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
    Query(args): Query<Rows>,
) -> Result<Json<Value>, ApiError> {
    manager
        .parquet_original_rows(&id, args)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn original(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let path = manager
        .parquet_original_verified_file(&id)
        .await
        .map_err(api_error)?;
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|e| api_error(e.to_string()))?;
    let stream = futures_util::stream::try_unfold(file, |mut file| async move {
        let mut buffer = vec![0u8; 64 * 1024];
        let n = file.read(&mut buffer).await?;
        if n == 0 {
            Ok::<_, std::io::Error>(None)
        } else {
            buffer.truncate(n);
            Ok(Some((buffer, file)))
        }
    });
    Response::builder()
        .header("Content-Type", "application/vnd.apache.parquet")
        .header("Content-Length", "52018294")
        .header(
            "Content-Disposition",
            "attachment; filename=Overture-2026-09-23.1-base-bathymetry-part-00000.parquet",
        )
        .body(Body::from_stream(stream))
        .map_err(|e| api_error(e.to_string()))
}
pub(super) fn routes() -> Router<JobManager> {
    Router::new()
        .route("/official-parquet", get(providers))
        .route("/official-parquet/search", post(search))
        .route("/official-parquet/download", post(download))
        .route("/vectors/{id}/parquet/metadata", get(metadata))
        .route("/vectors/{id}/parquet/rows", get(rows))
        .route("/vectors/{id}/parquet/original", get(original))
}
