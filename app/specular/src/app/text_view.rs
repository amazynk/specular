//! Text-like files in the viewer, drawn with the text stack over the page preview: plain
//! text and logs (line numbers, wrapping on or off, DOS art in its own tight lines), a
//! Markdown file's source (its rendered pages are the image; a button switches), CSV and
//! TSV tables as a grid with a sticky header row, and subtitle files as their list of cues
//! (picture cues as pictures).
//!
//! Only what is on screen is read: lines and rows come from the indexes built at open over
//! the mapped file, so a log of gigabytes or a table of millions of rows scrolls at once.
//! The wheel scrolls (Shift or a sideways wheel moves across unwrapped lines and columns);
//! the scroll bar drags.

#[cfg(test)]
mod shots;

use std::sync::Arc;

use specular_decode::text_dec::cue_time;
use specular_decode::{SubtitleModel, TextModel};
use specular_gfx::{Align, DrawList, Gpu, Rect, TexId, Text, TextStyle, UiRenderer, wgpu};
use specular_i18n::t;
use specular_theme::metrics::{FONT_BODY, FONT_SMALL, RADIUS_SM};
use specular_theme::{Color, Palette};
use specular_widgets::accesskit;

use super::App;
use crate::loader::ImageId;

/// AccessKit ids (the font specimen uses 0x6200...).
const A11Y_ID: u64 = 0x6300_0000_0000_0000;
const A11Y_WRAP: u64 = A11Y_ID + 1;
const A11Y_MODE: u64 = A11Y_ID + 2;
const A11Y_BODY: u64 = A11Y_ID + 3;

/// Characters of a line laid out when wrapping (the rest of a longer line is cut).
const WRAP_CHARS: usize = 4000;
/// Characters of a table cell shown.
const CELL_CHARS: usize = 200;
/// Widest table column, in characters.
const MAX_COLUMN_CHARS: usize = 40;
/// Rows sampled for the table's column widths.
const WIDTH_SAMPLE: usize = 200;
/// Tallest picture cue (logical px).
const CUE_PICTURE_H: f32 = 90.0;
/// Picture cue textures kept.
const MAX_TEXTURES: usize = 48;
/// Lines a wheel step scrolls.
const WHEEL_LINES: f32 = 3.0;

/// Which view of the model shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Lines,
    Table,
    Cues,
    /// Markdown shown as its rendered pages: only the switch is drawn.
    Rendered,
}

/// What the last frame drew, to hit-test.
#[derive(Clone, Debug, Default)]
pub(super) struct Drawn {
    pub area: Rect,
    pub wrap: Option<Rect>,
    pub mode: Option<Rect>,
    pub body: Rect,
    /// The scroll bar's track and thumb.
    pub track: Option<Rect>,
    pub thumb: Option<Rect>,
    /// The text on screen, for screen readers.
    pub visible: String,
    pub label: String,
}

#[derive(Default)]
pub(super) struct TextView {
    id: Option<ImageId>,
    model: Option<TextModel>,
    name: String,
    wrap: bool,
    rendered: bool,
    /// The first line, row or cue shown, and logical px scrolled into it.
    top: u64,
    offset: f32,
    /// Unwrapped text: characters scrolled sideways; tables: the first column shown.
    across: usize,
    /// Table column widths in characters.
    widths: Vec<usize>,
    drag: Option<f32>,
    drawn: Option<Drawn>,
    /// Picture cues as textures: (cue index, texture).
    tex: Vec<(usize, wgpu::Texture, TexId)>,
    /// Picture cues on screen without a texture yet.
    want: Vec<usize>,
    retired: Vec<TexId>,
}

impl TextView {
    pub(super) fn gpu_reset(&mut self) {
        self.tex.clear();
        self.retired.clear();
    }

    /// Shows `model` for image `id` (keeps the state when it is already shown).
    pub(super) fn show(&mut self, id: ImageId, model: &TextModel, name: &str) {
        if self.id == Some(id) {
            return;
        }
        self.clear();
        self.id = Some(id);
        self.model = Some(model.clone());
        self.name = name.to_owned();
        self.rendered = matches!(model, TextModel::Markdown(_));
        self.wrap = !matches!(model, TextModel::Plain { dos: true, .. });
        if let TextModel::Table(t) = model {
            self.widths = column_widths(&t.rows(0, WIDTH_SAMPLE), t.table.columns);
        }
    }

    pub(super) fn clear(&mut self) {
        let mut retired = std::mem::take(&mut self.retired);
        retired.extend(self.tex.drain(..).map(|t| t.2));
        *self = TextView {
            retired,
            ..TextView::default()
        };
    }

    pub(super) fn active(&self) -> bool {
        self.model.is_some()
    }

