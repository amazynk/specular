//! Convert… (DESIGN.md §6.5): the palette's File group, the gallery's selection or the image
//! in the viewer, written as new files in another format.
//!
//! A modal sheet drawn like the print dialog: the format and its quality or effort, the size
//! and bit depth in the start column; colour, HDR, metadata, the output folder and what to do
//! with a name that is taken in the end column; the file name pattern under them with a live
//! preview of the first image's new name. Convert starts [`specular_convert::run`] on a thread
//! of its own (its work on efficiency cores); the sheet then shows the count, the time left
//! and Stop, and can be hidden, the progress going on in a toast. At the end it lists what
//! could not be converted.
//!
//! The choices are settings (`[convert]` in settings.toml), saved when a batch starts or the
//! dialog closes.

#[cfg(test)]
mod shots;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use specular_convert::{
    Batch, ColorMode, Conflict, Depth, Failure, Format, HdrMode, MetaMode, NameFacts, Options,
    OutputDir, PatternError, Progress, Report, Resize,
};
use specular_core::{CancelToken, JobPool};
use specular_gfx::{Align, DrawList, Icon, Rect, Text, TextBlock, TextStyle, draw_icon};
use specular_i18n::t;
use specular_input::{Key, KeyCombo, Mods, NamedKey};
use specular_settings::{
    Convert, ConvertColour, ConvertConflict, ConvertDepth, ConvertFormat, ConvertHdr,
    ConvertMetadata, ConvertResize, ConvertTiffCompression, Origin, SettingEnum,
};
use specular_theme::metrics::{FONT_LABEL, FONT_SMALL, FONT_TITLE, RADIUS_LG, RADIUS_SM};
use specular_theme::{Color, Palette};
use specular_widgets::accesskit;
use specular_widgets::line_edit::{LineEdit, PendingClipboard};

use super::App;

const A11Y_BASE: u64 = 0x5200_0000_0000_0000;
/// The start column's rows the sheet has room for (the most any format shows).
const START_ROWS: usize = 9;
/// Pixel sizes the width, height and long edge step through.
const SIZES: [i64; 27] = [
    256, 320, 480, 512, 640, 720, 800, 960, 1024, 1080, 1200, 1280, 1440, 1600, 1920, 2048, 2160,
    2400, 2560, 3000, 3200, 3840, 4096, 5120, 6000, 7680, 8192,
];
/// Failures listed at the end before "and N more".
const MAX_FAILURES: usize = 8;

/// Which images.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Source {
    Current,
    Folder,
    Selection,
    Picks,
}

/// One stepper row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Row {
    Images,
    Format,
    Quality,
    Chroma,
    Effort,
    Compression,
    Depth,
    Resize,
    Width,
    Height,
    LongEdge,
    Percent,
    Upscale,
    Colour,
    Hdr,
    Metadata,
    Folder,
    Conflicts,
    /// Develop export: the preset, and the colour space.
    Preset,
    Space,
}

/// What a click or a screen reader hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Hit {
    Dec(Row),
    Inc(Row),
    Row(Row),
    Choose,
    /// Develop export: keep the choices as a preset.
    SavePreset,
    /// Develop export: delete the user's preset the choices are (asks first).
    DeletePreset,
    Name,
    Cancel,
    Start,
    Stop,
    Hide,
    Close,
}

/// Where the files go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FolderMode {
    Subfolder,
    Same,
    Chosen,
}

/// The dialog's choices: the `[convert]` settings, with the subfolder name and the chosen
/// folder kept while the other is picked.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Choices {
    pub s: Convert,
    pub sub_name: String,
    pub chosen: Option<PathBuf>,
}

/// The next option of a setting enum, wrapping.
pub(super) fn cycle<T: SettingEnum>(v: &mut T, d: i32) {
    let all = T::ALL;
    let i = all.iter().position(|x| x == v).unwrap_or(0);
    *v = all[(i as i32 + d).rem_euclid(all.len() as i32) as usize];
}

/// `v` stepped to the next multiple of 5 in direction `d`, within `lo..=hi`.
pub(super) fn step5(v: i64, d: i32, lo: i64, hi: i64) -> i64 {
    let n = if d > 0 {
        (v.div_euclid(5) + 1) * 5
    } else {
        (v - 1).div_euclid(5) * 5
    };
    n.clamp(lo, hi)
}

/// A quality that steps by 5 up to 100 and then to lossless (and back down from it).
pub(super) fn step_lossy(lossless: &mut bool, q: &mut i64, d: i32) {
    if *lossless {
        if d < 0 {
            *lossless = false;
            *q = 100;
        }
    } else if d > 0 && *q >= 100 {
        *lossless = true;
    } else {
        *q = step5(*q, d, 1, 100);
    }
}

/// `v` stepped to the next of [`SIZES`] in direction `d` (unchanged past the ends).
pub(super) fn step_size(v: i64, d: i32) -> i64 {
    if d > 0 {
        SIZES.iter().copied().find(|&s| s > v).unwrap_or(v)
    } else {
        SIZES.iter().rev().copied().find(|&s| s < v).unwrap_or(v)
    }
}

impl Choices {
    pub(super) fn new(s: Convert) -> Choices {
        let sub_name = if s.subfolder.trim().is_empty() {
            "converted".to_owned()
        } else {
            s.subfolder.clone()
        };
        let chosen = s.folder.clone();
        Choices {
            s,
            sub_name,
            chosen,
        }
    }

    pub(super) fn format(&self) -> Format {
        match self.s.format {
            ConvertFormat::Jpeg => Format::Jpeg,
            ConvertFormat::Png => Format::Png,
            ConvertFormat::Webp => Format::Webp,
            ConvertFormat::Jxl => Format::Jxl,
            ConvertFormat::Avif => Format::Avif,
            ConvertFormat::Tiff => Format::Tiff,
        }
    }

    pub(super) fn folder_mode(&self) -> FolderMode {
        if self.s.folder.is_some() {
            FolderMode::Chosen
        } else if self.s.subfolder.trim().is_empty() {
            FolderMode::Same
        } else {
            FolderMode::Subfolder
        }
    }

    fn set_folder_mode(&mut self, m: FolderMode) {
        match m {
            FolderMode::Subfolder => {
                self.s.folder = None;
                self.s.subfolder = self.sub_name.clone();
            }
            FolderMode::Same => {
                self.s.folder = None;
                self.s.subfolder = String::new();
            }
            FolderMode::Chosen => self.s.folder = self.chosen.clone(),
        }
    }

    /// A folder picked with Choose folder….
    pub(super) fn choose(&mut self, p: PathBuf) {
        self.chosen = Some(p);
        self.set_folder_mode(FolderMode::Chosen);
    }

    /// The start column: the images, the format and its own rows, the size.
    pub(super) fn start_rows(&self) -> Vec<Row> {
        let mut v = vec![Row::Images, Row::Format];
        match self.s.format {
            ConvertFormat::Jpeg => v.extend([Row::Quality, Row::Chroma]),
            ConvertFormat::Png => v.push(Row::Effort),
            // Lossless WebP has a single speed.
            ConvertFormat::Webp if self.s.webp_lossless => v.push(Row::Quality),
            ConvertFormat::Webp | ConvertFormat::Jxl | ConvertFormat::Avif => {
                v.extend([Row::Quality, Row::Effort])
            }
            ConvertFormat::Tiff => v.push(Row::Compression),
        }
        if self.format().sixteen() {
            v.push(Row::Depth);
        }
        v.push(Row::Resize);
        match self.s.resize {
            ConvertResize::None => {}
            ConvertResize::Fit => v.extend([Row::Width, Row::Height]),
            ConvertResize::LongEdge => v.push(Row::LongEdge),
            ConvertResize::Percent => v.push(Row::Percent),
        }
        if self.s.resize != ConvertResize::None {
            v.push(Row::Upscale);
        }
        debug_assert!(v.len() <= START_ROWS);
        v
    }

    /// The end column: colour, metadata and where the files go.
    pub(super) fn end_rows(&self) -> Vec<Row> {
        vec![
            Row::Colour,
            Row::Hdr,
            Row::Metadata,
            Row::Folder,
            Row::Conflicts,
        ]
    }

    /// Steps `row` by `d` (±1); false when nothing changed.
    pub(super) fn step(&mut self, row: Row, d: i32) -> bool {
        let before = self.clone();
        let s = &mut self.s;
        match row {
            Row::Images => {}
            Row::Format => cycle(&mut s.format, d),
            Row::Quality => match s.format {
                ConvertFormat::Webp => step_lossy(&mut s.webp_lossless, &mut s.webp_quality, d),
                ConvertFormat::Jxl => step_lossy(&mut s.jxl_lossless, &mut s.jxl_quality, d),
                ConvertFormat::Avif => s.avif_quality = step5(s.avif_quality, d, 1, 100),
                _ => s.jpeg_quality = step5(s.jpeg_quality, d, 1, 100),
            },
            Row::Chroma => s.jpeg_full_chroma = !s.jpeg_full_chroma,
            Row::Effort => {
                let e = match s.format {
                    ConvertFormat::Png => &mut s.png_effort,
                    ConvertFormat::Webp => &mut s.webp_effort,
                    ConvertFormat::Avif => &mut s.avif_effort,
                    _ => &mut s.jxl_effort,
                };
                *e = (*e + i64::from(d)).clamp(1, 9);
            }
            Row::Compression => cycle(&mut s.tiff_compression, d),
            Row::Depth => cycle(&mut s.depth, d),
            Row::Resize => cycle(&mut s.resize, d),
            Row::Width => s.width = step_size(s.width, d),
            Row::Height => s.height = step_size(s.height, d),
            Row::LongEdge => s.long_edge = step_size(s.long_edge, d),
            Row::Percent => s.percent = step5(s.percent, d, 5, 400),
            Row::Upscale => s.upscale = !s.upscale,
            Row::Colour => cycle(&mut s.colour, d),
            Row::Hdr => cycle(&mut s.hdr, d),
            Row::Metadata => cycle(&mut s.metadata, d),
            Row::Folder => {
                let mut modes = vec![FolderMode::Subfolder, FolderMode::Same];
                if self.chosen.is_some() {
                    modes.push(FolderMode::Chosen);
                }
                let i = modes
                    .iter()
                    .position(|m| *m == self.folder_mode())
                    .unwrap_or(0);
                let m = modes[(i as i32 + d).rem_euclid(modes.len() as i32) as usize];
                self.set_folder_mode(m);
            }
            Row::Conflicts => cycle(&mut s.conflicts, d),
            // Develop export's own rows.
            Row::Preset | Row::Space => {}
        }
        *self != before
    }

