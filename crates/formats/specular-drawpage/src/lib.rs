//! Page display lists for office and design drawings, drawn with vello_cpu.
//!
//! The format readers (`specular-visio`, `specular-odf`, `specular-wpg`, `specular-sketch`)
//! parse a document once into [`Page`]s: geometry in page points (y down), colours resolved,
//! images decoded. [`render`] draws any rectangle of a page at any scale, so tiles and deep
//! zoom cost only the pixels asked for.
//!
//! Drawn: fills (non-zero and even-odd), strokes (width, caps, joins, dashes), solid, linear,
//! radial and image paints, images, clip paths, opacity groups, and text boxes: paragraphs of
//! styled runs laid out in a box with word wrap, alignment and vertical anchoring. Text goes
//! through the drawing stack's fonts (`specular-vector`): the named family when installed,
//! else a metric twin, else the bundled Inter; a font without italic is slanted.
//!
//! Bounds: the readers cap their own lists ([`MAX_OPS`] is the shared limit); a render is at
//! most [`MAX_RENDER_SIDE`] wide and high and [`MAX_RENDER_PIXELS`] in all. Layers nest at
//! most [`MAX_LAYERS`] deep; deeper pushes are ignored with their pops.

mod render;
mod text;
pub mod xml;

use std::sync::Arc;

pub use kurbo;
pub use kurbo::{Affine, BezPath, Cap, Join, Point, Rect, Shape, Stroke};
pub use text::{HAlign, Paragraph, Run, TextBlock, VAlign, face};

/// The largest side of a single render (vello_cpu's pixmaps are `u16` wide).
pub const MAX_RENDER_SIDE: u32 = u16::MAX as u32;
/// The largest single render, in pixels (1 GiB of RGBA).
pub const MAX_RENDER_PIXELS: u64 = 256 * 1024 * 1024;
/// The most display-list entries a reader should produce per document.
pub const MAX_OPS: usize = 1 << 21;
/// Deepest clip/opacity nesting drawn.
pub const MAX_LAYERS: usize = 256;
/// Most image pixels a reader should decode per document (all images together).
pub const MAX_IMAGE_PIXELS: u64 = 1 << 28;

/// Straight (not premultiplied) RGBA8.
pub type Rgba = [u8; 4];

/// Decodes an embedded image (PNG, JPEG, BMP, GIF, TIFF…) to straight RGBA8:
/// `(width, height, pixels)`. Supplied by the caller, so the readers carry no codecs.
pub type ImageDecoder = Arc<dyn Fn(&[u8]) -> Option<(u32, u32, Vec<u8>)> + Send + Sync>;

/// Why a render failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RenderError {
    /// The caller cancelled the render.
    #[error("cancelled")]
    Cancelled,
    /// The requested raster is empty or larger than the render bounds.
    #[error("page raster {0}×{1} is outside the render bounds")]
    TooLarge(u32, u32),
    /// No such page.
    #[error("page {0} does not exist")]
    NoSuchPage(usize),
    /// The renderer could not draw the page.
    #[error("the page could not be drawn: {0}")]
    Failed(String),
}

/// The rectangle of a page to render, in output pixels, and the page's scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderRequest {
    /// Output pixels per point, horizontally.
    pub scale_x: f64,
    /// Output pixels per point, vertically.
    pub scale_y: f64,
    /// Left edge of the region in the scaled page raster.
    pub x: u32,
    /// Top edge of the region in the scaled page raster.
    pub y: u32,
    /// Region width in pixels.
    pub width: u32,
    /// Region height in pixels.
    pub height: u32,
}

/// A decoded image, ready to paint.
pub struct Image {
    /// Pixel size as stored (the pixmap may be halved to fit `u16` sides).
    pub width: u32,
    pub height: u32,
    pub(crate) pixmap: Arc<vello_cpu::Pixmap>,
}

impl std::fmt::Debug for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Image({}×{})", self.width, self.height)
    }
}

impl Image {
    /// From straight RGBA8 rows, top-down; reduced until each side fits a pixmap.
    pub fn from_rgba(width: u32, height: u32, data: Vec<u8>) -> Option<Image> {
        let (mut w, mut h, mut data) = (width, height, data);
        if w == 0 || h == 0 || data.len() as u64 != u64::from(w) * u64::from(h) * 4 {
            return None;
        }
        while w > u32::from(u16::MAX) || h > u32::from(u16::MAX) {
            (data, w, h) = halve(&data, w, h);
        }
        let opaque = data.chunks_exact(4).all(|p| p[3] == 255);
        let pix = vello_cpu::Pixmap::from_parts(
            data,
            w as u16,
            h as u16,
            vello_cpu::PixelMetadata::new(vello_cpu::peniko::ImageAlphaType::Alpha, !opaque),
        );
        Some(Image {
            width,
            height,
            pixmap: Arc::new(pix),
        })
    }

