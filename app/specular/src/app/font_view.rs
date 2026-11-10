//! Fonts (TTF, OTF, TTC/OTC, WOFF, WOFF2) in the viewer: a specimen page drawn on the GPU
//! with the text stack, in the file's own face (no fallback: a character it lacks shows its
//! missing glyph). The page has the face's names (family, style, version, designer,
//! licence), sliders for the axes of a variable font, a pangram at several sizes, the colour
//! glyphs of a colour font where the text stack draws them (or a line saying what it does
//! not), and every glyph in a grid of pages.
//!
//! The wheel scrolls the page; the sliders drag (or step with a screen reader); the arrows
//! above the grid turn its pages. A collection's faces are the file's pages (Alt+Page
//! Down/Up). The session's raster is a small preview drawn on the CPU from our own outlines
//! (thumbnails, printing); the specimen covers it.

#[cfg(test)]
mod shots;

use std::sync::Arc;

use specular_decode::FontModel;
use specular_decode::specular_font::{Face, OutlineKind};
use specular_gfx::{
    Align, DrawList, FontFace, Gpu, Rect, TexId, Text, TextBlock, TextStyle, UiRenderer, wgpu,
};
use specular_i18n::t;
use specular_theme::metrics::{FONT_BODY, FONT_LARGE, FONT_SMALL, RADIUS_SM};
use specular_theme::{Color, Palette};
use specular_widgets::accesskit;

use super::App;
use crate::loader::ImageId;

/// AccessKit ids: the specimen, its sliders, the grid's page buttons (the contents panel
/// uses 0x6100...).
const A11Y_ID: u64 = 0x6200_0000_0000_0000;
const A11Y_AXIS: u64 = A11Y_ID + 0x100;
const A11Y_PREV: u64 = A11Y_ID + 1;
const A11Y_NEXT: u64 = A11Y_ID + 2;

/// Glyphs on a page of the grid.
pub(super) const GRID_PAGE: usize = 256;
/// Sizes of the pangram lines (logical px).
const SIZES: [f32; 6] = [12.0, 16.0, 24.0, 36.0, 48.0, 72.0];
/// Side of a grid cell, and the glyph size in it (logical px).
const CELL: f32 = 60.0;
const CELL_GLYPH: f32 = 30.0;
/// The page's widest column (logical px).
const MAX_WIDTH: f32 = 1100.0;
/// Characters of a sample made from the face's own characters.
const SAMPLE_CHARS: usize = 40;
/// The colour row: glyphs at most, characters tried for a colour form, and the glyph size
/// (logical px).
const COLOUR_GLYPHS: usize = 16;
const COLOUR_TRIES: usize = 400;
const COLOUR_SIZE: f32 = 44.0;

/// A variable font's axis as the specimen shows it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct AxisInfo {
    pub tag: [u8; 4],
    pub name: String,
    pub min: f32,
    pub default: f32,
    pub max: f32,
}

/// What the specimen shows of a face, read once when it opens.
#[derive(Clone, Debug)]
pub(super) struct Specimen {
    pub family: String,
    /// "Bold Italic · Version 2.001".
    pub subtitle: String,
    /// Designer, licence and the like, one per line.
    pub credits: Vec<String>,
    /// The text of the size lines: the pangram, the face's own sample text, or a run of
    /// its characters when it lacks the pangram's.
    pub sample: String,
    pub axes: Vec<AxisInfo>,
    pub glyphs: u16,
    /// A character mapped to each glyph (0 for none), to label the grid.
    pub chars: Arc<Vec<u32>>,
    /// What the specimen cannot show, in words (colour layers it draws in one colour, a
    /// bitmap-only face).
    pub notes: Vec<String>,
    /// The face has colour glyphs the text stack draws in colour (COLR version 0, sbix,
    /// CBDT): the colour row shows some.
    pub colour: bool,
}

