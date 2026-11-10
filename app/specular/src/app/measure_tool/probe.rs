//! Inspection probes in the measurement tool (NDT):
//! radiograph evaluation and calibrated crack widths, computed by
//! [`specular_measure::inspect`].
//!
//! - One tool, Inspect (`i`), with a probe picked by its button or `[` / `]`: line profile,
//!   SNR, duplex-wire IQI, single-wire IQI, crack width and density point.
//! - Grey values are the file's stored values when it is DICOM/DICONDE (and densities its
//!   modality values when Rescale Type says OD), else the decoded grey (16-bit kept), else a
//!   photo's luma.
//! - A per-file sidecar `<file>.inspect.json` keeps what the user set: the scale from a known
//!   length, two grey → density points, and the basic spatial resolution (SRb). Only the
//!   sidecar is written, and only when one of them is set.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::{CancelToken, PixelLayout, Raster};
use specular_decode::{DecodeRequest, ModalityFrame, Rect as Region};
use specular_i18n::{I18n, t};
use specular_inspect::{SourceValue, source_pixel};
use specular_measure::calib::{Calibration, Source, Unit, format_sig};
use specular_measure::geom::{P, Shape};
use specular_measure::inspect::{self, Density, Grey, Probe, Reading, Refusal};
use specular_measure::list::Measurement;

use specular_log::{UI, debug, warn};

use super::{Ask, Done, ItemView, Kind, Pixels, Prompt};
use crate::app::App;

/// The largest sidecar read.
const MAX_SIDECAR: u64 = 64 * 1024;
/// Pixels to either side of a crack line that a cut looks at.
const CRACK_HALF: f64 = 25.0;
/// Largest block of pixels a probe reads (a line across a huge image is refused).
const MAX_BLOCK: u64 = 64 << 20;

pub(super) fn label(p: Probe) -> String {
    I18n::global().tr(&format!("ndt-probe-{}", p.key()))
}

pub(super) fn hint(p: Probe) -> String {
    I18n::global().tr(&format!("ndt-hint-{}", p.key()))
}

fn refusal(r: Refusal) -> String {
    I18n::global().tr(&format!("ndt-refused-{}", r.key()))
}

/// The tool's shape for a probe's points (`None` while incomplete or degenerate).
pub(super) fn shape(p: Probe, pts: &[P]) -> Option<Shape> {
    let s = match (p, pts) {
        (Probe::Density, &[a]) => Shape::Point(a),
        (Probe::Snr, &[a, b]) => Shape::rect(a, b),
        (Probe::Crack, &[a, b]) | (Probe::Profile | Probe::Duplex | Probe::Wire, &[a, b]) => {
            Shape::Line(a, b)
        }
        (Probe::Crack, v) if v.len() > 2 => Shape::Polyline(v.to_vec()),
        _ => return None,
    };
    let ok = match &s {
        Shape::Point(_) => true,
        Shape::Line(a, b) => (*b - *a).hypot() > 0.0,
        Shape::Rect { .. } => s.area_px().is_some_and(|v| v > 0.0),
        _ => s.is_valid(),
    };
    (s.is_valid() && ok).then_some(s)
}

// ----- The sidecar ---------------------------------------------------------------------

/// What the user set on a file, kept beside it.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Sidecar {
    /// The scale from a known length.
    pub scale: Option<Calibration>,
    /// Grey → density points, newest last (two make the calibration).
    pub density: Vec<(f64, f64)>,
    /// The basic spatial resolution, µm.
    pub srb_um: Option<f64>,
    /// The interpolated basic spatial resolution iSRb (ISO 19232-5:2018), µm, from the
    /// last duplex IQI measured.
    pub isrb_um: Option<f64>,
}

impl Sidecar {
    /// `weld.dcm.inspect.json` beside `weld.dcm`.
    pub fn path(file: &Path) -> PathBuf {
        let mut name = file.file_name().unwrap_or_default().to_os_string();
        name.push(".inspect.json");
        file.with_file_name(name)
    }

    pub fn is_empty(&self) -> bool {
        self.scale.is_none()
            && self.density.is_empty()
            && self.srb_um.is_none()
            && self.isrb_um.is_none()
    }

    /// The two-point density calibration, once there are two points.
    pub fn density(&self) -> Option<Density> {
        let n = self.density.len();
        let [(g0, d0), (g1, d1)] = self.density.get(n.checked_sub(2)?..)? else {
            return None;
        };
        Density::new([*g0, *g1], [*d0, *d1])
    }

