//! The view model: where the image sits in the window.
//!
//! Zoom is animated in log space (so 100→200 % feels like 400→800 %) around an anchor that
//! stays under the cursor. Panning follows the pointer exactly, glides with momentum after a
//! flick, rubber-bands past the edges while dragging and springs back on release. EXIF
//! orientation and user rotation are part of the transform; pixels are never rotated.
//!
//! All coordinates are physical pixels. "Stored" coordinates are pixels of the image as it
//! is stored in the file; "display" coordinates are after orientation.

use specular_core::Orientation;
use specular_theme::{Spring, SpringParams};

/// A rectangle in physical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PxRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl PxRect {
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.w * 0.5, self.y + self.h * 0.5)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoomMode {
    /// Whole image visible; follows window size. Never enlarges past 100 % unless
    /// `upscale` is set.
    Fit,
    /// Fill the window (cropping).
    Fill,
    /// The image's width fills the window's (a tall page shows its top); follows the
    /// window size.
    FitWidth,
    /// The image's height fills the window's (a wide page shows its start).
    FitHeight,
    /// The user zoomed: keep the scale.
    Free,
}

#[derive(Clone, Copy, Debug)]
struct Anchor {
    display: (f32, f32),
    screen: (f32, f32),
}

/// 2×3 affine: x' = a·x + b·y + c, y' = d·x + e·y + f.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    pub m: [f32; 6],
}

impl Affine {
    pub fn apply(&self, p: (f32, f32)) -> (f32, f32) {
        let m = &self.m;
        (
            m[0] * p.0 + m[1] * p.1 + m[2],
            m[3] * p.0 + m[4] * p.1 + m[5],
        )
    }
    pub fn then(&self, o: &Affine) -> Affine {
        // o ∘ self
        let a = &self.m;
        let b = &o.m;
        Affine {
            m: [
                b[0] * a[0] + b[1] * a[3],
                b[0] * a[1] + b[1] * a[4],
                b[0] * a[2] + b[1] * a[5] + b[2],
                b[3] * a[0] + b[4] * a[3],
                b[3] * a[1] + b[4] * a[4],
                b[3] * a[2] + b[4] * a[5] + b[5],
            ],
        }
    }
    pub fn invert(&self) -> Affine {
        let m = &self.m;
        let det = m[0] * m[4] - m[1] * m[3];
        let id = if det.abs() < 1e-12 { 0.0 } else { 1.0 / det };
        let a = m[4] * id;
        let b = -m[1] * id;
        let d = -m[3] * id;
        let e = m[0] * id;
        Affine {
            m: [a, b, -(a * m[2] + b * m[5]), d, e, -(d * m[2] + e * m[5])],
        }
    }
}

/// Display coordinates from stored coordinates for an orientation and stored size.
pub fn display_from_stored(o: Orientation, w: f32, h: f32) -> Affine {
    let (q, flip) = o.to_rotation_flip();
    let mut t = if flip {
        Affine {
            m: [-1.0, 0.0, w, 0.0, 1.0, 0.0],
        }
    } else {
        Affine {
            m: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        }
    };
    let (mut cw, mut ch) = (w, h);
    for _ in 0..q {
        // Quarter turn clockwise of a cw×ch image: (x, y) → (ch − y, x).
        t = t.then(&Affine {
            m: [0.0, -1.0, ch, 1.0, 0.0, 0.0],
        });
        std::mem::swap(&mut cw, &mut ch);
    }
    t
}

const MIN_SCALE: f32 = 1.0 / 64.0;
const MAX_SCALE: f32 = 64.0;
/// The deepest zoom a drawing allows ([`View::set_max_scale`]): its stored pixels are a
/// CPU picture, the GPU draws the vectors themselves at any zoom.
pub const DRAWING_MAX_SCALE: f32 = 512.0;
/// How close (relative) a continuous zoom must end to fit or 100 % to snap onto it.
pub const SOFT_SNAP: f32 = 0.04;

#[derive(Debug)]
pub struct View {
    stored: (u32, u32),
    orient: Orientation,
    pub viewport: PxRect,
    zoom: Spring,
    cx: Spring,
    cy: Spring,
    anchor: Option<Anchor>,
    pub mode: ZoomMode,
    /// Fit may enlarge small images.
    pub upscale_small: bool,
    /// Fitting the height of a wide image starts at its right edge (right-to-left reading).
    pub start_right: bool,
    drag: Option<DragState>,
    velocity: (f32, f32),
    momentum: bool,
    /// A stored pixel's height over its width as shown (a DICOM Presentation Pixel Aspect
    /// Ratio); display space is stored space with rows stretched by it before orienting.
    aspect: f32,
    /// The deepest zoom (screen pixels per stored pixel).
    max_scale: f32,
    /// A turn of display space on screen about the viewport's centre (radians, clockwise on
    /// screen): compare's alignment of one image onto another ([`View::with_rotation`]).
    /// Zero everywhere else.
    rotation: f32,
    /// Bumped whenever anything that changes the rendered image changes.
    pub revision: u64,
}

#[derive(Clone, Copy, Debug)]
struct DragState {
    /// Unclamped centre the pointer asks for.
    raw: (f32, f32),
    last_t: f64,
}

fn rubber(over: f32, dim: f32) -> f32 {
    // iOS-style: approaches `dim`·0.55 asymptotically.
    let c = 0.55;
    let s = over.signum();
    let o = over.abs();
    s * (1.0 - 1.0 / (o * c / dim.max(1.0) + 1.0)) * dim
}

impl Default for View {
    fn default() -> Self {
        View::new()
    }
}