impl Specimen {
    pub(super) fn of(face: &Face) -> Specimen {
        let n = &face.names;
        let pick = |a: &str, b: &str| {
            let a = a.trim();
            if a.is_empty() { b.trim() } else { a }.to_owned()
        };
        let family = pick(&n.typographic_family, &n.family);
        let style = pick(&n.typographic_subfamily, &n.subfamily);
        let version = n.version.trim().to_owned();
        let subtitle = [style, version]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        let subtitle = specular_i18n::dot_join(&subtitle);
        let clip = |v: &str| {
            let v = v.trim();
            (!v.is_empty()).then(|| v.chars().take(300).collect::<String>())
        };
        let credits = [
            clip(&n.designer).map(|v| t!("font-designer", value = v)),
            clip(&n.manufacturer).map(|v| t!("font-foundry", value = v)),
            clip(&n.licence).map(|v| t!("font-licence", value = v)),
            clip(&n.licence_url).map(|v| t!("font-licence-url", value = v)),
        ]
        .into_iter()
        .flatten()
        .collect();
        let has = |s: &str| {
            s.chars()
                .filter(|c| !c.is_whitespace())
                .all(|c| face.glyph_index(c).is_some_and(|g| g != 0))
        };
        let pangram = t!("font-pangram");
        let own = n.sample_text.trim();
        let sample = if !own.is_empty() && has(own) {
            own.to_owned()
        } else if has(&pangram) {
            pangram
        } else {
            // The face's own printable characters.
            face.chars()
                .iter()
                .filter_map(|&(c, g)| char::from_u32(c).filter(|c| !c.is_control() && g != 0))
                .filter(|c| !c.is_whitespace())
                .take(SAMPLE_CHARS)
                .collect()
        };
        let axes = face
            .axes
            .iter()
            .filter(|a| !a.hidden && a.max > a.min)
            .map(|a| AxisInfo {
                tag: a.tag,
                name: if a.name.trim().is_empty() {
                    a.tag_str()
                } else {
                    a.name.trim().to_owned()
                },
                min: a.min,
                default: a.default.clamp(a.min, a.max),
                max: a.max,
            })
            .collect();
        let mut chars = vec![0u32; usize::from(face.glyph_count)];
        for &(c, g) in face.chars().iter().rev() {
            if let Some(slot) = chars.get_mut(usize::from(g)) {
                *slot = c;
            }
        }
        let mut notes = Vec::new();
        let c = &face.colour;
        let colour =
            c.colr.is_some_and(|(v, _)| v == 0) || !c.sbix.is_empty() || !c.cbdt.is_empty();
        if let Some((v, _)) = c.colr {
            notes.push(if v >= 1 {
                t!("font-note-colr1")
            } else {
                t!("font-note-colr0")
            });
        }
        if !c.sbix.is_empty() || !c.cbdt.is_empty() {
            notes.push(t!("font-note-bitmaps"));
        }
        if c.svg {
            notes.push(t!("font-note-svg"));
        }
        match face.outlines {
            OutlineKind::None => notes.push(t!("font-note-no-glyphs")),
            OutlineKind::BitmapOnly if c.sbix.is_empty() && c.cbdt.is_empty() => {
                notes.push(t!("font-note-bitmap-only"))
            }
            _ => {}
        }
        Specimen {
            family: if family.is_empty() {
                t!("font-unnamed")
            } else {
                family
            },
            subtitle,
            credits,
            sample,
            axes,
            glyphs: face.glyph_count,
            chars: Arc::new(chars),
            notes,
            colour,
        }
    }

    /// The axis settings for the text stack.
    fn settings(&self, values: &[f32]) -> Vec<([u8; 4], f32)> {
        self.axes
            .iter()
            .zip(values)
            .map(|(a, &v)| (a.tag, v))
            .collect()
    }

    /// Pages of the glyph grid.
    pub(super) fn pages(&self) -> usize {
        usize::from(self.glyphs).div_ceil(GRID_PAGE).max(1)
    }
}

/// The specimen's state for the current image.
#[derive(Default)]
pub(super) struct FontView {
    id: Option<ImageId>,
    spec: Option<Specimen>,
    face: Option<FontFace>,
    /// Each axis's value.
    values: Vec<f32>,
    /// Logical px scrolled from the top.
    scroll: f32,
    /// The grid's page.
    page: usize,
    /// The axis whose slider is being dragged.
    drag: Option<usize>,
    drawn: Option<Drawn>,
    /// The colour row, uploaded for a scale and backdrop, and the characters it is made
    /// from.
    strip: Option<Strip>,
    chars: Option<Vec<(u32, u16)>>,
    /// Textures to release on the next upload.
    retired: Vec<TexId>,
}

