use super::*;
async fn list(State(m): State<JobManager>) -> Json<Vec<crate::three_d::Package>> {
    Json(m.list_three_d().await)
}
async fn discover(
    State(m): State<JobManager>,
    Json(r): Json<crate::three_d::DiscoverRequest>,
) -> std::result::Result<Json<crate::three_d::Discovery>, ApiError> {
    m.discover_three_d(r).await.map(Json).map_err(api_error)
}
async fn acquire(
    State(m): State<JobManager>,
    Json(r): Json<crate::three_d::AcquireRequest>,
) -> std::result::Result<Json<crate::three_d::Package>, ApiError> {
    m.acquire_three_d(r).await.map(Json).map_err(api_error)
}
async fn inspect(
    State(m): State<JobManager>,
    Path(id): Path<String>,
) -> std::result::Result<Json<crate::three_d::Package>, ApiError> {
    m.inspect_three_d(&id).await.map(Json).map_err(api_error)
}
async fn resource(
    State(m): State<JobManager>,
    Path((id, resource_id)): Path<(String, String)>,
) -> std::result::Result<Json<crate::three_d::ResourceData>, ApiError> {
    m.read_three_d_resource(crate::three_d::ResourceRequest { id, resource_id })
        .await
        .map(Json)
        .map_err(api_error)
}
async fn export(
    State(m): State<JobManager>,
    Path(id): Path<String>,
) -> std::result::Result<Response, ApiError> {
    let b = m.three_d_export(&id).await.map_err(api_error)?;
    Ok(([(header::CONTENT_TYPE, "application/zip")], b).into_response())
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImportQuery {
    name: String,
    license: String,
    attribution: String,
    license_url: Option<String>,
    permission_confirmed: bool,
}
async fn import(
    State(m): State<JobManager>,
    Query(q): Query<ImportQuery>,
    bytes: axum::body::Bytes,
) -> std::result::Result<Json<crate::three_d::Package>, ApiError> {
    m.import_three_d_archive(
        bytes.to_vec(),
        crate::three_d::LocalRequest {
            name: q.name,
            rights: crate::three_d::Rights {
                license: q.license,
                attribution: q.attribution,
                license_url: q.license_url,
                permission_confirmed: q.permission_confirmed,
            },
        },
    )
    .await
    .map(Json)
    .map_err(api_error)
}
pub(super) fn routes() -> Router<JobManager> {
    Router::new()
        .route("/three-d/discover", post(discover))
        .route("/three-d/packages", get(list).post(acquire))
        .route(
            "/three-d/packages/import",
            post(import).layer(DefaultBodyLimit::max(272 * 1024 * 1024)),
        )
        .route("/three-d/packages/{id}", get(inspect))
        .route(
            "/three-d/packages/{id}/resources/{resource_id}",
            get(resource),
        )
        .route("/three-d/packages/{id}/export", get(export))
}
