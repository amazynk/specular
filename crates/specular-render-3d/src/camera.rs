//! The orbit camera, with the MPR 3D view's conventions: unit right and down vectors (it
//! looks along `right × down`), a turntable orbit about the up axis (+z of the canonical
//! frame, see [`specular_mesh::preview::canonical`]), a zoom over the fitted size and a pan
//! in model units along right and down.

use std::f64::consts::PI;

use specular_mesh::Bounds;

pub type V3 = [f64; 3];

/// Radians per logical pixel dragged (as in the MPR 3D view).
pub const ORBIT_PER_PX: f64 = 0.01;
/// Radians per orbit key press (as in the MPR 3D view).
pub const ORBIT_KEY: f64 = PI / 12.0;
/// The zoom factor of one wheel notch or zoom key.
pub const ZOOM_STEP: f64 = 1.25;
/// The vertical field of view in perspective.
pub const FOV_Y: f64 = 35.0 * PI / 180.0;
/// The share of the view a fitted model fills.
const FIT: f64 = 0.88;
const ZOOM_MIN: f64 = 0.02;
const ZOOM_MAX: f64 = 2000.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Projection {
    #[default]
    Perspective,
    Orthographic,
}

impl Projection {
    pub fn id(self) -> &'static str {
        match self {
            Projection::Perspective => "perspective",
            Projection::Orthographic => "orthographic",
        }
    }

    pub fn toggled(self) -> Projection {
        match self {
            Projection::Perspective => Projection::Orthographic,
            Projection::Orthographic => Projection::Perspective,
        }
    }
}

/// How the surface draws.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Shading {
    #[default]
    Shaded,
    /// Shaded with the triangle edges drawn over it.
    Edges,
    /// Every edge, front and back, and no surface.
    Wireframe,
    /// See-through: surfaces add up, brightest where they are seen edge-on.
    XRay,
}

impl Shading {
    pub const ALL: [Shading; 4] = [
        Shading::Shaded,
        Shading::Edges,
        Shading::Wireframe,
        Shading::XRay,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Shading::Shaded => "shaded",
            Shading::Edges => "edges",
            Shading::Wireframe => "wireframe",
            Shading::XRay => "xray",
        }
    }

    pub fn next(self) -> Shading {
        let i = Shading::ALL.iter().position(|s| *s == self).unwrap_or(0);
        Shading::ALL[(i + 1) % Shading::ALL.len()]
    }
}

/// The standard views.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StandardView {
    /// From −y, z up.
    Front,
    /// From +z, y away.
    Top,
    /// From +x (the right side), z up.
    Side,
    /// From the front, right and above.
    Iso,
}

impl StandardView {
    pub const ALL: [StandardView; 4] = [
        StandardView::Front,
        StandardView::Top,
        StandardView::Side,
        StandardView::Iso,
    ];

