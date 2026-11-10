//! The hidden text layer (`TXTa`, `TXTz`), annotations (`ANTa`, `ANTz`) and the document
//! outline (`NAVM`).

use crate::Error;

/// Most zones per page.
const MAX_ZONES: usize = 1 << 20;
/// Deepest zone nesting (page, column, region, paragraph, line, word, character).
const MAX_DEPTH: usize = 16;

/// What a text zone is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneKind {
    Page,
    Column,
    Region,
    Paragraph,
    Line,
    Word,
    Character,
}

impl ZoneKind {
    fn from(b: u8) -> Option<ZoneKind> {
        Some(match b {
            1 => ZoneKind::Page,
            2 => ZoneKind::Column,
            3 => ZoneKind::Region,
            4 => ZoneKind::Paragraph,
            5 => ZoneKind::Line,
            6 => ZoneKind::Word,
            7 => ZoneKind::Character,
            _ => return None,
        })
    }
}

/// One zone: its rectangle in page pixels (top-down rows, unrotated page), the byte range of
/// its text in [`TextLayer::text`] and its depth in the tree (0 for the page).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zone {
    pub kind: ZoneKind,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub text: std::ops::Range<usize>,
    pub depth: u8,
}

/// The text of a page and its zones in reading order (parents before children).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextLayer {
    pub text: String,
    pub zones: Vec<Zone>,
}

impl TextLayer {
    /// The words (or the finest zones holding text) with their rectangles.
    pub fn words(&self) -> impl Iterator<Item = (&str, &Zone)> {
        self.zones
            .iter()
            .filter(|z| z.kind == ZoneKind::Word)
            .filter_map(|z| self.text.get(z.text.clone()).map(|t| (t.trim(), z)))
    }

    /// The text with DjVu's separators (vertical tab, group/record/unit separators) turned
    /// into line breaks and spaces, as it reads.
    pub fn plain(&self) -> String {
        let mut s = String::with_capacity(self.text.len());
        for c in self.text.chars() {
            match c {
                '\u{0b}' | '\u{1d}' | '\u{1e}' => {
                    if !s.ends_with('\n') {
                        s.push('\n');
                    }
                }
                '\u{1f}' => {
                    if !s.ends_with('\n') {
                        s.push('\n');
                    }
                }
                c => s.push(c),
            }
        }
        s.trim_end().to_string()
    }
}

struct Reader<'a> {
    d: &'a [u8],
    p: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], Error> {
        let s = self
            .d
            .get(self.p..self.p + n)
            .ok_or(Error::Malformed("truncated text layer"))?;
        self.p += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<i32, Error> {
        let b = self.take(2)?;
        Ok(i32::from(u16::from_be_bytes([b[0], b[1]])))
    }
    fn u24(&mut self) -> Result<usize, Error> {
        let b = self.take(3)?;
        Ok((usize::from(b[0]) << 16) | (usize::from(b[1]) << 8) | usize::from(b[2]))
    }
}

/// A zone in the reference decoder's bottom-up coordinates while the tree is read.
#[derive(Clone, Copy)]
struct Raw {
    kind: ZoneKind,
    xmin: i32,
    ymin: i32,
    xmax: i32,
    ymax: i32,
    start: i64,
    len: i64,
}

/// Decodes a page's text layer (the uncompressed `TXTa` bytes or the BZZ-decoded `TXTz`).
/// `height` is the page height, to turn rows top-down.
pub fn decode_text(d: &[u8], height: u32) -> Result<TextLayer, Error> {
    let mut r = Reader { d, p: 0 };
    let n = r.u24()?;
    let bytes = r.take(n)?;
    let text = String::from_utf8_lossy(bytes).into_owned();
    let mut layer = TextLayer {
        text,
        zones: Vec::new(),
    };
    if r.p >= d.len() {
        return Ok(layer);
    }
    let _version = r.u8()?;
    if r.p >= d.len() {
        return Ok(layer);
    }
    let mut raws = Vec::new();
    zone(&mut r, None, None, 0, &mut raws)?;
    let h = height as i32;
    let tlen = layer.text.len();
    for (z, depth) in raws {
        let s = (z.start.max(0) as usize).min(tlen);
        let e = ((z.start + z.len).max(0) as usize).clamp(s, tlen);
        // Keep ranges on character boundaries.
        let (mut s, mut e) = (s, e);
        while s > 0 && !layer.text.is_char_boundary(s) {
            s -= 1;
        }
        while e < tlen && !layer.text.is_char_boundary(e) {
            e += 1;
        }
        layer.zones.push(Zone {
            kind: z.kind,
            x: z.xmin,
            y: h - z.ymax,
            width: z.xmax - z.xmin,
            height: z.ymax - z.ymin,
            text: s..e,
            depth,
        });
    }
    Ok(layer)
}

