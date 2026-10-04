//! Bounded ODL structure parser; nested duplicate values cannot override grid
//! geometry. Only the single reviewed two-dimensional sinusoidal grid is used.
use super::*;

#[derive(Default)]
struct Node {
    values: BTreeMap<String, String>,
    children: Vec<(String, String, Node)>,
}
fn parse(text: &str) -> Result<Node> {
    if !text.is_ascii() || text.len() > MAX_METADATA {
        return Err("Invalid VIIRS ODL encoding or size".into());
    }
    let mut stack = vec![(String::new(), String::new(), Node::default())];
    let mut statement = String::new();
    let mut end = false;
    for line in text.trim_end_matches('\0').lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if end {
            return Err("VIIRS ODL has data after END".into());
        }
        statement.push_str(line);
        if statement.starts_with("/*") {
            return Err("VIIRS ODL comments are unsupported".into());
        }
        // Values may contain multi-line numeric tuples, but not nested tuples.
        if statement.contains('(') && !statement.ends_with(')') {
            continue;
        }
        if statement == "END" {
            if stack.len() != 1 {
                return Err("VIIRS ODL has unclosed groups".into());
            }
            end = true;
            statement.clear();
            continue;
        }
        let (key, value) = statement
            .split_once('=')
            .ok_or("VIIRS ODL assignment is malformed")?;
        let (key, value) = (key.trim(), value.trim());
        if value.is_empty() {
            return Err("VIIRS ODL value is empty".into());
        }
        if matches!(key, "GROUP" | "OBJECT") {
            if stack.len() >= 12 {
                return Err("VIIRS ODL nesting exceeds 12 levels".into());
            }
            stack.push((key.into(), value.into(), Node::default()));
        } else if matches!(key, "END_GROUP" | "END_OBJECT") {
            if stack.len() < 2 {
                return Err("VIIRS ODL closes a missing group".into());
            }
            let node = stack.pop().unwrap();
            if node.0 != key.strip_prefix("END_").unwrap() || node.1 != value {
                return Err("VIIRS ODL group endings differ".into());
            }
            let parent = &mut stack.last_mut().unwrap().2;
            if parent
                .children
                .iter()
                .any(|(kind, name, _)| kind == &node.0 && name == &node.1)
            {
                return Err("VIIRS ODL repeats a group".into());
            }
            parent.children.push(node);
        } else {
            let node = &mut stack.last_mut().unwrap().2;
            if node.values.insert(key.into(), value.into()).is_some() {
                return Err(format!("VIIRS ODL repeats {key}"));
            }
        }
        statement.clear();
    }
    if !end || !statement.is_empty() || stack.len() != 1 {
        return Err("VIIRS ODL is truncated".into());
    }
    Ok(stack.pop().unwrap().2)
}
fn child<'a>(node: &'a Node, name: &str) -> Result<&'a Node> {
    let mut children = node
        .children
        .iter()
        .filter(|(kind, value, _)| kind == "GROUP" && value == name);
    let node = &children
        .next()
        .ok_or_else(|| format!("VIIRS ODL lacks {name}"))?
        .2;
    if children.next().is_some() {
        return Err("VIIRS ODL group is ambiguous".into());
    }
    Ok(node)
}
fn value<'a>(node: &'a Node, key: &str) -> Result<&'a str> {
    node.values
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| format!("VIIRS ODL lacks {key}"))
}
fn tuple(node: &Node, key: &str, length: usize) -> Result<Vec<f64>> {
    let value = value(node, key)?
        .strip_prefix('(')
        .and_then(|v| v.strip_suffix(')'))
        .ok_or("VIIRS ODL numeric tuple is malformed")?;
    let numbers = value
        .split(',')
        .map(|s| s.trim().parse::<f64>().map_err(io_error))
        .collect::<Result<Vec<_>>>()?;
    if numbers.len() != length || numbers.iter().any(|v| !v.is_finite()) {
        return Err("VIIRS ODL numeric tuple is invalid".into());
    }
    Ok(numbers)
}
pub(super) struct Grid {
    pub name: String,
    pub bounds: [f64; 4],
    pub pixel_size: [f64; 2],
}
pub(super) fn grid(text: &str, h: u32, v: u32) -> Result<Grid> {
    let root = parse(text)?;
    let structure = child(&root, "GridStructure")?;
    if structure.children.len() != 1 {
        return Err("VIIRS product must identify one science grid".into());
    }
    let (kind, _, grid) = &structure.children[0];
    let name = value(grid, "GridName")?
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .ok_or("VIIRS grid name is not quoted")?;
    if kind != "GROUP"
        || name.is_empty()
        || name.len() > 80
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        || value(grid, "XDim")? != "1200"
        || value(grid, "YDim")? != "1200"
        || value(grid, "Projection")? != "GCTP_SNSOID"
        || grid
            .values
            .get("GridOrigin")
            .is_some_and(|s| s != "HDFE_GD_UL")
        || grid
            .values
            .get("PixelRegistration")
            .is_some_and(|s| s != "HDFE_CENTER")
    {
        return Err(
            "VIIRS science grid name, dimensions, registration or projection are unsupported"
                .into(),
        );
    }
    let params = tuple(grid, "ProjParams", 13)?;
    let radius = crate::providers::modis::RADIUS;
    let sphere = value(grid, "SphereCode")?;
    if !((sphere == "-1" && (params[0] - radius).abs() < 1e-8)
        || (sphere == "19" && (params[0] == 0.0 || (params[0] - radius).abs() < 1e-8)))
        || params[1..].iter().any(|v| *v != 0.0)
    {
        return Err("VIIRS sinusoidal sphere or projection parameters differ".into());
    }
    let ul = tuple(grid, "UpperLeftPointMtrs", 2)?;
    let lr = tuple(grid, "LowerRightMtrs", 2)?;
    let size = std::f64::consts::PI * radius / 18.0;
    let expected = [
        (f64::from(h) - 18.0) * size,
        (8.0 - f64::from(v)) * size,
        (f64::from(h) - 17.0) * size,
        (9.0 - f64::from(v)) * size,
    ];
    let bounds = [ul[0], lr[1], lr[0], ul[1]];
    if h >= 36
        || v >= 18
        || bounds
            .iter()
            .zip(expected)
            .any(|(actual, expected)| (*actual - expected).abs() > 0.02)
    {
        return Err("VIIRS embedded geometry differs from the horizontal/vertical tile".into());
    }
    let fields = child(grid, "DataField")?;
    for name in ["SurfReflect_M5", "SurfReflect_M4", "SurfReflect_M3"] {
        let matches: Vec<_> = fields
            .children
            .iter()
            .filter(|(_, _, node)| {
                node.values
                    .get("DataFieldName")
                    .is_some_and(|s| s == &format!("\"{name}\""))
            })
            .collect();
        if matches.len() != 1
            || matches[0].0 != "OBJECT"
            || value(&matches[0].2, "DataType")? != "DFNT_INT16"
            || value(&matches[0].2, "DimList")?.replace(' ', "") != "(\"YDim\",\"XDim\")"
        {
            return Err(format!(
                "VIIRS ODL {name} field type or dimension order differs"
            ));
        }
    }
    Ok(Grid {
        name: name.into(),
        bounds,
        pixel_size: [(lr[0] - ul[0]) / 1200.0, (ul[1] - lr[1]) / 1200.0],
    })
}
