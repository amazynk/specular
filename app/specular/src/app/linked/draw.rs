//! What linked views draw, in logical pixels: the bar of settings and the series strip above
//! the viewports, the grid of viewports (each slice as a texture with its readout,
//! orientation letters, reference lines, the 3D cursor and a gap/outside flag) and the series
//! being dragged; with their accessibility nodes.

use specular_gfx::{Align, DrawList, Rect, TexId, Text, TextStyle};
use specular_i18n::t;
use specular_theme::metrics::{FONT_BODY, GAP, PAD, RADIUS_MD, RADIUS_SM};
use specular_theme::{Color, Palette};
use specular_widgets::accesskit;

use super::super::medical::labels::{self, Edge};

/// A rectangle, logical pixels: x, y, width, height.
pub(crate) type R = [f64; 4];

/// AccessKit ids (MPR uses 0x5900…, medical 0x5800…).
const A11Y_BASE: u64 = 0x6300_0000_0000_0000;

/// Columns and rows of viewports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Grid {
    pub cols: u8,
    pub rows: u8,
}

impl Grid {
    pub(crate) const fn new(cols: u8, rows: u8) -> Grid {
        Grid { cols, rows }
    }

    pub(crate) fn count(self) -> usize {
        usize::from(self.cols) * usize::from(self.rows)
    }
}

/// The layouts `Alt+J` steps through: the common ones first, then the rest up to 3 × 3.
pub(crate) const GRIDS: [Grid; 9] = [
    Grid::new(2, 1),
    Grid::new(2, 2),
    Grid::new(3, 1),
    Grid::new(3, 2),
    Grid::new(3, 3),
    Grid::new(1, 2),
    Grid::new(1, 3),
    Grid::new(2, 3),
    Grid::new(1, 1),
];

/// The layout for `n` series: side by side for two, 2 × 2 up to four, 3 × 2 up to six, then
/// 3 × 3.
pub(crate) fn grid_for(n: usize) -> Grid {
    match n {
        0..=2 => Grid::new(2, 1),
        3 | 4 => Grid::new(2, 2),
        5 | 6 => Grid::new(3, 2),
        _ => Grid::new(3, 3),
    }
}

/// Where everything goes.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Layout {
    pub bar: R,
    pub strip: Option<R>,
    pub ports: Vec<R>,
}

/// Splits `area` (logical) into the bar (`bar_h` tall) on top, the strip (`strip_h`, 0 for
/// none) under it and `grid`'s viewports below (the toolbar floats over the bottom), one
/// pixel apart on whole physical pixels. Columns run right to left in right-to-left layouts (the images never mirror).
pub(crate) fn layout(
    area: R,
    scale: f64,
    rtl: bool,
    grid: Grid,
    bar_h: f64,
    strip_h: f64,
) -> Layout {
    let s = scale.max(0.1);
    let px = |v: f64| (v * s).round() / s;
    let (x0, y0, w, h) = (px(area[0]), px(area[1]), px(area[2]), px(area[3]));
    let bar_h = px(bar_h).min(h);
    let strip_h = px(strip_h).min((h - bar_h).max(0.0));
    let top = y0 + bar_h + strip_h;
    let bottom = y0 + h;
    let gap = 1.0 / s * s.round().max(1.0);
    let (cols, rows) = (usize::from(grid.cols.max(1)), usize::from(grid.rows.max(1)));
    let gh = (bottom - top).max(rows as f64);
    let cw = ((w - gap * (cols - 1) as f64) / cols as f64 * s).floor() / s;
    let ch = ((gh - gap * (rows - 1) as f64) / rows as f64 * s).floor() / s;
    let mut ports = Vec::with_capacity(cols * rows);
    for r in 0..rows {
        for c in 0..cols {
            let cc = if rtl { cols - 1 - c } else { c };
            let x = x0 + cc as f64 * (cw + gap);
            let y = top + r as f64 * (ch + gap);
            // The last column and row take what rounding left.
            let ww = if cc == cols - 1 { x0 + w - x } else { cw };
            let hh = if r == rows - 1 { bottom - y } else { ch };
            ports.push([x, y, ww.max(1.0), hh.max(1.0)]);
        }
    }
    Layout {
        bar: [x0, y0, w, bar_h],
        strip: (strip_h > 0.0).then_some([x0, y0 + bar_h, w, strip_h]),
        ports,
    }
}

