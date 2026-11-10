//! The geometry of the three MPR views: each a plane through the shared crosshair point,
//! with its own in-plane directions, zoom and pan, and the maps between a view's screen
//! rectangle (logical pixels) and patient coordinates (mm, LPS).
//!
//! The views start axial (screen right = patient left +x, down = posterior +y), coronal
//! (right +x, down = inferior −z) and sagittal (right +y, down −z), radiological convention.
//! Rotating in one view turns the other two about its normal, so the three planes stay
//! mutually perpendicular (oblique MPR) and the rotated view itself stays put.

use specular_dicom::volume::Geometry;
use specular_render::Window;
use specular_render_volume::{Plane, Slab};
use specular_theme::Color;

pub(crate) type V3 = [f64; 3];

pub(crate) fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(crate) fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(crate) fn add(a: V3, b: V3, k: f64) -> V3 {
    [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k]
}

pub(crate) fn sub(a: V3, b: V3) -> V3 {
    add(a, b, -1.0)
}

pub(crate) fn unit(v: V3) -> V3 {
    let l = dot(v, v).sqrt();
    if l > 1e-12 { v.map(|c| c / l) } else { v }
}

/// `v` turned by `angle` radians about the unit `axis` (right-handed).
pub(crate) fn turn(v: V3, axis: V3, angle: f64) -> V3 {
    let (s, c) = angle.sin_cos();
    let k = cross(axis, v);
    let d = dot(axis, v) * (1.0 - c);
    [0, 1, 2].map(|i| v[i] * c + k[i] * s + axis[i] * d)
}

/// A screen rectangle in logical pixels: x, y, width, height.
pub(crate) type R = [f64; 4];

/// The three views, in their slot order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Axial,
    Coronal,
    Sagittal,
}

pub(crate) const KINDS: [Kind; 3] = [Kind::Axial, Kind::Coronal, Kind::Sagittal];

impl Kind {
    /// Screen right and down in patient coordinates when not rotated.
    fn base(self) -> (V3, V3) {
        match self {
            Kind::Axial => ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            Kind::Coronal => ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
            Kind::Sagittal => ([0.0, 1.0, 0.0], [0.0, 0.0, -1.0]),
        }
    }

    pub(crate) fn message_id(self) -> &'static str {
        match self {
            Kind::Axial => "mpr-axial",
            Kind::Coronal => "mpr-coronal",
            Kind::Sagittal => "mpr-sagittal",
        }
    }

    /// The view's name without patient orientation: the volume's axes it shows.
    pub(crate) fn neutral_id(self) -> &'static str {
        match self {
            Kind::Axial => "mpr-xy",
            Kind::Coronal => "mpr-xz",
            Kind::Sagittal => "mpr-yz",
        }
    }

    /// The view's colour: its crosshair line in the other views and its name's marker
    /// (red axial, green coronal, yellow sagittal, as is usual in MPR viewers).
    pub(crate) fn colour(self) -> Color {
        match self {
            Kind::Axial => Color::rgba(0.95, 0.36, 0.33, 1.0),
            Kind::Coronal => Color::rgba(0.38, 0.82, 0.42, 1.0),
            Kind::Sagittal => Color::rgba(0.96, 0.82, 0.26, 1.0),
        }
    }
}

/// One view's frame: unit screen directions in patient space, zoom (1 fits the volume) and
/// pan (mm of the rectangle's centre from the volume's centre, along right and down).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Frame {
    pub right: V3,
    pub down: V3,
    pub zoom: f64,
    pub pan: [f64; 2],
}

/// Logical pixels from a line's end handle that still grab it.
pub(crate) const HANDLE_HIT: f64 = 12.0;
/// Slab thicknesses offered (mm).
pub(crate) const THICKNESSES: [f64; 10] = [1.0, 2.0, 3.0, 5.0, 10.0, 15.0, 20.0, 30.0, 50.0, 80.0];
/// The thickness a slab starts at.
const DEFAULT_SLAB: f64 = 10.0;

/// What a press in a view grabs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Grab {
    /// The crosshair: it follows the pointer.
    Move,
    /// A line's end handle: turning it rotates the other views; the pointer's screen angle
    /// about the crosshair when grabbed.
    Rotate(f64),
}

