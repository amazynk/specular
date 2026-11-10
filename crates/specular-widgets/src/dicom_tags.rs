//! The DICOM tag browser (`t`, or from the palette): the file's data set as a tree, one row
//! per element (tag, keyword, VR, length, value), sequences and their items expandable, with
//! a filter field. The application reads and localises the rows ([`DicomTags`]).
//!
//! Patient-identifying attributes (the application marks them) are listed with their values
//! hidden until "Show patient data" is switched on, for this opening of the panel only; a
//! hidden value is never matched by the filter. The rows are dropped when the panel closes,
//! and [`TagLine`]'s `Debug` never prints a value.

use std::collections::HashSet;

use specular_gfx::{Icon, Rect, draw_icon};
use specular_i18n::{fold_for_search, t};
use specular_theme::metrics;

use crate::ctx::{Ctx, HAlign, Target};
use specular_input::{KeyCombo, NamedKey};

use crate::{Chrome, Modal, Response, ViewerState};

/// The tag browser's content, localised by the application.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DicomTags {
    /// The file's name.
    pub title: String,
    /// The data set in reading order (the file meta information first).
    pub rows: Vec<TagLine>,
    /// Instead of the rows: "Reading tags…", or why they cannot be read.
    pub status: Option<String>,
    /// Under the list: what was left out of a very large data set.
    pub note: Option<String>,
    /// Column heads in place of Tag, Keyword, VR, Length and Value (a structured report's
    /// content tree: Item, Concept, Type, Relationship, Value), and the browser's title.
    pub heads: Option<([String; 5], String)>,
}

/// One row of the tree.
#[derive(Clone, Default, PartialEq)]
pub struct TagLine {
    /// Nesting: 0 for the data set, +1 for a sequence's items, +2 for their elements.
    pub depth: u16,
    /// "(0010,0010)"; empty for an item row.
    pub tag: String,
    /// "PatientName", or the item's name ("Item 2").
    pub keyword: String,
    pub vr: String,
    /// The value length as shown ("12", "undefined").
    pub length: String,
    /// The value as shown: shortened text, "3 items", "1,024 bytes: 00 1f …".
    pub value: String,
    /// Rows under this one (a sequence's items and their elements).
    pub descendants: u32,
    /// A patient-identifying attribute (or inside one): its value is hidden by default.
    pub identifying: bool,
    /// The row points at an image or a place the viewer can show (Enter, or its button,
    /// sends [`crate::Command::TagJump`] with the row's index).
    pub jump: bool,
}

impl std::fmt::Debug for TagLine {
    /// Values are left out: they may identify a patient.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TagLine")
            .field("depth", &self.depth)
            .field("tag", &self.tag)
            .field("keyword", &self.keyword)
            .field("vr", &self.vr)
            .field("descendants", &self.descendants)
            .field("identifying", &self.identifying)
            .field("jump", &self.jump)
            .finish_non_exhaustive()
    }
}

#[derive(Default)]
pub(crate) struct TagsState {
    pub sheet: DicomTags,
    pub query: String,
    /// The filter field's caret, selection and undo (the truth is `query`).
    pub query_edit: crate::line_edit::LineEdit,
    /// "Show patient data" is on.
    pub patient: bool,
    /// Rows (indices into `sheet.rows`) whose children are shown when not filtering.
    pub expanded: HashSet<usize>,
    /// The rows listed, in order.
    pub visible: Vec<usize>,
    /// Index into `visible`.
    pub selected: usize,
    pub scroll: f32,
    pub last_scroll: f32,
    pub dirty: bool,
    /// Folded search text of each row without its value, and of its value.
    hay: Vec<(String, String)>,
}

fn fold(s: &str) -> String {
    fold_for_search(s).to_lowercase()
}

impl TagsState {
    fn reset(&mut self) {
        *self = TagsState {
            dirty: true,
            ..TagsState::default()
        };
    }

