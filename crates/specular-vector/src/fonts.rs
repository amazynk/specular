//! Fonts for drawing text: the TrueType font a text style names when it is installed, else
//! a substitute (a metric-compatible family where one is known, else the bundled Inter),
//! with glyph outlines cached.
//!
//! SHX (compiled shape) fonts are drawn with their own shapes ([`crate::shx`]) when a style's
//! font was found ([`attach_shx`]: the drawing's folder and the user's SHX folders); else
//! with a substitute, and reported missing ([`missing_fonts`]). System fonts come from
//! fontique (the text stack's font discovery); [`Fonts::bundled_only`] never looks at them,
//! so tests and screenshots draw the same glyphs on every machine.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use kurbo::BezPath;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::prelude::{FontRef, MetadataProvider};
use skrifa::{GlyphId, Tag};

use crate::scene::{FontSpec, ShxRef, VectorScene};
use crate::shx::{self, ShxStyle};

/// Where a face's glyphs come from.
enum Backing {
    /// A TrueType/OpenType font at a variation location.
    Outline {
        data: Arc<dyn AsRef<[u8]> + Send + Sync>,
        index: u32,
        coords: Vec<skrifa::instance::NormalizedCoord>,
    },
    /// SHX shapes: single strokes.
    Shx(Arc<ShxStyle>),
}

/// A loaded font face.
pub struct Face {
    /// The family name as found.
    pub family: String,
    backing: Backing,
    pub units_per_em: f32,
    /// Cap height, ascent and descent (descent negative), font units.
    pub cap_height: f32,
    pub ascent: f32,
    pub descent: f32,
    /// Synthesised italic: the font has no italic of its own.
    pub fake_italic: bool,
    glyphs: Mutex<HashMap<u32, Option<Arc<Glyph>>>>,
}

/// One glyph's outline (font units, y up) and advance.
#[derive(Debug)]
pub struct Glyph {
    pub path: BezPath,
    pub advance: f32,
    /// Lines to stroke (an SHX shape), not an outline to fill.
    pub stroke: bool,
}

impl std::fmt::Debug for Face {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Face")
            .field("family", &self.family)
            .finish()
    }
}

struct Pen<'a>(&'a mut BezPath);

impl OutlinePen for Pen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to((f64::from(x), f64::from(y)));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to((f64::from(x), f64::from(y)));
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.0.quad_to(
            (f64::from(cx0), f64::from(cy0)),
            (f64::from(x), f64::from(y)),
        );
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.0.curve_to(
            (f64::from(cx0), f64::from(cy0)),
            (f64::from(cx1), f64::from(cy1)),
            (f64::from(x), f64::from(y)),
        );
    }
    fn close(&mut self) {
        self.0.close_path();
    }
}

impl Face {
    fn new(
        family: String,
        data: Arc<dyn AsRef<[u8]> + Send + Sync>,
        index: u32,
        bold: bool,
        fake_italic: bool,
    ) -> Option<Face> {
        let font = FontRef::from_index((*data).as_ref(), index).ok()?;
        let axes = font.axes();
        let coords = if bold && axes.iter().any(|a| a.tag() == Tag::new(b"wght")) {
            let loc = axes.location([(Tag::new(b"wght"), 700.0)]);
            loc.coords().to_vec()
        } else {
            let loc = axes.location([(Tag::new(b"wght"), 400.0)]);
            loc.coords().to_vec()
        };
        let m = font.metrics(Size::unscaled(), LocationRef::new(&coords));
        let upem = f32::from(m.units_per_em.max(1));
        let cap = m.cap_height.filter(|c| *c > 0.0).unwrap_or(upem * 0.7);
        Some(Face {
            family,
            backing: Backing::Outline {
                data: data.clone(),
                index,
                coords,
            },
            units_per_em: upem,
            cap_height: cap,
            ascent: m.ascent,
            descent: m.descent,
            fake_italic,
            glyphs: Mutex::new(HashMap::new()),
        })
    }