/// A crosshair line in a view: where the `other` view's plane cuts it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Line {
    pub other: usize,
    /// The crosshair on screen, and the line's unit screen direction.
    pub at: (f64, f64),
    pub dir: (f64, f64),
}

/// The views' shared state.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Views {
    pub frames: [Frame; 3],
    /// The crosshair: the point all three planes pass through.
    pub c: V3,
    /// The volume's centre and corners.
    pub vc: V3,
    pub corners: [V3; 8],
    /// The smallest voxel edge (mm): one wheel notch.
    pub step: f64,
    /// Slab thickness (0: a thin plane) and how its samples combine.
    pub thickness: f64,
    pub slab: Slab,
    /// Whose anatomy the orientation letters name (a quadruped's Le/Rt, D/V, Cr/Cd).
    pub anatomy: specular_dicom::Anatomy,
}

impl Views {
    pub(crate) fn new(g: &Geometry) -> Views {
        let v = g.voxel_mm();
        let mut views = Views {
            frames: [Frame {
                right: [1.0, 0.0, 0.0],
                down: [0.0, 1.0, 0.0],
                zoom: 1.0,
                pan: [0.0; 2],
            }; 3],
            c: g.centre(),
            vc: g.centre(),
            corners: g.corners(),
            step: v[0].min(v[1]).min(v[2]).max(1e-3),
            thickness: 0.0,
            slab: Slab::Mip,
            anatomy: specular_dicom::Anatomy::Biped,
        };
        views.reset();
        views
    }

    /// Every view back to its plane through the volume's centre, fitted. The slab stays.
    pub(crate) fn reset(&mut self) {
        for (f, k) in self.frames.iter_mut().zip(KINDS) {
            let (right, down) = k.base();
            *f = Frame {
                right,
                down,
                zoom: 1.0,
                pan: [0.0; 2],
            };
        }
        self.c = self.vc;
    }

    pub(crate) fn normal(&self, v: usize) -> V3 {
        let f = &self.frames[v];
        unit(cross(f.right, f.down))
    }

    /// The view is turned away from its anatomical plane.
    pub(crate) fn oblique(&self, v: usize) -> bool {
        let (r, d) = KINDS[v].base();
        dot(self.normal(v), unit(cross(r, d))) < 1.0 - 1e-9
    }

    /// The plane's position (mm): the crosshair's distance along the view's normal, signed
    /// as the patient axis the view steps along (axial +z, coronal +y, sagittal +x, as the
    /// DICOM coordinates read; an oblique view keeps its anatomical plane's sign).
    pub(crate) fn position(&self, v: usize) -> f64 {
        let n = self.normal(v);
        let axis = [2, 1, 0][v];
        let d = dot(self.c, n);
        if n[axis] < 0.0 { -d } else { d }
    }

    /// Patient mm per logical pixel: the whole volume fits the rectangle at zoom 1 (in the
    /// view's unrotated frame, so rotating does not rescale).
    pub(crate) fn mm(&self, v: usize, r: R) -> f64 {
        let (right, down) = KINDS[v].base();
        let mut fit = 1e-3f64;
        for k in &self.corners {
            let d = sub(*k, self.vc);
            fit = fit
                .max(2.0 * dot(d, right).abs() / r[2].max(1.0))
                .max(2.0 * dot(d, down).abs() / r[3].max(1.0));
        }
        fit * 1.04 / self.frames[v].zoom
    }

    /// The patient point at the rectangle's centre.
    fn centre(&self, v: usize) -> V3 {
        let f = &self.frames[v];
        let n = self.normal(v);
        let p = add(self.vc, n, dot(sub(self.c, self.vc), n));
        add(add(p, f.right, f.pan[0]), f.down, f.pan[1])
    }

    pub(crate) fn to_screen(&self, v: usize, r: R, p: V3) -> (f64, f64) {
        let f = &self.frames[v];
        let mm = self.mm(v, r);
        let d = sub(p, self.centre(v));
        (
            r[0] + r[2] / 2.0 + dot(d, f.right) / mm,
            r[1] + r[3] / 2.0 + dot(d, f.down) / mm,
        )
    }

