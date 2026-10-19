//! Native geoBoundaries gbOpen country files. This is not an OGC/WFS service.
//! Download the publisher's complete original GeoJSON at its immutable Git ref.
use super::*;

#[cfg(test)]
mod tests;

const API_HOST: &str = "www.geoboundaries.org";
const RELEASE_TYPE: &str = "gbOpen";
const LICENSE: &str = "https://creativecommons.org/licenses/by/4.0/";
const ATTRIBUTION: &str = "geoBoundaries · William & Mary geoLab · CC BY 4.0";
const MAX_DISCOVERY: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Layer {
    pub boundary_id: String,
    pub country_iso3: String,
    pub country_name: String,
    pub admin_level: String,
    pub year_represented: String,
    pub original_source: String,
    pub original_license: String,
    pub license_source: String,
    pub source_data_update_date: String,
    pub build_date: String,
    pub unit_count: usize,
    pub release_ref: String,
    pub download_url: String,
    /// The complete publisher response for this boundary, including optional fields.
    pub metadata: Value,
    /// SHA-256 of canonical JSON for this individual metadata record.
    pub metadata_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Service {
    pub release_type: String,
    pub country_iso3: String,
    /// Hash and byte size of the actual discovery HTTP document.
    pub discovery_receipt: PageReceipt,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub release_type: String,
    pub layer: Layer,
    pub discovery_receipt: PageReceipt,
    /// Full commit resolved by the publisher's GitHub redirect, when supplied.
    pub resolved_release_commit: String,
    /// Exact publisher CRS declaration retained in the managed source document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_crs: Option<Value>,
    pub attribution: String,
    pub geometry_selection: String,
}

fn digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn iso3(s: &str) -> bool {
    s.len() == 3 && s != "ALL" && s.bytes().all(|b| b.is_ascii_uppercase())
}

fn admin(s: &str) -> bool {
    matches!(s, "ADM0" | "ADM1" | "ADM2" | "ADM3" | "ADM4" | "ADM5")
}

fn layer_api(country: &str, level: &str) -> String {
    format!("https://{API_HOST}/api/current/{RELEASE_TYPE}/{country}/{level}/")
}

fn root_url(raw: &str) -> Result<(Url, String)> {
    let u = public_url(raw)?;
    let parts: Vec<_> = u.path().trim_matches('/').split('/').collect();
    if u.host_str() != Some(API_HOST)
        || u.query().is_some()
        || parts.len() != 5
        || parts[..3] != ["api", "current", RELEASE_TYPE]
        || !iso3(parts[3])
        || parts[4] != "ALL"
    {
        return Err(
            "Use the geoBoundaries gbOpen country API URL ending in an ISO3 code and /ALL/".into(),
        );
    }
    let country = parts[3].to_string();
    Ok((
        Url::parse(&format!(
            "https://{API_HOST}/api/current/{RELEASE_TYPE}/{country}/ALL/"
        ))
        .map_err(io_error)?,
        country,
    ))
}

/// Only the exact original release file, with a commit hash, is accepted.
/// Publisher GitHub redirects must retain the repository, commit and filename.
fn download_url(raw: &str, country: &str, level: &str) -> Result<(Url, String)> {
    let u = public_url(raw)?;
    if u.query().is_some() {
        return Err("geoBoundaries file URLs must not contain query parameters".into());
    }
    let path: Vec<_> = u.path().trim_matches('/').split('/').collect();
    let tail = match u.host_str() {
        Some("github.com") if path.starts_with(&["wmgeolab", "geoBoundaries", "raw"]) => &path[3..],
        Some("raw.githubusercontent.com") if path.starts_with(&["wmgeolab", "geoBoundaries"]) => {
            &path[2..]
        }
        Some("media.githubusercontent.com")
            if path.starts_with(&["media", "wmgeolab", "geoBoundaries"]) =>
        {
            &path[3..]
        }
        _ => {
            return Err(
                "geoBoundaries downloads must use the publisher's release repository".into(),
            )
        }
    };
    if tail.len() != 6
        || !(7..=40).contains(&tail[0].len())
        || !tail[0]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || tail[1..5] != ["releaseData", RELEASE_TYPE, country, level]
        || tail[5] != format!("geoBoundaries-{country}-{level}.geojson")
    {
        return Err(
            "geoBoundaries file must be the complete original GeoJSON pinned to a Git commit"
                .into(),
        );
    }
    let release_ref = tail[0].to_string();
    Ok((u, release_ref))
}

fn text(v: &Value, key: &str, max: usize) -> Result<String> {
    let value = v[key]
        .as_str()
        .ok_or_else(|| format!("geoBoundaries metadata has no {key}"))?;
    if !clean(value, max) {
        return Err(format!("Invalid geoBoundaries metadata field {key}"));
    }
    Ok(value.to_string())
}

fn parse_layer(v: &Value, country: &str) -> Result<Layer> {
    if !v.is_object() || serde_json::to_vec(v).map_err(io_error)?.len() > 65_536 {
        return Err("Invalid geoBoundaries layer metadata".into());
    }
    let country_iso3 = text(v, "boundaryISO", 3)?;
    let admin_level = text(v, "boundaryType", 4)?;
    let boundary_id = text(v, "boundaryID", 120)?;
    if country_iso3 != country
        || !iso3(&country_iso3)
        || !admin(&admin_level)
        || !boundary_id.starts_with(&format!("{country}-{admin_level}-"))
    {
        return Err(
            "geoBoundaries metadata changed the selected country or administrative level".into(),
        );
    }
    let unit_count = text(v, "admUnitCount", 12)?
        .parse::<usize>()
        .map_err(|_| "Invalid geoBoundaries administrative-unit count")?;
    if unit_count == 0 || unit_count > vector::MAX_FEATURES {
        return Err("geoBoundaries country file exceeds the complete feature limit".into());
    }
    let (download, release_ref) =
        download_url(&text(v, "gjDownloadURL", 2048)?, country, &admin_level)?;
    Ok(Layer {
        boundary_id,
        country_iso3,
        country_name: text(v, "boundaryName", 160)?,
        admin_level,
        year_represented: text(v, "boundaryYearRepresented", 120)?,
        original_source: text(v, "boundarySource", 8192)?,
        original_license: text(v, "boundaryLicense", 2048)?,
        license_source: text(v, "licenseSource", 4096)?,
        source_data_update_date: text(v, "sourceDataUpdateDate", 160)?,
        build_date: text(v, "buildDate", 160)?,
        unit_count,
        release_ref,
        download_url: download.to_string(),
        metadata: v.clone(),
        metadata_sha256: hash(&serde_json::to_vec(v).map_err(io_error)?),
    })
}

fn validate_layer(layer: &Layer) -> Result<()> {
    if parse_layer(&layer.metadata, &layer.country_iso3)? != *layer {
        return Err("geoBoundaries layer metadata or fingerprint changed".into());
    }
    Ok(())
}

fn validate_discovery(receipt: &PageReceipt, root: &str, count: Option<usize>) -> Result<()> {
    if receipt.url != root
        || receipt.bytes == 0
        || receipt.bytes > MAX_DISCOVERY
        || !digest(&receipt.sha256)
        || receipt.parameters.is_some()
        || !(1..=6).contains(&receipt.returned)
        || count.is_some_and(|n| receipt.returned != n)
    {
        return Err("Invalid geoBoundaries discovery receipt".into());
    }
    Ok(())
}

/// The review pins known publisher metadata, not a future file response hash.
pub(crate) fn validate_review(
    layer: &Layer,
    receipt: &PageReceipt,
    service_url: &str,
) -> Result<()> {
    let (root, country) = root_url(service_url)?;
    validate_layer(layer)?;
    validate_discovery(receipt, root.as_str(), None)?;
    if root.as_str() != service_url || layer.country_iso3 != country {
        return Err("geoBoundaries review changed its country or discovery source".into());
    }
    Ok(())
}

async fn fetch_bytes(
    settings: &crate::ProxySettings,
    start: &Url,
    max: usize,
    file_identity: Option<(&str, &str, &str)>,
) -> Result<(Vec<u8>, String)> {
    let mut u = start.clone();
    let mut visited = BTreeSet::new();
    for _ in 0..5 {
        if !visited.insert(u.to_string()) {
            return Err("geoBoundaries download repeated a redirect".into());
        }
        let c = client_with_timeout(&u, settings, Duration::from_secs(90)).await?;
        let response = c
            .get(u.clone())
            .header(
                "Accept",
                "application/geo+json, application/json, text/plain",
            )
            .send()
            .await
            .map_err(|_| "Cannot reach geoBoundaries; check the service and proxy settings")?;
        if response.status().is_redirection() {
            let location = response
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .ok_or("geoBoundaries redirect has no destination")?;
            let next = public_url(u.join(location).map_err(io_error)?.as_str())?;
            match file_identity {
                Some((country, level, git_ref)) => {
                    let (_, redirected_ref) = download_url(next.as_str(), country, level)?;
                    if !redirected_ref.starts_with(git_ref) {
                        return Err("geoBoundaries redirect changed the release commit".into());
                    }
                }
                None if next != *start => {
                    return Err(
                        "geoBoundaries metadata redirected outside the selected endpoint".into(),
                    )
                }
                None => {}
            }
            u = next;
            continue;
        }
        if !response.status().is_success() {
            return Err(format!(
                "geoBoundaries returned HTTP {}",
                response.status().as_u16()
            ));
        }
        let mime = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("");
        if !matches!(
            mime,
            "application/json" | "application/geo+json" | "text/plain" | "application/octet-stream"
        ) {
            return Err("geoBoundaries did not return a JSON document".into());
        }
        if response.content_length().is_some_and(|n| n > max as u64) {
            return Err("geoBoundaries original country file exceeds the complete download limit; no simplified substitute was used".into());
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk
                .map_err(|_| "geoBoundaries download was interrupted; no partial file was saved")?;
            if bytes.len().saturating_add(chunk.len()) > max {
                return Err("geoBoundaries original country file exceeds the complete download limit; no simplified substitute was used".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        return Ok((bytes, u.to_string()));
    }
    Err("geoBoundaries download exceeded the redirect limit".into())
}

pub(super) async fn discover(
    request: ConnectRequest,
    settings: crate::ProxySettings,
) -> Result<FeatureService> {
    let name = request.name.trim();
    if !clean(name, 80) || request.protocol != "geoBoundaries" {
        return Err("Enter a geoBoundaries connection name of 1–80 characters".into());
    }
    let (root, country) = root_url(request.url.trim())?;
    let (bytes, _) = fetch_bytes(&settings, &root, MAX_DISCOVERY, None).await?;
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|_| "geoBoundaries metadata is not valid JSON")?;
    let records = value
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 6)
        .ok_or("geoBoundaries has no available levels for this country")?;
    let mut collections = Vec::new();
    let mut ids = BTreeSet::new();
    for v in records {
        let layer = parse_layer(v, &country)?;
        if !ids.insert(layer.admin_level.clone()) {
            return Err("geoBoundaries repeated an administrative level".into());
        }
        let title = format!("{} · {}", layer.country_name, layer.admin_level);
        let description = format!("Complete country boundaries · {} · source year {} · original license {} · release {}. geoBoundaries CC BY 4.0; original attributes and geometry retained.", layer.admin_level, layer.year_represented, layer.original_license, layer.release_ref);
        collections.push(Collection {
            id: layer.admin_level.clone(),
            title,
            description,
            items_url: layer_api(&layer.country_iso3, &layer.admin_level),
            license_links: vec![LICENSE.into()],
            arcgis: None,
            wfs: None,
            geoboundaries: Some(layer),
        });
    }
    collections.sort_by(|a, b| a.id.cmp(&b.id));
    let service = FeatureService {
        id: Uuid::new_v4().to_string(),
        name: name.into(),
        url: root.to_string(),
        title: format!("geoBoundaries · {country} · gbOpen"),
        collections,
        connected_at: now(),
        arcgis: None,
        overpass: None,
        wfs: None,
        geoboundaries: Some(Service {
            release_type: RELEASE_TYPE.into(),
            country_iso3: country,
            discovery_receipt: PageReceipt {
                url: root.to_string(),
                sha256: hash(&bytes),
                bytes: bytes.len(),
                returned: records.len(),
                parameters: None,
            },
        }),
    };
    validate_service(&service)?;
    Ok(service)
}

pub(super) fn validate_service(s: &FeatureService) -> Result<()> {
    let (root, country) = root_url(&s.url)?;
    let gb = s
        .geoboundaries
        .as_ref()
        .ok_or("Missing geoBoundaries service metadata")?;
    if s.url != root.as_str()
        || gb.country_iso3 != country
        || gb.release_type != RELEASE_TYPE
        || s.arcgis.is_some()
        || s.overpass.is_some()
        || s.wfs.is_some()
        || s.collections.is_empty()
        || s.collections.len() > 6
    {
        return Err("Invalid saved geoBoundaries service".into());
    }
    validate_discovery(&gb.discovery_receipt, &s.url, Some(s.collections.len()))?;
    let mut ids = BTreeSet::new();
    for collection in &s.collections {
        let layer = collection
            .geoboundaries
            .as_ref()
            .ok_or("Missing geoBoundaries layer metadata")?;
        validate_layer(layer)?;
        if layer.country_iso3 != country
            || !ids.insert(&collection.id)
            || collection.id != layer.admin_level
            || collection.items_url != layer_api(&country, &layer.admin_level)
            || collection.license_links != [LICENSE]
            || collection.arcgis.is_some()
            || collection.wfs.is_some()
            || !clean(&collection.title, 240)
            || collection.description.len() > 16_384
        {
            return Err("Invalid saved geoBoundaries collection".into());
        }
    }
    Ok(())
}

fn validate_crs(crs: Option<&Value>) -> Result<()> {
    if let Some(crs) = crs {
        let name = crs["properties"]["name"].as_str().unwrap_or("");
        if crs["type"] != "name"
            || name != "urn:ogc:def:crs:OGC:1.3:CRS84"
            || serde_json::to_vec(crs).map_err(io_error)?.len() > 4096
        {
            return Err("geoBoundaries file is not declared WGS84 longitude/latitude".into());
        }
    }
    Ok(())
}

fn validate_geometry(v: &Value, layer: &Layer) -> Result<usize> {
    if v["type"] != "FeatureCollection" || v.get("geodSource").is_some() {
        return Err("geoBoundaries did not return an original FeatureCollection".into());
    }
    validate_crs(v.get("crs"))?;
    let features = v["features"]
        .as_array()
        .ok_or("geoBoundaries file has no features array")?;
    if features.len() != layer.unit_count {
        return Err(
            "geoBoundaries original file does not match the declared administrative-unit count"
                .into(),
        );
    }
    let mut ids = BTreeSet::new();
    for feature in features {
        if feature["type"] != "Feature"
            || !matches!(
                feature["geometry"]["type"].as_str(),
                Some("Polygon" | "MultiPolygon")
            )
            || feature["properties"].as_object().is_none()
        {
            return Err(
                "geoBoundaries country file contains an unsupported boundary feature".into(),
            );
        }
        let props = &feature["properties"];
        let shape = props["shapeID"]
            .as_str()
            .ok_or("geoBoundaries boundary feature has no shapeID")?;
        if !clean(shape, 256)
            || !ids.insert(shape)
            || props["shapeGroup"] != layer.country_iso3
            || props["shapeType"] != layer.admin_level
            || props["shapeName"].as_str().is_none()
        {
            return Err("geoBoundaries boundary feature identity changed or is duplicated".into());
        }
    }
    vector::validate_integer_precision(v)?;
    Ok(features.len())
}

pub(super) async fn query(
    s: FeatureService,
    c: Collection,
    request: QueryRequest,
    settings: crate::ProxySettings,
) -> Result<(Value, Provenance, String)> {
    validate_service(&s)?;
    validate_query(&request)?;
    if s.id != request.service_id
        || c.id != request.collection_id
        || !s.collections.contains(&c)
        || request.response_format.is_some()
    {
        return Err("geoBoundaries country or administrative-level selection changed".into());
    }
    let layer = c
        .geoboundaries
        .clone()
        .ok_or("Missing geoBoundaries layer metadata")?;
    let (url, git_ref) =
        download_url(&layer.download_url, &layer.country_iso3, &layer.admin_level)?;
    let (bytes, final_url) = fetch_bytes(
        &settings,
        &url,
        vector::MAX_BYTES,
        Some((&layer.country_iso3, &layer.admin_level, &git_ref)),
    )
    .await?;
    let mut data: Value = serde_json::from_slice(&bytes)
        .map_err(|_| "geoBoundaries original file is not valid GeoJSON")?;
    let count = validate_geometry(&data, &layer)?;
    let source_crs = data.get("crs").cloned();
    let (_, resolved_release_commit) =
        download_url(&final_url, &layer.country_iso3, &layer.admin_level)?;
    let receipt = PageReceipt {
        url: final_url,
        sha256: hash(&bytes),
        bytes: bytes.len(),
        returned: count,
        parameters: None,
    };
    let source = Provenance {
        service_url: s.url,
        service_name: s.name,
        collection_id: c.id,
        collection_title: c.title.clone(),
        license_links: vec![LICENSE.into()],
        requested_bounds: request.bounds,
        area_geometry: request.area_geometry,
        requested_at: now(),
        pages: vec![receipt],
        number_matched: Some(count),
        feature_count: count,
        selection: "geoboundaries-full-administrative-level".into(),
        arcgis: None,
        wfs: None,
        geoboundaries: Some(Snapshot {
            release_type: RELEASE_TYPE.into(),
            layer,
            discovery_receipt: s.geoboundaries.unwrap().discovery_receipt,
            resolved_release_commit,
            source_crs,
            attribution: ATTRIBUTION.into(),
            geometry_selection: "geoboundaries-full-administrative-level".into(),
        }),
    };
    validate_provenance(&source, count)?;
    data.as_object_mut()
        .ok_or("Invalid geoBoundaries GeoJSON object")?
        .insert(
            "geodSource".into(),
            serde_json::to_value(&source).map_err(io_error)?,
        );
    Ok((data, source, c.title))
}

pub(super) fn validate_provenance(s: &Provenance, count: usize) -> Result<()> {
    let (root, country) = root_url(&s.service_url)?;
    let snapshot = s
        .geoboundaries
        .as_ref()
        .ok_or("Missing geoBoundaries snapshot provenance")?;
    let layer = &snapshot.layer;
    validate_layer(layer)?;
    validate_crs(snapshot.source_crs.as_ref())?;
    bounds(s.requested_bounds)?;
    if let Some(area) = &s.area_geometry {
        validate_area(area, s.requested_bounds)?;
    }
    validate_discovery(&snapshot.discovery_receipt, root.as_str(), None)?;
    if s.service_url != root.as_str()
        || !clean(&s.service_name, 80)
        || !clean(&s.collection_title, 240)
        || snapshot.release_type != RELEASE_TYPE
        || layer.country_iso3 != country
        || s.collection_id != layer.admin_level
        || s.selection != "geoboundaries-full-administrative-level"
        || snapshot.geometry_selection != s.selection
        || snapshot.attribution != ATTRIBUTION
        || s.feature_count != count
        || count != layer.unit_count
        || s.number_matched != Some(count)
        || s.license_links != [LICENSE]
        || s.pages.len() != 1
        || s.arcgis.is_some()
        || s.wfs.is_some()
        || chrono::DateTime::parse_from_rfc3339(&s.requested_at).is_err()
    {
        return Err("Invalid geoBoundaries country-file provenance".into());
    }
    let receipt = &s.pages[0];
    let (_, ref_value) = download_url(&receipt.url, &country, &layer.admin_level)?;
    if !ref_value.starts_with(&layer.release_ref)
        || snapshot.resolved_release_commit != ref_value
        || !digest(&receipt.sha256)
        || receipt.bytes == 0
        || receipt.bytes > vector::MAX_BYTES
        || receipt.returned != count
        || receipt.parameters.is_some()
    {
        return Err("Invalid geoBoundaries original-file receipt".into());
    }
    Ok(())
}

/// Rendering normalization only: the managed source bytes retain this declaration.
/// WGS84 coordinates, all properties and the complete country's features are unchanged.
pub(crate) fn normalize_for_display(data: &mut Value, source: &Provenance) -> Result<()> {
    let snapshot = source
        .geoboundaries
        .as_ref()
        .ok_or("Missing geoBoundaries snapshot provenance")?;
    if data.get("crs") != snapshot.source_crs.as_ref() {
        return Err("geoBoundaries source CRS differs from its recorded declaration".into());
    }
    validate_crs(data.get("crs"))?;
    data.as_object_mut()
        .ok_or("Invalid geoBoundaries source object")?
        .remove("crs");
    Ok(())
}
