//! The password prompt as data, its drawing and its accessibility tree. Nothing here holds
//! app state, so the screenshot tests draw it without an app.
//!
//! A small sheet: the title, the archive's name, the password field (dots, or the text when
//! "Show password" is on), the show toggle, the "Wrong password" line after a failed try, and
//! Cancel and Open at the end edge. Right to left mirrors it.

use std::sync::Arc;

use specular_gfx::{Align, DrawList, Icon, Rect, Text, TextBlock, TextStyle, draw_icon};
use specular_theme::metrics::{FONT_LABEL, FONT_SMALL, FONT_TITLE, RADIUS_LG, RADIUS_SM};
use specular_theme::{Color, Palette};
use specular_widgets::accesskit;
use zeroize::Zeroizing;

pub(in crate::app) const A11Y_BASE: u64 = 0x5700_0000_0000_0000;

/// What has the keyboard, in focus order; also what a click or a screen reader hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Part {
    Field,
    Show,
    Cancel,
    Open,
}

impl Part {
    pub const ORDER: [Part; 4] = [Part::Field, Part::Show, Part::Cancel, Part::Open];
}

/// Everything the prompt draws. Not `Debug`: `field` may hold the password.
#[derive(Clone)]
pub(in crate::app) struct PromptView {
    pub title: String,
    /// The archive's name.
    pub subtitle: String,
    pub label: String,
    /// What the field shows: a dot per character, or the password while it is shown.
    pub field: Zeroizing<String>,
    /// The caret, a byte offset into `field`.
    pub caret: usize,
    pub show: bool,
    pub show_label: String,
    /// After a failed try.
    pub error: Option<String>,
    pub cancel: String,
    pub open: String,
    pub focus: Part,
}

/// Where things landed.
#[derive(Clone, Debug, Default)]
pub(in crate::app) struct Drawn {
    pub sheet: Rect,
    pub hits: Vec<(Rect, Part)>,
    /// The field's laid-out text, the x it is drawn at (a click places the caret), and
    /// whether its carets count from the right ([`mirrored_dots`]).
    pub field_text: Option<(Arc<TextBlock>, f32, bool)>,
}

