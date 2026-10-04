//! Explicit global Web Mercator XYZ / bottom-origin TMS rendered tiles.
//! Configuration is user supplied, never inferred from a URL or capabilities.
use super::*;
const VERSION: &str = "xyz-1";
const GRID: &str = "WebMercator";
const HALF_WORLD: f64 = std::f64::consts::PI * 6_378_137.;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GridOptions {
    pub tile_size: u32,
    pub min_zoom: u32,
    pub max_zoom: u32,
    pub zoom_offset: i32,
    pub format: String,
    pub attribution: String,
    pub access_constraints: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Configuration {
    pub scheme: String,
    pub url_template: String,
    pub grid: GridOptions,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub configuration: Configuration,
    pub logical_zoom: u32,
    pub matrix_set: String,
    pub matrix: wmts::Matrix,
    pub requested_bounds: [f64; 4],
    pub pixel_window: [u64; 4],
    /// Rows always use the top-origin logical grid; URLs record actual TMS Y.
    pub tiles: Vec<wmts::TileReceipt>,
    pub archive_sha256: String,
    pub archive_bytes: usize,
}
fn valid_template_path(path: &str) -> bool {
    path.split('/').all(|segment| {
        let bytes = segment.as_bytes();
        let mut decoded = Vec::with_capacity(bytes.len());
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == b'%' {
                let Some(pair) = bytes.get(index + 1..index + 3) else {
                    return false;
                };
                let (Some(high), Some(low)) = (
                    char::from(pair[0]).to_digit(16),
                    char::from(pair[1]).to_digit(16),
                ) else {
                    return false;
                };
                let value = (high * 16 + low) as u8;
                // Encoded layer names are valid. Separators, placeholders and
                // a second decoding layer must not change the template shape.
                if matches!(value, b'/' | b'\\' | b'%' | b'?' | b'#' | b'{' | b'}') {
                    return false;
                }
                decoded.push(value);
                index += 3;
            } else {
                if bytes[index] == b'\\' {
                    return false;
                }
                decoded.push(bytes[index]);
                index += 1;
            }
        }
        std::str::from_utf8(&decoded)
            .is_ok_and(|value| !matches!(value, "." | "..") && !value.chars().any(char::is_control))
    })
}

fn template_url(raw: &str) -> Result<Url> {
    if !text_valid(raw, 2048)
        || ["{z}", "{x}", "{y}"]
            .iter()
            .any(|token| raw.matches(token).count() != 1)
    {
        return Err("Use one {z}, {x} and {y} in the tile URL path".into());
    }
    let substituted = raw
        .replace("{z}", "0")
        .replace("{x}", "0")
        .replace("{y}", "0");
    if substituted.contains(['{', '}']) {
        return Err("Unsupported tile URL placeholder".into());
    }
    let u = features::public_url(&substituted)?;
    let path = raw
        .split_once("://")
        .and_then(|(_, rest)| rest.split_once('/'))
        .map(|(_, p)| p)
        .unwrap_or("");
    if u.query().is_some()
        || u.fragment().is_some()
        || !valid_template_path(path)
        || ["{z}", "{x}", "{y}"].iter().any(|t| !path.contains(t))
    {
        return Err("Tile templates require a public HTTPS path without query parameters".into());
    }
    let host = u.host_str().unwrap_or("");
    if host == "tile.openstreetmap.org" || host.ends_with(".tile.openstreetmap.org") {
        return Err(
            "The standard OpenStreetMap tile server does not permit offline downloads".into(),
        );
    }
    Url::parse(raw).map_err(io_error)
}

