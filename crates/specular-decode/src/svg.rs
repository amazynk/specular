//! SVG / SVGZ via usvg + resvg (pure Rust).
//!
//! Vector images have no pixel size, so the session picks one: the "full" (scale 1) raster
//! is the intrinsic size × k, with k a power of two chosen so the long side is at least
//! [`MIN_LONG_SIDE`] px (small icons become crisp at normal zoom), capped at [`MAX_SIDE`] px per
//! side and [`MAX_PIXELS`] (a fractional k for huge drawings). Scales 1/2/4/8 and regions are
//! rendered directly, never resampled. The background is transparent (the app draws its
//! checkerboard).
//!
//! **Text** is shaped by usvg with the system fonts plus the bundled UI fonts as a fallback.
//! The font database is built once, on the first SVG that has text (on the decode thread that
//! opens it), and shared; SVGs without text never load it.
//!
//! **Embedded images** (`<image>`): `data:` URIs, and relative references to files in the SVG's
//! own folder (or below it), are decoded by Specular's decoders through the vendored resvg's
//! raster hook (`third_party/resvg`), once per session. Network URLs, `file:` URLs, absolute
//! paths and references that leave the folder (`..`, symlinks) are refused.
//!
//! **CSS** that usvg does not handle is rewritten first (`svg_css`): custom properties
//! (`var(--x)`) and Tailwind's `fill-current` / `stroke-current`.

use crate::{
    DecodeError, DecodeRequest, Decoder, Format, ImageInfo, Registry, Result, Session, svg_css,
};
use parking_lot::Mutex;
use resvg::usvg::fontdb;
use resvg::{tiny_skia, usvg};
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, RasterPlane, Source};
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, OnceLock};

pub const MIN_LONG_SIDE: f32 = 1024.0;
pub const MAX_SIDE: f32 = 16384.0;
pub const MAX_PIXELS: f32 = 64e6;

/// Largest decompressed SVGZ.
const MAX_SVGZ: usize = 512 << 20;
/// Largest file an `<image>` may reference.
const MAX_IMAGE_FILE: u64 = 256 << 20;
/// Largest embedded raster, in pixels.
const MAX_IMAGE_PIXELS: u64 = 64_000_000;

pub struct SvgDecoder;

fn skip_ws(head: &[u8]) -> &[u8] {
    let head = head.strip_prefix(b"\xef\xbb\xbf").unwrap_or(head);
    let n = head.iter().take_while(|b| b.is_ascii_whitespace()).count();
    &head[n..]
}

fn is_gzip(data: &[u8]) -> bool {
    data.starts_with(&[0x1f, 0x8b])
}

impl Decoder for SvgDecoder {
    fn format(&self) -> Format {
        Format::Svg
    }
    fn probe(&self, head: &[u8]) -> bool {
        let h = skip_ws(head);
        // An XML prolog or comment may come first; `open` rejects XML that is not SVG. SVGZ
        // (gzip) is recognised by extension only, as gzip is not specific to SVG.
        h.starts_with(b"<svg")
            || h.starts_with(b"<?xml") && h.windows(4).any(|w| w == b"<svg")
            || h.starts_with(b"<!DOCTYPE svg")
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["svg", "svgz"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        open_svg(source, FontSet::System)
    }
}

/// Which fonts text is shaped with.
#[derive(Clone, Copy, PartialEq, Debug)]
enum FontSet {
    /// The system's fonts, then the bundled ones.
    System,
    /// The bundled fonts only (deterministic renders for the tests).
    #[cfg_attr(not(test), allow(dead_code))]
    BundledOnly,
}

/// A font database with its generic families set.
struct Fonts {
    db: Arc<fontdb::Database>,
    /// The default family (`serif`, as in browsers).
    serif: String,
}

fn fonts(set: FontSet) -> &'static Fonts {
    static SYSTEM: OnceLock<Fonts> = OnceLock::new();
    static BUNDLED: OnceLock<Fonts> = OnceLock::new();
    let cell = match set {
        FontSet::System => &SYSTEM,
        FontSet::BundledOnly => &BUNDLED,
    };
    cell.get_or_init(|| load_fonts(set))
}

