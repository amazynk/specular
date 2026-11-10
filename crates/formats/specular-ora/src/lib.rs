//! OpenRaster (`.ora`) and Krita (`.kra`) documents.
//!
//! Both are ZIP files whose first member is (normally) an uncompressed `mimetype`
//! (`image/openraster` or `application/x-krita`) and which carry a flattened
//! `mergedimage.png`. That PNG is what gets shown; this crate finds it, the
//! small preview (`Thumbnails/thumbnail.png`, `preview.png`), and the canvas
//! size (`stack.xml` / `maindoc.xml`) without inflating any image data.
//!
//! Krita paint layers are decoded one at a time as layer pages
//! ([`Document::kra_layer_page`], the tiled LZF layer data of [`kra`], or a plain PNG where
//! a layer member holds one, with their transparency masks); group layers and, for a
//! document without a merged image, the whole layer tree are composited
//! ([`Document::kra_composite`], `kra_comp`). For an OpenRaster file without a merged image,
//! [`Document::composite`] flattens the PNG layers of `stack.xml`, honouring offsets,
//! opacity, visibility, every `composite-op` of the OpenRaster specification (its blend
//! modes and Porter-Duff operators; Krita's `krita:` ones too) and stack isolation. Blend
//! formulas are the ones shared with the other layered formats (`specular-blend`).
//!
//! Krita ops drawn: the W3C and Photoshop modes, HSL modes, grain merge and extract, the
//! documented arithmetic modes (allanon, parallel, geometric mean, negation,
//! additive-subtractive, gamma dark and light, arc tangent, reflect, glow, freeze, heat),
//! erase, behind, copy, destination-in and destination-atop; the binary ops (and, or, xor
//! and their negations, implies, converse) and modulo and divisive modulo, by the
//! definitions of Krita's manual; hue, saturation, colour and lightness ops (with their
//! increase and decrease variants) in Krita's HSY, HSV, HSL and HSI models. Its other ops
//! (the continuous and shift modulo variants, dissolve and the rest) are drawn as `normal`
//! ([`Document::kra_notes`] names them).
//!
//! Krita layers drawn besides paint and group layers: filter layers and filter masks of the
//! per-pixel filters (invert, desaturate, HSV adjustment, per-channel curves; `kra_fx`),
//! colour fill layers, clone layers, and, through a [`KraHost`], vector layers (their SVG)
//! and file layers (the image they name). Other filters and generators, transform,
//! selection and colorize masks are left out and named. CMYK, Lab, XYZ and YCbCr Krita
//! documents show their merged image only. OpenRaster text and filter layers are refused.
//!
//! Hostile input is bounded: members are inflated only up to their declared
//! size (itself capped, and checked against the DEFLATE ratio limit), XML is
//! capped, and layer count, nesting and canvas size have limits.

use std::borrow::Cow;
use std::fmt;
use std::ops::Range;

mod composite;
pub mod kra;
mod kra_comp;
mod kra_fx;
#[doc(hidden)]
pub mod kra_svg;
mod stack;
pub mod zip;

pub use composite::Rgba;
pub use kra::{KraFilterSpec, KraImage, KraKind, KraLayer, KraMask, KraNode, KraOutput, KraPixel};
pub use kra_comp::{Extra as KraExtra, KraBlend, KraHost};
pub use kra_fx::{Hsx as KraHsx, HsxOp as KraHsxOp};
pub use stack::{Item, Layer, Node, PixelLayer, Stack};
pub use zip::{Entry, Zip};

/// Largest `stack.xml` / `maindoc.xml` read.
pub const MAX_XML: u64 = 16 << 20;
/// Layers composited at most.
pub const MAX_LAYERS: usize = 1000;
/// Stack nesting depth at most.
pub const MAX_DEPTH: usize = 32;
/// Largest canvas (and layer) composited, in pixels.
pub const MAX_CANVAS_PIXELS: u64 = 1 << 26;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// OpenRaster.
    Ora,
    /// Krita.
    Kra,
}

