//! Krita paint layers: the layer list of `maindoc.xml` and the tiled layer data in
//! `<image>/layers/<file>`.
//!
//! A layer file (version 2) is a short text header (`VERSION 2`, `TILEWIDTH`, `TILEHEIGHT`,
//! `PIXELSIZE`, `DATA <tiles>`), then per tile a line `x,y,LZF,<n>` and `n` bytes: a flag
//! byte (1: LZF-compressed, 0: raw) and the tile's pixels. Compressed tiles hold the pixels
//! byte-planar (every pixel's first byte, then every pixel's second byte, …); raw ones are
//! interleaved. Version 1 files (Krita 1.x) are a tile count, then per tile `x,y,w,h` and
//! the raw interleaved pixels. Tile positions are in layer coordinates; the layer's `x`/`y`
//! in `maindoc.xml` place them on the canvas. Pixels outside every tile are the layer's
//! default pixel (`<file>.defaultpixel`, transparent when missing).
//!
//! Pixels are stored as Krita keeps them in memory: 8- and 16-bit integer RGB as B, G, R, A;
//! half and float RGB as R, G, B, A; grey as Y, A; all little-endian.

use quick_xml::Reader;
use quick_xml::events::Event;
use rayon::prelude::*;

use crate::stack::{attrs, get};
use crate::{Error, MAX_CANVAS_PIXELS, MAX_LAYERS};

/// Tiles in one layer file at most.
const MAX_TILES: usize = 1 << 22;
/// Largest tile side.
const MAX_TILE_SIDE: u32 = 1024;

/// A colour model and depth of Krita's that layer pages decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KraPixel {
    /// `RGBA`: B, G, R, A bytes.
    Rgba8,
    /// `RGBA16`: B, G, R, A `u16`.
    Rgba16,
    /// `RGBAF16`: R, G, B, A halves.
    RgbaF16,
    /// `RGBAF32`: R, G, B, A floats.
    RgbaF32,
    /// `GRAYA`: Y, A bytes.
    GrayA8,
    /// `GRAYAU16`: Y, A `u16`.
    GrayA16,
    /// `GRAYAF16`: Y, A halves.
    GrayAF16,
    /// `GRAYAF32`: Y, A floats.
    GrayAF32,
}

impl KraPixel {
    /// From Krita's colour space id (`colorspacename`); CMYK, Lab, XYZ and YCbCr are `None`.
    pub fn from_name(name: &str) -> Option<KraPixel> {
        Some(match name {
            "RGBA" => KraPixel::Rgba8,
            "RGBA16" => KraPixel::Rgba16,
            "RGBAF16" => KraPixel::RgbaF16,
            "RGBAF32" => KraPixel::RgbaF32,
            "GRAYA" => KraPixel::GrayA8,
            "GRAYAU16" | "GRAYA16" => KraPixel::GrayA16,
            "GRAYAF16" => KraPixel::GrayAF16,
            "GRAYAF32" => KraPixel::GrayAF32,
            _ => return None,
        })
    }

    /// Bytes per stored pixel.
    pub fn size(self) -> usize {
        match self {
            KraPixel::GrayA8 => 2,
            KraPixel::Rgba8 | KraPixel::GrayA16 | KraPixel::GrayAF16 => 4,
            KraPixel::Rgba16 | KraPixel::RgbaF16 | KraPixel::GrayAF32 => 8,
            KraPixel::RgbaF32 => 16,
        }
    }

    pub fn is_float(self) -> bool {
        matches!(
            self,
            KraPixel::RgbaF16 | KraPixel::RgbaF32 | KraPixel::GrayAF16 | KraPixel::GrayAF32
        )
    }

    pub fn is_gray(self) -> bool {
        matches!(
            self,
            KraPixel::GrayA8 | KraPixel::GrayA16 | KraPixel::GrayAF16 | KraPixel::GrayAF32
        )
    }
}

/// The pixel format of a decoded layer page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KraOutput {
    /// Straight RGBA8.
    Rgba8,
    /// Straight native-endian `u16` RGBA, as coded (the layer's profile applies).
    Rgba16,
    /// Straight native-endian half-float RGBA, linear light.
    Rgba16F,
    /// Straight native-endian `u16` grey and alpha, as coded (or sRGB-coded from linear
    /// float grey).
    GrayAlpha16,
}

impl KraOutput {
    pub fn bytes(self) -> usize {
        match self {
            KraOutput::Rgba8 | KraOutput::GrayAlpha16 => 4,
            KraOutput::Rgba16 | KraOutput::Rgba16F => 8,
        }
    }
}

/// A Krita layer, as a page shows it: a paint layer, or a group layer (its children
/// composited, [`crate::Document::kra_layer_page`]). Every node of the layer tree carries
/// one ([`KraNode::layer`]); for nodes that are neither, only the name, visibility,
/// opacity and blend mode mean anything.
#[derive(Clone, Debug, PartialEq)]
pub struct KraLayer {
    pub name: String,
    /// The tiled pixel data member, `<image>/layers/<file>` (empty for a group).
    pub member: String,
    /// Canvas position of the layer's origin.
    pub x: i64,
    pub y: i64,
    /// 0..=1.
    pub opacity: f32,
    /// Its own `visible` (a hidden group above it is not counted).
    pub visible: bool,
    /// The layer's pixels; for a group, the image's colour model (what it composites in).
    pub pixel: KraPixel,
    /// Float pixels hold linear light (the document's profile is a linear one).
    pub linear: bool,
    /// Krita's composite op id (`compositeop`: `normal`, `multiply`, `burn`, …).
    pub blend: String,
    /// "Inherit alpha": the alpha bit of `channelflags` is off, so the layer paints only
    /// where what is below it is opaque, keeping that alpha.
    pub inherit_alpha: bool,
    /// Its visible transparency masks.
    pub masks: Vec<KraMask>,
    /// The visible masks that are not applied (selection, filter, transform and colorize
    /// masks): Krita's node type of each and its name.
    pub skipped_masks: Vec<(String, String)>,
    /// Its node in [`crate::Document::kra_nodes`].
    pub node: usize,
    /// A group layer.
    pub group: bool,
    /// Animated (`keyframes`): the pixel data read is its first keyframe's.
    pub animated: bool,
    /// A layer style (drop shadow, stroke, …, in `annotations/layerstyles.asl`) is attached;
    /// it is not drawn.
    pub style: bool,
}

impl KraLayer {
    /// The pixel format [`crate::Document::kra_layer_page`] decodes this layer to.
    pub fn output(&self) -> KraOutput {
        output_for(self.pixel, self.linear)
    }
}