    pub fn to_json(&self) -> String {
        let mut o = serde_json::Map::new();
        o.insert("version".into(), 1.into());
        if let Some(c) = &self.scale {
            o.insert(
                "scale".into(),
                serde_json::json!({ "m": c.m, "unit": c.unit.key() }),
            );
        }
        if !self.density.is_empty() {
            o.insert(
                "density".into(),
                serde_json::json!(
                    self.density
                        .iter()
                        .map(|(g, d)| [*g, *d])
                        .collect::<Vec<_>>()
                ),
            );
        }
        if let Some(s) = self.srb_um {
            o.insert("srb_um".into(), s.into());
        }
        if let Some(s) = self.isrb_um {
            o.insert("isrb_um".into(), s.into());
        }
        let mut s = serde_json::to_string_pretty(&serde_json::Value::Object(o)).unwrap_or_default();
        s.push('\n');
        s
    }

    /// A sidecar's contents; anything unreadable is left out.
    pub fn parse(bytes: &[u8]) -> Option<Sidecar> {
        let v: serde_json::Value = serde_json::from_slice(bytes).ok()?;
        let num = |v: &serde_json::Value| v.as_f64().filter(|x| x.is_finite());
        let scale = v.get("scale").and_then(|s| {
            let m = s.get("m")?.as_array()?;
            let row = |r: &serde_json::Value| -> Option<[f64; 2]> {
                let r = r.as_array()?;
                Some([num(r.first()?)?, num(r.get(1)?)?])
            };
            let m = [row(m.first()?)?, row(m.get(1)?)?];
            let unit = Unit::from_key(s.get("unit")?.as_str()?)?;
            let c = Calibration {
                m,
                unit,
                source: Source::User,
            };
            (unit != Unit::Px && c.pixel_size() > 0.0 && c.pixel_size().is_finite()).then_some(c)
        });
        let density = v
            .get("density")
            .and_then(|d| d.as_array())
            .map(|d| {
                d.iter()
                    .filter_map(|p| {
                        let p = p.as_array()?;
                        Some((num(p.first()?)?, num(p.get(1)?)?))
                    })
                    .take(8)
                    .collect()
            })
            .unwrap_or_default();
        let um = |key: &str| v.get(key).and_then(num).filter(|s| *s > 0.0 && *s < 1e6);
        Some(Sidecar {
            scale,
            density,
            srb_um: um("srb_um"),
            isrb_um: um("isrb_um"),
        })
    }

    /// Reads `file`'s sidecar (`None`: there is none, or it is not ours).
    pub fn read(file: &Path) -> Option<Sidecar> {
        let p = Sidecar::path(file);
        let meta = std::fs::metadata(&p).ok()?;
        if !meta.is_file() || meta.len() > MAX_SIDECAR {
            return None;
        }
        Sidecar::parse(&std::fs::read(&p).ok()?)
    }

    /// Writes `file`'s sidecar, or removes it when empty.
    pub fn write(&self, file: &Path) -> std::io::Result<()> {
        let p = Sidecar::path(file);
        if self.is_empty() {
            return match std::fs::remove_file(&p) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            };
        }
        specular_markup::write_atomic(&p, self.to_json().as_bytes())
    }
}

// ----- Reading pixels ------------------------------------------------------------------

/// Where a probe reads its values.
pub(super) struct ProbeSource {
    pub px: Pixels,
    /// DICOM/DICONDE stored and modality values.
    pub modality: Option<Arc<ModalityFrame>>,
    /// The image's size.
    pub size: (u32, u32),
}

/// A probe's blocks: grey values, the densities the file states (Rescale Type OD), and
/// whether they came from a reduced copy.
struct Blocks {
    grey: Grey,
    od: Option<Grey>,
    approximate: bool,
}

/// The luma of a pixel (stored values; 16-bit kept as is).
fn luma(v: SourceValue) -> Option<f64> {
    let y = |r: f64, g: f64, b: f64| 0.2126 * r + 0.7152 * g + 0.0722 * b;
    Some(match v {
        SourceValue::Gray(g) => f64::from(g),
        SourceValue::Gray16(g) => f64::from(g),
        SourceValue::GrayAlpha16([g, _]) => f64::from(g),
        SourceValue::Rgb([r, g, b])
        | SourceValue::Rgba([r, g, b, _])
        | SourceValue::YCbCr { rgb: [r, g, b], .. } => y(f64::from(r), f64::from(g), f64::from(b)),
        SourceValue::Rgba16([r, g, b, _]) => y(f64::from(r), f64::from(g), f64::from(b)),
    })
}

fn from_raster(r: &Raster, x0: u32, y0: u32, w: u32, h: u32) -> Option<Grey> {
    if matches!(r.layout, PixelLayout::Rgba16F(_)) {
        return None;
    }
    let mut data = Vec::with_capacity(w as usize * h as usize);
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            data.push(source_pixel(r, x, y).and_then(luma).unwrap_or(f64::NAN));
        }
    }
    Grey::new(x0, y0, w, h, data)
}

