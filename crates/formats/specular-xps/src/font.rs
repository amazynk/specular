//! Fonts: embedded (and obfuscated) OpenType parts, glyph outlines through skrifa.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use skrifa::GlyphId;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::prelude::{FontRef, MetadataProvider};
use vello_cpu::kurbo::BezPath;

/// Glyph outlines cached per font.
const MAX_CACHED_GLYPHS: usize = 65_536;

pub struct Font {
    data: Arc<Vec<u8>>,
    index: u32,
    pub units_per_em: f32,
    glyph_count: u32,
    glyphs: Mutex<HashMap<u32, Arc<BezPath>>>,
}

impl std::fmt::Debug for Font {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Font").field("index", &self.index).finish()
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

/// Whether `d` starts like an OpenType file or collection.
fn is_sfnt(d: &[u8]) -> bool {
    d.len() >= 12
        && matches!(
            &d[..4],
            [0, 1, 0, 0] | b"true" | b"OTTO" | b"ttcf" | b"typ1"
        )
}

/// The 16 key bytes of an obfuscated font named after a GUID, in the order the specification
/// lays them over the GUID string `{33221100-5544-7766-8899-AABBCCDDEEFF}`; `None` when the
/// name holds no GUID.
pub fn guid_key(part: &str) -> Option<[u8; 16]> {
    let file = part.rsplit('/').next()?;
    let stem = file.split('.').next()?;
    let hex: Vec<u8> = stem
        .bytes()
        .filter(|c| !matches!(c, b'{' | b'}' | b'-'))
        .collect();
    if hex.len() != 32 {
        return None;
    }
    let byte = |i: usize| -> Option<u8> {
        let s = std::str::from_utf8(hex.get(i..i + 2)?).ok()?;
        u8::from_str_radix(s, 16).ok()
    };
    // Positions in the dash-free string of bytes 00..FF.
    const AT: [usize; 16] = [6, 4, 2, 0, 10, 8, 14, 12, 16, 18, 20, 22, 24, 26, 28, 30];
    let mut key = [0u8; 16];
    for (k, &at) in key.iter_mut().zip(AT.iter()) {
        *k = byte(at)?;
    }
    Some(key)
}

/// Undoes the obfuscation of the first 32 bytes. The specification's order is tried first;
/// if the result is not an OpenType file, the plain reversed GUID string is tried (some
/// writers); else the bytes are returned as the first attempt left them.
pub fn deobfuscate(mut data: Vec<u8>, part: &str) -> Vec<u8> {
    if is_sfnt(&data) || data.len() < 32 {
        return data;
    }
    let Some(key) = guid_key(part) else {
        return data;
    };
    let orig: Vec<u8> = data[..32].to_vec();
    xor_key(&mut data, &orig, &key);
    if is_sfnt(&data) {
        return data;
    }
    // Alternative: the GUID string's bytes in written order, reversed.
    let file = part.rsplit('/').next().unwrap_or("");
    let hex: Vec<u8> = file
        .split('.')
        .next()
        .unwrap_or("")
        .bytes()
        .filter(|c| !matches!(c, b'{' | b'}' | b'-'))
        .collect();
    let mut alt = [0u8; 16];
    for (i, a) in alt.iter_mut().enumerate() {
        *a = std::str::from_utf8(&hex[i * 2..i * 2 + 2])
            .ok()
            .and_then(|s| u8::from_str_radix(s, 16).ok())
            .unwrap_or(0);
    }
    let mut tried = data.clone();
    xor_key(&mut tried, &orig, &alt);
    if is_sfnt(&tried) { tried } else { data }
}

/// `out[i] = orig[i] ^ key[15 - i % 16]` over the first 32 bytes.
pub fn xor_key(out: &mut [u8], orig: &[u8], key: &[u8; 16]) {
    for (i, (o, s)) in out.iter_mut().zip(orig).take(32).enumerate() {
        *o = s ^ key[15 - (i % 16)];
    }
}

impl Font {
    /// A font from its bytes (face `index` of a collection).
    pub fn new(data: Vec<u8>, index: u32) -> Option<Font> {
        let data = Arc::new(data);
        let f = FontRef::from_index(&data, index).ok()?;
        let m = f.metrics(Size::unscaled(), LocationRef::default());
        let upem = f32::from(m.units_per_em);
        if upem <= 0.0 {
            return None;
        }
        Some(Font {
            index,
            units_per_em: upem,
            glyph_count: u32::from(m.glyph_count),
            data,
            glyphs: Mutex::new(HashMap::new()),
        })
    }

