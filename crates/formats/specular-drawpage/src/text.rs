//! Text boxes: paragraphs of styled runs laid out in a box (word wrap, alignment, vertical
//! anchoring), with the drawing stack's fonts.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use kurbo::Affine;
use specular_vector::fonts::{Face, Fonts};
use specular_vector::scene::FontSpec;

use crate::Rgba;

/// Horizontal alignment of a paragraph's lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HAlign {
    #[default]
    Left,
    Centre,
    Right,
    /// Lines but the last stretched to the box width.
    Justify,
}

/// Where the laid-out text sits in its box vertically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VAlign {
    #[default]
    Top,
    Middle,
    Bottom,
}

/// A run of text in one style.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub text: String,
    /// Family name as the document gives it ("Calibri", "Liberation Sans").
    pub family: String,
    pub bold: bool,
    pub italic: bool,
    /// Em size, points.
    pub size: f64,
    pub color: Rgba,
    pub underline: bool,
    pub strike: bool,
    /// Extra space after each character, points.
    pub letter_spacing: f64,
}

impl Run {
    pub fn new(text: impl Into<String>, family: impl Into<String>, size: f64) -> Run {
        Run {
            text: text.into(),
            family: family.into(),
            bold: false,
            italic: false,
            size,
            color: [0, 0, 0, 255],
            underline: false,
            strike: false,
            letter_spacing: 0.0,
        }
    }
}

/// A paragraph: runs laid out together, broken into lines at the box width.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Paragraph {
    pub runs: Vec<Run>,
    pub align: HAlign,
    /// Line height as a multiple of the font's (ascent − descent); 0 means 1.
    pub line_spacing: f64,
    /// Fixed line height in points (overrides `line_spacing` when set).
    pub line_height: Option<f64>,
    pub space_before: f64,
    pub space_after: f64,
    /// Left and right indents, points; first-line indent relative to the left one.
    pub indent_left: f64,
    pub indent_right: f64,
    pub indent_first: f64,
    /// Bullet text drawn before the first line (in the first run's style).
    pub bullet: Option<String>,
}

/// A box of text. Box space: origin at the box's top left, x right, y down, points;
/// `transform` maps it to the page (rotation, flips).
#[derive(Debug, Clone, PartialEq)]
pub struct TextBlock {
    pub transform: Affine,
    pub width: f64,
    pub height: f64,
    /// Break lines at the box width (else only at line breaks).
    pub wrap: bool,
    pub valign: VAlign,
    pub paragraphs: Vec<Paragraph>,
}

impl TextBlock {
    pub fn new(transform: Affine, width: f64, height: f64) -> TextBlock {
        TextBlock {
            transform,
            width,
            height,
            wrap: true,
            valign: VAlign::Top,
            paragraphs: Vec::new(),
        }
    }

    /// No visible characters.
    pub fn is_empty(&self) -> bool {
        self.paragraphs
            .iter()
            .all(|p| p.runs.iter().all(|r| r.text.trim().is_empty()))
    }
}

/// The face for a family name and style: installed, else a metric twin, else a generic of
/// the same kind (serif, monospace, sans), else the bundled default. Cached per process.
pub fn face(family: &str, bold: bool, italic: bool) -> Arc<Face> {
    type Cache = Mutex<HashMap<(String, bool, bool), Arc<Face>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let key = (family.to_ascii_lowercase(), bold, italic);
    let cache = CACHE.get_or_init(Default::default);
    if let Ok(c) = cache.lock()
        && let Some(f) = c.get(&key)
    {
        return f.clone();
    }
    let fonts = Fonts::shared();
    let f = resolve(&fonts, family, bold, italic);
    if let Ok(mut c) = cache.lock() {
        if c.len() > 4096 {
            c.clear();
        }
        c.insert(key, f.clone());
    }
    f
}

fn resolve(fonts: &Fonts, family: &str, bold: bool, italic: bool) -> Arc<Face> {
    let spec = FontSpec::default();
    let family = family.trim().trim_matches(['\'', '"']);
    if !family.is_empty() {
        let r = fonts.resolve(&spec, Some(family), bold, italic);
        if !r.substituted {
            return r.face;
        }
    }
    let lower = family.to_ascii_lowercase();
    let generic = if lower.contains("mono")
        || lower.contains("courier")
        || lower.contains("consol")
        || lower.contains("typewriter")
    {
        "Courier New"
    } else if (lower.contains("times")
        || lower.contains("serif")
        || lower.contains("roman")
        || lower.contains("georgia")
        || lower.contains("cambria")
        || lower.contains("garamond")
        || lower.contains("bookman")
        || lower.contains("book antiqua"))
        && !lower.contains("sans")
    {
        "Times New Roman"
    } else {
        "Arial"
    };
    fonts.resolve(&spec, Some(generic), bold, italic).face
}

