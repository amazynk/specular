//! Bitmap fonts: X11 PCF, Adobe BDF, Windows FNT (alone or inside a `.fon` NE executable)
//! and the Linux console's PSF 1 and 2, read into one model of glyph bitmaps, and a sheet
//! of their glyphs to look at.
//!
//! - PCF ("\1fcp"): a table of contents of typed tables, each with its own format word (byte
//!   and bit order, row padding, scan unit): properties, metrics (compressed or not), bitmaps,
//!   encodings and accelerators are read.
//! - BDF: text; FONT, SIZE, the properties, then each glyph's ENCODING, DWIDTH, BBX and hex
//!   BITMAP rows.
//! - FNT: version 2 or 3 headers, a character table of widths and offsets, glyphs stored in
//!   columns of 8 pixels. Vector FNT fonts are refused. A `.fon` keeps its FNT fonts as
//!   RT_FONT resources of an NE executable: each one is a page.
//! - PSF 1 (36 04: 256 or 512 glyphs 8 wide) and PSF 2 (72 B5 4A 86): fixed cells, with an
//!   optional Unicode table naming each glyph's characters.

use crate::Error;

/// The bitmap font formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BitmapKind {
    Pcf,
    Bdf,
    /// A bare Windows `.fnt`.
    Fnt,
    /// Windows `.fon`: FNT fonts in an NE executable.
    Fon,
    Psf1,
    Psf2,
}

impl BitmapKind {
    pub fn name(self) -> &'static str {
        match self {
            BitmapKind::Pcf => "X11 PCF",
            BitmapKind::Bdf => "BDF",
            BitmapKind::Fnt => "Windows FNT",
            BitmapKind::Fon => "Windows FON",
            BitmapKind::Psf1 => "PSF 1",
            BitmapKind::Psf2 => "PSF 2",
        }
    }
}

/// One glyph: its character (or glyph index when the font names none), ink box and advance.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BitmapGlyph {
    pub code: u32,
    pub width: u32,
    pub height: u32,
    /// Left of the ink from the origin.
    pub left: i32,
    /// Top of the ink above the baseline.
    pub top: i32,
    pub advance: i32,
    /// `width × height` pixels, 1 for ink.
    pub bits: Vec<u8>,
}

impl BitmapGlyph {
    pub fn ink(&self, x: u32, y: u32) -> bool {
        self.bits[(y * self.width + x) as usize] != 0
    }
}

/// One font (a `.fon` may hold several).
#[derive(Clone, Debug, Default)]
pub struct BitmapFont {
    pub name: String,
    pub family: String,
    /// Pixel height (the em), when the font says.
    pub pixel_size: u32,
    pub ascent: i32,
    pub descent: i32,
    /// "ISO8859-1", "Unicode", "Windows-1252"…
    pub encoding: String,
    pub glyphs: Vec<BitmapGlyph>,
    /// Further facts: (label, value).
    pub facts: Vec<(String, String)>,
}

/// The most glyphs read from a font.
const MAX_GLYPHS: usize = 1 << 17;
/// The largest glyph side read.
const MAX_SIDE: u32 = 512;

fn bad(s: &str) -> Error {
    Error::Malformed(s.into())
}

/// Which bitmap font `d` is, by its signature (BDF by its first line).
pub fn sniff_bitmap(d: &[u8]) -> Option<BitmapKind> {
    if d.starts_with(b"\x01fcp") {
        return Some(BitmapKind::Pcf);
    }
    if d.starts_with(b"STARTFONT ") {
        return Some(BitmapKind::Bdf);
    }
    if d.starts_with(&[0x36, 0x04]) && d.len() >= 4 {
        return Some(BitmapKind::Psf1);
    }
    if d.starts_with(&[0x72, 0xB5, 0x4A, 0x86]) {
        return Some(BitmapKind::Psf2);
    }
    if d.len() >= 0x40 && d.starts_with(b"MZ") {
        let ne = u32le(d, 0x3C) as usize;
        if d.get(ne..ne + 2) == Some(b"NE") && ne_fonts(d, ne).is_some_and(|f| !f.is_empty()) {
            return Some(BitmapKind::Fon);
        }
        return None;
    }
    if d.len() >= 118
        && matches!(u16le(d, 0), 0x200 | 0x300)
        && u32le(d, 2) as usize <= d.len() + 16
    {
        let (first, last) = (d[95], d[96]);
        if first <= last && u16le(d, 88) > 0 && u16le(d, 88) <= 512 {
            return Some(BitmapKind::Fnt);
        }
    }
    None
}