    /// The engine's options for these choices.
    pub(super) fn options(&self) -> Options {
        let s = &self.s;
        let px = |v: i64| v.clamp(1, 65535) as u32;
        Options {
            format: self.format(),
            jpeg_quality: s.jpeg_quality.clamp(1, 100) as u8,
            jpeg_full_chroma: s.jpeg_full_chroma,
            png_effort: s.png_effort.clamp(1, 9) as u8,
            webp_quality: (!s.webp_lossless).then(|| s.webp_quality.clamp(1, 100) as u8),
            webp_effort: s.webp_effort.clamp(1, 9) as u8,
            jxl_quality: (!s.jxl_lossless).then(|| s.jxl_quality.clamp(1, 100) as u8),
            jxl_effort: s.jxl_effort.clamp(1, 9) as u8,
            avif_quality: s.avif_quality.clamp(1, 100) as u8,
            avif_effort: s.avif_effort.clamp(1, 9) as u8,
            tiff_deflate: s.tiff_compression == ConvertTiffCompression::Deflate,
            depth: match s.depth {
                ConvertDepth::Auto => Depth::Auto,
                ConvertDepth::Eight => Depth::Eight,
                ConvertDepth::Sixteen => Depth::Sixteen,
            },
            resize: match s.resize {
                ConvertResize::None => Resize::None,
                ConvertResize::Fit => Resize::Fit {
                    width: px(s.width),
                    height: px(s.height),
                },
                ConvertResize::LongEdge => Resize::LongEdge(px(s.long_edge)),
                ConvertResize::Percent => Resize::Percent(s.percent.clamp(1, 400) as u32),
            },
            upscale: s.upscale,
            color: match s.colour {
                ConvertColour::Keep => ColorMode::Keep,
                ConvertColour::Srgb => ColorMode::Srgb,
            },
            hdr: match s.hdr {
                ConvertHdr::Keep => HdrMode::Keep,
                ConvertHdr::ToneMap => HdrMode::ToneMap,
            },
            metadata: match s.metadata {
                ConvertMetadata::All => MetaMode::All,
                ConvertMetadata::NoLocation => MetaMode::NoLocation,
                ConvertMetadata::Strip => MetaMode::Strip,
            },
            output: match &s.folder {
                Some(p) => OutputDir::Folder(p.clone()),
                None => OutputDir::Subfolder(s.subfolder.clone()),
            },
            pattern: s.pattern.clone(),
            conflict: match s.conflicts {
                ConvertConflict::Suffix => Conflict::Suffix,
                ConvertConflict::Skip => Conflict::Skip,
            },
        }
    }
}

/// What the draw function shows for one row.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct RowView {
    pub row: Row,
    pub label: String,
    pub value: String,
}

/// What the sheet shows.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum PhaseView {
    Setup {
        start: Vec<RowView>,
        end: Vec<RowView>,
        name: String,
        caret: usize,
        selection: (usize, usize),
        /// The first image's new name, or what is wrong with the pattern.
        preview: Result<String, String>,
        /// What an input method is composing in the name field (IME preedit).
        preedit: String,
    },
    Running {
        status: String,
        progress: f32,
        stopping: bool,
    },
    Done {
        summary: String,
        lines: Vec<String>,
    },
}

/// Everything the dialog draws (so screenshots need no app).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct DialogView {
    pub phase: PhaseView,
    /// Index into [`focusables`](Self::focusables).
    pub focus: Option<usize>,
    /// Develop's Export… (its title, its start button, Save preset).
    pub export: bool,
    /// Develop's Export…: Delete preset's label, while the choices are a user's preset.
    pub delete_preset: Option<String>,
}

impl DialogView {
    /// The keyboard focus order.
    pub fn focusables(&self) -> Vec<Hit> {
        match &self.phase {
            PhaseView::Setup { start, end, .. } => start
                .iter()
                .chain(end)
                .map(|r| Hit::Row(r.row))
                .chain([Hit::Choose])
                .chain(self.export.then_some(Hit::SavePreset))
                .chain(self.delete_preset.is_some().then_some(Hit::DeletePreset))
                .chain([Hit::Name, Hit::Cancel, Hit::Start])
                .collect(),
            // As seen: Stop, then Hide (the default) at the end.
            PhaseView::Running { .. } => vec![Hit::Stop, Hit::Hide],
            PhaseView::Done { .. } => vec![Hit::Close],
        }
    }

    fn rows(&self) -> Vec<&RowView> {
        match &self.phase {
            PhaseView::Setup { start, end, .. } => start.iter().chain(end).collect(),
            _ => Vec::new(),
        }
    }

    fn can_start(&self) -> bool {
        matches!(&self.phase, PhaseView::Setup { preview: Ok(_), .. })
    }
}

/// Where things landed.
#[derive(Clone, Debug, Default)]
pub(super) struct Drawn {
    pub sheet: Rect,
    pub hits: Vec<(Rect, Hit)>,
    pub rows: Vec<(Rect, Row)>,
    /// The name field, and its text with the x it is drawn at.
    pub field: Option<Rect>,
    pub field_text: Option<(Arc<TextBlock>, f32)>,
    /// Where the input method's window opens: the composition and the caret.
    pub ime: Option<Rect>,
    /// The texts under the title (status, summary, report lines), for screen readers.
    pub texts: Vec<(Rect, String)>,
}

/// Mirrors LTR-laid-out rects for right-to-left.
#[derive(Clone, Copy)]
pub(super) struct Mirror {
    pub ww: f32,
    pub rtl: bool,
}

impl Mirror {
    pub fn r(self, r: Rect) -> Rect {
        if self.rtl { r.mirror_x(self.ww) } else { r }
    }
    /// The x of a block `w` wide whose start edge is at `x` (LTR coordinates).
    pub fn x(self, x: f32, w: f32) -> f32 {
        if self.rtl { self.ww - x - w } else { x }
    }
}

pub(super) struct Styles {
    pub title: TextStyle,
    pub label: TextStyle,
    pub value: TextStyle,
    pub small: TextStyle,
    pub button: TextStyle,
}

impl Styles {
    pub fn new(ts: f32) -> Styles {
        Styles {
            title: TextStyle::new(FONT_TITLE * ts).weight(650.0),
            label: TextStyle::new(FONT_LABEL * ts),
            value: TextStyle::new(FONT_LABEL * ts).weight(550.0),
            small: TextStyle::new(FONT_SMALL * ts),
            button: TextStyle::new(FONT_LABEL * ts).weight(550.0),
        }
    }
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
    let mi = Mirror { ww, rtl };
    let st = Styles::new(ts);
    let u = ts.max(1.0);
    let pad = 20.0;
    let row_h = (32.0 * u).round();
    let bh = (32.0 * u).round();
    let title = text.layout(&title_text(v.export), st.title, None, Align::Start);
    let focus = v.focus.and_then(|i| v.focusables().get(i).copied());
    let mut drawn = Drawn::default();

