//! Borders and watermark: added to exports and prints only, never to the edit's
//! pixels, so they are not a graph node ([`crate::model::Stage::Output`]). Develop shows
//! them on the picture only while asked to (a view toggle), from the same [`Layout`] and
//! watermark raster.
//!
//! - **Border**: `border` as a fraction of the long side or in pixels, in `colour` (sRGB),
//!   on every side; with `aspect` > 0 the canvas then grows on two sides to that width/height
//!   ratio (a square for 1), the photo centred.
//! - **Watermark**: an SVG file, or the built-in text template, with variables replaced
//!   (`$(title)`, `$(date)`, `$(filename)`, `$(exif.camera)`, …, see [`Vars`]), drawn by the
//!   viewer's SVG renderer at its final size, placed at one of nine anchors inside the photo
//!   with a margin, `scale` of the photo's width, at `opacity`. Blended in linear light of the
//!   export space.

use std::path::Path;

use super::get;
use crate::export::render::{Exported, Samples};
use crate::export::{Space, Transfer};
use crate::model::{Fields, Value};

/// Borders and watermark params (fields of [`crate::model::ModuleKind::FRAME`], version 1).
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// Border width: a fraction of the long side (0 … 0.5), or pixels with `border_px`.
    pub border: f32,
    pub border_px: bool,
    /// sRGB, 0 … 1.
    pub colour: [f32; 3],
    /// Canvas width / height to fit (0: none).
    pub aspect: f32,
    pub watermark: bool,
    /// An SVG file; empty: the built-in text template with [`Frame::text`].
    pub svg: String,
    pub text: String,
    /// 0 … 8: top-left, top, top-right, left, centre, right, bottom-left, bottom,
    /// bottom-right.
    pub anchor: u8,
    /// Watermark width, a fraction of the photo's width (0.02 … 1).
    pub scale: f32,
    pub opacity: f32,
    /// Margin, a fraction of the photo's long side (0 … 0.25).
    pub margin: f32,
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            border: 0.0,
            border_px: false,
            colour: [1.0, 1.0, 1.0],
            aspect: 0.0,
            watermark: false,
            svg: String::new(),
            text: "© $(exif.artist) $(date)".into(),
            anchor: 8,
            scale: 0.25,
            opacity: 0.6,
            margin: 0.02,
        }
    }
}

/// Anchors, in the order of [`Frame::anchor`].
pub const ANCHORS: usize = 9;

impl Frame {
    pub fn from_fields(f: &Fields) -> Self {
        let d = Self::default();
        let px = f.bool_or("border_px", false);
        let c = f.floats_or("colour", d.colour);
        Self {
            border: if px {
                get(f, "border", 0.0, 0.0, 4096.0).round()
            } else {
                get(f, "border", 0.0, 0.0, 0.5)
            },
            border_px: px,
            colour: c.map(|v| {
                if v.is_finite() {
                    v.clamp(0.0, 1.0)
                } else {
                    1.0
                }
            }),
            aspect: {
                let a = get(f, "aspect", 0.0, 0.0, 10.0);
                if a < 0.1 { 0.0 } else { a }
            },
            watermark: f.bool_or("watermark", false),
            svg: f.text("svg").unwrap_or("").to_owned(),
            text: f.text("text").unwrap_or(&d.text).to_owned(),
            anchor: f.i64_or("anchor", 8).clamp(0, 8) as u8,
            scale: get(f, "scale", d.scale, 0.02, 1.0),
            opacity: get(f, "opacity", d.opacity, 0.0, 1.0),
            margin: get(f, "margin", d.margin, 0.0, 0.25),
        }
    }

    pub fn default_fields() -> Fields {
        let d = Self::default();
        Fields::new()
            .with("border", Value::Float(d.border))
            .with("border_px", Value::Bool(d.border_px))
            .with("colour", Value::Floats(d.colour.to_vec()))
            .with("aspect", Value::Float(d.aspect))
            .with("watermark", Value::Bool(d.watermark))
            .with("svg", Value::Text(d.svg.clone()))
            .with("text", Value::Text(d.text.clone()))
            .with("anchor", Value::Int(i64::from(d.anchor)))
            .with("scale", Value::Float(d.scale))
            .with("opacity", Value::Float(d.opacity))
            .with("margin", Value::Float(d.margin))
    }