    /// A report's content tree opens fully expanded: it is read as a document. A data set's
    /// sequences stay closed.
    fn open_report(&mut self) {
        if self.sheet.heads.is_some() {
            self.expanded = (self.sheet.rows.iter().enumerate())
                .filter(|(_, r)| r.descendants > 0)
                .map(|(i, _)| i)
                .collect();
        }
    }

    pub fn type_text(&mut self, s: &str) {
        self.edit().insert(s);
        self.edited();
    }

    pub fn backspace(&mut self) {
        let mut clip = crate::line_edit::MemoryClipboard::default();
        let k = specular_input::Key::Named(specular_input::NamedKey::Backspace);
        self.edit()
            .key(k, specular_input::Mods::NONE, false, &mut clip);
        self.edited();
    }

    /// The filter field, caught up with `query`.
    pub fn edit(&mut self) -> &mut crate::line_edit::LineEdit {
        self.query_edit.sync(&self.query);
        &mut self.query_edit
    }

    /// The field edited: the tree follows it, from its top.
    pub fn edited(&mut self) {
        if self.query != self.query_edit.text() {
            self.query = self.query_edit.text().to_owned();
            self.selected = 0;
            self.scroll = 0.0;
            self.dirty = true;
        }
    }

    pub fn move_sel(&mut self, d: i32) {
        if self.visible.is_empty() {
            return;
        }
        let n = self.visible.len() as i64;
        self.selected = (self.selected as i64 + d as i64).clamp(0, n - 1) as usize;
        self.scroll = f32::NAN;
    }

    pub(crate) fn filtering(&self) -> bool {
        !self.query.trim().is_empty()
    }

    /// Whether `row`'s value is shown.
    pub fn value_shown(&self, row: &TagLine) -> bool {
        !row.identifying || self.patient
    }

    /// Shows or hides the children of the selected row (`Some(true)`: expand, `Some(false)`:
    /// collapse, `None`: toggle). Collapsing a row without open children selects its parent.
    pub fn expand(&mut self, how: Option<bool>) {
        if self.filtering() {
            return;
        }
        let Some(&i) = self.visible.get(self.selected) else {
            return;
        };
        let has = self.sheet.rows[i].descendants > 0;
        let open = self.expanded.contains(&i);
        match (how, has, open) {
            (Some(true) | None, true, false) => {
                self.expanded.insert(i);
            }
            (Some(false) | None, true, true) => {
                self.expanded.remove(&i);
            }
            (Some(false), _, _) => {
                // To the parent: the nearest row above that is shallower.
                let d = self.sheet.rows[i].depth;
                if let Some(p) = (0..self.selected)
                    .rev()
                    .find(|&k| self.sheet.rows[self.visible[k]].depth < d)
                {
                    self.selected = p;
                    self.scroll = f32::NAN;
                }
                return;
            }
            _ => return,
        }
        self.rebuild();
    }

    /// The rows listed: every row whose ancestors are expanded or, while filtering, the rows
    /// that match and their ancestors.
    pub fn rebuild(&mut self) {
        self.dirty = false;
        let rows = &self.sheet.rows;
        let sel_row = self.visible.get(self.selected).copied();
        let mut vis = Vec::new();
        if self.filtering() {
            if self.hay.len() != rows.len() {
                self.hay = rows
                    .iter()
                    .map(|r| {
                        let compact: String =
                            r.tag.chars().filter(|c| c.is_ascii_hexdigit()).collect();
                        (
                            fold(&format!("{} {compact} {} {}", r.tag, r.keyword, r.vr)),
                            fold(&r.value),
                        )
                    })
                    .collect();
            }
            let q = fold(self.query.trim());
            let matched: Vec<bool> = rows
                .iter()
                .zip(&self.hay)
                .map(|(r, (h, v))| {
                    h.contains(&q) || (!r.identifying || self.patient) && v.contains(&q)
                })
                .collect();
            // Matches below each row, from prefix sums.
            let mut before = Vec::with_capacity(rows.len() + 1);
            before.push(0u32);
            for m in &matched {
                before.push(before.last().unwrap() + *m as u32);
            }
            for (i, r) in rows.iter().enumerate() {
                let end = (i + 1 + r.descendants as usize).min(rows.len());
                if matched[i] || before[end] > before[i + 1] {
                    vis.push(i);
                }
            }
        } else {
            let mut i = 0;
            while i < rows.len() {
                vis.push(i);
                let r = &rows[i];
                i += 1;
                if r.descendants > 0 && !self.expanded.contains(&(i - 1)) {
                    i += r.descendants as usize;
                }
            }
        }
        self.visible = vis;
        // The selection stays on its row when it is still listed.
        self.selected = sel_row
            .and_then(|s| self.visible.iter().position(|&v| v == s))
            .unwrap_or(0)
            .min(self.visible.len().saturating_sub(1));
    }

