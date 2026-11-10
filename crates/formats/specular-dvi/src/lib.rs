//! TeX DVI files: the preamble (units, magnification, comment), the pages, the fonts they
//! use, and each page's rules and characters at their positions. [`tfm`] reads TeX font
//! metric files for the characters' widths, heights and depths.
//!
//! A DVI file says where characters go, not what they look like: drawing them needs the
//! fonts. [`page`] takes a callback for character widths (from TFM files when found) and
//! falls back to half an em when there is none, so text still lands roughly in place.
//! Written from the DVI format description in TeX: The Program (§583–§590) and dvitype.
//!
//! Every count is bounded: damaged files fail with a reason, never panic.

use std::collections::BTreeMap;

#[doc(hidden)]
pub mod fixtures;
pub mod tfm;

/// Most pages indexed.
pub const MAX_PAGES: usize = 100_000;
/// Most items one page yields.
pub const MAX_ITEMS: usize = 2_000_000;
/// Deepest `push` nesting.
const MAX_STACK: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    NotDvi,
    Unsupported(String),
    Truncated,
    Malformed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotDvi => write!(f, "not a DVI file"),
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::Truncated => write!(f, "the DVI file is cut short"),
            Error::Malformed(s) => write!(f, "malformed: {s}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// A font the file defines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontDef {
    pub name: String,
    /// TFM checksum.
    pub checksum: u32,
    /// Scaled size, in DVI units.
    pub scale: u32,
    /// Design size, in DVI units.
    pub design: u32,
}

/// A DVI file's structure.
#[derive(Clone, Debug, PartialEq)]
pub struct Dvi {
    /// DVI format identification (2, or 3 for pTeX's vertical text).
    pub id: u8,
    pub num: u32,
    pub den: u32,
    /// Magnification × 1000.
    pub mag: u32,
    pub comment: String,
    /// Byte offset of each page's `bop`.
    pub pages: Vec<usize>,
    /// TeX's \count0 (the page number) of each page.
    pub page_numbers: Vec<i32>,
    pub fonts: BTreeMap<u32, FontDef>,
    /// The postamble's tallest page height plus depth and widest page width, DVI units.
    pub max_v: u32,
    pub max_h: u32,
    /// A `papersize=W,H` special's page size, in DVI units.
    pub paper: Option<(i64, i64)>,
    /// Whether the postamble was found (a complete file).
    pub complete: bool,
}

impl Dvi {
    /// Metres per DVI unit, magnification included.
    pub fn metres_per_unit(&self) -> f64 {
        self.num as f64 / self.den.max(1) as f64 * 1e-7 * self.mag as f64 / 1000.0
    }
}

/// Whether `data` starts a DVI file.
pub fn probe(data: &[u8]) -> bool {
    data.len() >= 15 && data[0] == 247 && matches!(data[1], 2 | 3)
}

struct R<'a> {
    d: &'a [u8],
    at: usize,
}

impl R<'_> {
    fn u(&mut self, n: usize) -> Result<u32> {
        let b = self.d.get(self.at..self.at + n).ok_or(Error::Truncated)?;
        self.at += n;
        Ok(b.iter().fold(0u32, |v, &x| v << 8 | x as u32))
    }
    fn s(&mut self, n: usize) -> Result<i32> {
        let v = self.u(n)?;
        let shift = 32 - 8 * n as u32;
        Ok(((v << shift) as i32) >> shift)
    }
    fn bytes(&mut self, n: usize) -> Result<&[u8]> {
        let b = self.d.get(self.at..self.at + n).ok_or(Error::Truncated)?;
        self.at += n;
        Ok(b)
    }
}

/// One command.
#[derive(Debug, Clone, PartialEq)]
enum Cmd {
    Set(u32),
    Put(u32),
    SetRule(i32, i32),
    PutRule(i32, i32),
    Nop,
    Bop([i32; 10]),
    Eop,
    Push,
    Pop,
    Right(i32),
    W(Option<i32>),
    X(Option<i32>),
    Down(i32),
    Y(Option<i32>),
    Z(Option<i32>),
    Font(u32),
    Special(Vec<u8>),
    FontDef(u32, FontDef),
    Pre,
    Post,
    PostPost,
    Dir,
}

fn font_def(r: &mut R, k: u32) -> Result<Cmd> {
    let checksum = r.u(4)?;
    let scale = r.u(4)?;
    let design = r.u(4)?;
    let a = r.u(1)? as usize;
    let l = r.u(1)? as usize;
    let n = r.bytes(a + l)?;
    let name = String::from_utf8_lossy(&n[a..]).into_owned();
    Ok(Cmd::FontDef(
        k,
        FontDef {
            name,
            checksum,
            scale,
            design,
        },
    ))
}

