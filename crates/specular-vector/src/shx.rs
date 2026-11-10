//! Own reader of compiled AutoCAD shape fonts (SHX), written from the published shape
//! description (AutoCAD Customization Guide, "Shape Descriptions" and "Special Codes"):
//! shape fonts (`AutoCAD-86 shapes 1.0`/`1.1`), Big Fonts with escape ranges (and extended
//! Big Fonts), and Unicode fonts (`AutoCAD-86 unifont 1.0`).
//!
//! A shape is a little pen program: vectors in 16 directions, displacements, octant,
//! fractional and bulge arcs, a location stack and subshapes. [`ShxFont::render`] runs it
//! into a [`BezPath`] of strokes (shape units, y up) and the pen's end, which is where the
//! next character starts. Fonts are never bundled: they come from the user's folders and
//! the drawing's own folder ([`find`]).

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use kurbo::{Arc as KArc, BezPath, Point, Vec2};

/// Which of the three compiled layouts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShxKind {
    /// A shape or text font: one-byte character codes.
    Shapes,
    /// A Big Font: two-byte codes whose lead byte is in an escape range.
    Bigfont,
    /// A Unicode font: shape numbers are code points.
    Unifont,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShxError {
    /// Not a compiled shape file.
    NotShx,
    /// A shape file that ends early or whose index points outside it.
    Damaged(&'static str),
}

impl fmt::Display for ShxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShxError::NotShx => f.write_str("not a compiled AutoCAD shape font"),
            ShxError::Damaged(m) => write!(f, "damaged shape font: {m}"),
        }
    }
}

impl std::error::Error for ShxError {}

/// A parsed SHX file.
#[derive(Debug)]
pub struct ShxFont {
    pub kind: ShxKind,
    /// The font's description (shape 0's name), e.g. "Simplex 1.0".
    pub name: String,
    /// Height of capitals above the baseline and of descenders below it, in shape units.
    pub above: u8,
    pub below: u8,
    /// 0 horizontal only, 2 also vertical.
    pub modes: u8,
    /// Big Font escape ranges: lead bytes that start a two-byte code.
    pub ranges: Vec<(u16, u16)>,
    /// Extended Big Font: the character box (height, width) primitives are scaled to.
    pub extended: Option<(u8, u8)>,
    /// Shape number → specification bytes (without the name).
    shapes: HashMap<u32, Box<[u8]>>,
}

/// Most shapes a file may hold (a Big Font has some tens of thousands).
const MAX_SHAPES: usize = 200_000;

fn u16le(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(at)?, *b.get(at + 1)?]))
}