fn load_fonts(set: FontSet) -> Fonts {
    let t0 = std::time::Instant::now();
    let mut db = fontdb::Database::new();
    if set == FontSet::System {
        db.load_system_fonts();
    }
    let system_faces = db.len();
    let mut bundled = None;
    for data in specular_theme::fonts::all() {
        let ids = db.load_font_source(fontdb::Source::Binary(Arc::new(data)));
        if bundled.is_none() {
            bundled = ids
                .first()
                .and_then(|&id| db.face(id))
                .and_then(|f| f.families.first())
                .map(|(name, _)| name.clone());
        }
    }
    let bundled = bundled.unwrap_or_else(|| specular_theme::fonts::UI_FAMILY.to_owned());
    let present: HashSet<&str> = db
        .faces()
        .flat_map(|f| f.families.iter().map(|(n, _)| n.as_str()))
        .collect();
    let pick = |names: &[&str], or: &str| {
        names
            .iter()
            .find(|n| present.contains(*n))
            .map_or_else(|| or.to_owned(), |n| (*n).to_owned())
    };
    let sans = pick(
        &[
            "Arial",
            "Helvetica",
            "Liberation Sans",
            "Arimo",
            "DejaVu Sans",
            "Noto Sans",
            "FreeSans",
        ],
        &bundled,
    );
    let serif = pick(
        &[
            "Times New Roman",
            "Times",
            "Liberation Serif",
            "Tinos",
            "DejaVu Serif",
            "Noto Serif",
            "FreeSerif",
            "Georgia",
        ],
        &sans,
    );
    let mono = pick(
        &[
            "Courier New",
            "Courier",
            "Liberation Mono",
            "Cousine",
            "DejaVu Sans Mono",
            "Noto Sans Mono",
            "FreeMono",
        ],
        &sans,
    );
    db.set_serif_family(serif.clone());
    db.set_sans_serif_family(sans.clone());
    db.set_monospace_family(mono.clone());
    db.set_cursive_family(sans.clone());
    db.set_fantasy_family(sans.clone());
    specular_log::debug!(
        target: specular_log::DECODE,
        "SVG fonts: {system_faces} system faces + bundled, serif {serif:?}, sans {sans:?}, mono {mono:?}, {:.1} ms",
        t0.elapsed().as_secs_f64() * 1e3
    );
    Fonts {
        db: Arc::new(db),
        serif,
    }
}

/// Whether the document may draw text: a `text` element, or an embedded SVG (which may).
fn needs_fonts(xml: &[u8]) -> bool {
    let text = memchr::memmem::find_iter(xml, b"text").any(|at| {
        at > 0
            && matches!(xml[at - 1], b'<' | b':')
            && xml
                .get(at + 4)
                .is_some_and(|&c| c.is_ascii_whitespace() || c == b'>' || c == b'/')
    });
    text || memchr::memmem::find(xml, b"svg+xml").is_some()
        || memchr::memmem::find(xml, b"<image").is_some()
            && memchr::memmem::find(xml, b".svg").is_some()
}

pub(crate) fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(Registry::default)
}

/// An SVG document (a Krita vector layer) drawn on a `w`×`h` canvas, its own size stretched
/// over it: premultiplied RGBA8. Only `data:` images are read (no files), text with the
/// system fonts. `None` when it cannot be read.
pub(crate) fn render_canvas(svg: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    resvg::set_raster_decoder(decode_raster);
    if !specular_limits::xml_within_depth(svg, specular_limits::MAX_XML_DEPTH) {
        return None;
    }
    let mut opt = usvg::Options::default();
    if needs_fonts(svg) {
        let f = fonts(FontSet::System);
        opt.fontdb = f.db.clone();
        opt.font_family = f.serif.clone();
    }
    opt.image_href_resolver = usvg::ImageHrefResolver {
        resolve_data: Box::new(resolve_data),
        resolve_string: Box::new(|_, _| None),
    };
    let tree = usvg::Tree::from_data(svg, &opt).ok()?;
    let size = tree.size();
    if !(size.width() > 0.0 && size.height() > 0.0) {
        return None;
    }
    let mut pm = tiny_skia::Pixmap::new(w, h)?;
    let t = tiny_skia::Transform::from_scale(w as f32 / size.width(), h as f32 / size.height());
    resvg::render(&tree, t, &mut pm.as_mut());
    Some(pm.take())
}