pub(crate) fn rect(r: R) -> Rect {
    Rect::new(r[0] as f32, r[1] as f32, r[2] as f32, r[3] as f32)
}

pub(crate) fn contains(r: R, p: (f64, f64)) -> bool {
    p.0 >= r[0] && p.1 >= r[1] && p.0 < r[0] + r[2] && p.1 < r[1] + r[3]
}

/// A button in the bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Control {
    Layout,
    Lines,
    Window,
    Zoom,
    Link,
    Align,
    Close,
}

/// The slice drawn in a viewport: its texture and where it lands (may reach past the
/// viewport, which clips it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Img {
    pub tex: TexId,
    pub rect: Rect,
}

/// One viewport's content.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct PortScene {
    pub rect: R,
    pub image: Option<Img>,
    /// Series, slice and window lines (top start corner).
    pub lines: Vec<String>,
    /// Why the slice shown is not at the linked position (a gap, outside the series) or
    /// that the series is not linked.
    pub flag: Option<String>,
    /// Reference lines of the active slice, and its slab's faces (logical pixels).
    pub refs: Vec<[(f32, f32); 2]>,
    pub faces: Vec<[(f32, f32); 2]>,
    /// The 3D cursor, where it is on this slice.
    pub cursor: Option<(f32, f32)>,
    pub edges: Vec<Edge>,
    /// "Drag a series here" (no series).
    pub empty: bool,
}

/// Everything drawn over the image area.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Scene {
    pub layout: Layout,
    pub ports: Vec<PortScene>,
    pub active: usize,
    /// The bar's buttons: what, label, toggled on (`None`: not a toggle).
    pub controls: Vec<(Control, String, Option<bool>)>,
    /// The strip's series: entry, label, shown in the active viewport.
    pub chips: Vec<(usize, String, bool)>,
    /// A series dragged: its label and the pointer (logical).
    pub ghost: Option<(String, (f32, f32))>,
    /// The 3D cursor in words (for screen readers), when placed.
    pub cursor: Option<String>,
}

/// What [`draw`] placed, for hit tests and accessibility.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Placed {
    pub controls: Vec<(Control, Rect)>,
    pub chips: Vec<(usize, Rect)>,
}

/// Bar and strip heights for text scale `ts`.
pub(crate) fn bar_height(ts: f32) -> f64 {
    f64::from(34.0 * ts)
}

pub(crate) fn strip_height(ts: f32) -> f64 {
    f64::from(32.0 * ts)
}

const REF: Color = Color::rgba(1.0, 0.82, 0.2, 1.0);
const CURSOR: Color = Color::rgba(0.3, 0.9, 1.0, 1.0);
const FLAG: Color = Color::rgba(1.0, 0.62, 0.25, 1.0);

/// A row of buttons in `r` from the start edge; returns each one's rectangle.
fn buttons(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    r: Rect,
    items: &[(String, bool)],
    rtl: bool,
    ts: f32,
) -> Vec<Rect> {
    let st = TextStyle::new(FONT_BODY * ts);
    let pad = 8.0 * ts;
    let gap = GAP * ts * 0.75;
    let bh = (r.h - 8.0 * ts).max(1.0);
    let y = r.y + (r.h - bh) / 2.0;
    let mut x = PAD * ts;
    let mut out = Vec::new();
    for (label, on) in items {
        let b = text.layout(label, st, None, Align::Start);
        let w = b.width + 2.0 * pad;
        let bx = if rtl { r.x + r.w - x - w } else { r.x + x };
        let br = Rect::new(bx, y, w, bh);
        let (fill, ink) = if *on {
            (pal.accent_fill, pal.accent_text)
        } else {
            (pal.surface_raised, pal.text)
        };
        dl.rect_border(br, RADIUS_SM, fill, pal.separator, 1.0);
        text.draw(dl, &b, bx + pad, y + (bh - b.height) / 2.0, ink);
        out.push(br);
        x += w + gap;
    }
    out
}

