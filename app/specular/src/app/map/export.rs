//! Map export: the map at a chosen scale (1 : N at a resolution), as PNG (with its
//! resolution recorded) or as a PDF page, or printed to scale. Under the map: the title,
//! the scale and coordinate system, a scale bar, a north arrow, a legend of the layers
//! shown (and of slope, aspect or weather colours) and the data's attribution.
//!
//! The map is drawn by the viewer's own renderers into tiles offscreen (the image or its
//! values with their look, the vector layers, the overlays), read back and written band by
//! band. At 1 : N, a metre on paper is N metres on the ground, at the map's centre.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use specular_geodesy::units::{self, Units};
use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_i18n::t;
use specular_print::layout::Paper;
use specular_theme::Color;

use super::services::{Button, Row, View};

/// The scales offered (1 : this).
pub(super) const SCALES: [f64; 12] = [
    500.0,
    1000.0,
    2500.0,
    5000.0,
    10_000.0,
    25_000.0,
    50_000.0,
    100_000.0,
    250_000.0,
    500_000.0,
    1_000_000.0,
    5_000_000.0,
];
/// The resolutions offered (dots per inch).
pub(super) const DPIS: [u32; 3] = [150, 300, 600];
/// The papers offered for PDF and printing.
pub(super) const PAPERS: [Paper; 3] = [Paper::A4, Paper::Letter, Paper::A3];
/// Pixels an export may have at most.
pub(super) const MAX_PIXELS: u64 = 200_000_000;
/// The longest side, pixels.
pub(super) const MAX_SIDE: u32 = 20_000;
/// The page margin, mm.
const MARGIN_MM: f64 = 10.0;
/// The furniture under the map, at 96 dpi (logical px).
pub(super) const STRIP: f32 = 92.0;
const MM_PER_IN: f64 = 25.4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Format {
    Png,
    Pdf,
}

/// The export panel's choices.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct Panel {
    /// Printing (a PDF page handed to the system) rather than saving.
    pub print: bool,
    pub format: Format,
    pub scale: f64,
    pub dpi: u32,
    pub paper: Paper,
    /// The row the keys change (format, scale, resolution, paper).
    pub row: usize,
    /// Why the last export failed, or that it is under way.
    pub status: Option<String>,
}

impl Panel {
    /// A panel starting at the scale nearest the screen's.
    pub(super) fn new(print: bool, screen_scale: f64) -> Panel {
        let scale = SCALES
            .iter()
            .copied()
            .min_by(|a, b| {
                (a.ln() - screen_scale.ln())
                    .abs()
                    .total_cmp(&(b.ln() - screen_scale.ln()).abs())
            })
            .unwrap_or(25_000.0);
        let locale = specular_i18n::I18n::global()
            .localizer()
            .format_language()
            .clone();
        Panel {
            print,
            format: if print { Format::Pdf } else { Format::Png },
            scale,
            dpi: 300,
            paper: Paper::for_locale(&locale.to_string()),
            row: 0,
            status: None,
        }
    }

    fn rows(&self) -> Vec<usize> {
        // Printing is always a PDF page: no format row.
        let mut v = if self.print {
            vec![1, 2, 3]
        } else {
            vec![0, 1, 2]
        };
        if !self.print && self.format == Format::Pdf {
            v.push(3);
        }
        v
    }

    /// Changes the chosen row's value by `d` steps.
    pub(super) fn change(&mut self, d: isize) {
        let rows = self.rows();
        let which = rows.get(self.row.min(rows.len() - 1)).copied().unwrap_or(0);
        let step = |i: usize, n: usize| (i as isize + d).clamp(0, n as isize - 1) as usize;
        match which {
            0 => {
                self.format = if self.format == Format::Png {
                    Format::Pdf
                } else {
                    Format::Png
                }
            }
            1 => {
                let i = SCALES.iter().position(|s| *s == self.scale).unwrap_or(5);
                self.scale = SCALES[step(i, SCALES.len())];
            }
            2 => {
                let i = DPIS.iter().position(|s| *s == self.dpi).unwrap_or(1);
                self.dpi = DPIS[step(i, DPIS.len())];
            }
            _ => {
                let i = PAPERS.iter().position(|p| *p == self.paper).unwrap_or(0);
                self.paper = PAPERS[step(i, PAPERS.len())];
            }
        }
        self.status = None;
    }

    pub(super) fn move_row(&mut self, d: isize) {
        let n = self.rows().len();
        self.row = (self.row as isize + d).clamp(0, n as isize - 1) as usize;
    }

    /// The panel's view; `size` is the export's size in pixels and ground metres.
    pub(super) fn view(&self, size: Option<Layout>, focused: bool) -> View {
        let title = if self.print {
            t!("map-print-title")
        } else {
            t!("map-export-title")
        };
        let mut lines = vec![t!("map-export-keys")];
        if let Some(l) = size {
            lines.push(t!(
                "map-export-size",
                width = i64::from(l.px.0),
                height = i64::from(l.px.1),
                across = units::format_value(l.ground.0 / 1000.0),
                down = units::format_value(l.ground.1 / 1000.0)
            ));
        }
        lines.extend(self.status.clone());
        let rows = self
            .rows()
            .into_iter()
            .map(|r| match r {
                0 => Row {
                    title: t!("map-export-format"),
                    sub: match self.format {
                        Format::Png => "PNG".to_owned(),
                        Format::Pdf => "PDF".to_owned(),
                    },
                    enabled: true,
                },
                1 => Row {
                    title: t!("map-export-scale"),
                    sub: scale_text(self.scale),
                    enabled: true,
                },
                2 => Row {
                    title: t!("map-export-resolution"),
                    sub: t!("map-export-dpi", dpi = i64::from(self.dpi)),
                    enabled: true,
                },
                _ => Row {
                    title: t!("map-export-paper"),
                    sub: self.paper.name().unwrap_or("").to_owned(),
                    enabled: true,
                },
            })
            .collect();
        View {
            title,
            lines,
            field: None,
            rows,
            cursor: self.row,
            scroll: 0,
            buttons: vec![
                if self.print {
                    Button::Print
                } else {
                    Button::Save
                },
                Button::Close,
            ],
            focused,
        }
    }
}

