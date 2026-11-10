//! Veterinary, orthopaedic and cephalometric geometry from points the user places
//! (DESIGN.md §7, "Vet, orthopaedic and dental geometry").
//!
//! Manual geometry only: every point is placed by hand, in the order [`Template::landmarks`]
//! gives, and [`compute`] turns them into numbers. Nothing is detected, and no value is
//! graded or compared with a norm (DESIGN.md §1, Non-goals).
//!
//! Every computation runs in physical space: the points go through the calibration's matrix
//! first, so non-square pixels keep angles and circles true. Lengths carry the calibration's
//! unit and source; angles are degrees; the distraction index is a ratio and the vertebral
//! scores are in vertebral bodies.
//!
//! # Definitions
//!
//! - **VHS** (Buchanan & Bücheler 1995): the long axis (carina to apex) and the short axis
//!   (widest, perpendicular) are each laid along the spine from the cranial end of T4 and
//!   counted in vertebrae; VHS is their sum. The user marks the cranial end of T4 and the
//!   caudal ends of T4 to T9; a length past T9 is extrapolated with the mean vertebra and
//!   says so.
//! - **VLAS** (Malcolm et al. 2018): ventral carina to the caudal border of the left atrium
//!   where it meets the caudal vena cava, counted the same way.
//! - **Norberg angle**: at each femoral head's centre (a circle through three rim points),
//!   between the line to the other centre and the line to the craniolateral acetabular rim.
//! - **TPA**: the angle between the medial tibial plateau (cranial and caudal edges) and the
//!   perpendicular to the functional axis (intercondylar eminences to the talus centre),
//!   `90° − ∠(axis, plateau)`.
//! - **Distraction index** (PennHIP geometry): the distance between the femoral head's and
//!   the acetabulum's centres over the femoral head's radius (both circles by three points).
//! - **Femoral head**: a circle through three rim points (centre and diameter).
//! - **Femoral offset**: from the head's centre perpendicular to the femoral shaft axis.
//! - **Leg length**: each leg's landmark's distance from a pelvic reference line, and the
//!   difference (left − right).
//! - **Acetabular angle** (Sharp): at the teardrop, between the inter-teardrop line (outward)
//!   and the line to the lateral acetabular edge.
//! - **Steiner** and **Downs**: the classical angles and distances from hand-placed
//!   cephalometric landmarks (see [`Template::Steiner`] and [`Template::Downs`]). Signed
//!   distances are positive towards the face (the side of the line away from sella), so the
//!   values hold for a profile facing either way.

use crate::calib::{Calibration, Quantity};
use crate::geom::P;

/// A geometry template: which points it asks for and what it computes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Template {
    Vhs,
    Vlas,
    Norberg,
    Tpa,
    Distraction,
    FemoralHead,
    Offset,
    LegLength,
    Acetabular,
    /// S, N, A, B, Pog, Go, Gn, U1 tip and apex, L1 tip and apex, occlusal plane front and
    /// back: SNA, SNB, ANB, SN–GoGn, U1–NA (° and mm), L1–NB (° and mm), Pog–NB, the
    /// interincisal angle and the occlusal plane to SN.
    Steiner,
    /// Po, Or, S, N, A, B, Pog, Gn, Go, Me, U1 tip and apex, L1 tip and apex, occlusal plane
    /// front and back: facial angle, angle of convexity, A–B plane, mandibular plane, Y axis,
    /// occlusal cant, interincisal, L1 to the occlusal and the mandibular plane (as
    /// deviations from 90°), U1 to A–Pog.
    Downs,
}

/// Which group a template belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Vet,
    Ortho,
    Ceph,
}

/// One point a template asks for: its key (the message `geometry-pt-<key>`, and the name in
/// CSV) and a number shown with it (a rim point's index, a vertebra).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Landmark {
    pub key: &'static str,
    pub n: Option<u8>,
}

const fn lm(key: &'static str) -> Landmark {
    Landmark { key, n: None }
}

const fn lmn(key: &'static str, n: u8) -> Landmark {
    Landmark { key, n: Some(n) }
}

/// The vertebrae marked for the vertebral scores: the cranial end of T4, then the caudal
/// ends of T4 to T9.
const VERTEBRAE: [Landmark; 7] = [
    lm("t4-cranial"),
    lmn("vertebra-caudal", 4),
    lmn("vertebra-caudal", 5),
    lmn("vertebra-caudal", 6),
    lmn("vertebra-caudal", 7),
    lmn("vertebra-caudal", 8),
    lmn("vertebra-caudal", 9),
];

