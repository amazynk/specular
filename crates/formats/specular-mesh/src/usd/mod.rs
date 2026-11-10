//! Universal Scene Description: USDA text layers, USDC binary ("crate") layers and USDZ
//! packages (a ZIP whose first USD file is the root layer, with the layers and textures it
//! uses beside it). The layers are composed into a stage ([`stage`]) and its meshes, implicit
//! shapes and instances read into one model with their transforms, materials and textures.
//! Layers and textures outside a package come only through the [`Resolver`], so only from the
//! model's own folder.

mod crate_file;
mod lz4;
mod stage;
mod usda;
mod value;

pub(crate) use stage::unit_of;

use std::collections::HashMap;
use std::io::{Cursor, Read};

use crate::builder::Builder;
use crate::{Error, Limits, Mesh, Resolver, Result};

/// Parses one layer, text or binary, by its signature.
pub(crate) fn parse_layer(bytes: &[u8]) -> Result<value::Layer> {
    if bytes.starts_with(b"#usda") {
        usda::parse(bytes)
    } else if bytes.starts_with(b"PXR-USDC") {
        crate_file::parse(bytes)
    } else {
        Err(Error::Malformed("not a USD layer".into()))
    }
}

/// Layers and textures beside the root file, by path relative to its folder.
struct Folder<'a> {
    resolver: &'a dyn Resolver,
}

impl stage::Files for Folder<'_> {
    fn read(&self, id: &str, max: usize) -> std::result::Result<Vec<u8>, String> {
        self.resolver.read(id, max)
    }
}

/// The files of a USDZ package, by path.
struct Package {
    files: HashMap<String, Vec<u8>>,
}

impl stage::Files for Package {
    fn read(&self, id: &str, max: usize) -> std::result::Result<Vec<u8>, String> {
        match self.files.get(id) {
            Some(b) if b.len() <= max => Ok(b.clone()),
            Some(_) => Err("too large".into()),
            None => Err("not in the package".into()),
        }
    }
}

/// Reads a USDA or USDC file (its side layers and textures through `resolver`).
pub(crate) fn read(bytes: &[u8], resolver: &dyn Resolver, limits: &Limits) -> Result<Mesh> {
    let layer = parse_layer(bytes)?;
    let files = Folder { resolver };
    // Side layers are named relative to the root's folder.
    compose(layer, "root", &files, limits)
}

/// Reads a USDZ package.
pub(crate) fn read_usdz(bytes: &[u8], limits: &Limits) -> Result<Mesh> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| Error::Malformed(format!("USDZ package: {e}")))?;
    let mut files = HashMap::new();
    let mut root = None;
    let mut budget = limits.max_unpacked;
    for i in 0..zip.len() {
        let mut f = zip
            .by_index(i)
            .map_err(|e| Error::Malformed(format!("USDZ package: {e}")))?;
        if !f.is_file() {
            continue;
        }
        let name = f.name().trim_start_matches('/').replace('\\', "/");
        let mut data = Vec::new();
        (&mut f)
            .take(budget as u64 + 1)
            .read_to_end(&mut data)
            .map_err(|e| Error::Malformed(format!("{name}: {e}")))?;
        if data.len() > budget {
            return Err(Error::TooLarge("USDZ package unpacks too large".into()));
        }
        budget -= data.len();
        let lower = name.to_ascii_lowercase();
        if root.is_none()
            && (lower.ends_with(".usd") || lower.ends_with(".usda") || lower.ends_with(".usdc"))
        {
            root = Some(name.clone());
        }
        files.insert(name, data);
    }
    let root = root.ok_or_else(|| Error::Malformed("USDZ package without a USD layer".into()))?;
    let layer = parse_layer(&files[&root])?;
    let pkg = Package { files };
    compose(layer, &root, &pkg, limits)
}

fn compose(
    layer: value::Layer,
    name: &str,
    files: &dyn stage::Files,
    limits: &Limits,
) -> Result<Mesh> {
    let mut b = Builder::new(limits);
    b.up = stage::up_of(&layer);
    b.unit = stage::unit_of(
        layer
            .meta("metersPerUnit")
            .and_then(value::V::as_f64)
            .unwrap_or(0.01),
    );
    b.title = layer
        .meta("defaultPrim")
        .and_then(value::V::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    b.generator = layer
        .meta("customLayerData")
        .and_then(|d| d.dict_get("creator"))
        .and_then(value::V::as_str)
        .or_else(|| {
            layer
                .meta("documentation")
                .and_then(value::V::as_str)
                .filter(|d| d.len() <= 120 && !d.contains('\n'))
        })
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    let mut st = stage::Stage::open(files, name, layer, limits.max_side_file);
    st.build(&mut b)?;
    b.finish()
}
