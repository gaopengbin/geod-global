use super::*;
#[test]
fn independent_upstream_raw_zlib_dense_simple_and_locations_controls() {
    for file in [
        "test.osm",
        "test.osm.pbf",
        "test_nozlib.osm.pbf",
        "test_nozlib_nodense.osm.pbf",
        "loc_on_ways.osm.pbf",
        "independent.osm",
        "independent.osm.pbf",
        "seatac-derived.osm",
        "seatac-derived.osm.pbf",
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/osm")
            .join(file);
        let bytes = std::fs::read(path).unwrap();
        let (data, source) = decode(&bytes).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert!(
            source.object_counts.node + source.object_counts.way + source.object_counts.relation
                > 0
        );
        assert_eq!(
            data["features"].as_array().unwrap().len(),
            source.object_counts.node + source.object_counts.way + source.object_counts.relation
        );
    }
    assert!(decode(include_bytes!(
        "../../../fixtures/osm/deleted_nodes.osh.pbf"
    ))
    .unwrap_err()
    .contains("HistoricalInformation"));
}
const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?><osm version="0.6" generator="independent test"><node id="1" lon="0" lat="0" version="2" user="A &amp; B" timestamp="2025-01-01T00:00:00Z"><tag k="name" v="测试 &amp; café"/></node><node id="2" lon="2" lat="1"/><way id="3"><nd ref="1"/><nd ref="2"/><tag k="highway" v="residential"/></way><relation id="4"><member type="way" ref="3" role="forward"/><tag k="type" v="route"/></relation></osm>"#;
#[test]
fn xml_preserves_objects_tags_metadata_and_complete_dependencies() {
    let (geo, source) = decode(XML.as_bytes()).unwrap();
    assert_eq!(
        source.object_counts,
        ObjectCounts {
            node: 2,
            way: 1,
            relation: 1
        }
    );
    assert!(source.dataset_timestamp.is_none());
    assert_eq!(
        geo["features"][0]["properties"]["tags"]["name"],
        "测试 & café"
    );
    assert_eq!(
        geo["features"][0]["properties"]["osm_metadata"]["user"],
        "A & B"
    );
    assert_eq!(
        geo["features"][2]["geometry"]["coordinates"],
        json!([[0., 0.], [2., 1.]])
    );
    assert_eq!(geo["features"][3]["geometry"]["type"], "GeometryCollection");
    let initial = super::super::initial("snapshot.osm", "reference").unwrap();
    let inspected = super::super::normalize(XML.as_bytes(), initial).unwrap();
    assert_eq!(inspected.asset.format, "osm-xml");
    assert_eq!(
        super::super::normalize(XML.as_bytes(), inspected.asset.clone())
            .unwrap()
            .asset,
        inspected.asset
    );
}
#[test]
fn xml_rejects_edits_history_entities_duplicate_tags_and_incomplete_geometry() {
    for text in [
        XML.replace("<osm ", "<osmChange "),
        XML.replace("id=\"1\"", "id=\"-1\""),
        XML.replace("id=\"1\"", "id=\"9007199254740992\""),
        XML.replace("version=\"2\"", "visible=\"false\""),
        XML.replace("ref=\"2\"", "ref=\"99\""),
        XML.replace(
            "<tag k=\"highway\" v=\"residential\"/>",
            "<tag k=\"highway\" v=\"residential\"/><tag k=\"highway\" v=\"secondary\"/>",
        ),
        XML.replace(
            "<osm version",
            "<!DOCTYPE osm [<!ENTITY x SYSTEM 'file:///secret'>]><osm version",
        ),
        XML.replace("<nd ref=\"1\"/>", "<nd ref=\"1\" lat=\"0\" lon=\"0\"/>"),
        XML.replace("id=\"2\" lon=\"2\"", "id=\"1\" lon=\"2\""),
    ] {
        assert!(decode(text.as_bytes()).is_err(), "{text}");
    }
}
fn var(mut n: u64) -> Vec<u8> {
    let mut v = Vec::new();
    while n >= 128 {
        v.push(n as u8 | 128);
        n >>= 7;
    }
    v.push(n as u8);
    v
}
fn numeric(id: u64, n: u64) -> Vec<u8> {
    let mut v = var(id << 3);
    v.extend(var(n));
    v
}
fn bytes(id: u64, b: &[u8]) -> Vec<u8> {
    let mut v = var(id << 3 | 2);
    v.extend(var(b.len() as u64));
    v.extend(b);
    v
}
fn block(kind: &str, payload: &[u8]) -> Vec<u8> {
    let blob = bytes(1, payload);
    let h = [bytes(1, kind.as_bytes()), numeric(3, blob.len() as u64)].concat();
    [(h.len() as u32).to_be_bytes().to_vec(), h, blob].concat()
}
fn header(features: &[&str]) -> Vec<u8> {
    block(
        "OSMHeader",
        &features
            .iter()
            .flat_map(|f| bytes(4, f.as_bytes()))
            .collect::<Vec<_>>(),
    )
}
fn point_file(group: &[u8], additional: &[u8]) -> Vec<u8> {
    let h = header(&["OsmSchema-V0.6", "DenseNodes"]);
    let table = bytes(1, b"");
    [
        h,
        block(
            "OSMData",
            &[bytes(1, &table), bytes(2, group), additional.to_vec()].concat(),
        ),
    ]
    .concat()
}
#[test]
fn pbf_non_dense_offsets_unknown_fields_and_checked_arithmetic() {
    let node = [
        numeric(1, 2),
        numeric(8, 4),
        numeric(9, 6),
        numeric(100, 123),
    ]
    .concat();
    let extra = [
        numeric(17, 1000),
        numeric(19, 1000000000),
        numeric(20, 2000000000),
    ]
    .concat();
    let file = point_file(&bytes(1, &node), &extra);
    let (geo, source) = decode(&file).unwrap();
    assert_eq!(source.object_counts.node, 1);
    let c = geo["features"][0]["geometry"]["coordinates"]
        .as_array()
        .unwrap();
    assert!((c[0].as_f64().unwrap() - 2.000003).abs() < 1e-12);
    assert!((c[1].as_f64().unwrap() - 1.000002).abs() < 1e-12);
    for invalid in [
        point_file(&bytes(1, &node), &numeric(17, 0)),
        point_file(&bytes(1, &node), &numeric(20, i64::MAX as u64)),
        point_file(&[bytes(1, &node), bytes(3, &numeric(1, 3))].concat(), &[]),
    ] {
        assert!(decode(&invalid).is_err());
    }
}
#[test]
fn pbf_rejects_features_wire_corruption_cardinality_history_and_trailing_data() {
    let dense = [bytes(1, &var(2)), bytes(8, &var(0)), bytes(9, &var(0))].concat();
    let good = point_file(&bytes(2, &dense), &[]);
    assert!(decode(&good).is_ok());
    for bad in [
        header(&["OsmSchema-V0.6", "HistoricalInformation"]),
        header(&["OsmSchema-V0.6", "FutureUnknown"]),
        point_file(&bytes(2, &[dense.clone(), bytes(8, &var(0))].concat()), &[]),
        point_file(
            &bytes(2, &[dense.clone(), bytes(10, &var(2))].concat()),
            &[],
        ),
        point_file(
            &bytes(2, &[dense, bytes(5, &bytes(6, &var(0)))].concat()),
            &[],
        ),
        [good.clone(), vec![0]].concat(),
        block("OSMData", b""),
        point_file(
            &bytes(
                1,
                &[numeric(1, 2), numeric(8, 0), numeric(9, 0), numeric(1, 4)].concat(),
            ),
            &[],
        ),
    ] {
        assert!(decode(&bad).is_err(), "{bad:?}");
    }
    for end in 0..good.len() {
        assert!(
            decode(&good[..end]).is_err() || end == header(&["OsmSchema-V0.6", "DenseNodes"]).len()
        );
    }
    assert!(decode(&point_file(
        &bytes(
            1,
            &[0x08, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 2]
        ),
        &[]
    ))
    .is_err());
}