/// The colour row as a texture: its glyphs composited on the backdrop.
struct Strip {
    key: (u32, [u8; 4]),
    /// Physical size.
    size: (u32, u32),
    /// UI scale it was drawn at.
    scale: f32,
    tex: Option<(wgpu::Texture, TexId)>,
}

fn rgba8(c: Color) -> [u8; 4] {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    [q(c.r), q(c.g), q(c.b), q(c.a)]
}

/// The face's first colour glyphs (by character) at `ppem`, composited left to right on
/// `backdrop`: RGBA rows and their size; `None` when none has a colour form.
pub(super) fn colour_row(
    face: &FontFace,
    chars: &[(u32, u16)],
    ppem: f32,
    backdrop: [u8; 4],
) -> Option<(Vec<u8>, u32, u32)> {
    let mut glyphs = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for &(_, gid) in chars.iter().take(COLOUR_TRIES) {
        if glyphs.len() >= COLOUR_GLYPHS {
            break;
        }
        if gid == 0 || !seen.insert(gid) {
            continue;
        }
        if let Some(g) = face.colour_glyph(gid, ppem) {
            glyphs.push(g);
        }
    }
    if glyphs.is_empty() {
        return None;
    }
    let gap = (ppem * 0.25).ceil();
    let ascent = glyphs.iter().map(|g| g.top).max().unwrap_or(0).max(0) as f32;
    let descent = glyphs
        .iter()
        .map(|g| g.height as i32 - g.top)
        .max()
        .unwrap_or(0)
        .max(0) as f32;
    let h = (ascent + descent).ceil().max(1.0) as u32;
    let mut x = 0.0f32;
    let mut places = Vec::with_capacity(glyphs.len());
    for g in &glyphs {
        let w = g.advance.max(g.width as f32 + g.left.max(0) as f32);
        places.push(x);
        x += w + gap;
    }
    let w = (x - gap).ceil().clamp(1.0, 8192.0) as u32;
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for _ in 0..w * h {
        out.extend_from_slice(&backdrop);
    }
    for (g, &px) in glyphs.iter().zip(&places) {
        let ox = (px + g.left as f32).round() as i64;
        let oy = (ascent - g.top as f32).round() as i64;
        for row in 0..g.height as i64 {
            let ty = oy + row;
            if !(0..i64::from(h)).contains(&ty) {
                continue;
            }
            for col in 0..g.width as i64 {
                let tx = ox + col;
                if !(0..i64::from(w)).contains(&tx) {
                    continue;
                }
                let si = ((row * i64::from(g.width) + col) * 4) as usize;
                let di = ((ty * i64::from(w) + tx) * 4) as usize;
                let a = u32::from(g.rgba[si + 3]);
                for k in 0..3 {
                    let s = u32::from(g.rgba[si + k]);
                    let d = u32::from(out[di + k]);
                    out[di + k] = ((s * a + d * (255 - a) + 127) / 255) as u8;
                }
            }
        }
    }
    Some((out, w, h))
}

/// What the last frame drew, to hit-test.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Drawn {
    pub area: Rect,
    /// Each axis's slider track.
    pub tracks: Vec<Rect>,
    pub prev: Option<Rect>,
    pub next: Option<Rect>,
    /// Height of the whole page (logical px).
    pub content: f32,
}

impl FontView {
    /// The GPU device was replaced: the colour row is made again when next drawn.
    pub(super) fn gpu_reset(&mut self) {
        self.strip = None;
        self.retired.clear();
    }

    /// Shows `model`'s face for image `id` (keeps the state when it is already shown).
    pub(super) fn show(&mut self, id: ImageId, model: &FontModel) {
        if self.id == Some(id) {
            return;
        }
        let face = model.face();
        let spec = Specimen::of(face);
        self.values = spec.axes.iter().map(|a| a.default).collect();
        self.face = FontFace::new(face.sfnt(), face.index());
        self.chars = spec.colour.then(|| face.chars().to_vec());
        if let Some((_, id)) = self.strip.take().and_then(|s| s.tex) {
            self.retired.push(id);
        }
        self.spec = Some(spec);
        self.id = Some(id);
        self.scroll = 0.0;
        self.page = 0;
        self.drag = None;
        self.drawn = None;
    }