/// The pixel format pixels of `pixel` decode to.
pub(crate) fn output_for(pixel: KraPixel, linear: bool) -> KraOutput {
    match pixel {
        KraPixel::Rgba8 => KraOutput::Rgba8,
        KraPixel::Rgba16 => KraOutput::Rgba16,
        KraPixel::RgbaF16 | KraPixel::RgbaF32 if linear => KraOutput::Rgba16F,
        KraPixel::RgbaF16 | KraPixel::RgbaF32 => KraOutput::Rgba16,
        _ => KraOutput::GrayAlpha16,
    }
}

/// A transparency mask: an 8-bit selection (`<image>/layers/<file>.pixelselection`, tiled
/// like layer data, one byte per pixel) whose value scales the alpha of what it masks.
#[derive(Clone, Debug, PartialEq)]
pub struct KraMask {
    pub name: String,
    pub member: String,
    pub x: i64,
    pub y: i64,
    /// A filter mask: the filter it applies to the layer's pixels through its selection
    /// (`member`); `None` for a transparency mask.
    pub filter: Option<KraFilterSpec>,
}

/// A Krita filter (of a filter layer or filter mask) or generator (of a fill layer): its id
/// (`filtername` / `generatorname`) and the member holding its settings
/// (`<image>/layers/<file>.filterconfig`).
#[derive(Clone, Debug, PartialEq)]
pub struct KraFilterSpec {
    pub name: String,
    pub config: String,
}

/// What a node of the layer tree is.
#[derive(Clone, Debug, PartialEq)]
pub enum KraKind {
    /// A paint layer with pixel data of a decoded colour model.
    Paint,
    /// A group layer; a pass-through group composites its children straight into the
    /// group's parent.
    Group { pass_through: bool },
    /// A filter layer: its filter applied to what is below it in its group, drawn over it
    /// through its selection ([`KraLayer::masks`]).
    Filter(KraFilterSpec),
    /// A fill layer: its generator's pixels (a colour, a pattern) through its selection.
    Fill(KraFilterSpec),
    /// A vector layer: its shapes, an SVG document (`<image>/layers/<file>.shapelayer/
    /// content.svg`), drawn through the composite's host.
    Vector { svg: String },
    /// A clone layer: what its source node (`clonefromuuid` / `clonefrom`) shows, at the
    /// clone's offset; `None` when the source is not in the file.
    Clone { source: Option<usize> },
    /// A file layer: an image file (`source`, as stored: usually relative to the document's
    /// folder) read through the composite's host.
    File { source: String },
    /// Anything else, left out of composites: Krita's node type, or `paintlayer` for a paint
    /// layer whose data is missing or whose colour model has no decoder.
    Other(String),
}

/// A node of `maindoc.xml`'s layer tree.
#[derive(Clone, Debug, PartialEq)]
pub struct KraNode {
    pub layer: KraLayer,
    pub kind: KraKind,
    /// Child nodes (indices into the node list), top-most first.
    pub children: Vec<usize>,
    /// Krita's node id (`uuid`), which clone layers name their source by.
    pub uuid: String,
    /// A clone layer's source as stored: its uuid and its name (empty otherwise).
    pub clone_from: (String, String),
}

/// What `maindoc.xml` says: the canvas size, the image name (the folder of its members),
/// the document profile's name and colour model, and the layer tree: every node in
/// document order (a group before its children; top-most first among siblings), and the
/// top level.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MainDoc {
    pub size: Option<(u32, u32)>,
    pub name: String,
    pub profile: String,
    /// The image's colour model, when it has a decoder.
    pub pixel: Option<KraPixel>,
    pub nodes: Vec<KraNode>,
    pub root: Vec<usize>,
    /// The projection's background colour (`ProjectionBackgroundColor`): one pixel of the
    /// image's colour model, as stored; empty when not given.
    pub background: Vec<u8>,
}

impl MainDoc {
    /// The layers shown as pages, in Krita's Layers docker order (depth first, top-most
    /// first): paint layers, and the group layers holding one when the image's colour model
    /// has a decoder.
    pub fn pages(&self) -> Vec<KraLayer> {
        fn walk(d: &MainDoc, ids: &[usize], out: &mut Vec<KraLayer>) -> bool {
            let mut any = false;
            for &i in ids {
                let n = &d.nodes[i];
                match n.kind {
                    KraKind::Paint => {
                        out.push(n.layer.clone());
                        any = true;
                    }
                    KraKind::Group { .. } => {
                        let at = out.len();
                        out.push(n.layer.clone());
                        let inner = walk(d, &n.children, out);
                        any |= inner;
                        if !inner || d.pixel.is_none() {
                            out.remove(at);
                        }
                    }
                    _ => {}
                }
            }
            any
        }
        let mut out = Vec::new();
        walk(self, &self.root, &mut out);
        out
    }
}

/// Whether a profile name names a linear-light profile (Krita's float defaults: `…-g10`,
/// scRGB).
pub(crate) fn linear_profile(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("g10") || n.contains("linear") || n.contains("scrgb")
}

/// Parses `maindoc.xml`. Never fails: what cannot be read is left out. Nodes nested deeper
/// than [`crate::MAX_DEPTH`] and those past [`MAX_LAYERS`] are dropped.
pub fn parse_maindoc(xml: &[u8]) -> MainDoc {
    let mut r = Reader::from_reader(xml);
    let mut buf = Vec::new();
    let mut doc = MainDoc::default();
    let mut image_cs = String::new();
    let mut in_image = false;
    // Open `<layer>` elements: their node, or `None` for one that was dropped.
    let mut open: Vec<Option<usize>> = Vec::new();
    loop {
        let (e, empty) = match r.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => (e, false),
            Ok(Event::Empty(e)) => (e, true),
            Ok(Event::Eof) | Err(_) => break,
            Ok(Event::End(e)) => {
                match e.local_name().as_ref() {
                    b"IMAGE" if in_image => break,
                    b"layer" if in_image => {
                        open.pop();
                    }
                    _ => {}
                }
                buf.clear();
                continue;
            }
            Ok(_) => {
                buf.clear();
                continue;
            }
        };
        let a = attrs(&e);
        match e.local_name().as_ref() {
            b"IMAGE" if !in_image => {
                in_image = true;
                let dim = |k: &[u8]| {
                    get(&a, k)
                        .and_then(|v| v.parse::<u32>().ok())
                        .filter(|&v| v > 0)
                };
                doc.size = dim(b"width").zip(dim(b"height"));
                // Not trimmed: the members sit under the name as written (" " is seen).
                doc.name = a
                    .iter()
                    .find(|(k, _)| k == b"name")
                    .map_or_else(String::new, |(_, v)| v.clone());
                doc.profile = get(&a, b"profile").unwrap_or_default().to_owned();
                image_cs = get(&a, b"colorspacename").unwrap_or_default().to_owned();
                doc.pixel = KraPixel::from_name(&image_cs);
            }
            b"layer" if in_image => {
                let parent = open.last().copied();
                // `matches!`, not `parent != Some(None)`: with the pinned nightly's niche layout
                // (outer `None` stored as -1) LLVM 21+ InstCombine folds that comparison into a
                // `trunc nuw` that is poison for `None`, and opt-level 1 then drops every layer.
                let keep = !matches!(parent, Some(None))
                    && open.len() < crate::MAX_DEPTH
                    && doc.nodes.len() < MAX_LAYERS;
                let id = keep.then_some(doc.nodes.len());
                if !empty {
                    open.push(id);
                }
                if let Some(id) = id {
                    let node = node(&a, id, &doc, &image_cs);
                    doc.nodes.push(node);
                    match parent {
                        Some(Some(p)) => doc.nodes[p].children.push(id),
                        _ => doc.root.push(id),
                    }
                }
            }
            b"ProjectionBackgroundColor" if in_image && open.is_empty() => {
                doc.background = get(&a, b"ColorData").and_then(base64).unwrap_or_default();
            }
            b"mask" if in_image => {
                if let Some(&Some(owner)) = open.last() {
                    let image = doc.name.clone();
                    mask(&a, &image, &mut doc.nodes[owner].layer);
                }
            }
            _ => {}
        }
        buf.clear();
    }
    resolve_clones(&mut doc.nodes);
    doc
}

