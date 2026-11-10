//! The print dialog (DESIGN.md §6.5): `Ctrl+P`, or Print… in the palette's File group.
//!
//! A modal sheet drawn like the other dialogs: page 1 (or any page, with the stepper under it)
//! previewed on the left exactly as it will print, the layout on the right, and Save as PDF…
//! and Print… at the bottom. One image per page (fit, fill or an exact width) or a contact
//! sheet of the folder, the gallery's selection or the picks, with captions and a header.
//!
//! The work happens on `specular-print`'s thread (its own GPU renderer on a clone of the
//! viewer's device): the preview is the PDF's raster at screen size, and the PDF streams to
//! disk band by band. Print… writes the PDF to the runtime directory and hands it to the
//! system ([`specular_platform::print`]: the print portal's own dialog, else `lp`).
//!
//! The choices last for the session (no settings); the paper starts from the locale.
//!
//! From Develop the dialog prints the developed photo (rendered at full size on the
//! GPU, `specular_develop::export::print`) and shows two more rows: the colour profile
//! (sRGB, or Develop's soft-proof profile, which the PDF then carries and its pixels are
//! converted to) and the rendering intent.
//!
//! **Free layouts** (print layouts, Phase 2): a page of image areas added, moved and
//! resized by hand ([`editor`]), each photo fitted or filling its area with a crop pan;
//! photos fill the areas in selection order, page after page. Margins and gaps are typed
//! in mm or inches (the locale's unit first). Layouts are saved by name in
//! `print-layouts.txt` in the settings folder, beside the other presets; six are built in.
//! Borderless is offered only when the printer reports it. From Develop the dialog adds
//! black point compensation (on for relative colorimetric), a soft-proofed preview and a
//! style applied to the printed copies only.

mod editor;
#[cfg(test)]
mod editor_shots;
#[cfg(test)]
mod shots;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crossbeam_channel::Receiver;
use specular_gfx::{
    Align, DrawList, Gpu, Icon, Rect, TexId, Text, TextStyle, UiRenderer, draw_icon, wgpu,
};
use specular_i18n::t;
use specular_input::{Key, KeyCombo, Mods, NamedKey};
use specular_print::files::{FileSource, Probed};
use specular_print::free::{FreeLayout, Guide, Placement};
use specular_print::layout::{Caption, Fit, Item, Kind, PageOrientation, PageSetup, Paper, Unit};
use specular_print::{Document, Reply, Worker};
use specular_widgets::line_edit::{LineEdit, PendingClipboard};

use editor::{Drag, EditorView, Grip, SnapMode};
use specular_theme::metrics::{FONT_LABEL, FONT_SMALL, FONT_TITLE, RADIUS_LG, RADIUS_SM};
use specular_theme::{Color, Palette};
use specular_widgets::accesskit;

use super::App;

const A11Y_BASE: u64 = 0x5100_0000_0000_0000;
const PAPERS: [Paper; 3] = [Paper::A4, Paper::Letter, Paper::A3];
const DPIS: [u32; 3] = [150, 300, 600];
const MAX_MARGIN_MM: f64 = 50.0;
/// Scales offered for printing drawings (1:n), after "as drawn".
const RATIOS: [f64; 13] = [
    1.0, 2.0, 5.0, 10.0, 20.0, 25.0, 50.0, 100.0, 200.0, 250.0, 500.0, 1000.0, 2500.0,
];

/// Which images.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Source {
    Current,
    Folder,
    Selection,
    Picks,
}

/// One line of the layout column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Row {
    Images,
    Layout,
    Paper,
    Orientation,
    Margins,
    Size,
    Width,
    Scale,
    Columns,
    Rows,
    Header,
    Captions,
    Units,
    Resolution,
    Copies,
    /// From Develop: the colour profile the PDF is made in.
    Profile,
    /// From Develop, printing through a profile: the rendering intent.
    Intent,
    /// From Develop, relative colorimetric: black point compensation.
    Bpc,
    /// From Develop, printing through a profile: the preview soft-proofed.
    Proof,
    /// From Develop: a style applied to the printed copies.
    Style,
    /// A free layout: the saved layout it is (or a name being typed).
    Saved,
    /// A free layout: print to the paper's edge (only when the printer reports it).
    Borderless,
    /// A free layout: the gap areas snap to.
    Gap,
    /// A free layout: fit or fill the selected area.
    Placement,
    /// A free layout: what edges snap to.
    Snap,
}

impl Row {
    /// Lengths that can be typed.
    fn typed(self) -> bool {
        matches!(self, Row::Margins | Row::Gap | Row::Width)
    }
}

/// What a click or a screen reader hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Hit {
    Dec(Row),
    Inc(Row),
    Row(Row),
    PrevPage,
    NextPage,
    Cancel,
    Save,
    Print,
    /// The value of a length row: starts typing it.
    Type(Row),
    /// The free layout's page (keyboard edits) and an area on it.
    Page,
    Area(usize),
    AddArea,
    RemoveArea,
    SaveLayout,
    DeleteLayout,
    /// Deleting a saved layout, asked first.
    ConfirmDelete,
    KeepLayout,
}

/// The dialog's choices, kept for the session.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Choices {
    pub sheet: bool,
    /// A free layout (its areas are the print state's).
    pub free: bool,
    /// The unit a free layout's lengths show and are typed in (the locale's first).
    pub free_unit: Unit,
    pub setup: PageSetup,
    pub unit: Unit,
    pub fit: Fit,
    /// The width of an exact size (mm), kept while fit or fill is chosen.
    pub width: f64,
    /// The scale of printing to scale (1:n), `None` as drawn; kept like the width.
    pub ratio: Option<f64>,
    pub single_caption: Caption,
    pub cols: u32,
    pub rows: u32,
    pub header: bool,
    pub sheet_caption: Caption,
    pub copies: u32,
}

impl Choices {
    fn for_locale() -> Choices {
        let locale = ["LC_PAPER", "LC_ALL", "LANG"]
            .iter()
            .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
            .unwrap_or_default();
        let paper = Paper::for_locale(&locale);
        Choices {
            sheet: false,
            free: false,
            free_unit: Unit::for_locale(&locale),
            setup: PageSetup {
                paper,
                ..PageSetup::default()
            },
            unit: Unit::for_paper(paper),
            fit: Fit::Fit,
            width: 100.0,
            ratio: None,
            single_caption: Caption::Off,
            cols: 4,
            rows: 6,
            header: true,
            sheet_caption: Caption::Name,
            copies: 1,
        }
    }

    pub(super) fn kind(&self) -> Kind {
        if self.sheet {
            Kind::Sheet {
                cols: self.cols,
                rows: self.rows,
                caption: self.sheet_caption,
                header: self.header,
            }
        } else {
            Kind::Single {
                fit: self.fit,
                caption: self.single_caption,
            }
        }
    }

    /// The rows shown for these choices.
    pub(super) fn rows(&self) -> Vec<Row> {
        if self.free {
            // The print state drops Borderless and Placement when they do not apply.
            return vec![
                Row::Images,
                Row::Layout,
                Row::Saved,
                Row::Paper,
                Row::Orientation,
                Row::Borderless,
                Row::Margins,
                Row::Gap,
                Row::Placement,
                Row::Snap,
                Row::Units,
                Row::Resolution,
                Row::Copies,
            ];
        }
        let mut v = vec![
            Row::Images,
            Row::Layout,
            Row::Paper,
            Row::Orientation,
            Row::Margins,
        ];
        if self.sheet {
            v.extend([Row::Columns, Row::Rows, Row::Header]);
        } else {
            v.push(Row::Size);
            match self.fit {
                Fit::Size { .. } => v.push(Row::Width),
                Fit::Scale { .. } => v.push(Row::Scale),
                _ => {}
            }
        }
        v.extend([Row::Captions, Row::Units, Row::Resolution, Row::Copies]);
        v
    }

