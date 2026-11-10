//! Markup on drawings and images: revision clouds, text,
//! arrows with a label, stamps, lengths and areas, and photos pinned to a point, in a sidecar
//! next to the file (`specular-markup`; XFDF for PDFs, JSON otherwise). The file itself is
//! never written: only the sidecar, and only when the user adds, moves, edits, deletes, undoes
//! or redoes something.
//!
//! - Coordinates are the sheet's units (a PDF page's points as displayed, else the image's
//!   pixels); this module maps them to stored pixels (a PDF page's raster is its points times
//!   the session's pixels per point) and the screen.
//! - The tool's bar has the tools, the colour, the line width, the stamp's words, undo, redo,
//!   delete, the measurements as markup, and the PDF and PNG exports; the list on the end side
//!   names the page's markup.
//! - Keyboard: letters pick the tools, the arrows move a crosshair (Shift: ten pixels), Space
//!   places a point, Enter finishes a cloud or an area (or edits the selected text), Tab walks
//!   the bar and the list, Delete deletes (after asking), Ctrl+Z / Ctrl+Shift+Z undo and redo.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crossbeam_channel::{Receiver, Sender};
use specular_gfx::winit::event_loop::ActiveEventLoop;
use specular_gfx::{Align, DrawList, Rect, Text, TextBlock, TextStyle};
use specular_i18n::t;
use specular_input::{Action, Group, Key, KeyCombo, NamedKey};
use specular_log::{UI, warn};
use specular_markup::{self as mk, Affine, Doc, Markup, Shape, Space, Stamp};
use specular_theme::metrics::{FONT_LABEL, FONT_SMALL, RADIUS_LG, RADIUS_SM};
use specular_theme::{Color, Palette};
use specular_widgets::accesskit;
use specular_widgets::line_edit::{LineEdit, PendingClipboard};

use super::App;
use super::integration::PasteTo;

#[cfg(test)]
mod shots;

const A11Y_BASE: u64 = 0x6500_0000_0000_0000;
/// "Delete this markup?"
pub(super) const DELETE_ITEM: u64 = super::items::MARKUP_DELETE;
/// A press and release closer than this (logical px) is a click, not a drag.
const CLICK_SLOP: f32 = 4.0;
/// A click this close (logical px) to a cloud's or an area's first corner closes it.
const CLOSE_RADIUS: f32 = 8.0;
/// A click this close (logical px) to a line picks it.
const PICK_RADIUS: f32 = 6.0;
/// The largest sidecar read (a sidecar this large is not one of ours).
const MAX_SIDECAR: u64 = 64 << 20;

/// A markup tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Select,
    Cloud,
    Text,
    Arrow,
    Stamp,
    Length,
    Area,
    Pin,
}

impl Kind {
    pub const ALL: [Kind; 8] = [
        Kind::Select,
        Kind::Cloud,
        Kind::Text,
        Kind::Arrow,
        Kind::Stamp,
        Kind::Length,
        Kind::Area,
        Kind::Pin,
    ];

    fn id(self) -> &'static str {
        match self {
            Kind::Select => "select",
            Kind::Cloud => "cloud",
            Kind::Text => "text",
            Kind::Arrow => "arrow",
            Kind::Stamp => "stamp",
            Kind::Length => "length",
            Kind::Area => "area",
            Kind::Pin => "pin",
        }
    }

    pub fn label(self) -> String {
        tr(&format!("markup-tool-{}", self.id()))
    }

    pub fn hint(self) -> String {
        tr(&format!("markup-hint-{}", self.id()))
    }

    fn letter(self) -> char {
        match self {
            Kind::Select => 'v',
            Kind::Cloud => 'c',
            Kind::Text => 't',
            Kind::Arrow => 'a',
            Kind::Stamp => 's',
            Kind::Length => 'l',
            Kind::Area => 'r',
            Kind::Pin => 'p',
        }
    }

    /// Points a shape takes (None: until finished).
    fn points(self) -> Option<usize> {
        match self {
            Kind::Select | Kind::Cloud | Kind::Area => None,
            Kind::Text | Kind::Stamp | Kind::Pin => Some(1),
            Kind::Arrow | Kind::Length => Some(2),
        }
    }

    /// A drag draws it in one go (two points, or a rectangle for a cloud).
    fn drags(self) -> bool {
        matches!(self, Kind::Arrow | Kind::Length | Kind::Cloud)
    }
}

/// The colours offered, with their names.
pub(super) const COLOURS: [([u8; 3], &str); 6] = [
    ([229, 57, 53], "markup-colour-red"),
    ([30, 136, 229], "markup-colour-blue"),
    ([67, 160, 71], "markup-colour-green"),
    ([251, 140, 0], "markup-colour-orange"),
    ([216, 27, 96], "markup-colour-magenta"),
    ([33, 33, 33], "markup-colour-black"),
];

/// Line widths offered (times the sheet's thin line), with their names.
const WIDTHS: [(f64, &str); 3] = [
    (1.0, "markup-width-thin"),
    (2.0, "markup-width-medium"),
    (4.0, "markup-width-thick"),
];

/// A message whose id is made at run time.
fn tr(id: &str) -> String {
    specular_i18n::I18n::global().tr(id)
}

/// The name of a kind of object ("Cloud").
fn kind_name(s: &Shape) -> String {
    tr(&format!("markup-kind-{}", s.kind()))
}

fn stamp_label(s: Stamp) -> String {
    match s {
        Stamp::Approved => t!("markup-stamp-approved"),
        Stamp::Rejected => t!("markup-stamp-rejected"),
        Stamp::Reviewed => t!("markup-stamp-reviewed"),
        Stamp::Custom => t!("markup-stamp-custom"),
    }
}

/// A stamp's words as drawn and exported.
pub(super) fn stamp_words(m: &Markup) -> String {
    match &m.shape {
        Shape::Stamp {
            kind: Stamp::Custom,
            ..
        } => m.text.to_uppercase(),
        Shape::Stamp { kind, .. } => stamp_label(*kind).to_uppercase(),
        _ => String::new(),
    }
}

/// What a click or a screen reader hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Hit {
    Tool(Kind),
    Colour,
    Width,
    StampKind,
    Undo,
    Redo,
    Delete,
    OpenPhoto,
    FromMeasure,
    ExportPdf,
    ExportPng,
    Done,
    Item(usize),
}

/// Part of an object as drawn (logical px).
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Prim {
    Line {
        pts: Vec<(f32, f32)>,
        closed: bool,
    },
    /// Text with its top left (or, centred, its top middle) here.
    Text {
        s: String,
        at: (f32, f32),
        px: f32,
        centre: bool,
    },
    /// A stamp centred here: its words and its second line.
    Stamp {
        c: (f32, f32),
        words: String,
        line: String,
        px: f32,
    },
    /// A pin: its marker's centre and radius, and its point.
    Pin {
        c: (f32, f32),
        r: f32,
        tip: (f32, f32),
    },
}

/// One object as drawn and listed.
#[derive(Clone, Debug, Default)]
pub(super) struct ItemView {
    /// "3. Cloud".
    pub title: String,
    /// Its text or value, and who made it when.
    pub detail: String,
    pub colour: [u8; 3],
    /// Line width (logical px).
    pub width: f32,
    pub prims: Vec<Prim>,
    /// Its extent on screen.
    pub bounds: Rect,
    /// The outline a click picks (logical px), and whether its inside counts.
    pub outline: Vec<(f32, f32)>,
    pub filled: bool,
    pub pin: bool,
}

/// The text being typed: what for, and the field.
#[derive(Clone, Debug)]
pub(super) struct PromptView {
    pub label: String,
    /// The whole text, lines joined by `\n`.
    pub text: String,
    /// Its lines, and the caret: line, byte in it.
    pub lines: Vec<String>,
    pub caret: (usize, usize),
    /// Shift+Enter starts a line (a text markup's field).
    pub multiline: bool,
    /// An input method's composition at the caret (empty: none).
    pub preedit: String,
    /// The selected bytes of the caret's line (start, end; equal: none).
    pub selection: (usize, usize),
}

/// The bar's text field: its label, lines and caret (line, byte in it).
#[derive(Clone, Copy, Debug)]
pub(super) struct Field<'a> {
    pub label: &'a str,
    pub lines: &'a [String],
    pub caret: (usize, usize),
    /// An input method's composition at the caret (empty: none).
    pub preedit: &'a str,
    /// The selected bytes of the caret's line (start, end; equal: none).
    pub selection: (usize, usize),
}

/// Everything the tool draws (logical px).
#[derive(Clone, Debug)]
pub(super) struct MarkupView {
    /// The bar and the list are shown (otherwise only the markup).
    pub open: bool,
    pub kind: Kind,
    pub colour: usize,
    pub width: usize,
    pub stamp: Stamp,
    pub items: Vec<ItemView>,
    pub selected: Option<usize>,
    /// The shape being drawn, to the pointer or crosshair.
    pub drawing: Vec<(f32, f32)>,
    pub drawing_closed: bool,
    pub marks: Vec<(f32, f32)>,
    pub cursor: Option<(f32, f32)>,
    pub status: String,
    pub prompt: Option<PromptView>,
    pub can_undo: bool,
    pub can_redo: bool,
    pub has_measurements: bool,
    pub focus: Option<usize>,
}

impl MarkupView {
    pub fn focusables(&self) -> Vec<Hit> {
        let mut v: Vec<Hit> = Kind::ALL.iter().map(|k| Hit::Tool(*k)).collect();
        v.extend([Hit::Colour, Hit::Width]);
        if self.kind == Kind::Stamp {
            v.push(Hit::StampKind);
        }
        v.extend([Hit::Undo, Hit::Redo, Hit::Delete]);
        if self.selected_pin() {
            v.push(Hit::OpenPhoto);
        }
        v.extend([Hit::FromMeasure, Hit::ExportPdf, Hit::ExportPng, Hit::Done]);
        v.extend((0..self.items.len()).map(Hit::Item));
        v
    }

    fn selected_pin(&self) -> bool {
        self.selected
            .and_then(|i| self.items.get(i))
            .is_some_and(|it| it.pin)
    }

    pub fn enabled(&self, h: Hit) -> bool {
        match h {
            Hit::Undo => self.can_undo,
            Hit::Redo => self.can_redo,
            Hit::Delete => self.selected.is_some(),
            Hit::OpenPhoto => self.selected_pin(),
            Hit::FromMeasure => self.has_measurements,
            _ => true,
        }
    }

    fn label(&self, h: Hit) -> String {
        match h {
            Hit::Tool(k) => k.label(),
            Hit::Colour => t!(
                "markup-colour",
                colour = tr(COLOURS[self.colour % COLOURS.len()].1)
            ),
            Hit::Width => t!(
                "markup-width",
                width = tr(WIDTHS[self.width % WIDTHS.len()].1)
            ),
            Hit::StampKind => t!("markup-stamp", words = stamp_label(self.stamp)),
            Hit::Undo => t!("markup-undo"),
            Hit::Redo => t!("markup-redo"),
            Hit::Delete => t!("markup-delete"),
            Hit::OpenPhoto => t!("markup-open-photo"),
            Hit::FromMeasure => t!("markup-from-measurements"),
            Hit::ExportPdf => t!("markup-export-pdf"),
            Hit::ExportPng => t!("markup-export-png"),
            Hit::Done => t!("markup-done"),
            Hit::Item(i) => self
                .items
                .get(i)
                .map(|it| format!("{}: {}", it.title, it.detail))
                .unwrap_or_default(),
        }
    }
}

/// Where the controls landed.
#[derive(Clone, Debug, Default)]
pub(super) struct Drawn {
    pub bar: Rect,
    pub panel: Rect,
    pub hits: Vec<(Rect, Hit)>,
    /// Where the field's input method candidate window opens.
    pub ime: Option<Rect>,
    /// The field's lines as drawn: each one's band, its laid-out text and the x that text
    /// starts at (a press there puts the caret).
    pub field_lines: Vec<FieldLine>,
}

/// A line of the bar's field as drawn: its band (the field's width), its laid-out text and
/// the x that text starts at (logical px).
pub(super) type FieldLine = (Rect, Arc<TextBlock>, f32);

/// `(line, x)` of items of `widths` flowed into lines of at most `max`, `gap` apart.
fn flow(widths: &[f32], max: f32, gap: f32) -> Vec<(usize, f32)> {
    let mut out = Vec::with_capacity(widths.len());
    let (mut line, mut x) = (0, 0.0);
    for &w in widths {
        if x > 0.0 && x + w > max {
            line += 1;
            x = 0.0;
        }
        out.push((line, x));
        x += w + gap;
    }
    out
}

/// A button of a tool bar.
pub(super) struct Button {
    pub label: String,
    pub enabled: bool,
    pub active: bool,
    pub focused: bool,
}

