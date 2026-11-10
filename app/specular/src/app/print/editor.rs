//! The free layout editor of the print dialog: the page preview with its image areas drawn
//! over it, a toolbar (Add area, Remove area, Save layout…, Delete layout) and the maths of
//! pointer and keyboard edits ([`specular_print::free`] does the layout maths).
//!
//! - **Pointer:** drag an area to move it, its handles to resize it, Shift+drag a filled
//!   photo to move its crop. Edges snap to the margins, the centre, the other areas and,
//!   with the grid on, the grid; the lines snapped to are drawn while dragging.
//! - **Keyboard** (the page focused): Space selects the next area, arrows move it, Shift
//!   with arrows resizes it, Alt with arrows moves a filled photo's crop, Delete removes it.
//! - Each area shows its printed size in the page's units and the photo's effective
//!   resolution, flagged below [`free::LOW_DPI`].
//!
//! The page itself is never mirrored in a right-to-left language: it prints as drawn.

use specular_gfx::{Align, DrawList, Icon, Rect, Text, TextStyle, draw_icon};
use specular_i18n::t;
use specular_print::free::{self, FreeLayout, Guide, Handle, Placement, Snap};
use specular_print::layout::{RectMm, Unit};
use specular_theme::metrics::{FONT_SMALL, RADIUS_SM};
use specular_theme::{Color, Palette};

use super::Hit;

/// How near (logical px) an edge must come to a line to snap.
const SNAP_PX: f32 = 6.0;
/// Half the size of a resize handle (logical px), and how far from it a press still takes it.
const HANDLE_PX: f32 = 4.0;
const HANDLE_REACH: f32 = 7.0;
/// The toolbar's height over the page.
pub(super) const TOOLBAR_H: f32 = 30.0;

/// What edges snap to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::app) enum SnapMode {
    Off,
    /// The margins, the centre and the other areas.
    #[default]
    Guides,
    /// Those, and a grid where none is near.
    Grid,
}

impl SnapMode {
    pub fn next(self, d: i32) -> SnapMode {
        let all = [SnapMode::Off, SnapMode::Guides, SnapMode::Grid];
        let i = all.iter().position(|m| *m == self).unwrap_or(1) as i32;
        all[(i + d).rem_euclid(3) as usize]
    }

    /// The snapping of a drag on a page drawn at `px_per_mm`, grid steps in `unit`.
    pub fn snap(self, px_per_mm: f64, unit: Unit) -> Snap {
        match self {
            SnapMode::Off => Snap::OFF,
            SnapMode::Guides | SnapMode::Grid => Snap {
                threshold: f64::from(SNAP_PX) / px_per_mm.max(1e-6),
                guides: true,
                grid: (self == SnapMode::Grid).then(|| grid_step(unit)),
            },
        }
    }
}

/// The grid's step (mm): 5 mm, or a quarter inch.
pub(super) fn grid_step(unit: Unit) -> f64 {
    match unit {
        Unit::In => specular_print::layout::MM_PER_IN / 4.0,
        Unit::Mm | Unit::Cm => 5.0,
    }
}

/// What a drag holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Grip {
    Edge(Handle),
    /// A filled photo's crop.
    Pan,
}

/// A drag in progress.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Drag {
    pub area: usize,
    pub grip: Grip,
    /// The area's rectangle and pan when it began.
    pub start: RectMm,
    pub pan: (f64, f64),
    /// Where the pointer was (logical px).
    pub from: (f32, f32),
}

/// One area as the overlay draws it.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct AreaView {
    /// Its place in the order photos fill the areas (1 first).
    pub n: usize,
    pub rect: RectMm,
    /// "102 × 152 mm · 300 dpi".
    pub label: String,
    /// "300 dpi", where the label does not fit.
    pub short: String,
    /// The screen reader's name of it.
    pub name: String,
    /// Under [`free::LOW_DPI`].
    pub low: bool,
    pub selected: bool,
}

/// The editor as the draw function sees it.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct EditorView {
    pub sheet: (f64, f64),
    pub printable: RectMm,
    pub areas: Vec<AreaView>,
    pub guides: Vec<Guide>,
    /// The grid's step (mm) while dragging with the grid on.
    pub grid: Option<f64>,
    /// Under the page: borderless printing's notice.
    pub notice: Option<String>,
    /// The toolbar: what each button does, its label, whether it can.
    pub tools: Vec<(Hit, String, bool)>,
}

