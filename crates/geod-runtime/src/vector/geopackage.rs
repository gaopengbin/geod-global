//! Read a complete local GeoPackage without executing its triggers or extensions.
//! Original SQLite bytes remain the source; GeoJSON is a versioned conversion.
use super::*;
use base64::Engine;
use rusqlite::{
    hooks::{AuthAction, AuthContext, Authorization},
    limits::Limit,
    types::ValueRef,
    Connection,
};
use std::{
    collections::BTreeSet,
    io::Cursor,
    time::{Duration, Instant},
};
mod crs;
#[cfg(test)]
mod tests;
mod wkb;

pub const CONVERSION: u32 = 1;
const MAX_LAYERS: usize = 32;
const MAX_FIELDS: usize = 128;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Field {
    pub name: String,
    pub field_type: String,
    pub nullable: bool,
    pub primary_key: bool,
    pub json_encoding: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Layer {
    pub table: String,
    pub identifier: Option<String>,
    pub description: String,
    pub geometry_column: String,
    pub geometry_type: String,
    pub srs_id: i32,
    pub organization: String,
    pub organization_coordsys_id: i32,
    pub definition: String,
    pub definition_12_063: Option<String>,
    pub z: u8,
    pub m: u8,
    pub feature_count: usize,
    pub fields: Vec<Field>,
    pub coordinate_operation: String,
    pub coordinate_definition: String,
    pub coordinate_operation_id: Option<u32>,
    pub coordinate_operation_direction: String,
    pub coordinate_accuracy_meters: Option<f64>,
    pub coordinate_operation_area: Option<[f64; 4]>,
    pub coordinates_outside_operation_area: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Content {
    pub table: String,
    pub data_type: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Provenance {
    pub conversion: u32,
    pub user_version: u32,
    pub layers: Vec<Layer>,
    pub other_contents: Vec<Content>,
    pub horizontal_crs: String,
    pub vertical_conversion: String,
    pub measure_encoding: String,
}
fn clean(s: &str, max: usize) -> bool {
    !s.is_empty() && s.chars().count() <= max && !s.chars().any(char::is_control)
}
fn quoted(s: &str) -> Result<String> {
    if !clean(s, 240) {
        return Err("Invalid GeoPackage table or field name".into());
    }
    Ok(format!("\"{}\"", s.replace('"', "\"\"")))
}
fn db_error(_: rusqlite::Error) -> String {
    "Invalid or unsupported GeoPackage database; check its schema, integrity and query limits"
        .into()
}
fn connection(bytes: &[u8]) -> Result<Connection> {
    if bytes.len() < 100 || bytes.len() > MAX_BYTES || !bytes.starts_with(b"SQLite format 3\0") {
        return Err("Choose a standalone GeoPackage database, up to 20 MiB".into());
    }
    if bytes[18] != 1 || bytes[19] != 1 {
        return Err("Checkpoint the GeoPackage into a standalone file before opening it".into());
    }
    let application = u32::from_be_bytes(bytes[68..72].try_into().unwrap());
    if application != 0x47504b47 {
        return Err("The SQLite file is not a supported GeoPackage".into());
    }
    let mut db = Connection::open_in_memory().map_err(db_error)?;
    db.deserialize_read_exact("main", Cursor::new(bytes), bytes.len(), true)
        .map_err(db_error)?;
    for (limit, value) in [
        (Limit::SQLITE_LIMIT_LENGTH, MAX_BYTES as i32),
        (Limit::SQLITE_LIMIT_SQL_LENGTH, 65536),
        (Limit::SQLITE_LIMIT_COLUMN, MAX_FIELDS as i32),
        (Limit::SQLITE_LIMIT_EXPR_DEPTH, 32),
        (Limit::SQLITE_LIMIT_COMPOUND_SELECT, 16),
        (Limit::SQLITE_LIMIT_ATTACHED, 0),
        (Limit::SQLITE_LIMIT_VDBE_OP, 100000),
        (Limit::SQLITE_LIMIT_WORKER_THREADS, 0),
    ] {
        db.set_limit(limit, value).map_err(db_error)?;
    }
    db.execute_batch("PRAGMA query_only=ON; PRAGMA trusted_schema=OFF; PRAGMA temp_store=MEMORY; PRAGMA cache_size=-8192;").map_err(db_error)?;
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut calls = 0;
    db.progress_handler(
        1000,
        Some(move || {
            calls += 1;
            calls > 50000 || Instant::now() > deadline
        }),
    )
    .map_err(db_error)?;
    // A GeoPackage may have a standard R-tree index. Its virtual/shadow tables
    // are retained in the original file, but never queried by this reader.
    let mut ordinary = BTreeSet::new();
    let mut count = 0;
    let mut stmt = db.prepare("PRAGMA main.table_list").map_err(db_error)?;
    for row in stmt
        .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, String>(2)?)))
        .map_err(db_error)?
    {
        let (name, kind) = row.map_err(db_error)?;
        count += 1;
        if count > 512 {
            return Err("GeoPackage has too many SQLite objects".into());
        }
        if kind == "table" {
            ordinary.insert(name);
        }
    }
    drop(stmt);
    for table in [
        "gpkg_spatial_ref_sys",
        "gpkg_contents",
        "gpkg_geometry_columns",
    ] {
        if !ordinary.contains(table) {
            return Err("GeoPackage requires its core feature metadata tables".into());
        }
    }
    let check: String = db
        .query_row("PRAGMA quick_check(1)", [], |r| r.get(0))
        .map_err(db_error)?;
    if check != "ok" {
        return Err("GeoPackage database integrity check failed".into());
    }
    db.authorizer(Some(move |ctx: AuthContext<'_>| match ctx.action {
        AuthAction::Select => Authorization::Allow,
        AuthAction::Read { table_name, .. }
            if ctx.database_name == Some("main") && ordinary.contains(table_name) =>
        {
            Authorization::Allow
        }
        AuthAction::Pragma {
            pragma_name,
            pragma_value,
        } if pragma_name == "table_xinfo" && pragma_value.is_some_and(|t| ordinary.contains(t)) => {
            Authorization::Allow
        }
        _ => Authorization::Deny,
    }))
    .map_err(db_error)?;
    Ok(db)
}
fn fields(db: &Connection, table: &str) -> Result<Vec<Field>> {
    let mut stmt = db
        .prepare(&format!("PRAGMA main.table_xinfo({})", quoted(table)?))
        .map_err(db_error)?;
    let mut out = vec![];
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i32>(3)?,
                r.get::<_, i32>(5)?,
                r.get::<_, i32>(6)?,
            ))
        })
        .map_err(db_error)?;
    for row in rows {
        let (name, kind, not_null, pk, hidden) = row.map_err(db_error)?;
        if hidden != 0
            || !clean(&name, 240)
            || kind.len() > 128
            || out.len() >= MAX_FIELDS
            || pk > 1
            || ![0, 1].contains(&not_null)
        {
            return Err("Unsupported GeoPackage field or generated column".into());
        }
        let upper = kind.to_uppercase();
        let base = upper.split('(').next().unwrap().trim();
        let encoding = match base {
            "BOOLEAN" => "boolean",
            "TINYINT" | "SMALLINT" | "MEDIUMINT" | "INT" | "INTEGER" => "number-or-decimal-string",
            "FLOAT" | "DOUBLE" | "REAL" => "number",
            "TEXT" | "DATE" | "DATETIME" => "string",
            "BLOB" => "base64",
            other if wkb::kind(other).is_some() || other == "GEOMETRY" => "geometry",
            _ => return Err("Unsupported GeoPackage field data type".into()),
        };
        out.push(Field {
            name,
            field_type: kind,
            nullable: not_null == 0 && pk == 0,
            primary_key: pk == 1,
            json_encoding: encoding.into(),
        });
    }
    if out.is_empty() {
        return Err("GeoPackage feature table has no fields".into());
    }
    Ok(out)
}
fn safe_integer(n: i64) -> Value {
    if (-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&n) {
        json!(n)
    } else {
        json!(n.to_string())
    }
}
fn property(value: ValueRef<'_>, field: &Field) -> Result<Value> {
    let v = match (field.json_encoding.as_str(), value) {
        (_, ValueRef::Null) if field.nullable => Value::Null,
        ("boolean", ValueRef::Integer(n)) if n == 0 || n == 1 => json!(n == 1),
        ("number-or-decimal-string", ValueRef::Integer(n)) => safe_integer(n),
        ("number", ValueRef::Real(n)) if n.is_finite() => json!(n),
        ("number", ValueRef::Integer(n))
            if (-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&n) =>
        {
            json!(n)
        }
        ("string", ValueRef::Text(s)) => {
            json!(std::str::from_utf8(s).map_err(|_| "Invalid GeoPackage UTF-8 text")?)
        }
        ("base64", ValueRef::Blob(b)) => json!(base64::engine::general_purpose::STANDARD.encode(b)),
        _ => return Err("GeoPackage field value does not match its recorded type".into()),
    };
    Ok(v)
}
pub fn validate(p: &Provenance, features: usize) -> Result<()> {
    if p.conversion != CONVERSION
        || !(10200..=10499).contains(&p.user_version)
        || p.layers.is_empty()
        || p.layers.len() > MAX_LAYERS
        || p.other_contents.len() > MAX_LAYERS
        || p.horizontal_crs != "EPSG:4326"
        || p.vertical_conversion != "source-z-retained; no-vertical-transformation"
        || p.measure_encoding != "geodMeasures; source-values; NaN-as-null"
    {
        return Err("Invalid saved GeoPackage source metadata".into());
    }
    let mut names = BTreeSet::new();
    let mut count = 0;
    for l in &p.layers {
        quoted(&l.table)?;
        quoted(&l.geometry_column)?;
        if !names.insert(&l.table)
            || l.description.len() > 65536
            || l.identifier.as_ref().is_some_and(|s| s.len() > 4096)
            || l.definition.len() > 65536
            || l.definition.is_empty()
            || l.definition_12_063
                .as_ref()
                .is_some_and(|s| s.len() > 65536)
            || !clean(&l.organization, 80)
            || l.z > 2
            || l.m > 2
            || l.fields.is_empty()
            || l.fields.len() > MAX_FIELDS
            || l.feature_count > MAX_FEATURES
            || l.coordinate_operation.len() > 4096
            || l.coordinate_operation.is_empty()
            || l.coordinate_definition.is_empty()
            || l.coordinate_definition.len() > 65792
            || !["forward", "reverse"].contains(&l.coordinate_operation_direction.as_str())
            || l.coordinate_operation_id == Some(0)
            || l.coordinate_accuracy_meters
                .is_some_and(|n| !n.is_finite() || n < 0.)
            || l.coordinates_outside_operation_area > MAX_COORDINATES
            || l.coordinate_operation_area.is_some_and(|b| {
                b.iter().any(|n| !n.is_finite())
                    || b[0] < -180.
                    || b[0] > 180.
                    || b[2] < -180.
                    || b[2] > 180.
                    || b[1] < -90.
                    || b[3] > 90.
                    || b[1] > b[3]
            })
            || wkb::kind(&l.geometry_type).is_none() && l.geometry_type != "GEOMETRY"
        {
            return Err("Invalid saved GeoPackage layer metadata".into());
        }
        let mut fields = BTreeSet::new();
        let mut keys = 0;
        let mut geometry = 0;
        for f in &l.fields {
            quoted(&f.name)?;
            if !fields.insert(&f.name)
                || f.field_type.len() > 128
                || ![
                    "boolean",
                    "number-or-decimal-string",
                    "number",
                    "string",
                    "base64",
                    "geometry",
                ]
                .contains(&f.json_encoding.as_str())
            {
                return Err("Invalid saved GeoPackage fields".into());
            }
            if f.primary_key {
                keys += 1;
                if f.json_encoding != "number-or-decimal-string" {
                    return Err(
                        "GeoPackage feature identifiers must be integer primary keys".into(),
                    );
                }
            }
            if f.name == l.geometry_column {
                geometry += 1;
                if f.json_encoding != "geometry" {
                    return Err(
                        "GeoPackage geometry column does not match its field definition".into(),
                    );
                }
            }
        }
        if keys != 1 || geometry != 1 {
            return Err(
                "GeoPackage requires one integer primary key and one geometry column per layer"
                    .into(),
            );
        }
        count += l.feature_count;
    }
    for c in &p.other_contents {
        quoted(&c.table)?;
        if !names.insert(&c.table) || !clean(&c.data_type, 80) {
            return Err("Invalid saved GeoPackage contents".into());
        }
    }
    if count != features {
        return Err("GeoPackage layer counts do not match its features".into());
    }
    Ok(())
}
/// Adapt explicitly declared WKT to the XY order used by local vector files.
/// This does not infer a CRS from coordinate values.
pub(crate) fn xy_definition(wkt: &str) -> Result<String> {
    validate_wkt_bounds(wkt)?;
    crs::definition(wkt, "", 0)
}
pub fn decode(bytes: &[u8]) -> Result<(Value, Provenance)> {
    let db = connection(bytes)?;
    let version = u32::from_be_bytes(bytes[60..64].try_into().unwrap());
    if !(10200..=10499).contains(&version) {
        return Err("Supported GeoPackage versions are 1.2 through 1.4".into());
    }
    let mut contents_stmt=db.prepare("SELECT table_name, data_type, identifier, description, srs_id FROM gpkg_contents ORDER BY table_name LIMIT 65").map_err(db_error)?;
    let contents = contents_stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                r.get::<_, Option<i32>>(4)?,
            ))
        })
        .map_err(db_error)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(db_error)?;
    if contents.len() > 2 * MAX_LAYERS {
        return Err("GeoPackage has too many content layers".into());
    }
    let expected: BTreeSet<_> = contents
        .iter()
        .filter(|c| c.1 == "features")
        .map(|c| c.0.clone())
        .collect();
    let mut stmt = db
        .prepare("SELECT table_name FROM gpkg_geometry_columns LIMIT 33")
        .map_err(db_error)?;
    let recorded = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(db_error)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(db_error)?;
    if recorded.len() != expected.len()
        || recorded.iter().collect::<BTreeSet<_>>().len() != recorded.len()
        || recorded.into_iter().collect::<BTreeSet<_>>() != expected
    {
        return Err("GeoPackage geometry metadata does not match its feature contents".into());
    }
    drop(stmt);
    let mut layers = vec![];
    let mut other = vec![];
    let mut features = vec![];
    let mut converted_size = 0;
    let mut coordinates = 0;
    let mut names = BTreeSet::new();
    for (table, data_type, identifier, description, contents_srs) in contents {
        quoted(&table)?;
        if !names.insert(table.clone()) {
            return Err("GeoPackage contains duplicate layer names".into());
        }
        if data_type != "features" {
            other.push(Content { table, data_type });
            continue;
        }
        if layers.len() >= MAX_LAYERS {
            return Err("GeoPackage exceeds 32 feature layers".into());
        }
        let mut stmt=db.prepare("SELECT column_name, geometry_type_name, srs_id, z, m FROM gpkg_geometry_columns WHERE table_name=?1 LIMIT 2").map_err(db_error)?;
        let mut rows = stmt.query([&table]).map_err(db_error)?;
        let r = rows
            .next()
            .map_err(db_error)?
            .ok_or("GeoPackage layer has no geometry metadata")?;
        let (column, kind, srs, z, m) = (
            r.get::<_, String>(0).map_err(db_error)?,
            r.get::<_, String>(1).map_err(db_error)?,
            r.get::<_, i32>(2).map_err(db_error)?,
            r.get::<_, u8>(3).map_err(db_error)?,
            r.get::<_, u8>(4).map_err(db_error)?,
        );
        if rows.next().map_err(db_error)?.is_some() || contents_srs != Some(srs) || z > 2 || m > 2 {
            return Err("GeoPackage layer coordinate metadata is inconsistent".into());
        }
        drop(rows);
        drop(stmt);
        let srs_fields = fields_metadata(&db, "gpkg_spatial_ref_sys")?;
        let def2 = if srs_fields.contains("definition_12_063") {
            "definition_12_063"
        } else {
            "NULL"
        };
        let mut stmt=db.prepare(&format!("SELECT organization, organization_coordsys_id, definition, {def2} FROM gpkg_spatial_ref_sys WHERE srs_id=?1 LIMIT 2")).map_err(db_error)?;
        let mut rows = stmt.query([srs]).map_err(db_error)?;
        let r = rows
            .next()
            .map_err(db_error)?
            .ok_or("GeoPackage layer has no spatial reference definition")?;
        let (organization, epsg, definition, definition_12_063) = (
            r.get::<_, String>(0).map_err(db_error)?,
            r.get::<_, i32>(1).map_err(db_error)?,
            r.get::<_, String>(2).map_err(db_error)?,
            r.get::<_, Option<String>>(3).map_err(db_error)?,
        );
        if rows.next().map_err(db_error)?.is_some() {
            return Err("GeoPackage has duplicate spatial reference definitions".into());
        }
        drop(rows);
        drop(stmt);
        let wkt = definition_12_063
            .as_deref()
            .filter(|s| !s.eq_ignore_ascii_case("undefined"))
            .unwrap_or(&definition);
        if wkt.is_empty() || wkt.len() > 65536 {
            return Err(
                "GeoPackage requires a bounded, defined coordinate reference system".into(),
            );
        }
        validate_wkt_bounds(wkt)?;
        let adapted = crs::definition(wkt, &organization, epsg)?;
        let parsed = proj_wkt::parse_crs(&adapted).map_err(|_| {
            "Unsupported or inconsistent GeoPackage coordinate reference definition"
        })?;
        if organization.eq_ignore_ascii_case("EPSG")
            && parsed.epsg() != 0
            && i64::from(parsed.epsg()) != i64::from(epsg)
        {
            return Err("GeoPackage EPSG identifier disagrees with its WKT definition".into());
        }
        // Preserve both SRS declarations, including an authority omitted from
        // the WKT. Reparse the combination so the parser checks its embedded
        // registry against the actual units/projection/ellipsoid. Never
        // replace an unparsed definition with an EPSG code alone.
        let coordinate_definition =
            if organization.eq_ignore_ascii_case("EPSG") && epsg > 0 && parsed.epsg() == 0 {
                let original = adapted.trim();
                let end = original
                    .chars()
                    .last()
                    .ok_or("Invalid GeoPackage coordinate definition")?;
                if end != ']' && end != ')' {
                    return Err("Invalid GeoPackage coordinate definition".into());
                }
                format!(
                    "{},AUTHORITY[\"EPSG\",\"{}\"]{}",
                    &original[..original.len() - 1],
                    epsg,
                    end
                )
            } else {
                adapted
            };
        proj_wkt::parse_crs(&coordinate_definition)
            .map_err(|_| "GeoPackage EPSG identifier disagrees with its WKT definition")?;
        let transform=proj_wkt::transform_from_crs_strings_horizontal(&coordinate_definition,"EPSG:4326").map_err(|_|"GeoPackage coordinate conversion is unavailable; required grids are not downloaded automatically")?;
        if transform.selected_operation().approximate {
            return Err("GeoPackage requires a supported coordinate operation; approximate datum shifts are not applied".into());
        }
        let operation = transform.selected_operation().name.clone();
        let selected_operation = transform.selected_operation();
        let mut field_list = fields(&db, &table)?;
        if let Some(f) = field_list
            .iter_mut()
            .find(|f| f.name == column && f.json_encoding == "base64")
        {
            f.json_encoding = "geometry".into();
        }
        let mut layer = Layer {
            table: table.clone(),
            identifier,
            description,
            geometry_column: column.clone(),
            geometry_type: kind,
            srs_id: srs,
            organization,
            organization_coordsys_id: epsg,
            definition,
            definition_12_063,
            z,
            m,
            feature_count: 0,
            fields: field_list,
            coordinate_operation: operation,
            coordinate_definition,
            coordinate_operation_id: selected_operation.id.map(|id| id.0),
            coordinate_operation_direction: match selected_operation.direction {
                proj_core::OperationStepDirection::Forward => "forward",
                proj_core::OperationStepDirection::Reverse => "reverse",
            }
            .into(),
            coordinate_accuracy_meters: selected_operation.accuracy.map(|a| a.meters),
            coordinate_operation_area: selected_operation
                .area_of_use
                .as_ref()
                .map(|a| [a.west, a.south, a.east, a.north]),
            coordinates_outside_operation_area: 0,
        };
        validate(
            &Provenance {
                conversion: CONVERSION,
                user_version: version,
                layers: vec![layer.clone()],
                other_contents: vec![],
                horizontal_crs: "EPSG:4326".into(),
                vertical_conversion: "source-z-retained; no-vertical-transformation".into(),
                measure_encoding: "geodMeasures; source-values; NaN-as-null".into(),
            },
            0,
        )?;
        let key = layer.fields.iter().find(|f| f.primary_key).unwrap();
        let geom = layer.fields.iter().position(|f| f.name == column).unwrap();
        let query = format!(
            "SELECT {} FROM {} ORDER BY {} LIMIT {}",
            layer
                .fields
                .iter()
                .map(|f| quoted(&f.name))
                .collect::<Result<Vec<_>>>()?
                .join(","),
            quoted(&table)?,
            quoted(&key.name)?,
            MAX_FEATURES + 1 - features.len()
        );
        let mut stmt = db.prepare(&query).map_err(db_error)?;
        let mut rows = stmt.query([]).map_err(db_error)?;
        let mut last = None;
        while let Some(r) = rows.next().map_err(db_error)? {
            if features.len() >= MAX_FEATURES {
                return Err("Vector file exceeds 50,000 features".into());
            }
            let mut properties = serde_json::Map::new();
            let mut identity = None;
            for (i, f) in layer.fields.iter().enumerate() {
                if i == geom {
                    continue;
                }
                let raw = r.get_ref(i).map_err(db_error)?;
                if f.primary_key {
                    if let ValueRef::Integer(n) = raw {
                        if last.is_some_and(|old| old >= n) {
                            return Err(
                                "GeoPackage has duplicate or invalid feature identifiers".into()
                            );
                        }
                        last = Some(n);
                        identity = Some(safe_integer(n));
                    } else {
                        return Err("GeoPackage feature identifiers must be integers".into());
                    }
                }
                properties.insert(f.name.clone(), property(raw, f)?);
            }
            let (geometry, measures) = match r.get_ref(geom).map_err(db_error)? {
                ValueRef::Null if layer.fields[geom].nullable => (Value::Null, None),
                ValueRef::Blob(raw) => wkb::decode(raw, &mut layer, &transform, &mut coordinates)?,
                _ => {
                    return Err(
                        "GeoPackage geometry must be a binary geometry or allowed NULL".into(),
                    )
                }
            };
            let mut feature = json!({"type":"Feature","id":identity.unwrap(),"properties":properties,"geometry":geometry,"geodLayer":table});
            if let Some(m) = measures {
                feature["geodMeasures"] = m;
            }
            converted_size += serde_json::to_vec(&feature).map_err(io_error)?.len();
            if converted_size > MAX_BYTES {
                return Err("Converted GeoPackage features exceed 20 MiB".into());
            }
            features.push(feature);
            layer.feature_count += 1;
        }
        layers.push(layer);
    }
    let provenance = Provenance {
        conversion: CONVERSION,
        user_version: version,
        layers,
        other_contents: other,
        horizontal_crs: "EPSG:4326".into(),
        vertical_conversion: "source-z-retained; no-vertical-transformation".into(),
        measure_encoding: "geodMeasures; source-values; NaN-as-null".into(),
    };
    validate(&provenance, features.len())?;
    let data = json!({"type":"FeatureCollection","features":features,"geodGeoPackage":provenance});
    if serde_json::to_vec(&data).map_err(io_error)?.len() > MAX_BYTES {
        return Err("Converted GeoPackage features exceed 20 MiB".into());
    }
    Ok((data, provenance))
}
fn fields_metadata(db: &Connection, table: &str) -> Result<BTreeSet<String>> {
    let stmt = db
        .prepare(&format!("SELECT * FROM {} LIMIT 0", quoted(table)?))
        .map_err(db_error)?;
    Ok(stmt.column_names().iter().map(|s| s.to_string()).collect())
}
fn validate_wkt_bounds(wkt: &str) -> Result<()> {
    let mut depth = 0usize;
    let mut quoted = false;
    let mut chars = wkt.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '"' {
            if quoted && chars.peek() == Some(&'"') {
                chars.next();
            } else {
                quoted = !quoted;
            }
        } else if !quoted {
            match c {
                '[' | '(' => {
                    depth += 1;
                    if depth > 32 {
                        return Err("GeoPackage coordinate definition has excessive nesting".into());
                    }
                }
                ']' | ')' => {
                    depth = depth
                        .checked_sub(1)
                        .ok_or("Invalid GeoPackage coordinate definition")?;
                }
                _ => {}
            }
        }
    }
    if quoted || depth != 0 {
        return Err("Invalid GeoPackage coordinate definition".into());
    }
    Ok(())
}