    /// The straight RGBA8 pixels of a decoded image, through `decode`.
    pub fn decode(decode: &ImageDecoder, bytes: &[u8]) -> Option<Image> {
        let (w, h, px) = decode(bytes)?;
        Image::from_rgba(w, h, px)
    }

    /// Pixmap pixels → stored pixels (1 unless halved).
    fn pixmap_scale(&self) -> (f64, f64) {
        (
            f64::from(self.width) / f64::from(self.pixmap.width()),
            f64::from(self.height) / f64::from(self.pixmap.height()),
        )
    }
}

fn halve(data: &[u8], w: u32, h: u32) -> (Vec<u8>, u32, u32) {
    let (w2, h2) = (w.div_ceil(2), h.div_ceil(2));
    let mut out = vec![0u8; w2 as usize * h2 as usize * 4];
    let (w, h) = (w as usize, h as usize);
    for y in 0..h2 as usize {
        let y0 = y * 2;
        let y1 = (y0 + 1).min(h - 1);
        for x in 0..w2 as usize {
            let x0 = x * 2;
            let x1 = (x0 + 1).min(w - 1);
            for c in 0..4 {
                let s = u32::from(data[(y0 * w + x0) * 4 + c])
                    + u32::from(data[(y0 * w + x1) * 4 + c])
                    + u32::from(data[(y1 * w + x0) * 4 + c])
                    + u32::from(data[(y1 * w + x1) * 4 + c]);
                out[(y * w2 as usize + x) * 4 + c] = ((s + 2) / 4) as u8;
            }
        }
    }
    (out, w2, h2)
}

/// A gradient stop: offset 0–1 and colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stop {
    pub offset: f32,
    pub color: Rgba,
}

/// How an area or a line is painted.
#[derive(Debug, Clone)]
pub enum Paint {
    Solid(Rgba),
    /// A linear gradient from `start` (offset 0) to `end` (offset 1), page points; padded.
    Linear {
        start: Point,
        end: Point,
        stops: Vec<Stop>,
    },
    /// A radial gradient: offset 0 at the centre of the unit circle, 1 on it; `transform`
    /// maps the unit circle to the page (an ellipse when non-uniform).
    Radial {
        transform: Affine,
        stops: Vec<Stop>,
    },
    /// An image, tiled when `repeat`, else padded; `transform` maps image pixels to the page.
    Image {
        image: Arc<Image>,
        transform: Affine,
        repeat: bool,
    },
}

impl Paint {
    /// Black.
    pub const BLACK: Paint = Paint::Solid([0, 0, 0, 255]);
}

/// One drawing operation; everything in page points.
#[derive(Debug, Clone)]
pub enum Op {
    Fill {
        path: BezPath,
        even_odd: bool,
        paint: Paint,
    },
    Stroke {
        path: BezPath,
        stroke: Stroke,
        paint: Paint,
    },
    /// An image: its pixel rectangle `(0, 0)–(width, height)` mapped by `transform`.
    Image {
        image: Arc<Image>,
        transform: Affine,
        opacity: f32,
    },
    Text(Box<TextBlock>),
    /// Clips what follows to `path` until the matching [`Op::Pop`].
    PushClip {
        path: BezPath,
        even_odd: bool,
    },
    /// Composites what follows at `opacity` until the matching [`Op::Pop`].
    PushOpacity(f32),
    Pop,
}

impl Op {
    /// The path elements this operation holds (0 for images, text and layers).
    pub fn path_elements(&self) -> usize {
        match self {
            Op::Fill { path, .. } | Op::Stroke { path, .. } | Op::PushClip { path, .. } => {
                path.elements().len()
            }
            _ => 0,
        }
    }
}

/// Most path elements held by one document's operations: [`MAX_OPS`] alone does not bound
/// memory when one huge path is placed many times (symbols, masters).
pub const MAX_PATH_ELEMENTS: usize = 1 << 23;

