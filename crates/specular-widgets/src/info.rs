//! The info panel: the metadata summary, or every field, in a glass side panel on the end
//! side of the window (right in LTR, left in RTL).

use specular_gfx::{Icon, Rect, draw_icon};
use specular_i18n::t;
use specular_theme::metrics;

use crate::ctx::{Ctx, HAlign, Target};
use crate::{Chrome, ViewerState};

/// The panel's width: wider with larger text (half as fast as the text grows), so labels and
/// values keep a readable measure.
pub(crate) fn panel_width(text_scale: f32) -> f32 {
    (metrics::PANEL_WIDTH * (1.0 + (text_scale.max(1.0) - 1.0) * 0.5)).round()
}

pub(crate) fn draw(ch: &mut Chrome, cx: &mut Ctx, st: &ViewerState, alpha: f32) {
    let m = metrics::TOOLBAR_MARGIN;
    let w = panel_width(ch.text_scale).min(cx.w - 2.0 * m);
    let bottom = m + metrics::TOOLBAR_HEIGHT + 12.0;
    let slide = (1.0 - alpha) * 24.0;
    let panel = cx.m(Rect::new(
        cx.w - m - w + slide,
        m,
        w,
        (cx.h - m - bottom).max(120.0),
    ));
    cx.dl.push_opacity(alpha);
    cx.dl.shadow(
        panel,
        metrics::RADIUS_LG,
        cx.pal.shadow,
        metrics::SHADOW_BLUR,
        metrics::SHADOW_Y,
    );
    cx.dl.glass(
        panel,
        metrics::RADIUS_LG,
        cx.pal.glass,
        cx.pal.glass_border,
        1.0,
    );
    cx.hit(panel, Target::InfoPanel);

    // Header: title and close button. A long file name wraps (breaking anywhere: names
    // often have no spaces) over up to three lines; a longer one is cut in the middle and
    // shown whole in the title's tooltip.
    let title_style = cx.style(metrics::FONT_TITLE).weight(600.0).break_anywhere();
    let row_h = cx.u(32.0);
    let title = st
        .file_name
        .map(str::to_owned)
        .unwrap_or_else(|| t!("toolbar-info"));
    let tw = panel.w - 32.0 - 36.0;
    let line_h = cx.layout("Ag", title_style, None).height;
    let (tb, _cut) = title_block(cx, &title, title_style, tw, line_h);
    let head_h = if tb.height > line_h + 0.5 {
        tb.height + (row_h - line_h).max(0.0)
    } else {
        row_h
    };
    let head = Rect::new(panel.x + 16.0, panel.y + 10.0, panel.w - 32.0, head_h);
    let close = cx.m_in(
        Rect::new(head.right() - 28.0, head.y + row_h * 0.5 - 14.0, 28.0, 28.0),
        panel,
    );
    cx.icon_button(
        close,
        Icon::Close,
        Target::InfoClose,
        cx.pal.text_muted,
        false,
        false,
    );
    let tr = cx.m_in(Rect::new(head.x, head.y, tw, head.h), panel);
    // The title copies the file name (click or right-click), shown on hover.
    if cx.is_hover(&Target::InfoTitle) {
        let bg = if cx.is_pressed(&Target::InfoTitle) {
            cx.pal.surface_pressed
        } else {
            cx.pal.surface_hover
        };
        cx.dl.rect(tr.inset(-4.0), metrics::RADIUS_SM, bg);
    }
    cx.dl.push_clip(tr);
    if tb.height > line_h + 0.5 {
        let top = Rect::new(
            tr.x,
            head.y + ((row_h - line_h) * 0.5).max(0.0),
            tr.w,
            tb.height,
        );
        cx.block_top(&tb, top, HAlign::Start, cx.pal.text);
    } else {
        cx.block_in(
            &tb,
            Rect::new(tr.x, head.y, tr.w, row_h),
            HAlign::Start,
            cx.pal.text,
        );
    }
    cx.dl.pop_clip();
    cx.hit(tr, Target::InfoTitle);

    // Tabs: Summary | All fields.
    let tabs = Rect::new(
        panel.x + 16.0,
        head.bottom() + 8.0,
        panel.w - 32.0,
        cx.u(30.0),
    );
    cx.dl
        .rect(tabs, metrics::RADIUS_SM + 2.0, cx.pal.surface_hover);
    let half = (tabs.w - 4.0) * 0.5;
    for (i, (label, all)) in [
        (t!("app-info-summary"), false),
        (t!("app-info-all-fields"), true),
    ]
    .into_iter()
    .enumerate()
    {
        let r = cx.m_in(
            Rect::new(
                tabs.x + 2.0 + i as f32 * half,
                tabs.y + 2.0,
                half,
                tabs.h - 4.0,
            ),
            tabs,
        );
        let tgt = Target::InfoTab(all);
        let selected = ch.info_all == all;
        if selected {
            cx.dl
                .shadow(r, metrics::RADIUS_SM, cx.pal.shadow.fade(0.5), 4.0, 1.0);
            cx.dl.rect(r, metrics::RADIUS_SM, cx.pal.surface_raised);
        } else if cx.is_hover(&tgt) {
            cx.dl.rect(r, metrics::RADIUS_SM, cx.pal.surface_hover);
        }
        let style = cx
            .style(metrics::FONT_LABEL)
            .weight(if selected { 600.0 } else { 450.0 });
        let c = if selected {
            cx.pal.text
        } else {
            cx.pal.text_muted
        };
        cx.label_fit(
            &label,
            style,
            r.inset_xy(6.0, 0.0),
            HAlign::Center,
            c,
            false,
        );
        cx.ring_if(&tgt, r, metrics::RADIUS_SM);
        cx.hit(r, tgt);
    }

    // Body.
    let body = Rect::new(
        panel.x,
        tabs.bottom() + 10.0,
        panel.w,
        panel.bottom() - tabs.bottom() - 14.0,
    );
    // The same margin on both sides; the scrollbar sits in the end margin. Text is clipped to
    // the inner column (a little slack for glyph overhang) so nothing can reach the edge.
    let inner = body.inset_xy(16.0, 0.0);
    cx.dl
        .push_clip(Rect::new(inner.x - 2.0, body.y, inner.w + 4.0, body.h));
    let mut y = body.y - ch.info_scroll;
    ch.info_rows.clear();
    // The All-fields row with the keyboard focus: where it is (top, height) and, when on
    // screen, its rectangle for the focus ring (drawn outside the text clip).
    let mut focus_row: Option<(f32, f32)> = None;
    let mut ring: Option<Rect> = None;
    let label_w = cx.u(112.0).min((inner.w * 0.4).round());
    // Values and tags wrap at words, and a word too long for the column (digests, hashes,
    // paths, base64) breaks at any character.
    let small = cx.style(metrics::FONT_SMALL).break_anywhere();
    let value_style = cx.style(metrics::FONT_BODY).break_anywhere();
    let group_style = cx.style(metrics::FONT_SMALL).weight(650.0);
    match st.metadata {
        None => {
            let r = Rect::new(inner.x, y + 8.0, inner.w, 24.0);
            cx.label(
                &t!("app-info-loading"),
                value_style,
                r,
                HAlign::Start,
                cx.pal.text_muted,
            );
            y += 40.0;
        }
        Some(md) if !ch.info_all => {
            for (gi, g) in md.summary.iter().enumerate() {
                if g.items.is_empty() {
                    continue;
                }
                let gb = cx.layout(&g.title, group_style, Some(inner.w));
                if y + gb.height > body.y - 40.0 && y < body.bottom() {
                    cx.block_top(
                        &gb,
                        Rect::new(inner.x, y + 6.0, inner.w, gb.height),
                        HAlign::Start,
                        cx.pal.text_faint,
                    );
                }
                y += gb.height + 12.0;
                for (ii, it) in g.items.iter().enumerate() {
                    let lb = cx.layout(&it.label, small, Some(label_w - 8.0));
                    // A label word too long for the label column (large text, long compounds)
                    // puts the label on its own line above the value.
                    let stacked = lb.width > label_w - 8.0 + 0.5;
                    let lb = if stacked {
                        cx.layout(&it.label, small, Some(inner.w))
                    } else {
                        lb
                    };
                    let (vx, vw) = if stacked {
                        (0.0, inner.w)
                    } else {
                        (label_w, inner.w - label_w)
                    };
                    let vy = if stacked { lb.height + 2.0 } else { 0.0 };
                    let value = ltr_values(&it.value, cx.rtl);
                    let vb = cx.layout(&value, value_style, Some(vw));
                    let db = it
                        .detail
                        .as_ref()
                        .map(|d| cx.layout(&ltr_values(d, cx.rtl), small, Some(vw)));
                    let vh = vb.height + db.as_ref().map_or(0.0, |d| d.height + 2.0);
                    let rh = if stacked {
                        vy + vh + 8.0
                    } else {
                        lb.height.max(vh) + 8.0
                    };
                    if y + rh > body.y && y < body.bottom() {
                        // The whole row copies its value, as in "All fields".
                        let tgt = Target::InfoSummary(gi, ii);
                        let row = Rect::new(inner.x - 4.0, y - 2.0, inner.w + 8.0, rh - 2.0);
                        if cx.is_hover(&tgt) {
                            let bg = if cx.is_pressed(&tgt) {
                                cx.pal.surface_pressed
                            } else {
                                cx.pal.surface_hover
                            };
                            cx.dl.rect(row, metrics::RADIUS_SM, bg);
                            let s = cx.u(14.0);
                            let ic = cx.m_in(Rect::new(row.right() - s - 4.0, y + 2.0, s, s), row);
                            draw_icon(
                                cx.dl,
                                Icon::Copy,
                                ic,
                                cx.pal.text_muted,
                                metrics::ICON_STROKE,
                                false,
                            );
                        }
                        cx.hit(row.intersect(&body), tgt);
                        let lw = if stacked { inner.w } else { label_w - 8.0 };
                        let lr = cx.m_in(Rect::new(inner.x, y + 2.0, lw, lb.height), inner);
                        cx.block_top(&lb, lr, HAlign::Start, cx.pal.text_muted);
                        let vr = cx.m_in(Rect::new(inner.x + vx, y + vy, vw, vb.height), inner);
                        cx.block_top(&vb, vr, HAlign::Start, cx.pal.text);
                        if let Some(d) = &db {
                            let dr = cx.m_in(
                                Rect::new(inner.x + vx, y + vy + vb.height + 2.0, vw, d.height),
                                inner,
                            );
                            cx.block_top(d, dr, HAlign::Start, cx.pal.text_faint);
                        }
                    }
                    y += rh;
                }
                if g.key == "location"
                    && let Some(gps) = &md.gps
                {
                    y = show_on_map(cx, gps, inner, body, y, &mut ring);
                }
                y += 10.0;
            }
            for wmsg in &md.warnings {
                let b = cx.layout(wmsg, small, Some(inner.w));
                cx.block_top(
                    &b,
                    Rect::new(inner.x, y, inner.w, b.height),
                    HAlign::Start,
                    cx.pal.danger,
                );
                y += b.height + 6.0;
            }
            if st.has_image {
                y = edit_metadata(cx, inner, body, y, &mut ring);
            }
        }
        Some(md) => {
            y += 8.0;
            let mono = cx.style(metrics::FONT_MONO).break_anywhere();
            // Height of a row whose tag and value each fit on one line. Rows far outside the
            // view count as that without being shaped; it must be their real height (not a
            // guess), or the content height, and with it the end of the scroll range, would
            // change as the view moves.
            let one_line = cx.layout("Ag", mono, None).height
                + cx.layout("Ag", value_style, None).height
                + 8.0;
            if md
                .all_fields
                .iter()
                .all(|g| g.fields.iter().all(|f| f.empty))
            {
                // Says so, rather than an empty panel that looks unfinished.
                let b = cx.layout(&t!("app-info-no-fields"), value_style, Some(inner.w));
                let r = Rect::new(inner.x, y, inner.w, b.height);
                cx.block_top(&b, r, HAlign::Start, cx.pal.text_muted);
                y += b.height + 16.0;
            }
            for (gi, g) in md.all_fields.iter().enumerate() {
                let shown = g.fields.iter().filter(|f| !f.empty).count();
                if shown == 0 {
                    continue;
                }
                // A group's first row is revealed with its heading (the first group's with the
                // padding above it).
                let mut head = Some(if ch.info_rows.is_empty() { y - 8.0 } else { y });
                let gb = cx.layout(&g.title, group_style, Some(inner.w));
                if y + gb.height > body.y - 40.0 && y < body.bottom() {
                    cx.block_top(
                        &gb,
                        Rect::new(inner.x, y + 6.0, inner.w, gb.height),
                        HAlign::Start,
                        cx.pal.text_faint,
                    );
                }
                y += gb.height + 12.0;
                for (fi, f) in g.fields.iter().enumerate().filter(|(_, f)| !f.empty) {
                    let tgt = Target::InfoCopy(gi, fi);
                    let focused = cx.is_focused(&tgt);
                    let top = head.take().unwrap_or(y);
                    ch.info_rows.push(tgt.clone());
                    // Rows far outside the view are skipped without shaping when possible.
                    if y > body.bottom() + 400.0 {
                        if focused {
                            focus_row = Some((top, y + one_line - top));
                        }
                        y += one_line;
                        continue;
                    }
                    let tb = cx.layout(&f.tag, mono, Some(inner.w));
                    let vb = cx.layout(
                        &ltr_values(&f.display, cx.rtl),
                        value_style,
                        Some(inner.w - 8.0),
                    );
                    let rh = tb.height + vb.height + 8.0;
                    let row = Rect::new(inner.x - 4.0, y - 2.0, inner.w + 8.0, rh - 2.0);
                    if focused {
                        focus_row = Some((top, y + rh - top));
                    }
                    if y + rh > body.y && y < body.bottom() {
                        // The whole row copies its value; hovering (or the keyboard on it)
                        // shows the copy icon.
                        if focused {
                            ring = Some(row);
                        }
                        if cx.is_hover(&tgt) || focused {
                            let bg = if cx.is_pressed(&tgt) {
                                cx.pal.surface_pressed
                            } else {
                                cx.pal.surface_hover
                            };
                            cx.dl.rect(row, metrics::RADIUS_SM, bg);
                            let s = cx.u(14.0);
                            let ic = cx.m_in(
                                Rect::new(row.right() - s - 4.0, y + (tb.height - s) * 0.5, s, s),
                                row,
                            );
                            draw_icon(
                                cx.dl,
                                Icon::Copy,
                                ic,
                                cx.pal.text_muted,
                                metrics::ICON_STROKE,
                                false,
                            );
                        }
                        cx.hit(row.intersect(&body), tgt);
                        cx.block_top(
                            &tb,
                            Rect::new(inner.x, y, inner.w, tb.height),
                            HAlign::Start,
                            cx.pal.text_muted,
                        );
                        let vr = cx.m_in(
                            Rect::new(inner.x + 8.0, y + tb.height + 1.0, inner.w - 8.0, vb.height),
                            inner,
                        );
                        cx.block_top(&vb, vr, HAlign::Start, cx.pal.text);
                    }
                    y += rh;
                }
                y += 10.0;
            }
        }
    }
    if !ch.info_all
        && let Some(r) = &ch.open_report
    {
        y = performance(cx, r, ch.info_perf, inner, body, y);
    }
    cx.dl.pop_clip();
    if let Some(r) = ring {
        cx.dl.push_clip(body);
        cx.focus_ring(r, metrics::RADIUS_SM);
        cx.dl.pop_clip();
    }
    if ch.info_reveal > 0 {
        ch.info_reveal -= 1;
        if let Some((ry, rh)) = focus_row {
            // Scroll so the row is in the view with a little room, its top when it is taller
            // than the view (the scrollbar clamps the offset to the range).
            let m = 6.0;
            if ry < body.y + m || rh + 2.0 * m > body.h {
                ch.info_scroll = (ch.info_scroll + ry - body.y - m).max(0.0);
            } else if ry + rh > body.bottom() - m {
                ch.info_scroll += ry + rh - (body.bottom() - m);
            } else {
                ch.info_reveal = 0;
            }
        } else {
            ch.info_reveal = 0;
        }
    }
    let content = y + ch.info_scroll - body.y;
    let max = (content - body.h + 8.0).max(0.0);
    // Clamped by the scrollbar (it redraws at once if the content shrank under the offset).
    crate::scrollbar::draw(ch, cx, crate::scrollbar::Area::Info, body, panel, max);
    cx.dl.pop_opacity();
}