const ORA_MIME: &[u8] = b"image/openraster";
const KRA_MIME: &[u8] = b"application/x-krita";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    NotDocument,
    Truncated(&'static str),
    Corrupt(&'static str),
    Unsupported(String),
    TooLarge(String),
    Inflate(String, String),
    Png(String, String),
    Cancelled,
    /// Neither a merged image nor anything to build one from.
    NoImage(Kind),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotDocument => f.write_str("not an OpenRaster or Krita document"),
            Error::Truncated(what) => write!(f, "file is truncated ({what})"),
            Error::Corrupt(what) => write!(f, "corrupt {what}"),
            Error::Unsupported(what) => write!(f, "{what} not supported"),
            Error::TooLarge(what) => write!(f, "{what} is too large"),
            Error::Inflate(name, why) => write!(f, "cannot decompress {name}: {why}"),
            Error::Png(name, why) => write!(f, "cannot decode {name}: {why}"),
            Error::Cancelled => f.write_str("cancelled"),
            Error::NoImage(Kind::Kra) => {
                f.write_str("Krita document has neither mergedimage.png nor preview.png")
            }
            Error::NoImage(Kind::Ora) => {
                f.write_str("OpenRaster document has neither mergedimage.png nor layers")
            }
        }
    }
}

impl std::error::Error for Error {}

/// Recognises the stored `mimetype` first member from the start of the
/// file. `head` must reach past the member's content (54 bytes for ORA, 57
/// for Krita, plus any extra field).
pub fn detect(head: &[u8]) -> Option<Kind> {
    let h = head.get(..30)?;
    if &h[..4] != b"PK\x03\x04" || h[8..10] != [0, 0] {
        return None;
    }
    let name_len = usize::from(u16::from_le_bytes([h[26], h[27]]));
    let extra = usize::from(u16::from_le_bytes([h[28], h[29]]));
    if head.get(30..30 + name_len)? != b"mimetype" {
        return None;
    }
    let size = u32::from_le_bytes([h[18], h[19], h[20], h[21]]) as usize;
    let body = head.get(30 + name_len + extra..)?;
    mime_kind(&body[..body.len().min(size)])
}

fn mime_kind(body: &[u8]) -> Option<Kind> {
    let mime = body.trim_ascii();
    if mime == ORA_MIME {
        Some(Kind::Ora)
    } else if mime == KRA_MIME {
        Some(Kind::Kra)
    } else {
        None
    }
}

/// [`detect`], falling back to the ZIP directory for writers that do not put
/// the `mimetype` member first or store it compressed (some OpenRaster tools,
/// older Krita templates): a ZIP whose `mimetype` member names the type, or,
/// with that member missing or empty, whose `maindoc.xml` declares the Krita
/// DTD. Only the directory and the start of those small members are read.
pub fn detect_file(data: &[u8]) -> Option<Kind> {
    if let Some(k) = detect(&data[..data.len().min(1024)]) {
        return Some(k);
    }
    if !data.starts_with(b"PK\x03\x04") {
        return None;
    }
    from_directory(&Zip::parse(data).ok()?, data)
}

fn from_directory(zip: &Zip, data: &[u8]) -> Option<Kind> {
    if let Some(e) = zip.find("mimetype").filter(|e| e.size > 0) {
        return mime_kind(&zip.read(data, e, 256).ok()?);
    }
    let head = zip.read_prefix(data, zip.find("maindoc.xml")?, 1024).ok()?;
    head.windows(KRA_DTD.len())
        .any(|w| w == KRA_DTD)
        .then_some(Kind::Kra)
}

/// Krita's document type, in `maindoc.xml`'s DOCTYPE and namespace.
const KRA_DTD: &[u8] = b"calligra.org/DTD/krita";

/// A parsed document: its ZIP directory and the members that matter. Holds
/// no reference to the file; every method takes the same bytes again.
#[derive(Clone, Debug)]
pub struct Document {
    pub kind: Kind,
    pub zip: Zip,
    /// Canvas size from `stack.xml` / `maindoc.xml`.
    pub canvas: Option<(u32, u32)>,
    /// OpenRaster: the layers of `stack.xml` with pixels in the file
    /// ([`Stack::pixel_layers`] whose member exists), top-most first; empty
    /// for Krita and when `stack.xml` cannot be read.
    pub layers: Vec<PixelLayer>,
    /// Krita: the layers shown as pages ([`kra::MainDoc::pages`]): the paint layers of
    /// `maindoc.xml` whose pixel data is in the file and whose colour model has a page
    /// decoder, and the groups holding them, in the Layers docker's order; empty for
    /// OpenRaster.
    pub kra_layers: Vec<KraLayer>,
    /// Krita: every node of the layer tree, a group before its children (a paint layer
    /// whose data is missing is [`KraKind::Other`]).
    pub kra_nodes: Vec<KraNode>,
    /// Krita: the top-level nodes, top-most first.
    pub kra_root: Vec<usize>,
    /// Krita: the image's colour model, when it has a decoder.
    pub kra_pixel: Option<KraPixel>,
    /// Krita: the image name, the folder of its members.
    pub kra_image: String,
    /// Krita: the name of the document's colour profile (as `maindoc.xml` gives it).
    pub kra_profile: String,
    /// Krita: the projection's background colour, one stored pixel of
    /// [`Document::kra_pixel`] (empty: transparent). The merged image and
    /// [`Document::kra_composite`] are drawn over it.
    pub kra_background: Vec<u8>,
}

