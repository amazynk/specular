//! The DICOMweb sheet as data, its drawing and its accessibility tree. Nothing here holds app
//! state, so the screenshot tests draw it without an app.
//!
//! The sheet has three pages ([`Page`]): the servers, a server's settings, and the study
//! search. Each is a form (text fields, a choice, a switch, laid out in one or two columns),
//! a list (servers, or studies with their series under the one opened), notes, a status line
//! with an optional progress bar, and buttons at the end edge.
//!
//! The list is meant to merge with the Studies browser of patient CDs (the patient CD browser):
//! [`ListEntry`] carries only display text, depth and state; the rows themselves are built
//! by the page from rows any source can give (QIDO-RS or a DICOMDIR).

use std::sync::Arc;

use specular_gfx::{Align, DrawList, Icon, Rect, TexId, Text, TextBlock, TextStyle, draw_icon};
use specular_i18n::t;
use specular_theme::metrics::{FONT_LABEL, FONT_SMALL, FONT_TITLE, RADIUS_LG, RADIUS_SM};
use specular_theme::{Color, Palette};
use specular_widgets::accesskit;

pub(in crate::app) const A11Y_BASE: u64 = 0x5600_0000_0000_0000;

/// The sheet's pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Page {
    Servers,
    Edit,
    Search,
    /// Send to PACS.
    Send,
}

/// The text fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in crate::app) enum FieldId {
    Name,
    Url,
    User,
    Secret,
    PatientName,
    PatientId,
    DateFrom,
    DateTo,
    Modality,
    Accession,
    /// DIMSE servers: host, port, the server's AE title and ours.
    Host,
    Port,
    AeTitle,
    OurAe,
    /// C-MOVE: the address and port our storage SCP listens on.
    ReceiveAddress,
    ReceivePort,
}

/// What a form cell is.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) enum Control {
    Field {
        id: FieldId,
        text: String,
        caret: usize,
        selection: (usize, usize),
        /// Passwords and tokens: drawn as dots, not exposed to screen readers.
        masked: bool,
        /// Shown in the empty field.
        hint: String,
        /// What an input method is composing (IME preedit), shown at the caret when the
        /// field has the keyboard.
        preedit: String,
    },
    /// The kind of credentials: steps through its values.
    Choice { value: String },
    /// Show patient data.
    Switch { on: bool },
}

/// One cell of the form.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct FormItem {
    pub label: String,
    pub control: Control,
}

/// One row of the list.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::app) struct ListEntry {
    pub title: String,
    pub detail: String,
    /// 0: a server or study; 1: a series under its study.
    pub depth: u8,
    pub selected: bool,
    /// A study whose series are shown under it.
    pub expanded: bool,
    /// "Load more" and similar rows: drawn as a link.
    pub action: bool,
    /// A picture at the row's start (the OMERO sheet's image thumbnails).
    pub thumb: Thumb,
}

/// A list row's picture.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(in crate::app) enum Thumb {
    /// None (and no room kept for one).
    #[default]
    None,
    /// Room kept, the picture still on its way.
    Pending,
    /// A texture, its size in pixels.
    Shown { tex: TexId, w: u32, h: u32 },
}

/// How a note reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Tone {
    Plain,
    Warning,
    Error,
}

/// The buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in crate::app) enum Button {
    Close,
    Add,
    Edit,
    Remove,
    Browse,
    Cancel,
    Save,
    Back,
    Search,
    Stop,
    /// Servers: C-ECHO or a one-study search.
    Verify,
    /// Send page: to the confirmation, then the send itself.
    Send,
    Confirm,
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct ButtonView {
    pub id: Button,
    pub label: String,
    pub enabled: bool,
    /// Filled: what Enter does.
    pub default: bool,
}

/// What a click or a screen reader hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Hit {
    Form(usize),
    Entry(usize),
    Button(Button),
}

/// Everything the sheet draws.
#[derive(Clone, Debug)]
pub(in crate::app) struct DialogView {
    pub page: Page,
    pub title: String,
    /// Under the title: the server searched, or a short explanation.
    pub subtitle: Option<String>,
    pub form: Vec<FormItem>,
    /// The list's heading ("Studies"), when it has one.
    pub list_title: Option<String>,
    pub list: Vec<ListEntry>,
    /// Shown in place of an empty list.
    pub empty: Option<String>,
    /// The first list row drawn (the list scrolls to keep the focus in view).
    pub scroll: usize,
    pub notes: Vec<(String, Tone)>,
    pub status: Option<(String, Tone)>,
    pub progress: Option<f32>,
    pub buttons: Vec<ButtonView>,
    /// Index into [`focusables`](DialogView::focusables).
    pub focus: Option<usize>,
}

