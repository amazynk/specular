//! Label masks: annotations painted as class indices (0 background, 1.. the classes in
//! [`crate::annot::classes`] order) over a region of the slide at a level, written as PNG or
//! TIFF tiles with a class table beside them, or as one OME-TIFF label pyramid.
//!
//! A pixel takes a polygon's class when its **centre** is inside (even-odd over the
//! polygon's rings, half-open crossings: a centre on a left edge is in, on a right edge out),
//! the rule of `specular-measure`'s ROI statistics, so an axis-aligned rectangle on integer
//! bounds covers exactly its pixels. A point marks the one pixel containing it; lines are
//! left out. Later annotations paint over earlier ones.
//!
//! Every tile and every pyramid level is rasterised from the vectors themselves at its own
//! scale, so memory stays at one tile row however large the slide, and reduced levels are
//! exact rather than resampled.

use std::io::{self, Seek, Write};
use std::path::{Path, PathBuf};

use crate::annot::{Annotation, Class, Geometry, Poly, class_index, classes, palette};
use crate::json;
use crate::tiff::{self, Level};

/// Calls `span(row, i0, i1)` for every run of pixels `i0..i1` on `row` whose centres are
/// inside `poly`, on a grid of `w` × `h` pixels of size `d` whose corner is `org`.
pub fn spans(
    poly: &Poly,
    org: (f64, f64),
    d: f64,
    w: u32,
    h: u32,
    mut span: impl FnMut(u32, u32, u32),
) {
    let rings: Vec<&Vec<(f64, f64)>> = std::iter::once(&poly.exterior).chain(&poly.holes).collect();
    let (mut y0, mut y1) = (f64::INFINITY, -f64::INFINITY);
    for r in &rings {
        for p in r.iter() {
            y0 = y0.min(p.1);
            y1 = y1.max(p.1);
        }
    }
    if !(y0.is_finite() && y1.is_finite()) || d <= 0.0 {
        return;
    }
    let row = |y: f64| ((y - org.1) / d - 0.5).ceil();
    let j0 = row(y0).max(0.0) as u32;
    let j1 = (row(y1).max(0.0) as u32).min(h);
    let mut xs: Vec<f64> = Vec::new();
    for j in j0..j1 {
        let cy = org.1 + (f64::from(j) + 0.5) * d;
        xs.clear();
        for r in &rings {
            let n = r.len();
            if n < 3 {
                continue;
            }
            for k in 0..n {
                let (a, b) = (r[k], r[(k + 1) % n]);
                if (a.1 <= cy) != (b.1 <= cy) {
                    xs.push(a.0 + (cy - a.1) * (b.0 - a.0) / (b.1 - a.1));
                }
            }
        }
        xs.sort_by(f64::total_cmp);
        for pair in xs.chunks_exact(2) {
            let col = |x: f64| ((x - org.0) / d - 0.5).ceil().clamp(0.0, f64::from(w)) as u32;
            let (i0, i1) = (col(pair[0]), col(pair[1]));
            if i1 > i0 {
                span(j, i0, i1);
            }
        }
    }
}

/// A region of the slide in level-0 pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Region {
    /// The region's size at downsample `d` (whole pixels, rounded up).
    pub fn size_at(&self, d: f64) -> (u32, u32) {
        let s = |v: f64| (v / d).ceil().clamp(1.0, f64::from(u32::MAX)) as u32;
        (s(self.w), s(self.h))
    }
}

/// Paints `items` into `out` (`tw` × `th` class indices, row-major), the tile at level pixel
/// (`tx`, `ty`) of `region` at downsample `d`. `classes` gives the indices (index + 1).
#[allow(clippy::too_many_arguments)]
pub fn paint(
    items: &[Annotation],
    classes: &[Class],
    region: &Region,
    d: f64,
    tx: u32,
    ty: u32,
    tw: u32,
    th: u32,
    out: &mut [u16],
) {
    let org = (region.x + f64::from(tx) * d, region.y + f64::from(ty) * d);
    let (ex, ey) = (org.0 + f64::from(tw) * d, org.1 + f64::from(th) * d);
    for a in items {
        let (x0, y0, x1, y1) = a.geometry.bounds();
        if x1 < org.0 || y1 < org.1 || x0 > ex || y0 > ey {
            continue;
        }
        let v = class_index(classes, a) as u16 + 1;
        match &a.geometry {
            g if g.is_area() => {
                for p in g.polys() {
                    spans(&p, org, d, tw, th, |j, i0, i1| {
                        let base = j as usize * tw as usize;
                        out[base + i0 as usize..base + i1 as usize].fill(v);
                    });
                }
            }
            Geometry::Point(p) => dot(out, org, d, tw, th, *p, v),
            Geometry::Points(ps) => {
                for p in ps {
                    dot(out, org, d, tw, th, *p, v);
                }
            }
            _ => {}
        }
    }
}

