//! Text: parley for shaping, bidi and line breaking, swash for rasterising, our own atlas.
//!
//! Layout happens in physical pixels so glyphs are hinted for nothing and positioned with
//! quarter-pixel horizontal precision; baselines snap to whole pixels. Gamma-correct blending
//! with a luminance-dependent coverage exponent keeps light-on-dark and dark-on-light text
//! equally heavy.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use parley::{
    Alignment, AlignmentOptions, FontContext, FontFeatures, FontStyle, FontWeight, LayoutContext,
    LineHeight, PositionedLayoutItem, StyleProperty,
};
use specular_theme::{Color, fonts};
use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::zeno::{Format, Vector};

use crate::atlas::Atlas;
use crate::draw::DrawList;

pub const ATLAS_SIZE: u32 = 2048;
const SUBPIXEL_BINS: f32 = 4.0;

/// How text should look.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    /// Logical pixels.
    pub size: f32,
    /// 100–900.
    pub weight: f32,
    /// Tabular figures (readouts that change while you watch).
    pub tabular: bool,
    pub italic: bool,
    /// Line height as a multiple of the size.
    pub line_height: f32,
    /// When wrapping, a word too long for a line breaks at any character (CSS
    /// `overflow-wrap: anywhere`), for hashes, digests and paths; text that fits wraps at words.
    pub break_anywhere: bool,
    /// Source code: a monospaced font where there is one, tabular figures, and always laid
    /// out left to right (a line of code with Arabic or Hebrew in it stays in reading order).
    pub code: bool,
}

impl TextStyle {
    pub const fn new(size: f32) -> TextStyle {
        TextStyle {
            size,
            weight: 400.0,
            tabular: false,
            italic: false,
            line_height: 1.3,
            break_anywhere: false,
            code: false,
        }
    }
    pub const fn code(mut self) -> TextStyle {
        self.code = true;
        self.tabular = true;
        self
    }
    pub const fn weight(mut self, w: f32) -> TextStyle {
        self.weight = w;
        self
    }
    pub const fn tabular(mut self) -> TextStyle {
        self.tabular = true;
        self
    }
    pub const fn line_height(mut self, lh: f32) -> TextStyle {
        self.line_height = lh;
        self
    }
    pub const fn break_anywhere(mut self) -> TextStyle {
        self.break_anywhere = true;
        self
    }
}

/// Horizontal alignment inside the wrap width. `Start`/`End` follow the paragraph direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
}

/// One element of a vector outline ([`Text::outlines`]): points are `[x, y]`, y down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathEl {
    Move([f32; 2]),
    Line([f32; 2]),
    /// Control point, end point.
    Quad([f32; 2], [f32; 2]),
    /// Two control points, end point.
    Cubic([f32; 2], [f32; 2], [f32; 2]),
    Close,
}

#[derive(Clone, Debug)]
struct RunFont {
    font: parley::FontData,
    size: f32,
    coords: Vec<i16>,
    coords_hash: u64,
}

#[derive(Clone, Copy, Debug)]
struct PlacedGlyph {
    run: u16,
    id: u16,
    /// Physical px relative to the block's top-left.
    x: f32,
    y: f32,
    /// Start of the glyph's cluster in the text (bytes).
    byte: u32,
}

/// A cluster of a laid-out line: the smallest piece of text a caret can be put around.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Cluster {
    /// Byte range in the text.
    start: u32,
    end: u32,
    /// Logical px from the block's left edge, and width.
    x: f32,
    advance: f32,
    /// Right to left: the caret before its first byte is on its right.
    rtl: bool,
}

/// A shaped, line-broken paragraph.
#[derive(Debug)]
pub struct TextBlock {
    /// Logical size.
    pub width: f32,
    pub height: f32,
    /// Distance from the top to the first baseline (logical).
    pub baseline: f32,
    pub lines: usize,
    pub rtl: bool,
    scale: f32,
    fonts: Vec<RunFont>,
    glyphs: Vec<PlacedGlyph>,
    /// Clusters in visual order (for carets in single-line blocks).
    clusters: Vec<Cluster>,
    text_len: u32,
}

impl TextBlock {
    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }

    /// The x of a caret before byte `byte` of a single-line block (logical px from its left
    /// edge). A byte no cluster starts at (the end of the text, or past it) gives the
    /// trailing edge of the cluster before it: the right of a left-to-right one, the left of
    /// a right-to-left one, so the caret after Hebrew text is on its left, where the next
    /// letter goes. An empty block gives 0.
    pub fn caret_x(&self, byte: usize) -> f32 {
        let b = byte as u32;
        // The cluster that ends last at or before `b`, for a caret after it.
        let mut before: Option<&Cluster> = None;
        for c in &self.clusters {
            if b >= c.start && b < c.end {
                let frac = (b - c.start) as f32 / (c.end - c.start).max(1) as f32;
                let frac = if c.rtl { 1.0 - frac } else { frac };
                return c.x + c.advance * frac;
            }
            if c.end <= b && before.is_none_or(|p| c.end > p.end) {
                before = Some(c);
            }
        }
        match before {
            Some(c) if c.rtl => c.x,
            Some(c) => c.x + c.advance,
            None => 0.0,
        }
    }

    /// Where the bytes `range` are on screen in a single-line block: their x extents (logical
    /// px from the left edge), left to right, one per visual piece. A range that crosses a
    /// change of direction is not contiguous on screen ("abc אבג" with "c א" selected shows
    /// as two pieces), so a selection is drawn as these spans, not from caret to caret.
    pub fn range_spans(&self, range: std::ops::Range<usize>) -> Vec<(f32, f32)> {
        let (s, e) = (range.start as u32, range.end as u32);
        let mut pieces: Vec<(f32, f32)> = Vec::new();
        if s >= e {
            return pieces;
        }
        for c in &self.clusters {
            let (a, b) = (c.start.max(s), c.end.min(e));
            if a >= b {
                continue;
            }
            let n = (c.end - c.start).max(1) as f32;
            let (fa, fb) = ((a - c.start) as f32 / n, (b - c.start) as f32 / n);
            let (l, r) = if c.rtl {
                (1.0 - fb, 1.0 - fa)
            } else {
                (fa, fb)
            };
            pieces.push((c.x + c.advance * l, c.x + c.advance * r));
        }
        pieces.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut out: Vec<(f32, f32)> = Vec::with_capacity(pieces.len());
        for (x0, x1) in pieces {
            match out.last_mut() {
                // Neighbouring clusters touch (within rounding): one span.
                Some(last) if x0 <= last.1 + 0.5 => last.1 = last.1.max(x1),
                _ => out.push((x0, x1)),
            }
        }
        out
    }

    /// The byte nearest to `x` (logical px from the left edge) in a single-line block: the
    /// boundary of the cluster under it on the side `x` is closer to.
    pub fn hit_byte(&self, x: f32) -> usize {
        let mut best = (f32::INFINITY, self.text_len);
        for c in &self.clusters {
            let (left, right) = if c.rtl {
                (c.end, c.start)
            } else {
                (c.start, c.end)
            };
            for (edge, byte) in [(c.x, left), (c.x + c.advance, right)] {
                let d = (x - edge).abs();
                if d < best.0 {
                    best = (d, byte);
                }
            }
        }
        best.1 as usize
    }
}

/// A cached layout's text and family (owned), with the rest of what it was laid out with.
struct LayoutKey {
    text: String,
    /// The family asked for first ([`Text::layout_family`]); `None`: the interface's.
    family: Option<String>,
    params: LayoutParams,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct LayoutParams {
    size: u32,
    weight: u32,
    flags: u8,
    lh: u32,
    width: u32,
    align: Align,
}

/// A layout key as borrowed parts: the cache is looked up with the caller's `&str`s, and
/// strings are only copied for a layout that is new (a lookup per text drawn per frame).
trait KeyParts {
    fn parts(&self) -> (&str, Option<&str>, LayoutParams);
}

impl KeyParts for LayoutKey {
    fn parts(&self) -> (&str, Option<&str>, LayoutParams) {
        (&self.text, self.family.as_deref(), self.params)
    }
}

impl KeyParts for (&str, Option<&str>, LayoutParams) {
    fn parts(&self) -> (&str, Option<&str>, LayoutParams) {
        *self
    }
}

impl std::hash::Hash for dyn KeyParts + '_ {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        self.parts().hash(h);
    }
}

impl PartialEq for dyn KeyParts + '_ {
    fn eq(&self, o: &Self) -> bool {
        self.parts() == o.parts()
    }
}

impl Eq for dyn KeyParts + '_ {}

// The same hash and equality as the borrowed form, as `Borrow` requires.
impl std::hash::Hash for LayoutKey {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        self.parts().hash(h);
    }
}

impl PartialEq for LayoutKey {
    fn eq(&self, o: &Self) -> bool {
        self.parts() == o.parts()
    }
}

impl Eq for LayoutKey {}