fn u16le(d: &[u8], at: usize) -> u16 {
    d.get(at..at + 2)
        .map_or(0, |b| u16::from_le_bytes([b[0], b[1]]))
}

fn u32le(d: &[u8], at: usize) -> u32 {
    d.get(at..at + 4)
        .map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Reads every font of a bitmap font file.
pub fn open_bitmap(d: &[u8]) -> Result<(BitmapKind, Vec<BitmapFont>), Error> {
    let kind = sniff_bitmap(d).ok_or_else(|| bad("not a bitmap font"))?;
    let fonts = match kind {
        BitmapKind::Pcf => vec![pcf(d)?],
        BitmapKind::Bdf => vec![bdf(d)?],
        BitmapKind::Fnt => vec![fnt(d)?],
        BitmapKind::Fon => {
            let ne = u32le(d, 0x3C) as usize;
            let mut out = Vec::new();
            for (at, len) in ne_fonts(d, ne).unwrap_or_default() {
                let part = d
                    .get(at..(at + len).min(d.len()))
                    .ok_or_else(|| bad("FON resource"))?;
                out.push(fnt(part)?);
            }
            if out.is_empty() {
                return Err(bad("a .fon without fonts"));
            }
            out
        }
        BitmapKind::Psf1 | BitmapKind::Psf2 => vec![psf(d, kind)?],
    };
    if fonts.iter().all(|f| f.glyphs.is_empty()) {
        return Err(bad("a font without glyphs"));
    }
    Ok((kind, fonts))
}

// ---------------------------------------------------------------- PCF

const PCF_PROPERTIES: u32 = 1;
const PCF_ACCELERATORS: u32 = 2;
const PCF_METRICS: u32 = 4;
const PCF_BITMAPS: u32 = 8;
const PCF_BDF_ENCODINGS: u32 = 32;
const PCF_BDF_ACCELERATORS: u32 = 256;

/// A PCF table's reader: its format word sets the byte order of what follows.
struct Table<'a> {
    d: &'a [u8],
    at: usize,
    format: u32,
}

impl Table<'_> {
    fn big(&self) -> bool {
        self.format & 4 != 0
    }
    fn u8(&mut self) -> Result<u8, Error> {
        let v = *self.d.get(self.at).ok_or_else(|| bad("PCF table"))?;
        self.at += 1;
        Ok(v)
    }
    fn i16(&mut self) -> Result<i16, Error> {
        let b = self
            .d
            .get(self.at..self.at + 2)
            .ok_or_else(|| bad("PCF table"))?;
        self.at += 2;
        Ok(if self.big() {
            i16::from_be_bytes([b[0], b[1]])
        } else {
            i16::from_le_bytes([b[0], b[1]])
        })
    }
    fn i32(&mut self) -> Result<i32, Error> {
        let b = self
            .d
            .get(self.at..self.at + 4)
            .ok_or_else(|| bad("PCF table"))?;
        self.at += 4;
        Ok(if self.big() {
            i32::from_be_bytes([b[0], b[1], b[2], b[3]])
        } else {
            i32::from_le_bytes([b[0], b[1], b[2], b[3]])
        })
    }
    /// left, right, width, ascent, descent.
    fn metrics(&mut self, compressed: bool) -> Result<[i32; 5], Error> {
        if compressed {
            let mut m = [0; 5];
            for v in &mut m {
                *v = i32::from(self.u8()?) - 0x80;
            }
            Ok(m)
        } else {
            let m = [
                i32::from(self.i16()?),
                i32::from(self.i16()?),
                i32::from(self.i16()?),
                i32::from(self.i16()?),
                i32::from(self.i16()?),
            ];
            self.i16()?;
            Ok(m)
        }
    }
}

