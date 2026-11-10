//! The right-click menu: groups of rows (icon, name, shortcut) under a small title, opened at
//! the pointer ([`Chrome::open_menu`](crate::Chrome::open_menu)). A row sends its
//! [`PaletteItem::id`] back as [`Command::Item`](crate::Command::Item), as the palette's
//! lists do, so the application handles both the same way.
//!
//! A row that leads to more choices (its icon is [`SUBMENU`]) opens them beside it, as a
//! submenu: resting the pointer on it a moment (or →, Enter, a click) sends its id, and the
//! rows the application answers with ([`Chrome::open_submenu`](crate::Chrome::open_submenu))
//! open next to it.

use specular_gfx::{Icon, Rect, TextStyle, draw_icon};
use specular_input::KeyCombo;
use specular_theme::{Color, Mode, metrics};

use crate::PaletteItem;
use crate::ctx::{Ctx, HAlign, Target};

/// How long the menu takes to fade and slide in (a strong ease-out: it is there at once and
/// settles, never lags the click).
const OPEN_SECS: f64 = 0.15;

/// How long a chosen row stays lit before the menu goes: long enough to see which one it took.
const CHOSEN_HOLD_SECS: f64 = 0.08;

/// How long the menu then takes to fade out (quicker than it came: the choice is made).
const CLOSE_SECS: f64 = 0.12;

/// How long the pointer rests on a row that leads to more choices before they open: passing
/// over it on the way to another row opens nothing.
pub(crate) const SUB_DELAY_SECS: f64 = 0.12;

/// How long the pointer may rest on another row on its way to an open submenu before that
/// row takes over (it went there after all).
pub(crate) const AIM_SECS: f64 = 0.3;

/// A row whose icon is this leads to more choices: it shows a chevron at its end, and no icon.
pub(crate) const SUBMENU: Icon = Icon::ChevronRight;

/// A row whose icon is this destroys something (Move to trash): it is drawn in the danger
/// colour, and lit with it.
const DESTRUCTIVE: Icon = Icon::Trash;

/// Among rows that show a tick for the choice made, this marks the others (the palette's
/// lists show it); a menu leaves them blank.
const UNCHECKED: Icon = Icon::Minus;

/// A row of the menu, or of its submenu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Row {
    Main(usize),
    Sub(usize),
}

/// The choices a row leads to, open beside it.
pub(crate) struct Sub {
    /// The menu's row it opened from.
    pub(crate) parent: usize,
    pub(crate) rows: Vec<PaletteItem>,
    pub(crate) breaks: Vec<usize>,
    /// The row the keyboard (or the pointer) is on; none while the keyboard is in the menu.
    pub(crate) sel: Option<usize>,
    /// Shown above the rows while more are coming ("Looking for applications…").
    pub(crate) loading: Option<String>,
    /// Where it was last drawn: the pointer heading there keeps it open.
    pub(crate) rect: Option<Rect>,
}

pub(crate) struct ContextMenu {
    /// Where it was opened (the pointer, or the focused item for the menu key).
    pub(crate) at: (f32, f32),
    /// Shown above the rows, muted (what the menu is about: a file's name).
    pub(crate) title: Option<String>,
    pub(crate) rows: Vec<PaletteItem>,
    /// The rows a divider goes above (the first row of every group but the first).
    pub(crate) breaks: Vec<usize>,
    /// The row lit: the one the keyboard or the pointer is on (one only, whichever moved last).
    pub(crate) sel: Option<usize>,
    /// When it was first drawn (NaN until then).
    pub(crate) opened: f64,
    /// The row chosen, and when the menu was first drawn after (NaN until then): it is going,
    /// with that row lit, and takes no input.
    pub(crate) chosen: Option<(Row, f64)>,
    pub(crate) sub: Option<Sub>,
    /// The row whose choices were asked for (its id was sent), and whether from the keyboard
    /// (its first row is then selected): they open with the next
    /// [`open_submenu`](crate::Chrome::open_submenu).
    pub(crate) sub_wanted: Option<(usize, bool)>,
    /// The row that leads to more choices the pointer is resting on, and since when: they are
    /// asked for after [`SUB_DELAY_SECS`].
    pub(crate) hover_wait: Option<(usize, f64)>,
    /// The other row the pointer is on while heading for the open submenu, and since when:
    /// it takes over after [`AIM_SECS`] there.
    pub(crate) aim: Option<(usize, f64)>,
}