/// "1:25,000" with the locale's grouping.
pub(super) fn scale_text(n: f64) -> String {
    let digits = format!("{:.0}", n.round());
    let mut g = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            g.push(',');
        }
        g.push(c);
    }
    t!("map-export-scale-value", n = g)
}

/// Where everything goes in an export (output pixels).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Layout {
    /// The whole image or page.
    pub px: (u32, u32),
    /// The map frame inside it: x, y, w, h.
    pub frame: (u32, u32, u32, u32),
    /// The strip of furniture under the map: x, y, w, h.
    pub strip: (u32, u32, u32, u32),
    /// The page in PDF points, for PDF.
    pub page_pt: Option<(f64, f64)>,
    pub landscape: bool,
    /// The map frame's ground size, metres (across, down).
    pub ground: (f64, f64),
    /// Output pixels per stored pixel.
    pub zoom: f64,
}

/// Lays out an export at 1 : `scale` and `dpi`: a PNG of the area of `view_stored` (the
/// part of the map on screen, stored px), or a PDF page of `paper` whose map frame shows
/// what fits around the centre. `m_stored`: ground metres per stored pixel at the centre.
pub(super) fn layout(p: &Panel, view_stored: (f64, f64), m_stored: f64) -> Result<Layout, String> {
    if !(m_stored.is_finite() && m_stored > 0.0) {
        return Err(t!("map-export-no-scale"));
    }
    let px_per_mm = f64::from(p.dpi) / MM_PER_IN;
    // Ground metres per output pixel.
    let m_out = p.scale * MM_PER_IN / 1000.0 / f64::from(p.dpi);
    let zoom = m_stored / m_out;
    let strip_h = (f64::from(STRIP) * f64::from(p.dpi) / 96.0).round() as u32;
    let l = if p.format == Format::Png && !p.print {
        let w = (view_stored.0 * zoom).round().max(1.0);
        let h = (view_stored.1 * zoom).round().max(1.0);
        if w > f64::from(MAX_SIDE) || h + f64::from(strip_h) > f64::from(MAX_SIDE) {
            return Err(t!(
                "map-export-too-large",
                width = w as i64,
                height = h as i64
            ));
        }
        let (w, h) = (w as u32, h as u32);
        Layout {
            px: (w, h + strip_h),
            frame: (0, 0, w, h),
            strip: (0, h, w, strip_h),
            page_pt: None,
            landscape: w > h,
            ground: (f64::from(w) * m_out, f64::from(h) * m_out),
            zoom,
        }
    } else {
        let (pw, ph) = p.paper.size_mm();
        let landscape = view_stored.0 > view_stored.1;
        let (mw, mh) = if landscape { (ph, pw) } else { (pw, ph) };
        let px = (
            (mw * px_per_mm).round() as u32,
            (mh * px_per_mm).round() as u32,
        );
        let m = (MARGIN_MM * px_per_mm).round() as u32;
        let fw = px.0.saturating_sub(2 * m);
        let fh = px.1.saturating_sub(2 * m + strip_h);
        Layout {
            px,
            frame: (m, m, fw, fh),
            strip: (m, m + fh, fw, strip_h),
            page_pt: Some((mw / MM_PER_IN * 72.0, mh / MM_PER_IN * 72.0)),
            landscape,
            ground: (f64::from(fw) * m_out, f64::from(fh) * m_out),
            zoom,
        }
    };
    if u64::from(l.px.0) * u64::from(l.px.1) > MAX_PIXELS {
        return Err(t!(
            "map-export-too-large",
            width = i64::from(l.px.0),
            height = i64::from(l.px.1)
        ));
    }
    Ok(l)
}

/// What the furniture strip shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Furniture {
    pub title: String,
    /// "Scale 1:25,000 · WGS 84 / UTM zone 32N".
    pub subtitle: String,
    /// The scale bar: its length on paper (logical px at 96 dpi) and label.
    pub bar: Option<(f32, String)>,
    /// North's angle clockwise from up, radians.
    pub north: Option<f64>,
    /// (colour, name) of the layers shown.
    pub legend: Vec<([u8; 3], String)>,
    /// A colour ramp (slope, aspect, weather) with its title and end labels.
    pub ramp: Option<super::terrain::Legend>,
    pub attribution: String,
}

/// A scale bar for the export: a round length of at most a fifth of the strip's width.
pub(super) fn bar_for(m_per_logical: f64, strip_w: f32, u: Units) -> Option<(f32, String)> {
    let c = units::scale_bar(m_per_logical * f64::from(strip_w) * 0.22, u)?;
    let px = (c.metres / m_per_logical) as f32;
    Some((
        px,
        t!(
            "map-length",
            value = units::format_value(c.value),
            unit = specular_i18n::unit_symbol(c.unit.symbol())
        ),
    ))
}

