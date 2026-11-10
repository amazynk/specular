//! The clear-text encoding (ISO/IEC 8632-4): keywords, numbers, quoted strings; elements end
//! with `;` or `/`; `%…%` are comments.

use crate::args::{Args, CellColour};
use crate::interp::{Interp, latin1};

/// Longest element kept (characters); longer ones are cut there.
const MAX_ELEMENT: usize = 64 << 20;

/// Element keyword → (class, id) in the binary numbering.
const ELEMENTS: &[(&str, u8, u8)] = &[
    ("BEGMF", 0, 1),
    ("ENDMF", 0, 2),
    ("BEGPIC", 0, 3),
    ("BEGPICBODY", 0, 4),
    ("ENDPIC", 0, 5),
    ("BEGTILEARRAY", 0, 19),
    ("MFVERSION", 1, 1),
    ("MFDESC", 1, 2),
    ("VDCTYPE", 1, 3),
    ("INTEGERPREC", 1, 4),
    ("REALPREC", 1, 5),
    ("INDEXPREC", 1, 6),
    ("COLRPREC", 1, 7),
    ("COLRINDEXPREC", 1, 8),
    ("MAXCOLRINDEX", 1, 9),
    ("COLRVALUEEXT", 1, 10),
    ("FONTLIST", 1, 13),
    ("COLRMODEL", 1, 19),
    ("SCALEMODE", 2, 1),
    ("COLRMODE", 2, 2),
    ("LINEWIDTHMODE", 2, 3),
    ("MARKERSIZEMODE", 2, 4),
    ("EDGEWIDTHMODE", 2, 5),
    ("VDCEXT", 2, 6),
    ("BACKCOLR", 2, 7),
    ("LINEEDGETYPEDEF", 2, 16),
    ("HATCHSTYLEDEF", 2, 17),
    ("GEOPATDEF", 2, 18),
    ("VDCINTEGERPREC", 3, 1),
    ("VDCREALPREC", 3, 2),
    ("AUXCOLR", 3, 3),
    ("TRANSPARENCY", 3, 4),
    ("CLIPRECT", 3, 5),
    ("CLIP", 3, 6),
    ("TRANSPCELLCOLR", 3, 20),
    ("LINE", 4, 1),
    ("DISJTLINE", 4, 2),
    ("MARKER", 4, 3),
    ("TEXT", 4, 4),
    ("RESTRTEXT", 4, 5),
    ("APNDTEXT", 4, 6),
    ("POLYGON", 4, 7),
    ("POLYGONSET", 4, 8),
    ("CELLARRAY", 4, 9),
    ("GDP", 4, 10),
    ("RECT", 4, 11),
    ("CIRCLE", 4, 12),
    ("ARC3PT", 4, 13),
    ("ARC3PTCLOSE", 4, 14),
    ("ARCCTR", 4, 15),
    ("ARCCTRCLOSE", 4, 16),
    ("ELLIPSE", 4, 17),
    ("ELLIPARC", 4, 18),
    ("ELLIPARCCLOSE", 4, 19),
    ("ARCCTRREV", 4, 20),
    ("CONNEDGE", 4, 21),
    ("HYPERBARC", 4, 22),
    ("PARABARC", 4, 23),
    ("NUB", 4, 24),
    ("NURB", 4, 25),
    ("POLYBEZIER", 4, 26),
    ("POLYSYMBOL", 4, 27),
    ("BITONALTILE", 4, 28),
    ("TILE", 4, 29),
    ("LINEINDEX", 5, 1),
    ("LINETYPE", 5, 2),
    ("LINEWIDTH", 5, 3),
    ("LINECOLR", 5, 4),
    ("MARKERINDEX", 5, 5),
    ("MARKERTYPE", 5, 6),
    ("MARKERSIZE", 5, 7),
    ("MARKERCOLR", 5, 8),
    ("TEXTINDEX", 5, 9),
    ("TEXTFONTINDEX", 5, 10),
    ("TEXTPREC", 5, 11),
    ("CHAREXPAN", 5, 12),
    ("CHARSPACE", 5, 13),
    ("TEXTCOLR", 5, 14),
    ("CHARHEIGHT", 5, 15),
    ("CHARORI", 5, 16),
    ("TEXTPATH", 5, 17),
    ("TEXTALIGN", 5, 18),
    ("CHARSETINDEX", 5, 19),
    ("ALTCHARSETINDEX", 5, 20),
    ("FILLINDEX", 5, 21),
    ("INTSTYLE", 5, 22),
    ("FILLCOLR", 5, 23),
    ("HATCHINDEX", 5, 24),
    ("PATINDEX", 5, 25),
    ("EDGEINDEX", 5, 26),
    ("EDGETYPE", 5, 27),
    ("EDGEWIDTH", 5, 28),
    ("EDGECOLR", 5, 29),
    ("EDGEVIS", 5, 30),
    ("FILLREFPT", 5, 31),
    ("PATTABLE", 5, 32),
    ("PATSIZE", 5, 33),
    ("COLRTABLE", 5, 34),
    ("ASF", 5, 35),
    ("LINECAP", 5, 37),
    ("LINEJOIN", 5, 38),
    ("EDGECAP", 5, 44),
    ("EDGEJOIN", 5, 45),
];