/// The field shows dots in a right-to-left interface: they are laid out from the right.
pub(super) fn mirrored_dots(v: &PromptView, rtl: bool) -> bool {
    rtl && !v.show
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

/// Dots for a hidden password: one per character.
pub(in crate::app) fn masked(s: &str) -> String {
    "•".repeat(s.chars().count())
}

/// Draws the prompt over a scrim in a window of `size` logical px.
pub(in crate::app) fn draw(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    v: &PromptView,
    size: (f32, f32),
    rtl: bool,
    ts: f32,
) -> Drawn {
    let (ww, wh) = size;
    dl.rect(Rect::new(0.0, 0.0, ww, wh), 0.0, pal.scrim);
    let pad = 20.0;
    let w = (ww - 32.0)
        .clamp(240.0, 420.0 + 120.0 * (ts - 1.0).max(0.0))
        .min(ww);
    let inner = w - 2.0 * pad;
    let mi = Mirror { ww, rtl };

    let title_st = TextStyle::new(FONT_TITLE * ts).weight(650.0);
    let label_st = TextStyle::new(FONT_LABEL * ts);
    let small_st = TextStyle::new(FONT_SMALL * ts);
    let button_st = TextStyle::new(FONT_LABEL * ts).weight(550.0);

    let title = text.layout(&v.title, title_st, Some(inner), Align::Start);
    let subtitle = text.layout(&v.subtitle, small_st, Some(inner), Align::Start);
    let label = text.layout(&v.label, small_st, Some(inner), Align::Start);
    let box_side = (16.0 * ts).max(18.0);
    let show = text.layout(
        &v.show_label,
        label_st,
        Some(inner - box_side - 10.0),
        Align::Start,
    );
    let error = v
        .error
        .as_ref()
        .map(|s| text.layout(s, label_st, Some(inner), Align::Start));
    let field_h = (26.0 * ts).max(30.0);
    let show_h = show.height.max(box_side);
    let bh = (24.0 * ts).max(32.0);

    let mut h = pad + title.height + 6.0 + subtitle.height + 14.0;
    h += label.height + 4.0 + field_h + 10.0 + show_h;
    if let Some(e) = &error {
        h += 10.0 + e.height;
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
    y += title.height + 6.0;
    text.draw(dl, &subtitle, mi.x(x0, subtitle.width), y, pal.text_muted);
    y += subtitle.height + 14.0;

    // The field.
    let top = y;
    text.draw(dl, &label, mi.x(x0, label.width), y, pal.text_muted);
    y += label.height + 4.0;
    let field = Rect::new(x0, y, inner, field_h);
    let focused = v.focus == Part::Field;
    let fr = mi.r(field);
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
    let block = text.layout(&v.field, label_st, None, Align::Start);
    // The dots have no direction of their own: a right-to-left interface lays them out
    // from the right, the caret counted from the right edge (the next dot goes on its left).
    let mirrored = mirrored_dots(v, rtl);
    let cx = if mirrored {
        block.width - block.caret_x(v.caret)
    } else {
        block.caret_x(v.caret)
    };
    let start_right = rtl || block.rtl;
    let bx = specular_widgets::line_edit::scroll_x0(
        block.width,
        cx,
        room,
        start_right,
        block.rtl || mirrored,
    );
    let ty = fr.y + (fr.h - block.height) * 0.5;
    dl.push_clip(room.inset_xy(-2.0, 0.0));
    text.draw(dl, &block, bx, ty, pal.text);
    if focused {
        let cx = if v.field.is_empty() && rtl {
            room.right()
        } else {
            bx + cx
        };
        dl.rect(
            Rect::new((cx - 0.75).round(), ty, 1.5, block.height),
            0.0,
            pal.text,
        );
    }
    dl.pop_clip();
    drawn.field_text = Some((block, bx, mirrored));
    drawn.hits.push((
        mi.r(Rect::new(x0, top, inner, y + field_h - top)),
        Part::Field,
    ));
    y += field_h + 10.0;

    // Show password.
    let bxr = Rect::new(x0, y + (show_h - box_side) * 0.5, box_side, box_side);
    let bm = mi.r(bxr);
    if v.show {
        dl.rect(bm, RADIUS_SM, pal.accent_fill);
        draw_icon(dl, Icon::Check, bm.inset(2.0), pal.accent_text, 2.0, false);
    } else {
        dl.rect_border(bm, RADIUS_SM, pal.surface_raised, pal.separator, 1.0);
    }
    let lx = x0 + box_side + 10.0;
    text.draw(
        dl,
        &show,
        mi.x(lx, show.width),
        y + (show_h - show.height) * 0.5,
        pal.text,
    );
    let row = mi.r(Rect::new(x0, y, box_side + 10.0 + show.width, show_h));
    if v.focus == Part::Show {
        dl.rect_border(
            row.inset(-3.0),
            RADIUS_SM + 3.0,
            Color::TRANSPARENT,
            pal.focus_ring,
            2.0,
        );
    }
    drawn.hits.push((row, Part::Show));
    y += show_h;

    if let Some(e) = &error {
        y += 10.0;
        text.draw(dl, e, mi.x(x0, e.width), y, pal.danger);
    }

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
        if default {
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

/// The prompt's nodes: a modal dialog with the password field (its value only as dots
/// while hidden), the toggle, the error as an alert, and the buttons.
pub(in crate::app) fn a11y(
    tree: &mut accesskit::TreeUpdate,
    v: &PromptView,
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
    let id_of = |p: Part| NodeId(A11Y_BASE + 0x10 + p as u64);
    let mut dialog = Node::new(Role::Dialog);
    dialog.set_modal();
    dialog.set_label(v.title.clone());
    dialog.set_description(v.subtitle.clone());
    dialog.set_bounds(bounds(d.sheet));
    let mut ids = Vec::new();
    let mut nodes = Vec::new();
    for part in Part::ORDER {
        let mut n = match part {
            Part::Field => {
                let mut n = Node::new(if v.show {
                    Role::TextInput
                } else {
                    Role::PasswordInput
                });
                n.set_label(v.label.clone());
                let run_id = NodeId(A11Y_BASE + 0x80);
                let chars = v.field[..v.caret.min(v.field.len())].chars().count();
                let run =
                    specular_widgets::text_run_sel(&mut n, run_id, &v.field, Some((chars, chars)));
                n.set_children(vec![run_id]);
                nodes.push((run_id, run));
                if let Some(e) = &v.error {
                    n.set_invalid(accesskit::Invalid::True);
                    n.set_description(e.clone());
                }
                n.add_action(accesskit::Action::SetValue);
                n
            }
            Part::Show => {
                let mut n = Node::new(Role::CheckBox);
                n.set_label(v.show_label.clone());
                n.set_toggled(if v.show {
                    accesskit::Toggled::True
                } else {
                    accesskit::Toggled::False
                });
                n.add_action(accesskit::Action::Click);
                n
            }
            Part::Cancel | Part::Open => {
                let mut n = Node::new(Role::Button);
                n.set_label(if part == Part::Open {
                    v.open.clone()
                } else {
                    v.cancel.clone()
                });
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
    if let Some(e) = &v.error {
        let id = NodeId(A11Y_BASE + 0x40);
        let mut n = Node::new(Role::Alert);
        n.set_live(accesskit::Live::Assertive);
        n.set_label(e.clone());
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
        0x80 => Some(Part::Field),
        k @ 0x10..0x14 => Part::ORDER.get(k as usize - 0x10).copied(),
        _ => None,
    }
}
