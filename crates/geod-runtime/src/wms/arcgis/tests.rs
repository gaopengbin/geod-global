use super::*;
#[test]
fn actual_public_service_discovery_fixtures() {
    let cases = [
        (
            "https://sampleserver6.arcgisonline.com/arcgis/rest/services/USA/MapServer",
            include_str!("../../../fixtures/arcgis-map/map-metadata.json"),
            Some(include_str!("../../../fixtures/arcgis-map/map-layers.json")),
        ),
        (
            "https://imagery.nationalmap.gov/arcgis/rest/services/USGSNAIPImagery/ImageServer",
            include_str!("../../../fixtures/arcgis-map/usgs-metadata.json"),
            None,
        ),
    ];
    for (url, raw, layers) in cases {
        let s = parse(
            &Url::parse(url).unwrap(),
            "Public test",
            raw.into(),
            layers.map(str::to_owned),
        )
        .unwrap();
        validate_service(&s).unwrap();
        assert!(!s.layers.is_empty());
    }
}
fn service(image: bool) -> MapService {
    let metadata=serde_json::json!({"currentVersion":11.3,"name":"Ortho","capabilities":if image{"Image,Metadata"}else{"Map,Query"},"supportedImageFormatTypes":"PNG32,JPG","maxImageWidth":4000,"maxImageHeight":4000,"copyrightText":"Test provider","layers":[{"id":2,"name":"Land"}],"documentInfo":{"Title":"Test land","AccessConstraints":"Dataset terms apply"}}).to_string();
    parse(&Url::parse(&format!("https://maps.example.com/arcgis/rest/services/Land/{}",if image{"ImageServer"}else{"MapServer"})).unwrap(),"Test",metadata,if image{None}else{Some(serde_json::json!({"layers":[{"id":2,"name":"Land","type":"Feature Layer","copyrightText":"Test provider","description":"Land map"}]}).to_string())}).unwrap()
}
fn asset() -> MapImage {
    let s = service(true);
    let requested = [-122.5, 37.7, -122.4, 37.8];
    let bounds = [-122.55, 37.7, -122.35, 37.8];
    let size = [2, 1];
    let raw=serde_json::json!({"href":"https://maps.example.com/arcgis/rest/directories/output/Land_ImageServer/_ags_image.png","width":2,"height":1,"extent":{"xmin":bounds[0],"ymin":bounds[1],"xmax":bounds[2],"ymax":bounds[3],"spatialReference":{"wkid":4326,"latestWkid":4326}}}).to_string();
    let l = &s.layers[0];
    MapImage{id:Uuid::new_v4().to_string(),name:"Test image".into(),width:2,height:1,bounds,bytes:100,sha256:"0".repeat(64),crs:"EPSG:4326".into(),image_extent:None,source:MapSource{xyz:None,service_url:s.url.clone(),service_name:s.name.clone(),service_title:s.title.clone(),version:s.version.clone(),map_endpoint:s.map_url.clone(),capabilities_sha256:s.capabilities_sha256.clone(),layer_name:l.name.clone(),layer_title:l.title.clone(),style:String::new(),time:None,request_crs:"EPSG:4326".into(),request_url:export_url(&s,l,requested,size,None,"735f227b-5f95-473f-967b-077f3419bc68").unwrap().to_string(),requested_at:now(),access_constraints:s.access_constraints.clone(),attribution:l.attribution.clone(),area_geometry:None,selection:"bbox-rendered-map".into(),wmts:None,arcgis:Some(Snapshot{capabilities:s.arcgis.clone().unwrap(),requested_bounds:requested,request_id:"735f227b-5f95-473f-967b-077f3419bc68".into(),export_sha256:hash(raw.as_bytes()),export_metadata:raw,image_url:"https://maps.example.com/arcgis/rest/directories/output/Land_ImageServer/_ags_image.png".into()})}}
}
#[test]
fn discovery_and_metadata_binding() {
    for image in [false, true] {
        let s = service(image);
        validate_service(&s).unwrap();
        assert_eq!(s.max_width, 2048);
        let mut changed = s.clone();
        changed.layers[0].title = "Other".into();
        assert!(validate_service(&changed).is_err());
    }
    let s = service(false);
    let u = export_url(
        &s,
        &s.layers[0],
        [-123., 37., -122., 38.],
        [512, 512],
        None,
        "735f227b-5f95-473f-967b-077f3419bc68",
    )
    .unwrap();
    let q: BTreeMap<_, _> = u.query_pairs().collect();
    assert_eq!(q["layers"], "show:2");
    assert_eq!(q["imageSR"], "4326");
    assert_eq!(q["f"], "json");
}
#[test]
fn actual_extent_is_preserved_and_forgery_rejected() {
    let a = asset();
    validate_asset(&a).unwrap();
    let mut wrong = a.clone();
    wrong.bounds = wrong.source.arcgis.as_ref().unwrap().requested_bounds;
    assert!(validate_asset(&wrong).is_err());
    for mutation in 0..4 {
        let mut wrong = a.clone();
        match mutation {
            0 => wrong.source.arcgis.as_mut().unwrap().export_sha256 = "f".repeat(64),
            1 => {
                wrong.source.arcgis.as_mut().unwrap().image_url =
                    "https://evil.example.com/_ags_image.png".into()
            }
            2 => wrong.source.layer_title = "Wrong".into(),
            _ => wrong.source.arcgis.as_mut().unwrap().requested_bounds = [-123., 37., -122., 38.],
        };
        assert!(validate_asset(&wrong).is_err());
    }
}
#[test]
fn output_href_stays_in_public_generated_directory() {
    let root =
        Url::parse("https://maps.example.com/arcgis/rest/services/Land/ImageServer").unwrap();
    for raw in [
        "https://maps.example.com/arcgis/rest/directories/output/Land_ImageServer/_ags_image.png",
        "https://maps.example.com/arcgisoutput/_ags_image.png",
    ] {
        assert!(
            image_url(&root, raw).is_ok(),
            "{raw}: {:?}",
            image_url(&root, raw)
        );
    }
    for raw in [
        "https://evil.example.com/arcgisoutput/_ags_image.png",
        "http://maps.example.com/arcgisoutput/_ags_image.png",
        "https://127.0.0.1/arcgisoutput/_ags_image.png",
        "https://maps.example.com/arcgis/rest/directories/output/Other_ImageServer/_ags_image.png",
        "https://maps.example.com/arcgis/admin/_ags_image.png",
        "https://maps.example.com/arcgisoutput/_ags_image.png?token=secret",
        "https://maps.example.com/arcgisoutput/_ags_image.png#fragment",
        "https://maps.example.com/arcgisoutput/_ags_image.jpg",
    ] {
        assert!(image_url(&root, raw).is_err(), "{raw}");
    }
}
#[test]
fn service_exception_never_counts_as_image() {
    assert!(json(
        r#"{"error":{"code":499,"message":"Token required"}}"#,
        MAX_JSON
    )
    .unwrap_err()
    .contains("authorization"));
    assert!(json(r#"{"error":{"code":400}}"#, MAX_JSON).is_err());
    let a = asset();
    let p = a.source.arcgis.unwrap();
    let mut v: Value = serde_json::from_str(&p.export_metadata).unwrap();
    v["extent"]["spatialReference"]["latestWkid"] = 3857.into();
    assert!(export_response(
        &service_url(&a.source.service_url).unwrap(),
        &v.to_string(),
        [2, 1],
        p.requested_bounds
    )
    .is_err());
}
#[test]
fn historical_time_is_explicit_and_bounded() {
    let t = time_dimension(&serde_json::json!({"timeExtent":[0,86400000]})).unwrap();
    assert!(!time_allowed(&t, None));
    assert!(time_allowed(&t, Some("1970-01-01")));
    assert!(time_allowed(&t, Some("1970-01-01T12:00:00Z")));
    assert!(!time_allowed(&t, Some("1970-01-03")));
    assert!(!time_allowed(&t, Some("1970-01-01T12:00:00.000000001Z")));
    let mut s = service(true);
    s.layers[0].time = t;
    let u = export_url(
        &s,
        &s.layers[0],
        [-123., 37., -122., 38.],
        [2, 2],
        Some("1970-01-01T12:00:00Z"),
        "735f227b-5f95-473f-967b-077f3419bc68",
    )
    .unwrap();
    assert_eq!(
        u.query_pairs().find(|(k, _)| k == "time").unwrap().1,
        "43200000"
    );
}
#[test]
fn package_uses_returned_extent_and_retains_raw_receipts() {
    let a = asset();
    let bytes = super::super::package(&a, vec![1, 2, 3]).unwrap();
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut world = String::new();
    zip.by_name("map.pgw")
        .unwrap()
        .read_to_string(&mut world)
        .unwrap();
    let v: Vec<f64> = world.lines().map(|n| n.parse().unwrap()).collect();
    assert!((v[0] - 0.1).abs() < 1e-12);
    assert!((v[4] - (-122.5)).abs() < 1e-12);
    let mut raw = String::new();
    zip.by_name("export-response.json")
        .unwrap()
        .read_to_string(&mut raw)
        .unwrap();
    assert_eq!(raw, a.source.arcgis.unwrap().export_metadata);
    assert!(zip.by_name("service-metadata.json").is_ok());
}