    /// Nothing to add.
    pub fn is_identity(&self) -> bool {
        self.border <= 0.0 && self.aspect == 0.0 && !(self.watermark && self.opacity > 0.0)
    }

    /// The border in pixels around a `w`×`h` photo.
    pub fn border_px_for(&self, w: u32, h: u32) -> u32 {
        if self.border_px {
            self.border as u32
        } else {
            (self.border * w.max(h) as f32).round() as u32
        }
    }

    /// Where things go for a `w`×`h` photo and a watermark of width/height `mark_aspect`.
    pub fn layout(&self, w: u32, h: u32, mark_aspect: Option<f32>) -> Layout {
        let b = self.border_px_for(w, h);
        let (mut cw, mut ch) = (w + 2 * b, h + 2 * b);
        if self.aspect > 0.0 {
            let a = self.aspect;
            if (cw as f32) < ch as f32 * a {
                cw = (ch as f32 * a).round() as u32;
            } else {
                ch = (cw as f32 / a).round() as u32;
            }
        }
        let (x, y) = ((cw - w) / 2, (ch - h) / 2);
        let mark = mark_aspect.filter(|a| *a > 0.0 && self.watermark).map(|a| {
            let mw = ((self.scale * w as f32).round() as u32).clamp(1, w.max(1));
            let mh = ((mw as f32 / a).round() as u32).clamp(1, h.max(1));
            let m = (self.margin * w.max(h) as f32).round() as u32;
            let col = u32::from(self.anchor % 3);
            let row = u32::from(self.anchor / 3);
            let place = |k: u32, room: u32, size: u32| -> u32 {
                let room = room.saturating_sub(size);
                match k {
                    0 => m.min(room),
                    1 => room / 2,
                    _ => room.saturating_sub(m),
                }
            };
            [x + place(col, w, mw), y + place(row, h, mh), mw, mh]
        });
        Layout {
            canvas: (cw, ch),
            image: [x, y, w, h],
            mark,
        }
    }
}

/// Where the photo and the watermark go on the canvas (pixels).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub canvas: (u32, u32),
    /// x, y, width, height.
    pub image: [u32; 4],
    pub mark: Option<[u32; 4]>,
}

/// The variables a watermark can name: `$(name)` becomes the value, unknown names
/// nothing, `$$` a dollar sign.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Vars(pub Vec<(String, String)>);

impl Vars {
    pub fn set(&mut self, k: &str, v: impl Into<String>) {
        let v = v.into();
        match self.0.iter_mut().find(|e| e.0 == k) {
            Some(e) => e.1 = v,
            None => self.0.push((k.to_owned(), v)),
        }
    }

    pub fn get(&self, k: &str) -> Option<&str> {
        self.0.iter().find(|e| e.0 == k).map(|e| e.1.as_str())
    }

