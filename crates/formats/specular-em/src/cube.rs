//! Gaussian cube files (`.cube`): volumetric data on a grid (electron densities, molecular
//! orbitals, electrostatic potentials) with the molecule's atoms.
//!
//! Two comment lines; the atom count and the grid origin (a negative count means an orbital
//! list follows the atoms, and a fifth number may give the values per point); three lines of
//! a point count and a step vector (a positive count means Bohr, a negative one Ångström);
//! one line per atom (atomic number, charge, position); the orbital list when announced;
//! then the values, z fastest, then y, then x, with every orbital's value at a point
//! together.

use crate::{Error, MAX_PLANE, Result};

/// Ångström per Bohr (CODATA 2018).
pub const BOHR: f64 = 0.529_177_210_903;

/// Grid points kept at most.
const MAX_POINTS: u64 = 1 << 30;

/// One atom: atomic number, charge and position (Ångström).
#[derive(Clone, Debug, PartialEq)]
pub struct Atom {
    pub number: u32,
    pub charge: f64,
    pub position: [f64; 3],
}

/// A parsed cube file.
#[derive(Clone, Debug, PartialEq)]
pub struct Cube {
    /// The two comment lines.
    pub comments: [String; 2],
    /// Grid origin, Ångström.
    pub origin: [f64; 3],
    /// Points along the three grid axes.
    pub size: [usize; 3],
    /// Step vector of each axis, Ångström.
    pub axes: [[f64; 3]; 3],
    /// The file gave lengths in Bohr (converted).
    pub bohr: bool,
    pub atoms: Vec<Atom>,
    /// The orbital numbers, when the file holds orbitals.
    pub orbitals: Vec<i64>,
    /// Values per grid point in the file (orbitals, or the fifth header number).
    pub per_point: usize,
    /// Every value of every point (`per_point` of them together), `x` slowest and `z` fastest
    /// as stored.
    pub values: Vec<f32>,
    /// The values ended early (missing points read as NaN).
    pub truncated: bool,
}

/// Fields of a line, split on white space.
fn fields(line: &str) -> Vec<&str> {
    line.split_whitespace().collect()
}

/// A header line of an integer then three numbers (and maybe a fifth).
fn int_vec(line: &str) -> Option<(i64, [f64; 3], Option<f64>)> {
    let f = fields(line);
    if !(4..=5).contains(&f.len()) {
        return None;
    }
    let n = f[0].parse::<i64>().ok()?;
    let mut v = [0.0; 3];
    for k in 0..3 {
        v[k] = f[k + 1].parse::<f64>().ok().filter(|x| x.is_finite())?;
    }
    let extra = match f.get(4) {
        Some(s) => Some(s.parse::<f64>().ok()?),
        None => None,
    };
    Some((n, v, extra))
}

/// Whether `head` (the start of a file) has a cube file's header lines: the counts line,
/// three axis lines with nonzero counts and the first atom line.
pub fn sniff(head: &[u8]) -> bool {
    let text = match std::str::from_utf8(head) {
        Ok(t) => t,
        Err(e) => match std::str::from_utf8(&head[..e.valid_up_to()]) {
            Ok(t) => t,
            Err(_) => return false,
        },
    };
    let mut lines = text.lines().skip(2);
    let Some((natoms, _, _)) = lines.next().and_then(int_vec) else {
        return false;
    };
    if natoms == 0 || natoms.unsigned_abs() > 100_000 {
        return false;
    }
    for _ in 0..3 {
        match lines.next().and_then(int_vec) {
            Some((n, v, None)) if n != 0 && v.iter().any(|x| *x != 0.0) => {}
            _ => return false,
        }
    }
    // The first atom: atomic number (0 for ghost atoms), charge and position.
    lines
        .next()
        .and_then(int_vec)
        .is_some_and(|(z, _, extra)| (0..=118).contains(&z) && extra.is_some())
}

