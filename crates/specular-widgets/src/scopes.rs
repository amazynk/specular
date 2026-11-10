//! Develop's scope dock: at the top of the panel, one row (the scope picker,
//! "Histogram ▾", which opens a menu of the scopes, and the scope's mode chip) over the
//! scope of the picture. The picture of the scope
//! comes from the application (made at the size [`ScopeDock::picture_size`] asks for); the
//! dock draws the tabs, the frame and the graticule over it: level lines for the waveform
//! and parade, quarter lines for the histogram, and for the vectorscope chroma rings, the
//! skin-tone line and the output gamut's primaries and secondaries (the proof profile's
//! too while soft-proofing, dashed).
//!
//! Scopes are mathematical axes: they never mirror in right-to-left languages (the header
//! row does). The chip at the row's end switches the waveform between luma and R, G and B
//! over each other, and the histogram between a log and a linear scale (log by default: a
//! dark or bright photo's pile at one end would flatten the rest).

use specular_gfx::{Align, DrawList, Icon, Rect, TexId, Text, TextStyle, draw_icon};
use specular_i18n::t;
use specular_theme::{Color, Palette, metrics};

/// Header row height and gaps (logical px at the standard text size).
const ROW: f32 = 26.0;
const GAP: f32 = 6.0;
/// The picture's height over its width: about 130 px in a standard panel (the panel's
/// controls are what the room is for; the vectorscope is a square of that height).
const ASPECT: f32 = 0.42;

/// Which scope shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Scope {
    #[default]
    Histogram,
    Waveform,
    Parade,
    Vectorscope,
}

impl Scope {
    pub const ALL: [Scope; 4] = [
        Scope::Histogram,
        Scope::Waveform,
        Scope::Parade,
        Scope::Vectorscope,
    ];

    pub fn name(self) -> String {
        match self {
            Scope::Histogram => t!("develop-scope-histogram"),
            Scope::Waveform => t!("develop-scope-waveform"),
            Scope::Parade => t!("develop-scope-parade"),
            Scope::Vectorscope => t!("develop-scope-vectorscope"),
        }
    }
}

/// The vectorscope's marks: OkLab a/b of red, yellow, green, cyan, blue, magenta.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Marks {
    pub gamut: [[f32; 2]; 6],
    /// The proof profile's, while soft-proofing.
    pub proof: Option<[[f32; 2]; 6]>,
    /// The skin-tone line's hue (radians from +a towards +b).
    pub skin: f32,
    /// a/b from −range to +range across the square.
    pub range: f32,
}

/// What a press hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// "Histogram ▾": the menu of scopes (the application opens it).
    Picker,
    /// The scope's mode chip: the waveform's Luma/RGB, the histogram's Log/Linear.
    Mode,
}

/// What a press (or a screen reader's click) did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Press {
    /// Not the dock's.
    Outside,
    /// The dock's; nothing changed.
    Took,
    /// The scope or its mode changed (say [`ScopeDock::status`]).
    Changed,
    /// The picker: open the menu of scopes under it ([`ScopeDock::choose`] for a row).
    Picker(Rect),
}

/// The dock; see the module docs.
#[derive(Clone, Debug, Default)]
pub struct ScopeDock {
    pub scope: Scope,
    /// The waveform shows R, G and B (else luma).
    pub rgb: bool,
    /// The histogram's heights are on a log scale (else linear).
    pub log: bool,
    /// The dock is shown.
    pub shown: bool,
    /// The picker's menu is open: its button says so to a screen reader.
    pub menu_open: bool,
    hover: Option<Part>,
    picker: Option<Rect>,
    mode: Option<Rect>,
    picture: Rect,
    area: Rect,
}

impl ScopeDock {
    pub fn new() -> ScopeDock {
        ScopeDock {
            shown: true,
            log: true,
            ..Default::default()
        }
    }

    /// The dock's height in a panel `panel_w` wide (logical px) at text scale `ts`; 0 when
    /// hidden.
    pub fn height(&self, _text: &mut Text, panel_w: f32, ts: f32) -> f32 {
        if !self.shown {
            return 0.0;
        }
        let g = ts.max(1.0);
        let (_, h) = self.picture_size(panel_w, ts);
        (ROW * g).round() + h + (3.0 * GAP * g).round()
    }