    /// Patient-identifying values hidden now.
    pub fn hidden(&self) -> usize {
        if self.patient {
            return 0;
        }
        self.sheet
            .rows
            .iter()
            .filter(|r| r.identifying && !r.tag.is_empty())
            .count()
    }
}

impl Chrome {
    /// Opens the tag browser with this content (the filter empty, patient data hidden).
    pub fn show_dicom_tags(&mut self, tags: DicomTags) {
        self.dicom_tags.reset();
        self.dicom_tags.sheet = tags;
        self.dicom_tags.open_report();
        self.open_modal(Modal::DicomTags);
    }

    /// Replaces the open tag browser's content (the rows once read); the filter and the
    /// patient data switch stay. Ignored when it is closed.
    pub fn set_dicom_tags(&mut self, tags: DicomTags) {
        if !self.dicom_tags_open() {
            return;
        }
        let s = &mut self.dicom_tags;
        let fresh = s.sheet.rows.len() != tags.rows.len();
        if fresh {
            s.expanded.clear();
            s.selected = 0;
            s.scroll = 0.0;
        }
        s.hay.clear();
        s.sheet = tags;
        if fresh {
            s.open_report();
        }
        s.dirty = true;
    }

    /// Whether the tag browser is open.
    pub fn dicom_tags_open(&self) -> bool {
        self.modal == Some(Modal::DicomTags)
    }

    /// A click on a row: selects it and opens or closes its children.
    pub(crate) fn tag_row_click(&mut self, i: usize) -> Response {
        let s = &mut self.dicom_tags;
        if i < s.visible.len() {
            s.selected = i;
            s.expand(None);
        }
        Response::used()
    }

    /// The jump button of a row (by position in the rows listed).
    pub(crate) fn tag_jump(&mut self, i: usize) -> Response {
        let s = &mut self.dicom_tags;
        match s.visible.get(i) {
            Some(&row) if s.sheet.rows[row].jump => {
                s.selected = i;
                Response::cmd(crate::Command::TagJump(row))
            }
            _ => Response::used(),
        }
    }

    pub(crate) fn toggle_patient_data(&mut self) -> Response {
        let s = &mut self.dicom_tags;
        s.patient = !s.patient;
        s.dirty = true;
        Response::used()
    }