fn open_svg(source: Arc<Source>, font_set: FontSet) -> Result<Arc<dyn Session>> {
    resvg::set_raster_decoder(decode_raster);
    let bytes = source.bytes();
    let mut xml: Cow<[u8]> = Cow::Borrowed(bytes);
    if is_gzip(bytes) {
        let data = specular_png::inflate_gzip(bytes, MAX_SVGZ)
            .map_err(|e| DecodeError::Malformed(format!("SVGZ: {e}")))?;
        xml = Cow::Owned(data);
    }
    let mut notes = Vec::new();
    if memchr::memmem::find(&xml, b"var(").is_some()
        && let Some(css) = std::str::from_utf8(&xml)
            .ok()
            .and_then(svg_css::resolve_vars)
    {
        xml = Cow::Owned(css.into_bytes());
        notes.push("CSS variables");
    }

    // roxmltree (usvg's parser) recurses once per element level, with no limit of its own.
    if !specular_limits::xml_within_depth(&xml, specular_limits::MAX_XML_DEPTH) {
        return Err(DecodeError::Malformed("elements nested too deeply".into()));
    }
    let mut opt = usvg::Options::default();
    if needs_fonts(&xml) {
        let f = fonts(font_set);
        opt.fontdb = f.db.clone();
        opt.font_family = f.serif.clone();
        notes.push("text");
    }
    if svg_css::uses_utilities(&xml) {
        opt.style_sheet = Some(svg_css::UTILITY_SHEET.to_owned());
    }
    let dir = source
        .path()
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf);
    // `@font-face` fonts (data: URLs or files beside the SVG), which usvg ignores.
    if notes.contains(&"text") {
        let faces = crate::svg_fonts::faces(&xml, dir.as_deref());
        if !faces.is_empty() {
            let (db, names) = crate::svg_fonts::database(&opt.fontdb, faces);
            opt.fontdb = db;
            opt.font_resolver.select_font = crate::svg_fonts::selector(names);
            notes.push("embedded fonts");
        }
    }
    opt.image_href_resolver = usvg::ImageHrefResolver {
        resolve_data: Box::new(resolve_data),
        resolve_string: Box::new(move |href, opts| resolve_file(href, dir.as_deref(), opts)),
    };
    let tree =
        usvg::Tree::from_data(&xml, &opt).map_err(|e| DecodeError::Malformed(e.to_string()))?;
    drop(opt);
    let size = tree.size();
    let (iw, ih) = (size.width(), size.height());
    if !(iw > 0.0 && ih > 0.0 && iw.is_finite() && ih.is_finite()) {
        return Err(DecodeError::Malformed("SVG without a size".into()));
    }
    let mut k = 1.0f32;
    while iw.max(ih) * k < MIN_LONG_SIDE {
        k *= 2.0;
    }
    k = k
        .min(MAX_SIDE / iw.max(ih))
        .min((MAX_PIXELS / (iw * ih)).sqrt());
    let (w, h) = (
        ((iw * k).round() as u32).max(1),
        ((ih * k).round() as u32).max(1),
    );
    let notes = notes.iter().map(|n| format!(", {n}")).collect::<String>();
    let info = ImageInfo {
        format: Format::Svg,
        width: w,
        height: h,
        orientation: Orientation::NORMAL,
        icc_profile: None,
        has_alpha: true,
        bit_depth: 8,
        progressive: false,
        coding: format!("vector {iw:.0}×{ih:.0}, rendered ×{k:.3}{notes}"),
        embedded_preview: None,
        fast_regions: true,
        scales: &[1, 2, 4, 8],
    };
    Ok(Arc::new(SvgSession {
        source,
        info,
        tree,
        rasters: Arc::default(),
    }))
}

/// A `data:` URI's content.
fn resolve_data(mime: &str, data: Arc<Vec<u8>>, opts: &usvg::Options) -> Option<usvg::ImageKind> {
    if mime == "image/svg+xml" {
        return nested_svg(&data, opts);
    }
    classify(data, opts)
}