fn validate_configuration(c: &Configuration) -> Result<()> {
    template_url(&c.url_template)?;
    let g = &c.grid;
    if !matches!(c.scheme.as_str(), "XYZ" | "TMS")
        || !matches!(g.tile_size, 256 | 512)
        || g.min_zoom > g.max_zoom
        || g.max_zoom > 24
        || !(-2..=2).contains(&g.zoom_offset)
        || i64::from(g.min_zoom) + i64::from(g.zoom_offset) < 0
        || !matches!(g.format.as_str(), "image/png" | "image/jpeg")
        || g.attribution.len() > 4096
        || g.access_constraints.len() > 16384
        || (!g.attribution.is_empty() && !text_valid(&g.attribution, 1024))
        || (!g.access_constraints.is_empty() && !text_valid(&g.access_constraints, 16384))
    {
        return Err("Invalid XYZ/TMS grid configuration".into());
    }
    Ok(())
}
fn fingerprint(c: &Configuration) -> Result<String> {
    Ok(hash(&serde_json::to_vec(c).map_err(io_error)?))
}
fn service(name: &str, c: Configuration) -> Result<MapService> {
    validate_configuration(&c)?;
    if !text_valid(name, 80) {
        return Err("Enter a map service name of 1–80 characters".into());
    }
    let url = template_url(&c.url_template)?.to_string();
    Ok(MapService {
        id: Uuid::new_v4().to_string(),
        name: name.into(),
        url: url.clone(),
        title: name.into(),
        version: VERSION.into(),
        map_url: url,
        layers: vec![MapLayer {
            name: "tiles".into(),
            title: name.into(),
            description:
                "Configured global Web Mercator rendered tile grid; no capability discovery".into(),
            crs: "EPSG:3857".into(),
            styles: Vec::new(),
            time: None,
            bounds: None,
            attribution: if c.grid.attribution.is_empty() {
                None
            } else {
                Some(c.grid.attribution.clone())
            },
            wmts: None,
        }],
        access_constraints: c.grid.access_constraints.clone(),
        max_width: MAX_EDGE,
        max_height: MAX_EDGE,
        capabilities_sha256: fingerprint(&c)?,
        connected_at: now(),
        wmts: None,
        arcgis: None,
        xyz: Some(c),
    })
}
pub(super) fn connect(
    name: &str,
    raw: &str,
    scheme: &str,
    grid: GridOptions,
) -> Result<MapService> {
    service(
        name,
        Configuration {
            scheme: scheme.into(),
            url_template: raw.into(),
            grid,
        },
    )
}
pub(super) fn validate_service(s: &MapService) -> Result<()> {
    let c = s.xyz.clone().ok_or("XYZ/TMS configuration is missing")?;
    let mut expected = service(&s.name, c)?;
    if !uuid_valid(&s.id) || instant(&s.connected_at).is_none() {
        return Err("Invalid saved XYZ/TMS service".into());
    }
    expected.id.clone_from(&s.id);
    expected.connected_at.clone_from(&s.connected_at);
    if *s != expected {
        return Err("XYZ/TMS service does not match its configured grid".into());
    }
    Ok(())
}
pub(super) fn matrix(c: &Configuration, zoom: u32) -> Result<wmts::Matrix> {
    validate_configuration(c)?;
    if zoom < c.grid.min_zoom || zoom > c.grid.max_zoom {
        return Err("Choose a configured XYZ/TMS tile level".into());
    }
    let count = 1_u32 << zoom;
    Ok(wmts::Matrix {
        id: zoom.to_string(),
        scale_denominator: (2. * HALF_WORLD / f64::from(c.grid.tile_size) / f64::from(count))
            / 0.00028,
        top_left: [-HALF_WORLD, HALF_WORLD],
        tile_width: c.grid.tile_size,
        tile_height: c.grid.tile_size,
        matrix_width: count,
        matrix_height: count,
    })
}
fn tile_url(c: &Configuration, zoom: u32, row: u32, col: u32) -> Result<Url> {
    let m = matrix(c, zoom)?;
    if row >= m.matrix_height || col >= m.matrix_width {
        return Err("XYZ/TMS tile lies outside the configured grid".into());
    }
    let y = if c.scheme == "TMS" {
        m.matrix_height - 1 - row
    } else {
        row
    };
    let z = i64::from(zoom) + i64::from(c.grid.zoom_offset);
    features::public_url(
        &c.url_template
            .replace("{z}", &z.to_string())
            .replace("{x}", &col.to_string())
            .replace("{y}", &y.to_string()),
    )
}
pub(super) fn tile_snapshot(s: &Snapshot) -> Result<wmts::Snapshot> {
    if s.matrix_set != GRID || s.matrix != matrix(&s.configuration, s.logical_zoom)? {
        return Err("XYZ/TMS matrix differs from the configured global grid".into());
    }
    Ok(wmts::Snapshot {
        resource_url: None,
        matrix_set: s.matrix_set.clone(),
        declared_crs: "EPSG:3857".into(),
        matrix: s.matrix.clone(),
        format: s.configuration.grid.format.clone(),
        time_identifier: None,
        requested_bounds: s.requested_bounds,
        pixel_window: s.pixel_window,
        tiles: s.tiles.clone(),
        archive_sha256: s.archive_sha256.clone(),
        archive_bytes: s.archive_bytes,
    })
}
fn grid_error(e: String) -> String {
    e.replace("WMTS", "XYZ/TMS")
}
pub(super) fn validate_asset(a: &MapImage) -> Result<()> {
    let src = &a.source;
    let s = src.xyz.as_ref().ok_or("XYZ/TMS source is missing")?;
    let expected = service(&src.service_name, s.configuration.clone())?;
    if !uuid_valid(&a.id)
        || !text_valid(&a.name, 120)
        || a.bytes == 0
        || a.bytes > MAX_PNG
        || !digest_valid(&a.sha256)
        || src.service_url != expected.url
        || src.map_endpoint != expected.map_url
        || src.service_title != expected.title
        || src.version != VERSION
        || src.layer_name != "tiles"
        || src.layer_title != expected.layers[0].title
        || !src.style.is_empty()
        || src.time.is_some()
        || src.request_crs != "EPSG:3857"
        || a.crs != "EPSG:3857"
        || src.wmts.is_some()
        || src.arcgis.is_some()
        || src.capabilities_sha256 != expected.capabilities_sha256
        || instant(&src.requested_at).is_none()
        || src.selection != "pixel-window-rendered-tiles"
        || src.access_constraints != expected.access_constraints
        || src.attribution != expected.layers[0].attribution
        || s.archive_bytes == 0
        || s.archive_bytes > 64 * 1024 * 1024
        || !digest_valid(&s.archive_sha256)
    {
        return Err("Invalid saved XYZ/TMS image metadata".into());
    }
    let m = matrix(&s.configuration, s.logical_zoom)?;
    if s.matrix_set != GRID || s.matrix != m {
        return Err("XYZ/TMS matrix differs from the configured global grid".into());
    }
    let p = wmts::plan(s.requested_bounds, "EPSG:3857", &m, &[]).map_err(grid_error)?;
    if a.image_extent != Some(p.extent)
        || a.bounds != p.bounds
        || s.pixel_window != p.window
        || u64::from(a.width) != p.window[2]
        || u64::from(a.height) != p.window[3]
        || s.tiles.len() != p.tiles.len()
    {
        return Err("XYZ/TMS output does not match its configured pixel grid".into());
    }
    for (r, (row, col)) in s.tiles.iter().zip(&p.tiles) {
        if r.row != *row
            || r.col != *col
            || r.bytes == 0
            || r.bytes > 4 * 1024 * 1024
            || !digest_valid(&r.sha256)
            || r.request_url != tile_url(&s.configuration, s.logical_zoom, *row, *col)?.to_string()
        {
            return Err("XYZ/TMS tile receipts do not match the saved grid".into());
        }
    }
    if src.request_url != s.tiles[0].request_url {
        return Err("XYZ/TMS first tile receipt changed".into());
    }
    validate_polygon(src.area_geometry.as_ref(), s.requested_bounds)?;
    Ok(())
}
fn validate_polygon(area: Option<&crate::crop::PolygonGeometry>, q: [f64; 4]) -> Result<()> {
    if let Some(area) = area {
        let b = area.bounds()?;
        if b[0] < q[0] || b[1] < q[1] || b[2] > q[2] || b[3] > q[3] {
            return Err("Map bounds must include the selected polygon".into());
        }
    }
    Ok(())
}
pub(super) async fn get(
    manager: &JobManager,
    r: MapRequest,
    service: MapService,
) -> Result<MapImage> {
    validate_service(&service)?;
    let c = service.xyz.as_ref().unwrap();
    let zoom = r
        .tile_matrix
        .as_deref()
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|v| Some(v.to_string()) == r.tile_matrix)
        .ok_or("Choose a configured XYZ/TMS tile level")?;
    if r.layer_name != "tiles"
        || !r.style.is_empty()
        || r.time.is_some()
        || r.tile_matrix_set.as_deref() != Some(GRID)
    {
        return Err("XYZ/TMS uses the configured grid without styles or a time dimension".into());
    }
    let m = matrix(c, zoom)?;
    let p = wmts::plan(r.bounds, "EPSG:3857", &m, &[]).map_err(grid_error)?;
    if u64::from(r.width) != p.window[2] || u64::from(r.height) != p.window[3] {
        return Err("XYZ/TMS dimensions must match the configured pixel grid".into());
    }
    validate_polygon(r.area_geometry.as_ref(), r.bounds)?;
    let s = Snapshot {
        configuration: c.clone(),
        logical_zoom: zoom,
        matrix_set: GRID.into(),
        matrix: m,
        requested_bounds: r.bounds,
        pixel_window: p.window,
        tiles: Vec::new(),
        archive_sha256: "0".repeat(64),
        archive_bytes: 1,
    };
    let layer = &service.layers[0];
    let mut a = MapImage {
        id: Uuid::new_v4().to_string(),
        name: format!("{} · {}", service.name, c.scheme),
        width: r.width,
        height: r.height,
        bounds: p.bounds,
        bytes: 1,
        sha256: "0".repeat(64),
        crs: "EPSG:3857".into(),
        image_extent: Some(p.extent),
        source: MapSource {
            service_url: service.url.clone(),
            service_name: service.name.clone(),
            service_title: service.title.clone(),
            version: VERSION.into(),
            map_endpoint: service.map_url.clone(),
            capabilities_sha256: service.capabilities_sha256.clone(),
            layer_name: layer.name.clone(),
            layer_title: layer.title.clone(),
            style: String::new(),
            time: None,
            request_crs: "EPSG:3857".into(),
            request_url: String::new(),
            requested_at: now(),
            access_constraints: service.access_constraints.clone(),
            attribution: layer.attribution.clone(),
            area_geometry: r.area_geometry,
            selection: "pixel-window-rendered-tiles".into(),
            wmts: None,
            arcgis: None,
            xyz: Some(s),
        },
    };
    let _permit = manager
        .inner
        .thumbnail_permits
        .acquire()
        .await
        .map_err(io_error)?;
    let settings = manager.proxy_settings().await;
    let responses = tokio::time::timeout(Duration::from_secs(120), async {
        let first = tile_url(c, zoom, p.tiles[0].0, p.tiles[0].1)?;
        let client = features::client(&first, &settings).await?;
        let mut bytes = Vec::new();
        for (row, col) in &p.tiles {
            let u = tile_url(c, zoom, *row, *col)?;
            let tile = wmts::fetch_tile(&client, &u, &c.grid.format)
                .await
                .map_err(grid_error)?;
            a.source
                .xyz
                .as_mut()
                .unwrap()
                .tiles
                .push(wmts::TileReceipt {
                    row: *row,
                    col: *col,
                    request_url: u.to_string(),
                    bytes: tile.len(),
                    sha256: hash(&tile),
                });
            bytes.push(tile);
        }
        Ok::<_, String>(bytes)
    })
    .await
    .map_err(|_| "XYZ/TMS retrieval timed out; no image was registered")??;
    a.source.request_url = a.source.xyz.as_ref().unwrap().tiles[0].request_url.clone();
    let (a, png, archive) = tokio::task::spawn_blocking(move || {
        let (png, archive) = wmts::assemble_grid(
            &a,
            &tile_snapshot(a.source.xyz.as_ref().unwrap())?,
            &responses,
        )
        .map_err(grid_error)?;
        let s = a.source.xyz.as_mut().unwrap();
        s.archive_bytes = archive.len();
        s.archive_sha256 = hash(&archive);
        a.bytes = png.len();
        a.sha256 = hash(&png);
        validate_asset(&a)?;
        Ok::<_, String>((a, png, archive))
    })
    .await
    .map_err(io_error)??;
    manager.save_map_image(a, png, Some(archive)).await
}

