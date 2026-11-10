//! Linked views of DICOM series (DESIGN.md §7.3): the geometry behind comparing series side
//! by side. Everything is in patient coordinates (LPS, mm), slice by slice, so gantry tilt,
//! uneven spacing, different slice thicknesses and gaps need no resampling.
//!
//! - [`Stack`]: a series' slices as [`Plane`]s, from the header scan alone.
//! - [`Stack::follow`]: synced scrolling. The slice of another series nearest a plane,
//!   measured along that series' own normal (by position, never by index), and whether the
//!   point falls inside a slice, in a gap, or outside the series.
//! - [`reference_line`]: where one slice plane (and its slab) cuts another slice, as a
//!   segment in the other slice's pixels, for oblique planes too.
//! - [`Stack::locate`]: the 3D cursor. The slice containing a patient point, and the
//!   point's pixel position in it.
//! - [`Link`]: how two series' coordinates relate: the same Frame of Reference, trusted
//!   patient positions, or a manual offset along the normal from one aligned slice pair
//!   ([`Link::aligned`]).

use crate::series::Series;

/// Most angle between two series' slice normals for synced scrolling, degrees: beyond it
/// the planes cross (axial against sagittal) and only reference lines link them. Wider than
/// any gantry tilt in use (up to 30°).
pub const SYNC_MAX_DEG: f64 = 35.0;

/// Least angle between two planes for a reference line, degrees: nearly parallel planes cut
/// far outside the image, or not at all.
pub const LINE_MIN_DEG: f64 = 10.0;

pub(crate) fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn mul(a: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit(v: [f64; 3]) -> Option<[f64; 3]> {
    let n = dot(v, v).sqrt();
    (n.is_finite() && n > 1e-9).then(|| mul(v, 1.0 / n))
}

/// One slice in the patient: where its first pixel's centre is, where its rows and columns
/// point, the pixel spacing and size, and its thickness.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    /// Image Position (Patient): the centre of the first stored pixel, mm.
    pub origin: [f64; 3],
    /// Direction of increasing column index (along a row), unit.
    pub row: [f64; 3],
    /// Direction of increasing row index (down the image), unit.
    pub col: [f64; 3],
    /// Pixel Spacing: between adjacent rows, then between adjacent columns, mm.
    pub spacing: [f64; 2],
    /// Columns, rows.
    pub size: [u32; 2],
    /// Slice thickness, mm (the spacing when the header states none).
    pub thickness: f64,
}

impl Plane {
    /// A plane from Image Orientation (Patient) direction cosines; `None` when they are not
    /// two directions.
    pub fn new(
        origin: [f64; 3],
        orientation: [f64; 6],
        spacing: [f64; 2],
        size: [u32; 2],
        thickness: f64,
    ) -> Option<Plane> {
        let row = unit([orientation[0], orientation[1], orientation[2]])?;
        let col = unit([orientation[3], orientation[4], orientation[5]])?;
        unit(cross(row, col))?;
        (spacing.iter().all(|s| s.is_finite() && *s > 0.0) && origin.iter().all(|x| x.is_finite()))
            .then_some(Plane {
                origin,
                row,
                col,
                spacing,
                size,
                thickness: if thickness.is_finite() && thickness > 0.0 {
                    thickness
                } else {
                    0.0
                },
            })
    }

    /// The slice normal (row × column), unit.
    pub fn normal(&self) -> [f64; 3] {
        unit(cross(self.row, self.col)).unwrap_or([0.0, 0.0, 1.0])
    }

    /// The patient point at pixel `(x, y)` (column, row; pixel centres on whole numbers).
    pub fn at(&self, x: f64, y: f64) -> [f64; 3] {
        add(
            self.origin,
            add(
                mul(self.row, x * self.spacing[1]),
                mul(self.col, y * self.spacing[0]),
            ),
        )
    }

