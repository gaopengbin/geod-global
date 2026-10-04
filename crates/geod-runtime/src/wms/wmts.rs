//! WMTS 1.0 KVP / advertised REST rendered tiles. Grid identifiers are opaque, never XYZ zooms.
use super::*;
const WMTS: &str = "http://www.opengis.net/wmts/1.0";
const OWS: &str = "http://www.opengis.net/ows/1.1";
const METERS_PER_DEGREE: f64 = std::f64::consts::PI * 6_378_137. / 180.;
const MAX_ARCHIVE: usize = 64 * 1024 * 1024;
const MAX_TILES: usize = 16;
#[cfg(test)]
mod parser_tests;
mod rest;
#[cfg(test)]
mod rest_tests;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Matrix {
    pub id: String,
    pub scale_denominator: f64,
    pub top_left: [f64; 2],
    pub tile_width: u32,
    pub tile_height: u32,
    pub matrix_width: u32,
    pub matrix_height: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MatrixSet {
    pub id: String,
    pub crs: String,
    pub declared_crs: String,
    pub matrices: Vec<Matrix>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Limits {
    pub matrix: String,
    pub min_row: u32,
    pub max_row: u32,
    pub min_col: u32,
    pub max_col: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Link {
    pub matrix_set: String,
    pub limits: Vec<Limits>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Layer {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_url: Option<String>,
    pub format: String,
    pub default_style: String,
    pub time_identifier: Option<String>,
    pub links: Vec<Link>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Capabilities {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub rest_only: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub capabilities_document: bool,
    pub matrix_sets: Vec<MatrixSet>,
    #[serde(default)]
    pub excluded_layers: Vec<ExcludedLayer>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExcludedLayer {
    pub name: String,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TileReceipt {
    pub row: u32,
    pub col: u32,
    pub request_url: String,
    pub bytes: usize,
    pub sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_url: Option<String>,
    pub matrix_set: String,
    pub declared_crs: String,
    pub matrix: Matrix,
    pub format: String,
    pub time_identifier: Option<String>,
    pub requested_bounds: [f64; 4],
    pub pixel_window: [u64; 4],
    pub tiles: Vec<TileReceipt>,
    pub archive_sha256: String,
    pub archive_bytes: usize,
}
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Plan {
    pub window: [u64; 4],
    pub extent: [f64; 4],
    pub bounds: [f64; 4],
    pub tiles: Vec<(u32, u32)>,
}

fn nodes<'a, 'b>(n: Node<'a, 'b>, ns: &'a str, tag: &'a str) -> impl Iterator<Item = Node<'a, 'b>> {
    n.children().filter(move |c| {
        c.is_element() && c.tag_name().namespace() == Some(ns) && c.tag_name().name() == tag
    })
}
fn field(n: Node<'_, '_>, ns: &str, tag: &str, max: usize) -> Result<String> {
    let values: Vec<_> = n
        .children()
        .filter(|c| {
            c.is_element() && c.tag_name().namespace() == Some(ns) && c.tag_name().name() == tag
        })
        .collect();
    if values.len() > 1 {
        return Err(format!("Duplicate WMTS {tag}"));
    }
    let value = values.first().and_then(Node::text).unwrap_or("").trim();
    if value.len() > max || value.chars().any(char::is_control) {
        return Err(format!("Invalid WMTS {tag}"));
    }
    Ok(value.into())
}
// OWS DescriptionType permits one translation per language. Prefer an
// unqualified label, then English, then the first advertised translation.
fn description(n: Node<'_, '_>, tag: &str, max: usize) -> Result<String> {
    let mut languages = BTreeSet::new();
    let mut selected: Option<(u8, String)> = None;
    for child in nodes(n, OWS, tag) {
        let language = child
            .ancestors()
            .find_map(|node| node.attribute(("http://www.w3.org/XML/1998/namespace", "lang")))
            .unwrap_or("")
            .to_ascii_lowercase();
        if !languages.insert(language.clone()) {
            return Err(format!("Duplicate WMTS {tag} translation"));
        }
        let value = child.text().unwrap_or("").trim();
        if value.len() > max || value.chars().any(char::is_control) {
            return Err(format!("Invalid WMTS {tag}"));
        }
        let priority = if language.is_empty() {
            0
        } else if language == "en" || language.starts_with("en-") {
            1
        } else {
            2
        };
        if selected
            .as_ref()
            .is_none_or(|(current, _)| priority < *current)
        {
            selected = Some((priority, value.into()));
        }
    }
    Ok(selected.map(|(_, value)| value).unwrap_or_default())
}
fn number<T: std::str::FromStr>(n: Node<'_, '_>, tag: &str) -> Result<T> {
    field(n, WMTS, tag, 128)?
        .parse()
        .map_err(|_| format!("Invalid WMTS {tag}"))
}
fn pair(raw: &str) -> Result<[f64; 2]> {
    let v: Vec<f64> = raw
        .split_whitespace()
        .map(str::parse)
        .collect::<std::result::Result<_, _>>()
        .map_err(io_error)?;
    if v.len() != 2 || v.iter().any(|n| !n.is_finite()) {
        return Err("Invalid WMTS coordinates".into());
    }
    Ok([v[0], v[1]])
}
fn crs(raw: &str) -> Result<&'static str> {
    if matches!(
        raw,
        "CRS:84" | "urn:ogc:def:crs:OGC:1.3:CRS84" | "http://www.opengis.net/def/crs/OGC/1.3/CRS84"
    ) {
        return Ok("EPSG:4326");
    }
    if raw == "EPSG:4326"
        || (raw.starts_with("urn:ogc:def:crs:EPSG:") && raw.ends_with(":4326"))
        || raw == "http://www.opengis.net/def/crs/EPSG/0/4326"
    {
        return Ok("EPSG:4326");
    }
    if raw == "EPSG:3857"
        || (raw.starts_with("urn:ogc:def:crs:EPSG:") && raw.ends_with(":3857"))
        || raw == "http://www.opengis.net/def/crs/EPSG/0/3857"
    {
        return Ok("EPSG:3857");
    }
    Err("Unsupported WMTS projection; use WGS84 or Web Mercator".into())
}
fn latitude_first(raw: &str) -> bool {
    crs(raw) == Ok("EPSG:4326") && !raw.contains("CRS84") && raw != "CRS:84"
}
fn resolution(crs: &str, m: &Matrix) -> f64 {
    m.scale_denominator * 0.00028
        / if crs == "EPSG:3857" {
            1.
        } else {
            METERS_PER_DEGREE
        }
}
fn validate_matrix(m: &Matrix) -> Result<()> {
    if !text_valid(&m.id, 256)
        || !m.scale_denominator.is_finite()
        || m.scale_denominator <= 0.
        || m.scale_denominator > 1e12
        || m.top_left.iter().any(|n| !n.is_finite() || n.abs() > 1e9)
        || [m.tile_width, m.tile_height]
            .iter()
            .any(|n| *n == 0 || *n > 1024)
        || [m.matrix_width, m.matrix_height]
            .iter()
            .any(|n| *n == 0 || *n > (1 << 24))
    {
        return Err("Invalid WMTS tile matrix".into());
    }
    Ok(())
}
pub(super) fn parse_capabilities(bytes: &[u8], root: &Url, name: &str) -> Result<MapService> {
    if bytes.len() > MAX_XML {
        return Err("WMTS capabilities exceed 8 MiB".into());
    }
    let xml = std::str::from_utf8(bytes).map_err(|_| "WMTS capabilities must be UTF-8")?;
    if xml.to_ascii_uppercase().contains("<!ENTITY") {
        return Err("WMTS entity declarations are not supported".into());
    }
    let doc = roxmltree::Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            nodes_limit: 300_000,
            ..Default::default()
        },
    )
    .map_err(|_| "Invalid WMTS XML")?;
    if doc
        .descendants()
        .any(|n| n.ancestors().take(34).count() > 32)
    {
        return Err("WMTS XML nesting exceeds the limit".into());
    }
    let r = doc.root_element();
    if r.tag_name().namespace() != Some(WMTS)
        || r.tag_name().name() != "Capabilities"
        || r.attribute("version") != Some("1.0.0")
    {
        return Err("Expected WMTS 1.0.0 capabilities".into());
    }
    let info = nodes(r, OWS, "ServiceIdentification")
        .next()
        .ok_or("WMTS service identification is missing")?;
    let tile_get = nodes(r, OWS, "OperationsMetadata")
        .flat_map(|n| nodes(n, OWS, "Operation"))
        .filter(|n| n.attribute("name") == Some("GetTile"))
        .flat_map(|n| n.descendants())
        .filter(|n| n.has_tag_name((OWS, "Get")))
        .find(|n| {
            n.descendants().any(|d| {
                d.has_tag_name((OWS, "Constraint"))
                    && d.attribute("name") == Some("GetEncoding")
                    && d.descendants()
                        .any(|v| v.has_tag_name((OWS, "Value")) && v.text() == Some("KVP"))
            })
        });
    let tile_url = tile_get
        .map(|get| {
            rest::map_endpoint(
                root,
                get.attribute(("http://www.w3.org/1999/xlink", "href"))
                    .ok_or("WMTS tile endpoint is missing")?,
            )
        })
        .transpose()?;
    let contents = nodes(r, WMTS, "Contents")
        .next()
        .ok_or("WMTS contents are missing")?;
    let mut sets = Vec::new();
    for set in nodes(contents, WMTS, "TileMatrixSet") {
        if sets.len() >= 128 {
            return Err("WMTS has too many tile matrix sets".into());
        }
        let declared = field(set, OWS, "SupportedCRS", 256)?;
        let Ok(normalized) = crs(&declared) else {
            continue;
        };
        let mut matrices = Vec::new();
        for matrix in nodes(set, WMTS, "TileMatrix") {
            if matrices.len() >= 64 {
                return Err("WMTS has too many tile matrices".into());
            }
            let mut origin = pair(&field(matrix, WMTS, "TopLeftCorner", 256)?)?;
            if latitude_first(&declared) {
                origin.swap(0, 1);
            }
            let m = Matrix {
                id: field(matrix, OWS, "Identifier", 256)?,
                scale_denominator: number(matrix, "ScaleDenominator")?,
                top_left: origin,
                tile_width: number(matrix, "TileWidth")?,
                tile_height: number(matrix, "TileHeight")?,
                matrix_width: number(matrix, "MatrixWidth")?,
                matrix_height: number(matrix, "MatrixHeight")?,
            };
            validate_matrix(&m)?;
            matrices.push(m);
        }
        sets.push(MatrixSet {
            id: field(set, OWS, "Identifier", 256)?,
            crs: normalized.into(),
            declared_crs: declared,
            matrices,
        });
    }
    let mut layers = Vec::new();
    let mut excluded_layers = Vec::new();
    for l in nodes(contents, WMTS, "Layer") {
        if layers.len() >= 4096 {
            return Err("WMTS has too many compatible layers".into());
        }
        let formats: Vec<_> = nodes(l, WMTS, "Format").filter_map(|n| n.text()).collect();
        if !formats
            .iter()
            .any(|f| matches!(*f, "image/png" | "image/jpeg"))
        {
            continue;
        }
        let dimensions: Vec<_> = nodes(l, WMTS, "Dimension").collect();
        if dimensions.len() > 1 {
            continue;
        }
        let (time, time_id) = if let Some(dim) = dimensions.first() {
            let id = field(*dim, OWS, "Identifier", 256)?;
            if !id.eq_ignore_ascii_case("time") {
                continue;
            }
            let values = nodes(*dim, WMTS, "Value")
                .filter_map(|n| n.text())
                .map(str::trim)
                .collect::<Vec<_>>()
                .join(",");
            let d = field(*dim, WMTS, "Default", 128)?;
            (
                Some(TimeDimension {
                    values,
                    default: if d.is_empty() { None } else { Some(d) },
                }),
                Some(id),
            )
        } else {
            (None, None)
        };
        let mut links = Vec::new();
        let mut bad_limits = false;
        for link in nodes(l, WMTS, "TileMatrixSetLink") {
            let id = field(link, WMTS, "TileMatrixSet", 256)?;
            if !sets.iter().any(|s| s.id == id) {
                continue;
            }
            let mut limits = Vec::new();
            for n in nodes(link, WMTS, "TileMatrixSetLimits")
                .flat_map(|n| nodes(n, WMTS, "TileMatrixLimits"))
            {
                limits.push(Limits {
                    matrix: field(n, WMTS, "TileMatrix", 256)?,
                    min_row: number(n, "MinTileRow")?,
                    max_row: number(n, "MaxTileRow")?,
                    min_col: number(n, "MinTileCol")?,
                    max_col: number(n, "MaxTileCol")?,
                });
            }
            let set = sets.iter().find(|s| s.id == id).unwrap();
            if limits.iter().any(|l| {
                set.matrices
                    .iter()
                    .find(|m| m.id == l.matrix)
                    .is_none_or(|m| {
                        l.min_row > l.max_row
                            || l.min_col > l.max_col
                            || l.max_row >= m.matrix_height
                            || l.max_col >= m.matrix_width
                    })
            }) {
                bad_limits = true;
                continue;
            }
            links.push(Link {
                matrix_set: id,
                limits,
            });
        }
        if bad_limits {
            excluded_layers.push(ExcludedLayer {
                name: field(l, OWS, "Identifier", 256)?,
                reason: if links.is_empty() {
                    "Declared tile limits exceed every compatible matrix set"
                } else {
                    "Some declared matrix set links have invalid tile limits"
                }
                .into(),
            });
        }
        if links.is_empty() {
            continue;
        }
        let mut styles = Vec::new();
        let mut defaults = Vec::new();
        for s in nodes(l, WMTS, "Style") {
            let id = field(s, OWS, "Identifier", 256)?;
            match s.attribute("isDefault").map(str::trim) {
                Some("true" | "1") => defaults.push(id.clone()),
                Some("false" | "0") | None => {}
                _ => return Err("Invalid WMTS style default flag".into()),
            }
            styles.push(id);
        }
        if defaults.len() != 1 {
            continue;
        }
        let resources: Vec<_> = nodes(l, WMTS, "ResourceURL")
            .filter(|r| r.attribute("resourceType") == Some("tile"))
            .collect();
        if resources.len() > 128 {
            return Err("Too many WMTS REST tile templates".into());
        }
        let selected = ["image/png", "image/jpeg"]
            .into_iter()
            .filter(|f| formats.contains(f))
            .find_map(|format| {
                let resource = resources
                    .iter()
                    .filter(|r| r.attribute("format") == Some(format))
                    .filter_map(|r| {
                        rest::template(root, r.attribute("template")?, time_id.as_deref()).ok()
                    })
                    .find(|r| {
                        (styles.len() == 1 || r.contains("{Style}"))
                            && (links.len() == 1 || r.contains("{TileMatrixSet}"))
                    });
                (resource.is_some() || tile_url.is_some()).then_some((format, resource))
            });
        let Some((format, resource_url)) = selected else {
            excluded_layers.push(ExcludedLayer {
                name: field(l, OWS, "Identifier", 256)?,
                reason: "No supported public WMTS REST tile template or KVP endpoint".into(),
            });
            continue;
        };
        if resource_url.is_none() && !resources.is_empty() {
            excluded_layers.push(ExcludedLayer {
                name: field(l, OWS, "Identifier", 256)?,
                reason: "Unsupported REST tile templates; using advertised KVP".into(),
            });
        }
        let bbox = nodes(l, OWS, "WGS84BoundingBox")
            .next()
            .map(|n| -> Result<[f64; 4]> {
                let lo = pair(&field(n, OWS, "LowerCorner", 256)?)?;
                let hi = pair(&field(n, OWS, "UpperCorner", 256)?)?;
                let b = [lo[0], lo[1], hi[0], hi[1]];
                features::bounds(b)?;
                Ok(b)
            })
            .transpose()?;
        let name = field(l, OWS, "Identifier", 256)?;
        let title = description(l, "Title", 1024)?;
        let normalized = sets
            .iter()
            .find(|s| s.id == links[0].matrix_set)
            .unwrap()
            .crs
            .clone();
        layers.push(MapLayer {
            name: name.clone(),
            title: if title.is_empty() { name } else { title },
            description: description(l, "Abstract", 16384)?,
            crs: normalized,
            styles,
            time,
            bounds: bbox,
            attribution: None,
            wmts: Some(Layer {
                resource_url,
                format: format.into(),
                default_style: defaults.remove(0),
                time_identifier: time_id,
                links,
            }),
        });
    }
    let title = description(info, "Title", 1024)?;
    let service = MapService {
        xyz: None,
        arcgis: None,
        id: Uuid::new_v4().to_string(),
        name: name.into(),
        url: root.to_string(),
        title: if title.is_empty() { name.into() } else { title },
        version: "1.0.0".into(),
        map_url: tile_url.as_ref().unwrap_or(root).to_string(),
        layers,
        access_constraints: field(info, OWS, "AccessConstraints", 16384)?,
        max_width: MAX_EDGE,
        max_height: MAX_EDGE,
        capabilities_sha256: hash(bytes),
        connected_at: now(),
        wmts: Some(Capabilities {
            rest_only: tile_url.is_none(),
            capabilities_document: false,
            matrix_sets: sets,
            excluded_layers,
        }),
    };
    if service.layers.is_empty() {
        return Err("No compatible WMTS layers with a supported tile address".into());
    }
    validate_service(&service)?;
    Ok(service)
}
pub(super) fn validate_service(s: &MapService) -> Result<()> {
    let root = service_url(&s.url)?;
    rest::map_endpoint(&root, &s.map_url)?;
    if !uuid_valid(&s.id)
        || !text_valid(&s.name, 80)
        || !text_valid(&s.title, 1024)
        || s.version != "1.0.0"
        || !digest_valid(&s.capabilities_sha256)
        || instant(&s.connected_at).is_none()
        || s.access_constraints.len() > 16384
        || s.max_width != MAX_EDGE
        || s.max_height != MAX_EDGE
        || s.layers.is_empty()
        || s.layers.len() > 4096
    {
        return Err("Invalid saved WMTS service".into());
    }
    let caps = s.wmts.as_ref().ok_or("WMTS matrix sets are missing")?;
    if caps.matrix_sets.is_empty() || caps.matrix_sets.len() > 128 {
        return Err("Invalid WMTS matrix set count".into());
    }
    if caps.excluded_layers.len() > 4096
        || caps
            .excluded_layers
            .iter()
            .any(|l| !text_valid(&l.name, 256) || !text_valid(&l.reason, 1024))
    {
        return Err("Invalid WMTS discovery exclusions".into());
    }
    let mut ids = BTreeSet::new();
    for set in &caps.matrix_sets {
        if !ids.insert(&set.id)
            || !text_valid(&set.id, 256)
            || crs(&set.declared_crs)? != set.crs
            || set.matrices.is_empty()
            || set.matrices.len() > 64
        {
            return Err("Invalid WMTS matrix set".into());
        }
        let mut seen = BTreeSet::new();
        for m in &set.matrices {
            validate_matrix(m)?;
            if !seen.insert(&m.id) {
                return Err("Duplicate WMTS matrix".into());
            }
        }
    }
    if caps.rest_only && s.map_url != s.url {
        return Err("Invalid REST-only WMTS service endpoint".into());
    }
    let mut names = BTreeSet::new();
    for l in &s.layers {
        let wmts = l.wmts.as_ref().ok_or("WMTS layer metadata is missing")?;
        if !names.insert(&l.name)
            || !text_valid(&l.name, 256)
            || !text_valid(&l.title, 1024)
            || l.description.len() > 16384
            || !matches!(wmts.format.as_str(), "image/png" | "image/jpeg")
            || l.styles.is_empty()
            || l.styles.len() > 256
            || l.styles.iter().any(|s| !text_valid(s, 256))
            || !l.styles.contains(&wmts.default_style)
            || wmts.links.is_empty()
            || wmts.links.len() > 128
            || l.time.as_ref().is_some_and(|d| {
                d.values.is_empty()
                    || d.values.len() > 65536
                    || d.default.as_ref().is_some_and(|t| t.len() > 128)
            })
            || wmts
                .time_identifier
                .as_ref()
                .is_some_and(|t| !t.eq_ignore_ascii_case("time"))
            || l.time.is_some() != wmts.time_identifier.is_some()
        {
            return Err("Invalid WMTS layer metadata".into());
        }
        if let Some(raw) = &wmts.resource_url {
            if rest::template(&root, raw, wmts.time_identifier.as_deref())? != *raw
                || (l.styles.len() > 1 && !raw.contains("{Style}"))
                || (wmts.links.len() > 1 && !raw.contains("{TileMatrixSet}"))
            {
                return Err("WMTS REST template does not match the layer dimensions".into());
            }
        } else if caps.rest_only {
            return Err("REST-only WMTS layer has no tile template".into());
        }
        if let Some(b) = l.bounds {
            features::bounds(b)?;
        }
        let mut links = BTreeSet::new();
        for link in &wmts.links {
            let set = caps
                .matrix_sets
                .iter()
                .find(|s| s.id == link.matrix_set)
                .ok_or("Unknown WMTS matrix set link")?;
            if !links.insert(&link.matrix_set) || link.limits.len() > 64 {
                return Err("Invalid WMTS matrix set link".into());
            }
            let mut limits = BTreeSet::new();
            for limit in &link.limits {
                let m = set
                    .matrices
                    .iter()
                    .find(|m| m.id == limit.matrix)
                    .ok_or("Unknown limited WMTS matrix")?;
                if !limits.insert((
                    &limit.matrix,
                    limit.min_row,
                    limit.max_row,
                    limit.min_col,
                    limit.max_col,
                )) || limit.min_row > limit.max_row
                    || limit.min_col > limit.max_col
                    || limit.max_row >= m.matrix_height
                    || limit.max_col >= m.matrix_width
                {
                    return Err("Invalid WMTS tile limits".into());
                }
            }
        }
        if l.crs
            != caps
                .matrix_sets
                .iter()
                .find(|s| s.id == wmts.links[0].matrix_set)
                .unwrap()
                .crs
        {
            return Err("WMTS layer CRS does not match its matrix set".into());
        }
    }
    Ok(())
}
fn project(b: [f64; 4], crs: &str) -> Result<[f64; 4]> {
    features::bounds(b)?;
    if crs == "EPSG:4326" {
        return Ok(b);
    }
    if crs != "EPSG:3857" || b[1] < -85.0511287798066 || b[3] > 85.0511287798066 {
        return Err("The selected region is outside Web Mercator".into());
    }
    let y = |lat: f64| {
        6_378_137.
            * (std::f64::consts::FRAC_PI_4 + lat.to_radians() / 2.)
                .tan()
                .ln()
    };
    Ok([
        b[0] * METERS_PER_DEGREE,
        y(b[1]),
        b[2] * METERS_PER_DEGREE,
        y(b[3]),
    ])
}
fn unproject(b: [f64; 4], crs: &str) -> [f64; 4] {
    if crs == "EPSG:4326" {
        return b;
    }
    let y =
        |v: f64| (2. * (v / 6_378_137.).exp().atan() - std::f64::consts::FRAC_PI_2).to_degrees();
    [
        b[0] / METERS_PER_DEGREE,
        y(b[1]),
        b[2] / METERS_PER_DEGREE,
        y(b[3]),
    ]
}
fn snap(n: f64) -> f64 {
    if (n - n.round()).abs() < 1e-7 {
        n.round()
    } else {
        n
    }
}
pub(super) fn plan(bounds: [f64; 4], crs: &str, m: &Matrix, limits: &[Limits]) -> Result<Plan> {
    validate_matrix(m)?;
    let b = project(bounds, crs)?;
    let r = resolution(crs, m);
    let v = [
        snap((b[0] - m.top_left[0]) / r).floor(),
        snap((m.top_left[1] - b[3]) / r).floor(),
        snap((b[2] - m.top_left[0]) / r).ceil(),
        snap((m.top_left[1] - b[1]) / r).ceil(),
    ];
    if v.iter()
        .any(|n| !n.is_finite() || *n < 0. || *n > (1_u64 << 34) as f64)
        || v[2] <= v[0]
        || v[3] <= v[1]
        || v[2] > f64::from(m.matrix_width) * f64::from(m.tile_width)
        || v[3] > f64::from(m.matrix_height) * f64::from(m.tile_height)
    {
        return Err("The selected region is outside the WMTS tile grid".into());
    }
    let window = [
        v[0] as u64,
        v[1] as u64,
        (v[2] - v[0]) as u64,
        (v[3] - v[1]) as u64,
    ];
    if window[2] > u64::from(MAX_EDGE) || window[3] > u64::from(MAX_EDGE) {
        return Err("Choose a coarser WMTS level or a smaller area (2048 pixels per edge)".into());
    }
    let rows = ((window[1] / u64::from(m.tile_height)) as u32)
        ..=(((window[1] + window[3] - 1) / u64::from(m.tile_height)) as u32);
    let cols = ((window[0] / u64::from(m.tile_width)) as u32)
        ..=(((window[0] + window[2] - 1) / u64::from(m.tile_width)) as u32);
    if (rows.clone().count() * cols.clone().count()) > MAX_TILES {
        return Err("Choose a coarser WMTS level or a smaller area (16 tiles per image)".into());
    }
    let mut tiles = Vec::new();
    for row in rows {
        for col in cols.clone() {
            if !limits.is_empty()
                && !limits.iter().any(|l| {
                    row >= l.min_row && row <= l.max_row && col >= l.min_col && col <= l.max_col
                })
            {
                return Err("The selected region exceeds the layer tile limits".into());
            }
            tiles.push((row, col));
        }
    }
    let extent = [
        m.top_left[0] + v[0] * r,
        m.top_left[1] - v[3] * r,
        m.top_left[0] + v[2] * r,
        m.top_left[1] - v[1] * r,
    ];
    let mut bounds = unproject(extent, crs);
    // Published scale denominators can put an exact world-edge pixel a few
    // floating-point ulps outside WGS84. Keep its native grid untouched.
    for (coordinate, limit) in bounds.iter_mut().zip([180., 90., 180., 90.]) {
        if coordinate.abs() > limit && coordinate.abs() - limit <= 1e-9 {
            *coordinate = coordinate.signum() * limit;
        }
    }
    features::bounds(bounds)?;
    Ok(Plan {
        window,
        extent,
        bounds,
        tiles,
    })
}
fn tile_url(endpoint: &str, source: &MapSource, s: &Snapshot, row: u32, col: u32) -> Result<Url> {
    if let Some(raw) = &s.resource_url {
        return rest::tile_url(raw, source, s, row, col);
    }
    let mut u = service_url(endpoint)?;
    for (k, v) in [
        ("SERVICE", "WMTS"),
        ("REQUEST", "GetTile"),
        ("VERSION", "1.0.0"),
        ("LAYER", &source.layer_name),
        ("STYLE", &source.style),
        ("FORMAT", &s.format),
        ("TILEMATRIXSET", &s.matrix_set),
        ("TILEMATRIX", &s.matrix.id),
    ] {
        u.query_pairs_mut().append_pair(k, v);
    }
    u.query_pairs_mut()
        .append_pair("TILEROW", &row.to_string())
        .append_pair("TILECOL", &col.to_string());
    if let (Some(key), Some(time)) = (&s.time_identifier, &source.time) {
        u.query_pairs_mut().append_pair(key, time);
    }
    Ok(u)
}
pub(super) fn validate_asset(a: &MapImage) -> Result<()> {
    let src = &a.source;
    let s = src.wmts.as_ref().ok_or("WMTS source is missing")?;
    let root = service_url(&src.service_url)?;
    rest::map_endpoint(&root, &src.map_endpoint)?;
    if let Some(raw) = &s.resource_url {
        if rest::template(&root, raw, s.time_identifier.as_deref())? != *raw {
            return Err("Invalid saved WMTS REST template".into());
        }
    }
    if !uuid_valid(&a.id)
        || !text_valid(&a.name, 120)
        || a.bytes == 0
        || a.bytes > MAX_PNG
        || !digest_valid(&a.sha256)
        || !text_valid(&src.service_name, 80)
        || !text_valid(&src.service_title, 1024)
        || !text_valid(&src.layer_name, 256)
        || !text_valid(&src.layer_title, 1024)
        || !text_valid(&src.style, 256)
        || src.version != "1.0.0"
        || src.request_crs != a.crs
        || crs(&s.declared_crs)? != a.crs
        || !text_valid(&s.matrix_set, 256)
        || !matches!(s.format.as_str(), "image/png" | "image/jpeg")
        || !digest_valid(&src.capabilities_sha256)
        || instant(&src.requested_at).is_none()
        || src.selection != "pixel-window-rendered-tiles"
        || src.time.as_ref().is_some_and(|t| instant(t).is_none())
        || s.time_identifier
            .as_ref()
            .is_some_and(|t| !t.eq_ignore_ascii_case("time"))
        || s.time_identifier.is_some() != src.time.is_some()
        || s.archive_bytes == 0
        || s.archive_bytes > MAX_ARCHIVE
        || !digest_valid(&s.archive_sha256)
        || src.access_constraints.len() > 16384
        || src
            .attribution
            .as_ref()
            .is_some_and(|t| !text_valid(t, 1024))
    {
        return Err("Invalid saved WMTS image metadata".into());
    }
    let p = plan(s.requested_bounds, &a.crs, &s.matrix, &[])?;
    if a.image_extent != Some(p.extent)
        || a.bounds != p.bounds
        || u64::from(a.width) != p.window[2]
        || u64::from(a.height) != p.window[3]
        || s.pixel_window != p.window
        || s.tiles.len() != p.tiles.len()
    {
        return Err("WMTS output does not match its declared tile grid".into());
    }
    for (receipt, (row, col)) in s.tiles.iter().zip(&p.tiles) {
        if receipt.row != *row
            || receipt.col != *col
            || receipt.bytes == 0
            || receipt.bytes > 4 * 1024 * 1024
            || !digest_valid(&receipt.sha256)
            || receipt.request_url != tile_url(&src.map_endpoint, src, s, *row, *col)?.to_string()
        {
            return Err("WMTS tile receipts do not match the saved grid".into());
        }
    }
    if src.request_url != s.tiles[0].request_url {
        return Err("WMTS first tile receipt changed".into());
    }
    if let Some(area) = &src.area_geometry {
        let b = area.bounds()?;
        let q = s.requested_bounds;
        if b[0] < q[0] || b[1] < q[1] || b[2] > q[2] || b[3] > q[3] {
            return Err("Map bounds must include the selected polygon".into());
        }
    }
    Ok(())
}
pub(super) async fn fetch_tile(c: &reqwest::Client, u: &Url, format: &str) -> Result<Vec<u8>> {
    let r = c
        .get(u.clone())
        .header("Accept", format)
        .send()
        .await
        .map_err(|_| "Cannot reach WMTS service; check the service and proxy settings")?;
    if !r.status().is_success() {
        return Err(format!("WMTS tile returned HTTP {}", r.status().as_u16()));
    }
    let media = r
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim();
    if media != format {
        return Err("WMTS returned an unexpected tile format or service exception".into());
    }
    let max = 4 * 1024 * 1024;
    if r.content_length().is_some_and(|n| n > max as u64) {
        return Err("WMTS tile exceeds 4 MiB".into());
    }
    let mut bytes = Vec::new();
    let mut stream = r.bytes_stream();
    while let Some(b) = stream.next().await {
        let b = b.map_err(|_| "WMTS tile response was interrupted")?;
        if b.len() > max - bytes.len() {
            return Err("WMTS tile exceeds 4 MiB".into());
        }
        bytes.extend_from_slice(&b);
    }
    Ok(bytes)
}
fn decode(bytes: &[u8], format: &str, m: &Matrix) -> Result<Vec<u8>> {
    let expected = m.tile_width as usize * m.tile_height as usize;
    if format == "image/jpeg" {
        let opts = zune_core::options::DecoderOptions::default()
            .set_max_width(1024)
            .set_max_height(1024)
            .set_strict_mode(true)
            .jpeg_set_out_colorspace(zune_core::colorspace::ColorSpace::RGB);
        let mut decoder = zune_jpeg::JpegDecoder::new_with_options(
            zune_core::bytestream::ZCursor::new(bytes),
            opts,
        );
        decoder
            .decode_headers()
            .map_err(|_| "Invalid WMTS JPEG tile")?;
        if decoder.dimensions() != Some((m.tile_width as usize, m.tile_height as usize)) {
            return Err("WMTS returned different tile dimensions".into());
        }
        let rgb = decoder.decode().map_err(|_| "Corrupt WMTS JPEG pixels")?;
        if rgb.len() != expected * 3 {
            return Err("Invalid decoded JPEG channel count".into());
        }
        return Ok(rgb
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect());
    }
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_limits(png::Limits {
        bytes: 32 * 1024 * 1024,
    });
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().map_err(|_| "Invalid WMTS PNG tile")?;
    if reader.info().width != m.tile_width
        || reader.info().height != m.tile_height
        || reader.info().animation_control.is_some()
        || reader.info().bit_depth == png::BitDepth::Sixteen
    {
        return Err("WMTS returned different tile dimensions or unsupported PNG pixels".into());
    }
    let mut pixels = vec![
        0;
        reader
            .output_buffer_size()
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or("WMTS tile decoding exceeds the memory limit")?
    ];
    let info = reader
        .next_frame(&mut pixels)
        .map_err(|_| "Corrupt WMTS PNG pixels")?;
    reader.finish().map_err(|_| "Incomplete WMTS PNG tile")?;
    pixels.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => pixels,
        png::ColorType::Rgb => pixels
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::Grayscale => pixels.iter().flat_map(|p| [*p, *p, *p, 255]).collect(),
        png::ColorType::GrayscaleAlpha => pixels
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        _ => return Err("Unsupported WMTS PNG channels".into()),
    };
    if rgba.len() != expected * 4 {
        return Err("Invalid WMTS decoded pixel count".into());
    }
    Ok(rgba)
}
fn tile_filename(s: &Snapshot, t: &TileReceipt) -> String {
    format!(
        "{}-{}.{}",
        t.row,
        t.col,
        if s.format == "image/png" {
            "png"
        } else {
            "jpg"
        }
    )
}
fn assemble(a: &MapImage, tiles: &[Vec<u8>]) -> Result<(Vec<u8>, Vec<u8>)> {
    assemble_grid(a, a.source.wmts.as_ref().unwrap(), tiles)
}
pub(super) fn assemble_grid(
    a: &MapImage,
    s: &Snapshot,
    tiles: &[Vec<u8>],
) -> Result<(Vec<u8>, Vec<u8>)> {
    let p = plan(s.requested_bounds, &a.crs, &s.matrix, &[])?;
    if s.pixel_window != p.window
        || s.tiles.len() != p.tiles.len()
        || tiles.len() != s.tiles.len()
        || s.tiles
            .iter()
            .zip(&p.tiles)
            .any(|(t, (row, col))| t.row != *row || t.col != *col)
        || s.tiles
            .iter()
            .zip(tiles)
            .any(|(t, b)| t.bytes != b.len() || t.sha256 != hash(b))
    {
        return Err("Source tiles do not match the planned pixel window".into());
    }
    let m = &s.matrix;
    let [x, y, w, h] = s.pixel_window;
    if u64::from(a.width) != w || u64::from(a.height) != h {
        return Err("Tile output dimensions do not match the planned window".into());
    }
    let mut pixels = vec![0; w as usize * h as usize * 4];
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o644);
    for (receipt, bytes) in s.tiles.iter().zip(tiles) {
        let rgba = decode(bytes, &s.format, m)?;
        let tile_width = u64::from(m.tile_width);
        let tile_height = u64::from(m.tile_height);
        let tx = u64::from(receipt.col) * tile_width;
        let ty = u64::from(receipt.row) * tile_height;
        let left = x.max(tx);
        let right = (x + w).min(tx + tile_width);
        let top = y.max(ty);
        let bottom = (y + h).min(ty + tile_height);
        for row in top..bottom {
            let src = ((row - ty) * tile_width + (left - tx)) as usize * 4;
            let dst = ((row - y) * w + (left - x)) as usize * 4;
            let len = (right - left) as usize * 4;
            pixels[dst..dst + len].copy_from_slice(&rgba[src..src + len]);
        }
        archive
            .start_file(tile_filename(s, receipt), opts)
            .map_err(io_error)?;
        archive.write_all(bytes).map_err(io_error)?;
    }
    let archive = archive.finish().map_err(io_error)?.into_inner();
    if archive.len() > MAX_ARCHIVE {
        return Err("WMTS tile archive exceeds 64 MiB".into());
    }
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, w as u32, h as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(io_error)?;
        writer.write_image_data(&pixels).map_err(io_error)?;
        writer.finish().map_err(io_error)?;
    }
    validate_png(&png, w as u32, h as u32)?;
    Ok((png, archive))
}
pub(super) fn read_archive(path: &Path, s: &Snapshot) -> Result<Vec<u8>> {
    let bytes = read_file(path, MAX_ARCHIVE)?;
    if bytes.len() != s.archive_bytes || hash(&bytes) != s.archive_sha256 {
        return Err("Saved WMTS tile archive changed; retrieve it again".into());
    }
    let mut zip = zip::ZipArchive::new(Cursor::new(&bytes)).map_err(io_error)?;
    if zip.len() != s.tiles.len() {
        return Err("WMTS tile archive count changed".into());
    }
    for t in &s.tiles {
        let mut f = zip.by_name(&tile_filename(s, t)).map_err(io_error)?;
        if f.size() != t.bytes as u64 || f.compression() != zip::CompressionMethod::Stored {
            return Err("Invalid saved WMTS tile archive".into());
        }
        let mut tile = Vec::new();
        f.read_to_end(&mut tile).map_err(io_error)?;
        if hash(&tile) != t.sha256 {
            return Err("Saved WMTS tile pixels changed".into());
        }
    }
    Ok(bytes)
}
pub(super) async fn get(
    manager: &JobManager,
    r: MapRequest,
    service: MapService,
) -> Result<MapImage> {
    validate_service(&service)?;
    let layer = service
        .layers
        .iter()
        .find(|l| l.name == r.layer_name)
        .ok_or("Unknown WMTS layer")?;
    let meta = layer.wmts.as_ref().unwrap();
    let set = service
        .wmts
        .as_ref()
        .unwrap()
        .matrix_sets
        .iter()
        .find(|s| Some(&s.id) == r.tile_matrix_set.as_ref())
        .ok_or("Choose an advertised WMTS tile matrix set")?;
    let link = meta
        .links
        .iter()
        .find(|l| l.matrix_set == set.id)
        .ok_or("The WMTS layer does not use this matrix set")?;
    let matrix = set
        .matrices
        .iter()
        .find(|m| Some(&m.id) == r.tile_matrix.as_ref())
        .ok_or("Choose an advertised WMTS tile level")?;
    let limits: Vec<_> = link
        .limits
        .iter()
        .filter(|l| l.matrix == matrix.id)
        .cloned()
        .collect();
    if !link.limits.is_empty() && limits.is_empty() {
        return Err("This WMTS matrix is not available for the layer".into());
    }
    let p = plan(r.bounds, &set.crs, matrix, &limits)?;
    if u64::from(r.width) != p.window[2] || u64::from(r.height) != p.window[3] {
        return Err("WMTS dimensions must match the declared pixel grid".into());
    }
    let style = if r.style.is_empty() {
        meta.default_style.clone()
    } else {
        r.style
    };
    if !layer.styles.contains(&style) {
        return Err("Choose an advertised WMTS style".into());
    }
    match (&layer.time, &r.time) {
        (Some(d), Some(t)) if time_supported(d, t) => {}
        (None, None) => {}
        _ => return Err("Choose an explicit time supported by the WMTS layer".into()),
    }
    if let Some(area) = &r.area_geometry {
        let b = area.bounds()?;
        if b[0] < r.bounds[0] || b[1] < r.bounds[1] || b[2] > r.bounds[2] || b[3] > r.bounds[3] {
            return Err("Map bounds must include the selected polygon".into());
        }
    }
    let snapshot = Snapshot {
        resource_url: meta.resource_url.clone(),
        matrix_set: set.id.clone(),
        declared_crs: set.declared_crs.clone(),
        matrix: matrix.clone(),
        format: meta.format.clone(),
        time_identifier: meta.time_identifier.clone(),
        requested_bounds: r.bounds,
        pixel_window: p.window,
        tiles: Vec::new(),
        archive_sha256: "0".repeat(64),
        archive_bytes: 1,
    };
    let source = MapSource {
        xyz: None,
        arcgis: None,
        service_url: service.url.clone(),
        service_name: service.name.clone(),
        service_title: service.title.clone(),
        version: service.version.clone(),
        map_endpoint: service.map_url.clone(),
        capabilities_sha256: service.capabilities_sha256.clone(),
        layer_name: layer.name.clone(),
        layer_title: layer.title.clone(),
        style,
        time: r.time,
        request_crs: set.crs.clone(),
        request_url: String::new(),
        requested_at: now(),
        access_constraints: service.access_constraints.clone(),
        attribution: layer.attribution.clone(),
        area_geometry: r.area_geometry,
        selection: "pixel-window-rendered-tiles".into(),
        wmts: Some(snapshot),
    };
    let mut a = MapImage {
        id: Uuid::new_v4().to_string(),
        name: format!(
            "{} · WMTS",
            layer.title.chars().take(110).collect::<String>()
        ),
        width: p.window[2] as u32,
        height: p.window[3] as u32,
        bounds: p.bounds,
        bytes: 1,
        sha256: "0".repeat(64),
        crs: set.crs.clone(),
        source,
        image_extent: Some(p.extent),
    };
    let _permit = manager
        .inner
        .thumbnail_permits
        .acquire()
        .await
        .map_err(io_error)?;
    let settings = manager.proxy_settings().await;
    let responses = tokio::time::timeout(Duration::from_secs(120), async {
        let client = features::client(&service_url(&service.url)?, &settings).await?;
        let mut tiles = Vec::new();
        for (row, col) in p.tiles {
            let s = a.source.wmts.as_ref().unwrap();
            let u = tile_url(&a.source.map_endpoint, &a.source, s, row, col)?;
            let bytes = fetch_tile(&client, &u, &s.format).await?;
            let receipt = TileReceipt {
                row,
                col,
                request_url: u.to_string(),
                bytes: bytes.len(),
                sha256: hash(&bytes),
            };
            a.source.wmts.as_mut().unwrap().tiles.push(receipt);
            tiles.push(bytes);
        }
        Ok::<_, String>(tiles)
    })
    .await
    .map_err(|_| "WMTS retrieval timed out; no image was registered")??;
    a.source.request_url = a.source.wmts.as_ref().unwrap().tiles[0].request_url.clone();
    let (a, png, archive) = tokio::task::spawn_blocking(move || {
        let (png, archive) = assemble(&a, &responses)?;
        let s = a.source.wmts.as_mut().unwrap();
        s.archive_sha256 = hash(&archive);
        s.archive_bytes = archive.len();
        a.sha256 = hash(&png);
        a.bytes = png.len();
        validate_asset(&a)?;
        Ok::<_, String>((a, png, archive))
    })
    .await
    .map_err(io_error)??;
    manager.save_map_image(a, png, Some(archive)).await
}
