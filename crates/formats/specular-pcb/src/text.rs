//! Board text drawn as strokes: ODB++ `T` records with the job's own stroke font
//! (`fonts/standard`), IPC-2581 `Text` with its embedded glyphs, and a built-in stroke font
//! for anything else (our own design: printable ASCII on a 4 × 6 grid).
//!
//! A [`StrokeFont`] holds each glyph's strokes in the font's units, its cell (`xsize` ×
//! `ysize`, the character pitch and height), the gap after each cell and where the cell's
//! lower-left corner sits in glyph coordinates. [`StrokeFont::draw`] lays a string out from a
//! lower-left point with a cell size and pen of the caller's, then places it with an affine
//! (rotation, mirror, position).

use std::collections::HashMap;

use specular_vector::kurbo::{Affine, Point};

use crate::features::{self, Pen};
use crate::gerber::Object;

/// One stroke of a glyph (font units).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Stroke {
    pub a: Point,
    pub b: Point,
    /// Square ends (ODB++ `S`), else round.
    pub square: bool,
    /// Clear (ODB++ `N`): drawn in the opposite polarity.
    pub clear: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct StrokeFont {
    /// Cell width and height (font units).
    pub xsize: f64,
    pub ysize: f64,
    /// Gap after each cell (font units).
    pub offset: f64,
    /// The cell's lower-left corner in glyph coordinates.
    pub origin: Point,
    pub glyphs: HashMap<char, Vec<Stroke>>,
}

/// Built-in glyphs: polylines of two-digit grid points (`x` 0–4, `y` 0–9 with the baseline at
/// 2 and the cap height at 8), separated by spaces; a single point is a dot.
const BUILTIN: [(char, &str); 94] = [
    ('!', "2824 22"),
    ('"', "1817 3837"),
    ('#', "1317 3337 0444 0646"),
    ('$', "473818070615354443321203 2921"),
    ('%', "0248 0818170708 3343423233"),
    ('&', "4216172837360403122244"),
    ('\'', "2826"),
    ('(', "392816142231"),
    (')', "192836342211"),
    ('*', "2723 0644 0446"),
    ('+', "2327 0545"),
    (',', "232211"),
    ('-', "0545"),
    ('.', "22"),
    ('/', "0248"),
    ('0', "183847433212030718 0347"),
    ('1', "172822 1232"),
    ('2', "07183847460242"),
    ('3', "07183847463515 354443321203"),
    ('4', "32380444"),
    ('5', "480805354443321203"),
    ('6', "473818070312324344351504"),
    ('7', "084812"),
    ('8', "15060718384746351504031232434435"),
    ('9', "031232434738180706153546"),
    (':', "26 23"),
    (';', "26 232211"),
    ('<', "480542"),
    ('=', "0646 0444"),
    ('>', "084502"),
    ('?', "071838474624 22"),
    ('@', "34141636334347381807031242"),
    ('A', "0206284642 0545"),
    ('B', "02083847463505 3544433202"),
    ('C', "4738180703123243"),
    ('D', "02083847433202"),
    ('E', "48080242 0535"),
    ('F', "480802 0535"),
    ('G', "47381807031232434525"),
    ('H', "0208 4842 0545"),
    ('I', "1838 2822 1232"),
    ('J', "2848 3833221203"),
    ('K', "0208 4804 1542"),
    ('L', "080242"),
    ('M', "0208254842"),
    ('N', "02084248"),
    ('O', "183847433212030718"),
    ('P', "02083847463505"),
    ('Q', "183847433212030718 2442"),
    ('R', "02083847463505 2542"),
    ('S', "473818070615354443321203"),
    ('T', "0848 2822"),
    ('U', "080312324348"),
    ('V', "082248"),
    ('W', "0812253248"),
    ('X', "0842 0248"),
    ('Y', "082548 2522"),
    ('Z', "08480242"),
    ('[', "39292131"),
    ('\\', "0842"),
    (']', "19292111"),
    ('^', "062846"),
    ('_', "0141"),
    ('`', "1827"),
    ('a', "16364542 433212031444"),
    ('b', "0802 0516364543321203"),
    ('c', "4536160503123243"),
    ('d', "4842 4536160503123243"),
    ('e', "04444536160503123243"),
    ('f', "38281712 0636"),
    ('g', "4536160504133344 4641301001"),
    ('h', "0802 0516364542"),
    ('i', "2622 28"),
    ('j', "3631201001 38"),
    ('k', "0802 4603 1442"),
    ('l', "18282332"),
    ('m', "0206 05162522 25364542"),
    ('n', "0206 0516364542"),
    ('o', "163645433212030516"),
    ('p', "0600 0516364543321203"),
    ('q', "4640 4536160503123243"),
    ('r', "0206 05163645"),
    ('s', "45361605143443321203"),
    ('t', "18132232 0636"),
    ('u', "0603123243 4642"),
    ('v', "062246"),
    ('w', "0612243246"),
    ('x', "0642 0246"),
    ('y', "0623 4610"),
    ('z', "06460242"),
    ('{', "39282615242231"),
    ('|', "2921"),
    ('}', "19282635242211"),
    ('~', "05163546"),
];