fn zone(
    r: &mut Reader<'_>,
    parent: Option<Raw>,
    prev: Option<Raw>,
    depth: usize,
    out: &mut Vec<(Raw, u8)>,
) -> Result<Raw, Error> {
    if depth > MAX_DEPTH || out.len() >= MAX_ZONES {
        return Err(Error::Malformed("text layer too deep or too large"));
    }
    let kind = ZoneKind::from(r.u8()?).ok_or(Error::Malformed("text zone type"))?;
    let mut x = r.u16()? - 0x8000;
    let mut y = r.u16()? - 0x8000;
    let w = r.u16()? - 0x8000;
    let h = r.u16()? - 0x8000;
    let mut start = i64::from(r.u16()? - 0x8000);
    let len = r.u24()? as i64;
    if let Some(p) = prev {
        if matches!(kind, ZoneKind::Page | ZoneKind::Paragraph | ZoneKind::Line) {
            x += p.xmin;
            y = p.ymin - (y + h);
        } else {
            x += p.xmax;
            y += p.ymin;
        }
        start += p.start + p.len;
    } else if let Some(p) = parent {
        x += p.xmin;
        y = p.ymax - (y + h);
        start += p.start;
    }
    let me = Raw {
        kind,
        xmin: x,
        ymin: y,
        xmax: x.saturating_add(w),
        ymax: y.saturating_add(h),
        start,
        len,
    };
    let children = r.u24()?;
    out.push((me, depth as u8));
    let mut last = None;
    for _ in 0..children {
        last = Some(zone(r, Some(me), last, depth + 1, out)?);
    }
    Ok(me)
}

/// A bookmark of the outline: its title, target (`#page-id`, `#12` or a URL) and children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bookmark {
    pub title: String,
    pub url: String,
    pub children: Vec<Bookmark>,
}

/// Decodes the BZZ-decoded `NAVM` outline.
pub fn decode_outline(d: &[u8]) -> Result<Vec<Bookmark>, Error> {
    let mut r = Reader { d, p: 0 };
    let total = r.u16()? as usize;
    let mut left = total;
    let mut out = Vec::new();
    while left > 0 && r.p < d.len() {
        out.push(bookmark(&mut r, &mut left, 0)?);
    }
    Ok(out)
}

fn bookmark(r: &mut Reader<'_>, left: &mut usize, depth: usize) -> Result<Bookmark, Error> {
    if depth > 64 || *left == 0 {
        return Err(Error::Malformed("outline"));
    }
    *left -= 1;
    let n = usize::from(r.u8()?);
    let tl = r.u24()?;
    let title = String::from_utf8_lossy(r.take(tl)?).into_owned();
    let ul = r.u24()?;
    let url = String::from_utf8_lossy(r.take(ul)?).into_owned();
    let mut children = Vec::new();
    for _ in 0..n {
        children.push(bookmark(r, left, depth + 1)?);
    }
    Ok(Bookmark {
        title,
        url,
        children,
    })
}

/// What a page's annotations say that a viewer uses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Annotations {
    /// `(background #RRGGBB)`.
    pub background: Option<[u8; 3]>,
    /// `(metadata (key "value") …)` pairs, e.g. Title, Author, Producer, CreationDate.
    pub metadata: Vec<(String, String)>,
    /// Number of hyperlink areas (`maparea`).
    pub links: usize,
}

/// An s-expression of the annotation syntax.
#[derive(Debug, Clone, PartialEq)]
enum Sx {
    Atom(String),
    Str(String),
    List(Vec<Sx>),
}

fn parse_sx(s: &[u8]) -> Vec<Sx> {
    let mut p = 0;
    let mut out = Vec::new();
    let mut guard = 0;
    while p < s.len() && guard < 1 << 20 {
        guard += 1;
        match parse_one(s, &mut p, 0) {
            Some(x) => out.push(x),
            None => p += 1,
        }
    }
    out
}

