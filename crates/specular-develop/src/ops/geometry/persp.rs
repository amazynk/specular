//! Perspective correction: keystone, rotation and upright as
//! one homography inside the fused warp, so the source is still sampled once.
//!
//! # Model
//!
//! The camera is turned about its centre: `H = K' · R · K⁻¹` on the oriented frame in square
//! pixels, centred on the frame centre (y down), with `K = diag(f, f, 1)` and
//! `R = Rz(roll) · Rx(pitch) · Ry(yaw)`. The focal length `f` (pixels) comes from the
//! module's own 35 mm value, else the file's `FocalLengthIn35mmFilm`, else 28 mm:
//! `f = f35 / 43.27 · hypot(w, h)`. The angles are physical, so a slider means the same on
//! every lens and size.
//!
//! - **pitch** (vertical, ±30° at ±100): positive widens the top (a camera tilted up).
//! - **yaw** (horizontal): positive widens the right side.
//! - **roll** (rotate, ±10°): positive turns clockwise on screen, as straighten does.
//!
//! The warped frame is moved back so the frame centre stays put, then scaled (aspect,
//! scale, the fill zoom) and shifted (offsets). The straighten rotation of the crop module
//! follows. [`Plan`](super::Plan) holds the inverse of the whole forward map.
//!
//! # Validity
//!
//! Pitch and yaw are scaled back together until the horizon line of `H` (where its third
//! row is 0) lies at least a tenth of the diagonal outside the frame: the warped frame then
//! stays a convex quadrilateral, and four corners inside a crop's region mean the whole crop
//! is inside ([`super::crop::Frame`]).
//!
//! # Modes
//!
//! - **off**: the sliders alone.
//! - **guided**: up to four lines the user drew ([`PerspParams::guides`], source fractions,
//!   so they follow lens correction and turns) are made plumb or level by
//!   [`super::upright::solve`] on every render; the sliders add to the result.
//! - **level, vertical, full, auto**: the angles the last automatic run found
//!   ([`PerspParams::solved`], from detected line segments, [`super::upright::auto`]); the
//!   sliders add to them.

pub use crate::colour::M3;
use crate::model::fields::choice;
use crate::model::params::ModuleParams;
use crate::model::{Edit, Fields, ModuleKind, Value};

/// The diagonal of a 36 × 24 mm frame.
pub const FULL_FRAME_DIAGONAL_MM: f64 = 43.266_615;
/// The focal length without one from the module or the file (35 mm equivalent).
pub const DEFAULT_FOCAL_MM35: f64 = 28.0;
/// Pitch and yaw at a slider's end (degrees).
pub const MAX_KEYSTONE_DEG: f64 = 30.0;
/// The rotate slider's end (degrees).
pub const MAX_ROTATE_DEG: f64 = 10.0;
/// The horizon line stays this far outside the frame (fraction of the diagonal).
pub const HORIZON_MARGIN: f64 = 0.1;
/// Guides one edit holds.
pub const MAX_GUIDES: usize = 4;
/// The x/y scale ratio at the aspect slider's end.
const ASPECT_AT_100: f64 = 1.5;

choice! {
    /// How the angles are found (see the module documentation).
    Mode {
        Off = "off",
        Guided = "guided",
        Level = "level",
        Vertical = "vertical",
        Full = "full",
        Auto = "auto",
    }
}

impl Mode {
    /// The modes the automatic run serves.
    pub fn is_automatic(self) -> bool {
        matches!(self, Mode::Level | Mode::Vertical | Mode::Full | Mode::Auto)
    }
}