/// Rows in groups, flattened: the rows, and the ones a divider goes above.
fn flatten(groups: Vec<Vec<PaletteItem>>) -> (Vec<PaletteItem>, Vec<usize>) {
    let mut rows = Vec::new();
    let mut breaks = Vec::new();
    for g in groups.into_iter().filter(|g| !g.is_empty()) {
        if !rows.is_empty() {
            breaks.push(rows.len());
        }
        rows.extend(g);
    }
    (rows, breaks)
}

/// Whether row `i` of `rows` can be chosen.
fn enabled(rows: &[PaletteItem], i: usize) -> bool {
    rows.get(i).is_some_and(|r| r.unavailable.is_none())
}

/// The next row of `rows` that can be chosen after `from` (before it, when not `forward`),
/// wrapping around; from none, the first (or last) one.
pub(crate) fn step(rows: &[PaletteItem], from: Option<usize>, forward: bool) -> Option<usize> {
    let n = rows.len();
    if n == 0 {
        return None;
    }
    let mut i = match (from, forward) {
        (None, true) => n - 1,
        (None, false) => 0,
        (Some(s), _) => s.min(n - 1),
    };
    for _ in 0..n {
        i = if forward {
            (i + 1) % n
        } else {
            (i + n - 1) % n
        };
        if enabled(rows, i) {
            return Some(i);
        }
    }
    None
}

impl Sub {
    pub(crate) fn new(
        parent: usize,
        items: Vec<PaletteItem>,
        loading: Option<String>,
        keyboard: bool,
    ) -> Self {
        let (rows, breaks) = flatten(vec![items]);
        let sel = if keyboard {
            step(&rows, None, true)
        } else {
            None
        };
        Self {
            parent,
            rows,
            breaks,
            sel,
            loading,
            rect: None,
        }
    }

    /// New rows (more came, or they changed): the selection stays on a row that is there.
    pub(crate) fn update(&mut self, items: Vec<PaletteItem>, loading: Option<String>) {
        let (rows, breaks) = flatten(vec![items]);
        self.sel = self
            .sel
            .map(|s| s.min(rows.len().saturating_sub(1)))
            .filter(|&s| enabled(&rows, s));
        self.rows = rows;
        self.breaks = breaks;
        self.loading = loading;
    }

    pub(crate) fn enabled(&self, j: usize) -> bool {
        enabled(&self.rows, j)
    }
}

impl ContextMenu {
    pub(crate) fn new(
        at: (f32, f32),
        title: Option<String>,
        groups: Vec<Vec<PaletteItem>>,
    ) -> Self {
        let (rows, breaks) = flatten(groups);
        Self {
            at,
            title,
            rows,
            breaks,
            sel: None,
            opened: f64::NAN,
            chosen: None,
            sub: None,
            sub_wanted: None,
            hover_wait: None,
            aim: None,
        }
    }

    /// The pointer is on row `i` (not on its way to the submenu): it is the row lit, another
    /// row's submenu closes, and a row that leads to more choices opens them once the pointer
    /// has rested there a moment (`hover_wait`).
    pub(crate) fn point_at(&mut self, i: usize, now: f64) {
        self.aim = None;
        if self.sub.as_ref().is_some_and(|s| s.parent == i) {
            // Back on its row: the keyboard is in the menu again.
            if let Some(s) = self.sub.as_mut() {
                s.sel = None;
            }
            self.sel = Some(i);
            return;
        }
        self.sub = None;
        if self.sub_wanted.is_some_and(|(w, _)| w != i) {
            self.sub_wanted = None;
        }
        if !self.enabled(i) {
            self.sel = None;
            self.hover_wait = None;
            return;
        }
        self.sel = Some(i);
        if !self.leads_on(i) {
            self.hover_wait = None;
        } else if self.sub_wanted.is_none() && self.hover_wait.is_none_or(|(w, _)| w != i) {
            self.hover_wait = Some((i, now));
        }
    }

    /// Its timings start at the first frame that shows them, not at the input: the main thread
    /// can be held between the two (a Service call waits for the app it calls), and the menu
    /// must not have faded out before it was ever seen.
    pub(crate) fn stamp(&mut self, now: f64) {
        if self.opened.is_nan() {
            self.opened = now;
        }
        if let Some((_, at)) = &mut self.chosen
            && at.is_nan()
        {
            *at = now;
        }
    }