/// Parses a cube file.
pub fn parse(data: &[u8]) -> Result<Cube> {
    let text = std::str::from_utf8(data)
        .map_err(|e| Error::Malformed(format!("not text at byte {}", e.valid_up_to())))?;
    let mut lines = text.lines();
    let c1 = lines
        .next()
        .ok_or(Error::Truncated("comments"))?
        .trim()
        .to_owned();
    let c2 = lines
        .next()
        .ok_or(Error::Truncated("comments"))?
        .trim()
        .to_owned();
    let (natoms, origin, nval) = lines
        .next()
        .and_then(int_vec)
        .ok_or_else(|| Error::Malformed("atom count line".into()))?;
    let mut size = [0usize; 3];
    let mut axes = [[0.0; 3]; 3];
    let mut bohr = true;
    for k in 0..3 {
        let (n, v, _) = lines
            .next()
            .and_then(int_vec)
            .ok_or_else(|| Error::Malformed("grid axis line".into()))?;
        if n == 0 || n.unsigned_abs() > 1 << 20 {
            return Err(Error::Malformed(format!("{n} grid points")));
        }
        // The first axis decides the unit (they agree in valid files).
        if k == 0 {
            bohr = n > 0;
        }
        size[k] = n.unsigned_abs() as usize;
        axes[k] = v;
    }
    let scale = if bohr { BOHR } else { 1.0 };
    for a in &mut axes {
        for x in a.iter_mut() {
            *x *= scale;
        }
    }
    let origin = origin.map(|x| x * scale);
    let points = size.iter().map(|&n| n as u64).product::<u64>();
    if points > MAX_POINTS || (size[1] * size[0]) as u64 > MAX_PLANE {
        return Err(Error::TooLarge);
    }
    // Every value takes at least two bytes of text (a digit and a separator): a grid with
    // more points than the file has bytes cannot be there, so no NaN padding of gigabytes.
    if points > data.len() as u64 {
        return Err(Error::Truncated("grid values"));
    }
    if natoms.unsigned_abs() > 1_000_000 {
        return Err(Error::Malformed(format!("{natoms} atoms")));
    }
    let mut atoms = Vec::with_capacity(natoms.unsigned_abs() as usize);
    for _ in 0..natoms.unsigned_abs() {
        let f = lines.next().map(fields).ok_or(Error::Truncated("atoms"))?;
        let num = |i: usize| -> Result<f64> {
            f.get(i)
                .and_then(|s| s.parse::<f64>().ok())
                .ok_or_else(|| Error::Malformed("atom line".into()))
        };
        let z = num(0)?;
        if !(0.0..=118.0).contains(&z) {
            return Err(Error::Malformed(format!("atomic number {z}")));
        }
        atoms.push(Atom {
            number: z as u32,
            charge: num(1)?,
            position: [num(2)? * scale, num(3)? * scale, num(4)? * scale],
        });
    }
    // The orbital list: a count, then that many numbers (possibly over several lines).
    let mut orbitals = Vec::new();
    let mut rest: Vec<&str> = Vec::new();
    if natoms < 0 {
        let mut want = None;
        for line in lines.by_ref() {
            for t in line.split_whitespace() {
                match want {
                    None => {
                        let n = t
                            .parse::<i64>()
                            .ok()
                            .filter(|n| (1..=100_000).contains(n))
                            .ok_or_else(|| Error::Malformed("orbital count".into()))?;
                        want = Some(n as usize);
                    }
                    Some(n) if orbitals.len() < n => orbitals.push(
                        t.parse::<i64>()
                            .map_err(|_| Error::Malformed("orbital number".into()))?,
                    ),
                    Some(_) => rest.push(t),
                }
            }
            if want.is_some_and(|n| orbitals.len() >= n) {
                break;
            }
        }
    }
    let per_point = if !orbitals.is_empty() {
        orbitals.len()
    } else {
        nval.filter(|v| *v >= 1.0 && *v <= 100_000.0)
            .map_or(1, |v| v as usize)
    };
    // Every value of every point; as for the points, each takes two bytes of text at least.
    let total = points.saturating_mul(per_point as u64);
    if total > MAX_POINTS || total > data.len() as u64 {
        return Err(if total > MAX_POINTS {
            Error::TooLarge
        } else {
            Error::Truncated("grid values")
        });
    }
    let mut values = Vec::with_capacity(total as usize);
    let take = |t: &str, values: &mut Vec<f32>| -> Result<bool> {
        let v = fast_float(t).ok_or_else(|| Error::Malformed(format!("value {t:?}")))?;
        values.push(v as f32);
        Ok(values.len() as u64 >= total)
    };
    let mut done = false;
    for t in rest {
        if take(t, &mut values)? {
            done = true;
            break;
        }
    }
    if !done {
        'outer: for line in lines {
            for t in line.split_whitespace() {
                if take(t, &mut values)? {
                    break 'outer;
                }
            }
        }
    }
    let truncated = (values.len() as u64) < total;
    values.resize(total as usize, f32::NAN);
    Ok(Cube {
        comments: [c1, c2],
        origin,
        size,
        axes,
        bohr,
        atoms,
        orbitals,
        per_point,
        values,
        truncated,
    })
}

/// A decimal number (`1.23456E-05`, `-0.5`, Fortran's `1.0D-03`).
fn fast_float(t: &str) -> Option<f64> {
    match t.parse::<f64>() {
        Ok(v) => Some(v),
        Err(_) => t.replace(['D', 'd'], "E").parse().ok(),
    }
}