/// Whether `head` starts with the keyword BEGMF (after blanks and comments).
pub(crate) fn starts_with_begmf(head: &[u8]) -> bool {
    let mut i = 0;
    loop {
        while i < head.len() && head[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < head.len() && head[i] == b'%' {
            match head[i + 1..].iter().position(|&c| c == b'%') {
                Some(j) => i += j + 2,
                None => return false,
            }
            continue;
        }
        break;
    }
    let word: Vec<u8> = head[i..]
        .iter()
        .take(16)
        .copied()
        .filter(|c| *c != b'_' && *c != b'$')
        .take_while(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    word == b"BEGMF"
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Word(String),
    Num(f64),
    Str(Vec<u8>),
}

/// Splits the file into elements (token lists).
struct Lexer<'a> {
    d: &'a [u8],
    p: usize,
}

impl Lexer<'_> {
    /// The next element's tokens; `None` at the end of the data.
    fn element(&mut self) -> Option<Vec<Tok>> {
        let mut toks = Vec::new();
        let start = self.p;
        loop {
            if self.p - start > MAX_ELEMENT {
                // Skip to the terminator.
                while self.p < self.d.len() && !matches!(self.d[self.p], b';' | b'/') {
                    self.p += 1;
                }
            }
            let Some(&c) = self.d.get(self.p) else {
                return (!toks.is_empty()).then_some(toks);
            };
            match c {
                b';' | b'/' => {
                    self.p += 1;
                    return Some(toks);
                }
                b'%' => {
                    self.p += 1;
                    while self.p < self.d.len() && self.d[self.p] != b'%' {
                        self.p += 1;
                    }
                    self.p += 1;
                }
                b'\'' | b'"' => {
                    self.p += 1;
                    let mut s = Vec::new();
                    while let Some(&x) = self.d.get(self.p) {
                        self.p += 1;
                        if x == c {
                            if self.d.get(self.p) == Some(&c) {
                                s.push(c);
                                self.p += 1;
                                continue;
                            }
                            break;
                        }
                        s.push(x);
                    }
                    toks.push(Tok::Str(s));
                }
                c if c.is_ascii_alphabetic() => {
                    let mut w = String::new();
                    while let Some(&x) = self.d.get(self.p) {
                        if x.is_ascii_alphanumeric() || x == b'_' || x == b'$' {
                            if x != b'_' && x != b'$' {
                                w.push(char::from(x.to_ascii_uppercase()));
                            }
                            self.p += 1;
                        } else {
                            break;
                        }
                    }
                    toks.push(Tok::Word(w));
                }
                c if c.is_ascii_digit() || matches!(c, b'+' | b'-' | b'.') => {
                    let s = self.p;
                    self.p += 1;
                    while let Some(&x) = self.d.get(self.p) {
                        if x.is_ascii_alphanumeric() || matches!(x, b'.' | b'#' | b'+' | b'-') {
                            // A sign only right after an exponent mark.
                            if matches!(x, b'+' | b'-')
                                && !matches!(self.d[self.p - 1], b'e' | b'E')
                            {
                                break;
                            }
                            self.p += 1;
                        } else {
                            break;
                        }
                    }
                    if let Some(v) = number(&self.d[s..self.p]) {
                        toks.push(Tok::Num(v));
                    }
                }
                _ => self.p += 1,
            }
        }
    }
}