    /// A row was chosen and the menu is on its way out.
    pub(crate) fn closing(&self) -> bool {
        self.chosen.is_some()
    }

    /// Whether row `i` can be chosen.
    pub(crate) fn enabled(&self, i: usize) -> bool {
        enabled(&self.rows, i)
    }

    /// Whether row `i` leads to more choices.
    pub(crate) fn leads_on(&self, i: usize) -> bool {
        self.rows.get(i).is_some_and(|r| r.icon == SUBMENU)
    }

    /// The next row that can be chosen after `from` (see [`step`]).
    pub(crate) fn step(&self, from: Option<usize>, forward: bool) -> Option<usize> {
        step(&self.rows, from, forward)
    }

    /// Whether the menu's row `i` is lit: the chosen one while it goes, the one whose submenu
    /// is open, or the selected one.
    fn lit(&self, i: usize) -> bool {
        match self.chosen {
            Some((Row::Main(c), _)) => c == i,
            Some((Row::Sub(_), _)) => self.sub.as_ref().is_some_and(|s| s.parent == i),
            None => self.sel == Some(i) || self.sub.as_ref().is_some_and(|s| s.parent == i),
        }
    }

    /// Whether the submenu's row `j` is lit.
    fn sub_lit(&self, j: usize) -> bool {
        match self.chosen {
            Some((Row::Sub(c), _)) => c == j,
            Some((Row::Main(_), _)) => false,
            None => self.sub.as_ref().is_some_and(|s| s.sel == Some(j)),
        }
    }
}

/// Whether the pointer, moving from `from` to `to`, is heading for `r` (a submenu): it is
/// inside the triangle from where it was to the submenu's near edge. Crossing other rows on
/// the way there must not close it.
pub(crate) fn heading_to(from: (f32, f32), to: (f32, f32), r: Rect) -> bool {
    let edge = if r.x >= from.0 { r.x } else { r.right() };
    let (a, b, c) = (from, (edge, r.y - 4.0), (edge, r.y + r.h + 4.0));
    let side =
        |p: (f32, f32), q: (f32, f32)| (q.0 - p.0) * (to.1 - p.1) - (q.1 - p.1) * (to.0 - p.0);
    let (d1, d2, d3) = (side(a, b), side(b, c), side(c, a));
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

/// The shortcut as the keymap writes it (`⇧⌘C` on macOS, `Ctrl+Shift+C` elsewhere), from the
/// form the application gives (`Ctrl+Shift+C`, `C`).
fn shortcut(key: &str) -> String {
    // A capital letter reads as Shift to the parser (`Ctrl+C` would be `Ctrl+Shift+C`, `C`
    // Shift+C); the application writes Shift out when it means it.
    let key = match key.rsplit_once('+') {
        Some((mods, k)) if k.chars().count() == 1 => format!("{mods}+{}", k.to_lowercase()),
        None if key.chars().count() == 1 => key.to_lowercase(),
        _ => key.to_owned(),
    };
    KeyCombo::parse(&key).map_or(key, |c| c.to_string())
}

/// What a panel (the menu, or its submenu) shows.
struct Panel<'a> {
    title: Option<&'a str>,
    loading: Option<&'a str>,
    rows: &'a [PaletteItem],
    breaks: &'a [usize],
}

/// A panel measured: its size, and where its parts go.
struct Layout {
    w: f32,
    h: f32,
    style: TextStyle,
    key_style: TextStyle,
    title_style: TextStyle,
    row: f32,
    icon: f32,
    pad: f32,
    inset: f32,
    gap: f32,
    lead: f32,
    key_w: f32,
    title_h: f32,
    keys: Vec<Option<String>>,
}

