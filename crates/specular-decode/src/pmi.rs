//! A STEP model's PMI (product manufacturing information) as the viewer shows it: dimensions,
//! geometric tolerances (feature control frames per ISO 1101 / ASME Y14.5), datums, datum
//! targets and annotations that are only drawn, plus the saved views that show them.
//!
//! OpenCASCADE's XDE reads them in the decode host ([`specular_sandbox::CadPmi`]); here they
//! are put in the mesh's frame and units, their faces become triangle ranges of the mesh, and
//! each gets its text: symbols and numbers only (the words are the app's). Characters are
//! those the UI font has (Ø for the diameter, circled letters for modifiers); the
//! characteristics' own symbols are drawn as strokes by the app ([`Characteristic::symbol`]
//! gives the Unicode one for other uses).
//!
//! A part placed several times in an assembly has its PMI once, where the file puts it; its
//! faces come once per instance. Each instance gets its own copy, moved with it: the move
//! from the instance nearest the annotation to each other one is found from their faces'
//! matching vertices (every instance is the same triangulation, placed).

use specular_mesh::preview::canonical;
use specular_mesh::{Bounds, Mesh, Up};
use specular_sandbox::cad::{
    CadDatumRef, CadModel, CadPmi, PMI_FIT, PMI_PLANE, PMI_PLUS_MINUS, PMI_POINT, PMI_RANGE,
    PMI_TEXT, PMI_VALUE,
};
use specular_sandbox::{FORM_VARIANCES, GRADES};

pub use specular_sandbox::cad::PmiKind;

/// A geometric characteristic (the first cell of a feature control frame).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Characteristic {
    Straightness,
    Flatness,
    Circularity,
    Cylindricity,
    LineProfile,
    SurfaceProfile,
    Angularity,
    Perpendicularity,
    Parallelism,
    Position,
    Concentricity,
    Coaxiality,
    Symmetry,
    CircularRunout,
    TotalRunout,
}

impl Characteristic {
    /// From XDE's `GeomToleranceType` code.
    pub fn from_code(c: u8) -> Option<Characteristic> {
        use Characteristic::*;
        Some(match c {
            1 => Angularity,
            2 => CircularRunout,
            3 => Circularity,
            4 => Coaxiality,
            5 => Concentricity,
            6 => Cylindricity,
            7 => Flatness,
            8 => Parallelism,
            9 => Perpendicularity,
            10 => Position,
            11 => LineProfile,
            12 => SurfaceProfile,
            13 => Straightness,
            14 => Symmetry,
            15 => TotalRunout,
            _ => return None,
        })
    }

    /// Its symbol as a Unicode character (ISO 1101; the app draws them as strokes too).
    pub fn symbol(self) -> char {
        use Characteristic::*;
        match self {
            Straightness => '⏤',
            Flatness => '⏥',
            Circularity => '○',
            Cylindricity => '⌭',
            LineProfile => '⌒',
            SurfaceProfile => '⌓',
            Angularity => '∠',
            Perpendicularity => '⟂',
            Parallelism => '⫽',
            Position => '⌖',
            Concentricity | Coaxiality => '◎',
            Symmetry => '⌯',
            CircularRunout => '↗',
            TotalRunout => '⌰',
        }
    }

    /// A stable id (for the app's strings: `pmi-char-<id>`).
    pub fn id(self) -> &'static str {
        use Characteristic::*;
        match self {
            Straightness => "straightness",
            Flatness => "flatness",
            Circularity => "circularity",
            Cylindricity => "cylindricity",
            LineProfile => "line-profile",
            SurfaceProfile => "surface-profile",
            Angularity => "angularity",
            Perpendicularity => "perpendicularity",
            Parallelism => "parallelism",
            Position => "position",
            Concentricity => "concentricity",
            Coaxiality => "coaxiality",
            Symmetry => "symmetry",
            CircularRunout => "circular-runout",
            TotalRunout => "total-runout",
        }
    }
}

/// What a dimension measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DimKind {
    Linear,
    Angular,
    Diameter,
    Radius,
    /// Curve length or along a path.
    Length,
    Thickness,
    Other,
}

impl DimKind {
    /// From XDE's `DimensionType` code.
    fn from_code(c: u8) -> DimKind {
        match c {
            1..=10 | 12 => DimKind::Linear,
            11 | 28 => DimKind::Angular,
            15 | 16 | 19 | 20 | 23 | 24 => DimKind::Diameter,
            17 | 18 | 21 | 22 | 25 | 26 => DimKind::Radius,
            13 | 14 | 29 => DimKind::Length,
            27 => DimKind::Thickness,
            _ => DimKind::Other,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            DimKind::Linear => "linear",
            DimKind::Angular => "angular",
            DimKind::Diameter => "diameter",
            DimKind::Radius => "radius",
            DimKind::Length => "length",
            DimKind::Thickness => "thickness",
            DimKind::Other => "other",
        }
    }
}

/// How an annotation is labelled in the view.
#[derive(Clone, Debug, PartialEq)]
pub enum Label {
    /// A dimension's text ("Ø35 +0/−0.2").
    Text(String),
    /// A feature control frame: its characteristic, then the tolerance cell and one cell per
    /// datum of its reference frame.
    Frame {
        characteristic: Option<Characteristic>,
        cells: Vec<String>,
    },
    /// A datum feature symbol (its letter), or a datum target ("A1").
    Datum(String),
    /// Drawn only: its drawing is its label.
    Drawing,
}

