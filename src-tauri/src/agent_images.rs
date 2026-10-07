//! User-selected images are decoded, bounded and copied into the owned Agent store.
//! No renderer or model supplied filesystem paths are accepted.
use base64::{engine::general_purpose::STANDARD, Engine};
use image::{GenericImageView, ImageDecoder, ImageFormat, ImageReader, Limits};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, io::Cursor, path::Path};

const UPLOAD_BYTES: usize = 2 * 1024 * 1024;
const IMAGE_BYTES: usize = 5 * 1024 * 1024;
const STORE_BYTES: u64 = 128 * 1024 * 1024;
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Attachment {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    pub bytes: usize,
    pub width: u32,
    pub height: u32,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u8,
    image: Attachment,
}
pub fn valid_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Storage {
    limit_bytes: u64,
    used_bytes: u64,
    image_count: usize,
    unused_bytes: u64,
    unused_count: usize,
    removed_bytes: u64,
    removed_count: usize,
}

/// Only complete, verified preview pairs in the owned directory may be removed.
/// Unknown files, symlinks, all conversation references and current drafts stay.
pub fn storage(home: &Path, keep: &HashSet<String>, cleanup: bool) -> Result<Storage, String> {
    let directory = home.join("images");
    let mut result = Storage {
        limit_bytes: STORE_BYTES,
        ..Storage::default()
    };
    let info = match std::fs::symlink_metadata(&directory) {
        Ok(info) => info,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(result),
        Err(_) => return Err("Agent image storage is unavailable.".into()),
    };
    if !info.is_dir() || info.file_type().is_symlink() {
        return Err("Invalid Agent image storage.".into());
    }
    for entry in std::fs::read_dir(&directory).map_err(|_| "Agent image storage is unavailable.")? {
        let entry = entry.map_err(|_| "Agent image storage is unavailable.")?;
        let info = std::fs::symlink_metadata(entry.path())
            .map_err(|_| "Agent image storage is unavailable.")?;
        if !info.is_file() || info.file_type().is_symlink() {
            continue;
        }
        result.used_bytes = result.used_bytes.saturating_add(info.len());
        let name = entry.file_name();
        let Some(id) = name
            .to_str()
            .and_then(|name| name.strip_suffix(".json"))
            .filter(|id| valid_id(id))
        else {
            continue;
        };
        let Ok((image, _)) = read(home, id) else {
            continue;
        };
        result.image_count += 1;
        if keep.contains(id) {
            continue;
        }
        let bytes = image.bytes as u64 + info.len();
        result.unused_count += 1;
        result.unused_bytes += bytes;
    }
    if !cleanup {
        return Ok(result);
    }
    // Recheck each pair immediately before removing it, without accepting paths
    // from the renderer. The desktop state lock prevents attach/send races.
    for entry in std::fs::read_dir(&directory).map_err(|_| "Agent image storage is unavailable.")? {
        let entry = entry.map_err(|_| "Agent image storage is unavailable.")?;
        let name = entry.file_name();
        let Some(id) = name
            .to_str()
            .and_then(|name| name.strip_suffix(".json"))
            .filter(|id| valid_id(id))
        else {
            continue;
        };
        if keep.contains(id) {
            continue;
        }
        let Ok((image, _)) = read(home, id) else {
            continue;
        };
        let metadata = std::fs::symlink_metadata(entry.path())
            .map_err(|_| "Agent image storage is unavailable.")?;
        std::fs::remove_file(directory.join(format!("{id}.png")))
            .map_err(|_| "Agent images could not be cleaned.")?;
        std::fs::remove_file(entry.path()).map_err(|_| "Agent images could not be cleaned.")?;
        result.removed_bytes += image.bytes as u64 + metadata.len();
        result.removed_count += 1;
    }
    result.used_bytes = result.used_bytes.saturating_sub(result.removed_bytes);
    result.image_count = result.image_count.saturating_sub(result.removed_count);
    result.unused_count = result.unused_count.saturating_sub(result.removed_count);
    result.unused_bytes = result.unused_bytes.saturating_sub(result.removed_bytes);
    Ok(result)
}
fn valid_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name.chars().count() <= 80
        && !name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\'))
}
fn decode(bytes: &[u8]) -> Result<image::DynamicImage, String> {
    let format =
        image::guess_format(bytes).map_err(|_| "Choose a valid PNG, JPEG or WebP image.")?;
    if !matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP
    ) {
        return Err("Choose a valid PNG, JPEG or WebP image.".into());
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits.clone());
    let decode = || -> image::ImageResult<image::DynamicImage> {
        let mut decoder = reader.into_decoder()?;
        // Account for the output pixel buffer as well as decoder allocations.
        limits.max_alloc = Some(
            (64_u64 * 1024 * 1024)
                .checked_sub(decoder.total_bytes())
                .ok_or_else(|| {
                    image::ImageError::Limits(image::error::LimitError::from_kind(
                        image::error::LimitErrorKind::InsufficientMemory,
                    ))
                })?,
        );
        decoder.set_limits(limits)?;
        let orientation = decoder.orientation()?;
        let mut decoded = image::DynamicImage::from_decoder(decoder)?;
        decoded.apply_orientation(orientation);
        Ok(decoded)
    };
    decode().map_err(|_| "This image is damaged or exceeds the supported dimensions.".into())
}
pub fn read(home: &Path, id: &str) -> Result<(Attachment, Vec<u8>), String> {
    if !valid_id(id) {
        return Err("Invalid Agent image reference.".into());
    }
    let directory = home.join("images");
    let directory_info =
        std::fs::symlink_metadata(&directory).map_err(|_| "Agent image could not be read.")?;
    if !directory_info.is_dir() || directory_info.file_type().is_symlink() {
        return Err("Invalid Agent image storage.".into());
    }
    let metadata_path = directory.join(format!("{id}.json"));
    let metadata_info =
        std::fs::symlink_metadata(&metadata_path).map_err(|_| "Agent image could not be read.")?;
    if !metadata_info.is_file()
        || metadata_info.file_type().is_symlink()
        || metadata_info.len() > 1024
    {
        return Err("Invalid Agent image metadata.".into());
    }
    let metadata = std::fs::read(metadata_path).map_err(|_| "Agent image could not be read.")?;
    if metadata.len() > 1024 {
        return Err("Invalid Agent image metadata.".into());
    }
    let record: Record =
        serde_json::from_slice(&metadata).map_err(|_| "Invalid Agent image metadata.")?;
    let image = record.image;
    if record.version != 1
        || image.id != id
        || !valid_name(&image.name)
        || image.mime_type != "image/png"
        || image.bytes == 0
        || image.bytes > IMAGE_BYTES
        || image.width == 0
        || image.width > 1024
        || image.height == 0
        || image.height > 1024
    {
        return Err("Invalid Agent image metadata.".into());
    }
    let path = directory.join(format!("{id}.png"));
    let info = std::fs::symlink_metadata(&path).map_err(|_| "Agent image could not be read.")?;
    if !info.is_file() || info.file_type().is_symlink() || info.len() != image.bytes as u64 {
        return Err("Agent image changed or is missing.".into());
    }
    let bytes = std::fs::read(path).map_err(|_| "Agent image could not be read.")?;
    if bytes.len() != image.bytes
        || format!("{:x}", Sha256::digest(&bytes)) != id
        || bytes.get(..8) != Some(b"\x89PNG\r\n\x1a\n")
        || decode(&bytes)?.dimensions() != (image.width, image.height)
    {
        return Err("Agent image changed or is missing.".into());
    }
    Ok((image, bytes))
}
pub fn ingest(home: &Path, name: &str, encoded: &str) -> Result<Attachment, String> {
    if !valid_name(name) || encoded.is_empty() || encoded.len() > UPLOAD_BYTES.div_ceil(3) * 4 {
        return Err("Choose an image up to 2 MiB with a valid filename.".into());
    }
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| "Choose a valid PNG, JPEG or WebP image.")?;
    if bytes.len() > UPLOAD_BYTES {
        return Err("Choose an image up to 2 MiB with a valid filename.".into());
    }
    let decoded = decode(&bytes)?;
    let decoded = if decoded.width() > 1024 || decoded.height() > 1024 {
        decoded.thumbnail(1024, 1024)
    } else {
        decoded
    };
    // Fresh encoding strips EXIF and ancillary upload metadata. Source files are untouched.
    let mut output = Cursor::new(Vec::new());
    decoded
        .write_to(&mut output, ImageFormat::Png)
        .map_err(|_| "Agent image preview could not be created.")?;
    let bytes = output.into_inner();
    if bytes.len() > IMAGE_BYTES {
        return Err("Agent image preview is too large.".into());
    }
    let id = format!("{:x}", Sha256::digest(&bytes));
    let directory = home.join("images");
    std::fs::create_dir_all(&directory).map_err(|_| "Agent image storage is unavailable.")?;
    let info =
        std::fs::symlink_metadata(&directory).map_err(|_| "Agent image storage is unavailable.")?;
    if !info.is_dir() || info.file_type().is_symlink() {
        return Err("Invalid Agent image storage.".into());
    }
    if directory.join(format!("{id}.json")).exists() {
        return read(home, &id).map(|(image, _)| image);
    }
    let mut total = 0;
    for entry in std::fs::read_dir(&directory).map_err(|_| "Agent image storage is unavailable.")? {
        let entry = entry.map_err(|_| "Agent image storage is unavailable.")?;
        total += entry
            .metadata()
            .map_err(|_| "Agent image storage is unavailable.")?
            .len();
        if total > STORE_BYTES {
            return Err("Agent image storage limit reached.".into());
        }
    }
    if total + bytes.len() as u64 + 1024 > STORE_BYTES {
        return Err("Agent image storage limit reached.".into());
    }
    let image = Attachment {
        id: id.clone(),
        name: name.trim().to_string(),
        mime_type: "image/png".into(),
        bytes: bytes.len(),
        width: decoded.width(),
        height: decoded.height(),
    };
    let write = |suffix: &str, bytes: &[u8]| -> Result<(), String> {
        let temporary = directory.join(format!("{id}.{suffix}.tmp"));
        std::fs::write(&temporary, bytes).map_err(|_| "Agent image could not be saved.")?;
        std::fs::rename(temporary, directory.join(format!("{id}.{suffix}")))
            .map_err(|_| "Agent image could not be saved.".into())
    };
    write("png", &bytes)?;
    write(
        "json",
        &serde_json::to_vec(&Record {
            version: 1,
            image: image.clone(),
        })
        .map_err(|_| "Agent image could not be saved.")?,
    )?;
    Ok(image)
}
pub fn preview(home: &Path, id: &str) -> Result<serde_json::Value, String> {
    let (image, bytes) = read(home, id)?;
    Ok(
        serde_json::json!({"image":image,"dataUrl":format!("data:image/png;base64,{}", STANDARD.encode(bytes))}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_storage_cleanup_keeps_conversations_drafts_and_unknown_files() {
        let home = tempfile::tempdir().unwrap();
        let make = |name, color| {
            let pixels = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
                2,
                2,
                image::Rgb(color),
            ));
            let mut png = Cursor::new(Vec::new());
            pixels.write_to(&mut png, ImageFormat::Png).unwrap();
            ingest(home.path(), name, &STANDARD.encode(png.into_inner())).unwrap()
        };
        let retained = make("conversation.png", [0, 90, 200]);
        let draft = make("draft.png", [20, 190, 0]);
        let unused = make("removed.png", [200, 0, 0]);
        let images = home.path().join("images");
        std::fs::write(home.path().join("source.png"), b"original source").unwrap();
        std::fs::write(images.join("notes.txt"), b"unknown file").unwrap();
        let keep = HashSet::from([retained.id.clone(), draft.id.clone()]);
        let before = storage(home.path(), &keep, false).unwrap();
        assert_eq!((before.image_count, before.unused_count), (3, 1));
        assert!(read(home.path(), &unused.id).is_ok());
        let cleaned = storage(home.path(), &keep, true).unwrap();
        assert_eq!(
            (
                cleaned.image_count,
                cleaned.unused_count,
                cleaned.removed_count
            ),
            (2, 0, 1)
        );
        assert_eq!(cleaned.removed_bytes, before.unused_bytes);
        assert_eq!(
            cleaned.used_bytes + cleaned.removed_bytes,
            before.used_bytes
        );
        assert_eq!(read(home.path(), &retained.id).unwrap().0, retained);
        assert_eq!(read(home.path(), &draft.id).unwrap().0, draft);
        assert!(!images.join(format!("{}.png", unused.id)).exists());
        assert!(!images.join(format!("{}.json", unused.id)).exists());
        assert_eq!(
            std::fs::read(home.path().join("source.png")).unwrap(),
            b"original source"
        );
        assert_eq!(
            std::fs::read(images.join("notes.txt")).unwrap(),
            b"unknown file"
        );
        assert_eq!(storage(home.path(), &keep, true).unwrap().removed_count, 0);
    }
    #[test]
    fn image_storage_refuses_invalid_directories_and_keeps_incomplete_pairs() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(
            storage(home.path(), &HashSet::new(), false)
                .unwrap()
                .used_bytes,
            0
        );
        std::fs::write(home.path().join("images"), b"not a directory").unwrap();
        assert!(storage(home.path(), &HashSet::new(), true).is_err());
        std::fs::remove_file(home.path().join("images")).unwrap();
        std::fs::create_dir(home.path().join("images")).unwrap();
        let path = home
            .path()
            .join("images")
            .join(format!("{}.png", "a".repeat(64)));
        std::fs::write(&path, b"not a managed image").unwrap();
        assert_eq!(
            storage(home.path(), &HashSet::new(), true)
                .unwrap()
                .removed_count,
            0
        );
        assert_eq!(std::fs::read(path).unwrap(), b"not a managed image");
    }
    fn encoded(format: ImageFormat) -> String {
        let image = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            40,
            20,
            image::Rgb([20, 90, 200]),
        ));
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, format).unwrap();
        STANDARD.encode(bytes.into_inner())
    }
    #[test]
    fn persistent_normalized_image_deduplicates_and_detects_changed_files() {
        let home = tempfile::tempdir().unwrap();
        let image = ingest(home.path(), "map.jpg", &encoded(ImageFormat::Jpeg)).unwrap();
        assert_eq!(image.mime_type, "image/png");
        assert_eq!((image.width, image.height), (40, 20));
        assert_eq!(
            ingest(home.path(), "other.jpg", &encoded(ImageFormat::Jpeg)).unwrap(),
            image
        );
        assert_eq!(read(home.path(), &image.id).unwrap().0, image);
        std::fs::write(
            home.path().join("images").join(format!("{}.png", image.id)),
            b"changed",
        )
        .unwrap();
        assert!(preview(home.path(), &image.id).is_err());
    }
    #[test]
    fn rejects_paths_unsupported_data_oversize_and_truncation() {
        let home = tempfile::tempdir().unwrap();
        assert!(ingest(home.path(), "../image.png", &encoded(ImageFormat::Png)).is_err());
        assert!(ingest(home.path(), "image.svg", &STANDARD.encode(b"<svg/>")).is_err());
        assert!(ingest(home.path(), "image.png", &"A".repeat(UPLOAD_BYTES * 2)).is_err());
        let mut truncated = STANDARD.decode(encoded(ImageFormat::Png)).unwrap();
        truncated.truncate(30);
        assert!(ingest(home.path(), "image.png", &STANDARD.encode(truncated)).is_err());
        assert!(read(home.path(), "../../outside").is_err());
    }
    #[test]
    fn resizes_and_accepts_all_three_formats_with_decoder_limits() {
        let home = tempfile::tempdir().unwrap();
        for format in [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::WebP] {
            ingest(home.path(), "image", &encoded(format)).unwrap();
        }
        let image = image::DynamicImage::new_rgb8(2048, 1024);
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Png).unwrap();
        let resized = ingest(
            home.path(),
            "large.png",
            &STANDARD.encode(bytes.into_inner()),
        )
        .unwrap();
        assert_eq!((resized.width, resized.height), (1024, 512));
        let image = image::DynamicImage::new_rgb8(4097, 1);
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Png).unwrap();
        assert!(ingest(
            home.path(),
            "too-wide.png",
            &STANDARD.encode(bytes.into_inner())
        )
        .is_err());
    }
    #[test]
    fn preserves_display_orientation_when_stripping_exif() {
        let home = tempfile::tempdir().unwrap();
        let original = STANDARD.decode(encoded(ImageFormat::Jpeg)).unwrap();
        let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        let mut rotated = vec![255, 216, 255, 225];
        rotated.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        rotated.extend_from_slice(exif);
        rotated.extend_from_slice(&original[2..]);
        let saved = ingest(home.path(), "oriented.jpg", &STANDARD.encode(rotated)).unwrap();
        assert_eq!((saved.width, saved.height), (20, 40));
        assert!(!read(home.path(), &saved.id)
            .unwrap()
            .1
            .windows(6)
            .any(|bytes| bytes == b"Exif\0\0"));
    }
}