fn layout(cx: &mut Ctx, p: &Panel) -> Layout {
    // Over a translucent material text needs presence: a medium weight, full contrast.
    let style = cx.style(metrics::FONT_LABEL).weight(500.0);
    let key_style = cx.style(metrics::FONT_LABEL).tabular();
    let title_style = cx.style(metrics::FONT_SMALL).weight(600.0);
    // A desktop menu's measure: 24 pt rows, 16 pt icons (as macOS draws its menus).
    let (row, icon, pad, inset) = (cx.u(24.0), cx.u(16.0), 5.0, 9.0);
    // Between groups: a hairline with a little room on both sides.
    let gap = 7.0;
    let any_icon = p.rows.iter().any(|r| r.icon != SUBMENU && !blank(p, r));
    let lead = if any_icon { inset + icon + 8.0 } else { inset };
    let keys: Vec<Option<String>> = p
        .rows
        .iter()
        .map(|r| {
            if r.icon == SUBMENU {
                return None;
            }
            // Isolated left to right, so `⌘C` reads the same in RTL.
            r.key
                .as_deref()
                .map(|k| format!("\u{2066}{}\u{2069}", shortcut(k)))
        })
        .collect();
    let has_sub = p.rows.iter().any(|r| r.icon == SUBMENU);
    let key_w = keys
        .iter()
        .flatten()
        .map(|k| cx.layout(k, key_style, None).width + 24.0)
        .fold(if has_sub { icon + 8.0 } else { 0.0 }, f32::max);
    let min_label = cx.u(120.0);
    let loading_w = p.loading.map_or(0.0, |l| cx.layout(l, style, None).width);
    let label_w = p
        .rows
        .iter()
        .map(|r| cx.layout(&r.label, style, None).width)
        .fold(min_label.max(loading_w), f32::max);
    let title_h = if p.title.is_some() { cx.u(24.0) } else { 0.0 };
    let loading_h = if p.loading.is_some() { row } else { 0.0 };
    let w = (lead + label_w + key_w + inset + pad * 2.0)
        .min(cx.u(320.0))
        .min((cx.w - 16.0).max(120.0));
    let h =
        title_h + loading_h + row * p.rows.len() as f32 + gap * p.breaks.len() as f32 + pad * 2.0;
    Layout {
        w,
        h,
        style,
        key_style,
        title_style,
        row,
        icon,
        pad,
        inset,
        gap,
        lead,
        key_w,
        title_h,
        keys,
    }
}

/// Whether row `r` of panel `p` shows no icon: an unchecked choice among checked ones.
fn blank(p: &Panel, r: &PaletteItem) -> bool {
    r.icon == UNCHECKED && p.rows.iter().any(|o| o.icon == Icon::Check)
}

/// The material: shadows, glass, edge. Larger and thicker than the bars: it floats over all.
fn material(cx: &mut Ctx, r: Rect) {
    let light = cx.pal.mode == Mode::Light;
    // Depth in three layers: a wide soft shadow that lifts it off the grid, a closer one
    // that gives it body, and a tight contact shadow that defines its edge.
    let radius = metrics::RADIUS_MD;
    cx.dl.shadow(r, radius, cx.pal.shadow.fade(1.2), 36.0, 18.0);
    cx.dl.shadow(r, radius, cx.pal.shadow.fade(0.8), 10.0, 4.0);
    cx.dl.shadow(r, radius, cx.pal.shadow.fade(0.6), 1.5, 0.5);
    // Mostly opaque, with a firmer edge than the bars: thumbnails behind must not show
    // through the text, and the menu must not melt into a light grid.
    let border = cx
        .pal
        .glass_border
        .with_alpha((cx.pal.glass_border.a * 1.8).min(1.0));
    cx.dl.glass(
        r,
        radius,
        cx.pal
            .surface_raised
            .with_alpha(if light { 0.92 } else { 0.86 }),
        border,
        1.0,
    );
    // Light catching the top edge of the material.
    let shine = Color::rgba(1.0, 1.0, 1.0, if light { 0.9 } else { 0.10 });
    cx.dl.line(
        (r.x + radius, r.y + 1.5),
        (r.right() - radius, r.y + 1.5),
        1.0,
        shine,
    );
}