/// One annotation.
#[derive(Clone, Debug, PartialEq)]
pub struct PmiItem {
    pub kind: PmiKind,
    pub dim: Option<DimKind>,
    pub characteristic: Option<Characteristic>,
    /// The file's name for it ("Position.1"), when it gives one.
    pub name: String,
    pub label: Label,
    /// The label on one line (lists, screen readers).
    pub text: String,
    /// Where its label sits, and where its leader ends (on its faces), in the mesh's frame.
    pub anchor: Option<[f32; 3]>,
    pub target: Option<[f32; 3]>,
    /// The annotation as drawn in the file, in the mesh's frame.
    pub polylines: Vec<Vec<[f32; 3]>>,
    /// Its faces as triangle ranges of the mesh: first triangle, count.
    pub triangles: Vec<(u32, u32)>,
    /// The box of its faces (else of its drawing), for going to it.
    pub bounds: Bounds,
}

/// A saved view (an AP242 camera) and the annotations it shows.
#[derive(Clone, Debug, PartialEq)]
pub struct PmiView {
    pub name: String,
    /// The view direction and up, in the mesh's frame.
    pub direction: [f64; 3],
    pub up: [f64; 3],
    pub orthographic: bool,
    pub items: Vec<usize>,
}

/// A model's PMI.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pmi {
    pub items: Vec<PmiItem>,
    pub views: Vec<PmiView>,
}

impl Pmi {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.views.is_empty()
    }

    /// How many of each kind: dimensions, tolerances, datums, datum targets, drawn only.
    pub fn counts(&self) -> [usize; 5] {
        let mut n = [0; 5];
        for i in &self.items {
            n[match i.kind {
                PmiKind::Dimension => 0,
                PmiKind::GeometricTolerance => 1,
                PmiKind::Datum => 2,
                PmiKind::DatumTarget => 3,
                PmiKind::Graphic => 4,
            }] += 1;
        }
        n
    }
}

/// A number as written on a drawing: `frac` decimals when the file says, else up to four
/// with trailing zeros dropped; a minus sign, not a hyphen.
pub fn number(v: f64, frac: u8) -> String {
    let s = if frac > 0 && frac <= 8 {
        format!("{v:.*}", frac as usize)
    } else {
        let s = format!("{v:.4}");
        let s = s.trim_end_matches('0').trim_end_matches('.');
        s.to_owned()
    };
    let s = if s == "-0" { "0".to_owned() } else { s };
    s.replacen('-', "−", 1)
}

/// A dimension's modifiers (XDE `DimensionModif`): what goes before the value, and after.
fn dim_modifiers(codes: &[u8]) -> (String, Vec<&'static str>) {
    let mut pre = String::new();
    let mut post = Vec::new();
    for &c in codes {
        match c {
            0 => pre.push_str("CR "),
            1 => pre.push('□'),
            2 => post.push("<ST>"),
            3 => post.push("<CF>"),
            4 => post.push("(LP)"),
            5 => post.push("(LS)"),
            6 => post.push("(GG)"),
            7 => post.push("(GX)"),
            8 => post.push("(GN)"),
            9 => post.push("(CC)"),
            10 => post.push("(CA)"),
            11 => post.push("(CV)"),
            12 => post.push("(SX)"),
            13 => post.push("(SN)"),
            14 => post.push("(SA)"),
            15 => post.push("(SM)"),
            16 => post.push("(SD)"),
            17 => post.push("(SR)"),
            19 => post.push("ACS"),
            20 => post.push("SCS"),
            21 => post.push("CT"),
            22 => post.push("Ⓕ"),
            23 => post.push("↔"),
            _ => {}
        }
    }
    (pre, post)
}

/// A dimension's text: prefix, value, tolerance, fit, qualifier and modifiers.
pub fn dimension_text(p: &CadPmi) -> String {
    let kind = DimKind::from_code(p.code);
    let frac = p.decimals.1;
    let (pre, post) = dim_modifiers(&p.modifiers);
    let sign = match (kind, p.code) {
        (DimKind::Diameter, 16) => "SØ",
        (DimKind::Diameter, _) => "Ø",
        (DimKind::Radius, 18) => "SR",
        (DimKind::Radius, _) => "R",
        _ => "",
    };
    let deg = if kind == DimKind::Angular { "°" } else { "" };
    let num = |v: f64| format!("{}{deg}", number(v, frac));
    let mut s = format!("{pre}{sign}");
    if p.flags & PMI_RANGE != 0 {
        s.push_str(&format!("{} – {}", num(p.lower), num(p.upper)));
    } else if p.flags & PMI_VALUE != 0 {
        s.push_str(&num(p.value));
    }
    if p.flags & PMI_PLUS_MINUS != 0 {
        let (up, low) = (p.upper, p.lower.abs());
        if (up - low).abs() <= 1e-9 * up.abs().max(1.0) && up != 0.0 {
            s.push_str(&format!(" ±{}", num(up)));
        } else {
            let plus = if up < 0.0 {
                num(up)
            } else {
                format!("+{}", num(up))
            };
            s.push_str(&format!(" {plus}/−{}", num(low)));
        }
    }
    if p.flags & PMI_FIT != 0 {
        let v = FORM_VARIANCES
            .get(p.fit_variance as usize)
            .copied()
            .unwrap_or("");
        let g = GRADES.get(p.fit_grade as usize).copied().unwrap_or("");
        // Holes take capitals, shafts small letters.
        let v = if p.fit_hole {
            v.to_owned()
        } else {
            v.to_lowercase()
        };
        s.push_str(&format!(" {v}{g}"));
    }
    match p.qualifier {
        1 => s.push_str(" MIN"),
        2 => s.push_str(" MAX"),
        3 => s.push_str(" AVG"),
        _ => {}
    }
    for m in post {
        s.push(' ');
        s.push_str(m);
    }
    s.trim().to_owned()
}

