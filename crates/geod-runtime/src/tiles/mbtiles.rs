//! Read-only MBTiles 1.3. TMS storage is exposed to the map as XYZ.
//! The database is copied unchanged and queried in memory, with no extension or file access.
use super::*;
use rusqlite::{
    hooks::{AuthAction, AuthContext, Authorization},
    limits::Limit,
    Connection,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Descriptor {
    pub format: String,
    pub scheme: String,
    pub min_zoom: u8,
    pub max_zoom: u8,
    pub bounds: [f64; 4],
    pub addressed_tiles: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile_size: Option<u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub content_type: String,
}
fn sqlite_error(_: rusqlite::Error) -> String {
    "Invalid or unsupported MBTiles database; check its schema, integrity and query limits".into()
}
fn connection(bytes: &[u8]) -> Result<Connection> {
    if bytes.len() < 100 || bytes.len() > MAX_PACKAGE || !bytes.starts_with(b"SQLite format 3\0") {
        return Err("Choose a valid MBTiles SQLite database, up to 128 MiB".into());
    }
    // A standalone copy cannot incorporate an external WAL or rollback journal.
    if bytes[18] != 1 || bytes[19] != 1 {
        return Err(
            "Checkpoint the MBTiles database into a standalone file before importing".into(),
        );
    }
    let mut db = Connection::open_in_memory().map_err(sqlite_error)?;
    db.deserialize_read_exact("main", Cursor::new(bytes), bytes.len(), true)
        .map_err(sqlite_error)?;
    for (limit, value) in [
        (Limit::SQLITE_LIMIT_LENGTH, (format::MAX_TILE + 8192) as i32),
        (Limit::SQLITE_LIMIT_SQL_LENGTH, 65536),
        (Limit::SQLITE_LIMIT_COLUMN, 64),
        (Limit::SQLITE_LIMIT_EXPR_DEPTH, 32),
        (Limit::SQLITE_LIMIT_COMPOUND_SELECT, 16),
        (Limit::SQLITE_LIMIT_ATTACHED, 0),
        (Limit::SQLITE_LIMIT_VDBE_OP, 100000),
        (Limit::SQLITE_LIMIT_WORKER_THREADS, 0),
    ] {
        db.set_limit(limit, value).map_err(sqlite_error)?;
    }
    db.execute_batch("PRAGMA query_only=ON; PRAGMA trusted_schema=OFF; PRAGMA temp_store=MEMORY; PRAGMA cache_size=-8192;").map_err(sqlite_error)?;
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    let mut operations = 0u32;
    db.progress_handler(
        1000,
        Some(move || {
            operations += 1;
            operations > 50000 || std::time::Instant::now() > deadline
        }),
    )
    .map_err(sqlite_error)?;
    let ok: String = db
        .query_row("PRAGMA quick_check(1)", [], |r| r.get(0))
        .map_err(sqlite_error)?;
    if ok != "ok" {
        return Err("MBTiles database integrity check failed".into());
    }
    let mut ordinary = BTreeSet::new();
    let mut stmt = db.prepare("PRAGMA main.table_list").map_err(sqlite_error)?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, String>(2)?)))
        .map_err(sqlite_error)?;
    for row in rows {
        let (name, kind) = row.map_err(sqlite_error)?;
        if name == "sqlite_schema" {
            continue;
        }
        if ordinary.len() >= 256 || !["table", "view"].contains(&kind.as_str()) {
            return Err(
                "MBTiles requires ordinary SQLite tables and views without extensions".into(),
            );
        }
        ordinary.insert(name);
    }
    drop(stmt);
    db.authorizer(Some(move |ctx: AuthContext<'_>| match ctx.action {
        AuthAction::Select => Authorization::Allow,
        AuthAction::Read { table_name, .. }
            if ctx.database_name == Some("main") && ordinary.contains(table_name) =>
        {
            Authorization::Allow
        }
        AuthAction::Function { function_name } if ["length", "typeof"].contains(&function_name) => {
            Authorization::Allow
        }
        _ => Authorization::Deny,
    }))
    .map_err(sqlite_error)?;
    Ok(db)
}
fn metadata(db: &Connection) -> Result<serde_json::Value> {
    let stmt = db
        .prepare("SELECT * FROM metadata LIMIT 0")
        .map_err(sqlite_error)?;
    let columns = stmt.column_names();
    if columns.len() != 2 || !columns.contains(&"name") || !columns.contains(&"value") {
        return Err("MBTiles metadata requires name and value columns".into());
    }
    drop(stmt);
    let mut stmt = db
        .prepare("SELECT name, value FROM metadata LIMIT 257")
        .map_err(sqlite_error)?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(sqlite_error)?;
    let mut metadata = BTreeMap::new();
    let mut size = 0usize;
    for row in rows {
        let (key, value) = row.map_err(sqlite_error)?;
        size += key.len() + value.len();
        if !clean(&key, 80)
            || size > format::MAX_METADATA
            || metadata.len() >= 256
            || metadata.insert(key, value).is_some()
        {
            return Err("Invalid, duplicate or excessive MBTiles metadata".into());
        }
    }
    let name = metadata
        .get("name")
        .filter(|n| clean(n, 1024))
        .ok_or("MBTiles metadata requires a name")?;
    let format = metadata
        .get("format")
        .ok_or("MBTiles metadata requires a tile format")?;
    if !["pbf", "png", "jpg"].contains(&format.as_str()) {
        return Err("Supported MBTiles formats are gzip MVT (pbf), PNG and JPEG".into());
    }
    if metadata.get("scheme").is_some_and(|s| s != "tms") {
        return Err("MBTiles must use the standard TMS tile row scheme".into());
    }
    let layers = if format == "pbf" {
        let json: serde_json::Value = serde_json::from_str(
            metadata
                .get("json")
                .ok_or("Vector MBTiles requires JSON layer metadata")?,
        )
        .map_err(|_| "Invalid MBTiles layer JSON")?;
        let layers = json
            .get("vector_layers")
            .and_then(|ls| ls.as_array())
            .filter(|ls| !ls.is_empty() && ls.len() <= 256)
            .ok_or("Vector MBTiles requires vector_layers")?;
        let mut names = BTreeSet::new();
        for l in layers {
            let id = l["id"]
                .as_str()
                .filter(|s| clean(s, 256))
                .ok_or("Invalid MBTiles layer name")?;
            let fields = l["fields"]
                .as_object()
                .filter(|f| f.len() <= 512)
                .ok_or("Invalid MBTiles layer fields")?;
            if !names.insert(id)
                || fields.iter().any(|(key, v)| {
                    !clean(key, 256) || !matches!(v.as_str(), Some("String" | "Number" | "Boolean"))
                })
            {
                return Err("Invalid MBTiles layer fields or duplicate layer".into());
            }
        }
        serde_json::Value::Array(layers.clone())
    } else {
        serde_json::json!([])
    };
    Ok(
        serde_json::json!({"name":name,"attribution":metadata.get("attribution"),"vector_layers":layers,"mbtiles":metadata}),
    )
}
fn image(raw: &[u8], format: &str) -> Result<Image> {
    let (width, height, content_type) = if format == "png" {
        if !raw.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Err("MBTiles PNG signature does not match its format".into());
        }
        let mut decoder = png::Decoder::new(Cursor::new(raw));
        decoder.set_limits(png::Limits {
            bytes: 16 * 1024 * 1024,
        });
        let mut reader = decoder
            .read_info()
            .map_err(|_| "Invalid MBTiles PNG tile")?;
        let info = reader.info();
        let (w, h) = (info.width, info.height);
        if w == 0 || w > 1024 || h != w || info.animation_control.is_some() {
            return Err(
                "MBTiles images must be square tiles of 1–1024 pixels without animation".into(),
            );
        }
        let mut pixels = vec![
            0;
            reader
                .output_buffer_size()
                .filter(|n| *n <= 16 * 1024 * 1024)
                .ok_or("MBTiles PNG decode exceeds its memory limit")?
        ];
        reader
            .next_frame(&mut pixels)
            .map_err(|_| "Corrupt MBTiles PNG pixels")?;
        reader.finish().map_err(|_| "Incomplete MBTiles PNG tile")?;
        (w, h, "image/png")
    } else {
        if !raw.starts_with(&[255, 216]) {
            return Err("MBTiles JPEG signature does not match its format".into());
        }
        let options = zune_core::options::DecoderOptions::default()
            .set_max_width(1024)
            .set_max_height(1024)
            .set_strict_mode(true)
            .jpeg_set_out_colorspace(zune_core::colorspace::ColorSpace::RGB);
        let mut decoder = zune_jpeg::JpegDecoder::new_with_options(
            zune_core::bytestream::ZCursor::new(raw),
            options,
        );
        decoder
            .decode_headers()
            .map_err(|_| "Invalid MBTiles JPEG tile")?;
        let (w, h) = decoder
            .dimensions()
            .ok_or("Missing MBTiles JPEG dimensions")?;
        if w == 0 || w > 1024 || h != w {
            return Err("MBTiles images must be square tiles of 1–1024 pixels".into());
        }
        if decoder
            .decode()
            .map_err(|_| "Corrupt MBTiles JPEG pixels")?
            .len()
            != w * h * 3
        {
            return Err("Invalid MBTiles JPEG pixels".into());
        }
        (w as u32, h as u32, "image/jpeg")
    };
    Ok(Image {
        width,
        height,
        content_type: content_type.into(),
    })
}
fn payload(raw: &[u8], format: &str) -> Result<(Vec<u8>, Vec<mvt::Layer>, Option<Image>)> {
    if raw.is_empty() || raw.len() > format::MAX_TILE {
        return Err("MBTiles tile exceeds 8 MiB or is empty".into());
    }
    if format == "pbf" {
        if !raw.starts_with(&[31, 139]) {
            return Err("MBTiles pbf tiles require gzip-compressed MVT data".into());
        }
        let decoded = format::decompress(raw, 2, format::MAX_TILE)?;
        let layers = mvt::inspect(&decoded)?;
        Ok((decoded, layers, None))
    } else {
        Ok((raw.to_vec(), Vec::new(), Some(image(raw, format)?)))
    }
}
fn union(tiles: &[TileReceipt]) -> [f64; 4] {
    tiles.iter().fold([180., 90., -180., -90.], |a, t| {
        let b = tile_bounds(&t.coordinate);
        [
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[2].max(b[2]),
            a[3].max(b[3]),
        ]
    })
}
pub(super) fn inspect(name: &str, bytes: &[u8]) -> Result<Package> {
    local::file_name(name)?;
    let db = connection(bytes)?;
    let metadata = metadata(&db)?;
    let rows = &metadata["mbtiles"];
    let format = rows["format"].as_str().unwrap();
    let mut stmt = db
        .prepare("SELECT zoom_level, tile_column, tile_row, tile_data FROM tiles LIMIT 513")
        .map_err(sqlite_error)?;
    let rows_iter = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, u8>(0)?,
                r.get::<_, u32>(1)?,
                r.get::<_, u32>(2)?,
                r.get::<_, Vec<u8>>(3)?,
            ))
        })
        .map_err(sqlite_error)?;
    let mut tiles = Vec::new();
    let mut coords = BTreeSet::new();
    let mut decoded_bytes = 0;
    for row in rows_iter {
        if tiles.len() >= MAX_TILES {
            return Err("Local MBTiles archive exceeds 512 tiles".into());
        }
        let (z, x, tms, raw) = row.map_err(sqlite_error)?;
        if z > 24 || tms >= 1u32 << z || x >= 1u32 << z {
            return Err("Invalid MBTiles TMS coordinate".into());
        }
        let coordinate = format::Coordinate {
            z,
            x,
            y: (1u32 << z) - 1 - tms,
        };
        if !coords.insert(format::tile_id(z, x, coordinate.y)?) {
            return Err("Duplicate MBTiles coordinate".into());
        }
        let (decoded, layers, image) = payload(&raw, format)?;
        decoded_bytes += image
            .as_ref()
            .map_or(decoded.len(), |i| i.width as usize * i.height as usize * 4);
        if decoded_bytes > 256 * 1024 * 1024 {
            return Err("Decoded MBTiles tiles exceed 256 MiB".into());
        }
        if layers.iter().any(|l| {
            !metadata["vector_layers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["id"].as_str() == Some(&l.name))
        }) {
            return Err("MBTiles contains an undeclared vector layer".into());
        }
        tiles.push(TileReceipt {
            coordinate,
            source_offset: None,
            package_offset: None,
            image,
            bytes: raw.len(),
            sha256: hash(&raw),
            layers,
        });
    }
    if tiles.is_empty() {
        return Err("MBTiles archive contains no tiles".into());
    }
    tiles.sort_by_key(|t| format::tile_id(t.coordinate.z, t.coordinate.x, t.coordinate.y).unwrap());
    let min_zoom = tiles.iter().map(|t| t.coordinate.z).min().unwrap();
    let max_zoom = tiles.iter().map(|t| t.coordinate.z).max().unwrap();
    for (key, expected) in [("minzoom", min_zoom), ("maxzoom", max_zoom)] {
        if let Some(value) = rows[key].as_str() {
            if value.parse::<u8>().ok() != Some(expected) {
                return Err("MBTiles zoom metadata does not match its tiles".into());
            }
        }
    }
    let coverage = union(&tiles);
    let bounds = if let Some(value) = rows["bounds"].as_str() {
        let values = value
            .split(',')
            .map(|s| s.trim().parse::<f64>())
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| "Invalid MBTiles bounds")?;
        values
            .try_into()
            .map_err(|_| "MBTiles bounds require west, south, east and north")?
    } else {
        coverage
    };
    features::bounds(bounds)?;
    if bounds[1] < -85.0511287798066 || bounds[3] > 85.0511287798066 {
        return Err("MBTiles bounds must use Web Mercator latitude limits".into());
    }
    let tile_size = tiles[0].image.as_ref().map(|i| i.width);
    if tiles
        .iter()
        .any(|t| t.image.as_ref().map(|i| i.width) != tile_size)
    {
        return Err("MBTiles images have inconsistent tile dimensions".into());
    }
    let descriptor = Descriptor {
        format: format.into(),
        scheme: "tms".into(),
        min_zoom,
        max_zoom,
        bounds,
        addressed_tiles: tiles.len(),
        tile_size,
    };
    let source_name = metadata["name"]
        .as_str()
        .unwrap()
        .chars()
        .take(80)
        .collect::<String>()
        .trim()
        .to_owned();
    let mut source = Source {
        id: Uuid::new_v4().to_string(),
        name: source_name.clone(),
        url: String::new(),
        etag: String::new(),
        total_bytes: bytes.len() as u64,
        header: None,
        mbtiles: Some(descriptor),
        metadata,
        connected_at: now(),
        ranges: Vec::new(),
        discovery_sha256: String::new(),
        local: Some(LocalSource {
            file_name: name.into(),
            sha256: hash(bytes),
        }),
    };
    source.discovery_sha256 = fingerprint(&source)?;
    let p = Package {
        id: Uuid::new_v4().to_string(),
        name: source_name,
        requested_bounds: bounds,
        tile_coverage_bounds: coverage,
        min_zoom,
        max_zoom,
        bytes: bytes.len(),
        sha256: hash(bytes),
        source,
        created_at: now(),
        ranges: Vec::new(),
        tiles,
        absent: Vec::new(),
        selection: "imported-archive".into(),
    };
    valid_package(&p)?;
    Ok(p)
}
pub(super) fn valid_source(s: &Source) -> Result<()> {
    let h = s.mbtiles.as_ref().ok_or("Missing MBTiles descriptor")?;
    let local = s
        .local
        .as_ref()
        .ok_or("MBTiles source must be a local file")?;
    local::file_name(&local.file_name)?;
    features::bounds(h.bounds)?;
    if !local.file_name.to_ascii_lowercase().ends_with(".mbtiles")
        || s.header.is_some()
        || !s.url.is_empty()
        || !s.etag.is_empty()
        || !s.ranges.is_empty()
        || !uuid(&s.id)
        || !clean(&s.name, 80)
        || !digest(&local.sha256)
        || s.total_bytes < 100
        || s.total_bytes > MAX_PACKAGE as u64
        || chrono::DateTime::parse_from_rfc3339(&s.connected_at).is_err()
        || !["pbf", "png", "jpg"].contains(&h.format.as_str())
        || h.scheme != "tms"
        || h.min_zoom > h.max_zoom
        || h.max_zoom > 24
        || h.addressed_tiles == 0
        || h.addressed_tiles > MAX_TILES
        || h.bounds[1] < -85.0511287798066
        || h.bounds[3] > 85.0511287798066
        || !s.metadata["mbtiles"].is_object()
        || !s.metadata["vector_layers"].is_array()
        || fingerprint(s)? != s.discovery_sha256
        || (h.format == "pbf") != h.tile_size.is_none()
        || h.tile_size.is_some_and(|n| n == 0 || n > 1024)
    {
        return Err("Invalid saved MBTiles source".into());
    }
    Ok(())
}
pub(super) fn valid_package(p: &Package) -> Result<()> {
    valid_source(&p.source)?;
    let h = p.source.mbtiles.as_ref().unwrap();
    features::bounds(p.tile_coverage_bounds)?;
    if !uuid(&p.id)
        || !clean(&p.name, 120)
        || p.bytes as u64 != p.source.total_bytes
        || p.sha256 != p.source.local.as_ref().unwrap().sha256
        || !digest(&p.sha256)
        || p.selection != "imported-archive"
        || p.min_zoom != h.min_zoom
        || p.max_zoom != h.max_zoom
        || p.requested_bounds != h.bounds
        || p.tiles.len() != h.addressed_tiles
        || !p.ranges.is_empty()
        || !p.absent.is_empty()
        || chrono::DateTime::parse_from_rfc3339(&p.created_at).is_err()
    {
        return Err("Invalid saved MBTiles package".into());
    }
    let mut ids = BTreeSet::new();
    for t in &p.tiles {
        if !ids.insert(format::tile_id(
            t.coordinate.z,
            t.coordinate.x,
            t.coordinate.y,
        )?) || t.coordinate.z < p.min_zoom
            || t.coordinate.z > p.max_zoom
            || t.bytes == 0
            || t.bytes > format::MAX_TILE
            || !digest(&t.sha256)
            || t.source_offset.is_some()
            || t.package_offset.is_some()
        {
            return Err("Invalid saved MBTiles tile receipt".into());
        }
        if h.format == "pbf" {
            if t.image.is_some() {
                return Err("Invalid MBTiles vector tile receipt".into());
            }
        } else if !t.layers.is_empty()
            || t.image.as_ref().is_none_or(|i| {
                Some(i.width) != h.tile_size
                    || i.height != i.width
                    || i.content_type
                        != if h.format == "png" {
                            "image/png"
                        } else {
                            "image/jpeg"
                        }
            })
        {
            return Err("Invalid MBTiles image receipt".into());
        }
    }
    if union(&p.tiles) != p.tile_coverage_bounds {
        return Err("MBTiles coverage receipt changed".into());
    }
    Ok(())
}
pub(super) fn verify(p: &Package, bytes: &[u8]) -> Result<()> {
    let checked = inspect(&p.source.local.as_ref().unwrap().file_name, bytes)?;
    if checked.source.mbtiles != p.source.mbtiles
        || checked.source.metadata != p.source.metadata
        || checked.tiles != p.tiles
        || checked.tile_coverage_bounds != p.tile_coverage_bounds
    {
        return Err("MBTiles contents do not match the saved inventory".into());
    }
    Ok(())
}
pub(super) fn read(p: &Package, bytes: &[u8], t: &TileReceipt) -> Result<Tile> {
    let db = connection(bytes)?;
    let c = &t.coordinate;
    let row = (1u32 << c.z) - 1 - c.y;
    let raw:Vec<u8>=db.query_row("SELECT tile_data FROM tiles WHERE zoom_level=?1 AND tile_column=?2 AND tile_row=?3 LIMIT 1",rusqlite::params![c.z,c.x,row],|r|r.get(0)).map_err(sqlite_error)?;
    if raw.len() != t.bytes || hash(&raw) != t.sha256 {
        return Err("Local MBTiles tile checksum changed".into());
    }
    let (decoded, layers, image) = payload(&raw, &p.source.mbtiles.as_ref().unwrap().format)?;
    if layers != t.layers || image != t.image {
        return Err("MBTiles tile receipt changed".into());
    }
    Ok(Tile {
        coordinate: c.clone(),
        data_base64: Some(STANDARD.encode(&decoded)),
        sha256: Some(hash(&decoded)),
        layers,
        content_type: image.map(|i| i.content_type),
    })
}
pub(super) fn export_note() -> &'static [u8] {
    b"GeoD Global local MBTiles import\n\nThe standalone original SQLite database is preserved unchanged, including its TMS rows and original metadata. source.json records the file name, SHA-256 and tile inventory in map XYZ coordinates. No original workstation path is added. This is an original-container export, not a PMTiles conversion, polygon clip or full-resolution vector dataset. Vector tiles retain quantized, potentially simplified, buffered and repeated features. Raster tiles are rendered map images, not scientific imagery bands. GeoD previews vectors using a local style; source styles, sprites and fonts are not included. Retain attribution and check dataset reuse terms. The MBTiles container format does not grant data rights.\n"
}

#[cfg(test)]
mod tests;