/// The blocks covering `b` (stored pixels) and `margin` pixels around it.
fn blocks(src: &ProbeSource, b: (f64, f64, f64, f64), margin: f64) -> Option<Blocks> {
    let (w, h) = src.size;
    let x0 = ((b.0 - margin).floor().max(0.0) as u32).min(w.saturating_sub(1));
    let y0 = ((b.1 - margin).floor().max(0.0) as u32).min(h.saturating_sub(1));
    let x1 = ((b.2 + margin).ceil().max(0.0) as u32).clamp(x0 + 1, w);
    let y1 = ((b.3 + margin).ceil().max(0.0) as u32).clamp(y0 + 1, h);
    let (bw, bh) = (x1 - x0, y1 - y0);
    if u64::from(bw) * u64::from(bh) > MAX_BLOCK {
        return None;
    }
    if let Some(m) = src.modality.as_ref().filter(|m| m.is_grey()) {
        let mut grey = Vec::with_capacity(bw as usize * bh as usize);
        let mut od = Vec::with_capacity(grey.capacity());
        for y in y0..y1 {
            for x in x0..x1 {
                grey.push(m.stored_at(x, y).map_or(f64::NAN, |v| v as f64));
                od.push(m.value_at(x, y).unwrap_or(f64::NAN));
            }
        }
        let is_od = m.unit().is_some_and(|u| u.eq_ignore_ascii_case("OD"));
        return Some(Blocks {
            grey: Grey::new(x0, y0, bw, bh, grey)?,
            od: if is_od {
                Grey::new(x0, y0, bw, bh, od)
            } else {
                None
            },
            approximate: false,
        });
    }
    if let Some((plane, _)) = &src.px.values {
        let mut data = Vec::with_capacity(bw as usize * bh as usize);
        for y in y0..y1 {
            for x in x0..x1 {
                data.push(plane.value(x, y).map_or(f64::NAN, f64::from));
            }
        }
        return Some(Blocks {
            grey: Grey::new(x0, y0, bw, bh, data)?,
            od: None,
            approximate: false,
        });
    }
    let done = |grey, approximate| {
        Some(Blocks {
            grey,
            od: None,
            approximate,
        })
    };
    if let Some(r) = &src.px.full {
        return done(from_raster(r, x0, y0, bw, bh)?, false);
    }
    if let Some((session, _)) = &src.px.regions {
        let cancel = CancelToken::new();
        if session.regions_ready() || session.prepare_regions(&cancel).is_ok() {
            let req = DecodeRequest {
                scale: 1,
                region: Some(Region {
                    x: x0,
                    y: y0,
                    w: bw,
                    h: bh,
                }),
                parallel: false,
                prepare_regions: false,
            };
            if let Ok(r) = session.decode(&req, &cancel)
                && let Some(g) = from_raster(&r, x0, y0, bw, bh)
            {
                return done(g, false);
            }
        }
    }
    let r = src.px.reduced.as_ref()?;
    done(from_raster(r, x0, y0, bw, bh)?, true)
}

/// What a probe found, the shape it measured (an SNR region grows to the standard's size),
/// and whether the values were approximate.
pub(super) struct Found {
    pub reading: Reading,
    pub shape: Option<Shape>,
    pub approximate: bool,
}