/// `mm` as the page's units show it: "102 mm", "4.25 in".
pub(super) fn length(mm: f64, unit: Unit) -> String {
    format!(
        "{} {}",
        unit.format(mm),
        specular_i18n::unit_symbol(unit.symbol())
    )
}

/// An area's label and screen-reader name: its printed size and, with a photo, the photo's
/// effective resolution (flagged when low).
pub(super) fn area_view(
    n: usize,
    rect: RectMm,
    photo: Option<((u32, u32), RectMm)>,
    unit: Unit,
    selected: bool,
) -> AreaView {
    let size = t!(
        "print-area-size",
        width = unit.format(rect.w),
        height = unit.format(rect.h),
        unit = specular_i18n::unit_symbol(unit.symbol())
    );
    let (short, dpi, low) = match photo {
        Some((px, image)) => {
            let d = free::effective_dpi(px, &image).round();
            let low = d < free::LOW_DPI;
            let short = t!("print-area-dpi", dpi = d as i64);
            let text = if low {
                t!("print-area-dpi-low", dpi = d as i64)
            } else {
                short.clone()
            };
            (short, text, low)
        }
        None => (t!("print-area-empty"), t!("print-area-empty"), false),
    };
    AreaView {
        n,
        rect,
        label: specular_i18n::dot_join(&[size.clone(), short.clone()]),
        short,
        name: t!("print-area-name", n = n as i64, size = size, detail = dpi),
        low,
        selected,
    }
}

/// Page coordinates (mm) of a point on the page drawn at `page`.
#[cfg(test)]
pub(super) fn to_mm(page: Rect, sheet: (f64, f64), p: (f32, f32)) -> (f64, f64) {
    let s = f64::from(page.w) / sheet.0.max(1e-6);
    (f64::from(p.0 - page.x) / s, f64::from(p.1 - page.y) / s)
}

/// An area's rectangle on screen.
pub(super) fn to_px(page: Rect, sheet: (f64, f64), r: &RectMm) -> Rect {
    let s = f64::from(page.w) / sheet.0.max(1e-6);
    Rect::new(
        page.x + (r.x * s) as f32,
        page.y + (r.y * s) as f32,
        (r.w * s) as f32,
        (r.h * s) as f32,
    )
}

/// What a press at `p` takes: the selected area's handles first, then the topmost area
/// under it (its body).
pub(super) fn grip_at(
    layout: &FreeLayout,
    selected: Option<usize>,
    page: Rect,
    p: (f32, f32),
) -> Option<(usize, Handle)> {
    let sheet = layout.sheet_mm();
    if let Some(i) = selected
        && let Some(a) = layout.areas.get(i)
    {
        let r = to_px(page, sheet, &a.rect);
        for h in Handle::RESIZE {
            let (fx, fy) = h.at();
            let (hx, hy) = (r.x + r.w * fx as f32, r.y + r.h * fy as f32);
            if (p.0 - hx).abs() <= HANDLE_REACH && (p.1 - hy).abs() <= HANDLE_REACH {
                return Some((i, h));
            }
        }
    }
    layout
        .areas
        .iter()
        .enumerate()
        .rev()
        .find(|(_, a)| to_px(page, sheet, &a.rect).contains(p))
        .map(|(i, _)| (i, Handle::Move))
}

/// Applies a drag at pointer `p` to `layout`; returns the guides snapped to.
pub(super) fn drag_to(
    layout: &mut FreeLayout,
    d: &Drag,
    page: Rect,
    p: (f32, f32),
    snap: SnapMode,
    unit: Unit,
    photo: Option<(u32, u32)>,
) -> Vec<Guide> {
    let sheet = layout.sheet_mm();
    let px_per_mm = f64::from(page.w) / sheet.0.max(1e-6);
    let delta = (
        f64::from(p.0 - d.from.0) / px_per_mm,
        f64::from(p.1 - d.from.1) / px_per_mm,
    );
    match d.grip {
        Grip::Edge(h) => {
            let (r, g) = free::drag(
                layout,
                d.area,
                d.start,
                h,
                delta,
                &snap.snap(px_per_mm, unit),
            );
            if let Some(a) = layout.areas.get_mut(d.area) {
                a.rect = r;
            }
            g
        }
        Grip::Pan => {
            if let (Some(a), Some(px)) = (layout.areas.get_mut(d.area), photo) {
                let start = free::Area {
                    rect: d.start,
                    placement: Placement::Fill,
                    pan: d.pan,
                };
                a.pan = free::pan_by(&start, px, delta);
            }
            Vec::new()
        }
    }
}

