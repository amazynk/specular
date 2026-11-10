//! DICOM export in medical mode: an anonymised copy of the image, its series or its study
//! (PS3.15 Annex E Basic Profile and its options), or an IHE PDI patient disc folder
//! (DICOMDIR, `DICOM/`, `README.TXT`, `INDEX.HTM`), optionally anonymised. Both from the
//! palette ("Export anonymised copy…", "Export patient disc folder…").
//!
//! A modal sheet drawn like the print dialog: which images, the profile options, burned-in
//! text, the folder (the system folder chooser), Cancel and Export, with progress and Stop
//! while the files are written on a thread. The source files are never written; the output
//! goes to the chosen folder under new names (the old ones may carry the patient's name).
//!
//! Burned-in text: "Mask burned-in text" (in the palette, or the dialog's Burned-in text row)
//! shows the image with rectangles in image pixels that the export blacks out in every
//! frame, suggested by an edge text detector ([`specular_dicom::detect`]) and edited by
//! dragging. Masks apply to the images of the same size. A file whose Burned In Annotation is
//! YES is not exported without masks; the result says how many were skipped.
//!
//! The options last for the session (no settings).

#[cfg(test)]
mod shots;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crossbeam_channel::{Receiver, Sender};
use specular_dicom::anon::{self, AnonError, Options, Risk, Temporal};
use specular_dicom::mask::Rect as Area;
use specular_dicom::{Dicom, detect, pdi};
use specular_gfx::{Align, DrawList, Icon, Rect, Text, TextStyle, draw_icon};
use specular_i18n::t;
use specular_input::{Key, KeyCombo, Mods, NamedKey};
use specular_theme::metrics::{FONT_LABEL, FONT_SMALL, FONT_TITLE, RADIUS_LG, RADIUS_SM};
use specular_theme::{Color, Palette};
use specular_widgets::accesskit;

use super::App;

const A11Y_BASE: u64 = 0x5A00_0000_0000_0000;
/// The mask tool's bar and areas.
const MASK_A11Y_BASE: u64 = 0x5A10_0000_0000_0000;
/// Files read when looking for the series and study of the image.
const MAX_SCAN: usize = 20_000;
/// The smallest mask (image pixels) a drag makes.
const MIN_AREA: f32 = 2.0;

/// Which export.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    /// De-identified copies, one file each.
    Copy,
    /// An IHE PDI folder.
    Disc,
}

/// Which images.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Scope {
    Current,
    Series,
    Study,
}

/// One line of the dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Row {
    Images,
    Anonymise,
    Dates,
    Patient,
    Device,
    Institution,
    Descriptions,
    Structured,
    Uids,
    Overlays,
    BurnedIn,
    Folder,
}

impl Row {
    /// Rows that do something when activated rather than step through values.
    pub(super) fn is_action(self) -> bool {
        matches!(self, Row::BurnedIn | Row::Folder)
    }
}

/// What a click or a screen reader hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Hit {
    Dec(Row),
    Inc(Row),
    Row(Row),
    Cancel,
    Export,
}

/// The dialog's choices, kept for the session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Choices {
    /// Patient disc folders only (copies are always anonymised).
    pub anonymise: bool,
    pub options: Options,
}

impl Default for Choices {
    fn default() -> Self {
        Choices {
            anonymise: true,
            options: Options::default(),
        }
    }
}

impl Choices {
    /// The rows shown for an export of `kind`.
    pub(super) fn rows(&self, kind: Kind) -> Vec<Row> {
        let mut v = vec![Row::Images];
        if kind == Kind::Disc {
            v.push(Row::Anonymise);
        }
        if kind == Kind::Copy || self.anonymise {
            v.extend([
                Row::Dates,
                Row::Patient,
                Row::Device,
                Row::Institution,
                Row::Descriptions,
                Row::Structured,
                Row::Uids,
                Row::Overlays,
                Row::BurnedIn,
            ]);
        }
        v.push(Row::Folder);
        v
    }

    /// Steps `row` by `d` (±1); false when nothing changed.
    pub(super) fn step(&mut self, row: Row, d: i32) -> bool {
        let before = self.clone();
        let o = &mut self.options;
        match row {
            Row::Images | Row::BurnedIn | Row::Folder => {}
            Row::Anonymise => self.anonymise = !self.anonymise,
            Row::Dates => {
                let all = [Temporal::Remove, Temporal::Full, Temporal::Modified];
                let i = all.iter().position(|t| *t == o.temporal).unwrap_or(0);
                o.temporal = all[(i as i32 + d).rem_euclid(3) as usize];
            }
            Row::Patient => o.patient_characteristics = !o.patient_characteristics,
            Row::Device => o.device_identity = !o.device_identity,
            Row::Institution => o.institution_identity = !o.institution_identity,
            Row::Descriptions => o.clean_descriptors = !o.clean_descriptors,
            Row::Structured => o.clean_structured_content = !o.clean_structured_content,
            Row::Uids => o.retain_uids = !o.retain_uids,
            Row::Overlays => o.keep_overlays = !o.keep_overlays,
        }
        *self != before
    }
}

/// What the draw function shows for one row.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct RowView {
    pub row: Row,
    pub label: String,
    pub value: String,
}

/// Everything the dialog draws (so screenshots need no app).
#[derive(Clone, Debug)]
pub(super) struct DialogView {
    pub title: String,
    pub rows: Vec<RowView>,
    /// Under the rows: the burned-in text risk of the image.
    pub warning: Option<String>,
    /// Over the buttons: progress of the export.
    pub status: Option<String>,
    pub progress: Option<f32>,
    pub busy: bool,
    pub can_export: bool,
    /// Index into [`DialogView::focusables`].
    pub focus: Option<usize>,
}

impl DialogView {
    /// The keyboard focus order: the rows, then Cancel and Export.
    pub fn focusables(&self) -> Vec<Hit> {
        self.rows
            .iter()
            .map(|r| Hit::Row(r.row))
            .chain([Hit::Cancel, Hit::Export])
            .collect()
    }
}

/// Where things landed.
#[derive(Clone, Debug, Default)]
pub(super) struct Drawn {
    pub sheet: Rect,
    pub hits: Vec<(Rect, Hit)>,
    pub rows: Vec<(Rect, Row)>,
    pub warning: Option<Rect>,
}

