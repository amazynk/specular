//! Inverse pole figure colouring.
//!
//! A sample direction is carried into the crystal frame by the orientation (Bunge Euler angles,
//! sample → crystal), folded into the standard sector of the phase's Laue group, and coloured by
//! where it lies in that spherical triangle: each corner owns one primary (red for the c axis
//! or [001], then green and blue), and a primary's weight is the direction's angular distance
//! from the opposite edge relative to its corner's. Square roots brighten the middle, and the
//! largest weight is scaled to one, so the centre of the triangle is white (the look of the
//! EDAX TSL key).
//!
//! Crystal frames follow the usual convention for EBSD files (and orix): Cartesian x along a,
//! z along c (c* for the low symmetries). For hexagonal crystals the sector of 6/mmm runs from
//! [0001] (red) over [2-1-10] (green, along x) to [10-10] (blue, 30° from x).

use std::f64::consts::PI;

/// The Laue group of a phase: what decides the standard sector of its IPF key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Symmetry {
    /// -1
    Triclinic,
    /// 2/m
    Monoclinic,
    /// mmm
    Orthorhombic,
    /// 4/m
    TetragonalLow,
    /// 4/mmm
    Tetragonal,
    /// -3
    TrigonalLow,
    /// -3m
    Trigonal,
    /// 6/m
    HexagonalLow,
    /// 6/mmm
    Hexagonal,
    /// m-3
    CubicLow,
    /// m-3m
    Cubic,
}

impl Symmetry {
    /// From the point group code of a TSL .ang header ("Symmetry 43"): 1, 2 (20, 21, 211…),
    /// 22, 4, 42, 3, 32, 6, 62, 23, 43.
    pub fn from_tsl(code: &str) -> Option<Symmetry> {
        let code = code.trim();
        Some(match code {
            "1" | "-1" => Symmetry::Triclinic,
            "2" | "20" | "21" | "211" | "121" | "112" => Symmetry::Monoclinic,
            "22" | "222" => Symmetry::Orthorhombic,
            "4" => Symmetry::TetragonalLow,
            "42" | "422" => Symmetry::Tetragonal,
            "3" => Symmetry::TrigonalLow,
            "32" | "321" | "312" => Symmetry::Trigonal,
            "6" => Symmetry::HexagonalLow,
            "62" | "622" => Symmetry::Hexagonal,
            "23" => Symmetry::CubicLow,
            "43" | "432" => Symmetry::Cubic,
            _ => return Symmetry::from_name(code),
        })
    }

    /// From the Laue group number of an Oxford .ctf phase line (1 = -1 … 9 = 6/mmm, 10 = m-3,
    /// 11 = m-3m).
    pub fn from_laue_id(id: u32) -> Option<Symmetry> {
        Some(match id {
            1 => Symmetry::Triclinic,
            2 => Symmetry::Monoclinic,
            3 => Symmetry::Orthorhombic,
            4 => Symmetry::TetragonalLow,
            5 => Symmetry::Tetragonal,
            6 => Symmetry::TrigonalLow,
            7 => Symmetry::Trigonal,
            8 => Symmetry::HexagonalLow,
            9 => Symmetry::Hexagonal,
            10 => Symmetry::CubicLow,
            11 => Symmetry::Cubic,
            _ => return None,
        })
    }