/// A key on the focused page: moves (arrows), resizes (Shift) or pans (Alt) the selected
/// area by one `step` (mm). True when something changed.
pub(super) fn nudge(
    layout: &mut FreeLayout,
    area: usize,
    (dx, dy): (f64, f64),
    resize: bool,
    pan: Option<(u32, u32)>,
    step: f64,
) -> bool {
    let Some(a) = layout.areas.get(area).copied() else {
        return false;
    };
    let before = a;
    let delta = (dx * step, dy * step);
    if let Some(px) = pan {
        if a.placement != Placement::Fill {
            return false;
        }
        // The crop moves the other way from the photo: an arrow shows more of that side.
        let p = free::pan_by(&a, px, (-delta.0, -delta.1));
        layout.areas[area].pan = p;
    } else {
        let h = if resize { Handle::SE } else { Handle::Move };
        let (r, _) = free::drag(layout, area, a.rect, h, delta, &Snap::OFF);
        layout.areas[area].rect = r;
    }
    layout.areas[area] != before
}

/// Draws the toolbar in `bar` (start-aligned buttons); returns their rectangles.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_toolbar(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    v: &EditorView,
    bar: Rect,
    m: &dyn Fn(Rect) -> Rect,
    focus: Option<Hit>,
    ts: f32,
) -> Vec<(Rect, Hit)> {
    let st = TextStyle::new(FONT_SMALL * ts).weight(550.0);
    let mut x = bar.x;
    let mut out = Vec::new();
    for (hit, label, enabled) in &v.tools {
        let b = text.layout(label, st, None, Align::Start);
        let icon = matches!(hit, Hit::AddArea);
        let bw = b.width + 20.0 + if icon { 16.0 } else { 0.0 };
        if x + bw > bar.right() {
            break;
        }
        let r = Rect::new(x, bar.y + 2.0, bw, bar.h - 4.0);
        x += bw + 6.0;
        let rm = m(r);
        let danger = matches!(hit, Hit::ConfirmDelete);
        let (fill, ink) = if danger {
            (pal.danger_fill, pal.accent_text)
        } else {
            (pal.surface_raised, pal.text)
        };
        let ink = if *enabled { ink } else { ink.with_alpha(0.45) };
        dl.rect_border(rm, RADIUS_SM, fill, pal.separator, 1.0);
        if focus == Some(*hit) {
            dl.rect_border(
                rm.inset(-3.0),
                RADIUS_SM + 3.0,
                Color::TRANSPARENT,
                pal.focus_ring,
                2.0,
            );
        }
        let mut tx = rm.x + 10.0;
        if icon {
            let ir = Rect::new(rm.x + 7.0, rm.y + (rm.h - 12.0) * 0.5, 12.0, 12.0);
            let ir = if m(Rect::new(0.0, 0.0, 1.0, 1.0)).x != 0.0 {
                Rect::new(rm.right() - 19.0, ir.y, 12.0, 12.0)
            } else {
                tx += 16.0;
                ir
            };
            draw_icon(dl, Icon::Plus, ir, ink, 1.6, false);
        }
        text.draw(dl, &b, tx, rm.y + (rm.h - b.height) * 0.5, ink);
        out.push((rm, *hit));
    }
    out
}