impl View {
    pub fn new() -> View {
        View {
            stored: (1, 1),
            orient: Orientation::NORMAL,
            viewport: PxRect {
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 1.0,
            },
            zoom: Spring::new(0.0, SpringParams::VIEW),
            cx: Spring::new(0.0, SpringParams::VIEW),
            cy: Spring::new(0.0, SpringParams::VIEW),
            anchor: None,
            mode: ZoomMode::Fit,
            upscale_small: false,
            start_right: false,
            drag: None,
            velocity: (0.0, 0.0),
            momentum: false,
            aspect: 1.0,
            max_scale: MAX_SCALE,
            rotation: 0.0,
            revision: 0,
        }
    }

    /// Display coordinates from stored coordinates (rows stretched by the pixel aspect,
    /// then oriented).
    fn display_from_stored(&self) -> Affine {
        let (w, h) = (self.stored.0 as f32, self.stored.1 as f32 * self.aspect);
        Affine {
            m: [1.0, 0.0, 0.0, 0.0, self.aspect, 0.0],
        }
        .then(&display_from_stored(self.orient, w, h))
    }

    /// Lets zoom go up to `max` screen pixels per stored pixel (`None`: the usual 64×), for
    /// content drawn at the exact zoom rather than from stored pixels (drawings).
    pub fn set_max_scale(&mut self, max: Option<f32>) {
        let max = max
            .filter(|m| m.is_finite() && *m >= 1.0)
            .unwrap_or(MAX_SCALE);
        if max != self.max_scale {
            self.max_scale = max;
            if self.scale() > max {
                self.zoom_to(max, None);
            }
        }
    }

    /// The deepest zoom allowed now.
    pub fn max_scale(&self) -> f32 {
        self.max_scale
    }

    /// A stored pixel's height over its width as shown (1: square).
    pub fn pixel_aspect(&self) -> f32 {
        self.aspect
    }

    /// Shows stored pixels `aspect` times as tall as wide (a DICOM Presentation Pixel Aspect
    /// Ratio; 1 for square). Keeps the zoom mode; [`View::set_image`] resets it to 1.
    pub fn set_pixel_aspect(&mut self, aspect: f32) {
        let a = if aspect.is_finite() && aspect > 0.0 {
            aspect.clamp(0.01, 100.0)
        } else {
            1.0
        };
        if a == self.aspect {
            return;
        }
        let (dw, dh) = self.display_size();
        let (fx, fy) = (self.cx.value / dw, self.cy.value / dh);
        self.aspect = a;
        let (dw, dh) = self.display_size();
        self.cx.snap(fx * dw);
        self.cy.snap(fy * dh);
        if self.mode != ZoomMode::Free {
            self.apply_mode(false);
        } else {
            self.clamp_now();
        }
        self.bump();
    }

    /// A still view of the image (loupe, navigator): `scale` screen pixels per image pixel,
    /// display point `center` in the middle of `viewport`, no clamping.
    pub fn fixed(
        stored: (u32, u32),
        orient: Orientation,
        viewport: PxRect,
        scale: f32,
        center: (f32, f32),
    ) -> View {
        let mut v = View::new();
        v.stored = (stored.0.max(1), stored.1.max(1));
        v.orient = orient;
        v.viewport = viewport;
        v.mode = ZoomMode::Free;
        v.zoom.snap(scale.max(1e-6).log2());
        v.cx.snap(center.0);
        v.cy.snap(center.1);
        v
    }

    /// The view turned by `radians` (clockwise on screen) about the viewport's centre: a
    /// still view of an image aligned onto another (compare).
    pub fn with_rotation(mut self, radians: f32) -> View {
        self.rotation = if radians.is_finite() { radians } else { 0.0 };
        self.bump();
        self
    }

    /// The turn [`View::with_rotation`] set (radians).
    pub fn rotation(&self) -> f32 {
        self.rotation
    }

    /// Display-space rectangle (x0, y0, x1, y1) the viewport shows now, clamped to the image
    /// (turned: the box round what it shows).
    pub fn visible_display(&self) -> (f32, f32, f32, f32) {
        let (dw, dh) = self.display_size();
        let vp = self.viewport;
        let mut r = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for c in [
            (vp.x, vp.y),
            (vp.x + vp.w, vp.y),
            (vp.x, vp.y + vp.h),
            (vp.x + vp.w, vp.y + vp.h),
        ] {
            let p = self.screen_to_display(c);
            r = (r.0.min(p.0), r.1.min(p.1), r.2.max(p.0), r.3.max(p.1));
        }
        (
            r.0.clamp(0.0, dw),
            r.1.clamp(0.0, dh),
            r.2.clamp(0.0, dw),
            r.3.clamp(0.0, dh),
        )
    }

    pub fn stored_size(&self) -> (u32, u32) {
        self.stored
    }

    pub fn orientation(&self) -> Orientation {
        self.orient
    }

    pub fn display_size(&self) -> (f32, f32) {
        let (w, h) = (self.stored.0 as f32, self.stored.1 as f32 * self.aspect);
        if self.orient.swaps_axes() {
            (h, w)
        } else {
            (w, h)
        }
    }

    /// Current (animated) scale: screen pixels per image pixel.
    pub fn scale(&self) -> f32 {
        self.zoom.value.exp2()
    }

    /// Where the zoom animation is heading.
    pub fn target_scale(&self) -> f32 {
        self.zoom.target.exp2()
    }

    pub fn fit_scale(&self) -> f32 {
        let (dw, dh) = self.display_size();
        let s = (self.viewport.w / dw).min(self.viewport.h / dh);
        if self.upscale_small { s } else { s.min(1.0) }
    }

    /// Whether the view is (or is animating to) fit: scale and centre match fit within a
    /// tolerance, so float drift does not count as zoomed or panned.
    pub fn is_at_fit(&self) -> bool {
        let (f, t) = (self.fit_scale(), self.target_scale());
        if f <= 0.0 || (t / f - 1.0).abs() > 1e-3 {
            return false;
        }
        let (dw, dh) = self.display_size();
        // Within one screen pixel of centred.
        (self.cx.target - dw * 0.5).abs() * t < 1.0 && (self.cy.target - dh * 0.5).abs() * t < 1.0
    }