impl DialogView {
    /// The keyboard focus order: the form, the list, then the buttons.
    pub fn focusables(&self) -> Vec<Hit> {
        (0..self.form.len())
            .map(Hit::Form)
            .chain((0..self.list.len()).map(Hit::Entry))
            .chain(
                self.buttons
                    .iter()
                    .filter(|b| b.enabled)
                    .map(|b| Hit::Button(b.id)),
            )
            .collect()
    }

    pub fn focused(&self) -> Option<Hit> {
        self.focus.and_then(|i| self.focusables().get(i).copied())
    }

    /// The field with the keyboard, if any.
    pub fn focused_field(&self) -> Option<FieldId> {
        match self.focused()? {
            Hit::Form(i) => match &self.form.get(i)?.control {
                Control::Field { id, .. } => Some(*id),
                _ => None,
            },
            _ => None,
        }
    }
}

/// Where things landed.
#[derive(Clone, Debug, Default)]
pub(in crate::app) struct Drawn {
    pub sheet: Rect,
    pub hits: Vec<(Rect, Hit)>,
    /// The focused text field: its laid-out text and the x it is drawn at.
    pub field_text: Option<(Arc<TextBlock>, f32)>,
    pub field: Option<Rect>,
    /// Where the input method's window opens: the composition and the caret.
    pub ime: Option<Rect>,
    /// List rows that fit.
    pub visible: usize,
}

#[derive(Clone, Copy)]
struct Mirror {
    ww: f32,
    rtl: bool,
}

impl Mirror {
    fn r(self, r: Rect) -> Rect {
        if self.rtl { r.mirror_x(self.ww) } else { r }
    }
    fn x(self, x: f32, w: f32) -> f32 {
        if self.rtl { self.ww - x - w } else { x }
    }
}

fn tone_color(pal: &Palette, tone: Tone) -> Color {
    match tone {
        Tone::Plain => pal.text_muted,
        Tone::Warning => pal.warning,
        Tone::Error => pal.danger,
    }
}

/// Dots for a masked field: one per character.
pub(in crate::app) fn masked(s: &str) -> String {
    "•".repeat(s.chars().count())
}

/// A byte offset of `s` as the offset into its [`masked`] form.
fn masked_byte(s: &str, b: usize) -> usize {
    s[..b.min(s.len())].chars().count() * '•'.len_utf8()
}

/// The sheet's width in a window `ww` wide.
fn sheet_width(ww: f32, ts: f32) -> f32 {
    (ww - 32.0).clamp(260.0, 560.0 + 80.0 * (ts - 1.0).max(0.0))
}