/// Raster formats resvg is told about (their size is read from the header); everything else
/// our decoders read is decoded now and handed over as a PNG.
fn classify(data: Arc<Vec<u8>>, opts: &usvg::Options) -> Option<usvg::ImageKind> {
    match registry().probe(&data).map(|d| d.format()) {
        Some(Format::Jpeg) => Some(usvg::ImageKind::JPEG(data)),
        Some(Format::Png) => Some(usvg::ImageKind::PNG(data)),
        Some(Format::Gif) => Some(usvg::ImageKind::GIF(data)),
        Some(Format::WebP) => Some(usvg::ImageKind::WEBP(data)),
        Some(Format::Svg) => nested_svg(&data, opts),
        _ if is_gzip(&data) => nested_svg(&data, opts),
        _ => {
            let (w, h, rgba) = decode_rgba(&data)?;
            let png = specular_png::encode(w, h, specular_png::ColorType::Rgba, &rgba).ok()?;
            Some(usvg::ImageKind::PNG(Arc::new(png)))
        }
    }
}

/// How deep SVG `data:` images may nest in each other. usvg parses each one from inside the
/// parse of the one around it, on this thread, without a limit of its own: gzip'd levels cost
/// about 100 bytes each, enough for a small file to overflow the stack.
const MAX_NESTED_SVG: u32 = 4;
/// A nested SVGZ image inflates to at most this much.
const MAX_NESTED_SVGZ: usize = 64 << 20;

thread_local! {
    /// SVG images being parsed on this thread, inside each other.
    static NESTED_SVG: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

fn nested_svg(data: &[u8], opts: &usvg::Options) -> Option<usvg::ImageKind> {
    let depth = NESTED_SVG.with(|d| d.get());
    if depth >= MAX_NESTED_SVG {
        specular_log::debug!(target: specular_log::DECODE, "SVG image not loaded: SVG images nested too deeply");
        return None;
    }
    struct Leave;
    impl Drop for Leave {
        fn drop(&mut self) {
            NESTED_SVG.with(|d| d.set(d.get() - 1));
        }
    }
    let xml: Cow<[u8]> = if is_gzip(data) {
        Cow::Owned(specular_png::inflate_gzip(data, MAX_NESTED_SVGZ).ok()?)
    } else {
        Cow::Borrowed(data)
    };
    if !specular_limits::xml_within_depth(&xml, specular_limits::MAX_XML_DEPTH) {
        specular_log::debug!(target: specular_log::DECODE, "SVG image not loaded: elements nested too deeply");
        return None;
    }
    NESTED_SVG.with(|d| d.set(depth + 1));
    let _leave = Leave;
    // External files are not read from nested SVGs (usvg's rule): no escape through them.
    usvg::Tree::from_data_nested(&xml, opts)
        .ok()
        .map(usvg::ImageKind::SVG)
}

/// A non-`data:` reference: a file in the SVG's folder, or nothing.
fn resolve_file(href: &str, dir: Option<&Path>, opts: &usvg::Options) -> Option<usvg::ImageKind> {
    let refuse = |why: &str| {
        specular_log::debug!(target: specular_log::DECODE, "SVG image {href:?} not loaded: {why}");
        None
    };
    let Some(rel) = local_reference(href) else {
        return refuse("not a relative path inside the SVG's folder");
    };
    let Some(dir) = dir.and_then(|d| d.canonicalize().ok()) else {
        return refuse("the SVG has no folder");
    };
    let Ok(path) = dir.join(&rel).canonicalize() else {
        return refuse("no such file");
    };
    if !path.starts_with(&dir) {
        return refuse("outside the SVG's folder");
    }
    match std::fs::metadata(&path) {
        Ok(m) if m.is_file() && m.len() <= MAX_IMAGE_FILE => {}
        _ => return refuse("not a regular file, or too large"),
    }
    let Ok(data) = std::fs::read(&path) else {
        return refuse("unreadable");
    };
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    if matches!(ext.as_deref(), Some("svg" | "svgz")) {
        return nested_svg(&data, opts);
    }
    classify(Arc::new(data), opts)
}

/// The relative path an `href` names, when it is one that cannot leave the folder: no URL
/// scheme, not absolute, no `..`.
fn local_reference(href: &str) -> Option<PathBuf> {
    let h = href.trim();
    if let Some(colon) = h.find(':') {
        let scheme = &h[..colon];
        if scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        {
            return None;
        }
    }
    if h.starts_with("//") || h.starts_with('\\') {
        return None;
    }
    let h = h.split(['?', '#']).next()?;
    let path = PathBuf::from(percent_decode(h)?);
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return None;
    }
    Some(path)
}

fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    if out.contains(&0) {
        return None;
    }
    String::from_utf8(out).ok()
}

/// Decodes an embedded raster with our decoders: straight-alpha RGBA8.
pub(crate) fn decode_rgba(data: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let session = registry()
        .open(Arc::new(Source::from_vec("svg-image", data.to_vec())))
        .ok()?;
    let info = session.info();
    if info.width as u64 * info.height as u64 > MAX_IMAGE_PIXELS {
        specular_log::debug!(target: specular_log::DECODE, "SVG image of {}×{} px not loaded: too large", info.width, info.height);
        return None;
    }
    let r = session
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .ok()?;
    to_rgba8(r)
}

/// A raster as straight-alpha RGBA8 rows: width, height, bytes.
pub(crate) fn to_rgba8(r: Raster) -> Option<(u32, u32, Vec<u8>)> {
    let (w, h) = (r.width as usize, r.height as usize);
    let rows = |bpp: usize, f: &mut dyn FnMut(&[u8], &mut [u8])| -> Option<Vec<u8>> {
        let pl = r.planes.first()?;
        let mut out = vec![0u8; w * h * 4];
        for (y, row) in out.chunks_exact_mut(w * 4).enumerate() {
            let src = pl.data.get(y * pl.stride..y * pl.stride + w * bpp)?;
            for (s, d) in src.chunks_exact(bpp).zip(row.chunks_exact_mut(4)) {
                f(s, d);
            }
        }
        Some(out)
    };
    let hi = |s: &[u8], i: usize| (u16::from_ne_bytes([s[2 * i], s[2 * i + 1]]) >> 8) as u8;
    let out = match r.layout {
        PixelLayout::Rgba => rows(4, &mut |s, d| d.copy_from_slice(s))?,
        PixelLayout::Gray | PixelLayout::Rgb | PixelLayout::YCbCr => {
            crate::gainmap::to_rgba8(&r).planes.swap_remove(0).data
        }
        PixelLayout::Rgba16(e) if matches!(e.transfer, specular_core::Transfer::Srgb) => {
            rows(8, &mut |s, d| {
                for (i, c) in d.iter_mut().enumerate() {
                    *c = hi(s, i);
                }
            })?
        }
        PixelLayout::Gray16 => rows(2, &mut |s, d| {
            let v = hi(s, 0);
            d.copy_from_slice(&[v, v, v, 255]);
        })?,
        PixelLayout::GrayAlpha16 => rows(4, &mut |s, d| {
            let v = hi(s, 0);
            d.copy_from_slice(&[v, v, v, hi(s, 1)]);
        })?,
        other => {
            specular_log::debug!(target: specular_log::DECODE, "SVG image in {other:?} not drawn");
            return None;
        }
    };
    Some((r.width, r.height, out))
}

/// Decoded embedded rasters of one session, keyed by the address of their (immutable,
/// tree-owned) encoded bytes.
#[derive(Default)]
struct RasterCache {
    entries: Mutex<HashMap<usize, Arc<RasterSlot>>>,
}

/// One embedded raster: decoded by the first render that needs it (`None`: undecodable).
type RasterSlot = OnceLock<Option<Arc<tiny_skia::Pixmap>>>;

thread_local! {
    /// The cache of the session rendering on this thread (resvg renders on the caller's).
    static RENDERING: RefCell<Option<Arc<RasterCache>>> = const { RefCell::new(None) };
}

/// Restores the previous [`RENDERING`] cache when a render ends (or unwinds).
struct Rendering(Option<Arc<RasterCache>>);

impl Rendering {
    fn enter(cache: &Arc<RasterCache>) -> Rendering {
        Rendering(RENDERING.with(|c| c.replace(Some(cache.clone()))))
    }
}

