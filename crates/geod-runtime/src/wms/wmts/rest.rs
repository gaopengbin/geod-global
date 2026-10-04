//! Bounded WMTS ResourceURL path templates; identifiers remain opaque.
use super::*;

pub(super) fn map_endpoint(root: &Url, raw: &str) -> Result<Url> {
    let mut u = features::public_url(root.join(raw).map_err(io_error)?.as_str())?;
    if u.origin() != root.origin()
        || u.query_pairs().any(|(k, _)| {
            !matches!(
                k.to_ascii_lowercase().as_str(),
                "service" | "version" | "request"
            )
        })
    {
        return Err(
            "WMTS request links must use the connected public origin without custom parameters"
                .into(),
        );
    }
    u.set_query(None);
    Ok(u)
}

pub(super) fn template(root: &Url, raw: &str, time: Option<&str>) -> Result<String> {
    if !text_valid(raw, 2048)
        || !raw.is_ascii()
        || raw.chars().any(char::is_whitespace)
        || raw.contains(['%', '\\', '?', '#'])
    {
        return Err("Unsupported WMTS REST tile template".into());
    }
    let mut remaining = raw;
    let mut names = BTreeSet::new();
    let mut probe = raw.to_string();
    while let Some((before, after)) = remaining.split_once('{') {
        if before.contains('}') {
            return Err("Unsupported WMTS REST tile template".into());
        }
        let (key, tail) = after
            .split_once('}')
            .ok_or("Unsupported WMTS REST tile template")?;
        if !names.insert(key)
            || !(matches!(
                key,
                "Layer" | "Style" | "TileMatrixSet" | "TileMatrix" | "TileRow" | "TileCol"
            ) || time == Some(key))
        {
            return Err("Unsupported WMTS REST tile placeholder".into());
        }
        probe = probe.replace(&format!("{{{key}}}"), "0");
        remaining = tail;
    }
    if remaining.contains('}')
        || ["TileMatrix", "TileRow", "TileCol"]
            .iter()
            .any(|k| !names.contains(k))
        || time.is_some_and(|t| !names.contains(t))
    {
        return Err(
            "WMTS REST template must include tile coordinates and every supported dimension".into(),
        );
    }
    let u = map_endpoint(root, &probe)?;
    let canonical = root
        .join(raw)
        .map_err(io_error)?
        .to_string()
        .replace("%7B", "{")
        .replace("%7D", "}");
    let path = canonical
        .split_once("://")
        .and_then(|(_, r)| r.split_once('/'))
        .map(|(_, p)| p)
        .unwrap_or("");
    if names
        .iter()
        .any(|key| !path.contains(&format!("{{{key}}}")))
        || canonical.contains('%')
        || u.host_str().is_some_and(|h| {
            h == "tile.openstreetmap.org" || h.ends_with(".tile.openstreetmap.org")
        })
    {
        return Err("WMTS REST placeholders require an allowed public URL path".into());
    }
    Ok(canonical)
}

fn segment(raw: &str) -> Result<String> {
    if !text_valid(raw, 256) || matches!(raw, "." | "..") {
        return Err("Unsupported WMTS REST identifier".into());
    }
    Ok(raw
        .as_bytes()
        .iter()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(*b, b'-' | b'.' | b'_' | b'~') {
                char::from(*b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect())
}

pub(super) fn tile_url(
    raw: &str,
    source: &MapSource,
    s: &Snapshot,
    row: u32,
    col: u32,
) -> Result<Url> {
    let root = service_url(&source.service_url)?;
    let mut url = template(&root, raw, s.time_identifier.as_deref())?;
    for (k, v) in [
        ("Layer", source.layer_name.as_str()),
        ("Style", source.style.as_str()),
        ("TileMatrixSet", s.matrix_set.as_str()),
        ("TileMatrix", s.matrix.id.as_str()),
        ("TileRow", &row.to_string()),
        ("TileCol", &col.to_string()),
    ] {
        if url.contains(&format!("{{{k}}}")) {
            url = url.replace(&format!("{{{k}}}"), &segment(v)?);
        }
    }
    if let Some(key) = &s.time_identifier {
        url = url.replace(
            &format!("{{{key}}}"),
            &segment(source.time.as_deref().ok_or("WMTS REST time is missing")?)?,
        );
    }
    let u = features::public_url(&url)?;
    if u.origin() != root.origin() || u.query().is_some() {
        return Err("Invalid resolved WMTS REST tile URL".into());
    }
    Ok(u)
}
