//! Acquisition coverage is native geometry evidence, never a bbox/count guess.
use super::*;
use crate::crop::PolygonGeometry;
use geo::{Area, BooleanOps, BoundingRect, LineString, MultiPolygon, Polygon, Validation};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Scope {
    pub bounds: [f64; 4],
    pub geometry: Option<PolygonGeometry>,
    pub scenes: Vec<(String, Option<PolygonGeometry>)>,
}
pub(super) fn polygon(value: &PolygonGeometry) -> Result<MultiPolygon<f64>> {
    value.bounds()?;
    let parts = match value {
        PolygonGeometry::Polygon(rings) => vec![rings],
        PolygonGeometry::MultiPolygon(parts) => parts.iter().collect(),
    };
    let line = |ring: &Vec<[f64; 2]>| {
        LineString::from(ring.iter().map(|p| (p[0], p[1])).collect::<Vec<_>>())
    };
    let shape = MultiPolygon(
        parts
            .iter()
            .map(|rings| Polygon::new(line(&rings[0]), rings[1..].iter().map(line).collect()))
            .collect(),
    );
    if !shape.is_valid() {
        return Err(
            "Coverage needs valid source polygons; do not replace them with bounding boxes.".into(),
        );
    }
    Ok(shape)
}
pub(super) fn read_geometry(item: &Value) -> Option<PolygonGeometry> {
    let geometry = serde_json::from_value::<PolygonGeometry>(item["geometry"].clone()).ok()?;
    polygon(&geometry).ok()?;
    let bbox: [f64; 4] = serde_json::from_value(item["bbox"].clone()).ok()?;
    let extent = geometry.bounds().ok()?;
    if extent[0] < bbox[0] - 1e-6
        || extent[1] < bbox[1] - 1e-6
        || extent[2] > bbox[2] + 1e-6
        || extent[3] > bbox[3] + 1e-6
    {
        return None;
    }
    Some(geometry)
}