fn u32le(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// Splits a shape's bytes into its name and specification.
fn split_name(b: &[u8]) -> (&[u8], &[u8]) {
    match b.iter().position(|&c| c == 0) {
        Some(i) => (&b[..i], &b[i + 1..]),
        None => (&[][..], b),
    }
}

/// Whether `head` starts like a compiled shape file.
pub fn is_shx(head: &[u8]) -> bool {
    head.starts_with(b"AutoCAD-86 ")
}

impl ShxFont {
    /// Parses a compiled shape, Big Font or Unicode font file.
    pub fn parse(b: &[u8]) -> Result<ShxFont, ShxError> {
        if !is_shx(b) {
            return Err(ShxError::NotShx);
        }
        // The signature line ends with CR LF and a Ctrl-Z.
        let end = b
            .windows(3)
            .take(64)
            .position(|w| w == b"\r\n\x1a")
            .ok_or(ShxError::NotShx)?;
        let sig = String::from_utf8_lossy(&b[..end]).to_ascii_lowercase();
        let at = end + 3;
        let damaged = ShxError::Damaged;
        let mut f = ShxFont {
            kind: ShxKind::Shapes,
            name: String::new(),
            above: 0,
            below: 0,
            modes: 0,
            ranges: Vec::new(),
            extended: None,
            shapes: HashMap::new(),
        };
        let mut info: Option<Vec<u8>> = None;
        if sig.contains(" shapes ") {
            // First and last number, then the count, the index and the shapes in order.
            let n = usize::from(u16le(b, at + 4).ok_or(damaged("no index"))?);
            let mut idx = at + 6;
            let mut data = idx + n * 4;
            if data > b.len() {
                return Err(damaged("index past the end"));
            }
            for _ in 0..n {
                let num = u16le(b, idx).ok_or(damaged("index"))?;
                let len = usize::from(u16le(b, idx + 2).ok_or(damaged("index"))?);
                idx += 4;
                let bytes = b
                    .get(data..data + len)
                    .ok_or(damaged("shape past the end"))?;
                data += len;
                if num == 0 {
                    info = Some(bytes.to_vec());
                } else {
                    let (_, spec) = split_name(bytes);
                    f.shapes.insert(u32::from(num), spec.into());
                }
            }
        } else if sig.contains(" bigfont ") {
            f.kind = ShxKind::Bigfont;
            let n = usize::from(u16le(b, at + 2).ok_or(damaged("no index"))?);
            let nr = usize::from(u16le(b, at + 4).ok_or(damaged("no index"))?);
            if n > MAX_SHAPES {
                return Err(damaged("too many shapes"));
            }
            let mut p = at + 6;
            for _ in 0..nr {
                let lo = u16le(b, p).ok_or(damaged("escape ranges"))?;
                let hi = u16le(b, p + 2).ok_or(damaged("escape ranges"))?;
                f.ranges.push((lo.min(hi), lo.max(hi)));
                p += 4;
            }
            for _ in 0..n {
                let num = u16le(b, p).ok_or(damaged("index"))?;
                let len = usize::from(u16le(b, p + 2).ok_or(damaged("index"))?);
                let off = u32le(b, p + 4).ok_or(damaged("index"))? as usize;
                p += 8;
                if num == 0 && len == 0 && off == 0 {
                    continue;
                }
                let bytes = b.get(off..off + len).ok_or(damaged("shape past the end"))?;
                if num == 0 {
                    info = Some(bytes.to_vec());
                } else {
                    // Big Font shapes are stored without their names.
                    f.shapes.insert(u32::from(num), bytes.into());
                }
            }
        } else if sig.contains(" unifont ") {
            f.kind = ShxKind::Unifont;
            let n = u32le(b, at).ok_or(damaged("no count"))? as usize;
            if n > MAX_SHAPES {
                return Err(damaged("too many shapes"));
            }
            let ilen = usize::from(u16le(b, at + 4).ok_or(damaged("no font information"))?);
            let mut p = at + 6;
            info = Some(
                b.get(p..p + ilen)
                    .ok_or(damaged("font information"))?
                    .to_vec(),
            );
            p += ilen;
            for _ in 1..n {
                let (Some(num), Some(len)) = (u16le(b, p), u16le(b, p + 2)) else {
                    break;
                };
                let len = usize::from(len);
                let Some(bytes) = b.get(p + 4..p + 4 + len) else {
                    return Err(damaged("shape past the end"));
                };
                p += 4 + len;
                let (_, spec) = split_name(bytes);
                f.shapes.insert(u32::from(num), spec.into());
            }
        } else {
            return Err(ShxError::NotShx);
        }
        let info = info.ok_or(damaged("no font information"))?;
        let (name, rest) = split_name(&info);
        f.name = String::from_utf8_lossy(name).trim().to_string();
        if f.kind == ShxKind::Bigfont && rest.len() >= 5 {
            // Extended: height, 0, modes, width, 0.
            f.above = rest[0];
            f.modes = rest[2];
            f.extended = Some((rest[0], rest[3]));
        } else {
            f.above = *rest.first().unwrap_or(&0);
            f.below = *rest.get(1).unwrap_or(&0);
            f.modes = *rest.get(2).unwrap_or(&0);
        }
        if f.above == 0 {
            // A font that never says: the usual 3:1 split of a 9-unit cell.
            f.above = 6;
            f.below = 2;
        }
        Ok(f)
    }

    /// Number of shapes (shape 0 not counted).
    pub fn len(&self) -> usize {
        self.shapes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.shapes.is_empty()
    }

    pub fn has(&self, code: u32) -> bool {
        self.shapes.contains_key(&code)
    }

    /// Whether `lead` starts a two-byte code (Big Fonts).
    pub fn is_lead(&self, lead: u8) -> bool {
        self.ranges
            .iter()
            .any(|&(a, b)| (a..=b).contains(&u16::from(lead)))
    }

    /// Draws shape `code` (horizontal text): the strokes and where the pen ends.
    pub fn render(&self, code: u32) -> Option<(BezPath, Point)> {
        let spec = self.shapes.get(&code)?;
        let mut r = Run::new(self);
        r.exec(spec, 0);
        Some((r.path, r.pos))
    }
}

/// The 16 directions of a vector byte's low nibble.
const DIRS: [(f64, f64); 16] = [
    (1.0, 0.0),
    (1.0, 0.5),
    (1.0, 1.0),
    (0.5, 1.0),
    (0.0, 1.0),
    (-0.5, 1.0),
    (-1.0, 1.0),
    (-1.0, 0.5),
    (-1.0, 0.0),
    (-1.0, -0.5),
    (-1.0, -1.0),
    (-0.5, -1.0),
    (0.0, -1.0),
    (0.5, -1.0),
    (1.0, -1.0),
    (1.0, -0.5),
];

/// Subshape nesting a font may use.
const MAX_DEPTH: u32 = 8;
/// Commands one character may run (subshapes included).
const MAX_STEPS: usize = 20_000;

struct Run<'f> {
    font: &'f ShxFont,
    pos: Point,
    pen: bool,
    /// Whether the path's current subpath ends at `pos`.
    open: bool,
    scale: f64,
    /// Extra scale of an extended Big Font primitive (x, y).
    box_scale: Vec2,
    stack: Vec<Point>,
    path: BezPath,
    steps: usize,
}