/// Paints panel `p` measured as `l` in `r`; row `i` is lit when `is_lit(i)`, and is hit as
/// `target(i)` when `live`. Returns the rows' rectangles.
fn paint(
    cx: &mut Ctx,
    p: &Panel,
    l: &Layout,
    r: Rect,
    is_lit: impl Fn(usize) -> bool,
    target: impl Fn(usize) -> Target,
    live: bool,
) -> Vec<Rect> {
    material(cx, r);
    // Going, it takes no clicks: they reach what is under it.
    if live {
        cx.hit(r, Target::MenuBg);
    }
    cx.popovers.push(r);

    let (pad, inset, row, icon, gap) = (l.pad, l.inset, l.row, l.icon, l.gap);
    let w = r.w;
    let mut y = r.y + pad;
    if let Some(title) = p.title {
        let tr = cx.m_in(
            Rect::new(r.x + pad + inset, y, w - (pad + inset) * 2.0, l.title_h),
            r,
        );
        cx.label_fit(
            title,
            l.title_style,
            tr,
            HAlign::Start,
            cx.pal.text_muted,
            false,
        );
        y += l.title_h;
    }
    if let Some(loading) = p.loading {
        let lr = cx.m_in(
            Rect::new(r.x + pad + l.lead, y, w - pad * 2.0 - l.lead - inset, row),
            r,
        );
        cx.label_fit(
            loading,
            l.style,
            lr,
            HAlign::Start,
            cx.pal.text_muted,
            false,
        );
        y += row;
    }
    let mut rects = Vec::with_capacity(p.rows.len());
    for (i, (it, key)) in p.rows.iter().zip(&l.keys).enumerate() {
        if p.breaks.contains(&i) {
            let ly = (y + gap * 0.5).round() + 0.5;
            let sep = cx.pal.separator;
            cx.dl.line(
                (r.x + pad + inset, ly),
                (r.right() - pad - inset, ly),
                1.0,
                sep.with_alpha((sep.a * 1.4).min(1.0)),
            );
            y += gap;
        }
        let rr = Rect::new(r.x + pad, y, w - pad * 2.0, row);
        rects.push(rr);
        y += row;
        let enabled = it.unavailable.is_none();
        let lit = enabled && is_lit(i);
        let danger = it.icon == DESTRUCTIVE;
        if lit {
            let fill = if danger {
                cx.pal.danger_fill
            } else {
                cx.pal.accent_fill
            };
            cx.dl.rect(rr, 5.0, fill);
        }
        let (fg, glyph) = if !enabled {
            let c = cx.pal.text_faint.with_alpha(0.55);
            (c, c)
        } else if lit {
            (cx.pal.accent_text, cx.pal.accent_text)
        } else if danger {
            (cx.pal.danger, cx.pal.danger)
        } else {
            // Icons as strong as the text: muted ones made the rows look washed out.
            (cx.pal.text, cx.pal.text)
        };
        if it.icon != SUBMENU && !blank(p, it) {
            let ir = cx.m_in(Rect::new(rr.x + inset, rr.y, icon, rr.h), rr);
            let ir = ir.centered(icon, icon);
            if it.icon == Icon::Zynk && enabled {
                // Zynk's mark keeps its own colours, at full strength; on the lit row a
                // light ring keeps its blue tile apart from the highlight.
                draw_icon(
                    cx.dl,
                    it.icon,
                    ir,
                    Color::rgba(1.0, 1.0, 1.0, 1.0),
                    1.5,
                    false,
                );
                if lit {
                    let s = icon * (20.0 / 24.0);
                    let ring = ir.centered(s, s).inset(-1.0);
                    let c = cx.pal.accent_text.with_alpha(0.7);
                    cx.dl.stroke_rect(ring, s * 0.22 + 1.0, c, 1.0);
                }
            } else {
                // Strokes as light as the label's: heavier ones outweighed the words.
                draw_icon(cx.dl, it.icon, ir, glyph, 1.4, false);
            }
        }
        let kw = if key.is_some() || it.icon == SUBMENU {
            l.key_w
        } else {
            0.0
        };
        let lr = Rect::new(
            rr.x + l.lead,
            rr.y,
            (rr.w - l.lead - kw - inset).max(0.0),
            rr.h,
        );
        let lr = cx.m_in(lr, rr);
        // A row that leads on has its chevron; the "…" the palette's list shows goes.
        let label = if it.icon == SUBMENU {
            it.label.trim_end_matches(['…', '.']).trim_end()
        } else {
            it.label.as_str()
        };
        let label = cx.name(label);
        cx.label_fit(&label, l.style, lr, HAlign::Start, fg, false);
        if let Some(k) = key {
            let kr = cx.m_in(Rect::new(rr.right() - kw - inset, rr.y, kw, rr.h), rr);
            let kc = if lit {
                cx.pal.accent_text.with_alpha(0.8)
            } else {
                cx.pal.text_muted
            };
            cx.label_fit(k, l.key_style, kr, HAlign::End, kc, false);
        } else if it.icon == SUBMENU {
            let s = cx.u(12.0);
            let cr = cx.m_in(Rect::new(rr.right() - inset - s, rr.y, s, rr.h), rr);
            let cr = cr.centered(s, s);
            let cc = if lit { glyph } else { cx.pal.text_muted };
            draw_icon(cx.dl, SUBMENU, cr, cc, 1.75, cx.rtl);
        }
        if live {
            cx.hit(rr, target(i));
        }
    }
    rects
}