    /// Steps `row` by `d` (±1); false when it is at its end.
    fn step(&mut self, row: Row, d: i32) -> bool {
        let before = *self;
        let cycle = |i: usize, n: usize| (i as i32 + d).rem_euclid(n as i32) as usize;
        let s = &mut self.setup;
        match row {
            Row::Images => {}
            Row::Layout => {
                // One per page → contact sheet → free layout.
                let i = if self.free {
                    2
                } else {
                    usize::from(self.sheet)
                };
                let j = cycle(i, 3);
                self.sheet = j == 1;
                self.free = j == 2;
            }
            Row::Paper => {
                let i = PAPERS.iter().position(|p| *p == s.paper).unwrap_or(0);
                let old = s.paper;
                s.paper = PAPERS[cycle(i, PAPERS.len())];
                // The unit follows the paper unless it was changed by hand.
                if self.unit == Unit::for_paper(old) {
                    self.unit = Unit::for_paper(s.paper);
                }
            }
            Row::Orientation => {
                s.orientation = match s.orientation {
                    PageOrientation::Portrait => PageOrientation::Landscape,
                    PageOrientation::Landscape => PageOrientation::Portrait,
                }
            }
            Row::Margins => {
                let unit = if self.free { self.free_unit } else { self.unit };
                let m = unit.snap(s.margin + f64::from(d) * unit.step_mm());
                s.margin = m.clamp(0.0, MAX_MARGIN_MM);
            }
            Row::Size => {
                let all = [
                    Fit::Fit,
                    Fit::Fill,
                    Fit::Size { width: self.width },
                    Fit::Scale { ratio: self.ratio },
                ];
                let i = match self.fit {
                    Fit::Fit => 0,
                    Fit::Fill => 1,
                    Fit::Size { .. } => 2,
                    Fit::Scale { .. } => 3,
                };
                self.fit = all[cycle(i, all.len())];
            }
            Row::Width => {
                let max = s.printable().w.max(10.0);
                let w = self
                    .unit
                    .snap(self.width + f64::from(d) * self.unit.step_mm());
                self.width = w.clamp(10.0, max);
                self.fit = Fit::Size { width: self.width };
            }
            Row::Scale => {
                // As drawn, then the usual scales.
                let i = self
                    .ratio
                    .and_then(|r| RATIOS.iter().position(|v| *v == r))
                    .map_or(0, |i| i + 1);
                let j = cycle(i, RATIOS.len() + 1);
                self.ratio = j.checked_sub(1).map(|j| RATIOS[j]);
                self.fit = Fit::Scale { ratio: self.ratio };
            }
            Row::Columns => self.cols = (self.cols as i32 + d).clamp(1, 10) as u32,
            Row::Rows => self.rows = (self.rows as i32 + d).clamp(1, 12) as u32,
            Row::Header => self.header = !self.header,
            Row::Captions => {
                let all = [Caption::Off, Caption::Name, Caption::NameDate];
                let c = if self.sheet {
                    &mut self.sheet_caption
                } else {
                    &mut self.single_caption
                };
                let i = all.iter().position(|x| x == c).unwrap_or(0);
                *c = all[cycle(i, 3)];
            }
            Row::Units if self.free => self.free_unit = self.free_unit.next(d),
            Row::Units => {
                self.unit = match self.unit {
                    Unit::Cm | Unit::Mm => Unit::In,
                    Unit::In => Unit::Cm,
                }
            }
            Row::Resolution => {
                let i = DPIS.iter().position(|v| *v == s.dpi).unwrap_or(1);
                s.dpi = DPIS[cycle(i, DPIS.len())];
            }
            Row::Copies => self.copies = (self.copies as i32 + d).clamp(1, 99) as u32,
            // The dialog's own (Develop's print, the free layout), not choices.
            Row::Profile
            | Row::Intent
            | Row::Bpc
            | Row::Proof
            | Row::Style
            | Row::Saved
            | Row::Borderless
            | Row::Gap
            | Row::Placement
            | Row::Snap => {}
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
    /// Being typed: `value` is the text, this the caret (a byte index into it).
    pub editing: Option<usize>,
}

/// Everything the dialog draws (so screenshots need no app).
#[derive(Clone, Debug)]
pub(super) struct DialogView {
    pub rows: Vec<RowView>,
    /// `(page, pages)`, 1-based; `pages` 0 while the images are being read.
    pub page: (usize, usize),
    /// The page's width over its height.
    pub aspect: f32,
    pub preview: Option<TexId>,
    /// Over the preview: "Preparing…", or the progress of a PDF being written.
    pub status: Option<String>,
    pub progress: Option<f32>,
    pub busy: bool,
    pub can_print: bool,
    /// Index into [`focusables`].
    pub focus: Option<usize>,
    /// A free layout's editor over the preview.
    pub editor: Option<EditorView>,
}

impl DialogView {
    /// The keyboard focus order, as the dialog reads: (a free layout) the toolbar's buttons
    /// that can be used and the page, then the rows, then Cancel, Save as PDF, Print.
    pub fn focusables(&self) -> Vec<Hit> {
        let editor = self.editor.iter().flat_map(|e| {
            e.tools
                .iter()
                .filter(|t| t.2)
                .map(|t| t.0)
                .chain(std::iter::once(Hit::Page))
                .collect::<Vec<_>>()
        });
        editor
            .chain(self.rows.iter().map(|r| Hit::Row(r.row)))
            .chain([Hit::Cancel, Hit::Save, Hit::Print])
            .collect()
    }
}

/// Where things landed.
#[derive(Clone, Debug, Default)]
pub(super) struct Drawn {
    pub sheet: Rect,
    /// The page within the preview area (logical px).
    pub page: Rect,
    pub hits: Vec<(Rect, Hit)>,
    pub rows: Vec<(Rect, Row)>,
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
    let w = (ww - 32.0).clamp(200.0, 880.0);
    let h = (wh - 32.0).clamp(200.0, 600.0);
    let sheet = Rect::new(((ww - w) * 0.5).round(), ((wh - h) * 0.5).round(), w, h);
    dl.shadow(sheet, RADIUS_LG, pal.shadow, 40.0, 14.0);
    dl.rect_border(sheet, RADIUS_LG, pal.surface, pal.glass_border, 1.0);
    let m = |r: Rect| if rtl { r.mirror_x(ww) } else { r };
    // A text block at `x` (start edge, LTR coordinates).
    let start_x = |x: f32, bw: f32| if rtl { ww - x - bw } else { x };

    let pad = 20.0;
    let title_st = TextStyle::new(FONT_TITLE * ts).weight(650.0);
    let label_st = TextStyle::new(FONT_LABEL * ts);
    let value_st = TextStyle::new(FONT_LABEL * ts).weight(550.0);
    let small_st = TextStyle::new(FONT_SMALL * ts);
    let button_st = TextStyle::new(FONT_LABEL * ts).weight(550.0);

    // The controls column at the end, the preview at the start.
    let col_w = (w * 0.42).clamp(220.0, 340.0);
    let col_x = sheet.right() - pad - col_w;
    let title = text.layout(&t!("print-title"), title_st, None, Align::Start);
    text.draw(
        dl,
        &title,
        start_x(col_x, title.width),
        sheet.y + pad,
        pal.text,
    );
    let bh = 32.0;
    let by = sheet.bottom() - pad - bh;
    let rows_top = sheet.y + pad + title.height + 12.0;
    let rows_h = by - 14.0 - rows_top;
    let row_h = (rows_h / v.rows.len().max(1) as f32).clamp(22.0, 34.0);
    let focus = v.focus.map(|i| v.focusables()[i]);
    let mut drawn = Drawn {
        sheet,
        ..Drawn::default()
    };
    let arrow = (row_h - 4.0).min(26.0);
    for (i, rv) in v.rows.iter().enumerate() {
        let r = Rect::new(col_x, rows_top + i as f32 * row_h, col_w, row_h);
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
        // ‹ value ›, at the end of the row.
        let sw = (col_w * 0.56).min(190.0);
        let sx = r.right() - sw;
        let dec = Rect::new(sx, r.y + (row_h - arrow) * 0.5, arrow, arrow);
        let inc = Rect::new(r.right() - arrow, dec.y, arrow, arrow);
        for (b, icon, hit) in [
            (dec, Icon::ChevronLeft, Hit::Dec(rv.row)),
            (inc, Icon::ChevronRight, Hit::Inc(rv.row)),
        ] {
            let b = m(b);
            dl.rect(b, RADIUS_SM, pal.surface_raised);
            let c = if v.busy { pal.text_faint } else { pal.text };
            draw_icon(dl, icon, b.inset(arrow * 0.18), c, 1.6, rtl);
            drawn.hits.push((b, hit));
        }
        let val = text.layout(&rv.value, value_st, None, Align::Start);
        let field = Rect::new(
            sx + arrow + 2.0,
            r.y + 2.0,
            sw - 2.0 * arrow - 4.0,
            row_h - 4.0,
        );
        if let Some(caret) = rv.editing {
            // A field: the typed text from its start, the caret after what precedes it.
            let fm = m(field);
            dl.rect_border(fm, RADIUS_SM, pal.surface, pal.focus_ring, 1.5);
            dl.push_clip(fm);
            let tx = start_x(field.x + 6.0, val.width);
            text.draw(dl, &val, tx, r.y + (row_h - val.height) * 0.5, pal.text);
            let before = text.layout(
                &rv.value[..caret.min(rv.value.len())],
                value_st,
                None,
                Align::Start,
            );
            let cx = if rtl {
                tx + val.width - before.width
            } else {
                tx + before.width
            };
            dl.rect(Rect::new(cx, r.y + 5.0, 1.5, row_h - 10.0), 0.0, pal.text);
            dl.pop_clip();
        } else {
            let mid = sx + arrow + (sw - 2.0 * arrow - val.width.min(sw - 2.0 * arrow)) * 0.5;
            dl.push_clip(m(Rect::new(sx + arrow, r.y, sw - 2.0 * arrow, row_h)));
            text.draw(
                dl,
                &val,
                start_x(mid, val.width),
                r.y + (row_h - val.height) * 0.5,
                pal.text,
            );
            dl.pop_clip();
        }
        if rv.row.typed() && !v.busy {
            drawn.hits.push((m(field), Hit::Type(rv.row)));
        }
        drawn.hits.push((m(r), Hit::Row(rv.row)));
        drawn.rows.push((m(r), rv.row));
    }

    // Buttons: Cancel, Save as PDF…, Print… (the default: filled).
    let labels = [
        (Hit::Print, t!("print-print")),
        (Hit::Save, t!("print-save-pdf")),
        (Hit::Cancel, t!("app-cancel")),
    ];
    let mut x = sheet.right() - pad;
    let default = if v.can_print { Hit::Print } else { Hit::Save };
    for (hit, label) in labels {
        let b = text.layout(&label, button_st, None, Align::Start);
        let bw = (b.width + 28.0).max(84.0);
        x -= bw;
        let r = Rect::new(x, by, bw, bh);
        x -= 8.0;
        let enabled = match hit {
            Hit::Cancel => true,
            Hit::Print => v.can_print && !v.busy && v.page.1 > 0,
            _ => !v.busy && v.page.1 > 0,
        };
        let rm = m(r);
        let (fill, ink) = if hit == default {
            (pal.accent_fill, pal.accent_text)
        } else {
            (pal.surface_raised, pal.text)
        };
        let fill = if enabled { fill } else { fill.with_alpha(0.45) };
        let ink = if enabled { ink } else { ink.with_alpha(0.5) };
        if hit == default {
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

    // The preview: the page on a darker well, with the page stepper under it.
    let well = Rect::new(
        sheet.x + pad,
        sheet.y + pad,
        col_x - 24.0 - sheet.x - pad,
        sheet.h - 2.0 * pad,
    );
    let wellm = m(well);
    dl.rect(wellm, RADIUS_SM + 2.0, pal.sidebar);
    let step_h = 30.0;
    // A free layout: its toolbar over the page, its notice under it.
    let tool_h = if v.editor.is_some() {
        editor::TOOLBAR_H + 8.0
    } else {
        0.0
    };
    let note_h = if v.editor.as_ref().is_some_and(|e| e.notice.is_some()) {
        20.0
    } else {
        0.0
    };
    if let Some(e) = &v.editor {
        let bar = Rect::new(
            well.x + 12.0,
            well.y + 10.0,
            well.w - 24.0,
            editor::TOOLBAR_H,
        );
        let hits = editor::draw_toolbar(dl, text, pal, e, bar, &m, focus, ts);
        drawn.hits.extend(hits);
    }
    let area = Rect::new(
        well.x + 16.0,
        well.y + 16.0 + tool_h,
        well.w - 32.0,
        well.h - 32.0 - step_h - tool_h - note_h,
    );
    let (pw, ph) = if area.w / area.h > v.aspect {
        (area.h * v.aspect, area.h)
    } else {
        (area.w, area.w / v.aspect)
    };
    let page = m(Rect::new(
        (area.x + (area.w - pw) * 0.5).round(),
        (area.y + (area.h - ph) * 0.5).round(),
        pw.round(),
        ph.round(),
    ));
    drawn.page = page;
    dl.shadow(page, 0.0, pal.shadow, 10.0, 3.0);
    dl.rect(page, 0.0, Color::WHITE);
    if let Some(tex) = v.preview {
        dl.image(page, [0.0, 0.0, 1.0, 1.0], tex, 0.0);
    }
    if let Some(e) = &v.editor {
        editor::draw_overlay(dl, text, pal, e, page, focus == Some(Hit::Page), ts);
        drawn.hits.push((page, Hit::Page));
        if let Some(n) = &e.notice {
            let b = text.layout(n, small_st, Some(well.w - 32.0), Align::Center);
            text.draw(
                dl,
                &b,
                wellm.x + (wellm.w - b.width) * 0.5,
                page.bottom() + 6.0,
                pal.warning,
            );
        }
    }
    if let Some(s) = &v.status {
        let b = text.layout(s, small_st, Some(page.w - 24.0), Align::Center);
        let bx = Rect::new(
            page.x + (page.w - b.width) * 0.5 - 10.0,
            page.y + (page.h - b.height) * 0.5 - 8.0,
            b.width + 20.0,
            b.height + 16.0 + if v.progress.is_some() { 10.0 } else { 0.0 },
        );
        dl.rect(bx, RADIUS_SM, Color::rgba(0.0, 0.0, 0.0, 0.72));
        text.draw(dl, &b, bx.x + 10.0, bx.y + 8.0, Color::WHITE);
        if let Some(p) = v.progress {
            let bar = Rect::new(bx.x + 10.0, bx.bottom() - 10.0, bx.w - 20.0, 3.0);
            dl.rect(bar, 1.5, Color::WHITE.with_alpha(0.25));
            dl.rect(
                Rect::new(bar.x, bar.y, bar.w * p.clamp(0.0, 1.0), bar.h),
                1.5,
                Color::WHITE,
            );
        }
    }
    // ‹ Page 1 of 3 ›
    let sy = well.bottom() - 12.0 - step_h + 4.0;
    let label = if v.page.1 == 0 {
        String::new()
    } else {
        t!(
            "print-page",
            page = v.page.0 as i64,
            pages = v.page.1 as i64
        )
    };
    let lb = text.layout(&label, label_st, None, Align::Start);
    let cx = well.x + well.w * 0.5;
    let a = 26.0;
    let prev = Rect::new(cx - lb.width * 0.5 - 8.0 - a, sy, a, a);
    let next = Rect::new(cx + lb.width * 0.5 + 8.0, sy, a, a);
    text.draw(
        dl,
        &lb,
        start_x(cx - lb.width * 0.5, lb.width),
        sy + (a - lb.height) * 0.5,
        pal.text_muted,
    );
    if v.page.1 > 1 {
        for (r, icon, hit, on) in [
            (prev, Icon::ChevronLeft, Hit::PrevPage, v.page.0 > 1),
            (next, Icon::ChevronRight, Hit::NextPage, v.page.0 < v.page.1),
        ] {
            let r = m(r);
            let c = if on { pal.text } else { pal.text_faint };
            draw_icon(dl, icon, r.inset(5.0), c, 1.6, rtl);
            drawn.hits.push((r, hit));
        }
    }
    drawn
}

/// The dialog's AccessKit nodes: a modal dialog with a spin button per row, the page stepper
/// and the buttons.
pub(super) fn a11y(tree: &mut accesskit::TreeUpdate, v: &DialogView, d: &Drawn, scale: f64) {
    use accesskit::{Node, NodeId, Role};
    let bounds = |r: Rect| accesskit::Rect {
        x0: f64::from(r.x) * scale,
        y0: f64::from(r.y) * scale,
        x1: f64::from(r.x + r.w) * scale,
        y1: f64::from(r.y + r.h) * scale,
    };
    let mut dialog = Node::new(Role::Dialog);
    dialog.set_modal();
    dialog.set_label(t!("print-title"));
    dialog.set_bounds(bounds(d.sheet));
    let mut ids = Vec::new();
    let mut nodes = Vec::new();
    let focusables = v.focusables();
    let focus_hit = v.focus.map(|i| focusables[i]);
    let mut focus_id = None;
    for (i, rv) in v.rows.iter().enumerate() {
        let id = NodeId(A11Y_BASE + 1 + i as u64);
        let mut n = Node::new(Role::SpinButton);
        n.set_label(rv.label.clone());
        n.set_value(rv.value.clone());
        if let Some((r, _)) = d.rows.iter().find(|(_, row)| *row == rv.row) {
            n.set_bounds(bounds(*r));
        }
        if !v.busy {
            n.add_action(accesskit::Action::Increment);
            n.add_action(accesskit::Action::Decrement);
        }
        n.add_action(accesskit::Action::Focus);
        if focus_hit == Some(Hit::Row(rv.row)) {
            focus_id = Some(id);
        }
        ids.push(id);
        nodes.push((id, n));
    }
    let page_label = if v.page.1 > 0 {
        t!(
            "print-page",
            page = v.page.0 as i64,
            pages = v.page.1 as i64
        )
    } else {
        t!("print-preparing")
    };
    let id = NodeId(A11Y_BASE + 0x100);
    let mut page = Node::new(if v.editor.is_some() {
        Role::Group
    } else {
        Role::Image
    });
    page.set_bounds(bounds(d.page));
    if let Some(e) = &v.editor {
        page.set_label(format!("{page_label}. {}", t!("print-editor")));
        // Focused by the keyboard; its areas are what a screen reader operates.
        if focus_hit == Some(Hit::Page) {
            focus_id = Some(id);
        }
        let mut kids = Vec::new();
        // In the order photos fill them (reading order).
        let mut order: Vec<usize> = (0..e.areas.len()).collect();
        order.sort_by_key(|&k| e.areas[k].n);
        for k in order {
            let a = &e.areas[k];
            let aid = NodeId(A11Y_BASE + 0x200 + k as u64);
            let mut n = Node::new(Role::Button);
            n.set_label(a.name.clone());
            n.set_bounds(bounds(editor::to_px(d.page, e.sheet, &a.rect)));
            n.add_action(accesskit::Action::Click);
            if a.selected {
                n.set_selected(true);
            }
            kids.push(aid);
            nodes.push((aid, n));
        }
        page.set_children(kids);
        for (k, (hit, label, enabled)) in e.tools.iter().enumerate() {
            let tid = NodeId(A11Y_BASE + 0x180 + k as u64);
            let mut n = Node::new(Role::Button);
            n.set_label(label.clone());
            if let Some((r, _)) = d.hits.iter().find(|(_, h)| h == hit) {
                n.set_bounds(bounds(*r));
            }
            if *enabled {
                n.add_action(accesskit::Action::Click);
                n.add_action(accesskit::Action::Focus);
            } else {
                n.set_disabled();
            }
            if focus_hit == Some(*hit) {
                focus_id = Some(tid);
            }
            ids.push(tid);
            nodes.push((tid, n));
        }
    } else {
        page.set_label(page_label);
    }
    ids.push(id);
    nodes.push((id, page));
    for (k, (r, hit)) in buttons(d).into_iter().enumerate() {
        let id = NodeId(A11Y_BASE + 0x101 + k as u64);
        let mut n = Node::new(Role::Button);
        n.set_label(match hit {
            Hit::PrevPage => t!("print-previous-page"),
            Hit::NextPage => t!("print-next-page"),
            Hit::Cancel => t!("app-cancel"),
            Hit::Save => t!("print-save-pdf"),
            _ => t!("print-print"),
        });
        n.set_bounds(bounds(r));
        n.add_action(accesskit::Action::Click);
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
        let id = NodeId(A11Y_BASE + 0x1ff);
        ids.push(id);
        nodes.push((id, n));
    }
    // The preview column (a free layout's toolbar, then its page) reads before the rows.
    if v.editor.is_some() {
        let rank = |id: &NodeId| match id.0 - A11Y_BASE {
            0x180..0x200 => 0,
            0x100 => 1,
            _ => 2,
        };
        ids.sort_by_key(rank);
    }
    dialog.set_children(ids);
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        root.push_child(NodeId(A11Y_BASE));
    }
    tree.nodes.push((NodeId(A11Y_BASE), dialog));
    tree.nodes.extend(nodes);
    tree.focus = focus_id.unwrap_or(NodeId(A11Y_BASE));
}

/// The node `id` stands for: a row's spin button, the free layout's page, an area or a
/// toolbar button, or a button.
fn a11y_target(v: &DialogView, d: &Drawn, id: u64) -> Option<Hit> {
    let k = id.checked_sub(A11Y_BASE)?;
    if (1..=v.rows.len() as u64).contains(&k) {
        return Some(Hit::Row(v.rows[k as usize - 1].row));
    }
    if let Some(e) = &v.editor {
        if k == 0x100 {
            return Some(Hit::Page);
        }
        if (0x180..0x200).contains(&k) {
            return e.tools.get((k - 0x180) as usize).map(|t| t.0);
        }
        if (0x200..0x200 + e.areas.len() as u64).contains(&k) {
            return Some(Hit::Area((k - 0x200) as usize));
        }
    }
    let k = k.checked_sub(0x101)? as usize;
    buttons(d).get(k).map(|b| b.1)
}

/// The page buttons and the dialog's buttons as drawn, in reading order: the page buttons
/// under the preview, then Cancel, Save as PDF and Print (as Tab visits them).
fn buttons(d: &Drawn) -> Vec<(Rect, Hit)> {
    let order = [
        Hit::PrevPage,
        Hit::NextPage,
        Hit::Cancel,
        Hit::Save,
        Hit::Print,
    ];
    order
        .iter()
        .filter_map(|o| d.hits.iter().find(|(_, h)| h == o).copied())
        .collect()
}

/// What the PDF being written is for.
#[derive(Clone, Debug)]
enum Purpose {
    Save(PathBuf),
    Print(PathBuf),
}

struct Busy {
    job: u64,
    purpose: Purpose,
    done: usize,
    total: usize,
    /// [`super::gpu_reset::gpu_resets`] when it started, and whether it is the second try
    /// after a GPU reset.
    since: u64,
    retried: bool,
}

/// `(document revision, page, long side in px)` of a preview.
type PreviewKey = (u64, usize, u32);

/// The preview texture.
struct Preview {
    _tex: wgpu::Texture,
    id: TexId,
    /// `(rev, page, long_px)` it was asked for.
    key: PreviewKey,
}

/// The open dialog.
pub(super) struct Dialog {
    sources: Vec<(Source, Vec<PathBuf>)>,
    source: usize,
    title: String,
    probes: HashMap<Source, Vec<Probed>>,
    probing: Option<(Source, Receiver<Vec<Probed>>)>,
    doc: Option<Arc<Document>>,
    /// Bumped with every new document.
    rev: u64,
    /// The UI language's generation the document was laid out in.
    language: u64,
    page: usize,
    preview: Option<Preview>,
    /// A preview that arrived and waits for the next frame's upload.
    pending: Option<(PreviewKey, u32, u32, Vec<u8>)>,
    /// `(rev, page, long_px)` last asked for, and the job.
    asked: Option<(PreviewKey, u64)>,
    busy: Option<Busy>,
    can_print: bool,
    focus: Option<usize>,
    drawn: Drawn,
    /// Where the next Save as PDF goes (from the file dialog).
    save_to: Option<Receiver<PathBuf>>,
    /// Printing Develop's photos.
    develop: Option<DevelopPrint>,
    /// The images of the document laid out (sizes for the editor's resolutions).
    items: Vec<Item>,
    /// A free layout: the selected area, a drag in progress and the lines it snapped to.
    selected: Option<usize>,
    drag: Option<Drag>,
    guides: Vec<Guide>,
    /// A length or a layout name being typed.
    typing: Option<(Row, LineEdit)>,
    /// Deleting the saved layout is being asked.
    confirm: bool,
}

/// Printing the photo open in Develop: its edit, and how its colours go to the printer.
pub(super) struct DevelopPrint {
    pub path: PathBuf,
    pub edit: specular_develop::model::Edit,
    /// The developed photo's size (its aspect lays the page out).
    pub size: (u32, u32),
    /// The soft-proof profile: its name and bytes.
    pub profile: Option<(String, Arc<Vec<u8>>)>,
    /// Print through the profile (else sRGB).
    pub through: bool,
    /// Relative colorimetric (else perceptual).
    pub relative: bool,
    /// Black point compensation with relative colorimetric (on by default).
    pub bpc: bool,
    /// The preview soft-proofed through the profile.
    pub proof: bool,
    /// Develop's proof view (paper white, ink black) for the preview.
    pub view: specular_develop::proof::ProofSettings,
    /// The styles to choose from (listed names) and the one applied to the printed
    /// copies.
    pub styles: Vec<(String, specular_develop::model::Style)>,
    pub style: Option<usize>,
}

impl DevelopPrint {
    fn rows(&self) -> Vec<RowView> {
        let rv = |row: Row, value: String| RowView {
            row,
            label: row_label(row),
            value,
            editing: None,
        };
        let on = |b: bool| if b { t!("print-on") } else { t!("print-off") };
        let mut v = vec![rv(
            Row::Profile,
            match (&self.profile, self.through) {
                (Some((name, _)), true) => specular_i18n::isolate(name),
                _ => t!("print-profile-srgb"),
            },
        )];
        if self.printing_through() {
            v.push(rv(
                Row::Intent,
                if self.relative {
                    t!("print-intent-relative")
                } else {
                    t!("print-intent-perceptual")
                },
            ));
            if self.relative {
                v.push(rv(Row::Bpc, on(self.bpc)));
            }
            v.push(rv(Row::Proof, on(self.proof)));
        }
        v.push(rv(
            Row::Style,
            match self.style.and_then(|i| self.styles.get(i)) {
                Some((name, _)) => specular_i18n::isolate(name),
                None => t!("print-style-none"),
            },
        ));
        v
    }

    fn printing_through(&self) -> bool {
        self.through && self.profile.is_some()
    }

    /// The style applied to the printed copies.
    pub(super) fn style(&self) -> Option<specular_develop::model::Style> {
        self.style
            .and_then(|i| self.styles.get(i))
            .map(|(_, s)| s.clone())
    }

    /// The rendering intent and black point compensation the print uses: perceptual
    /// always compensates; relative colorimetric as switched.
    pub(super) fn intent(&self) -> (specular_color::transform::Intent, bool) {
        use specular_color::transform::Intent;
        if self.relative {
            (Intent::Relative, self.bpc)
        } else {
            (Intent::Perceptual, true)
        }
    }

    /// The printer profile for the PDF, when printing through one.
    fn output(&self) -> Option<Result<specular_print::profile::PrintProfile, String>> {
        let (_, icc) = self.profile.as_ref().filter(|_| self.through)?;
        let (intent, bpc) = self.intent();
        Some(specular_print::profile::PrintProfile::new(
            icc.to_vec(),
            intent,
            bpc,
        ))
    }

    /// The preview's soft proof, when it is on.
    fn filter(&self) -> Option<specular_print::PreviewFilter> {
        let (_, icc) = self
            .profile
            .as_ref()
            .filter(|_| self.through && self.proof)?;
        let (_, bpc) = self.intent();
        match super::develop::proof_filter(icc, self.relative, bpc, self.view) {
            Ok(f) => Some(f),
            Err(e) => {
                specular_log::warn!("print: soft proof: {e}");
                None
            }
        }
    }
}

/// The app's print state.
#[derive(Default)]
pub(super) struct PrintState {
    pub dialog: Option<Dialog>,
    choices: Option<Choices>,
    worker: Option<Worker>,
    /// The system print call running for a temporary PDF.
    printing: Option<(
        PathBuf,
        Receiver<Result<specular_platform::print::Printed, String>>,
    )>,
    /// Retired preview textures, unregistered at the next upload.
    retired: Vec<TexId>,
    /// The device was replaced: the worker is started again on the new one once it is idle.
    respawn: bool,
    /// A PDF a GPU reset ended, written again on the new worker.
    retry: Option<Purpose>,
    /// The PDF being started is that second try.
    retrying: bool,
    /// The free layout being used (for the session), the listed layout it came from and
    /// whether it was changed since.
    free: Option<FreeLayout>,
    free_from: Option<usize>,
    free_edited: bool,
    /// The user's saved layouts, read on first use.
    user_layouts: Option<Vec<FreeLayout>>,
    snap: SnapMode,
    /// Whether the printer reported borderless printing, and the query running.
    borderless: Option<specular_platform::print::Borderless>,
    borderless_rx: Option<Receiver<specular_platform::print::Borderless>>,
}

impl std::fmt::Debug for PrintState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrintState")
            .field("open", &self.dialog.is_some())
            .finish_non_exhaustive()
    }
}

impl PrintState {
    /// The GPU device was replaced: the preview is made and uploaded again.
    pub(super) fn gpu_reset(&mut self) {
        self.retired.clear();
        self.respawn = self.worker.is_some();
        if let Some(d) = self.dialog.as_mut() {
            d.preview = None;
            d.asked = None;
        }
    }