    /// The keyboard in the tag browser: Tab goes round the field, the switch and Close;
    /// Up, Down, Page Up/Down, Home and End move in the tree, Enter opens and closes a
    /// sequence. The filter field (`Chrome::field_key`) has the editing keys: Backspace,
    /// Delete, Shift+Home/End, the clipboard and undo. Right and Left are the field's while
    /// it holds a filter (the caret moves; with Shift it selects) and the tree's while it is
    /// empty (they open and close a sequence, mirrored in a right-to-left layout): a filtered
    /// list has no sequences to open, and an empty field no caret to move.
    pub(crate) fn tags_key(
        &mut self,
        combo: &KeyCombo,
        named: Option<NamedKey>,
        repeat: bool,
        st: &ViewerState,
    ) -> Response {
        if let Some(back) = crate::focus::is_tab(combo) {
            self.tags_tab(back);
            return Response::used();
        }
        if let Some(t) = self.modal_focus.clone() {
            if matches!(named, Some(NamedKey::Enter | NamedKey::Space)) {
                return if repeat {
                    Response::used()
                } else {
                    self.activate(t, st)
                };
            }
            // Any other key goes back to the field and does what it does there.
            self.modal_focus = None;
            if let Some(r) = self.field_key(combo) {
                return r;
            }
        }
        let (fwd, bwd) = if self.last_rtl {
            (NamedKey::ArrowLeft, NamedKey::ArrowRight)
        } else {
            (NamedKey::ArrowRight, NamedKey::ArrowLeft)
        };
        let s = &mut self.dicom_tags;
        match named {
            Some(NamedKey::ArrowDown) => s.move_sel(1),
            Some(NamedKey::ArrowUp) => s.move_sel(-1),
            Some(NamedKey::PageDown) => s.move_sel(10),
            Some(NamedKey::PageUp) => s.move_sel(-10),
            Some(NamedKey::Home) => s.move_sel(i32::MIN / 2),
            Some(NamedKey::End) => s.move_sel(i32::MAX / 2),
            Some(NamedKey::Enter) => {
                if let Some(&row) = s.visible.get(s.selected)
                    && s.sheet.rows[row].jump
                {
                    return if repeat {
                        Response::used()
                    } else {
                        Response::cmd(crate::Command::TagJump(row))
                    };
                }
                s.expand(None)
            }
            Some(NamedKey::Backspace) => s.backspace(),
            Some(n) if n == fwd => s.expand(Some(true)),
            Some(n) if n == bwd => s.expand(Some(false)),
            _ => {}
        }
        Response::used()
    }

    /// The tag browser's Tab order: the field (no target), the switch, Close.
    pub(crate) fn tags_tab(&mut self, back: bool) {
        let order = [None, Some(Target::TagPatient), Some(Target::ModalClose)];
        let at = order
            .iter()
            .position(|o| *o == self.modal_focus)
            .unwrap_or(0);
        let n = order.len();
        let to = if back { (at + n - 1) % n } else { (at + 1) % n };
        self.modal_focus = order[to].clone();
        self.reveal_focus = true;
    }
}

/// The screen-reader name of a row: what the columns show, the hidden value as hidden.
pub(crate) fn row_label(s: &TagsState, row: &TagLine) -> String {
    let value = if s.value_shown(row) {
        row.value.clone()
    } else {
        t!("dicom-tags-hidden")
    };
    let mut out = String::new();
    for part in [&row.keyword, &row.tag, &row.vr] {
        if !part.is_empty() {
            if !out.is_empty() {
                out.push_str(", ");
            }
            out.push_str(part);
        }
    }
    if !value.is_empty() {
        out.push_str(": ");
        out.push_str(&value);
    }
    out
}