    /// The variables of the photo at `path` with content `bytes`: `filename`, `title`
    /// (the description or Windows title, else the file's stem), `date` (capture date, else
    /// the file's time; `YYYY-MM-DD`), `exif.camera`, `exif.lens`, `exif.iso`,
    /// `exif.exposure`, `exif.aperture`, `exif.focal`, `exif.artist`, `exif.copyright`,
    /// `exif.date`.
    pub fn of_file(path: &Path, bytes: &[u8]) -> Vars {
        let mut v = Vars::default();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let stem = path
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        v.set("filename", name);
        let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        if let Some(d) = specular_convert::capture_date(bytes, modified) {
            v.set("date", format!("{:04}-{:02}-{:02}", d[0], d[1], d[2]));
            v.set(
                "exif.date",
                format!(
                    "{:04}-{:02}-{:02} {:02}:{:02}",
                    d[0], d[1], d[2], d[3], d[4]
                ),
            );
        }
        let tiff = specular_decode::exif_tiff(bytes);
        if let Some((c, _)) = tiff.as_deref().and_then(specular_decode::exif_facts) {
            v.set("exif.camera", c.camera.trim());
            if let Some(l) = c.lens {
                v.set("exif.lens", l.trim());
            }
            if let Some(i) = c.iso {
                v.set("exif.iso", format!("ISO {i}"));
            }
            if let Some(t) = c.exposure_time.filter(|t| *t > 0.0) {
                v.set("exif.exposure", exposure_text(t));
            }
            if let Some(f) = c.f_number {
                v.set("exif.aperture", format!("f/{}", trim_float(f)));
            }
            if let Some(f) = c.focal_length {
                v.set("exif.focal", format!("{} mm", trim_float(f)));
            }
        }
        let exif = tiff
            .as_deref()
            .and_then(specular_convert::exif::Exif::parse);
        let ascii = |tag: u16| -> Option<String> {
            let e = exif.as_ref()?.ifd0.iter().find(|e| e.tag == tag)?;
            let s = match e.ty {
                2 => String::from_utf8_lossy(&e.data).into_owned(),
                // XP tags: UTF-16LE in bytes.
                1 => {
                    let u: Vec<u16> = e
                        .data
                        .chunks_exact(2)
                        .map(|c| u16::from_le_bytes([c[0], c[1]]))
                        .collect();
                    String::from_utf16_lossy(&u)
                }
                _ => return None,
            };
            let s = s.trim_matches(|c: char| c == '\0' || c.is_whitespace());
            (!s.is_empty()).then(|| s.to_owned())
        };
        if let Some(a) = ascii(0x013B).or_else(|| ascii(0x9C9D)) {
            v.set("exif.artist", a);
        }
        if let Some(c) = ascii(0x8298) {
            v.set("exif.copyright", c);
        }
        v.set(
            "title",
            ascii(0x9C9B).or_else(|| ascii(0x010E)).unwrap_or(stem),
        );
        v
    }

    /// `s` with every `$(name)` replaced.
    pub fn expand(&self, s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut rest = s;
        while let Some(i) = rest.find('$') {
            out.push_str(&rest[..i]);
            let after = &rest[i + 1..];
            if let Some(a) = after.strip_prefix('$') {
                out.push('$');
                rest = a;
            } else if let Some(body) = after.strip_prefix('(')
                && let Some(j) = body.find(')')
            {
                out.push_str(self.get(body[..j].trim()).unwrap_or(""));
                rest = &body[j + 1..];
            } else {
                out.push('$');
                rest = after;
            }
        }
        out.push_str(rest);
        out
    }

    /// [`Vars::expand`] with each value escaped for XML text and attributes.
    pub fn expand_xml(&self, s: &str) -> String {
        let esc = Vars(
            self.0
                .iter()
                .map(|(k, v)| (k.clone(), xml_escape(v)))
                .collect(),
        );
        esc.expand(s)
    }
}

fn trim_float(v: f64) -> String {
    let s = format!("{v:.1}");
    s.strip_suffix(".0").map(str::to_owned).unwrap_or(s)
}

fn exposure_text(t: f64) -> String {
    if t >= 0.5 {
        format!("{} s", trim_float(t))
    } else {
        format!("1/{} s", (1.0 / t).round() as u64)
    }
}

/// `s` safe inside XML text and attribute values.
pub fn xml_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            c if (c as u32) < 0x20 && c != '\t' => {}
            c => o.push(c),
        }
    }
    o
}

/// Why there is no watermark.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarkError {
    /// The SVG file could not be read.
    Read(String),
    /// The SVG did not draw.
    Draw,
    /// The text is empty after its variables.
    Empty,
}