impl<'a> std::borrow::Borrow<dyn KeyParts + 'a> for LayoutKey {
    fn borrow(&self) -> &(dyn KeyParts + 'a) {
        self
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct GlyphKey {
    font: u64,
    index: u32,
    glyph: u16,
    size: u32,
    bin: u8,
    coords: u64,
}

#[derive(Clone, Copy, Debug)]
struct GlyphEntry {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    left: i32,
    top: i32,
}

/// The text system. Create it with [`Text::new`] (it scans system fonts, so the app does it
/// on a background thread at startup).
pub struct Text {
    fcx: FontContext,
    /// The context with system fonts, being built in the background.
    system: Option<std::thread::JoinHandle<FontContext>>,
    /// Whether the platform's fonts are used (else nothing is warmed up: the bundled fonts
    /// are all loaded already).
    system_fonts: bool,
    /// Texts to shape ahead, off this thread (see [`Text::set_warm_up`]).
    warm_texts: Option<WarmUpTexts>,
    /// A copy of the font context being warmed up on another thread.
    warming: Option<std::thread::JoinHandle<FontContext>>,
    /// Warm up again when the current warm-up is done (the locale changed meanwhile).
    warm_again: bool,
    lcx: LayoutContext<()>,
    scx: ScaleContext,
    /// Shapes text in one given face ([`Text::layout_face`]).
    shaper: swash::shape::ShapeContext,
    layouts: HashMap<LayoutKey, (Arc<TextBlock>, u64)>,
    glyphs: HashMap<GlyphKey, Option<GlyphEntry>>,
    /// Marks placed in the atlas: (mark, px per unit) → (atlas generation, position).
    marks: HashMap<(crate::Mark, u32), (u64, (u32, u32))>,
    pub atlas: Atlas,
    frame: u64,
    scale: f32,
    locale: Option<String>,
    family: String,
    /// Set when the atlas overflowed during this frame; the caller should redraw.
    pub needs_redraw: bool,
    /// Every text laid out is kept here while recording ([`Text::record`]; screenshot tests).
    recorded: Option<Vec<String>>,
}

impl std::fmt::Debug for Text {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Text")
            .field("layouts", &self.layouts.len())
            .field("glyphs", &self.glyphs.len())
            .finish()
    }
}

/// Text in Latin letters, digits, punctuation and common symbols only (no script for which
/// the locale changes the font).
fn latin_only(text: &str) -> bool {
    text.chars()
        .all(|c| (c as u32) < 0x250 || ('\u{2000}'..='\u{2BFF}').contains(&c))
}

fn hash64(v: &[i16]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

impl Default for Text {
    fn default() -> Self {
        Text::new()
    }
}

/// `text` in `style` shaped (not yet broken into lines) at `scale`, and the bytes put in
/// front of it (a left-to-right mark for code), which offsets leave out.
fn build_layout(
    lcx: &mut LayoutContext<()>,
    fcx: &mut FontContext,
    family: &str,
    locale: Option<&str>,
    text: &str,
    style: TextStyle,
    scale: f32,
) -> (parley::Layout<()>, u32) {
    // Code is laid out left to right whatever its first strong letter: a left-to-right
    // mark in front sets the paragraph direction (and is left out of the byte offsets).
    let (src, pre): (Cow<str>, u32) = if style.code {
        (
            Cow::Owned(format!("\u{200E}{text}")),
            '\u{200E}'.len_utf8() as u32,
        )
    } else {
        (Cow::Borrowed(text), 0)
    };
    let family: Cow<str> = if style.code {
        Cow::Owned(format!("monospace, {family}"))
    } else {
        Cow::Borrowed(family)
    };
    let mut b = lcx.ranged_builder(fcx, &src, scale, true);
    b.push_default(StyleProperty::FontFamily(parley::FontFamily::Source(
        family,
    )));
    b.push_default(StyleProperty::FontSize(style.size));
    b.push_default(StyleProperty::FontWeight(FontWeight::new(style.weight)));
    b.push_default(StyleProperty::LineHeight(LineHeight::FontSizeRelative(
        style.line_height,
    )));
    if style.italic {
        b.push_default(StyleProperty::FontStyle(FontStyle::Italic));
    }
    if style.break_anywhere {
        b.push_default(StyleProperty::OverflowWrap(parley::OverflowWrap::Anywhere));
    }
    if style.tabular {
        b.push_default(StyleProperty::FontFeatures(FontFeatures::Source(
            Cow::Borrowed("\"tnum\" 1"),
        )));
    }
    // The locale picks language-specific fonts and forms for scripts that have them (Han,
    // Arabic, Cyrillic ...). Latin text gets none, and with a locale fontique cannot cache
    // the fallback for most (script, language) pairs: every new layout ran a fontconfig
    // match (1-3 ms), which made scrolling a gallery of new file names drop frames.
    if !latin_only(text)
        && let Some(loc) = locale.and_then(|l| parley::Language::parse(l).ok())
    {
        b.push_default(StyleProperty::Locale(Some(loc)));
    }
    (b.build(&src), pre)
}

/// Each line's ink as laid out (device px from the layout's left): from the left of its
/// leftmost visible cluster to the right of its rightmost; `None` for a line with nothing
/// to see. Spaces, controls (bidi isolates and marks) and zero-advance clusters are not ink.
fn line_inks(layout: &parley::Layout<()>, src: &str) -> Vec<Option<(f32, f32)>> {
    layout
        .lines()
        .map(|line| {
            let mut ink: Option<(f32, f32)> = None;
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(gr) = item else {
                    continue;
                };
                let mut x = gr.offset();
                for c in gr.run().visual_clusters() {
                    let a = c.advance();
                    let seen = a > 0.0
                        && src.get(c.text_range()).is_some_and(|t| {
                            t.chars()
                                .any(|ch| !ch.is_whitespace() && !is_bidi_control(ch))
                        });
                    if seen {
                        ink = Some(ink.map_or((x, x + a), |(lo, hi)| (lo.min(x), hi.max(x + a))));
                    }
                    x += a;
                }
            }
            ink
        })
        .collect()
}

fn is_bidi_control(c: char) -> bool {
    matches!(
        c,
        '\u{200E}' | '\u{200F}' | '\u{061C}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
    )
}

/// A face of a font file drawn with its own glyphs only (a font specimen): no fallback to
/// other fonts, so a character the face lacks shows as its missing glyph.
#[derive(Clone)]
pub struct FontFace {
    font: parley::FontData,
}

impl std::fmt::Debug for FontFace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontFace")
            .field("index", &self.font.index)
            .finish_non_exhaustive()
    }
}

struct SharedBytes(Arc<Vec<u8>>);

impl AsRef<[u8]> for SharedBytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl FontFace {
    /// Face `index` of the font file `data` (an SFNT or a collection); `None` when the text
    /// stack cannot read it.
    pub fn new(data: Arc<Vec<u8>>, index: u32) -> Option<FontFace> {
        swash::FontRef::from_index(&data, index as usize)?;
        let blob = parley::fontique::Blob::new(Arc::new(SharedBytes(data)));
        Some(FontFace {
            font: parley::FontData::new(blob, index),
        })
    }

    /// Ascent and descent (positive, below the baseline) of the face at `size` px, at the
    /// variation `axes`.
    pub fn metrics(&self, size: f32, axes: &[([u8; 4], f32)]) -> (f32, f32) {
        let Some(font) =
            swash::FontRef::from_index(self.font.data.data(), self.font.index as usize)
        else {
            return (size, 0.0);
        };
        let coords: Vec<i16> = font
            .variations()
            .normalized_coords(settings(axes))
            .collect();
        let m = font.metrics(&coords).scale(size);
        (m.ascent, m.descent)
    }

    /// Glyph `glyph` in colour at `ppem` physical px, when the face has a colour form of it
    /// the text stack draws (COLR version 0 layers in the first palette, or a bitmap from
    /// sbix/CBDT); `None` otherwise.
    pub fn colour_glyph(&self, glyph: u16, ppem: f32) -> Option<ColourGlyph> {
        let font = swash::FontRef::from_index(self.font.data.data(), self.font.index as usize)?;
        let mut scx = ScaleContext::new();
        let mut scaler = scx.builder(font).size(ppem).hint(false).build();
        let img = Render::new(&[
            Source::ColorOutline(0),
            Source::ColorBitmap(StrikeWith::BestFit),
        ])
        .render(&mut scaler, glyph)?;
        let (w, h) = (img.placement.width, img.placement.height);
        if img.content != swash::scale::image::Content::Color
            || w == 0
            || h == 0
            || img.data.len() != (w * h * 4) as usize
        {
            return None;
        }
        // A bitmap strike is drawn at its own size: scale it to the size asked for.
        let advance = font.glyph_metrics(&[]).scale(ppem).advance_width(glyph);
        Some(ColourGlyph {
            width: w,
            height: h,
            left: img.placement.left,
            top: img.placement.top,
            advance,
            rgba: img.data,
        })
    }
}

/// A glyph drawn in colour ([`FontFace::colour_glyph`]): straight RGBA rows, its placement
/// from the pen position on the baseline (physical px, `top` up from the baseline) and its
/// advance.
#[derive(Clone, Debug, PartialEq)]
pub struct ColourGlyph {
    pub width: u32,
    pub height: u32,
    pub left: i32,
    pub top: i32,
    pub advance: f32,
    pub rgba: Vec<u8>,
}

fn settings(axes: &[([u8; 4], f32)]) -> impl Iterator<Item = swash::Setting<f32>> + Clone + '_ {
    axes.iter().map(|(t, v)| swash::Setting {
        tag: swash::tag_from_bytes(t),
        value: *v,
    })
}

