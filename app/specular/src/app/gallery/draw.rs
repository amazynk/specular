//! Drawing the gallery: bars, tiles (placeholders, thumbnails by atlas page, captions and
//! badges), the details view, sticky group headers, the scrollbar, tooltips, the rubber band,
//! quick look, compare and the shared-element transition.
//!
//! Thumbnails are drawn grouped by atlas page after every placeholder, so a screen of tiles
//! costs one draw call per page; text and badges come after them.

use std::path::Path;

use specular_gfx::{Align, Backdrop, DrawList, Rect, TexId, Text, TextStyle, draw_icon};
use specular_theme::{Color, Palette};

use super::atlas::Placed;
use super::filterbar::Stop;
use super::*;

const RADIUS: f32 = 6.0;
const CAPTION: TextStyle = TextStyle::new(12.0);
const SMALL: TextStyle = TextStyle::new(11.0);
const BAR: TextStyle = TextStyle::new(13.0);
const HEADER: TextStyle = TextStyle::new(14.0).weight(600.0);
const BADGE: TextStyle = TextStyle::new(10.0).weight(700.0);
/// New captions shaped per frame while flinging.
const FAST_NEW_CAPTIONS: u32 = 8;
/// Time a frame may spend shaping new captions. A layout usually takes 0.1-0.5 ms, but
/// on a loaded machine one took up to 8 ms: eight of them made a 67 ms frame mid-drag.
/// Captions kept (then the cache starts over). The map is made at this size once: growing it
/// mid-scroll allocated ~0.5 MB of fresh pages in one frame, which took 18-90 ms when the
/// machine was short of memory.
pub(super) const CAPTIONS_KEPT: usize = 4096;
pub(super) const CAPTION_BUDGET: std::time::Duration = std::time::Duration::from_millis(2);
/// How long the scrollbar stays after scrolling (s).
const SCROLLBAR_S: f64 = 1.2;

/// What a frame of the gallery needs from the application.
pub(in crate::app) struct Frame<'a> {
    pub pal: &'a Palette,
    pub now: f64,
    pub reduce_motion: bool,
    /// Logical window size.
    pub size: (f32, f32),
    pub scale: f32,
    pub rtl: bool,
    /// The accessibility text scale (1 = 100 %): the facets panel's text and rows follow it.
    pub text_scale: f32,
    /// Under transparent thumbnails.
    pub backdrop: Backdrop,
}

/// Image draws of one frame, drawn by page; each with its opacity (fading in).
struct Images(Vec<(usize, Rect, [f32; 4], TexId, f32, f32)>);

impl Images {
    fn push(&mut self, p: &Placed, r: Rect, uv: [f32; 4], radius: f32) {
        self.push_faded(p, r, uv, radius, 1.0);
    }
    fn push_faded(&mut self, p: &Placed, r: Rect, uv: [f32; 4], radius: f32, a: f32) {
        self.0.push((p.page, r, uv, p.tex, radius, a));
    }
    fn flush(&mut self, dl: &mut DrawList, backdrop: Backdrop) {
        self.0.sort_by_key(|d| d.0);
        for (_, r, uv, tex, radius, a) in self.0.drain(..) {
            if a < 1.0 {
                dl.push_opacity(a);
                dl.image_over(r, uv, tex, radius, backdrop);
                dl.pop_opacity();
            } else {
                dl.image_over(r, uv, tex, radius, backdrop);
            }
        }
    }
}

/// `uv` cropped so the image fills `r` (centre crop).
fn crop_uv(p: &Placed, r: Rect) -> [f32; 4] {
    // `uv` is normalised x, y, w, h.
    let [u, v, uw, vh] = p.uv;
    let ia = p.width.max(1) as f32 / p.height.max(1) as f32;
    let ta = r.w.max(1.0) / r.h.max(1.0);
    if ia > ta {
        let nw = uw * ta / ia;
        [u + (uw - nw) * 0.5, v, nw, vh]
    } else {
        let nh = vh * ia / ta;
        [u, v + (vh - nh) * 0.5, uw, nh]
    }
}

/// The rect inside `r` that shows the whole image.
fn fit_rect(p: &Placed, r: Rect) -> Rect {
    fit(p.width as f32, p.height as f32, r)
}

/// The rect inside `r` that shows the whole image no larger than its own pixels (`scale`:
/// device pixels per logical pixel), so a small image keeps sharp on padding rather than
/// being enlarged into a blur.
fn fit_sharp(p: &Placed, r: Rect, scale: f32) -> Rect {
    fit_at_most(p, r, scale, (p.width, p.height))
}

/// The rect inside `r` that shows the whole image no larger than a picture of `size`
/// pixels (its display size, of which `p` may be a smaller thumbnail) at its own pixels.
fn fit_at_most(p: &Placed, r: Rect, scale: f32, size: (u32, u32)) -> Rect {
    let (w, h) = (p.width.max(1) as f32, p.height.max(1) as f32);
    let own = size.0.max(size.1).max(1) as f32 / w.max(h);
    let s = (r.w / w).min(r.h / h).min(own / scale.max(0.01));
    r.centered(w * s, h * s)
}

/// How much showing `p` in `r` enlarges its pixels (device px per thumbnail px): filling `r`
/// (centre crop) or fitting in it.
fn enlargement(p: &Placed, r: Rect, scale: f32, crop: bool) -> f32 {
    let (w, h) = (p.width.max(1) as f32, p.height.max(1) as f32);
    let (sx, sy) = (r.w * scale / w, r.h * scale / h);
    if crop { sx.max(sy) } else { sx.min(sy) }
}

/// Whether the thumbnail of class `class` holds every pixel there is to show: the picture
/// itself is no larger (`source`: its display size from the header, when known), or all the
/// file gave was a preview smaller than any the generator takes when it can decode (the
/// decode failed at this size). Such a thumbnail is never enlarged. A larger file's
/// thumbnail is a decode made for the class, shown like any other.
fn whole_picture(p: &Placed, source: Option<(u32, u32)>, class: u32) -> bool {
    let long = p.width.max(p.height);
    match source.filter(|s| s.0 > 0 && s.1 > 0) {
        Some((w, h)) => {
            let sl = w.max(h);
            long + 1 >= sl || (long as f32) < 0.75 * class.min(sl) as f32
        }
        None => long < class,
    }
}

/// Whether the tile shows `p` whole, at no more than its own pixels, on the tile's
/// background instead of filling it: a small picture (or the only, low-res preview of one)
/// that filling or fitting would enlarge, or a thin strip whose short side filling would
/// enlarge a lot. A photo's usual crop (its short side enlarged by its aspect ratio, 1.33
/// for 4:3, 1.78 for 16:9) stays a crop.
fn shown_small(p: &Placed, r: Rect, scale: f32, crop: bool, whole: bool) -> bool {
    let e = enlargement(p, r, scale, crop);
    whole && e > 1.01 || crop && e > 2.0
}

impl Gallery {
    /// The display size of the picture at `path` (see [`Gallery::source_size`]).
    fn source_size_at(&self, path: &Path) -> Option<(u32, u32)> {
        match self.by_path.get(path) {
            Some(&i) => self.source_size(&self.entries[i]),
            None => self
                .metas
                .get(path)
                .filter(|m| m.width > 0 && m.height > 0)
                .map(|m| (m.width, m.height)),
        }
    }

    /// `p`, the best thumbnail of `path` for `class`, filling `r` by a centre crop: its
    /// rect and uv. A small picture the crop would enlarge comes whole instead, at its own
    /// pixels, centred in `r` (the caller draws the background around it), and `true`.
    fn crop_or_whole(
        &self,
        atlas: &super::atlas::Atlas,
        path: &Path,
        p: &Placed,
        r: Rect,
        class: u32,
    ) -> (Rect, [f32; 4], bool) {
        let small = atlas.contains(path, class)
            && shown_small(
                p,
                r,
                self.scale,
                true,
                whole_picture(p, self.source_size_at(path), class),
            );
        if small {
            (fit_sharp(p, r, self.scale), p.uv, true)
        } else {
            (r, crop_uv(p, r), false)
        }
    }
}

fn fit(w: f32, h: f32, r: Rect) -> Rect {
    let s = (r.w / w.max(1.0)).min(r.h / h.max(1.0));
    r.centered(w * s, h * s)
}

/// The cells of a folder's cover mosaic in `r`: one image whole, two side by side, three as a
/// tall one beside two stacked, four in a 2×2 grid.
pub(super) fn mosaic_cells(r: Rect, n: usize, gap: f32) -> Vec<Rect> {
    let (hw, hh) = ((r.w - gap) / 2.0, (r.h - gap) / 2.0);
    let (x2, y2) = (r.x + hw + gap, r.y + hh + gap);
    match n {
        0 => Vec::new(),
        1 => vec![r],
        2 => vec![Rect::new(r.x, r.y, hw, r.h), Rect::new(x2, r.y, hw, r.h)],
        3 => vec![
            Rect::new(r.x, r.y, hw, r.h),
            Rect::new(x2, r.y, hw, hh),
            Rect::new(x2, y2, hw, hh),
        ],
        _ => vec![
            Rect::new(r.x, r.y, hw, hh),
            Rect::new(x2, r.y, hw, hh),
            Rect::new(r.x, y2, hw, hh),
            Rect::new(x2, y2, hw, hh),
        ],
    }
}

fn lerp_rect(a: Rect, b: Rect, t: f32) -> Rect {
    Rect::new(
        a.x + (b.x - a.x) * t,
        a.y + (b.y - a.y) * t,
        a.w + (b.w - a.w) * t,
        a.h + (b.h - a.h) * t,
    )
}