    fn choices(&self) -> Choices {
        self.choices.unwrap_or_else(Choices::for_locale)
    }

    /// Borderless can be offered: the printer said it can.
    fn borderless_ok(&self) -> bool {
        self.borderless == Some(specular_platform::print::Borderless::Yes)
    }

    /// The user's saved layouts (read from the settings folder once).
    fn user_layouts(&mut self) -> &mut Vec<FreeLayout> {
        self.user_layouts.get_or_insert_with(load_layouts)
    }

    /// The layouts listed: the shipped ones (on the locale's paper), then the user's.
    fn all_layouts(&mut self) -> Vec<FreeLayout> {
        let mut v = specular_print::free::builtin(Paper::for_locale(&locale()));
        v.extend(self.user_layouts().iter().cloned());
        v
    }

    /// The free layout, the first shipped one to begin with.
    fn free_layout(&mut self) -> &mut FreeLayout {
        if self.free.is_none() {
            let first = self.all_layouts().into_iter().next().unwrap_or_default();
            self.free = Some(first);
            self.free_from = Some(0);
            self.free_edited = false;
        }
        let ok = self.borderless_ok();
        let f = self.free.get_or_insert_with(FreeLayout::default);
        if f.borderless && !ok {
            let m = f.margin;
            f.set_margin(m, false);
        }
        f
    }