/// Texts to shape ahead (see [`Text::set_warm_up`]).
pub type WarmUpTexts = Arc<dyn Fn() -> Vec<String> + Send + Sync>;

/// Shapes `texts` with `fcx` (regular and semibold, the weights of lists and their
/// headings), so it has found and loaded the fallback fonts they need. Off the UI thread.
fn warm(fcx: &mut FontContext, family: &str, locale: Option<&str>, texts: &[String]) {
    let mut lcx = LayoutContext::new();
    for t in texts {
        for w in [400.0, 600.0] {
            let style = TextStyle::new(14.0).weight(w);
            let _ = build_layout(&mut lcx, fcx, family, locale, t, style, 1.0);
        }
    }
}

/// A font context with the bundled fonts and, if `system`, the platform's fonts.
fn font_context(system: bool) -> FontContext {
    let collection = parley::fontique::Collection::new(parley::fontique::CollectionOptions {
        shared: false,
        system_fonts: system,
    });
    let mut fcx = FontContext {
        collection,
        source_cache: parley::fontique::SourceCache::default(),
    };
    for data in fonts::all() {
        fcx.collection
            .register_fonts(parley::fontique::Blob::new(Arc::new(data)), None);
    }
    fcx
}

impl Text {
    /// Text with the bundled fonts only (about 1 ms). System fonts, which take 50-70 ms to
    /// discover, load on a background thread and are swapped in by `begin_frame` when ready;
    /// until then text outside the bundled fonts' coverage falls back to missing glyphs.
    pub fn new() -> Text {
        Text::with_fonts(true)
    }

    /// Text with the bundled fonts only, never the platform's: the same glyphs on every machine
    /// (screenshot tests). Text outside the bundled fonts' coverage draws as missing glyphs.
    pub fn bundled_only() -> Text {
        Text::with_fonts(false)
    }

    fn with_fonts(system_fonts: bool) -> Text {
        let t0 = std::time::Instant::now();
        let fcx = font_context(false);
        specular_log::debug!(target: specular_log::TEXT, "fonts: bundled fonts {:.1} ms", t0.elapsed().as_secs_f64() * 1000.0);
        let system = system_fonts
            .then(|| {
                specular_thread::builder("font-system")
                    .spawn(|| {
                        let t0 = std::time::Instant::now();
                        let f = font_context(true);
                        specular_log::debug!(target: specular_log::TEXT, "fonts: system fonts {:.1} ms", t0.elapsed().as_secs_f64() * 1000.0);
                        f
                    })
                    .ok()
            })
            .flatten();
        let family = format!(
            "\"{}\", \"{}\", \"{}\", system-ui, sans-serif",
            fonts::UI_FAMILY,
            fonts::ARABIC_FAMILY,
            fonts::HEBREW_FAMILY
        );
        Text {
            fcx,
            lcx: LayoutContext::new(),
            scx: ScaleContext::new(),
            shaper: swash::shape::ShapeContext::new(),
            layouts: HashMap::new(),
            glyphs: HashMap::new(),
            marks: HashMap::new(),
            atlas: Atlas::new(ATLAS_SIZE),
            frame: 0,
            scale: 1.0,
            locale: None,
            family,
            needs_redraw: false,
            recorded: None,
            system,
            system_fonts,
            warm_texts: None,
            warming: None,
            warm_again: false,
        }
    }

    /// BCP-47 tag used for shaping (e.g. "ar", "ja"); clears cached layouts.
    pub fn set_locale(&mut self, locale: Option<String>) {
        if self.locale != locale {
            self.locale = locale;
            self.layouts.clear();
            // The fallback fonts are found per script and locale.
            self.start_warm_up();
        }
    }

    /// Texts to shape ahead whenever the fonts or the locale change: their fallback fonts
    /// are then found (a fontconfig match) and loaded (a CJK font takes 100 ms or more) on
    /// another thread, on a copy of the font context that replaces this one when done,
    /// rather than on the UI thread the first time they are drawn. `texts` runs on that
    /// thread too. For lists that show many scripts at once (the language list).
    pub fn set_warm_up(&mut self, texts: WarmUpTexts) {
        self.warm_texts = Some(texts);
        self.start_warm_up();
    }

    fn start_warm_up(&mut self) {
        let Some(texts) = self.warm_texts.clone() else {
            return;
        };
        // Not before the system fonts are in (they start it); one at a time.
        if !self.system_fonts || self.system.is_some() {
            return;
        }
        if self.warming.is_some() {
            self.warm_again = true;
            return;
        }
        let mut fcx = self.fcx.clone();
        let family = self.family.clone();
        let locale = self.locale.clone();
        self.warming = specular_thread::builder("font-warm")
            .spawn(move || {
                let t0 = std::time::Instant::now();
                let texts = texts();
                warm(&mut fcx, &family, locale.as_deref(), &texts);
                specular_log::debug!(target: specular_log::TEXT, "fonts: {} texts warmed up in {:.1} ms", texts.len(), t0.elapsed().as_secs_f64() * 1000.0);
                fcx
            })
            .ok();
    }

    /// Whether a warm-up ([`Text::set_warm_up`]) is still running.
    pub fn warming_up(&self) -> bool {
        self.warming.is_some() || self.warm_again
    }

    /// Call at the start of each frame with the window's scale factor.
    pub fn begin_frame(&mut self, scale: f32) {
        self.frame += 1;
        self.needs_redraw = false;
        if self.system.as_ref().is_some_and(|h| h.is_finished())
            && let Some(Ok(fcx)) = self.system.take().map(|h| h.join())
        {
            // New font blobs get new ids, so cached glyphs of the old context are never hit.
            self.fcx = fcx;
            self.layouts.clear();
            self.start_warm_up();
        }
        if self.warming.as_ref().is_some_and(|h| h.is_finished())
            && let Some(r) = self.warming.take().map(|h| h.join())
        {
            // The warmed copy knows everything this one did when it was made, and the
            // fallbacks and fonts of the texts besides. Fonts both loaded meanwhile differ in
            // id only (their glyphs are drawn again once); laid-out text stays valid.
            if let Ok(fcx) = r {
                self.fcx = fcx;
            }
            if std::mem::take(&mut self.warm_again) {
                self.start_warm_up();
            }
        }
        if (scale - self.scale).abs() > 1e-4 {
            self.scale = scale;
            self.layouts.clear();
        }
    }

    /// System fonts are still loading; draw again soon (they may cover text that is drawn
    /// with missing glyphs now).
    pub fn fonts_pending(&self) -> bool {
        self.system.is_some()
    }

    /// Drops layouts not used for a while.
    pub fn end_frame(&mut self) {
        // Every 30 frames, not each one: a walk over the whole map.
        if self.layouts.len() > 512 && self.frame.is_multiple_of(30) {
            let f = self.frame;
            self.layouts.retain(|_, (_, used)| f - *used < 120);
        }
    }

    /// Starts (or stops) keeping every text laid out, for [`Text::take_recorded`]: the
    /// screenshot tests check what a shot drew against the language it was drawn in.
    #[doc(hidden)]
    pub fn record(&mut self, on: bool) {
        self.recorded = on.then(Vec::new);
    }

    /// The texts laid out since the last call (while recording).
    #[doc(hidden)]
    pub fn take_recorded(&mut self) -> Vec<String> {
        self.recorded
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }

    /// Shapes `text`. `max_width` (logical) wraps lines; `None` keeps one line per paragraph.
    pub fn layout(
        &mut self,
        text: &str,
        style: TextStyle,
        max_width: Option<f32>,
        align: Align,
    ) -> Arc<TextBlock> {
        self.layout_in(None, text, style, max_width, align)
    }

    /// Like [`Text::layout`], in font family `family` (its name as installed) first: what it
    /// lacks comes from the interface's fonts, then the platform's for each script.
    pub fn layout_family(
        &mut self,
        family: &str,
        text: &str,
        style: TextStyle,
        max_width: Option<f32>,
        align: Align,
    ) -> Arc<TextBlock> {
        let family = family.trim();
        let family = (!family.is_empty()).then_some(family);
        self.layout_in(family, text, style, max_width, align)
    }

    /// The font families there are (the bundled ones, and the platform's once loaded), sorted
    /// and without repeats. Hidden families (a leading `.`, as macOS names its own) are left
    /// out.
    pub fn family_names(&mut self) -> Vec<String> {
        let mut v: Vec<String> = self
            .fcx
            .collection
            .family_names()
            .filter(|n| !n.is_empty() && !n.starts_with('.'))
            .map(str::to_owned)
            .collect();
        v.sort_by_key(|n| n.to_lowercase());
        v.dedup();
        v
    }

    /// Registers the fonts of a font file (TTF, OTF, a collection); returns their family
    /// names. They stay until the platform's fonts are swapped in (with [`Text::new`]), so
    /// this is for [`Text::bundled_only`] (screenshot tests).
    #[doc(hidden)]
    pub fn add_font(&mut self, data: Vec<u8>) -> Vec<String> {
        let ids = self
            .fcx
            .collection
            .register_fonts(parley::fontique::Blob::new(Arc::new(data)), None);
        self.layouts.clear();
        ids.into_iter()
            .filter_map(|(id, _)| self.fcx.collection.family_name(id).map(str::to_owned))
            .collect()
    }