    /// From a Hermann–Mauguin point or Laue group name ("m-3m", "m3m", "6/mmm", "mmm"…).
    pub fn from_name(name: &str) -> Option<Symmetry> {
        let n: String = name
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_ascii_lowercase();
        Some(match n.as_str() {
            "1" | "-1" => Symmetry::Triclinic,
            "2" | "m" | "2/m" | "112" | "121" | "211" | "11m" | "1m1" | "m11" => {
                Symmetry::Monoclinic
            }
            "222" | "mm2" | "mmm" | "2mm" | "m2m" => Symmetry::Orthorhombic,
            "4" | "-4" | "4/m" => Symmetry::TetragonalLow,
            "422" | "4mm" | "-42m" | "-4m2" | "4/mmm" => Symmetry::Tetragonal,
            "3" | "-3" => Symmetry::TrigonalLow,
            "32" | "321" | "312" | "3m" | "3m1" | "31m" | "-3m" | "-3m1" | "-31m" => {
                Symmetry::Trigonal
            }
            "6" | "-6" | "6/m" => Symmetry::HexagonalLow,
            "622" | "6mm" | "-6m2" | "-62m" | "6/mmm" => Symmetry::Hexagonal,
            "23" | "m-3" | "m3" => Symmetry::CubicLow,
            "432" | "-43m" | "m-3m" | "m3m" => Symmetry::Cubic,
            _ => return None,
        })
    }

    /// The Laue group's Hermann–Mauguin symbol.
    pub fn laue(self) -> &'static str {
        match self {
            Symmetry::Triclinic => "-1",
            Symmetry::Monoclinic => "2/m",
            Symmetry::Orthorhombic => "mmm",
            Symmetry::TetragonalLow => "4/m",
            Symmetry::Tetragonal => "4/mmm",
            Symmetry::TrigonalLow => "-3",
            Symmetry::Trigonal => "-3m",
            Symmetry::HexagonalLow => "6/m",
            Symmetry::Hexagonal => "6/mmm",
            Symmetry::CubicLow => "m-3",
            Symmetry::Cubic => "m-3m",
        }
    }

    /// Folds a crystal direction into the standard sector (the same sector orix uses): the
    /// symmetrically equivalent direction inside it.
    pub fn reduce(self, v: [f64; 3]) -> [f64; 3] {
        let [x, y, z] = v;
        match self {
            Symmetry::Cubic => {
                let (mut a, mut b, mut c) = (x.abs(), y.abs(), z.abs());
                // Sort ascending: a ≤ b ≤ c; the sector is 0 ≤ y ≤ x ≤ z.
                if a > b {
                    std::mem::swap(&mut a, &mut b);
                }
                if b > c {
                    std::mem::swap(&mut b, &mut c);
                }
                if a > b {
                    std::mem::swap(&mut a, &mut b);
                }
                [b, a, c]
            }
            Symmetry::CubicLow => {
                let (a, b, c) = (x.abs(), y.abs(), z.abs());
                // The 3-fold axis along [111] permutes cyclically; z must be the largest.
                if a >= b && a >= c {
                    [b, c, a]
                } else if b >= a && b >= c {
                    [c, a, b]
                } else {
                    [a, b, c]
                }
            }
            Symmetry::Orthorhombic => [x.abs(), y.abs(), z.abs()],
            Symmetry::Monoclinic => {
                let z = z.abs();
                if y < 0.0 { [-x, -y, z] } else { [x, y, z] }
            }
            Symmetry::Triclinic => {
                if z < 0.0 {
                    [-x, -y, -z]
                } else {
                    v
                }
            }
            Symmetry::Hexagonal => azimuthal(x, y, z.abs(), 60.0, true, 0.0),
            Symmetry::HexagonalLow => azimuthal(x, y, z.abs(), 60.0, false, 0.0),
            Symmetry::Tetragonal => azimuthal(x, y, z.abs(), 90.0, true, 0.0),
            Symmetry::TetragonalLow => azimuthal(x, y, z.abs(), 90.0, false, 0.0),
            Symmetry::Trigonal | Symmetry::TrigonalLow => {
                // Inversion brings the direction to the upper hemisphere.
                let (x, y, z) = if z < 0.0 { (-x, -y, -z) } else { (x, y, z) };
                if self == Symmetry::Trigonal {
                    azimuthal(x, y, z, 120.0, true, -30.0)
                } else {
                    azimuthal(x, y, z, 120.0, false, 0.0)
                }
            }
        }
    }

    /// The IPF key of this Laue group, or `None` for those whose standard sector is not a
    /// triangle (m-3, 2/m, -1): such phases are drawn grey.
    pub fn key(self) -> Option<Key> {
        let r3 = 1.0 / 3f64.sqrt();
        let r2 = 1.0 / 2f64.sqrt();
        let z = [0.0, 0.0, 1.0];
        let az = |deg: f64| {
            let a = deg.to_radians();
            [a.cos(), a.sin(), 0.0]
        };
        let corners = match self {
            Symmetry::Cubic => [z, [r2, 0.0, r2], [r3, r3, r3]],
            Symmetry::Hexagonal => [z, az(0.0), az(30.0)],
            Symmetry::HexagonalLow => [z, az(0.0), az(60.0)],
            Symmetry::Tetragonal => [z, az(0.0), az(45.0)],
            Symmetry::TetragonalLow | Symmetry::Orthorhombic => [z, az(0.0), az(90.0)],
            Symmetry::Trigonal => [z, az(-30.0), az(30.0)],
            Symmetry::TrigonalLow => [z, az(0.0), az(120.0)],
            Symmetry::CubicLow | Symmetry::Monoclinic | Symmetry::Triclinic => return None,
        };
        Some(Key::new(self, corners))
    }
}