    pub(super) fn clear(&mut self) {
        let mut retired = std::mem::take(&mut self.retired);
        retired.extend(self.strip.take().and_then(|s| s.tex).map(|t| t.1));
        *self = FontView {
            retired,
            ..FontView::default()
        };
    }

    /// Releases retired textures and uploads the colour row for `scale` and `backdrop` when
    /// the face has one and it is not uploaded yet.
    pub(super) fn prepare(&mut self, gpu: &Gpu, ui: &mut UiRenderer, scale: f32, backdrop: Color) {
        for id in self.retired.drain(..) {
            ui.unregister_texture(id);
        }
        let (Some(spec), Some(face)) = (&self.spec, &self.face) else {
            return;
        };
        if !spec.colour {
            return;
        }
        let key = (scale.to_bits(), rgba8(backdrop));
        if self.strip.as_ref().is_some_and(|s| s.key == key) {
            return;
        }
        if let Some((_, id)) = self.strip.take().and_then(|s| s.tex) {
            ui.unregister_texture(id);
        }
        let row = self
            .chars
            .as_deref()
            .and_then(|chars| colour_row(face, chars, COLOUR_SIZE * scale, key.1));
        let (tex, size) = match row {
            Some((rgba, w, h)) => (Some(upload(gpu, ui, w, h, &rgba)), (w, h)),
            None => (None, (0, 0)),
        };
        self.strip = Some(Strip {
            key,
            size,
            scale,
            tex,
        });
    }

    pub(super) fn active(&self) -> bool {
        self.spec.is_some()
    }

    fn set_value(&mut self, axis: usize, track: Rect, x: f32, rtl: bool) -> bool {
        let Some(a) = self.spec.as_ref().and_then(|s| s.axes.get(axis)) else {
            return false;
        };
        let mut f = ((x - track.x) / track.w.max(1.0)).clamp(0.0, 1.0);
        if rtl {
            f = 1.0 - f;
        }
        let v = a.min + f * (a.max - a.min);
        // Whole units, as font tools show them.
        let v = v.round().clamp(a.min, a.max);
        let old = self.values.get(axis).copied();
        if let Some(slot) = self.values.get_mut(axis) {
            *slot = v;
        }
        old != Some(v)
    }
}

