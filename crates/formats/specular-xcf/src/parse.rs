//! Header, properties, layer and channel structures, hierarchies and tile tables.

use std::collections::HashSet;

use crate::read::Reader;
use crate::{Error, Info, Layer, MAX_DIMENSION, MAX_PIXELS, Result, Xcf};

/// Image base type (`PROP` independent; from the header).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaseType {
    Rgb,
    Grey,
    Indexed,
}

/// Sample type of the pixel data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Component {
    U8,
    U16,
    U32,
    Half,
    Float,
    Double,
}

impl Component {
    pub fn bytes(self) -> usize {
        match self {
            Component::U8 => 1,
            Component::U16 | Component::Half => 2,
            Component::U32 | Component::Float => 4,
            Component::Double => 8,
        }
    }

    pub fn bits(self) -> u8 {
        (self.bytes() * 8) as u8
    }

    pub fn is_float(self) -> bool {
        matches!(self, Component::Half | Component::Float | Component::Double)
    }
}

/// Image precision: sample type and transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Precision {
    pub component: Component,
    /// Linear light; otherwise sRGB-TRC coded ("non-linear" and "perceptual").
    pub linear: bool,
}

/// Tile compression (`PROP_COMPRESSION`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compression {
    None,
    Rle,
    Zlib,
}

/// Layer pixel type (`GimpImageType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LayerType {
    Rgb,
    Rgba,
    Grey,
    GreyA,
    Indexed,
    IndexedA,
}

impl LayerType {
    fn from_u32(v: u32) -> Option<LayerType> {
        Some(match v {
            0 => LayerType::Rgb,
            1 => LayerType::Rgba,
            2 => LayerType::Grey,
            3 => LayerType::GreyA,
            4 => LayerType::Indexed,
            5 => LayerType::IndexedA,
            _ => return None,
        })
    }

    pub(crate) fn channels(self) -> usize {
        match self {
            LayerType::Rgb => 3,
            LayerType::Rgba => 4,
            LayerType::Grey | LayerType::Indexed => 1,
            LayerType::GreyA | LayerType::IndexedA => 2,
        }
    }

    pub(crate) fn has_alpha(self) -> bool {
        matches!(
            self,
            LayerType::Rgba | LayerType::GreyA | LayerType::IndexedA
        )
    }
}

/// What a plane's samples are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlaneKind {
    Layer(LayerType),
    /// One linear coverage channel.
    Mask,
}

/// One tiled pixel buffer (a layer's or a mask's level 0).
#[derive(Clone, Debug)]
pub(crate) struct Plane {
    pub kind: PlaneKind,
    pub width: u32,
    pub height: u32,
    /// Bytes per pixel.
    pub bpp: usize,
    /// Canvas position of the top-left pixel.
    pub x: i32,
    pub y: i32,
    /// Tile offsets, row-major, 64×64 tiles.
    pub tiles: Vec<u64>,
}