fn pcf(d: &[u8]) -> Result<BitmapFont, Error> {
    let n = (u32le(d, 4) as usize).min(64);
    let mut toc = Vec::new();
    for k in 0..n {
        let e = 8 + 16 * k;
        toc.push((
            u32le(d, e),
            u32le(d, e + 8) as usize,
            u32le(d, e + 12) as usize,
        ));
    }
    let table = |kind: u32| -> Option<Table<'_>> {
        let &(_, size, at) = toc.iter().find(|t| t.0 == kind)?;
        let t = d.get(at..at.checked_add(size)?)?;
        Some(Table {
            d: t,
            at: 4,
            format: u32le(t, 0),
        })
    };
    let mut font = BitmapFont::default();
    // Properties.
    if let Some(mut t) = table(PCF_PROPERTIES) {
        let count = (t.i32()?.max(0) as usize).min(4096);
        let mut props = Vec::with_capacity(count);
        for _ in 0..count {
            let name = t.i32()?;
            let is_string = t.u8()? != 0;
            let value = t.i32()?;
            props.push((name, is_string, value));
        }
        if count & 3 != 0 {
            t.at += 4 - (count & 3);
        }
        let size = t.i32()?.max(0) as usize;
        let strings =
            t.d.get(t.at..(t.at + size).min(t.d.len()))
                .unwrap_or_default();
        let s = |o: i32| -> String {
            let o = o.max(0) as usize;
            let b = strings.get(o..).unwrap_or_default();
            String::from_utf8_lossy(b.split(|&c| c == 0).next().unwrap_or_default()).into_owned()
        };
        let mut registry = String::new();
        let mut encoding = String::new();
        for (name, is_string, value) in props {
            let key = s(name);
            let val = if is_string {
                s(value)
            } else {
                value.to_string()
            };
            match key.as_str() {
                "FONT" => font.name = val,
                "FAMILY_NAME" => font.family = val,
                "PIXEL_SIZE" => font.pixel_size = value.max(0) as u32,
                "CHARSET_REGISTRY" => registry = val,
                "CHARSET_ENCODING" => encoding = val,
                "FOUNDRY" | "WEIGHT_NAME" | "SLANT" | "COPYRIGHT" | "POINT_SIZE" | "SPACING" => {
                    font.facts.push((key, val))
                }
                _ => {}
            }
        }
        if !registry.is_empty() {
            font.encoding = format!("{registry}-{encoding}");
        }
    }
    // Font ascent and descent.
    if let Some(mut t) = table(PCF_BDF_ACCELERATORS).or_else(|| table(PCF_ACCELERATORS)) {
        t.at += 8;
        font.ascent = t.i32()?;
        font.descent = t.i32()?;
    }
    let mut m = table(PCF_METRICS).ok_or_else(|| bad("PCF without metrics"))?;
    let compressed = m.format & 0x100 != 0;
    let count = if compressed {
        i32::from(m.i16()?)
    } else {
        m.i32()?
    }
    .clamp(0, MAX_GLYPHS as i32) as usize;
    let mut metrics = Vec::with_capacity(count);
    for _ in 0..count {
        metrics.push(m.metrics(compressed)?);
    }
    let mut b = table(PCF_BITMAPS).ok_or_else(|| bad("PCF without bitmaps"))?;
    let bcount = (b.i32()?.max(0) as usize).min(count);
    let mut offsets = Vec::with_capacity(bcount);
    for _ in 0..bcount {
        offsets.push(b.i32()?.max(0) as usize);
    }
    let mut sizes = [0usize; 4];
    for s in &mut sizes {
        *s = b.i32()?.max(0) as usize;
    }
    let pad = 1usize << (b.format & 3);
    let unit = 1usize << ((b.format >> 4) & 3);
    let msbit = b.format & 8 != 0;
    let lsbyte = !b.big();
    let data =
        b.d.get(b.at..(b.at + sizes[(b.format & 3) as usize]).min(b.d.len()))
            .unwrap_or_default();
    // Glyph index → characters.
    let mut codes: Vec<Vec<u32>> = vec![Vec::new(); count];
    if let Some(mut e) = table(PCF_BDF_ENCODINGS) {
        let (min2, max2) = (e.i16()? as i32, e.i16()? as i32);
        let (min1, max1) = (e.i16()? as i32, e.i16()? as i32);
        e.i16()?;
        for b1 in min1..=max1.min(255) {
            for b2 in min2..=max2.min(255) {
                let i = e.i16()? as u16;
                if i != 0xFFFF && usize::from(i) < count {
                    codes[usize::from(i)].push((b1 as u32) << 8 | b2 as u32);
                }
            }
        }
    }
    for (i, &[left, right, width, ascent, descent]) in metrics.iter().enumerate() {
        let w = (right - left).clamp(0, MAX_SIDE as i32) as u32;
        let h = (ascent + descent).clamp(0, MAX_SIDE as i32) as u32;
        let row = (w as usize).div_ceil(8).div_ceil(pad) * pad;
        let start = offsets.get(i).copied().unwrap_or(usize::MAX);
        let mut bits = vec![0u8; (w * h) as usize];
        for y in 0..h as usize {
            for x in 0..w as usize {
                let mut byte = start.saturating_add(y * row + x / 8);
                if lsbyte && unit > 1 {
                    // Bytes of a scan unit stored least significant first.
                    let base = byte - byte % unit;
                    byte = base + (unit - 1 - byte % unit);
                }
                let v = data.get(byte).copied().unwrap_or(0);
                let bit = if msbit { 7 - x % 8 } else { x % 8 };
                bits[y * w as usize + x] = (v >> bit) & 1;
            }
        }
        let glyph = BitmapGlyph {
            code: 0,
            width: w,
            height: h,
            left,
            top: ascent,
            advance: width,
            bits,
        };
        let cs = &codes[i];
        if cs.is_empty() && table(PCF_BDF_ENCODINGS).is_none() {
            font.glyphs.push(BitmapGlyph {
                code: i as u32,
                ..glyph.clone()
            });
        }
        for &c in cs {
            font.glyphs.push(BitmapGlyph {
                code: c,
                ..glyph.clone()
            });
        }
    }
    font.glyphs.sort_by_key(|g| g.code);
    if font.encoding.is_empty() {
        font.encoding = "font-specific".into();
    }
    Ok(font)
}

