//! MOD13Q1/MYD13Q1 v061 ancillary science layers. Colours are display-only;
//! flags, ranks, angles and day numbers keep their original scalar type and DN.
use super::*;
use crate::{io_error, providers::vegetation};
use std::collections::BTreeMap;

pub(crate) const DEFINITION: &str =
    "https://lpdaac.usgs.gov/documents/621/MOD13_User_Guide_V61.pdf";
pub(crate) const PALETTE: &str = "modis13-science-v1";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScienceDisplay {
    pub product: String,
    pub band: String,
    pub layer: String,
    pub kind: String,
    pub unit: String,
    pub scale: f64,
    pub offset: f64,
    pub valid_range: [i32; 2],
    pub display_range: [i32; 2],
    pub palette: String,
    pub sample_count: u32,
    pub valid_sample_count: u32,
    pub out_of_range_sample_count: u32,
    pub counts_full_resolution: bool,
    pub pixel_interpretation: String,
    pub definition: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calendar_year: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SciencePixel {
    pub within_range: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub converted_value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<quality::QualityPixel>,
}

pub(crate) fn profile(key: &str, item: &str) -> Result<reflectance::Profile> {
    let layer = vegetation::layer(key).ok_or("Unsupported MODIS science layer")?;
    vegetation::identity(item).ok_or("Invalid MODIS science product identity")?;
    Ok(reflectance::Profile {
        product: vegetation::PRODUCT.into(),
        signed: layer.signed,
        scale: layer.scale,
        offset: 0.0,
        nodata: layer.nodata,
        sample_bits: (layer.bits != 16).then_some(layer.bits),
        science_key: Some(layer.key.into()),
        calendar_year: if key == "vi_doy" {
            Some(item[9..13].parse().map_err(io_error)?)
        } else {
            None
        },
    })
}

fn layer_name(key: &str) -> &'static str {
    vegetation::ASSETS
        .iter()
        .find(|(k, _)| *k == key)
        .unwrap()
        .1
}

pub(crate) fn metadata(profile: &reflectance::Profile) -> Result<String> {
    let key = profile
        .science_key
        .as_deref()
        .ok_or("Missing science layer identity")?;
    let layer = vegetation::layer(key).ok_or("Unsupported science profile")?;
    let year = profile.calendar_year.map_or(String::new(), |year| {
        format!("<Item name=\"CALENDAR_YEAR\">{year}</Item>")
    });
    Ok(format!("<GDALMetadata><Item name=\"PRODUCT\">{}</Item><Item name=\"SCIENCE_KEY\">{key}</Item><Item name=\"SCIENCE_LAYER\">{}</Item><Item name=\"UNIT\" sample=\"0\" role=\"unittype\">{}</Item><Item name=\"SCALE\" sample=\"0\" role=\"scale\">{}</Item><Item name=\"OFFSET\" sample=\"0\" role=\"offset\">0</Item><Item name=\"VALID_MIN\">{}</Item><Item name=\"VALID_MAX\">{}</Item><Item name=\"DEFINITION\">{DEFINITION}</Item>{year}</GDALMetadata>", vegetation::PRODUCT, layer_name(key), layer.unit, layer.scale, layer.range[0], layer.range[1]))
}