impl App {
    /// Shows the specimen of the current image when it is a font, else drops it.
    pub(super) fn font_sync(&mut self) {
        let cur = self.current_id();
        let model = cur
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.session.as_ref())
            .and_then(|s| s.font().cloned());
        match (cur, model) {
            (Some(id), Some(m)) => {
                if self.font_view.id != Some(id) {
                    self.font_view.show(id, &m);
                    self.redraw = true;
                }
            }
            _ => {
                if self.font_view.active() {
                    self.font_view.clear();
                    self.redraw = true;
                }
            }
        }
    }

    pub(super) fn font_active(&self) -> bool {
        self.font_view.active() && !self.gallery_active()
    }

    /// A press at logical `lp` over the specimen: a slider takes it (and drags), the grid's
    /// arrows turn its pages. True when taken.
    pub(super) fn font_press(&mut self, lp: (f32, f32)) -> bool {
        if !self.font_active() {
            return false;
        }
        let Some(d) = self.font_view.drawn.clone() else {
            return false;
        };
        if !d.area.contains(lp) {
            return false;
        }
        let rtl = self.rtl();
        if let Some(i) = d
            .tracks
            .iter()
            .position(|r| r.inset_xy(0.0, -6.0).contains(lp))
        {
            self.font_view.drag = Some(i);
            self.font_view.set_value(i, d.tracks[i], lp.0, rtl);
            self.redraw = true;
            return true;
        }
        let pages = self.font_view.spec.as_ref().map_or(1, Specimen::pages);
        if d.prev.is_some_and(|r| r.contains(lp)) {
            self.font_view.page = self.font_view.page.saturating_sub(1);
            self.redraw = true;
            return true;
        }
        if d.next.is_some_and(|r| r.contains(lp)) {
            self.font_view.page = (self.font_view.page + 1).min(pages - 1);
            self.redraw = true;
            return true;
        }
        false
    }

    /// The pointer moved to `p` (physical): drags a slider. True when one is dragged.
    pub(super) fn font_motion(&mut self, p: (f32, f32)) -> bool {
        let Some(i) = self.font_view.drag else {
            return false;
        };
        let lp = self.logical(p);
        let rtl = self.rtl();
        if let Some(track) = self
            .font_view
            .drawn
            .as_ref()
            .and_then(|d| d.tracks.get(i).copied())
            && self.font_view.set_value(i, track, lp.0, rtl)
        {
            self.redraw = true;
        }
        true
    }

    /// The button went up: ends a slider drag. True when one was dragged.
    pub(super) fn font_release(&mut self) -> bool {
        self.font_view.drag.take().is_some()
    }

    /// The wheel over the specimen scrolls it. True when taken.
    pub(super) fn font_wheel(&mut self, dy: f32, pixels: bool, lp: (f32, f32)) -> bool {
        if !self.font_active() {
            return false;
        }
        let Some(d) = &self.font_view.drawn else {
            return false;
        };
        if !d.area.contains(lp) {
            return false;
        }
        let step = if pixels { dy } else { dy * 48.0 };
        let max = (d.content - d.area.h).max(0.0);
        self.font_view.scroll = (self.font_view.scroll - step).clamp(0.0, max);
        self.redraw = true;
        true
    }

    /// A screen reader's action on a slider or a page button; true when the node is the
    /// specimen's.
    pub(super) fn font_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        let id = req.target_node.0;
        if !(A11Y_ID..A11Y_AXIS + 0x100).contains(&id) {
            return false;
        }
        let v = &mut self.font_view;
        let pages = v.spec.as_ref().map_or(1, Specimen::pages);
        match id {
            A11Y_PREV => v.page = v.page.saturating_sub(1),
            A11Y_NEXT => v.page = (v.page + 1).min(pages - 1),
            _ if id >= A11Y_AXIS => {
                let i = (id - A11Y_AXIS) as usize;
                if let Some(a) = v.spec.as_ref().and_then(|s| s.axes.get(i)).cloned()
                    && let Some(slot) = v.values.get_mut(i)
                {
                    let step = ((a.max - a.min) / 20.0).max(1.0);
                    *slot = match (req.action, &req.data) {
                        (accesskit::Action::Increment, _) => *slot + step,
                        (accesskit::Action::Decrement, _) => *slot - step,
                        (
                            accesskit::Action::SetValue,
                            Some(accesskit::ActionData::NumericValue(x)),
                        ) => *x as f32,
                        _ => *slot,
                    }
                    .round()
                    .clamp(a.min, a.max);
                }
            }
            _ => {}
        }
        self.redraw = true;
        true
    }
}

/// A slider: track, filled part, knob and value.
#[allow(clippy::too_many_arguments)]
fn slider(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    a: &AxisInfo,
    value: f32,
    x: f32,
    y: f32,
    w: f32,
    rtl: bool,
    ts: f32,
) -> Rect {
    let st = TextStyle::new(FONT_SMALL * ts);
    let label_w = 110.0 * ts;
    let value_w = 56.0 * ts;
    let name = text.layout(&a.name, st, Some(label_w - 8.0 * ts), Align::Start);
    let v = text.layout(
        &specular_i18n::fixed(value, 0),
        st.tabular(),
        None,
        Align::Start,
    );
    let track_w = (w - label_w - value_w).max(40.0);
    let row_h = 26.0 * ts;
    let cy = y + row_h / 2.0;
    let (lx, tx, vx) = if rtl {
        (x + w - name.width, x + value_w, x)
    } else {
        (x, x + label_w, x + label_w + track_w + 10.0 * ts)
    };
    let line_h = name.height / name.lines.max(1) as f32;
    text.draw(dl, &name, lx, cy - line_h / 2.0, pal.text_muted);
    let track = Rect::new(tx, cy - 2.0 * ts, track_w, 4.0 * ts);
    dl.rect(track, 2.0 * ts, pal.separator);
    let mut f = ((value - a.min) / (a.max - a.min)).clamp(0.0, 1.0);
    if rtl {
        f = 1.0 - f;
    }
    let kx = track.x + f * track.w;
    let filled = if rtl {
        Rect::new(kx, track.y, track.right() - kx, track.h)
    } else {
        Rect::new(track.x, track.y, kx - track.x, track.h)
    };
    dl.rect(filled, 2.0 * ts, pal.accent);
    dl.circle((kx, cy), 7.0 * ts, pal.accent);
    text.draw(dl, &v, vx, cy - v.height / 2.0, pal.text);
    track
}

