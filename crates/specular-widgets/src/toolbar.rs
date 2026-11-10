//! The floating toolbar (bottom centre), its zoom presets popover and the edge arrows.

use std::sync::Arc;

use specular_gfx::{Icon, Rect, TextBlock, TextStyle, draw_icon};
use specular_i18n::{I18n, t};
use specular_input::Action;
use specular_settings::{SettingEnum, ToolbarItem};
use specular_theme::metrics;

use crate::ctx::{Ctx, FieldGeom, HAlign, Target, digit_bytes};
use crate::line_edit::LineEdit;
use crate::{Chrome, ViewerState};

/// The action a toolbar item runs. The zoom readout opens the presets, which are zoom
/// actions, so it follows the zoom rule.
pub(crate) fn item_action(item: ToolbarItem, st: &ViewerState) -> Option<Action> {
    match item {
        ToolbarItem::ZoomLevel => Some(Action::ZoomFit),
        _ => item_label(item, st).1,
    }
}

/// Tooltip label and the action whose shortcut is shown.
pub(crate) fn item_label(item: ToolbarItem, st: &ViewerState) -> (String, Option<Action>) {
    use ToolbarItem as T;
    match item {
        T::Open => (t!("toolbar-open"), Some(Action::Open)),
        T::Prev => (t!("toolbar-prev"), Some(Action::PrevImage)),
        T::Next => (t!("toolbar-next"), Some(Action::NextImage)),
        T::ZoomOut => (t!("toolbar-zoom-out"), Some(Action::ZoomOut)),
        T::ZoomLevel => (t!("toolbar-zoom-level"), None),
        T::ZoomIn => (t!("toolbar-zoom-in"), Some(Action::ZoomIn)),
        T::Fit if st.at_fit => (t!("toolbar-zoom-100"), Some(Action::ZoomTo(100.0))),
        T::Fit => (t!("toolbar-zoom-fit"), Some(Action::ZoomFit)),
        T::RotateLeft => (t!("toolbar-rotate-left"), Some(Action::RotateLeft)),
        T::RotateRight => (t!("toolbar-rotate-right"), Some(Action::RotateRight)),
        T::Info => (t!("toolbar-info"), Some(Action::ToggleInfo)),
        T::Copy => (t!("toolbar-copy"), Some(Action::CopyImage)),
        T::Delete => (t!("toolbar-delete"), Some(Action::MoveToTrash)),
        T::Palette => (t!("toolbar-palette"), Some(Action::Palette)),
        T::Settings => (t!("toolbar-settings"), Some(Action::Settings)),
        T::Fullscreen if st.fullscreen => (
            t!("toolbar-fullscreen-exit"),
            Some(Action::ToggleFullscreen),
        ),
        T::Fullscreen => (
            t!("toolbar-fullscreen-enter"),
            Some(Action::ToggleFullscreen),
        ),
        T::Slideshow => (t!("toolbar-slideshow"), Some(Action::Slideshow)),
        T::Mpr if st.actions.mpr => (t!("toolbar-mpr-close"), Some(Action::Mpr)),
        T::Mpr => (t!("toolbar-mpr"), Some(Action::Mpr)),
        T::ExportSlideshow => (
            t!("palette-export-slideshow"),
            Some(Action::ExportSlideshow),
        ),
        T::Discord => (t!("toolbar-discord"), Some(Action::JoinDiscord)),
        T::Spacer | T::Separator => (item_name(item), None),
    }
}

fn zoom_text(st: &ViewerState) -> String {
    zoom_label(st.zoom, st.at_fit)
}

pub(crate) fn zoom_label(zoom: f32, at_fit: bool) -> String {
    let loc = I18n::global().localizer();
    let digits = if zoom < 0.1 { 1 } else { 0 };
    let pct = loc.percent(zoom as f64, digits);
    if at_fit {
        t!("readout-zoom-fit", zoom = pct)
    } else {
        t!("readout-zoom", zoom = pct)
    }
}

/// Inner padding of the bar, and the gap between its items.
pub(crate) const PAD: f32 = 6.0;
pub(crate) const GAP: f32 = 2.0;
/// A separator's slot (a hairline in its middle).
pub(crate) const SEPARATOR_W: f32 = 9.0;
/// A spacer's width when the bar has no room to give it.
pub(crate) const SPACER_MIN: f32 = 12.0;
/// The widest spacers stretch the bar.
pub(crate) const MAX_W: f32 = 960.0;

/// Where the items of a bar go: each item's x (from the bar's left edge, left to right) and
/// width, and the bar's width.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BarLayout {
    pub slots: Vec<(f32, f32)>,
    pub width: f32,
}

/// Lays out a bar: `lead` is the position readout's width (0: none), `items` each item's
/// natural width and whether it is a spacer. Without spacers the bar is as wide as its items;
/// with spacers it takes `room` (when that is wider), the spacers sharing the rest equally.
pub(crate) fn bar_layout(lead: f32, items: &[(f32, bool)], room: f32) -> BarLayout {
    let natural = items.iter().map(|(w, _)| w).sum::<f32>()
        + GAP * items.len().saturating_sub(1) as f32
        + PAD * 2.0
        + lead;
    let spacers = items.iter().filter(|(_, s)| *s).count();
    let (width, extra) = if spacers > 0 && room > natural {
        (room, (room - natural) / spacers as f32)
    } else {
        (natural, 0.0)
    };
    let mut x = PAD + lead;
    let mut slots = Vec::with_capacity(items.len());
    for (w, spacer) in items {
        let w = if *spacer { w + extra } else { *w };
        slots.push((x, w));
        x += w + GAP;
    }
    BarLayout { slots, width }
}