    /// An SHX face: cap height `above` shape units, the em sized so a fallback face's
    /// capitals come out about as tall.
    fn shx(family: String, style: Arc<ShxStyle>) -> Face {
        let above = style.above() as f32;
        let below = style.below() as f32;
        Face {
            family,
            backing: Backing::Shx(style),
            units_per_em: above / 0.72,
            cap_height: above,
            ascent: above * 1.2,
            descent: -below,
            fake_italic: true,
            glyphs: Mutex::new(HashMap::new()),
        }
    }

    fn font(&self) -> Option<FontRef<'_>> {
        match &self.backing {
            Backing::Outline { data, index, .. } => {
                FontRef::from_index((**data).as_ref(), *index).ok()
            }
            Backing::Shx(_) => None,
        }
    }

    /// Whether the face draws single strokes (SHX).
    pub fn is_shx(&self) -> bool {
        matches!(self.backing, Backing::Shx(_))
    }

    /// Whether the face maps `c`.
    pub fn has(&self, c: char) -> bool {
        if let Backing::Shx(st) = &self.backing {
            return c == ' ' || st.lookup(c).is_some();
        }
        self.font()
            .and_then(|f| f.charmap().map(c))
            .is_some_and(|g| g != GlyphId::NOTDEF)
    }

    /// The glyph for `c` (outline in font units, y up), cached; `None` when unmapped.
    pub fn glyph(&self, c: char) -> Option<Arc<Glyph>> {
        let key = c as u32;
        if let Ok(g) = self.glyphs.lock()
            && let Some(hit) = g.get(&key)
        {
            return hit.clone();
        }
        let made = self.make_glyph(c);
        if let Ok(mut g) = self.glyphs.lock() {
            g.insert(key, made.clone());
        }
        made
    }

    fn make_glyph(&self, c: char) -> Option<Arc<Glyph>> {
        let coords = match &self.backing {
            Backing::Outline { coords, .. } => coords,
            Backing::Shx(st) => return self.shx_glyph(st, c),
        };
        let font = self.font()?;
        let gid = font.charmap().map(c)?;
        if gid == GlyphId::NOTDEF {
            return None;
        }
        let loc = LocationRef::new(coords);
        let advance = font
            .glyph_metrics(Size::unscaled(), loc)
            .advance_width(gid)
            .unwrap_or(self.units_per_em * 0.5);
        let mut path = BezPath::new();
        if let Some(o) = font.outline_glyphs().get(gid) {
            let _ = o.draw(
                DrawSettings::unhinted(Size::unscaled(), loc),
                &mut Pen(&mut path),
            );
        }
        Some(Arc::new(Glyph {
            path,
            advance,
            stroke: false,
        }))
    }

    fn shx_glyph(&self, st: &ShxStyle, c: char) -> Option<Arc<Glyph>> {
        let Some((font, code)) = st.lookup(c) else {
            // A font without a space shape: an empty advance of 2/3 the cap height.
            return (c == ' ').then(|| {
                Arc::new(Glyph {
                    path: BezPath::new(),
                    advance: self.cap_height * 2.0 / 3.0,
                    stroke: true,
                })
            });
        };
        let (path, end) = font.render(code)?;
        // A Big Font's shapes are in its own units: scaled to the main font's cap height.
        let cell = font.extended.map_or(font.above, |e| e.0).max(1);
        let k = f64::from(self.cap_height) / f64::from(cell);
        let path = if (k - 1.0).abs() > 1e-12 {
            kurbo::Affine::scale(k) * path
        } else {
            path
        };
        let mut advance = (end.x * k) as f32;
        if font.extended.is_some() && advance <= 0.0 {
            // Extended Big Font characters are placed by box; the advance is the box width.
            advance = f32::from(font.extended.map_or(cell, |e| e.1)) * k as f32;
        }
        Some(Arc::new(Glyph {
            path,
            advance,
            stroke: true,
        }))
    }
}

/// Which font a style resolved to.
#[derive(Clone, Debug)]
pub struct Resolved {
    pub face: Arc<Face>,
    /// The font the drawing asked for is not what draws (SHX, or not installed).
    pub substituted: bool,
}

