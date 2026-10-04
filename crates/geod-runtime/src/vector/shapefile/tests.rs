use super::*;
use std::io::{Cursor, Write};
const FIXTURE: &[u8] = include_bytes!("../../../fixtures/shapefile/independent-pyshp.zip");
fn bundle(entries: BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(vec![]));
    for (name, bytes) in entries {
        zip.start_file(
            name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
        zip.write_all(&bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}
fn changed(name: &str, change: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
    let mut m = archive::members(FIXTURE).unwrap();
    change(m.get_mut(name).unwrap());
    bundle(m)
}
#[test]
fn independent_producer_preserves_all_types_crs_fields_deleted_null_and_measures() {
    assert_eq!(
        hash(FIXTURE),
        "bd7cda6db0f916640b17f5de2d0a198bade8d9da7175836d66d30b1b99665019"
    );
    let (data, p) = decode(FIXTURE, "zip").unwrap();
    assert_eq!(p.layers.len(), 12);
    assert_eq!(data["features"].as_array().unwrap().len(), 14);
    let feature = |name: &str, id: u64| {
        data["features"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["geodLayer"] == name && f["id"] == id)
            .unwrap()
    };
    let merc = feature("type_01", 0);
    let position = merc["geometry"]["coordinates"].as_array().unwrap();
    assert!((position[0].as_f64().unwrap() - 12.).abs() < 1e-10);
    assert!((position[1].as_f64().unwrap() - 48.).abs() < 1e-10);
    assert_eq!(merc["properties"]["large"], "9007199254740993");
    assert_eq!(
        merc["properties"]["precise"],
        "1234567890.123456789012345678"
    );
    assert_eq!(merc["properties"]["name"], "  中文 München");
    assert_eq!(merc["properties"]["date"], "2026-10-03");
    assert_eq!(merc["properties"]["active"], true);
    assert!(merc["properties"]["empty"].is_null());
    assert_eq!(feature("type_21", 0)["properties"]["name"], "  中文属性");
    assert_eq!(feature("type_11", 0)["geometry"]["coordinates"][2], 18.25);
    assert_eq!(feature("type_11", 0)["geodMeasures"], 7.5);
    assert_eq!(feature("type_11", 1)["geodDeleted"], true);
    assert!(feature("type_11", 1)["geodMeasures"].is_null());
    assert!(feature("type_11", 2)["geometry"].is_null());
    for name in ["type_05", "type_15", "type_25"] {
        let f = feature(name, 0);
        assert_eq!(f["geometry"]["type"], "MultiPolygon");
        let polys = f["geometry"]["coordinates"].as_array().unwrap();
        assert_eq!(polys.len(), 2);
        assert_eq!(
            polys
                .iter()
                .map(|p| p.as_array().unwrap().len())
                .sum::<usize>(),
            3
        );
        let points = polys
            .iter()
            .flat_map(|p| p.as_array().unwrap())
            .flat_map(|r| r.as_array().unwrap());
        assert!(points
            .clone()
            .all(|p| (p[0].as_f64().unwrap() - 15.).abs() < 0.04
                && (p[1].as_f64().unwrap() - 42.).abs() < 0.02));
    }
    assert_eq!(
        p.layers
            .iter()
            .find(|l| l.table == "type_11")
            .unwrap()
            .deleted_count,
        1
    );
    assert_eq!(
        p.layers
            .iter()
            .find(|l| l.table == "type_21")
            .unwrap()
            .encoding,
        "GBK"
    );
}
#[test]
fn missing_companions_unknown_prj_and_invalid_encoding_fail_without_guesses() {
    for suffix in ["shx", "dbf", "prj"] {
        let mut m = archive::members(FIXTURE).unwrap();
        m.remove(&format!("type_01.{suffix}"));
        assert!(decode(&bundle(m), "zip").is_err());
    }
    for bad in ["","EPSG:4326","GEOGCS[\"Unknown\",DATUM[\"unregistered\",SPHEROID[\"other\",6371000,0]],PRIMEM[\"Greenwich\",0],UNIT[\"Degree\",0.017453292519943295]]"]{
        assert!(decode(&changed("type_01.prj",|b|*b=bad.as_bytes().to_vec()),"zip").is_err());
    }
    assert!(decode(&changed("type_21.cpg", |b| *b = b"UTF-8".to_vec()), "zip").is_err());
    let mut m = archive::members(FIXTURE).unwrap();
    m.remove("type_21.cpg");
    m.get_mut("type_21.dbf").unwrap()[29] = 0;
    assert!(decode(&bundle(m), "zip")
        .unwrap_err()
        .contains("no supported CPG"));
}
#[test]
fn malformed_index_record_counts_numeric_dates_and_polygon_topology_are_rejected() {
    for bytes in [
        changed("type_01.shx", |b| b[103] = 51),
        changed("type_01.shp", |b| b[107] ^= 1),
        changed("type_01.dbf", |b| b[4] = 2),
        changed("type_01.dbf", |b| b[32 + 11] = b'M'),
        changed("type_05.shp", |b| {
            b[164 + 16..164 + 24].copy_from_slice(&501100f64.to_le_bytes())
        }),
    ] {
        assert!(decode(&bytes, "zip").is_err());
    }
    let invalid = changed("type_01.dbf", |b| {
        let h = u16::from_le_bytes(b[8..10].try_into().unwrap()) as usize;
        b[h + 1 + 48] = b'x';
    });
    assert!(decode(&invalid, "zip").is_err());
}
#[test]
fn unsafe_or_case_ambiguous_members_and_decompression_limits_are_rejected() {
    let mut crc = FIXTURE.to_vec();
    let central = crc.windows(4).position(|b| b == b"PK\x01\x02").unwrap();
    crc[central + 16] ^= 1;
    assert!(archive::members(&crc).unwrap_err().contains("CRC"));
    let mut m = archive::members(FIXTURE).unwrap();
    let v = m.remove("type_01.shp").unwrap();
    m.insert("../type_01.shp".into(), v);
    assert!(decode(&bundle(m), "zip").is_err());
    let mut m = archive::members(FIXTURE).unwrap();
    m.insert("TYPE_01.SHP".into(), m["type_01.shp"].clone());
    assert!(decode(&bundle(m), "zip").is_err());
    let mut m = archive::members(FIXTURE).unwrap();
    m.insert("large.txt".into(), vec![b' '; MAX_BYTES + 1]);
    assert!(archive::members(&bundle(m)).is_err());
}
#[test]
fn polygon_topology_budget_rejects_work_before_unbounded_pairwise_checks() {
    let members = archive::members(FIXTURE).unwrap();
    let transform =
        proj_wkt::transform_from_crs_strings_horizontal("EPSG:32633", "EPSG:4326").unwrap();
    assert!(geometry::decode(
        &members["type_05.shp"],
        &members["type_05.shx"],
        &transform,
        0
    )
    .err()
    .unwrap()
    .contains("work limit"));
}
#[test]
fn missing_optional_measure_block_is_distinct_from_truncated_payload() {
    let mut m = archive::members(FIXTURE).unwrap();
    // MultiPointM permits omission of the complete M range/array block.
    let shp = m.get_mut("type_28.shp").unwrap();
    let removed = 16 + 2 * 8;
    shp.truncate(shp.len() - removed);
    let n = shp.len() as u32 / 2;
    shp[24..28].copy_from_slice(&n.to_be_bytes());
    let len = n - 54;
    shp[104..108].copy_from_slice(&len.to_be_bytes());
    m.get_mut("type_28.shx").unwrap()[104..108].copy_from_slice(&len.to_be_bytes());
    let (data, _) = decode(&bundle(m), "zip").unwrap();
    let f = data["features"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["geodLayer"] == "type_28")
        .unwrap();
    assert_eq!(f["geodMeasures"], json!([null, null]));
    assert!(decode(
        &changed("type_28.shp", |b| {
            b.pop();
        }),
        "zip"
    )
    .is_err());
}
#[tokio::test]
async fn sidecar_reference_managed_copy_original_export_and_restart_are_verified() {
    let temp = tempfile::tempdir().unwrap();
    let folder = temp.path().join("input");
    std::fs::create_dir(&folder).unwrap();
    for (n, b) in archive::members(FIXTURE)
        .unwrap()
        .into_iter()
        .filter(|(n, _)| n.starts_with("type_21."))
    {
        std::fs::write(folder.join(n), b).unwrap();
    }
    let path = folder.join("type_21.shp");
    let root = temp.path().join("runtime");
    let manager = JobManager::open(&root).await.unwrap();
    let reference = manager.open_vector_path(path.clone(), false).await.unwrap();
    let managed = manager.open_vector_path(path.clone(), true).await.unwrap();
    assert_eq!(reference.shapefile.as_ref().unwrap().container, "sidecars");
    assert_eq!(managed.storage_mode, "managed");
    let original = manager.vector_original_bytes(&reference.id).await.unwrap();
    assert_eq!(hash(&original), reference.source_sha256);
    assert_eq!(
        archive::members(&original).unwrap()["type_21.dbf"],
        std::fs::read(folder.join("type_21.dbf")).unwrap()
    );
    assert!(manager
        .export_vector_original_path(&reference.id, folder.join("type_21.prj"))
        .await
        .is_err());
    assert!(manager
        .export_vector_path(&reference.id, folder.join("type_21.dbf"))
        .await
        .is_err());
    let exported = temp.path().join("original.zip");
    manager
        .export_vector_original_path(&managed.id, exported.clone())
        .await
        .unwrap();
    assert_eq!(std::fs::read(exported).unwrap(), original);
    drop(manager);
    std::fs::rename(&folder, temp.path().join("gone")).unwrap();
    let manager = JobManager::open(&root).await.unwrap();
    assert!(manager.inspect_vector(&reference.id).await.is_err());
    let current = manager.inspect_vector(&managed.id).await.unwrap();
    assert_eq!(current.asset, managed);
    assert_eq!(
        current.geojson["features"][0]["properties"]["name"],
        "  中文属性"
    );
    assert_eq!(
        manager.vector_original_bytes(&managed.id).await.unwrap(),
        original
    );
}
