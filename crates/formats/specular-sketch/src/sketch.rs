//! Sketch layer trees drawn into display lists.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;
use specular_drawpage::kurbo::{self, BezPath, Ellipse, Point, Rect, RoundedRect, Shape, Vec2};
use specular_drawpage::{
    Affine, Cap, HAlign, Image, ImageDecoder, Join, MAX_IMAGE_PIXELS, MAX_OPS, Op, Page, Paint,
    Paragraph, Rgba, Run, Stop, Stroke, TextBlock, VAlign,
};
use specular_ora::zip::Zip;

use crate::{Error, MAX_IMAGE_MEMBER, MAX_JSON, SketchDocument, SketchFacts, member, zip_of};

/// Most layers drawn per document (symbol instances count their masters' layers again).
pub const MAX_LAYERS: usize = 1 << 20;
/// Deepest group nesting drawn.
pub const MAX_DEPTH: usize = 64;
/// Deepest symbol-in-symbol nesting drawn.
pub const MAX_SYMBOL_DEPTH: usize = 16;
/// Most pages read.
const MAX_PAGES: usize = 4096;

struct Ctx<'a> {
    data: &'a [u8],
    zip: &'a Zip,
    decode: Option<ImageDecoder>,
    symbols: HashMap<String, &'a Value>,
    /// Shared layer and text styles by id: their style objects.
    styles: HashMap<String, &'a Value>,
    images: HashMap<String, Option<Arc<Image>>>,
    image_pixels: u64,
    layers: usize,
    facts: SketchFacts,
    ops: Vec<Op>,
    /// Path elements in the operations made ([`specular_drawpage::MAX_PATH_ELEMENTS`]).
    path_elements: usize,
    /// Symbol ids being drawn (to stop recursion).
    symbol_stack: Vec<String>,
}

fn json(data: &[u8], zip: &Zip, name: &str) -> Result<Option<Value>, Error> {
    let Some(b) = member(data, zip, name, MAX_JSON)? else {
        return Ok(None);
    };
    serde_json::from_slice(b.get(data))
        .map(Some)
        .map_err(|e| Error::Malformed(format!("{name}: {e}")))
}

/// The parsed parts of a document.
struct Parsed {
    meta: Option<Value>,
    doc: Value,
    /// Page JSON in document order.
    pages: Vec<Value>,
}

fn parse(data: &[u8], zip: &Zip) -> Result<Parsed, Error> {
    let doc = json(data, zip, "document.json")?.ok_or(Error::NotSketch)?;
    let meta = json(data, zip, "meta.json").ok().flatten();
    let mut pages = Vec::new();
    let refs: Vec<String> = doc
        .get("pages")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|p| p.get("_ref").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let refs = if refs.is_empty() {
        // Fall back to whatever pages the archive holds.
        zip.entries
            .iter()
            .filter(|e| e.name.starts_with("pages/") && e.name.ends_with(".json"))
            .map(|e| e.name.trim_end_matches(".json").to_string())
            .collect()
    } else {
        refs
    };
    for r in refs.iter().take(MAX_PAGES) {
        let name = if r.ends_with(".json") {
            r.clone()
        } else {
            format!("{r}.json")
        };
        if let Some(p) = json(data, zip, &name)? {
            pages.push(p);
        }
    }
    if pages.is_empty() {
        return Err(Error::Unsupported("Sketch document without pages".into()));
    }
    Ok(Parsed { meta, doc, pages })
}

fn facts_of(meta: Option<&Value>, pages: &[Value]) -> SketchFacts {
    let mut f = SketchFacts::default();
    if let Some(m) = meta {
        f.app_version = m
            .get("appVersion")
            .and_then(Value::as_str)
            .map(str::to_string);
        f.version = m.get("version").and_then(Value::as_u64);
        f.compatibility_version = m.get("compatibilityVersion").and_then(Value::as_u64);
        f.app = m.get("app").and_then(Value::as_str).map(str::to_string);
    }
    f.sketch_pages = pages.iter().map(|p| name_of(p).to_string()).collect();
    f
}

fn name_of(v: &Value) -> &str {
    v.get("name").and_then(Value::as_str).unwrap_or("")
}

fn class_of(v: &Value) -> &str {
    v.get("_class").and_then(Value::as_str).unwrap_or("")
}