/// A tolerance's modifier (XDE `GeomToleranceModif`) as written in its cell; all around and
/// all over go on the leader, not in the frame.
fn tolerance_modifier(c: u8) -> Option<&'static str> {
    Some(match c {
        0 => "ACS",
        1 => "CZ",
        2 => "ERE",
        3 => "Ⓕ",
        5 => "LE",
        6 => "MD",
        8 => "LD",
        9 => "NC",
        10 => "PD",
        11 => "Ⓡ",
        12 => "SEP REQT",
        13 => "<ST>",
        14 => "Ⓣ",
        _ => return None,
    })
}

/// A datum reference's modifier (XDE `DatumSingleModif`).
fn datum_modifier(c: u8) -> Option<&'static str> {
    Some(match c {
        0 => "ACS",
        1 => "ALS",
        2 => "[BSC]",
        3 => "[CF]",
        4 => "[u]",
        5 => "[v]",
        6 => "[w]",
        7 => "[x]",
        8 => "[y]",
        9 => "[z]",
        10 => "[DV]",
        11 => "Ⓕ",
        12 => "Ⓛ",
        13 => "[SL]",
        14 => "MD",
        15 => "Ⓜ",
        16 => "LD",
        17 => "><",
        18 => "PD",
        19 => "[PL]",
        20 => "[PT]",
        21 => "▷",
        _ => return None,
    })
}

/// A feature control frame's cells after the symbol: the tolerance, then each datum (one per
/// position; datums sharing a position are a common datum, "A-B").
pub fn frame_cells(p: &CadPmi) -> Vec<String> {
    let mut tol = String::new();
    match p.value_type {
        1 => tol.push('Ø'),
        2 => tol.push_str("SØ"),
        _ => {}
    }
    tol.push_str(&number(p.value, p.decimals.1));
    let mut extra: Vec<String> = Vec::new();
    match p.zone {
        1 if p.zone_value > 0.0 => extra.push(format!("Ⓟ {}", number(p.zone_value, 0))),
        1 => extra.push("Ⓟ".into()),
        3 => extra.push("UZ".into()),
        _ => {}
    }
    extra.extend(
        p.modifiers
            .iter()
            .filter_map(|&c| tolerance_modifier(c))
            .map(str::to_owned),
    );
    match p.material {
        1 => extra.push("Ⓜ".into()),
        2 => extra.push("Ⓛ".into()),
        _ => {}
    }
    if p.max_value > 0.0 {
        extra.push(format!("/ {} MAX", number(p.max_value, 0)));
    }
    for e in extra {
        tol.push(' ');
        tol.push_str(&e);
    }
    let mut cells = vec![tol];
    let mut by_pos: Vec<(u8, Vec<&CadDatumRef>)> = Vec::new();
    for d in &p.datums {
        match by_pos.iter_mut().find(|(k, _)| *k == d.position) {
            Some((_, v)) => v.push(d),
            None => by_pos.push((d.position, vec![d])),
        }
    }
    by_pos.sort_by_key(|(k, _)| *k);
    for (_, ds) in by_pos {
        let cell = ds
            .iter()
            .map(|d| {
                let mut s = d.letter.clone();
                for m in d.modifiers.iter().filter_map(|&c| datum_modifier(c)) {
                    s.push(' ');
                    s.push_str(m);
                }
                s
            })
            .collect::<Vec<_>>()
            .join("-");
        cells.push(cell);
    }
    cells
}

/// Mesh triangles of each MESH record: where it starts in the mesh [`crate::occt_dec`] built
/// (the records grouped by colour, in first-seen order).
pub(crate) fn record_starts(model: &CadModel) -> Vec<u32> {
    let mut groups: Vec<(Option<[u32; 4]>, Vec<usize>)> = Vec::new();
    for (i, m) in model.meshes.iter().enumerate() {
        let key = m.colour.map(|c| c.map(f32::to_bits));
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, v)) => v.push(i),
            None => groups.push((key, vec![i])),
        }
    }
    let mut starts = vec![0u32; model.meshes.len()];
    let mut at = 0u32;
    for (_, members) in &groups {
        for &i in members {
            starts[i] = at;
            at += (model.meshes[i].indices.len() / 3) as u32;
        }
    }
    starts
}

fn place(p: [f32; 3], scale: f32, up: Up) -> [f32; 3] {
    canonical(p.map(|v| v * scale), up)
}

/// An affine map of model coordinates: `m * p + t`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Xf {
    m: [[f64; 3]; 3],
    t: [f64; 3],
}

impl Xf {
    fn point(&self, p: [f32; 3]) -> [f32; 3] {
        let p = p.map(f64::from);
        std::array::from_fn(|r| {
            (self.m[r][0] * p[0] + self.m[r][1] * p[1] + self.m[r][2] * p[2] + self.t[r]) as f32
        })
    }

