//! The thermal report dialog as data, its drawing and its accessibility tree. Nothing here
//! holds app state, so the screenshot tests draw it without an app.
//!
//! A sheet: the title, what the report holds, the report's title and notes fields, "Include
//! the visible photo" (when there is one), "Add the other thermal images in this folder",
//! "Also save a Word document", "Also save a KMZ of the places", and Cancel and Save PDF… at
//! the end edge. Right to left mirrors it. The inspection report (any photo) uses the same
//! sheet with its own words.

use std::sync::Arc;

use specular_gfx::{Align, DrawList, Icon, Rect, Text, TextBlock, TextStyle, draw_icon};
use specular_theme::metrics::{FONT_LABEL, FONT_SMALL, FONT_TITLE, RADIUS_LG, RADIUS_SM};
use specular_theme::{Color, Palette};
use specular_widgets::{accesskit, line_edit};

pub(in crate::app) const A11Y_BASE: u64 = 0x5A00_0000_0000_0000;

/// What has the keyboard, in focus order; also what a click or a screen reader hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Part {
    Title,
    Notes,
    Photo,
    Folder,
    /// Also save a Word document.
    Docx,
    /// Also save a KMZ of the photos' places.
    Kmz,
    Cancel,
    Save,
}

impl Part {
    pub const ORDER: [Part; 8] = [
        Part::Title,
        Part::Notes,
        Part::Photo,
        Part::Folder,
        Part::Docx,
        Part::Kmz,
        Part::Cancel,
        Part::Save,
    ];

    pub fn is_field(self) -> bool {
        matches!(self, Part::Title | Part::Notes)
    }
}

/// A text field as drawn: its label, text and caret (a byte offset).
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::app) struct FieldView {
    pub label: String,
    pub text: String,
    pub caret: usize,
    /// The selected bytes (empty: none).
    pub selection: std::ops::Range<usize>,
    /// An input method's composition at the caret (empty: none); only the focused field's.
    pub preedit: String,
}

/// Everything the dialog draws.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct ReportView {
    pub heading: String,
    /// What it holds: the image's name (and frame).
    pub subtitle: String,
    pub title: FieldView,
    pub notes: FieldView,
    /// "Include the visible photo", and whether it is on; `None` when there is no photo.
    pub photo: Option<(String, bool)>,
    pub folder: (String, bool),
    /// "Also save a Word document (DOCX)" and "Also save a KMZ of the places".
    pub docx: (String, bool),
    pub kmz: (String, bool),
    /// The report being made (Save is then not offered again).
    pub busy: Option<String>,
    pub cancel: String,
    pub save: String,
    pub focus: Part,
}

impl ReportView {
    /// The parts that can have the keyboard.
    pub fn parts(&self) -> Vec<Part> {
        Part::ORDER
            .into_iter()
            .filter(|p| *p != Part::Photo || self.photo.is_some())
            .collect()
    }
}