    /// Whether the view covers the image (not Markdown shown rendered).
    pub(super) fn covers(&self) -> bool {
        self.model.is_some() && self.mode() != Mode::Rendered
    }

    fn mode(&self) -> Mode {
        match &self.model {
            Some(TextModel::Table(_)) => Mode::Table,
            Some(TextModel::Subtitles(_)) => Mode::Cues,
            Some(TextModel::Markdown(_)) if self.rendered => Mode::Rendered,
            _ => Mode::Lines,
        }
    }

    /// Lines, rows or cues in the model.
    fn total(&self) -> u64 {
        match &self.model {
            Some(TextModel::Plain { file, .. } | TextModel::Markdown(file)) => file.index.len(),
            Some(TextModel::Table(t)) => t.table.len(),
            Some(TextModel::Subtitles(s)) => s.track.cues.len() as u64,
            None => 0,
        }
    }

    /// Releases retired textures and uploads the picture cues on screen.
    pub(super) fn prepare(&mut self, gpu: &Gpu, ui: &mut UiRenderer) {
        for id in self.retired.drain(..) {
            ui.unregister_texture(id);
        }
        let Some(TextModel::Subtitles(s)) = &self.model else {
            return;
        };
        let s = s.clone();
        for i in std::mem::take(&mut self.want) {
            if self.tex.iter().any(|t| t.0 == i) {
                continue;
            }
            let Some(specular_decode::specular_subs::Body::Bitmap(b)) =
                s.track.cues.get(i).map(|c| &c.body)
            else {
                continue;
            };
            if b.w == 0 || b.h == 0 || b.rgba.len() < (b.w * b.h * 4) as usize {
                continue;
            }
            if self.tex.len() >= MAX_TEXTURES {
                let old = self.tex.remove(0);
                ui.unregister_texture(old.2);
            }
            let (t, id) = upload(gpu, ui, b.w, b.h, &b.rgba);
            self.tex.push((i, t, id));
        }
    }

    /// Whether a picture cue on screen still waits for its texture.
    pub(super) fn wants_upload(&self) -> bool {
        !self.want.is_empty()
    }

    /// Scrolls by `dy` logical px (down positive) and `dx` across.
    fn scroll(&mut self, dy: f32, dx: f32, line_h: f32, char_w: f32) {
        self.offset += dy;
        if !self.wrap || self.mode() == Mode::Table {
            // Fixed rows: whole rows into `top`.
            let rows = (self.offset / line_h).floor();
            let top = (self.top as f64 + f64::from(rows)).max(0.0);
            self.top = (top as u64).min(self.total().saturating_sub(1));
            self.offset -= rows * line_h;
            if top <= 0.0 && self.offset < 0.0 {
                self.offset = 0.0;
            }
        }
        if dx != 0.0 {
            let step = match self.mode() {
                Mode::Table => (dx / (char_w * 8.0)).round() as i64,
                _ => (dx / char_w).round() as i64,
            };
            let max = match self.mode() {
                Mode::Table => self.widths.len().saturating_sub(1) as i64,
                _ => 1_000_000,
            };
            self.across = (self.across as i64 + step).clamp(0, max) as usize;
        }
    }

    /// Jumps to a fraction of the way down.
    fn seek(&mut self, f: f32) {
        let total = self.total();
        self.top = ((f.clamp(0.0, 1.0) as f64 * total as f64) as u64).min(total.saturating_sub(1));
        self.offset = 0.0;
    }
}

impl App {
    /// Shows the current image's text model, else drops it.
    pub(super) fn text_sync(&mut self) {
        let cur = self.current_id();
        let session = cur
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.session.as_ref());
        let model = session.and_then(|s| s.text().cloned());
        let name = session
            .and_then(|s| s.source().path().file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        match (cur, model) {
            (Some(id), Some(m)) => {
                if self.text_view.id != Some(id) {
                    self.text_view.show(id, &m, &name);
                    self.redraw = true;
                }
            }
            _ => {
                if self.text_view.active() {
                    self.text_view.clear();
                    self.redraw = true;
                }
            }
        }
    }

    /// Whether the text view covers the image.
    pub(super) fn text_active(&self) -> bool {
        self.text_view.covers() && !self.gallery_active()
    }

