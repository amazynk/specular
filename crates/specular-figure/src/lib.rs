//! Figures and slides from the viewer (DESIGN.md §6.5).
//!
//! A [`Figure`] is a page in output pixels: image panels ([`Picture`], their pixels exactly as
//! the viewer drew them, 16 bits per sample) and vector items ([`Item`]: the scale bar, the
//! colour bar's frame and ticks, labels as glyph outlines, measurements, inset outlines), with
//! provenance as key–value metadata. [`compose::compose`] lays one out from a [`compose::Spec`];
//! [`svg`], [`pdf`] and [`raster`] write it (vectors stay vectors in SVG and PDF, the pictures
//! are embedded at their own resolution); [`render`] reads the display path back from the GPU
//! at 16 bits; [`pptx`] writes key images as slides.
//!
//! Pictures are never resampled unless their placement asks for another size: a picture
//! placed at its own size at whole-pixel offsets is copied sample for sample by every writer.

pub mod compose;
pub mod nice;
pub mod pdf;
pub mod pptx;
pub mod raster;
pub mod render;
pub mod svg;
pub mod text;

#[cfg(test)]
mod tests;

/// A point in figure px, y down.
pub type Pt = [f32; 2];

/// One element of a vector path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum El {
    Move(Pt),
    Line(Pt),
    /// Control point, end point.
    Quad(Pt, Pt),
    /// Two control points, end point.
    Cubic(Pt, Pt, Pt),
    Close,
}

/// A path: the elements in order.
pub type Path = Vec<El>;

/// A rectangle as a closed path.
pub fn rect_path(x: f32, y: f32, w: f32, h: f32) -> Path {
    vec![
        El::Move([x, y]),
        El::Line([x + w, y]),
        El::Line([x + w, y + h]),
        El::Line([x, y + h]),
        El::Close,
    ]
}

/// The points a path passes through or is pulled by (control points included).
pub fn points(p: &[El]) -> impl Iterator<Item = Pt> + '_ {
    p.iter().flat_map(|e| {
        let v: [Option<Pt>; 3] = match *e {
            El::Move(a) | El::Line(a) => [Some(a), None, None],
            El::Quad(c, a) => [Some(c), Some(a), None],
            El::Cubic(c1, c2, a) => [Some(c1), Some(c2), Some(a)],
            El::Close => [None, None, None],
        };
        v.into_iter().flatten()
    })
}

/// The bounds of a path, `[x0, y0, x1, y1]`; `None` when it has no points.
pub fn bounds(p: &[El]) -> Option<[f32; 4]> {
    points(p).fold(None, |b, q| {
        Some(match b {
            None => [q[0], q[1], q[0], q[1]],
            Some([x0, y0, x1, y1]) => [x0.min(q[0]), y0.min(q[1]), x1.max(q[0]), y1.max(q[1])],
        })
    })
}

/// An sRGB colour with straight alpha.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

impl Rgba {
    pub const WHITE: Rgba = Rgba::rgb(255, 255, 255);
    pub const BLACK: Rgba = Rgba::rgb(0, 0, 0);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Rgba {
        Rgba { r, g, b, a: 1.0 }
    }

    pub const fn with_alpha(self, a: f32) -> Rgba {
        Rgba { a, ..self }
    }

    /// `#rrggbb`.
    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

/// How a path's outline is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    pub colour: Rgba,
    /// Figure px.
    pub width: f32,
}

/// What an item is, for readers of the file (tests, screen readers, other tools).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// A shape with no particular meaning.
    Plain,
    /// The scale bar itself (its length is the calibrated length).
    ScaleBar,
    /// Text: a label, a tick value, a title, a caption.
    Text,
    /// The colour bar's frame.
    ColourBar,
    /// A tick on the colour bar.
    Tick,
    /// A measurement or region of interest.
    Measurement,
    /// The outline of an inset's region on the main panel, and around the inset.
    InsetOutline,
}

impl Role {
    /// The id used in the files (`data-role` in SVG, the marked-content tag in PDF).
    pub fn id(self) -> &'static str {
        match self {
            Role::Plain => "plain",
            Role::ScaleBar => "scale-bar",
            Role::Text => "text",
            Role::ColourBar => "colour-bar",
            Role::Tick => "tick",
            Role::Measurement => "measurement",
            Role::InsetOutline => "inset-outline",
        }
    }
}

/// A vector item: a path, filled and/or stroked.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub path: Path,
    pub fill: Option<Rgba>,
    pub stroke: Option<Stroke>,
    pub role: Role,
    /// The text the outlines spell (text items), for screen readers and text extraction.
    pub text: Option<String>,
    /// Facts kept with the item (`length`, `unit`, `value`, …), written as attributes.
    pub data: Vec<(&'static str, String)>,
}

impl Item {
    pub fn fill(path: Path, colour: Rgba, role: Role) -> Item {
        Item {
            path,
            fill: Some(colour),
            stroke: None,
            role,
            text: None,
            data: Vec::new(),
        }
    }

