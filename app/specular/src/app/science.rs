//! Science mode (FITS, XISF; DigitalMicrograph, MRC, Gwyddion, Igor): a stretch and display
//! limits drawn on the GPU from the real values, plane sliders for the axes past the image,
//! channel compositing, the value and sky (or physical) position under the pointer, a scale
//! bar, an optional coordinate grid and, for height maps, optional plane levelling.
//!
//! When the current image is scientific ([`specular_decode::Session::science`]) its planes
//! are read off the UI thread ([`specular_decode::Session::nd_plane`]) with their limits
//! ([`Limits`]) and, for histogram equalisation, their tables, then uploaded as f32 values
//! ([`ImageRenderer::set_value_channels`]) and drawn through a [`ScienceLook`]
//! ([`ImageRenderer::set_science`]): changing the stretch or hiding a channel never reads or
//! uploads again. The keymap's science mode ([`Mode::Science`], `[science]` in
//! `keymap.toml`, over the viewer's keys) is turned on: by default `s` cycles the stretch,
//! `l` the limits, `.` / `,` step the selected axis, `a` selects another, `c` composites the
//! channel axis in colour, `1`–`4` show or hide a channel, `w` shows the coordinate grid and
//! `p` levels a height map (scanning probe: its least-squares plane subtracted, for display) and
//! `0` brings back the automatic look (linear, zscale). Every action is in
//! [`Group::Science`], so a profile can offer or hide them together.
//!
//! A one-shot colour camera's raw frame (`BAYERPAT`) is uploaded once and demosaiced on the
//! GPU ([`ImageRenderer::set_value_cfa`]) into red, green and blue, each with its own
//! automatic stretch ([`Stretch::Auto`], PixInsight's AutoSTF, unlinked) until another
//! stretch is chosen. `b` shows the raw mosaic instead (kept across files) and `Alt+b` steps
//! the pattern of this file, for a header that has it wrong or none; SER video does the same
//! through its session ([`specular_decode::Session::set_debayer`]).
//!
//! The look (stretch, limits, grid, levelling) stays across files; the plane, the composite and the
//! hidden channels are per file. The automatic look matches the CPU raster the decoder
//! makes (zscale limits, linear; the channel axis' first three positions as red, green and
//! blue), so the switch to the GPU path does not show.
//!
//! [`ImageRenderer::set_value_channels`]: specular_render::ImageRenderer::set_value_channels
//! [`ImageRenderer::set_science`]: specular_render::ImageRenderer::set_science
//! [`ImageRenderer::set_value_cfa`]: specular_render::ImageRenderer::set_value_cfa
//! [`Mode::Science`]: specular_input::Mode::Science
//! [`Group::Science`]: specular_input::Group::Science

pub(super) mod astro;
mod coast;
mod ion;
#[cfg(test)]
mod shots;
pub(super) mod sky;
#[cfg(test)]
mod tests;
mod thermal;
pub(super) mod thermal_report;
pub(super) mod thermal_seq;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use specular_core::CancelToken;
use specular_core::nd::{self, Cfa, Limits, Stretch};
use specular_decode::science::wcs::{self, Frame, Wcs};
use specular_decode::science::{
    self as dsci, AxisKind, ClassScheme, ColourTable, DiffractionGeometry, NdPlane, RootFacts,
    ScienceSummary, TemperatureUnit, ThermalConditions,
};
use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_i18n::t;
use specular_input::{Action, ScienceState};
use specular_render::{ChannelLook, Hillshade, MAX_CHANNELS, Relief, ScienceLook};
use specular_theme::metrics::{FONT_SMALL, PAD, RADIUS_MD, TOOLBAR_MARGIN};
use specular_theme::{Color, Palette};
use specular_widgets::accesskit;

use super::App;
use crate::loader::{Event, ImageId, SciencePlane};

/// AccessKit ids: the readout, the sliders' group, the scale bar, then a slider per axis
/// (medical uses 0x5800…, cull 0x5000…).
const A11Y_ID: u64 = 0x5900_0000_0000_0000;
const A11Y_SLIDERS: u64 = A11Y_ID + 1;
const A11Y_SCALE: u64 = A11Y_ID + 2;
const A11Y_SLIDER: u64 = A11Y_ID + 0x100;

/// Logical width of a slider's track.
const TRACK_W: f32 = 200.0;
/// Longest scale bar, logical pixels.
const SCALE_MAX_PX: f32 = 120.0;

/// What is read for the shown planes: a change re-reads (from the session's cache).
#[derive(Clone, Debug, PartialEq)]
struct Request {
    /// Position on every axis (the composited axis at 0).
    index: Vec<u32>,
    /// The axis composited and how many of its positions.
    composite: Option<(usize, u32)>,
    limits: Limits,
    /// Histogram equalisation needs each plane's table.
    histeq: bool,
    /// Planes are levelled (a height map with levelling on).
    level: bool,
    /// The raw frame is demosaiced with this pattern (one plane read, three channels).
    cfa: Option<Cfa>,
    /// The automatic stretch: limits and midtones from each channel's statistics.
    auto: bool,
    /// The file's display ranges (ImageJ's min/max) per position of the axis given, used as
    /// the limits until a way of finding them is chosen; empty when not.
    ranges: (Option<usize>, Vec<(f64, f64)>),
    /// The plane's centred log power spectrum is shown instead of the plane.
    fft: bool,
    /// A mass spectrometry image shows this m/z window's ion image instead of the total ion
    /// current.
    ion: Option<dsci::IonWindow>,
    /// A thermal image's temperatures are computed under these object parameters, in this
    /// unit.
    thermal: Option<(ThermalConditions, TemperatureUnit)>,
}

/// Science-mode state for the current image.
pub(super) struct Science {
    /// The image the state below belongs to.
    id: Option<ImageId>,
    summary: Option<Arc<ScienceSummary>>,
    /// Position on each axis, and the axis the plane keys step.
    pub(super) index: Vec<u32>,
    axis: usize,
    composite: bool,
    hidden: [bool; MAX_CHANNELS],
    /// A display range set through scripting (`view-set`), over the computed limits of
    /// every plane. This image only.
    range: Option<(f32, f32)>,
    /// Kept across files.
    pub(super) stretch: Stretch,
    pub(super) limits: Limits,
    pub(super) grid: bool,
    /// Plane levelling of height maps.
    pub(super) level: bool,
    /// A way of finding the limits was chosen ([`Action::ScienceLimits`]): files' own display
    /// ranges no longer stand in for them. Kept across files.
    pub(super) limits_chosen: bool,
    /// Relief shading of elevation models. Kept across files.
    pub(super) hillshade: bool,
    /// What the relief shows: the shade, or (the map view's terrain tools) slope or aspect
    /// colours.
    pub(super) relief_kind: Relief,
    /// A colour map chosen for a weather grid (the map view's weather colours), over the
    /// file's; for this image only.
    pub(super) weather: Option<(specular_inspect::ColourMap, Arc<ColourTable>)>,
    /// A stretch was chosen ([`Action::ScienceStretch`]): colour camera frames no longer
    /// default to the automatic one. Kept across files.
    pub(super) stretch_chosen: bool,
    /// Colour camera raw frames show debayered. Kept across files.
    pub(super) debayer: bool,
    /// Blinking subframes in cull mode: every frame is drawn with the limits and midtones of
    /// the first one shown (per channel), so frames compare as they are. Set by cull mode;
    /// kept across files, cleared when the look is changed.
    pub(super) blink_on: bool,
    pub(super) blink: Option<Vec<(f32, f32, f32)>>,
    /// Slides' annotations are hidden ([`Action::ToggleAnnotations`]). Kept across files.
    pub(super) annotations_hidden: bool,
    /// Bayer patterns chosen for files ([`Action::NextBayerPattern`]), over their headers'.
    pub(super) cfa_override: HashMap<PathBuf, Cfa>,
    /// The current image's file.
    path: Option<PathBuf>,
    /// The read in flight, and what the shown planes were read for.
    inflight: Option<Request>,
    shown: Option<Request>,
    planes: Vec<SciencePlane>,
    /// The planes are on the image's GPU texture, and its look is up to date.
    uploaded: bool,
    sent: bool,
    /// Reading failed: no science mode for this image.
    off: bool,
    readout: Vec<String>,
    /// The readout's lines about the point under the pointer (read out to screen readers).
    readout_at: std::ops::Range<usize>,
    /// The sliders as last drawn: axis and track (logical px).
    pub(super) tracks: Vec<(usize, Rect)>,
    /// A slider being dragged.
    drag: Option<usize>,
    /// The grid in stored pixels, for the image and the positions of the further WCS axes.
    grid_cache: Option<(ImageId, Vec<usize>, Arc<Vec<GridLine>>)>,
    /// A colour table the user imported ([`Action::ImportLut`]): single planes show through
    /// it instead of the file's. Kept across files.
    pub(super) user_lut: Option<Arc<ColourTable>>,
    /// The gain of seismic sections (limits symmetric about 0), apart from the other
    /// images' limits. Kept across files.
    pub(super) gain: Limits,
    /// Seismic traces are drawn as wiggles once they are far enough apart. Kept across files.
    pub(super) wiggle: bool,
    /// An unstructured mesh's edges are drawn over its faces. Kept across files.
    pub(super) wireframe: bool,
    /// The mesh edges in view (stored px) for the image, plane position and visible
    /// rectangle (as bits); `None` inside when there are too many to draw.
    edge_cache: Option<(ImageId, [u64; 4], Option<MeshEdges>)>,
    /// The colour table file being chosen and read: its name and table (`None`: not one).
    lut_import: Option<crossbeam_channel::Receiver<(String, Option<Arc<ColourTable>>)>>,
    /// The plane's power spectrum is shown ([`Action::ScienceFft`]).
    fft: bool,
    /// The spectrum under the pointer (an EDS hypermap's pixel), plotted.
    spectrum: Option<Arc<SpectrumPlot>>,
    /// Source catalogues overlaid on sky images, by name ([`Action::ScienceCatalogue`]).
    /// Kept across files.
    pub(super) catalogues: Vec<(String, Arc<specular_decode::science::catalogue::Catalogue>)>,
    /// The catalogue file being chosen and read.
    catalogue_rx: Option<crossbeam_channel::Receiver<sky::Loaded>>,
    /// The catalogues' markers in stored px, for the image, the positions of the further WCS
    /// axes and the number of catalogues.
    marker_cache: Option<sky::MarkerCache>,
    /// The credible-region contours in stored px, for the image and plane.
    contour_cache: Option<(ImageId, Vec<u32>, Arc<Vec<GridLine>>)>,
    /// The ion image shown (mass spectrometry), instead of the total ion current.
    ion: Option<dsci::IonWindow>,
    /// The m/z prompt is open: what is typed.
    ion_prompt: Option<String>,
    /// The ion images' m/z tolerance. Kept across files.
    pub(super) ion_tolerance: dsci::IonTolerance,
    /// The temperature unit chosen ([`Action::ScienceTemperatureUnit`]). Kept across files.
    pub(super) unit_chosen: Option<TemperatureUnit>,
    /// The temperature unit until one is chosen (the settings' units, else the locale's).
    unit_default: TemperatureUnit,
    /// A thermal image's object parameters as edited, over the file's.
    conditions: Option<ThermalConditions>,
    /// The isotherm: above (else below) the level, in degrees Celsius.
    isotherm: Option<(bool, f64)>,
    /// The object parameters prompt is open.
    thermal_prompt: Option<thermal::Prompt>,
    /// The temperature grid being saved: the file's name, or why it failed. Kept across
    /// files.
    export_rx: Option<crossbeam_channel::Receiver<Result<String, String>>>,
    /// A thermal sequence's playback and its measurements over time.
    pub(super) seq: thermal_seq::SeqState,
}

impl Default for Science {
    fn default() -> Self {
        Science {
            id: None,
            summary: None,
            index: Vec::new(),
            axis: 0,
            composite: false,
            hidden: [false; MAX_CHANNELS],
            stretch: Stretch::Linear,
            limits: Limits::ZScale,
            grid: false,
            level: false,
            limits_chosen: false,
            hillshade: false,
            relief_kind: Relief::Shade,
            weather: None,
            range: None,
            stretch_chosen: false,
            debayer: true,
            blink_on: false,
            blink: None,
            annotations_hidden: false,
            cfa_override: HashMap::new(),
            path: None,
            inflight: None,
            shown: None,
            planes: Vec::new(),
            uploaded: false,
            sent: false,
            off: false,
            readout: Vec::new(),
            readout_at: 0..0,
            tracks: Vec::new(),
            drag: None,
            grid_cache: None,
            user_lut: None,
            lut_import: None,
            fft: false,
            spectrum: None,
            ion: None,
            ion_prompt: None,
            ion_tolerance: ion::DEFAULT_TOLERANCE,
            unit_chosen: None,
            unit_default: TemperatureUnit::Celsius,
            conditions: None,
            isotherm: None,
            thermal_prompt: None,
            export_rx: None,
            seq: thermal_seq::SeqState::default(),
            gain: Limits::SEISMIC,
            wiggle: true,
            wireframe: false,
            edge_cache: None,
            catalogues: Vec::new(),
            catalogue_rx: None,
            marker_cache: None,
            contour_cache: None,
        }
    }
}

/// An unstructured mesh's edges in view: segments x0, y0, x1, y1 in stored pixels.
type MeshEdges = Arc<Vec<[f32; 4]>>;

/// One coordinate line: points in stored pixels (or logical px once placed) and its label.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct GridLine {
    pub pts: Vec<(f32, f32)>,
    pub label: String,
}

/// One plane slider as drawn.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct SliderRow {
    pub axis: usize,
    pub name: String,
    /// What the position stands for (an extension's name, a world value), when known.
    pub world: Option<String>,
    pub index: u32,
    pub len: u32,
    pub selected: bool,
}

/// The scale bar: its length on screen (logical px) and what it measures.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ScaleBar {
    pub px: f32,
    pub label: String,
}

/// What is drawn over a science image besides the readout.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Overlay {
    pub sliders: Vec<SliderRow>,
    pub scale: Option<ScaleBar>,
    /// In logical px.
    pub grid: Vec<GridLine>,
    /// The spectrum under the pointer.
    pub spectrum: Option<Arc<SpectrumPlot>>,
    /// Seismic wiggle traces, when shown.
    pub wiggle: Option<Wiggles>,
    /// An unstructured mesh's edges in view, in logical px, when shown.
    pub edges: Vec<[(f32, f32); 2]>,
    /// A slide's annotations, in logical px.
    pub shapes: Vec<super::annotations::ShapeDraw>,
    /// A sky map's credible-region contours, in logical px.
    pub contours: Vec<GridLine>,
    /// Catalogue sources on the image, in logical px, the hovered one's index, and its
    /// fields.
    pub markers: Vec<sky::Marker>,
    pub hovered: Option<usize>,
    pub tip: Option<sky::Tip>,
    /// DS9 regions, SAMP highlights and the sky survey.
    pub astro: astro::AstroOverlay,
}

/// A pixel's spectrum for the plot: its values against their x positions (energy,
/// wavelength, m/z).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct SpectrumPlot {
    /// The image pixel it belongs to, and the plane.
    pub at: (u32, u32),
    pub index: Vec<u32>,
    pub spectrum: dsci::Spectrum,
    /// What the end labels carry after the value: the x unit, else the axis name ("m/z").
    pub unit: String,
}

/// Seismic traces as wiggles (logical px): each trace's line, and the positive lobes
/// filled (variable area) as short strokes from the trace's zero line.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Wiggles {
    pub lines: Vec<Vec<(f32, f32)>>,
    pub fill: Vec<[(f32, f32); 2]>,
    /// Stroke width of the fill (logical px).
    pub fill_width: f32,
}

/// Traces closer than this (logical px) are not drawn as wiggles.
pub(super) const WIGGLE_MIN_PX: f32 = 4.0;
/// Fill strokes drawn at most.
const WIGGLE_MAX_FILL: f32 = 150_000.0;

/// The wiggles of the traces (columns) of `plane` within stored pixels `x0..x1`, rows
/// `y0..y1`, at `px` logical px per stored pixel: a trace's excursion is one trace spacing
/// at `clip` (clipped there). `to_screen` maps stored pixels to logical px. `None` when the
/// traces are closer than [`WIGGLE_MIN_PX`].
pub(super) fn wiggles(
    plane: &NdPlane,
    clip: f32,
    (x0, y0, x1, y1): (f32, f32, f32, f32),
    px: f32,
    to_screen: impl Fn((f32, f32)) -> (f32, f32),
) -> Option<Wiggles> {
    if px < WIGGLE_MIN_PX || clip.is_nan() || clip <= 0.0 {
        return None;
    }
    let (w, h) = (plane.width as usize, plane.height as usize);
    if plane.values.len() != w * h || w == 0 || h == 0 {
        return None;
    }
    let tx0 = (x0.floor().max(0.0) as usize).min(w);
    let tx1 = (x1.ceil().max(0.0) as usize).min(w);
    let ky0 = (y0.floor().max(0.0) as usize).min(h);
    let ky1 = (y1.ceil().max(0.0) as usize + 1).min(h);
    if tx0 >= tx1 || ky0 >= ky1 {
        return None;
    }
    let dx = |v: f32| (v / clip).clamp(-1.0, 1.0);
    let at = |x: usize, k: usize| plane.values[k * w + x];
    let mut out = Wiggles::default();
    // Fill rows: every 2 logical px, fewer when that would be too many strokes.
    let rows = (ky1 - ky0) as f32;
    let traces = (tx1 - tx0) as f32;
    let ds = (2.0 / px).max(traces * rows / WIGGLE_MAX_FILL);
    out.fill_width = ds * px + 0.5;
    for x in tx0..tx1 {
        let base = x as f32 + 0.5;
        let mut line: Vec<(f32, f32)> = Vec::new();
        for k in ky0..ky1 {
            let v = at(x, k);
            if !v.is_finite() {
                if line.len() > 1 {
                    out.lines.push(std::mem::take(&mut line));
                }
                line.clear();
                continue;
            }
            line.push(to_screen((base + dx(v), k as f32 + 0.5)));
        }
        if line.len() > 1 {
            out.lines.push(line);
        }
        // Variable area: the positive side, sampled every `ds` rows (linear between samples).
        let mut t = ky0 as f32;
        while t <= (ky1 - 1) as f32 {
            let k = t as usize;
            let f = t - k as f32;
            let a = at(x, k);
            let b = if k + 1 < ky1 { at(x, k + 1) } else { a };
            let v = a + (b - a) * f;
            if v > 0.0 && v.is_finite() {
                let y = t + 0.5;
                out.fill
                    .push([to_screen((base, y)), to_screen((base + dx(v), y))]);
            }
            t += ds;
        }
    }
    Some(out)
}

