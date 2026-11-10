//! The Open address dialog as data, its drawing and its accessibility tree. Nothing here
//! holds app state, so the screenshot tests draw it without an app.
//!
//! A small sheet: the title, what can be opened, the address field, a line saying why the
//! address can't be opened (or that the network is off), the recent addresses (host and
//! path), and Cancel and Open at the end edge. Right to left mirrors it; addresses stay left
//! to right inside their boxes.

use std::ops::Range;
use std::sync::Arc;

use specular_gfx::{Align, DrawList, Rect, Text, TextBlock, TextStyle};
use specular_theme::metrics::{FONT_LABEL, FONT_SMALL, FONT_TITLE, RADIUS_LG, RADIUS_SM};
use specular_theme::{Color, Palette};
use specular_widgets::accesskit;
use specular_widgets::line_edit::{compose, draw_preedit_underline, field_x0, ime_area};

pub(in crate::app) const A11Y_BASE: u64 = 0x6A00_0000_0000_0000;

/// What has the keyboard, in focus order; also what a click or a screen reader hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Part {
    Field,
    /// A recent address, by its place in the list.
    Recent(usize),
    Cancel,
    Open,
}

/// Everything the dialog draws.
#[derive(Clone, Debug, Default)]
pub(in crate::app) struct AddressView {
    pub title: String,
    pub intro: String,
    pub label: String,
    pub text: String,
    pub caret: usize,
    pub selection: Range<usize>,
    /// An input method's composition at the caret (empty: none).
    pub preedit: String,
    /// Why the address can't be opened (after Open), or that the network is off.
    pub error: Option<String>,
    pub recent_label: String,
    /// The recent addresses as shown: host and path.
    pub recent: Vec<String>,
    pub cancel: String,
    pub open: String,
    pub focus: Option<Part>,
}

impl AddressView {
    /// The parts in focus order.
    pub(in crate::app) fn focusables(&self) -> Vec<Part> {
        let mut v = vec![Part::Field];
        v.extend((0..self.recent.len()).map(Part::Recent));
        v.extend([Part::Cancel, Part::Open]);
        v
    }
}

/// Where things landed.
#[derive(Clone, Debug, Default)]
pub(in crate::app) struct Drawn {
    pub sheet: Rect,
    pub hits: Vec<(Rect, Part)>,
    /// The field's laid-out text and the x it is drawn at (a click places the caret).
    pub field_text: Option<(Arc<TextBlock>, f32)>,
    /// Where the input method's candidate window opens.
    pub ime: Option<Rect>,
}

