//! Overpass geometry normalization. Legacy registrations retain their original
//! conversion; new snapshots retain relation structure and resolve dependencies.
use super::*;
use std::collections::BTreeSet;
mod legacy;

pub(super) fn convert(raw: &Value) -> Result<Value> {
    legacy::convert(raw)
}

fn identifier(value: &Value) -> Result<u64> {
    value
        .as_u64()
        .filter(|id| *id > 0 && *id <= 9_007_199_254_740_991)
        .ok_or_else(|| "OSM element or member has an invalid ID".into())
}
fn element_key(element: &Value) -> Result<(String, u64)> {
    let kind = element["type"]
        .as_str()
        .filter(|kind| ["node", "way", "relation"].contains(kind))
        .ok_or("Unsupported OSM element type")?;
    Ok((kind.into(), identifier(&element["id"])?))
}
fn tags(element: &Value) -> Result<Value> {
    let tags = element.get("tags").cloned().unwrap_or_else(|| json!({}));
    if !tags
        .as_object()
        .is_some_and(|tags| tags.values().all(Value::is_string))
    {
        return Err("OSM tags must be strings".into());
    }
    Ok(tags)
}

/// Area inference only applies to closed ways. Explicit area=no always wins;
/// road loops, coastlines and river centerlines remain lines by default.
fn area_tags(tags: &Value) -> bool {
    if tags["area"] == "no" {
        return false;
    }
    if tags["area"] == "yes" {
        return true;
    }
    #[derive(Deserialize)]
    struct Rule {
        key: String,
        polygon: String,
        #[serde(default)]
        values: Vec<String>,
    }
    static RULES: std::sync::OnceLock<Vec<Rule>> = std::sync::OnceLock::new();
    let rules = RULES.get_or_init(|| {
        serde_json::from_str(include_str!("osm/polygon-features.json"))
            .expect("bundled OSM polygon rules must be valid JSON")
    });
    rules.iter().any(|rule| {
        let Some(value) = tags[&rule.key].as_str().filter(|value| *value != "no") else {
            return false;
        };
        match rule.polygon.as_str() {
            "all" => true,
            "whitelist" => rule.values.iter().any(|candidate| candidate == value),
            "blacklist" => !rule.values.iter().any(|candidate| candidate == value),
            _ => false,
        }
    })
}

struct Segment {
    points: Vec<Value>,
    start_node: Option<u64>,
    end_node: Option<u64>,
}
fn same_point(a: &Value, b: &Value) -> bool {
    a[0].as_f64() == b[0].as_f64() && a[1].as_f64() == b[1].as_f64()
}
fn connected(a: &Value, a_id: Option<u64>, b: &Value, b_id: Option<u64>) -> Result<bool> {
    match (a_id, b_id) {
        (Some(a_id), Some(b_id)) => {
            if a_id != b_id {
                return Ok(false);
            }
            if !same_point(a, b) {
                return Err("OSM node identity has inconsistent coordinates".into());
            }
            Ok(true)
        }
        _ => Ok(same_point(a, b)),
    }
}
fn rings(mut segments: Vec<Segment>) -> Result<Vec<Vec<Value>>> {
    let mut result = Vec::new();
    while let Some(mut ring) = segments.pop() {
        while !connected(
            &ring.points[0],
            ring.start_node,
            ring.points.last().unwrap(),
            ring.end_node,
        )? {
            let mut matching = Vec::new();
            for (i, segment) in segments.iter().enumerate() {
                if connected(
                    &segment.points[0],
                    segment.start_node,
                    ring.points.last().unwrap(),
                    ring.end_node,
                )? {
                    matching.push((i, false));
                } else if connected(
                    segment.points.last().unwrap(),
                    segment.end_node,
                    ring.points.last().unwrap(),
                    ring.end_node,
                )? {
                    matching.push((i, true));
                }
            }
            if matching.len() != 1 {
                return Err("OSM multipolygon has missing or ambiguous member connections".into());
            }
            let (index, reverse) = matching[0];
            let mut next = segments.swap_remove(index);
            if reverse {
                next.points.reverse();
                std::mem::swap(&mut next.start_node, &mut next.end_node);
            }
            ring.end_node = next.end_node;
            ring.points.extend(next.points.into_iter().skip(1));
        }
        if ring.points.len() < 4 {
            return Err("OSM multipolygon has a degenerate ring".into());
        }
        result.push(ring.points);
    }
    Ok(result)
}
fn contains(ring: &[Value], point: &Value, budget: &mut usize) -> Result<bool> {
    *budget += ring.len();
    if *budget > 2_000_000 {
        return Err("OSM hole assignment exceeds its geometry complexity limit".into());
    }
    let (x, y) = (point[0].as_f64().unwrap(), point[1].as_f64().unwrap());
    let mut inside = false;
    for edge in ring.windows(2) {
        let (ax, ay) = (edge[0][0].as_f64().unwrap(), edge[0][1].as_f64().unwrap());
        let (bx, by) = (edge[1][0].as_f64().unwrap(), edge[1][1].as_f64().unwrap());
        if (ay > y) != (by > y) && x < (bx - ax) * (y - ay) / (by - ay) + ax {
            inside = !inside;
        }
    }
    Ok(inside)
}

