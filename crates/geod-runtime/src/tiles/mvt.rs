//! Bounded MVT v1/v2 protobuf validation before exposing tiles to the renderer.
use super::*;
use format::varint;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Layer {
    pub name: String,
    pub extent: u32,
    pub version: u8,
    pub features: usize,
}
enum Value<'a> {
    Integer(u64),
    Bytes(&'a [u8]),
    Fixed(&'a [u8]),
}
fn fields(b: &[u8]) -> Result<Vec<(u64, Value<'_>)>> {
    let mut p = 0;
    let mut out = Vec::new();
    while p < b.len() {
        if out.len() >= 200_000 {
            return Err("MVT protobuf contains too many fields".into());
        }
        let key = varint(b, &mut p)?;
        let number = key >> 3;
        if number == 0 || number >= (1 << 29) {
            return Err("Invalid MVT protobuf field".into());
        }
        let value = match key & 7 {
            0 => Value::Integer(varint(b, &mut p)?),
            1 | 5 => {
                let length = if key & 7 == 1 { 8 } else { 4 };
                let slice = b.get(p..p + length).ok_or("Truncated MVT fixed field")?;
                p += length;
                Value::Fixed(slice)
            }
            2 => {
                let length = usize::try_from(varint(b, &mut p)?).map_err(io_error)?;
                let end = p
                    .checked_add(length)
                    .filter(|n| *n <= b.len())
                    .ok_or("Truncated MVT field")?;
                let slice = &b[p..end];
                p = end;
                Value::Bytes(slice)
            }
            _ => return Err("Unsupported MVT protobuf wire type".into()),
        };
        out.push((number, value));
    }
    Ok(out)
}
fn packed(raw: &[u8]) -> Result<Vec<u64>> {
    let mut p = 0;
    let mut values = Vec::new();
    while p < raw.len() {
        values.push(varint(raw, &mut p)?);
    }
    Ok(values)
}
pub fn inspect(raw: &[u8]) -> Result<Vec<Layer>> {
    if raw.len() > format::MAX_TILE {
        return Err("MVT tile exceeds its decoded size limit".into());
    }
    let mut layers = Vec::new();
    let mut names = BTreeSet::new();
    for (field, value) in fields(raw)? {
        if field != 3 {
            continue;
        }
        let Value::Bytes(bytes) = value else {
            return Err("Invalid MVT layer field".into());
        };
        let mut name = None;
        let mut extent = 4096;
        let mut version = None;
        let mut features = Vec::new();
        let mut keys = 0;
        let mut values = 0;
        for (key, value) in fields(bytes)? {
            match (key, value) {
                (1, Value::Bytes(b)) => {
                    if name.is_some() {
                        return Err("Duplicate MVT layer name".into());
                    }
                    name = Some(
                        std::str::from_utf8(b)
                            .map_err(|_| "MVT layer name is not UTF-8")?
                            .to_string(),
                    );
                }
                (2, Value::Bytes(b)) => {
                    if features.len() >= 50_000 {
                        return Err("MVT tile contains too many features".into());
                    }
                    features.push(b);
                }
                (3, Value::Bytes(b)) => {
                    std::str::from_utf8(b).map_err(|_| "MVT property key is not UTF-8")?;
                    keys += 1;
                }
                (4, Value::Bytes(b)) => {
                    let f = fields(b)?;
                    if f.len() != 1 {
                        return Err("Invalid MVT property value".into());
                    }
                    match &f[0] {
                        (1, Value::Bytes(b)) => {
                            std::str::from_utf8(b).map_err(|_| "MVT string value is not UTF-8")?;
                        }
                        (2, Value::Fixed(b)) if b.len() == 4 => {}
                        (3, Value::Fixed(b)) if b.len() == 8 => {}
                        (4..=6, Value::Integer(_)) => {}
                        (7, Value::Integer(0 | 1)) => {}
                        _ => return Err("Invalid MVT property encoding".into()),
                    }
                    values += 1;
                }
                (5, Value::Integer(n)) => {
                    extent = u32::try_from(n).map_err(io_error)?;
                }
                (15, Value::Integer(n)) => {
                    if version.is_some() {
                        return Err("Duplicate MVT layer version".into());
                    }
                    version = Some(n);
                }
                (1..=5 | 15, _) => return Err("Invalid MVT layer encoding".into()),
                _ => {}
            }
        }
        let name = name
            .filter(|s| clean(s, 256))
            .ok_or("MVT layer name is missing")?;
        let version = version
            .filter(|n| [1, 2].contains(n))
            .ok_or("Unsupported MVT layer version")? as u8;
        if extent == 0
            || extent > 1_048_576
            || !names.insert(name.clone())
            || layers.len() >= 256
            || keys > 50_000
            || values > 50_000
        {
            return Err("Invalid MVT layer dimensions or property tables".into());
        }
        for f in &features {
            let mut geometry = None;
            let mut kind = None;
            let mut tags = Vec::new();
            for (key, value) in fields(f)? {
                match (key, value) {
                    (1, Value::Integer(_)) => {}
                    (2, Value::Bytes(b)) => tags.extend(packed(b)?),
                    (3, Value::Integer(n)) => {
                        if kind.is_some() {
                            return Err("Duplicate MVT geometry type".into());
                        }
                        kind = Some(n);
                    }
                    (4, Value::Bytes(b)) => {
                        if geometry.is_some() {
                            return Err("Duplicate MVT geometry stream".into());
                        }
                        geometry = Some(packed(b)?);
                    }
                    (1..=4, _) => return Err("Invalid MVT feature encoding".into()),
                    _ => {}
                }
            }
            if tags.len() % 2 != 0 || tags.chunks_exact(2).any(|t| t[0] >= keys || t[1] >= values) {
                return Err("MVT property index is outside its table".into());
            }
            let kind = kind
                .filter(|k| [1, 2, 3].contains(k))
                .ok_or("Invalid MVT geometry type")?;
            let commands = geometry.ok_or("MVT geometry is missing")?;
            let mut i = 0;
            let (mut x, mut y) = (0i64, 0i64);
            let mut moved = false;
            while i < commands.len() {
                let command = commands[i] & 7;
                let count = commands[i] >> 3;
                i += 1;
                if count == 0
                    || ![1, 2, 7].contains(&command)
                    || command == 7 && (count != 1 || kind != 3 || !moved)
                    || command == 2 && (!moved || kind == 1)
                {
                    return Err("Invalid MVT geometry command".into());
                }
                if command == 7 {
                    continue;
                }
                let length = usize::try_from(count.checked_mul(2).ok_or("MVT geometry overflow")?)
                    .map_err(io_error)?;
                let end = i
                    .checked_add(length)
                    .filter(|n| *n <= commands.len())
                    .ok_or("Truncated MVT geometry command")?;
                for pair in commands[i..end].chunks_exact(2) {
                    let delta = |n: u64| -> Result<i64> {
                        if n > u32::MAX as u64 {
                            return Err("MVT coordinate delta exceeds signed 32 bits".into());
                        }
                        Ok(((n >> 1) as i64) ^ (-((n & 1) as i64)))
                    };
                    x = x
                        .checked_add(delta(pair[0])?)
                        .ok_or("MVT coordinate overflow")?;
                    y = y
                        .checked_add(delta(pair[1])?)
                        .ok_or("MVT coordinate overflow")?;
                    if x.abs() > i32::MAX as i64 || y.abs() > i32::MAX as i64 {
                        return Err("MVT coordinate overflow".into());
                    }
                }
                moved = true;
                i = end;
            }
            if !moved {
                return Err("MVT feature has no coordinates".into());
            }
        }
        layers.push(Layer {
            name,
            extent,
            version,
            features: features.len(),
        });
    }
    Ok(layers)
}