/// The room spacers may stretch the bar to in a window `w` wide.
pub(crate) fn room(w: f32) -> f32 {
    (w - metrics::TOOLBAR_MARGIN * 2.0).min(MAX_W)
}

/// How many items, from the start, stay on a bar that may be `room` wide: all of them when
/// they fit, else as many as fit before a More button (which holds the rest). A spacer or
/// separator never ends the bar before the More button.
pub(crate) fn fold(lead: f32, items: &[ToolbarItem], widths: &[(f32, bool)], room: f32) -> usize {
    let n = widths.len();
    if bar_layout(lead, widths, room).width <= room + 0.5 {
        return n;
    }
    let with_more = |k: usize| {
        let mut w = widths[..k].to_vec();
        w.push((metrics::BUTTON, false));
        bar_layout(lead, &w, room).width
    };
    let mut k = (0..n)
        .rev()
        .find(|&k| with_more(k) <= room + 0.5)
        .unwrap_or(0);
    while k > 0 && items[k - 1].is_layout() {
        k -= 1;
    }
    k
}

/// The readouts and the layout of one toolbar, measured at full size.
pub(crate) struct Measured {
    /// The position readout and the width its text keeps.
    pos: Option<(Arc<TextBlock>, f32)>,
    zoom: Option<Arc<TextBlock>>,
    /// Width of the position readout with its margins (0: none).
    lead: f32,
    /// How many items are on the bar; the rest are in the More menu, whose button then has
    /// the slot after them.
    pub shown: usize,
    pub layout: BarLayout,
}

/// The position readout's text ("12 / 348", or "4–5 / 20" for a two-page spread).
fn position_text(st: &ViewerState) -> String {
    match st.spread {
        Some(last) => t!(
            "readout-position-spread",
            first = (st.index.min(last) + 1) as i64,
            last = (st.index.max(last) + 1) as i64,
            count = st.count as i64
        ),
        None => t!(
            "readout-position",
            index = (st.index + 1) as i64,
            count = st.count as i64
        ),
    }
}

/// Lays out a toolbar holding `items` for a window whose spacers may take `room`. With
/// `overflow`, the items that don't fit in `room` go to the More menu (see [`fold`]); without
/// it (the preview in Settings, which scales down instead) the bar holds them all.
pub(crate) fn measure(
    cx: &mut Ctx,
    st: &ViewerState,
    items: &[ToolbarItem],
    room: f32,
    overflow: bool,
) -> Measured {
    use ToolbarItem as T;
    let readout_style = cx.style(metrics::FONT_LABEL).tabular().weight(500.0);
    let pos = (st.count > 1).then(|| {
        let b = cx.layout(&position_text(st), readout_style, None);
        // As wide as the widest index for this folder (digits are tabular), so stepping from
        // image 9 to 10 doesn't resize the bar.
        let widest = if st.spread.is_some() {
            t!(
                "readout-position-spread",
                first = st.count as i64,
                last = st.count as i64,
                count = st.count as i64
            )
        } else {
            t!(
                "readout-position",
                index = st.count as i64,
                count = st.count as i64
            )
        };
        let w = b.width.max(cx.layout(&widest, readout_style, None).width);
        (b, w)
    });
    let zoom = items.contains(&T::ZoomLevel).then(|| {
        // Nothing open: no percentage to show.
        let z = if st.has_image {
            zoom_text(st)
        } else {
            "—".into()
        };
        cx.layout(&z, readout_style, None)
    });
    // The readout keeps the width of its widest usual text, so the bar doesn't change size as
    // the percentage changes. Digits are tabular, so 8s stand in for any digit.
    let zoom_min_w = zoom.as_ref().map_or(0.0, |_| {
        [(0.088, true), (8.88, true), (88.88, false)]
            .into_iter()
            .map(|(z, fit)| cx.layout(&zoom_label(z, fit), readout_style, None).width)
            .fold(0.0, f32::max)
    });
    let widths: Vec<(f32, bool)> = items
        .iter()
        .map(|it| match it {
            T::ZoomLevel => (
                zoom.as_ref()
                    .map_or(60.0, |b| (b.width.max(zoom_min_w) + 20.0).max(64.0)),
                false,
            ),
            T::Spacer => (SPACER_MIN, true),
            T::Separator => (SEPARATOR_W, false),
            _ => (metrics::BUTTON, false),
        })
        .collect();
    let lead = pos.as_ref().map_or(0.0, |(_, w)| w + 22.0);
    let shown = if overflow {
        fold(lead, items, &widths, room)
    } else {
        items.len()
    };
    let layout = if shown < items.len() {
        let mut w = widths[..shown].to_vec();
        w.push((metrics::BUTTON, false));
        bar_layout(lead, &w, room)
    } else {
        bar_layout(lead, &widths, room)
    };
    Measured {
        pos,
        zoom,
        lead,
        shown,
        layout,
    }
}