/// Folds the azimuth of (x, y) into [start, start + period / 2] (with mirrors) or
/// [start, start + period) (rotations only).
fn azimuthal(x: f64, y: f64, z: f64, period: f64, mirror: bool, start: f64) -> [f64; 3] {
    let r = x.hypot(y);
    if r == 0.0 {
        return [0.0, 0.0, z];
    }
    let mut phi = (y.atan2(x).to_degrees() - start).rem_euclid(period);
    if mirror && phi > period / 2.0 {
        phi = period - phi;
    }
    let a = (phi + start).to_radians();
    [r * a.cos(), r * a.sin(), z]
}

/// A Laue group's colour key: the corners of its standard sector (red, green, blue) and the
/// inward normals of the opposite edges.
#[derive(Clone, Copy, Debug)]
pub struct Key {
    symmetry: Symmetry,
    normals: [[f64; 3]; 3],
    /// Angular distance of each corner from its opposite edge.
    reach: [f64; 3],
}

impl Key {
    fn new(symmetry: Symmetry, c: [[f64; 3]; 3]) -> Key {
        let mut normals = [[0.0; 3]; 3];
        let mut reach = [0.0; 3];
        for i in 0..3 {
            let (j, k) = ((i + 1) % 3, (i + 2) % 3);
            let mut n = normalise(cross(c[j], c[k]));
            if dot(n, c[i]) < 0.0 {
                n = [-n[0], -n[1], -n[2]];
            }
            normals[i] = n;
            reach[i] = dot(n, c[i]).clamp(-1.0, 1.0).asin();
        }
        Key {
            symmetry,
            normals,
            reach,
        }
    }

    pub fn symmetry(&self) -> Symmetry {
        self.symmetry
    }

    /// The colour of a crystal direction already in the standard sector.
    pub fn sector_colour(&self, h: [f64; 3]) -> [f32; 3] {
        let c: [f64; 3] = std::array::from_fn(|i| {
            let d = dot(self.normals[i], h).clamp(-1.0, 1.0).asin() / self.reach[i];
            d.clamp(0.0, 1.0).sqrt()
        });
        let m = c[0].max(c[1]).max(c[2]);
        if m <= 0.0 {
            return [0.0; 3];
        }
        [(c[0] / m) as f32, (c[1] / m) as f32, (c[2] / m) as f32]
    }

    /// The colour of any crystal direction (folded into the sector first).
    pub fn direction_colour(&self, h: [f64; 3]) -> [f32; 3] {
        self.sector_colour(self.symmetry.reduce(normalise(h)))
    }
}

