//! Review-only model entry. Network extraction is reachable only by native confirmation.
use super::*;
use crate::features::{FeatureService, QueryRequest};
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Scope {
    request: QueryRequest,
    service_sha256: String,
    service_name: String,
    service_url: String,
    collection_title: String,
    protocol: String,
    name: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Args {
    service_id: String,
    collection_id: String,
    bounds: [f64; 4],
    name: Option<String>,
    page_size: Option<usize>,
    response_format: Option<String>,
    #[serde(default)]
    use_attached_polygon: bool,
}
fn protocol(service: &FeatureService) -> &'static str {
    if service.wfs.is_some() {
        "WFS 2"
    } else if service.arcgis.is_some() {
        "ArcGIS"
    } else if service.overpass.is_some() {
        "Overpass"
    } else {
        "OGC API Features"
    }
}
fn fingerprint(service: &FeatureService) -> Result<String> {
    Ok(digest(&serde_json::to_vec(service).map_err(io_error)?))
}
impl Scope {
    fn validate(&self) -> Result<()> {
        crate::features::bounds(self.request.bounds)?;
        crate::features::public_url(&self.service_url)?;
        if !uuid(&self.request.service_id)
            || self.request.collection_id.is_empty()
            || self.request.collection_id.chars().count() > 160
            || self.request.collection_id.chars().any(char::is_control)
            || !["WFS 2", "ArcGIS", "Overpass", "OGC API Features"]
                .contains(&self.protocol.as_str())
            || self.name.is_empty()
            || self.name.trim() != self.name
            || self.name.chars().count() > 120
            || self.name.chars().any(char::is_control)
            || self.service_sha256.len() != 64
            || !self
                .service_sha256
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            || !(1..=200).contains(&self.request.page_size.unwrap_or(200))
            || self.service_name.chars().count() > 80
            || self.collection_title.chars().count() > 240
        {
            return Err("Vector review scope changed or is invalid".into());
        }
        if let Some(area) = &self.request.area_geometry {
            let b = area.bounds()?;
            let q = self.request.bounds;
            if b[0] < q[0] || b[1] < q[1] || b[2] > q[2] || b[3] > q[3] {
                return Err("Query bounds must include the saved polygon".into());
            }
        }
        if self.protocol == "Overpass" {
            crate::features::overpass::query_bounds(self.request.bounds)?;
            if self.request.page_size.is_some() {
                return Err("Overpass extraction does not use page sizes".into());
            }
        }
        if self.protocol != "WFS 2" && self.request.response_format.is_some() {
            return Err("Response format selection requires WFS 2".into());
        }
        Ok(())
    }
    fn verify_service(&self, service: &FeatureService) -> Result<()> {
        self.validate()?;
        if fingerprint(service)? != self.service_sha256
            || service.id != self.request.service_id
            || service.name != self.service_name
            || service.url != self.service_url
            || protocol(service) != self.protocol
        {
            return Err(
                "Saved vector service changed. Create a new review before extracting".into(),
            );
        }
        let collection = service
            .collections
            .iter()
            .find(|c| c.id == self.request.collection_id)
            .ok_or("Unknown saved feature collection")?;
        if collection.title != self.collection_title {
            return Err("Saved vector collection changed".into());
        }
        if let Some(layer) = &collection.wfs {
            if !layer
                .formats
                .iter()
                .any(|f| Some(f.id.as_str()) == self.request.response_format.as_deref())
            {
                return Err("Choose an advertised WFS response format".into());
            }
        }
        Ok(())
    }
}
impl Plan {
    fn vector_id(&self) -> String {
        let hash = Sha256::digest(format!("{}:{}:vector", self.policy, self.id));
        let mut bytes: [u8; 16] = hash[..16].try_into().unwrap();
        bytes[6] = (bytes[6] & 15) | 64;
        bytes[8] = (bytes[8] & 63) | 128;
        Uuid::from_bytes(bytes).to_string()
    }
    fn vector_approval(&self) -> crate::vector::VectorApproval {
        crate::vector::VectorApproval {
            plan_id: self.id.clone(),
            plan_hash: self.hash.clone(),
            session_id: self.session_id.clone(),
            approved_at: now(),
            policy: self.policy.clone(),
        }
    }
}
pub(super) fn validate(action: &Action) -> Result<()> {
    let Action::Vector { scope } = action else {
        return Err("Not a vector review".into());
    };
    scope.validate()
}
async fn save(manager: &JobManager, session: &str, scope: Scope) -> Result<Value> {
    if !uuid(session) {
        return Err("Invalid Agent conversation ID".into());
    }
    manager.inner.store.lock().await.accepting_jobs()?;
    scope.validate()?;
    let mut plan = Plan {
        id: Uuid::new_v4().to_string(),
        session_id: session.into(),
        policy: POLICY.into(),
        hash: String::new(),
        created_at: now(),
        expires_at: (Utc::now() + chrono::Duration::minutes(TTL_MINUTES)).to_rfc3339(),
        action: Action::Vector { scope },
    };
    plan.hash = plan.effective_hash()?;
    plan.validate(session)?;
    write_record(&manager.inner.root, "plans", &plan.id, &plan).await?;
    status(manager, &plan).await
}
pub(super) async fn prepare(
    manager: &JobManager,
    session: &str,
    args: Args,
    context: Option<MapContext>,
) -> Result<Value> {
    let geometry = attached_geometry(context, args.use_attached_polygon)?;
    let service = manager
        .inner
        .feature_services
        .lock()
        .await
        .get(&args.service_id)
        .cloned()
        .ok_or("Connect a vector service in the app before preparing extraction")?;
    let collection = service
        .collections
        .iter()
        .find(|c| c.id == args.collection_id)
        .ok_or("Choose a real collection from the saved service")?;
    let response_format = if let Some(layer) = &collection.wfs {
        Some(
            args.response_format
                .unwrap_or_else(|| layer.default_format.clone()),
        )
    } else {
        args.response_format
    };
    let scope = Scope {
        request: QueryRequest {
            service_id: service.id.clone(),
            collection_id: collection.id.clone(),
            bounds: args.bounds,
            area_geometry: geometry,
            page_size: if service.overpass.is_some() {
                args.page_size
            } else {
                Some(args.page_size.unwrap_or(200))
            },
            response_format,
        },
        service_sha256: fingerprint(&service)?,
        service_name: service.name.clone(),
        service_url: service.url.clone(),
        collection_title: collection.title.clone(),
        protocol: protocol(&service).into(),
        name: args
            .name
            .unwrap_or_else(|| collection.title.chars().take(110).collect::<String>())
            .trim()
            .into(),
    };
    scope.verify_service(&service)?;
    save(manager, session, scope).await
}
fn validate_result(plan: &Plan, asset: &crate::vector::VectorAsset) -> Result<()> {
    let Action::Vector { scope } = &plan.action else {
        return Err("Not a vector review".into());
    };
    let a = asset
        .agent_approval
        .as_ref()
        .ok_or("Missing vector confirmation")?;
    let date = DateTime::parse_from_rfc3339(&a.approved_at).map_err(io_error)?;
    if asset.id != plan.vector_id()
        || asset.name != scope.name
        || a.plan_id != plan.id
        || a.plan_hash != plan.hash
        || a.session_id != plan.session_id
        || a.policy != plan.policy
        || date < DateTime::parse_from_rfc3339(&plan.created_at).map_err(io_error)?
        || date > DateTime::parse_from_rfc3339(&plan.expires_at).map_err(io_error)?
    {
        return Err("Vector result does not match its confirmed review".into());
    }
    let (collection, bounds, geometry, url, name) = if let Some(s) = &asset.remote_source {
        let matches_protocol = match scope.protocol.as_str() {
            "OGC API Features" => {
                s.wfs.is_none() && s.arcgis.is_none() && s.selection == "bbox-full-features"
            }
            "ArcGIS" => {
                s.arcgis.is_some() && s.wfs.is_none() && s.selection == "bbox-full-features"
            }
            "WFS 2" => {
                s.arcgis.is_none()
                    && s.selection == "wfs-bbox-full-features"
                    && s.wfs.as_ref().is_some_and(|w| {
                        Some(w.format.id.as_str()) == scope.request.response_format.as_deref()
                            && Some(w.page_size) == scope.request.page_size
                    })
            }
            _ => false,
        };
        if !matches_protocol
            || asset.osm_source.is_some()
            || s.collection_title != scope.collection_title
        {
            return Err("Vector result protocol differs from its review".into());
        }
        (
            &s.collection_id,
            s.requested_bounds,
            &s.area_geometry,
            &s.service_url,
            &s.service_name,
        )
    } else if let Some(s) = &asset.osm_source {
        if scope.protocol != "Overpass"
            || s.selection != "overpass-bbox-full-geometry"
            || s.preset_title != scope.collection_title
        {
            return Err("Vector result protocol differs from its review".into());
        }
        (
            &s.preset,
            s.requested_bounds,
            &s.area_geometry,
            &s.service_url,
            &s.service_name,
        )
    } else {
        return Err("Reviewed vector has no original service provenance".into());
    };
    if *collection != scope.request.collection_id
        || bounds != scope.request.bounds
        || *geometry != scope.request.area_geometry
        || *url != scope.service_url
        || *name != scope.service_name
    {
        return Err("Vector result query differs from its review".into());
    }
    Ok(())
}
pub(super) async fn status(manager: &JobManager, plan: &Plan) -> Result<Value> {
    let Action::Vector { scope } = &plan.action else {
        return Err("Not a vector review".into());
    };
    let asset = manager
        .existing_reviewed_vector(&plan.vector_id(), &plan.vector_approval())
        .await?;
    if let Some(asset) = &asset {
        validate_result(plan, asset)?;
    }
    let replacement = revision::replacement(&manager.inner.root, plan).await?;
    let state = if asset.is_some() {
        "submitted"
    } else if replacement.is_some() {
        "superseded"
    } else if plan.expired() {
        "expired"
    } else {
        "pending"
    };
    let polygon = scope.request.area_geometry.as_ref().map(|g| Ok::<_, String>(json!({"bounds":g.bounds()?,"sha256":digest(&serde_json::to_vec(g).map_err(io_error)?)}))).transpose()?;
    Ok(
        json!({"planId":plan.id,"planHash":plan.hash,"kind":"vector","status":state,"source":scope.service_name,"bounds":scope.request.bounds,"boundsCrs":"EPSG:4326","polygon":polygon,
        "replacedBy":replacement.map(|r|r.plan_id),"expiresAt":plan.expires_at,"approvalRequired":true,"expectedBytes":null,"jobs":[],"notes":["The server returns complete matching features; geometry is not clipped to the query area.","Feature count and byte size are known only after extraction."],
        "vectorReview":{"serviceId":scope.request.service_id,"serviceSha256":scope.service_sha256,"collectionId":scope.request.collection_id,"collectionTitle":scope.collection_title,"protocol":scope.protocol,"name":scope.name,"pageSize":scope.request.page_size,"responseFormat":scope.request.response_format,"selection":if scope.protocol=="Overpass"{"overpass-bbox-full-geometry"}else if scope.protocol=="WFS 2"{"wfs-bbox-full-features"}else{"bbox-full-features"},"liveAvailabilityChecked":false,"clipped":false},
        "vector":asset.as_ref().map(|a|json!({"id":a.id,"name":a.name,"format":a.format,"bytes":a.bytes,"featureCount":a.feature_count,"coordinateCount":a.coordinate_count,"sourceSha256":a.source_sha256,"geojsonSha256":a.geojson_sha256,"verified":true}))}),
    )
}
pub(super) async fn commit(manager: &JobManager, plan: &Plan) -> Result<Value> {
    let Action::Vector { scope } = &plan.action else {
        return Err("Not a vector review".into());
    };
    if let Some(asset) = manager
        .recover_reviewed_vector(&plan.vector_id(), &plan.vector_approval(), |asset| {
            validate_result(plan, asset)
        })
        .await?
    {
        validate_result(plan, &asset)?;
        return status(manager, plan).await;
    }
    if plan.expired() {
        return Err("Agent plan expired. Create a new plan before confirming.".into());
    }
    // Ordinary connection edits do not use agent_commits. Hold this guard until
    // the verified file and approval are atomically registered.
    let services = manager.inner.feature_services.lock().await;
    let service = services
        .get(&scope.request.service_id)
        .ok_or("Saved vector service was removed")?;
    scope.verify_service(service)?;
    let approval = plan.vector_approval();
    let snapshot = manager
        .feature_snapshot(scope.request.clone(), service.clone())
        .await?;
    let asset = manager
        .import_feature_snapshot(
            snapshot,
            Some((plan.vector_id(), approval, scope.name.clone())),
        )
        .await?;
    drop(services);
    validate_result(plan, &asset)?;
    status(manager, plan).await
}
pub(super) fn draft(plan: &Plan) -> Result<Value> {
    let Action::Vector { scope } = &plan.action else {
        return Err("Not a vector review".into());
    };
    Ok(
        json!({"planId":plan.id,"planHash":plan.hash,"kind":"vector","parameters":{"kind":"vector","bounds":scope.request.bounds,"name":scope.name,"keepPolygon":scope.request.area_geometry.is_some()},
        "fields":{"name":true,"bounds":true,"polygon":scope.request.area_geometry.is_some()},"boundsCrs":"EPSG:4326"}),
    )
}
pub(super) async fn revise(
    manager: &JobManager,
    session: &str,
    mut scope: Scope,
    bounds: [f64; 4],
    name: String,
    keep_polygon: bool,
) -> Result<Value> {
    if keep_polygon && scope.request.area_geometry.is_none() {
        return Err("The vector review has no saved polygon".into());
    }
    scope.request.bounds = bounds;
    scope.name = name.trim().into();
    if !keep_polygon {
        scope.request.area_geometry = None;
    }
    let services = manager.inner.feature_services.lock().await;
    scope.verify_service(
        services
            .get(&scope.request.service_id)
            .ok_or("Saved vector service was removed")?,
    )?;
    drop(services);
    save(manager, session, scope).await
}
pub(super) fn definition() -> Value {
    json!({"name":"geod_vector_extract_plan","description":"Prepare a native review for one actual collection of an already saved OGC API Features, ArcGIS, WFS 2 or Overpass service. Use geod_feature_services/collections to select real IDs. Pins saved service fingerprint and user-specified WGS84 bounds; useAttachedPolygon keeps the local polygon as provenance, does not clip geometry. No external query or file creation until the user confirms the plan card. Counts/bytes and current availability remain unknown. WFS uses an advertised responseFormat; Overpass uses a saved preset and requires pageSize to be omitted; never arbitrary QL or a default endpoint. Never approve or execute from chat.",
        "inputSchema":{"type":"object","properties":{"serviceId":{"type":"string","pattern":"^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$"},"collectionId":{"type":"string","minLength":1,"maxLength":160},"bounds":{"type":"array","minItems":4,"maxItems":4,"items":{"type":"number"}},"name":{"type":"string","minLength":1,"maxLength":120},"pageSize":{"type":"integer","minimum":1,"maximum":200,"description":"OGC/ArcGIS/WFS only; omit for Overpass. Defaults to 200 for paged protocols."},"responseFormat":{"type":"string","maxLength":160},"useAttachedPolygon":{"type":"boolean","default":false}},"required":["serviceId","collectionId","bounds"],"additionalProperties":false}})
}