/// Under the summary's location: the "Show on map" button and, when the photo records which
/// way the camera faced, a compass with that bearing (north up in every language: it is
/// geography, not reading order). Returns the `y` below them; the button's focus ring goes
/// to `ring`.
fn show_on_map(
    cx: &mut Ctx,
    gps: &specular_metadata::Gps,
    inner: Rect,
    body: Rect,
    y: f32,
    ring: &mut Option<Rect>,
) -> f32 {
    let style = cx.style(metrics::FONT_LABEL).weight(550.0);
    let bh = cx.u(28.0);
    let compass = gps.direction_deg.filter(|d| d.is_finite());
    let room = inner.w - if compass.is_some() { bh + 8.0 } else { 0.0 };
    let label = cx.layout(&t!("app-info-show-on-map"), style, Some(room - 16.0));
    let bh = bh.max(label.height + 8.0);
    let y = y + 4.0;
    if y + bh > body.y && y < body.bottom() {
        let tgt = Target::InfoShowOnMap;
        let r = cx.m_in(
            Rect::new(inner.x, y, (label.width + 24.0).min(room), bh),
            inner,
        );
        let fill = if cx.is_pressed(&tgt) {
            cx.pal.surface_pressed
        } else if cx.is_hover(&tgt) {
            cx.pal.surface_hover
        } else {
            cx.pal.surface_raised
        };
        cx.dl
            .rect_border(r, metrics::RADIUS_SM + 2.0, fill, cx.pal.separator, 1.0);
        cx.block_in(&label, r, HAlign::Center, cx.pal.text);
        cx.hit(r.intersect(&body), tgt.clone());
        if cx.is_focused(&tgt) {
            *ring = Some(r);
        }
        if let Some(deg) = compass {
            let s = cx.u(28.0);
            let c = cx.m_in(
                Rect::new(inner.right() - s, y + (bh - s) * 0.5, s, s),
                inner,
            );
            let (cx0, cy0) = (c.x + s * 0.5, c.y + s * 0.5);
            let rad = s * 0.5 - 1.0;
            cx.dl
                .rect_border(c, s * 0.5, cx.pal.surface_raised, cx.pal.separator, 1.0);
            // North tick, then the bearing (clockwise from north).
            cx.dl
                .line((cx0, c.y + 2.0), (cx0, c.y + 6.0), 1.5, cx.pal.text_faint);
            let a = (deg as f32).to_radians();
            let tip = (cx0 + a.sin() * (rad - 3.0), cy0 - a.cos() * (rad - 3.0));
            cx.dl.line((cx0, cy0), tip, 2.0, cx.pal.accent);
            cx.dl.circle(tip, 2.5, cx.pal.accent);
            cx.dl.circle((cx0, cy0), 2.0, cx.pal.text_muted);
        }
    }
    y + bh + 4.0
}