/// A number: integer, real (with exponent) or based integer (`16#FF`).
fn number(b: &[u8]) -> Option<f64> {
    let s = std::str::from_utf8(b).ok()?;
    if let Some((base, digits)) = s.split_once('#') {
        let (neg, base) = match base.strip_prefix('-') {
            Some(b) => (true, b),
            None => (false, base.trim_start_matches('+')),
        };
        let base: u32 = base.parse().ok()?;
        if !(2..=16).contains(&base) {
            return None;
        }
        let v = i64::from_str_radix(digits, base).ok()? as f64;
        return Some(if neg { -v } else { v });
    }
    s.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Parameters of one clear-text element.
struct Toks<'a> {
    t: &'a [Tok],
    i: usize,
    components: usize,
}

impl Toks<'_> {
    fn num(&mut self) -> Option<f64> {
        match self.t.get(self.i)? {
            Tok::Num(v) => {
                self.i += 1;
                Some(*v)
            }
            _ => None,
        }
    }
}

impl Args for Toks<'_> {
    fn more(&self) -> bool {
        self.i < self.t.len()
    }
    fn int(&mut self) -> Option<i64> {
        self.num().map(|v| v.round() as i64)
    }
    fn index(&mut self) -> Option<i64> {
        self.int()
    }
    fn enumv(&mut self, names: &[&str]) -> Option<i64> {
        match self.t.get(self.i)? {
            Tok::Word(w) => {
                let i = names.iter().position(|n| n.eq_ignore_ascii_case(w));
                if i.is_some() {
                    self.i += 1;
                }
                i.map(|i| i as i64)
            }
            Tok::Num(_) => self.int(),
            Tok::Str(_) => None,
        }
    }
    fn real(&mut self) -> Option<f64> {
        self.num()
    }
    fn vdc(&mut self) -> Option<f64> {
        self.num()
    }
    fn ci(&mut self) -> Option<u32> {
        self.num()
            .map(|v| v.max(0.0).min(f64::from(u32::MAX)) as u32)
    }
    fn cd(&mut self) -> Option<[f64; 4]> {
        let mut c = [0.0; 4];
        for v in c.iter_mut().take(self.components) {
            *v = self.num()?;
        }
        Some(c)
    }
    fn string(&mut self) -> Option<Vec<u8>> {
        match self.t.get(self.i)? {
            Tok::Str(s) => {
                self.i += 1;
                Some(s.clone())
            }
            _ => None,
        }
    }
    fn cell_mode(&mut self) -> i64 {
        1
    }
    fn cells(
        &mut self,
        nx: usize,
        ny: usize,
        _local_prec: i64,
        _mode: i64,
        indexed: bool,
        out: &mut dyn FnMut(CellColour),
    ) -> Option<()> {
        for _ in 0..nx.saturating_mul(ny) {
            if indexed {
                out(CellColour::Index(self.ci()?));
            } else {
                out(CellColour::Direct(self.cd()?));
            }
        }
        Some(())
    }
}

/// Interprets a clear-text metafile; returns the BEGMF name.
pub(crate) fn run(data: &[u8], it: &mut Interp) -> Option<String> {
    let mut lx = Lexer { d: data, p: 0 };
    let mut name = None;
    while let Some(toks) = lx.element() {
        let Some(Tok::Word(w)) = toks.first() else {
            continue;
        };
        let params = &toks[1..];
        match w.as_str() {
            "BEGMFDEFAULTS" => {
                it.set_mdr(true);
                continue;
            }
            "ENDMFDEFAULTS" => {
                it.set_mdr(false);
                continue;
            }
            _ => {}
        }
        let Some(&(_, class, id)) = ELEMENTS.iter().find(|e| e.0 == w) else {
            // Unknown keywords (application structures, escapes, segments): nothing drawn.
            continue;
        };
        if class == 0 && id == 1 {
            name = params.iter().find_map(|t| match t {
                Tok::Str(s) => Some(latin1(s)),
                _ => None,
            });
        }
        let mut a = Toks {
            t: params,
            i: 0,
            components: usize::from(it.prec().components),
        };
        it.element(class, id, &mut a);
        if it.ended {
            break;
        }
    }
    name.filter(|s| !s.is_empty())
}
