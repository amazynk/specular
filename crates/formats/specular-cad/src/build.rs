//! Entities → nodes: shapes in the coordinates of their space (model, paper or a block
//! definition), with layer/colour/lineweight/linetype still symbolic (BYLAYER, BYBLOCK) so
//! a block can be instantiated under different references.

use std::collections::HashMap;

use kurbo::{Affine, BezPath, Point, Rect, Shape as _, Vec2};
use specular_vector::{
    Color, FontSpec, HAlign, LW_DEFAULT, Layer, Linetype, Lineweight, Paragraph, Shape, Span,
    TextBlock, VAlign,
};

use crate::doc::{Ent, Header};
use crate::geom::{self, M3, Spline, V3};
use crate::hatch::{self, PatternLine};
use crate::text::{self, Decoder};
use crate::tokens::Group;
use crate::{Limits, aci};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ColorSpec {
    ByLayer,
    ByBlock,
    Is(Color),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum LwSpec {
    ByLayer,
    ByBlock,
    Is(Lineweight),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum LtSpec {
    ByLayer,
    ByBlock,
    Continuous,
    Is(u32),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Props {
    pub layer: u32,
    pub color: ColorSpec,
    pub lw: LwSpec,
    pub lt: LtSpec,
    pub lt_scale: f64,
    pub handle: u64,
}

/// A paper-space viewport as stored.
#[derive(Clone, Debug)]
pub(crate) struct VpDef {
    pub paper: Rect,
    pub transform: Affine,
    pub frozen: Vec<u64>,
    pub clip: Option<u64>,
}

pub(crate) enum Node {
    Draw {
        props: Props,
        /// The plane the shapes lie in, for block references that tilt it; the shapes
        /// are already its plan view.
        plane: Plane,
        shapes: Vec<Shape>,
    },
    Insert {
        props: Props,
        block: usize,
        xfs: Vec<M3>,
        attribs: Vec<Node>,
    },
    /// XLINE (both ways) or RAY: clipped to the sheet's extents once they are known.
    Infinite {
        props: Props,
        origin: Point,
        dir: Vec2,
        ray: bool,
    },
    Viewport(Box<VpDef>),
}

/// An entity's plane: extrusion and elevation along it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Plane {
    pub n: V3,
    pub elev: f64,
}

impl Plane {
    fn of(ty: &str, e: &Ent<'_>) -> Plane {
        let wcs = |z: f64| Plane { n: V3::Z, elev: z };
        match ty {
            "LINE" | "POINT" | "3DFACE" | "IMAGE" | "WIPEOUT" | "MESH" | "LEADER" | "MLEADER"
            | "MULTILEADER" => wcs(e.f(30, 0.0)),
            "ELLIPSE" | "SPLINE" | "MTEXT" => {
                let n = e.extrusion().norm().unwrap_or(V3::Z);
                let p = e.p3(10);
                Plane {
                    n,
                    elev: n.x * p.x + n.y * p.y + n.z * p.z,
                }
            }
            "LWPOLYLINE" => Plane {
                n: e.extrusion(),
                elev: e.f(38, 0.0),
            },
            "POLYLINE" if e.i(70, 0) & (8 | 16 | 64) != 0 => wcs(0.0),
            _ => Plane {
                n: e.extrusion(),
                elev: e.f(30, 0.0),
            },
        }
    }
}

/// Where the entities being converted live.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Space {
    /// Model or paper space: ATTDEFs show their tag.
    Top,
    /// A block definition: only constant ATTDEFs show (their value).
    Block,
}

pub(crate) struct Builder<'a> {
    pub limits: &'a Limits,
    pub header: &'a Header,
    pub dec: Decoder,
    pub layers: Vec<Layer>,
    layer_map: HashMap<String, u32>,
    layer_cache: HashMap<&'a [u8], u32>,
    pub layer_handles: HashMap<u64, u32>,
    pub linetypes: Vec<Linetype>,
    lt_map: HashMap<String, u32>,
    pub fonts: Vec<FontSpec>,
    font_map: HashMap<String, u32>,
    /// Block name (lower case) → index, and each block's base point.
    pub block_map: HashMap<String, usize>,
    pub block_bases: Vec<V3>,
    pub counts: HashMap<String, u64>,
    pub warnings: Vec<String>,
    /// Hatch segments generated so far ([`hatch::MAX_TOTAL`]).
    hatched: usize,
    /// INSERT array cells so far ([`MAX_ARRAY_CELLS`]).
    array_cells: u64,
}

/// INSERT array cells a file may declare in all (each is a transform kept per insert).
const MAX_ARRAY_CELLS: u64 = 1_000_000;

fn lower(s: &str) -> String {
    s.to_lowercase()
}