/// At the foot of the summary: the "Edit metadata…" button (the metadata editor). Returns
/// the `y` below it; its focus ring goes to `ring`.
fn edit_metadata(cx: &mut Ctx, inner: Rect, body: Rect, y: f32, ring: &mut Option<Rect>) -> f32 {
    let style = cx.style(metrics::FONT_LABEL).weight(550.0);
    let label = cx.layout(&t!("app-info-edit-metadata"), style, Some(inner.w - 16.0));
    let bh = cx.u(28.0).max(label.height + 8.0);
    let y = y + 4.0;
    if y + bh > body.y && y < body.bottom() {
        let tgt = Target::InfoEditMetadata;
        let r = cx.m_in(
            Rect::new(inner.x, y, (label.width + 24.0).min(inner.w), bh),
            inner,
        );
        let fill = if cx.is_pressed(&tgt) {
            cx.pal.surface_pressed
        } else if cx.is_hover(&tgt) {
            cx.pal.surface_hover
        } else {
            cx.pal.surface_raised
        };
        cx.dl
            .rect_border(r, metrics::RADIUS_SM + 2.0, fill, cx.pal.separator, 1.0);
        cx.block_in(&label, r, HAlign::Center, cx.pal.text);
        cx.hit(r.intersect(&body), tgt.clone());
        if cx.is_focused(&tgt) {
            *ring = Some(r);
        }
    }
    y + bh + 4.0
}