/// The fields of [`ModuleKind::PERSPECTIVE`], version 1.
#[derive(Clone, Debug, PartialEq)]
pub struct PerspParams {
    pub mode: Mode,
    /// −100..100: pitch, ±[`MAX_KEYSTONE_DEG`] at the ends.
    pub vertical: f32,
    /// −100..100: yaw.
    pub horizontal: f32,
    /// Degrees, ±[`MAX_ROTATE_DEG`]: added to the roll.
    pub rotate: f32,
    /// −100..100: wider (positive) or taller, [`ASPECT_AT_100`] between x and y at the ends.
    pub aspect: f32,
    /// Percent, 50..150.
    pub scale: f32,
    /// −100..100: a quarter of the frame's width (height) at the ends.
    pub offset_x: f32,
    pub offset_y: f32,
    /// 35 mm equivalent focal length; 0 takes the file's. Written only when set.
    pub focal_mm35: f32,
    /// Drawn lines, `[x0, y0, x1, y1]` in fractions of the source frame. Written only when
    /// there are some.
    pub guides: Vec<[f32; 4]>,
    /// Zoom so the keystone leaves no empty edges.
    pub auto_scale: bool,
    /// Pitch, yaw and roll (degrees) the last automatic run found. Written only when set.
    pub solved: Option<[f32; 3]>,
}

impl Default for PerspParams {
    fn default() -> Self {
        Self {
            mode: Mode::Off,
            vertical: 0.0,
            horizontal: 0.0,
            rotate: 0.0,
            aspect: 0.0,
            scale: 100.0,
            offset_x: 0.0,
            offset_y: 0.0,
            focal_mm35: 0.0,
            guides: Vec::new(),
            auto_scale: true,
            solved: None,
        }
    }
}

impl ModuleParams for PerspParams {
    const KIND: ModuleKind = ModuleKind::PERSPECTIVE;
    const VERSION: u16 = 1;

    fn to_fields(&self) -> Fields {
        let mut f = Fields::new()
            .with("mode", self.mode.into())
            .with("vertical", Value::Float(self.vertical))
            .with("horizontal", Value::Float(self.horizontal))
            .with("rotate", Value::Float(self.rotate))
            .with("aspect", Value::Float(self.aspect))
            .with("scale", Value::Float(self.scale))
            .with("offset_x", Value::Float(self.offset_x))
            .with("offset_y", Value::Float(self.offset_y))
            .with("auto_scale", Value::Bool(self.auto_scale));
        if self.focal_mm35 > 0.0 {
            f.set("focal_mm35", Value::Float(self.focal_mm35));
        }
        if !self.guides.is_empty() {
            f.set(
                "guides",
                Value::Floats(self.guides.iter().flatten().copied().collect()),
            );
        }
        if let Some(s) = self.solved {
            f.set("solved", Value::Floats(s.to_vec()));
        }
        f
    }

    fn from_fields(x: &Fields) -> Self {
        let d = Self::default();
        let c = |name: &str, def: f32, lo: f32, hi: f32| {
            let v = x.f32_or(name, def);
            if v.is_finite() { v.clamp(lo, hi) } else { def }
        };
        let guides = match x.get("guides") {
            Some(Value::Floats(v)) => v
                .chunks_exact(4)
                .filter(|g| g.iter().all(|v| v.is_finite()))
                .take(MAX_GUIDES)
                .map(|g| [g[0], g[1], g[2], g[3]])
                .collect(),
            _ => Vec::new(),
        };
        let solved = match x.get("solved") {
            Some(Value::Floats(v)) if v.len() == 3 && v.iter().all(|v| v.is_finite()) => {
                Some([v[0], v[1], v[2]].map(|a| a.clamp(-45.0, 45.0)))
            }
            _ => None,
        };
        Self {
            mode: x.choice_or("mode", d.mode),
            vertical: c("vertical", 0.0, -100.0, 100.0),
            horizontal: c("horizontal", 0.0, -100.0, 100.0),
            rotate: c("rotate", 0.0, -10.0, 10.0),
            aspect: c("aspect", 0.0, -100.0, 100.0),
            scale: c("scale", 100.0, 50.0, 150.0),
            offset_x: c("offset_x", 0.0, -100.0, 100.0),
            offset_y: c("offset_y", 0.0, -100.0, 100.0),
            focal_mm35: c("focal_mm35", 0.0, 0.0, 2000.0),
            guides,
            auto_scale: x.bool_or("auto_scale", d.auto_scale),
            solved,
        }
    }
}

impl PerspParams {
    /// Nothing to do: no angles, no scale or shift (a drawn guide alone changes nothing
    /// until the mode is guided).
    pub fn is_neutral(&self) -> bool {
        let solved = self.mode.is_automatic() && self.solved.is_some_and(|s| s != [0.0; 3]);
        let guided = self.mode == Mode::Guided && !self.guides.is_empty();
        !solved
            && !guided
            && self.vertical == 0.0
            && self.horizontal == 0.0
            && self.rotate == 0.0
            && self.aspect == 0.0
            && self.scale == 100.0
            && self.offset_x == 0.0
            && self.offset_y == 0.0
    }