    /// What the Saved row says: the layout's name (shipped names translated), marked when
    /// changed, or "Custom".
    fn saved_label(&mut self) -> String {
        let edited = self.free_edited;
        let name = match (self.free_from, self.free.as_ref()) {
            (Some(_), Some(f)) => layout_name(f),
            _ => return t!("print-layout-custom"),
        };
        if edited {
            t!("print-layout-changed", name = name)
        } else {
            name
        }
    }

    /// The listed index of the user's layout in use, if it is one.
    fn user_layout_index(&mut self) -> Option<usize> {
        let builtin = specular_print::free::builtin(Paper::A4).len();
        self.free_from.and_then(|i| i.checked_sub(builtin))
    }

    /// The free layout's editor as drawn.
    fn editor_view(&mut self, c: &Choices) -> Option<EditorView> {
        let ok = self.borderless_ok();
        let user = self.user_layout_index().is_some();
        let fl = self.free_layout().clone();
        let d = self.dialog.as_ref()?;
        let rtl = d.doc.as_ref().is_some_and(|doc| doc.rtl);
        let order = fl.fill_order(rtl);
        let n = order.len();
        let mut areas: Vec<editor::AreaView> = Vec::with_capacity(n);
        for (i, a) in fl.areas.iter().enumerate() {
            let k = order.iter().position(|&o| o == i).unwrap_or(i);
            let photo = d.items.get(d.page * n + k).map(|it| {
                let (image, _) = specular_print::free::place(a, it.size);
                (it.size, image)
            });
            areas.push(editor::area_view(
                k + 1,
                a.rect,
                photo,
                c.free_unit,
                d.selected == Some(i),
            ));
        }
        let busy = d.busy.is_some();
        let tools = if d.confirm {
            vec![
                (Hit::ConfirmDelete, t!("print-layout-delete-confirm"), true),
                (Hit::KeepLayout, t!("app-cancel"), true),
            ]
        } else {
            vec![
                (Hit::AddArea, t!("print-area-add"), !busy),
                (
                    Hit::RemoveArea,
                    t!("print-area-remove"),
                    !busy && d.selected.is_some(),
                ),
                (Hit::SaveLayout, t!("print-layout-save"), !busy),
                (Hit::DeleteLayout, t!("print-layout-delete"), !busy && user),
            ]
        };
        let notice = if d.confirm {
            Some(t!("print-layout-delete-ask", name = layout_name(&fl)))
        } else if fl.borderless && ok {
            Some(t!("print-borderless-note"))
        } else {
            None
        };
        Some(EditorView {
            sheet: fl.sheet_mm(),
            printable: fl.printable(),
            areas,
            guides: d.guides.clone(),
            grid: (d.drag.is_some() && self.snap == SnapMode::Grid)
                .then(|| editor::grid_step(c.free_unit)),
            notice,
            tools,
        })
    }

    /// The dialog as the draw function sees it.
    fn view(&mut self) -> Option<DialogView> {
        let c = self.choices();
        let editor = if c.free { self.editor_view(&c) } else { None };
        let saved = if c.free {
            self.saved_label()
        } else {
            String::new()
        };
        let ok = self.borderless_ok();
        let snap = self.snap;
        let fl = if c.free {
            Some(self.free_layout().clone())
        } else {
            None
        };
        let d = self.dialog.as_ref()?;
        let (src, paths) = &d.sources[d.source];
        let pages = d.doc.as_ref().map_or(0, |doc| doc.pages.len());
        let (pw, ph) = c.setup.size_mm();
        let (status, progress) = match &d.busy {
            Some(b) => (
                Some(t!(
                    "print-writing",
                    page = (b.done + 1).min(b.total.max(1)) as i64,
                    pages = b.total as i64
                )),
                Some(b.done as f32 / b.total.max(1) as f32),
            ),
            None if d.preview.is_none() || d.doc.is_none() => (Some(t!("print-preparing")), None),
            None => (None, None),
        };
        let mut rows = row_views(&c, source_label(*src, paths.len()));
        if let Some(fl) = &fl {
            free_rows(&mut rows, &c, fl, d.selected, &saved, ok, snap);
        }
        if let Some((row, le)) = &d.typing
            && let Some(r) = rows.iter_mut().find(|r| r.row == *row)
        {
            r.value = le.text().to_owned();
            r.editing = Some(le.caret());
            if *row == Row::Saved {
                r.label = t!("print-layout-name");
            }
        }
        if let Some(dp) = &d.develop {
            rows.splice(1..1, dp.rows());
        }
        Some(DialogView {
            rows,
            page: (d.page + 1, pages),
            aspect: (pw / ph) as f32,
            preview: d.preview.as_ref().map(|p| p.id),
            status,
            progress,
            busy: d.busy.is_some(),
            can_print: d.can_print,
            focus: d.focus,
            editor,
        })
    }

    /// Draws the dialog (when open) over everything and asks for the preview at the size it
    /// is shown; returns what the accessibility tree needs.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw(
        &mut self,
        dl: &mut DrawList,
        gpu: &Gpu,
        ui: &mut UiRenderer,
        text: &mut Text,
        pal: &Palette,
        size: (f32, f32),
        scale: f32,
        rtl: bool,
        ts: f32,
    ) -> Option<DialogView> {
        for id in self.retired.drain(..) {
            ui.unregister_texture(id);
        }
        // A preview that arrived: into a texture.
        if let Some(d) = self.dialog.as_mut()
            && let Some((key, w, h, rgba)) = d.pending.take()
        {
            let (tex, id) = upload(gpu, ui, w, h, &rgba);
            if let Some(old) = d.preview.replace(Preview { _tex: tex, id, key }) {
                ui.unregister_texture(old.id);
            }
        }
        let v = self.view()?;
        let drawn = draw(dl, text, pal, &v, size, rtl, ts);
        let long_px = (drawn.page.w.max(drawn.page.h) * scale).round() as u32;
        let d = self.dialog.as_mut()?;
        d.drawn = drawn;
        // Ask for the page at this size unless it is shown or on its way.
        if let (Some(doc), Some(w)) = (d.doc.clone(), self.worker.as_mut())
            && d.busy.is_none()
            && long_px > 16
        {
            let key = (d.rev, d.page, long_px);
            let shown = d.preview.as_ref().is_some_and(|p| p.key == key);
            if !shown && d.asked.map(|a| a.0) != Some(key) {
                let job = w.preview(doc, d.page, long_px);
                d.asked = Some((key, job));
            }
        }
        Some(v)
    }

    /// The dialog's nodes in the accessibility tree.
    pub(super) fn a11y(&self, tree: &mut accesskit::TreeUpdate, v: &DialogView, scale: f64) {
        if let Some(d) = &self.dialog {
            a11y(tree, v, &d.drawn, scale);
        }
    }
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// A temporary PDF for the system to print.
fn temp_pdf() -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(|d| PathBuf::from(d).join("specular"))
        .unwrap_or_else(|| std::env::temp_dir().join("specular-print"));
    let _ = std::fs::create_dir_all(&dir);
    dir.join(format!(
        "print-{}-{}.pdf",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ))
}

fn row_label(row: Row) -> String {
    match row {
        Row::Images => t!("print-images"),
        Row::Layout => t!("print-layout"),
        Row::Paper => t!("print-paper"),
        Row::Orientation => t!("print-orientation"),
        Row::Margins => t!("print-margins"),
        Row::Size => t!("print-size"),
        Row::Width => t!("print-width"),
        Row::Scale => t!("print-scale"),
        Row::Columns => t!("print-columns"),
        Row::Rows => t!("print-rows"),
        Row::Header => t!("print-header"),
        Row::Captions => t!("print-captions"),
        Row::Units => t!("print-units"),
        Row::Resolution => t!("print-resolution"),
        Row::Copies => t!("print-copies"),
        Row::Profile => t!("print-profile"),
        Row::Intent => t!("print-intent"),
        Row::Bpc => t!("print-bpc"),
        Row::Proof => t!("print-soft-proof"),
        Row::Style => t!("print-style"),
        Row::Saved => t!("print-layout-saved-row"),
        Row::Borderless => t!("print-borderless"),
        Row::Gap => t!("print-gap"),
        Row::Placement => t!("print-area-placement"),
        Row::Snap => t!("print-snap"),
    }
}

/// The POSIX locale paper and units follow.
fn locale() -> String {
    ["LC_PAPER", "LC_ALL", "LANG"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
        .unwrap_or_default()
}

/// A layout's name as listed: shipped names in the UI language.
fn layout_name(l: &FreeLayout) -> String {
    match l.id.as_str() {
        "one-border" => t!("print-layout-builtin-one-border"),
        "two-up" => t!("print-layout-builtin-two-up"),
        "four-up" => t!("print-layout-builtin-four-up"),
        "contact-3x3" => t!("print-layout-builtin-contact"),
        "8x10-letter" => t!("print-layout-builtin-8x10"),
        "a4-mat" => t!("print-layout-builtin-mat"),
        _ => specular_i18n::isolate(&l.name),
    }
}

/// The saved layouts' file, with the other presets in the settings folder (none in tests).
fn layouts_file() -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    Some(
        specular_settings::Locations::detect()
            .config_dir
            .join("print-layouts.txt"),
    )
}

/// The user's saved layouts.
fn load_layouts() -> Vec<FreeLayout> {
    let Some(text) = layouts_file().and_then(|p| std::fs::read_to_string(p).ok()) else {
        return Vec::new();
    };
    specular_print::free::parse_all(&text)
        .into_iter()
        .filter(|l| l.id.is_empty())
        .collect()
}

/// Writes the user's layouts (atomically).
fn save_layouts(layouts: &[FreeLayout]) {
    let Some(path) = layouts_file() else { return };
    let text = format!(
        "# Specular: saved print layouts. One [section] per layout; lengths in mm.\n{}",
        specular_print::free::write_all(layouts)
    );
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("txt.tmp");
    match std::fs::write(&tmp, text) {
        Ok(()) => {
            if let Err(e) = std::fs::rename(&tmp, &path) {
                specular_log::warn!("print: cannot save the layouts: {e}");
            }
        }
        Err(e) => specular_log::warn!("print: cannot save the layouts: {e}"),
    }
}

