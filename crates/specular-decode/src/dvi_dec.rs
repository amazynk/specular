//! TeX DVI pages through `specular-dvi`: rules, and characters drawn from the fonts the
//! system's TeX installation has (TFM metrics, Type 1 outlines through `specular-font`).
//! A character whose Type 1 font is not found is drawn as a grey box of its TFM size (or of
//! an estimate without one), so the page's layout still shows. Virtual fonts and PK bitmaps
//! are not read.
//!
//! Each DVI page is a session page, drawn at [`DPI`] on white; scales and regions are drawn
//! directly from the page's items.

use crate::{DecodeError, DecodeRequest, Decoder, Format, HeaderFacts, ImageInfo, Result, Session};
use resvg::tiny_skia as sk;
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, RasterPlane, Source};
use specular_dvi::{Dvi, Item, tfm::Tfm};
use specular_font::{PathEl, Type1Glyphs};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

/// Pixels per inch of the full raster.
pub const DPI: f64 = 200.0;
/// Largest side drawn.
const MAX_SIDE: f64 = 16384.0;
/// Largest font file read.
const MAX_FONT: u64 = 16 << 20;
/// Most font files indexed.
const MAX_INDEXED: usize = 200_000;

/// The TeX font folders looked in (the system's TeX Live and the user's own tree).
fn roots() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = [
        "/usr/share/texlive/texmf-dist/fonts",
        "/usr/share/texmf/fonts",
        "/usr/local/share/texmf/fonts",
        "/Library/TeX/Root/texmf-dist/fonts",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();
    if let Some(h) = std::env::var_os("HOME") {
        v.push(PathBuf::from(h).join("texmf/fonts"));
    }
    v
}

/// `name.tfm` and `name.pfb` files under the TeX font folders, by lower-case file name.
fn font_index() -> &'static HashMap<String, PathBuf> {
    static INDEX: OnceLock<HashMap<String, PathBuf>> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut out = HashMap::new();
        for root in roots() {
            for sub in ["tfm", "type1"] {
                let mut stack = vec![(root.join(sub), 0u32)];
                while let Some((dir, depth)) = stack.pop() {
                    let Ok(rd) = std::fs::read_dir(&dir) else {
                        continue;
                    };
                    for e in rd.flatten() {
                        if out.len() >= MAX_INDEXED {
                            return out;
                        }
                        let p = e.path();
                        let Ok(t) = e.file_type() else { continue };
                        if t.is_dir() && depth < 8 {
                            stack.push((p, depth + 1));
                        } else if t.is_file()
                            && let Some(n) = p.file_name().and_then(|n| n.to_str())
                        {
                            let n = n.to_ascii_lowercase();
                            if n.ends_with(".tfm") || n.ends_with(".pfb") {
                                out.entry(n).or_insert(p);
                            }
                        }
                    }
                }
            }
        }
        out
    })
}

fn read_font_file(name: &str, ext: &str) -> Option<Vec<u8>> {
    let p = font_index().get(&format!("{}.{ext}", name.to_ascii_lowercase()))?;
    let meta = std::fs::metadata(p).ok()?;
    (meta.len() <= MAX_FONT)
        .then(|| std::fs::read(p).ok())
        .flatten()
}

/// What was found for one font.
struct Loaded {
    tfm: Option<Tfm>,
    glyphs: Option<Type1Glyphs>,
}

pub struct DviDecoder;