// ---------------------------------------------------------------- BDF

fn bdf(d: &[u8]) -> Result<BitmapFont, Error> {
    let text = String::from_utf8_lossy(d);
    let mut font = BitmapFont::default();
    let mut lines = text.lines();
    let mut registry = String::new();
    let mut encoding = String::new();
    let mut default_width = 0i32;
    while let Some(line) = lines.next() {
        let mut it = line.split_whitespace();
        let Some(key) = it.next() else { continue };
        let rest = line[key.len()..].trim();
        let unquote = |s: &str| s.trim_matches('"').to_owned();
        match key {
            "FONT" => font.name = rest.to_owned(),
            "SIZE" => {
                if font.pixel_size == 0 {
                    font.pixel_size = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                }
            }
            "PIXEL_SIZE" => font.pixel_size = rest.parse().unwrap_or(font.pixel_size),
            "FAMILY_NAME" => font.family = unquote(rest),
            "FONT_ASCENT" => font.ascent = rest.parse().unwrap_or(0),
            "FONT_DESCENT" => font.descent = rest.parse().unwrap_or(0),
            "CHARSET_REGISTRY" => registry = unquote(rest),
            "CHARSET_ENCODING" => encoding = unquote(rest),
            "DWIDTH" => default_width = it.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            "FOUNDRY" | "WEIGHT_NAME" | "COPYRIGHT" => font.facts.push((key.into(), unquote(rest))),
            "STARTCHAR" => {
                if font.glyphs.len() >= MAX_GLYPHS {
                    break;
                }
                let mut g = BitmapGlyph {
                    advance: default_width,
                    ..BitmapGlyph::default()
                };
                let mut code: i64 = -1;
                let (mut bx, mut by) = (0i32, 0i32);
                while let Some(l) = lines.next() {
                    let mut f = l.split_whitespace();
                    match f.next() {
                        Some("ENCODING") => {
                            code = f.next().and_then(|v| v.parse().ok()).unwrap_or(-1);
                            if code == -1 {
                                // A glyph outside the encoding may name its code after.
                                code = f.next().and_then(|v| v.parse().ok()).unwrap_or(-1);
                            }
                        }
                        Some("DWIDTH") => {
                            g.advance = f.next().and_then(|v| v.parse().ok()).unwrap_or(0)
                        }
                        Some("BBX") => {
                            let v: Vec<i32> = f.filter_map(|v| v.parse().ok()).collect();
                            if v.len() == 4 {
                                g.width = v[0].clamp(0, MAX_SIDE as i32) as u32;
                                g.height = v[1].clamp(0, MAX_SIDE as i32) as u32;
                                (bx, by) = (v[2], v[3]);
                            }
                        }
                        Some("BITMAP") => {
                            let mut bits = vec![0u8; (g.width * g.height) as usize];
                            for y in 0..g.height as usize {
                                let Some(row) = lines.next() else { break };
                                let row = row.trim();
                                if row == "ENDCHAR" {
                                    break;
                                }
                                for x in 0..g.width as usize {
                                    let nib = row
                                        .as_bytes()
                                        .get(x / 4)
                                        .and_then(|c| (*c as char).to_digit(16))
                                        .unwrap_or(0);
                                    bits[y * g.width as usize + x] =
                                        ((nib >> (3 - x % 4)) & 1) as u8;
                                }
                            }
                            g.bits = bits;
                        }
                        Some("ENDCHAR") => break,
                        _ => {}
                    }
                }
                if g.bits.len() != (g.width * g.height) as usize {
                    g.bits = vec![0; (g.width * g.height) as usize];
                }
                g.left = bx;
                g.top = by + g.height as i32;
                if code >= 0 {
                    g.code = code as u32;
                    font.glyphs.push(g);
                }
            }
            "ENDFONT" => break,
            _ => {}
        }
    }
    if !d.starts_with(b"STARTFONT") {
        return Err(bad("BDF header"));
    }
    font.glyphs.sort_by_key(|g| g.code);
    font.encoding = if registry.is_empty() {
        "font-specific".into()
    } else {
        format!("{registry}-{encoding}")
    };
    Ok(font)
}

