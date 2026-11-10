//! DS9 region files (`.reg`, DS9 version 4.1 and the older forms DS9 still reads).
//!
//! [`parse`] reads every line it can and gives a plain reason for each one it cannot:
//! - systems: `image`, `physical` (through `LTM`/`LTV`), `fk5`/`j2000`, `fk4`/`b1950`,
//!   `icrs`, `galactic` and `wcs` (the image's own sky frame); `ecliptic`, `linear`,
//!   `detector` and `amplifier` are refused by name;
//! - shapes: circle, ellipse (and elliptical annuli), box (and box annuli), polygon, point
//!   (every marker), line, vector, text, annulus; `panda`, `epanda` and `bpanda` are read as
//!   their inner and outer edges (an approximation, said so); ruler, compass, projection,
//!   segment and composite are refused;
//! - properties: colour, width, dash, text, font size, include/exclude (`-`), source or
//!   background, and the `global` line's defaults.
//!
//! [`Region::place`] puts a region on an image: sky regions through its WCS (positions,
//! sizes from the local pixel scale and angles from the local direction of north, as DS9
//! and astropy-regions do), image regions directly. Placed shapes are in display pixels
//! (top-down, the first pixel spanning 0–1). [`write`] does the reverse for shapes drawn on
//! an image, in `fk5` when it has sky coordinates, else in `image`.

use std::f64::consts::PI;
use std::fmt::Write as _;

use specular_fits::catalogue::angle as sexagesimal;
use specular_fits::wcs::Wcs;

use crate::frames::{self, Sky};

/// The most regions one file gives.
pub const MAX_REGIONS: usize = 100_000;
/// The most vertices of one polygon.
pub const MAX_VERTICES: usize = 100_000;

/// A coordinate system of a region file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System {
    Image,
    Physical,
    Fk5,
    /// FK4 at B1950.
    Fk4,
    Icrs,
    Galactic,
    /// The image's own sky frame.
    Wcs,
}

impl System {
    pub fn is_sky(self) -> bool {
        !matches!(self, System::Image | System::Physical)
    }

    /// The word a region file uses.
    pub fn word(self) -> &'static str {
        match self {
            System::Image => "image",
            System::Physical => "physical",
            System::Fk5 => "fk5",
            System::Fk4 => "fk4",
            System::Icrs => "icrs",
            System::Galactic => "galactic",
            System::Wcs => "wcs",
        }
    }
}

/// A point marker's look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marker {
    Circle,
    Box,
    Diamond,
    Cross,
    X,
    Arrow,
    BoxCircle,
}

impl Marker {
    fn of(w: &str) -> Option<Marker> {
        Some(match w {
            "circle" => Marker::Circle,
            "box" => Marker::Box,
            "diamond" => Marker::Diamond,
            "cross" => Marker::Cross,
            "x" => Marker::X,
            "arrow" => Marker::Arrow,
            "boxcircle" => Marker::BoxCircle,
            _ => return None,
        })
    }

    pub fn word(self) -> &'static str {
        match self {
            Marker::Circle => "circle",
            Marker::Box => "box",
            Marker::Diamond => "diamond",
            Marker::Cross => "cross",
            Marker::X => "x",
            Marker::Arrow => "arrow",
            Marker::BoxCircle => "boxcircle",
        }
    }
}

/// A length as written: in pixels of the region's system, or an angle on the sky (degrees).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Len {
    Px(f64),
    Deg(f64),
}

/// A region's shape, in its file's terms: positions in the system's units (degrees for sky
/// systems, 1-based pixels otherwise), angles in degrees counter-clockwise from the x axis.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Circle {
        c: (f64, f64),
        r: Len,
    },
    /// One ellipse, or (several radii pairs) an elliptical annulus.
    Ellipse {
        c: (f64, f64),
        radii: Vec<(Len, Len)>,
        angle: f64,
    },
    /// One box, or (several size pairs) a box annulus. Sizes are full widths and heights.
    Box {
        c: (f64, f64),
        sizes: Vec<(Len, Len)>,
        angle: f64,
    },
    Polygon(Vec<(f64, f64)>),
    Point {
        c: (f64, f64),
        marker: Marker,
        size: f64,
    },
    Line {
        a: (f64, f64),
        b: (f64, f64),
        arrows: (bool, bool),
    },
    Vector {
        a: (f64, f64),
        len: Len,
        angle: f64,
        arrow: bool,
    },
    Text {
        c: (f64, f64),
    },
    /// Concentric circles.
    Annulus {
        c: (f64, f64),
        radii: Vec<Len>,
    },
}

impl Shape {
    /// The word a region file uses.
    pub fn word(&self) -> &'static str {
        match self {
            Shape::Circle { .. } => "circle",
            Shape::Ellipse { .. } => "ellipse",
            Shape::Box { .. } => "box",
            Shape::Polygon(_) => "polygon",
            Shape::Point { .. } => "point",
            Shape::Line { .. } => "line",
            Shape::Vector { .. } => "vector",
            Shape::Text { .. } => "text",
            Shape::Annulus { .. } => "annulus",
        }
    }
}

/// How a region is drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct Props {
    /// sRGB, 0–255.
    pub color: [u8; 3],
    pub width: f64,
    pub dash: bool,
    pub text: String,
    /// Font size in points.
    pub font_size: f64,
    /// `-` in front of the shape: an excluded region.
    pub include: bool,
    /// A background region (else a source).
    pub background: bool,
    pub tags: Vec<String>,
}

