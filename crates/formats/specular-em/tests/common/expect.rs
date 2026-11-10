//! The oracles' expectations (`gen/oracle.py`): `REL.K.txt` and `REL.K.f32` under an
//! `expected/` directory. Shared by the specular-em and specular-spm tests.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// One image's expectation.
#[derive(Debug, Default)]
pub struct Expect {
    /// The file, relative to the root.
    pub rel: String,
    /// The image or channel ordinal.
    pub k: usize,
    pub size: (usize, usize, usize),
    pub planes: Vec<usize>,
    /// Per dimension, x first: (scale, offset, unit).
    pub axes: Vec<(f64, f64, String)>,
    pub voxel: Option<[f64; 3]>,
    /// A map's first grid point (Å), x, y, z.
    pub origin: Option<[f64; 3]>,
    pub unit: Option<String>,
    pub title: Option<String>,
    pub labels: Vec<(usize, String)>,
    pub complex: bool,
    pub error: Option<String>,
    /// The stored planes, concatenated.
    pub values: Vec<f32>,
}

/// Every expectation under `root/expected`, sorted by file and ordinal.
pub fn all(root: &Path) -> Vec<Expect> {
    let dir = root.join("expected");
    let mut files = Vec::new();
    walk(&dir, &mut files);
    let mut out: Vec<Expect> = files
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "txt"))
        .map(|p| read(&dir, p))
        .collect();
    out.sort_by(|a, b| (&a.rel, a.k).cmp(&(&b.rel, b.k)));
    out
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn read(dir: &Path, txt: &Path) -> Expect {
    let rel_k = txt
        .strip_prefix(dir)
        .unwrap()
        .to_string_lossy()
        .trim_end_matches(".txt")
        .to_string();
    let (rel, k) = rel_k.rsplit_once('.').unwrap();
    let mut e = Expect {
        rel: rel.to_string(),
        k: k.parse().unwrap(),
        ..Expect::default()
    };
    for line in std::fs::read_to_string(txt).unwrap().lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let num = |i: usize| f[i].parse::<f64>().unwrap();
        match f[0] {
            "size" => {
                e.size = (
                    f[1].parse().unwrap(),
                    f[2].parse().unwrap(),
                    f[3].parse().unwrap(),
                );
            }
            "planes" => e.planes = f[1..].iter().map(|s| s.parse().unwrap()).collect(),
            "axis" => e
                .axes
                .push((num(2), num(3), f.get(4).copied().unwrap_or("").to_string())),
            "voxel" => e.voxel = Some([num(1), num(2), num(3)]),
            "origin" => e.origin = Some([num(1), num(2), num(3)]),
            "unit" => e.unit = Some(f.get(1).copied().unwrap_or("").to_string()),
            "title" => e.title = Some(f[1..].join("\t")),
            "label" => e.labels.push((f[1].parse().unwrap(), f[2..].join("\t"))),
            "complex" => e.complex = true,
            "error" => e.error = Some(f[1..].join("\t")),
            other => panic!("{}: unknown line {other:?}", txt.display()),
        }
    }
    if e.error.is_none() {
        let b = std::fs::read(txt.with_extension("f32")).unwrap();
        e.values = b
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let (w, h, _) = e.size;
        assert_eq!(e.values.len(), e.planes.len() * w * h, "{rel_k}");
    }
    e
}

impl Expect {
    /// Expected plane number `i` of [`Expect::planes`].
    pub fn plane(&self, i: usize) -> &[f32] {
        let n = self.size.0 * self.size.1;
        &self.values[i * n..(i + 1) * n]
    }

    pub fn name(&self) -> String {
        format!("{} #{}", self.rel, self.k)
    }
}

/// Compares a plane: bitwise (NaN equal to NaN), or within 1e-6 relative for magnitudes of
/// complex values. Returns the number of mismatches and a description of the first.
pub fn compare(got: &[f32], want: &[f32], complex: bool) -> (usize, Option<String>) {
    let mut bad = 0;
    let mut first = None;
    for (i, (&g, &w)) in got.iter().zip(want).enumerate() {
        let ok = if g.is_nan() || w.is_nan() {
            g.is_nan() && w.is_nan()
        } else if complex {
            (g - w).abs() <= 1e-6 * g.abs().max(w.abs()) || g == w
        } else {
            g.to_bits() == w.to_bits() || (g == 0.0 && w == 0.0)
        };
        if !ok {
            bad += 1;
            first.get_or_insert_with(|| format!("at {i}: {g} vs {w}"));
        }
    }
    (bad, first)
}

/// Whether two calibration numbers agree (the oracles compute in f64 from the same values).
pub fn close(a: f64, b: f64) -> bool {
    a == b || (a - b).abs() <= 1e-9 * a.abs().max(b.abs()) || (a.abs() < 1e-300 && b.abs() < 1e-300)
}