/// Draws the sheet over a scrim in a window of `size` logical px.
pub(in crate::app) fn draw(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    v: &DialogView,
    size: (f32, f32),
    rtl: bool,
    ts: f32,
) -> Drawn {
    let (ww, wh) = size;
    dl.rect(Rect::new(0.0, 0.0, ww, wh), 0.0, pal.scrim);
    let pad = 20.0;
    let w = sheet_width(ww, ts).min(ww);
    let inner = w - 2.0 * pad;
    let mi = Mirror { ww, rtl };

    let title_st = TextStyle::new(FONT_TITLE * ts).weight(650.0);
    let label_st = TextStyle::new(FONT_LABEL * ts);
    let strong_st = TextStyle::new(FONT_LABEL * ts).weight(550.0);
    let small_st = TextStyle::new(FONT_SMALL * ts);

    let title = text.layout(&v.title, title_st, Some(inner), Align::Start);
    let subtitle = v
        .subtitle
        .as_ref()
        .map(|s| text.layout(s, small_st, Some(inner), Align::Start));
    // Two columns of form cells when they fit.
    let cols = if inner >= 400.0 * ts.clamp(1.0, 1.5) && v.form.len() > 1 {
        2
    } else {
        1
    };
    let gap = 12.0;
    let cell_w = (inner - gap * (cols as f32 - 1.0)) / cols as f32;
    let label_h = text.layout("Ag", small_st, None, Align::Start).height;
    let field_h = (26.0 * ts).max(30.0);
    let cell_h = label_h + 4.0 + field_h;
    let all_rows = v.form.len().div_ceil(cols);
    let form_height = |rows: usize| {
        if rows == 0 {
            0.0
        } else {
            rows as f32 * cell_h + (rows - 1) as f32 * 10.0 + 14.0
        }
    };
    let list_title = v
        .list_title
        .as_ref()
        .map(|s| text.layout(s, strong_st, Some(inner), Align::Start));
    let line_h = text.layout("Ag", label_st, None, Align::Start).height;
    let small_h = label_h;
    let entry_h = (line_h + small_h + 12.0).max(40.0);
    let notes: Vec<_> = v
        .notes
        .iter()
        .map(|(s, tone)| (text.layout(s, small_st, Some(inner), Align::Start), *tone))
        .collect();
    let status = v
        .status
        .as_ref()
        .map(|(s, tone)| (text.layout(s, label_st, Some(inner), Align::Start), *tone));
    let empty = v
        .empty
        .as_ref()
        .filter(|_| v.list.is_empty())
        .map(|s| text.layout(s, label_st, Some(inner - 16.0), Align::Start));
    let bh = (24.0 * ts).max(32.0);

    // Everything but the form and the list; then the form rows, and as many list rows as fit.
    let mut fixed = pad + title.height + 10.0;
    if let Some(s) = &subtitle {
        fixed += s.height + 10.0;
    }
    if let Some(l) = &list_title {
        fixed += l.height + 8.0;
    }
    for (n, _) in &notes {
        fixed += n.height + 6.0;
    }
    if let Some((s, _)) = &status {
        fixed += s.height + 10.0 + if v.progress.is_some() { 10.0 } else { 0.0 };
    }
    fixed += 18.0 + bh + pad;
    let avail = wh - 16.0;
    let has_list = !v.list.is_empty() || empty.is_some() || list_title.is_some();
    let min_list = if !has_list {
        0.0
    } else if let Some(e) = &empty {
        e.height + 24.0
    } else {
        v.list.len().clamp(1, 3) as f32 * entry_h + 8.0
    };
    // With large text in a small window, the form shows the rows that fit around the
    // focused field (the rest come into view as the focus moves).
    let mut rows = all_rows;
    while rows > 1 && fixed + form_height(rows) + min_list > avail {
        rows -= 1;
    }
    let first_row = match focus_form(v) {
        Some(i) if rows < all_rows => (i / cols + 1).saturating_sub(rows),
        _ => 0,
    };
    let form_h = form_height(rows);
    fixed += form_h;
    let room = (avail - fixed - 8.0).max(0.0);
    let visible = if v.list.is_empty() {
        0
    } else {
        ((room / entry_h).floor() as usize)
            .clamp(1, 9)
            .min(v.list.len())
    };
    let list_h = if !has_list {
        0.0
    } else if let Some(e) = &empty {
        e.height + 24.0
    } else {
        visible as f32 * entry_h + 8.0
    };
    let h = (fixed + list_h).min(wh - 16.0).max(140.0);
    let sheet = Rect::new(((ww - w) * 0.5).round(), ((wh - h) * 0.5).round(), w, h);
    dl.shadow(sheet, RADIUS_LG, pal.shadow, 40.0, 14.0);
    dl.rect_border(sheet, RADIUS_LG, pal.surface, pal.glass_border, 1.0);
    let x0 = sheet.x + pad;
    let mut y = sheet.y + pad;
    let focus = v.focused();
    let mut drawn = Drawn {
        sheet,
        visible,
        ..Drawn::default()
    };
    text.draw(dl, &title, mi.x(x0, title.width), y, pal.text);
    y += title.height + 10.0;
    if let Some(s) = &subtitle {
        text.draw(dl, s, mi.x(x0, s.width), y, pal.text_muted);
        y += s.height + 10.0;
    }

    // The form.
    for (i, item) in v.form.iter().enumerate() {
        let (r, c) = (i / cols, i % cols);
        if r < first_row || r >= first_row + rows {
            continue;
        }
        let r = r - first_row;
        let cx = x0 + c as f32 * (cell_w + gap);
        let cy = y + r as f32 * (cell_h + 10.0);
        let lab = text.layout(&item.label, small_st, Some(cell_w), Align::Start);
        text.draw(dl, &lab, mi.x(cx, lab.width), cy, pal.text_muted);
        let fr = Rect::new(cx, cy + label_h + 4.0, cell_w, field_h);
        let focused = focus == Some(Hit::Form(i));
        match &item.control {
            Control::Field {
                text: s,
                caret,
                selection,
                masked: hide,
                hint,
                preedit,
                ..
            } => {
                let shown;
                let (s, caret, selection) = if *hide {
                    shown = masked(s);
                    (
                        shown.as_str(),
                        masked_byte(s, *caret),
                        (masked_byte(s, selection.0), masked_byte(s, selection.1)),
                    )
                } else {
                    (s.as_str(), *caret, *selection)
                };
                // Never into a hidden field (passwords, tokens).
                let pre = if focused && !*hide {
                    preedit.as_str()
                } else {
                    ""
                };
                let (block, bx, ime) = draw_field(
                    dl, text, pal, label_st, mi, fr, s, hint, caret, selection, focused, pre,
                );
                if focused {
                    // Not while composing: a click would land in the composition.
                    drawn.field_text = pre.is_empty().then_some((block, bx));
                    drawn.field = Some(mi.r(fr));
                    drawn.ime = ime;
                }
            }
            Control::Choice { value } => {
                let rm = mi.r(fr);
                dl.rect_border(rm, RADIUS_SM, pal.surface_raised, pal.separator, 1.0);
                let b = text.layout(value, label_st, None, Align::Start);
                let arrow = (field_h - 10.0).min(20.0 * ts.max(1.0));
                dl.push_clip(rm);
                text.draw(
                    dl,
                    &b,
                    mi.x(fr.x + 10.0, b.width),
                    fr.y + (field_h - b.height) * 0.5,
                    pal.text,
                );
                dl.pop_clip();
                let ar = mi.r(Rect::new(
                    fr.right() - arrow - 6.0,
                    fr.y + (field_h - arrow) * 0.5,
                    arrow,
                    arrow,
                ));
                draw_icon(dl, Icon::ChevronDown, ar, pal.text_muted, 1.6, false);
            }
            Control::Switch { on } => {
                let tw = (field_h * 1.6).round();
                let th = (field_h * 0.8).round();
                let track = Rect::new(fr.x, fr.y + (field_h - th) * 0.5, tw, th);
                let tm = mi.r(track);
                dl.rect(
                    tm,
                    th * 0.5,
                    if *on {
                        pal.accent_fill
                    } else {
                        pal.surface_raised
                    },
                );
                if !*on {
                    dl.rect_border(tm, th * 0.5, Color::TRANSPARENT, pal.separator, 1.0);
                }
                let k = th - 6.0;
                let kx = if *on {
                    track.right() - 3.0 - k
                } else {
                    track.x + 3.0
                };
                let knob = mi.r(Rect::new(kx, track.y + 3.0, k, k));
                dl.rect(
                    knob,
                    k * 0.5,
                    if *on { pal.accent_text } else { pal.text_muted },
                );
                let state = if *on {
                    t!("dicomweb-switch-on")
                } else {
                    t!("dicomweb-switch-off")
                };
                let b = text.layout(&state, label_st, None, Align::Start);
                text.draw(
                    dl,
                    &b,
                    mi.x(track.right() + 10.0, b.width),
                    fr.y + (field_h - b.height) * 0.5,
                    pal.text,
                );
            }
        }
        let hit_r = mi.r(Rect::new(cx, cy, cell_w, cell_h));
        if focused && !matches!(item.control, Control::Field { .. }) {
            dl.rect_border(
                mi.r(fr).inset(-3.0),
                RADIUS_SM + 3.0,
                Color::TRANSPARENT,
                pal.focus_ring,
                2.0,
            );
        }
        drawn.hits.push((hit_r, Hit::Form(i)));
    }
    y += form_h;

    // The list.
    if let Some(l) = &list_title {
        text.draw(dl, l, mi.x(x0, l.width), y, pal.text);
        y += l.height + 8.0;
    }
    if has_list {
        let box_r = Rect::new(x0, y, inner, list_h - 8.0 + 4.0);
        let bm = mi.r(box_r);
        dl.rect_border(bm, RADIUS_SM, pal.surface_raised, pal.separator, 1.0);
        if let Some(e) = &empty {
            text.draw(dl, e, mi.x(x0 + 8.0, e.width), y + 10.0, pal.text_muted);
        } else {
            let first = v.scroll.min(v.list.len().saturating_sub(visible));
            dl.push_clip(bm);
            for (k, entry) in v.list.iter().enumerate().skip(first).take(visible) {
                let ey = y + 2.0 + (k - first) as f32 * entry_h;
                let r = Rect::new(x0 + 2.0, ey, inner - 4.0, entry_h - 2.0);
                let rm = mi.r(r);
                if entry.selected {
                    dl.rect(rm, RADIUS_SM, pal.accent_soft);
                }
                if focus == Some(Hit::Entry(k)) {
                    dl.rect_border(
                        rm.inset(1.0),
                        RADIUS_SM,
                        Color::TRANSPARENT,
                        pal.focus_ring,
                        2.0,
                    );
                }
                let indent = 8.0 + f32::from(entry.depth) * 22.0 * ts.max(1.0);
                let icon_w = 14.0 * ts.max(1.0);
                if v.page == Page::Search
                    && entry.depth == 0
                    && !entry.action
                    && entry.thumb == Thumb::None
                {
                    let icon = if entry.expanded {
                        Icon::ChevronDown
                    } else {
                        Icon::ChevronRight
                    };
                    let ir = mi.r(Rect::new(
                        r.x + indent - 2.0,
                        ey + (line_h - icon_w) * 0.5 + 6.0,
                        icon_w,
                        icon_w,
                    ));
                    draw_icon(dl, icon, ir, pal.text_muted, 1.5, rtl);
                }
                let mut tx = r.x + indent + icon_w + 6.0;
                if entry.thumb != Thumb::None {
                    let side = entry_h - 8.0;
                    let cell = Rect::new(tx, ey + 3.0, side, side);
                    match entry.thumb {
                        Thumb::Shown { tex, w, h } if w > 0 && h > 0 => {
                            let k = side / w.max(h) as f32;
                            let (iw, ih) = (w as f32 * k, h as f32 * k);
                            let ir = cell.centered(iw, ih);
                            dl.image(mi.r(ir), [0.0, 0.0, 1.0, 1.0], tex, 2.0);
                        }
                        _ => dl.rect(mi.r(cell), 2.0, pal.surface_hover),
                    }
                    tx += side + 8.0;
                }
                let tw = r.right() - tx - 8.0;
                let (st, ink) = if entry.action {
                    (strong_st, pal.accent)
                } else {
                    (label_st, pal.text)
                };
                let tb = text.layout(&entry.title, st, None, Align::Start);
                let ty = if entry.detail.is_empty() {
                    ey + (entry_h - tb.height) * 0.5
                } else {
                    ey + 5.0
                };
                dl.push_clip(mi.r(Rect::new(tx, ey, tw, entry_h)));
                text.draw(dl, &tb, mi.x(tx, tb.width.min(tw)), ty, ink);
                if !entry.detail.is_empty() {
                    let db = text.layout(&entry.detail, small_st, None, Align::Start);
                    text.draw(
                        dl,
                        &db,
                        mi.x(tx, db.width.min(tw)),
                        ty + tb.height + 2.0,
                        pal.text_muted,
                    );
                }
                dl.pop_clip();
                drawn.hits.push((rm, Hit::Entry(k)));
            }
            dl.pop_clip();
            // A scroll mark when the list does not fit.
            if v.list.len() > visible && visible > 0 {
                let track_h = box_r.h - 8.0;
                let th = (track_h * visible as f32 / v.list.len() as f32).max(16.0);
                let ty =
                    box_r.y + 4.0 + (track_h - th) * first as f32 / (v.list.len() - visible) as f32;
                let tr = mi.r(Rect::new(box_r.right() - 6.0, ty, 3.0, th));
                dl.rect(tr, 1.5, pal.text_faint);
            }
        }
        y += list_h;
    }

    for (n, tone) in &notes {
        text.draw(dl, n, mi.x(x0, n.width), y, tone_color(pal, *tone));
        y += n.height + 6.0;
    }
    if let Some((s, tone)) = &status {
        y += 4.0;
        let ink = if *tone == Tone::Plain {
            pal.text
        } else {
            tone_color(pal, *tone)
        };
        text.draw(dl, s, mi.x(x0, s.width), y, ink);
        y += s.height;
        if let Some(p) = v.progress {
            let bar = Rect::new(x0, y + 6.0, inner, 4.0);
            dl.rect(mi.r(bar), 2.0, pal.surface_raised);
            let fill = Rect::new(bar.x, bar.y, bar.w * p.clamp(0.0, 1.0), bar.h);
            dl.rect(mi.r(fill), 2.0, pal.accent);
        }
    }

    // Buttons at the end edge, the default last.
    let button_st = TextStyle::new(FONT_LABEL * ts).weight(550.0);
    let by = sheet.bottom() - pad - bh;
    let mut x = sheet.right() - pad;
    for b in v.buttons.iter().rev() {
        let lb = text.layout(&b.label, button_st, None, Align::Start);
        let bw = (lb.width + 28.0).max(76.0);
        x -= bw;
        let r = Rect::new(x, by, bw, bh);
        x -= 8.0;
        let rm = mi.r(r);
        let (fill, ink) = if b.default {
            (pal.accent_fill, pal.accent_text)
        } else {
            (pal.surface_raised, pal.text)
        };
        let fill = if b.enabled {
            fill
        } else {
            fill.with_alpha(0.45)
        };
        let ink = if b.enabled { ink } else { ink.with_alpha(0.5) };
        if b.default {
            dl.rect(rm, RADIUS_SM + 2.0, fill);
        } else {
            dl.rect_border(rm, RADIUS_SM + 2.0, fill, pal.separator, 1.0);
        }
        if focus == Some(Hit::Button(b.id)) {
            dl.rect_border(
                rm.inset(-4.0),
                RADIUS_SM + 6.0,
                Color::TRANSPARENT,
                pal.focus_ring,
                2.0,
            );
        }
        text.draw(
            dl,
            &lb,
            rm.x + (rm.w - lb.width) * 0.5,
            rm.y + (bh - lb.height) * 0.5,
            ink,
        );
        if b.enabled {
            drawn.hits.push((rm, Hit::Button(b.id)));
        }
    }
    drawn
}

