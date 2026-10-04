//! GeoPackage WKB's XY order overrides SRS axis order. Keep original WKT and
//! units; adapt only the engine input (OGC StandardGeoPackageBinary note).
use super::*;
#[derive(Clone)]
enum Node {
    Atom(String),
    Element(String, Vec<Node>),
}
impl Node {
    fn atom(&self) -> Option<&str> {
        if let Self::Atom(s) = self {
            Some(s.trim_matches('"'))
        } else {
            None
        }
    }
    fn name(&self, name: &str) -> bool {
        matches!(self,Self::Element(n,_) if n.eq_ignore_ascii_case(name))
    }
    fn write(&self) -> String {
        match self {
            Self::Atom(s) => s.clone(),
            Self::Element(n, fields) => format!(
                "{}[{}]",
                n,
                fields.iter().map(Node::write).collect::<Vec<_>>().join(",")
            ),
        }
    }
}
struct Parser<'a> {
    text: &'a str,
    at: usize,
    nodes: usize,
}
impl Parser<'_> {
    fn skip(&mut self) {
        while self
            .text
            .as_bytes()
            .get(self.at)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.at += 1
        }
    }
    fn node(&mut self, depth: usize) -> Result<Node> {
        self.nodes += 1;
        if depth > 32 || self.nodes > 8192 {
            return Err("GeoPackage coordinate definition has excessive nesting".into());
        }
        self.skip();
        let start = self.at;
        if self.text.as_bytes().get(self.at) == Some(&b'"') {
            self.at += 1;
            loop {
                match self.text.as_bytes().get(self.at) {
                    None => return Err("Invalid GeoPackage coordinate definition".into()),
                    Some(b'"') => {
                        self.at += 1;
                        if self.text.as_bytes().get(self.at) == Some(&b'"') {
                            self.at += 1
                        } else {
                            break;
                        }
                    }
                    _ => self.at += 1,
                }
            }
            return Ok(Node::Atom(self.text[start..self.at].to_string()));
        }
        while self
            .text
            .as_bytes()
            .get(self.at)
            .is_some_and(|b| !b"[],()".contains(b))
        {
            self.at += 1
        }
        let token = self.text[start..self.at].trim();
        if token.is_empty() {
            return Err("Invalid GeoPackage coordinate definition".into());
        }
        let bracket = self.text.as_bytes().get(self.at).copied();
        if !matches!(bracket, Some(b'[' | b'(')) {
            return Ok(Node::Atom(token.into()));
        }
        let close = if bracket == Some(b'[') { b']' } else { b')' };
        self.at += 1;
        let mut fields = vec![];
        loop {
            self.skip();
            if self.text.as_bytes().get(self.at) == Some(&close) {
                self.at += 1;
                break;
            }
            fields.push(self.node(depth + 1)?);
            self.skip();
            match self.text.as_bytes().get(self.at) {
                Some(b',') => self.at += 1,
                Some(b) if *b == close => {
                    self.at += 1;
                    break;
                }
                _ => return Err("Invalid GeoPackage coordinate definition".into()),
            }
        }
        Ok(Node::Element(token.into(), fields))
    }
}
fn adapt_axes(node: &mut Node) -> Result<()> {
    let Node::Element(name, fields) = node else {
        return Ok(());
    };
    for field in fields.iter_mut() {
        adapt_axes(field)?;
    }
    if ![
        "GEOGCS",
        "GEOGCRS",
        "GEODCRS",
        "GEODETICCRS",
        "PROJCS",
        "PROJCRS",
        "PROJECTEDCRS",
    ]
    .iter()
    .any(|n| name.eq_ignore_ascii_case(n))
    {
        return Ok(());
    }
    let axes = fields
        .iter()
        .enumerate()
        .filter(|(_, f)| f.name("AXIS"))
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    if axes.is_empty() {
        return Ok(());
    }
    if !(2..=3).contains(&axes.len()) {
        return Err("Unsupported GeoPackage SRS axes".into());
    }
    let direction = |i| match &fields[i] {
        Node::Element(_, f) => f.get(1).and_then(Node::atom).map(str::to_ascii_lowercase),
        _ => None,
    };
    match (direction(axes[0]).as_deref(), direction(axes[1]).as_deref()) {
        (Some("east"), Some("north")) => {}
        (Some("north"), Some("east")) => fields.swap(axes[0], axes[1]),
        _ => return Err("Unsupported GeoPackage SRS axis directions".into()),
    }
    for (order, index) in axes.iter().enumerate() {
        if let Node::Element(_, f) = &mut fields[*index] {
            for child in f {
                if let Node::Element(n, values) = child {
                    if n.eq_ignore_ascii_case("ORDER") {
                        *values = vec![Node::Atom((order + 1).to_string())];
                    }
                }
            }
        }
    }
    Ok(())
}
fn key(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect()
}
fn mercator_extension(s: &str) -> bool {
    let mut pairs = BTreeMap::new();
    for token in s.split_whitespace() {
        let Some(token) = token.strip_prefix('+') else {
            return false;
        };
        let (k, v) = token.split_once('=').unwrap_or((token, ""));
        if pairs.insert(k, v).is_some() {
            return false;
        }
    }
    if pairs.get("proj") != Some(&"merc")
        || pairs.get("a") != Some(&"6378137")
        || pairs.get("b") != Some(&"6378137")
        || pairs.get("units") != Some(&"m")
    {
        return false;
    }
    pairs.into_iter().all(|(k, v)| match k {
        "proj" => v == "merc",
        "a" | "b" => v == "6378137",
        "units" => v == "m",
        "lat_ts" | "lon_0" | "x_0" | "y_0" => v.parse::<f64>() == Ok(0.),
        "k" | "k_0" => v.parse::<f64>() == Ok(1.),
        "nadgrids" => v == "@null",
        "wktext" | "no_defs" | "type" => v.is_empty() || k == "type" && v == "crs",
        _ => false,
    })
}
fn web_mercator(node: &mut Node, epsg: i32) -> Result<()> {
    let Node::Element(_, fields) = node else {
        return Ok(());
    };
    if epsg != 3857 {
        return Ok(());
    }
    // Honor GDAL's WKT1 spherical extension only with standard parameters.
    let sphere=fields.iter().any(|f|matches!(f,Node::Element(n,v) if n.eq_ignore_ascii_case("EXTENSION")&&v.first().and_then(Node::atom)==Some("PROJ4")&&v.get(1).and_then(Node::atom).is_some_and(mercator_extension)));
    if sphere {
        for f in fields.iter_mut() {
            if let Node::Element(n, v) = f {
                if n.eq_ignore_ascii_case("PROJECTION")
                    && v.first()
                        .and_then(Node::atom)
                        .is_some_and(|s| key(s) == "mercator1sp")
                {
                    *v = vec![Node::Atom(
                        "\"Popular_Visualisation_Pseudo_Mercator\"".into(),
                    )];
                }
            }
        }
    }
    fn check(n: &Node) -> Result<()> {
        let Node::Element(name, fields) = n else {
            return Ok(());
        };
        if name.eq_ignore_ascii_case("PARAMETER") {
            let k = fields
                .first()
                .and_then(Node::atom)
                .ok_or("Invalid GeoPackage projection parameter")?;
            let value = fields
                .get(1)
                .and_then(Node::atom)
                .and_then(|v| v.parse::<f64>().ok())
                .ok_or("Invalid GeoPackage projection parameter")?;
            let expected = match key(k).as_str() {
                "centralmeridian"
                | "longitudeofnaturalorigin"
                | "latitudeoforigin"
                | "latitudeofnaturalorigin"
                | "falseeasting"
                | "falsenorthing" => 0.,
                "scalefactor" | "scalefactoratnaturalorigin" => 1.,
                _ => return Err("Unsupported Pseudo-Mercator parameter".into()),
            };
            if value != expected {
                return Err("GeoPackage EPSG identifier disagrees with its WKT definition".into());
            }
        }
        for child in fields {
            check(child)?;
        }
        Ok(())
    }
    check(node)
}
pub(super) fn definition(wkt: &str, organization: &str, epsg: i32) -> Result<String> {
    let mut parser = Parser {
        text: wkt,
        at: 0,
        nodes: 0,
    };
    let mut node = parser.node(0)?;
    parser.skip();
    if parser.at != wkt.len() {
        return Err("Invalid GeoPackage coordinate definition".into());
    }
    adapt_axes(&mut node)?;
    if organization.eq_ignore_ascii_case("EPSG") {
        web_mercator(&mut node, epsg)?;
    }
    Ok(node.write())
}
