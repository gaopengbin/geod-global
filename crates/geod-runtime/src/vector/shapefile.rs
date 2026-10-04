//! Bounded Shapefile bundles. Original members and DBF record positions survive
//! conversion; coordinates are never used to guess a missing PRJ or encoding.
use super::*;
use std::collections::BTreeSet;
mod archive;
mod dbf;
mod geometry;
#[cfg(test)]
mod tests;
pub use archive::sidecar_bundle;

pub const CONVERSION: u32 = 1;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Member {
    pub name: String,
    pub bytes: usize,
    pub sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Field {
    pub name: String,
    pub field_type: String,
    pub width: u8,
    pub decimals: u8,
    pub json_encoding: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Layer {
    pub table: String,
    pub shape_type: u32,
    pub feature_count: usize,
    pub deleted_count: usize,
    pub null_geometry_count: usize,
    pub fields: Vec<Field>,
    pub encoding: String,
    pub encoding_source: String,
    pub language_driver_id: u8,
    pub cpg: Option<String>,
    pub definition: String,
    pub coordinate_definition: String,
    pub coordinate_operation: String,
    pub coordinate_operation_id: Option<u32>,
    pub coordinate_operation_direction: String,
    pub coordinate_accuracy_meters: Option<f64>,
    pub coordinate_operation_area: Option<[f64; 4]>,
    pub coordinates_outside_operation_area: usize,
    pub coordinates_clamped_to_bounds: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Provenance {
    pub conversion: u32,
    /// ZIP is byte-identical; sidecars is a deterministic bundle of exact members.
    pub container: String,
    pub files: Vec<Member>,
    pub layers: Vec<Layer>,
    pub horizontal_crs: String,
    pub vertical_conversion: String,
    pub measure_encoding: String,
    pub numeric_encoding: String,
    pub deleted_records: String,
    pub polygon_rings: String,
}
fn policy(container: &str) -> Provenance {
    Provenance {
        conversion: CONVERSION,
        container: container.into(),
        files: vec![],
        layers: vec![],
        horizontal_crs: "EPSG:4326".into(),
        vertical_conversion: "source-z-retained; no-vertical-transformation".into(),
        measure_encoding: "geodMeasures; source-values; below-minus-1e38-as-null".into(),
        numeric_encoding: "DBF N/F as exact trimmed decimal strings".into(),
        deleted_records: "retained-with-geodDeleted; excluded-from-map".into(),
        polygon_rings: "source-XY orientation-and-containment; no-topology-repair".into(),
    }
}
pub fn validate(p: &Provenance, count: usize) -> Result<()> {
    let mut expected = policy(&p.container);
    expected.files = p.files.clone();
    expected.layers = p.layers.clone();
    if *p != expected
        || !["zip", "sidecars"].contains(&p.container.as_str())
        || p.files.is_empty()
        || p.files.len() > 256
        || p.layers.is_empty()
        || p.layers.len() > 32
    {
        return Err("Invalid saved Shapefile conversion policy".into());
    }
    let mut names = BTreeSet::new();
    let mut size = 0usize;
    for f in &p.files {
        archive::safe_name(&f.name)?;
        if !names.insert(f.name.to_lowercase())
            || f.sha256.len() != 64
            || !f
                .sha256
                .bytes()
                .all(|n| n.is_ascii_hexdigit() && !n.is_ascii_uppercase())
        {
            return Err("Invalid saved Shapefile member receipt".into());
        }
        size = size
            .checked_add(f.bytes)
            .ok_or("Shapefile member size overflow")?;
    }
    if size > MAX_BYTES {
        return Err("Shapefile members exceed 20 MiB".into());
    }
    let mut layers = BTreeSet::new();
    let mut records = 0usize;
    for l in &p.layers {
        archive::safe_name(&format!("{}.shp", l.table))?;
        if !layers.insert(l.table.to_lowercase())
            || !geometry::supported(l.shape_type)
            || l.feature_count > MAX_FEATURES
            || l.deleted_count > l.feature_count
            || l.null_geometry_count > l.feature_count
            || l.fields.len() > 128
            || l.definition.is_empty()
            || l.definition.len() > 65536
            || l.coordinate_definition.is_empty()
            || l.coordinate_definition.len() > 65536
            || l.coordinate_operation.is_empty()
            || l.coordinate_operation.len() > 512
            || !["forward", "reverse"].contains(&l.coordinate_operation_direction.as_str())
            || l.coordinate_accuracy_meters
                .is_some_and(|n| !n.is_finite() || n < 0.)
            || l.coordinate_operation_area.is_some_and(|b| {
                b.iter().any(|n| !n.is_finite())
                    || b[0] < -180.
                    || b[2] > 180.
                    || b[1] < -90.
                    || b[3] > 90.
                    || b[1] > b[3]
            })
            || l.coordinates_outside_operation_area > MAX_COORDINATES
            || l.coordinates_clamped_to_bounds > MAX_COORDINATES
            || !["cpg", "ldid", "ascii-only"].contains(&l.encoding_source.as_str())
            || l.encoding.is_empty()
            || l.encoding.len() > 80
            || l.cpg.as_ref().is_some_and(|s| s.is_empty() || s.len() > 80)
        {
            return Err("Invalid saved Shapefile layer metadata".into());
        }
        let mut fields = BTreeSet::new();
        for f in &l.fields {
            if f.name.is_empty()
                || f.name.len() > 64
                || f.name.chars().any(char::is_control)
                || !fields.insert(f.name.to_lowercase())
                || f.width == 0
                || !["C", "N", "F", "L", "D"].contains(&f.field_type.as_str())
                || f.json_encoding != dbf::json_encoding(f.field_type.as_bytes()[0])?
            {
                return Err("Invalid saved Shapefile field metadata".into());
            }
        }
        for suffix in ["shp", "shx", "dbf", "prj"] {
            if !names.contains(&format!("{}.{}", l.table, suffix).to_lowercase()) {
                return Err("Shapefile requires SHP, SHX, DBF and a defined PRJ".into());
            }
        }
        records = records
            .checked_add(l.feature_count)
            .ok_or("Shapefile record count overflow")?;
    }
    if records != count || records > MAX_FEATURES {
        return Err("Shapefile layer counts differ from its records".into());
    }
    Ok(())
}
pub fn decode(bytes: &[u8], container: &str) -> Result<(Value, Provenance)> {
    let members = archive::members(bytes)?;
    let mut source = policy(container);
    source.files = members
        .iter()
        .map(|(name, b)| Member {
            name: name.clone(),
            bytes: b.len(),
            sha256: hash(b),
        })
        .collect();
    let mut features = vec![];
    let mut coordinates = 0usize;
    let mut topology_work = 0u64;
    let mut converted_size = 0usize;
    for (name, shp) in members
        .iter()
        .filter(|(n, _)| n.to_lowercase().ends_with(".shp"))
    {
        if source.layers.len() >= 32 {
            return Err("Shapefile bundle exceeds 32 layers".into());
        }
        let table = &name[..name.len() - 4];
        let member = |suffix: &str| -> Result<&[u8]> {
            members
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(&format!("{table}.{suffix}")))
                .map(|(_, b)| b.as_slice())
                .ok_or_else(|| {
                    format!("Shapefile layer {table} is missing its .{suffix} companion")
                })
        };
        let prj = member("prj")?;
        let definition = std::str::from_utf8(prj.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(prj))
            .map_err(|_| "Shapefile PRJ requires UTF-8 WKT")?
            .trim()
            .to_string();
        if definition.is_empty() || definition.len() > 65536 {
            return Err("Shapefile requires a bounded, defined PRJ coordinate system".into());
        }
        let coordinate_definition = geopackage::xy_definition(&definition)
            .map_err(|_| "Unsupported or inconsistent Shapefile PRJ coordinate system")?;
        let transform=proj_wkt::transform_from_crs_strings_horizontal(&coordinate_definition,"EPSG:4326").map_err(|_|"Shapefile coordinate conversion is unavailable; required grids are not downloaded automatically")?;
        if transform.selected_operation().approximate {
            return Err("Shapefile requires a supported coordinate operation; approximate datum shifts are not applied".into());
        }
        let cpg = members
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(&format!("{table}.cpg")))
            .map(|(_, b)| b.as_slice());
        let attrs = dbf::decode(member("dbf")?, cpg)?;
        if attrs.rows.len() + features.len() > MAX_FEATURES {
            return Err("Shapefile bundle exceeds 50,000 records".into());
        }
        let shapes = geometry::decode(
            shp,
            member("shx")?,
            &transform,
            geometry::MAX_TOPOLOGY_WORK - topology_work,
        )
        .map_err(|e| format!("Shapefile layer {table}: {e}"))?;
        topology_work += shapes.topology_work;
        if attrs.rows.len() != shapes.records.len() {
            return Err("Shapefile SHP, SHX and DBF record counts do not match".into());
        }
        coordinates += shapes.coordinates;
        if coordinates > MAX_COORDINATES {
            return Err("Shapefile bundle exceeds 500,000 coordinates".into());
        }
        let op = transform.selected_operation();
        let mut layer = Layer {
            table: table.into(),
            shape_type: shapes.shape_type,
            feature_count: attrs.rows.len(),
            deleted_count: 0,
            null_geometry_count: 0,
            fields: attrs.fields,
            encoding: attrs.encoding,
            encoding_source: attrs.encoding_source,
            language_driver_id: attrs.ldid,
            cpg: attrs.cpg,
            definition,
            coordinate_definition,
            coordinate_operation: op.name.clone(),
            coordinate_operation_id: op.id.map(|i| i.0),
            coordinate_operation_direction: match op.direction {
                proj_core::OperationStepDirection::Forward => "forward",
                proj_core::OperationStepDirection::Reverse => "reverse",
            }
            .into(),
            coordinate_accuracy_meters: op.accuracy.map(|a| a.meters),
            coordinate_operation_area: op
                .area_of_use
                .as_ref()
                .map(|a| [a.west, a.south, a.east, a.north]),
            coordinates_outside_operation_area: shapes.outside,
            coordinates_clamped_to_bounds: shapes.clamped,
        };
        for (index, ((properties, deleted), (geometry, measures))) in
            attrs.rows.into_iter().zip(shapes.records).enumerate()
        {
            layer.deleted_count += usize::from(deleted);
            layer.null_geometry_count += usize::from(geometry.is_null());
            let mut feature = json!({"type":"Feature","id":index,"geodLayer":table,"geometry":geometry,"properties":properties});
            if deleted {
                feature["geodDeleted"] = json!(true);
            }
            if let Some(m) = measures {
                feature["geodMeasures"] = m;
            }
            converted_size += serde_json::to_vec(&feature).map_err(io_error)?.len();
            if converted_size > MAX_BYTES {
                return Err("Converted Shapefile exceeds 20 MiB".into());
            }
            features.push(feature);
        }
        source.layers.push(layer);
    }
    if source.layers.is_empty() {
        return Err("ZIP archive contains no Shapefile layer".into());
    }
    validate(&source, features.len())?;
    let data = json!({"type":"FeatureCollection","features":features,"geodShapefile":source});
    if serde_json::to_vec(&data).map_err(io_error)?.len() > MAX_BYTES {
        return Err("Converted Shapefile exceeds 20 MiB".into());
    }
    Ok((data, source))
}