// ---------------------------------------------------------------- FNT / FON

/// The RT_FONT resources (offset, length) of an NE executable.
fn ne_fonts(d: &[u8], ne: usize) -> Option<Vec<(usize, usize)>> {
    let rt = ne + usize::from(u16le(d, ne + 0x24));
    let shift = u32::from(u16le(d, rt)).min(16);
    let mut at = rt + 2;
    let mut out = Vec::new();
    for _ in 0..64 {
        let id = u16le(d, at);
        if id == 0 {
            break;
        }
        let n = usize::from(u16le(d, at + 2));
        at += 8;
        for k in 0..n.min(256) {
            let e = at + 12 * k;
            if id == 0x8008 {
                let off = (u16le(d, e) as usize) << shift;
                let len = (u16le(d, e + 2) as usize) << shift;
                out.push((off, len));
            }
        }
        at += 12 * n;
        if at > d.len() {
            return None;
        }
    }
    Some(out)
}

fn fnt(d: &[u8]) -> Result<BitmapFont, Error> {
    if d.len() < 118 {
        return Err(bad("FNT header"));
    }
    let version = u16le(d, 0);
    if u16le(d, 66) & 1 != 0 {
        return Err(Error::Unsupported(
            "Vector Windows fonts are not drawn: only bitmap ones are.".into(),
        ));
    }
    let points = u16le(d, 68);
    let ascent = i32::from(u16le(d, 74));
    let height = u32::from(u16le(d, 88)).min(MAX_SIDE);
    let charset = d[85];
    let (first, last) = (u32::from(d[95]), u32::from(d[96]));
    let face = u32le(d, 105) as usize;
    let (table, entry) = if version >= 0x300 { (148, 6) } else { (118, 4) };
    let mut font = BitmapFont {
        name: d
            .get(face..)
            .map(|b| {
                String::from_utf8_lossy(b.split(|&c| c == 0).next().unwrap_or_default())
                    .into_owned()
            })
            .unwrap_or_default(),
        pixel_size: height,
        ascent,
        descent: height as i32 - ascent,
        encoding: charset_name(charset).into(),
        ..BitmapFont::default()
    };
    font.family = font.name.clone();
    font.facts.push(("Points".into(), points.to_string()));
    let weight = u16le(d, 83);
    if weight > 0 {
        font.facts.push(("Weight".into(), weight.to_string()));
    }
    let copyright = String::from_utf8_lossy(d[6..66].split(|&c| c == 0).next().unwrap_or_default())
        .trim()
        .to_owned();
    if !copyright.is_empty() {
        font.facts.push(("Copyright".into(), copyright));
    }
    for c in first..=last {
        let e = table + entry * (c - first) as usize;
        let width = u32::from(u16le(d, e)).min(MAX_SIDE);
        let off = if version >= 0x300 {
            u32le(d, e + 2) as usize
        } else {
            usize::from(u16le(d, e + 2))
        };
        let mut bits = vec![0u8; (width * height) as usize];
        for x in 0..width as usize {
            for y in 0..height as usize {
                let byte = d
                    .get(off + (x / 8) * height as usize + y)
                    .copied()
                    .unwrap_or(0);
                bits[y * width as usize + x] = (byte >> (7 - x % 8)) & 1;
            }
        }
        font.glyphs.push(BitmapGlyph {
            code: c,
            width,
            height,
            left: 0,
            top: ascent,
            advance: width as i32,
            bits,
        });
    }
    Ok(font)
}