impl Decoder for DviDecoder {
    fn format(&self) -> Format {
        Format::Dvi
    }
    fn probe(&self, head: &[u8]) -> bool {
        specular_dvi::probe(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["dvi"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let dvi = specular_dvi::parse(source.bytes()).map_err(|e| match e {
            specular_dvi::Error::NotDvi => DecodeError::UnsupportedFormat,
            specular_dvi::Error::Truncated => DecodeError::Truncated(e.to_string()),
            e => DecodeError::Malformed(e.to_string()),
        })?;
        if dvi.id == 3 {
            return Err(DecodeError::Unsupported(
                "this DVI file is pTeX's extended format (vertical Japanese text), which is not drawn"
                    .into(),
            ));
        }
        let mut fonts = HashMap::new();
        for (k, f) in &dvi.fonts {
            let tfm = read_font_file(&f.name, "tfm").and_then(|d| specular_dvi::tfm::parse(&d));
            let glyphs = read_font_file(&f.name, "pfb")
                .and_then(|d| specular_font::type1_glyphs_by_code(&d).ok());
            fonts.insert(*k, Loaded { tfm, glyphs });
        }
        let shared = Arc::new(Shared {
            header: header(&dvi, &fonts),
            dvi,
            fonts,
            source,
        });
        Ok(Arc::new(DviSession::new(shared, 0)?))
    }
}

fn header(dvi: &Dvi, fonts: &HashMap<u32, Loaded>) -> HeaderFacts {
    let mut fields = vec![
        ("Pages".to_owned(), dvi.pages.len().to_string()),
        (
            "Magnification".to_owned(),
            format!("{:.3}", dvi.mag as f64 / 1000.0),
        ),
    ];
    let mut names: BTreeMap<&str, bool> = BTreeMap::new();
    for (k, f) in &dvi.fonts {
        let drawn = fonts.get(k).is_some_and(|l| l.glyphs.is_some());
        *names.entry(f.name.as_str()).or_default() |= drawn;
    }
    let (found, missing): (Vec<_>, Vec<_>) = names.iter().partition(|(_, d)| **d);
    let list = |v: &[(&&str, &bool)]| v.iter().map(|(n, _)| **n).collect::<Vec<_>>().join(", ");
    if !found.is_empty() {
        fields.push(("Fonts drawn".to_owned(), list(&found)));
    }
    if !missing.is_empty() {
        fields.push(("Fonts shown as boxes".to_owned(), list(&missing)));
    }
    if !dvi.complete {
        fields.push((
            "Note".to_owned(),
            "the file has no postamble (cut short)".into(),
        ));
    }
    let mut summary = Vec::new();
    if !dvi.comment.is_empty() {
        summary.push(("comment", dvi.comment.clone()));
        fields.insert(0, ("Comment".to_owned(), dvi.comment.clone()));
    }
    if !missing.is_empty() {
        summary.push((
            "approximated",
            format!(
                "{} of {} fonts not found: shown as boxes",
                missing.len(),
                names.len()
            ),
        ));
    }
    HeaderFacts { summary, fields }
}

struct Shared {
    dvi: Dvi,
    fonts: HashMap<u32, Loaded>,
    source: Arc<Source>,
    header: HeaderFacts,
}

struct DviSession {
    shared: Arc<Shared>,
    page: u32,
    info: ImageInfo,
    items: Vec<Item>,
    /// Pixels per DVI unit at scale 1.
    k: f64,
}

impl DviSession {
    fn new(shared: Arc<Shared>, page: u32) -> Result<DviSession> {
        let dvi = &shared.dvi;
        let fonts = &shared.fonts;
        let width = |f: u32, c: u32| -> Option<i64> {
            let scale = dvi.fonts.get(&f)?.scale;
            fonts.get(&f)?.tfm.as_ref()?.dims(c, scale).map(|d| d.0)
        };
        let items = specular_dvi::page(dvi, shared.source.bytes(), page as usize, &width)
            .map_err(|e| DecodeError::Malformed(e.to_string()))?;
        let inch = 0.0254 / dvi.metres_per_unit();
        let (w, h) = match dvi.paper {
            Some((w, h)) if w > 0 && h > 0 => (w as f64, h as f64),
            _ => (
                (8.5 * inch).max(dvi.max_h as f64 + 2.0 * inch),
                (11.0 * inch).max(dvi.max_v as f64 + 2.0 * inch),
            ),
        };
        let mut k = DPI / inch;
        k = k.min(MAX_SIDE / w.max(h));
        let info = ImageInfo {
            format: Format::Dvi,
            width: ((w * k).round() as u32).max(1),
            height: ((h * k).round() as u32).max(1),
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: 8,
            progressive: false,
            coding: format!(
                "TeX DVI, page {} of {}",
                dvi.page_numbers.get(page as usize).copied().unwrap_or(0),
                dvi.pages.len()
            ),
            embedded_preview: None,
            fast_regions: true,
            scales: &[1, 2, 4, 8],
        };
        Ok(DviSession {
            shared: shared.clone(),
            page,
            info,
            items,
            k,
        })
    }

    /// Draws the page with `k` pixels per DVI unit, shifted by (`ox`, `oy`) pixels.
    fn render(
        &self,
        pm: &mut sk::Pixmap,
        k: f64,
        ox: f64,
        oy: f64,
        cancel: &CancelToken,
    ) -> Result<()> {
        pm.fill(sk::Color::WHITE);
        let dvi = &self.shared.dvi;
        let inch = 0.0254 / dvi.metres_per_unit();
        let px = |v: i64| (v as f64 + inch) * k;
        let mut ink = sk::Paint::default();
        ink.set_color_rgba8(0, 0, 0, 255);
        ink.anti_alias = true;
        let mut grey = sk::Paint::default();
        grey.set_color_rgba8(200, 200, 200, 255);
        let id = sk::Transform::from_translate(-ox as f32, -oy as f32);
        for (n, it) in self.items.iter().enumerate() {
            if n.is_multiple_of(4096) && cancel.is_cancelled() {
                return Err(DecodeError::Cancelled);
            }
            match *it {
                Item::Rule { x, y, w, h } => {
                    let (l, t) = (px(x), px(y - h));
                    let (r, b) = (l + (w as f64 * k).max(1.0), t + (h as f64 * k).max(1.0));
                    if let Some(rect) = sk::Rect::from_ltrb(l as f32, t as f32, r as f32, b as f32)
                    {
                        pm.fill_rect(rect, &ink, id, None);
                    }
                }
                Item::Char { font, code, x, y } => {
                    let Some(def) = dvi.fonts.get(&font) else {
                        continue;
                    };
                    let loaded = self.shared.fonts.get(&font);
                    let size = def.scale as f64 * k;
                    let (bx, by) = (px(x), px(y));
                    let glyph = loaded.and_then(|l| l.glyphs.as_ref()).and_then(|g| {
                        Some((g.units_per_em, g.glyphs.get(code as usize)?.as_ref()?))
                    });
                    if let Some((upem, (_, path))) = glyph {
                        let s = (size / upem as f64) as f32;
                        if let Some(p) = sk_path(path) {
                            let t = sk::Transform::from_row(
                                s,
                                0.0,
                                0.0,
                                -s,
                                (bx - ox) as f32,
                                (by - oy) as f32,
                            );
                            pm.fill_path(&p, &ink, sk::FillRule::Winding, t, None);
                        }
                        continue;
                    }
                    let dims = loaded
                        .and_then(|l| l.tfm.as_ref())
                        .and_then(|t| t.dims(code, def.scale));
                    let (w, h, d) = match dims {
                        Some((w, h, d)) => (w as f64 * k, h as f64 * k, d as f64 * k),
                        None => (size * 0.5, size * 0.7, size * 0.0),
                    };
                    let inset = (w * 0.08).min(1.0);
                    if let Some(rect) = sk::Rect::from_ltrb(
                        (bx + inset) as f32,
                        (by - h) as f32,
                        (bx + w - inset).max(bx + inset + 1.0) as f32,
                        (by + d).max(by - h + 1.0) as f32,
                    ) {
                        pm.fill_rect(rect, &grey, id, None);
                    }
                }
            }
        }
        Ok(())
    }
}

fn sk_path(path: &[PathEl]) -> Option<sk::Path> {
    let mut pb = sk::PathBuilder::new();
    for el in path {
        match *el {
            PathEl::MoveTo(x, y) => pb.move_to(x, y),
            PathEl::LineTo(x, y) => pb.line_to(x, y),
            PathEl::QuadTo(a, b, x, y) => pb.quad_to(a, b, x, y),
            PathEl::CubicTo(a, b, c, d, x, y) => pb.cubic_to(a, b, c, d, x, y),
            PathEl::Close => pb.close(),
        }
    }
    pb.finish()
}

impl Session for DviSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.shared.source
    }
    fn header(&self) -> Option<&HeaderFacts> {
        Some(&self.shared.header)
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
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
        let mut pm = sk::Pixmap::new(w, h).ok_or(DecodeError::TooLarge)?;
        self.render(&mut pm, self.k / scale as f64, x as f64, y as f64, cancel)?;
        let data = pm.take();
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
        px * 4 * 2
    }
    fn memory_bytes(&self) -> usize {
        self.items.len() * std::mem::size_of::<Item>()
    }
    fn page_count(&self) -> u32 {
        self.shared.dvi.pages.len() as u32
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page as usize >= self.shared.dvi.pages.len() {
            return Err(DecodeError::Malformed("no such page".into()));
        }
        Ok(Arc::new(DviSession::new(self.shared.clone(), page)?))
    }
}
