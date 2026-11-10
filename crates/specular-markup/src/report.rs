//! A thermal inspection report as a PDF: A4 pages, one per image, with a short factual
//! layout.
//!
//! - Page 1 opens with the report's title, a line under it (date, author) and the notes.
//! - Each image: its name; the thermal image in its palette with the measurements drawn on
//!   it (numbered as in the table) and a colour scale with its coldest and hottest
//!   temperature, the visible photo beside it when there is one; the facts (camera, date,
//!   place, object parameters) as label and value rows; the measurements table (spot, line or
//!   area: minimum, maximum, mean, the emissivity used).
//! - Right-to-left languages mirror the layout: tables start on the right, labels align
//!   right, the thermal image is first on the right.
//!
//! Text is Helvetica (WinAnsi), searchable and copyable. A line with characters WinAnsi has
//! no code for (Hebrew, Arabic, CJK…) is drawn as outlines set by the caller's text system
//! (an [`OutlineText`], the viewer's, with its shaping, bidi and fallback fonts), as the
//! marked-up PDF export does; without one such characters print as `?`.

use std::fmt::Write as _;

use specular_pdf::annotate::num;

use crate::P;
use crate::pdf::{OutlineText, Pen, text_string};

/// A4 portrait, points.
const PAGE_W: f64 = 595.0;
const PAGE_H: f64 = 842.0;
const MARGIN: f64 = 40.0;
/// Room between the thermal image and the photo.
const GUTTER: f64 = 14.0;
/// Tallest an image may be drawn, points.
const IMAGE_H: f64 = 250.0;

const INK: [u8; 3] = [24, 24, 28];
const MUTED: [u8; 3] = [96, 98, 106];
const RULE: [u8; 3] = [200, 202, 208];
const BAND: [u8; 3] = [236, 237, 240];

/// An image, 8-bit RGB, rows top down.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub rgb: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// A measurement drawn on the thermal image: points in its pixels, closed for an area, one
/// point for a spot, and its number.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub points: Vec<(f64, f64)>,
    pub closed: bool,
    pub label: String,
}

/// The palette from coldest to hottest, with the temperatures at its ends ("12.4 °C").
#[derive(Clone, Debug, PartialEq)]
pub struct Scale {
    pub colours: Vec<[u8; 3]>,
    pub low: String,
    pub high: String,
}

/// One image's page.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// The file's name.
    pub title: String,
    pub thermal: Image,
    pub marks: Vec<Mark>,
    pub scale: Option<Scale>,
    pub visible: Option<Image>,
    /// Captions under the images.
    pub thermal_caption: String,
    pub visible_caption: String,
    /// Label and value rows: camera, date, GPS, object parameters.
    pub facts: Vec<(String, String)>,
    /// The measurements: a header row, then a row per measurement.
    pub table: Vec<Vec<String>>,
    /// Shown instead of the table when it is empty.
    pub no_table: String,
}

/// The report.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub title: String,
    /// Under the title (date, author).
    pub subtitle: String,
    pub notes: String,
    /// Headings ("Notes", "Measurements").
    pub notes_heading: String,
    pub table_heading: String,
    pub entries: Vec<Entry>,
    /// "Page 1 of 2" for each page, from the caller (localised).
    pub page_labels: Vec<String>,
    pub rtl: bool,
    /// PDF date (`D:20261025120000Z`), when known.
    pub created: Option<String>,
    pub producer: String,
}

/// One page as it is drawn: the content stream and its images.
struct Page<'a> {
    pen: Pen<'a>,
    images: Vec<Image>,
    rtl: bool,
}

fn colour(c: [u8; 3]) -> String {
    let [r, g, b] = c.map(|v| num(f64::from(v) / 255.0));
    format!("{r} {g} {b}")
}