fn dot(out: &mut [u16], org: (f64, f64), d: f64, tw: u32, th: u32, p: (f64, f64), v: u16) {
    let (i, j) = (((p.0 - org.0) / d).floor(), ((p.1 - org.1) / d).floor());
    if i >= 0.0 && j >= 0.0 && i < f64::from(tw) && j < f64::from(th) {
        out[j as usize * tw as usize + i as usize] = v;
    }
}

/// How tiles are written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileFormat {
    Png,
    Tiff,
}

impl TileFormat {
    pub fn extension(self) -> &'static str {
        match self {
            TileFormat::Png => "png",
            TileFormat::Tiff => "tif",
        }
    }
}

/// What to write.
#[derive(Clone, Debug, PartialEq)]
pub struct MaskJob {
    pub region: Region,
    /// Level-0 pixels per output pixel (1 for full resolution).
    pub downsample: f64,
    /// Tile edge, output pixels.
    pub tile: u32,
    /// Leave out tiles without any label.
    pub skip_empty: bool,
}

/// Most output pixels in one label image file (PNG/TIFF tiles; the pyramid tiles itself).
pub const MAX_TILE: u32 = 16_384;
/// Most tiles one export writes as separate files.
pub const MAX_FILES: usize = 100_000;

/// Why a mask could not be written.
#[derive(Debug)]
pub enum MaskError {
    Io(io::Error),
    /// Nothing to paint (no polygons, rectangles or points).
    NoAreas,
    /// More tiles than [`MAX_FILES`].
    TooManyTiles(usize),
    /// A tile edge of 0 or over [`MAX_TILE`], or a bad downsample.
    BadTile,
}

impl std::fmt::Display for MaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MaskError::Io(e) => write!(f, "{e}"),
            MaskError::NoAreas => f.write_str("no regions or points to label"),
            MaskError::TooManyTiles(n) => write!(f, "{n} tiles, over {MAX_FILES}"),
            MaskError::BadTile => f.write_str("tile size or level out of range"),
        }
    }
}

impl From<io::Error> for MaskError {
    fn from(e: io::Error) -> Self {
        MaskError::Io(e)
    }
}

/// What an export wrote.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Written {
    pub files: Vec<PathBuf>,
    /// Lines left out (masks hold regions and points).
    pub lines_skipped: usize,
    /// Tiles left out for being empty.
    pub empty_skipped: usize,
}

fn check(items: &[Annotation], job: &MaskJob) -> Result<(), MaskError> {
    if !items
        .iter()
        .any(|a| !matches!(a.geometry, Geometry::Line(_)))
    {
        return Err(MaskError::NoAreas);
    }
    if job.tile == 0 || job.tile > MAX_TILE || !(job.downsample.is_finite() && job.downsample > 0.0)
    {
        return Err(MaskError::BadTile);
    }
    Ok(())
}

/// The class table beside the tiles: index, name and colour of every class.
pub fn class_table(classes: &[Class]) -> String {
    let mut s = String::from("{\"background\":0,\"classes\":[");
    for (i, c) in classes.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let colour = c.colour.unwrap_or_else(|| palette(i));
        s.push_str(&format!(
            "{{\"index\":{},\"name\":{},\"color\":{}}}",
            i + 1,
            json::string(&c.name),
            json::rgb(colour)
        ));
    }
    s.push_str("]}\n");
    s
}

fn wide(classes: &[Class]) -> bool {
    classes.len() > 255
}