impl<'a> Builder<'a> {
    pub fn new(limits: &'a Limits, header: &'a Header, dec: Decoder) -> Builder<'a> {
        Builder {
            limits,
            header,
            dec,
            layers: Vec::new(),
            layer_map: HashMap::new(),
            layer_cache: HashMap::new(),
            layer_handles: HashMap::new(),
            linetypes: Vec::new(),
            lt_map: HashMap::new(),
            fonts: Vec::new(),
            font_map: HashMap::new(),
            block_map: HashMap::new(),
            block_bases: Vec::new(),
            counts: HashMap::new(),
            warnings: Vec::new(),
            hatched: 0,
            array_cells: 0,
        }
    }

    pub fn warn(&mut self, w: impl Into<String>) {
        let w = w.into();
        if !self.warnings.contains(&w) && self.warnings.len() < 200 {
            self.warnings.push(w);
        }
    }

    // ------------------------------------------------------------------------- tables

    pub fn linetype_table(&mut self, ents: &[Ent<'a>]) {
        for e in ents {
            if !e.is("LTYPE") {
                continue;
            }
            let name = self.dec.decode(e.s(2));
            if name.is_empty() || self.lt_map.contains_key(&lower(&name)) {
                continue;
            }
            let pattern: Vec<f64> =
                e.g.iter()
                    .filter(|g| g.code == 49)
                    .map(|g| g.f64())
                    .take(64)
                    .collect();
            let idx = self.linetypes.len() as u32;
            self.lt_map.insert(lower(&name), idx);
            self.linetypes.push(Linetype {
                name,
                description: self.dec.decode(e.s(3)),
                pattern,
            });
        }
    }

    fn linetype_named(&mut self, name: &[u8]) -> LtSpec {
        if name.is_empty() || name.eq_ignore_ascii_case(b"BYLAYER") {
            return LtSpec::ByLayer;
        }
        if name.eq_ignore_ascii_case(b"BYBLOCK") {
            return LtSpec::ByBlock;
        }
        if name.eq_ignore_ascii_case(b"CONTINUOUS") {
            return LtSpec::Continuous;
        }
        let n = lower(&self.dec.decode(name));
        match self.lt_map.get(&n) {
            Some(&i) if !self.linetypes[i as usize].continuous() => LtSpec::Is(i),
            Some(_) => LtSpec::Continuous,
            None => {
                self.warn(format!("linetype {n} is not defined"));
                LtSpec::Continuous
            }
        }
    }

    pub fn layer_table(&mut self, ents: &[Ent<'a>]) {
        for e in ents {
            if !e.is("LAYER") {
                continue;
            }
            let name = self.dec.decode(e.s(2));
            if self.layer_map.contains_key(&lower(&name)) {
                continue;
            }
            let c = e.i(62, 7);
            let color = match e.get(420) {
                Some(g) => aci::true_color(g.i64()),
                None => aci::color(c.abs()),
            };
            let linetype = match self.linetype_named(e.s(6)) {
                LtSpec::Is(i) => Some(i),
                _ => None,
            };
            let lw = match e.i(370, -3) {
                v if v < 0 => LW_DEFAULT,
                v => v.min(211) as Lineweight,
            };
            let idx = self.layers.len() as u32;
            self.layer_map.insert(lower(&name), idx);
            if e.has(5) {
                self.layer_handles.insert(e.handle(), idx);
            }
            self.layers.push(Layer {
                name,
                generated: None,
                color,
                linetype,
                lineweight: lw,
                on: c >= 0,
                frozen: e.i(70, 0) & 1 != 0,
                plot: e.i(290, 1) != 0,
                items: 0,
            });
        }
        self.layer_index("0");
    }

    /// The layer named `name`, created (visible, foreground, continuous) when the table
    /// does not define it.
    pub fn layer_index(&mut self, name: &str) -> u32 {
        let k = lower(name);
        if let Some(&i) = self.layer_map.get(&k) {
            return i;
        }
        let idx = self.layers.len() as u32;
        self.layer_map.insert(k, idx);
        self.layers.push(Layer {
            name: name.to_string(),
            generated: None,
            color: Color::Foreground,
            linetype: None,
            lineweight: LW_DEFAULT,
            on: true,
            frozen: false,
            plot: true,
            items: 0,
        });
        idx
    }

    fn layer_of(&mut self, raw: &'a [u8]) -> u32 {
        if let Some(&i) = self.layer_cache.get(raw) {
            return i;
        }
        let name = self.dec.decode(raw);
        let name = if name.is_empty() { "0".into() } else { name };
        let i = self.layer_index(&name);
        self.layer_cache.insert(raw, i);
        i
    }

    pub fn layer0(&mut self) -> u32 {
        self.layer_index("0")
    }

    pub fn style_table(&mut self, ents: &[Ent<'a>]) {
        for e in ents {
            if !e.is("STYLE") {
                continue;
            }
            let name = self.dec.decode(e.s(2));
            if name.is_empty() || self.font_map.contains_key(&lower(&name)) {
                continue;
            }
            // Extended data of the ACAD application: TrueType family and bold/italic flags.
            let mut family = None;
            let mut flags = 0i64;
            let mut app = false;
            for g in e.all.iter().filter(|g| g.code >= 1000) {
                match g.code {
                    1001 => app = g.is("ACAD"),
                    1000 if app && family.is_none() => {
                        let f = self.dec.decode(g.tbytes());
                        if !f.is_empty() {
                            family = Some(f);
                        }
                    }
                    1071 if app => flags = g.i64(),
                    _ => {}
                }
            }
            let idx = self.fonts.len() as u32;
            self.font_map.insert(lower(&name), idx);
            let wf = e.f(41, 1.0);
            self.fonts.push(FontSpec {
                style: name,
                file: self.dec.decode(e.s(3)),
                family,
                bold: flags & 0x0200_0000 != 0,
                italic: flags & 0x0100_0000 != 0,
                width_factor: if wf > 0.0 { wf } else { 1.0 },
                oblique: e.f(50, 0.0),
                bigfont: self.dec.decode(e.s(4)),
                shx: None,
            });
        }
        if self.fonts.is_empty() {
            self.font_map.insert("standard".into(), 0);
            self.fonts.push(FontSpec {
                style: "Standard".into(),
                file: "txt".into(),
                family: None,
                bold: false,
                italic: false,
                width_factor: 1.0,
                oblique: 0.0,
                bigfont: String::new(),
                shx: None,
            });
        }
    }

    fn font_of(&mut self, raw: &[u8]) -> u32 {
        if raw.is_empty() {
            return self.font_map.get("standard").copied().unwrap_or(0);
        }
        let n = lower(&self.dec.decode(raw));
        self.font_map
            .get(&n)
            .or_else(|| self.font_map.get("standard"))
            .copied()
            .unwrap_or(0)
    }

    // ------------------------------------------------------------------------- entities

    fn props(&mut self, e: &Ent<'a>) -> Props {
        let layer = self.layer_of(e.s(8));
        let color = if let Some(g) = e.get(420) {
            ColorSpec::Is(aci::true_color(g.i64()))
        } else {
            match e.i(62, 256) {
                0 => ColorSpec::ByBlock,
                256 => ColorSpec::ByLayer,
                c => ColorSpec::Is(aci::color(c.abs())),
            }
        };
        let lw = match e.i(370, -1) {
            -1 => LwSpec::ByLayer,
            -2 => LwSpec::ByBlock,
            v if v < 0 => LwSpec::Is(LW_DEFAULT),
            v => LwSpec::Is(v.min(211) as Lineweight),
        };
        let lt = self.linetype_named(e.s(6));
        let s = e.f(48, 1.0);
        Props {
            layer,
            color,
            lw,
            lt,
            lt_scale: if s > 0.0 { s } else { 1.0 },
            handle: e.handle(),
        }
    }

    fn count(&mut self, ty: &[u8]) {
        let name = String::from_utf8_lossy(ty).to_ascii_uppercase();
        *self.counts.entry(name).or_insert(0) += 1;
    }

    /// Converts and counts a list of records.
    pub fn convert_all(&mut self, ents: &[Ent<'a>], space: Space) -> Vec<Node> {
        let mut out = Vec::new();
        for e in ents {
            self.count(e.ty);
            for s in &e.subs {
                if !(e.is("POLYLINE") && s.is("VERTEX") || e.is("INSERT") && s.is("ATTRIB")) {
                    self.count(s.ty);
                }
            }
            if e.i(60, 0) == 1 {
                continue;
            }
            if let Some(n) = self.convert(e, space) {
                out.push(n);
            }
        }
        out
    }

    fn draw(&mut self, e: &Ent<'a>, shapes: Vec<Shape>) -> Option<Node> {
        if shapes.is_empty() {
            return None;
        }
        let ty = String::from_utf8_lossy(e.ty).to_ascii_uppercase();
        Some(Node::Draw {
            props: self.props(e),
            plane: Plane::of(&ty, e),
            shapes,
        })
    }

    fn stroke(path: BezPath) -> Shape {
        Shape::Stroke {
            path,
            weight: 0,
            dash: None,
        }
    }

    fn convert(&mut self, e: &Ent<'a>, space: Space) -> Option<Node> {
        let ty = String::from_utf8_lossy(e.ty).to_ascii_uppercase();
        match ty.as_str() {
            "LINE" => {
                let mut p = BezPath::new();
                p.move_to(e.p2(10));
                p.line_to(e.p2(11));
                self.draw(e, vec![Self::stroke(p)])
            }
            "POINT" => self.draw(e, vec![Shape::Point(e.p2(10))]),
            "CIRCLE" => {
                let r = e.f(40, 0.0);
                if r <= 0.0 {
                    return None;
                }
                let o = geom::ocs(e.extrusion(), e.f(30, 0.0));
                let mut p = geom::ellipse(e.p2(10), Vec2::new(r, 0.0), Vec2::new(0.0, r));
                p.apply_affine(o);
                self.draw(e, vec![Self::stroke(p)])
            }
            "ARC" => {
                let r = e.f(40, 0.0);
                if r <= 0.0 {
                    return None;
                }
                let a0 = e.f(50, 0.0).to_radians();
                let sweep = ccw_sweep(a0, e.f(51, 360.0).to_radians());
                let mut p = BezPath::new();
                geom::ellipse_arc(
                    &mut p,
                    e.p2(10),
                    Vec2::new(r, 0.0),
                    Vec2::new(0.0, r),
                    a0,
                    sweep,
                    true,
                );
                p.apply_affine(geom::ocs(e.extrusion(), e.f(30, 0.0)));
                self.draw(e, vec![Self::stroke(p)])
            }
            "ELLIPSE" => {
                let c = e.p2(10);
                let m = e.p3(11);
                let n = e.extrusion().norm().unwrap_or(V3::Z);
                let ratio = e.f(40, 1.0);
                let mi = n.cross(m);
                let minor = Vec2::new(mi.x, mi.y) * ratio;
                let s = e.f(41, 0.0);
                let en = e.f(42, std::f64::consts::TAU);
                let sweep = ccw_sweep(s, en);
                let mut p = BezPath::new();
                geom::ellipse_arc(&mut p, c, m.xy(), minor, s, sweep, true);
                if (sweep - std::f64::consts::TAU).abs() < 1e-9 {
                    p.close_path();
                }
                self.draw(e, vec![Self::stroke(p)])
            }
            "LWPOLYLINE" => {
                let shapes = self.lwpolyline(e);
                self.draw(e, shapes)
            }
            "POLYLINE" => {
                let shapes = self.polyline(e);
                self.draw(e, shapes)
            }
            "SPLINE" | "HELIX" => {
                let p = self.spline(e)?;
                self.draw(e, vec![Self::stroke(p)])
            }
            "TEXT" => {
                let t = self.text(e, TextKind::Text, None)?;
                self.draw(e, vec![Shape::Text(Box::new(t))])
            }
            "ATTRIB" => {
                if e.i(70, 0) & 1 != 0 {
                    return None;
                }
                let t = self.text(e, TextKind::Attrib, None)?;
                self.draw(e, vec![Shape::Text(Box::new(t))])
            }
            "ATTDEF" => {
                let flags = e.i(70, 0);
                if flags & 1 != 0 {
                    return None;
                }
                let content = match space {
                    Space::Top => e.s(2),
                    Space::Block if flags & 2 != 0 => e.s(1),
                    Space::Block => return None,
                };
                let t = self.text(e, TextKind::Attrib, Some(content))?;
                self.draw(e, vec![Shape::Text(Box::new(t))])
            }
            "MTEXT" => {
                let t = self.mtext(e)?;
                self.draw(e, vec![Shape::Text(Box::new(t))])
            }
            "INSERT" => self.insert(e, space),
            "DIMENSION" | "ARC_DIMENSION" | "LARGE_RADIAL_DIMENSION" => {
                self.block_ref(e, e.s(2), vec![M3::ocs(e.extrusion(), 0.0)], "DIMENSION")
            }
            "ACAD_TABLE" => {
                // The table's look is its anonymous *T block, placed like an INSERT.
                let end =
                    e.g.iter()
                        .position(|g| g.code == 100 && g.is("AcDbTable"))
                        .unwrap_or(e.g.len());
                let head = Ent {
                    ty: e.ty,
                    g: &e.g[..end],
                    all: &e.g[..end],
                    subs: Vec::new(),
                };
                self.insert(&head, space)
            }
            "SOLID" | "TRACE" => {
                let o = geom::ocs(e.extrusion(), e.f(30, 0.0));
                let (a, b, c) = (e.p2(10), e.p2(11), e.p2(12));
                let d = if e.has(13) { e.p2(13) } else { c };
                let mut p = BezPath::new();
                p.move_to(a);
                p.line_to(b);
                p.line_to(d);
                p.line_to(c);
                p.close_path();
                p.apply_affine(o);
                self.draw(
                    e,
                    vec![Shape::Fill {
                        path: p,
                        even_odd: false,
                    }],
                )
            }
            "3DFACE" => {
                let pts = [e.p2(10), e.p2(11), e.p2(12), e.p2(13)];
                let hidden = e.i(70, 0);
                let mut p = BezPath::new();
                for i in 0..4 {
                    if hidden & (1 << i) != 0 {
                        continue;
                    }
                    let (a, b) = (pts[i], pts[(i + 1) % 4]);
                    if a != b {
                        p.move_to(a);
                        p.line_to(b);
                    }
                }
                if p.elements().is_empty() {
                    return None;
                }
                self.draw(e, vec![Self::stroke(p)])
            }
            "HATCH" | "MPOLYGON" => {
                let (shapes, color) = self.hatch(e);
                let mut node = self.draw(e, shapes)?;
                if let (Some(c), Node::Draw { props, .. }) = (color, &mut node) {
                    props.color = ColorSpec::Is(c);
                }
                Some(node)
            }
            "LEADER" => {
                let shapes = self.leader(e);
                self.draw(e, shapes)
            }
            "MLEADER" | "MULTILEADER" => {
                let shapes = self.mleader(e);
                self.draw(e, shapes)
            }
            "WIPEOUT" => {
                let p = self.wipeout(e)?;
                let mut node = self.draw(
                    e,
                    vec![Shape::Fill {
                        path: p,
                        even_odd: false,
                    }],
                )?;
                if let Node::Draw { props, .. } = &mut node {
                    props.color = ColorSpec::Is(Color::Background);
                }
                Some(node)
            }
            "XLINE" | "RAY" => {
                let dir = e.p3(11).xy();
                if dir.hypot() < 1e-12 {
                    return None;
                }
                Some(Node::Infinite {
                    props: self.props(e),
                    origin: e.p2(10),
                    dir,
                    ray: ty == "RAY",
                })
            }
            "IMAGE" => {
                // The picture lives in another file; its frame is drawn where it goes.
                self.warn("images are drawn as their frames".to_string());
                let p = Self::image_frame(e)?;
                self.draw(e, vec![Self::stroke(p)])
            }
            "MESH" => {
                let p = Self::mesh(e)?;
                self.draw(e, vec![Self::stroke(p)])
            }
            "VIEWPORT" => self.viewport(e),
            "SEQEND" | "VERTEX" | "ENDBLK" => None,
            other => {
                self.warn(format!("{other} entities are not drawn"));
                None
            }
        }
    }

    // --------------------------------------------------------------------- polylines

    fn lwpolyline(&mut self, e: &Ent<'a>) -> Vec<Shape> {
        let mut verts: Vec<(Point, f64, f64, f64)> = Vec::new();
        let cw = e.f(43, 0.0);
        let mut x = None;
        for g in e.g {
            match g.code {
                10 => x = Some(g.f64()),
                20 => {
                    if let Some(x) = x.take()
                        && verts.len() < self.limits.max_points
                    {
                        verts.push((Point::new(x, g.f64()), cw, cw, 0.0));
                    }
                }
                40 => {
                    if let Some(v) = verts.last_mut() {
                        v.1 = g.f64();
                    }
                }
                41 => {
                    if let Some(v) = verts.last_mut() {
                        v.2 = g.f64();
                    }
                }
                42 => {
                    if let Some(v) = verts.last_mut() {
                        v.3 = g.f64();
                    }
                }
                _ => {}
            }
        }
        if verts.len() >= self.limits.max_points {
            self.warn("a polyline was cut at the point limit");
        }
        let closed = e.i(70, 0) & 1 != 0;
        let o = geom::ocs(e.extrusion(), e.f(38, 0.0));
        poly_shapes(&verts, closed, o)
    }

    fn polyline(&mut self, e: &Ent<'a>) -> Vec<Shape> {
        let flags = e.i(70, 0);
        let vs: Vec<&Ent<'a>> = e.subs.iter().filter(|s| s.is("VERTEX")).collect();
        if flags & 64 != 0 {
            // Polyface mesh: position vertices, then faces of up to four 1-based indices
            // (negative: invisible edge).
            let pos: Vec<Point> = vs
                .iter()
                .filter(|v| v.i(70, 0) & 64 != 0)
                .map(|v| v.p2(10))
                .collect();
            let mut p = BezPath::new();
            for f in vs.iter().filter(|v| v.i(70, 0) & 192 == 128) {
                let idx: Vec<i64> = (71..=74).map(|c| f.i(c, 0)).filter(|&i| i != 0).collect();
                for k in 0..idx.len() {
                    let a = idx[k];
                    let b = idx[(k + 1) % idx.len()];
                    if a < 0 {
                        continue;
                    }
                    let pa = pos.get((a.unsigned_abs() as usize).wrapping_sub(1));
                    let pb = pos.get((b.unsigned_abs() as usize).wrapping_sub(1));
                    if let (Some(&pa), Some(&pb)) = (pa, pb) {
                        p.move_to(pa);
                        p.line_to(pb);
                    }
                }
            }
            return if p.elements().is_empty() {
                vec![]
            } else {
                vec![Self::stroke(p)]
            };
        }
        if flags & 16 != 0 {
            // Polygon mesh: M rows of N vertices.
            let m = e.i(71, 0).max(0) as usize;
            let n = e.i(72, 0).max(0) as usize;
            let pts: Vec<Point> = vs.iter().map(|v| v.p2(10)).collect();
            if m == 0 || n == 0 || m.saturating_mul(n) > pts.len() {
                return vec![];
            }
            let mut p = BezPath::new();
            let (mc, nc) = (flags & 1 != 0, flags & 32 != 0);
            for i in 0..m {
                p.move_to(pts[i * n]);
                for j in 1..n {
                    p.line_to(pts[i * n + j]);
                }
                if nc {
                    p.close_path();
                }
            }
            for j in 0..n {
                p.move_to(pts[j]);
                for i in 1..m {
                    p.line_to(pts[i * n + j]);
                }
                if mc {
                    p.close_path();
                }
            }
            return vec![Self::stroke(p)];
        }
        let is3d = flags & 8 != 0;
        let (dw0, dw1) = (e.f(40, 0.0), e.f(41, 0.0));
        let fitted = flags & 6 != 0;
        let verts: Vec<(Point, f64, f64, f64)> = vs
            .iter()
            .filter(|v| !(fitted && v.i(70, 0) & 16 != 0))
            .take(self.limits.max_points)
            .map(|v| {
                (
                    v.p2(10),
                    v.f(40, dw0),
                    v.f(41, dw1),
                    if is3d { 0.0 } else { v.f(42, 0.0) },
                )
            })
            .collect();
        let o = if is3d {
            Affine::IDENTITY
        } else {
            geom::ocs(e.extrusion(), e.f(30, 0.0))
        };
        poly_shapes(&verts, flags & 1 != 0, o)
    }

    // ------------------------------------------------------------------------ splines

    fn spline(&mut self, e: &Ent<'a>) -> Option<BezPath> {
        let mut knots = Vec::new();
        let mut weights = Vec::new();
        let mut ctrl = Vec::new();
        let mut fit = Vec::new();
        let (mut cx, mut fx) = (None, None);
        let (mut t0, mut t1) = (None, None);
        let mut tx = (0.0, 0.0);
        let mut seen = false;
        for g in e.g {
            match g.code {
                100 if g.is("AcDbSpline") => seen = true,
                100 if seen && !g.is("AcDbEntity") => break,
                40 => knots.push(g.f64()),
                41 => weights.push(g.f64()),
                10 => cx = Some(g.f64()),
                20 => {
                    if let Some(x) = cx.take() {
                        ctrl.push(Point::new(x, g.f64()));
                    }
                }
                11 => fx = Some(g.f64()),
                21 => {
                    if let Some(x) = fx.take() {
                        fit.push(Point::new(x, g.f64()));
                    }
                }
                12 => tx.0 = g.f64(),
                22 => t0 = Some(Vec2::new(tx.0, g.f64())),
                13 => tx.1 = g.f64(),
                23 => t1 = Some(Vec2::new(tx.1, g.f64())),
                _ => {}
            }
            if ctrl.len() > self.limits.max_points || knots.len() > self.limits.max_points {
                self.warn("a spline was skipped at the point limit");
                return None;
            }
        }
        let degree = e.i(71, 3).clamp(1, 32) as usize;
        if !ctrl.is_empty() {
            let s = Spline {
                degree,
                knots,
                ctrl,
                weights,
            };
            if let Some(p) = s.to_path(self.limits.max_points) {
                return Some(p);
            }
            if fit.len() < 2 {
                // Inconsistent knots: fall back to the control polygon.
                self.warn("a spline with inconsistent knots is drawn as its control polygon");
                let mut p = BezPath::new();
                for (i, &q) in s.ctrl.iter().enumerate() {
                    if i == 0 {
                        p.move_to(q);
                    } else {
                        p.line_to(q);
                    }
                }
                return (s.ctrl.len() >= 2).then_some(p);
            }
        }
        if fit.len() >= 2 {
            let mut p = geom::through_points(&fit, t0, t1);
            if e.i(70, 0) & 1 != 0 {
                p.close_path();
            }
            return Some(p);
        }
        None
    }

    // -------------------------------------------------------------------------- text

    fn style_span(&mut self, e: &Ent<'a>, height: f64) -> Span {
        let font = self.font_of(e.s(7));
        let fs = self.fonts.get(font as usize).cloned().unwrap_or_default();
        let mut sp = Span::new("", font, height);
        let wf = e.f(41, fs.width_factor);
        sp.width_factor = if wf > 0.0 { wf } else { 1.0 };
        sp.oblique = e.f(51, fs.oblique);
        sp
    }

    fn text(&mut self, e: &Ent<'a>, kind: TextKind, content: Option<&[u8]>) -> Option<TextBlock> {
        let raw = content.unwrap_or_else(|| e.get(1).map_or(&b""[..], |g| g.bytes()));
        let s = self.dec.decode(raw);
        if s.trim().is_empty() {
            return None;
        }
        let h = e.f(40, 0.0);
        let h = if h > 0.0 { h } else { 1.0 };
        let base = self.style_span(e, h);
        let spans = text::single_line(&s, &base);
        let ha = e.i(72, 0);
        let va = match kind {
            TextKind::Text => e.i(73, 0),
            TextKind::Attrib => e.i(74, 0),
        };
        let (halign, valign) = match ha {
            1 => (HAlign::Center, valign_of(va)),
            2 => (HAlign::Right, valign_of(va)),
            3 | 5 => (HAlign::Fit, VAlign::Baseline),
            4 => (HAlign::Center, VAlign::Middle),
            _ => (HAlign::Left, valign_of(va)),
        };
        let p10 = e.p2(10);
        let p11 = if e.has(11) { e.p2(11) } else { p10 };
        let mut rot = e.f(50, 0.0).to_radians();
        let mut fit = None;
        let anchor = if halign == HAlign::Fit {
            let d = p11 - p10;
            if d.hypot() > 1e-12 {
                rot = d.atan2();
                fit = Some((d.hypot(), ha == 3));
            }
            p10
        } else if ha == 0 && va == 0 {
            p10
        } else {
            p11
        };
        let mirror = e.i(71, 0);
        let mx = if mirror & 2 != 0 { -1.0 } else { 1.0 };
        let my = if mirror & 4 != 0 { -1.0 } else { 1.0 };
        let transform = geom::ocs(e.extrusion(), e.f(30, 0.0))
            * Affine::translate(anchor.to_vec2())
            * Affine::rotate(rot)
            * Affine::scale_non_uniform(mx, my);
        Some(TextBlock {
            transform,
            halign: if fit.is_none() && halign == HAlign::Fit {
                HAlign::Left
            } else {
                halign
            },
            valign,
            wrap: 0.0,
            fit,
            line_spacing: 1.0,
            paragraphs: vec![Paragraph { spans, align: None }],
        })
    }

    fn mtext(&mut self, e: &Ent<'a>) -> Option<TextBlock> {
        let mut raw: Vec<u8> = Vec::new();
        for g in e.g.iter().filter(|g| g.code == 3) {
            raw.extend_from_slice(g.bytes());
        }
        if let Some(g) = e.get(1) {
            raw.extend_from_slice(g.bytes());
        }
        let s = self.dec.decode(&raw);
        if s.trim().is_empty() {
            return None;
        }
        let h = e.f(40, 0.0);
        let h = if h > 0.0 { h } else { 1.0 };
        let mut base = self.style_span(e, h);
        base.width_factor = self
            .fonts
            .get(base.font as usize)
            .map_or(1.0, |f| f.width_factor);
        base.oblique = self
            .fonts
            .get(base.font as usize)
            .map_or(0.0, |f| f.oblique);
        let paragraphs = text::mtext(&s, &base);
        let attach = e.i(71, 1).clamp(1, 9);
        let halign = match (attach - 1) % 3 {
            0 => HAlign::Left,
            1 => HAlign::Center,
            _ => HAlign::Right,
        };
        let valign = match (attach - 1) / 3 {
            0 => VAlign::Top,
            1 => VAlign::Middle,
            _ => VAlign::Bottom,
        };
        let n = e.extrusion().norm().unwrap_or(V3::Z);
        let x = if e.has(11) { e.p3(11).norm() } else { None };
        let x = x.unwrap_or_else(|| {
            let r = e.f(50, 0.0).to_radians();
            let (ax, ay) = geom::ocs_axes(n);
            V3::new(
                ax.x * r.cos() + ay.x * r.sin(),
                ax.y * r.cos() + ay.y * r.sin(),
                ax.z * r.cos() + ay.z * r.sin(),
            )
        });
        let y = n.cross(x).norm().unwrap_or(V3::new(-x.y, x.x, 0.0));
        let ins = e.p2(10);
        let spacing = e.f(44, 1.0);
        Some(TextBlock {
            transform: Affine::new([x.x, x.y, y.x, y.y, ins.x, ins.y]),
            halign,
            valign,
            wrap: e.f(41, 0.0).max(0.0),
            fit: None,
            line_spacing: if spacing > 0.0 { spacing } else { 1.0 },
            paragraphs,
        })
    }

    // ---------------------------------------------------------------------- blocks

    fn insert(&mut self, e: &Ent<'a>, space: Space) -> Option<Node> {
        let name = e.s(2);
        let key = lower(&self.dec.decode(name));
        let Some(&block) = self.block_map.get(&key) else {
            self.warn(format!("block {key} is not defined"));
            return None;
        };
        let base = self.block_bases[block];
        let (sx, sy) = (e.f(41, 1.0), e.f(42, 1.0));
        let rot = e.f(50, 0.0).to_radians();
        let mut cols = e.i(70, 1).max(1) as u64;
        let mut rows = e.i(71, 1).max(1) as u64;
        let max = self.limits.max_array.max(1) as u64;
        if cols.saturating_mul(rows) > max {
            self.warn(format!("an INSERT array was cut to {max} copies"));
            cols = cols.min(max);
            rows = rows.min(max / cols).max(1);
        }
        // Arrays share one budget per file: each cell is a transform kept with the insert.
        self.array_cells += cols * rows;
        if self.array_cells > MAX_ARRAY_CELLS {
            self.warn("INSERT arrays past the limit were drawn once");
            cols = 1;
            rows = 1;
        }
        let (cs, rs) = (e.f(44, 0.0), e.f(45, 0.0));
        let head = M3::ocs(e.extrusion(), 0.0)
            * M3::translate(e.p3(10))
            * M3::from_affine(Affine::rotate(rot));
        let tail =
            M3::scale(sx, sy, e.f(43, 1.0)) * M3::translate(V3::new(-base.x, -base.y, -base.z));
        let mut xfs = Vec::with_capacity((rows * cols) as usize);
        for r in 0..rows {
            for c in 0..cols {
                let cell = V3::new(c as f64 * cs, r as f64 * rs, 0.0);
                xfs.push(head * M3::translate(cell) * tail);
            }
        }
        let attribs = if e.subs.is_empty() {
            Vec::new()
        } else {
            e.subs
                .iter()
                .filter(|s| s.is("ATTRIB") && s.i(60, 0) != 1)
                .filter_map(|s| self.convert(s, space))
                .collect()
        };
        Some(Node::Insert {
            props: self.props(e),
            block,
            xfs,
            attribs,
        })
    }

    fn block_ref(&mut self, e: &Ent<'a>, name: &[u8], xfs: Vec<M3>, what: &str) -> Option<Node> {
        if name.is_empty() {
            return None;
        }
        let key = lower(&self.dec.decode(name));
        let Some(&block) = self.block_map.get(&key) else {
            self.warn(format!("{what} block {key} is not defined"));
            return None;
        };
        Some(Node::Insert {
            props: self.props(e),
            block,
            xfs,
            attribs: Vec::new(),
        })
    }

    // ------------------------------------------------------------------------ hatch

    fn hatch(&mut self, e: &Ent<'a>) -> (Vec<Shape>, Option<Color>) {
        let g = e.g;
        let mut c = Cursor { g, i: 0 };
        // Up to the boundary path count.
        let mut elevation = 0.0;
        let mut solid = false;
        while let Some(x) = c.peek() {
            match x.code {
                30 => elevation = x.f64(),
                70 => solid = x.i64() & 1 != 0,
                91 => break,
                _ => {}
            }
            c.i += 1;
        }
        let n_paths = c.take(91).map_or(0, |x| x.i64()).clamp(0, 100_000) as usize;
        let mut paths: Vec<BezPath> = Vec::new();
        let mut points = 0usize;
        for _ in 0..n_paths {
            // Find the next path's flags.
            while let Some(x) = c.peek() {
                if x.code == 92 || x.code == 75 || x.code == 76 {
                    break;
                }
                c.i += 1;
            }
            let Some(flags) = c.take(92) else { break };
            let path = if flags.i64() & 2 != 0 {
                hatch_polyline(&mut c)
            } else {
                hatch_edges(&mut c, self.limits.max_points)
            };
            if let Some(p) = path {
                points += p.elements().len();
                paths.push(p);
            }
            if points > self.limits.max_points {
                self.warn("a hatch boundary was cut at the point limit");
                break;
            }
            if let Some(n) = c.take(97) {
                for _ in 0..n.i64().clamp(0, 1_000_000) {
                    if c.take(330).is_none() {
                        break;
                    }
                }
            }
        }
        // Pattern lines and gradient after the boundaries.
        let mut lines: Vec<PatternLine> = Vec::new();
        let mut gradient = false;
        let mut grad_color = None;
        let rest = &g[c.i.min(g.len())..];
        let mut k = 0;
        while k < rest.len() {
            let x = &rest[k];
            match x.code {
                53 => {
                    let mut l = PatternLine {
                        angle: x.f64().to_radians(),
                        base: Point::ZERO,
                        offset: Vec2::ZERO,
                        dashes: Vec::new(),
                    };
                    let mut j = k + 1;
                    while j < rest.len() {
                        let y = &rest[j];
                        match y.code {
                            43 => l.base.x = y.f64(),
                            44 => l.base.y = y.f64(),
                            45 => l.offset.x = y.f64(),
                            46 => l.offset.y = y.f64(),
                            79 => {}
                            49 => {
                                if l.dashes.len() < 64 {
                                    l.dashes.push(y.f64())
                                }
                            }
                            _ => break,
                        }
                        j += 1;
                    }
                    lines.push(l);
                    k = j;
                    continue;
                }
                450 => gradient = x.i64() != 0,
                421 if gradient && grad_color.is_none() => {
                    grad_color = Some(aci::true_color(x.i64()))
                }
                _ => {}
            }
            k += 1;
        }
        if paths.is_empty() {
            return (Vec::new(), None);
        }
        let o = geom::ocs(e.extrusion(), elevation);
        if solid || gradient || lines.is_empty() {
            if !solid && !gradient {
                // A pattern without line data: nothing to draw.
                return (Vec::new(), None);
            }
            let mut all = BezPath::new();
            for p in &paths {
                all.extend(p.iter());
            }
            all.apply_affine(o);
            return (
                vec![Shape::Fill {
                    path: all,
                    even_odd: true,
                }],
                if gradient { grad_color } else { None },
            );
        }
        let bounds = paths
            .iter()
            .map(|p| p.bounding_box())
            .reduce(|a, b| a.union(b))
            .unwrap_or(Rect::ZERO);
        let tol = (bounds.width().max(bounds.height()) * 1e-4).max(1e-9);
        let segs = hatch::segments(&paths, tol);
        // Each hatch is capped, and the file's hatches share one budget.
        let cap = self
            .limits
            .max_points
            .min(200_000)
            .min(hatch::MAX_TOTAL.saturating_sub(self.hatched));
        let (mut p, cut) = hatch::pattern(&segs, bounds, &lines, cap);
        self.hatched += p.elements().len();
        if cut {
            self.warn("a hatch pattern was cut at the segment limit");
        }
        if p.elements().is_empty() {
            return (Vec::new(), None);
        }
        p.apply_affine(o);
        (vec![Self::stroke(p)], None)
    }

    // ---------------------------------------------------------------- leaders etc.

    fn arrow_size(&self) -> f64 {
        self.header.dimasz * self.header.dimscale
    }

    fn leader(&mut self, e: &Ent<'a>) -> Vec<Shape> {
        let mut pts = Vec::new();
        let mut x = None;
        for g in e.g {
            match g.code {
                10 => x = Some(g.f64()),
                20 => {
                    if let Some(x) = x.take() {
                        pts.push(Point::new(x, g.f64()));
                    }
                }
                _ => {}
            }
        }
        if pts.len() < 2 {
            return vec![];
        }
        let mut p = BezPath::new();
        p.move_to(pts[0]);
        for &q in &pts[1..] {
            p.line_to(q);
        }
        let mut out = vec![Self::stroke(p)];
        if e.i(71, 1) != 0
            && let Some(a) = arrow(pts[0], pts[1], self.arrow_size())
        {
            out.push(a);
        }
        out
    }

    fn mleader(&mut self, e: &Ent<'a>) -> Vec<Shape> {
        let mut out = Vec::new();
        let mut depth = 0; // 1 context, 2 leader, 3 leader line
        let mut line: Vec<Point> = Vec::new();
        let mut landing: Option<Point> = None;
        let mut dogleg = (Vec2::ZERO, 0.0);
        let mut x = None;
        let mut arrow_size = self.arrow_size();
        let mut text_h = 0.0;
        let mut text_at = None;
        let mut text_dir = Vec2::new(1.0, 0.0);
        let mut content: Option<&[u8]> = None;
        // Per leader: its vertices, the landing point, and the dogleg (direction, length).
        type Leader = (Vec<Point>, Option<Point>, (Vec2, f64));
        let mut leaders: Vec<Leader> = Vec::new();
        let mut lines_of_leader: Vec<Vec<Point>> = Vec::new();
        for g in e.all {
            match (g.code, depth) {
                (300, _) => depth = 1,
                (302, _) => {
                    depth = 2;
                    landing = None;
                    dogleg = (Vec2::ZERO, 0.0);
                    lines_of_leader.clear();
                }
                (304, d) if g.is("LEADER_LINE{") && d >= 2 => {
                    depth = 3;
                    line.clear();
                }
                (305, 3) => {
                    lines_of_leader.push(std::mem::take(&mut line));
                    depth = 2;
                }
                (303, 2) => {
                    for l in lines_of_leader.drain(..) {
                        leaders.push((l, landing, dogleg));
                    }
                    depth = 1;
                }
                (301, _) => depth = 0,
                (10, 2 | 3) => x = Some(g.f64()),
                (20, 3) => {
                    if let Some(x) = x.take() {
                        line.push(Point::new(x, g.f64()));
                    }
                }
                (20, 2) => {
                    if let Some(x) = x.take() {
                        landing = Some(Point::new(x, g.f64()));
                    }
                }
                (11, 2) => dogleg.0.x = g.f64(),
                (21, 2) => dogleg.0.y = g.f64(),
                (40, 2) => dogleg.1 = g.f64(),
                (140, 1) => {
                    if g.f64() > 0.0 {
                        arrow_size = g.f64()
                    }
                }
                (41, 1) => text_h = g.f64(),
                (12, 1) => text_at = Some(Point::new(g.f64(), 0.0)),
                (22, 1) => {
                    if let Some(p) = text_at.as_mut() {
                        p.y = g.f64();
                    }
                }
                (13, 1) => text_dir.x = g.f64(),
                (23, 1) => text_dir.y = g.f64(),
                (304, 1) => content = Some(g.bytes()),
                _ => {}
            }
            if g.code == 1001 {
                break;
            }
        }
        for (pts, landing, (dir, len)) in leaders {
            let mut all = pts.clone();
            if let Some(l) = landing {
                all.push(l);
                if len > 0.0 && dir.hypot() > 1e-12 {
                    all.push(l + dir.normalize() * len);
                }
            }
            if all.len() < 2 {
                continue;
            }
            let mut p = BezPath::new();
            p.move_to(all[0]);
            for &q in &all[1..] {
                p.line_to(q);
            }
            out.push(Self::stroke(p));
            if let Some(a) = arrow(all[0], all[1], arrow_size) {
                out.push(a);
            }
        }
        if let (Some(raw), Some(at)) = (content, text_at) {
            let s = self.dec.decode(raw);
            if !s.trim().is_empty() {
                let h = if text_h > 0.0 { text_h } else { 2.5 };
                let base = self.style_span(e, h);
                let rot = text_dir.y.atan2(text_dir.x);
                out.push(Shape::Text(Box::new(TextBlock {
                    transform: Affine::translate(at.to_vec2()) * Affine::rotate(rot),
                    halign: HAlign::Left,
                    valign: VAlign::Top,
                    wrap: 0.0,
                    fit: None,
                    line_spacing: 1.0,
                    paragraphs: text::mtext(&s, &base),
                })));
            }
        }
        out
    }

    fn wipeout(&mut self, e: &Ent<'a>) -> Option<BezPath> {
        let ins = e.p2(10);
        let u = e.p3(11).xy();
        let v = e.p3(12).xy();
        let size = e.p2(13);
        let (w, h) = (size.x.max(1e-12), size.y.max(1e-12));
        let mut clip = Vec::new();
        let mut x = None;
        for g in e.g {
            match g.code {
                14 => x = Some(g.f64()),
                24 => {
                    if let Some(x) = x.take() {
                        clip.push(Point::new(x, g.f64()));
                    }
                }
                _ => {}
            }
        }
        if clip.len() == 2 {
            let (a, b) = (clip[0], clip[1]);
            clip = vec![a, Point::new(b.x, a.y), b, Point::new(a.x, b.y)];
        }
        if clip.len() < 3 {
            clip = vec![
                Point::new(-0.5, -0.5),
                Point::new(0.5, -0.5),
                Point::new(0.5, 0.5),
                Point::new(-0.5, 0.5),
            ];
        }
        let mut p = BezPath::new();
        for (i, q) in clip.iter().enumerate() {
            // Clip vertices are in image coordinates, centred, y down.
            let px = (q.x + 0.5) * w;
            let py = (0.5 - q.y) * h;
            let pt = ins + u * px + v * py;
            if i == 0 {
                p.move_to(pt);
            } else {
                p.line_to(pt);
            }
        }
        p.close_path();
        Some(p)
    }

    /// An IMAGE's frame: its clip boundary when clipping is on, else the whole picture.
    /// Boundary vertices are pixel coordinates from the top-left corner.
    fn image_frame(e: &Ent<'a>) -> Option<BezPath> {
        let ins = e.p2(10);
        let u = e.p3(11).xy();
        let v = e.p3(12).xy();
        let size = e.p2(13);
        let (w, h) = (size.x, size.y);
        if !(w > 0.0 && h > 0.0) || u.hypot() * v.hypot() < 1e-300 {
            return None;
        }
        let mut clip = Vec::new();
        if e.i(280, 0) == 1 {
            let mut x = None;
            for g in e.g {
                match g.code {
                    14 => x = Some(g.f64()),
                    24 => {
                        if let Some(x) = x.take() {
                            clip.push(Point::new(x, g.f64()));
                        }
                    }
                    _ => {}
                }
            }
        }
        if clip.len() == 2 {
            let (a, b) = (clip[0], clip[1]);
            clip = vec![a, Point::new(b.x, a.y), b, Point::new(a.x, b.y)];
        }
        if clip.len() < 3 {
            clip = vec![
                Point::new(-0.5, -0.5),
                Point::new(w - 0.5, -0.5),
                Point::new(w - 0.5, h - 0.5),
                Point::new(-0.5, h - 0.5),
            ];
        }
        let mut p = BezPath::new();
        for (i, q) in clip.iter().enumerate() {
            let pt = ins + u * (q.x + 0.5) + v * (h - q.y - 0.5);
            if i == 0 {
                p.move_to(pt);
            } else {
                p.line_to(pt);
            }
        }
        p.close_path();
        Some(p)
    }

    /// A MESH's control cage as a wireframe, each shared edge once (subdivision is not
    /// applied).
    fn mesh(e: &Ent<'a>) -> Option<BezPath> {
        let mut verts = Vec::new();
        let mut list = Vec::new();
        let mut x = None;
        let mut in_faces = false;
        let mut left = 0usize;
        for g in e.g {
            match g.code {
                10 => x = Some(g.f64()),
                20 => {
                    if let Some(x) = x.take() {
                        verts.push(Point::new(x, g.f64()));
                    }
                }
                93 => {
                    in_faces = true;
                    left = g.i64().max(0) as usize;
                }
                90 if in_faces && left > 0 => {
                    list.push(g.i64());
                    left -= 1;
                }
                94 => in_faces = false,
                _ => {}
            }
        }
        let mut seen = std::collections::HashSet::new();
        let mut p = BezPath::new();
        let mut i = 0;
        while i < list.len() {
            let n = list[i].max(0) as usize;
            let end = (i + 1).saturating_add(n).min(list.len());
            let face = &list[i + 1..end];
            i = end;
            for (k, &a) in face.iter().enumerate() {
                let b = face[(k + 1) % face.len()];
                let (Ok(a), Ok(b)) = (usize::try_from(a), usize::try_from(b)) else {
                    continue;
                };
                if a == b
                    || a >= verts.len()
                    || b >= verts.len()
                    || !seen.insert((a.min(b), a.max(b)))
                {
                    continue;
                }
                p.move_to(verts[a]);
                p.line_to(verts[b]);
            }
        }
        (!p.elements().is_empty()).then_some(p)
    }

    fn viewport(&mut self, e: &Ent<'a>) -> Option<Node> {
        if e.i(69, 0) == 1 || e.i(68, 1) == 0 {
            return None;
        }
        let vh = e.f(45, 0.0);
        let (w, h) = (e.f(40, 0.0), e.f(41, 0.0));
        if !(vh > 0.0 && w > 0.0 && h > 0.0) {
            if !e.has(45) {
                self.warn("R12 viewports are not shown");
            }
            return None;
        }
        let c = e.p2(10);
        let s = h / vh;
        let view = e.p2(12).to_vec2() + e.p3(17).xy();
        let transform = Affine::translate(c.to_vec2())
            * Affine::rotate(e.f(51, 0.0).to_radians())
            * Affine::scale(s)
            * Affine::translate(-view);
        let frozen =
            e.g.iter()
                .filter(|g| g.code == 341)
                .map(|g| g.handle())
                .collect();
        let clip = (e.i(90, 0) & 0x10000 != 0)
            .then(|| e.get(340).map(|g| g.handle()))
            .flatten();
        Some(Node::Viewport(Box::new(VpDef {
            paper: Rect::new(c.x - w / 2.0, c.y - h / 2.0, c.x + w / 2.0, c.y + h / 2.0),
            transform,
            frozen,
            clip,
        })))
    }
}

#[derive(Clone, Copy, PartialEq)]
enum TextKind {
    Text,
    Attrib,
}

fn valign_of(v: i64) -> VAlign {
    match v {
        1 => VAlign::Bottom,
        2 => VAlign::Middle,
        3 => VAlign::Top,
        _ => VAlign::Baseline,
    }
}

/// Counter-clockwise sweep from `a0` to `a1` in (0, 2π].
fn ccw_sweep(a0: f64, a1: f64) -> f64 {
    let tau = std::f64::consts::TAU;
    let mut s = (a1 - a0) % tau;
    if s <= 1e-12 {
        s += tau;
    }
    s
}

/// A filled arrowhead with its tip at `tip`, pointing away from `toward`.
fn arrow(tip: Point, toward: Point, size: f64) -> Option<Shape> {
    let d = toward - tip;
    let len = d.hypot();
    if !(size > 0.0 && len > size) {
        return None;
    }
    let u = d / len;
    let n = Vec2::new(-u.y, u.x) * (size / 6.0);
    let b = tip + u * size;
    let mut p = BezPath::new();
    p.move_to(tip);
    p.line_to(b + n);
    p.line_to(b - n);
    p.close_path();
    Some(Shape::Fill {
        path: p,
        even_odd: false,
    })
}

/// Shapes of a 2D polyline: a stroke, or a fill when it has width.
fn poly_shapes(verts: &[(Point, f64, f64, f64)], closed: bool, o: Affine) -> Vec<Shape> {
    if verts.is_empty() {
        return vec![];
    }
    let n = verts.len();
    let segs = if closed { n } else { n - 1 };
    let wide = verts.iter().any(|v| v.1 > 0.0 || v.2 > 0.0);
    if !wide {
        let mut p = BezPath::new();
        p.move_to(verts[0].0);
        if n == 1 {
            p.line_to(verts[0].0);
        }
        for i in 0..segs {
            let a = verts[i];
            let b = verts[(i + 1) % n];
            geom::bulge_to(&mut p, a.0, b.0, a.3);
        }
        if closed && n > 1 {
            p.close_path();
        }
        p.apply_affine(o);
        return vec![Builder::stroke(p)];
    }
    // Width: one outline per segment (start and end widths, arcs sampled), filled.
    let mut p = BezPath::new();
    for i in 0..segs {
        let a = verts[i];
        let b = verts[(i + 1) % n];
        let samples = geom::bulge_samples(a.0, b.0, a.3, 16);
        let (w0, w1) = (a.1, a.2);
        let center = if a.3.abs() > 1e-10 {
            Some(geom::bulge_arc(a.0, b.0, a.3).0)
        } else {
            None
        };
        let normal = |q: Point| -> Vec2 {
            match center {
                Some(c) => {
                    let r = q - c;
                    let l = r.hypot();
                    if l > 1e-12 { r / l } else { Vec2::ZERO }
                }
                None => {
                    let d = b.0 - a.0;
                    let l = d.hypot();
                    if l > 1e-12 {
                        Vec2::new(-d.y, d.x) / l
                    } else {
                        Vec2::ZERO
                    }
                }
            }
        };
        let mut left = Vec::new();
        let mut right = Vec::new();
        for (q, f) in samples {
            let w = (w0 + (w1 - w0) * f) / 2.0;
            let nn = normal(q) * w;
            left.push(q + nn);
            right.push(q - nn);
        }
        if left.len() < 2 {
            continue;
        }
        p.move_to(left[0]);
        for &q in &left[1..] {
            p.line_to(q);
        }
        for &q in right.iter().rev() {
            p.line_to(q);
        }
        p.close_path();
    }
    p.apply_affine(o);
    vec![Shape::Fill {
        path: p,
        even_odd: false,
    }]
}

struct Cursor<'g, 'a> {
    g: &'g [Group<'a>],
    i: usize,
}

impl<'g, 'a> Cursor<'g, 'a> {
    fn peek(&self) -> Option<&'g Group<'a>> {
        self.g.get(self.i)
    }

    /// The next group if it has `code`.
    fn take(&mut self, code: i32) -> Option<&'g Group<'a>> {
        let g = self.g.get(self.i).filter(|g| g.code == code)?;
        self.i += 1;
        Some(g)
    }

    fn f(&mut self, code: i32) -> Option<f64> {
        self.take(code).map(|g| g.f64())
    }

    fn pt(&mut self, x: i32) -> Option<Point> {
        let a = self.f(x)?;
        let b = self.f(x + 10)?;
        Some(Point::new(a, b))
    }
}

fn hatch_polyline(c: &mut Cursor<'_, '_>) -> Option<BezPath> {
    let has_bulge = c.take(72).is_some_and(|g| g.i64() != 0);
    let _closed = c.take(73);
    let n = c.take(93)?.i64().clamp(0, 10_000_000) as usize;
    let mut verts: Vec<(Point, f64)> = Vec::new();
    for _ in 0..n {
        let Some(p) = c.pt(10) else { break };
        let b = if has_bulge {
            c.f(42).unwrap_or(0.0)
        } else {
            0.0
        };
        verts.push((p, b));
    }
    if verts.len() < 2 {
        return None;
    }
    let mut p = BezPath::new();
    p.move_to(verts[0].0);
    for i in 0..verts.len() {
        let (a, b) = (verts[i], verts[(i + 1) % verts.len()]);
        if i + 1 == verts.len() && a.0 == b.0 {
            break;
        }
        geom::bulge_to(&mut p, a.0, b.0, a.1);
    }
    p.close_path();
    Some(p)
}

fn hatch_edges(c: &mut Cursor<'_, '_>, max_points: usize) -> Option<BezPath> {
    let n = c.take(93)?.i64().clamp(0, 10_000_000) as usize;
    let mut out = BezPath::new();
    for _ in 0..n {
        let Some(t) = c.take(72) else { break };
        let mut seg = BezPath::new();
        match t.i64() {
            1 => {
                let (Some(a), Some(b)) = (c.pt(10), c.pt(11)) else {
                    break;
                };
                seg.move_to(a);
                seg.line_to(b);
            }
            2 => {
                let ctr = c.pt(10).unwrap_or(Point::ZERO);
                let r = c.f(40).unwrap_or(0.0);
                let s = c.f(50).unwrap_or(0.0).to_radians();
                let e = c.f(51).unwrap_or(0.0).to_radians();
                let ccw = c.take(73).is_none_or(|g| g.i64() != 0);
                let sweep = full_or(s, e);
                let (a0, sw) = if ccw { (s, sweep) } else { (-s, -sweep) };
                geom::ellipse_arc(
                    &mut seg,
                    ctr,
                    Vec2::new(r, 0.0),
                    Vec2::new(0.0, r),
                    a0,
                    sw,
                    true,
                );
            }
            3 => {
                let ctr = c.pt(10).unwrap_or(Point::ZERO);
                let major = c.pt(11).unwrap_or(Point::new(1.0, 0.0)).to_vec2();
                let ratio = c.f(40).unwrap_or(1.0);
                let s = c.f(50).unwrap_or(0.0).to_radians();
                let e = c.f(51).unwrap_or(0.0).to_radians();
                let ccw = c.take(73).is_none_or(|g| g.i64() != 0);
                let minor = Vec2::new(-major.y, major.x) * ratio;
                // Angles → ellipse parameters.
                let param = |a: f64| a.sin().atan2(a.cos() * ratio);
                let (s, e) = if ccw { (s, e) } else { (-s, -e) };
                let (ps, pe) = (param(s), param(e));
                let sweep = if ccw {
                    full_or(ps, pe)
                } else {
                    -full_or(pe, ps)
                };
                let sweep = if ratio.abs() < 1e-12 { 0.0 } else { sweep };
                geom::ellipse_arc(&mut seg, ctr, major, minor, ps, sweep, true);
            }
            4 => {
                let degree = c.take(94).map_or(3, |g| g.i64()).clamp(1, 32) as usize;
                let rational = c.take(73).is_some_and(|g| g.i64() != 0);
                let _periodic = c.take(74);
                let nk = c
                    .take(95)
                    .map_or(0, |g| g.i64())
                    .clamp(0, max_points as i64) as usize;
                let nc = c
                    .take(96)
                    .map_or(0, |g| g.i64())
                    .clamp(0, max_points as i64) as usize;
                let mut knots = Vec::with_capacity(nk);
                for _ in 0..nk {
                    let Some(k) = c.f(40) else { break };
                    knots.push(k);
                }
                let mut ctrl = Vec::with_capacity(nc);
                let mut weights = Vec::new();
                for _ in 0..nc {
                    let Some(p) = c.pt(10) else { break };
                    ctrl.push(p);
                    if rational && let Some(w) = c.f(42) {
                        weights.push(w);
                    }
                }
                let mut fit = Vec::new();
                if let Some(nf) = c.take(97) {
                    for _ in 0..nf.i64().clamp(0, max_points as i64) {
                        let Some(p) = c.pt(11) else { break };
                        fit.push(p);
                    }
                    let t0 = c.pt(12).map(|p| p.to_vec2());
                    let t1 = c.pt(13).map(|p| p.to_vec2());
                    if ctrl.is_empty() && fit.len() >= 2 {
                        seg = geom::through_points(&fit, t0, t1);
                    }
                }
                if !ctrl.is_empty() {
                    let s = Spline {
                        degree,
                        knots,
                        ctrl,
                        weights,
                    };
                    if let Some(p) = s.to_path(max_points) {
                        seg = p;
                    } else {
                        seg.move_to(s.ctrl[0]);
                        for &q in &s.ctrl[1..] {
                            seg.line_to(q);
                        }
                    }
                }
            }
            _ => break,
        }
        // Join edges into one boundary: the first move starts it, later ones connect.
        for (k, el) in seg.elements().iter().enumerate() {
            match (k, el) {
                (0, kurbo::PathEl::MoveTo(p)) if !out.elements().is_empty() => out.line_to(*p),
                (_, el) => out.push(*el),
            }
        }
    }
    if out.elements().is_empty() {
        return None;
    }
    out.close_path();
    Some(out)
}

/// The counter-clockwise sweep from `a` to `b`, a full turn when they coincide.
fn full_or(a: f64, b: f64) -> f64 {
    let tau = std::f64::consts::TAU;
    let s = (b - a).rem_euclid(tau);
    if s < 1e-9 { tau } else { s }
}
