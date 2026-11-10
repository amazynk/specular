//! Lays out a figure: an optional title, then a row of panels (the split channels, the main
//! image, an inset), a colour bar with ticks and units at the end of the row, and the drawn
//! provenance under it. The scale bar, measurements and the inset's outline go on the main
//! panel. Sizes follow the main panel (a 512 px CT gets 18 px text, a 4000 px micrograph
//! 72 px), so the figure looks the same at any resolution.
//!
//! Pictures land at whole-pixel offsets at their own size: the main panel's pixels are copied
//! into every output as they are.

use crate::nice::{self, BarLength};
use crate::text::Shaper;
use crate::{Figure, Item, Path, Picture, Rgb16, Rgba, Role, rect_path};

/// An image panel and the label over it.
#[derive(Clone, Debug, PartialEq)]
pub struct Panel {
    pub px: Rgb16,
    pub label: Option<String>,
}

/// The main panel's calibration along its horizontal: `per_px` units of `unit` per panel px.
#[derive(Clone, Debug, PartialEq)]
pub struct ScaleSpec {
    pub per_px: f64,
    /// The unit's symbol (`mm`, `µm`, `″`).
    pub unit: String,
    /// Metres per unit, for lengths (the bar then picks the SI unit that reads best).
    pub metres: Option<f64>,
    /// The calibration is an estimate (the label gets `≈`).
    pub approximate: bool,
}

/// A colour bar: the ramp from `lo` (first entry) to `hi` (last), in `unit`.
#[derive(Clone, Debug, PartialEq)]
pub struct LegendSpec {
    pub ramp: Vec<[u16; 3]>,
    pub lo: f64,
    pub hi: f64,
    pub unit: String,
}

/// A zoomed region of the main panel, shown beside it.
#[derive(Clone, Debug, PartialEq)]
pub struct InsetSpec {
    /// Rendered at [`inset_size`].
    pub px: Rgb16,
    /// The region on the main panel: x, y, w, h in panel px.
    pub region: [f32; 4],
}

/// A measurement or region on the main panel (panel px) and its value.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub path: Path,
    pub closed: bool,
    pub label: Option<String>,
}

/// What a figure shows.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Spec {
    pub title: Option<String>,
    pub main: Option<Panel>,
    /// Split channels, shown before the main panel (which is then the merge).
    pub channels: Vec<Panel>,
    pub inset: Option<InsetSpec>,
    pub scale: Option<ScaleSpec>,
    pub legend: Option<LegendSpec>,
    pub marks: Vec<Mark>,
    /// Provenance and display settings, always embedded as metadata.
    pub meta: Vec<(String, String)>,
    /// Lines drawn under the panels (the provenance block, when chosen).
    pub caption: Vec<String>,
    pub deep: bool,
}

/// The main panel's text size (px) and the spacing derived from it.
pub fn unit(main: (u32, u32)) -> f32 {
    (main.0.min(main.1) as f32 / 28.0).clamp(12.0, 72.0).round()
}

/// The size an inset of `region` (panel px) is shown at beside a `main` panel: as tall as the
/// panel, at most as wide.
pub fn inset_size(main: (u32, u32), region: [f32; 4]) -> (u32, u32) {
    let (rw, rh) = (region[2].max(1.0), region[3].max(1.0));
    let h = main.1.max(1);
    let w = ((h as f32 * rw / rh).round() as u32).max(1);
    if w <= main.0 {
        (w, h)
    } else {
        let w = main.0.max(1);
        (w, ((w as f32 * rh / rw).round() as u32).clamp(1, h))
    }
}

const MARK: Rgba = Rgba::rgb(255, 214, 0);
const INSET: Rgba = Rgba::rgb(0, 229, 255);
const INK: Rgba = Rgba::rgb(20, 20, 20);
const MUTED: Rgba = Rgba::rgb(70, 70, 70);

/// Text over an image: light letters with a dark halo, so they read on any pixels.
fn halo_text(items: &mut Vec<Item>, path: Path, text: &str, s: f32) {
    items.push(Item::stroke(
        path.clone(),
        Rgba::BLACK.with_alpha(0.85),
        s * 0.16,
        Role::Plain,
    ));
    let mut it = Item::fill(path, Rgba::WHITE, Role::Text);
    it.text = Some(text.to_owned());
    items.push(it);
}

fn text_item(path: Path, text: &str, colour: Rgba) -> Item {
    let mut it = Item::fill(path, colour, Role::Text);
    it.text = Some(text.to_owned());
    it
}

