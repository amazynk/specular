//! Figures and key images (DESIGN.md §6.5).
//!
//! - **Export figure…** (`Ctrl+Shift+E`): the view as SVG, PDF, or 16-bit PNG or TIFF. The
//!   image part is the display path read back from the GPU ([`specular_figure::render`]) at
//!   1:1 unless a size is picked, with a vector scale bar from the image's calibration (its
//!   source shown in the dialog), a colour bar when a colour map or window is on, an inset
//!   of the last rectangle drawn, split R, G, B channels beside the merge, a title, the
//!   measurements as vectors, and the provenance (file, calibration, display settings,
//!   version) in the file's metadata and, when chosen, drawn under the figure.
//! - **Key images** (`K` marks or unmarks): the view is captured when marked. "Copy key
//!   images" puts them side by side on the clipboard as one image; "Export key images…"
//!   writes slides (PPTX, one per image, notes with the measurements) or a folder of PNGs.
//! - **Teaching file** (medical): the key images with nothing about the patient: titles
//!   from the modality only, burned-in text blacked out (the user's masks, else the text the
//!   detector finds; an image marked as having burned-in text and none found is left out),
//!   anonymised DICOM copies in a folder export ([`specular_dicom::anon`]), and case notes.
//!
//! Dialogs are drawn like the other sheets (a modal over a scrim) and last for the session.

pub(super) mod build;
mod capture;
#[cfg(test)]
mod shots;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use crossbeam_channel::Receiver;
use specular_gfx::{Align, DrawList, Icon, Rect, Text, TextStyle, draw_icon};
use specular_i18n::t;
use specular_input::{Key, KeyCombo, Mods, NamedKey};
use specular_theme::metrics::{FONT_LABEL, FONT_SMALL, FONT_TITLE, RADIUS_LG, RADIUS_SM};
use specular_theme::{Color, Palette};
use specular_widgets::accesskit;

use super::App;
pub(super) use build::{KeyImage, Measured, read_case};

/// What the measurement tool gives a figure: the calibration, the measurements and the last
/// rectangle drawn.
pub(super) type ForFigure = (
    specular_measure::calib::Calibration,
    Vec<Measured>,
    Option<(f64, f64, f64, f64)>,
);

const A11Y_BASE: u64 = 0x6400_0000_0000_0000;
/// Longest title or case notes typed.
const MAX_TEXT: usize = 400;

/// Which dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Figure,
    Keys,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Format {
    Svg,
    Pdf,
    Png,
    Tiff,
}

impl Format {
    const ALL: [Format; 4] = [Format::Svg, Format::Pdf, Format::Png, Format::Tiff];

    pub(super) fn ext(self) -> &'static str {
        match self {
            Format::Svg => "svg",
            Format::Pdf => "pdf",
            Format::Png => "png",
            Format::Tiff => "tif",
        }
    }
}

/// Which part of the image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Region {
    Visible,
    Whole,
}

/// The main panel's size: the image's own pixels, or a width (resampled).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Size {
    Pixels,
    Width(u32),
}

impl Size {
    const ALL: [Size; 3] = [Size::Pixels, Size::Width(1200), Size::Width(2400)];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum KeyFormat {
    Pptx,
    Folder,
    /// A teaching case folder (IHE TCE-style, for Radiopaedia's uploader): medical only,
    /// always anonymised.
    Case,
}

/// One line of a dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Row {
    Format,
    Region,
    Size,
    ScaleBar,
    ColourBar,
    Inset,
    Channels,
    Measurements,
    Title,
    Provenance,
    File,
    Images,
    KeyFormat,
    Preset,
    Overlays,
    Notes,
    Masks,
    Destination,
}

/// How a row is used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RowKind {
    /// ‹ value › stepped with the arrows.
    Step,
    /// A button (chooses a file or folder).
    Action,
    /// Typed text.
    Text,
    /// A fact, not a control.
    Info,
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

/// The figure dialog's choices, kept for the session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FigureChoices {
    pub format: Format,
    pub region: Region,
    pub size: Size,
    pub scale_bar: bool,
    pub colour_bar: bool,
    pub inset: bool,
    pub split: bool,
    pub marks: bool,
    pub title: String,
    /// Provenance drawn under the figure (it is always in the metadata).
    pub drawn: bool,
}

impl Default for FigureChoices {
    fn default() -> Self {
        FigureChoices {
            format: Format::Svg,
            region: Region::Visible,
            size: Size::Pixels,
            scale_bar: true,
            colour_bar: true,
            inset: false,
            split: false,
            marks: true,
            title: String::new(),
            drawn: false,
        }
    }
}

/// The key-image dialog's choices, kept for the session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct KeyChoices {
    pub format: KeyFormat,
    pub teaching: bool,
    pub overlays: bool,
    pub notes: String,
}

impl Default for KeyChoices {
    fn default() -> Self {
        KeyChoices {
            format: KeyFormat::Pptx,
            teaching: false,
            overlays: true,
            notes: String::new(),
        }
    }
}

/// What the image offers, found when the dialog opens: each optional part, or why not.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Avail {
    /// The calibration's source (shown), when there is one.
    pub calibration: Option<String>,
    /// The colour bar's description ("viridis, 0–4095"), when a map or window is on.
    pub legend: Option<String>,
    pub inset: bool,
    /// The image is in colour (it splits into R, G, B).
    pub split: bool,
    pub marks: usize,
    /// The whole image's full-resolution pixels are on the GPU (1:1 is exact).
    pub full_res: bool,
    /// The view is at 1:1 or closer, so the visible area's full-resolution tiles are.
    pub zoomed_in: bool,
    /// A DICOM image (teaching export applies).
    pub medical: bool,
    /// Key images in the set.
    pub keys: usize,
}

/// What the draw function shows for one row.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct RowView {
    pub row: Row,
    pub kind: RowKind,
    pub label: String,
    pub value: String,
    /// Steppable (an unavailable part shows why instead).
    pub enabled: bool,
}