impl Document {
    pub fn open(data: &[u8]) -> Result<Document, Error> {
        let zip = Zip::parse(data)?;
        let kind = match detect(&data[..data.len().min(1024)]) {
            Some(k) => k,
            None => from_directory(&zip, data).ok_or(Error::NotDocument)?,
        };
        let mut doc = Document {
            kind,
            zip,
            canvas: None,
            layers: Vec::new(),
            kra_layers: Vec::new(),
            kra_nodes: Vec::new(),
            kra_root: Vec::new(),
            kra_pixel: None,
            kra_image: String::new(),
            kra_profile: String::new(),
            kra_background: Vec::new(),
        };
        match kind {
            Kind::Ora => {
                if let Ok(s) = doc.stack(data) {
                    doc.canvas = Some((s.width, s.height));
                    let mut layers = s.pixel_layers();
                    layers.retain(|l| doc.zip.find(&l.src).is_some());
                    doc.layers = layers;
                }
            }
            Kind::Kra => {
                if let Ok(x) = doc.xml(data, "maindoc.xml") {
                    let mut m = kra::parse_maindoc(&x);
                    for n in &mut m.nodes {
                        if n.kind == KraKind::Paint && doc.zip.find(&n.layer.member).is_none() {
                            n.kind = KraKind::Other("paintlayer".into());
                        }
                    }
                    doc.canvas = m.size;
                    doc.kra_layers = m.pages();
                    doc.kra_pixel = m.pixel;
                    doc.kra_nodes = m.nodes;
                    doc.kra_root = m.root;
                    doc.kra_image = m.name;
                    doc.kra_profile = m.profile;
                    doc.kra_background = m.background;
                }
            }
        }
        Ok(doc)
    }