    /// Patient point `p` in this slice: its pixel position (column, row) and its signed
    /// distance from the plane along the normal, mm.
    pub fn project(&self, p: [f64; 3]) -> (f64, f64, f64) {
        let d = sub(p, self.origin);
        let n = self.normal();
        // The row and column directions may not be quite orthogonal: solve in the plane.
        let (a, b, c) = (dot(self.row, self.row), dot(self.row, self.col), {
            dot(self.col, self.col)
        });
        let (u, v) = (dot(d, self.row), dot(d, self.col));
        let det = a * c - b * b;
        let (s, t) = if det.abs() > 1e-12 {
            ((u * c - v * b) / det, (v * a - u * b) / det)
        } else {
            (u, v)
        };
        (s / self.spacing[1], t / self.spacing[0], dot(d, n))
    }

    /// The slice's position along its normal, mm (z for an axial slice).
    pub fn position(&self) -> f64 {
        dot(self.origin, self.normal())
    }

    /// The image's centre in the patient.
    pub fn centre(&self) -> [f64; 3] {
        let (w, h) = (f64::from(self.size[0]), f64::from(self.size[1]));
        self.at((w - 1.0) / 2.0, (h - 1.0) / 2.0)
    }

    /// The image's outer corners (pixel edges, not centres), in drawing order.
    pub fn corners(&self) -> [[f64; 3]; 4] {
        let (w, h) = (f64::from(self.size[0]), f64::from(self.size[1]));
        [
            self.at(-0.5, -0.5),
            self.at(w - 0.5, -0.5),
            self.at(w - 0.5, h - 0.5),
            self.at(-0.5, h - 0.5),
        ]
    }

    /// Whether pixel position `(x, y)` is on the image.
    pub fn contains(&self, x: f64, y: f64) -> bool {
        let (w, h) = (f64::from(self.size[0]), f64::from(self.size[1]));
        x >= -0.5 && y >= -0.5 && x <= w - 0.5 && y <= h - 0.5
    }

    /// The same slice moved by `d` (mm).
    pub fn shifted(&self, d: [f64; 3]) -> Plane {
        Plane {
            origin: add(self.origin, d),
            ..*self
        }
    }

    /// Width and height of the image in the patient, mm.
    pub fn extent_mm(&self) -> (f64, f64) {
        (
            f64::from(self.size[0]) * self.spacing[1],
            f64::from(self.size[1]) * self.spacing[0],
        )
    }
}

/// The angle between two planes' normals, degrees (0 to 90; parallel planes, either way
/// round, are 0).
pub fn angle_deg(a: &Plane, b: &Plane) -> f64 {
    dot(a.normal(), b.normal())
        .abs()
        .min(1.0)
        .acos()
        .to_degrees()
}

/// How a series' coordinates map into another's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Link {
    /// The same Frame of Reference UID: coordinates are shared.
    Frame,
    /// Different Frames of Reference the user linked as they are (the same patient
    /// position, such as two series of one session with a new UID).
    Position,
    /// A manual alignment: add this (mm) to a point of the first series to get the same
    /// anatomy in the second.
    Offset([f64; 3]),
}

impl Link {
    /// The link a user makes by aligning slice `a` of one series with slice `b` of another:
    /// the offset between their centres along `a`'s normal (the in-plane position stays).
    pub fn aligned(a: &Plane, b: &Plane) -> Link {
        let n = a.normal();
        Link::Offset(mul(n, dot(sub(b.centre(), a.centre()), n)))
    }

    /// Point `p` of the first series in the second's coordinates.
    pub fn map(&self, p: [f64; 3]) -> [f64; 3] {
        match self {
            Link::Frame | Link::Position => p,
            Link::Offset(d) => add(p, *d),
        }
    }

    /// The same link the other way round.
    pub fn inverse(&self) -> Link {
        match *self {
            Link::Offset(d) => Link::Offset(mul(d, -1.0)),
            l => l,
        }
    }

    /// The offset, mm (0 when coordinates are shared).
    pub fn offset(&self) -> [f64; 3] {
        match self {
            Link::Offset(d) => *d,
            _ => [0.0; 3],
        }
    }
}