fn source_label(s: Source, n: usize) -> String {
    let n = n as i64;
    match s {
        Source::Current => t!("print-images-current"),
        Source::Folder => t!("print-images-folder", count = n),
        Source::Selection => t!("print-images-selection", count = n),
        Source::Picks => t!("print-images-picked", count = n),
    }
}

/// The rows as the dialog shows them.
pub(super) fn row_views(c: &Choices, source: String) -> Vec<RowView> {
    let len = |mm: f64| {
        format!(
            "{} {}",
            c.unit.format(mm),
            specular_i18n::unit_symbol(c.unit.symbol())
        )
    };
    c.rows()
        .into_iter()
        .map(|row| {
            let value = match row {
                Row::Images => source.clone(),
                Row::Layout if c.free => t!("print-layout-free"),
                Row::Layout if c.sheet => t!("print-layout-sheet"),
                Row::Layout => t!("print-layout-single"),
                Row::Paper => c.setup.paper.name().unwrap_or("—").to_owned(),
                Row::Orientation => match c.setup.orientation {
                    PageOrientation::Portrait => t!("print-orientation-portrait"),
                    PageOrientation::Landscape => t!("print-orientation-landscape"),
                },
                Row::Margins => len(c.setup.margin),
                Row::Size => match c.fit {
                    Fit::Fit => t!("print-size-fit"),
                    Fit::Fill => t!("print-size-fill"),
                    Fit::Size { .. } => t!("print-size-exact"),
                    Fit::Scale { .. } => t!("print-size-scale"),
                },
                Row::Width => len(c.width),
                Row::Scale => match c.ratio {
                    Some(n) => specular_measure::scale::ratio_text(n),
                    None => t!("print-scale-as-drawn"),
                },
                Row::Columns => c.cols.to_string(),
                Row::Rows => c.rows.to_string(),
                Row::Header if c.header => t!("print-on"),
                Row::Header => t!("print-off"),
                Row::Captions => match if c.sheet {
                    c.sheet_caption
                } else {
                    c.single_caption
                } {
                    Caption::Off => t!("print-captions-off"),
                    Caption::Name => t!("print-captions-name"),
                    Caption::NameDate => t!("print-captions-name-date"),
                },
                Row::Resolution => t!("print-dpi", dpi = i64::from(c.setup.dpi)),
                Row::Copies => c.copies.to_string(),
                Row::Units if c.free => match c.free_unit {
                    Unit::Mm => t!("print-units-mm"),
                    Unit::Cm => t!("print-units-cm"),
                    Unit::In => t!("print-units-in"),
                },
                Row::Units => match c.unit {
                    Unit::Cm | Unit::Mm => t!("print-units-cm"),
                    Unit::In => t!("print-units-in"),
                },
                // Filled in by the print state (Develop's and the free layout's).
                _ => String::new(),
            };
            RowView {
                row,
                label: row_label(row),
                value,
                editing: None,
            }
        })
        .collect()
}

/// The free layout's rows: Borderless only when the printer can, Photo only with an area
/// selected, and their values.
pub(super) fn free_rows(
    rows: &mut Vec<RowView>,
    c: &Choices,
    fl: &FreeLayout,
    selected: Option<usize>,
    saved: &str,
    borderless_ok: bool,
    snap: SnapMode,
) {
    let selected = selected.and_then(|i| fl.areas.get(i));
    rows.retain(|r| match r.row {
        Row::Borderless => borderless_ok,
        Row::Placement => selected.is_some(),
        _ => true,
    });
    let on = |b: bool| if b { t!("print-on") } else { t!("print-off") };
    for r in rows.iter_mut() {
        match r.row {
            Row::Saved => r.value = saved.to_owned(),
            Row::Borderless => r.value = on(fl.borderless),
            Row::Margins if fl.borderless => r.value = t!("print-margins-none"),
            Row::Margins => r.value = editor::length(c.setup.margin, c.free_unit),
            Row::Gap => r.value = editor::length(fl.gap, c.free_unit),
            Row::Placement => {
                r.value = match selected.map(|a| a.placement) {
                    Some(Placement::Fill) => t!("print-area-fill"),
                    _ => t!("print-area-fit"),
                }
            }
            Row::Snap => {
                r.value = match snap {
                    SnapMode::Off => t!("print-snap-off"),
                    SnapMode::Guides => t!("print-snap-guides"),
                    SnapMode::Grid => t!("print-snap-grid"),
                }
            }
            _ => {}
        }
    }
}

/// The notes beside the scale bars of pages printed to scale, in the reader's language.
/// The document for `items` in the UI language: page numbers and scale notes in it, the
/// pages mirrored when it is written right to left.
fn document(
    choices: &Choices,
    items: &[Item],
    title: &str,
    rtl: bool,
    free: Option<&FreeLayout>,
) -> Document {
    if choices.free
        && let Some(f) = free
    {
        return f.document(choices.setup.dpi, items, rtl);
    }
    let mut doc = specular_print::layout(
        choices.setup,
        choices.kind(),
        items,
        title,
        &|page, pages| t!("print-page", page = page as i64, pages = pages as i64),
    );
    scale_notes(&mut doc, items);
    if rtl {
        doc = doc.right_to_left();
    }
    doc
}

pub(super) fn scale_notes(doc: &mut Document, items: &[Item]) {
    for page in &mut doc.pages {
        let name = page
            .images
            .first()
            .and_then(|p| items.get(p.item))
            .map_or(String::new(), |i| i.name.clone());
        let Some(m) = page.scale.as_mut() else {
            continue;
        };
        let mut note = t!(
            "print-scale-note",
            name = name,
            scale = m.ratio.clone(),
            length = m.length.clone()
        );
        let (rows, cols) = m.tiles;
        if rows * cols > 1 {
            let (r, c) = m.tile;
            note.push_str("  ");
            note.push_str(&t!(
                "print-scale-tile",
                n = i64::from(r * cols + c + 1),
                count = i64::from(rows * cols),
                row = i64::from(r + 1),
                column = i64::from(c + 1)
            ));
        }
        m.note = note;
    }
}

/// Uploads an RGBA preview into a new sRGB texture the UI can draw.
fn upload(gpu: &Gpu, ui: &mut UiRenderer, w: u32, h: u32, rgba: &[u8]) -> (wgpu::Texture, TexId) {
    let size = wgpu::Extent3d {
        width: w,
        height: h,
        depth_or_array_layers: 1,
    };
    let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("print preview"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    gpu.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * w),
            rows_per_image: None,
        },
        size,
    );
    let view = tex.create_view(&Default::default());
    let id = ui.register_texture(gpu, &view);
    (tex, id)
}

impl App {
    /// The system print call or the probe panicked: nothing waits for them.
    pub(super) fn print_lost(&mut self, task: &str) {
        match task {
            "print-system" => {
                if let Some((tmp, _)) = self.print.printing.take() {
                    let _ = std::fs::remove_file(tmp);
                }
            }
            "print-probe" => {
                if let Some(d) = self.print.dialog.as_mut() {
                    d.probing = None;
                }
            }
            _ => {}
        }
    }

    fn print_choices(&self) -> Choices {
        self.print.choices()
    }

