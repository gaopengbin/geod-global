//! Private desktop protocol for immutable STAC asset ranges, not a URL proxy.
use geod_runtime::{stac::preview::protocol_selection, JobManager};
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
                .expect("Fixed STAC preview preflight")
        } else {
            let range = request
                .headers()
                .get(http::header::RANGE)
                .and_then(|value| value.to_str().ok());
            let result = if request.method() != http::Method::GET
                || request.uri().query().is_some()
                || !request.body().is_empty()
                || request
                    .headers()
                    .get_all(http::header::RANGE)
                    .iter()
                    .count()
                    != 1
            {
                Err("Only a single bounded STAC TIFF Range read is supported".into())
            } else if let Some(range) = range {
                match protocol_selection(request.uri().path()) {
                    Ok((snapshot, key)) => manager.read_stac_preview(&snapshot, &key, range).await,
                    Err(error) => Err(error),
                }
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
                    .expect("Fixed STAC preview error headers"),
            }
        };
        responder.respond(response);
    });
}