impl<'a> Page<'a> {
    fn new(outlines: Option<&'a OutlineText<'a>>, rtl: bool) -> Page<'a> {
        let mut pen = Pen(Vec::new(), outlines);
        // y down from the top left, as laid out.
        pen.op(&format!("1 0 0 -1 0 {} cm", num(PAGE_H)));
        Page {
            pen,
            images: Vec::new(),
            rtl,
        }
    }

    /// The box `x..x + w` as drawn: mirrored in right-to-left layouts.
    fn mx(&self, x: f64, w: f64) -> f64 {
        if self.rtl { PAGE_W - x - w } else { x }
    }

    fn fill(&mut self, c: [u8; 3]) {
        self.pen.op(&format!("{} rg", colour(c)));
    }

    /// Text at the start of the box `x..x + w` (logical, before mirroring), baseline `y`.
    #[allow(clippy::too_many_arguments)]
    fn text(&mut self, s: &str, x: f64, w: f64, y: f64, size: f64, bold: bool, c: [u8; 3]) {
        let s = fit(&self.pen, s, w, size, bold);
        let tw = self.pen.width(&s, size, bold);
        let bx = self.mx(x, w);
        let tx = if self.rtl { bx + w - tw } else { bx };
        self.fill(c);
        self.pen.text(&s, tx, y, size, bold);
    }

    /// Text wrapped to `w`; the baseline of the line after it.
    #[allow(clippy::too_many_arguments)]
    fn paragraph(
        &mut self,
        s: &str,
        x: f64,
        w: f64,
        mut y: f64,
        size: f64,
        c: [u8; 3],
        bottom: f64,
    ) -> f64 {
        for line in wrap(&self.pen, s, w, size) {
            if y > bottom {
                break;
            }
            self.text(&line, x, w, y, size, false, c);
            y += size * 1.35;
        }
        y
    }

    fn rule(&mut self, x: f64, w: f64, y: f64) {
        self.pen.op(&format!("{} RG 0.6 w", colour(RULE)));
        self.pen.op(&format!(
            "{} {} m {} {} l S",
            num(x),
            num(y),
            num(x + w),
            num(y)
        ));
    }

    /// Draws `img` in the box at `(x, y)` (logical), `w × h`.
    fn image(&mut self, img: Image, x: f64, y: f64, w: f64, h: f64) {
        let k = self.images.len();
        self.images.push(img);
        let x = self.mx(x, w);
        self.pen.op(&format!(
            "q {} 0 0 {} {} {} cm /Im{k} Do Q",
            num(w),
            num(-h),
            num(x),
            num(y + h)
        ));
    }
}

/// `s` shortened with "…" to fit `w`.
fn fit(pen: &Pen<'_>, s: &str, w: f64, size: f64, bold: bool) -> String {
    if pen.width(s, size, bold) <= w {
        return s.to_owned();
    }
    let chars: Vec<char> = s.chars().collect();
    let (mut lo, mut hi) = (0, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let t: String = chars[..mid].iter().collect::<String>() + "…";
        if pen.width(&t, size, bold) <= w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    chars[..lo].iter().collect::<String>().trim_end().to_owned() + "…"
}

/// `s` broken into lines no wider than `w` at spaces (and at its own line breaks).
fn wrap(pen: &Pen<'_>, s: &str, w: f64, size: f64) -> Vec<String> {
    let mut out = Vec::new();
    for para in s.lines() {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let next = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && pen.width(&next, size, false) > w {
                out.push(std::mem::take(&mut line));
                line = word.to_owned();
            } else {
                line = next;
            }
        }
        out.push(line);
    }
    out
}

/// The measurements drawn over the thermal image placed at `(x, y)` (logical), `k` points
/// per pixel.
fn marks(page: &mut Page<'_>, marks: &[Mark], x: f64, y: f64, w: f64, k: f64) {
    let to = |p: (f64, f64), page: &Page<'_>| {
        let px = x + p.0 * k;
        let px = if page.rtl {
            page.mx(x, w) + (px - x)
        } else {
            px
        };
        P::new(px, y + p.1 * k)
    };
    for m in marks {
        let pts: Vec<P> = m.points.iter().map(|&p| to(p, page)).collect();
        let Some(&first) = pts.first() else { continue };
        // A dark halo under a white line keeps it visible on any palette.
        for (c, lw) in [([0, 0, 0], 2.2), ([255, 255, 255], 1.0)] {
            page.pen.op(&format!("{} RG {} w", colour(c), num(lw)));
            if pts.len() == 1 {
                let (cx, cy, r) = (first.x, first.y, 5.0);
                page.pen.op(&format!(
                    "{} {} m {} {} l {} {} m {} {} l S",
                    num(cx - r),
                    num(cy),
                    num(cx + r),
                    num(cy),
                    num(cx),
                    num(cy - r),
                    num(cx),
                    num(cy + r)
                ));
            } else {
                page.pen.poly(&pts, m.closed);
                page.pen.op("S");
            }
        }
        let size = 7.0;
        let tw = page.pen.width(&m.label, size, true);
        let (bx, by) = (first.x + 3.0, first.y - size - 4.0);
        page.fill([0, 0, 0]);
        page.pen.op(&format!(
            "{} {} {} {} re f",
            num(bx),
            num(by),
            num(tw + 4.0),
            num(size + 3.0)
        ));
        page.fill([255, 255, 255]);
        page.pen
            .text(&m.label, bx + 2.0, by + size + 0.5, size, true);
    }
}

