//! Loopback development adapter; desktop commands use JobManager directly.
use crate::{CreateJobRequest, CreateProjectRequest, JobManager, ProxySettings, RasterRecipe};
use axum::{
    extract::{DefaultBodyLimit, Path, Query, Request, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
mod stac;
mod three_d;
mod tiles;
mod wcs;

pub const ALLOWED_ORIGIN: &str = "http://127.0.0.1:4317";

async fn browser_boundary(request: Request, next: Next) -> Response {
    let origin = request.headers().get(header::ORIGIN);
    if origin.is_some_and(|value| value != ALLOWED_ORIGIN) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error":"Origin is not allowed"})),
        )
            .into_response();
    }
    let host_valid = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|host| {
            host == "127.0.0.1"
                || host
                    .strip_prefix("127.0.0.1:")
                    .is_some_and(|port| port.parse::<u16>().is_ok())
        });
    if !host_valid {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error":"Only a loopback Host is allowed"})),
        )
            .into_response();
    }
    let has_origin = origin.is_some();
    let mut response = if request.method() == Method::OPTIONS {
        if !has_origin {
            return StatusCode::FORBIDDEN.into_response();
        }
        StatusCode::NO_CONTENT.into_response()
    } else {
        if request.method() == Method::POST
            && request
                .headers()
                .get("x-geod-client")
                .is_none_or(|value| value != "geod-global")
        {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({"error":"Missing GeoD client header"})),
            )
                .into_response();
        }
        next.run(request).await
    };
    if has_origin {
        let headers = response.headers_mut();
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static(ALLOWED_ORIGIN),
        );
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static("GET, POST, OPTIONS"),
        );
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            HeaderValue::from_static("Content-Type, X-GeoD-Client, Range"),
        );
        headers.insert(header::VARY, HeaderValue::from_static("Origin"));
    }
    response
}

type ApiError = (StatusCode, Json<serde_json::Value>);
fn api_error(error: String) -> ApiError {
    (StatusCode::BAD_REQUEST, Json(json!({"error":error})))
}
async fn health(State(manager): State<JobManager>) -> Json<crate::RuntimeHealth> {
    Json(manager.health())
}

async fn elevation_preview(
    State(manager): State<JobManager>,
    Path(item): Path<String>,
    headers: HeaderMap,
) -> std::result::Result<Response, ApiError> {
    let range = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| api_error("A bounded Range header is required".into()))?;
    manager
        .read_elevation_preview(&item, range)
        .await
        .map(|data| data.into_response().map(axum::body::Body::from))
        .map_err(api_error)
}

