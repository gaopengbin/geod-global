//! Public ArcGIS REST rendered snapshots, with exact discovery/export receipts.
//! These PNGs preserve server rendering, not scientific source band values.
use super::*;
use serde_json::Value;
const MAX_JSON: usize = 2 * 1024 * 1024;
const MAX_EXPORT: usize = 64 * 1024;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Capabilities {
    pub service_type: String,
    pub metadata: String,
    pub layers_metadata: Option<String>,
    pub excluded_layers: Vec<wmts::ExcludedLayer>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub capabilities: Capabilities,
    pub requested_bounds: [f64; 4],
    pub request_id: String,
    pub export_metadata: String,
    pub export_sha256: String,
    pub image_url: String,
}
fn json(raw: &str, max: usize) -> Result<Value> {
    if raw.is_empty() || raw.len() > max {
        return Err("ArcGIS response exceeds the size limit".into());
    }
    let v: Value = serde_json::from_str(raw).map_err(|_| "ArcGIS returned invalid JSON")?;
    if !v.is_object() {
        return Err("ArcGIS did not return a JSON object".into());
    }
    if let Some(e) = v.get("error") {
        let code = e["code"].as_i64().unwrap_or(0);
        return Err(if matches!(code, 498 | 499) {
            "This ArcGIS service requires authorization; choose a public service".into()
        } else {
            format!("ArcGIS service reported error {code}; no image was saved")
        });
    }
    Ok(v)
}
fn kind(root: &Url) -> Result<&'static str> {
    service_url(root.as_str())?;
    if !root.path().contains("/rest/services/") || root.path().contains('%') {
        return Err("Enter a public ArcGIS REST MapServer or ImageServer root URL".into());
    }
    match root.path().rsplit('/').next() {
        Some("MapServer") => Ok("MapServer"),
        Some("ImageServer") => Ok("ImageServer"),
        _ => Err("Enter a public ArcGIS REST MapServer or ImageServer root URL".into()),
    }
}
fn field(v: &Value, key: &str, max: usize) -> Result<String> {
    let s = v.get(key).and_then(Value::as_str).unwrap_or("").trim();
    if s.len() > max
        || s.chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return Err(format!("Invalid ArcGIS {key} metadata"));
    }
    Ok(s.into())
}
fn time_dimension(v: &Value) -> Result<Option<TimeDimension>> {
    if v.is_null() {
        return Ok(None);
    }
    let e = v["timeExtent"]
        .as_array()
        .filter(|a| a.len() == 2)
        .ok_or("ArcGIS time extent is unsupported")?;
    let millis = e
        .iter()
        .map(|n| {
            n.as_i64()
                .ok_or_else(|| "ArcGIS time extent is unsupported".into())
        })
        .collect::<Result<Vec<_>>>()?;
    if millis[0] > millis[1] {
        return Err("ArcGIS time extent is unsupported".into());
    }
    let dates = millis
        .iter()
        .map(|n| {
            chrono::DateTime::from_timestamp_millis(*n)
                .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
                .ok_or_else(|| "ArcGIS time extent is unsupported".into())
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Some(TimeDimension {
        values: dates.join("/"),
        default: Some(dates[1].clone()),
    }))
}
fn time_allowed(dim: &Option<TimeDimension>, value: Option<&str>) -> bool {
    match (dim, value) {
        (None, None) => true,
        (Some(d), Some(raw)) => {
            let ends: Vec<_> = d.values.split('/').filter_map(instant).collect();
            ends.len() == 2
                && instant(raw).is_some_and(|t| {
                    t >= ends[0] && t <= ends[1] && t.timestamp_subsec_nanos() % 1_000_000 == 0
                })
        }
        _ => false,
    }
}
fn extent(v: &Value) -> Result<[f64; 4]> {
    let sr = &v["spatialReference"];
    if sr["wkid"] != 4326
        || sr.get("latestWkid").is_some_and(|n| n != 4326)
        || sr.get("wkt").is_some()
    {
        return Err("ArcGIS returned a different image projection".into());
    }
    let mut b = [0.; 4];
    for (i, k) in ["xmin", "ymin", "xmax", "ymax"].iter().enumerate() {
        b[i] = v[k].as_f64().ok_or("Invalid ArcGIS extent")?;
    }
    features::bounds(b)?;
    Ok(b)
}
fn fingerprint(c: &Capabilities) -> String {
    let mut raw = c.metadata.as_bytes().to_vec();
    if let Some(l) = &c.layers_metadata {
        raw.push(0);
        raw.extend(l.as_bytes());
    }
    hash(&raw)
}
fn parse(
    root: &Url,
    name: &str,
    metadata: String,
    layers_metadata: Option<String>,
) -> Result<MapService> {
    let service_type = kind(root)?;
    let v = json(&metadata, MAX_JSON)?;
    let required = if service_type == "MapServer" {
        "Map"
    } else {
        "Image"
    };
    if !field(&v, "capabilities", 1024)?
        .split(',')
        .any(|c| c.trim() == required)
    {
        return Err("ArcGIS service does not advertise image export".into());
    }
    if v["hasMultidimensions"] == true || v["hasMultidimensionalInfo"] == true {
        return Err("Multidimensional ArcGIS imagery requires an explicit slice; this service is not supported yet".into());
    }
    let version = v["currentVersion"]
        .as_f64()
        .filter(|n| n.is_finite() && *n >= 10. && *n < 100.)
        .ok_or("Unsupported ArcGIS service version")?
        .to_string();
    let title = if service_type == "MapServer" {
        field(&v["documentInfo"], "Title", 1024)?
    } else {
        field(&v, "name", 1024)?
    };
    let title = if title.is_empty() { name.into() } else { title };
    let attribution = field(&v, "copyrightText", 1024)?;
    let access_constraints = field(&v["documentInfo"], "AccessConstraints", 16384)?;
    let mut excluded_layers = Vec::new();
    let mut layers = Vec::new();
    if service_type == "MapServer" {
        if !field(&v, "supportedImageFormatTypes", 1024)?
            .split(',')
            .any(|f| f.trim().eq_ignore_ascii_case("PNG32"))
        {
            return Err("ArcGIS service does not advertise PNG32 export".into());
        }
        let detail = json(
            layers_metadata
                .as_deref()
                .ok_or("ArcGIS layer metadata is missing")?,
            MAX_JSON,
        )?;
        let full = detail["layers"]
            .as_array()
            .filter(|a| a.len() <= 4096)
            .ok_or("ArcGIS layer metadata is invalid")?;
        let declared = v["layers"]
            .as_array()
            .filter(|a| !a.is_empty() && a.len() <= 4096)
            .ok_or("ArcGIS service has no layers")?;
        let mut seen = BTreeSet::new();
        for l in declared {
            let id = l["id"]
                .as_u64()
                .filter(|n| *n <= i32::MAX as u64)
                .ok_or("Invalid ArcGIS layer identifier")?;
            if !seen.insert(id) {
                return Err("Duplicate ArcGIS layer identifier".into());
            }
            let label = field(l, "name", 1024)?;
            if !text_valid(&label, 1024) {
                return Err("Invalid ArcGIS layer title".into());
            }
            let matches: Vec<_> = full
                .iter()
                .filter(|d| d["id"].as_u64() == Some(id))
                .collect();
            if matches.len() != 1 || matches[0]["name"].as_str() != l["name"].as_str() {
                return Err("ArcGIS layer discovery changed; reconnect the service".into());
            }
            let d = matches[0];
            if l["subLayerIds"].as_array().is_some_and(|a| !a.is_empty())
                || d["type"] == "Group Layer"
            {
                excluded_layers.push(wmts::ExcludedLayer {
                    name: label,
                    reason: "Choose individual layers instead of a group layer".into(),
                });
                continue;
            }
            if d["rangeInfos"].as_array().is_some_and(|a| !a.is_empty())
                || d["hasMultidimensions"] == true
            {
                excluded_layers.push(wmts::ExcludedLayer {
                    name: label,
                    reason: "This layer requires unsupported range or dimension selections".into(),
                });
                continue;
            }
            let time = match time_dimension(&d["timeInfo"]) {
                Ok(t) => t,
                Err(_) => {
                    excluded_layers.push(wmts::ExcludedLayer {
                        name: label,
                        reason: "ArcGIS time extent is unsupported".into(),
                    });
                    continue;
                }
            };
            let copyright = field(d, "copyrightText", 1024)?;
            layers.push(MapLayer {
                name: id.to_string(),
                title: label,
                description: field(d, "description", 16384)?,
                crs: "EPSG:4326".into(),
                styles: vec![],
                time,
                bounds: if d["extent"]["spatialReference"]["wkid"] == 4326 {
                    Some(extent(&d["extent"])?)
                } else {
                    None
                },
                attribution: if !copyright.is_empty() {
                    Some(copyright)
                } else if !attribution.is_empty() {
                    Some(attribution.clone())
                } else {
                    None
                },
                wmts: None,
            });
        }
    } else {
        if layers_metadata.is_some() {
            return Err("Unexpected ArcGIS image layer metadata".into());
        }
        let time = time_dimension(&v["timeInfo"])?;
        if v["hasLiveData"] == true && time.is_none() {
            return Err("ArcGIS live imagery requires a declared time extent".into());
        }
        layers.push(MapLayer {
            name: "image".into(),
            title: title.clone(),
            description: field(&v, "description", 16384)?,
            crs: "EPSG:4326".into(),
            styles: vec![],
            time,
            bounds: if v["fullExtent"]["spatialReference"]["wkid"] == 4326 {
                Some(extent(&v["fullExtent"])?)
            } else {
                None
            },
            attribution: if attribution.is_empty() {
                None
            } else {
                Some(attribution)
            },
            wmts: None,
        });
    }
    if layers.is_empty() {
        return Err("ArcGIS service has no supported renderable layers".into());
    }
    let size = |k| {
        v[k].as_u64()
            .filter(|n| *n > 0 && *n <= u32::MAX as u64)
            .map(|n| (n as u32).min(MAX_EDGE))
            .ok_or_else(|| format!("ArcGIS does not declare a valid {k}"))
    };
    let c = Capabilities {
        service_type: service_type.into(),
        metadata,
        layers_metadata,
        excluded_layers,
    };
    Ok(MapService {
        xyz: None,
        id: Uuid::new_v4().to_string(),
        name: name.into(),
        url: root.to_string(),
        title,
        version,
        map_url: format!(
            "{}/{}",
            root,
            if service_type == "MapServer" {
                "export"
            } else {
                "exportImage"
            }
        ),
        layers,
        access_constraints,
        max_width: size("maxImageWidth")?,
        max_height: size("maxImageHeight")?,
        capabilities_sha256: fingerprint(&c),
        connected_at: now(),
        wmts: None,
        arcgis: Some(c),
    })
}
pub(super) fn validate_service(s: &MapService) -> Result<()> {
    let c = s.arcgis.as_ref().ok_or("Missing ArcGIS metadata")?;
    if s.wmts.is_some()
        || !uuid_valid(&s.id)
        || !text_valid(&s.name, 80)
        || instant(&s.connected_at).is_none()
    {
        return Err("Invalid saved ArcGIS service".into());
    }
    let root = service_url(&s.url)?;
    let mut expected = parse(
        &root,
        &s.name,
        c.metadata.clone(),
        c.layers_metadata.clone(),
    )?;
    expected.id = s.id.clone();
    expected.connected_at = s.connected_at.clone();
    if *s != expected {
        return Err("ArcGIS service metadata changed; reconnect the service".into());
    }
    Ok(())
}
fn export_url(
    s: &MapService,
    l: &MapLayer,
    bounds: [f64; 4],
    size: [u32; 2],
    time: Option<&str>,
    request_id: &str,
) -> Result<Url> {
    features::bounds(bounds)?;
    if !uuid_valid(request_id) {
        return Err("Invalid ArcGIS request identifier".into());
    }
    if !time_allowed(&l.time, time) {
        return Err("Choose an explicit time supported by the ArcGIS layer".into());
    }
    let mut u = Url::parse(&s.map_url).map_err(io_error)?;
    u.query_pairs_mut()
        .append_pair("f", "json")
        // An ignored client parameter prevents cached export JSON from pointing
        // at expired generated images. It is persisted and validated verbatim.
        .append_pair("_geodRequest", request_id)
        .append_pair("bbox", &bounds.map(|n| n.to_string()).join(","))
        .append_pair("bboxSR", "4326")
        .append_pair("imageSR", "4326")
        .append_pair("size", &format!("{},{}", size[0], size[1]))
        .append_pair("format", "png32")
        .append_pair("transparent", "true");
    if s.arcgis
        .as_ref()
        .is_some_and(|c| c.service_type == "MapServer")
    {
        u.query_pairs_mut()
            .append_pair("layers", &format!("show:{}", l.name))
            .append_pair("dpi", "96")
            .append_pair("rotation", "0");
    } else {
        u.query_pairs_mut().append_pair("adjustAspectRatio", "true");
    }
    if let Some(t) = time {
        u.query_pairs_mut().append_pair(
            "time",
            &instant(t)
                .ok_or("Invalid ArcGIS time")?
                .timestamp_millis()
                .to_string(),
        );
    }
    Ok(u)
}
fn image_url(root: &Url, raw: &str) -> Result<Url> {
    kind(root)?;
    let u = features::public_url(raw)?;
    if u.origin() != root.origin()
        || u.query().is_some()
        || u.fragment().is_some()
        || u.path().contains('%')
    {
        return Err("ArcGIS output image must stay on the connected public service origin".into());
    }
    let (base, tail) = root
        .path()
        .split_once("/rest/services/")
        .ok_or("Invalid ArcGIS service URL")?;
    let service_folder = tail.replace('/', "_");
    let directories = format!("{base}/rest/directories/");
    let output = format!("{base}/arcgisoutput/");
    let path = if let Some(p) = u.path().strip_prefix(&directories) {
        let parts: Vec<_> = p.split('/').collect();
        if !(2..=3).contains(&parts.len())
            || parts[0].is_empty()
            || !parts[0]
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
            || (parts.len() == 3 && parts[1] != service_folder)
        {
            return Err("ArcGIS image URL is outside the generated output directory".into());
        }
        parts.last().copied().unwrap_or("")
    } else if let Some(p) = u
        .path()
        .strip_prefix(&output)
        .or_else(|| u.path().strip_prefix("/arcgisoutput/"))
    {
        let parts: Vec<_> = p.split('/').collect();
        if parts.len() > 2 || (parts.len() == 2 && parts[0] != service_folder) {
            return Err("ArcGIS image URL is outside the generated output directory".into());
        }
        parts.last().copied().unwrap_or("")
    } else {
        return Err("ArcGIS image URL is outside the generated output directory".into());
    };
    if !path.starts_with("_ags_")
        || !path.ends_with(".png")
        || path.len() > 180
        || !path
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.'))
    {
        return Err("ArcGIS did not return a supported generated PNG URL".into());
    }
    Ok(u)
}
fn export_response(
    root: &Url,
    raw: &str,
    size: [u32; 2],
    requested: [f64; 4],
) -> Result<(Url, [f64; 4])> {
    let v = json(raw, MAX_EXPORT)?;
    if v["width"].as_u64() != Some(u64::from(size[0]))
        || v["height"].as_u64() != Some(u64::from(size[1]))
    {
        return Err("ArcGIS returned different image dimensions".into());
    }
    let b = extent(&v["extent"])?;
    // ArcGIS may expand the bounding rectangle to preserve aspect ratio. Reject
    // cropping or unrelated returned extents; allow floating-point roundoff.
    let eps = 1e-9;
    if b[0] > requested[0] + eps
        || b[1] > requested[1] + eps
        || b[2] < requested[2] - eps
        || b[3] < requested[3] - eps
        || ((b[0] + b[2]) - (requested[0] + requested[2])).abs() > eps
        || ((b[1] + b[3]) - (requested[1] + requested[3])).abs() > eps
    {
        return Err("ArcGIS returned an unexpected image extent".into());
    }
    let href = v["href"]
        .as_str()
        .ok_or("ArcGIS export response has no image URL")?;
    Ok((image_url(root, href)?, b))
}
pub(super) fn validate_asset(a: &MapImage) -> Result<()> {
    let s = &a.source;
    let p = s.arcgis.as_ref().ok_or("Missing ArcGIS export metadata")?;
    if s.wmts.is_some()
        || a.image_extent.is_some()
        || !uuid_valid(&a.id)
        || !text_valid(&a.name, 120)
        || a.bytes == 0
        || a.bytes > MAX_PNG
        || !digest_valid(&a.sha256)
        || a.crs != "EPSG:4326"
        || s.request_crs != "EPSG:4326"
        || !s.style.is_empty()
        || s.selection != "bbox-rendered-map"
        || instant(&s.requested_at).is_none()
        || !text_valid(&s.service_name, 80)
    {
        return Err("Invalid saved ArcGIS map image".into());
    }
    let root = service_url(&s.service_url)?;
    let service = parse(
        &root,
        &s.service_name,
        p.capabilities.metadata.clone(),
        p.capabilities.layers_metadata.clone(),
    )?;
    let l = service
        .layers
        .iter()
        .find(|l| l.name == s.layer_name)
        .ok_or("Saved ArcGIS layer is not declared by the service")?;
    if service.arcgis.as_ref() != Some(&p.capabilities)
        || service.title != s.service_title
        || service.version != s.version
        || service.map_url != s.map_endpoint
        || service.capabilities_sha256 != s.capabilities_sha256
        || service.access_constraints != s.access_constraints
        || l.title != s.layer_title
        || l.attribution != s.attribution
        || a.width == 0
        || a.height == 0
        || a.width > service.max_width
        || a.height > service.max_height
        || hash(p.export_metadata.as_bytes()) != p.export_sha256
        || export_url(
            &service,
            l,
            p.requested_bounds,
            [a.width, a.height],
            s.time.as_deref(),
            &p.request_id,
        )?
        .as_str()
            != s.request_url
    {
        return Err("ArcGIS request or discovery metadata changed".into());
    }
    let (url, b) = export_response(
        &root,
        &p.export_metadata,
        [a.width, a.height],
        p.requested_bounds,
    )?;
    if url.as_str() != p.image_url || b != a.bounds {
        return Err("ArcGIS image grid does not match the export response".into());
    }
    if let Some(area) = &s.area_geometry {
        let b = area.bounds()?;
        if b[0] < p.requested_bounds[0]
            || b[1] < p.requested_bounds[1]
            || b[2] > p.requested_bounds[2]
            || b[3] > p.requested_bounds[3]
        {
            return Err("Map bounds must include the selected polygon".into());
        }
    }
    Ok(())
}
async fn fetch_json(c: &reqwest::Client, u: &Url, max: usize) -> Result<String> {
    let r = c
        .get(u.clone())
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|_| "Cannot reach ArcGIS service; check the service and proxy settings")?;
    if !r.status().is_success() {
        return Err(format!(
            "ArcGIS service returned HTTP {}",
            r.status().as_u16()
        ));
    }
    let media = r
        .headers()
        .get("content-type")
        .and_then(|s| s.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim();
    // ArcGIS REST's f=json is commonly served as text/plain by older servers.
    if !matches!(media, "application/json" | "text/plain" | "text/json") {
        return Err("ArcGIS did not return JSON".into());
    }
    if r.content_length().is_some_and(|n| n > max as u64) {
        return Err("ArcGIS response exceeds the size limit".into());
    }
    let mut bytes = Vec::new();
    let mut stream = r.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let b = chunk.map_err(|_| "ArcGIS response was interrupted")?;
        if b.len() > max - bytes.len() {
            return Err("ArcGIS response exceeds the size limit".into());
        }
        bytes.extend_from_slice(&b);
    }
    let raw = String::from_utf8(bytes).map_err(|_| "ArcGIS response is not UTF-8")?;
    json(&raw, max)?;
    Ok(raw)
}
pub(super) async fn connect(
    root: &Url,
    name: &str,
    settings: &crate::proxy::ProxySettings,
) -> Result<MapService> {
    let service_type = kind(root)?;
    let (metadata, layers) = tokio::time::timeout(Duration::from_secs(60), async {
        let c = features::client(root, settings).await?;
        let mut u = root.clone();
        u.query_pairs_mut().append_pair("f", "json");
        let metadata = fetch_json(&c, &u, MAX_JSON).await?;
        let layers = if service_type == "MapServer" {
            let mut u = Url::parse(&format!("{root}/layers")).map_err(io_error)?;
            u.query_pairs_mut().append_pair("f", "json");
            Some(fetch_json(&c, &u, MAX_JSON).await?)
        } else {
            None
        };
        Ok::<_, String>((metadata, layers))
    })
    .await
    .map_err(|_| "ArcGIS service discovery timed out")??;
    let root = root.clone();
    let name = name.to_string();
    tokio::task::spawn_blocking(move || {
        let s = parse(&root, &name, metadata, layers)?;
        validate_service(&s)?;
        Ok(s)
    })
    .await
    .map_err(io_error)?
}
pub(super) async fn get(
    manager: &JobManager,
    r: MapRequest,
    service: MapService,
) -> Result<MapImage> {
    validate_service(&service)?;
    if !r.style.is_empty() || r.tile_matrix.is_some() || r.tile_matrix_set.is_some() {
        return Err("ArcGIS snapshots use the default service rendering; tile or style overrides are unsupported".into());
    }
    if r.width == 0 || r.height == 0 || r.width > service.max_width || r.height > service.max_height
    {
        return Err(
            "Choose image dimensions within the advertised limit, up to 2048 per edge".into(),
        );
    }
    let layer = service
        .layers
        .iter()
        .find(|l| l.name == r.layer_name)
        .ok_or("Unknown ArcGIS layer")?;
    if let Some(area) = &r.area_geometry {
        let b = area.bounds()?;
        if b[0] < r.bounds[0] || b[1] < r.bounds[1] || b[2] > r.bounds[2] || b[3] > r.bounds[3] {
            return Err("Map bounds must include the selected polygon".into());
        }
    }
    let root = service_url(&service.url)?;
    let request_id = Uuid::new_v4().to_string();
    let request = export_url(
        &service,
        layer,
        r.bounds,
        [r.width, r.height],
        r.time.as_deref(),
        &request_id,
    )?;
    let _permit = manager
        .inner
        .thumbnail_permits
        .acquire()
        .await
        .map_err(io_error)?;
    let settings = manager.proxy_settings().await;
    let (raw, url, bounds, bytes) = tokio::time::timeout(Duration::from_secs(60), async {
        let c = features::client(&root, &settings).await?;
        let raw = fetch_json(&c, &request, MAX_EXPORT).await?;
        let (url, bounds) = export_response(&root, &raw, [r.width, r.height], r.bounds)?;
        let bytes = fetch(&c, &url, MAX_PNG, true).await?;
        Ok::<_, String>((raw, url, bounds, bytes))
    })
    .await
    .map_err(|_| "ArcGIS retrieval timed out; no image was registered")??;
    let a = MapImage {
        id: Uuid::new_v4().to_string(),
        name: format!(
            "{} · ArcGIS",
            layer.title.chars().take(110).collect::<String>()
        ),
        width: r.width,
        height: r.height,
        bounds,
        bytes: bytes.len(),
        sha256: hash(&bytes),
        crs: "EPSG:4326".into(),
        image_extent: None,
        source: MapSource {
            xyz: None,
            service_url: service.url.clone(),
            service_name: service.name.clone(),
            service_title: service.title.clone(),
            version: service.version.clone(),
            map_endpoint: service.map_url.clone(),
            capabilities_sha256: service.capabilities_sha256.clone(),
            layer_name: layer.name.clone(),
            layer_title: layer.title.clone(),
            style: String::new(),
            time: r.time,
            request_crs: "EPSG:4326".into(),
            request_url: request.to_string(),
            requested_at: now(),
            access_constraints: service.access_constraints.clone(),
            attribution: layer.attribution.clone(),
            area_geometry: r.area_geometry,
            selection: "bbox-rendered-map".into(),
            wmts: None,
            arcgis: Some(Snapshot {
                capabilities: service.arcgis.clone().ok_or("Missing ArcGIS metadata")?,
                requested_bounds: r.bounds,
                request_id,
                export_sha256: hash(raw.as_bytes()),
                export_metadata: raw,
                image_url: url.to_string(),
            }),
        },
    };
    let (a, bytes) = tokio::task::spawn_blocking(move || {
        validate_asset(&a)?;
        validate_png(&bytes, a.width, a.height)?;
        Ok::<_, String>((a, bytes))
    })
    .await
    .map_err(io_error)??;
    manager.save_map_image(a, bytes, None).await
}