impl Default for Props {
    fn default() -> Props {
        Props {
            color: [0, 255, 0],
            width: 1.0,
            dash: false,
            text: String::new(),
            font_size: 10.0,
            include: true,
            background: false,
            tags: Vec::new(),
        }
    }
}

/// One region.
#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    pub system: System,
    pub shape: Shape,
    pub props: Props,
    /// Why it is only an approximation of what the file describes (a panda's edges).
    pub approximate: Option<Approx>,
}

/// What an approximated region left out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Approx {
    /// A panda, epanda or bpanda: its inner and outer edges, without the angle sectors.
    PandaEdges,
}

/// Why a line was not read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// A coordinate system this reader does not convert (`fk4`, `ecliptic`, …).
    System(String),
    /// A shape this reader does not draw (`ruler`, `compass`, …).
    Shape(String),
    /// The numbers do not make the shape.
    Malformed(String),
    /// More than [`MAX_REGIONS`].
    TooMany,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::System(s) => write!(f, "the {s} coordinate system is not supported"),
            Refused::Shape(s) => write!(f, "{s} regions are not supported"),
            Refused::Malformed(s) => write!(f, "a {s} region has unreadable values"),
            Refused::TooMany => write!(f, "more than {MAX_REGIONS} regions"),
        }
    }
}

/// A file read: its regions, and each line refused (1-based) with the reason.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Parsed {
    pub regions: Vec<Region>,
    pub refused: Vec<(usize, Refused)>,
}

/// Whether `head` (a file's first bytes) looks like a DS9 region file.
pub fn sniff(head: &[u8]) -> bool {
    let t = String::from_utf8_lossy(&head[..head.len().min(4096)]).to_ascii_lowercase();
    t.contains("region file format: ds9")
        || t.lines().any(|l| {
            let l = l.trim_start_matches(['+', '-', ' ']);
            [
                "circle(", "ellipse(", "box(", "polygon(", "point(", "annulus(",
            ]
            .iter()
            .any(|s| l.starts_with(s))
        })
}

// ---------------------------------------------------------------------------------------------
// Parsing.

/// Splits a line into regions at `;` outside braces and quotes.
fn split_semicolons(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut quote, mut start) = (0i32, None::<char>, 0);
    let mut comment = false;
    for (i, ch) in line.char_indices() {
        match (quote, ch) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '{') => depth += 1,
            (None, '}') => depth -= 1,
            (None, '"' | '\'') if comment || depth > 0 => quote = Some(ch),
            (None, '#') => comment = true,
            (None, ';') if depth <= 0 => {
                out.push(&line[start..i]);
                start = i + 1;
                comment = false;
            }
            _ => {}
        }
    }
    out.push(&line[start..]);
    out
}

/// The shape part and the property part (after `#`, outside braces and quotes).
fn split_comment(s: &str) -> (&str, Option<&str>) {
    let (mut depth, mut quote) = (0i32, None::<char>);
    for (i, ch) in s.char_indices() {
        match (quote, ch) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '{') => depth += 1,
            (None, '}') => depth -= 1,
            (None, '"') if depth > 0 => quote = Some(ch),
            (None, '#') if depth <= 0 => return (&s[..i], Some(&s[i + 1..])),
            _ => {}
        }
    }
    (s, None)
}

/// Tokens of a property list: words, `key=value` with braced or quoted values kept whole.
fn prop_tokens(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' | '"' | '\'' => {
                let close = if ch == '{' { '}' } else { ch };
                cur.push(ch);
                for c in chars.by_ref() {
                    cur.push(c);
                    if c == close {
                        break;
                    }
                }
            }
            c if c.is_whitespace() => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    // `key = value` with spaces: joined.
    let mut joined: Vec<String> = Vec::new();
    let mut i = 0;
    while i < out.len() {
        if out[i] == "=" && !joined.is_empty() && i + 1 < out.len() {
            let v = out[i + 1].clone();
            if let Some(last) = joined.last_mut() {
                last.push('=');
                last.push_str(&v);
            }
            i += 2;
            continue;
        }
        if out[i].ends_with('=') && i + 1 < out.len() {
            joined.push(format!("{}{}", out[i], out[i + 1]));
            i += 2;
            continue;
        }
        joined.push(out[i].clone());
        i += 1;
    }
    joined
}

/// A braced or quoted value without its delimiters.
fn unquote(v: &str) -> String {
    let v = v.trim();
    for (o, c) in [('{', '}'), ('"', '"'), ('\'', '\'')] {
        if v.len() >= 2 && v.starts_with(o) && v.ends_with(c) {
            return v[o.len_utf8()..v.len() - c.len_utf8()].to_owned();
        }
    }
    v.to_owned()
}

/// A DS9 colour: a name or `#rrggbb` (`#rgb`).
pub fn color(v: &str) -> Option<[u8; 3]> {
    let v = v.trim().to_ascii_lowercase();
    if let Some(h) = v.strip_prefix('#') {
        let hex = |s: &str| u8::from_str_radix(s, 16).ok();
        return match h.len() {
            6 => Some([hex(&h[0..2])?, hex(&h[2..4])?, hex(&h[4..6])?]),
            3 => {
                let d = |i: usize| hex(&h[i..=i]).map(|x| x * 17);
                Some([d(0)?, d(1)?, d(2)?])
            }
            _ => None,
        };
    }
    Some(match v.as_str() {
        "white" => [255, 255, 255],
        "black" => [0, 0, 0],
        "red" => [255, 0, 0],
        "green" => [0, 255, 0],
        "blue" => [0, 0, 255],
        "cyan" => [0, 255, 255],
        "magenta" => [255, 0, 255],
        "yellow" => [255, 255, 0],
        "orange" => [255, 165, 0],
        "gray" | "grey" => [190, 190, 190],
        "pink" => [255, 192, 203],
        "purple" => [160, 32, 240],
        "brown" => [165, 42, 42],
        _ => return None,
    })
}