/// A button's icon, whether it mirrors in right-to-left layouts, and whether it shows as on.
/// `None` for the zoom readout, spacers and separators.
pub(crate) fn item_icon(
    it: ToolbarItem,
    st: &ViewerState,
    info_open: bool,
    rtl: bool,
) -> Option<(Icon, bool, bool)> {
    use ToolbarItem as T;
    Some(match it {
        T::Fit if st.at_fit => (Icon::ActualSize, false, false),
        T::Fullscreen if st.fullscreen => (Icon::ExitFullscreen, false, false),
        T::Info => (Icon::Info, false, info_open),
        T::Mpr => (Icon::Grid, false, st.actions.mpr),
        T::ZoomLevel | T::Spacer | T::Separator => return None,
        _ => {
            let (icon, flip) = item_glyph(it, rtl)?;
            (icon, flip, false)
        }
    })
}

/// The icon that stands for an item (on the bar in its usual state, and in the lists of
/// Settings and the customising tray), and whether it mirrors in right-to-left layouts.
/// `None` for the zoom readout, which shows a percentage instead.
pub(crate) fn item_glyph(it: ToolbarItem, rtl: bool) -> Option<(Icon, bool)> {
    use ToolbarItem as T;
    Some(match it {
        T::Spacer => (Icon::Spacer, false),
        T::Separator => (Icon::Separator, false),
        T::ZoomLevel => return None,
        T::Open => (Icon::Folder, false),
        T::Prev => (Icon::ChevronLeft, rtl),
        T::Next => (Icon::ChevronRight, rtl),
        T::ZoomOut => (Icon::ZoomOut, false),
        T::ZoomIn => (Icon::ZoomIn, false),
        T::Fit => (Icon::Fit, false),
        T::RotateLeft => (Icon::RotateLeft, false),
        T::RotateRight => (Icon::RotateRight, false),
        T::Info => (Icon::Info, false),
        T::Copy => (Icon::Copy, false),
        T::Delete => (Icon::Trash, false),
        T::Palette => (Icon::Search, false),
        T::Settings => (Icon::Settings, false),
        T::Fullscreen => (Icon::Fullscreen, false),
        T::Slideshow => (Icon::Play, rtl),
        T::ExportSlideshow => (Icon::Film, false),
        T::Mpr => (Icon::Grid, false),
        // A brand mark: never mirrored.
        T::Discord => (Icon::Discord, false),
    })
}

/// The name of an item as Settings lists it ("Next image", "Flexible space").
pub(crate) fn item_name(it: ToolbarItem) -> String {
    match specular_settings::find(crate::toolbar_edit::KEY) {
        Some(m) => I18n::global().tr(&m.option_id(it.as_str())),
        None => it.as_str().to_owned(),
    }
}

/// A separator's hairline in the middle of its slot `r`, on a bar whose top is `top` and
/// height `h` (`s`: the bar's scale).
pub(crate) fn separator(cx: &mut Ctx, r: Rect, top: f32, h: f32, s: f32) {
    let line = Rect::new(
        (r.center().0 - 0.5).round(),
        top + 12.0 * s,
        1.0,
        h - 24.0 * s,
    );
    cx.dl.rect(line, 0.0, cx.pal.separator);
}

/// The chip that turns `c`'s tools on, a glass pill centred above the bar `bar`.
fn profile_chip(cx: &mut Ctx, bar: Rect, c: specular_input::Capability) {
    let style = cx.style(metrics::FONT_LABEL).weight(500.0);
    let b = cx.layout(&crate::palette::tools(c), style, None);
    let h = cx.u(30.0);
    let w = (b.width + 28.0).min(cx.w - 16.0);
    let r = Rect::new(((cx.w - w) * 0.5).round(), (bar.y - h - 10.0).round(), w, h);
    let t = Target::ProfileChip(c);
    cx.dl.shadow(
        r,
        h * 0.5,
        cx.pal.shadow,
        metrics::SHADOW_BLUR,
        metrics::SHADOW_Y,
    );
    cx.dl
        .glass(r, h * 0.5, cx.pal.glass, cx.pal.glass_border, 1.0);
    if cx.is_hover(&t) {
        cx.dl.rect(r, h * 0.5, cx.pal.surface_hover);
    }
    cx.block_in(&b, r, HAlign::Center, cx.pal.text);
    cx.ring_if(&t, r, h * 0.5);
    cx.hit(r, t);
}

/// `items` without the zoom controls and Fit, and without the separators left at an end or
/// doubled by their going.
#[cfg(test)]
pub(crate) fn without_view_controls(items: &[ToolbarItem]) -> Vec<ToolbarItem> {
    use ToolbarItem as T;
    let mut out: Vec<ToolbarItem> = Vec::with_capacity(items.len());
    for &it in items {
        if matches!(it, T::ZoomOut | T::ZoomLevel | T::ZoomIn | T::Fit) {
            continue;
        }
        if it == T::Separator && out.last().is_none_or(|l| *l == T::Separator) {
            continue;
        }
        out.push(it);
    }
    while out.last() == Some(&T::Separator) {
        out.pop();
    }
    out
}

/// The bar's items for what is on screen: MPR only while the image is a volume (or MPR is
/// open), as its action is tier 1 for any volume.
pub(crate) fn for_content(mut items: Vec<ToolbarItem>, st: &ViewerState) -> Vec<ToolbarItem> {
    if !(st.actions.volume || st.actions.mpr) {
        items.retain(|it| *it != ToolbarItem::Mpr);
    }
    items
}

/// Where a bar `width` wide goes (fully shown).
fn place(cx: &Ctx, width: f32) -> Rect {
    let h = metrics::TOOLBAR_HEIGHT;
    Rect::new(
        ((cx.w - width) * 0.5).round(),
        cx.h - metrics::TOOLBAR_MARGIN - h,
        width,
        h,
    )
}