    match &v.phase {
        PhaseView::Setup {
            start,
            end,
            name,
            caret,
            selection,
            preview,
            preedit,
        } => {
            let field_area = 22.0 * u + row_h + 8.0 + 20.0 * u;
            let w = (ww - 32.0).clamp(200.0, 820.0);
            let h = (2.0 * pad
                + title.height
                + 12.0
                + START_ROWS as f32 * row_h
                + 14.0
                + field_area
                + 16.0
                + bh)
                .min(wh - 32.0)
                .max(200.0);
            let sheet = sheet_rect(dl, pal, ww, wh, w, h);
            drawn.sheet = sheet;
            text.draw(
                dl,
                &title,
                mi.x(sheet.x + pad, title.width),
                sheet.y + pad,
                pal.text,
            );
            let gap = 24.0;
            let col_w = (w - 2.0 * pad - gap) * 0.5;
            let rows_top = sheet.y + pad + title.height + 12.0;
            let xs = [sheet.x + pad, sheet.x + pad + col_w + gap];
            let arrow = stepper_arrow(row_h);
            for (col, rows) in [start, end].into_iter().enumerate() {
                // One stepper width per column, so the arrows line up.
                let (label_w, value_w) = rows.iter().fold((0.0f32, 0.0f32), |(l, v), rv| {
                    (
                        l.max(text.measure(&rv.label, st.label)),
                        v.max(text.measure(&rv.value, st.value)),
                    )
                });
                let sw = stepper_width(col_w, arrow, label_w, value_w);
                for (i, rv) in rows.iter().enumerate() {
                    let r = Rect::new(xs[col], rows_top + i as f32 * row_h, col_w, row_h);
                    let f = focus == Some(Hit::Row(rv.row));
                    draw_row(dl, text, pal, &st, mi, rv, r, sw, f, &mut drawn);
                }
            }
            // Choose folder…, under the end column's rows.
            let label = t!("convert-choose-folder");
            let b = text.layout(&label, st.button, None, Align::Start);
            let cb = Rect::new(
                xs[1] + 4.0,
                rows_top + end.len() as f32 * row_h + 8.0,
                (b.width + 28.0).max(84.0),
                bh - 4.0,
            );
            button(
                dl,
                text,
                pal,
                mi,
                &b,
                cb,
                false,
                true,
                focus == Some(Hit::Choose),
            );
            drawn.hits.push((mi.r(cb), Hit::Choose));
            // Develop export: Save preset, after Choose folder….
            if v.export {
                let b = text.layout(
                    &t!("develop-export-save-preset"),
                    st.button,
                    None,
                    Align::Start,
                );
                let sb = Rect::new(cb.right() + 8.0, cb.y, (b.width + 28.0).max(84.0), cb.h);
                button(
                    dl,
                    text,
                    pal,
                    mi,
                    &b,
                    sb,
                    false,
                    true,
                    focus == Some(Hit::SavePreset),
                );
                drawn.hits.push((mi.r(sb), Hit::SavePreset));
                if let Some(label) = &v.delete_preset {
                    let b = text.layout(label, st.button, None, Align::Start);
                    let db = Rect::new(sb.right() + 8.0, cb.y, (b.width + 28.0).max(84.0), cb.h);
                    button(
                        dl,
                        text,
                        pal,
                        mi,
                        &b,
                        db,
                        false,
                        true,
                        focus == Some(Hit::DeletePreset),
                    );
                    drawn.hits.push((mi.r(db), Hit::DeletePreset));
                }
            }

            // The name pattern and its preview.
            let ny = rows_top + START_ROWS as f32 * row_h + 14.0;
            let lab = text.layout(&t!("convert-name"), st.label, None, Align::Start);
            text.draw(
                dl,
                &lab,
                mi.x(sheet.x + pad + 4.0, lab.width),
                ny,
                pal.text_muted,
            );
            let field = Rect::new(sheet.x + pad, ny + 22.0 * u, w - 2.0 * pad, row_h);
            let focused = focus == Some(Hit::Name);
            let pre = if focused { preedit.as_str() } else { "" };
            let (block, x0, ime) = draw_field(
                dl, text, pal, &st, mi, field, name, *caret, *selection, focused, pre,
            );
            drawn.field = Some(mi.r(field));
            // Not while composing: a click would land in the composition.
            drawn.field_text = pre.is_empty().then_some((block, x0));
            drawn.ime = ime;
            drawn.hits.push((mi.r(field), Hit::Name));
            let (msg, ink) = match preview {
                Ok(s) => (s.as_str(), pal.text_muted),
                Err(e) => (e.as_str(), pal.danger),
            };
            let pb = text.layout(msg, st.small, None, Align::Start);
            let pr = Rect::new(
                field.x + 4.0,
                field.bottom() + 8.0,
                field.w - 8.0,
                pb.height,
            );
            dl.push_clip(mi.r(pr));
            text.draw(dl, &pb, mi.x(pr.x, pb.width.min(pr.w)), pr.y, ink);
            dl.pop_clip();
            drawn.texts.push((mi.r(pr), msg.to_owned()));

            let by = sheet.bottom() - pad - bh;
            let can_start = v.can_start();
            buttons(
                dl,
                text,
                pal,
                &st,
                mi,
                &[
                    (Hit::Start, start_text(v.export), can_start, true),
                    (Hit::Cancel, t!("app-cancel"), true, false),
                ],
                sheet.right() - pad,
                by,
                bh,
                focus,
                &mut drawn,
            );
        }
        PhaseView::Running {
            status,
            progress,
            stopping,
        } => {
            let w = (ww - 32.0).clamp(200.0, 520.0);
            let sb = text.layout(status, st.label, Some(w - 2.0 * pad), Align::Start);
            let h = (2.0 * pad + title.height + 16.0 + sb.height + 14.0 + 6.0 + 24.0 + bh)
                .min(wh - 32.0)
                .max(120.0);
            let sheet = sheet_rect(dl, pal, ww, wh, w, h);
            drawn.sheet = sheet;
            text.draw(
                dl,
                &title,
                mi.x(sheet.x + pad, title.width),
                sheet.y + pad,
                pal.text,
            );
            let y = sheet.y + pad + title.height + 16.0;
            text.draw(dl, &sb, mi.x(sheet.x + pad, sb.width), y, pal.text);
            drawn.texts.push((
                mi.r(Rect::new(sheet.x + pad, y, sb.width, sb.height)),
                status.clone(),
            ));
            let bar = Rect::new(sheet.x + pad, y + sb.height + 14.0, w - 2.0 * pad, 6.0);
            dl.rect(bar, 3.0, pal.surface_raised);
            let done = Rect::new(bar.x, bar.y, bar.w * progress.clamp(0.0, 1.0), bar.h);
            dl.rect(mi.r(done), 3.0, pal.accent);
            buttons(
                dl,
                text,
                pal,
                &st,
                mi,
                &[
                    (Hit::Hide, t!("convert-hide"), true, true),
                    (Hit::Stop, t!("convert-stop"), !stopping, false),
                ],
                sheet.right() - pad,
                sheet.bottom() - pad - bh,
                bh,
                focus,
                &mut drawn,
            );
        }
        PhaseView::Done { summary, lines } => {
            let w = (ww - 32.0).clamp(200.0, 560.0);
            let tw = w - 2.0 * pad;
            let sb = text.layout(summary, st.value, Some(tw), Align::Start);
            let blocks: Vec<_> = lines
                .iter()
                .map(|l| text.layout(l, st.label, Some(tw), Align::Start))
                .collect();
            let lines_h: f32 = blocks.iter().map(|b| b.height + 4.0).sum();
            let h = (2.0 * pad + title.height + 16.0 + sb.height + 10.0 + lines_h + 16.0 + bh)
                .min(wh - 32.0)
                .max(120.0);
            let sheet = sheet_rect(dl, pal, ww, wh, w, h);
            drawn.sheet = sheet;
            text.draw(
                dl,
                &title,
                mi.x(sheet.x + pad, title.width),
                sheet.y + pad,
                pal.text,
            );
            let mut y = sheet.y + pad + title.height + 16.0;
            text.draw(dl, &sb, mi.x(sheet.x + pad, sb.width), y, pal.text);
            drawn.texts.push((
                mi.r(Rect::new(sheet.x + pad, y, sb.width, sb.height)),
                summary.clone(),
            ));
            y += sb.height + 10.0;
            let by = sheet.bottom() - pad - bh;
            dl.push_clip(Rect::new(sheet.x, y, sheet.w, (by - 8.0 - y).max(0.0)));
            for (b, l) in blocks.iter().zip(lines) {
                text.draw(dl, b, mi.x(sheet.x + pad, b.width), y, pal.text_muted);
                drawn.texts.push((
                    mi.r(Rect::new(sheet.x + pad, y, b.width, b.height)),
                    l.clone(),
                ));
                y += b.height + 4.0;
            }
            dl.pop_clip();
            buttons(
                dl,
                text,
                pal,
                &st,
                mi,
                &[(Hit::Close, t!("convert-close"), true, true)],
                sheet.right() - pad,
                by,
                bh,
                focus,
                &mut drawn,
            );
        }
    }
    drawn
}

fn title_text(export: bool) -> String {
    if export {
        t!("develop-export-title")
    } else {
        t!("convert-title")
    }
}

fn start_text(export: bool) -> String {
    if export {
        t!("develop-export-start")
    } else {
        t!("convert-start")
    }
}

/// The sheet, centred, with its shadow.
pub(super) fn sheet_rect(
    dl: &mut DrawList,
    pal: &Palette,
    ww: f32,
    wh: f32,
    w: f32,
    h: f32,
) -> Rect {
    let sheet = Rect::new(((ww - w) * 0.5).round(), ((wh - h) * 0.5).round(), w, h);
    dl.shadow(sheet, RADIUS_LG, pal.shadow, 40.0, 14.0);
    dl.rect_border(sheet, RADIUS_LG, pal.surface, pal.glass_border, 1.0);
    sheet
}

/// Space between a row's label and its stepper, and around the label (logical px).
pub(super) const LABEL_GAP: f32 = 12.0;
/// Space between a stepper's value and its arrows.
pub(super) const VALUE_PAD: f32 = 6.0;

/// The side of a stepper's arrow buttons in a row `row_h` high.
pub(super) fn stepper_arrow(row_h: f32) -> f32 {
    (row_h - 6.0).min(26.0)
}

/// The width of a column's `‹ value ›` steppers: their usual share of the column, wider when
/// the longest value (`value_w`) needs it, as far as the longest label (`label_w`) still fits
/// beside them. A value longer still is cut with "…".
pub(super) fn stepper_width(col_w: f32, arrow: f32, label_w: f32, value_w: f32) -> f32 {
    let base = (col_w * 0.56).min(220.0);
    let room = (col_w - label_w - LABEL_GAP).max(base);
    (value_w + 2.0 * (arrow + VALUE_PAD)).clamp(base, room)
}