const VHS: [Landmark; 11] = [
    lm("long-axis-carina"),
    lm("long-axis-apex"),
    lmn("short-axis", 1),
    lmn("short-axis", 2),
    VERTEBRAE[0],
    VERTEBRAE[1],
    VERTEBRAE[2],
    VERTEBRAE[3],
    VERTEBRAE[4],
    VERTEBRAE[5],
    VERTEBRAE[6],
];

const VLAS: [Landmark; 9] = [
    lm("vlas-carina"),
    lm("vlas-atrium"),
    VERTEBRAE[0],
    VERTEBRAE[1],
    VERTEBRAE[2],
    VERTEBRAE[3],
    VERTEBRAE[4],
    VERTEBRAE[5],
    VERTEBRAE[6],
];

const NORBERG: [Landmark; 8] = [
    lmn("left-head-rim", 1),
    lmn("left-head-rim", 2),
    lmn("left-head-rim", 3),
    lmn("right-head-rim", 1),
    lmn("right-head-rim", 2),
    lmn("right-head-rim", 3),
    lm("left-acetabular-rim"),
    lm("right-acetabular-rim"),
];

const TPA: [Landmark; 4] = [
    lm("eminence"),
    lm("talus"),
    lm("plateau-cranial"),
    lm("plateau-caudal"),
];

const DISTRACTION: [Landmark; 6] = [
    lmn("head-rim", 1),
    lmn("head-rim", 2),
    lmn("head-rim", 3),
    lmn("acetabulum-rim", 1),
    lmn("acetabulum-rim", 2),
    lmn("acetabulum-rim", 3),
];

const FEMORAL_HEAD: [Landmark; 3] = [lmn("head-rim", 1), lmn("head-rim", 2), lmn("head-rim", 3)];

const OFFSET: [Landmark; 5] = [
    lmn("head-rim", 1),
    lmn("head-rim", 2),
    lmn("head-rim", 3),
    lm("shaft-proximal"),
    lm("shaft-distal"),
];

const LEG_LENGTH: [Landmark; 4] = [
    lmn("reference", 1),
    lmn("reference", 2),
    lm("left-landmark"),
    lm("right-landmark"),
];

const ACETABULAR: [Landmark; 3] = [lm("teardrop"), lm("teardrop-other"), lm("acetabular-edge")];

const STEINER: [Landmark; 13] = [
    lm("s"),
    lm("n"),
    lm("a"),
    lm("b"),
    lm("pog"),
    lm("go"),
    lm("gn"),
    lm("u1-tip"),
    lm("u1-apex"),
    lm("l1-tip"),
    lm("l1-apex"),
    lm("occlusal-front"),
    lm("occlusal-back"),
];

const DOWNS: [Landmark; 16] = [
    lm("po"),
    lm("or"),
    lm("s"),
    lm("n"),
    lm("a"),
    lm("b"),
    lm("pog"),
    lm("gn"),
    lm("go"),
    lm("me"),
    lm("u1-tip"),
    lm("u1-apex"),
    lm("l1-tip"),
    lm("l1-apex"),
    lm("occlusal-front"),
    lm("occlusal-back"),
];

impl Template {
    pub const ALL: [Template; 11] = [
        Template::Vhs,
        Template::Vlas,
        Template::Norberg,
        Template::Tpa,
        Template::Distraction,
        Template::FemoralHead,
        Template::Offset,
        Template::LegLength,
        Template::Acetabular,
        Template::Steiner,
        Template::Downs,
    ];