pub(crate) fn draw(ch: &mut Chrome, cx: &mut Ctx) {
    let on_field = ch.modal_focus.is_none();
    if ch.dicom_tags.dirty {
        ch.dicom_tags.rebuild();
    }
    let hidden = ch.dicom_tags.hidden();
    let s = &mut ch.dicom_tags;
    let w = (cx.w - 48.0).min(cx.u(980.0));
    let h = (cx.h - 64.0).min(cx.h * 0.86).max(160.0);
    let r = Rect::new(((cx.w - w) * 0.5).round(), ((cx.h - h) * 0.5).round(), w, h);
    cx.sheet(r);
    let inner = Rect::new(r.x + 20.0, r.y, r.w - 40.0, r.h);

    // Title and Close.
    let title_s = cx.style(metrics::FONT_TITLE).weight(650.0);
    let title = if let Some((_, title)) = &s.sheet.heads {
        title.clone()
    } else if s.sheet.title.is_empty() {
        t!("dicom-tags-title")
    } else {
        t!("dicom-tags-title-file", file = s.sheet.title.clone())
    };
    let close = cx.m_in(Rect::new(r.right() - 46.0, r.y + 12.0, 32.0, 32.0), r);
    cx.close_button(close);
    let tr = cx.m_in(
        Rect::new(inner.x, r.y + 12.0, inner.w - 48.0, cx.u(32.0).max(32.0)),
        r,
    );
    cx.label_fit(&title, title_s, tr, HAlign::Start, cx.pal.text, true);

    // The filter field and the patient data switch.
    let field_h = cx.u(36.0);
    let y = tr.bottom() + 10.0;
    let sw_label_s = cx.style(metrics::FONT_LABEL);
    let sw_label = cx.layout(&t!("dicom-tags-show-patient"), sw_label_s, None);
    let sw_w = 42.0 + 10.0 + sw_label.width + 12.0;
    let field = cx.m_in(
        Rect::new(inner.x, y, (inner.w - sw_w - 16.0).max(120.0), field_h),
        r,
    );
    cx.field(
        field,
        &s.query,
        Some(&s.query_edit),
        &t!("dicom-tags-filter"),
        on_field,
        true,
    );
    let sw_row = cx.m_in(Rect::new(inner.right() - sw_w, y, sw_w, field_h), r);
    let tgt = Target::TagPatient;
    let sw = cx.m_in(
        Rect::new(sw_row.x + 6.0, sw_row.center().1 - 11.0, 42.0, 22.0),
        sw_row,
    );
    let on = s.patient;
    cx.dl.rect(
        sw,
        11.0,
        if on {
            cx.pal.accent_fill
        } else {
            cx.pal.surface_pressed
        },
    );
    if cx.is_hover(&tgt) {
        cx.dl.rect(sw, 11.0, cx.pal.surface_hover);
    }
    let knob_x = if on != cx.rtl {
        sw.right() - 11.0
    } else {
        sw.x + 11.0
    };
    cx.dl
        .circle((knob_x, sw.center().1), 8.0, specular_theme::Color::WHITE);
    let lr = cx.m_in(
        Rect::new(sw_row.x + 58.0, sw_row.y, sw_label.width, sw_row.h),
        sw_row,
    );
    cx.block_in(&sw_label, lr, HAlign::Start, cx.pal.text);
    cx.ring_if(&tgt, sw_row, metrics::RADIUS_SM + 1.0);
    cx.hit(sw_row, tgt);

    // Column heads.
    let small = cx.style(metrics::FONT_SMALL);
    let mono = small.tabular();
    let head_s = small.weight(600.0);
    let row_h = cx.u(26.0);
    let report = s.sheet.heads.is_some();
    // A report's item numbers ("1.5.2") are shorter than tags; its concepts are longer.
    let col_tag = if report {
        // Tabular digits: the longest number is the widest.
        let longest = (s.sheet.rows.iter())
            .map(|r| r.tag.as_str())
            .max_by_key(|t| t.len())
            .unwrap_or("");
        let deepest = cx
            .text
            .measure(longest, mono)
            .max(cx.text.measure("1.1.1", mono));
        deepest.min(cx.text.measure("(0000,0000)", mono)) + 14.0
    } else {
        cx.text.measure("(0000,0000)", mono) + 14.0
    };
    let heads = match &s.sheet.heads {
        Some((h, _)) => h.clone(),
        None => [
            t!("dicom-tags-col-tag"),
            t!("dicom-tags-col-keyword"),
            t!("dicom-tags-col-vr"),
            t!("dicom-tags-col-length"),
            t!("dicom-tags-col-value"),
        ],
    };
    // A report's relationships ("HAS OBS CONTEXT") in place of lengths.
    let len_sample = if s.sheet.heads.is_some() {
        "HAS CONCEPT MOD"
    } else {
        "4294967295"
    };
    let len_s = if s.sheet.heads.is_some() { small } else { mono };
    // A report's value types ("CONTAINER", "SCOORD3D") in place of VRs.
    let vr_sample = if s.sheet.heads.is_some() {
        "CONTAINER"
    } else {
        "VRW"
    };
    let col_vr = cx
        .text
        .measure(vr_sample, len_s)
        .max(cx.text.measure(&heads[2], head_s))
        + 12.0;
    let col_len = cx
        .text
        .measure(len_sample, len_s)
        .max(cx.text.measure(&heads[3], head_s))
        + 14.0;
    let share = if report { 0.45 } else { 0.36 };
    let col_kw = ((inner.w - col_tag - col_vr - col_len) * share).clamp(120.0, cx.u(300.0));
    let col_val = (inner.w - col_tag - col_kw - col_vr - col_len).max(40.0);
    let cols = [col_tag, col_kw, col_vr, col_len, col_val];
    let col_x = |k: usize| inner.x + cols[..k].iter().sum::<f32>();
    let head_y = field.bottom() + 12.0;
    for (k, head) in heads.iter().enumerate() {
        let cr = cx.m_in(Rect::new(col_x(k), head_y, cols[k] - 8.0, row_h), r);
        cx.label_fit(head, head_s, cr, HAlign::Start, cx.pal.text_muted, false);
    }
    let sep = cx.m_in(Rect::new(inner.x, head_y + row_h, inner.w, 1.0), r);
    cx.dl.rect(sep, 0.0, cx.pal.separator);

    // Footer: what is hidden or left out.
    let foot_h = cx.u(28.0);
    let mut foot = Vec::new();
    if hidden > 0 {
        foot.push(t!("dicom-tags-hidden-count", count = hidden as i64));
    }
    if let Some(n) = &s.sheet.note {
        foot.push(n.clone());
    }
    let list = Rect::new(
        inner.x - 6.0,
        head_y + row_h + 4.0,
        inner.w + 12.0,
        (r.bottom() - 12.0 - foot_h - (head_y + row_h + 4.0)).max(row_h),
    );
    if !foot.is_empty() {
        let fr = cx.m_in(
            Rect::new(inner.x, r.bottom() - 8.0 - foot_h, inner.w, foot_h),
            r,
        );
        cx.label_fit(
            &specular_i18n::dot_join(&foot),
            small,
            fr,
            HAlign::Start,
            cx.pal.text_faint,
            false,
        );
    }

    let empty = match &s.sheet.status {
        Some(st) => Some(st.clone()),
        None if s.visible.is_empty() => Some(t!("dicom-tags-no-match")),
        None => None,
    };
    if let Some(e) = empty {
        let body = cx.style(metrics::FONT_BODY);
        cx.label(&e, body, list, HAlign::Center, cx.pal.text_muted);
        return;
    }

    let content = s.visible.len() as f32 * row_h;
    let max = (content - list.h).max(0.0);
    if s.scroll.is_nan() {
        let top = s.selected as f32 * row_h;
        let cur = s.last_scroll.clamp(0.0, max);
        s.scroll = if top < cur {
            top
        } else if top + row_h > cur + list.h {
            top + row_h - list.h
        } else {
            cur
        };
    }
    s.scroll = s.scroll.clamp(0.0, max);
    s.last_scroll = s.scroll;
    cx.dl.push_clip(list);
    let first = (s.scroll / row_h).floor() as usize;
    let shown = (list.h / row_h).ceil() as usize + 1;
    let filtering = s.filtering();
    let indent = cx.u(14.0);
    for vi in first..s.visible.len().min(first + shown) {
        let ri = s.visible[vi];
        let row = &s.sheet.rows[ri];
        let rr = Rect::new(list.x, list.y + vi as f32 * row_h - s.scroll, list.w, row_h);
        let t = Target::TagRow(vi);
        let mut jump_hit = None;
        if vi == s.selected {
            cx.dl.rect(
                rr.inset_xy(0.0, 1.0),
                metrics::RADIUS_SM + 1.0,
                cx.pal.accent_soft,
            );
        } else if cx.is_hover(&t) {
            cx.dl.rect(
                rr.inset_xy(0.0, 1.0),
                metrics::RADIUS_SM + 1.0,
                cx.pal.surface_hover,
            );
        }
        let item = row.tag.is_empty();
        let colour = if item { cx.pal.text_muted } else { cx.pal.text };
        if !item {
            let cr = cx.m_in(Rect::new(col_x(0), rr.y, cols[0] - 6.0, row_h), rr);
            cx.label_fit(&row.tag, mono, cr, HAlign::Start, cx.pal.text_muted, false);
        }
        // The keyword, indented by depth, after the disclosure triangle of a parent row.
        let ind = indent * row.depth as f32;
        let tri = 14.0;
        if row.descendants > 0 {
            let open = filtering || s.expanded.contains(&ri);
            let ir = cx
                .m_in(Rect::new(col_x(1) + ind, rr.y, tri, row_h), rr)
                .centered(10.0, 10.0);
            let icon = match (open, cx.rtl) {
                (true, _) => Icon::ChevronDown,
                (false, false) => Icon::ChevronRight,
                (false, true) => Icon::ChevronLeft,
            };
            draw_icon(cx.dl, icon, ir, cx.pal.text_faint, 1.5, false);
        }
        let kw_x = col_x(1) + ind + tri + 2.0;
        let kr = cx.m_in(
            Rect::new(kw_x, rr.y, (cols[1] - ind - tri - 8.0).max(8.0), row_h),
            rr,
        );
        let kw_s = if item {
            specular_gfx::TextStyle {
                italic: true,
                ..small
            }
        } else {
            small
        };
        cx.label_fit(&row.keyword, kw_s, kr, HAlign::Start, colour, false);
        let vr = cx.m_in(Rect::new(col_x(2), rr.y, cols[2] - 6.0, row_h), rr);
        cx.label_fit(&row.vr, mono, vr, HAlign::Start, cx.pal.text_muted, false);
        let lr = cx.m_in(Rect::new(col_x(3), rr.y, cols[3] - 10.0, row_h), rr);
        let len_align = if s.sheet.heads.is_some() {
            HAlign::Start
        } else {
            HAlign::End
        };
        cx.label_fit(&row.length, len_s, lr, len_align, cx.pal.text_muted, false);
        // A jump row's button at the end of the value column.
        let jump_w = if row.jump { row_h } else { 0.0 };
        let vr = cx.m_in(
            Rect::new(col_x(4), rr.y, (cols[4] - 6.0 - jump_w).max(8.0), row_h),
            rr,
        );
        if row.jump {
            let jr = cx.m_in(
                Rect::new(col_x(4) + cols[4] - jump_w, rr.y, jump_w, row_h),
                rr,
            );
            let jt = Target::TagJump(vi);
            if cx.is_hover(&jt) {
                cx.dl
                    .rect(jr.inset(2.0), metrics::RADIUS_SM, cx.pal.surface_hover);
            }
            draw_icon(
                cx.dl,
                Icon::Image,
                jr.centered(14.0, 14.0),
                cx.pal.accent,
                1.5,
                false,
            );
            jump_hit = Some((jr.intersect(&list), jt));
        }
        if s.value_shown(row) {
            cx.label_fit(&row.value, small, vr, HAlign::Start, colour, false);
        } else if !item {
            cx.label_fit(
                &t!("dicom-tags-hidden"),
                specular_gfx::TextStyle {
                    italic: true,
                    ..small
                },
                vr,
                HAlign::Start,
                cx.pal.text_faint,
                false,
            );
        }
        cx.hit(rr.intersect(&list), t);
        // The button over its row.
        if let Some((r, jt)) = jump_hit {
            cx.hit(r, jt);
        }
    }
    cx.dl.pop_clip();
    crate::scrollbar::draw(
        ch,
        cx,
        crate::scrollbar::Area::DicomTags,
        list,
        Rect::new(r.x, list.y, r.w, list.h),
        max,
    );
}