    /// A press at logical `lp`: the switches, the scroll bar. True when taken.
    pub(super) fn text_press(&mut self, lp: (f32, f32)) -> bool {
        if !self.text_view.active() || self.gallery_active() {
            return false;
        }
        let Some(d) = self.text_view.drawn.clone() else {
            return false;
        };
        let v = &mut self.text_view;
        if d.wrap.is_some_and(|r| r.contains(lp)) {
            v.wrap = !v.wrap;
            v.offset = 0.0;
            v.across = 0;
            self.redraw = true;
            return true;
        }
        if d.mode.is_some_and(|r| r.contains(lp)) {
            v.rendered = !v.rendered;
            self.redraw = true;
            return true;
        }
        if !v.covers() || !d.area.contains(lp) {
            return false;
        }
        if let Some(track) = d.track
            && track.inset_xy(-4.0, 0.0).contains(lp)
        {
            let grab = d
                .thumb
                .filter(|t| t.inset_xy(-4.0, 0.0).contains(lp))
                .map_or(0.0, |t| lp.1 - t.y);
            v.drag = Some(grab);
            if grab == 0.0 {
                v.seek((lp.1 - track.y) / track.h.max(1.0));
            }
            self.redraw = true;
            return true;
        }
        // Presses on the text itself are the view's (no image panning under it).
        true
    }

    /// The pointer moved to `p` (physical): drags the scroll bar. True when dragging.
    pub(super) fn text_motion(&mut self, p: (f32, f32)) -> bool {
        let Some(grab) = self.text_view.drag else {
            return false;
        };
        let lp = self.logical(p);
        if let Some(d) = &self.text_view.drawn
            && let (Some(track), Some(thumb)) = (d.track, d.thumb)
        {
            let span = (track.h - thumb.h).max(1.0);
            let f = (lp.1 - grab - track.y) / span;
            self.text_view.seek(f);
            self.redraw = true;
        }
        true
    }

    pub(super) fn text_release(&mut self) -> bool {
        self.text_view.drag.take().is_some()
    }

    /// The wheel over the view scrolls it (`dx` sideways, or Shift with `dy`). True when
    /// taken.
    pub(super) fn text_wheel(&mut self, dx: f32, dy: f32, pixels: bool, lp: (f32, f32)) -> bool {
        if !self.text_active() {
            return false;
        }
        let Some(d) = &self.text_view.drawn else {
            return false;
        };
        if !d.area.contains(lp) {
            return false;
        }
        let ts = self.chrome.text_scale;
        let line_h = (FONT_BODY * ts * 1.35).round();
        let char_w = FONT_BODY * ts * 0.6;
        let k = if pixels { 1.0 } else { line_h * WHEEL_LINES };
        let (mut dx, mut dy) = (-dx * k, -dy * k);
        if self.mods.shift_key() && dx == 0.0 {
            std::mem::swap(&mut dx, &mut dy);
        }
        self.text_view.scroll(dy, dx, line_h, char_w);
        self.redraw = true;
        true
    }

    /// A screen reader's action on a switch; true when the node is the view's.
    pub(super) fn text_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        let id = req.target_node.0;
        if !(A11Y_ID..=A11Y_BODY).contains(&id) {
            return false;
        }
        let v = &mut self.text_view;
        match (id, req.action) {
            (A11Y_WRAP, accesskit::Action::Click) => v.wrap = !v.wrap,
            (A11Y_MODE, accesskit::Action::Click) => v.rendered = !v.rendered,
            (A11Y_BODY, accesskit::Action::ScrollDown) => {
                v.top = (v.top + 20).min(v.total().saturating_sub(1))
            }
            (A11Y_BODY, accesskit::Action::ScrollUp) => v.top = v.top.saturating_sub(20),
            _ => {}
        }
        self.redraw = true;
        true
    }
}

/// Column widths in characters from sampled rows.
fn column_widths(rows: &[Vec<String>], columns: usize) -> Vec<usize> {
    let mut w = vec![3usize; columns.max(1)];
    for r in rows {
        for (k, f) in r.iter().enumerate() {
            if let Some(slot) = w.get_mut(k) {
                let n = f.lines().map(|l| l.chars().count()).max().unwrap_or(0);
                *slot = (*slot).max(n.min(MAX_COLUMN_CHARS));
            }
        }
    }
    w
}

/// Tabs to spaces at stops of 8; control characters (terminal colours) as spaces.
fn clean_line(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    let mut col = 0usize;
    for c in s.chars() {
        match c {
            '\t' => {
                let n = 8 - col % 8;
                o.extend(std::iter::repeat_n(' ', n));
                col += n;
            }
            c if c.is_control() => {
                o.push(' ');
                col += 1;
            }
            c => {
                o.push(c);
                col += 1;
            }
        }
    }
    o
}

/// Whether `s` holds right-to-left letters (laid out with bidi, not as code).
fn has_rtl(s: &str) -> bool {
    s.chars()
        .any(|c| matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF))
}

fn one_line(s: &str) -> String {
    s.replace(['\r', '\n'], " ⏎ ")
}