fn charset_name(c: u8) -> &'static str {
    match c {
        0 => "Windows ANSI",
        1 => "default",
        2 => "symbol",
        77 => "Macintosh",
        128 => "Shift JIS",
        129 => "Hangul",
        134 => "GB 2312",
        136 => "Big5",
        161 => "Greek",
        162 => "Turkish",
        177 => "Hebrew",
        178 => "Arabic",
        186 => "Baltic",
        204 => "Cyrillic",
        222 => "Thai",
        238 => "Eastern European",
        255 => "OEM",
        _ => "other",
    }
}

// ---------------------------------------------------------------- PSF

fn psf(d: &[u8], kind: BitmapKind) -> Result<BitmapFont, Error> {
    let (count, height, width, head, table) = if kind == BitmapKind::Psf1 {
        let mode = d[2];
        let count = if mode & 1 != 0 { 512 } else { 256 };
        (count, u32::from(d[3]), 8, 4, mode & 6 != 0)
    } else {
        let head = u32le(d, 8) as usize;
        let flags = u32le(d, 12);
        (
            (u32le(d, 16) as usize).min(MAX_GLYPHS),
            u32le(d, 24),
            u32le(d, 28),
            head,
            flags & 1 != 0,
        )
    };
    if height == 0 || width == 0 || height > MAX_SIDE || width > MAX_SIDE {
        return Err(bad("PSF glyph size"));
    }
    let row = (width as usize).div_ceil(8);
    let size = row * height as usize;
    let mut font = BitmapFont {
        name: String::new(),
        pixel_size: height,
        ascent: height as i32,
        descent: 0,
        encoding: if table {
            "Unicode".into()
        } else {
            "glyph order".into()
        },
        ..BitmapFont::default()
    };
    let mut glyphs = Vec::with_capacity(count);
    for i in 0..count {
        let at = head + i * size;
        let Some(g) = d.get(at..at + size) else {
            break;
        };
        let mut bits = vec![0u8; (width * height) as usize];
        for y in 0..height as usize {
            for x in 0..width as usize {
                bits[y * width as usize + x] = (g[y * row + x / 8] >> (7 - x % 8)) & 1;
            }
        }
        glyphs.push(BitmapGlyph {
            code: i as u32,
            width,
            height,
            left: 0,
            top: height as i32,
            advance: width as i32,
            bits,
        });
    }
    let read = glyphs.len();
    if table {
        // Each glyph's characters (its first names it); sequences after FFFE are skipped.
        let mut at = head + read * size;
        let mut named: Vec<Option<u32>> = vec![None; read];
        for slot in named.iter_mut() {
            let mut seq = false;
            if kind == BitmapKind::Psf1 {
                while let Some(b) = d.get(at..at + 2) {
                    at += 2;
                    match u16::from_le_bytes([b[0], b[1]]) {
                        0xFFFF => break,
                        0xFFFE => seq = true,
                        u if !seq && slot.is_none() => *slot = Some(u32::from(u)),
                        _ => {}
                    }
                }
            } else {
                let end = d[at.min(d.len())..]
                    .iter()
                    .position(|&b| b == 0xFF)
                    .map_or(d.len(), |p| at + p);
                let part = &d[at.min(d.len())..end];
                let first = part.split(|&b| b == 0xFE).next().unwrap_or_default();
                if let Some(c) = std::str::from_utf8(first)
                    .ok()
                    .and_then(|s| s.chars().next())
                {
                    *slot = Some(c as u32);
                }
                at = end + 1;
            }
        }
        let mut out = Vec::with_capacity(read);
        for (g, c) in glyphs.into_iter().zip(named) {
            if let Some(c) = c {
                out.push(BitmapGlyph { code: c, ..g });
            }
        }
        out.sort_by_key(|g| g.code);
        out.dedup_by_key(|g| g.code);
        glyphs = out;
    }
    font.glyphs = glyphs;
    font.facts.push(("Glyphs stored".into(), read.to_string()));
    Ok(font)
}

