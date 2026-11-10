//! Compare (DESIGN.md §6.3): 2–4 images from the gallery's selection (`c`), the palette's
//! "Compare with…" or `specular --compare`, in one of four modes, with one zoom and centre
//! for all of them.
//!
//! - **Side by side**: each image in a cell of its own (a 2×2 grid for four on a narrow
//!   window), fitted to it; zoom and pan move every image alike, or with two images each on
//!   its own once unlinked (`l`, for shots that do not line up).
//! - **Split** (`split.rs`): the second image (or the one picked with ←/→) over the first,
//!   cut at a divider that is dragged or stepped, down or across.
//! - **Overlay**: the second image over the first, blended with a slider; `t` flickers
//!   between them, `Shift+T` by itself (at most [`FLICKER_START`] changes a second until
//!   the setting allows more: photosensitivity). Both are rendered every frame into
//!   textures of their own, so a flicker only changes how the two are drawn.
//! - **Difference**: each further image against the first, as the absolute, squared,
//!   relative or signed error, or the error over the dimmed first image
//!   (`specular_inspect::diff`), with MAE, MSE, PSNR, SSIM and ΔE2000 of the visible region
//!   or the whole image (`Shift+M`) from GPU reductions (`specular_render::Comparer`).
//!
//! An image (or frame) smaller than its cell is never enlarged to fit: at fit it shows at
//! 100 %, centred, as in the gallery and viewer; zoomed in, its pane grows until it fills
//! the cell, and only then is cropped and panned (`QuickPane::zoomed`).
//!
//! In the split, overlay and difference view the images share one frame. How images of
//! different sizes sit in it (`align.rs`): top-left (the default), centred, scaled to the
//! first, or aligned automatically. They are compared over their overlap, and the caption
//! says so. The pixel probe (`probe.rs`) reads both images under the pointer.
//!
//! The metrics are taken from what the viewer draws: linear light (BT.709, 1.0 = SDR white)
//! after any gain map and tone mapping for the display, premultiplied, after channel
//! isolation; alpha counts as a fourth channel when either image has alpha (normal view).
//! MAE, MSE, PSNR and SSIM are taken in the metric space (linear light by default, or
//! encoded sRGB: the setting, and `m`); ΔE2000 always in CIELAB. The alpha-over-checkerboard
//! and matte views compare the plain image (their backdrop is no part of either image).
//!
//! Every mode goes through the image renderer once it shows anything but the thumbnails
//! (quick.rs): a channel view, a zoom past fit, the overlay or the difference view.

use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use specular_gfx::{Backdrop, DrawList, Rect, Text, TextStyle};
use specular_i18n::t;
use specular_input::fixed::CompareCmd;
use specular_input::{Key, KeyCombo, NamedKey};
use specular_inspect::{ChannelView, DiffKind, MetricSpace, diff};
use specular_log::{RENDER, debug, info, warn};
use specular_render::{Comparer, ComparerBuild, DiffParams, MetricPair, MetricResult, Place};
use specular_theme::{Color, Palette};
use specular_widgets::{PaletteItem, accesskit};

use super::draw::{Frame, text_in, truncate};
use super::quick::{DiffPane, QuickPane, QuickPlan, compare_cells};
use super::{A11Y_BASE, App, EntryKind, Gallery, item};
use specular_gfx::Icon;

mod align;
mod metrics;
mod probe;
mod split;

pub(in crate::app) use align::AlignMode;
use align::Auto;
use probe::Probe;
use split::Split;

/// Compare's palette codes.
pub(super) mod code {
    pub const SIDE: u64 = 600;
    pub const OVERLAY: u64 = 601;
    pub const DIFF: u64 = 602; // + 0..3 (absolute, squared, relative)
    pub const FLICKER: u64 = 605;
    pub const FIT: u64 = 606;
    pub const ACTUAL: u64 = 607;
    pub const SPLIT: u64 = 608;
    pub const SPLIT_DIRECTION: u64 = 609;
    pub const AUTO_FLICKER: u64 = 610;
    pub const ALIGN: u64 = 611;
    pub const SPACE: u64 = 612;
    pub const WHOLE: u64 = 613;
    pub const PROBE: u64 = 614;
    pub const LINK: u64 = 615;
    /// The difference kinds after the first three (signed, over the image).
    pub const DIFF_MORE: u64 = 620; // + 0..2

    pub fn owns(c: u64) -> bool {
        (600..650).contains(&c)
    }
}

/// Compare's screen-reader nodes: the group, then its controls and panes.
pub(super) const A11Y_COMPARE: u64 = A11Y_BASE + (6 << 40);
const A11Y_CONTROL: u64 = 1;
const A11Y_PANE: u64 = 100;

const BODY: TextStyle = TextStyle::new(13.0);
const SMALL: TextStyle = TextStyle::new(11.0);

/// Zoom steps of `+` / `-` and the wheel's notch.
const ZOOM_STEP: f32 = 1.25;
/// Blend steps of `[` / `]` in the overlay.
const BLEND_STEP: f32 = 0.1;
/// Flickers / metric runs of the `--show` benchmarks.
const BENCH_FLICKERS: usize = 40;
const BENCH_METRICS: usize = 20;
/// A benchmark whose images are not on the GPU by then gives up (screenshots go on).
const BENCH_START: Duration = Duration::from_secs(20);
/// Polling the metric readback.
const POLL: Duration = Duration::from_millis(2);
/// Auto-flicker: the rate it starts at (image changes a second; never above the setting's
/// limit, 3 unless the user raises it), its step and its slowest.
pub(super) const FLICKER_START: f32 = 2.0;
const FLICKER_STEP: f32 = 0.5;
const FLICKER_SLOWEST: f32 = 1.0;

/// What compare shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(in crate::app) enum CmpMode {
    #[default]
    Side,
    Split,
    Overlay,
    Diff,
}

impl CmpMode {
    /// From `--compare-mode` (`side`, `split`, `overlay`, `diff`).
    pub(in crate::app) fn from_id(s: &str) -> Option<CmpMode> {
        Some(match s.trim() {
            "side" => CmpMode::Side,
            "split" => CmpMode::Split,
            "overlay" => CmpMode::Overlay,
            "diff" | "difference" => CmpMode::Diff,
            _ => return None,
        })
    }

    /// The second image is drawn over the first in one pane.
    fn layered(self) -> bool {
        matches!(self, CmpMode::Split | CmpMode::Overlay)
    }
}

/// Auto-flicker: changes a second and when the next one is due.
#[derive(Clone, Copy, Debug)]
pub(super) struct AutoFlicker {
    pub rate: f32,
    pub next: Instant,
}

/// A control on compare's toolbar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CAct {
    Side,
    Split,
    Overlay,
    Diff,
    Flicker,
    AutoFlicker,
    Blend,
    Divider,
    SplitDirection,
    Kind,
    ScaleDown,
    ScaleUp,
    Space,
    Align,
    Link,
    Probe,
    Zoom,
}

/// Every control, in reading order (their screen-reader ids follow it).
/// The split's divider first: it is over the image, above the toolbar.
const CONTROLS: [CAct; 17] = [
    CAct::Divider,
    CAct::Side,
    CAct::Split,
    CAct::Overlay,
    CAct::Diff,
    CAct::Flicker,
    CAct::AutoFlicker,
    CAct::Blend,
    CAct::SplitDirection,
    CAct::Kind,
    CAct::ScaleDown,
    CAct::ScaleUp,
    CAct::Space,
    CAct::Align,
    CAct::Link,
    CAct::Probe,
    CAct::Zoom,
];

/// The `--show` benchmarks: flicker latency, or difference-metric time.
#[derive(Debug)]
struct Bench {
    metrics: bool,
    since: Instant,
    started: bool,
    samples: Vec<f64>,
    record: Vec<f64>,
}

/// Compare's state besides the images (`Gallery::compare`).
#[derive(Debug)]
pub(in crate::app) struct CompareState {
    pub(super) mode: CmpMode,
    pub(super) kind: DiffKind,
    /// Zoom over fit (1: the whole image) about `at`, a normalised display-space point.
    pub(super) zoom: f32,
    pub(super) at: (f32, f32),
    /// The overlay's top image (1…n − 1; the first is underneath).
    pub(super) b: usize,
    /// The overlay's opacity of the top image (0: the first image only).
    pub(super) blend: f32,
    /// The difference view's scale exponent (`DiffKind::full_scale`).
    pub(super) k: i32,
    /// Side by side with two images: zoom and pan together (else each pane at its own
    /// centre, `ats`).
    pub(super) link: bool,
    pub(super) ats: [(f32, f32); 4],
    pub(super) split: Split,
    /// The space MAE, MSE, PSNR and SSIM are taken in (from the setting when compare opens).
    pub(super) space: MetricSpace,
    /// Measure the whole overlap rather than what is in view.
    pub(super) whole: bool,
    pub(super) align: AlignMode,
    auto: Auto,
    pub(super) flicker: Option<AutoFlicker>,
    pub(super) probe: Probe,
    /// This frame's panes by image (for the probe) and every image's place in the frame.
    geo: Vec<(usize, QuickPane)>,
    pub(super) places: Vec<Place>,
    /// Either compared image is HDR (ΔE clips it, and the caption says so).
    hdr: bool,
    /// Physical px per logical px of the last plan.
    scale: f32,
    /// 100 % (one image pixel per screen pixel) as a zoom over fit, for the first image.
    z100: f32,
    /// How far the first image (or the frame) grows with the zoom before it is cropped:
    /// a small image shows at 100 % at fit and grows until it fills its pane
    /// (`QuickPane::zoomed`); 1 for a large one.
    grow: f32,
    /// Display sizes of the images (once known), for the captions.
    pub(super) sizes: Vec<Option<(f32, f32)>>,
    /// Metrics per compared image (1…3), of the pairs measured last.
    metrics: Vec<(usize, MetricResult)>,
    measured_key: u64,
    in_flight: Option<u64>,
    /// This frame's metric pairs (compared image, pair) and their key.
    pairs: Vec<(usize, MetricPair)>,
    pairs_key: u64,
    pub(in crate::app) comparer: Option<Comparer>,
    build: Option<ComparerBuild>,
    /// Panning: the pointer's last position.
    drag: Option<(f32, f32)>,
    slider_drag: bool,
    /// The toolbar's controls as drawn, the blend slider's track and the image rects.
    hits: Vec<(Rect, CAct)>,
    slider: Option<Rect>,
    panes: Vec<(usize, Rect)>,
    /// When a flicker happened (until the frame showing it is presented).
    toggled: Option<Instant>,
    bench: Option<Bench>,
}

impl Default for CompareState {
    fn default() -> Self {
        CompareState {
            mode: CmpMode::Side,
            kind: DiffKind::Absolute,
            zoom: 1.0,
            at: (0.5, 0.5),
            b: 1,
            blend: 0.5,
            k: diff::DEFAULT_SCALE,
            link: true,
            ats: [(0.5, 0.5); 4],
            split: Split::default(),
            space: MetricSpace::Linear,
            whole: false,
            align: AlignMode::TopLeft,
            auto: Auto::None,
            flicker: None,
            probe: Probe::default(),
            geo: Vec::new(),
            places: Vec::new(),
            hdr: false,
            scale: 1.0,
            z100: 1.0,
            grow: 1.0,
            sizes: Vec::new(),
            metrics: Vec::new(),
            measured_key: 0,
            in_flight: None,
            pairs: Vec::new(),
            pairs_key: 0,
            comparer: None,
            build: None,
            drag: None,
            slider_drag: false,
            hits: Vec::new(),
            slider: None,
            panes: Vec::new(),
            toggled: None,
            bench: None,
        }
    }
}

impl CompareState {
    /// A new comparison: back to side by side at fit in `space` (the GPU passes are kept).
    pub(super) fn reset(&mut self, space: MetricSpace) {
        let comparer = self.comparer.take();
        let build = self.build.take();
        self.auto.cancel();
        *self = CompareState {
            comparer,
            build,
            space,
            ..CompareState::default()
        };
    }

    /// The images go through the image renderer (rather than showing their thumbnails):
    /// in a channel view (`normal` false), zoomed, with the probe on, or in any mode but
    /// side by side.
    pub(super) fn renders(&self, normal: bool) -> bool {
        !normal || self.zoom > 1.0 || self.mode != CmpMode::Side || self.probe.on
    }

    /// Which of `paths` the current mode renders.
    pub(super) fn rendered(&self, paths: &[PathBuf]) -> Vec<PathBuf> {
        if self.mode.layered() {
            return [0, self.b.min(paths.len().saturating_sub(1))]
                .iter()
                .filter_map(|&k| paths.get(k).cloned())
                .collect();
        }
        paths.to_vec()
    }

    /// The difference view's metrics are in for what it shows, and an automatic alignment
    /// is found or given up (true in the other modes).
    pub(super) fn measured(&self) -> bool {
        let aligned = self.align != AlignMode::Auto || !self.auto.running();
        aligned
            && (self.mode != CmpMode::Diff
                || self.comparer.is_some()
                    && self.in_flight.is_none()
                    && self.measured_key == self.pairs_key)
    }

    /// Every image's place in the frame under the alignment, for `sizes` (kept in
    /// `places`); returns the frame.
    pub(super) fn place_images(&mut self) -> (f32, f32) {
        let (frame, places) =
            align::places(self.align, &self.sizes, self.b, self.auto.found(self.b));
        self.places = places;
        frame
    }

    /// Shots: the probe at logical point `p` over panes `geo` (by image); returns the
    /// texels to read (pane, physical px) for A and B.
    #[cfg(test)]
    pub(super) fn probe_shot(
        &mut self,
        geo: Vec<(usize, QuickPane)>,
        p: (f32, f32),
        scale: f32,
    ) -> probe::Wanted {
        self.geo = geo;
        self.probe.on = true;
        let hits =
            probe::locate(&self.geo, &self.places, p, scale, [0, self.b]).unwrap_or([None, None]);
        for (k, h) in hits.iter().enumerate() {
            self.probe.px[k] =
                h.and_then(|h| probe::info(&h, specular_widgets::SourceState::Unavailable, None));
        }
        [0, 1].map(|k| hits[k].and_then(|h| h.texel.map(|t| (h.pane, t))))
    }

    /// Shots: the probe's readback came in.
    #[cfg(test)]
    pub(super) fn probe_read(&mut self, wanted: [bool; 2], texels: &[[f32; 4]]) {
        self.probe.read(wanted, texels);
    }

    /// The panes pan apart (two images side by side, unlinked).
    pub(super) fn unlinked(&self, n: usize) -> bool {
        !self.link && n == 2 && self.mode == CmpMode::Side
    }

