//! The raw volume dialog as data, its drawing and its accessibility tree. Nothing here holds
//! app state, so it is drawn and checked without an app.
//!
//! A small sheet like the password prompt: the title, the file's name, a `‹ value ›` stepper
//! per field (the guess, width, height, slices, sample type, byte order, header offset, pixel
//! size, slice spacing), a line saying how much of the file the layout takes (in red when it
//! does not fit), and Cancel and Open at the end edge. Right to left mirrors it.

use specular_gfx::{Align, DrawList, Icon, Rect, Text, TextStyle, draw_icon};
use specular_theme::metrics::{FONT_LABEL, FONT_SMALL, FONT_TITLE, RADIUS_LG, RADIUS_SM};
use specular_theme::{Color, Palette};
use specular_widgets::accesskit;

pub(in crate::app) const A11Y_BASE: u64 = 0x5D40_0000_0000_0000;

/// A stepper row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in crate::app) enum Row {
    Guess,
    Width,
    Height,
    Slices,
    Type,
    Order,
    Offset,
    Pixel,
    Spacing,
}

impl Row {
    pub const ALL: [Row; 9] = [
        Row::Guess,
        Row::Width,
        Row::Height,
        Row::Slices,
        Row::Type,
        Row::Order,
        Row::Offset,
        Row::Pixel,
        Row::Spacing,
    ];

    /// Rows whose value is a number, drawn with tabular figures so stepping doesn't jiggle.
    /// Words (sample type, byte order) keep proportional forms: Inter's tabular hyphen is
    /// as wide as a digit and spaces out "16-bit" and "Little-endian".
    pub fn figures(self) -> bool {
        !matches!(self, Row::Type | Row::Order)
    }

    /// Rows whose value can be typed.
    pub fn typed(self) -> bool {
        matches!(
            self,
            Row::Width | Row::Height | Row::Slices | Row::Offset | Row::Pixel | Row::Spacing
        )
    }
}

/// What has the keyboard, in focus order; also what a click or a screen reader hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Part {
    Row(Row),
    Cancel,
    Open,
}

impl Part {
    pub fn order() -> Vec<Part> {
        Row::ALL
            .iter()
            .map(|&r| Part::Row(r))
            .chain([Part::Cancel, Part::Open])
            .collect()
    }

    fn index(self) -> u64 {
        match self {
            Part::Row(r) => Row::ALL.iter().position(|x| *x == r).unwrap_or(0) as u64,
            Part::Cancel => 0x20,
            Part::Open => 0x21,
        }
    }
}

/// A click: a row, one of its arrows, or a button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Hit {
    Part(Part),
    Dec(Row),
    Inc(Row),
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct RowView {
    pub row: Row,
    pub label: String,
    pub value: String,
}

/// Everything the dialog draws.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct DialogView {
    pub title: String,
    /// The file's name and what is asked.
    pub subtitle: String,
    pub rows: Vec<RowView>,
    /// How much of the file the layout takes.
    pub status: String,
    /// The layout does not fit the file (Open is off).
    pub bad: bool,
    pub cancel: String,
    pub open: String,
    pub focus: Part,
}

/// Where things landed.
#[derive(Clone, Debug, Default)]
pub(in crate::app) struct Drawn {
    pub sheet: Rect,
    pub hits: Vec<(Rect, Hit)>,
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
    v: &DialogView,
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
    let value_st = TextStyle::new(FONT_LABEL * ts);
    let small_st = TextStyle::new(FONT_SMALL * ts);
    let button_st = TextStyle::new(FONT_LABEL * ts).weight(550.0);

    let title = text.layout(&v.title, title_st, Some(inner), Align::Start);
    let subtitle = text.layout(&v.subtitle, small_st, Some(inner), Align::Start);
    let status = text.layout(&v.status, small_st, Some(inner), Align::Start);
    let row_h = (24.0 * ts).max(30.0);
    let bh = (24.0 * ts).max(32.0);

    let mut h = pad + title.height + 6.0 + subtitle.height + 14.0;
    h += row_h * v.rows.len() as f32 + 12.0 + status.height;
    h += 20.0 + bh + pad;
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
    text.draw(dl, &title, mi.x(x0, title.width), y, pal.text);
    y += title.height + 6.0;
    text.draw(dl, &subtitle, mi.x(x0, subtitle.width), y, pal.text_muted);
    y += subtitle.height + 14.0;