/// A toggle button: its label, filled when on.
fn button(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    label: &str,
    r: Rect,
    on: bool,
    ts: f32,
) {
    let fill = if on {
        pal.accent_soft
    } else {
        pal.surface_raised
    };
    dl.rect_border(r, RADIUS_SM, fill, pal.separator, 1.0);
    let b = text.layout(label, TextStyle::new(FONT_SMALL * ts), None, Align::Start);
    text.draw(
        dl,
        &b,
        r.x + (r.w - b.width) / 2.0,
        r.y + (r.h - b.height) / 2.0,
        pal.text,
    );
}

/// The width a toggle with `label` needs.
fn button_w(text: &mut Text, label: &str, ts: f32) -> f32 {
    text.layout(label, TextStyle::new(FONT_SMALL * ts), None, Align::Start)
        .width
        + 20.0 * ts
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    backdrop: Color,
    v: &mut TextView,
    area: Rect,
    rtl: bool,
    ts: f32,
) {
    let Some(model) = v.model.clone() else {
        v.drawn = None;
        return;
    };
    let mode = v.mode();
    let pad = 8.0 * ts;
    let bar_h = 30.0 * ts;
    let bh = 22.0 * ts;
    let mut drawn = Drawn {
        area,
        ..Drawn::default()
    };
    // The Markdown switch, alone over the rendered pages.
    let mode_label = if v.rendered {
        t!("text-view-show-source")
    } else {
        t!("text-view-show-rendered")
    };
    if mode == Mode::Rendered {
        let w = button_w(text, &mode_label, ts);
        let r = Rect::new(
            if rtl {
                area.x + pad
            } else {
                area.right() - w - pad
            },
            area.y + pad,
            w,
            bh,
        );
        button(dl, text, pal, &mode_label, r, false, ts);
        drawn.mode = Some(r);
        drawn.label = t!("text-view-markdown", name = v.name.clone());
        v.drawn = Some(drawn);
        return;
    }
    dl.rect(area, 0.0, backdrop);
    // The bar: what the file holds, and the switches.
    let bar = Rect::new(area.x, area.y, area.w, bar_h);
    dl.rect(bar, 0.0, pal.surface);
    dl.line(
        (bar.x, bar.bottom()),
        (bar.right(), bar.bottom()),
        1.0,
        pal.separator,
    );
    let total = v.total();
    let status = match &model {
        TextModel::Plain { file, .. } | TextModel::Markdown(file) => specular_i18n::dot_join(&[
            file.charset.name().to_owned(),
            t!("text-view-lines", count = total as i64),
        ]),
        TextModel::Table(t) => t!(
            "text-view-rows",
            rows = total.saturating_sub(u64::from(t.table.header)) as i64,
            columns = t.table.columns as i64
        ),
        TextModel::Subtitles(s) => specular_i18n::dot_join(&[
            s.format.to_owned(),
            t!("text-view-cues", count = total as i64),
        ]),
    };
    let sb = text.layout(&status, TextStyle::new(FONT_SMALL * ts), None, Align::Start);
    let sx = if rtl {
        bar.right() - pad - sb.width
    } else {
        bar.x + pad
    };
    text.draw(
        dl,
        &sb,
        sx,
        bar.y + (bar_h - sb.height) / 2.0,
        pal.text_muted,
    );
    let mut bx = if rtl { bar.x + pad } else { bar.right() - pad };
    let mut place = |w: f32| {
        if rtl {
            let r = Rect::new(bx, bar.y + (bar_h - bh) / 2.0, w, bh);
            bx += w + 6.0 * ts;
            r
        } else {
            bx -= w;
            let r = Rect::new(bx, bar.y + (bar_h - bh) / 2.0, w, bh);
            bx -= 6.0 * ts;
            r
        }
    };
    if matches!(model, TextModel::Markdown(_)) {
        let w = button_w(text, &mode_label, ts);
        let r = place(w);
        button(dl, text, pal, &mode_label, r, false, ts);
        drawn.mode = Some(r);
    }
    if mode == Mode::Lines {
        let label = t!("text-view-wrap");
        let w = button_w(text, &label, ts);
        let r = place(w);
        button(dl, text, pal, &label, r, v.wrap, ts);
        drawn.wrap = Some(r);
    }
    // The body, with the scroll bar on the end side.
    let sbw = 10.0 * ts;
    let body = Rect::new(area.x, bar.bottom(), area.w, (area.h - bar_h).max(0.0));
    let track = Rect::new(
        if rtl {
            body.x + 2.0
        } else {
            body.right() - sbw - 2.0
        },
        body.y + 2.0,
        sbw,
        (body.h - 4.0).max(0.0),
    );
    let inner = Rect::new(
        if rtl { body.x + sbw + 4.0 } else { body.x },
        body.y,
        (body.w - sbw - 4.0).max(0.0),
        body.h,
    );
    drawn.body = inner;
    dl.push_clip(inner);
    let mut visible = String::new();
    let shown = match &model {
        TextModel::Plain { file, dos } => {
            draw_lines(dl, text, pal, v, file, *dos, inner, rtl, ts, &mut visible)
        }
        TextModel::Markdown(file) => {
            draw_lines(dl, text, pal, v, file, false, inner, rtl, ts, &mut visible)
        }
        TextModel::Table(t) => draw_table(dl, text, pal, v, t, inner, rtl, ts, &mut visible),
        TextModel::Subtitles(s) => draw_cues(dl, text, pal, v, s, inner, rtl, ts, &mut visible),
    };
    dl.pop_clip();
    // The scroll bar: the thumb's length is the share on screen.
    // A table's header row is not scrolled: the bar covers the rows under it.
    let base = match &model {
        TextModel::Table(t) => u64::from(t.table.header),
        _ => 0,
    };
    let rows = total.saturating_sub(base);
    let all_shown = v.top <= base && shown >= rows;
    if rows > 1 && track.h > 0.0 && !all_shown {
        dl.rect(track, sbw / 2.0, pal.surface_hover);
        let frac = (shown as f32 / rows as f32).clamp(0.02, 1.0);
        let th = (track.h * frac).max(18.0 * ts).min(track.h);
        let pos = (v.top.saturating_sub(base) as f64 / rows.saturating_sub(shown).max(1) as f64)
            .clamp(0.0, 1.0) as f32;
        let thumb = Rect::new(track.x, track.y + (track.h - th) * pos, track.w, th);
        dl.rect(thumb, sbw / 2.0, pal.text_faint);
        drawn.track = Some(track);
        drawn.thumb = Some(thumb);
    }
    drawn.visible = visible;
    drawn.label = match &model {
        TextModel::Table(_) => t!("text-view-table", name = v.name.clone()),
        TextModel::Subtitles(_) => t!("text-view-subtitles", name = v.name.clone()),
        TextModel::Markdown(_) => t!("text-view-markdown", name = v.name.clone()),
        TextModel::Plain { .. } => t!("text-view-text", name = v.name.clone()),
    };
    v.drawn = Some(drawn);
}