/// Draws the dialog over a scrim in a window of `size` logical px.
pub(super) fn draw(
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
    let title_st = TextStyle::new(FONT_TITLE * ts).weight(650.0);
    let label_st = TextStyle::new(FONT_LABEL * ts);
    let value_st = TextStyle::new(FONT_LABEL * ts).weight(550.0);
    let small_st = TextStyle::new(FONT_SMALL * ts);
    let button_st = TextStyle::new(FONT_LABEL * ts).weight(550.0);

    let w = (ww - 32.0).clamp(280.0, 560.0);
    let inner = w - 2.0 * pad;
    let title = text.layout(&v.title, title_st, Some(inner), Align::Start);
    let warning = v
        .warning
        .as_ref()
        .map(|s| text.layout(s, small_st, Some(inner - 28.0), Align::Start));
    let bh = 32.0;
    let status_h = if v.status.is_some() { 30.0 } else { 0.0 };
    let warn_h = warning.as_ref().map_or(0.0, |b| b.height.max(18.0) + 14.0);
    let fixed = pad + title.height + 14.0 + warn_h + status_h + 14.0 + bh + pad;
    let max_h = (wh - 32.0).max(200.0);
    let row_h = ((max_h - fixed) / v.rows.len().max(1) as f32).clamp(22.0, 36.0);
    let h = (fixed + row_h * v.rows.len() as f32).min(max_h);
    let sheet = Rect::new(((ww - w) * 0.5).round(), ((wh - h) * 0.5).round(), w, h);
    dl.shadow(sheet, RADIUS_LG, pal.shadow, 40.0, 14.0);
    dl.rect_border(sheet, RADIUS_LG, pal.surface, pal.glass_border, 1.0);
    let m = |r: Rect| if rtl { r.mirror_x(ww) } else { r };
    // A text block at `x` (start edge, LTR coordinates).
    let start_x = |x: f32, bw: f32| if rtl { ww - x - bw } else { x };
    text.draw(
        dl,
        &title,
        start_x(sheet.x + pad, title.width),
        sheet.y + pad,
        pal.text,
    );
    let focus = v.focus.map(|i| v.focusables()[i]);
    let mut drawn = Drawn {
        sheet,
        ..Drawn::default()
    };
    let rows_top = sheet.y + pad + title.height + 14.0;
    let arrow = (row_h - 6.0).min(26.0);
    let ink = if v.busy { pal.text_faint } else { pal.text };
    for (i, rv) in v.rows.iter().enumerate() {
        let r = Rect::new(sheet.x + pad, rows_top + i as f32 * row_h, inner, row_h);
        if focus == Some(Hit::Row(rv.row)) {
            dl.rect_border(
                m(r.inset(-2.0)),
                RADIUS_SM,
                Color::TRANSPARENT,
                pal.focus_ring,
                2.0,
            );
        }
        let lab = text.layout(&rv.label, label_st, None, Align::Start);
        text.draw(
            dl,
            &lab,
            start_x(r.x + 4.0, lab.width),
            r.y + (row_h - lab.height) * 0.5,
            pal.text_muted,
        );
        let sw = (inner * 0.52).min(250.0);
        let sx = r.right() - sw;
        let val = text.layout(&rv.value, value_st, None, Align::Start);
        if rv.row.is_action() {
            // A button-like field: the value, with an icon at its end.
            let b = Rect::new(sx, r.y + (row_h - arrow) * 0.5, sw, arrow);
            let bm = m(b);
            dl.rect_border(bm, RADIUS_SM, pal.surface_raised, pal.separator, 1.0);
            let icon = if rv.row == Row::Folder {
                Icon::Folder
            } else {
                Icon::ChevronRight
            };
            let ib = m(Rect::new(b.right() - arrow, b.y, arrow, arrow));
            draw_icon(dl, icon, ib.inset(arrow * 0.22), ink, 1.6, rtl);
            dl.push_clip(m(Rect::new(b.x + 8.0, b.y, sw - 8.0 - arrow, arrow)));
            text.draw(
                dl,
                &val,
                start_x(b.x + 8.0, val.width),
                b.y + (arrow - val.height) * 0.5,
                ink,
            );
            dl.pop_clip();
        } else {
            // ‹ value ›
            let dec = Rect::new(sx, r.y + (row_h - arrow) * 0.5, arrow, arrow);
            let inc = Rect::new(r.right() - arrow, dec.y, arrow, arrow);
            for (b, icon, hit) in [
                (dec, Icon::ChevronLeft, Hit::Dec(rv.row)),
                (inc, Icon::ChevronRight, Hit::Inc(rv.row)),
            ] {
                let b = m(b);
                dl.rect(b, RADIUS_SM, pal.surface_raised);
                draw_icon(dl, icon, b.inset(arrow * 0.18), ink, 1.6, rtl);
                drawn.hits.push((b, hit));
            }
            let room = sw - 2.0 * arrow;
            let mid = sx + arrow + (room - val.width.min(room)) * 0.5;
            dl.push_clip(m(Rect::new(sx + arrow, r.y, room, row_h)));
            text.draw(
                dl,
                &val,
                start_x(mid, val.width),
                r.y + (row_h - val.height) * 0.5,
                ink,
            );
            dl.pop_clip();
        }
        drawn.hits.push((m(r), Hit::Row(rv.row)));
        drawn.rows.push((m(r), rv.row));
    }
    let mut y = rows_top + v.rows.len() as f32 * row_h + 10.0;
    if let Some(b) = &warning {
        let box_ = Rect::new(sheet.x + pad, y, inner, b.height.max(18.0) + 8.0);
        dl.rect(m(box_), RADIUS_SM, pal.warning.with_alpha(0.14));
        let ib = m(Rect::new(box_.x + 6.0, box_.y + 5.0, 16.0, 16.0));
        draw_icon(dl, Icon::Info, ib, pal.warning, 1.6, rtl);
        text.draw(
            dl,
            b,
            start_x(box_.x + 28.0, b.width),
            box_.y + 4.0,
            pal.text,
        );
        drawn.warning = Some(m(box_));
        y = box_.bottom() + 6.0;
    }
    let by = sheet.bottom() - pad - bh;
    if let Some(s) = &v.status {
        let b = text.layout(s, small_st, Some(inner), Align::Start);
        let sy = (by - 14.0 - status_h).max(y);
        text.draw(dl, &b, start_x(sheet.x + pad, b.width), sy, pal.text_muted);
        if let Some(p) = v.progress {
            let bar = Rect::new(sheet.x + pad, sy + b.height + 6.0, inner, 4.0);
            dl.rect(m(bar), 2.0, pal.separator);
            let done = Rect::new(bar.x, bar.y, bar.w * p.clamp(0.0, 1.0), bar.h);
            dl.rect(m(done), 2.0, pal.accent);
        }
    }

    // Buttons: Cancel (Stop while busy), Export (the default: filled).
    let labels = [
        (Hit::Export, t!("anon-export")),
        (
            Hit::Cancel,
            if v.busy {
                t!("convert-stop")
            } else {
                t!("app-cancel")
            },
        ),
    ];
    let mut x = sheet.right() - pad;
    for (hit, label) in labels {
        let b = text.layout(&label, button_st, None, Align::Start);
        let bw = (b.width + 28.0).max(84.0);
        x -= bw;
        let r = Rect::new(x, by, bw, bh);
        x -= 8.0;
        let enabled = hit == Hit::Cancel || (v.can_export && !v.busy);
        let rm = m(r);
        let (fill, ink) = if hit == Hit::Export {
            (pal.accent_fill, pal.accent_text)
        } else {
            (pal.surface_raised, pal.text)
        };
        let fill = if enabled { fill } else { fill.with_alpha(0.45) };
        let ink = if enabled { ink } else { ink.with_alpha(0.5) };
        if hit == Hit::Export {
            dl.rect(rm, RADIUS_SM + 2.0, fill);
        } else {
            dl.rect_border(rm, RADIUS_SM + 2.0, fill, pal.separator, 1.0);
        }
        if focus == Some(hit) {
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
            &b,
            rm.x + (rm.w - b.width) * 0.5,
            rm.y + (bh - b.height) * 0.5,
            ink,
        );
        drawn.hits.push((rm, hit));
    }
    drawn
}

fn bounds(r: Rect, scale: f64) -> accesskit::Rect {
    accesskit::Rect {
        x0: f64::from(r.x) * scale,
        y0: f64::from(r.y) * scale,
        x1: f64::from(r.x + r.w) * scale,
        y1: f64::from(r.y + r.h) * scale,
    }
}