    pub fn fill_scale(&self) -> f32 {
        let (dw, dh) = self.display_size();
        (self.viewport.w / dw).max(self.viewport.h / dh)
    }

    fn bump(&mut self) {
        self.revision += 1;
    }

    /// A new image: fit it, centred, without animation.
    pub fn set_image(&mut self, w: u32, h: u32, orient: Orientation) {
        self.stored = (w.max(1), h.max(1));
        self.orient = orient;
        self.aspect = 1.0;
        self.mode = ZoomMode::Fit;
        self.anchor = None;
        self.drag = None;
        self.momentum = false;
        let s = self.fit_scale();
        self.zoom.snap(s.log2());
        let (dw, dh) = self.display_size();
        self.cx.snap(dw * 0.5);
        self.cy.snap(dh * 0.5);
        self.bump();
    }

    /// Same image, new orientation (rotate/flip): keep the zoom mode.
    pub fn set_orientation(&mut self, orient: Orientation) {
        let (dw, dh) = self.display_size();
        let fx = self.cx.value / dw;
        let fy = self.cy.value / dh;
        let swapped = orient.swaps_axes() != self.orient.swaps_axes();
        self.orient = orient;
        let (dw, dh) = self.display_size();
        if swapped {
            self.cx.snap(fy * dw);
            self.cy.snap(fx * dh);
        }
        let _ = (fx, fy);
        if self.mode != ZoomMode::Free {
            self.apply_mode(true);
        }
        let (cx, cy) = (self.cx.value.min(dw), self.cy.value.min(dh));
        self.cx.snap(cx);
        self.cy.snap(cy);
        self.clamp_targets();
        self.bump();
    }

    pub fn set_viewport(&mut self, vp: PxRect) {
        if vp == self.viewport {
            return;
        }
        self.viewport = vp;
        if self.mode != ZoomMode::Free {
            self.apply_mode(false);
        } else {
            self.clamp_now();
        }
        self.bump();
    }

    fn apply_mode(&mut self, animate: bool) {
        let (dw, dh) = self.display_size();
        let s = match self.mode {
            ZoomMode::Fill => self.fill_scale(),
            ZoomMode::FitWidth | ZoomMode::FitHeight => {
                let side = if self.mode == ZoomMode::FitWidth {
                    self.viewport.w / dw
                } else {
                    self.viewport.h / dh
                };
                let fit = self.fit_scale();
                side.clamp(MIN_SCALE.min(fit), self.max_scale.max(fit))
            }
            _ => self.fit_scale(),
        };
        // Fitting one side starts at the top (width) or the reading side (height) of what
        // overflows: the left, or the right for right-to-left reading.
        let (cx, cy) = match self.mode {
            ZoomMode::FitWidth => (dw * 0.5, (self.viewport.h * 0.5 / s).min(dh * 0.5)),
            ZoomMode::FitHeight => {
                let from_edge = (self.viewport.w * 0.5 / s).min(dw * 0.5);
                let cx = if self.start_right {
                    dw - from_edge
                } else {
                    from_edge
                };
                (cx, dh * 0.5)
            }
            _ => (dw * 0.5, dh * 0.5),
        };
        self.anchor = None;
        if animate {
            self.zoom.set(s.log2());
            self.cx.set(cx);
            self.cy.set(cy);
        } else {
            self.zoom.snap(s.log2());
            self.cx.snap(cx);
            self.cy.snap(cy);
        }
    }

    /// Fits the image's width (`width`) or height to the window, animated unless `snap`.
    pub fn zoom_fit_side(&mut self, width: bool, snap: bool) {
        self.mode = if width {
            ZoomMode::FitWidth
        } else {
            ZoomMode::FitHeight
        };
        self.apply_mode(!snap);
        self.bump();
    }

    pub fn zoom_fit(&mut self) {
        self.mode = ZoomMode::Fit;
        self.apply_mode(true);
        self.bump();
    }

    pub fn zoom_fill(&mut self) {
        self.mode = ZoomMode::Fill;
        self.apply_mode(true);
        self.bump();
    }

    /// Display point under a screen point (current, animated state).
    pub fn screen_to_display(&self, p: (f32, f32)) -> (f32, f32) {
        let s = self.scale();
        let (vx, vy) = self.viewport.center();
        let (dx, dy) = ((p.0 - vx) / s, (p.1 - vy) / s);
        let (sn, cs) = self.rotation.sin_cos();
        (
            self.cx.value + cs * dx + sn * dy,
            self.cy.value - sn * dx + cs * dy,
        )
    }

    pub fn display_to_screen(&self, d: (f32, f32)) -> (f32, f32) {
        let s = self.scale();
        let (vx, vy) = self.viewport.center();
        let (dx, dy) = (d.0 - self.cx.value, d.1 - self.cy.value);
        let (sn, cs) = self.rotation.sin_cos();
        ((cs * dx - sn * dy) * s + vx, (sn * dx + cs * dy) * s + vy)
    }

    /// Zooms to `scale`, keeping the image point under `at` (screen) fixed; `None` keeps the
    /// viewport centre fixed.
    pub fn zoom_to(&mut self, scale: f32, at: Option<(f32, f32)>) {
        // A NaN target would never settle and poison every later zoom (winit documents
        // that pinch deltas may be NaN).
        if !scale.is_finite() || scale <= 0.0 {
            return;
        }
        let scale = scale.clamp(MIN_SCALE.min(self.fit_scale()), self.max_scale);
        let at = at.unwrap_or_else(|| self.viewport.center());
        // Keep the existing anchor when the zoom is still animating so repeated wheel
        // ticks accumulate around the same point.
        let display = match self.anchor {
            Some(a) if a.screen == at => a.display,
            _ => self.screen_to_display(at),
        };
        self.anchor = Some(Anchor {
            display,
            screen: at,
        });
        self.momentum = false;
        self.mode = ZoomMode::Free;
        self.zoom.set(scale.log2());
        self.bump();
    }

