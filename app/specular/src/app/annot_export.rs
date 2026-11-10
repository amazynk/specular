//! "Export annotations…": a slide's annotations (its sidecar's, the imported ones, and the
//! regions and points drawn with the measuring tools) as COCO JSON, QuPath GeoJSON or label
//! masks ([`specular_pathology`]), in level-0 pixel coordinates.
//!
//! A modal sheet like the DICOM export's: the format; for masks, PNG or TIFF tiles or an
//! OME-TIFF pyramid, the region (the whole slide, the visible area or the last drawn
//! region), the level, the tile size and whether empty tiles are written; then Cancel and
//! Export…, which asks for the file (or the folder, for tiles) and writes on a thread.
//! The choices last for the session.

#[cfg(test)]
mod shots;

use std::path::{Path, PathBuf};

use crossbeam_channel::{Receiver, bounded};
use specular_gfx::{Align, DrawList, Icon, Rect, Text, TextStyle, draw_icon};
use specular_i18n::t;
use specular_input::{Key, KeyCombo, NamedKey};
use specular_pathology::annot::{Annotation, Geometry, classes};
use specular_pathology::mask::{self, MaskJob, Region, TileFormat};
use specular_pathology::{coco, qupath};
use specular_theme::metrics::{FONT_LABEL, FONT_SMALL, FONT_TITLE, RADIUS_LG, RADIUS_SM};
use specular_theme::{Color, Palette};
use specular_widgets::accesskit;

use super::App;