    /// The point of the view's plane under screen point `p`.
    pub(crate) fn to_patient(&self, v: usize, r: R, p: (f64, f64)) -> V3 {
        let f = &self.frames[v];
        let mm = self.mm(v, r);
        let c = self.centre(v);
        let x = (p.0 - r[0] - r[2] / 2.0) * mm;
        let y = (p.1 - r[1] - r[3] / 2.0) * mm;
        add(add(c, f.right, x), f.down, y)
    }

    /// The view as a plane for the GPU: `r` in logical pixels, `scale` physical pixels per
    /// logical pixel (the rectangle is expected on whole physical pixels).
    pub(crate) fn plane(&self, v: usize, r: R, scale: f64, window: Window) -> Plane {
        let f = &self.frames[v];
        let px = [
            (r[0] * scale).round().max(0.0) as u32,
            (r[1] * scale).round().max(0.0) as u32,
            (r[2] * scale).round().max(0.0) as u32,
            (r[3] * scale).round().max(0.0) as u32,
        ];
        let mut p = Plane::centred(
            px,
            self.centre(v),
            f.right,
            f.down,
            self.mm(v, r) / scale,
            window,
        );
        p.thickness = self.thickness;
        p.slab = if self.thickness > 0.0 {
            self.slab
        } else {
            Slab::Average
        };
        p
    }

    /// The crosshair lines in view `v`: where the two other planes cut it.
    pub(crate) fn lines(&self, v: usize, r: R) -> [Line; 2] {
        let f = &self.frames[v];
        let n = self.normal(v);
        let at = self.to_screen(v, r, self.c);
        let others = [(v + 1) % 3, (v + 2) % 3];
        others.map(|o| {
            let d = cross(n, self.normal(o));
            let (x, y) = (dot(d, f.right), dot(d, f.down));
            let l = x.hypot(y).max(1e-12);
            Line {
                other: o,
                at,
                dir: (x / l, y / l),
            }
        })
    }

    /// Distance of the end handles from the crosshair (logical pixels).
    pub(crate) fn handle_radius(r: R) -> f64 {
        0.4 * r[2].min(r[3])
    }

    /// The end handles of the lines in view `v`.
    pub(crate) fn handles(&self, v: usize, r: R) -> Vec<(f64, f64)> {
        let k = Self::handle_radius(r);
        self.lines(v, r)
            .iter()
            .flat_map(|l| [1.0, -1.0].map(|s| (l.at.0 + s * k * l.dir.0, l.at.1 + s * k * l.dir.1)))
            .collect()
    }

    /// What a press at `p` in view `v` grabs; `hit` the handles' reach in logical pixels.
    pub(crate) fn grab(&self, v: usize, r: R, p: (f64, f64), hit: f64) -> Grab {
        let near = self
            .handles(v, r)
            .iter()
            .any(|h| (h.0 - p.0).hypot(h.1 - p.1) <= hit);
        if near {
            let at = self.to_screen(v, r, self.c);
            Grab::Rotate((p.1 - at.1).atan2(p.0 - at.0))
        } else {
            Grab::Move
        }
    }

    /// The screen angle of `p` about the crosshair in view `v`.
    pub(crate) fn angle(&self, v: usize, r: R, p: (f64, f64)) -> f64 {
        let at = self.to_screen(v, r, self.c);
        (p.1 - at.1).atan2(p.0 - at.0)
    }

    /// Turns the other two views about view `v`'s normal by `angle` radians (clockwise on
    /// screen, as the lines turn).
    pub(crate) fn rotate(&mut self, v: usize, angle: f64) {
        let n = self.normal(v);
        for o in [(v + 1) % 3, (v + 2) % 3] {
            let f = &mut self.frames[o];
            f.right = unit(turn(f.right, n, angle));
            f.down = unit(turn(f.down, n, angle));
        }
    }

    /// Moves the crosshair to the plane point under `p` in view `v`.
    pub(crate) fn move_to(&mut self, v: usize, r: R, p: (f64, f64)) {
        let q = self.to_patient(v, r, p);
        self.c = self.clamped(q);
    }

    /// `p` with each view's position kept within the volume's extent along its normal.
    fn clamped(&self, mut p: V3) -> V3 {
        for v in 0..3 {
            let n = self.normal(v);
            let (lo, hi) = self.extent(n);
            let a = dot(p, n);
            p = add(p, n, a.clamp(lo, hi) - a);
        }
        p
    }