    pub fn id(self) -> &'static str {
        match self {
            StandardView::Front => "front",
            StandardView::Top => "top",
            StandardView::Side => "side",
            StandardView::Iso => "iso",
        }
    }

    /// The right and down vectors.
    pub fn basis(self) -> (V3, V3) {
        match self {
            StandardView::Front => ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
            StandardView::Top => ([1.0, 0.0, 0.0], [0.0, -1.0, 0.0]),
            StandardView::Side => ([0.0, 1.0, 0.0], [0.0, 0.0, -1.0]),
            StandardView::Iso => {
                let [r, d, _] = specular_mesh::preview::ISO;
                (r.map(f64::from), d.map(f64::from))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub right: V3,
    pub down: V3,
    /// Over the bounding sphere filling the shorter side.
    pub zoom: f64,
    /// Model units along right and down the model is moved by.
    pub pan: [f64; 2],
    pub projection: Projection,
    /// First person (walking, flying, a viewpoint inside the model): the eye is here, in
    /// perspective, looking along `right × down`; `pan` is unused and `zoom` sets only the
    /// scale on the focal plane a model radius away. `None`: the orbit camera.
    pub eye: Option<V3>,
    /// The vertical field of view in perspective (radians).
    pub fov: f64,
}

/// Where a camera is, independent of the view's size: what viewpoints store.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub eye: V3,
    /// Unit view direction and up vector (orthogonal to it).
    pub direction: V3,
    pub up: V3,
    pub projection: Projection,
    /// Vertical field of view (radians), in perspective.
    pub fov: f64,
    /// The height the view shows (model units), in orthographic.
    pub height: f64,
    /// How far ahead the orbit centre is (model units): the eye's distance to the target.
    pub distance: f64,
}

impl Default for Camera {
    /// The iso view, unfitted.
    fn default() -> Camera {
        let (right, down) = StandardView::Iso.basis();
        Camera {
            right,
            down,
            zoom: 1.0,
            pan: [0.0; 2],
            projection: Projection::default(),
            eye: None,
            fov: FOV_Y,
        }
    }
}

/// Where the camera is for one frame of a `w` × `h` px view.
#[derive(Clone, Copy, Debug)]
pub struct Projected {
    /// Rows of the world-to-clip matrix (reversed depth: 1 nearest).
    pub rows: [[f64; 4]; 4],
    /// The eye in perspective, `None` in orthographic.
    pub eye: Option<V3>,
    /// The point at the view's centre (on the focal plane in perspective).
    pub target: V3,
    /// Model units per px on the focal plane.
    pub units_per_px: f64,
    pub forward: V3,
}

impl Camera {
    /// Turns the object by `dx` radians about the up axis (dragging right brings its right
    /// side forward) and tips it by `dy` about the view's horizontal.
    pub fn orbit(&mut self, dx: f64, dy: f64) {
        if self.eye.is_some() {
            // First person: the drag turns the view about the eye.
            let (yaw, pitch) = self.yaw_pitch();
            let lim = PI / 2.0 - 1e-3;
            let next = Camera::first_person(
                self.eye.unwrap_or_default(),
                yaw - dx,
                (pitch - dy).clamp(-lim, lim),
                self.fov,
            );
            (self.right, self.down) = (next.right, next.down);
            return;
        }
        let z = [0.0, 0.0, 1.0];
        let right = turn(self.right, z, -dx);
        let down = turn(self.down, z, -dx);
        let down = turn(down, right, -dy);
        self.right = unit(right);
        self.down = unit(sub(down, scale(right, dot(down, right))));
    }

    pub fn zoom_by(&mut self, f: f64) {
        self.zoom = (self.zoom * f).clamp(ZOOM_MIN, ZOOM_MAX);
    }

    /// Pans by a drag of `dx`, `dy` px at `units_per_px`.
    pub fn pan_px(&mut self, dx: f64, dy: f64, units_per_px: f64) {
        self.pan[0] += dx * units_per_px;
        self.pan[1] += dy * units_per_px;
    }

    pub fn forward(&self) -> V3 {
        unit(cross(self.right, self.down))
    }

    /// A first-person camera at `eye` turned `yaw` radians from +x about +z (counter-
    /// clockwise from above) and `pitch` up from the horizontal, in perspective.
    pub fn first_person(eye: V3, yaw: f64, pitch: f64, fov: f64) -> Camera {
        let (sp, cp) = pitch.sin_cos();
        let (sy, cy) = yaw.sin_cos();
        let f = [cp * cy, cp * sy, sp];
        let right = unit([sy, -cy, 0.0]);
        let down = unit(cross(f, right));
        Camera {
            right,
            down,
            zoom: 1.0,
            pan: [0.0; 2],
            projection: Projection::Perspective,
            eye: Some(eye),
            fov,
        }
    }

    /// The forward direction's yaw (from +x about +z) and pitch (up from the horizontal).
    pub fn yaw_pitch(&self) -> (f64, f64) {
        let f = self.forward();
        let pitch = f[2].clamp(-1.0, 1.0).asin();
        let yaw = if f[0].abs() + f[1].abs() > 1e-9 {
            f[1].atan2(f[0])
        } else {
            // Straight up or down: the yaw of the view's right vector.
            (-self.right[0]).atan2(self.right[1])
        };
        (yaw, pitch)
    }

    /// Where the camera is for a `w` × `h` px view of `b`.
    pub fn pose(&self, b: &Bounds, w: f64, h: f64) -> Pose {
        let p = self.project(b, w, h, 0.0);
        let f = p.forward;
        let up = scale(self.down, -1.0);
        let height = p.units_per_px * h.max(1.0);
        match p.eye {
            Some(eye) => Pose {
                eye,
                direction: f,
                up,
                projection: Projection::Perspective,
                fov: self.fov,
                height,
                distance: len(sub(p.target, eye)),
            },
            None => {
                let back = 2.0 * radius(b) + len(sub(p.target, centre(b)));
                Pose {
                    eye: sub(p.target, scale(f, back)),
                    direction: f,
                    up,
                    projection: Projection::Orthographic,
                    fov: self.fov,
                    height,
                    distance: back,
                }
            }
        }
    }

    /// The camera at `pose` for a `w` × `h` px view of `b`: an orbit camera when the box's
    /// centre is ahead of the eye, else first person.
    pub fn from_pose(pose: &Pose, b: &Bounds, w: f64, h: f64) -> Camera {
        let f = unit(pose.direction);
        let mut up = sub(pose.up, scale(f, dot(pose.up, f)));
        if len(up) < 1e-9 {
            // Up along the view: any perpendicular.
            up = if f[2].abs() < 0.9 {
                [0.0, 0.0, 1.0]
            } else {
                [0.0, 1.0, 0.0]
            };
            up = sub(up, scale(f, dot(up, f)));
        }
        let down = scale(unit(up), -1.0);
        let right = unit(cross(down, f));
        let (w, h) = (w.max(1.0), h.max(1.0));
        let fov = if pose.fov.is_finite() && pose.fov > 1e-3 && pose.fov < PI - 1e-3 {
            pose.fov
        } else {
            FOV_Y
        };
        let mut cam = Camera {
            right,
            down,
            zoom: 1.0,
            pan: [0.0; 2],
            projection: pose.projection,
            eye: None,
            fov,
        };
        let c = centre(b);
        let base = 2.0 * radius(b) / w.min(h);
        let ahead = dot(sub(c, pose.eye), f);
        match pose.projection {
            Projection::Orthographic => {
                let upp = (pose.height / h).max(1e-12);
                let target = add(pose.eye, scale(f, ahead));
                let off = sub(c, target);
                cam.pan = [dot(off, right), dot(off, down)];
                cam.zoom = (base / upp).clamp(ZOOM_MIN, ZOOM_MAX);
            }
            Projection::Perspective if ahead > 1e-6 * radius(b) => {
                let target = add(pose.eye, scale(f, ahead));
                let off = sub(c, target);
                cam.pan = [dot(off, right), dot(off, down)];
                // dist = upp·h / 2 / tan(fov/2) at the target.
                let upp = 2.0 * ahead * (fov / 2.0).tan() / h;
                let zoom = base / upp;
                if (ZOOM_MIN..=ZOOM_MAX).contains(&zoom) {
                    cam.zoom = zoom;
                } else {
                    cam.eye = Some(pose.eye);
                }
            }
            Projection::Perspective => cam.eye = Some(pose.eye),
        }
        cam
    }

    /// Back to an orbit camera from first person, where the box's centre allows (the view
    /// is unchanged).
    pub fn to_orbit(&mut self, b: &Bounds, w: f64, h: f64) {
        if self.eye.is_some() {
            let p = self.pose(b, w, h);
            *self = Camera::from_pose(&p, b, w, h);
        }
    }

    /// Looks from a standard view, fitted.
    pub fn look(&mut self, v: StandardView, b: &Bounds, w: f64, h: f64) {
        (self.right, self.down) = v.basis();
        self.fit(b, w, h);
    }

    /// Centres the box `b` and zooms so it fills the view.
    pub fn fit(&mut self, b: &Bounds, w: f64, h: f64) {
        self.eye = None;
        let c = centre(b);
        let (mut r0, mut r1, mut d0, mut d1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for corner in corners(b) {
            let p = sub(corner, c);
            let (r, d) = (dot(p, self.right), dot(p, self.down));
            (r0, r1, d0, d1) = (r0.min(r), r1.max(r), d0.min(d), d1.max(d));
        }
        self.pan = [-(r0 + r1) / 2.0, -(d0 + d1) / 2.0];
        let (w, h) = (w.max(1.0), h.max(1.0));
        let mut need = ((r1 - r0) / (FIT * w)).max((d1 - d0) / (FIT * h));
        if self.projection == Projection::Perspective {
            // The nearest eye distance that shows every corner (its offset from the centred
            // target over the half field of view, less its depth beyond the target).
            let ty = (self.fov / 2.0).tan() * FIT;
            let tx = ty * w / h;
            let (mr, md) = ((r0 + r1) / 2.0, (d0 + d1) / 2.0);
            let f = self.forward();
            let mut dist = 0f64;
            for corner in corners(b) {
                let p = sub(corner, c);
                let depth = dot(p, f);
                dist = dist
                    .max((dot(p, self.right) - mr).abs() / tx - depth)
                    .max((dot(p, self.down) - md).abs() / ty - depth)
                    .max(-depth * 1.05);
            }
            need = 2.0 * dist * (self.fov / 2.0).tan() / h;
        }
        let base = 2.0 * radius(b) / w.min(h);
        self.zoom = if need > 0.0 {
            (base / need).clamp(ZOOM_MIN, ZOOM_MAX)
        } else {
            1.0
        };
    }

    /// The ray through the point `x`, `y` px from the top left of a `w` × `h` px view of
    /// `b`: where it starts (the eye, or well in front of the box in orthographic) and its
    /// unit direction.
    pub fn ray(&self, b: &Bounds, x: f64, y: f64, w: f64, h: f64) -> (V3, V3) {
        let p = self.project(b, w, h, 0.0);
        let (w, h) = (w.max(1.0), h.max(1.0));
        let on_plane = add(
            p.target,
            add(
                scale(self.right, (x - w / 2.0) * p.units_per_px),
                scale(self.down, (y - h / 2.0) * p.units_per_px),
            ),
        );
        match p.eye {
            Some(eye) => (eye, unit(sub(on_plane, eye))),
            None => {
                let back = 4.0 * radius(b) + len(sub(p.target, centre(b)));
                (sub(on_plane, scale(p.forward, back)), p.forward)
            }
        }
    }

    /// Model units per px on the focal plane.
    pub fn units_per_px(&self, b: &Bounds, w: f64, h: f64) -> f64 {
        2.0 * radius(b) / w.min(h).max(1.0) / self.zoom
    }

    /// The camera for a `w` × `h` px view of `b`; `depth` is how far from the box's centre
    /// geometry may lie (at least its radius; more for the grid).
    pub fn project(&self, b: &Bounds, w: f64, h: f64, depth: f64) -> Projected {
        let (w, h) = (w.max(1.0), h.max(1.0));
        let c = centre(b);
        let (r, d) = (self.right, self.down);
        let f = self.forward();
        let upp = self.units_per_px(b, w, h);
        if let Some(eye) = self.eye {
            // First person: in perspective from the eye, the near plane close by (reversed
            // depth with an infinite far plane keeps the precision).
            let t = (self.fov / 2.0).tan();
            let dist = upp * h / 2.0 / t;
            let near = (radius(b) * 1e-4).max(1e-9);
            let ax = 1.0 / (t * w / h);
            return Projected {
                rows: [
                    row(scale(r, ax), -dot(eye, r) * ax),
                    row(scale(d, -1.0 / t), dot(eye, d) / t),
                    [0.0, 0.0, 0.0, near],
                    row(f, -dot(eye, f)),
                ],
                eye: Some(eye),
                target: add(eye, scale(f, dist)),
                units_per_px: upp,
                forward: f,
            };
        }
        let target = sub(sub(c, scale(r, self.pan[0])), scale(d, self.pan[1]));
        let reach = depth.max(radius(b)) + len(sub(target, c)) + 1e-9;
        match self.projection {
            Projection::Orthographic => {
                let sx = 2.0 / (upp * w);
                let sy = 2.0 / (upp * h);
                let sz = 1.0 / (2.0 * reach);
                Projected {
                    rows: [
                        row(scale(r, sx), -dot(target, r) * sx),
                        row(scale(d, -sy), dot(target, d) * sy),
                        row(scale(f, -sz), (reach + dot(target, f)) * sz),
                        [0.0, 0.0, 0.0, 1.0],
                    ],
                    eye: None,
                    target,
                    units_per_px: upp,
                    forward: f,
                }
            }
            Projection::Perspective => {
                let t = (self.fov / 2.0).tan();
                let dist = upp * h / 2.0 / t;
                let eye = sub(target, scale(f, dist));
                let near = (dist - reach).max(dist * 1e-3);
                let ax = 1.0 / (t * w / h);
                Projected {
                    rows: [
                        row(scale(r, ax), -dot(eye, r) * ax),
                        row(scale(d, -1.0 / t), dot(eye, d) / t),
                        [0.0, 0.0, 0.0, near],
                        row(f, -dot(eye, f)),
                    ],
                    eye: Some(eye),
                    target,
                    units_per_px: upp,
                    forward: f,
                }
            }
        }
    }
}

impl Projected {
    /// Where `p` lands, in px from the view's top left, and its depth (1 nearest); `None`
    /// behind the eye.
    pub fn to_px(&self, p: V3, w: f64, h: f64) -> Option<(f64, f64, f64)> {
        let clip = self
            .rows
            .map(|r| r[0] * p[0] + r[1] * p[1] + r[2] * p[2] + r[3]);
        if clip[3] <= 0.0 {
            return None;
        }
        let (x, y, z) = (clip[0] / clip[3], clip[1] / clip[3], clip[2] / clip[3]);
        Some(((x + 1.0) / 2.0 * w, (1.0 - y) / 2.0 * h, z))
    }
}

/// The grid's spacing for `b`: 1, 2 or 5 × 10ⁿ model units, about a twentieth of its
/// diameter.
pub fn grid_step(b: &Bounds) -> f64 {
    let want = (2.0 * radius(b) / 20.0).max(1e-9);
    let p = 10f64.powf(want.log10().floor());
    [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|m| m * p)
        .find(|s| *s >= want * 0.999)
        .unwrap_or(10.0 * p)
}

pub(crate) fn centre(b: &Bounds) -> V3 {
    let c = b.centre();
    c.map(f64::from)
}

/// Half the box's diagonal (a tiny positive length for an empty or flat-point box).
pub(crate) fn radius(b: &Bounds) -> f64 {
    if b.is_empty() {
        return 1.0;
    }
    f64::from(b.radius()).max(1e-6)
}

pub(crate) fn corners(b: &Bounds) -> [V3; 8] {
    let (lo, hi) = (b.min.map(f64::from), b.max.map(f64::from));
    std::array::from_fn(|i| {
        [
            if i & 1 == 0 { lo[0] } else { hi[0] },
            if i & 2 == 0 { lo[1] } else { hi[1] },
            if i & 4 == 0 { lo[2] } else { hi[2] },
        ]
    })
}

fn row(v: V3, k: f64) -> [f64; 4] {
    [v[0], v[1], v[2], k]
}

pub(crate) fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(crate) fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub(crate) fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(crate) fn scale(a: V3, k: f64) -> V3 {
    a.map(|c| c * k)
}

pub(crate) fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(crate) fn len(a: V3) -> f64 {
    dot(a, a).sqrt()
}

pub(crate) fn unit(a: V3) -> V3 {
    let l = len(a);
    if l > 0.0 { scale(a, 1.0 / l) } else { a }
}

/// `v` turned by `angle` radians about the unit `axis` (Rodrigues).
fn turn(v: V3, axis: V3, angle: f64) -> V3 {
    let (s, c) = angle.sin_cos();
    let k = cross(axis, v);
    let d = dot(axis, v) * (1.0 - c);
    [
        v[0] * c + k[0] * s + axis[0] * d,
        v[1] * c + k[1] * s + axis[1] * d,
        v[2] * c + k[2] * s + axis[2] * d,
    ]
}