/// Lines of text with their numbers. Returns how many lines are on screen.
#[allow(clippy::too_many_arguments)]
fn draw_lines(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    v: &mut TextView,
    file: &specular_decode::specular_textdoc::TextFile,
    dos: bool,
    r: Rect,
    rtl: bool,
    ts: f32,
    visible: &mut String,
) -> u64 {
    let size = FONT_BODY * ts;
    let mut code = TextStyle::new(size).code();
    if dos {
        // DOS art: block characters touch line to line.
        code.line_height = 1.0;
    }
    let line_h = (size * code.line_height).round().max(1.0);
    let total = file.index.len();
    let digits = total.max(1).to_string().len();
    let num_style = TextStyle::new(FONT_SMALL * ts).tabular();
    let gutter = text
        .layout(&"0".repeat(digits), num_style, None, Align::Start)
        .width
        + 16.0 * ts;
    let char_w = text.layout("0000000000", code, None, Align::Start).width / 10.0;
    let text_x = if rtl { r.x + 8.0 * ts } else { r.x + gutter };
    let text_w = (r.w - gutter - 8.0 * ts).max(20.0);
    // Wrapped lines differ in height: settle `offset` against the lines it crosses.
    let height_of = |text: &mut Text, s: &str| -> f32 {
        let s = clean_line(s);
        let s: String = s.chars().take(WRAP_CHARS).collect();
        let st = if has_rtl(&s) {
            TextStyle::new(size)
        } else {
            code
        };
        text.layout(&s, st, Some(text_w), Align::Start)
            .height
            .max(line_h)
    };
    if v.wrap {
        while v.offset < 0.0 && v.top > 0 {
            v.top -= 1;
            let l = file.lines(v.top, 1);
            v.offset += height_of(text, l.first().map_or("", String::as_str));
        }
        v.offset = v.offset.max(0.0);
        loop {
            if v.top + 1 >= total {
                v.offset = 0.0;
                break;
            }
            let l = file.lines(v.top, 1);
            let h = height_of(text, l.first().map_or("", String::as_str));
            if v.offset < h {
                break;
            }
            v.offset -= h;
            v.top += 1;
        }
    }
    v.top = v.top.min(total.saturating_sub(1));
    let rows = (r.h / line_h).ceil() as usize + 1;
    let lines = file.lines(v.top, rows);
    let mut y = r.y + 4.0 * ts - if v.wrap { v.offset } else { 0.0 };
    let mut shown = 0u64;
    let cols = (text_w / char_w.max(1.0)).ceil() as usize + 1;
    for (k, line) in lines.iter().enumerate() {
        if y > r.bottom() {
            break;
        }
        let n = v.top + k as u64;
        let line = clean_line(line);
        let rtl_line = has_rtl(&line);
        let st = if rtl_line { TextStyle::new(size) } else { code };
        let block = if v.wrap {
            let s: String = line.chars().take(WRAP_CHARS).collect();
            text.layout(&s, st, Some(text_w), Align::Start)
        } else {
            // Only the characters across the view are laid out.
            let s: String = line.chars().skip(v.across).take(cols).collect();
            text.layout(&s, st, None, Align::Start)
        };
        let nb = text.layout(&(n + 1).to_string(), num_style, None, Align::Start);
        let nx = if rtl {
            r.right() - 8.0 * ts - nb.width
        } else {
            r.x + gutter - 8.0 * ts - nb.width
        };
        text.draw(dl, &nb, nx, y + (line_h - nb.height) / 2.0, pal.text_faint);
        let bx = if rtl_line || rtl {
            text_x + text_w - block.width
        } else {
            text_x
        };
        let bx = if rtl && !rtl_line { text_x } else { bx };
        text.draw(dl, &block, bx, y, pal.text);
        if visible.len() < 4000 {
            visible.push_str(&line);
            visible.push('\n');
        }
        y += block.height.max(line_h);
        shown += 1;
    }
    // The gutter's edge.
    let gx = if rtl {
        r.right() - gutter + 4.0 * ts
    } else {
        r.x + gutter - 4.0 * ts
    };
    dl.line((gx, r.y), (gx, r.bottom()), 1.0, pal.separator);
    shown
}