/// Runs probe `p` over `shape` (off the UI thread).
pub(super) fn read(
    p: Probe,
    shape: &Shape,
    cal: &Calibration,
    src: &ProbeSource,
    density: Option<Density>,
    srb_um: Option<f64>,
) -> Option<Found> {
    let refused = |r| {
        Some(Found {
            reading: Reading::Refused(r),
            shape: None,
            approximate: false,
        })
    };
    let pts = shape.points();
    let (iw, ih) = src.size;
    match (p, shape) {
        (Probe::Snr, Shape::Rect { x0, y0, x1, y1 }) => {
            let Some(region) = inspect::snr_region((*x0, *y0, *x1, *y1), iw, ih) else {
                return refused(Refusal::TooSmall);
            };
            let rb = (
                f64::from(region.0),
                f64::from(region.1),
                f64::from(region.2),
                f64::from(region.3),
            );
            let b = blocks(src, rb, 0.0)?;
            let grown = Shape::Rect {
                x0: rb.0,
                y0: rb.1,
                x1: rb.2,
                y1: rb.3,
            };
            Some(match inspect::snr(&b.grey, region, srb_um) {
                Some(s) => Found {
                    reading: Reading::Snr(s),
                    shape: Some(grown),
                    approximate: b.approximate,
                },
                None => Found {
                    reading: Reading::Refused(Refusal::NoNoise),
                    shape: Some(grown),
                    approximate: b.approximate,
                },
            })
        }
        (Probe::Density, Shape::Point(at)) => {
            let b = blocks(src, (at.x, at.y, at.x, at.y), 2.0)?;
            let grey = b.grey.sample(*at);
            let density = match &b.od {
                Some(od) => Some(od.sample(*at)),
                None => density.map(|d| d.at(grey)),
            };
            Some(Found {
                reading: Reading::Density { grey, density },
                shape: None,
                approximate: b.approximate,
            })
        }
        (Probe::Crack, s) if s.is_path() => {
            let b = blocks(src, s.bounds(), CRACK_HALF + 2.0)?;
            let reading =
                match inspect::crack(&b.grey, &pts, inspect::CRACK_POINTS, CRACK_HALF, cal) {
                    Ok(c) => Reading::Crack(c),
                    Err(r) => Reading::Refused(r),
                };
            Some(Found {
                reading,
                shape: None,
                approximate: b.approximate,
            })
        }
        (Probe::Profile | Probe::Duplex | Probe::Wire, Shape::Line(a, bb)) => {
            let (a, bb) = (*a, *bb);
            let width = if p == Probe::Profile {
                1
            } else {
                inspect::IQI_WIDTH
            };
            let b = blocks(src, shape.bounds(), f64::from(width) / 2.0 + 2.0)?;
            let (values, step) = inspect::profile(&b.grey, a, bb, width);
            let reading = if p == Probe::Profile {
                let density = match &b.od {
                    Some(od) => Some(inspect::profile(od, a, bb, 1).0),
                    None => density.map(|d| values.iter().map(|g| d.at(*g)).collect()),
                };
                Reading::Profile {
                    values,
                    step,
                    density,
                }
            } else {
                let Some(mm) = inspect::mm_per_px(cal, a, bb) else {
                    return refused(Refusal::NeedsScale);
                };
                let step_mm = mm * step;
                let r = if p == Probe::Duplex {
                    inspect::duplex(&values, step_mm).map(Reading::Duplex)
                } else {
                    inspect::wire(&values, step_mm).map(Reading::Wire)
                };
                r.unwrap_or_else(Reading::Refused)
            };
            Some(Found {
                reading,
                shape: None,
                approximate: b.approximate,
            })
        }
        _ => None,
    }
}

// ----- Showing them --------------------------------------------------------------------

/// Integer-looking values whole; others to four significant digits.
fn n4(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e9 {
        specular_i18n::fixed(v, 0)
    } else {
        format_sig(v, 4)
    }
}

/// Three significant digits without trailing zeros ("0.08", "40").
fn sig3(v: f64) -> String {
    let s = format_sig(v, 3);
    if s.contains('.') && !s.contains(['e', 'E']) {
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        s
    }
}

fn mm(v: f64) -> String {
    super::length_with_unit(sig3(v), "mm")
}