    fn layout_in(
        &mut self,
        family: Option<&str>,
        text: &str,
        style: TextStyle,
        max_width: Option<f32>,
        align: Align,
    ) -> Arc<TextBlock> {
        if let Some(r) = &mut self.recorded {
            r.push(text.to_owned());
        }
        let params = LayoutParams {
            size: style.size.to_bits(),
            weight: style.weight.to_bits(),
            flags: style.tabular as u8
                | (style.italic as u8) << 1
                | (style.break_anywhere as u8) << 2
                | (style.code as u8) << 3,
            lh: style.line_height.to_bits(),
            width: max_width.map_or(u32::MAX, f32::to_bits),
            align,
        };
        let parts = (text, family, params);
        if let Some((b, used)) = self.layouts.get_mut(&parts as &dyn KeyParts) {
            *used = self.frame;
            return b.clone();
        }
        let block = Arc::new(self.shape(family, text, style, max_width, align));
        let key = LayoutKey {
            text: text.to_owned(),
            family: family.map(str::to_owned),
            params,
        };
        self.layouts.insert(key, (block.clone(), self.frame));
        block
    }

    /// Shapes `text` in `face` alone at `size` (logical px) and the variation `axes` (tag,
    /// user value), left to right, broken at spaces to fit `max_width` when given (a word
    /// longer than that stays whole). Not cached: the caller keeps the block.
    pub fn layout_face(
        &mut self,
        face: &FontFace,
        text: &str,
        size: f32,
        axes: &[([u8; 4], f32)],
        max_width: Option<f32>,
    ) -> TextBlock {
        let s = self.scale;
        let mut block = TextBlock {
            width: 0.0,
            height: 0.0,
            baseline: 0.0,
            lines: 0,
            rtl: false,
            scale: s,
            fonts: Vec::new(),
            glyphs: Vec::new(),
            clusters: Vec::new(),
            text_len: text.len() as u32,
        };
        let data = face.font.data.data();
        let Some(font) = swash::FontRef::from_index(data, face.font.index as usize) else {
            return block;
        };
        let ppem = size * s;
        let mut shaper = self
            .shaper
            .builder(font)
            .size(ppem)
            .variations(settings(axes))
            .build();
        let coords = shaper.normalized_coords().to_vec();
        let m = font.metrics(&coords).scale(ppem);
        // Clusters as (byte, glyphs (id, x offset, y offset), advance, is a space).
        type Shaped = (u32, Vec<(u16, f32, f32)>, f32, bool);
        let mut shaped: Vec<Shaped> = Vec::new();
        shaper.add_str(text);
        shaper.shape_with(|c| {
            let mut x = 0.0;
            let mut glyphs = Vec::with_capacity(c.glyphs.len());
            for g in c.glyphs {
                glyphs.push((g.id, x + g.x, g.y, g.advance));
                x += g.advance;
            }
            shaped.push((
                c.source.start,
                glyphs.iter().map(|g| (g.0, g.1, g.2)).collect(),
                x,
                c.info.is_whitespace(),
            ));
        });
        // Greedy lines: break before a word that would cross the width.
        let limit = max_width.map(|w| w * s);
        let mut lines: Vec<std::ops::Range<usize>> = Vec::new();
        let mut start = 0;
        let mut x = 0.0f32;
        let mut i = 0;
        while i < shaped.len() {
            if shaped[i].3 {
                x += shaped[i].2;
                i += 1;
                continue;
            }
            let end = (i..shaped.len())
                .find(|&j| shaped[j].3)
                .unwrap_or(shaped.len());
            let w: f32 = shaped[i..end].iter().map(|c| c.2).sum();
            if let Some(l) = limit
                && x + w > l
                && i > start
            {
                lines.push(start..i);
                start = i;
                x = 0.0;
            }
            x += w;
            i = end;
        }
        lines.push(start..shaped.len());
        let coords_hash = hash64(&coords);
        block.fonts.push(RunFont {
            font: face.font.clone(),
            size: ppem,
            coords,
            coords_hash,
        });
        let line_h = (m.ascent + m.descent + m.leading).max(ppem * 0.5).ceil();
        let mut width = 0.0f32;
        for (n, r) in lines.iter().enumerate() {
            let base = (m.ascent + n as f32 * line_h).round();
            let mut x = 0.0;
            // Spaces at the start of a wrapped line take no room.
            let skip = if n > 0 {
                shaped[r.clone()].iter().take_while(|c| c.3).count()
            } else {
                0
            };
            let mut ink = 0.0f32;
            for c in &shaped[r.start + skip..r.end] {
                for &(id, dx, dy) in &c.1 {
                    block.glyphs.push(PlacedGlyph {
                        run: 0,
                        id,
                        x: x + dx,
                        y: base - dy,
                        byte: c.0,
                    });
                }
                x += c.2;
                if !c.3 {
                    ink = x;
                }
            }
            width = width.max(ink);
        }
        block.lines = lines.len();
        block.width = width / s;
        block.baseline = m.ascent.round() / s;
        block.height = (lines.len() as f32 * line_h) / s;
        block
    }

    /// Glyph `glyph` of `face` at `size` (logical px) and the variation `axes`, its origin at
    /// the block's left edge on the baseline; the block is as wide as its advance.
    pub fn layout_glyph(
        &mut self,
        face: &FontFace,
        glyph: u16,
        size: f32,
        axes: &[([u8; 4], f32)],
    ) -> TextBlock {
        let s = self.scale;
        let ppem = size * s;
        let mut block = TextBlock {
            width: 0.0,
            height: 0.0,
            baseline: 0.0,
            lines: 1,
            rtl: false,
            scale: s,
            fonts: Vec::new(),
            glyphs: Vec::new(),
            clusters: Vec::new(),
            text_len: 0,
        };
        let data = face.font.data.data();
        let Some(font) = swash::FontRef::from_index(data, face.font.index as usize) else {
            return block;
        };
        let coords: Vec<i16> = font
            .variations()
            .normalized_coords(settings(axes))
            .collect();
        let m = font.metrics(&coords).scale(ppem);
        let advance = font.glyph_metrics(&coords).scale(ppem).advance_width(glyph);
        let coords_hash = hash64(&coords);
        block.fonts.push(RunFont {
            font: face.font.clone(),
            size: ppem,
            coords,
            coords_hash,
        });
        block.glyphs.push(PlacedGlyph {
            run: 0,
            id: glyph,
            x: 0.0,
            y: m.ascent.round(),
            byte: 0,
        });
        block.width = advance / s;
        block.baseline = m.ascent.round() / s;
        block.height = (m.ascent + m.descent) / s;
        block
    }

    /// Width in logical pixels of a single line.
    pub fn measure(&mut self, text: &str, style: TextStyle) -> f32 {
        self.layout(text, style, None, Align::Start).width
    }

