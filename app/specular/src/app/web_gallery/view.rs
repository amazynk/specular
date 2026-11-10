//! The client gallery and client picks dialogs as data, their drawing and their accessibility
//! tree. Nothing here holds app state, so the screenshot tests draw it without an app.
//!
//! One sheet layout serves both: the heading and a muted subtitle, then rows top to bottom
//! (text fields with their label above, `‹ value ›` choices, checkboxes, a secondary
//! button, notes), a status line, and the buttons at the end edge, the default last. Right
//! to left mirrors it.

use std::sync::Arc;

use specular_gfx::{Align, DrawList, Icon, Rect, Text, TextBlock, TextStyle, draw_icon};
use specular_theme::metrics::{FONT_LABEL, FONT_SMALL, FONT_TITLE, RADIUS_LG, RADIUS_SM};
use specular_theme::{Color, Palette};
use specular_widgets::{accesskit, line_edit};

pub(in crate::app) const A11Y_BASE: u64 = 0x5B00_0000_0000_0000;

/// What can have the keyboard, in no particular order (the rows give the order).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in crate::app) enum Part {
    Title,
    Watermark,
    Source,
    Theme,
    Size,
    Format,
    Captions,
    Exif,
    Location,
    /// The client's list (picks dialog).
    Paste,
    /// Choose picks file….
    Choose,
    /// How the matched photos are marked.
    ApplyAs,
    Cancel,
    /// Write gallery… / Apply.
    Ok,
}

impl Part {
    const ALL: [Part; 14] = [
        Part::Title,
        Part::Watermark,
        Part::Source,
        Part::Theme,
        Part::Size,
        Part::Format,
        Part::Captions,
        Part::Exif,
        Part::Location,
        Part::Paste,
        Part::Choose,
        Part::ApplyAs,
        Part::Cancel,
        Part::Ok,
    ];

    pub fn is_field(self) -> bool {
        matches!(self, Part::Title | Part::Watermark | Part::Paste)
    }

    fn index(self) -> u64 {
        Part::ALL.iter().position(|p| *p == self).unwrap_or(0) as u64
    }
}

/// A text field as drawn: its label, text and caret (a byte offset).
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::app) struct FieldView {
    pub label: String,
    pub text: String,
    pub caret: usize,
    pub selection: std::ops::Range<usize>,
    /// An input method's composition at the caret (empty: none); only the focused field's.
    pub preedit: String,
}

/// One row of the sheet.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) enum Row {
    Field(Part, FieldView),
    /// `label  ‹ value ›`.
    Choice(Part, String, String),
    Check(Part, String, bool),
    /// A secondary button on a line of its own.
    Button(Part, String),
    /// Text; `strong` ones sit in a tinted box with an icon (what the user must know).
    Note(String, bool),
}

impl Row {
    fn part(&self) -> Option<Part> {
        match self {
            Row::Field(p, _) | Row::Choice(p, ..) | Row::Check(p, ..) | Row::Button(p, _) => {
                Some(*p)
            }
            Row::Note(..) => None,
        }
    }
}

/// Everything a dialog draws.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct FormView {
    pub heading: String,
    pub subtitle: String,
    pub rows: Vec<Row>,
    /// Work going on (the rows are then dimmed and the default button off).
    pub busy: Option<String>,
    pub cancel: String,
    pub ok: String,
    pub ok_enabled: bool,
    pub focus: Part,
}

impl FormView {
    /// The parts Tab visits, in order.
    pub fn parts(&self) -> Vec<Part> {
        self.rows
            .iter()
            .filter_map(Row::part)
            .chain([Part::Cancel, Part::Ok])
            .collect()
    }
}

/// What a click or a screen reader hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Hit {
    Part(Part),
    Dec(Part),
    Inc(Part),
}

/// Where things landed.
#[derive(Clone, Debug, Default)]
pub(in crate::app) struct Drawn {
    pub sheet: Rect,
    pub hits: Vec<(Rect, Hit)>,
    /// Each field's laid-out text and the x it is drawn at (a click places the caret).
    pub fields: Vec<(Part, Arc<TextBlock>, f32)>,
    /// Where the input method's candidate window opens.
    pub ime: Option<Rect>,
}