/// A tool bar at the top of `area`: the title, the buttons flowed into lines, then a field
/// (label and text, with a caret) when one is open, then the status line. Returns the bar
/// and each button's rectangle.
#[allow(clippy::too_many_arguments)]
pub(super) fn bar(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    title: &str,
    buttons: &[Button],
    field: Option<Field<'_>>,
    status: &str,
    extra_h: f32,
    area: Rect,
    rtl: bool,
    ts: f32,
) -> (Rect, Vec<Rect>, Option<Rect>, Vec<FieldLine>) {
    let ww = area.x * 2.0 + area.w;
    let m = |r: Rect| if rtl { r.mirror_x(ww) } else { r };
    let title_st = TextStyle::new(FONT_LABEL * ts).weight(650.0);
    let button_st = TextStyle::new(FONT_LABEL * ts).weight(550.0);
    let small_st = TextStyle::new(FONT_SMALL * ts);
    let pad = 12.0;
    let gap = 6.0;
    let bh = (28.0 * ts).max(28.0);
    let max_w = (area.w - 32.0).max(160.0);
    let title_b = text.layout(title, title_st, None, Align::Start);
    let blocks: Vec<_> = buttons
        .iter()
        .map(|b| text.layout(&b.label, button_st, None, Align::Start))
        .collect();
    let mut widths = vec![title_b.width + 10.0];
    widths.extend(blocks.iter().map(|b| (b.width + 20.0).max(44.0)));
    let inner = max_w - 2.0 * pad;
    let places = flow(&widths, inner, gap);
    let lines = places.last().map_or(1, |p| p.0 + 1);
    let used = places
        .iter()
        .zip(&widths)
        .map(|((_, x), w)| x + w)
        .fold(0.0f32, f32::max);
    let status_b = text.layout(status, small_st, Some(inner), Align::Center);
    let field_b = field.map(|f| {
        // The caret's line shows the composition at the caret.
        let lines: Vec<_> = f
            .lines
            .iter()
            .enumerate()
            .map(|(i, v)| {
                if i == f.caret.0 && !f.preedit.is_empty() {
                    let (shown, _) = specular_widgets::line_edit::compose(v, f.caret.1, f.preedit);
                    text.layout(&shown, button_st, None, Align::Start)
                } else {
                    text.layout(v, button_st, None, Align::Start)
                }
            })
            .collect();
        (
            text.layout(f.label, small_st, Some(inner), Align::Start),
            lines,
            // The caret: line, byte in it, and the composition's bytes (at the caret).
            (f.caret.0, f.caret.1),
            f.preedit.len(),
            f.selection,
        )
    });
    // The field grows a line at a time.
    let line_h = (bh - 12.0).max(FONT_LABEL * ts * 1.3);
    let field_box = |n: usize| bh.max(n as f32 * line_h + 12.0);
    let field_h = field_b
        .as_ref()
        .map_or(0.0, |(l, v, ..)| l.height + 4.0 + field_box(v.len()) + gap);
    let w = (used.max(status_b.width) + 2.0 * pad)
        .max(if field.is_some() { 360.0 * ts } else { 0.0 })
        .min(max_w);
    let h = pad + lines as f32 * (bh + gap) + field_h + extra_h + status_b.height + pad;
    let r = Rect::new(
        (area.x + (area.w - w) * 0.5).round(),
        area.y + 12.0,
        w.round(),
        h.round(),
    );
    dl.shadow(r, RADIUS_LG, pal.shadow, 24.0, 8.0);
    dl.rect_border(r, RADIUS_LG, pal.surface, pal.glass_border, 1.0);
    let at = |i: usize| {
        let (line, x) = places[i];
        Rect::new(
            r.x + pad + x,
            r.y + pad + line as f32 * (bh + gap),
            widths[i],
            bh,
        )
    };
    let tr = m(at(0));
    text.draw(
        dl,
        &title_b,
        if rtl {
            tr.right() - title_b.width
        } else {
            tr.x
        },
        tr.y + (bh - title_b.height) * 0.5,
        pal.text,
    );
    let mut rects = Vec::with_capacity(buttons.len());
    for (k, b) in buttons.iter().enumerate() {
        let br = m(at(k + 1));
        let (fill, ink) = if b.active {
            (pal.accent_fill, pal.accent_text)
        } else {
            (pal.surface_raised, pal.text)
        };
        let ink = if b.enabled { ink } else { ink.with_alpha(0.45) };
        if b.active {
            dl.rect(br, RADIUS_SM + 2.0, fill);
        } else {
            dl.rect_border(br, RADIUS_SM + 2.0, fill, pal.separator, 1.0);
        }
        if b.focused {
            dl.rect_border(
                br.inset(-3.0),
                RADIUS_SM + 5.0,
                Color::TRANSPARENT,
                pal.focus_ring,
                2.0,
            );
        }
        let tb = &blocks[k];
        text.draw(
            dl,
            tb,
            br.x + (br.w - tb.width) * 0.5,
            br.y + (bh - tb.height) * 0.5,
            ink,
        );
        rects.push(br);
    }
    let mut y = r.y + pad + lines as f32 * (bh + gap);
    let mut ime = None;
    let mut field_lines = Vec::new();
    if let Some((l, lines, (cl, cb), pre, sel)) = &field_b {
        let lx = if rtl {
            r.right() - pad - l.width
        } else {
            r.x + pad
        };
        text.draw(dl, l, lx, y, pal.text_muted);
        y += l.height + 4.0;
        let fh = field_box(lines.len());
        let fr = Rect::new(r.x + pad, y, r.w - 2.0 * pad, fh);
        dl.rect_border(fr, RADIUS_SM + 2.0, pal.surface_raised, pal.focus_ring, 2.0);
        dl.push_clip(fr.inset(2.0));
        for (i, v) in lines.iter().enumerate() {
            let tx = if rtl {
                fr.right() - 8.0 - v.width
            } else {
                fr.x + 8.0
            };
            let ly = if lines.len() == 1 {
                fr.y + (bh - v.height) * 0.5
            } else {
                fr.y + 6.0 + i as f32 * line_h + (line_h - v.height) * 0.5
            };
            // The band a press picks this line in: the whole field for one line.
            let band = if lines.len() == 1 {
                fr
            } else {
                let top = if i == 0 {
                    fr.y
                } else {
                    fr.y + 6.0 + i as f32 * line_h
                };
                let bottom = if i + 1 == lines.len() {
                    fr.bottom()
                } else {
                    fr.y + 6.0 + (i + 1) as f32 * line_h
                };
                Rect::new(fr.x, top, fr.w, bottom - top)
            };
            field_lines.push((band, v.clone(), tx));
            if i == *cl && *pre == 0 && sel.0 < sel.1 {
                for (a, b) in v.range_spans(sel.0..sel.1) {
                    dl.rect(Rect::new(tx + a, ly, b - a, v.height), 0.0, pal.selection);
                }
            }
            text.draw(dl, v, tx, ly, pal.text);
            if i == *cl {
                let x = v.caret_x(*cb + *pre);
                let cx = (tx + x).clamp(fr.x + 4.0, fr.right() - 4.0);
                let (top, bottom) = if lines.len() == 1 {
                    (fr.y + 6.0, fr.bottom() - 6.0)
                } else {
                    (
                        fr.y + 6.0 + i as f32 * line_h,
                        fr.y + 6.0 + (i + 1) as f32 * line_h,
                    )
                };
                dl.line((cx, top + 1.0), (cx, bottom - 1.0), 1.5, pal.text);
                let caret = Rect::new(cx - 0.75, top + 1.0, 1.5, bottom - top - 2.0);
                ime = Some(if *pre > 0 {
                    let range = *cb..*cb + *pre;
                    specular_widgets::line_edit::draw_preedit_underline(
                        dl,
                        v,
                        tx,
                        range.clone(),
                        ly + v.height,
                        pal.text_muted,
                    );
                    specular_widgets::line_edit::ime_area(v, tx, range, caret.y, caret.h, fr)
                } else {
                    caret
                });
            }
        }
        dl.pop_clip();
        y += fh + gap;
    }
    let _ = y;
    text.draw(
        dl,
        &status_b,
        r.x + (r.w - status_b.width) * 0.5,
        r.bottom() - pad - status_b.height,
        pal.text_muted,
    );
    (r, rects, ime, field_lines)
}

fn ink(c: [u8; 3]) -> Color {
    Color::rgba(
        f32::from(c[0]) / 255.0,
        f32::from(c[1]) / 255.0,
        f32::from(c[2]) / 255.0,
        1.0,
    )
}

/// The outline of a colour swatch in the list: the muted text colour, which stands out from
/// the panel in either theme.
fn swatch_edge(pal: &Palette) -> Color {
    pal.text_muted
}

fn polyline(dl: &mut DrawList, pts: &[(f32, f32)], closed: bool, c: Color, w: f32) {
    let n = pts.len();
    let segs = if closed && n > 2 {
        n
    } else {
        n.saturating_sub(1)
    };
    for i in 0..segs {
        dl.line(pts[i], pts[(i + 1) % n], w, c);
    }
}