/// Grey for phases without a key.
pub const GREY: [f32; 3] = [0.5, 0.5, 0.5];

/// The orientation matrix of Bunge Euler angles (φ1, Φ, φ2): rows are the crystal axes in
/// the sample frame, so `g · v_sample = v_crystal`.
pub fn matrix(euler: [f32; 3]) -> [[f64; 3]; 3] {
    let (s1, c1) = f64::from(euler[0]).sin_cos();
    let (s, c) = f64::from(euler[1]).sin_cos();
    let (s2, c2) = f64::from(euler[2]).sin_cos();
    [
        [c1 * c2 - s1 * s2 * c, s1 * c2 + c1 * s2 * c, s2 * s],
        [-c1 * s2 - s1 * c2 * c, -s1 * s2 + c1 * c2 * c, c2 * s],
        [s1 * s, -c1 * s, c],
    ]
}

/// The crystal direction parallel to the sample direction `dir`.
pub fn crystal_direction(euler: [f32; 3], dir: [f32; 3]) -> [f64; 3] {
    mul(&matrix(euler), dir.map(f64::from))
}

/// The IPF colour (0..=1 per channel) of sample direction `dir` for a crystal of orientation
/// `euler` (Bunge, radians): grey for Laue groups without a triangular sector, black for
/// non-finite angles.
pub fn colour(euler: [f32; 3], sym: Symmetry, direction: [f32; 3]) -> [f32; 3] {
    if euler.iter().any(|v| !v.is_finite()) {
        return [0.0; 3];
    }
    match sym.key() {
        Some(k) => k.direction_colour(crystal_direction(euler, direction)),
        None => GREY,
    }
}

