//! Implicit tiling (3D Tiles 1.1, and the 1.0 `3DTILES_implicit_tiling` extension): a
//! quadtree or octree whose tiles exist where a subtree file's availability bits say, with
//! content and subtree addresses from URI templates.

use serde_json::Value;

use crate::fetch::{self, Fetch};
use crate::volume::Given;
use crate::{Error, MAX_JSON, Result};

/// The shared description of one implicit tree.
#[derive(Clone, Debug)]
pub struct Scheme {
    pub octree: bool,
    pub subtree_levels: u32,
    /// Levels that may hold tiles (the root is level 0).
    pub available_levels: u32,
    /// Templates, still to be resolved against `base` after substitution.
    pub subtree_uri: String,
    pub content_uris: Vec<String>,
    /// The JSON file that declares the tree.
    pub base: String,
    /// The root's volume as given and its geometric error.
    pub given: Given,
    pub error: f64,
}

impl Scheme {
    /// Children per tile.
    pub fn branching(&self) -> u64 {
        if self.octree { 8 } else { 4 }
    }

    /// Reads `implicitTiling` (or the 1.0 extension) of a tile.
    pub fn parse(
        imp: &Value,
        tile: &Value,
        base: &str,
        given: Given,
        error: f64,
    ) -> Result<Scheme> {
        let scheme = imp
            .get("subdivisionScheme")
            .and_then(Value::as_str)
            .unwrap_or("");
        let octree = match scheme {
            "QUADTREE" => false,
            "OCTREE" => true,
            s => {
                return Err(Error::Unsupported(format!(
                    "an implicit tree subdivided as “{s}”"
                )));
            }
        };
        let subtree_levels = imp
            .get("subtreeLevels")
            .and_then(Value::as_u64)
            .filter(|&n| (1..=16).contains(&n))
            .ok_or_else(|| Error::Malformed("implicit tiling without subtree levels".into()))?
            as u32;
        let available_levels = match imp.get("availableLevels").and_then(Value::as_u64) {
            Some(n) => n,
            None => imp
                .get("maximumLevel")
                .and_then(Value::as_u64)
                .map(|m| m + 1)
                .ok_or_else(|| Error::Malformed("implicit tiling without levels".into()))?,
        }
        .min(31) as u32;
        let subtree_uri = imp
            .pointer("/subtrees/uri")
            .or_else(|| imp.pointer("/subtrees/url"))
            .and_then(Value::as_str)
            .ok_or_else(|| Error::Malformed("implicit tiling without a subtree address".into()))?
            .to_owned();
        if matches!(given, Given::Sphere(_)) {
            return Err(Error::Unsupported(
                "an implicit tree bounded by a sphere (the standard allows only boxes and regions)"
                    .into(),
            ));
        }
        let content_uris = crate::tileset::content_uris(tile);
        Ok(Scheme {
            octree,
            subtree_levels,
            available_levels,
            subtree_uri,
            content_uris,
            base: base.to_owned(),
            given,
            error,
        })
    }

    /// A template filled in for one tile.
    pub fn fill(template: &str, level: u32, xyz: [u64; 3]) -> String {
        template
            .replace("{level}", &level.to_string())
            .replace("{x}", &xyz[0].to_string())
            .replace("{y}", &xyz[1].to_string())
            .replace("{z}", &xyz[2].to_string())
    }
}

/// Availability: all, none, or one bit per item.
#[derive(Clone, Debug)]
pub enum Bits {
    Constant(bool),
    Stream(Vec<u8>),
}

impl Bits {
    pub fn get(&self, i: u64) -> bool {
        match self {
            Bits::Constant(b) => *b,
            Bits::Stream(s) => s
                .get((i / 8) as usize)
                .is_some_and(|b| b & (1 << (i % 8)) != 0),
        }
    }
}

/// One subtree file.
#[derive(Clone, Debug)]
pub struct Subtree {
    pub tiles: Bits,
    pub contents: Vec<Bits>,
    pub children: Bits,
}

/// Bits of `x`, `y` (and `z`) interleaved, x lowest.
pub fn morton(xyz: [u64; 3], octree: bool) -> u64 {
    let mut out = 0u64;
    let dims = if octree { 3 } else { 2 };
    for bit in 0..(64 / dims) {
        for (d, v) in xyz.iter().take(dims).enumerate() {
            out |= ((v >> bit) & 1) << (bit * dims + d);
        }
    }
    out
}