/// Draws the markup over the image, and when the tool is open its bar (top) and list (on
/// the end side) inside `area` (the viewport, logical px).
#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    v: &MarkupView,
    area: Rect,
    rtl: bool,
    ts: f32,
) -> Drawn {
    let focus = v.focus.and_then(|i| v.focusables().get(i).copied());
    let mut drawn = Drawn::default();
    dl.push_clip(area);
    for (i, it) in v.items.iter().enumerate() {
        let c = ink(it.colour);
        for p in &it.prims {
            match p {
                Prim::Line { pts, closed } => polyline(dl, pts, *closed, c, it.width),
                Prim::Text { s, at, px, centre } => {
                    if *px < 3.0 {
                        continue;
                    }
                    let b = text.layout(s, TextStyle::new(*px).weight(500.0), None, Align::Start);
                    let x = if *centre { at.0 - b.width / 2.0 } else { at.0 };
                    text.draw(dl, &b, x, at.1, c);
                }
                Prim::Stamp {
                    c: ctr,
                    words,
                    line,
                    px,
                } => {
                    let px = px.max(3.0);
                    let wb =
                        text.layout(words, TextStyle::new(px).weight(750.0), None, Align::Start);
                    let lb = (!line.is_empty()).then(|| {
                        text.layout(
                            line,
                            TextStyle::new(px * mk::STAMP_SUB as f32),
                            None,
                            Align::Start,
                        )
                    });
                    let pad = px * mk::STAMP_PAD as f32;
                    let fw = wb.width.max(lb.as_ref().map_or(0.0, |b| b.width)) + 2.0 * pad;
                    let fh = wb.height + lb.as_ref().map_or(0.0, |b| b.height) + 2.0 * pad;
                    let fr = Rect::new(ctr.0 - fw / 2.0, ctr.1 - fh / 2.0, fw, fh);
                    dl.rect_border(
                        fr,
                        px * 0.2,
                        Color::rgba(1.0, 1.0, 1.0, 0.55),
                        c,
                        (px * 0.1).max(1.0),
                    );
                    text.draw(dl, &wb, ctr.0 - wb.width / 2.0, fr.y + pad, c);
                    if let Some(lb) = &lb {
                        text.draw(dl, lb, ctr.0 - lb.width / 2.0, fr.y + pad + wb.height, c);
                    }
                }
                Prim::Pin { c: ctr, r, tip } => {
                    dl.line(*tip, (ctr.0, ctr.1 + r * 0.8), (r * 0.5).max(1.5), c);
                    dl.circle(*ctr, r + 1.0, Color::rgba(1.0, 1.0, 1.0, 0.9));
                    dl.circle(*ctr, *r, c);
                    dl.circle(*ctr, r * 0.4, Color::WHITE);
                }
            }
        }
        if v.selected == Some(i) {
            dl.stroke_rect(it.bounds.inset(-3.0), RADIUS_SM, pal.accent, 1.5);
            for (x, y) in [
                (it.bounds.x - 3.0, it.bounds.y - 3.0),
                (it.bounds.right() + 3.0, it.bounds.y - 3.0),
                (it.bounds.x - 3.0, it.bounds.bottom() + 3.0),
                (it.bounds.right() + 3.0, it.bounds.bottom() + 3.0),
            ] {
                dl.rect(Rect::new(x - 3.0, y - 3.0, 6.0, 6.0), 1.0, pal.accent);
            }
        }
    }
    if !v.drawing.is_empty() {
        polyline(dl, &v.drawing, v.drawing_closed, pal.accent, 1.5);
    }
    for m in &v.marks {
        dl.circle(*m, 4.0, Color::rgba(0.0, 0.0, 0.0, 0.7));
        dl.circle(*m, 2.5, pal.accent);
    }
    if let Some((x, y)) = v.cursor {
        for (a, b) in [
            ((x - 12.0, y), (x - 3.0, y)),
            ((x + 3.0, y), (x + 12.0, y)),
            ((x, y - 12.0), (x, y - 3.0)),
            ((x, y + 3.0), (x, y + 12.0)),
        ] {
            dl.line(a, b, 3.5, Color::rgba(0.0, 0.0, 0.0, 0.7));
            dl.line(a, b, 1.5, pal.accent);
        }
    }
    dl.pop_clip();
    if !v.open {
        return drawn;
    }

    let hits: Vec<Hit> = v
        .focusables()
        .into_iter()
        .filter(|h| !matches!(h, Hit::Item(_)))
        .collect();
    let buttons: Vec<Button> = hits
        .iter()
        .map(|h| Button {
            label: v.label(*h),
            enabled: v.enabled(*h),
            active: *h == Hit::Tool(v.kind) || *h == Hit::Done,
            focused: focus == Some(*h),
        })
        .collect();
    let field = v.prompt.as_ref().map(|p| Field {
        label: &p.label,
        lines: &p.lines,
        caret: p.caret,
        preedit: &p.preedit,
        selection: p.selection,
    });
    let (bar_r, rects, ime, field_lines) = bar(
        dl,
        text,
        pal,
        &t!("markup-title"),
        &buttons,
        field,
        &v.status,
        0.0,
        area,
        rtl,
        ts,
    );
    drawn.bar = bar_r;
    drawn.ime = ime;
    drawn.field_lines = field_lines;
    drawn.hits = rects.into_iter().zip(hits).collect();

    // The list, on the end side under the bar.
    let pad = 12.0;
    let title_st = TextStyle::new(FONT_LABEL * ts).weight(650.0);
    let row_st = TextStyle::new(FONT_LABEL * ts).weight(600.0);
    let small_st = TextStyle::new(FONT_SMALL * ts);
    let ww = area.x * 2.0 + area.w;
    let m = |r: Rect| if rtl { r.mirror_x(ww) } else { r };
    let pw = (260.0 * ts).min(area.w * 0.4).max(160.0);
    let top = bar_r.bottom() + 12.0;
    let room = area.bottom() - 12.0 - top;
    if room < 60.0 {
        return drawn;
    }
    let head = text.layout(
        &t!("markup-count", count = v.items.len() as i64),
        title_st,
        Some(pw - 2.0 * pad),
        Align::Start,
    );
    let tw = pw - 2.0 * pad - 8.0 - 14.0;
    let rows: Vec<_> = v
        .items
        .iter()
        .map(|it| {
            let a = text.layout(&it.title, row_st, Some(tw), Align::Start);
            let b = text.layout(&it.detail, small_st, Some(tw), Align::Start);
            let h = 8.0 + a.height + 2.0 + b.height + 8.0;
            (a, b, h)
        })
        .collect();
    let empty = v
        .items
        .is_empty()
        .then(|| text.layout(&t!("markup-empty"), small_st, Some(tw), Align::Start));
    let content = head.height
        + 8.0
        + rows.iter().map(|r| r.2 + 4.0).sum::<f32>()
        + empty.as_ref().map_or(0.0, |e| e.height + 8.0);
    let ph = (pad + content + pad).min(room);
    let panel = m(Rect::new(area.right() - 12.0 - pw, top, pw, ph.round()));
    dl.shadow(panel, RADIUS_LG, pal.shadow, 24.0, 8.0);
    dl.rect_border(panel, RADIUS_LG, pal.surface, pal.glass_border, 1.0);
    let sx = |r: Rect, w: f32| if rtl { r.right() - pad - w } else { r.x + pad };
    text.draw(dl, &head, sx(panel, head.width), panel.y + pad, pal.text);
    let mut y = panel.y + pad + head.height + 8.0;
    if let Some(e) = &empty {
        text.draw(dl, e, sx(panel, e.width), y, pal.text_muted);
    }
    let want = match focus {
        Some(Hit::Item(i)) => Some(i),
        _ => v.selected,
    };
    let bottom = panel.bottom() - pad;
    let mut first = 0;
    if let Some(want) = want.filter(|w| *w < rows.len()) {
        while first < want && y + rows[first..=want].iter().map(|r| r.2 + 4.0).sum::<f32>() > bottom
        {
            first += 1;
        }
    }
    dl.push_clip(panel.inset(1.0));
    for (i, (a, b, h)) in rows.iter().enumerate().skip(first) {
        if y + h > bottom + 0.5 {
            break;
        }
        let r = Rect::new(panel.x + 6.0, y, panel.w - 12.0, *h);
        if v.selected == Some(i) {
            dl.rect(r, RADIUS_SM + 2.0, pal.accent.with_alpha(0.18));
        }
        if focus == Some(Hit::Item(i)) {
            dl.rect_border(r, RADIUS_SM + 2.0, Color::TRANSPARENT, pal.focus_ring, 2.0);
        }
        // The swatch, then the lines.
        let sw = if rtl {
            Rect::new(r.right() - 18.0, r.y + 10.0, 10.0, 10.0)
        } else {
            Rect::new(r.x + 8.0, r.y + 10.0, 10.0, 10.0)
        };
        // Outlined, so black ink shows on the dark panel and white on the light one.
        dl.rect_border(sw, 2.0, ink(v.items[i].colour), swatch_edge(pal), 1.0);
        let ix = |w: f32| {
            if rtl {
                r.right() - 24.0 - w
            } else {
                r.x + 24.0
            }
        };
        text.draw(dl, a, ix(a.width), r.y + 8.0, pal.text);
        text.draw(dl, b, ix(b.width), r.y + 10.0 + a.height, pal.text_muted);
        drawn.hits.push((r, Hit::Item(i)));
        y += h + 4.0;
    }
    dl.pop_clip();
    drawn.panel = panel;
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

fn node(h: Hit) -> u64 {
    match h {
        Hit::Tool(k) => 0x1 + k as u64,
        Hit::Colour => 0x10,
        Hit::Width => 0x11,
        Hit::StampKind => 0x12,
        Hit::Undo => 0x13,
        Hit::Redo => 0x14,
        Hit::Delete => 0x15,
        Hit::OpenPhoto => 0x16,
        Hit::FromMeasure => 0x17,
        Hit::ExportPdf => 0x18,
        Hit::ExportPng => 0x19,
        Hit::Done => 0x1A,
        Hit::Item(i) => 0x100 + i as u64,
    }
}

/// The control behind AccessKit node `id`.
pub(super) fn target(id: u64, v: &MarkupView) -> Option<Hit> {
    let k = id.checked_sub(A11Y_BASE)?;
    v.focusables().into_iter().find(|h| node(*h) == k)
}

const LIST_NODE: u64 = 0x20;
const FIELD_NODE: u64 = 0x21;

/// The tool's AccessKit nodes: a toolbar with its buttons (and the text field when it is
/// open), and a list box of the page's markup.
pub(super) fn a11y(tree: &mut accesskit::TreeUpdate, v: &MarkupView, d: &Drawn, scale: f64) {
    use accesskit::{Node, NodeId, Role, Toggled};
    if !v.open {
        return;
    }
    let mut bar_n = Node::new(Role::Toolbar);
    bar_n.set_label(t!("markup-title"));
    bar_n.set_description(v.status.clone());
    bar_n.set_bounds(bounds(d.bar, scale));
    let mut list = Node::new(Role::ListBox);
    list.set_label(t!("markup-list"));
    list.set_bounds(bounds(d.panel, scale));
    list.set_size_of_set(v.items.len());
    let focusables = v.focusables();
    let focus_hit = v.focus.and_then(|i| focusables.get(i).copied());
    let mut focus_id = None;
    let (mut bar_ids, mut list_ids, mut nodes) = (Vec::new(), Vec::new(), Vec::new());
    for h in &focusables {
        let id = NodeId(A11Y_BASE + node(*h));
        let mut n = match h {
            Hit::Item(i) => {
                let mut n = Node::new(Role::ListBoxOption);
                n.set_selected(v.selected == Some(*i));
                n.set_position_in_set(*i + 1);
                n
            }
            Hit::Tool(k) => {
                let mut n = Node::new(Role::Button);
                n.set_toggled(if *k == v.kind {
                    Toggled::True
                } else {
                    Toggled::False
                });
                n
            }
            _ => Node::new(Role::Button),
        };
        n.set_label(v.label(*h));
        if let Some((r, _)) = d.hits.iter().find(|(_, x)| x == h) {
            n.set_bounds(bounds(*r, scale));
        }
        if v.enabled(*h) {
            n.add_action(accesskit::Action::Click);
        } else {
            n.set_disabled();
        }
        n.add_action(accesskit::Action::Focus);
        if focus_hit == Some(*h) {
            focus_id = Some(id);
        }
        if matches!(h, Hit::Item(_)) {
            list_ids.push(id);
        } else {
            bar_ids.push(id);
        }
        nodes.push((id, n));
    }
    if let Some(p) = &v.prompt {
        let id = NodeId(A11Y_BASE + FIELD_NODE);
        let mut n = Node::new(if p.multiline {
            Role::MultilineTextInput
        } else {
            Role::TextInput
        });
        n.set_label(p.label.clone());
        n.set_value(p.text.clone());
        n.set_bounds(bounds(d.bar, scale));
        bar_ids.push(id);
        nodes.push((id, n));
        if focus_id.is_none() {
            focus_id = Some(id);
        }
    }
    bar_n.set_children(bar_ids);
    list.set_children(list_ids);
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        root.push_child(NodeId(A11Y_BASE));
        root.push_child(NodeId(A11Y_BASE + LIST_NODE));
    }
    tree.nodes.push((NodeId(A11Y_BASE), bar_n));
    tree.nodes.push((NodeId(A11Y_BASE + LIST_NODE), list));
    tree.nodes.extend(nodes);
    if let Some(id) = focus_id {
        tree.focus = id;
    }
}

/// An object as drawn: `map` takes sheet units to logical px, `ppu` is logical px per unit.
pub(super) fn item_view(
    n: usize,
    m: &Markup,
    map: &dyn Fn(mk::P) -> (f32, f32),
    ppu: f64,
) -> ItemView {
    let words = stamp_words(m);
    let px = (m.size * ppu) as f32;
    let width = ((m.width * ppu) as f32).max(1.0);
    let mut prims = Vec::new();
    let mut outline = Vec::new();
    let mut filled = false;
    let line = |pts: &[mk::P]| pts.iter().map(|p| map(*p)).collect::<Vec<_>>();
    match &m.shape {
        Shape::Cloud(pts) => {
            let arcs = mk::cloud::arcs(pts, mk::cloud::arc_length(m.width, m.size));
            let flat = mk::cloud::flatten(&arcs, 0.4 / ppu.max(1e-9));
            prims.push(Prim::Line {
                pts: line(&flat),
                closed: false,
            });
            outline = line(pts);
            filled = true;
        }
        Shape::Area(pts) => {
            prims.push(Prim::Line {
                pts: line(pts),
                closed: true,
            });
            let a = map(mk::label_anchor(&m.shape));
            prims.push(Prim::Text {
                s: m.text.clone(),
                at: (a.0, a.1 - px * 0.6),
                px,
                centre: true,
            });
            outline = line(pts);
            filled = true;
        }
        Shape::Length(a, b) => {
            prims.push(Prim::Line {
                pts: line(&[*a, *b]),
                closed: false,
            });
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let len = dx.hypot(dy).max(1e-9);
            let (nx, ny) = (-dy / len * m.size * 0.4, dx / len * m.size * 0.4);
            for e in [a, b] {
                prims.push(Prim::Line {
                    pts: line(&[
                        mk::P::new(e.x - nx, e.y - ny),
                        mk::P::new(e.x + nx, e.y + ny),
                    ]),
                    closed: false,
                });
            }
            let c = map(mk::label_anchor(&m.shape));
            prims.push(Prim::Text {
                s: m.text.clone(),
                at: (c.0, c.1 - px * 1.45),
                px,
                centre: true,
            });
            outline = line(&[*a, *b]);
        }
        Shape::Arrow { tail, head } => {
            let tri = mk::arrow_head(*tail, *head, m.width, m.size);
            prims.push(Prim::Line {
                pts: line(&[*tail, *head]),
                closed: false,
            });
            prims.push(Prim::Line {
                pts: line(&[tri[1], tri[0], tri[2]]),
                closed: false,
            });
            if !m.text.is_empty() {
                let t = map(*tail);
                let h = map(*head);
                let w = mk::helvetica::width(&m.text, m.size, false) * ppu;
                let x = if h.0 >= t.0 {
                    t.0 - w as f32 - px * 0.3
                } else {
                    t.0 + px * 0.3
                };
                prims.push(Prim::Text {
                    s: m.text.clone(),
                    at: (x, t.1 - px * 0.6),
                    px,
                    centre: false,
                });
            }
            outline = line(&[*tail, *head]);
        }
        Shape::Text(at) => {
            for (i, l) in m.text.split('\n').enumerate() {
                let p = map(mk::P::new(at.x, at.y + m.size * 1.2 * i as f64));
                prims.push(Prim::Text {
                    s: l.to_owned(),
                    at: p,
                    px,
                    centre: false,
                });
            }
            filled = true;
        }
        Shape::Stamp { at, .. } => {
            prims.push(Prim::Stamp {
                c: map(*at),
                words: words.clone(),
                line: m.stamp_line(),
                px,
            });
            filled = true;
        }
        Shape::Pin { at, .. } => {
            let (c, r) = mk::pin_marker(*at, m.size);
            prims.push(Prim::Pin {
                c: map(c),
                r: ((r * ppu) as f32).max(5.0),
                tip: map(*at),
            });
            filled = true;
        }
    }
    let b = m.bounds(&words);
    let corners = [
        map(mk::P::new(b[0], b[1])),
        map(mk::P::new(b[2], b[1])),
        map(mk::P::new(b[0], b[3])),
        map(mk::P::new(b[2], b[3])),
    ];
    let (x0, y0, x1, y1) = corners.iter().fold(
        (
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ),
        |a, p| (a.0.min(p.0), a.1.min(p.1), a.2.max(p.0), a.3.max(p.1)),
    );
    let mut bounds = Rect::from_min_max(x0, y0, x1, y1);
    if let Shape::Pin { .. } = &m.shape
        && let Some(Prim::Pin { c, r, tip }) = prims.first()
    {
        bounds = Rect::from_min_max(c.0 - r, c.1 - r, c.0 + r, tip.1);
    }
    let kind = kind_name(&m.shape);
    let what = match &m.shape {
        Shape::Stamp { .. } => words,
        Shape::Pin { photo, .. } => photo
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default(),
        _ => m.text.replace('\n', " "),
    };
    let who = m.stamp_line();
    let detail = match (what.is_empty(), who.is_empty()) {
        (false, false) => specular_i18n::dot_join(&[what, who]),
        (false, true) => what,
        (true, _) => who,
    };
    ItemView {
        title: t!("markup-item", n = n as i64, kind = kind),
        detail,
        colour: m.colour,
        width,
        prims,
        bounds,
        outline,
        filled,
        pin: matches!(m.shape, Shape::Pin { .. }),
    }
}