    /// Shows scope `s` (a row of the picker's menu); whether that changed anything.
    pub fn choose(&mut self, s: Scope) -> bool {
        let changed = !self.shown || self.scope != s;
        self.shown = true;
        self.scope = s;
        changed
    }

    /// The scope picture's size (logical px) in a panel `panel_w` wide.
    pub fn picture_size(&self, panel_w: f32, ts: f32) -> (f32, f32) {
        let g = ts.max(1.0);
        let w = (panel_w - 2.0 * (metrics::PAD * g).round())
            .max(40.0)
            .round();
        (w, (w * ASPECT).round())
    }

    /// Where the scope picture went at the last draw (logical px).
    pub fn picture_rect(&self) -> Rect {
        self.picture
    }

    /// Steps to the next view: histogram, waveform luma, waveform RGB, parade, vectorscope,
    /// hidden, and round again. Returns the new state's name for the screen reader.
    pub fn cycle(&mut self) -> String {
        if !self.shown {
            self.shown = true;
            self.scope = Scope::Histogram;
        } else {
            match (self.scope, self.rgb) {
                (Scope::Histogram, _) => {
                    self.scope = Scope::Waveform;
                    self.rgb = false;
                }
                (Scope::Waveform, false) => self.rgb = true,
                (Scope::Waveform, true) => self.scope = Scope::Parade,
                (Scope::Parade, _) => self.scope = Scope::Vectorscope,
                (Scope::Vectorscope, _) => self.shown = false,
            }
        }
        self.status()
    }

    /// The state in words.
    pub fn status(&self) -> String {
        if !self.shown {
            return t!("develop-scopes-hidden");
        }
        match self.mode_name() {
            Some(mode) => format!("{} · {mode}", self.scope.name()),
            None => self.scope.name(),
        }
    }

    pub fn contains(&self, p: (f32, f32)) -> bool {
        self.shown && self.area.contains(p)
    }

    fn hit(&self, p: (f32, f32)) -> Option<Part> {
        if self.mode.is_some_and(|r| r.contains(p)) {
            return Some(Part::Mode);
        }
        self.picker.filter(|r| r.contains(p)).map(|_| Part::Picker)
    }

    /// A press at `p`.
    pub fn press(&mut self, p: (f32, f32)) -> Press {
        if !self.contains(p) {
            return Press::Outside;
        }
        match self.hit(p) {
            Some(part) => self.activate(part),
            None => Press::Took,
        }
    }

    /// Asks for the picker's menu, or flips the scope's mode.
    pub fn activate(&mut self, part: Part) -> Press {
        match part {
            Part::Picker => Press::Picker(self.picker.unwrap_or(self.area)),
            Part::Mode => match self.scope {
                Scope::Waveform => {
                    self.rgb = !self.rgb;
                    Press::Changed
                }
                Scope::Histogram => {
                    self.log = !self.log;
                    Press::Changed
                }
                Scope::Parade | Scope::Vectorscope => Press::Took,
            },
        }
    }

    /// The current scope's mode in words (its chip), if it has one.
    fn mode_name(&self) -> Option<String> {
        match self.scope {
            Scope::Waveform if self.rgb => Some(t!("develop-scope-rgb")),
            Scope::Waveform => Some(t!("develop-scope-luma")),
            Scope::Histogram if self.log => Some(t!("develop-scope-log")),
            Scope::Histogram => Some(t!("develop-scope-linear")),
            Scope::Parade | Scope::Vectorscope => None,
        }
    }

    /// The pointer moved; whether the hover changed.
    pub fn pointer_move(&mut self, p: Option<(f32, f32)>) -> bool {
        let h = p.filter(|&p| self.contains(p)).and_then(|p| self.hit(p));
        let changed = h != self.hover;
        self.hover = h;
        changed
    }