    fn vector(&self, v: [f32; 3]) -> [f32; 3] {
        let v = v.map(f64::from);
        std::array::from_fn(|r| {
            (self.m[r][0] * v[0] + self.m[r][1] * v[1] + self.m[r][2] * v[2]) as f32
        })
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `c` (three columns) inverted, `None` when singular.
fn inverse(c: [[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    // Rows of the inverse of the matrix whose columns are c[0..3].
    let (r0, r1, r2) = (cross(c[1], c[2]), cross(c[2], c[0]), cross(c[0], c[1]));
    let det = dot(c[0], r0);
    (det.abs() > 1e-18).then(|| [r0, r1, r2].map(|r| r.map(|v| v / det)))
}

/// The map taking each `pairs[i].0` to `pairs[i].1`: from four well-spread pairs (three and
/// the normal when they lie in a plane), checked on all.
fn fit(pairs: &[([f64; 3], [f64; 3])]) -> Option<Xf> {
    let (p0, _) = *pairs.first()?;
    let far = |from: &dyn Fn([f64; 3]) -> f64| {
        pairs
            .iter()
            .copied()
            .max_by(|a, b| from(a.0).total_cmp(&from(b.0)))
    };
    let a = far(&|p| dot(sub(p, p0), sub(p, p0)))?;
    let u = sub(a.0, p0);
    let b = far(&|p| {
        let c = cross(u, sub(p, p0));
        dot(c, c)
    })?;
    let n = cross(u, sub(b.0, p0));
    let c = far(&|p| dot(n, sub(p, p0)).abs())?;
    let size = dot(u, u).sqrt();
    if size < 1e-12 || dot(n, n).sqrt() < 1e-9 * size * size {
        return None;
    }
    let q0 = pairs[0].1;
    let (pc, qc) = if dot(n, sub(c.0, p0)).abs() > 1e-6 * size * size {
        (sub(c.0, p0), sub(c.1, q0))
    } else {
        // Flat: the normals stand in (a proper rotation assumed).
        (n, cross(sub(a.1, q0), sub(b.1, q0)))
    };
    let inv = inverse([u, sub(b.0, p0), pc])?;
    let qcols = [sub(a.1, q0), sub(b.1, q0), qc];
    // m = Q · P⁻¹ (P, Q with the differences as columns).
    let m: [[f64; 3]; 3] = std::array::from_fn(|r| {
        std::array::from_fn(|k| (0..3).map(|j| qcols[j][r] * inv[j][k]).sum())
    });
    let t: [f64; 3] = std::array::from_fn(|r| q0[r] - dot(m[r], p0));
    let xf = Xf { m, t };
    let tol = 1e-4 * size.max(1.0);
    pairs
        .iter()
        .all(|(p, q)| {
            let r = xf.point(p.map(|v| v as f32)).map(f64::from);
            dot(sub(r, *q), sub(r, *q)).sqrt() <= tol
        })
        .then_some(xf)
}

/// The vertices of a face as written (its triangles' vertices, first to last), of one MESH
/// record.
fn face_vertices(model: &CadModel, f: &[u32; 3]) -> Option<(u32, u32)> {
    let m = model.meshes.get(f[0] as usize)?;
    let lo = f[1] as usize * 3;
    let hi = lo.checked_add(f[2] as usize * 3)?;
    let ix = m.indices.get(lo..hi)?;
    Some((*ix.iter().min()?, *ix.iter().max()?))
}

/// The copies of `p`, one per part instance its faces are on: (its faces there, the move from
/// where the file puts it). One copy, unmoved, when its faces are on one instance or none,
/// or when the instances can't be matched.
fn instance_copies(model: &CadModel, p: &CadPmi) -> Vec<(Vec<[u32; 3]>, Option<Xf>)> {
    let whole = || vec![(p.faces.clone(), None)];
    let mut groups: Vec<(u32, Vec<[u32; 3]>)> = Vec::new();
    for f in &p.faces {
        let Some(m) = model.meshes.get(f[0] as usize) else {
            continue;
        };
        match groups.iter_mut().find(|g| g.0 == m.node) {
            Some(g) => g.1.push(*f),
            None => groups.push((m.node, vec![*f])),
        }
    }
    if groups.len() < 2 || groups.iter().any(|g| g.1.len() != groups[0].1.len()) {
        return whole();
    }
    // Where the annotation is, to find the instance it was written for.
    let at = if p.flags & PMI_TEXT != 0 {
        Some(p.text)
    } else if let Some(q) = p.polylines.iter().flatten().next() {
        Some(*q)
    } else if p.flags & PMI_PLANE != 0 {
        Some([p.plane[0], p.plane[1], p.plane[2]])
    } else if p.flags & PMI_POINT != 0 {
        Some(p.point)
    } else {
        None
    };
    let verts = |faces: &[[u32; 3]]| -> Vec<[f64; 3]> {
        let mut out = Vec::new();
        for f in faces {
            let (Some((lo, hi)), Some(m)) =
                (face_vertices(model, f), model.meshes.get(f[0] as usize))
            else {
                continue;
            };
            out.extend(
                m.positions
                    .get(lo as usize..=hi as usize)
                    .unwrap_or(&[])
                    .iter()
                    .map(|q| q.map(f64::from)),
            );
        }
        out
    };
    let reference = match at {
        None => 0,
        Some(a) => {
            let a = a.map(f64::from);
            let dist = |faces: &[[u32; 3]]| {
                verts(faces)
                    .iter()
                    .map(|q| dot(sub(*q, a), sub(*q, a)))
                    .fold(f64::INFINITY, f64::min)
            };
            (0..groups.len())
                .min_by(|&x, &y| dist(&groups[x].1).total_cmp(&dist(&groups[y].1)))
                .unwrap_or(0)
        }
    };
    let ref_faces = &groups[reference].1;
    let mut out = Vec::with_capacity(groups.len());
    for (k, (_, faces)) in groups.iter().enumerate() {
        if k == reference {
            out.push((faces.clone(), None));
            continue;
        }
        // Matching vertices, face by face (at most a few thousand).
        let mut pairs = Vec::new();
        let mut same = true;
        for (fr, fk) in ref_faces.iter().zip(faces) {
            let (Some(a), Some(b)) = (face_vertices(model, fr), face_vertices(model, fk)) else {
                same = false;
                break;
            };
            if a.1 - a.0 != b.1 - b.0 {
                same = false;
                break;
            }
            let (ma, mb) = (&model.meshes[fr[0] as usize], &model.meshes[fk[0] as usize]);
            let n = (a.1 - a.0 + 1) as usize;
            let step = (n / 64).max(1);
            for o in (0..n).step_by(step) {
                if let (Some(x), Some(y)) = (
                    ma.positions.get(a.0 as usize + o),
                    mb.positions.get(b.0 as usize + o),
                ) {
                    pairs.push((x.map(f64::from), y.map(f64::from)));
                }
            }
            if pairs.len() > 4096 {
                break;
            }
        }
        match fit(&pairs).filter(|_| same) {
            Some(xf) => out.push((faces.clone(), Some(xf))),
            None => return whole(),
        }
    }
    out
}

/// Each PMI record's copies, one per part instance it is on ([`instance_copies`]).
pub(crate) type Copies = Vec<Vec<(Vec<[u32; 3]>, Option<Xf>)>>;

/// The copies of every PMI record of `model`, found while it still has its meshes (the
/// decoder hands them on to the viewer's mesh before [`from_model`] places the PMI).
pub(crate) fn instance_split(model: &CadModel) -> Copies {
    model
        .pmi
        .iter()
        .map(|p| instance_copies(model, p))
        .collect()
}

/// `p` moved by `xf`, on `faces`.
fn moved(p: &CadPmi, faces: Vec<[u32; 3]>, xf: Option<Xf>) -> CadPmi {
    let mut q = CadPmi {
        faces: Vec::new(),
        polylines: Vec::new(),
        ..p.clone()
    };
    q.faces = faces;
    let Some(x) = xf else {
        q.polylines = p.polylines.clone();
        return q;
    };
    q.text = x.point(p.text);
    q.point = x.point(p.point);
    q.point2 = x.point(p.point2);
    let o = x.point([p.plane[0], p.plane[1], p.plane[2]]);
    let d = x.vector([p.plane[3], p.plane[4], p.plane[5]]);
    let n = x.vector([p.plane[6], p.plane[7], p.plane[8]]);
    q.plane = [o[0], o[1], o[2], d[0], d[1], d[2], n[0], n[1], n[2]];
    q.polylines = p
        .polylines
        .iter()
        .map(|l| l.iter().map(|&v| x.point(v)).collect())
        .collect();
    q
}

/// The PMI and views of `model`, in the frame of `mesh` (built from the model's MESH records
/// with coordinates times `scale`; `starts` from [`record_starts`]). Each record comes once
/// per part instance: as `split` says ([`instance_split`], when `model` no longer has its
/// meshes), else as found from `model`'s meshes.
pub(crate) fn from_model(
    model: &CadModel,
    mesh: &Mesh,
    scale: f32,
    starts: &[u32],
    split: Option<Copies>,
) -> Pmi {
    let up = mesh.up;
    let mut items = Vec::with_capacity(model.pmi.len());
    // The items each PMI record became (one per part instance).
    let mut copies_of: Vec<Vec<usize>> = Vec::with_capacity(model.pmi.len());
    let copies: Vec<CadPmi> = model
        .pmi
        .iter()
        .enumerate()
        .flat_map(|(i, p)| {
            let c = split
                .as_ref()
                .and_then(|s| s.get(i).cloned())
                .unwrap_or_else(|| instance_copies(model, p));
            let first = copies_of.iter().map(Vec::len).sum::<usize>();
            copies_of.push((first..first + c.len()).collect());
            c.into_iter()
                .map(|(faces, xf)| moved(p, faces, xf))
                .collect::<Vec<_>>()
        })
        .collect();
    for p in &copies {
        let mut triangles: Vec<(u32, u32)> = p
            .faces
            .iter()
            .filter_map(|f| Some((starts.get(f[0] as usize)? + f[1], f[2])))
            .filter(|r| r.1 > 0)
            .collect();
        triangles.sort_unstable();
        triangles.dedup();
        // The faces' box, and their vertex nearest the label for the leader.
        let mut bounds = Bounds::EMPTY;
        let mut verts: Vec<[f32; 3]> = Vec::new();
        for &(first, count) in &triangles {
            let lo = first as usize * 3;
            let hi = (lo + count as usize * 3).min(mesh.indices.len());
            for &i in mesh.indices.get(lo..hi).unwrap_or(&[]) {
                if let Some(&v) = mesh.positions.get(i as usize) {
                    let q = canonical(v, up);
                    bounds.add(q);
                    if verts.len() < 200_000 {
                        verts.push(q);
                    }
                }
            }
        }
        let polylines: Vec<Vec<[f32; 3]>> = p
            .polylines
            .iter()
            .map(|l| l.iter().map(|&q| place(q, scale, up)).collect())
            .collect();
        let mut drawn = Bounds::EMPTY;
        for q in polylines.iter().flatten() {
            drawn.add(*q);
        }
        if bounds.is_empty() {
            bounds = drawn;
        }
        let anchor = if p.flags & PMI_TEXT != 0 {
            Some(place(p.text, scale, up))
        } else if !drawn.is_empty() {
            Some(centre(&drawn))
        } else if p.flags & PMI_PLANE != 0 {
            Some(place([p.plane[0], p.plane[1], p.plane[2]], scale, up))
        } else if p.flags & PMI_POINT != 0 {
            Some(place(p.point, scale, up))
        } else if !bounds.is_empty() {
            Some(centre(&bounds))
        } else {
            None
        };
        let target = match anchor {
            Some(a) if !verts.is_empty() => verts
                .iter()
                .copied()
                .min_by(|x, y| dist2(*x, a).total_cmp(&dist2(*y, a))),
            _ if p.flags & PMI_POINT != 0 => Some(place(p.point, scale, up)),
            _ => None,
        };
        let (dim, characteristic, label) = match p.kind {
            PmiKind::Dimension => (
                Some(DimKind::from_code(p.code)),
                None,
                Label::Text(dimension_text(p)),
            ),
            PmiKind::GeometricTolerance => {
                let c = Characteristic::from_code(p.code);
                (
                    None,
                    c,
                    Label::Frame {
                        characteristic: c,
                        cells: frame_cells(p),
                    },
                )
            }
            PmiKind::Datum => (None, None, Label::Datum(p.letter.clone())),
            PmiKind::DatumTarget => (
                None,
                None,
                Label::Datum(format!("{}{}", p.letter, p.target_number)),
            ),
            PmiKind::Graphic => (None, None, Label::Drawing),
        };
        let text = match &label {
            Label::Text(s) | Label::Datum(s) => s.clone(),
            Label::Frame {
                characteristic,
                cells,
            } => {
                // The symbol is drawn, and named in words by the app.
                let _ = characteristic;
                cells.join(" | ")
            }
            Label::Drawing => p.name.clone(),
        };
        items.push(PmiItem {
            kind: p.kind,
            dim,
            characteristic,
            name: p.name.clone(),
            label,
            text,
            anchor,
            target,
            polylines,
            triangles,
            bounds,
        });
    }
    let views = model
        .views
        .iter()
        .map(|v| PmiView {
            name: v.name.clone(),
            direction: canonical(v.direction, up).map(f64::from),
            up: canonical(v.up, up).map(f64::from),
            orthographic: v.projection != 1,
            items: v
                .pmi
                .iter()
                .flat_map(|&i| copies_of.get(i as usize).cloned().unwrap_or_default())
                .collect(),
        })
        .collect();
    Pmi { items, views }
}

/// `mesh` with the triangles of `ranges` (first, count) in a material of their own (`colour`,
/// linear RGBA), drawn last; the rest as they were.
pub fn highlighted(mesh: &Mesh, ranges: &[(u32, u32)], colour: [f32; 4]) -> Mesh {
    let tris = mesh.indices.len() / 3;
    let mut lit = vec![false; tris];
    for &(first, count) in ranges {
        let end = (first as usize).saturating_add(count as usize).min(tris);
        for l in lit.iter_mut().take(end).skip(first as usize) {
            *l = true;
        }
    }
    let mut out = mesh.clone();
    out.indices.clear();
    out.parts.clear();
    let put = |out: &mut Mesh, t: usize| {
        out.indices
            .extend_from_slice(&mesh.indices[3 * t..3 * t + 3]);
    };
    let all = [specular_mesh::Part {
        first: 0,
        count: tris as u32,
        material: None,
    }];
    let parts = if mesh.parts.is_empty() {
        &all[..]
    } else {
        &mesh.parts[..]
    };
    for p in parts {
        let first = (out.indices.len() / 3) as u32;
        let end = ((p.first + p.count) as usize).min(tris);
        for (t, &l) in lit.iter().enumerate().take(end).skip(p.first as usize) {
            if !l {
                put(&mut out, t);
            }
        }
        let count = (out.indices.len() / 3) as u32 - first;
        if count > 0 {
            out.parts.push(specular_mesh::Part {
                first,
                count,
                material: p.material,
            });
        }
    }
    let first = (out.indices.len() / 3) as u32;
    for (t, &l) in lit.iter().enumerate() {
        if l {
            put(&mut out, t);
        }
    }
    let count = (out.indices.len() / 3) as u32 - first;
    if count > 0 {
        out.materials.push(specular_mesh::Material {
            name: "PMI".into(),
            base_color: colour,
            texture: None,
        });
        out.parts.push(specular_mesh::Part {
            first,
            count,
            material: Some((out.materials.len() - 1) as u32),
        });
    }
    out
}

fn centre(b: &Bounds) -> [f32; 3] {
    std::array::from_fn(|k| (b.min[k] + b.max[k]) / 2.0)
}

fn dist2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum()
}

/// The info panel's PMI facts (field, value).
pub(crate) fn facts(pmi: &Pmi) -> Vec<(String, String)> {
    let [dims, tols, datums, targets, drawn] = pmi.counts();
    let mut out = Vec::new();
    for (k, n) in [
        ("PMI dimensions", dims),
        ("PMI geometric tolerances", tols),
        ("PMI datums", datums),
        ("PMI datum targets", targets),
        ("PMI drawn only", drawn),
        ("Saved views", pmi.views.len()),
    ] {
        if n > 0 {
            out.push((k.to_owned(), n.to_string()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_sandbox::cad::CadMesh;

    fn tol() -> CadPmi {
        CadPmi {
            kind: PmiKind::GeometricTolerance,
            code: 10,
            flags: PMI_VALUE,
            value: 0.25,
            value_type: 1,
            material: 1,
            datums: vec![
                CadDatumRef {
                    letter: "A".into(),
                    position: 1,
                    ..CadDatumRef::default()
                },
                CadDatumRef {
                    letter: "C".into(),
                    position: 3,
                    modifiers: vec![15],
                    ..CadDatumRef::default()
                },
                CadDatumRef {
                    letter: "B".into(),
                    position: 2,
                    ..CadDatumRef::default()
                },
                CadDatumRef {
                    letter: "D".into(),
                    position: 2,
                    ..CadDatumRef::default()
                },
            ],
            ..CadPmi::default()
        }
    }

    #[test]
    fn feature_control_frame() {
        assert_eq!(frame_cells(&tol()), ["Ø0.25 Ⓜ", "A", "B-D", "C Ⓜ"]);
        let mut t = tol();
        t.value_type = 0;
        t.material = 0;
        t.zone = 1;
        t.zone_value = 12.0;
        t.modifiers = vec![3, 15, 14];
        t.datums.clear();
        assert_eq!(frame_cells(&t), ["0.25 Ⓟ 12 Ⓕ Ⓣ"]);
        assert_eq!(Characteristic::from_code(10).map(|c| c.symbol()), Some('⌖'));
        assert_eq!(Characteristic::from_code(0), None);
    }

    #[test]
    fn dimension_texts() {
        let d = |code, flags, value, upper, lower| CadPmi {
            code,
            flags,
            value,
            upper,
            lower,
            ..CadPmi::default()
        };
        let pm = PMI_VALUE | PMI_PLUS_MINUS;
        assert_eq!(dimension_text(&d(15, pm, 35.0, 0.0, 0.2)), "Ø35 +0/−0.2");
        assert_eq!(dimension_text(&d(15, pm, 25.0, 0.15, 0.15)), "Ø25 ±0.15");
        assert_eq!(dimension_text(&d(11, pm, 60.0, 0.5, 0.5)), "60° ±0.5°");
        assert_eq!(dimension_text(&d(17, PMI_VALUE, 3.5, 0.0, 0.0)), "R3.5");
        assert_eq!(
            dimension_text(&d(2, PMI_VALUE | PMI_RANGE, 35.0, 35.2, 34.8)),
            "34.8 – 35.2"
        );
        let mut f = d(15, PMI_VALUE | PMI_FIT, 20.0, 0.0, 0.0);
        (f.fit_variance, f.fit_grade, f.fit_hole) = (10, 7, true);
        assert_eq!(dimension_text(&f), "Ø20 G6");
        f.fit_hole = false;
        f.decimals = (0, 2);
        f.modifiers = vec![4];
        assert_eq!(dimension_text(&f), "Ø20.00 g6 (LP)");
        assert_eq!(number(-0.00001, 0), "0");
        assert_eq!(number(-1.5, 0), "−1.5");
    }

    #[test]
    fn faces_follow_the_mesh_grouping() {
        // Two records in one colour around one in another: the mesh groups them by colour.
        let rec = |colour: Option<[f32; 4]>, tris: usize| CadMesh {
            node: 0,
            colour,
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            indices: [0, 1, 2].repeat(tris),
        };
        let red = Some([1.0, 0.0, 0.0, 1.0]);
        let model = CadModel {
            meshes: vec![rec(red, 2), rec(None, 3), rec(red, 4)],
            pmi: vec![CadPmi {
                kind: PmiKind::Datum,
                letter: "A".into(),
                flags: PMI_TEXT,
                text: [5.0, 0.0, 0.0],
                faces: vec![[2, 1, 2], [1, 0, 3]],
                ..CadPmi::default()
            }],
            ..CadModel::default()
        };
        assert_eq!(record_starts(&model), vec![0, 6, 2]);
        let mesh = Mesh {
            positions: vec![[0.0; 3], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]],
            indices: [0, 1, 2].repeat(9),
            up: Up::Z,
            ..Mesh::default()
        };
        let p = from_model(&model, &mesh, 2.0, &record_starts(&model), None);
        let it = &p.items[0];
        assert_eq!(it.triangles, vec![(3, 2), (6, 3)]);
        assert_eq!(it.anchor, Some([10.0, 0.0, 0.0]));
        assert_eq!(it.target, Some([2.0, 0.0, 0.0]));
        assert_eq!(it.label, Label::Datum("A".into()));
        assert_eq!(p.counts(), [0, 0, 1, 0, 0]);
        assert_eq!(facts(&p), vec![("PMI datums".to_owned(), "1".to_owned())]);
    }

    /// A tetrahedron part placed twice: as written, and turned 90° about z and moved 10 in x;
    /// a datum on two of its faces written near the first instance.
    #[test]
    fn pmi_follows_each_part_instance() {
        let tet = [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 3.0, 0.0],
            [0.0, 0.0, 4.0],
        ];
        let turn = |p: [f32; 3]| [10.0 - p[1], p[0], p[2]];
        let rec = |node: u32, f: &dyn Fn([f32; 3]) -> [f32; 3]| CadMesh {
            node,
            colour: None,
            positions: tet.iter().map(|&p| f(p)).collect(),
            normals: vec![[0.0, 0.0, 1.0]; 4],
            indices: vec![0, 2, 1, 0, 1, 3, 0, 3, 2, 1, 2, 3],
        };
        let datum = CadPmi {
            kind: PmiKind::Datum,
            letter: "A".into(),
            flags: PMI_TEXT | PMI_PLANE,
            text: [1.0, 1.0, 6.0],
            plane: [1.0, 1.0, 6.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0],
            polylines: vec![vec![[1.0, 1.0, 6.0], [2.0, 1.0, 6.0]]],
            // Faces 0 and 1 (triangles 0..2) of each instance.
            faces: vec![[0, 0, 2], [1, 0, 2]],
            ..CadPmi::default()
        };
        let model = CadModel {
            meshes: vec![rec(1, &|p| p), rec(2, &turn)],
            pmi: vec![datum],
            views: vec![specular_sandbox::cad::CadView {
                name: "V".into(),
                direction: [0.0, 0.0, -1.0],
                up: [0.0, 1.0, 0.0],
                pmi: vec![0],
                ..Default::default()
            }],
            ..CadModel::default()
        };
        let starts = record_starts(&model);
        let mesh = Mesh {
            positions: model
                .meshes
                .iter()
                .flat_map(|m| m.positions.clone())
                .collect(),
            indices: model
                .meshes
                .iter()
                .enumerate()
                .flat_map(|(k, m)| m.indices.iter().map(move |i| i + 4 * k as u32))
                .collect(),
            up: Up::Z,
            ..Mesh::default()
        };
        let p = from_model(&model, &mesh, 1.0, &starts, None);
        assert_eq!(p.items.len(), 2, "one per instance");
        let (a, b) = (&p.items[0], &p.items[1]);
        assert_eq!(a.anchor, Some([1.0, 1.0, 6.0]));
        // Turned and moved with the second instance; its faces only.
        let close = |x: [f32; 3], y: [f32; 3]| (0..3).all(|k| (x[k] - y[k]).abs() < 1e-4);
        assert!(close(b.anchor.unwrap(), [9.0, 1.0, 6.0]), "{:?}", b.anchor);
        assert!(
            close(b.polylines[0][1], [9.0, 2.0, 6.0]),
            "{:?}",
            b.polylines
        );
        assert_eq!(a.triangles, vec![(0, 2)]);
        assert_eq!(b.triangles, vec![(4, 2)]);
        assert!(b.bounds.min[0] >= 7.0 - 1e-4);
        assert!(close(b.target.unwrap(), turn(a.target.unwrap())));
        // The view shows both copies.
        assert_eq!(p.views[0].items, vec![0, 1]);
        // Written near the second instance instead: that one stays, the first moves back.
        let mut m2 = model.clone();
        m2.pmi[0].text = turn([1.0, 1.0, 6.0]);
        m2.pmi[0].flags = PMI_TEXT;
        m2.pmi[0].polylines.clear();
        let p2 = from_model(&m2, &mesh, 1.0, &starts, None);
        assert!(close(p2.items[0].anchor.unwrap(), [1.0, 1.0, 6.0]));
        assert!(close(p2.items[1].anchor.unwrap(), [9.0, 1.0, 6.0]));
        // Faces on one instance only: one item, as written.
        let mut m3 = model.clone();
        m3.pmi[0].faces.truncate(1);
        assert_eq!(from_model(&m3, &mesh, 1.0, &starts, None).items.len(), 1);
    }

    #[test]
    fn highlight_moves_triangles_to_their_own_part() {
        let mesh = Mesh {
            positions: vec![[0.0; 3]; 3],
            indices: (0..5).flat_map(|t| [t, t, t]).collect(),
            parts: vec![
                specular_mesh::Part {
                    first: 0,
                    count: 3,
                    material: Some(0),
                },
                specular_mesh::Part {
                    first: 3,
                    count: 2,
                    material: None,
                },
            ],
            ..Mesh::default()
        };
        let h = highlighted(&mesh, &[(1, 3)], [1.0, 0.5, 0.0, 1.0]);
        assert_eq!(h.indices, [0, 0, 0, 4, 4, 4, 1, 1, 1, 2, 2, 2, 3, 3, 3]);
        let parts: Vec<_> = h
            .parts
            .iter()
            .map(|p| (p.first, p.count, p.material))
            .collect();
        assert_eq!(parts, [(0, 1, Some(0)), (1, 1, None), (2, 3, Some(0))]);
        assert_eq!(h.materials.len(), 1);
    }
}
