//! Bounded, explicit dependency discovery. This is not a geometry repair engine
//! or a complete glTF/3D Tiles conformance validator.
use super::{io_error, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Tileset,
    Gltf,
    Glb,
    B3dm,
    Buffer,
    Png,
    Jpeg,
    Schema,
}
impl Kind {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Tileset | Self::Gltf | Self::Schema => "json",
            Self::Glb => "glb",
            Self::B3dm => "b3dm",
            Self::Buffer => "bin",
            Self::Png => "png",
            Self::Jpeg => "jpg",
        }
    }
    pub fn media(self) -> &'static str {
        match self {
            Self::Tileset | Self::Gltf | Self::Schema => "application/json",
            Self::Glb => "model/gltf-binary",
            Self::B3dm | Self::Buffer => "application/octet-stream",
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Purpose {
    Content,
    Buffer,
    Image,
    Schema,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Reference {
    pub pointer: String,
    pub uri: String,
    pub purpose: Purpose,
    pub min_bytes: u64,
}
#[derive(Debug)]
pub struct Analysis {
    pub kind: Kind,
    pub references: Vec<Reference>,
    pub document: Option<Value>,
    pub tile_count: usize,
}

fn integer(v: &Value, limit: u64) -> Result<u64> {
    v.as_u64()
        .filter(|n| *n <= limit)
        .ok_or_else(|| "Invalid 3D integer or resource limit".into())
}
fn number(v: &Value) -> Result<f64> {
    v.as_f64()
        .filter(|n| n.is_finite() && *n >= 0.0)
        .ok_or_else(|| "Invalid 3D geometric error".into())
}
fn array(v: &Value, len: usize) -> bool {
    v.as_array()
        .is_some_and(|a| a.len() == len && a.iter().all(|n| n.as_f64().is_some_and(f64::is_finite)))
}
fn volume(v: &Value) -> Result<()> {
    let Some(o) = v.as_object() else {
        return Err("3D tile has no bounding volume".into());
    };
    if o.len() != 1 {
        return Err("Ambiguous or unsupported 3D bounding volume".into());
    }
    let valid = if let Some(a) = o.get("box") {
        array(a, 12)
    } else if let Some(a) = o.get("sphere") {
        array(a, 4) && a[3].as_f64().unwrap_or(-1.0) > 0.0
    } else if let Some(a) = o.get("region") {
        array(a, 6)
            && a[0].as_f64().unwrap().abs() <= std::f64::consts::PI
            && a[2].as_f64().unwrap().abs() <= std::f64::consts::PI
            && a[1].as_f64().unwrap().abs() <= std::f64::consts::FRAC_PI_2
            && a[3].as_f64().unwrap().abs() <= std::f64::consts::FRAC_PI_2
            && a[1].as_f64() <= a[3].as_f64()
            && a[4].as_f64() <= a[5].as_f64()
    } else {
        false
    };
    if valid {
        Ok(())
    } else {
        Err("Invalid or unsupported 3D bounding volume".into())
    }
}
fn extensions(v: &Value, gltf: bool) -> Result<()> {
    let allowed = if gltf {
        &[
            "KHR_materials_unlit",
            "KHR_texture_transform",
            "KHR_mesh_quantization",
            "EXT_mesh_features",
            "EXT_structural_metadata",
            "EXT_mesh_gpu_instancing",
        ][..]
    } else {
        &[][..]
    };
    for key in ["extensionsUsed", "extensionsRequired"] {
        if let Some(all) = v.get(key) {
            let all = all
                .as_array()
                .filter(|a| a.len() <= 16)
                .ok_or("Invalid 3D extensions")?;
            for e in all {
                if e.as_str().is_none_or(|s| !allowed.contains(&s)) {
                    return Err(format!("Unsupported 3D extension: {e}"));
                }
            }
        }
    }
    if let Some(required) = v["extensionsRequired"].as_array() {
        if required.iter().any(|e| {
            v["extensionsUsed"]
                .as_array()
                .is_none_or(|a| !a.contains(e))
        }) {
            return Err("Required 3D extension is not declared as used".into());
        }
    }
    fn walk(v: &Value, allowed: &[&str]) -> Result<()> {
        match v {
            Value::Object(o) => {
                if let Some(e) = o.get("extensions") {
                    let e = e.as_object().ok_or("Invalid 3D extensions object")?;
                    if let Some(k) = e.keys().find(|k| !allowed.contains(&k.as_str())) {
                        return Err(format!("Unsupported 3D extension: {k}"));
                    }
                }
                for x in o.values() {
                    walk(x, allowed)?;
                }
            }
            Value::Array(a) => {
                for x in a {
                    walk(x, allowed)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    walk(v, allowed)
}
fn reference(
    v: &Value,
    pointer: String,
    purpose: Purpose,
    min_bytes: u64,
    out: &mut Vec<Reference>,
) -> Result<()> {
    let uri = v
        .as_str()
        .filter(|s| {
            !s.is_empty()
                && s.len()
                    <= if s.starts_with("data:") {
                        super::MAX_RESOURCE * 4 / 3 + 64
                    } else {
                        2048
                    }
                && !s.chars().any(char::is_control)
        })
        .ok_or("Invalid 3D dependency URI")?;
    if uri.starts_with("data:") {
        if !matches!(purpose, Purpose::Image | Purpose::Buffer) {
            return Err("Inline 3D content is unsupported".into());
        }
        let (header, b64) = uri.split_once(',').ok_or("Invalid inline glTF resource")?;
        if ![
            "data:application/octet-stream;base64",
            "data:application/gltf-buffer;base64",
            "data:image/png;base64",
            "data:image/jpeg;base64",
        ]
        .contains(&header)
        {
            return Err("Unsupported inline glTF resource".into());
        }
        let data = STANDARD
            .decode(b64)
            .map_err(|_| "Invalid inline glTF resource")?;
        if (data.len() as u64) < min_bytes || data.len() > super::MAX_RESOURCE {
            return Err("Inline glTF resource length mismatch".into());
        }
        if purpose == Purpose::Image {
            image_kind(&data)?;
        }
        if purpose == Purpose::Buffer && !header.contains("application/")
            || purpose == Purpose::Image && !header.contains("image/")
        {
            return Err("Inline glTF resource type mismatch".into());
        }
        return Ok(());
    }
    out.push(Reference {
        pointer,
        uri: uri.into(),
        purpose,
        min_bytes,
    });
    if out.len() > super::MAX_RESOURCES {
        return Err("3D dependency count exceeds the package limit".into());
    }
    Ok(())
}
fn tile(
    v: &Value,
    p: &str,
    depth: usize,
    count: &mut usize,
    out: &mut Vec<Reference>,
) -> Result<()> {
    *count += 1;
    if depth > 32 || *count > 4096 || !v.is_object() {
        return Err("3D tile tree exceeds the supported depth or size".into());
    }
    if v.get("implicitTiling").is_some() {
        return Err("Implicit 3D Tiles are not supported; no partial package was saved".into());
    }
    volume(&v["boundingVolume"])?;
    number(&v["geometricError"])?;
    if let Some(x) = v.get("viewerRequestVolume") {
        volume(x)?;
    }
    if let Some(x) = v.get("refine") {
        if !["ADD", "REPLACE"].contains(&x.as_str().unwrap_or("")) {
            return Err("Invalid 3D refinement mode".into());
        }
    }
    if let Some(x) = v.get("transform") {
        if !array(x, 16)
            || [3, 7, 11].iter().any(|i| x[*i].as_f64() != Some(0.0))
            || x[15].as_f64() != Some(1.0)
        {
            return Err("Invalid affine 3D transform".into());
        }
        let a = x.as_array().unwrap();
        let f = |i: usize| a[i].as_f64().unwrap();
        let det = f(0) * (f(5) * f(10) - f(9) * f(6)) - f(4) * (f(1) * f(10) - f(9) * f(2))
            + f(8) * (f(1) * f(6) - f(5) * f(2));
        if !det.is_finite() || det.abs() < 1e-20 {
            return Err("Singular 3D transform".into());
        }
    }
    if v.get("content").is_some() && v.get("contents").is_some() {
        return Err("Ambiguous 3D tile content".into());
    }
    let mut content = |x: &Value, cp: String| -> Result<()> {
        if !x.is_object() {
            return Err("Invalid 3D tile content".into());
        }
        if let Some(b) = x.get("boundingVolume") {
            volume(b)?;
        }
        let (key, u) = match (x.get("uri"), x.get("url")) {
            (Some(u), None) => ("uri", u),
            (None, Some(u)) => ("url", u),
            _ => return Err("Missing or ambiguous 3D content URI".into()),
        };
        reference(u, format!("{cp}/{key}"), Purpose::Content, 0, out)
    };
    if let Some(x) = v.get("content") {
        content(x, format!("{p}/content"))?;
    }
    if let Some(xs) = v.get("contents") {
        let xs = xs
            .as_array()
            .filter(|a| a.len() <= 64)
            .ok_or("Invalid 3D multiple contents")?;
        for (i, x) in xs.iter().enumerate() {
            content(x, format!("{p}/contents/{i}"))?;
        }
    }
    if let Some(xs) = v.get("children") {
        let xs = xs
            .as_array()
            .filter(|a| a.len() <= 4096)
            .ok_or("Invalid 3D children")?;
        for (i, x) in xs.iter().enumerate() {
            tile(x, &format!("{p}/children/{i}"), depth + 1, count, out)?;
        }
    }
    Ok(())
}
fn gltf(v: &Value, binary_len: Option<usize>) -> Result<Vec<Reference>> {
    if v["asset"]["version"] != "2.0" {
        return Err("Only glTF 2.0 models are supported".into());
    }
    extensions(v, true)?;
    credits(v)?;
    let mut refs = Vec::new();
    for (key, purpose) in [("buffers", Purpose::Buffer), ("images", Purpose::Image)] {
        if let Some(xs) = v.get(key) {
            let xs = xs
                .as_array()
                .filter(|a| a.len() <= 256)
                .ok_or("Invalid glTF resource array")?;
            for (i, x) in xs.iter().enumerate() {
                let min = if purpose == Purpose::Buffer {
                    integer(&x["byteLength"], super::MAX_RESOURCE as u64)?
                } else {
                    0
                };
                if let Some(u) = x.get("uri") {
                    if x.get("bufferView").is_some() {
                        return Err("Ambiguous glTF image storage".into());
                    }
                    reference(u, format!("/{key}/{i}/uri"), purpose, min, &mut refs)?;
                } else if purpose == Purpose::Buffer {
                    if i != 0 || binary_len.is_none_or(|n| (n as u64) < min || n as u64 > min + 3) {
                        return Err("GLB binary buffer length mismatch".into());
                    }
                } else {
                    integer(&x["bufferView"], 256)?;
                    if !["image/png", "image/jpeg"].contains(&x["mimeType"].as_str().unwrap_or(""))
                    {
                        return Err("Unsupported embedded glTF image".into());
                    }
                }
            }
        }
    }
    if let Some(u) = v.pointer("/extensions/EXT_structural_metadata/schemaUri") {
        reference(
            u,
            "/extensions/EXT_structural_metadata/schemaUri".into(),
            Purpose::Schema,
            0,
            &mut refs,
        )?;
    }
    // Standard fetchable URIs must occur at one of the locations collected above.
    // Unknown 'uri' fields are rejected instead of silently acquiring an incomplete graph.
    audit_uris(v, "", &refs)?;
    if let Some(xs) = v.get("bufferViews") {
        let xs = xs
            .as_array()
            .filter(|a| a.len() <= 8192)
            .ok_or("Invalid glTF buffer views")?;
        for x in xs {
            let i = integer(&x["buffer"], 255)? as usize;
            let off = match x.get("byteOffset") {
                Some(n) => integer(n, super::MAX_RESOURCE as u64)?,
                None => 0,
            };
            let len = integer(&x["byteLength"], super::MAX_RESOURCE as u64)?;
            let total = v["buffers"][i]["byteLength"]
                .as_u64()
                .ok_or("glTF buffer view refers to a missing buffer")?;
            if off + len > total {
                return Err("glTF buffer view exceeds its buffer".into());
            }
        }
    }
    Ok(refs)
}
fn credits(v: &Value) -> Result<()> {
    let plain = |x: &Value| {
        x.as_str().is_some_and(|s| {
            s.len() <= 4096 && !s.contains(['<', '>']) && !s.chars().any(char::is_control)
        })
    };
    if v["asset"].get("copyright").is_some_and(|c| !plain(c)) {
        return Err("Offline 3D credits must use plain text".into());
    }
    if let Some(all) = v.pointer("/asset/extras/cesium/credits") {
        let all = all
            .as_array()
            .filter(|a| a.len() <= 64)
            .ok_or("Invalid 3D credits")?;
        if all.iter().any(|c| !plain(&c["html"])) {
            return Err("Offline 3D credits must use plain text".into());
        }
    }
    Ok(())
}
fn audit_uris(v: &Value, p: &str, refs: &[Reference]) -> Result<()> {
    match v {
        Value::Object(o) => {
            for (k, x) in o {
                let next = format!("{p}/{}", k.replace('~', "~0").replace('/', "~1"));
                if ["uri", "url", "schemaUri"].contains(&k.as_str())
                    && !x.as_str().is_some_and(|s| s.starts_with("data:"))
                    && !refs.iter().any(|r| r.pointer == next)
                {
                    return Err(format!("Unsupported external 3D reference: {next}"));
                }
                audit_uris(x, &next, refs)?;
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                audit_uris(x, &format!("{p}/{i}"), refs)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn image_kind(b: &[u8]) -> Result<Kind> {
    if b.starts_with(b"\x89PNG\r\n\x1a\n") && b.len() >= 33 {
        Ok(Kind::Png)
    } else if b.starts_with(b"\xff\xd8\xff") && b.len() >= 4 && b.ends_with(b"\xff\xd9") {
        Ok(Kind::Jpeg)
    } else {
        Err("Only PNG or JPEG glTF textures are supported".into())
    }
}
fn u32_at(b: &[u8], i: usize) -> Result<usize> {
    Ok(u32::from_le_bytes(
        b.get(i..i + 4)
            .ok_or("Truncated 3D binary header")?
            .try_into()
            .unwrap(),
    ) as usize)
}
pub fn binary_document(b: &[u8], kind: Kind) -> Result<(Value, usize, usize, Option<usize>)> {
    let start = if kind == Kind::B3dm {
        if b.get(..4) != Some(b"b3dm") || u32_at(b, 4)? != 1 || u32_at(b, 8)? != b.len() {
            return Err("Invalid b3dm header or length".into());
        }
        let ft = u32_at(b, 12)?;
        let fb = u32_at(b, 16)?;
        let bt = u32_at(b, 20)?;
        let bb = u32_at(b, 24)?;
        let start = 28usize
            .checked_add(ft)
            .and_then(|n| n.checked_add(fb))
            .and_then(|n| n.checked_add(bt))
            .and_then(|n| n.checked_add(bb))
            .filter(|n| *n <= b.len())
            .ok_or("Invalid b3dm table lengths")?;
        if start % 8 != 0 || ft == 0 {
            return Err("Unsupported b3dm alignment or feature table".into());
        }
        let table: Value = serde_json::from_slice(&b[28..28 + ft]).map_err(io_error)?;
        integer(&table["BATCH_LENGTH"], 1_000_000)?;
        if let Some(r) = table.get("RTC_CENTER") {
            if !array(r, 3) {
                return Err("Unsupported b3dm RTC_CENTER storage".into());
            }
        }
        if bt > 0 {
            let _: Value =
                serde_json::from_slice(&b[28 + ft + fb..28 + ft + fb + bt]).map_err(io_error)?;
        }
        start
    } else {
        0
    };
    let glb = &b[start..];
    if glb.get(..4) != Some(b"glTF") || u32_at(glb, 4)? != 2 || u32_at(glb, 8)? != glb.len() {
        return Err("Invalid GLB version or length".into());
    }
    let len = u32_at(glb, 12)?;
    if len % 4 != 0 || u32_at(glb, 16)? != 0x4e4f534a || len > glb.len().saturating_sub(20) {
        return Err("Invalid GLB JSON chunk".into());
    }
    let doc = serde_json::from_slice(&glb[20..20 + len]).map_err(io_error)?;
    let tail = 20 + len;
    let bin = if tail == glb.len() {
        None
    } else {
        let n = u32_at(glb, tail)?;
        if n % 4 != 0 || u32_at(glb, tail + 4)? != 0x004e4942 || tail + 8 + n != glb.len() {
            return Err("Unsupported GLB chunk structure".into());
        }
        Some(n)
    };
    Ok((doc, start + 20, start + tail, bin))
}
pub fn analyze(b: &[u8], purpose: Purpose) -> Result<Analysis> {
    if b.is_empty() || b.len() > super::MAX_RESOURCE {
        return Err("3D resource is empty or exceeds 32 MiB".into());
    }
    let mut count = 0;
    let (kind, document, refs) = match purpose {
        Purpose::Buffer => (Kind::Buffer, None, Vec::new()),
        Purpose::Image => (image_kind(b)?, None, Vec::new()),
        Purpose::Schema => {
            let v: Value = serde_json::from_slice(b).map_err(io_error)?;
            if !v.is_object() || v.get("$ref").is_some() {
                return Err("Unsupported external 3D metadata schema".into());
            }
            fn no_ref(v: &Value) -> bool {
                match v {
                    Value::Object(o) => !o.contains_key("$ref") && o.values().all(no_ref),
                    Value::Array(a) => a.iter().all(no_ref),
                    _ => true,
                }
            }
            if !no_ref(&v) {
                return Err("External schema references are unsupported".into());
            }
            (Kind::Schema, Some(v), Vec::new())
        }
        Purpose::Content => {
            if b.starts_with(b"glTF") || b.starts_with(b"b3dm") {
                let k = if b.starts_with(b"b3dm") {
                    Kind::B3dm
                } else {
                    Kind::Glb
                };
                let (v, _, _, bin) = binary_document(b, k)?;
                let refs = gltf(&v, bin)?;
                (k, Some(v), refs)
            } else {
                let v: Value = serde_json::from_slice(b).map_err(|_| {
                    "Unsupported 3D content; use explicit tilesets, glTF, GLB or b3dm"
                })?;
                if v.get("root").is_some() {
                    if !["1.0", "1.1"].contains(&v["asset"]["version"].as_str().unwrap_or("")) {
                        return Err("Only 3D Tiles 1.0 and 1.1 are supported".into());
                    }
                    extensions(&v, false)?;
                    credits(&v)?;
                    number(&v["geometricError"])?;
                    let mut refs = Vec::new();
                    tile(&v["root"], "/root", 0, &mut count, &mut refs)?;
                    if let Some(u) = v.get("schemaUri") {
                        reference(u, "/schemaUri".into(), Purpose::Schema, 0, &mut refs)?;
                    }
                    audit_uris(&v, "", &refs)?;
                    (Kind::Tileset, Some(v), refs)
                } else {
                    let refs = gltf(&v, None)?;
                    (Kind::Gltf, Some(v), refs)
                }
            }
        }
    };
    Ok(Analysis {
        kind,
        references: refs,
        document,
        tile_count: count,
    })
}
pub fn rewrite(b: &[u8], kind: Kind, replacements: &[(String, String)]) -> Result<Vec<u8>> {
    if replacements.is_empty() {
        return Ok(b.to_vec());
    }
    let (mut v, range) = if matches!(kind, Kind::Glb | Kind::B3dm) {
        let (v, start, end, _) = binary_document(b, kind)?;
        (v, Some((start, end)))
    } else {
        (serde_json::from_slice(b).map_err(io_error)?, None)
    };
    for (p, u) in replacements {
        let target = v
            .pointer_mut(p)
            .ok_or("Saved 3D dependency pointer changed")?;
        if !target.is_string() {
            return Err("Saved 3D dependency pointer changed".into());
        }
        *target = Value::String(u.clone());
    }
    let mut json = serde_json::to_vec(&v).map_err(io_error)?;
    let Some((start, end)) = range else {
        return Ok(json);
    };
    let alignment = if kind == Kind::B3dm { 8 } else { 4 };
    while !(start + json.len() + b.len() - end).is_multiple_of(alignment) {
        json.push(b' ');
    }
    let mut out = b[..start].to_vec();
    out[start - 8..start - 4].copy_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend(json);
    out.extend_from_slice(&b[end..]);
    let glb_start = start - 20;
    let glb_len = out.len() - glb_start;
    out[glb_start + 8..glb_start + 12].copy_from_slice(&(glb_len as u32).to_le_bytes());
    if kind == Kind::B3dm {
        let n = out.len() as u32;
        out[8..12].copy_from_slice(&n.to_le_bytes());
    }
    Ok(out)
}