    fn extent(&self, n: V3) -> (f64, f64) {
        self.corners
            .iter()
            .map(|k| dot(*k, n))
            .fold((f64::MAX, f64::MIN), |(lo, hi), a| (lo.min(a), hi.max(a)))
    }

    /// Steps view `v`'s plane `steps` voxels along its normal (within the volume).
    pub(crate) fn scroll(&mut self, v: usize, steps: f64) {
        // Forward is up the position the readout shows.
        let mut n = self.normal(v);
        if n[[2, 1, 0][v]] < 0.0 {
            n = n.map(|x| -x);
        }
        let q = add(self.c, n, steps * self.step);
        self.c = self.clamped(q);
    }

    /// Zooms view `v` by `factor` keeping the point under `p` in place.
    pub(crate) fn zoom_at(&mut self, v: usize, r: R, factor: f64, p: (f64, f64)) {
        let q = self.to_patient(v, r, p);
        let f = &mut self.frames[v];
        f.zoom = (f.zoom * factor).clamp(0.25, 64.0);
        let q2 = self.to_patient(v, r, p);
        let d = sub(q, q2);
        let f = &mut self.frames[v];
        f.pan[0] += dot(d, f.right);
        f.pan[1] += dot(d, f.down);
    }

    /// Pans view `v` by a drag of (`dx`, `dy`) logical pixels.
    pub(crate) fn pan_by(&mut self, v: usize, r: R, dx: f64, dy: f64) {
        let mm = self.mm(v, r);
        let f = &mut self.frames[v];
        f.pan[0] -= dx * mm;
        f.pan[1] -= dy * mm;
    }

    /// The end of a measurement in `measures` drawn within `radius` (logical px) of `p` in
    /// view `v` (rectangle `r`), as (measurement, end): the nearest. Only measurements drawn
    /// in the view (both ends on its plane) can be grabbed.
    pub(crate) fn measure_end_at(
        &self,
        v: usize,
        r: R,
        measures: &[[V3; 2]],
        p: (f64, f64),
        radius: f64,
    ) -> Option<(usize, usize)> {
        let mut best: Option<(f64, (usize, usize))> = None;
        for (i, m) in measures.iter().enumerate() {
            if !(self.on_plane(v, m[0]) && self.on_plane(v, m[1])) {
                continue;
            }
            for (e, &q) in m.iter().enumerate() {
                let s = self.to_screen(v, r, q);
                let d = (s.0 - p.0).hypot(s.1 - p.1);
                if d <= radius && best.is_none_or(|(b, _)| d < b) {
                    best = Some((d, (i, e)));
                }
            }
        }
        best.map(|(_, at)| at)
    }

    /// Whether patient point `p` lies on view `v`'s plane (within half a voxel).
    pub(crate) fn on_plane(&self, v: usize, p: V3) -> bool {
        (dot(sub(p, self.c), self.normal(v))).abs() <= self.step / 2.0 + 1e-6
    }

    /// The next slab mode: thin, MIP, MinIP, average, thin.
    pub(crate) fn next_slab(&mut self) {
        let (t, s) = match (self.thickness > 0.0, self.slab) {
            (false, _) => (DEFAULT_SLAB, Slab::Mip),
            (true, Slab::Mip) => (self.thickness, Slab::MinIp),
            (true, Slab::MinIp) => (self.thickness, Slab::Average),
            (true, Slab::Average) => (0.0, self.slab),
        };
        self.thickness = t;
        self.slab = s;
    }

    /// Picks a slab mode (`None`: thin).
    pub(crate) fn set_slab(&mut self, s: Option<Slab>) {
        match s {
            None => self.thickness = 0.0,
            Some(s) => {
                self.slab = s;
                if self.thickness <= 0.0 {
                    self.thickness = DEFAULT_SLAB;
                }
            }
        }
    }

    /// The next thickness up or down the list (down from the thinnest: a thin plane).
    pub(crate) fn step_thickness(&mut self, thicker: bool) {
        let t = self.thickness;
        self.thickness = if thicker {
            THICKNESSES
                .iter()
                .copied()
                .find(|&x| x > t + 1e-9)
                .unwrap_or(THICKNESSES[THICKNESSES.len() - 1])
        } else {
            THICKNESSES
                .iter()
                .rev()
                .copied()
                .find(|&x| x < t - 1e-9)
                .unwrap_or(0.0)
        };
    }
}