/// The watermark's SVG with its variables replaced: the chosen file, or the built-in text
/// template (white text with a soft dark outline).
pub fn mark_svg(frame: &Frame, vars: &Vars) -> Result<String, MarkError> {
    if !frame.svg.is_empty() {
        let text =
            std::fs::read_to_string(&frame.svg).map_err(|e| MarkError::Read(e.to_string()))?;
        return Ok(vars.expand_xml(&text));
    }
    let text = vars.expand(&frame.text);
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return Err(MarkError::Empty);
    }
    let n = text.chars().count().max(1) as f32;
    // About 0.6 em a character; 4 % room on each side.
    let (size, w) = (100.0f32, 100.0 * 0.6 * n + 16.0);
    Ok(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.0}\" height=\"{h:.0}\" viewBox=\"0 0 {w:.0} {h:.0}\">\
         <text x=\"50%\" y=\"{base:.0}\" text-anchor=\"middle\" font-family=\"sans-serif\" font-size=\"{size:.0}\" \
         fill=\"#ffffff\" stroke=\"#000000\" stroke-opacity=\"0.35\" stroke-width=\"4\" paint-order=\"stroke\">{t}</text></svg>",
        h = size * 1.3,
        base = size * 1.0,
        t = xml_escape(&text),
    ))
}

/// A drawn watermark: straight-alpha sRGB RGBA8 rows.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

impl Mark {
    pub fn aspect(&self) -> f32 {
        self.w as f32 / self.h.max(1) as f32
    }
}

/// Draws `svg` through the viewer's SVG renderer `width` px wide (drawn at that size when
/// larger than the renderer's 1024 px, else reduced by area).
pub fn render_mark(svg: &str, width: u32) -> Result<Mark, MarkError> {
    let (w0, h0, px) = specular_decode::decode_rgba8(svg.as_bytes()).ok_or(MarkError::Draw)?;
    let width = width.max(1);
    let (w0, h0, px) = if width > w0 {
        let h = ((width as f64 * f64::from(h0) / f64::from(w0)).round() as u32).max(1);
        let wrapped = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{h}\" viewBox=\"0 0 {width} {h}\">\
             <image width=\"{width}\" height=\"{h}\" preserveAspectRatio=\"none\" href=\"data:image/svg+xml;base64,{}\"/></svg>",
            crate::sidecar::b64::encode(svg.as_bytes())
        );
        specular_decode::decode_rgba8(wrapped.as_bytes())
            .filter(|(w, _, _)| *w >= width)
            .unwrap_or((w0, h0, px))
    } else {
        (w0, h0, px)
    };
    let h = ((width as f64 * f64::from(h0) / f64::from(w0)).round() as u32).max(1);
    Ok(Mark {
        w: width,
        h,
        rgba: resize_rgba(&px, w0, h0, width, h),
    })
}