impl Drop for Rendering {
    fn drop(&mut self) {
        let prev = self.0.take();
        RENDERING.with(|c| *c.borrow_mut() = prev);
    }
}

/// resvg's raster hook.
fn decode_raster(kind: &usvg::ImageKind) -> Option<Arc<tiny_skia::Pixmap>> {
    let data = match kind {
        usvg::ImageKind::JPEG(d)
        | usvg::ImageKind::PNG(d)
        | usvg::ImageKind::GIF(d)
        | usvg::ImageKind::WEBP(d) => d,
        usvg::ImageKind::SVG(_) => return None,
    };
    let Some(cache) = RENDERING.with(|c| c.borrow().clone()) else {
        return decode_pixmap(data);
    };
    let cell = cache
        .entries
        .lock()
        .entry(Arc::as_ptr(data) as usize)
        .or_default()
        .clone();
    cell.get_or_init(|| decode_pixmap(data)).clone()
}

fn decode_pixmap(data: &[u8]) -> Option<Arc<tiny_skia::Pixmap>> {
    let (w, h, mut rgba) = decode_rgba(data)?;
    for p in rgba.chunks_exact_mut(4) {
        let a = p[3] as u32;
        if a != 255 {
            for c in &mut p[..3] {
                *c = ((*c as u32 * a + 127) / 255) as u8;
            }
        }
    }
    tiny_skia::Pixmap::from_vec(rgba, tiny_skia::IntSize::from_wh(w, h)?).map(Arc::new)
}

struct SvgSession {
    source: Arc<Source>,
    info: ImageInfo,
    tree: usvg::Tree,
    rasters: Arc<RasterCache>,
}

impl Session for SvgSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    /// Vector art stores no samples: transparency is not a property of the encoding.
    fn alpha(&self) -> Option<crate::Alpha> {
        None
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let scale = req.scale.clamp(1, 8);
        let (sw, sh) = (
            self.info.width.div_ceil(scale),
            self.info.height.div_ceil(scale),
        );
        let (x, y, w, h) = match req.region {
            Some(r) => {
                let x = r.x.min(sw);
                let y = r.y.min(sh);
                (x, y, r.w.min(sw - x), r.h.min(sh - y))
            }
            None => (0, 0, sw, sh),
        };
        if w == 0 || h == 0 {
            return Err(DecodeError::Unsupported("empty region".into()));
        }
        let mut pm = tiny_skia::Pixmap::new(w, h).ok_or(DecodeError::TooLarge)?;
        let size = self.tree.size();
        let (fx, fy) = (sw as f32 / size.width(), sh as f32 / size.height());
        let t = tiny_skia::Transform::from_row(fx, 0.0, 0.0, fy, -(x as f32), -(y as f32));
        {
            let _rendering = Rendering::enter(&self.rasters);
            resvg::render(&self.tree, t, &mut pm.as_mut());
        }
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let mut data = pm.take();
        // Premultiplied → straight alpha.
        for p in data.chunks_exact_mut(4) {
            let a = p[3] as u32;
            if a != 0 && a != 255 {
                for c in &mut p[..3] {
                    *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
                }
            }
        }
        Ok(Raster {
            width: w,
            height: h,
            origin: (x, y),
            scale,
            layout: PixelLayout::Rgba,
            planes: vec![RasterPlane {
                width: w,
                height: h,
                stride: w as usize * 4,
                data,
                h_div: 1,
                v_div: 1,
            }],
            truncated: false,
        })
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let s = req.scale.clamp(1, 8);
        let px = match req.region {
            Some(r) => r.w as u64 * r.h as u64,
            None => self.info.width.div_ceil(s) as u64 * self.info.height.div_ceil(s) as u64,
        };
        // The pixmap plus resvg's layer buffers.
        px * 4 * 2
    }
    /// The decoded embedded rasters.
    fn memory_bytes(&self) -> usize {
        self.rasters
            .entries
            .lock()
            .values()
            .filter_map(|c| c.get().and_then(Option::as_ref))
            .map(|p| p.data().len())
            .sum()
    }
    fn trim_memory(&self) -> usize {
        let held = self.memory_bytes();
        self.rasters.entries.lock().clear();
        held
    }
}

#[cfg(test)]
mod tests;