/// The headline and detail lines of a probe's measurement.
pub(super) fn lines(m: &Measurement, pending: bool) -> (String, Vec<String>) {
    let Some(r) = &m.reading else {
        return (
            String::new(),
            vec![if pending {
                t!("ndt-reading")
            } else {
                t!("ndt-no-pixels")
            }],
        );
    };
    match r {
        Reading::Refused(why) => (refusal(*why), Vec::new()),
        Reading::Profile {
            values, density, ..
        } => {
            let (lo, hi) = range(values);
            let mean = values.iter().sum::<f64>() / values.len().max(1) as f64;
            let mut d = vec![t!("ndt-profile-mean", mean = n4(mean))];
            if let Some(od) = density {
                let (a, b) = range(od);
                d.push(t!("ndt-profile-density", min = sig3(a), max = sig3(b)));
            }
            (t!("ndt-profile-range", min = n4(lo), max = n4(hi)), d)
        }
        Reading::Snr(s) => {
            let mut d = Vec::new();
            match s.normalised {
                Some((srb, n)) => d.push(t!("ndt-snrn", value = sig3(n), srb = sig3(srb))),
                None => d.push(t!("ndt-snrn-needs-srb")),
            }
            d.push(t!(
                "ndt-snr-stats",
                mean = n4(s.mean),
                sd = format_sig(s.sd, 4),
                w = i64::from(s.region.2 - s.region.0),
                h = i64::from(s.region.3 - s.region.1)
            ));
            (t!("ndt-snr", value = sig3(s.snr)), d)
        }
        Reading::Duplex(dx) => {
            let dips = dx
                .pairs
                .iter()
                .filter(|p| p.span.0 != p.span.1 || p.dip > 0.0)
                .map(|p| {
                    let name = p
                        .number
                        .map_or_else(|| "?".to_owned(), |(n, _)| format!("D{n}"));
                    let dip = specular_i18n::I18n::global()
                        .localizer()
                        .percent(p.dip / 100.0, 0);
                    format!("{name} {dip}")
                })
                .collect::<Vec<_>>();
            let dips = specular_i18n::dot_join(&dips);
            let head = match dx.srb() {
                Some((n, d)) => t!(
                    "ndt-duplex-result",
                    pair = format!("D{n}"),
                    srb = sig3(d * 1000.0),
                    unsharpness = mm(2.0 * d)
                ),
                None => t!("ndt-duplex-all-resolved"),
            };
            let mut d = Vec::new();
            if let Some(i) = dx.isrb {
                d.push(t!(
                    "ndt-duplex-isrb",
                    isrb = sig3(i * 1000.0),
                    unsharpness = mm(2.0 * i)
                ));
            }
            d.push(t!("ndt-duplex-dips", dips = dips));
            (head, d)
        }
        Reading::Wire(w) => (
            t!(
                "ndt-wire-result",
                wire = format!("W{}", w.number),
                diameter = mm(w.nominal)
            ),
            vec![t!(
                "ndt-wire-measured",
                width = mm(w.fwhm),
                diameter = mm(w.diameter)
            )],
        ),
        Reading::Crack(c) => {
            let v = |x: f64| super::length_with_unit(sig3(x), m.cal.unit.symbol());
            (
                t!("ndt-crack-mean", width = v(c.widths.1)),
                vec![
                    t!("ndt-crack-range", min = v(c.widths.0), max = v(c.widths.2)),
                    t!("ndt-crack-cuts", count = c.cuts.len() as i64),
                ],
            )
        }
        Reading::Density { grey, density } => (
            match density {
                Some(d) => t!("ndt-density-value", grey = n4(*grey), density = sig3(*d)),
                None => t!("ndt-density-grey", grey = n4(*grey)),
            },
            if density.is_none() {
                vec![t!("ndt-density-uncalibrated")]
            } else {
                Vec::new()
            },
        ),
    }
}

fn range(v: &[f64]) -> (f64, f64) {
    v.iter()
        .filter(|x| x.is_finite())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| {
            (a.min(*x), b.max(*x))
        })
}

/// A probe measurement's row in the list: its outline, the crack's cuts as guides, and the
/// profile as a small plot.
pub(super) fn item(
    m: &Measurement,
    pending: bool,
    badge: String,
    screen: impl Fn(P) -> (f32, f32),
) -> ItemView {
    let probe = m.probe.unwrap_or(Probe::Profile);
    let (value, detail) = lines(m, pending);
    let mut guides = Vec::new();
    let mut plot = Vec::new();
    match &m.reading {
        Some(Reading::Crack(c)) => {
            guides = c
                .cuts
                .iter()
                .map(|k| vec![screen(k.edges.0), screen(k.edges.1)])
                .collect();
        }
        Some(Reading::Profile { values, .. }) => plot.clone_from(values),
        _ => {}
    }
    let outline: Vec<(f32, f32)> = m.shape.outline().iter().map(|p| screen(*p)).collect();
    ItemView {
        title: t!(
            "measure-tool-item",
            n = i64::from(m.id),
            shape = label(probe)
        ),
        value,
        detail,
        badge,
        caveat: m.cal.source.is_approximate() || m.approximate,
        outline,
        closed: m.shape.is_area(),
        handles: Vec::new(),
        guides,
        plot,
    }
}

// ----- The tool ------------------------------------------------------------------------

/// The status line while a prompt asks for a density or the SRb.
pub(super) fn prompt_text(p: &Prompt) -> String {
    let value = if p.text.is_empty() {
        "…".to_owned()
    } else {
        p.text.clone()
    };
    match p.ask {
        Ask::Srb => t!("ndt-ask-srb", value = value),
        _ => t!("ndt-ask-density", value = value),
    }
}

impl App {
    /// Picks the next (or previous) probe; the points placed so far go.
    pub(super) fn measure_step_probe(&mut self, forward: bool) {
        let Some(t) = self.measure.tool.as_mut() else {
            return;
        };
        let all = Probe::ALL;
        let i = all.iter().position(|p| *p == t.probe).unwrap_or(0);
        let step = if forward { 1 } else { all.len() - 1 };
        t.probe = all[(i + step) % all.len()];
        t.kind = Kind::Inspect;
        t.pts.clear();
        t.status = None;
        t.prompt = None;
        self.measure.probe = t.probe;
        let msg = format!("{}: {}", label(t.probe), hint(t.probe));
        self.chrome.announce_status(msg);
        self.redraw = true;
    }