/// Area-weighted resize of straight-alpha RGBA8 (premultiplied while averaging).
fn resize_rgba(src: &[u8], w0: u32, h0: u32, w: u32, h: u32) -> Vec<u8> {
    if (w0, h0) == (w, h) {
        return src.to_vec();
    }
    let fx = w0 as f64 / w as f64;
    let fy = h0 as f64 / h as f64;
    let mut out = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let (y0, y1) = (y as f64 * fy, (y + 1) as f64 * fy);
        for x in 0..w {
            let (x0, x1) = (x as f64 * fx, (x + 1) as f64 * fx);
            let mut acc = [0f64; 4];
            let mut wsum = 0.0;
            let ye = (y1.ceil() as u32).min(h0);
            let xe = (x1.ceil() as u32).min(w0);
            for sy in y0.floor() as u32..ye {
                let wy = (f64::from(sy + 1)).min(y1) - f64::from(sy).max(y0);
                for sx in x0.floor() as u32..xe {
                    let wx = (f64::from(sx + 1)).min(x1) - f64::from(sx).max(x0);
                    let wt = wx * wy;
                    if wt <= 0.0 {
                        continue;
                    }
                    let i = ((sy * w0 + sx) * 4) as usize;
                    let a = f64::from(src[i + 3]) / 255.0;
                    for k in 0..3 {
                        acc[k] += f64::from(src[i + k]) * a * wt;
                    }
                    acc[3] += a * wt;
                    wsum += wt;
                }
            }
            let o = ((y * w + x) * 4) as usize;
            if wsum > 0.0 && acc[3] > 0.0 {
                for k in 0..3 {
                    out[o + k] = (acc[k] / acc[3]).round().clamp(0.0, 255.0) as u8;
                }
                out[o + 3] = (acc[3] / wsum * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

fn srgb_decode(v: f64) -> f64 {
    Transfer::Srgb.decode(v)
}

/// An sRGB colour as linear values of `space`.
pub fn srgb_in(space: Space, c: [f32; 3]) -> [f64; 3] {
    let lin = c.map(|v| srgb_decode(f64::from(v)));
    let rec2020 = crate::colour::mulv(&crate::colour::rec709_to_rec2020(), lin);
    crate::colour::mulv(&space.from_rec2020(), rec2020)
}

/// Adds the frame to an exported image: a new canvas in the border colour with the photo
/// in it, and the watermark over the photo. `mark` must be drawn for this photo's width
/// (see [`Frame::layout`]).
pub fn apply(img: &mut Exported, frame: &Frame, mark: Option<&Mark>, space: Space, float: bool) {
    let lay = frame.layout(img.width, img.height, mark.map(Mark::aspect));
    let transfer = if float {
        Transfer::Linear
    } else {
        space.transfer()
    };
    let border = srgb_in(space, frame.colour).map(|v| transfer.encode(v));
    let (cw, ch) = lay.canvas;
    let n = cw as usize * ch as usize * 3;
    // Blends the watermark over the photo (canvas coordinates) in linear light.
    let mark_px = |x: u32, y: u32| -> Option<([f64; 3], f64)> {
        let m = mark?;
        let [mx, my, mw, mh] = lay.mark?;
        if x < mx || y < my || x >= mx + mw || y >= my + mh {
            return None;
        }
        let (u, v) = (
            ((x - mx) * m.w / mw).min(m.w - 1),
            ((y - my) * m.h / mh).min(m.h - 1),
        );
        let i = ((v * m.w + u) * 4) as usize;
        let a = f64::from(m.rgba[i + 3]) / 255.0 * f64::from(frame.opacity);
        if a <= 0.0 {
            return None;
        }
        let c = srgb_in(
            space,
            [
                f32::from(m.rgba[i]) / 255.0,
                f32::from(m.rgba[i + 1]) / 255.0,
                f32::from(m.rgba[i + 2]) / 255.0,
            ],
        );
        Some((c, a))
    };
    let blend = |stored: [f64; 3], x: u32, y: u32| -> [f64; 3] {
        match mark_px(x, y) {
            // Float samples are linear and unclipped: highlights under the mark keep their
            // value above 1 (the transfer would clip them to 0..1).
            Some((c, a)) if float => std::array::from_fn(|k| stored[k] * (1.0 - a) + c[k] * a),
            Some((c, a)) => std::array::from_fn(|k| {
                let l = transfer.decode(stored[k]);
                transfer.encode(l * (1.0 - a) + c[k] * a)
            }),
            None => stored,
        }
    };
    fn build<T: Copy>(
        src: &[T],
        n: usize,
        lay: &Layout,
        fill: [T; 3],
        mut px: impl FnMut(&[T], u32, u32) -> [T; 3],
    ) -> Vec<T> {
        let (cw, ch) = lay.canvas;
        let [ix, iy, iw, ih] = lay.image;
        let mut out = Vec::with_capacity(n);
        for y in 0..ch {
            for x in 0..cw {
                if x >= ix && y >= iy && x < ix + iw && y < iy + ih {
                    let i = (((y - iy) * iw + (x - ix)) * 3) as usize;
                    out.extend_from_slice(&px(&src[i..i + 3], x, y));
                } else {
                    out.extend_from_slice(&fill);
                }
            }
        }
        out
    }
    img.samples = match &img.samples {
        Samples::Eight(s) => {
            let fill = border.map(|v| (v * 255.0).round() as u8);
            Samples::Eight(build(s, n, &lay, fill, |p, x, y| {
                let o = blend([0, 1, 2].map(|k| f64::from(p[k]) / 255.0), x, y);
                o.map(|v| (v * 255.0).round().clamp(0.0, 255.0) as u8)
            }))
        }
        Samples::Sixteen(s) => {
            let fill = border.map(|v| (v * 65535.0).round() as u16);
            Samples::Sixteen(build(s, n, &lay, fill, |p, x, y| {
                let o = blend([0, 1, 2].map(|k| f64::from(p[k]) / 65535.0), x, y);
                o.map(|v| (v * 65535.0).round().clamp(0.0, 65535.0) as u16)
            }))
        }
        Samples::Float(s) => {
            let fill = border.map(|v| v as f32);
            Samples::Float(build(s, n, &lay, fill, |p, x, y| {
                let o = blend([f64::from(p[0]), f64::from(p[1]), f64::from(p[2])], x, y);
                o.map(|v| v as f32)
            }))
        }
    };
    img.width = cw;
    img.height = ch;
}

/// The coverage of an exported image with `edit`'s borders and watermark (when its frame
/// module is on): `alpha` (`w × h`) placed on the canvas [`add_to`] makes, the border
/// opaque, the watermark's own coverage over the photo's. Returns the canvas size and
/// values; `None` when the frame adds nothing.
pub fn add_alpha(
    alpha: &[f32],
    w: u32,
    h: u32,
    edit: &crate::model::Edit,
    vars: &Vars,
) -> Option<(u32, u32, Vec<f32>)> {
    let m = edit
        .module(crate::model::ModuleKind::FRAME, 0)
        .filter(|m| m.enabled)?;
    let frame = Frame::from_fields(&m.params.to_fields());
    if frame.is_identity() {
        return None;
    }
    let mark = frame
        .watermark
        .then(|| {
            let width = ((frame.scale * w as f32).round() as u32).max(1);
            mark_svg(&frame, vars)
                .and_then(|svg| render_mark(&svg, width))
                .ok()
        })
        .flatten();
    let lay = frame.layout(w, h, mark.as_ref().map(Mark::aspect));
    let (cw, ch) = lay.canvas;
    let [ix, iy, iw, ih] = lay.image;
    let mut out = vec![1f32; cw as usize * ch as usize];
    for y in 0..ih.min(h) {
        for x in 0..iw.min(w) {
            let mut a = alpha[(y * w + x) as usize];
            let (cx, cy) = (x + ix, y + iy);
            if let (Some(m), Some([mx, my, mw, mh])) = (&mark, lay.mark)
                && cx >= mx
                && cy >= my
                && cx < mx + mw
                && cy < my + mh
            {
                let (u, v) = (
                    ((cx - mx) * m.w / mw).min(m.w - 1),
                    ((cy - my) * m.h / mh).min(m.h - 1),
                );
                let am =
                    f32::from(m.rgba[((v * m.w + u) * 4 + 3) as usize]) / 255.0 * frame.opacity;
                a += am.clamp(0.0, 1.0) * (1.0 - a);
            }
            out[(cy * cw + cx) as usize] = a;
        }
    }
    Some((cw, ch, out))
}

/// Adds `edit`'s borders and watermark (when its frame module is on) to an exported image.
/// A watermark that does not draw is left out (`Err` says why; the border is still added).
pub fn add_to(
    img: &mut Exported,
    edit: &crate::model::Edit,
    vars: &Vars,
    space: Space,
    float: bool,
) -> Result<(), MarkError> {
    let Some(m) = edit
        .module(crate::model::ModuleKind::FRAME, 0)
        .filter(|m| m.enabled)
    else {
        return Ok(());
    };
    let frame = Frame::from_fields(&m.params.to_fields());
    if frame.is_identity() {
        return Ok(());
    }
    let mut problem = Ok(());
    let mark = if frame.watermark {
        let width = ((frame.scale * img.width as f32).round() as u32).max(1);
        match mark_svg(&frame, vars).and_then(|svg| render_mark(&svg, width)) {
            Ok(m) => Some(m),
            Err(e) => {
                problem = Err(e);
                None
            }
        }
    } else {
        None
    };
    apply(img, &frame, mark.as_ref(), space, float);
    problem
}