    /// Multiplies the zoom at once, without animation (touchpad pinch, smooth Ctrl+wheel):
    /// the image point under `at` (screen; `None`: the viewport centre) stays where it is and
    /// a running zoom animation stops where it is shown now. Factors that are not finite and
    /// positive are ignored.
    pub fn zoom_direct_by(&mut self, factor: f32, at: Option<(f32, f32)>) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let scale = (self.scale() * factor).clamp(MIN_SCALE.min(self.fit_scale()), self.max_scale);
        let at = at.unwrap_or_else(|| self.viewport.center());
        let d = self.screen_to_display(at);
        let (vx, vy) = self.viewport.center();
        self.anchor = None;
        self.momentum = false;
        self.velocity = (0.0, 0.0);
        self.mode = ZoomMode::Free;
        self.zoom.snap(scale.log2());
        self.cx.snap(d.0 - (at.0 - vx) / scale);
        self.cy.snap(d.1 - (at.1 - vy) / scale);
        self.clamp_now();
        self.bump();
    }

    /// End of a continuous zoom (pinch released): a scale within [`SOFT_SNAP`] of fit or of
    /// 100 % glides onto it (fit recentres; 100 % keeps the point under `at`). Returns whether
    /// it snapped.
    pub fn soft_snap(&mut self, at: Option<(f32, f32)>) -> bool {
        let t = self.target_scale();
        let fit = self.fit_scale();
        let near = |a: f32, b: f32| (a / b - 1.0).abs() < SOFT_SNAP && (a / b - 1.0).abs() > 1e-5;
        if near(t, fit) {
            self.zoom_fit();
            true
        } else if near(t, 1.0) {
            self.zoom_to(1.0, at);
            true
        } else {
            false
        }
    }

    /// Multiplies the target zoom.
    pub fn zoom_by(&mut self, factor: f32, at: Option<(f32, f32)>) {
        let t = self.target_scale() * factor;
        // Snap through 100 % and fit so they are easy to hit with the wheel.
        let fit = self.fit_scale();
        let cur = self.target_scale();
        let t = if (cur < 1.0 && t > 1.0) || (cur > 1.0 && t < 1.0) {
            1.0
        } else if (cur < fit && t > fit) || (cur > fit && t < fit) {
            fit
        } else {
            t
        };
        if (t - fit).abs() < 1e-4 && self.mode != ZoomMode::Free {
            return;
        }
        self.zoom_to(t, at);
        if (t - fit).abs() < 1e-4 {
            self.mode = ZoomMode::Fit;
            self.anchor = None;
            self.apply_mode(true);
        }
    }

    /// Double-click: fit ↔ 100 % at the cursor.
    pub fn toggle_fit_actual(&mut self, at: (f32, f32)) {
        let fit = self.fit_scale();
        if self.mode == ZoomMode::Fit || (self.target_scale() - fit).abs() < 1e-3 {
            if (fit - 1.0).abs() < 1e-3 {
                self.zoom_to(2.0, Some(at));
            } else {
                self.zoom_to(1.0, Some(at));
            }
        } else {
            self.zoom_fit();
        }
    }

    /// Zooms so the screen rectangle fills the viewport (z + drag).
    pub fn zoom_rect(&mut self, a: (f32, f32), b: (f32, f32)) {
        let (w, h) = ((a.0 - b.0).abs(), (a.1 - b.1).abs());
        if w < 4.0 || h < 4.0 {
            return;
        }
        let da = self.screen_to_display(a);
        let db = self.screen_to_display(b);
        let s = self.scale() * (self.viewport.w / w).min(self.viewport.h / h);
        let s = s.clamp(MIN_SCALE, self.max_scale);
        self.anchor = None;
        self.mode = ZoomMode::Free;
        self.zoom.set(s.log2());
        self.cx.set((da.0 + db.0) * 0.5);
        self.cy.set((da.1 + db.1) * 0.5);
        self.clamp_targets();
        self.bump();
    }

    /// Centres on a display point (navigator).
    pub fn center_on(&mut self, d: (f32, f32), animate: bool) {
        self.anchor = None;
        self.momentum = false;
        if self.mode != ZoomMode::Free {
            self.mode = ZoomMode::Free;
        }
        if animate {
            self.cx.set(d.0);
            self.cy.set(d.1);
        } else {
            self.cx.snap(d.0);
            self.cy.snap(d.1);
        }
        self.clamp_targets();
        if !animate {
            self.clamp_now();
        }
        self.bump();
    }

    /// Shows the stored-pixel rectangle `x0, y0, x1, y1` (as oriented) centred, at `scale`
    /// (screen pixels per image pixel), or fitted into the viewport when `None` (a DICOM
    /// presentation state's displayed area).
    pub fn show_stored_rect(&mut self, r: (f32, f32, f32, f32), scale: Option<f32>) {
        let d = self.display_from_stored();
        let (a, b) = (d.apply((r.0, r.1)), d.apply((r.2, r.3)));
        let (dw, dh) = ((a.0 - b.0).abs(), (a.1 - b.1).abs());
        if !(dw > 0.0 && dh > 0.0) {
            return;
        }
        let s = scale
            .unwrap_or_else(|| (self.viewport.w / dw).min(self.viewport.h / dh))
            .clamp(MIN_SCALE.min(self.fit_scale()), self.max_scale);
        if !s.is_finite() {
            return;
        }
        self.anchor = None;
        self.momentum = false;
        self.mode = ZoomMode::Free;
        self.zoom.set(s.log2());
        self.cx.set((a.0 + b.0) * 0.5);
        self.cy.set((a.1 + b.1) * 0.5);
        self.clamp_targets();
        self.bump();
    }

    pub fn center(&self) -> (f32, f32) {
        (self.cx.value, self.cy.value)
    }

    /// Pans by a screen-space delta (keyboard, scroll).
    pub fn pan_by(&mut self, dx: f32, dy: f32, animate: bool) {
        let s = self.scale();
        self.anchor = None;
        self.momentum = false;
        let (tx, ty) = (self.cx.target - dx / s, self.cy.target - dy / s);
        if animate {
            self.cx.set(tx);
            self.cy.set(ty);
        } else {
            self.cx.snap(tx);
            self.cy.snap(ty);
        }
        self.clamp_targets();
        if !animate {
            self.clamp_now();
        }
        self.bump();
    }

    pub fn can_pan(&self) -> bool {
        let (dw, dh) = self.display_size();
        let s = self.scale();
        dw * s > self.viewport.w + 0.5 || dh * s > self.viewport.h + 0.5
    }

    pub fn begin_drag(&mut self, t: f64) {
        self.anchor = None;
        self.momentum = false;
        self.velocity = (0.0, 0.0);
        self.cx.snap(self.cx.value);
        self.cy.snap(self.cy.value);
        self.drag = Some(DragState {
            raw: (self.cx.value, self.cy.value),
            last_t: t,
        });
    }

    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// Pointer moved by (`dx`, `dy`) screen pixels at time `t` (seconds).
    pub fn drag_by(&mut self, dx: f32, dy: f32, t: f64) {
        let s = self.scale();
        let Some(mut d) = self.drag else { return };
        d.raw.0 -= dx / s;
        d.raw.1 -= dy / s;
        let dt = (t - d.last_t).max(1e-4) as f32;
        let inst = (dx / dt, dy / dt);
        // Exponentially smoothed velocity in screen px/s.
        let k = (dt / 0.04).min(1.0);
        self.velocity = (
            self.velocity.0 + (inst.0 - self.velocity.0) * k,
            self.velocity.1 + (inst.1 - self.velocity.1) * k,
        );
        d.last_t = t;
        self.drag = Some(d);
        let ((x0, x1), (y0, y1)) = self.center_range(s);
        let rb = |v: f32, lo: f32, hi: f32, dim: f32| {
            if v < lo {
                lo - rubber(lo - v, dim)
            } else if v > hi {
                hi + rubber(v - hi, dim)
            } else {
                v
            }
        };
        let vw = self.viewport.w / s;
        let vh = self.viewport.h / s;
        self.cx.snap(rb(d.raw.0, x0, x1, vw * 0.25));
        self.cy.snap(rb(d.raw.1, y0, y1, vh * 0.25));
        self.bump();
    }

    pub fn end_drag(&mut self, t: f64) {
        if let Some(d) = self.drag.take() {
            // A pause before release cancels the flick.
            if t - d.last_t > 0.06 {
                self.velocity = (0.0, 0.0);
            }
            let sp = (self.velocity.0.powi(2) + self.velocity.1.powi(2)).sqrt();
            self.momentum = sp > 120.0;
            self.clamp_targets();
        }
    }

    /// Stops any motion (click during a glide).
    pub fn stop(&mut self) {
        self.momentum = false;
        self.velocity = (0.0, 0.0);
    }

    fn center_range(&self, s: f32) -> ((f32, f32), (f32, f32)) {
        let (dw, dh) = self.display_size();
        let hw = self.viewport.w / s * 0.5;
        let hh = self.viewport.h / s * 0.5;
        let rx = if dw <= 2.0 * hw {
            (dw * 0.5, dw * 0.5)
        } else {
            (hw, dw - hw)
        };
        let ry = if dh <= 2.0 * hh {
            (dh * 0.5, dh * 0.5)
        } else {
            (hh, dh - hh)
        };
        (rx, ry)
    }

    fn clamp_targets(&mut self) {
        let s = self.target_scale();
        let ((x0, x1), (y0, y1)) = self.center_range(s);
        self.cx.target = self.cx.target.clamp(x0, x1);
        self.cy.target = self.cy.target.clamp(y0, y1);
    }

    fn clamp_now(&mut self) {
        let s = self.scale();
        let ((x0, x1), (y0, y1)) = self.center_range(s);
        self.cx.snap(self.cx.value.clamp(x0, x1));
        self.cy.snap(self.cy.value.clamp(y0, y1));
    }

    /// Advances animations by `dt` seconds. Returns true while anything moves.
    pub fn step(&mut self, dt: f32) -> bool {
        let mut moving = false;
        if self.zoom.step(dt) {
            moving = true;
        }
        if let Some(a) = self.anchor {
            let s = self.scale();
            let (vx, vy) = self.viewport.center();
            let c = (
                a.display.0 - (a.screen.0 - vx) / s,
                a.display.1 - (a.screen.1 - vy) / s,
            );
            let ((x0, x1), (y0, y1)) = self.center_range(s);
            let c = (c.0.clamp(x0, x1), c.1.clamp(y0, y1));
            self.cx.snap(c.0);
            self.cy.snap(c.1);
            if !moving {
                self.anchor = None;
            }
        } else if self.drag.is_none() {
            if self.momentum {
                let s = self.scale();
                self.cx.snap(self.cx.value - self.velocity.0 * dt / s);
                self.cy.snap(self.cy.value - self.velocity.1 * dt / s);
                let decay = (-dt * 4.5).exp();
                self.velocity = (self.velocity.0 * decay, self.velocity.1 * decay);
                let ((x0, x1), (y0, y1)) = self.center_range(s);
                if self.cx.value < x0 || self.cx.value > x1 {
                    self.velocity.0 = 0.0;
                }
                if self.cy.value < y0 || self.cy.value > y1 {
                    self.velocity.1 = 0.0;
                }
                if (self.velocity.0.powi(2) + self.velocity.1.powi(2)).sqrt() < 15.0 {
                    self.momentum = false;
                }
                self.clamp_targets();
                moving = true;
            } else {
                let ((x0, x1), (y0, y1)) = self.center_range(self.target_scale());
                self.cx.target = self.cx.target.clamp(x0, x1);
                self.cy.target = self.cy.target.clamp(y0, y1);
            }
            if self.cx.step(dt) {
                moving = true;
            }
            if self.cy.step(dt) {
                moving = true;
            }
        }
        if moving {
            self.bump();
        }
        moving
    }

    /// Jumps to the end of every animation (no zoom animation, benchmarks).
    pub fn settle(&mut self) {
        self.momentum = false;
        self.velocity = (0.0, 0.0);
        for _ in 0..2000 {
            if !self.step(1.0 / 120.0) {
                break;
            }
        }
        self.bump();
    }

    pub fn is_animating(&self) -> bool {
        self.momentum
            || !self.zoom.settled()
            || !self.cx.settled()
            || !self.cy.settled()
            || self.anchor.is_some()
    }

    /// Stored-image coordinates from window coordinates (physical px).
    pub fn stored_from_screen(&self) -> Affine {
        let s = self.scale();
        let (vx, vy) = self.viewport.center();
        // display = R(−θ)·(p − v)/s + c
        let (sn, cs) = self.rotation.sin_cos();
        let disp = Affine {
            m: [
                cs / s,
                sn / s,
                self.cx.value - (cs * vx + sn * vy) / s,
                -sn / s,
                cs / s,
                self.cy.value - (-sn * vx + cs * vy) / s,
            ],
        };
        disp.then(&self.display_from_stored().invert())
    }

    /// Window coordinates (physical px) from stored coordinates, worked out in `f64` as
    /// `[a, b, c, d, e, f]` (x' = a·x + b·y + c, y' = d·x + e·y + f): for content drawn at
    /// the exact zoom, where `f32` would wobble far in.
    pub fn screen_from_stored_f64(&self) -> [f64; 6] {
        let s = f64::from(self.scale());
        let (vx, vy) = self.viewport.center();
        let (cx, cy) = (f64::from(self.cx.value), f64::from(self.cy.value));
        let d = self.display_from_stored().m.map(f64::from);
        // screen = R(θ)·(display − c)·s + v
        let (sn, cs) = f64::from(self.rotation).sin_cos();
        let (r0, r1) = ([cs * s, -sn * s], [sn * s, cs * s]);
        let (ex, ey) = (d[2] - cx, d[5] - cy);
        [
            r0[0] * d[0] + r0[1] * d[3],
            r0[0] * d[1] + r0[1] * d[4],
            r0[0] * ex + r0[1] * ey + f64::from(vx),
            r1[0] * d[0] + r1[1] * d[3],
            r1[0] * d[1] + r1[1] * d[4],
            r1[0] * ex + r1[1] * ey + f64::from(vy),
        ]
    }

    /// Screen rectangle covered by the image (clipped to nothing; may exceed the window).
    pub fn image_screen_rect(&self) -> PxRect {
        let (dw, dh) = self.display_size();
        let a = self.display_to_screen((0.0, 0.0));
        let b = self.display_to_screen((dw, dh));
        PxRect {
            x: a.0,
            y: a.1,
            w: b.0 - a.0,
            h: b.1 - a.1,
        }
    }

    /// Stored-pixel rectangle visible in the viewport (x0, y0, x1, y1), clamped to the image.
    /// `at_target` uses where the animation is heading instead of where it is.
    pub fn visible_stored(&self, at_target: bool) -> (f32, f32, f32, f32) {
        let s = if at_target {
            self.target_scale()
        } else {
            self.scale()
        };
        let (cx, cy) = if at_target {
            (self.cx.target, self.cy.target)
        } else {
            (self.cx.value, self.cy.value)
        };
        let hw = self.viewport.w / s * 0.5;
        let hh = self.viewport.h / s * 0.5;
        let (w, h) = (self.stored.0 as f32, self.stored.1 as f32);
        let inv = self.display_from_stored().invert();
        // The viewport's corners in display space (turned with the view).
        let (sn, cs) = self.rotation.sin_cos();
        let turn = |x: f32, y: f32| (cx + cs * x + sn * y, cy - sn * x + cs * y);
        let corners = [turn(-hw, -hh), turn(hw, -hh), turn(-hw, hh), turn(hw, hh)];
        let mut r = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for c in corners {
            let p = inv.apply(c);
            r = (r.0.min(p.0), r.1.min(p.1), r.2.max(p.0), r.3.max(p.1));
        }
        (r.0.max(0.0), r.1.max(0.0), r.2.min(w), r.3.min(h))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_fit_tolerates_drift_but_not_zoom_or_pan() {
        let mut v = View::new();
        v.set_viewport(PxRect {
            x: 0.0,
            y: 0.0,
            w: 1000.0,
            h: 500.0,
        });
        v.set_image(4000, 3000, Orientation::NORMAL);
        assert!(v.is_at_fit());
        v.zoom_to(v.fit_scale() * 1.00001, None);
        assert!(v.is_at_fit(), "float drift");
        v.zoom_to(1.0, None);
        assert!(!v.is_at_fit(), "zoomed in");
        v.zoom_fit();
        assert!(v.is_at_fit(), "animating back to fit counts");
        v.settle();
        v.zoom_to(v.fit_scale() * 0.5, None);
        assert!(!v.is_at_fit(), "zoomed out");
    }

    #[test]
    fn orientation_maps_corners() {
        // Orientation 6 = rotate 90° clockwise: stored (0,0) goes to display top-right.
        let t = display_from_stored(Orientation(6), 400.0, 300.0);
        assert_eq!(t.apply((0.0, 0.0)), (300.0, 0.0));
        assert_eq!(t.apply((400.0, 300.0)), (0.0, 400.0));
        let i = t.invert();
        let p = i.apply(t.apply((123.0, 45.0)));
        assert!((p.0 - 123.0).abs() < 1e-3 && (p.1 - 45.0).abs() < 1e-3);
        // Mirror.
        let m = display_from_stored(Orientation(2), 400.0, 300.0);
        assert_eq!(m.apply((0.0, 0.0)), (400.0, 0.0));
    }

    #[test]
    fn fit_and_anchor_zoom() {
        let mut v = View::new();
        v.set_viewport(PxRect {
            x: 0.0,
            y: 0.0,
            w: 1000.0,
            h: 500.0,
        });
        v.set_image(4000, 1000, Orientation::NORMAL);
        assert!((v.scale() - 0.25).abs() < 1e-5);
        let at = (900.0, 200.0);
        let before = v.screen_to_display(at);
        v.zoom_to(1.0, Some(at));
        for _ in 0..200 {
            v.step(1.0 / 120.0);
        }
        assert!((v.scale() - 1.0).abs() < 1e-3);
        let after = v.screen_to_display(at);
        assert!((before.0 - after.0).abs() < 1.0, "{before:?} {after:?}");
        assert!((before.1 - after.1).abs() < 1.0);
    }

    #[test]
    fn shows_a_stored_rect() {
        let mut v = View::new();
        v.set_viewport(PxRect {
            x: 0.0,
            y: 0.0,
            w: 1000.0,
            h: 500.0,
        });
        // Turned a quarter: the stored rect 100..300 × 0..100 is display 900..1000 × 100..300.
        v.set_image(1000, 1000, Orientation(6));
        v.show_stored_rect((100.0, 0.0, 300.0, 100.0), None);
        v.settle();
        assert!((v.scale() - 2.5).abs() < 1e-3, "{}", v.scale());
        let c = v.screen_to_display((500.0, 250.0));
        // Clamped so the image still fills the viewport: as far right as it goes.
        assert!(c.0 > 799.0 && (c.1 - 200.0).abs() < 1.0, "{c:?}");
        // A magnification.
        v.show_stored_rect((0.0, 0.0, 1000.0, 1000.0), Some(0.5));
        v.settle();
        assert!((v.scale() - 0.5).abs() < 1e-3);
    }

    fn test_view() -> View {
        let mut v = View::new();
        v.set_viewport(PxRect {
            x: 0.0,
            y: 0.0,
            w: 1000.0,
            h: 500.0,
        });
        v.set_image(4000, 2000, Orientation::NORMAL);
        v
    }

    #[test]
    fn direct_zoom_is_immediate_and_keeps_the_point_under_the_fingers() {
        let mut v = test_view();
        let at = (800.0, 120.0);
        let before = v.screen_to_display(at);
        let mut f = 1.0;
        for _ in 0..40 {
            v.zoom_direct_by(1.02, Some(at));
            f *= 1.02;
        }
        assert!(!v.is_animating(), "no animation to wait for");
        assert!((v.scale() / (0.25 * f) - 1.0).abs() < 1e-4);
        let after = v.screen_to_display(at);
        assert!((before.0 - after.0).abs() < 0.5, "{before:?} {after:?}");
        assert!((before.1 - after.1).abs() < 0.5);
    }

    #[test]
    fn fit_width_shows_the_top_and_follows_the_window() {
        let mut v = View::new();
        let vp = |w, h| PxRect {
            x: 0.0,
            y: 0.0,
            w,
            h,
        };
        v.set_viewport(vp(1000.0, 500.0));
        // A tall page: 1000 x 3000.
        v.set_image(1000, 3000, Orientation::NORMAL);
        v.zoom_fit_side(true, true);
        assert_eq!(v.mode, ZoomMode::FitWidth);
        assert!((v.scale() - 1.0).abs() < 1e-6);
        // Its top: the centre half a window below the top edge.
        assert!((v.center().1 - 250.0).abs() < 1e-3, "{:?}", v.center());
        v.set_viewport(vp(500.0, 500.0));
        assert!((v.scale() - 0.5).abs() < 1e-6);
        assert!((v.center().1 - 500.0).abs() < 1e-3, "{:?}", v.center());
        // Fit height of the same page: all of it, centred (it fits the width).
        v.zoom_fit_side(false, true);
        assert!((v.scale() - 500.0 / 3000.0).abs() < 1e-6);
        assert!((v.center().0 - 500.0).abs() < 1e-3);
        // A wide spread fitted to the height starts at its left.
        v.set_image(3000, 1000, Orientation::NORMAL);
        v.zoom_fit_side(false, true);
        assert!((v.scale() - 0.5).abs() < 1e-6);
        assert!((v.center().0 - 500.0).abs() < 1e-3, "{:?}", v.center());
        // Read right to left, at its right; still there after a resize.
        v.start_right = true;
        v.zoom_fit_side(false, true);
        assert!((v.center().0 - 2500.0).abs() < 1e-3, "{:?}", v.center());
        v.set_viewport(vp(400.0, 500.0));
        assert!((v.center().0 - 2600.0).abs() < 1e-3, "{:?}", v.center());
        // A page narrower than the window stays centred either way.
        v.set_image(1000, 3000, Orientation::NORMAL);
        v.set_viewport(vp(1000.0, 500.0));
        v.zoom_fit_side(false, true);
        assert!((v.center().0 - 500.0).abs() < 1e-3, "{:?}", v.center());
    }

    #[test]
    fn direct_zoom_starts_from_what_is_shown() {
        let mut v = test_view();
        v.zoom_to(1.0, None);
        v.step(1.0 / 120.0);
        let shown = v.scale();
        assert!(shown < 0.9, "still animating");
        v.zoom_direct_by(1.1, None);
        assert!(
            (v.scale() / (shown * 1.1) - 1.0).abs() < 1e-4,
            "no jump to the old target"
        );
        assert!((v.target_scale() - v.scale()).abs() < 1e-6);
    }

    #[test]
    fn nan_and_bad_factors_are_ignored() {
        let mut v = test_view();
        let s = v.scale();
        v.zoom_direct_by(f32::NAN, None);
        v.zoom_direct_by(0.0, None);
        v.zoom_direct_by(f32::INFINITY, None);
        v.zoom_to(f32::NAN, None);
        v.zoom_to(s * f32::NAN, Some((1.0, 1.0)));
        assert_eq!(v.scale(), s);
        assert!(v.target_scale().is_finite());
        assert!(v.center().0.is_finite() && v.center().1.is_finite());
    }

    #[test]
    fn soft_snap_to_fit_and_actual_size() {
        let mut v = test_view();
        v.zoom_direct_by(1.03, None);
        assert!(v.soft_snap(None));
        v.settle();
        assert_eq!(v.mode, ZoomMode::Fit);
        assert!((v.scale() - 0.25).abs() < 1e-5);
        v.zoom_direct_by(3.9, Some((300.0, 300.0)));
        assert!(v.soft_snap(Some((300.0, 300.0))));
        v.settle();
        assert!((v.scale() - 1.0).abs() < 1e-4);
        v.zoom_direct_by(1.5, None);
        assert!(!v.soft_snap(None), "far from both");
        assert!((v.scale() - 1.5).abs() < 1e-4);
    }

    /// A pixel aspect stretches rows before the orientation: a 100×50 image at 2:1 fits as
    /// a square, turned or not; a new image is square again.
    #[test]
    fn pixel_aspect_stretches_rows() {
        let mut v = View::new();
        v.upscale_small = true;
        v.set_viewport(PxRect {
            x: 0.0,
            y: 0.0,
            w: 400.0,
            h: 400.0,
        });
        v.set_image(100, 50, Orientation::NORMAL);
        v.set_pixel_aspect(2.0);
        assert_eq!(v.display_size(), (100.0, 100.0));
        assert!((v.scale() - 4.0).abs() < 1e-5);
        let t = v.stored_from_screen().invert();
        let (a, b) = (t.apply((0.0, 0.0)), t.apply((100.0, 50.0)));
        assert!(
            (a.0, a.1) == (0.0, 0.0) && (b.0 - 400.0).abs() < 1e-3 && (b.1 - 400.0).abs() < 1e-3
        );
        // A quarter turn: stored rows (now columns) still twice as long on screen.
        v.set_orientation(Orientation::from_rotation_flip(1, false));
        let t = v.stored_from_screen().invert();
        let (a, b) = (t.apply((0.0, 0.0)), t.apply((0.0, 1.0)));
        assert!(((a.0 - b.0).abs() - 8.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3);
        let (a, b) = (t.apply((0.0, 0.0)), t.apply((1.0, 0.0)));
        assert!(((a.1 - b.1).abs() - 4.0).abs() < 1e-3);
        let r = v.visible_stored(false);
        assert!(
            (r.2 - 100.0).abs() < 1e-3 && (r.3 - 50.0).abs() < 1e-3,
            "{r:?}"
        );
        v.set_pixel_aspect(f32::NAN);
        assert_eq!(v.pixel_aspect(), 1.0);
        v.set_pixel_aspect(3.0);
        v.set_image(10, 10, Orientation::NORMAL);
        assert_eq!(v.pixel_aspect(), 1.0);
    }

    #[test]
    fn a_turned_view_agrees_with_itself() {
        let vp = PxRect {
            x: 0.0,
            y: 0.0,
            w: 200.0,
            h: 100.0,
        };
        let v = View::fixed((400, 300), Orientation::NORMAL, vp, 0.5, (200.0, 150.0))
            .with_rotation(0.1);
        // Screen ↔ display round trip, and the stored transform agrees with it.
        let p = (37.0, 81.0);
        let d = v.screen_to_display(p);
        let q = v.display_to_screen(d);
        assert!(
            (q.0 - p.0).abs() < 1e-3 && (q.1 - p.1).abs() < 1e-3,
            "{q:?}"
        );
        let st = v.stored_from_screen().apply(p);
        assert!((st.0 - d.0).abs() < 1e-3 && (st.1 - d.1).abs() < 1e-3);
        let f = v.screen_from_stored_f64();
        let back = (
            f[0] * f64::from(d.0) + f[1] * f64::from(d.1) + f[2],
            f[3] * f64::from(d.0) + f[4] * f64::from(d.1) + f[5],
        );
        assert!((back.0 - 37.0).abs() < 1e-3 && (back.1 - 81.0).abs() < 1e-3);
        // A turn clockwise on screen: display +x points down-right.
        let a = v.display_to_screen((200.0, 150.0));
        let b = v.display_to_screen((210.0, 150.0));
        assert!(b.1 > a.1 && b.0 > a.0);
        // The visible box holds every corner of the viewport.
        let (x0, y0, x1, y1) = v.visible_display();
        let corner = v.screen_to_display((200.0, 0.0));
        assert!(corner.0 <= x1 + 1e-3 && corner.1 >= y0 - 1e-3 && x0 < x1 && y0 < y1);
        // No turn: as before.
        let plain = View::fixed((400, 300), Orientation::NORMAL, vp, 0.5, (200.0, 150.0));
        assert_eq!(plain.rotation(), 0.0);
        assert_eq!(plain.visible_display(), (0.0, 50.0, 400.0, 250.0));
    }
}