/// Where the bar is this frame once fully shown, with the list stored now (what it shows
/// while Settings is open).
pub(crate) fn bar_rect(cx: &mut Ctx, st: &ViewerState) -> Rect {
    let items = for_content(crate::toolbar_edit::current(st.store), st);
    let ms = measure(cx, st, &items, room(cx.w), true);
    place(cx, ms.layout.width)
}

/// Draws the toolbar; returns where the bar and its menu buttons are (its open menus are
/// drawn later, [`draw_menus`]).
pub(crate) fn draw(ch: &mut Chrome, cx: &mut Ctx, st: &ViewerState, alpha: f32) -> Menus {
    use ToolbarItem as T;
    // An item of the bar being dragged (bar_drag.rs): the bar shows where it would land.
    let drag = ch
        .bar_press
        .as_ref()
        .filter(|d| d.active)
        .map(|d| d.shown());
    // While Settings is open the bar shows the stored list (the toolbar list's moves show on
    // the bar as they are made).
    let items: Vec<ToolbarItem> = if let Some((l, _)) = &drag {
        l.clone()
    } else if ch.modal.is_some() {
        crate::toolbar_edit::current(st.store)
    } else {
        st.settings.viewing.toolbar_items.clone()
    };
    // Nothing open: the zoom controls and Fit stay, greyed, so the bar keeps its size from
    // the start screen to an image and back.
    // MPR only for a volume, also under Settings (a dragged bar is made of what showed).
    let items = if drag.is_none() {
        for_content(items, st)
    } else {
        items
    };
    let ms = measure(cx, st, &items, room(cx.w), true);
    let readout_style = cx.style(metrics::FONT_LABEL).tabular().weight(500.0);
    let h = metrics::TOOLBAR_HEIGHT;
    let mut bar = place(cx, ms.layout.width);
    bar.y += (1.0 - alpha) * 10.0;

    cx.dl.push_opacity(alpha);
    cx.dl.shadow(
        bar,
        h * 0.5,
        cx.pal.shadow,
        metrics::SHADOW_BLUR,
        metrics::SHADOW_Y,
    );
    cx.dl
        .glass(bar, h * 0.5, cx.pal.glass, cx.pal.glass_border, 1.0);
    cx.hit(bar, Target::ToolbarBg);

    let by = bar.y + (h - metrics::BUTTON) * 0.5;
    if let Some((b, text_w)) = &ms.pos {
        let r = cx.m_in(
            Rect::new(bar.x + PAD + 2.0, by, text_w + 16.0, metrics::BUTTON),
            bar,
        );
        if let Some(edit) = ch.goto.as_ref().filter(|_| !ch.goto_time) {
            goto_field(cx, r, edit, st, readout_style);
        } else {
            let t = Target::Position;
            if cx.is_hover(&t) && t.disabled(st).is_none() {
                cx.dl
                    .rect(r, metrics::RADIUS_SM + 2.0, cx.pal.surface_hover);
            }
            cx.block_in(b, r, HAlign::Center, cx.pal.text_muted);
            cx.ring_if(&t, r, metrics::RADIUS_SM + 2.0);
        }
        cx.hit(r, Target::Position);
        // Hairline between readout and buttons.
        let x = bar.x + PAD + ms.lead;
        let sep = cx.m_in(Rect::new(x - 5.0, bar.y + 12.0, 1.0, h - 24.0), bar);
        cx.dl.rect(sep, 0.0, cx.pal.separator);
    }
    // A domain image with its profile off: the chip that turns its tools on.
    let chip = st.actions.content_profile().chip();
    let mut zoom_rect = None;
    let mut slots = Vec::with_capacity(items.len());
    let slot = |cx: &Ctx, i: usize| {
        let (x, w) = ms.layout.slots[i];
        cx.m_in(Rect::new(bar.x + x, by, w, metrics::BUTTON), bar)
    };
    // The items that don't fit are in the More menu, whose button takes the next slot.
    let more = (ms.shown < items.len()).then(|| slot(cx, ms.shown));
    ch.toolbar_overflow = items[ms.shown..]
        .iter()
        .copied()
        .filter(|it| !it.is_layout())
        .collect();
    if ch.toolbar_overflow.is_empty() {
        ch.overflow_menu = None;
    }
    // The bar's controls in order, for the keyboard (focus.rs): the chip above the bar
    // first, as it is read.
    ch.bar_stops = chip
        .map(Target::ProfileChip)
        .into_iter()
        .chain(ms.pos.iter().map(|_| Target::Position))
        .chain(
            items[..ms.shown]
                .iter()
                .filter(|it| !it.is_layout())
                .map(|it| Target::Toolbar(*it)),
        )
        .chain(more.map(|_| Target::ToolbarMore))
        .collect();
    for (i, it) in items[..ms.shown].iter().enumerate() {
        let r = slot(cx, i);
        slots.push(r);
        if drag.as_ref().is_some_and(|(_, at)| *at == i) {
            crate::bar_drag::socket(cx, r);
            continue;
        }
        let t = Target::Toolbar(*it);
        let enabled = t.disabled(st).is_none();
        match it {
            // Spacers and separators can be picked up too.
            T::Spacer => cx.hit(r, Target::BarSlot(i)),
            T::Separator => {
                separator(cx, r, bar.y, h, 1.0);
                cx.hit(r, Target::BarSlot(i));
            }
            T::ZoomLevel => {
                let hov = enabled && (cx.is_hover(&t) || ch.zoom_menu);
                if hov {
                    cx.dl
                        .rect(r, metrics::RADIUS_SM + 2.0, cx.pal.surface_hover);
                }
                if let Some(b) = &ms.zoom {
                    let c = if enabled {
                        cx.pal.text
                    } else {
                        cx.pal.text_faint.with_alpha(0.55)
                    };
                    cx.block_in(b, r, HAlign::Center, c);
                }
                cx.ring_if(&t, r, metrics::RADIUS_SM + 2.0);
                cx.hit(r, t);
                zoom_rect = Some(r);
            }
            _ => {
                if let Some((icon, flip, active)) = item_icon(*it, st, ch.info_open, cx.rtl) {
                    let color = cx.pal.text;
                    cx.icon_button_e(r, icon, t, color, flip, active, enabled);
                }
            }
        }
    }
    if let Some(r) = more {
        let open = ch.overflow_menu.is_some();
        let color = cx.pal.text;
        cx.icon_button_e(r, Icon::More, Target::ToolbarMore, color, false, open, true);
    }
    if let Some(c) = chip {
        profile_chip(cx, bar, c);
    }
    cx.dl.pop_opacity();

    // The window changed size under a press: the bar it was made on is gone, nothing moves
    // (what was drawn this frame is not the stored order, so it is not recorded either).
    if ch
        .bar_press
        .as_ref()
        .is_some_and(|d| d.geom.window != (cx.w, cx.h))
    {
        ch.bar_press = None;
        ch.bar_geom = None;
    } else if ch.bar_press.is_none() {
        ch.bar_geom = Some(crate::bar_drag::BarGeom {
            bar,
            slots,
            items: items.clone(),
            rtl: cx.rtl,
            window: (cx.w, cx.h),
        });
    }
    if drag.is_some() {
        crate::bar_drag::ghost(ch, cx, st, ms.zoom.as_deref());
    }
    Menus {
        bar,
        more,
        zoom: zoom_rect,
    }
}