fn layers_of(v: &Value) -> &[Value] {
    v.get("layers")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn num(v: &Value, k: &str) -> Option<f64> {
    v.get(k).and_then(Value::as_f64).filter(|x| x.is_finite())
}

fn flag(v: &Value, k: &str) -> bool {
    v.get(k).and_then(Value::as_bool).unwrap_or(false)
}

fn visible(v: &Value) -> bool {
    v.get("isVisible").and_then(Value::as_bool).unwrap_or(true)
}

fn frame(v: &Value) -> Rect {
    let Some(f) = v.get("frame") else {
        return Rect::ZERO;
    };
    let g = |k| num(f, k).unwrap_or(0.0);
    let (w, h) = (g("width").max(0.0), g("height").max(0.0));
    Rect::new(g("x"), g("y"), g("x") + w, g("y") + h)
}

/// "{0.5, 1}" → (0.5, 1).
fn point_str(s: &str) -> Option<Point> {
    let s = s.trim().trim_start_matches('{').trim_end_matches('}');
    let (a, b) = s.split_once(',')?;
    let (x, y) = (a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?);
    (x.is_finite() && y.is_finite()).then(|| Point::new(x, y))
}

fn color(v: Option<&Value>) -> Option<Rgba> {
    let c = v?;
    let ch = |k: &str| (num(c, k).unwrap_or(0.0).clamp(0.0, 1.0) * 255.0).round() as u8;
    Some([ch("red"), ch("green"), ch("blue"), {
        let a = num(c, "alpha").unwrap_or(1.0).clamp(0.0, 1.0);
        (a * 255.0).round() as u8
    }])
}

fn with_alpha(c: Rgba, a: f64) -> Rgba {
    [
        c[0],
        c[1],
        c[2],
        (f64::from(c[3]) * a.clamp(0.0, 1.0)).round() as u8,
    ]
}

fn context_opacity(v: Option<&Value>) -> f64 {
    v.and_then(|c| c.get("contextSettings"))
        .and_then(|c| num(c, "opacity"))
        .unwrap_or(1.0)
        .clamp(0.0, 1.0)
}

/// The layer's own transform within its parent: frame offset, then rotation and flips about
/// the frame's centre.
fn local(v: &Value) -> Affine {
    let f = frame(v);
    let (w, h) = (f.width(), f.height());
    let mut t = Affine::translate((f.x0, f.y0));
    let rot = num(v, "rotation").unwrap_or(0.0);
    let fx = flag(v, "isFlippedHorizontal");
    let fy = flag(v, "isFlippedVertical");
    if rot != 0.0 || fx || fy {
        let c = Vec2::new(w / 2.0, h / 2.0);
        t = t
            * Affine::translate(c)
            * Affine::rotate(-rot.to_radians())
            * Affine::scale_non_uniform(if fx { -1.0 } else { 1.0 }, if fy { -1.0 } else { 1.0 })
            * Affine::translate(-c);
    }
    t
}

impl<'a> Ctx<'a> {
    fn count(&mut self, class: &str) {
        if let Some(e) = self.facts.layer_counts.iter_mut().find(|(k, _)| k == class) {
            e.1 += 1;
        } else if self.facts.layer_counts.len() < 256 {
            self.facts.layer_counts.push((class.to_string(), 1));
        }
    }

    fn not_drawn(&mut self, what: &str) {
        if let Some(e) = self.facts.not_drawn.iter_mut().find(|(k, _)| k == what) {
            e.1 += 1;
        } else if self.facts.not_drawn.len() < 64 {
            self.facts.not_drawn.push((what.to_string(), 1));
        }
    }

    fn push(&mut self, op: Op) -> bool {
        let elements = self.path_elements + op.path_elements();
        if self.ops.len() >= MAX_OPS || elements > specular_drawpage::MAX_PATH_ELEMENTS {
            self.facts.limited = true;
            return false;
        }
        self.path_elements = elements;
        self.ops.push(op);
        true
    }

    fn image(&mut self, r: &str) -> Option<Arc<Image>> {
        if let Some(i) = self.images.get(r) {
            return i.clone();
        }
        let img = self.load_image(r);
        if img.is_none() {
            self.facts.images_missing += 1;
        }
        if self.images.len() < 65_536 {
            self.images.insert(r.to_string(), img.clone());
        }
        img
    }

    fn load_image(&mut self, r: &str) -> Option<Arc<Image>> {
        let dec = self.decode.clone()?;
        let mut name = r.to_string();
        if self.zip.find(&name).is_none() {
            // References may omit the extension.
            name = self
                .zip
                .entries
                .iter()
                .find(|e| e.name.starts_with(r) && e.name.len() <= r.len() + 5)?
                .name
                .clone();
        }
        let b = member(self.data, self.zip, &name, MAX_IMAGE_MEMBER).ok()??;
        let (w, h, px) = dec(b.get(self.data))?;
        let n = u64::from(w) * u64::from(h);
        if self.image_pixels + n > MAX_IMAGE_PIXELS {
            self.facts.limited = true;
            return None;
        }
        self.image_pixels += n;
        Image::from_rgba(w, h, px).map(Arc::new)
    }

    // ----- Layers ----------------------------------------------------------------------

    fn children(&mut self, list: &'a [Value], t: Affine, depth: usize, ov: &Overrides) {
        let mut masked = false;
        for c in list {
            if masked && flag(c, "shouldBreakMaskChain") {
                self.push(Op::Pop);
                masked = false;
            }
            if flag(c, "hasClippingMask") && visible(c) {
                if masked {
                    self.push(Op::Pop);
                    masked = false;
                }
                self.layer(c, t, depth, ov);
                if let Some((path, eo)) = self.mask_path(c, t * local(c))
                    && self.push(Op::PushClip { path, even_odd: eo })
                {
                    masked = true;
                }
                continue;
            }
            self.layer(c, t, depth, ov);
        }
        if masked {
            self.push(Op::Pop);
        }
    }

    fn mask_path(&self, v: &Value, t: Affine) -> Option<(BezPath, bool)> {
        match class_of(v) {
            "shapeGroup" => self.shape_group_path(v, t),
            "group" => {
                let mut all = BezPath::new();
                for c in layers_of(v) {
                    if let Some((p, _)) = self.mask_path(c, t * local(c)) {
                        all.extend(p);
                    }
                }
                (!all.elements().is_empty()).then_some((all, false))
            }
            _ => {
                let mut p = shape_path(v)?;
                p.apply_affine(t);
                Some((p, winding(v)))
            }
        }
    }

    fn layer(&mut self, v: &'a Value, parent: Affine, depth: usize, ov: &Overrides) {
        if !visible(v) {
            return;
        }
        if depth > MAX_DEPTH
            || self.layers >= MAX_LAYERS
            || self.path_elements >= specular_drawpage::MAX_PATH_ELEMENTS
        {
            self.facts.limited = true;
            return;
        }
        self.layers += 1;
        let class = class_of(v);
        self.count(class);
        let t = parent * local(v);
        let id = v.get("do_objectID").and_then(Value::as_str).unwrap_or("");
        let shared = ov
            .style
            .get(id)
            .and_then(|sid| self.styles.get(sid.as_str()).copied());
        let style = shared.or_else(|| v.get("style"));
        let opacity = context_opacity(style);
        if opacity <= 0.0 {
            return;
        }
        let group = opacity < 1.0 && self.push(Op::PushOpacity(opacity as f32));
        if let Some(s) = style {
            self.effects(s);
        }
        match class {
            "group" | "artboard" | "symbolMaster" => self.children(layers_of(v), t, depth + 1, ov),
            "shapeGroup" => {
                if let Some((path, eo)) = self.shape_group_path(v, t) {
                    self.styled(&path, eo, true, style, t, frame(v));
                }
            }
            "rectangle" | "oval" | "triangle" | "star" | "polygon" | "shapePath" => {
                if let Some(mut p) = shape_path(v) {
                    p.apply_affine(t);
                    let closed = class != "shapePath" || closed_of(v);
                    self.styled(&p, winding(v), closed, style, t, frame(v));
                }
            }
            "text" => self.text(v, t, ov, style, shared.is_some()),
            "bitmap" => self.bitmap(v, t),
            "symbolInstance" => self.symbol(v, t, depth, ov),
            "slice" | "MSImmutableHotspotLayer" | "hotspot" => {}
            other => self.not_drawn(other),
        }
        if group {
            self.push(Op::Pop);
        }
    }

    fn effects(&mut self, s: &Value) {
        let enabled = |k: &str| {
            s.get(k)
                .and_then(Value::as_array)
                .is_some_and(|a| a.iter().any(|x| flag(x, "isEnabled")))
        };
        if enabled("shadows") || enabled("innerShadows") {
            self.not_drawn("shadows");
        }
        if s.get("blur").is_some_and(|b| flag(b, "isEnabled")) {
            self.not_drawn("blurs");
        }
        let blend = s
            .get("contextSettings")
            .and_then(|c| c.get("blendMode"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if blend != 0 {
            self.not_drawn("blend modes");
        }
    }

    fn shape_group_path(&self, v: &Value, t: Affine) -> Option<(BezPath, bool)> {
        let mut all = BezPath::new();
        let mut eo = winding(v);
        for (i, c) in layers_of(v).iter().enumerate() {
            if !visible(c) {
                continue;
            }
            let ct = t * local(c);
            let p = match class_of(c) {
                "shapeGroup" => self.shape_group_path(c, ct).map(|(p, _)| p),
                _ => shape_path(c).map(|mut p| {
                    p.apply_affine(ct);
                    p
                }),
            };
            let Some(p) = p else { continue };
            let op = c
                .get("booleanOperation")
                .and_then(Value::as_i64)
                .unwrap_or(-1);
            if i > 0 && op >= 1 {
                eo = true;
            }
            all.extend(p);
        }
        (!all.elements().is_empty()).then_some((all, eo))
    }

    // ----- Style -----------------------------------------------------------------------

    fn paint(&mut self, f: &Value, t: Affine, fr: Rect) -> Option<Paint> {
        let a = context_opacity(Some(f));
        match f.get("fillType").and_then(Value::as_u64).unwrap_or(0) {
            0 => color(f.get("color")).map(|c| Paint::Solid(with_alpha(c, a))),
            1 => {
                let g = f.get("gradient")?;
                let stops: Vec<Stop> = g
                    .get("stops")
                    .and_then(Value::as_array)?
                    .iter()
                    .take(256)
                    .filter_map(|s| {
                        Some(Stop {
                            offset: num(s, "position").unwrap_or(0.0) as f32,
                            color: with_alpha(color(s.get("color"))?, a),
                        })
                    })
                    .collect();
                let mut stops = stops;
                stops.sort_by(|x, y| x.offset.total_cmp(&y.offset));
                let pt = |k: &str, d: Point| {
                    let p = g
                        .get(k)
                        .and_then(Value::as_str)
                        .and_then(point_str)
                        .unwrap_or(d);
                    Point::new(p.x * fr.width(), p.y * fr.height())
                };
                let from = pt("from", Point::new(0.5, 0.0));
                let to = pt("to", Point::new(0.5, 1.0));
                match g.get("gradientType").and_then(Value::as_u64).unwrap_or(0) {
                    1 => {
                        let d = to - from;
                        let r = d.hypot().max(1e-6);
                        let e = num(g, "elipseLength").filter(|e| *e > 0.0).unwrap_or(1.0);
                        let ratio = if fr.width() > 0.0 {
                            e * fr.height().max(1e-6) / fr.width().max(1e-6)
                        } else {
                            1.0
                        };
                        let transform = t
                            * Affine::translate(from.to_vec2())
                            * Affine::rotate(d.y.atan2(d.x))
                            * Affine::scale_non_uniform(r, r * ratio.max(1e-6));
                        Some(Paint::Radial { transform, stops })
                    }
                    k => {
                        if k == 2 {
                            self.not_drawn("angular gradients (drawn as linear)");
                        }
                        Some(Paint::Linear {
                            start: t * from,
                            end: t * to,
                            stops,
                        })
                    }
                }
            }
            4 => {
                let r = f.get("image")?.get("_ref").and_then(Value::as_str)?;
                let img = self.image(r)?;
                let (iw, ih) = (f64::from(img.width), f64::from(img.height));
                let (w, h) = (fr.width().max(1e-6), fr.height().max(1e-6));
                let kind = f
                    .get("patternFillType")
                    .and_then(Value::as_u64)
                    .unwrap_or(1);
                let (transform, repeat) = match kind {
                    0 => {
                        let s = num(f, "patternTileScale")
                            .filter(|s| *s > 0.0)
                            .unwrap_or(1.0);
                        (t * Affine::scale(s), true)
                    }
                    2 => (t * Affine::scale_non_uniform(w / iw, h / ih), false),
                    k => {
                        let s = if k == 3 {
                            (w / iw).min(h / ih)
                        } else {
                            (w / iw).max(h / ih)
                        };
                        (
                            t * Affine::translate(((w - iw * s) / 2.0, (h - ih * s) / 2.0))
                                * Affine::scale(s),
                            false,
                        )
                    }
                };
                Some(Paint::Image {
                    image: img,
                    transform,
                    repeat,
                })
            }
            _ => {
                self.not_drawn("noise fills");
                None
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn styled(
        &mut self,
        path: &BezPath,
        even_odd: bool,
        closed: bool,
        style: Option<&'a Value>,
        t: Affine,
        fr: Rect,
    ) {
        let Some(s) = style else { return };
        let fr = Rect::new(0.0, 0.0, fr.width(), fr.height());
        let eo = even_odd;
        if let Some(fills) = s.get("fills").and_then(Value::as_array) {
            for f in fills.iter().filter(|f| flag(f, "isEnabled")) {
                if let Some(paint) = self.paint(f, t, fr) {
                    self.push(Op::Fill {
                        path: path.clone(),
                        even_odd: eo,
                        paint,
                    });
                }
            }
        }
        let opts = s.get("borderOptions");
        let mut base = Stroke::new(1.0);
        if let Some(o) = opts {
            base.start_cap = match o.get("lineCapStyle").and_then(Value::as_u64) {
                Some(1) => Cap::Round,
                Some(2) => Cap::Square,
                _ => Cap::Butt,
            };
            base.end_cap = base.start_cap;
            base.join = match o.get("lineJoinStyle").and_then(Value::as_u64) {
                Some(1) => Join::Round,
                Some(2) => Join::Bevel,
                _ => Join::Miter,
            };
        }
        let dashes: Vec<f64> = opts
            .and_then(|o| o.get("dashPattern"))
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_f64)
                    .filter(|d| d.is_finite() && *d >= 0.0)
                    .take(64)
                    .collect()
            })
            .unwrap_or_default();
        // The stroke scales with the layer's transform (symbols scaled to their instance).
        let k = t.determinant().abs().sqrt();
        let k = if k.is_finite() && k > 0.0 { k } else { 1.0 };
        let Some(borders) = s.get("borders").and_then(Value::as_array) else {
            return;
        };
        for b in borders.iter().filter(|b| flag(b, "isEnabled")) {
            let th = num(b, "thickness").unwrap_or(1.0) * k;
            if th <= 0.0 {
                continue;
            }
            let Some(paint) = self.paint(b, t, fr) else {
                continue;
            };
            let pos = b.get("position").and_then(Value::as_u64).unwrap_or(0);
            let mut st = base.clone();
            if !dashes.is_empty() && dashes.iter().any(|d| *d > 0.0) {
                st = st.with_dashes(0.0, dashes.iter().map(|d| d * k));
            }
            match (pos, closed) {
                (1, true) => {
                    st.width = th * 2.0;
                    if self.push(Op::PushClip {
                        path: path.clone(),
                        even_odd: eo,
                    }) {
                        self.push(Op::Stroke {
                            path: path.clone(),
                            stroke: st,
                            paint,
                        });
                        self.push(Op::Pop);
                    }
                }
                (2, true) => {
                    st.width = th * 2.0;
                    let mut clip = path
                        .bounding_box()
                        .inflate(th * 4.0 + 1.0, th * 4.0 + 1.0)
                        .to_path(0.1);
                    clip.extend(path.iter());
                    if self.push(Op::PushClip {
                        path: clip,
                        even_odd: true,
                    }) {
                        self.push(Op::Stroke {
                            path: path.clone(),
                            stroke: st,
                            paint,
                        });
                        self.push(Op::Pop);
                    }
                }
                _ => {
                    st.width = th;
                    self.push(Op::Stroke {
                        path: path.clone(),
                        stroke: st,
                        paint,
                    });
                }
            }
        }
    }

    // ----- Bitmaps, text, symbols ---------------------------------------------------------

    fn bitmap(&mut self, v: &Value, t: Affine) {
        let Some(r) = v
            .get("image")
            .and_then(|i| i.get("_ref"))
            .and_then(Value::as_str)
        else {
            return;
        };
        let Some(img) = self.image(r) else { return };
        let f = frame(v);
        let transform = t * Affine::scale_non_uniform(
            f.width() / f64::from(img.width),
            f.height() / f64::from(img.height),
        );
        self.push(Op::Image {
            image: img,
            transform,
            opacity: 1.0,
        });
    }

    /// `style`: the layer's style, or a shared text style swapped in by an override
    /// (`restyled`), which then styles every character.
    fn text(
        &mut self,
        v: &Value,
        t: Affine,
        ov: &Overrides,
        style: Option<&Value>,
        restyled: bool,
    ) {
        let f = frame(v);
        let id = v.get("do_objectID").and_then(Value::as_str).unwrap_or("");
        let Some(attr) = v.get("attributedString") else {
            return;
        };
        let override_text = ov.text.get(id).cloned();
        let string: String = match &override_text {
            Some(s) => s.clone(),
            None => match attr.get("string").and_then(Value::as_str) {
                Some(s) => s.to_string(),
                None => {
                    if attr.get("archivedAttributedString").is_some() {
                        self.not_drawn("text in the old archived encoding");
                    }
                    return;
                }
            },
        };
        let default_attrs = style
            .and_then(|s| s.get("textStyle"))
            .and_then(|s| s.get("encodedAttributes"));
        let valign = match style
            .and_then(|s| s.get("textStyle"))
            .and_then(|s| s.get("verticalAlignment"))
            .and_then(Value::as_u64)
        {
            Some(1) => VAlign::Middle,
            Some(2) => VAlign::Bottom,
            _ => VAlign::Top,
        };
        // Character ranges (UTF-16 units) with their attributes.
        let mut ranges: Vec<(usize, usize, &Value)> = Vec::new();
        if !restyled && let Some(a) = attr.get("attributes").and_then(Value::as_array) {
            for r in a.iter().take(10_000) {
                let loc = r.get("location").and_then(Value::as_u64).unwrap_or(0) as usize;
                let len = r.get("length").and_then(Value::as_u64).unwrap_or(0) as usize;
                if let Some(at) = r.get("attributes") {
                    ranges.push((loc, loc.saturating_add(len), at));
                }
            }
            // Replaced text takes the first run's style throughout.
            if override_text.is_some() {
                ranges.truncate(1);
                if let Some(r) = ranges.first_mut() {
                    *r = (0, usize::MAX, r.2);
                }
            }
        }
        let empty = Value::Null;
        // (key, attributes): the range index, or usize::MAX for the defaults.
        let attrs_at = |u: usize| -> (usize, &Value) {
            if let Some(i) = ranges.iter().position(|(a, b, _)| u >= *a && u < *b) {
                return (i, ranges[i].2);
            }
            (
                usize::MAX,
                default_attrs
                    .or_else(|| ranges.first().map(|r| r.2))
                    .unwrap_or(&empty),
            )
        };
        // Fill colour on the layer overrides the runs' colour.
        let fill: Option<Rgba> = style
            .and_then(|s| s.get("fills"))
            .and_then(Value::as_array)
            .and_then(|a| a.iter().rev().find(|f| flag(f, "isEnabled")))
            .filter(|f| f.get("fillType").and_then(Value::as_u64).unwrap_or(0) == 0)
            .and_then(|f| color(f.get("color")));
        let mut paragraphs: Vec<Paragraph> = Vec::new();
        let mut para = Paragraph::default();
        let mut started = false;
        let mut unit = 0usize;
        let mut cur: Option<(usize, Run)> = None;
        let flush = |cur: &mut Option<(usize, Run)>, para: &mut Paragraph| {
            if let Some((_, r)) = cur.take() {
                para.runs.push(r);
            }
        };
        for ch in string.chars() {
            let (key, a) = attrs_at(unit);
            unit += ch.len_utf16();
            if !started {
                self.paragraph_style(a, &mut para);
                started = true;
            }
            if ch == '\n' || ch == '\u{2029}' {
                flush(&mut cur, &mut para);
                if para.runs.is_empty() {
                    let mut r = self.run_of(a, fill);
                    r.text.clear();
                    para.runs.push(r);
                }
                paragraphs.push(std::mem::take(&mut para));
                started = false;
                continue;
            }
            match &mut cur {
                Some((k, r)) if *k == key => r.text.push(ch),
                _ => {
                    flush(&mut cur, &mut para);
                    let mut r = self.run_of(a, fill);
                    r.text.push(ch);
                    cur = Some((key, r));
                }
            }
        }
        flush(&mut cur, &mut para);
        if !para.runs.is_empty() {
            paragraphs.push(para);
        }
        if paragraphs.is_empty() {
            return;
        }
        let behaviour = v.get("textBehaviour").and_then(Value::as_u64).unwrap_or(0);
        let mut b = TextBlock::new(t, f.width(), f.height());
        // Auto-width text is laid out on one line per paragraph, at its frame; a little
        // slack keeps rounding from wrapping fixed boxes.
        b.wrap = behaviour != 0;
        if b.wrap {
            b.width += 0.5;
        }
        b.valign = valign;
        b.paragraphs = paragraphs;
        self.push(Op::Text(Box::new(b)));
    }

    fn paragraph_style(&mut self, a: &Value, p: &mut Paragraph) {
        let Some(ps) = a.get("paragraphStyle") else {
            return;
        };
        p.align = match ps.get("alignment").and_then(Value::as_u64) {
            Some(1) => HAlign::Right,
            Some(2) => HAlign::Centre,
            Some(3) => HAlign::Justify,
            _ => HAlign::Left,
        };
        let max = num(ps, "maximumLineHeight").filter(|x| *x > 0.0);
        let min = num(ps, "minimumLineHeight").filter(|x| *x > 0.0);
        p.line_height = max.or(min);
        p.space_after = num(ps, "paragraphSpacing").unwrap_or(0.0).max(0.0);
    }

    fn run_of(&mut self, a: &Value, fill: Option<Rgba>) -> Run {
        let font = a
            .get("MSAttributedStringFontAttribute")
            .and_then(|f| f.get("attributes"));
        let ps_name = font
            .and_then(|f| f.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("Helvetica");
        let size = font
            .and_then(|f| num(f, "size"))
            .filter(|s| *s > 0.0)
            .unwrap_or(12.0);
        if !self.facts.fonts.iter().any(|f| f == ps_name) && self.facts.fonts.len() < 256 {
            self.facts.fonts.push(ps_name.to_string());
        }
        let (family, bold, italic) = family_of(ps_name);
        let mut r = Run::new(String::new(), family, size);
        r.bold = bold;
        r.italic = italic;
        r.color = color(a.get("MSAttributedStringColorAttribute"))
            .or(fill)
            .unwrap_or([0, 0, 0, 255]);
        r.letter_spacing = num(a, "kerning").unwrap_or(0.0);
        r.underline = a.get("underlineStyle").and_then(Value::as_u64).unwrap_or(0) != 0;
        r.strike = a
            .get("strikethroughStyle")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            != 0;
        r
    }

    fn symbol(&mut self, v: &'a Value, t: Affine, depth: usize, ov: &Overrides) {
        let id = v.get("do_objectID").and_then(Value::as_str).unwrap_or("");
        let sid = ov.symbol.get(id).cloned().or_else(|| {
            v.get("symbolID")
                .and_then(Value::as_str)
                .map(str::to_string)
        });
        let Some(sid) = sid else { return };
        let Some(master) = self.symbols.get(sid.as_str()).copied() else {
            self.not_drawn("symbols from missing libraries");
            return;
        };
        if self.symbol_stack.len() >= MAX_SYMBOL_DEPTH || self.symbol_stack.contains(&sid) {
            self.facts.limited |= self.symbol_stack.len() >= MAX_SYMBOL_DEPTH;
            return;
        }
        let fi = frame(v);
        let fm = frame(master);
        let sx = if fm.width() > 0.0 {
            fi.width() / fm.width()
        } else {
            1.0
        };
        let sy = if fm.height() > 0.0 {
            fi.height() / fm.height()
        } else {
            1.0
        };
        // Overrides: ours (from an outer instance) win over this instance's own.
        let mut inner = Overrides::default();
        if let Some(a) = v.get("overrideValues").and_then(Value::as_array) {
            for o in a.iter().take(4096) {
                let (Some(name), Some(val)) = (
                    o.get("overrideName").and_then(Value::as_str),
                    o.get("value").and_then(Value::as_str),
                ) else {
                    continue;
                };
                let Some((path, prop)) = name.rsplit_once('_') else {
                    continue;
                };
                let target = path.rsplit('/').next().unwrap_or(path).to_string();
                match prop {
                    "stringValue" => {
                        inner.text.insert(target, val.to_string());
                    }
                    "layerStyle" | "textStyle" => {
                        inner.style.insert(target, val.to_string());
                    }
                    "symbolID" => {
                        inner.symbol.insert(target, val.to_string());
                    }
                    _ => {}
                }
            }
        }
        for (k, val) in &ov.text {
            inner.text.insert(k.clone(), val.clone());
        }
        for (k, val) in &ov.style {
            inner.style.insert(k.clone(), val.clone());
        }
        for (k, val) in &ov.symbol {
            inner.symbol.insert(k.clone(), val.clone());
        }
        self.symbol_stack.push(sid);
        let mt = t * Affine::scale_non_uniform(sx, sy);
        if flag(master, "hasBackgroundColor")
            && flag(master, "includeBackgroundColorInInstance")
            && let Some(c) = color(master.get("backgroundColor"))
        {
            self.push(Op::Fill {
                path: (mt * Rect::new(0.0, 0.0, fm.width(), fm.height()).to_path(0.1)),
                even_odd: false,
                paint: Paint::Solid(c),
            });
        }
        self.children(layers_of(master), mt, depth + 1, &inner);
        self.symbol_stack.pop();
    }
}

/// Text and symbol overrides by target layer id.
#[derive(Default)]
struct Overrides {
    text: HashMap<String, String>,
    symbol: HashMap<String, String>,
    /// Shared style ids (layer and text styles).
    style: HashMap<String, String>,
}

/// "Helvetica-BoldOblique" → ("Helvetica", bold, italic); "OpenSans-Regular" → "Open Sans".
fn family_of(ps: &str) -> (String, bool, bool) {
    let (fam, style) = ps.split_once('-').unwrap_or((ps, ""));
    let fam = fam
        .strip_suffix("PSMT")
        .or_else(|| fam.strip_suffix("MT"))
        .unwrap_or(fam);
    let lower = format!("{} {}", style.to_ascii_lowercase(), ps.to_ascii_lowercase());
    let bold = ["bold", "black", "heavy", "semibold", "demi"]
        .iter()
        .any(|k| style.to_ascii_lowercase().contains(k))
        || (style.is_empty() && lower.contains("bold"));
    let italic = lower.contains("italic") || lower.contains("oblique");
    // Spaces at lower→upper transitions: "OpenSans" → "Open Sans", "SFProText" → "SFPro Text".
    let mut out = String::with_capacity(fam.len() + 4);
    let mut prev: Option<char> = None;
    for c in fam.chars() {
        if let Some(p) = prev
            && p.is_lowercase()
            && c.is_uppercase()
        {
            out.push(' ');
        }
        out.push(c);
        prev = Some(c);
    }
    let out = match out.as_str() {
        ".SFNS Text" | ".SFNSText" | ".SFNS Display" | "SFPro Text" | "SFPro Display" => {
            "SF Pro".to_string()
        }
        _ => out,
    };
    (out, bold, italic)
}

fn winding(v: &Value) -> bool {
    v.get("windingRule").and_then(Value::as_u64) == Some(1)
        || v.get("style")
            .and_then(|s| s.get("windingRule"))
            .and_then(Value::as_u64)
            == Some(1)
}

fn closed_of(v: &Value) -> bool {
    v.get("isClosed")
        .or_else(|| v.get("path").and_then(|p| p.get("isClosed")))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// A shape's outline in its own frame space (0..width, 0..height).
fn shape_path(v: &Value) -> Option<BezPath> {
    let f = frame(v);
    let (w, h) = (f.width(), f.height());
    match class_of(v) {
        "oval" => Some(Ellipse::from_rect(Rect::new(0.0, 0.0, w, h)).to_path(0.05)),
        "rectangle" => {
            let pts = points_of(v);
            let mut radii: [f64; 4] = [0.0; 4];
            if pts.len() == 4 {
                for (i, p) in pts.iter().enumerate() {
                    radii[i] = num(p, "cornerRadius").unwrap_or(0.0).max(0.0);
                }
            }
            if radii.iter().all(|r| *r == 0.0) {
                let r = num(v, "fixedRadius").unwrap_or(0.0).max(0.0);
                radii = [r; 4];
            }
            let lim = w.min(h) / 2.0;
            let radii = radii.map(|r| r.min(lim));
            if radii.iter().all(|r| *r == 0.0) {
                return Some(Rect::new(0.0, 0.0, w, h).to_path(0.1));
            }
            Some(
                RoundedRect::from_rect(
                    Rect::new(0.0, 0.0, w, h),
                    kurbo::RoundedRectRadii::new(radii[0], radii[1], radii[2], radii[3]),
                )
                .to_path(0.05),
            )
        }
        _ => {
            let pts = points_of(v);
            if pts.len() < 2 {
                return None;
            }
            let at = |p: &Value, k: &str| -> Option<Point> {
                let q = p.get(k).and_then(Value::as_str).and_then(point_str)?;
                Some(Point::new(q.x * w, q.y * h))
            };
            let mut path = BezPath::new();
            let first = at(&pts[0], "point")?;
            path.move_to(first);
            let n = pts.len();
            let closed = closed_of(v);
            let segs = if closed { n } else { n - 1 };
            for i in 0..segs {
                let a = &pts[i];
                let b = &pts[(i + 1) % n];
                let pa = at(a, "point")?;
                let pb = at(b, "point")?;
                let c1 = curve(a, "curveFrom", "hasCurveFrom", &at).unwrap_or(pa);
                let c2 = curve(b, "curveTo", "hasCurveTo", &at).unwrap_or(pb);
                if c1 == pa && c2 == pb {
                    path.line_to(pb);
                } else {
                    path.curve_to(c1, c2, pb);
                }
            }
            if closed {
                path.close_path();
            }
            Some(path)
        }
    }
}

fn curve(
    p: &Value,
    k: &str,
    has: &str,
    at: &dyn Fn(&Value, &str) -> Option<Point>,
) -> Option<Point> {
    if p.get(has).and_then(Value::as_bool) == Some(false) {
        return None;
    }
    // Straight points (curveMode 1) carry stale control points in old files.
    if p.get("curveMode").and_then(Value::as_u64) == Some(1)
        && p.get(has).and_then(Value::as_bool).is_none()
    {
        return None;
    }
    at(p, k)
}

fn points_of(v: &Value) -> &[Value] {
    v.get("points")
        .or_else(|| v.get("path").and_then(|p| p.get("points")))
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn collect_symbols<'a>(v: &'a Value, out: &mut HashMap<String, &'a Value>, depth: usize) {
    if depth > MAX_DEPTH || out.len() > 1 << 20 {
        return;
    }
    for l in layers_of(v) {
        if class_of(l) == "symbolMaster" {
            if let Some(id) = l.get("symbolID").and_then(Value::as_str) {
                out.entry(id.to_string()).or_insert(l);
            }
        } else if matches!(class_of(l), "group" | "artboard") {
            collect_symbols(l, out, depth + 1);
        }
    }
}

fn symbol_map(p: &Parsed) -> HashMap<String, &Value> {
    let mut symbols = HashMap::new();
    for page in &p.pages {
        collect_symbols(page, &mut symbols, 0);
    }
    if let Some(fs) = p.doc.get("foreignSymbols").and_then(Value::as_array) {
        for s in fs {
            if let Some(m) = s.get("symbolMaster")
                && let Some(id) = m.get("symbolID").and_then(Value::as_str)
            {
                symbols.entry(id.to_string()).or_insert(m);
            }
        }
    }
    symbols
}

fn style_map<'d>(doc: &'d Value) -> HashMap<String, &'d Value> {
    let mut out = HashMap::new();
    let mut add = |s: &'d Value| {
        let (Some(id), Some(v)) = (s.get("do_objectID").and_then(Value::as_str), s.get("value"))
        else {
            return;
        };
        if out.len() < 1 << 20 {
            out.entry(id.to_string()).or_insert(v);
        }
    };
    for k in ["layerStyles", "layerTextStyles"] {
        if let Some(a) = doc
            .get(k)
            .and_then(|c| c.get("objects"))
            .and_then(Value::as_array)
        {
            a.iter().for_each(&mut add);
        }
    }
    for k in ["foreignLayerStyles", "foreignTextStyles"] {
        if let Some(a) = doc.get(k).and_then(Value::as_array) {
            for f in a {
                if let Some(l) = f.get("localSharedStyle") {
                    add(l);
                }
            }
        }
    }
    out
}

fn open_parts(data: &[u8]) -> Result<(Zip, Parsed), Error> {
    let zip = zip_of(data, Error::NotSketch)?;
    if zip.find("document.json").is_none() {
        return Err(Error::NotSketch);
    }
    let parsed = parse(data, &zip)?;
    Ok((zip, parsed))
}

fn new_ctx<'a>(
    data: &'a [u8],
    zip: &'a Zip,
    p: &'a Parsed,
    decode: Option<ImageDecoder>,
) -> Ctx<'a> {
    Ctx {
        data,
        zip,
        decode,
        symbols: symbol_map(p),
        styles: style_map(&p.doc),
        images: HashMap::new(),
        image_pixels: 0,
        layers: 0,
        facts: facts_of(p.meta.as_ref(), &p.pages),
        ops: Vec::new(),
        path_elements: 0,
        symbol_stack: Vec::new(),
    }
}