    /// The file states densities (DICOM Rescale Type OD).
    fn measure_file_density(&self) -> bool {
        self.medical_frame_values()
            .is_some_and(|m| m.unit().is_some_and(|u| u.eq_ignore_ascii_case("OD")))
    }

    /// Adds a probe's measurement and reads it; a density point asks for its known density.
    pub(super) fn measure_probe_add(&mut self, p: Probe, shape: Shape) {
        let at = shape.points().first().copied().unwrap_or_default();
        let id = self
            .measure
            .list
            .push(Measurement::probed(p, shape, self.measure.cal));
        if let Some(t) = self.measure.tool.as_mut() {
            t.selected = Some(id);
            t.status = None;
        }
        self.measure_probe_read(id);
        if p == Probe::Density && !self.measure_file_density() {
            if let Some(t) = self.measure.tool.as_mut() {
                t.prompt = Some(Prompt {
                    a: at,
                    b: at,
                    text: String::new(),
                    unit: 0,
                    ask: Ask::Density(id),
                });
                t.focus = None;
            }
            self.chrome
                .announce_status(t!("ndt-ask-density", value = "…".to_owned()));
        }
        self.redraw = true;
    }

    fn measure_probe_source(&self) -> ProbeSource {
        let modality = self.medical_frame_values();
        let size = modality
            .as_ref()
            .map(|m| (m.width(), m.height()))
            .or_else(|| {
                let i = self.images.get(&self.current_id()?)?;
                i.info.as_ref().map(|f| (f.width, f.height))
            })
            .unwrap_or((0, 0));
        ProbeSource {
            px: self.measure_pixels(),
            modality,
            size,
        }
    }

    /// Reads probe measurement `id` on a background thread.
    pub(super) fn measure_probe_read(&mut self, id: u32) {
        let (Some(key), Some(m)) = (self.measure.key.clone(), self.measure.list.get(id)) else {
            return;
        };
        let Some(p) = m.probe else { return };
        let (shape, cal) = (m.shape.clone(), m.cal);
        let src = self.measure_probe_source();
        if src.size.0 == 0 || src.size.1 == 0 {
            self.measure.missing.insert(id);
            return;
        }
        let density = self.measure.side.density();
        let srb = self.measure.side.srb_um;
        self.measure.pending.insert(id);
        let tx = self.measure.tx.clone();
        self.loader.background("measure-probe", move |l| {
            let t0 = std::time::Instant::now();
            let found = read(p, &shape, &cal, &src, density, srb);
            debug!(target: UI, "measure: {} #{id} in {:.1} ms", p.key(), t0.elapsed().as_secs_f64() * 1e3);
            let _ = tx.send(Done::Probe { key, id, found });
            l.waker()();
        });
    }

    /// A probe's reading arrived.
    pub(super) fn measure_probe_done(&mut self, id: u32, found: Option<Found>) {
        self.measure.pending.remove(&id);
        let Some(m) = self.measure.list.get_mut(id) else {
            return;
        };
        let Some(f) = found else {
            m.reading = None;
            self.measure.missing.insert(id);
            return;
        };
        if let Some(s) = f.shape {
            m.shape = s;
        }
        m.approximate = f.approximate;
        m.reading = Some(f.reading);
        let probe = m.probe.unwrap_or(Probe::Profile);
        let (head, _) = lines(m, false);
        let (srb, isrb) = match &m.reading {
            Some(Reading::Duplex(d)) => (d.srb().map(|s| s.1 * 1000.0), d.isrb.map(|i| i * 1000.0)),
            _ => (None, None),
        };
        let needs_srb = matches!(&m.reading, Some(Reading::Snr(s)) if s.normalised.is_none());
        if let Some(t) = self.measure.tool.as_mut()
            && t.selected == Some(id)
            && t.prompt.is_none()
        {
            t.status = Some(head.clone());
        }
        self.chrome
            .announce_status(format!("{}: {head}", label(probe)));
        if let Some(s) = srb {
            self.measure.side.isrb_um = isrb;
            self.measure_set_srb(s);
        }
        if needs_srb
            && self.measure.side.srb_um.is_none()
            && let Some(t) = self.measure.tool.as_mut()
            && t.prompt.is_none()
        {
            t.prompt = Some(Prompt {
                a: P::default(),
                b: P::default(),
                text: String::new(),
                unit: 0,
                ask: Ask::Srb,
            });
            t.focus = None;
        }
        self.redraw = true;
    }

    /// Sets the image's SRb (µm), keeps it in the sidecar and normalises every SNR with it.
    fn measure_set_srb(&mut self, srb: f64) {
        self.measure.side.srb_um = Some(srb);
        self.measure_write_side();
        let ids: Vec<u32> = self.measure.list.items().iter().map(|m| m.id).collect();
        for id in ids {
            if let Some(m) = self.measure.list.get_mut(id)
                && let Some(Reading::Snr(s)) = &mut m.reading
            {
                s.normalised = Some((srb, inspect::normalise(s.snr, srb)));
            }
        }
        self.redraw = true;
    }

