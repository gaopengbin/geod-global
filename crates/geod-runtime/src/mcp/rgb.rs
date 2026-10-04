//! Scientific RGB shares the same native task, file validation and export path.
use super::{arguments, to_value, validate_id, Backend, ErrorData, IdArgs, PixelArgs};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    request: crate::RgbRequest,
}
pub(super) enum Operation {
    Plan(crate::RgbRequest),
    Run(crate::RgbRequest),
    Inspect(String),
    Pixel(PixelArgs),
    Package(String),
}
pub(super) fn parse(name: &str, value: Value) -> Result<Operation, ErrorData> {
    Ok(match name {
        "geod_rgb_plan" | "geod_rgb_run" => {
            let Request { request } = arguments(value)?;
            for id in &request.job_ids {
                validate_id(id)?;
            }
            if let Some(id) = &request.project_id {
                validate_id(id)?;
            }
            if let Some(mask) = &request.quality_mask {
                let [first, second] = mask.job_ids();
                validate_id(first)?;
                validate_id(second)?;
                if first == second || request.job_ids.iter().any(|id| id == first || id == second) {
                    return Err(ErrorData::invalid_params(
                        "Two distinct product-specific quality-layer job IDs are required",
                        None,
                    ));
                }
            }
            if request
                .job_ids
                .iter()
                .enumerate()
                .any(|(i, id)| request.job_ids[..i].contains(id))
            {
                return Err(ErrorData::invalid_params(
                    "Three distinct red, green, blue local job IDs are required",
                    None,
                ));
            }
            if name == "geod_rgb_plan" {
                Operation::Plan(request)
            } else {
                Operation::Run(request)
            }
        }
        "geod_rgb_inspect" | "geod_rgb_package" => {
            let IdArgs { id } = arguments(value)?;
            validate_id(&id)?;
            if name == "geod_rgb_inspect" {
                Operation::Inspect(id)
            } else {
                Operation::Package(id)
            }
        }
        "geod_rgb_pixel" => {
            let point: PixelArgs = arguments(value)?;
            validate_id(&point.id)?;
            if !point.x.is_finite() || !point.y.is_finite() {
                return Err(ErrorData::invalid_params(
                    "RGB coordinates must be finite source-CRS coordinates",
                    None,
                ));
            }
            Operation::Pixel(point)
        }
        _ => {
            return Err(ErrorData::invalid_params(
                "Unknown scientific RGB tool",
                None,
            ))
        }
    })
}
pub(super) async fn execute(backend: &Backend, operation: Operation) -> crate::Result<Value> {
    use reqwest::Method;
    match operation {
        Operation::Plan(r) => match backend {
            Backend::Direct(m) => to_value(Box::pin(m.plan_scientific_rgb(r)).await?),
            Backend::Server { .. } => {
                backend
                    .http(Method::POST, "/rasters/rgb/plan", Some(to_value(r)?))
                    .await
            }
        },
        Operation::Run(r) => {
            let job = match backend {
                Backend::Direct(m) => to_value(Box::pin(m.run_scientific_rgb(r)).await?)?,
                Backend::Server { .. } => {
                    backend
                        .http(Method::POST, "/rasters/rgb", Some(to_value(r)?))
                        .await?
                }
            };
            backend.submitted(job).await
        }
        Operation::Inspect(id) => {
            let mut value = match backend {
                Backend::Direct(m) => to_value(Box::pin(m.inspect_scientific_rgb(&id)).await?)?,
                Backend::Server { .. } => {
                    backend
                        .http(Method::GET, &format!("/jobs/{id}/rgb"), None)
                        .await?
                }
            };
            value
                .as_object_mut()
                .ok_or("Invalid RGB inspection")?
                .remove("previewDataUrl");
            value["previewOmitted"] = json!(true);
            Ok(value)
        }
        Operation::Pixel(p) => match backend {
            Backend::Direct(m) => {
                to_value(Box::pin(m.sample_scientific_rgb(&p.id, p.x, p.y)).await?)
            }
            Backend::Server { .. } => {
                backend
                    .http(
                        Method::GET,
                        &format!("/jobs/{}/rgb/pixel?x={}&y={}", p.id, p.x, p.y),
                        None,
                    )
                    .await
            }
        },
        Operation::Package(id) => match backend {
            Backend::Direct(m) => to_value(Box::pin(m.prepare_artifact(&id)).await?),
            Backend::Server { .. } => {
                backend
                    .http(Method::POST, &format!("/jobs/{id}/package"), None)
                    .await
            }
        },
    }
}
pub(super) fn tools(write: bool, id: &Value) -> Vec<(&'static str, &'static str, Value)> {
    let mut request = json!({"type":"object","properties":{"request":{"type":"object","properties":{"jobIds":{"type":"array","minItems":3,"maxItems":3,"uniqueItems":true,"items":id["properties"]["id"].clone(),"description":"Completed red, green, blue jobs in that order; same scene or same processed project/grid"},"projectId":id["properties"]["id"].clone(),"name":{"type":"string","minLength":1,"maxLength":512}},"required":["jobIds"],"additionalProperties":false}},"required":["request"],"additionalProperties":false});
    let modis = json!({"type":"object","properties":{
        "qcJobId":id["properties"]["id"].clone(),"stateJobId":id["properties"]["id"].clone(),
        "policy":{"type":"string","enum":["clear","clear_best"],"description":"Explicit clear state, no shadow/cirrus/internal/adjacent cloud; clear_best also requires ideal MODLAND and best RGB band QC"},
        "excludeSnow":{"type":"boolean","default":false,"description":"Exclude MOD35 snow bit 12 and internal snow bit 15"}},
        "required":["qcJobId","stateJobId","policy"],"additionalProperties":false,
        "description":"MODIS v061 screening using matched QC/state; project mosaics use coherent original-scene selection. Accepted DN retained, rejected pixels become NoData."});
    let landsat = json!({"type":"object","properties":{
        "qaPixelJobId":id["properties"]["id"].clone(),"qaRadsatJobId":id["properties"]["id"].clone(),
        "policy":{"type":"string","enum":["cloud_free","cloud_free_conservative"],"description":"Reject fill, dilated/high cloud, high cirrus/shadow, RGB band saturation and terrain occlusion. Conservative also requires clear bit, explicit low cloud/shadow/cirrus confidence and unused RADSAT bits zero."},
        "excludeSnow":{"type":"boolean","default":false,"description":"Reject high snow; conservative additionally requires explicit low snow confidence."}},
        "required":["qaPixelJobId","qaRadsatJobId","policy"],"additionalProperties":false,
        "description":"Landsat 8/9 C2 L2 matched same-scene QA_PIXEL/QA_RADSAT. Water and non-RGB-band saturation alone remain accepted. No resampling."});
    request["properties"]["request"]["properties"]["qualityMask"] =
        json!({"oneOf":[modis,landsat]});
    let mut point = id.clone();
    point["properties"]["x"] =
        json!({"type":"number","description":"X in the saved RGB source CRS"});
    point["properties"]["y"] =
        json!({"type":"number","description":"Y in the saved RGB source CRS"});
    point["required"] = json!(["id", "x", "y"]);
    let mut tools=vec![("geod_rgb_plan","Read-only preflight of three verified local reflectance bands. Reports exact grid, calibration, source pins and required disk space; no resampling or output is performed.",request.clone()),("geod_rgb_inspect","Verify a completed managed scientific RGB's own 16-bit GeoTIFF, calibration, grid and file checksum. Works after parent files are removed. Display PNG is omitted.",id.clone()),("geod_rgb_pixel","Read raw RGB DN, per-channel NoData and reflectance from the completed scientific RGB at an original-grid coordinate. No resampling or external requests.",point)];
    if write {
        tools.extend([("geod_rgb_run","Queue a native three-band Int16/UInt16 RGB GeoTIFF from pinned jobs. Optional product-specific MODIS or Landsat qualityMask retains accepted DN and sets rejected pixels to NoData. Without a mask all DN remain unchanged. Calibration/grid retained, all output samples checked. Poll geod_job_status until terminal AND settled=true; only succeeded is ready.",request),("geod_rgb_package","Prepare a verified local ZIP with the completed RGB GeoTIFF, display preview, source provenance and checksums. Returns its managed local path; source originals are not included. No external requests or arbitrary output paths.",id.clone())]);
    }
    tools
}