pub(crate) fn validate_metadata<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    job: &Job,
    profile: &reflectance::Profile,
) -> Result<()> {
    let Some(key) = profile.science_key.as_deref() else {
        return Ok(());
    };
    if key != job.asset_key {
        return Err("MODIS science layer identity differs".into());
    }
    let layer = vegetation::layer(key).ok_or("Unknown science layer")?;
    let xml = decoder
        .get_tag(Tag::Unknown(42112))
        .map_err(io_error)?
        .into_string()
        .map_err(io_error)?;
    if xml.len() > 128 * 1024 {
        return Err("Science metadata exceeds its limit".into());
    }
    let document = roxmltree::Document::parse(xml.trim_end_matches('\0')).map_err(io_error)?;
    if document.root_element().tag_name().name() != "GDALMetadata" {
        return Err("Invalid science metadata root".into());
    }
    let mut tags = BTreeMap::new();
    for node in document
        .root_element()
        .children()
        .filter(|n| n.is_element())
    {
        let key = node
            .attribute("name")
            .ok_or("Missing science metadata name")?;
        let value = node.text().unwrap_or("").trim();
        if node.tag_name().name() != "Item"
            || node.attribute("sample").is_some_and(|s| s != "0")
            || tags
                .insert(key, value)
                .is_some_and(|previous| previous != value)
        {
            return Err("Invalid or duplicate science metadata item".into());
        }
    }
    let value = |key| tags.get(key).copied();
    let number = |key| value(key).and_then(|v| v.parse::<f64>().ok());
    let differs = if job.kind == "download" {
        let period = vegetation::period(&job.item_id).ok_or("Invalid science period")?;
        value("LOCALGRANULEID") != Some(format!("{}.hdf", job.item_id).as_str())
            || value("SHORTNAME") != Some(&job.item_id[..7])
            || value("VERSIONID") != Some("61")
            || value("long_name") != Some(layer_name(key).replace('_', " ").as_str())
            || value("RANGEBEGINNINGDATE") != Some(&period[0][..10])
            || value("RANGEENDINGDATE") != Some(&period[1][..10])
            || value("units")
                != Some(if key == "vi_doy" {
                    "Julian day of the year"
                } else {
                    layer.unit
                })
            || value("valid_range")
                != Some(format!("{}, {}", layer.range[0], layer.range[1]).as_str())
            || number("_FillValue") != Some(f64::from(layer.nodata))
            || if layer.scale != 1.0 {
                number("scale_factor") != Some(1.0 / layer.scale)
                    || number("add_offset") != Some(0.0)
            } else {
                number("scale_factor").is_some_and(|n| n != 1.0)
                    || number("add_offset").is_some_and(|n| n != 0.0)
            }
    } else {
        value("PRODUCT") != Some(vegetation::PRODUCT)
            || value("SCIENCE_KEY") != Some(key)
            || value("SCIENCE_LAYER") != Some(layer_name(key))
            || value("UNIT") != Some(layer.unit)
            || number("SCALE") != Some(layer.scale)
            || number("OFFSET") != Some(0.0)
            || number("VALID_MIN") != Some(f64::from(layer.range[0]))
            || number("VALID_MAX") != Some(f64::from(layer.range[1]))
            || value("DEFINITION") != Some(DEFINITION)
            || value("CALENDAR_YEAR").and_then(|y| y.parse::<i32>().ok()) != profile.calendar_year
    };
    if differs {
        return Err("Science file identity, period, units, fill or calibration differs from its reviewed product".into());
    }
    Ok(())
}

pub(crate) fn within_range(key: &str, value: i32, year: Option<i32>) -> bool {
    let layer = vegetation::layer(key).unwrap();
    value != layer.nodata
        && (layer.range[0]..=layer.range[1]).contains(&value)
        && (key != "vi_doy"
            || year
                .and_then(|y| chrono::NaiveDate::from_yo_opt(y, value as u32))
                .is_some())
}

const QUALITY_COLORS: [[u8; 3]; 4] = [
    [37, 99, 235],
    [217, 119, 6],
    [139, 92, 246],
    [100, 116, 139],
];
pub(crate) fn color(key: &str, value: i32) -> [u8; 3] {
    let layer = vegetation::layer(key).unwrap();
    if key == "vi_quality" {
        return QUALITY_COLORS[(value & 3) as usize];
    }
    if key == "vi_reliability" {
        return usize::try_from(value)
            .ok()
            .and_then(|v| QUALITY_COLORS.get(v))
            .copied()
            .unwrap_or([100, 116, 139]);
    }
    let gray = (((value - layer.range[0]) as f64 / (layer.range[1] - layer.range[0]) as f64)
        .clamp(0.0, 1.0)
        * 255.0)
        .round() as u8;
    [gray; 3]
}
pub(crate) fn classes(key: &str, preview: &[i32]) -> Vec<RasterClass> {
    let labels = match key {
        "vi_quality" => [
            "Produced · good",
            "Produced · check flags",
            "Likely cloudy",
            "Not produced · other",
        ],
        "vi_reliability" => ["Good", "Marginal", "Snow or ice", "Cloudy"],
        _ => return Vec::new(),
    };
    labels
        .iter()
        .enumerate()
        .map(|(value, label)| {
            let [r, g, b] = QUALITY_COLORS[value];
            RasterClass {
                value: value as u8,
                label: (*label).into(),
                color: format!("#{r:02x}{g:02x}{b:02x}"),
                count: preview
                    .iter()
                    .filter(|v| {
                        **v != vegetation::layer(key).unwrap().nodata
                            && if key == "vi_quality" {
                                (**v & 3) == value as i32
                            } else {
                                **v == value as i32
                            }
                    })
                    .count() as u64,
            }
        })
        .collect()
}