fn i8b(s: &[u8], i: usize) -> f64 {
    f64::from(*s.get(i).unwrap_or(&0) as i8)
}

fn u8b(s: &[u8], i: usize) -> u8 {
    *s.get(i).unwrap_or(&0)
}

/// Start octant, octant count (1–8) and direction of a `(-)0SC` byte.
fn octants(b: u8) -> (f64, f64, f64) {
    let v = b as i8;
    let dir = if v < 0 { -1.0 } else { 1.0 };
    let a = v.unsigned_abs();
    let s = f64::from((a >> 4) & 7);
    let c = match a & 7 {
        0 => 8.0,
        c => f64::from(c),
    };
    (s, c, dir)
}

impl<'f> Run<'f> {
    fn new(font: &'f ShxFont) -> Run<'f> {
        Run {
            font,
            pos: Point::ZERO,
            pen: true,
            open: false,
            scale: 1.0,
            box_scale: Vec2::new(1.0, 1.0),
            stack: Vec::new(),
            path: BezPath::new(),
            steps: 0,
        }
    }

    fn d(&self, x: f64, y: f64) -> Vec2 {
        Vec2::new(
            x * self.scale * self.box_scale.x,
            y * self.scale * self.box_scale.y,
        )
    }

    fn start_stroke(&mut self) {
        if !self.open {
            self.path.move_to(self.pos);
            self.open = true;
        }
    }

    fn line(&mut self, v: Vec2) {
        let to = self.pos + v;
        if self.pen && v != Vec2::ZERO {
            self.start_stroke();
            self.path.line_to(to);
        } else if !self.pen {
            self.open = false;
        }
        self.pos = to;
    }

    /// An arc about `c` of radius vector `r` (x, y), from angle `a0` sweeping `sweep`
    /// (radians, positive counter-clockwise).
    fn arc(&mut self, c: Point, r: Vec2, a0: f64, sweep: f64) {
        let end = c + Vec2::new(r.x * (a0 + sweep).cos(), r.y * (a0 + sweep).sin());
        if self.pen && r.x.abs() > 0.0 {
            self.start_stroke();
            let a = KArc::new(c, r, a0, sweep, 0.0);
            a.to_cubic_beziers(0.01, |p1, p2, p| self.path.curve_to(p1, p2, p));
        } else if !self.pen {
            self.open = false;
        }
        self.pos = end;
    }

    fn bulge(&mut self, dx: f64, dy: f64, b: f64) {
        let v = self.d(dx, dy);
        if b == 0.0 || v == Vec2::ZERO {
            self.line(v);
            return;
        }
        // Bulge = tan(sweep / 4), stored × 127.
        let t = (b / 127.0).clamp(-1.0, 1.0);
        let sweep = 4.0 * t.atan();
        let chord = v.hypot();
        let mid = self.pos + v * 0.5;
        let normal = Vec2::new(-v.y, v.x) / chord;
        let c = mid + normal * (chord / 2.0 * (1.0 - t * t) / (2.0 * t));
        let r = (self.pos - c).hypot();
        let a0 = (self.pos - c).atan2();
        self.arc(c, Vec2::new(r, r), a0, sweep);
        // Land exactly on the end point.
        self.pos = mid + v * 0.5;
    }

    /// Runs one shape's specification.
    fn exec(&mut self, s: &[u8], depth: u32) {
        let mut i = 0;
        while i < s.len() {
            match self.step(s, i, true, depth) {
                Some(next) => i = next,
                None => return,
            }
        }
    }

    /// One command at `i`: the next command's index, `None` at the end of the shape.
    /// With `run` false the command is only skipped (code 14 in horizontal text).
    fn step(&mut self, s: &[u8], i: usize, run: bool, depth: u32) -> Option<usize> {
        self.steps += 1;
        if self.steps > MAX_STEPS {
            return None;
        }
        let b = s[i];
        let len = b >> 4;
        if len != 0 {
            if run {
                let (x, y) = DIRS[usize::from(b & 15)];
                let v = self.d(x * f64::from(len), y * f64::from(len));
                self.line(v);
            }
            return Some(i + 1);
        }
        match b {
            0 => None,
            1 => {
                if run {
                    self.pen = true;
                }
                Some(i + 1)
            }
            2 => {
                if run {
                    self.pen = false;
                    self.open = false;
                }
                Some(i + 1)
            }
            3 | 4 => {
                let f = f64::from(u8b(s, i + 1));
                if run && f > 0.0 {
                    if b == 3 {
                        self.scale /= f;
                    } else {
                        self.scale *= f;
                    }
                }
                Some(i + 2)
            }
            5 => {
                if run && self.stack.len() < 16 {
                    self.stack.push(self.pos);
                }
                Some(i + 1)
            }
            6 => {
                if run && let Some(p) = self.stack.pop() {
                    self.pos = p;
                    self.open = false;
                }
                Some(i + 1)
            }
            7 => self.subshape(s, i, run, depth),
            8 => {
                if run {
                    let v = self.d(i8b(s, i + 1), i8b(s, i + 2));
                    self.line(v);
                }
                Some(i + 3)
            }
            9 => {
                let mut j = i + 1;
                while j + 1 < s.len() {
                    let (x, y) = (i8b(s, j), i8b(s, j + 1));
                    j += 2;
                    if x == 0.0 && y == 0.0 {
                        break;
                    }
                    if run {
                        let v = self.d(x, y);
                        self.line(v);
                    }
                }
                Some(j)
            }
            10 => {
                if run {
                    let r = f64::from(u8b(s, i + 1)) * self.scale;
                    let (so, c, dir) = octants(u8b(s, i + 2));
                    let a0 = so * std::f64::consts::FRAC_PI_4;
                    self.octant_arc(r, a0, dir * c * std::f64::consts::FRAC_PI_4);
                }
                Some(i + 3)
            }
            11 => {
                if run {
                    let start = f64::from(u8b(s, i + 1));
                    let end = f64::from(u8b(s, i + 2));
                    let r =
                        (f64::from(u8b(s, i + 3)) * 256.0 + f64::from(u8b(s, i + 4))) * self.scale;
                    let (so, c, dir) = octants(u8b(s, i + 5));
                    let oct = std::f64::consts::FRAC_PI_4;
                    // The end offset counts from the last octant boundary crossed.
                    let whole = if end != 0.0 { c - 1.0 } else { c };
                    let a0 = so * oct + dir * start * oct / 256.0;
                    let a1 = (so + dir * whole) * oct + dir * end * oct / 256.0;
                    self.octant_arc(r, a0, a1 - a0);
                }
                Some(i + 6)
            }
            12 => {
                if run {
                    self.bulge(i8b(s, i + 1), i8b(s, i + 2), i8b(s, i + 3));
                }
                Some(i + 4)
            }
            13 => {
                let mut j = i + 1;
                while j + 1 < s.len() {
                    let (x, y) = (i8b(s, j), i8b(s, j + 1));
                    if x == 0.0 && y == 0.0 {
                        j += 2;
                        break;
                    }
                    if run {
                        self.bulge(x, y, i8b(s, j + 2));
                    }
                    j += 3;
                }
                Some(j)
            }
            14 => {
                // Vertical text only: skip the next command.
                if i + 1 >= s.len() {
                    return None;
                }
                let next = self.step(s, i + 1, false, depth)?;
                Some(next)
            }
            _ => Some(i + 1),
        }
    }

    /// An arc whose start is the pen, at angle `a0` on a circle of radius `r`.
    fn octant_arc(&mut self, r: f64, a0: f64, sweep: f64) {
        let rv = Vec2::new(r * self.box_scale.x, r * self.box_scale.y);
        let c = self.pos - Vec2::new(rv.x * a0.cos(), rv.y * a0.sin());
        self.arc(c, rv, a0, sweep);
    }

    fn subshape(&mut self, s: &[u8], i: usize, run: bool, depth: u32) -> Option<usize> {
        let font = self.font;
        let (code, next, placed) = match font.kind {
            ShxKind::Unifont => (
                u32::from(u16::from_be_bytes([u8b(s, i + 1), u8b(s, i + 2)])),
                i + 3,
                None,
            ),
            ShxKind::Bigfont if u8b(s, i + 1) == 0 => {
                // Extended Big Font: 7, 0, primitive (2 bytes), x, y, width, height.
                let code = u32::from(u16::from_be_bytes([u8b(s, i + 2), u8b(s, i + 3)]));
                let place = (
                    i8b(s, i + 4),
                    i8b(s, i + 5),
                    f64::from(u8b(s, i + 6)),
                    f64::from(u8b(s, i + 7)),
                );
                (code, i + 8, Some(place))
            }
            _ => (u32::from(u8b(s, i + 1)), i + 2, None),
        };
        if !run || depth >= MAX_DEPTH {
            return Some(next);
        }
        let Some(spec) = font.shapes.get(&code) else {
            return Some(next);
        };
        match placed {
            None => self.exec(spec, depth + 1),
            Some((x, y, w, h)) => {
                // The primitive is scaled from the font's character box to w × h at (x, y),
                // and the pen comes back to where it was.
                let back = self.pos;
                let cell = f64::from(font.extended.map_or(font.above, |e| e.0).max(1));
                let saved = (self.box_scale, self.pen);
                self.pos = back + self.d(x, y);
                self.open = false;
                self.box_scale = Vec2::new(
                    saved.0.x * if w > 0.0 { w / cell } else { 1.0 },
                    saved.0.y * if h > 0.0 { h / cell } else { 1.0 },
                );
                self.pen = true;
                self.exec(spec, depth + 1);
                (self.box_scale, self.pen) = saved;
                self.pos = back;
                self.open = false;
            }
        }
        Some(next)
    }
}

/// A text style's SHX fonts, found and loaded: the main font, the Big Font, and the code
/// page that turns text back into Big Font codes.
#[derive(Debug)]
pub struct ShxStyle {
    pub main: Option<Arc<ShxFont>>,
    pub big: Option<Arc<ShxFont>>,
    pub encoding: Option<&'static encoding_rs::Encoding>,
}

impl ShxStyle {
    /// The font and shape number that draw `c`, if any.
    pub fn lookup(&self, c: char) -> Option<(&ShxFont, u32)> {
        let u = c as u32;
        if let Some(big) = &self.big
            && !c.is_ascii()
            && let Some(code) = self.big_code(big, c)
        {
            return Some((big, code));
        }
        if let Some(m) = &self.main {
            let hit = |code: u32| m.has(code).then_some((&**m, code));
            let found = match m.kind {
                ShxKind::Unifont => hit(u).or_else(|| match c {
                    // AutoCAD's own symbol points for %%c.
                    '⌀' | '∅' => hit(0x2205).or_else(|| hit(0x2300)),
                    _ => None,
                }),
                _ => {
                    let legacy = match c {
                        '°' => hit(127),
                        '±' => hit(128),
                        '⌀' | '∅' => hit(129),
                        _ => None,
                    };
                    if c.is_ascii() {
                        hit(u)
                    } else {
                        legacy.or_else(|| self.single_byte(c).and_then(hit))
                    }
                }
            };
            if found.is_some() {
                return found;
            }
        }
        // ASCII a Big Font has (some carry their own Latin).
        self.big
            .as_ref()
            .and_then(|b| b.has(u).then_some((&**b, u)))
    }