/// Lines in a box at the top start corner of `r` (the first brighter), and the box.
fn readout(
    dl: &mut DrawList,
    text: &mut Text,
    lines: &[String],
    r: Rect,
    rtl: bool,
    ts: f32,
) -> Rect {
    let st = TextStyle::new(FONT_BODY * ts).tabular();
    let m = 6.0 * ts;
    let max_w = (r.w - 2.0 * m - 2.0 * PAD).max(0.0);
    let blocks: Vec<_> = lines
        .iter()
        .map(|l| {
            let l = super::super::convert::ellipsize(text, l, st, max_w);
            text.layout(&l, st, None, Align::Start)
        })
        .collect();
    let gap = 2.0 * ts;
    let w = blocks.iter().map(|b| b.width).fold(0.0, f32::max) + 2.0 * PAD;
    let h = blocks.iter().map(|b| b.height).sum::<f32>()
        + gap * blocks.len().saturating_sub(1) as f32
        + 2.0 * PAD;
    let x = if rtl { r.x + r.w - m - w } else { r.x + m };
    let b = Rect::new(x, r.y + m, w, h);
    dl.rect(b, RADIUS_MD, Color::rgba(0.0, 0.0, 0.0, 0.6));
    let mut ty = b.y + PAD;
    for (i, bl) in blocks.iter().enumerate() {
        let tx = if rtl { x + w - PAD - bl.width } else { x + PAD };
        let c = if i == 0 {
            Color::WHITE
        } else {
            Color::WHITE.with_alpha(0.82)
        };
        text.draw(dl, bl, tx, ty, c);
        ty += bl.height + gap;
    }
    b
}

/// Draws the scene; `ts` the text scale.
pub(crate) fn draw(
    dl: &mut DrawList,
    text: &mut Text,
    sc: &Scene,
    pal: &Palette,
    rtl: bool,
    ts: f32,
) -> Placed {
    let mut out = Placed::default();
    // The bar.
    let bar = rect(sc.layout.bar);
    dl.rect(bar, 0.0, pal.surface);
    dl.rect(
        Rect::new(bar.x, bar.y + bar.h - 1.0, bar.w, 1.0),
        0.0,
        pal.separator,
    );
    let items: Vec<(String, bool)> = sc
        .controls
        .iter()
        .map(|(_, l, on)| (l.clone(), on.unwrap_or(false)))
        .collect();
    let rects = buttons(dl, text, pal, bar, &items, rtl, ts);
    out.controls = sc.controls.iter().map(|c| c.0).zip(rects).collect();
    // The viewports.
    for (i, p) in sc.ports.iter().enumerate() {
        let r = rect(p.rect);
        dl.rect(r, 0.0, Color::BLACK);
        dl.push_clip(r);
        if let Some(img) = p.image {
            dl.image(img.rect, [0.0, 0.0, 1.0, 1.0], img.tex, 0.0);
        }
        for f in &p.faces {
            dl.line(f[0], f[1], 1.0, REF.with_alpha(0.55));
        }
        for l in &p.refs {
            dl.line(l[0], l[1], 3.0, Color::rgba(0.0, 0.0, 0.0, 0.45));
            dl.line(l[0], l[1], 1.5, REF);
        }
        if let Some(c) = p.cursor {
            let k = 7.0 * ts;
            for (a, b) in [
                ((c.0 - k, c.1), (c.0 + k, c.1)),
                ((c.0, c.1 - k), (c.0, c.1 + k)),
            ] {
                dl.line(a, b, 3.0, Color::rgba(0.0, 0.0, 0.0, 0.5));
                dl.line(a, b, 1.5, CURSOR);
            }
        }
        labels::draw(dl, text, &p.edges, ts);
        if !p.lines.is_empty() {
            readout(dl, text, &p.lines, r, rtl, ts);
        }
        if p.empty {
            let st = TextStyle::new(FONT_BODY * ts);
            let b = text.layout(&t!("linked-empty"), st, Some(r.w.max(1.0)), Align::Center);
            text.draw(
                dl,
                &b,
                r.x + (r.w - b.width) / 2.0,
                r.y + (r.h - b.height) / 2.0,
                pal.text_faint,
            );
        }
        if let Some(f) = &p.flag {
            let st = TextStyle::new(FONT_BODY * ts);
            let l = super::super::convert::ellipsize(text, f, st, (r.w - 4.0 * PAD * ts).max(0.0));
            let b = text.layout(&l, st, None, Align::Start);
            let w = b.width + 2.0 * PAD;
            let h = b.height + PAD;
            // Above the bottom edge's orientation letter.
            let br = Rect::new(r.x + (r.w - w) / 2.0, r.y + r.h - h - 34.0 * ts, w, h);
            dl.rect(br, RADIUS_MD, Color::rgba(0.0, 0.0, 0.0, 0.7));
            text.draw(dl, &b, br.x + PAD, br.y + PAD / 2.0, FLAG);
        }
        dl.pop_clip();
        // The active viewport's outline.
        if i == sc.active {
            dl.stroke_rect(r.inset(1.0), 0.0, pal.accent, 2.0);
        }
    }
    // The strip.
    if let Some(s) = sc.layout.strip {
        let sr = rect(s);
        dl.rect(sr, 0.0, pal.surface);
        dl.rect(
            Rect::new(sr.x, sr.y + sr.h - 1.0, sr.w, 1.0),
            0.0,
            pal.separator,
        );
        dl.push_clip(sr);
        let items: Vec<(String, bool)> = sc.chips.iter().map(|c| (c.1.clone(), c.2)).collect();
        let rects = buttons(dl, text, pal, sr, &items, rtl, ts);
        out.chips = sc.chips.iter().map(|c| c.0).zip(rects).collect();
        dl.pop_clip();
    }
    if let Some((label, at)) = &sc.ghost {
        let st = TextStyle::new(FONT_BODY * ts);
        let b = text.layout(label, st, None, Align::Start);
        let r = Rect::new(
            at.0 + 12.0 * ts,
            at.1 + 12.0 * ts,
            b.width + 2.0 * PAD,
            b.height + PAD,
        );
        dl.rect_border(
            r,
            RADIUS_SM,
            pal.surface_raised.with_alpha(0.92),
            pal.accent,
            1.5,
        );
        text.draw(dl, &b, r.x + PAD, r.y + PAD / 2.0, pal.text);
    }
    out
}