/// Strokes of a polyline string of [`BUILTIN`].
fn builtin_strokes(s: &str) -> Vec<Stroke> {
    let mut out = Vec::new();
    for run in s.split_whitespace() {
        let pts: Vec<Point> = run
            .as_bytes()
            .chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| Point::new(f64::from(c[0] - b'0'), f64::from(c[1] - b'0')))
            .collect();
        if pts.len() == 1 {
            out.push(Stroke {
                a: pts[0],
                b: pts[0],
                square: false,
                clear: false,
            });
        }
        for w in pts.windows(2) {
            out.push(Stroke {
                a: w[0],
                b: w[1],
                square: false,
                clear: false,
            });
        }
    }
    out
}

impl StrokeFont {
    /// The built-in font: a 6-unit cell (4-unit glyphs and a 2-unit gap), cap height 6.
    pub fn builtin() -> StrokeFont {
        StrokeFont {
            xsize: 6.0,
            ysize: 6.0,
            offset: 0.0,
            origin: Point::new(-1.0, 2.0),
            glyphs: BUILTIN
                .iter()
                .map(|(c, s)| (*c, builtin_strokes(s)))
                .collect(),
        }
    }

    /// An ODB++ font file (`fonts/standard`): `XSIZE`, `YSIZE`, `OFFSET`, then
    /// `CHAR c` … `LINE x1 y1 x2 y2 pol shape width` … `ECHAR` blocks, in inches.
    /// `None` when it has no glyphs.
    pub fn odb(text: &str) -> Option<StrokeFont> {
        let mut f = StrokeFont {
            xsize: 0.0,
            ysize: 0.0,
            offset: 0.0,
            origin: Point::ZERO,
            glyphs: HashMap::new(),
        };
        let mut cur: Option<(char, Vec<Stroke>)> = None;
        let num = |s: &str| s.parse::<f64>().ok().filter(|v| v.is_finite());
        for line in text.lines() {
            let t: Vec<&str> = line.split_whitespace().collect();
            match t.first().map(|s| s.to_ascii_uppercase()).as_deref() {
                Some("XSIZE") => f.xsize = t.get(1).and_then(|s| num(s)).unwrap_or(0.0),
                Some("YSIZE") => f.ysize = t.get(1).and_then(|s| num(s)).unwrap_or(0.0),
                Some("OFFSET") => f.offset = t.get(1).and_then(|s| num(s)).unwrap_or(0.0),
                Some("CHAR") => {
                    // `CHAR` alone (or a space after it) is the space character.
                    let c = line
                        .trim_start()
                        .get(4..)
                        .and_then(|r| r.strip_prefix(' ').unwrap_or(r).chars().next())
                        .unwrap_or(' ');
                    cur = Some((c, Vec::new()));
                }
                Some("LINE") if t.len() >= 5 => {
                    let v: Option<Vec<f64>> = t[1..5].iter().map(|s| num(s)).collect();
                    if let (Some((_, strokes)), Some(v)) = (cur.as_mut(), v) {
                        strokes.push(Stroke {
                            a: Point::new(v[0], v[1]),
                            b: Point::new(v[2], v[3]),
                            clear: t.get(5).is_some_and(|p| p.eq_ignore_ascii_case("N")),
                            square: t.get(6).is_some_and(|s| s.eq_ignore_ascii_case("S")),
                        });
                    }
                }
                Some("ECHAR") => {
                    if let Some((c, s)) = cur.take() {
                        f.glyphs.insert(c, s);
                    }
                }
                _ => {}
            }
        }
        if f.glyphs.is_empty() || f.xsize <= 0.0 || f.ysize <= 0.0 {
            return None;
        }
        // The cell around the glyphs: centred on the capitals' and digits' middle, its
        // bottom at their baseline (the standard font draws about x = 0 from y = -0.1).
        let (mut x0, mut x1, mut y0) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY);
        for (c, s) in &f.glyphs {
            if c.is_ascii_uppercase() || c.is_ascii_digit() {
                for k in s {
                    x0 = x0.min(k.a.x.min(k.b.x));
                    x1 = x1.max(k.a.x.max(k.b.x));
                    y0 = y0.min(k.a.y.min(k.b.y));
                }
            }
        }
        if x0.is_finite() && x1.is_finite() && y0.is_finite() {
            f.origin = Point::new((x0 + x1) / 2.0 - f.xsize / 2.0, y0);
        }
        Some(f)
    }

    /// `text` as stroke objects: laid out from its lower-left corner at the origin in cells
    /// `cell_w` × `cell_h` wide and high (`gap` more between cells), stroked with `width`,
    /// then placed by `place`. Characters the font lacks take the built-in font's glyph,
    /// else leave their cell empty; `missing` counts those.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        text: &str,
        cell_w: f64,
        cell_h: f64,
        gap: f64,
        width: f64,
        place: Affine,
        dark: bool,
        missing: &mut u64,
    ) -> Vec<Object> {
        let builtin = std::sync::LazyLock::force(&BUILTIN_FONT);
        let mut out = Vec::new();
        let mut line = 0.0;
        let mut col = 0usize;
        for c in text.chars() {
            if c == '\n' {
                line += 1.0;
                col = 0;
                continue;
            }
            let (font, strokes) = match self.glyphs.get(&c) {
                Some(s) => (self, s),
                None => match builtin.glyphs.get(&c) {
                    Some(s) => (builtin, s),
                    None => {
                        if !c.is_whitespace() {
                            *missing += 1;
                        }
                        col += 1;
                        continue;
                    }
                },
            };
            let sx = cell_w / font.xsize;
            let sy = cell_h / font.ysize;
            let x = col as f64 * (cell_w + gap);
            let y = -line * cell_h * 1.5;
            let cell = place
                * Affine::translate((x, y))
                * Affine::scale_non_uniform(sx, sy)
                * Affine::translate(-font.origin.to_vec2());
            for k in strokes {
                let pen = if k.square {
                    Pen::Square(width)
                } else {
                    Pen::Round(width)
                };
                out.push(features::line(cell * k.a, cell * k.b, pen, dark != k.clear));
            }
            col += 1;
        }
        out
    }

    /// The width a string takes in cells of `cell_w` with `gap` between them (its longest
    /// line).
    pub fn advance(text: &str, cell_w: f64, gap: f64) -> f64 {
        let n = text
            .split('\n')
            .map(|l| l.chars().count())
            .max()
            .unwrap_or(0);
        if n == 0 {
            0.0
        } else {
            n as f64 * cell_w + (n - 1) as f64 * gap
        }
    }
}