/// Distance from `p` to the segment `a`–`b`.
fn seg_dist(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
}

fn inside(p: (f32, f32), poly: &[(f32, f32)]) -> bool {
    let mut c = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + n - 1) % n]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 {
            c = !c;
        }
    }
    c
}

/// The topmost object at `p` (logical px).
pub(super) fn pick(items: &[ItemView], p: (f32, f32)) -> Option<usize> {
    items.iter().enumerate().rev().find_map(|(i, it)| {
        if !it.bounds.inset(-PICK_RADIUS).contains(p) {
            return None;
        }
        if it.outline.len() < 2 {
            return Some(i);
        }
        let n = it.outline.len();
        let segs = if it.filled { n } else { n - 1 };
        let near = (0..segs).any(|k| {
            seg_dist(p, it.outline[k], it.outline[(k + 1) % n]) <= PICK_RADIUS + it.width / 2.0
        });
        (near || (it.filled && inside(p, &it.outline))).then_some(i)
    })
}

// ---------------------------------------------------------------------------------------------
// State.

/// What is being typed.
#[derive(Clone, Debug)]
enum Asking {
    /// A new text here.
    Text(mk::P),
    /// An arrow's label.
    Arrow(mk::P, mk::P),
    /// A custom stamp's words.
    Stamp(mk::P),
    /// New words for an existing object.
    Edit(String),
}

/// The text being typed. A text markup's field takes several lines: `field` edits the line
/// with the caret, `above` and `below` hold the others (Shift+Enter splits a line, Up and
/// Down move between them, Backspace at a line's start and Delete at its end join two).
struct Prompt {
    asking: Asking,
    field: LineEdit,
    above: Vec<String>,
    below: Vec<String>,
    multiline: bool,
    /// An input method's composition at the caret.
    preedit: String,
}

impl Prompt {
    fn new(asking: Asking, text: &str, multiline: bool) -> Prompt {
        let mut above: Vec<String> = if multiline {
            text.split('\n').map(str::to_owned).collect()
        } else {
            vec![text.to_owned()]
        };
        let last = above.pop().unwrap_or_default();
        Prompt {
            asking,
            field: LineEdit::new(&last),
            above,
            below: Vec::new(),
            multiline,
            preedit: String::new(),
        }
    }

    fn view(&self) -> PromptView {
        let l = match &self.asking {
            Asking::Text(_) => t!("markup-ask-text"),
            Asking::Arrow(..) => t!("markup-ask-label"),
            Asking::Stamp(_) => t!("markup-ask-stamp"),
            Asking::Edit(_) => t!("markup-ask-edit"),
        };
        PromptView {
            label: if self.multiline {
                specular_i18n::dot_join(&[l, t!("markup-ask-new-line")])
            } else {
                l
            },
            text: self.text(),
            lines: self.lines(),
            caret: self.caret(),
            multiline: self.multiline,
            preedit: self.preedit.clone(),
            selection: {
                let r = self.field.selection();
                (r.start, r.end)
            },
        }
    }

    /// A press on line `line` at byte `at` of it (`x` logical px, `now` in seconds, `shift`
    /// extends on the caret's line): the caret goes there, the composition is dropped.
    fn press(&mut self, line: usize, at: usize, x: f32, now: f64, shift: bool) {
        self.preedit.clear();
        let here = self.above.len();
        if line != here {
            let lines = self.lines();
            let Some(l) = lines.get(line).cloned() else {
                return;
            };
            self.above = lines[..line].to_vec();
            self.below = lines[line + 1..].to_vec();
            self.edit_line(&l, at);
        }
        let at = at.min(self.field.text().len());
        self.field.press(at, x, now, shift && line == here);
    }

    /// Every line, joined by `\n`.
    fn text(&self) -> String {
        self.lines().join("\n")
    }

    fn lines(&self) -> Vec<String> {
        let mut v = self.above.clone();
        v.push(self.field.text().to_owned());
        v.extend(self.below.iter().cloned());
        v
    }

    /// The caret: line, byte in it.
    fn caret(&self) -> (usize, usize) {
        (self.above.len(), self.field.caret())
    }

    /// `line` in the field with the caret at `at` (kept on a character boundary).
    fn edit_line(&mut self, line: &str, at: usize) {
        let mut at = at.min(line.len());
        while !line.is_char_boundary(at) {
            at -= 1;
        }
        self.field = LineEdit::new(line);
        self.field.set_caret(at, false);
    }

    /// Shift+Enter: the line splits at the caret (over a selection, which goes).
    fn new_line(&mut self) -> bool {
        if !self.multiline {
            return false;
        }
        let sel = self.field.selection();
        let t = self.field.text().to_owned();
        let (head, tail) = (&t[..sel.start], &t[sel.end..]);
        self.above.push(head.to_owned());
        self.edit_line(tail, 0);
        true
    }

    /// Up (`-1`) or Down (`1`): the caret to the line above or below, at the same byte or
    /// the line's end.
    fn move_line(&mut self, d: i32) -> bool {
        let at = self.field.caret();
        let cur = self.field.text().to_owned();
        let next = if d < 0 {
            self.above.pop()
        } else if self.below.is_empty() {
            None
        } else {
            Some(self.below.remove(0))
        };
        let Some(next) = next else {
            return false;
        };
        if d < 0 {
            self.below.insert(0, cur);
        } else {
            self.above.push(cur);
        }
        self.edit_line(&next, at);
        true
    }

    /// Backspace at a line's start joins it to the line above; Delete at its end, the line
    /// below to it.
    fn join(&mut self, back: bool) -> bool {
        if self.field.has_selection() {
            return false;
        }
        let t = self.field.text().to_owned();
        if back && self.field.caret() == 0 {
            let Some(prev) = self.above.pop() else {
                return false;
            };
            self.edit_line(&format!("{prev}{t}"), prev.len());
            true
        } else if !back && self.field.caret() == t.len() && !self.below.is_empty() {
            let next = self.below.remove(0);
            self.edit_line(&format!("{t}{next}"), t.len());
            true
        } else {
            false
        }
    }

    /// Pasted text: its lines become lines in a text markup's field, spaces elsewhere.
    fn paste(&mut self, s: &str) {
        let s = s.replace("\r\n", "\n");
        if !self.multiline {
            self.field.paste(&s);
            return;
        }
        for (i, part) in s.split('\n').enumerate() {
            if i > 0 {
                self.new_line();
            }
            self.field.paste(part);
        }
    }
}

/// A move of the selected object by dragging.
#[derive(Clone, Copy, Debug)]
struct Drag {
    from: mk::P,
    now: mk::P,
    /// The object was already selected when pressed (a second click on a pin opens it).
    again: bool,
}

/// The open tool.
pub(super) struct Tool {
    kind: Kind,
    colour: usize,
    width: usize,
    stamp: Stamp,
    /// Points placed (sheet units).
    pts: Vec<mk::P>,
    hover: Option<mk::P>,
    /// The keyboard crosshair (sheet units), once an arrow moved it.
    cursor: Option<mk::P>,
    /// The left button went down here (logical px) and is still down.
    press: Option<(f32, f32)>,
    drag: Option<Drag>,
    /// The selected object's id.
    selected: Option<String>,
    focus: Option<usize>,
    prompt: Option<Prompt>,
    status: Option<String>,
    drawn: Drawn,
}

impl Tool {
    fn new() -> Tool {
        Tool {
            kind: Kind::Cloud,
            colour: 0,
            width: 1,
            stamp: Stamp::Approved,
            pts: Vec::new(),
            hover: None,
            cursor: None,
            press: None,
            drag: None,
            selected: None,
            focus: None,
            prompt: None,
            status: None,
            drawn: Drawn::default(),
        }
    }
}

/// The markup of the file shown.
struct Loaded {
    path: PathBuf,
    space: Space,
    doc: Doc,
    /// A PDF's pages' maps from user space to the page as displayed, once read.
    transforms: Option<Arc<Vec<Option<Affine>>>>,
    /// The stored size of page 0 (a JSON sidecar's size).
    size: (u32, u32),
}

/// Results of background work.
enum Done {
    Loaded {
        path: PathBuf,
        space: Space,
        transforms: Option<Arc<Vec<Option<Affine>>>>,
        doc: Result<Doc, String>,
        size: (u32, u32),
    },
    Saved {
        path: PathBuf,
        transforms: Option<Arc<Vec<Option<Affine>>>>,
        result: Result<(), String>,
    },
    Exported(Result<PathBuf, String>),
    Photo {
        path: PathBuf,
        page: u32,
        at: mk::P,
        photo: PathBuf,
    },
}

/// The markup of the current file and the tool.
pub(super) struct MarkupState {
    pub tool: Option<Tool>,
    loaded: Option<Loaded>,
    /// A load in flight for this file.
    loading: Option<PathBuf>,
    /// The object the delete confirmation is about.
    deleting: Option<String>,
    /// Saves are numbered; an older one never overwrites a newer one.
    save_seq: u64,
    saved_seq: Arc<Mutex<u64>>,
    /// Shown when the tool is closed.
    pub visible: bool,
    tx: Sender<Done>,
    rx: Receiver<Done>,
}

impl Default for MarkupState {
    fn default() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        MarkupState {
            tool: None,
            loaded: None,
            loading: None,
            deleting: None,
            save_seq: 0,
            saved_seq: Arc::new(Mutex::new(0)),
            visible: true,
            tx,
            rx,
        }
    }
}

impl std::fmt::Debug for MarkupState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MarkupState")
            .field("open", &self.tool.is_some())
            .field("items", &self.loaded.as_ref().map(|l| l.doc.len()))
            .finish_non_exhaustive()
    }
}

impl MarkupState {
    pub(super) fn set_drawn(&mut self, d: Drawn) {
        if let Some(t) = self.tool.as_mut() {
            t.drawn = d;
        }
    }

    /// The text field is open: input method commits go to it.
    pub(super) fn wants_text(&self) -> bool {
        self.prompting()
    }

    /// An input method's composition (empty: none), kept while the field is open.
    pub(super) fn set_preedit(&mut self, s: &str) {
        if let Some(p) = self.tool.as_mut().and_then(|t| t.prompt.as_mut()) {
            p.preedit = s.to_owned();
        }
    }

    /// Where the input method's candidate window opens (the field's caret).
    pub(super) fn ime_area(&self) -> Option<Rect> {
        self.prompting()
            .then(|| self.tool.as_ref()?.drawn.ime)
            .flatten()
    }

    /// An input method's commit: typed into the field. False when no field is open.
    pub(super) fn commit(&mut self, s: &str) -> bool {
        let s: String = s.chars().filter(|c| !c.is_control()).collect();
        match self.tool.as_mut().and_then(|t| t.prompt.as_mut()) {
            Some(p) => {
                p.preedit.clear();
                if !s.is_empty() {
                    p.field.insert(&s);
                }
                true
            }
            None => false,
        }
    }

    pub(super) fn a11y(&self, tree: &mut accesskit::TreeUpdate, v: &MarkupView, scale: f64) {
        if let Some(t) = &self.tool {
            a11y(tree, v, &t.drawn, scale);
        }
    }

    /// The tool asks for text (it takes the keys).
    pub(super) fn prompting(&self) -> bool {
        self.tool.as_ref().is_some_and(|t| t.prompt.is_some())
    }
}

/// Reads a PDF's page maps (user space to the page as displayed).
fn pdf_transforms(path: &Path) -> Result<Vec<Option<Affine>>, String> {
    let data = std::fs::read(path).map_err(|e| crate::error_text::io_reason(&e))?;
    let doc = specular_pdf::Document::open(Arc::new(data)).map_err(|e| e.to_string())?;
    Ok((0..doc.page_count()).map(|i| doc.user_to_page(i)).collect())
}

fn read_sidecar(
    side: &Path,
    space: Space,
    transforms: Option<&[Option<Affine>]>,
    size: (u32, u32),
) -> Result<Doc, String> {
    let meta = match std::fs::metadata(side) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Doc::new(space)),
        Err(e) => return Err(crate::error_text::io_reason(&e)),
    };
    if meta.len() > MAX_SIDECAR {
        return Err(t!("markup-sidecar-too-large"));
    }
    let text = std::fs::read_to_string(side).map_err(|e| crate::error_text::io_reason(&e))?;
    let pages = match space {
        Space::PdfPage => {
            let tr = transforms.unwrap_or(&[]);
            mk::xfdf::read(&text, &|p| tr.get(p as usize).copied().flatten())
                .map_err(|e| e.to_string())?
        }
        Space::Pixels => mk::json::read(&text, size)?,
    };
    Ok(Doc::with_pages(space, pages))
}

impl App {
    /// The current file, its page and session, once the session is open.
    fn markup_current(&self) -> Option<(PathBuf, u32, Arc<dyn specular_decode::Session>)> {
        let id = self.current_id()?;
        let img = self.images.get(&id)?;
        Some((img.path.clone(), img.page, img.session.clone()?))
    }