/// Draws the areas, guides and margins over the page preview at `page`.
pub(super) fn draw_overlay(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    v: &EditorView,
    page: Rect,
    focused: bool,
    ts: f32,
) {
    let px = |r: &RectMm| to_px(page, v.sheet, r);
    let s = f64::from(page.w) / v.sheet.0.max(1e-6);
    // The margins: a faint line inside the sheet.
    let pr = px(&v.printable);
    if pr.w < page.w - 0.5 {
        dl.stroke_rect(pr, 0.0, Color::rgba(0.2, 0.45, 0.85, 0.35), 1.0);
    }
    if let Some(g) = v.grid {
        let c = Color::rgba(0.2, 0.45, 0.85, 0.12);
        let mut x = g;
        while x < v.sheet.0 {
            let sx = page.x + (x * s) as f32;
            dl.line((sx, page.y), (sx, page.bottom()), 1.0, c);
            x += g;
        }
        let mut y = g;
        while y < v.sheet.1 {
            let sy = page.y + (y * s) as f32;
            dl.line((page.x, sy), (page.right(), sy), 1.0, c);
            y += g;
        }
    }
    let label_st = TextStyle::new(FONT_SMALL * ts * 0.9).weight(550.0);
    let blue = Color::rgba(0.16, 0.42, 0.86, 1.0);
    for a in &v.areas {
        let r = px(&a.rect);
        let (c, w) = if a.selected {
            (blue, 2.0)
        } else {
            (Color::rgba(0.16, 0.42, 0.86, 0.6), 1.0)
        };
        dl.stroke_rect(r, 0.0, c, w);
        // The label chip at the area's bottom left (the page is not mirrored).
        // One line: the size and resolution, or the resolution alone when that is too wide
        // (the warning mark says it is low).
        let mut b = text.layout(&a.label, label_st, None, Align::Start);
        if b.width > r.w - 12.0 {
            b = text.layout(&a.short, label_st, None, Align::Start);
        }
        if r.w > 40.0 && r.h > b.height + 8.0 {
            let chip = Rect::new(
                r.x + 4.0,
                r.bottom() - b.height - 8.0,
                b.width + 8.0,
                b.height + 4.0,
            );
            let (bg, ink) = if a.low {
                (pal.danger_fill, pal.accent_text)
            } else {
                (Color::rgba(0.0, 0.0, 0.0, 0.62), Color::WHITE)
            };
            dl.rect(chip, RADIUS_SM, bg);
            dl.push_clip(chip);
            text.draw(dl, &b, chip.x + 4.0, chip.y + 2.0, ink);
            dl.pop_clip();
            if a.low && r.h > b.height + 30.0 && r.w > 30.0 {
                let ir = Rect::new(r.x + 6.0, r.y + 6.0, 16.0, 16.0);
                dl.rect(ir.inset(-2.0), RADIUS_SM, pal.danger_fill);
                draw_icon(dl, Icon::Alert, ir, pal.accent_text, 1.6, false);
            }
        }
        if a.selected {
            for h in Handle::RESIZE {
                let (fx, fy) = h.at();
                let (hx, hy) = (r.x + r.w * fx as f32, r.y + r.h * fy as f32);
                let hr = Rect::new(
                    hx - HANDLE_PX,
                    hy - HANDLE_PX,
                    2.0 * HANDLE_PX,
                    2.0 * HANDLE_PX,
                );
                dl.rect_border(hr, 1.0, Color::WHITE, blue, 1.5);
            }
        }
    }
    let magenta = Color::rgba(0.9, 0.1, 0.6, 0.9);
    for g in &v.guides {
        match *g {
            Guide::X(x) => {
                let sx = page.x + (x * s) as f32;
                dl.line((sx, page.y), (sx, page.bottom()), 1.0, magenta);
            }
            Guide::Y(y) => {
                let sy = page.y + (y * s) as f32;
                dl.line((page.x, sy), (page.right(), sy), 1.0, magenta);
            }
        }
    }
    if focused {
        dl.rect_border(
            page.inset(-4.0),
            RADIUS_SM,
            Color::TRANSPARENT,
            pal.focus_ring,
            2.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> FreeLayout {
        let mut l = FreeLayout::default();
        l.grid(2, 1, Placement::Fill);
        l
    }

    #[test]
    fn presses_take_handles_then_areas() {
        let l = layout();
        // A4 drawn 210 px wide: 1 px per mm.
        let page = Rect::new(100.0, 50.0, 210.0, 297.0);
        let a0 = l.areas[0].rect;
        let inside = (100.0 + a0.x as f32 + 20.0, 50.0 + a0.y as f32 + 20.0);
        assert_eq!(grip_at(&l, None, page, inside), Some((0, Handle::Move)));
        // The bottom-right corner of the selected area is its handle.
        let corner = (
            100.0 + a0.right() as f32 + 3.0,
            50.0 + a0.bottom() as f32 - 2.0,
        );
        assert_eq!(grip_at(&l, Some(0), page, corner), Some((0, Handle::SE)));
        assert_eq!(grip_at(&l, None, page, (102.0, 52.0)), None);
        let (x, y) = to_mm(page, l.sheet_mm(), (100.0 + 30.0, 50.0 + 40.0));
        assert!((x - 30.0).abs() < 1e-4 && (y - 40.0).abs() < 1e-4);
    }

    #[test]
    fn dragging_and_keys_edit_the_layout() {
        let mut l = FreeLayout {
            areas: vec![
                free::Area::new(RectMm::new(20.0, 200.0, 60.0, 40.0), Placement::Fill),
                free::Area::new(RectMm::new(50.0, 50.0, 60.0, 40.0), Placement::Fill),
            ],
            ..FreeLayout::default()
        };
        let page = Rect::new(0.0, 0.0, 420.0, 594.0); // 2 px per mm
        let start = l.areas[1].rect;
        let d = Drag {
            area: 1,
            grip: Grip::Edge(Handle::Move),
            start,
            pan: (0.0, 0.0),
            from: (300.0, 300.0),
        };
        // 13 px = 6.5 mm down: nothing near; with the grid on, the nearest 5 mm.
        let g = drag_to(
            &mut l,
            &d,
            page,
            (300.0, 313.0),
            SnapMode::Grid,
            Unit::Mm,
            None,
        );
        assert!(!g.iter().any(|g| matches!(g, Guide::Y(_))), "{g:?}");
        assert!(
            (l.areas[1].rect.y % 5.0).abs() < 1e-6,
            "{:?}",
            l.areas[1].rect
        );
        // Panning a filled photo.
        let d = Drag {
            grip: Grip::Pan,
            ..d
        };
        drag_to(
            &mut l,
            &d,
            page,
            (300.0, 280.0),
            SnapMode::Off,
            Unit::Mm,
            Some((2000, 3000)),
        );
        assert!(l.areas[1].pan.1 > 0.0, "{:?}", l.areas[1].pan);
        // Keys: move, resize, pan.
        let before = l.areas[0].rect;
        assert!(nudge(&mut l, 0, (0.0, 1.0), false, None, 1.0));
        assert!((l.areas[0].rect.y - before.y - 1.0).abs() < 1e-6);
        assert!(nudge(&mut l, 0, (-1.0, 0.0), true, None, 1.0));
        assert!((l.areas[0].rect.w - before.w + 1.0).abs() < 1e-6);
        assert!(nudge(&mut l, 0, (1.0, 0.0), false, Some((3000, 1000)), 2.0));
        l.areas[0].placement = Placement::Fit;
        assert!(!nudge(
            &mut l,
            0,
            (1.0, 0.0),
            false,
            Some((3000, 1000)),
            2.0
        ));
    }

    #[test]
    fn labels_say_the_size_and_flag_low_resolution() {
        specular_i18n::I18n::with_thread_language(specular_i18n::langid!("en-US"), || {
            let r = RectMm::new(0.0, 0.0, 101.6, 152.4);
            let plain = |s: &str| s.replace(['\u{2068}', '\u{2069}'], "");
            let ok = area_view(1, r, Some(((1200, 1800), r)), Unit::In, false);
            let ok = AreaView {
                label: plain(&ok.label),
                ..ok
            };
            assert!(ok.label.contains("4 × 6"), "{}", ok.label);
            assert!(ok.label.contains("300 dpi") && !ok.low, "{}", ok.label);
            let low = area_view(2, r, Some(((600, 900), r)), Unit::Mm, true);
            let low = AreaView {
                label: plain(&low.label),
                ..low
            };
            assert!(low.low && low.label.contains("150"), "{}", low.label);
            assert!(low.label.contains("101.6 × 152.4"), "{}", low.label);
            let empty = area_view(3, r, None, Unit::Mm, false);
            assert!(!empty.low);
            assert!(empty.name.contains('3'));
        });
    }
}
