//! Diagrams drawn as vector pages (`specular-drawpage`), cleanly (no hand-drawn wobble):
//!
//! - draw.io / diagrams.net (`.drawio`, also `.dio`): an `mxfile` of diagrams (pages), each
//!   an mxGraph model, stored plain or compressed (base64 of raw DEFLATE of the URI-encoded
//!   XML). Vertices (rectangles, rounded, ellipses, rhombi, triangles, cylinders, hexagons,
//!   text, images, swimlanes), edges (straight or through their waypoints, orthogonal ones
//!   as elbows, arrowheads) and labels (HTML reduced to text) are drawn; other stencils are
//!   drawn as their box.
//! - Excalidraw (`.excalidraw`): JSON elements: rectangles, ellipses, diamonds, lines,
//!   arrows, free drawing, text, images (from the file's own data URLs) and frames.
//!
//! Nothing is fetched: pictures linked by URL are left out and counted.

mod drawio;
mod excalidraw;

use specular_drawpage::{Affine, BezPath, HAlign, Paragraph, Point, Run, TextBlock, VAlign};
use specular_drawpage::{Op, Page, Paint, Rgba, Stroke};

pub use drawio::{is_drawio, open_drawio};
pub use excalidraw::{is_excalidraw, open_excalidraw};

/// Why a diagram was not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Malformed(String),
    Unsupported(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Malformed(s) => write!(f, "damaged diagram: {s}"),
            Error::Unsupported(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for Error {}

/// A read diagram: its pages and what was left out.
pub struct Diagram {
    pub pages: Vec<Page>,
    /// Shapes on all pages.
    pub shapes: usize,
    /// Connectors on all pages.
    pub connectors: usize,
    /// Pictures not shown (linked by URL, or not decodable).
    pub missing_pictures: usize,
    /// Shapes drawn as their box (stencils not known here).
    pub approximated: usize,
}

/// Most elements read from one diagram.
pub(crate) const MAX_ELEMENTS: usize = 200_000;
/// Margin around the drawing, points.
pub(crate) const MARGIN: f64 = 20.0;

/// `#rgb`, `#rrggbb` (draw.io, Excalidraw), a few CSS names; `None` for none/transparent.
pub(crate) fn colour(s: &str) -> Option<Rgba> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix('#') {
        let v = |i: usize, n: usize| u8::from_str_radix(h.get(i..i + n)?, 16).ok();
        return match h.len() {
            3 => Some([v(0, 1)? * 17, v(1, 1)? * 17, v(2, 1)? * 17, 255]),
            6 => Some([v(0, 2)?, v(2, 2)?, v(4, 2)?, 255]),
            8 => Some([v(0, 2)?, v(2, 2)?, v(4, 2)?, v(6, 2)?]),
            _ => None,
        };
    }
    Some(match s.to_ascii_lowercase().as_str() {
        "black" => [0, 0, 0, 255],
        "white" => [255, 255, 255, 255],
        "red" => [255, 0, 0, 255],
        "green" => [0, 128, 0, 255],
        "blue" => [0, 0, 255, 255],
        "gray" | "grey" => [128, 128, 128, 255],
        "yellow" => [255, 255, 0, 255],
        "orange" => [255, 165, 0, 255],
        _ => return None,
    })
}

/// `c` at `opacity` (0–1).
pub(crate) fn faded(c: Rgba, opacity: f64) -> Rgba {
    [
        c[0],
        c[1],
        c[2],
        (f64::from(c[3]) * opacity.clamp(0.0, 1.0)).round() as u8,
    ]
}

/// Standard base64 (whitespace skipped).
pub(crate) fn base64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            c if c.is_ascii_whitespace() => continue,
            _ => return None,
        };
        acc = acc << 6 | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

/// The bytes of a `data:` URL (base64 or percent-encoded).
pub(crate) fn data_url(s: &str) -> Option<Vec<u8>> {
    let rest = s.strip_prefix("data:")?;
    let (head, data) = rest.split_once(',')?;
    if head.ends_with(";base64") || head.contains(";base64") {
        base64(data)
    } else {
        Some(percent_decode(data).into_bytes())
    }
}