/// The dialog's AccessKit nodes: a modal dialog with a spin button per option, a button per
/// action row (masks, folder), the warning, the progress and Cancel / Export.
pub(super) fn a11y(tree: &mut accesskit::TreeUpdate, v: &DialogView, d: &Drawn, scale: f64) {
    use accesskit::{Node, NodeId, Role};
    let mut dialog = Node::new(Role::Dialog);
    dialog.set_modal();
    dialog.set_label(v.title.clone());
    dialog.set_bounds(bounds(d.sheet, scale));
    let mut ids = Vec::new();
    let mut nodes = Vec::new();
    let focusables = v.focusables();
    let focus_hit = v.focus.map(|i| focusables[i]);
    let mut focus_id = None;
    for (i, rv) in v.rows.iter().enumerate() {
        let id = NodeId(A11Y_BASE + 1 + i as u64);
        let mut n = if rv.row.is_action() {
            let mut n = Node::new(Role::Button);
            if !v.busy {
                n.add_action(accesskit::Action::Click);
            }
            n
        } else {
            let mut n = Node::new(Role::SpinButton);
            if !v.busy {
                n.add_action(accesskit::Action::Increment);
                n.add_action(accesskit::Action::Decrement);
            }
            n
        };
        n.set_label(rv.label.clone());
        n.set_value(rv.value.clone());
        if let Some((r, _)) = d.rows.iter().find(|(_, row)| *row == rv.row) {
            n.set_bounds(bounds(*r, scale));
        }
        n.add_action(accesskit::Action::Focus);
        if focus_hit == Some(Hit::Row(rv.row)) {
            focus_id = Some(id);
        }
        ids.push(id);
        nodes.push((id, n));
    }
    if let (Some(s), Some(r)) = (&v.warning, d.warning) {
        let mut n = Node::new(Role::Label);
        n.set_value(s.clone());
        n.set_bounds(bounds(r, scale));
        let id = NodeId(A11Y_BASE + 0x100);
        ids.push(id);
        nodes.push((id, n));
    }
    for (k, (r, hit)) in buttons(d).into_iter().enumerate() {
        let id = NodeId(A11Y_BASE + 0x101 + k as u64);
        let mut n = Node::new(Role::Button);
        n.set_label(match hit {
            Hit::Cancel if v.busy => t!("convert-stop"),
            Hit::Cancel => t!("app-cancel"),
            _ => t!("anon-export"),
        });
        n.set_bounds(bounds(r, scale));
        if hit == Hit::Cancel || (v.can_export && !v.busy) {
            n.add_action(accesskit::Action::Click);
        } else {
            n.set_disabled();
        }
        n.add_action(accesskit::Action::Focus);
        if focus_hit == Some(hit) {
            focus_id = Some(id);
        }
        ids.push(id);
        nodes.push((id, n));
    }
    if let Some(s) = &v.status {
        let mut n = Node::new(Role::Status);
        n.set_live(accesskit::Live::Polite);
        n.set_label(s.clone());
        if let Some(p) = v.progress {
            n.set_numeric_value(f64::from(p) * 100.0);
        }
        let id = NodeId(A11Y_BASE + 0x1ff);
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

/// The node `id` stands for: a row or a button.
pub(super) fn a11y_target(v: &DialogView, d: &Drawn, id: u64) -> Option<Hit> {
    let k = id.checked_sub(A11Y_BASE)?;
    if (1..=v.rows.len() as u64).contains(&k) {
        return Some(Hit::Row(v.rows[k as usize - 1].row));
    }
    let k = k.checked_sub(0x101)? as usize;
    buttons(d).get(k).map(|b| b.1)
}

/// Cancel and Export as drawn, in reading order (as Tab visits them).
fn buttons(d: &Drawn) -> Vec<(Rect, Hit)> {
    [Hit::Cancel, Hit::Export]
        .iter()
        .filter_map(|o| d.hits.iter().find(|(_, h)| h == o).copied())
        .collect()
}

fn on_off(on: bool) -> String {
    if on { t!("anon-on") } else { t!("anon-off") }
}

fn keep_remove(keep: bool) -> String {
    if keep {
        t!("anon-keep")
    } else {
        t!("anon-remove")
    }
}

fn row_label(row: Row) -> String {
    match row {
        Row::Images => t!("anon-images"),
        Row::Anonymise => t!("anon-anonymise"),
        Row::Dates => t!("anon-dates"),
        Row::Patient => t!("anon-patient"),
        Row::Device => t!("anon-device"),
        Row::Institution => t!("anon-institution"),
        Row::Descriptions => t!("anon-descriptions"),
        Row::Structured => t!("anon-structured"),
        Row::Uids => t!("anon-uids"),
        Row::Overlays => t!("anon-overlays"),
        Row::BurnedIn => t!("anon-burned-in"),
        Row::Folder => t!("anon-folder"),
    }
}

pub(super) fn scope_label(s: Scope, n: usize) -> String {
    let n = n as i64;
    match s {
        Scope::Current => t!("anon-images-current"),
        Scope::Series => t!("anon-images-series", count = n),
        Scope::Study => t!("anon-images-study", count = n),
    }
}

pub(super) fn masks_label(n: usize) -> String {
    if n == 0 {
        t!("anon-masks-none")
    } else {
        t!("anon-masks", count = n as i64)
    }
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

/// The rows as the dialog shows them.
pub(super) fn row_views(
    c: &Choices,
    kind: Kind,
    images: String,
    masks: usize,
    folder: Option<&Path>,
) -> Vec<RowView> {
    let o = &c.options;
    c.rows(kind)
        .into_iter()
        .map(|row| {
            let value = match row {
                Row::Images => images.clone(),
                Row::Anonymise => on_off(c.anonymise),
                Row::Dates => match o.temporal {
                    Temporal::Remove => t!("anon-dates-remove"),
                    Temporal::Full => t!("anon-dates-keep"),
                    Temporal::Modified => t!("anon-dates-shift"),
                },
                Row::Patient => keep_remove(o.patient_characteristics),
                Row::Device => keep_remove(o.device_identity),
                Row::Institution => keep_remove(o.institution_identity),
                Row::Descriptions if o.clean_descriptors => t!("anon-descriptions-clean"),
                Row::Descriptions => t!("anon-descriptions-remove"),
                Row::Structured if o.clean_structured_content => t!("anon-descriptions-clean"),
                Row::Structured => t!("anon-descriptions-remove"),
                Row::Uids if o.retain_uids => t!("anon-uids-keep"),
                Row::Uids => t!("anon-uids-replace"),
                Row::Overlays => keep_remove(o.keep_overlays),
                Row::BurnedIn => masks_label(masks),
                Row::Folder => folder.map_or_else(|| t!("anon-folder-choose"), file_name),
            };
            RowView {
                row,
                label: row_label(row),
                value,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// The mask tool.

/// What a click on the mask tool hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MaskHit {
    Suggest,
    Remove,
    Clear,
    Done,
    Area(usize),
}

/// Everything the mask tool draws (logical px).
#[derive(Clone, Debug, Default)]
pub(super) struct MaskView {
    /// The masks on screen.
    pub areas: Vec<Rect>,
    /// Their size in image pixels (for screen readers).
    pub sizes: Vec<(u32, u32)>,
    pub selected: Option<usize>,
    /// The rectangle being dragged out.
    pub drawing: Option<Rect>,
    /// Under the bar: the hint, or what Suggest found.
    pub status: String,
    pub busy: bool,
    /// Index into [`MaskView::focusables`].
    pub focus: Option<usize>,
}

impl MaskView {
    pub fn focusables(&self) -> Vec<MaskHit> {
        [
            MaskHit::Suggest,
            MaskHit::Remove,
            MaskHit::Clear,
            MaskHit::Done,
        ]
        .into_iter()
        .chain((0..self.areas.len()).map(MaskHit::Area))
        .collect()
    }
}

/// Where the mask tool's controls landed.
#[derive(Clone, Debug, Default)]
pub(super) struct MaskDrawn {
    pub bar: Rect,
    pub hits: Vec<(Rect, MaskHit)>,
}

/// Draws the masks over the image and the tool's bar at the top of `area` (the viewport,
/// logical px).
pub(super) fn draw_mask(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    v: &MaskView,
    area: Rect,
    rtl: bool,
    ts: f32,
) -> MaskDrawn {
    let focus = v.focus.map(|i| v.focusables()[i]);
    let mut drawn = MaskDrawn::default();
    dl.push_clip(area);
    for (i, r) in v.areas.iter().enumerate() {
        let sel = v.selected == Some(i);
        let edge = if sel { pal.accent } else { Color::WHITE };
        dl.rect_border(*r, 0.0, Color::rgba(0.0, 0.0, 0.0, 0.82), edge, 1.5);
        if focus == Some(MaskHit::Area(i)) {
            dl.rect_border(r.inset(-3.0), 2.0, Color::TRANSPARENT, pal.focus_ring, 2.0);
        }
        drawn.hits.push((*r, MaskHit::Area(i)));
    }
    if let Some(r) = v.drawing {
        dl.rect_border(r, 0.0, Color::rgba(0.0, 0.0, 0.0, 0.5), pal.accent, 1.5);
    }
    dl.pop_clip();

    // The bar: title, Suggest, Remove, Clear, Done.
    let title_st = TextStyle::new(FONT_LABEL * ts).weight(650.0);
    let button_st = TextStyle::new(FONT_LABEL * ts).weight(550.0);
    let small_st = TextStyle::new(FONT_SMALL * ts);
    let title = text.layout(&t!("anon-mask-title"), title_st, None, Align::Start);
    let buttons = [
        (MaskHit::Suggest, t!("anon-mask-suggest"), !v.busy),
        (
            MaskHit::Remove,
            t!("anon-mask-remove"),
            v.selected.is_some(),
        ),
        (MaskHit::Clear, t!("anon-mask-clear"), !v.areas.is_empty()),
        (MaskHit::Done, t!("anon-mask-done"), true),
    ];
    let blocks: Vec<_> = buttons
        .iter()
        .map(|(_, l, _)| text.layout(l, button_st, None, Align::Start))
        .collect();
    let bh = 30.0;
    let gap = 8.0;
    let pad = 12.0;
    let bws: Vec<f32> = blocks.iter().map(|b| (b.width + 24.0).max(64.0)).collect();
    let row_w = title.width + 16.0 + bws.iter().sum::<f32>() + gap * 3.0;
    let max_w = (area.w - 32.0).max(120.0);
    let hint = text.layout(&v.status, small_st, Some(max_w - 2.0 * pad), Align::Center);
    let w = (row_w + 2.0 * pad).max(hint.width + 2.0 * pad).min(max_w);
    let h = pad + bh + 6.0 + hint.height + pad;
    let bar = Rect::new(
        (area.x + (area.w - w) * 0.5).round(),
        area.y + 12.0,
        w.round(),
        h.round(),
    );
    let ww = area.x * 2.0 + area.w;
    let m = |r: Rect| if rtl { r.mirror_x(ww) } else { r };
    let start_x = |x: f32, bw: f32| if rtl { ww - x - bw } else { x };
    dl.shadow(bar, RADIUS_LG, pal.shadow, 24.0, 8.0);
    dl.rect_border(bar, RADIUS_LG, pal.surface, pal.glass_border, 1.0);
    text.draw(
        dl,
        &title,
        start_x(bar.x + pad, title.width),
        bar.y + pad + (bh - title.height) * 0.5,
        pal.text,
    );
    let mut x = bar.right() - pad;
    for (i, (hit, _, on)) in buttons.iter().enumerate().rev() {
        x -= bws[i];
        let r = m(Rect::new(x, bar.y + pad, bws[i], bh));
        x -= gap;
        let done = *hit == MaskHit::Done;
        let (fill, ink) = if done {
            (pal.accent_fill, pal.accent_text)
        } else {
            (pal.surface_raised, pal.text)
        };
        let ink = if *on { ink } else { ink.with_alpha(0.45) };
        if done {
            dl.rect(r, RADIUS_SM + 2.0, fill);
        } else {
            dl.rect_border(r, RADIUS_SM + 2.0, fill, pal.separator, 1.0);
        }
        if focus == Some(*hit) {
            dl.rect_border(
                r.inset(-3.0),
                RADIUS_SM + 5.0,
                Color::TRANSPARENT,
                pal.focus_ring,
                2.0,
            );
        }
        let b = &blocks[i];
        text.draw(
            dl,
            b,
            r.x + (r.w - b.width) * 0.5,
            r.y + (bh - b.height) * 0.5,
            ink,
        );
        drawn.hits.push((r, *hit));
    }
    text.draw(
        dl,
        &hint,
        bar.x + (bar.w - hint.width) * 0.5,
        bar.y + pad + bh + 6.0,
        pal.text_muted,
    );
    drawn.bar = bar;
    drawn
}

/// The mask tool's AccessKit nodes: a toolbar with its buttons and a button per mask.
pub(super) fn mask_a11y(tree: &mut accesskit::TreeUpdate, v: &MaskView, d: &MaskDrawn, scale: f64) {
    use accesskit::{Node, NodeId, Role};
    let mut bar = Node::new(Role::Toolbar);
    bar.set_label(t!("anon-mask-title"));
    bar.set_description(v.status.clone());
    bar.set_bounds(bounds(d.bar, scale));
    let focusables = v.focusables();
    let focus_hit = v.focus.map(|i| focusables[i]);
    let mut focus_id = None;
    let mut ids = Vec::new();
    let mut nodes = Vec::new();
    for (r, hit) in &d.hits {
        let id = NodeId(MASK_A11Y_BASE + mask_node(*hit));
        let mut n = Node::new(Role::Button);
        let (label, on) = match hit {
            MaskHit::Suggest => (t!("anon-mask-suggest"), !v.busy),
            MaskHit::Remove => (t!("anon-mask-remove"), v.selected.is_some()),
            MaskHit::Clear => (t!("anon-mask-clear"), !v.areas.is_empty()),
            MaskHit::Done => (t!("anon-mask-done"), true),
            MaskHit::Area(i) => {
                let (w, h) = v.sizes.get(*i).copied().unwrap_or_default();
                n.set_selected(v.selected == Some(*i));
                (
                    t!(
                        "anon-mask-area",
                        n = (*i + 1) as i64,
                        width = i64::from(w),
                        height = i64::from(h)
                    ),
                    true,
                )
            }
        };
        n.set_label(label);
        n.set_bounds(bounds(*r, scale));
        if on {
            n.add_action(accesskit::Action::Click);
        } else {
            n.set_disabled();
        }
        n.add_action(accesskit::Action::Focus);
        if focus_hit == Some(*hit) {
            focus_id = Some(id);
        }
        ids.push(id);
        nodes.push((id, n));
    }
    bar.set_children(ids);
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        root.push_child(NodeId(MASK_A11Y_BASE));
    }
    tree.nodes.push((NodeId(MASK_A11Y_BASE), bar));
    tree.nodes.extend(nodes);
    if let Some(id) = focus_id {
        tree.focus = id;
    }
}

fn mask_node(h: MaskHit) -> u64 {
    match h {
        MaskHit::Suggest => 1,
        MaskHit::Remove => 2,
        MaskHit::Clear => 3,
        MaskHit::Done => 4,
        MaskHit::Area(i) => 0x100 + i as u64,
    }
}

fn mask_target(id: u64, areas: usize) -> Option<MaskHit> {
    match id.checked_sub(MASK_A11Y_BASE)? {
        1 => Some(MaskHit::Suggest),
        2 => Some(MaskHit::Remove),
        3 => Some(MaskHit::Clear),
        4 => Some(MaskHit::Done),
        k if (0x100..0x100 + areas as u64).contains(&k) => {
            Some(MaskHit::Area((k - 0x100) as usize))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// The work.

/// The image's series and study, from the headers of its folder.
struct Scanned {
    series: Vec<PathBuf>,
    study: Vec<PathBuf>,
    risk: Risk,
}

/// Reads the headers of the files next to `file` and keeps those of its series and study.
fn scan(file: &Path, cancel: &AtomicBool) -> Scanned {
    use specular_dicom::series::read_header;
    let mut out = Scanned {
        series: vec![file.to_owned()],
        study: vec![file.to_owned()],
        risk: Risk::None,
    };
    if let Ok(bytes) = std::fs::read(file)
        && let Ok(d) = Dicom::parse(&bytes)
    {
        out.risk = anon::risk(&d, &bytes);
    }
    let Ok(Some(me)) = read_header(file) else {
        return out;
    };
    let Some(dir) = file.parent() else {
        return out;
    };
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    let mut files: Vec<PathBuf> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.path())
        .filter(|p| super::series::is_dicom_name(p))
        .take(MAX_SCAN)
        .collect();
    files.sort();
    let mut series = Vec::new();
    let mut study = Vec::new();
    for p in files {
        if cancel.load(Ordering::Relaxed) {
            return out;
        }
        let Ok(Some(i)) = read_header(&p) else {
            continue;
        };
        if me.study_uid.is_empty() || i.study_uid != me.study_uid {
            continue;
        }
        if i.series_uid == me.series_uid {
            series.push((i.instance_number.unwrap_or(i64::MAX), p.clone()));
        }
        study.push(p);
    }
    series.sort();
    if !series.is_empty() {
        out.series = series.into_iter().map(|(_, p)| p).collect();
    }
    if !study.is_empty() {
        out.study = study;
    }
    out
}

/// Masks and the image size they are for.
#[derive(Clone, Debug, Default)]
pub(super) struct Masks {
    pub size: (u32, u32),
    pub areas: Vec<Area>,
}

/// One export to run.
struct Work {
    kind: Kind,
    anonymise: bool,
    options: Options,
    files: Vec<PathBuf>,
    masks: Masks,
    folder: PathBuf,
}

/// What an export did.
#[derive(Debug, Default)]
struct Report {
    kind: Option<Kind>,
    written: usize,
    burned_in: usize,
    failed: usize,
    cancelled: bool,
    error: Option<String>,
    folder: PathBuf,
}

/// A free file name in `dir`: `IM00001.dcm`, … (the source names may name the patient).
fn create_new(dir: &Path, n: &mut usize) -> std::io::Result<(PathBuf, std::fs::File)> {
    loop {
        *n += 1;
        let p = dir.join(format!("IM{:05}.dcm", *n));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&p)
        {
            Ok(f) => return Ok((p, f)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && *n < 1_000_000 => {}
            Err(e) => return Err(e),
        }
    }
}

/// Anonymises one file with the masks that fit it.
fn anonymise_one(
    bytes: &[u8],
    w: &Work,
    session: &anon::Session,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, AnonError> {
    let d = Dicom::parse(bytes)?;
    let fits = d
        .pixel
        .as_ref()
        .is_some_and(|p| (p.columns, p.rows) == w.masks.size);
    let masks: &[Area] = if fits { &w.masks.areas } else { &[] };
    let out = anon::anonymise(
        bytes,
        &anon::Request {
            options: &w.options,
            session,
            masks,
            allow_burned_in: false,
            cancel: Some(cancel),
        },
    )?;
    Ok(out.bytes)
}

fn run(w: Work, cancel: &AtomicBool, progress: &Sender<(usize, usize)>) -> Report {
    use std::io::Write;
    let session = anon::Session::new();
    let total = w.files.len();
    let mut r = Report {
        kind: Some(w.kind),
        folder: w.folder.clone(),
        ..Report::default()
    };
    if let Err(e) = std::fs::create_dir_all(&w.folder) {
        r.error = Some(crate::error_text::io_reason(&e));
        return r;
    }
    let mut disc = match w.kind {
        Kind::Copy => None,
        Kind::Disc => match pdi::Disc::create(&w.folder, w.anonymise) {
            Ok(d) => Some(d),
            Err(pdi::PdiError::NotEmpty) => {
                r.error = Some(t!("anon-not-empty"));
                return r;
            }
            Err(e) => {
                r.error = Some(crate::error_text::reason(&e));
                return r;
            }
        },
    };
    let mut n = 0;
    for (i, p) in w.files.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            r.cancelled = true;
            break;
        }
        let _ = progress.send((i, total));
        let bytes = match std::fs::read(p) {
            Ok(b) => b,
            Err(e) => {
                specular_log::warn!("anonymise {}: {e}", p.display());
                r.failed += 1;
                continue;
            }
        };
        let out = if w.kind == Kind::Copy || w.anonymise {
            match anonymise_one(&bytes, &w, &session, cancel) {
                Ok(b) => b,
                Err(AnonError::BurnedIn) => {
                    r.burned_in += 1;
                    continue;
                }
                Err(e) => {
                    if cancel.load(Ordering::Relaxed) {
                        r.cancelled = true;
                        break;
                    }
                    specular_log::warn!("anonymise {}: {e}", p.display());
                    r.failed += 1;
                    continue;
                }
            }
        } else {
            bytes
        };
        let ok = match disc.as_mut() {
            Some(d) => match d.add(&out) {
                Ok(_) => true,
                Err(e) => {
                    specular_log::warn!("patient disc {}: {e}", p.display());
                    false
                }
            },
            None => match create_new(&w.folder, &mut n) {
                Ok((path, mut f)) => {
                    let res = f.write_all(&out).and_then(|_| f.sync_all());
                    if let Err(e) = &res {
                        specular_log::warn!("anonymise {}: {e}", path.display());
                        drop(f);
                        let _ = std::fs::remove_file(&path);
                    }
                    res.is_ok()
                }
                Err(e) => {
                    r.error = Some(crate::error_text::reason(&e));
                    break;
                }
            },
        };
        if ok {
            r.written += 1;
        } else {
            r.failed += 1;
        }
    }
    let _ = progress.send((total, total));
    if let Some(d) = disc
        && !r.cancelled
        && r.error.is_none()
    {
        match d.finish(None) {
            Ok(wr) => r.written = wr.instances,
            Err(e) => r.error = Some(crate::error_text::reason(&e)),
        }
    }
    r
}

/// The image size and the masks the detector suggests for it.
type Suggested = Option<((u32, u32), Vec<Area>)>;

/// The suggested masks of frame 0 of `file`, and its size.
fn suggest(file: &Path) -> Suggested {
    let bytes = std::fs::read(file).ok()?;
    let d = Dicom::parse(&bytes).ok()?;
    let f = d.decode_stored(&bytes, 0, None).ok()?;
    Some(((f.width, f.height), detect::suggest(&f)))
}

// ---------------------------------------------------------------------------------------------
// State.

struct Job {
    cancel: Arc<AtomicBool>,
    progress: Receiver<(usize, usize)>,
    report: Receiver<Report>,
    done: usize,
    total: usize,
}

/// The open dialog.
pub(super) struct Dialog {
    kind: Kind,
    file: PathBuf,
    scopes: Vec<(Scope, Vec<PathBuf>)>,
    scope: usize,
    /// The user stepped the images row (the scan no longer picks it).
    scope_chosen: bool,
    scanning: Option<(Receiver<Scanned>, Arc<AtomicBool>)>,
    risk: Risk,
    picking: Option<Receiver<Option<PathBuf>>>,
    /// Export once the folder is chosen.
    export_after_pick: bool,
    job: Option<Job>,
    focus: Option<usize>,
    drawn: Drawn,
    /// Hidden while the mask tool is open.
    hidden: bool,
}

/// The open mask tool.
pub(super) struct MaskTool {
    selected: Option<usize>,
    /// The drag making a mask: its start and current point (image pixels).
    drag: Option<((f32, f32), (f32, f32))>,
    suggesting: Option<Receiver<Suggested>>,
    status: Option<String>,
    focus: Option<usize>,
    drawn: MaskDrawn,
}

/// The app's export state.
#[derive(Default)]
pub(super) struct AnonState {
    pub dialog: Option<Dialog>,
    pub tool: Option<MaskTool>,
    choices: Choices,
    folder: Option<PathBuf>,
    masks: Masks,
}

impl std::fmt::Debug for AnonState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnonState")
            .field("open", &self.dialog.is_some())
            .field("masking", &self.tool.is_some())
            .finish_non_exhaustive()
    }
}

impl AnonState {
    /// The burned-in text masks drawn this session and the image size they are for
    /// (teaching files black out the same areas).
    pub(super) fn masks(&self) -> ((u32, u32), Vec<Area>) {
        (self.masks.size, self.masks.areas.clone())
    }

    /// The dialog is on screen (not hidden behind the mask tool).
    pub(super) fn visible(&self) -> bool {
        self.dialog.as_ref().is_some_and(|d| !d.hidden)
    }

    fn view(&self) -> Option<DialogView> {
        let d = self.dialog.as_ref().filter(|d| !d.hidden)?;
        let (scope, paths) = &d.scopes[d.scope];
        let images = if d.scanning.is_some() && *scope == Scope::Current && d.scopes.len() == 1 {
            specular_i18n::dot_join(&[scope_label(*scope, 1), t!("anon-images-reading")])
        } else {
            scope_label(*scope, paths.len())
        };
        let anonymised = d.kind == Kind::Copy || self.choices.anonymise;
        let warning = match &d.risk {
            Risk::Present if anonymised && self.masks.areas.is_empty() => {
                Some(t!("anon-risk-present"))
            }
            Risk::Possible if anonymised && self.masks.areas.is_empty() => {
                Some(t!("anon-risk-possible"))
            }
            _ => None,
        };
        let (status, progress) = match &d.job {
            Some(j) if j.cancel.load(Ordering::Relaxed) => (Some(t!("anon-stopping")), None),
            Some(j) => (
                Some(t!(
                    "anon-progress",
                    done = (j.done + 1).min(j.total.max(1)) as i64,
                    total = j.total as i64
                )),
                Some(j.done as f32 / j.total.max(1) as f32),
            ),
            None => (None, None),
        };
        Some(DialogView {
            title: match d.kind {
                Kind::Copy => t!("anon-title"),
                Kind::Disc => t!("anon-disc-title"),
            },
            rows: row_views(
                &self.choices,
                d.kind,
                images,
                self.masks.areas.len(),
                self.folder.as_deref(),
            ),
            warning,
            status,
            progress,
            busy: d.job.is_some(),
            can_export: d.job.is_none() && d.picking.is_none(),
            focus: d.focus,
        })
    }

    /// Draws the dialog (when open and not hidden).
    pub(super) fn draw(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        size: (f32, f32),
        rtl: bool,
        ts: f32,
    ) -> Option<DialogView> {
        let v = self.view()?;
        let drawn = draw(dl, text, pal, &v, size, rtl, ts);
        if let Some(d) = self.dialog.as_mut() {
            d.drawn = drawn;
        }
        Some(v)
    }

    /// The dialog's nodes in the accessibility tree.
    pub(super) fn a11y(&self, tree: &mut accesskit::TreeUpdate, v: &DialogView, scale: f64) {
        if let Some(d) = &self.dialog {
            a11y(tree, v, &d.drawn, scale);
        }
    }

    /// Stores where the mask tool's controls were drawn.
    pub(super) fn set_mask_drawn(&mut self, d: MaskDrawn) {
        if let Some(t) = self.tool.as_mut() {
            t.drawn = d;
        }
    }

    pub(super) fn mask_a11y(&self, tree: &mut accesskit::TreeUpdate, v: &MaskView, scale: f64) {
        if let Some(t) = &self.tool {
            mask_a11y(tree, v, &t.drawn, scale);
        }
    }
}

impl App {
    /// The scan, export or detector panicked: the dialog stops waiting for it.
    pub(super) fn anon_lost(&mut self, task: &str) {
        if let Some(d) = self.anon.dialog.as_mut() {
            match task {
                "anon-scan" => d.scanning = None,
                "anon-export" => d.job = None,
                _ => {}
            }
        }
        if task == "anon-suggest"
            && let Some(t) = self.anon.tool.as_mut()
        {
            t.suggesting = None;
        }
    }

    fn anon_current(&self) -> Option<PathBuf> {
        self.current.and_then(|i| self.entries.get(i)).cloned()
    }

    pub(super) fn anon_open(&mut self, kind: Kind) {
        if self.anon.dialog.is_some() {
            return;
        }
        let Some(file) = self.anon_current() else {
            return;
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = crossbeam_channel::bounded(1);
        let (f, c) = (file.clone(), cancel.clone());
        self.loader.background("anon-scan", move |l| {
            let _ = tx.send(scan(&f, &c));
            l.waker()();
        });
        // Masks made on another image size do not apply here.
        let size = self.view.stored_size();
        if self.anon.masks.size != size {
            self.anon.masks = Masks {
                size,
                areas: Vec::new(),
            };
        }
        self.anon.dialog = Some(Dialog {
            kind,
            file: file.clone(),
            scopes: vec![(Scope::Current, vec![file])],
            scope: 0,
            scope_chosen: false,
            scanning: Some((rx, cancel)),
            risk: Risk::None,
            picking: None,
            export_after_pick: false,
            job: None,
            focus: Some(0),
            drawn: Drawn::default(),
            hidden: false,
        });
        self.chrome.announce_status(match kind {
            Kind::Copy => t!("anon-title"),
            Kind::Disc => t!("anon-disc-title"),
        });
        self.redraw = true;
    }

    fn anon_close(&mut self) {
        if let Some(d) = self.anon.dialog.take() {
            if let Some((_, c)) = d.scanning {
                c.store(true, Ordering::Relaxed);
            }
            if let Some(j) = d.job {
                j.cancel.store(true, Ordering::Relaxed);
            }
        }
        self.redraw = true;
    }

    fn anon_view(&self) -> Option<DialogView> {
        self.anon.view()
    }

    /// Replies from the scan, the folder chooser, the export and the detector.
    pub(super) fn anon_tick(&mut self) {
        let mut report = None;
        let mut picked = None;
        if let Some(d) = self.anon.dialog.as_mut() {
            if let Some((rx, _)) = &d.scanning
                && let Ok(s) = rx.try_recv()
            {
                d.scanning = None;
                d.risk = s.risk;
                if s.series.len() > 1 {
                    d.scopes.push((Scope::Series, s.series.clone()));
                }
                if s.study.len() > s.series.len().max(1) {
                    d.scopes.push((Scope::Study, s.study));
                }
                // A disc holds the study unless the user chose otherwise.
                if d.kind == Kind::Disc && !d.scope_chosen {
                    d.scope = d.scopes.len() - 1;
                }
                self.redraw = true;
            }
            if let Some(rx) = &d.picking
                && let Ok(p) = rx.try_recv()
            {
                d.picking = None;
                picked = Some(p);
                self.redraw = true;
            }
            if let Some(j) = d.job.as_mut() {
                while let Ok((done, total)) = j.progress.try_recv() {
                    j.done = done;
                    j.total = total;
                    self.redraw = true;
                }
                if let Ok(r) = j.report.try_recv() {
                    d.job = None;
                    report = Some(r);
                }
            }
        }
        if let Some(p) = picked {
            let after = self
                .anon
                .dialog
                .as_mut()
                .is_some_and(|d| std::mem::take(&mut d.export_after_pick));
            if let Some(p) = p {
                self.chrome.announce_status(file_name(&p));
                self.anon.folder = Some(p);
                if after {
                    self.anon_export();
                }
            }
        }
        if let Some(r) = report {
            self.anon_finished(r);
        }
        if let Some(t) = self.anon.tool.as_mut()
            && let Some(rx) = &t.suggesting
            && let Ok(s) = rx.try_recv()
        {
            t.suggesting = None;
            let found = match s {
                Some((size, areas)) if size == self.anon.masks.size => {
                    let n = areas.len();
                    for a in areas {
                        if !self.anon.masks.areas.contains(&a) {
                            self.anon.masks.areas.push(a);
                        }
                    }
                    n
                }
                _ => 0,
            };
            let msg = t!("anon-mask-suggested", count = found as i64);
            t.status = Some(msg.clone());
            self.chrome.announce_status(msg);
            self.redraw = true;
        }
    }

    fn anon_finished(&mut self, r: Report) {
        let now = self.now();
        if let Some(e) = r.error {
            self.chrome.warning(t!("anon-failed", reason = e), now);
            return;
        }
        let folder = file_name(&r.folder);
        let mut msg = if r.cancelled {
            t!("anon-stopped")
        } else if r.kind == Some(Kind::Disc) {
            t!("anon-disc-done", count = r.written as i64, folder = folder)
        } else {
            t!("anon-done", count = r.written as i64, folder = folder)
        };
        if r.burned_in > 0 {
            msg = format!(
                "{msg}. {}",
                t!("anon-skipped-burned-in", count = r.burned_in as i64)
            );
        }
        if r.failed > 0 {
            msg = format!(
                "{msg}. {}",
                t!("anon-skipped-failed", count = r.failed as i64)
            );
        }
        if r.burned_in > 0 || r.failed > 0 {
            self.chrome.warning(msg, now);
        } else {
            self.chrome.toast(msg, None, now);
        }
        let what = if r.kind == Some(Kind::Disc) {
            "disc"
        } else {
            "folder"
        };
        let source = self.anon_current();
        let mut paths = vec![r.folder.as_path()];
        paths.extend(source.as_deref());
        let extra = [
            ("written", serde_json::json!(r.written)),
            ("skipped_burned_in", serde_json::json!(r.burned_in)),
            ("failed", serde_json::json!(r.failed)),
            ("cancelled", serde_json::json!(r.cancelled)),
        ];
        self.audit(super::audit::Event::Anonymise, what, &paths, &extra);
        if !r.cancelled {
            self.anon_close();
        }
        self.redraw = true;
    }

    fn anon_pick_folder(&mut self, then_export: bool) {
        let Some(d) = self.anon.dialog.as_mut() else {
            return;
        };
        if d.picking.is_some() || d.job.is_some() {
            return;
        }
        d.export_after_pick = then_export;
        let start = self
            .anon
            .folder
            .clone()
            .or_else(|| d.file.parent().map(Path::to_owned));
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("dlg-anon", move |l| {
            let mut dlg = rfd::FileDialog::new();
            if let Some(dir) = start {
                dlg = dlg.set_directory(dir);
            }
            let _ = tx.send(dlg.pick_folder());
            l.waker()();
        });
        d.picking = Some(rx);
    }

    fn anon_export(&mut self) {
        let Some(folder) = self.anon.folder.clone() else {
            self.anon_pick_folder(true);
            return;
        };
        let choices = self.anon.choices.clone();
        let masks = self.anon.masks.clone();
        let Some(d) = self.anon.dialog.as_mut() else {
            return;
        };
        if d.job.is_some() || d.picking.is_some() {
            return;
        }
        let files = d.scopes[d.scope].1.clone();
        let total = files.len();
        let work = Work {
            kind: d.kind,
            anonymise: d.kind == Kind::Copy || choices.anonymise,
            options: choices.options,
            files,
            masks,
            folder,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let (ptx, prx) = crossbeam_channel::unbounded();
        let (rtx, rrx) = crossbeam_channel::bounded(1);
        let c = cancel.clone();
        specular_log::info!("anonymised export: {total} files ({:?})", work.kind);
        self.loader.background("anon-export", move |l| {
            let wake = l.waker();
            let (tx, w2) = (ptx, wake.clone());
            let (fwd_tx, fwd_rx) = crossbeam_channel::unbounded::<(usize, usize)>();
            // Progress wakes the UI at most every file.
            let fwd = specular_thread::spawn("anon-progress", move || {
                for p in fwd_rx {
                    let _ = tx.send(p);
                    w2();
                }
            });
            let r = run(work, &c, &fwd_tx);
            drop(fwd_tx);
            let _ = fwd.join();
            let _ = rtx.send(r);
            wake();
        });
        d.job = Some(Job {
            cancel,
            progress: prx,
            report: rrx,
            done: 0,
            total,
        });
        self.redraw = true;
    }

    fn anon_step(&mut self, row: Row, delta: i32) {
        let Some(d) = self.anon.dialog.as_mut() else {
            return;
        };
        if d.job.is_some() {
            return;
        }
        if row == Row::Images {
            if d.scopes.len() > 1 {
                d.scope = (d.scope as i32 + delta).rem_euclid(d.scopes.len() as i32) as usize;
                d.scope_chosen = true;
                let (s, p) = &d.scopes[d.scope];
                let label = scope_label(*s, p.len());
                self.chrome.announce_status(label);
            }
            return;
        }
        if row.is_action() {
            self.anon_activate(row);
            return;
        }
        if self.anon.choices.step(row, delta) {
            // Focus stays on the row (rows come and go with Anonymise).
            if let Some(v) = self.anon_view()
                && let Some(i) = v.focusables().iter().position(|h| *h == Hit::Row(row))
                && let Some(d) = self.anon.dialog.as_mut()
            {
                d.focus = Some(i);
                if let Some(rv) = v.rows.iter().find(|r| r.row == row) {
                    self.chrome.announce_status(rv.value.clone());
                }
            }
        }
    }

    fn anon_activate(&mut self, row: Row) {
        match row {
            Row::Folder => self.anon_pick_folder(false),
            Row::BurnedIn => self.anon_mask_open(),
            r => self.anon_step(r, 1),
        }
    }

    fn anon_hit(&mut self, h: Hit) {
        match h {
            Hit::Dec(r) => self.anon_step(r, -1),
            Hit::Inc(r) => self.anon_step(r, 1),
            Hit::Row(r) => {
                if let Some(v) = self.anon_view()
                    && let Some(i) = v.focusables().iter().position(|x| *x == Hit::Row(r))
                    && let Some(d) = self.anon.dialog.as_mut()
                {
                    d.focus = Some(i);
                }
                if r.is_action() {
                    self.anon_activate(r);
                }
            }
            Hit::Cancel => {
                let job = self.anon.dialog.as_ref().and_then(|d| d.job.as_ref());
                if let Some(j) = job {
                    j.cancel.store(true, Ordering::Relaxed);
                } else {
                    self.anon_close();
                }
            }
            Hit::Export => self.anon_export(),
        }
        self.redraw = true;
    }

    /// Keys while the dialog is shown (it takes them all).
    pub(super) fn anon_key(&mut self, combo: Option<&KeyCombo>) -> bool {
        let Some(v) = self.anon_view() else {
            return false;
        };
        let Some(combo) = combo else { return true };
        let focusables = v.focusables();
        let n = focusables.len();
        let focus = v.focus.map(|i| focusables[i]);
        let rtl = self.rtl();
        let move_focus = |me: &mut App, d: i32| {
            if let Some(dl) = me.anon.dialog.as_mut() {
                let i = dl
                    .focus
                    .map_or(0, |i| (i as i32 + d).rem_euclid(n as i32) as usize);
                dl.focus = Some(i);
            }
        };
        let plain = combo.mods == Mods::NONE;
        match combo.key {
            Key::Named(NamedKey::Escape) => self.anon_hit(Hit::Cancel),
            Key::Named(NamedKey::Tab) if combo.mods.shift => move_focus(self, -1),
            Key::Named(NamedKey::Tab) => move_focus(self, 1),
            Key::Named(NamedKey::ArrowUp) if plain => move_focus(self, -1),
            Key::Named(NamedKey::ArrowDown) if plain => move_focus(self, 1),
            Key::Named(k @ (NamedKey::ArrowLeft | NamedKey::ArrowRight)) if plain => {
                let forward = (k == NamedKey::ArrowRight) != rtl;
                match focus {
                    Some(Hit::Row(r)) if !r.is_action() => {
                        self.anon_step(r, if forward { 1 } else { -1 })
                    }
                    _ => move_focus(self, if forward { 1 } else { -1 }),
                }
            }
            Key::Named(NamedKey::Enter | NamedKey::Space) => match focus {
                Some(h @ (Hit::Cancel | Hit::Export)) => self.anon_hit(h),
                Some(Hit::Row(r)) if r.is_action() => self.anon_activate(r),
                Some(Hit::Row(r)) if combo.key == Key::Named(NamedKey::Space) => {
                    self.anon_step(r, 1)
                }
                _ => self.anon_hit(Hit::Export),
            },
            _ => {}
        }
        self.redraw = true;
        true
    }

    /// A click while the dialog is shown (it takes them all).
    pub(super) fn anon_click(&mut self, p: (f32, f32), down: bool) -> bool {
        let Some(d) = self.anon.dialog.as_ref().filter(|d| !d.hidden) else {
            return false;
        };
        if down
            && let Some(h) = d
                .drawn
                .hits
                .iter()
                .find(|(r, h)| r.contains(p) && !matches!(h, Hit::Row(_)))
                .or_else(|| d.drawn.hits.iter().find(|(r, _)| r.contains(p)))
                .map(|x| x.1)
        {
            self.anon_hit(h);
        }
        true
    }

    /// A screen reader's request on the dialog's or the mask tool's nodes; false when not
    /// ours.
    pub(super) fn anon_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        if self.anon.tool.is_some() && req.target_node.0 & !0xFFFF == MASK_A11Y_BASE {
            let n = self.anon.masks.areas.len();
            if let Some(h) = mask_target(req.target_node.0, n) {
                match req.action {
                    accesskit::Action::Click => self.anon_mask_hit(h),
                    accesskit::Action::Focus => {
                        if let Some(v) = self.anon_mask_view()
                            && let Some(i) = v.focusables().iter().position(|x| *x == h)
                            && let Some(t) = self.anon.tool.as_mut()
                        {
                            t.focus = Some(i);
                        }
                    }
                    _ => {}
                }
            }
            self.redraw = true;
            return true;
        }
        let Some(v) = self.anon_view() else {
            return false;
        };
        let Some(d) = self.anon.dialog.as_ref() else {
            return false;
        };
        let Some(hit) = a11y_target(&v, &d.drawn, req.target_node.0) else {
            return req.target_node.0 == A11Y_BASE;
        };
        match (req.action, hit) {
            (accesskit::Action::Increment, Hit::Row(r)) => self.anon_step(r, 1),
            (accesskit::Action::Decrement, Hit::Row(r)) => self.anon_step(r, -1),
            (accesskit::Action::Click, h) => self.anon_hit(h),
            (accesskit::Action::Focus, h) => {
                if let Some(i) = v.focusables().iter().position(|x| *x == h)
                    && let Some(d) = self.anon.dialog.as_mut()
                {
                    d.focus = Some(i);
                }
            }
            _ => {}
        }
        self.redraw = true;
        true
    }

    // -- The mask tool ------------------------------------------------------------------------

    /// Opens the mask tool over the image (hiding the dialog while it is open); the first time
    /// for an image size, the detector suggests masks.
    pub(super) fn anon_mask_open(&mut self) {
        if self.anon.tool.is_some() || self.shown.is_none() {
            return;
        }
        let Some(file) = self.anon_current() else {
            return;
        };
        let size = self.view.stored_size();
        if self.anon.masks.size != size {
            self.anon.masks = Masks {
                size,
                areas: Vec::new(),
            };
        }
        if let Some(d) = self.anon.dialog.as_mut() {
            d.hidden = true;
        }
        let mut tool = MaskTool {
            selected: None,
            drag: None,
            suggesting: None,
            status: None,
            focus: None,
            drawn: MaskDrawn::default(),
        };
        if self.anon.masks.areas.is_empty() {
            tool.suggesting = Some(self.anon_suggest(file));
        }
        self.anon.tool = Some(tool);
        self.chrome.announce_status(t!("anon-mask-hint"));
        self.redraw = true;
    }

    fn anon_suggest(&self, file: PathBuf) -> Receiver<Suggested> {
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("anon-suggest", move |l| {
            let _ = tx.send(suggest(&file));
            l.waker()();
        });
        rx
    }

    fn anon_mask_close(&mut self) {
        self.anon.tool = None;
        if let Some(d) = self.anon.dialog.as_mut() {
            d.hidden = false;
        }
        let n = self.anon.masks.areas.len();
        self.chrome.announce_status(masks_label(n));
        self.redraw = true;
    }

    /// Image pixels from a window point (physical px), clamped to the image.
    fn anon_stored(&self, p: (f32, f32)) -> (f32, f32) {
        let (x, y) = self.view.stored_from_screen().apply(p);
        let (w, h) = self.view.stored_size();
        (x.clamp(0.0, w as f32), y.clamp(0.0, h as f32))
    }

    /// An image-pixel rectangle on screen (logical px).
    fn anon_screen_rect(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        let a = self.view.stored_from_screen().invert();
        let s = self.dl.scale.max(0.1);
        let ps = [(x0, y0), (x1, y0), (x0, y1), (x1, y1)].map(|p| {
            let q = a.apply(p);
            (q.0 / s, q.1 / s)
        });
        let lx = ps.iter().map(|p| p.0).fold(f32::MAX, f32::min);
        let ly = ps.iter().map(|p| p.1).fold(f32::MAX, f32::min);
        let hx = ps.iter().map(|p| p.0).fold(f32::MIN, f32::max);
        let hy = ps.iter().map(|p| p.1).fold(f32::MIN, f32::max);
        Rect::new(lx, ly, hx - lx, hy - ly)
    }

    /// What the mask tool draws, when open.
    pub(super) fn anon_mask_view(&self) -> Option<MaskView> {
        let t = self.anon.tool.as_ref()?;
        let m = &self.anon.masks;
        let areas = m
            .areas
            .iter()
            .map(|a| {
                self.anon_screen_rect(
                    a.x as f32,
                    a.y as f32,
                    (a.x + a.w) as f32,
                    (a.y + a.h) as f32,
                )
            })
            .collect();
        let drawing = t.drag.map(|(a, b)| {
            self.anon_screen_rect(a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1))
        });
        Some(MaskView {
            areas,
            sizes: m.areas.iter().map(|a| (a.w, a.h)).collect(),
            selected: t.selected,
            drawing,
            status: t.status.clone().unwrap_or_else(|| t!("anon-mask-hint")),
            busy: t.suggesting.is_some(),
            focus: t.focus,
        })
    }

    fn anon_mask_hit(&mut self, h: MaskHit) {
        let Some(t) = self.anon.tool.as_mut() else {
            return;
        };
        match h {
            MaskHit::Suggest => {
                if t.suggesting.is_none()
                    && let Some(f) = self.anon_current()
                {
                    let rx = self.anon_suggest(f);
                    if let Some(t) = self.anon.tool.as_mut() {
                        t.suggesting = Some(rx);
                    }
                }
            }
            MaskHit::Remove => {
                if let Some(i) = t.selected.take()
                    && i < self.anon.masks.areas.len()
                {
                    self.anon.masks.areas.remove(i);
                    t.focus = None;
                    let n = self.anon.masks.areas.len();
                    self.chrome.announce_status(masks_label(n));
                }
            }
            MaskHit::Clear => {
                self.anon.masks.areas.clear();
                t.selected = None;
                t.focus = None;
                self.chrome.announce_status(masks_label(0));
            }
            MaskHit::Done => self.anon_mask_close(),
            MaskHit::Area(i) => {
                t.selected = Some(i);
            }
        }
        self.redraw = true;
    }

    /// Keys while the mask tool is open: Esc and Enter finish, Delete removes the selected
    /// mask, Tab moves the focus. It takes every key (the image must not change under it).
    pub(super) fn anon_mask_key(&mut self, combo: Option<&KeyCombo>) -> bool {
        let Some(v) = self.anon_mask_view() else {
            return false;
        };
        let Some(combo) = combo else { return true };
        let focusables = v.focusables();
        let n = focusables.len();
        let focus = v.focus.map(|i| focusables[i]);
        let move_focus = |me: &mut App, d: i32| {
            if let Some(t) = me.anon.tool.as_mut() {
                let i = t
                    .focus
                    .map_or(0, |i| (i as i32 + d).rem_euclid(n as i32) as usize);
                t.focus = Some(i);
                if let MaskHit::Area(k) = focusables[i] {
                    t.selected = Some(k);
                }
            }
        };
        match combo.key {
            Key::Named(NamedKey::Escape) => {
                let dragging = self.anon.tool.as_mut().and_then(|t| t.drag.take());
                if dragging.is_none() {
                    self.anon_mask_close();
                }
            }
            Key::Named(NamedKey::Tab) if combo.mods.shift => move_focus(self, -1),
            Key::Named(NamedKey::Tab) => move_focus(self, 1),
            Key::Named(NamedKey::Delete | NamedKey::Backspace) => {
                self.anon_mask_hit(MaskHit::Remove)
            }
            Key::Named(NamedKey::Enter | NamedKey::Space) => match focus {
                Some(h) => self.anon_mask_hit(h),
                None => self.anon_mask_close(),
            },
            _ => {}
        }
        self.redraw = true;
        true
    }

    /// The left button while the mask tool is open: the bar's buttons, selecting a mask, or
    /// dragging out a new one. `p` is in physical px, `lp` in logical px.
    pub(super) fn anon_mask_button(&mut self, p: (f32, f32), lp: (f32, f32), down: bool) -> bool {
        let Some(t) = self.anon.tool.as_ref() else {
            return false;
        };
        if down {
            if let Some(h) = t
                .drawn
                .hits
                .iter()
                .find(|(r, h)| r.contains(lp) && !matches!(h, MaskHit::Area(_)))
                .map(|x| x.1)
            {
                self.anon_mask_hit(h);
                return true;
            }
            if t.drawn.bar.contains(lp) {
                return true;
            }
            let q = self.anon_stored(p);
            let inside = self
                .anon
                .masks
                .areas
                .iter()
                .rposition(|a| a.contains(q.0 as u32, q.1 as u32));
            if let Some(t) = self.anon.tool.as_mut() {
                t.selected = inside;
                t.drag = Some((q, q));
            }
            self.redraw = true;
            return true;
        }
        let Some((a, b)) = self.anon.tool.as_mut().and_then(|t| t.drag.take()) else {
            return true;
        };
        let (x0, y0) = (a.0.min(b.0).floor(), a.1.min(b.1).floor());
        let (x1, y1) = (a.0.max(b.0).ceil(), a.1.max(b.1).ceil());
        if x1 - x0 >= MIN_AREA && y1 - y0 >= MIN_AREA {
            let area = Area::new(x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32);
            self.anon.masks.areas.push(area);
            let n = self.anon.masks.areas.len();
            if let Some(t) = self.anon.tool.as_mut() {
                t.selected = Some(n - 1);
                t.status = None;
            }
            self.chrome.announce_status(masks_label(n));
        }
        self.redraw = true;
        true
    }

    /// Pointer motion while the mask tool is open; true while dragging out a mask.
    pub(super) fn anon_mask_motion(&mut self, p: (f32, f32)) -> bool {
        let dragging = self.anon.tool.as_ref().is_some_and(|t| t.drag.is_some());
        if !dragging {
            return false;
        }
        let q = self.anon_stored(p);
        if let Some(t) = self.anon.tool.as_mut()
            && let Some((_, b)) = t.drag.as_mut()
        {
            *b = q;
        }
        self.redraw = true;
        true
    }

    /// `--show anon-export`, `anon-disc` and `anon-mask` (screenshots).
    pub(super) fn anon_show(&mut self, what: &str) -> bool {
        match what {
            "anon-export" => self.anon_open(Kind::Copy),
            "anon-disc" => self.anon_open(Kind::Disc),
            "anon-mask" => self.anon_mask_open(),
            _ => return false,
        }
        true
    }
}