/// Points every clone layer at its source node: by uuid, else by name (Krita 2 wrote names
/// only). A clone that reaches itself through other clones is left without one.
fn resolve_clones(nodes: &mut [KraNode]) {
    for i in 0..nodes.len() {
        if !matches!(nodes[i].kind, KraKind::Clone { .. }) {
            continue;
        }
        let (uuid, name) = &nodes[i].clone_from;
        let found = nodes
            .iter()
            .position(|n| !uuid.is_empty() && n.uuid == *uuid)
            .or_else(|| {
                nodes
                    .iter()
                    .position(|n| !name.is_empty() && n.layer.name == *name)
            })
            .filter(|&s| s != i);
        nodes[i].kind = KraKind::Clone { source: found };
    }
    for i in 0..nodes.len() {
        let mut at = i;
        let mut steps = 0;
        while let KraKind::Clone { source: Some(s) } = nodes[at].kind {
            at = s;
            steps += 1;
            if at == i || steps > nodes.len() {
                nodes[i].kind = KraKind::Clone { source: None };
                break;
            }
        }
    }
}

/// Standard base64 (padding optional, at most 64 bytes out); `None` when malformed.
fn base64(s: &str) -> Option<Vec<u8>> {
    let s = s.trim().trim_end_matches('=');
    if s.len() > 88 {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        acc = acc << 6 | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

/// A `<layer>` element's node.
fn node(a: &[(Vec<u8>, String)], id: usize, doc: &MainDoc, image_cs: &str) -> KraNode {
    let num = |k: &[u8]| {
        get(a, k)
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite())
    };
    let pos = |k: &[u8]| num(k).map_or(0, |v| v.clamp(-1e9, 1e9) as i64);
    // Krita 1.x wrote no node type: its layers were paint layers.
    let nodetype = get(a, b"nodetype").unwrap_or("paintlayer");
    let file = get(a, b"filename").unwrap_or_default();
    let flags = get(a, b"channelflags").unwrap_or_default();
    let group = nodetype == "grouplayer";
    let pixel = if group {
        doc.pixel
    } else {
        KraPixel::from_name(get(a, b"colorspacename").unwrap_or(image_cs))
    };
    let spec = |k: &[u8]| {
        get(a, k)
            .filter(|n| !n.is_empty() && !file.is_empty())
            .map(|n| KraFilterSpec {
                name: n.to_owned(),
                config: format!("{}/layers/{file}.filterconfig", doc.name),
            })
    };
    let source = get(a, b"source").unwrap_or_default();
    let kind = match (nodetype, spec(b"filtername"), spec(b"generatorname")) {
        ("paintlayer", ..) if !file.is_empty() && pixel.is_some() => KraKind::Paint,
        ("grouplayer", ..) => KraKind::Group {
            pass_through: get(a, b"passthrough") == Some("1"),
        },
        ("adjustmentlayer", Some(f), _) => KraKind::Filter(f),
        ("generatorlayer", _, Some(g)) => KraKind::Fill(g),
        ("shapelayer", ..) if !file.is_empty() => KraKind::Vector {
            svg: format!("{}/layers/{file}.shapelayer/content.svg", doc.name),
        },
        // Resolved once the whole tree is read (`resolve_clones`).
        ("clonelayer", ..) => KraKind::Clone { source: None },
        ("filelayer", ..) if !source.is_empty() => KraKind::File {
            source: source.to_owned(),
        },
        (t, ..) => KraKind::Other(t.to_owned()),
    };
    // A filter or fill layer's selection is a transparency mask of its effect.
    let masks = if matches!(kind, KraKind::Filter(_) | KraKind::Fill(_)) {
        vec![KraMask {
            name: String::new(),
            member: format!("{}/layers/{file}.pixelselection", doc.name),
            x: pos(b"x"),
            y: pos(b"y"),
            filter: None,
        }]
    } else {
        Vec::new()
    };
    KraNode {
        layer: KraLayer {
            name: get(a, b"name").unwrap_or_default().to_owned(),
            member: if kind == KraKind::Paint {
                format!("{}/layers/{file}", doc.name)
            } else {
                String::new()
            },
            x: pos(b"x"),
            y: pos(b"y"),
            opacity: num(b"opacity").map_or(1.0, |v| (v.clamp(0.0, 255.0) / 255.0) as f32),
            visible: get(a, b"visible") != Some("0"),
            pixel: pixel.unwrap_or(KraPixel::Rgba8),
            linear: linear_profile(&doc.profile),
            blend: get(a, b"compositeop").unwrap_or("normal").to_owned(),
            inherit_alpha: flags.len() >= 2 && flags.ends_with('0'),
            masks,
            skipped_masks: Vec::new(),
            node: id,
            group,
            animated: get(a, b"keyframes").is_some_and(|k| !k.is_empty()),
            style: get(a, b"layerstyle").is_some_and(|k| !k.is_empty()),
        },
        uuid: get(a, b"uuid").unwrap_or_default().to_owned(),
        clone_from: if nodetype == "clonelayer" {
            (
                get(a, b"clonefromuuid").unwrap_or_default().to_owned(),
                get(a, b"clonefrom").unwrap_or_default().to_owned(),
            )
        } else {
            Default::default()
        },
        kind,
        children: Vec::new(),
    }
}

/// Adds a `<mask>` element to the layer it belongs to: a visible transparency mask is
/// applied, any other visible mask noted as skipped (16 of each at most).
fn mask(a: &[(Vec<u8>, String)], image: &str, l: &mut KraLayer) {
    if get(a, b"visible") == Some("0") {
        return;
    }
    let pos = |k: &[u8]| {
        get(a, k)
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite())
            .map_or(0, |v| v.clamp(-1e9, 1e9) as i64)
    };
    let name = get(a, b"name").unwrap_or_default().to_owned();
    let file = get(a, b"filename").unwrap_or_default();
    let kind = get(a, b"nodetype").unwrap_or("mask");
    let filter = match get(a, b"filtername") {
        Some(f) if kind == "filtermask" && !f.is_empty() && !file.is_empty() => {
            Some(KraFilterSpec {
                name: f.to_owned(),
                config: format!("{image}/layers/{file}.filterconfig"),
            })
        }
        _ => None,
    };
    if (kind == "transparencymask" || filter.is_some()) && !file.is_empty() {
        if l.masks.len() < 16 {
            l.masks.push(KraMask {
                name,
                member: format!("{image}/layers/{file}.pixelselection"),
                x: pos(b"x"),
                y: pos(b"y"),
                filter,
            });
        }
    } else if l.skipped_masks.len() < 16 {
        l.skipped_masks.push((kind.to_owned(), name));
    }
}