fn next(r: &mut R) -> Result<Cmd> {
    let op = r.u(1)? as u8;
    let len = |o: u8, base: u8| (o - base + 1) as usize;
    Ok(match op {
        0..=127 => Cmd::Set(op as u32),
        128..=131 => Cmd::Set(r.u(len(op, 128))?),
        132 => Cmd::SetRule(r.s(4)?, r.s(4)?),
        133..=136 => Cmd::Put(r.u(len(op, 133))?),
        137 => Cmd::PutRule(r.s(4)?, r.s(4)?),
        138 => Cmd::Nop,
        139 => {
            let mut c = [0i32; 10];
            for v in &mut c {
                *v = r.s(4)?;
            }
            r.s(4)?;
            Cmd::Bop(c)
        }
        140 => Cmd::Eop,
        141 => Cmd::Push,
        142 => Cmd::Pop,
        143..=146 => Cmd::Right(r.s(len(op, 143))?),
        147 => Cmd::W(None),
        148..=151 => Cmd::W(Some(r.s(len(op, 148))?)),
        152 => Cmd::X(None),
        153..=156 => Cmd::X(Some(r.s(len(op, 153))?)),
        157..=160 => Cmd::Down(r.s(len(op, 157))?),
        161 => Cmd::Y(None),
        162..=165 => Cmd::Y(Some(r.s(len(op, 162))?)),
        166 => Cmd::Z(None),
        167..=170 => Cmd::Z(Some(r.s(len(op, 167))?)),
        171..=234 => Cmd::Font((op - 171) as u32),
        235..=238 => Cmd::Font(r.u(len(op, 235))?),
        239..=242 => {
            let k = r.u(len(op, 239))? as usize;
            Cmd::Special(r.bytes(k)?.to_vec())
        }
        243..=246 => {
            let k = r.u(len(op, 243))?;
            font_def(r, k)?
        }
        247 => Cmd::Pre,
        248 => Cmd::Post,
        249 => Cmd::PostPost,
        // pTeX's `dir`.
        255 => {
            r.u(1)?;
            Cmd::Dir
        }
        _ => return Err(Error::Malformed(format!("undefined command {op}"))),
    })
}

/// A `papersize=W,H` special's size in DVI units (TeX points scaled by num/den).
fn papersize(s: &[u8], num: u32, den: u32) -> Option<(i64, i64)> {
    let s = std::str::from_utf8(s).ok()?.trim();
    let v = s.strip_prefix("papersize=")?;
    let (w, h) = v.split_once(',')?;
    let unit = |t: &str| -> Option<f64> {
        let t = t.trim();
        let split = t.find(|c: char| c.is_ascii_alphabetic())?;
        let (n, u) = t.split_at(split);
        let n: f64 = n.trim().parse().ok()?;
        let pt = match u.trim_start_matches("true") {
            "pt" => 1.0,
            "in" => 72.27,
            "mm" => 72.27 / 25.4,
            "cm" => 72.27 / 2.54,
            "bp" => 72.27 / 72.0,
            _ => return None,
        };
        // Points to DVI units: 1 pt = 65536 sp at the standard num/den.
        let metres_per_unit = num as f64 / den.max(1) as f64 * 1e-7;
        Some(n * pt * 0.0254 / 72.27 / metres_per_unit)
    };
    Some((unit(w)? as i64, unit(h)? as i64))
}

/// Reads the preamble, indexes the pages and collects the fonts.
pub fn parse(data: &[u8]) -> Result<Dvi> {
    if !probe(data) {
        return Err(Error::NotDvi);
    }
    let mut r = R { d: data, at: 1 };
    let id = r.u(1)? as u8;
    let num = r.u(4)?;
    let den = r.u(4)?;
    let mag = r.u(4)?;
    if num == 0 || den == 0 {
        return Err(Error::Malformed("zero unit".into()));
    }
    let k = r.u(1)? as usize;
    let comment = String::from_utf8_lossy(r.bytes(k)?).trim().to_owned();
    let mut dvi = Dvi {
        id,
        num,
        den,
        mag: if mag == 0 { 1000 } else { mag },
        comment,
        pages: Vec::new(),
        page_numbers: Vec::new(),
        fonts: BTreeMap::new(),
        max_v: 0,
        max_h: 0,
        paper: None,
        complete: false,
    };
    loop {
        let at = r.at;
        let cmd = match next(&mut r) {
            Ok(c) => c,
            Err(Error::Truncated) if !dvi.pages.is_empty() => break,
            Err(e) => return Err(e),
        };
        match cmd {
            Cmd::Bop(c) => {
                if dvi.pages.len() >= MAX_PAGES {
                    break;
                }
                dvi.pages.push(at);
                dvi.page_numbers.push(c[0]);
            }
            Cmd::FontDef(k, f) => {
                dvi.fonts.entry(k).or_insert(f);
            }
            Cmd::Special(s) if dvi.paper.is_none() => dvi.paper = papersize(&s, num, den),
            Cmd::Post => {
                r.u(4)?;
                r.u(4)?;
                r.u(4)?;
                r.u(4)?;
                dvi.max_v = r.u(4)?;
                dvi.max_h = r.u(4)?;
                dvi.complete = true;
                break;
            }
            Cmd::Pre | Cmd::PostPost => {
                return Err(Error::Malformed(
                    "preamble or postamble out of place".into(),
                ));
            }
            _ => {}
        }
    }
    if dvi.pages.is_empty() {
        return Err(Error::Malformed("no pages".into()));
    }
    Ok(dvi)
}