    pub fn stroke(path: Path, colour: Rgba, width: f32, role: Role) -> Item {
        Item {
            path,
            fill: None,
            stroke: Some(Stroke { colour, width }),
            role,
            text: None,
            data: Vec::new(),
        }
    }

    pub fn with_data(mut self, key: &'static str, value: impl Into<String>) -> Item {
        self.data.push((key, value.into()));
        self
    }

    pub fn data(&self, key: &str) -> Option<&str> {
        self.data
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.as_str())
    }
}

/// Interleaved 16-bit RGB samples, sRGB-encoded, rows top to bottom.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rgb16 {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u16>,
}

impl Rgb16 {
    /// A canvas of one colour.
    pub fn filled(width: u32, height: u32, rgb: [u16; 3]) -> Rgb16 {
        let n = width as usize * height as usize;
        let mut data = Vec::with_capacity(n * 3);
        for _ in 0..n {
            data.extend_from_slice(&rgb);
        }
        Rgb16 {
            width,
            height,
            data,
        }
    }

    /// From 8-bit RGB (each code times 257).
    pub fn from_rgb8(width: u32, height: u32, rgb: &[u8]) -> Rgb16 {
        Rgb16 {
            width,
            height,
            data: rgb.iter().map(|&v| u16::from(v) * 257).collect(),
        }
    }

    pub fn px(&self, x: u32, y: u32) -> [u16; 3] {
        let i = (y as usize * self.width as usize + x as usize) * 3;
        [self.data[i], self.data[i + 1], self.data[i + 2]]
    }

    /// 8-bit RGB, rounded.
    pub fn to_rgb8(&self) -> Vec<u8> {
        self.data.iter().map(|&v| to8(v)).collect()
    }

    /// 16-bit samples big-endian (PNG's order).
    pub fn to_be_bytes(&self) -> Vec<u8> {
        self.data.iter().flat_map(|v| v.to_be_bytes()).collect()
    }

    /// The samples fit in 8 bits exactly (every one a multiple of 257).
    pub fn is_8bit(&self) -> bool {
        self.data.iter().all(|&v| v % 257 == 0)
    }
}

/// A 16-bit code to 8 bits, rounded.
pub fn to8(v: u16) -> u8 {
    ((u32::from(v) * 255 + 32_767) / 65_535) as u8
}

/// An image placed on the figure.
#[derive(Clone, Debug, PartialEq)]
pub struct Picture {
    /// Placement in figure px.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub px: Rgb16,
    /// Interpolate when scaled (a colour ramp); else nearest (image pixels stay squares).
    pub smooth: bool,
    /// What the panel shows (a channel's name, "inset"), for readers of the file.
    pub name: String,
}

impl Picture {
    /// Placed at its own size at whole-pixel offsets: writers copy it sample for sample.
    pub fn is_exact(&self) -> bool {
        self.w == self.px.width as f32
            && self.h == self.px.height as f32
            && self.x.fract() == 0.0
            && self.y.fract() == 0.0
    }
}

/// A figure: a white page of `width` × `height` px with pictures, then items on top.
#[derive(Clone, Debug, PartialEq)]
pub struct Figure {
    pub width: u32,
    pub height: u32,
    pub background: Rgba,
    pub pictures: Vec<Picture>,
    pub items: Vec<Item>,
    /// Provenance and display settings, in order (`file`, `calibration`, `display`, …).
    pub meta: Vec<(String, String)>,
    pub title: String,
    /// Embed pictures with 16 bits per sample (else 8).
    pub deep: bool,
}

impl Figure {
    pub fn meta(&self, key: &str) -> Option<&str> {
        self.meta
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// The items of a role.
    pub fn items_of(&self, role: Role) -> impl Iterator<Item = &Item> {
        self.items.iter().filter(move |i| i.role == role)
    }
}

/// Text for XML: `&`, `<`, `>`, quotes escaped; control characters dropped.
pub fn xml_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            '\t' | '\n' | '\r' => o.push(c),
            c if (c as u32) < 0x20 => {}
            c => o.push(c),
        }
    }
    o
}

/// Standard base64 with padding.
pub fn base64(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut o = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        o.push(A[(n >> 18) as usize & 63] as char);
        o.push(A[(n >> 12) as usize & 63] as char);
        o.push(if c.len() > 1 {
            A[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        o.push(if c.len() > 2 {
            A[n as usize & 63] as char
        } else {
            '='
        });
    }
    o
}

/// A PNG of `px`: 16 bits per sample when `deep`, else 8; `text` as `tEXt` chunks.
pub fn png(px: &Rgb16, deep: bool, text: &[(&str, &str)]) -> Result<Vec<u8>, String> {
    let opts = specular_png::EncodeOptions {
        sixteen_bit: deep,
        level: Some(6),
        srgb: true,
        text,
        ..Default::default()
    };
    let data = if deep { px.to_be_bytes() } else { px.to_rgb8() };
    specular_png::encode_with(
        px.width,
        px.height,
        specular_png::ColorType::Rgb,
        &data,
        &opts,
    )
    .map_err(|e| e.to_string())
}