/// Draws the furniture into `strip` (logical px of the output).
pub(super) fn draw_furniture(
    dl: &mut DrawList,
    text: &mut Text,
    f: &Furniture,
    strip: Rect,
    rtl: bool,
) {
    let ink = Color::hex(0x1a1a1a);
    let soft = Color::hex(0x5a5a5a);
    let pad = 8.0;
    dl.rect(strip, 0.0, Color::hex(0xffffff));
    dl.line((strip.x, strip.y), (strip.x + strip.w, strip.y), 0.75, soft);
    let w3 = strip.w / 3.0;
    // Start: title, scale and CRS, attribution.
    let start = Rect::new(
        if rtl { strip.x + 2.0 * w3 } else { strip.x },
        strip.y,
        w3,
        strip.h,
    );
    let at = |r: Rect, bw: f32| if rtl { r.x + r.w - pad - bw } else { r.x + pad };
    let mut y = strip.y + pad;
    for (s, style, c) in [
        (&f.title, TextStyle::new(13.0).weight(650.0), ink),
        (&f.subtitle, TextStyle::new(10.0), ink),
        (&f.attribution, TextStyle::new(8.5), soft),
    ] {
        if s.is_empty() {
            continue;
        }
        let b = text.layout(s, style, Some(w3 - 2.0 * pad), Align::Start);
        text.draw(dl, &b, at(start, b.width), y, c);
        y += b.height + 3.0;
    }
    // Middle: the scale bar and the north arrow.
    let mid = Rect::new(strip.x + w3, strip.y, w3, strip.h);
    if let Some((len, label)) = &f.bar {
        let x0 = mid.x + (mid.w - len) / 2.0;
        let yb = mid.y + mid.h * 0.62;
        let h = 5.0;
        dl.rect(Rect::new(x0, yb, len / 2.0, h), 0.0, ink);
        dl.stroke_rect(Rect::new(x0, yb, *len, h), 0.0, ink, 0.75);
        let b = text.layout(label, TextStyle::new(9.0).tabular(), None, Align::Start);
        text.draw(dl, &b, x0 + len - b.width / 2.0, yb + h + 2.0, ink);
        let z = text.layout("0", TextStyle::new(9.0).tabular(), None, Align::Start);
        text.draw(dl, &z, x0 - z.width / 2.0, yb + h + 2.0, ink);
    }
    if let Some(a) = f.north {
        let c = (mid.x + mid.w / 2.0, mid.y + mid.h * 0.3);
        let (s, co) = (a.sin() as f32, a.cos() as f32);
        let pt = |along: f32, across: f32| {
            (c.0 + along * s + across * co, c.1 - along * co + across * s)
        };
        let tip = pt(14.0, 0.0);
        dl.line(pt(-10.0, 0.0), tip, 1.5, ink);
        dl.line(pt(4.0, -5.0), tip, 1.5, ink);
        dl.line(pt(4.0, 5.0), tip, 1.5, ink);
        let n = text.layout("N", TextStyle::new(9.0).weight(700.0), None, Align::Start);
        let q = pt(22.0, 0.0);
        text.draw(dl, &n, q.0 - n.width / 2.0, q.1 - n.height / 2.0, ink);
    }
    // End: the legend.
    let end = Rect::new(
        if rtl { strip.x } else { strip.x + 2.0 * w3 },
        strip.y,
        w3,
        strip.h,
    );
    let mut y = end.y + pad;
    let st = TextStyle::new(9.0);
    if let Some(r) = &f.ramp {
        let tb = text.layout(
            &r.title,
            st.weight(600.0),
            Some(w3 - 2.0 * pad),
            Align::Start,
        );
        text.draw(dl, &tb, at(end, tb.width), y, ink);
        y += tb.height + 2.0;
        let rw = w3 - 2.0 * pad;
        let n = r.ramp.len().max(1);
        for (i, c) in r.ramp.iter().enumerate() {
            let k = if rtl { n - 1 - i } else { i };
            dl.rect(
                Rect::new(
                    end.x + pad + rw * k as f32 / n as f32,
                    y,
                    rw / n as f32 + 0.5,
                    7.0,
                ),
                0.0,
                Color::rgba(c[0], c[1], c[2], 1.0),
            );
        }
        y += 8.0;
        if let (Some(a), Some(b)) = (r.ticks.first(), r.ticks.last()) {
            let la = text.layout(&a.1, st.tabular(), None, Align::Start);
            let lb = text.layout(&b.1, st.tabular(), None, Align::Start);
            let (left, right) = if rtl { (&lb, &la) } else { (&la, &lb) };
            text.draw(dl, left, end.x + pad, y, soft);
            text.draw(dl, right, end.x + end.w - pad - right.width, y, soft);
            y += la.height + 3.0;
        }
    }
    for (c, name) in f.legend.iter().take(6) {
        let b = text.layout(name, st, Some(w3 - 2.0 * pad - 16.0), Align::Start);
        if y + b.height > end.y + end.h - 2.0 {
            break;
        }
        let sw = Rect::new(
            if rtl {
                end.x + end.w - pad - 10.0
            } else {
                end.x + pad
            },
            y + 1.0,
            10.0,
            8.0,
        );
        dl.rect(
            sw,
            1.0,
            Color::rgba(
                f32::from(c[0]) / 255.0,
                f32::from(c[1]) / 255.0,
                f32::from(c[2]) / 255.0,
                1.0,
            ),
        );
        let tx = if rtl {
            sw.x - 6.0 - b.width
        } else {
            sw.x + sw.w + 6.0
        };
        text.draw(dl, &b, tx, y, ink);
        y += b.height + 2.0;
    }
}

