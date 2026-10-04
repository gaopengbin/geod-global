//! Private desktop protocol for reviewed public DEM byte ranges, not a URL proxy.
use geod_runtime::JobManager;
use tauri::{http, Manager};

pub fn handle<R: tauri::Runtime>(
    context: tauri::UriSchemeContext<'_, R>,
    request: http::Request<Vec<u8>>,
    responder: tauri::UriSchemeResponder,
) {
    let manager = context.app_handle().state::<JobManager>().inner().clone();
    tauri::async_runtime::spawn(async move {
        let response = if request.method() == http::Method::OPTIONS {
            http::Response::builder()
                .status(204)
                .header("access-control-allow-origin", "*")
                .header("access-control-allow-methods", "GET, OPTIONS")
                .header("access-control-allow-headers", "Range")
                .body(Vec::new())
                .expect("Fixed preview preflight")
        } else {
            let item = request.uri().path().strip_prefix('/').unwrap_or("");
            let range = request
                .headers()
                .get(http::header::RANGE)
                .and_then(|value| value.to_str().ok());
            let result = if request.method() != http::Method::GET
                || request.uri().query().is_some()
                || !request.body().is_empty()
            {
                Err("Only public elevation Range reads are supported".into())
            } else if let Some(range) = range {
                manager.read_elevation_preview(item, range).await
            } else {
                Err("A bounded Range header is required".into())
            };
            match result {
                Ok(data) => data.into_response(),
                Err(error) => http::Response::builder()
                    .status(400)
                    .header("access-control-allow-origin", "*")
                    .header("content-type", "text/plain; charset=utf-8")
                    .header("x-content-type-options", "nosniff")
                    .body(error.into_bytes())
                    .expect("Fixed preview error headers"),
            }
        };
        responder.respond(response);
    });
}