/// A one-line text field; returns its text block and the x it is drawn at.
#[allow(clippy::too_many_arguments)]
fn draw_field(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    st: TextStyle,
    mi: Mirror,
    field: Rect,
    s: &str,
    hint: &str,
    caret: usize,
    selection: (usize, usize),
    focused: bool,
    preedit: &str,
) -> (Arc<TextBlock>, f32, Option<Rect>) {
    let fr = mi.r(field);
    dl.rect_border(
        fr,
        RADIUS_SM,
        pal.surface_raised,
        if focused {
            pal.focus_ring
        } else {
            pal.separator
        },
        if focused { 2.0 } else { 1.0 },
    );
    let inner = fr.inset_xy(10.0, 0.0);
    // A composition sits at the caret, underlined; the caret follows it.
    let (composed, composing) = specular_widgets::line_edit::compose(s, caret, preedit);
    let block = text.layout(&composed, st, None, Align::Start);
    let cx = block.caret_x(composing.end);
    // Text that reads right to left starts at the right edge.
    let start_right = mi.rtl || block.rtl;
    let x0 = specular_widgets::line_edit::field_x0(&block, cx, inner, start_right);
    let ty = fr.y + (fr.h - block.height) * 0.5;
    dl.push_clip(inner.inset_xy(-2.0, 0.0));
    if composed.is_empty() && !hint.is_empty() {
        let hb = text.layout(hint, st, None, Align::Start);
        let hx = if mi.rtl {
            inner.right() - hb.width
        } else {
            inner.x
        };
        text.draw(dl, &hb, hx, ty, pal.text_faint);
    }
    if focused && preedit.is_empty() {
        for (a, b) in block.range_spans(selection.0..selection.1) {
            dl.rect(
                Rect::new(x0 + a, ty, b - a, block.height),
                0.0,
                pal.selection,
            );
        }
    }
    text.draw(dl, &block, x0, ty, pal.text);
    if !preedit.is_empty() {
        specular_widgets::line_edit::draw_preedit_underline(
            dl,
            &block,
            x0,
            composing.clone(),
            ty + block.height,
            pal.text_muted,
        );
    }
    let mut ime = None;
    if focused {
        let empty = composed.is_empty();
        let cx = if empty && mi.rtl {
            inner.right()
        } else {
            x0 + cx
        };
        dl.rect(
            Rect::new((cx - 0.75).round(), ty, 1.5, block.height),
            0.0,
            pal.text,
        );
        ime = Some(if empty {
            Rect::new((cx - 0.75).round(), ty, 1.5, block.height)
        } else {
            specular_widgets::line_edit::ime_area(&block, x0, composing, ty, block.height, inner)
        });
    }
    dl.pop_clip();
    (block, x0, ime)
}