/// The accessible name of viewport `i`: its number, whether active, then what it shows.
pub(crate) fn port_label(sc: &Scene, i: usize) -> String {
    let p = &sc.ports[i];
    let n = (i + 1) as i64;
    let count = sc.ports.len() as i64;
    let active = if i == sc.active { "yes" } else { "no" };
    let mut parts = vec![t!("linked-viewport", n = n, count = count, active = active)];
    if p.empty {
        parts.push(t!("linked-empty"));
    }
    parts.extend(p.lines.iter().cloned());
    parts.extend(p.flag.iter().cloned());
    if !p.edges.is_empty() {
        parts.push(labels::a11y_label(&p.edges));
    }
    if p.cursor.is_some()
        && let Some(c) = &sc.cursor
    {
        parts.push(c.clone());
    }
    parts.join(", ")
}

/// The accessibility nodes: the bar's buttons, each viewport (named, the active one says
/// so and is selected) and the strip's series.
pub(crate) fn a11y(tree: &mut accesskit::TreeUpdate, sc: &Scene, placed: &Placed, scale: f64) {
    use accesskit::{Action, Node, NodeId, Role, Toggled};
    let bounds = |r: Rect| accesskit::Rect {
        x0: f64::from(r.x) * scale,
        y0: f64::from(r.y) * scale,
        x1: f64::from(r.x + r.w) * scale,
        y1: f64::from(r.y + r.h) * scale,
    };
    let mut children = Vec::new();
    let mut bar = Node::new(Role::Toolbar);
    bar.set_label(t!("palette-compare-series"));
    bar.set_bounds(bounds(rect(sc.layout.bar)));
    for (i, (c, r)) in placed.controls.iter().enumerate() {
        let Some((_, label, on)) = sc.controls.iter().find(|x| x.0 == *c) else {
            continue;
        };
        let mut n = Node::new(Role::Button);
        n.set_label(label.clone());
        n.set_bounds(bounds(*r));
        n.add_action(Action::Click);
        if let Some(on) = on {
            n.set_toggled(if *on { Toggled::True } else { Toggled::False });
        }
        let id = NodeId(A11Y_BASE + 0x100 + i as u64);
        tree.nodes.push((id, n));
        bar.push_child(id);
    }
    let bid = NodeId(A11Y_BASE + 1);
    tree.nodes.push((bid, bar));
    children.push(bid);
    if let Some(s) = sc.layout.strip {
        let mut strip = Node::new(Role::Toolbar);
        strip.set_label(t!("dicom-series-group"));
        strip.set_bounds(bounds(rect(s)));
        let clip = rect(s);
        for (i, (k, r)) in placed.chips.iter().enumerate() {
            // Chips scrolled off the strip are left out.
            if r.x + r.w <= clip.x || r.x >= clip.x + clip.w {
                continue;
            }
            let Some((_, label, on)) = sc.chips.iter().find(|c| c.0 == *k) else {
                continue;
            };
            let mut n = Node::new(Role::Button);
            n.set_label(label.clone());
            n.set_bounds(bounds(r.intersect(&clip)));
            n.add_action(Action::Click);
            n.set_toggled(if *on { Toggled::True } else { Toggled::False });
            let id = NodeId(A11Y_BASE + 0x300 + i as u64);
            tree.nodes.push((id, n));
            strip.push_child(id);
        }
        let sid = NodeId(A11Y_BASE + 2);
        tree.nodes.push((sid, strip));
        children.push(sid);
    }
    for (i, p) in sc.ports.iter().enumerate() {
        let mut n = Node::new(Role::Image);
        n.set_label(port_label(sc, i));
        n.set_bounds(bounds(rect(p.rect)));
        n.add_action(Action::Click);
        if i == sc.active {
            n.set_selected(true);
        }
        let id = NodeId(A11Y_BASE + 0x200 + i as u64);
        tree.nodes.push((id, n));
        children.push(id);
    }
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        for c in children {
            root.push_child(c);
        }
    }
}

