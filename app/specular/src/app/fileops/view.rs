//! The file-operation sheets as data, their drawing and their accessibility tree: one layout
//! serves rename (a name field), batch rename (a pattern field over a live preview), copy and
//! move to a folder (recent folders and what to do with names that are taken), confirming a
//! replace, and the preview of an settings import. Nothing here holds app state, so the
//! screenshot tests draw it without an app.
//!
//! Top to bottom: the title and a muted subtitle, the field with its label, a red line saying
//! why the text can't be used, muted notes, a list (rows of "start … end", a problem in red
//! in place of the end), "and N more", a row of options, and the buttons at the end edge, the
//! default first. Right to left mirrors it.

use std::sync::Arc;

use specular_gfx::{Align, DrawList, Rect, Text, TextBlock, TextStyle};
use specular_theme::metrics::{FONT_LABEL, FONT_SMALL, FONT_TITLE, RADIUS_LG, RADIUS_SM};
use specular_theme::{Color, Palette};
use specular_widgets::accesskit;

pub(in crate::app) const A11Y_BASE: u64 = 0x5800_0000_0000_0000;

/// Rows shown at once; the list scrolls past that.
pub(in crate::app) const MAX_ROWS: usize = 9;

/// What has the keyboard or was hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Part {
    Field,
    /// A row of the list, by its index in [`SheetView::rows`].
    Row(usize),
    /// An option of the choice row.
    Choice(usize),
    /// A button, by its index in [`SheetView::buttons`].
    Button(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct FieldView {
    pub label: String,
    pub text: String,
    /// Byte offsets into `text`.
    pub caret: usize,
    pub selection: (usize, usize),
    /// What an input method is composing (IME preedit), shown at the caret.
    pub preedit: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct RowView {
    pub start: String,
    /// What it becomes or where it is (muted when `quiet`).
    pub end: Option<String>,
    /// Why this row can't be done, shown in place of `end`.
    pub problem: Option<String>,
    pub quiet: bool,
    /// The row can be chosen (a folder); chosen rows are highlighted.
    pub choosable: bool,
    pub chosen: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct ChoiceView {
    pub label: String,
    pub options: Vec<String>,
    pub selected: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct ButtonView {
    pub label: String,
    pub enabled: bool,
    /// What Enter does; drawn filled.
    pub default: bool,
}

/// Everything a sheet draws.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct SheetView {
    pub title: String,
    pub subtitle: Option<String>,
    pub field: Option<FieldView>,
    pub error: Option<String>,
    pub notes: Vec<String>,
    pub list_label: Option<String>,
    /// The rows in view (at most [`MAX_ROWS`]).
    pub rows: Vec<RowView>,
    /// Choosable rows are ticked each on its own (check boxes, Develop's modules); otherwise
    /// one of them is chosen (radio buttons, a folder).
    pub multi: bool,
    pub more: Option<String>,
    pub choice: Option<ChoiceView>,
    /// From the end edge: the default first.
    pub buttons: Vec<ButtonView>,
    pub focus: Part,
}

impl SheetView {
    /// The parts Tab visits, in order.
    pub fn focusables(&self) -> Vec<Part> {
        let mut out = Vec::new();
        if self.field.is_some() {
            out.push(Part::Field);
        }
        out.extend(
            self.rows
                .iter()
                .enumerate()
                .filter(|(_, r)| r.choosable)
                .map(|(i, _)| Part::Row(i)),
        );
        if let Some(c) = &self.choice {
            out.push(Part::Choice(c.selected));
        }
        // Reading order: the buttons from the start edge, the default last.
        out.extend((0..self.buttons.len()).rev().map(Part::Button));
        out
    }
}

/// Where things landed.
#[derive(Clone, Debug, Default)]
pub(in crate::app) struct Drawn {
    pub sheet: Rect,
    pub hits: Vec<(Rect, Part)>,
    /// The field's laid-out text and the x it is drawn at (a click places the caret); not
    /// while an input method composes.
    pub field_text: Option<(Arc<TextBlock>, f32)>,
    /// Where the input method's window opens: the composition and the caret.
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

/// `s` cut with "…" in the middle to fit `max_w` (file names keep their ends); bidi
/// isolates in it stay balanced (see [`specular_i18n::ellipsis_cut`]).
fn ellipsize(text: &mut Text, s: &str, style: TextStyle, max_w: f32) -> String {
    specular_i18n::ellipsize_to_fit(s, true, |t| text.measure(t, style) <= max_w)
}

/// Draws the sheet over a scrim in a window of `size` logical px.
pub(in crate::app) fn draw(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    v: &SheetView,
    size: (f32, f32),
    rtl: bool,
    ts: f32,
) -> Drawn {
    let (ww, wh) = size;
    dl.rect(Rect::new(0.0, 0.0, ww, wh), 0.0, pal.scrim);
    let pad = 20.0;
    let wide = !v.rows.is_empty();
    let base = if wide { 560.0 } else { 440.0 };
    let w = (ww - 32.0)
        .clamp(260.0, base + 140.0 * (ts - 1.0).max(0.0))
        .min(ww);
    let inner = w - 2.0 * pad;
    let mi = Mirror { ww, rtl };

    let title_st = TextStyle::new(FONT_TITLE * ts).weight(650.0);
    let label_st = TextStyle::new(FONT_LABEL * ts);
    let small_st = TextStyle::new(FONT_SMALL * ts);
    let button_st = TextStyle::new(FONT_LABEL * ts).weight(550.0);

    let title = text.layout(&v.title, title_st, Some(inner), Align::Start);
    let subtitle = v
        .subtitle
        .as_ref()
        .map(|s| text.layout(s, small_st, Some(inner), Align::Start));
    let field_label = v
        .field
        .as_ref()
        .map(|f| text.layout(&f.label, small_st, Some(inner), Align::Start));
    let error = v
        .error
        .as_ref()
        .map(|s| text.layout(s, label_st, Some(inner), Align::Start));
    let notes: Vec<_> = v
        .notes
        .iter()
        .map(|s| text.layout(s, small_st, Some(inner), Align::Start))
        .collect();
    let list_label = v
        .list_label
        .as_ref()
        .map(|s| text.layout(s, small_st, Some(inner), Align::Start));
    let more = v
        .more
        .as_ref()
        .map(|s| text.layout(s, small_st, Some(inner), Align::Start));
    let choice_label = v
        .choice
        .as_ref()
        .map(|c| text.layout(&c.label, small_st, Some(inner), Align::Start));
    let field_h = (26.0 * ts).max(30.0);
    let row_h = (22.0 * ts).max(26.0);
    let seg_h = (24.0 * ts).max(30.0);
    let bh = (24.0 * ts).max(32.0);

    let mut h = pad + title.height;
    if let Some(s) = &subtitle {
        h += 6.0 + s.height;
    }
    if let Some(l) = &field_label {
        h += 14.0 + l.height + 4.0 + field_h;
    }
    if let Some(e) = &error {
        h += 8.0 + e.height;
    }
    for n in &notes {
        h += 6.0 + n.height;
    }
    if !v.rows.is_empty() || list_label.is_some() {
        h += 14.0;
        if let Some(l) = &list_label {
            h += l.height + 4.0;
        }
        h += row_h * v.rows.len() as f32;
    }
    if let Some(m) = &more {
        h += 4.0 + m.height;
    }
    if let Some(l) = &choice_label {
        h += 14.0 + l.height + 4.0 + seg_h;
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
    text.draw(dl, &title, mi.x(x0, title.width), y, pal.text);
    y += title.height;
    if let Some(s) = &subtitle {
        y += 6.0;
        text.draw(dl, s, mi.x(x0, s.width), y, pal.text_muted);
        y += s.height;
    }

    // The field.
    if let (Some(f), Some(l)) = (&v.field, &field_label) {
        y += 14.0;
        let top = y;
        text.draw(dl, l, mi.x(x0, l.width), y, pal.text_muted);
        y += l.height + 4.0;
        let focused = v.focus == Part::Field;
        let fr = mi.r(Rect::new(x0, y, inner, field_h));
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
        // A composition sits at the caret, underlined; the caret follows it.
        let pre = if focused { f.preedit.as_str() } else { "" };
        let (shown, composing) = specular_widgets::line_edit::compose(&f.text, f.caret, pre);
        let block = text.layout(&shown, label_st, None, Align::Start);
        let cx = block.caret_x(composing.end);
        let bx = specular_widgets::line_edit::field_x0(&block, cx, room, rtl || block.rtl);
        let ty = fr.y + (fr.h - block.height) * 0.5;
        dl.push_clip(room.inset_xy(-2.0, 0.0));
        if focused && pre.is_empty() {
            for (a, b) in block.range_spans(f.selection.0..f.selection.1) {
                dl.rect(
                    Rect::new(bx + a, ty, b - a, block.height),
                    0.0,
                    pal.selection,
                );
            }
        }
        text.draw(dl, &block, bx, ty, pal.text);
        if !pre.is_empty() {
            specular_widgets::line_edit::draw_preedit_underline(
                dl,
                &block,
                bx,
                composing.clone(),
                ty + block.height,
                pal.text_muted,
            );
        }
        if focused {
            dl.rect(
                Rect::new((bx + cx - 0.75).round(), ty, 1.5, block.height),
                0.0,
                pal.text,
            );
            drawn.ime = Some(specular_widgets::line_edit::ime_area(
                &block,
                bx,
                composing,
                ty,
                block.height,
                room,
            ));
        }
        dl.pop_clip();
        if pre.is_empty() {
            drawn.field_text = Some((block, bx));
        }
        drawn.hits.push((
            mi.r(Rect::new(x0, top, inner, y + field_h - top)),
            Part::Field,
        ));
        y += field_h;
    }
    if let Some(e) = &error {
        y += 8.0;
        text.draw(dl, e, mi.x(x0, e.width), y, pal.danger);
        y += e.height;
    }
    for n in &notes {
        y += 6.0;
        text.draw(dl, n, mi.x(x0, n.width), y, pal.text_muted);
        y += n.height;
    }

    // The list.
    if !v.rows.is_empty() || list_label.is_some() {
        y += 14.0;
        if let Some(l) = &list_label {
            text.draw(dl, l, mi.x(x0, l.width), y, pal.text_muted);
            y += l.height + 4.0;
        }
        let list = Rect::new(x0, y, inner, row_h * v.rows.len() as f32);
        if !v.rows.is_empty() {
            dl.rect_border(
                mi.r(list.inset(-1.0)),
                RADIUS_SM,
                pal.surface_raised,
                pal.separator,
                1.0,
            );
        }
        let start_w = if v
            .rows
            .iter()
            .any(|r| r.end.is_some() || r.problem.is_some())
        {
            (inner * 0.44).floor()
        } else {
            inner - 16.0
        };
        let arrow = if rtl { "←" } else { "→" };
        let arrow_w = text.measure(arrow, small_st);
        for (i, r) in v.rows.iter().enumerate() {
            let rr = Rect::new(x0, y, inner, row_h);
            let rm = mi.r(rr);
            if r.chosen {
                dl.rect(rm.inset(1.0), RADIUS_SM, pal.selection);
            }
            if v.focus == Part::Row(i) {
                dl.rect_border(
                    rm.inset(1.0),
                    RADIUS_SM,
                    Color::TRANSPARENT,
                    pal.focus_ring,
                    2.0,
                );
            }
            let s = ellipsize(text, &r.start, label_st, start_w - 8.0);
            let sb = text.layout(&s, label_st, None, Align::Start);
            let sy = y + (row_h - sb.height) * 0.5;
            text.draw(dl, &sb, mi.x(x0 + 8.0, sb.width), sy, pal.text);
            let end = r.problem.as_ref().or(r.end.as_ref());
            if let Some(e) = end {
                let ax = x0 + start_w + 4.0;
                let ab = text.layout(arrow, small_st, None, Align::Start);
                text.draw(
                    dl,
                    &ab,
                    mi.x(ax, ab.width),
                    y + (row_h - ab.height) * 0.5,
                    pal.text_muted,
                );
                let ex = ax + arrow_w + 8.0;
                let ew = (x0 + inner - 8.0 - ex).max(0.0);
                let e = ellipsize(text, e, label_st, ew);
                let eb = text.layout(&e, label_st, None, Align::Start);
                let ink = if r.problem.is_some() {
                    pal.danger
                } else if r.quiet {
                    pal.text_muted
                } else {
                    pal.text
                };
                text.draw(
                    dl,
                    &eb,
                    mi.x(ex, eb.width),
                    y + (row_h - eb.height) * 0.5,
                    ink,
                );
            }
            if r.choosable {
                drawn.hits.push((rm, Part::Row(i)));
            }
            y += row_h;
        }
    }
    if let Some(m) = &more {
        y += 4.0;
        text.draw(dl, m, mi.x(x0, m.width), y, pal.text_muted);
        y += m.height;
    }

    // The options.
    if let (Some(c), Some(l)) = (&v.choice, &choice_label) {
        y += 14.0;
        text.draw(dl, l, mi.x(x0, l.width), y, pal.text_muted);
        y += l.height + 4.0;
        let n = c.options.len().max(1) as f32;
        let seg_w = inner / n;
        for (i, o) in c.options.iter().enumerate() {
            let r = Rect::new(x0 + seg_w * i as f32, y, seg_w, seg_h);
            let rm = mi.r(r);
            let on = i == c.selected;
            if on {
                dl.rect(rm.inset(1.0), RADIUS_SM, pal.accent_fill);
            } else {
                dl.rect_border(
                    rm.inset(1.0),
                    RADIUS_SM,
                    pal.surface_raised,
                    pal.separator,
                    1.0,
                );
            }
            if v.focus == Part::Choice(i) {
                dl.rect_border(
                    rm.inset(-2.0),
                    RADIUS_SM + 2.0,
                    Color::TRANSPARENT,
                    pal.focus_ring,
                    2.0,
                );
            }
            let s = ellipsize(text, o, label_st, seg_w - 12.0);
            let b = text.layout(&s, label_st, None, Align::Start);
            text.draw(
                dl,
                &b,
                rm.x + (rm.w - b.width) * 0.5,
                rm.y + (rm.h - b.height) * 0.5,
                if on { pal.accent_text } else { pal.text },
            );
            drawn.hits.push((rm, Part::Choice(i)));
        }
    }

    // The buttons, the default at the end edge.
    let by = sheet.bottom() - pad - bh;
    let mut x = sheet.right() - pad;
    for (i, b) in v.buttons.iter().enumerate() {
        let lb = text.layout(&b.label, button_st, None, Align::Start);
        let bw = (lb.width + 28.0).max(84.0);
        x -= bw;
        let rm = mi.r(Rect::new(x, by, bw, bh));
        x -= 8.0;
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
        if v.focus == Part::Button(i) {
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
        drawn.hits.push((rm, Part::Button(i)));
    }
    drawn
}

fn node_id(p: Part) -> accesskit::NodeId {
    accesskit::NodeId(
        A11Y_BASE
            + match p {
                Part::Field => 0x10,
                Part::Button(i) => 0x20 + i as u64,
                Part::Choice(i) => 0x40 + i as u64,
                Part::Row(i) => 0x100 + i as u64,
            },
    )
}

/// The part a screen reader's request is for.
pub(in crate::app) fn a11y_target(id: u64) -> Option<Part> {
    match id.checked_sub(A11Y_BASE)? {
        0x10 | 0x80 => Some(Part::Field),
        k @ 0x20..0x40 => Some(Part::Button((k - 0x20) as usize)),
        k @ 0x40..0x60 => Some(Part::Choice((k - 0x40) as usize)),
        k @ 0x100..0x200 => Some(Part::Row((k - 0x100) as usize)),
        _ => None,
    }
}

/// The sheet's nodes: a modal dialog with the field (invalid with the reason while the text
/// can't be used), the notes, the list, the options as radio buttons and the buttons.
pub(in crate::app) fn a11y(tree: &mut accesskit::TreeUpdate, v: &SheetView, d: &Drawn, scale: f64) {
    use accesskit::{Node, NodeId, Role};
    let bounds = |r: Rect| accesskit::Rect {
        x0: f64::from(r.x) * scale,
        y0: f64::from(r.y) * scale,
        x1: f64::from(r.x + r.w) * scale,
        y1: f64::from(r.y + r.h) * scale,
    };
    let hit = |p: Part| {
        d.hits
            .iter()
            .find(|(_, q)| *q == p)
            .map(|(r, _)| bounds(*r))
    };
    let mut dialog = Node::new(Role::Dialog);
    dialog.set_modal();
    dialog.set_label(v.title.clone());
    if let Some(s) = &v.subtitle {
        dialog.set_description(s.clone());
    }
    dialog.set_bounds(bounds(d.sheet));
    let mut ids = Vec::new();
    let mut nodes = Vec::new();
    if let Some(f) = &v.field {
        let mut n = Node::new(Role::TextInput);
        n.set_label(f.label.clone());
        let run_id = NodeId(A11Y_BASE + 0x80);
        let (a, b) = (
            f.text[..f.selection.0.min(f.text.len())].chars().count(),
            f.text[..f.selection.1.min(f.text.len())].chars().count(),
        );
        let run = specular_widgets::text_run_sel(&mut n, run_id, &f.text, Some((a, b)));
        n.set_children(vec![run_id]);
        nodes.push((run_id, run));
        if let Some(e) = &v.error {
            n.set_invalid(accesskit::Invalid::True);
            n.set_description(e.clone());
        }
        if let Some(r) = hit(Part::Field) {
            n.set_bounds(r);
        }
        n.add_action(accesskit::Action::SetValue);
        n.add_action(accesskit::Action::Focus);
        ids.push(node_id(Part::Field));
        nodes.push((node_id(Part::Field), n));
    }
    if let Some(e) = &v.error {
        let id = NodeId(A11Y_BASE + 0x60);
        let mut n = Node::new(Role::Alert);
        n.set_live(accesskit::Live::Polite);
        n.set_label(e.clone());
        ids.push(id);
        nodes.push((id, n));
    }
    for (i, s) in v.notes.iter().enumerate() {
        let id = NodeId(A11Y_BASE + 0x68 + i as u64);
        let mut n = Node::new(Role::Label);
        n.set_value(s.clone());
        ids.push(id);
        nodes.push((id, n));
    }
    if !v.rows.is_empty() {
        let list_id = NodeId(A11Y_BASE + 0x70);
        let mut list = Node::new(Role::List);
        if let Some(l) = &v.list_label {
            list.set_label(l.clone());
        }
        let mut kids = Vec::new();
        for (i, r) in v.rows.iter().enumerate() {
            let p = Part::Row(i);
            let mut n = Node::new(match (r.choosable, v.multi) {
                (true, true) => Role::CheckBox,
                (true, false) => Role::RadioButton,
                (false, _) => Role::ListItem,
            });
            let mut label = r.start.clone();
            if let Some(e) = r.problem.as_ref().or(r.end.as_ref()) {
                label = format!("{label} → {e}");
            }
            n.set_label(label);
            if r.problem.is_some() {
                n.set_invalid(accesskit::Invalid::True);
            }
            if r.choosable {
                n.set_toggled(if r.chosen {
                    accesskit::Toggled::True
                } else {
                    accesskit::Toggled::False
                });
                n.add_action(accesskit::Action::Click);
                n.add_action(accesskit::Action::Focus);
            }
            if let Some(b) = hit(p) {
                n.set_bounds(b);
            }
            kids.push(node_id(p));
            nodes.push((node_id(p), n));
        }
        list.set_children(kids);
        ids.push(list_id);
        nodes.push((list_id, list));
    }
    if let Some(m) = &v.more {
        let id = NodeId(A11Y_BASE + 0x78);
        let mut n = Node::new(Role::Label);
        n.set_value(m.clone());
        ids.push(id);
        nodes.push((id, n));
    }
    if let Some(c) = &v.choice {
        let group_id = NodeId(A11Y_BASE + 0x79);
        let mut g = Node::new(Role::RadioGroup);
        g.set_label(c.label.clone());
        let mut kids = Vec::new();
        for (i, o) in c.options.iter().enumerate() {
            let p = Part::Choice(i);
            let mut n = Node::new(Role::RadioButton);
            n.set_label(o.clone());
            n.set_toggled(if i == c.selected {
                accesskit::Toggled::True
            } else {
                accesskit::Toggled::False
            });
            n.add_action(accesskit::Action::Click);
            n.add_action(accesskit::Action::Focus);
            if let Some(b) = hit(p) {
                n.set_bounds(b);
            }
            kids.push(node_id(p));
            nodes.push((node_id(p), n));
        }
        g.set_children(kids);
        ids.push(group_id);
        nodes.push((group_id, g));
    }
    for (i, b) in v.buttons.iter().enumerate().rev() {
        let p = Part::Button(i);
        let mut n = Node::new(Role::Button);
        n.set_label(b.label.clone());
        // A disabled button is read as unavailable and offers no click.
        if b.enabled {
            n.add_action(accesskit::Action::Click);
        } else {
            n.set_disabled();
        }
        n.add_action(accesskit::Action::Focus);
        if let Some(r) = hit(p) {
            n.set_bounds(r);
        }
        ids.push(node_id(p));
        nodes.push((node_id(p), n));
    }
    dialog.set_children(ids);
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        root.push_child(NodeId(A11Y_BASE));
    }
    tree.nodes.push((NodeId(A11Y_BASE), dialog));
    tree.nodes.extend(nodes);
    tree.focus = node_id(v.focus);
}