/// Everything the dialog draws (so screenshots need no app).
#[derive(Clone, Debug)]
pub(super) struct DialogView {
    pub title: String,
    pub rows: Vec<RowView>,
    /// Under the rows: a note (teaching) or why Export is off.
    pub note: Option<String>,
    pub status: Option<String>,
    pub busy: bool,
    pub can_export: bool,
    /// Index into [`DialogView::focusables`].
    pub focus: Option<usize>,
    /// An input method's composition in the focused text row (empty: none).
    pub preedit: String,
}

impl DialogView {
    /// The keyboard focus order: the rows that are controls, then Cancel and Export.
    pub fn focusables(&self) -> Vec<Hit> {
        self.rows
            .iter()
            .filter(|r| r.kind != RowKind::Info)
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
    pub note: Option<Rect>,
    /// Where the focused text row's input method candidate window opens.
    pub ime: Option<Rect>,
}

fn on_off(on: bool) -> String {
    if on {
        t!("figure-on")
    } else {
        t!("figure-off")
    }
}

fn row_label(row: Row) -> String {
    match row {
        Row::Format | Row::KeyFormat => t!("figure-format"),
        Row::Region => t!("figure-region"),
        Row::Size => t!("figure-size"),
        Row::ScaleBar => t!("figure-scale-bar"),
        Row::ColourBar => t!("figure-colour-bar"),
        Row::Inset => t!("figure-inset"),
        Row::Channels => t!("figure-channels"),
        Row::Measurements => t!("figure-measurements"),
        Row::Title => t!("figure-title-row"),
        Row::Provenance => t!("figure-provenance"),
        Row::File => t!("figure-file"),
        Row::Images => t!("figure-keys-images"),
        Row::Preset => t!("figure-keys-preset"),
        Row::Overlays => t!("figure-measurements"),
        Row::Notes => t!("figure-keys-notes"),
        Row::Masks => t!("figure-keys-masks"),
        Row::Destination => t!("figure-keys-destination"),
    }
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

/// The figure dialog's rows.
pub(super) fn figure_rows(c: &FigureChoices, a: &Avail, file: Option<&Path>) -> Vec<RowView> {
    let step = |row, value: String, enabled| RowView {
        row,
        kind: RowKind::Step,
        label: row_label(row),
        value,
        enabled,
    };
    let mut v = vec![
        step(
            Row::Format,
            match c.format {
                // Names, the same in every language.
                Format::Svg => "SVG".to_owned(),
                Format::Pdf => "PDF".to_owned(),
                Format::Png => t!("figure-format-png"),
                Format::Tiff => t!("figure-format-tiff"),
            },
            true,
        ),
        step(
            Row::Region,
            match c.region {
                Region::Visible => t!("figure-region-visible"),
                Region::Whole => t!("figure-region-whole"),
            },
            true,
        ),
        step(
            Row::Size,
            match c.size {
                Size::Pixels => t!("figure-size-pixels"),
                Size::Width(w) => t!("figure-size-width", width = i64::from(w)),
            },
            true,
        ),
        match &a.calibration {
            Some(src) if c.scale_bar => step(
                Row::ScaleBar,
                t!("figure-on-from", source = src.clone()),
                true,
            ),
            Some(_) => step(Row::ScaleBar, on_off(false), true),
            None => step(Row::ScaleBar, t!("figure-scale-bar-none"), false),
        },
        match &a.legend {
            Some(d) if c.colour_bar => step(
                Row::ColourBar,
                t!("figure-on-from", source = d.clone()),
                true,
            ),
            Some(_) => step(Row::ColourBar, on_off(false), true),
            None => step(Row::ColourBar, t!("figure-colour-bar-none"), false),
        },
        if a.inset {
            step(
                Row::Inset,
                if c.inset {
                    t!("figure-inset-rectangle")
                } else {
                    on_off(false)
                },
                true,
            )
        } else {
            step(Row::Inset, t!("figure-inset-none"), false)
        },
        if a.split {
            step(
                Row::Channels,
                if c.split {
                    t!("figure-channels-split")
                } else {
                    t!("figure-channels-merged")
                },
                true,
            )
        } else {
            step(Row::Channels, t!("figure-channels-one"), false)
        },
        if a.marks > 0 {
            step(
                Row::Measurements,
                if c.marks {
                    t!("figure-measurements-on", count = a.marks as i64)
                } else {
                    on_off(false)
                },
                true,
            )
        } else {
            step(Row::Measurements, t!("figure-measurements-none"), false)
        },
        RowView {
            row: Row::Title,
            kind: RowKind::Text,
            label: row_label(Row::Title),
            value: c.title.clone(),
            enabled: true,
        },
        step(
            Row::Provenance,
            if c.drawn {
                t!("figure-provenance-drawn")
            } else {
                t!("figure-provenance-metadata")
            },
            true,
        ),
    ];
    v.push(RowView {
        row: Row::File,
        kind: RowKind::Action,
        label: row_label(Row::File),
        value: file.map_or_else(|| t!("figure-choose"), file_name),
        enabled: true,
    });
    v
}

/// The key-image dialog's rows.
pub(super) fn key_rows(c: &KeyChoices, a: &Avail, dest: Option<&Path>) -> Vec<RowView> {
    let step = |row, value: String| RowView {
        row,
        kind: RowKind::Step,
        label: row_label(row),
        value,
        enabled: true,
    };
    let mut v = vec![
        RowView {
            row: Row::Images,
            kind: RowKind::Info,
            label: row_label(Row::Images),
            value: t!("figure-keys-count", count = a.keys as i64),
            enabled: true,
        },
        step(
            Row::KeyFormat,
            match c.format {
                KeyFormat::Pptx => t!("figure-keys-pptx"),
                KeyFormat::Folder => t!("figure-keys-folder"),
                KeyFormat::Case => t!("figure-keys-case"),
            },
        ),
    ];
    if a.medical && c.format != KeyFormat::Case {
        v.push(step(
            Row::Preset,
            if c.teaching {
                t!("figure-keys-teaching")
            } else {
                t!("figure-keys-standard")
            },
        ));
    }
    v.push(step(
        Row::Overlays,
        if c.overlays {
            t!("figure-keys-overlays-on")
        } else {
            on_off(false)
        },
    ));
    if (c.teaching || c.format == KeyFormat::Case) && a.medical {
        v.push(RowView {
            row: Row::Masks,
            kind: RowKind::Info,
            label: row_label(Row::Masks),
            value: t!("figure-keys-masks-auto"),
            enabled: true,
        });
        v.push(RowView {
            row: Row::Notes,
            kind: RowKind::Text,
            label: row_label(Row::Notes),
            value: c.notes.clone(),
            enabled: true,
        });
    }
    v.push(RowView {
        row: Row::Destination,
        kind: RowKind::Action,
        label: row_label(Row::Destination),
        value: dest.map_or_else(|| t!("figure-choose"), file_name),
        enabled: true,
    });
    v
}

/// Draws a dialog over a scrim in a window of `size` logical px.
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

    let w = (ww - 32.0).clamp(280.0, 580.0);
    let inner = w - 2.0 * pad;
    let title = text.layout(&v.title, title_st, Some(inner), Align::Start);
    let note = v
        .note
        .as_ref()
        .map(|s| text.layout(s, small_st, Some(inner - 28.0), Align::Start));
    let bh = 32.0;
    let status_h = if v.status.is_some() { 26.0 } else { 0.0 };
    let note_h = note.as_ref().map_or(0.0, |b| b.height.max(18.0) + 14.0);
    let fixed = pad + title.height + 14.0 + note_h + status_h + 14.0 + bh + pad;
    let max_h = (wh - 32.0).max(200.0);
    let row_h = ((max_h - fixed) / v.rows.len().max(1) as f32).clamp(22.0, 36.0);
    let h = (fixed + row_h * v.rows.len() as f32).min(max_h);
    let sheet = Rect::new(((ww - w) * 0.5).round(), ((wh - h) * 0.5).round(), w, h);
    dl.shadow(sheet, RADIUS_LG, pal.shadow, 40.0, 14.0);
    dl.rect_border(sheet, RADIUS_LG, pal.surface, pal.glass_border, 1.0);
    let m = |r: Rect| if rtl { r.mirror_x(ww) } else { r };
    let start_x = |x: f32, bw: f32| if rtl { ww - x - bw } else { x };
    text.draw(
        dl,
        &title,
        start_x(sheet.x + pad, title.width),
        sheet.y + pad,
        pal.text,
    );
    let focus = v.focus.and_then(|i| v.focusables().get(i).copied());
    let mut drawn = Drawn {
        sheet,
        ..Drawn::default()
    };
    let rows_top = sheet.y + pad + title.height + 14.0;
    let arrow = (row_h - 6.0).min(26.0);
    for (i, rv) in v.rows.iter().enumerate() {
        let r = Rect::new(sheet.x + pad, rows_top + i as f32 * row_h, inner, row_h);
        let ink = if v.busy || !rv.enabled {
            pal.text_faint
        } else {
            pal.text
        };
        let focused = focus == Some(Hit::Row(rv.row));
        if focused && rv.kind != RowKind::Text {
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
        let sw = (inner * 0.58).min(300.0);
        let sx = r.right() - sw;
        // The focused text row shows the composition at its end, underlined.
        let composing = (focused && rv.kind == RowKind::Text && !v.busy && !v.preedit.is_empty())
            .then(|| specular_widgets::line_edit::compose(&rv.value, rv.value.len(), &v.preedit));
        let val = match &composing {
            Some((shown, _)) => text.layout(shown, value_st, None, Align::Start),
            None => text.layout(&rv.value, value_st, None, Align::Start),
        };
        match rv.kind {
            RowKind::Action => {
                let b = Rect::new(sx, r.y + (row_h - arrow) * 0.5, sw, arrow);
                let bm = m(b);
                dl.rect_border(bm, RADIUS_SM, pal.surface_raised, pal.separator, 1.0);
                let ib = m(Rect::new(b.right() - arrow, b.y, arrow, arrow));
                draw_icon(dl, Icon::Folder, ib.inset(arrow * 0.22), ink, 1.6, rtl);
                dl.push_clip(m(Rect::new(b.x + 8.0, b.y, sw - 8.0 - arrow, arrow)));
                text.draw(
                    dl,
                    &val,
                    start_x(b.x + 8.0, val.width),
                    b.y + (arrow - val.height) * 0.5,
                    ink,
                );
                dl.pop_clip();
            }
            RowKind::Text => {
                let b = Rect::new(sx, r.y + (row_h - arrow) * 0.5, sw, arrow);
                let bm = m(b);
                let border = if focused {
                    pal.focus_ring
                } else {
                    pal.separator
                };
                dl.rect_border(
                    bm,
                    RADIUS_SM,
                    pal.surface_raised,
                    border,
                    if focused { 2.0 } else { 1.0 },
                );
                let room = sw - 16.0;
                dl.push_clip(m(Rect::new(b.x + 8.0, b.y, room, arrow)));
                // The end of a long text stays in sight.
                let shift = (val.width - room + 4.0).max(0.0);
                let tx = start_x(b.x + 8.0 - shift, val.width);
                let ty = b.y + (arrow - val.height) * 0.5;
                text.draw(dl, &val, tx, ty, ink);
                if focused && !v.busy {
                    // After the last letter typed (or composed), where bidi puts it.
                    let end = composing.as_ref().map_or(rv.value.len(), |c| c.1.end);
                    let cx = if val.is_empty() {
                        if rtl { tx - 2.0 } else { tx + 1.0 }
                    } else {
                        tx + val.caret_x(end)
                    };
                    let caret = Rect::new(cx, b.y + 5.0, 1.5, arrow - 10.0);
                    dl.rect(caret, 0.0, pal.text);
                    drawn.ime = Some(match &composing {
                        Some((_, range)) => {
                            specular_widgets::line_edit::draw_preedit_underline(
                                dl,
                                &val,
                                tx,
                                range.clone(),
                                ty + val.height,
                                pal.text_muted,
                            );
                            specular_widgets::line_edit::ime_area(
                                &val,
                                tx,
                                range.clone(),
                                caret.y,
                                caret.h,
                                m(Rect::new(b.x + 8.0, b.y, room, arrow)),
                            )
                        }
                        None => caret,
                    });
                }
                dl.pop_clip();
            }
            RowKind::Info => {
                dl.push_clip(m(Rect::new(sx, r.y, sw, row_h)));
                text.draw(
                    dl,
                    &val,
                    start_x(r.right() - val.width.min(sw), val.width),
                    r.y + (row_h - val.height) * 0.5,
                    pal.text,
                );
                dl.pop_clip();
            }
            RowKind::Step => {
                let dec = Rect::new(sx, r.y + (row_h - arrow) * 0.5, arrow, arrow);
                let inc = Rect::new(r.right() - arrow, dec.y, arrow, arrow);
                if rv.enabled {
                    for (b, icon, hit) in [
                        (dec, Icon::ChevronLeft, Hit::Dec(rv.row)),
                        (inc, Icon::ChevronRight, Hit::Inc(rv.row)),
                    ] {
                        let b = m(b);
                        dl.rect(b, RADIUS_SM, pal.surface_raised);
                        draw_icon(dl, icon, b.inset(arrow * 0.18), ink, 1.6, rtl);
                        drawn.hits.push((b, hit));
                    }
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
        }
        if rv.kind != RowKind::Info {
            drawn.hits.push((m(r), Hit::Row(rv.row)));
        }
        drawn.rows.push((m(r), rv.row));
    }
    let mut y = rows_top + v.rows.len() as f32 * row_h + 10.0;
    if let Some(b) = &note {
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
        drawn.note = Some(m(box_));
        y = box_.bottom() + 6.0;
    }
    let by = sheet.bottom() - pad - bh;
    if let Some(s) = &v.status {
        let b = text.layout(s, small_st, Some(inner), Align::Start);
        let sy = (by - 10.0 - status_h).max(y);
        text.draw(dl, &b, start_x(sheet.x + pad, b.width), sy, pal.text_muted);
    }
    let labels = [
        (Hit::Export, t!("figure-export")),
        (Hit::Cancel, t!("app-cancel")),
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

/// The "Key image" badge at the bottom start corner of the image area (logical px).
pub(super) fn draw_badge(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    area: Rect,
    rtl: bool,
    ts: f32,
) {
    let st = TextStyle::new(FONT_SMALL * ts).weight(600.0);
    let b = text.layout(&t!("figure-key-badge"), st, None, Align::Start);
    let (h, icon) = (b.height + 10.0, b.height);
    let w = b.width + icon + 24.0;
    let r = Rect::new(area.x + 12.0, area.bottom() - 12.0 - h, w, h);
    let r = if rtl {
        Rect::new(area.right() - 12.0 - w, r.y, w, h)
    } else {
        r
    };
    dl.rect(r, h * 0.5, pal.surface.with_alpha(0.85));
    let ib = Rect::new(r.x + 8.0, r.y + 5.0, icon, icon);
    let tx = if rtl { r.x + 8.0 } else { ib.right() + 6.0 };
    let ib = if rtl {
        Rect::new(r.right() - 8.0 - icon, ib.y, icon, icon)
    } else {
        ib
    };
    draw_icon(dl, Icon::Check, ib, pal.accent, 1.8, rtl);
    text.draw(dl, &b, tx, r.y + 5.0, pal.text);
}

fn bounds(r: Rect, scale: f64) -> accesskit::Rect {
    accesskit::Rect {
        x0: f64::from(r.x) * scale,
        y0: f64::from(r.y) * scale,
        x1: f64::from(r.x + r.w) * scale,
        y1: f64::from(r.y + r.h) * scale,
    }
}

/// The dialog's AccessKit nodes: a modal dialog with a spin button per option, a text input
/// per typed row, a button per file row, labels for facts, the note, the status and Cancel /
/// Export.
pub(super) fn a11y(tree: &mut accesskit::TreeUpdate, v: &DialogView, d: &Drawn, scale: f64) {
    use accesskit::{Action, Node, NodeId, Role};
    let mut dialog = Node::new(Role::Dialog);
    dialog.set_modal();
    dialog.set_label(v.title.clone());
    dialog.set_bounds(bounds(d.sheet, scale));
    let mut ids = Vec::new();
    let mut nodes = Vec::new();
    let focusables = v.focusables();
    let focus_hit = v.focus.and_then(|i| focusables.get(i).copied());
    let mut focus_id = None;
    for (i, rv) in v.rows.iter().enumerate() {
        let id = NodeId(A11Y_BASE + 1 + i as u64);
        let mut n = match rv.kind {
            RowKind::Action => {
                let mut n = Node::new(Role::Button);
                if !v.busy {
                    n.add_action(Action::Click);
                }
                n
            }
            RowKind::Text => {
                let mut n = Node::new(Role::TextInput);
                if v.busy {
                    n.set_read_only();
                }
                n
            }
            RowKind::Info => Node::new(Role::Label),
            RowKind::Step => {
                let mut n = Node::new(Role::SpinButton);
                if rv.enabled && !v.busy {
                    n.add_action(Action::Increment);
                    n.add_action(Action::Decrement);
                } else {
                    n.set_disabled();
                }
                n
            }
        };
        n.set_label(rv.label.clone());
        n.set_value(rv.value.clone());
        if let Some((r, _)) = d.rows.iter().find(|(_, row)| *row == rv.row) {
            n.set_bounds(bounds(*r, scale));
        }
        if rv.kind != RowKind::Info {
            n.add_action(Action::Focus);
        }
        if focus_hit == Some(Hit::Row(rv.row)) {
            focus_id = Some(id);
        }
        ids.push(id);
        nodes.push((id, n));
    }
    if let (Some(s), Some(r)) = (&v.note, d.note) {
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
            Hit::Cancel => t!("app-cancel"),
            _ => t!("figure-export"),
        });
        n.set_bounds(bounds(r, scale));
        if hit == Hit::Cancel || (v.can_export && !v.busy) {
            n.add_action(Action::Click);
        } else {
            n.set_disabled();
        }
        n.add_action(Action::Focus);
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
        let rv = &v.rows[k as usize - 1];
        return (rv.kind != RowKind::Info).then_some(Hit::Row(rv.row));
    }
    let k = k.checked_sub(0x101)? as usize;
    buttons(d).get(k).map(|b| b.1)
}

/// Cancel and Export as drawn, in reading order.
fn buttons(d: &Drawn) -> Vec<(Rect, Hit)> {
    [Hit::Cancel, Hit::Export]
        .iter()
        .filter_map(|o| d.hits.iter().find(|(_, h)| h == o).copied())
        .collect()
}

/// Steps through `all` from `cur` by `d`.
fn cycle<T: Copy + PartialEq>(all: &[T], cur: T, d: i32) -> T {
    let i = all.iter().position(|x| *x == cur).unwrap_or(0) as i32;
    all[(i + d).rem_euclid(all.len() as i32) as usize]
}

// ---------------------------------------------------------------------------------------------
// State.

/// What a finished export says.
pub(super) enum Done {
    Saved { name: String, skipped: usize },
    Failed(String),
}

/// The open dialog.
pub(super) struct Dialog {
    pub kind: Kind,
    pub avail: Avail,
    picking: Option<Receiver<Option<PathBuf>>>,
    export_after_pick: bool,
    job: Option<Receiver<capture::Reply>>,
    pub focus: Option<usize>,
    drawn: Drawn,
    /// The image's file (its folder and name suggest the output's).
    source: Option<PathBuf>,
    /// An input method's composition for the focused text row.
    preedit: String,
}

/// The app's figure and key-image state.
#[derive(Default)]
pub(super) struct FigureState {
    pub dialog: Option<Dialog>,
    pub figure: FigureChoices,
    pub keys_choices: KeyChoices,
    /// The figure's file and the key images' file or folder, chosen in the dialogs.
    file: Option<PathBuf>,
    dest: Option<PathBuf>,
    /// The key images, in the order they were marked.
    pub keys: Vec<KeyImage>,
    /// The text system that shapes figure labels into outlines (kept between exports).
    text: Option<Text>,
}

impl std::fmt::Debug for FigureState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FigureState")
            .field("open", &self.dialog.as_ref().map(|d| d.kind))
            .field("keys", &self.keys.len())
            .finish_non_exhaustive()
    }
}

impl FigureState {
    pub(super) fn visible(&self) -> bool {
        self.dialog.is_some()
    }

    /// Whether `path` page `frame` is a key image.
    pub(super) fn is_key(&self, path: &Path, frame: u32) -> bool {
        self.keys.iter().any(|k| k.path == path && k.frame == frame)
    }

    fn view(&self) -> Option<DialogView> {
        let d = self.dialog.as_ref()?;
        let busy = d.job.is_some();
        let (title, rows, note, can_export) = match d.kind {
            Kind::Figure => {
                let rows = figure_rows(&self.figure, &d.avail, self.file.as_deref());
                let c = &self.figure;
                let exact = c.size != Size::Pixels
                    || d.avail.full_res
                    || (c.region == Region::Visible && d.avail.zoomed_in);
                let note = (!exact).then(|| t!("figure-not-full-res"));
                (t!("figure-dialog-title"), rows, note, exact)
            }
            Kind::Keys => {
                let teaching = self.keys_choices.teaching && d.avail.medical;
                let rows = key_rows(&self.keys_choices, &d.avail, self.dest.as_deref());
                let note = if d.avail.keys == 0 {
                    Some(t!("figure-keys-empty"))
                } else if teaching {
                    Some(t!("figure-keys-teaching-note"))
                } else {
                    None
                };
                let title = if teaching {
                    t!("figure-teaching-title")
                } else {
                    t!("figure-keys-title")
                };
                (title, rows, note, d.avail.keys > 0)
            }
        };
        Some(DialogView {
            title,
            rows,
            note,
            status: busy.then(|| t!("figure-working")),
            busy,
            can_export: can_export && d.picking.is_none(),
            focus: d.focus,
            preedit: d.preedit.clone(),
        })
    }

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

    pub(super) fn a11y(&self, tree: &mut accesskit::TreeUpdate, v: &DialogView, scale: f64) {
        if let Some(d) = &self.dialog {
            a11y(tree, v, &d.drawn, scale);
        }
    }

    /// A text row (title, notes) has the keyboard: input method commits go to it.
    pub(super) fn wants_text(&self) -> bool {
        self.view().is_some_and(|v| {
            !v.busy
                && matches!(
                    v.focus.and_then(|i| v.focusables().get(i).copied()),
                    Some(Hit::Row(Row::Title | Row::Notes))
                )
        })
    }

    /// An input method's composition (empty: none), kept while a text row has the keyboard.
    pub(super) fn set_preedit(&mut self, s: &str) {
        let wants = self.wants_text();
        if let Some(d) = self.dialog.as_mut() {
            d.preedit = if wants { s.to_owned() } else { String::new() };
        }
    }

    /// Where the input method's candidate window opens (the focused text row's caret).
    pub(super) fn ime_area(&self) -> Option<Rect> {
        self.wants_text()
            .then(|| self.dialog.as_ref()?.drawn.ime)
            .flatten()
    }
}

impl App {
    /// A background task panicked: the dialog stops waiting for it.
    pub(super) fn figure_lost(&mut self, task: &str) {
        if let Some(d) = self.figure.dialog.as_mut() {
            match task {
                "figure-export" => d.job = None,
                "dlg-figure" => d.picking = None,
                _ => {}
            }
        }
    }

    fn figure_view(&self) -> Option<DialogView> {
        self.figure.view()
    }

    /// Opens the figure dialog (`Kind::Figure`) or the key-image dialog, with the teaching
    /// preset on when `teaching`.
    pub(super) fn figure_open(&mut self, kind: Kind, teaching: bool) {
        if self.figure.dialog.is_some() {
            return;
        }
        let avail = match kind {
            Kind::Figure => match self.figure_avail() {
                Ok(a) => a,
                Err(e) => {
                    let now = self.now();
                    self.chrome.warning(e, now);
                    return;
                }
            },
            Kind::Keys => Avail {
                keys: self.figure.keys.len(),
                medical: self.figure.keys.iter().any(|k| k.dicom) || self.medical_state().is_some(),
                ..Avail::default()
            },
        };
        if kind == Kind::Keys {
            self.figure.keys_choices.teaching = teaching;
        }
        let source = self.current.and_then(|i| self.entries.get(i)).cloned();
        self.figure.dialog = Some(Dialog {
            kind,
            avail,
            picking: None,
            export_after_pick: false,
            job: None,
            focus: Some(0),
            drawn: Drawn::default(),
            source,
            preedit: String::new(),
        });
        if let Some(v) = self.figure_view() {
            self.chrome.announce_status(v.title);
        }
        self.redraw = true;
    }

    fn figure_close(&mut self) {
        self.figure.dialog = None;
        self.redraw = true;
    }

    /// Replies from the file chooser and the export.
    pub(super) fn figure_tick(&mut self) {
        let mut picked = None;
        let mut done = None;
        if let Some(d) = self.figure.dialog.as_mut() {
            if let Some(rx) = &d.picking
                && let Ok(p) = rx.try_recv()
            {
                d.picking = None;
                picked = Some((p, std::mem::take(&mut d.export_after_pick), d.kind));
                self.redraw = true;
            }
            if let Some(rx) = &d.job
                && let Ok((r, text)) = rx.try_recv()
            {
                d.job = None;
                done = Some(r);
                if text.is_some() {
                    self.figure.text = text;
                }
            }
        }
        if let Some((p, after, kind)) = picked
            && let Some(p) = p
        {
            self.chrome.announce_status(file_name(&p));
            match kind {
                Kind::Figure => {
                    // The extension follows the format.
                    let p = p.with_extension(self.figure.figure.format.ext());
                    self.figure.file = Some(p);
                }
                Kind::Keys => self.figure.dest = Some(p),
            }
            if after {
                self.figure_export();
            }
        }
        if let Some(r) = done {
            let now = self.now();
            match r {
                Done::Saved { name, skipped } => {
                    if skipped > 0 {
                        self.chrome.warning(
                            t!("figure-saved-skipped", name = name, count = skipped as i64),
                            now,
                        );
                    } else {
                        self.chrome
                            .toast(t!("figure-saved", name = name), None, now);
                    }
                    let to = self
                        .figure
                        .file
                        .clone()
                        .or_else(|| self.figure.dest.clone());
                    let paths: Vec<&std::path::Path> = to.as_deref().into_iter().collect();
                    let extra = [("skipped", serde_json::json!(skipped))];
                    self.audit(super::audit::Event::Export, "figure", &paths, &extra);
                    self.figure_close();
                }
                Done::Failed(e) => {
                    self.chrome.warning(t!("figure-failed", reason = e), now);
                }
            }
            self.redraw = true;
        }
    }

    /// Asks for the figure's file or the key images' file or folder.
    fn figure_pick(&mut self, then_export: bool) {
        let Some(d) = self.figure.dialog.as_mut() else {
            return;
        };
        if d.picking.is_some() || d.job.is_some() {
            return;
        }
        d.export_after_pick = then_export;
        let dir = d
            .source
            .as_ref()
            .and_then(|p| p.parent().map(Path::to_owned));
        let stem = d
            .source
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "figure".into());
        let teaching = self.figure.keys_choices.teaching && d.avail.medical;
        let (folder, name, ext) = match d.kind {
            Kind::Figure => {
                let ext = self.figure.figure.format.ext();
                (false, format!("{stem}-figure.{ext}"), ext)
            }
            Kind::Keys
                if matches!(
                    self.figure.keys_choices.format,
                    KeyFormat::Folder | KeyFormat::Case
                ) =>
            {
                (true, String::new(), "")
            }
            // A teaching file's name says nothing about the patient.
            Kind::Keys if teaching => (false, "teaching-file.pptx".into(), "pptx"),
            Kind::Keys => (false, format!("{stem}-key-images.pptx"), "pptx"),
        };
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("dlg-figure", move |l| {
            let mut dlg = rfd::FileDialog::new();
            if let Some(dir) = dir {
                dlg = dlg.set_directory(dir);
            }
            let picked = if folder {
                dlg.pick_folder()
            } else {
                dlg.set_file_name(name).add_filter(ext, &[ext]).save_file()
            };
            let _ = tx.send(picked);
            l.waker()();
        });
        d.picking = Some(rx);
    }

    fn figure_export(&mut self) {
        let Some(d) = self.figure.dialog.as_ref() else {
            return;
        };
        if d.job.is_some() || d.picking.is_some() {
            return;
        }
        let kind = d.kind;
        let target = match kind {
            Kind::Figure => self.figure.file.clone(),
            Kind::Keys => self.figure.dest.clone(),
        };
        let Some(target) = target else {
            self.figure_pick(true);
            return;
        };
        let started = match kind {
            Kind::Figure => self.figure_export_figure(target),
            Kind::Keys => self.figure_export_keys(target),
        };
        match started {
            Ok(rx) => {
                if let Some(d) = self.figure.dialog.as_mut() {
                    d.job = Some(rx);
                }
                self.chrome.announce_status(t!("figure-working"));
            }
            Err(e) => {
                let now = self.now();
                self.chrome.warning(t!("figure-failed", reason = e), now);
            }
        }
        self.redraw = true;
    }

    fn figure_step(&mut self, row: Row, delta: i32) {
        let Some(d) = self.figure.dialog.as_ref() else {
            return;
        };
        if d.job.is_some() {
            return;
        }
        let a = d.avail.clone();
        let c = &mut self.figure.figure;
        let k = &mut self.figure.keys_choices;
        let changed = match row {
            Row::Format => {
                c.format = cycle(&Format::ALL, c.format, delta);
                if let Some(f) = self.figure.file.as_mut() {
                    *f = f.with_extension(c.format.ext());
                }
                true
            }
            Row::Region => {
                c.region = cycle(&[Region::Visible, Region::Whole], c.region, delta);
                true
            }
            Row::Size => {
                c.size = cycle(&Size::ALL, c.size, delta);
                true
            }
            Row::ScaleBar if a.calibration.is_some() => {
                c.scale_bar = !c.scale_bar;
                true
            }
            Row::ColourBar if a.legend.is_some() => {
                c.colour_bar = !c.colour_bar;
                true
            }
            Row::Inset if a.inset => {
                c.inset = !c.inset;
                true
            }
            Row::Channels if a.split => {
                c.split = !c.split;
                true
            }
            Row::Measurements if a.marks > 0 => {
                c.marks = !c.marks;
                true
            }
            Row::Provenance => {
                c.drawn = !c.drawn;
                true
            }
            Row::KeyFormat => {
                let all: &[KeyFormat] = if a.medical {
                    &[KeyFormat::Pptx, KeyFormat::Folder, KeyFormat::Case]
                } else {
                    &[KeyFormat::Pptx, KeyFormat::Folder]
                };
                k.format = cycle(all, k.format, delta);
                // A file and a folder are not interchangeable.
                self.figure.dest = None;
                true
            }
            Row::Preset if a.medical => {
                k.teaching = !k.teaching;
                self.figure.dest = None;
                true
            }
            Row::Overlays => {
                k.overlays = !k.overlays;
                true
            }
            Row::File | Row::Destination => {
                self.figure_pick(false);
                false
            }
            _ => false,
        };
        if changed && let Some(v) = self.figure_view() {
            if let Some(i) = v.focusables().iter().position(|h| *h == Hit::Row(row))
                && let Some(d) = self.figure.dialog.as_mut()
            {
                d.focus = Some(i);
            }
            if let Some(rv) = v.rows.iter().find(|r| r.row == row) {
                self.chrome.announce_status(rv.value.clone());
            }
        }
        self.redraw = true;
    }

    fn figure_text(&mut self) -> Option<&mut String> {
        let d = self.figure.dialog.as_ref()?;
        let v = self.figure.view()?;
        match v.focus.and_then(|i| v.focusables().get(i).copied()) {
            Some(Hit::Row(Row::Title)) if d.job.is_none() => Some(&mut self.figure.figure.title),
            Some(Hit::Row(Row::Notes)) if d.job.is_none() => {
                Some(&mut self.figure.keys_choices.notes)
            }
            _ => None,
        }
    }

    fn figure_hit(&mut self, h: Hit) {
        match h {
            Hit::Dec(r) => self.figure_step(r, -1),
            Hit::Inc(r) => self.figure_step(r, 1),
            Hit::Row(r) => {
                if let Some(v) = self.figure_view()
                    && let Some(i) = v.focusables().iter().position(|x| *x == Hit::Row(r))
                    && let Some(d) = self.figure.dialog.as_mut()
                {
                    d.focus = Some(i);
                }
                if matches!(r, Row::File | Row::Destination) {
                    self.figure_pick(false);
                }
            }
            Hit::Cancel => {
                if self.figure.dialog.as_ref().is_some_and(|d| d.job.is_none()) {
                    self.figure_close();
                }
            }
            Hit::Export => self.figure_export(),
        }
        self.redraw = true;
    }

    /// Keys while a dialog is shown (it takes them all). `text` is what the key typed.
    pub(super) fn figure_key(&mut self, combo: Option<&KeyCombo>, text: Option<&str>) -> bool {
        let Some(v) = self.figure_view() else {
            return false;
        };
        let focusables = v.focusables();
        let n = focusables.len();
        let focus = v.focus.and_then(|i| focusables.get(i).copied());
        let typing = matches!(focus, Some(Hit::Row(Row::Title | Row::Notes)));
        let rtl = self.rtl();
        let move_focus = |me: &mut App, d: i32| {
            if let Some(dl) = me.figure.dialog.as_mut() {
                let i = dl
                    .focus
                    .map_or(0, |i| (i as i32 + d).rem_euclid(n as i32) as usize);
                dl.focus = Some(i);
            }
        };
        self.redraw = true;
        let Some(combo) = combo else {
            if typing && let (Some(t), Some(s)) = (text, self.figure_text()) {
                push_text(s, t);
            }
            return true;
        };
        let plain = combo.mods == Mods::NONE;
        let shortcut = combo.mods.ctrl || combo.mods.meta || combo.mods.alt;
        match combo.key {
            Key::Named(NamedKey::Escape) => self.figure_hit(Hit::Cancel),
            Key::Named(NamedKey::Tab) if combo.mods.shift => move_focus(self, -1),
            Key::Named(NamedKey::Tab) => move_focus(self, 1),
            Key::Named(NamedKey::ArrowUp) if plain => move_focus(self, -1),
            Key::Named(NamedKey::ArrowDown) if plain => move_focus(self, 1),
            Key::Named(NamedKey::Backspace) if typing => {
                if let Some(s) = self.figure_text() {
                    s.pop();
                }
            }
            Key::Named(k @ (NamedKey::ArrowLeft | NamedKey::ArrowRight)) if plain => {
                let forward = (k == NamedKey::ArrowRight) != rtl;
                match focus {
                    Some(Hit::Row(r)) if !typing && !matches!(r, Row::File | Row::Destination) => {
                        self.figure_step(r, if forward { 1 } else { -1 })
                    }
                    _ if typing => {}
                    _ => move_focus(self, if forward { 1 } else { -1 }),
                }
            }
            Key::Named(NamedKey::Enter) => match focus {
                Some(h @ (Hit::Cancel | Hit::Export)) => self.figure_hit(h),
                Some(Hit::Row(r @ (Row::File | Row::Destination))) => self.figure_hit(Hit::Row(r)),
                _ => self.figure_hit(Hit::Export),
            },
            Key::Named(NamedKey::Space) if !typing => match focus {
                Some(h @ (Hit::Cancel | Hit::Export)) => self.figure_hit(h),
                Some(Hit::Row(r @ (Row::File | Row::Destination))) => self.figure_hit(Hit::Row(r)),
                Some(Hit::Row(r)) => self.figure_step(r, 1),
                _ => {}
            },
            _ if typing && !shortcut => {
                let typed = text.map(str::to_owned).or_else(|| match combo.key {
                    Key::Char(c) => Some(c.to_string()),
                    Key::Named(NamedKey::Space) => Some(" ".into()),
                    _ => None,
                });
                if let (Some(t), Some(s)) = (typed, self.figure_text()) {
                    push_text(s, &t);
                }
            }
            _ => {}
        }
        true
    }

    /// A click while a dialog is shown (it takes them all).
    pub(super) fn figure_click(&mut self, p: (f32, f32), down: bool) -> bool {
        let Some(d) = self.figure.dialog.as_ref() else {
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
            self.figure_hit(h);
        }
        true
    }

    /// A screen reader's request on a dialog's nodes; false when not ours.
    pub(super) fn figure_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        let Some(v) = self.figure_view() else {
            return false;
        };
        let Some(d) = self.figure.dialog.as_ref() else {
            return false;
        };
        let Some(hit) = a11y_target(&v, &d.drawn, req.target_node.0) else {
            return req.target_node.0 & !0xFFFF == A11Y_BASE;
        };
        match (req.action, hit) {
            (accesskit::Action::Increment, Hit::Row(r)) => self.figure_step(r, 1),
            (accesskit::Action::Decrement, Hit::Row(r)) => self.figure_step(r, -1),
            (accesskit::Action::Click, h) => self.figure_hit(h),
            (accesskit::Action::Focus, h) => {
                if let Some(i) = v.focusables().iter().position(|x| *x == h)
                    && let Some(d) = self.figure.dialog.as_mut()
                {
                    d.focus = Some(i);
                }
            }
            (accesskit::Action::SetValue, Hit::Row(Row::Title | Row::Notes)) => {
                if let Some(accesskit::ActionData::Value(s)) = &req.data
                    && let Some(t) = self.figure_text()
                {
                    t.clear();
                    push_text(t, s);
                }
            }
            _ => {}
        }
        self.redraw = true;
        true
    }