/// A PNG of `rgba` with its resolution recorded (`pHYs`, pixels per metre), so it prints
/// at the scale it was made for.
pub(super) fn png_with_dpi(w: u32, h: u32, rgba: &[u8], dpi: u32) -> Result<Vec<u8>, String> {
    let png = specular_png::encode(w, h, specular_png::ColorType::Rgba, rgba)
        .map_err(|e| e.to_string())?;
    // After the signature (8) and IHDR (8 + 13 + 4).
    let at = 8 + 25;
    if png.len() < at {
        return Err("PNG too short".into());
    }
    let ppm = (f64::from(dpi) / 0.0254).round() as u32;
    let mut chunk = Vec::with_capacity(21);
    chunk.extend_from_slice(&9u32.to_be_bytes());
    let mut body = b"pHYs".to_vec();
    body.extend_from_slice(&ppm.to_be_bytes());
    body.extend_from_slice(&ppm.to_be_bytes());
    body.push(1);
    chunk.extend_from_slice(&body);
    chunk.extend_from_slice(&crc32(&body).to_be_bytes());
    let mut out = Vec::with_capacity(png.len() + chunk.len());
    out.extend_from_slice(&png[..at]);
    out.extend_from_slice(&chunk);
    out.extend_from_slice(&png[at..]);
    Ok(out)
}

/// CRC-32 (ISO 3309), as PNG chunks carry.
fn crc32(b: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for &x in b {
        c ^= u32::from(x);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xedb8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}

/// Writes a finished export: PNG bytes, or a one-page PDF of the RGB rows.
pub(super) fn write_file(
    path: &Path,
    l: &Layout,
    rgba: &[u8],
    dpi: u32,
    title: &str,
    pdf: bool,
) -> Result<(), String> {
    let (w, h) = l.px;
    if pdf {
        let f = std::fs::File::create(path).map_err(|e| e.to_string())?;
        let meta = specular_print::pdf::Meta {
            title: title.to_owned(),
            date: String::new(),
            ..Default::default()
        };
        let mut wr = specular_print::pdf::PdfWriter::new(std::io::BufWriter::new(f), &meta)
            .map_err(|e| e.to_string())?;
        wr.begin_page(l.page_pt.unwrap_or((f64::from(w), f64::from(h))), w, h)
            .map_err(|e| e.to_string())?;
        let row = w as usize;
        let mut band = Vec::with_capacity(row * 3 * 256);
        for rows in rgba.chunks(row * 4 * 256) {
            band.clear();
            for p in rows.chunks_exact(4) {
                band.extend_from_slice(&p[..3]);
            }
            wr.write_rows(&band).map_err(|e| e.to_string())?;
        }
        wr.end_page().map_err(|e| e.to_string())?;
        let (mut out, _) = wr.finish().map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())?;
        Ok(())
    } else {
        let bytes = png_with_dpi(w, h, rgba, dpi)?;
        super::super::gallery::marks::write_atomic(path, &bytes).map_err(|e| e.to_string())
    }
}

/// How an export ended.
#[derive(Debug)]
pub(in crate::app) enum Done {
    /// A file name was chosen: export into it.
    Chosen(PathBuf),
    /// Saved to this file.
    Saved(PathBuf),
    /// Handed to the system's printing (or why not).
    Printed(Result<(), String>),
    /// The dialog was closed.
    Cancelled,
}

// ----- The app's side ------------------------------------------------------------------------

use specular_gfx::{Compositor, UiRenderer, wgpu};
use specular_input::{Key, KeyCombo, NamedKey};
use specular_render::{PxRect, RenderOptions, View as RenderView};

use super::super::App;
use super::services::Hit;
use super::{Focus, Job, terrain, view as map_view, weather};

/// Tiles drawn at once, pixels a side at most.
const TILE: u32 = 2048;
/// The output format of the final pass (8-bit, sRGB-encoded by the shader).
const OUT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

impl App {
    /// The export's layout for the panel's choices and the view now.
    pub(super) fn gis_export_layout(&self) -> Option<Result<Layout, String>> {
        let p = self.map.t.export.as_ref()?;
        let geo = self.map.geo.as_ref()?;
        let c = self.gis_centre();
        let [ex, ny] = terrain::ground_steps(geo.as_ref(), c.0, c.1)?;
        let m_stored = (ex.abs() + ny.abs()) / 2.0;
        // The part of the map on screen (display px).
        let (x0, y0, x1, y1) = self.view.visible_display();
        Some(layout(
            p,
            (f64::from(x1 - x0).max(1.0), f64::from(y1 - y0).max(1.0)),
            m_stored,
        ))
    }

    pub(super) fn gis_export_key(&mut self, c: &KeyCombo) -> bool {
        let rtl = self.rtl();
        let Some(p) = self.map.t.export.as_mut() else {
            return false;
        };
        let fwd = if rtl { -1 } else { 1 };
        match c.key {
            Key::Named(NamedKey::ArrowDown) => p.move_row(1),
            Key::Named(NamedKey::ArrowUp) => p.move_row(-1),
            Key::Named(NamedKey::ArrowRight) => p.change(fwd),
            Key::Named(NamedKey::ArrowLeft) => p.change(-fwd),
            Key::Named(NamedKey::Enter) => self.gis_export_go(),
            _ => return false,
        }
        true
    }

    pub(super) fn gis_export_hit(&mut self, h: Hit) {
        let Some(p) = self.map.t.export.as_mut() else {
            return;
        };
        match h {
            Hit::Row(i) => {
                if p.row == i {
                    p.change(1);
                }
                p.row = i;
            }
            Hit::Button(Button::Save | Button::Print) => self.gis_export_go(),
            Hit::Button(_) => {
                self.map.t.export = None;
                self.map.focus = None;
            }
            Hit::Field => {}
        }
        self.redraw = true;
    }