/// Font discovery and the faces loaded so far. Cheap to share.
pub struct Fonts {
    system: Option<Mutex<SystemFonts>>,
    bundled: Arc<Face>,
    bundled_bold: Arc<Face>,
    fallbacks: Vec<Arc<Face>>,
    faces: Mutex<FaceCache>,
}

/// Faces by (lower-case family, bold, italic); `None` for one not found.
type FaceCache = HashMap<(String, bool, bool), Option<Arc<Face>>>;

struct SystemFonts {
    collection: fontique::Collection,
    cache: fontique::SourceCache,
}

impl std::fmt::Debug for Fonts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fonts")
            .field("system", &self.system.is_some())
            .finish()
    }
}

/// Known TrueType file names (lower case, without extension) and their families.
const FILE_FAMILIES: &[(&str, &str)] = &[
    ("arial", "Arial"),
    ("arialbd", "Arial"),
    ("ariali", "Arial"),
    ("arialn", "Arial Narrow"),
    ("times", "Times New Roman"),
    ("timesbd", "Times New Roman"),
    ("cour", "Courier New"),
    ("calibri", "Calibri"),
    ("cambria", "Cambria"),
    ("verdana", "Verdana"),
    ("tahoma", "Tahoma"),
    ("segoeui", "Segoe UI"),
    ("consola", "Consolas"),
    ("isocpeur", "ISOCPEUR"),
    ("isocteur", "ISOCTEUR"),
    ("isocp", "ISOCP"),
    ("romantic", "Romantic"),
    ("swiss", "Swis721 BT"),
    ("simsun", "SimSun"),
    ("msgothic", "MS Gothic"),
    ("msmincho", "MS Mincho"),
    ("malgun", "Malgun Gothic"),
    ("gulim", "Gulim"),
    ("dejavusans", "DejaVu Sans"),
    ("liberationsans-regular", "Liberation Sans"),
];

/// Metric-compatible stand-ins for common Windows families.
const METRIC_TWINS: &[(&str, &[&str])] = &[
    (
        "Arial",
        &["Liberation Sans", "Arimo", "Helvetica", "Nimbus Sans"],
    ),
    (
        "Arial Narrow",
        &["Liberation Sans Narrow", "Nimbus Sans Narrow"],
    ),
    (
        "Times New Roman",
        &["Liberation Serif", "Tinos", "Times", "Nimbus Roman"],
    ),
    (
        "Courier New",
        &["Liberation Mono", "Cousine", "Nimbus Mono PS"],
    ),
    ("Calibri", &["Carlito"]),
    ("Cambria", &["Caladea"]),
    ("Verdana", &["DejaVu Sans"]),
    ("Tahoma", &["DejaVu Sans"]),
    ("Consolas", &["DejaVu Sans Mono", "Liberation Mono"]),
    ("Segoe UI", &["Noto Sans", "Open Sans"]),
    ("SimSun", &["Noto Serif CJK SC", "Noto Sans CJK SC"]),
    ("MS Gothic", &["Noto Sans CJK JP"]),
    ("MS Mincho", &["Noto Serif CJK JP"]),
    ("Malgun Gothic", &["Noto Sans CJK KR"]),
    ("Gulim", &["Noto Sans CJK KR"]),
];

/// Families SHX text falls back to, in order: plain sans faces close to the AutoCAD
/// `txt`/`simplex`/`romans` proportions.
const SHX_TWINS: &[&str] = &[
    "Liberation Sans Narrow",
    "Arial Narrow",
    "DejaVu Sans Condensed",
];

/// Fonts tried for characters the chosen face lacks.
const FALLBACK_FAMILIES: &[&str] = &[
    "Noto Sans",
    "DejaVu Sans",
    "Noto Sans CJK SC",
    "Noto Sans CJK JP",
    "Noto Sans CJK KR",
    "Noto Sans Symbols",
    "Noto Sans Symbols2",
];

impl Fonts {
    /// The bundled fonts only: the same glyphs on every machine.
    pub fn bundled_only() -> Fonts {
        Fonts::make(false)
    }

    /// The bundled fonts and the platform's (found on first use).
    pub fn with_system() -> Fonts {
        Fonts::make(true)
    }