/// The readout of an unstructured mesh at stored pixel `x`, `y`: the face (and the node
/// nearest when the values sit on nodes) and the exact value there, interpolated across the
/// face for node values, rather than the painted pixel's.
fn mesh_lines(
    mesh: &dyn dsci::MeshProbe,
    x: f64,
    y: f64,
    index: &[u32],
    unit: Option<&str>,
) -> Vec<String> {
    let Some(hit) = mesh.at(x, y, index) else {
        return vec![t!("science-mesh-outside")];
    };
    let face = hit.face.to_string();
    let mut lines = vec![match hit.node {
        Some(n) => t!("science-mesh-face-node", face = face, node = n.to_string()),
        None => t!("science-mesh-face", face = face),
    }];
    if mesh.facts().location.is_some() {
        let v = value_text(hit.value as f32);
        lines.push(match unit.map(str::trim).filter(|u| !u.is_empty()) {
            Some(u) => t!("science-value-unit", value = v, unit = u.to_owned()),
            None => t!("science-value", value = v),
        });
    }
    lines
}

/// Most mesh edges drawn at once: past it the wireframe waits for the user to zoom in.
pub(super) const MAX_MESH_EDGES: usize = 80_000;

/// Draws an unstructured mesh's edges (logical px), clipped to `clip`.
pub(super) fn draw_edges(dl: &mut DrawList, edges: &[[(f32, f32); 2]], clip: Rect) {
    let ink = Color::rgba(0.0, 0.0, 0.0, 0.6);
    dl.push_clip(clip);
    for e in edges {
        dl.line(e[0], e[1], 1.0, ink);
    }
    dl.pop_clip();
}

/// Draws wiggle traces (logical px), clipped to `clip`.
pub(super) fn draw_wiggles(dl: &mut DrawList, w: &Wiggles, clip: Rect) {
    let ink = Color::rgba(0.0, 0.0, 0.0, 0.85);
    dl.push_clip(clip);
    for s in &w.fill {
        dl.line(s[0], s[1], w.fill_width, ink);
    }
    for l in &w.lines {
        for p in l.windows(2) {
            dl.line(p[0], p[1], 1.0, ink);
        }
    }
    dl.pop_clip();
}

/// Default colours of `n` composited channels: grey for one, magenta and green for two,
/// red, green and blue for three, yellow for a fourth, then cyan, magenta, orange, violet
/// and dimmer shades up to [`MAX_CHANNELS`] (added, so many channels need dimmer inks).
pub(super) fn channel_colours(n: usize) -> Vec<[f32; 3]> {
    match n {
        0 | 1 => vec![[1.0; 3]],
        2 => vec![[1.0, 0.0, 1.0], [0.0, 1.0, 0.0]],
        _ => [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 0.5, 0.0],
            [0.5, 0.0, 1.0],
            [0.5, 1.0, 0.0],
            [0.0, 0.5, 1.0],
            [1.0, 0.0, 0.5],
            [0.0, 1.0, 0.5],
            [0.6, 0.6, 0.6],
            [0.6, 0.3, 0.0],
            [0.0, 0.4, 0.4],
            [0.4, 0.0, 0.4],
        ][..n.min(MAX_CHANNELS)]
            .to_vec(),
    }
}

/// The set bits of a data quality value, "0, 3, 31"; `None` for 0 and for anything that is
/// not a whole number of 64 bits or fewer.
pub(super) fn flag_bits(v: f32) -> Option<String> {
    if !(v > 0.0 && v.fract() == 0.0 && v < 18_446_744_073_709_551_616.0) {
        return None;
    }
    let n = v as u64;
    let bits: Vec<String> = (0..64)
        .filter(|b| n >> b & 1 == 1)
        .map(|b: u32| b.to_string())
        .collect();
    Some(bits.join(", "))
}

/// The name of class `n` of a categorical plane's scheme.
pub(super) fn class_name(scheme: ClassScheme, n: u8) -> Option<String> {
    Some(match (scheme, n) {
        (ClassScheme::Sentinel2Scl, 0) => t!("s2-class-no-data"),
        (ClassScheme::Sentinel2Scl, 1) => t!("s2-class-defective"),
        (ClassScheme::Sentinel2Scl, 2) => t!("s2-class-dark"),
        (ClassScheme::Sentinel2Scl, 3) => t!("s2-class-cloud-shadow"),
        (ClassScheme::Sentinel2Scl, 4) => t!("s2-class-vegetation"),
        (ClassScheme::Sentinel2Scl, 5) => t!("s2-class-not-vegetated"),
        (ClassScheme::Sentinel2Scl, 6) => t!("s2-class-water"),
        (ClassScheme::Sentinel2Scl, 7) => t!("s2-class-unclassified"),
        (ClassScheme::Sentinel2Scl, 8) => t!("s2-class-cloud-medium"),
        (ClassScheme::Sentinel2Scl, 9) => t!("s2-class-cloud-high"),
        (ClassScheme::Sentinel2Scl, 10) => t!("s2-class-cirrus"),
        (ClassScheme::Sentinel2Scl, 11) | (ClassScheme::Sentinel2L1cMask, 3) => {
            t!("s2-class-snow")
        }
        (ClassScheme::Sentinel2L1cMask, 0) => t!("s2-class-clear"),
        (ClassScheme::Sentinel2L1cMask, 1) => t!("s2-class-opaque-cloud"),
        (ClassScheme::Sentinel2L1cMask, 2) => t!("s2-class-cirrus"),
        _ => return None,
    })
}

/// A value for the readout: four significant digits, scientific notation for very small
/// or large magnitudes; a true minus.
pub(super) fn value_text(v: f32) -> String {
    if v.is_nan() {
        return "NaN".to_owned();
    }
    let a = v.abs();
    let s = if v == 0.0 {
        "0".to_owned()
    } else if v.is_infinite() {
        if v > 0.0 { "∞" } else { "-∞" }.to_owned()
    } else if !(1e-3..1e6).contains(&a) {
        format!("{v:.3e}")
    } else if (v - v.round()).abs() < 1e-6 * a.max(1.0) && a >= 1.0 {
        format!("{}", v.round() as i64)
    } else {
        let decimals = (3 - a.log10().floor() as i32).clamp(0, 6) as usize;
        let s = format!("{v:.decimals$}");
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            s
        }
    };
    s.replace('-', "\u{2212}")
}

/// An axis' world value (a frequency, a wavelength, a time): seven significant digits, so
/// neighbouring planes read apart; a true minus.
pub(super) fn world_text(v: f64) -> String {
    let a = v.abs();
    let s = if v == 0.0 || !v.is_finite() {
        return value_text(v as f32);
    } else if !(1e-3..1e7).contains(&a) {
        let s = format!("{v:.6e}");
        match s.split_once('e') {
            Some((m, e)) if m.contains('.') => {
                format!("{}e{e}", m.trim_end_matches('0').trim_end_matches('.'))
            }
            _ => s,
        }
    } else {
        let decimals = (6 - a.log10().floor() as i32).clamp(0, 9) as usize;
        trimmed(v, decimals)
    };
    s.replace('-', "\u{2212}")
}

/// A number with up to `max` decimals, trailing zeros dropped.
pub(super) fn trimmed(x: f64, max: usize) -> String {
    let s = format!("{x:.max$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        s
    }
}

impl Request {
    /// The file's display range for the plane at `position`, when it stands in for the
    /// limits.
    fn file_range(&self, position: &[u32]) -> Option<(f32, f32)> {
        let (axis, ranges) = &self.ranges;
        let k = axis
            .and_then(|a| position.get(a))
            .map_or(0, |&i| i as usize);
        ranges
            .get(k)
            .or(ranges.first())
            .filter(|(lo, hi)| lo.is_finite() && hi.is_finite() && hi > lo)
            .map(|&(lo, hi)| (lo as f32, hi as f32))
    }
}

/// The name of a stretch.
pub(super) fn stretch_name(s: Stretch) -> String {
    match s {
        Stretch::Linear => t!("science-stretch-linear"),
        Stretch::Sqrt => t!("science-stretch-sqrt"),
        Stretch::Log => t!("science-stretch-log"),
        Stretch::Asinh => t!("science-stretch-asinh"),
        Stretch::HistEq => t!("science-stretch-histeq"),
        Stretch::Auto => t!("science-stretch-auto"),
    }
}

/// A read plane as `req` shows it: levelled (a height map), then its centred log power
/// spectrum ([`specular_fft::power_spectrum`]) when the spectrum is asked for.
fn transform(plane: Arc<NdPlane>, req: &Request) -> Result<Arc<NdPlane>, String> {
    let mut plane = plane;
    if req.level {
        plane = Arc::new(NdPlane {
            values: dsci::level_plane(&plane.values, plane.width as usize),
            ..(*plane).clone()
        });
    }
    if req.fft {
        let t0 = std::time::Instant::now();
        let values = specular_fft::power_spectrum(
            &plane.values,
            plane.width as usize,
            plane.height as usize,
        )
        .ok_or_else(|| t!("error-reason-no-image"))?;
        specular_log::debug!(
            target: specular_log::DECODE,
            "power spectrum {}×{}: {:.1} ms",
            plane.width,
            plane.height,
            t0.elapsed().as_secs_f64() * 1e3
        );
        plane = Arc::new(NdPlane {
            values,
            ..(*plane).clone()
        });
    }
    Ok(plane)
}

/// The spatial frequency and period under the pointer on a power spectrum: in 1/unit of the
/// plane's calibration (a metric length in the SI prefix of the period), else in 1/px.
fn fft_lines(s: &dsci::ScienceSummary, x: f32, y: f32) -> Vec<String> {
    let (w, h) = (s.width as usize, s.height as usize);
    let (i, j) = (f64::from(x.floor()), f64::from(y.floor()));
    let (sx, sy, unit) = match &s.pixel {
        Some(c) if c.x.step != 0.0 && c.y.step != 0.0 && c.unit_x.trim() == c.unit_y.trim() => {
            (c.x.step.abs(), c.y.step.abs(), c.unit_x.trim().to_owned())
        }
        _ => (1.0, 1.0, "px".to_owned()),
    };
    let u = specular_fft::frequency(i, w, sx);
    let v = specular_fft::frequency(j, h, sy);
    let f = u.hypot(v);
    let mut lines = Vec::new();
    // Frequencies in 1/(the unit the period is written in).
    let (scale, shown) = match metres(&unit) {
        Some(m) if f > 0.0 => {
            let (_, su) = si_length(m / f);
            let per = metres(su).unwrap_or(1.0);
            (per / m, su.to_owned())
        }
        _ => (1.0, unit.clone()),
    };
    let ft = |q: f64| significant(q * scale, 4);
    lines.push(t!(
        "science-fft-frequency",
        u = ft(u),
        v = ft(v),
        unit = format!("1/{shown}")
    ));
    if f > 0.0 {
        let period = format!("{} {shown}", significant(1.0 / (f * scale), 4));
        lines.push(t!("science-fft-period", period = period));
    }
    lines
}

/// `v` to `n` significant digits, without trailing zeros ("0.004688", "142.7").
fn significant(v: f64, n: i32) -> String {
    if v == 0.0 || !v.is_finite() {
        return world_text(v);
    }
    let decimals = (n - 1 - v.abs().log10().floor() as i32).clamp(0, 12) as usize;
    let s = format!("{v:.decimals$}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.')
    } else {
        &s
    };
    s.replace('-', "\u{2212}")
}

/// The crystal orientation under the pointer: Bunge Euler angles in degrees and the phase.
fn euler_lines(o: &dsci::Orientations, x: f32, y: f32, w: u32, h: u32) -> Vec<String> {
    let ox = (f64::from(x) * o.width as f64 / f64::from(w.max(1))) as usize;
    let oy = (f64::from(y) * o.height as f64 / f64::from(h.max(1))) as usize;
    let Some((e, phase)) = o.at(ox, oy) else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    if e.iter().all(|a| a.is_finite()) {
        let d = |a: f32| format!("{}°", specular_i18n::fixed(f64::from(a).to_degrees(), 1));
        lines.push(t!(
            "science-euler",
            phi1 = d(e[0]),
            phi = d(e[1]),
            phi2 = d(e[2])
        ));
    }
    if let Some(p) = phase {
        lines.push(t!(
            "science-phase",
            phase = p.name.clone(),
            symmetry = p.symmetry.clone()
        ));
    }
    lines
}

/// The automatic stretch's samples per colour of a raw frame.
const CFA_SAMPLES: usize = 1 << 20;

/// The planes to show for one read `plane` under `req`: itself with its limits (and table),
/// or, for a raw colour frame, three channels (red, green, blue) sharing it, each with the
/// limits of its own colour's samples.
fn prepare(plane: Arc<NdPlane>, req: &Request, position: &[u32]) -> Vec<SciencePlane> {
    let width = plane.width as usize;
    let file = req.file_range(position);
    let one = |values: &[f32], width: usize| {
        let (limits, mid) = if req.auto {
            match nd::stf(values) {
                Some((lo, hi, mid)) => (Some((lo, hi)), mid),
                None => (None, 0.5),
            }
        } else {
            (file.or_else(|| req.limits.compute(values, width)), 0.5)
        };
        let lut = match limits {
            Some((lo, hi)) if req.histeq => nd::histeq_lut(values, lo, hi),
            _ => Vec::new(),
        };
        (limits, lut, mid)
    };
    match req.cfa {
        Some(cfa) => (0..3)
            .map(|c| {
                let s = cfa.samples(&plane.values, width, c, CFA_SAMPLES);
                // The samples are rows of one colour: zscale's lines are their halves.
                let (limits, lut, mid) = one(&s, (width / 2).max(1));
                SciencePlane {
                    plane: plane.clone(),
                    limits,
                    lut,
                    mid,
                }
            })
            .collect(),
        None => {
            let (limits, lut, mid) = one(&plane.values, width);
            vec![SciencePlane {
                plane,
                limits,
                lut,
                mid,
            }]
        }
    }
}

/// The name of a way of finding the limits.
pub(super) fn limits_name(l: Limits) -> String {
    match l {
        Limits::ZScale => t!("science-limits-zscale"),
        Limits::MinMax => t!("science-limits-minmax"),
        Limits::Percentile(p) => t!(
            "science-limits-percentile",
            percent = trimmed(f64::from(p), 2)
        ),
        Limits::Symmetric(p) => t!(
            "science-limits-symmetric",
            percent = trimmed(f64::from(p), 2)
        ),
    }
}

/// The seismic readout of column `x`, row `y` of the plane at `index`: the cell (volumes),
/// the trace and CDP, X / Y, and the time or depth.
pub(super) fn seismic_lines(s: &dsci::Seismic, index: &[u32], x: u32, y: u32) -> Vec<String> {
    let p = s.at(index, x, y);
    let mut lines = Vec::new();
    if let (Some(il), Some(xl)) = (p.inline, p.crossline) {
        lines.push(t!(
            "science-seismic-cell",
            inline = il.to_string(),
            crossline = xl.to_string()
        ));
    }
    match p.trace {
        Some(t) => lines.push(t!(
            "science-seismic-trace",
            trace = t.to_string(),
            cdp = p.cdp.map_or_else(String::new, |c| c.to_string())
        )),
        None => lines.push(t!("science-seismic-gap")),
    }
    if let Some([x, y]) = p.xy {
        lines.push(t!(
            "science-seismic-xy",
            x = trimmed(x, 2),
            y = trimmed(y, 2)
        ));
    }
    let time = trimmed(p.time, 3);
    lines.push(if s.depth {
        t!("science-seismic-depth", depth = time, unit = s.unit.clone())
    } else {
        t!("science-seismic-time", time = time)
    });
    lines
}

/// The name of an axis for the sliders and the readout.
fn axis_name(s: &ScienceSummary, axis: usize) -> String {
    let a = &s.axes[axis];
    if !a.name.trim().is_empty() {
        return a.name.trim().to_owned();
    }
    match a.kind {
        AxisKind::Cube => t!("science-axis-unnamed"),
        AxisKind::Extension => t!("science-axis-extension"),
        AxisKind::Channel => t!("science-axis-channel"),
        AxisKind::Field => t!("science-axis-field"),
        AxisKind::Bin => t!("science-axis-bin"),
        AxisKind::Band => t!("science-axis-band"),
        AxisKind::Part => t!("science-axis-part"),
        AxisKind::Frame => t!("science-axis-frame"),
    }
}