fn parse_one(s: &[u8], p: &mut usize, depth: usize) -> Option<Sx> {
    while *p < s.len() && s[*p].is_ascii_whitespace() {
        *p += 1;
    }
    let c = *s.get(*p)?;
    match c {
        b'(' => {
            if depth > 64 {
                return None;
            }
            *p += 1;
            let mut items = Vec::new();
            loop {
                while *p < s.len() && s[*p].is_ascii_whitespace() {
                    *p += 1;
                }
                match s.get(*p) {
                    None => return Some(Sx::List(items)),
                    Some(b')') => {
                        *p += 1;
                        return Some(Sx::List(items));
                    }
                    _ => match parse_one(s, p, depth + 1) {
                        Some(x) => items.push(x),
                        None => *p += 1,
                    },
                }
            }
        }
        b')' => None,
        b'"' => {
            *p += 1;
            let mut v = Vec::new();
            while let Some(&c) = s.get(*p) {
                *p += 1;
                match c {
                    b'"' => break,
                    b'\\' => {
                        let Some(&e) = s.get(*p) else { break };
                        *p += 1;
                        match e {
                            b'n' => v.push(b'\n'),
                            b't' => v.push(b'\t'),
                            b'0'..=b'7' => {
                                let mut n = u32::from(e - b'0');
                                for _ in 0..2 {
                                    match s.get(*p) {
                                        Some(&d @ b'0'..=b'7') => {
                                            n = n * 8 + u32::from(d - b'0');
                                            *p += 1;
                                        }
                                        _ => break,
                                    }
                                }
                                v.push(n as u8);
                            }
                            e => v.push(e),
                        }
                    }
                    c => v.push(c),
                }
            }
            Some(Sx::Str(String::from_utf8_lossy(&v).into_owned()))
        }
        _ => {
            let start = *p;
            while *p < s.len() && !s[*p].is_ascii_whitespace() && s[*p] != b'(' && s[*p] != b')' {
                *p += 1;
            }
            Some(Sx::Atom(
                String::from_utf8_lossy(&s[start..*p]).into_owned(),
            ))
        }
    }
}

fn hex_colour(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// Reads the annotations (all of a page's chunks concatenated, decompressed).
pub fn decode_annotations(d: &[u8], into: &mut Annotations) {
    for sx in parse_sx(d) {
        let Sx::List(items) = sx else { continue };
        let Some(Sx::Atom(head)) = items.first() else {
            continue;
        };
        match head.as_str() {
            "background" => {
                if let Some(Sx::Atom(c)) = items.get(1) {
                    into.background = hex_colour(c);
                }
            }
            "metadata" => {
                for it in &items[1..] {
                    if let Sx::List(kv) = it
                        && let (Some(Sx::Atom(k)), Some(v)) = (kv.first(), kv.get(1))
                    {
                        let v = match v {
                            Sx::Str(s) | Sx::Atom(s) => s.clone(),
                            Sx::List(_) => continue,
                        };
                        if into.metadata.len() < 256 {
                            into.metadata.push((k.clone(), v));
                        }
                    }
                }
            }
            "maparea" => into.links += 1,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annotations_background_metadata_and_links() {
        let mut a = Annotations::default();
        decode_annotations(
            b"(background #ff8000) (metadata (Title \"A \\\"b\\\"\") (Producer \"x\\101\")) (maparea \"#2\" \"\" (rect 1 2 3 4))",
            &mut a,
        );
        assert_eq!(a.background, Some([255, 128, 0]));
        assert_eq!(a.metadata[0], ("Title".into(), "A \"b\"".into()));
        assert_eq!(a.metadata[1], ("Producer".into(), "xA".into()));
        assert_eq!(a.links, 1);
    }

    #[test]
    fn hostile_annotations_do_not_hang() {
        let mut a = Annotations::default();
        decode_annotations(&[b'('; 10_000], &mut a);
        decode_annotations(b")))\"unterminated", &mut a);
    }

    #[test]
    fn text_zones_relative_coordinates() {
        // Page 100×50 holding one line with two words.
        let text = b"ab cd";
        let mut d = Vec::new();
        d.extend_from_slice(&[0, 0, text.len() as u8]);
        d.extend_from_slice(text);
        d.push(1);
        let zone =
            |d: &mut Vec<u8>, t: u8, x: i32, y: i32, w: i32, h: i32, s: i32, len: u32, n: u32| {
                d.push(t);
                for v in [x, y, w, h, s] {
                    d.extend_from_slice(&((v + 0x8000) as u16).to_be_bytes());
                }
                d.extend_from_slice(&len.to_be_bytes()[1..]);
                d.extend_from_slice(&n.to_be_bytes()[1..]);
            };
        zone(&mut d, 1, 0, 0, 100, 50, 0, 5, 1);
        // Line: from the page's top-left, 10 right, 5 down, 80×20.
        zone(&mut d, 5, 10, 5, 80, 20, 0, 5, 2);
        // First word from the line's top-left; second from the first's lower right.
        zone(&mut d, 6, 0, 0, 30, 20, 0, 3, 0);
        zone(&mut d, 6, 10, 0, 40, 20, 0, 2, 0);
        let t = decode_text(&d, 50).unwrap();
        assert_eq!(t.zones.len(), 4);
        let w: Vec<_> = t
            .words()
            .map(|(s, z)| (s, z.x, z.y, z.width, z.height))
            .collect();
        assert_eq!(w, vec![("ab", 10, 5, 30, 20), ("cd", 50, 5, 40, 20)]);
    }
}