    pub(super) fn print_open(&mut self) {
        if self.print.dialog.is_some() {
            return;
        }
        let mut sources = Vec::new();
        if let Some(p) = self.current.and_then(|i| self.entries.get(i)) {
            sources.push((Source::Current, vec![p.clone()]));
        }
        // The folder: the gallery's files in its order while it is shown (also with no image
        // open), else the viewer's.
        let files: Vec<PathBuf> = if self.gallery_active() {
            self.gallery.image_order()
        } else {
            self.entries.clone()
        };
        if !files.is_empty() {
            sources.push((Source::Folder, files));
        }
        let mut first = 0;
        if self.gallery_active() {
            let sel: Vec<PathBuf> = self
                .gallery
                .targets()
                .into_iter()
                .filter(|p| !p.is_dir())
                .collect();
            if sel.len() > 1 {
                sources.push((Source::Selection, sel));
                first = sources.len() - 1;
            }
        }
        let picks = self.marked_files();
        if !picks.is_empty() {
            sources.push((Source::Picks, picks));
        }
        // From Develop: the developed photo, or the selection or picks developed (not the
        // whole folder).
        if self.develop_active() {
            sources.retain(|s| s.0 != Source::Folder);
            first = 0;
        }
        if sources.is_empty() {
            return;
        }
        let mut choices = self.print_choices();
        // From the gallery's selection: a contact sheet; from the viewer: the image.
        if sources[first].0 == Source::Selection {
            choices.sheet = true;
        } else if sources[first].0 == Source::Current {
            choices.sheet = false;
        }
        self.print.choices = Some(choices);
        if self.print.worker.is_none()
            && let Some(g) = self.gfx.as_ref()
        {
            match Worker::spawn(g.gpu.clone(), false, self.loader.waker()) {
                Ok(w) => self.print.worker = Some(w),
                Err(e) => {
                    specular_log::warn!("print: cannot start the print thread: {e}");
                    return;
                }
            }
        }
        let title = self
            .folder
            .as_deref()
            .map(file_name)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                sources[0]
                    .1
                    .first()
                    .map(|p| file_name(p))
                    .unwrap_or_default()
            });
        self.print.dialog = Some(Dialog {
            sources,
            source: first,
            title,
            probes: HashMap::new(),
            probing: None,
            doc: None,
            rev: 0,
            language: 0,
            page: 0,
            preview: None,
            pending: None,
            asked: None,
            busy: None,
            can_print: specular_platform::print::available(),
            focus: Some(0),
            drawn: Drawn::default(),
            save_to: None,
            develop: self.develop_print(),
            items: Vec::new(),
            selected: None,
            drag: None,
            guides: Vec::new(),
            typing: None,
            confirm: false,
        });
        // Asked once a session: whether the printer can print borderless.
        if self.print.borderless.is_none() && self.print.borderless_rx.is_none() && !cfg!(test) {
            let (tx, rx) = crossbeam_channel::bounded(1);
            self.loader.background("print-noborder", move |l| {
                let _ = tx.send(specular_platform::print::borderless());
                l.waker()();
            });
            self.print.borderless_rx = Some(rx);
        }
        if choices.free {
            self.print_sync_setup();
        }
        self.print_source_changed();
        self.chrome.announce_status(t!("print-title"));
        self.redraw = true;
    }

    /// After a GPU reset: the print thread starts again on the new device once no PDF is
    /// being written, and a PDF the reset ended is written again (said in a toast).
    fn print_respawn(&mut self) {
        let busy = self.print.dialog.as_ref().is_some_and(|d| d.busy.is_some());
        if !self.print.respawn || busy || !self.gpu_ready() {
            return;
        }
        self.print.respawn = false;
        let Some(g) = self.gfx.as_ref() else {
            return;
        };
        self.print.worker = match Worker::spawn(g.gpu.clone(), false, self.loader.waker()) {
            Ok(w) => Some(w),
            Err(e) => {
                specular_log::warn!("print: cannot start the print thread: {e}");
                None
            }
        };
        if self.print.dialog.is_none() {
            self.print.retry = None;
            return;
        }
        self.print_source_changed();
        if let Some(purpose) = self.print.retry.take() {
            let now = self.now();
            self.chrome.toast(t!("toast-gpu-reset-retry"), None, now);
            self.print.retrying = true;
            self.print_write(purpose);
            self.print.retrying = false;
        }
    }

    fn print_close(&mut self) {
        if let Some(d) = self.print.dialog.take() {
            if let Some(p) = d.preview {
                self.print.retired.push(p.id);
            }
            if d.busy.is_some()
                && let Some(w) = self.print.worker.as_mut()
            {
                w.cancel();
            }
        }
        self.redraw = true;
    }

    /// Probes the chosen images (headers only, on a thread) unless already known.
    fn print_source_changed(&mut self) {
        let Some(d) = self.print.dialog.as_mut() else {
            return;
        };
        let (src, paths) = d.sources[d.source].clone();
        // Develop's photos: developed for the page, each at its printed size.
        if let Some(dp) = &d.develop {
            let paths = if src == Source::Current {
                vec![dp.path.clone()]
            } else {
                paths
            };
            let open = (dp.path.clone(), dp.edit.clone());
            let gpu = self.gfx.as_ref().map(|g| g.gpu.clone());
            if let (Some(w), Some(gpu)) = (self.print.worker.as_mut(), gpu) {
                w.set_source(Arc::new(super::develop::DevelopSource::new(
                    &gpu,
                    self.loader.registry(),
                    paths.clone(),
                    open.clone(),
                    dp.style(),
                )));
                w.set_preview_filter(dp.filter());
            }
            d.doc = None;
            d.asked = None;
            if src == Source::Current {
                d.probes.insert(
                    src,
                    vec![Probed {
                        size: Some(dp.size),
                        name: file_name(&dp.path),
                        modified: None,
                        taken: None,
                        scale: None,
                    }],
                );
            }
            if d.probes.contains_key(&src) {
                self.print_relayout();
                return;
            }
            // Other photos: their headers, then the size their edit develops to.
            let (tx, rx) = crossbeam_channel::bounded(1);
            let registry = self.loader.registry();
            self.loader.background("print-probe", move |l| {
                let mut probed = FileSource::new(registry, paths.clone()).probe();
                for (p, path) in probed.iter_mut().zip(&paths) {
                    let edit = if *path == open.0 {
                        open.1.clone()
                    } else {
                        super::develop::saved_edit(path)
                    };
                    p.size = p.size.map(|s| super::develop::developed_size(s, &edit));
                }
                let _ = tx.send(probed);
                l.waker()();
            });
            d.probing = Some((src, rx));
            return;
        }
        if let Some(w) = self.print.worker.as_mut() {
            w.set_preview_filter(None);
        }
        if let Some(w) = self.print.worker.as_mut() {
            w.set_source(Arc::new(FileSource::new(
                self.loader.registry(),
                paths.clone(),
            )));
        }
        d.doc = None;
        d.asked = None;
        if d.probes.contains_key(&src) {
            self.print_relayout();
            return;
        }
        let (tx, rx) = crossbeam_channel::bounded(1);
        let registry = self.loader.registry();
        self.loader.background("print-probe", move |l| {
            let _ = tx.send(FileSource::new(registry, paths).probe());
            l.waker()();
        });
        d.probing = Some((src, rx));
    }

    /// Lays the document out again after a change.
    fn print_relayout(&mut self) {
        let choices = self.print_choices();
        let rtl = self.rtl();
        let free = if choices.free {
            Some(self.print.free_layout().clone())
        } else {
            None
        };
        let Some(d) = self.print.dialog.as_mut() else {
            return;
        };
        let src = d.sources[d.source].0;
        let Some(probes) = d.probes.get(&src) else {
            return;
        };
        let items: Vec<Item> = probes
            .iter()
            .map(|p| Item {
                size: p.size.unwrap_or((3, 2)),
                name: p.name.clone(),
                date: p
                    .date()
                    .map(|s| super::gallery::date_text(s, specular_i18n::DateStyle::Medium)),
                scale: p.scale.clone(),
            })
            .collect();
        let doc = document(&choices, &items, &d.title, rtl, free.as_ref());
        d.items = items;
        d.page = d.page.min(doc.pages.len().saturating_sub(1));
        d.doc = Some(Arc::new(doc));
        d.rev += 1;
        d.language = specular_i18n::I18n::global().generation();
        self.redraw = true;
    }

    /// The UI language changed: an open dialog lays its page out again at once (page
    /// number, scale notes, right-to-left order).
    pub(super) fn print_language_changed(&mut self) {
        if self.print.dialog.as_ref().is_some_and(|d| {
            d.doc.is_some() && d.language != specular_i18n::I18n::global().generation()
        }) {
            self.print_relayout();
        }
    }

    fn print_view(&mut self) -> Option<DialogView> {
        self.print.view()
    }

    /// Replies from the print thread, the save dialog and the system print call.
    pub(super) fn print_tick(&mut self) {
        self.print_language_changed();
        self.print_respawn();
        if let Some(rx) = &self.print.borderless_rx
            && let Ok(b) = rx.try_recv()
        {
            self.print.borderless = Some(b);
            self.print.borderless_rx = None;
            // A layout saved borderless prints with margins where the printer cannot.
            if self.print.free.as_ref().is_some_and(|f| f.borderless) && !self.print.borderless_ok()
            {
                self.print.free_layout();
                self.print_relayout();
            }
            self.redraw = true;
        }
        let now = self.now();
        if let Some((tmp, rx)) = &self.print.printing
            && let Ok(r) = rx.try_recv()
        {
            let _ = std::fs::remove_file(tmp);
            self.print.printing = None;
            let msg = match r {
                Ok(_) => {
                    self.audit(super::audit::Event::Print, "printer", &[], &[]);
                    t!("print-sent")
                }
                Err(e) => t!("print-failed", reason = e),
            };
            self.chrome.toast(msg, None, now);
            self.redraw = true;
        }
        let Some(d) = self.print.dialog.as_mut() else {
            // Late replies of a closed dialog.
            if let Some(w) = &self.print.worker {
                while w.try_recv().is_some() {}
            }
            return;
        };
        if let Some((src, rx)) = &d.probing
            && let Ok(p) = rx.try_recv()
        {
            d.probes.insert(*src, p);
            d.probing = None;
            self.print_relayout();
        }
        let Some(d) = self.print.dialog.as_mut() else {
            return;
        };
        if let Some(rx) = &d.save_to
            && let Ok(path) = rx.try_recv()
        {
            d.save_to = None;
            self.print_write(Purpose::Save(path));
        }
        let mut finished = None;
        if let (Some(w), Some(d)) = (self.print.worker.as_ref(), self.print.dialog.as_mut()) {
            while let Some(r) = w.try_recv() {
                match r {
                    Reply::Preview {
                        job,
                        width,
                        height,
                        rgba,
                        ..
                    } => {
                        if let Some((key, j)) = d.asked
                            && j == job
                        {
                            d.pending = Some((key, width, height, rgba));
                            self.redraw = true;
                        }
                    }
                    Reply::PreviewFailed { error, .. } => {
                        specular_log::warn!("print preview: {error}");
                    }
                    Reply::Progress { job, done, total } => {
                        if let Some(b) = d.busy.as_mut().filter(|b| b.job == job) {
                            b.done = done;
                            b.total = total;
                            self.redraw = true;
                        }
                    }
                    Reply::Pdf { job, result } => {
                        if let Some(b) = d.busy.take_if(|b| b.job == job) {
                            let reset = result.is_err()
                                && !b.retried
                                && (super::gpu_reset::gpu_resets() != b.since
                                    || self.print.respawn);
                            if reset {
                                // Written again once the worker is on the new device.
                                self.print.retry = Some(b.purpose);
                                self.print.respawn = true;
                            } else {
                                finished = Some((b.purpose, result));
                            }
                        }
                    }
                    Reply::Cancelled { job } => {
                        if d.busy.as_ref().is_some_and(|b| b.job == job) {
                            d.busy = None;
                        }
                        self.redraw = true;
                    }
                }
            }
        }
        if let Some((purpose, result)) = finished {
            self.print_finished(purpose, result);
        }
    }

    fn print_finished(
        &mut self,
        purpose: Purpose,
        result: Result<specular_print::PdfDone, String>,
    ) {
        let now = self.now();
        let copies = self.print_choices().copies;
        match (purpose, result) {
            (Purpose::Save(_), Ok(done)) => {
                self.audit(super::audit::Event::Export, "pdf", &[&done.path], &[]);
                let mut msg = t!("print-saved", file = file_name(&done.path));
                if done.missing > 0 {
                    msg = format!(
                        "{msg}. {}",
                        t!("print-missing", count = done.missing as i64)
                    );
                }
                self.chrome.toast(msg, None, now);
                self.print_close();
            }
            (Purpose::Save(_), Err(e)) => {
                self.chrome
                    .warning(t!("print-save-failed", reason = e), now);
            }
            (Purpose::Print(tmp), Ok(_)) => {
                let Some(d) = self.print.dialog.as_ref() else {
                    return;
                };
                let c = self.print_choices();
                let paper = c.setup.paper;
                let (w, h) = paper.size_mm();
                let job = specular_platform::print::Job {
                    title: d.title.clone(),
                    copies,
                    paper: specular_platform::print::Paper {
                        pwg: match paper {
                            Paper::Custom { .. } => String::new(),
                            _ => paper.pwg_name(),
                        },
                        display: paper.name().unwrap_or("").to_owned(),
                        width_mm: w,
                        height_mm: h,
                    },
                    landscape: c.setup.orientation == PageOrientation::Landscape,
                };
                let (tx, rx) = crossbeam_channel::bounded(1);
                let pdf = tmp.clone();
                self.loader.background("print-system", move |l| {
                    let r = specular_platform::print::print(&pdf, &job)
                        .map_err(|e| crate::error_text::reason(&e));
                    let _ = tx.send(r);
                    l.waker()();
                });
                self.print.printing = Some((tmp, rx));
                self.print_close();
            }
            (Purpose::Print(tmp), Err(e)) => {
                let _ = std::fs::remove_file(tmp);
                self.chrome.warning(t!("print-failed", reason = e), now);
            }
        }
        self.redraw = true;
    }

    /// Starts writing the PDF.
    fn print_write(&mut self, purpose: Purpose) {
        let path = match &purpose {
            Purpose::Save(p) | Purpose::Print(p) => p.clone(),
        };
        let unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let output = match self
            .print
            .dialog
            .as_ref()
            .and_then(|d| d.develop.as_ref())
            .and_then(DevelopPrint::output)
        {
            Some(Ok(p)) => Some(Arc::new(p)),
            Some(Err(e)) => {
                let now = self.now();
                self.chrome
                    .warning(t!("print-profile-failed", reason = e), now);
                return;
            }
            None => None,
        };
        let Some(d) = self.print.dialog.as_mut() else {
            return;
        };
        let (Some(doc), Some(w)) = (d.doc.clone(), self.print.worker.as_mut()) else {
            return;
        };
        let meta = specular_print::pdf::Meta {
            title: d.title.clone(),
            date: specular_print::pdf::date(unix),
            output,
        };
        let total = doc.pages.len();
        let job = w.pdf(doc, meta, path);
        // The preview job was abandoned with it.
        d.asked = None;
        d.busy = Some(Busy {
            job,
            purpose,
            done: 0,
            total,
            since: super::gpu_reset::gpu_resets(),
            retried: std::mem::take(&mut self.print.retrying),
        });
        self.redraw = true;
    }

    fn print_save(&mut self) {
        let Some(d) = self.print.dialog.as_mut() else {
            return;
        };
        if d.busy.is_some() || d.doc.is_none() || d.save_to.is_some() {
            return;
        }
        let stem = if d.sources[d.source].0 == Source::Current {
            d.sources[d.source]
                .1
                .first()
                .and_then(|p| p.file_stem())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        } else {
            d.title.clone()
        };
        let dir = self.folder.clone();
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("dlg-print", move |l| {
            let mut dlg = rfd::FileDialog::new()
                .add_filter("PDF", &["pdf"])
                .set_file_name(format!("{stem}.pdf"));
            if let Some(dir) = dir {
                dlg = dlg.set_directory(dir);
            }
            if let Some(p) = dlg.save_file() {
                let _ = tx.send(p);
                l.waker()();
            }
        });
        d.save_to = Some(rx);
    }

    fn print_print(&mut self) {
        let Some(d) = self.print.dialog.as_ref() else {
            return;
        };
        if !d.can_print {
            let now = self.now();
            self.chrome.warning(t!("print-unavailable"), now);
            return;
        }
        if d.busy.is_some() || d.doc.is_none() || self.print.printing.is_some() {
            return;
        }
        self.print_write(Purpose::Print(temp_pdf()));
    }

    fn print_step(&mut self, row: Row, delta: i32) {
        let Some(d) = self.print.dialog.as_mut() else {
            return;
        };
        if d.busy.is_some() {
            return;
        }
        if matches!(
            row,
            Row::Profile | Row::Intent | Row::Bpc | Row::Proof | Row::Style
        ) {
            let Some(dp) = d.develop.as_mut() else { return };
            match row {
                Row::Profile => {
                    if dp.profile.is_none() {
                        let now = self.now();
                        self.chrome.toast(t!("print-profile-none"), None, now);
                        return;
                    }
                    dp.through = !dp.through;
                }
                Row::Intent => dp.relative = !dp.relative,
                Row::Bpc => dp.bpc = !dp.bpc,
                Row::Proof => dp.proof = !dp.proof,
                _ => {
                    let n = dp.styles.len() as i32 + 1;
                    let i = dp.style.map_or(0, |i| i as i32 + 1);
                    dp.style = ((i + delta).rem_euclid(n) as usize).checked_sub(1);
                }
            }
            if row == Row::Style {
                // The photos develop again with the style.
                self.print_source_changed();
            } else {
                let f = dp.filter();
                if let Some(w) = self.print.worker.as_mut() {
                    w.set_preview_filter(f);
                }
                // The preview is made again through the new proof.
                d.rev += 1;
            }
            self.print_announce(row);
            self.redraw = true;
            return;
        }
        if matches!(
            row,
            Row::Saved | Row::Borderless | Row::Gap | Row::Placement | Row::Snap
        ) {
            self.print_free_step(row, delta);
            return;
        }
        if row == Row::Images {
            if d.sources.len() > 1 {
                d.source = (d.source as i32 + delta).rem_euclid(d.sources.len() as i32) as usize;
                d.page = 0;
                let (src, paths) = &d.sources[d.source];
                let label = source_label(*src, paths.len());
                self.print_source_changed();
                self.chrome.announce_status(label);
            }
            return;
        }
        let mut c = self.print_choices();
        if c.step(row, delta) {
            self.print.choices = Some(c);
            if c.free {
                self.print_sync_free(row);
            }
            // Focus stays on the row (rows come and go with the layout).
            if let Some(v) = self.print_view()
                && let Some(i) = v.focusables().iter().position(|h| *h == Hit::Row(row))
                && let Some(d) = self.print.dialog.as_mut()
            {
                d.focus = Some(i);
                if let Some(rv) = v.rows.iter().find(|r| r.row == row) {
                    self.chrome.announce_status(rv.value.clone());
                }
            }
            self.print_relayout();
        }
    }

    fn print_page(&mut self, delta: i32) {
        let Some(d) = self.print.dialog.as_mut() else {
            return;
        };
        let pages = d.doc.as_ref().map_or(0, |doc| doc.pages.len());
        if pages == 0 {
            return;
        }
        let p = (d.page as i32 + delta).clamp(0, pages as i32 - 1) as usize;
        if p != d.page {
            d.page = p;
            self.chrome.announce_status(t!(
                "print-page",
                page = (p + 1) as i64,
                pages = pages as i64
            ));
            self.redraw = true;
        }
    }

    fn print_hit(&mut self, h: Hit) {
        match h {
            Hit::Dec(r) => self.print_step(r, -1),
            Hit::Inc(r) => self.print_step(r, 1),
            Hit::Row(r) => {
                if let Some(v) = self.print_view()
                    && let Some(i) = v.focusables().iter().position(|x| *x == Hit::Row(r))
                    && let Some(d) = self.print.dialog.as_mut()
                {
                    d.focus = Some(i);
                }
            }
            Hit::PrevPage => self.print_page(-1),
            Hit::NextPage => self.print_page(1),
            Hit::Cancel => {
                let busy = self.print.dialog.as_ref().is_some_and(|d| d.busy.is_some());
                if busy {
                    if let Some(w) = self.print.worker.as_mut() {
                        w.cancel();
                    }
                } else {
                    self.print_close();
                }
            }
            Hit::Save => self.print_save(),
            Hit::Print => self.print_print(),
            Hit::Type(r) => self.print_type_start(r, None),
            Hit::Page => self.print_focus(Hit::Page),
            Hit::Area(i) => {
                if let Some(d) = self.print.dialog.as_mut() {
                    d.selected = Some(i);
                }
                self.print_focus(Hit::Page);
                self.print_announce_area();
            }
            Hit::AddArea => self.print_add_area(),
            Hit::RemoveArea => self.print_remove_area(),
            Hit::SaveLayout => {
                let name = if self.print.user_layout_index().is_some() {
                    self.print
                        .free
                        .as_ref()
                        .map(|f| f.name.clone())
                        .unwrap_or_default()
                } else {
                    String::new()
                };
                self.print_type_start(Row::Saved, Some(name));
            }
            Hit::DeleteLayout => {
                if self.print.user_layout_index().is_some()
                    && let Some(d) = self.print.dialog.as_mut()
                {
                    d.confirm = true;
                    let name = self
                        .print
                        .free
                        .as_ref()
                        .map(layout_name)
                        .unwrap_or_default();
                    self.chrome
                        .announce_status(t!("print-layout-delete-ask", name = name));
                    self.print_focus(Hit::ConfirmDelete);
                }
            }
            Hit::ConfirmDelete => self.print_delete_layout(),
            Hit::KeepLayout => {
                if let Some(d) = self.print.dialog.as_mut() {
                    d.confirm = false;
                }
                self.print_focus(Hit::Page);
            }
        }
        self.redraw = true;
    }

    /// Keys while the dialog is open (it takes them all).
    pub(super) fn print_key(&mut self, combo: Option<&KeyCombo>, typed: Option<&str>) -> bool {
        if self
            .print
            .dialog
            .as_ref()
            .is_some_and(|d| d.typing.is_some())
        {
            self.print_type_key(combo, typed);
            self.redraw = true;
            return true;
        }
        let Some(v) = self.print_view() else {
            return false;
        };
        let focusables = v.focusables();
        let n = focusables.len();
        let focus = v.focus.and_then(|i| focusables.get(i).copied());
        // A digit on a length row starts typing it.
        if let Some(Hit::Row(r)) = focus
            && r.typed()
            && let Some(t) = typed.filter(|t| {
                !t.is_empty()
                    && t.chars()
                        .all(|c| c.is_ascii_digit() || c == '.' || c == ',')
            })
            && combo.is_none_or(|c| !c.mods.ctrl && !c.mods.alt)
        {
            self.print_type_start(r, Some(t.to_owned()));
            self.redraw = true;
            return true;
        }
        let Some(combo) = combo else { return true };
        let rtl = self.rtl();
        if focus == Some(Hit::Page) && self.print_page_key(combo) {
            self.redraw = true;
            return true;
        }
        let move_focus = |me: &mut App, d: i32| {
            if let Some(dl) = me.print.dialog.as_mut() {
                let i = dl
                    .focus
                    .map_or(0, |i| (i as i32 + d).rem_euclid(n as i32) as usize);
                dl.focus = Some(i);
            }
        };
        let plain = combo.mods == Mods::NONE;
        match combo.key {
            Key::Named(NamedKey::Escape) => self.print_hit(Hit::Cancel),
            Key::Named(NamedKey::Tab) if combo.mods.shift => move_focus(self, -1),
            Key::Named(NamedKey::Tab) => move_focus(self, 1),
            Key::Named(NamedKey::ArrowUp) if plain => move_focus(self, -1),
            Key::Named(NamedKey::ArrowDown) if plain => move_focus(self, 1),
            Key::Named(k @ (NamedKey::ArrowLeft | NamedKey::ArrowRight)) if plain => {
                let forward = (k == NamedKey::ArrowRight) != rtl;
                match focus {
                    Some(Hit::Row(r)) => self.print_step(r, if forward { 1 } else { -1 }),
                    _ => move_focus(self, if forward { 1 } else { -1 }),
                }
            }
            Key::Named(NamedKey::PageUp) => self.print_page(-1),
            Key::Named(NamedKey::PageDown) => self.print_page(1),
            Key::Named(NamedKey::Enter | NamedKey::Space) => match focus {
                Some(h @ (Hit::Cancel | Hit::Save | Hit::Print)) => self.print_hit(h),
                Some(Hit::Row(r)) if combo.key == Key::Named(NamedKey::Space) => {
                    self.print_step(r, 1)
                }
                _ => self.print_hit(if v.can_print { Hit::Print } else { Hit::Save }),
            },
            Key::Char('p') if combo.mods.ctrl => {
                self.print_hit(if v.can_print { Hit::Print } else { Hit::Save })
            }
            _ => {}
        }
        self.redraw = true;
        true
    }

    /// A press or release while the dialog is open (it takes them all; outside the sheet
    /// does nothing). On a free layout's page a press selects an area and starts a drag.
    pub(super) fn print_click(&mut self, p: (f32, f32), down: bool, primary: bool) -> bool {
        let Some(d) = self.print.dialog.as_mut() else {
            return false;
        };
        if !down {
            if d.drag.take().is_some() {
                d.guides.clear();
                self.print_relayout();
                self.print_announce_area();
            }
            return true;
        }
        if !primary {
            return true;
        }
        let hit = d
            .drawn
            .hits
            .iter()
            .find(|(r, h)| r.contains(p) && !matches!(h, Hit::Row(_) | Hit::Page))
            .or_else(|| d.drawn.hits.iter().find(|(r, _)| r.contains(p)))
            .map(|x| x.1);
        // A click elsewhere ends typing (what was typed is used when it reads).
        if d.typing.is_some() && !matches!(hit, Some(Hit::Type(_))) {
            self.print_type_commit(false);
        }
        match hit {
            Some(Hit::Page) => self.print_press_page(p),
            Some(h) => self.print_hit(h),
            None => {}
        }
        true
    }

    /// The pointer moved while the dialog is open: a drag on the free layout's page.
    pub(super) fn print_drag(&mut self, p: (f32, f32)) {
        let c = self.print_choices();
        let snap = self.print.snap;
        let PrintState { dialog, free, .. } = &mut self.print;
        let (Some(d), Some(fl)) = (dialog.as_mut(), free.as_mut()) else {
            return;
        };
        let Some(drag) = d.drag else { return };
        let photo = photo_in(d, fl, drag.area);
        let page = d.drawn.page;
        d.guides = editor::drag_to(fl, &drag, page, p, snap, c.free_unit, photo);
        self.print.free_edited = true;
        self.redraw = true;
    }

    /// A screen reader's request on one of the dialog's nodes; false when not ours.
    pub(super) fn print_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        let Some(v) = self.print_view() else {
            return false;
        };
        let Some(d) = self.print.dialog.as_ref() else {
            return false;
        };
        let Some(hit) = a11y_target(&v, &d.drawn, req.target_node.0) else {
            return req.target_node.0 == A11Y_BASE;
        };
        match (req.action, hit) {
            (accesskit::Action::Increment, Hit::Row(r)) => self.print_step(r, 1),
            (accesskit::Action::Decrement, Hit::Row(r)) => self.print_step(r, -1),
            (accesskit::Action::Click, h) => self.print_hit(h),
            (accesskit::Action::Focus, h) => {
                if let Some(i) = v.focusables().iter().position(|x| *x == h)
                    && let Some(d) = self.print.dialog.as_mut()
                {
                    d.focus = Some(i);
                }
            }
            _ => {}
        }
        self.redraw = true;
        true
    }
}