fn artboard_background(a: &Value) -> Rgba {
    if flag(a, "hasBackgroundColor") {
        color(a.get("backgroundColor"))
            .map(|c| with_alpha(c, 1.0))
            .filter(|c| c[3] > 0)
            .map(|c| {
                // Composited on white: pages are opaque.
                let k = f64::from(c[3]) / 255.0;
                let m = |x: u8| (f64::from(x) * k + 255.0 * (1.0 - k)).round() as u8;
                [m(c[0]), m(c[1]), m(c[2]), 255]
            })
            .unwrap_or([255, 255, 255, 255])
    } else {
        [255, 255, 255, 255]
    }
}

/// Draws every artboard and symbol master of a Sketch document as a page, then the layers
/// of each Sketch page outside artboards as one page sized to their extent.
pub fn open_sketch(data: &[u8], decode: Option<ImageDecoder>) -> Result<SketchDocument, Error> {
    let (zip, parsed) = open_parts(data)?;
    let masters = parsed
        .pages
        .iter()
        .flat_map(layers_of)
        .filter(|l| class_of(l) == "symbolMaster")
        .count();
    // Symbol masters are pages of their own only in a pure symbol library.
    let mut doc = open_pages(data, &zip, &parsed, decode.clone(), false);
    if doc.pages.is_empty() && masters > 0 {
        doc = open_pages(data, &zip, &parsed, decode, true);
    }
    doc.facts.symbol_masters = masters;
    if doc.pages.is_empty() {
        return Err(Error::Unsupported(
            "Sketch document with nothing to draw (empty pages)".into(),
        ));
    }
    Ok(doc)
}