    // The steppers: the label at the start edge, `‹ value ›` at the end edge.
    let arrow = (row_h - 6.0).min(26.0);
    let sw = (inner * 0.58).min(260.0 * ts.max(1.0));
    for rv in &v.rows {
        let r = Rect::new(x0, y, inner, row_h);
        if v.focus == Part::Row(rv.row) {
            dl.rect_border(
                mi.r(r.inset(-2.0)),
                RADIUS_SM,
                Color::TRANSPARENT,
                pal.focus_ring,
                2.0,
            );
        }
        let sx = r.right() - sw;
        let lab = text.layout(&rv.label, label_st, None, Align::Start);
        let lr = Rect::new(r.x + 4.0, r.y, (sx - 8.0 - r.x - 4.0).max(0.0), row_h);
        dl.push_clip(mi.r(lr));
        text.draw(
            dl,
            &lab,
            mi.x(lr.x, lab.width),
            r.y + (row_h - lab.height) * 0.5,
            pal.text_muted,
        );
        dl.pop_clip();
        let dec = Rect::new(sx, r.y + (row_h - arrow) * 0.5, arrow, arrow);
        let inc = Rect::new(r.right() - arrow, dec.y, arrow, arrow);
        for (b, icon, hit) in [
            (dec, Icon::ChevronLeft, Hit::Dec(rv.row)),
            (inc, Icon::ChevronRight, Hit::Inc(rv.row)),
        ] {
            let b = mi.r(b);
            dl.rect(b, RADIUS_SM, pal.surface_raised);
            draw_icon(dl, icon, b.inset(arrow * 0.18), pal.text, 1.6, mi.rtl);
            drawn.hits.push((b, hit));
        }
        let room = sw - 2.0 * arrow;
        let st = if rv.row.figures() {
            value_st.tabular()
        } else {
            value_st
        };
        let val = text.layout(&rv.value, st, None, Align::Start);
        let mid = sx + arrow + (room - val.width.min(room)) * 0.5;
        dl.push_clip(mi.r(Rect::new(sx + arrow, r.y, room, row_h)));
        text.draw(
            dl,
            &val,
            mi.x(mid, val.width),
            r.y + (row_h - val.height) * 0.5,
            pal.text,
        );
        dl.pop_clip();
        drawn.hits.push((mi.r(r), Hit::Part(Part::Row(rv.row))));
        y += row_h;
    }
    y += 12.0;
    let ink = if v.bad { pal.danger } else { pal.text_muted };
    text.draw(dl, &status, mi.x(x0, status.width), y, ink);

    // The buttons, Open (what Enter does) at the end edge.
    let by = sheet.bottom() - pad - bh;
    let mut x = sheet.right() - pad;
    for (part, s, default) in [
        (Part::Open, &v.open, true),
        (Part::Cancel, &v.cancel, false),
    ] {
        let lb = text.layout(s, button_st, None, Align::Start);
        let bw = (lb.width + 28.0).max(76.0);
        x -= bw;
        let rm = mi.r(Rect::new(x, by, bw, bh));
        x -= 8.0;
        let off = default && v.bad;
        if default && !off {
            dl.rect(rm, RADIUS_SM + 2.0, pal.accent_fill);
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
        let ink = if off {
            pal.text_muted
        } else if default {
            pal.accent_text
        } else {
            pal.text
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
    accesskit::NodeId(A11Y_BASE + 0x100 + p.index())
}

/// The dialog's nodes: a modal dialog with a spin button per field, the status (an alert
/// when the layout does not fit) and the buttons.
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
    let mut dialog = Node::new(Role::Dialog);
    dialog.set_modal();
    dialog.set_label(v.title.clone());
    dialog.set_description(v.subtitle.clone());
    dialog.set_bounds(bounds(d.sheet));
    let mut ids = Vec::new();
    let mut nodes = Vec::new();
    for part in Part::order() {
        let mut n = match part {
            Part::Row(row) => {
                let Some(rv) = v.rows.iter().find(|r| r.row == row) else {
                    continue;
                };
                let mut n = Node::new(Role::SpinButton);
                n.set_label(rv.label.clone());
                n.set_value(rv.value.clone());
                n.add_action(accesskit::Action::Increment);
                n.add_action(accesskit::Action::Decrement);
                if row.typed() {
                    n.add_action(accesskit::Action::SetValue);
                }
                n
            }
            Part::Cancel | Part::Open => {
                let mut n = Node::new(Role::Button);
                n.set_label(if part == Part::Open {
                    v.open.clone()
                } else {
                    v.cancel.clone()
                });
                // Disabled: read as unavailable, no click.
                if part == Part::Open && v.bad {
                    n.set_disabled();
                } else {
                    n.add_action(accesskit::Action::Click);
                }
                n
            }
        };
        if let Some((r, _)) = d.hits.iter().find(|(_, h)| *h == Hit::Part(part)) {
            n.set_bounds(bounds(*r));
        }
        n.add_action(accesskit::Action::Focus);
        ids.push(id_of(part));
        nodes.push((id_of(part), n));
    }
    let sid = NodeId(A11Y_BASE + 0x40);
    let mut s = Node::new(if v.bad { Role::Alert } else { Role::Label });
    if v.bad {
        s.set_live(accesskit::Live::Polite);
    }
    s.set_label(v.status.clone());
    ids.push(sid);
    nodes.push((sid, s));
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
    let k = id.checked_sub(A11Y_BASE + 0x100)?;
    Part::order().into_iter().find(|p| p.index() == k)
}