    /// Pitch, yaw and roll (radians) the sliders give.
    pub fn slider_angles(&self) -> [f64; 3] {
        let k = MAX_KEYSTONE_DEG / 100.0;
        [
            (f64::from(self.vertical) * k).to_radians(),
            (f64::from(self.horizontal) * k).to_radians(),
            f64::from(self.rotate).to_radians(),
        ]
    }
}

/// The edit's perspective params: `None` when the module is missing or off.
pub fn persp_params(edit: &Edit) -> Option<PerspParams> {
    edit.modules
        .iter()
        .find(|m| m.kind == ModuleKind::PERSPECTIVE && m.enabled)
        .map(|m| PerspParams::from_fields(&m.params.to_fields()))
}

/// The focal length in pixels of a `w × h` oriented frame: the module's 35 mm value, else
/// the file's, else [`DEFAULT_FOCAL_MM35`].
pub fn focal_px(p: &PerspParams, file_mm35: Option<f64>, w: f64, h: f64) -> f64 {
    let f35 = if p.focal_mm35 > 0.0 {
        f64::from(p.focal_mm35)
    } else {
        file_mm35
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(DEFAULT_FOCAL_MM35)
    };
    f35 / FULL_FRAME_DIAGONAL_MM * w.hypot(h)
}

/// A file's `FocalLengthIn35mmFilm` (Exif 0xA405), from its head.
pub fn focal35_of_file(data: &[u8]) -> Option<f64> {
    let exif = specular_convert::source_meta(data).exif?;
    let d = exif
        .exif
        .iter()
        .chain(&exif.ifd0)
        .find(|e| e.tag == 0xA405)
        .map(|e| e.data.as_slice())?;
    let v = f64::from(u16::from_le_bytes([*d.first()?, *d.get(1)?]));
    (v > 0.0).then_some(v)
}

/// The 35 mm equivalent of `f` pixels on a `w × h` frame.
pub fn focal_mm35(f: f64, w: f64, h: f64) -> f64 {
    f * FULL_FRAME_DIAGONAL_MM / w.hypot(h).max(1e-9)
}

pub fn identity() -> M3 {
    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
}

pub fn mul(a: &M3, b: &M3) -> M3 {
    crate::colour::mul(a, b)
}

pub fn inverse(m: &M3) -> Option<M3> {
    crate::colour::inv(m)
}

/// `m` applied to point `p`; `None` on or beyond the horizon (`w ≤ 0`).
pub fn apply(m: &M3, p: (f64, f64)) -> Option<(f64, f64)> {
    let z = m[2][0] * p.0 + m[2][1] * p.1 + m[2][2];
    if z.is_nan() || z <= 1e-12 {
        return None;
    }
    Some((
        (m[0][0] * p.0 + m[0][1] * p.1 + m[0][2]) / z,
        (m[1][0] * p.0 + m[1][1] * p.1 + m[1][2]) / z,
    ))
}

/// The rotation of the camera: yaw about y, then pitch about x, then roll about z.
pub fn rotation(pitch: f64, yaw: f64, roll: f64) -> M3 {
    let (sp, cp) = pitch.sin_cos();
    let (sy, cy) = yaw.sin_cos();
    let (sr, cr) = roll.sin_cos();
    let rx = [[1.0, 0.0, 0.0], [0.0, cp, -sp], [0.0, sp, cp]];
    let ry = [[cy, 0.0, sy], [0.0, 1.0, 0.0], [-sy, 0.0, cy]];
    let rz = [[cr, -sr, 0.0], [sr, cr, 0.0], [0.0, 0.0, 1.0]];
    mul(&rz, &mul(&rx, &ry))
}

/// A turn by `angle` (radians, clockwise on screen) about the origin.
pub fn turn(angle: f64) -> M3 {
    let (s, c) = angle.sin_cos();
    [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]]
}

