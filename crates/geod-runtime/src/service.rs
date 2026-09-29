//! Loopback development adapter; desktop commands use JobManager directly.
use crate::{CreateJobRequest, CreateProjectRequest, JobManager, RasterRecipe};
use axum::{
    extract::{DefaultBodyLimit, Path, Query, Request, State},
    http::{header, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::json;

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
            HeaderValue::from_static("Content-Type, X-GeoD-Client"),
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

async fn diagnostics(State(manager): State<JobManager>) -> Json<serde_json::Value> {
    Json(manager.diagnostics().await)
}
async fn jobs(State(manager): State<JobManager>) -> Json<Vec<crate::Job>> {
    Json(manager.list().await)
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectDownloadRequest {
    asset_key: String,
}
async fn download_project(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
    Json(request): Json<ProjectDownloadRequest>,
) -> std::result::Result<(StatusCode, Json<crate::ProjectDownloads>), ApiError> {
    manager
        .enqueue_project(&id, &request.asset_key)
        .await
        .map(|downloads| (StatusCode::ACCEPTED, Json(downloads)))
        .map_err(api_error)
}
async fn mosaic_project(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
    Json(request): Json<ProjectDownloadRequest>,
) -> std::result::Result<(StatusCode, Json<crate::Job>), ApiError> {
    manager
        .run_project_mosaic(&id, &request.asset_key)
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

async fn raster(
    State(manager): State<JobManager>,
    Path(id): Path<String>,
) -> std::result::Result<Json<crate::RasterInspection>, ApiError> {
    manager
        .inspect_raster(&id)
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
    let (filename, bytes) = manager
        .mosaic_metadata_bytes(&id)
        .await
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
        .route("/diagnostics", get(diagnostics))
        .route("/jobs", get(jobs).post(create))
        .route("/projects", get(projects).post(create_project))
        .route("/projects/{id}/downloads", post(download_project))
        .route("/projects/{id}/mosaics", post(mosaic_project))
        .route("/jobs/{id}", get(job))
        .route("/jobs/{id}/cancel", post(cancel))
        .route("/jobs/{id}/retry", post(retry))
        .route("/jobs/{id}/raster", get(raster))
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