/// The table grid: row numbers, the header row kept on top, columns from `v.across`.
#[allow(clippy::too_many_arguments)]
fn draw_table(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    v: &mut TextView,
    t: &specular_decode::specular_textdoc::TableFile,
    r: Rect,
    rtl: bool,
    ts: f32,
    visible: &mut String,
) -> u64 {
    let st = TextStyle::new(FONT_SMALL * ts);
    let head_st = st.weight(600.0);
    let num_st = st.tabular();
    let row_h = (FONT_BODY * ts * 1.35).round();
    let char_w = FONT_SMALL * ts * 0.62;
    let total = t.table.len();
    let header = t.table.header;
    let first_data = u64::from(header);
    v.top = v.top.clamp(
        first_data.min(total),
        total.saturating_sub(1).max(first_data.min(total)),
    );
    let digits = total.max(1).to_string().len();
    let gutter = text
        .layout(&"0".repeat(digits), num_st, None, Align::Start)
        .width
        + 14.0 * ts;
    let rows_on = ((r.h / row_h).ceil() as usize).saturating_sub(usize::from(header)) + 1;
    let data = t.rows(v.top, rows_on);
    let head = if header {
        t.rows(0, 1).into_iter().next()
    } else {
        None
    };
    // Columns from `across`, as many as fit.
    let mut cols: Vec<(usize, f32, f32)> = Vec::new();
    let mut x = gutter;
    for (k, &w) in v.widths.iter().enumerate().skip(v.across) {
        let wpx = (w as f32 + 2.0) * char_w;
        cols.push((k, x, wpx));
        x += wpx;
        if x > r.w {
            break;
        }
    }
    let px = |x: f32, w: f32| if rtl { r.right() - x - w } else { r.x + x };
    let mut y = r.y;
    let draw_row = |dl: &mut DrawList,
                    text: &mut Text,
                    y: f32,
                    label: String,
                    cells: &[String],
                    bold: bool,
                    fill: Option<Color>| {
        if let Some(f) = fill {
            dl.rect(Rect::new(r.x, y, r.w, row_h), 0.0, f);
        }
        let nb = text.layout(&label, num_st, None, Align::Start);
        text.draw(
            dl,
            &nb,
            px(gutter - 6.0 * ts - nb.width, nb.width),
            y + (row_h - nb.height) / 2.0,
            pal.text_faint,
        );
        for &(k, x, w) in &cols {
            let Some(c) = cells.get(k) else { continue };
            let s: String = one_line(c).chars().take(CELL_CHARS).collect();
            let b = text.layout(&s, if bold { head_st } else { st }, None, Align::Start);
            let cx = px(x + 4.0 * ts, w - 8.0 * ts);
            dl.push_clip(Rect::new(cx, y, (w - 8.0 * ts).max(1.0), row_h));
            let bx = if rtl {
                cx + (w - 8.0 * ts) - b.width
            } else {
                cx
            };
            text.draw(dl, &b, bx, y + (row_h - b.height) / 2.0, pal.text);
            dl.pop_clip();
        }
        dl.line((r.x, y + row_h), (r.right(), y + row_h), 1.0, pal.separator);
    };
    if let Some(h) = &head {
        draw_row(dl, text, y, String::new(), h, true, Some(pal.surface));
        if visible.len() < 4000 {
            visible.push_str(&h.join("\t"));
            visible.push('\n');
        }
        y += row_h;
    }
    let mut shown = 0;
    for (k, row) in data.iter().enumerate() {
        if y > r.bottom() {
            break;
        }
        let n = v.top + k as u64;
        let fill = (n % 2 == 1).then_some(pal.surface_hover);
        draw_row(
            dl,
            text,
            y,
            (n + 1 - first_data).to_string(),
            row,
            false,
            fill,
        );
        if visible.len() < 4000 {
            visible.push_str(&row.join("\t"));
            visible.push('\n');
        }
        y += row_h;
        shown += 1;
    }
    // Column rules.
    for &(_, x, w) in &cols {
        let lx = px(x + w, 0.0);
        dl.line((lx, r.y), (lx, y.min(r.bottom())), 1.0, pal.separator);
    }
    let gx = px(gutter, 0.0);
    dl.line((gx, r.y), (gx, y.min(r.bottom())), 1.0, pal.separator);
    shown
}