/// Where the bar's menus open from, as the bar was drawn ([`draw_menus`]).
pub(crate) struct Menus {
    pub bar: Rect,
    /// The More button.
    more: Option<Rect>,
    /// The zoom readout (on the bar).
    zoom: Option<Rect>,
}

/// The bar's open menus (its context menu, More, the zoom presets). Drawn after everything
/// else outside the dialogs (the toasts, the insights indicator), so nothing covers an open
/// menu or takes its clicks.
pub(crate) fn draw_menus(ch: &Chrome, cx: &mut Ctx, st: &ViewerState, m: &Menus) {
    let bar = m.bar;
    if let Some(p) = ch.toolbar_menu {
        let p = if p.0.is_nan() {
            (bar.x + bar.w * 0.5, bar.y)
        } else {
            p
        };
        crate::toolbar_edit::context_menu(cx, p);
    }
    if let (Some(sel), Some(r)) = (ch.overflow_menu, m.more) {
        overflow_menu(ch, cx, st, r, sel);
    }
    // The zoom readout in the More menu opens its presets over the More button.
    if ch.zoom_menu
        && let Some(zr) = m.zoom.or(m.more)
    {
        zoom_menu(cx, zr, ch.zoom_sel, st.fullscreen);
    }
}

/// The More menu: the items that don't fit on the bar, as rows (icon, name, shortcut) above
/// its button `anchor`, lined up with its end edge. `sel`: the row the keyboard is on.
fn overflow_menu(ch: &Chrome, cx: &mut Ctx, st: &ViewerState, anchor: Rect, sel: Option<usize>) {
    let items = &ch.toolbar_overflow;
    let style = cx.style(metrics::FONT_BODY);
    let small = cx.style(metrics::FONT_SMALL);
    let (row, icon, pad) = (cx.u(32.0), cx.u(metrics::ICON), 4.0);
    // In a window too short for all the rows (the 260 px minimum holds about seven), the rows
    // close up so the last stays on screen.
    let fit = ((cx.h - 16.0 - pad * 2.0) / items.len().max(1) as f32).floor();
    let row = row.min(fit.max(cx.u(24.0)));
    let lead = 12.0 + icon + 10.0;
    let labels: Vec<(String, Option<String>)> = items
        .iter()
        .map(|it| {
            let (mut label, a) = item_label(*it, st);
            if *it == ToolbarItem::ZoomLevel {
                label = format!("{label} ({})", zoom_text(st));
            }
            let key = a
                .and_then(|a| st.keymap.primary_key(a))
                // Isolated left to right, so `]` or `Ctrl+,` read the same in RTL.
                .map(|k| format!("\u{2066}{k}\u{2069}"));
            (label, key)
        })
        .collect();
    let key_w = labels
        .iter()
        .filter_map(|(_, k)| k.as_ref())
        .map(|k| cx.layout(k, small, None).width + 16.0)
        .fold(0.0f32, f32::max);
    let min_label = cx.u(120.0);
    let label_w = labels
        .iter()
        .map(|(l, _)| cx.layout(l, style, None).width)
        .fold(min_label, f32::max);
    let w = (lead + label_w + key_w + 14.0 + pad * 2.0).min((cx.w - 16.0).max(120.0));
    let h = row * items.len() as f32 + pad * 2.0;
    // Its end edge lines up with the button's (the right edge, or the left one in RTL).
    let x = if cx.rtl { anchor.x } else { anchor.right() - w };
    let x = x.clamp(8.0, (cx.w - w - 8.0).max(8.0));
    let y = (anchor.y - h - 14.0).max(8.0);
    let r = Rect::new(x.round(), y.round(), w, h);
    cx.dl
        .shadow(r, metrics::RADIUS_MD, cx.pal.shadow, 20.0, 6.0);
    cx.dl.glass(
        r,
        metrics::RADIUS_MD,
        cx.pal.glass,
        cx.pal.glass_border,
        1.0,
    );
    cx.hit(r, Target::ToolbarBg);
    cx.popovers.push(r);
    for (i, (it, (label, key))) in items.iter().zip(&labels).enumerate() {
        let rr = Rect::new(r.x + pad, r.y + pad + i as f32 * row, w - pad * 2.0, row);
        let t = Target::Toolbar(*it);
        let enabled = t.disabled(st).is_none();
        let (glyph, flip, active) = match item_icon(*it, st, ch.info_open, cx.rtl) {
            Some((g, f, a)) => (Some(g), f, a),
            None => (None, false, false),
        };
        if enabled && (cx.is_hover(&t) || sel == Some(i)) {
            cx.dl.rect(rr, metrics::RADIUS_SM, cx.pal.surface_hover);
        } else if active {
            cx.dl.rect(rr, metrics::RADIUS_SM, cx.pal.accent_soft);
        }
        let color = if !enabled {
            cx.pal.text_faint.with_alpha(0.55)
        } else if active {
            cx.pal.accent
        } else {
            cx.pal.text
        };
        if let Some(g) = glyph {
            let ir = cx.m_in(Rect::new(rr.x + 12.0, rr.y, icon, rr.h), rr);
            let ir = ir.centered(icon, icon);
            draw_icon(cx.dl, g, ir, color, metrics::ICON_STROKE, flip);
        }
        let kw = key.as_ref().map_or(0.0, |_| key_w);
        let lr = Rect::new(rr.x + lead, rr.y, (rr.w - lead - kw - 10.0).max(0.0), rr.h);
        let lr = cx.m_in(lr, rr);
        cx.label_fit(label, style, lr, HAlign::Start, color, false);
        if let Some(k) = key {
            let kr = cx.m_in(Rect::new(rr.right() - kw - 10.0, rr.y, kw, rr.h), rr);
            let kc = if enabled {
                cx.pal.text_muted
            } else {
                cx.pal.text_faint.with_alpha(0.55)
            };
            cx.label_fit(k, small, kr, HAlign::End, kc, false);
        }
        if sel == Some(i) {
            cx.dl.rect_border(
                rr,
                metrics::RADIUS_SM,
                specular_theme::Color::TRANSPARENT,
                cx.pal.focus_ring,
                2.0,
            );
        }
        cx.hit(rr, t);
    }
}