/// Where things landed.
#[derive(Clone, Debug, Default)]
pub(in crate::app) struct Drawn {
    pub sheet: Rect,
    pub hits: Vec<(Rect, Part)>,
    /// Each field's laid-out text and the x it is drawn at (a click places the caret).
    pub fields: Vec<(Part, Arc<TextBlock>, f32)>,
    /// Where the input method's candidate window opens: the focused field's caret or
    /// composition.
    pub ime: Option<Rect>,
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

/// Draws the dialog over a scrim in a window of `size` logical px.
pub(in crate::app) fn draw(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    v: &ReportView,
    size: (f32, f32),
    rtl: bool,
    ts: f32,
) -> Drawn {
    let (ww, wh) = size;
    dl.rect(Rect::new(0.0, 0.0, ww, wh), 0.0, pal.scrim);
    let pad = 20.0;
    let w = (ww - 32.0)
        .clamp(260.0, 460.0 + 140.0 * (ts - 1.0).max(0.0))
        .min(ww);
    let inner = w - 2.0 * pad;
    let mi = Mirror { ww, rtl };
    let title_st = TextStyle::new(FONT_TITLE * ts).weight(650.0);
    let label_st = TextStyle::new(FONT_LABEL * ts);
    let small_st = TextStyle::new(FONT_SMALL * ts);
    let button_st = TextStyle::new(FONT_LABEL * ts).weight(550.0);

    let heading = text.layout(&v.heading, title_st, Some(inner), Align::Start);
    let subtitle = text.layout(&v.subtitle, small_st, Some(inner), Align::Start);
    let box_side = (16.0 * ts).max(18.0);
    let field_h = (26.0 * ts).max(30.0);
    let bh = (24.0 * ts).max(32.0);
    let checks: Vec<(Part, Arc<TextBlock>, bool)> = [
        v.photo.as_ref().map(|(s, on)| (Part::Photo, s, *on)),
        Some((Part::Folder, &v.folder.0, v.folder.1)),
        Some((Part::Docx, &v.docx.0, v.docx.1)),
        Some((Part::Kmz, &v.kmz.0, v.kmz.1)),
    ]
    .into_iter()
    .flatten()
    .map(|(p, s, on)| {
        (
            p,
            text.layout(s, label_st, Some(inner - box_side - 10.0), Align::Start),
            on,
        )
    })
    .collect();
    let labels: Vec<Arc<TextBlock>> = [&v.title, &v.notes]
        .iter()
        .map(|f| text.layout(&f.label, small_st, Some(inner), Align::Start))
        .collect();
    let busy = v
        .busy
        .as_ref()
        .map(|s| text.layout(s, label_st, Some(inner), Align::Start));

    let mut h = pad + heading.height + 6.0 + subtitle.height + 14.0;
    for l in &labels {
        h += l.height + 4.0 + field_h + 12.0;
    }
    for (_, b, _) in &checks {
        h += b.height.max(box_side) + 10.0;
    }
    if let Some(b) = &busy {
        h += b.height + 10.0;
    }
    h += 14.0 + bh + pad;
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

    // The fields.
    for ((part, f), label) in [(Part::Title, &v.title), (Part::Notes, &v.notes)]
        .into_iter()
        .zip(&labels)
    {
        let top = y;
        text.draw(dl, label, mi.x(x0, label.width), y, pal.text_muted);
        y += label.height + 4.0;
        let fr = mi.r(Rect::new(x0, y, inner, field_h));
        let focused = v.focus == part;
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
        let block = text.layout(&shown, label_st, None, Align::Start);
        let cx = block.caret_x(composing.end);
        // Longer than the field: the caret stays in view (the shared field helper).
        let bx = line_edit::field_x0(&block, cx, room, rtl || block.rtl);
        let ty = fr.y + (fr.h - block.height) * 0.5;
        dl.push_clip(room.inset_xy(-2.0, 0.0));
        if focused && composing.is_empty() {
            for (a, b) in block.range_spans(f.selection.clone()) {
                dl.rect(
                    Rect::new(bx + a, ty, b - a, block.height),
                    0.0,
                    pal.selection,
                );
            }
        }
        text.draw(dl, &block, bx, ty, pal.text);
        if focused {
            let cx = if shown.is_empty() && rtl {
                room.right()
            } else {
                bx + cx
            };
            let caret = Rect::new((cx - 0.75).round(), ty, 1.5, block.height);
            dl.rect(caret, 0.0, pal.text);
            drawn.ime = Some(if composing.is_empty() {
                caret
            } else {
                line_edit::draw_preedit_underline(
                    dl,
                    &block,
                    bx,
                    composing.clone(),
                    ty + block.height,
                    pal.text_muted,
                );
                line_edit::ime_area(&block, bx, composing, ty, block.height, room)
            });
        }
        dl.pop_clip();
        drawn.fields.push((part, block, bx));
        drawn
            .hits
            .push((mi.r(Rect::new(x0, top, inner, y + field_h - top)), part));
        y += field_h + 12.0;
    }

    // The checkboxes.
    for (part, label, on) in &checks {
        let row_h = label.height.max(box_side);
        let bm = mi.r(Rect::new(
            x0,
            y + (row_h - box_side) * 0.5,
            box_side,
            box_side,
        ));
        if *on {
            dl.rect(bm, RADIUS_SM, pal.accent_fill);
            draw_icon(dl, Icon::Check, bm.inset(2.0), pal.accent_text, 2.0, false);
        } else {
            dl.rect_border(bm, RADIUS_SM, pal.surface_raised, pal.separator, 1.0);
        }
        let lx = x0 + box_side + 10.0;
        text.draw(
            dl,
            label,
            mi.x(lx, label.width),
            y + (row_h - label.height) * 0.5,
            pal.text,
        );
        let row = mi.r(Rect::new(x0, y, box_side + 10.0 + label.width, row_h));
        if v.focus == *part {
            dl.rect_border(
                row.inset(-3.0),
                RADIUS_SM + 3.0,
                Color::TRANSPARENT,
                pal.focus_ring,
                2.0,
            );
        }
        drawn.hits.push((row, *part));
        y += row_h + 10.0;
    }
    if let Some(b) = &busy {
        text.draw(dl, b, mi.x(x0, b.width), y, pal.text_muted);
    }

    // The buttons, Save (what Enter does) at the end edge.
    let by = sheet.bottom() - pad - bh;
    let mut x = sheet.right() - pad;
    for (part, s, default) in [
        (Part::Save, &v.save, true),
        (Part::Cancel, &v.cancel, false),
    ] {
        let lb = text.layout(s, button_st, None, Align::Start);
        let bw = (lb.width + 28.0).max(76.0);
        x -= bw;
        let rm = mi.r(Rect::new(x, by, bw, bh));
        x -= 8.0;
        let enabled = !(default && v.busy.is_some());
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
            dl.rect_border(
                rm.inset(-4.0),
                RADIUS_SM + 6.0,
                Color::TRANSPARENT,
                pal.focus_ring,
                2.0,
            );
        }
        let ink = if default { pal.accent_text } else { pal.text };
        text.draw(
            dl,
            &lb,
            rm.x + (rm.w - lb.width) * 0.5,
            rm.y + (bh - lb.height) * 0.5,
            ink,
        );
        drawn.hits.push((rm, part));
    }
    drawn
}