/// `s` cut with "…" to fit `max_w`.
pub(super) fn ellipsize(text: &mut Text, s: &str, style: TextStyle, max_w: f32) -> String {
    if text.measure(s, style) <= max_w {
        return s.to_owned();
    }
    let chars: Vec<char> = s.chars().collect();
    // An isolate cut open is closed after the ellipsis.
    let cut = |n: usize| {
        let s = chars[..n].iter().collect::<String>().trim_end().to_owned() + "…";
        specular_i18n::balance_isolates(&s)
    };
    let (mut lo, mut hi) = (0, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if text.measure(&cut(mid), style) <= max_w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    cut(lo)
}

/// A label and `‹ value ›` at the row's end, the stepper `sw` wide.
#[allow(clippy::too_many_arguments)]
fn draw_row(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    st: &Styles,
    mi: Mirror,
    rv: &RowView,
    r: Rect,
    sw: f32,
    focused: bool,
    drawn: &mut Drawn,
) {
    let row_h = r.h;
    if focused {
        dl.rect_border(
            mi.r(r.inset(-2.0)),
            RADIUS_SM,
            Color::TRANSPARENT,
            pal.focus_ring,
            2.0,
        );
    }
    let sx = r.right() - sw;
    let lab = text.layout(&rv.label, st.label, None, Align::Start);
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
    let arrow = stepper_arrow(row_h);
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
    let inner = sw - 2.0 * arrow;
    let value = ellipsize(text, &rv.value, st.value, inner - 2.0 * VALUE_PAD);
    let val = text.layout(&value, st.value, None, Align::Start);
    let mid = sx + arrow + (inner - val.width.min(inner)) * 0.5;
    dl.push_clip(mi.r(Rect::new(sx + arrow, r.y, inner, row_h)));
    text.draw(
        dl,
        &val,
        mi.x(mid, val.width),
        r.y + (row_h - val.height) * 0.5,
        pal.text,
    );
    dl.pop_clip();
    drawn.hits.push((mi.r(r), Hit::Row(rv.row)));
    drawn.rows.push((mi.r(r), rv.row));
}

/// The one-line name field; returns its text block and the x it is drawn at.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_field(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    st: &Styles,
    mi: Mirror,
    field: Rect,
    name: &str,
    caret: usize,
    selection: (usize, usize),
    focused: bool,
    preedit: &str,
) -> (Arc<TextBlock>, f32, Option<Rect>) {
    let fr = mi.r(field);
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
    let inner = fr.inset_xy(10.0, 0.0);
    // A composition sits at the caret, underlined; the caret follows it.
    let (shown, composing) = specular_widgets::line_edit::compose(name, caret, preedit);
    let block = text.layout(&shown, st.label, None, Align::Start);
    let cx = block.caret_x(composing.end);
    // The pattern is left-to-right text: it starts at the field's start edge while it fits,
    // and scrolls to keep the caret in view when it doesn't.
    let x0 = specular_widgets::line_edit::field_x0(&block, cx, inner, mi.rtl);
    let ty = fr.y + (fr.h - block.height) * 0.5;
    dl.push_clip(inner.inset_xy(-2.0, 0.0));
    if focused && preedit.is_empty() {
        // In visual order: a selection across a change of direction is in pieces.
        for (a, b) in block.range_spans(selection.0..selection.1) {
            dl.rect(
                Rect::new(x0 + a, ty, b - a, block.height),
                0.0,
                pal.selection,
            );
        }
    }
    text.draw(dl, &block, x0, ty, pal.text);
    if !preedit.is_empty() {
        specular_widgets::line_edit::draw_preedit_underline(
            dl,
            &block,
            x0,
            composing.clone(),
            ty + block.height,
            pal.text_muted,
        );
    }
    let mut ime = None;
    if focused {
        dl.rect(
            Rect::new((x0 + cx - 0.75).round(), ty, 1.5, block.height),
            0.0,
            pal.text,
        );
        ime = Some(specular_widgets::line_edit::ime_area(
            &block,
            x0,
            composing,
            ty,
            block.height,
            inner,
        ));
    }
    dl.pop_clip();
    (block, x0, ime)
}

/// A push button `r` (LTR coordinates) with its label laid out in `b`.
#[allow(clippy::too_many_arguments)]
pub(super) fn button(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    mi: Mirror,
    b: &TextBlock,
    r: Rect,
    default: bool,
    enabled: bool,
    focused: bool,
) {
    let rm = mi.r(r);
    let (fill, ink) = if default {
        (pal.accent_fill, pal.accent_text)
    } else {
        (pal.surface_raised, pal.text)
    };
    let fill = if enabled { fill } else { fill.with_alpha(0.45) };
    let ink = if enabled { ink } else { ink.with_alpha(0.5) };
    if default {
        dl.rect(rm, RADIUS_SM + 2.0, fill);
    } else {
        dl.rect_border(rm, RADIUS_SM + 2.0, fill, pal.separator, 1.0);
    }
    if focused {
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
        b,
        rm.x + (rm.w - b.width) * 0.5,
        rm.y + (rm.h - b.height) * 0.5,
        ink,
    );
}

/// A row of buttons ending at `end_x`, the first at the end: `(hit, label, enabled, default)`.
#[allow(clippy::too_many_arguments)]
fn buttons(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    st: &Styles,
    mi: Mirror,
    list: &[(Hit, String, bool, bool)],
    end_x: f32,
    y: f32,
    bh: f32,
    focus: Option<Hit>,
    drawn: &mut Drawn,
) {
    let mut x = end_x;
    for (hit, label, enabled, default) in list {
        let b = text.layout(label, st.button, None, Align::Start);
        let bw = (b.width + 28.0).max(84.0);
        x -= bw;
        let r = Rect::new(x, y, bw, bh);
        x -= 8.0;
        button(
            dl,
            text,
            pal,
            mi,
            &b,
            r,
            *default,
            *enabled,
            focus == Some(*hit),
        );
        drawn.hits.push((mi.r(r), *hit));
    }
}