    /// The key in files and messages (`geometry-<key>`).
    pub fn key(self) -> &'static str {
        match self {
            Template::Vhs => "vhs",
            Template::Vlas => "vlas",
            Template::Norberg => "norberg",
            Template::Tpa => "tpa",
            Template::Distraction => "distraction-index",
            Template::FemoralHead => "femoral-head",
            Template::Offset => "femoral-offset",
            Template::LegLength => "leg-length",
            Template::Acetabular => "acetabular-angle",
            Template::Steiner => "steiner",
            Template::Downs => "downs",
        }
    }

    pub fn from_key(key: &str) -> Option<Template> {
        Template::ALL.into_iter().find(|t| t.key() == key)
    }

    pub fn family(self) -> Family {
        match self {
            Template::Vhs
            | Template::Vlas
            | Template::Norberg
            | Template::Tpa
            | Template::Distraction => Family::Vet,
            Template::FemoralHead
            | Template::Offset
            | Template::LegLength
            | Template::Acetabular => Family::Ortho,
            Template::Steiner | Template::Downs => Family::Ceph,
        }
    }

    /// The points asked for, in order.
    pub fn landmarks(self) -> &'static [Landmark] {
        match self {
            Template::Vhs => &VHS,
            Template::Vlas => &VLAS,
            Template::Norberg => &NORBERG,
            Template::Tpa => &TPA,
            Template::Distraction => &DISTRACTION,
            Template::FemoralHead => &FEMORAL_HEAD,
            Template::Offset => &OFFSET,
            Template::LegLength => &LEG_LENGTH,
            Template::Acetabular => &ACETABULAR,
            Template::Steiner => &STEINER,
            Template::Downs => &DOWNS,
        }
    }

    /// The next template (wrapping).
    pub fn next(self) -> Template {
        let i = Template::ALL.iter().position(|t| *t == self).unwrap_or(0);
        Template::ALL[(i + 1) % Template::ALL.len()]
    }

    pub fn prev(self) -> Template {
        let i = Template::ALL.iter().position(|t| *t == self).unwrap_or(0);
        Template::ALL[(i + Template::ALL.len() - 1) % Template::ALL.len()]
    }
}

/// A computed value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    /// A length or an angle, with its calibration.
    Quantity(Quantity),
    /// A unitless ratio (the distraction index).
    Ratio(f64),
    /// Vertebral bodies (VHS, VLAS).
    Vertebrae(f64),
}

impl Value {
    pub fn number(self) -> f64 {
        match self {
            Value::Quantity(q) => q.value,
            Value::Ratio(v) | Value::Vertebrae(v) => v,
        }
    }

    /// The unit's key in files: a length unit's key, `deg`, `ratio` or `vertebrae`.
    pub fn unit_key(self) -> &'static str {
        match self {
            Value::Quantity(q) if q.kind == crate::calib::Kind::Angle => "deg",
            Value::Quantity(q) => q.unit.key(),
            Value::Ratio(_) => "ratio",
            Value::Vertebrae(_) => "vertebrae",
        }
    }
}

/// One result: its key (the message `geometry-out-<key>`, and the name in CSV), the value
/// and whether it was extrapolated past the marked vertebrae.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Readout {
    pub key: &'static str,
    pub value: Value,
    pub extrapolated: bool,
}

/// What [`compute`] gives: the readouts (the first is the headline) and the construction
/// lines to draw, each an open path in stored pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub readouts: Vec<Readout>,
    pub guides: Vec<Vec<P>>,
}

// ---------------------------------------------------------------------------------------------
// Physical-space helpers.

/// The calibration's matrix and its inverse (physical ↔ stored pixels).
struct Space {
    m: [[f64; 2]; 2],
    inv: [[f64; 2]; 2],
}

impl Space {
    fn new(cal: &Calibration) -> Option<Space> {
        let m = cal.m;
        let det = m[0][0] * m[1][1] - m[0][1] * m[1][0];
        if det == 0.0 || !det.is_finite() {
            return None;
        }
        let inv = [
            [m[1][1] / det, -m[0][1] / det],
            [-m[1][0] / det, m[0][0] / det],
        ];
        Some(Space { m, inv })
    }

    fn phys(&self, p: P) -> P {
        P::new(
            self.m[0][0] * p.x + self.m[0][1] * p.y,
            self.m[1][0] * p.x + self.m[1][1] * p.y,
        )
    }

    fn pixel(&self, p: P) -> P {
        P::new(
            self.inv[0][0] * p.x + self.inv[0][1] * p.y,
            self.inv[1][0] * p.x + self.inv[1][1] * p.y,
        )
    }

    fn path(&self, v: &[P]) -> Vec<P> {
        v.iter().map(|p| self.pixel(*p)).collect()
    }
}

fn scale(p: P, k: f64) -> P {
    P::new(p.x * k, p.y * k)
}

fn dot(a: P, b: P) -> f64 {
    a.x * b.x + a.y * b.y
}

fn cross(a: P, b: P) -> f64 {
    a.x * b.y - a.y * b.x
}

/// The angle between two vectors, 0‥180°.
fn vang(u: P, w: P) -> Option<f64> {
    (u.hypot() > 0.0 && w.hypot() > 0.0).then(|| cross(u, w).abs().atan2(dot(u, w)).to_degrees())
}