/// Writes the region's labels as tiles `<stem>_x<X>_y<Y>.<ext>` (level-0 position of each
/// tile's corner, QuPath's tile naming) in `dir`, and the class table `<stem>-classes.json`.
/// 8-bit grey up to 255 classes, else 16-bit.
pub fn write_tiles(
    items: &[Annotation],
    job: &MaskJob,
    format: TileFormat,
    dir: &Path,
    stem: &str,
) -> Result<Written, MaskError> {
    check(items, job)?;
    let cls = classes(items);
    let (w, h) = job.region.size_at(job.downsample);
    let t = job.tile;
    let (nx, ny) = (w.div_ceil(t), h.div_ceil(t));
    let n = nx as usize * ny as usize;
    if n > MAX_FILES {
        return Err(MaskError::TooManyTiles(n));
    }
    let mut out = Written {
        lines_skipped: items
            .iter()
            .filter(|a| matches!(a.geometry, Geometry::Line(_)))
            .count(),
        ..Written::default()
    };
    let mut buf = Vec::new();
    for ty in 0..ny {
        for tx in 0..nx {
            let (x, y) = (tx * t, ty * t);
            let (tw, th) = (t.min(w - x), t.min(h - y));
            buf.clear();
            buf.resize(tw as usize * th as usize, 0u16);
            paint(
                items,
                &cls,
                &job.region,
                job.downsample,
                x,
                y,
                tw,
                th,
                &mut buf,
            );
            if job.skip_empty && buf.iter().all(|v| *v == 0) {
                out.empty_skipped += 1;
                continue;
            }
            let (lx, ly) = (
                (job.region.x + f64::from(x) * job.downsample).round() as i64,
                (job.region.y + f64::from(y) * job.downsample).round() as i64,
            );
            let path = dir.join(format!("{stem}_x{lx}_y{ly}.{}", format.extension()));
            let bytes = match format {
                TileFormat::Png => png(&buf, tw, th, wide(&cls))?,
                TileFormat::Tiff => {
                    let mut c = io::Cursor::new(Vec::new());
                    let samples = samples(&buf, wide(&cls));
                    tiff::write(
                        &mut c,
                        &[Level {
                            width: tw,
                            height: th,
                        }],
                        tw.max(th).next_multiple_of(16),
                        if wide(&cls) { 16 } else { 8 },
                        None,
                        false,
                        |_, _, _, _| {
                            tile_of(
                                &samples,
                                tw,
                                th,
                                wide(&cls),
                                tw.max(th).next_multiple_of(16),
                            )
                        },
                    )?;
                    c.into_inner()
                }
            };
            std::fs::write(&path, bytes)?;
            out.files.push(path);
        }
    }
    let table = dir.join(format!("{stem}-classes.json"));
    std::fs::write(&table, class_table(&cls))?;
    out.files.push(table);
    Ok(out)
}

/// Little-endian samples of `v` (8 or 16 bits).
fn samples(v: &[u16], wide: bool) -> Vec<u8> {
    if wide {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    } else {
        v.iter().map(|&x| x as u8).collect()
    }
}

/// A `t` × `t` tile padded with zeros from `w` × `h` samples.
fn tile_of(s: &[u8], w: u32, h: u32, wide: bool, t: u32) -> Vec<u8> {
    let b = if wide { 2 } else { 1 };
    let mut out = vec![0u8; (t * t) as usize * b];
    for y in 0..h.min(t) as usize {
        let src = &s[y * w as usize * b..(y + 1) * w as usize * b];
        out[y * t as usize * b..][..src.len()].copy_from_slice(src);
    }
    out
}

fn png(v: &[u16], w: u32, h: u32, wide: bool) -> Result<Vec<u8>, MaskError> {
    let data: Vec<u8> = if wide {
        v.iter().flat_map(|x| x.to_be_bytes()).collect()
    } else {
        v.iter().map(|&x| x as u8).collect()
    };
    specular_png::encode_with(
        w,
        h,
        specular_png::ColorType::Gray,
        &data,
        &specular_png::EncodeOptions {
            sixteen_bit: wide,
            level: Some(6),
            ..Default::default()
        },
    )
    .map_err(|e| MaskError::Io(io::Error::other(e.to_string())))
}

