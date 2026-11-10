//! Two dialogs about problems, both local only:
//!
//! - the crash prompt at the start after a crash: what happened, the report on demand, Copy,
//!   Save diagnostics bundle…, Dismiss;
//! - the diagnostics bundle: exactly what the zip will hold (each file, its size, and its
//!   text when selected), "Include file names" (off by default), the privacy note, where
//!   to send it, and Save…. Nothing is sent anywhere.

use std::sync::Arc;

use specular_gfx::{Icon, Rect, TextBlock, draw_icon};
use specular_i18n::{ByteUnits, I18n, t};
use specular_theme::metrics;

use crate::ctx::{Ctx, HAlign, Target};
use crate::{Chrome, Command, Modal, Response};

/// The crash prompt's content.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CrashSheet {
    /// The report, redacted (as it would go into the bundle).
    pub report: String,
}

/// One file of the diagnostics bundle, as previewed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiagEntry {
    pub name: String,
    /// What it holds, in the UI language.
    pub description: String,
    /// Exactly what is saved.
    pub text: String,
}

/// The diagnostics dialog's content ([`Chrome::show_diagnostics`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiagSheet {
    pub entries: Vec<DiagEntry>,
    /// File names are kept (else replaced by `file-1.jpg`…).
    pub include_names: bool,
    /// Where to report a problem and the support address (shown, never contacted).
    pub issues: String,
    pub email: String,
}

#[derive(Default)]
pub(crate) struct CrashState {
    pub sheet: CrashSheet,
    pub show_report: bool,
    pub scroll: f32,
}

#[derive(Default)]
pub(crate) struct DiagState {
    pub sheet: DiagSheet,
    /// The entry previewed.
    pub selected: usize,
    pub scroll: f32,
    /// The file list's scroll (when it holds more rows than fit) and where it was drawn.
    pub list_scroll: f32,
    pub list: Option<Rect>,
    /// Scroll the list to the selected row on the next draw (keyboard moves).
    pub reveal: bool,
    /// The row with the keyboard focus at the last draw (a newly focused row is scrolled to).
    pub focused: Option<usize>,
}

impl Chrome {
    /// After a crash: offers the report (next start).
    pub fn show_crash(&mut self, sheet: CrashSheet) {
        self.crash = CrashState {
            sheet,
            ..CrashState::default()
        };
        self.open_modal(Modal::Crash);
    }

    /// Opens the diagnostics dialog with this preview.
    pub fn show_diagnostics(&mut self, sheet: DiagSheet) {
        self.diag = DiagState {
            sheet,
            ..DiagState::default()
        };
        self.open_modal(Modal::Diagnostics);
    }

    /// Replaces the preview (after "Include file names" changed); the entry shown stays.
    pub fn set_diagnostics(&mut self, sheet: DiagSheet) {
        if self.modal != Some(Modal::Diagnostics)
            && !self.modal_parents.iter().any(|p| p.0 == Modal::Diagnostics)
        {
            return;
        }
        self.diag.selected = self
            .diag
            .selected
            .min(sheet.entries.len().saturating_sub(1));
        self.diag.sheet = sheet;
    }

    pub(crate) fn crash_click(&mut self, t: &Target) -> Response {
        match t {
            Target::CrashView => {
                self.crash.show_report ^= true;
                self.crash.scroll = 0.0;
                Response::used()
            }
            Target::CrashCopy => {
                self.toast(t!("crash-copied"), None, self.last_now);
                Response::cmd(Command::CopyText(self.crash.sheet.report.clone()))
            }
            Target::CrashSave => {
                self.close_modal();
                Response::cmd(Command::Action(specular_input::Action::SaveDiagnostics))
            }
            _ => {
                self.close_modal();
                Response::used()
            }
        }
    }

    pub(crate) fn diag_click(&mut self, t: &Target) -> Response {
        match t {
            Target::DiagEntry(i) => {
                self.diag.selected = *i;
                self.diag.scroll = 0.0;
                Response::used()
            }
            Target::DiagNames => {
                let on = !self.diag.sheet.include_names;
                self.diag.sheet.include_names = on;
                Response::cmd(Command::DiagNames(on))
            }
            Target::DiagSave => {
                let names = self.diag.sheet.include_names;
                self.close_modal();
                Response::cmd(Command::DiagSave {
                    include_names: names,
                })
            }
            _ => {
                self.close_modal();
                Response::used()
            }
        }
    }

    /// The Tab order of the two dialogs.
    pub(crate) fn crash_stops(&self) -> Vec<Target> {
        vec![
            Target::CrashView,
            Target::CrashCopy,
            Target::CrashSave,
            Target::CrashDismiss,
            Target::ModalClose,
        ]
    }