    fn markup_space(s: &dyn specular_decode::Session) -> Space {
        if s.page_points().is_some() {
            Space::PdfPage
        } else {
            Space::Pixels
        }
    }

    /// Stored pixels per sheet unit on the current page.
    fn markup_k(&self) -> Option<(f64, f64)> {
        let (_, _, s) = self.markup_current()?;
        match s.page_points() {
            Some((pw, ph)) if pw > 0.0 && ph > 0.0 => Some((
                f64::from(s.info().width) / f64::from(pw),
                f64::from(s.info().height) / f64::from(ph),
            )),
            _ => Some((1.0, 1.0)),
        }
    }

    /// A sheet point on screen (logical px).
    fn markup_screen(&self, p: mk::P, k: (f64, f64)) -> (f32, f32) {
        let a = self.view.stored_from_screen().invert();
        let s = self.dl.scale.max(0.1);
        let q = a.apply(((p.x * k.0) as f32, (p.y * k.1) as f32));
        (q.0 / s, q.1 / s)
    }

    /// The sheet point under a window point (physical px), clamped to the image.
    fn markup_sheet(&self, p: (f32, f32)) -> Option<mk::P> {
        let k = self.markup_k()?;
        let (x, y) = self.view.stored_from_screen().apply(p);
        let (w, h) = self.view.stored_size();
        Some(mk::P::new(
            f64::from(x.clamp(0.0, w as f32)) / k.0,
            f64::from(y.clamp(0.0, h as f32)) / k.1,
        ))
    }

    fn markup_page(&self) -> u32 {
        self.markup_current().map_or(0, |c| c.1)
    }

    /// Loads the current file's markup when the file changed.
    fn markup_sync(&mut self) {
        let Some((path, _, session)) = self.markup_current() else {
            return;
        };
        if self.markup.loaded.as_ref().is_some_and(|l| l.path == path)
            || self.markup.loading.as_ref() == Some(&path)
        {
            return;
        }
        let space = App::markup_space(&*session);
        let side = mk::sidecar_path(&path, space);
        let size = (session.info().width, session.info().height);
        self.markup.loaded = None;
        if !side.exists() {
            self.markup.loaded = Some(Loaded {
                path,
                space,
                doc: Doc::new(space),
                transforms: None,
                size,
            });
            return;
        }
        self.markup.loading = Some(path.clone());
        let tx = self.markup.tx.clone();
        self.loader.background("markup-load", move |l| {
            let transforms = match space {
                Space::PdfPage => match pdf_transforms(&path) {
                    Ok(t) => Some(Arc::new(t)),
                    Err(e) => {
                        let _ = tx.send(Done::Loaded {
                            path,
                            space,
                            transforms: None,
                            doc: Err(e),
                            size,
                        });
                        l.waker()();
                        return;
                    }
                },
                Space::Pixels => None,
            };
            let doc = read_sidecar(
                &side,
                space,
                transforms.as_deref().map(|v| v.as_slice()),
                size,
            );
            let _ = tx.send(Done::Loaded {
                path,
                space,
                transforms,
                doc,
                size,
            });
            l.waker()();
        });
    }

    pub(super) fn markup_tick(&mut self) {
        self.markup_sync();
        while let Ok(d) = self.markup.rx.try_recv() {
            self.redraw = true;
            let now = self.now();
            match d {
                Done::Loaded {
                    path,
                    space,
                    transforms,
                    doc,
                    size,
                } => {
                    if self.markup.loading.as_ref() == Some(&path) {
                        self.markup.loading = None;
                    }
                    let current = self.markup_current().is_some_and(|c| c.0 == path);
                    let doc = match doc {
                        Ok(d) => d,
                        Err(e) => {
                            if current {
                                self.chrome
                                    .toast(t!("markup-load-failed", reason = e), None, now);
                            }
                            Doc::new(space)
                        }
                    };
                    if current {
                        self.markup.loaded = Some(Loaded {
                            path,
                            space,
                            doc,
                            transforms,
                            size,
                        });
                    }
                }
                Done::Saved {
                    path,
                    transforms,
                    result,
                } => {
                    if let Some(l) = self.markup.loaded.as_mut()
                        && l.path == path
                        && l.transforms.is_none()
                    {
                        l.transforms = transforms;
                    }
                    if let Err(e) = result {
                        self.chrome
                            .toast(t!("markup-save-failed", reason = e), None, now);
                    }
                }
                Done::Exported(r) => {
                    let msg = match r {
                        Ok(p) => t!(
                            "markup-exported",
                            name = p
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default()
                        ),
                        Err(e) => t!("markup-export-failed", reason = e),
                    };
                    self.chrome.toast(msg, None, now);
                }
                Done::Photo {
                    path,
                    page,
                    at,
                    photo,
                } => {
                    if self.markup.loaded.as_ref().is_some_and(|l| l.path == path) {
                        let mut m = self.markup_new(Shape::Pin { at, photo });
                        m.text = match &m.shape {
                            Shape::Pin { photo, .. } => photo
                                .file_name()
                                .map(|f| f.to_string_lossy().into_owned())
                                .unwrap_or_default(),
                            _ => String::new(),
                        };
                        self.markup_push(page, m);
                    }
                }
            }
        }
    }

    /// The sheet's thin line and its text height, from its size.
    fn markup_defaults(&self) -> (f64, f64) {
        let k = self.markup_k().unwrap_or((1.0, 1.0));
        let (w, h) = self.view.stored_size();
        let diag = (f64::from(w) / k.0).hypot(f64::from(h) / k.1).max(1.0);
        (diag * 0.0008, diag * 0.012)
    }

    fn markup_new(&self, shape: Shape) -> Markup {
        let (thin, size) = self.markup_defaults();
        let (colour, width) = self
            .markup
            .tool
            .as_ref()
            .map_or((0, 1), |t| (t.colour, t.width));
        let author = self.settings.general.your_name.trim().to_owned();
        Markup::new(
            shape,
            COLOURS[colour % COLOURS.len()].0,
            thin * WIDTHS[width % WIDTHS.len()].0,
            size,
            &author,
        )
    }

    /// Edits the current file's markup on `page`; saves when it changed.
    fn markup_edit(&mut self, page: u32, f: impl FnOnce(&mut Vec<Markup>)) -> bool {
        let Some(l) = self.markup.loaded.as_mut() else {
            return false;
        };
        let changed = l.doc.edit(page, f);
        if changed {
            self.markup_save();
        }
        self.redraw = true;
        changed
    }

    fn markup_push(&mut self, page: u32, m: Markup) {
        let id = m.id.clone();
        let kind = kind_name(&m.shape);
        if self.markup_edit(page, |v| v.push(m))
            && let Some(t) = self.markup.tool.as_mut()
        {
            t.selected = Some(id);
            t.status = None;
            self.chrome.announce_status(t!("markup-added", kind = kind));
        }
    }

    /// Writes the sidecar (in the background; the newest edit wins).
    fn markup_save(&mut self) {
        let Some(l) = self.markup.loaded.as_ref() else {
            return;
        };
        self.markup.save_seq += 1;
        let seq = self.markup.save_seq;
        let saved = self.markup.saved_seq.clone();
        let path = l.path.clone();
        let space = l.space;
        let size = l.size;
        let transforms = l.transforms.clone();
        let pages: Vec<(u32, Vec<Markup>)> = l.doc.pages().map(|(p, v)| (p, v.to_vec())).collect();
        let tx = self.markup.tx.clone();
        self.loader.background("markup-save", move |l| {
            let side = mk::sidecar_path(&path, space);
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let refs: Vec<(u32, &[Markup])> =
                pages.iter().map(|(p, v)| (*p, v.as_slice())).collect();
            let mut transforms = transforms;
            let text = match space {
                Space::PdfPage => {
                    if transforms.is_none() {
                        transforms = pdf_transforms(&path).ok().map(Arc::new);
                    }
                    let Some(tr) = transforms.clone() else {
                        let _ = tx.send(Done::Saved {
                            path,
                            transforms: None,
                            result: Err(t!("markup-pdf-unreadable")),
                        });
                        l.waker()();
                        return;
                    };
                    mk::xfdf::write(
                        &name,
                        &refs,
                        &|p| tr.get(p as usize).copied().flatten(),
                        &stamp_words,
                    )
                }
                Space::Pixels => mk::json::write(&name, size, &refs),
            };
            let result = {
                let mut last = saved
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if *last > seq {
                    Ok(())
                } else {
                    *last = seq;
                    mk::write_atomic(&side, text.as_bytes())
                        .map_err(|e| crate::error_text::io_reason(&e))
                }
            };
            let _ = tx.send(Done::Saved {
                path,
                transforms,
                result,
            });
            l.waker()();
        });
    }