    /// Marks the current image as a key image, or unmarks it.
    pub(super) fn figure_toggle_key(&mut self) {
        let now = self.now();
        let Some(path) = self.current.and_then(|i| self.entries.get(i)).cloned() else {
            return;
        };
        let frame = self.figure_frame();
        if let Some(i) = self
            .figure
            .keys
            .iter()
            .position(|k| k.path == path && k.frame == frame)
        {
            self.figure.keys.remove(i);
            let msg = t!("figure-key-unmarked", count = self.figure.keys.len() as i64);
            self.chrome.toast(msg, None, now);
            self.redraw = true;
            return;
        }
        match self.figure_capture_key(path, frame) {
            Ok(k) => {
                self.figure.keys.push(k);
                let msg = t!("figure-key-marked", count = self.figure.keys.len() as i64);
                self.chrome.toast(msg, None, now);
            }
            Err(e) => {
                self.chrome.warning(t!("figure-failed", reason = e), now);
            }
        }
        self.redraw = true;
    }

    /// Whether the image on screen is a key image (its badge shows).
    pub(super) fn figure_current_is_key(&self) -> bool {
        if self.figure.keys.is_empty() || self.figure.visible() {
            return false;
        }
        self.current
            .and_then(|i| self.entries.get(i))
            .is_some_and(|p| self.figure.is_key(p, self.figure_frame()))
    }