/// The toolbar as it would look holding `items`, scaled down to fit `area` (the preview in
/// Settings). Drawn flat (the sheet has no backdrop to blur) and inert.
pub(crate) fn preview(
    cx: &mut Ctx,
    st: &ViewerState,
    info_open: bool,
    items: &[ToolbarItem],
    area: Rect,
) {
    use ToolbarItem as T;
    let ms = measure(cx, st, items, room(cx.w), false);
    let full_h = metrics::TOOLBAR_HEIGHT;
    let s = (area.w / ms.layout.width).min(area.h / full_h).min(1.0);
    let (w, h) = (ms.layout.width * s, full_h * s);
    let bar = Rect::new(
        (area.x + (area.w - w) * 0.5).round(),
        (area.y + (area.h - h) * 0.5).round(),
        w,
        h,
    );
    cx.dl
        .shadow(bar, h * 0.5, cx.pal.shadow.fade(0.5), 8.0, 2.0);
    cx.dl.rect_border(
        bar,
        h * 0.5,
        cx.pal.surface_raised,
        cx.pal.glass_border,
        1.0,
    );
    let bh = metrics::BUTTON * s;
    let by = bar.y + (h - bh) * 0.5;
    // The readouts at the preview's scale.
    let readout_style = cx.style(metrics::FONT_LABEL * s).tabular().weight(500.0);
    if let Some((_, text_w)) = &ms.pos {
        let r = cx.m_in(
            Rect::new(bar.x + (PAD + 2.0) * s, by, (text_w + 16.0) * s, bh),
            bar,
        );
        let b = cx.layout(&position_text(st), readout_style, None);
        cx.block_in(&b, r, HAlign::Center, cx.pal.text_muted);
        let x = bar.x + (PAD + ms.lead - 5.0) * s;
        let sep = cx.m_in(Rect::new(x, bar.y + 12.0 * s, 1.0, h - 24.0 * s), bar);
        cx.dl.rect(sep, 0.0, cx.pal.separator);
    }
    let icon = metrics::ICON * s;
    for (it, (x, iw)) in items.iter().zip(&ms.layout.slots) {
        let r = cx.m_in(Rect::new(bar.x + x * s, by, iw * s, bh), bar);
        match it {
            T::Spacer => {}
            T::Separator => separator(cx, r, bar.y, h, s),
            T::ZoomLevel => {
                let z = cx.layout(&zoom_text(st), readout_style, None);
                cx.block_in(&z, r, HAlign::Center, cx.pal.text);
            }
            _ => {
                if let Some((ic, flip, active)) = item_icon(*it, st, info_open, cx.rtl) {
                    if active {
                        cx.dl
                            .rect(r, (metrics::RADIUS_SM + 2.0) * s, cx.pal.accent_soft);
                    }
                    let c = if active { cx.pal.accent } else { cx.pal.text };
                    draw_icon(
                        cx.dl,
                        ic,
                        r.centered(icon, icon),
                        c,
                        (metrics::ICON_STROKE * s).max(1.0),
                        flip,
                    );
                }
            }
        }
    }
}