/// The dialog's AccessKit nodes: a modal dialog with a spin button per row, the name field,
/// the texts and the buttons.
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
    dialog.set_label(title_text(v.export));
    dialog.set_bounds(bounds(d.sheet));
    let mut ids = Vec::new();
    let mut nodes = Vec::new();
    let focusables = v.focusables();
    let focus_hit = v.focus.and_then(|i| focusables.get(i).copied());
    let mut focus_id = None;
    for (i, rv) in v.rows().into_iter().enumerate() {
        let id = NodeId(A11Y_BASE + 1 + i as u64);
        let mut n = Node::new(Role::SpinButton);
        n.set_label(rv.label.clone());
        n.set_value(rv.value.clone());
        if let Some((r, _)) = d.rows.iter().find(|(_, row)| *row == rv.row) {
            n.set_bounds(bounds(*r));
        }
        n.add_action(accesskit::Action::Increment);
        n.add_action(accesskit::Action::Decrement);
        n.add_action(accesskit::Action::Focus);
        if focus_hit == Some(Hit::Row(rv.row)) {
            focus_id = Some(id);
        }
        ids.push(id);
        nodes.push((id, n));
    }
    // The texts: the name preview, the progress, the report.
    for (k, (r, s)) in d.texts.iter().enumerate() {
        let id = NodeId(A11Y_BASE + 0x200 + k as u64);
        let mut n = match v.phase {
            PhaseView::Running { progress, .. } => {
                let mut n = Node::new(Role::ProgressIndicator);
                n.set_numeric_value(f64::from(progress) * 100.0);
                n.set_min_numeric_value(0.0);
                n.set_max_numeric_value(100.0);
                n.set_live(accesskit::Live::Polite);
                n
            }
            PhaseView::Setup { .. } => {
                let mut n = Node::new(Role::Status);
                n.set_live(accesskit::Live::Polite);
                n
            }
            PhaseView::Done { .. } => Node::new(Role::Label),
        };
        n.set_label(s.clone());
        n.set_bounds(bounds(*r));
        ids.push(id);
        nodes.push((id, n));
    }
    for (k, hit) in focusables
        .iter()
        .filter(|h| !matches!(h, Hit::Row(_)))
        .enumerate()
    {
        let id = NodeId(A11Y_BASE + 0x100 + k as u64);
        let r = d.hits.iter().find(|(_, h)| h == hit).map(|x| x.0);
        let mut n = if *hit == Hit::Name {
            let mut n = Node::new(Role::TextInput);
            n.set_label(t!("convert-name"));
            if let PhaseView::Setup {
                name,
                caret,
                selection,
                ..
            } = &v.phase
            {
                // The selection in characters, from its anchor to the caret.
                let chars = |b: usize| name[..b.min(name.len())].chars().count();
                let anchor = if *caret == selection.0 {
                    selection.1
                } else {
                    selection.0
                };
                let a11y_sel = (chars(anchor), chars(*caret));
                let run_id = NodeId(A11Y_BASE + 0x1ff);
                let run = specular_widgets::text_run_sel(&mut n, run_id, name, Some(a11y_sel));
                n.set_children(vec![run_id]);
                nodes.push((run_id, run));
            }
            n.add_action(accesskit::Action::SetValue);
            n.add_action(accesskit::Action::SetTextSelection);
            n
        } else {
            let mut n = Node::new(Role::Button);
            n.set_label(match hit {
                Hit::Choose => t!("convert-choose-folder"),
                Hit::Cancel => t!("app-cancel"),
                Hit::Start => start_text(v.export),
                Hit::SavePreset => t!("develop-export-save-preset"),
                Hit::DeletePreset => v.delete_preset.clone().unwrap_or_default(),
                Hit::Stop => t!("convert-stop"),
                Hit::Hide => t!("convert-hide"),
                _ => t!("convert-close"),
            });
            let disabled = match hit {
                Hit::Start => !v.can_start(),
                Hit::Stop => matches!(v.phase, PhaseView::Running { stopping: true, .. }),
                _ => false,
            };
            if disabled {
                n.set_disabled();
            } else {
                n.add_action(accesskit::Action::Click);
            }
            n
        };
        if let Some(r) = r {
            n.set_bounds(bounds(r));
        }
        n.add_action(accesskit::Action::Focus);
        if focus_hit == Some(*hit) {
            focus_id = Some(id);
        }
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

/// The control node `id` stands for.
fn a11y_target(v: &DialogView, id: u64) -> Option<Hit> {
    let k = id.checked_sub(A11Y_BASE)?;
    let rows = v.rows();
    if (1..=rows.len() as u64).contains(&k) {
        return Some(Hit::Row(rows[k as usize - 1].row));
    }
    let k = k.checked_sub(0x100)?;
    if k >= 0x100 {
        return None;
    }
    v.focusables()
        .into_iter()
        .filter(|h| !matches!(h, Hit::Row(_)))
        .nth(k as usize)
}

pub(super) fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub(super) fn row_label(row: Row) -> String {
    match row {
        Row::Images => t!("convert-images"),
        Row::Format => t!("settings-convert-format"),
        Row::Quality => t!("convert-quality"),
        Row::Chroma => t!("convert-chroma"),
        Row::Effort => t!("convert-effort"),
        Row::Compression => t!("settings-convert-tiff-compression"),
        Row::Depth => t!("settings-convert-depth"),
        Row::Resize => t!("settings-convert-resize"),
        Row::Width => t!("settings-convert-width"),
        Row::Height => t!("settings-convert-height"),
        Row::LongEdge => t!("settings-convert-long-edge"),
        Row::Percent => t!("settings-convert-percent"),
        Row::Upscale => t!("settings-convert-upscale"),
        Row::Colour => t!("settings-convert-colour"),
        Row::Hdr => t!("settings-convert-hdr"),
        Row::Metadata => t!("settings-convert-metadata"),
        Row::Folder => t!("settings-convert-folder"),
        Row::Conflicts => t!("settings-convert-conflicts"),
        Row::Preset => t!("develop-export-preset"),
        Row::Space => t!("develop-export-space"),
    }
}

/// The Settings window's name for option `opt` of `[convert] key`.
pub(super) fn option_label(key: &str, opt: &str) -> String {
    specular_i18n::I18n::global().tr(&format!(
        "settings-convert-{}-{}",
        key.replace('_', "-"),
        opt.replace('_', "-")
    ))
}

pub(super) fn source_label(s: Source, n: usize) -> String {
    let n = n as i64;
    match s {
        Source::Current => t!("convert-images-current"),
        Source::Folder => t!("convert-images-folder", count = n),
        Source::Selection => t!("convert-images-selection", count = n),
        Source::Picks => t!("convert-images-picked", count = n),
    }
}

fn row_value(c: &Choices, row: Row, source: &str) -> String {
    let s = &c.s;
    let px = |v: i64| t!("convert-pixels", value = v);
    match row {
        Row::Images => source.to_owned(),
        Row::Format => option_label("format", s.format.as_str()),
        Row::Quality => match s.format {
            ConvertFormat::Webp if s.webp_lossless => t!("convert-lossless"),
            ConvertFormat::Jxl if s.jxl_lossless => t!("convert-lossless"),
            ConvertFormat::Webp => s.webp_quality.to_string(),
            ConvertFormat::Jxl => s.jxl_quality.to_string(),
            ConvertFormat::Avif => s.avif_quality.to_string(),
            _ => s.jpeg_quality.to_string(),
        },
        Row::Chroma if s.jpeg_full_chroma => t!("convert-chroma-full"),
        Row::Chroma => t!("convert-chroma-half"),
        Row::Effort => {
            let v = match s.format {
                ConvertFormat::Png => s.png_effort,
                ConvertFormat::Webp => s.webp_effort,
                ConvertFormat::Avif => s.avif_effort,
                _ => s.jxl_effort,
            };
            if v == 1 {
                t!("convert-effort-fastest", value = v)
            } else {
                v.to_string()
            }
        }
        Row::Compression => option_label("tiff_compression", s.tiff_compression.as_str()),
        Row::Depth => option_label("depth", s.depth.as_str()),
        Row::Resize => option_label("resize", s.resize.as_str()),
        Row::Width => px(s.width),
        Row::Height => px(s.height),
        Row::LongEdge => px(s.long_edge),
        Row::Percent => t!("unit-percent", value = s.percent),
        Row::Upscale if s.upscale => t!("convert-on"),
        Row::Upscale => t!("convert-off"),
        Row::Colour => option_label("colour", s.colour.as_str()),
        Row::Hdr => option_label("hdr", s.hdr.as_str()),
        Row::Metadata => option_label("metadata", s.metadata.as_str()),
        Row::Folder => match c.folder_mode() {
            FolderMode::Subfolder => t!("convert-folder-subfolder", name = c.sub_name.clone()),
            FolderMode::Same => t!("convert-folder-same"),
            FolderMode::Chosen => s
                .folder
                .as_deref()
                .map(|p| {
                    let n = file_name(p);
                    let n = if n.is_empty() {
                        p.to_string_lossy().into_owned()
                    } else {
                        n
                    };
                    specular_i18n::isolate(&n)
                })
                .unwrap_or_default(),
        },
        Row::Conflicts => option_label("conflicts", s.conflicts.as_str()),
        Row::Preset | Row::Space => String::new(),
    }
}

/// The rows as the dialog shows them.
pub(super) fn row_views(c: &Choices, rows: &[Row], source: &str) -> Vec<RowView> {
    rows.iter()
        .map(|&row| RowView {
            row,
            label: row_label(row),
            value: row_value(c, row, source),
        })
        .collect()
}

pub(super) fn pattern_error(e: &PatternError) -> String {
    match e {
        PatternError::Unclosed => t!("convert-name-error-unclosed"),
        PatternError::Stray => t!("convert-name-error-stray"),
        PatternError::Unknown(tok) => {
            t!("convert-name-error-unknown", token = format!("{{{tok}}}"))
        }
        PatternError::Format(tok) => t!("convert-name-error-format", token = format!("{{{tok}}}")),
        PatternError::Empty => t!("convert-name-error-empty"),
    }
}

/// The size and capture date of the first image, once read.
pub(super) use specular_convert::Probe;

/// "photo.heic → converted/photo.jpg" for the first image, or what is wrong with `pattern`.
pub(super) fn preview(
    c: &Choices,
    pattern: &str,
    first: Option<&Path>,
    probe: Option<Probe>,
) -> Result<String, String> {
    let mut o = c.options();
    o.pattern = pattern.to_owned();
    let resize = o.resize;
    let upscale = o.upscale;
    let ext = o.format.ext();
    let output = o.output.clone();
    let batch = Batch::new(Vec::new(), o).map_err(|e| pattern_error(&e))?;
    let Some(src) = first else {
        return Ok(String::new());
    };
    let stem = src
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (width, height) = probe.as_ref().map_or((0, 0), |&((w, h), ..)| {
        specular_convert::output_size(w, h, resize, upscale)
    });
    let name = batch.preview_name(&NameFacts {
        stem: &stem,
        index: 1,
        date: probe.as_ref().and_then(|p| p.1),
        width,
        height,
        ext,
        camera: probe.as_ref().and_then(|p| p.2.as_deref()),
        ..Default::default()
    });
    let out = match &output {
        OutputDir::Subfolder(s) if s.trim().is_empty() => name,
        _ => {
            let dir = specular_convert::output_dir(src, &output);
            format!("{}{}{name}", file_name(&dir), std::path::MAIN_SEPARATOR)
        }
    };
    Ok(t!(
        "convert-name-preview",
        source = file_name(src),
        output = out
    ))
}

/// A duration left, in seconds (by fives) under a minute, else in minutes.
pub(super) fn eta_text(d: Duration) -> String {
    let s = d.as_secs_f64();
    if s < 60.0 {
        let n = ((s / 5.0).ceil() * 5.0).max(5.0) as i64;
        t!("convert-eta-seconds", count = n)
    } else {
        t!("convert-eta-minutes", count = (s / 60.0).round() as i64)
    }
}

/// "Converting 3 of 20 · about 10 seconds left".
pub(super) fn progress_text(p: &Progress) -> String {
    progress_text_for(p, false)
}

/// [`progress_text`], or "Exporting 3 of 20 …" for Develop's export.
pub(super) fn progress_text_for(p: &Progress, export: bool) -> String {
    let (done, total) = (p.done as i64, p.total as i64);
    match p.eta {
        Some(eta) if p.done > 0 && p.done < p.total => {
            let time = eta_text(eta);
            if export {
                t!(
                    "develop-export-progress-eta",
                    done = done,
                    total = total,
                    time = time
                )
            } else {
                t!(
                    "convert-progress-eta",
                    done = done,
                    total = total,
                    time = time
                )
            }
        }
        _ if export => t!("develop-export-progress", done = done, total = total),
        _ => t!("convert-progress", done = done, total = total),
    }
}

pub(super) fn failure_text(f: &Failure) -> String {
    match f {
        Failure::Open(e) => t!("convert-failure-open", error = e.clone()),
        Failure::Decode(e) => t!("convert-failure-decode", error = e.clone()),
        Failure::Encode(e) => t!("convert-failure-encode", error = e.clone()),
        Failure::Write(e) => t!("convert-failure-write", error = e.clone()),
        Failure::SameAsSource(_) => t!("convert-failure-same"),
    }
}

/// The end report: a summary, then what was skipped or failed and why.
#[cfg(test)]
pub(super) fn report_lines(r: &Report) -> (String, Vec<String>) {
    report_lines_for(r, false)
}

/// [`report_lines`], in Develop export's words when `export`.
pub(super) fn report_lines_for(r: &Report, export: bool) -> (String, Vec<String>) {
    let converted = r.converted() as i64;
    let summary = match (r.cancelled, export) {
        (true, false) => t!("convert-stopped", count = converted),
        (false, false) => t!("convert-done", count = converted),
        (true, true) => t!("develop-export-stopped", count = converted),
        (false, true) => t!("develop-export-done", count = converted),
    };
    let mut lines = Vec::new();
    if r.skipped() > 0 {
        lines.push(t!("convert-done-skipped", count = r.skipped() as i64));
    }
    if r.failed() > 0 {
        lines.push(t!("convert-done-failed", count = r.failed() as i64));
    }
    for (p, f) in r.failures().take(MAX_FAILURES) {
        lines.push(t!(
            "convert-failure",
            file = file_name(p),
            reason = failure_text(f)
        ));
    }
    if r.failed() > MAX_FAILURES {
        lines.push(t!(
            "convert-report-more",
            count = (r.failed() - MAX_FAILURES) as i64
        ));
    }
    (summary, lines)
}

pub(super) enum Phase {
    Setup,
    Running,
    Done(Report),
}

/// The open dialog.
pub(super) struct Dialog {
    pub sources: Vec<(Source, Vec<PathBuf>)>,
    pub source: usize,
    pub choices: Choices,
    pub name: LineEdit,
    /// The first image's facts, and the image they are of.
    pub probe: Option<(PathBuf, Option<Probe>)>,
    pub probing: Option<(PathBuf, Receiver<Option<Probe>>)>,
    /// The folder dialog of Choose folder….
    pub picking: Option<Receiver<PathBuf>>,
    pub phase: Phase,
    pub focus: Option<usize>,
    pub drawn: Drawn,
    /// What an input method is composing in the name field (IME preedit).
    pub preedit: String,
    /// Develop's Export… (its own choices; `choices` then unused).
    pub export: Option<Box<super::develop::export::Export>>,
}

/// A batch being converted (or exported from Develop).
pub(super) struct Run {
    pub cancel: CancelToken,
    /// Progress, and the fraction done when finer than whole images.
    pub progress: Receiver<(Progress, Option<f32>)>,
    pub report: Receiver<Report>,
    pub last: Progress,
    pub fraction: Option<f32>,
    pub stopping: bool,
    /// The progress toast while the dialog is hidden, and when it was last updated.
    pub toast: Option<u64>,
    pub toast_at: f64,
    /// Develop's export: its progress shows in the downloads indicator, not a toast.
    pub export: bool,
}

impl Run {
    /// The fraction done.
    pub fn done(&self) -> f32 {
        self.fraction
            .unwrap_or(self.last.done as f32 / self.last.total.max(1) as f32)
    }
}

/// The app's convert state.
#[derive(Default)]
pub(super) struct ConvertState {
    pub dialog: Option<Dialog>,
    pub run: Option<Run>,
}

impl std::fmt::Debug for ConvertState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConvertState")
            .field("open", &self.dialog.is_some())
            .field("running", &self.run.is_some())
            .finish_non_exhaustive()
    }
}