/// `%XX` sequences decoded (as UTF-8).
pub(crate) fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && let Some(v) = s
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// HTML label text: tags dropped (line breaks kept), entities read.
pub(crate) fn html_text(s: &str) -> String {
    let mut out = String::new();
    let mut tag = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match (in_tag, c) {
            (false, '<') => {
                in_tag = true;
                tag.clear();
            }
            (true, '>') => {
                in_tag = false;
                let t = tag.trim_start_matches('/');
                let name = t
                    .split(|c: char| c.is_whitespace() || c == '/')
                    .next()
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let closing = tag.starts_with('/');
                let block = matches!(name.as_str(), "div" | "p" | "li" | "tr");
                if (name == "br" || (block && (closing || !out.is_empty()))) && !out.ends_with('\n')
                {
                    out.push('\n');
                }
            }
            (true, c) => tag.push(c),
            (false, c) => out.push(c),
        }
    }
    entities(out.trim_end_matches('\n'))
}

/// The common HTML entities and numeric references.
pub(crate) fn entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let Some(end) = tail[..tail.len().min(12)].find(';') else {
            out.push('&');
            rest = &tail[1..];
            continue;
        };
        let name = &tail[1..end];
        let c = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some('\u{a0}'),
            n if n.starts_with("#x") || n.starts_with("#X") => u32::from_str_radix(&n[2..], 16)
                .ok()
                .and_then(char::from_u32),
            n if n.starts_with('#') => n[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match c {
            Some(c) => {
                out.push(c);
                rest = &tail[end + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Text in a box: one paragraph a line, in one style.
#[allow(clippy::too_many_arguments)]
pub(crate) fn text_block(
    text: &str,
    rect: (f64, f64, f64, f64),
    family: &str,
    size: f64,
    colour: Rgba,
    bold: bool,
    italic: bool,
    align: HAlign,
    valign: VAlign,
    rotation: f64,
) -> Option<Op> {
    if text.trim().is_empty() {
        return None;
    }
    let (x, y, w, h) = rect;
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let t = Affine::translate((cx, cy))
        * Affine::rotate(rotation.to_radians())
        * Affine::translate((-w / 2.0, -h / 2.0));
    let mut b = TextBlock::new(t, w.max(1.0), h.max(1.0));
    b.valign = valign;
    for line in text.split('\n') {
        let mut r = Run::new(line, family, size.max(1.0));
        r.color = colour;
        r.bold = bold;
        r.italic = italic;
        b.paragraphs.push(Paragraph {
            runs: vec![r],
            align,
            ..Paragraph::default()
        });
    }
    Some(Op::Text(Box::new(b)))
}

/// An arrowhead at `tip` pointing away from `from`: filled, open (two strokes) or a bar.
pub(crate) fn arrowhead(
    ops: &mut Vec<Op>,
    tip: Point,
    from: Point,
    kind: &str,
    size: f64,
    paint: &Paint,
    stroke: &Stroke,
) {
    let (dx, dy) = (tip.x - from.x, tip.y - from.y);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-9 || kind == "none" || kind.is_empty() {
        return;
    }
    let (ux, uy) = (dx / len, dy / len);
    let (px, py) = (-uy, ux);
    let back = Point::new(tip.x - ux * size, tip.y - uy * size);
    let half = size * 0.45;
    let l = Point::new(back.x + px * half, back.y + py * half);
    let r = Point::new(back.x - px * half, back.y - py * half);
    let mut p = BezPath::new();
    match kind {
        "open" | "arrow" | "openThin" => {
            p.move_to(l);
            p.line_to(tip);
            p.line_to(r);
            ops.push(Op::Stroke {
                path: p,
                stroke: Stroke::new(stroke.width).with_caps(specular_drawpage::Cap::Round),
                paint: paint.clone(),
            });
        }
        "bar" | "dash" => {
            let half = size * 0.6;
            p.move_to(Point::new(tip.x + px * half, tip.y + py * half));
            p.line_to(Point::new(tip.x - px * half, tip.y - py * half));
            ops.push(Op::Stroke {
                path: p,
                stroke: Stroke::new(stroke.width),
                paint: paint.clone(),
            });
        }
        "oval" | "dot" | "circle" | "circle_outline" => {
            let c = Point::new(tip.x - ux * size * 0.35, tip.y - uy * size * 0.35);
            let e = specular_drawpage::kurbo::Circle::new(c, size * 0.35);
            let path = specular_drawpage::Shape::to_path(&e, 0.1);
            if kind == "circle_outline" {
                ops.push(Op::Stroke {
                    path,
                    stroke: Stroke::new(stroke.width),
                    paint: paint.clone(),
                });
            } else {
                ops.push(Op::Fill {
                    path,
                    even_odd: false,
                    paint: paint.clone(),
                });
            }
        }
        "diamond" | "diamondThin" => {
            let mid = Point::new(tip.x - ux * size, tip.y - uy * size);
            let far = Point::new(tip.x - ux * size * 2.0, tip.y - uy * size * 2.0);
            p.move_to(tip);
            p.line_to(Point::new(mid.x + px * half, mid.y + py * half));
            p.line_to(far);
            p.line_to(Point::new(mid.x - px * half, mid.y - py * half));
            p.close_path();
            ops.push(Op::Fill {
                path: p,
                even_odd: false,
                paint: paint.clone(),
            });
        }
        _ => {
            p.move_to(tip);
            p.line_to(l);
            p.line_to(r);
            p.close_path();
            ops.push(Op::Fill {
                path: p,
                even_odd: false,
                paint: paint.clone(),
            });
        }
    }
}

/// A page sized to `bounds` (x0, y0, x1, y1) plus the margin, its ops moved to fit.
pub(crate) fn page(
    name: String,
    bounds: (f64, f64, f64, f64),
    ops: Vec<Op>,
    background: Rgba,
) -> Page {
    let (x0, y0, x1, y1) = if bounds.0.is_finite() && bounds.2 > bounds.0 {
        bounds
    } else {
        (0.0, 0.0, 100.0, 100.0)
    };
    let shift = Affine::translate((MARGIN - x0, MARGIN - y0));
    let mut p = Page::new(name, (x1 - x0 + 2.0 * MARGIN, y1 - y0 + 2.0 * MARGIN));
    p.background = background;
    p.ops = ops.into_iter().map(|o| moved(o, shift)).collect();
    p
}

fn moved(o: Op, a: Affine) -> Op {
    match o {
        Op::Fill {
            path,
            even_odd,
            paint,
        } => Op::Fill {
            path: a * path,
            even_odd,
            paint: moved_paint(paint, a),
        },
        Op::Stroke {
            path,
            stroke,
            paint,
        } => Op::Stroke {
            path: a * path,
            stroke,
            paint: moved_paint(paint, a),
        },
        Op::Image {
            image,
            transform,
            opacity,
        } => Op::Image {
            image,
            transform: a * transform,
            opacity,
        },
        Op::Text(mut t) => {
            t.transform = a * t.transform;
            Op::Text(t)
        }
        Op::PushClip { path, even_odd } => Op::PushClip {
            path: a * path,
            even_odd,
        },
        o => o,
    }
}

fn moved_paint(p: Paint, a: Affine) -> Paint {
    match p {
        Paint::Linear { start, end, stops } => Paint::Linear {
            start: a * start,
            end: a * end,
            stops,
        },
        p => p,
    }
}

/// Grows `b` to take in the point.
pub(crate) fn grow(b: &mut (f64, f64, f64, f64), x: f64, y: f64) {
    if x.is_finite() && y.is_finite() {
        b.0 = b.0.min(x);
        b.1 = b.1.min(y);
        b.2 = b.2.max(x);
        b.3 = b.3.max(y);
    }
}

pub(crate) const EMPTY_BOUNDS: (f64, f64, f64, f64) = (
    f64::INFINITY,
    f64::INFINITY,
    f64::NEG_INFINITY,
    f64::NEG_INFINITY,
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(colour("#fff"), Some([255, 255, 255, 255]));
        assert_eq!(colour("#1e1e1e"), Some([30, 30, 30, 255]));
        assert_eq!(colour("none"), None);
        assert_eq!(base64("aGk=").unwrap(), b"hi");
        assert_eq!(percent_decode("a%20b%C3%A9"), "a bé");
        assert_eq!(html_text("<b>A</b><br>B&amp;C<div>D</div>"), "A\nB&C\nD");
    }
}
