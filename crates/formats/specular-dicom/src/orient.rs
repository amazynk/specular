//! Patient orientation of a frame (PS3.3 C.7.6.2.1.1 and C.7.6.1.1.1): where the image's rows
//! and columns point in the patient (LPS: x to the patient's left, y to posterior, z to the
//! head), and the orientation labels (L R A P S I, combined for oblique directions) of any
//! direction in the image plane.
//!
//! Veterinary images whose Anatomical Orientation Type (0010,2210) is QUADRUPED keep the same
//! axes (C.7.6.2.1.1: x to the animal's left, y dorsal, z cranial) but name them as
//! C.7.6.1.1.1 does: Le / Rt, D / V, Cr / Cd; on the head y is D / V and z rostral (R) / Cd;
//! on proximal limbs y is Cr / Cd and z proximal (Pr) / distal (Di); on distal limbs y is
//! D / palmar (Pa, forelimb) or plantar (Pl, hindlimb) and z Pr / Di. The body region comes
//! from Body Part Examined (0018,0015).

use crate::display::functional;
use crate::parse::{DataSet, Values};

/// Where a frame's axes point in the patient's LPS coordinates (unit vectors).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PatientAxes {
    /// Direction of increasing column index (along a row, to the right of the stored image).
    pub row: [f64; 3],
    /// Direction of increasing row index (down the stored image).
    pub col: [f64; 3],
    /// From Patient Orientation (0020,0020) letters, not direction cosines.
    pub from_letters: bool,
    /// Whose anatomy the letters name.
    pub anatomy: Anatomy,
}

/// The body plan the orientation letters name (Anatomical Orientation Type, 0010,2210).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Anatomy {
    /// Absent or BIPED: L R A P S I.
    #[default]
    Biped,
    /// QUADRUPED, with the body region the letters depend on.
    Quadruped(Region),
}

/// A quadruped's body region, which changes the names of the y and z directions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Region {
    /// Neck, trunk and tail (and anything not named otherwise).
    #[default]
    Trunk,
    Head,
    /// Above the carpus or tarsus.
    ProximalLimb,
    /// Carpus and below.
    DistalForelimb,
    /// Tarsus and below.
    DistalHindlimb,
}

impl Region {
    /// The region Body Part Examined names (DICOM and veterinary terms, any case).
    pub fn of(body_part: &str) -> Region {
        let b = body_part.to_ascii_uppercase();
        let has = |words: &[&str]| words.iter().any(|w| b.contains(w));
        if has(&[
            "CARPUS",
            "CARPAL",
            "MANUS",
            "FOREPAW",
            "FORE PAW",
            "FOREFOOT",
            "FORE FOOT",
        ]) {
            Region::DistalForelimb
        } else if has(&[
            "TARSUS",
            "TARSAL",
            "PES",
            "HINDPAW",
            "HIND PAW",
            "HINDFOOT",
            "HIND FOOT",
            "HOCK",
        ]) {
            Region::DistalHindlimb
        } else if has(&[
            "SHOULDER",
            "HUMERUS",
            "ELBOW",
            "RADIUS",
            "ULNA",
            "ANTEBRACH",
            "FOREARM",
            "FEMUR",
            "STIFLE",
            "KNEE",
            "TIBIA",
            "FIBULA",
            "CRUS",
            "LIMB",
            "EXTREMITY",
        ]) {
            Region::ProximalLimb
        } else if has(&[
            "HEAD", "SKULL", "BRAIN", "NOSE", "NASAL", "MAXILLA", "MANDIBLE", "TMJ", "EAR",
            "TEETH", "TOOTH", "DENTAL", "ORBIT", "EYE", "FACE", "SINUS",
        ]) {
            Region::Head
        } else {
            Region::Trunk
        }
    }
}

impl Anatomy {
    /// From Anatomical Orientation Type (0010,2210) and Body Part Examined (0018,0015).
    pub(crate) fn read(ds: &DataSet, v: &Values) -> Anatomy {
        let text = |tag| {
            ds.get(tag)
                .and_then(|e| v.text_n(e, 0))
                .map(|s| s.trim().to_ascii_uppercase())
        };
        Anatomy::of(text(0x0010_2210).as_deref(), text(0x0018_0015).as_deref())
    }