impl ConvertState {
    /// The dialog as the draw function sees it.
    fn view(&self) -> Option<DialogView> {
        let d = self.dialog.as_ref()?;
        let export = d.export.is_some();
        let phase = match &d.phase {
            Phase::Setup => {
                let c = &d.choices;
                let (src, paths) = &d.sources[d.source];
                let label = source_label(*src, paths.len());
                let sel = d.name.selection();
                let probe = d
                    .probe
                    .as_ref()
                    .filter(|(p, _)| Some(p) == paths.first())
                    .and_then(|x| x.1.clone());
                let first = paths.first().map(|p| p.as_path());
                let (start, end, preview) = match &d.export {
                    Some(e) => (
                        e.row_views(&e.start_rows(), &label, d.name.text()),
                        e.row_views(&e.end_rows(), &label, d.name.text()),
                        e.preview(d.name.text(), first, probe),
                    ),
                    None => (
                        row_views(c, &c.start_rows(), &label),
                        row_views(c, &c.end_rows(), &label),
                        preview(c, d.name.text(), first, probe),
                    ),
                };
                PhaseView::Setup {
                    start,
                    end,
                    name: d.name.text().to_owned(),
                    caret: d.name.caret(),
                    selection: (sel.start, sel.end),
                    preview,
                    preedit: d.preedit.clone(),
                }
            }
            Phase::Running => {
                let (p, progress, stopping) = self
                    .run
                    .as_ref()
                    .map_or((Progress::default(), 0.0, false), |r| {
                        (r.last, r.done(), r.stopping)
                    });
                PhaseView::Running {
                    status: progress_text_for(&p, export),
                    progress,
                    stopping,
                }
            }
            Phase::Done(r) => {
                let (summary, lines) = report_lines_for(r, export);
                PhaseView::Done { summary, lines }
            }
        };
        let delete_preset = match d.phase {
            Phase::Setup => d
                .export
                .as_ref()
                .and_then(|e| e.delete_label(d.name.text())),
            _ => None,
        };
        Some(DialogView {
            phase,
            focus: d.focus,
            export,
            delete_preset,
        })
    }

    /// Draws the dialog (when open) over everything; returns what the accessibility tree
    /// needs.
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
        self.dialog.as_mut()?.drawn = drawn;
        Some(v)
    }

    /// The dialog's nodes in the accessibility tree.
    pub(super) fn a11y(&self, tree: &mut accesskit::TreeUpdate, v: &DialogView, scale: f64) {
        if let Some(d) = &self.dialog {
            a11y(tree, v, &d.drawn, scale);
        }
    }

    /// Where the input method's window opens while the name field has the keyboard: at the
    /// composition and the caret.
    pub(super) fn ime_area(&self) -> Option<Rect> {
        let d = self.dialog.as_ref().filter(|_| self.wants_text())?;
        d.drawn.ime.or(d.drawn.field)
    }

    /// What the input method composes in the name field (empty: nothing).
    pub(super) fn set_preedit(&mut self, s: &str) {
        let wants = self.wants_text();
        if let Some(d) = self.dialog.as_mut() {
            if wants {
                s.clone_into(&mut d.preedit);
            } else {
                d.preedit.clear();
            }
        }
    }

    /// True while the name field has the keyboard.
    pub(super) fn wants_text(&self) -> bool {
        self.view().is_some_and(|v| {
            v.focus.and_then(|i| v.focusables().get(i).copied()) == Some(Hit::Name)
        })
    }
}

impl App {
    /// The batch or its probe panicked: the run ends (the toast says why).
    pub(super) fn convert_lost(&mut self, task: &str) {
        match task {
            "conv-batch" => self.convert.run = None,
            "conv-probe" => {
                if let Some(d) = self.convert.dialog.as_mut() {
                    d.probing = None;
                }
            }
            _ => {}
        }
    }

    pub(super) fn convert_open(&mut self) {
        if self.convert.dialog.is_some() {
            return;
        }
        // A batch is running: show its progress.
        if self.convert.run.is_some() {
            if let Some(r) = self.convert.run.as_mut()
                && let Some(id) = r.toast.take()
            {
                self.chrome.dismiss_toast(id);
            }
            let export = self.convert.run.as_ref().is_some_and(|r| r.export);
            self.convert.dialog = Some(Dialog {
                sources: Vec::new(),
                source: 0,
                choices: Choices::new(self.settings.convert.clone()),
                name: LineEdit::new(&self.settings.convert.pattern),
                probe: None,
                probing: None,
                picking: None,
                phase: Phase::Running,
                focus: Some(0),
                drawn: Drawn::default(),
                preedit: String::new(),
                export: export.then(|| Box::new(super::develop::export::Export::idle())),
            });
            self.chrome.announce_status(title_text(export));
            self.redraw = true;
            return;
        }
        let mut sources = Vec::new();
        if let Some(p) = self.current.and_then(|i| self.entries.get(i)) {
            sources.push((Source::Current, vec![p.clone()]));
        }
        let mut first = 0;
        if self.gallery_active() {
            let sel: Vec<PathBuf> = self
                .gallery
                .targets()
                .into_iter()
                .filter(|p| !p.is_dir())
                .collect();
            if !sel.is_empty() {
                sources.push((Source::Selection, sel));
                first = sources.len() - 1;
            }
        }
        let files: Vec<PathBuf> = self.entries.clone();
        if files.len() > 1 {
            sources.push((Source::Folder, files));
        }
        let picks = self.marked_files();
        if !picks.is_empty() {
            sources.push((Source::Picks, picks));
        }
        if sources.is_empty() {
            return;
        }
        let s = self.settings.convert.clone();
        self.convert.dialog = Some(Dialog {
            sources,
            source: first,
            name: LineEdit::new(&s.pattern),
            choices: Choices::new(s),
            probe: None,
            probing: None,
            picking: None,
            phase: Phase::Setup,
            // On the format.
            focus: Some(1),
            drawn: Drawn::default(),
            preedit: String::new(),
            export: None,
        });
        self.convert_probe();
        self.chrome.announce_status(t!("convert-title"));
        self.redraw = true;
    }

    /// Reads the first image's size and date (for the name preview) unless known.
    pub(super) fn convert_probe(&mut self) {
        let Some(d) = self.convert.dialog.as_mut() else {
            return;
        };
        let Some(first) = d.sources.get(d.source).and_then(|s| s.1.first()).cloned() else {
            return;
        };
        if d.probe.as_ref().is_some_and(|p| p.0 == first)
            || d.probing.as_ref().is_some_and(|p| p.0 == first)
        {
            return;
        }
        let (tx, rx) = crossbeam_channel::bounded(1);
        let registry = self.loader.registry();
        let path = first.clone();
        self.loader.background("conv-probe", move |l| {
            let _ = tx.send(specular_convert::probe(&registry, &path));
            l.waker()();
        });
        d.probing = Some((first, rx));
    }

    /// Saves the dialog's choices as the `[convert]` settings (a pattern with an error keeps
    /// the last good one).
    fn convert_save_choices(&mut self) {
        let Some(d) = self.convert.dialog.as_ref() else {
            return;
        };
        if !matches!(d.phase, Phase::Setup) {
            return;
        }
        if d.export.is_some() {
            self.develop_export_save_choices();
            return;
        }
        let mut s = d.choices.s.clone();
        if specular_convert::Pattern::parse(d.name.text()).is_ok() {
            s.pattern = d.name.text().to_owned();
        } else {
            s.pattern = self.settings.convert.pattern.clone();
        }
        if self.settings.convert == s {
            return;
        }
        if self.args.screenshot.is_some() {
            let mut all = (*self.settings).clone();
            all.convert = s;
            self.settings = Arc::new(all);
        } else {
            self.store.update(Origin::Program, |x| x.convert = s);
            self.settings = self.store.snapshot();
        }
    }