/// The angle between two lines, 0‥90°.
fn lang(u: P, w: P) -> Option<f64> {
    vang(u, w).map(|a| a.min(180.0 - a))
}

/// The circle through three points: centre and radius.
fn circle(a: P, b: P, c: P) -> Option<(P, f64)> {
    let (b, c) = (b - a, c - a);
    let d = 2.0 * cross(b, c);
    let span = b.hypot().max(c.hypot());
    if span == 0.0 || d.abs() <= 1e-12 * span * span {
        return None;
    }
    let (bb, cc) = (dot(b, b), dot(c, c));
    let o = P::new((c.y * bb - b.y * cc) / d, (b.x * cc - c.x * bb) / d);
    Some((a + o, o.hypot()))
}

/// The foot of the perpendicular from `p` to the line through `a` and `b`.
fn foot(p: P, a: P, b: P) -> Option<P> {
    let d = b - a;
    let l2 = dot(d, d);
    (l2 > 0.0).then(|| a + scale(d, dot(p - a, d) / l2))
}

/// The distance of `p` from the line through `a` and `b`.
fn line_distance(p: P, a: P, b: P) -> Option<f64> {
    let d = b - a;
    let l = d.hypot();
    (l > 0.0).then(|| cross(d, p - a).abs() / l)
}

/// The signed distance of `p` from the line through `a` and `b`: positive on the side away
/// from `back` (the face's side when `back` is sella or porion).
fn signed_distance(p: P, a: P, b: P, back: P) -> Option<f64> {
    let d = b - a;
    let l = d.hypot();
    if l == 0.0 {
        return None;
    }
    let side = cross(d, back - a);
    let s = if side > 0.0 { -1.0 } else { 1.0 };
    Some(s * cross(d, p - a) / l)
}

/// A circle as a closed path (physical space).
fn ring(c: P, r: f64) -> Vec<P> {
    (0..=64)
        .map(|i| {
            let t = f64::from(i) / 64.0 * std::f64::consts::TAU;
            P::new(c.x + r * t.cos(), c.y + r * t.sin())
        })
        .collect()
}

/// A line through `a` and `b` extended by `k` of its length at both ends.
fn extended(a: P, b: P, k: f64) -> Vec<P> {
    let d = b - a;
    vec![a - scale(d, k), b + scale(d, k)]
}

/// A length laid along the marked vertebrae from the first mark, in vertebrae, and whether it
/// ran past the last mark (then extrapolated with the mean vertebra).
pub fn vertebrae(length: f64, marks: &[P]) -> Option<(f64, bool)> {
    let steps: Vec<f64> = marks.windows(2).map(|w| (w[1] - w[0]).hypot()).collect();
    if steps.is_empty() || steps.iter().any(|s| *s <= 0.0) || !length.is_finite() {
        return None;
    }
    let mut left = length;
    for (i, s) in steps.iter().enumerate() {
        if left <= *s {
            return Some((i as f64 + left / s, false));
        }
        left -= s;
    }
    let mean = steps.iter().sum::<f64>() / steps.len() as f64;
    Some((steps.len() as f64 + left / mean, true))
}

// ---------------------------------------------------------------------------------------------
// The templates.

struct Out<'a> {
    cal: &'a Calibration,
    sp: &'a Space,
    readouts: Vec<Readout>,
    guides: Vec<Vec<P>>,
}

impl Out<'_> {
    fn length(&mut self, key: &'static str, v: f64) {
        self.readouts.push(Readout {
            key,
            value: Value::Quantity(self.cal.length(v)),
            extrapolated: false,
        });
    }

    fn angle(&mut self, key: &'static str, v: f64) {
        self.readouts.push(Readout {
            key,
            value: Value::Quantity(self.cal.angle(v)),
            extrapolated: false,
        });
    }

    fn other(&mut self, key: &'static str, value: Value, extrapolated: bool) {
        self.readouts.push(Readout {
            key,
            value,
            extrapolated,
        });
    }

    fn guide(&mut self, v: &[P]) {
        let g = self.sp.path(v);
        self.guides.push(g);
    }
}