fn id_of(p: Part) -> accesskit::NodeId {
    accesskit::NodeId(A11Y_BASE + 0x10 + p as u64)
}

/// The dialog's nodes: a modal dialog with the two text fields, the checkboxes and the
/// buttons.
pub(in crate::app) fn a11y(
    tree: &mut accesskit::TreeUpdate,
    v: &ReportView,
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
    let mut dialog = Node::new(Role::Dialog);
    dialog.set_modal();
    dialog.set_label(v.heading.clone());
    dialog.set_description(v.subtitle.clone());
    dialog.set_bounds(bounds(d.sheet));
    let mut ids = Vec::new();
    let mut nodes = Vec::new();
    for part in v.parts() {
        let mut n = match part {
            Part::Title | Part::Notes => {
                let f = if part == Part::Title {
                    &v.title
                } else {
                    &v.notes
                };
                let mut n = Node::new(Role::TextInput);
                n.set_label(f.label.clone());
                let run_id = NodeId(A11Y_BASE + 0x80 + part as u64);
                let chars = f.text[..f.caret.min(f.text.len())].chars().count();
                let run =
                    specular_widgets::text_run_sel(&mut n, run_id, &f.text, Some((chars, chars)));
                n.set_children(vec![run_id]);
                nodes.push((run_id, run));
                n.add_action(accesskit::Action::SetValue);
                n
            }
            Part::Photo | Part::Folder | Part::Docx | Part::Kmz => {
                let (label, on) = match part {
                    Part::Photo => v.photo.clone().unwrap_or_default(),
                    Part::Docx => v.docx.clone(),
                    Part::Kmz => v.kmz.clone(),
                    _ => v.folder.clone(),
                };
                let mut n = Node::new(Role::CheckBox);
                n.set_label(label);
                n.set_toggled(if on {
                    accesskit::Toggled::True
                } else {
                    accesskit::Toggled::False
                });
                n.add_action(accesskit::Action::Click);
                n
            }
            Part::Cancel | Part::Save => {
                let mut n = Node::new(Role::Button);
                n.set_label(if part == Part::Save {
                    v.save.clone()
                } else {
                    v.cancel.clone()
                });
                if part == Part::Save && v.busy.is_some() {
                    n.set_disabled();
                }
                n.add_action(accesskit::Action::Click);
                n
            }
        };
        if let Some((r, _)) = d.hits.iter().find(|(_, p)| *p == part) {
            n.set_bounds(bounds(*r));
        }
        n.add_action(accesskit::Action::Focus);
        ids.push(id_of(part));
        nodes.push((id_of(part), n));
    }
    if let Some(b) = &v.busy {
        let id = NodeId(A11Y_BASE + 0x40);
        let mut n = Node::new(Role::Status);
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
    match id.checked_sub(A11Y_BASE)? {
        k @ 0x80..0x88 => Part::ORDER.get(k as usize - 0x80).copied(),
        k @ 0x10..0x18 => Part::ORDER.get(k as usize - 0x10).copied(),
        _ => None,
    }
}