    /// What the tool draws: with the tool open, or while the page has markup to show.
    pub(super) fn markup_view(&self) -> Option<MarkupView> {
        let open = self.markup.tool.is_some();
        let l = self.markup.loaded.as_ref()?;
        let (path, page, _) = self.markup_current()?;
        if l.path != path || (!open && !self.markup.visible) {
            return None;
        }
        let items = l.doc.page(page);
        if !open && items.is_empty() {
            return None;
        }
        let k = self.markup_k()?;
        let s = self.dl.scale.max(0.1);
        // Logical px per sheet unit.
        let a = self.view.stored_from_screen().invert();
        let o = a.apply((0.0, 0.0));
        let e = a.apply((k.0 as f32, 0.0));
        let ppu = f64::from(((e.0 - o.0).hypot(e.1 - o.1)) / s);
        let t = self.markup.tool.as_ref();
        let drag = t.and_then(|t| t.drag);
        let sel_id = t.and_then(|t| t.selected.clone());
        let mut selected = None;
        let views: Vec<ItemView> = items
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let moving = drag.filter(|_| sel_id.as_deref() == Some(m.id.as_str()));
                if sel_id.as_deref() == Some(m.id.as_str()) {
                    selected = Some(i);
                }
                match moving {
                    Some(d) => {
                        let mut m = m.clone();
                        let (dx, dy) = (d.now.x - d.from.x, d.now.y - d.from.y);
                        m.shape.map(|p| mk::P::new(p.x + dx, p.y + dy));
                        item_view(i + 1, &m, &|p| self.markup_screen(p, k), ppu)
                    }
                    None => item_view(i + 1, m, &|p| self.markup_screen(p, k), ppu),
                }
            })
            .collect();
        let Some(t) = t else {
            return Some(MarkupView {
                open: false,
                kind: Kind::Select,
                colour: 0,
                width: 0,
                stamp: Stamp::Approved,
                items: views,
                selected: None,
                drawing: Vec::new(),
                drawing_closed: false,
                marks: Vec::new(),
                cursor: None,
                status: String::new(),
                prompt: None,
                can_undo: false,
                can_redo: false,
                has_measurements: false,
                focus: None,
            });
        };
        let to = |p: mk::P| self.markup_screen(p, k);
        let marks: Vec<(f32, f32)> = t.pts.iter().map(|p| to(*p)).collect();
        let mut drawing = marks.clone();
        let mut closed = false;
        if let Some(h) = t.cursor.or(t.hover)
            && !t.pts.is_empty()
            && t.prompt.is_none()
        {
            if t.kind == Kind::Cloud && t.pts.len() == 1 && t.press.is_some() {
                // A drag: the rectangle.
                let a = t.pts[0];
                drawing = [a, mk::P::new(h.x, a.y), h, mk::P::new(a.x, h.y)]
                    .iter()
                    .map(|p| to(*p))
                    .collect();
                closed = true;
            } else {
                drawing.push(to(h));
                closed = matches!(t.kind, Kind::Cloud | Kind::Area) && t.pts.len() >= 2;
            }
        }
        let status = if let Some(s) = &t.status {
            s.clone()
        } else {
            t.kind.hint()
        };
        let prompt = t.prompt.as_ref().map(Prompt::view);
        Some(MarkupView {
            open: true,
            kind: t.kind,
            colour: t.colour,
            width: t.width,
            stamp: t.stamp,
            items: views,
            selected,
            drawing,
            drawing_closed: closed,
            marks,
            cursor: t.cursor.map(to),
            status,
            prompt,
            can_undo: l.doc.can_undo(),
            can_redo: l.doc.can_redo(),
            has_measurements: !self.measure_markup().is_empty(),
            focus: t.focus,
        })
    }

    pub(super) fn markup_open(&mut self) {
        if self.markup.tool.is_some() || self.shown.is_none() {
            return;
        }
        self.markup_sync();
        self.markup.tool = Some(Tool::new());
        self.markup.visible = true;
        self.chrome.announce_status(Kind::Cloud.hint());
        self.redraw = true;
    }

    fn markup_close(&mut self) {
        self.markup.tool = None;
        let n = self
            .markup
            .loaded
            .as_ref()
            .map_or(0, |l| l.doc.page(self.markup_page()).len());
        self.chrome
            .announce_status(t!("markup-count", count = n as i64));
        self.redraw = true;
    }

    fn markup_set_kind(&mut self, k: Kind) {
        if let Some(t) = self.markup.tool.as_mut() {
            t.kind = k;
            t.pts.clear();
            t.prompt = None;
            t.status = None;
        }
        self.chrome.announce_status(k.hint());
        self.redraw = true;
    }

    fn markup_selected(&self) -> Option<(u32, Markup)> {
        let id = self.markup.tool.as_ref()?.selected.clone()?;
        let page = self.markup_page();
        let m = self
            .markup
            .loaded
            .as_ref()?
            .doc
            .page(page)
            .iter()
            .find(|m| m.id == id)?
            .clone();
        Some((page, m))
    }

    /// Finishes the shape being drawn.
    fn markup_finish(&mut self) {
        let Some(t) = self.markup.tool.as_mut() else {
            return;
        };
        let pts = std::mem::take(&mut t.pts);
        let kind = t.kind;
        let page = self.markup_page();
        match kind {
            Kind::Cloud if pts.len() >= 3 => {
                let m = self.markup_new(Shape::Cloud(pts));
                self.markup_push(page, m);
            }
            Kind::Cloud if pts.len() == 2 => {
                // A rectangle from a drag.
                let (a, b) = (pts[0], pts[1]);
                let m = self.markup_new(Shape::Cloud(vec![
                    a,
                    mk::P::new(b.x, a.y),
                    b,
                    mk::P::new(a.x, b.y),
                ]));
                self.markup_push(page, m);
            }
            Kind::Area if pts.len() >= 3 => {
                let mut m = self.markup_new(Shape::Area(pts.clone()));
                m.text = self.markup_value(&m.shape);
                self.markup_push(page, m);
            }
            Kind::Length if pts.len() == 2 => {
                let mut m = self.markup_new(Shape::Length(pts[0], pts[1]));
                m.text = self.markup_value(&m.shape);
                self.markup_push(page, m);
            }
            Kind::Text if pts.len() == 1 => self.markup_ask(Asking::Text(pts[0]), ""),
            Kind::Arrow if pts.len() == 2 => self.markup_ask(Asking::Arrow(pts[0], pts[1]), ""),
            Kind::Stamp if pts.len() == 1 => {
                let st = self
                    .markup
                    .tool
                    .as_ref()
                    .map_or(Stamp::Approved, |t| t.stamp);
                if st == Stamp::Custom {
                    self.markup_ask(Asking::Stamp(pts[0]), "");
                } else {
                    let m = self.markup_new(Shape::Stamp {
                        at: pts[0],
                        kind: st,
                    });
                    self.markup_push(page, m);
                }
            }
            Kind::Pin if pts.len() == 1 => self.markup_pick_photo(page, pts[0]),
            _ => {}
        }
        self.redraw = true;
    }

    /// A measurement's value as the measurement tool gives it.
    fn markup_value(&mut self, s: &Shape) -> String {
        let Some(k) = self.markup_k() else {
            return String::new();
        };
        let st = |p: &mk::P| specular_measure::geom::P::new(p.x * k.0, p.y * k.1);
        let shape = match s {
            Shape::Length(a, b) => specular_measure::geom::Shape::Line(st(a), st(b)),
            Shape::Area(pts) => {
                specular_measure::geom::Shape::Polygon(pts.iter().map(st).collect())
            }
            _ => return String::new(),
        };
        self.measure_value(shape)
    }

    fn markup_ask(&mut self, asking: Asking, text: &str) {
        // A text markup takes several lines (Shift+Enter); labels and stamps one.
        let page = self.markup_page();
        let multiline = match &asking {
            Asking::Text(_) => true,
            Asking::Edit(id) => self.markup.loaded.as_ref().is_some_and(|l| {
                l.doc
                    .page(page)
                    .iter()
                    .any(|m| &m.id == id && matches!(m.shape, Shape::Text(_)))
            }),
            _ => false,
        };
        if let Some(t) = self.markup.tool.as_mut() {
            t.prompt = Some(Prompt::new(asking, text, multiline));
            t.focus = None;
        }
        self.chrome.announce_status(t!("markup-ask-text"));
        self.redraw = true;
    }

    /// The typed text is done (Enter).
    fn markup_answer(&mut self) {
        let Some(p) = self.markup.tool.as_mut().and_then(|t| t.prompt.take()) else {
            return;
        };
        // Blank lines at the ends go; inner ones stay.
        let text = p
            .text()
            .trim_matches(|c: char| c.is_whitespace())
            .to_owned();
        let page = self.markup_page();
        match p.asking {
            Asking::Text(at) if !text.is_empty() => {
                let mut m = self.markup_new(Shape::Text(at));
                m.text = text;
                self.markup_push(page, m);
            }
            Asking::Arrow(tail, head) => {
                let mut m = self.markup_new(Shape::Arrow { tail, head });
                m.text = text;
                self.markup_push(page, m);
            }
            Asking::Stamp(at) if !text.is_empty() => {
                let mut m = self.markup_new(Shape::Stamp {
                    at,
                    kind: Stamp::Custom,
                });
                m.text = text;
                self.markup_push(page, m);
            }
            Asking::Edit(id) => {
                self.markup_edit(page, |v| {
                    if let Some(m) = v.iter_mut().find(|m| m.id == id) {
                        m.text = text;
                    }
                });
            }
            _ => {}
        }
        self.redraw = true;
    }

    /// Asks the user for a photo to pin at `at` (the file dialog runs in the background).
    fn markup_pick_photo(&mut self, page: u32, at: mk::P) {
        let Some(l) = self.markup.loaded.as_ref() else {
            return;
        };
        let path = l.path.clone();
        let dir = path.parent().map(Path::to_path_buf);
        let tx = self.markup.tx.clone();
        self.loader.background("markup-photo", move |l| {
            let mut dlg = rfd::FileDialog::new().add_filter(
                "Photos",
                &[
                    "jpg", "jpeg", "heic", "heif", "png", "webp", "avif", "tif", "tiff", "dng",
                ],
            );
            if let Some(d) = dir {
                dlg = dlg.set_directory(d);
            }
            let Some(photo) = dlg.pick_file() else { return };
            let photo = std::path::absolute(&photo).unwrap_or(photo);
            let _ = tx.send(Done::Photo {
                path,
                page,
                at,
                photo,
            });
            l.waker()();
        });
    }

    /// Opens a pin's photo in a new window (the sheet stays where it is).
    fn markup_open_photo(&mut self) {
        let Some((_, m)) = self.markup_selected() else {
            return;
        };
        let Shape::Pin { photo, .. } = &m.shape else {
            return;
        };
        let now = self.now();
        if !photo.exists() {
            self.chrome.toast(
                t!(
                    "markup-photo-missing",
                    name = photo
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                ),
                None,
                now,
            );
            return;
        }
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        if let Err(e) = std::process::Command::new(exe)
            .arg("--new-window")
            .arg(photo)
            .spawn()
        {
            warn!(target: UI, "markup: opening the photo: {e}");
        }
    }

    /// Asks before deleting the selected object.
    fn markup_ask_delete(&mut self) {
        let Some((_, m)) = self.markup_selected() else {
            return;
        };
        let v = item_view(0, &m, &|_| (0.0, 0.0), 1.0);
        let name = if v.detail.is_empty() {
            kind_name(&m.shape)
        } else {
            format!("{} “{}”", kind_name(&m.shape), v.detail)
        };
        self.markup.deleting = Some(m.id.clone());
        self.chrome.confirm_destructive(
            t!("markup-delete-ask", name = name),
            t!("markup-delete-detail"),
            t!("markup-delete-ok"),
            DELETE_ITEM,
        );
        self.redraw = true;
    }

    /// The deletion was confirmed.
    pub(super) fn markup_delete_confirmed(&mut self) {
        let Some(id) = self.markup.deleting.take() else {
            return;
        };
        let page = self.markup_page();
        if self.markup_edit(page, |v| v.retain(|m| m.id != id)) {
            if let Some(t) = self.markup.tool.as_mut() {
                t.selected = None;
            }
            self.chrome.announce_status(t!("markup-deleted"));
        }
    }

    fn markup_undo(&mut self, redo: bool) {
        let Some(l) = self.markup.loaded.as_mut() else {
            return;
        };
        let page = if redo { l.doc.redo() } else { l.doc.undo() };
        if page.is_some() {
            self.markup_save();
            self.chrome.announce_status(if redo {
                t!("markup-redone")
            } else {
                t!("markup-undone")
            });
        }
        self.redraw = true;
    }

    /// The measurement tool's lengths and areas as markup.
    fn markup_from_measure(&mut self) {
        let Some(k) = self.markup_k() else {
            return;
        };
        let items = self.measure_markup();
        if items.is_empty() {
            return;
        }
        let page = self.markup_page();
        let made: Vec<Markup> = items
            .into_iter()
            .map(|(pts, area, value)| {
                let pts: Vec<mk::P> = pts
                    .iter()
                    .map(|p| mk::P::new(p.0 / k.0, p.1 / k.1))
                    .collect();
                let shape = if area {
                    Shape::Area(pts)
                } else {
                    Shape::Length(pts[0], pts[pts.len() - 1])
                };
                let mut m = self.markup_new(shape);
                m.text = value;
                m
            })
            .collect();
        let n = made.len();
        if self.markup_edit(page, |v| v.extend(made)) {
            let now = self.now();
            self.chrome.toast(
                t!("markup-from-measurements-done", count = n as i64),
                None,
                now,
            );
        }
    }

    /// Clouds around `rects` (stored pixels of the current page), from sheet compare.
    pub(super) fn markup_add_clouds(&mut self, rects: &[[f64; 4]], text: &str) -> usize {
        self.markup_sync();
        let Some(k) = self.markup_k() else {
            return 0;
        };
        if self.markup.loaded.is_none() {
            return 0;
        }
        let page = self.markup_page();
        let made: Vec<Markup> = rects
            .iter()
            .map(|r| {
                let (x0, y0, x1, y1) = (r[0] / k.0, r[1] / k.1, r[2] / k.0, r[3] / k.1);
                let mut m = self.markup_new(Shape::Cloud(vec![
                    mk::P::new(x0, y0),
                    mk::P::new(x1, y0),
                    mk::P::new(x1, y1),
                    mk::P::new(x0, y1),
                ]));
                m.text = text.to_owned();
                m
            })
            .collect();
        let n = made.len();
        self.markup_edit(page, |v| v.extend(made));
        n
    }

    /// Saves the marked-up sheet as a PDF (the original with the markup as annotations) or a
    /// PNG of the current page.
    fn markup_export(&mut self, png: bool) {
        self.markup_sync();
        let Some(l) = self.markup.loaded.as_ref() else {
            return;
        };
        let Some((path, page, session)) = self.markup_current() else {
            return;
        };
        let space = l.space;
        // A PDF exports every marked page; anything else its current page as one.
        let mut doc = Doc::new(space);
        if space == Space::PdfPage && !png {
            for (p, v) in l.doc.pages() {
                let v = v.to_vec();
                doc.edit(p, |x| *x = v);
            }
        } else {
            let v = l.doc.page(page).to_vec();
            doc.edit(if space == Space::PdfPage { page } else { 0 }, |x| *x = v);
        }
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "sheet".to_owned());
        let dir = path.parent().map(Path::to_path_buf);
        let tx = self.markup.tx.clone();
        let size = (session.info().width, session.info().height);
        self.loader.background("markup-export", move |l| {
            let (ext, filter) = if png { ("png", "PNG") } else { ("pdf", "PDF") };
            let mut dlg = rfd::FileDialog::new()
                .add_filter(filter, &[ext])
                .set_file_name(format!("{stem}-markup.{ext}"));
            if let Some(d) = dir {
                dlg = dlg.set_directory(d);
            }
            let Some(out) = dlg.save_file() else { return };
            if out == path {
                let _ = tx.send(Done::Exported(Err(t!("markup-export-same-file"))));
                l.waker()();
                return;
            }
            let r = export_bytes(&path, &*session, space, page, &doc, png, size)
                .and_then(|b| std::fs::write(&out, b).map_err(|e| crate::error_text::io_reason(&e)))
                .map(|()| out);
            let _ = tx.send(Done::Exported(r));
            l.waker()();
        });
    }

    pub(super) fn markup_paste(&mut self, s: &str) {
        if let Some(p) = self.markup.tool.as_mut().and_then(|t| t.prompt.as_mut()) {
            p.paste(s);
            self.redraw = true;
        }
    }

    fn markup_hit(&mut self, h: Hit) {
        let n = self.markup_view().map_or(0, |v| v.items.len());
        match h {
            Hit::Tool(k) => self.markup_set_kind(k),
            Hit::Colour | Hit::Width | Hit::StampKind => {
                let mut said = String::new();
                if let Some(t) = self.markup.tool.as_mut() {
                    match h {
                        Hit::Colour => {
                            t.colour = (t.colour + 1) % COLOURS.len();
                            said = tr(COLOURS[t.colour].1);
                        }
                        Hit::Width => {
                            t.width = (t.width + 1) % WIDTHS.len();
                            said = tr(WIDTHS[t.width].1);
                        }
                        _ => {
                            let i = Stamp::ALL.iter().position(|s| *s == t.stamp).unwrap_or(0);
                            t.stamp = Stamp::ALL[(i + 1) % Stamp::ALL.len()];
                            said = stamp_label(t.stamp);
                        }
                    }
                }
                // The new colour or width also goes to the selected object.
                if matches!(h, Hit::Colour | Hit::Width)
                    && let Some((page, m)) = self.markup_selected()
                {
                    let fresh = self.markup_new(m.shape.clone());
                    self.markup_edit(page, |v| {
                        if let Some(x) = v.iter_mut().find(|x| x.id == m.id) {
                            if h == Hit::Colour {
                                x.colour = fresh.colour;
                            } else {
                                x.width = fresh.width;
                            }
                        }
                    });
                }
                self.chrome.announce_status(said);
            }
            Hit::Undo => self.markup_undo(false),
            Hit::Redo => self.markup_undo(true),
            Hit::Delete => self.markup_ask_delete(),
            Hit::OpenPhoto => self.markup_open_photo(),
            Hit::FromMeasure => self.markup_from_measure(),
            Hit::ExportPdf => self.markup_export(false),
            Hit::ExportPng => self.markup_export(true),
            Hit::Done => self.markup_close(),
            Hit::Item(i) if i < n => {
                let id = self
                    .markup
                    .loaded
                    .as_ref()
                    .and_then(|l| l.doc.page(self.markup_page()).get(i))
                    .map(|m| m.id.clone());
                if let Some(t) = self.markup.tool.as_mut() {
                    t.selected = id;
                }
                if let Some(v) = self.markup_view()
                    && let Some(it) = v.items.get(i)
                {
                    self.chrome
                        .announce_status(format!("{}: {}", it.title, it.detail));
                }
            }
            Hit::Item(_) => {}
        }
        self.redraw = true;
    }

    /// Places a point of the shape being drawn.
    fn markup_place(&mut self, p: mk::P) {
        let Some(t) = self.markup.tool.as_mut() else {
            return;
        };
        if t.prompt.is_some() || t.kind == Kind::Select {
            return;
        }
        t.pts.push(p);
        if t.kind.points() == Some(t.pts.len()) {
            self.markup_finish();
        }
        self.redraw = true;
    }

    fn markup_nudge(&mut self, dx: f32, dy: f32, step: f32) {
        let Some(k) = self.markup_k() else { return };
        let s = self.dl.scale.max(0.1);
        let a = self.view.stored_from_screen();
        // One logical pixel in sheet units.
        let o = a.apply((0.0, 0.0));
        let e = a.apply((s, 0.0));
        let unit = f64::from((e.0 - o.0).hypot(e.1 - o.1)) / k.0;
        let (w, h) = self.view.stored_size();
        let centre = mk::P::new(f64::from(w) / 2.0 / k.0, f64::from(h) / 2.0 / k.1);
        let Some(t) = self.markup.tool.as_mut() else {
            return;
        };
        let c = t.cursor.or(t.hover).unwrap_or(centre);
        let q = mk::P::new(
            (c.x + f64::from(dx * step) * unit).clamp(0.0, f64::from(w) / k.0),
            (c.y + f64::from(dy * step) * unit).clamp(0.0, f64::from(h) / k.1),
        );
        t.cursor = Some(q);
        if let Some(d) = t.drag.as_mut() {
            d.now = q;
        }
        self.redraw = true;
    }

    pub(super) fn markup_key(
        &mut self,
        combo: Option<&KeyCombo>,
        text: Option<&str>,
        el: &ActiveEventLoop,
    ) -> bool {
        self.markup_sync();
        let Some(v) = self.markup_view() else {
            if let Some(c) = combo
                && c.key == Key::Named(NamedKey::Escape)
            {
                self.markup_close();
            }
            return true;
        };
        // The text field takes the typing.
        if self.markup.prompting() {
            if let Some(c) = combo {
                let plain = !c.mods.ctrl && !c.mods.alt && !c.mods.meta;
                // A text markup's lines: Shift+Enter, Up, Down, and Backspace or Delete across
                // a line break.
                let lines = plain
                    && self
                        .markup
                        .tool
                        .as_mut()
                        .and_then(|t| t.prompt.as_mut())
                        .is_some_and(|p| match c.key {
                            Key::Named(NamedKey::Enter) if c.mods.shift => p.new_line(),
                            Key::Named(NamedKey::ArrowUp) if !c.mods.shift => p.move_line(-1),
                            Key::Named(NamedKey::ArrowDown) if !c.mods.shift => p.move_line(1),
                            Key::Named(NamedKey::Backspace) if !c.mods.shift => p.join(true),
                            Key::Named(NamedKey::Delete) if !c.mods.shift => p.join(false),
                            _ => false,
                        });
                if lines {
                    self.redraw = true;
                    return true;
                }
                match c.key {
                    Key::Named(NamedKey::Enter) => {
                        self.markup_answer();
                        return true;
                    }
                    Key::Named(NamedKey::Escape) => {
                        if let Some(t) = self.markup.tool.as_mut() {
                            t.prompt = None;
                        }
                        self.redraw = true;
                        return true;
                    }
                    _ => {}
                }
                let rtl = self.rtl();
                let mut clip = PendingClipboard::default();
                let used = self
                    .markup
                    .tool
                    .as_mut()
                    .and_then(|t| t.prompt.as_mut())
                    .is_some_and(|p| p.field.key(c.key, c.mods, rtl, &mut clip).used());
                if used {
                    self.field_clipboard(clip, PasteTo::Markup);
                    self.redraw = true;
                    return true;
                }
                if c.mods.ctrl || c.mods.alt || c.mods.meta {
                    return true;
                }
            }
            if let Some(s) = text {
                let s: String = s.chars().filter(|c| !c.is_control()).collect();
                if !s.is_empty()
                    && let Some(p) = self.markup.tool.as_mut().and_then(|t| t.prompt.as_mut())
                {
                    p.field.insert(&s);
                    self.redraw = true;
                }
            }
            return true;
        }
        let Some(combo) = combo else { return true };
        let focusables = v.focusables();
        let n = focusables.len();
        let focus = v.focus.and_then(|i| focusables.get(i).copied());
        let mods = combo.mods;
        let plain = !mods.ctrl && !mods.alt && !mods.meta;
        let move_focus = |me: &mut App, d: i32| {
            if let Some(t) = me.markup.tool.as_mut() {
                let i = t.focus.map_or(if d > 0 { 0 } else { n - 1 }, |i| {
                    (i as i32 + d).rem_euclid(n as i32) as usize
                });
                t.focus = Some(i);
            }
            if let Some(v) = me.markup_view()
                && let Some(h) = v.focus.and_then(|i| v.focusables().get(i).copied())
            {
                me.chrome.announce_status(v.label(h));
            }
        };
        let step = if mods.shift { 10.0 } else { 1.0 };
        match combo.key {
            Key::Named(NamedKey::Escape) => {
                let t = self.markup.tool.as_mut();
                match t {
                    Some(t) if t.drag.is_some() => t.drag = None,
                    Some(t) if !t.pts.is_empty() => t.pts.clear(),
                    Some(t) if t.focus.is_some() => t.focus = None,
                    Some(t) if t.selected.is_some() => t.selected = None,
                    _ => self.markup_close(),
                }
            }
            Key::Named(NamedKey::Tab) if mods.shift => move_focus(self, -1),
            Key::Named(NamedKey::Tab) => move_focus(self, 1),
            Key::Char('z' | 'Z') if mods.ctrl && mods.shift => self.markup_hit(Hit::Redo),
            Key::Char('z' | 'Z') if mods.ctrl => self.markup_hit(Hit::Undo),
            Key::Char('y' | 'Y') if mods.ctrl => self.markup_hit(Hit::Redo),
            Key::Char('e' | 'E') if mods.ctrl && mods.shift => self.markup_hit(Hit::ExportPng),
            Key::Char('e' | 'E') if mods.ctrl => self.markup_hit(Hit::ExportPdf),
            Key::Named(NamedKey::Delete | NamedKey::Backspace) => self.markup_hit(Hit::Delete),
            Key::Named(NamedKey::Enter | NamedKey::Space) if focus.is_some() => {
                if let Some(h) = focus
                    && v.enabled(h)
                {
                    self.markup_hit(h);
                }
            }
            Key::Named(NamedKey::ArrowUp | NamedKey::ArrowDown)
                if matches!(focus, Some(Hit::Item(_))) =>
            {
                let d = if combo.key == Key::Named(NamedKey::ArrowUp) {
                    -1
                } else {
                    1
                };
                if let Some(Hit::Item(i)) = focus {
                    let j = (i as i32 + d).clamp(0, v.items.len() as i32 - 1) as usize;
                    if let Some(k) = focusables.iter().position(|h| *h == Hit::Item(j))
                        && let Some(t) = self.markup.tool.as_mut()
                    {
                        t.focus = Some(k);
                    }
                    self.markup_hit(Hit::Item(j));
                }
            }
            Key::Named(NamedKey::ArrowLeft) => self.markup_nudge(-1.0, 0.0, step),
            Key::Named(NamedKey::ArrowRight) => self.markup_nudge(1.0, 0.0, step),
            Key::Named(NamedKey::ArrowUp) => self.markup_nudge(0.0, -1.0, step),
            Key::Named(NamedKey::ArrowDown) => self.markup_nudge(0.0, 1.0, step),
            Key::Named(NamedKey::Space) => {
                let at = self.markup.tool.as_ref().and_then(|t| t.cursor.or(t.hover));
                match at {
                    Some(p) => self.markup_place(p),
                    None => self.markup_nudge(0.0, 0.0, 0.0),
                }
            }
            Key::Named(NamedKey::Enter) => {
                let (kind, n_pts) = self
                    .markup
                    .tool
                    .as_ref()
                    .map_or((Kind::Select, 0), |t| (t.kind, t.pts.len()));
                if matches!(kind, Kind::Cloud | Kind::Area) && n_pts >= 3 {
                    self.markup_finish();
                } else if let Some((_, m)) = self.markup_selected() {
                    match &m.shape {
                        Shape::Pin { .. } => self.markup_open_photo(),
                        Shape::Text(_)
                        | Shape::Arrow { .. }
                        | Shape::Stamp {
                            kind: Stamp::Custom,
                            ..
                        } => self.markup_ask(Asking::Edit(m.id.clone()), &m.text),
                        _ => {}
                    }
                } else {
                    self.markup_close();
                }
            }
            Key::Char(c) if plain => {
                if let Some(k) = Kind::ALL
                    .iter()
                    .find(|k| k.letter() == c.to_ascii_lowercase())
                {
                    self.markup_set_kind(*k);
                } else if let Some(a) = self.keymap.lookup(combo)
                    && a.group() == Group::ZoomPan
                {
                    self.action(a, el);
                }
            }
            _ => {
                if let Some(a) = self.keymap.lookup(combo)
                    && (a.group() == Group::ZoomPan || a == Action::ToggleFullscreen)
                {
                    self.action(a, el);
                }
            }
        }
        self.redraw = true;
        true
    }

    /// The left button while the tool is open: the bar and the list, else drawing, picking
    /// and moving. `p` is in physical px, `lp` in logical px.
    pub(super) fn markup_button(&mut self, p: (f32, f32), lp: (f32, f32), down: bool) -> bool {
        if self.markup_field_button(lp, down) {
            self.redraw = true;
            return true;
        }
        let Some(t) = self.markup.tool.as_ref() else {
            return false;
        };
        let Some(q) = self.markup_sheet(p) else {
            return true;
        };
        if down {
            if let Some(h) = t
                .drawn
                .hits
                .iter()
                .find(|(r, _)| r.contains(lp))
                .map(|x| x.1)
            {
                if self.markup_view().is_some_and(|v| v.enabled(h)) {
                    self.markup_hit(h);
                }
                return true;
            }
            if t.drawn.bar.contains(lp) || t.drawn.panel.contains(lp) || t.prompt.is_some() {
                return true;
            }
            let kind = t.kind;
            if kind == Kind::Select {
                let v = self.markup_view();
                let hit = v.as_ref().and_then(|v| pick(&v.items, lp));
                let id = hit.and_then(|i| {
                    self.markup
                        .loaded
                        .as_ref()?
                        .doc
                        .page(self.markup_page())
                        .get(i)
                        .map(|m| m.id.clone())
                });
                let pin = hit
                    .and_then(|i| v.as_ref()?.items.get(i).map(|it| it.pin))
                    .unwrap_or(false);
                if let Some(t) = self.markup.tool.as_mut() {
                    let again = id.is_some() && t.selected == id;
                    t.selected = id.clone();
                    t.press = Some(lp);
                    t.focus = None;
                    t.drag = id.is_some().then_some(Drag {
                        from: q,
                        now: q,
                        again,
                    });
                }
                if let Some(i) = hit
                    && let Some(it) = v.as_ref().and_then(|v| v.items.get(i))
                {
                    self.chrome
                        .announce_status(format!("{}: {}", it.title, it.detail));
                }
                if pin {
                    self.chrome.announce_status(t!("markup-pin-hint"));
                }
                self.redraw = true;
                return true;
            }
            // A click on the first corner closes a cloud or an area.
            let close = matches!(kind, Kind::Cloud | Kind::Area) && t.pts.len() >= 3 && {
                let k = self.markup_k().unwrap_or((1.0, 1.0));
                let f = self.markup_screen(t.pts[0], k);
                (f.0 - lp.0).hypot(f.1 - lp.1) <= CLOSE_RADIUS
            };
            if close {
                self.markup_finish();
            } else {
                if let Some(t) = self.markup.tool.as_mut() {
                    t.press = Some(lp);
                    t.cursor = None;
                    t.focus = None;
                }
                self.markup_place(q);
            }
            self.redraw = true;
            return true;
        }
        let Some(start) = self.markup.tool.as_mut().and_then(|t| t.press.take()) else {
            return true;
        };
        let dragged = (start.0 - lp.0).hypot(start.1 - lp.1) > CLICK_SLOP;
        // The end of a move.
        if let Some(d) = self.markup.tool.as_mut().and_then(|t| t.drag.take()) {
            if dragged {
                let id = self.markup.tool.as_ref().and_then(|t| t.selected.clone());
                let page = self.markup_page();
                let (dx, dy) = (q.x - d.from.x, q.y - d.from.y);
                self.markup_edit(page, |v| {
                    if let Some(m) = v.iter_mut().find(|m| Some(&m.id) == id.as_ref()) {
                        m.shape.map(|p| mk::P::new(p.x + dx, p.y + dy));
                    }
                });
                self.chrome.announce_status(t!("markup-moved"));
            } else if d.again
                && self
                    .markup_selected()
                    .is_some_and(|(_, m)| matches!(m.shape, Shape::Pin { .. }))
            {
                self.markup_open_photo();
            }
            self.redraw = true;
            return true;
        }
        let (kind, n_pts) = self
            .markup
            .tool
            .as_ref()
            .map_or((Kind::Select, 0), |t| (t.kind, t.pts.len()));
        if dragged && kind.drags() && n_pts == 1 {
            if kind == Kind::Cloud {
                if let Some(t) = self.markup.tool.as_mut() {
                    t.pts.push(q);
                }
                self.markup_finish();
            } else {
                self.markup_place(q);
            }
        }
        self.redraw = true;
        true
    }

    /// A press in the bar's field puts the caret (Shift extends, two select the word, three
    /// all), the release ends a drag; false when neither.
    fn markup_field_button(&mut self, lp: (f32, f32), down: bool) -> bool {
        let now = self.now();
        let shift = self.mods.shift_key();
        let Some(t) = self.markup.tool.as_mut() else {
            return false;
        };
        let Some(p) = t.prompt.as_mut() else {
            return false;
        };
        if !down {
            let was = p.field.dragging();
            p.field.release();
            return was;
        }
        let Some((i, (_, block, x0))) = t
            .drawn
            .field_lines
            .iter()
            .enumerate()
            .find(|(_, (r, ..))| r.contains(lp))
        else {
            return false;
        };
        p.press(i, block.hit_byte(lp.0 - x0), lp.0, now, shift);
        true
    }

    /// Pointer motion while the tool is open. True while the button is down.
    pub(super) fn markup_motion(&mut self, p: (f32, f32)) -> bool {
        if self.markup.tool.is_none() {
            return false;
        }
        let lp = self.logical(p);
        if let Some(t) = self.markup.tool.as_mut()
            && let Some(pr) = t.prompt.as_mut()
            && pr.field.dragging()
        {
            // The drag selects on the caret's line.
            if let Some((_, block, x0)) = t.drawn.field_lines.get(pr.above.len()) {
                let at = block.hit_byte(lp.0 - x0).min(pr.field.text().len());
                pr.field.drag_to(at);
                self.redraw = true;
            }
            return true;
        }
        let q = self.markup_sheet(p);
        let Some(t) = self.markup.tool.as_mut() else {
            return false;
        };
        t.hover = q;
        if let (Some(d), Some(q)) = (t.drag.as_mut(), q) {
            d.now = q;
            self.redraw = true;
        }
        if !t.pts.is_empty() {
            t.cursor = None;
            self.redraw = true;
        }
        t.press.is_some()
    }

    pub(super) fn markup_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        if self.markup.tool.is_none() || req.target_node.0 & !0xFFFF != A11Y_BASE {
            return false;
        }
        if let Some(v) = self.markup_view()
            && let Some(h) = target(req.target_node.0, &v)
        {
            match req.action {
                accesskit::Action::Click if v.enabled(h) => self.markup_hit(h),
                accesskit::Action::Focus => {
                    if let Some(i) = v.focusables().iter().position(|x| *x == h)
                        && let Some(t) = self.markup.tool.as_mut()
                    {
                        t.focus = Some(i);
                    }
                }
                _ => {}
            }
        }
        self.redraw = true;
        true
    }

    /// A background task panicked: nothing waits for it any more.
    pub(super) fn markup_lost(&mut self, task: &str) {
        if task == "markup-load"
            && let Some(path) = self.markup.loading.take()
            && let Some((cur, _, s)) = self.markup_current()
            && cur == path
        {
            let space = App::markup_space(&*s);
            let size = (s.info().width, s.info().height);
            self.markup.loaded = Some(Loaded {
                path,
                space,
                doc: Doc::new(space),
                transforms: None,
                size,
            });
        }
    }

    /// `--show markup` (screenshots): the tool over the sheet with one of each kind, placed
    /// by the sheet's size. Nothing is saved.
    pub(super) fn markup_show(&mut self, what: &str) -> bool {
        if what != "markup" {
            return false;
        }
        self.markup_open();
        let Some(k) = self.markup_k() else {
            return true;
        };
        if self.markup.loaded.is_none() {
            let Some((path, _, s)) = self.markup_current() else {
                return true;
            };
            let space = App::markup_space(&*s);
            self.markup.loaded = Some(Loaded {
                path,
                space,
                doc: Doc::new(space),
                transforms: None,
                size: (s.info().width, s.info().height),
            });
        }
        let (w, h) = self.view.stored_size();
        let (w, h) = (f64::from(w) / k.0, f64::from(h) / k.1);
        let at = |x: f64, y: f64| mk::P::new(w * x, h * y);
        let page = self.markup_page();
        let mut items = vec![
            self.markup_new(Shape::Cloud(vec![
                at(0.15, 0.2),
                at(0.4, 0.2),
                at(0.4, 0.4),
                at(0.15, 0.4),
            ])),
            self.markup_new(Shape::Arrow {
                tail: at(0.5, 0.15),
                head: at(0.4, 0.3),
            }),
            self.markup_new(Shape::Text(at(0.15, 0.5))),
            self.markup_new(Shape::Stamp {
                at: at(0.7, 0.75),
                kind: Stamp::Approved,
            }),
            self.markup_new(Shape::Length(at(0.15, 0.8), at(0.55, 0.8))),
            self.markup_new(Shape::Pin {
                at: at(0.6, 0.45),
                photo: PathBuf::from("IMG_0042.jpg"),
            }),
        ];
        items[1].text = "Check".into();
        items[2].text = "New door here".into();
        let len = items[4].shape.clone();
        items[4].text = self.markup_value(&len);
        if let Some(l) = self.markup.loaded.as_mut() {
            l.doc.edit(page, |v| v.extend(items));
        }
        self.redraw = true;
        true
    }

    /// The palette's and keymap's markup action.
    pub(super) fn markup_action(&mut self, a: Action) -> bool {
        match a {
            Action::Markup => {
                if self.markup.tool.is_some() {
                    self.markup_close();
                } else {
                    self.markup_open();
                }
            }
            _ => return false,
        }
        true
    }

    /// Markup drawn into a figure (Export figure): each object's outline and its words.
    pub(super) fn markup_for_figure(&self) -> Vec<super::figure::Measured> {
        let Some(v) = self.markup_view() else {
            return Vec::new();
        };
        let Some(k) = self.markup_k() else {
            return Vec::new();
        };
        let Some(l) = self.markup.loaded.as_ref() else {
            return Vec::new();
        };
        let tol = 0.5 / k.0.max(1e-9);
        let _ = v;
        l.doc
            .page(self.markup_page())
            .iter()
            .map(|m| {
                let st = |p: &mk::P| (p.x * k.0, p.y * k.1);
                let (outline, closed): (Vec<(f64, f64)>, bool) = match &m.shape {
                    Shape::Cloud(pts) => {
                        let arcs = mk::cloud::arcs(pts, mk::cloud::arc_length(m.width, m.size));
                        (
                            mk::cloud::flatten(&arcs, tol).iter().map(st).collect(),
                            false,
                        )
                    }
                    Shape::Area(pts) => (pts.iter().map(st).collect(), true),
                    s => {
                        let b = m.bounds(&stamp_words(m));
                        match s {
                            Shape::Arrow { tail, head } => (vec![st(tail), st(head)], false),
                            Shape::Length(a, b) => (vec![st(a), st(b)], false),
                            Shape::Pin { at, .. } => (vec![st(at)], false),
                            _ => (
                                vec![
                                    st(&mk::P::new(b[0], b[1])),
                                    st(&mk::P::new(b[2], b[1])),
                                    st(&mk::P::new(b[2], b[3])),
                                    st(&mk::P::new(b[0], b[3])),
                                ],
                                true,
                            ),
                        }
                    }
                };
                let label = match &m.shape {
                    Shape::Stamp { .. } => stamp_words(m),
                    _ => m.text.replace('\n', " "),
                };
                super::figure::Measured {
                    outline,
                    closed,
                    label: label.clone(),
                    lines: vec![label],
                }
            })
            .collect()
    }
}