    /// Save…: a file name first (on the dialog's thread); Print…: straight to the system.
    fn gis_export_go(&mut self) {
        let Some(Ok(l)) = self.gis_export_layout() else {
            if let (Some(Err(e)), Some(p)) = (self.gis_export_layout(), self.map.t.export.as_mut())
            {
                p.status = Some(e);
            }
            return;
        };
        let Some(p) = self.map.t.export.clone() else {
            return;
        };
        let stem = self
            .current_id()
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.path.file_stem().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "map".to_owned());
        if p.print {
            let path = specular_settings::Locations::detect()
                .cache_dir
                .join(format!("specular-map-print-{}.pdf", std::process::id()));
            if let Err(e) = self.gis_export_write(&path, &l, &p, &stem) {
                if let Some(p) = self.map.t.export.as_mut() {
                    p.status = Some(e);
                }
                return;
            }
            let (pw, ph) = p.paper.size_mm();
            let job = specular_platform::print::Job {
                title: stem,
                copies: 1,
                paper: specular_platform::print::Paper {
                    pwg: p.paper.pwg_name(),
                    display: p.paper.name().unwrap_or("").to_owned(),
                    width_mm: pw,
                    height_mm: ph,
                },
                landscape: l.landscape,
            };
            let tx = self.map.channel();
            if let Some(p) = self.map.t.export.as_mut() {
                p.status = Some(t!("map-export-printing"));
            }
            self.loader.background("map-print", move |lo| {
                let r = specular_platform::print::print(&path, &job)
                    .map(|_| ())
                    .map_err(|e| e.to_string());
                let _ = std::fs::remove_file(&path);
                let _ = tx.send(Job::Exported(Done::Printed(r)));
                lo.waker()();
            });
            return;
        }
        let pdf = p.format == Format::Pdf;
        let name = format!(
            "{stem}-{}.{}",
            p.scale as u64,
            if pdf { "pdf" } else { "png" }
        );
        let (title, filter) = (t!("palette-map-export"), if pdf { "PDF" } else { "PNG" });
        let ext = if pdf { "pdf" } else { "png" };
        let tx = self.map.channel();
        self.loader.background("dlg-map-export", move |lo| {
            let r = rfd::FileDialog::new()
                .set_title(&title)
                .set_file_name(&name)
                .add_filter(filter, &[ext])
                .save_file();
            let _ = tx.send(Job::Exported(r.map_or(Done::Cancelled, Done::Chosen)));
            lo.waker()();
        });
    }

    /// An export ended (or its file was chosen).
    pub(super) fn gis_exported(&mut self, d: Done) {
        let now = self.now();
        match d {
            Done::Chosen(path) => {
                let (Some(Ok(l)), Some(p)) = (self.gis_export_layout(), self.map.t.export.clone())
                else {
                    return;
                };
                let stem = path
                    .file_stem()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                match self.gis_export_write(&path, &l, &p, &stem) {
                    Ok(()) => self.gis_exported(Done::Saved(path)),
                    Err(e) => {
                        if let Some(p) = self.map.t.export.as_mut() {
                            p.status = Some(e);
                        }
                    }
                }
            }
            Done::Saved(path) => {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.chrome
                    .toast(t!("map-export-saved", name = name), None, now);
                self.map.t.export = None;
                if self.map.focus == Some(Focus::Export) {
                    self.map.focus = None;
                }
            }
            Done::Printed(Ok(())) => {
                self.chrome.toast(t!("map-export-printed"), None, now);
                self.map.t.export = None;
                self.map.focus = None;
            }
            Done::Printed(Err(e)) => {
                if let Some(p) = self.map.t.export.as_mut() {
                    p.status = Some(t!("map-export-print-failed", reason = e));
                }
            }
            Done::Cancelled => {}
        }
        self.redraw = true;
    }

    /// The GPU device was replaced: the export's compositor and UI renderer are made again
    /// at the next export.
    pub(in crate::app) fn gis_export_gpu_reset(&mut self) {
        self.map.t.export_kit = None;
    }

    /// Renders the export and writes it to `path`.
    fn gis_export_write(
        &mut self,
        path: &Path,
        l: &Layout,
        p: &Panel,
        title: &str,
    ) -> Result<(), String> {
        let t0 = std::time::Instant::now();
        // A GPU reset on the way: once more on the new device.
        let rgba = self.with_gpu_retry(|app| app.gis_export_render(l, p, title))?;
        let pdf = p.print || p.format == Format::Pdf;
        write_file(path, l, &rgba, p.dpi, title, pdf)?;
        specular_log::debug!(target: specular_log::UI, "map export {}×{} in {:.0} ms", l.px.0, l.px.1, t0.elapsed().as_secs_f64() * 1e3);
        Ok(())
    }

    /// The furniture of an export at `l`.
    fn gis_export_furniture(&self, l: &Layout, p: &Panel, title: &str, dl_scale: f32) -> Furniture {
        let units = self.gis_units();
        let m_out = p.scale * MM_PER_IN / 1000.0 / f64::from(p.dpi);
        let m_logical = m_out * f64::from(dl_scale);
        let crs = self
            .map
            .geo
            .as_ref()
            .map(|g| g.crs_name())
            .unwrap_or_default();
        let subtitle = if crs.is_empty() {
            scale_text(p.scale)
        } else {
            t!(
                "map-export-subtitle",
                scale = scale_text(p.scale),
                crs = crs
            )
        };
        let mut legend: Vec<([u8; 3], String)> = self
            .map
            .layers
            .iter()
            .filter(|ly| ly.visible && ly.feature_count() > 0)
            .map(|ly| (ly.ink(), ly.shown_name()))
            .collect();
        legend.reverse();
        let ramp = self
            .science_relief()
            .and_then(|h| terrain::relief_legend(h.kind))
            .or_else(|| {
                let m = self.science_weather()?;
                Some(terrain::Legend {
                    title: self
                        .science_position()
                        .and_then(|(s, i)| {
                            s.dataset
                                .clone()
                                .or_else(|| s.unit_at(&i).map(str::to_owned))
                        })
                        .unwrap_or_default(),
                    ramp: (0..=32).map(|i| m.sample(i as f32 / 32.0)).collect(),
                    ticks: Vec::new(),
                })
            });
        let mut sources: Vec<String> = vec![title.to_owned()];
        sources.extend(
            self.map
                .layers
                .iter()
                .filter(|ly| ly.visible && ly.added.is_some())
                .map(|ly| ly.shown_name()),
        );
        sources.dedup();
        let north = {
            // North from the map's centre on the export (its orientation is the screen's).
            self.gis_north().or(Some(0.0))
        };
        Furniture {
            title: title.to_owned(),
            subtitle,
            bar: bar_for(m_logical, l.strip.2 as f32 / dl_scale, units),
            north,
            legend,
            ramp,
            attribution: t!("map-export-data", sources = sources.join(" · ")),
        }
    }

    /// Draws the export in tiles on the GPU and reads it back as RGBA.
    fn gis_export_render(&mut self, l: &Layout, p: &Panel, title: &str) -> Result<Vec<u8>, String> {
        let furniture = self.gis_export_furniture(l, p, title, f64::from(p.dpi) as f32 / 96.0);
        let dl_scale = p.dpi as f32 / 96.0;
        let id = self.map.id;
        let centre = {
            let (x0, y0, x1, y1) = self.view.visible_display();
            ((x0 + x1) / 2.0, (y0 + y1) / 2.0)
        };
        let (stored, orient) = (self.view.stored_size(), self.view.orientation());
        let opts = {
            let mut o = RenderOptions {
                hq: true,
                canvas: [1.0; 4],
                checker_a: [1.0; 4],
                checker_b: [1.0; 4],
                grid: None,
                nearest_from: f32::INFINITY,
                ..RenderOptions::default()
            };
            super::super::inspect::apply_display_aids(&mut o, &self.chrome, &self.settings);
            o
        };
        let frame_px = PxRect {
            x: l.frame.0 as f32,
            y: l.frame.1 as f32,
            w: l.frame.2 as f32,
            h: l.frame.3 as f32,
        };
        let full_view = RenderView::fixed(stored, orient, frame_px, l.zoom as f32, centre);
        let vector = self.map.vector;
        let has_layers = !self.map.layers.is_empty() && self.map.scene.is_some();
        let map_frame = id.map(|id| map_view::MapFrame {
            id,
            key: 0,
            dark: false,
            paper: vector,
        });
        // The overlays in output logical px (the whole export's coordinates).
        let to_logical = |v: &RenderView, s: (f64, f64)| {
            let m = v.screen_from_stored_f64();
            let k = f64::from(dl_scale);
            (
                ((m[0] * s.0 + m[1] * s.1 + m[2]) / k) as f32,
                ((m[3] * s.0 + m[4] * s.1 + m[5]) / k) as f32,
            )
        };
        let frame_logical = Rect::new(
            frame_px.x / dl_scale,
            frame_px.y / dl_scale,
            frame_px.w / dl_scale,
            frame_px.h / dl_scale,
        );
        let contours = self
            .map
            .t
            .contours
            .as_ref()
            .filter(|_| self.map.t.contours_on)
            .map(|c| terrain::view_of(c, |s| to_logical(&full_view, s), frame_logical));
        let arrows = match (&self.map.t.wind, self.map.geo.clone()) {
            (Some((_, u, v)), Some(geo)) if self.map.t.wind_on => {
                let inv = full_view.stored_from_screen();
                let size = self.map.size;
                weather::arrows(
                    u,
                    v,
                    size,
                    geo.as_ref(),
                    frame_logical,
                    |q| {
                        let (x, y) = inv.apply((q.0 * dl_scale, q.1 * dl_scale));
                        let s = (f64::from(x), f64::from(y));
                        (s.0 >= 0.0 && s.1 >= 0.0 && s.0 <= size.0 && s.1 <= size.1).then_some(s)
                    },
                    |s| to_logical(&full_view, s),
                    weather::ARROW_SPACING,
                )
            }
            _ => Vec::new(),
        };
        let pins: Vec<((f32, f32), Option<f64>)> = match (&self.map.t.placed, self.map.geo.clone())
        {
            (Some(pl), Some(geo)) if self.map.t.photos_on => pl
                .iter()
                .filter_map(|(_, lat, lon, d)| {
                    let s = geo.from_lonlat(*lon, *lat)?;
                    Some((to_logical(&full_view, s), *d))
                })
                .collect(),
            _ => Vec::new(),
        };
        let rtl = self.rtl();
        let gfx = self.gfx.as_mut().ok_or_else(|| t!("map-export-no-gpu"))?;
        let gpu = gfx.gpu.clone();
        let img = id.and_then(|id| self.gpu_images.get(id)).map(|g| &*g);
        let scene = Scene {
            stored,
            orient,
            centre,
            img: img.filter(|_| !vector),
            opts,
            frame: map_frame.filter(|_| has_layers),
            contours,
            arrows,
            pins,
            furniture,
            rtl,
            dpi: p.dpi,
        };
        // The export's own text system (the window's draws the frame), kept for the next.
        let mut kit = self.map.t.export_kit.take().unwrap_or_else(|| {
            let t = Text::new();
            let t0 = std::time::Instant::now();
            while t.fonts_pending() && t0.elapsed() < std::time::Duration::from_secs(5) {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Kit::new(&gpu, t)
        });
        let r = render_tiles(
            &gpu,
            &mut gfx.renderer,
            &mut self.map,
            self.cad.renderer_mut(),
            l,
            &scene,
            &mut kit,
            TILE,
        );
        self.map.t.export_kit = Some(kit);
        self.map.bump();
        r
    }
}