/// A tile of a layer file: its position in layer coordinates, size, whether it is LZF
/// compressed, and its stored pixels.
pub(crate) struct Tile<'a> {
    pub(crate) x: i64,
    pub(crate) y: i64,
    pub(crate) w: u32,
    pub(crate) h: u32,
    pub(crate) lzf: bool,
    pub(crate) data: &'a [u8],
}

/// Reads one `\n`-terminated header line.
fn line<'a>(d: &'a [u8], p: &mut usize) -> Result<&'a [u8], Error> {
    let rest = d.get(*p..).unwrap_or_default();
    let n = rest
        .iter()
        .take(256)
        .position(|&c| c == b'\n')
        .ok_or(if rest.len() < 256 {
            Error::Truncated("Krita layer data")
        } else {
            Error::Corrupt("Krita layer header")
        })?;
    *p += n + 1;
    Ok(rest[..n].trim_ascii())
}

fn num<T: std::str::FromStr>(s: &[u8]) -> Result<T, Error> {
    std::str::from_utf8(s)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .ok_or(Error::Corrupt("Krita layer header"))
}

/// A layer member stored as a plain PNG instead of Krita's tiled data, rewritten as a
/// version 2 layer file of raw 64×64 tiles in `pixel`'s layout (the layer's `x`/`y` still
/// place it); any other member is returned as it is. PNG samples are taken as the layer's
/// stored coding (8- and 16-bit integers as they are, floats as 0..1).
pub(crate) fn plain_tiles<'d>(
    bytes: std::borrow::Cow<'d, [u8]>,
    pixel: KraPixel,
) -> Result<std::borrow::Cow<'d, [u8]>, Error> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Ok(bytes);
    }
    let bad = |e: specular_png::PngError| Error::Png("Krita layer".into(), e.to_string());
    let info = specular_png::read_info(&bytes).map_err(bad)?;
    let (w, h) = (info.width as usize, info.height as usize);
    if (w as u64) * (h as u64) > MAX_CANVAS_PIXELS {
        return Err(Error::TooLarge("Krita layer".into()));
    }
    let img = specular_png::decode(
        &bytes,
        &specular_png::DecodeOptions {
            output: specular_png::Output::Rgba16,
            ..Default::default()
        },
    )
    .map_err(bad)?;
    let plane = &img.planes[0];
    let sample = |x: usize, y: usize, c: usize| -> u16 {
        let o = y * plane.stride + (x * 4 + c) * 2;
        u16::from_ne_bytes([plane.data[o], plane.data[o + 1]])
    };
    let ps = pixel.size();
    const T: usize = 64;
    let (tx, ty) = (w.div_ceil(T), h.div_ceil(T));
    let mut out = format!(
        "VERSION 2\nTILEWIDTH {T}\nTILEHEIGHT {T}\nPIXELSIZE {ps}\nDATA {}\n",
        tx * ty
    )
    .into_bytes();
    out.try_reserve(tx * ty * (T * T * ps + 32))
        .map_err(|_| Error::TooLarge("Krita layer".into()))?;
    let mut px = vec![0u8; ps];
    for j in 0..ty {
        for i in 0..tx {
            out.extend_from_slice(
                format!("{},{},LZF,{}\n", i * T, j * T, T * T * ps + 1).as_bytes(),
            );
            out.push(0);
            for y in j * T..(j + 1) * T {
                for x in i * T..(i + 1) * T {
                    let v = if x < w && y < h {
                        [0, 1, 2, 3].map(|c| sample(x, y, c))
                    } else {
                        [0; 4]
                    };
                    store_pixel(pixel, v, &mut px);
                    out.extend_from_slice(&px);
                }
            }
        }
    }
    Ok(std::borrow::Cow::Owned(out))
}

/// 16-bit straight RGBA as a stored pixel of `pixel` (grey takes the Rec. 709 luma).
fn store_pixel(pixel: KraPixel, v: [u16; 4], o: &mut [u8]) {
    let f = v.map(|x| f32::from(x) / 65535.0);
    let grey16 = ((f32::from(v[0]) * 0.2126 + f32::from(v[1]) * 0.7152 + f32::from(v[2]) * 0.0722)
        + 0.5) as u16;
    let gf = f32::from(grey16) / 65535.0;
    let b8 = |x: u16| ((u32::from(x) * 255 + 32767) / 65535) as u8;
    match pixel {
        KraPixel::Rgba8 => o.copy_from_slice(&[b8(v[2]), b8(v[1]), b8(v[0]), b8(v[3])]),
        KraPixel::Rgba16 => {
            for (k, c) in [2, 1, 0, 3].into_iter().enumerate() {
                o[k * 2..k * 2 + 2].copy_from_slice(&v[c].to_le_bytes());
            }
        }
        KraPixel::RgbaF16 => {
            for c in 0..4 {
                o[c * 2..c * 2 + 2].copy_from_slice(&f32_to_f16(f[c]).to_le_bytes());
            }
        }
        KraPixel::RgbaF32 => {
            for c in 0..4 {
                o[c * 4..c * 4 + 4].copy_from_slice(&f[c].to_le_bytes());
            }
        }
        KraPixel::GrayA8 => o.copy_from_slice(&[b8(grey16), b8(v[3])]),
        KraPixel::GrayA16 => {
            o[..2].copy_from_slice(&grey16.to_le_bytes());
            o[2..4].copy_from_slice(&v[3].to_le_bytes());
        }
        KraPixel::GrayAF16 => {
            o[..2].copy_from_slice(&f32_to_f16(gf).to_le_bytes());
            o[2..4].copy_from_slice(&f32_to_f16(f[3]).to_le_bytes());
        }
        KraPixel::GrayAF32 => {
            o[..4].copy_from_slice(&gf.to_le_bytes());
            o[4..8].copy_from_slice(&f[3].to_le_bytes());
        }
    }
}

