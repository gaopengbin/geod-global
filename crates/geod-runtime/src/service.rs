//! Loopback development adapter; desktop commands use JobManager directly.
use crate::{CreateJobRequest, JobManager};
use axum::{
    extract::{DefaultBodyLimit, Path, Request, State},
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
async fn jobs(State(manager): State<JobManager>) -> Json<Vec<crate::Job>> {
    Json(manager.list().await)
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

pub fn router(manager: JobManager) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/jobs", get(jobs).post(create))
        .route("/jobs/{id}/cancel", post(cancel))
        .route("/jobs/{id}/retry", post(retry))
        .layer(DefaultBodyLimit::max(8192))
        .layer(middleware::from_fn(browser_boundary))
        .with_state(manager)
}

pub async fn serve(manager: JobManager, port: u16) -> crate::Result<()> {
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
        .await
        .map_err(|e| e.to_string())?;
    println!(
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