/// What "Copy value" of the All-fields row (`g`, `f`) puts on the clipboard: the whole value,
/// also when the row shows it shortened.
/// The value of Summary row `i` of group `g`, as shown.
pub(crate) fn summary_text<'a>(st: &ViewerState<'a>, g: usize, i: usize) -> Option<&'a str> {
    Some(&st.metadata?.summary.get(g)?.items.get(i)?.value)
}

pub(crate) fn copy_text<'a>(st: &ViewerState<'a>, g: usize, f: usize) -> Option<&'a str> {
    let md = st.metadata?;
    Some(md.all_fields.get(g)?.fields.get(f)?.copy_text())
}

/// The panel title: `s` wrapped at `w` over at most three lines, or cut in the middle with an
/// ellipsis to fit them (then `true`).
fn title_block(
    cx: &mut Ctx,
    s: &str,
    style: specular_gfx::TextStyle,
    w: f32,
    line_h: f32,
) -> (std::sync::Arc<specular_gfx::TextBlock>, bool) {
    const LINES: f32 = 3.0;
    let b = cx.layout(s, style, Some(w));
    if b.height <= line_h * LINES + 0.5 {
        return (b, false);
    }
    // The longest middle cut that fits. Word joiners keep the line from breaking at the
    // ellipsis, so the cut name flows over the lines like the whole one does.
    let chars: Vec<char> = s.chars().collect();
    let cut = |keep: usize| -> String {
        let mut o: String = chars[..keep.div_ceil(2)].iter().collect();
        o.push_str("\u{2060}…\u{2060}");
        o.extend(chars[chars.len() - keep / 2..].iter());
        o
    };
    let (mut lo, mut hi) = (0, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if cx.layout(&cut(mid), style, Some(w)).height <= line_h * LINES + 0.5 {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    (cx.layout(&cut(lo), style, Some(w)), true)
}

/// The "Performance" section at the end of the summary: a header that expands it (collapsed by
/// default), then why the image took as long as it did to open, a hint, and the stages.
/// Returns the y below it.
fn performance(
    cx: &mut Ctx,
    r: &crate::OpenReport,
    open: bool,
    inner: Rect,
    body: Rect,
    mut y: f32,
) -> f32 {
    let visible = |y: f32, h: f32| y + h > body.y && y < body.bottom();
    let small = cx.style(metrics::FONT_SMALL).break_anywhere();
    let value_style = cx.style(metrics::FONT_BODY).break_anywhere();
    let group_style = cx.style(metrics::FONT_SMALL).weight(650.0);
    let head = Rect::new(inner.x - 6.0, y + 2.0, inner.w + 12.0, cx.u(28.0));
    if visible(head.y, head.h) {
        // Symmetric around `inner`, so the same in both directions.
        let hr = head;
        if cx.is_hover(&Target::InfoPerf) {
            cx.dl.rect(hr, metrics::RADIUS_SM, cx.pal.surface_hover);
        }
        let ic = cx.m_in(
            Rect::new(head.x + 4.0, head.y + (head.h - 16.0) / 2.0, 16.0, 16.0),
            hr,
        );
        draw_icon(
            cx.dl,
            if open { Icon::Minus } else { Icon::Plus },
            ic,
            cx.pal.text_faint,
            metrics::ICON_STROKE,
            false,
        );
        let tr = cx.m_in(Rect::new(head.x + 26.0, head.y, head.w - 30.0, head.h), hr);
        cx.label(
            &t!("open-report-title"),
            group_style,
            tr,
            HAlign::Start,
            cx.pal.text_faint,
        );
        cx.ring_if(&Target::InfoPerf, hr, metrics::RADIUS_SM);
        // Only the part inside the scrolled body takes clicks (not the tabs above it).
        cx.hit(hr.intersect(&body), Target::InfoPerf);
    }
    y += head.h + 8.0;
    if !open {
        return y;
    }
    let hb = cx.layout(&r.headline, value_style, Some(inner.w));
    if visible(y, hb.height) {
        let hr = cx.m_in(Rect::new(inner.x, y, inner.w, hb.height), inner);
        cx.block_top(&hb, hr, HAlign::Start, cx.pal.text);
    }
    y += hb.height + 4.0;
    if let Some(h) = &r.hint {
        let b = cx.layout(h, small, Some(inner.w));
        if visible(y, b.height) {
            let hr = cx.m_in(Rect::new(inner.x, y, inner.w, b.height), inner);
            cx.block_top(&b, hr, HAlign::Start, cx.pal.accent);
        }
        y += b.height + 4.0;
    }
    y += 6.0;
    let label_w = cx.u(112.0).min((inner.w * 0.4).round());
    let vw = inner.w - label_w;
    for (label, value) in &r.rows {
        let lb = cx.layout(label, small, Some(label_w - 8.0));
        let vb = cx.layout(&ltr_values(value, cx.rtl), small, Some(vw));
        let rh = lb.height.max(vb.height) + 6.0;
        if visible(y, rh) {
            let lr = cx.m_in(Rect::new(inner.x, y, label_w - 8.0, lb.height), inner);
            cx.block_top(&lb, lr, HAlign::Start, cx.pal.text_muted);
            let vr = cx.m_in(Rect::new(inner.x + label_w, y, vw, vb.height), inner);
            cx.block_top(&vb, vr, HAlign::Start, cx.pal.text);
        }
        y += rh;
    }
    y + 10.0
}

/// Keeps numbers and units in their written order inside right-to-left UI.
///
/// In an RTL paragraph, neutral characters between numbers take the paragraph direction, so
/// `6000×4000` would show as `4000×6000` and `±0.3 EV` would lose its sign to the far end.
/// A value with no right-to-left letters is isolated as a whole (LRI…PDI). In a value that
/// mixes in localised words, each stretch between RTL letters and brackets that holds a digit
/// (`17127 × 2000`, `1/100`, `7:05`) is isolated on its own, so the words keep their RTL order
/// around it and brackets still pair up. Labels are left alone.
pub(crate) fn ltr_values(s: &str, rtl: bool) -> std::borrow::Cow<'_, str> {
    use specular_i18n::is_rtl_letter;
    use specular_i18n::{LRI, PDI};
    /// Stretches end at anything from an RTL block (letters and also Arabic punctuation such
    /// as "،") and at brackets.
    fn breaks(c: char) -> bool {
        matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF)
            || matches!(c, '(' | ')' | '[' | ']' | '{' | '}' | '\n')
    }
    if !rtl || s.is_empty() {
        return s.into();
    }
    // Already laid out for right to left (parts joined by `specular_i18n::join_parts`: an
    // RLM, each part its own isolate): left as it is, or the parts would merge.
    if s.starts_with(specular_i18n::RLM) && specular_i18n::isolates_balanced(s) {
        return s.into();
    }
    if !s.chars().any(is_rtl_letter) {
        return format!("{LRI}{s}{PDI}").into();
    }
    // Mixed text, e.g. "الثلاثاء، 2 أغسطس 2016، 7:05 م" from specular-metadata's own Fluent
    // bundle: resolve its nested FSIs and make the paragraph right-to-left, as the UI
    // localizer does for its messages.
    let s = specular_i18n::for_rtl(s);
    let mut out = String::with_capacity(s.len() + 16);
    let mut rest = s.as_str();
    while !rest.is_empty() {
        // A run of breaking characters, copied as is.
        let n = rest.find(|c: char| !breaks(c)).unwrap_or(rest.len());
        out.push_str(&rest[..n]);
        rest = &rest[n..];
        // A stretch without them: isolate its core if it holds a digit. The core leaves out
        // surrounding spaces and any isolate controls (Fluent puts FSI…PDI around each
        // placeable) whose partner is outside the stretch, so that isolates stay nested.
        let n = rest.find(breaks).unwrap_or(rest.len());
        let seg = &rest[..n];
        let (a, b) = core_span(seg);
        let core = &seg[a..b];
        // Already one left-to-right isolate (numbers joined by `×` or `·`, from `for_rtl`).
        let isolated = core
            .strip_prefix(LRI)
            .and_then(|c| c.strip_suffix(PDI))
            .is_some_and(balanced);
        if core.chars().any(|c| c.is_ascii_digit()) && balanced(core) && !isolated {
            out.push_str(&seg[..a]);
            out.push(LRI);
            out.push_str(core);
            out.push(PDI);
            out.push_str(&seg[b..]);
        } else {
            out.push_str(seg);
        }
        rest = &rest[n..];
    }
    out.into()
}