/// `K · R · K⁻¹` for focal length `f` (pixels), moved so the origin stays put.
pub fn keystone(f: f64, pitch: f64, yaw: f64, roll: f64) -> M3 {
    let r = rotation(pitch, yaw, roll);
    let h = [
        [r[0][0], r[0][1], f * r[0][2]],
        [r[1][0], r[1][1], f * r[1][2]],
        [r[2][0] / f, r[2][1] / f, r[2][2]],
    ];
    // Where the frame centre went (r[2][2] > 0 for angles under 90°).
    let (cx, cy) = (f * r[0][2] / r[2][2], f * r[1][2] / r[2][2]);
    mul(&[[1.0, 0.0, -cx], [0.0, 1.0, -cy], [0.0, 0.0, 1.0]], &h)
}

/// The least distance (pixels) from the corners of the centred `w × h` frame to the horizon
/// line of the camera turned by `pitch` and `yaw`; infinite without one.
pub fn horizon_distance(f: f64, w: f64, h: f64, pitch: f64, yaw: f64) -> f64 {
    let r = rotation(pitch, yaw, 0.0);
    let (a, b, c) = (r[2][0], r[2][1], f * r[2][2]);
    let n = a.hypot(b);
    if n < 1e-12 {
        return f64::INFINITY;
    }
    [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .iter()
        .map(|&(sx, sy)| (a * sx * w / 2.0 + b * sy * h / 2.0 + c) / n)
        .fold(f64::INFINITY, f64::min)
}

/// `pitch` and `yaw` scaled back together until the horizon is [`HORIZON_MARGIN`] of the
/// diagonal outside the frame.
pub fn clamp_angles(f: f64, w: f64, h: f64, pitch: f64, yaw: f64) -> (f64, f64) {
    let need = HORIZON_MARGIN * w.hypot(h);
    let ok = |t: f64| horizon_distance(f, w, h, pitch * t, yaw * t) >= need;
    if ok(1.0) {
        return (pitch, yaw);
    }
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..50 {
        let m = 0.5 * (lo + hi);
        if ok(m) {
            lo = m;
        } else {
            hi = m;
        }
    }
    (pitch * lo, yaw * lo)
}

/// The scale and shift after the keystone: `aspect`, `scale` (times `zoom`) and offsets.
fn placement(p: &PerspParams, w: f64, h: f64, zoom: f64) -> M3 {
    let r = ASPECT_AT_100.powf(f64::from(p.aspect) / 200.0);
    let s = f64::from(p.scale) / 100.0 * zoom;
    let (tx, ty) = (
        f64::from(p.offset_x) / 100.0 * w / 4.0,
        f64::from(p.offset_y) / 100.0 * h / 4.0,
    );
    [[s * r, 0.0, tx], [0.0, s / r, ty], [0.0, 0.0, 1.0]]
}

/// Whether the centred `w × h` frame, mapped back through `fwd`, lies inside the oriented
/// frame (the region is convex: the corners decide).
pub fn covers(fwd: &M3, w: f64, h: f64) -> bool {
    let Some(back) = inverse(fwd) else {
        return false;
    };
    [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .iter()
        .all(|&(sx, sy)| {
            apply(&back, (sx * w / 2.0, sy * h / 2.0)).is_some_and(|(x, y)| {
                x.abs() <= w / 2.0 * (1.0 + 1e-9) && y.abs() <= h / 2.0 * (1.0 + 1e-9)
            })
        })
}

/// The least zoom (about the frame centre, keeping the user's scale out of it) at which the
/// keystoned frame covers the whole `w × h` frame; 1 when none does.
pub fn fill_zoom(p: &PerspParams, h_c: &M3, w: f64, h: f64) -> f64 {
    let unscaled = PerspParams {
        scale: 100.0,
        offset_x: 0.0,
        offset_y: 0.0,
        ..p.clone()
    };
    let at = |z: f64| covers(&mul(&placement(&unscaled, w, h, z), h_c), w, h);
    let (mut lo, mut hi) = (0.05, 40.0);
    if !at(hi) {
        return 1.0;
    }
    if at(lo) {
        return lo;
    }
    for _ in 0..60 {
        let m = (lo * hi).sqrt();
        if at(m) {
            hi = m;
        } else {
            lo = m;
        }
    }
    hi
}

/// The forward map oriented frame → frame before straightening (both centred, square
/// pixels) for camera angles `angles` (pitch, yaw, roll; radians) on a `w × h` frame with
/// focal length `f` (pixels): keystone, then placement.
pub fn forward(p: &PerspParams, angles: [f64; 3], f: f64, w: f64, h: f64) -> M3 {
    let (pitch, yaw) = clamp_angles(f, w, h, angles[0], angles[1]);
    let h_c = keystone(f, pitch, yaw, angles[2]);
    let zoom = if p.auto_scale {
        fill_zoom(p, &h_c, w, h)
    } else {
        1.0
    };
    mul(&placement(p, w, h, zoom), &h_c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_round_trip_and_stay_lean() {
        let d = PerspParams::default();
        let f = d.to_fields();
        assert!(f.get("guides").is_none() && f.get("solved").is_none());
        assert!(f.get("focal_mm35").is_none());
        assert_eq!(PerspParams::from_fields(&f), d);
        let p = PerspParams {
            mode: Mode::Guided,
            vertical: 20.0,
            focal_mm35: 35.0,
            guides: vec![[0.1, 0.2, 0.1, 0.8], [0.9, 0.2, 0.85, 0.8]],
            solved: Some([1.0, -2.0, 0.5]),
            ..d
        };
        assert_eq!(PerspParams::from_fields(&p.to_fields()), p);
        assert!(PerspParams::default().is_neutral());
        assert!(!p.is_neutral());
    }

    #[test]
    fn keystone_keeps_the_centre_and_widens_the_top() {
        let f = 3000.0;
        let h = keystone(f, 10f64.to_radians(), 0.0, 0.0);
        let c = apply(&h, (0.0, 0.0)).unwrap();
        assert!(c.0.abs() < 1e-9 && c.1.abs() < 1e-9);
        let tl = apply(&h, (-1000.0, -1000.0)).unwrap();
        let tr = apply(&h, (1000.0, -1000.0)).unwrap();
        let bl = apply(&h, (-1000.0, 1000.0)).unwrap();
        let br = apply(&h, (1000.0, 1000.0)).unwrap();
        assert!(tr.0 - tl.0 > br.0 - bl.0, "{tl:?} {tr:?} {bl:?} {br:?}");
        let h = keystone(f, 0.0, 10f64.to_radians(), 0.0);
        let tr = apply(&h, (1000.0, -1000.0)).unwrap();
        let br = apply(&h, (1000.0, 1000.0)).unwrap();
        let tl = apply(&h, (-1000.0, -1000.0)).unwrap();
        let bl = apply(&h, (-1000.0, 1000.0)).unwrap();
        assert!(br.1 - tr.1 > bl.1 - tl.1);
    }

    #[test]
    fn the_clamp_keeps_the_horizon_away() {
        let (w, h) = (6000.0, 4000.0);
        let f = 12.0 / FULL_FRAME_DIAGONAL_MM * f64::hypot(w, h);
        let (p, y) = clamp_angles(f, w, h, 30f64.to_radians(), 30f64.to_radians());
        assert!(p < 30f64.to_radians() && (p - y).abs() < 1e-12);
        let d = horizon_distance(f, w, h, p, y);
        assert!((d - 0.1 * w.hypot(h)).abs() < 1.0, "{d}");
        // A long lens keeps the slider's angle.
        let f = 50.0 / FULL_FRAME_DIAGONAL_MM * f64::hypot(w, h);
        let a = 30f64.to_radians();
        assert_eq!(clamp_angles(f, w, h, a, 0.0), (a, 0.0));
    }

    #[test]
    fn the_fill_zoom_covers_the_frame_and_no_more() {
        let (w, h) = (3000.0, 2000.0);
        let p = PerspParams::default();
        let f = focal_px(&p, None, w, h);
        let h_c = keystone(f, 15f64.to_radians(), -6f64.to_radians(), 0.0);
        let z = fill_zoom(&p, &h_c, w, h);
        let fwd = |z: f64| mul(&placement(&p, w, h, z), &h_c);
        assert!(covers(&fwd(z), w, h));
        assert!(!covers(&fwd(z * 0.999), w, h));
    }
}