fn open_pages(
    data: &[u8],
    zip: &Zip,
    parsed: &Parsed,
    decode: Option<ImageDecoder>,
    with_symbols: bool,
) -> SketchDocument {
    let mut cx = new_ctx(data, zip, parsed, decode);
    let none = Overrides::default();
    let mut pages = Vec::new();
    for sp in &parsed.pages {
        let pname = name_of(sp).to_string();
        let mut loose: Vec<&Value> = Vec::new();
        for l in layers_of(sp) {
            if !matches!(class_of(l), "artboard" | "symbolMaster") {
                loose.push(l);
                continue;
            }
            if class_of(l) == "symbolMaster" && !with_symbols {
                continue;
            }
            if !visible(l) || pages.len() >= MAX_PAGES {
                continue;
            }
            let f = frame(l);
            if f.width() <= 0.0 || f.height() <= 0.0 {
                continue;
            }
            cx.count(class_of(l));
            cx.ops = Vec::new();
            cx.children(layers_of(l), Affine::IDENTITY, 1, &none);
            let aname = name_of(l).to_string();
            let mut page = Page::new(format!("{pname} / {aname}"), (f.width(), f.height()));
            page.background = artboard_background(l);
            page.ops = std::mem::take(&mut cx.ops);
            cx.facts.page_labels.push(page.name.clone());
            cx.facts.artboards.push(Some(aname));
            pages.push(page);
        }
        if loose.is_empty() || pages.len() >= MAX_PAGES {
            continue;
        }
        cx.ops = Vec::new();
        // The extent: what is drawn, and the visible layers' frames (a bitmap whose image is
        // missing still has its place).
        let mut frames: Option<Rect> = None;
        for l in loose {
            cx.layer(l, Affine::IDENTITY, 1, &none);
            if visible(l) {
                let f = local(l).transform_rect_bbox(Rect::new(
                    0.0,
                    0.0,
                    frame(l).width(),
                    frame(l).height(),
                ));
                if f.width() > 0.0 && f.height() > 0.0 {
                    frames = Some(frames.map_or(f, |r| r.union(f)));
                }
            }
        }
        let mut ops = std::mem::take(&mut cx.ops);
        let bounds = match (specular_drawpage::content_bounds(&ops), frames) {
            (Some(a), Some(b)) => Some(a.union(b)),
            (a, b) => a.or(b),
        };
        if let Some(b) = bounds
            && b.width() > 0.0
            && b.height() > 0.0
        {
            specular_drawpage::translate_ops(&mut ops, Vec2::new(-b.x0, -b.y0));
            let mut page = Page::new(pname.clone(), (b.width(), b.height()));
            page.ops = ops;
            cx.facts.page_labels.push(pname);
            cx.facts.artboards.push(None);
            pages.push(page);
        }
    }
    SketchDocument {
        pages,
        facts: cx.facts,
    }
}