/// Where a point falls in a series.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fit {
    /// Within a slice's thickness (or half the spacing, whichever is larger).
    Inside,
    /// Between two slices further apart than the series' spacing: slices are missing, or
    /// the series skips there.
    Gap,
    /// Beyond the first or last slice.
    Outside,
}

/// The slice picked for a point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pick {
    /// Index into [`Stack::planes`].
    pub slice: usize,
    /// Signed distance from the point to that slice's plane, along its normal, mm.
    pub distance: f64,
    pub fit: Fit,
}

/// The 3D cursor in one series.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Located {
    pub pick: Pick,
    /// The point's pixel position (column, row) in the picked slice.
    pub pixel: (f64, f64),
    /// The point projects onto the image (not beside it).
    pub on_image: bool,
}

/// A series' slices in display order, with what linking needs to know about it.
#[derive(Clone, Debug, PartialEq)]
pub struct Stack {
    pub frame_of_reference: String,
    pub study_uid: String,
    pub modality: String,
    pub planes: Vec<Plane>,
    /// Each plane's file (index into the series' items) and frame in it.
    pub source: Vec<(usize, u32)>,
    /// Typical distance between neighbouring slices along the normal, mm (0 for one slice).
    pub spacing: f64,
    /// Whose anatomy the orientation letters name (the first slice with geometry's).
    pub anatomy: crate::orient::Anatomy,
}

/// Median of `v` (empty: 0).
fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

impl Stack {
    /// The slices of `series` with geometry (position, orientation, pixel spacing and size);
    /// a multi-frame file gives one plane per frame position. `None` when no slice has it.
    pub fn of<K>(series: &Series<K>) -> Option<Stack> {
        let mut planes = Vec::new();
        let mut source = Vec::new();
        let mut frame_of_reference = String::new();
        let mut anatomy = None;
        for (k, it) in series.items.iter().enumerate() {
            let i = &it.instance;
            let (Some(o), Some(sp)) = (i.orientation, i.pixel_spacing) else {
                continue;
            };
            if frame_of_reference.is_empty() {
                frame_of_reference.clone_from(&i.frame_of_reference);
            }
            anatomy.get_or_insert(i.anatomy);
            let size = [i.columns, i.rows];
            let thick = i.slice_thickness.unwrap_or(0.0);
            let positions: Vec<[f64; 3]> = if i.frame_positions.len() > 1 {
                i.frame_positions.clone()
            } else {
                i.position.into_iter().collect()
            };
            for (f, p) in positions.into_iter().enumerate() {
                if let Some(pl) = Plane::new(p, o, sp, size, thick) {
                    planes.push(pl);
                    source.push((k, f as u32));
                }
            }
        }
        if planes.is_empty() {
            return None;
        }
        let mut st = Stack {
            frame_of_reference,
            study_uid: series.study_uid.clone(),
            modality: series.modality.clone(),
            planes,
            source,
            spacing: 0.0,
            anatomy: anatomy.unwrap_or_default(),
        };
        st.settle();
        Some(st)
    }

    /// A stack from planes (tests and synthetic series).
    pub fn from_planes(frame_of_reference: &str, modality: &str, planes: Vec<Plane>) -> Stack {
        let source = (0..planes.len()).map(|k| (k, 0)).collect();
        let mut st = Stack {
            frame_of_reference: frame_of_reference.to_owned(),
            study_uid: String::new(),
            modality: modality.to_owned(),
            planes,
            source,
            spacing: 0.0,
            anatomy: Default::default(),
        };
        st.settle();
        st
    }

    /// Fills in the spacing and the thickness of slices that state none.
    fn settle(&mut self) {
        let n = self.normal();
        let along: Vec<f64> = self.planes.iter().map(|p| dot(p.origin, n)).collect();
        let steps: Vec<f64> = along
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .filter(|d| *d > 1e-6)
            .collect();
        self.spacing = median(steps);
        for p in &mut self.planes {
            if p.thickness <= 0.0 {
                p.thickness = self.spacing;
            }
        }
    }