    fn shape(
        &mut self,
        family: Option<&str>,
        text: &str,
        style: TextStyle,
        max_width: Option<f32>,
        align: Align,
    ) -> TextBlock {
        let s = self.scale;
        // A chosen family goes first (quotes taken out: the list is CSS-like), the
        // interface's after it for what it lacks.
        let families: Cow<str> = match family {
            Some(f) => Cow::Owned(format!(
                "\"{}\", {}",
                f.replace(['"', '\\'], ""),
                self.family
            )),
            None => Cow::Borrowed(self.family.as_str()),
        };
        let (mut layout, pre) = build_layout(
            &mut self.lcx,
            &mut self.fcx,
            &families,
            self.locale.as_deref(),
            text,
            style,
            s,
        );
        let al = match align {
            Align::Start => Alignment::Start,
            Align::Center => Alignment::Center,
            Align::End => Alignment::End,
        };
        let src: Cow<str> = if pre > 0 {
            Cow::Owned(format!("\u{200E}{text}"))
        } else {
            Cow::Borrowed(text)
        };
        // Parley breaks lines by advances summed in logical order, and its trailing
        // whitespace is not always the whitespace a line ends with on screen once mixed
        // left-to-right and right-to-left runs are reordered: a line can come out wider than
        // the width it was broken at. Measure each line as drawn (ink from its first to its
        // last visible cluster) and break again, narrower by the overshoot, until it fits.
        //
        // A word longer than the line (a path, a hash, a backtrace frame) overshoots at every
        // width, so narrowing cannot help it. Reordering overshoots by a few spaces at most:
        // once the limit would drop by more than a quarter, or six tries leave the widest
        // line as wide as the first, break at the full width again. (Narrowing on regardless
        // took the limit down to a pixel, and a crash report with one long path came out
        // one word per line.)
        let max = max_width.map(|w| w * s);
        let mut limit = max;
        let mut first_widest: Option<f32> = None;
        let mut fits = true;
        for _ in 0..6 {
            layout.break_all_lines(limit);
            layout.align(al, AlignmentOptions::default());
            let (Some(m), Some(l)) = (max, limit) else {
                break;
            };
            let widest = line_inks(&layout, &src)
                .iter()
                .flatten()
                .map(|(lo, hi)| hi - lo)
                .fold(if layout.is_rtl() { 0.0 } else { layout.width() }, f32::max);
            let over = widest - m;
            fits = over <= 0.01;
            if fits {
                break;
            }
            let first = *first_widest.get_or_insert(widest);
            if l - over < m * 0.75 {
                fits = widest < first - 0.01;
                break;
            }
            fits = widest < first - 0.01;
            limit = Some(l - over);
        }
        if !fits {
            layout.break_all_lines(max);
            layout.align(al, AlignmentOptions::default());
        }

        let mut fonts: Vec<RunFont> = Vec::new();
        let mut glyphs = Vec::new();
        let mut clusters = Vec::new();
        let mut baseline = None;
        let mut lines = 0;
        // Where each line's glyphs and clusters start.
        let mut line_at: Vec<(usize, usize)> = Vec::new();
        for line in layout.lines() {
            lines += 1;
            line_at.push((glyphs.len(), clusters.len()));
            if baseline.is_none() {
                baseline = Some(line.metrics().baseline);
            }
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(gr) = item else {
                    continue;
                };
                let run = gr.run();
                let font = run.font();
                let coords = run.normalized_coords();
                let ch = hash64(coords);
                let size = run.font_size();
                let idx = match fonts.iter().position(|f| {
                    f.font.data.id() == font.data.id()
                        && f.font.index == font.index
                        && f.size == size
                        && f.coords_hash == ch
                }) {
                    Some(i) => i,
                    None => {
                        fonts.push(RunFont {
                            font: font.clone(),
                            size,
                            coords: coords.to_vec(),
                            coords_hash: ch,
                        });
                        fonts.len() - 1
                    }
                };
                // Every glyph run here has one style (only defaults are set), so it covers its
                // run's clusters from the first.
                let mut x = gr.offset();
                let mut bytes = Vec::new();
                for c in run.visual_clusters() {
                    let r = c.text_range();
                    let start = (r.start as u32).saturating_sub(pre);
                    bytes.extend(c.glyphs().map(|_| start));
                    if r.start as u32 >= pre {
                        clusters.push(Cluster {
                            start,
                            end: r.end as u32 - pre,
                            x: x / s,
                            advance: c.advance() / s,
                            rtl: c.is_rtl(),
                        });
                    }
                    x += c.advance();
                }
                for (i, g) in gr.positioned_glyphs().enumerate() {
                    glyphs.push(PlacedGlyph {
                        run: idx as u16,
                        id: g.id as u16,
                        x: g.x,
                        y: g.y,
                        byte: bytes.get(i).copied().unwrap_or(0),
                    });
                }
            }
        }
        // Parley aligns lines inside the line-breaking width, so a wrapped right-to-left
        // paragraph starts `max_width - width` from the left. Blocks are positioned by the
        // widgets from their own width, so move the glyphs back to start at 0.
        let rtl = layout.is_rtl();
        let mut width = layout.width();
        if rtl {
            // Right to left: each line placed by its ink as drawn, not by parley's offsets,
            // whose trailing whitespace can be on the wrong side of a line that mixes
            // directions (a line ending in a space has it on its left; a line whose last
            // logical run is left to right can have it in the middle).
            let inks = line_inks(&layout, &src);
            let w = inks
                .iter()
                .flatten()
                .map(|(lo, hi)| hi - lo)
                .fold(0.0f32, f32::max);
            width = w;
            line_at.push((glyphs.len(), clusters.len()));
            for (i, ink) in inks.iter().enumerate() {
                let Some((lo, hi)) = *ink else { continue };
                let dx = match align {
                    Align::Start => w - hi,
                    Align::Center => (w - (hi - lo)) * 0.5 - lo,
                    Align::End => -lo,
                };
                let (g0, c0) = line_at[i];
                let (g1, c1) = line_at[i + 1];
                for g in &mut glyphs[g0..g1] {
                    g.x += dx;
                }
                for c in &mut clusters[c0..c1] {
                    c.x += dx / s;
                }
            }
        } else {
            let shift = layout
                .lines()
                .map(|l| l.metrics().offset)
                .fold(f32::INFINITY, f32::min);
            if shift.is_finite() && shift > 0.0 {
                for g in &mut glyphs {
                    g.x -= shift;
                }
                for c in &mut clusters {
                    c.x -= shift / s;
                }
            }
        }
        TextBlock {
            width: width / s,
            height: layout.height() / s,
            baseline: baseline.unwrap_or(style.size) / s,
            lines,
            rtl: layout.is_rtl(),
            scale: s,
            fonts,
            glyphs,
            clusters,
            text_len: text.len() as u32,
        }
    }

    fn raster(&mut self, f: &RunFont, glyph: u16, bin: u8) -> Option<GlyphEntry> {
        let key = GlyphKey {
            font: f.font.data.id(),
            index: f.font.index,
            glyph,
            size: f.size.to_bits(),
            bin,
            coords: f.coords_hash,
        };
        if let Some(e) = self.glyphs.get(&key) {
            return *e;
        }
        let font = swash::FontRef::from_index(f.font.data.data(), f.font.index as usize)?;
        let mut scaler = self
            .scx
            .builder_with_id(font, [f.font.data.id(), f.font.index as u64])
            .size(f.size)
            .hint(false)
            .normalized_coords(f.coords.iter())
            .build();
        let img = Render::new(&[
            Source::ColorOutline(0),
            Source::ColorBitmap(StrikeWith::BestFit),
            Source::Outline,
        ])
        .format(Format::Alpha)
        .offset(Vector::new(bin as f32 / SUBPIXEL_BINS, 0.0))
        .render(&mut scaler, glyph);
        let entry = match img {
            Some(img) if img.placement.width > 0 && img.placement.height > 0 => {
                let (w, h) = (img.placement.width, img.placement.height);
                let pos = match self.atlas.alloc(w, h) {
                    Some(p) => p,
                    None => {
                        // Full: start over. Glyphs already placed this frame are stale, so
                        // ask for another frame.
                        self.atlas.reset();
                        self.glyphs.clear();
                        self.needs_redraw = true;
                        self.atlas.alloc(w, h)?
                    }
                };
                let data = if img.data.len() == (w * h) as usize {
                    Cow::Borrowed(&img.data[..])
                } else {
                    // Colour formats come back as RGBA; keep the alpha.
                    Cow::Owned(img.data.chunks_exact(4).map(|p| p[3]).collect::<Vec<u8>>())
                };
                self.atlas.write(pos.0, pos.1, w, h, &data);
                Some(GlyphEntry {
                    x: pos.0,
                    y: pos.1,
                    w,
                    h,
                    left: img.placement.left,
                    top: img.placement.top,
                })
            }
            _ => None,
        };
        self.glyphs.insert(key, entry);
        entry
    }

    /// Puts the marks `dl` draws ([`DrawList::mark`]) in the atlas, rasterising those not
    /// there yet: each mark primitive's index and its atlas position (the primitive's `uv`;
    /// `None` when it does not fit even in an empty atlas). Call before the atlas is uploaded.
    pub fn place_marks(&mut self, dl: &DrawList) -> Vec<(usize, Option<[f32; 4]>)> {
        let mut out = Vec::with_capacity(dl.marks.len());
        for m in &dl.marks {
            let key = (m.mark, m.k.to_bits());
            let pos = match self.marks.get(&key) {
                Some(&(generation, pos)) if generation == self.atlas.generation => Some(pos),
                _ => {
                    let pos = match self.atlas.alloc(m.w, m.h) {
                        Some(p) => Some(p),
                        None => {
                            // Full: start over, as for glyphs; this frame's text is stale.
                            self.atlas.reset();
                            self.glyphs.clear();
                            self.needs_redraw = true;
                            self.atlas.alloc(m.w, m.h)
                        }
                    };
                    if let Some(p) = pos {
                        let data = m.mark.raster(m.k, m.w, m.h);
                        self.atlas.write(p.0, p.1, m.w, m.h, &data);
                        self.marks.insert(key, (self.atlas.generation, p));
                    }
                    pos
                }
            };
            let uv = pos.map(|(x, y)| [x as f32, y as f32, m.w as f32, m.h as f32]);
            out.push((m.prim as usize, uv));
        }
        out
    }

    /// Draws `block` with its top-left at logical (`x`, `y`).
    pub fn draw(&mut self, dl: &mut DrawList, block: &TextBlock, x: f32, y: f32, color: Color) {
        self.draw_colored(dl, block, x, y, &mut |_| color);
    }

    /// Draws `block` with each glyph in the colour `color_at` gives for the byte its cluster
    /// starts at (syntax colouring).
    pub fn draw_colored(
        &mut self,
        dl: &mut DrawList,
        block: &TextBlock,
        x: f32,
        y: f32,
        color_at: &mut dyn FnMut(usize) -> Color,
    ) {
        let s = block.scale;
        let ox = x * s;
        let oy = (y * s).round();
        let mut last: Option<(Color, [f32; 4], f32)> = None;
        for g in &block.glyphs {
            let color = color_at(g.byte as usize);
            let (lin, gamma) = match last {
                Some((c, lin, gamma)) if c == color => (lin, gamma),
                _ => {
                    let lin = color.to_linear_premul();
                    // Coverage exponent: dark text gets heavier, light text lighter, so that
                    // blending in linear light looks like the platform's text.
                    let gamma = 0.62 + (1.30 - 0.62) * color.luminance();
                    last = Some((color, lin, gamma));
                    (lin, gamma)
                }
            };
            let f = &block.fonts[g.run as usize];
            let gx = ox + g.x;
            let ix = gx.floor();
            let bin = (((gx - ix) * SUBPIXEL_BINS).floor() as u8).min(SUBPIXEL_BINS as u8 - 1);
            let gy = (oy + g.y).round();
            if let Some(e) = self.raster(f, g.id, bin) {
                dl.glyph(
                    ix + e.left as f32,
                    gy - e.top as f32,
                    e.w as f32,
                    e.h as f32,
                    [e.x as f32, e.y as f32],
                    lin,
                    gamma,
                );
            }
        }
    }

    /// The outlines of `block`'s glyphs with its top-left at logical (`x`, `y`), in logical
    /// px with y down: the text as vectors for exports (figures in SVG and PDF), with the
    /// shaping and glyph positions [`Text::draw`] uses. Glyphs without an outline (bitmap
    /// emoji) are left out.
    pub fn outlines(&mut self, block: &TextBlock, x: f32, y: f32) -> Vec<PathEl> {
        let s = block.scale;
        let mut out = Vec::new();
        for g in &block.glyphs {
            let f = &block.fonts[g.run as usize];
            let Some(font) = swash::FontRef::from_index(f.font.data.data(), f.font.index as usize)
            else {
                continue;
            };
            let mut scaler = self
                .scx
                .builder_with_id(font, [f.font.data.id(), f.font.index as u64])
                .size(f.size)
                .hint(false)
                .normalized_coords(f.coords.iter())
                .build();
            let Some(outline) = scaler.scale_outline(g.id) else {
                continue;
            };
            let (ox, oy) = (x * s + g.x, y * s + g.y);
            let p = |v: swash::zeno::Vector| [(ox + v.x) / s, (oy - v.y) / s];
            use swash::zeno::{Command, PathData};
            for c in outline.path().commands() {
                out.push(match c {
                    Command::MoveTo(a) => PathEl::Move(p(a)),
                    Command::LineTo(a) => PathEl::Line(p(a)),
                    Command::QuadTo(c, a) => PathEl::Quad(p(c), p(a)),
                    Command::CurveTo(c1, c2, a) => PathEl::Cubic(p(c1), p(c2), p(a)),
                    Command::Close => PathEl::Close,
                });
            }
        }
        out
    }

    /// The text of `text` laid out on one line, in visual (left-to-right) order, by cluster,
    /// without bidi controls. For tests of bidirectional text.
    pub fn visual_order(&mut self, text: &str) -> String {
        let mut b = self.lcx.ranged_builder(&mut self.fcx, text, 1.0, true);
        b.push_default(StyleProperty::FontFamily(parley::FontFamily::Source(
            Cow::Borrowed(self.family.as_str()),
        )));
        let mut layout = b.build(text);
        layout.break_all_lines(None);
        let mut parts = Vec::new();
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(gr) = item else {
                    continue;
                };
                let mut x = gr.offset();
                for c in gr.run().visual_clusters() {
                    parts.push((x, &text[c.text_range()]));
                    x += c.advance();
                }
            }
        }
        parts.sort_by(|a, b| a.0.total_cmp(&b.0));
        parts
            .iter()
            .flat_map(|p| p.1.chars())
            .filter(|c| {
                !matches!(
                    c,
                    '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}' | '\u{061C}'
                )
            })
            .collect()
    }

    /// Lays out and draws in one call; returns the block for measuring.
    pub fn draw_str(
        &mut self,
        dl: &mut DrawList,
        text: &str,
        style: TextStyle,
        x: f32,
        y: f32,
        color: Color,
    ) -> Arc<TextBlock> {
        let b = self.layout(text, style, None, Align::Start);
        self.draw(dl, &b, x, y, color);
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A chosen family draws what it has; the rest (and a family not installed) falls back
    /// to the interface's fonts, Hebrew included.
    #[test]
    fn a_chosen_family_with_fallback() {
        let mut t = Text::bundled_only();
        t.begin_frame(1.0);
        let names = t.family_names();
        assert!(names.iter().any(|n| n == fonts::UI_FAMILY), "{names:?}");
        let serif = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../formats/specular-font/fixtures/serif.otf"
        ))
        .unwrap();
        let fam = t.add_font(serif);
        assert_eq!(fam.len(), 1, "{fam:?}");
        assert!(t.family_names().contains(&fam[0]));
        let style = TextStyle::new(20.0);
        let ui = t.layout("Boa 12", style, None, Align::Start).width;
        let own = t
            .layout_family(&fam[0], "Boa 12", style, None, Align::Start)
            .width;
        assert!((own - ui).abs() > 1.0, "{own} vs {ui}");
        let none = t
            .layout_family("No Such Family", "Boa 12", style, None, Align::Start)
            .width;
        assert!((none - ui).abs() < 0.01);
        assert_eq!(
            t.layout_family("", "Boa 12", style, None, Align::Start)
                .width,
            ui
        );
        // Hebrew is not in it: the bundled Hebrew font draws it, as without a choice.
        let he = t.layout("שלום", style, None, Align::Start).width;
        let he_own = t
            .layout_family(&fam[0], "שלום", style, None, Align::Start)
            .width;
        assert!((he - he_own).abs() < 0.01 && he > 10.0);
    }

    /// Outlines sit where the glyphs are drawn: inside the block, moved by its position, and
    /// right-to-left text has them too.
    #[test]
    fn outlines_follow_the_layout() {
        let mut t = Text::bundled_only();
        t.begin_frame(2.0);
        for s in ["Hl 50 µm", "שלום 50"] {
            let b = t.layout(s, TextStyle::new(20.0), None, Align::Start);
            let at = |v: &[PathEl]| -> Vec<[f32; 2]> {
                v.iter()
                    .flat_map(|e| match *e {
                        PathEl::Move(a) | PathEl::Line(a) => vec![a],
                        PathEl::Quad(c, a) => vec![c, a],
                        PathEl::Cubic(c1, c2, a) => vec![c1, c2, a],
                        PathEl::Close => vec![],
                    })
                    .collect()
            };
            let o = t.outlines(&b, 0.0, 0.0);
            let pts = at(&o);
            assert!(pts.len() > 20, "{s}: {}", pts.len());
            for p in &pts {
                assert!(
                    p[0] >= -1.0 && p[0] <= b.width + 1.0,
                    "{s}: {p:?} {}",
                    b.width
                );
                assert!(
                    p[1] >= -1.0 && p[1] <= b.height + 1.0,
                    "{s}: {p:?} {}",
                    b.height
                );
            }
            let moved = at(&t.outlines(&b, 10.0, 5.0));
            for (a, m) in pts.iter().zip(&moved) {
                assert!((m[0] - a[0] - 10.0).abs() < 1e-3 && (m[1] - a[1] - 5.0).abs() < 1e-3);
            }
        }
    }

    #[test]
    fn latin_only_text() {
        assert!(latin_only("IMG_000123.jpg"));
        assert!(latin_only("8. října 2024, 14:05\u{202f}PM ★★★"));
        assert!(!latin_only("Фото.jpg"));
        assert!(!latin_only("写真.jpg"));
    }

    #[test]
    fn isolated_numbers_keep_their_order_in_rtl_text() {
        let mut t = Text::new();
        // Fluent's FSI isolates around each placeable do not keep "w × h" in order...
        let fluent = "\u{2068}17127\u{2069} × \u{2068}2000\u{2069} (\u{2068}34.3\u{2069} ميغابكسل)";
        assert!(t.visual_order(fluent).ends_with("2000 × 17127"));
        // ...an LRI around the whole value (what the info panel adds) does.
        let isolated = "\u{2066}\u{2068}17127\u{2069} × \u{2068}2000\u{2069}\u{2069} (\u{2066}\u{2068}34.3\u{2069}\u{2069} ميغابكسل)";
        assert!(t.visual_order(isolated).ends_with("17127 × 2000"));
        // What `specular_i18n::for_rtl` makes of "חלון ({ $width } × { $height })": the run
        // in one LRI keeps "1600 × 900"; Fluent's isolates alone read "900 × 1600".
        let run = "חלון (\u{2066}\u{2066}1600\u{2069} × \u{2066}900\u{2069}\u{2069})";
        assert!(t.visual_order(run).starts_with(")1600 × 900("));
        let bare = "חלון (\u{2066}1600\u{2069} × \u{2066}900\u{2069})";
        assert!(t.visual_order(bare).starts_with(")900 × 1600("));
    }

    /// A panel row "name — details" in Hebrew. Joined plainly, a value from the file reads
    /// backwards ("±0.2 100") and a Latin file name starts a left-to-right paragraph (it
    /// lands on the left, the details on its right). As `specular_i18n::join_parts` joins
    /// it (an RLM, each part in FSI…PDI) the name keeps its order and sits on the right.
    #[test]
    fn joined_row_parts_keep_their_order_in_rtl_text() {
        let mut t = Text::new();
        let (f, p, rlm) = ('\u{2068}', '\u{2069}', '\u{200F}');
        assert_eq!(t.visual_order("מרחק — 100 ±0.2"), "±0.2 100 — קחרמ");
        assert_eq!(
            t.visual_order(&format!("{rlm}{f}100 ±0.2{p} — {f}מרחק{p}")),
            "קחרמ — 100 ±0.2"
        );
        assert_eq!(
            t.visual_order("architecture.ifc — המודל שנפתח"),
            "architecture.ifc — חתפנש לדומה"
        );
        assert_eq!(
            t.visual_order(&format!("{rlm}{f}architecture.ifc{p} — {f}המודל שנפתח{p}")),
            "חתפנש לדומה — architecture.ifc"
        );
        assert_eq!(
            t.visual_order(&format!("{rlm}{f}Ø0.25 | A | B{p} — {f}מיקום{p}")),
            "םוקימ — Ø0.25 | A | B"
        );
    }

    /// The OCR list's key chip in Hebrew, "ודאות: { $percent }" with CLDR's "99%": the per
    /// cent sign stays on the right of the digits (as in the toolbar's "(13%)"), so the chip
    /// reads "ודאות:" then "99%" from the right. Arabic's percent (with CLDR's LRMs) too.
    #[test]
    fn a_percent_after_rtl_text_keeps_its_side() {
        let mut t = Text::new();
        let (l, p, lrm) = ('\u{2066}', '\u{2069}', '\u{200E}');
        assert_eq!(t.visual_order(&format!("ודאות: {l}99%{p}")), "99% :תואדו");
        let ar = t.visual_order(&format!("الثقة: {l}99{lrm}%{lrm}{p}"));
        assert!(ar.starts_with("99% :"), "{ar:?}");
    }

    /// A sign before a number in a right-to-left sentence: outside the number's isolate it
    /// takes the sentence's direction and lands on the right ("2.5−", "(3+)"); moved inside
    /// (what `specular_i18n::for_rtl` does) it stays on the left.
    #[test]
    fn signs_stay_with_their_numbers_in_rtl_text() {
        let mut t = Text::new();
        let (l, p) = ('\u{2066}', '\u{2069}');
        assert_eq!(t.visual_order(&format!("שינוי −{l}2.5{p}")), "2.5− יוניש");
        assert_eq!(t.visual_order(&format!("שינוי {l}−2.5{p}")), "−2.5 יוניש");
        // A unit in an isolate of its own lands on the left of the number; in the number's
        // isolate it follows it.
        assert_eq!(
            t.visual_order(&format!("ערך {l}−5.419e-11{p} {l}m{p}")),
            "m −5.419e-11 ךרע"
        );
        assert_eq!(
            t.visual_order(&format!("ערך {l}−5.419e-11 m{p}")),
            "−5.419e-11 m ךרע"
        );
        // Brackets are listed as written; the shaper draws them mirrored, "(+3)".
        assert_eq!(t.visual_order(&format!("עוד ({l}+3{p})")), ")+3( דוע");
        assert_eq!(
            t.visual_order(&format!("\u{200F}{l}± 0.5{p} · Tab סבולת")),
            "תלובס Tab · ± 0.5"
        );
    }

    /// A BCF comment in the Hebrew panel: joined right to left, an all-Latin comment reads
    /// author-first from the right; as `specular_i18n::join_labelled` joins it (an LRM, the
    /// parts isolated) it reads normally, and a Hebrew comment still reads right to left.
    #[test]
    fn a_latin_comment_reads_left_to_right_in_rtl_text() {
        let mut t = Text::new();
        let (f, p) = ('\u{2068}', '\u{2069}');
        assert_eq!(
            t.visual_order(&format!("\u{200F}{f}dana{p}: {f}Wall moved.{p}")),
            "Wall moved. :dana"
        );
        assert_eq!(
            t.visual_order(&format!("\u{200E}{f}dana{p}: {f}Wall moved.{p}")),
            "dana: Wall moved."
        );
        assert_eq!(
            t.visual_order(&format!("\u{200F}{f}dana{p}: {f}הקיר זז.{p}")),
            ".זז ריקה :dana"
        );
    }

    /// The annotation export's level line in Hebrew ("{ $level } · { $width } × { $height }")
    /// holds no letters: with no strong character the paragraph is left to right (parley's
    /// default, UAX #9 P3), so it reads "1 · 24,576 × 17,920" with Fluent's isolates alone
    /// and with the LRI that `specular_i18n::for_rtl` adds, and the block is not right to
    /// left. Drawn at the dialog's value box, nothing reverses it.
    #[test]
    fn a_line_of_only_numbers_reads_left_to_right() {
        let mut t = Text::new();
        let (f, l, p) = ('\u{2068}', '\u{2066}', '\u{2069}');
        for s in [
            format!("{f}1{p} · {f}24,576{p} × {f}17,920{p}"),
            format!("{l}{l}1{p} · {l}24,576{p} × {l}17,920{p}{p}"),
        ] {
            assert_eq!(t.visual_order(&s), "1 · 24,576 × 17,920", "{s:?}");
            let b = t.layout(&s, TextStyle::new(14.0), None, Align::Start);
            assert!(!b.rtl, "{s:?}");
        }
    }

    /// A wrapped right-to-left paragraph whose lines end in a space (on their left): the
    /// glyphs stay inside the block's width, so a block placed at `right - width` keeps
    /// within `right` (the Hebrew About box's attributions reached past it).
    #[test]
    fn wrapped_rtl_lines_stay_inside_the_block() {
        let mut t = Text::new();
        t.begin_frame(1.0);
        let s = "\u{200f}שמות מקומות: GeoNames (www.geonames.org), ברישיון Creative Commons ייחוס 4.0 (CC BY 4.0, https://creativecommons.org/licenses/by/4.0/). חיפוש לא מקוון באמצעות ספריית reverse_geocoder מאת Grant Miner (MIT או Apache-2.0). הנתונים הומרו לרשימת ערים; הם מסופקים כמות שהם, ללא אחריות.";
        let style = TextStyle::new(12.0);
        let max = 472.0;
        let block = t.layout(s, style, Some(max), Align::Start);
        assert!(block.rtl && block.lines > 2);
        // The same layout in parley: where each glyph ends, before the block's shift.
        let mut b = t.lcx.ranged_builder(&mut t.fcx, s, 1.0, true);
        b.push_default(StyleProperty::FontFamily(parley::FontFamily::Source(
            Cow::Borrowed(t.family.as_str()),
        )));
        b.push_default(StyleProperty::FontSize(style.size));
        b.push_default(StyleProperty::LineHeight(LineHeight::FontSizeRelative(
            style.line_height,
        )));
        let mut layout = b.build(s);
        layout.break_all_lines(Some(max));
        layout.align(Alignment::Start, AlignmentOptions::default());
        let mut first = None;
        let mut right = f32::MIN;
        let mut spaced = false;
        for line in layout.lines() {
            spaced |= line.metrics().trailing_whitespace > 0.0;
            for item in line.items() {
                if let PositionedLayoutItem::GlyphRun(gr) = item {
                    for g in gr.positioned_glyphs() {
                        first.get_or_insert(g.x);
                        right = right.max(g.x + g.advance);
                    }
                }
            }
        }
        assert!(spaced, "a line ends in a space");
        let shift = first.unwrap() - block.glyphs[0].x;
        assert!(
            right - shift <= block.width + 0.01,
            "{} > {}",
            right - shift,
            block.width
        );
        assert!(block.width <= max);
    }

    /// A word longer than the line overshoots it at any width: the rest still wraps at the
    /// full width, not one word per line.
    #[test]
    fn a_long_word_does_not_narrow_the_wrap() {
        let mut t = Text::bundled_only();
        t.begin_frame(1.0);
        let s = "Checksum 3f9a0c4e7b21d8f6a5c3e9b07d14f2a68c5e3b9d0f71a2c4e6b8d0f13579bdf2 \
                 matches the one saved earlier, with the rest of this sentence wrapping as usual.";
        let style = TextStyle::new(12.0);
        let one_line = t.layout(s, style, None, Align::Start).width;
        let block = t.layout(s, style, Some(200.0), Align::Start);
        assert!(block.width > 200.0, "the checksum overshoots");
        assert!(
            (block.lines as f32) < one_line / 200.0 + 3.0,
            "{} lines",
            block.lines
        );
    }

    /// Mixed Hebrew and Latin wrapped at a fixed width (Info values in a right-to-left
    /// layout): every line's ink stays inside the wrap width and the block's width.
    #[test]
    fn mixed_direction_lines_fit_the_wrap_width() {
        let mut bad = Vec::new();
        for (bundled, locale) in [(false, None), (true, Some("he")), (true, None)] {
            let mut t = if bundled {
                Text::bundled_only()
            } else {
                Text::new()
            };
            t.set_locale(locale.map(str::to_owned));
            for b in mixed_direction_fits(&mut t) {
                bad.push(format!("bundled {bundled} {locale:?}: {b}"));
            }
        }
        assert!(bad.is_empty(), "{bad:#?}");
    }

    fn mixed_direction_fits(t: &mut Text) -> Vec<String> {
        t.begin_frame(1.0);
        let (f, l, p) = ('\u{2068}', '\u{2066}', '\u{2069}');
        let texts = [
            "מודל פני קרקע: TIN surface Existing Ground (EG-2024), 1532 נקודות, 2987 משולשים"
                .to_owned(),
            format!(
                "מודל פני קרקע {f}Existing Ground Surface{p} · {f}1532{p} נקודות · {f}2987{p} משולשים"
            ),
            "Surface EG-2024 מודל פני קרקע עם 1532 נקודות and 2987 triangles in total".to_owned(),
            format!("{f}Existing ground surface from the survey of March 2024{p} (מודל פני קרקע)"),
            "\u{200f}LandXML 1.2 · Civil 3D 2024 · מודל פני קרקע · Existing Ground".to_owned(),
            // As the Info panel isolates left-to-right stretches with digits in RTL layouts.
            format!(
                "\u{200f}מודל פני קרקע {l}Existing Ground EG-2024 with 1532 points and 2987 triangles{p}"
            ),
            format!(
                "{l}TIN EG-2024, 1532 points{p} מודל פני קרקע {l}2987 triangles, 0.5 m grid{p} סוף"
            ),
            // The Info panel's survey surface in Hebrew, after `ltr_values`.
            format!(
                "{l}{l}Existing ground{p}: {l}12,480{p}{p} נקודות{l}, {l}24,612{p}{p} משולשים; שטח בהיטל {l}{l}182,340.5{p} {l}m\u{b2}{p},{p} שטח פנים {l}{l}184,101.2{p} {l}m\u{b2}{p}; {l}1,203,310{p} {l}m\u{b3}{p}{p} מעל הנקודה הנמוכה ביותר; גבהים {l}{l}{l}312.05{p}\u{2013}{l}341.88{p}{p} {l}m{p};{p} קווי גובה כל {l}{l}2{p} {l}m{p}{p}"
            ),
        ];
        let mut bad = Vec::new();
        for (n, s) in texts.iter().enumerate() {
            for max in [90.0f32, 140.0, 174.0, 188.0, 196.0, 210.0] {
                for scale in [1.0f32, 2.0] {
                    t.begin_frame(scale);
                    let b = t.layout(
                        s,
                        TextStyle::new(13.0).break_anywhere(),
                        Some(max),
                        Align::Start,
                    );
                    // Ink: clusters with something to see (spaces at a line's end are
                    // outside the line's width by design, and draw nothing).
                    let inked = b.clusters.iter().filter(|c| {
                        s.get(c.start as usize..c.end as usize).is_some_and(|t| {
                            t.chars().any(|ch| {
                                !ch.is_whitespace()
                                    && !ch.is_control()
                                    && !('\u{2066}'..='\u{2069}').contains(&ch)
                                    && ch != '\u{200f}'
                            })
                        })
                    });
                    let (lo, hi) = inked.fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), c| {
                        (lo.min(c.x), hi.max(c.x + c.advance))
                    });
                    if b.width > max + 0.01 || lo < -0.01 || hi > b.width + 0.01 {
                        bad.push(format!(
                            "text {n} at {max} (x{scale}): width {} ink {lo}..{hi}",
                            b.width
                        ));
                    }
                }
            }
        }
        bad
    }

    /// Carets and hits in code: positions grow with the byte offset (also through Hebrew,
    /// which code keeps left to right), and a hit at a caret returns its byte.
    #[test]
    fn code_carets_are_left_to_right() {
        let mut t = Text::bundled_only();
        t.begin_frame(1.5);
        let style = TextStyle::new(12.0).code();
        // Hebrew keeps its own order inside the line, but the line reads left to right.
        let s = "title = \"שלום עולם\" # x";
        let b = t.layout(s, style, None, Align::Start);
        assert!(!b.rtl);
        let x = |c: char| b.caret_x(s.find(c).unwrap());
        assert!(x('=') < x('ש') && x('ש') < x('#'));
        // The first Hebrew letter is on the right of its word.
        assert!(x('ש') > x('ם'));
        assert_eq!(b.hit_byte(x('ש') - 0.1), s.find('ש').unwrap());
        for s in ["key = \"value\" # x", ""] {
            let b = t.layout(s, style, None, Align::Start);
            assert!(!b.rtl, "{s}");
            assert_eq!(b.caret_x(0), 0.0);
            let mut prev = -1.0;
            for (i, _) in s.char_indices().chain([(s.len(), ' ')]) {
                let x = b.caret_x(i);
                assert!(x > prev, "{s:?} at {i}: {x} <= {prev}");
                prev = x;
                assert_eq!(b.hit_byte(x + 0.1), i, "{s:?} at {i}");
            }
            assert!((b.caret_x(s.len()) - b.width).abs() < 0.5 || s.is_empty());
            // Colours by byte: the glyph of '=' is asked for with its offset.
            let mut asked = Vec::new();
            let mut dl = DrawList::default();
            t.draw_colored(&mut dl, &b, 0.0, 0.0, &mut |i| {
                asked.push(i);
                Color::WHITE
            });
            if let Some(eq) = s.find('=') {
                assert!(asked.contains(&eq), "{s:?}: {asked:?}");
            }
        }
    }

    /// Selection spans: one for a run of one direction, two where a selection crosses into
    /// text of the other direction, and none for an empty range.
    #[test]
    fn range_spans_follow_the_text_on_screen() {
        let mut t = Text::bundled_only();
        t.begin_frame(1.0);
        let style = TextStyle::new(14.0);
        let s = "abc \u{5d0}\u{5d1}\u{5d2} def";
        let b = t.layout(s, style, None, Align::Start);
        assert!(!b.rtl);
        assert!(b.range_spans(2..2).is_empty());
        let whole = b.range_spans(0..s.len());
        assert_eq!(whole.len(), 1);
        assert!(whole[0].0.abs() < 0.5 && (whole[0].1 - b.width).abs() < 0.5);
        // "c א": "c " on the left of the Hebrew word, "א" at its right end.
        let alef = s.find('\u{5d0}').unwrap();
        let spans = b.range_spans(2..alef + 2);
        assert_eq!(spans.len(), 2, "{spans:?}");
        assert!((spans[0].0 - b.caret_x(2)).abs() < 0.5);
        assert!(spans[1].1 > spans[0].1 && spans[0].1 < spans[1].0);
        // Within the Hebrew word: one span, between its carets.
        let gimel = s.find('\u{5d2}').unwrap();
        let spans = b.range_spans(alef..gimel);
        assert_eq!(spans.len(), 1);
        let (x0, x1) = (b.caret_x(alef), b.caret_x(gimel));
        assert!((spans[0].0 - x0.min(x1)).abs() < 0.5 && (spans[0].1 - x0.max(x1)).abs() < 0.5);
        // A right-to-left paragraph.
        let s = "\u{5e9}\u{5dc}\u{5d5}\u{5dd} abc";
        let b = t.layout(s, style, None, Align::Start);
        assert!(b.rtl);
        let spans = b.range_spans(0..s.find(' ').unwrap());
        assert_eq!(spans.len(), 1);
        assert!((spans[0].1 - b.width).abs() < 0.5, "{spans:?} {}", b.width);
    }

    /// Carets in right-to-left and mixed text follow the letters on screen: in Hebrew they
    /// move right to left as the byte grows, the caret after the last letter is on its left
    /// (where the next one goes), digits and Latin inside keep their own order, and a hit at
    /// each caret gives its byte back.
    #[test]
    fn rtl_carets_follow_the_letters() {
        let mut t = Text::bundled_only();
        t.begin_frame(1.5);
        let style = TextStyle::new(14.0);
        let he = "\u{5e9}\u{5dc}\u{5d5}\u{5dd}";
        let b = t.layout(he, style, None, Align::Start);
        assert!(b.rtl);
        let mut prev = f32::INFINITY;
        for (i, _) in he.char_indices().chain([(he.len(), ' ')]) {
            let x = b.caret_x(i);
            assert!(x < prev, "{i}: {x} >= {prev}");
            prev = x;
            assert_eq!(b.hit_byte(x), i, "hit at {i}");
        }
        assert!((b.caret_x(0) - b.width).abs() < 0.5);
        assert!(b.caret_x(he.len()).abs() < 0.5, "end on the left");
        // Hebrew with a Latin word and digits: the end is after "12", on the right of "2",
        // which is left of the Hebrew word.
        let s = "\u{5e9}\u{5dc}\u{5d5}\u{5dd} abc 12";
        let b = t.layout(s, style, None, Align::Start);
        assert!(b.rtl);
        let x = |c: char| b.caret_x(s.find(c).unwrap());
        assert!(x('a') < x('b') && x('b') < x('c'));
        assert!(x('1') < x('2') && x('2') < b.caret_x(s.len()));
        assert!(b.caret_x(s.len()) < x('\u{5dd}'));
        // Latin text with a Hebrew word at its end: the caret after it is on the word's left.
        let s = "abc \u{5d0}\u{5d1}";
        let b = t.layout(s, style, None, Align::Start);
        assert!(!b.rtl);
        assert!(b.caret_x(s.len()) < b.caret_x(s.find('\u{5d1}').unwrap()));
        assert!(b.caret_x(s.len()) > b.caret_x(3));
        // Empty: at 0.
        let b = t.layout("", style, None, Align::Start);
        assert_eq!(b.caret_x(0), 0.0);
    }
}