/// A `KEY value` header line's value.
fn keyed<T: std::str::FromStr>(l: &[u8], key: &[u8]) -> Result<T, Error> {
    let v = l
        .strip_prefix(key)
        .filter(|v| v.first().is_some_and(u8::is_ascii_whitespace))
        .ok_or(Error::Corrupt("Krita layer header"))?;
    num(v)
}

/// The tiles of a layer file, in file order. A file cut short yields the whole tiles before
/// the cut and `true`.
pub(crate) fn tiles(d: &[u8], pixel_size: usize) -> Result<(Vec<Tile<'_>>, bool), Error> {
    let mut p = 0;
    let first = line(d, &mut p)?;
    let mut out = Vec::new();
    if first.starts_with(b"VERSION") {
        let version: u32 = keyed(first, b"VERSION")?;
        if version != 2 {
            return Err(Error::Unsupported(format!(
                "Krita layer data version {version}"
            )));
        }
        let tw: u32 = keyed(line(d, &mut p)?, b"TILEWIDTH")?;
        let th: u32 = keyed(line(d, &mut p)?, b"TILEHEIGHT")?;
        let ps: usize = keyed(line(d, &mut p)?, b"PIXELSIZE")?;
        let count: usize = keyed(line(d, &mut p)?, b"DATA")?;
        if !(1..=MAX_TILE_SIDE).contains(&tw) || !(1..=MAX_TILE_SIDE).contains(&th) {
            return Err(Error::Unsupported(format!("Krita {tw}×{th} tiles")));
        }
        if ps != pixel_size {
            return Err(Error::Corrupt("Krita layer pixel size"));
        }
        if count > MAX_TILES {
            return Err(Error::TooLarge("Krita layer tile count".into()));
        }
        out.reserve(count.min(d.len() / 16));
        for _ in 0..count {
            let l = match line(d, &mut p) {
                Ok(l) => l,
                Err(Error::Truncated(_)) => return Ok((out, true)),
                Err(e) => return Err(e),
            };
            let mut f = l.split(|&c| c == b',');
            let (Some(x), Some(y), Some(method), Some(n), None) =
                (f.next(), f.next(), f.next(), f.next(), f.next())
            else {
                return Err(Error::Corrupt("Krita tile header"));
            };
            if method != b"LZF" {
                return Err(Error::Unsupported(format!(
                    "Krita tile compression {}",
                    String::from_utf8_lossy(method)
                )));
            }
            let (x, y, n): (i64, i64, usize) = (num(x)?, num(y)?, num(n)?);
            let Some(body) = d.get(p..p.saturating_add(n)) else {
                return Ok((out, true));
            };
            p += n;
            let Some((&flag, data)) = body.split_first() else {
                return Err(Error::Corrupt("Krita tile"));
            };
            out.push(Tile {
                x,
                y,
                w: tw,
                h: th,
                lzf: flag == 1,
                data,
            });
        }
    } else {
        // Version 1: a tile count, then `x,y,w,h` and raw pixels per tile.
        let count: usize = num(first)?;
        if count > MAX_TILES {
            return Err(Error::TooLarge("Krita layer tile count".into()));
        }
        for _ in 0..count {
            let l = match line(d, &mut p) {
                Ok(l) => l,
                Err(Error::Truncated(_)) => return Ok((out, true)),
                Err(e) => return Err(e),
            };
            let f: Vec<&[u8]> = l.split(|&c| c == b',').collect();
            let [x, y, w, h] = f[..] else {
                return Err(Error::Corrupt("Krita tile header"));
            };
            let (x, y, w, h): (i64, i64, u32, u32) = (num(x)?, num(y)?, num(w)?, num(h)?);
            if !(1..=MAX_TILE_SIDE).contains(&w) || !(1..=MAX_TILE_SIDE).contains(&h) {
                return Err(Error::Unsupported(format!("Krita {w}×{h} tiles")));
            }
            let n = w as usize * h as usize * pixel_size;
            let Some(data) = d.get(p..p + n) else {
                return Ok((out, true));
            };
            p += n;
            out.push(Tile {
                x,
                y,
                w,
                h,
                lzf: false,
                data,
            });
        }
    }
    Ok((out, false))
}

/// LZF decompression of `src` into `dst` (cleared first), which must come out exactly
/// `size` bytes long. Each control byte below 32 starts a run of that many plus one
/// literals; any other is a back reference of `(ctrl >> 5) + 2` bytes (a 7 there adds the
/// next byte to the length) at distance `((ctrl & 31) << 8 | next) + 1`.
pub fn lzf_decompress(src: &[u8], dst: &mut Vec<u8>, size: usize) -> Result<(), Error> {
    const BAD: Error = Error::Corrupt("Krita LZF tile");
    dst.clear();
    dst.reserve(size);
    let mut i = 0;
    while i < src.len() {
        let ctrl = usize::from(src[i]);
        i += 1;
        if ctrl < 32 {
            let n = ctrl + 1;
            let lit = src.get(i..i + n).ok_or(BAD)?;
            if dst.len() + n > size {
                return Err(BAD);
            }
            dst.extend_from_slice(lit);
            i += n;
        } else {
            let mut len = ctrl >> 5;
            if len == 7 {
                len += usize::from(*src.get(i).ok_or(BAD)?);
                i += 1;
            }
            len += 2;
            let back = ((ctrl & 31) << 8 | usize::from(*src.get(i).ok_or(BAD)?)) + 1;
            i += 1;
            if back > dst.len() || dst.len() + len > size {
                return Err(BAD);
            }
            let from = dst.len() - back;
            if back >= len {
                dst.extend_from_within(from..from + len);
            } else {
                // Overlapping: the run repeats with period `back`.
                for k in 0..len {
                    let b = dst[from + k];
                    dst.push(b);
                }
            }
        }
    }
    if dst.len() != size {
        return Err(BAD);
    }
    Ok(())
}

