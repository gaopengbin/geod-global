//! Source geometries stay native. Models receive a session-scoped, hashed
//! reference rather than thousands of editable polygon coordinates.
use super::*;
use crate::crop::PolygonGeometry;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Query {
    pub provider: String,
    pub candidate_id: String,
    pub lookup_id: Option<String>,
    pub country_code: Option<String>,
    pub admin_level: Option<u8>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Reference {
    id: String,
    sha256: String,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Record {
    id: String,
    session_id: String,
    created_at: String,
    geometry: PolygonGeometry,
    sha256: String,
    name: String,
    provenance: Value,
}
pub(super) async fn read(manager: &JobManager, session: &str, query: Query) -> Result<Value> {
    let (name, geometry, provenance) = match query.provider.as_str() {
        "census" if query.country_code.is_none() && query.admin_level.is_none() => {
            places::boundary(
                manager,
                query
                    .lookup_id
                    .as_deref()
                    .ok_or("Use the actual place lookupId.")?,
                &query.candidate_id,
            )
            .await?
        }
        "natural-earth" | "geoboundaries" if query.lookup_id.is_none() => {
            regions::boundary(
                manager,
                &query.provider,
                &query.candidate_id,
                query
                    .country_code
                    .as_deref()
                    .ok_or("Use the actual candidate countryCode.")?,
                query
                    .admin_level
                    .ok_or("Use the actual candidate adminLevel.")?,
            )
            .await?
        }
        _ => return Err("Use an actual supported boundary candidate reference.".into()),
    };
    save(manager, session, name, geometry, provenance).await
}
pub(super) async fn save(
    manager: &JobManager,
    session: &str,
    name: String,
    geometry: PolygonGeometry,
    provenance: Value,
) -> Result<Value> {
    if !uuid(session) {
        return Err("Invalid Agent conversation ID.".into());
    }
    let bounds = geometry.bounds()?;
    let sha256 = digest(&serde_json::to_vec(&geometry).map_err(io_error)?);
    let id = Uuid::new_v4().to_string();
    let record = Record {
        id: id.clone(),
        session_id: session.into(),
        created_at: now(),
        geometry,
        sha256: sha256.clone(),
        name: name.clone(),
        provenance: provenance.clone(),
    };
    write_record(&manager.inner.root, "boundaries", &id, &record).await?;
    Ok(
        json!({"boundary":{"id":id,"sha256":sha256},"name":name,"bounds":bounds,"provenance":provenance,
        "geometryType":match &record.geometry { PolygonGeometry::Polygon(_)=>"Polygon", PolygonGeometry::MultiPolygon(_)=>"MultiPolygon" },
        "next":"Pass this exact boundary reference to geod_project_plan or geod_clip_plan. Coordinates remain native. This reads a boundary only; no project, download or crop has been created. Preserve source year and precision limitations; a boundary is not imagery coverage proof."}),
    )
}
pub(super) async fn resolve(
    manager: &JobManager,
    session: &str,
    reference: Reference,
) -> Result<PolygonGeometry> {
    let record: Record = read_record(&manager.inner.root, "boundaries", &reference.id).await?;
    let fresh = DateTime::parse_from_rfc3339(&record.created_at).is_ok_and(|d| {
        let age = Utc::now().signed_duration_since(d);
        age >= chrono::Duration::zero() && age < chrono::Duration::minutes(TTL_MINUTES)
    });
    if record.id != reference.id
        || record.session_id != session
        || !fresh
        || record.sha256 != reference.sha256
        || digest(&serde_json::to_vec(&record.geometry).map_err(io_error)?) != reference.sha256
    {
        return Err("Boundary changed, expired or belongs to another conversation. Read its source boundary again.".into());
    }
    record.geometry.bounds()?;
    Ok(record.geometry)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn source_polygon_is_pinned_in_project_review_and_map_without_starting_downloads() {
        let root = tempfile::tempdir().unwrap();
        let manager = JobManager::open(root.path()).await.unwrap();
        let session = Uuid::new_v4().to_string();
        let receipt = super::super::tests::project_search_fixture(&manager, &session).await;
        let geometry = PolygonGeometry::MultiPolygon(vec![
            vec![vec![
                [-123.001, 37.947],
                [-122.999, 37.947],
                [-123.0, 37.949],
                [-123.001, 37.947],
            ]],
            vec![vec![
                [-123.0009, 37.948],
                [-123.0008, 37.948],
                [-123.0008, 37.9481],
                [-123.0009, 37.948],
            ]],
        ]);
        let source = save(
            &manager,
            &session,
            "Source fixture".into(),
            geometry.clone(),
            json!({"fixture":true}),
        )
        .await
        .unwrap();
        let args = json!({"searchId":receipt.id,"itemIds":[receipt.candidates[0].item_id],"name":"Administrative polygon review","boundary":source["boundary"]});
        let review = call(
            manager.clone(),
            &session,
            "geod_project_plan",
            args.clone(),
            None,
        )
        .await
        .unwrap();
        assert_eq!(review["status"], "pending");
        assert_eq!(review["polygon"]["sha256"], source["boundary"]["sha256"]);
        let view = manager
            .agent_plan_map_preview(
                &session,
                review["planId"].as_str().unwrap(),
                review["planHash"].as_str().unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(view["geometry"], serde_json::to_value(geometry).unwrap());
        let mut conflicting = args;
        conflicting["useAttachedPolygon"] = json!(true);
        assert!(call(
            manager.clone(),
            &session,
            "geod_project_plan",
            conflicting,
            None
        )
        .await
        .is_err());
        assert!(manager.list().await.is_empty());
        assert!(manager.inner.projects.lock().await.is_empty());
        manager.shutdown().await.unwrap();
    }
    #[tokio::test]
    async fn source_boundary_pins_geometry_and_rejects_cross_session_hash_and_tampering() {
        let root = tempfile::tempdir().unwrap();
        let manager = JobManager::open(root.path()).await.unwrap();
        let session = Uuid::new_v4().to_string();
        let geometry = PolygonGeometry::Polygon(vec![vec![
            [-74.0, 40.0],
            [-73.0, 40.0],
            [-73.0, 41.0],
            [-74.0, 40.0],
        ]]);
        let result = save(
            &manager,
            &session,
            "Source fixture".into(),
            geometry.clone(),
            json!({"fixture":true}),
        )
        .await
        .unwrap();
        assert!(result.get("coordinates").is_none());
        assert!(manager.list().await.is_empty());
        let reference: Reference = serde_json::from_value(result["boundary"].clone()).unwrap();
        assert_eq!(
            resolve(&manager, &session, reference.clone())
                .await
                .unwrap(),
            geometry
        );
        assert!(
            resolve(&manager, &Uuid::new_v4().to_string(), reference.clone())
                .await
                .is_err()
        );
        let mut changed = reference.clone();
        changed.sha256 = "0".repeat(64);
        assert!(resolve(&manager, &session, changed).await.is_err());
        let mut record: Record = read_record(&manager.inner.root, "boundaries", &reference.id)
            .await
            .unwrap();
        record.geometry =
            PolygonGeometry::Polygon(vec![vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 0.0]]]);
        write_record(&manager.inner.root, "boundaries", &reference.id, &record)
            .await
            .unwrap();
        assert!(resolve(&manager, &session, reference).await.is_err());
        manager.shutdown().await.unwrap();
    }
}