/// The cue list: times, then the words or the picture.
#[allow(clippy::too_many_arguments)]
fn draw_cues(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    v: &mut TextView,
    s: &Arc<SubtitleModel>,
    r: Rect,
    rtl: bool,
    ts: f32,
    visible: &mut String,
) -> u64 {
    use specular_decode::specular_subs::Body;
    let st = TextStyle::new(FONT_BODY * ts);
    let time_st = TextStyle::new(FONT_SMALL * ts).tabular();
    let total = s.track.cues.len() as u64;
    let pad = 8.0 * ts;
    let time_w = text
        .layout("0:00:00.000 → 0:00:00.000", time_st, None, Align::Start)
        .width
        + 2.0 * pad;
    let text_w = (r.w - time_w - pad).max(40.0);
    let height = |text: &mut Text, i: u64| -> f32 {
        match s.track.cues.get(i as usize).map(|c| &c.body) {
            Some(Body::Text(c)) => {
                text.layout(&c.plain_text(), st, Some(text_w), Align::Start)
                    .height
                    + 2.0 * pad
            }
            Some(Body::Bitmap(b)) => (b.h as f32 / ts.max(0.1)).min(CUE_PICTURE_H * ts) + 2.0 * pad,
            None => 0.0,
        }
    };
    while v.offset < 0.0 && v.top > 0 {
        v.top -= 1;
        v.offset += height(text, v.top);
    }
    v.offset = v.offset.max(0.0);
    loop {
        if v.top + 1 >= total {
            v.offset = 0.0;
            break;
        }
        let h = height(text, v.top);
        if v.offset < h {
            break;
        }
        v.offset -= h;
        v.top += 1;
    }
    v.top = v.top.min(total.saturating_sub(1));
    let mut y = r.y - v.offset;
    let mut shown = 0;
    let tx = if rtl { r.x + pad } else { r.x + time_w };
    for i in v.top..total {
        if y > r.bottom() {
            break;
        }
        let Some(c) = s.track.cues.get(i as usize) else {
            break;
        };
        let h = height(text, i);
        if i % 2 == 1 {
            dl.rect(Rect::new(r.x, y, r.w, h), 0.0, pal.surface_hover);
        }
        let times = format!("{} → {}", cue_time(c.start_us), cue_time(c.end_us));
        let tb = text.layout(&times, time_st, None, Align::Start);
        let tbx = if rtl {
            r.right() - pad - tb.width
        } else {
            r.x + pad
        };
        text.draw(dl, &tb, tbx, y + pad, pal.text_muted);
        match &c.body {
            Body::Text(cue) => {
                let words = cue.plain_text();
                let b = text.layout(&words, st, Some(text_w), Align::Start);
                let bx = if rtl { tx + text_w - b.width } else { tx };
                text.draw(dl, &b, bx, y + pad, pal.text);
                if visible.len() < 4000 {
                    visible.push_str(&times);
                    visible.push(' ');
                    visible.push_str(&words);
                    visible.push('\n');
                }
            }
            Body::Bitmap(b) => {
                let ph = (b.h as f32 / ts.max(0.1)).min(CUE_PICTURE_H * ts);
                let pw = (b.w as f32 * ph / b.h.max(1) as f32).min(text_w);
                let pr = Rect::new(if rtl { tx + text_w - pw } else { tx }, y + pad, pw, ph);
                // Picture subtitles are light on dark: on a dark plate.
                dl.rect(pr, 2.0, Color::hex(0x1f1f1f));
                match v.tex.iter().find(|t| t.0 == i as usize) {
                    Some((_, _, id)) => dl.image(pr, [0.0, 0.0, 1.0, 1.0], *id, 2.0),
                    None => {
                        if !v.want.contains(&(i as usize)) {
                            v.want.push(i as usize);
                        }
                    }
                }
                if visible.len() < 4000 {
                    visible.push_str(&times);
                    visible.push(' ');
                    visible.push_str(&t!("text-view-picture-cue"));
                    visible.push('\n');
                }
            }
        }
        y += h;
        shown += 1;
    }
    shown
}