    /// From the text of Anatomical Orientation Type (0010,2210) and Body Part Examined
    /// (0018,0015), as a header or a DICOMweb answer gives them.
    pub fn of(orientation_type: Option<&str>, body_part: Option<&str>) -> Anatomy {
        let upper = |s: &str| s.trim().to_ascii_uppercase();
        match orientation_type.map(upper).as_deref() {
            Some("QUADRUPED") => Anatomy::Quadruped(
                body_part
                    .map(upper)
                    .map_or(Region::Trunk, |b| Region::of(&b)),
            ),
            _ => Anatomy::Biped,
        }
    }

    /// The names of the +x, -x, +y, -y, +z and -z directions.
    pub fn names(self) -> [&'static str; 6] {
        match self {
            Anatomy::Biped => ["L", "R", "P", "A", "S", "I"],
            Anatomy::Quadruped(r) => {
                let (y, z): ([&str; 2], [&str; 2]) = match r {
                    Region::Trunk => (["D", "V"], ["Cr", "Cd"]),
                    Region::Head => (["D", "V"], ["R", "Cd"]),
                    Region::ProximalLimb => (["Cr", "Cd"], ["Pr", "Di"]),
                    Region::DistalForelimb => (["D", "Pa"], ["Pr", "Di"]),
                    Region::DistalHindlimb => (["D", "Pl"], ["Pr", "Di"]),
                };
                ["Le", "Rt", y[0], y[1], z[0], z[1]]
            }
        }
    }

    /// The label of an LPS direction (see [`label`]) in this anatomy's names.
    pub fn label(self, v: [f64; 3]) -> Option<String> {
        let v = norm(v)?;
        let n = self.names();
        let mut parts: Vec<(f64, &str)> = v
            .iter()
            .enumerate()
            .map(|(i, &c)| (c.abs(), if c >= 0.0 { n[2 * i] } else { n[2 * i + 1] }))
            .filter(|(m, _)| *m >= OBLIQUE_TOLERANCE)
            .collect();
        parts.sort_by(|a, b| b.0.total_cmp(&a.0));
        Some(parts.into_iter().map(|(_, c)| c).collect())
    }
}

/// Components smaller than this (sin 10°) are left out of a label, so a plane tilted by less
/// than about 10° is labelled by its main axis alone.
pub const OBLIQUE_TOLERANCE: f64 = 0.173_648_177_666_930_3;

fn norm(v: [f64; 3]) -> Option<[f64; 3]> {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    (l.is_finite() && l > 1e-6).then(|| [v[0] / l, v[1] / l, v[2] / l])
}

/// The label of an LPS direction: the letter of each component at least
/// [`OBLIQUE_TOLERANCE`] long, largest first (`"L"`, `"RA"`, `"PFL"`… with S and I for head
/// and feet). `None` for a zero vector.
pub fn label(v: [f64; 3]) -> Option<String> {
    Anatomy::Biped.label(v)
}

/// One quadruped Patient Orientation value (C.7.6.1.1.1: "LE", "CRD", "DIPA"…) as a
/// direction. Medial and lateral depend on the side imaged: `None`.
fn quadruped_letters(s: &str) -> Option<[f64; 3]> {
    let mut v = [0.0; 3];
    let mut w = 1.0;
    let mut rest = s.trim().to_ascii_uppercase();
    while !rest.is_empty() {
        // Two-letter codes first (CR before R, DI before D…).
        let (code, axis, sign) = [
            ("LE", 0, 1.0),
            ("RT", 0, -1.0),
            ("CR", 2, 1.0),
            ("CD", 2, -1.0),
            ("PR", 2, 1.0),
            ("DI", 2, -1.0),
            ("PA", 1, -1.0),
            ("PL", 1, -1.0),
            ("D", 1, 1.0),
            ("V", 1, -1.0),
            ("R", 2, 1.0),
        ]
        .into_iter()
        .find(|(c, _, _)| rest.starts_with(c))?;
        v[axis] += sign * w;
        w *= 0.6;
        rest.drain(..code.len());
    }
    norm(v)
}