/// Where level `l` of a subtree starts in its tile bits: (N^l − 1)/(N − 1).
pub fn level_offset(l: u32, branching: u64) -> u64 {
    (branching.pow(l) - 1) / (branching - 1)
}

/// Reads a subtree file (binary `subt`, or its JSON form), with external buffers resolved
/// against `key`.
pub fn read_subtree(bytes: &[u8], key: &str, fetch: &dyn Fetch) -> Result<Subtree> {
    let (json, bin): (&[u8], &[u8]) = if bytes.starts_with(b"subt") {
        let u64_at = |o: usize| {
            bytes
                .get(o..o + 8)
                .map(|s| u64::from_le_bytes(s.try_into().unwrap_or([0; 8])))
        };
        let jl = u64_at(8).ok_or_else(|| Error::Malformed("short subtree file".into()))?;
        let bl = u64_at(16).ok_or_else(|| Error::Malformed("short subtree file".into()))?;
        let j_end = 24usize
            .checked_add(usize::try_from(jl).unwrap_or(usize::MAX))
            .filter(|&e| e <= bytes.len())
            .ok_or_else(|| Error::Malformed("subtree JSON runs past the end".into()))?;
        let b_end = j_end
            .checked_add(usize::try_from(bl).unwrap_or(usize::MAX))
            .filter(|&e| e <= bytes.len())
            .ok_or_else(|| Error::Malformed("subtree data runs past the end".into()))?;
        (&bytes[24..j_end], &bytes[j_end..b_end])
    } else {
        (bytes, &[])
    };
    let doc: Value =
        serde_json::from_slice(json).map_err(|e| Error::Malformed(format!("subtree JSON: {e}")))?;
    let arr = |k: &str| {
        doc.get(k)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let mut buffers: Vec<Option<Vec<u8>>> = Vec::new();
    for (i, b) in arr("buffers").iter().enumerate() {
        let data = match b.get("uri").and_then(Value::as_str) {
            None if i == 0 => Some(bin.to_vec()),
            None => None,
            Some(u) => fetch::resolve(key, u).and_then(|k| fetch::load(fetch, &k, MAX_JSON).ok()),
        };
        buffers.push(data);
    }
    let views = arr("bufferViews");
    let bits = |v: Option<&Value>| -> Result<Bits> {
        let v = v.ok_or_else(|| Error::Malformed("subtree without availability".into()))?;
        if let Some(c) = v.get("constant").and_then(Value::as_u64) {
            return Ok(Bits::Constant(c == 1));
        }
        let vi = v
            .get("bitstream")
            .or_else(|| v.get("bufferView"))
            .and_then(Value::as_u64)
            .ok_or_else(|| Error::Malformed("availability without bits".into()))?;
        let view = views
            .get(vi as usize)
            .ok_or_else(|| Error::Malformed("availability names a missing view".into()))?;
        let buf = view.get("buffer").and_then(Value::as_u64).unwrap_or(0) as usize;
        let off = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
        let len = view.get("byteLength").and_then(Value::as_u64).unwrap_or(0) as usize;
        let data = buffers
            .get(buf)
            .and_then(Option::as_ref)
            .and_then(|b| b.get(off..off.checked_add(len)?))
            .ok_or_else(|| Error::Malformed("availability bits are missing".into()))?;
        Ok(Bits::Stream(data.to_vec()))
    };
    let tiles = bits(doc.get("tileAvailability"))?;
    let contents = match doc
        .get("contentAvailability")
        .or_else(|| doc.pointer("/extensions/3DTILES_multiple_contents/contentAvailability"))
    {
        Some(Value::Array(a)) => a.iter().map(|v| bits(Some(v))).collect::<Result<_>>()?,
        Some(v @ Value::Object(_)) => vec![bits(Some(v))?],
        _ => Vec::new(),
    };
    let children = bits(doc.get("childSubtreeAvailability"))?;
    Ok(Subtree {
        tiles,
        contents,
        children,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn morton_order() {
        assert_eq!(morton([1, 0, 0], false), 1);
        assert_eq!(morton([0, 1, 0], false), 2);
        assert_eq!(morton([3, 3, 0], false), 15);
        assert_eq!(morton([0, 0, 1], true), 4);
        assert_eq!(morton([2, 0, 0], true), 8);
        assert_eq!(level_offset(0, 4), 0);
        assert_eq!(level_offset(2, 4), 5);
        assert_eq!(level_offset(2, 8), 9);
    }
}
