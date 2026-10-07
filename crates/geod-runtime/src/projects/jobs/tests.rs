//! Association fixtures exercise metadata only, never claim a real download.
use super::*;
use crate::{new_download_job, CreateJobRequest};
use serde_json::json;

fn project() -> Project {
    serde_json::from_value(json!({
        "id":"11111111-1111-4111-8111-111111111111", "name":"Association fixture",
        "bounds":[0,0,1,1], "createdAt":"2025-01-01", "updatedAt":"2025-01-01",
        "scenes":[{"itemId":"SCENE", "date":"2025-01-01", "cloud":null,
            "crs":null, "gridCode":null, "bbox":[0,0,1,1],
            "assets":{"scl":{"href":"https://example.test/a.tif","mediaType":"image/tiff"},
                "product":{"href":"https://example.test/a.zip","mediaType":"application/zip"}}}],
        "stacItems":[{"snapshotId":"snapshot", "assetKey":"band", "title":"STAC",
            "itemId":"SCENE", "collectionId":null, "serviceName":"Fixture", "mediaType":"image/tiff",
            "datetime":null,"startDatetime":null,"endDatetime":null,"bbox":null,
            "href":"https://example.test/stac.tif"}],
        "wcsItems":[{"planId":"plan", "title":"WCS", "coverageId":"coverage",
            "serviceName":"Fixture", "mediaType":"image/tiff", "bounds":[0,0,1,1],
            "href":"https://example.test/wcs.tif"}]
    })).unwrap()
}
fn job(key: &str, href: &str) -> Job {
    new_download_job(CreateJobRequest {
        item_id: "SCENE".into(),
        asset_key: key.into(),
        href: href.into(),
        media_type: "image/tiff".into(),
        title: None,
    })
}

#[test]
fn source_pins_require_exact_address_and_identity_for_every_status() {
    let project = project();
    let statuses = [
        JobStatus::Queued,
        JobStatus::Running,
        JobStatus::Succeeded,
        JobStatus::Failed,
        JobStatus::Cancelled,
        JobStatus::Interrupted,
    ];
    for status in statuses {
        let mut fixed = job("scl", "https://example.test/a.tif");
        fixed.status = status;
        let mut other_address = fixed.clone();
        other_address.id = uuid::Uuid::new_v4().to_string();
        other_address.href.push_str("?changed=1");
        let mut other_item = fixed.clone();
        other_item.id = uuid::Uuid::new_v4().to_string();
        other_item.item_id = "ANOTHER_SCENE".into();
        let mut other_kind = fixed.clone();
        other_kind.id = uuid::Uuid::new_v4().to_string();
        other_kind.kind = "raster_clip".into();
        let ids = related_ids(
            &project,
            &[fixed.clone(), other_address, other_item, other_kind],
        );
        assert_eq!(ids, HashSet::from([fixed.id]));
    }
}

#[test]
fn stac_and_wcs_cannot_match_by_display_item_name_or_address_alone() {
    let project = project();
    let mut stac = job("stac_asset", "https://example.test/stac.tif");
    stac.stac_source = Some(crate::stac::SourcePin {
        snapshot_id: "snapshot".into(),
        asset_key: "band".into(),
    });
    let mut wcs = job("wcs_coverage", "https://example.test/wcs.tif");
    wcs.wcs_source = Some(crate::wcs::SourcePin {
        plan_id: "plan".into(),
    });
    let mut missing_pin = stac.clone();
    missing_pin.id = uuid::Uuid::new_v4().to_string();
    missing_pin.stac_source = None;
    let mut wrong_pin = stac.clone();
    wrong_pin.id = uuid::Uuid::new_v4().to_string();
    wrong_pin.stac_source.as_mut().unwrap().asset_key = "other-band".into();
    let mut wrong_address = wcs.clone();
    wrong_address.id = uuid::Uuid::new_v4().to_string();
    wrong_address.href.push_str("?changed=1");
    let mut wrong_plan = wcs.clone();
    wrong_plan.id = uuid::Uuid::new_v4().to_string();
    wrong_plan.wcs_source.as_mut().unwrap().plan_id = "other-plan".into();
    assert_eq!(
        related_ids(
            &project,
            &[
                missing_pin,
                wrong_pin,
                wrong_address,
                wrong_plan,
                stac.clone(),
                wcs.clone()
            ]
        ),
        HashSet::from([stac.id, wcs.id])
    );
}

#[test]
fn processing_descendants_resolve_out_of_order_and_unrelated_cycles_terminate() {
    let project = project();
    let mut mosaic = job("scl", "project-output");
    mosaic.kind = "raster_mosaic".into();
    mosaic.mosaic = Some(crate::mosaic::MosaicSpec {
        project_id: project.id.clone(),
        asset_key: "scl".into(),
        sources: vec![],
        coverage_sources: vec![],
        vi_selection: None,
    });
    let mut child = job("scl", "child");
    child.kind = "raster_clip".into();
    child.parent_id = Some(mosaic.id.clone());
    let mut grandchild = job("scl", "grandchild");
    grandchild.kind = "raster_clip".into();
    grandchild.recipe = Some(serde_json::from_value(json!({"schemaVersion":"geod-raster-recipe/v1", "name":"fixture",
        "source":{"jobId":child.id,"sha256":"a".repeat(64)},
        "operation":{"type":"clip","crs":"EPSG:4326","bounds":[0,0,1,1]},"output":{"format":"GeoTIFF"}})).unwrap());
    let mut foreign = mosaic.clone();
    foreign.id = uuid::Uuid::new_v4().to_string();
    foreign.mosaic.as_mut().unwrap().project_id = uuid::Uuid::new_v4().to_string();
    let mut cycle_a = job("scl", "a");
    let mut cycle_b = job("scl", "b");
    cycle_a.parent_id = Some(cycle_b.id.clone());
    cycle_b.parent_id = Some(cycle_a.id.clone());
    assert_eq!(
        related_ids(
            &project,
            &[
                grandchild.clone(),
                cycle_a,
                cycle_b,
                child.clone(),
                foreign,
                mosaic.clone()
            ]
        ),
        HashSet::from([mosaic.id, child.id, grandchild.id])
    );
}

#[test]
fn safe_preparation_requires_a_successful_exact_parent_and_matching_checksum() {
    let project = project();
    let mut source = job("product", "https://example.test/a.zip");
    source.status = JobStatus::Succeeded;
    source.sha256 = Some("a".repeat(64));
    let mut prepared = job("scl", &source.href);
    prepared.kind = "raster_prepare".into();
    prepared.parent_id = Some(source.id.clone());
    prepared.safe = Some(crate::safe::SafeSpec {
        source_job_id: source.id.clone(),
        source_sha256: "a".repeat(64),
    });
    assert_eq!(
        related_ids(&project, &[prepared.clone(), source.clone()]),
        HashSet::from([source.id.clone(), prepared.id.clone()])
    );
    for mismatch in ["status", "hash", "address", "parent"] {
        let mut source = source.clone();
        let mut prepared = prepared.clone();
        match mismatch {
            "status" => source.status = JobStatus::Failed,
            "hash" => prepared.safe.as_mut().unwrap().source_sha256 = "b".repeat(64),
            "address" => prepared.href.push_str("?changed=1"),
            _ => prepared.safe.as_mut().unwrap().source_job_id = uuid::Uuid::new_v4().to_string(),
        }
        assert_eq!(
            related_ids(&project, &[prepared, source.clone()]),
            HashSet::from([source.id]),
            "{mismatch}"
        );
    }
}