/// What exports draw with, kept from one to the next: the compositor, the UI renderer and
/// the text system (glyphs cached in one live in the other's atlas, so they go together).
pub(in crate::app) struct Kit {
    comp: Compositor,
    ui: UiRenderer,
    text: Text,
    tile: (u32, u32),
}

impl Kit {
    pub(super) fn new(gpu: &specular_gfx::Gpu, text: Text) -> Kit {
        let tile = (256, 256);
        let comp = Compositor::new(gpu, OUT, 16, tile.0, tile.1);
        let ui = UiRenderer::new(gpu, &comp);
        Kit {
            comp,
            ui,
            text,
            tile,
        }
    }

    fn set_tile(&mut self, gpu: &specular_gfx::Gpu, tile: (u32, u32)) {
        if tile != self.tile {
            self.tile = tile;
            self.comp.resize(gpu, tile.0, tile.1);
            self.ui.compositor_resized(gpu, &self.comp);
        }
    }
}

/// What an export draws, besides its layout.
pub(super) struct Scene<'a> {
    pub stored: (u32, u32),
    pub orient: specular_core::Orientation,
    /// The display point at the map frame's centre.
    pub centre: (f32, f32),
    /// The image (or its values with their look), unless the map is drawn by its layers.
    pub img: Option<&'a specular_render::GpuImage>,
    pub opts: RenderOptions,
    /// The vector layers (the map's scene), when there are any.
    pub frame: Option<map_view::MapFrame>,
    /// Overlays in output logical px.
    pub contours: Option<terrain::ContourView>,
    pub arrows: Vec<weather::Arrow>,
    pub pins: Vec<((f32, f32), Option<f64>)>,
    pub furniture: Furniture,
    pub rtl: bool,
    pub dpi: u32,
}

