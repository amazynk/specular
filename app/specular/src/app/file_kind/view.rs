//! The "Which kind of file is …?" prompt as data, its drawing and its accessibility tree.
//! Nothing here holds app state, so it is drawn and checked without an app.
//!
//! A small sheet like the raw volume dialog: the question, why it is asked, one radio row
//! per format (its name, how sure the check is, and a line saying what it is), a
//! `‹ value ›` stepper for how widely to remember the answer, a line on changing it later,
//! and Cancel and Open at the end edge. Right to left mirrors it.

use specular_gfx::{Align, DrawList, Icon, Rect, Text, TextStyle, draw_icon};
use specular_theme::metrics::{FONT_LABEL, FONT_SMALL, FONT_TITLE, RADIUS_LG, RADIUS_SM};
use specular_theme::{Color, Palette};
use specular_widgets::accesskit;

pub(in crate::app) const A11Y_BASE: u64 = 0x5D50_0000_0000_0000;

/// What has the keyboard, in focus order; also what a click or a screen reader hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Part {
    /// A format row, by index.
    Option(usize),
    /// How widely the answer is remembered.
    Scope,
    Cancel,
    Open,
}

impl Part {
    pub fn order(options: usize, scope: bool) -> Vec<Part> {
        (0..options)
            .map(Part::Option)
            .chain(scope.then_some(Part::Scope))
            .chain([Part::Cancel, Part::Open])
            .collect()
    }

    fn index(self) -> u64 {
        match self {
            Part::Option(i) => i as u64,
            Part::Scope => 0x800,
            Part::Cancel => 0x801,
            Part::Open => 0x802,
        }
    }
}

/// A click: a part, or one of the scope stepper's arrows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Hit {
    Part(Part),
    Dec,
    Inc,
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct OptionView {
    pub name: String,
    /// How sure the check is ("Likely"), or empty.
    pub tag: String,
    /// One line on what the format is.
    pub about: String,
}

/// Everything the prompt draws.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct PromptView {
    pub title: String,
    /// Why it is asked.
    pub subtitle: String,
    pub options: Vec<OptionView>,
    /// The chosen row.
    pub selected: usize,
    /// The scope stepper's label and value; `None` when the answer is not remembered (raw
    /// samples, whose layout is remembered per file).
    pub scope: Option<(String, String)>,
    /// Where to change it later.
    pub note: String,
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
        .clamp(260.0, 500.0 + 140.0 * (ts - 1.0).max(0.0))
        .min(ww);
    let inner = w - 2.0 * pad;
    let mi = Mirror { ww, rtl };

    let title_st = TextStyle::new(FONT_TITLE * ts).weight(650.0);
    let name_st = TextStyle::new(FONT_LABEL * ts).weight(600.0);
    let label_st = TextStyle::new(FONT_LABEL * ts);
    let small_st = TextStyle::new(FONT_SMALL * ts);
    let button_st = TextStyle::new(FONT_LABEL * ts).weight(550.0);

    let title = text.layout(&v.title, title_st, Some(inner), Align::Start);
    let subtitle = text.layout(&v.subtitle, small_st, Some(inner), Align::Start);
    let note = text.layout(&v.note, small_st, Some(inner), Align::Start);
    let radio = (14.0 * ts).max(14.0);
    let text_x = radio + 12.0;
    let text_w = (inner - text_x - 8.0).max(40.0);
    // Each row: the name (and tag) on one line, what it is under it.
    let rows: Vec<_> = v
        .options
        .iter()
        .map(|o| {
            let name = text.layout(&o.name, name_st, None, Align::Start);
            let tag =
                (!o.tag.is_empty()).then(|| text.layout(&o.tag, small_st, None, Align::Start));
            let about = text.layout(&o.about, small_st, Some(text_w), Align::Start);
            let h = 8.0 + name.height + 2.0 + about.height + 8.0;
            (name, tag, about, h)
        })
        .collect();
    let step_h = (24.0 * ts).max(30.0);
    let bh = (24.0 * ts).max(32.0);

    let mut h = pad + title.height + 6.0 + subtitle.height + 12.0;
    h += rows.iter().map(|r| r.3 + 2.0).sum::<f32>();
    if v.scope.is_some() {
        h += 10.0 + step_h;
    }
    h += 10.0 + note.height + 18.0 + bh + pad;
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
    y += subtitle.height + 12.0;

    for (i, (name, tag, about, rh)) in rows.iter().enumerate() {
        let r = Rect::new(x0, y, inner, *rh);
        let on = v.selected == i;
        if on {
            dl.rect(mi.r(r), RADIUS_SM, pal.surface_raised);
        }
        if v.focus == Part::Option(i) {
            dl.rect_border(
                mi.r(r.inset(-2.0)),
                RADIUS_SM + 2.0,
                Color::TRANSPARENT,
                pal.focus_ring,
                2.0,
            );
        }
        // The radio: a ring, filled in the middle when chosen.
        let c = Rect::new(
            r.x + 6.0,
            r.y + 8.0 + (name.height - radio) * 0.5,
            radio,
            radio,
        );
        let c = mi.r(c);
        dl.rect_border(
            c,
            radio * 0.5,
            Color::TRANSPARENT,
            if on { pal.accent_fill } else { pal.text_muted },
            1.6,
        );
        if on {
            dl.rect(c.inset(radio * 0.25), radio * 0.25, pal.accent_fill);
        }
        let tx = r.x + 6.0 + text_x;
        text.draw(dl, name, mi.x(tx, name.width), r.y + 8.0, pal.text);
        if let Some(t) = tag {
            let room = r.right() - 8.0 - (tx + name.width + 10.0);
            if t.width <= room {
                text.draw(
                    dl,
                    t,
                    mi.x(r.right() - 8.0 - t.width, t.width),
                    r.y + 8.0 + (name.height - t.height) * 0.5,
                    pal.text_muted,
                );
            }
        }
        text.draw(
            dl,
            about,
            mi.x(tx, about.width),
            r.y + 8.0 + name.height + 2.0,
            pal.text_muted,
        );
        drawn.hits.push((mi.r(r), Hit::Part(Part::Option(i))));
        y += rh + 2.0;
    }

    // How widely to remember it: the label at the start edge, `‹ value ›` at the end edge.
    if let Some((label, value)) = &v.scope {
        y += 10.0;
        let r = Rect::new(x0, y, inner, step_h);
        if v.focus == Part::Scope {
            dl.rect_border(
                mi.r(r.inset(-2.0)),
                RADIUS_SM,
                Color::TRANSPARENT,
                pal.focus_ring,
                2.0,
            );
        }
        let arrow = (step_h - 6.0).min(26.0);
        let sw = (inner * 0.62).min(300.0 * ts.max(1.0));
        let sx = r.right() - sw;
        let lab = text.layout(label, label_st, None, Align::Start);
        let lr = Rect::new(r.x + 4.0, r.y, (sx - 8.0 - r.x - 4.0).max(0.0), step_h);
        dl.push_clip(mi.r(lr));
        text.draw(
            dl,
            &lab,
            mi.x(lr.x, lab.width),
            r.y + (step_h - lab.height) * 0.5,
            pal.text_muted,
        );
        dl.pop_clip();
        let dec = Rect::new(sx, r.y + (step_h - arrow) * 0.5, arrow, arrow);
        let inc = Rect::new(r.right() - arrow, dec.y, arrow, arrow);
        for (b, icon, hit) in [
            (dec, Icon::ChevronLeft, Hit::Dec),
            (inc, Icon::ChevronRight, Hit::Inc),
        ] {
            let b = mi.r(b);
            dl.rect(b, RADIUS_SM, pal.surface_raised);
            draw_icon(dl, icon, b.inset(arrow * 0.18), pal.text, 1.6, mi.rtl);
            drawn.hits.push((b, hit));
        }
        let room = sw - 2.0 * arrow;
        let val = text.layout(value, label_st, None, Align::Start);
        let mid = sx + arrow + (room - val.width.min(room)) * 0.5;
        dl.push_clip(mi.r(Rect::new(sx + arrow, r.y, room, step_h)));
        text.draw(
            dl,
            &val,
            mi.x(mid, val.width),
            r.y + (step_h - val.height) * 0.5,
            pal.text,
        );
        dl.pop_clip();
        drawn.hits.push((mi.r(r), Hit::Part(Part::Scope)));
        y += step_h;
    }
    y += 10.0;
    text.draw(dl, &note, mi.x(x0, note.width), y, pal.text_muted);

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
        drawn.hits.push((rm, Hit::Part(part)));
    }
    drawn
}