const A11Y_BASE: u64 = 0x5E00_0000_0000_0000;
/// Tile edges offered (output pixels).
pub(super) const TILES: [u32; 4] = [256, 512, 1024, 2048];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Format {
    Coco,
    GeoJson,
    Masks,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Output {
    Png,
    Tiff,
    Ome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Area {
    Slide,
    View,
    Drawn,
}

/// One line of the dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Row {
    Format,
    Output,
    Region,
    Level,
    Tile,
    Empty,
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

/// The dialog's choices, kept for the session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Choices {
    pub format: Format,
    pub output: Output,
    pub area: Area,
    /// Index into the slide's levels.
    pub level: usize,
    /// Index into [`TILES`].
    pub tile: usize,
    pub skip_empty: bool,
}

impl Default for Choices {
    fn default() -> Self {
        Choices {
            format: Format::GeoJson,
            output: Output::Ome,
            area: Area::Slide,
            level: 0,
            tile: 1,
            skip_empty: true,
        }
    }
}

fn cycle<T: Copy + PartialEq>(all: &[T], v: T, d: i32) -> T {
    let i = all.iter().position(|x| *x == v).unwrap_or(0) as i32;
    all[(i + d).rem_euclid(all.len() as i32) as usize]
}

impl Choices {
    /// The rows shown: masks have their own.
    pub(super) fn rows(&self) -> Vec<Row> {
        let mut v = vec![Row::Format];
        if self.format == Format::Masks {
            v.extend([Row::Output, Row::Region, Row::Level, Row::Tile]);
            if self.output != Output::Ome {
                v.push(Row::Empty);
            }
        }
        v
    }

    /// Steps `row` by `d` (±1) with `levels` levels and `drawn` a region to export; false
    /// when nothing changed.
    pub(super) fn step(&mut self, row: Row, d: i32, levels: usize, drawn: bool) -> bool {
        let before = self.clone();
        match row {
            Row::Format => {
                self.format = cycle(
                    &[Format::Coco, Format::GeoJson, Format::Masks],
                    self.format,
                    d,
                )
            }
            Row::Output => {
                self.output = cycle(&[Output::Png, Output::Tiff, Output::Ome], self.output, d)
            }
            Row::Region => {
                let all: &[Area] = if drawn {
                    &[Area::Slide, Area::View, Area::Drawn]
                } else {
                    &[Area::Slide, Area::View]
                };
                self.area = cycle(all, self.area, d);
            }
            Row::Level => {
                let n = levels.max(1) as i32;
                self.level = (self.level as i32 + d).rem_euclid(n) as usize;
            }
            Row::Tile => self.tile = (self.tile as i32 + d).rem_euclid(TILES.len() as i32) as usize,
            Row::Empty => self.skip_empty = !self.skip_empty,
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
}

/// Everything the dialog draws (so screenshots need no app).
#[derive(Clone, Debug)]
pub(super) struct DialogView {
    pub title: String,
    pub rows: Vec<RowView>,
    /// What will be exported ("12 shapes, 3 classes").
    pub summary: String,
    /// A note under the summary (lines left out).
    pub note: Option<String>,
    pub status: Option<String>,
    pub busy: bool,
    pub can_export: bool,
    /// Index into [`DialogView::focusables`].
    pub focus: Option<usize>,
}

impl DialogView {
    /// The keyboard focus order: the rows, then Cancel and Export.
    pub fn focusables(&self) -> Vec<Hit> {
        self.rows
            .iter()
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
    pub summary: Option<Rect>,
}

fn row_label(row: Row) -> String {
    match row {
        Row::Format => t!("annot-export-format"),
        Row::Output => t!("annot-export-output"),
        Row::Region => t!("annot-export-region"),
        Row::Level => t!("annot-export-level"),
        Row::Tile => t!("annot-export-tile"),
        Row::Empty => t!("annot-export-empty"),
    }
}

/// The rows as the dialog shows them, `levels` the slide's level sizes (full size first).
pub(super) fn row_views(c: &Choices, levels: &[(u32, u32)]) -> Vec<RowView> {
    c.rows()
        .into_iter()
        .map(|row| {
            let value = match row {
                Row::Format => match c.format {
                    Format::Coco => t!("annot-export-format-coco"),
                    Format::GeoJson => t!("annot-export-format-geojson"),
                    Format::Masks => t!("annot-export-format-masks"),
                },
                Row::Output => match c.output {
                    Output::Png => t!("annot-export-output-png"),
                    Output::Tiff => t!("annot-export-output-tiff"),
                    Output::Ome => t!("annot-export-output-ome"),
                },
                Row::Region => match c.area {
                    Area::Slide => t!("annot-export-region-slide"),
                    Area::View => t!("annot-export-region-view"),
                    Area::Drawn => t!("annot-export-region-drawn"),
                },
                Row::Level => {
                    let (w, h) = levels.get(c.level).copied().unwrap_or((0, 0));
                    t!(
                        "annot-export-level-value",
                        level = c.level as i64,
                        width = i64::from(w),
                        height = i64::from(h)
                    )
                }
                Row::Tile => format!("{0} × {0}", TILES[c.tile % TILES.len()]),
                Row::Empty => {
                    if c.skip_empty {
                        t!("annot-export-empty-skip")
                    } else {
                        t!("annot-export-empty-keep")
                    }
                }
            };
            RowView {
                row,
                label: row_label(row),
                value,
            }
        })
        .collect()
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
    let pad = 20.0;
    let title_st = TextStyle::new(FONT_TITLE * ts).weight(650.0);
    let label_st = TextStyle::new(FONT_LABEL * ts);
    let value_st = TextStyle::new(FONT_LABEL * ts).weight(550.0);
    let small_st = TextStyle::new(FONT_SMALL * ts);
    let button_st = TextStyle::new(FONT_LABEL * ts).weight(550.0);

    let w = (ww - 32.0).clamp(280.0, 520.0);
    let inner = w - 2.0 * pad;
    let title = text.layout(&v.title, title_st, Some(inner), Align::Start);
    let summary = text.layout(&v.summary, label_st, Some(inner - 28.0), Align::Start);
    let note = v
        .note
        .as_ref()
        .map(|s| text.layout(s, small_st, Some(inner - 28.0), Align::Start));
    let bh = 32.0;
    let status_h = if v.status.is_some() { 26.0 } else { 0.0 };
    let sum_h = summary.height + note.as_ref().map_or(0.0, |b| b.height + 4.0) + 12.0;
    let fixed = pad + title.height + 14.0 + sum_h + 14.0 + status_h + 14.0 + bh + pad;
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
    let focus = v.focus.map(|i| v.focusables()[i]);
    let mut drawn = Drawn {
        sheet,
        ..Drawn::default()
    };
    // What goes out: the shapes and classes, with the note.
    let top = sheet.y + pad + title.height + 14.0;
    let box_ = Rect::new(sheet.x + pad, top, inner, sum_h);
    dl.rect(m(box_), RADIUS_SM, pal.accent.with_alpha(0.10));
    let ib = m(Rect::new(box_.x + 6.0, box_.y + 6.0, 16.0, 16.0));
    draw_icon(dl, Icon::Info, ib, pal.accent, 1.6, rtl);
    text.draw(
        dl,
        &summary,
        start_x(box_.x + 28.0, summary.width),
        box_.y + 6.0,
        pal.text,
    );
    if let Some(b) = &note {
        text.draw(
            dl,
            b,
            start_x(box_.x + 28.0, b.width),
            box_.y + 6.0 + summary.height + 4.0,
            pal.text_muted,
        );
    }
    drawn.summary = Some(m(box_));
    let rows_top = box_.bottom() + 14.0;
    let arrow = (row_h - 6.0).min(26.0);
    let ink = if v.busy { pal.text_faint } else { pal.text };
    for (i, rv) in v.rows.iter().enumerate() {
        let r = Rect::new(sheet.x + pad, rows_top + i as f32 * row_h, inner, row_h);
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
        let sw = (inner * 0.56).min(260.0);
        let sx = r.right() - sw;
        let val = text.layout(&rv.value, value_st, None, Align::Start);
        // ‹ value ›
        let dec = Rect::new(sx, r.y + (row_h - arrow) * 0.5, arrow, arrow);
        let inc = Rect::new(r.right() - arrow, dec.y, arrow, arrow);
        for (b, icon, hit) in [
            (dec, Icon::ChevronLeft, Hit::Dec(rv.row)),
            (inc, Icon::ChevronRight, Hit::Inc(rv.row)),
        ] {
            let b = m(b);
            dl.rect(b, RADIUS_SM, pal.surface_raised);
            draw_icon(dl, icon, b.inset(arrow * 0.18), ink, 1.6, rtl);
            drawn.hits.push((b, hit));
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
        drawn.hits.push((m(r), Hit::Row(rv.row)));
        drawn.rows.push((m(r), rv.row));
    }
    let by = sheet.bottom() - pad - bh;
    if let Some(s) = &v.status {
        let b = text.layout(s, small_st, Some(inner), Align::Start);
        text.draw(
            dl,
            &b,
            start_x(sheet.x + pad, b.width),
            by - 14.0 - b.height,
            pal.text_muted,
        );
    }
    let labels = [
        (Hit::Export, t!("annot-export-export")),
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

fn bounds(r: Rect, scale: f64) -> accesskit::Rect {
    accesskit::Rect {
        x0: f64::from(r.x) * scale,
        y0: f64::from(r.y) * scale,
        x1: f64::from(r.x + r.w) * scale,
        y1: f64::from(r.y + r.h) * scale,
    }
}

/// Cancel and Export as drawn, in reading order.
fn buttons(d: &Drawn) -> Vec<(Rect, Hit)> {
    [Hit::Cancel, Hit::Export]
        .iter()
        .filter_map(|o| d.hits.iter().find(|(_, h)| h == o).copied())
        .collect()
}

/// The dialog's AccessKit nodes: a modal dialog with a spin button per option, the summary,
/// the status and Cancel / Export.
pub(super) fn a11y(tree: &mut accesskit::TreeUpdate, v: &DialogView, d: &Drawn, scale: f64) {
    use accesskit::{Node, NodeId, Role};
    let mut dialog = Node::new(Role::Dialog);
    dialog.set_modal();
    dialog.set_label(v.title.clone());
    dialog.set_bounds(bounds(d.sheet, scale));
    let mut ids = Vec::new();
    let mut nodes = Vec::new();
    let focusables = v.focusables();
    let focus_hit = v.focus.map(|i| focusables[i]);
    let mut focus_id = None;
    if let Some(r) = d.summary {
        let mut n = Node::new(Role::Label);
        let mut s = v.summary.clone();
        if let Some(note) = &v.note {
            s = format!("{s}. {note}");
        }
        n.set_value(s);
        n.set_bounds(bounds(r, scale));
        let id = NodeId(A11Y_BASE + 0x100);
        ids.push(id);
        nodes.push((id, n));
    }
    for (i, rv) in v.rows.iter().enumerate() {
        let id = NodeId(A11Y_BASE + 1 + i as u64);
        let mut n = Node::new(Role::SpinButton);
        if !v.busy {
            n.add_action(accesskit::Action::Increment);
            n.add_action(accesskit::Action::Decrement);
        }
        n.set_label(rv.label.clone());
        n.set_value(rv.value.clone());
        if let Some((r, _)) = d.rows.iter().find(|(_, row)| *row == rv.row) {
            n.set_bounds(bounds(*r, scale));
        }
        n.add_action(accesskit::Action::Focus);
        if focus_hit == Some(Hit::Row(rv.row)) {
            focus_id = Some(id);
        }
        ids.push(id);
        nodes.push((id, n));
    }
    for (k, (r, hit)) in buttons(d).into_iter().enumerate() {
        let id = NodeId(A11Y_BASE + 0x101 + k as u64);
        let mut n = Node::new(Role::Button);
        n.set_label(match hit {
            Hit::Cancel => t!("app-cancel"),
            _ => t!("annot-export-export"),
        });
        n.set_bounds(bounds(r, scale));
        if hit == Hit::Cancel || (v.can_export && !v.busy) {
            n.add_action(accesskit::Action::Click);
        } else {
            n.set_disabled();
        }
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
        return Some(Hit::Row(v.rows[k as usize - 1].row));
    }
    let k = k.checked_sub(0x101)? as usize;
    buttons(d).get(k).map(|b| b.1)
}

/// The summary line for `items`.
pub(super) fn summary(items: &[Annotation]) -> (String, Option<String>) {
    let n = items.len() as i64;
    let k = classes(items).len() as i64;
    let lines = items
        .iter()
        .any(|a| matches!(a.geometry, Geometry::Line(_)));
    (
        t!("annot-export-what", count = n, classes = k),
        lines.then(|| t!("annot-export-lines")),
    )
}

/// Now as an RFC 3339 UTC time (COCO's `date_created`).
fn rfc3339_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil date from days since 1970 (Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

/// One export, ready for its thread.
#[derive(Clone, Debug)]
pub(super) struct Job {
    pub items: Vec<Annotation>,
    pub choices: Choices,
    /// Level-0 size, the region to label and the chosen level's downsample.
    pub size: (u32, u32),
    pub region: Region,
    pub downsample: f64,
    pub file: String,
    pub stem: String,
    pub pixel_um: Option<f64>,
}

/// Writes `job` to `out` (a file, or a folder for tiles): the files written.
pub(super) fn write(job: &Job, out: &Path) -> Result<usize, String> {
    let io = |e: std::io::Error| crate::error_text::io_reason(&e);
    match job.choices.format {
        Format::Coco => {
            let image = coco::Image {
                file_name: job.file.clone(),
                width: job.size.0,
                height: job.size.1,
            };
            let created = rfc3339_now();
            let (text, _) = coco::write(
                &job.items,
                &image,
                &format!("Specular {}", env!("CARGO_PKG_VERSION")),
                &created,
            );
            std::fs::write(out, text).map_err(io)?;
            Ok(1)
        }
        Format::GeoJson => {
            std::fs::write(out, qupath::write(&job.items).0).map_err(io)?;
            Ok(1)
        }
        Format::Masks => {
            let mj = MaskJob {
                region: job.region,
                downsample: job.downsample,
                tile: TILES[job.choices.tile % TILES.len()],
                skip_empty: job.choices.skip_empty,
            };
            match job.choices.output {
                Output::Ome => {
                    let f = std::fs::File::create(out).map_err(io)?;
                    let mut w = std::io::BufWriter::new(f);
                    mask::write_ome_tiff(&job.items, &mj, &job.stem, job.pixel_um, &mut w)
                        .map_err(|e| e.to_string())?;
                    use std::io::Write as _;
                    w.flush().map_err(io)?;
                    Ok(1)
                }
                o => {
                    let fmt = if o == Output::Png {
                        TileFormat::Png
                    } else {
                        TileFormat::Tiff
                    };
                    let r = mask::write_tiles(&job.items, &mj, fmt, out, &job.stem)
                        .map_err(|e| e.to_string())?;
                    Ok(r.files.len())
                }
            }
        }
    }
}

/// The open dialog.
#[derive(Debug)]
pub(super) struct Dialog {
    pub levels: Vec<(u32, u32)>,
    /// What goes out, counted when the dialog opened: summary, note, anything at all.
    pub what: (String, Option<String>, bool),
    pub focus: Option<usize>,
    pub busy: bool,
    pub status: Option<String>,
    pub rx: Option<Receiver<Result<usize, String>>>,
}

/// The export's session state.
#[derive(Debug, Default)]
pub(super) struct AnnotExport {
    pub dialog: Option<Dialog>,
    pub choices: Choices,
    pub drawn: Option<Drawn>,
}

impl App {
    /// The slide's levels: the science summary's, else the image's own size.
    fn annot_levels(&self) -> Vec<(u32, u32)> {
        if let Some(s) = self.science_summary_current()
            && !s.levels.is_empty()
        {
            return s.levels.clone();
        }
        vec![self.view.stored_size()]
    }

    pub(super) fn annot_export_open(&mut self) {
        let levels = self.annot_levels();
        let drawn = self.stain_region().is_some();
        let c = &mut self.annot_export.choices;
        c.level = c.level.min(levels.len().saturating_sub(1));
        if c.area == Area::Drawn && !drawn {
            c.area = Area::Slide;
        }
        let items = self.pathology_annotations();
        let (s, note) = summary(&items);
        self.annot_export.dialog = Some(Dialog {
            levels,
            what: (s, note, !items.is_empty()),
            focus: Some(0),
            busy: false,
            status: None,
            rx: None,
        });
        self.redraw = true;
    }

    fn annot_export_close(&mut self) {
        self.annot_export.dialog = None;
        self.annot_export.drawn = None;
        self.redraw = true;
    }

    pub(super) fn annot_export_view(&self) -> Option<DialogView> {
        let d = self.annot_export.dialog.as_ref()?;
        let (summary, note, any) = d.what.clone();
        Some(DialogView {
            title: t!("annot-export-title"),
            rows: row_views(&self.annot_export.choices, &d.levels),
            summary,
            note,
            status: d.status.clone(),
            busy: d.busy,
            can_export: any,
            focus: d.focus,
        })
    }

    /// The export of what the dialog says.
    fn annot_job(&self) -> Option<Job> {
        let d = self.annot_export.dialog.as_ref()?;
        let c = self.annot_export.choices.clone();
        let items = self.pathology_annotations();
        let size = d
            .levels
            .first()
            .copied()
            .unwrap_or_else(|| self.view.stored_size());
        let region = match c.area {
            Area::Slide => Region {
                x: 0.0,
                y: 0.0,
                w: f64::from(size.0),
                h: f64::from(size.1),
            },
            Area::View => {
                let (x0, y0, x1, y1) = self.view.visible_stored(true);
                Region {
                    x: f64::from(x0),
                    y: f64::from(y0),
                    w: f64::from(x1 - x0).max(1.0),
                    h: f64::from(y1 - y0).max(1.0),
                }
            }
            Area::Drawn => {
                let (x0, y0, x1, y1) = self.stain_region()?.bounds();
                Region {
                    x: x0,
                    y: y0,
                    w: (x1 - x0).max(1.0),
                    h: (y1 - y0).max(1.0),
                }
            }
        };
        let lv = d.levels.get(c.level).copied().unwrap_or(size);
        let downsample = (f64::from(size.0) / f64::from(lv.0.max(1))).max(1.0);
        let path = self.current.and_then(|i| self.entries.get(i));
        let file = path
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let stem = path
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "slide".to_owned());
        let pixel_um = self
            .science_summary_current()
            .and_then(|s| s.pixel.as_ref())
            .and_then(|p| match p.unit_x.as_str() {
                "µm" | "um" => Some(p.x.step.abs()),
                "m" => Some(p.x.step.abs() * 1e6),
                "mm" => Some(p.x.step.abs() * 1000.0),
                "nm" => Some(p.x.step.abs() / 1000.0),
                _ => None,
            });
        Some(Job {
            items,
            choices: c,
            size,
            region,
            downsample,
            file,
            stem,
            pixel_um,
        })
    }

    fn annot_export_run(&mut self) {
        let Some(job) = self.annot_job() else { return };
        if job.items.is_empty() {
            return;
        }
        let dir = self
            .current
            .and_then(|i| self.entries.get(i))
            .and_then(|p| p.parent())
            .map(Path::to_path_buf);
        let (tx, rx) = bounded(1);
        if let Some(d) = self.annot_export.dialog.as_mut() {
            d.busy = true;
            d.status = Some(t!("annot-export-writing"));
            d.rx = Some(rx);
        }
        self.loader.background("annot-export", move |l| {
            let (name, ext, folder) = match (job.choices.format, job.choices.output) {
                (Format::Coco, _) => (format!("{}-coco.json", job.stem), "json", false),
                (Format::GeoJson, _) => (format!("{}.geojson", job.stem), "geojson", false),
                (Format::Masks, Output::Ome) => {
                    (format!("{}-labels.ome.tif", job.stem), "tif", false)
                }
                (Format::Masks, _) => (String::new(), "", true),
            };
            let picked: Option<PathBuf> = if folder {
                let mut dlg = rfd::FileDialog::new();
                if let Some(d) = &dir {
                    dlg = dlg.set_directory(d);
                }
                dlg.pick_folder()
            } else {
                let mut dlg = rfd::FileDialog::new()
                    .add_filter(ext.to_uppercase(), &[ext])
                    .set_file_name(name);
                if let Some(d) = &dir {
                    dlg = dlg.set_directory(d);
                }
                dlg.save_file()
            };
            let r = match picked {
                Some(out) => write(&job, &out),
                None => Ok(0),
            };
            let _ = tx.send(r);
            l.waker()();
        });
        self.redraw = true;
    }

    pub(super) fn annot_export_tick(&mut self) {
        let Some(d) = self.annot_export.dialog.as_mut() else {
            return;
        };
        let Some(rx) = &d.rx else { return };
        let Ok(r) = rx.try_recv() else { return };
        d.rx = None;
        d.busy = false;
        match r {
            Ok(0) => d.status = None,
            Ok(n) => {
                self.annot_export_close();
                let now = self.now();
                self.chrome
                    .toast(t!("annot-export-done", files = n as i64), None, now);
            }
            Err(e) => d.status = Some(t!("annot-export-failed", reason = e)),
        }
        self.redraw = true;
    }

    fn annot_step(&mut self, row: Row, d: i32) {
        let drawn = self.stain_region().is_some();
        let Some(dlg) = self.annot_export.dialog.as_ref() else {
            return;
        };
        if dlg.busy {
            return;
        }
        let n = dlg.levels.len();
        if self.annot_export.choices.step(row, d, n, drawn) {
            // A row list that shrank keeps the focus inside it.
            let len = self.annot_export.choices.rows().len() + 2;
            if let Some(dlg) = self.annot_export.dialog.as_mut() {
                dlg.focus = dlg.focus.map(|f| f.min(len - 1));
            }
            self.redraw = true;
        }
    }

    fn annot_hit(&mut self, h: Hit) {
        match h {
            Hit::Dec(r) => self.annot_step(r, -1),
            Hit::Inc(r) | Hit::Row(r) => self.annot_step(r, 1),
            Hit::Cancel => self.annot_export_close(),
            Hit::Export => self.annot_export_run(),
        }
    }

    pub(super) fn annot_export_key(&mut self, combo: Option<&KeyCombo>) -> bool {
        let Some(c) = combo else { return true };
        let Some(v) = self.annot_export_view() else {
            return false;
        };
        let f = v.focusables();
        let focus = self.annot_export.dialog.as_ref().and_then(|d| d.focus);
        let set_focus = |a: &mut App, i: usize| {
            if let Some(d) = a.annot_export.dialog.as_mut() {
                d.focus = Some(i);
            }
            a.redraw = true;
        };
        let rtl = self.rtl();
        match (&c.key, c.mods) {
            (Key::Named(NamedKey::Escape), _) => self.annot_export_close(),
            (Key::Named(NamedKey::Tab), m) => {
                let n = f.len();
                let i = focus.unwrap_or(0);
                let next = if m.shift {
                    (i + n - 1) % n
                } else {
                    (i + 1) % n
                };
                set_focus(self, next);
            }
            (Key::Named(NamedKey::ArrowDown), _) => {
                set_focus(self, (focus.unwrap_or(0) + 1).min(f.len() - 1))
            }
            (Key::Named(NamedKey::ArrowUp), _) => {
                set_focus(self, focus.unwrap_or(0).saturating_sub(1))
            }
            (Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowRight), _) => {
                if let Some(Hit::Row(r)) = focus.map(|i| f[i]) {
                    let right = matches!(c.key, Key::Named(NamedKey::ArrowRight));
                    self.annot_step(r, if right != rtl { 1 } else { -1 });
                }
            }
            (Key::Named(NamedKey::Enter | NamedKey::Space), _) => match focus.map(|i| f[i]) {
                Some(Hit::Row(r)) => self.annot_step(r, 1),
                Some(h) => self.annot_hit(h),
                None => {}
            },
            _ => {}
        }
        true
    }

    pub(super) fn annot_export_click(&mut self, p: (f32, f32), down: bool) -> bool {
        if !down {
            return true;
        }
        let Some(d) = self.annot_export.drawn.clone() else {
            return true;
        };
        if let Some((_, h)) = d.hits.iter().rev().find(|(r, _)| r.contains(p)) {
            self.annot_hit(*h);
        } else if !d.sheet.contains(p) {
            self.annot_export_close();
        }
        true
    }

    pub(super) fn annot_export_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        let (Some(v), Some(d)) = (self.annot_export_view(), self.annot_export.drawn.clone()) else {
            return false;
        };
        let Some(h) = a11y_target(&v, &d, req.target_node.0) else {
            return false;
        };
        let f = v.focusables();
        match (req.action, h) {
            (accesskit::Action::Increment, Hit::Row(r)) => self.annot_step(r, 1),
            (accesskit::Action::Decrement, Hit::Row(r)) => self.annot_step(r, -1),
            (accesskit::Action::Click, h) => self.annot_hit(h),
            (accesskit::Action::Focus, h) => {
                if let Some(dlg) = self.annot_export.dialog.as_mut() {
                    dlg.focus = f.iter().position(|x| *x == h);
                }
                self.redraw = true;
            }
            _ => {}
        }
        true
    }
}