    /// The ascender in ems (the typographic ascent), 0.8 when the font does not say.
    pub fn ascent(&self) -> f64 {
        let Some(f) = self.font() else { return 0.8 };
        let m = f.metrics(Size::unscaled(), LocationRef::default());
        let a = f64::from(m.ascent / self.units_per_em);
        if a.is_finite() && a > 0.0 { a } else { 0.8 }
    }

    /// The bundled UI face, for text whose font is missing.
    pub fn fallback() -> Option<Arc<Font>> {
        static F: OnceLock<Option<Arc<Font>>> = OnceLock::new();
        F.get_or_init(|| Font::new(specular_theme::fonts::INTER.to_vec(), 0).map(Arc::new))
            .clone()
    }

    fn font(&self) -> Option<FontRef<'_>> {
        FontRef::from_index(&self.data, self.index).ok()
    }

    /// A short-lived handle for laying out one run.
    pub fn face(&self) -> Option<Face<'_>> {
        let f = self.font()?;
        Some(Face {
            charmap: f.charmap(),
            metrics: f.glyph_metrics(Size::unscaled(), LocationRef::default()),
            font: self,
        })
    }

    /// The outline of glyph `gid` in font units (y up), cached.
    pub fn outline(&self, gid: u32) -> Arc<BezPath> {
        if let Ok(g) = self.glyphs.lock()
            && let Some(p) = g.get(&gid)
        {
            return p.clone();
        }
        let mut path = BezPath::new();
        if gid < self.glyph_count
            && let Some(f) = self.font()
            && let Some(o) = f.outline_glyphs().get(GlyphId::new(gid))
        {
            let _ = o.draw(
                DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
                &mut Pen(&mut path),
            );
        }
        let path = Arc::new(path);
        if let Ok(mut g) = self.glyphs.lock() {
            if g.len() >= MAX_CACHED_GLYPHS {
                g.clear();
            }
            g.insert(gid, path.clone());
        }
        path
    }
}

pub struct Face<'a> {
    charmap: skrifa::charmap::Charmap<'a>,
    metrics: skrifa::metrics::GlyphMetrics<'a>,
    font: &'a Font,
}

impl Face<'_> {
    /// The glyph for `c`, also trying the symbol-font private-use range.
    pub fn glyph(&self, c: u32) -> u32 {
        for cand in [c, 0xF000 | (c & 0xFF)] {
            if let Some(g) = self.charmap.map(cand)
                && g != GlyphId::NOTDEF
            {
                return g.to_u32();
            }
            if c >= 0x100 {
                break;
            }
        }
        0
    }

    /// Advance of `gid` in ems.
    pub fn advance(&self, gid: u32) -> f64 {
        let a = self.metrics.advance_width(GlyphId::new(gid)).unwrap_or(0.0);
        f64::from(a / self.font.units_per_em)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_key_order() {
        let k = guid_key("Resources/{00112233-4455-6677-8899-AABBCCDDEEFF}.odttf").unwrap();
        assert_eq!(
            k,
            [
                0x33, 0x22, 0x11, 0x00, 0x55, 0x44, 0x77, 0x66, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
                0xEE, 0xFF
            ]
        );
        assert!(guid_key("x/font.ttf").is_none());
    }

    #[test]
    fn obfuscation_round_trip() {
        let font = specular_theme::fonts::INTER.to_vec();
        let name = "Documents/1/Resources/Fonts/7FAF9A0E-C9F2-4C34-A1D7-9C5B9B2EB4CE.odttf";
        let key = guid_key(name).unwrap();
        let mut obf = font.clone();
        xor_key(&mut obf, &font, &key);
        assert_ne!(obf, font);
        assert_eq!(deobfuscate(obf, name), font);
        let f = Font::new(font, 0).unwrap();
        let face = f.face().unwrap();
        let g = face.glyph('A' as u32);
        assert!(g != 0);
        assert!(face.advance(g) > 0.3);
        assert!(f.outline(g).elements().len() > 3);
    }
}