/// Draws an export in tiles of at most `max_tile` pixels a side and reads it back as
/// opaque RGBA: the image through `images` (the viewer's renderer, which has the image's
/// values and look), the map's vector layers through `vectors`, then the overlays and the
/// furniture.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_tiles(
    gpu: &specular_gfx::Gpu,
    images: &mut specular_render::ImageRenderer,
    map: &mut super::Map,
    mut vectors: Option<&mut specular_render::vector::VectorRenderer>,
    l: &Layout,
    sc: &Scene,
    kit: &mut Kit,
    max_tile: u32,
) -> Result<Vec<u8>, String> {
    let (w, h) = l.px;
    let dl_scale = sc.dpi as f32 / 96.0;
    let max = gpu.max_texture_size().min(max_tile).max(16);
    let tile = (w.min(max), h.min(max));
    kit.set_tile(gpu, tile);
    let Kit { comp, ui, text, .. } = kit;
    let out = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("map export tile"),
        size: wgpu::Extent3d {
            width: tile.0,
            height: tile.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OUT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let out_view = out.create_view(&Default::default());
    let pitch = (tile.0 * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("map export readback"),
        size: u64::from(pitch) * u64::from(tile.1),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let frame_px = PxRect {
        x: l.frame.0 as f32,
        y: l.frame.1 as f32,
        w: l.frame.2 as f32,
        h: l.frame.3 as f32,
    };
    let frame_logical = Rect::new(
        frame_px.x / dl_scale,
        frame_px.y / dl_scale,
        frame_px.w / dl_scale,
        frame_px.h / dl_scale,
    );
    let strip = Rect::new(
        l.strip.0 as f32 / dl_scale,
        l.strip.1 as f32 / dl_scale,
        l.strip.2 as f32 / dl_scale,
        l.strip.3 as f32 / dl_scale,
    );
    let mut rgba = vec![255u8; w as usize * h as usize * 4];
    text.begin_frame(dl_scale);
    let mut y0 = 0u32;
    while y0 < h {
        let rows = (h - y0).min(tile.1);
        let mut x0 = 0u32;
        while x0 < w {
            let cols = (w - x0).min(tile.0);
            let mut enc = gpu.device.create_command_encoder(&Default::default());
            drop(enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("map export clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &comp.scene.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            }));
            let vp = PxRect {
                x: frame_px.x - x0 as f32,
                y: frame_px.y - y0 as f32,
                w: frame_px.w,
                h: frame_px.h,
            };
            let tv = RenderView::fixed(sc.stored, sc.orient, vp, l.zoom as f32, sc.centre);
            if let Some(g) = sc.img {
                images.render_inset(
                    gpu,
                    &mut enc,
                    &comp.scene.view,
                    tile,
                    g,
                    &tv,
                    &sc.opts,
                    None,
                );
            }
            if let (Some(mf), Some(r)) = (sc.frame.as_ref(), vectors.as_deref_mut()) {
                map_view::render(
                    map,
                    r,
                    gpu,
                    &mut enc,
                    &comp.scene.view,
                    tile,
                    &tv,
                    mf,
                    f64::from(dl_scale),
                );
            }
            // The overlays and the furniture, shifted to the tile.
            let mut dl = DrawList::new(dl_scale);
            let (ox, oy) = (x0 as f32 / dl_scale, y0 as f32 / dl_scale);
            let shift = |p: (f32, f32)| (p.0 - ox, p.1 - oy);
            let clip = frame_logical.translate(-ox, -oy);
            if let Some(c) = &sc.contours {
                let shifted = terrain::ContourView {
                    lines: c
                        .lines
                        .iter()
                        .map(|(v, i)| (v.iter().map(|p| shift(*p)).collect(), *i))
                        .collect(),
                    labels: c
                        .labels
                        .iter()
                        .map(|(s, p)| (s.clone(), shift(*p)))
                        .collect(),
                };
                terrain::draw_contours(
                    &mut dl,
                    text,
                    &specular_theme::Palette::light(),
                    &shifted,
                    clip,
                    1.0,
                );
            }
            if !sc.arrows.is_empty() {
                let a: Vec<weather::Arrow> = sc
                    .arrows
                    .iter()
                    .map(|a| weather::Arrow {
                        tail: shift(a.tail),
                        head: shift(a.head),
                    })
                    .collect();
                weather::draw_arrows(&mut dl, &a, clip, 1.0);
            }
            for (pt, dir) in &sc.pins {
                super::view::draw_photo_pin(&mut dl, shift(*pt), *dir, 1.0);
            }
            let st = strip.translate(-ox, -oy);
            if st.y < tile.1 as f32 / dl_scale && st.y + st.h > 0.0 {
                draw_furniture(&mut dl, text, &sc.furniture, st, sc.rtl);
            }
            comp.begin(&mut enc);
            ui.render(gpu, &mut enc, &comp.comp.view, tile, &dl, text);
            comp.finish(&mut enc, &out_view);
            enc.copy_texture_to_buffer(
                out.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(pitch),
                        rows_per_image: Some(tile.1),
                    },
                },
                wgpu::Extent3d {
                    width: tile.0,
                    height: rows,
                    depth_or_array_layers: 1,
                },
            );
            gpu.queue.submit([enc.finish()]);
            let slice = readback.slice(..u64::from(pitch) * u64::from(rows));
            slice.map_async(wgpu::MapMode::Read, |_| {});
            gpu.wait().map_err(|e| format!("GPU readback: {e}"))?;
            {
                let data = slice
                    .get_mapped_range()
                    .map_err(|e| format!("GPU readback: {e}"))?;
                for (y, src) in data
                    .chunks_exact(pitch as usize)
                    .take(rows as usize)
                    .enumerate()
                {
                    let at = ((y0 as usize + y) * w as usize + x0 as usize) * 4;
                    rgba[at..at + cols as usize * 4].copy_from_slice(&src[..cols as usize * 4]);
                }
            }
            readback.unmap();
            x0 += cols;
        }
        y0 += rows;
    }
    text.end_frame();
    // Opaque: the paper is white.
    for px in rgba.chunks_exact_mut(4) {
        px[3] = 255;
    }
    Ok(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_at_a_scale() {
        let p = Panel {
            print: false,
            format: Format::Png,
            scale: 25_000.0,
            dpi: 300,
            paper: Paper::A4,
            row: 0,
            status: None,
        };
        // 10 m pixels, 1000 × 600 of them on screen: 10 km across.
        let l = layout(&p, (1000.0, 600.0), 10.0).unwrap();
        // 10 km at 1:25,000 is 40 cm: 4724 px at 300 dpi.
        assert!((i64::from(l.frame.2) - 4724).abs() <= 1, "{l:?}");
        assert!((l.ground.0 - 10_000.0).abs() < 5.0, "{l:?}");
        assert_eq!(l.px.1, l.frame.3 + l.strip.3);
        // A4 landscape PDF: the frame shows what fits inside the margins.
        let pdf = Panel {
            format: Format::Pdf,
            ..p.clone()
        };
        let l = layout(&pdf, (1000.0, 600.0), 10.0).unwrap();
        assert!(l.landscape);
        assert_eq!(l.px, (3508, 2480));
        let (pw, _) = l.page_pt.unwrap();
        assert!((pw - 297.0 / 25.4 * 72.0).abs() < 1e-9);
        // The frame's ground width is (297 − 20) mm × 25,000.
        assert!((l.ground.0 - 0.277 * 25_000.0).abs() < 5.0, "{l:?}");
        // Too large.
        let big = Panel {
            scale: 500.0,
            dpi: 600,
            ..p
        };
        assert!(layout(&big, (1000.0, 600.0), 10.0).is_err());
        assert!(layout(&pdf, (1.0, 1.0), f64::NAN).is_err());
    }

    #[test]
    fn panel_rows_and_text() {
        let mut p = Panel::new(false, 30_000.0);
        assert_eq!(p.scale, 25_000.0);
        p.row = 1;
        p.change(1);
        assert_eq!(p.scale, 50_000.0);
        p.row = 0;
        p.change(1);
        assert_eq!(p.format, Format::Pdf);
        assert_eq!(p.rows(), vec![0, 1, 2, 3]);
        assert_eq!(
            scale_text(25_000.0).replace(['\u{2068}', '\u{2069}'], ""),
            "1:25,000"
        );
        assert_eq!(
            scale_text(500.0).replace(['\u{2068}', '\u{2069}'], ""),
            "1:500"
        );
        let pr = Panel::new(true, 1.0e9);
        assert_eq!(pr.format, Format::Pdf);
        assert_eq!(pr.scale, 5_000_000.0);
    }

    #[test]
    fn png_records_its_resolution() {
        let png = png_with_dpi(2, 1, &[255, 0, 0, 255, 0, 0, 255, 255], 300).unwrap();
        let i = png.windows(4).position(|w| w == b"pHYs").unwrap();
        let ppm = u32::from_be_bytes([png[i + 4], png[i + 5], png[i + 6], png[i + 7]]);
        assert_eq!(ppm, 11_811);
        assert_eq!(png[i + 12], 1);
        let crc = u32::from_be_bytes([png[i + 13], png[i + 14], png[i + 15], png[i + 16]]);
        assert_eq!(crc, crc32(&png[i..i + 13]));
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }
}