/// Maps a slice's pixel position to the viewport's logical pixels: the image of `size`
/// pixels (columns, rows) at `spacing` mm (between rows, between columns) shown at `zoom`
/// logical pixels per mm, its centre `pan` mm (right, down) from the viewport's centre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Mapping {
    pub port: R,
    pub size: [u32; 2],
    pub spacing: [f64; 2],
    pub zoom: f64,
    pub pan: (f64, f64),
}

impl Mapping {
    /// The image's rectangle on screen.
    pub(crate) fn image_rect(self) -> R {
        let w = f64::from(self.size[0]) * self.spacing[1] * self.zoom;
        let h = f64::from(self.size[1]) * self.spacing[0] * self.zoom;
        let cx = self.port[0] + self.port[2] / 2.0 + self.pan.0 * self.zoom;
        let cy = self.port[1] + self.port[3] / 2.0 + self.pan.1 * self.zoom;
        [cx - w / 2.0, cy - h / 2.0, w, h]
    }

    /// Pixel (column, row; centres on whole numbers) to screen.
    pub(crate) fn to_screen(self, x: f64, y: f64) -> (f64, f64) {
        let r = self.image_rect();
        (
            r[0] + (x + 0.5) * self.spacing[1] * self.zoom,
            r[1] + (y + 0.5) * self.spacing[0] * self.zoom,
        )
    }

    /// Screen to pixel.
    pub(crate) fn to_pixel(self, p: (f64, f64)) -> (f64, f64) {
        let r = self.image_rect();
        (
            (p.0 - r[0]) / (self.spacing[1] * self.zoom) - 0.5,
            (p.1 - r[1]) / (self.spacing[0] * self.zoom) - 0.5,
        )
    }

    /// The zoom that fits the whole image in the viewport (logical pixels per mm).
    pub(crate) fn fit(port: R, size: [u32; 2], spacing: [f64; 2]) -> f64 {
        let w = f64::from(size[0].max(1)) * spacing[1];
        let h = f64::from(size[1].max(1)) * spacing[0];
        (port[2] / w).min(port[3] / h).max(1e-6)
    }
}