#[cfg(test)]
mod template_path_tests {
    use super::*;

    #[test]
    fn public_encoded_layer_names_retain_exact_tms_request_paths() {
        let template = "https://tiles.geoservice.dlr.de/service/tms/1.0.0/eoc%3Abasemap@EPSG%3A3857@png/{z}/{x}/{y}.png";
        assert!(template_url(template).is_ok());
        assert!(template_url("https://example.com/caf%C3%A9/{z}/{x}/{y}.png").is_ok());
        let config = Configuration {
            scheme: "TMS".into(),
            url_template: template.into(),
            grid: GridOptions {
                tile_size: 256,
                min_zoom: 0,
                max_zoom: 18,
                zoom_offset: 0,
                format: "image/png".into(),
                attribution: String::new(),
                access_constraints: String::new(),
            },
        };
        assert_eq!(
            tile_url(&config, 6, 21, 34).unwrap().as_str(),
            "https://tiles.geoservice.dlr.de/service/tms/1.0.0/eoc%3Abasemap@EPSG%3A3857@png/6/34/42.png"
        );
    }

    #[test]
    fn encoded_template_structure_controls_and_traversal_remain_rejected() {
        for segment in [
            "%2f",
            "%5C",
            "%25",
            "%3F",
            "%23",
            "%7Bz%7D",
            "%00",
            "%0d",
            "%C2%85",
            "%2e",
            ".%2E",
            "%252e%252e",
            "%FF",
            "%C0%AF",
            "%",
            "%2",
            "%2G",
        ] {
            let raw = format!("https://example.com/{segment}/{{z}}/{{x}}/{{y}}.png");
            assert!(template_url(&raw).is_err(), "accepted {segment}");
        }
    }
}
