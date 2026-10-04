use super::*;
pub(super) fn kind(name: &str) -> Option<u32> {
    Some(match name {
        "POINT" => 1,
        "LINESTRING" => 2,
        "POLYGON" => 3,
        "MULTIPOINT" => 4,
        "MULTILINESTRING" => 5,
        "MULTIPOLYGON" => 6,
        "GEOMETRYCOLLECTION" => 7,
        _ => return None,
    })
}
fn type_name(id: u32) -> &'static str {
    match id {
        1 => "Point",
        2 => "LineString",
        3 => "Polygon",
        4 => "MultiPoint",
        5 => "MultiLineString",
        6 => "MultiPolygon",
        7 => "GeometryCollection",
        _ => unreachable!(),
    }
}
struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
    source_bounds: [Option<(f64, f64)>; 4],
    coordinates: usize,
    outside_operation_area: usize,
    parts: usize,
    transform: &'a proj_core::Transform,
}
impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N]> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or("Invalid GeoPackage geometry length")?;
        let b = self
            .bytes
            .get(self.offset..end)
            .ok_or("Truncated GeoPackage geometry")?
            .try_into()
            .unwrap();
        self.offset = end;
        Ok(b)
    }
    fn uint(&mut self, little: bool) -> Result<u32> {
        let b = self.take()?;
        Ok(if little {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    }
    fn double(&mut self, little: bool) -> Result<f64> {
        let b = self.take()?;
        Ok(if little {
            f64::from_le_bytes(b)
        } else {
            f64::from_be_bytes(b)
        })
    }
    fn count(&mut self, little: bool) -> Result<usize> {
        let n = self.uint(little)? as usize;
        if n > MAX_COORDINATES || n > self.bytes.len().saturating_sub(self.offset) / 4 {
            return Err("GeoPackage geometry exceeds coordinate or part limits".into());
        }
        Ok(n)
    }
    fn point(
        &mut self,
        little: bool,
        z: bool,
        m: bool,
        empty_allowed: bool,
    ) -> Result<(Value, Value)> {
        let x = self.double(little)?;
        let y = self.double(little)?;
        let height = if z { Some(self.double(little)?) } else { None };
        let measure = if m { Some(self.double(little)?) } else { None };
        if empty_allowed
            && x.is_nan()
            && y.is_nan()
            && height.is_none_or(f64::is_nan)
            && measure.is_none_or(f64::is_nan)
        {
            return Ok((json!([]), json!([])));
        }
        if !x.is_finite()
            || !y.is_finite()
            || height.is_some_and(|h| !h.is_finite())
            || measure.is_some_and(f64::is_infinite)
        {
            return Err("GeoPackage positions require finite XY/Z coordinates and finite or missing measures".into());
        }
        self.coordinates += 1;
        if self.coordinates > MAX_COORDINATES {
            return Err("Vector file exceeds 500,000 coordinates".into());
        }
        for (i, n) in [Some(x), Some(y), height, measure.filter(|m| m.is_finite())]
            .into_iter()
            .enumerate()
        {
            if let Some(n) = n {
                let (lo, hi) = self.source_bounds[i].get_or_insert((n, n));
                *lo = lo.min(n);
                *hi = hi.max(n);
            }
        }
        let (lon, lat) = self
            .transform
            .convert((x, y))
            .map_err(|_| "GeoPackage coordinate conversion failed for a source position")?;
        if !lon.is_finite()
            || !lat.is_finite()
            || !(-180.0..=180.0).contains(&lon)
            || !(-90.0..=90.0).contains(&lat)
        {
            return Err("GeoPackage conversion returned invalid WGS84 coordinates".into());
        }
        if self
            .transform
            .selected_operation()
            .area_of_use
            .as_ref()
            .is_some_and(|a| {
                let longitude_inside = if a.west <= a.east {
                    lon >= a.west && lon <= a.east
                } else {
                    lon >= a.west || lon <= a.east
                };
                !longitude_inside || lat < a.south || lat > a.north
            })
        {
            self.outside_operation_area += 1;
        }
        let mut p = vec![json!(lon), json!(lat)];
        if let Some(h) = height {
            p.push(json!(h));
        }
        Ok((
            Value::Array(p),
            measure.map_or(
                Value::Null,
                |n| if n.is_nan() { Value::Null } else { json!(n) },
            ),
        ))
    }
    fn points(&mut self, little: bool, z: bool, m: bool, ring: bool) -> Result<(Value, Value)> {
        let count = self.count(little)?;
        if count
            > self.bytes.len().saturating_sub(self.offset)
                / (8 * (2 + usize::from(z) + usize::from(m)))
            || count != 0 && count < if ring { 4 } else { 2 }
        {
            return Err("Invalid GeoPackage line or ring length".into());
        }
        let mut points = Vec::with_capacity(count);
        let mut measures = Vec::with_capacity(if m { count } else { 0 });
        for _ in 0..count {
            let (p, v) = self.point(little, z, m, false)?;
            points.push(p);
            if m {
                measures.push(v)
            }
        }
        if ring && !points.is_empty() && points.first() != points.last() {
            return Err("Polygon rings must be closed".into());
        }
        Ok((Value::Array(points), Value::Array(measures)))
    }
    fn geometry(
        &mut self,
        depth: usize,
        expected: Option<(u32, bool, bool)>,
    ) -> Result<(Value, Value, u32, bool, bool)> {
        self.parts += 1;
        if depth > 8 || self.parts > 100000 {
            return Err("GeoPackage geometry has excessive nesting or parts".into());
        }
        let little = match self.take::<1>()?[0] {
            0 => false,
            1 => true,
            _ => return Err("Invalid GeoPackage WKB byte order".into()),
        };
        let code = self.uint(little)?;
        let base = code % 1000;
        let dimension = code / 1000;
        if !(1..=7).contains(&base) || dimension > 3 {
            return Err("Unsupported GeoPackage geometry type; curves and surfaces require a separate adapter".into());
        }
        let z = dimension == 1 || dimension == 3;
        let m = dimension == 2 || dimension == 3;
        if expected.is_some_and(|(kind, ez, em)| kind != 0 && kind != base || ez != z || em != m) {
            return Err("GeoPackage nested geometry type or dimensions are inconsistent".into());
        }
        let (coordinates, measures) = match base {
            1 => self.point(little, z, m, true)?,
            2 => self.points(little, z, m, false)?,
            3 => {
                let n = self.count(little)?;
                let mut rings = Vec::with_capacity(n);
                let mut values = Vec::with_capacity(if m { n } else { 0 });
                for _ in 0..n {
                    let (c, v) = self.points(little, z, m, true)?;
                    if c.as_array().unwrap().is_empty() {
                        return Err("GeoPackage polygon rings cannot be empty".into());
                    }
                    rings.push(c);
                    if m {
                        values.push(v)
                    }
                }
                (Value::Array(rings), Value::Array(values))
            }
            4..=7 => {
                let n = self.count(little)?;
                let mut children = Vec::with_capacity(n);
                let mut values = Vec::with_capacity(if m { n } else { 0 });
                for _ in 0..n {
                    let (g, v, _, _, _) = self.geometry(
                        depth + 1,
                        Some((if base == 7 { 0 } else { base - 3 }, z, m)),
                    )?;
                    children.push(if base == 7 {
                        g
                    } else {
                        g["coordinates"].clone()
                    });
                    if m {
                        values.push(v)
                    }
                }
                (Value::Array(children), Value::Array(values))
            }
            _ => unreachable!(),
        };
        let geometry = if base == 7 {
            json!({"type":"GeometryCollection","geometries":coordinates})
        } else if (4..=6).contains(&base)
            && coordinates
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c.as_array().unwrap().is_empty())
        {
            // GeoJSON positions cannot contain empty tuples. A collection
            // retains every ordered empty member without silently dropping it.
            json!({"type":"GeometryCollection","geodOriginalGeometryType":type_name(base),"geometries":coordinates.as_array().unwrap().iter().map(|c|json!({"type":type_name(base-3),"coordinates":c})).collect::<Vec<_>>()})
        } else {
            json!({"type":type_name(base),"coordinates":coordinates})
        };
        Ok((geometry, measures, base, z, m))
    }
}
pub(super) fn decode(
    raw: &[u8],
    layer: &mut Layer,
    transform: &proj_core::Transform,
    total: &mut usize,
) -> Result<(Value, Option<Value>)> {
    if raw.len() < 13
        || raw.len() > MAX_BYTES
        || raw[..2] != *b"GP"
        || raw[2] != 0
        || raw[3] & 0xe0 != 0
    {
        return Err("Invalid or extended GeoPackage binary geometry header".into());
    }
    let flags = raw[3];
    let little = flags & 1 != 0;
    let envelope = (flags >> 1) & 7;
    let empty = flags & 0x10 != 0;
    let srs = if little {
        i32::from_le_bytes(raw[4..8].try_into().unwrap())
    } else {
        i32::from_be_bytes(raw[4..8].try_into().unwrap())
    };
    if srs != layer.srs_id || envelope > 4 || empty && envelope != 0 {
        return Err("GeoPackage geometry SRS or envelope flags differ from its layer".into());
    }
    let axes: &[usize] = match envelope {
        0 => &[],
        1 => &[0, 1],
        2 => &[0, 1, 2],
        3 => &[0, 1, 3],
        4 => &[0, 1, 2, 3],
        _ => unreachable!(),
    };
    let mut reader = Reader {
        bytes: raw,
        offset: 8,
        source_bounds: [None; 4],
        coordinates: 0,
        outside_operation_area: 0,
        parts: 0,
        transform,
    };
    let mut ranges = vec![];
    for axis in axes {
        ranges.push((*axis, reader.double(little)?, reader.double(little)?));
    }
    let (g, m, geometry_kind, z, has_m) = reader.geometry(0, None)?;
    if reader.offset != raw.len()
        || kind(&layer.geometry_type).is_some_and(|expected| expected != geometry_kind)
        || layer.z == 0 && z
        || layer.z == 1 && !z
        || layer.m == 0 && has_m
        || layer.m == 1 && !has_m
        || empty != (reader.coordinates == 0)
    {
        return Err(
            "GeoPackage geometry content differs from its declared type, dimensions or empty flag"
                .into(),
        );
    }
    for (axis, min, max) in ranges {
        if axis == 2 && !z || axis == 3 && !has_m {
            return Err("GeoPackage envelope dimensions differ from its geometry".into());
        }
        if axis == 3 && min.is_nan() && max.is_nan() && reader.source_bounds[axis].is_none() {
            continue;
        }
        if !min.is_finite() || !max.is_finite() || min > max {
            return Err("Invalid GeoPackage geometry envelope".into());
        }
        if let Some((lo, hi)) = reader.source_bounds[axis] {
            let tolerance = min.abs().max(max.abs()).max(1.) * f64::EPSILON * 16.;
            if lo < min - tolerance || hi > max + tolerance {
                return Err("GeoPackage geometry lies outside its declared envelope".into());
            }
        }
    }
    *total += reader.coordinates;
    layer.coordinates_outside_operation_area += reader.outside_operation_area;
    if *total > MAX_COORDINATES {
        return Err("Vector file exceeds 500,000 coordinates".into());
    }
    Ok((g, has_m.then_some(m)))
}