    /// Copies the key images to the clipboard as one picture (medical ones masked).
    pub(super) fn figure_copy_keys(&mut self) {
        let now = self.now();
        if self.figure.keys.is_empty() {
            self.chrome.toast(t!("figure-keys-empty"), None, now);
            return;
        }
        let keys = self.figure.keys.clone();
        let masks = self.figure_user_masks();
        let clip = self.clipboard.clone();
        self.loader.background("figure-copy", move |l| {
            let with: Vec<(&KeyImage, Vec<specular_dicom::mask::Rect>)> = keys
                .iter()
                .map(|k| {
                    // Medical images go through the masks: the user's, else the text found.
                    let mut a = capture::masks_for(k, &masks);
                    if k.dicom && a.is_empty() {
                        a = capture::detected(k);
                    }
                    (k, a)
                })
                .collect();
            let px = build::tiled(&with, true, 120_000_000);
            clip.put_rgba(
                l,
                px.width as usize,
                px.height as usize,
                build::rgba8(&px),
                crate::loader::Copied::KeyImages,
            );
        });
    }

    /// Screenshots (`--show`): `figure`, `figure-keys`, `figure-teaching`.
    pub(super) fn figure_show(&mut self, s: &str) -> bool {
        match s {
            "figure" => self.figure_open(Kind::Figure, false),
            "figure-keys" => self.figure_open(Kind::Keys, false),
            "figure-teaching" => self.figure_open(Kind::Keys, true),
            _ => return false,
        }
        true
    }
}

/// Appends typed text, dropping control characters and keeping to [`MAX_TEXT`].
fn push_text(s: &mut String, t: &str) {
    for c in t.chars().filter(|c| !c.is_control()) {
        if s.chars().count() >= MAX_TEXT {
            break;
        }
        s.push(c);
    }
}