    /// The series' normal: the middle slice's.
    pub fn normal(&self) -> [f64; 3] {
        self.planes
            .get(self.planes.len() / 2)
            .map_or([0.0, 0.0, 1.0], Plane::normal)
    }

    /// The middle slice (where a viewport starts).
    pub fn middle(&self) -> usize {
        self.planes.len() / 2
    }

    /// How far around slice `k` a point still counts as in it: half its thickness, or half
    /// the spacing when that is larger (slices that do not touch still fill the stack).
    fn half_extent(&self, k: usize) -> f64 {
        let t = self.planes.get(k).map_or(0.0, |p| p.thickness);
        (t.max(self.spacing) / 2.0).max(1e-3)
    }

    /// Where a point `d` mm from slice `k` (along its normal) falls: inside it, in a gap,
    /// or beyond the series' ends (`p` is the point).
    fn fit(&self, k: usize, d: f64, p: [f64; 3]) -> Fit {
        if d.abs() <= self.half_extent(k) + 1e-6 {
            return Fit::Inside;
        }
        let n = self.normal();
        let s = dot(p, n);
        let (lo, hi) = self
            .planes
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), pl| {
                let a = dot(pl.origin, n);
                (lo.min(a), hi.max(a))
            });
        if s > lo && s < hi {
            Fit::Gap
        } else {
            Fit::Outside
        }
    }

    /// The slice nearest patient point `p`: smallest distance from the point to each slice's
    /// own plane (so tilted and unevenly spaced slices are exact), and how it fits.
    pub fn nearest(&self, p: [f64; 3]) -> Option<Pick> {
        let (k, d) = self
            .planes
            .iter()
            .enumerate()
            .map(|(k, pl)| (k, dot(sub(p, pl.origin), pl.normal())))
            .min_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))?;
        Some(Pick {
            slice: k,
            distance: d,
            fit: self.fit(k, d, p),
        })
    }

    /// Synced scrolling: the slice of this series that shows the same place as `src` (a
    /// slice of another series; `link` maps its coordinates into this one's). Each slice is
    /// measured at the centre of its image, from where the source plane crosses the line
    /// through that centre along the slice's normal, so a tilted stack is followed where
    /// the view is looked at, not at an image corner. `None` when the planes cross at more
    /// than [`SYNC_MAX_DEG`].
    pub fn follow(&self, src: &Plane, link: Link) -> Option<Pick> {
        let src = src.shifted(link.offset());
        if angle_deg(&src, self.planes.get(self.middle())?) > SYNC_MAX_DEG {
            return None;
        }
        let ns = src.normal();
        let (k, d, q) = self
            .planes
            .iter()
            .enumerate()
            .filter_map(|(k, pl)| {
                let c = pl.centre();
                let n = pl.normal();
                let den = dot(n, ns);
                (den.abs() > 1e-6).then(|| {
                    let t = -dot(sub(c, src.origin), ns) / den;
                    (k, t, add(c, mul(n, t)))
                })
            })
            .min_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))?;
        Some(Pick {
            slice: k,
            distance: d,
            fit: self.fit(k, d, q),
        })
    }

    /// The 3D cursor: the slice holding patient point `p` and where the point is on it.
    pub fn locate(&self, p: [f64; 3]) -> Option<Located> {
        let pick = self.nearest(p)?;
        let pl = &self.planes[pick.slice];
        let (x, y, _) = pl.project(p);
        Some(Located {
            pick,
            pixel: (x, y),
            on_image: pl.contains(x, y),
        })
    }

    /// Slices missing in this series between `a` and `b` (indices), from the spacing: 0
    /// when they are neighbours at the usual distance.
    pub fn missing_between(&self, a: usize, b: usize) -> u32 {
        let (Some(pa), Some(pb)) = (self.planes.get(a), self.planes.get(b)) else {
            return 0;
        };
        if self.spacing <= 0.0 {
            return 0;
        }
        let d = dot(sub(pb.origin, pa.origin), self.normal()).abs();
        let steps = (d / self.spacing).round();
        (steps - 1.0).max(0.0) as u32
    }
}

