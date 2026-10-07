//! ArcGIS polygon rings use clockwise shells and counterclockwise holes.
//! Preserve every position, island and hole; never substitute an envelope.
use crate::{crop::PolygonGeometry, Result};
use serde_json::Value;

fn area(ring: &[[f64; 2]]) -> f64 {
    // Translate near the first position to avoid cancellation on small islands.
    let [x, y] = ring[0];
    ring.windows(2)
        .map(|p| (p[0][0] - x) * (p[1][1] - y) - (p[1][0] - x) * (p[0][1] - y))
        .sum::<f64>()
        / 2.0
}
fn contains(ring: &[[f64; 2]], point: [f64; 2]) -> bool {
    let mut inside = false;
    for edge in ring.windows(2) {
        let [a, b] = [edge[0], edge[1]];
        if (a[1] > point[1]) != (b[1] > point[1])
            && point[0] < (b[0] - a[0]) * (point[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
    }
    inside
}
pub(super) fn geometry(value: &Value) -> Result<PolygonGeometry> {
    if value.get("curveRings").is_some() {
        return Err("Unsupported curved boundary.".into());
    }
    let rings: Vec<Vec<[f64; 2]>> =
        serde_json::from_value(value["rings"].clone()).map_err(|_| "Invalid city boundary.")?;
    // Validate before topology work, and keep native crop complexity limits.
    PolygonGeometry::Polygon(rings.clone()).bounds()?;
    if rings.len() > 1000 {
        return Err("City boundary has too many rings.".into());
    }
    let mut polygons = Vec::new();
    let mut holes = Vec::new();
    for ring in rings {
        let signed = area(&ring);
        if signed < 0.0 {
            polygons.push(vec![ring]);
        } else if signed > 0.0 {
            holes.push(ring);
        } else {
            return Err("City boundary has a degenerate ring.".into());
        }
    }
    for hole in holes {
        let index = polygons
            .iter()
            .enumerate()
            .filter(|(_, p)| contains(&p[0], hole[0]))
            .min_by(|(_, a), (_, b)| area(&a[0]).abs().total_cmp(&area(&b[0]).abs()))
            .map(|(i, _)| i)
            .ok_or("City boundary hole has no enclosing shell.")?;
        polygons[index].push(hole);
    }
    let geometry = if polygons.len() == 1 {
        PolygonGeometry::Polygon(polygons.remove(0))
    } else {
        PolygonGeometry::MultiPolygon(polygons)
    };
    geometry.bounds()?;
    Ok(geometry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn islands_holes_and_nested_islands_are_preserved_without_rewriting_vertices() {
        let shell = json!([[0, 0], [0, 10], [10, 10], [10, 0], [0, 0]]);
        let hole = json!([[2, 2], [8, 2], [8, 8], [2, 8], [2, 2]]);
        let island = json!([[3, 3], [3, 4], [4, 4], [4, 3], [3, 3]]);
        let other = json!([[20, 0], [20, 1], [21, 1], [21, 0], [20, 0]]);
        let shape =
            geometry(&json!({"rings":[hole.clone(),other.clone(),island.clone(),shell.clone()]}))
                .unwrap();
        let PolygonGeometry::MultiPolygon(parts) = shape else {
            panic!("islands lost")
        };
        assert_eq!(parts.len(), 3);
        let with_hole = parts.iter().find(|p| p.len() == 2).unwrap();
        let expected: Vec<Vec<[f64; 2]>> = serde_json::from_value(json!([shell, hole])).unwrap();
        assert_eq!(*with_hole, expected);
        assert_eq!(
            parts
                .iter()
                .map(|p| p.iter().map(Vec::len).sum::<usize>())
                .sum::<usize>(),
            20
        );
    }
    #[test]
    fn malformed_and_orphan_holes_never_become_polygons() {
        for rings in [
            json!([[[0, 0], [1, 0], [1, 1], [0, 0]]]),
            json!([[[0, 0], [0, 1], [1, 1], [1, 0]]]),
            json!([[[0, 0], [0, 1], [200, 1], [0, 0]]]),
        ] {
            assert!(geometry(&json!({"rings":rings})).is_err());
        }
    }
}
