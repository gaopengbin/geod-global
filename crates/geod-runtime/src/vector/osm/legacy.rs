//! Full embedded Overpass geometry, including joined multipolygon members.
//! No public Overpass backend is contacted by the application.
use super::*;
use std::collections::BTreeSet;

fn points(v: &Value) -> Result<Vec<Value>> {
    let mut stats = Stats::default();
    let values = v
        .as_array()
        .ok_or("OSM way is missing complete embedded geometry")?;
    if values.len() < 2 {
        return Err("OSM way has insufficient geometry".into());
    }
    values
        .iter()
        .map(|p| {
            let p = json!([p["lon"], p["lat"]]);
            position(&p, &mut stats)?;
            Ok(p)
        })
        .collect()
}
fn rings(mut segments: Vec<Vec<Value>>) -> Result<Vec<Vec<Value>>> {
    let mut result = Vec::new();
    while let Some(mut ring) = segments.pop() {
        while ring.first() != ring.last() {
            let matching = segments
                .iter()
                .enumerate()
                .filter_map(|(i, s)| {
                    if s.first() == ring.last() {
                        Some((i, false))
                    } else if s.last() == ring.last() {
                        Some((i, true))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            if matching.len() != 1 {
                return Err("OSM multipolygon has missing or ambiguous member connections".into());
            }
            let (i, reverse) = matching[0];
            let mut next = segments.swap_remove(i);
            if reverse {
                next.reverse();
            }
            ring.extend(next.into_iter().skip(1));
        }
        if ring.len() < 4 {
            return Err("OSM multipolygon has a degenerate ring".into());
        }
        result.push(ring);
    }
    Ok(result)
}
fn contains(ring: &[Value], point: &Value, budget: &mut usize) -> Result<bool> {
    *budget += ring.len();
    if *budget > 2_000_000 {
        return Err("OSM hole assignment exceeds its geometry complexity limit".into());
    }
    let x = point[0].as_f64().unwrap();
    let y = point[1].as_f64().unwrap();
    let mut inside = false;
    for edge in ring.windows(2) {
        let ax = edge[0][0].as_f64().unwrap();
        let ay = edge[0][1].as_f64().unwrap();
        let bx = edge[1][0].as_f64().unwrap();
        let by = edge[1][1].as_f64().unwrap();
        if (ay > y) != (by > y) && x < (bx - ax) * (y - ay) / (by - ay) + ax {
            inside = !inside;
        }
    }
    Ok(inside)
}
fn multipolygon(element: &Value) -> Result<Value> {
    if element["tags"]["type"] != "multipolygon" && element["tags"]["type"] != "boundary" {
        return Err(
            "Convert non-area OSM relations to GeoJSON with a suitable source tool first".into(),
        );
    }
    let members = element["members"]
        .as_array()
        .ok_or("OSM relation has no members")?;
    if members.len() > 1024 {
        return Err("OSM relation exceeds 1,024 members".into());
    }
    let mut outer = Vec::new();
    let mut inner = Vec::new();
    for m in members {
        match (m["type"].as_str(),m["role"].as_str()) {
            (Some("way"),Some("outer"))=>outer.push(points(&m["geometry"])?),
            (Some("way"),Some("inner"))=>inner.push(points(&m["geometry"])?),
            (Some("node"),Some("label"|"admin_centre"))=>{},
            _=>return Err("OSM area relations require complete outer/inner way geometry; unsupported members are not omitted".into())
        }
    }
    let mut polygons = rings(outer)?
        .into_iter()
        .map(|r| vec![r])
        .collect::<Vec<_>>();
    if polygons.is_empty() {
        return Err("OSM multipolygon has no outer ring".into());
    }
    let mut budget = 0;
    for hole in rings(inner)? {
        let mut owners = Vec::new();
        for (i, p) in polygons.iter().enumerate() {
            if contains(&p[0], &hole[0], &mut budget)? {
                owners.push(i);
            }
        }
        if owners.len() != 1 {
            return Err("OSM inner ring does not belong to one outer polygon".into());
        }
        polygons[owners[0]].push(hole);
    }
    Ok(json!({"type":"MultiPolygon","coordinates":polygons}))
}
pub(super) fn convert(raw: &Value) -> Result<Value> {
    if raw
        .get("remark")
        .is_some_and(|v| !v.as_str().is_some_and(str::is_empty))
    {
        return Err("OSM extract reports incomplete or failed processing; partial features are not accepted".into());
    }
    if !raw["generator"]
        .as_str()
        .is_some_and(|s| s.starts_with("Overpass API"))
    {
        return Err("Unsupported OSM JSON generator".into());
    }
    let elements = raw["elements"]
        .as_array()
        .ok_or("OSM extract has no elements array")?;
    if elements.len() > MAX_FEATURES {
        return Err("OSM extract exceeds 50,000 elements".into());
    }
    let mut features = Vec::new();
    let mut ids = BTreeSet::new();
    for e in elements {
        let kind = e["type"].as_str().ok_or("OSM element has no type")?;
        let id = e["id"]
            .as_u64()
            .filter(|v| *v > 0 && *v <= 9_007_199_254_740_991)
            .ok_or("OSM element has an invalid ID")?;
        let identity = format!("{kind}/{id}");
        if !ids.insert(identity.clone()) {
            return Err("OSM extract contains duplicate element identity".into());
        }
        let tags = e.get("tags").cloned().unwrap_or_else(|| json!({}));
        if !tags
            .as_object()
            .is_some_and(|v| v.values().all(Value::is_string))
        {
            return Err("OSM tags must be strings".into());
        }
        let geometry = match kind {
            "node" => json!({"type":"Point","coordinates":[e["lon"],e["lat"]]}),
            "way" => {
                let points = points(&e["geometry"])?;
                let area = tags["area"] == "yes"
                    || (tags["area"] != "no"
                        && tags.get("highway").is_none()
                        && ["building", "landuse", "leisure", "amenity"]
                            .iter()
                            .any(|key| tags.get(key).is_some_and(|v| v != "no")))
                    || (tags["area"] != "no" && tags["natural"] == "water");
                if area {
                    if points.len() < 4 || points.first() != points.last() {
                        return Err("OSM area way is not a complete closed ring".into());
                    }
                    json!({"type":"Polygon","coordinates":[points]})
                } else {
                    json!({"type":"LineString","coordinates":points})
                }
            }
            "relation" => multipolygon(e)?,
            _ => return Err("Unsupported OSM element type".into()),
        };
        features.push(json!({"type":"Feature","id":identity,"properties":{"osm_type":kind,"osm_id":id,"tags":tags},"geometry":geometry}));
    }
    Ok(json!({"type":"FeatureCollection","features":features,
        "attribution":"© OpenStreetMap contributors","license":"https://www.openstreetmap.org/copyright",
        "data_timestamp":raw["osm3s"]["timestamp_osm_base"]}))
}