/// The sheet's AccessKit nodes: a modal dialog with the form's controls, the list (a tree
/// of studies and series, or a list of servers), the notes, the status and the buttons.
pub(in crate::app) fn a11y(
    tree: &mut accesskit::TreeUpdate,
    v: &DialogView,
    d: &Drawn,
    scale: f64,
) {
    use accesskit::{Node, NodeId, Role};
    let bounds = |r: Rect| accesskit::Rect {
        x0: f64::from(r.x) * scale,
        y0: f64::from(r.y) * scale,
        x1: f64::from(r.x + r.w) * scale,
        y1: f64::from(r.y + r.h) * scale,
    };
    let hit_rect = |h: Hit| d.hits.iter().find(|(_, x)| *x == h).map(|x| x.0);
    let mut dialog = Node::new(Role::Dialog);
    dialog.set_modal();
    dialog.set_label(v.title.clone());
    if let Some(s) = &v.subtitle {
        dialog.set_description(s.clone());
    }
    dialog.set_bounds(bounds(d.sheet));
    let focus_hit = v.focused();
    let mut focus_id = None;
    let mut ids = Vec::new();
    let mut nodes = Vec::new();
    for (i, item) in v.form.iter().enumerate() {
        let id = NodeId(A11Y_BASE + 0x100 + i as u64);
        let mut n = match &item.control {
            Control::Field {
                text: s,
                caret,
                selection,
                masked: hide,
                ..
            } => {
                let mut n = Node::new(if *hide {
                    Role::PasswordInput
                } else {
                    Role::TextInput
                });
                let run_id = NodeId(A11Y_BASE + 0x800 + i as u64);
                let value = if *hide { masked(s) } else { s.clone() };
                let chars = |b: usize| s[..b.min(s.len())].chars().count();
                let anchor = if *caret == selection.0 {
                    selection.1
                } else {
                    selection.0
                };
                let run = specular_widgets::text_run_sel(
                    &mut n,
                    run_id,
                    &value,
                    Some((chars(anchor), chars(*caret))),
                );
                n.set_children(vec![run_id]);
                nodes.push((run_id, run));
                n.add_action(accesskit::Action::SetValue);
                n.add_action(accesskit::Action::SetTextSelection);
                n
            }
            // Steps through its values (no list opens): a spin button, as in Convert.
            Control::Choice { value } => {
                let mut n = Node::new(Role::SpinButton);
                n.set_value(value.clone());
                n.add_action(accesskit::Action::Click);
                n.add_action(accesskit::Action::Increment);
                n.add_action(accesskit::Action::Decrement);
                n
            }
            Control::Switch { on } => {
                let mut n = Node::new(Role::Switch);
                n.set_toggled(if *on {
                    accesskit::Toggled::True
                } else {
                    accesskit::Toggled::False
                });
                n.add_action(accesskit::Action::Click);
                n
            }
        };
        n.set_label(item.label.clone());
        if let Some(r) = hit_rect(Hit::Form(i)) {
            n.set_bounds(bounds(r));
        }
        n.add_action(accesskit::Action::Focus);
        if focus_hit == Some(Hit::Form(i)) {
            focus_id = Some(id);
        }
        ids.push(id);
        nodes.push((id, n));
    }
    if !v.list.is_empty() {
        let list_id = NodeId(A11Y_BASE + 0x1000);
        let tree_like = v.page == Page::Search;
        let mut list = Node::new(if tree_like { Role::Tree } else { Role::ListBox });
        if let Some(t) = &v.list_title {
            list.set_label(t.clone());
        }
        let mut kids = Vec::new();
        for (k, e) in v.list.iter().enumerate() {
            let id = NodeId(A11Y_BASE + 0x1001 + k as u64);
            let mut n = Node::new(if e.action {
                Role::Button
            } else if tree_like {
                Role::TreeItem
            } else {
                Role::ListBoxOption
            });
            n.set_label(e.title.clone());
            if !e.detail.is_empty() {
                n.set_description(e.detail.clone());
            }
            if tree_like && !e.action {
                n.set_level(usize::from(e.depth) + 1);
                if e.depth == 0 {
                    n.set_expanded(e.expanded);
                }
            }
            if !e.action {
                n.set_selected(e.selected);
            }
            match hit_rect(Hit::Entry(k)) {
                Some(r) => n.set_bounds(bounds(r)),
                None => n.set_hidden(),
            }
            n.add_action(accesskit::Action::Click);
            n.add_action(accesskit::Action::Focus);
            n.add_action(accesskit::Action::ScrollIntoView);
            if focus_hit == Some(Hit::Entry(k)) {
                focus_id = Some(id);
            }
            kids.push(id);
            nodes.push((id, n));
        }
        list.set_children(kids);
        ids.push(list_id);
        nodes.push((list_id, list));
    } else if let Some(e) = &v.empty {
        let id = NodeId(A11Y_BASE + 0x1000);
        let mut n = Node::new(Role::Label);
        n.set_value(e.clone());
        ids.push(id);
        nodes.push((id, n));
    }
    for (k, (s, _)) in v.notes.iter().enumerate() {
        let id = NodeId(A11Y_BASE + 0x40 + k as u64);
        let mut n = Node::new(Role::Label);
        n.set_value(s.clone());
        ids.push(id);
        nodes.push((id, n));
    }
    if let Some((s, _)) = &v.status {
        let id = NodeId(A11Y_BASE + 0x7f);
        let mut n = Node::new(Role::Status);
        n.set_live(accesskit::Live::Polite);
        n.set_label(s.clone());
        if let Some(p) = v.progress {
            n.set_numeric_value(f64::from(p) * 100.0);
            n.set_min_numeric_value(0.0);
            n.set_max_numeric_value(100.0);
        }
        ids.push(id);
        nodes.push((id, n));
    }
    for (k, b) in v.buttons.iter().enumerate() {
        let id = NodeId(A11Y_BASE + 0x80 + k as u64);
        let mut n = Node::new(Role::Button);
        n.set_label(b.label.clone());
        if let Some(r) = hit_rect(Hit::Button(b.id)) {
            n.set_bounds(bounds(r));
        }
        if b.enabled {
            n.add_action(accesskit::Action::Click);
            n.add_action(accesskit::Action::Focus);
        } else {
            n.set_disabled();
        }
        if focus_hit == Some(Hit::Button(b.id)) {
            focus_id = Some(id);
        }
        ids.push(id);
        nodes.push((id, n));
    }
    dialog.set_children(ids);
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        root.push_child(NodeId(A11Y_BASE));
    }
    tree.nodes.push((NodeId(A11Y_BASE), dialog));
    tree.nodes.extend(nodes);
    tree.focus = focus_id.unwrap_or(NodeId(A11Y_BASE));
}

/// The control node `id` stands for.
pub(in crate::app) fn a11y_target(v: &DialogView, id: u64) -> Option<Hit> {
    let k = id.checked_sub(A11Y_BASE)?;
    match k {
        0x80..0x100 => v.buttons.get(k as usize - 0x80).map(|b| Hit::Button(b.id)),
        0x100..0x800 => {
            ((k as usize - 0x100) < v.form.len()).then(|| Hit::Form(k as usize - 0x100))
        }
        0x800..=0xfff => {
            ((k as usize - 0x800) < v.form.len()).then(|| Hit::Form(k as usize - 0x800))
        }
        0x1001.. => {
            let i = (k - 0x1001) as usize;
            (i < v.list.len()).then_some(Hit::Entry(i))
        }
        _ => None,
    }
}

/// The form cell with the keyboard focus.
fn focus_form(v: &DialogView) -> Option<usize> {
    match v.focused()? {
        Hit::Form(i) => Some(i),
        _ => None,
    }
}
