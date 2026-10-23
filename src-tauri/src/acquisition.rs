//! Public campaign labels only. This file cannot configure networking or credentials.
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Acquisition {
    pub schema_version: u8,
    pub source: String,
    pub campaign: String,
}

pub fn read_from(directory: &Path) -> Option<Acquisition> {
    let path = directory.join("acquisition.json");
    let metadata = std::fs::symlink_metadata(&path).ok()?;
    if !metadata.is_file() || metadata.len() > 512 {
        return None;
    }
    let value: Acquisition = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    (value.schema_version == 1
        && matches!(value.source.as_str(), "zhihu" | "bilibili")
        && value.campaign == "landsat_batch_20261023")
        .then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn campaign_labels_are_bounded_and_cannot_add_configuration() {
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let directory = std::env::temp_dir().join(format!("geod-acquisition-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        assert!(read_from(&directory).is_none());
        for source in ["zhihu", "bilibili"] {
            std::fs::write(directory.join("acquisition.json"), format!(r#"{{"schemaVersion":1,"source":"{source}","campaign":"landsat_batch_20261023"}}"#)).unwrap();
            assert_eq!(read_from(&directory).unwrap().source, source);
        }
        for text in [
            r#"{"schemaVersion":2,"source":"zhihu","campaign":"landsat_batch_20261023"}"#,
            r#"{"schemaVersion":1,"source":"https://private.test","campaign":"landsat_batch_20261023"}"#,
            r#"{"schemaVersion":1,"source":"zhihu","campaign":"arbitrary"}"#,
            r#"{"schemaVersion":1,"source":"zhihu","campaign":"landsat_batch_20261023","url":"https://private.test"}"#,
            "broken",
        ] {
            std::fs::write(directory.join("acquisition.json"), text).unwrap();
            assert!(read_from(&directory).is_none());
        }
        std::fs::write(directory.join("acquisition.json"), " ".repeat(513)).unwrap();
        assert!(read_from(&directory).is_none());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