    fn xml<'d>(&self, data: &'d [u8], name: &str) -> Result<Cow<'d, [u8]>, Error> {
        let e = self
            .zip
            .find(name)
            .ok_or(Error::Corrupt("document: member missing"))?;
        self.zip.read(data, e, MAX_XML)
    }

    /// The parsed `stack.xml` (OpenRaster).
    pub fn stack(&self, data: &[u8]) -> Result<Stack, Error> {
        stack::parse(&self.xml(data, "stack.xml")?)
    }

    /// `mergedimage.png`, the flattened full-size image.
    pub fn merged(&self) -> Option<&Entry> {
        self.zip.find("mergedimage.png")
    }

    /// The small preview: `Thumbnails/thumbnail.png` (OpenRaster, at most
    /// 256×256) or `preview.png` (Krita).
    pub fn preview(&self) -> Option<&Entry> {
        match self.kind {
            Kind::Ora => self.zip.find("Thumbnails/thumbnail.png"),
            Kind::Kra => self.zip.find("preview.png"),
        }
    }

    pub fn read<'d>(&self, data: &'d [u8], e: &Entry) -> Result<Cow<'d, [u8]>, Error> {
        self.zip.read(data, e, zip::MAX_MEMBER)
    }

    pub fn read_prefix<'d>(
        &self,
        data: &'d [u8],
        e: &Entry,
        n: usize,
    ) -> Result<Cow<'d, [u8]>, Error> {
        self.zip.read_prefix(data, e, n)
    }

    /// The bytes of a stored member, for zero-copy use.
    pub fn stored_range(&self, data: &[u8], e: &Entry) -> Result<Option<Range<usize>>, Error> {
        self.zip.stored_range(data, e)
    }

    /// Layer `layer` (one of [`Document::layers`]) alone on the canvas: its
    /// PNG at its position, its alpha times its opacity, transparent around
    /// it; its blend mode is not applied. `cancel` is polled every 64 rows.
    pub fn layer_page(
        &self,
        data: &[u8],
        layer: &PixelLayer,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Rgba, Error> {
        let (w, h) = self.canvas.ok_or(Error::Corrupt("stack.xml: image size"))?;
        composite::layer_page(self, data, layer, w, h, cancel)
    }

    /// Flattens the layers of `stack.xml` (OpenRaster without a merged
    /// image). `cancel` is polled between layers.
    pub fn composite(&self, data: &[u8], cancel: &dyn Fn() -> bool) -> Result<Rgba, Error> {
        if self.kind != Kind::Ora {
            return Err(Error::Unsupported("Krita layer data".into()));
        }
        let stack = self.stack(data)?;
        composite::run(self, data, &stack, cancel)
    }

    /// Krita layer `layer` (one of [`Document::kra_layers`]) alone on the canvas, its alpha
    /// times its opacity and its transparency masks; its blend mode is not applied. A paint
    /// layer is its tiles at its offset over its default pixel (only its and its masks'
    /// members are read); a group is its children composited ([`Document::kra_composite`]).
    /// `cancel` is polled between tile rows, which decode in parallel.
    pub fn kra_layer_page(
        &self,
        data: &[u8],
        layer: &KraLayer,
        cancel: &(dyn Fn() -> bool + Sync),
    ) -> Result<KraImage, Error> {
        self.kra_layer_page_with(data, layer, None, cancel)
    }

    /// [`Document::kra_layer_page`] with `host` drawing the vector and file layers of a
    /// group.
    pub fn kra_layer_page_with(
        &self,
        data: &[u8],
        layer: &KraLayer,
        host: Option<&dyn KraHost>,
        cancel: &(dyn Fn() -> bool + Sync),
    ) -> Result<KraImage, Error> {
        if layer.group {
            if self
                .kra_nodes
                .get(layer.node)
                .is_none_or(|n| !n.layer.group)
            {
                return Err(Error::Corrupt("Krita layer: no such group"));
            }
            return kra_comp::composite(self, data, Some(layer.node), host, cancel);
        }
        let mut img = self.kra_paint_page(data, layer, cancel)?;
        if !layer.masks.is_empty() {
            kra_comp::mask_page(self, data, layer, &mut img, cancel)?;
        }
        Ok(img)
    }

    /// The whole Krita layer tree composited over transparency in the image's colour model
    /// (the merged image's content, less what [`Document::kra_notes`] names), without vector
    /// and file layers. `cancel` is polled between 64-row bands, which composite in
    /// parallel.
    pub fn kra_composite(
        &self,
        data: &[u8],
        cancel: &(dyn Fn() -> bool + Sync),
    ) -> Result<KraImage, Error> {
        self.kra_composite_with(data, None, cancel)
    }

    /// [`Document::kra_composite`] with `host` drawing vector and file layers.
    pub fn kra_composite_with(
        &self,
        data: &[u8],
        host: Option<&dyn KraHost>,
        cancel: &(dyn Fn() -> bool + Sync),
    ) -> Result<KraImage, Error> {
        if self.kind != Kind::Kra {
            return Err(Error::NotDocument);
        }
        kra_comp::composite(self, data, None, host, cancel)
    }

    /// Whether [`Document::kra_composite`] has something to draw: a canvas within the
    /// limits, an image colour model with a decoder and a visible paint layer.
    pub fn kra_can_composite(&self) -> bool {
        fn any(d: &Document, ids: &[usize]) -> bool {
            ids.iter().any(|&i| {
                let n = &d.kra_nodes[i];
                n.layer.visible
                    && match n.kind {
                        KraKind::Paint
                        | KraKind::Fill(_)
                        | KraKind::Vector { .. }
                        | KraKind::File { .. }
                        | KraKind::Clone { source: Some(_) } => true,
                        KraKind::Group { .. } => any(d, &n.children),
                        _ => false,
                    }
            })
        }
        self.kind == Kind::Kra
            && self.kra_pixel.is_some()
            && self
                .canvas
                .is_some_and(|(w, h)| u64::from(w) * u64::from(h) <= MAX_CANVAS_PIXELS)
            && any(self, &self.kra_root)
    }

    /// What a composite of node `node` (a group page; `None`: the whole tree) leaves out or
    /// approximates, one note each, for its visible nodes: layers not drawn and why, masks not
    /// applied, composite ops drawn as `normal`. Masks of `node` itself count too. `host` is
    /// the one the composite gets (without one, vector and file layers are not drawn).
    pub fn kra_notes(
        &self,
        data: &[u8],
        node: Option<usize>,
        host: Option<&dyn KraHost>,
    ) -> Vec<String> {
        fn kind_name(t: &str) -> String {
            match t {
                "adjustmentlayer" => "filter layer".into(),
                "generatorlayer" => "fill layer".into(),
                "shapelayer" => "vector layer".into(),
                "clonelayer" => "clone layer".into(),
                "filelayer" => "file layer".into(),
                "paintlayer" => "paint layer (no decodable data)".into(),
                "selectionmask" => "selection mask".into(),
                "filtermask" => "filter mask".into(),
                "transformmask" => "transform mask".into(),
                "colorizemask" => "colorize mask".into(),
                t => t.into(),
            }
        }
        struct Ctx<'a> {
            doc: &'a Document,
            data: &'a [u8],
            host: Option<&'a dyn KraHost>,
            grey: bool,
            linear: bool,
        }
        fn masks(c: &Ctx, l: &KraLayer, out: &mut Vec<String>) {
            for (k, name) in &l.skipped_masks {
                if k != "selectionmask" {
                    out.push(format!("{} “{name}” skipped", kind_name(k)));
                }
            }
            for m in &l.masks {
                if let Some(spec) = &m.filter
                    && let Err(why) = kra_fx::Fx::new(
                        &spec.name,
                        &kra_comp::spec_params(c.doc, c.data, spec),
                        c.grey,
                    )
                {
                    out.push(filter_note("filter mask", &m.name, &spec.name, why));
                }
            }
        }
        fn filter_note(kind: &str, name: &str, id: &str, why: &str) -> String {
            if why == "not drawn" {
                format!("{kind} “{name}” ({id}) not drawn")
            } else {
                format!("{kind} “{name}” ({id} {why}) not drawn")
            }
        }
        fn walk(c: &Ctx, ids: &[usize], out: &mut Vec<String>) {
            for &i in ids {
                let n = &c.doc.kra_nodes[i];
                let l = &n.layer;
                if !l.visible {
                    continue;
                }
                match &n.kind {
                    KraKind::Other(t) => {
                        out.push(format!("{} “{}” skipped", kind_name(t), l.name));
                        continue;
                    }
                    KraKind::Filter(spec) => {
                        let p = kra_comp::spec_params(c.doc, c.data, spec);
                        if let Err(why) = kra_fx::Fx::new(&spec.name, &p, c.grey) {
                            out.push(filter_note("filter layer", &l.name, &spec.name, why));
                            continue;
                        }
                    }
                    KraKind::Fill(spec) => {
                        let p = kra_comp::spec_params(c.doc, c.data, spec);
                        if let Err(why) = kra_fx::Generator::new(&spec.name, &p, c.linear) {
                            out.push(filter_note("fill layer", &l.name, &spec.name, why));
                            continue;
                        }
                    }
                    KraKind::Vector { svg } => match c.doc.zip.find(svg) {
                        _ if c.host.is_none() => {
                            out.push(format!("vector layer “{}” skipped", l.name));
                            continue;
                        }
                        None => {
                            let old = svg.replace("content.svg", "content.xml");
                            out.push(if c.doc.zip.find(&old).is_some() {
                                format!(
                                    "vector layer “{}” skipped (Krita 2 or 3 vector format)",
                                    l.name
                                )
                            } else {
                                format!(
                                    "vector layer “{}” skipped (its shapes are missing)",
                                    l.name
                                )
                            });
                            continue;
                        }
                        Some(e) => {
                            let text = c
                                .doc
                                .zip
                                .read(c.data, e, MAX_XML)
                                .is_ok_and(|b| b.windows(5).any(|w| w == b"<text"));
                            if text {
                                out.push(format!(
                                    "text of vector layer “{}” drawn with the fonts installed here",
                                    l.name
                                ));
                            }
                        }
                    },
                    KraKind::File { source } => match c.host.map(|h| h.locate(source)) {
                        None => {
                            out.push(format!("file layer “{}” skipped", l.name));
                            continue;
                        }
                        Some(Err(why)) => {
                            out.push(format!("file layer “{}”: {why}", l.name));
                            continue;
                        }
                        Some(Ok(())) => {}
                    },
                    KraKind::Clone { source: None } => {
                        out.push(format!(
                            "clone layer “{}” skipped: its source is not in the file",
                            l.name
                        ));
                        continue;
                    }
                    _ => {}
                }
                if l.animated {
                    out.push(format!(
                        "animated layer “{}” drawn at its first keyframe",
                        l.name
                    ));
                }
                if l.style {
                    out.push(format!("layer style of “{}” not drawn", l.name));
                }
                if KraBlend::from_id(&l.blend).is_none() {
                    out.push(format!(
                        "blend mode “{}” of “{}” drawn as normal",
                        l.blend, l.name
                    ));
                }
                masks(c, l, out);
                if let KraKind::Group { .. } = n.kind {
                    walk(c, &n.children, out);
                }
            }
        }
        let c = Ctx {
            doc: self,
            data,
            host,
            grey: self.kra_pixel.is_some_and(KraPixel::is_gray),
            linear: kra::linear_profile(&self.kra_profile),
        };
        let mut out = Vec::new();
        match node {
            Some(g) => {
                let Some(n) = self.kra_nodes.get(g) else {
                    return out;
                };
                masks(&c, &n.layer, &mut out);
                if let KraKind::Group { .. } = n.kind {
                    walk(&c, &n.children, &mut out);
                }
            }
            None => walk(&c, &self.kra_root, &mut out),
        }
        out.dedup();
        out
    }

    /// A paint layer alone on the canvas, without its masks.
    fn kra_paint_page(
        &self,
        data: &[u8],
        layer: &KraLayer,
        cancel: &(dyn Fn() -> bool + Sync),
    ) -> Result<KraImage, Error> {
        let (w, h) = self
            .canvas
            .ok_or(Error::Corrupt("maindoc.xml: image size"))?;
        let e = self
            .zip
            .find(&layer.member)
            .ok_or(Error::Corrupt("Krita layer: member missing"))?;
        let bytes = kra::plain_tiles(self.read(data, e)?, layer.pixel)?;
        let default = self
            .zip
            .find(&format!("{}.defaultpixel", layer.member))
            .and_then(|e| self.zip.read(data, e, 64).ok());
        kra::layer_page(layer, &bytes, default.as_deref(), w, h, cancel)
    }

    /// The pixel format [`Document::kra_composite`] comes out in (`None`: the image's colour
    /// model has no decoder).
    pub fn kra_output(&self) -> Option<KraOutput> {
        let linear = kra::linear_profile(&self.kra_profile);
        self.kra_pixel.map(|p| kra::output_for(p, linear))
    }

    /// The inflated size of the members a composite of node `node` (`None`: the whole tree)
    /// reads: its visible paint layers and masks.
    pub fn kra_member_bytes(&self, node: Option<usize>) -> u64 {
        fn size(d: &Document, l: &KraLayer, paint: bool) -> u64 {
            let own = if paint {
                d.zip.find(&l.member).map_or(0, |e| e.size)
            } else {
                0
            };
            own + l
                .masks
                .iter()
                .filter_map(|m| d.zip.find(&m.member))
                .map(|e| e.size)
                .sum::<u64>()
        }
        fn walk(d: &Document, ids: &[usize]) -> u64 {
            ids.iter()
                .map(|&i| {
                    let n = &d.kra_nodes[i];
                    if !n.layer.visible {
                        return 0;
                    }
                    size(d, &n.layer, n.kind == KraKind::Paint)
                        + match n.kind {
                            KraKind::Group { .. } => walk(d, &n.children),
                            _ => 0,
                        }
                })
                .sum()
        }
        match node {
            Some(g) => self
                .kra_nodes
                .get(g)
                .map_or(0, |n| size(self, &n.layer, false) + walk(self, &n.children)),
            None => walk(self, &self.kra_root),
        }
    }

    /// The ICC profile a Krita layer's pixels are coded in: its own (`<file>.icc`), else
    /// the document's (`<image>/annotations/icc`, also that of a composite: `layer: None`).
    pub fn kra_layer_icc(&self, data: &[u8], layer: Option<&KraLayer>) -> Option<Vec<u8>> {
        let own = layer
            .filter(|l| !l.member.is_empty())
            .map(|l| format!("{}.icc", l.member));
        let doc = format!("{}/annotations/icc", self.kra_image);
        own.iter().chain([&doc]).find_map(|n| {
            let e = self.zip.find(n)?;
            self.zip
                .read(data, e, 4 << 20)
                .ok()
                .map(|b| b.into_owned())
                .filter(|b| b.len() >= 132)
        })
    }
}