/// A small arrow button for the grid's pages.
fn arrow(dl: &mut DrawList, pal: &Palette, r: Rect, left: bool, on: bool, ts: f32) {
    dl.rect(
        r,
        RADIUS_SM,
        if on { pal.surface_raised } else { pal.surface },
    );
    let (cx, cy) = r.center();
    let s = 4.5 * ts;
    let ink = if on { pal.text } else { pal.text_faint };
    let dx = if left { s * 0.5 } else { -s * 0.5 };
    dl.line((cx + dx, cy - s), (cx - dx, cy), 1.6 * ts, ink);
    dl.line((cx - dx, cy), (cx + dx, cy + s), 1.6 * ts, ink);
}

/// The block of one glyph centred in `cell`.
fn draw_cell(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    g: &TextBlock,
    label: &TextBlock,
    cell: Rect,
) {
    dl.rect(cell.inset(1.0), RADIUS_SM, pal.surface);
    let gx = cell.x + (cell.w - g.width) / 2.0;
    let gy = cell.y + (cell.h - label.height - g.height) / 2.0;
    text.draw(dl, g, gx, gy, pal.text);
    let lx = cell.x + (cell.w - label.width) / 2.0;
    text.draw(
        dl,
        label,
        lx,
        cell.bottom() - label.height - 2.0,
        pal.text_faint,
    );
}

/// The grid label of glyph `gid`: its character's code point, else its number.
fn glyph_label(chars: &[u32], gid: u16) -> String {
    match chars.get(usize::from(gid)).copied().unwrap_or(0) {
        0 => format!("#{gid}"),
        c => format!("{c:04X}"),
    }
}