#[cfg(test)]
mod tests;
pub(super) fn rectangle(b: [f64; 4]) -> PolygonGeometry {
    PolygonGeometry::Polygon(vec![vec![
        [b[0], b[1]],
        [b[2], b[1]],
        [b[2], b[3]],
        [b[0], b[3]],
        [b[0], b[1]],
    ]])
}
pub(super) fn report(scope: &Scope) -> Result<Value> {
    if !valid_bounds(scope.bounds) || scope.scenes.len() > 200 {
        return Err("Invalid bounded coverage scope.".into());
    }
    let target = scope
        .geometry
        .as_ref()
        .cloned()
        .unwrap_or_else(|| rectangle(scope.bounds));
    if scope.geometry.is_some() {
        let b = target.bounds()?;
        if b[0] < scope.bounds[0] - 1e-6
            || b[1] < scope.bounds[1] - 1e-6
            || b[2] > scope.bounds[2] + 1e-6
            || b[3] > scope.bounds[3] + 1e-6
        {
            return Err(
                "Search and project bounds must include the entire selected polygon.".into(),
            );
        }
    }
    let mut missing = if scope.geometry.is_some() {
        polygon(&target)?
    } else {
        MultiPolygon(vec![geo::Rect::new(
            (scope.bounds[0], scope.bounds[1]),
            (scope.bounds[2], scope.bounds[3]),
        )
        .to_polygon()])
    };
    let total = missing.unsigned_area();
    let mut unknown = Vec::new();
    let mut positions = 0;
    for (id, shape) in &scope.scenes {
        let Some(shape) = shape else {
            unknown.push(id.clone());
            continue;
        };
        let shape = polygon(shape)?;
        positions += shape
            .0
            .iter()
            .map(|p| p.exterior().0.len() + p.interiors().iter().map(|r| r.0.len()).sum::<usize>())
            .sum::<usize>();
        if positions > 100_000 {
            return Err("Coverage geometry limit reached; use a smaller native selection.".into());
        }
        missing = std::panic::catch_unwind(|| missing.difference(&shape))
            .map_err(|_| "Source footprint coverage calculation failed.")?;
    }
    let gap = missing.unsigned_area();
    // Only floating point residue is tolerated, never a visible percentage gap.
    let complete = gap <= total * 1e-10;
    let status = if complete {
        "complete"
    } else if unknown.is_empty() {
        "partial"
    } else {
        "unknown"
    };
    let missing_bounds = if complete {
        Vec::new()
    } else {
        missing
            .0
            .iter()
            .filter_map(|p| {
                p.bounding_rect()
                    .map(|r| [r.min().x, r.min().y, r.max().x, r.max().y])
            })
            .take(8)
            .collect()
    };
    Ok(
        json!({"status":status,"coveredFraction":if complete {1.0} else {(1.0-gap/total).clamp(0.0,1.0)},
        "target":if scope.geometry.is_some(){"polygon"}else{"rectangle"},"basis":"catalog-footprints","areaMethod":"planar-wgs84",
        "scopeSha256":digest(&serde_json::to_vec(&target).map_err(io_error)?),"sceneCount":scope.scenes.len(),"unknownItemIds":unknown,
        "missingBounds":missing_bounds,"missingParts":if complete {0}else{missing.0.len()},
        "note":"Geometric catalog-footprint coverage only. It does not prove valid pixels or cloud-free coverage; those require downloaded raster checks."}),
    )
}
pub(super) fn complete(scope: &Scope) -> Result<()> {
    if report(scope)?["status"] != "complete" {
        return Err("Selected imagery does not cover the requested area. Check geod_scene_coverage, continue the catalog search, or ask the user to revise constraints. No download was approved.".into());
    }
    Ok(())
}
pub(super) fn project_scope(request: &crate::projects::CreateProjectRequest) -> Scope {
    Scope {
        bounds: request.bounds,
        geometry: request.geometry.clone(),
        scenes: request
            .scenes
            .iter()
            .map(|s| (s.item_id.clone(), s.footprint.clone()))
            .collect(),
    }
}
pub(super) fn action_scope(action: &Action) -> Option<Scope> {
    match action {
        Action::Download {
            acquisition,
            query,
            files,
            ..
        } => Some(acquisition.clone().unwrap_or_else(|| {
            Scope {
                bounds: query.bounds,
                geometry: None,
                scenes: files
                    .iter()
                    .map(|f| (f.request.item_id.clone(), None))
                    .collect(),
            }
        })),
        Action::Project { request, .. } => Some(project_scope(request)),
        _ => None,
    }
}
pub(super) async fn selection(
    manager: &JobManager,
    session: &str,
    search_id: &str,
    item_ids: &[String],
    geometry: Option<PolygonGeometry>,
) -> Result<Scope> {
    let receipt = search(manager, session, search_id).await?;
    if item_ids.is_empty()
        || item_ids.len() > MAX_FILES
        || item_ids
            .iter()
            .any(|id| !receipt.candidates.iter().any(|c| &c.item_id == id))
    {
        return Err("Choose scene IDs returned by this native search.".into());
    }
    let scope = Scope {
        bounds: receipt.query.bounds,
        geometry,
        scenes: receipt
            .candidates
            .iter()
            .filter(|c| item_ids.contains(&c.item_id))
            .map(|c| (c.item_id.clone(), c.footprint.clone()))
            .collect(),
    };
    complete(&scope)?;
    Ok(scope)
}
pub(super) async fn search(
    manager: &JobManager,
    session: &str,
    search_id: &str,
) -> Result<SearchReceipt> {
    let receipt: SearchReceipt = read_record(&manager.inner.root, "searches", search_id).await?;
    if receipt.id != search_id
        || receipt.session_id != session
        || DateTime::parse_from_rfc3339(&receipt.retrieved_at).map_or(true, |d| {
            let age = Utc::now().signed_duration_since(d);
            age < chrono::Duration::zero() || age > chrono::Duration::minutes(TTL_MINUTES)
        })
    {
        return Err(
            "Scene search is stale or belongs to another conversation. Search again.".into(),
        );
    }
    receipt.query.validate()?;
    Ok(receipt)
}
pub(super) async fn check(
    manager: &JobManager,
    session: &str,
    args: Value,
    context: Option<MapContext>,
) -> Result<Value> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Args {
        search_id: String,
        item_ids: Option<Vec<String>>,
        boundary: Option<boundary::Reference>,
        #[serde(default)]
        use_attached_polygon: bool,
    }
    let a: Args = serde_json::from_value(args).map_err(|_| "Invalid coverage selection.")?;
    let receipt = search(manager, session, &a.search_id).await?;
    let geometry = source_geometry(
        manager,
        session,
        context,
        a.use_attached_polygon,
        a.boundary,
    )
    .await?;
    let mut candidates = receipt.candidates.iter().collect::<Vec<_>>();
    candidates.sort_by(|a, b| b.date.cmp(&a.date).then(a.item_id.cmp(&b.item_id)));
    let explicit = a.item_ids.is_some();
    if let Some(ids) = &a.item_ids {
        if ids.is_empty()
            || ids.len() > MAX_FILES
            || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
            || ids
                .iter()
                .any(|id| !candidates.iter().any(|c| &c.item_id == id))
        {
            return Err("Choose distinct scenes from this native search.".into());
        }
        candidates.retain(|c| ids.contains(&c.item_id));
    }
    let mut scope = Scope {
        bounds: receipt.query.bounds,
        geometry,
        scenes: Vec::new(),
    };
    let mut fraction = 0.0;
    for candidate in candidates {
        if scope.scenes.len() >= MAX_FILES {
            break;
        }
        let mut trial = scope.clone();
        trial
            .scenes
            .push((candidate.item_id.clone(), candidate.footprint.clone()));
        let result = report(&trial)?;
        let next = result["coveredFraction"].as_f64().unwrap();
        if explicit || next > fraction + 1e-12 {
            scope = trial;
            fraction = next;
        }
        if !explicit && result["status"] == "complete" {
            break;
        }
    }
    let mut coverage = report(&scope)?;
    if !explicit && coverage["status"] != "complete" {
        let unknown = receipt
            .candidates
            .iter()
            .filter(|c| c.footprint.is_none())
            .map(|c| c.item_id.clone())
            .collect::<Vec<_>>();
        if !unknown.is_empty() {
            coverage["status"] = json!("unknown");
            coverage["unknownItemIds"] = json!(unknown);
        }
    }
    Ok(
        json!({"searchId":receipt.id,"coverage":coverage,"recommendedItemIds":scope.scenes.iter().map(|s| &s.0).collect::<Vec<_>>(),
        "moreAvailable":receipt.more_available,"canContinue":receipt.next.is_some(),
        "next":if coverage["status"]=="complete" {"Prepare a review with exactly these scenes and the same polygon. Downloads retain originals; use the project workflow to deliver a polygon-masked mosaic."} else {"Do not prepare or confirm an incomplete download. Use geod_scene_search_more when canContinue is true, then check coverage again. Otherwise search other dates within the user's constraints. Ask through a decision card before relaxing date/cloud/product requirements."}}),
    )
}