    /// One instance for the process: system fonts, unless `SPECULAR_BUNDLED_FONTS` is set
    /// (screenshot tests).
    pub fn shared() -> Arc<Fonts> {
        static SHARED: OnceLock<Arc<Fonts>> = OnceLock::new();
        SHARED
            .get_or_init(|| {
                Arc::new(if std::env::var_os("SPECULAR_BUNDLED_FONTS").is_some() {
                    Fonts::bundled_only()
                } else {
                    Fonts::with_system()
                })
            })
            .clone()
    }

    fn make(system: bool) -> Fonts {
        let inter: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(specular_theme::fonts::INTER);
        let bundled = Face::new(
            specular_theme::fonts::UI_FAMILY.to_string(),
            inter.clone(),
            0,
            false,
            true,
        )
        .expect("the bundled font parses");
        let bundled_bold = Face::new(
            specular_theme::fonts::UI_FAMILY.to_string(),
            inter,
            0,
            true,
            true,
        )
        .expect("the bundled font parses");
        let mut fallbacks = Vec::new();
        for (data, family) in [
            (
                specular_theme::fonts::NOTO_SANS_ARABIC,
                specular_theme::fonts::ARABIC_FAMILY,
            ),
            (
                specular_theme::fonts::NOTO_SANS_HEBREW,
                specular_theme::fonts::HEBREW_FAMILY,
            ),
        ] {
            let d: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(data);
            if let Some(f) = Face::new(family.to_string(), d, 0, false, true) {
                fallbacks.push(Arc::new(f));
            }
        }
        let system = system.then(|| {
            Mutex::new(SystemFonts {
                collection: fontique::Collection::new(fontique::CollectionOptions {
                    shared: false,
                    system_fonts: true,
                }),
                cache: fontique::SourceCache::default(),
            })
        });
        let fonts = Fonts {
            system,
            bundled: Arc::new(bundled),
            bundled_bold: Arc::new(bundled_bold),
            fallbacks,
            faces: Mutex::new(HashMap::new()),
        };
        let mut more = Vec::new();
        for fam in FALLBACK_FAMILIES {
            if let Some(f) = fonts.system_face(fam, false, false) {
                more.push(f);
            }
        }
        let mut fonts = fonts;
        fonts.fallbacks.extend(more);
        fonts
    }

    /// Whether system fonts are looked up.
    pub fn uses_system(&self) -> bool {
        self.system.is_some()
    }

    /// The bundled face (Inter).
    pub fn default_face(&self, bold: bool) -> Arc<Face> {
        if bold {
            self.bundled_bold.clone()
        } else {
            self.bundled.clone()
        }
    }

    /// A face for a character `primary` lacks, if any font has it.
    pub fn fallback_for(&self, primary: &Face, c: char) -> Option<Arc<Face>> {
        if primary.has(c) {
            return None;
        }
        if !std::ptr::eq(primary, &*self.bundled) && self.bundled.has(c) {
            return Some(self.bundled.clone());
        }
        self.fallbacks.iter().find(|f| f.has(c)).cloned()
    }

    fn system_face(&self, family: &str, bold: bool, italic: bool) -> Option<Arc<Face>> {
        let key = (family.to_ascii_lowercase(), bold, italic);
        if let Ok(m) = self.faces.lock()
            && let Some(hit) = m.get(&key)
        {
            return hit.clone();
        }
        let found = self.load_system(family, bold, italic).map(Arc::new);
        if let Ok(mut m) = self.faces.lock() {
            m.insert(key, found.clone());
        }
        found
    }

    fn load_system(&self, family: &str, bold: bool, italic: bool) -> Option<Face> {
        let sys = self.system.as_ref()?;
        let mut sys = sys.lock().ok()?;
        let SystemFonts { collection, cache } = &mut *sys;
        let fam = collection.family_by_name(family)?;
        let name = fam.name().to_string();
        let weight = fontique::FontWeight::new(if bold { 700.0 } else { 400.0 });
        let style = if italic {
            fontique::FontStyle::Italic
        } else {
            fontique::FontStyle::Normal
        };
        let font = fam.match_font(fontique::FontWidth::NORMAL, style, weight, true)?;
        let fake_italic = italic && font.style() == fontique::FontStyle::Normal;
        let index = font.index();
        let blob = font.load(Some(cache))?;
        let data: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(BlobBytes(blob));
        Face::new(
            name,
            data,
            index,
            bold && !font.weight().value().ge(&600.0),
            fake_italic,
        )
        .or_else(|| {
            let blob = font.load(Some(cache))?;
            Face::new(
                fam.name().to_string(),
                Arc::new(BlobBytes(blob)),
                index,
                false,
                fake_italic,
            )
        })
    }