    pub(crate) fn diag_stops(&self) -> Vec<Target> {
        let mut v: Vec<Target> = (0..self.diag.sheet.entries.len())
            .map(Target::DiagEntry)
            .collect();
        v.extend([
            Target::DiagNames,
            Target::DiagCancel,
            Target::DiagSave,
            Target::ModalClose,
        ]);
        v
    }
}

fn button(cx: &mut Ctx, r: Rect, b: &TextBlock, t: Target, primary: bool) {
    crate::insights::button(cx, r, b, t.clone(), primary);
    if cx.is_focused(&t) {
        cx.button_ring(r, metrics::RADIUS_SM + 2.0);
    }
}

/// A row of buttons at the end of `r`'s bottom; returns its top.
fn buttons(cx: &mut Ctx, r: Rect, list: &[(String, Target, bool)]) -> f32 {
    let s = cx.style(metrics::FONT_LABEL);
    let bh = cx.u(32.0);
    let blocks: Vec<Arc<TextBlock>> = list.iter().map(|(l, _, _)| cx.layout(l, s, None)).collect();
    let mut x = r.right() - 20.0;
    let y = r.bottom() - 20.0 - bh;
    for ((_, t, primary), b) in list.iter().zip(&blocks).rev() {
        let w = b.width + 24.0;
        x -= w;
        let br = cx.m_in(Rect::new(x, y, w, bh), r);
        button(cx, br, b, t.clone(), *primary);
        x -= 8.0;
    }
    y
}

/// Text in a bordered box that scrolls (`scroll` is clamped here).
fn text_box(cx: &mut Ctx, area: Rect, text: &str, scroll: &mut f32) -> f32 {
    cx.dl.rect_border(
        area,
        metrics::RADIUS_SM + 1.0,
        cx.pal.surface_raised,
        cx.pal.separator,
        1.0,
    );
    let inner = area.inset(10.0);
    // Monospaced so the report's `key:   value` columns line up; long paths and backtrace
    // frames break anywhere instead of running past the box.
    let mono = cx.style(metrics::FONT_MONO).code().break_anywhere();
    // Long reports: the first lines that fit a few screens (the file holds all of it).
    let shown: String = text.lines().take(400).collect::<Vec<_>>().join("\n");
    let b = cx.layout(&shown, mono, Some(inner.w));
    let max = (b.height - inner.h).max(0.0);
    *scroll = scroll.clamp(0.0, max);
    cx.dl.push_clip(inner);
    let r = Rect::new(inner.x, inner.y - *scroll, inner.w, b.height);
    cx.block_top(&b, r, HAlign::Start, cx.pal.text);
    cx.dl.pop_clip();
    max
}

pub(crate) fn draw_crash(ch: &mut Chrome, cx: &mut Ctx) {
    let w = (cx.w - 48.0).min(cx.u(620.0));
    let inner_w = w - 48.0;
    let title_s = cx.style(metrics::FONT_TITLE).weight(650.0);
    let body_s = cx.style(metrics::FONT_BODY);
    let title = cx.layout(&t!("crash-title"), title_s, Some(inner_w - 40.0));
    let body = cx.layout(&t!("crash-body"), body_s, Some(inner_w));
    let bh = cx.u(32.0);
    let report_h = if ch.crash.show_report {
        (cx.h * 0.45).max(120.0)
    } else {
        0.0
    };
    let content = title.height
        + 12.0
        + body.height
        + if report_h > 0.0 { 14.0 + report_h } else { 0.0 }
        + 20.0
        + bh;
    let h = (content + 48.0).min(cx.h - 32.0);
    let r = Rect::new(((cx.w - w) * 0.5).round(), ((cx.h - h) * 0.5).round(), w, h);
    cx.sheet(r);
    let close = cx.m_in(Rect::new(r.right() - 46.0, r.y + 14.0, 32.0, 32.0), r);
    cx.close_button(close);
    let ir = cx.m_in(Rect::new(r.x + 24.0, r.y + 24.0, 22.0, 22.0), r);
    draw_icon(cx.dl, Icon::Alert, ir, cx.pal.warning, 1.8, false);
    let tr = cx.m_in(
        Rect::new(r.x + 54.0, r.y + 24.0, inner_w - 70.0, title.height),
        r,
    );
    cx.block_top(&title, tr, HAlign::Start, cx.pal.text);
    let y = r.y + 24.0 + title.height + 12.0;
    let br = cx.m_in(Rect::new(r.x + 24.0, y, inner_w, body.height), r);
    cx.block_top(&body, br, HAlign::Start, cx.pal.text_muted);
    let view = if ch.crash.show_report {
        t!("crash-hide")
    } else {
        t!("crash-view")
    };
    let top = buttons(
        cx,
        r,
        &[
            (view, Target::CrashView, false),
            (t!("crash-copy"), Target::CrashCopy, false),
            (t!("crash-save-bundle"), Target::CrashSave, false),
            (t!("crash-dismiss"), Target::CrashDismiss, true),
        ],
    );
    if ch.crash.show_report {
        let area = Rect::new(
            r.x + 24.0,
            y + body.height + 14.0,
            inner_w,
            (top - 20.0 - (y + body.height + 14.0)).max(40.0),
        );
        let report = ch.crash.sheet.report.clone();
        text_box(cx, area, &report, &mut ch.crash.scroll);
        cx.hit(area, Target::ModalBody);
    }
}