/// The photo size in area `area` of the page shown, if a photo lands there.
fn photo_in(d: &Dialog, fl: &FreeLayout, area: usize) -> Option<(u32, u32)> {
    let rtl = d.doc.as_ref().is_some_and(|doc| doc.rtl);
    let order = fl.fill_order(rtl);
    let k = order.iter().position(|&o| o == area)?;
    d.items.get(d.page * order.len() + k).map(|it| it.size)
}

/// Free layouts: the editor's edits, typed lengths and saved layouts.
impl App {
    /// Says a row's new value.
    fn print_announce(&mut self, row: Row) {
        if let Some(v) = self.print_view()
            && let Some(rv) = v.rows.iter().find(|r| r.row == row)
        {
            self.chrome.announce_status(rv.value.clone());
        }
    }

    /// Says the selected area's size and resolution.
    fn print_announce_area(&mut self) {
        let sel = self.print.dialog.as_ref().and_then(|d| d.selected);
        if let Some(v) = self.print_view()
            && let (Some(e), Some(i)) = (&v.editor, sel)
            && let Some(a) = e.areas.get(i)
        {
            self.chrome.announce_status(a.name.clone());
        }
    }

    /// Puts the keyboard focus on `h`.
    fn print_focus(&mut self, h: Hit) {
        if let Some(v) = self.print_view()
            && let Some(i) = v.focusables().iter().position(|x| *x == h)
            && let Some(d) = self.print.dialog.as_mut()
        {
            d.focus = Some(i);
        }
    }

    /// The sheet rows follow the free layout (its paper, orientation and margin).
    fn print_sync_setup(&mut self) {
        let f = self.print.free_layout().clone();
        let mut c = self.print_choices();
        c.setup.paper = f.paper;
        c.setup.orientation = f.orientation;
        c.setup.margin = f.margin;
        self.print.choices = Some(c);
    }