    /// The face for a text style, with an inline family (MTEXT `\f`) taking precedence.
    pub fn resolve(
        &self,
        spec: &FontSpec,
        family: Option<&str>,
        bold: bool,
        italic: bool,
    ) -> Resolved {
        let bold = bold || spec.bold;
        let italic = italic || spec.italic;
        let want = family
            .map(str::to_string)
            .or_else(|| spec.family.clone().filter(|f| !f.is_empty()))
            .or_else(|| family_of_file(&spec.file));
        let shx = family.is_none() && spec.is_shx();
        if family.is_none()
            && let Some(r) = &spec.shx
            && let Some(face) = r.face.clone()
        {
            return Resolved {
                face,
                substituted: false,
            };
        }
        if !shx && let Some(w) = &want {
            if let Some(f) = self.system_face(w, bold, italic) {
                return Resolved {
                    face: f,
                    substituted: false,
                };
            }
            let twins = METRIC_TWINS
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(w))
                .map(|t| t.1)
                .unwrap_or(&[]);
            for t in twins {
                if let Some(f) = self.system_face(t, bold, italic) {
                    return Resolved {
                        face: f,
                        substituted: true,
                    };
                }
            }
        }
        if shx {
            for t in SHX_TWINS {
                if let Some(f) = self.system_face(t, bold, italic) {
                    return Resolved {
                        face: f,
                        substituted: true,
                    };
                }
            }
        }
        // Nothing asked for (no font file): the default draws as asked.
        let substituted = shx || want.is_some();
        Resolved {
            face: self.default_face(bold),
            substituted,
        }
    }
}

struct BlobBytes(fontique::Blob<u8>);

impl AsRef<[u8]> for BlobBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

/// The family a font file name stands for: a known Windows file, else its stem.
pub fn family_of_file(file: &str) -> Option<String> {
    let name = file.rsplit(['/', '\\']).next().unwrap_or(file).trim();
    if name.is_empty() {
        return None;
    }
    let stem = match name.rfind('.') {
        Some(i) => &name[..i],
        None => name,
    };
    let lower = stem.to_ascii_lowercase();
    if let Some((_, f)) = FILE_FAMILIES.iter().find(|(k, _)| *k == lower) {
        return Some((*f).to_string());
    }
    Some(stem.to_string())
}

/// Looks for every SHX font the scene's styles name (main and Big Font), first in
/// `drawing_dir`, then in the user's SHX folders ([`shx::folders`]), and records what was
/// found in [`FontSpec::shx`]. `encoding` is the drawing's code page, which maps text back
/// to Big Font codes. Autodesk's fonts are never bundled: what is not found is substituted.
pub fn attach_shx(
    scene: &mut VectorScene,
    drawing_dir: Option<&std::path::Path>,
    encoding: Option<&'static encoding_rs::Encoding>,
) {
    let mut dirs: Vec<std::path::PathBuf> = drawing_dir.map(Into::into).into_iter().collect();
    dirs.extend(shx::folders());
    attach_shx_in(scene, &dirs, encoding);
}