pub(crate) fn draw_diag(ch: &mut Chrome, cx: &mut Ctx) {
    let w = (cx.w - 48.0).min(cx.u(760.0));
    let h = (cx.h - 48.0).min(cx.h * 0.92).max(260.0);
    let r = Rect::new(((cx.w - w) * 0.5).round(), ((cx.h - h) * 0.5).round(), w, h);
    let inner = Rect::new(r.x + 24.0, r.y + 24.0, w - 48.0, h - 48.0);
    cx.sheet(r);
    let close = cx.m_in(Rect::new(r.right() - 46.0, r.y + 14.0, 32.0, 32.0), r);
    cx.close_button(close);
    let title_s = cx.style(metrics::FONT_TITLE).weight(650.0);
    let body_s = cx.style(metrics::FONT_BODY);
    let small = cx.style(metrics::FONT_SMALL);
    let head_s = cx.style(metrics::FONT_SMALL).weight(650.0);
    let s = &mut ch.diag;

    // From the top: the title, what the bundle is, "Contents".
    let mut y = inner.y;
    let text = |cx: &mut Ctx, y: &mut f32, s: &str, style, color, w: f32, gap: f32| {
        let b = cx.layout(s, style, Some(w));
        let br = cx.m_in(Rect::new(inner.x, *y, w, b.height), r);
        cx.block_top(&b, br, HAlign::Start, color);
        *y += b.height + gap;
    };
    text(
        cx,
        &mut y,
        &t!("diag-title"),
        title_s,
        cx.pal.text,
        inner.w - 40.0,
        8.0,
    );
    text(
        cx,
        &mut y,
        &t!("diag-intro"),
        body_s,
        cx.pal.text_muted,
        inner.w,
        12.0,
    );
    text(
        cx,
        &mut y,
        &t!("diag-contents"),
        head_s,
        cx.pal.text_faint,
        inner.w,
        6.0,
    );

    // From the bottom: the buttons, where to send it, the privacy note, the names switch;
    // each part on lines of its own with space between.
    let top = buttons(
        cx,
        r,
        &[
            (t!("diag-cancel"), Target::DiagCancel, false),
            (t!("diag-save"), Target::DiagSave, true),
        ],
    );
    let privacy = cx.layout(&t!("diag-privacy"), small, Some(inner.w));
    let where_ = cx.layout(
        &t!(
            "diag-where",
            issues = s.sheet.issues.clone(),
            email = s.sheet.email.clone()
        ),
        small,
        Some(inner.w),
    );
    let wy = top - 14.0 - where_.height;
    let wr = cx.m_in(Rect::new(inner.x, wy, inner.w, where_.height), r);
    cx.block_top(&where_, wr, HAlign::Start, cx.pal.text_muted);
    let py = wy - 8.0 - privacy.height;
    let pr = cx.m_in(Rect::new(inner.x, py, inner.w, privacy.height), r);
    cx.block_top(&privacy, pr, HAlign::Start, cx.pal.text_muted);

    let tgt = Target::DiagNames;
    let label = cx.layout(
        &t!("diag-include-names"),
        cx.style(metrics::FONT_LABEL),
        None,
    );
    let help = cx.layout(&t!("diag-include-names-help"), small, Some(inner.w - 30.0));
    let ch_h = label.height + 2.0 + help.height;
    let cy = py - 14.0 - ch_h;
    let cr = cx.m_in(
        Rect::new(inner.x - 6.0, cy - 4.0, inner.w + 12.0, ch_h + 8.0),
        r,
    );
    if cx.is_hover(&tgt) {
        cx.dl.rect(cr, metrics::RADIUS_SM, cx.pal.surface_hover);
    }
    let bx = cx.m_in(
        Rect::new(inner.x, cy + (label.height - 18.0) * 0.5, 18.0, 18.0),
        r,
    );
    if s.sheet.include_names {
        cx.dl.rect(bx, metrics::RADIUS_SM, cx.pal.accent_fill);
        draw_icon(
            cx.dl,
            Icon::Check,
            bx.inset(2.0),
            cx.pal.accent_text,
            2.0,
            false,
        );
    } else {
        cx.dl.rect_border(
            bx,
            metrics::RADIUS_SM,
            cx.pal.surface_raised,
            cx.pal.text_muted,
            1.5,
        );
    }
    let lr = cx.m_in(
        Rect::new(inner.x + 28.0, cy, inner.w - 28.0, label.height),
        r,
    );
    cx.block_top(&label, lr, HAlign::Start, cx.pal.text);
    let hr = cx.m_in(
        Rect::new(
            inner.x + 28.0,
            cy + label.height + 2.0,
            inner.w - 30.0,
            help.height,
        ),
        r,
    );
    cx.block_top(&help, hr, HAlign::Start, cx.pal.text_muted);
    cx.ring_if(&tgt, cr, metrics::RADIUS_SM);
    cx.hit(cr, tgt);

    // Between: the files (name and size) beside the selected one's text, exactly as saved.
    let area_h = (cy - 14.0 - y).max(0.0);
    let list_w = cx.u(260.0).min(inner.w * 0.42);
    // Every file listed: shorter rows when the window is short, then a scrolling list.
    let n = s.sheet.entries.len().max(1) as f32;
    let row_h = cx.u(26.0).min((area_h / n).floor()).max(cx.u(18.0));
    let loc = I18n::global().localizer();
    let mono = small.tabular();
    let size_w = cx.u(72.0);
    let list = Rect::new(inner.x - 6.0, y, list_w, area_h);
    let max = (n * row_h - area_h).max(0.0);
    let focused = (0..s.sheet.entries.len()).find(|&i| cx.is_focused(&Target::DiagEntry(i)));
    let newly = focused.filter(|_| focused != s.focused);
    s.focused = focused;
    if s.reveal || newly.is_some() {
        let top = newly.unwrap_or(s.selected) as f32 * row_h;
        s.list_scroll = s.list_scroll.clamp((top + row_h - area_h).max(0.0), top);
        s.reveal = false;
    }
    s.list_scroll = s.list_scroll.clamp(0.0, max);
    s.list = Some(cx.m_in(list, r));
    cx.dl.push_clip(list);
    for (i, e) in s.sheet.entries.iter().enumerate() {
        let ry = y + i as f32 * row_h - s.list_scroll;
        if ry + row_h < y || ry > y + area_h {
            continue;
        }
        let rr = cx.m_in(Rect::new(inner.x - 6.0, ry, list_w, row_h), r);
        let t = Target::DiagEntry(i);
        if i == s.selected {
            cx.dl.rect(rr, metrics::RADIUS_SM + 1.0, cx.pal.accent_soft);
        } else if cx.is_hover(&t) {
            cx.dl
                .rect(rr, metrics::RADIUS_SM + 1.0, cx.pal.surface_hover);
        }
        let nr = cx.m_in(Rect::new(inner.x, ry, list_w - size_w - 12.0, row_h), r);
        cx.label_fit(&e.name, mono, nr, HAlign::Start, cx.pal.text, true);
        let size = t!(
            "diag-size",
            size = loc.bytes(e.text.len() as u64, ByteUnits::Decimal)
        );
        let sr = cx.m_in(
            Rect::new(inner.x - 12.0 + list_w - size_w, ry, size_w, row_h),
            r,
        );
        cx.label_fit(&size, mono, sr, HAlign::End, cx.pal.text_muted, false);
        cx.ring_if(&t, rr, metrics::RADIUS_SM + 1.0);
        cx.hit(rr.intersect(&list), t);
    }
    cx.dl.pop_clip();
    let px = inner.x + list_w + 8.0;
    let pw = inner.right() - px;
    if let Some(e) = s.sheet.entries.get(s.selected)
        && area_h >= 40.0
    {
        let cap = cx.layout(&e.description, small, Some(pw));
        let capr = cx.m_in(Rect::new(px, y, pw, cap.height), r);
        cx.block_top(&cap, capr, HAlign::Start, cx.pal.text_muted);
        let box_y = y + cap.height + 6.0;
        let area = cx.m_in(Rect::new(px, box_y, pw, (y + area_h - box_y).max(20.0)), r);
        let text = e.text.clone();
        text_box(cx, area, &text, &mut s.scroll);
        cx.hit(area, Target::ModalBody);
    }
}