// ---------------------------------------------------------------- sheet

/// Where a sheet puts its glyphs.
struct Layout {
    left: i32,
    top: i32,
    zoom: u32,
    pad: u32,
    per_row: usize,
    cell: (u32, u32),
    size: (u32, u32),
}

fn layout(font: &BitmapFont, shown: usize) -> Layout {
    let glyphs = &font.glyphs[..shown];
    let left = glyphs.iter().map(|g| g.left).min().unwrap_or(0).min(0);
    let right = glyphs
        .iter()
        .map(|g| (g.left + g.width as i32).max(g.advance))
        .max()
        .unwrap_or(1);
    let top = glyphs
        .iter()
        .map(|g| g.top)
        .max()
        .unwrap_or(0)
        .max(font.ascent);
    let bottom = glyphs
        .iter()
        .map(|g| g.height as i32 - g.top)
        .max()
        .unwrap_or(0)
        .max(font.descent);
    let (cw, ch) = (
        (right - left).clamp(1, MAX_SIDE as i32) as u32,
        (top + bottom).clamp(1, MAX_SIDE as i32) as u32,
    );
    let zoom = (24 / ch).clamp(1, 3);
    let per_row = match shown {
        0..=512 => 16,
        513..=4096 => 32,
        _ => 64,
    };
    let rows = shown.div_ceil(per_row).max(1) as u32;
    let pad = 2 * zoom;
    let cell = (cw * zoom + 2 * pad, ch * zoom + 2 * pad);
    Layout {
        left,
        top,
        zoom,
        pad,
        per_row,
        cell,
        size: (per_row as u32 * cell.0 + 1, rows * cell.1 + 1),
    }
}

/// The size of the sheet [`sheet`] draws.
pub fn sheet_size(font: &BitmapFont, max_glyphs: usize) -> (u32, u32) {
    layout(font, font.glyphs.len().min(max_glyphs)).size
}

