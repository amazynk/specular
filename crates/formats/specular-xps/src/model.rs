//! The parsed page: an owned tree of canvases, paths and glyph runs with resources resolved.

use std::sync::Arc;

use vello_cpu::kurbo::{Affine, Point};

use crate::font::Font;
use crate::geom::Geom;

/// Straight sRGB colour, components 0-1.
pub type Rgba = [f32; 4];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spread {
    Pad,
    Reflect,
    Repeat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileMode {
    None,
    Tile,
    FlipX,
    FlipY,
    FlipXY,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stop {
    pub offset: f32,
    pub color: Rgba,
}

/// Where a tile brush's content sits: its viewbox (content space) shown in its viewport
/// (brush space), and how it repeats.
#[derive(Clone, Debug, PartialEq)]
pub struct Tiling {
    pub viewbox: [f64; 4],
    pub viewport: [f64; 4],
    pub mode: TileMode,
}

#[derive(Clone, Debug)]
pub enum Brush {
    Solid(Rgba),
    Linear {
        start: Point,
        end: Point,
        stops: Vec<Stop>,
        spread: Spread,
        linear_rgb: bool,
        transform: Affine,
        opacity: f32,
    },
    Radial {
        center: Point,
        origin: Point,
        rx: f64,
        ry: f64,
        stops: Vec<Stop>,
        spread: Spread,
        linear_rgb: bool,
        transform: Affine,
        opacity: f32,
    },
    Image {
        /// The image part.
        part: String,
        tiling: Tiling,
        transform: Affine,
        opacity: f32,
    },
    Visual {
        visual: Arc<Node>,
        tiling: Tiling,
        transform: Affine,
        opacity: f32,
    },
}

/// What every element has.
#[derive(Clone, Debug)]
pub struct Common {
    pub transform: Affine,
    pub clip: Option<Arc<Geom>>,
    pub opacity: f32,
    pub mask: Option<Arc<Brush>>,
}

impl Default for Common {
    fn default() -> Self {
        Common {
            transform: Affine::IDENTITY,
            clip: None,
            opacity: 1.0,
            mask: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cap {
    Flat,
    Square,
    Round,
    Triangle,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Join {
    Miter,
    Bevel,
    Round,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StrokeStyle {
    pub width: f64,
    pub dash: Vec<f64>,
    pub dash_offset: f64,
    pub dash_cap: Cap,
    pub start_cap: Cap,
    pub end_cap: Cap,
    pub join: Join,
    pub miter: f64,
}

#[derive(Clone, Debug)]
pub struct PathNode {
    pub common: Common,
    pub data: Arc<Geom>,
    pub fill: Option<Arc<Brush>>,
    pub stroke: Option<(Arc<Brush>, StrokeStyle)>,
}

/// One placed glyph: its id and origin in the element's space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub gid: u32,
    pub at: Point,
}

#[derive(Clone, Debug)]
pub struct GlyphsNode {
    pub common: Common,
    pub font: Arc<Font>,
    pub em: f64,
    pub glyphs: Vec<Placed>,
    pub fill: Option<Arc<Brush>>,
    pub bold: bool,
    pub italic: bool,
    pub sideways: bool,
}

#[derive(Clone, Debug)]
pub struct CanvasNode {
    pub common: Common,
    pub children: Vec<Node>,
}

#[derive(Clone, Debug)]
pub enum Node {
    Canvas(CanvasNode),
    Path(PathNode),
    Glyphs(GlyphsNode),
}

impl Node {
    pub fn common(&self) -> &Common {
        match self {
            Node::Canvas(c) => &c.common,
            Node::Path(p) => &p.common,
            Node::Glyphs(g) => &g.common,
        }
    }
}

/// A parsed page.
#[derive(Debug)]
pub struct Page {
    pub nodes: Vec<Node>,
    pub text: String,
}

fn srgb_from_linear(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Parses an XPS colour: `#RRGGBB`, `#AARRGGBB`, `sc#[A,]R,G,B` (scRGB), or a
/// `ContextColor` (shown with a plain approximation of its channels).
pub fn color(s: &str) -> Option<Rgba> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix('#') {
        let v = u32::from_str_radix(h, 16).ok()?;
        let b = |sh: u32| ((v >> sh) & 0xFF) as f32 / 255.0;
        return match h.len() {
            6 => Some([b(16), b(8), b(0), 1.0]),
            8 => Some([b(16), b(8), b(0), b(24)]),
            3 => {
                let n = |sh: u32| ((v >> sh) & 0xF) as f32 / 15.0;
                Some([n(8), n(4), n(0), 1.0])
            }
            _ => None,
        };
    }
    if let Some(rest) = s.strip_prefix("sc#") {
        let v: Vec<f32> = rest
            .split(',')
            .map(|t| t.trim().parse::<f32>())
            .collect::<Result<_, _>>()
            .ok()?;
        let (a, rgb) = match v.as_slice() {
            [r, g, b] => (1.0, [*r, *g, *b]),
            [a, r, g, b] => (*a, [*r, *g, *b]),
            _ => return None,
        };
        return Some([
            srgb_from_linear(rgb[0]),
            srgb_from_linear(rgb[1]),
            srgb_from_linear(rgb[2]),
            a.clamp(0.0, 1.0),
        ]);
    }
    if let Some(rest) = s.strip_prefix("ContextColor") {
        // `ContextColor profile.icc a,c1,c2,…`
        let vals = rest.trim().split_once(char::is_whitespace)?.1;
        let v: Vec<f32> = vals
            .split(',')
            .filter_map(|t| t.trim().parse::<f32>().ok())
            .map(|x| x.clamp(0.0, 1.0))
            .collect();
        let (a, ch) = v.split_first()?;
        let rgb = match ch {
            [g] => [*g, *g, *g],
            [r, g, b] => [*r, *g, *b],
            [c, m, y, k, ..] => [
                (1.0 - c) * (1.0 - k),
                (1.0 - m) * (1.0 - k),
                (1.0 - y) * (1.0 - k),
            ],
            [x, ..] => [*x, *x, *x],
            [] => return None,
        };
        return Some([rgb[0], rgb[1], rgb[2], *a]);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours() {
        assert_eq!(color("#FF0000"), Some([1.0, 0.0, 0.0, 1.0]));
        assert_eq!(color("#80000000"), Some([0.0, 0.0, 0.0, 128.0 / 255.0]));
        let c = color("sc#1,0.5,0,1").unwrap();
        assert!((c[0] - 0.735).abs() < 0.01 && c[1] == 0.0 && c[3] == 1.0);
        let c = color("ContextColor /x.icc 1,0,0,0,1").unwrap();
        assert_eq!(c, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(color("red"), None);
    }
}