/// IEEE half from f32, round to nearest even (overflow to infinity, NaN kept).
pub(crate) fn f32_to_f16(v: f32) -> u16 {
    let b = v.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let exp = ((b >> 23) & 0xff) as i32;
    let man = b & 0x7f_ffff;
    if exp == 0xff {
        return sign | 0x7c00 | if man != 0 { 0x200 } else { 0 };
    }
    let e = exp - 127 + 15;
    if e >= 31 {
        return sign | 0x7c00;
    }
    if e <= 0 {
        if e < -10 {
            return sign;
        }
        let m = man | 0x80_0000;
        let shift = (14 - e) as u32;
        let half = 1u32 << (shift - 1);
        let rest = m & ((1 << shift) - 1);
        let mut h = m >> shift;
        if rest > half || (rest == half && h & 1 == 1) {
            h += 1;
        }
        return sign | h as u16;
    }
    let mut h = ((e as u32) << 10) | (man >> 13);
    let rest = man & 0x1fff;
    if rest > 0x1000 || (rest == 0x1000 && h & 1 == 1) {
        h += 1;
    }
    sign | h as u16
}

pub(crate) fn f16_to_f32(h: u16) -> f32 {
    let sign = u32::from(h & 0x8000) << 16;
    let exp = u32::from((h >> 10) & 0x1f);
    let man = u32::from(h & 0x3ff);
    let bits = match exp {
        0 if man == 0 => sign,
        0 => {
            // Subnormal: normalise.
            let lz = man.leading_zeros() - 21;
            sign | ((113 - lz) << 23) | ((man << (lz + 1)) & 0x3ff) << 13
        }
        31 => sign | 0x7f80_0000 | (man << 13),
        e => sign | ((e + 112) << 23) | (man << 13),
    };
    f32::from_bits(bits)
}

/// The sRGB curve, linear 0..=1 to coded 0..=65535.
pub(crate) fn srgb16(v: f32) -> u16 {
    let v = if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) };
    let c = if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (c * 65535.0 + 0.5) as u16
}

pub(crate) fn unit16(v: f32) -> u16 {
    let v = if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) };
    (v * 65535.0 + 0.5) as u16
}

/// Converts stored pixel `s` (interleaved, `pixel.size()` bytes) to `out`
/// (`output.bytes()` bytes) with its alpha times `opacity` (0..=255); `linear` float grey
/// is sRGB-coded.
#[inline]
fn convert(
    pixel: KraPixel,
    linear: bool,
    output: KraOutput,
    opacity: u32,
    s: &[u8],
    out: &mut [u8],
) {
    let u16le = |i: usize| u16::from_le_bytes([s[i], s[i + 1]]);
    let f32le = |i: usize| f32::from_le_bytes([s[i], s[i + 1], s[i + 2], s[i + 3]]);
    let op16 = |a: u16| ((u32::from(a) * opacity + 127) / 255) as u16;
    let opf = |a: f32| a * opacity as f32 / 255.0;
    let put16 = |out: &mut [u8], v: [u16; 4]| {
        for (o, v) in out.chunks_exact_mut(2).zip(v) {
            o.copy_from_slice(&v.to_ne_bytes());
        }
    };
    match (pixel, output) {
        (KraPixel::Rgba8, _) => {
            let a = (u32::from(s[3]) * opacity + 127) / 255;
            out.copy_from_slice(&[s[2], s[1], s[0], a as u8]);
        }
        (KraPixel::Rgba16, _) => {
            put16(out, [u16le(4), u16le(2), u16le(0), op16(u16le(6))]);
        }
        (KraPixel::RgbaF16, KraOutput::Rgba16F) => {
            let a = match opacity {
                255 => u16le(6),
                _ => f32_to_f16(opf(f16_to_f32(u16le(6)))),
            };
            put16(out, [u16le(0), u16le(2), u16le(4), a]);
        }
        (KraPixel::RgbaF16, _) => {
            let f = |i| unit16(f16_to_f32(u16le(i)));
            put16(out, [f(0), f(2), f(4), unit16(opf(f16_to_f32(u16le(6))))]);
        }
        (KraPixel::RgbaF32, KraOutput::Rgba16F) => {
            let f = |i| f32_to_f16(f32le(i));
            put16(out, [f(0), f(4), f(8), f32_to_f16(opf(f32le(12)))]);
        }
        (KraPixel::RgbaF32, _) => {
            let f = |i| unit16(f32le(i));
            put16(out, [f(0), f(4), f(8), unit16(opf(f32le(12)))]);
        }
        (KraPixel::GrayA8, _) => {
            let a = (u32::from(s[1]) * opacity + 127) / 255;
            out[..2].copy_from_slice(&(u16::from(s[0]) * 257).to_ne_bytes());
            out[2..4].copy_from_slice(&((a * 257) as u16).to_ne_bytes());
        }
        (KraPixel::GrayA16, _) => {
            let v = [u16le(0), op16(u16le(2))];
            out[..2].copy_from_slice(&v[0].to_ne_bytes());
            out[2..4].copy_from_slice(&v[1].to_ne_bytes());
        }
        (KraPixel::GrayAF16 | KraPixel::GrayAF32, _) => {
            let (y, a) = if pixel == KraPixel::GrayAF16 {
                (f16_to_f32(u16le(0)), f16_to_f32(u16le(2)))
            } else {
                (f32le(0), f32le(4))
            };
            let y = if linear { srgb16(y) } else { unit16(y) };
            out[..2].copy_from_slice(&y.to_ne_bytes());
            out[2..4].copy_from_slice(&unit16(opf(a)).to_ne_bytes());
        }
    }
}

/// A decoded layer page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KraImage {
    pub width: u32,
    pub height: u32,
    pub format: KraOutput,
    /// Row-major, `format.bytes()` per pixel.
    pub data: Vec<u8>,
    /// The layer data ended early or a tile was corrupt (those pixels are the default pixel).
    pub truncated: bool,
}