fn is_initiator(c: char) -> bool {
    matches!(c, '\u{2066}'..='\u{2068}')
}

/// Whether every isolate initiator in `s` has its PDI in `s` and the other way round.
fn balanced(s: &str) -> bool {
    let mut depth = 0u32;
    for c in s.chars() {
        if is_initiator(c) {
            depth += 1;
        } else if c == '\u{2069}' {
            let Some(d) = depth.checked_sub(1) else {
                return false;
            };
            depth = d;
        }
    }
    depth == 0
}

/// Separators between values (`, ` `; `): they belong to the right-to-left sentence around a
/// value, not inside its left-to-right isolate, or they show on the number's far side.
fn separator(c: char) -> bool {
    c.is_whitespace() || matches!(c, ',' | ';' | '،' | '؛')
}

/// Byte range of `seg` without leading/trailing whitespace, separators and unpaired isolate
/// controls.
fn core_span(seg: &str) -> (usize, usize) {
    let (mut a, mut b) = (0, seg.len());
    loop {
        let core = &seg[a..b];
        let t = core.trim_start_matches(separator);
        if t.len() != core.len() {
            a += core.len() - t.len();
            continue;
        }
        let t = core.trim_end_matches(separator);
        if t.len() != core.len() {
            b -= core.len() - t.len();
            continue;
        }
        let mut cs = core.chars();
        if let Some(c) = cs.next()
            && (c == '\u{2069}' || (is_initiator(c) && !closes_within(core)))
        {
            a += c.len_utf8();
            continue;
        }
        if let Some(c) = core.chars().next_back()
            && (is_initiator(c) || (c == '\u{2069}' && !opens_within(core)))
        {
            b -= c.len_utf8();
            continue;
        }
        return (a, b);
    }
}

