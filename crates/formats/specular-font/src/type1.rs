//! PostScript Type 1 fonts (`.pfb` binary segments, `.pfa` hex): read and rebuilt as an
//! OpenType CFF font, so the faces, previews and the text stack treat them like any other.
//!
//! The clear-text part gives the names, matrix, bounding box and encoding; the eexec-encrypted
//! part the `Private` dictionary, `Subrs` and `CharStrings`. Each Type 1 charstring is run
//! (subroutines, `seac` accents, flex and hint-replacement other-subroutines, `div`) to an
//! outline, which is written back as a hint-free Type 2 charstring. Hints are dropped. The
//! `cmap` maps the glyph names that name Unicode characters (the Adobe Latin names, `uniXXXX`
//! and `uXXXX`). Written from Adobe's Type 1 Font Format and CFF specifications.
//!
//! Every count and nesting depth is bounded: damaged fonts fail with a reason, never panic.

use crate::names_std::CFF_STANDARD;
use crate::sfnt;
use crate::{Error, PathEl};

/// Most glyphs read.
const MAX_GLYPHS: usize = 65_000;
/// Most subroutines read.
const MAX_SUBRS: usize = 65_536;
/// Most operators run for one glyph (subroutines included).
const MAX_OPS: usize = 100_000;
/// Deepest subroutine nesting.
const MAX_DEPTH: usize = 16;

/// Whether `bytes` starts a Type 1 font: a PFB segment header, or PFA's comment.
pub(crate) fn sniff(bytes: &[u8]) -> bool {
    (bytes.len() > 6 && bytes[0] == 0x80 && bytes[1] == 1)
        || bytes.starts_with(b"%!PS-AdobeFont-1")
        || bytes.starts_with(b"%!FontType1")
}

fn damaged() -> Error {
    Error::Damaged("Type 1")
}

/// The clear-text and the decrypted private parts.
fn segments(bytes: &[u8]) -> Result<(Vec<u8>, Vec<u8>), Error> {
    let (clear, encrypted) = if bytes.first() == Some(&0x80) {
        let mut clear = Vec::new();
        let mut enc = Vec::new();
        let mut at = 0usize;
        while let Some(&[0x80, kind]) = bytes.get(at..at + 2) {
            if kind == 3 {
                break;
            }
            let len = bytes
                .get(at + 2..at + 6)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
                .ok_or(Error::Truncated)?;
            let body = bytes.get(at + 6..at + 6 + len).ok_or(Error::Truncated)?;
            match kind {
                1 if enc.is_empty() => clear.extend_from_slice(body),
                1 => {}
                2 => enc.extend_from_slice(body),
                _ => return Err(damaged()),
            }
            at += 6 + len;
        }
        (clear, enc)
    } else {
        let pos = find(bytes, b"eexec").ok_or(Error::MissingTable("eexec"))?;
        let clear = bytes[..pos].to_vec();
        let mut rest = &bytes[pos + 5..];
        while let Some((&c, r)) = rest.split_first() {
            if !c.is_ascii_whitespace() {
                break;
            }
            rest = r;
        }
        let hex = rest.len() >= 4 && rest[..4].iter().all(u8::is_ascii_hexdigit);
        let enc = if hex {
            let mut out = Vec::with_capacity(rest.len() / 2);
            let mut hi: Option<u8> = None;
            for &c in rest {
                let v = match c {
                    b'0'..=b'9' => c - b'0',
                    b'a'..=b'f' => c - b'a' + 10,
                    b'A'..=b'F' => c - b'A' + 10,
                    c if c.is_ascii_whitespace() => continue,
                    _ => break,
                };
                match hi.take() {
                    Some(h) => out.push(h << 4 | v),
                    None => hi = Some(v),
                }
            }
            out
        } else {
            rest.to_vec()
        };
        (clear, enc)
    };
    if encrypted.len() < 4 {
        return Err(Error::Truncated);
    }
    Ok((clear, decrypt(&encrypted, 55665, 4)))
}

fn decrypt(data: &[u8], key: u16, skip: usize) -> Vec<u8> {
    let mut r = key;
    let mut out = Vec::with_capacity(data.len());
    for &c in data {
        out.push(c ^ (r >> 8) as u8);
        r = (c as u16)
            .wrapping_add(r)
            .wrapping_mul(52845)
            .wrapping_add(22719);
    }
    out.split_off(skip.min(out.len()))
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// A PostScript token scanner over mixed text and binary.
struct Scan<'a> {
    d: &'a [u8],
    at: usize,
}

impl<'a> Scan<'a> {
    fn skip_space(&mut self) {
        while let Some(&c) = self.d.get(self.at) {
            if c == b'%' {
                while self
                    .d
                    .get(self.at)
                    .is_some_and(|&c| c != b'\n' && c != b'\r')
                {
                    self.at += 1;
                }
            } else if c.is_ascii_whitespace() {
                self.at += 1;
            } else {
                break;
            }
        }
    }

    /// The next token: a name with its slash, a delimiter, or a run of regular characters.
    fn token(&mut self) -> Option<&'a [u8]> {
        self.skip_space();
        let start = self.at;
        let c = *self.d.get(self.at)?;
        let delim = |c: u8| b"()<>[]{}/%".contains(&c) || c.is_ascii_whitespace();
        self.at += 1;
        if c == b'/' || !delim(c) {
            while self.d.get(self.at).is_some_and(|&c| !delim(c)) {
                self.at += 1;
            }
        }
        Some(&self.d[start..self.at])
    }

    fn number(&mut self) -> Option<f64> {
        std::str::from_utf8(self.token()?).ok()?.parse().ok()
    }

    /// A `(string)` with balanced parentheses and backslash escapes.
    fn string(&mut self) -> Option<String> {
        self.skip_space();
        if self.d.get(self.at) != Some(&b'(') {
            return None;
        }
        self.at += 1;
        let mut depth = 1;
        let mut out = Vec::new();
        while let Some(&c) = self.d.get(self.at) {
            self.at += 1;
            match c {
                b'\\' => {
                    if let Some(&n) = self.d.get(self.at) {
                        self.at += 1;
                        out.push(n);
                    }
                }
                b'(' => {
                    depth += 1;
                    out.push(c);
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    out.push(c);
                }
                _ => out.push(c),
            }
        }
        Some(String::from_utf8_lossy(&out).into_owned())
    }

    /// Up to `n` numbers inside `[…]` or `{…}`.
    fn array(&mut self, n: usize) -> Vec<f64> {
        let mut v = Vec::new();
        if !matches!(self.token(), Some(b"[" | b"{")) {
            return v;
        }
        while let Some(t) = self.token() {
            if t == b"]" || t == b"}" {
                break;
            }
            if let Some(x) = std::str::from_utf8(t).ok().and_then(|s| s.parse().ok())
                && v.len() < n
            {
                v.push(x);
            }
        }
        v
    }

    /// `n` binary bytes after an `RD`/`-|` token and its one space.
    fn binary(&mut self, n: usize) -> Option<&'a [u8]> {
        let b = self.d.get(self.at + 1..self.at + 1 + n)?;
        self.at += 1 + n;
        Some(b)
    }
}

/// What the clear text says.
#[derive(Default)]
struct Top {
    font_name: String,
    family: Option<String>,
    full: Option<String>,
    weight: Option<String>,
    notice: Option<String>,
    italic_angle: f64,
    fixed_pitch: bool,
    underline_position: f64,
    underline_thickness: f64,
    matrix: Option<Vec<f64>>,
    bbox: [f64; 4],
    /// Code → glyph name.
    encoding: Vec<(u8, String)>,
}