/// Draws the dialog over a scrim in a window of `size` logical px.
pub(in crate::app) fn draw(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    v: &AddressView,
    size: (f32, f32),
    rtl: bool,
    ts: f32,
) -> Drawn {
    let (ww, wh) = size;
    dl.rect(Rect::new(0.0, 0.0, ww, wh), 0.0, pal.scrim);
    let pad = 20.0;
    let w = (ww - 32.0)
        .clamp(260.0, 520.0 + 140.0 * (ts - 1.0).max(0.0))
        .min(ww);
    let inner = w - 2.0 * pad;
    let m = |r: Rect| if rtl { r.mirror_x(ww) } else { r };
    let sx = |x: f32, bw: f32| if rtl { ww - x - bw } else { x };

    let title_st = TextStyle::new(FONT_TITLE * ts).weight(650.0);
    let label_st = TextStyle::new(FONT_LABEL * ts);
    let small_st = TextStyle::new(FONT_SMALL * ts);
    let button_st = TextStyle::new(FONT_LABEL * ts).weight(550.0);

    let title = text.layout(&v.title, title_st, Some(inner), Align::Start);
    let intro = text.layout(&v.intro, small_st, Some(inner), Align::Start);
    let label = text.layout(&v.label, small_st, Some(inner), Align::Start);
    let error = v
        .error
        .as_ref()
        .map(|s| text.layout(s, small_st, Some(inner), Align::Start));
    let recent_label = text.layout(&v.recent_label, small_st, Some(inner), Align::Start);
    let field_h = (26.0 * ts).max(30.0);
    let row_h = (22.0 * ts).max(26.0);
    let bh = (24.0 * ts).max(32.0);

    let mut h = pad + title.height + 6.0 + intro.height + 14.0;
    h += label.height + 4.0 + field_h;
    if let Some(e) = &error {
        h += 8.0 + e.height;
    }
    if !v.recent.is_empty() {
        h += 16.0 + recent_label.height + 4.0 + row_h * v.recent.len() as f32;
    }
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
    text.draw(dl, &title, sx(x0, title.width), y, pal.text);
    y += title.height + 6.0;
    text.draw(dl, &intro, sx(x0, intro.width), y, pal.text_muted);
    y += intro.height + 14.0;

    // The field.
    let top = y;
    text.draw(dl, &label, sx(x0, label.width), y, pal.text_muted);
    y += label.height + 4.0;
    let focused = v.focus == Some(Part::Field);
    let fr = m(Rect::new(x0, y, inner, field_h));
    dl.rect_border(
        fr,
        RADIUS_SM,
        pal.surface_raised,
        if focused {
            pal.focus_ring
        } else if v.error.is_some() {
            pal.danger
        } else {
            pal.separator
        },
        if focused { 2.0 } else { 1.0 },
    );
    let room = fr.inset_xy(10.0, 0.0);
    let pre = if focused { v.preedit.as_str() } else { "" };
    let (shown, composing) = compose(&v.text, v.caret, pre);
    let block = text.layout(&shown, label_st, None, Align::Start);
    let cx = block.caret_x(composing.end);
    let bx = field_x0(&block, cx, room, rtl || block.rtl);
    let ty = fr.y + (fr.h - block.height) * 0.5;
    dl.push_clip(room.inset_xy(-2.0, 0.0));
    if focused && composing.is_empty() {
        for (a, b) in block.range_spans(v.selection.clone()) {
            dl.rect(
                Rect::new(bx + a, ty, b - a, block.height),
                0.0,
                pal.selection,
            );
        }
    }
    text.draw(dl, &block, bx, ty, pal.text);
    if focused {
        let x = if shown.is_empty() && rtl {
            room.right()
        } else {
            bx + cx
        };
        let caret = Rect::new((x - 0.75).round(), ty, 1.5, block.height);
        dl.rect(caret, 0.0, pal.text);
        drawn.ime = Some(if composing.is_empty() {
            caret
        } else {
            draw_preedit_underline(
                dl,
                &block,
                bx,
                composing.clone(),
                ty + block.height,
                pal.text_muted,
            );
            ime_area(&block, bx, composing, ty, block.height, room)
        });
    }
    dl.pop_clip();
    drawn.field_text = Some((block, bx));
    drawn
        .hits
        .push((m(Rect::new(x0, top, inner, y + field_h - top)), Part::Field));
    y += field_h;

    if let Some(e) = &error {
        y += 8.0;
        text.draw(dl, e, sx(x0, e.width), y, pal.danger);
        y += e.height;
    }

    // The recent addresses.
    if !v.recent.is_empty() {
        y += 16.0;
        text.draw(
            dl,
            &recent_label,
            sx(x0, recent_label.width),
            y,
            pal.text_muted,
        );
        y += recent_label.height + 4.0;
        for (i, s) in v.recent.iter().enumerate() {
            let r = m(Rect::new(x0, y, inner, row_h));
            if v.focus == Some(Part::Recent(i)) {
                dl.rect(r, RADIUS_SM, pal.accent_soft);
                dl.rect_border(r, RADIUS_SM, Color::TRANSPARENT, pal.focus_ring, 2.0);
            }
            let b = text.layout(s, label_st, None, Align::Start);
            let tr = r.inset_xy(8.0, 0.0);
            dl.push_clip(tr);
            // The end of a long address (its file) stays in sight.
            let bx = if b.width <= tr.w {
                if rtl { tr.right() - b.width } else { tr.x }
            } else {
                tr.right() - b.width
            };
            text.draw(dl, &b, bx, r.y + (row_h - b.height) * 0.5, pal.text);
            dl.pop_clip();
            drawn.hits.push((r, Part::Recent(i)));
            y += row_h;
        }
    }
    let _ = y;

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
        let rm = m(Rect::new(x, by, bw, bh));
        x -= 8.0;
        if default {
            dl.rect(rm, RADIUS_SM + 2.0, pal.accent_fill);
        } else {
            dl.rect_border(rm, RADIUS_SM + 2.0, pal.surface_raised, pal.separator, 1.0);
        }
        if v.focus == Some(part) {
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

fn node_id(p: Part) -> accesskit::NodeId {
    accesskit::NodeId(match p {
        Part::Field => A11Y_BASE + 0x10,
        Part::Cancel => A11Y_BASE + 0x11,
        Part::Open => A11Y_BASE + 0x12,
        Part::Recent(i) => A11Y_BASE + 0x100 + i as u64,
    })
}

/// The dialog's nodes: a modal dialog with the address field, the error as an alert, the
/// recent addresses as a list and the buttons.
pub(in crate::app) fn a11y(
    tree: &mut accesskit::TreeUpdate,
    v: &AddressView,
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
    let at = |p: Part| d.hits.iter().find(|(_, q)| *q == p).map(|x| x.0);
    let mut dialog = Node::new(Role::Dialog);
    dialog.set_modal();
    dialog.set_label(v.title.clone());
    dialog.set_description(v.intro.clone());
    dialog.set_bounds(bounds(d.sheet));
    let mut ids = Vec::new();
    let mut nodes = Vec::new();
    // The field.
    let mut n = Node::new(Role::TextInput);
    n.set_label(v.label.clone());
    let run_id = NodeId(A11Y_BASE + 0x80);
    let chars = |b: usize| v.text[..b.min(v.text.len())].chars().count();
    let sel = (chars(v.selection.start), chars(v.selection.end));
    let sel = if v.caret == v.selection.start {
        (sel.1, sel.0)
    } else {
        sel
    };
    let run = specular_widgets::text_run_sel(&mut n, run_id, &v.text, Some(sel));
    n.set_children(vec![run_id]);
    nodes.push((run_id, run));
    if let Some(e) = &v.error {
        n.set_invalid(accesskit::Invalid::True);
        n.set_description(e.clone());
    }
    if let Some(r) = at(Part::Field) {
        n.set_bounds(bounds(r));
    }
    n.add_action(accesskit::Action::Focus);
    n.add_action(accesskit::Action::SetValue);
    ids.push(node_id(Part::Field));
    nodes.push((node_id(Part::Field), n));
    if let Some(e) = &v.error {
        let id = NodeId(A11Y_BASE + 0x40);
        let mut n = Node::new(Role::Alert);
        n.set_live(accesskit::Live::Assertive);
        n.set_label(e.clone());
        ids.push(id);
        nodes.push((id, n));
    }
    if !v.recent.is_empty() {
        let list_id = NodeId(A11Y_BASE + 0x50);
        let mut list = Node::new(Role::List);
        list.set_label(v.recent_label.clone());
        let mut rows = Vec::new();
        for (i, s) in v.recent.iter().enumerate() {
            let p = Part::Recent(i);
            let mut n = Node::new(Role::ListItem);
            n.set_label(s.clone());
            if let Some(r) = at(p) {
                n.set_bounds(bounds(r));
            }
            n.add_action(accesskit::Action::Click);
            n.add_action(accesskit::Action::Focus);
            rows.push(node_id(p));
            nodes.push((node_id(p), n));
        }
        list.set_children(rows);
        ids.push(list_id);
        nodes.push((list_id, list));
    }
    for (p, s) in [(Part::Cancel, &v.cancel), (Part::Open, &v.open)] {
        let mut n = Node::new(Role::Button);
        n.set_label(s.clone());
        if let Some(r) = at(p) {
            n.set_bounds(bounds(r));
        }
        n.add_action(accesskit::Action::Click);
        n.add_action(accesskit::Action::Focus);
        ids.push(node_id(p));
        nodes.push((node_id(p), n));
    }
    dialog.set_children(ids);
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        root.push_child(NodeId(A11Y_BASE));
    }
    tree.nodes.push((NodeId(A11Y_BASE), dialog));
    tree.nodes.extend(nodes);
    tree.focus = node_id(v.focus.unwrap_or(Part::Field));
}

/// The part a screen reader's request is for.
pub(in crate::app) fn a11y_target(v: &AddressView, node: u64) -> Option<Part> {
    v.focusables().into_iter().find(|p| node_id(*p).0 == node)
}