/// One Patient Orientation value ("A", "FL", "HP"…) as a direction: each letter a unit axis,
/// later letters weighted less so the label reads back in the same order.
fn letters(s: &str) -> Option<[f64; 3]> {
    let mut v = [0.0; 3];
    let mut w = 1.0;
    for c in s.trim().chars() {
        let (i, sign) = match c.to_ascii_uppercase() {
            'L' => (0, 1.0),
            'R' => (0, -1.0),
            'P' => (1, 1.0),
            'A' => (1, -1.0),
            'H' => (2, 1.0),
            'F' => (2, -1.0),
            _ => return None,
        };
        v[i] += sign * w;
        w *= 0.6;
    }
    norm(v)
}

impl PatientAxes {
    /// From Image Orientation (Patient) (0020,0037) of the frame (per-frame Plane Orientation,
    /// else shared, else the image's), else Patient Orientation (0020,0020).
    pub(crate) fn read(ds: &DataSet, v: &Values, frame: u32) -> Option<PatientAxes> {
        let anatomy = Anatomy::read(ds, v);
        let src = functional(ds, frame, 0x0020_9116)
            .filter(|d| d.get(0x0020_0037).is_some())
            .unwrap_or(ds);
        if let Some(e) = src.get(0x0020_0037) {
            let n: Vec<f64> = (0..6).filter_map(|i| v.number(e, i)).collect();
            if let [a, b, c, d, e, f] = n[..]
                && let (Some(row), Some(col)) = (norm([a, b, c]), norm([d, e, f]))
            {
                return Some(PatientAxes {
                    row,
                    col,
                    from_letters: false,
                    anatomy,
                });
            }
        }
        let e = ds.get(0x0020_0020)?;
        let (r, c) = (v.text_n(e, 0)?, v.text_n(e, 1)?);
        let read = if anatomy == Anatomy::Biped {
            letters
        } else {
            quadruped_letters
        };
        Some(PatientAxes {
            row: read(r)?,
            col: read(c)?,
            from_letters: true,
            anatomy,
        })
    }

    /// The patient direction of a direction (dx, dy) in stored pixel coordinates.
    pub fn direction(&self, dx: f64, dy: f64) -> [f64; 3] {
        [0, 1, 2].map(|i| dx * self.row[i] + dy * self.col[i])
    }