/// Levels of a pyramid over `w` × `h`: halved until both fit one `tile`.
pub fn pyramid_levels(w: u32, h: u32, tile: u32) -> Vec<Level> {
    let mut out = vec![Level {
        width: w,
        height: h,
    }];
    let (mut w, mut h) = (w, h);
    while (w > tile || h > tile) && out.len() < 32 {
        w = w.div_ceil(2);
        h = h.div_ceil(2);
        out.push(Level {
            width: w,
            height: h,
        });
    }
    out
}

/// The OME-XML of a single-plane label image with its class table.
pub fn ome_xml(
    name: &str,
    w: u32,
    h: u32,
    wide: bool,
    classes: &[Class],
    pixel_um: Option<f64>,
) -> String {
    let ty = if wide { "uint16" } else { "uint8" };
    let phys = pixel_um.map_or(String::new(), |p| {
        format!(" PhysicalSizeX=\"{p}\" PhysicalSizeXUnit=\"µm\" PhysicalSizeY=\"{p}\" PhysicalSizeYUnit=\"µm\"")
    });
    let mut map = String::new();
    map.push_str("<M K=\"0\">background</M>");
    for (i, c) in classes.iter().enumerate() {
        map.push_str(&format!("<M K=\"{}\">{}</M>", i + 1, xml_escape(&c.name)));
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<OME xmlns=\"http://www.openmicroscopy.org/Schemas/OME/2016-06\" \
xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
xsi:schemaLocation=\"http://www.openmicroscopy.org/Schemas/OME/2016-06 http://www.openmicroscopy.org/Schemas/OME/2016-06/ome.xsd\" \
Creator=\"Specular\">\
<Image ID=\"Image:0\" Name=\"{}\"><Pixels ID=\"Pixels:0\" DimensionOrder=\"XYZCT\" Type=\"{ty}\" \
SizeX=\"{w}\" SizeY=\"{h}\" SizeZ=\"1\" SizeC=\"1\" SizeT=\"1\"{phys}>\
<Channel ID=\"Channel:0:0\" Name=\"labels\" SamplesPerPixel=\"1\"/>\
<TiffData IFD=\"0\" PlaneCount=\"1\"/></Pixels>\
<AnnotationRef ID=\"Annotation:0\"/></Image>\
<StructuredAnnotations><MapAnnotation ID=\"Annotation:0\" Namespace=\"specular.labels\"><Value>{map}</Value></MapAnnotation></StructuredAnnotations>\
</OME>",
        xml_escape(name)
    )
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Writes the region's labels as an OME-TIFF label pyramid (BigTIFF, tiled, Deflate; the
/// reduced levels in SubIFDs of the full one, as Bio-Formats reads them) to `out`.
pub fn write_ome_tiff<W: Write + Seek>(
    items: &[Annotation],
    job: &MaskJob,
    name: &str,
    pixel_um: Option<f64>,
    out: &mut W,
) -> Result<Written, MaskError> {
    check(items, job)?;
    let cls = classes(items);
    let (w, h) = job.region.size_at(job.downsample);
    let t = job.tile.next_multiple_of(16);
    let levels = pyramid_levels(w, h, t);
    let wide = wide(&cls);
    let xml = ome_xml(name, w, h, wide, &cls, pixel_um.map(|p| p * job.downsample));
    let mut buf = Vec::new();
    tiff::write(
        out,
        &levels,
        t,
        if wide { 16 } else { 8 },
        Some(&xml),
        true,
        |level, tx, ty, lv| {
            let d = job.downsample * f64::from(1u32 << level);
            let (x, y) = (tx * t, ty * t);
            let (tw, th) = (t.min(lv.width - x), t.min(lv.height - y));
            buf.clear();
            buf.resize(tw as usize * th as usize, 0u16);
            paint(items, &cls, &job.region, d, x, y, tw, th, &mut buf);
            tile_of(&samples(&buf, wide), tw, th, wide, t)
        },
    )?;
    Ok(Written {
        lines_skipped: items
            .iter()
            .filter(|a| matches!(a.geometry, Geometry::Line(_)))
            .count(),
        ..Written::default()
    })
}

#[cfg(test)]
mod tests;