/// A line segment in a slice's pixels (column, row).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    pub a: (f64, f64),
    pub b: (f64, f64),
}

impl Segment {
    pub fn length(&self) -> f64 {
        (self.b.0 - self.a.0).hypot(self.b.1 - self.a.1)
    }
}

/// A reference line: where a slice (and its slab) cuts another slice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RefLine {
    /// The slice plane itself.
    pub centre: Option<Segment>,
    /// The slab's two faces (half the thickness each side), when the slab is wider than a
    /// pixel of the other slice.
    pub slab: [Option<Segment>; 2],
}

/// Where plane `src` (bounded by its image) cuts the image of `dst`, in `dst`'s pixels:
/// `None` when the planes are within [`LINE_MIN_DEG`] of parallel or do not meet on both
/// images. Points of `src` are taken through `link` first.
pub fn cut(src: &Plane, dst: &Plane, link: Link) -> Option<Segment> {
    let src = src.shifted(link.offset());
    if angle_deg(&src, dst) < LINE_MIN_DEG {
        return None;
    }
    let n = dst.normal();
    let c = src.corners();
    let d: Vec<f64> = c.iter().map(|p| dot(sub(*p, dst.origin), n)).collect();
    // Where the edges of src's image cross dst's plane.
    let mut hits: Vec<[f64; 3]> = Vec::new();
    for i in 0..4 {
        let j = (i + 1) % 4;
        let (di, dj) = (d[i], d[j]);
        if di == 0.0 {
            hits.push(c[i]);
        } else if di * dj < 0.0 {
            let t = di / (di - dj);
            hits.push(add(c[i], mul(sub(c[j], c[i]), t)));
        }
    }
    if hits.len() < 2 {
        return None;
    }
    // The two hits furthest apart (a corner on the plane can repeat).
    let (mut a, mut b, mut best) = (hits[0], hits[1], -1.0);
    for i in 0..hits.len() {
        for j in i + 1..hits.len() {
            let l = dot(sub(hits[i], hits[j]), sub(hits[i], hits[j]));
            if l > best {
                (a, b, best) = (hits[i], hits[j], l);
            }
        }
    }
    let (ax, ay, _) = dst.project(a);
    let (bx, by, _) = dst.project(b);
    let (w, h) = (f64::from(dst.size[0]), f64::from(dst.size[1]));
    clip((ax, ay), (bx, by), [-0.5, -0.5, w - 0.5, h - 0.5])
}

/// The reference line of `src` (the slice shown in the active view) on `dst`, with the
/// slab's faces at `slab` mm (`None`: the slice's own thickness).
pub fn reference_line(src: &Plane, dst: &Plane, link: Link, slab: Option<f64>) -> Option<RefLine> {
    let centre = cut(src, dst, link)?;
    let t = slab.unwrap_or(src.thickness);
    let px = dst.spacing[0].min(dst.spacing[1]);
    let faces = if t > px {
        let n = src.normal();
        [-0.5, 0.5].map(|k| cut(&src.shifted(mul(n, k * t)), dst, link))
    } else {
        [None, None]
    };
    Some(RefLine {
        centre: Some(centre),
        slab: faces,
    })
}

/// Liang–Barsky: the part of segment `a`–`b` inside `[x0, y0, x1, y1]`.
fn clip(a: (f64, f64), b: (f64, f64), r: [f64; 4]) -> Option<Segment> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let mut t0: f64 = 0.0;
    let mut t1: f64 = 1.0;
    for (p, q) in [
        (-dx, a.0 - r[0]),
        (dx, r[2] - a.0),
        (-dy, a.1 - r[1]),
        (dy, r[3] - a.1),
    ] {
        if p.abs() < 1e-12 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    (t0 < t1 - 1e-9).then_some(Segment {
        a: (a.0 + t0 * dx, a.1 + t0 * dy),
        b: (a.0 + t1 * dx, a.1 + t1 * dy),
    })
}

#[cfg(test)]
#[path = "linked_tests.rs"]
mod tests;