/// One page: its size in points, background and operations.
#[derive(Debug, Clone)]
pub struct Page {
    pub name: String,
    /// Width and height in points.
    pub size: (f64, f64),
    /// Painted under everything (opaque white for paper).
    pub background: Rgba,
    pub ops: Vec<Op>,
}

impl Page {
    pub fn new(name: impl Into<String>, size: (f64, f64)) -> Page {
        Page {
            name: name.into(),
            size,
            background: [255, 255, 255, 255],
            ops: Vec::new(),
        }
    }
}

/// Draws `rect` of `page` at `req`'s scale into straight RGBA8 (opaque: composited on white
/// under the page background).
pub fn render(
    page: &Page,
    req: RenderRequest,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, RenderError> {
    let (w, h) = (req.width, req.height);
    if w == 0
        || h == 0
        || w > MAX_RENDER_SIDE
        || h > MAX_RENDER_SIDE
        || u64::from(w) * u64::from(h) > MAX_RENDER_PIXELS
        || !(req.scale_x.is_finite() && req.scale_x > 0.0)
        || !(req.scale_y.is_finite() && req.scale_y > 0.0)
    {
        return Err(RenderError::TooLarge(w, h));
    }
    if cancelled() {
        return Err(RenderError::Cancelled);
    }
    render::render(
        page,
        (req.scale_x, req.scale_y),
        (req.x, req.y),
        (w as u16, h as u16),
        cancelled,
    )
    .ok_or_else(|| {
        if cancelled() {
            RenderError::Cancelled
        } else {
            RenderError::Failed("out of memory".into())
        }
    })
}

/// The bounding box of a page's drawn content (fills, strokes widened, images, text boxes),
/// page points; `None` for an empty page. For formats whose page is the drawing's extent.
pub fn content_bounds(ops: &[Op]) -> Option<Rect> {
    let mut r: Option<Rect> = None;
    let mut add = |b: Rect| {
        if b.x0.is_finite() && b.y0.is_finite() && b.x1.is_finite() && b.y1.is_finite() {
            r = Some(r.map_or(b, |r| r.union(b)));
        }
    };
    for op in ops {
        match op {
            Op::Fill { path, .. } if !path.elements().is_empty() => add(path.bounding_box()),
            Op::Stroke { path, stroke, .. } if !path.elements().is_empty() => add(path
                .bounding_box()
                .inflate(stroke.width / 2.0, stroke.width / 2.0)),
            Op::Image {
                image, transform, ..
            } => add(transform.transform_rect_bbox(Rect::new(
                0.0,
                0.0,
                f64::from(image.width),
                f64::from(image.height),
            ))),
            Op::Text(t) => add(t
                .transform
                .transform_rect_bbox(Rect::new(0.0, 0.0, t.width, t.height))),
            _ => {}
        }
    }
    r
}

/// Moves every operation by `offset` points (to put a drawing's extent at the page origin).
pub fn translate_ops(ops: &mut [Op], offset: kurbo::Vec2) {
    let t = Affine::translate(offset);
    for op in ops {
        match op {
            Op::Fill { path, paint, .. } | Op::Stroke { path, paint, .. } => {
                path.apply_affine(t);
                translate_paint(paint, t);
            }
            Op::PushClip { path, .. } => path.apply_affine(t),
            Op::Image { transform, .. } => *transform = t * *transform,
            Op::Text(b) => b.transform = t * b.transform,
            Op::PushOpacity(_) | Op::Pop => {}
        }
    }
}

fn translate_paint(p: &mut Paint, t: Affine) {
    match p {
        Paint::Solid(_) => {}
        Paint::Linear { start, end, .. } => {
            *start = t * *start;
            *end = t * *end;
        }
        Paint::Radial { transform, .. } | Paint::Image { transform, .. } => {
            *transform = t * *transform
        }
    }
}

/// Parses a CSS-style `#rrggbb` / `#rgb` / `#rrggbbaa` colour.
pub fn hex_color(s: &str) -> Option<Rgba> {
    let h = s.trim().strip_prefix('#')?;
    let v = |i: usize, n: usize| u8::from_str_radix(h.get(i..i + n)?, 16).ok();
    match h.len() {
        3 => Some([v(0, 1)? * 17, v(1, 1)? * 17, v(2, 1)? * 17, 255]),
        6 => Some([v(0, 2)?, v(2, 2)?, v(4, 2)?, 255]),
        8 => Some([v(0, 2)?, v(2, 2)?, v(4, 2)?, v(6, 2)?]),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