/// Draws the specimen over `area` (logical x, y, w, h) on `backdrop`; records what it drew
/// for hit-testing.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    backdrop: Color,
    v: &mut FontView,
    area: (f32, f32, f32, f32),
    rtl: bool,
    ts: f32,
) {
    let (ax, ay, aw, ah) = area;
    let area_r = Rect::new(ax, ay, aw, ah);
    dl.rect(area_r, 0.0, backdrop);
    let (Some(spec), Some(face)) = (v.spec.as_ref(), v.face.as_ref()) else {
        v.drawn = None;
        return;
    };
    let settings = spec.settings(&v.values);
    let margin = (32.0 * ts).min(aw * 0.06);
    let w = (aw - 2.0 * margin).min(MAX_WIDTH * ts).max(80.0);
    let x0 = ax + (aw - w) / 2.0;
    let top = ay + margin - v.scroll;
    let mut y = top;
    let start = |bw: f32| if rtl { x0 + w - bw } else { x0 };
    dl.push_clip(area_r);
    // Names, in the UI font (the family may lack the letters of its own name).
    let title = text.layout(
        &spec.family,
        TextStyle::new(FONT_LARGE * 1.6 * ts).weight(600.0),
        Some(w),
        Align::Start,
    );
    text.draw(dl, &title, start(title.width), y, pal.text);
    y += title.height + 4.0 * ts;
    if !spec.subtitle.is_empty() {
        let b = text.layout(
            &spec.subtitle,
            TextStyle::new(FONT_BODY * ts),
            Some(w),
            Align::Start,
        );
        text.draw(dl, &b, start(b.width), y, pal.text_muted);
        y += b.height + 6.0 * ts;
    }
    for c in &spec.credits {
        let b = text.layout(c, TextStyle::new(FONT_SMALL * ts), Some(w), Align::Start);
        text.draw(dl, &b, start(b.width), y, pal.text_muted);
        y += b.height + 2.0 * ts;
    }
    y += 14.0 * ts;
    // The axes of a variable font.
    let mut tracks = Vec::with_capacity(spec.axes.len());
    for (i, a) in spec.axes.iter().enumerate() {
        let value = v.values.get(i).copied().unwrap_or(a.default);
        let sw = w.min(520.0 * ts);
        let sx = if rtl { x0 + w - sw } else { x0 };
        tracks.push(slider(dl, text, pal, a, value, sx, y, sw, rtl, ts));
        y += 28.0 * ts;
    }
    if !spec.axes.is_empty() {
        y += 10.0 * ts;
    }
    // What cannot be shown, in words.
    for n in &spec.notes {
        let b = text.layout(n, TextStyle::new(FONT_SMALL * ts), Some(w), Align::Start);
        text.draw(dl, &b, start(b.width), y, pal.warning);
        y += b.height + 2.0 * ts;
    }
    if !spec.notes.is_empty() {
        y += 10.0 * ts;
    }
    // The colour row, drawn in colour from its texture.
    if let Some(st) = &v.strip
        && let Some((_, tex)) = &st.tex
    {
        let k = st.scale.max(0.1);
        let (w_l, h_l) = (st.size.0 as f32 / k, st.size.1 as f32 / k);
        let shown = w_l.min(w);
        let uw = shown / w_l;
        let rx = if rtl { x0 + w - shown } else { x0 };
        dl.image(Rect::new(rx, y, shown, h_l), [0.0, 0.0, uw, 1.0], *tex, 0.0);
        y += h_l + 16.0 * ts;
    }
    // The sample at each size, in the face, left to right as the face shapes it.
    let label_st = TextStyle::new(FONT_SMALL * ts).tabular();
    let gutter = 40.0 * ts;
    for size in SIZES {
        let px = size * ts;
        let label = text.layout(&specular_i18n::fixed(size, 0), label_st, None, Align::Start);
        let block = text.layout_face(face, &spec.sample, px, &settings, Some(w - gutter));
        let bh = block.height.max(label.height);
        if y + bh >= ay && y <= ay + ah {
            let (lx, bx) = if rtl {
                (x0 + w - label.width, x0)
            } else {
                (x0, x0 + gutter)
            };
            text.draw(
                dl,
                &label,
                lx,
                y + block.baseline - label.baseline,
                pal.text_faint,
            );
            text.draw(dl, &block, bx, y, pal.text);
        }
        y += bh + 10.0 * ts;
    }
    y += 16.0 * ts;
    // The glyph grid, a page at a time.
    let pages = spec.pages();
    v.page = v.page.min(pages - 1);
    let first = v.page * GRID_PAGE;
    let last = (first + GRID_PAGE).min(usize::from(spec.glyphs));
    let head = if spec.glyphs == 0 {
        t!("font-glyphs-none")
    } else {
        t!(
            "font-glyphs-range",
            first = (first + 1) as i64,
            last = last as i64,
            count = i64::from(spec.glyphs)
        )
    };
    let hb = text.layout(
        &head,
        TextStyle::new(FONT_BODY * ts).weight(600.0),
        None,
        Align::Start,
    );
    let bs = 26.0 * ts;
    let (prev, next) = if pages > 1 {
        // Previous sits on the reading side's start.
        let a = Rect::new(x0 + w - 2.0 * bs - 6.0 * ts, y - 2.0 * ts, bs, bs);
        let b = Rect::new(x0 + w - bs, y - 2.0 * ts, bs, bs);
        let (p, n) = if rtl {
            (
                Rect::new(x0 + bs + 6.0 * ts, a.y, bs, bs),
                Rect::new(x0, a.y, bs, bs),
            )
        } else {
            (a, b)
        };
        arrow(dl, pal, p, !rtl, v.page > 0, ts);
        arrow(dl, pal, n, rtl, v.page + 1 < pages, ts);
        (Some(p), Some(n))
    } else {
        (None, None)
    };
    text.draw(
        dl,
        &hb,
        start(hb.width),
        y + (bs - hb.height) / 2.0 - 2.0 * ts,
        pal.text,
    );
    y += bs + 10.0 * ts;
    let cell = CELL * ts;
    let cols = ((w / cell).floor() as usize).max(1);
    let grid_x = x0 + (w - cols as f32 * cell) / 2.0;
    let lst = TextStyle::new(FONT_SMALL * 0.8 * ts).tabular();
    for (k, gid) in (first..last).enumerate() {
        let (row, col) = (k / cols, k % cols);
        let col = if rtl { cols - 1 - col } else { col };
        let r = Rect::new(
            grid_x + col as f32 * cell,
            y + row as f32 * cell,
            cell,
            cell,
        );
        if r.bottom() < ay || r.y > ay + ah {
            continue;
        }
        let gid = gid as u16;
        let g = text.layout_glyph(face, gid, CELL_GLYPH * ts, &settings);
        let label = text.layout(&glyph_label(&spec.chars, gid), lst, None, Align::Start);
        draw_cell(dl, text, pal, &g, &label, r);
    }
    let rows = (last - first).div_ceil(cols);
    y += rows as f32 * cell + margin;
    dl.pop_clip();
    let content = y - top;
    // A window made taller, or a face with fewer glyphs: the scroll stays in range.
    v.scroll = v.scroll.min((content - ah).max(0.0));
    v.drawn = Some(Drawn {
        area: area_r,
        tracks,
        prev,
        next,
        content,
    });
}

