//! Four unchanged, corrected SG2017 WRB originals. The `latest` URL is not a
//! licence or version declaration: exact whole-file hashes pin this scope.
use super::*;
use std::sync::LazyLock;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewedIndex {
    schema_version: u32,
    dataset_id: String,
    release: String,
    readme_url: String,
    legend_url: String,
    license_evidence_url: String,
    class_legend: Vec<Class>,
    originals: Vec<Original>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Class {
    code: u8,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Original {
    id: String,
    layer: String,
    publisher_layer: String,
    tile: String,
    url: String,
    bytes: u64,
    sha256: String,
    bounds: [f64; 4],
    width: u32,
    height: u32,
    bands: u8,
    data_type: String,
    nodata: u8,
    crs: String,
    pixel_interpretation: String,
    pixel_scale: [f64; 3],
    tiepoint: [f64; 6],
    storage_layout: String,
    compression: u16,
    rows_per_strip: u32,
    minimum: u8,
    maximum: u8,
    valid_zero_count: u64,
    no_data_count: u64,
    samples: Vec<Sample>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Sample {
    column: u32,
    row: u32,
    values: Vec<u8>,
    no_data: Vec<bool>,
}

static INDEX: LazyLock<ReviewedIndex> = LazyLock::new(|| {
    serde_json::from_str(include_str!("soilgrids-reviewed-index.json"))
        .expect("Reviewed complete corrected SoilGrids WRB 2017 originals")
});

pub(super) fn definitions() -> Vec<Dataset> {
    let mut d = base(
        "soilgrids",
        "SoilGrids250m WRB 2017 corrected originals",
        "soil",
        &INDEX.readme_url,
    );
    d.description = "Four complete ISRIC GeoTIFF originals in the corrected geographic distribution of the SG2017 WRB soil-class maps: tiles 530 and 531 for MostProbable and Acrisols. The publisher README identifies a rerun of the 2017 inputs to correct low-latitude distortion. This finite scope does not include current depth-dependent physical soil properties, other soil classes or global discovery.".into();
    d.version = INDEX.release.clone();
    d.license = "Open Data Commons Open Database License (ODbL) 1.0; SoilGrids 2017".into();
    d.license_url = "https://opendatacommons.org/licenses/odbl/1-0/".into();
    d.attribution = format!("ISRIC - World Soil Information; Hengl et al. (2017), SoilGrids250m: global gridded soil information based on Machine Learning, doi:10.1371/journal.pone.0169748. Retain the corrected SG2017 WRB distribution, selected publisher layer, tile, original SHA-256 and access date. The SoilGrids 2017 FAQ establishes ODbL: {}. Later product licences do not silently replace this release's licence.", INDEX.license_evidence_url);
    d.nominal_resolution = "250 m nominal; unchanged EPSG:4326 4096 x 4096 Area grids, approximately 0.002083333 degree spacing".into();
    d.unit = "WRB class code or unconverted Acrisols raw score".into();
    let legend = INDEX
        .class_legend
        .iter()
        .map(|v| format!("{} {}", v.code, v.name))
        .collect::<Vec<_>>()
        .join(", ");
    let categorical = format!("Unchanged UInt8 categorical values. Exact publisher class legend ({}): {}. Class 0 is valid Acrisols; 255 is the TIFF NoData value. Scale 1, offset 0. These are predicted WRB categories, not physical measurements or a depth profile.", INDEX.legend_url, legend);
    let score = "Unchanged UInt8 Acrisols model score: scale 1, offset 0, NoData 255. Zero is valid. The reviewed TIFF and VRT do not declare a probability calibration, percent unit or scaling factor; display and inspection retain the raw byte without dividing by 100, adding a percent sign or treating it as a measured soil property.";
    d.value_interpretation = format!("{categorical} {score}");
    d.layers = vec![
        scaled(
            layer(
                "most-probable",
                "Most probable WRB soil class",
                "WRB class code",
                &categorical,
            ),
            1.,
            0.,
            255.,
        ),
        scaled(
            layer(
                "acrisols-score",
                "Acrisols raw model score",
                "raw score",
                score,
            ),
            1.,
            0.,
            255.,
        ),
    ];
    d.default_layer = Some("most-probable".into());
    d.warnings = vec![
        "Only the two exact original tile footprints (530 and 531) are reviewed for each of the two layers. An intersecting tile does not prove valid pixels or scientific accuracy throughout the selected area. Complete original grids are downloaded without clipping or resampling.".into(),
        "The official path contains latest, but the publisher README identifies corrected 2017 WRB inputs. No year, month, depth, country, region or season filter is offered; this is not the later SoilGrids depth-property product.".into(),
        "MostProbable uses the exact publisher class legend, including valid class 0=Acrisols. Acrisols remains a raw score because this corrected file's percent/probability encoding is unverified. Only 255 is embedded NoData; valid zeros and categorical labels are preserved.".into(),
        "The grayscale map preview displays original numeric codes/scores for browsing. It does not introduce a publisher colour legend, an inferred uncertainty interval, a recalibrated probability or new soil classifications.".into(),
    ];
    d.limits = vec!["Two exact 4096 x 4096 single-band UInt8 original TIFFs per selected layer, four originals in total. Exact full-file byte length and SHA-256 must match before acceptance. Existing 512 MiB transfer, raster dimensions, strips, cumulative decoding, preview and timeout limits remain unchanged.".into()];
    vec![d]
}

fn validate_index() -> Result<()> {
    if INDEX.schema_version != 1
        || INDEX.dataset_id != "soilgrids"
        || INDEX.originals.len() != 4
        || INDEX.class_legend.len() != 30
        || INDEX
            .class_legend
            .iter()
            .enumerate()
            .any(|(n, c)| usize::from(c.code) != n || c.name.is_empty())
        || INDEX.class_legend[0].name != "Acrisols"
    {
        return Err("Reviewed corrected SG2017 WRB scope or original class legend changed".into());
    }
    for t in &INDEX.originals {
        features::bounds(t.bounds)?;
        let valid_layer = matches!(
            (t.layer.as_str(), t.publisher_layer.as_str()),
            ("most-probable", "MostProbable") | ("acrisols-score", "Acrisols")
        );
        if !valid_layer
            || !matches!(t.tile.as_str(), "530" | "531")
            || t.id != format!("wrb-{}-{}", t.layer, t.tile)
            || t.url
                != format!(
                    "https://files.isric.org/soilgrids/latest/data/wrb/{}/{}.tif",
                    t.publisher_layer, t.tile
                )
            || t.bytes == 0
            || t.bytes > 512 * 1024 * 1024
            || t.sha256.len() != 64
            || !t
                .sha256
                .bytes()
                .all(|v| v.is_ascii_hexdigit() && !v.is_ascii_uppercase())
            || t.width != 4096
            || t.height != 4096
            || t.bands != 1
            || t.data_type != "UInt8"
            || t.nodata != 255
            || t.crs != "EPSG:4326"
            || t.pixel_interpretation != "area"
            || t.storage_layout != "strip"
            || t.rows_per_strip != 2
            || t.compression != 8
            || t.pixel_scale[0] <= 0.
            || t.pixel_scale[0] != t.pixel_scale[1]
            || t.pixel_scale[2] != 0.
            || t.tiepoint != [0., 0., 0., t.bounds[0], t.bounds[3], 0.]
            || t.minimum != 0
            || t.maximum == 255
            || t.valid_zero_count == 0
            || t.no_data_count == 0
            || t.valid_zero_count + t.no_data_count > u64::from(t.width) * u64::from(t.height)
            || t.samples.iter().any(|p| {
                p.column >= t.width
                    || p.row >= t.height
                    || p.values.len() != 1
                    || p.no_data != [p.values[0] == 255]
            })
        {
            return Err(
                "Reviewed corrected SG2017 WRB original grid or scientific byte profile changed"
                    .into(),
            );
        }
    }
    Ok(())
}

pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    features::bounds(request.bounds)?;
    validate_index()?;
    if request.dataset_id != "soilgrids"
        || request.year.is_some()
        || request.month.is_some()
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
        || !matches!(
            request.layer.as_deref(),
            Some("most-probable" | "acrisols-score")
        )
    {
        return Err("Choose the reviewed corrected SG2017 WRB MostProbable class or unconverted Acrisols score; this finite scope has no year, month, depth, country, region or season selection".into());
    }
    let sources = INDEX
        .originals
        .iter()
        .filter(|t| {
            Some(t.layer.as_str()) == request.layer.as_deref()
                && request.bounds[0] < t.bounds[2]
                && request.bounds[2] > t.bounds[0]
                && request.bounds[1] < t.bounds[3]
                && request.bounds[3] > t.bounds[1]
        })
        .map(|t| {
            let mut s = source(
                format!(
                    "SoilGrids WRB 2017 corrected {} tile {}",
                    t.publisher_layer, t.tile
                ),
                t.url.clone(),
                Some(t.bounds),
            );
            s.id = t.id.clone();
            s
        })
        .collect::<Vec<_>>();
    if sources.is_empty() {
        return Err("The selected area intersects none of the two exact reviewed corrected SG2017 WRB original grids; no global tile, other layer or demonstration region is substituted".into());
    }
    Ok(sources)
}

pub(crate) fn validate_original(
    proof: &SourceProvenance,
    url: &str,
    bytes: u64,
    sha256: &str,
) -> Result<()> {
    super::validate_provenance(proof, url)?;
    let t = INDEX
        .originals
        .iter()
        .find(|v| v.id == proof.source_id)
        .ok_or("Unreviewed corrected SoilGrids WRB original")?;
    if proof.dataset.id != "soilgrids"
        || proof.request.layer.as_deref() != Some(t.layer.as_str())
        || proof.layer.id != t.layer
        || proof.source_bounds != Some(t.bounds)
        || url != t.url
        || bytes != t.bytes
        || sha256 != t.sha256
    {
        return Err("Corrected SG2017 WRB original differs from the exact native source, full byte length or SHA-256; a replacement under latest is not accepted".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