/// The name DS9 knows for a colour, else `#rrggbb`.
fn color_word(c: [u8; 3]) -> String {
    for name in [
        "white", "black", "red", "green", "blue", "cyan", "magenta", "yellow",
    ] {
        if color(name) == Some(c) {
            return name.to_owned();
        }
    }
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// Applies a property list to `p`; the point marker, line arrows and vector arrow it names.
fn apply_props(tokens: &[String], p: &mut Props, extra: &mut Extra) {
    let mut i = 0;
    while i < tokens.len() {
        let t = &tokens[i];
        let (k, v) = t.split_once('=').map_or((t.as_str(), ""), |(k, v)| (k, v));
        let k = k.to_ascii_lowercase();
        // Numbers after a value that takes several (`point=cross 11`, `line=1 0`).
        let mut more = |n: usize| -> Vec<String> {
            let mut out = Vec::new();
            while out.len() < n
                && i + 1 < tokens.len()
                && tokens[i + 1].parse::<f64>().is_ok()
                && !tokens[i + 1].contains('=')
            {
                i += 1;
                out.push(tokens[i].clone());
            }
            out
        };
        match k.as_str() {
            "color" => {
                if let Some(c) = color(&unquote(v)) {
                    p.color = c;
                }
            }
            "width" => {
                if let Ok(w) = v.parse::<f64>()
                    && w.is_finite()
                    && w > 0.0
                {
                    p.width = w.min(64.0);
                }
            }
            "dash" => p.dash = v.trim() == "1",
            "text" => p.text = unquote(v),
            "font" => {
                let f = unquote(v);
                if let Some(s) = f.split_whitespace().nth(1).and_then(|s| s.parse().ok()) {
                    p.font_size = s;
                }
            }
            "include" => p.include = v.trim() != "0",
            "source" => p.background = v.trim() == "0",
            "background" => p.background = v.trim() != "0",
            "tag" => p.tags.push(unquote(v)),
            "point" => {
                extra.marker = Marker::of(&v.to_ascii_lowercase());
                if let Some(s) = more(1).first().and_then(|s| s.parse().ok()) {
                    extra.size = Some(s);
                }
            }
            "line" => {
                let mut nums = vec![v.to_owned()];
                nums.extend(more(1));
                let flag = |k: usize| nums.get(k).is_some_and(|s| s.trim() == "1");
                extra.arrows = Some((flag(0), flag(1)));
            }
            "vector" => extra.vector_arrow = Some(v.trim() == "1"),
            "dashlist" | "ruler" | "compass" | "textangle" | "textrotate" => {
                more(3);
            }
            _ => {}
        }
        i += 1;
    }
}

#[derive(Default)]
struct Extra {
    marker: Option<Marker>,
    size: Option<f64>,
    arrows: Option<(bool, bool)>,
    vector_arrow: Option<bool>,
}

/// The arguments of `name(args)` or `name args`: values split at commas and spaces, braced
/// or quoted text kept whole.
fn args_of(rest: &str) -> Vec<String> {
    let r = rest.trim();
    let inner = if let Some(s) = r.strip_prefix('(') {
        s.rfind(')').map_or(s, |e| &s[..e])
    } else {
        r
    };
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '{' | '"' | '\'' if ch == '{' || cur.is_empty() => {
                let close = if ch == '{' { '}' } else { ch };
                let mut s = String::new();
                for c in chars.by_ref() {
                    if c == close {
                        break;
                    }
                    s.push(c);
                }
                out.push(format!("{{{s}}}"));
            }
            ',' | ' ' | '\t' => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// A position value: degrees (sexagesimal hours for the first value of an equatorial
/// system) for sky systems, pixels otherwise.
fn coord(s: &str, sys: System, first: bool) -> Option<f64> {
    if !sys.is_sky() {
        let v: f64 = s.trim_end_matches(['i', 'p']).parse().ok()?;
        return v.is_finite().then_some(v);
    }
    let s = s.trim_end_matches('d');
    let hours = first && sys != System::Galactic && s.contains(':');
    let v = sexagesimal(s, hours)?;
    v.is_finite().then_some(v)
}

/// A length: `"` arcseconds, `'` arcminutes, `d` degrees, `i`/`p` pixels; bare numbers are
/// degrees in sky systems and pixels otherwise.
fn length(s: &str, sys: System) -> Option<Len> {
    let (num, unit) = match s.char_indices().last() {
        Some((i, c @ ('"' | '\'' | 'd' | 'i' | 'p' | 'r'))) => (&s[..i], Some(c)),
        _ => (s, None),
    };
    let v: f64 = num.parse().ok()?;
    if !v.is_finite() {
        return None;
    }
    Some(match unit {
        Some('"') => Len::Deg(v / 3600.0),
        Some('\'') => Len::Deg(v / 60.0),
        Some('d') => Len::Deg(v),
        Some('r') => Len::Deg(v.to_degrees()),
        Some(_) => Len::Px(v),
        None if sys.is_sky() => Len::Deg(v),
        None => Len::Px(v),
    })
}

fn number(s: &str) -> Option<f64> {
    s.trim_end_matches('d')
        .parse()
        .ok()
        .filter(|v: &f64| v.is_finite())
}

/// Reads a region file.
pub fn parse(text: &str) -> Parsed {
    let mut out = Parsed::default();
    let mut sys = System::Physical;
    // A system this reader does not convert, in force until the next system line.
    let mut skip: Option<String> = None;
    let mut global = Props::default();
    let mut global_extra = Extra::default();
    for (n, raw) in text.lines().enumerate() {
        let line_no = n + 1;
        for part in split_semicolons(raw) {
            let mut s = part.trim();
            if s.is_empty() {
                continue;
            }
            // DS9 writes text, vector, ruler, compass, projection and composite regions as
            // comments; anything else after `#` is a comment.
            if let Some(c) = s.strip_prefix('#') {
                let c = c.trim_start();
                let word: String = c
                    .chars()
                    .take_while(|ch| ch.is_ascii_alphabetic())
                    .collect::<String>()
                    .to_ascii_lowercase();
                if matches!(
                    word.as_str(),
                    "text"
                        | "vector"
                        | "ruler"
                        | "compass"
                        | "projection"
                        | "composite"
                        | "segment"
                ) && c[word.len()..].trim_start().starts_with('(')
                {
                    s = c;
                } else {
                    continue;
                }
            }
            let (body, comment) = split_comment(s);
            let mut body = body.trim();
            let mut props = global.clone();
            let mut extra = Extra {
                marker: global_extra.marker,
                size: global_extra.size,
                arrows: global_extra.arrows,
                vector_arrow: global_extra.vector_arrow,
            };
            if let Some(b) = body.strip_prefix('-') {
                props.include = false;
                body = b.trim_start();
            } else if let Some(b) = body.strip_prefix('+') {
                body = b.trim_start();
            }
            let word: String = body
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
                .to_ascii_lowercase();
            let rest = &body[word.len()..];
            if word == "global" {
                apply_props(&prop_tokens(rest), &mut global, &mut global_extra);
                continue;
            }
            let next = match word.as_str() {
                "image" => Some(System::Image),
                "physical" => Some(System::Physical),
                "fk5" | "j2000" => Some(System::Fk5),
                "fk4" | "b1950" => Some(System::Fk4),
                "icrs" => Some(System::Icrs),
                "galactic" => Some(System::Galactic),
                "wcs" => Some(System::Wcs),
                _ => None,
            };
            // A system alone on its part, or before a shape (`fk5;circle(…)` or
            // `fk5 circle(…)`).
            let mut word = word;
            let mut rest = rest;
            if let Some(nsys) = next {
                sys = nsys;
                skip = None;
                let r = rest.trim_start();
                if r.is_empty() {
                    continue;
                }
                let w: String = r
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric())
                    .collect::<String>()
                    .to_ascii_lowercase();
                rest = &r[w.len()..];
                word = w;
            }
            if matches!(
                word.as_str(),
                "ecliptic" | "linear" | "detector" | "amplifier"
            ) || (word.len() == 4 && word.starts_with("wcs"))
            {
                // Shapes until the next system line are refused with it.
                skip = Some(word.clone());
                let r = rest.trim_start();
                if !r.is_empty() && !r.starts_with('#') {
                    out.refused.push((line_no, Refused::System(word)));
                }
                continue;
            }
            if next.is_some() {
                skip = None;
            }
            if word.is_empty() {
                continue;
            }
            if let Some(name) = &skip {
                out.refused.push((line_no, Refused::System(name.clone())));
                continue;
            }
            if let Some(c) = comment {
                apply_props(&prop_tokens(c), &mut props, &mut extra);
            }
            if out.regions.len() >= MAX_REGIONS {
                out.refused.push((line_no, Refused::TooMany));
                return out;
            }
            // `circle point x y` and kin: an old way of writing a point.
            let mut rest = rest.trim_start();
            if let Some(m) = Marker::of(&word)
                && rest.to_ascii_lowercase().starts_with("point")
            {
                extra.marker = Some(m);
                rest = &rest[5..];
                word = "point".into();
            }
            match shape(&word, &args_of(rest), sys, &mut props, &extra) {
                Ok((shape, approximate)) => out.regions.push(Region {
                    system: sys,
                    shape,
                    props,
                    approximate,
                }),
                Err(r) => out.refused.push((line_no, r)),
            }
        }
    }
    out
}

/// The shape of `word(args)`.
fn shape(
    word: &str,
    a: &[String],
    sys: System,
    props: &mut Props,
    extra: &Extra,
) -> Result<(Shape, Option<Approx>), Refused> {
    let bad = || Refused::Malformed(word.to_owned());
    let at = |i: usize| -> Result<(f64, f64), Refused> {
        Ok((
            coord(a.get(i).ok_or_else(bad)?, sys, true).ok_or_else(bad)?,
            coord(a.get(i + 1).ok_or_else(bad)?, sys, false).ok_or_else(bad)?,
        ))
    };
    let len = |i: usize| -> Result<Len, Refused> {
        length(a.get(i).ok_or_else(bad)?, sys).ok_or_else(bad)
    };
    let num =
        |i: usize| -> Result<f64, Refused> { number(a.get(i).ok_or_else(bad)?).ok_or_else(bad) };
    // Trailing text in braces (`text x y {Hello}`).
    let braced = a.iter().find(|s| s.starts_with('{')).map(|s| unquote(s));
    let a: Vec<String> = a.iter().filter(|s| !s.starts_with('{')).cloned().collect();
    let a = &a[..];
    let at = |i| at(i);
    let shape = match word {
        "circle" if a.len() >= 3 => Shape::Circle {
            c: at(0)?,
            r: len(2)?,
        },
        "ellipse" if a.len() >= 5 && !a.len().is_multiple_of(2) => {
            let n = (a.len() - 3) / 2;
            let radii = (0..n)
                .map(|k| Ok((len(2 + 2 * k)?, len(3 + 2 * k)?)))
                .collect::<Result<Vec<_>, Refused>>()?;
            Shape::Ellipse {
                c: at(0)?,
                radii,
                angle: num(a.len() - 1)?,
            }
        }
        // An ellipse without its angle.
        "ellipse" if a.len() == 4 => Shape::Ellipse {
            c: at(0)?,
            radii: vec![(len(2)?, len(3)?)],
            angle: 0.0,
        },
        "box" if a.len() >= 5 && !a.len().is_multiple_of(2) => {
            let n = (a.len() - 3) / 2;
            let sizes = (0..n)
                .map(|k| Ok((len(2 + 2 * k)?, len(3 + 2 * k)?)))
                .collect::<Result<Vec<_>, Refused>>()?;
            Shape::Box {
                c: at(0)?,
                sizes,
                angle: num(a.len() - 1)?,
            }
        }
        "box" if a.len() == 4 => Shape::Box {
            c: at(0)?,
            sizes: vec![(len(2)?, len(3)?)],
            angle: 0.0,
        },
        "polygon" if a.len() >= 6 && a.len().is_multiple_of(2) => {
            if a.len() / 2 > MAX_VERTICES {
                return Err(bad());
            }
            Shape::Polygon(
                (0..a.len() / 2)
                    .map(|k| at(2 * k))
                    .collect::<Result<_, _>>()?,
            )
        }
        "point" if a.len() >= 2 => Shape::Point {
            c: at(0)?,
            marker: extra.marker.unwrap_or(Marker::BoxCircle),
            size: extra.size.unwrap_or(11.0),
        },
        "line" if a.len() >= 4 => Shape::Line {
            a: at(0)?,
            b: at(2)?,
            arrows: extra.arrows.unwrap_or((false, false)),
        },
        "vector" if a.len() >= 4 => Shape::Vector {
            a: at(0)?,
            len: len(2)?,
            angle: num(3)?,
            arrow: extra.vector_arrow.unwrap_or(true),
        },
        "text" if a.len() >= 2 => {
            if let Some(t) = braced {
                props.text = t;
            }
            Shape::Text { c: at(0)? }
        }
        "annulus" if a.len() >= 4 => {
            // `annulus x y inner outer n=k`: k rings between.
            let rings = a
                .iter()
                .find_map(|s| s.strip_prefix("n=")?.parse::<usize>().ok());
            let plain: Vec<usize> = (2..a.len()).filter(|&i| !a[i].contains('=')).collect();
            let radii = match rings {
                Some(k) if plain.len() == 2 && (1..=1000).contains(&k) => {
                    let (lo, hi) = (len(plain[0])?, len(plain[1])?);
                    (0..=k)
                        .map(|j| lerp(lo, hi, j as f64 / k as f64))
                        .collect::<Option<Vec<_>>>()
                        .ok_or_else(bad)?
                }
                _ => plain.iter().map(|&i| len(i)).collect::<Result<_, _>>()?,
            };
            Shape::Annulus { c: at(0)?, radii }
        }
        // panda(x, y, start, stop, n, inner, outer, nr)
        "panda" if a.len() >= 8 => {
            return Ok((
                Shape::Annulus {
                    c: at(0)?,
                    radii: vec![len(5)?, len(6)?],
                },
                Some(Approx::PandaEdges),
            ));
        }
        // epanda(x, y, start, stop, n, inner a, inner b, outer a, outer b, nr, angle)
        "epanda" | "bpanda" if a.len() >= 11 => {
            let pairs = vec![(len(5)?, len(6)?), (len(7)?, len(8)?)];
            let (c, angle) = (at(0)?, num(10)?);
            let shape = if word == "epanda" {
                Shape::Ellipse {
                    c,
                    radii: pairs,
                    angle,
                }
            } else {
                Shape::Box {
                    c,
                    sizes: pairs,
                    angle,
                }
            };
            return Ok((shape, Some(Approx::PandaEdges)));
        }
        "circle" | "ellipse" | "box" | "polygon" | "point" | "line" | "vector" | "text"
        | "annulus" | "panda" | "epanda" | "bpanda" => return Err(bad()),
        other => return Err(Refused::Shape(other.to_owned())),
    };
    Ok((shape, None))
}

fn lerp(a: Len, b: Len, f: f64) -> Option<Len> {
    match (a, b) {
        (Len::Px(x), Len::Px(y)) => Some(Len::Px(x + (y - x) * f)),
        (Len::Deg(x), Len::Deg(y)) => Some(Len::Deg(x + (y - x) * f)),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Placing.

/// The image regions are placed on.
#[derive(Clone, Copy, Debug)]
pub struct Target<'a> {
    /// Image height in pixels (the display frame is top-down).
    pub height: f64,
    /// The image's WCS and the 0-based positions of its further axes.
    pub wcs: Option<(&'a Wcs, &'a [usize])>,
    /// `LTM1_1`, `LTM2_2` and `LTV1`, `LTV2`: physical = LTM · image + LTV.
    pub physical: Option<([f64; 2], [f64; 2])>,
}

/// The image-to-physical transform of a header (IRAF's `LTM1_1`, `LTM2_2`, `LTV1`, `LTV2`,
/// as DS9 reads them for its `physical` system): `None` when it has none of them (physical
/// is then image). A missing `LTM` is 1, a missing `LTV` 0. The off-diagonal `LTM1_2` and
/// `LTM2_1` (a rotated physical frame) are not applied; DS9 files rarely need them.
pub fn physical_of(h: &specular_fits::Header) -> Option<([f64; 2], [f64; 2])> {
    let keys = ["LTM1_1", "LTM2_2", "LTV1", "LTV2"];
    if keys.iter().all(|k| h.f64(k).is_none()) {
        return None;
    }
    Some((
        [
            h.f64("LTM1_1").unwrap_or(1.0),
            h.f64("LTM2_2").unwrap_or(1.0),
        ],
        [h.f64("LTV1").unwrap_or(0.0), h.f64("LTV2").unwrap_or(0.0)],
    ))
}

/// A shape placed on the image, in display pixels. Angles are counter-clockwise on screen,
/// in degrees from the x axis.
#[derive(Clone, Debug, PartialEq)]
pub enum Placed {
    Ellipse {
        c: (f64, f64),
        r: (f64, f64),
        angle: f64,
    },
    /// Full width and height.
    Rect {
        c: (f64, f64),
        size: (f64, f64),
        angle: f64,
    },
    Polygon(Vec<(f64, f64)>),
    Line {
        a: (f64, f64),
        b: (f64, f64),
        arrows: (bool, bool),
    },
    Point {
        c: (f64, f64),
        marker: Marker,
        size: f64,
    },
    Text {
        c: (f64, f64),
    },
}

impl Placed {
    /// The outline (display px): an ellipse as `n` points, a rectangle's corners, a line's
    /// two ends, a point or a text's position alone.
    pub fn outline(&self, n: usize) -> Vec<(f64, f64)> {
        match self {
            Placed::Ellipse { c, r, angle } => {
                let (s, co) = (-angle.to_radians()).sin_cos();
                (0..n.max(8))
                    .map(|k| {
                        let t = 2.0 * PI * k as f64 / n.max(8) as f64;
                        let (x, y) = (r.0 * t.cos(), r.1 * t.sin());
                        (c.0 + x * co - y * s, c.1 + x * s + y * co)
                    })
                    .collect()
            }
            Placed::Rect { c, size, angle } => {
                let (s, co) = (-angle.to_radians()).sin_cos();
                let (hw, hh) = (size.0 / 2.0, size.1 / 2.0);
                [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)]
                    .iter()
                    .map(|&(x, y)| (c.0 + x * co - y * s, c.1 + x * s + y * co))
                    .collect()
            }
            Placed::Polygon(p) => p.clone(),
            Placed::Line { a, b, .. } => vec![*a, *b],
            Placed::Point { c, .. } | Placed::Text { c } => vec![*c],
        }
    }

    /// Whether the outline closes.
    pub fn closed(&self) -> bool {
        matches!(
            self,
            Placed::Ellipse { .. } | Placed::Rect { .. } | Placed::Polygon(_)
        )
    }
}

impl System {
    /// The celestial frame of a sky system (`wcs`: the image's, `image`'s own).
    fn sky(self, image: Option<Sky>) -> Option<Sky> {
        match self {
            System::Fk5 => Some(Sky::Fk5),
            System::Fk4 => Some(Sky::Fk4),
            System::Icrs => Some(Sky::Icrs),
            System::Galactic => Some(Sky::Galactic),
            System::Wcs => image,
            System::Image | System::Physical => None,
        }
    }

    fn of_sky(s: Sky) -> System {
        match s {
            Sky::Fk5 => System::Fk5,
            Sky::Fk4 => System::Fk4,
            Sky::Icrs => System::Icrs,
            Sky::Galactic => System::Galactic,
        }
    }
}

impl Target<'_> {
    /// FITS image pixel (1-based) to display.
    fn display(&self, x: f64, y: f64) -> (f64, f64) {
        (x - 0.5, self.height + 0.5 - y)
    }

    /// Display to FITS image pixel.
    fn fits(&self, p: (f64, f64)) -> (f64, f64) {
        (p.0 + 0.5, self.height + 0.5 - p.1)
    }

    /// Sky position in `sys` → the image's own frame (degrees). A `wcs` position is taken
    /// as it is, whatever the image's frame.
    fn in_image_frame(&self, sys: System, lng: f64, lat: f64) -> Option<(f64, f64)> {
        let (w, _) = self.wcs?;
        if sys == System::Wcs {
            return w.celestial.as_ref().map(|_| (lng, lat));
        }
        let image = frames::of_wcs(w)?;
        Some(frames::convert(sys.sky(Some(image))?, image, lng, lat))
    }

    /// A position of `sys` in display pixels.
    pub fn point(&self, sys: System, p: (f64, f64)) -> Option<(f64, f64)> {
        match sys {
            System::Image => Some(self.display(p.0, p.1)),
            System::Physical => {
                let (m, v) = self.physical.unwrap_or(([1.0, 1.0], [0.0, 0.0]));
                if m[0] == 0.0 || m[1] == 0.0 {
                    return None;
                }
                Some(self.display((p.0 - v[0]) / m[0], (p.1 - v[1]) / m[1]))
            }
            _ => {
                let (w, rest) = self.wcs?;
                let (a, b) = self.in_image_frame(sys, p.0, p.1)?;
                let d = w.display_of(a, b, rest)?;
                // The far side of the sky can project back onto the image.
                let back = w.sky_at(d.0, d.1, rest)?;
                (separation(a, b, back.0, back.1) < 1e-4).then_some(d)
            }
        }
    }

    /// At a sky position of `sys` (degrees): display pixels per degree, and the screen angle
    /// (counter-clockwise from +x, degrees) of the region frame's north there.
    fn local(&self, sys: System, p: (f64, f64)) -> Option<(f64, f64)> {
        let c = self.point(sys, p)?;
        let (w, rest) = self.wcs?;
        // A small step north (south at the north pole) in the region's own frame.
        let step = 1.0 / 3600.0;
        let (lat2, sign) = if p.1 + step > 90.0 {
            (p.1 - step, -1.0)
        } else {
            (p.1 + step, 1.0)
        };
        let (a, b) = self.in_image_frame(sys, p.0, lat2)?;
        let n = w.display_of(a, b, rest)?;
        let (dx, dy) = ((n.0 - c.0) * sign, (n.1 - c.1) * sign);
        let px = dx.hypot(dy);
        if !(px.is_finite() && px > 0.0) {
            return None;
        }
        Some((px / step, (-dy).atan2(dx).to_degrees()))
    }

    /// A length of `sys` at `at` in display pixels.
    fn length(&self, sys: System, at: (f64, f64), l: Len) -> Option<f64> {
        match (l, sys) {
            (Len::Px(v), System::Physical) => {
                let (m, _) = self.physical.unwrap_or(([1.0, 1.0], [0.0, 0.0]));
                Some(v / m[0].abs().max(f64::MIN_POSITIVE))
            }
            (Len::Px(v), _) => Some(v),
            (Len::Deg(v), s) if s.is_sky() => Some(v * self.local(s, at)?.0),
            (Len::Deg(v), _) => {
                // An angular size on an image region: through the scale at the centre.
                let (w, rest) = self.wcs?;
                let c = self.point(sys, at)?;
                Some(v * 3600.0 / w.scale_at(c.0, c.1, rest)?)
            }
        }
    }

    /// A region angle of `sys` at `at`, on screen.
    fn angle(&self, sys: System, at: (f64, f64), a: f64) -> Option<f64> {
        if !sys.is_sky() {
            return Some(a);
        }
        Some(a + self.local(sys, at)?.1 - 90.0)
    }
}