    fn convert_close(&mut self) {
        self.convert_save_choices();
        self.convert.dialog = None;
        self.redraw = true;
    }

    fn convert_view(&self) -> Option<DialogView> {
        self.convert.view()
    }

    /// Starts converting the chosen images.
    fn convert_start(&mut self) {
        if self.convert.run.is_some() {
            let now = self.now();
            let msg = if self
                .convert
                .dialog
                .as_ref()
                .is_some_and(|d| d.export.is_some())
            {
                t!("develop-export-running")
            } else {
                t!("convert-running")
            };
            self.chrome.warning(msg, now);
            return;
        }
        let Some(v) = self.convert_view() else {
            return;
        };
        if !v.can_start() {
            return;
        }
        self.convert_save_choices();
        if v.export {
            self.develop_export_start();
            return;
        }
        let Some(d) = self.convert.dialog.as_mut() else {
            return;
        };
        let mut options = d.choices.options();
        options.pattern = d.name.text().to_owned();
        let paths = d.sources[d.source].1.clone();
        let Ok(batch) = Batch::new(paths, options) else {
            return;
        };
        let batch = Arc::new(batch);
        let threads = std::thread::available_parallelism()
            .map_or(2, |n| n.get())
            .div_ceil(2)
            .max(1);
        let cancel = CancelToken::new();
        let (ptx, prx) = crossbeam_channel::unbounded();
        let (rtx, rrx) = crossbeam_channel::bounded(1);
        let registry = self.loader.registry();
        let wake = self.loader.waker();
        let c = cancel.clone();
        let total = batch.inputs.len();
        specular_log::info!(
            "convert: {total} image(s) to {} on {threads} thread(s)",
            batch.options.format.id()
        );
        self.loader.background("conv-batch", move |_| {
            let pool = JobPool::background(threads, "conv-job");
            let w = wake.clone();
            let report = specular_convert::run(
                batch,
                registry,
                &pool,
                threads,
                &c,
                Arc::new(move |p: &Progress| {
                    let _ = ptx.send((*p, None));
                    w();
                }),
            );
            let _ = rtx.send(report);
            wake();
        });
        self.convert.run = Some(Run {
            cancel,
            progress: prx,
            report: rrx,
            last: Progress {
                total,
                ..Progress::default()
            },
            fraction: None,
            stopping: false,
            toast: None,
            toast_at: 0.0,
            export: false,
        });
        d.phase = Phase::Running;
        // On Hide, not Stop (Stop comes first in reading order).
        d.focus = Some(1);
        self.chrome
            .announce_status(t!("convert-progress", done = 0, total = total as i64));
        self.redraw = true;
    }

    pub(super) fn convert_stop(&mut self) {
        if let Some(r) = self.convert.run.as_mut()
            && !r.stopping
        {
            r.cancel.cancel();
            r.stopping = true;
            self.redraw = true;
        }
    }

    /// Hides the dialog of a running batch; its progress goes on in a toast.
    fn convert_hide(&mut self) {
        self.convert.dialog = None;
        let now = self.now();
        if let Some(r) = self.convert.run.as_mut() {
            if r.export {
                // Develop's export goes on in the downloads indicator.
                self.develop_export_refresh();
            } else {
                let msg = progress_text(&r.last);
                r.toast = Some(self.chrome.toast(msg, None, now));
                r.toast_at = now;
            }
        }
        self.redraw = true;
    }

    /// Replies of the probe, the folder dialog and the batch.
    pub(super) fn convert_tick(&mut self) {
        if let Some(d) = self.convert.dialog.as_mut() {
            if let Some((p, rx)) = &d.probing
                && let Ok(r) = rx.try_recv()
            {
                d.probe = Some((p.clone(), r));
                d.probing = None;
                self.redraw = true;
            }
            if let Some(rx) = &d.picking
                && let Ok(p) = rx.try_recv()
            {
                d.picking = None;
                let v = match d.export.as_mut() {
                    Some(e) => {
                        e.choose(p);
                        e.value(Row::Folder, "")
                    }
                    None => {
                        d.choices.choose(p);
                        row_value(&d.choices, Row::Folder, "")
                    }
                };
                self.chrome.announce_status(v);
                self.redraw = true;
            }
        }
        let now = self.now();
        let Some(r) = self.convert.run.as_mut() else {
            return;
        };
        let mut changed = false;
        while let Ok((p, f)) = r.progress.try_recv() {
            r.last = p;
            r.fraction = f;
            changed = true;
        }
        if let Ok(report) = r.report.try_recv() {
            let toast = r.toast;
            let export = r.export;
            self.convert.run = None;
            if export {
                self.develop_export_refresh();
            }
            self.convert_finished(report, toast, export);
            return;
        }
        if changed && r.export {
            self.redraw = true;
            self.develop_export_refresh();
        } else if changed {
            self.redraw = true;
            if let Some(id) = r.toast
                && now - r.toast_at >= 1.0
            {
                let msg = progress_text(&r.last);
                r.toast = Some(self.chrome.update_toast(id, msg, now));
                r.toast_at = now;
            }
        }
    }

    fn convert_finished(&mut self, report: Report, toast: Option<u64>, export: bool) {
        let now = self.now();
        specular_log::info!(
            "{}: {} written, {} skipped, {} failed{} in {:.1} s",
            if export { "develop export" } else { "convert" },
            report.converted(),
            report.skipped(),
            report.failed(),
            if report.cancelled { ", stopped" } else { "" },
            report.elapsed.as_secs_f64()
        );
        for (p, f) in report.failures() {
            specular_log::warn!("convert: {}: {f}", p.display());
        }
        if let Some(id) = toast {
            self.chrome.dismiss_toast(id);
        }
        let (summary, lines) = report_lines_for(&report, export);
        if let Some(d) = self.convert.dialog.as_mut() {
            d.phase = Phase::Done(report);
            d.focus = Some(0);
            self.chrome.announce_status(summary);
        } else {
            let mut msg = summary;
            for l in lines.iter().take(3) {
                msg = format!("{msg}. {l}");
            }
            if report.failed() > 0 {
                self.chrome.warning(msg, now);
            } else {
                self.chrome.toast(msg, None, now);
            }
        }
        self.redraw = true;
    }

    fn convert_choose_folder(&mut self) {
        let Some(d) = self.convert.dialog.as_mut() else {
            return;
        };
        if d.picking.is_some() {
            return;
        }
        let chosen = match &d.export {
            Some(e) => e.chosen.clone(),
            None => d.choices.chosen.clone(),
        };
        let start = chosen.or_else(|| {
            d.sources[d.source]
                .1
                .first()
                .and_then(|p| p.parent())
                .map(Path::to_path_buf)
        });
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("dlg-convert", move |l| {
            let mut dlg = rfd::FileDialog::new();
            if let Some(dir) = start {
                dlg = dlg.set_directory(dir);
            }
            if let Some(p) = dlg.pick_folder() {
                let _ = tx.send(p);
                l.waker()();
            }
        });
        d.picking = Some(rx);
    }

    fn convert_set_focus(&mut self, h: Hit) {
        if let Some(v) = self.convert_view()
            && let Some(i) = v.focusables().iter().position(|x| *x == h)
            && let Some(d) = self.convert.dialog.as_mut()
        {
            d.focus = Some(i);
        }
    }

    fn convert_step(&mut self, row: Row, delta: i32) {
        let Some(d) = self.convert.dialog.as_mut() else {
            return;
        };
        if !matches!(d.phase, Phase::Setup) {
            return;
        }
        if row == Row::Images {
            if d.sources.len() > 1 {
                d.source = (d.source as i32 + delta).rem_euclid(d.sources.len() as i32) as usize;
                let (src, paths) = &d.sources[d.source];
                let label = source_label(*src, paths.len());
                self.convert_probe();
                self.chrome.announce_status(label);
            }
            return;
        }
        let changed = match d.export.as_mut() {
            Some(e) => e.step(row, delta, &mut d.name),
            None => d.choices.step(row, delta),
        };
        if changed {
            // Focus stays on the row (rows come and go with the format and the size).
            self.convert_set_focus(Hit::Row(row));
            if let Some(d) = self.convert.dialog.as_ref() {
                let v = match &d.export {
                    Some(e) => e.value(row, ""),
                    None => row_value(&d.choices, row, ""),
                };
                self.chrome.announce_status(v);
            }
        }
    }

    fn convert_hit(&mut self, h: Hit) {
        if h != Hit::DeletePreset
            && let Some(e) = self.convert.dialog.as_mut().and_then(|d| d.export.as_mut())
        {
            e.disarm();
        }
        match h {
            Hit::Dec(r) => self.convert_step(r, -1),
            Hit::Inc(r) => self.convert_step(r, 1),
            Hit::Row(_) | Hit::Name => self.convert_set_focus(h),
            Hit::Choose => {
                self.convert_set_focus(h);
                self.convert_choose_folder();
            }
            Hit::SavePreset => {
                self.convert_set_focus(h);
                self.develop_export_save_preset();
            }
            Hit::DeletePreset => {
                self.convert_set_focus(h);
                self.develop_export_delete_preset();
                // Deleted: the button goes; focus stays on Save preset.
                if self
                    .convert_view()
                    .is_some_and(|v| v.delete_preset.is_none())
                {
                    self.convert_set_focus(Hit::SavePreset);
                }
            }
            Hit::Cancel | Hit::Close => self.convert_close(),
            Hit::Start => self.convert_start(),
            Hit::Stop => self.convert_stop(),
            Hit::Hide => self.convert_hide(),
        }
        self.redraw = true;
    }

    /// Text into the name field (typed keys and input-method commits).
    pub(super) fn convert_text(&mut self, s: &str) {
        if let Some(d) = self.convert.dialog.as_mut()
            && d.name.insert(s)
        {
            self.redraw = true;
        }
    }