    /// After a sheet row changed in a free layout: the layout follows it.
    fn print_sync_free(&mut self, row: Row) {
        let c = self.print_choices();
        match row {
            Row::Layout => {
                self.print.free_layout();
                self.print_sync_setup();
                if let Some(d) = self.print.dialog.as_mut() {
                    d.selected = None;
                }
            }
            Row::Paper | Row::Orientation => {
                self.print
                    .free_layout()
                    .set_sheet(c.setup.paper, c.setup.orientation);
                self.print.free_edited = true;
            }
            Row::Margins => {
                let f = self.print.free_layout();
                let b = f.borderless;
                f.set_margin(c.setup.margin, b);
                self.print.free_edited = true;
            }
            _ => {}
        }
    }

    /// Steps one of the free layout's own rows.
    fn print_free_step(&mut self, row: Row, delta: i32) {
        let unit = self.print_choices().free_unit;
        let mut relayout = true;
        match row {
            Row::Saved => {
                let all = self.print.all_layouts();
                if all.is_empty() {
                    return;
                }
                let n = all.len() as i32;
                let next = match self.print.free_from {
                    Some(i) => (i as i32 + delta).rem_euclid(n),
                    None if delta > 0 => 0,
                    None => n - 1,
                } as usize;
                self.print.free = Some(all[next].clone());
                self.print.free_from = Some(next);
                self.print.free_edited = false;
                self.print.free_layout();
                self.print_sync_setup();
                if let Some(d) = self.print.dialog.as_mut() {
                    d.selected = None;
                    d.confirm = false;
                }
            }
            Row::Borderless => {
                if !self.print.borderless_ok() {
                    return;
                }
                let f = self.print.free_layout();
                let (m, b) = (f.margin, f.borderless);
                f.set_margin(m, !b);
                self.print.free_edited = true;
            }
            Row::Gap => {
                let f = self.print.free_layout();
                f.gap = unit
                    .snap(f.gap + f64::from(delta) * unit.step_mm())
                    .clamp(0.0, MAX_MARGIN_MM);
                self.print.free_edited = true;
                relayout = false;
            }
            Row::Placement => {
                let Some(i) = self.print.dialog.as_ref().and_then(|d| d.selected) else {
                    return;
                };
                let f = self.print.free_layout();
                let Some(a) = f.areas.get_mut(i) else { return };
                a.placement = match a.placement {
                    Placement::Fit => Placement::Fill,
                    Placement::Fill => Placement::Fit,
                };
                self.print.free_edited = true;
            }
            Row::Snap => {
                self.print.snap = self.print.snap.next(delta);
                relayout = false;
            }
            _ => return,
        }
        self.print_announce(row);
        if relayout {
            self.print_relayout();
        }
        self.redraw = true;
    }

    /// Starts typing `row` (a length, or a layout's name), with `text` or its value.
    fn print_type_start(&mut self, row: Row, text: Option<String>) {
        let c = self.print_choices();
        let unit = if c.free { c.free_unit } else { c.unit };
        let replace = text.is_some();
        let text = text.unwrap_or_else(|| match row {
            Row::Margins => unit.format(c.setup.margin),
            Row::Gap => unit.format(self.print.free_layout().gap),
            Row::Width => unit.format(c.width),
            _ => String::new(),
        });
        let Some(d) = self.print.dialog.as_mut() else {
            return;
        };
        if d.busy.is_some() {
            return;
        }
        let mut le = LineEdit::new(&text);
        if !replace {
            le.select_all();
        }
        d.typing = Some((row, le));
        if row != Row::Saved {
            self.print_focus(Hit::Row(row));
        }
        self.redraw = true;
    }

    /// Keys while typing: Enter uses it, Escape drops it, Tab uses it and moves on.
    fn print_type_key(&mut self, combo: Option<&KeyCombo>, typed: Option<&str>) {
        if let Some(c) = combo {
            match c.key {
                Key::Named(NamedKey::Escape) => {
                    if let Some(d) = self.print.dialog.as_mut() {
                        d.typing = None;
                    }
                    return;
                }
                Key::Named(NamedKey::Enter) => {
                    self.print_type_commit(true);
                    return;
                }
                Key::Named(NamedKey::Tab) => {
                    self.print_type_commit(false);
                    return;
                }
                _ => {}
            }
            let rtl = self.rtl();
            if let Some(d) = self.print.dialog.as_mut()
                && let Some((_, le)) = d.typing.as_mut()
            {
                let mut clip = PendingClipboard::default();
                if le.key(c.key, c.mods, rtl, &mut clip).used() {
                    return;
                }
                if c.mods.ctrl || c.mods.alt || c.mods.meta {
                    return;
                }
            }
        }
        if let Some(t) = typed.filter(|t| !t.chars().any(char::is_control))
            && let Some(d) = self.print.dialog.as_mut()
            && let Some((_, le)) = d.typing.as_mut()
        {
            le.insert(t);
        }
    }

    /// Uses what was typed. A length that does not read says so and stays (`stay`) or is
    /// dropped. True when it was used.
    fn print_type_commit(&mut self, stay: bool) -> bool {
        let Some((row, text)) = self
            .print
            .dialog
            .as_ref()
            .and_then(|d| d.typing.as_ref())
            .map(|(r, le)| (*r, le.text().to_owned()))
        else {
            return false;
        };
        let now = self.now();
        let mut c = self.print_choices();
        let unit = if c.free { c.free_unit } else { c.unit };
        let ok = if row == Row::Saved {
            let name = text.trim();
            if name.is_empty() {
                false
            } else {
                self.print_save_layout(name.to_owned());
                true
            }
        } else if let Some(mm) = unit.parse(&text) {
            match row {
                Row::Margins => {
                    c.setup.margin = mm.clamp(0.0, MAX_MARGIN_MM);
                    self.print.choices = Some(c);
                    if c.free {
                        self.print_sync_free(Row::Margins);
                    }
                }
                Row::Gap => {
                    self.print.free_layout().gap = mm.clamp(0.0, MAX_MARGIN_MM);
                    self.print.free_edited = true;
                }
                Row::Width => {
                    let max = c.setup.printable().w.max(10.0);
                    c.width = mm.clamp(10.0, max);
                    c.fit = Fit::Size { width: c.width };
                    self.print.choices = Some(c);
                }
                _ => {}
            }
            true
        } else {
            false
        };
        if let Some(d) = self.print.dialog.as_mut()
            && (ok || !stay)
        {
            d.typing = None;
        }
        if ok {
            if row != Row::Saved {
                self.print_announce(row);
                self.print_relayout();
            }
        } else if stay {
            let msg = if row == Row::Saved {
                t!("print-layout-name-empty")
            } else {
                t!("print-length-invalid", unit = unit.symbol())
            };
            self.chrome.warning(msg, now);
        }
        self.redraw = true;
        ok
    }

    /// Keys on the focused page; false when not one of its keys.
    fn print_page_key(&mut self, combo: &KeyCombo) -> bool {
        let c = self.print_choices();
        let step = match c.free_unit {
            Unit::In => specular_print::layout::MM_PER_IN / 16.0,
            Unit::Mm | Unit::Cm => 1.0,
        };
        let step = if combo.mods.ctrl { step * 10.0 } else { step };
        let n = self.print.free.as_ref().map_or(0, |f| f.areas.len());
        let dir = match combo.key {
            Key::Named(NamedKey::ArrowLeft) => Some((-1.0, 0.0)),
            Key::Named(NamedKey::ArrowRight) => Some((1.0, 0.0)),
            Key::Named(NamedKey::ArrowUp) => Some((0.0, -1.0)),
            Key::Named(NamedKey::ArrowDown) => Some((0.0, 1.0)),
            _ => None,
        };
        match combo.key {
            Key::Named(NamedKey::Space | NamedKey::Enter) if !combo.mods.ctrl => {
                if n == 0 {
                    return true;
                }
                if let Some(d) = self.print.dialog.as_mut() {
                    let d_ = if combo.mods.shift { n - 1 } else { 1 };
                    d.selected = Some(d.selected.map_or(0, |i| (i + d_) % n));
                }
                self.print_announce_area();
                true
            }
            Key::Named(NamedKey::Delete | NamedKey::Backspace) => {
                self.print_remove_area();
                true
            }
            _ if dir.is_some() => {
                let Some(dir) = dir else { return false };
                let PrintState { dialog, free, .. } = &mut self.print;
                let (Some(d), Some(fl)) = (dialog.as_mut(), free.as_mut()) else {
                    return true;
                };
                if n == 0 {
                    return true;
                }
                let i = *d.selected.get_or_insert(0);
                let pan = combo.mods.alt.then(|| photo_in(d, fl, i)).flatten();
                if combo.mods.alt && pan.is_none() {
                    return true;
                }
                if editor::nudge(fl, i, dir, combo.mods.shift, pan, step) {
                    self.print.free_edited = true;
                    self.print_relayout();
                    self.print_announce_area();
                }
                true
            }
            _ => false,
        }
    }

    /// A press on the free layout's page: a handle or an area starts a drag (Shift on a
    /// filled photo moves its crop); elsewhere nothing is selected.
    fn print_press_page(&mut self, p: (f32, f32)) {
        let shift = self.mods.shift_key();
        self.print_focus(Hit::Page);
        let PrintState { dialog, free, .. } = &mut self.print;
        let (Some(d), Some(fl)) = (dialog.as_mut(), free.as_ref()) else {
            return;
        };
        if d.busy.is_some() {
            return;
        }
        match editor::grip_at(fl, d.selected, d.drawn.page, p) {
            Some((i, h)) => {
                let a = fl.areas[i];
                let grip = if h == specular_print::free::Handle::Move
                    && shift
                    && a.placement == Placement::Fill
                {
                    Grip::Pan
                } else {
                    Grip::Edge(h)
                };
                d.selected = Some(i);
                d.drag = Some(Drag {
                    area: i,
                    grip,
                    start: a.rect,
                    pan: a.pan,
                    from: p,
                });
            }
            None => d.selected = None,
        }
        self.redraw = true;
    }

    fn print_add_area(&mut self) {
        let i = self.print.free_layout().add_area();
        self.print.free_edited = true;
        if let Some(d) = self.print.dialog.as_mut() {
            d.selected = Some(i);
        }
        self.print_relayout();
        self.print_announce_area();
    }

    fn print_remove_area(&mut self) {
        let Some(i) = self.print.dialog.as_mut().and_then(|d| d.selected.take()) else {
            return;
        };
        let f = self.print.free_layout();
        if i < f.areas.len() {
            f.areas.remove(i);
            self.print.free_edited = true;
            self.chrome.announce_status(t!("print-area-removed"));
            self.print_relayout();
        }
    }

    /// Saves the free layout as `name` (replacing the user's layout of that name).
    fn print_save_layout(&mut self, name: String) {
        let mut l = self.print.free_layout().clone();
        l.name = name.clone();
        l.id.clear();
        let users = self.print.user_layouts();
        let idx = match users.iter().position(|u| u.name == name) {
            Some(i) => {
                users[i] = l.clone();
                i
            }
            None => {
                users.push(l.clone());
                users.len() - 1
            }
        };
        save_layouts(users);
        let builtin = specular_print::free::builtin(Paper::A4).len();
        self.print.free = Some(l);
        self.print.free_from = Some(builtin + idx);
        self.print.free_edited = false;
        let now = self.now();
        self.chrome.toast(
            t!("print-layout-saved", name = specular_i18n::isolate(&name)),
            None,
            now,
        );
        self.redraw = true;
    }

    /// Deletes the saved layout in use (after the dialog asked); the page keeps its areas.
    fn print_delete_layout(&mut self) {
        let Some(i) = self.print.user_layout_index() else {
            return;
        };
        let users = self.print.user_layouts();
        if i >= users.len() {
            return;
        }
        let gone = users.remove(i);
        save_layouts(users);
        self.print.free_from = None;
        if let Some(f) = self.print.free.as_mut() {
            f.name.clear();
        }
        if let Some(d) = self.print.dialog.as_mut() {
            d.confirm = false;
        }
        let now = self.now();
        self.chrome.toast(
            t!(
                "print-layout-deleted",
                name = specular_i18n::isolate(&gone.name)
            ),
            None,
            now,
        );
        self.print_focus(Hit::Page);
        self.redraw = true;
    }
}