/// `s` starts with an initiator: is its PDI inside `s`?
fn closes_within(s: &str) -> bool {
    let mut depth = 0u32;
    for c in s.chars() {
        if is_initiator(c) {
            depth += 1;
        } else if c == '\u{2069}' {
            depth -= 1;
            if depth == 0 {
                return true;
            }
        }
    }
    false
}

/// `s` ends with a PDI: is its initiator inside `s`?
fn opens_within(s: &str) -> bool {
    let mut depth = 0u32;
    for c in s.chars().rev() {
        if c == '\u{2069}' {
            depth += 1;
        } else if is_initiator(c) {
            depth -= 1;
            if depth == 0 {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{balanced, ltr_values};
    use specular_i18n::is_rtl_letter;

    #[test]
    fn isolates_values_only_in_rtl() {
        assert_eq!(ltr_values("6000×4000", false), "6000×4000");
        assert_eq!(ltr_values("6000×4000", true), "\u{2066}6000×4000\u{2069}");
        // Parts joined for right to left keep their isolates.
        let joined = specular_i18n::join_parts(
            &["35 mm f/1.8", "עדשה 2"],
            " · ",
            specular_i18n::Direction::Rtl,
        );
        assert_eq!(ltr_values(&joined, true), joined);
        assert_eq!(ltr_values("1/100 ث", true), "\u{2066}1/100\u{2069} ث");
        assert_eq!(
            ltr_values("17127 × 2000 (34.3 ميغابكسل)", true),
            "\u{2066}17127 × 2000\u{2069} (\u{2066}34.3\u{2069} ميغابكسل)"
        );
        // Fluent's FSI…PDI around numbers become LRI and stay inside the stretch's LRI (one
        // already isolated alone is not wrapped again).
        assert_eq!(
            ltr_values(
                "\u{2068}6000\u{2069} × \u{2068}4000\u{2069} (\u{2068}24\u{2069} ميغابكسل)",
                true
            ),
            "\u{2066}\u{2066}6000\u{2069} × \u{2066}4000\u{2069}\u{2069} (\u{2066}24\u{2069} ميغابكسل)"
        );
    }

    #[test]
    fn isolate_pairs_are_not_split() {
        // Fluent's nesting for "date-time": FSI[FSI(weekday)، FSI(date)]، FSI(time). The
        // stretch "2016" + PDI + PDI ends with PDIs of isolates opened before it; they stay outside the
        // new LRI, and FSIs holding Arabic become RLI.
        let s = "\u{2068}\u{2068}الثلاثاء\u{2069}، \u{2068}2 أغسطس 2016\u{2069}\u{2069}، \u{2068}7:05 م\u{2069}";
        let out = ltr_values(s, true);
        assert!(balanced(&out), "{out:?}");
        assert_eq!(
            out,
            "\u{200F}\u{2067}\u{2067}الثلاثاء\u{2069}، \u{2067}\u{2066}2\u{2069} أغسطس \u{2066}2016\u{2069}\u{2069}\u{2069}، \u{2067}\u{2066}7:05\u{2069} م\u{2069}"
        );
    }

    /// Separators between values stay outside the values' isolates, so in a right-to-left
    /// sentence ", " follows the word before it (shaped and read back in visual order).
    #[test]
    fn separators_stay_with_the_rtl_sentence() {
        let s = "Ground: 1,234 נקודות, 2,345 משולשים; שטח בהיטל 24,612 m², שטח פנים 25,001 m²";
        let out = ltr_values(s, true);
        assert!(balanced(&out), "{out:?}");
        assert!(out.contains("\u{2066}2,345\u{2069} "), "{out:?}");
        assert!(out.contains("\u{2066}24,612 m²\u{2069},"), "{out:?}");
        let mut text = specular_gfx::Text::new();
        let v = text.visual_order(&out);
        // Visual order, left to right: each comma sits at the left end of the word it follows
        // (read first, at the right) and the numbers keep their digits' order.
        assert!(v.contains(",תודוקנ"), "{v:?}");
        assert!(v.contains("2,345 ,"), "{v:?}");
        assert!(v.contains("24,612 m²"), "{v:?}");
        assert!(!v.contains(", 2,345") && !v.contains(", 25,001"), "{v:?}");
    }

    /// Real catalogues: Fluent and ICU4X → `ltr_values` → parley, read back in visual order.
    #[test]
    fn metadata_values_read_correctly_in_rtl() {
        use specular_metadata::{i18n::Localizer, strings};
        let mut text = specular_gfx::Text::new();
        for lang in ["ar", "he"] {
            let l = Localizer::new(lang.parse().unwrap());

            // Width before height, left to right; in ar the value is read first, so it is
            // at the right.
            let dims = strings::dimensions(&l, 17127, 2000);
            let shown = ltr_values(&dims, true);
            assert!(balanced(&shown), "{lang}: {shown:?}");
            let v = text.visual_order(&shown);
            assert!(v.contains("17127 × 2000"), "{lang}: {v:?}");
            assert!(v.contains("34.3"), "{lang}: {v:?}");
            if lang == "ar" {
                assert!(v.ends_with("17127 × 2000"), "{lang}: {v:?}");
            }

            // "<weekday>، 2 <month> 2016، 7:05 م", read right to left: the weekday at the
            // right, then the day, the year and the time at the left.
            for off in [None, Some(60)] {
                let dt = strings::date_time(&l, 2016, 8, 2, 19, 5, off).unwrap();
                let shown = ltr_values(&dt, true);
                assert!(balanced(&shown), "{lang}: {shown:?}");
                let v = text.visual_order(&shown);
                let time = if lang == "ar" { "7:05" } else { "19:05" };
                let (t, y) = (v.find(time).unwrap(), v.find("2016").unwrap());
                let day = v.rfind('2').unwrap();
                assert!(t < y && y < day, "{lang}: {v:?}");
                let last = v.trim_end().chars().last().unwrap();
                assert!(is_rtl_letter(last), "{lang}: weekday not first: {v:?}");
                if off.is_some() {
                    assert!(v.contains("UTC+1"), "{lang}: {v:?}");
                }
            }
        }
    }
}
