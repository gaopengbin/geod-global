use super::*;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportQuery {
    name: String,
}
async fn import(
    State(m): State<JobManager>,
    Query(q): Query<ImportQuery>,
    bytes: axum::body::Bytes,
) -> std::result::Result<Json<crate::tiles::Package>, ApiError> {
    m.import_tile_bytes(q.name, bytes.to_vec())
        .await
        .map(Json)
        .map_err(api_error)
}
async fn sources(State(m): State<JobManager>) -> Json<Vec<crate::tiles::Source>> {
    Json(m.list_tile_sources().await)
}
async fn connect(
    State(m): State<JobManager>,
    Json(r): Json<crate::tiles::ConnectRequest>,
) -> std::result::Result<Json<crate::tiles::Source>, ApiError> {
    m.connect_tiles(r).await.map(Json).map_err(api_error)
}
async fn forget(
    State(m): State<JobManager>,
    Path(id): Path<String>,
) -> std::result::Result<Json<serde_json::Value>, ApiError> {
    m.forget_tile_source(&id).await.map_err(api_error)?;
    Ok(Json(json!({"removed":true})))
}
async fn packages(State(m): State<JobManager>) -> Json<Vec<crate::tiles::Package>> {
    Json(m.list_tile_packages().await)
}
async fn extract(
    State(m): State<JobManager>,
    Json(r): Json<crate::tiles::ExtractRequest>,
) -> std::result::Result<Json<crate::tiles::Package>, ApiError> {
    m.extract_tiles(r).await.map(Json).map_err(api_error)
}
async fn inspect(
    State(m): State<JobManager>,
    Path(id): Path<String>,
) -> std::result::Result<Json<crate::tiles::Inspection>, ApiError> {
    m.inspect_tile_package(&id)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn tile(
    State(m): State<JobManager>,
    Path((id, z, x, y)): Path<(String, u8, u32, u32)>,
) -> std::result::Result<Json<crate::tiles::Tile>, ApiError> {
    m.read_tile(crate::tiles::TileRequest { id, z, x, y })
        .await
        .map(Json)
        .map_err(api_error)
}
async fn export(
    State(m): State<JobManager>,
    Path(id): Path<String>,
) -> std::result::Result<Response, ApiError> {
    let b = m.tile_export(&id).await.map_err(api_error)?;
    Ok(([(header::CONTENT_TYPE, "application/zip")], b).into_response())
}
pub(super) fn routes() -> Router<JobManager> {
    Router::new()
        .route("/tile-sources", get(sources).post(connect))
        .route("/tile-sources/{id}/forget", post(forget))
        .route("/tile-packages", get(packages).post(extract))
        .route(
            "/tile-packages/import",
            post(import).layer(DefaultBodyLimit::max(128 * 1024 * 1024)),
        )
        .route("/tile-packages/{id}", get(inspect))
        .route("/tile-packages/{id}/tiles/{z}/{x}/{y}", get(tile))
        .route("/tile-packages/{id}/export", get(export))
}