fn flags(value: u16) -> quality::QualityPixel {
    let field = |name: &str, start: u8, end: u8, labels: &[&str]| {
        let code = (u32::from(value) >> start) & ((1 << (end - start + 1)) - 1);
        let label = labels.get(code as usize).copied().filter(|s| !s.is_empty());
        quality::QualityField {
            name: name.into(),
            start_bit: start,
            end_bit: end,
            value: code,
            label: label.unwrap_or("Unspecified code").into(),
            defined: label.is_some(),
        }
    };
    quality::QualityPixel {
        layer: layer_name("vi_quality").into(),
        binary: format!("{value:016b}"),
        hex: format!("0x{value:04X}"),
        covered: None,
        fields: vec![
            field(
                "MODLAND quality",
                0,
                1,
                &[
                    "Produced · good",
                    "Produced · check flags",
                    "Likely cloudy",
                    "Not produced · other",
                ],
            ),
            field(
                "VI usefulness",
                2,
                5,
                &[
                    "Best",
                    "Lower",
                    "Reduced",
                    "",
                    "Reduced",
                    "",
                    "",
                    "",
                    "Reduced",
                    "Reduced",
                    "Reduced",
                    "",
                    "Lowest",
                    "Unusable quality",
                    "Faulty L1B",
                    "Unusable or not processed",
                ],
            ),
            field(
                "Aerosol quantity",
                6,
                7,
                &["Climatology", "Low", "Intermediate", "High"],
            ),
            field("Adjacent cloud", 8, 8, &["No", "Yes"]),
            field("BRDF correction", 9, 9, &["No", "Yes"]),
            field("Mixed clouds", 10, 10, &["No", "Yes"]),
            field(
                "Land / water",
                11,
                13,
                &[
                    "Shallow ocean",
                    "Land",
                    "Coast or shoreline",
                    "Shallow inland water",
                    "Ephemeral water",
                    "Deep inland water",
                    "Continental ocean",
                    "Deep ocean",
                ],
            ),
            field("Possible snow / ice", 14, 14, &["No", "Yes"]),
            field("Possible shadow", 15, 15, &["No", "Yes"]),
        ],
    }
}
pub(crate) fn pixel(key: &str, value: i32, year: Option<i32>) -> SciencePixel {
    let layer = vegetation::layer(key).unwrap();
    let missing = value == layer.nodata;
    SciencePixel {
        within_range: within_range(key, value, year),
        converted_value: (!missing && matches!(layer.kind, "reflectance" | "angle"))
            .then_some(f64::from(value) * layer.scale),
        date: if key == "vi_doy" && !missing {
            year.and_then(|y| {
                u32::try_from(value)
                    .ok()
                    .and_then(|v| chrono::NaiveDate::from_yo_opt(y, v))
            })
            .map(|d| d.to_string())
        } else {
            None
        },
        flags: (key == "vi_quality" && !missing).then(|| flags(value as u16)),
    }
}
pub(crate) fn display(
    key: &str,
    profile: &reflectance::Profile,
    preview: &[i32],
) -> ScienceDisplay {
    let layer = vegetation::layer(key).unwrap();
    ScienceDisplay {
        product: vegetation::PRODUCT.into(),
        band: key.into(),
        layer: layer_name(key).into(),
        kind: layer.kind.into(),
        unit: layer.unit.into(),
        scale: layer.scale,
        offset: 0.0,
        valid_range: layer.range,
        display_range: layer.range,
        palette: PALETTE.into(),
        sample_count: preview.len() as u32,
        valid_sample_count: preview.iter().filter(|v| **v != layer.nodata).count() as u32,
        out_of_range_sample_count: preview
            .iter()
            .filter(|v| **v != layer.nodata && !within_range(key, **v, profile.calendar_year))
            .count() as u32,
        counts_full_resolution: false,
        pixel_interpretation: "PixelIsArea".into(),
        definition: DEFINITION.into(),
        calendar_year: profile.calendar_year,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_rank_fill_high_unsigned_bit_and_sparse_flags_remain_distinct() {
        assert!(!within_range("vi_reliability", -1, None));
        assert!(within_range("vi_reliability", 0, None));
        assert!(!within_range("vi_quality", 65535, None));
        let p = pixel("vi_quality", 32768, None).flags.unwrap();
        assert_eq!(p.fields[8].value, 1);
        assert_eq!(p.binary, "1000000000000000");
        assert!(!pixel("vi_quality", 12, None).flags.unwrap().fields[1].defined);
        assert_eq!(
            pixel("vi_relative_azimuth", -4000, None).converted_value,
            None
        );
        assert_eq!(
            pixel("vi_relative_azimuth", -1234, None).converted_value,
            Some(-12.34)
        );
    }
    #[test]
    fn day_number_uses_its_source_year_and_keeps_invalid_raw_values() {
        assert_eq!(
            pixel("vi_doy", 60, Some(2024)).date.as_deref(),
            Some("2024-02-29")
        );
        assert_eq!(
            pixel("vi_doy", 60, Some(2025)).date.as_deref(),
            Some("2025-03-01")
        );
        assert!(!pixel("vi_doy", 366, Some(2025)).within_range);
        assert_eq!(pixel("vi_doy", 366, Some(2025)).date, None);
        assert_eq!(pixel("vi_doy", -1, Some(2024)).date, None);
        let a = profile("vi_doy", "MOD13Q1.A2025177.h08v05.061.2025195142416").unwrap();
        let b = profile("vi_doy", "MOD13Q1.A2024177.h08v05.061.2024195142416").unwrap();
        assert_ne!(a, b);
    }
}
