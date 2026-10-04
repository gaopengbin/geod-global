use super::*;
pub(super) struct Table {
    pub fields: Vec<Field>,
    pub rows: Vec<(serde_json::Map<String, Value>, bool)>,
    pub encoding: String,
    pub encoding_source: String,
    pub ldid: u8,
    pub cpg: Option<String>,
}
enum Decoder {
    Ascii,
    Latin1,
    Code(&'static encoding_rs::Encoding),
}
impl Decoder {
    fn decode(&self, b: &[u8]) -> Result<String> {
        match self {
            Self::Ascii if b.is_ascii()=>Ok(String::from_utf8(b.to_vec()).unwrap()),
            Self::Ascii=>Err("DBF has non-ASCII text but no supported CPG or language driver; provide its explicit .cpg file".into()),
            Self::Latin1=>Ok(b.iter().map(|n|char::from(*n)).collect()),
            Self::Code(e)=>e.decode_without_bom_handling_and_without_replacement(b).map(|s|s.into_owned()).ok_or_else(||"DBF text is invalid in its declared encoding".into()),
        }
    }
    fn name(&self) -> &str {
        match self {
            Self::Ascii => "ASCII",
            Self::Latin1 => "ISO-8859-1",
            Self::Code(e) => e.name(),
        }
    }
}
fn encoding(cpg: Option<&str>, ldid: u8) -> Result<(Decoder, &'static str)> {
    if let Some(c) = cpg {
        let c = c.trim_matches('"').to_lowercase().replace('_', "-");
        let label = match c.as_str() {
            "65001" => "utf-8",
            "1252" => "windows-1252",
            "1251" => "windows-1251",
            "936" => "gbk",
            "950" => "big5",
            "932" => "shift-jis",
            "949" => "euc-kr",
            other => other,
        };
        if ["iso-8859-1", "8859-1", "latin1", "latin-1"].contains(&label) {
            return Ok((Decoder::Latin1, "cpg"));
        }
        let e = encoding_rs::Encoding::for_label(label.as_bytes())
            .ok_or("Unsupported DBF CPG encoding")?;
        if [
            encoding_rs::UTF_16LE,
            encoding_rs::UTF_16BE,
            encoding_rs::REPLACEMENT,
        ]
        .contains(&e)
        {
            return Err("Unsupported DBF CPG encoding".into());
        }
        return Ok((Decoder::Code(e), "cpg"));
    }
    Ok(match ldid {
        0x03 => (Decoder::Code(encoding_rs::WINDOWS_1252), "ldid"),
        // Explicit GDAL-compatible interpretation; CPG always takes precedence.
        0x57 => (Decoder::Latin1, "ldid"),
        0xc9 => (Decoder::Code(encoding_rs::WINDOWS_1251), "ldid"),
        0x7a => (Decoder::Code(encoding_rs::GBK), "ldid"),
        0x78 => (Decoder::Code(encoding_rs::BIG5), "ldid"),
        0x7b => (Decoder::Code(encoding_rs::SHIFT_JIS), "ldid"),
        0x79 => (Decoder::Code(encoding_rs::EUC_KR), "ldid"),
        _ => (Decoder::Ascii, "ascii-only"),
    })
}
pub(super) fn json_encoding(t: u8) -> Result<&'static str> {
    match t {
        b'C' => Ok("string"),
        b'N' | b'F' => Ok("decimal-string"),
        b'L' => Ok("boolean-or-null"),
        b'D' => Ok("iso-date-or-null"),
        _ => Err("Unsupported DBF field type; memo and binary fields are not read".into()),
    }
}
fn decimal(s: &str) -> bool {
    let s = s.strip_prefix(['+', '-']).unwrap_or(s);
    let mut parts = s.split(['e', 'E']);
    let mantissa = parts.next().unwrap();
    let exponent = parts.next();
    if parts.next().is_some() {
        return false;
    }
    let mut dot = false;
    let mut digits = 0;
    for c in mantissa.bytes() {
        if c.is_ascii_digit() {
            digits += 1;
        } else if c == b'.' && !dot {
            dot = true;
        } else {
            return false;
        }
    }
    digits > 0
        && exponent.is_none_or(|e| {
            let e = e.strip_prefix(['+', '-']).unwrap_or(e);
            !e.is_empty() && e.bytes().all(|c| c.is_ascii_digit())
        })
}
fn property(b: &[u8], f: &Field, d: &Decoder) -> Result<Value> {
    if f.field_type == "C" {
        let end = b
            .iter()
            .rposition(|n| *n != b' ' && *n != 0)
            .map_or(0, |i| i + 1);
        return Ok(json!(d.decode(&b[..end])?));
    }
    let s = std::str::from_utf8(b)
        .map_err(|_| "DBF numeric, date and logical fields require ASCII")?
        .trim();
    match f.field_type.as_str() {
        "N" | "F" if s.is_empty() || s.bytes().all(|n| n == b'*') => Ok(Value::Null),
        "N" | "F" if decimal(s) => Ok(json!(s)),
        "L" => match s.to_ascii_uppercase().as_str() {
            "T" | "Y" => Ok(json!(true)),
            "F" | "N" => Ok(json!(false)),
            "" | "?" => Ok(Value::Null),
            _ => Err("Invalid DBF logical field".into()),
        },
        "D" if s.is_empty() || s == "00000000" => Ok(Value::Null),
        "D" if s.len() == 8 && s.bytes().all(|n| n.is_ascii_digit()) => {
            chrono::NaiveDate::parse_from_str(s, "%Y%m%d")
                .map(|v| json!(v.format("%Y-%m-%d").to_string()))
                .map_err(|_| "Invalid DBF calendar date".into())
        }
        _ => Err("Invalid DBF numeric or date field".into()),
    }
}
pub(super) fn decode(bytes: &[u8], cpg: Option<&[u8]>) -> Result<Table> {
    if bytes.len() < 33 || bytes[0] != 3 {
        return Err("Supported DBF tables use dBASE III without memo fields".into());
    }
    let count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let header = u16::from_le_bytes(bytes[8..10].try_into().unwrap()) as usize;
    let record = u16::from_le_bytes(bytes[10..12].try_into().unwrap()) as usize;
    let end = header
        .checked_add(
            count
                .checked_mul(record)
                .ok_or("DBF record size overflow")?,
        )
        .ok_or("DBF record size overflow")?;
    if count > MAX_FEATURES
        || header < 33
        || !(header - 33).is_multiple_of(32)
        || (header - 33) / 32 > 128
        || header > bytes.len()
        || bytes[header - 1] != 0x0d
        || record == 0
        || end > bytes.len()
        || !(end == bytes.len() || end + 1 == bytes.len() && bytes[end] == 0x1a)
    {
        return Err("Invalid DBF header, record count or file length".into());
    }
    let cpg = cpg
        .map(|b| -> Result<String> {
            if b.len() > 80 {
                return Err("DBF CPG exceeds 80 bytes".into());
            }
            let s = std::str::from_utf8(b.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(b))
                .map_err(|_| "DBF CPG requires an ASCII encoding name")?
                .trim();
            if s.is_empty() || !s.is_ascii() || s.chars().any(char::is_control) {
                return Err("Invalid DBF CPG encoding name".into());
            }
            Ok(s.into())
        })
        .transpose()?;
    let (decoder, origin) = encoding(cpg.as_deref(), bytes[29])?;
    let mut fields = vec![];
    let mut names = BTreeSet::new();
    let mut width = 1usize;
    for field in bytes[32..header - 1].chunks_exact(32) {
        let name_end = field[..11].iter().position(|n| *n == 0).unwrap_or(11);
        let name = decoder.decode(&field[..name_end])?;
        if name.is_empty()
            || name.len() > 64
            || name.chars().any(char::is_control)
            || !names.insert(name.to_lowercase())
            || field[16] == 0
        {
            return Err("DBF has invalid or ambiguous field names or widths".into());
        }
        let t = field[11];
        let code = json_encoding(t)?;
        if t == b'D' && field[16] != 8
            || t == b'L' && field[16] != 1
            || t == b'C' && field[17] != 0
            || [b'N', b'F'].contains(&t) && field[17] >= field[16]
        {
            return Err("Unsupported DBF field width or decimal count".into());
        }
        width += field[16] as usize;
        fields.push(Field {
            name,
            field_type: char::from(t).to_string(),
            width: field[16],
            decimals: field[17],
            json_encoding: code.into(),
        });
    }
    if width != record {
        return Err("DBF field widths do not match its record length".into());
    }
    let mut rows = Vec::with_capacity(count);
    for row in bytes[header..end].chunks_exact(record) {
        if ![b' ', b'*'].contains(&row[0]) {
            return Err("Invalid DBF deletion flag".into());
        }
        let mut props = serde_json::Map::new();
        let mut offset = 1;
        for f in &fields {
            let n = f.width as usize;
            props.insert(
                f.name.clone(),
                property(&row[offset..offset + n], f, &decoder)?,
            );
            offset += n;
        }
        rows.push((props, row[0] == b'*'));
    }
    Ok(Table {
        fields,
        rows,
        encoding: decoder.name().into(),
        encoding_source: origin.into(),
        ldid: bytes[29],
        cpg,
    })
}