/// Lays out `e` from `y`; the y after it.
fn entry(page: &mut Page<'_>, r: &Report, e: &Entry, mut y: f64) -> f64 {
    let full = PAGE_W - 2.0 * MARGIN;
    let bottom = PAGE_H - MARGIN - 20.0;
    page.text(&e.title, MARGIN, full, y + 11.0, 12.0, true, INK);
    y += 20.0;
    // The images: thermal first (at the start), the photo beside it.
    let n = if e.visible.is_some() { 2.0 } else { 1.0 };
    let slot = (full - GUTTER * (n - 1.0)) / n;
    let fit_box = |img: &Image| {
        let (iw, ih) = (f64::from(img.width.max(1)), f64::from(img.height.max(1)));
        let k = (slot / iw).min(IMAGE_H / ih);
        (iw * k, ih * k, k)
    };
    let (tw, th, tk) = fit_box(&e.thermal);
    page.image(e.thermal.clone(), MARGIN, y, tw, th);
    marks(page, &e.marks, MARGIN, y, tw, tk);
    let mut below = y + th;
    if let Some(v) = &e.visible {
        let (vw, vh, _) = fit_box(v);
        let vx = MARGIN + slot + GUTTER;
        page.image(v.clone(), vx, y, vw, vh);
        page.text(&e.visible_caption, vx, vw, y + vh + 11.0, 8.0, false, MUTED);
        below = below.max(y + vh + 14.0);
    }
    // The scale under the thermal image, its ends labelled.
    let mut sy = y + th + 4.0;
    if let Some(s) = &e.scale {
        let mut rgb = Vec::with_capacity(s.colours.len() * 3);
        let cols: Vec<[u8; 3]> = if page.rtl {
            // Cold stays at the start of the reading direction.
            s.colours.iter().rev().copied().collect()
        } else {
            s.colours.clone()
        };
        for c in &cols {
            rgb.extend_from_slice(c);
        }
        let bar = Image {
            rgb,
            width: cols.len().max(1) as u32,
            height: 1,
        };
        page.image(bar, MARGIN, sy, tw, 8.0);
        let lw = tw / 2.0;
        page.text(&s.low, MARGIN, lw, sy + 17.0, 8.0, false, INK);
        let hw = page.pen.width(&s.high, 8.0, false).min(lw);
        page.text(&s.high, MARGIN + tw - hw, hw, sy + 17.0, 8.0, false, INK);
        sy += 22.0;
    }
    page.text(&e.thermal_caption, MARGIN, tw, sy + 9.0, 8.0, false, MUTED);
    y = below.max(sy + 14.0) + 10.0;
    // The facts: label and value columns.
    let label_w = e
        .facts
        .iter()
        .map(|(l, _)| page.pen.width(l, 9.0, true))
        .fold(0.0, f64::max)
        .min(full * 0.35);
    for (l, v) in &e.facts {
        if y > bottom {
            break;
        }
        page.text(l, MARGIN, label_w, y + 9.0, 9.0, true, INK);
        page.text(
            v,
            MARGIN + label_w + 10.0,
            full - label_w - 10.0,
            y + 9.0,
            9.0,
            false,
            INK,
        );
        y += 13.0;
    }
    y += 8.0;
    // The measurements.
    page.text(&r.table_heading, MARGIN, full, y + 10.0, 10.0, true, INK);
    y += 16.0;
    if e.table.len() <= 1 {
        page.text(&e.no_table, MARGIN, full, y + 9.0, 9.0, false, MUTED);
        return y + 14.0;
    }
    let cols = e.table.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0.0f64; cols];
    for (i, row) in e.table.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            widths[c] = widths[c].max(page.pen.width(cell, 9.0, i == 0) + 10.0);
        }
    }
    let total: f64 = widths.iter().sum();
    if total > full {
        for w in &mut widths {
            *w *= full / total;
        }
    }
    let row_h = 14.0;
    for (i, row) in e.table.iter().enumerate() {
        if y + row_h > bottom {
            break;
        }
        if i == 0 {
            page.fill(BAND);
            page.pen.op(&format!(
                "{} {} {} {} re f",
                num(MARGIN),
                num(y),
                num(full),
                num(row_h)
            ));
        }
        let mut x = MARGIN;
        for (c, cell) in row.iter().enumerate() {
            page.text(cell, x + 4.0, widths[c] - 6.0, y + 10.0, 9.0, i == 0, INK);
            x += widths[c];
        }
        y += row_h;
        page.rule(MARGIN, full, y);
    }
    y + 6.0
}