/// What a page shows, in DVI units from the reference point (1 inch in from the top left),
/// y down.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// A filled box with its bottom left at (`x`, `y`).
    Rule { x: i64, y: i64, w: i64, h: i64 },
    /// A character with its reference point (on the baseline) at (`x`, `y`).
    Char {
        font: u32,
        code: u32,
        x: i64,
        y: i64,
    },
}

/// A page's rules and characters. `width(font, code)` is a character's advance in DVI
/// units (from its TFM); `None` advances half the font's size.
pub fn page(
    dvi: &Dvi,
    data: &[u8],
    index: usize,
    width: &dyn Fn(u32, u32) -> Option<i64>,
) -> Result<Vec<Item>> {
    let start = *dvi
        .pages
        .get(index)
        .ok_or_else(|| Error::Malformed("no such page".into()))?;
    let mut r = R { d: data, at: start };
    let mut items = Vec::new();
    let (mut h, mut v, mut w, mut x, mut y, mut z) = (0i64, 0i64, 0i64, 0i64, 0i64, 0i64);
    let mut stack: Vec<[i64; 6]> = Vec::new();
    let mut font: Option<u32> = None;
    let adv = |font: Option<u32>, c: u32| -> i64 {
        font.map_or(0, |f| {
            width(f, c).unwrap_or_else(|| dvi.fonts.get(&f).map_or(0, |d| d.scale as i64 / 2))
        })
    };
    loop {
        let cmd = match next(&mut r) {
            Ok(c) => c,
            Err(Error::Truncated) => break,
            Err(e) => return Err(e),
        };
        if items.len() > MAX_ITEMS {
            break;
        }
        match cmd {
            Cmd::Set(c) | Cmd::Put(c) => {
                if let Some(f) = font {
                    items.push(Item::Char {
                        font: f,
                        code: c,
                        x: h,
                        y: v,
                    });
                }
                if matches!(cmd, Cmd::Set(_)) {
                    h += adv(font, c);
                }
            }
            Cmd::SetRule(a, b) | Cmd::PutRule(a, b) => {
                if a > 0 && b > 0 {
                    items.push(Item::Rule {
                        x: h,
                        y: v,
                        w: b as i64,
                        h: a as i64,
                    });
                }
                if matches!(cmd, Cmd::SetRule(..)) {
                    h += b as i64;
                }
            }
            Cmd::Push => {
                if stack.len() >= MAX_STACK {
                    return Err(Error::Malformed("pushes nested too deeply".into()));
                }
                stack.push([h, v, w, x, y, z]);
            }
            Cmd::Pop => {
                let s = stack
                    .pop()
                    .ok_or_else(|| Error::Malformed("pop without push".into()))?;
                [h, v, w, x, y, z] = s;
            }
            Cmd::Right(d) => h += d as i64,
            Cmd::W(d) => {
                if let Some(d) = d {
                    w = d as i64;
                }
                h += w;
            }
            Cmd::X(d) => {
                if let Some(d) = d {
                    x = d as i64;
                }
                h += x;
            }
            Cmd::Down(d) => v += d as i64,
            Cmd::Y(d) => {
                if let Some(d) = d {
                    y = d as i64;
                }
                v += y;
            }
            Cmd::Z(d) => {
                if let Some(d) = d {
                    z = d as i64;
                }
                v += z;
            }
            Cmd::Font(f) => font = Some(f),
            Cmd::Eop | Cmd::Post => break,
            Cmd::Bop(_) if r.at - 45 != start => break,
            _ => {}
        }
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_reads() {
        let d = [0xFF, 0xFE, 0x00, 0x80];
        let mut r = R { d: &d, at: 0 };
        assert_eq!(r.s(1).unwrap(), -1);
        assert_eq!(r.s(1).unwrap(), -2);
        assert_eq!(r.s(2).unwrap(), 128);
    }

    #[test]
    fn paper() {
        let p = papersize(b"papersize=8.5in,11in", 25_400_000, 473_628_672).unwrap();
        // 8.5 in = 614.295 pt = 40 258 437 sp.
        assert!((p.0 - 40_258_437).abs() < 50, "{p:?}");
    }
}
