//! Bounded, read-only OOXML content preview. Never extract archive paths,
//! follow external relationships, run formulas, macros or document actions.
use quick_xml::{events::Event, Reader};
use std::{
    collections::HashMap,
    io::{Cursor, Read},
};

pub const OFFICE_BYTES: usize = 2 * 1024 * 1024;
pub const PREVIEW_BYTES: usize = 256 * 1024;
const INVALID: &str = "Choose a valid, unencrypted DOCX, XLSX or PPTX file.";
const LIMIT: &str = "Office document content exceeds the local preview limit.";
pub fn mime(extension: &str) -> Option<&'static str> {
    match extension {
        "docx" => Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
        "xlsx" => Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        "pptx" => Some("application/vnd.openxmlformats-officedocument.presentationml.presentation"),
        _ => None,
    }
}
pub fn extension(media_type: &str) -> Option<&'static str> {
    ["docx", "xlsx", "pptx"]
        .into_iter()
        .find(|name| mime(name) == Some(media_type))
}
enum Token {
    Start(String, HashMap<String, String>),
    End(String),
    Text(String),
}
fn local(value: &str) -> Result<String, String> {
    Ok(value.rsplit(':').next().ok_or(INVALID)?.to_owned())
}
fn xml(bytes: &[u8]) -> Result<Vec<Token>, String> {
    let decoded;
    let text = if bytes.starts_with(b"\xff\xfe") || bytes.starts_with(b"\xfe\xff") {
        if !bytes.len().is_multiple_of(2) {
            return Err(INVALID.into());
        }
        let little = bytes.starts_with(b"\xff\xfe");
        let units: Vec<_> = bytes[2..]
            .chunks_exact(2)
            .map(|pair| {
                if little {
                    u16::from_le_bytes([pair[0], pair[1]])
                } else {
                    u16::from_be_bytes([pair[0], pair[1]])
                }
            })
            .collect();
        decoded = String::from_utf16(&units).map_err(|_| INVALID)?;
        &decoded
    } else {
        std::str::from_utf8(bytes).map_err(|_| INVALID)?
    };
    let mut reader = Reader::from_str(text);
    let mut result = Vec::new();
    let mut depth = 0usize;
    loop {
        if result.len() > 200_000 {
            return Err(LIMIT.into());
        }
        let event = reader.read_event().map_err(|_| INVALID)?;
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(element) | Event::Empty(element) => {
                let name = local(element.name().as_ref())?;
                let mut attrs = HashMap::new();
                for attribute in element.attributes() {
                    let attribute = attribute.map_err(|_| INVALID)?;
                    attrs.insert(
                        attribute.key.as_ref().to_owned(),
                        attribute
                            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                            .map_err(|_| INVALID)?
                            .into_owned(),
                    );
                    if attrs.len() > 64 {
                        return Err(LIMIT.into());
                    }
                }
                result.push(Token::Start(name.clone(), attrs));
                if empty {
                    result.push(Token::End(name));
                } else {
                    depth += 1;
                    if depth > 64 {
                        return Err(LIMIT.into());
                    }
                }
            }
            Event::End(element) => {
                depth = depth.checked_sub(1).ok_or(INVALID)?;
                result.push(Token::End(local(element.name().as_ref())?));
            }
            Event::Text(value) => result.push(Token::Text(value.as_ref().into())),
            Event::CData(value) => result.push(Token::Text(value.as_ref().into())),
            Event::GeneralRef(value) => {
                let text = if let Some(character) = value.resolve_char_ref().map_err(|_| INVALID)? {
                    character.to_string()
                } else {
                    quick_xml::escape::resolve_predefined_entity(value.as_ref())
                        .ok_or(INVALID)?
                        .into()
                };
                result.push(Token::Text(text));
            }
            Event::DocType(_) => return Err(INVALID.into()),
            Event::Eof => break,
            _ => (),
        }
    }
    if depth != 0 {
        return Err(INVALID.into());
    }
    Ok(result)
}
fn append(output: &mut String, text: &str) -> Result<(), String> {
    if output.len().saturating_add(text.len()) > PREVIEW_BYTES {
        return Err(LIMIT.into());
    }
    output.push_str(text);
    Ok(())
}
struct Package(HashMap<String, Vec<u8>>);
impl Package {
    fn open(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > OFFICE_BYTES || !bytes.starts_with(b"PK\x03\x04") {
            return Err(INVALID.into());
        }
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| INVALID)?;
        if zip.len() > 512 {
            return Err(LIMIT.into());
        }
        let mut total = 0u64;
        let mut parts = HashMap::new();
        for index in 0..zip.len() {
            let mut entry = zip.by_index(index).map_err(|_| INVALID)?;
            let name = entry.name().to_owned();
            if entry.encrypted()
                || name.len() > 256
                || name.contains('\\')
                || name.starts_with('/')
                || name.split('/').any(|part| part == ".." || part == ".")
                || name.chars().any(char::is_control)
                || entry
                    .unix_mode()
                    .is_some_and(|mode| mode & 0o170000 == 0o120000)
            {
                return Err(INVALID.into());
            }
            if entry.is_dir() {
                continue;
            }
            total = total.saturating_add(entry.size());
            if entry.size() > 4 * 1024 * 1024 || total > 16 * 1024 * 1024 {
                return Err(LIMIT.into());
            }
            let expected = entry.size() as usize;
            let mut content = Vec::with_capacity(expected);
            entry
                .by_ref()
                .take(expected as u64 + 1)
                .read_to_end(&mut content)
                .map_err(|_| INVALID)?;
            if content.len() != expected || parts.insert(name, content).is_some() {
                return Err(INVALID.into());
            }
        }
        Ok(Self(parts))
    }
    fn tokens(&self, part: &str) -> Result<Vec<Token>, String> {
        xml(self.0.get(part).ok_or(INVALID)?)
    }
    fn relationships(&self, part: &str) -> Result<Vec<(String, String, String)>, String> {
        let location = if part.is_empty() {
            "_rels/.rels".into()
        } else {
            let (folder, file) = part.rsplit_once('/').unwrap_or(("", part));
            format!("{folder}/_rels/{file}.rels")
                .trim_start_matches('/')
                .to_owned()
        };
        if !self.0.contains_key(&location) {
            return Ok(Vec::new());
        }
        let mut result = Vec::new();
        let mut ids = std::collections::HashSet::new();
        for token in self.tokens(&location)? {
            if let Token::Start(name, attrs) = token {
                if name == "Relationship" {
                    let id = attrs.get("Id").ok_or(INVALID)?;
                    if !ids.insert(id.clone()) {
                        return Err(INVALID.into());
                    }
                    if attrs
                        .get("TargetMode")
                        .is_some_and(|mode| mode == "External")
                    {
                        continue;
                    }
                    let target = target(part, attrs.get("Target").ok_or(INVALID)?)?;
                    result.push((
                        id.clone(),
                        attrs.get("Type").ok_or(INVALID)?.clone(),
                        target,
                    ));
                }
            }
        }
        Ok(result)
    }
}
fn target(source: &str, value: &str) -> Result<String, String> {
    let mut decoded = Vec::new();
    let raw = value.as_bytes();
    let mut index = 0;
    while index < raw.len() {
        if raw[index] == b'%' {
            if index + 2 >= raw.len() {
                return Err(INVALID.into());
            }
            let high = (raw[index + 1] as char).to_digit(16).ok_or(INVALID)?;
            let low = (raw[index + 2] as char).to_digit(16).ok_or(INVALID)?;
            decoded.push((high * 16 + low) as u8);
            index += 3;
        } else {
            decoded.push(raw[index]);
            index += 1;
        }
    }
    let value = std::str::from_utf8(&decoded).map_err(|_| INVALID)?;
    if value.contains(['\\', ':', '?', '#']) || value.chars().any(char::is_control) {
        return Err(INVALID.into());
    }
    let mut parts = if value.starts_with('/') {
        Vec::new()
    } else {
        source
            .rsplit_once('/')
            .map(|(folder, _)| folder.split('/').map(str::to_owned).collect())
            .unwrap_or_default()
    };
    for item in value.split('/') {
        match item {
            "" | "." => (),
            ".." => {
                parts.pop().ok_or(INVALID)?;
            }
            _ => parts.push(item.to_owned()),
        }
    }
    if parts.is_empty() {
        return Err(INVALID.into());
    }
    Ok(parts.join("/"))
}
fn relationship_id(attrs: &HashMap<String, String>) -> Result<&str, String> {
    let values: Vec<_> = attrs
        .iter()
        .filter(|(key, _)| key.ends_with(":id"))
        .map(|(_, value)| value.as_str())
        .collect();
    if values.len() != 1 {
        return Err(INVALID.into());
    }
    Ok(values[0])
}
fn words(tokens: Vec<Token>) -> Result<String, String> {
    let mut output = String::new();
    let mut text = false;
    let mut cells = 0usize;
    for token in tokens {
        match token {
            Token::Start(name, _) if name == "t" => text = true,
            Token::Start(name, _) if name == "tc" => cells += 1,
            Token::Start(name, _) if matches!(name.as_str(), "tab" | "br" | "cr") => {
                append(&mut output, if name == "tab" { "\t" } else { "\n" })?
            }
            Token::End(name) if name == "t" => text = false,
            Token::End(name) if name == "p" => {
                append(&mut output, if cells > 0 { " " } else { "\n" })?
            }
            Token::End(name) if name == "tc" => {
                cells = cells.saturating_sub(1);
                output.truncate(output.trim_end_matches(' ').len());
                append(&mut output, "\t")?;
            }
            Token::End(name) if name == "tr" => {
                if output.ends_with('\t') {
                    output.pop();
                }
                append(&mut output, "\n")?;
            }
            Token::Text(value) if text => append(&mut output, &value)?,
            _ => (),
        }
    }
    Ok(output)
}
fn strings(tokens: Vec<Token>) -> Result<Vec<String>, String> {
    let mut result = Vec::new();
    let mut value = String::new();
    let mut text = false;
    let mut phonetic = false;
    let mut total = 0usize;
    for token in tokens {
        match token {
            Token::Start(name, _) if name == "si" => value.clear(),
            Token::Start(name, _) if name == "rPh" => phonetic = true,
            Token::End(name) if name == "rPh" => phonetic = false,
            Token::Start(name, _) if name == "t" && !phonetic => text = true,
            Token::End(name) if name == "t" => text = false,
            Token::Text(part) if text => append(&mut value, &part)?,
            Token::End(name) if name == "si" => {
                total += value.len();
                if total > PREVIEW_BYTES || result.len() > 50_000 {
                    return Err(LIMIT.into());
                }
                result.push(value.clone());
            }
            _ => (),
        }
    }
    Ok(result)
}
fn sheet(tokens: Vec<Token>, shared: &[String]) -> Result<String, String> {
    let mut output = String::new();
    let mut reference = String::new();
    let mut kind = String::new();
    let mut value = String::new();
    let mut formula = String::new();
    let mut field = String::new();
    for token in tokens {
        match token {
            Token::Start(name, attrs) if name == "c" => {
                reference = attrs.get("r").cloned().unwrap_or_default();
                kind = attrs.get("t").cloned().unwrap_or_default();
                value.clear();
                formula.clear();
            }
            Token::Start(name, _) if matches!(name.as_str(), "t" | "v" | "f") => field = name,
            Token::End(name) if matches!(name.as_str(), "t" | "v" | "f") => field.clear(),
            Token::Text(part) if field == "f" => append(&mut formula, &part)?,
            Token::Text(part) if matches!(field.as_str(), "t" | "v") => append(&mut value, &part)?,
            Token::End(name) if name == "c" => {
                let stored = if kind == "s" {
                    shared
                        .get(value.parse::<usize>().map_err(|_| INVALID)?)
                        .ok_or(INVALID)?
                        .as_str()
                } else {
                    &value
                };
                append(&mut output, &reference)?;
                append(&mut output, "\t")?;
                append(&mut output, stored)?;
                if !formula.is_empty() {
                    append(&mut output, "\t[formula: ")?;
                    append(&mut output, &formula)?;
                    append(&mut output, "]")?;
                }
                append(&mut output, "\n")?;
            }
            _ => (),
        }
    }
    Ok(output)
}
pub fn preview(bytes: &[u8], extension: &str) -> Result<String, String> {
    let package = Package::open(bytes)?;
    let main = package
        .relationships("")?
        .into_iter()
        .find(|(_, kind, _)| kind.ends_with("/officeDocument"))
        .ok_or(INVALID)?
        .2;
    let main_type = match extension {
        "docx" => {
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"
        }
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
        "pptx" => {
            "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"
        }
        _ => return Err(INVALID.into()),
    };
    let mut declared = false;
    for token in package.tokens("[Content_Types].xml")? {
        if let Token::Start(name, attrs) = token {
            if name == "Override" || name == "Default" {
                let media = attrs.get("ContentType").ok_or(INVALID)?;
                if media.contains("macroEnabled") || media.contains("vbaProject") {
                    return Err(INVALID.into());
                }
                if name == "Override"
                    && target("", attrs.get("PartName").ok_or(INVALID)?)? == main
                    && media == main_type
                {
                    declared = true;
                }
            }
        }
    }
    if !declared {
        return Err(INVALID.into());
    }
    let relationships = package.relationships(&main)?;
    let mut output = String::new();
    if extension == "docx" {
        append(&mut output, &words(package.tokens(&main)?)?)?;
        for (_, kind, part) in relationships {
            if ["/header", "/footer", "/footnotes", "/endnotes"]
                .iter()
                .any(|suffix| kind.ends_with(suffix))
            {
                append(&mut output, "\n")?;
                append(&mut output, &words(package.tokens(&part)?)?)?;
            }
        }
    } else if extension == "pptx" {
        let mut number = 0usize;
        for token in package.tokens(&main)? {
            if let Token::Start(name, attrs) = token {
                if name == "sldId" {
                    let id = relationship_id(&attrs)?;
                    let part = &relationships
                        .iter()
                        .find(|(key, kind, _)| key == id && kind.ends_with("/slide"))
                        .ok_or(INVALID)?
                        .2;
                    number += 1;
                    append(&mut output, &format!("[{number}]\n"))?;
                    append(&mut output, &words(package.tokens(part)?)?)?;
                    for (_, kind, note) in package.relationships(part)? {
                        if kind.ends_with("/notesSlide") {
                            append(&mut output, &words(package.tokens(&note)?)?)?;
                        }
                    }
                    append(&mut output, "\n")?;
                }
            }
        }
    } else {
        let shared = if let Some((_, _, part)) = relationships
            .iter()
            .find(|(_, kind, _)| kind.ends_with("/sharedStrings"))
        {
            strings(package.tokens(part)?)?
        } else {
            Vec::new()
        };
        for token in package.tokens(&main)? {
            if let Token::Start(name, attrs) = token {
                if name == "sheet" {
                    let id = relationship_id(&attrs)?;
                    let title = attrs.get("name").ok_or(INVALID)?;
                    let (_, kind, part) = relationships
                        .iter()
                        .find(|(key, _, _)| key == id)
                        .ok_or(INVALID)?;
                    append(&mut output, &format!("[{title}]\n"))?;
                    if kind.ends_with("/worksheet") {
                        append(&mut output, &sheet(package.tokens(part)?, &shared)?)?;
                    } else if !kind.ends_with("/chartsheet") {
                        return Err(INVALID.into());
                    }
                    append(&mut output, "\n")?;
                }
            }
        }
    }
    if output
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err(INVALID.into());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    #[test]
    fn word_table_retains_empty_columns() {
        let tokens =
            super::xml(b"<tbl><tr><tc><p><t>A</t></p></tc><tc><p/></tc></tr></tbl>").unwrap();
        assert_eq!(super::words(tokens).unwrap(), "A\t\n");
    }
}