fn ease(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

pub(super) fn avg_color(a: [u8; 4]) -> Color {
    Color::rgba(
        f32::from(a[0]) / 255.0,
        f32::from(a[1]) / 255.0,
        f32::from(a[2]) / 255.0,
        1.0,
    )
}

const LABEL_COLORS: [u32; 4] = [0xE5484D, 0xF5D90A, 0x46A758, 0x3E63DD];

fn label_color(l: u8) -> Color {
    Color::hex(LABEL_COLORS[(l.clamp(1, 4) - 1) as usize])
}

/// A centred line of text in `r`.
pub(super) fn text_in(
    text: &mut Text,
    dl: &mut DrawList,
    s: &str,
    style: TextStyle,
    r: Rect,
    color: Color,
) -> f32 {
    let b = text.layout(s, style, None, Align::Start);
    let x = r.x + (r.w - b.width) * 0.5;
    let y = r.y + (r.h - b.height) * 0.5;
    text.draw(dl, &b, x, y, color);
    b.width
}

/// A line of text vertically centred in `r`, starting at its leading edge (trailing for RTL).
pub(super) fn text_at(
    text: &mut Text,
    dl: &mut DrawList,
    s: &str,
    style: TextStyle,
    r: Rect,
    color: Color,
    rtl: bool,
) -> f32 {
    let b = text.layout(s, style, None, Align::Start);
    let x = if rtl { r.right() - b.width } else { r.x };
    text.draw(dl, &b, x, r.y + (r.h - b.height) * 0.5, color);
    b.width
}

/// `s` cut at the end to fit `w` with an ellipsis (for prose, where the start matters most).
fn truncate_end(text: &mut Text, s: &str, style: TextStyle, w: f32) -> String {
    if text.measure(s, style) <= w {
        return s.to_owned();
    }
    let chars: Vec<char> = s.chars().collect();
    let mut n = chars.len();
    while n > 0 {
        n -= 1;
        let c: String = chars[..n].iter().collect::<String>().trim_end().to_owned() + "…";
        if text.measure(&c, style) <= w {
            return c;
        }
    }
    "…".to_owned()
}

/// `s` cut at its end to fit `w` with an ellipsis, its bidi isolates kept balanced (a line
/// joined right to left by `I18n::join`).
fn truncate_isolated(text: &mut Text, s: &str, style: TextStyle, w: f32) -> String {
    if text.measure(s, style) <= w {
        return s.to_owned();
    }
    let mut n = specular_i18n::strip_isolates(s).chars().count();
    while n > 1 {
        n -= 1;
        let c = specular_i18n::truncate(s, n);
        if text.measure(&c, style) <= w {
            return c;
        }
    }
    "…".to_owned()
}

/// `s` cut to fit `w` with an ellipsis.
pub(in crate::app) fn truncate(text: &mut Text, s: &str, style: TextStyle, w: f32) -> String {
    if text.measure(s, style) <= w {
        return s.to_owned();
    }
    let chars: Vec<char> = s.chars().collect();
    // Keep the extension visible: "IMG_48…62.HEIC" reads better than "IMG_4862.H…".
    let (lo_keep, mut lo, mut hi) = (6.min(chars.len()), 0usize, chars.len());
    let make = |n: usize| -> String {
        if n >= chars.len() {
            return s.to_owned();
        }
        let tail = lo_keep.min(n / 2);
        let head = n - tail;
        let h: String = chars[..head].iter().collect();
        let t: String = chars[chars.len() - tail..].iter().collect();
        format!("{h}…{t}")
    };
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if text.measure(&make(mid), style) <= w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    make(lo)
}

/// `s` on up to two lines that each fit `w`, as Finder writes a name under its icon: as much
/// as fits on the first (broken after a space, `-` or `_` in its second half, so a word stays
/// whole), the rest on the second, cut in the middle if it is still too long (the extension
/// stays). The lines are joined by `\n`.
fn two_lines(text: &mut Text, s: &str, style: TextStyle, w: f32) -> String {
    if text.measure(s, style) <= w {
        return s.to_owned();
    }
    let chars: Vec<char> = s.chars().collect();
    let fits = |text: &mut Text, n: usize| {
        let head: String = chars[..n].iter().collect();
        text.measure(&head, style) <= w
    };
    // The longest start that fits (at least one character).
    let (mut lo, mut hi) = (1usize, chars.len() - 1);
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if fits(text, mid) {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let n = (lo / 2 + 1..=lo)
        .rev()
        .find(|&i| matches!(chars[i - 1], ' ' | '-' | '_'))
        .unwrap_or(lo);
    let first: String = chars[..n].iter().collect();
    let rest: String = chars[n..].iter().collect();
    let second = truncate(text, rest.trim_start(), style, w);
    format!("{}\n{second}", first.trim_end())
}

fn stars(n: u8) -> String {
    "★".repeat(n as usize)
}

impl Gallery {
    /// Draws the gallery (and its transitions). Returns when to wake for the next change
    /// (seconds from now) and whether it is animating (draw again at once).
    pub(in crate::app) fn draw(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        f: &Frame,
    ) -> (Option<f64>, bool) {
        use super::scrollbench::{D_BARS, D_FACETS, D_LAYOUT, D_PRE, D_REST, D_SCROLL};
        let mut lap = std::time::Instant::now();
        let now = f.now;
        self.backdrop = f.backdrop;
        let dt = (now - self.last_frame).clamp(0.0, 0.1) as f32;
        self.last_frame = now;
        self.hits.clear();
        self.bar_tip_now = None;
        self.cap_new = 0;
        self.cap_start = None;
        self.cap_more = false;
        let (w, h) = f.size;
        if (w - self.vw).abs() > 0.5
            || (h - self.vh).abs() > 0.5
            || f.scale != self.scale
            || f.rtl != self.rtl
        {
            self.vw = w;
            self.vh = h;
            self.scale = f.scale;
            self.rtl = f.rtl;
            self.relayout = true;
        }
        self.set_text_scale(f.text_scale);
        // The facets panel docks at the trailing edge and narrows the grid (its scrollbar stays reachable).
        let dock = if self.keywords.open || self.meta_edit.open {
            crate::app::side_panel::width(self.vw, f.text_scale)
        } else if self.filter_open && self.facets.open || self.library_sidebar_shown() {
            self.facets_width(f.text_scale.clamp(0.75, 3.0))
        } else {
            0.0
        };
        if (dock - self.facets.dock).abs() > 0.5 {
            self.facets.dock = dock;
            self.relayout = true;
        }
        // The tile size follows the slider / wheel smoothly, the tile under the pointer staying.
        let mut resizing = false;
        let ds = self.view.size - self.size_shown;
        if ds.abs() > 0.5
            && !f.reduce_motion
            && self
                .stats
                .entered
                .is_some_and(|t| t.elapsed().as_secs_f64() > 0.3)
        {
            self.size_shown += ds * (1.0 - (-dt * 16.0).exp());
            self.relayout = true;
            resizing = true;
        } else if ds != 0.0 {
            self.size_shown = self.view.size;
            self.relayout = true;
        }
        self.lap(D_PRE, &mut lap);
        if self.relayout {
            self.relayout = false;
            let a = self.anchor_path();
            self.relayout_now(a, false);
        }
        self.lap(D_LAYOUT, &mut lap);
        let mut animating = resizing;
        let mut wake: Option<f64> = None;
        let mut soon = |t: f64| wake = Some(wake.map_or(t, |w: f64| w.min(t)).max(0.0));
        // A pending automatic similar-images scan: a presented frame resets the app's wake.
        if let Some(t) = self.similar.auto_at {
            soon(
                t.saturating_duration_since(std::time::Instant::now())
                    .as_secs_f64(),
            );
        }

        // Transition progress.
        let mut alpha = 1.0;
        let mut flight = None;
        if let Some(tr) = self.transition.clone() {
            let t = ((now - tr.start) / tr.duration.max(1e-3)).clamp(0.0, 1.0) as f32;
            let e = ease(t);
            alpha = if tr.opening { 1.0 - e } else { e };
            if t >= 1.0 {
                self.transition = None;
            } else {
                animating = true;
                if !tr.fade_only {
                    flight = Some((tr, e));
                }
            }
        }
        if !self.active && self.transition.is_none() && flight.is_none() {
            return (None, false);
        }

        // Smooth scrolling (wheel steps, keyboard) toward the target.
        let gh = self.grid_h();
        self.scroll_target = self.layout.clamp_scroll(self.scroll_target, gh);
        let before = self.scroll;
        let diff = self.scroll_target - self.scroll;
        if diff.abs() > 0.5 && !f.reduce_motion {
            self.scroll += diff * (1.0 - (-dt * 18.0).exp());
            animating = true;
        } else {
            self.scroll = self.scroll_target;
        }
        if (self.scroll - before).abs() > 0.01 {
            self.scroll_dir = (self.scroll - before).signum();
            self.scrolled_at = now;
        }

        let pal = f.pal;
        dl.push_opacity(alpha);
        dl.rect(Rect::new(0.0, 0.0, w, h), 0.0, pal.canvas);
        let top = self.grid_top();
        self.lap(D_SCROLL, &mut lap);
        dl.push_clip(Rect::new(0.0, top, w, gh));
        self.draw_grid(dl, text, f, &mut lap);
        if !self.fade_in.is_empty() {
            soon(0.0);
        }
        // Where the grid meets a bar with more of it behind: a soft fade into the bar (a
        // shadow of the bar in the page's colour), not a hard cut through the tiles. Under the
        // sticky headers, which cover it.
        let below = self.layout.clamp_scroll(self.scroll + 24.0, gh) - self.scroll;
        let (under_top, under_bottom) = (
            (self.scroll / 24.0).clamp(0.0, 1.0),
            (below / 24.0).clamp(0.0, 1.0),
        );
        for (y, a) in [(top - 24.0, under_top), (top + gh, under_bottom)] {
            if a > 0.0 && self.view.mode != Mode::Details {
                let c = pal.canvas.with_alpha(pal.canvas.a * a);
                dl.shadow(Rect::new(-24.0, y, w + 48.0, 24.0), 0.0, c, 5.0, 0.0);
            }
        }
        self.draw_headers(dl, text, f);
        self.draw_band(dl, pal);
        dl.pop_clip();
        self.draw_scrollbar(dl, text, f, &mut soon);
        self.draw_top_bar(dl, text, f);
        if self.filter_open {
            self.draw_filter_bar(dl, text, f);
        }
        self.draw_timeline(dl, text, f);
        self.draw_local_banner(dl, text, f);
        if self.view.mode == Mode::Details {
            self.draw_details_header(dl, text, f);
        }
        self.draw_bottom_bar(dl, text, f);
        if self.draw_sel_bar(dl, text, f) {
            soon(0.0);
        }
        self.draw_empty(dl, text, f, &mut soon);
        self.lap(D_BARS, &mut lap);
        if let Some(t) = self.facets_step(now) {
            soon(t);
        }
        if let Some(t) = self.bar_step(now) {
            soon(t);
        }
        self.draw_facets(dl, text, f);
        self.draw_keywords(dl, text, f);
        self.draw_meta_edit(dl, text, f);
        // The walking bar's stripe moves at ~30 fps.
        if self.draw_library(dl, text, f) {
            soon(0.033);
        }
        self.draw_picker(dl, text, f);
        if self.draw_info(dl, text, f) {
            soon(0.0);
        }
        self.lap(D_FACETS, &mut lap);
        self.draw_tooltip(dl, text, f, &mut soon);
        self.draw_bar_tip(dl, text, f, &mut soon);
        self.draw_drag(dl, text, f);
        if self.compare.is_some() {
            self.draw_compare(dl, text, f);
        } else if self.quick.is_some() {
            self.draw_quick(dl, text, f);
        }
        if self.draw_similar(dl, text, f) {
            soon(0.0);
        }
        if self.draw_map(dl, text, f) {
            soon(0.25);
        }
        if self.draw_people(dl, text, f) {
            soon(0.25);
        }
        if self.draw_import(dl, text, f) {
            soon(0.25);
        }
        dl.pop_opacity();
        if let Some((tr, e)) = flight {
            self.draw_flight(dl, &tr, e);
        }
        if self.listing {
            soon(0.25);
        }
        if let Some(t) = self.library_wake() {
            soon(t);
        }
        // Folder changes are taken once they settle: wake then, as cull does.
        if let Some(t) = self.watch.as_ref().and_then(|w| w.due()) {
            soon(
                t.saturating_duration_since(std::time::Instant::now())
                    .as_secs_f64(),
            );
        }
        // Thumbnails refused for memory: look again whether they fit.
        if self
            .failed
            .values()
            .any(|e| model::needs_memory(e).is_some())
        {
            soon((self.memory_retry_at - now).max(0.0));
        }
        // Uploads the budget left for later; full quality once a fling has come to rest.
        if self.upload_more || std::mem::take(&mut self.engine_retry) {
            soon(0.0);
        }
        if self.wish_fast {
            soon(0.05);
        }
        if self.cap_more {
            soon(0.0);
        }
        // Files from another application over the window: follow the pointer (X11 polls it).
        if !self.drop_hover.is_empty() {
            soon(0.05);
        }
        self.note_fill();
        animating |= self.bench_step(dl.batches.len());
        animating |= self.bench_busy();
        self.lap(D_REST, &mut lap);
        if let Some(b) = self.sbench.as_mut() {
            b.prof.caps += self.cap_new;
        }
        (wake, animating)
    }

    /// The Properties card beside its tile: a header (thumbnail, name, what it is) over
    /// label and value rows, on a raised surface that floats over the grid. True while it is
    /// still fading in.
    fn draw_info(&mut self, dl: &mut DrawList, text: &mut Text, f: &Frame) -> bool {
        if let Some(c) = self.info.as_mut()
            && c.opened.is_nan()
        {
            c.opened = f.now;
        }
        let Some(card) = self.info.as_ref() else {
            return false;
        };
        let pal = f.pal;
        let (pad, gap, thumb) = (16.0, 12.0, self.u(52.0));
        let w = self.u(320.0).min(self.vw - 24.0);
        let title_style = self.ts(TextStyle::new(14.0).weight(600.0));
        let sub_style = self.ts(TextStyle::new(12.0));
        let label_style = self.ts(TextStyle::new(12.0));
        let value_style = self.ts(TextStyle::new(12.0).weight(500.0));
        let close_s = 24.0;
        // Header: the thumbnail, then the name (two lines at most) and what it is.
        let head_w = (w - pad * 2.0 - thumb - gap - close_s - 4.0).max(40.0);
        let title: Vec<_> = two_lines(text, &card.title, title_style, head_w)
            .split('\n')
            .map(|l| text.layout(l, title_style, None, Align::Start))
            .collect();
        let title_h: f32 = title.iter().map(|b| b.height).sum();
        let sb = text.layout(&card.subtitle, sub_style, Some(head_w), Align::Start);
        let head_h = (title_h + 2.0 + sb.height).max(thumb);
        // Rows: labels in a column as wide as the widest (a third of the card at most),
        // values beside them, wrapped.
        let label_w = card
            .rows
            .iter()
            .map(|(k, _)| text.measure(k, label_style))
            .fold(0.0, f32::max)
            .min(w * 0.34);
        let value_w = (w - pad * 2.0 - label_w - gap).max(40.0);
        let rows: Vec<_> = card
            .rows
            .iter()
            .map(|(k, v)| {
                let k = truncate(text, k, label_style, label_w);
                let vb = text.layout(v, value_style, Some(value_w), Align::Start);
                (k, vb)
            })
            .collect();
        let row_gap = 6.0;
        let rows_h: f32 = rows.iter().map(|(_, vb)| vb.height + row_gap).sum();
        let body_h = if rows.is_empty() {
            0.0
        } else {
            gap * 2.0 + rows_h - row_gap
        };
        let h = pad + head_h + body_h + pad;

        // Beside the tile on the end side, else the start side, else over it; level with its
        // top, kept between the bars.
        let tile = card
            .path
            .as_deref()
            .and_then(|p| self.tile_of(p))
            .or(card.tile)
            .and_then(|i| self.tile_rect(i))
            .filter(|r| r.bottom() > self.grid_top() && r.y < self.grid_top() + self.grid_h());
        let (top, bottom) = (self.grid_top() + 8.0, self.vh - BOTTOM_BAR - 8.0);
        let (x, y) = match tile {
            Some(t) => {
                let end = if self.rtl {
                    t.x - 10.0 - w
                } else {
                    t.right() + 10.0
                };
                let start = if self.rtl {
                    t.right() + 10.0
                } else {
                    t.x - 10.0 - w
                };
                let fits = |x: f32| x >= 12.0 && x + w <= self.vw - 12.0;
                let x = if fits(end) {
                    end
                } else if fits(start) {
                    start
                } else {
                    t.x + (t.w - w) * 0.5
                };
                (x, t.y)
            }
            None => ((self.vw - w) * 0.5, top + (bottom - top - h) * 0.3),
        };
        let x = x.clamp(12.0, (self.vw - w - 12.0).max(12.0));
        let y = y.min(bottom - h).max(top);
        // Fades in, settling a few pixels from the tile's side.
        let t = (((f.now - card.opened) / 0.15) as f32).clamp(0.0, 1.0);
        let e = 1.0 - (1.0 - t).powi(3);
        let toward = tile.map_or(0.0, |r| if x > r.x { -1.0 } else { 1.0 });
        let slide = if f.reduce_motion {
            0.0
        } else {
            6.0 * (1.0 - e) * toward
        };
        let r = Rect::new((x + slide).round(), y.round(), w, h);
        dl.push_opacity(e);
        dl.shadow(r, 12.0, pal.shadow.fade(1.1), 28.0, 12.0);
        dl.shadow(r, 12.0, pal.shadow.fade(0.6), 4.0, 1.0);
        dl.rect_border(r, 12.0, pal.surface_raised, pal.separator, 1.0);
        self.hits.push((r, Hit::InfoCard));

        // Header.
        let tr = Rect::new(r.x + pad, r.y + pad, thumb, thumb);
        let tr = self.flip_in(tr, r);
        let card_class = specular_thumbs::class_for(thumb * self.scale);
        let mut atlas = self.atlas.take();
        let placed = card
            .path
            .as_deref()
            .zip(atlas.as_mut())
            .and_then(|(p, atlas)| {
                let pl = atlas.best(p, card_class)?;
                let c = self.crop_or_whole(atlas, p, &pl, tr, card_class);
                Some((pl, c))
            });
        self.atlas = atlas;
        match (card.kind, placed) {
            (Some(EntryKind::Image), Some((p, (ir, uv, small)))) => {
                if small {
                    dl.rect(tr, 8.0, pal.surface);
                }
                let radius = if small { 3.0 } else { 8.0 };
                dl.image_over(ir, uv, p.tex, radius, self.backdrop);
                dl.stroke_rect(tr.inset(0.5), 7.5, pal.text.with_alpha(0.08), 1.0);
            }
            (kind, _) => {
                dl.rect(tr, 8.0, pal.surface);
                let icon = match kind {
                    Some(EntryKind::Folder) => Icon::Folder,
                    Some(EntryKind::Image) => Icon::Image,
                    None => Icon::Grid,
                    _ => Icon::Image,
                };
                let c = if kind == Some(EntryKind::Folder) {
                    pal.accent
                } else {
                    pal.text_muted
                };
                draw_icon(
                    dl,
                    icon,
                    tr.centered(thumb * 0.5, thumb * 0.5),
                    c,
                    1.6,
                    false,
                );
            }
        }
        let hx = r.x + pad + thumb + gap;
        let mut hy = r.y + pad + (head_h - title_h - 2.0 - sb.height) * 0.5;
        for b in &title {
            let lr = self.flip_in(Rect::new(hx, hy, head_w, b.height), r);
            let lx = if self.rtl { lr.right() - b.width } else { lr.x };
            text.draw(dl, b, lx, lr.y, pal.text);
            hy += b.height;
        }
        let sub_r = self.flip_in(Rect::new(hx, hy + 2.0, head_w, sb.height), r);
        let sx = if self.rtl {
            sub_r.right() - sb.width
        } else {
            sub_r.x
        };
        text.draw(dl, &sb, sx, sub_r.y, pal.text_muted);
        let close = self.flip_in(
            Rect::new(
                r.right() - pad - close_s + 6.0,
                r.y + pad - 6.0,
                close_s,
                close_s,
            ),
            r,
        );
        let hovered = self.pointer.is_some_and(|p| close.contains(p));
        if hovered {
            dl.rect(close, 6.0, pal.surface_hover);
        }
        draw_icon(
            dl,
            Icon::Close,
            close.centered(14.0, 14.0),
            pal.text_muted,
            1.5,
            false,
        );
        self.hits.push((close, Hit::InfoClose));

        // Rows, under a hairline.
        if !rows.is_empty() {
            let ly = (r.y + pad + head_h + gap).round() + 0.5;
            dl.line((r.x + pad, ly), (r.right() - pad, ly), 1.0, pal.separator);
            let mut y = r.y + pad + head_h + gap * 2.0;
            for (k, vb) in &rows {
                // Level with the value's first line.
                let first = vb.height / vb.lines.max(1) as f32;
                let lr = Rect::new(r.x + pad, y, label_w, first);
                let vr = Rect::new(r.x + pad + label_w + gap, y, value_w, vb.height);
                let (lr, vr) = (self.flip_in(lr, r), self.flip_in(vr, r));
                // Labels end-aligned against their values, as a form reads.
                let lw = text.measure(k, label_style);
                let lx = if self.rtl { lr.x } else { lr.right() - lw };
                let kr = Rect::new(lx, lr.y, lw, lr.h);
                text_at(text, dl, k, label_style, kr, pal.text_muted, self.rtl);
                let vx = if self.rtl {
                    vr.right() - vb.width
                } else {
                    vr.x
                };
                text.draw(dl, vb, vx, vr.y, pal.text);
                y += vb.height + row_gap;
            }
        }
        dl.pop_opacity();
        if let Some(c) = self.info.as_mut() {
            c.rect = Some(r);
            c.close = Some(close);
        }
        t < 1.0
    }

    /// The selection bar: with two or more items selected, a pill over the foot of the grid
    /// with what is done to all of them at once (clear, the count, compare, Zynk, trash, and
    /// More for the rest), as photo libraries show it. True while it is still rising in.
    fn draw_sel_bar(&mut self, dl: &mut DrawList, text: &mut Text, f: &Frame) -> bool {
        use super::code;
        let n = self.selected.len();
        let covered = self.compare.is_some()
            || self.quick.is_some()
            || self.info.is_some()
            || self.similar.is_open()
            || self.map.open
            || self.people.open
            || self.import.open;
        if n < 2 || covered {
            self.sel_bar = None;
            self.layout.tail = 0.0;
            return false;
        }
        // The grid scrolls far enough that its last row clears the bar.
        self.layout.tail = SEL_BAR_ROOM;
        let since = self.sel_bar.as_ref().map_or(f.now, |b| b.since);
        let pal = f.pal;
        let images = self
            .selected
            .iter()
            .filter(|p| {
                self.by_path
                    .get(p.as_path())
                    .is_some_and(|&i| self.entries[i].kind == EntryKind::Image)
            })
            .count();
        // What it offers, after the clear button and the count.
        let mut offers: Vec<(Hit, Icon, String)> = Vec::new();
        if (2..=4).contains(&images) {
            let name = t!("gallery-palette-compare");
            offers.push((Hit::SelBar(code::COMPARE), Icon::Grid, name));
        }
        // Zynk too when it isn't installed: they then say what it is and where to get it.
        let name = t!("palette-share-with-zynk");
        offers.push((Hit::SelBar(code::ZYNK_SHARE), Icon::Zynk, name));
        let name = t!("palette-send-with-zynk");
        offers.push((Hit::SelBar(code::ZYNK_SEND), Icon::Zynk, name));
        let trash = t!("gallery-trash-count", count = n as i64);
        offers.push((Hit::SelBar(code::TRASH), Icon::Trash, trash));
        offers.push((Hit::SelMore, Icon::More, t!("gallery-more-menu")));

        let (pad, button, h) = (4.0, 32.0, SEL_BAR);
        let count = t!(
            "gallery-selected",
            count = n as i64,
            total = self.tiles.len() as i64
        );
        let count_style = BAR.weight(600.0);
        let count_w = text.measure(&count, count_style);
        // Clear, the count, a divider, then the offers 2 px apart.
        let offers_w = offers.len() as f32 * (button + 2.0) - 2.0;
        let w = pad + button + 4.0 + count_w + 12.0 + 1.0 + 8.0 + offers_w + pad;
        // Centred on the grid, not the window: beside the sidebar or facets panel when docked.
        let gw = self.grid_w();
        let gx = if self.rtl { self.vw - gw } else { 0.0 };
        let w = w.min(gw - 24.0);
        // Rises a few pixels into place as it fades in (or only fades, with reduced motion).
        let t = (((f.now - since) / 0.18) as f32).clamp(0.0, 1.0);
        let e = 1.0 - (1.0 - t).powi(3);
        let rise = if f.reduce_motion {
            0.0
        } else {
            8.0 * (1.0 - e)
        };
        let y = self.vh - BOTTOM_BAR - 12.0 - h + rise;
        let r = Rect::new((gx + (gw - w) * 0.5).round(), y.round(), w, h);
        dl.push_opacity(e);
        dl.shadow(r, h * 0.5, pal.shadow.fade(1.0), 20.0, 8.0);
        dl.shadow(r, h * 0.5, pal.shadow.fade(0.5), 3.0, 1.0);
        dl.rect_border(r, h * 0.5, pal.surface_raised, pal.separator, 1.0);
        self.hits.push((r, Hit::SelBarBg));

        let mut buttons: Vec<(Rect, Hit, String)> = Vec::new();
        let by = r.y + (h - button) * 0.5;
        let mut x = r.x + pad;
        let clear = (
            Hit::SelBar(code::SELECT_NONE),
            Icon::Close,
            t!("gallery-select-none"),
        );
        let place = |x: &mut f32, wd: f32| {
            let br = Rect::new(*x, by, wd, button);
            *x += wd;
            br
        };
        let clear_r = place(&mut x, button);
        x += 4.0;
        let count_r = place(&mut x, count_w);
        x += 12.0;
        let sep_x = x;
        x += 1.0 + 8.0;
        let mut rects = vec![(clear_r, clear)];
        for (k, o) in offers.into_iter().enumerate() {
            if k > 0 {
                x += 2.0;
            }
            rects.push((place(&mut x, button), o));
        }
        for (br, (hit, icon, name)) in rects {
            let br = self.flip_in(br, r);
            let hovered = self.pointer.is_some_and(|p| br.contains(p));
            if hovered {
                dl.rect(br, button * 0.5, pal.surface_hover);
                self.bar_tip_now = Some((br, name.clone()));
            }
            // Trash shows what it would do when the pointer is on it.
            let c = match icon {
                Icon::Trash if hovered => pal.danger,
                Icon::Close => pal.text_muted,
                _ => pal.text,
            };
            draw_icon(dl, icon, br.centered(18.0, 18.0), c, 1.5, false);
            self.hits.push((br, hit.clone()));
            buttons.push((br, hit, name));
        }
        let count_r = self.flip_in(count_r, r);
        text_at(text, dl, &count, count_style, count_r, pal.text, self.rtl);
        let sx = self.flip_in(Rect::new(sep_x, r.y + 10.0, 1.0, h - 20.0), r);
        dl.rect(sx, 0.0, pal.separator);
        dl.pop_opacity();
        self.sel_bar = Some(SelBar {
            rect: r,
            buttons,
            since,
        });
        t < 1.0
    }

    /// `r` mirrored inside `within` for right-to-left layouts.
    fn flip_in(&self, r: Rect, within: Rect) -> Rect {
        if self.rtl {
            Rect::new(within.x + within.right() - r.right(), r.y, r.w, r.h)
        } else {
            r
        }
    }

    /// How far `path`'s thumbnail has faded in (1 when it was already there).
    fn fade_of(&self, path: &Path, now: f64) -> f32 {
        self.fade_in.get(path).map_or(1.0, |&t| {
            let t = ((now - t) / THUMB_FADE_S).clamp(0.0, 1.0) as f32;
            1.0 - (1.0 - t) * (1.0 - t)
        })
    }

    /// The colour a tile shows before its thumbnail: the image's average, else the surface.
    fn placeholder(&self, path: &Path, pal: &Palette) -> Color {
        self.averages
            .get(path)
            .map_or(pal.surface, |a| avg_color(*a))
    }

    /// Placeholders, then thumbnails by page, then captions, badges and rings.
    fn draw_grid(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        f: &Frame,
        lap: &mut std::time::Instant,
    ) {
        use super::scrollbench::{D_FLUSH, D_OVER, D_TILES};
        let pal = f.pal;
        let gh = self.grid_h();
        let range = self.layout.items_in(self.scroll, self.scroll + gh);
        let class = self.class();
        let (cover_small, cover_big) = (self.cover_class(4), self.cover_class(1));
        let details = self.view.mode == Mode::Details;
        let uncropped = self.view.mode == Mode::Uncropped;
        let mut images = Images(Vec::with_capacity(range.len() + 8));
        if f.reduce_motion {
            self.fade_in.clear();
        } else {
            self.fade_in.retain(|_, t| f.now - *t < THUMB_FADE_S);
        }
        let Some(mut atlas) = self.atlas.take() else {
            // Before the first upload: placeholders only.
            for i in range {
                if let Some(r) = self.tile_rect(i) {
                    dl.rect(r, RADIUS, pal.surface);
                }
            }
            return;
        };
        let hover = self.hover.map(|h| h.0);
        // The tile under a press not yet a drag or a band sinks a little: the click is felt
        // on the way down, before anything opens.
        let pressed = self
            .press
            .as_ref()
            .filter(|p| !p.moved && !p.band && !p.drag)
            .and_then(|p| p.tile)
            .filter(|_| !details);
        let focus = self.focus_index();
        // The focus ring only while the grid has the keyboard; with it in the filter bar or
        // on the indicator the focused tile looks like any other (selected or not), so only
        // one place shows a ring.
        let grid_kbd = self.grid_has_keyboard();
        // Pass 1: backgrounds and placeholders; collect the thumbnails.
        let mut overlays: Vec<(usize, Rect, Option<Rect>)> = Vec::with_capacity(range.len());
        for i in range.clone() {
            let Some(r) = self.tile_rect(i) else { continue };
            let r = if pressed == Some(i) {
                r.centered(r.w * 0.97, r.h * 0.97)
            } else {
                r
            };
            let e = &self.entries[self.tiles[i].entry];
            let selected = self.selected.contains(&e.path);
            if details {
                let row = self.flip(Rect::new(0.0, r.y, self.grid_w(), r.h));
                if selected {
                    dl.rect(row.inset_xy(6.0, 1.0), 4.0, pal.accent_soft);
                } else if hover == Some(i) {
                    dl.rect(row.inset_xy(6.0, 1.0), 4.0, pal.surface_hover);
                } else if i % 2 == 1 {
                    dl.rect(row, 0.0, pal.surface.with_alpha(pal.surface.a * 0.35));
                }
                let th = Rect::new(
                    if self.rtl { r.right() - 24.0 } else { r.x },
                    r.y + 2.0,
                    24.0,
                    (r.h - 4.0).min(24.0),
                );
                let mut img = None;
                if e.kind == EntryKind::Image
                    && let Some(p) = atlas.best(&e.path, class).or_else(|| {
                        // A version shows its photo until its own thumbnail is made.
                        self.edited
                            .photo(&e.path)
                            .and_then(|ph| atlas.best(ph, class))
                    })
                {
                    let (ir, uv, small) = self.crop_or_whole(&atlas, &e.path, &p, th, class);
                    if small {
                        dl.rect(th, 3.0, pal.surface);
                    }
                    images.push(&p, ir, uv, 3.0);
                    img = Some(th);
                } else {
                    let avg = self
                        .averages
                        .get(e.path.as_path())
                        .map_or(pal.surface_raised, |a| avg_color(*a));
                    dl.rect(th, 3.0, avg);
                }
                overlays.push((i, r, img));
                continue;
            }
            if selected {
                let cap = self.caption_h();
                dl.rect(
                    Rect::new(r.x - 4.0, r.y - 4.0, r.w + 8.0, r.h + cap + 6.0),
                    RADIUS + 3.0,
                    pal.accent_soft,
                );
            } else if hover == Some(i) {
                let cap = self.caption_h();
                dl.rect(
                    Rect::new(r.x - 4.0, r.y - 4.0, r.w + 8.0, r.h + cap + 6.0),
                    RADIUS + 3.0,
                    pal.surface_hover,
                );
            }
            let mut shown = None;
            match e.kind {
                EntryKind::Image => {
                    let placed = atlas.best(&e.path, class).or_else(|| {
                        // A version shows its photo until its own thumbnail is made.
                        self.edited
                            .photo(&e.path)
                            .and_then(|ph| atlas.best(ph, class))
                    });
                    // The thumbnail of the tile's own size (not a smaller one standing in while
                    // it loads) that is still too small for the tile: the image itself is small
                    // (or a thin strip), so it is shown whole at its size, centred on the
                    // tile's background, never enlarged into a blur.
                    let small = placed.is_some_and(|p| {
                        atlas.contains(&e.path, class) && {
                            let whole = whole_picture(&p, self.source_size(e), class);
                            let area = if uncropped { r.inset(4.0) } else { r };
                            shown_small(&p, area, self.scale, !uncropped, whole)
                        }
                    });
                    match placed {
                        Some(p) if uncropped || small => {
                            let a = self.fade_of(&e.path, f.now);
                            dl.rect(r, RADIUS, pal.surface.with_alpha(pal.surface.a * 0.5));
                            let fr = if small {
                                fit_sharp(&p, r.inset(4.0), self.scale)
                            } else {
                                fit_rect(&p, r.inset(4.0))
                            };
                            images.push_faded(&p, fr, p.uv, 3.0, a);
                            shown = Some(fr);
                        }
                        Some(p) => {
                            let a = self.fade_of(&e.path, f.now);
                            if a < 1.0 {
                                dl.rect(r, RADIUS, self.placeholder(&e.path, pal));
                            }
                            images.push_faded(&p, r, crop_uv(&p, r), RADIUS, a);
                            shown = Some(r);
                        }
                        None => {
                            let fill = self
                                .averages
                                .get(e.path.as_path())
                                .map_or(pal.surface, |a| avg_color(*a));
                            let pr = match (uncropped, self.aspect_of(e)) {
                                (true, Some(a)) => fit(a, 1.0, r.inset(4.0)),
                                _ => r,
                            };
                            dl.rect(pr, RADIUS, fill);
                        }
                    }
                }
                EntryKind::Folder | EntryKind::Archive => {
                    dl.rect(r, RADIUS, pal.surface_raised);
                    let covers = self.shown_covers(&e.path);
                    if !covers.is_empty() {
                        // The covers (a folder's first images, an archive's first pages) fill
                        // the tile edge to edge (a thin frame, hairline gaps) at every size;
                        // the folder or archive badge sits over the corner.
                        let n = covers.len();
                        let pad = (r.w.min(r.h) * 0.02).clamp(2.0, 5.0);
                        let gap = (r.w.min(r.h) * 0.012).clamp(1.5, 3.0);
                        let radius = (RADIUS - pad * 0.5).max(2.0);
                        let cc = if n >= 4 { cover_small } else { cover_big };
                        for (p, cr) in covers.iter().zip(mosaic_cells(r.inset(pad), n, gap)) {
                            match atlas.best(p, cc) {
                                Some(pl) => {
                                    let a = self.fade_of(p, f.now);
                                    let (ir, uv, small) =
                                        self.crop_or_whole(&atlas, p, &pl, cr, cc);
                                    if small {
                                        dl.rect(cr, radius, pal.surface);
                                    } else if a < 1.0 {
                                        dl.rect(cr, radius, self.placeholder(p, pal));
                                    }
                                    let radius = if small { 2.0 } else { radius };
                                    images.push_faded(&pl, ir, uv, radius, a);
                                }
                                None => {
                                    let fill = self
                                        .averages
                                        .get(p.as_path())
                                        .map_or(pal.surface, |a| avg_color(*a));
                                    dl.rect(cr, radius, fill);
                                }
                            }
                        }
                    }
                }
                EntryKind::Other => {
                    dl.rect(r, RADIUS, pal.surface_raised);
                }
            }
            overlays.push((i, r, shown));
        }
        self.lap(D_TILES, lap);
        // Pass 2: the thumbnails, one batch per atlas page.
        images.flush(dl, self.backdrop);
        self.lap(D_FLUSH, lap);
        // Pass 3: icons, text, badges, focus.
        for (i, r, shown) in overlays {
            let e = self.entries[self.tiles[i].entry].clone();
            let m = self.marks.mark(&e.path);
            let selected = self.selected.contains(&e.path);
            if details {
                self.draw_detail_row(dl, text, f, i, &e, r, m);
            } else {
                self.draw_tile_overlay(dl, text, f, i, &e, r, shown, m, &mut atlas);
            }
            if m.flag == -1 && !details {
                // Rejects are dimmed.
                dl.rect(r, RADIUS, pal.canvas.with_alpha(0.55));
            }
            if focus == Some(i) && grid_kbd {
                let ring = if details {
                    self.flip(Rect::new(4.0, r.y, self.grid_w() - 8.0, r.h))
                } else {
                    let cap = self.caption_h();
                    Rect::new(r.x - 3.0, r.y - 3.0, r.w + 6.0, r.h + cap + 4.0)
                };
                let wdt = if self.pointer_input { 1.5 } else { 2.5 };
                dl.stroke_rect(ring, RADIUS + 2.0, pal.focus_ring, wdt);
            } else if selected && !details {
                dl.stroke_rect(r.inset(-1.0), RADIUS + 1.0, pal.accent, 2.0);
            }
        }
        self.atlas = Some(atlas);
        self.lap(D_OVER, lap);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_tile_overlay(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        f: &Frame,
        i: usize,
        e: &Entry,
        r: Rect,
        shown: Option<Rect>,
        m: Mark,
        atlas: &mut Atlas,
    ) {
        let pal = f.pal;
        let icon_side = (r.w.min(r.h) * 0.32).clamp(18.0, 64.0);
        match e.kind {
            EntryKind::Folder => {
                let has_covers = !self.shown_covers(&e.path).is_empty();
                if has_covers {
                    let ir = Rect::new(r.x + 6.0, r.bottom() - 26.0, 20.0, 20.0);
                    dl.rect(ir.inset(-3.0), 5.0, pal.surface_raised);
                    draw_icon(dl, Icon::Folder, ir, pal.accent, 1.6, false);
                } else {
                    // An empty folder: the glyph filled with a tint of its outline (tab and
                    // body, side by side so the tint does not double), so it reads as a
                    // folder at a glance rather than a blank tile.
                    let ir = r.centered(icon_side, icon_side);
                    let k = ir.w / 24.0;
                    let at = |x0: f32, y0: f32, x1: f32, y1: f32| {
                        Rect::from_min_max(
                            ir.x + x0 * k,
                            ir.y + y0 * k,
                            ir.x + x1 * k,
                            ir.y + y1 * k,
                        )
                    };
                    let tint = pal.accent.with_alpha(pal.accent.a * 0.16);
                    let clear = Color::TRANSPARENT;
                    let (tab, body) = ([2.0 * k, k, 0.0, 0.0], [0.0, 2.0 * k, 2.0 * k, 2.0 * k]);
                    dl.rect_full(at(2.0, 3.0, 9.0, 6.0), tab, tint, clear, 0.0);
                    dl.rect_full(at(2.0, 6.0, 22.0, 20.0), body, tint, clear, 0.0);
                    draw_icon(dl, Icon::Folder, ir, pal.accent, 1.8, false);
                }
            }
            EntryKind::Archive => {
                let ext = e.ext().to_uppercase();
                if !self.shown_covers(&e.path).is_empty() {
                    // Over the mosaic's corner, like a folder's badge.
                    let w = (ext.chars().count() as f32 * 7.5 + 10.0).min(r.w - 12.0);
                    let ir = Rect::new(r.x + 6.0, r.bottom() - 26.0, w, 20.0);
                    dl.rect(ir.inset(-3.0), 5.0, pal.surface_raised);
                    text_in(text, dl, &ext, BADGE, ir, pal.text_muted);
                } else {
                    // An encrypted archive not unlocked yet: a lock, not its pages.
                    let icon = if self.locked.contains(&e.path) {
                        Icon::Lock
                    } else {
                        Icon::Image
                    };
                    draw_icon(
                        dl,
                        icon,
                        r.centered(icon_side, icon_side),
                        pal.text_muted,
                        1.6,
                        false,
                    );
                    text_in(
                        text,
                        dl,
                        &ext,
                        BADGE,
                        Rect::new(r.x, r.bottom() - 24.0, r.w, 18.0),
                        pal.text_muted,
                    );
                }
            }
            EntryKind::Other => {
                let ext = e.ext().to_uppercase();
                let s = if ext.is_empty() {
                    "—".to_owned()
                } else {
                    ext
                };
                text_in(
                    text,
                    dl,
                    &s,
                    TextStyle::new((r.w * 0.14).clamp(11.0, 22.0)).weight(600.0),
                    r,
                    pal.text_muted,
                );
            }
            EntryKind::Image => {
                // A hairline inside a filled tile: a white-backed image on the white page
                // keeps its edge.
                if shown.is_some() && self.view.mode != Mode::Uncropped {
                    dl.stroke_rect(r.inset(0.5), RADIUS - 0.5, pal.text.with_alpha(0.07), 1.0);
                }
                if let Some(error) = self.failed.get(&e.path) {
                    let memory = match **error {
                        specular_decode::DecodeError::NeedsMemory(n) => Some(n),
                        _ => None,
                    };
                    // Not enough free RAM: the icon moves up for "Needs 24 GB of free RAM"
                    // (the full text is in the tooltip), when the tile has room for it.
                    let words = memory.filter(|_| r.w >= 96.0 && r.h >= 80.0);
                    let side = icon_side * 0.7;
                    let at = match words {
                        Some(_) => r.centered(side, side).translate(0.0, -side * 0.35),
                        None => r.centered(side, side),
                    };
                    // What kind of file it is, with a small alert on it: the file is likely
                    // fine, only its thumbnail could not be made (the tooltip says why).
                    let kind = if specular_query::is_video_ext(&e.ext()) {
                        Icon::Film
                    } else {
                        Icon::Image
                    };
                    draw_icon(dl, kind, at, pal.text_muted, 1.6, false);
                    let mark = (side * 0.42).max(10.0);
                    let mr = Rect::new(
                        at.right() - mark * 0.6,
                        at.bottom() - mark * 0.6,
                        mark,
                        mark,
                    );
                    dl.rect(mr.inset(-2.0), mark, pal.surface_raised);
                    draw_icon(dl, Icon::Alert, mr, pal.warning, 1.6, false);
                    if let Some(n) = words {
                        let s = crate::error_text::needs_memory_short(n);
                        let style = TextStyle::new((r.w * 0.075).clamp(10.0, 13.0));
                        let b = text.layout(&s, style, Some(r.w - 12.0), Align::Center);
                        let y = (at.bottom() + 4.0).min(r.bottom() - b.height - 4.0);
                        text.draw(dl, &b, r.x + 6.0, y, pal.text_muted);
                    }
                }
            }
        }
        let area = shown.unwrap_or(r);
        let meta = self.metas.get(e.path.as_path()).cloned();
        // The info band covers the bottom 20 px; corner marks sit above it.
        let info_band = self.info_overlay && e.kind == EntryKind::Image && r.h >= 110.0;
        let band = if info_band { 20.0 } else { 0.0 };
        // Badges (top leading corner): kind of file.
        if self.show_badges && r.w >= 72.0 {
            let mut badges: Vec<String> = Vec::new();
            // It opens only with a vendor's free decoder that is not here (Settings ›
            // Optional decoders): said first, it is why the tile stays empty.
            if e.kind == EntryKind::Image
                && specular_decode::optional::needs_decoder(&e.path).is_some()
            {
                badges.push(t!("gallery-badge-needs-decoder"));
            }
            // Files of the stack (a photo's Develop versions are not).
            let files: Vec<usize> = self.tiles[i]
                .members
                .iter()
                .copied()
                .filter(|&k| self.edited.photo(&self.entries[k].path).is_none())
                .collect();
            let members = files.len();
            let raw = meta.as_ref().is_some_and(|m| m.raw) || model::is_raw_ext(&e.ext());
            if members > 1 {
                let any_raw = files
                    .iter()
                    .any(|&k| model::is_raw_ext(&self.entries[k].ext()));
                let any_other = files
                    .iter()
                    .any(|&k| !model::is_raw_ext(&self.entries[k].ext()));
                if any_raw && any_other {
                    badges.push("RAW+JPEG".into());
                }
            } else if raw {
                badges.push("RAW".into());
            }
            if let Some(m) = &meta {
                // Motion Photo / Live Photo: the viewer's badge, in words.
                if m.motion {
                    badges.push(if m.live_photo {
                        t!("motion-badge-live")
                    } else {
                        t!("motion-badge")
                    });
                }
                if m.hdr {
                    badges.push("HDR".into());
                }
                if m.pages > 1 {
                    badges.push(t!("gallery-badge-pages", count = i64::from(m.pages)));
                }
            }
            // Develop: edited, and the photo's versions (virtual copies).
            badges.extend(self.develop_badge(e));
            if let Some(&n) = self.versions.get(&e.path) {
                badges.push(t!("gallery-badge-versions", count = i64::from(n)));
            }
            let mut x = if self.rtl {
                area.right() - 6.0
            } else {
                area.x + 6.0
            };
            let y = area.y + 6.0;
            for b in badges {
                let tw = text.measure(&b, BADGE) + 8.0;
                let br = Rect::new(if self.rtl { x - tw } else { x }, y, tw, 16.0);
                if br.x < area.x || br.right() > area.right() {
                    break;
                }
                dl.rect(br, 4.0, Color::rgba(0.0, 0.0, 0.0, 0.6));
                text_in(text, dl, &b, BADGE, br, Color::WHITE);
                x += if self.rtl { -(tw + 4.0) } else { tw + 4.0 };
            }
            // What moves (bottom trailing corner, above a stack's count): a play mark, and a
            // movie's length ("1:05") or an animated image's format ("GIF"), one place for both.
            if let Some(m) = meta.as_ref().filter(|m| m.video || m.animated) {
                let s = if m.video {
                    m.duration_us
                        .map(|d| {
                            // Under a second reads "0:01", not "0:00" (as if it were empty).
                            let secs = (d as f64 / 1e6).max(if d > 0 { 1.0 } else { 0.0 });
                            t!(
                                "gallery-video-duration",
                                duration = crate::app::playback::clock(secs)
                            )
                        })
                        .unwrap_or_default()
                } else {
                    m.format.clone()
                };
                let tw = if s.is_empty() {
                    0.0
                } else {
                    text.measure(&s, BADGE) + 6.0
                };
                let w = 18.0 + tw;
                let lift = if members > 1 { 22.0 } else { 0.0 };
                let br = Rect::new(
                    if self.rtl {
                        area.x + 6.0
                    } else {
                        area.right() - w - 6.0
                    },
                    area.bottom() - band - 22.0 - lift,
                    w,
                    16.0,
                );
                if br.x >= area.x && br.right() <= area.right() {
                    dl.rect(br, 4.0, Color::rgba(0.0, 0.0, 0.0, 0.6));
                    let icon = Rect::new(br.x + 2.0, br.y, 16.0, 16.0);
                    draw_icon(dl, Icon::Play, icon.inset(3.0), Color::WHITE, 1.4, false);
                    if !s.is_empty() {
                        let tr = Rect::new(br.x + 16.0, br.y, tw, 16.0);
                        text_in(text, dl, &s, BADGE, tr, Color::WHITE);
                    }
                }
            }
            // Stack count (bottom trailing corner).
            if members > 1 && !self.expanded.contains(&e.path) {
                let s = members.to_string();
                let tw = text.measure(&s, BADGE).max(10.0) + 10.0;
                let br = Rect::new(
                    if self.rtl {
                        area.x + 6.0
                    } else {
                        area.right() - tw - 6.0
                    },
                    area.bottom() - band - 22.0,
                    tw,
                    16.0,
                );
                // Two offset cards: "more behind this one".
                dl.rect(
                    br.translate(2.0, -2.0),
                    4.0,
                    Color::rgba(1.0, 1.0, 1.0, 0.5),
                );
                dl.rect(br, 4.0, Color::rgba(0.0, 0.0, 0.0, 0.7));
                text_in(text, dl, &s, BADGE, br, Color::WHITE);
            }
        }
        // Marks: label dot and pick/reject (top trailing), stars (bottom leading).
        let corner = if self.rtl {
            area.x + 6.0
        } else {
            area.right() - 6.0
        };
        let mut cx = corner;
        let step = if self.rtl { 20.0 } else { -20.0 };
        if m.flag != 0 {
            let br = Rect::new(
                cx + if self.rtl { 0.0 } else { -18.0 },
                area.y + 6.0,
                18.0,
                18.0,
            );
            let (bg, icon) = if m.flag == 1 {
                (pal.accent, Icon::Check)
            } else {
                (pal.danger, Icon::Close)
            };
            dl.circle(br.center(), 9.0, bg);
            draw_icon(dl, icon, br.inset(4.0), Color::WHITE, 1.8, false);
            cx += step;
        }
        if m.label > 0 {
            let c = (cx + if self.rtl { 9.0 } else { -9.0 }, area.y + 15.0);
            dl.circle(c, 7.0, Color::WHITE.with_alpha(0.9));
            dl.circle(c, 5.5, label_color(m.label));
        }
        if m.rating > 0 && r.w >= 64.0 {
            let s = stars(m.rating);
            let b = text.layout(&s, SMALL, None, Align::Start);
            let br = Rect::new(
                if self.rtl {
                    area.right() - b.width - 14.0
                } else {
                    area.x + 6.0
                },
                area.bottom() - band - 22.0,
                b.width + 8.0,
                16.0,
            );
            dl.rect(br, 4.0, Color::rgba(0.0, 0.0, 0.0, 0.55));
            text.draw(
                dl,
                &b,
                br.x + 4.0,
                br.y + (16.0 - b.height) * 0.5,
                Color::hex(0xFFD60A),
            );
        }
        // Info overlay: size and date over the bottom of the image.
        if info_band {
            let dims = meta
                .as_ref()
                .filter(|m| m.width > 0)
                .map(|m| (m.width, m.height))
                .or(e.dims);
            let mut parts = Vec::new();
            if let Some((iw, ih)) = dims {
                parts.push(format!("{iw}×{ih}"));
            }
            if let Some(d) = model::when(e, meta.as_deref()) {
                parts.push(date_text(d, specular_i18n::DateStyle::Short));
            }
            if !parts.is_empty() {
                let s = specular_i18n::dot_join(&parts);
                let br = Rect::new(area.x, area.bottom() - 20.0, area.w, 20.0);
                dl.rect_full(
                    br,
                    [0.0, 0.0, RADIUS, RADIUS],
                    Color::rgba(0.0, 0.0, 0.0, 0.5),
                    Color::TRANSPARENT,
                    0.0,
                );
                let s = if I18n::global().direction().is_rtl() {
                    truncate_isolated(text, &s, SMALL, br.w - 8.0)
                } else {
                    truncate(text, &s, SMALL, br.w - 8.0)
                };
                text_in(text, dl, &s, SMALL, br, Color::WHITE);
            }
        }
        // Caption.
        if self.captions() {
            let line_h = self.u(16.0);
            let cr = Rect::new(r.x, r.bottom() + 3.0, r.w, line_h);
            let style = self.ts(CAPTION);
            let wkey = cr.w.round() as u32;
            let cap = match self.cap_cache.get(&e.path) {
                Some((w, s)) if *w == wkey => s.clone(),
                // Flinging: a few new captions per frame (shaping a screenful of new names
                // took ~5 ms); the rest when the view slows down.
                _ if self.wish_fast && self.cap_new >= FAST_NEW_CAPTIONS => String::new(),
                // Over this frame's shaping budget: the rest next frame.
                _ if self
                    .cap_start
                    .is_some_and(|t| t.elapsed() >= CAPTION_BUDGET) =>
                {
                    self.cap_more = true;
                    String::new()
                }
                _ => {
                    let mut t = std::time::Instant::now();
                    self.cap_start.get_or_insert(t);
                    self.cap_new += 1;
                    let s = two_lines(text, &e.name, style, cr.w - 2.0);
                    if self.cap_cache.len() >= CAPTIONS_KEPT {
                        self.cap_cache.clear();
                    }
                    self.cap_cache.insert(e.path.clone(), (wkey, s.clone()));
                    self.lap(super::scrollbench::D_SHAPE, &mut t);
                    s
                }
            };
            let color = if self.selected.contains(&e.path) {
                f.pal.text
            } else {
                f.pal.text_muted
            };
            let last = cap.matches('\n').count();
            let ext = e.ext();
            for (k, line) in cap.split('\n').enumerate() {
                if line.is_empty() {
                    continue;
                }
                let lr = cr.translate(0.0, k as f32 * line_h);
                // A file's extension fainter than its name, which is what one reads (left to
                // right names only: split, a right-to-left one would reorder).
                let dot = line.rfind('.').filter(|&d| {
                    k == last
                        && d > 0
                        && e.kind != EntryKind::Folder
                        && !ext.is_empty()
                        && line[d + 1..].eq_ignore_ascii_case(&ext)
                        && !line.chars().any(|c| matches!(c, '\u{0590}'..='\u{08FF}'))
                });
                match dot {
                    Some(d) => {
                        let (stem, tail) = line.split_at(d);
                        let (ws, wt) = (text.measure(stem, style), text.measure(tail, style));
                        let x = lr.x + (lr.w - ws - wt) * 0.5;
                        text_in(text, dl, stem, style, Rect::new(x, lr.y, ws, lr.h), color);
                        let tr = Rect::new(x + ws, lr.y, wt, lr.h);
                        text_in(text, dl, tail, style, tr, f.pal.text_faint);
                    }
                    None => {
                        text_in(text, dl, line, style, lr, color);
                    }
                }
            }
        }
        let _ = atlas;
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_detail_row(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        f: &Frame,
        i: usize,
        e: &Entry,
        r: Rect,
        m: Mark,
    ) {
        let pal = f.pal;
        let cols = self.detail_columns();
        let meta = self.metas.get(e.path.as_path()).cloned();
        let loc = I18n::global().localizer();
        let y = r.y;
        let rh = r.h;
        let cell = |x0: f32, x1: f32| Rect::new(x0, y, (x1 - x0 - 8.0).max(0.0), rh);
        let color = if m.flag == -1 {
            pal.text_faint
        } else {
            pal.text
        };
        match e.kind {
            EntryKind::Folder => draw_icon(
                dl,
                Icon::Folder,
                Rect::new(cols[0] - 30.0, y + 4.0, 20.0, rh - 8.0),
                pal.accent,
                1.5,
                self.rtl,
            ),
            EntryKind::Archive | EntryKind::Other if !self.failed.contains_key(&e.path) => {}
            _ => {}
        }
        let mut name = e.name.clone();
        let members = self.tiles[i].members.len();
        if members > 1 && !self.expanded.contains(&e.path) {
            name = format!("{name}  (+{})", members - 1);
        }
        let nr = cell(cols[0], cols[1]);
        let name = truncate(text, &name, BAR, nr.w - 70.0);
        let used = text_at(text, dl, &name, BAR, self.flip(nr), color, self.rtl);
        // Marks after the name.
        let mut x = nr.x + used + 8.0;
        if m.rating > 0 {
            let s = stars(m.rating);
            x += text_at(
                text,
                dl,
                &s,
                SMALL,
                self.flip(Rect::new(x, y, 80.0, rh)),
                Color::hex(0xFFB800),
                self.rtl,
            ) + 6.0;
        }
        if m.label > 0 {
            dl.circle(
                self.flip(Rect::new(x, y + rh / 2.0 - 5.0, 10.0, 10.0))
                    .center(),
                5.0,
                label_color(m.label),
            );
            x += 14.0;
        }
        if m.flag == 1 {
            draw_icon(
                dl,
                Icon::Check,
                self.flip(Rect::new(x, y + 7.0, 14.0, 14.0)),
                pal.accent,
                1.8,
                false,
            );
        }
        let muted = pal.text_muted;
        if let Some(d) = model::when(e, meta.as_deref()) {
            let s = date_text(d, specular_i18n::DateStyle::Medium);
            text_at(
                text,
                dl,
                &s,
                BAR,
                self.flip(cell(cols[1], cols[2])),
                muted,
                self.rtl,
            );
        }
        if e.kind != EntryKind::Folder {
            let s = loc.bytes(e.len, specular_i18n::ByteUnits::Decimal);
            text_at(
                text,
                dl,
                &s,
                BAR.tabular(),
                self.flip(cell(cols[2], cols[3])),
                muted,
                self.rtl,
            );
        }
        let kind = match e.kind {
            EntryKind::Folder => t!("gallery-kind-folder"),
            _ => meta
                .as_ref()
                .map_or_else(|| e.ext().to_uppercase(), |m| m.format.clone()),
        };
        text_at(
            text,
            dl,
            &kind,
            BAR,
            self.flip(cell(cols[3], cols[4])),
            muted,
            self.rtl,
        );
        let dims = meta
            .as_ref()
            .filter(|m| m.width > 0)
            .map(|m| (m.width, m.height))
            .or(e.dims);
        if let Some((iw, ih)) = dims {
            text_at(
                text,
                dl,
                &format!("{iw} × {ih}"),
                BAR.tabular(),
                self.flip(cell(cols[4], self.grid_w() - 12.0)),
                muted,
                self.rtl,
            );
        }
    }

    /// Mirrors a rect for right-to-left layouts.
    pub(super) fn flip(&self, r: Rect) -> Rect {
        if self.rtl { r.mirror_x(self.vw) } else { r }
    }

    /// Leading x of the details columns: name, date, size, type, dimensions.
    pub(super) fn detail_columns(&self) -> [f32; 5] {
        let w = self.grid_w();
        let name = 56.0;
        if w < 560.0 {
            return [name, w * 0.55, w * 0.75, w * 2.0, w * 2.0];
        }
        [name, w * 0.42, w * 0.62, w * 0.74, w * 0.85]
    }

    fn draw_details_header(&mut self, dl: &mut DrawList, text: &mut Text, f: &Frame) {
        let pal = f.pal;
        let dh = self.details_header_h();
        let y = self.grid_top() - dh;
        let bar = self.flip(Rect::new(0.0, y, self.grid_w(), dh));
        dl.rect(bar, 0.0, pal.canvas);
        dl.rect(
            Rect::new(bar.x, bar.bottom() - 1.0, bar.w, 1.0),
            0.0,
            pal.separator,
        );
        let cols = self.detail_columns();
        let keys = [
            SortKey::Name,
            SortKey::DateTaken,
            SortKey::Size,
            SortKey::Type,
            SortKey::Dimensions,
        ];
        for (k, key) in keys.iter().enumerate() {
            let x0 = cols[k];
            let x1 = if k + 1 < 5 {
                cols[k + 1]
            } else {
                self.grid_w() - 12.0
            };
            if x0 >= self.grid_w() {
                continue;
            }
            let r = Rect::new(x0, y, (x1 - x0 - 8.0).max(0.0), dh);
            let active = self.view.sort == *key || (k == 1 && self.view.sort == SortKey::Modified);
            let label = sort_label(*key);
            let c = if active { pal.text } else { pal.text_muted };
            let fr = self.flip(r);
            let used = text_at(
                text,
                dl,
                &label,
                self.ts(SMALL.weight(600.0)),
                fr,
                c,
                self.rtl,
            );
            if active {
                let ax = if self.rtl {
                    fr.right() - used - 14.0
                } else {
                    fr.x + used + 4.0
                };
                let icon = if self.view.desc {
                    Icon::ChevronRight
                } else {
                    Icon::ChevronLeft
                };
                // A chevron turned to point down (descending) or up.
                let ir = Rect::new(ax, y + ((dh - 10.0) * 0.5).round(), 10.0, 10.0);
                let _ = icon;
                let (a, b, c2) = if self.view.desc {
                    (
                        (ir.x, ir.y + 2.0),
                        (ir.x + 5.0, ir.y + 7.0),
                        (ir.x + 10.0, ir.y + 2.0),
                    )
                } else {
                    (
                        (ir.x, ir.y + 7.0),
                        (ir.x + 5.0, ir.y + 2.0),
                        (ir.x + 10.0, ir.y + 7.0),
                    )
                };
                dl.line(a, b, 1.5, c);
                dl.line(b, c2, 1.5, c);
            }
            self.hits.push((fr, Hit::Column(*key)));
        }
    }

    /// Group headers in the grid and the sticky one at the top.
    fn draw_headers(&mut self, dl: &mut DrawList, text: &mut Text, f: &Frame) {
        if self.layout.headers.is_empty() {
            return;
        }
        let pal = f.pal;
        let top = self.grid_top();
        let hh = self.layout.params.map_or(36.0, |p| p.header);
        let gh = self.grid_h();
        let count_in = |g: &Gallery, k: usize| {
            let first = g.layout.headers[k].first;
            let end = g
                .layout
                .headers
                .get(k + 1)
                .map_or(g.tiles.len(), |h| h.first);
            end - first
        };
        for k in 0..self.layout.headers.len() {
            let hd = &self.layout.headers[k];
            let y = top + hd.y - self.scroll;
            if y > top + gh || y + hh < top {
                continue;
            }
            let label = self
                .group_labels
                .get(hd.group as usize)
                .cloned()
                .unwrap_or_default();
            let n = count_in(self, k);
            self.header_line(dl, text, pal, y, hh, &label, n);
        }
        if let Some((k, push)) = self.layout.sticky_header(self.scroll) {
            let hd = &self.layout.headers[k];
            if hd.y < self.scroll {
                let label = self
                    .group_labels
                    .get(hd.group as usize)
                    .cloned()
                    .unwrap_or_default();
                let n = count_in(self, k);
                let y = top - push;
                dl.rect(
                    self.flip(Rect::new(0.0, y, self.grid_w(), hh)),
                    0.0,
                    pal.canvas.with_alpha(0.94),
                );
                self.header_line(dl, text, pal, y, hh, &label, n);
                dl.rect(
                    self.flip(Rect::new(0.0, y + hh - 1.0, self.grid_w(), 1.0)),
                    0.0,
                    pal.separator,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn header_line(
        &self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        y: f32,
        hh: f32,
        label: &str,
        n: usize,
    ) {
        let r = self.flip(Rect::new(14.0, y, self.grid_w() - 28.0, hh));
        let used = text_at(text, dl, label, self.ts(HEADER), r, pal.text, self.rtl);
        let loc = I18n::global().localizer();
        let cr = if self.rtl {
            Rect::new(r.x, y, r.w - used - 10.0, hh)
        } else {
            Rect::new(r.x + used + 10.0, y, r.w - used - 10.0, hh)
        };
        text_at(
            text,
            dl,
            &loc.integer(n as i64),
            self.ts(SMALL.tabular()),
            cr,
            pal.text_faint,
            self.rtl,
        );
    }

    fn draw_band(&mut self, dl: &mut DrawList, pal: &Palette) {
        let (Some(p), Some(q)) = (self.press.as_ref(), self.pointer) else {
            return;
        };
        if !p.band {
            return;
        }
        let y0 = self.grid_top() + p.content_y - self.scroll;
        let r = Rect::from_min_max(p.at.0.min(q.0), y0.min(q.1), p.at.0.max(q.0), y0.max(q.1));
        dl.rect_border(r, 2.0, pal.accent.with_alpha(0.15), pal.accent, 1.0);
    }

    fn draw_scrollbar(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        f: &Frame,
        soon: &mut impl FnMut(f64),
    ) {
        let gh = self.grid_h();
        let total = self.layout.extent();
        if total <= gh + 1.0 {
            return;
        }
        let top = self.grid_top();
        let x = if self.rtl {
            self.vw - self.grid_w() + 2.0
        } else {
            self.grid_w() - 12.0
        };
        let track = Rect::new(x, top + 4.0, 10.0, gh - 8.0);
        self.hits.push((track.inset_xy(-4.0, 0.0), Hit::Scrollbar));
        let since = f.now - self.scrolled_at;
        let hovered = self
            .pointer
            .is_some_and(|p| track.inset_xy(-8.0, 0.0).contains(p));
        let dragging = self.scrollbar_drag.is_some();
        let a = if dragging || hovered {
            1.0
        } else if since < SCROLLBAR_S {
            soon(SCROLLBAR_S - since);
            1.0
        } else if since < SCROLLBAR_S + 0.3 {
            soon(0.016);
            (1.0 - (since - SCROLLBAR_S) / 0.3) as f32
        } else {
            0.0
        };
        if a <= 0.0 {
            return;
        }
        let pal = f.pal;
        let th = (gh * gh / total).max(28.0).min(track.h);
        let ty = track.y + (track.h - th) * (self.scroll / (total - gh)).clamp(0.0, 1.0);
        let wide = hovered || dragging;
        let bw = if wide { 8.0 } else { 5.0 };
        let bx = if self.rtl {
            track.x + 1.0
        } else {
            track.right() - bw - 1.0
        };
        if wide {
            dl.rect(
                Rect::new(bx, track.y, bw, track.h),
                bw / 2.0,
                pal.text.with_alpha(0.06 * a),
            );
        }
        dl.rect(
            Rect::new(bx, ty, bw, th),
            bw / 2.0,
            pal.text.with_alpha(0.45 * a),
        );
        // Where you are: the date or the first letter of the top tile, while dragging or
        // scrolling fast.
        let fast = (self.scroll_target - self.scroll).abs() > gh * 0.5;
        if (dragging || fast || hovered)
            && let Some(label) = self.scroll_label()
        {
            let b = text.layout(&label, BAR.weight(600.0), None, Align::Start);
            let lw = b.width + 20.0;
            let lr = Rect::new(
                if self.rtl {
                    bx + bw + 8.0
                } else {
                    bx - lw - 8.0
                },
                (ty + th / 2.0 - 15.0).clamp(top, top + gh - 30.0),
                lw,
                30.0,
            );
            dl.shadow(lr, 8.0, pal.shadow, 8.0, 2.0);
            dl.rect(lr, 8.0, pal.surface_raised);
            text.draw(
                dl,
                &b,
                lr.x + 10.0,
                lr.y + (30.0 - b.height) / 2.0,
                pal.text,
            );
        }
    }

    /// The scrollbar's bubble: what the tile at the top of the view is sorted by.
    fn scroll_label(&self) -> Option<String> {
        let i = self.layout.items_in(self.scroll, self.scroll + 1.0).start;
        let e = self.entry_of(i)?;
        match self.view.sort {
            SortKey::Name | SortKey::Type => {
                let src = if self.view.sort == SortKey::Type {
                    e.ext().to_uppercase()
                } else {
                    e.name.clone()
                };
                src.chars().next().map(|c| c.to_uppercase().collect())
            }
            SortKey::DateTaken | SortKey::Modified => {
                let d = if self.view.sort == SortKey::Modified {
                    model::modified_secs(e)
                } else {
                    model::when(e, self.metas.get(e.path.as_path()).map(|m| &**m))
                }?;
                Some(date_text(d, specular_i18n::DateStyle::Medium))
            }
            SortKey::Size => Some(
                I18n::global()
                    .localizer()
                    .bytes(e.len, specular_i18n::ByteUnits::Decimal),
            ),
            SortKey::Dimensions => {
                let m = self.metas.get(e.path.as_path())?;
                Some(format!(
                    "{} MP",
                    specular_i18n::fixed(m.width as f64 * m.height as f64 / 1e6, 0)
                ))
            }
        }
    }

    pub(super) fn bar_button(
        &mut self,
        dl: &mut DrawList,
        pal: &Palette,
        r: Rect,
        icon: Icon,
        on: bool,
        hit: Hit,
    ) {
        let hovered = self.pointer.is_some_and(|p| r.contains(p));
        if on {
            dl.rect(r, 6.0, pal.accent_soft);
        } else if hovered {
            dl.rect(r, 6.0, pal.surface_hover);
        }
        let c = if on { pal.accent } else { pal.text };
        draw_icon(dl, icon, r.centered(18.0, 18.0), c, 1.6, self.rtl);
        if hovered && let Some(tip) = self.bar_tip_text(&hit) {
            self.bar_tip_now = Some((r, tip));
        }
        self.hits.push((r, hit));
    }

    /// What a bar button's tooltip says.
    fn bar_tip_text(&self, h: &Hit) -> Option<String> {
        let with_key = |label: String, key: &str| {
            let Some(k) = specular_input::KeyCombo::parse(key) else {
                return label;
            };
            t!(
                "tooltip-with-shortcut",
                label = label,
                shortcut = k.to_string()
            )
        };
        Some(match h {
            Hit::Home => t!("gallery-home"),
            Hit::Up => {
                let parent = self.dir.as_deref()?.parent()?;
                t!("gallery-a11y-up", folder = super::file_name(parent))
            }
            Hit::Filter => with_key(t!("cheatsheet-gallery-filter"), "Ctrl+f"),
            Hit::FacetsButton => t!("gallery-facets"),
            Hit::Library(super::library::Side::Toggle) => t!("library-locations"),
            Hit::CloseFilter => t!("gallery-filter-clear"),
            Hit::More => t!("gallery-more-menu"),
            Hit::SizeDown => t!("cheatsheet-gallery-smaller"),
            Hit::SizeUp => t!("cheatsheet-gallery-bigger"),
            Hit::Layout(m) => super::mode_label(*m),
            _ => return None,
        })
    }

    /// The tooltip of the bar button under the pointer, after the usual wait (none again on
    /// a button just pressed until the pointer has left it).
    fn draw_bar_tip(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        f: &Frame,
        soon: &mut impl FnMut(f64),
    ) {
        let Some((r, tip)) = self.bar_tip_now.take() else {
            self.bar_tip = None;
            return;
        };
        let since = match self.bar_tip {
            Some((r0, since)) if r0 == r => since,
            _ => f.now,
        };
        self.bar_tip = Some((r, since));
        let wait = TOOLTIP_S - (f.now - since);
        if wait > 0.0 {
            if wait.is_finite() {
                soon(wait);
            }
            return;
        }
        let bounds = Rect::new(0.0, 0.0, self.vw, self.vh);
        specular_widgets::draw_tooltip(dl, text, f.pal, &tip, r, bounds, f.text_scale);
    }

    fn draw_top_bar(&mut self, dl: &mut DrawList, text: &mut Text, f: &Frame) {
        let pal = f.pal;
        let w = self.vw;
        let bar = Rect::new(0.0, 0.0, w, TOP_BAR);
        dl.rect(bar, 0.0, pal.surface);
        dl.rect(Rect::new(0.0, TOP_BAR - 1.0, w, 1.0), 0.0, pal.separator);
        // Trailing buttons: View, filter.
        let view_label = t!("gallery-view-menu");
        let vw_ = text.measure(&view_label, BAR) + 38.0;
        let view_r = self.flip(Rect::new(w - vw_ - 8.0, 6.0, vw_, 32.0));
        let hovered = self.pointer.is_some_and(|p| view_r.contains(p));
        if hovered {
            dl.rect(view_r, 6.0, pal.surface_hover);
        }
        // A menu: its label, then a chevron (an icon here would repeat the bottom bar's grid
        // layout button).
        let (tr, cr) = if self.rtl {
            (
                Rect::new(view_r.x + 26.0, 6.0, vw_ - 36.0, 32.0),
                Rect::new(view_r.x + 10.0, 16.0, 12.0, 12.0),
            )
        } else {
            (
                Rect::new(view_r.x + 10.0, 6.0, vw_ - 36.0, 32.0),
                Rect::new(view_r.right() - 22.0, 16.0, 12.0, 12.0),
            )
        };
        text_at(text, dl, &view_label, BAR, tr, pal.text, self.rtl);
        draw_icon(dl, Icon::ChevronDown, cr, pal.text_muted, 1.6, false);
        self.hits.push((view_r, Hit::ViewMenu));
        let filter_on = self.filter_open || !self.filter.is_empty();
        let trail = w - vw_ - 8.0;
        let fr = self.flip(Rect::new(trail - 40.0, 6.0, 32.0, 32.0));
        self.bar_button(dl, pal, fr, Icon::Search, filter_on, Hit::Filter);
        // The panes at the trailing edge are toggled from beside them: the facets, and in
        // the Library its locations (one pane at a time).
        let facets_on = self.filter_open && self.facets.open;
        let fa = self.flip(Rect::new(trail - 80.0, 6.0, 32.0, 32.0));
        self.bar_button(dl, pal, fa, Icon::Sliders, facets_on, Hit::FacetsButton);
        let mut end = trail - 80.0;
        if self.library.on {
            end -= 40.0;
            let lr = self.flip(Rect::new(end, 6.0, 32.0, 32.0));
            let on = self.library_sidebar_shown();
            let hit = Hit::Library(super::library::Side::Toggle);
            self.bar_button(dl, pal, lr, Icon::Folder, on, hit);
        }
        // The background work indicator (drawn by the chrome) ends before those buttons.
        self.work_end = end - 8.0;
        let chip = if self.work_chip > 0.0 {
            self.work_chip + 8.0
        } else {
            0.0
        };
        // Leading: the start screen, then up and the breadcrumb (or the Library's title).
        let home = self.flip(Rect::new(8.0, 6.0, 32.0, 32.0));
        self.bar_button(dl, pal, home, Icon::Home, false, Hit::Home);
        if self.library.on {
            let avail = (end - 16.0) - 48.0 - chip;
            self.draw_library_title(dl, text, f, avail);
            return;
        }
        let up = self.flip(Rect::new(48.0, 6.0, 32.0, 32.0));
        let can_up = self.dir.as_ref().is_some_and(|d| d.parent().is_some());
        if can_up {
            self.bar_button(dl, pal, up, Icon::ChevronUp, false, Hit::Up);
        }
        let lead = if can_up { 88.0 } else { 48.0 };
        let Some(dir) = self.dir.clone() else { return };
        let mut crumbs: Vec<(String, std::path::PathBuf)> = Vec::new();
        for a in dir.ancestors() {
            let name = a
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| a.display().to_string());
            crumbs.push((name, a.to_path_buf()));
            // From the home folder, by its name (the house button is the start screen).
            if home_dir().is_some_and(|home| a == home) {
                break;
            }
        }
        crumbs.reverse();
        let avail = (end - 16.0) - lead - chip;
        let sep = " › ";
        let sep_w = text.measure(sep, BAR);
        // Drop leading crumbs until it fits (an ellipsis stands for them).
        let n_crumbs = crumbs.len();
        let widths: Vec<f32> = crumbs
            .iter()
            .enumerate()
            .map(|(k, c)| {
                // The current folder is drawn bold.
                let style = if k + 1 == n_crumbs {
                    BAR.weight(600.0)
                } else {
                    BAR
                };
                text.measure(&c.0, style) + 10.0
            })
            .collect();
        let mut first = 0;
        let total =
            |from: usize| widths[from..].iter().sum::<f32>() + sep_w * (widths.len() - from) as f32;
        while first + 1 < crumbs.len() && total(first) > avail {
            first += 1;
        }
        let mut x = lead;
        if first > 0 {
            x += text_at(
                text,
                dl,
                "… ›",
                BAR,
                self.flip(Rect::new(x, 6.0, 40.0, 32.0)),
                pal.text_faint,
                self.rtl,
            ) + 6.0;
        }
        let last = crumbs.len() - 1;
        for (k, (name, path)) in crumbs.iter().enumerate().skip(first) {
            let cw = widths[k].min(avail.max(40.0));
            let r = self.flip(Rect::new(x, 6.0, cw, 32.0));
            let is_last = k == last;
            let hovered = !is_last && self.pointer.is_some_and(|p| r.contains(p));
            if hovered {
                dl.rect(r, 6.0, pal.surface_hover);
            }
            let style = if is_last { BAR.weight(600.0) } else { BAR };
            let name = truncate(text, name, style, cw - 8.0);
            text_in(
                text,
                dl,
                &name,
                style,
                r,
                if is_last { pal.text } else { pal.text_muted },
            );
            if !is_last {
                self.hits.push((r, Hit::Crumb(path.clone())));
                x += cw;
                x += text_at(
                    text,
                    dl,
                    sep,
                    BAR,
                    self.flip(Rect::new(x, 6.0, sep_w, 32.0)),
                    pal.text_faint,
                    self.rtl,
                );
            }
        }
    }

    fn draw_filter_bar(&mut self, dl: &mut DrawList, text: &mut Text, f: &Frame) {
        let pal = f.pal;
        let w = self.vw;
        let y = TOP_BAR;
        // Heights grow with the text scale (never below the standard ones); the text with it.
        let bh = self.filter_bar_h();
        let bar_style = self.ts(BAR);
        let small = self.ts(SMALL);
        let fh = self.u(28.0);
        let fy = y + ((bh - fh) * 0.5).round();
        let ic = self.u(16.0);
        let icy = fy + ((fh - ic) * 0.5).round();
        let stop = self.stop();
        self.bar.rects.clear();
        dl.rect(Rect::new(0.0, y, w, bh), 0.0, pal.surface);
        dl.rect(Rect::new(0.0, y + bh - 1.0, w, 1.0), 0.0, pal.separator);
        let field_w = (w * 0.34).clamp(160.0, 320.0 * self.text_scale.max(1.0));
        let field = self.flip(Rect::new(10.0, fy, field_w, fh));
        dl.rect_border(
            field,
            6.0,
            pal.canvas,
            if self.filter_focus {
                pal.focus_ring
            } else {
                pal.separator
            },
            if self.filter_focus { 1.5 } else { 1.0 },
        );
        let ir = if self.rtl {
            Rect::new(field.right() - ic - 8.0, icy, ic, ic)
        } else {
            Rect::new(field.x + 8.0, icy, ic, ic)
        };
        draw_icon(dl, Icon::Search, ir, pal.text_faint, 1.5, self.rtl);
        let lead = ic + 14.0;
        let tr = if self.rtl {
            Rect::new(field.x + 8.0, fy, field_w - lead - 8.0, fh)
        } else {
            Rect::new(field.x + lead, fy, field_w - lead - 8.0, fh)
        };
        let caret_h = (bar_style.size * 1.25).round();
        let caret_y = fy + ((fh - caret_h) * 0.5).round();
        // Clip to the field; a long placeholder gets an ellipsis and a long value keeps the
        // caret in view.
        dl.push_clip(tr);
        // A composition belongs to the focused field (this one, or the import's); it is
        // drawn at the caret.
        if !self.filter_focus && !self.import_wants_text() {
            self.preedit.clear();
        }
        let preedit = if self.filter_focus {
            self.preedit.clone()
        } else {
            String::new()
        };
        self.ime_area = None;
        self.filter_drawn = None;
        if self.filter.text.is_empty() && preedit.is_empty() {
            let ph = truncate_end(
                text,
                &t!("gallery-filter-query-placeholder"),
                bar_style,
                tr.w,
            );
            text_at(text, dl, &ph, bar_style, tr, pal.text_faint, self.rtl);
            if self.filter_focus {
                let cx = if self.rtl { tr.right() - 1.5 } else { tr.x };
                let caret = Rect::new(cx, caret_y, 1.5, caret_h);
                dl.rect(caret, 0.0, pal.text);
                self.ime_area = Some(caret);
            }
        } else {
            // The composition sits at the caret.
            let caret = self.filter_caret();
            let ft = &self.filter.text;
            let pre = preedit.len();
            let s = format!("{}{}{}", &ft[..caret], preedit, &ft[caret..]);
            let block = text.layout(&s, bar_style, None, Align::Start);
            let cx = block.caret_x(caret + pre);
            // At the start edge while it fits; scrolled to keep the caret in view when not.
            let x0 = specular_widgets::line_edit::field_x0(&block, cx, tr, self.rtl);
            // The selection, behind the text (not while composing: the composition is at the
            // caret, and the selection would be replaced by it).
            let sel = self.filter_selection();
            if self.filter_focus && pre == 0 && !sel.is_empty() {
                for (a, b) in block.range_spans(sel) {
                    let r = Rect::new(x0 + a, caret_y, b - a, caret_h);
                    dl.rect(r, 0.0, pal.selection);
                }
            }
            text.draw(dl, &block, x0, tr.y + (tr.h - block.height) * 0.5, pal.text);
            // Errors are underlined where they are.
            let at = |b: usize| if b > caret { b + pre } else { b };
            for e in self.filter.query().errors {
                let (a, b) = (e.span.start.min(ft.len()), e.span.end.min(ft.len()));
                if !(ft.is_char_boundary(a) && ft.is_char_boundary(b)) || a > b {
                    continue;
                }
                let (xa, xb) = (block.caret_x(at(a)), block.caret_x(at(b)));
                dl.rect(
                    Rect::new(
                        x0 + xa.min(xb),
                        caret_y + caret_h - 1.0,
                        (xa - xb).abs().max(6.0),
                        1.5,
                    ),
                    0.0,
                    pal.danger,
                );
            }
            if self.filter_focus {
                let cx = (x0 + cx - 0.75).round().clamp(tr.x, tr.right() - 1.5);
                dl.rect(Rect::new(cx, caret_y, 1.5, caret_h), 0.0, pal.text);
                // The composition ends at the caret: underlined, and the candidate window
                // opens under it.
                let (pa, pb) = (x0 + block.caret_x(caret), x0 + block.caret_x(caret + pre));
                let (x0, x1) = (pa.min(pb), pa.max(pb));
                if pre > 0 {
                    dl.rect(
                        Rect::new(x0, caret_y + caret_h - 1.0, x1 - x0, 1.0),
                        0.0,
                        pal.text_muted,
                    );
                }
                let (x0, x1) = (x0.min(cx).max(tr.x), x1.max(cx + 1.5).min(tr.right()));
                self.ime_area = Some(Rect::new(x0, caret_y, (x1 - x0).max(1.5), caret_h));
            }
            self.filter_drawn = Some((block, x0));
        }
        dl.pop_clip();
        self.hits.push((field, Hit::FilterField));
        self.bar.rects.push((Stop::Field, field));
        let cs = self.u(32.0);
        let close = self.flip(Rect::new(
            w - cs - 8.0,
            y + ((bh - cs) * 0.5).round(),
            cs,
            cs,
        ));
        self.bar_button(dl, pal, close, Icon::Close, false, Hit::CloseFilter);
        self.bar.rects.push((Stop::Clear, close));
        // The facets button.
        // Truncated first, then sized to what is left (a width measured back can be a hair
        // short of what truncation measures, which cut whole words in Arabic).
        let label = truncate(
            text,
            &t!("gallery-facets"),
            bar_style,
            140.0 * self.text_scale.max(1.0),
        );
        let bw = text.measure(&label, bar_style).ceil() + 26.0;
        let bx = w - cs - 8.0 - 8.0 - bw;
        let br = self.flip(Rect::new(bx, fy, bw, fh));
        let hovered = self.pointer.is_some_and(|p| br.contains(p));
        let on = self.facets.open;
        let fill = if on {
            pal.accent_soft
        } else if hovered {
            pal.surface_hover
        } else {
            pal.canvas
        };
        dl.rect_border(
            br,
            fh * 0.5,
            fill,
            if on { pal.accent } else { pal.separator },
            1.0,
        );
        text_in(
            text,
            dl,
            &label,
            bar_style,
            br,
            if on { pal.accent } else { pal.text },
        );
        self.hits.push((br, Hit::FacetsButton));
        self.bar.rects.push((Stop::Facets, br));
        let end = bx - 8.0;
        let mut x = 10.0 + field_w + 10.0;
        // The first error, in words (the field underlines them all).
        let q = self.filter.query();
        if let Some(e) = q.errors.first() {
            let msg = facets::query_error(e);
            let avail = (end - x - ic - 6.0).max(0.0);
            let ir = self.flip(Rect::new(x, icy, ic, ic));
            draw_icon(dl, Icon::Alert, ir, pal.danger, 1.5, false);
            let m = truncate_end(text, &msg, small, avail);
            let mw = text.measure(&m, small);
            let mr = self.flip(Rect::new(x + ic + 6.0, fy, mw, fh));
            text_at(text, dl, &m, small, mr, pal.danger, self.rtl);
            x += ic + 6.0 + mw + 12.0;
        }
        // Chips: the field terms, each removable.
        let ch = self.u(24.0);
        let cy = y + ((bh - ch) * 0.5).round();
        let xs = self.u(12.0);
        let mut shown = 0;
        for (k, (chip, label)) in self.filter.chips().into_iter().enumerate() {
            let label = truncate(text, &label, small, 220.0 * self.text_scale.max(1.0));
            let tw = text.measure(&label, small) + 22.0 + xs;
            if x + tw > end {
                break;
            }
            let r = self.flip(Rect::new(x, cy, tw, ch));
            let hovered = self.pointer.is_some_and(|p| r.contains(p));
            dl.rect(
                r,
                ch * 0.5,
                if hovered {
                    pal.surface_hover
                } else {
                    pal.accent_soft
                },
            );
            let lr = if self.rtl {
                Rect::new(r.x + xs + 10.0, r.y, tw - xs - 20.0, ch)
            } else {
                Rect::new(r.x + 10.0, r.y, tw - xs - 20.0, ch)
            };
            text_at(text, dl, &label, small, lr, pal.text, self.rtl);
            let xy = r.y + ((ch - xs) * 0.5).round();
            let xr = if self.rtl {
                Rect::new(r.x + 6.0, xy, xs, xs)
            } else {
                Rect::new(r.right() - xs - 6.0, xy, xs, xs)
            };
            draw_icon(dl, Icon::Close, xr, pal.text_muted, 1.5, false);
            self.hits.push((r, Hit::Chip(chip)));
            self.bar.rects.push((Stop::Chip(k), r));
            shown += 1;
            x += tw + 6.0;
        }
        self.bar.chips_shown = shown;
        // The keyboard focus on a control of the bar (the field shows its own).
        if let Some((_, r)) = self
            .bar
            .rects
            .iter()
            .find(|(s, _)| *s == stop && *s != Stop::Field)
        {
            let ring = r.inset(-2.0);
            let wdt = if self.pointer_input { 1.5 } else { 2.0 };
            dl.stroke_rect(ring, (ring.h * 0.5).min(8.0), pal.focus_ring, wdt);
        }
    }

    fn draw_bottom_bar(&mut self, dl: &mut DrawList, text: &mut Text, f: &Frame) {
        let pal = f.pal;
        let w = self.vw;
        let y = self.vh - BOTTOM_BAR;
        dl.rect(Rect::new(0.0, y, w, BOTTOM_BAR), 0.0, pal.surface);
        dl.rect(Rect::new(0.0, y, w, 1.0), 0.0, pal.separator);
        let loc = I18n::global().localizer();
        // Leading: counts.
        let status = if !self.selected.is_empty() {
            t!(
                "gallery-selected",
                count = self.selected.len() as i64,
                total = self.tiles.len() as i64
            )
        } else if self.listing {
            t!("gallery-listing", count = self.entries.len() as i64)
        } else if self.tiles.len() != self.entries.len() && !self.filter.is_empty() {
            t!(
                "gallery-shown-of",
                count = self.tiles.len() as i64,
                total = self.entries.len() as i64
            )
        } else {
            t!("gallery-items", count = self.tiles.len() as i64)
        };
        let _ = &loc;
        // Trailing controls, laid out from the trailing edge.
        let mut x = w - 8.0;
        let mut place = |wd: f32| {
            x -= wd;
            let r = Rect::new(x, y + 6.0, wd, 32.0);
            x -= 4.0;
            r
        };
        let more = place(32.0);
        let sort_text = format!(
            "{} {}",
            sort_label(self.view.sort),
            if self.view.desc { "↓" } else { "↑" }
        );
        let sort_w = text.measure(&sort_text, BAR) + 20.0;
        let sort = place(sort_w);
        let plus = place(28.0);
        let slider_w = if w > 640.0 { 110.0 } else { 0.0 };
        let slider = place(slider_w);
        let minus = place(28.0);
        let gap = place(8.0);
        let _ = gap;
        let modes = [
            (Mode::Squares, Icon::Grid),
            (Mode::Justified, Icon::Image),
            (Mode::Uncropped, Icon::Fit),
            (Mode::Details, Icon::More),
        ];
        let mut mode_rects = Vec::new();
        for _ in 0..3 {
            mode_rects.push(place(30.0));
        }
        let status_w = x - 16.0;
        let status = truncate(text, &status, BAR, status_w.max(0.0));
        // What is left after the count: the chrome shows downloads there.
        let count_end = 14.0 + text.measure(&status, BAR) + 20.0;
        self.status_free = (count_end, (x - 8.0).max(count_end));
        text_at(
            text,
            dl,
            &status,
            BAR,
            self.flip(Rect::new(14.0, y, status_w.max(0.0), BOTTOM_BAR)),
            pal.text_muted,
            self.rtl,
        );
        let flip = |g: &Gallery, r: Rect| g.flip(r);
        self.bar_button(dl, pal, flip(self, more), Icon::More, false, Hit::More);
        // Sort.
        let sr = flip(self, sort);
        if self.pointer.is_some_and(|p| sr.contains(p)) {
            dl.rect(sr, 6.0, pal.surface_hover);
        }
        text_in(text, dl, &sort_text, BAR, sr, pal.text);
        self.hits.push((sr, Hit::Sort));
        let size_enabled = self.view.mode != Mode::Details;
        if size_enabled {
            self.bar_button(dl, pal, flip(self, plus), Icon::Plus, false, Hit::SizeUp);
            if slider_w > 0.0 {
                let r = flip(self, slider);
                let track = Rect::new(r.x + 4.0, r.y + 15.0, r.w - 8.0, 3.0);
                dl.rect(track, 1.5, pal.separator);
                // Small at the leading end (on the right in right-to-left layouts), the
                // fill from there to the knob.
                let v = slider_value(self.view.size);
                let (kx, from) = if self.rtl {
                    (track.right() - track.w * v, track.right())
                } else {
                    (track.x + track.w * v, track.x)
                };
                dl.rect(
                    Rect::new(from.min(kx), track.y, (kx - from).abs(), 3.0),
                    1.5,
                    pal.accent,
                );
                dl.circle((kx, track.y + 1.5), 7.0, pal.surface_raised);
                dl.stroke_rect(
                    Rect::new(kx - 7.0, track.y - 5.5, 14.0, 14.0),
                    7.0,
                    pal.separator,
                    1.0,
                );
                self.hits.push((r, Hit::Slider));
            }
            self.bar_button(
                dl,
                pal,
                flip(self, minus),
                Icon::Minus,
                false,
                Hit::SizeDown,
            );
        }
        // Layouts (the details view is in the View menu).
        for (k, r) in mode_rects.into_iter().rev().enumerate() {
            let (m, icon) = modes[k];
            let on = self.view.mode == m;
            let fr = flip(self, r);
            self.bar_button(dl, pal, fr, icon, on, Hit::Layout(m));
            if m == Mode::Justified {
                // The justified icon: three uneven bars.
                let c = if on { pal.accent } else { pal.text };
                dl.rect(
                    fr.centered(18.0, 18.0),
                    0.0,
                    if on { pal.accent_soft } else { pal.surface },
                );
                let b = fr.centered(18.0, 18.0);
                for (row, ws) in [[0.6f32, 0.4], [0.35, 0.65], [0.5, 0.5]].iter().enumerate() {
                    let yy = b.y + 1.0 + row as f32 * 6.0;
                    let w0 = 18.0 * ws[0] - 1.0;
                    dl.rect(Rect::new(b.x, yy, w0, 4.5), 1.0, c);
                    dl.rect(Rect::new(b.x + w0 + 2.0, yy, 18.0 - w0 - 2.0, 4.5), 1.0, c);
                }
            }
        }
    }

    fn draw_empty(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        f: &Frame,
        soon: &mut impl FnMut(f64),
    ) {
        if !self.tiles.is_empty() {
            return;
        }
        let pal = f.pal;
        let r = self.flip(Rect::new(
            0.0,
            self.grid_top(),
            self.grid_w(),
            self.grid_h(),
        ));
        let msg = if self.library.on {
            match self.library_empty_text() {
                Some(m) => m,
                None => return,
            }
        } else if let Some(e) = &self.list_error {
            t!("gallery-list-error", reason = e.clone())
        } else if self.listing {
            // No spinner: a word after a moment, for slow network folders.
            let waited = self
                .stats
                .entered
                .map_or(1.0, |t| t.elapsed().as_secs_f64());
            if waited < 0.4 {
                soon(0.4 - waited);
                return;
            }
            t!("gallery-listing", count = self.entries.len() as i64)
        } else if !self.entries.is_empty() {
            t!("gallery-empty-filter")
        } else if self.dir.is_none() {
            t!("gallery-no-folder")
        } else {
            t!("gallery-empty")
        };
        text_in(text, dl, &msg, TextStyle::new(15.0), r, pal.text_muted);
    }

    fn draw_tooltip(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        f: &Frame,
        soon: &mut impl FnMut(f64),
    ) {
        let Some((i, since)) = self.hover else { return };
        if self.press.is_some() || self.quick.is_some() || self.compare.is_some() {
            return;
        }
        let wait = TOOLTIP_S - (f.now - since);
        if wait > 0.0 {
            soon(wait);
            return;
        }
        let (Some(p), Some(e)) = (self.pointer, self.entry_of(i).cloned()) else {
            return;
        };
        let pal = f.pal;
        let mut lines = vec![e.name.clone()];
        let meta = self.metas.get(e.path.as_path()).cloned();
        let loc = I18n::global().localizer();
        let dims = meta
            .as_ref()
            .filter(|m| m.width > 0)
            .map(|m| (m.width, m.height))
            .or(e.dims);
        let mut second = Vec::new();
        if let Some((w, h)) = dims {
            second.push(format!(
                "{} × {}",
                loc.integer(w as i64),
                loc.integer(h as i64)
            ));
        }
        if e.kind != EntryKind::Folder {
            second.push(loc.bytes(e.len, specular_i18n::ByteUnits::Decimal));
        }
        if let Some(m) = &meta {
            second.push(m.format.clone());
        }
        if !second.is_empty() {
            lines.push(specular_i18n::dot_join(&second));
        }
        if let Some(d) = model::when(&e, meta.as_deref()) {
            lines.push(date_text(d, specular_i18n::DateStyle::Long));
        }
        if let Some(error) = self.failed.get(&e.path) {
            lines.push(failure_line(error));
        }
        if let Some(id) = specular_decode::optional::needs_decoder(&e.path) {
            lines.push(t!(
                "gallery-needs-decoder",
                decoder = id.decoder_name(),
                vendor = id.vendor()
            ));
        }
        if let Some(m) = &meta {
            let mut cam = Vec::new();
            if let Some(c) = &m.camera {
                cam.push(c.clone());
            }
            if let Some(l) = &m.lens {
                cam.push(l.clone());
            }
            if !cam.is_empty() {
                lines.push(specular_i18n::dot_join(&cam));
            }
        }
        let blocks: Vec<_> = lines
            .iter()
            .enumerate()
            .map(|(k, l)| {
                text.layout(
                    l,
                    if k == 0 { SMALL.weight(600.0) } else { SMALL },
                    Some(320.0),
                    Align::Start,
                )
            })
            .collect();
        let tw = blocks.iter().map(|b| b.width).fold(0.0, f32::max) + 16.0;
        let th = blocks.iter().map(|b| b.height).sum::<f32>() + 12.0;
        let mut x = p.0 + 14.0;
        let mut y = p.1 + 18.0;
        if x + tw > self.vw - 4.0 {
            x = (p.0 - tw - 8.0).max(4.0);
        }
        if y + th > self.vh - BOTTOM_BAR {
            y = (p.1 - th - 10.0).max(TOP_BAR);
        }
        let r = Rect::new(x, y, tw, th);
        dl.shadow(r, 6.0, pal.shadow, 8.0, 2.0);
        dl.rect(r, 6.0, pal.tooltip_bg);
        let mut yy = y + 6.0;
        for b in &blocks {
            text.draw(dl, b, x + 8.0, yy, pal.tooltip_text);
            yy += b.height;
        }
    }

    /// A drag over the grid: the folder tile it would land in, tinted and outlined, and a pill
    /// at the pointer saying what a drop does there ("Move to Holiday", or the file count over
    /// anything else).
    fn draw_drag(&mut self, dl: &mut DrawList, text: &mut Text, f: &Frame) {
        let pal = f.pal;
        let target = self.drop_target.clone();
        if let Some(t) = &target
            && let Some(r) = self.tile_rect(t.tile)
        {
            dl.rect(r, RADIUS, pal.accent.with_alpha(0.16));
            dl.stroke_rect(r.inset(-3.0), RADIUS + 2.0, pal.accent, 3.0);
        }
        let internal = self.press.as_ref().is_some_and(|p| p.drag);
        let Some(q) = self.pointer.filter(|_| internal) else {
            return;
        };
        let s = match &target {
            Some(t) => {
                let folder = t
                    .dest
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if t.copy {
                    t!("gallery-drop-copy", folder = folder)
                } else {
                    t!("gallery-drop-move", folder = folder)
                }
            }
            None => t!("gallery-drag-count", count = self.targets().len() as i64),
        };
        let style = SMALL.weight(600.0);
        let tw = text.measure(&s, style).min(self.vw * 0.5) + 18.0;
        let r = Rect::new(q.0 + 12.0, q.1 + 12.0, tw, 24.0);
        dl.shadow(r, 12.0, pal.shadow, 8.0, 2.0);
        dl.rect(r, 12.0, pal.accent_fill);
        let s = truncate(text, &s, style, tw - 18.0);
        text_in(text, dl, &s, style, r, pal.accent_text);
    }

    /// Quick look: the focused image large over a dimmed grid.
    fn draw_quick(&mut self, dl: &mut DrawList, text: &mut Text, f: &Frame) {
        let Some(path) = self.quick.clone() else {
            return;
        };
        let pal = f.pal;
        dl.rect(
            Rect::new(0.0, 0.0, self.vw, self.vh),
            0.0,
            Color::rgba(0.0, 0.0, 0.0, 0.82),
        );
        let cap = self.quick_caption_h();
        let area = super::quick::quick_area(self.vw, self.vh, cap);
        // A channel view rendered by the image renderer (`quick.rs`), else the thumbnail.
        self.draw_pane(dl, &path, area, pal);
        let e = self
            .by_path
            .get(path.as_path())
            .map(|&i| self.entries[i].clone());
        if let Some(e) = e {
            let label = self.quick_caption(&e);
            text_in(
                text,
                dl,
                &label,
                self.ts(BAR),
                Rect::new(0.0, self.vh - 14.0 - cap, self.vw, cap),
                Color::WHITE,
            );
        }
    }

    pub(super) fn quick_caption(&self, e: &Entry) -> String {
        let meta = self.metas.get(e.path.as_path());
        let mut parts = vec![e.name.clone()];
        if let Some(m) = meta.filter(|m| m.width > 0) {
            parts.push(format!("{} × {}", m.width, m.height));
        }
        let m = self.marks.mark(&e.path);
        if m.rating > 0 {
            parts.push(stars(m.rating));
        }
        // The viewer's channel badge is not drawn over the gallery: the caption names the view.
        if let Some((_, _, channel)) = self.pane_tex(&e.path) {
            parts.push(specular_widgets::channel_label(channel));
        }
        specular_i18n::I18n::global().join(&parts, "  ·  ")
    }

    /// The best thumbnail of `path` fitted in `area` (its average colour until it comes),
    /// never larger than the picture's own pixels: the rect drawn (`area` without an atlas).
    pub(super) fn draw_big(
        &mut self,
        dl: &mut DrawList,
        path: &Path,
        area: Rect,
        pal: &Palette,
    ) -> Rect {
        let Some(mut atlas) = self.atlas.take() else {
            return area;
        };
        let mut images = Images(Vec::new());
        let source = self.source_size_at(path);
        let r = match atlas.best(path, 512) {
            Some(p) => {
                // A small picture at its own pixels; a larger one's thumbnail stands in for
                // the sharp render on its way, at the size the picture shows (never past
                // 100 %).
                let whole = atlas.contains(path, 512) && whole_picture(&p, source, 512);
                let r = match source.filter(|s| s.0 > 0 && s.1 > 0) {
                    _ if whole => fit_sharp(&p, area, self.scale),
                    Some(s) => fit_at_most(&p, area, self.scale, s),
                    None => fit_rect(&p, area),
                };
                dl.shadow(r, 4.0, pal.shadow, 24.0, 6.0);
                images.push(&p, r, p.uv, 4.0);
                r
            }
            None => {
                let e = self.by_path.get(path).map(|&i| &self.entries[i]);
                let a = e.and_then(|e| self.aspect_of(e)).unwrap_or(1.5);
                let fill = self
                    .averages
                    .get(path)
                    .map_or(pal.surface, |c| avg_color(*c));
                let r = match source.filter(|s| s.0 > 0 && s.1 > 0) {
                    Some(s) => {
                        let f = fit(a, 1.0, area);
                        let k =
                            (s.0.max(s.1) as f32 / self.scale.max(0.01) / f.w.max(f.h)).min(1.0);
                        area.centered(f.w * k, f.h * k)
                    }
                    None => fit(a, 1.0, area),
                };
                dl.rect(r, 4.0, fill);
                r
            }
        };
        images.flush(dl, self.backdrop);
        self.atlas = Some(atlas);
        r
    }

    /// Quick look's or a compare cell's image in `area`: its channel view rendered by the
    /// image renderer (`quick.rs`), else its thumbnail.
    fn draw_pane(&mut self, dl: &mut DrawList, path: &Path, area: Rect, pal: &Palette) {
        match self.pane_tex(path) {
            Some((tex, r, _)) => {
                dl.shadow(r, 4.0, pal.shadow, 24.0, 6.0);
                dl.image(r, [0.0, 0.0, 1.0, 1.0], tex, 4.0);
            }
            None => {
                self.draw_big(dl, path, area, pal);
            }
        }
    }

    /// The shared element: the thumbnail between its tile and the viewer's image rect.
    fn draw_flight(&mut self, dl: &mut DrawList, tr: &Transition, e: f32) {
        let Some(path) = tr.path.clone() else { return };
        let tile = if tr.tile.w > 0.0 {
            tr.tile
        } else {
            match self.tile_of(&path).and_then(|i| self.tile_rect(i)) {
                Some(r) => r,
                None => return,
            }
        };
        let Some(mut atlas) = self.atlas.take() else {
            return;
        };
        if let Some(p) = atlas.best(&path, 512) {
            let img = fit_rect(&p, tr.image);
            let (from, to) = if tr.opening { (tile, img) } else { (img, tile) };
            let r = lerp_rect(from, to, e);
            // Opening: it fades out at the end, showing the viewer's sharp image under it.
            let a = if tr.opening {
                (1.0 - (e - 0.75).max(0.0) * 4.0).clamp(0.0, 1.0)
            } else {
                1.0
            };
            dl.push_opacity(a);
            let uv = if tr.opening {
                crop_uv(&p, lerp_rect(tile, img, 1.0 - e))
            } else {
                crop_uv(&p, r)
            };
            let uv = if self.view.mode == Mode::Uncropped {
                p.uv
            } else {
                uv
            };
            let radius = RADIUS * if tr.opening { 1.0 - e } else { e };
            dl.image_over(r, uv, p.tex, radius, self.backdrop);
            dl.pop_opacity();
        }
        self.atlas = Some(atlas);
    }

    /// Fill times for the measurements (first frame with every visible tile shown / sharp).
    fn note_fill(&mut self) {
        let Some(t0) = self.stats.entered else { return };
        if self.stats.sharp.is_some() || self.tiles.is_empty() || self.listing {
            return;
        }
        let Some(atlas) = self.atlas.as_ref() else {
            return;
        };
        let class = self.class();
        let range = self
            .layout
            .items_in(self.scroll, self.scroll + self.grid_h());
        let mut all_shown = true;
        let mut all_sharp = true;
        for i in range {
            let Some(e) = self.entry_of(i) else { continue };
            if e.kind != EntryKind::Image || self.failed.contains_key(&e.path) {
                continue;
            }
            let any = specular_thumbs::CLASSES
                .iter()
                .any(|&c| atlas.contains(&e.path, c));
            all_shown &= any;
            all_sharp &= atlas.contains(&e.path, class) && atlas.is_sharp(&e.path, class);
            if !all_shown {
                break;
            }
        }
        if all_shown && self.stats.filled.is_none() {
            self.stats.filled = Some(t0.elapsed());
            info!(target: UI, "gallery: screen filled in {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0);
        }
        if all_shown && all_sharp {
            self.stats.sharp = Some(t0.elapsed());
            info!(target: UI, "gallery: screen sharp in {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0);
        }
    }
}

/// The folder the path bar shows as `~`. None on Windows, which has no `~` (`HOME` there is
/// set only by Unix-style shells).
pub(super) fn home_dir() -> Option<std::path::PathBuf> {
    if cfg!(windows) {
        return None;
    }
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(Into::into)
}

#[cfg(test)]
mod small_tests {
    use super::*;

    fn placed(w: u32, h: u32) -> Placed {
        Placed {
            tex: TexId(1),
            page: 0,
            uv: [0.0, 0.0, 0.5, 0.5],
            width: w,
            height: h,
        }
    }

    /// A small picture is never enlarged in a tile, at any scale factor: it shows whole, at
    /// its own pixels (in physical pixels), centred.
    #[test]
    fn small_pictures_keep_their_pixels() {
        let tile = Rect::new(10.0, 10.0, 160.0, 160.0);
        let p = placed(64, 48);
        for scale in [1.0_f32, 1.5, 2.0] {
            let whole = whole_picture(&p, Some((64, 48)), 256);
            assert!(whole);
            assert!(shown_small(&p, tile, scale, true, whole));
            assert!(shown_small(&p, tile.inset(4.0), scale, false, whole));
            let r = fit_sharp(&p, tile.inset(4.0), scale);
            assert!((r.w * scale - 64.0).abs() < 1e-3 && (r.h * scale - 48.0).abs() < 1e-3);
            let c = tile.inset(4.0);
            assert!(((r.x - c.x) - (c.right() - r.right())).abs() < 1e-3);
        }
        // Big enough for the tile in physical pixels: filled as usual.
        let big = placed(400, 300);
        let whole = whole_picture(&big, Some((400, 300)), 512);
        assert!(!shown_small(&big, tile, 1.0, true, whole));
        // At 2x the same tile needs 320 device px: 300 rows would be enlarged.
        assert!(shown_small(&big, tile, 2.0, true, whole));
    }

    /// A large file's thumbnail is a decode made for its class: not a small picture. A
    /// preview smaller than any the generator takes (the decode failed) is all there is.
    #[test]
    fn large_files_are_not_small_pictures() {
        // A 6000 px photo's class-256 decode.
        let p = placed(256, 171);
        assert!(!whole_picture(&p, Some((6000, 4000)), 256));
        // Its 3/4-class embedded preview (accepted with `prefer_embedded`).
        assert!(!whole_picture(&placed(200, 133), Some((6000, 4000)), 256));
        // Only a 160 px EXIF thumbnail came out of the file.
        assert!(whole_picture(&placed(160, 120), Some((6000, 4500)), 512));
        // Unknown size: smaller than the class means the picture is that small.
        assert!(whole_picture(&placed(100, 80), None, 256));
        assert!(!whole_picture(&placed(256, 200), None, 256));
        // A photo's usual crop stays a crop; a thin strip of a large file is shown whole.
        let tile = Rect::new(0.0, 0.0, 240.0, 240.0);
        assert!(!shown_small(&placed(512, 341), tile, 1.0, true, false));
        assert!(shown_small(&placed(512, 40), tile, 1.0, true, false));
    }

    /// A larger picture's 512 px thumbnail standing in for it in quick look or a compare
    /// cell shows at the picture's size, never past 100 %; a large picture still fits.
    #[test]
    fn big_thumbnails_stop_at_the_pictures_size() {
        let area = Rect::new(0.0, 0.0, 1000.0, 800.0);
        let p = placed(512, 341);
        for scale in [1.0_f32, 1.5, 2.0] {
            let r = fit_at_most(&p, area, scale, (800, 533));
            assert!((r.w * scale - 800.0).abs() < 0.5, "{scale}: {r:?}");
            assert!(((r.x - area.x) - (area.right() - r.right())).abs() < 1e-3);
        }
        // A 6000 px photo fills the area as before.
        let r = fit_at_most(&p, area, 1.0, (6000, 4000));
        assert_eq!(r, fit_rect(&p, area));
        // Its own pixels: the same as `fit_sharp`.
        assert_eq!(
            fit_at_most(&p, area, 2.0, (512, 341)),
            fit_sharp(&p, area, 2.0)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The gallery as laid out and its filter bar as drawn at text scale `ts`.
    fn at_scale(ts: f32) -> Gallery {
        let mut g = Gallery {
            active: true,
            filter_open: true,
            vw: 1600.0,
            vh: 1000.0,
            ..Gallery::default()
        };
        g.filter.text = "camera:x-t5 rating>=3 beach".into();
        g.set_text_scale(ts);
        let pal = Palette::dark();
        let f = Frame {
            pal: &pal,
            now: 1.0,
            reduce_motion: true,
            size: (1600.0, 1000.0),
            scale: 1.0,
            rtl: false,
            text_scale: ts,
            backdrop: Backdrop::None,
        };
        let mut dl = DrawList::new(1.0);
        let mut text = Text::new();
        g.draw_filter_bar(&mut dl, &mut text, &f);
        g
    }

    /// Larger text grows the captions, group headers and bars; the thumbnails keep their size
    /// (so a text-scale change never resizes what is being scrolled through).
    #[test]
    fn text_scale_grows_captions_and_bars_not_thumbnails() {
        let items: Vec<Item> = (0..40)
            .map(|i| Item {
                aspect: 1.5,
                group: i / 10,
            })
            .collect();
        let (a, b) = (at_scale(1.0), at_scale(2.0));
        let (pa, pb) = (a.params(), b.params());
        // Two caption lines.
        assert_eq!(pa.caption, 38.0);
        assert_eq!(pb.caption, 76.0);
        assert_eq!(pb.header, 72.0);
        assert_eq!(pa.tile, pb.tile);
        for mode in [Mode::Squares, Mode::Justified] {
            let (la, lb) = (
                Layout::build(&items, &pa, mode),
                Layout::build(&items, &pb, mode),
            );
            for (ca, cb) in la.cells.iter().zip(&lb.cells) {
                assert_eq!((ca.x, ca.w, ca.h), (cb.x, cb.w, cb.h), "{mode:?}");
            }
        }
        // The bar, its field, chips and buttons.
        assert_eq!(b.filter_bar_h(), 2.0 * a.filter_bar_h());
        assert_eq!(b.grid_top(), TOP_BAR + 80.0);
        assert!(a.bar.chips_shown == 2 && b.bar.chips_shown == 2);
        let rect = |g: &Gallery, s: Stop| {
            g.bar
                .rects
                .iter()
                .find(|(x, _)| *x == s)
                .map(|(_, r)| *r)
                .expect("drawn")
        };
        for s in [Stop::Field, Stop::Chip(0), Stop::Facets, Stop::Clear] {
            let (ra, rb) = (rect(&a, s), rect(&b, s));
            assert_eq!(rb.h, 2.0 * ra.h, "{s:?}");
            // Inside the bar.
            assert!(
                rb.y >= TOP_BAR && rb.bottom() <= TOP_BAR + b.filter_bar_h(),
                "{s:?}"
            );
        }
        assert!(rect(&b, Stop::Chip(0)).w > rect(&a, Stop::Chip(0)).w);
        // Quick look's caption.
        assert_eq!(b.quick_caption_h(), 60.0);
        // Below 100 % the text shrinks but the boxes keep the standard size.
        let s = at_scale(0.8);
        assert_eq!(s.params().caption, 38.0);
        assert_eq!(s.filter_bar_h(), FILTER_BAR);
        assert!((s.ts(CAPTION).size - CAPTION.size * 0.8).abs() < 1e-4);
    }

    #[test]
    fn mosaic_fills_the_tile() {
        let r = Rect::new(10.0, 20.0, 300.0, 300.0);
        for n in 1..=4 {
            let cells = mosaic_cells(r, n, 3.0);
            assert_eq!(cells.len(), n);
            // Edge to edge: the cells reach every side of the tile, and only the gaps are
            // left uncovered.
            let x0 = cells.iter().map(|c| c.x).fold(f32::MAX, f32::min);
            let y0 = cells.iter().map(|c| c.y).fold(f32::MAX, f32::min);
            let x1 = cells.iter().map(|c| c.x + c.w).fold(0.0, f32::max);
            let y1 = cells.iter().map(|c| c.y + c.h).fold(0.0, f32::max);
            assert_eq!((x0, y0, x1, y1), (10.0, 20.0, 310.0, 320.0), "{n}");
            let area: f32 = cells.iter().map(|c| c.w * c.h).sum();
            assert!(area >= r.w * r.h * 0.97, "{n}: {area}");
        }
        assert!(mosaic_cells(r, 0, 3.0).is_empty());
    }
}

/// Why a tile has no thumbnail, for its tooltip, in the user's language: "This image needs
/// 24 GB of free RAM, …", or "No thumbnail: the file is incomplete. …".
pub(super) fn failure_line(error: &specular_decode::DecodeError) -> String {
    match error {
        specular_decode::DecodeError::NeedsMemory(n) => crate::error_text::needs_memory_text(*n),
        // An office document without a stored preview: not a failure.
        specular_decode::DecodeError::Unsupported(m)
            if m == specular_decode::office_dec::NO_PREVIEW =>
        {
            t!("viewer-no-preview")
        }
        e => t!(
            "gallery-thumb-failed",
            reason = crate::error_text::decode_reason(e)
        ),
    }
}

#[cfg(test)]
mod failure_tests {
    use super::*;
    use specular_decode::DecodeError;

    /// The tooltip of a failed tile says why, worded by the app (not the decoder's English).
    #[test]
    fn a_failed_tile_says_why() {
        let cut = failure_line(&DecodeError::Truncated("strip 12".into()));
        assert_eq!(
            cut,
            t!(
                "gallery-thumb-failed",
                reason = t!("error-reason-truncated")
            )
        );
        assert!(!cut.contains("strip 12"), "{cut}");
        let bug = failure_line(&DecodeError::Internal("internal error in x".into()));
        assert!(bug.contains(&t!("error-reason-internal")), "{bug}");
        let n = specular_core::NeedsMemory {
            needed: 24 << 30,
            free: 9 << 30,
        };
        assert_eq!(
            failure_line(&DecodeError::NeedsMemory(n)),
            crate::error_text::needs_memory_text(n)
        );
    }
}
