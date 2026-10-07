//! Bounded files selected explicitly in the renderer. Owned copies only.
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[path = "agent_audio.rs"]
mod audio;
#[path = "agent_office.rs"]
mod office;
#[path = "agent_video.rs"]
mod video;
use std::{collections::HashSet, path::Path};

const DOCUMENT_BYTES: usize = 64 * 1024;
const PDF_BYTES: usize = 2 * 1024 * 1024;
const STORE_BYTES: u64 = 32 * 1024 * 1024;
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Document {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    pub bytes: usize,
    pub characters: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages: Option<usize>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u8,
    document: Document,
}
fn valid_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name.chars().count() <= 80
        && !name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\'))
        && name.rsplit_once('.').is_some_and(|(_, extension)| {
            [
                "txt", "md", "csv", "json", "geojson", "pdf", "docx", "xlsx", "pptx", "wav", "mp3",
                "flac", "ogg", "mp4", "webm",
            ]
            .contains(&extension.to_ascii_lowercase().as_str())
        })
}
fn text(bytes: &[u8]) -> Result<&str, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "Choose a UTF-8 text document.")?;
    if text
        .trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
        .is_empty()
        || text
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\t' | '\n' | '\r'))
    {
        return Err("Choose a UTF-8 text document.".into());
    }
    Ok(text)
}
fn pdf_pages(bytes: &[u8]) -> Result<usize, String> {
    if bytes.len() > PDF_BYTES || !bytes.starts_with(b"%PDF-") {
        return Err("Choose a valid PDF up to 2 MiB.".into());
    }
    let pdf = lopdf::Document::load_mem_with_options(
        bytes,
        lopdf::LoadOptions {
            strict: true,
            max_decompressed_size: Some(8 * 1024 * 1024),
            ..Default::default()
        },
    )
    .map_err(|_| "Choose a valid, unencrypted PDF.")?;
    if pdf.is_encrypted() || pdf.encryption_state.is_some() {
        return Err("Choose a valid, unencrypted PDF.".into());
    }
    let count = pdf.page_iter().take(21).count();
    if count == 0 || count > 20 {
        return Err("Choose a PDF with 1 to 20 pages.".into());
    }
    Ok(count)
}
fn document_id(bytes: &[u8], pages: Option<usize>, mime: &str) -> String {
    let mut hash = Sha256::new();
    if let Some(pages) = pages {
        hash.update(format!("application/pdf;pages={pages}\0").as_bytes());
    } else if office::extension(mime).is_some()
        || audio::extension(mime).is_some()
        || video::extension(mime).is_some()
    {
        hash.update(format!("{mime}\0").as_bytes());
    }
    hash.update(bytes);
    format!("{:x}", hash.finalize())
}
fn suffix(document: &Document) -> &'static str {
    if document.mime_type == "application/pdf" {
        "pdf"
    } else if let Some(extension) = office::extension(&document.mime_type) {
        extension
    } else if let Some(extension) = audio::extension(&document.mime_type) {
        extension
    } else if let Some(extension) = video::extension(&document.mime_type) {
        extension
    } else {
        "txt"
    }
}
fn directory(home: &Path) -> Result<std::path::PathBuf, String> {
    let path = home.join("documents");
    let info =
        std::fs::symlink_metadata(&path).map_err(|_| "Agent document storage is unavailable.")?;
    if !info.is_dir() || info.file_type().is_symlink() {
        return Err("Invalid Agent document storage.".into());
    }
    Ok(path)
}
pub fn read(home: &Path, id: &str) -> Result<(Document, Vec<u8>), String> {
    read_with_media(home, id).map(|(document, bytes, _)| (document, bytes))
}
enum MediaPreview {
    Audio(audio::AudioInfo),
    Video(video::VideoInfo),
}
fn read_with_media(
    home: &Path,
    id: &str,
) -> Result<(Document, Vec<u8>, Option<MediaPreview>), String> {
    if !super::images::valid_id(id) {
        return Err("Invalid Agent document reference.".into());
    }
    let directory = directory(home)?;
    let metadata_path = directory.join(format!("{id}.json"));
    let info = std::fs::symlink_metadata(&metadata_path)
        .map_err(|_| "Agent document could not be read.")?;
    if !info.is_file() || info.file_type().is_symlink() || info.len() > 1024 {
        return Err("Invalid Agent document metadata.".into());
    }
    let record: Record = serde_json::from_slice(
        &std::fs::read(metadata_path).map_err(|_| "Agent document could not be read.")?,
    )
    .map_err(|_| "Invalid Agent document metadata.")?;
    let document = record.document;
    let is_pdf = document.mime_type == "application/pdf";
    let office_extension = office::extension(&document.mime_type);
    let audio_extension = audio::extension(&document.mime_type);
    let video_extension = video::extension(&document.mime_type);
    if record.version != 1
        || document.id != id
        || !valid_name(&document.name)
        || document.bytes == 0
        || if is_pdf {
            document.bytes > PDF_BYTES
                || document.characters != 0
                || !document.name.to_ascii_lowercase().ends_with(".pdf")
                || !document
                    .pages
                    .is_some_and(|pages| (1..=20).contains(&pages))
        } else if let Some(extension) = office_extension.or(audio_extension).or(video_extension) {
            document.bytes
                > if office_extension.is_some() {
                    office::OFFICE_BYTES
                } else if audio_extension.is_some() {
                    audio::AUDIO_BYTES
                } else {
                    video::VIDEO_BYTES
                }
                || document.pages.is_some()
                || document.characters != 0
                || !document
                    .name
                    .to_ascii_lowercase()
                    .ends_with(&format!(".{extension}"))
        } else {
            document.mime_type != "text/plain"
                || document.bytes > DOCUMENT_BYTES
                || document.characters == 0
                || document.characters > document.bytes
                || document.pages.is_some()
                || !document.name.rsplit_once('.').is_some_and(|(_, ext)| {
                    ["txt", "md", "csv", "json", "geojson"]
                        .contains(&ext.to_ascii_lowercase().as_str())
                })
        }
    {
        return Err("Invalid Agent document metadata.".into());
    }
    let path = directory.join(format!("{id}.{}", suffix(&document)));
    let info =
        std::fs::symlink_metadata(&path).map_err(|_| "Agent document changed or is missing.")?;
    if !info.is_file() || info.file_type().is_symlink() || info.len() != document.bytes as u64 {
        return Err("Agent document changed or is missing.".into());
    }
    let bytes = std::fs::read(path).map_err(|_| "Agent document could not be read.")?;
    let media_info = if let Some(extension) = audio_extension {
        Some(MediaPreview::Audio(audio::inspect(&bytes, extension)?))
    } else if let Some(extension) = video_extension {
        Some(MediaPreview::Video(video::inspect(&bytes, extension)?))
    } else {
        None
    };
    let characters = if is_pdf {
        if Some(pdf_pages(&bytes)?) != document.pages {
            return Err("Agent document changed or is missing.".into());
        }
        0
    } else if let Some(extension) = office_extension {
        office::preview(&bytes, extension)?;
        0
    } else if audio_extension.is_some() || video_extension.is_some() {
        0
    } else {
        text(&bytes)?.chars().count()
    };
    if bytes.len() != document.bytes
        || document_id(&bytes, document.pages, &document.mime_type) != id
        || characters != document.characters
    {
        return Err("Agent document changed or is missing.".into());
    }
    Ok((document, bytes, media_info))
}
pub fn ingest(home: &Path, name: &str, encoded: &str) -> Result<Document, String> {
    let is_pdf = name.to_ascii_lowercase().ends_with(".pdf");
    let extension = name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default();
    let office_mime = office::mime(&extension);
    let audio_mime = audio::mime(&extension);
    let video_mime = video::mime(&extension);
    let limit = if video_mime.is_some() {
        video::VIDEO_BYTES
    } else if is_pdf || office_mime.is_some() || audio_mime.is_some() {
        PDF_BYTES
    } else {
        DOCUMENT_BYTES
    };
    let limit_error = if is_pdf {
        "Choose a valid PDF up to 2 MiB."
    } else if office_mime.is_some() {
        "Choose DOCX, XLSX or PPTX files up to 2 MiB each."
    } else if audio_mime.is_some() {
        "Choose an audio clip up to 2 MiB and 10 minutes."
    } else if video_mime.is_some() {
        "Choose a video up to 8 MiB, 10 minutes and 4096 pixels per side."
    } else {
        "Choose TXT, Markdown, CSV, JSON or GeoJSON files up to 64 KiB each."
    };
    if !valid_name(name) || encoded.is_empty() || encoded.len() > limit.div_ceil(3) * 4 {
        return Err(limit_error.into());
    }
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| "File could not be read.")?;
    if bytes.is_empty() || bytes.len() > limit {
        return Err(limit_error.into());
    }
    let pages = if is_pdf {
        Some(pdf_pages(&bytes)?)
    } else {
        None
    };
    let characters = if is_pdf {
        0
    } else if office_mime.is_some() {
        office::preview(&bytes, &extension)?;
        0
    } else if audio_mime.is_some() {
        audio::inspect(&bytes, &extension)?;
        0
    } else if video_mime.is_some() {
        video::inspect(&bytes, &extension)?;
        0
    } else {
        text(&bytes)?.chars().count()
    };
    let mime = if is_pdf {
        "application/pdf"
    } else {
        office_mime
            .or(audio_mime)
            .or(video_mime)
            .unwrap_or("text/plain")
    };
    let document = Document {
        id: document_id(&bytes, pages, mime),
        name: name.trim().into(),
        mime_type: mime.into(),
        bytes: bytes.len(),
        characters,
        pages,
    };
    std::fs::create_dir_all(home.join("documents"))
        .map_err(|_| "Agent document storage is unavailable.")?;
    let directory = directory(home)?;
    if directory.join(format!("{}.json", document.id)).exists() {
        return read(home, &document.id).map(|(document, _)| document);
    }
    let mut total = 0_u64;
    for entry in
        std::fs::read_dir(&directory).map_err(|_| "Agent document storage is unavailable.")?
    {
        let info = entry
            .map_err(|_| "Agent document storage is unavailable.")?
            .metadata()
            .map_err(|_| "Agent document storage is unavailable.")?;
        total = total.saturating_add(info.len());
    }
    if total.saturating_add(bytes.len() as u64 + 1024) > STORE_BYTES {
        return Err("Agent document storage limit reached.".into());
    }
    let write = |suffix: &str, bytes: &[u8]| -> Result<(), String> {
        let temporary = directory.join(format!("{}.{suffix}.tmp", document.id));
        std::fs::write(&temporary, bytes).map_err(|_| "Agent document could not be saved.")?;
        std::fs::rename(
            temporary,
            directory.join(format!("{}.{suffix}", document.id)),
        )
        .map_err(|_| "Agent document could not be saved.".into())
    };
    write(suffix(&document), &bytes)?;
    write(
        "json",
        &serde_json::to_vec(&Record {
            version: 1,
            document: document.clone(),
        })
        .map_err(|_| "Agent document could not be saved.")?,
    )?;
    Ok(document)
}
pub fn preview(home: &Path, id: &str) -> Result<serde_json::Value, String> {
    let (document, bytes, media_info) = read_with_media(home, id)?;
    if let Some(info) = media_info {
        let mut preview = serde_json::json!({"document":document,"dataUrl":format!("data:{};base64,{}",document.mime_type,STANDARD.encode(bytes))});
        match info {
            MediaPreview::Audio(info) => {
                preview["audio"] =
                    serde_json::to_value(info).map_err(|_| "Invalid audio preview.")?
            }
            MediaPreview::Video(info) => {
                preview["video"] =
                    serde_json::to_value(info).map_err(|_| "Invalid video preview.")?
            }
        };
        return Ok(preview);
    }
    if document.mime_type == "application/pdf" {
        return Ok(
            serde_json::json!({"document":document,"dataUrl":format!("data:application/pdf;base64,{}", STANDARD.encode(bytes))}),
        );
    }
    if let Some(extension) = office::extension(&document.mime_type) {
        let text = office::preview(&bytes, extension)?;
        return Ok(
            serde_json::json!({"document":document,"previewCharacters":text.chars().count(),"text":text}),
        );
    }
    Ok(serde_json::json!({"document": document, "text": text(&bytes)?}))
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Storage {
    limit_bytes: u64,
    used_bytes: u64,
    document_count: usize,
    unused_bytes: u64,
    unused_count: usize,
    removed_bytes: u64,
    removed_count: usize,
}
pub fn storage(home: &Path, keep: &HashSet<String>, cleanup: bool) -> Result<Storage, String> {
    let mut result = Storage {
        limit_bytes: STORE_BYTES,
        ..Storage::default()
    };
    if !home
        .join("documents")
        .try_exists()
        .map_err(|_| "Agent document storage is unavailable.")?
    {
        return Ok(result);
    }
    let directory = directory(home)?;
    let mut unused = Vec::new();
    for entry in
        std::fs::read_dir(&directory).map_err(|_| "Agent document storage is unavailable.")?
    {
        let entry = entry.map_err(|_| "Agent document storage is unavailable.")?;
        let info = std::fs::symlink_metadata(entry.path())
            .map_err(|_| "Agent document storage is unavailable.")?;
        if !info.is_file() || info.file_type().is_symlink() {
            continue;
        }
        result.used_bytes = result.used_bytes.saturating_add(info.len());
        let name = entry.file_name();
        let Some(id) = name
            .to_str()
            .and_then(|name| name.strip_suffix(".json"))
            .filter(|id| super::images::valid_id(id))
        else {
            continue;
        };
        let Ok((document, _)) = read(home, id) else {
            continue;
        };
        result.document_count += 1;
        if keep.contains(id) {
            continue;
        }
        result.unused_count += 1;
        result.unused_bytes += document.bytes as u64 + info.len();
        unused.push(id.to_owned());
    }
    if cleanup {
        for id in unused {
            let (document, _) = read(home, &id)?;
            let metadata = directory.join(format!("{id}.json"));
            let bytes = document.bytes as u64
                + std::fs::symlink_metadata(&metadata)
                    .map_err(|_| "Agent documents could not be cleaned.")?
                    .len();
            std::fs::remove_file(directory.join(format!("{id}.{}", suffix(&document))))
                .map_err(|_| "Agent documents could not be cleaned.")?;
            std::fs::remove_file(metadata).map_err(|_| "Agent documents could not be cleaned.")?;
            result.removed_count += 1;
            result.removed_bytes += bytes;
        }
        result.used_bytes = result.used_bytes.saturating_sub(result.removed_bytes);
        result.document_count = result.document_count.saturating_sub(result.removed_count);
        result.unused_count = result.unused_count.saturating_sub(result.removed_count);
        result.unused_bytes = result.unused_bytes.saturating_sub(result.removed_bytes);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn video_documents_preserve_packets_preview_metadata_and_reference_protection() {
        let home = tempfile::tempdir().unwrap();
        let fixtures: [(&str, &[u8]); 3] = [
            ("clip.mp4", include_bytes!("../../agent/fixtures/clip.mp4")),
            (
                "clip.webm",
                include_bytes!("../../agent/fixtures/clip.webm"),
            ),
            (
                "clip-vp8.webm",
                include_bytes!("../../agent/fixtures/clip-vp8.webm"),
            ),
        ];
        let mut keep = HashSet::new();
        for (name, bytes) in fixtures {
            let document = ingest(home.path(), name, &STANDARD.encode(bytes)).unwrap();
            assert_eq!(
                document.mime_type,
                video::mime(name.rsplit_once('.').unwrap().1).unwrap()
            );
            assert_eq!(document.characters, 0);
            assert_eq!(document.pages, None);
            assert_eq!(read(home.path(), &document.id).unwrap().1, bytes);
            let preview = preview(home.path(), &document.id).unwrap();
            let info = &preview["video"];
            assert!(
                (2900..=3200).contains(&info["durationMs"].as_u64().unwrap()),
                "{name}: {info}"
            );
            assert_eq!(info["width"], 320);
            assert_eq!(info["height"], 180);
            assert_eq!(info["hasAudio"], true);
            assert_eq!(
                preview["dataUrl"],
                format!(
                    "data:{};base64,{}",
                    document.mime_type,
                    STANDARD.encode(bytes)
                )
            );
            assert_eq!(
                ingest(home.path(), name, &STANDARD.encode(bytes)).unwrap(),
                document
            );
            if let Ok(export) = std::env::var("GEOD_AGENT_VIDEO_FIXTURE") {
                let path = Path::new(&export);
                let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap()
                    .join(".verification");
                assert!(path.is_absolute() && path.starts_with(root));
                std::fs::create_dir_all(path).unwrap();
                std::fs::write(
                    path.join(format!("{name}.json")),
                    serde_json::to_vec(&serde_json::json!({"document":document,"video":info}))
                        .unwrap(),
                )
                .unwrap();
            }
            keep.insert(document.id);
        }
        assert_eq!(storage(home.path(), &keep, true).unwrap().removed_count, 0);
        assert!(ingest(
            home.path(),
            "bad.mp4",
            &STANDARD.encode(&fixtures[0].1[..fixtures[0].1.len() - 100])
        )
        .is_err());
        assert!(ingest(
            home.path(),
            "bad.webm",
            &STANDARD.encode(&fixtures[1].1[..fixtures[1].1.len() - 100])
        )
        .is_err());
        assert!(ingest(
            home.path(),
            "audio.mp4",
            &STANDARD.encode(include_bytes!("../../agent/fixtures/tone.mp3"))
        )
        .is_err());
        assert!(ingest(
            home.path(),
            "unsupported.mp4",
            &STANDARD.encode(include_bytes!("../../agent/fixtures/clip-unsupported.mp4"))
        )
        .unwrap_err()
        .contains("H.264"));
        assert!(ingest(
            home.path(),
            "too-long.mp4",
            &STANDARD.encode(include_bytes!("../../agent/fixtures/clip-too-long.mp4"))
        )
        .unwrap_err()
        .contains("10 minutes"));
        assert!(ingest(
            home.path(),
            "oversized.mp4",
            &STANDARD.encode(vec![0; video::VIDEO_BYTES + 1])
        )
        .is_err());
        // A huge table declaration and an unknown-size EBML element are rejected
        // before the demuxer can allocate a size claimed by the file.
        assert!(video::inspect(
            b"\0\0\0\x10ftypisom\0\0\0\0\0\0\0\x14stts\0\0\0\0\xff\xff\xff\xff\0\0\0\0",
            "mp4"
        )
        .is_err());
        assert!(video::inspect(&[0x1a, 0x45, 0xdf, 0xa3, 0xff], "webm").is_err());
        assert_eq!(
            storage(home.path(), &HashSet::new(), true)
                .unwrap()
                .removed_count,
            3
        );
    }
    #[test]
    fn audio_documents_decode_originals_preview_duration_and_protect_references() {
        let home = tempfile::tempdir().unwrap();
        let fixtures: [(&str, &[u8]); 4] = [
            ("tone.wav", include_bytes!("../../agent/fixtures/tone.wav")),
            ("tone.mp3", include_bytes!("../../agent/fixtures/tone.mp3")),
            (
                "tone.flac",
                include_bytes!("../../agent/fixtures/tone.flac"),
            ),
            ("tone.ogg", include_bytes!("../../agent/fixtures/tone.ogg")),
        ];
        let mut protected = HashSet::new();
        for (name, bytes) in fixtures {
            let document = ingest(home.path(), name, &STANDARD.encode(bytes)).unwrap();
            assert_eq!(
                document.mime_type,
                audio::mime(name.rsplit_once('.').unwrap().1).unwrap()
            );
            assert_eq!(document.characters, 0);
            assert_eq!(document.pages, None);
            assert_eq!(read(home.path(), &document.id).unwrap().1, bytes);
            assert_eq!(
                ingest(home.path(), name, &STANDARD.encode(bytes)).unwrap(),
                document
            );
            let preview = preview(home.path(), &document.id).unwrap();
            assert_eq!(
                preview["dataUrl"],
                format!(
                    "data:{};base64,{}",
                    document.mime_type,
                    STANDARD.encode(bytes)
                )
            );
            let info = &preview["audio"];
            assert!((2900..=3200).contains(&info["durationMs"].as_u64().unwrap()));
            assert_eq!(info["sampleRate"], 16000);
            assert_eq!(info["channels"], 1);
            if let Ok(export) = std::env::var("GEOD_AGENT_AUDIO_FIXTURE") {
                let path = Path::new(&export);
                let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap()
                    .join(".verification");
                assert!(path.is_absolute() && path.starts_with(root));
                std::fs::create_dir_all(path).unwrap();
                std::fs::write(
                    path.join(format!("{name}.json")),
                    serde_json::to_vec(&serde_json::json!({"document":document,"audio":info}))
                        .unwrap(),
                )
                .unwrap();
            }
            protected.insert(document.id);
        }
        assert_eq!(
            storage(home.path(), &protected, true)
                .unwrap()
                .removed_count,
            0
        );
        for (name, bytes) in [
            ("fake.wav", b"RIFFxxxxWAVE".as_slice()),
            ("wrong.flac", fixtures[0].1),
            (
                "wrong.ogg",
                include_bytes!("../../agent/fixtures/tone-wrong-codec.ogg").as_slice(),
            ),
        ] {
            assert!(ingest(home.path(), name, &STANDARD.encode(bytes))
                .unwrap_err()
                .contains("valid WAV"));
        }
        assert!(ingest(
            home.path(),
            "oversized.wav",
            &STANDARD.encode(vec![0; audio::AUDIO_BYTES + 1])
        )
        .is_err());
        assert!(ingest(
            home.path(),
            "too-long.flac",
            &STANDARD.encode(include_bytes!("../../agent/fixtures/tone-too-long.flac"))
        )
        .unwrap_err()
        .contains("10 minutes"));
        let first = &fixtures[0];
        let id = document_id(first.1, None, "audio/wav");
        let mut changed = first.1.to_vec();
        *changed.last_mut().unwrap() ^= 1;
        std::fs::write(
            home.path().join("documents").join(format!("{id}.wav")),
            changed,
        )
        .unwrap();
        assert!(read(home.path(), &id).is_err());
        // Corrupt files are preserved for diagnosis; only valid unreferenced
        // owned copies are removed, and source fixture files remain unchanged.
        assert_eq!(
            storage(home.path(), &HashSet::new(), true)
                .unwrap()
                .removed_count,
            3
        );
        assert_eq!(
            std::fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap()
                    .join("agent/fixtures/tone.wav")
            )
            .unwrap(),
            first.1
        );
    }
    #[test]
    fn office_documents_preserve_originals_preview_content_order_and_cleanup_protection() {
        let home = tempfile::tempdir().unwrap();
        let fixtures: [(&str, &[u8]); 3] = [
            (
                "notes.docx",
                include_bytes!("../../agent/fixtures/notes.docx"),
            ),
            (
                "data.xlsx",
                include_bytes!("../../agent/fixtures/data.xlsx"),
            ),
            (
                "slides.pptx",
                include_bytes!("../../agent/fixtures/slides.pptx"),
            ),
        ];
        let mut protected = Vec::new();
        for (name, source) in fixtures {
            let document = ingest(home.path(), name, &STANDARD.encode(source)).unwrap();
            let extension = name.rsplit_once('.').unwrap().1;
            assert_eq!(document.mime_type, office::mime(extension).unwrap());
            assert_eq!(document.pages, None);
            assert_eq!(read(home.path(), &document.id).unwrap().1, source);
            let value = preview(home.path(), &document.id).unwrap();
            let text = value["text"].as_str().unwrap();
            assert_eq!(document.characters, 0);
            assert_eq!(
                value["previewCharacters"].as_u64().unwrap() as usize,
                text.chars().count()
            );
            match extension {
                "docx" => {
                    assert!(text.contains("北京 🌍 & <GeoD>"));
                    assert!(text.contains("<script>window.officeInjected=true</script>"));
                    assert!(text.contains("GeoD header"));
                    assert!(text.contains("城市\t经度\n北京\t116.4\n"));
                }
                "xlsx" => {
                    assert!(text.contains("[数据]\nA1\t城市"));
                    assert!(text.contains("B2\t1.25"));
                    assert!(text.contains("[formula: SUM(B2:B3)]"));
                    assert!(text.contains("[说明]"));
                }
                "pptx" => {
                    assert!(
                        text.find("第一张 & GeoD").unwrap()
                            < text.find("Second in presentation order").unwrap()
                    );
                }
                _ => unreachable!(),
            }
            if let Ok(export) = std::env::var("GEOD_AGENT_OFFICE_FIXTURE") {
                let path = Path::new(&export);
                let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap()
                    .join(".verification");
                assert!(path.is_absolute() && path.starts_with(root));
                std::fs::create_dir_all(path).unwrap();
                std::fs::write(
                    path.join(format!("{name}.json")),
                    serde_json::to_vec(&value).unwrap(),
                )
                .unwrap();
            }
            protected.push(document.id);
        }
        assert_eq!(
            storage(home.path(), &protected.iter().cloned().collect(), true)
                .unwrap()
                .removed_count,
            0
        );
        assert!(ingest(home.path(), "invalid.docx", &STANDARD.encode(b"not a zip")).is_err());
        let source = fixtures[0].1;
        let modify = |part: &str, replacement: &[u8]| {
            use std::io::{Read, Write};
            let mut input = zip::ZipArchive::new(std::io::Cursor::new(source)).unwrap();
            let mut output = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
            for index in 0..input.len() {
                let mut file = input.by_index(index).unwrap();
                let name = file.name().to_owned();
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes).unwrap();
                output
                    .start_file(
                        &name,
                        zip::write::SimpleFileOptions::default()
                            .compression_method(zip::CompressionMethod::Deflated),
                    )
                    .unwrap();
                output
                    .write_all(if name == part { replacement } else { &bytes })
                    .unwrap();
            }
            output.finish().unwrap().into_inner()
        };
        let entity=modify("word/document.xml",b"<!DOCTYPE document [<!ENTITY other SYSTEM 'file:///private'>]><document><t>&other;</t></document>");
        assert!(ingest(home.path(), "entity.docx", &STANDARD.encode(entity)).is_err());
        let oversized = modify("word/document.xml", &vec![b' '; 4 * 1024 * 1024 + 1]);
        assert!(ingest(home.path(), "large.docx", &STANDARD.encode(oversized)).is_err());
        let macro_package=modify("[Content_Types].xml",b"<Types><Default Extension='bin' ContentType='application/vnd.ms-office.vbaProject'/><Override PartName='/word/document.xml' ContentType='application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml'/></Types>");
        assert!(ingest(home.path(), "macro.docx", &STANDARD.encode(macro_package)).is_err());
        let changed = home
            .path()
            .join("documents")
            .join(format!("{}.docx", protected[0]));
        std::fs::write(&changed, b"changed").unwrap();
        assert!(read(home.path(), &protected[0]).is_err());
        // Invalid copies are not silently counted or deleted by cleanup.
        let cleaned = storage(home.path(), &HashSet::new(), true).unwrap();
        assert_eq!(cleaned.removed_count, 2);
        assert!(changed.exists());
    }
    fn sample_pdf() -> Vec<u8> {
        use lopdf::{
            content::{Content, Operation},
            dictionary, Object, Stream,
        };
        let mut pdf = lopdf::Document::with_version("1.5");
        let pages = pdf.new_object_id();
        let font =
            pdf.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
        let resources = pdf.add_object(dictionary! {"Font"=>dictionary!{"F1"=>font}});
        let mut kids = Vec::new();
        for number in 1..=2 {
            let content = Content {
                operations: vec![
                    Operation::new("BT", vec![]),
                    Operation::new("Tf", vec!["F1".into(), 18.into()]),
                    Operation::new("Td", vec![50.into(), 760.into()]),
                    Operation::new(
                        "Tj",
                        vec![Object::string_literal(format!(
                            "GeoD PDF check - page {number}"
                        ))],
                    ),
                    Operation::new("ET", vec![]),
                ],
            };
            let stream = pdf.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
            let page = pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"Contents"=>stream,"Resources"=>resources,"MediaBox"=>vec![0.into(),0.into(),595.into(),842.into()]});
            kids.push(page.into());
        }
        pdf.objects.insert(
            pages,
            dictionary! {"Type"=>"Pages","Kids"=>kids,"Count"=>2}.into(),
        );
        let catalog = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
        pdf.trailer.set("Root", catalog);
        let mut bytes = Vec::new();
        pdf.save_to(&mut bytes).unwrap();
        bytes
    }
    #[test]
    fn pdf_ingestion_preserves_bytes_pages_and_cleanup_protects_references() {
        let home = tempfile::tempdir().unwrap();
        let bytes = sample_pdf();
        let document = ingest(home.path(), "sample.pdf", &STANDARD.encode(&bytes)).unwrap();
        assert_eq!(document.mime_type, "application/pdf");
        assert_eq!(document.pages, Some(2));
        assert_eq!(document.characters, 0);
        assert_eq!(
            read(home.path(), &document.id).unwrap(),
            (document.clone(), bytes.clone())
        );
        assert_eq!(
            storage(home.path(), &HashSet::from([document.id.clone()]), true)
                .unwrap()
                .removed_count,
            0
        );
        assert!(ingest(
            home.path(),
            "damaged.pdf",
            &STANDARD.encode(b"%PDF-1.5\nnot valid")
        )
        .is_err());
        let metadata = home
            .path()
            .join("documents")
            .join(format!("{}.json", document.id));
        let mut forged = document.clone();
        forged.pages = Some(3);
        std::fs::write(
            &metadata,
            serde_json::to_vec(&Record {
                version: 1,
                document: forged,
            })
            .unwrap(),
        )
        .unwrap();
        assert!(read(home.path(), &document.id).is_err());
        std::fs::write(
            &metadata,
            serde_json::to_vec(&Record {
                version: 1,
                document: document.clone(),
            })
            .unwrap(),
        )
        .unwrap();
        if let Ok(value) = std::env::var("GEOD_AGENT_PDF_FIXTURE") {
            let directory = std::path::PathBuf::from(value);
            let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
            assert!(
                directory.is_absolute()
                    && directory.starts_with(root.join(".verification"))
                    && !directory.exists()
            );
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join("sample.pdf"), &bytes).unwrap();
            std::fs::write(
                directory.join("metadata.json"),
                serde_json::to_vec(&document).unwrap(),
            )
            .unwrap();
        }
        assert_eq!(
            storage(home.path(), &HashSet::new(), true)
                .unwrap()
                .removed_count,
            1
        );
        assert!(!home
            .path()
            .join("documents")
            .join(format!("{}.pdf", document.id))
            .exists());
    }
    #[test]
    fn documents_roundtrip_preserves_utf8_and_detects_forged_or_changed_files() {
        let home = tempfile::tempdir().unwrap();
        let source = "\u{feff}名称,经度,纬度\r\n北京,116.4,39.9 🌍\r\n";
        let document =
            ingest(home.path(), "坐标.csv", &STANDARD.encode(source.as_bytes())).unwrap();
        assert_eq!(
            read(home.path(), &document.id).unwrap(),
            (document.clone(), source.as_bytes().to_vec())
        );
        assert_eq!(document.characters, source.chars().count());
        assert_eq!(
            document.id,
            format!("{:x}", Sha256::digest(source.as_bytes()))
        );
        assert_eq!(
            ingest(home.path(), "copy.csv", &STANDARD.encode(source)).unwrap(),
            document
        );
        assert!(ingest(home.path(), "../bad.txt", &STANDARD.encode("safe text")).is_err());
        assert!(ingest(home.path(), "binary.pdf", &STANDARD.encode("safe text")).is_err());
        assert!(ingest(home.path(), "bad.txt", &STANDARD.encode([255, 254, 0])).is_err());
        assert!(ingest(home.path(), "bad.txt", &STANDARD.encode("\u{feff}  ")).is_err());
        assert!(ingest(
            home.path(),
            "big.txt",
            &STANDARD.encode(vec![b'x'; DOCUMENT_BYTES + 1])
        )
        .is_err());
        let metadata = home
            .path()
            .join("documents")
            .join(format!("{}.json", document.id));
        std::fs::write(
            &metadata,
            serde_json::to_vec(
                &serde_json::json!({"version":1,"document":document,"path":"outside"}),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(read(home.path(), &document.id).is_err());
        std::fs::write(
            &metadata,
            serde_json::to_vec(&Record {
                version: 1,
                document: document.clone(),
            })
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            home.path()
                .join("documents")
                .join(format!("{}.txt", document.id)),
            vec![b'x'; source.len()],
        )
        .unwrap();
        assert!(preview(home.path(), &document.id).is_err());
    }
    #[test]
    fn documents_cleanup_retains_conversations_drafts_and_original_sources() {
        let home = tempfile::tempdir().unwrap();
        let saved = ingest(
            home.path(),
            "saved.md",
            &STANDARD.encode("Saved conversation document"),
        )
        .unwrap();
        let draft = ingest(
            home.path(),
            "draft.json",
            &STANDARD.encode("{\"draft\":true}"),
        )
        .unwrap();
        let unused = ingest(
            home.path(),
            "removed.txt",
            &STANDARD.encode("Unused document"),
        )
        .unwrap();
        std::fs::write(home.path().join("source.csv"), b"Original source").unwrap();
        std::fs::write(
            home.path().join("documents").join("notes.txt"),
            b"Unknown file",
        )
        .unwrap();
        let keep = HashSet::from([saved.id.clone(), draft.id.clone()]);
        let before = storage(home.path(), &keep, false).unwrap();
        assert_eq!((before.document_count, before.unused_count), (3, 1));
        let after = storage(home.path(), &keep, true).unwrap();
        assert_eq!(
            (
                after.document_count,
                after.unused_count,
                after.removed_count
            ),
            (2, 0, 1)
        );
        assert_eq!(after.used_bytes + after.removed_bytes, before.used_bytes);
        assert!(read(home.path(), &saved.id).is_ok());
        assert!(read(home.path(), &draft.id).is_ok());
        assert!(!home
            .path()
            .join("documents")
            .join(format!("{}.txt", unused.id))
            .exists());
        assert_eq!(
            std::fs::read(home.path().join("source.csv")).unwrap(),
            b"Original source"
        );
        assert_eq!(
            std::fs::read(home.path().join("documents").join("notes.txt")).unwrap(),
            b"Unknown file"
        );
    }
}