/// The readouts and construction lines of `template` from its points (stored pixels, in the
/// order of [`Template::landmarks`]) through `cal`. `None` when a point is missing or the
/// geometry is degenerate (three rim points on a line, two points of a line together).
pub fn compute(template: Template, pts: &[P], cal: &Calibration) -> Option<Outcome> {
    if pts.len() != template.landmarks().len() || pts.iter().any(|p| !p.is_finite()) {
        return None;
    }
    let sp = Space::new(cal)?;
    let q: Vec<P> = pts.iter().map(|p| sp.phys(*p)).collect();
    let mut o = Out {
        cal,
        sp: &sp,
        readouts: Vec::new(),
        guides: Vec::new(),
    };
    match template {
        Template::Vhs => {
            let (l, s) = ((q[1] - q[0]).hypot(), (q[3] - q[2]).hypot());
            let (lv, le) = vertebrae(l, &q[4..])?;
            let (sv, se) = vertebrae(s, &q[4..])?;
            if l == 0.0 || s == 0.0 {
                return None;
            }
            o.other("vhs", Value::Vertebrae(lv + sv), le || se);
            o.other("long-axis", Value::Vertebrae(lv), le);
            o.other("short-axis", Value::Vertebrae(sv), se);
            o.length("long-axis-length", l);
            o.length("short-axis-length", s);
            o.guide(&q[0..2]);
            o.guide(&q[2..4]);
            o.guide(&q[4..]);
        }
        Template::Vlas => {
            let l = (q[1] - q[0]).hypot();
            if l == 0.0 {
                return None;
            }
            let (v, e) = vertebrae(l, &q[2..])?;
            o.other("vlas", Value::Vertebrae(v), e);
            o.length("vlas-length", l);
            o.guide(&q[0..2]);
            o.guide(&q[2..]);
        }
        Template::Norberg => {
            let (cl, rl) = circle(q[0], q[1], q[2])?;
            let (cr, rr) = circle(q[3], q[4], q[5])?;
            let left = vang(cr - cl, q[6] - cl)?;
            let right = vang(cl - cr, q[7] - cr)?;
            o.angle("norberg-left", left);
            o.angle("norberg-right", right);
            o.guide(&ring(cl, rl));
            o.guide(&ring(cr, rr));
            o.guide(&[q[6], cl, cr, q[7]]);
        }
        Template::Tpa => {
            let axis = q[0] - q[1];
            let plateau = q[2] - q[3];
            let tpa = 90.0 - vang(axis, plateau)?;
            o.angle("tpa", tpa);
            o.guide(&[q[1], q[0]]);
            o.guide(&extended(q[3], q[2], 0.25));
            // The perpendicular to the axis at the eminence, as long as the plateau.
            let n = P::new(-axis.y, axis.x);
            let n = scale(n, plateau.hypot() * 0.75 / n.hypot());
            o.guide(&[q[0] - n, q[0] + n]);
        }
        Template::Distraction => {
            let (ch, rh) = circle(q[0], q[1], q[2])?;
            let (ca, ra) = circle(q[3], q[4], q[5])?;
            let d = (ca - ch).hypot();
            o.other("distraction-index", Value::Ratio(d / rh), false);
            o.length("centre-distance", d);
            o.length("head-radius", rh);
            o.guide(&ring(ch, rh));
            o.guide(&ring(ca, ra));
            o.guide(&[ch, ca]);
        }
        Template::FemoralHead => {
            let (c, r) = circle(q[0], q[1], q[2])?;
            o.length("head-diameter", 2.0 * r);
            o.guide(&ring(c, r));
            o.guide(&cross_at(c, r * 0.15));
        }
        Template::Offset => {
            let (c, r) = circle(q[0], q[1], q[2])?;
            let f = foot(c, q[3], q[4])?;
            o.length("offset", (c - f).hypot());
            o.length("head-diameter", 2.0 * r);
            o.guide(&ring(c, r));
            // The shaft axis drawn from the distal mark to past the foot.
            let far = if dot(f - q[4], q[3] - q[4]) > dot(q[3] - q[4], q[3] - q[4]) {
                f
            } else {
                q[3]
            };
            o.guide(&[q[4], far]);
            o.guide(&[c, f]);
        }
        Template::LegLength => {
            let left = line_distance(q[2], q[0], q[1])?;
            let right = line_distance(q[3], q[0], q[1])?;
            o.length("leg-difference", left - right);
            o.length("leg-left", left);
            o.length("leg-right", right);
            o.guide(&extended(q[0], q[1], 0.3));
            for p in [q[2], q[3]] {
                if let Some(f) = foot(p, q[0], q[1]) {
                    o.guide(&[p, f]);
                }
            }
        }
        Template::Acetabular => {
            let a = 180.0 - vang(q[1] - q[0], q[2] - q[0])?;
            o.angle("acetabular-angle", a);
            o.guide(&[q[1], q[0], q[0] - scale(q[1] - q[0], 0.25)]);
            o.guide(&[q[0], q[2]]);
        }
        Template::Steiner => steiner(&mut o, &q)?,
        Template::Downs => downs(&mut o, &q)?,
    }
    Some(Outcome {
        readouts: o.readouts,
        guides: o.guides,
    })
}