/// Draws the menu, and its submenu beside the row it came from; returns the submenu's
/// rectangle, and whether the menu is still fading in, or once a row was chosen, whether it
/// is still going (when it is not, it is gone).
pub(crate) fn draw(cx: &mut Ctx, m: &ContextMenu, now: f64) -> (Option<Rect>, bool) {
    let main = Panel {
        title: m.title.as_deref(),
        loading: None,
        rows: &m.rows,
        breaks: &m.breaks,
    };
    let l = layout(cx, &main);
    let (w, h) = (l.w, l.h);

    // At the pointer, opening towards the end side and downwards; flipped where it would not
    // fit, then kept on screen.
    let (px, py) = m.at;
    let mut x = if cx.rtl { px - w } else { px };
    if x + w > cx.w - 8.0 {
        x = px - w;
    } else if x < 8.0 {
        x = px;
    }
    let x = x.clamp(8.0, (cx.w - w - 8.0).max(8.0));
    let y = if py + h > cx.h - 8.0 { py - h } else { py };
    let y = y.clamp(8.0, (cx.h - h - 8.0).max(8.0));

    // Fades in and settles from a few pixels away from where it opens.
    let t = (((now - m.opened) / OPEN_SECS) as f32).clamp(0.0, 1.0);
    let ease = 1.0 - (1.0 - t).powi(4);
    // Once a row is chosen it holds, lit, then fades where it is.
    let leaving = m.chosen.map(|(_, at)| {
        let t = (((now - at - CHOSEN_HOLD_SECS) / CLOSE_SECS) as f32).clamp(0.0, 1.0);
        ((1.0 - t).powi(2), t < 1.0)
    });
    let alpha = ease * leaving.map_or(1.0, |(a, _)| a);
    // It settles towards where it was opened from, never away from it.
    let slide = if y < py { 4.0 } else { -4.0 } * (1.0 - ease);
    let r = Rect::new(x.round(), (y + slide).round(), w, h);
    let live = m.chosen.is_none();
    cx.dl.push_opacity(alpha);
    let rows = paint(cx, &main, &l, r, |i| m.lit(i), Target::MenuItem, live);

    // The submenu: no animation (it follows the pointer row to row), its first row level with
    // the row it came from, on the end side unless there is no room there.
    let mut sub_rect = None;
    if let Some(s) = &m.sub
        && let Some(&pr) = rows.get(s.parent)
    {
        let panel = Panel {
            title: None,
            loading: s.loading.as_deref(),
            rows: &s.rows,
            breaks: &s.breaks,
        };
        let mut sl = layout(cx, &panel);
        // Its width holds as rows come and go (the applications Open with finds), and starts
        // wide while they are being looked for, so it does not jump under the pointer.
        let floor = if s.loading.is_some() {
            cx.u(220.0)
        } else {
            0.0
        };
        sl.w =
            sl.w.max(floor)
                .max(s.rect.map_or(0.0, |r| r.w))
                .min((cx.w - 16.0).max(120.0));
        let overlap = 3.0;
        let end = r.right() - overlap;
        let start = r.x + overlap - sl.w;
        let fits_end = end + sl.w <= cx.w - 8.0;
        let fits_start = start >= 8.0;
        let sx = match (cx.rtl, fits_end, fits_start) {
            (false, true, _) | (true, true, false) => end,
            _ => start,
        };
        let sx = sx.clamp(8.0, (cx.w - sl.w - 8.0).max(8.0));
        let sy = (pr.y - sl.pad).clamp(8.0, (cx.h - sl.h - 8.0).max(8.0));
        let sr = Rect::new(sx.round(), sy.round(), sl.w, sl.h);
        paint(
            cx,
            &panel,
            &sl,
            sr,
            |j| m.sub_lit(j),
            Target::MenuSubItem,
            live,
        );
        sub_rect = Some(sr);
    }
    cx.dl.pop_opacity();
    (sub_rect, leaving.map_or(t < 1.0, |(_, going)| going))
}