/// What position `index` on `axis` stands for: its label, else its world value and unit.
fn axis_world(s: &ScienceSummary, axis: usize, index: u32) -> Option<String> {
    let a = &s.axes[axis];
    match a.kind {
        AxisKind::Bin => {
            let l = a.labels.get(index as usize)?;
            return Some(t!("science-bin-size", size = l.trim().to_owned()));
        }
        AxisKind::Band if a.labels.get(index as usize)?.trim().is_empty() => {
            return Some(t!("science-band-all"));
        }
        // Complex values: magnitude, real, imaginary, phase.
        AxisKind::Part => {
            return Some(match index {
                0 => t!("science-part-magnitude"),
                1 => t!("science-part-real"),
                2 => t!("science-part-imaginary"),
                3 => t!("science-part-phase"),
                _ => t!("science-part-db"),
            });
        }
        _ => {}
    }
    if let Some(l) = a
        .labels
        .get(index as usize)
        .filter(|l| !l.trim().is_empty())
    {
        return Some(l.trim().to_owned());
    }
    let wcs = a
        .wcs_axis
        .and_then(|k| s.wcs.as_ref()?.axis_value(k, index as usize));
    let v = wcs.or_else(|| Some(a.linear?.at(f64::from(index))))?;
    if metres(&a.unit).is_some() {
        return Some(quantity_text(v, &a.unit));
    }
    let v = world_text(v);
    Some(if a.unit.trim().is_empty() {
        v
    } else {
        format!("{v} {}", specular_i18n::unit_symbol(a.unit.trim()))
    })
}

/// Metres per `unit` for a length unit ("nm", "µm", "Å"…); `None` for anything else.
pub(super) fn metres(unit: &str) -> Option<f64> {
    Some(match unit.trim() {
        "m" => 1.0,
        "cm" => 1e-2,
        "mm" => 1e-3,
        "µm" | "μm" | "um" | "micron" | "microns" => 1e-6,
        "nm" => 1e-9,
        "Å" | "A" | "Angstrom" | "angstrom" | "Ang" => 1e-10,
        "pm" => 1e-12,
        "fm" => 1e-15,
        _ => return None,
    })
}

/// A length in metres with the SI prefix that keeps it between 1 and 1000 (metres at most).
fn si_length(m: f64) -> (f64, &'static str) {
    const PREFIXES: [(f64, &str); 5] = [
        (1.0, "m"),
        (1e-3, "mm"),
        (1e-6, "µm"),
        (1e-9, "nm"),
        (1e-12, "pm"),
    ];
    let a = m.abs();
    PREFIXES
        .iter()
        .find(|(f, _)| a >= *f * 0.999_999)
        .map_or((m / 1e-12, "pm"), |&(f, u)| (m / f, u))
}

/// An area of `v` square `unit`s: µm² or mm² for metric units, else as the unit gives it.
pub(crate) fn area_text(v: f64, unit: &str) -> String {
    match metres(unit) {
        Some(f) if v.is_finite() => {
            let um2 = v * (f / 1e-6) * (f / 1e-6);
            if um2.abs() >= 1e6 {
                format!(
                    "{} {}",
                    world_text(um2 / 1e6),
                    specular_i18n::unit_symbol("mm²")
                )
            } else {
                format!("{} µm²", world_text(um2))
            }
        }
        _ if unit.trim().is_empty() => format!("{} px²", world_text(v)),
        _ => format!("{} {}²", world_text(v), unit.trim()),
    }
}

/// A physical quantity: lengths with a fitting SI prefix ("1.25 µm"), anything else as a
/// world value and its unit.
pub(crate) fn quantity_text(v: f64, unit: &str) -> String {
    match metres(unit) {
        Some(f) if v != 0.0 && v.is_finite() => {
            let (x, u) = si_length(v * f);
            format!("{} {}", world_text(x), specular_i18n::unit_symbol(u))
        }
        Some(_) => format!(
            "{} {}",
            world_text(v),
            specular_i18n::unit_symbol(unit.trim())
        ),
        None if unit.trim().is_empty() => world_text(v),
        None => format!("{} {}", world_text(v), unit.trim()),
    }
}

/// The positions of the WCS axes past the first two, from the positions on the image's
/// axes.
fn wcs_rest(s: &ScienceSummary, w: &Wcs, index: &[u32]) -> Vec<usize> {
    let mut rest = vec![0; w.axes.len().saturating_sub(2)];
    for (a, &i) in s.axes.iter().zip(index) {
        if let Some(k) = a.wcs_axis.filter(|&k| k >= 2)
            && let Some(r) = rest.get_mut(k - 2)
        {
            *r = i as usize;
        }
    }
    rest
}

/// The sky position lines of the readout: right ascension and declination sexagesimal then
/// in degrees, or another frame's longitude and latitude.
pub(super) fn sky_lines(frame: &Frame, lng: f64, lat: f64) -> Vec<String> {
    let lng = lng.rem_euclid(360.0);
    match frame {
        Frame::Equatorial { .. } => vec![
            t!("science-radec", ra = wcs::hms(lng), dec = wcs::dms(lat)),
            t!(
                "science-radec-deg",
                ra = specular_i18n::fixed_plain(lng, 5),
                dec = signed(lat, 5)
            ),
        ],
        _ => vec![t!(
            "science-lonlat",
            lon = specular_i18n::fixed_plain(lng, 4),
            lat = signed(lat, 4)
        )],
    }
}

/// The scale bar for `arcsec_px` arcseconds per image pixel at `px_per_pixel` logical px per
/// image pixel: the longest round length that fits [`SCALE_MAX_PX`].
pub(super) fn scale_bar(arcsec_px: f64, px_per_pixel: f64) -> Option<ScaleBar> {
    const ARCSEC: [f64; 27] = [
        0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 20.0,
        30.0, 60.0, 120.0, 300.0, 600.0, 900.0, 1200.0, 1800.0, 3600.0, 7200.0, 18000.0, 36000.0,
    ];
    if !(arcsec_px > 0.0 && px_per_pixel > 0.0) {
        return None;
    }
    let per_px = arcsec_px / px_per_pixel;
    let max = per_px * f64::from(SCALE_MAX_PX);
    let len = ARCSEC
        .iter()
        .copied()
        .rfind(|&l| l <= max)
        .or_else(|| (max > ARCSEC[ARCSEC.len() - 1]).then_some(ARCSEC[ARCSEC.len() - 1]))?;
    let px = (len / per_px) as f32;
    if px < 8.0 {
        return None;
    }
    let label = if len < 60.0 {
        t!("science-scale-arcsec", value = trimmed(len, 3))
    } else if len < 3600.0 {
        t!("science-scale-arcmin", value = trimmed(len / 60.0, 2))
    } else {
        t!("science-scale-deg", value = trimmed(len / 3600.0, 2))
    };
    Some(ScaleBar { px, label })
}

/// "WCS not supported: <reason>", in the readout and in place of the scale bar.
pub(super) fn wcs_unsupported(why: &wcs::Unsupported) -> String {
    t!("science-wcs-unsupported", reason = why.to_string())
}

/// The scale bar for images with a physical pixel size (electron microscopy and the like) and
/// no sky coordinates: the longest 1-2-5 length that fits [`SCALE_MAX_PX`] at `px_per_pixel`
/// logical px per image pixel, metric lengths with the SI prefix that suits them (0.5 nm,
/// 200 µm), other units as the file names them.
pub(super) fn linear_scale_bar(step: f64, unit: &str, px_per_pixel: f64) -> Option<ScaleBar> {
    if !(step > 0.0 && step.is_finite() && px_per_pixel > 0.0) {
        return None;
    }
    let metres = metres(unit);
    // In metres for metric lengths, else in the file's unit.
    let per_px = step * metres.unwrap_or(1.0) / px_per_pixel;
    let max = per_px * f64::from(SCALE_MAX_PX);
    if !max.is_finite() || max <= 0.0 {
        return None;
    }
    let decade = 10f64.powi(max.log10().floor() as i32);
    let len = [5.0, 2.0, 1.0]
        .into_iter()
        .map(|m| m * decade)
        .find(|&l| l <= max * (1.0 + 1e-9))?;
    let px = (len / per_px) as f32;
    if px < 8.0 {
        return None;
    }
    let (value, unit) = match metres {
        Some(_) => {
            let (scale, name) = [
                (1e3, "km"),
                (1.0, "m"),
                (1e-3, "mm"),
                (1e-6, "µm"),
                (1e-9, "nm"),
            ]
            .into_iter()
            .find(|(s, _)| len >= s * (1.0 - 1e-9))
            // Below a nanometre stays in nm (0.2 nm), as microscopy labels it.
            .unwrap_or((1e-9, "nm"));
            (len / scale, name.to_owned())
        }
        None => (len, unit.trim().to_owned()),
    };
    // A unitless axis is the number alone: the message's space would survive `trim` inside
    // the bidi isolates and double the gap before an axis title.
    let label = if unit.is_empty() {
        trimmed(value, 6)
    } else if unit == "mm" {
        // As the measuring tools write millimetres (Hebrew "מ״מ").
        super::measure_tool::length_with_unit(trimmed(value, 6), &unit)
    } else {
        t!(
            "science-scale-length",
            value = trimmed(value, 6),
            unit = specular_i18n::unit_symbol(&unit)
        )
    };
    Some(ScaleBar { px, label })
}

/// The readout of a ROOT histogram at displayed pixel (`x`, `y`): each axis's bin (its
/// label, or its range in the axis's unit, and its number); for a graph, the point.
fn root_lines(r: &RootFacts, x: u32, y: u32, index: &[u32]) -> Vec<String> {
    let bins = r.bins_at(x, y, index);
    if !r.points.is_empty() {
        let Some(&(px, py)) = bins
            .first()
            .and_then(|&b| r.points.get((b as usize).checked_sub(1)?))
        else {
            return Vec::new();
        };
        let a = r.axes.first();
        return vec![t!(
            "science-root-point",
            x = quantity_text(px, a.map_or("", |a| a.unit.as_str())),
            y = world_text(py)
        )];
    }
    r.axes
        .iter()
        .zip(&bins)
        .zip(["x", "y", "z"])
        .filter_map(|((a, &b), name)| {
            let range = match a.label(b) {
                Some(l) => l.to_owned(),
                None => {
                    let (lo, hi) = a.bin_range(b)?;
                    format!(
                        "{} – {}",
                        quantity_text(lo, &a.unit),
                        quantity_text(hi, &a.unit)
                    )
                }
            };
            let axis = if a.title.trim().is_empty() {
                name.to_owned()
            } else {
                a.title.clone()
            };
            Some(t!(
                "science-root-bin",
                axis = axis,
                range = range,
                bin = i64::from(b)
            ))
        })
        .collect()
}

/// The readout of a cytometry plot at stored pixel (`x`, `y`): the parameters' values there
/// and the events in the bin.
fn cytometry_lines(
    c: &dsci::CytometryFacts,
    index: &[u32],
    x: f64,
    y: f64,
    count: Option<f32>,
) -> Vec<String> {
    let mut lines = Vec::new();
    let v = |v: f64| significant(v, 4);
    match c.values_at(index, x, y) {
        (Some((xn, xv)), Some((yn, yv))) => lines.push(t!(
            "science-cytometry-at",
            x = xn,
            xv = v(xv),
            y = yn,
            yv = v(yv)
        )),
        (Some((xn, xv)), None) => lines.push(t!("science-cytometry-at-x", x = xn, xv = v(xv))),
        _ => {}
    }
    if let Some(n) = count.filter(|n| n.is_finite()) {
        lines.push(t!("science-cytometry-events", count = n.round() as i64));
    }
    lines
}

/// The notes under a cytometry plot's readout: its scales and compensation.
fn cytometry_notes(c: &dsci::CytometryFacts, index: &[u32]) -> Vec<String> {
    let (xs, ys) = c.scale_names(index);
    let mut lines = vec![if ys.is_empty() {
        t!("science-cytometry-scale-x", x = xs)
    } else {
        t!("science-cytometry-scales", x = xs, y = ys)
    }];
    match (&c.compensation, index.get(4).copied().unwrap_or(0)) {
        (Some(k), 0) => lines.push(t!("science-cytometry-compensated", keyword = k.clone())),
        (Some(_), _) => lines.push(t!("science-cytometry-uncompensated")),
        (None, _) => {}
    }
    lines
}

/// A cytometry plot's axis ticks as grid lines (stored pixels), labelled on the bottom and
/// left edges.
fn cytometry_ticks(c: &dsci::CytometryFacts, index: &[u32]) -> Vec<GridLine> {
    c.ticks(index)
        .into_iter()
        .map(|(a, b, label)| GridLine {
            // A point just inside the edge carries the label there.
            pts: vec![a, (a.0 + 0.03 * (b.0 - a.0), a.1 + 0.03 * (b.1 - a.1)), b],
            label,
        })
        .collect()
}

/// The notes under a ROOT histogram's readout: hidden flow bins and resampling.
fn root_notes(r: &RootFacts) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some((n, sum)) = r.flow {
        lines.push(t!(
            "science-root-flow",
            count = n as i64,
            sum = significant(sum, 4)
        ));
    }
    if r.resampled {
        lines.push(t!("science-root-resampled"));
    }
    lines
}

/// The scale bar of a ROOT histogram: a length along its x axis, in the axis's unit and
/// named by its title.
fn root_scale_bar(r: &RootFacts, px_per_pixel: f64) -> Option<ScaleBar> {
    let a = r.axes.first().filter(|_| r.points.is_empty())?;
    let bar = linear_scale_bar(a.pixel_step()?, &a.unit, px_per_pixel)?;
    if a.title.trim().is_empty() {
        return Some(bar);
    }
    Some(ScaleBar {
        px: bar.px,
        label: t!(
            "science-scale-axis",
            length = bar.label.trim().to_owned(),
            axis = a.title.clone()
        ),
    })
}

/// A round grid step (degrees) for a span of `span` degrees: at most about five lines.
/// Right ascension steps in time (seconds, minutes, hours of 15°).
fn grid_step(span: f64, time: bool) -> f64 {
    let steps: &[f64] = if time {
        &[
            1.0, 2.0, 5.0, 10.0, 15.0, 20.0, 30.0, 60.0, 120.0, 300.0, 600.0, 900.0, 1200.0,
            1800.0, 3600.0, 7200.0, 10800.0, 14400.0, 21600.0, 28800.0, 43200.0,
        ]
    } else {
        &[
            1.0, 2.0, 5.0, 10.0, 15.0, 20.0, 30.0, 60.0, 120.0, 300.0, 600.0, 900.0, 1200.0,
            1800.0, 3600.0, 7200.0, 18000.0, 36000.0, 54000.0, 72000.0, 108000.0, 162000.0,
        ]
    };
    // Seconds of time are 15″.
    let unit = if time { 15.0 / 3600.0 } else { 1.0 / 3600.0 };
    steps
        .iter()
        .map(|s| s * unit)
        .find(|s| span / s <= 5.0)
        .unwrap_or(if time { 180.0 } else { 45.0 })
}

/// A grid line's label at `v` degrees for step `step`.
fn grid_label(frame: &Frame, v: f64, step: f64, lat: bool) -> String {
    let eq = matches!(frame, Frame::Equatorial { .. });
    if eq && !lat {
        let h = wcs::hms(v);
        if step >= 15.0 {
            format!("{}h", &h[..2])
        } else if step >= 15.0 / 60.0 {
            h[..5].to_owned()
        } else {
            h[..8].to_owned()
        }
    } else if eq {
        let d = wcs::dms(v);
        if step >= 1.0 {
            format!("{}°", &d[..3])
        } else if step >= 1.0 / 60.0 {
            d[..6].to_owned()
        } else {
            d[..9].to_owned()
        }
        .replace('-', "\u{2212}")
    } else {
        format!("{}°", trimmed(v.rem_euclid(360.0), 4)).replace('-', "\u{2212}")
    }
}

/// The resolution rings of a `w × h` diffraction frame in stored pixels, each labelled with
/// its d-spacing where it first enters the image.
pub(super) fn ring_lines(g: &DiffractionGeometry, w: u32, h: u32) -> Vec<GridLine> {
    let mut out = Vec::new();
    for r in g.rings(w, h) {
        let label = t!("science-ring", d = value_text(r.d as f32));
        for (k, seg) in r.segments.into_iter().enumerate() {
            out.push(GridLine {
                pts: seg.into_iter().map(|(x, y)| (x as f32, y as f32)).collect(),
                label: if k == 0 { label.clone() } else { String::new() },
            });
        }
    }
    out
}

/// The scattering angle, q and d under stored pixel coordinates `(x, y)` of a diffraction
/// frame whose header gives the whole geometry.
pub(super) fn scatter_line(g: &DiffractionGeometry, x: f64, y: f64) -> Option<String> {
    let s = g.at(x, y)?;
    Some(t!(
        "science-scatter",
        q = value_text(s.q as f32),
        d = value_text(s.d as f32),
        tth = value_text(s.two_theta as f32)
    ))
}