pub(crate) fn mul(g: &[[f64; 3]; 3], v: [f64; 3]) -> [f64; 3] {
    [dot(g[0], v), dot(g[1], v), dot(g[2], v)]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalise(v: [f64; 3]) -> [f64; 3] {
    let n = dot(v, v).sqrt();
    if n > 0.0 { v.map(|c| c / n) } else { v }
}

/// Angle between two directions, radians.
pub fn angle(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (a, b) = (normalise(a), normalise(b));
    // atan2 of |a×b| and a·b keeps precision at small angles.
    let c = cross(a, b);
    dot(c, c).sqrt().atan2(dot(a, b)).clamp(0.0, PI)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5)
    }

    #[test]
    fn cubic_corners() {
        let k = Symmetry::Cubic.key().unwrap();
        assert!(close(k.direction_colour([0.0, 0.0, 1.0]), [1.0, 0.0, 0.0]));
        assert!(close(k.direction_colour([1.0, 0.0, 1.0]), [0.0, 1.0, 0.0]));
        assert!(close(k.direction_colour([1.0, 1.0, 1.0]), [0.0, 0.0, 1.0]));
        // Equivalent directions anywhere on the sphere.
        assert!(close(k.direction_colour([0.0, -1.0, 0.0]), [1.0, 0.0, 0.0]));
        assert!(close(k.direction_colour([0.0, -1.0, 1.0]), [0.0, 1.0, 0.0]));
        assert!(close(
            k.direction_colour([-1.0, 1.0, -1.0]),
            [0.0, 0.0, 1.0]
        ));
        let mid = k.direction_colour([0.3, 0.13, 0.9]);
        assert!(mid.iter().all(|&c| c > 0.0) && mid.contains(&1.0));
    }

    #[test]
    fn hexagonal_corners() {
        let k = Symmetry::Hexagonal.key().unwrap();
        let s3 = 3f64.sqrt() / 2.0;
        // [0001] red, [2-1-10] (x) green, [10-10] (30° from x) blue.
        assert!(close(k.direction_colour([0.0, 0.0, -1.0]), [1.0, 0.0, 0.0]));
        assert!(close(k.direction_colour([1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]));
        assert!(close(k.direction_colour([-0.5, s3, 0.0]), [0.0, 1.0, 0.0]));
        assert!(close(k.direction_colour([s3, 0.5, 0.0]), [0.0, 0.0, 1.0]));
        assert!(close(k.direction_colour([0.0, 1.0, 0.0]), [0.0, 0.0, 1.0]));
    }

    #[test]
    fn sectors_match_orix() {
        // Folding stays inside the sectors orix uses (checked against its sector normals).
        let checks: [(Symmetry, &[[f64; 3]]); 8] = [
            (
                Symmetry::Cubic,
                &[[1.0, -1.0, 0.0], [-1.0, 0.0, 1.0], [0.0, 1.0, 0.0]],
            ),
            (
                Symmetry::CubicLow,
                &[
                    [1.0, 0.0, 0.0],
                    [0.0, -1.0, 1.0],
                    [-1.0, 0.0, 1.0],
                    [0.0, 1.0, 0.0],
                ],
            ),
            (
                Symmetry::Hexagonal,
                &[[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.5, -0.866_025_4, 0.0]],
            ),
            (
                Symmetry::HexagonalLow,
                &[[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.866_025_4, -0.5, 0.0]],
            ),
            (
                Symmetry::Tetragonal,
                &[[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [1.0, -1.0, 0.0]],
            ),
            (
                Symmetry::Trigonal,
                &[
                    [0.0, 0.0, 1.0],
                    [0.5, 0.866_025_4, 0.0],
                    [0.5, -0.866_025_4, 0.0],
                ],
            ),
            (
                Symmetry::TrigonalLow,
                &[[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.866_025_4, 0.5, 0.0]],
            ),
            (
                Symmetry::Orthorhombic,
                &[[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]],
            ),
        ];
        let mut seed = 12345u64;
        let mut rnd = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((seed >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
        };
        for (sym, normals) in checks {
            for _ in 0..2000 {
                let v = normalise([rnd(), rnd(), rnd()]);
                let h = sym.reduce(v);
                assert!((dot(h, h) - 1.0).abs() < 1e-9);
                for n in normals {
                    assert!(dot(*n, h) > -1e-6, "{sym:?} {v:?} -> {h:?}");
                }
            }
        }
    }

    #[test]
    fn identity_orientation() {
        let z = colour([0.0; 3], Symmetry::Cubic, [0.0, 0.0, 1.0]);
        assert!(close(z, [1.0, 0.0, 0.0]));
        // Φ = 45° about x (φ1 = 0): sample z lies along crystal [0 1 1] → green.
        let g = colour(
            [0.0, std::f32::consts::FRAC_PI_4, 0.0],
            Symmetry::Cubic,
            [0.0, 0.0, 1.0],
        );
        // (f32 angles: the square root magnifies the rounding left in the other channels.)
        assert!(g[1] == 1.0 && g[0] < 1e-3 && g[2] < 1e-3, "{g:?}");
        assert_eq!(
            colour([f32::NAN; 3], Symmetry::Cubic, [0.0, 0.0, 1.0]),
            [0.0; 3]
        );
        assert_eq!(colour([0.0; 3], Symmetry::Triclinic, [0.0, 0.0, 1.0]), GREY);
    }

    #[test]
    fn names() {
        assert_eq!(Symmetry::from_tsl("43"), Some(Symmetry::Cubic));
        assert_eq!(Symmetry::from_tsl("62"), Some(Symmetry::Hexagonal));
        assert_eq!(Symmetry::from_tsl("m3m"), Some(Symmetry::Cubic));
        assert_eq!(Symmetry::from_laue_id(9), Some(Symmetry::Hexagonal));
        assert_eq!(Symmetry::from_name("6/MMM"), Some(Symmetry::Hexagonal));
        assert_eq!(Symmetry::Trigonal.laue(), "-3m");
        assert!((angle([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]) - PI / 2.0).abs() < 1e-12);
    }
}