    /// Text pasted into the name field (the clipboard, read in the background).
    pub(super) fn convert_paste(&mut self, s: &str) {
        if let Some(d) = self.convert.dialog.as_mut()
            && matches!(d.phase, Phase::Setup)
        {
            d.name.paste(s);
            self.redraw = true;
        }
    }

    /// The pointer moved while the dialog is open: a press in the name field drags a selection.
    pub(super) fn convert_drag(&mut self, p: (f32, f32)) {
        if let Some(d) = self.convert.dialog.as_mut()
            && d.name.dragging()
            && let Some((block, x0)) = &d.drawn.field_text
        {
            d.name.drag_to(block.hit_byte(p.0 - x0));
            self.redraw = true;
        }
    }

    /// Editing keys in the name field; false when the key is not one.
    fn convert_field_key(&mut self, combo: Option<&KeyCombo>, text: Option<&str>) -> bool {
        let now = self.now();
        let Some(d) = self.convert.dialog.as_mut() else {
            return false;
        };
        if let Some(c) = combo {
            // The pattern reads in the direction of its first strong character.
            let rtl = d.drawn.field_text.as_ref().is_some_and(|(b, _)| b.rtl);
            let mut clip = PendingClipboard::default();
            let handled = d.name.key(c.key, c.mods, rtl, &mut clip).used();
            if handled {
                // Offered once the keys are let go or rest (not on every step of a held
                // Shift+Arrow): see `convert_key_up` and `convert_primary_tick`.
                if specular_widgets::line_edit::PRIMARY_SELECTION {
                    d.name.offer_primary_later(now);
                }
                self.field_clipboard(clip, super::integration::PasteTo::ConvertName);
            }
            if handled {
                self.redraw = true;
                return true;
            }
            if c.mods.ctrl || c.mods.alt || c.mods.meta {
                return false;
            }
        }
        let Some(t) = text else {
            return false;
        };
        let s: String = t.chars().filter(|c| !c.is_control()).collect();
        if s.is_empty() {
            return false;
        }
        self.last_key_text = Some((s.clone(), Instant::now()));
        self.convert_text(&s);
        true
    }

    /// Keys while the dialog is open (it takes them all); `text` is what the key types.
    pub(super) fn convert_key(&mut self, combo: Option<&KeyCombo>, text: Option<&str>) -> bool {
        let Some(v) = self.convert_view() else {
            return false;
        };
        let focusables = v.focusables();
        let n = focusables.len();
        let focus = v.focus.and_then(|i| focusables.get(i).copied());
        if focus == Some(Hit::Name) && self.convert_field_key(combo, text) {
            return true;
        }
        let Some(combo) = combo else { return true };
        let rtl = self.rtl();
        let move_focus = |me: &mut App, d: i32| {
            if let Some(dl) = me.convert.dialog.as_mut() {
                let i = dl
                    .focus
                    .map_or(0, |i| (i as i32 + d).rem_euclid(n as i32) as usize);
                dl.focus = Some(i);
            }
        };
        let plain = combo.mods == Mods::NONE;
        let setup = matches!(v.phase, PhaseView::Setup { .. });
        let default = match v.phase {
            PhaseView::Setup { .. } => Hit::Start,
            PhaseView::Running { .. } => Hit::Hide,
            PhaseView::Done { .. } => Hit::Close,
        };
        match combo.key {
            Key::Named(NamedKey::Escape) => self.convert_hit(match v.phase {
                PhaseView::Running { .. } => Hit::Hide,
                _ => Hit::Cancel,
            }),
            Key::Named(NamedKey::Tab) if combo.mods.shift => move_focus(self, -1),
            Key::Named(NamedKey::Tab) => move_focus(self, 1),
            Key::Named(NamedKey::ArrowUp) if plain => move_focus(self, -1),
            Key::Named(NamedKey::ArrowDown) if plain => move_focus(self, 1),
            Key::Named(k @ (NamedKey::ArrowLeft | NamedKey::ArrowRight)) if plain => {
                let forward = (k == NamedKey::ArrowRight) != rtl;
                match focus {
                    Some(Hit::Row(r)) => self.convert_step(r, if forward { 1 } else { -1 }),
                    _ => move_focus(self, if forward { 1 } else { -1 }),
                }
            }
            Key::Named(NamedKey::Enter) => match focus {
                Some(Hit::Row(_) | Hit::Name) | None => self.convert_hit(default),
                Some(h) => self.convert_hit(h),
            },
            Key::Named(NamedKey::Space) => match focus {
                Some(Hit::Row(r)) => self.convert_step(r, 1),
                Some(h) if h != Hit::Name => self.convert_hit(h),
                _ => {}
            },
            Key::Char('s' | 'S') if combo.mods.ctrl && setup => self.convert_hit(Hit::Start),
            _ => {}
        }
        self.redraw = true;
        true
    }

    /// A click while the dialog is open (it takes them all; outside the sheet does nothing).
    pub(super) fn convert_click(&mut self, p: (f32, f32), down: bool) -> bool {
        let (now, shift) = (self.now(), self.mods.shift_key());
        let Some(d) = self.convert.dialog.as_mut() else {
            return false;
        };
        if !down {
            let was = d.name.dragging();
            d.name.release();
            if was {
                self.convert_primary();
            }
            return true;
        }
        let hit = d
            .drawn
            .hits
            .iter()
            .find(|(r, h)| r.contains(p) && !matches!(h, Hit::Row(_)))
            .or_else(|| d.drawn.hits.iter().find(|(r, _)| r.contains(p)))
            .map(|x| x.1);
        if hit == Some(Hit::Name)
            && let Some((block, x0)) = &d.drawn.field_text
        {
            let at = block.hit_byte(p.0 - x0);
            d.name.press(at, p.0, now, shift);
        }
        if let Some(h) = hit {
            self.convert_hit(h);
        }
        true
    }

    /// Copy-on-select in the name field: a new selection becomes the primary selection (X11,
    /// Wayland; nothing elsewhere).
    fn convert_primary(&mut self) {
        if !specular_widgets::line_edit::PRIMARY_SELECTION {
            return;
        }
        let mut clip = PendingClipboard::default();
        if let Some(d) = self.convert.dialog.as_mut() {
            d.name.offer_primary(&mut clip);
        }
        self.field_clipboard(clip, super::integration::PasteTo::ConvertName);
    }

    /// A key let go while the dialog is open: a selection the keys made in the name field is
    /// offered to the primary selection now.
    pub(super) fn convert_key_up(&mut self) {
        if self
            .convert
            .dialog
            .as_ref()
            .is_some_and(|d| d.name.primary_pending())
        {
            self.convert_primary();
        }
    }

    /// Offers a selection the keys made in the name field once they have rested; the time
    /// to look again while one waits.
    pub(super) fn convert_primary_tick(&mut self, now: f64) -> Option<f64> {
        if !specular_widgets::line_edit::PRIMARY_SELECTION {
            return None;
        }
        let d = self.convert.dialog.as_mut()?;
        if !d.name.primary_pending() {
            return None;
        }
        let mut clip = PendingClipboard::default();
        d.name.offer_primary_due(now, &mut clip);
        let wake = d.name.primary_wake(now);
        self.field_clipboard(clip, super::integration::PasteTo::ConvertName);
        wake
    }

    /// A middle click at `p` while the dialog is open (X11, Wayland): in the name field the
    /// caret goes there and the primary selection is pasted at it. False when not in the
    /// field.
    pub(super) fn convert_middle(&mut self, p: (f32, f32)) -> bool {
        if !specular_widgets::line_edit::PRIMARY_SELECTION {
            return false;
        }
        let Some(d) = self.convert.dialog.as_mut() else {
            return false;
        };
        let in_name = d
            .drawn
            .hits
            .iter()
            .any(|(r, h)| *h == Hit::Name && r.contains(p));
        if !in_name || !matches!(d.phase, Phase::Setup) {
            return false;
        }
        let at = d
            .drawn
            .field_text
            .as_ref()
            .map_or(0, |(block, x0)| block.hit_byte(p.0 - x0));
        let mut clip = PendingClipboard::default();
        d.name.paste_primary_at(at, &mut clip);
        self.convert_set_focus(Hit::Name);
        self.field_clipboard(clip, super::integration::PasteTo::ConvertName);
        self.redraw = true;
        true
    }

    /// A screen reader's request on one of the dialog's nodes; false when not ours.
    pub(super) fn convert_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        let Some(v) = self.convert_view() else {
            return false;
        };
        let Some(hit) = a11y_target(&v, req.target_node.0) else {
            return (A11Y_BASE..A11Y_BASE + 0x1000).contains(&req.target_node.0);
        };
        match (req.action, hit) {
            (accesskit::Action::Increment, Hit::Row(r)) => self.convert_step(r, 1),
            (accesskit::Action::Decrement, Hit::Row(r)) => self.convert_step(r, -1),
            (accesskit::Action::SetValue, Hit::Name) => {
                if let Some(accesskit::ActionData::Value(s)) = &req.data
                    && let Some(d) = self.convert.dialog.as_mut()
                {
                    d.name.set_text(s);
                }
            }
            (accesskit::Action::SetTextSelection, Hit::Name) => {
                if let Some(accesskit::ActionData::SetTextSelection(sel)) = &req.data
                    && let Some(d) = self.convert.dialog.as_mut()
                {
                    d.name
                        .set_a11y_selection(sel.anchor.character_index, sel.focus.character_index);
                }
                self.convert_set_focus(Hit::Name);
                self.convert_primary();
            }
            (accesskit::Action::Click, h) => self.convert_hit(h),
            (accesskit::Action::Focus, h) => self.convert_set_focus(h),
            _ => {}
        }
        self.redraw = true;
        true
    }
}
