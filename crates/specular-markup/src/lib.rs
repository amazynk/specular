//! Markup on drawings and images, kept beside the file and never in it.
//!
//! - [`Markup`]: a revision cloud, a text, an arrow (leader) with an optional label, a stamp
//!   (Approved, Rejected, Reviewed or the user's own words, with the date and the user's name),
//!   a length or an area with its value as measured, or a photo pinned to a point (its path;
//!   the photo is never copied). Each has a colour and a line width.
//! - Coordinates are *sheet units*, y down: for a PDF page the page as displayed (points,
//!   origin at the top left, rotation and crop box applied); for anything else the image's
//!   pixels. Sizes (line width, text height) are in the same units, so markup scales with the
//!   sheet.
//! - [`Doc`]: the markup of every page of a file, with undo and redo per edit.
//! - The sidecar ([`sidecar_path`]): `<file>.markup.xfdf` for PDFs, XFDF (ISO 19444-1) in the
//!   page's user space, which Acrobat and other PDF tools import; `<file>.markup.json`
//!   otherwise ([`json`]). Only the sidecar is ever written.
//! - [`pdf::export`]: the original PDF with the markup added as real annotations, each with
//!   its appearance (an incremental update; the page itself is untouched, so it keeps its
//!   vectors and scale).
//! - [`align`] and [`compare`]: two revisions of a sheet aligned (two point pairs, or
//!   automatically) and shown in two colours or as a difference, with the changed regions.

pub mod align;
pub mod cloud;
pub mod compare;
pub mod docx;
pub mod helvetica;
pub mod json;
pub mod pdf;
pub mod report;
pub mod xfdf;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// A point in sheet units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct P {
    pub x: f64,
    pub y: f64,
}

impl P {
    pub const fn new(x: f64, y: f64) -> P {
        P { x, y }
    }
    pub fn dist(self, o: P) -> f64 {
        (self.x - o.x).hypot(self.y - o.y)
    }
}

/// What a sheet's units are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space {
    /// A PDF page as displayed: points, y down.
    PdfPage,
    /// An image's pixels.
    Pixels,
}

/// A stamp's words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stamp {
    Approved,
    Rejected,
    Reviewed,
    /// The user's own words ([`Markup::text`]).
    Custom,
}

impl Stamp {
    pub const ALL: [Stamp; 4] = [
        Stamp::Approved,
        Stamp::Rejected,
        Stamp::Reviewed,
        Stamp::Custom,
    ];

    /// The name in sidecars.
    pub fn id(self) -> &'static str {
        match self {
            Stamp::Approved => "approved",
            Stamp::Rejected => "rejected",
            Stamp::Reviewed => "reviewed",
            Stamp::Custom => "custom",
        }
    }

    pub fn from_id(s: &str) -> Option<Stamp> {
        Stamp::ALL.into_iter().find(|k| k.id() == s)
    }

    /// The PDF stamp icon name (`/Name`; ISO 32000-1 Table 181 where there is one).
    pub fn pdf_name(self) -> &'static str {
        match self {
            Stamp::Approved => "Approved",
            Stamp::Rejected => "NotApproved",
            Stamp::Reviewed => "Reviewed",
            Stamp::Custom => "Custom",
        }
    }
}

/// A markup object's geometry.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// A revision cloud around a polygon.
    Cloud(Vec<P>),
    /// Text, its first line's top left corner here.
    Text(P),
    /// An arrow from `tail` to its head at `head`; the label (if any) sits at the tail.
    Arrow { tail: P, head: P },
    /// A stamp centred here.
    Stamp { at: P, kind: Stamp },
    /// A measured length.
    Length(P, P),
    /// A measured area.
    Area(Vec<P>),
    /// A photo pinned to this point.
    Pin { at: P, photo: PathBuf },
}