/// Layer `l` alone on a `w`×`h` canvas: its tiles from `layer_data` at the layer's offset,
/// the default pixel `default` (or transparent) elsewhere, alpha times the layer opacity.
/// Tiles outside the canvas are not decompressed; tile rows are decoded in parallel.
/// `cancel` is polled between tile rows.
pub(crate) fn layer_page(
    l: &KraLayer,
    layer_data: &[u8],
    default: Option<&[u8]>,
    w: u32,
    h: u32,
    cancel: &(dyn Fn() -> bool + Sync),
) -> Result<KraImage, Error> {
    if u64::from(w) * u64::from(h) > MAX_CANVAS_PIXELS {
        return Err(Error::TooLarge(format!("{w}×{h} canvas")));
    }
    let ps = l.pixel.size();
    let output = l.output();
    let ob = output.bytes();
    let opacity = (l.opacity.clamp(0.0, 1.0) * 255.0).round() as u32;
    let (tiles, mut truncated) = tiles(layer_data, ps)?;
    let size = w as usize * h as usize * ob;
    // Probe first so a canvas that cannot be had is an error, not an abort; the zeroed
    // allocation then maps lazily, its pages first touched by the parallel bands below.
    let mut probe: Vec<u8> = Vec::new();
    probe
        .try_reserve_exact(size)
        .map_err(|_| Error::TooLarge(format!("{w}×{h} canvas")))?;
    drop(probe);
    let mut data = vec![0u8; size];
    // The default pixel, when it is not transparent black.
    let fill = default
        .filter(|d| d.len() == ps && d.iter().any(|&b| b != 0))
        .map(|d| {
            let mut px = vec![0u8; ob];
            convert(l.pixel, l.linear, output, opacity, d, &mut px);
            px
        })
        .filter(|px| px.iter().any(|&b| b != 0));
    if cancel() {
        return Err(Error::Cancelled);
    }
    let (cw, ch) = (i64::from(w), i64::from(h));
    // Tiles touching the canvas, by canvas row of their top edge.
    let mut visible: Vec<&Tile<'_>> = tiles
        .iter()
        .filter(|t| {
            let (x0, y0) = (l.x.saturating_add(t.x), l.y.saturating_add(t.y));
            x0 < cw && y0 < ch && x0 + i64::from(t.w) > 0 && y0 + i64::from(t.h) > 0
        })
        .collect();
    visible.sort_by_key(|t| (t.y, t.x));
    let stride = w as usize * ob;
    // Bands of canvas rows, one per tile row when the tiles sit on one grid of equal rows
    // (always, from Krita); otherwise one band for everything.
    let th = visible.first().map_or(64, |t| t.h);
    let grid = visible.iter().all(|t| {
        t.h == th && t.y.rem_euclid(i64::from(th)) == visible[0].y.rem_euclid(i64::from(th))
    });
    let mut groups: Vec<(i64, Vec<&Tile<'_>>)> = Vec::new();
    for t in visible {
        let key = if grid { t.y } else { 0 };
        match groups.last_mut() {
            Some((k, g)) if *k == key => g.push(t),
            _ => groups.push((key, vec![t])),
        }
    }
    // Split the canvas into the bands' rows (disjoint, top to bottom), and the rows between
    // them into bands of their own (in 64-row pieces) for the default pixel.
    let mut jobs: Vec<(i64, &mut [u8], &[&Tile<'_>])> = Vec::with_capacity(groups.len() * 2 + 1);
    fn gap<'a, T>(
        jobs: &mut Vec<(i64, &'a mut [u8], &'a [T])>,
        top: i64,
        rows: &'a mut [u8],
        stride: usize,
    ) {
        for (k, piece) in rows.chunks_mut(stride.max(1) * 64).enumerate() {
            jobs.push((top + k as i64 * 64, piece, &[]));
        }
    }
    let gaps = fill.is_some();
    let mut rest: &mut [u8] = &mut data;
    let mut row = 0i64;
    for (key, g) in &groups {
        let (top, bottom) = if grid {
            let top = (l.y + key).clamp(row, ch);
            (top, (l.y + key + i64::from(th)).clamp(top, ch))
        } else {
            (0, ch)
        };
        let tail = std::mem::take(&mut rest);
        let (between, tail) = tail.split_at_mut((top - row) as usize * stride);
        if gaps {
            gap(&mut jobs, row, between, stride);
        }
        let (band, tail) = tail.split_at_mut((bottom - top) as usize * stride);
        rest = tail;
        row = bottom;
        jobs.push((top, band, g));
    }
    if gaps {
        gap(&mut jobs, row, rest, stride);
    }
    let bad = jobs
        .into_par_iter()
        .map(|(top, band, g)| -> Result<bool, Error> {
            if cancel() {
                return Err(Error::Cancelled);
            }
            if let Some(px) = &fill {
                for o in band.chunks_exact_mut(ob) {
                    o.copy_from_slice(px);
                }
            }
            let mut buf = Vec::new();
            let mut bad = false;
            for t in g {
                let n = t.w as usize * t.h as usize;
                let src: &[u8] = if t.lzf {
                    if lzf_decompress(t.data, &mut buf, n * ps).is_err() {
                        bad = true;
                        continue;
                    }
                    &buf
                } else {
                    match t.data.get(..n * ps) {
                        Some(d) => d,
                        None => {
                            bad = true;
                            continue;
                        }
                    }
                };
                blit_layer(l, t, src, top, band, w, h, output, opacity);
            }
            Ok(bad)
        })
        .collect::<Result<Vec<bool>, Error>>()?;
    truncated |= bad.iter().any(|&b| b);
    Ok(KraImage {
        width: w,
        height: h,
        format: output,
        data,
        truncated,
    })
}

/// Writes tile `t` (decompressed pixels `src`: byte-planar when LZF, else interleaved) into
/// `band`, the canvas rows from `top` on, through a [`blit`] made for the layer's pixels.
#[allow(clippy::too_many_arguments)]
fn blit_layer(
    l: &KraLayer,
    t: &Tile<'_>,
    src: &[u8],
    top: i64,
    band: &mut [u8],
    w: u32,
    h: u32,
    output: KraOutput,
    opacity: u32,
) {
    let at = (l.x + t.x, l.y + t.y);
    let (ob, linear) = (output.bytes(), l.linear);
    macro_rules! go {
        ($p:expr, $n:literal) => {
            blit::<$n>(t, src, at, top, band, (w, h), ob, |s, o| {
                convert($p, linear, output, opacity, s, o)
            })
        };
    }
    match l.pixel {
        KraPixel::Rgba8 => go!(KraPixel::Rgba8, 4),
        KraPixel::Rgba16 => go!(KraPixel::Rgba16, 8),
        KraPixel::RgbaF16 => go!(KraPixel::RgbaF16, 8),
        KraPixel::RgbaF32 => go!(KraPixel::RgbaF32, 16),
        KraPixel::GrayA8 => go!(KraPixel::GrayA8, 2),
        KraPixel::GrayA16 => go!(KraPixel::GrayA16, 4),
        KraPixel::GrayAF16 => go!(KraPixel::GrayAF16, 4),
        KraPixel::GrayAF32 => go!(KraPixel::GrayAF32, 8),
    }
}

/// [`blit_layer`] for `PS`-byte pixels: tile `t` with its origin at canvas `at`, each pixel
/// converted by `conv` into `ob` bytes.
#[allow(clippy::too_many_arguments)]
#[inline]
fn blit<const PS: usize>(
    t: &Tile<'_>,
    src: &[u8],
    (x0, y0): (i64, i64),
    top: i64,
    band: &mut [u8],
    (w, h): (u32, u32),
    ob: usize,
    conv: impl Fn(&[u8], &mut [u8]),
) {
    let stride = w as usize * ob;
    let rows = (band.len() / stride.max(1)) as i64;
    let n = t.w as usize * t.h as usize;
    let (cx0, cx1) = (x0.max(0), (x0 + i64::from(t.w)).min(i64::from(w)));
    let (cy0, cy1) = (
        y0.max(top),
        (y0 + i64::from(t.h)).min(top + rows).min(i64::from(h)),
    );
    if cx0 >= cx1 {
        return;
    }
    let span = (cx1 - cx0) as usize;
    for cy in cy0..cy1 {
        let i0 = (cy - y0) as usize * t.w as usize + (cx0 - x0) as usize;
        let drow = &mut band[(cy - top) as usize * stride + cx0 as usize * ob..][..span * ob];
        if t.lzf {
            let planes: [&[u8]; PS] = std::array::from_fn(|c| &src[c * n + i0..][..span]);
            for (k, o) in drow.chunks_exact_mut(ob).enumerate() {
                let px: [u8; PS] = std::array::from_fn(|c| planes[c][k]);
                conv(&px, o);
            }
        } else {
            let s = &src[i0 * PS..][..span * PS];
            for (p, o) in s.chunks_exact(PS).zip(drow.chunks_exact_mut(ob)) {
                conv(p, o);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_decodes() {
        assert_eq!(
            base64("lmyWbJZs//8=").unwrap(),
            [0x96, 0x6c, 0x96, 0x6c, 0x96, 0x6c, 0xff, 0xff]
        );
        assert_eq!(base64("AAAAAA==").unwrap(), [0; 4]);
        assert_eq!(base64("QUJD").unwrap(), b"ABC");
        assert!(base64("QU*D").is_none());
    }

    #[test]
    fn halves_round_trip() {
        for v in [0.0f32, 1.0, -2.5, 0.5, 65504.0, 6.1e-5, 1e-7, 0.333] {
            let h = f32_to_f16(v);
            let back = f16_to_f32(h);
            assert!((back - v).abs() <= v.abs() * 1e-3 + 1e-7, "{v} {back}");
        }
        assert_eq!(f32_to_f16(1.0), 0x3c00);
        assert_eq!(f32_to_f16(1e6), 0x7c00);
        assert_eq!(f16_to_f32(0x0001), 2f32.powi(-24));
    }

    fn group(name: &str) -> String {
        format!(r#"<layer name="{name}" nodetype="grouplayer" filename="{name}"><layers>"#)
    }

    fn paint(name: &str) -> String {
        format!(r#"<layer name="{name}" nodetype="paintlayer" filename="{name}"/>"#)
    }

    const GROUP_END: &str = "</layers></layer>";

    fn names(doc: &MainDoc, ids: &[usize]) -> Vec<String> {
        ids.iter()
            .map(|&i| doc.nodes[i].layer.name.clone())
            .collect()
    }

    /// A layer under a dropped one is dropped too, never moved to the top level, and the
    /// tree picks up again once the dropped element closes.
    #[test]
    fn layers_under_dropped_nodes_are_dropped() {
        let deep = crate::MAX_DEPTH + 2;
        let mut xml = String::from(
            r#"<DOC><IMAGE name="i" width="4" height="4" colorspacename="RGBA"><layers>"#,
        );
        for d in 0..deep {
            xml += &group(&format!("g{d}"));
            xml += &paint(&format!("p{d}"));
        }
        xml += &GROUP_END.repeat(deep);
        xml += &paint("after");
        xml += "</layers></IMAGE></DOC>";
        let doc = parse_maindoc(xml.as_bytes());
        // A node sits at the depth of the open `<layer>` elements around it and is kept while
        // that is under MAX_DEPTH: g{d} is at depth d and p{d} at d + 1, so g0..g{M-1},
        // p0..p{M-2} and "after" are kept, and g{M-1} is left with no children.
        let m = crate::MAX_DEPTH;
        assert_eq!(doc.nodes.len(), 2 * m);
        assert_eq!(names(&doc, &doc.root), ["g0", "after"]);
        let by_name = |s: String| doc.nodes.iter().find(|n| n.layer.name == s).unwrap();
        assert_eq!(
            names(&doc, &by_name(format!("g{}", m - 2)).children),
            [format!("p{}", m - 2), format!("g{}", m - 1)]
        );
        assert!(by_name(format!("g{}", m - 1)).children.is_empty());
        for n in &doc.nodes {
            let d: usize = n.layer.name[1..].parse().unwrap_or(0);
            let kept = match n.layer.name.as_bytes()[0] {
                b'g' => d < m,
                b'p' => d + 1 < m,
                _ => n.layer.name == "after",
            };
            assert!(kept, "{}", n.layer.name);
        }

        // Past the layer limit: a group that does not fit drops its whole subtree.
        let mut xml = String::from(
            r#"<DOC><IMAGE name="i" width="4" height="4" colorspacename="RGBA"><layers>"#,
        );
        for i in 0..MAX_LAYERS {
            xml += &paint(&format!("p{i}"));
        }
        xml += &group("late");
        xml += &paint("inner");
        xml += GROUP_END;
        xml += "</layers></IMAGE></DOC>";
        let doc = parse_maindoc(xml.as_bytes());
        assert_eq!(doc.nodes.len(), MAX_LAYERS);
        assert_eq!(doc.root.len(), MAX_LAYERS);
        assert!(doc.nodes.iter().all(|n| n.layer.name.starts_with('p')));
    }

    #[test]
    fn lzf_literals_and_overlapping_runs() {
        // "ab" literal, then a 7-byte run back 2 (overlapping), then a long run with an extra
        // length byte.
        let src = [1, b'a', b'b', (5 << 5), 1, (7 << 5), 3, 0];
        let mut out = Vec::new();
        lzf_decompress(&src, &mut out, 2 + 7 + 12).unwrap();
        assert_eq!(&out[..], b"ababababaaaaaaaaaaaaa");
        assert!(lzf_decompress(&src, &mut out, 20).is_err());
        assert!(lzf_decompress(&[0x20, 0], &mut out, 3).is_err());
        assert!(lzf_decompress(&[3, 1], &mut out, 4).is_err());
    }
}