/// The label of a scale bar: `50 µm`, `≈ 2 cm` when estimated.
pub fn bar_label(b: &BarLength, approximate: bool) -> String {
    let v = format!("{} {}", nice::label(b.value), b.unit);
    if approximate { format!("≈ {v}") } else { v }
}

/// Lays out `spec`; `None` without a main panel.
pub fn compose(spec: &Spec, shaper: &mut dyn Shaper) -> Option<Figure> {
    let main = spec.main.as_ref()?;
    let (w0, h0) = (main.px.width, main.px.height);
    let s = unit((w0, h0));
    let gap = s.round();
    let margin = s.round();
    let mut items = Vec::new();
    let mut pictures = Vec::new();
    let mut y = margin;
    let mut right = margin;
    if let Some(t) = spec.title.as_deref().filter(|t| !t.trim().is_empty()) {
        let sh = shaper.shape(t, s * 1.25, true);
        items.push(text_item(sh.at(margin, y), t, INK));
        right = right.max(margin + sh.width);
        y += (sh.height + gap * 0.5).round();
    }
    // The panels: channels (same size as the main panel), then the main panel.
    let panels: Vec<&Panel> = spec
        .channels
        .iter()
        .filter(|p| (p.px.width, p.px.height) == (w0, h0))
        .chain([main])
        .collect();
    let label_size = s * 0.9;
    let label_h = if panels.iter().any(|p| p.label.is_some()) {
        (label_size * 1.45).round()
    } else {
        0.0
    };
    let top = y + label_h;
    let mut x = margin;
    let mut main_at = (x, top);
    for (i, p) in panels.iter().enumerate() {
        if let Some(l) = &p.label {
            let sh = shaper.shape(l, label_size, true);
            items.push(text_item(sh.at(x, y), l, INK));
        }
        pictures.push(Picture {
            x,
            y: top,
            w: w0 as f32,
            h: h0 as f32,
            px: p.px.clone(),
            smooth: false,
            name: p.label.clone().unwrap_or_default(),
        });
        if i + 1 == panels.len() {
            main_at = (x, top);
        }
        x += w0 as f32 + gap;
    }
    let (mx, my) = main_at;
    let mut bottom = top + h0 as f32;

    // Measurements and regions on the main panel.
    let shift = |p: &Path| crate::text::translate(p, mx, my);
    for m in &spec.marks {
        let p = shift(&m.path);
        items.push(Item::stroke(
            p.clone(),
            Rgba::BLACK.with_alpha(0.7),
            s * 0.3,
            Role::Plain,
        ));
        let mut it = Item::stroke(p.clone(), MARK, s * 0.14, Role::Measurement);
        if let Some(l) = &m.label {
            it.data.push(("value", l.clone()));
        }
        items.push(it);
        if let (Some(l), Some(b)) = (&m.label, crate::bounds(&p)) {
            let sh = shaper.shape(l, s * 0.75, false);
            // Beside the shape's end, kept inside the panel.
            let end = last_point(&p).unwrap_or([b[2], b[1]]);
            let lx = (end[0] + s * 0.4)
                .min(mx + w0 as f32 - sh.width - 2.0)
                .max(mx + 2.0);
            let ly = (end[1] - sh.height * 0.5)
                .min(my + h0 as f32 - sh.height - 2.0)
                .max(my + 2.0);
            halo_text(&mut items, sh.at(lx, ly), l, s * 0.75);
        }
    }

    // The inset: its region outlined on the main panel, the zoomed copy beside it.
    if let Some(inset) = &spec.inset {
        let [rx, ry, rw, rh] = inset.region;
        let lw = (s * 0.14).max(1.0);
        items.push(Item::stroke(
            rect_path(mx + rx, my + ry, rw, rh),
            INSET,
            lw,
            Role::InsetOutline,
        ));
        let (iw, ih) = (inset.px.width as f32, inset.px.height as f32);
        pictures.push(Picture {
            x,
            y: top,
            w: iw,
            h: ih,
            px: inset.px.clone(),
            smooth: false,
            name: "inset".into(),
        });
        items.push(Item::stroke(
            rect_path(x + lw * 0.5, top + lw * 0.5, iw - lw, ih - lw),
            INSET,
            lw,
            Role::InsetOutline,
        ));
        x += iw + gap;
        bottom = bottom.max(top + ih);
    }

    // The scale bar, bottom right of the main panel.
    if let Some(sc) = &spec.scale
        && let Some(b) = nice::bar(sc.per_px, &sc.unit, sc.metres, f64::from(w0), 0.2)
    {
        let t = (s * 0.35).max(2.0);
        let len = b.px as f32;
        let x1 = mx + w0 as f32 - s;
        let y1 = my + h0 as f32 - s;
        let bar = rect_path(x1 - len, y1 - t, len, t);
        items.push(Item::stroke(
            bar.clone(),
            Rgba::BLACK.with_alpha(0.85),
            s * 0.16,
            Role::Plain,
        ));
        items.push(
            Item::fill(bar, Rgba::WHITE, Role::ScaleBar)
                .with_data("length", nice::label(b.value))
                .with_data("unit", b.unit.clone())
                .with_data("length-px", format!("{}", b.px)),
        );
        let label = bar_label(&b, sc.approximate);
        let sh = shaper.shape(&label, s * 0.8, true);
        halo_text(
            &mut items,
            sh.at(
                x1 - len * 0.5 - sh.width * 0.5,
                y1 - t - s * 0.25 - sh.height,
            ),
            &label,
            s * 0.8,
        );
    }

    // The colour bar at the end of the row.
    if let Some(lg) = &spec.legend
        && lg.ramp.len() >= 2
    {
        let bw = (s * 1.1).round();
        let bh = (h0 as f32 * 0.8).max(s * 6.0).round();
        let unit_sh = (!lg.unit.is_empty()).then(|| shaper.shape(&lg.unit, s * 0.8, false));
        let unit_h = unit_sh.as_ref().map_or(0.0, |u| u.height + s * 0.3);
        let by = (top + (h0 as f32 - bh) * 0.5).max(top + unit_h).round();
        let n = lg.ramp.len() as u32;
        // Top row is the high end.
        let data: Vec<u16> = lg.ramp.iter().rev().flat_map(|c| *c).collect();
        pictures.push(Picture {
            x,
            y: by,
            w: bw,
            h: bh,
            px: Rgb16 {
                width: 1,
                height: n,
                data,
            },
            smooth: true,
            name: "colour bar".into(),
        });
        let frame_w = (s * 0.06).max(1.0);
        items.push(
            Item::stroke(rect_path(x, by, bw, bh), INK, frame_w, Role::ColourBar)
                .with_data("lo", nice::label(lg.lo))
                .with_data("hi", nice::label(lg.hi))
                .with_data("unit", lg.unit.clone()),
        );
        if let Some(u) = &unit_sh {
            items.push(text_item(u.at(x, by - unit_h), &lg.unit, INK));
        }
        let mut label_w: f32 = 0.0;
        let span = lg.hi - lg.lo;
        for v in nice::ticks(lg.lo, lg.hi, 6) {
            let k = if span == 0.0 {
                0.5
            } else {
                ((v - lg.lo) / span) as f32
            };
            let ty = by + bh * (1.0 - k);
            let tick = vec![
                crate::El::Move([x + bw, ty]),
                crate::El::Line([x + bw + s * 0.35, ty]),
            ];
            items.push(
                Item::stroke(tick, INK, frame_w, Role::Tick).with_data("value", nice::label(v)),
            );
            let l = nice::label(v);
            let sh = shaper.shape(&l, s * 0.75, false);
            label_w = label_w.max(sh.width);
            let mut it = text_item(sh.at(x + bw + s * 0.5, ty - sh.height * 0.5), &l, INK);
            it.data.push(("value", l.clone()));
            items.push(it);
        }
        x += bw + s * 0.5 + label_w + gap;
        bottom = bottom.max(by + bh + s * 0.5);
    }
    right = right.max(x - gap + margin);

    // The provenance block.
    let mut cy = bottom + gap;
    for line in spec.caption.iter().filter(|l| !l.is_empty()) {
        let sh = shaper.shape(line, s * 0.6, false);
        items.push(text_item(sh.at(margin, cy), line, MUTED));
        right = right.max(margin + sh.width + margin);
        cy += sh.height;
    }
    let height = if spec.caption.is_empty() {
        bottom + margin
    } else {
        cy + margin
    };
    Some(Figure {
        width: right.ceil() as u32,
        height: height.ceil() as u32,
        background: Rgba::WHITE,
        pictures,
        items,
        meta: spec.meta.clone(),
        title: spec.title.clone().unwrap_or_default(),
        deep: spec.deep,
    })
}

fn last_point(p: &[crate::El]) -> Option<[f32; 2]> {
    p.iter().rev().find_map(|e| match *e {
        crate::El::Move(a) | crate::El::Line(a) => Some(a),
        crate::El::Quad(_, a) | crate::El::Cubic(_, _, a) => Some(a),
        crate::El::Close => None,
    })
}