fn cross_at(c: P, r: f64) -> Vec<P> {
    vec![
        P::new(c.x - r, c.y),
        P::new(c.x + r, c.y),
        c,
        P::new(c.x, c.y - r),
        P::new(c.x, c.y + r),
    ]
}

fn steiner(o: &mut Out<'_>, q: &[P]) -> Option<()> {
    let [s, n, a, b, pog, go, gn, u1t, u1a, l1t, l1a, ocf, ocb] = q.try_into().ok()?;
    let sna = vang(s - n, a - n)?;
    let snb = vang(s - n, b - n)?;
    o.angle("sna", sna);
    o.angle("snb", snb);
    o.angle("anb", sna - snb);
    o.angle("sn-gogn", lang(n - s, gn - go)?);
    o.angle("u1-na-angle", lang(u1t - u1a, a - n)?);
    o.length("u1-na-mm", signed_distance(u1t, n, a, s)?);
    o.angle("l1-nb-angle", lang(l1t - l1a, b - n)?);
    o.length("l1-nb-mm", signed_distance(l1t, n, b, s)?);
    o.length("pog-nb-mm", signed_distance(pog, n, b, s)?);
    o.angle("interincisal", vang(u1a - u1t, l1a - l1t)?);
    o.angle("occlusal-sn", lang(n - s, ocf - ocb)?);
    o.guide(&[s, n]);
    o.guide(&extended(n, a, 0.15));
    o.guide(&extended(n, b, 0.15));
    o.guide(&extended(go, gn, 0.1));
    o.guide(&[u1a, u1t]);
    o.guide(&[l1a, l1t]);
    o.guide(&extended(ocb, ocf, 0.2));
    Some(())
}

fn downs(o: &mut Out<'_>, q: &[P]) -> Option<()> {
    let [
        po,
        or,
        s,
        n,
        a,
        b,
        pog,
        gn,
        go,
        me,
        u1t,
        u1a,
        l1t,
        l1a,
        ocf,
        ocb,
    ] = q.try_into().ok()?;
    o.angle("facial-angle", vang(po - or, pog - n)?);
    let convex = 180.0 - vang(n - a, pog - a)?;
    let sign = if signed_distance(a, n, pog, s)? < 0.0 {
        -1.0
    } else {
        1.0
    };
    o.angle("convexity", sign * convex);
    let ab = lang(b - a, pog - n)?;
    let b_ahead = signed_distance(b, n, pog, s)? - signed_distance(a, n, pog, s)?;
    o.angle("ab-plane", if b_ahead < 0.0 { -ab } else { ab });
    o.angle("mandibular-plane", lang(or - po, me - go)?);
    o.angle("y-axis", vang(gn - s, or - po)?);
    o.angle("occlusal-cant", lang(or - po, ocf - ocb)?);
    o.angle("interincisal", vang(u1a - u1t, l1a - l1t)?);
    o.angle("l1-occlusal", vang(l1t - l1a, ocb - ocf)? - 90.0);
    o.angle("l1-mandibular", vang(l1t - l1a, go - me)? - 90.0);
    o.length("u1-apog-mm", signed_distance(u1t, a, pog, s)?);
    o.guide(&extended(po, or, 0.1));
    o.guide(&extended(n, pog, 0.1));
    o.guide(&[n, a, pog]);
    o.guide(&extended(a, b, 0.3));
    o.guide(&extended(go, me, 0.1));
    o.guide(&[s, gn]);
    o.guide(&[u1a, u1t]);
    o.guide(&[l1a, l1t]);
    o.guide(&extended(ocb, ocf, 0.2));
    Some(())
}

/// Whether a value went through a calibration that is only an estimate.
pub fn is_approximate(r: &Readout) -> bool {
    match r.value {
        Value::Quantity(q) => q.source.is_approximate(),
        _ => false,
    }
}

#[cfg(test)]
#[path = "clinical_tests.rs"]
mod tests;