struct Context<'a> {
    elements: BTreeMap<(String, u64), &'a Value>,
    active_relations: BTreeSet<u64>,
    stats: Stats,
    expanded_members: usize,
}
impl Context<'_> {
    /// Validate every source object, including non-rendered relation labels and
    /// recursive dependencies. This pass visits source coordinates/references
    /// once; it does not recursively expand geometry or add output features.
    fn validate_sources(&self, elements: &[Value]) -> Result<()> {
        let mut stats = Stats::default();
        let mut references = 0usize;
        for element in elements {
            match element["type"].as_str().unwrap() {
                "node" => Self::validate_source_point(element, &mut stats)?,
                "way" => self.validate_source_way(element, &mut stats, &mut references)?,
                "relation" => {
                    let members = element["members"]
                        .as_array()
                        .filter(|members| !members.is_empty() && members.len() <= 1024)
                        .ok_or("OSM relation requires 1–1,024 members")?;
                    Self::source_references(&mut references, members.len())?;
                    for member in members {
                        let key = self.member_key(member)?;
                        let dependency = self.elements.get(&key).copied();
                        match key.0.as_str() {
                            "node" => {
                                if member.get("lon").is_some() || member.get("lat").is_some() {
                                    Self::validate_source_point(member, &mut stats)?;
                                    if dependency
                                        .is_some_and(|node| !Self::same_source_point(member, node))
                                    {
                                        return Err("OSM member coordinate differs from its node dependency".into());
                                    }
                                } else if dependency.is_none() {
                                    return Err("OSM relation has a missing node dependency".into());
                                }
                            }
                            "way" => {
                                if let Some(geometry) = member.get("geometry") {
                                    let geometry =
                                        Self::validate_source_points(geometry, &mut stats)?;
                                    if let Some(way) = dependency {
                                        self.validate_member_points(geometry, way)?;
                                    }
                                } else if dependency.is_none() {
                                    return Err("OSM relation has a missing way dependency".into());
                                }
                            }
                            "relation" if dependency.is_none() => {
                                return Err(
                                    "OSM relation has a missing nested relation dependency".into(),
                                );
                            }
                            _ => {}
                        }
                    }
                }
                _ => unreachable!(),
            }
        }
        Ok(())
    }
    fn source_references(total: &mut usize, count: usize) -> Result<()> {
        *total += count;
        if *total > MAX_COORDINATES {
            return Err("OSM source exceeds 500,000 node and member references".into());
        }
        Ok(())
    }
    fn same_source_point(a: &Value, b: &Value) -> bool {
        a["lon"].as_f64() == b["lon"].as_f64() && a["lat"].as_f64() == b["lat"].as_f64()
    }
    fn validate_source_point(element: &Value, stats: &mut Stats) -> Result<()> {
        position(&json!([element["lon"], element["lat"]]), stats)
    }
    fn validate_source_points<'a>(
        geometry: &'a Value,
        stats: &mut Stats,
    ) -> Result<&'a Vec<Value>> {
        let points = geometry
            .as_array()
            .filter(|points| points.len() >= 2)
            .ok_or("OSM way is missing complete embedded geometry")?;
        for point in points {
            Self::validate_source_point(point, stats)?;
        }
        Ok(points)
    }
    fn validate_source_way(
        &self,
        element: &Value,
        stats: &mut Stats,
        references: &mut usize,
    ) -> Result<()> {
        let nodes = element
            .get("nodes")
            .map(|nodes| {
                let nodes = nodes
                    .as_array()
                    .filter(|nodes| nodes.len() >= 2)
                    .ok_or("OSM way has an invalid node list")?;
                Self::source_references(references, nodes.len())?;
                nodes.iter().map(identifier).collect::<Result<Vec<_>>>()
            })
            .transpose()?;
        if let Some(geometry) = element.get("geometry") {
            let points = geometry
                .as_array()
                .filter(|points| points.len() >= 2)
                .ok_or("OSM way is missing complete embedded geometry")?;
            if nodes
                .as_ref()
                .is_some_and(|nodes| nodes.len() != points.len())
            {
                return Err("OSM way geometry does not contain every referenced node".into());
            }
            Self::validate_source_points(geometry, stats)?;
            if let Some(nodes) = &nodes {
                for (id, point) in nodes.iter().zip(points) {
                    if self
                        .elements
                        .get(&("node".into(), *id))
                        .is_some_and(|node| !Self::same_source_point(point, node))
                    {
                        return Err("OSM way geometry differs from its node dependency".into());
                    }
                }
            }
        } else {
            let nodes =
                nodes.ok_or("OSM way is missing complete geometry and node dependencies")?;
            for id in nodes {
                if !self.elements.contains_key(&("node".into(), id)) {
                    return Err("OSM way has a missing node dependency".into());
                }
            }
        }
        Ok(())
    }
    fn validate_member_points(&self, geometry: &[Value], way: &Value) -> Result<()> {
        if let Some(points) = way.get("geometry") {
            let points = points
                .as_array()
                .ok_or("OSM way has invalid embedded geometry")?;
            if points.len() != geometry.len()
                || points
                    .iter()
                    .zip(geometry)
                    .any(|(a, b)| !Self::same_source_point(a, b))
            {
                return Err("OSM member geometry differs from its way dependency".into());
            }
        } else {
            let nodes = way["nodes"]
                .as_array()
                .ok_or("OSM way has no complete node dependencies")?;
            if nodes.len() != geometry.len() {
                return Err("OSM member geometry differs from its way dependency".into());
            }
            for (node, point) in nodes.iter().zip(geometry) {
                let id = identifier(node)?;
                let node = self
                    .elements
                    .get(&("node".into(), id))
                    .ok_or("OSM way has a missing node dependency")?;
                if !Self::same_source_point(node, point) {
                    return Err("OSM member geometry differs from its node dependency".into());
                }
            }
        }
        Ok(())
    }
    fn point(&mut self, element: &Value) -> Result<Value> {
        let point = json!([element["lon"], element["lat"]]);
        position(&point, &mut self.stats)?;
        Ok(point)
    }
    fn way_points(&mut self, element: &Value) -> Result<Vec<Value>> {
        let nodes = element
            .get("nodes")
            .map(|nodes| {
                let nodes = nodes
                    .as_array()
                    .filter(|nodes| nodes.len() >= 2)
                    .ok_or("OSM way has an invalid node list")?;
                nodes.iter().map(identifier).collect::<Result<Vec<_>>>()
            })
            .transpose()?;
        if let Some(geometry) = element.get("geometry") {
            let geometry = geometry
                .as_array()
                .filter(|points| points.len() >= 2)
                .ok_or("OSM way is missing complete embedded geometry")?;
            if nodes
                .as_ref()
                .is_some_and(|nodes| nodes.len() != geometry.len())
            {
                return Err("OSM way geometry does not contain every referenced node".into());
            }
            if let Some(nodes) = &nodes {
                for (id, embedded) in nodes.iter().zip(geometry) {
                    if let Some(node) = self.elements.get(&("node".into(), *id)) {
                        if embedded["lon"].as_f64() != node["lon"].as_f64()
                            || embedded["lat"].as_f64() != node["lat"].as_f64()
                        {
                            return Err("OSM way geometry differs from its node dependency".into());
                        }
                    }
                }
            }
            return geometry.iter().map(|point| self.point(point)).collect();
        }
        let nodes = nodes.ok_or("OSM way is missing complete geometry and node dependencies")?;
        nodes
            .into_iter()
            .map(|id| {
                let node = *self
                    .elements
                    .get(&("node".into(), id))
                    .ok_or("OSM way has a missing node dependency")?;
                self.point(node)
            })
            .collect()
    }
    fn member_key(&self, member: &Value) -> Result<(String, u64)> {
        let kind = member["type"]
            .as_str()
            .filter(|kind| ["node", "way", "relation"].contains(kind))
            .ok_or("Unsupported OSM relation member type")?;
        if !member["role"].is_string() {
            return Err("OSM relation member has no string role".into());
        }
        Ok((kind.into(), identifier(&member["ref"])?))
    }
    fn members<'a>(&mut self, element: &'a Value) -> Result<&'a Vec<Value>> {
        let members = element["members"]
            .as_array()
            .filter(|members| !members.is_empty())
            .ok_or("OSM relation has no members")?;
        if members.len() > 1024 {
            return Err("OSM relation exceeds 1,024 members".into());
        }
        self.expanded_members += members.len();
        if self.expanded_members > MAX_COORDINATES {
            return Err("OSM relations exceed the geometry expansion limit".into());
        }
        for member in members {
            self.member_key(member)?;
        }
        Ok(members)
    }
    fn member_way(&mut self, member: &Value) -> Result<Vec<Value>> {
        let key = self.member_key(member)?;
        if let Some(element) = self.elements.get(&key).copied() {
            let points = self.way_points(element)?;
            if let Some(embedded) = member.get("geometry") {
                let embedded = embedded
                    .as_array()
                    .ok_or("OSM member has invalid embedded geometry")?;
                if embedded.len() != points.len()
                    || embedded.iter().zip(&points).any(|(source, point)| {
                        source["lon"].as_f64() != point[0].as_f64()
                            || source["lat"].as_f64() != point[1].as_f64()
                    })
                {
                    return Err("OSM member geometry differs from its way dependency".into());
                }
            }
            Ok(points)
        } else {
            self.way_points(member)
        }
    }
    fn member_segment(&mut self, member: &Value) -> Result<Segment> {
        let key = self.member_key(member)?;
        let element = self.elements.get(&key).copied().unwrap_or(member);
        let points = self.member_way(member)?;
        let endpoints = element
            .get("nodes")
            .and_then(Value::as_array)
            .map(|nodes| {
                (
                    nodes.first().and_then(Value::as_u64),
                    nodes.last().and_then(Value::as_u64),
                )
            })
            .unwrap_or((None, None));
        Ok(Segment {
            points,
            start_node: endpoints.0,
            end_node: endpoints.1,
        })
    }
    fn multipolygon(&mut self, element: &Value) -> Result<Value> {
        let members = self.members(element)?;
        let mut outer = Vec::new();
        let mut inner = Vec::new();
        for member in members {
            match (member["type"].as_str(), member["role"].as_str()) {
                (Some("way"), Some("outer" | "")) => outer.push(self.member_segment(member)?),
                (Some("way"), Some("inner")) => inner.push(self.member_segment(member)?),
                (Some("node"), Some("label" | "admin_centre")) => {},
                _ => return Err("OSM area relations require outer/inner way geometry; unsupported members are not omitted".into()),
            }
        }
        let mut polygons = rings(outer)?
            .into_iter()
            .map(|ring| vec![ring])
            .collect::<Vec<_>>();
        if polygons.is_empty() {
            return Err("OSM multipolygon has no outer ring".into());
        }
        let mut budget = 0;
        for hole in rings(inner)? {
            let owners = polygons
                .iter()
                .enumerate()
                .map(|(index, polygon)| {
                    contains(&polygon[0], &hole[0], &mut budget).map(|inside| (index, inside))
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .filter_map(|(index, inside)| inside.then_some(index))
                .collect::<Vec<_>>();
            if owners.len() != 1 {
                return Err("OSM inner ring does not belong to one outer polygon".into());
            }
            polygons[owners[0]].push(hole);
        }
        Ok(json!({"type":"MultiPolygon","coordinates":polygons}))
    }
    fn geometry(&mut self, element: &Value, depth: usize) -> Result<Value> {
        if depth > 8 {
            return Err("OSM relation nesting exceeds eight levels".into());
        }
        let (kind, id) = element_key(element)?;
        match kind.as_str() {
            "node" => Ok(json!({"type":"Point","coordinates":self.point(element)?})),
            "way" => {
                let points = self.way_points(element)?;
                let tags = tags(element)?;
                let closed = points.len() >= 4
                    && points.first() == points.last()
                    && element
                        .get("nodes")
                        .is_none_or(|nodes| nodes[0] == nodes[nodes.as_array().unwrap().len() - 1]);
                if tags["area"] == "yes" && !closed {
                    return Err("OSM area way is not a complete closed ring".into());
                }
                if closed && area_tags(&tags) {
                    Ok(json!({"type":"Polygon","coordinates":[points]}))
                } else {
                    Ok(json!({"type":"LineString","coordinates":points}))
                }
            }
            "relation" => {
                if !self.active_relations.insert(id) {
                    return Err("OSM relation dependencies contain a cycle".into());
                }
                let result = self.relation_geometry(element, depth);
                self.active_relations.remove(&id);
                result
            }
            _ => unreachable!(),
        }
    }
    fn relation_geometry(&mut self, element: &Value, depth: usize) -> Result<Value> {
        if ["multipolygon", "boundary"]
            .iter()
            .any(|kind| element["tags"]["type"] == *kind)
        {
            return self.multipolygon(element);
        }
        let members = self.members(element)?;
        let mut geometries = Vec::new();
        for member in members {
            let key = self.member_key(member)?;
            let geometry = match key.0.as_str() {
                "way" => json!({"type":"LineString","coordinates":self.member_way(member)?}),
                "node" => {
                    let node = self.elements.get(&key).copied().unwrap_or(member);
                    if node != member
                        && member.get("lon").is_some()
                        && (member["lon"] != node["lon"] || member["lat"] != node["lat"])
                    {
                        return Err("OSM member coordinate differs from its node dependency".into());
                    }
                    json!({"type":"Point","coordinates":self.point(node)?})
                }
                "relation" => {
                    let relation = *self
                        .elements
                        .get(&key)
                        .ok_or("OSM relation has a missing nested relation dependency")?;
                    self.geometry(relation, depth + 1)?
                }
                _ => unreachable!(),
            };
            geometries.push(geometry);
        }
        Ok(json!({"type":"GeometryCollection","geometries":geometries}))
    }
}

pub(super) fn convert_current(raw: &Value) -> Result<Value> {
    let count = raw["elements"]
        .as_array()
        .ok_or("OSM extract has no elements array")?
        .len();
    convert_current_selected(raw, count)
}

/// Online snapshots put matching objects first, followed by their dependencies.
/// Dependencies resolve complete geometry but do not become extra query results.
pub(super) fn convert_current_selected(raw: &Value, selected_count: usize) -> Result<Value> {
    if raw
        .get("remark")
        .is_some_and(|value| !value.as_str().is_some_and(str::is_empty))
    {
        return Err("OSM extract reports incomplete or failed processing; partial features are not accepted".into());
    }
    if !raw["generator"]
        .as_str()
        .is_some_and(|generator| generator.starts_with("Overpass API"))
    {
        return Err("Unsupported OSM JSON generator".into());
    }
    convert_elements(raw, selected_count)
}

/// Original XML/PBF snapshots use the same geometry rules, without fabricating
/// an Overpass generator or query timestamp.
pub(super) fn convert_elements(raw: &Value, selected_count: usize) -> Result<Value> {
    let elements = raw["elements"]
        .as_array()
        .ok_or("OSM extract has no elements array")?;
    if elements.len() > MAX_COORDINATES
        || selected_count > MAX_FEATURES
        || selected_count > elements.len()
    {
        return Err("OSM extract exceeds the element or feature limit".into());
    }
    let mut context = Context {
        elements: BTreeMap::new(),
        active_relations: BTreeSet::new(),
        stats: Stats::default(),
        expanded_members: 0,
    };
    for element in elements {
        let key = element_key(element)?;
        tags(element)?;
        if context.elements.insert(key, element).is_some() {
            return Err("OSM extract contains duplicate element identity".into());
        }
    }
    context.validate_sources(elements)?;
    let mut features = Vec::new();
    for element in elements.iter().take(selected_count) {
        let (kind, id) = element_key(element)?;
        let geometry = context.geometry(element, 0)?;
        let mut properties = json!({"osm_type":kind,"osm_id":id,"tags":tags(element)?});
        if let Some(nodes) = element.get("nodes") {
            properties["osm_nodes"] = nodes.clone();
        }
        if let Some(members) = element["members"].as_array() {
            properties["osm_members"] = Value::Array(members.iter().map(|member|
                json!({"type":member["type"],"ref":member["ref"],"role":member["role"]})).collect());
        }
        let metadata = [
            "version",
            "timestamp",
            "changeset",
            "uid",
            "user",
            "visible",
        ]
        .iter()
        .filter_map(|key| {
            element
                .get(*key)
                .map(|value| ((*key).to_string(), value.clone()))
        })
        .collect::<serde_json::Map<_, _>>();
        if !metadata.is_empty() {
            properties["osm_metadata"] = Value::Object(metadata);
        }
        features.push(json!({"type":"Feature","id":format!("{kind}/{id}"),"properties":properties,"geometry":geometry}));
    }
    Ok(json!({"type":"FeatureCollection","features":features,
        "attribution":"© OpenStreetMap contributors","license":"https://www.openstreetmap.org/copyright",
        "data_timestamp":raw["osm3s"]["timestamp_osm_base"]}))
}