/// Sets a line Helvetica cannot write (Hebrew, Arabic, CJK…) as outlines with `text`, the
/// viewer's own text system (shaping, bidi and fallback fonts), so the exported PDF needs no
/// embedded font; origin at the left end of the baseline. `text` is made on first use (it
/// scans the system fonts; up to 5 s for them, as the map export waits).
pub(super) fn outline_text(
    text: &std::cell::RefCell<Option<Text>>,
    s: &str,
    size: f64,
    bold: bool,
) -> Option<mk::pdf::Outlined> {
    let mut slot = text.borrow_mut();
    let t = slot.get_or_insert_with(|| {
        let t = Text::new();
        let t0 = std::time::Instant::now();
        while t.fonts_pending() && t0.elapsed() < std::time::Duration::from_secs(5) {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        t
    });
    let style = TextStyle::new(size as f32).weight(if bold { 750.0 } else { 500.0 });
    let b = t.layout(s, style, None, Align::Start);
    let p = |a: [f32; 2]| mk::P::new(f64::from(a[0]), f64::from(a[1]));
    use mk::pdf::PathEl as E;
    use specular_gfx::PathEl as G;
    let path = t
        .outlines(&b, 0.0, -b.baseline)
        .into_iter()
        .map(|e| match e {
            G::Move(a) => E::Move(p(a)),
            G::Line(a) => E::Line(p(a)),
            G::Quad(c, a) => E::Quad(p(c), p(a)),
            G::Cubic(c1, c2, a) => E::Cubic(p(c1), p(c2), p(a)),
            G::Close => E::Close,
        })
        .collect();
    Some(mk::pdf::Outlined {
        path,
        width: f64::from(b.width),
    })
}

/// The marked-up sheet as PDF or PNG bytes.
fn export_bytes(
    path: &Path,
    session: &dyn specular_decode::Session,
    space: Space,
    page: u32,
    doc: &Doc,
    png: bool,
    size: (u32, u32),
) -> Result<Vec<u8>, String> {
    let reason = |e: specular_pdf::annotate::AnnotateError| match e {
        specular_pdf::annotate::AnnotateError::Encrypted => t!("markup-export-encrypted"),
        specular_pdf::annotate::AnnotateError::Damaged => t!("markup-export-damaged"),
        specular_pdf::annotate::AnnotateError::NoPage(_) => t!("markup-export-damaged"),
    };
    let (base, units, out_page) = match space {
        Space::PdfPage => (
            std::fs::read(path).map_err(|e| crate::error_text::io_reason(&e))?,
            1.0,
            page as usize,
        ),
        Space::Pixels => {
            let cancel = specular_core::CancelToken::new();
            let r = session
                .decode(&specular_decode::DecodeRequest::full(1), &cancel)
                .map_err(|e| e.to_string())?;
            let rgba = crate::clipboard::to_rgba(&r);
            let rgb: Vec<u8> = rgba
                .chunks_exact(4)
                .flat_map(|p| {
                    let a = u32::from(p[3]);
                    let f = |c: u8| ((u32::from(c) * a + 255 * (255 - a)) / 255) as u8;
                    [f(p[0]), f(p[1]), f(p[2])]
                })
                .collect();
            let (pdf, k) = mk::pdf::image_sheet(&rgb, r.width, r.height);
            (pdf, k, 0)
        }
    };
    let text = std::cell::RefCell::new(None);
    let outlines = |s: &str, size: f64, bold: bool| outline_text(&text, s, size, bold);
    let marked =
        mk::pdf::export(&base, doc, units, &stamp_words, Some(&outlines)).map_err(reason)?;
    if !png {
        return Ok(marked);
    }
    let d = specular_pdf::Document::open(Arc::new(marked)).map_err(|e| e.to_string())?;
    let (pw, ph) = d
        .page_size(out_page)
        .ok_or_else(|| t!("markup-export-damaged"))?;
    // The page at the size it is shown, within the renderer's bounds.
    let (mut w, mut h) = (size.0.max(1), size.1.max(1));
    let cap = (specular_pdf::MAX_RENDER_PIXELS as f64 / (f64::from(w) * f64::from(h)))
        .sqrt()
        .min(f64::from(specular_pdf::MAX_RENDER_SIDE) / f64::from(w.max(h)))
        .min(1.0);
    w = ((f64::from(w) * cap) as u32).max(1);
    h = ((f64::from(h) * cap) as u32).max(1);
    let req = specular_pdf::RenderRequest {
        page: out_page,
        scale_x: f64::from(w) / f64::from(pw),
        scale_y: f64::from(h) / f64::from(ph),
        x: 0,
        y: 0,
        width: w,
        height: h,
    };
    let rgba = d.render(req, &|| false).map_err(|e| e.to_string())?;
    let rgb: Vec<u8> = rgba
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    specular_png::encode(w, h, specular_png::ColorType::Rgb, &rgb).map_err(|e| e.to_string())
}