/// One glyph placed in box space.
pub(crate) struct Placed {
    pub face: Arc<Face>,
    pub c: char,
    /// Baseline origin, box space.
    pub x: f64,
    pub y: f64,
    /// Font units → points.
    pub scale: f64,
    pub color: Rgba,
}

/// A decoration line (underline, strike-through), box space.
pub(crate) struct Line {
    pub x0: f64,
    pub x1: f64,
    pub y: f64,
    pub thickness: f64,
    pub color: Rgba,
}

/// A glyph of a paragraph before line breaking.
struct G {
    face: Arc<Face>,
    c: char,
    adv: f64,
    scale: f64,
    asc: f64,
    desc: f64,
    color: Rgba,
    underline: bool,
    strike: bool,
    size: f64,
    /// A break opportunity after this glyph (space, hyphen).
    brk: bool,
    space: bool,
    newline: bool,
}

/// Most glyphs laid out per text box.
const MAX_GLYPHS: usize = 200_000;

fn glyphs_of(p: &Paragraph, out: &mut Vec<G>) {
    let fonts = Fonts::shared();
    let mut runs: Vec<Run> = Vec::new();
    if let Some(b) = &p.bullet
        && let Some(first) = p.runs.first()
    {
        let mut r = first.clone();
        r.text = format!("{b}\u{2002}");
        r.underline = false;
        r.strike = false;
        runs.push(r);
    }
    runs.extend(p.runs.iter().cloned());
    for r in &runs {
        if !(r.size.is_finite() && r.size > 0.0) {
            continue;
        }
        let base = face(&r.family, r.bold, r.italic);
        for c in r.text.chars() {
            if out.len() >= MAX_GLYPHS {
                return;
            }
            let newline = matches!(c, '\n' | '\u{2028}' | '\u{b}');
            let tab = c == '\t';
            let c = match c {
                '\t' => ' ',
                '\r' => continue,
                '\u{a0}' => ' ',
                c => c,
            };
            let mut f = base.clone();
            if !newline
                && !c.is_whitespace()
                && !f.has(c)
                && let Some(fb) = fonts.fallback_for(&f, c)
            {
                f = fb;
            }
            let scale = r.size / f64::from(f.units_per_em.max(1.0));
            let adv = if newline {
                0.0
            } else {
                f.glyph(c)
                    .map_or(f64::from(f.units_per_em) * 0.5, |g| f64::from(g.advance))
                    * scale
                    + r.letter_spacing
            };
            let space = c == ' ';
            out.push(G {
                asc: f64::from(f.ascent) * scale,
                desc: f64::from(f.descent) * scale,
                face: f,
                c,
                adv: if tab { adv * 4.0 } else { adv },
                scale,
                color: r.color,
                underline: r.underline,
                strike: r.strike,
                size: r.size,
                brk: space || c == '-' || c == '\u{200b}' || is_cjk(c),
                space,
                newline,
            });
        }
    }
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32, 0x3000..=0x9fff | 0xac00..=0xd7af | 0xf900..=0xfaff | 0xff00..=0xffef)
}