    /// Draws the dock in `area` (its rect in the panel, logical px) with the scope picture
    /// `tex` (`None` while it is being made).
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        area: Rect,
        tex: Option<TexId>,
        marks: &Marks,
        rtl: bool,
        ts: f32,
    ) {
        self.area = area;
        self.picker = None;
        self.mode = None;
        if !self.shown {
            return;
        }
        let g = ts.max(1.0);
        let u = |v: f32| (v * g).round();
        let pad = u(metrics::PAD);
        // One row: the picker at the start, the mode chip at the end (the row mirrors; the
        // scope below does not).
        let row = Rect::new(area.x + pad, area.y + u(GAP), area.w - 2.0 * pad, u(ROW));
        let style = tab_style(ts);
        let chev = u(14.0);
        let name = self.scope.name();
        let b = text.layout(&name, style, Some(row.w * 0.6), Align::Start);
        let pw = (b.width + chev + u(20.0)).min(row.w * 0.7);
        let picker = Rect::new(if rtl { row.right() - pw } else { row.x }, row.y, pw, row.h);
        if self.hover == Some(Part::Picker) {
            dl.rect(picker, metrics::RADIUS_SM, pal.surface_hover);
        }
        let tx = if rtl {
            picker.right() - u(8.0) - b.width
        } else {
            picker.x + u(8.0)
        };
        text.draw(dl, &b, tx, row.y + (row.h - b.height) * 0.5, pal.text);
        let cx = if rtl {
            picker.x + u(4.0)
        } else {
            picker.right() - u(4.0) - chev
        };
        let ir = Rect::new(cx, row.y + (row.h - chev) * 0.5, chev, chev);
        draw_icon(
            dl,
            Icon::ChevronDown,
            ir,
            pal.text_muted,
            metrics::ICON_STROKE,
            false,
        );
        self.picker = Some(picker);
        if let Some(m) = self.mode_name() {
            let small = TextStyle::new(metrics::FONT_SMALL * ts);
            let b = text.layout(&m, small, None, Align::Start);
            let w = b.width + u(16.0);
            let h = (b.height + u(6.0)).min(row.h);
            let r = Rect::new(
                if rtl { row.x } else { row.right() - w },
                row.y + (row.h - h) * 0.5,
                w,
                h,
            );
            let fill = if self.hover == Some(Part::Mode) {
                pal.surface_hover
            } else {
                pal.surface
            };
            dl.rect_border(r, metrics::RADIUS_SM, fill, pal.separator, 1.0);
            text.draw(
                dl,
                &b,
                r.x + (r.w - b.width) * 0.5,
                r.y + (r.h - b.height) * 0.5,
                pal.text_muted,
            );
            self.mode = Some(r);
        }
        let (pw, ph) = self.picture_size(area.w, ts);
        let pic = Rect::new(area.x + pad, row.bottom() + u(GAP), pw, ph);
        // The vectorscope is square, centred.
        let shown = if self.scope == Scope::Vectorscope {
            Rect::new(pic.x + (pic.w - pic.h) * 0.5, pic.y, pic.h, pic.h)
        } else {
            pic
        };
        self.picture = shown;
        dl.rect(pic, metrics::RADIUS_SM, Color::hex(0x121214));
        if let Some(t) = tex {
            dl.image(shown, [0.0, 0.0, 1.0, 1.0], t, 0.0);
        }
        let grid = Color::rgba(1.0, 1.0, 1.0, 0.16);
        let label = Color::rgba(1.0, 1.0, 1.0, 0.55);
        let small = TextStyle::new(metrics::FONT_SMALL * ts * 0.85);
        match self.scope {
            Scope::Histogram => {
                for q in 1..4 {
                    let x = shown.x + shown.w * q as f32 / 4.0;
                    dl.line((x, shown.y), (x, shown.bottom()), 1.0, grid);
                }
            }
            Scope::Waveform | Scope::Parade => {
                for (q, s) in [
                    (0.0, "0"),
                    (0.25, ""),
                    (0.5, "50"),
                    (0.75, ""),
                    (1.0, "100"),
                ] {
                    let y = shown.bottom() - shown.h * q;
                    let y = y.clamp(shown.y + 0.5, shown.bottom() - 0.5);
                    dl.line((shown.x, y), (shown.right(), y), 1.0, grid);
                    if !s.is_empty() {
                        // Numbers on the left: the axis does not mirror.
                        let b = text.layout(s, small, None, Align::Start);
                        let ty = (y - b.height - 1.0).max(shown.y);
                        text.draw(dl, &b, shown.x + 3.0, ty, label);
                    }
                }
                if self.scope == Scope::Parade {
                    for k in 1..3 {
                        let x = shown.x + shown.w * k as f32 / 3.0;
                        dl.line((x, shown.y), (x, shown.bottom()), 1.0, grid);
                    }
                }
            }
            Scope::Vectorscope => self.draw_vector(dl, shown, marks, ts),
        }
        dl.stroke_rect(pic, metrics::RADIUS_SM, pal.separator, 1.0);
    }

    fn draw_vector(&self, dl: &mut DrawList, r: Rect, m: &Marks, ts: f32) {
        let c = r.center();
        let range = if m.range > 0.0 { m.range } else { 0.4 };
        let k = r.w * 0.5 / range;
        let at = |ab: [f32; 2]| (c.0 + ab[0] * k, c.1 - ab[1] * k);
        let grid = Color::rgba(1.0, 1.0, 1.0, 0.16);
        // Rings every 0.1 of chroma, and the axes.
        for ring in 1..4 {
            polygon(dl, &circle(c, ring as f32 * 0.1 * k, 72), grid, 1.0, false);
        }
        dl.line((r.x, c.1), (r.right(), c.1), 1.0, grid);
        dl.line((c.0, r.y), (c.0, r.bottom()), 1.0, grid);
        // Skin-tone line.
        let len = r.w * 0.5;
        let skin = (c.0 + m.skin.cos() * len, c.1 - m.skin.sin() * len);
        dl.line(c, skin, 1.0, Color::rgba(1.0, 0.85, 0.7, 0.5));
        // The gamut's hexagon and its corners, in their own colours.
        let colours = [
            Color::hex(0xff4040),
            Color::hex(0xffe040),
            Color::hex(0x40ff60),
            Color::hex(0x40f0ff),
            Color::hex(0x5070ff),
            Color::hex(0xff50f0),
        ];
        let g = ts.max(1.0);
        let pts: Vec<(f32, f32)> = m.gamut.iter().map(|&p| at(p)).collect();
        polygon(dl, &pts, Color::rgba(1.0, 1.0, 1.0, 0.3), 1.0, true);
        for (p, col) in pts.iter().zip(colours) {
            let s = 3.0 * g;
            dl.stroke_rect(Rect::new(p.0 - s, p.1 - s, 2.0 * s, 2.0 * s), 0.0, col, 1.5);
        }
        if let Some(proof) = m.proof {
            let pts: Vec<(f32, f32)> = proof.iter().map(|&p| at(p)).collect();
            dashed_polygon(dl, &pts, Color::rgba(1.0, 1.0, 1.0, 0.75), 1.0, 4.0 * g);
            for (p, col) in pts.iter().zip(colours) {
                dl.circle(*p, 2.5 * g, col);
            }
        }
    }

    /// The dock's screen-reader nodes: the picker (a button with a menu), the scope as an
    /// image, and its mode chip.
    pub fn a11y(&self, tree: &mut accesskit::TreeUpdate, base: u64, scale: f64) {
        use accesskit::{Action, Node, NodeId, Role};
        if !self.shown {
            return;
        }
        let bounds = |r: Rect| accesskit::Rect {
            x0: f64::from(r.x) * scale,
            y0: f64::from(r.y) * scale,
            x1: f64::from(r.x + r.w) * scale,
            y1: f64::from(r.y + r.h) * scale,
        };
        let mut group = Node::new(Role::Group);
        group.set_label(t!("develop-scopes"));
        group.set_bounds(bounds(self.area));
        let mut kids = Vec::new();
        if let Some(r) = self.picker {
            let mut n = Node::new(Role::Button);
            n.set_label(t!("develop-scopes"));
            n.set_value(self.scope.name());
            n.set_has_popup(accesskit::HasPopup::Menu);
            n.set_expanded(self.menu_open);
            n.add_action(Action::Click);
            n.set_bounds(bounds(r));
            kids.push(NodeId(base + 2));
            tree.nodes.push((NodeId(base + 2), n));
        }
        let mut img = Node::new(Role::Image);
        img.set_label(self.status());
        img.set_bounds(bounds(self.picture));
        kids.push(NodeId(base + 8));
        tree.nodes.push((NodeId(base + 8), img));
        if let (Some(r), Some(mode)) = (self.mode, self.mode_name()) {
            let mut n = Node::new(Role::Button);
            n.set_label(if self.scope == Scope::Histogram {
                t!("develop-scope-scale")
            } else {
                t!("develop-scope-mode")
            });
            n.set_value(mode);
            n.add_action(Action::Click);
            n.set_bounds(bounds(r));
            kids.push(NodeId(base + 9));
            tree.nodes.push((NodeId(base + 9), n));
        }
        group.set_children(kids);
        let root = tree.tree.as_ref().map_or(NodeId(1), |t| t.root);
        if let Some((_, w)) = tree.nodes.iter_mut().find(|(id, _)| *id == root) {
            w.push_child(NodeId(base));
        }
        tree.nodes.push((NodeId(base), group));
    }

    /// A screen reader's click on one of the dock's nodes; `None` when it is not one.
    pub fn a11y_click(&mut self, base: u64, id: u64) -> Option<Press> {
        match id.checked_sub(base)? {
            2 => Some(self.activate(Part::Picker)),
            9 => Some(self.activate(Part::Mode)),
            0 | 8 => Some(Press::Took),
            _ => None,
        }
    }
}