impl Shape {
    /// Its name in sidecars.
    pub fn kind(&self) -> &'static str {
        match self {
            Shape::Cloud(_) => "cloud",
            Shape::Text(_) => "text",
            Shape::Arrow { .. } => "arrow",
            Shape::Stamp { .. } => "stamp",
            Shape::Length(..) => "length",
            Shape::Area(_) => "area",
            Shape::Pin { .. } => "pin",
        }
    }

    /// The points that define it.
    pub fn points(&self) -> Vec<P> {
        match self {
            Shape::Cloud(p) | Shape::Area(p) => p.clone(),
            Shape::Text(p) | Shape::Stamp { at: p, .. } | Shape::Pin { at: p, .. } => vec![*p],
            Shape::Arrow { tail, head } => vec![*tail, *head],
            Shape::Length(a, b) => vec![*a, *b],
        }
    }

    /// Every point moved by `f`.
    pub fn map(&mut self, f: impl Fn(P) -> P) {
        match self {
            Shape::Cloud(p) | Shape::Area(p) => p.iter_mut().for_each(|q| *q = f(*q)),
            Shape::Text(p) | Shape::Stamp { at: p, .. } | Shape::Pin { at: p, .. } => *p = f(*p),
            Shape::Arrow { tail, head } => {
                *tail = f(*tail);
                *head = f(*head);
            }
            Shape::Length(a, b) => {
                *a = f(*a);
                *b = f(*b);
            }
        }
    }
}

/// One markup object.
#[derive(Clone, Debug, PartialEq)]
pub struct Markup {
    /// Unique within the file (the PDF annotation's `/NM`).
    pub id: String,
    pub shape: Shape,
    /// sRGB.
    pub colour: [u8; 3],
    /// Line width in sheet units.
    pub width: f64,
    /// Text height in sheet units (a stamp's words, a label, a pin's marker).
    pub size: f64,
    /// A text's words (lines split by `\n`), an arrow's label, a custom stamp's words, a
    /// measurement's value, a pin's caption.
    pub text: String,
    /// Who made it (the name in Settings).
    pub author: String,
    /// When, UTC, `2026-10-24T14:03:00Z`.
    pub date: String,
}

impl Markup {
    /// A new object with a fresh id and today's date.
    pub fn new(shape: Shape, colour: [u8; 3], width: f64, size: f64, author: &str) -> Markup {
        Markup {
            id: new_id(),
            shape,
            colour,
            width,
            size,
            text: String::new(),
            author: author.to_owned(),
            date: now_local(),
        }
    }

    /// Its extent in sheet units (x0, y0, x1, y1), text measured as Helvetica; `stamp` is a
    /// stamp's words.
    pub fn bounds(&self, stamp: &str) -> [f64; 4] {
        self.bounds_with(stamp, &helvetica::width)
    }

    /// [`Markup::bounds`] with text measured by `width`.
    pub fn bounds_with(&self, stamp: &str, width: &TextWidth<'_>) -> [f64; 4] {
        let text_extent = |t: &str, size: f64| text_extent_with(t, size, width);
        let mut b = Bounds::default();
        match &self.shape {
            Shape::Text(at) => {
                let (w, h) = text_extent(&self.text, self.size);
                b.add(*at);
                b.add(P::new(at.x + w, at.y + h));
            }
            Shape::Stamp { at, .. } => {
                let (w, h) = stamp_extent_with(stamp, &self.stamp_line(), self.size, width);
                b.add(P::new(at.x - w / 2.0, at.y - h / 2.0));
                b.add(P::new(at.x + w / 2.0, at.y + h / 2.0));
            }
            Shape::Pin { at, .. } => {
                let r = self.size * 0.6;
                b.add(P::new(at.x - r, at.y - 2.6 * r));
                b.add(P::new(at.x + r, at.y));
            }
            Shape::Cloud(pts) => {
                // The arcs bulge out by up to half their chord.
                for p in pts {
                    b.add(*p);
                }
                let pad = cloud::arc_length(self.width, self.size) * 0.6;
                b.pad(pad);
            }
            s => {
                for p in s.points() {
                    b.add(p);
                }
                if let Shape::Arrow { tail, .. } = s
                    && !self.text.is_empty()
                {
                    let (w, h) = text_extent(&self.text, self.size);
                    b.add(P::new(tail.x - w - self.size * 0.3, tail.y - h));
                    b.add(P::new(tail.x + w + self.size * 0.3, tail.y + h));
                }
                if matches!(s, Shape::Length(..) | Shape::Area(_)) && !self.text.is_empty() {
                    let (w, h) = text_extent(&self.text, self.size);
                    let c = label_anchor(s);
                    b.add(P::new(c.x - w / 2.0, c.y - h * 1.5));
                    b.add(P::new(c.x + w / 2.0, c.y + h * 0.5));
                }
            }
        }
        b.pad(self.width.max(0.0) + self.size * 0.15);
        b.get()
    }