/// The AccessKit nodes: the specimen as a group with its sliders and page buttons.
pub(super) fn a11y(tree: &mut accesskit::TreeUpdate, v: &FontView, sf: f64) {
    use accesskit::{Node, NodeId, Role};
    let (Some(spec), Some(d)) = (&v.spec, &v.drawn) else {
        return;
    };
    let bounds = |r: Rect| accesskit::Rect {
        x0: f64::from(r.x) * sf,
        y0: f64::from(r.y) * sf,
        x1: f64::from(r.x + r.w) * sf,
        y1: f64::from(r.y + r.h) * sf,
    };
    let mut g = Node::new(Role::Group);
    g.set_label(t!("font-specimen", family = spec.family.clone()));
    g.set_bounds(bounds(d.area));
    let mut children = Vec::new();
    for (i, (a, r)) in spec.axes.iter().zip(&d.tracks).enumerate() {
        let id = NodeId(A11Y_AXIS + i as u64);
        let mut n = Node::new(Role::Slider);
        n.set_label(a.name.clone());
        n.set_numeric_value(f64::from(v.values.get(i).copied().unwrap_or(a.default)));
        n.set_min_numeric_value(f64::from(a.min));
        n.set_max_numeric_value(f64::from(a.max));
        n.set_bounds(bounds(*r));
        n.add_action(accesskit::Action::Increment);
        n.add_action(accesskit::Action::Decrement);
        n.add_action(accesskit::Action::SetValue);
        g.push_child(id);
        children.push((id, n));
    }
    for (id, r, label) in [
        (A11Y_PREV, d.prev, t!("font-glyphs-previous")),
        (A11Y_NEXT, d.next, t!("font-glyphs-next")),
    ] {
        let Some(r) = r else { continue };
        let mut n = Node::new(Role::Button);
        n.set_label(label);
        n.set_bounds(bounds(r));
        n.add_action(accesskit::Action::Click);
        g.push_child(NodeId(id));
        children.push((NodeId(id), n));
    }
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        root.push_child(NodeId(A11Y_ID));
    }
    tree.nodes.push((NodeId(A11Y_ID), g));
    tree.nodes.extend(children);
}

/// Uploads an opaque RGBA image into a new sRGB texture the UI can draw.
fn upload(gpu: &Gpu, ui: &mut UiRenderer, w: u32, h: u32, rgba: &[u8]) -> (wgpu::Texture, TexId) {
    let size = wgpu::Extent3d {
        width: w,
        height: h,
        depth_or_array_layers: 1,
    };
    let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("font colour glyphs"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_labels_name_the_character_or_the_glyph() {
        let chars = vec![0, 0x41, 0x1F600];
        assert_eq!(glyph_label(&chars, 0), "#0");
        assert_eq!(glyph_label(&chars, 1), "0041");
        assert_eq!(glyph_label(&chars, 2), "1F600");
        assert_eq!(glyph_label(&chars, 9), "#9");
    }
}