/// Draws each Sketch page whole (artboards at their places, with their backgrounds, and the
/// layers between them), sized to its extent: what Sketch's preview shows. Also returns the
/// index of the page the document was last viewed on.
pub fn open_sketch_overview(
    data: &[u8],
    decode: Option<ImageDecoder>,
) -> Result<(Vec<Page>, usize), Error> {
    let (zip, parsed) = open_parts(data)?;
    let mut cx = new_ctx(data, &zip, &parsed, decode);
    let none = Overrides::default();
    let mut pages = Vec::new();
    for sp in &parsed.pages {
        cx.ops = Vec::new();
        for l in layers_of(sp) {
            if matches!(class_of(l), "artboard" | "symbolMaster") {
                if !visible(l) {
                    continue;
                }
                let f = frame(l);
                let r = f.to_path(0.1);
                cx.push(Op::Fill {
                    path: r.clone(),
                    even_odd: false,
                    paint: Paint::Solid(artboard_background(l)),
                });
                cx.push(Op::PushClip {
                    path: r,
                    even_odd: false,
                });
                cx.children(layers_of(l), Affine::translate((f.x0, f.y0)), 1, &none);
                cx.push(Op::Pop);
            } else {
                cx.layer(l, Affine::IDENTITY, 1, &none);
            }
        }
        let mut ops = std::mem::take(&mut cx.ops);
        let b = specular_drawpage::content_bounds(&ops).unwrap_or(Rect::new(0.0, 0.0, 1.0, 1.0));
        specular_drawpage::translate_ops(&mut ops, Vec2::new(-b.x0, -b.y0));
        let mut page = Page::new(name_of(sp), (b.width().max(1.0), b.height().max(1.0)));
        page.background = [255, 255, 255, 255];
        page.ops = ops;
        pages.push(page);
    }
    let current = parsed
        .doc
        .get("currentPageIndex")
        .and_then(Value::as_u64)
        .map_or(0, |i| i as usize)
        .min(pages.len().saturating_sub(1));
    Ok((pages, current))
}

#[cfg(test)]
pub(crate) fn family_of_for_tests(ps: &str) -> (String, bool, bool) {
    family_of(ps)
}
