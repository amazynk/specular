//! Text as glyph outlines: the figure's labels are vectors in every output, shaped by the
//! viewer's own text system (bidi, fallback fonts and all), so a Hebrew caption in a PDF needs
//! no embedded font and looks as it does on screen.

use crate::{El, Path};

/// A line of text shaped at a size: its outlines with the line's top-left at (0, 0).
#[derive(Clone, Debug, PartialEq)]
pub struct Shaped {
    pub width: f32,
    pub height: f32,
    pub path: Path,
}

impl Shaped {
    /// The outlines moved by (`dx`, `dy`).
    pub fn at(&self, dx: f32, dy: f32) -> Path {
        translate(&self.path, dx, dy)
    }
}

/// A path moved by (`dx`, `dy`).
pub fn translate(p: &[El], dx: f32, dy: f32) -> Path {
    let t = |a: [f32; 2]| [a[0] + dx, a[1] + dy];
    p.iter()
        .map(|e| match *e {
            El::Move(a) => El::Move(t(a)),
            El::Line(a) => El::Line(t(a)),
            El::Quad(c, a) => El::Quad(t(c), t(a)),
            El::Cubic(c1, c2, a) => El::Cubic(t(c1), t(c2), t(a)),
            El::Close => El::Close,
        })
        .collect()
}

/// Shapes one line of text at `size` px.
pub trait Shaper {
    fn shape(&mut self, text: &str, size: f32, bold: bool) -> Shaped;
}

impl Shaper for specular_gfx::Text {
    fn shape(&mut self, text: &str, size: f32, bold: bool) -> Shaped {
        use specular_gfx::{Align, PathEl, TextStyle};
        let style = TextStyle::new(size).weight(if bold { 650.0 } else { 450.0 });
        let b = self.layout(text, style, None, Align::Start);
        let path = self
            .outlines(&b, 0.0, 0.0)
            .into_iter()
            .map(|e| match e {
                PathEl::Move(a) => El::Move(a),
                PathEl::Line(a) => El::Line(a),
                PathEl::Quad(c, a) => El::Quad(c, a),
                PathEl::Cubic(c1, c2, a) => El::Cubic(c1, c2, a),
                PathEl::Close => El::Close,
            })
            .collect();
        Shaped {
            width: b.width,
            height: b.height,
            path,
        }
    }
}

/// A stand-in shaper: each character a box 0.6 × size wide (tests that need no fonts).
#[derive(Clone, Copy, Debug, Default)]
pub struct Boxes;

impl Shaper for Boxes {
    fn shape(&mut self, text: &str, size: f32, _bold: bool) -> Shaped {
        let w = size * 0.6;
        let mut path = Vec::new();
        for (i, c) in text.chars().enumerate() {
            if c.is_whitespace() {
                continue;
            }
            let x = i as f32 * w;
            path.extend(crate::rect_path(
                x + w * 0.1,
                size * 0.25,
                w * 0.8,
                size * 0.9,
            ));
        }
        Shaped {
            width: text.chars().count() as f32 * w,
            height: size * 1.3,
            path,
        }
    }
}