fn read_top(clear: &[u8]) -> Top {
    let mut t = Top {
        underline_position: -100.0,
        underline_thickness: 50.0,
        ..Default::default()
    };
    let mut s = Scan { d: clear, at: 0 };
    let mut standard = false;
    while let Some(tok) = s.token() {
        match tok {
            b"/FontName" => {
                if let Some(n) = s.token() {
                    t.font_name = String::from_utf8_lossy(n.strip_prefix(b"/").unwrap_or(n)).into();
                }
            }
            b"/FamilyName" => t.family = s.string(),
            b"/FullName" => t.full = s.string(),
            b"/Weight" => t.weight = s.string(),
            b"/Notice" => t.notice = s.string(),
            b"/ItalicAngle" => t.italic_angle = s.number().unwrap_or(0.0),
            b"/isFixedPitch" => t.fixed_pitch = s.token() == Some(b"true"),
            b"/UnderlinePosition" => t.underline_position = s.number().unwrap_or(-100.0),
            b"/UnderlineThickness" => t.underline_thickness = s.number().unwrap_or(50.0),
            b"/FontMatrix" => {
                let m = s.array(6);
                if m.len() == 6 {
                    t.matrix = Some(m);
                }
            }
            b"/FontBBox" => {
                let b = s.array(4);
                if b.len() == 4 {
                    t.bbox = [b[0], b[1], b[2], b[3]];
                }
            }
            b"/Encoding" => {
                if s.token() == Some(b"StandardEncoding") {
                    standard = true;
                    continue;
                }
                // `256 array 0 1 255 {…} for dup 32 /space put … readonly def`
                while let Some(tok) = s.token() {
                    if tok == b"def" || tok == b"readonly" {
                        break;
                    }
                    if tok == b"dup" {
                        let code = s.number();
                        let name = s.token();
                        if let (Some(c), Some(n)) = (code, name)
                            && let Some(n) = n.strip_prefix(b"/")
                            && (0.0..=255.0).contains(&c)
                        {
                            t.encoding
                                .push((c as u8, String::from_utf8_lossy(n).into()));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if standard {
        t.encoding = (0..=255u8)
            .filter_map(|c| standard_name(c).map(|n| (c, n.to_owned())))
            .collect();
    }
    t
}

/// The private part: subroutines, charstrings (by name, in file order) and `lenIV`.
struct Private {
    len_iv: i32,
    subrs: Vec<Vec<u8>>,
    glyphs: Vec<(String, Vec<u8>)>,
}

fn read_private(p: &[u8]) -> Result<Private, Error> {
    let mut s = Scan { d: p, at: 0 };
    let mut out = Private {
        len_iv: 4,
        subrs: Vec::new(),
        glyphs: Vec::new(),
    };
    let rd = |t: &[u8]| t == b"RD" || t == b"-|";
    while let Some(tok) = s.token() {
        match tok {
            b"/lenIV" => out.len_iv = s.number().unwrap_or(4.0) as i32,
            b"/Subrs" => {
                let n = (s.number().unwrap_or(0.0).max(0.0) as usize).min(MAX_SUBRS);
                out.subrs = vec![Vec::new(); n];
                let mut seen = 0;
                while seen < n {
                    let Some(t) = s.token() else { break };
                    if t != b"dup" {
                        if t == b"/CharStrings" || t == b"ND" || t == b"|-" || t == b"def" {
                            if t == b"/CharStrings" {
                                s.at -= t.len();
                            }
                            if seen > 0 || t == b"/CharStrings" {
                                break;
                            }
                        }
                        continue;
                    }
                    let i = s.number().ok_or_else(damaged)? as usize;
                    let len = s.number().ok_or_else(damaged)? as usize;
                    if !s.token().is_some_and(rd) {
                        return Err(damaged());
                    }
                    let b = s.binary(len).ok_or(Error::Truncated)?;
                    if let Some(slot) = out.subrs.get_mut(i) {
                        *slot = b.to_vec();
                    }
                    seen += 1;
                }
            }
            b"/CharStrings" => {
                let n = s.number().unwrap_or(0.0).max(0.0) as usize;
                while out.glyphs.len() < n.min(MAX_GLYPHS) {
                    let Some(t) = s.token() else { break };
                    if t == b"end" {
                        break;
                    }
                    let Some(name) = t.strip_prefix(b"/") else {
                        continue;
                    };
                    let len = s.number().ok_or_else(damaged)? as usize;
                    if !s.token().is_some_and(rd) {
                        return Err(damaged());
                    }
                    let b = s.binary(len).ok_or(Error::Truncated)?;
                    out.glyphs
                        .push((String::from_utf8_lossy(name).into_owned(), b.to_vec()));
                }
                break;
            }
            _ => {}
        }
    }
    if out.glyphs.is_empty() {
        return Err(Error::NoGlyphs);
    }
    Ok(out)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Seg {
    Move(f64, f64),
    Line(f64, f64),
    Curve(f64, f64, f64, f64, f64, f64),
}

/// One glyph's outline in font units and its advance.
#[derive(Default)]
struct Glyph {
    segs: Vec<Seg>,
    width: f64,
}

struct Run<'a> {
    p: &'a Private,
    by_name: &'a std::collections::HashMap<&'a str, usize>,
    ops: usize,
}

impl Run<'_> {
    fn decrypt(&self, cs: &[u8]) -> Vec<u8> {
        if self.p.len_iv < 0 {
            cs.to_vec()
        } else {
            decrypt(cs, 4330, self.p.len_iv as usize)
        }
    }

    /// Runs glyph `index`, offset by (`dx`, `dy`) (`seac` accents); `depth` bounds `seac`.
    fn glyph(&mut self, index: usize, dx: f64, dy: f64, depth: usize) -> Result<Glyph, Error> {
        let cs = self.decrypt(&self.p.glyphs[index].1);
        let mut st = State {
            g: Glyph::default(),
            x: 0.0,
            y: 0.0,
            dx,
            dy,
            stack: Vec::new(),
            ps: Vec::new(),
            flex: None,
            open: false,
            done: false,
            seac: None,
        };
        self.exec(&cs, &mut st, 0)?;
        if let Some((asb, adx, ady, b, a)) = st.seac {
            if depth > 0 {
                return Err(damaged());
            }
            let find =
                |code: f64| standard_name(code as u8).and_then(|n| self.by_name.get(n).copied());
            let (Some(base), Some(accent)) = (find(b), find(a)) else {
                return Ok(st.g);
            };
            let width = st.g.width;
            let mut g = self.glyph(base, dx, dy, depth + 1)?;
            let acc = self.glyph(accent, dx + adx - asb, dy + ady, depth + 1)?;
            g.segs.extend(acc.segs);
            g.width = width;
            return Ok(g);
        }
        Ok(st.g)
    }

    fn exec(&mut self, cs: &[u8], st: &mut State, depth: usize) -> Result<(), Error> {
        if depth > MAX_DEPTH {
            return Err(damaged());
        }
        let mut i = 0;
        while i < cs.len() && !st.done {
            self.ops += 1;
            if self.ops > MAX_OPS || st.stack.len() > 64 {
                return Err(damaged());
            }
            let v = cs[i];
            i += 1;
            if v >= 32 {
                let n = match v {
                    32..=246 => v as i32 - 139,
                    247..=250 => {
                        let w = *cs.get(i).ok_or_else(damaged)? as i32;
                        i += 1;
                        (v as i32 - 247) * 256 + w + 108
                    }
                    251..=254 => {
                        let w = *cs.get(i).ok_or_else(damaged)? as i32;
                        i += 1;
                        -(v as i32 - 251) * 256 - w - 108
                    }
                    _ => {
                        let b = cs.get(i..i + 4).ok_or_else(damaged)?;
                        i += 4;
                        i32::from_be_bytes([b[0], b[1], b[2], b[3]])
                    }
                };
                st.stack.push(n as f64);
                continue;
            }
            let op = if v == 12 {
                let e = *cs.get(i).ok_or_else(damaged)?;
                i += 1;
                1200 + e as u32
            } else {
                v as u32
            };
            let a = std::mem::take(&mut st.stack);
            let arg = |k: usize| a.get(k).copied().unwrap_or(0.0);
            match op {
                13 => {
                    // hsbw: side bearing and width.
                    st.x = arg(0);
                    st.y = 0.0;
                    st.g.width = arg(1);
                }
                1207 => {
                    st.x = arg(0);
                    st.y = arg(1);
                    st.g.width = arg(2);
                }
                21 => st.rmove(arg(0), arg(1)),
                22 => st.rmove(arg(0), 0.0),
                4 => st.rmove(0.0, arg(0)),
                5 => st.line(arg(0), arg(1)),
                6 => st.line(arg(0), 0.0),
                7 => st.line(0.0, arg(0)),
                8 => st.curve(arg(0), arg(1), arg(2), arg(3), arg(4), arg(5)),
                30 => st.curve(0.0, arg(0), arg(1), arg(2), arg(3), 0.0),
                31 => st.curve(arg(0), 0.0, arg(1), arg(2), 0.0, arg(3)),
                9 => st.open = false,
                14 => st.done = true,
                10 => {
                    let mut a = a;
                    let n = a.pop().unwrap_or(-1.0);
                    let sub = usize::try_from(n as i64)
                        .ok()
                        .and_then(|n| self.p.subrs.get(n))
                        .ok_or_else(damaged)?;
                    let sub = self.decrypt(sub);
                    st.stack = a;
                    self.exec(&sub, st, depth + 1)?;
                    continue;
                }
                11 => {
                    st.stack = a;
                    return Ok(());
                }
                1206 => st.seac = Some((arg(0), arg(1), arg(2), arg(3), arg(4))),
                1212 => {
                    let mut a = a;
                    let d = a.pop().unwrap_or(1.0);
                    let n = a.pop().unwrap_or(0.0);
                    a.push(if d == 0.0 { 0.0 } else { n / d });
                    st.stack = a;
                    continue;
                }
                1216 => {
                    // callothersubr: args… n othersubr#
                    let mut a = a;
                    let which = a.pop().unwrap_or(-1.0) as i32;
                    let n = (a.pop().unwrap_or(0.0).max(0.0) as usize).min(a.len());
                    let args = a.split_off(a.len() - n);
                    match which {
                        1 => st.flex = Some(Vec::new()),
                        0 => {
                            let pts = st.flex.take().unwrap_or_default();
                            if pts.len() >= 7 {
                                let p = &pts[pts.len() - 7..];
                                let (ox, oy) = (st.dx, st.dy);
                                for c in [&p[1..4], &p[4..7]] {
                                    st.g.segs.push(Seg::Curve(
                                        c[0].0 + ox,
                                        c[0].1 + oy,
                                        c[1].0 + ox,
                                        c[1].1 + oy,
                                        c[2].0 + ox,
                                        c[2].1 + oy,
                                    ));
                                }
                                st.x = p[6].0;
                                st.y = p[6].1;
                            }
                            st.ps = vec![st.y, st.x];
                        }
                        2 => {}
                        _ => st.ps = args.into_iter().rev().collect(),
                    }
                    st.stack = a;
                    continue;
                }
                1217 => {
                    let mut a = a;
                    a.push(st.ps.pop().unwrap_or(0.0));
                    st.stack = a;
                    continue;
                }
                1233 => {
                    st.x = arg(0);
                    st.y = arg(1);
                }
                // Hints and dot sections: dropped.
                1 | 3 | 1200 | 1201 | 1202 => {}
                _ => {}
            }
        }
        Ok(())
    }
}

struct State {
    g: Glyph,
    x: f64,
    y: f64,
    dx: f64,
    dy: f64,
    stack: Vec<f64>,
    ps: Vec<f64>,
    flex: Option<Vec<(f64, f64)>>,
    open: bool,
    done: bool,
    seac: Option<(f64, f64, f64, f64, f64)>,
}

impl State {
    fn rmove(&mut self, dx: f64, dy: f64) {
        self.x += dx;
        self.y += dy;
        if let Some(f) = &mut self.flex {
            f.push((self.x, self.y));
            return;
        }
        self.g
            .segs
            .push(Seg::Move(self.x + self.dx, self.y + self.dy));
        self.open = true;
    }

    fn line(&mut self, dx: f64, dy: f64) {
        self.x += dx;
        self.y += dy;
        self.g
            .segs
            .push(Seg::Line(self.x + self.dx, self.y + self.dy));
    }

    fn curve(&mut self, a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) {
        let (x1, y1) = (self.x + a, self.y + b);
        let (x2, y2) = (x1 + c, y1 + d);
        self.x = x2 + e;
        self.y = y2 + f;
        let (ox, oy) = (self.dx, self.dy);
        self.g.segs.push(Seg::Curve(
            x1 + ox,
            y1 + oy,
            x2 + ox,
            y2 + oy,
            self.x + ox,
            self.y + oy,
        ));
    }
}

// ----- Writing CFF and the SFNT -------------------------------------------------------------

/// A Type 2 / DICT number.
fn t2_number(out: &mut Vec<u8>, v: f64) {
    if v.fract() != 0.0 {
        out.push(255);
        out.extend_from_slice(&((v * 65536.0).round() as i32).to_be_bytes());
        return;
    }
    let v = v as i32;
    match v {
        -107..=107 => out.push((v + 139) as u8),
        108..=1131 => {
            let w = v - 108;
            out.extend_from_slice(&[(w >> 8) as u8 + 247, w as u8]);
        }
        -1131..=-108 => {
            let w = -v - 108;
            out.extend_from_slice(&[(w >> 8) as u8 + 251, w as u8]);
        }
        _ => {
            out.push(28);
            out.extend_from_slice(&(v.clamp(-32768, 32767) as i16).to_be_bytes());
        }
    }
}

fn dict_int(out: &mut Vec<u8>, v: i32) {
    match v {
        -107..=107 => out.push((v + 139) as u8),
        _ => {
            out.push(29);
            out.extend_from_slice(&v.to_be_bytes());
        }
    }
}

/// A DICT real number (nibbles).
fn dict_real(out: &mut Vec<u8>, v: f64) {
    let s = format!("{v:e}");
    let mut nib = Vec::new();
    let (mant, exp) = s.split_once('e').unwrap_or((&s, "0"));
    for c in mant.chars() {
        match c {
            '0'..='9' => nib.push(c as u8 - b'0'),
            '.' => nib.push(0xA),
            '-' => nib.push(0xE),
            _ => {}
        }
    }
    let e: i32 = exp.parse().unwrap_or(0);
    if e != 0 {
        nib.push(if e < 0 { 0xC } else { 0xB });
        for c in e.abs().to_string().chars() {
            nib.push(c as u8 - b'0');
        }
    }
    nib.push(0xF);
    if !nib.len().is_multiple_of(2) {
        nib.push(0xF);
    }
    out.push(30);
    for p in nib.chunks(2) {
        out.push(p[0] << 4 | p[1]);
    }
}

fn index(items: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(items.len() as u16).to_be_bytes());
    if items.is_empty() {
        return out;
    }
    out.push(4);
    let mut off = 1u32;
    out.extend_from_slice(&off.to_be_bytes());
    for it in items {
        off += it.len() as u32;
        out.extend_from_slice(&off.to_be_bytes());
    }
    for it in items {
        out.extend_from_slice(it);
    }
    out
}

fn charstring(g: &Glyph) -> Vec<u8> {
    let mut out = Vec::new();
    t2_number(&mut out, g.width.round());
    let (mut x, mut y) = (0.0f64, 0.0f64);
    let mut first = true;
    for s in &g.segs {
        match *s {
            Seg::Move(a, b) => {
                t2_number(&mut out, a - x);
                t2_number(&mut out, b - y);
                out.push(21);
                (x, y) = (a, b);
                first = false;
            }
            Seg::Line(a, b) => {
                if first {
                    continue;
                }
                t2_number(&mut out, a - x);
                t2_number(&mut out, b - y);
                out.push(5);
                (x, y) = (a, b);
            }
            Seg::Curve(a, b, c, d, e, f) => {
                if first {
                    continue;
                }
                for (px, py) in [(a, b), (c, d), (e, f)] {
                    t2_number(&mut out, px - x);
                    t2_number(&mut out, py - y);
                    (x, y) = (px, py);
                }
                out.push(8);
            }
        }
    }
    out.push(14);
    out
}

fn bounds(g: &Glyph) -> Option<[f64; 4]> {
    let mut b: Option<[f64; 4]> = None;
    let mut add = |x: f64, y: f64| {
        let v = b.get_or_insert([x, y, x, y]);
        v[0] = v[0].min(x);
        v[1] = v[1].min(y);
        v[2] = v[2].max(x);
        v[3] = v[3].max(y);
    };
    for s in &g.segs {
        match *s {
            Seg::Move(x, y) | Seg::Line(x, y) => add(x, y),
            Seg::Curve(a, b, c, d, e, f) => {
                add(a, b);
                add(c, d);
                add(e, f);
            }
        }
    }
    b
}

fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(|u| u.to_be_bytes()).collect()
}

fn name_table(names: &[(u16, String)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut strings = Vec::new();
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&(names.len() as u16).to_be_bytes());
    out.extend_from_slice(&(6 + 12 * names.len() as u16).to_be_bytes());
    for (id, s) in names {
        let b = utf16(s);
        for v in [3u16, 1, 0x409, *id, b.len() as u16, strings.len() as u16] {
            out.extend_from_slice(&v.to_be_bytes());
        }
        strings.extend_from_slice(&b);
    }
    out.extend_from_slice(&strings);
    out
}

/// A format 4 `cmap` from sorted (code point, glyph) pairs of the BMP.
fn cmap_table(map: &[(u32, u16)]) -> Vec<u8> {
    let mut segs: Vec<(u16, u16, u16)> = Vec::new(); // start, end, first glyph
    for &(c, g) in map {
        let c = c as u16;
        match segs.last_mut() {
            Some(s) if s.1.wrapping_add(1) == c && s.2.wrapping_add(c.wrapping_sub(s.0)) == g => {
                s.1 = c;
            }
            _ => segs.push((c, c, g)),
        }
    }
    segs.push((0xFFFF, 0xFFFF, 0));
    let n = segs.len() as u16;
    let mut sub = Vec::new();
    let len = 16 + 8 * n;
    for v in [4u16, len, 0, n * 2] {
        sub.extend_from_slice(&v.to_be_bytes());
    }
    let mut pow = 1u16;
    let mut log = 0u16;
    while pow * 2 <= n {
        pow *= 2;
        log += 1;
    }
    for v in [pow * 2, log, n * 2 - pow * 2] {
        sub.extend_from_slice(&v.to_be_bytes());
    }
    for s in &segs {
        sub.extend_from_slice(&s.1.to_be_bytes());
    }
    sub.extend_from_slice(&0u16.to_be_bytes());
    for s in &segs {
        sub.extend_from_slice(&s.0.to_be_bytes());
    }
    for s in &segs {
        let delta = if s.0 == 0xFFFF {
            1
        } else {
            s.2.wrapping_sub(s.0)
        };
        sub.extend_from_slice(&delta.to_be_bytes());
    }
    for _ in &segs {
        sub.extend_from_slice(&0u16.to_be_bytes());
    }
    let mut out = Vec::new();
    for v in [0u16, 1, 3, 1] {
        out.extend_from_slice(&v.to_be_bytes());
    }
    out.extend_from_slice(&12u32.to_be_bytes());
    out.extend_from_slice(&sub);
    out
}

fn weight_class(w: Option<&str>) -> u16 {
    let w = w.unwrap_or("").to_ascii_lowercase();
    if w.contains("thin") || w.contains("hairline") {
        100
    } else if w.contains("extralight") || w.contains("ultralight") {
        200
    } else if w.contains("light") {
        300
    } else if w.contains("medium") {
        500
    } else if w.contains("semibold") || w.contains("demi") {
        600
    } else if w.contains("extrabold") || w.contains("ultrabold") || w.contains("heavy") {
        800
    } else if w.contains("black") {
        900
    } else if w.contains("bold") {
        700
    } else {
        400
    }
}

/// Converts a Type 1 font to an OpenType (CFF) SFNT.
pub(crate) fn to_sfnt(bytes: &[u8]) -> Result<Vec<u8>, Error> {
    let (clear, private) = segments(bytes)?;
    let top = read_top(&clear);
    let p = read_private(&private)?;
    // .notdef first.
    let mut order: Vec<usize> = Vec::with_capacity(p.glyphs.len() + 1);
    let notdef = p.glyphs.iter().position(|g| g.0 == ".notdef");
    let by_name: std::collections::HashMap<&str, usize> = p
        .glyphs
        .iter()
        .enumerate()
        .map(|(i, g)| (g.0.as_str(), i))
        .collect();
    if let Some(n) = notdef {
        order.push(n);
    }
    order.extend((0..p.glyphs.len()).filter(|&i| Some(i) != notdef));
    let mut run = Run {
        p: &p,
        by_name: &by_name,
        ops: 0,
    };
    let mut glyphs: Vec<(String, Glyph)> = Vec::with_capacity(order.len() + 1);
    if notdef.is_none() {
        glyphs.push((".notdef".into(), Glyph::default()));
    }
    for &i in &order {
        run.ops = 0;
        // A glyph that fails to run is kept empty (with no width): the others still show.
        let g = run.glyph(i, 0.0, 0.0, 0).unwrap_or_default();
        glyphs.push((p.glyphs[i].0.clone(), g));
    }
    let n = glyphs.len();
    // Strings and SIDs.
    let mut strings: Vec<Vec<u8>> = Vec::new();
    let mut sid = |s: &str| -> i32 {
        if let Some(i) = CFF_STANDARD.iter().position(|n| *n == s) {
            return i as i32;
        }
        strings.push(s.as_bytes().to_vec());
        390 + strings.len() as i32
    };
    let full = top.full.clone().unwrap_or_else(|| top.font_name.clone());
    let family = top.family.clone().unwrap_or_else(|| full.clone());
    let sid_full = sid(&full);
    let sid_family = sid(&family);
    let sid_weight = top.weight.as_deref().map(&mut sid);
    let charset_sids: Vec<i32> = glyphs[1..].iter().map(|g| sid(&g.0)).collect();
    let charstrings: Vec<Vec<u8>> = glyphs.iter().map(|g| charstring(&g.1)).collect();
    let name_index = index(&[top.font_name.as_bytes().to_vec()]);
    let string_index = index(&strings);
    let gsubr_index = index(&[]);
    let mut charset = vec![0u8];
    for s in &charset_sids {
        charset.extend_from_slice(&(*s as u16).to_be_bytes());
    }
    let cs_index = index(&charstrings);
    let private_dict = {
        let mut d = Vec::new();
        dict_int(&mut d, 0);
        d.push(20);
        dict_int(&mut d, 0);
        d.push(21);
        d
    };
    let top_dict = |charset_at: i32, cs_at: i32, priv_at: i32| -> Vec<u8> {
        let mut d = Vec::new();
        dict_int(&mut d, sid_full);
        d.push(2);
        dict_int(&mut d, sid_family);
        d.push(3);
        if let Some(w) = sid_weight {
            dict_int(&mut d, w);
            d.push(4);
        }
        if top.fixed_pitch {
            dict_int(&mut d, 1);
            d.extend_from_slice(&[12, 1]);
        }
        if top.italic_angle != 0.0 {
            dict_real(&mut d, top.italic_angle);
            d.extend_from_slice(&[12, 2]);
        }
        for v in top.bbox {
            dict_real(&mut d, v);
        }
        d.push(5);
        if let Some(m) = &top.matrix
            && m != &[0.001, 0.0, 0.0, 0.001, 0.0, 0.0]
        {
            for v in m {
                dict_real(&mut d, *v);
            }
            d.extend_from_slice(&[12, 7]);
        }
        for (v, op) in [(charset_at, 15u8), (cs_at, 17)] {
            d.push(29);
            d.extend_from_slice(&v.to_be_bytes());
            d.push(op);
        }
        d.push(29);
        d.extend_from_slice(&(private_dict.len() as i32).to_be_bytes());
        d.push(29);
        d.extend_from_slice(&priv_at.to_be_bytes());
        d.push(18);
        d
    };
    // The Top DICT's size does not depend on the offsets (5-byte integers).
    let top_len = index(&[top_dict(0, 0, 0)]).len();
    let charset_at = 4 + name_index.len() + top_len + string_index.len() + gsubr_index.len();
    let cs_at = charset_at + charset.len();
    let priv_at = cs_at + cs_index.len();
    let mut cff = vec![1, 0, 4, 4];
    cff.extend_from_slice(&name_index);
    cff.extend_from_slice(&index(&[top_dict(
        charset_at as i32,
        cs_at as i32,
        priv_at as i32,
    )]));
    cff.extend_from_slice(&string_index);
    cff.extend_from_slice(&gsubr_index);
    cff.extend_from_slice(&charset);
    cff.extend_from_slice(&cs_index);
    cff.extend_from_slice(&private_dict);

    // Metrics.
    let upem = top
        .matrix
        .as_ref()
        .map(|m| m[0])
        .filter(|a| *a > 0.0)
        .map_or(1000.0, |a| (1.0 / a).round())
        .clamp(16.0, 16384.0);
    let gb: Vec<Option<[f64; 4]>> = glyphs.iter().map(|g| bounds(&g.1)).collect();
    let mut bbox = top.bbox;
    if bbox == [0.0; 4] {
        bbox = gb.iter().flatten().fold([0.0, 0.0, 0.0, 0.0], |a, b| {
            [
                a[0].min(b[0]),
                a[1].min(b[1]),
                a[2].max(b[2]),
                a[3].max(b[3]),
            ]
        });
    }
    let i16v = |v: f64| (v.round().clamp(-32768.0, 32767.0) as i16).to_be_bytes();
    let u16v = |v: f64| (v.round().clamp(0.0, 65535.0) as u16).to_be_bytes();
    let bold = weight_class(top.weight.as_deref()) >= 600;
    let italic = top.italic_angle != 0.0;
    let mut head = Vec::with_capacity(54);
    head.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    head.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    head.extend_from_slice(&0u32.to_be_bytes());
    head.extend_from_slice(&0x5F0F_3CF5u32.to_be_bytes());
    head.extend_from_slice(&3u16.to_be_bytes());
    head.extend_from_slice(&(upem as u16).to_be_bytes());
    head.extend_from_slice(&[0; 16]);
    for v in bbox {
        head.extend_from_slice(&i16v(v));
    }
    head.extend_from_slice(&(bold as u16 | (italic as u16) << 1).to_be_bytes());
    head.extend_from_slice(&8u16.to_be_bytes());
    head.extend_from_slice(&2i16.to_be_bytes());
    head.extend_from_slice(&[0, 0, 0, 0]);
    let max_adv = glyphs.iter().map(|g| g.1.width).fold(0.0, f64::max);
    let mut hhea = Vec::with_capacity(36);
    hhea.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    hhea.extend_from_slice(&i16v(bbox[3]));
    hhea.extend_from_slice(&i16v(bbox[1]));
    hhea.extend_from_slice(&0i16.to_be_bytes());
    hhea.extend_from_slice(&u16v(max_adv));
    hhea.extend_from_slice(&i16v(bbox[0]));
    hhea.extend_from_slice(&i16v(0.0));
    hhea.extend_from_slice(&i16v(bbox[2]));
    hhea.extend_from_slice(&1i16.to_be_bytes());
    hhea.extend_from_slice(&[0; 14]);
    hhea.extend_from_slice(&(n as u16).to_be_bytes());
    let mut hmtx = Vec::with_capacity(n * 4);
    for (g, b) in glyphs.iter().zip(&gb) {
        hmtx.extend_from_slice(&u16v(g.1.width));
        hmtx.extend_from_slice(&i16v(b.map_or(0.0, |b| b[0])));
    }
    let mut maxp = 0x0000_5000u32.to_be_bytes().to_vec();
    maxp.extend_from_slice(&(n as u16).to_be_bytes());
    // Unicode.
    let mut map: Vec<(u32, u16)> = glyphs
        .iter()
        .enumerate()
        .filter_map(|(gid, g)| unicode_of(&g.0).map(|u| (u, gid as u16)))
        .filter(|(u, _)| *u < 0xFFFF)
        .collect();
    map.sort();
    map.dedup_by_key(|m| m.0);
    let top_of = |name: &str| {
        by_name
            .get(name)
            .and_then(|_| glyphs.iter().position(|g| g.0 == name))
            .and_then(|i| gb[i])
            .map(|b| b[3])
    };
    let mut os2 = Vec::with_capacity(96);
    let avg = {
        let w: Vec<f64> = glyphs
            .iter()
            .map(|g| g.1.width)
            .filter(|w| *w > 0.0)
            .collect();
        w.iter().sum::<f64>() / w.len().max(1) as f64
    };
    os2.extend_from_slice(&3u16.to_be_bytes());
    os2.extend_from_slice(&i16v(avg));
    os2.extend_from_slice(&weight_class(top.weight.as_deref()).to_be_bytes());
    os2.extend_from_slice(&5u16.to_be_bytes());
    os2.extend_from_slice(&0u16.to_be_bytes()); // fsType: installable
    os2.extend_from_slice(&[0; 20]); // sub/superscript sizes and offsets
    os2.extend_from_slice(&i16v(top.underline_thickness)); // yStrikeoutSize
    os2.extend_from_slice(&i16v(upem * 0.25)); // yStrikeoutPosition
    os2.extend_from_slice(&0i16.to_be_bytes()); // sFamilyClass
    os2.extend_from_slice(&[0; 10]); // PANOSE
    os2.extend_from_slice(&[0; 16]); // ulUnicodeRange
    os2.extend_from_slice(b"    ");
    let sel: u16 = if italic { 1 } else { 0 } | if bold { 0x20 } else { 0 };
    os2.extend_from_slice(&(if sel == 0 { 0x40 } else { sel }).to_be_bytes());
    let first = map.first().map_or(0x20, |m| m.0) as u16;
    let last = map.last().map_or(0x20, |m| m.0) as u16;
    os2.extend_from_slice(&first.to_be_bytes());
    os2.extend_from_slice(&last.to_be_bytes());
    os2.extend_from_slice(&i16v(bbox[3]));
    os2.extend_from_slice(&i16v(bbox[1]));
    os2.extend_from_slice(&0i16.to_be_bytes());
    os2.extend_from_slice(&u16v(bbox[3]));
    os2.extend_from_slice(&u16v(-bbox[1]));
    os2.extend_from_slice(&[0; 8]); // code page ranges
    os2.extend_from_slice(&i16v(top_of("x").unwrap_or(upem * 0.45)));
    os2.extend_from_slice(&i16v(top_of("H").unwrap_or(upem * 0.7)));
    os2.extend_from_slice(&0u16.to_be_bytes());
    os2.extend_from_slice(&0x20u16.to_be_bytes());
    os2.extend_from_slice(&1u16.to_be_bytes());
    let mut post = 0x0003_0000u32.to_be_bytes().to_vec();
    post.extend_from_slice(&((top.italic_angle * 65536.0).round() as i32).to_be_bytes());
    post.extend_from_slice(&i16v(top.underline_position));
    post.extend_from_slice(&i16v(top.underline_thickness));
    post.extend_from_slice(&(top.fixed_pitch as u32).to_be_bytes());
    post.extend_from_slice(&[0; 16]);
    let subfamily = match (bold, italic) {
        (true, true) => "Bold Italic",
        (true, false) => "Bold",
        (false, true) => "Italic",
        _ => "Regular",
    };
    let mut names = Vec::new();
    if let Some(n) = &top.notice {
        names.push((0u16, n.clone()));
    }
    names.extend([
        (1u16, family.clone()),
        (2, subfamily.to_owned()),
        (3, top.font_name.clone()),
        (4, full.clone()),
        (6, top.font_name.clone()),
    ]);
    Ok(sfnt::write(
        sfnt::OTTO,
        vec![
            (*b"CFF ", cff),
            (*b"head", head),
            (*b"hhea", hhea),
            (*b"hmtx", hmtx),
            (*b"maxp", maxp),
            (*b"OS/2", os2),
            (*b"post", post),
            (*b"name", name_table(&names)),
            (*b"cmap", cmap_table(&map)),
        ],
    ))
}

/// A Type 1 font's glyphs by its own encoding, for drawing text set in that encoding (a
/// TeX DVI file's characters).
#[derive(Clone, Debug)]
pub struct Type1Glyphs {
    pub font_name: String,
    /// Font units per em (1000 for the usual matrix).
    pub units_per_em: f32,
    /// By character code: the advance and outline in font units, y up.
    pub glyphs: Vec<Option<(f32, Vec<PathEl>)>>,
}

/// Reads a Type 1 font's glyphs by character code.
pub fn glyphs_by_code(bytes: &[u8]) -> Result<Type1Glyphs, Error> {
    let (clear, private) = segments(bytes)?;
    let top = read_top(&clear);
    let p = read_private(&private)?;
    let by_name: std::collections::HashMap<&str, usize> = p
        .glyphs
        .iter()
        .enumerate()
        .map(|(i, g)| (g.0.as_str(), i))
        .collect();
    let mut run = Run {
        p: &p,
        by_name: &by_name,
        ops: 0,
    };
    let mut glyphs = vec![None; 256];
    for (code, name) in &top.encoding {
        let Some(&i) = by_name.get(name.as_str()) else {
            continue;
        };
        run.ops = 0;
        let Ok(g) = run.glyph(i, 0.0, 0.0, 0) else {
            continue;
        };
        let mut path = Vec::with_capacity(g.segs.len() + 8);
        let mut open = false;
        for s in &g.segs {
            match *s {
                Seg::Move(x, y) => {
                    if open {
                        path.push(PathEl::Close);
                    }
                    path.push(PathEl::MoveTo(x as f32, y as f32));
                    open = true;
                }
                Seg::Line(x, y) if open => path.push(PathEl::LineTo(x as f32, y as f32)),
                Seg::Curve(a, b, c, d, e, f) if open => path.push(PathEl::CubicTo(
                    a as f32, b as f32, c as f32, d as f32, e as f32, f as f32,
                )),
                _ => {}
            }
        }
        if open {
            path.push(PathEl::Close);
        }
        glyphs[*code as usize] = Some((g.width as f32, path));
    }
    let upem = top
        .matrix
        .as_ref()
        .map(|m| m[0])
        .filter(|a| *a > 0.0)
        .map_or(1000.0, |a| 1.0 / a) as f32;
    Ok(Type1Glyphs {
        font_name: top.font_name,
        units_per_em: upem,
        glyphs,
    })
}

/// Adobe StandardEncoding.
fn standard_name(code: u8) -> Option<&'static str> {
    const LOW: [&str; 95] = [
        "space",
        "exclam",
        "quotedbl",
        "numbersign",
        "dollar",
        "percent",
        "ampersand",
        "quoteright",
        "parenleft",
        "parenright",
        "asterisk",
        "plus",
        "comma",
        "hyphen",
        "period",
        "slash",
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "colon",
        "semicolon",
        "less",
        "equal",
        "greater",
        "question",
        "at",
        "A",
        "B",
        "C",
        "D",
        "E",
        "F",
        "G",
        "H",
        "I",
        "J",
        "K",
        "L",
        "M",
        "N",
        "O",
        "P",
        "Q",
        "R",
        "S",
        "T",
        "U",
        "V",
        "W",
        "X",
        "Y",
        "Z",
        "bracketleft",
        "backslash",
        "bracketright",
        "asciicircum",
        "underscore",
        "quoteleft",
        "a",
        "b",
        "c",
        "d",
        "e",
        "f",
        "g",
        "h",
        "i",
        "j",
        "k",
        "l",
        "m",
        "n",
        "o",
        "p",
        "q",
        "r",
        "s",
        "t",
        "u",
        "v",
        "w",
        "x",
        "y",
        "z",
        "braceleft",
        "bar",
        "braceright",
        "asciitilde",
    ];
    const HIGH: &[(u8, &str)] = &[
        (161, "exclamdown"),
        (162, "cent"),
        (163, "sterling"),
        (164, "fraction"),
        (165, "yen"),
        (166, "florin"),
        (167, "section"),
        (168, "currency"),
        (169, "quotesingle"),
        (170, "quotedblleft"),
        (171, "guillemotleft"),
        (172, "guilsinglleft"),
        (173, "guilsinglright"),
        (174, "fi"),
        (175, "fl"),
        (177, "endash"),
        (178, "dagger"),
        (179, "daggerdbl"),
        (180, "periodcentered"),
        (182, "paragraph"),
        (183, "bullet"),
        (184, "quotesinglbase"),
        (185, "quotedblbase"),
        (186, "quotedblright"),
        (187, "guillemotright"),
        (188, "ellipsis"),
        (189, "perthousand"),
        (191, "questiondown"),
        (193, "grave"),
        (194, "acute"),
        (195, "circumflex"),
        (196, "tilde"),
        (197, "macron"),
        (198, "breve"),
        (199, "dotaccent"),
        (200, "dieresis"),
        (202, "ring"),
        (203, "cedilla"),
        (205, "hungarumlaut"),
        (206, "ogonek"),
        (207, "caron"),
        (208, "emdash"),
        (225, "AE"),
        (227, "ordfeminine"),
        (232, "Lslash"),
        (233, "Oslash"),
        (234, "OE"),
        (235, "ordmasculine"),
        (241, "ae"),
        (245, "dotlessi"),
        (248, "lslash"),
        (249, "oslash"),
        (250, "oe"),
        (251, "germandbls"),
    ];
    match code {
        32..=126 => Some(LOW[code as usize - 32]),
        _ => HIGH.iter().find(|h| h.0 == code).map(|h| h.1),
    }
}

/// The Unicode character a glyph name stands for: `uniXXXX`, `uXXXX[XX]`, or an Adobe Glyph
/// List name of the Latin, punctuation and Greek capitals sets.
fn unicode_of(name: &str) -> Option<u32> {
    let base = name.split('.').next().unwrap_or(name);
    if let Some(h) = base.strip_prefix("uni")
        && h.len() == 4
    {
        return u32::from_str_radix(h, 16).ok();
    }
    if let Some(h) = base.strip_prefix('u')
        && (4..=6).contains(&h.len())
        && h.chars().all(|c| c.is_ascii_hexdigit())
    {
        return u32::from_str_radix(h, 16).ok();
    }
    if name != base {
        return None;
    }
    if base.len() == 1 && base.as_bytes()[0].is_ascii_alphabetic() {
        return Some(base.as_bytes()[0] as u32);
    }
    AGL.iter().find(|a| a.0 == name).map(|a| a.1 as u32)
}

const AGL: &[(&str, u16)] = &[
    ("space", 0x20),
    ("exclam", 0x21),
    ("quotedbl", 0x22),
    ("numbersign", 0x23),
    ("dollar", 0x24),
    ("percent", 0x25),
    ("ampersand", 0x26),
    ("quotesingle", 0x27),
    ("parenleft", 0x28),
    ("parenright", 0x29),
    ("asterisk", 0x2A),
    ("plus", 0x2B),
    ("comma", 0x2C),
    ("hyphen", 0x2D),
    ("period", 0x2E),
    ("slash", 0x2F),
    ("zero", 0x30),
    ("one", 0x31),
    ("two", 0x32),
    ("three", 0x33),
    ("four", 0x34),
    ("five", 0x35),
    ("six", 0x36),
    ("seven", 0x37),
    ("eight", 0x38),
    ("nine", 0x39),
    ("colon", 0x3A),
    ("semicolon", 0x3B),
    ("less", 0x3C),
    ("equal", 0x3D),
    ("greater", 0x3E),
    ("question", 0x3F),
    ("at", 0x40),
    ("bracketleft", 0x5B),
    ("backslash", 0x5C),
    ("bracketright", 0x5D),
    ("asciicircum", 0x5E),
    ("underscore", 0x5F),
    ("grave", 0x60),
    ("braceleft", 0x7B),
    ("bar", 0x7C),
    ("braceright", 0x7D),
    ("asciitilde", 0x7E),
    ("exclamdown", 0xA1),
    ("cent", 0xA2),
    ("sterling", 0xA3),
    ("currency", 0xA4),
    ("yen", 0xA5),
    ("brokenbar", 0xA6),
    ("section", 0xA7),
    ("dieresis", 0xA8),
    ("copyright", 0xA9),
    ("ordfeminine", 0xAA),
    ("guillemotleft", 0xAB),
    ("logicalnot", 0xAC),
    ("registered", 0xAE),
    ("macron", 0xAF),
    ("degree", 0xB0),
    ("plusminus", 0xB1),
    ("twosuperior", 0xB2),
    ("threesuperior", 0xB3),
    ("acute", 0xB4),
    ("mu", 0xB5),
    ("paragraph", 0xB6),
    ("periodcentered", 0xB7),
    ("cedilla", 0xB8),
    ("onesuperior", 0xB9),
    ("ordmasculine", 0xBA),
    ("guillemotright", 0xBB),
    ("onequarter", 0xBC),
    ("onehalf", 0xBD),
    ("threequarters", 0xBE),
    ("questiondown", 0xBF),
    ("Agrave", 0xC0),
    ("Aacute", 0xC1),
    ("Acircumflex", 0xC2),
    ("Atilde", 0xC3),
    ("Adieresis", 0xC4),
    ("Aring", 0xC5),
    ("AE", 0xC6),
    ("Ccedilla", 0xC7),
    ("Egrave", 0xC8),
    ("Eacute", 0xC9),
    ("Ecircumflex", 0xCA),
    ("Edieresis", 0xCB),
    ("Igrave", 0xCC),
    ("Iacute", 0xCD),
    ("Icircumflex", 0xCE),
    ("Idieresis", 0xCF),
    ("Eth", 0xD0),
    ("Ntilde", 0xD1),
    ("Ograve", 0xD2),
    ("Oacute", 0xD3),
    ("Ocircumflex", 0xD4),
    ("Otilde", 0xD5),
    ("Odieresis", 0xD6),
    ("multiply", 0xD7),
    ("Oslash", 0xD8),
    ("Ugrave", 0xD9),
    ("Uacute", 0xDA),
    ("Ucircumflex", 0xDB),
    ("Udieresis", 0xDC),
    ("Yacute", 0xDD),
    ("Thorn", 0xDE),
    ("germandbls", 0xDF),
    ("agrave", 0xE0),
    ("aacute", 0xE1),
    ("acircumflex", 0xE2),
    ("atilde", 0xE3),
    ("adieresis", 0xE4),
    ("aring", 0xE5),
    ("ae", 0xE6),
    ("ccedilla", 0xE7),
    ("egrave", 0xE8),
    ("eacute", 0xE9),
    ("ecircumflex", 0xEA),
    ("edieresis", 0xEB),
    ("igrave", 0xEC),
    ("iacute", 0xED),
    ("icircumflex", 0xEE),
    ("idieresis", 0xEF),
    ("eth", 0xF0),
    ("ntilde", 0xF1),
    ("ograve", 0xF2),
    ("oacute", 0xF3),
    ("ocircumflex", 0xF4),
    ("otilde", 0xF5),
    ("odieresis", 0xF6),
    ("divide", 0xF7),
    ("oslash", 0xF8),
    ("ugrave", 0xF9),
    ("uacute", 0xFA),
    ("ucircumflex", 0xFB),
    ("udieresis", 0xFC),
    ("yacute", 0xFD),
    ("thorn", 0xFE),
    ("ydieresis", 0xFF),
    ("dotlessi", 0x131),
    ("Lslash", 0x141),
    ("lslash", 0x142),
    ("OE", 0x152),
    ("oe", 0x153),
    ("Scaron", 0x160),
    ("scaron", 0x161),
    ("Ydieresis", 0x178),
    ("Zcaron", 0x17D),
    ("zcaron", 0x17E),
    ("florin", 0x192),
    ("dotlessj", 0x237),
    ("circumflex", 0x2C6),
    ("caron", 0x2C7),
    ("breve", 0x2D8),
    ("dotaccent", 0x2D9),
    ("ring", 0x2DA),
    ("ogonek", 0x2DB),
    ("tilde", 0x2DC),
    ("hungarumlaut", 0x2DD),
    ("Gamma", 0x393),
    ("Delta", 0x394),
    ("Theta", 0x398),
    ("Lambda", 0x39B),
    ("Xi", 0x39E),
    ("Pi", 0x3A0),
    ("Sigma", 0x3A3),
    ("Upsilon", 0x3A5),
    ("Phi", 0x3A6),
    ("Psi", 0x3A8),
    ("Omega", 0x3A9),
    ("endash", 0x2013),
    ("emdash", 0x2014),
    ("quoteleft", 0x2018),
    ("quoteright", 0x2019),
    ("quotesinglbase", 0x201A),
    ("quotedblleft", 0x201C),
    ("quotedblright", 0x201D),
    ("quotedblbase", 0x201E),
    ("dagger", 0x2020),
    ("daggerdbl", 0x2021),
    ("bullet", 0x2022),
    ("ellipsis", 0x2026),
    ("perthousand", 0x2030),
    ("guilsinglleft", 0x2039),
    ("guilsinglright", 0x203A),
    ("fraction", 0x2044),
    ("Euro", 0x20AC),
    ("trademark", 0x2122),
    ("minus", 0x2212),
    ("ff", 0xFB00),
    ("fi", 0xFB01),
    ("fl", 0xFB02),
    ("ffi", 0xFB03),
    ("ffl", 0xFB04),
];

/// A generated font for tests elsewhere.
#[doc(hidden)]
pub mod fixture {
    /// Encrypts like eexec / charstring encryption (the inverse of `decrypt`).
    pub fn encrypt(plain: &[u8], key: u16, lead: usize) -> Vec<u8> {
        let mut r = key;
        let mut out = Vec::new();
        for &p in std::iter::repeat_n(&0u8, lead).chain(plain) {
            let c = p ^ (r >> 8) as u8;
            r = (c as u16)
                .wrapping_add(r)
                .wrapping_mul(52845)
                .wrapping_add(22719);
            out.push(c);
        }
        out
    }

    /// A two-glyph font: `.notdef` and an "A" that is a 300×700 box at x = 100, with its
    /// rectangle drawn through a subroutine.
    pub fn tiny_font(pfb: bool) -> Vec<u8> {
        let clear = b"%!PS-AdobeFont-1.0: Tiny 001\n/FontName /Tiny def\n/FullName (Tiny Regular) def\n/Weight (Bold) def\n/Encoding StandardEncoding def\n/FontMatrix [0.001 0 0 0.001 0 0] readonly def\n/FontBBox {0 0 500 700} readonly def\ncurrentfile eexec\n";
        let cs = |plain: &[u8]| encrypt(plain, 4330, 4);
        // hsbw 0 500; rmoveto 100 0; callsubr 0; closepath; endchar
        let a = cs(&[139, 248, 136, 13, 239, 139, 21, 139, 10, 9, 14]);
        // rlineto 300 0; rlineto 0 700; return
        let sub = cs(&[247, 192, 139, 5, 139, 249, 80, 5, 11]);
        let notdef = cs(&[139, 248, 136, 13, 14]);
        let mut private = b"dup /Private 8 dict dup begin /lenIV 4 def /Subrs 1 array\n".to_vec();
        private.extend_from_slice(format!("dup 0 {} RD ", sub.len()).as_bytes());
        private.extend_from_slice(&sub);
        private.extend_from_slice(b" NP\nND\n2 index /CharStrings 2 dict dup begin\n");
        for (n, c) in [(".notdef", &notdef), ("A", &a)] {
            private.extend_from_slice(format!("/{n} {} RD ", c.len()).as_bytes());
            private.extend_from_slice(c);
            private.extend_from_slice(b" ND\n");
        }
        private.extend_from_slice(b"end\nend\n");
        let enc = encrypt(&private, 55665, 4);
        let mut out = Vec::new();
        if pfb {
            for (kind, body) in [(1u8, &clear[..]), (2, &enc[..])] {
                out.extend_from_slice(&[0x80, kind]);
                out.extend_from_slice(&(body.len() as u32).to_le_bytes());
                out.extend_from_slice(body);
            }
            out.extend_from_slice(&[0x80, 3]);
        } else {
            out.extend_from_slice(clear);
            for (i, b) in enc.iter().enumerate() {
                out.extend_from_slice(format!("{b:02x}").as_bytes());
                if i % 32 == 31 {
                    out.push(b'\n');
                }
            }
            out.extend_from_slice(b"\n0000000000000000\ncleartomark\n");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::{encrypt, tiny_font};
    use super::*;

    #[test]
    fn crypt_round_trip() {
        let e = encrypt(b"hello", 55665, 4);
        assert_eq!(decrypt(&e, 55665, 4), b"hello");
    }

    #[test]
    fn tiny_font_opens() {
        for pfb in [true, false] {
            let bytes = tiny_font(pfb);
            assert!(sniff(&bytes));
            let file = crate::open(&bytes, Default::default()).unwrap();
            assert_eq!(file.container, crate::Container::Type1);
            let face = &file.faces[0];
            assert_eq!(face.glyph_count, 2);
            let gid = face.glyph_index('A').unwrap();
            assert_eq!(face.glyph_name(gid).as_deref(), Some("A"));
            assert_eq!(face.advance(gid), Some(500));
            assert_eq!(face.outline(gid).unwrap().bbox, [100.0, 0.0, 400.0, 700.0]);
            assert_eq!(face.weight, 700);
            let by_code = glyphs_by_code(&bytes).unwrap();
            assert_eq!(by_code.glyphs[65].as_ref().unwrap().0, 500.0);
            assert!(by_code.glyphs[66].is_none());
        }
    }

    #[test]
    fn damaged_fonts_fail_cleanly() {
        let bytes = tiny_font(true);
        for n in 0..bytes.len() {
            let _ = crate::open(&bytes[..n], Default::default());
            let _ = glyphs_by_code(&bytes[..n]);
        }
        let mut flipped = bytes.clone();
        for i in (200..flipped.len()).step_by(7) {
            flipped[i] ^= 0x5A;
            let _ = crate::open(&flipped, Default::default());
        }
    }

    #[test]
    fn names() {
        assert_eq!(unicode_of("A"), Some(0x41));
        assert_eq!(unicode_of("uni20AC"), Some(0x20AC));
        assert_eq!(unicode_of("u1F600"), Some(0x1F600));
        assert_eq!(unicode_of("Aacute"), Some(0xC1));
        assert_eq!(unicode_of("A.sc"), None);
        assert_eq!(unicode_of("foo"), None);
        assert_eq!(standard_name(65), Some("A"));
        assert_eq!(standard_name(225), Some("AE"));
        assert_eq!(standard_name(10), None);
    }
}
