//! The display list a picture is parsed into: everything in page points (y down), colours
//! resolved, ready to draw at any scale.

use std::sync::Arc;

use kurbo::{Affine, BezPath, Rect, Stroke};
use vello_cpu::Pixmap;

/// Straight RGBA8.
pub(crate) type Rgba = [u8; 4];

/// How an area is painted.
#[derive(Clone, Debug)]
pub(crate) enum Paint {
    Solid(Rgba),
    /// A pattern table tiled across the area: pattern pixels → page.
    Pattern {
        pixmap: Arc<Pixmap>,
        transform: Affine,
    },
}

/// Horizontal text alignment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum HAlign {
    Normal,
    Left,
    Centre,
    Right,
    /// A fraction of the text's width.
    Continuous(f64),
}

/// Vertical text alignment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum VAlign {
    Normal,
    Top,
    Cap,
    Half,
    Base,
    Bottom,
    /// A fraction of the cap height above the baseline.
    Continuous(f64),
}

/// One run of a text element (TEXT, then any APPEND TEXT).
#[derive(Clone, Debug)]
pub(crate) struct TextRun {
    /// Index into the document's fonts (0-based).
    pub font: usize,
    pub text: Vec<char>,
    pub colour: Rgba,
    /// Character height relative to the element's first run.
    pub height: f64,
    /// Character expansion factor (width).
    pub expansion: f64,
    /// Extra space after each character, in character heights.
    pub spacing: f64,
}

/// A text element. Text space: origin at the text's reference point, x along the base
/// vector, y along the up vector, one unit the character (cap) height.
#[derive(Clone, Debug)]
pub(crate) struct TextOp {
    /// Text space → page.
    pub transform: Affine,
    pub halign: HAlign,
    pub valign: VAlign,
    /// 0 right, 1 left, 2 up, 3 down.
    pub path: u8,
    /// RESTRICTED TEXT: the box (text units) the text is squeezed into.
    pub restrict: Option<(f64, f64)>,
    pub runs: Vec<TextRun>,
}

/// One drawing operation.
#[derive(Clone, Debug)]
pub(crate) enum Op {
    Fill {
        path: BezPath,
        even_odd: bool,
        paint: Paint,
    },
    Stroke {
        path: BezPath,
        style: Stroke,
        colour: Rgba,
    },
    /// Hatch lines (or other strokes) clipped to an area.
    ClippedStroke {
        clip: BezPath,
        even_odd: bool,
        lines: BezPath,
        width: f64,
        colour: Rgba,
    },
    /// A cell array: image pixels → page.
    Image {
        pixmap: Arc<Pixmap>,
        transform: Affine,
    },
    Text(Box<TextOp>),
    /// The clip rectangle for what follows (`None`: the whole page).
    Clip(Option<Rect>),
}

/// One picture: a page.
#[derive(Clone, Debug)]
pub(crate) struct Picture {
    pub name: String,
    /// Points.
    pub size: (f64, f64),
    pub background: Rgba,
    pub ops: Vec<Op>,
}
