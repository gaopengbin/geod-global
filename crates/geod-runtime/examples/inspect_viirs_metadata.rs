use hdf5_reader::Hdf5File;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("HDF5 path required")?;
    let file = Hdf5File::open(path)?;
    for group in ["/", "/HDFEOS/ADDITIONAL/FILE_ATTRIBUTES"] {
        let g = file.group(group)?;
        let wanted = [
            "LocalGranuleID",
            "VersionID",
            "ShortName",
            "HorizontalTileNumber",
            "VerticalTileNumber",
            "RangeBeginningDate",
            "RangeBeginningTime",
            "RangeEndingDate",
            "RangeEndingTime",
            "SensorShortname",
        ];
        for key in wanted {
            match g.attribute(key) {
                Ok(a) => println!(
                    "{}",
                    serde_json::json!({"group":group,"key":key,"elements":a.num_elements()?,"rawBytes":a.raw_data.len(),"text":a.read_string().ok()})
                ),
                Err(_) => println!(
                    "{}",
                    serde_json::json!({"group":group,"key":key,"absent":true})
                ),
            }
        }
    }
    let d = file.dataset("/HDFEOS INFORMATION/StructMetadata.0")?;
    println!(
        "{}",
        serde_json::json!({"structShape":d.shape(),"dtype":format!("{:?}",d.dtype()),"text":d.read_string().ok()})
    );
    let d = file.dataset("/HDFEOS/GRIDS/VIIRS_Grid_1km_L3_2d/Data Fields/SurfReflect_M5")?;
    println!(
        "{}",
        serde_json::json!({"bandShape":d.shape(),"dtype":format!("{:?}",d.dtype()),"chunks":d.chunks()})
    );
    for key in ["scale_factor", "add_offset", "_FillValue", "valid_range"] {
        let a = d.attribute(key)?;
        println!(
            "{}",
            serde_json::json!({"key":key,"f64":a.read_1d::<f64>().ok(),"f32":a.read_1d::<f32>().ok(),"i16":a.read_1d::<i16>().ok(),"elements":a.num_elements()?})
        );
    }
    Ok(())
}