/// The position readout while an image number is typed into it: the number in the language's
/// digits and grouping (or, before anything is typed, the range in faint text), its
/// selection and the caret. It is recorded as the field drawn ([`FieldGeom`], whose `digits`
/// map what is drawn to the typed digits), so clicks, drags and middle clicks reach it.
fn goto_field(cx: &mut Ctx, r: Rect, edit: &LineEdit, st: &ViewerState, style: TextStyle) {
    cx.dl.rect_border(
        r,
        metrics::RADIUS_SM + 2.0,
        cx.pal.surface_raised,
        cx.pal.focus_ring.with_alpha(0.7),
        1.0,
    );
    cx.text_field = Some(r);
    // The caret is as tall as the text line (it grows with the text size), within the field.
    let caret_h = |b: &TextBlock| b.height.min(r.h - 4.0).max(r.h - 16.0).round();
    let composing = !cx.preedit.is_empty();
    if edit.is_empty() && !composing {
        let hint = t!("toolbar-go-to-range", count = st.count as i64);
        let b = cx.layout(&hint, style, None);
        cx.block_in(&b, r, HAlign::Center, cx.pal.text_faint);
        // At the start of the hint.
        let edge = (r.w - b.width) * 0.5;
        let x = if cx.rtl {
            r.right() - edge + 1.0
        } else {
            r.x + edge - 2.5
        };
        let ch = caret_h(&b);
        let caret = Rect::new(x, r.center().1 - ch * 0.5, 1.5, ch);
        cx.dl.rect(caret, 0.0, cx.pal.accent);
        cx.ime_area = Some(caret);
        cx.field_geom = Some(FieldGeom {
            rect: r,
            text: None,
            digits: None,
        });
        return;
    }
    let shown = goto_shown(edit.text());
    // Digit `k` of the text is the `k`th digit drawn; a caret after it goes after it (a
    // group separator that follows stays after the caret).
    let (starts, ends) = digit_bytes(&shown);
    let after = |k: usize| match k {
        0 => starts[0],
        k => ends.get(k - 1).copied().unwrap_or(shown.len()),
    };
    let at = after(edit.caret());
    // A number being composed (an input method in full-width mode) shows at the caret.
    let pre = cx.preedit.len();
    let text = format!("{}{}{}", &shown[..at], cx.preedit, &shown[at..]);
    let b = cx.layout(&text, style, None);
    let c = at + pre;
    // Centred while it fits (the readout is as wide as this folder's numbers). A longer
    // number scrolls within the field: its end shows, or the caret when that is further in.
    let inner = r.inset(4.0);
    let x0 = if b.width <= inner.w {
        r.x + (r.w - b.width) * 0.5
    } else {
        (inner.right() - b.width).max(inner.x - b.caret_x(c))
    };
    let ch = caret_h(&b);
    let top = r.center().1 - ch * 0.5;
    cx.dl.push_clip(r);
    if !composing && edit.has_selection() {
        let sel = edit.selection();
        let range = starts[sel.start.min(starts.len() - 1)]..after(sel.end);
        for (a, z) in b.range_spans(range) {
            cx.dl
                .rect(Rect::new(x0 + a, top, z - a, ch), 0.0, cx.pal.selection);
        }
    }
    let y = r.y + (r.h - b.height) * 0.5;
    cx.text.draw(cx.dl, &b, x0, y, cx.pal.text);
    cx.dl.pop_clip();
    // Digits run left to right in every script, so at the end the caret is on the right.
    let x = x0 + b.caret_x(c) + if c == text.len() { 1.0 } else { -0.75 };
    let caret = Rect::new(x, top, 1.5, ch);
    cx.dl.rect(caret, 0.0, cx.pal.accent);
    cx.ime_area = Some(caret);
    cx.field_geom = Some(FieldGeom {
        rect: r,
        text: Some((b, x0)),
        digits: Some(text),
    });
}

/// The go-to field's typed digits as drawn: the number in the language's digits and
/// grouping, with any leading zeros (left by deleting the first digit) kept.
fn goto_shown(typed: &str) -> String {
    let loc = I18n::global().localizer();
    let rest = typed.trim_start_matches('0');
    let mut s = loc.integer_plain(0).repeat(typed.len() - rest.len());
    if let Ok(n) = rest.parse::<i64>() {
        s.push_str(&loc.integer(n));
    }
    s
}

/// The zoom presets popover's entries; the last, Full screen, is ticked while the window is
/// full screen (choosing it again keeps it so; a fit entry keeps full screen too).
pub(crate) fn zoom_entries() -> [(String, Action); 7] {
    let loc = I18n::global().localizer();
    [
        (t!("zoom-preset-fit"), Action::ZoomFit),
        (t!("zoom-preset-fill"), Action::ZoomFill),
        (t!("zoom-preset-actual"), Action::ZoomTo(100.0)),
        (
            t!("zoom-preset-percent", zoom = loc.percent(2.0, 0)),
            Action::ZoomTo(200.0),
        ),
        (
            t!("zoom-preset-percent", zoom = loc.percent(4.0, 0)),
            Action::ZoomTo(400.0),
        ),
        (
            t!("zoom-preset-percent", zoom = loc.percent(8.0, 0)),
            Action::ZoomTo(800.0),
        ),
        (t!("zoom-preset-fullscreen"), Action::ToggleFullscreen),
    ]
}