/// Lays `t` out: glyphs and decoration lines in box space.
pub(crate) fn layout(t: &TextBlock) -> (Vec<Placed>, Vec<Line>) {
    struct L {
        start: usize,
        end: usize,
        width: f64,
        y: f64,
        last: bool,
        align: HAlign,
        left: f64,
        avail: f64,
    }
    let mut all: Vec<G> = Vec::new();
    let mut lines: Vec<L> = Vec::new();
    let mut y = 0.0;
    for (pi, p) in t.paragraphs.iter().enumerate() {
        let start = all.len();
        glyphs_of(p, &mut all);
        let end = all.len();
        if pi > 0 {
            y += p.space_before;
        }
        let first_size = p.runs.first().map_or(10.0, |r| r.size);
        let empty_line = {
            let f = p
                .runs
                .first()
                .map(|r| face(&r.family, r.bold, r.italic))
                .unwrap_or_else(|| Fonts::shared().default_face(false));
            let s = first_size / f64::from(f.units_per_em.max(1.0));
            (f64::from(f.ascent) * s, f64::from(f.descent) * s)
        };
        let spacing = if p.line_spacing > 0.0 {
            p.line_spacing
        } else {
            1.0
        };
        let mut i = start;
        let mut first = true;
        loop {
            let left = p.indent_left + if first { p.indent_first } else { 0.0 };
            let avail = (t.width - left - p.indent_right).max(1.0);
            // Greedy: take glyphs until the width is exceeded, then back up to a break.
            let mut j = i;
            let mut w = 0.0;
            let mut last_break: Option<usize> = None;
            let mut hard = false;
            while j < end {
                let g = &all[j];
                if g.newline {
                    hard = true;
                    break;
                }
                if t.wrap && w + g.adv > avail && j > i && !g.space {
                    break;
                }
                w += g.adv;
                if g.brk {
                    last_break = Some(j);
                }
                j += 1;
            }
            let mut line_end = j;
            if j < end
                && !hard
                && t.wrap
                && let Some(b) = last_break
            {
                line_end = b + 1;
            }
            if line_end == i && i < end && !hard {
                line_end = i + 1;
            }
            // Trailing spaces do not count towards the width.
            let mut vis_end = line_end;
            while vis_end > i && all[vis_end - 1].space {
                vis_end -= 1;
            }
            let width: f64 = all[i..vis_end].iter().map(|g| g.adv).sum();
            let (mut asc, mut desc) = (0.0f64, 0.0f64);
            for g in &all[i..line_end] {
                asc = asc.max(g.asc);
                desc = desc.min(g.desc);
            }
            if line_end == i {
                (asc, desc) = empty_line;
            }
            let natural = asc - desc;
            let lh = p.line_height.unwrap_or(natural * spacing);
            let next = if hard { line_end + 1 } else { line_end };
            let done = next >= end;
            lines.push(L {
                start: i,
                end: vis_end,
                width,
                // Extra leading goes above the line, as the office suites do.
                y: y + lh + desc,
                last: done || hard,
                align: p.align,
                left,
                avail,
            });
            y += lh;
            first = false;
            i = next;
            if done {
                // A trailing hard break makes an empty last line.
                if hard && next == end {
                    y += natural * spacing;
                }
                break;
            }
            if lines.len() > MAX_GLYPHS {
                break;
            }
        }
        y += p.space_after;
    }
    let total = y;
    let dy = match t.valign {
        VAlign::Top => 0.0,
        VAlign::Middle => (t.height - total) / 2.0,
        VAlign::Bottom => t.height - total,
    };
    let mut placed = Vec::with_capacity(all.len());
    let mut deco = Vec::new();
    for l in &lines {
        let slack = l.avail - l.width;
        let (mut x, gap) = match l.align {
            HAlign::Left => (l.left, 0.0),
            HAlign::Centre => (l.left + slack / 2.0, 0.0),
            HAlign::Right => (l.left + slack, 0.0),
            HAlign::Justify => {
                let spaces = all[l.start..l.end].iter().filter(|g| g.space).count();
                if l.last || spaces == 0 || slack <= 0.0 || !t.wrap {
                    (l.left, 0.0)
                } else {
                    (l.left, slack / spaces as f64)
                }
            }
        };
        let base = l.y + dy;
        for g in &all[l.start..l.end] {
            let adv = g.adv + if g.space { gap } else { 0.0 };
            if g.underline {
                deco.push(Line {
                    x0: x,
                    x1: x + adv,
                    y: base + g.size * 0.12,
                    thickness: g.size * 0.06,
                    color: g.color,
                });
            }
            if g.strike {
                deco.push(Line {
                    x0: x,
                    x1: x + adv,
                    y: base - g.size * 0.28,
                    thickness: g.size * 0.06,
                    color: g.color,
                });
            }
            if !g.space && !g.newline {
                placed.push(Placed {
                    face: g.face.clone(),
                    c: g.c,
                    x,
                    y: base,
                    scale: g.scale,
                    color: g.color,
                });
            }
            x += adv;
        }
    }
    (placed, deco)
}