    fn single_byte(&self, c: char) -> Option<u32> {
        let mut buf = [0u8; 4];
        let s = c.encode_utf8(&mut buf);
        match self.encoding {
            Some(e) => {
                let (out, _, bad) = e.encode(s);
                (!bad && out.len() == 1).then(|| u32::from(out[0]))
            }
            None => (u32::from(c) < 256).then_some(u32::from(c)),
        }
    }

    fn big_code(&self, big: &ShxFont, c: char) -> Option<u32> {
        let mut buf = [0u8; 4];
        let s = c.encode_utf8(&mut buf);
        let e = self.encoding?;
        let (out, _, bad) = e.encode(s);
        if bad {
            return None;
        }
        let code = match *out {
            [lead, trail] if big.is_lead(lead) => u32::from(lead) << 8 | u32::from(trail),
            [one] => u32::from(one),
            _ => return None,
        };
        big.has(code).then_some(code)
    }

    /// The main font's cap height, or the Big Font's.
    pub fn above(&self) -> f64 {
        let f = self.main.as_ref().or(self.big.as_ref());
        f.map_or(6.0, |f| f64::from(f.above.max(1)))
    }

    pub fn below(&self) -> f64 {
        let f = self.main.as_ref().or(self.big.as_ref());
        f.map_or(2.0, |f| f64::from(f.below))
    }
}

/// The folders the user added for SHX fonts (Settings), searched after the drawing's own.
static FOLDERS: RwLock<Vec<PathBuf>> = RwLock::new(Vec::new());

/// Sets the user's SHX font folders.
pub fn set_folders(dirs: Vec<PathBuf>) {
    if let Ok(mut f) = FOLDERS.write() {
        *f = dirs;
    }
}

/// The user's SHX font folders.
pub fn folders() -> Vec<PathBuf> {
    FOLDERS.read().map(|f| f.clone()).unwrap_or_default()
}

/// Splits the setting's text (folders separated by `;`, or by new lines) into folders.
pub fn parse_folders(text: &str) -> Vec<PathBuf> {
    text.split([';', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// The file name a style names, with `.shx` added when it has no extension; `None` for
/// a TrueType or other file.
pub fn shx_file_name(file: &str) -> Option<String> {
    let name = file.rsplit(['/', '\\']).next().unwrap_or(file).trim();
    if name.is_empty() {
        return None;
    }
    let lower = name.to_ascii_lowercase();
    match lower.rfind('.') {
        Some(i) if &lower[i..] == ".shx" => Some(name.to_string()),
        Some(_) => None,
        None => Some(format!("{name}.shx")),
    }
}

/// Finds `name` (case-insensitively) in `dirs`, in order.
pub fn find(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    let want = name.to_ascii_lowercase();
    for d in dirs {
        let exact = d.join(name);
        if exact.is_file() {
            return Some(exact);
        }
        let Ok(rd) = std::fs::read_dir(d) else {
            continue;
        };
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().to_ascii_lowercase() == want && e.path().is_file() {
                return Some(e.path());
            }
        }
    }
    None
}

/// Loaded fonts by path, size and modification time: each file is read once.
type Loaded = HashMap<PathBuf, (u64, Option<std::time::SystemTime>, Option<Arc<ShxFont>>)>;

fn loaded() -> &'static Mutex<Loaded> {
    static L: OnceLock<Mutex<Loaded>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Largest SHX file read (the largest Big Fonts are a few MB).
const MAX_FILE: u64 = 64 << 20;

/// Loads (once) the SHX font at `path`; `None` when it is not a readable shape font.
pub fn load(path: &Path) -> Option<Arc<ShxFont>> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > MAX_FILE {
        return None;
    }
    let stamp = (meta.len(), meta.modified().ok());
    if let Ok(m) = loaded().lock()
        && let Some((len, t, f)) = m.get(path)
        && (*len, *t) == stamp
    {
        return f.clone();
    }
    let font = std::fs::read(path)
        .ok()
        .and_then(|b| ShxFont::parse(&b).ok())
        .map(Arc::new);
    if let Ok(mut m) = loaded().lock() {
        m.insert(path.to_path_buf(), (stamp.0, stamp.1, font.clone()));
    }
    font
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A compiled shapes 1.0 file of `shapes` (number, name, spec) and font info.
    pub(crate) fn shapes_file(above: u8, below: u8, shapes: &[(u16, &str, &[u8])]) -> Vec<u8> {
        let mut entries: Vec<(u16, Vec<u8>)> = vec![(0, {
            let mut v = b"Test font".to_vec();
            v.extend_from_slice(&[0, above, below, 0, 0]);
            v
        })];
        for (n, name, spec) in shapes {
            let mut v = name.as_bytes().to_vec();
            v.push(0);
            v.extend_from_slice(spec);
            entries.push((*n, v));
        }
        let mut out = b"AutoCAD-86 shapes 1.0\r\n\x1a".to_vec();
        let first = entries.iter().map(|e| e.0).min().unwrap();
        let last = entries.iter().map(|e| e.0).max().unwrap();
        for v in [first, last, entries.len() as u16] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for (n, v) in &entries {
            out.extend_from_slice(&n.to_le_bytes());
            out.extend_from_slice(&(v.len() as u16).to_le_bytes());
        }
        for (_, v) in &entries {
            out.extend_from_slice(v);
        }
        out
    }

    fn pts(p: &BezPath) -> Vec<(i64, i64)> {
        p.elements()
            .iter()
            .filter_map(|e| match e {
                kurbo::PathEl::MoveTo(p) | kurbo::PathEl::LineTo(p) => Some(*p),
                kurbo::PathEl::CurveTo(_, _, p) => Some(*p),
                _ => None,
            })
            .map(|p| ((p.x * 100.0).round() as i64, (p.y * 100.0).round() as i64))
            .collect()
    }

    #[test]
    fn vectors_pen_and_advance() {
        // "L": up 6 (pen down from the start), pen up, back down, right 4, pen up to 6.
        let f = ShxFont::parse(&shapes_file(
            6,
            2,
            &[(76, "L", &[2, 0x64, 1, 0x6C, 0x40, 2, 0x20, 0])],
        ))
        .unwrap();
        assert_eq!(f.kind, ShxKind::Shapes);
        assert_eq!((f.above, f.below, f.name.as_str()), (6, 2, "Test font"));
        let (p, end) = f.render(76).unwrap();
        assert_eq!(pts(&p), vec![(0, 600), (0, 0), (400, 0)]);
        assert_eq!(end, Point::new(6.0, 0.0));
        // Pen down at the start of a shape.
        let f = ShxFont::parse(&shapes_file(6, 2, &[(45, "-", &[0x20, 0])])).unwrap();
        assert_eq!(pts(&f.render(45).unwrap().0), vec![(0, 0), (200, 0)]);
    }

    #[test]
    fn special_codes() {
        let spec: &[u8] = &[
            2, 8, 2, 0xFE, // displacement (2, -2)
            1, 3, 2, 0x40, // divide by 2: 4 → 2
            4, 2, 9, 1, 1, 0, 2, 0, 0, // multiply back; two displacements
            5, 0x14, 6, // push, up 1, pop
            0x10, 0,
        ];
        let f = ShxFont::parse(&shapes_file(6, 2, &[(65, "A", spec)])).unwrap();
        let (p, end) = f.render(65).unwrap();
        assert_eq!(
            pts(&p),
            vec![
                (200, -200),
                (400, -200),
                (500, -100),
                (500, 100),
                (500, 200),
                (500, 100),
                (600, 100)
            ]
        );
        assert_eq!(end, Point::new(6.0, 1.0));
    }

    #[test]
    fn arcs() {
        // Octant arc: radius 2 from octant 0 through 4 octants (a half circle, CCW).
        let f = ShxFont::parse(&shapes_file(6, 2, &[(79, "O", &[10, 2, 0x04, 0])])).unwrap();
        let (_, end) = f.render(79).unwrap();
        assert!((end - Point::new(-4.0, 0.0)).hypot() < 1e-9, "{end:?}");
        // The guide's example: 55° to 95°, radius 3.
        let f = ShxFont::parse(&shapes_file(
            6,
            2,
            &[(80, "P", &[11, 56, 28, 0, 3, 0x12, 0])],
        ))
        .unwrap();
        let (_, end) = f.render(80).unwrap();
        let a0 = (45.0 + 56.0 * 45.0 / 256.0f64).to_radians();
        let a1 = (90.0 + 28.0 * 45.0 / 256.0f64).to_radians();
        let c = Point::new(-3.0 * a0.cos(), -3.0 * a0.sin());
        let want = c + Vec2::new(3.0 * a1.cos(), 3.0 * a1.sin());
        assert!((end - want).hypot() < 1e-9, "{end:?} {want:?}");
        // Bulge 127 is a half circle; the guide's S: 13,(0,5,127),(0,5,-127),(0,0).
        let f = ShxFont::parse(&shapes_file(
            6,
            2,
            &[(83, "S", &[13, 0, 5, 127, 0, 5, 0x81, 0, 0, 0])],
        ))
        .unwrap();
        let (p, end) = f.render(83).unwrap();
        assert!((end - Point::new(0.0, 10.0)).hypot() < 1e-9);
        let bb = kurbo::Shape::bounding_box(&p);
        // CCW from (0,0) to (0,5) bulges right, then the CW half bulges left.
        assert!(
            (bb.x0 + 2.5).abs() < 0.01 && (bb.x1 - 2.5).abs() < 0.01,
            "{bb:?}"
        );
    }

    #[test]
    fn subshapes_and_vertical_only() {
        let f = ShxFont::parse(&shapes_file(
            6,
            2,
            &[
                (1, "bar", &[0x14, 0]),
                // Vertical-only displacement skipped, then the bar, then right 2.
                (66, "B", &[14, 8, 0x10, 0x10, 7, 1, 0x20, 0]),
            ],
        ))
        .unwrap();
        let (p, end) = f.render(66).unwrap();
        assert_eq!(pts(&p), vec![(0, 0), (0, 100), (200, 100)]);
        assert_eq!(end, Point::new(2.0, 1.0));
    }

    #[test]
    fn hostile_files_fail_cleanly() {
        let good = shapes_file(6, 2, &[(65, "A", &[7, 65, 0x14, 0])]);
        // A shape calling itself stops at the nesting limit.
        let f = ShxFont::parse(&good).unwrap();
        assert!(f.render(65).is_some());
        for n in 0..good.len() {
            let _ = ShxFont::parse(&good[..n]);
        }
        let mut x: u32 = 0x9e37_79b9;
        for _ in 0..2000 {
            let mut b = good.clone();
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            let at = 24 + (x as usize % (b.len() - 24));
            b[at] = (x >> 24) as u8;
            if let Ok(f) = ShxFont::parse(&b) {
                for c in 0..=255 {
                    let _ = f.render(c);
                }
            }
        }
        assert_eq!(ShxFont::parse(b"hello").unwrap_err(), ShxError::NotShx);
    }

    #[test]
    fn file_names() {
        assert_eq!(shx_file_name("romans").as_deref(), Some("romans.shx"));
        assert_eq!(
            shx_file_name("C:\\Fonts\\RomanS.SHX").as_deref(),
            Some("RomanS.SHX")
        );
        assert_eq!(shx_file_name("arial.ttf"), None);
        assert_eq!(
            parse_folders(" /a ; ;/b c\n/d"),
            vec![
                PathBuf::from("/a"),
                PathBuf::from("/b c"),
                PathBuf::from("/d")
            ]
        );
    }
}