async fn diagnostics(State(manager): State<JobManager>) -> Json<serde_json::Value> {
    Json(manager.diagnostics().await)
}
// Read-only status for the development browser. Credential mutations deliberately
// have no loopback HTTP route; they require the desktop's local command ACL.
async fn provider_accounts(State(manager): State<JobManager>) -> Json<Vec<crate::AccountStatus>> {
    Json(manager.provider_accounts().await)
}
// Public OData metadata only; this does not submit or read account credentials.
async fn resolve_copernicus_products(
    State(manager): State<JobManager>,
    Json(request): Json<crate::providers::copernicus::ResolveProductsRequest>,
) -> std::result::Result<Json<Vec<crate::providers::copernicus::ProductAsset>>, ApiError> {
    manager
        .resolve_copernicus_products(request)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn proxy_settings(State(manager): State<JobManager>) -> Json<ProxySettings> {
    Json(manager.proxy_settings().await)
}
async fn save_proxy_settings(
    State(manager): State<JobManager>,
    Json(request): Json<ProxySettings>,
) -> std::result::Result<Json<ProxySettings>, ApiError> {
    manager
        .save_proxy_settings(request)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn test_proxy_settings(
    State(manager): State<JobManager>,
    Json(request): Json<ProxySettings>,
) -> std::result::Result<Json<crate::ProxyTest>, ApiError> {
    manager
        .test_proxy_settings(request)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn jobs(State(manager): State<JobManager>) -> Json<Vec<crate::Job>> {
    Json(manager.list().await)
}
async fn vectors(State(manager): State<JobManager>) -> Json<Vec<crate::vector::VectorAsset>> {
    Json(manager.list_vectors().await)
}
async fn feature_services(
    State(manager): State<JobManager>,
) -> Json<Vec<crate::features::FeatureService>> {
    Json(manager.list_feature_services().await)
}
async fn connect_feature_service(
    State(manager): State<JobManager>,
    Json(request): Json<crate::features::ConnectRequest>,
) -> std::result::Result<Json<crate::features::FeatureService>, ApiError> {
    manager
        .connect_feature_service(request)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn forget_feature_service(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
) -> std::result::Result<Json<serde_json::Value>, ApiError> {
    manager
        .forget_feature_service(&id)
        .await
        .map(|()| Json(json!({"removed":true})))
        .map_err(api_error)
}
async fn query_features(
    State(manager): State<JobManager>,
    Json(request): Json<crate::features::QueryRequest>,
) -> std::result::Result<Json<crate::vector::VectorAsset>, ApiError> {
    manager
        .query_features(request)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn import_vector(
    State(manager): State<JobManager>,
    Json(request): Json<crate::vector::ImportVectorRequest>,
) -> std::result::Result<Json<crate::vector::VectorAsset>, ApiError> {
    manager
        .import_vector(request)
        .await
        .map(Json)
        .map_err(api_error)
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct VectorFileQuery {
    name: String,
}
async fn import_vector_file(
    State(manager): State<JobManager>,
    Query(query): Query<VectorFileQuery>,
    bytes: axum::body::Bytes,
) -> std::result::Result<Json<crate::vector::VectorAsset>, ApiError> {
    manager
        .import_vector_file_bytes(query.name, bytes.to_vec())
        .await
        .map(Json)
        .map_err(api_error)
}
async fn vector_original(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
) -> std::result::Result<Response, ApiError> {
    let bytes = manager
        .vector_original_bytes(&id)
        .await
        .map_err(api_error)?;
    let mime = if bytes.starts_with(b"SQLite format 3\0") {
        "application/geopackage+sqlite3"
    } else if bytes.starts_with(b"PK\x03\x04") {
        "application/zip"
    } else if crate::vector::local_osm::is_xml(&bytes) {
        "application/xml"
    } else if crate::vector::local_osm::is_pbf(&bytes) {
        "application/vnd.openstreetmap.data+pbf"
    } else {
        "application/json"
    };
    Ok(([(header::CONTENT_TYPE, mime)], bytes).into_response())
}
async fn map_services(State(m): State<JobManager>) -> Json<Vec<crate::wms::MapService>> {
    Json(m.list_map_services().await)
}
async fn connect_map_service(
    State(m): State<JobManager>,
    Json(r): Json<crate::wms::ConnectRequest>,
) -> std::result::Result<Json<crate::wms::MapService>, ApiError> {
    m.connect_map_service(r).await.map(Json).map_err(api_error)
}
async fn forget_map_service(
    State(m): State<JobManager>,
    Path(id): Path<String>,
) -> std::result::Result<Json<serde_json::Value>, ApiError> {
    m.forget_map_service(&id)
        .await
        .map(|()| Json(json!({"removed":true})))
        .map_err(api_error)
}
async fn map_images(State(m): State<JobManager>) -> Json<Vec<crate::wms::MapImage>> {
    Json(m.list_map_images().await)
}
async fn get_map_image(
    State(m): State<JobManager>,
    Json(r): Json<crate::wms::MapRequest>,
) -> std::result::Result<Json<crate::wms::MapImage>, ApiError> {
    m.get_map_image(r).await.map(Json).map_err(api_error)
}
async fn inspect_map_image(
    State(m): State<JobManager>,
    Path(id): Path<String>,
) -> std::result::Result<Json<crate::wms::MapInspection>, ApiError> {
    m.inspect_map_image(&id).await.map(Json).map_err(api_error)
}
async fn export_map_image(
    State(m): State<JobManager>,
    Path(id): Path<String>,
) -> std::result::Result<Response, ApiError> {
    let bytes = m.export_map_image(&id).await.map_err(api_error)?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip".into()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"geod-wms-{id}.zip\""),
            ),
            (header::CACHE_CONTROL, "no-store".into()),
        ],
        bytes,
    )
        .into_response())
}
async fn inspect_vector(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
) -> std::result::Result<Json<crate::vector::VectorInspection>, ApiError> {
    manager
        .inspect_vector(&id)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn forget_vector(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
) -> std::result::Result<Json<serde_json::Value>, ApiError> {
    manager
        .forget_vector(&id)
        .await
        .map(|()| Json(json!({"removed":true})))
        .map_err(api_error)
}
async fn projects(State(manager): State<JobManager>) -> Json<Vec<crate::Project>> {
    Json(manager.list_projects().await)
}
async fn create_project(
    State(manager): State<JobManager>,
    Json(request): Json<CreateProjectRequest>,
) -> std::result::Result<(StatusCode, Json<crate::Project>), ApiError> {
    manager
        .create_project(request)
        .await
        .map(|project| (StatusCode::CREATED, Json(project)))
        .map_err(api_error)
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RenameProjectRequest {
    name: String,
}
async fn add_project_scenes(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
    Json(request): Json<crate::AddProjectScenesRequest>,
) -> std::result::Result<Json<crate::Project>, ApiError> {
    manager
        .add_project_scenes(&id, request)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn rename_project(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
    Json(request): Json<RenameProjectRequest>,
) -> std::result::Result<Json<crate::Project>, ApiError> {
    manager
        .rename_project(&id, &request.name)
        .await
        .map(Json)
        .map_err(api_error)
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectDownloadRequest {
    asset_key: String,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectedProjectDownloadRequest {
    asset_key: String,
    #[serde(default)]
    item_ids: Option<Vec<String>>,
}
async fn download_project(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
    Json(request): Json<SelectedProjectDownloadRequest>,
) -> std::result::Result<(StatusCode, Json<crate::ProjectDownloads>), ApiError> {
    manager
        .enqueue_project_selection(&id, &request.asset_key, request.item_ids)
        .await
        .map(|downloads| (StatusCode::ACCEPTED, Json(downloads)))
        .map_err(api_error)
}
async fn prepare_project(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
    Json(request): Json<ProjectDownloadRequest>,
) -> std::result::Result<(StatusCode, Json<crate::ProjectDownloads>), ApiError> {
    manager
        .prepare_project(&id, &request.asset_key)
        .await
        .map(|jobs| (StatusCode::ACCEPTED, Json(jobs)))
        .map_err(api_error)
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectMosaicRequest {
    asset_key: String,
    #[serde(default)]
    vi_quality: Option<crate::mosaic::vegetation::Request>,
}
async fn mosaic_project(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
    Json(request): Json<ProjectMosaicRequest>,
) -> std::result::Result<(StatusCode, Json<crate::Job>), ApiError> {
    manager
        .run_project_mosaic_with_selection(&id, &request.asset_key, request.vi_quality)
        .await
        .map(|job| (StatusCode::ACCEPTED, Json(job)))
        .map_err(api_error)
}
async fn job(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
) -> std::result::Result<Json<JobSnapshot>, ApiError> {
    manager
        .get_with_settled(&id)
        .await
        .map(|(job, settled)| Json(JobSnapshot { job, settled }))
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(json!({"error":"Unknown job"}))))
}

#[derive(serde::Serialize)]
struct JobSnapshot {
    #[serde(flatten)]
    job: crate::Job,
    settled: bool,
}
async fn recipes(State(manager): State<JobManager>) -> Json<Vec<crate::SavedRecipe>> {
    Json(manager.list_recipes().await)
}
async fn plan_recipe(
    State(manager): State<JobManager>,
    Json(recipe): Json<RasterRecipe>,
) -> std::result::Result<Json<crate::RecipePlan>, ApiError> {
    manager
        .plan_recipe(recipe)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn save_recipe(
    State(manager): State<JobManager>,
    Json(recipe): Json<RasterRecipe>,
) -> std::result::Result<(StatusCode, Json<crate::SavedRecipe>), ApiError> {
    manager
        .save_recipe(recipe)
        .await
        .map(|recipe| (StatusCode::CREATED, Json(recipe)))
        .map_err(api_error)
}
async fn run_recipe(
    State(manager): State<JobManager>,
    Json(recipe): Json<RasterRecipe>,
) -> std::result::Result<(StatusCode, Json<crate::Job>), ApiError> {
    manager
        .run_recipe(recipe)
        .await
        .map(|job| (StatusCode::ACCEPTED, Json(job)))
        .map_err(api_error)
}
async fn create(
    State(manager): State<JobManager>,
    Json(request): Json<CreateJobRequest>,
) -> std::result::Result<(StatusCode, Json<crate::Job>), ApiError> {
    manager
        .create(request)
        .await
        .map(|job| (StatusCode::ACCEPTED, Json(job)))
        .map_err(api_error)
}
async fn cancel(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
) -> std::result::Result<Json<crate::Job>, ApiError> {
    manager.cancel(&id).await.map(Json).map_err(api_error)
}
async fn retry(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
) -> std::result::Result<Json<crate::Job>, ApiError> {
    manager.retry(&id).await.map(Json).map_err(api_error)
}

#[derive(Default, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RasterQuery {
    aerial_view: Option<crate::raster::AerialView>,
}

async fn raster(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
    Query(query): Query<RasterQuery>,
) -> std::result::Result<Json<crate::RasterInspection>, ApiError> {
    manager
        .inspect_raster_view(&id, query.aerial_view)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn thumbnail(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
) -> std::result::Result<Json<crate::thumbnail::FileThumbnail>, ApiError> {
    manager
        .file_thumbnail(&id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn composite(
    State(manager): State<JobManager>,
    Json(request): Json<crate::CompositeRequest>,
) -> std::result::Result<Json<crate::CompositeInspection>, ApiError> {
    manager
        .inspect_composite(request)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn composite_pixel(
    State(manager): State<JobManager>,
    Json(request): Json<crate::CompositePixelRequest>,
) -> std::result::Result<Json<crate::CompositePixel>, ApiError> {
    manager
        .sample_composite(request)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn plan_rgb(
    State(manager): State<JobManager>,
    Json(request): Json<crate::RgbRequest>,
) -> std::result::Result<Json<crate::RgbPlan>, ApiError> {
    manager
        .plan_scientific_rgb(request)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn run_rgb(
    State(manager): State<JobManager>,
    Json(request): Json<crate::RgbRequest>,
) -> std::result::Result<Json<crate::Job>, ApiError> {
    manager
        .run_scientific_rgb(request)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn inspect_rgb(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
) -> std::result::Result<Json<crate::CompositeInspection>, ApiError> {
    manager
        .inspect_scientific_rgb(&id)
        .await
        .map(Json)
        .map_err(api_error)
}
async fn pixel_rgb(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
    Query(point): Query<PixelQuery>,
) -> std::result::Result<Json<crate::CompositePixel>, ApiError> {
    manager
        .sample_scientific_rgb(&id, point.x, point.y)
        .await
        .map(Json)
        .map_err(api_error)
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PixelQuery {
    x: f64,
    y: f64,
}

async fn pixel(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
    Query(point): Query<PixelQuery>,
) -> std::result::Result<Json<crate::RasterPixel>, ApiError> {
    manager
        .sample_raster(&id, point.x, point.y)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn prepare_artifact(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
) -> std::result::Result<Json<crate::artifact::ArtifactPackage>, ApiError> {
    manager
        .prepare_artifact(&id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn download_artifact(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
) -> std::result::Result<Response, ApiError> {
    let (package, bytes) = manager.artifact_bytes(&id).await.map_err(api_error)?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", package.filename),
            ),
            (header::CACHE_CONTROL, "no-store".to_owned()),
            (header::ETAG, format!("\"{}\"", package.sha256)),
        ],
        bytes,
    )
        .into_response())
}
async fn download_derived(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
) -> std::result::Result<Response, ApiError> {
    let (filename, bytes) = manager.derived_bytes(&id).await.map_err(api_error)?;
    Ok((
        [
            (header::CONTENT_TYPE, "image/tiff".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        bytes,
    )
        .into_response())
}
async fn download_mosaic_metadata(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
) -> std::result::Result<Response, ApiError> {
    let (filename, bytes) = if manager
        .get(&id)
        .await
        .is_some_and(|job| job.kind == "raster_rgb")
    {
        manager.scientific_rgb_metadata_bytes(&id).await
    } else {
        manager.mosaic_metadata_bytes(&id).await
    }
    .map_err(api_error)?;
    Ok((
        [
            (
                header::CONTENT_TYPE,
                "application/json; charset=utf-8".to_owned(),
            ),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        bytes,
    )
        .into_response())
}

pub fn router(manager: JobManager) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/preview/elevation/{item}", get(elevation_preview))
        .route("/diagnostics", get(diagnostics))
        .route("/accounts", get(provider_accounts))
        .route(
            "/providers/copernicus/products",
            post(resolve_copernicus_products),
        )
        .route("/proxy", get(proxy_settings).post(save_proxy_settings))
        .route("/proxy/test", post(test_proxy_settings))
        .route("/jobs", get(jobs).post(create))
        .route(
            "/vectors",
            get(vectors)
                .post(import_vector)
                .layer(DefaultBodyLimit::max(2 * crate::vector::MAX_BYTES + 4096)),
        )
        .route("/vectors/{id}", get(inspect_vector))
        .route(
            "/vectors/import-file",
            post(import_vector_file).layer(DefaultBodyLimit::max(crate::vector::MAX_BYTES)),
        )
        .route("/vectors/{id}/source", get(vector_original))
        .route("/vectors/{id}/forget", post(forget_vector))
        .route(
            "/feature-services",
            get(feature_services).post(connect_feature_service),
        )
        .route(
            "/feature-services/{id}/forget",
            post(forget_feature_service),
        )
        .route("/feature-services/query", post(query_features))
        .route("/map-services", get(map_services).post(connect_map_service))
        .route("/map-services/{id}/forget", post(forget_map_service))
        .route("/map-images", get(map_images).post(get_map_image))
        .route("/map-images/{id}", get(inspect_map_image))
        .route("/map-images/{id}/export", get(export_map_image))
        .route("/projects", get(projects).post(create_project))
        .route("/projects/{id}/rename", post(rename_project))
        .route("/projects/{id}/scenes", post(add_project_scenes))
        .route("/projects/{id}/downloads", post(download_project))
        .route("/projects/{id}/mosaics", post(mosaic_project))
        .route("/projects/{id}/rasters", post(prepare_project))
        .route("/jobs/{id}", get(job))
        .route("/jobs/{id}/cancel", post(cancel))
        .route("/jobs/{id}/retry", post(retry))
        .route("/jobs/{id}/raster", get(raster))
        .route("/rasters/composite", post(composite))
        .route("/rasters/composite/pixel", post(composite_pixel))
        .route("/rasters/rgb/plan", post(plan_rgb))
        .route("/rasters/rgb", post(run_rgb))
        .route("/jobs/{id}/rgb", get(inspect_rgb))
        .route("/jobs/{id}/rgb/pixel", get(pixel_rgb))
        .route("/jobs/{id}/thumbnail", get(thumbnail))
        .route("/jobs/{id}/pixel", get(pixel))
        .route("/jobs/{id}/file", get(download_derived))
        .route("/jobs/{id}/metadata", get(download_mosaic_metadata))
        .route(
            "/jobs/{id}/package",
            get(download_artifact).post(prepare_artifact),
        )
        .route("/recipes", get(recipes).post(save_recipe))
        .route("/recipes/plan", post(plan_recipe))
        .route("/recipes/run", post(run_recipe))
        .merge(stac::routes())
        .merge(wcs::routes())
        .merge(tiles::routes())
        .merge(three_d::routes())
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
        .layer(middleware::from_fn(browser_boundary))
        .with_state(manager)
}

pub async fn serve(manager: JobManager, port: u16) -> crate::Result<()> {
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
        .await
        .map_err(|e| e.to_string())?;
    eprintln!(
        "GeoD runtime listening on http://127.0.0.1:{port}; storage {}",
        manager.storage_root().display()
    );
    axum::serve(listener, router(manager))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|e| e.to_string())
}
