use super::*;

const XML: &str = r#"<Capabilities xmlns="http://www.opengis.net/wmts/1.0" xmlns:ows="http://www.opengis.net/ows/1.1" xmlns:xlink="http://www.w3.org/1999/xlink" version="1.0.0">
<ows:ServiceIdentification><ows:Title>Fixture service</ows:Title></ows:ServiceIdentification>
<ows:OperationsMetadata><ows:Operation name="GetTile"><ows:DCP><ows:HTTP><ows:Get xlink:href="https://maps.example.com/wmts/wmts.cgi"><ows:Constraint name="GetEncoding"><ows:AllowedValues><ows:Value>KVP</ows:Value></ows:AllowedValues></ows:Constraint></ows:Get></ows:HTTP></ows:DCP></ows:Operation></ows:OperationsMetadata>
<Contents><Layer><ows:Identifier>fixture</ows:Identifier><ows:Title>Fixture layer</ows:Title><ows:Abstract>Fixture description</ows:Abstract><Style isDefault="true"><ows:Identifier>default</ows:Identifier></Style><Format>image/png</Format><TileMatrixSetLink><TileMatrixSet>grid</TileMatrixSet></TileMatrixSetLink></Layer>
<TileMatrixSet><ows:Identifier>grid</ows:Identifier><ows:SupportedCRS>EPSG:4326</ows:SupportedCRS><TileMatrix><ows:Identifier>level</ows:Identifier><ScaleDenominator>397569609.982</ScaleDenominator><TopLeftCorner>90 -180</TopLeftCorner><TileWidth>256</TileWidth><TileHeight>256</TileHeight><MatrixWidth>2</MatrixWidth><MatrixHeight>1</MatrixHeight></TileMatrix></TileMatrixSet></Contents></Capabilities>"#;

fn parse(xml: &str) -> Result<MapService> {
    parse_capabilities(
        xml.as_bytes(),
        &Url::parse("https://maps.example.com/wmts/wmts.cgi").unwrap(),
        "User connection name",
    )
}

#[test]
fn multilingual_ows_descriptions_have_a_deterministic_fallback() {
    let xml = XML
        .replace(
            "<ows:Title>Fixture service</ows:Title>",
            "<ows:Title xml:lang=\"zh\">示例服务</ows:Title><ows:Title xml:lang=\"EN\">English service</ows:Title>",
        )
        .replace(
            "<ows:Title>Fixture layer</ows:Title>",
            "<ows:Title xml:lang=\"en\">English layer</ows:Title><ows:Title>Default layer</ows:Title><ows:Title xml:lang=\"zh\">示例图层</ows:Title>",
        )
        .replace(
            "<ows:Abstract>Fixture description</ows:Abstract>",
            "<ows:Abstract xml:lang=\"zh\">第一段说明</ows:Abstract><ows:Abstract xml:lang=\"ja\">別の説明</ows:Abstract>",
        );
    let service = parse(&xml).unwrap();
    assert_eq!(service.title, "English service");
    assert_eq!(service.layers[0].title, "Default layer");
    assert_eq!(service.layers[0].description, "第一段说明");
    let english = xml.replace(
        "<ows:Abstract xml:lang=\"ja\">別の説明</ows:Abstract>",
        "<ows:Abstract xml:lang=\"en-GB\">English description</ows:Abstract>",
    );
    assert_eq!(
        parse(&english).unwrap().layers[0].description,
        "English description"
    );
    let inherited = XML.replace("<Layer>", "<Layer xml:lang=\"zh\">").replace(
        "<ows:Title>Fixture layer</ows:Title>",
        "<ows:Title>示例图层</ows:Title><ows:Title xml:lang=\"en\">English layer</ows:Title>",
    );
    assert_eq!(parse(&inherited).unwrap().layers[0].title, "English layer");
}

#[test]
fn all_xml_boolean_spellings_of_style_defaults_are_accepted() {
    for default in ["true", "1", " 1 "] {
        let xml = XML.replace("isDefault=\"true\"", &format!("isDefault=\"{default}\""));
        for other in ["isDefault=\"false\"", "isDefault=\"0\"", ""] {
            let xml = xml.replace(
                "<Format>",
                &format!(
                    "<Style {other}><ows:Identifier>alternative</ows:Identifier></Style><Format>"
                ),
            );
            let service = parse(&xml).unwrap();
            assert_eq!(service.layers[0].styles, ["default", "alternative"]);
            assert_eq!(
                service.layers[0].wmts.as_ref().unwrap().default_style,
                "default"
            );
        }
    }
    assert!(parse(&XML.replace("isDefault=\"true\"", "isDefault=\"yes\"")).is_err());
}

#[test]
fn optional_ows_titles_fall_back_to_the_connection_name_and_layer_identifier() {
    let xml = XML
        .replace("<ows:Title>Fixture service</ows:Title>", "")
        .replace("<ows:Title>Fixture layer</ows:Title>", "")
        .replace("<ows:Abstract>Fixture description</ows:Abstract>", "");
    let service = parse(&xml).unwrap();
    assert_eq!(service.title, "User connection name");
    assert_eq!(service.layers[0].title, "fixture");
    assert!(service.layers[0].description.is_empty());
}

#[test]
fn translations_do_not_relax_identifier_or_same_language_uniqueness() {
    let duplicate_identifier = XML.replace(
        "<ows:Identifier>fixture</ows:Identifier>",
        "<ows:Identifier>fixture</ows:Identifier><ows:Identifier>other</ows:Identifier>",
    );
    assert!(parse(&duplicate_identifier).is_err());
    let duplicate_language = XML.replace(
        "<ows:Title>Fixture layer</ows:Title>",
        "<ows:Title xml:lang=\"en\">First</ows:Title><ows:Title xml:lang=\"EN\">Second</ows:Title>",
    );
    assert!(parse(&duplicate_language).is_err());
}