fn tab_style(ts: f32) -> TextStyle {
    TextStyle::new(metrics::FONT_SMALL * ts).weight(600.0)
}

/// Points around a circle.
fn circle(c: (f32, f32), r: f32, n: usize) -> Vec<(f32, f32)> {
    (0..n)
        .map(|i| {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            (c.0 + r * a.cos(), c.1 + r * a.sin())
        })
        .collect()
}

fn polygon(dl: &mut DrawList, pts: &[(f32, f32)], col: Color, w: f32, _open: bool) {
    for i in 0..pts.len() {
        dl.line(pts[i], pts[(i + 1) % pts.len()], w, col);
    }
}

fn dashed_polygon(dl: &mut DrawList, pts: &[(f32, f32)], col: Color, w: f32, dash: f32) {
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        let n = (len / dash).floor().max(1.0) as usize;
        for k in (0..n).step_by(2) {
            let t0 = k as f32 / n as f32;
            let t1 = ((k + 1) as f32 / n as f32).min(1.0);
            let p = |t: f32| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            dl.line(p(t0), p(t1), w, col);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_walks_every_view_and_back() {
        let mut d = ScopeDock::new();
        let mut seen = Vec::new();
        for _ in 0..6 {
            d.cycle();
            seen.push((d.shown, d.scope, d.rgb));
        }
        assert_eq!(
            seen,
            [
                (true, Scope::Waveform, false),
                (true, Scope::Waveform, true),
                (true, Scope::Parade, true),
                (true, Scope::Vectorscope, true),
                (false, Scope::Vectorscope, true),
                (true, Scope::Histogram, true),
            ]
        );
        // Compact: well under half the panel's width.
        let h = d.picture_size(300.0, 1.0).1;
        assert!((100.0..150.0).contains(&h), "{h}");
    }

    #[test]
    fn picker_asks_for_the_menu_and_the_chip_flips_the_mode() {
        let mut d = ScopeDock::new();
        assert!(d.log, "the histogram starts on a log scale");
        assert!(matches!(d.a11y_click(100, 102), Some(Press::Picker(_))));
        // The chip: the histogram's scale, then the waveform's luma or RGB.
        assert_eq!(d.a11y_click(100, 109), Some(Press::Changed));
        assert!(!d.log);
        assert!(d.choose(Scope::Waveform));
        assert!(!d.choose(Scope::Waveform));
        assert_eq!(d.a11y_click(100, 109), Some(Press::Changed));
        assert!(d.rgb);
        d.choose(Scope::Parade);
        assert_eq!(d.a11y_click(100, 109), Some(Press::Took));
        d.shown = false;
        assert!(d.choose(Scope::Parade), "choosing shows the dock again");
        assert_eq!(d.a11y_click(100, 120), None);
        assert_eq!(d.a11y_click(100, 50), None);
    }
}