    /// Auto-flicker on (entering the overlay) at the slower of [`FLICKER_START`] and
    /// `limit`, or off.
    pub(super) fn toggle_auto_flicker(&mut self, limit: f32, now: Instant) {
        if self.flicker.take().is_some() {
            return;
        }
        self.mode = CmpMode::Overlay;
        let rate = FLICKER_START.min(flicker_limit(limit));
        self.flicker = Some(AutoFlicker {
            rate,
            next: now + Duration::from_secs_f32(1.0 / rate),
        });
    }

    /// `[` / `]` with auto-flicker on: slower or faster, never past `limit`.
    pub(super) fn step_flicker(&mut self, up: bool, limit: f32) {
        if let Some(f) = self.flicker.as_mut() {
            let r = f.rate + if up { FLICKER_STEP } else { -FLICKER_STEP };
            f.rate = r.clamp(
                FLICKER_SLOWEST.min(flicker_limit(limit)),
                flicker_limit(limit),
            );
        }
    }

    /// Whether the caption gives the alignment (the images differ in size, or are aligned
    /// automatically).
    fn note_shown(&self) -> bool {
        let first = self.sizes.first().copied().flatten();
        self.align == AlignMode::Auto || self.sizes.iter().flatten().any(|s| Some(*s) != first)
    }

    /// The metrics of the pairs of images `keys` (shots).
    #[cfg(test)]
    pub(super) fn set_metrics(&mut self, keys: &[usize], r: specular_render::MetricsReadback) {
        self.metrics = r
            .pairs
            .into_iter()
            .zip(keys)
            .filter_map(|(m, k)| m.map(|m| (*k, m)))
            .collect();
        self.measured_key = self.pairs_key;
    }

    /// Caption lines under each pane: the difference view adds its metrics, and a note on
    /// the alignment when the images differ in size.
    pub(super) fn caption_lines(&self) -> usize {
        if self.mode != CmpMode::Diff {
            return 1;
        }
        if self.note_shown() { 4 } else { 3 }
    }

    /// A benchmark runs (screenshots wait for it).
    pub(super) fn bench_running(&self) -> bool {
        self.bench.is_some()
    }

    /// The panes laid out: Side shows every image, Overlay one cell, Diff one per image
    /// after the first.
    fn cells_for(&self, n: usize) -> usize {
        match self.mode {
            CmpMode::Side => n,
            CmpMode::Split | CmpMode::Overlay => 1,
            CmpMode::Diff => n.saturating_sub(1),
        }
    }

    fn max_zoom(&self) -> f32 {
        (self.z100 * 8.0).max(8.0)
    }

    /// The zoom set to `z` about the normalised point `u` of the panes (the point under it
    /// stays put), the centre held where the image covers the pane.
    pub(super) fn zoom_about(&mut self, z: f32, u: (f32, f32)) {
        let z0 = self.crop_zoom();
        let z = z.clamp(1.0, self.max_zoom());
        let c = clamp_at(self.at, z0);
        let q = (c.0 + (u.0 - 0.5) / z0, c.1 + (u.1 - 0.5) / z0);
        self.zoom = z;
        let z1 = self.crop_zoom();
        self.at = clamp_at((q.0 - (u.0 - 0.5) / z1, q.1 - (u.1 - 0.5) / z1), z1);
    }

    /// The zoom past what the panes show whole: a small image's pane grows with the zoom
    /// first (`grow`), and only the rest crops it.
    fn crop_zoom(&self) -> f32 {
        crop_of(self.zoom, self.grow)
    }

    /// Pans by `d` logical px on a pane `r` large.
    fn pan(&mut self, d: (f32, f32), r: Rect) {
        self.at = panned(self.at, self.crop_zoom(), d, r);
    }

    /// Pans pane `k` alone (unlinked).
    fn pan_pane(&mut self, k: usize, d: (f32, f32), r: Rect) {
        let z = self.crop_zoom();
        if let Some(at) = self.ats.get_mut(k) {
            *at = panned(*at, z, d, r);
        }
    }

    /// Links the panes again (or unlinks them, each starting where the shared view is).
    fn toggle_link(&mut self) {
        self.link = !self.link;
        self.ats = [self.at; 4];
    }

    /// Fit, or 100 % of the first image.
    fn toggle_actual(&mut self) {
        if (self.zoom - 1.0).abs() < 1e-3 && self.z100 > 1.0 + 1e-3 {
            self.zoom_about(self.z100, (0.5, 0.5));
        } else {
            self.zoom = 1.0;
            self.at = (0.5, 0.5);
        }
    }

    /// The zoom as a percentage of 100 %.
    fn zoom_percent(&self) -> f32 {
        self.zoom / self.z100.max(1e-6) * 100.0
    }

    fn step_scale(&mut self, up: bool) {
        let (lo, hi) = diff::SCALE_RANGE;
        self.k = (self.k + if up { 1 } else { -1 }).clamp(lo, hi);
    }

    /// `d`: the difference view, then its next kind, then side by side.
    fn next_kind(&mut self) {
        match (self.mode, self.kind.next()) {
            (CmpMode::Diff, Some(k)) => self.kind = k,
            (CmpMode::Diff, None) => self.mode = CmpMode::Side,
            _ => {
                self.mode = CmpMode::Diff;
                self.kind = DiffKind::Absolute;
            }
        }
    }

    fn step_blend(&mut self, up: bool) {
        let b = self.blend + if up { BLEND_STEP } else { -BLEND_STEP };
        self.blend = (b / BLEND_STEP).round() * BLEND_STEP;
        self.blend = self.blend.clamp(0.0, 1.0);
    }
}

/// The rate limit from the setting (changes a second), kept sane.
pub(super) fn flicker_limit(setting: f32) -> f32 {
    if setting.is_finite() {
        setting.clamp(FLICKER_SLOWEST, 10.0)
    } else {
        3.0
    }
}

/// The zoom `zoom` (over fit) past what a pane that grows `grow` times with it shows whole.
fn crop_of(zoom: f32, grow: f32) -> f32 {
    let z = zoom.max(1.0);
    z / z.min(grow.max(1.0))
}

/// Centre `at` panned by `d` logical px on a pane `r` large at zoom `zoom` (past whole).
fn panned(at: (f32, f32), zoom: f32, d: (f32, f32), r: Rect) -> (f32, f32) {
    let z = zoom.max(1.0);
    let c = clamp_at(at, z);
    clamp_at(
        (
            c.0 - d.0 / (r.w.max(1.0) * z),
            c.1 - d.1 / (r.h.max(1.0) * z),
        ),
        z,
    )
}

/// The centre held far enough from the edges that the image covers the pane at zoom `z`.
fn clamp_at(at: (f32, f32), z: f32) -> (f32, f32) {
    let half = 0.5 / z.max(1.0);
    (at.0.clamp(half, 1.0 - half), at.1.clamp(half, 1.0 - half))
}

/// The display size of an image `stored` px large under orientation `swaps`.
fn display_of(w: u32, h: u32, swaps: bool) -> (f32, f32) {
    if swaps {
        (h as f32, w as f32)
    } else {
        (w as f32, h as f32)
    }
}

/// The largest width and height among `sizes`: the frame the overlay and difference view
/// align every image in (top-left).
pub(super) fn frame_of(sizes: &[Option<(f32, f32)>]) -> (f32, f32) {
    sizes
        .iter()
        .flatten()
        .fold((1.0f32, 1.0f32), |f, s| (f.0.max(s.0), f.1.max(s.1)))
}

/// The overlap of two images aligned top-left (`align::overlap` for any alignment).
#[cfg(test)]
fn overlap(a: (f32, f32), b: (f32, f32)) -> (f32, f32) {
    (a.0.min(b.0), a.1.min(b.1))
}

/// The metrics' region: the visible part of the frame (`visible`) within the overlap `o`
/// (both frame rectangles), or None when empty.
pub(super) fn metric_region(
    visible: (f32, f32, f32, f32),
    o: (f32, f32, f32, f32),
) -> Option<(f32, f32, f32, f32)> {
    let r = (
        visible.0.max(o.0),
        visible.1.max(o.1),
        visible.2.min(o.2),
        visible.3.min(o.3),
    );
    (r.2 - r.0 >= 0.5 && r.3 - r.1 >= 0.5).then_some(r)
}

/// The channel the difference view compares: the alpha views over a backdrop compare the
/// plain image.
pub(super) fn diff_channel(c: ChannelView) -> ChannelView {
    match c {
        ChannelView::AlphaChecker | ChannelView::AlphaMatte => ChannelView::Normal,
        c => c,
    }
}

/// The error kind's name.
fn kind_name(k: DiffKind) -> String {
    match k {
        DiffKind::Absolute => t!("compare-kind-absolute"),
        DiffKind::Squared => t!("compare-kind-squared"),
        DiffKind::Relative => t!("compare-kind-relative"),
        DiffKind::Signed => t!("compare-kind-signed"),
        DiffKind::OnImage => t!("compare-kind-on-image"),
    }
}

/// The alignment's name.
fn align_name(a: AlignMode) -> String {
    match a {
        AlignMode::TopLeft => t!("compare-align-top-left"),
        AlignMode::Centre => t!("compare-align-centre"),
        AlignMode::ScaleToA => t!("compare-align-scaled"),
        AlignMode::Auto => t!("compare-align-auto"),
    }
}

/// The metric space's name.
fn space_name(s: MetricSpace) -> String {
    match s {
        MetricSpace::Linear => t!("compare-space-linear"),
        MetricSpace::Srgb => t!("compare-space-srgb"),
    }
}

/// A control's key, as the key help and screen readers give it.
fn act_key(a: CAct) -> Option<&'static str> {
    Some(match a {
        CAct::Side | CAct::Overlay => "O",
        CAct::Split => "S",
        CAct::SplitDirection => "H",
        CAct::Diff | CAct::Kind => "D",
        CAct::Flicker => "T",
        CAct::AutoFlicker => "Shift+T",
        CAct::Blend | CAct::ScaleDown | CAct::Divider => "[",
        CAct::ScaleUp => "]",
        CAct::Space => "M",
        CAct::Align => "A",
        CAct::Link => "L",
        CAct::Probe => "P",
        CAct::Zoom => "Ctrl+1",
    })
}

fn median(v: &[f64]) -> (f64, f64, f64) {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    match s.len() {
        0 => (0.0, 0.0, 0.0),
        n => (s[0], s[n / 2], s[n - 1]),
    }
}

impl App {
    /// The setting's metric space.
    fn metric_space_setting(&self) -> MetricSpace {
        match self.settings.gallery.compare_metric_space {
            specular_settings::CompareMetricSpace::Linear => MetricSpace::Linear,
            specular_settings::CompareMetricSpace::Srgb => MetricSpace::Srgb,
        }
    }

    /// The fastest auto-flicker the setting allows (changes a second).
    fn flicker_setting(&self) -> f32 {
        flicker_limit(self.settings.gallery.compare_flicker_limit as f32)
    }

    /// Compares `paths` (2–4 images; `--compare`, "Compare with…") in `mode`: the gallery
    /// of the first one's folder, and compare over it. Images from other folders are
    /// compared by path (listed after the folder's for opening).
    pub(in crate::app) fn compare_files(&mut self, paths: Vec<PathBuf>, mode: CmpMode) {
        let paths: Vec<PathBuf> = paths
            .into_iter()
            .map(|p| std::path::absolute(&p).unwrap_or(p))
            .collect();
        if !(2..=4).contains(&paths.len()) {
            let now = self.now();
            self.chrome.toast(t!("gallery-compare-hint"), None, now);
            return;
        }
        let dir = paths[0].parent().map(std::path::Path::to_path_buf);
        self.gallery_enter_at(dir, Some(paths[0].clone()));
        self.gallery.selected = paths.iter().cloned().collect();
        self.compare_start(paths, mode);
    }

    /// "Compare with…": the current image and another chosen in a file dialog (in the
    /// background; [`App::tick_compare`] takes the answer).
    pub(in crate::app) fn compare_with_open(&mut self) {
        let Some(cur) = self.current.and_then(|i| self.entries.get(i)).cloned() else {
            return;
        };
        let dir = cur.parent().map(std::path::Path::to_path_buf);
        let title = t!("compare-with-title");
        let (tx, rx) = std::sync::mpsc::channel();
        self.gallery.compare_with = Some((cur, rx));
        self.loader.background("compare-with", move |l| {
            let mut dlg = rfd::FileDialog::new().set_title(title);
            if let Some(d) = dir {
                dlg = dlg.set_directory(d);
            }
            if let Some(p) = dlg.pick_file() {
                let _ = tx.send(p);
            }
            l.waker()();
        });
    }