/// Angle between two directions (degrees).
pub fn separation(a1: f64, b1: f64, a2: f64, b2: f64) -> f64 {
    let (a1, b1, a2, b2) = (
        a1.to_radians(),
        b1.to_radians(),
        a2.to_radians(),
        b2.to_radians(),
    );
    let s = ((b2 - b1) / 2.0).sin().powi(2) + b1.cos() * b2.cos() * ((a2 - a1) / 2.0).sin().powi(2);
    (2.0 * s.sqrt().clamp(0.0, 1.0).asin()).to_degrees()
}

impl Region {
    /// The shapes this region draws on `t`, in display pixels; empty when it cannot be
    /// placed (sky coordinates without a WCS, off the projection).
    pub fn place(&self, t: &Target) -> Vec<Placed> {
        self.try_place(t).unwrap_or_default()
    }

    fn try_place(&self, t: &Target) -> Option<Vec<Placed>> {
        let sys = self.system;
        let p = |q: (f64, f64)| t.point(sys, q);
        Some(match &self.shape {
            Shape::Circle { c, r } => {
                let r = t.length(sys, *c, *r)?;
                vec![Placed::Ellipse {
                    c: p(*c)?,
                    r: (r, r),
                    angle: 0.0,
                }]
            }
            Shape::Annulus { c, radii } => {
                let at = p(*c)?;
                radii
                    .iter()
                    .map(|r| {
                        let r = t.length(sys, *c, *r)?;
                        Some(Placed::Ellipse {
                            c: at,
                            r: (r, r),
                            angle: 0.0,
                        })
                    })
                    .collect::<Option<_>>()?
            }
            Shape::Ellipse { c, radii, angle } => {
                let (at, ang) = (p(*c)?, t.angle(sys, *c, *angle)?);
                radii
                    .iter()
                    .map(|(a, b)| {
                        Some(Placed::Ellipse {
                            c: at,
                            r: (t.length(sys, *c, *a)?, t.length(sys, *c, *b)?),
                            angle: ang,
                        })
                    })
                    .collect::<Option<_>>()?
            }
            Shape::Box { c, sizes, angle } => {
                let (at, ang) = (p(*c)?, t.angle(sys, *c, *angle)?);
                sizes
                    .iter()
                    .map(|(a, b)| {
                        Some(Placed::Rect {
                            c: at,
                            size: (t.length(sys, *c, *a)?, t.length(sys, *c, *b)?),
                            angle: ang,
                        })
                    })
                    .collect::<Option<_>>()?
            }
            Shape::Polygon(v) => vec![Placed::Polygon(
                v.iter().map(|q| p(*q)).collect::<Option<_>>()?,
            )],
            Shape::Point { c, marker, size } => vec![Placed::Point {
                c: p(*c)?,
                marker: *marker,
                size: *size,
            }],
            Shape::Line { a, b, arrows } => vec![Placed::Line {
                a: p(*a)?,
                b: p(*b)?,
                arrows: *arrows,
            }],
            Shape::Vector {
                a,
                len,
                angle,
                arrow,
            } => {
                let at = p(*a)?;
                let l = t.length(sys, *a, *len)?;
                let ang = t.angle(sys, *a, *angle)?.to_radians();
                vec![Placed::Line {
                    a: at,
                    b: (at.0 + l * ang.cos(), at.1 - l * ang.sin()),
                    arrows: (false, *arrow),
                }]
            }
            Shape::Text { c } => vec![Placed::Text { c: p(*c)? }],
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Writing.

/// A shape drawn on an image, to be written (display pixels, angles counter-clockwise on
/// screen in degrees).
#[derive(Clone, Debug, PartialEq)]
pub struct Drawn {
    pub shape: Placed,
    pub text: String,
    pub color: Option<[u8; 3]>,
}

/// Arcseconds, compactly.
fn arcsec(deg: f64) -> String {
    format!("{}\"", trim(deg * 3600.0, 4))
}

fn trim(v: f64, decimals: usize) -> String {
    let s = format!("{v:.decimals$}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        s
    };
    if s == "-0" { "0".into() } else { s }
}

/// Writes shapes drawn on `t` as a DS9 region file: in the image's sky frame (`icrs`, `fk5`,
/// `fk4` or `galactic`; degrees, sizes in arcseconds) when it has one, else in `image`. Shapes that fall off the projection are left out; their number comes back.
pub fn write(shapes: &[Drawn], t: &Target) -> (String, usize) {
    let sky = t
        .wcs
        .and_then(|(w, _)| frames::of_wcs(w))
        .map(System::of_sky);
    let mut out = String::new();
    out.push_str("# Region file format: DS9 version 4.1\n");
    out.push_str(
        "global color=green dashlist=8 3 width=1 font=\"helvetica 10 normal roman\" select=1 \
         highlite=1 dash=0 fixed=0 edit=1 move=1 delete=1 include=1 source=1\n",
    );
    out.push_str(sky.map_or("image", System::word));
    out.push('\n');
    let mut skipped = 0;
    for d in shapes {
        match line(d, t, sky) {
            Some(l) => {
                out.push_str(&l);
                out.push('\n');
            }
            None => skipped += 1,
        }
    }
    (out, skipped)
}

/// One region's line.
fn line(d: &Drawn, t: &Target, sky: Option<System>) -> Option<String> {
    let mut props = String::new();
    if let Some(c) = d.color {
        let _ = write!(props, " color={}", color_word(c));
    }
    let text = d.text.replace(['{', '}'], "");
    if !text.trim().is_empty() {
        let _ = write!(props, " text={{{text}}}");
    }
    let props = if props.is_empty() {
        String::new()
    } else {
        format!(" #{props}")
    };
    let Some(sys) = sky else {
        let px = |p: (f64, f64)| {
            let q = t.fits(p);
            format!("{},{}", trim(q.0, 4), trim(q.1, 4))
        };
        let body = match &d.shape {
            Placed::Ellipse { c, r, .. } if (r.0 - r.1).abs() < 1e-9 => {
                format!("circle({},{})", px(*c), trim(r.0, 4))
            }
            Placed::Ellipse { c, r, angle } => format!(
                "ellipse({},{},{},{})",
                px(*c),
                trim(r.0, 4),
                trim(r.1, 4),
                trim(*angle, 4)
            ),
            Placed::Rect { c, size, angle } => format!(
                "box({},{},{},{})",
                px(*c),
                trim(size.0, 4),
                trim(size.1, 4),
                trim(*angle, 4)
            ),
            Placed::Polygon(v) => format!(
                "polygon({})",
                v.iter().map(|q| px(*q)).collect::<Vec<_>>().join(",")
            ),
            Placed::Line { a, b, .. } => format!("line({},{})", px(*a), px(*b)),
            Placed::Point { c, marker, .. } => {
                return Some(format!(
                    "point({}) # point={}{}",
                    px(*c),
                    marker.word(),
                    props.strip_prefix(" #").unwrap_or("")
                ));
            }
            Placed::Text { c } => format!("# text({})", px(*c)),
        };
        return Some(format!("{body}{props}"));
    };
    let (w, rest) = t.wcs?;
    // Display position → sky in the image's frame.
    let at = |p: (f64, f64)| -> Option<String> {
        let (a, b) = w.sky_at(p.0, p.1, rest)?;
        Some(format!("{},{}", trim(a.rem_euclid(360.0), 8), trim(b, 8)))
    };
    let sky_of = |p: (f64, f64)| w.sky_at(p.0, p.1, rest);
    // Local scale and north at a display position.
    let local = |p: (f64, f64)| -> Option<(f64, f64)> {
        let (a, b) = sky_of(p)?;
        t.local(sys, (a, b))
    };
    let body = match &d.shape {
        Placed::Ellipse { c, r, angle } => {
            let (scale, north) = local(*c)?;
            if (r.0 - r.1).abs() < 1e-9 {
                format!("circle({},{})", at(*c)?, arcsec(r.0 / scale))
            } else {
                format!(
                    "ellipse({},{},{},{})",
                    at(*c)?,
                    arcsec(r.0 / scale),
                    arcsec(r.1 / scale),
                    trim((angle - north + 90.0).rem_euclid(360.0), 5)
                )
            }
        }
        Placed::Rect { c, size, angle } => {
            let (scale, north) = local(*c)?;
            format!(
                "box({},{},{},{})",
                at(*c)?,
                arcsec(size.0 / scale),
                arcsec(size.1 / scale),
                trim((angle - north + 90.0).rem_euclid(360.0), 5)
            )
        }
        Placed::Polygon(v) => format!(
            "polygon({})",
            v.iter()
                .map(|q| at(*q))
                .collect::<Option<Vec<_>>>()?
                .join(",")
        ),
        Placed::Line { a, b, .. } => format!("line({},{})", at(*a)?, at(*b)?),
        Placed::Point { c, marker, .. } => {
            return Some(format!(
                "point({}) # point={}{}",
                at(*c)?,
                marker.word(),
                props.strip_prefix(" #").unwrap_or("")
            ));
        }
        Placed::Text { c } => format!("# text({})", at(*c)?),
    };
    Some(format!("{body}{props}"))
}

#[cfg(test)]
mod tests;