impl Drawn {
    fn rect_of(&self, part: Part) -> Option<Rect> {
        self.hits
            .iter()
            .find(|(_, h)| *h == Hit::Part(part))
            .map(|(r, _)| *r)
    }
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

fn focus_ring(dl: &mut DrawList, pal: &Palette, r: Rect, radius: f32) {
    dl.rect_border(r, radius, Color::TRANSPARENT, pal.focus_ring, 2.0);
}

/// Draws the dialog over a scrim in a window of `size` logical px.
pub(in crate::app) fn draw(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    v: &FormView,
    size: (f32, f32),
    rtl: bool,
    ts: f32,
) -> Drawn {
    let (ww, wh) = size;
    dl.rect(Rect::new(0.0, 0.0, ww, wh), 0.0, pal.scrim);
    let pad = 20.0;
    let w = (ww - 32.0)
        .clamp(280.0, 520.0 + 160.0 * (ts - 1.0).max(0.0))
        .min(ww);
    let inner = w - 2.0 * pad;
    let mi = Mirror { ww, rtl };
    let title_st = TextStyle::new(FONT_TITLE * ts).weight(650.0);
    let label_st = TextStyle::new(FONT_LABEL * ts);
    let value_st = TextStyle::new(FONT_LABEL * ts).weight(550.0);
    let small_st = TextStyle::new(FONT_SMALL * ts);
    let button_st = TextStyle::new(FONT_LABEL * ts).weight(550.0);
    let busy = v.busy.is_some();
    let ink = if busy { pal.text_faint } else { pal.text };

    let heading = text.layout(&v.heading, title_st, Some(inner), Align::Start);
    let subtitle = text.layout(&v.subtitle, small_st, Some(inner), Align::Start);
    let box_side = (16.0 * ts).max(18.0);
    let field_h = (26.0 * ts).max(30.0);
    let row_h = (24.0 * ts).max(30.0);
    let bh = (24.0 * ts).max(32.0);
    let arrow = (row_h - 6.0).min(26.0 * ts);
    let icon_w = 28.0;

    // Lay out the rows' text first: the sheet's height follows from it.
    let blocks: Vec<Arc<TextBlock>> = v
        .rows
        .iter()
        .map(|r| match r {
            Row::Field(_, f) => text.layout(&f.label, small_st, Some(inner), Align::Start),
            Row::Choice(_, l, _) => text.layout(l, label_st, Some(inner * 0.42), Align::Start),
            Row::Check(_, l, _) => {
                text.layout(l, label_st, Some(inner - box_side - 10.0), Align::Start)
            }
            Row::Button(_, l) => text.layout(l, button_st, None, Align::Start),
            Row::Note(t, strong) => text.layout(
                t,
                if *strong { label_st } else { small_st },
                Some(if *strong { inner - icon_w - 8.0 } else { inner }),
                Align::Start,
            ),
        })
        .collect();
    let heights: Vec<f32> = v
        .rows
        .iter()
        .zip(&blocks)
        .map(|(r, b)| match r {
            Row::Field(..) => b.height + 4.0 + field_h,
            Row::Choice(..) => row_h.max(b.height),
            Row::Check(..) => b.height.max(box_side),
            Row::Button(..) => bh,
            Row::Note(_, true) => b.height + 12.0,
            Row::Note(_, false) => b.height,
        })
        .collect();
    let gap = 10.0;
    let status = v
        .busy
        .as_ref()
        .map(|s| text.layout(s, label_st, Some(inner), Align::Start));
    let mut h = pad + heading.height + 6.0 + subtitle.height + 14.0;
    h += heights.iter().map(|x| x + gap).sum::<f32>();
    if let Some(b) = &status {
        h += b.height + 10.0;
    }
    h += 6.0 + bh + pad;
    let h = h.min(wh - 16.0);
    let sheet = Rect::new(((ww - w) * 0.5).round(), ((wh - h) * 0.5).round(), w, h);
    dl.shadow(sheet, RADIUS_LG, pal.shadow, 40.0, 14.0);
    dl.rect_border(sheet, RADIUS_LG, pal.surface, pal.glass_border, 1.0);
    let mut drawn = Drawn {
        sheet,
        ..Drawn::default()
    };
    let x0 = sheet.x + pad;
    let mut y = sheet.y + pad;
    text.draw(dl, &heading, mi.x(x0, heading.width), y, pal.text);
    y += heading.height + 6.0;
    text.draw(dl, &subtitle, mi.x(x0, subtitle.width), y, pal.text_muted);
    y += subtitle.height + 14.0;
    dl.push_clip(sheet);

    for ((row, block), rh) in v.rows.iter().zip(&blocks).zip(&heights) {
        match row {
            Row::Field(part, f) => {
                let top = y;
                text.draw(dl, block, mi.x(x0, block.width), y, pal.text_muted);
                let fy = y + block.height + 4.0;
                let fr = mi.r(Rect::new(x0, fy, inner, field_h));
                let focused = v.focus == *part;
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
                let room = fr.inset_xy(10.0, 0.0);
                let pre = if focused { f.preedit.as_str() } else { "" };
                let (shown, composing) = line_edit::compose(&f.text, f.caret, pre);
                let tb = text.layout(&shown, label_st, None, Align::Start);
                let cx = tb.caret_x(composing.end);
                let bx = line_edit::field_x0(&tb, cx, room, rtl || tb.rtl);
                let ty = fr.y + (fr.h - tb.height) * 0.5;
                dl.push_clip(room.inset_xy(-2.0, 0.0));
                if focused && composing.is_empty() {
                    for (a, b) in tb.range_spans(f.selection.clone()) {
                        dl.rect(Rect::new(bx + a, ty, b - a, tb.height), 0.0, pal.selection);
                    }
                }
                text.draw(dl, &tb, bx, ty, ink);
                if focused && !busy {
                    let cx = if shown.is_empty() && rtl {
                        room.right()
                    } else {
                        bx + cx
                    };
                    let caret = Rect::new((cx - 0.75).round(), ty, 1.5, tb.height);
                    dl.rect(caret, 0.0, pal.text);
                    drawn.ime = Some(if composing.is_empty() {
                        caret
                    } else {
                        line_edit::draw_preedit_underline(
                            dl,
                            &tb,
                            bx,
                            composing.clone(),
                            ty + tb.height,
                            pal.text_muted,
                        );
                        line_edit::ime_area(&tb, bx, composing, ty, tb.height, room)
                    });
                }
                dl.pop_clip();
                drawn.fields.push((*part, tb, bx));
                drawn.hits.push((
                    mi.r(Rect::new(x0, top, inner, fy + field_h - top)),
                    Hit::Part(*part),
                ));
            }
            Row::Choice(part, _, value) => {
                let r = Rect::new(x0, y, inner, *rh);
                if v.focus == *part {
                    focus_ring(dl, pal, mi.r(r.inset(-2.0)), RADIUS_SM);
                }
                text.draw(
                    dl,
                    block,
                    mi.x(x0 + 4.0, block.width),
                    y + (rh - block.height) * 0.5,
                    pal.text_muted,
                );
                let sw = (inner * 0.56).min(300.0 * ts);
                let sx = r.right() - sw;
                let dec = Rect::new(sx, y + (rh - arrow) * 0.5, arrow, arrow);
                let inc = Rect::new(r.right() - arrow, dec.y, arrow, arrow);
                for (b, icon, hit) in [
                    (dec, Icon::ChevronLeft, Hit::Dec(*part)),
                    (inc, Icon::ChevronRight, Hit::Inc(*part)),
                ] {
                    let b = mi.r(b);
                    dl.rect(b, RADIUS_SM, pal.surface_raised);
                    draw_icon(dl, icon, b.inset(arrow * 0.18), ink, 1.6, rtl);
                    drawn.hits.push((b, hit));
                }
                let val = text.layout(value, value_st, None, Align::Start);
                let room = sw - 2.0 * arrow;
                let mid = sx + arrow + (room - val.width.min(room)) * 0.5;
                dl.push_clip(mi.r(Rect::new(sx + arrow, y, room, *rh)));
                text.draw(
                    dl,
                    &val,
                    mi.x(mid, val.width),
                    y + (rh - val.height) * 0.5,
                    ink,
                );
                dl.pop_clip();
                drawn.hits.push((mi.r(r), Hit::Part(*part)));
            }
            Row::Check(part, _, on) => {
                let bm = mi.r(Rect::new(x0, y + (rh - box_side) * 0.5, box_side, box_side));
                if *on {
                    dl.rect(bm, RADIUS_SM, pal.accent_fill);
                    draw_icon(dl, Icon::Check, bm.inset(2.0), pal.accent_text, 2.0, false);
                } else {
                    dl.rect_border(bm, RADIUS_SM, pal.surface_raised, pal.separator, 1.0);
                }
                let lx = x0 + box_side + 10.0;
                text.draw(
                    dl,
                    block,
                    mi.x(lx, block.width),
                    y + (rh - block.height) * 0.5,
                    ink,
                );
                let row = mi.r(Rect::new(x0, y, box_side + 10.0 + block.width, *rh));
                if v.focus == *part {
                    focus_ring(dl, pal, row.inset(-3.0), RADIUS_SM + 3.0);
                }
                drawn.hits.push((row, Hit::Part(*part)));
            }
            Row::Button(part, _) => {
                let bw = (block.width + 28.0).max(76.0);
                let rm = mi.r(Rect::new(x0, y, bw, bh));
                dl.rect_border(rm, RADIUS_SM + 2.0, pal.surface_raised, pal.separator, 1.0);
                if v.focus == *part {
                    focus_ring(dl, pal, rm.inset(-4.0), RADIUS_SM + 6.0);
                }
                text.draw(
                    dl,
                    block,
                    rm.x + (rm.w - block.width) * 0.5,
                    rm.y + (bh - block.height) * 0.5,
                    ink,
                );
                drawn.hits.push((rm, Hit::Part(*part)));
            }
            Row::Note(_, true) => {
                let bx = Rect::new(x0, y, inner, *rh);
                dl.rect(mi.r(bx), RADIUS_SM, pal.accent.with_alpha(0.10));
                let ib = mi.r(Rect::new(bx.x + 6.0, bx.y + 6.0, 16.0, 16.0));
                draw_icon(dl, Icon::Info, ib, pal.accent, 1.6, rtl);
                text.draw(
                    dl,
                    block,
                    mi.x(bx.x + icon_w, block.width),
                    bx.y + 6.0,
                    pal.text,
                );
            }
            Row::Note(_, false) => {
                text.draw(dl, block, mi.x(x0, block.width), y, pal.text_muted);
            }
        }
        y += rh + gap;
    }
    if let Some(b) = &status {
        text.draw(dl, b, mi.x(x0, b.width), y, pal.text_muted);
    }
    dl.pop_clip();

    // The buttons, the default (what Enter does) at the end edge.
    let by = sheet.bottom() - pad - bh;
    let mut x = sheet.right() - pad;
    for (part, s, default) in [(Part::Ok, &v.ok, true), (Part::Cancel, &v.cancel, false)] {
        let lb = text.layout(s, button_st, None, Align::Start);
        let bw = (lb.width + 28.0).max(84.0);
        x -= bw;
        let rm = mi.r(Rect::new(x, by, bw, bh));
        x -= 8.0;
        let enabled = !default || (v.ok_enabled && !busy);
        if default {
            let fill = if enabled {
                pal.accent_fill
            } else {
                pal.accent_fill.with_alpha(0.45)
            };
            dl.rect(rm, RADIUS_SM + 2.0, fill);
        } else {
            dl.rect_border(rm, RADIUS_SM + 2.0, pal.surface_raised, pal.separator, 1.0);
        }
        if v.focus == part {
            focus_ring(dl, pal, rm.inset(-4.0), RADIUS_SM + 6.0);
        }
        let ink = match (default, enabled) {
            (true, true) => pal.accent_text,
            (true, false) => pal.accent_text.with_alpha(0.6),
            _ => pal.text,
        };
        text.draw(
            dl,
            &lb,
            rm.x + (rm.w - lb.width) * 0.5,
            rm.y + (bh - lb.height) * 0.5,
            ink,
        );
        drawn.hits.push((rm, Hit::Part(part)));
    }
    drawn
}

fn id_of(p: Part) -> accesskit::NodeId {
    accesskit::NodeId(A11Y_BASE + 0x10 + p.index())
}

/// The dialog's nodes: a modal dialog with its fields, choices (spin buttons), checkboxes,
/// notes and buttons.
pub(in crate::app) fn a11y(tree: &mut accesskit::TreeUpdate, v: &FormView, d: &Drawn, scale: f64) {
    use accesskit::{Node, NodeId, Role};
    let bounds = |r: Rect| accesskit::Rect {
        x0: f64::from(r.x) * scale,
        y0: f64::from(r.y) * scale,
        x1: f64::from(r.x + r.w) * scale,
        y1: f64::from(r.y + r.h) * scale,
    };
    let mut dialog = Node::new(Role::Dialog);
    dialog.set_modal();
    dialog.set_label(v.heading.clone());
    dialog.set_description(v.subtitle.clone());
    dialog.set_bounds(bounds(d.sheet));
    let mut ids = Vec::new();
    let mut nodes = Vec::new();
    let busy = v.busy.is_some();
    let mut notes = 0u64;
    for row in &v.rows {
        let (id, mut n) = match row {
            Row::Field(part, f) => {
                let mut n = Node::new(Role::TextInput);
                n.set_label(f.label.clone());
                let run_id = NodeId(A11Y_BASE + 0x80 + part.index());
                let chars = f.text[..f.caret.min(f.text.len())].chars().count();
                let run =
                    specular_widgets::text_run_sel(&mut n, run_id, &f.text, Some((chars, chars)));
                n.set_children(vec![run_id]);
                nodes.push((run_id, run));
                n.add_action(accesskit::Action::SetValue);
                (id_of(*part), n)
            }
            Row::Choice(part, label, value) => {
                let mut n = Node::new(Role::SpinButton);
                n.set_label(label.clone());
                n.set_value(value.clone());
                if !busy {
                    n.add_action(accesskit::Action::Increment);
                    n.add_action(accesskit::Action::Decrement);
                }
                (id_of(*part), n)
            }
            Row::Check(part, label, on) => {
                let mut n = Node::new(Role::CheckBox);
                n.set_label(label.clone());
                n.set_toggled(if *on {
                    accesskit::Toggled::True
                } else {
                    accesskit::Toggled::False
                });
                n.add_action(accesskit::Action::Click);
                (id_of(*part), n)
            }
            Row::Button(part, label) => {
                let mut n = Node::new(Role::Button);
                n.set_label(label.clone());
                n.add_action(accesskit::Action::Click);
                (id_of(*part), n)
            }
            Row::Note(t, _) => {
                let mut n = Node::new(Role::Label);
                n.set_value(t.clone());
                notes += 1;
                ids.push(NodeId(A11Y_BASE + 0x40 + notes));
                nodes.push((NodeId(A11Y_BASE + 0x40 + notes), n));
                continue;
            }
        };
        if let Some(part) = row.part() {
            if let Some(r) = d.rect_of(part) {
                n.set_bounds(bounds(r));
            }
            n.add_action(accesskit::Action::Focus);
        }
        ids.push(id);
        nodes.push((id, n));
    }
    for (part, label) in [(Part::Cancel, &v.cancel), (Part::Ok, &v.ok)] {
        let mut n = Node::new(Role::Button);
        n.set_label(label.clone());
        if part == Part::Ok && (busy || !v.ok_enabled) {
            n.set_disabled();
        } else {
            n.add_action(accesskit::Action::Click);
        }
        n.add_action(accesskit::Action::Focus);
        if let Some(r) = d.rect_of(part) {
            n.set_bounds(bounds(r));
        }
        ids.push(id_of(part));
        nodes.push((id_of(part), n));
    }
    if let Some(b) = &v.busy {
        let id = NodeId(A11Y_BASE + 0x3f);
        let mut n = Node::new(Role::Status);
        n.set_live(accesskit::Live::Polite);
        n.set_label(b.clone());
        ids.push(id);
        nodes.push((id, n));
    }
    dialog.set_children(ids);
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        root.push_child(NodeId(A11Y_BASE));
    }
    tree.nodes.push((NodeId(A11Y_BASE), dialog));
    tree.nodes.extend(nodes);
    tree.focus = id_of(v.focus);
}

/// The part a screen reader's request is for.
pub(in crate::app) fn a11y_target(id: u64) -> Option<Part> {
    let k = id.checked_sub(A11Y_BASE)?;
    let i = match k {
        0x80..0x90 => k - 0x80,
        0x10..0x20 => k - 0x10,
        _ => return None,
    };
    Part::ALL.get(i as usize).copied()
}