fn id_of(p: Part) -> accesskit::NodeId {
    accesskit::NodeId(A11Y_BASE + 0x100 + p.index())
}

/// The prompt's nodes: a modal dialog with a radio group of formats (each labelled with
/// its name and described by what it is), the scope as a spin button, the note and the
/// buttons.
pub(in crate::app) fn a11y(
    tree: &mut accesskit::TreeUpdate,
    v: &PromptView,
    d: &Drawn,
    scale: f64,
) {
    use accesskit::{Node, NodeId, Role, Toggled};
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
    let group_id = NodeId(A11Y_BASE + 0x20);
    let mut group = Node::new(Role::RadioGroup);
    group.set_label(v.title.clone());
    let mut radios = Vec::new();
    for part in Part::order(v.options.len(), v.scope.is_some()) {
        let mut n = match part {
            Part::Option(i) => {
                let o = &v.options[i];
                let mut n = Node::new(Role::RadioButton);
                n.set_label(if o.tag.is_empty() {
                    o.name.clone()
                } else {
                    format!("{}, {}", o.name, o.tag)
                });
                n.set_description(o.about.clone());
                n.set_toggled(if v.selected == i {
                    Toggled::True
                } else {
                    Toggled::False
                });
                n.add_action(accesskit::Action::Click);
                n
            }
            Part::Scope => {
                let Some((label, value)) = &v.scope else {
                    continue;
                };
                let mut n = Node::new(Role::SpinButton);
                n.set_label(label.clone());
                n.set_value(value.clone());
                n.add_action(accesskit::Action::Increment);
                n.add_action(accesskit::Action::Decrement);
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
        if let Some((r, _)) = d.hits.iter().find(|(_, h)| *h == Hit::Part(part)) {
            n.set_bounds(bounds(*r));
        }
        n.add_action(accesskit::Action::Focus);
        if matches!(part, Part::Option(_)) {
            radios.push(id_of(part));
        } else {
            ids.push(id_of(part));
        }
        nodes.push((id_of(part), n));
    }
    group.set_children(radios);
    ids.insert(0, group_id);
    nodes.push((group_id, group));
    let nid = NodeId(A11Y_BASE + 0x40);
    let mut n = Node::new(Role::Label);
    n.set_label(v.note.clone());
    ids.push(nid);
    nodes.push((nid, n));
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
    match k {
        0x800 => Some(Part::Scope),
        0x801 => Some(Part::Cancel),
        0x802 => Some(Part::Open),
        i if i < 0x800 => Some(Part::Option(i as usize)),
        _ => None,
    }
}