    /// The file dialog of "Compare with…" answered (or was closed).
    fn poll_compare_with(&mut self) {
        let Some((a, rx)) = &self.gallery.compare_with else {
            return;
        };
        match rx.try_recv() {
            Ok(b) => {
                let a = a.clone();
                self.gallery.compare_with = None;
                self.compare_files(vec![a, b], CmpMode::Side);
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => self.gallery.compare_with = None,
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }

    /// Opens compare on `paths` (2–4 images) in `mode`, from fit.
    pub(in crate::app) fn compare_start(&mut self, paths: Vec<PathBuf>, mode: CmpMode) {
        let space = self.metric_space_setting();
        let ga = &mut self.gallery;
        ga.focus = paths.first().cloned();
        ga.compare = (paths.len() >= 2).then_some(paths);
        ga.wish_key = 0;
        ga.cmp.reset(space);
        ga.cmp.mode = mode;
        ga.dirty = true;
        self.redraw = true;
    }

    /// [`App::quick_plan`] for compare: the images it renders this frame through the image
    /// renderer, and the differences drawn from them (`size`: the window in logical px).
    pub(in crate::app) fn compare_plan(
        &mut self,
        size: (f32, f32),
        scale: f32,
        backdrop: Backdrop,
    ) -> Option<QuickPlan> {
        let Some(paths) = self.gallery.compare.clone() else {
            self.review_tiles(&[]);
            return None;
        };
        let n = paths.len();
        // The images' display sizes and alpha, once known.
        let infos: Vec<Option<((f32, f32), bool)>> = paths
            .iter()
            .map(|p| {
                let img = self.images.get(self.by_path.get(p)?)?;
                let info = img.info.as_ref()?;
                let o = img.orient.unwrap_or(info.orientation);
                Some((
                    display_of(info.width, info.height, o.swaps_axes()),
                    info.has_alpha,
                ))
            })
            .collect();
        let hdr = paths.iter().any(|p| {
            self.by_path
                .get(p)
                .and_then(|id| self.gpu_images.get(*id))
                .is_some_and(|g| g.hdr)
        });
        let sizes: Vec<Option<(f32, f32)>> = infos.iter().map(|i| i.map(|i| i.0)).collect();
        let any_alpha = infos.iter().flatten().any(|i| i.1);
        let normal = self.chrome.channel_view(true) == ChannelView::Normal;
        let cap = self.gallery.quick_caption_h();
        let ga = &mut self.gallery;
        let cmp = &mut ga.cmp;
        if cmp.b == 0 || cmp.b >= n {
            cmp.b = 1;
        }
        cmp.sizes = sizes.clone();
        let frame = cmp.place_images();
        let places = cmp.places.clone();
        cmp.hdr = hdr;
        cmp.scale = scale;
        let cells = compare_layout(
            size.0,
            size.1,
            cmp.cells_for(n),
            cap,
            cmp.caption_lines(),
            ga.text_scale,
        );
        // 100 %: one display pixel of the first image (or the frame) per physical pixel.
        let first = if cmp.mode == CmpMode::Side {
            sizes.first().copied().flatten()
        } else {
            Some(frame)
        };
        if let (Some((dw, dh)), Some((_, area))) = (first, cells.first()) {
            let fit = (area.w * scale / dw.max(1.0)).min(area.h * scale / dh.max(1.0));
            cmp.z100 = (1.0 / fit.max(1e-6)).max(1.0);
            cmp.grow = super::quick::enlargement(dw, dh, *area, scale);
        }
        if !cmp.renders(normal) {
            cmp.pairs.clear();
            cmp.geo.clear();
            self.review_tiles(&[]);
            return None;
        }
        let (mode, zoom, at, b) = (cmp.mode, cmp.zoom, cmp.at, cmp.b);
        let (kind, k, diffs_on) = (cmp.kind, cmp.k, cmp.comparer.is_some());
        let (space, whole, unlinked, ats) = (cmp.space, cmp.whole, cmp.unlinked(n), cmp.ats);
        let zoomed = mode != CmpMode::Side || zoom > 1.0 || cmp.probe.on;
        let shown: Vec<usize> = if mode.layered() {
            vec![0, b]
        } else {
            (0..n).collect()
        };
        let ch = diff_channel(self.chrome.channel_view(any_alpha));
        let mut panes = Vec::new();
        let mut idx = vec![None; n];
        for i in shown {
            let area = match mode {
                CmpMode::Side => cells.get(i),
                CmpMode::Split | CmpMode::Overlay => cells.first(),
                CmpMode::Diff => cells.get(i.saturating_sub(1)),
            };
            let Some(&(_, area)) = area else { continue };
            let Some(mut p) = self.quick_pane(paths[i].clone(), area, scale, backdrop, zoomed)
            else {
                continue;
            };
            if mode != CmpMode::Side {
                p = p.framed(frame, area, scale);
                p.place = places.get(i).copied().unwrap_or(Place::IDENTITY);
            }
            p = p.zoomed(zoom);
            p.at = if unlinked { ats[i] } else { at };
            if mode == CmpMode::Diff {
                // Over nothing, every image in the same view.
                p.opts.canvas = [0.0; 4];
                p.opts.checker_a = [0.0; 4];
                p.opts.checker_b = [0.0; 4];
                p.opts.checker_cell = 1.0e6;
                p.opts.channel = ch;
                p.channel = ch;
            }
            idx[i] = Some(panes.len());
            panes.push(p);
        }
        let alpha: Vec<bool> = infos.iter().map(|i| i.is_some_and(|i| i.1)).collect();
        let (diffs, pairs) = if mode == CmpMode::Diff && diffs_on {
            let opts = DiffOptions {
                full_scale: kind.full_scale(k),
                kind,
                channel: ch,
                space,
                whole,
            };
            diff_panes(&panes, &idx, &sizes, &places, &alpha, &opts)
        } else {
            (Vec::new(), Vec::new())
        };
        // The metrics' key: what is compared, where, and what of it is on the GPU.
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (mode, kind, space, whole).hash(&mut h);
        for (kb, p) in &pairs {
            (kb, p.a.id, p.b.id, p.channel, p.alpha).hash(&mut h);
            for v in [p.region.0, p.region.1, p.region.2, p.region.3] {
                v.to_bits().hash(&mut h);
            }
            for pl in [p.a.place, p.b.place] {
                for v in [pl.scale, pl.rotation, pl.offset.0, pl.offset.1] {
                    v.to_bits().hash(&mut h);
                }
            }
            for id in [p.a.id, p.b.id] {
                self.gpu_images.get(id).map(|g| g.revision).hash(&mut h);
            }
        }
        if let Some(g) = &self.gfx {
            g.renderer.pool.len().hash(&mut h);
        }
        let cmp = &mut self.gallery.cmp;
        cmp.pairs = pairs;
        cmp.pairs_key = h.finish() | 1;
        cmp.geo = idx
            .iter()
            .enumerate()
            .filter_map(|(i, j)| j.map(|j| (i, panes[j].clone())))
            .collect();
        self.review_tiles(&panes);
        (!panes.is_empty()).then_some(QuickPlan { panes, diffs })
    }

    /// Between frames: builds the GPU passes when the difference view or the probe needs
    /// them, picks up finished metrics, probe values and alignments, asks for new ones when
    /// what is compared changed, and flickers. Returns when to wake.
    pub(in crate::app) fn tick_compare(&mut self) -> Option<Instant> {
        self.poll_compare_with();
        if self
            .gallery
            .cmp
            .build
            .as_mut()
            .is_some_and(ComparerBuild::failed)
        {
            // The build panicked: no "Measuring…" for ever, back to side by side, and say so.
            // Choosing Difference again tries again.
            self.gallery.cmp.build = None;
            self.gallery.cmp.mode = CmpMode::Side;
            self.gallery.cmp.probe.on = false;
            let now = self.now();
            self.chrome.warning(
                t!("compare-diff-failed", reason = t!("error-reason-internal")),
                now,
            );
            self.redraw = true;
        }
        let open = self.gallery.active && self.gallery.compare.is_some();
        // `--show …-probe`: the probe at the middle of the first pane once laid out.
        if open
            && self.gallery.cmp.probe.show_at
            && let Some(r) = self.gallery.cmp.geo.first().map(|g| g.1.placed().0)
        {
            self.gallery.cmp.probe.show_at = false;
            self.compare_probe_at((r.x + r.w * 0.4, r.y + r.h * 0.5), false);
        }
        let flicker = self.tick_auto_flicker(open);
        self.tick_align(open);
        let diff_on = open && self.gallery.cmp.mode == CmpMode::Diff;
        let probe_on = open && self.gallery.cmp.probe.on;
        let Some(gfx) = self.gfx.as_mut() else {
            return flicker;
        };
        let cmp = &mut self.gallery.cmp;
        if let Some(b) = cmp.build.as_mut()
            && let Some(c) = b.try_take()
        {
            cmp.comparer = Some(c);
            cmp.build = None;
            self.redraw = true;
        }
        if (diff_on || probe_on) && cmp.comparer.is_none() && cmp.build.is_none() {
            let wake = self.loader.waker();
            cmp.build = Some(Comparer::spawn(&gfx.gpu, move || wake()));
        }
        let Some(c) = cmp.comparer.as_mut() else {
            return flicker;
        };
        // The probe: what came back, then what the pointer wants next.
        if let Some(t) = c.poll_probe(&gfx.gpu) {
            let wanted = cmp.probe.want_read.take().unwrap_or([false; 2]);
            cmp.probe.read(wanted, &t);
            self.redraw = true;
        }
        if probe_on
            && !c.probe_busy()
            && let Some(want) = cmp.probe.want.take()
        {
            let targets = &self.gallery.quick_targets;
            let texels: Vec<(&specular_gfx::wgpu::Texture, (u32, u32))> = want
                .iter()
                .flatten()
                .filter_map(|(pane, at)| targets.get(*pane).map(|t| (t.texture(), *at)))
                .collect();
            let wanted =
                [0, 1].map(|k| want[k].is_some_and(|(pane, _)| targets.get(pane).is_some()));
            let mut enc = gfx.gpu.device.create_command_encoder(&Default::default());
            if c.request_probe(&mut enc, &texels) {
                gfx.gpu.queue.submit([enc.finish()]);
                c.probe_after_submit();
                cmp.probe.want_read = Some(wanted);
            } else {
                cmp.probe.texels = [None, None];
            }
        }
        if let Some(r) = c.poll(&gfx.gpu) {
            let key = cmp.in_flight.take().unwrap_or(0);
            let ks: Vec<usize> = cmp.pairs.iter().map(|p| p.0).collect();
            debug!(target: RENDER, "compare: metrics in {:.2} ms (recorded in {:.2} ms)", r.latency_ms, c.last_record_ms);
            if diff_on {
                cmp.metrics = r
                    .pairs
                    .into_iter()
                    .zip(ks)
                    .filter_map(|(m, k)| m.map(|m| (k, m)))
                    .collect();
                cmp.measured_key = key;
                if let Some(b) = cmp.bench.as_mut().filter(|b| b.metrics) {
                    b.samples.push(r.latency_ms);
                    b.record.push(c.last_record_ms);
                    if b.samples.len() >= BENCH_METRICS {
                        let (lo, med, hi) = median(&b.samples);
                        let (rlo, rmed, rhi) = median(&b.record);
                        let px = cmp.metrics.first().map_or((0, 0), |m| m.1.samples);
                        info!(target: RENDER, "compare bench: difference metrics (MAE/MSE/PSNR, SSIM, ΔE2000) of {} × {} samples: request → result min {lo:.2} / median {med:.2} / max {hi:.2} ms; CPU recording min {rlo:.2} / median {rmed:.2} / max {rhi:.2} ms ({} runs)", px.0, px.1, b.samples.len());
                        cmp.bench = None;
                    } else {
                        // Measure again.
                        cmp.measured_key = 0;
                    }
                }
            }
            self.redraw = true;
        }
        if !diff_on {
            cmp.metrics.clear();
            cmp.measured_key = 0;
            if !c.busy() && !c.probe_busy() && !probe_on {
                c.trim();
            }
        } else if cmp.measured_key != cmp.pairs_key && cmp.in_flight.is_none() && !c.busy() {
            let key = cmp.pairs_key;
            let pairs: Vec<MetricPair> = cmp.pairs.iter().map(|p| p.1).collect();
            let mut enc = gfx.gpu.device.create_command_encoder(&Default::default());
            if c.request(
                &gfx.gpu,
                &mut gfx.renderer,
                &mut self.gpu_images,
                &mut enc,
                &pairs,
            ) {
                gfx.gpu.queue.submit([enc.finish()]);
                c.after_submit();
                cmp.in_flight = Some(key);
            } else {
                // Nothing to measure (no overlap in view, or not on the GPU yet).
                cmp.metrics.clear();
                cmp.measured_key = key;
                self.redraw = true;
            }
        }
        if let Some(b) = cmp.bench.as_mut()
            && !b.started
            && b.since.elapsed() > BENCH_START
        {
            warn!(target: RENDER, "compare bench: the images never came; giving up");
            cmp.bench = None;
        }
        let poll = (c.busy() || c.probe_busy()).then(|| Instant::now() + POLL);
        match (poll, flicker) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Auto-flicker: the change that is due, and when the next one is.
    fn tick_auto_flicker(&mut self, open: bool) -> Option<Instant> {
        let limit = self.flicker_setting();
        let cmp = &mut self.gallery.cmp;
        if !open || cmp.mode != CmpMode::Overlay {
            cmp.flicker = None;
            return None;
        }
        let f = cmp.flicker.as_mut()?;
        // The setting may have been lowered meanwhile.
        f.rate = f.rate.min(limit);
        let now = Instant::now();
        if now < f.next {
            return Some(f.next);
        }
        let period = Duration::from_secs_f32(1.0 / f.rate.max(FLICKER_SLOWEST));
        f.next = (f.next + period).max(now + period / 2);
        let next = f.next;
        cmp.blend = if cmp.blend >= 0.5 { 0.0 } else { 1.0 };
        cmp.toggled = Some(now);
        self.redraw = true;
        Some(next)
    }

    /// The automatic alignment: starts it for the image the keys act on once both
    /// overviews are in, and takes its result (falling back on top-left when it fails).
    fn tick_align(&mut self, open: bool) {
        let cmp = &mut self.gallery.cmp;
        if !open || cmp.align != AlignMode::Auto {
            cmp.auto.cancel();
            return;
        }
        let b = cmp.b;
        if let Auto::Running { rx, b: rb, .. } = &cmp.auto {
            let rb = *rb;
            match rx.try_recv() {
                Ok(Some(place)) if rb == b => {
                    info!(target: RENDER, "compare: aligned automatically: {:.3}° {:.4}× shift {:.1}, {:.1}", place.rotation.to_degrees(), place.scale, place.offset.0, place.offset.1);
                    cmp.auto = Auto::Found(align::Found { b, place });
                    self.gallery.dirty = true;
                    self.redraw = true;
                }
                Ok(_) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    cmp.auto = Auto::None;
                    cmp.align = AlignMode::TopLeft;
                    self.gallery.dirty = true;
                    self.redraw = true;
                    let now = self.now();
                    self.chrome.toast(t!("compare-align-failed"), None, now);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
            return;
        }
        if cmp.auto.found(b).is_some() {
            return;
        }
        let Some(paths) = self.gallery.compare.clone() else {
            return;
        };
        let side = |p: &PathBuf| {
            let img = self.images.get(self.by_path.get(p)?)?;
            let info = img.info.as_ref()?;
            let o = img.orient.unwrap_or(info.orientation);
            let (_, r) = img.overview.as_ref()?;
            Some((
                r.clone(),
                o,
                display_of(info.width, info.height, o.swaps_axes()),
            ))
        };
        let (Some(a), Some(bb)) = (paths.first().and_then(side), paths.get(b).and_then(side))
        else {
            return;
        };
        let token = specular_core::CancelToken::new();
        let (tx, rx) = std::sync::mpsc::channel();
        let wake = self.loader.waker();
        self.loader.pool().submit_labeled(
            specular_core::Priority::Visible,
            token.clone(),
            "compare: auto align",
            move |cancel| {
                let t0 = Instant::now();
                let found = align::find(a, bb, cancel);
                debug!(target: RENDER, "compare: auto align in {:.1} ms: {}", t0.elapsed().as_secs_f64() * 1e3, if found.is_some() { "found" } else { "nothing to trust" });
                if !cancel.is_cancelled() {
                    let _ = tx.send(found);
                }
                wake();
            },
        );
        self.gallery.cmp.auto = Auto::Running { b, rx, token };
    }

    /// After a frame is presented (at `now`): the time since a flicker (debug log, and the flicker
    /// benchmark's samples); `ready`: compare's images are on the GPU. True: draw again.
    pub(in crate::app) fn compare_presented(&mut self, ready: bool, now: Instant) -> bool {
        let cmp = &mut self.gallery.cmp;
        let bench = cmp.bench.as_mut().filter(|b| !b.metrics);
        if let Some(t) = cmp.toggled.take() {
            let ms = (now - t).as_secs_f64() * 1000.0;
            debug!(target: RENDER, "compare: flicker → present {ms:.2} ms");
            if let Some(b) = bench {
                b.samples.push(ms);
            }
        }
        let Some(b) = cmp.bench.as_mut().filter(|b| !b.metrics) else {
            return false;
        };
        if !b.started {
            if !ready {
                return false;
            }
            b.started = true;
        }
        if b.samples.len() >= BENCH_FLICKERS {
            let (lo, med, hi) = median(&b.samples);
            info!(target: RENDER, "compare bench: flicker → present min {lo:.2} / median {med:.2} / max {hi:.2} ms ({} flickers)", b.samples.len());
            cmp.bench = None;
            return true;
        }
        self.compare_flicker();
        true
    }

    /// `t`: shows the other image of the overlay (entering it), at once.
    pub(super) fn compare_flicker(&mut self) {
        let ga = &mut self.gallery;
        let Some(paths) = ga.compare.as_ref() else {
            return;
        };
        let cmp = &mut ga.cmp;
        if cmp.mode != CmpMode::Overlay {
            cmp.mode = CmpMode::Overlay;
            cmp.blend = 0.0;
        }
        cmp.blend = if cmp.blend >= 0.5 { 0.0 } else { 1.0 };
        cmp.toggled = Some(Instant::now());
        let shown = if cmp.blend > 0.5 { cmp.b } else { 0 };
        if let Some(name) = paths
            .get(shown)
            .and_then(|p| ga.by_path.get(p.as_path()))
            .map(|&i| ga.entries[i].name.clone())
        {
            let which = if shown == 0 { "A" } else { "B" };
            ga.announce = Some(t!("compare-showing-which", which = which, name = name));
        }
        self.redraw = true;
    }

    /// `Shift+T`: auto-flicker on or off, said aloud.
    fn compare_auto_flicker(&mut self) {
        let limit = self.flicker_setting();
        let now = Instant::now();
        let cmp = &mut self.gallery.cmp;
        cmp.toggle_auto_flicker(limit, now);
        let msg = match cmp.flicker {
            Some(f) => t!(
                "compare-auto-flicker-on",
                rate = diff::metric_text(f64::from(f.rate))
            ),
            None => t!("compare-auto-flicker-off"),
        };
        self.gallery.announce = Some(msg);
        self.redraw = true;
    }

    /// The next alignment (`a`), said aloud.
    fn compare_align(&mut self) {
        let cmp = &mut self.gallery.cmp;
        cmp.align = cmp.align.next();
        if cmp.align != AlignMode::Auto {
            cmp.auto.cancel();
        }
        self.gallery.announce = Some(align_name(cmp.align));
    }

    /// `p`: the probe on or off.
    fn compare_probe(&mut self) {
        let p = &mut self.gallery.cmp.probe;
        p.on = !p.on;
        p.clear();
        if p.on
            && let Some(at) = self.gallery.pointer
        {
            self.compare_probe_at(at, false);
        }
    }

    /// `l`: link or unlink two images side by side. False when it does not apply.
    fn compare_link(&mut self) -> bool {
        let n = self.gallery.compare.as_ref().map_or(0, Vec::len);
        let cmp = &mut self.gallery.cmp;
        if n != 2 {
            return false;
        }
        cmp.mode = CmpMode::Side;
        cmp.toggle_link();
        self.gallery.announce = Some(if cmp.link {
            t!("compare-linked")
        } else {
            t!("compare-unlinked")
        });
        true
    }

    /// A toolbar control, a palette entry or a screen reader's click.
    pub(super) fn compare_act(&mut self, a: CAct) {
        let cmp = &mut self.gallery.cmp;
        match a {
            CAct::Side => cmp.mode = CmpMode::Side,
            CAct::Split => cmp.mode = CmpMode::Split,
            CAct::Overlay => cmp.mode = CmpMode::Overlay,
            CAct::Diff => cmp.mode = CmpMode::Diff,
            CAct::Kind => {
                cmp.mode = CmpMode::Diff;
                cmp.kind = cmp.kind.next().unwrap_or(DiffKind::Absolute);
            }
            CAct::ScaleDown => cmp.step_scale(false),
            CAct::ScaleUp => cmp.step_scale(true),
            CAct::Zoom => cmp.toggle_actual(),
            CAct::Flicker => self.compare_flicker(),
            CAct::AutoFlicker => self.compare_auto_flicker(),
            CAct::SplitDirection => {
                cmp.mode = CmpMode::Split;
                cmp.split.across = !cmp.split.across;
            }
            CAct::Space => {
                cmp.space = match cmp.space {
                    MetricSpace::Linear => MetricSpace::Srgb,
                    MetricSpace::Srgb => MetricSpace::Linear,
                };
                self.gallery.announce = Some(space_name(self.gallery.cmp.space));
            }
            CAct::Align => self.compare_align(),
            CAct::Link => {
                self.compare_link();
            }
            CAct::Probe => self.compare_probe(),
            CAct::Blend | CAct::Divider => {}
        }
        self.gallery.dirty = true;
        self.redraw = true;
    }

    /// One of the registry's compare actions (palette, remote control). False: not one, or
    /// compare is closed (`CompareWith` opens it).
    pub(in crate::app) fn compare_action(&mut self, a: specular_input::Action) -> bool {
        use specular_input::Action as A;
        if a == A::CompareWith {
            self.compare_with_open();
            return true;
        }
        if self.gallery.compare.is_none() {
            return false;
        }
        let act = match a {
            A::CompareSide => CAct::Side,
            A::CompareSplit => CAct::Split,
            A::CompareSplitDirection => CAct::SplitDirection,
            A::CompareOverlay => CAct::Overlay,
            A::CompareDifference => {
                self.gallery.cmp.next_kind();
                self.gallery.dirty = true;
                self.redraw = true;
                return true;
            }
            A::CompareFlicker => CAct::Flicker,
            A::CompareAutoFlicker => CAct::AutoFlicker,
            A::CompareAlign => CAct::Align,
            A::CompareMetricSpace => CAct::Space,
            A::CompareWholeImage => {
                self.gallery.cmp.whole = !self.gallery.cmp.whole;
                self.gallery.dirty = true;
                self.redraw = true;
                return true;
            }
            A::CompareProbe => CAct::Probe,
            A::CompareLink => CAct::Link,
            A::CompareClose => {
                self.gallery.compare = None;
                self.gallery.cmp.auto.cancel();
                self.redraw = true;
                return true;
            }
            _ => return false,
        };
        self.gallery.active = true;
        self.compare_act(act);
        true
    }

    /// Compare's own keys (with compare open), from [`specular_input::fixed::COMPARE_KEYS`]:
    /// the modes, flicker, the alignment, the metric space and region, the probe, the link,
    /// `[` / `]` (blend, divider, gain or flicker rate), ←/→ the image the keys act on, zoom
    /// and fit. False: not here (the gallery's keys run).
    pub(super) fn compare_cmd(&mut self, cmd: CompareCmd, combo: &KeyCombo) -> bool {
        use CompareCmd as C;
        let limit = self.flicker_setting();
        let cmp = &mut self.gallery.cmp;
        match cmd {
            C::Overlay => {
                cmp.mode = if cmp.mode == CmpMode::Overlay {
                    CmpMode::Side
                } else {
                    CmpMode::Overlay
                };
            }
            C::Split => {
                cmp.mode = if cmp.mode == CmpMode::Split {
                    CmpMode::Side
                } else {
                    CmpMode::Split
                };
            }
            C::SplitDirection => self.compare_act(CAct::SplitDirection),
            C::Difference => cmp.next_kind(),
            C::Flicker => self.compare_flicker(),
            C::AutoFlicker => self.compare_auto_flicker(),
            C::Align => self.compare_align(),
            C::MetricSpace => self.compare_act(CAct::Space),
            C::WholeImage => {
                cmp.whole = !cmp.whole;
                let msg = if cmp.whole {
                    t!("compare-region-whole")
                } else {
                    t!("compare-region-view")
                };
                self.gallery.announce = Some(msg);
            }
            C::Probe => self.compare_probe(),
            C::Link => {
                if !self.compare_link() {
                    return false;
                }
            }
            C::StepDown | C::StepUp => {
                let up = cmd == C::StepUp;
                match cmp.mode {
                    CmpMode::Overlay if cmp.flicker.is_some() => cmp.step_flicker(up, limit),
                    CmpMode::Overlay => cmp.step_blend(up),
                    CmpMode::Split => cmp.split.step(up),
                    CmpMode::Diff => cmp.step_scale(up),
                    CmpMode::Side => return false,
                }
            }
            C::ZoomIn => cmp.zoom_about(cmp.zoom * ZOOM_STEP, (0.5, 0.5)),
            C::ZoomOut => cmp.zoom_about(cmp.zoom / ZOOM_STEP, (0.5, 0.5)),
            C::Fit => {
                cmp.zoom = 1.0;
                cmp.at = (0.5, 0.5);
                cmp.ats = [cmp.at; 4];
            }
            C::Actual => {
                let z = cmp.z100;
                cmp.zoom_about(z, (0.5, 0.5));
            }
            C::Pick => {
                let right = match combo.key {
                    Key::Named(NamedKey::ArrowRight) => !self.gallery.rtl,
                    Key::Named(NamedKey::ArrowLeft) => self.gallery.rtl,
                    _ => return false,
                };
                // The overlay's top image, else which image the keys act on.
                if !self.compare_pick(right) {
                    let ga = &mut self.gallery;
                    let Some(c) = ga.compare.as_ref() else {
                        return false;
                    };
                    let k = ga
                        .focus
                        .as_ref()
                        .and_then(|f| c.iter().position(|p| p == f))
                        .unwrap_or(0);
                    let k = if right {
                        (k + 1) % c.len()
                    } else {
                        (k + c.len() - 1) % c.len()
                    };
                    ga.focus = Some(c[k].clone());
                }
            }
            // `c` and Escape are the gallery's (they close compare there).
            C::Close => return false,
        }
        self.gallery.dirty = true;
        self.redraw = true;
        true
    }

    /// ←/→ in the split, the overlay and the difference view: the second image (B), over
    /// the first or measured against it. False elsewhere.
    pub(super) fn compare_pick(&mut self, right: bool) -> bool {
        let ga = &mut self.gallery;
        let Some(paths) = ga.compare.as_ref() else {
            return false;
        };
        let n = paths.len();
        if !ga.cmp.mode.layered() || n < 3 {
            return false;
        }
        let b = ga.cmp.b.clamp(1, n - 1) - 1;
        let b = if right {
            (b + 1) % (n - 1)
        } else {
            (b + n - 2) % (n - 1)
        };
        ga.cmp.b = b + 1;
        ga.focus = paths.get(b + 1).cloned();
        ga.cmp.probe.clear();
        self.redraw = true;
        true
    }

    /// The wheel over compare: zooms about the pointer.
    pub(super) fn compare_wheel(&mut self, lines: (f32, f32), pixels: bool, p: (f32, f32)) {
        let dy = if pixels { lines.1 / 40.0 } else { lines.1 };
        if dy.abs() < 1e-3 {
            return;
        }
        let cmp = &mut self.gallery.cmp;
        let u = cmp
            .panes
            .iter()
            .find(|(_, r)| r.contains(p))
            .map_or((0.5, 0.5), |(_, r)| {
                (
                    ((p.0 - r.x) / r.w.max(1.0)).clamp(0.0, 1.0),
                    ((p.1 - r.y) / r.h.max(1.0)).clamp(0.0, 1.0),
                )
            });
        cmp.zoom_about(cmp.zoom * ZOOM_STEP.powf(dy), u);
        self.redraw = true;
    }

    /// A mouse button with compare open (logical px): the toolbar, the split's divider,
    /// panning, the blend slider; the other buttons close compare.
    pub(super) fn compare_button(
        &mut self,
        down: bool,
        b: specular_widgets::Button,
        p: (f32, f32),
    ) -> bool {
        use specular_widgets::Button;
        let ga = &mut self.gallery;
        if !down {
            ga.cmp.drag = None;
            ga.cmp.slider_drag = false;
            ga.cmp.split.drag = false;
            return true;
        }
        if b != Button::Primary {
            ga.compare = None;
            ga.cmp.auto.cancel();
            return true;
        }
        let hit = ga.cmp.hits.iter().find(|(r, _)| r.contains(p)).map(|h| h.1);
        match hit {
            Some(CAct::Blend) => {
                ga.cmp.slider_drag = true;
                ga.compare_slider_to(p.0);
            }
            Some(CAct::Divider) => {
                ga.cmp.split.drag = true;
                ga.compare_divider_to(p);
            }
            Some(a) => self.compare_act(a),
            None => {
                let pane = ga.cmp.panes.iter().find(|(_, r)| r.contains(p)).copied();
                if let Some((k, _)) = pane {
                    ga.cmp.drag = Some(p);
                    if ga.cmp.mode == CmpMode::Side
                        && let Some(path) = ga.compare.as_ref().and_then(|c| c.get(k))
                    {
                        ga.focus = Some(path.clone());
                    }
                }
            }
        }
        self.redraw = true;
        true
    }

    /// The pointer moved with compare open: the probe, then panning, the blend slider or
    /// the divider. True: used.
    pub(super) fn compare_pointer_move(&mut self, p: (f32, f32)) -> bool {
        // Alt holds the probe where it is.
        if self.gallery.cmp.probe.on && !self.mods.alt_key() {
            self.compare_probe_at(p, true);
        }
        let n = self.gallery.compare.as_ref().map_or(0, Vec::len);
        let ga = &mut self.gallery;
        if ga.cmp.slider_drag {
            ga.compare_slider_to(p.0);
            self.redraw = true;
            return true;
        }
        if ga.cmp.split.drag {
            ga.compare_divider_to(p);
            self.redraw = true;
            return true;
        }
        let Some(q) = ga.cmp.drag else {
            return false;
        };
        let pane = ga
            .cmp
            .panes
            .iter()
            .find(|(_, r)| r.contains(q))
            .or(ga.cmp.panes.first())
            .copied();
        if let Some((k, r)) = pane {
            let d = (p.0 - q.0, p.1 - q.1);
            if ga.cmp.unlinked(n) {
                ga.cmp.pan_pane(k, d, r);
            } else {
                ga.cmp.pan(d, r);
            }
        }
        ga.cmp.drag = Some(p);
        self.redraw = true;
        true
    }

    /// The probe at logical point `p`: A's and B's pixels there, their file values when a
    /// full-resolution overview has them, and the texels to read for what is displayed.
    /// `keep`: the displayed values stay until the readback answers.
    pub(super) fn compare_probe_at(&mut self, p: (f32, f32), keep: bool) {
        let Some(paths) = self.gallery.compare.clone() else {
            return;
        };
        let cmp = &self.gallery.cmp;
        let scale = cmp.scale;
        let targets = [0, cmp.b.min(paths.len().saturating_sub(1))];
        let hits = probe::locate(&cmp.geo, &cmp.places, p, scale, targets);
        let Some(hits) = hits else {
            if self.gallery.cmp.probe.px.iter().any(Option::is_some) {
                self.gallery.cmp.probe.clear();
                self.redraw = true;
            }
            return;
        };
        let mut px = [None, None];
        for (k, hit) in hits.iter().enumerate() {
            let Some(hit) = hit else { continue };
            let source = hit
                .stored
                .zip(paths.get(targets[k]))
                .and_then(|(s, path)| {
                    let img = self.images.get(self.by_path.get(path)?)?;
                    let info = img.info.as_ref()?;
                    let (kind, r) = img.overview.as_ref()?;
                    (*kind == crate::loader::OverviewKind::Level(1) && info.bit_depth >= 8)
                        .then(|| specular_inspect::source_pixel(r, s.0, s.1))
                        .flatten()
                })
                .map_or(specular_widgets::SourceState::Unavailable, |v| {
                    specular_widgets::SourceState::Value(v)
                });
            let old = self.gallery.cmp.probe.px[k];
            px[k] = probe::info(hit, source, old.as_ref().filter(|_| keep));
        }
        let pr = &mut self.gallery.cmp.probe;
        if pr.px != px {
            pr.px = px;
            pr.want = Some([0, 1].map(|k| hits[k].and_then(|h| h.texel.map(|t| (h.pane, t)))));
            self.redraw = true;
        }
    }

    /// Compare's palette entries (with compare open).
    pub(super) fn compare_menu_items(&self) -> Vec<PaletteItem> {
        if self.gallery.compare.is_none() {
            return Vec::new();
        }
        let diff = |c: u64, k: DiffKind, key: Option<&'static str>| {
            item(
                c,
                t!("gallery-palette-compare-difference", kind = kind_name(k)),
                key,
                Icon::Image,
            )
        };
        let mut v = vec![
            item(
                code::SIDE,
                t!("gallery-palette-compare-side"),
                None,
                Icon::Grid,
            ),
            item(
                code::SPLIT,
                t!("palette-compare-split"),
                Some("S"),
                Icon::Image,
            ),
            item(
                code::SPLIT_DIRECTION,
                t!("palette-compare-split-direction"),
                Some("H"),
                Icon::Image,
            ),
            item(
                code::OVERLAY,
                t!("gallery-palette-compare-overlay"),
                Some("O"),
                Icon::Image,
            ),
            diff(code::DIFF, DiffKind::Absolute, Some("D")),
            diff(code::DIFF + 1, DiffKind::Squared, None),
            diff(code::DIFF + 2, DiffKind::Relative, None),
            diff(code::DIFF_MORE, DiffKind::Signed, None),
            diff(code::DIFF_MORE + 1, DiffKind::OnImage, None),
            item(
                code::FLICKER,
                t!("gallery-palette-compare-flicker"),
                Some("T"),
                Icon::Image,
            ),
            item(
                code::AUTO_FLICKER,
                t!("palette-compare-auto-flicker"),
                Some("Shift+T"),
                Icon::Image,
            ),
            item(
                code::ALIGN,
                t!("palette-compare-align"),
                Some("A"),
                Icon::Image,
            ),
            item(
                code::SPACE,
                t!("palette-compare-metric-space"),
                Some("M"),
                Icon::Image,
            ),
            item(
                code::WHOLE,
                t!("palette-compare-whole-image"),
                Some("Shift+M"),
                Icon::Image,
            ),
            item(
                code::PROBE,
                t!("palette-compare-probe"),
                Some("P"),
                Icon::Image,
            ),
        ];
        if self.gallery.compare.as_ref().is_some_and(|c| c.len() == 2) {
            v.push(item(
                code::LINK,
                t!("palette-compare-link"),
                Some("L"),
                Icon::Image,
            ));
        }
        v.push(item(
            code::FIT,
            t!("compare-zoom-fit"),
            Some("F"),
            Icon::Fit,
        ));
        v.push(item(
            code::ACTUAL,
            t!("gallery-palette-compare-actual"),
            Some("Ctrl+1"),
            Icon::ActualSize,
        ));
        v
    }

    /// One of compare's palette codes.
    pub(super) fn compare_code(&mut self, c: u64) {
        if self.gallery.compare.is_none() {
            return;
        }
        let cmp = &mut self.gallery.cmp;
        match c {
            code::SIDE => cmp.mode = CmpMode::Side,
            code::OVERLAY => cmp.mode = CmpMode::Overlay,
            c if (code::DIFF..code::DIFF + 3).contains(&c) => {
                cmp.mode = CmpMode::Diff;
                cmp.kind = DiffKind::ALL[(c - code::DIFF) as usize];
            }
            c if (code::DIFF_MORE..code::DIFF_MORE + 2).contains(&c) => {
                cmp.mode = CmpMode::Diff;
                cmp.kind = DiffKind::ALL[3 + (c - code::DIFF_MORE) as usize];
            }
            code::FLICKER => self.compare_flicker(),
            code::AUTO_FLICKER => self.compare_act(CAct::AutoFlicker),
            code::SPLIT => cmp.mode = CmpMode::Split,
            code::SPLIT_DIRECTION => self.compare_act(CAct::SplitDirection),
            code::ALIGN => self.compare_act(CAct::Align),
            code::SPACE => self.compare_act(CAct::Space),
            code::WHOLE => cmp.whole = !cmp.whole,
            code::PROBE => self.compare_act(CAct::Probe),
            code::LINK => self.compare_act(CAct::Link),
            code::FIT => {
                cmp.zoom = 1.0;
                cmp.at = (0.5, 0.5);
            }
            code::ACTUAL => {
                let z = cmp.z100;
                cmp.zoom_about(z, (0.5, 0.5));
            }
            _ => {}
        }
        self.gallery.dirty = true;
        self.redraw = true;
    }

    /// A screen reader's action on one of compare's nodes. True: it was one.
    pub(in crate::app) fn compare_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        use accesskit::{Action, ActionData};
        let target = req.target_node.0;
        if !self.gallery.active
            || self.gallery.compare.is_none()
            || !(A11Y_COMPARE..A11Y_COMPARE + A11Y_PANE + 8).contains(&target)
        {
            return false;
        }
        let i = target - A11Y_COMPARE;
        let Some(a) = i
            .checked_sub(A11Y_CONTROL)
            .and_then(|i| CONTROLS.get(i as usize).copied())
        else {
            return true;
        };
        let cmp = &mut self.gallery.cmp;
        match (a, req.action) {
            (CAct::Blend, Action::Increment | Action::Decrement) => {
                cmp.step_blend(req.action == Action::Increment);
            }
            (CAct::Blend, Action::SetValue) => {
                if let Some(ActionData::NumericValue(v)) = req.data {
                    cmp.blend = (v as f32 / 100.0).clamp(0.0, 1.0);
                }
            }
            (CAct::Divider, Action::Increment | Action::Decrement) => {
                cmp.split.step(req.action == Action::Increment);
            }
            (CAct::Divider, Action::SetValue) => {
                if let Some(ActionData::NumericValue(v)) = req.data {
                    cmp.split.at = (v as f32 / 100.0).clamp(0.0, 1.0);
                }
            }
            (CAct::Blend | CAct::Divider, _) => {}
            (a, Action::Click) => self.compare_act(a),
            _ => {}
        }
        self.redraw = true;
        true
    }

    /// `--show gallery-compare-…`: `-overlay` (the second image half over the first),
    /// `-diff` (the absolute error of the second and third against the first), `-split`
    /// and `-split-h` (the divider down and across), `-diff-signed`, `-probe` (the probe at
    /// the middle of the first image), `-metrics` (two images' metrics, the whole image),
    /// `-aligned` (scaled to A), and the benchmarks `-flicker-bench` (flicker → present
    /// latency), `-diff-bench` and `-ssim-bench` (metric time in view and of the whole
    /// image), which log their results. False when `rest` is not one of them.
    pub(super) fn compare_show(&mut self, rest: &str) -> bool {
        let (mode, count, bench) = match rest {
            "-compare-side" | "-compare-side-zoom" => (CmpMode::Side, 2, None),
            "-compare-overlay" | "-compare-overlay-zoom" => (CmpMode::Overlay, 3, None),
            "-compare-diff" => (CmpMode::Diff, 3, None),
            "-compare-split" | "-compare-split-h" => (CmpMode::Split, 2, None),
            "-compare-diff-signed" | "-compare-metrics" => (CmpMode::Diff, 2, None),
            "-compare-probe" => (CmpMode::Overlay, 2, None),
            "-compare-aligned" => (CmpMode::Diff, 2, None),
            "-compare-flicker-bench" => (CmpMode::Overlay, 2, Some(false)),
            "-compare-diff-bench" | "-compare-ssim-bench" => (CmpMode::Diff, 2, Some(true)),
            _ => return false,
        };
        let ga = &mut self.gallery;
        let p: Vec<PathBuf> = ga
            .tile_paths()
            .filter(|p| {
                ga.by_path
                    .get(p.as_path())
                    .is_some_and(|&i| ga.entries[i].kind == EntryKind::Image)
            })
            .take(count)
            .cloned()
            .collect();
        ga.selected = p.iter().cloned().collect();
        self.compare_start(p, mode);
        let cmp = &mut self.gallery.cmp;
        match rest {
            "-compare-split-h" => cmp.split.across = true,
            "-compare-diff-signed" => cmp.kind = DiffKind::Signed,
            "-compare-metrics" | "-compare-ssim-bench" => cmp.whole = true,
            "-compare-probe" => {
                cmp.probe.on = true;
                cmp.probe.show_at = true;
            }
            "-compare-aligned" => cmp.align = AlignMode::ScaleToA,
            // Three times fit: a small image's pane grows, a large one crops.
            "-compare-side-zoom" | "-compare-overlay-zoom" => cmp.zoom = 3.0,
            _ => {}
        }
        cmp.bench = bench.map(|metrics| Bench {
            metrics,
            since: Instant::now(),
            started: metrics,
            samples: Vec::new(),
            record: Vec::new(),
        });
        true
    }
}

/// How the difference view draws and measures.
#[derive(Clone, Copy, Debug)]
pub(super) struct DiffOptions {
    pub full_scale: f32,
    pub kind: DiffKind,
    pub channel: ChannelView,
    pub space: MetricSpace,
    /// The whole overlap rather than what shows of it.
    pub whole: bool,
}

/// The difference view's panes: each further image (`idx`: its pane) against the first,
/// placed in the frame (`places`) and compared over their overlap, and the metric pairs of
/// what shows of it (or of all of it).
pub(super) fn diff_panes(
    panes: &[QuickPane],
    idx: &[Option<usize>],
    sizes: &[Option<(f32, f32)>],
    places: &[Place],
    has_alpha: &[bool],
    o: &DiffOptions,
) -> (Vec<DiffPane>, Vec<(usize, MetricPair)>) {
    let mut diffs = Vec::new();
    let mut pairs = Vec::new();
    let (Some(Some(ia)), Some(Some(sa))) = (idx.first(), sizes.first()) else {
        return (diffs, pairs);
    };
    let (ia, sa) = (*ia, *sa);
    let place = |k: usize| places.get(k).copied().unwrap_or(Place::IDENTITY);
    let pa = place(0);
    let va = panes[ia].view();
    for (kb, ib) in idx.iter().enumerate().skip(1) {
        let (Some(ib), Some(Some(sb))) = (*ib, sizes.get(kb)) else {
            continue;
        };
        let ov = align::overlap(sa, &pa, *sb, &place(kb));
        let (rect, px) = panes[ib].placed();
        let (pw, ph) = (px.0 as f32, px.1 as f32);
        let overlap = ov.map_or([0.0; 4], |r| {
            let p0 = va.display_to_screen(pa.from_frame((r.0, r.1)));
            let p1 = va.display_to_screen(pa.from_frame((r.2, r.3)));
            [
                p0.0.clamp(0.0, pw),
                p0.1.clamp(0.0, ph),
                p1.0.clamp(0.0, pw),
                p1.1.clamp(0.0, ph),
            ]
        });
        let alpha = o.channel == ChannelView::Normal
            && (has_alpha.first() == Some(&true) || has_alpha.get(kb) == Some(&true));
        diffs.push(DiffPane {
            a: ia,
            b: ib,
            image: kb,
            rect,
            px,
            params: DiffParams {
                kind: o.kind,
                full_scale: o.full_scale,
                overlap,
                alpha,
                space: o.space,
            },
        });
        let region = ov.and_then(|r| {
            if o.whole {
                Some(r)
            } else {
                metric_region(panes[ia].visible_frame(), r)
            }
        });
        if let Some(region) = region {
            pairs.push((
                kb,
                MetricPair {
                    a: panes[ia].metric_image(),
                    b: panes[ib].metric_image(),
                    region,
                    channel: o.channel,
                    alpha,
                    space: o.space,
                },
            ));
        }
    }
    (diffs, pairs)
}

/// Compare's cells (whole cell, image area) in a window of `vw` × `vh`: `count` of them
/// above the toolbar, each with a caption of `lines` lines `cap` high.
pub(super) fn compare_layout(
    vw: f32,
    vh: f32,
    count: usize,
    cap: f32,
    lines: usize,
    text_scale: f32,
) -> Vec<(Rect, Rect)> {
    compare_cells(vw, vh - bar_height(text_scale), count, cap * lines as f32)
}

/// The toolbar's height at the gallery's text scale.
pub(super) fn bar_height(text_scale: f32) -> f32 {
    let g = if text_scale.is_finite() {
        text_scale.clamp(1.0, 3.0)
    } else {
        1.0
    };
    (48.0 * g).round()
}

impl Gallery {
    /// Compare is open.
    pub(in crate::app) fn cmp_open(&self) -> bool {
        self.compare.is_some()
    }

    /// How many images compare shows (0: closed).
    pub(in crate::app) fn compare_count(&self) -> usize {
        self.compare.as_ref().map_or(0, Vec::len)
    }

    /// The blend slider at `x` (logical px).
    fn compare_slider_to(&mut self, x: f32) {
        let Some(r) = self.cmp.slider else { return };
        let t = ((x - r.x) / r.w.max(1.0)).clamp(0.0, 1.0);
        self.cmp.blend = if self.rtl { 1.0 - t } else { t };
        self.cmp.toggled = Some(Instant::now());
    }

    /// The split's divider at the pointer `p` (logical px).
    fn compare_divider_to(&mut self, p: (f32, f32)) {
        let Some((_, r)) = self.cmp.panes.first().copied() else {
            return;
        };
        let rtl = self.rtl;
        self.cmp.split.drag_to(p, r, rtl);
    }

    /// Image `k`'s name: its listing entry's, else (an image from another folder) its
    /// file name.
    fn compare_name(&self, k: usize) -> String {
        let Some(p) = self.compare.as_ref().and_then(|c| c.get(k)) else {
            return String::new();
        };
        match self.by_path.get(p.as_path()) {
            Some(&i) => self.entries[i].name.clone(),
            None => super::file_name(p),
        }
    }

    /// How images `a` and `b` are lined up and how much of them is compared ("Aligned
    /// top-left, 4000 × 3000 compared"), when the images differ in size or are aligned
    /// automatically.
    fn aligned_note(&self, a: usize, b: usize) -> Option<String> {
        let s = &self.cmp.sizes;
        let (Some(Some(sa)), Some(Some(sb))) = (s.get(a), s.get(b)) else {
            return None;
        };
        let place = |k: usize| self.cmp.places.get(k).copied().unwrap_or(Place::IDENTITY);
        let (pa, pb) = (place(a), place(b));
        let (w, h) = align::overlap(*sa, &pa, *sb, &pb).map_or((0, 0), |o| {
            ((o.2 - o.0).round() as i64, (o.3 - o.1).round() as i64)
        });
        let differ = sa != sb;
        match self.cmp.align {
            AlignMode::TopLeft => differ.then(|| t!("compare-aligned", width = w, height = h)),
            AlignMode::Centre => {
                differ.then(|| t!("compare-aligned-centre", width = w, height = h))
            }
            AlignMode::ScaleToA => differ.then(|| {
                t!(
                    "compare-aligned-scaled",
                    scale = metrics::factor_text(pb.scale),
                    width = w,
                    height = h
                )
            }),
            AlignMode::Auto => Some(match self.cmp.auto.found(b) {
                Some(p) => t!(
                    "compare-aligned-auto",
                    angle = metrics::angle_text(p.rotation.to_degrees()),
                    scale = metrics::factor_text(p.scale),
                    width = w,
                    height = h
                ),
                None => t!("compare-aligning"),
            }),
        }
    }

    /// The metrics line of image `k` against the first: MAE, MSE, PSNR (and how it was
    /// sampled).
    fn metrics_line(&self, k: usize) -> String {
        match self.cmp.metrics.iter().find(|m| m.0 == k) {
            Some((_, m)) => metrics::error_line(m),
            None if self.cmp.measured() && self.cmp.comparer.is_some() => {
                t!("compare-no-overlap")
            }
            None => t!("compare-pending"),
        }
    }

    /// The second metrics line of image `k`: the region, SSIM and ΔE2000.
    fn quality_line(&self, k: usize) -> String {
        match self.cmp.metrics.iter().find(|m| m.0 == k) {
            Some((_, m)) => metrics::quality_line(m, self.cmp.whole, self.cmp.hdr),
            None => metrics::region_text(self.cmp.whole),
        }
    }

    /// The screen-reader text of pane `k` (as drawn).
    fn compare_pane_label(&self, k: usize) -> String {
        match self.cmp.mode {
            CmpMode::Side => self.compare_name(k),
            CmpMode::Split => t!(
                "compare-split-label",
                a = self.compare_name(0),
                b = self.compare_name(self.cmp.b),
                percent = self.cmp.split.percent() as i64
            ),
            CmpMode::Overlay => {
                let mut s = t!(
                    "compare-overlay-label",
                    a = self.compare_name(0),
                    b = self.compare_name(self.cmp.b),
                    percent = (self.cmp.blend * 100.0).round() as i64
                );
                if self.cmp.flicker.is_some() {
                    s.push_str(", ");
                    s.push_str(&t!(
                        "compare-showing-which",
                        which = self.shown_letter(),
                        name = self.compare_name(self.shown_image())
                    ));
                }
                s
            }
            CmpMode::Diff => {
                let mut s = t!(
                    "compare-vs",
                    b = self.compare_name(k),
                    a = self.compare_name(0)
                );
                s.push_str(", ");
                s.push_str(&kind_name(self.cmp.kind));
                if let Some(n) = self.aligned_note(0, k) {
                    s.push_str(", ");
                    s.push_str(&n);
                }
                for line in [self.metrics_line(k), self.quality_line(k)] {
                    s.push_str(", ");
                    s.push_str(&line.replace("  ·  ", ", "));
                }
                s
            }
        }
    }

    /// The image the overlay shows most of (0, or the second).
    fn shown_image(&self) -> usize {
        if self.cmp.blend > 0.5 { self.cmp.b } else { 0 }
    }

    fn shown_letter(&self) -> &'static str {
        if self.shown_image() == 0 { "A" } else { "B" }
    }

    /// A small "A" or "B" in the top start corner of `r`.
    fn compare_badge(&self, dl: &mut DrawList, text: &mut Text, pal: &Palette, r: Rect, s: &str) {
        let style = self.ts(SMALL);
        let w = text.measure(s, style) + 12.0;
        let h = self.u(20.0);
        let b = Rect::new(r.x + 8.0, r.y + 8.0, w, h);
        let b = if self.rtl {
            Rect::new(r.right() - 8.0 - w, b.y, w, h)
        } else {
            b
        };
        dl.rect(b, 4.0, pal.surface);
        dl.stroke_rect(b, 4.0, pal.separator, 1.0);
        text_in(text, dl, s, style, b, pal.text);
    }

    /// Compare over the grid: its panes, captions and toolbar.
    pub(super) fn draw_compare(&mut self, dl: &mut DrawList, text: &mut Text, f: &Frame) {
        let Some(paths) = self.compare.clone() else {
            return;
        };
        let pal = f.pal;
        let n = paths.len();
        if self.cmp.b == 0 || self.cmp.b >= n {
            self.cmp.b = 1;
        }
        dl.rect(Rect::new(0.0, 0.0, self.vw, self.vh), 0.0, pal.canvas);
        let cap = self.quick_caption_h();
        let mode = self.cmp.mode;
        let cells = compare_layout(
            self.vw,
            self.vh,
            self.cmp.cells_for(n),
            cap,
            self.cmp.caption_lines(),
            self.text_scale,
        );
        let style = self.ts(BODY);
        self.cmp.panes.clear();
        self.cmp.hits.clear();
        match mode {
            CmpMode::Side => {
                let focus = self.focus.clone();
                for (k, (p, (cell, area))) in paths.iter().zip(cells).enumerate() {
                    let r = self.compare_image(dl, p, area, pal, 1.0);
                    self.cmp.panes.push((k, r));
                    if focus.as_ref() == Some(p) {
                        dl.stroke_rect(cell.inset(-6.0), 10.0, pal.focus_ring, 2.0);
                    }
                    let s = match self.by_path.get(p.as_path()) {
                        Some(&i) => {
                            let e = self.entries[i].clone();
                            self.quick_caption(&e)
                        }
                        None => super::file_name(p),
                    };
                    let s = truncate(text, &s, style, cell.w);
                    let line = Rect::new(cell.x, cell.bottom() - cap - 4.0, cell.w, cap);
                    text_in(text, dl, &s, style, line, pal.text);
                }
            }
            CmpMode::Split | CmpMode::Overlay => {
                if let Some(&(cell, area)) = cells.first() {
                    let b = self.cmp.b;
                    let r = self.compare_image(dl, &paths[0], area, pal, 1.0);
                    if mode == CmpMode::Split {
                        let clip = self.cmp.split.clip(r, self.rtl);
                        dl.push_clip(clip);
                        self.compare_image(dl, &paths[b], area, pal, 1.0);
                        dl.pop_clip();
                        self.draw_divider(dl, pal, r);
                        // Badges at the start corner: A's of the whole pane, B's of its part.
                        self.compare_badge(dl, text, pal, r, "A");
                        self.compare_badge(dl, text, pal, clip, "B");
                    } else {
                        self.compare_image(dl, &paths[b], area, pal, self.cmp.blend);
                        if self.cmp.flicker.is_some()
                            || self.cmp.blend == 0.0
                            || self.cmp.blend == 1.0
                        {
                            let l = self.shown_letter();
                            self.compare_badge(dl, text, pal, r, l);
                        }
                    }
                    self.cmp.panes.push((0, r));
                    let mut parts = vec![self.compare_name(0), self.compare_name(b)];
                    parts.extend(self.aligned_note(0, b));
                    let s = specular_i18n::I18n::global().join(&parts, "  ·  ");
                    let s = truncate(text, &s, style, cell.w);
                    let line = Rect::new(cell.x, cell.bottom() - cap - 4.0, cell.w, cap);
                    text_in(text, dl, &s, style, line, pal.text);
                }
            }
            CmpMode::Diff => {
                for (k, (cell, area)) in (1..n).zip(cells) {
                    let r = match self.diff_tex(k) {
                        Some((tex, r)) => {
                            dl.shadow(r, 4.0, pal.shadow, 24.0, 6.0);
                            // Outside the overlap: no image to compare (zero error is black).
                            dl.rect(r, 4.0, pal.surface_raised);
                            dl.image(r, [0.0, 0.0, 1.0, 1.0], tex, 4.0);
                            r
                        }
                        // Until both images are rendered and the passes are built.
                        None => self.compare_image(dl, &paths[k], area, pal, 0.35),
                    };
                    self.cmp.panes.push((k, r));
                    let s = t!(
                        "compare-vs",
                        b = self.compare_name(k),
                        a = self.compare_name(0)
                    );
                    let lines = self.cmp.caption_lines();
                    let mut y = cell.bottom() - cap * lines as f32 - 4.0;
                    let mut line = |text: &mut Text, s: &str, color| {
                        let s = truncate(text, s, style, cell.w);
                        text_in(
                            text,
                            dl,
                            &s,
                            style,
                            Rect::new(cell.x, y, cell.w, cap),
                            color,
                        );
                        y += cap;
                    };
                    line(text, &s, pal.text);
                    if lines > 3 {
                        let note = self.aligned_note(0, k).unwrap_or_default();
                        line(text, &note, pal.text_muted);
                    }
                    line(text, &self.metrics_line(k), pal.text_muted);
                    line(text, &self.quality_line(k), pal.text_muted);
                }
            }
        }
        self.draw_compare_bar(dl, text, pal);
        if self.cmp.probe.on {
            self.draw_probe(dl, text, pal);
        }
        self.draw_compare_tip(dl, text, f);
    }

    /// The split's divider over pane `r`, with its handle; its grab strip is a control.
    fn draw_divider(&mut self, dl: &mut DrawList, pal: &Palette, r: Rect) {
        let m = self.cmp.split.line(r, self.rtl);
        let (line, strip, knob) = if self.cmp.split.across {
            (
                Rect::new(r.x, m - 1.0, r.w, 2.0),
                Rect::new(r.x, m - split::GRAB, r.w, split::GRAB * 2.0),
                (r.x + r.w * 0.5, m),
            )
        } else {
            (
                Rect::new(m - 1.0, r.y, 2.0, r.h),
                Rect::new(m - split::GRAB, r.y, split::GRAB * 2.0, r.h),
                (m, r.y + r.h * 0.5),
            )
        };
        dl.rect(line, 0.0, pal.text);
        dl.circle(knob, 9.0, pal.accent);
        dl.circle(knob, 4.0, pal.text);
        self.cmp.hits.push((strip, CAct::Divider));
    }

    /// The probe's panel in the bottom start corner, above the toolbar: A and B in two
    /// columns, then their ΔE2000.
    fn draw_probe(&mut self, dl: &mut DrawList, text: &mut Text, pal: &Palette) {
        let p = &self.cmp.probe;
        if p.px.iter().all(Option::is_none) {
            return;
        }
        let mut rows = specular_widgets::pixel_columns(p.px[0].as_ref(), p.px[1].as_ref());
        rows.insert(0, (String::new(), "A".to_owned(), "B".to_owned()));
        if let Some(de) = p.delta_e() {
            rows.push((
                t!("compare-probe-delta-e"),
                diff::metric_text(f64::from(de)),
                String::new(),
            ));
        }
        let style = self.ts(SMALL);
        let line = self.u(18.0);
        let label_w = rows
            .iter()
            .map(|r| text.measure(&r.0, style))
            .fold(0.0f32, f32::max);
        let col_w = rows
            .iter()
            .flat_map(|r| [text.measure(&r.1, style), text.measure(&r.2, style)])
            .fold(self.u(60.0), f32::max);
        let pad = 10.0;
        let w = pad * 2.0 + label_w + 12.0 + col_w * 2.0 + 12.0;
        let h = pad * 2.0 + line * rows.len() as f32;
        // The top start corner, clear of the captions and the toolbar.
        let bottom = 16.0 + h;
        let panel = self.flip(Rect::new(16.0, bottom - h, w, h));
        dl.shadow(panel, 6.0, pal.shadow, 16.0, 4.0);
        dl.rect(panel, 6.0, pal.surface);
        dl.stroke_rect(panel, 6.0, pal.separator, 1.0);
        for (i, (l, a, b)) in rows.iter().enumerate() {
            let y = bottom - h + pad + line * i as f32;
            let colour = if i == 0 { pal.text } else { pal.text_muted };
            let cells = [
                (16.0 + pad, label_w, l, pal.text_muted),
                (16.0 + pad + label_w + 12.0, col_w, a, colour),
                (16.0 + pad + label_w + 24.0 + col_w, col_w, b, colour),
            ];
            for (x, cw, s, c) in cells {
                let r = self.flip(Rect::new(x, y, cw, line));
                let s = truncate(text, s, style, cw);
                let tw = text.measure(&s, style);
                let tr = if self.rtl {
                    Rect::new(r.right() - tw, r.y, tw, r.h)
                } else {
                    Rect::new(r.x, r.y, tw, r.h)
                };
                text_in(text, dl, &s, style, tr, c);
            }
        }
    }

    /// The metric space's tooltip while the pointer is over its button: what is measured
    /// and how.
    fn draw_compare_tip(&mut self, dl: &mut DrawList, text: &mut Text, f: &Frame) {
        let Some(r) = self
            .cmp
            .hits
            .iter()
            .find(|h| h.1 == CAct::Space)
            .map(|h| h.0)
            .filter(|r| self.pointer.is_some_and(|p| r.contains(p)))
        else {
            return;
        };
        let tip = metrics::space_help(self.cmp.space);
        let bounds = Rect::new(0.0, 0.0, self.vw, self.vh);
        specular_widgets::draw_tooltip(dl, text, f.pal, &tip, r, bounds, f.text_scale);
    }

    /// One image of compare in `area` at `opacity`: its render (`quick.rs`), else its
    /// thumbnail. Returns where it went.
    fn compare_image(
        &mut self,
        dl: &mut DrawList,
        path: &std::path::Path,
        area: Rect,
        pal: &Palette,
        opacity: f32,
    ) -> Rect {
        dl.push_opacity(opacity);
        let r = match self.pane_tex(path) {
            Some((tex, r, _)) => {
                if opacity >= 1.0 {
                    dl.shadow(r, 4.0, pal.shadow, 24.0, 6.0);
                }
                dl.image(r, [0.0, 0.0, 1.0, 1.0], tex, 4.0);
                r
            }
            None => self.draw_big(dl, path, area, pal),
        };
        dl.pop_opacity();
        r
    }

    /// A toolbar button at `x` (left to right; mirrored in right-to-left layouts), `on`
    /// when it is the current choice. Returns its width.
    #[allow(clippy::too_many_arguments)]
    fn compare_button_draw(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        (x, y, h): (f32, f32, f32),
        label: &str,
        act: CAct,
        on: bool,
    ) -> f32 {
        let style = self.ts(BODY);
        let w = text.measure(label, style) + 16.0;
        let r = self.flip(Rect::new(x, y, w, h));
        let hover = self.pointer.is_some_and(|p| r.contains(p));
        let (fill, fg) = if on {
            (pal.accent_soft, pal.text)
        } else if hover {
            (pal.surface_hover, pal.text)
        } else {
            (pal.surface_raised, pal.text)
        };
        dl.rect(r, 6.0, fill);
        if on {
            dl.stroke_rect(r, 6.0, pal.accent, 1.0);
        }
        text_in(text, dl, label, style, r, fg);
        self.cmp.hits.push((r, act));
        w
    }

    /// A button if it fits before `end`: its width (and the gap after it), else 0.
    #[allow(clippy::too_many_arguments)]
    fn compare_button_fit(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        (x, y, h, end): (f32, f32, f32, f32),
        label: &str,
        act: CAct,
        on: bool,
    ) -> f32 {
        let w = text.measure(label, self.ts(BODY)) + 16.0;
        if x + w > end {
            return 0.0;
        }
        self.compare_button_draw(dl, text, pal, (x, y, h), label, act, on) + 8.0
    }

    /// Compare's toolbar: the modes, then the split's direction, the overlay's flicker and
    /// blend, or the difference view's kind, gain, colour ramp and metric space; the
    /// alignment, the link and the probe as room allows; the zoom at the end.
    fn draw_compare_bar(&mut self, dl: &mut DrawList, text: &mut Text, pal: &Palette) {
        self.cmp.slider = None;
        let bh = bar_height(self.text_scale);
        let bar = Rect::new(0.0, self.vh - bh, self.vw, bh);
        dl.rect(bar, 0.0, pal.surface);
        dl.rect(Rect::new(0.0, bar.y, self.vw, 1.0), 0.0, pal.separator);
        let h = self.u(30.0);
        let y = bar.y + ((bh - h) * 0.5).round();
        let row = (y, h);
        let mode = self.cmp.mode;
        let mut x = 16.0;
        for (act, label, on) in [
            (CAct::Side, t!("compare-side"), mode == CmpMode::Side),
            (CAct::Split, t!("compare-split"), mode == CmpMode::Split),
            (
                CAct::Overlay,
                t!("compare-overlay"),
                mode == CmpMode::Overlay,
            ),
            (CAct::Diff, t!("compare-difference"), mode == CmpMode::Diff),
        ] {
            x += self.compare_button_draw(dl, text, pal, (x, row.0, row.1), &label, act, on) + 4.0;
        }
        x += 12.0;
        // The zoom, at the trailing end.
        let zoom = if (self.cmp.zoom - 1.0).abs() < 1e-3 {
            t!("compare-zoom-fit")
        } else {
            t!(
                "compare-zoom",
                percent = self.cmp.zoom_percent().round() as i64
            )
        };
        let zw = text.measure(&zoom, self.ts(BODY)) + 16.0;
        let zx = self.vw - 16.0 - zw;
        self.compare_button_draw(dl, text, pal, (zx, row.0, row.1), &zoom, CAct::Zoom, false);
        let end = zx - 12.0;
        let small = self.ts(SMALL);
        let body = self.ts(BODY);
        let n = self.compare.as_ref().map_or(0, Vec::len);
        match mode {
            CmpMode::Side => {
                if n == 2 {
                    let label = t!("compare-link");
                    let on = self.cmp.link;
                    x += self.compare_button_fit(
                        dl,
                        text,
                        pal,
                        (x, y, h, end),
                        &label,
                        CAct::Link,
                        on,
                    );
                }
            }
            CmpMode::Split => {
                let label = if self.cmp.split.across {
                    t!("compare-split-across")
                } else {
                    t!("compare-split-down")
                };
                x += self.compare_button_fit(
                    dl,
                    text,
                    pal,
                    (x, y, h, end),
                    &label,
                    CAct::SplitDirection,
                    false,
                ) + 4.0;
            }
            CmpMode::Overlay => {
                let label = t!("compare-flicker");
                x += self.compare_button_draw(
                    dl,
                    text,
                    pal,
                    (x, row.0, row.1),
                    &label,
                    CAct::Flicker,
                    false,
                ) + 4.0;
                let label = t!("compare-auto-flicker");
                let on = self.cmp.flicker.is_some();
                x += self.compare_button_fit(
                    dl,
                    text,
                    pal,
                    (x, y, h, end),
                    &label,
                    CAct::AutoFlicker,
                    on,
                ) + 8.0;
                if let Some(fl) = self.cmp.flicker {
                    let s = t!(
                        "compare-flicker-rate",
                        rate = diff::metric_text(f64::from(fl.rate))
                    );
                    let sw = text.measure(&s, small);
                    if x + sw <= end {
                        let r = self.flip(Rect::new(x, y, sw, h));
                        text_in(text, dl, &s, small, r, pal.text_muted);
                        x += sw + 12.0;
                    }
                } else {
                    let label = t!("compare-blend");
                    let lw = text.measure(&label, body);
                    let track_w = self.u(140.0);
                    let loc = specular_i18n::I18n::global().localizer();
                    let pct = loc.percent(f64::from(self.cmp.blend), 0);
                    let pw = text.measure(&loc.percent(1.0, 0), body);
                    if x + lw + 12.0 + track_w + 12.0 + pw <= end {
                        let r = self.flip(Rect::new(x, y, lw, h));
                        text_in(text, dl, &label, body, r, pal.text_muted);
                        x += lw + 12.0;
                        let line = Rect::new(x, y + h * 0.5 - 2.0, track_w, 4.0);
                        let track = self.flip(line);
                        dl.rect(track, 2.0, pal.surface_raised);
                        let v = self.cmp.blend;
                        let fill = self.flip(Rect::new(line.x, line.y, line.w * v, line.h));
                        dl.rect(fill, 2.0, pal.accent);
                        let knob = if self.rtl {
                            track.right() - track.w * v
                        } else {
                            track.x + track.w * v
                        };
                        dl.circle((knob, track.y + 2.0), 8.0, pal.accent);
                        self.cmp.slider = Some(track);
                        self.cmp
                            .hits
                            .push((Rect::new(track.x - 8.0, y, track.w + 16.0, h), CAct::Blend));
                        x += track_w + 12.0;
                        let r = self.flip(Rect::new(x, y, pw, h));
                        text_in(text, dl, &pct, body, r, pal.text);
                        x += pw + 12.0;
                    }
                }
            }
            CmpMode::Diff => {
                let label = kind_name(self.cmp.kind);
                x += self.compare_button_draw(
                    dl,
                    text,
                    pal,
                    (x, row.0, row.1),
                    &label,
                    CAct::Kind,
                    false,
                ) + 12.0;
                x += self.compare_button_draw(
                    dl,
                    text,
                    pal,
                    (x, row.0, row.1),
                    "−",
                    CAct::ScaleDown,
                    false,
                ) + 4.0;
                // The ramp between the gain buttons, from no error to the full scale (the
                // signed error's diverging map, darker to lighter).
                let ramp_w = self.u(64.0);
                let steps = 16;
                let rh = self.u(10.0);
                let ry = y + (h - rh) * 0.5;
                let signed = self.cmp.kind == DiffKind::Signed;
                for i in 0..steps {
                    let v = (i as f32 + 0.5) / steps as f32;
                    let c = if signed {
                        specular_inspect::ColourMap::Vik.sample(v)
                    } else {
                        diff::false_colour(v)
                    };
                    let seg = Rect::new(
                        x + ramp_w * i as f32 / steps as f32,
                        ry,
                        ramp_w / steps as f32 + 0.5,
                        rh,
                    );
                    dl.rect(self.flip(seg), 0.0, Color::rgba(c[0], c[1], c[2], 1.0));
                }
                let ramp = self.flip(Rect::new(x, ry, ramp_w, rh));
                dl.stroke_rect(ramp, 0.0, pal.separator, 1.0);
                x += ramp_w + 4.0;
                x += self.compare_button_draw(
                    dl,
                    text,
                    pal,
                    (x, row.0, row.1),
                    "+",
                    CAct::ScaleUp,
                    false,
                ) + 12.0;
                // The gain, then the metric space.
                let gain = t!(
                    "compare-gain",
                    gain = metrics::gain_text(self.cmp.kind.gain(self.cmp.k))
                );
                let gw = text.measure(&gain, small);
                if x + gw <= end {
                    let r = self.flip(Rect::new(x, y, gw, h));
                    text_in(text, dl, &gain, small, r, pal.text_muted);
                    x += gw + 12.0;
                }
                let label = space_name(self.cmp.space);
                x += self.compare_button_fit(
                    dl,
                    text,
                    pal,
                    (x, y, h, end),
                    &label,
                    CAct::Space,
                    false,
                );
            }
        }
        if mode != CmpMode::Side {
            let label = align_name(self.cmp.align);
            x += self.compare_button_fit(dl, text, pal, (x, y, h, end), &label, CAct::Align, false);
        }
        let label = t!("compare-probe");
        let on = self.cmp.probe.on;
        self.compare_button_fit(dl, text, pal, (x, y, h, end), &label, CAct::Probe, on);
    }

    /// Compare's screen-reader nodes: a group of its controls and panes.
    pub(super) fn compare_a11y(
        &self,
        tree: &mut accesskit::TreeUpdate,
        bounds: &dyn Fn(Rect) -> accesskit::Rect,
    ) {
        use accesskit::{Action, Node, NodeId, Orientation, Role, Toggled};
        if self.compare.is_none() {
            return;
        }
        let mode = self.cmp.mode;
        let mut group = Node::new(Role::Group);
        group.set_label(t!("compare-a11y-group"));
        let mut children = Vec::new();
        let mut nodes = Vec::new();
        for (i, a) in CONTROLS.iter().enumerate() {
            let Some(r) = self.cmp.hits.iter().find(|h| h.1 == *a).map(|h| h.0) else {
                continue;
            };
            let id = NodeId(A11Y_COMPARE + A11Y_CONTROL + i as u64);
            let mut n = Node::new(Role::Button);
            n.set_bounds(bounds(r));
            let label = match a {
                CAct::Side => t!("compare-side"),
                CAct::Split => t!("compare-split"),
                CAct::Overlay => t!("compare-overlay"),
                CAct::Diff => t!("compare-difference"),
                CAct::Flicker => t!("compare-flicker"),
                CAct::AutoFlicker => t!("compare-auto-flicker"),
                CAct::Blend => t!("compare-a11y-blend"),
                CAct::Divider => t!("compare-a11y-divider"),
                CAct::SplitDirection => t!("palette-compare-split-direction"),
                CAct::Kind => kind_name(self.cmp.kind),
                CAct::ScaleDown => t!("compare-scale-down"),
                CAct::ScaleUp => t!("compare-scale-up"),
                CAct::Space => t!("compare-a11y-space"),
                CAct::Align => t!("compare-a11y-align"),
                CAct::Link => t!("compare-link"),
                CAct::Probe => t!("compare-probe"),
                CAct::Zoom => t!("compare-a11y-zoom"),
            };
            match a {
                CAct::Side | CAct::Split | CAct::Overlay | CAct::Diff => {
                    let on = matches!(
                        (a, mode),
                        (CAct::Side, CmpMode::Side)
                            | (CAct::Split, CmpMode::Split)
                            | (CAct::Overlay, CmpMode::Overlay)
                            | (CAct::Diff, CmpMode::Diff)
                    );
                    n.set_toggled(Toggled::from(on));
                }
                CAct::AutoFlicker => n.set_toggled(Toggled::from(self.cmp.flicker.is_some())),
                CAct::Link => n.set_toggled(Toggled::from(self.cmp.link)),
                CAct::Probe => n.set_toggled(Toggled::from(self.cmp.probe.on)),
                CAct::Space => {
                    n.set_value(space_name(self.cmp.space));
                    n.set_description(metrics::space_help(self.cmp.space));
                }
                CAct::Align => n.set_value(align_name(self.cmp.align)),
                CAct::SplitDirection => n.set_value(if self.cmp.split.across {
                    t!("compare-split-across")
                } else {
                    t!("compare-split-down")
                }),
                CAct::Zoom => {
                    let z = if (self.cmp.zoom - 1.0).abs() < 1e-3 {
                        t!("compare-zoom-fit")
                    } else {
                        t!(
                            "compare-zoom",
                            percent = self.cmp.zoom_percent().round() as i64
                        )
                    };
                    n.set_value(z);
                }
                CAct::ScaleDown | CAct::ScaleUp => {
                    n.set_value(t!(
                        "compare-gain",
                        gain = metrics::gain_text(self.cmp.kind.gain(self.cmp.k))
                    ));
                }
                _ => {}
            }
            if let Some(k) = act_key(*a)
                && *a != CAct::Space
            {
                n.set_keyboard_shortcut(k);
                n.set_description(format!("{label} ({k})"));
            } else if let Some(k) = act_key(*a) {
                n.set_keyboard_shortcut(k);
            }
            n.set_label(label);
            match a {
                CAct::Blend => {
                    n.set_role(Role::Slider);
                    n.set_orientation(Orientation::Horizontal);
                    n.set_numeric_value(f64::from((self.cmp.blend * 100.0).round()));
                    n.set_min_numeric_value(0.0);
                    n.set_max_numeric_value(100.0);
                    n.set_numeric_value_step(f64::from(BLEND_STEP * 100.0).round());
                    n.add_action(Action::Increment);
                    n.add_action(Action::Decrement);
                    n.add_action(Action::SetValue);
                }
                CAct::Divider => {
                    n.set_role(Role::Slider);
                    n.set_orientation(if self.cmp.split.across {
                        Orientation::Vertical
                    } else {
                        Orientation::Horizontal
                    });
                    n.set_numeric_value(self.cmp.split.percent());
                    n.set_min_numeric_value(0.0);
                    n.set_max_numeric_value(100.0);
                    n.set_numeric_value_step(f64::from(split::STEP * 100.0).round());
                    n.add_action(Action::Increment);
                    n.add_action(Action::Decrement);
                    n.add_action(Action::SetValue);
                }
                _ => n.add_action(Action::Click),
            }
            children.push(id);
            nodes.push((id, n));
        }
        // The space the difference view measures in.
        if mode == CmpMode::Diff {
            let id = NodeId(A11Y_COMPARE + A11Y_PANE - 1);
            let mut n = Node::new(Role::Label);
            n.set_value(metrics::space_help(self.cmp.space));
            children.push(id);
            nodes.push((id, n));
        }
        // The probe's readout.
        if self.cmp.probe.on && self.cmp.probe.px.iter().any(Option::is_some) {
            let id = NodeId(A11Y_COMPARE + A11Y_PANE - 2);
            let mut n = Node::new(Role::Label);
            let p = &self.cmp.probe;
            let side = |k: usize| {
                p.px[k]
                    .as_ref()
                    .map_or_else(|| "–".to_owned(), specular_widgets::pixel_text)
            };
            n.set_label(t!("compare-probe"));
            n.set_value(t!(
                "compare-probe-a11y",
                a = side(0),
                b = side(1),
                delta = p
                    .delta_e()
                    .map_or_else(|| "–".to_owned(), |d| diff::metric_text(f64::from(d)))
            ));
            children.push(id);
            nodes.push((id, n));
        }
        for (k, r) in &self.cmp.panes {
            let id = NodeId(A11Y_COMPARE + A11Y_PANE + *k as u64);
            let mut n = Node::new(Role::Image);
            n.set_label(self.compare_pane_label(*k));
            n.set_bounds(bounds(*r));
            children.push(id);
            nodes.push((id, n));
        }
        group.set_children(children);
        if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
            root.push_child(NodeId(A11Y_COMPARE));
        }
        tree.nodes.push((NodeId(A11Y_COMPARE), group));
        tree.nodes.extend(nodes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_keeps_the_point_under_the_pointer() {
        let mut c = CompareState {
            z100: 4.0,
            ..CompareState::default()
        };
        let u = (0.8, 0.3);
        let before = (c.at.0 + (u.0 - 0.5) / c.zoom, c.at.1 + (u.1 - 0.5) / c.zoom);
        c.zoom_about(2.0, u);
        let after = (c.at.0 + (u.0 - 0.5) / c.zoom, c.at.1 + (u.1 - 0.5) / c.zoom);
        assert!((before.0 - after.0).abs() < 1e-5 && (before.1 - after.1).abs() < 1e-5);
        // Never below fit nor past 8× 100 %; the centre keeps the image over the pane.
        c.zoom_about(0.1, u);
        assert_eq!((c.zoom, c.at), (1.0, (0.5, 0.5)));
        c.zoom_about(1000.0, (1.0, 1.0));
        assert_eq!(c.zoom, 32.0);
        assert!(c.at.0 <= 1.0 - 0.5 / 32.0 + 1e-6);
    }

    #[test]
    fn panning_follows_the_pointer_and_stops_at_the_edges() {
        let mut c = CompareState {
            zoom: 2.0,
            ..CompareState::default()
        };
        let r = Rect::new(0.0, 0.0, 400.0, 300.0);
        c.pan((-40.0, 0.0), r);
        assert!((c.at.0 - 0.55).abs() < 1e-5, "{:?}", c.at);
        c.pan((-4000.0, -4000.0), r);
        assert_eq!(c.at, (0.75, 0.75));
        // At fit there is nothing to pan.
        c.zoom = 1.0;
        c.pan((100.0, 100.0), r);
        assert_eq!(clamp_at(c.at, 1.0), (0.5, 0.5));
    }

    #[test]
    fn fit_and_actual_size_toggle() {
        let mut c = CompareState {
            z100: 3.0,
            ..CompareState::default()
        };
        c.toggle_actual();
        assert_eq!(c.zoom, 3.0);
        assert!((c.zoom_percent() - 100.0).abs() < 1e-3);
        c.toggle_actual();
        assert_eq!((c.zoom, c.at), (1.0, (0.5, 0.5)));
        // An image smaller than its cell is at 100 % or more at fit already.
        let mut c = CompareState::default();
        c.toggle_actual();
        assert_eq!(c.zoom, 1.0);
    }

    /// A small image (its pane grows four times before it fills its cell): zooming in
    /// grows the pane with nothing to pan, then crops it and pans as before.
    #[test]
    fn small_images_grow_before_they_pan() {
        assert_eq!(crop_of(1.0, 4.0), 1.0);
        assert_eq!(crop_of(3.0, 4.0), 1.0);
        assert_eq!(crop_of(8.0, 4.0), 2.0);
        assert_eq!(crop_of(8.0, 1.0), 8.0);
        let mut c = CompareState {
            grow: 4.0,
            ..CompareState::default()
        };
        // At 100 % already: fit, and no 100 % step.
        c.toggle_actual();
        assert_eq!(c.zoom, 1.0);
        c.zoom_about(3.0, (0.9, 0.9));
        assert_eq!((c.zoom, c.at), (3.0, (0.5, 0.5)));
        let r = Rect::new(0.0, 0.0, 600.0, 300.0);
        c.pan((-100.0, -100.0), r);
        assert_eq!(c.at, (0.5, 0.5));
        // Twice what fills the cell: half of it shows, and a drag moves it.
        c.zoom_about(8.0, (0.5, 0.5));
        c.pan((-60.0, 0.0), r);
        assert!((c.at.0 - 0.55).abs() < 1e-5, "{:?}", c.at);
        c.pan((-4000.0, -4000.0), r);
        assert_eq!(c.at, (0.75, 0.75));
        c.zoom_about(1.0, (0.5, 0.5));
        assert_eq!((c.zoom, c.at), (1.0, (0.5, 0.5)));
    }

    #[test]
    fn blend_and_scale_steps_stay_in_range() {
        let mut c = CompareState::default();
        for _ in 0..20 {
            c.step_blend(true);
        }
        assert_eq!(c.blend, 1.0);
        c.step_blend(false);
        assert!((c.blend - 0.9).abs() < 1e-6);
        for _ in 0..40 {
            c.step_scale(true);
        }
        assert_eq!(c.k, diff::SCALE_RANGE.1);
        for _ in 0..40 {
            c.step_scale(false);
        }
        assert_eq!(c.k, diff::SCALE_RANGE.0);
    }

    #[test]
    fn different_sizes_align_top_left_over_their_overlap() {
        let sizes = [Some((4000.0, 3000.0)), Some((3000.0, 4000.0)), None];
        assert_eq!(frame_of(&sizes), (4000.0, 4000.0));
        assert_eq!(frame_of(&[None]), (1.0, 1.0));
        let o = overlap((4000.0, 3000.0), (3000.0, 4000.0));
        assert_eq!(o, (3000.0, 3000.0));
        // The region is what shows of the first image, within the overlap.
        let o = (0.0, 0.0, o.0, o.1);
        assert_eq!(
            metric_region((0.0, 0.0, 4000.0, 3000.0), o),
            Some((0.0, 0.0, 3000.0, 3000.0))
        );
        assert_eq!(
            metric_region((1000.0, 500.0, 2000.0, 1500.0), o),
            Some((1000.0, 500.0, 2000.0, 1500.0))
        );
        // Zoomed into the part only one image covers: nothing to measure.
        assert_eq!(metric_region((3200.0, 0.0, 4000.0, 1000.0), o), None);
        // An overlap that does not start at the origin (centred, scaled to A).
        assert_eq!(
            metric_region((0.0, 0.0, 400.0, 300.0), (50.0, 0.0, 350.0, 300.0)),
            Some((50.0, 0.0, 350.0, 300.0))
        );
    }

    #[test]
    fn the_modes_render_what_they_show() {
        let paths: Vec<PathBuf> = ["a", "b", "c"].iter().map(PathBuf::from).collect();
        let mut c = CompareState::default();
        // Side by side at fit in the normal view: the thumbnails.
        assert!(!c.renders(true));
        assert!(c.renders(false));
        assert_eq!(c.cells_for(3), 3);
        c.mode = CmpMode::Overlay;
        c.b = 2;
        assert!(c.renders(true));
        assert_eq!(c.rendered(&paths), vec![paths[0].clone(), paths[2].clone()]);
        assert_eq!(c.cells_for(3), 1);
        assert!(c.measured());
        c.mode = CmpMode::Diff;
        assert_eq!(c.rendered(&paths), paths);
        assert_eq!(c.cells_for(3), 2);
        // No passes yet: not measured.
        assert!(!c.measured());
        // The split renders the two it shows, in one cell.
        c.mode = CmpMode::Split;
        assert_eq!(c.rendered(&paths), vec![paths[0].clone(), paths[2].clone()]);
        assert_eq!(c.cells_for(3), 1);
        // The probe needs the renders even side by side at fit.
        c.mode = CmpMode::Side;
        c.probe.on = true;
        assert!(c.renders(true));
        // Reset keeps nothing but the GPU passes, and takes the space it is given.
        c.zoom = 3.0;
        c.reset(MetricSpace::Srgb);
        assert_eq!((c.mode, c.zoom, c.b), (CmpMode::Side, 1.0, 1));
        assert_eq!(c.space, MetricSpace::Srgb);
        assert!(!c.probe.on && c.link && c.flicker.is_none());
    }

    #[test]
    fn auto_flicker_never_passes_its_limit() {
        let now = Instant::now();
        let mut c = CompareState::default();
        // Starts at 2 a second, entering the overlay.
        c.toggle_auto_flicker(3.0, now);
        assert_eq!(c.mode, CmpMode::Overlay);
        assert_eq!(c.flicker.unwrap().rate, FLICKER_START);
        for _ in 0..20 {
            c.step_flicker(true, 3.0);
        }
        assert_eq!(c.flicker.unwrap().rate, 3.0);
        // A raised limit allows more, up to 10; a lowered one starts lower.
        for _ in 0..40 {
            c.step_flicker(true, 25.0);
        }
        assert_eq!(c.flicker.unwrap().rate, 10.0);
        for _ in 0..40 {
            c.step_flicker(false, 3.0);
        }
        assert_eq!(c.flicker.unwrap().rate, 1.0);
        c.toggle_auto_flicker(3.0, now);
        assert!(c.flicker.is_none());
        c.toggle_auto_flicker(1.5, now);
        assert_eq!(c.flicker.unwrap().rate, 1.5);
        assert_eq!(flicker_limit(f32::NAN), 3.0);
        assert_eq!(flicker_limit(0.0), 1.0);
    }

    #[test]
    fn unlinked_panes_pan_on_their_own() {
        let mut c = CompareState {
            zoom: 2.0,
            ..CompareState::default()
        };
        assert!(!c.unlinked(2));
        c.toggle_link();
        assert!(c.unlinked(2) && !c.unlinked(3));
        let r = Rect::new(0.0, 0.0, 400.0, 300.0);
        c.pan_pane(1, (-40.0, 0.0), r);
        assert_eq!(c.ats[0], (0.5, 0.5));
        assert!((c.ats[1].0 - 0.55).abs() < 1e-5);
        // Linked again: both from the shared view.
        c.toggle_link();
        assert!(c.link && c.ats[1] == c.at);
        // The modes parse from the command line.
        assert_eq!(CmpMode::from_id("split"), Some(CmpMode::Split));
        assert_eq!(CmpMode::from_id("diff"), Some(CmpMode::Diff));
        assert_eq!(CmpMode::from_id("wipe"), None);
    }

    #[test]
    fn the_layout_leaves_room_for_the_toolbar() {
        let bh = bar_height(1.0);
        for (mode, count) in [
            (CmpMode::Side, 4),
            (CmpMode::Overlay, 1),
            (CmpMode::Diff, 3),
        ] {
            let c = CompareState {
                mode,
                ..CompareState::default()
            };
            let cells = compare_layout(1280.0, 800.0, count, 30.0, c.caption_lines(), 1.0);
            assert_eq!(cells.len(), count);
            for (cell, area) in &cells {
                assert!(cell.bottom() <= 800.0 - bh + 1e-3, "{mode:?}");
                assert!(area.bottom() < cell.bottom());
            }
        }
        // Three caption lines in the difference view: names, errors, SSIM and ΔE.
        let side = compare_layout(1280.0, 800.0, 2, 30.0, 1, 1.0);
        let diff = compare_layout(1280.0, 800.0, 2, 30.0, 3, 1.0);
        assert!((side[0].1.h - diff[0].1.h - 60.0).abs() < 1e-3);
        assert!(bar_height(2.0) > bh);
        // A fourth line, on the alignment, once the images differ in size or are aligned
        // automatically.
        let mut c = CompareState {
            mode: CmpMode::Diff,
            sizes: vec![Some((4000.0, 3000.0)), Some((4000.0, 3000.0)), None],
            ..CompareState::default()
        };
        assert_eq!(c.caption_lines(), 3);
        c.align = AlignMode::Auto;
        assert_eq!(c.caption_lines(), 4);
        c.align = AlignMode::TopLeft;
        c.sizes[2] = Some((3000.0, 2000.0));
        assert_eq!(c.caption_lines(), 4);
        c.mode = CmpMode::Overlay;
        assert_eq!(c.caption_lines(), 1);
    }

    #[test]
    fn diff_compares_the_plain_image_under_backdrop_views() {
        assert_eq!(diff_channel(ChannelView::AlphaChecker), ChannelView::Normal);
        assert_eq!(diff_channel(ChannelView::AlphaMatte), ChannelView::Normal);
        assert_eq!(diff_channel(ChannelView::Alpha), ChannelView::Alpha);
        assert_eq!(diff_channel(ChannelView::Red), ChannelView::Red);
    }
}