    /// Re-reads the probes whose values depend on what the sidecar holds.
    fn measure_probe_reread(&mut self) {
        let ids: Vec<u32> = self
            .measure
            .list
            .items()
            .iter()
            .filter(|m| m.probe.is_some())
            .map(|m| m.id)
            .collect();
        for id in ids {
            self.measure_probe_read(id);
        }
    }

    /// The typed density or SRb (Enter in the prompt). An empty answer only closes it.
    pub(super) fn measure_probe_answer(&mut self) {
        let Some(t) = self.measure.tool.as_mut() else {
            return;
        };
        let Some(p) = t.prompt.take() else { return };
        let text = p.text.replace(',', ".");
        let text = text.trim();
        self.redraw = true;
        if text.is_empty() {
            return;
        }
        let v: f64 = text.parse().unwrap_or(f64::NAN);
        let bad = |me: &mut App, p: Prompt, msg: String| {
            if let Some(t) = me.measure.tool.as_mut() {
                t.prompt = Some(p);
                t.status = Some(msg.clone());
            }
            me.chrome.announce_status(msg);
        };
        match p.ask {
            Ask::Srb => {
                if !(v.is_finite() && v > 0.0 && v < 1e5) {
                    return bad(self, p, t!("ndt-bad-srb"));
                }
                // A typed SRb replaces the measured one and its interpolation.
                self.measure.side.isrb_um = None;
                self.measure_set_srb(v);
                let msg = t!("ndt-srb-set", srb = sig3(v));
                if let Some(t) = self.measure.tool.as_mut() {
                    t.status = Some(msg.clone());
                }
                self.chrome.announce_status(msg);
            }
            Ask::Density(id) => {
                let grey = match self.measure.list.get(id).and_then(|m| m.reading.as_ref()) {
                    Some(Reading::Density { grey, .. }) => Some(*grey),
                    _ => None,
                };
                let Some(grey) = grey.filter(|g| g.is_finite()) else {
                    return bad(self, p, t!("ndt-reading"));
                };
                if !(v.is_finite() && (0.0..=10.0).contains(&v)) {
                    return bad(self, p, t!("ndt-bad-density"));
                }
                let side = &mut self.measure.side;
                side.density.retain(|(g, _)| *g != grey);
                side.density.push((grey, v));
                let n = side.density.len();
                if n > 8 {
                    side.density.drain(..n - 8);
                }
                let done = side.density().is_some();
                self.measure_write_side();
                self.measure_probe_reread();
                let msg = if done {
                    t!("ndt-density-calibrated")
                } else {
                    t!("ndt-density-one-more")
                };
                if let Some(t) = self.measure.tool.as_mut() {
                    t.status = Some(msg.clone());
                }
                self.chrome.announce_status(msg);
            }
            Ask::Length => {}
        }
    }

    /// Reads the image's sidecar in the background.
    pub(super) fn measure_read_side(&mut self) {
        let Some(key) = self.measure.key.clone() else {
            return;
        };
        let tx = self.measure.tx.clone();
        self.loader.background("measure-sidecar", move |l| {
            let side = Sidecar::read(&key.path);
            let _ = tx.send(Done::Side(key, side));
            l.waker()();
        });
    }

    /// The sidecar was read: its scale replaces the file's, and the probes are read again.
    pub(super) fn measure_side_read(&mut self, side: Option<Sidecar>) {
        let Some(s) = side else { return };
        if let Some(c) = s.scale
            && self.measure.cal.source != Source::User
        {
            self.measure.cal = c;
            self.measure.region_of.clear();
            let ids: Vec<u32> = self.measure.list.items().iter().map(|m| m.id).collect();
            for id in ids {
                if let Some(m) = self.measure.list.get_mut(id) {
                    m.cal = c;
                }
            }
        }
        self.measure.side = s;
        self.measure_probe_reread();
        self.redraw = true;
    }