impl Plane {
    pub(crate) fn tiles_x(&self) -> u32 {
        self.width.div_ceil(64)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Node {
    pub depth: u32,
    pub group: bool,
    pub visible: bool,
    pub mode: u32,
    pub opacity: f32,
    /// Raw `PROP_COMPOSITE_MODE` / `PROP_COMPOSITE_SPACE` / `PROP_BLEND_SPACE` (absolute value;
    /// 0 = not set).
    pub composite_mode: u32,
    pub composite_space: u32,
    pub blend_space: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub plane: Option<Plane>,
    /// The layer mask, when present and applied.
    pub mask: Option<Plane>,
    /// The pixel or mask structure could not be read (rendered without it, flagged truncated).
    pub broken: bool,
    /// GIMP 3 layer effects, bottom (applied first) to top.
    pub filters: Vec<crate::filters::Filter>,
}

#[derive(Clone, Debug)]
pub(crate) struct Doc {
    pub precision: Precision,
    pub compression: Compression,
    pub base: BaseType,
    pub width: u32,
    pub height: u32,
    pub colormap: Vec<[u8; 3]>,
    /// Layers and groups in file order (top first; a group precedes its children).
    pub nodes: Vec<Node>,
    /// Sample bytes are big-endian (v12+); otherwise little-endian.
    pub big_endian: bool,
    /// Some layer could not be read.
    pub broken: bool,
    /// Visible channels, top first: GIMP draws them over the layers in their colour.
    pub overlays: Vec<Overlay>,
}

/// A visible channel (not the selection): its colour drawn over the image where the channel
/// is set (or, with "show masked", where it is not).
#[derive(Clone, Debug)]
pub(crate) struct Overlay {
    pub plane: Plane,
    /// sRGB-coded.
    pub color: [f32; 3],
    pub opacity: f32,
    pub inverted: bool,
}

const PROP_END: u32 = 0;
const PROP_COLORMAP: u32 = 1;
const PROP_SELECTION: u32 = 4;
const PROP_OPACITY: u32 = 6;
const PROP_MODE: u32 = 7;
const PROP_VISIBLE: u32 = 8;
const PROP_APPLY_MASK: u32 = 11;
const PROP_SHOW_MASKED: u32 = 14;
const PROP_OFFSETS: u32 = 15;
const PROP_COLOR: u32 = 16;
const PROP_COMPRESSION: u32 = 17;
const PROP_PARASITES: u32 = 21;
const PROP_GROUP_ITEM: u32 = 29;
const PROP_ITEM_PATH: u32 = 30;
const PROP_FLOAT_OPACITY: u32 = 33;
const PROP_COMPOSITE_MODE: u32 = 35;
const PROP_COMPOSITE_SPACE: u32 = 36;
const PROP_BLEND_SPACE: u32 = 37;
const PROP_FLOAT_COLOR: u32 = 38;

/// Deepest group nesting kept (GIMP has no limit; real files stay far below).
const MAX_NESTING: u32 = 64;

/// Mode number for "Normal" (GIMP 2.10+), the default.
pub(crate) const MODE_NORMAL: u32 = 28;
const MODE_OVERLAY_LEGACY: u32 = 5;
const MODE_SOFTLIGHT_LEGACY: u32 = 19;

fn version(head: &[u8]) -> Result<u32> {
    if !crate::probe(head) {
        return Err(Error::NotXcf);
    }
    let v = &head[9..13];
    if v == b"file" {
        return Ok(0);
    }
    let s = std::str::from_utf8(&v[1..]).map_err(|_| Error::NotXcf)?;
    s.parse().map_err(|_| Error::NotXcf)
}

fn precision(version: u32, p: u32) -> Result<Precision> {
    use Component::*;
    let (component, linear) = if version == 4 {
        match p {
            0 => (U8, false),
            1 => (U16, false),
            2 => (U32, true),
            3 => (Half, true),
            4 => (Float, true),
            _ => return Err(Error::Unsupported(format!("precision {p}"))),
        }
    } else if version < 7 {
        let c = match p / 100 {
            1 => U8,
            2 => U16,
            3 => U32,
            4 => Half,
            5 => Float,
            _ => return Err(Error::Unsupported(format!("precision {p}"))),
        };
        match p % 100 {
            0 => (c, true),
            50 => (c, false),
            _ => return Err(Error::Unsupported(format!("precision {p}"))),
        }
    } else {
        let c = match p / 100 {
            1 => U8,
            2 => U16,
            3 => U32,
            5 => Half,
            6 => Float,
            7 => Double,
            _ => return Err(Error::Unsupported(format!("precision {p}"))),
        };
        match p % 100 {
            0 => (c, true),
            50 | 75 => (c, false),
            _ => return Err(Error::Unsupported(format!("precision {p}"))),
        }
    };
    Ok(Precision { component, linear })
}

fn check_size(w: u32, h: u32, what: &str) -> Result<()> {
    if w == 0 || h == 0 {
        return Err(Error::Malformed(format!("{what} size {w}×{h}")));
    }
    if w > MAX_DIMENSION || h > MAX_DIMENSION {
        return Err(Error::TooLarge);
    }
    Ok(())
}

pub(crate) fn parse(data: &[u8]) -> Result<Xcf> {
    let version = version(data.get(..14).ok_or(Error::NotXcf)?)?;
    let wide = version >= 11;
    let mut r = Reader::new(data, wide);
    r.pos = 14;
    let width = r.u32("header")?;
    let height = r.u32("header")?;
    check_size(width, height, "image")?;
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(Error::TooLarge);
    }
    let base = match r.u32("header")? {
        0 => BaseType::Rgb,
        1 => BaseType::Grey,
        2 => BaseType::Indexed,
        b => return Err(Error::Malformed(format!("base type {b}"))),
    };
    let precision = if version >= 4 {
        precision(version, r.u32("precision")?)?
    } else {
        Precision {
            component: Component::U8,
            linear: false,
        }
    };

    // Image properties.
    let mut colormap = Vec::new();
    let mut compression = Compression::None;
    let mut icc = None;
    loop {
        let ty = r.u32("image properties")?;
        let len = r.u32("image properties")? as usize;
        match ty {
            PROP_END => break,
            PROP_COLORMAP => {
                let n = r.u32("colormap")? as usize;
                if n > 256 {
                    return Err(Error::Malformed(format!("{n} colormap entries")));
                }
                if version == 0 {
                    // v0 wrote the map wrongly; GIMP substitutes a grey ramp.
                    r.skip(n, "colormap")?;
                    colormap = (0..n).map(|i| [i as u8; 3]).collect();
                } else {
                    let b = r.bytes(n * 3, "colormap")?;
                    colormap = b.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
                }
            }
            PROP_COMPRESSION => {
                let b = r.bytes(len, "compression")?;
                compression = match b.first() {
                    Some(0) => Compression::None,
                    Some(1) => Compression::Rle,
                    Some(2) => Compression::Zlib,
                    Some(c) => {
                        return Err(Error::Unsupported(format!("tile compression {c}")));
                    }
                    None => return Err(Error::Malformed("empty compression property".into())),
                };
            }
            PROP_PARASITES => {
                let start = r.pos;
                let body = r.bytes(len, "parasites")?;
                icc = parasites(body, start, "icc-profile");
            }
            _ => r.skip(len, "image property")?,
        }
    }

    let mut layer_ptrs = Vec::new();
    loop {
        let p = r.ptr("layer list")?;
        if p == 0 {
            break;
        }
        layer_ptrs.push(p);
    }

    let mut nodes = Vec::with_capacity(layer_ptrs.len());
    let mut live_filters = 0;
    let mut broken = false;
    let mut cut = false;
    // GIMP writes every layer and pixel buffer once. A layer listed again, or a tile table
    // shared by several drawables, is refused: a few bytes each, they made parsing and
    // rendering the whole buffer again per reference (quadratic in the file size).
    let mut seen_layers = HashSet::new();
    let mut levels = HashSet::new();
    for &p in &layer_ptrs {
        // Like GIMP, a layer that cannot be read ends the stack (keeping what loaded).
        let read = if seen_layers.insert(p) {
            layer(data, p, wide, version, precision.component, &mut levels)
        } else {
            Err(Error::Malformed("layer listed twice".into()))
        };
        match read {
            Ok((node, filters)) => {
                live_filters += filters;
                broken |= node.broken;
                nodes.push(node);
            }
            Err(e) if nodes.is_empty() => return Err(e),
            Err(_) => {
                broken = true;
                cut = true;
                break;
            }
        }
    }
    layer_ptrs.truncate(nodes.len());

    // Channels follow the layer list (only when every layer was read: the list position is
    // known then).
    let mut overlays = Vec::new();
    if !cut {
        loop {
            let Ok(p) = r.ptr("channel list") else {
                broken = true;
                break;
            };
            if p == 0 {
                break;
            }
            match channel(data, p, wide, precision.component, 0, 0, &mut levels) {
                Ok((props, plane)) => {
                    if props.visible
                        && !props.selection
                        && props.opacity > 0.0
                        && plane.width == width
                        && plane.height == height
                    {
                        overlays.push(Overlay {
                            plane,
                            color: props.color,
                            opacity: props.opacity,
                            inverted: props.show_masked,
                        });
                    }
                }
                Err(_) => {
                    broken = true;
                    break;
                }
            }
        }
    }
    if base == BaseType::Indexed && colormap.is_empty() {
        colormap = vec![[0; 3]];
    }

    let mut doc = Doc {
        precision,
        compression,
        base,
        width,
        height,
        colormap,
        nodes,
        big_endian: version >= 12,
        broken,
        overlays,
    };
    fix_depths(&mut doc.nodes);
    let layers = doc
        .nodes
        .iter()
        .zip(&layer_names(data, &layer_ptrs, wide))
        .map(|(n, name)| Layer {
            name: name.clone(),
            depth: n.depth,
            group: n.group,
            visible: n.visible,
            mode: n.mode,
            opacity: n.opacity,
            x: n.x,
            y: n.y,
            width: n.width,
            height: n.height,
            has_mask: n.mask.is_some(),
        })
        .collect();
    let info = Info {
        version,
        width,
        height,
        base,
        precision,
        compression,
        layer_count: doc.nodes.iter().filter(|n| !n.group).count(),
        group_count: doc.nodes.iter().filter(|n| n.group).count(),
        live_filters,
        undrawn_filters: {
            let mut v: Vec<String> = Vec::new();
            for n in &doc.nodes {
                for f in n.filters.iter().filter(|f| f.visible && !f.drawn()) {
                    if !v.contains(&f.op) {
                        v.push(f.op.clone());
                    }
                }
            }
            v
        },
        has_alpha: crate::render::may_have_alpha(&doc),
    };
    Ok(Xcf {
        info,
        layers,
        icc,
        doc,
    })
}

/// A depth may only grow by one from one node to the next, and only below a group; anything
/// else (a damaged path) is clamped so the tree stays well-formed.
fn fix_depths(nodes: &mut [Node]) {
    let mut max = 0u32;
    for n in nodes.iter_mut() {
        n.depth = n.depth.min(max);
        // Nesting is capped so deep (damaged) trees cannot exhaust the stack when rendered.
        max = if n.group && n.depth < MAX_NESTING {
            n.depth + 1
        } else {
            n.depth
        };
    }
}

fn layer_names(data: &[u8], ptrs: &[u64], wide: bool) -> Vec<String> {
    ptrs.iter()
        .map(|&p| {
            let mut r = match Reader::at(data, p, wide, "layer") {
                Ok(r) => r,
                Err(_) => return String::new(),
            };
            let _ = r.bytes(12, "layer");
            r.string("layer name").unwrap_or_default()
        })
        .collect()
}

/// Finds the parasite `want` in a `PROP_PARASITES` body starting at file offset `base`; returns
/// its data's file range.
fn parasites(body: &[u8], base: usize, want: &str) -> Option<std::ops::Range<usize>> {
    let mut r = Reader::new(body, false);
    while r.pos < body.len() {
        let name = r.string("parasite").ok()?;
        let _flags = r.u32("parasite").ok()?;
        let size = r.u32("parasite").ok()? as usize;
        let start = r.pos;
        r.skip(size, "parasite").ok()?;
        if name == want && size > 0 {
            return Some(base + start..base + start + size);
        }
    }
    None
}

fn layer(
    data: &[u8],
    ptr: u64,
    wide: bool,
    version: u32,
    component: Component,
    levels: &mut HashSet<u64>,
) -> Result<(Node, usize)> {
    let mut r = Reader::at(data, ptr, wide, "layer")?;
    let width = r.u32("layer")?;
    let height = r.u32("layer")?;
    let ty_raw = r.u32("layer")?;
    let ty = LayerType::from_u32(ty_raw)
        .ok_or_else(|| Error::Malformed(format!("layer type {ty_raw}")))?;
    let _name = r.string("layer name")?;

    let mut node = Node {
        depth: 0,
        group: false,
        visible: true,
        mode: MODE_NORMAL,
        opacity: 1.0,
        composite_mode: 0,
        composite_space: 0,
        blend_space: 0,
        x: 0,
        y: 0,
        width,
        height,
        plane: None,
        mask: None,
        broken: false,
        filters: Vec::new(),
    };
    let mut apply_mask = true;
    let mut float_opacity = None;
    // GIMP creates the layer as (2.10+) Normal before reading PROP_MODE; files before 2.10
    // always write PROP_MODE with legacy numbers.
    let mut mode = None;
    loop {
        let ty = r.u32("layer properties")?;
        let len = r.u32("layer properties")? as usize;
        let body = r.bytes(len, "layer property")?;
        let mut b = Reader::new(body, false);
        match ty {
            PROP_END => break,
            PROP_OPACITY => {
                node.opacity = (b.u32("opacity")?.min(255) as f32) / 255.0;
            }
            PROP_FLOAT_OPACITY => float_opacity = Some(b.f32("opacity")?),
            PROP_MODE => {
                let mut m = b.u32("mode")?;
                if m == MODE_OVERLAY_LEGACY {
                    m = MODE_SOFTLIGHT_LEGACY;
                }
                mode = Some(m);
            }
            PROP_VISIBLE => node.visible = b.u32("visible")? != 0,
            PROP_APPLY_MASK => apply_mask = b.u32("apply mask")? != 0,
            PROP_OFFSETS => {
                node.x = b.i32("offsets")?;
                node.y = b.i32("offsets")?;
            }
            PROP_GROUP_ITEM => node.group = true,
            PROP_ITEM_PATH => {
                node.depth = (len / 4).saturating_sub(1) as u32;
            }
            PROP_COMPOSITE_MODE => node.composite_mode = b.i32("composite mode")?.unsigned_abs(),
            PROP_COMPOSITE_SPACE => {
                node.composite_space = b.i32("composite space")?.unsigned_abs();
            }
            PROP_BLEND_SPACE => node.blend_space = b.i32("blend space")?.unsigned_abs(),
            _ => {}
        }
    }
    if let Some(o) = float_opacity {
        node.opacity = if o.is_nan() { 1.0 } else { o.clamp(0.0, 1.0) };
    }
    node.mode = match mode {
        Some(m) if m <= 63 => m,
        _ => MODE_NORMAL,
    };

    let hierarchy = r.ptr("layer hierarchy pointer")?;
    let mask_ptr = r.ptr("layer mask pointer")?;
    let mut filters = 0;
    if version >= 20 {
        // Effect pointers until 0, the top effect first.
        let mut ptrs = Vec::new();
        while let Ok(p) = r.ptr("layer effects") {
            if p == 0 || ptrs.len() >= 256 {
                break;
            }
            ptrs.push(p);
        }
        filters = ptrs.len();
        for &p in ptrs.iter().rev() {
            match crate::filters::read(data, p, wide) {
                Ok(f) => node.filters.push(f),
                Err(_) => node.filters.push(crate::filters::Filter::unreadable()),
            }
        }
    }

    if node.group {
        // Group hierarchies are not used (GIMP rebuilds the projection).
        node.width = node.width.max(1);
        node.height = node.height.max(1);
    } else {
        check_size(width, height, "layer")?;
        let kind = PlaneKind::Layer(ty);
        let bpp = if matches!(ty, LayerType::Indexed | LayerType::IndexedA) {
            ty.channels()
        } else {
            ty.channels() * component.bytes()
        };
        match hierarchy_plane(
            data, hierarchy, wide, kind, width, height, bpp, node.x, node.y, levels,
        ) {
            Ok(p) => node.plane = Some(p),
            Err(_) => node.broken = true,
        }
    }
    if mask_ptr != 0 && apply_mask {
        match mask(data, mask_ptr, wide, component, node.x, node.y, levels) {
            Ok(m) => node.mask = Some(m),
            Err(_) => node.broken = true,
        }
    }
    Ok((node, filters))
}

fn mask(
    data: &[u8],
    ptr: u64,
    wide: bool,
    component: Component,
    x: i32,
    y: i32,
    levels: &mut HashSet<u64>,
) -> Result<Plane> {
    channel(data, ptr, wide, component, x, y, levels).map(|(_, p)| p)
}

/// The properties of a channel (or layer mask) that matter here.
struct ChannelProps {
    visible: bool,
    selection: bool,
    show_masked: bool,
    opacity: f32,
    color: [f32; 3],
}

fn channel(
    data: &[u8],
    ptr: u64,
    wide: bool,
    component: Component,
    x: i32,
    y: i32,
    levels: &mut HashSet<u64>,
) -> Result<(ChannelProps, Plane)> {
    let mut m = Reader::at(data, ptr, wide, "channel")?;
    let mw = m.u32("channel")?;
    let mh = m.u32("channel")?;
    let _name = m.string("channel name")?;
    let mut props = ChannelProps {
        visible: false,
        selection: false,
        show_masked: false,
        opacity: 1.0,
        color: [0.0; 3],
    };
    let mut float_opacity = None;
    let mut float_color = None;
    loop {
        let ty = m.u32("channel properties")?;
        let len = m.u32("channel properties")? as usize;
        let body = m.bytes(len, "channel property")?;
        let mut b = Reader::new(body, false);
        match ty {
            PROP_END => break,
            PROP_SELECTION => props.selection = true,
            PROP_VISIBLE => props.visible = b.u32("visible")? != 0,
            PROP_SHOW_MASKED => props.show_masked = b.u32("show masked")? != 0,
            PROP_OPACITY => props.opacity = (b.u32("opacity")?.min(255) as f32) / 255.0,
            PROP_FLOAT_OPACITY => float_opacity = Some(b.f32("opacity")?),
            PROP_COLOR => {
                if let [r, g, bl, ..] = *body {
                    props.color = [r, g, bl].map(|v| f32::from(v) / 255.0);
                }
            }
            PROP_FLOAT_COLOR => {
                float_color = Some([b.f32("colour")?, b.f32("colour")?, b.f32("colour")?]);
            }
            _ => {}
        }
    }
    if let Some(o) = float_opacity {
        props.opacity = if o.is_nan() { 1.0 } else { o.clamp(0.0, 1.0) };
    }
    if let Some(c) = float_color.filter(|c| c.iter().all(|v| v.is_finite())) {
        props.color = c;
    }
    let h = m.ptr("channel hierarchy pointer")?;
    check_size(mw, mh, "channel")?;
    let plane = hierarchy_plane(
        data,
        h,
        wide,
        PlaneKind::Mask,
        mw,
        mh,
        component.bytes(),
        x,
        y,
        levels,
    )?;
    Ok((props, plane))
}

#[allow(clippy::too_many_arguments)]
fn hierarchy_plane(
    data: &[u8],
    ptr: u64,
    wide: bool,
    kind: PlaneKind,
    width: u32,
    height: u32,
    bpp: usize,
    x: i32,
    y: i32,
    levels: &mut HashSet<u64>,
) -> Result<Plane> {
    let mut r = Reader::at(data, ptr, wide, "hierarchy")?;
    let hw = r.u32("hierarchy")?;
    let hh = r.u32("hierarchy")?;
    let hbpp = r.u32("hierarchy")? as usize;
    if hw != width || hh != height {
        return Err(Error::Malformed(format!(
            "hierarchy {hw}×{hh} for a {width}×{height} buffer"
        )));
    }
    if hbpp != bpp {
        return Err(Error::Malformed(format!(
            "{hbpp} bytes per pixel where {bpp} were expected"
        )));
    }
    let level = r.ptr("level pointer")?;
    let mut l = Reader::at(data, level, wide, "level")?;
    let lw = l.u32("level")?;
    let lh = l.u32("level")?;
    if lw != width || lh != height {
        return Err(Error::Malformed(format!(
            "level {lw}×{lh} for a {width}×{height} buffer"
        )));
    }
    let n = (width.div_ceil(64) as usize) * (height.div_ceil(64) as usize);
    let ptr_bytes = if wide { 8 } else { 4 };
    if n.saturating_mul(ptr_bytes) > data.len() {
        return Err(Error::Truncated("tile table".into()));
    }
    if !levels.insert(level) {
        return Err(Error::Malformed("tile table shared by two buffers".into()));
    }
    let mut tiles = Vec::with_capacity(n);
    for _ in 0..n {
        // A table cut short leaves the remaining tiles absent (transparent).
        match l.ptr("tile table") {
            Ok(p) => tiles.push(p),
            Err(_) => break,
        }
        if tiles.last() == Some(&0) {
            tiles.pop();
            break;
        }
    }
    Ok(Plane {
        kind,
        width,
        height,
        bpp,
        x,
        y,
        tiles,
    })
}