/// The zoom presets above `anchor`; `sel`: the row the keyboard is on; `fullscreen`: the
/// Full screen row is ticked.
fn zoom_menu(cx: &mut Ctx, anchor: Rect, sel: Option<usize>, fullscreen: bool) {
    let entries = zoom_entries();
    let style = cx.style(metrics::FONT_BODY);
    let blocks: Vec<_> = entries
        .iter()
        .map(|(s, _)| cx.layout(s, style, None))
        .collect();
    // Room for the tick at the start of the Full screen row (every menu's ticks lead).
    let tick = 16.0;
    let w = blocks.iter().map(|b| b.width).fold(120.0f32, f32::max) + 28.0 + tick + 6.0;
    let row = 30.0;
    let h = row * entries.len() as f32 + 8.0;
    let x = (anchor.center().0 - w * 0.5).clamp(8.0, cx.w - w - 8.0);
    let r = Rect::new(x, anchor.y - h - 14.0, w, h);
    cx.dl
        .shadow(r, metrics::RADIUS_MD, cx.pal.shadow, 20.0, 6.0);
    cx.dl.glass(
        r,
        metrics::RADIUS_MD,
        cx.pal.glass,
        cx.pal.glass_border,
        1.0,
    );
    cx.hit(r, Target::ToolbarBg);
    cx.popovers.push(r);
    for (i, ((_, a), b)) in entries.iter().zip(&blocks).enumerate() {
        let rr = Rect::new(r.x + 4.0, r.y + 4.0 + i as f32 * row, w - 8.0, row);
        let t = Target::ZoomPreset(*a);
        if cx.is_hover(&t) || sel == Some(i) {
            cx.dl.rect(rr, metrics::RADIUS_SM, cx.pal.surface_hover);
        }
        let tr = rr.inset_xy(10.0, 0.0);
        let lr = cx.m_in(
            Rect::new(tr.x + tick + 6.0, tr.y, tr.w - tick - 6.0, tr.h),
            tr,
        );
        cx.block_in(b, lr, HAlign::Start, cx.pal.text);
        if *a == Action::ToggleFullscreen {
            // A divider sets it apart from the zoom levels.
            let sep = Rect::new(rr.x + 6.0, rr.y - 0.5, rr.w - 12.0, 1.0);
            cx.dl.rect(sep, 0.0, cx.pal.separator);
            if fullscreen {
                // The tick at the row's start (its right end right to left).
                let ir = Rect::new(tr.x, rr.center().1 - tick * 0.5, tick, tick);
                let ir = cx.m_in(ir, tr);
                draw_icon(cx.dl, Icon::Check, ir, cx.pal.accent, 2.0, false);
            }
        }
        if sel == Some(i) {
            cx.focus_ring(rr, metrics::RADIUS_SM);
        }
        cx.hit(rr, t);
    }
}

/// A round arrow button at the left or right edge, vertically centred.
pub(crate) fn edge_arrow(cx: &mut Ctx, left: bool, next: bool, alpha: f32, inset: f32) {
    let s = metrics::EDGE_ARROW;
    let x = if left {
        16.0 + inset
    } else {
        cx.w - 16.0 - s - inset
    };
    let r = Rect::new(x, (cx.h - s) * 0.5, s, s);
    let t = if next {
        Target::EdgeNext
    } else {
        Target::EdgePrev
    };
    cx.dl.push_opacity(alpha);
    cx.dl.shadow(r, s * 0.5, cx.pal.shadow, 16.0, 4.0);
    cx.dl
        .glass(r, s * 0.5, cx.pal.glass, cx.pal.glass_border, 1.0);
    if cx.is_hover(&t) {
        cx.dl.rect(r, s * 0.5, cx.pal.surface_hover);
    }
    let icon = if left {
        Icon::ChevronLeft
    } else {
        Icon::ChevronRight
    };
    draw_icon(cx.dl, icon, r.centered(22.0, 22.0), cx.pal.text, 2.0, false);
    cx.hit(r, t);
    cx.dl.pop_opacity();
}

/// The viewer's back button's size; it sits 16 px in from the top leading corner.
pub(crate) const BACK_SIZE: f32 = metrics::BUTTON + 6.0;

/// The viewer's back button, at the top leading corner: back to the gallery.
pub(crate) fn back_button(cx: &mut Ctx, alpha: f32) {
    let s = BACK_SIZE;
    let r = cx.m(Rect::new(16.0, 16.0, s, s));
    let t = Target::ViewerBack;
    cx.dl.push_opacity(alpha);
    cx.dl.shadow(
        r,
        s * 0.5,
        cx.pal.shadow,
        metrics::SHADOW_BLUR,
        metrics::SHADOW_Y,
    );
    cx.dl
        .glass(r, s * 0.5, cx.pal.glass, cx.pal.glass_border, 1.0);
    if cx.is_hover(&t) {
        cx.dl.rect(r, s * 0.5, cx.pal.surface_hover);
    }
    let rtl = cx.rtl;
    draw_icon(
        cx.dl,
        Icon::ChevronLeft,
        r.centered(20.0, 20.0),
        cx.pal.text,
        2.0,
        rtl,
    );
    cx.ring_if(&t, r, s * 0.5);
    cx.hit(r, t);
    cx.dl.pop_opacity();
}