/// The coordinate grid of a `w × h` image in stored pixels: lines of constant longitude and
/// latitude at round steps, each split where it leaves the projection, labelled where it
/// first enters the image.
pub(super) fn grid(wc: &Wcs, w: u32, h: u32, rest: &[usize]) -> Vec<GridLine> {
    let Some(c) = &wc.celestial else {
        return Vec::new();
    };
    let (fw, fh) = (f64::from(w), f64::from(h));
    let Some((lng_c, _)) = wc.sky_at(fw / 2.0, fh / 2.0, rest) else {
        return Vec::new();
    };
    // The image's extent in the sky, from a lattice over it.
    let n = 16;
    let (mut d0, mut d1, mut b0, mut b1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for j in 0..=n {
        for i in 0..=n {
            let x = fw * f64::from(i) / f64::from(n);
            let y = fh * f64::from(j) / f64::from(n);
            if let Some((l, b)) = wc.sky_at(x, y, rest) {
                let d = (l - lng_c + 540.0).rem_euclid(360.0) - 180.0;
                d0 = d0.min(d);
                d1 = d1.max(d);
                b0 = b0.min(b);
                b1 = b1.max(b);
            }
        }
    }
    if d0 > d1 {
        return Vec::new();
    }
    let inside = |p: (f64, f64)| p.0 >= 0.0 && p.1 >= 0.0 && p.0 <= fw && p.1 <= fh;
    let (mut l0, mut l1) = (lng_c + d0, lng_c + d1);
    for pole in [90.0, -90.0] {
        if wc.display_of(0.0, pole, rest).is_some_and(inside) {
            (l0, l1) = (0.0, 360.0);
            if pole > 0.0 {
                b1 = 90.0;
            } else {
                b0 = -90.0;
            }
        }
    }
    let eq = matches!(c.frame, Frame::Equatorial { .. });
    let lng_step = grid_step(l1 - l0, eq);
    let lat_step = grid_step(b1 - b0, false);
    let samples = 96;
    let jump = (fw + fh) / 4.0;
    let mut out = Vec::new();
    let trace = |out: &mut Vec<GridLine>,
                 pts: &mut dyn Iterator<Item = Option<(f64, f64)>>,
                 label: String| {
        let mut seg: Vec<(f32, f32)> = Vec::new();
        let mut labelled = false;
        let mut prev: Option<(f64, f64)> = None;
        let mut flush = |seg: &mut Vec<(f32, f32)>, labelled: &mut bool| {
            if seg.len() >= 2 {
                let first_in = seg
                    .iter()
                    .any(|&(x, y)| inside((f64::from(x), f64::from(y))));
                if first_in {
                    let l = if *labelled {
                        String::new()
                    } else {
                        *labelled = true;
                        label.clone()
                    };
                    out.push(GridLine {
                        pts: std::mem::take(seg),
                        label: l,
                    });
                }
            }
            seg.clear();
        };
        for p in pts {
            match p {
                Some(p)
                    if prev.is_none_or(|q| (p.0 - q.0).hypot(p.1 - q.1) < jump)
                        && p.0.is_finite()
                        && p.1.is_finite() =>
                {
                    seg.push((p.0 as f32, p.1 as f32));
                    prev = Some(p);
                }
                Some(p) if p.0.is_finite() && p.1.is_finite() => {
                    flush(&mut seg, &mut labelled);
                    seg.push((p.0 as f32, p.1 as f32));
                    prev = Some(p);
                }
                _ => {
                    flush(&mut seg, &mut labelled);
                    prev = None;
                }
            }
        }
        flush(&mut seg, &mut labelled);
    };
    let (lb0, lb1) = ((b0 - lat_step).max(-90.0), (b1 + lat_step).min(90.0));
    let mut k = (l0 / lng_step).ceil();
    while k * lng_step <= l1 + 1e-9 && out.len() < 400 {
        let lng = k * lng_step;
        if !(l1 - l0 >= 360.0 && lng >= l0 + 360.0 - 1e-9) {
            let mut pts = (0..=samples).map(|i| {
                let b = lb0 + (lb1 - lb0) * f64::from(i) / f64::from(samples);
                wc.display_of(lng, b, rest)
            });
            trace(
                &mut out,
                &mut pts,
                grid_label(&c.frame, lng.rem_euclid(360.0), lng_step, false),
            );
        }
        k += 1.0;
    }
    let (ll0, ll1) = (l0 - lng_step, l1 + lng_step);
    let mut k = (b0 / lat_step).ceil();
    while k * lat_step <= b1 + 1e-9 && out.len() < 800 {
        let lat = k * lat_step;
        if lat.abs() < 90.0 - 1e-9 {
            let mut pts = (0..=samples * 2).map(|i| {
                let l = ll0 + (ll1 - ll0) * f64::from(i) / f64::from(samples * 2);
                wc.display_of(l, lat, rest)
            });
            trace(
                &mut out,
                &mut pts,
                grid_label(&c.frame, lat, lat_step, true),
            );
        }
        k += 1.0;
    }
    out
}

impl Science {
    /// Science mode is on: the image is scientific and its planes could be read.
    fn active(&self) -> bool {
        self.summary.is_some() && !self.off
    }

    /// The image is a seismic section (SEG-Y, Seismic Unix).
    fn seismic(&self) -> Option<&Arc<dsci::Seismic>> {
        self.summary.as_ref().and_then(|s| s.seismic.as_ref())
    }

    /// A flow cytometry plot (FCS).
    fn cytometry(&self) -> bool {
        self.summary.as_ref().is_some_and(|s| s.cytometry.is_some())
    }

    /// The limits in effect: the seismic gain for seismic sections.
    fn limits_now(&self) -> Limits {
        if self.seismic().is_some() {
            self.gain
        } else if self.thermal().is_some() && !self.limits_chosen {
            // A thermal image spans its coldest to its hottest pixel, as cameras show it.
            Limits::MinMax
        } else if self.cytometry() && !self.limits_chosen {
            // A density plot: no bins to zero to the fullest one.
            Limits::MinMax
        } else {
            self.limits
        }
    }

    /// The limits "Next limits" chooses: the one after those shown, or the first of the
    /// cycle while the file's own display range is shown (it is not in the cycle). Not the
    /// one after the stored setting, which the file range or a thermal or density default
    /// hid.
    fn next_limits(&self) -> Limits {
        let file = self
            .shown
            .as_ref()
            .is_some_and(|r| r.file_range(&r.index).is_some());
        if file && !self.limits_chosen {
            Limits::ALL[0]
        } else {
            self.limits_now().next()
        }
    }

    /// The image is shown from its raster (a whole-slide image): only its physical scale
    /// (scale bar, position readout) is science mode's; no planes, keys or look.
    fn raster_only(&self) -> bool {
        self.summary.as_ref().is_some_and(|s| s.raster_only)
    }

    /// An OMERO image with channels to switch (rendered by its server: the channel keys
    /// change what the server renders, nothing else of science mode applies).
    pub(super) fn omero_channels(&self) -> Option<&Arc<specular_decode::OmeroFacts>> {
        let s = self.summary.as_ref()?;
        s.omero.as_ref().filter(|o| o.channels.len() >= 2)
    }

    /// The axis composited, when compositing could be turned on: the channel axis, else the
    /// selected one.
    fn composite_axis(&self) -> Option<usize> {
        let s = self.summary.as_ref()?;
        s.channel_axis
            .or(Some(self.axis))
            .filter(|&a| s.axes.get(a).is_some_and(|a| a.len >= 2))
    }

    /// The axes with more than one position, the ones a slider steps.
    fn steppable(&self) -> Vec<usize> {
        let Some(s) = &self.summary else {
            return Vec::new();
        };
        let comp = self.composite.then(|| self.composite_axis()).flatten();
        (0..s.axes.len())
            .filter(|&a| s.axes[a].len >= 2 && Some(a) != comp)
            .collect()
    }

    /// The Bayer pattern raw frames are demosaiced with now: the file's override, else its
    /// header's; `None` with debayering off or for anything but one plane.
    pub(super) fn cfa(&self) -> Option<Cfa> {
        if !self.debayer {
            return None;
        }
        let s = self.summary.as_ref().filter(|s| s.planes() == 1)?;
        self.path
            .as_ref()
            .and_then(|p| self.cfa_override.get(p))
            .copied()
            .or(s.cfa)
    }

    /// The pattern the file would be demosaiced with, debayering on or off.
    fn cfa_known(&self) -> Option<Cfa> {
        let s = self.summary.as_ref().filter(|s| s.planes() == 1)?;
        self.path
            .as_ref()
            .and_then(|p| self.cfa_override.get(p))
            .copied()
            .or(s.cfa)
    }

    /// The shown planes' limits and midtones, once every one has its limits (for blinking).
    pub(super) fn planes_look(&self) -> Option<Vec<(f32, f32, f32)>> {
        if self.planes.is_empty() {
            return None;
        }
        self.planes
            .iter()
            .map(|p| p.limits.map(|(lo, hi)| (lo, hi, p.mid)))
            .collect()
    }

    /// The stretch in effect: the automatic one for a debayered frame until one is chosen.
    pub(super) fn stretch_now(&self) -> Stretch {
        if self.cfa().is_some() && !self.stretch_chosen {
            Stretch::Auto
        } else if self.cytometry() && !self.stretch_chosen {
            // Event counts span decades: a density plot's usual log scale.
            Stretch::Log
        } else {
            self.stretch
        }
    }

    /// What should be read now.
    fn request(&self) -> Option<Request> {
        self.summary.as_ref()?;
        let mut index = self.index.clone();
        let fft = self.fft;
        let composite = (self.composite && !fft)
            .then(|| self.composite_axis())
            .flatten()
            .map(|a| {
                index[a] = 0;
                let n = self.summary.as_ref().map_or(1, |s| s.axes[a].len);
                (a, n.min(MAX_CHANNELS as u32))
            });
        Some(Request {
            index,
            composite,
            limits: self.limits_now(),
            histeq: self.stretch_now() == Stretch::HistEq,
            level: self.level && self.summary.as_ref().is_some_and(|s| s.levelling),
            cfa: self.cfa().filter(|_| !fft),
            auto: self.stretch_now() == Stretch::Auto,
            ranges: self
                .summary
                .as_ref()
                .filter(|_| !self.limits_chosen && self.cfa().is_none() && !fft)
                .map_or((None, Vec::new()), |s| {
                    (s.channel_axis, s.display_ranges.clone())
                }),
            fft,
            ion: self.ion,
            thermal: self.conditions_now().map(|c| (c, self.unit_now())),
        })
    }

    /// The relief shading the image suits: one georeferenced band (an elevation model), lit
    /// by the conventional north-west sun, with the ground step at its centre and the
    /// elevations' unit in metres.
    fn relief(&self) -> Option<Hillshade> {
        let s = self.summary.as_ref()?;
        if self.composite && self.composite_axis().is_some() {
            return None;
        }
        let g = s.geo.as_deref()?;
        let (sx, sy) = g.ground_step(f64::from(s.width) / 2.0, f64::from(s.height) / 2.0)?;
        let z = match s.unit.as_deref().map(|u| u.trim().to_ascii_lowercase()) {
            Some(u) if u == "ftus" || u.contains("survey") => 1200.0 / 3937.0,
            Some(u) if u == "ft" || u.starts_with("foot") || u.starts_with("feet") => 0.3048,
            _ => 1.0,
        };
        Some(Hillshade {
            step: [sx as f32, sy as f32],
            z,
            kind: self.relief_kind,
            ..Hillshade::default()
        })
    }

    /// The hillshade drawn: on, and the image suits it.
    fn hillshade_now(&self) -> Option<Hillshade> {
        self.hillshade.then(|| self.relief()).flatten()
    }

    /// The look of the shown planes.
    fn look(&self) -> Option<ScienceLook> {
        if self.planes.is_empty() {
            return None;
        }
        let n = self.planes.len();
        // The file's own channel colours (OME) when it gives one for every channel shown.
        let own = self
            .summary
            .as_ref()
            .map(|s| s.channel_colours.as_slice())
            .filter(|c| n > 1 && c.len() >= n);
        let colours = own.map_or_else(|| channel_colours(n), |c| c[..n].to_vec());
        let channels = self
            .planes
            .iter()
            .enumerate()
            .map(|(k, p)| {
                let (lo, hi) = self.range.or(p.limits).unwrap_or((0.0, 0.0));
                // Blinking: the first frame's limits for every frame (unless a range is set).
                let (lo, hi, mid) = match self
                    .blink
                    .as_ref()
                    .filter(|b| self.blink_on && self.range.is_none() && b.len() == n)
                {
                    Some(b) => b[k],
                    None => (lo, hi, p.mid),
                };
                ChannelLook {
                    lo,
                    hi,
                    colour: colours[k.min(colours.len() - 1)],
                    on: n == 1 || !self.hidden[k],
                    mid,
                }
            })
            .collect();
        let want = self.stretch_now();
        let histeq = want == Stretch::HistEq && self.planes.iter().all(|p| !p.lut.is_empty());
        // The planes shown were read for another stretch (tables, automatic limits): linear
        // until the new ones come.
        let auto_read = self.shown.as_ref().is_some_and(|r| r.auto);
        let stretch =
            if (want == Stretch::HistEq && !histeq) || (want == Stretch::Auto) != auto_read {
                Stretch::Linear
            } else {
                want
            };
        Some(ScienceLook {
            stretch,
            channels,
            luts: if histeq {
                self.planes.iter().map(|p| p.lut.clone()).collect()
            } else {
                Vec::new()
            },
            colour_table: (n == 1 && !self.fft)
                .then(|| self.colour_table())
                .flatten()
                .or_else(|| self.user_lut.clone().filter(|_| self.fft)),
            hillshade: self.hillshade_now().filter(|_| n == 1 && !self.fft),
            isotherm: self.isotherm_look().filter(|_| n == 1 && !self.fft),
        })
    }

    /// The colour table a single plane shows through: the user's, else the file's for the
    /// plane's channel.
    fn colour_table(&self) -> Option<Arc<ColourTable>> {
        if let Some((_, t)) = &self.weather {
            return Some(t.clone());
        }
        if let Some(t) = &self.user_lut {
            return Some(t.clone());
        }
        let s = self.summary.as_ref()?;
        let k = s
            .channel_axis
            .and_then(|a| self.index.get(a))
            .map_or(0, |&i| i as usize);
        s.luts.get(k).or(s.luts.first()).cloned()
    }

    /// A channel's name: the axis' label for it, else its number.
    fn channel_name(&self, k: usize) -> String {
        self.composite_axis()
            .and_then(|a| {
                self.summary.as_ref()?.axes[a]
                    .labels
                    .get(k)
                    .filter(|l| !l.trim().is_empty())
                    .map(|l| l.trim().to_owned())
            })
            .unwrap_or_else(|| (k + 1).to_string())
    }

    /// A channel's name for a sentence: the axis' label for it, else "Channel n".
    fn channel_title(&self, k: usize) -> String {
        let name = self.channel_name(k);
        if name == (k + 1).to_string() {
            t!("science-channel-unnamed", number = name)
        } else {
            name
        }
    }
}

impl App {
    /// Keeps science mode in step with the current image: reads its planes, uploads them,
    /// sets the look and the keymap's layer. Cheap when nothing changed.
    pub(super) fn science_sync(&mut self) {
        self.science_lut_poll();
        self.science_catalogue_poll();
        self.science_export_poll();
        self.thermal_report_poll();
        let cur = self.current_id();
        if cur != self.science.id {
            if let Some(old) = self.science.id
                && self.science.uploaded
                && let (Some(gfx), Some(g)) = (self.gfx.as_mut(), self.gpu_images.get(old))
            {
                gfx.renderer.clear_values(g);
            }
            let cfa_override = std::mem::take(&mut self.science.cfa_override);
            let lut_import = self.science.lut_import.take();
            let catalogue_rx = self.science.catalogue_rx.take();
            let catalogues = std::mem::take(&mut self.science.catalogues);
            let s = &self.science;
            self.science = Science {
                id: cur,
                stretch: s.stretch,
                limits: s.limits,
                grid: s.grid,
                level: s.level,
                stretch_chosen: s.stretch_chosen,
                limits_chosen: s.limits_chosen,
                gain: s.gain,
                wiggle: s.wiggle,
                wireframe: s.wireframe,
                ion_tolerance: s.ion_tolerance,
                unit_chosen: s.unit_chosen,
                export_rx: s.export_rx.clone(),
                debayer: s.debayer,
                blink_on: s.blink_on,
                blink: s.blink.clone(),
                cfa_override,
                user_lut: s.user_lut.clone(),
                lut_import,
                catalogue_rx,
                catalogues,
                path: cur
                    .and_then(|id| self.images.get(&id))
                    .map(|i| i.path.clone()),
                grid_cache: s.grid_cache.clone(),
                ..Default::default()
            };
        }
        let Some(id) = cur else {
            self.science_key_mode(false);
            return;
        };
        if self.science.summary.is_none() && !self.science.off {
            let summary = self
                .images
                .get(&id)
                .and_then(|i| i.session.as_ref())
                // Video (SER) plays in the animation player instead.
                .filter(|s| s.animation().is_none())
                .and_then(|s| s.science().cloned());
            if let Some(s) = summary {
                self.science.index = if s.start.len() == s.axes.len() {
                    s.start.clone()
                } else {
                    vec![0; s.axes.len()]
                };
                self.science.composite = s.channel_axis.is_some();
                self.science.axis = (0..s.axes.len())
                    .find(|&a| s.axes[a].len >= 2 && Some(a) != s.channel_axis)
                    .or(s.channel_axis)
                    .unwrap_or(0);
                let s = self.astro_with_solution(s);
                self.science.summary = Some(Arc::new(s));
            }
        }
        self.science.unit_default = self.temperature_unit_default();
        let on = self.science.active()
            && (!self.science.raster_only() || self.science.omero_channels().is_some());
        self.science_key_mode(on && !self.gallery_active());
        if !on {
            return;
        }
        if let Some(want) = self.science.request()
            && self.science.shown.as_ref() != Some(&want)
            && self.science.inflight.is_none()
        {
            self.science_fetch(id, want);
        }
        if self.science.planes.is_empty() {
            return;
        }
        let Some(gfx) = self.gfx.as_mut() else { return };
        let Some(g) = self.gpu_images.get(id) else {
            return;
        };
        let cfa = self.science.shown.as_ref().and_then(|r| r.cfa);
        if (!self.science.uploaded || !g.has_values())
            && let Some(cfa) = cfa
        {
            let p = &self.science.planes[0].plane;
            if (p.width, p.height) != (g.width, g.height) {
                return;
            }
            gfx.renderer.set_value_cfa(&gfx.gpu, g, &p.values, cfa);
            self.science.uploaded = true;
            self.science.sent = false;
        }
        if !self.science.uploaded || !g.has_values() {
            let planes: Vec<&[f32]> = self
                .science
                .planes
                .iter()
                .filter(|p| (p.plane.width, p.plane.height) == (g.width, g.height))
                .map(|p| p.plane.values.as_slice())
                .collect();
            if planes.len() != self.science.planes.len() {
                return;
            }
            gfx.renderer.set_value_channels(&gfx.gpu, g, &planes);
            self.science.uploaded = true;
            self.science.sent = false;
        }
        if !self.science.sent {
            let look = self.science.look();
            if g.science() != look.as_ref() {
                gfx.renderer.set_science(&gfx.gpu, g, look);
                self.redraw = true;
            }
            self.science.sent = true;
        }
    }

    /// Turns the keymap's science mode on or off (over the viewer, under cull mode).
    fn science_key_mode(&mut self, on: bool) {
        if self.keys.science != on {
            self.keys.science = on;
            self.keys_sync_modes();
        }
    }

    /// Science mode is on for the current image (its readout and overlay show).
    fn science_shown(&self) -> bool {
        self.science.active() && self.science.id == self.current_id()
    }

    /// What the map view reads of science mode (`map.rs`): the summary, the planes shown
    /// with their names, and whether they suit relief shading (an elevation model).
    #[allow(clippy::type_complexity)]
    pub(super) fn science_for_map(
        &self,
    ) -> Option<(Arc<ScienceSummary>, Vec<(String, Arc<NdPlane>)>, bool)> {
        if !self.science_shown() {
            return None;
        }
        let sc = &self.science;
        let s = sc.summary.clone()?;
        if sc.planes.is_empty() {
            return None;
        }
        let planes = sc
            .planes
            .iter()
            .enumerate()
            .map(|(k, p)| {
                let name = if sc.planes.len() > 1 {
                    sc.channel_name(k)
                } else {
                    String::new()
                };
                (name, p.plane.clone())
            })
            .collect();
        Some((s, planes, sc.relief().is_some()))
    }

    /// The map view's terrain tools: shows `kind` (slope or aspect colours, or the shade),
    /// or the plain values when it is already shown. Returns what is shown now (`None`:
    /// the image does not suit relief).
    pub(super) fn science_set_relief(&mut self, kind: Relief) -> Option<Option<Relief>> {
        self.science.relief()?;
        let sc = &mut self.science;
        if sc.hillshade && sc.relief_kind == kind {
            sc.hillshade = false;
        } else {
            sc.hillshade = true;
            sc.relief_kind = kind;
        }
        let now = sc.hillshade.then_some(sc.relief_kind);
        self.science_restyle();
        Some(now)
    }

    /// The relief drawn now, if any.
    pub(super) fn science_relief(&self) -> Option<Hillshade> {
        self.science_shown()
            .then(|| self.science.hillshade_now())
            .flatten()
    }

    /// The summary and position of the planes shown (the map view's time slider and wind).
    pub(super) fn science_position(&self) -> Option<(Arc<ScienceSummary>, Vec<u32>)> {
        if !self.science_shown() {
            return None;
        }
        Some((self.science.summary.clone()?, self.science.index.clone()))
    }

    /// The planes shown are the ones asked for (a played time step has arrived).
    pub(super) fn science_settled(&self) -> bool {
        self.science.inflight.is_none() && self.science.shown == self.science.request()
    }

    /// A weather colour map for this image (`None`: the file's colours again).
    pub(super) fn science_set_weather(&mut self, m: Option<specular_inspect::ColourMap>) {
        self.science.weather = m.map(|m| (m, super::map::weather_table(m)));
        self.science_restyle();
    }

    /// The current scientific image has a z axis MPR stacks (cheap: the action's state).
    pub(super) fn science_is_volume(&self) -> bool {
        self.science_shown()
            && self.science.omero_channels().is_none()
            && self
                .science
                .summary
                .as_deref()
                .and_then(super::mpr::science_z_axis)
                .is_some()
    }

    /// The current scientific image as an MPR volume (a z-stack or cube), at the shown
    /// positions of its other axes: with channels composited, the first channel shown (MPR
    /// is grey) and its display range.
    pub(super) fn science_volume(&self) -> Option<super::mpr::ScienceStack> {
        if !self.science_shown() || self.science.omero_channels().is_some() {
            return None;
        }
        let sc = &self.science;
        let s = sc.summary.as_ref()?;
        let mut index = sc.index.clone();
        let mut limits = sc
            .range
            .or_else(|| sc.planes.first().and_then(|p| p.limits));
        let comp = sc.composite.then(|| sc.composite_axis()).flatten();
        if let Some(c) = comp
            && let Some(len) = s.axes.get(c).map(|a| a.len as usize)
        {
            let k = (0..len.min(MAX_CHANNELS))
                .find(|&k| !sc.hidden[k])
                .unwrap_or(0);
            if let Some(i) = index.get_mut(c) {
                *i = k as u32;
            }
            limits = sc
                .range
                .or_else(|| sc.planes.get(k).and_then(|p| p.limits))
                .or(limits);
        }
        super::mpr::science_stack(s, &index, limits, comp.is_some())
    }

    /// With channels composited (a channel axis that is not z, two or more shown): every
    /// shown channel as an MPR stack at its display range, with its colour (linear RGB, as
    /// the 2D composite draws it), the first shown first. Empty otherwise.
    /// The shown channels of a composite for MPR: each one's stack, colour and name.
    pub(super) fn science_volume_channels(
        &self,
    ) -> Vec<(super::mpr::ScienceStack, [f32; 3], String)> {
        if !self.science_shown() || self.science.omero_channels().is_some() {
            return Vec::new();
        }
        let sc = &self.science;
        let Some(s) = sc.summary.as_ref() else {
            return Vec::new();
        };
        let Some(c) = sc.composite.then(|| sc.composite_axis()).flatten() else {
            return Vec::new();
        };
        if s.channel_axis != Some(c) || super::mpr::science_z_axis(s) == Some(c) {
            return Vec::new();
        }
        let n = (s.axes[c].len as usize).min(MAX_CHANNELS);
        let own = Some(s.channel_colours.as_slice()).filter(|o| n > 1 && o.len() >= n);
        let colours = own.map_or_else(|| channel_colours(n), |o| o[..n].to_vec());
        let shown: Vec<usize> = (0..n).filter(|&k| !sc.hidden[k]).collect();
        if shown.len() < 2 {
            return Vec::new();
        }
        shown
            .into_iter()
            .filter_map(|k| {
                let mut index = sc.index.clone();
                *index.get_mut(c)? = k as u32;
                let limits = sc.range.or_else(|| sc.planes.get(k).and_then(|p| p.limits));
                let mut st = super::mpr::science_stack(s, &index, limits, false)?;
                // The automatic stretch's balance, as the 2D view draws the channel.
                st.mid = sc.planes.get(k).map_or(0.5, |p| p.mid);
                Some((st, colours[k.min(colours.len() - 1)], sc.channel_name(k)))
            })
            .collect()
    }

    /// Scripting: the display range of the first plane shown, `None` outside science mode.
    pub(super) fn science_range(&self) -> Option<(f32, f32)> {
        if !self.science_shown() {
            return None;
        }
        let sc = &self.science;
        sc.range
            .or_else(|| sc.planes.first().and_then(|p| p.limits))
    }

    /// Scripting: sets the display range of every plane (`None`: the computed limits).
    /// False outside science mode.
    pub(super) fn science_set_range(&mut self, r: Option<(f32, f32)>) -> bool {
        if !self.science_shown() {
            return false;
        }
        self.science.range = r;
        self.science_restyle();
        true
    }

    /// Scripting: the channels (names, and which show), when the image has a channel axis.
    pub(super) fn science_channels(&self) -> Option<(Vec<String>, Vec<bool>)> {
        if !self.science_shown() {
            return None;
        }
        let sc = &self.science;
        let axis = sc.composite_axis()?;
        let n = sc.summary.as_ref()?.axes[axis].len as usize;
        let names = (0..n).map(|k| sc.channel_name(k)).collect();
        let shown = (0..n)
            .map(|k| {
                if sc.composite {
                    !sc.hidden.get(k).copied().unwrap_or(false)
                } else {
                    sc.index.get(axis).is_some_and(|&i| i as usize == k)
                }
            })
            .collect();
        Some((names, shown))
    }

    /// Scripting: shows channel `only` alone (composite images hide the others; others step
    /// to it), or every channel (`None`). False when there is no such channel.
    pub(super) fn science_show_channel(&mut self, only: Option<usize>) -> bool {
        let Some((names, _)) = self.science_channels() else {
            return false;
        };
        let Some(axis) = self.science.composite_axis() else {
            return false;
        };
        match only {
            Some(k) if k >= names.len() => false,
            Some(k) if self.science.composite => {
                for (i, h) in self.science.hidden.iter_mut().enumerate() {
                    *h = i != k;
                }
                self.science_restyle();
                true
            }
            Some(k) => {
                self.science_step(axis, Some(k as u32), 0);
                true
            }
            None => {
                self.science.hidden = [false; MAX_CHANNELS];
                self.science.composite = names.len() <= MAX_CHANNELS;
                self.science_restyle();
                true
            }
        }
    }

    /// The user imported a colour table (it stays over the file's colours).
    pub(super) fn science_user_lut(&self) -> bool {
        self.science.user_lut.is_some()
    }

    /// The weather colour map shown, if one was chosen.
    pub(super) fn science_weather(&self) -> Option<specular_inspect::ColourMap> {
        self.science.weather.as_ref().map(|w| w.0)
    }

    /// The display colour (display-coded) of a plane cell (`x`, `y`) as science mode draws
    /// it: the value through the look and the relief, `None` without data (the 3-D drape and
    /// the map export). `None` without a single plane shown.
    pub(super) fn science_colourer(
        &self,
    ) -> Option<impl Fn(usize, usize) -> Option<[f32; 3]> + '_> {
        let sc = &self.science;
        let look = sc.look()?;
        let [p] = sc.planes.as_slice() else {
            return None;
        };
        let (w, h) = (p.plane.width as usize, p.plane.height as usize);
        let values = &p.plane.values;
        let fc = self
            .chrome
            .inspect
            .false_colour()
            .filter(|_| look.colour_table.is_none());
        let coloured = fc.is_some() || look.colour_table.is_some();
        Some(move |x: usize, y: usize| {
            let v = *values.get(y * w + x)?;
            if !v.is_finite() {
                return None;
            }
            let mut rgb = look.colour(&[v]);
            if let Some(m) = fc {
                rgb = m.sample(rgb[0]);
            }
            if let Some(hs) = look.hillshade {
                match hs.kind {
                    Relief::Shade => {
                        let sh = hs.shade(values, w, h, x, y)?;
                        rgb = if coloured {
                            rgb.map(|c| c * sh)
                        } else {
                            [sh; 3]
                        };
                    }
                    _ => rgb = hs.colour(values, w, h, x, y)?,
                }
            }
            Some(rgb)
        })
    }

    /// What the science actions depend on ([`specular_input::ActionState::science`]).
    pub(super) fn science_state(&self) -> Option<ScienceState> {
        let sc = &self.science;
        if self.science_shown()
            && let Some(o) = sc.omero_channels()
        {
            return Some(ScienceState {
                planes: 1,
                channels: o.channels.len() as u32,
                composite: true,
                ..ScienceState::default()
            });
        }
        if !self.science_shown() || sc.raster_only() {
            return None;
        }
        let s = sc.summary.as_ref()?;
        Some(ScienceState {
            planes: s.planes(),
            axes: s.axes.iter().filter(|a| a.len >= 2).count() as u32,
            channels: sc.composite_axis().map_or(0, |a| s.axes[a].len),
            composite: sc.composite,
            // The grid action: sky coordinates, a georeferenced image's coastlines and
            // graticule, or a diffraction frame's resolution rings.
            celestial: s.wcs.as_ref().is_some_and(|w| w.celestial.is_some())
                || s.geo.is_some()
                || s.diffraction.as_ref().is_some_and(|g| g.complete()),
            levelling: s.levelling,
            relief: sc.relief().is_some(),
            seismic: s.seismic.is_some(),
            mass_spec: s.mass_spec.is_some(),
            thermal: s.thermal.is_some(),
            radiometric: s.thermal.as_ref().is_some_and(|t| t.adjustable()),
            sequence: s.thermal.as_ref().is_some_and(|t| t.frame_count() > 1),
            ugrid: s.mesh.is_some(),
            samp: self.astro.samp.connected,
        })
    }

    /// Reads the planes of `req` on a worker, with their limits and tables.
    fn science_fetch(&mut self, id: ImageId, req: Request) {
        let Some(session) = self.images.get(&id).and_then(|i| i.session.clone()) else {
            return;
        };
        self.science.inflight = Some(req.clone());
        let mass_spec = self
            .science
            .summary
            .as_ref()
            .and_then(|s| s.mass_spec.clone());
        let thermal = self.science.thermal().cloned();
        self.loader.background("science-planes", move |l| {
            let cancel = CancelToken::new();
            // A reason the user can read; stopping is no failure (an empty reason).
            let why = |e: specular_decode::DecodeError| match e {
                specular_decode::DecodeError::Cancelled => String::new(),
                e => crate::error_text::decode_reason(&e),
            };
            let positions: Vec<Vec<u32>> = match req.composite {
                Some((axis, n)) => (0..n)
                    .map(|k| {
                        let mut i = req.index.clone();
                        i[axis] = k;
                        i
                    })
                    .collect(),
                None => vec![req.index.clone()],
            };
            let planes = positions
                .iter()
                .map(|p| {
                    let plane = match (req.ion, &mass_spec, req.thermal, &thermal) {
                        (Some(w), Some(ms), ..) => ms.ion_plane(w, p, &cancel).map_err(why)?,
                        (_, _, Some((c, u)), Some(t)) => {
                            let frame = p.first().copied().unwrap_or(0);
                            t.plane_at(frame, &c, u, &cancel).map_err(why)?
                        }
                        _ => session
                            .nd_plane(p, &cancel)
                            .ok_or_else(|| t!("error-reason-no-image"))?
                            .map_err(why)?,
                    };
                    Ok(prepare(transform(plane, &req)?, &req, p))
                })
                .collect::<Result<Vec<_>, String>>()
                .map(|v| v.into_iter().flatten().collect());
            l.post(Event::ScienceValues { id, planes });
        });
    }

    /// Planes arrived.
    pub(super) fn science_values(
        &mut self,
        id: ImageId,
        planes: Result<Vec<SciencePlane>, String>,
    ) {
        if self.science.id != Some(id) {
            return;
        }
        let req = self.science.inflight.take();
        match planes {
            Ok(p) if !p.is_empty() => {
                let thermal = req.as_ref().is_some_and(|r| r.thermal.is_some());
                self.science.planes = p;
                self.science.shown = req;
                self.science.uploaded = false;
                if thermal {
                    // Measurements are of the temperatures: of these ones now.
                    self.measure_restat();
                }
            }
            Ok(_) => self.science.off = true,
            Err(e) => {
                specular_log::debug!(target: specular_log::DECODE, "img={id} science planes: {e}");
                // An ion image or the temperatures were asked for: say why they are not shown.
                if !e.is_empty()
                    && req
                        .as_ref()
                        .is_some_and(|r| r.ion.is_some() || r.thermal.is_some())
                {
                    let now = self.now();
                    self.chrome
                        .toast(t!("science-values-failed", reason = e), None, now);
                }
                self.science.off = true;
            }
        }
        self.redraw = true;
        self.science_sync();
        self.science_readout();
    }

    /// The look changed: send it again.
    pub(super) fn science_restyle(&mut self) {
        self.science.sent = false;
        self.science_sync();
        self.science_readout();
        self.redraw = true;
    }

    /// Moves the selected axis' position by `d` (wrapping), or to `to`.
    pub(super) fn science_step(&mut self, axis: usize, to: Option<u32>, d: i64) {
        let Some(s) = self.science.summary.clone() else {
            return;
        };
        let Some(len) = s.axes.get(axis).map(|a| a.len).filter(|&n| n >= 2) else {
            return;
        };
        if self.science.composite && self.science.composite_axis() == Some(axis) {
            self.science.composite = false;
        }
        self.science.axis = axis;
        let cur = i64::from(self.science.index[axis]);
        let next = match to {
            Some(t) => t.min(len - 1),
            None => (cur + d).rem_euclid(i64::from(len)) as u32,
        };
        self.science.index[axis] = next;
        self.science_sync();
        self.science_readout();
        self.redraw = true;
    }

    /// Asks for a colour table file and reads it on the dialog's thread
    /// ([`App::science_lut_poll`] applies it).
    fn science_lut_import(&mut self) {
        let (tx, rx) = crossbeam_channel::bounded(1);
        let filter = t!("science-lut-filter");
        self.loader.background("dlg-science-lut", move |l| {
            let Some(p) = rfd::FileDialog::new()
                .add_filter(filter, &["lut", "LUT", "txt"])
                .pick_file()
            else {
                return;
            };
            let name = p
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            // A colour table is at most a few kilobytes of text.
            let table = std::fs::metadata(&p)
                .ok()
                .filter(|m| m.len() <= 1 << 20)
                .and_then(|_| std::fs::read(&p).ok())
                .and_then(|b| dsci::read_lut(&b));
            let _ = tx.send((name, table));
            l.waker()();
        });
        self.science.lut_import = Some(rx);
    }

    /// Applies a colour table read from a file, once there.
    fn science_lut_poll(&mut self) {
        let Some(Ok((name, table))) = self.science.lut_import.as_ref().map(|r| r.try_recv()) else {
            return;
        };
        self.science.lut_import = None;
        let now = self.now();
        let msg = if table.is_some() {
            t!("science-lut-toast", name = name)
        } else {
            t!("science-lut-failed", name = name)
        };
        self.chrome.toast(msg, None, now);
        if table.is_some() {
            self.science.user_lut = table;
            self.science.sent = false;
            self.redraw = true;
        }
    }

    /// Science actions; true when `a` was one.
    pub(super) fn science_action(&mut self, a: Action) -> bool {
        if self.astro_action(a) {
            return true;
        }
        if matches!(a, Action::ToggleDebayer | Action::NextBayerPattern) {
            self.debayer_action(a);
            return true;
        }
        if a == Action::ToggleThermalVisible {
            self.thermal_visible_toggle();
            return true;
        }
        // Any other image: the inspection report (same dialog, its measurements and facts).
        if a == Action::ThermalReport && self.science.thermal().is_none() {
            self.inspection_report_open();
            return true;
        }
        if a == Action::ToggleAnnotations {
            if self.annotations_capable() || self.pathology_has_imported() {
                let sc = &mut self.science;
                sc.annotations_hidden = !sc.annotations_hidden;
                let msg = if sc.annotations_hidden {
                    t!("science-annotations-off")
                } else {
                    t!("science-annotations-on")
                };
                let now = self.now();
                self.chrome.toast(msg, None, now);
                self.redraw = true;
            }
            return true;
        }
        if !matches!(
            a,
            Action::ScienceStretch
                | Action::ScienceLimits
                | Action::ScienceNextPlane
                | Action::SciencePrevPlane
                | Action::ScienceNextAxis
                | Action::ScienceComposite
                | Action::ScienceChannel(_)
                | Action::ScienceGrid
                | Action::ScienceLevel
                | Action::ScienceHillshade
                | Action::ScienceFft
                | Action::ScienceWiggle
                | Action::ScienceWireframe
                | Action::ScienceReset
                | Action::ScienceImportLut
                | Action::ScienceCatalogue
                | Action::ScienceClearCatalogues
                | Action::ScienceIonImage
                | Action::ScienceTemperatureUnit
                | Action::ScienceIsotherm
                | Action::ScienceThermalParameters
                | Action::ScienceExportTemperatures
                | Action::ScienceThermalPlay
                | Action::ScienceThermalSeriesCsv
                | Action::ThermalReport
        ) {
            return false;
        }
        if !self.science.active() {
            return true;
        }
        // An OMERO image: the channel keys ask its server for another rendering.
        if let Some(o) = self.science.omero_channels().cloned() {
            if let Action::ScienceChannel(n) = a {
                self.omero_toggle_channel(&o, usize::from(n.max(1) - 1));
            }
            return true;
        }
        let now = self.now();
        match a {
            Action::ScienceStretch => {
                self.science.blink = None;
                self.science.stretch = self.science.stretch_now().next();
                self.science.stretch_chosen = true;
                let msg = t!(
                    "science-stretch-toast",
                    stretch = stretch_name(self.science.stretch)
                );
                self.chrome.toast(msg, None, now);
                self.science_restyle();
            }
            Action::ScienceLimits => {
                self.science.blink = None;
                if self.science.seismic().is_some() {
                    self.science.gain = self.science.gain.next_symmetric();
                } else {
                    self.science.limits = self.science.next_limits();
                    self.science.limits_chosen = true;
                }
                let msg = t!(
                    "science-limits-toast",
                    limits = limits_name(self.science.limits_now())
                );
                self.chrome.toast(msg, None, now);
                self.science_restyle();
            }
            Action::ScienceReset => {
                self.science.blink = None;
                self.science.stretch = Stretch::Linear;
                self.science.stretch_chosen = false;
                self.science.limits = Limits::ZScale;
                self.science.limits_chosen = false;
                self.science.gain = Limits::SEISMIC;
                self.science.user_lut = None;
                self.science.hidden = [false; MAX_CHANNELS];
                self.science.ion = None;
                self.science.isotherm = None;
                self.science.conditions = None;
                self.science.composite = self
                    .science
                    .summary
                    .as_ref()
                    .is_some_and(|s| s.channel_axis.is_some());
                self.chrome.toast(t!("science-reset-toast"), None, now);
                self.science_restyle();
            }
            Action::ScienceImportLut => self.science_lut_import(),
            Action::ScienceCatalogue => self.science_catalogue_open(),
            Action::ScienceClearCatalogues => self.science_catalogues_clear(),
            Action::ScienceIonImage => self.science_ion_open(),
            Action::ScienceTemperatureUnit => self.science_temperature_unit(),
            Action::ScienceIsotherm => self.science_isotherm(),
            Action::ScienceThermalParameters => self.science_thermal_open(),
            Action::ScienceExportTemperatures => self.science_export_temperatures(),
            Action::ScienceThermalPlay => self.thermal_seq_play(),
            Action::ScienceThermalSeriesCsv => self.thermal_seq_csv(),
            Action::ThermalReport => self.thermal_report_open(),
            Action::ScienceNextPlane | Action::SciencePrevPlane => {
                let d = if a == Action::ScienceNextPlane { 1 } else { -1 };
                let mut axis = self.science.axis;
                let steppable = self.science.steppable();
                if !steppable.contains(&axis)
                    && let Some(&first) = steppable.first()
                {
                    axis = first;
                }
                self.science_step(axis, None, d);
            }
            Action::ScienceNextAxis => {
                let Some(s) = self.science.summary.clone() else {
                    return true;
                };
                let all: Vec<usize> = (0..s.axes.len()).filter(|&a| s.axes[a].len >= 2).collect();
                if let Some(i) = all.iter().position(|&a| a == self.science.axis) {
                    self.science.axis = all[(i + 1) % all.len()];
                } else if let Some(&a) = all.first() {
                    self.science.axis = a;
                }
                let msg = t!(
                    "science-axis-toast",
                    axis = axis_name(&s, self.science.axis)
                );
                self.chrome.toast(msg, None, now);
                self.science_readout();
            }
            Action::ScienceComposite => {
                self.science.composite = !self.science.composite;
                let msg = if self.science.composite {
                    t!("science-composite-on")
                } else {
                    t!("science-composite-off")
                };
                self.chrome.toast(msg, None, now);
                self.science_restyle();
            }
            Action::ScienceChannel(n) => {
                let k = usize::from(n.max(1) - 1).min(MAX_CHANNELS - 1);
                self.science.hidden[k] = !self.science.hidden[k];
                let channel = self.science.channel_title(k);
                let msg = if self.science.hidden[k] {
                    t!("science-channel-off", channel = channel)
                } else {
                    t!("science-channel-on", channel = channel)
                };
                self.chrome.toast(msg, None, now);
                self.science_restyle();
            }
            Action::ScienceLevel => {
                self.science.level = !self.science.level;
                let msg = if self.science.level {
                    t!("science-level-on")
                } else {
                    t!("science-level-off")
                };
                self.chrome.toast(msg, None, now);
                self.science_sync();
                self.science_readout();
            }
            Action::ScienceHillshade => {
                // From slope or aspect colours, back to the shade; else on or off.
                if self.science.hillshade && self.science.relief_kind != Relief::Shade {
                    self.science.relief_kind = Relief::Shade;
                } else {
                    self.science.hillshade = !self.science.hillshade;
                    self.science.relief_kind = Relief::Shade;
                }
                let msg = match self.science.hillshade_now() {
                    Some(h) => t!(
                        "science-hillshade-on",
                        azimuth = h.azimuth.to_string(),
                        altitude = h.altitude.to_string()
                    ),
                    None => t!("science-hillshade-off"),
                };
                self.chrome.toast(msg, None, now);
                self.science_restyle();
            }
            Action::ScienceFft => {
                self.science.fft = !self.science.fft;
                let msg = if self.science.fft {
                    t!("science-fft-on")
                } else {
                    t!("science-fft-off")
                };
                self.chrome.toast(msg, None, now);
                self.science_sync();
                self.science_readout();
            }
            Action::ScienceWireframe => {
                self.science.wireframe = !self.science.wireframe;
                let msg = if !self.science.wireframe {
                    t!("science-wireframe-off")
                } else if self.science_mesh_edges().is_none() {
                    t!("science-wireframe-dense")
                } else {
                    t!("science-wireframe-on")
                };
                self.chrome.toast(msg, None, now);
            }
            Action::ScienceWiggle => {
                self.science.wiggle = !self.science.wiggle;
                let msg = if self.science.wiggle {
                    t!("science-wiggle-on")
                } else {
                    t!("science-wiggle-off")
                };
                self.chrome.toast(msg, None, now);
            }
            Action::ScienceGrid => {
                self.science.grid = !self.science.grid;
                let rings = self
                    .science
                    .summary
                    .as_ref()
                    .is_some_and(|s| s.diffraction.is_some());
                let msg = match (self.science.grid, rings) {
                    (true, false) => t!("science-grid-on"),
                    (false, false) => t!("science-grid-off"),
                    (true, true) => t!("science-rings-on"),
                    (false, true) => t!("science-rings-off"),
                };
                self.chrome.toast(msg, None, now);
            }
            _ => {}
        }
        self.redraw = true;
        true
    }

    /// Whether the debayer actions apply: a one-plane science image (FITS, XISF) or video
    /// whose frames are raw colour samples (SER).
    /// The current image has annotations to show or hide.
    /// The current image's science summary (a slide's levels and annotations).
    pub(super) fn science_summary_current(&self) -> Option<&ScienceSummary> {
        if self.science.id.is_none() || self.science.id != self.current_id() {
            return None;
        }
        self.science.summary.as_deref()
    }

    pub(super) fn annotations_capable(&self) -> bool {
        self.science_shown()
            && self.science.summary.as_ref().is_some_and(|s| {
                s.annotations
                    .as_ref()
                    .is_some_and(|a| !a.set.items.is_empty())
            })
    }

    pub(super) fn debayer_capable(&self) -> bool {
        let session = self
            .current_id()
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.session.as_ref());
        if session.is_some_and(|s| s.can_debayer()) {
            return true;
        }
        self.science_state().is_some_and(|s| s.planes == 1)
    }

    /// [`Action::ToggleDebayer`] and [`Action::NextBayerPattern`].
    fn debayer_action(&mut self, a: Action) {
        let now = self.now();
        let session = self
            .current_id()
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.session.clone())
            .filter(|s| s.can_debayer());
        let on = if let Some(s) = session {
            // Video: its session decodes the frames; the player starts again where it was.
            let file = s.science().and_then(|f| f.cfa);
            let next = match a {
                Action::ToggleDebayer => match s.debayer() {
                    Some(_) => None,
                    None => Some(file.unwrap_or(Cfa::RGGB)),
                },
                _ => Some(s.debayer().or(file).map_or(Cfa::RGGB, Cfa::next)),
            };
            s.set_debayer(next);
            if let Some(an) = self.anim.as_mut() {
                an.scale = 0;
            }
            self.sync_animation();
            next
        } else if self.science_state().is_some_and(|s| s.planes == 1) {
            let sc = &mut self.science;
            match a {
                Action::ToggleDebayer if sc.cfa().is_some() => sc.debayer = false,
                Action::ToggleDebayer => {
                    sc.debayer = true;
                    if sc.cfa_known().is_none()
                        && let Some(p) = sc.path.clone()
                    {
                        sc.cfa_override.insert(p, Cfa::RGGB);
                    }
                }
                _ => {
                    let next = sc.cfa_known().map_or(Cfa::RGGB, Cfa::next);
                    if let Some(p) = sc.path.clone() {
                        sc.cfa_override.insert(p, next);
                    }
                    sc.debayer = true;
                }
            }
            let on = sc.cfa();
            self.science_restyle();
            on
        } else {
            return;
        };
        let msg = match (a, on) {
            (Action::NextBayerPattern, Some(c)) => t!("science-bayer-pattern", pattern = c.name()),
            (_, Some(c)) => t!("science-debayer-on", pattern = c.name()),
            (_, None) => t!("science-debayer-off"),
        };
        self.chrome.toast(msg, None, now);
        self.redraw = true;
    }

    /// The slider position under logical point `lp`, when it is over a track.
    fn science_slider_at(&self, lp: (f32, f32)) -> Option<(usize, u32)> {
        let rtl = self.rtl();
        let s = self.science.summary.as_ref()?;
        self.science.tracks.iter().find_map(|(axis, r)| {
            let hit = Rect::new(r.x - 8.0, r.y - 10.0, r.w + 16.0, r.h + 20.0);
            hit.contains(lp)
                .then(|| (*axis, slider_value(*r, lp.0, s.axes[*axis].len, rtl)))
        })
    }

    /// A press: on a slider's track it jumps there and starts a drag. True when taken.
    pub(super) fn science_press(&mut self, lp: (f32, f32)) -> bool {
        if !self.science.active() || self.gallery_active() {
            return false;
        }
        // A catalogue source under the pointer: highlighted in the SAMP applications.
        if self.astro.samp.connected
            && let Some(id) = self.science.id
            && let Some(m) = self.science_markers(id)
        {
            let to_screen = self.view.stored_from_screen().invert();
            let ls = self.dl.scale.max(0.1);
            let placed: Vec<sky::Marker> = m
                .iter()
                .map(|m| {
                    let q = to_screen.apply(m.at);
                    sky::Marker {
                        at: (q.0 / ls, q.1 / ls),
                        ..*m
                    }
                })
                .collect();
            let hit = self
                .science_tip(&placed)
                .map(|(k, _)| (placed[k].cat, placed[k].row));
            self.astro_press(hit);
        }
        let Some((axis, v)) = self.science_slider_at(lp) else {
            return false;
        };
        self.science.drag = Some(axis);
        self.science_step(axis, Some(v), 0);
        true
    }

    /// Ends a slider drag; true when there was one.
    pub(super) fn science_release(&mut self) -> bool {
        self.science.drag.take().is_some()
    }

    /// Pointer motion: drives a slider drag and updates the readout. True while dragging.
    pub(super) fn science_motion(&mut self, p: (f32, f32)) -> bool {
        if let Some(axis) = self.science.drag {
            let lp = self.logical(p);
            let rtl = self.rtl();
            if let (Some(s), Some((_, r))) = (
                self.science.summary.clone(),
                self.science
                    .tracks
                    .iter()
                    .find(|(a, _)| *a == axis)
                    .copied(),
            ) {
                let v = slider_value(r, lp.0, s.axes[axis].len, rtl);
                if v != self.science.index[axis] {
                    self.science_step(axis, Some(v), 0);
                }
            }
            return true;
        }
        if self.science.active() {
            self.science_readout_at(Some(p));
            // The hovered catalogue source's tooltip follows the pointer.
            if !self.science.catalogues.is_empty() {
                self.redraw = true;
            }
        }
        false
    }

    /// The readout read out to screen readers: the lines about the point under the pointer,
    /// or every line when `all`. `None` outside science mode.
    pub(super) fn science_cursor_lines(&self, all: bool) -> Option<&[String]> {
        let sc = &self.science;
        let r = &sc.readout;
        let at = sc.readout_at.start.min(r.len())..sc.readout_at.end.min(r.len());
        sc.active().then(|| if all { &r[..] } else { &r[at] })
    }

    fn science_readout(&mut self) {
        let p = self.pointer;
        self.science_readout_at(p);
    }

    /// The stored-pixel rectangle the viewport shows: x0, y0, x1, y1.
    fn science_visible(&self) -> [f64; 4] {
        let vp = self.view.viewport;
        let from = self.view.stored_from_screen();
        let corners = [
            (vp.x, vp.y),
            (vp.x + vp.w, vp.y),
            (vp.x, vp.y + vp.h),
            (vp.x + vp.w, vp.y + vp.h),
        ]
        .map(|c| from.apply(c));
        let lo = |f: fn(&(f32, f32)) -> f32| corners.iter().map(f).fold(f32::MAX, f32::min);
        let hi = |f: fn(&(f32, f32)) -> f32| corners.iter().map(f).fold(f32::MIN, f32::max);
        [lo(|c| c.0), lo(|c| c.1), hi(|c| c.0), hi(|c| c.1)].map(f64::from)
    }

    /// An unstructured mesh's edges in view (stored px), cached until the view moves;
    /// `None` without a mesh or with more than [`MAX_MESH_EDGES`] of them in view.
    fn science_mesh_edges(&mut self) -> Option<MeshEdges> {
        let id = self.science.id?;
        let mesh = self.science.summary.as_ref()?.mesh.clone()?;
        let visible = self.science_visible();
        let key = visible.map(f64::to_bits);
        if let Some((i, k, e)) = &self.science.edge_cache
            && *i == id
            && *k == key
        {
            return e.clone();
        }
        let e = mesh.edges(visible, MAX_MESH_EDGES).map(Arc::new);
        self.science.edge_cache = Some((id, key, e.clone()));
        e
    }

    /// Rebuilds the readout: the value(s) and sky position under the pointer and the look
    /// (the planes are on the sliders). Redraws when it changed.
    fn science_readout_at(&mut self, p: Option<(f32, f32)>) {
        let sc = &self.science;
        let mut lines = Vec::new();
        if let Some(text) = &sc.ion_prompt {
            lines.push(t!("science-ion-prompt", text = text.clone()));
            lines.push(t!(
                "science-ion-prompt-hint",
                tolerance = ion::tolerance_text(sc.ion_tolerance)
            ));
        }
        let mut at_lines = lines.len()..lines.len();
        if let Some(s) = sc.summary.as_ref().filter(|_| sc.active()) {
            // A B1950 image's conversions, said with its sky position under the pointer
            // (read out with it), else among the notes.
            let mut fk4 = s
                .wcs
                .as_ref()
                .map(|w| {
                    sky::fk4_notes(
                        w,
                        sc.catalogues.iter().map(|(_, c)| c.as_ref()),
                        self.astro.samp.connected,
                    )
                })
                .unwrap_or_default();
            let at = p
                .filter(|p| super::rect_contains(self.view.viewport, *p))
                .map(|p| self.view.stored_from_screen().apply(p))
                .filter(|&(x, y)| {
                    x >= 0.0 && y >= 0.0 && x < s.width as f32 && y < s.height as f32
                });
            let shown_cfa = sc.shown.as_ref().and_then(|r| r.cfa);
            let fft = sc.shown.as_ref().is_some_and(|r| r.fft);
            if fft {
                if let Some((x, y)) = at {
                    if let Some(v) = sc
                        .planes
                        .first()
                        .and_then(|pl| pl.plane.value(x as u32, y as u32))
                    {
                        lines.push(t!("science-fft-power", value = value_text(v)));
                    }
                    lines.extend(fft_lines(s, x, y));
                }
                at_lines.end = lines.len();
                lines.push(t!("science-fft"));
            } else if let Some((x, y)) = at {
                // (channel, value) pairs; a composite leaves its hidden channels out.
                let vals: Vec<(usize, f32)> = match (shown_cfa, sc.planes.first()) {
                    (Some(cfa), Some(pl)) => {
                        let p = &pl.plane;
                        let (w, h) = (p.width as usize, p.height as usize);
                        let (x, y) = (x as usize, y as usize);
                        if x < w && y < h && p.values.len() == w * h {
                            cfa.debayer_at(&p.values, w, h, x, y)
                                .into_iter()
                                .enumerate()
                                .collect()
                        } else {
                            Vec::new()
                        }
                    }
                    _ => sc
                        .planes
                        .iter()
                        .enumerate()
                        .filter(|(k, _)| {
                            sc.planes.len() == 1 || !sc.hidden.get(*k).copied().unwrap_or(false)
                        })
                        .filter_map(|(k, pl)| Some((k, pl.plane.value(x as u32, y as u32)?)))
                        .collect(),
                };
                if let Some(cy) = &s.cytometry {
                    let count = vals.first().map(|v| v.1);
                    lines.extend(cytometry_lines(
                        cy,
                        &sc.index,
                        f64::from(x),
                        f64::from(y),
                        count,
                    ));
                } else if let Some(mesh) = s.mesh.as_ref().filter(|_| sc.planes.len() == 1) {
                    lines.extend(mesh_lines(
                        mesh.as_ref(),
                        f64::from(x),
                        f64::from(y),
                        &sc.index,
                        s.unit_at(&sc.index),
                    ));
                } else if vals.len() == 1
                    && sc.planes.len() == 1
                    && let Some(scheme) = s.classes
                    && let Some(n) = scheme.class_of(vals[0].1)
                    && let Some(name) = class_name(scheme, n)
                {
                    lines.push(t!(
                        "science-value-class",
                        value = n.to_string(),
                        class = name
                    ));
                } else if vals.len() == 1 && sc.planes.len() == 1 {
                    let v = value_text(vals[0].1);
                    let flags = s
                        .flags_at(&sc.index)
                        .then(|| flag_bits(vals[0].1))
                        .flatten();
                    let unit = match sc.shown.as_ref().and_then(|r| r.thermal) {
                        Some((_, u)) => Some(u.symbol()),
                        None => s.unit_at(&sc.index).map(str::trim),
                    };
                    lines.push(match (flags, unit) {
                        // The whole number, never in scientific notation.
                        (Some(bits), _) => {
                            let n = (vals[0].1 as u64).to_string();
                            t!("science-value-flags", value = n, bits = bits)
                        }
                        (None, Some(u)) => {
                            t!("science-value-unit", value = v, unit = u.to_owned())
                        }
                        (None, None) => t!("science-value", value = v),
                    });
                } else {
                    // Four channels a line, so many channels never run off the readout.
                    for chunk in vals.chunks(4) {
                        let text = chunk
                            .iter()
                            .map(|&(k, v)| {
                                let name = match shown_cfa {
                                    Some(_) => ["R", "G", "B"][k.min(2)].to_owned(),
                                    None => sc.channel_name(k),
                                };
                                format!("{name} {}", value_text(v))
                            })
                            .collect::<Vec<_>>()
                            .join("  ");
                        lines.push(t!("science-values", values = text));
                    }
                }
                if let Some(ms) = &s.mass_spec {
                    lines.extend(ion::spot_lines(ms, &sc.index, x as u32, y as u32));
                }
                // The map view writes positions in the chosen format.
                let map_lines = self.gis_position_lines(f64::from(x), f64::from(y));
                if let Some(l) = map_lines {
                    lines.extend(l);
                } else if let Some(seis) = &s.seismic {
                    lines.extend(seismic_lines(seis, &sc.index, x as u32, y as u32));
                } else if let Some(w) = &s.wcs
                    && let Some(c) = &w.celestial
                    && let Some((l, b)) =
                        w.sky_at(f64::from(x), f64::from(y), &wcs_rest(s, w, &sc.index))
                {
                    lines.extend(sky_lines(&c.frame, l, b));
                    lines.append(&mut fk4);
                } else if let Some(g) = &s.geo {
                    lines.extend(super::geo::geo_position_lines(
                        g,
                        f64::from(x),
                        f64::from(y),
                    ));
                } else if let Some((lon, lat)) = s
                    .lonlat
                    .as_ref()
                    .and_then(|g| g.at(f64::from(x), f64::from(y)))
                {
                    // Geolocation arrays (an HDF-EOS swath).
                    lines.push(t!(
                        "geo-lonlat",
                        lon = specular_i18n::fixed_plain(lon, 6),
                        lat = specular_i18n::fixed_plain(lat, 6)
                    ));
                } else if let Some(why) = s.wcs.as_ref().and_then(|w| w.unsupported.as_ref()) {
                    // Never sky coordinates from a header only half understood.
                    lines.push(wcs_unsupported(why));
                } else if let Some(l) = s
                    .diffraction
                    .as_ref()
                    .and_then(|g| scatter_line(g, f64::from(x), f64::from(y)))
                {
                    lines.push(l);
                } else if let Some(r) = &s.root {
                    lines.extend(root_lines(r, x as u32, y as u32, &sc.index));
                } else if let Some(c) = &s.pixel {
                    let (x, y) = (f64::from(x.floor()), f64::from(y.floor()));
                    lines.push(t!(
                        "science-position",
                        x = quantity_text(c.x.at(x), &c.unit_x),
                        y = quantity_text(c.y.at(y), &c.unit_y)
                    ));
                }
                if let Some(o) = &s.orientations {
                    lines.extend(euler_lines(o, x, y, s.width, s.height));
                }
                at_lines.end = lines.len();
            }
            lines.append(&mut fk4);
            lines.extend(sc.thermal_lines());
            if let Some(r) = &s.root {
                lines.extend(root_notes(r));
            }
            if let Some(cy) = &s.cytometry {
                lines.extend(cytometry_notes(cy, &sc.index));
            }
            if s.mass_spec.is_some() {
                lines.push(match sc.shown.as_ref().and_then(|r| r.ion) {
                    Some(w) => t!("science-ion-window", window = ion::window_text(w)),
                    None => t!("science-ion-tic"),
                });
            }
            if !s.raster_only && !fft {
                match sc.stretch_now() {
                    // Its limits are its own.
                    Stretch::Auto => lines.push(stretch_name(Stretch::Auto)),
                    s => {
                        let file = sc
                            .shown
                            .as_ref()
                            .is_some_and(|r| r.file_range(&r.index).is_some());
                        let limits = if file {
                            t!("science-limits-file")
                        } else {
                            limits_name(sc.limits_now())
                        };
                        lines.push(t!(
                            "science-look",
                            stretch = stretch_name(s),
                            limits = limits
                        ));
                    }
                }
                match (sc.cfa(), sc.cfa_known()) {
                    (Some(c), _) => lines.push(t!("science-debayer-on", pattern = c.name())),
                    (None, Some(_)) => lines.push(t!("science-debayer-off")),
                    _ => {}
                }
            }
            if sc.level && s.levelling {
                lines.push(t!("science-levelled"));
            }
            // The sun's position, for the shade (slope and aspect colours have their legend).
            if let Some(h) = sc.hillshade_now().filter(|h| h.kind == Relief::Shade) {
                lines.push(t!(
                    "science-hillshaded",
                    azimuth = h.azimuth.to_string(),
                    altitude = h.altitude.to_string()
                ));
            }
        }
        let spectrum = self.science_spectrum_at(p);
        if spectrum != self.science.spectrum {
            self.science.spectrum = spectrum;
            self.redraw = true;
        }
        self.science.readout_at = at_lines;
        if lines != self.science.readout {
            self.science.readout = lines;
            self.redraw = true;
        }
    }

    /// The spectrum of the hypermap pixel under the pointer (kept while the pointer stays on
    /// the pixel; `None` off the image or without spectra).
    fn science_spectrum_at(&self, p: Option<(f32, f32)>) -> Option<Arc<SpectrumPlot>> {
        let sc = &self.science;
        let s = sc.summary.as_ref().filter(|_| sc.active())?;
        let spectra = s.spectra.as_ref()?;
        if sc.shown.as_ref().is_some_and(|r| r.fft) {
            return None;
        }
        let (x, y) = p
            .filter(|p| super::rect_contains(self.view.viewport, *p))
            .map(|p| self.view.stored_from_screen().apply(p))
            .filter(|&(x, y)| x >= 0.0 && y >= 0.0 && x < s.width as f32 && y < s.height as f32)?;
        let at = (x as u32, y as u32);
        if let Some(old) = sc
            .spectrum
            .as_ref()
            .filter(|o| o.at == at && o.index == sc.index)
        {
            return Some(old.clone());
        }
        let spectrum = spectra.at(at.0, at.1, &sc.index)?;
        let (name, unit) = spectra.x_axis();
        Some(Arc::new(SpectrumPlot {
            at,
            index: sc.index.clone(),
            spectrum,
            unit: if unit.trim().is_empty() { name } else { unit },
        }))
    }

    /// The readout's lines when it shows.
    pub(super) fn science_lines(&self) -> Option<Vec<String>> {
        (self.science_shown() && !self.science.readout.is_empty())
            .then(|| self.science.readout.clone())
    }

    /// The sliders, scale bar and grid over the image, in logical px.
    pub(super) fn science_overlay(&mut self) -> Option<Overlay> {
        if !self.science_shown() {
            return None;
        }
        let id = self.science.id?;
        let s = self.science.summary.clone()?;
        let comp = self
            .science
            .composite
            .then(|| self.science.composite_axis())
            .flatten();
        let sliders = (0..s.axes.len())
            .filter(|&a| s.axes[a].len >= 2 && Some(a) != comp)
            .map(|a| SliderRow {
                axis: a,
                name: axis_name(&s, a),
                world: axis_world(&s, a, self.science.index[a]),
                index: self.science.index[a],
                len: s.axes[a].len,
                selected: a == self.science.axis,
            })
            .collect();
        let ls = self.dl.scale.max(0.1);
        let px_per_pixel = f64::from(self.view.scale() / ls);
        let vp = self.view.viewport;
        let centre = self
            .view
            .stored_from_screen()
            .apply((vp.x + vp.w / 2.0, vp.y + vp.h / 2.0));
        let scale = match (&s.wcs, &s.pixel) {
            _ if s.geo.is_some() => s.geo.as_deref().and_then(|g| {
                super::geo::geo_scale_bar(g, f64::from(centre.0), f64::from(centre.1), px_per_pixel)
            }),
            (Some(w), _) => match &w.unsupported {
                Some(why) => Some(ScaleBar {
                    px: 0.0,
                    label: wcs_unsupported(why),
                }),
                None => w
                    .arcsec_per_pixel()
                    .and_then(|a| scale_bar(a, px_per_pixel)),
            },
            (None, Some(c)) => linear_scale_bar(c.x.step.abs(), &c.unit_x, px_per_pixel),
            (None, None) => s
                .root
                .as_deref()
                .and_then(|r| root_scale_bar(r, px_per_pixel)),
        };
        let fft = self.science.shown.as_ref().is_some_and(|r| r.fft);
        // A spectrum's pixels are frequencies: no lengths on it.
        let scale = scale.filter(|_| !fft);
        let mut grid = Vec::new();
        // Sky coordinates, else coastlines and a graticule on a georeferenced image, else a
        // diffraction frame's resolution rings; none on a power spectrum.
        let wcs = s.wcs.as_ref().filter(|w| w.celestial.is_some());
        let lines = if let Some(cy) = &s.cytometry {
            // A cytometry plot's axes are always drawn.
            Some(Arc::new(cytometry_ticks(cy, &self.science.index)))
        } else if !self.science.grid || fft {
            None
        } else if wcs.is_some() || s.geo.is_some() {
            let rest = wcs.map_or_else(Vec::new, |w| wcs_rest(&s, w, &self.science.index));
            Some(match &self.science.grid_cache {
                Some((i, r, l)) if *i == id && *r == rest => l.clone(),
                _ => {
                    let l = Arc::new(match (wcs, &s.geo) {
                        (Some(w), _) => super::science::grid(w, s.width, s.height, &rest),
                        (None, Some(g)) => coast::geo_grid(g, s.width, s.height),
                        (None, None) => Vec::new(),
                    });
                    self.science.grid_cache = Some((id, rest, l.clone()));
                    l
                }
            })
        } else if let Some(g) = &s.diffraction {
            Some(match &self.science.grid_cache {
                Some((i, _, l)) if *i == id => l.clone(),
                _ => {
                    let l = Arc::new(ring_lines(g, s.width, s.height));
                    self.science.grid_cache = Some((id, Vec::new(), l.clone()));
                    l
                }
            })
        } else {
            None
        };
        let to_screen = self.view.stored_from_screen().invert();
        let place = |lines: &[GridLine]| -> Vec<GridLine> {
            lines
                .iter()
                .map(|g| GridLine {
                    pts: g
                        .pts
                        .iter()
                        .map(|&p| {
                            let q = to_screen.apply(p);
                            (q.0 / ls, q.1 / ls)
                        })
                        .collect(),
                    label: g.label.clone(),
                })
                .collect()
        };
        if let Some(lines) = lines {
            grid = place(&lines);
        }
        let wiggle = match (self.science.seismic(), self.science.planes.first()) {
            (Some(_), Some(p)) if self.science.wiggle && self.science.planes.len() == 1 => {
                let clip = p.limits.map_or(0.0, |(lo, hi)| lo.abs().max(hi.abs()));
                let from = self.view.stored_from_screen();
                let corners = [
                    (vp.x, vp.y),
                    (vp.x + vp.w, vp.y),
                    (vp.x, vp.y + vp.h),
                    (vp.x + vp.w, vp.y + vp.h),
                ]
                .map(|c| from.apply(c));
                let lo = |f: fn(&(f32, f32)) -> f32| corners.iter().map(f).fold(f32::MAX, f32::min);
                let hi = |f: fn(&(f32, f32)) -> f32| corners.iter().map(f).fold(f32::MIN, f32::max);
                let visible = (lo(|c| c.0), lo(|c| c.1), hi(|c| c.0), hi(|c| c.1));
                let to_screen = from.invert();
                wiggles(&p.plane, clip, visible, px_per_pixel as f32, |q| {
                    let s = to_screen.apply(q);
                    (s.0 / ls, s.1 / ls)
                })
            }
            _ => None,
        };
        let edges = if self.science.wireframe {
            self.science_mesh_edges().map_or_else(Vec::new, |e| {
                e.iter()
                    .map(|e| {
                        let a = to_screen.apply((e[0], e[1]));
                        let b = to_screen.apply((e[2], e[3]));
                        [(a.0 / ls, a.1 / ls), (b.0 / ls, b.1 / ls)]
                    })
                    .collect()
            })
        } else {
            Vec::new()
        };
        let mut shapes = Vec::new();
        if !self.science.annotations_hidden
            && let Some(a) = &s.annotations
        {
            let to_screen = self.view.stored_from_screen().invert();
            let view = Rect::new(vp.x / ls, vp.y / ls, vp.w / ls, vp.h / ls);
            let pixel = s.pixel.clone();
            let index = &self.science.index;
            shapes = super::annotations::place_with(
                &a.set,
                |p| {
                    let q = to_screen.apply(p);
                    (q.0 / ls, q.1 / ls)
                },
                view,
                |r| {
                    let c = pixel.as_ref()?;
                    (c.unit_x == c.unit_y)
                        .then(|| quantity_text(r.length(c.x.step.abs(), c.y.step.abs()), &c.unit_x))
                },
                |k| a.opacity(k, index),
            );
        }
        let contours = self
            .science_contours(id)
            .map(|l| place(&l))
            .unwrap_or_default();
        let markers: Vec<sky::Marker> = self
            .science_markers(id)
            .map(|m| {
                m.iter()
                    .map(|m| {
                        let q = to_screen.apply(m.at);
                        sky::Marker {
                            at: (q.0 / ls, q.1 / ls),
                            ..*m
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        let (hovered, tip) = self
            .science_tip(&markers)
            .map_or((None, None), |(k, t)| (Some(k), Some(t)));
        let astro = self.astro_overlay(id, &markers, &|p: (f64, f64)| {
            let q = to_screen.apply((p.0 as f32, p.1 as f32));
            (q.0 / ls, q.1 / ls)
        });
        Some(Overlay {
            sliders,
            scale,
            grid,
            spectrum: self.science.spectrum.clone(),
            wiggle,
            edges,
            shapes,
            contours,
            markers,
            hovered,
            tip,
            astro,
        })
    }

    /// A screen reader's action on a slider; true when the node is science mode's.
    pub(super) fn science_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        let id = req.target_node.0;
        if !(A11Y_ID..A11Y_ID + 0x1000).contains(&id) {
            return false;
        }
        if id >= A11Y_SLIDER {
            let axis = (id - A11Y_SLIDER) as usize;
            if let Some(len) = self
                .science
                .summary
                .as_ref()
                .and_then(|s| s.axes.get(axis))
                .map(|a| a.len)
            {
                match req.action {
                    accesskit::Action::Increment => self.science_step(axis, None, 1),
                    accesskit::Action::Decrement => self.science_step(axis, None, -1),
                    accesskit::Action::SetValue => {
                        if let Some(accesskit::ActionData::NumericValue(v)) = req.data {
                            let v = (v.round().max(1.0) as u32).min(len) - 1;
                            self.science_step(axis, Some(v), 0);
                        }
                    }
                    accesskit::Action::Focus => self.science.axis = axis,
                    _ => {}
                }
            }
        }
        self.redraw = true;
        true
    }
}

/// The position on a track `r` (logical px) of `len` positions under `x`; right to left in
/// right-to-left layouts.
pub(super) fn slider_value(r: Rect, x: f32, len: u32, rtl: bool) -> u32 {
    let mut f = ((x - r.x) / r.w.max(1.0)).clamp(0.0, 1.0);
    if rtl {
        f = 1.0 - f;
    }
    ((f * (len.max(1) - 1) as f32).round() as u32).min(len.saturating_sub(1))
}

/// Ink of the coordinate grid.
pub(super) const GRID_INK: Color = Color::rgba(0.55, 0.95, 0.65, 0.8);
pub(super) use sky::{CONTOUR_INK, draw_markers, draw_tip};

/// Draws coordinate lines or contours (logical px) in `ink`, clipped to `clip`.
pub(super) fn draw_grid(
    dl: &mut DrawList,
    text: &mut Text,
    lines: &[GridLine],
    clip: Rect,
    ts: f32,
    ink: Color,
) {
    dl.push_clip(clip);
    for g in lines {
        for w in g.pts.windows(2) {
            dl.line(w[0], w[1], 1.0, ink);
        }
    }
    let st = TextStyle::new(FONT_SMALL * ts).tabular();
    let mut placed = Vec::new();
    for g in lines.iter().filter(|g| !g.label.is_empty()) {
        let Some(&p) = g.pts.iter().find(|p| clip.inset(2.0).contains(**p)) else {
            continue;
        };
        let b = text.layout(&g.label, st, None, Align::Start);
        let x = p.0.clamp(
            clip.x + 2.0,
            (clip.right() - b.width - 6.0).max(clip.x + 2.0),
        );
        let y = p.1.clamp(
            clip.y + 2.0,
            (clip.bottom() - b.height - 4.0).max(clip.y + 2.0),
        );
        let r = Rect::new(x, y, b.width + 6.0, b.height + 2.0);
        // Labels meeting in a corner: the first one stays.
        if placed.iter().any(|q: &Rect| {
            r.x < q.right() && q.x < r.right() && r.y < q.bottom() && q.y < r.bottom()
        }) {
            continue;
        }
        placed.push(r);
        dl.rect(r, 3.0, Color::rgba(0.0, 0.0, 0.0, 0.55));
        text.draw(dl, &b, x + 3.0, y + 1.0, ink.with_alpha(1.0));
    }
    dl.pop_clip();
}

/// Draws the plane sliders in a panel at the bottom-start corner of `area` (logical x, y,
/// w, h); returns the panel and each slider's track.
pub(super) fn draw_sliders(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    rows: &[SliderRow],
    area: (f32, f32, f32, f32),
    rtl: bool,
    ts: f32,
) -> (Rect, Vec<(usize, Rect)>) {
    let (ax, ay, aw, ah) = area;
    let m = TOOLBAR_MARGIN;
    let st = TextStyle::new(FONT_SMALL * ts).tabular();
    let labels: Vec<_> = rows
        .iter()
        .map(|r| {
            let s = slider_text(r);
            text.layout(&s, st, None, Align::Start)
        })
        .collect();
    let track_h = 16.0 * ts;
    let gap = 6.0 * ts;
    let tw = (TRACK_W * ts).min(aw - 2.0 * m - 2.0 * PAD).max(40.0);
    let w = labels.iter().map(|b| b.width).fold(tw, f32::max) + 2.0 * PAD;
    let h = labels.iter().map(|b| b.height + track_h).sum::<f32>()
        + gap * rows.len().saturating_sub(1) as f32
        + 2.0 * PAD;
    let x = if rtl { ax + aw - m - w } else { ax + m };
    let y = ay + ah - m - h;
    let panel = Rect::new(x, y, w, h);
    dl.glass(panel, RADIUS_MD, pal.glass, pal.glass_border, 1.0);
    let mut tracks = Vec::new();
    let mut ty = y + PAD;
    for (r, b) in rows.iter().zip(&labels) {
        let lx = if rtl { x + w - PAD - b.width } else { x + PAD };
        let c = if r.selected { pal.text } else { pal.text_muted };
        text.draw(dl, b, lx, ty, c);
        ty += b.height;
        let tx = if rtl { x + w - PAD - tw } else { x + PAD };
        let cy = ty + track_h / 2.0;
        let track = Rect::new(tx, cy - 2.0 * ts, tw, 4.0 * ts);
        dl.rect(track, 2.0 * ts, pal.separator);
        let f = if r.len > 1 {
            r.index as f32 / (r.len - 1) as f32
        } else {
            0.0
        };
        let fx = if rtl {
            tx + tw * (1.0 - f)
        } else {
            tx + tw * f
        };
        let ink = if r.selected {
            pal.accent
        } else {
            pal.text_muted
        };
        let filled = if rtl {
            Rect::new(fx, track.y, tx + tw - fx, track.h)
        } else {
            Rect::new(tx, track.y, fx - tx, track.h)
        };
        dl.rect(filled, 2.0 * ts, ink);
        dl.circle((fx, cy), 6.0 * ts, ink);
        tracks.push((r.axis, track));
        ty += track_h + gap;
    }
    (panel, tracks)
}

/// A slider's text: its axis, position and what the position stands for.
fn slider_text(r: &SliderRow) -> String {
    let (index, count) = (i64::from(r.index) + 1, i64::from(r.len));
    match &r.world {
        Some(world) => t!(
            "science-plane-world",
            axis = r.name.clone(),
            index = index,
            count = count,
            world = world.clone()
        ),
        None => t!(
            "science-plane",
            axis = r.name.clone(),
            index = index,
            count = count
        ),
    }
}

/// Draws a pixel's spectrum as a chart at the end side of the image `area` (logical), above
/// the readout box `below` when there is one; returns its rectangle.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_spectrum(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    plot: &SpectrumPlot,
    area: (f32, f32, f32, f32),
    below: Option<Rect>,
    rtl: bool,
    ts: f32,
) -> Rect {
    let (ax, ay, aw, ah) = area;
    let m = TOOLBAR_MARGIN;
    let w = (320.0 * ts).min(aw - 2.0 * m).max(120.0);
    let h = (160.0 * ts).min(ah / 2.0 - 2.0 * m).max(80.0);
    let x = if rtl { ax + m } else { ax + aw - m - w };
    let bottom = below.map_or(ay + ah - m, |b| b.y - m);
    let y = (bottom - h).max(ay + m);
    let r = Rect::new(x, y, w, h);
    dl.rect_border(
        r,
        RADIUS_MD,
        pal.surface.with_alpha(0.92),
        pal.separator,
        1.0,
    );
    let pad = PAD * ts * 0.75;
    let st = TextStyle::new(FONT_SMALL * ts).weight(600.0);
    let small = TextStyle::new(FONT_SMALL * ts).tabular();
    let title = text.layout(
        &t!(
            "science-spectrum-title",
            x = plot.at.0 as i64,
            y = plot.at.1 as i64
        ),
        st,
        Some(w - 2.0 * pad),
        Align::Start,
    );
    let tx = if rtl {
        x + w - pad - title.width
    } else {
        x + pad
    };
    text.draw(dl, &title, tx, y + pad, pal.text);
    let sp = &plot.spectrum;
    let n = sp.y.len();
    if n < 2 && !(sp.sticks && n == 1) {
        return r;
    }
    let peak =
        sp.y.iter()
            .filter(|v| v.is_finite())
            .fold(0.0f32, |a, &b| a.max(b));
    let peak = if peak > 0.0 { peak } else { 1.0 };
    // The x extent: the first and last values (channels, wavelengths), or the smallest and
    // largest (m/z values in any order).
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for k in 0..n {
        let v = sp.x_at(k);
        if v.is_finite() {
            lo = lo.min(v);
            hi = hi.max(v);
        }
    }
    // No finite x value at all.
    if lo > hi {
        return r;
    }
    if hi == lo {
        (lo, hi) = (lo - 0.5, hi + 0.5);
    }
    let unit = plot.unit.trim();
    let ends = [lo, hi].map(|e| {
        let v = world_text(e);
        text.layout(
            &if unit.is_empty() {
                v
            } else {
                format!("{v} {unit}")
            },
            small,
            None,
            Align::Start,
        )
    });
    let top = text.layout(&value_text(peak), small, None, Align::Start);
    let label_w = top.width + 4.0 * ts;
    let py0 = y + pad + title.height + 4.0 * ts;
    let py1 = y + h - pad - ends[0].height - 2.0 * ts;
    let (px0, px1) = if rtl {
        (x + pad, x + w - pad - label_w)
    } else {
        (x + pad + label_w, x + w - pad)
    };
    if py1 - py0 < 10.0 || px1 - px0 < 10.0 {
        return r;
    }
    let axis = pal.text_faint;
    dl.line((px0, py1), (px1, py1), 1.0, axis);
    let ax_x = if rtl { px1 } else { px0 };
    dl.line((ax_x, py0), (ax_x, py1), 1.0, axis);
    let lx = if rtl { px1 + 4.0 * ts } else { x + pad };
    text.draw(dl, &top, lx, py0 - top.height / 2.0, pal.text_muted);
    let (sx, ex) = if rtl {
        (px1 - ends[0].width, px0)
    } else {
        (px0, px1 - ends[1].width)
    };
    text.draw(dl, &ends[0], sx, py1 + 2.0 * ts, pal.text_muted);
    text.draw(dl, &ends[1], ex, py1 + 2.0 * ts, pal.text_muted);
    // One column per screen px: the values whose x falls in it at their highest, so narrow
    // peaks stay.
    let cols = (px1 - px0).max(1.0) as usize;
    let column = |c: usize| (c as f32 + 0.5) / cols as f32;
    let mut top_of = vec![f32::NAN; cols];
    for k in 0..n {
        let (xv, v) = (sp.x_at(k), sp.y[k]);
        if !xv.is_finite() || !v.is_finite() {
            continue;
        }
        let c = (((xv - lo) / (hi - lo)) * cols as f64).floor() as usize;
        let t = &mut top_of[c.min(cols - 1)];
        *t = if t.is_nan() { v } else { t.max(v) };
    }
    let colour = pal.accent;
    let at = |c: usize, v: f32| {
        let f = column(c);
        let fx = if rtl { 1.0 - f } else { f };
        (
            px0 + fx * (px1 - px0),
            py1 - (v / peak).clamp(0.0, 1.0) * (py1 - py0),
        )
    };
    if sp.sticks {
        // Centroided peaks: a stick per column holding one.
        for (c, &v) in top_of.iter().enumerate() {
            if v.is_finite() {
                let p = at(c, v);
                dl.line((p.0, py1), p, 1.0 * ts, colour);
            }
        }
    } else {
        // A curve through the columns holding values (straight across empty ones).
        let mut prev: Option<(f32, f32)> = None;
        for (c, &v) in top_of.iter().enumerate() {
            if v.is_nan() {
                continue;
            }
            let p = at(c, v.max(0.0));
            if let Some(q) = prev {
                dl.line(q, p, 1.5 * ts, colour);
            }
            prev = Some(p);
        }
    }
    r
}

/// Draws the scale bar at the bottom-start corner of `area`, above `below` (the sliders'
/// panel) when there is one; returns its rectangle.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_scale(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    bar: &ScaleBar,
    area: (f32, f32, f32, f32),
    below: Option<Rect>,
    rtl: bool,
    ts: f32,
) -> Rect {
    let (ax, ay, aw, ah) = area;
    let m = TOOLBAR_MARGIN;
    let st = TextStyle::new(FONT_SMALL * ts).tabular();
    let b = text.layout(&bar.label, st, None, Align::Start);
    // A bar of no length is a message in the bar's place (WCS not supported).
    let has_bar = bar.px > 0.0;
    let w = bar.px.max(b.width) + 2.0 * PAD;
    let h = b.height + if has_bar { 10.0 * ts } else { 0.0 } + 2.0 * PAD;
    let bottom = below.map_or(ay + ah - m, |r| r.y - 8.0 * ts);
    let x = if rtl { ax + aw - m - w } else { ax + m };
    let y = bottom - h;
    let r = Rect::new(x, y, w, h);
    dl.glass(r, RADIUS_MD, pal.glass, pal.glass_border, 1.0);
    let lx = x + (w - b.width) / 2.0;
    text.draw(dl, &b, lx, y + PAD, pal.text);
    if !has_bar {
        return r;
    }
    let by = y + PAD + b.height + 6.0 * ts;
    let x0 = x + (w - bar.px) / 2.0;
    let x1 = x0 + bar.px;
    let lw = 2.0 * ts;
    dl.line((x0, by), (x1, by), lw, pal.text);
    dl.line((x0, by - 4.0 * ts), (x0, by + 4.0 * ts), lw, pal.text);
    dl.line((x1, by - 4.0 * ts), (x1, by + 4.0 * ts), lw, pal.text);
    r
}

/// The sliders' panel, rows and tracks, for [`a11y`].
pub(super) type SliderNodes<'a> = (Rect, &'a [SliderRow], &'a [(usize, Rect)]);

/// The AccessKit nodes: the readout (a status, not live), the sliders (with their values,
/// stepped and set by a screen reader) and the scale bar.
pub(super) fn a11y(
    tree: &mut accesskit::TreeUpdate,
    lines: Option<(&[String], Rect)>,
    sliders: Option<SliderNodes<'_>>,
    scale: Option<(&ScaleBar, Rect)>,
    sf: f64,
) {
    use accesskit::{Node, NodeId, Orientation, Role};
    let bounds = |r: Rect| accesskit::Rect {
        x0: f64::from(r.x) * sf,
        y0: f64::from(r.y) * sf,
        x1: f64::from(r.x + r.w) * sf,
        y1: f64::from(r.y + r.h) * sf,
    };
    let mut top = Vec::new();
    if let Some((lines, r)) = lines {
        let mut n = Node::new(Role::Status);
        n.set_label(format!("{}: {}", t!("science-readout"), lines.join(", ")));
        n.set_bounds(bounds(r));
        top.push((NodeId(A11Y_ID), n));
    }
    let mut children = Vec::new();
    if let Some((panel, rows, tracks)) = sliders {
        let mut g = Node::new(Role::Group);
        g.set_label(t!("science-sliders"));
        g.set_bounds(bounds(panel));
        for r in rows {
            let id = NodeId(A11Y_SLIDER + r.axis as u64);
            let mut n = Node::new(Role::Slider);
            n.set_label(r.name.clone());
            n.set_orientation(Orientation::Horizontal);
            n.set_numeric_value(f64::from(r.index) + 1.0);
            n.set_min_numeric_value(1.0);
            n.set_max_numeric_value(f64::from(r.len));
            n.set_numeric_value_step(1.0);
            n.set_value(slider_text(r));
            if let Some((_, t)) = tracks.iter().find(|(a, _)| *a == r.axis) {
                n.set_bounds(bounds(*t));
            }
            n.add_action(accesskit::Action::Increment);
            n.add_action(accesskit::Action::Decrement);
            n.add_action(accesskit::Action::SetValue);
            n.add_action(accesskit::Action::Focus);
            g.push_child(id);
            children.push((id, n));
        }
        top.push((NodeId(A11Y_SLIDERS), g));
    }
    if let Some((bar, r)) = scale {
        let mut n = Node::new(Role::Label);
        n.set_label(t!("science-scale-a11y", length = bar.label.clone()));
        n.set_bounds(bounds(r));
        top.push((NodeId(A11Y_SCALE), n));
    }
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        for (id, _) in &top {
            root.push_child(*id);
        }
    }
    tree.nodes.extend(top);
    tree.nodes.extend(children);
}

/// `v` with a sign ("+12.5", "−3.0") and `digits` decimals in the UI's number format.
fn signed(v: f64, digits: u8) -> String {
    let sign = if v < 0.0 { '\u{2212}' } else { '+' };
    format!("{sign}{}", specular_i18n::fixed_plain(v.abs(), digits))
}
