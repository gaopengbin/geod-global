//! Bounded wire reader for the public OSM-binary schema. No generated decoder
//! allocates packed arrays before their cardinality / decompressed-byte checks.
use super::*;
use flate2::bufread::ZlibDecoder;
use std::io::Read;

#[derive(Clone, Copy)]
enum Field<'a> {
    Var(u64),
    Bytes(&'a [u8]),
    Fixed,
}
struct Message<'a>(Vec<(u32, Field<'a>)>);
fn varint(input: &mut &[u8]) -> Result<u64> {
    let mut n = 0u64;
    for shift in (0..70).step_by(7) {
        let (&b, rest) = input.split_first().ok_or("Truncated OSM PBF varint")?;
        *input = rest;
        if shift == 63 && b > 1 {
            return Err("OSM PBF varint overflows 64 bits".into());
        }
        n |= u64::from(b & 127) << shift;
        if b < 128 {
            return Ok(n);
        }
    }
    Err("Invalid OSM PBF varint".into())
}
fn take<'a>(input: &mut &'a [u8], count: usize) -> Result<&'a [u8]> {
    if count > input.len() {
        return Err("Truncated OSM PBF message".into());
    }
    let (value, rest) = input.split_at(count);
    *input = rest;
    Ok(value)
}
fn zig(n: u64) -> i64 {
    ((n >> 1) as i64) ^ (-((n & 1) as i64))
}
fn add(n: &mut i64, delta: i64) -> Result<i64> {
    *n = n
        .checked_add(delta)
        .ok_or("OSM PBF delta overflows 64 bits")?;
    Ok(*n)
}
impl<'a> Message<'a> {
    fn parse(mut input: &'a [u8]) -> Result<Self> {
        let mut fields = Vec::new();
        while !input.is_empty() {
            if fields.len() >= 500_000 {
                return Err("OSM PBF exceeds its field limit".into());
            }
            let tag = varint(&mut input)?;
            let id = tag >> 3;
            if id == 0 || id > 536_870_911 {
                return Err("Invalid OSM PBF field number".into());
            }
            let field = match tag & 7 {
                0 => Field::Var(varint(&mut input)?),
                2 => {
                    let len = usize::try_from(varint(&mut input)?)
                        .map_err(|_| "OSM PBF length overflow")?;
                    Field::Bytes(take(&mut input, len)?)
                }
                1 => {
                    take(&mut input, 8)?;
                    Field::Fixed
                }
                5 => {
                    take(&mut input, 4)?;
                    Field::Fixed
                }
                _ => return Err("Unsupported OSM PBF wire type".into()),
            };
            fields.push((id as u32, field));
        }
        Ok(Self(fields))
    }
    fn one(&self, id: u32) -> Result<Option<Field<'a>>> {
        let mut found = self.0.iter().filter(|(i, _)| *i == id).map(|(_, f)| *f);
        let value = found.next();
        if found.next().is_some() {
            return Err("Duplicate OSM PBF singleton field".into());
        }
        Ok(value)
    }
    fn number(&self, id: u32) -> Result<Option<u64>> {
        match self.one(id)? {
            Some(Field::Var(n)) => Ok(Some(n)),
            None => Ok(None),
            _ => Err("OSM PBF numeric field has an invalid wire type".into()),
        }
    }
    fn bytes(&self, id: u32) -> Result<Option<&'a [u8]>> {
        match self.one(id)? {
            Some(Field::Bytes(b)) => Ok(Some(b)),
            None => Ok(None),
            _ => Err("OSM PBF message field has an invalid wire type".into()),
        }
    }
    fn required(&self, id: u32) -> Result<u64> {
        self.number(id)?
            .ok_or_else(|| "OSM PBF is missing a required numeric field".into())
    }
    fn messages(&self, id: u32) -> Result<Vec<&'a [u8]>> {
        self.0
            .iter()
            .filter(|(i, _)| *i == id)
            .map(|(_, f)| match f {
                Field::Bytes(b) => Ok(*b),
                _ => Err("OSM PBF repeated message has an invalid wire type".into()),
            })
            .collect()
    }
    fn packed(&self, id: u32) -> Result<Vec<u64>> {
        let mut result = Vec::new();
        for (_, f) in self.0.iter().filter(|(i, _)| *i == id) {
            match f {
                Field::Var(n) => result.push(*n),
                Field::Bytes(b) => {
                    let mut input = *b;
                    while !input.is_empty() {
                        if result.len() >= 500_000 {
                            return Err("OSM PBF packed array exceeds 500,000 values".into());
                        }
                        result.push(varint(&mut input)?);
                    }
                }
                _ => return Err("Invalid OSM PBF packed field".into()),
            }
            if result.len() > 500_000 {
                return Err("OSM PBF packed array exceeds 500,000 values".into());
            }
        }
        Ok(result)
    }
    fn text(&self, id: u32) -> Result<Option<String>> {
        self.bytes(id)?.map(utf8).transpose()
    }
}
fn utf8(b: &[u8]) -> Result<String> {
    std::str::from_utf8(b)
        .map(str::to_owned)
        .map_err(|_| "OSM PBF contains invalid UTF-8".into())
}
fn timestamp(n: i64, granularity: i64) -> Result<String> {
    let ms = n
        .checked_mul(granularity)
        .filter(|n| *n >= 0)
        .ok_or("Invalid / overflowing OSM PBF timestamp")?;
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true))
        .ok_or_else(|| "OSM PBF timestamp is outside the supported range".into())
}
fn header(bytes: &[u8], source: &mut Provenance) -> Result<()> {
    let m = Message::parse(bytes)?;
    source.required_features = m
        .messages(4)?
        .into_iter()
        .map(utf8)
        .collect::<Result<_>>()?;
    source.optional_features = m
        .messages(5)?
        .into_iter()
        .map(utf8)
        .collect::<Result<_>>()?;
    for feature in &source.required_features {
        if !["OsmSchema-V0.6", "DenseNodes"].contains(&feature.as_str()) {
            return Err(format!("Unsupported required OSM PBF feature: {feature}"));
        }
    }
    if !source
        .required_features
        .iter()
        .any(|f| f == "OsmSchema-V0.6")
    {
        return Err("OSM PBF requires OsmSchema-V0.6".into());
    }
    source.generator = m.text(16)?;
    source.source = m.text(17)?;
    source.dataset_timestamp = m
        .number(32)?
        .map(|n| timestamp(n as i64, 1000))
        .transpose()?;
    source.replication_sequence = m
        .number(33)?
        .map(|n| integer(n as i64, false).map(|n| n.to_string()))
        .transpose()?;
    source.replication_base_url = m.text(34)?;
    if let Some(b) = m.bytes(1)? {
        let b = Message::parse(b)?;
        let w = zig(b.required(1)?) as f64 * 1e-9;
        let e = zig(b.required(2)?) as f64 * 1e-9;
        let n = zig(b.required(3)?) as f64 * 1e-9;
        let s = zig(b.required(4)?) as f64 * 1e-9;
        if !valid_bounds([w, s, e, n]) {
            return Err("Invalid OSM PBF declared bounds".into());
        }
        source.declared_bounds = Some([w, s, e, n]);
    }
    validate(source, 0)
}
fn blob(bytes: &[u8], remaining: usize) -> Result<Vec<u8>> {
    let m = Message::parse(bytes)?;
    let encodings =
        m.0.iter()
            .filter(|(i, _)| [1, 3, 4, 5, 6, 7].contains(i))
            .collect::<Vec<_>>();
    if encodings.len() != 1 {
        return Err("OSM PBF blob must have one encoding".into());
    }
    let raw_size = m
        .number(2)?
        .map(|n| usize::try_from(n).map_err(|_| "OSM PBF raw size overflow"))
        .transpose()?;
    if raw_size.is_some_and(|n| n > remaining) {
        return Err("OSM PBF decompressed blocks exceed 20 MiB".into());
    }
    if encodings[0].0 == 1 {
        let data = m.bytes(1)?.unwrap();
        if data.len() > remaining || raw_size.is_some_and(|n| n != data.len()) {
            return Err("Invalid OSM PBF raw blob size".into());
        }
        return Ok(data.to_vec());
    }
    if encodings[0].0 != 3 {
        return Err(
            "OSM PBF supports raw and zlib compression; this blob encoding is unsupported".into(),
        );
    }
    let expected = raw_size.ok_or("Compressed OSM PBF blob requires raw_size")?;
    let mut decoder = ZlibDecoder::new(m.bytes(3)?.unwrap());
    let mut out = Vec::new();
    (&mut decoder)
        .take(remaining as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|e| format!("Invalid OSM PBF zlib blob: {e}"))?;
    if out.len() != expected || !decoder.get_ref().is_empty() {
        return Err("OSM PBF zlib size or trailing bytes differ from its blob".into());
    }
    Ok(out)
}
struct Block {
    strings: Vec<String>,
    granularity: i64,
    date_granularity: i64,
    lat_offset: i64,
    lon_offset: i64,
    locations: bool,
}
impl Block {
    fn string(&self, n: u64) -> Result<&str> {
        self.strings
            .get(usize::try_from(n).map_err(|_| "OSM PBF string index overflow")?)
            .map(String::as_str)
            .ok_or_else(|| "OSM PBF string index is outside its table".into())
    }
    fn coord(&self, n: i64, lon: bool) -> Result<f64> {
        let value = n
            .checked_mul(self.granularity)
            .and_then(|v| {
                v.checked_add(if lon {
                    self.lon_offset
                } else {
                    self.lat_offset
                })
            })
            .ok_or("OSM PBF coordinate overflows 64 bits")?;
        let limit = if lon { 180_000_000_000 } else { 90_000_000_000 };
        if !(-limit..=limit).contains(&value) {
            return Err("OSM PBF coordinate is outside WGS84".into());
        }
        Ok(value as f64 * 1e-9)
    }
    fn tags(&self, keys: &[u64], vals: &[u64]) -> Result<Value> {
        if keys.len() != vals.len() {
            return Err("OSM PBF tag key/value lengths differ".into());
        }
        let mut tags = BTreeMap::new();
        for (k, v) in keys.iter().zip(vals) {
            let key = self.string(*k)?;
            if key.is_empty() || tags.insert(key, self.string(*v)?).is_some() {
                return Err("OSM PBF has an empty or duplicate tag key".into());
            }
        }
        Ok(json!(tags))
    }
    fn info(&self, element: &mut Value, bytes: &[u8]) -> Result<()> {
        let m = Message::parse(bytes)?;
        for (id, key) in [(1, "version"), (3, "changeset"), (4, "uid")] {
            if let Some(n) = m.number(id)? {
                if id != 3 && n > i32::MAX as u64 {
                    return Err("Invalid OSM PBF int32 metadata".into());
                }
                element[key] = json!(integer(n as i64, false)?);
            }
        }
        if let Some(n) = m.number(2)? {
            element["timestamp"] = json!(timestamp(n as i64, self.date_granularity)?);
        }
        if let Some(n) = m.number(5)? {
            element["user"] = json!(self.string(n)?);
        }
        if let Some(n) = m.number(6)? {
            if n != 1 {
                return Err("Deleted / historical OSM objects are not supported".into());
            }
            element["visible"] = json!(true);
        }
        Ok(())
    }
    fn simple(&self, kind: &str, bytes: &[u8]) -> Result<Value> {
        let m = Message::parse(bytes)?;
        let encoded = m.required(1)?;
        let id = integer(
            if kind == "node" {
                zig(encoded)
            } else {
                encoded as i64
            },
            true,
        )?;
        let mut element =
            json!({"type":kind,"id":id,"tags":self.tags(&m.packed(2)?,&m.packed(3)?)?});
        if let Some(info) = m.bytes(4)? {
            self.info(&mut element, info)?;
        }
        if kind == "node" {
            element["lat"] = json!(self.coord(zig(m.required(8)?), false)?);
            element["lon"] = json!(self.coord(zig(m.required(9)?), true)?);
        } else if kind == "way" {
            let mut id = 0;
            let refs = m
                .packed(8)?
                .into_iter()
                .map(|n| add(&mut id, zig(n)).and_then(|n| integer(n, true)))
                .collect::<Result<Vec<_>>>()?;
            let lat = m.packed(9)?;
            let lon = m.packed(10)?;
            if !lat.is_empty() || !lon.is_empty() {
                if !self.locations || lat.len() != refs.len() || lon.len() != refs.len() {
                    return Err(
                        "Invalid OSM PBF LocationsOnWays arrays or feature declaration".into(),
                    );
                }
                let mut x = 0;
                let mut y = 0;
                let mut geometry = Vec::new();
                for (lat, lon) in lat.iter().zip(lon) {
                    geometry.push(json!({"lon":self.coord(add(&mut x,zig(lon))?,true)?,"lat":self.coord(add(&mut y,zig(*lat))?,false)?}));
                }
                element["geometry"] = json!(geometry);
            }
            element["nodes"] = json!(refs);
        } else {
            let roles = m.packed(8)?;
            let refs = m.packed(9)?;
            let types = m.packed(10)?;
            if roles.len() != refs.len() || types.len() != refs.len() {
                return Err(format!(
                    "OSM relation/{id} has inconsistent PBF member arrays"
                ));
            }
            if refs.len() > 1024 {
                return Err(format!("OSM relation/{id} exceeds 1,024 members"));
            }
            let mut id = 0;
            let mut members = Vec::new();
            for ((role, id_delta), kind) in roles.iter().zip(refs).zip(types) {
                let kind = match kind {
                    0 => "node",
                    1 => "way",
                    2 => "relation",
                    _ => return Err("Invalid OSM PBF member type".into()),
                };
                members.push(json!({"type":kind,"ref":integer(add(&mut id,zig(id_delta))?,true)?,"role":self.string(*role)?}));
            }
            element["members"] = json!(members);
        }
        Ok(element)
    }
    fn dense(
        &self,
        bytes: &[u8],
        elements: &mut Vec<Value>,
        source: &mut Provenance,
    ) -> Result<()> {
        let m = Message::parse(bytes)?;
        let ids = m.packed(1)?;
        let lat = m.packed(8)?;
        let lon = m.packed(9)?;
        let tags = m.packed(10)?;
        if ids.len() != lat.len()
            || ids.len() != lon.len()
            || ids.len() > MAX_FEATURES - elements.len()
        {
            return Err("OSM PBF dense node arrays differ or exceed 50,000 objects".into());
        }
        let info = m.bytes(5)?.map(Message::parse).transpose()?;
        let columns = (1..=6)
            .map(|i| {
                info.as_ref()
                    .map(|m| m.packed(i))
                    .unwrap_or_else(|| Ok(Vec::new()))
            })
            .collect::<Result<Vec<_>>>()?;
        if columns
            .iter()
            .any(|c| !c.is_empty() && c.len() != ids.len())
        {
            return Err("OSM PBF dense metadata arrays differ from node count".into());
        }
        let mut id = 0;
        let mut x = 0;
        let mut y = 0;
        let mut cursor = 0;
        let mut metadata = [0i64; 6];
        for i in 0..ids.len() {
            let mut element = json!({"type":"node","id":integer(add(&mut id,zig(ids[i]))?,true)?,"lat":self.coord(add(&mut y,zig(lat[i]))?,false)?,"lon":self.coord(add(&mut x,zig(lon[i]))?,true)?});
            let mut keys = Vec::new();
            let mut vals = Vec::new();
            if !tags.is_empty() {
                loop {
                    let key = *tags
                        .get(cursor)
                        .ok_or("OSM PBF dense tags lack a node delimiter")?;
                    cursor += 1;
                    if key == 0 {
                        break;
                    }
                    keys.push(key);
                    vals.push(*tags.get(cursor).ok_or("OSM PBF dense tags lack a value")?);
                    cursor += 1;
                }
            }
            element["tags"] = self.tags(&keys, &vals)?;
            for (col, key) in [
                "version",
                "timestamp",
                "changeset",
                "uid",
                "user",
                "visible",
            ]
            .iter()
            .enumerate()
            {
                if columns[col].is_empty() {
                    continue;
                }
                let n = columns[col][i];
                match col {
                    0 => {
                        if n > i32::MAX as u64 {
                            return Err("Invalid OSM PBF dense version".into());
                        }
                        element[*key] = json!(n);
                    }
                    1 => {
                        element[*key] = json!(timestamp(
                            add(&mut metadata[col], zig(n))?,
                            self.date_granularity
                        )?)
                    }
                    2 => element[*key] = json!(integer(add(&mut metadata[col], zig(n))?, false)?),
                    3 | 4 => {
                        if n > u32::MAX as u64 {
                            return Err("Invalid OSM PBF dense sint32 metadata".into());
                        }
                        let v = integer(add(&mut metadata[col], zig(n))?, false)?;
                        if v > i32::MAX as i64 {
                            return Err("OSM PBF dense metadata exceeds int32".into());
                        }
                        element[*key] = if col == 4 {
                            json!(self.string(v as u64)?)
                        } else {
                            json!(v)
                        };
                    }
                    5 => {
                        if n != 1 {
                            return Err("Deleted / historical OSM objects are not supported".into());
                        }
                        element[*key] = json!(true);
                    }
                    _ => unreachable!(),
                }
            }
            push(elements, source, element)?;
        }
        if cursor != tags.len() {
            return Err("OSM PBF dense tags have trailing values".into());
        }
        Ok(())
    }
}
fn data(bytes: &[u8], elements: &mut Vec<Value>, source: &mut Provenance) -> Result<()> {
    let m = Message::parse(bytes)?;
    let table = Message::parse(m.bytes(1)?.ok_or("OSM PBF block has no string table")?)?;
    let strings = table
        .messages(1)?
        .into_iter()
        .map(utf8)
        .collect::<Result<Vec<_>>>()?;
    if strings.first().map(String::as_str) != Some("") {
        return Err("OSM PBF string table slot zero must be empty".into());
    }
    let granularity = m.number(17)?.unwrap_or(100);
    let date_granularity = m.number(18)?.unwrap_or(1000);
    if granularity == 0
        || granularity > i32::MAX as u64
        || date_granularity == 0
        || date_granularity > i32::MAX as u64
    {
        return Err("Invalid OSM PBF coordinate or date granularity".into());
    }
    let block = Block {
        strings,
        granularity: granularity as i64,
        date_granularity: date_granularity as i64,
        lat_offset: m.number(19)?.unwrap_or(0) as i64,
        lon_offset: m.number(20)?.unwrap_or(0) as i64,
        locations: source
            .optional_features
            .iter()
            .any(|s| s == "LocationsOnWays"),
    };
    for group in m.messages(2)? {
        let group = Message::parse(group)?;
        let kinds = (1..=5)
            .filter(|id| group.0.iter().any(|(i, _)| i == id))
            .collect::<Vec<_>>();
        if kinds.len() != 1 || kinds[0] == 5 {
            return Err(
                "OSM PBF groups require one node / way / relation type; changesets are unsupported"
                    .into(),
            );
        }
        let kind = kinds[0];
        if kind == 2 {
            if !source.required_features.iter().any(|s| s == "DenseNodes") {
                return Err("OSM PBF dense nodes require the DenseNodes feature".into());
            }
            block.dense(group.bytes(2)?.unwrap(), elements, source)?;
        } else {
            for b in group.messages(kind)? {
                push(
                    elements,
                    source,
                    block.simple(
                        match kind {
                            1 => "node",
                            3 => "way",
                            4 => "relation",
                            _ => unreachable!(),
                        },
                        b,
                    )?,
                )?;
            }
        }
    }
    Ok(())
}
pub(super) fn decode(mut input: &[u8]) -> Result<(Vec<Value>, Provenance)> {
    let mut source = Provenance::new("pbf");
    let mut elements = Vec::new();
    let mut seen_header = false;
    let mut decompressed = 0usize;
    let mut blocks = 0;
    while !input.is_empty() {
        blocks += 1;
        if blocks > 1024 {
            return Err("OSM PBF exceeds 1,024 file blocks".into());
        }
        let length = u32::from_be_bytes(take(&mut input, 4)?.try_into().unwrap()) as usize;
        if length == 0 || length >= 65_536 {
            return Err("Invalid OSM PBF file block header length".into());
        }
        let h = Message::parse(take(&mut input, length)?)?;
        let kind = h.text(1)?.ok_or("OSM PBF block requires a type")?;
        let size = usize::try_from(h.required(3)?).map_err(|_| "OSM PBF blob size overflow")?;
        if size == 0 || size > MAX_BYTES {
            return Err("OSM PBF compressed blob exceeds 20 MiB".into());
        }
        let compressed = take(&mut input, size)?;
        match kind.as_str() {
            "OSMHeader" => {
                if seen_header {
                    return Err("OSM PBF snapshot has duplicate headers".into());
                }
                let b = blob(compressed, MAX_BYTES - decompressed)?;
                decompressed += b.len();
                header(&b, &mut source)?;
                seen_header = true;
            }
            "OSMData" => {
                if !seen_header {
                    return Err("OSM PBF data appears before its header".into());
                }
                let b = blob(compressed, MAX_BYTES - decompressed)?;
                decompressed += b.len();
                data(&b, &mut elements, &mut source)?;
            }
            _ => {
                if !seen_header {
                    return Err("OSM PBF requires its header as the first file block".into());
                }
                if !source.ignored_block_types.contains(&kind) {
                    source.ignored_block_types.push(kind);
                    if source.ignored_block_types.len() > 64 {
                        return Err("OSM PBF has too many extension block types".into());
                    }
                }
            }
        }
    }
    if !seen_header {
        return Err("OSM PBF snapshot has no header".into());
    }
    Ok((elements, source))
}