/// The report as a PDF.
pub fn write(r: &Report, outlines: Option<&OutlineText<'_>>) -> Vec<u8> {
    let full = PAGE_W - 2.0 * MARGIN;
    let mut pages = Vec::new();
    for (i, e) in r.entries.iter().enumerate() {
        let mut page = Page::new(outlines, r.rtl);
        let mut y = MARGIN;
        if i == 0 {
            page.text(&r.title, MARGIN, full, y + 16.0, 18.0, true, INK);
            y += 26.0;
            if !r.subtitle.is_empty() {
                page.text(&r.subtitle, MARGIN, full, y + 9.0, 9.0, false, MUTED);
                y += 16.0;
            }
            if !r.notes.trim().is_empty() {
                page.text(&r.notes_heading, MARGIN, full, y + 10.0, 10.0, true, INK);
                y = page.paragraph(&r.notes, MARGIN, full, y + 24.0, 9.0, INK, 360.0) - 4.0;
            }
            page.rule(MARGIN, full, y + 4.0);
            y += 16.0;
        }
        entry(&mut page, r, e, y);
        if let Some(l) = r.page_labels.get(i) {
            page.text(l, MARGIN, full, PAGE_H - MARGIN + 10.0, 8.0, false, MUTED);
        }
        pages.push(page);
    }
    assemble(r, pages)
}

/// The file: catalog, pages, fonts, content streams and images.
fn assemble(r: &Report, pages: Vec<Page<'_>>) -> Vec<u8> {
    // 1 catalog, 2 pages, 3 info, 4 Helvetica, 5 Helvetica-Bold, then per page: the page,
    // its content and its images.
    let mut objects: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        Vec::new(),
        Vec::new(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
            .to_vec(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>"
            .to_vec(),
    ];
    let mut kids = Vec::new();
    for page in pages {
        let page_id = objects.len() + 1;
        let content_id = page_id + 1;
        kids.push(page_id);
        let mut xobjects = String::new();
        let mut image_objs = Vec::new();
        for (k, img) in page.images.iter().enumerate() {
            let id = content_id + 1 + k;
            let _ = write!(xobjects, "/Im{k} {id} 0 R ");
            image_objs.push(image_object(img));
        }
        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /Helv 4 0 R /HeBo 5 0 R >> /XObject << {xobjects}>> >> /Contents {content_id} 0 R >>",
                num(PAGE_W),
                num(PAGE_H)
            )
            .into_bytes(),
        );
        let data = miniz_oxide::deflate::compress_to_vec_zlib(&page.pen.0, 6);
        let mut content = format!(
            "<< /Length {} /Filter /FlateDecode >>\nstream\n",
            data.len()
        )
        .into_bytes();
        content.extend_from_slice(&data);
        content.extend_from_slice(b"\nendstream");
        objects.push(content);
        objects.extend(image_objs);
    }
    objects[1] = format!(
        "<< /Type /Pages /Kids [{}] /Count {} >>",
        kids.iter()
            .map(|k| format!("{k} 0 R"))
            .collect::<Vec<_>>()
            .join(" "),
        kids.len()
    )
    .into_bytes();
    let mut info = format!(
        "<< /Title {} /Producer {}",
        text_string(&r.title),
        text_string(&r.producer)
    );
    if let Some(d) = &r.created {
        let _ = write!(info, " /CreationDate ({d})");
    }
    info.push_str(" >>");
    objects[2] = info.into_bytes();
    let mut out = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info 3 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

/// An image XObject: RGB, Flate.
fn image_object(img: &Image) -> Vec<u8> {
    let data = miniz_oxide::deflate::compress_to_vec_zlib(&img.rgb, 6);
    let mut o = format!(
        "<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB \
         /BitsPerComponent 8 /Interpolate {} /Filter /FlateDecode /Length {} >>\nstream\n",
        img.width,
        img.height,
        // The colour scale is stretched smoothly; pictures keep their pixels.
        if img.height == 1 { "true" } else { "false" },
        data.len()
    )
    .into_bytes();
    o.extend_from_slice(&data);
    o.extend_from_slice(b"\nendstream");
    o
}
