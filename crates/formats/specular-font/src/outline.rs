//! Glyph outlines as path commands in font units, y up.

/// One path command.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathEl {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    /// Control point, end point.
    QuadTo(f32, f32, f32, f32),
    /// Two control points, end point.
    CubicTo(f32, f32, f32, f32, f32, f32),
    Close,
}

/// A glyph's outline and the box of its points (control points included).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outline {
    pub path: Vec<PathEl>,
    /// x min, y min, x max, y max.
    pub bbox: [f32; 4],
}

impl Outline {
    pub(crate) fn finish(path: Vec<PathEl>) -> Option<Outline> {
        if path.is_empty() {
            return None;
        }
        let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        let mut add = |x: f32, y: f32| {
            b[0] = b[0].min(x);
            b[1] = b[1].min(y);
            b[2] = b[2].max(x);
            b[3] = b[3].max(y);
        };
        for el in &path {
            match *el {
                PathEl::MoveTo(x, y) | PathEl::LineTo(x, y) => add(x, y),
                PathEl::QuadTo(a, b, x, y) => {
                    add(a, b);
                    add(x, y)
                }
                PathEl::CubicTo(a, b, c, d, x, y) => {
                    add(a, b);
                    add(c, d);
                    add(x, y)
                }
                PathEl::Close => {}
            }
        }
        Some(Outline { path, bbox: b })
    }
}

/// Builds a path, closing an open contour at each move and at the end (CFF semantics).
#[derive(Default)]
pub(crate) struct Builder {
    pub path: Vec<PathEl>,
    open: bool,
}

impl Builder {
    pub fn move_to(&mut self, x: f32, y: f32) {
        if self.open {
            self.path.push(PathEl::Close);
        }
        self.open = true;
        self.path.push(PathEl::MoveTo(x, y));
    }
    pub fn line_to(&mut self, x: f32, y: f32) {
        self.path.push(PathEl::LineTo(x, y));
    }
    pub fn cubic_to(&mut self, a: f32, b: f32, c: f32, d: f32, x: f32, y: f32) {
        self.path.push(PathEl::CubicTo(a, b, c, d, x, y));
    }
    pub fn close(&mut self) {
        if self.open {
            self.path.push(PathEl::Close);
            self.open = false;
        }
    }
}