    /// The label of the direction (dx, dy) in stored pixel coordinates (see [`label`]).
    pub fn label(&self, dx: f64, dy: f64) -> Option<String> {
        self.anatomy
            .label(self.direction(dx, dy))
            .filter(|s| !s.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_axes_and_obliques() {
        assert_eq!(label([1.0, 0.0, 0.0]).unwrap(), "L");
        assert_eq!(label([-1.0, 0.0, 0.0]).unwrap(), "R");
        assert_eq!(label([0.0, -1.0, 0.0]).unwrap(), "A");
        assert_eq!(label([0.0, 0.0, -1.0]).unwrap(), "I");
        // 5° off R towards A: within tolerance.
        let (s, c) = 5f64.to_radians().sin_cos();
        assert_eq!(label([-c, -s, 0.0]).unwrap(), "R");
        // 30°: oblique, the larger component first.
        let (s, c) = 30f64.to_radians().sin_cos();
        assert_eq!(label([-c, -s, 0.0]).unwrap(), "RA");
        assert_eq!(label([s, c, 0.0]).unwrap(), "PL");
        assert_eq!(label([0.6, -0.5, 0.62]).unwrap(), "SLA");
        assert_eq!(label([0.0; 3]), None);
    }

    #[test]
    fn axial_directions() {
        // Axial: rows to the patient's left, columns to posterior.
        let a = PatientAxes {
            row: [1.0, 0.0, 0.0],
            col: [0.0, 1.0, 0.0],
            from_letters: false,
            anatomy: Anatomy::Biped,
        };
        assert_eq!(a.label(1.0, 0.0).unwrap(), "L");
        assert_eq!(a.label(-1.0, 0.0).unwrap(), "R");
        assert_eq!(a.label(0.0, 1.0).unwrap(), "P");
        assert_eq!(a.label(0.0, -1.0).unwrap(), "A");
        // Screen rotated 45°: a diagonal of the image.
        assert_eq!(a.label(1.0, 1.0).unwrap(), "LP");
    }

    #[test]
    fn patient_orientation_letters() {
        let r = letters("A").unwrap();
        assert_eq!(label(r).unwrap(), "A");
        let c = letters("FL").unwrap();
        assert_eq!(label(c).unwrap(), "IL");
        let h = letters("HP").unwrap();
        assert_eq!(label([-h[0], -h[1], -h[2]]).unwrap(), "IA");
        assert!(letters("X").is_none());
    }

    /// The quadruped names of C.7.6.1.1.1 on the axes of C.7.6.2.1.1: x to the animal's left,
    /// y dorsal (cranial on proximal limbs), z cranial (rostral on the head, proximal on
    /// limbs).
    #[test]
    fn anatomy_from_header_text() {
        assert_eq!(Anatomy::of(None, Some("HEAD")), Anatomy::Biped);
        assert_eq!(Anatomy::of(Some("BIPED"), None), Anatomy::Biped);
        assert_eq!(
            Anatomy::of(Some(" quadruped "), None),
            Anatomy::Quadruped(Region::Trunk)
        );
        assert_eq!(
            Anatomy::of(Some("QUADRUPED"), Some("skull ")),
            Anatomy::Quadruped(Region::Head)
        );
    }

    #[test]
    fn quadruped_labels() {
        let q = Anatomy::Quadruped;
        let trunk = q(Region::Trunk);
        assert_eq!(trunk.label([1.0, 0.0, 0.0]).unwrap(), "Le");
        assert_eq!(trunk.label([-1.0, 0.0, 0.0]).unwrap(), "Rt");
        assert_eq!(trunk.label([0.0, 1.0, 0.0]).unwrap(), "D");
        assert_eq!(trunk.label([0.0, -1.0, 0.0]).unwrap(), "V");
        assert_eq!(trunk.label([0.0, 0.0, 1.0]).unwrap(), "Cr");
        assert_eq!(trunk.label([0.0, 0.0, -1.0]).unwrap(), "Cd");
        let (s, c) = 30f64.to_radians().sin_cos();
        assert_eq!(trunk.label([0.0, -s, c]).unwrap(), "CrV");
        assert_eq!(q(Region::Head).label([0.0, 0.0, 1.0]).unwrap(), "R");
        let prox = q(Region::ProximalLimb);
        assert_eq!(prox.label([0.0, 1.0, 0.0]).unwrap(), "Cr");
        assert_eq!(prox.label([0.0, 0.0, -1.0]).unwrap(), "Di");
        assert_eq!(
            q(Region::DistalForelimb).label([0.0, -1.0, 0.0]).unwrap(),
            "Pa"
        );
        assert_eq!(
            q(Region::DistalHindlimb).label([0.0, -1.0, 0.0]).unwrap(),
            "Pl"
        );
        assert_eq!(
            q(Region::DistalHindlimb).label([0.0, 0.0, 1.0]).unwrap(),
            "Pr"
        );
        assert_eq!(Region::of("CARPUS"), Region::DistalForelimb);
        assert_eq!(Region::of("pes"), Region::DistalHindlimb);
        assert_eq!(Region::of("STIFLE"), Region::ProximalLimb);
        assert_eq!(Region::of("SKULL"), Region::Head);
        assert_eq!(Region::of("THORAX"), Region::Trunk);
        // Patient Orientation codes: two-letter ones before one-letter ones.
        let v = quadruped_letters("CRD").unwrap();
        assert_eq!(trunk.label(v).unwrap(), "CrD");
        let v = quadruped_letters("LE").unwrap();
        assert_eq!(trunk.label(v).unwrap(), "Le");
        assert!(quadruped_letters("M").is_none());
    }
}