    /// A stamp's second line: the day and the author.
    pub fn stamp_line(&self) -> String {
        let day = day_of(&self.date);
        match (day.is_empty(), self.author.trim().is_empty()) {
            (false, false) => format!("{day} · {}", self.author.trim()),
            (false, true) => day.to_owned(),
            (true, false) => self.author.trim().to_owned(),
            (true, true) => String::new(),
        }
    }
}

/// Where a measurement's value is written: above the middle of a length, at the middle of an
/// area's points.
pub fn label_anchor(s: &Shape) -> P {
    match s {
        Shape::Length(a, b) => P::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0),
        Shape::Area(pts) | Shape::Cloud(pts) if !pts.is_empty() => {
            let n = pts.len() as f64;
            P::new(
                pts.iter().map(|p| p.x).sum::<f64>() / n,
                pts.iter().map(|p| p.y).sum::<f64>() / n,
            )
        }
        s => s.points().first().copied().unwrap_or_default(),
    }
}

/// An arrow's head: its tip at `head`, then the two back corners.
pub fn arrow_head(tail: P, head: P, width: f64, size: f64) -> [P; 3] {
    let len = (width * 5.0).max(size * 0.7);
    let d = tail.dist(head).max(1e-9);
    let (ux, uy) = ((head.x - tail.x) / d, (head.y - tail.y) / d);
    let l = len.min(d);
    let back = P::new(head.x - ux * l, head.y - uy * l);
    let (nx, ny) = (-uy * l * 0.4, ux * l * 0.4);
    [
        head,
        P::new(back.x + nx, back.y + ny),
        P::new(back.x - nx, back.y - ny),
    ]
}

/// A pin's round marker above its point: centre and radius.
pub fn pin_marker(at: P, size: f64) -> (P, f64) {
    let r = size * 0.6;
    (P::new(at.x, at.y - 1.6 * r), r)
}

/// Width and height of `text` (lines split by `\n`) at height `size` in Helvetica: the
/// widest line, and 1.2 × size per line.
pub fn text_extent(text: &str, size: f64) -> (f64, f64) {
    text_extent_with(text, size, &helvetica::width)
}

/// Measures text: the width of a line at a size, bold or not.
pub type TextWidth<'a> = dyn Fn(&str, f64, bool) -> f64 + 'a;

/// [`text_extent`] with lines measured by `width`.
pub fn text_extent_with(text: &str, size: f64, width: &TextWidth<'_>) -> (f64, f64) {
    let lines: Vec<&str> = text.split('\n').collect();
    let w = lines
        .iter()
        .map(|l| width(l, size, false))
        .fold(0.0, f64::max);
    (w, lines.len() as f64 * size * 1.2)
}

/// Stamp layout, shared by the screen and PDF: the frame's padding, the words' height and the
/// second line's height, for a stamp of height `size`.
pub const STAMP_PAD: f64 = 0.35;
pub const STAMP_SUB: f64 = 0.42;

/// A stamp's frame: width and height.
pub fn stamp_extent(words: &str, line: &str, size: f64) -> (f64, f64) {
    stamp_extent_with(words, line, size, &helvetica::width)
}

/// [`stamp_extent`] with text measured by `width`.
pub fn stamp_extent_with(words: &str, line: &str, size: f64, width: &TextWidth<'_>) -> (f64, f64) {
    let pad = size * STAMP_PAD;
    let w = width(words, size, true).max(width(line, size * STAMP_SUB, false));
    let sub = if line.is_empty() {
        0.0
    } else {
        size * STAMP_SUB * 1.3
    };
    (w + 2.0 * pad, size + sub + 2.0 * pad)
}

#[derive(Debug)]
struct Bounds([f64; 4]);

impl Default for Bounds {
    fn default() -> Self {
        Bounds([
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ])
    }
}

impl Bounds {
    fn add(&mut self, p: P) {
        let b = &mut self.0;
        b[0] = b[0].min(p.x);
        b[1] = b[1].min(p.y);
        b[2] = b[2].max(p.x);
        b[3] = b[3].max(p.y);
    }
    fn pad(&mut self, d: f64) {
        let b = &mut self.0;
        b[0] -= d;
        b[1] -= d;
        b[2] += d;
        b[3] += d;
    }
    fn get(&self) -> [f64; 4] {
        if self.0[0].is_finite() {
            self.0
        } else {
            [0.0; 4]
        }
    }
}