/// The element symbol of an atomic number ("X" for 0, a ghost atom).
pub fn symbol(z: u32) -> &'static str {
    const S: [&str; 119] = [
        "X", "H", "He", "Li", "Be", "B", "C", "N", "O", "F", "Ne", "Na", "Mg", "Al", "Si", "P",
        "S", "Cl", "Ar", "K", "Ca", "Sc", "Ti", "V", "Cr", "Mn", "Fe", "Co", "Ni", "Cu", "Zn",
        "Ga", "Ge", "As", "Se", "Br", "Kr", "Rb", "Sr", "Y", "Zr", "Nb", "Mo", "Tc", "Ru", "Rh",
        "Pd", "Ag", "Cd", "In", "Sn", "Sb", "Te", "I", "Xe", "Cs", "Ba", "La", "Ce", "Pr", "Nd",
        "Pm", "Sm", "Eu", "Gd", "Tb", "Dy", "Ho", "Er", "Tm", "Yb", "Lu", "Hf", "Ta", "W", "Re",
        "Os", "Ir", "Pt", "Au", "Hg", "Tl", "Pb", "Bi", "Po", "At", "Rn", "Fr", "Ra", "Ac", "Th",
        "Pa", "U", "Np", "Pu", "Am", "Cm", "Bk", "Cf", "Es", "Fm", "Md", "No", "Lr", "Rf", "Db",
        "Sg", "Bh", "Hs", "Mt", "Ds", "Rg", "Cn", "Nh", "Fl", "Mc", "Lv", "Ts", "Og",
    ];
    S.get(z as usize).copied().unwrap_or("?")
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORBITAL: &str = "\
 water orbital
 MO coefficients
   -3   -1.000000   -1.000000   -1.000000
    2    1.000000    0.000000    0.000000
    3    0.000000    1.000000    0.000000
    2    0.000000    0.000000    1.000000
    8    8.000000    0.000000    0.000000    0.000000
    1    1.000000    1.430000    1.100000    0.000000
    1    1.000000   -1.430000    1.100000    0.000000
    2    5    6
 1.0 10.0 2.0 20.0
 3.0 30.0 4.0 40.0 5.0 50.0 6.0 60.0
 7.0 70.0 8.0 80.0 9.0 90.0 10.0 100.0 11.0 110.0 12.0 120.0
";

    #[test]
    fn orbitals_bohr() {
        assert!(sniff(ORBITAL.as_bytes()));
        let c = parse(ORBITAL.as_bytes()).unwrap();
        assert_eq!(c.size, [2, 3, 2]);
        assert!(c.bohr);
        assert_eq!(c.orbitals, [5, 6]);
        assert_eq!(c.atoms.len(), 3);
        assert_eq!(symbol(c.atoms[0].number), "O");
        assert!((c.axes[0][0] - BOHR).abs() < 1e-12);
        assert!((c.origin[0] + BOHR).abs() < 1e-12);
        assert_eq!(c.per_point, 2);
        let first: Vec<f32> = c.values.iter().step_by(2).copied().collect();
        assert_eq!(first, (1..=12).map(|v| v as f32).collect::<Vec<_>>());
        assert_eq!(c.values[1], 10.0);
        assert_eq!(c.values[23], 120.0);
        assert!(!c.truncated);
    }

    #[test]
    fn angstrom_and_truncation() {
        let t = "a\nb\n 1 0 0 0\n -2 0.5 0 0\n -2 0 0.5 0\n -1 0 0 0.5\n 6 0 0 0 0\n 1e-3 2D-1 3\n";
        let c = parse(t.as_bytes()).unwrap();
        assert!(!c.bohr);
        assert_eq!(c.axes[0][0], 0.5);
        assert_eq!(&c.values[..3], &[1e-3, 0.2, 3.0]);
        assert!(c.values[3].is_nan());
        assert!(c.truncated);
    }

    #[test]
    fn luts_are_not_cubes() {
        let lut = "TITLE \"x\"\nLUT_3D_SIZE 33\n0.0 0.0 0.0\n1.0 0.0 0.0\n";
        assert!(!sniff(lut.as_bytes()));
        assert!(!sniff(b"# comment\n# two\n3 0 0 0\n"));
    }

    #[test]
    fn hostile_grid_larger_than_the_file() {
        // 1024³ points declared in a file of a hundred bytes: refused, not padded to 4 GiB.
        let t = "a\nb\n 1 0 0 0\n 1024 1 0 0\n 1024 0 1 0\n 1024 0 0 1\n 6 0 0 0 0\n 1 2 3\n";
        assert_eq!(
            parse(t.as_bytes()).err(),
            Some(Error::Truncated("grid values"))
        );
    }
}