/// [`attach_shx`] with the folders given.
pub fn attach_shx_in(
    scene: &mut VectorScene,
    dirs: &[std::path::PathBuf],
    encoding: Option<&'static encoding_rs::Encoding>,
) {
    for spec in &mut scene.fonts {
        let main = if spec.is_shx() {
            shx::shx_file_name(&spec.file)
        } else {
            None
        };
        let big = shx::shx_file_name(&spec.bigfont);
        if main.is_none() && big.is_none() {
            continue;
        }
        let mut missing = Vec::new();
        let mut get = |name: Option<String>| {
            let name = name?;
            let found = shx::find(&name, dirs).and_then(|p| shx::load(&p).map(|f| (p, f)));
            if found.is_none() {
                missing.push(name);
            }
            found
        };
        let m = get(main);
        let b = get(big);
        let face = m.as_ref().map(|(p, _)| {
            let family = p
                .file_stem()
                .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
            Arc::new(Face::shx(
                family,
                Arc::new(ShxStyle {
                    main: m.as_ref().map(|x| x.1.clone()),
                    big: b.as_ref().map(|x| x.1.clone()),
                    encoding,
                }),
            ))
        });
        spec.shx = Some(ShxRef {
            face,
            main: m.map(|x| x.0),
            big: b.map(|x| x.0),
            missing,
        });
    }
}

/// A font a drawing names that is not what draws its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingFont {
    /// The file (or TrueType family) asked for.
    pub file: String,
    /// The text styles that name it.
    pub styles: Vec<String>,
    /// The family that draws the text instead; empty when only a Big Font is missing (its
    /// characters come from the fallback fonts).
    pub substitute: String,
}

/// What the info panel lists as missing: every font file (or family) a style names that
/// was not found, with the styles that name it and what draws instead, sorted by file.
pub fn missing_fonts(fonts: &Fonts, specs: &[FontSpec]) -> Vec<MissingFont> {
    let mut v: Vec<MissingFont> = Vec::new();
    let mut add = |file: &str, style: &str, substitute: String| {
        if let Some(m) = v.iter_mut().find(|m| m.file.eq_ignore_ascii_case(file)) {
            if !m.styles.iter().any(|s| s == style) {
                m.styles.push(style.to_string());
            }
            return;
        }
        v.push(MissingFont {
            file: file.to_string(),
            styles: vec![style.to_string()],
            substitute,
        });
    };
    for s in specs {
        if s.file.is_empty() && s.family.is_none() && s.bigfont.is_empty() {
            continue;
        }
        let r = fonts.resolve(s, None, false, false);
        if let Some(x) = &s.shx {
            for name in &x.missing {
                let sub = if x.face.is_none() {
                    r.face.family.clone()
                } else {
                    String::new()
                };
                add(name, &s.style, sub);
            }
            if !x.missing.is_empty() || x.face.is_some() {
                continue;
            }
        }
        if r.substituted && (!s.file.is_empty() || s.family.is_some()) {
            let file = if s.file.is_empty() {
                s.family.clone().unwrap_or_default()
            } else {
                s.file.clone()
            };
            add(&file, &s.style, r.face.family.clone());
        }
    }
    v.sort_by_key(|m| m.file.to_ascii_lowercase());
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_glyphs() {
        let f = Fonts::bundled_only();
        let face = f.default_face(false);
        let g = face.glyph('A').expect("A");
        assert!(g.advance > 0.0);
        assert!(!g.path.elements().is_empty());
        assert!(face.cap_height > 0.0 && face.cap_height < face.units_per_em);
        assert!(face.glyph('\u{4e00}').is_none());
    }

    #[test]
    fn shx_and_missing_are_substituted() {
        let f = Fonts::bundled_only();
        let shx = FontSpec {
            style: "Standard".into(),
            file: "txt".into(),
            ..Default::default()
        };
        assert!(shx.is_shx());
        assert!(f.resolve(&shx, None, false, false).substituted);
        let none = FontSpec::default();
        assert!(!f.resolve(&none, None, false, false).substituted);
        let ttf = FontSpec {
            file: "arial.ttf".into(),
            ..Default::default()
        };
        assert!(f.resolve(&ttf, None, false, false).substituted);
        let files: Vec<String> = missing_fonts(&f, &[shx, none, ttf.clone(), ttf])
            .into_iter()
            .map(|m| m.file)
            .collect();
        assert_eq!(files, vec!["arial.ttf".to_string(), "txt".to_string()]);
        assert_eq!(
            family_of_file("C:\\Fonts\\ARIAL.TTF").as_deref(),
            Some("Arial")
        );
    }
}