/// The markup of one file, by page, with its undo history.
#[derive(Clone, Debug)]
pub struct Doc {
    pub space: Space,
    pages: BTreeMap<u32, Vec<Markup>>,
    undo: Vec<Step>,
    redo: Vec<Step>,
}

/// A page as it was before an edit.
#[derive(Clone, Debug)]
struct Step {
    page: u32,
    before: Vec<Markup>,
}

/// Edits kept for undo.
const UNDO_DEPTH: usize = 200;

impl Doc {
    pub fn new(space: Space) -> Doc {
        Doc {
            space,
            pages: BTreeMap::new(),
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    /// A document of `pages` (read from a sidecar), with no history.
    pub fn with_pages(space: Space, pages: BTreeMap<u32, Vec<Markup>>) -> Doc {
        let mut d = Doc::new(space);
        d.pages = pages.into_iter().filter(|(_, v)| !v.is_empty()).collect();
        d
    }

    pub fn page(&self, page: u32) -> &[Markup] {
        self.pages.get(&page).map_or(&[], Vec::as_slice)
    }

    /// Pages with markup, in order.
    pub fn pages(&self) -> impl Iterator<Item = (u32, &[Markup])> {
        self.pages.iter().map(|(p, v)| (*p, v.as_slice()))
    }

    pub fn is_empty(&self) -> bool {
        self.pages.values().all(Vec::is_empty)
    }

    pub fn len(&self) -> usize {
        self.pages.values().map(Vec::len).sum()
    }

    /// Edits page `page` with `f`; one undo step when anything changed. True when it did.
    pub fn edit(&mut self, page: u32, f: impl FnOnce(&mut Vec<Markup>)) -> bool {
        let list = self.pages.entry(page).or_default();
        let before = list.clone();
        f(list);
        if *list == before {
            return false;
        }
        if list.is_empty() {
            self.pages.remove(&page);
        }
        self.undo.push(Step { page, before });
        if self.undo.len() > UNDO_DEPTH {
            self.undo.remove(0);
        }
        self.redo.clear();
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Undoes the last edit; the page it was on.
    pub fn undo(&mut self) -> Option<u32> {
        let s = self.undo.pop()?;
        let now = self.swap(s.page, s.before);
        self.redo.push(Step {
            page: s.page,
            before: now,
        });
        Some(s.page)
    }

    /// Redoes the last undone edit; the page it was on.
    pub fn redo(&mut self) -> Option<u32> {
        let s = self.redo.pop()?;
        let now = self.swap(s.page, s.before);
        self.undo.push(Step {
            page: s.page,
            before: now,
        });
        Some(s.page)
    }

    fn swap(&mut self, page: u32, list: Vec<Markup>) -> Vec<Markup> {
        let old = if list.is_empty() {
            self.pages.remove(&page)
        } else {
            self.pages.insert(page, list)
        };
        old.unwrap_or_default()
    }
}

/// Where the markup of `file` is kept: `plan.pdf.markup.xfdf` for a PDF, else
/// `photo.jpg.markup.json`.
pub fn sidecar_path(file: &Path, space: Space) -> PathBuf {
    let mut name = file.file_name().unwrap_or_default().to_os_string();
    name.push(match space {
        Space::PdfPage => ".markup.xfdf",
        Space::Pixels => ".markup.json",
    });
    file.with_file_name(name)
}

/// Writes `bytes` to `path` through a temporary file in the same folder, so a failed write
/// never leaves half a sidecar.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(format!(".tmp{}", std::process::id()));
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// A new unique id.
pub fn new_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut x = t ^ n.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ u64::from(std::process::id()) << 40;
    // splitmix64
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    format!("spec-{:016x}", x ^ (x >> 31))
}

/// Now, UTC, `2026-10-24T14:03:00Z`.
pub fn now_utc() -> String {
    let s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    iso_of(s as i64)
}

/// Now, local time with its UTC offset, `2026-10-24T16:03:00+02:00`: a stamp shows the day
/// it was placed where it was placed (a stamp placed at 01:00 in Sydney is not yesterday's).
pub fn now_local() -> String {
    chrono::Local::now()
        .format("%Y-%m-%dT%H:%M:%S%:z")
        .to_string()
}

/// The UTC offset of an ISO date-time (`Z`, `+02:00`, `-05:30`) as (sign, hours, minutes);
/// `None` without one or for `Z`.
fn iso_offset(iso: &str) -> Option<(char, &str, &str)> {
    let rest = iso.get(19..)?;
    let sign = rest.chars().next().filter(|c| matches!(c, '+' | '-'))?;
    let (h, m) = (rest.get(1..3)?, rest.get(4..6)?);
    (h.bytes().chain(m.bytes()).all(|b| b.is_ascii_digit())).then_some((sign, h, m))
}

/// `secs` since 1970 as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn iso_of(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

/// Year, month, day of a day count since 1970-01-01 (Howard Hinnant's algorithm).
fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// The day of an ISO date-time (`2026-10-24`), or empty.
pub fn day_of(iso: &str) -> &str {
    iso.get(..10)
        .filter(|d| d.as_bytes().get(4) == Some(&b'-'))
        .unwrap_or("")
}

/// An ISO date-time as a PDF date (`D:20261024140300Z`, `D:20261024160300+02'00'`).
pub fn pdf_date(iso: &str) -> String {
    let digits: String = iso
        .get(..19)
        .unwrap_or(iso)
        .chars()
        .filter(char::is_ascii_digit)
        .take(14)
        .collect();
    match iso_offset(iso) {
        Some((s, h, m)) => format!("D:{digits}{s}{h}'{m}'"),
        None => format!("D:{digits}Z"),
    }
}

/// A PDF date (`D:20261024140300Z`, `D:20261024160300+02'00'`, or shorter) as ISO, with its
/// UTC offset when it has one.
pub fn iso_of_pdf_date(d: &str) -> String {
    let body = d.trim_start_matches("D:");
    let digits: String = body.chars().take_while(char::is_ascii_digit).collect();
    let part = |a: usize, b: usize, def: &str| digits.get(a..b).unwrap_or(def).to_owned();
    if digits.len() < 4 {
        return String::new();
    }
    let rest = &body[digits.len()..];
    let zone = match rest.chars().next() {
        Some(s @ ('+' | '-')) => {
            let n: String = rest.chars().filter(char::is_ascii_digit).take(4).collect();
            match (n.get(0..2), n.get(2..4)) {
                (Some(h), Some(m)) => format!("{s}{h}:{m}"),
                (Some(h), None) => format!("{s}{h}:00"),
                _ => "Z".to_owned(),
            }
        }
        _ => "Z".to_owned(),
    };
    format!(
        "{}-{}-{}T{}:{}:{}{zone}",
        part(0, 4, "1970"),
        part(4, 6, "01"),
        part(6, 8, "01"),
        part(8, 10, "00"),
        part(10, 12, "00"),
        part(12, 14, "00")
    )
}

/// `#rrggbb`.
pub fn hex_colour(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

/// `#rrggbb` (or `#rgb`) back.
pub fn parse_colour(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().strip_prefix('#')?;
    let v = |i: usize, n: usize| u8::from_str_radix(h.get(i..i + n)?, 16).ok();
    match h.len() {
        6 => Some([v(0, 2)?, v(2, 2)?, v(4, 2)?]),
        3 => Some([v(0, 1)? * 17, v(1, 1)? * 17, v(2, 1)? * 17]),
        _ => None,
    }
}

/// An affine map `[a, b, c, d, e, f]`: x' = a x + c y + e, y' = b x + d y + f (PDF order).
pub type Affine = [f64; 6];

pub fn apply(m: &Affine, p: P) -> P {
    P::new(
        m[0] * p.x + m[2] * p.y + m[4],
        m[1] * p.x + m[3] * p.y + m[5],
    )
}

pub fn invert(m: &Affine) -> Option<Affine> {
    let det = m[0] * m[3] - m[1] * m[2];
    if !det.is_finite() || det.abs() < 1e-12 {
        return None;
    }
    let (a, b, c, d) = (m[3] / det, -m[1] / det, -m[2] / det, m[0] / det);
    Some([a, b, c, d, -(a * m[4] + c * m[5]), -(b * m[4] + d * m[5])])
}

#[cfg(test)]
mod tests;