/// A sheet of the glyphs in character order: rows of 16 (32 past 512 glyphs, 64 past 4096)
/// cells of the font's line, drawn black on white with a grey grid; small fonts drawn 2× or
/// 3×. At most `max_glyphs` are shown. Gives (width, height, grey pixels).
pub fn sheet(font: &BitmapFont, max_glyphs: usize) -> (u32, u32, Vec<u8>) {
    let shown = font.glyphs.len().min(max_glyphs);
    let l = layout(font, shown);
    let (w, h) = l.size;
    let (cell_w, cell_h) = l.cell;
    let zoom = l.zoom;
    let mut px = vec![255u8; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            if x % cell_w == 0 || y % cell_h == 0 {
                px[(y * w + x) as usize] = 200;
            }
        }
    }
    for (i, g) in font.glyphs[..shown].iter().enumerate() {
        let cx = (i % l.per_row) as u32 * cell_w + l.pad;
        let cy = (i / l.per_row) as u32 * cell_h + l.pad;
        let ox = (g.left - l.left) as u32 * zoom;
        let oy = (l.top - g.top).max(0) as u32 * zoom;
        for y in 0..g.height {
            for x in 0..g.width {
                if !g.ink(x, y) {
                    continue;
                }
                for dy in 0..zoom {
                    for dx in 0..zoom {
                        let (sx, sy) = (cx + ox + x * zoom + dx, cy + oy + y * zoom + dy);
                        if sx < w && sy < h {
                            px[(sy * w + sx) as usize] = 0;
                        }
                    }
                }
            }
        }
    }
    (w, h, px)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn psf2_with_unicode() {
        let mut d = vec![0x72, 0xB5, 0x4A, 0x86];
        for v in [0u32, 32, 1, 2, 2, 2, 4] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        // Two glyphs 4 wide, 2 tall: "A" (a bar), "é".
        d.extend_from_slice(&[0xF0, 0x00, 0x00, 0x90]);
        d.extend_from_slice(b"A\xFF\xC3\xA9\xFF");
        let (k, f) = open_bitmap(&d).unwrap();
        assert_eq!(k, BitmapKind::Psf2);
        let f = &f[0];
        assert_eq!(f.glyphs.len(), 2);
        assert_eq!(f.glyphs[0].code, 'A' as u32);
        assert_eq!(f.glyphs[1].code, 'é' as u32);
        assert!(f.glyphs[0].ink(3, 0) && !f.glyphs[0].ink(0, 1));
        assert!(f.glyphs[1].ink(0, 1) && f.glyphs[1].ink(3, 1));
        let (w, h, px) = sheet(f, 100);
        assert_eq!(px.len(), (w * h) as usize);
        assert!(px.contains(&0));
    }

    #[test]
    fn bdf_glyph() {
        let t = "STARTFONT 2.1\nFONT test\nSIZE 8 75 75\nFONTBOUNDINGBOX 4 4 0 0\nSTARTPROPERTIES 1\nFONT_ASCENT 4\nENDPROPERTIES\nCHARS 1\nSTARTCHAR A\nENCODING 65\nDWIDTH 5 0\nBBX 4 2 0 1\nBITMAP\n90\n60\nENDCHAR\nENDFONT\n";
        let (k, f) = open_bitmap(t.as_bytes()).unwrap();
        assert_eq!(k, BitmapKind::Bdf);
        let g = &f[0].glyphs[0];
        assert_eq!(
            (g.code, g.width, g.height, g.top, g.advance),
            (65, 4, 2, 3, 5)
        );
        assert!(g.ink(0, 0) && g.ink(3, 0) && g.ink(1, 1) && !g.ink(0, 1));
    }

    #[test]
    fn vector_fnt_is_refused() {
        let mut d = vec![0u8; 200];
        d[..2].copy_from_slice(&0x200u16.to_le_bytes());
        d[2..6].copy_from_slice(&200u32.to_le_bytes());
        d[66] = 1;
        d[88] = 10;
        d[95] = 32;
        d[96] = 33;
        assert_eq!(sniff_bitmap(&d), Some(BitmapKind::Fnt));
        assert!(matches!(open_bitmap(&d), Err(Error::Unsupported(_))));
    }
}