    /// Writes the sidecar in the background (only beside a file that exists).
    pub(super) fn measure_write_side(&mut self) {
        let Some(key) = self.measure.key.clone() else {
            return;
        };
        let side = self.measure.side.clone();
        let tx = self.measure.tx.clone();
        self.loader.background("measure-sc-save", move |l| {
            if key.path.is_file()
                && let Err(e) = side.write(&key.path)
            {
                warn!(target: UI, "measure: sidecar of {}: {e}", key.path.display());
                let _ = tx.send(Done::SideFailed(e.to_string()));
                l.waker()();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidecar_round_trips() {
        let s = Sidecar {
            scale: Some(Calibration::spacing(0.05, 0.05, Unit::Mm, Source::User).unwrap()),
            density: vec![(1000.0, 3.5), (30000.0, 1.2)],
            srb_um: Some(63.0),
            isrb_um: Some(58.2),
        };
        let back = Sidecar::parse(s.to_json().as_bytes()).unwrap();
        assert_eq!(back, s);
        assert!((back.density().unwrap().at(1000.0) - 3.5).abs() < 1e-12);
        assert_eq!(
            Sidecar::path(Path::new("/x/weld.dcm")),
            PathBuf::from("/x/weld.dcm.inspect.json")
        );
        // Junk is ignored, not trusted.
        let j = Sidecar::parse(br#"{"scale":{"m":[[0,0],[0,0]],"unit":"mm"},"srb_um":-4}"#);
        assert_eq!(j, Some(Sidecar::default()));
        assert_eq!(Sidecar::parse(b"not json"), None);
    }

    #[test]
    fn probes_make_their_shapes() {
        let (a, b, c) = (P::new(1.0, 1.0), P::new(9.0, 4.0), P::new(12.0, 9.0));
        assert_eq!(shape(Probe::Profile, &[a, b]), Some(Shape::Line(a, b)));
        assert_eq!(shape(Probe::Snr, &[b, a]), Some(Shape::rect(b, a)));
        assert_eq!(shape(Probe::Density, &[a]), Some(Shape::Point(a)));
        assert_eq!(
            shape(Probe::Crack, &[a, b, c]),
            Some(Shape::Polyline(vec![a, b, c]))
        );
        assert_eq!(shape(Probe::Wire, &[a, a]), None);
    }

    fn raster_source(w: u32, h: u32, f: impl Fn(u32, u32) -> u16) -> ProbeSource {
        let mut data = Vec::with_capacity(w as usize * h as usize * 2);
        for y in 0..h {
            for x in 0..w {
                data.extend(f(x, y).to_ne_bytes());
            }
        }
        let r = Raster {
            width: w,
            height: h,
            origin: (0, 0),
            scale: 1,
            layout: PixelLayout::Gray16,
            planes: vec![specular_core::RasterPlane {
                width: w,
                height: h,
                stride: w as usize * 2,
                data,
                h_div: 1,
                v_div: 1,
            }],
            truncated: false,
        };
        ProbeSource {
            px: Pixels {
                full: Some(Arc::new(r)),
                regions: None,
                reduced: None,
                values: None,
            },
            modality: None,
            size: (w, h),
        }
    }

    #[test]
    fn probes_read_a_16_bit_raster() {
        // 30000 with a dark wire 10 px wide at x = 50, 0.02 mm a pixel.
        let src = raster_source(
            100,
            80,
            |x, _| {
                if (45..55).contains(&x) { 20000 } else { 30000 }
            },
        );
        let cal = Calibration::spacing(0.02, 0.02, Unit::Mm, Source::User).unwrap();
        let line = Shape::Line(P::new(10.0, 40.0), P::new(90.0, 40.0));
        let f = read(Probe::Profile, &line, &cal, &src, None, None).unwrap();
        let Reading::Profile { values, .. } = &f.reading else {
            panic!("{:?}", f.reading)
        };
        assert_eq!(values.len(), 81);
        assert_eq!(values[40], 20000.0);
        assert_eq!(values[0], 30000.0);
        let f = read(
            Probe::Crack,
            &Shape::Line(P::new(50.0, 5.0), P::new(50.0, 75.0)),
            &cal,
            &src,
            None,
            None,
        )
        .unwrap();
        let Reading::Crack(c) = &f.reading else {
            panic!("{:?}", f.reading)
        };
        assert!((c.widths.1 - 0.2).abs() < 0.01, "{:?}", c.widths);
        // Without a scale the IQI probes say why.
        let f = read(
            Probe::Wire,
            &line,
            &Calibration::uncalibrated(),
            &src,
            None,
            None,
        )
        .unwrap();
        assert_eq!(f.reading, Reading::Refused(Refusal::NeedsScale));
        // An upright SNR region grows to 20 × 55 (clear of the wire: no noise at all).
        let f = read(
            Probe::Snr,
            &Shape::rect(P::new(70.0, 10.0), P::new(72.0, 40.0)),
            &cal,
            &src,
            None,
            Some(50.0),
        )
        .unwrap();
        assert_eq!(f.reading, Reading::Refused(Refusal::NoNoise));
        let Some(Shape::Rect { x0, x1, y0, y1 }) = f.shape else {
            panic!()
        };
        assert_eq!((x1 - x0, y1 - y0), (20.0, 55.0));
    }
}