/// The AccessKit nodes: the view as a group with its switches and the text on screen.
pub(super) fn a11y(tree: &mut accesskit::TreeUpdate, v: &TextView, sf: f64) {
    use accesskit::{Node, NodeId, Role, Toggled};
    let Some(d) = &v.drawn else {
        return;
    };
    let bounds = |r: Rect| accesskit::Rect {
        x0: f64::from(r.x) * sf,
        y0: f64::from(r.y) * sf,
        x1: f64::from(r.x + r.w) * sf,
        y1: f64::from(r.y + r.h) * sf,
    };
    let mut g = Node::new(Role::Group);
    g.set_label(d.label.clone());
    g.set_bounds(bounds(d.area));
    let mut children = Vec::new();
    if let Some(r) = d.wrap {
        let mut n = Node::new(Role::CheckBox);
        n.set_label(t!("text-view-wrap"));
        n.set_toggled(if v.wrap {
            Toggled::True
        } else {
            Toggled::False
        });
        n.set_bounds(bounds(r));
        n.add_action(accesskit::Action::Click);
        g.push_child(NodeId(A11Y_WRAP));
        children.push((NodeId(A11Y_WRAP), n));
    }
    if let Some(r) = d.mode {
        let mut n = Node::new(Role::Button);
        n.set_label(if v.rendered {
            t!("text-view-show-source")
        } else {
            t!("text-view-show-rendered")
        });
        n.set_bounds(bounds(r));
        n.add_action(accesskit::Action::Click);
        g.push_child(NodeId(A11Y_MODE));
        children.push((NodeId(A11Y_MODE), n));
    }
    if v.covers() {
        let mut n = Node::new(Role::Document);
        n.set_label(d.label.clone());
        n.set_value(d.visible.clone());
        n.set_bounds(bounds(d.body));
        n.add_action(accesskit::Action::ScrollDown);
        n.add_action(accesskit::Action::ScrollUp);
        g.push_child(NodeId(A11Y_BODY));
        children.push((NodeId(A11Y_BODY), n));
    }
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        root.push_child(NodeId(A11Y_ID));
    }
    tree.nodes.push((NodeId(A11Y_ID), g));
    tree.nodes.extend(children);
}

/// Uploads an RGBA picture into a new sRGB texture the UI can draw.
fn upload(gpu: &Gpu, ui: &mut UiRenderer, w: u32, h: u32, rgba: &[u8]) -> (wgpu::Texture, TexId) {
    let size = wgpu::Extent3d {
        width: w,
        height: h,
        depth_or_array_layers: 1,
    };
    let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("subtitle picture"),
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
        &rgba[..(w * h * 4) as usize],
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(clean_line("a\tb\x1b[0m"), "a       b [0m");
        assert!(has_rtl("abc שלום"));
        assert!(!has_rtl("abc"));
        let w = column_widths(&[vec!["name".into(), "x".repeat(100)], vec!["a".into()]], 3);
        assert_eq!(w, [4, MAX_COLUMN_CHARS, 3]);
    }

    #[test]
    fn scrolls_fixed_rows() {
        let file = specular_decode::specular_textdoc::TextFile::open(
            Arc::new(
                (0..100)
                    .map(|i| format!("line {i}\n"))
                    .collect::<String>()
                    .into_bytes(),
            ),
            false,
            &|| false,
        )
        .unwrap();
        let mut v = TextView::default();
        v.show(
            1,
            &TextModel::Plain {
                file: Arc::new(file),
                dos: false,
            },
            "a.txt",
        );
        v.wrap = false;
        v.scroll(55.0, 0.0, 10.0, 6.0);
        assert_eq!((v.top, v.offset), (5, 5.0));
        v.scroll(-100.0, 0.0, 10.0, 6.0);
        assert_eq!(v.top, 0);
        v.scroll(1e6, 0.0, 10.0, 6.0);
        assert_eq!(v.top, 99);
        v.seek(0.5);
        assert_eq!(v.top, 50);
        v.scroll(0.0, 60.0, 10.0, 6.0);
        assert_eq!(v.across, 10);
    }
}