static BUILTIN_FONT: std::sync::LazyLock<StrokeFont> =
    std::sync::LazyLock::new(StrokeFont::builtin);

/// The built-in font, shared.
pub(crate) fn builtin() -> &'static StrokeFont {
    &BUILTIN_FONT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_covers_printable_ascii() {
        let f = StrokeFont::builtin();
        for c in '!'..='~' {
            let s = f.glyphs.get(&c).unwrap_or_else(|| panic!("{c}"));
            assert!(!s.is_empty(), "{c}");
            for k in s {
                for p in [k.a, k.b] {
                    assert!(
                        (0.0..=4.0).contains(&p.x) && (0.0..=9.0).contains(&p.y),
                        "{c}"
                    );
                }
            }
        }
    }

    #[test]
    fn reads_an_odb_font() {
        let t = "XSIZE 0.302\nYSIZE 0.302\nOFFSET 0\n\nCHAR !\nLINE 0 0 0 0.2 P R 0.012\n\
                 LINE 0 -0.1 0 -0.1 P R 0.012\nECHAR\n\nCHAR A\nLINE -0.1 -0.1 0 0.2 P R 0.012\n\
                 LINE 0 0.2 0.1 -0.1 P S 0.012\nECHAR\n";
        let f = StrokeFont::odb(t).unwrap();
        assert_eq!(f.glyphs.len(), 2);
        assert!(f.glyphs[&'A'][1].square);
        assert!((f.origin.x + 0.151).abs() < 1e-9, "{:?}", f.origin);
        assert!((f.origin.y + 0.1).abs() < 1e-9);
    }

    #[test]
    fn lays_out_cells() {
        let f = StrokeFont::builtin();
        let mut missing = 0;
        // "I" in a 6 × 6 cell at 1:1: the stem at x = 3 (cell left 0, grid 2 + 1), from the
        // baseline 0 up to 6.
        let o = f.draw(
            "I",
            6.0,
            6.0,
            0.0,
            0.0,
            Affine::IDENTITY,
            true,
            &mut missing,
        );
        let seg = o.iter().find_map(|o| match o.segment {
            Some(crate::gerber::Segment::Line(a, b)) if (a.x - b.x).abs() < 1e-9 => Some((a, b)),
            _ => None,
        });
        let (a, b) = seg.unwrap();
        assert!((a.x - 3.0).abs() < 1e-9 && (a.y - 6.0).abs() < 1e-9 && b.y.abs() < 1e-9);
        f.draw(
            "\u{5d0}",
            1.0,
            1.0,
            0.0,
            0.1,
            Affine::IDENTITY,
            true,
            &mut missing,
        );
        assert_eq!(missing, 1);
    }
}
