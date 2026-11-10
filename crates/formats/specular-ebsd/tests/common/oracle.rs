//! Comparison with the orix oracle's records (`gen/oracle.py`).

use std::path::Path;

use specular_ebsd::ipf::{self, Symmetry};
use specular_em::dataset::Dataset;

const LAUE: [&str; 11] = [
    "-1", "2/m", "mmm", "4/m", "4/mmm", "-3", "-3m", "6/m", "6/mmm", "m-3", "m-3m",
];

/// Mismatch counts for one file.
#[derive(Debug, Default)]
pub struct Tally {
    pub points: usize,
    /// Points orix indexes and we do not, or the other way round.
    pub indexed: usize,
    /// Points that land outside our raster.
    pub placed: usize,
    /// Orientation more than 1e-3 rad from orix's.
    pub orientation: usize,
    /// Phase symmetry differs.
    pub laue: usize,
    /// Folded sample-Z direction more than 1e-3 rad from orix's.
    pub folded: usize,
    /// IPF-Z colour more than 2/255 from the key evaluated on orix's direction.
    pub colour: usize,
    /// Not compared: points sharing their pixel with another (scattered point lists).
    pub shared: usize,
    /// Not compared: orix indexes them, we follow OIM and do not (CI < 0).
    pub ci_negative: usize,
    /// Largest colour difference seen.
    pub colour_max: f32,
    /// Mean |difference| from orix's own (MTEX HSV) key, per channel, and points compared.
    pub orix_key_sum: f64,
    pub orix_key_n: usize,
}

impl Tally {
    pub fn mismatches(&self) -> usize {
        self.indexed + self.placed + self.orientation + self.laue + self.folded + self.colour
    }

    pub fn add(&mut self, o: &Tally) {
        self.points += o.points;
        self.indexed += o.indexed;
        self.placed += o.placed;
        self.orientation += o.orientation;
        self.laue += o.laue;
        self.folded += o.folded;
        self.colour += o.colour;
        self.shared += o.shared;
        self.ci_negative += o.ci_negative;
        self.colour_max = self.colour_max.max(o.colour_max);
        self.orix_key_sum += o.orix_key_sum;
        self.orix_key_n += o.orix_key_n;
    }
}

/// The oracle's `.txt` (None when orix failed on the file: its error text).
pub fn load(expected: &Path) -> Result<Vec<[f32; 15]>, String> {
    let txt = std::fs::read_to_string(expected.with_extension(format!(
        "{}.txt",
        expected.extension().unwrap_or_default().to_string_lossy()
    )))
    .map_err(|e| e.to_string())?;
    if let Some(e) = txt.lines().find(|l| l.starts_with("error")) {
        return Err(e.to_owned());
    }
    let bin = std::fs::read(expected.with_extension(format!(
        "{}.f32",
        expected.extension().unwrap_or_default().to_string_lossy()
    )))
    .map_err(|e| e.to_string())?;
    Ok(bin
        .chunks_exact(60)
        .map(|c| {
            let mut r = [0f32; 15];
            for (k, v) in r.iter_mut().enumerate() {
                *v = f32::from_le_bytes(c[k * 4..k * 4 + 4].try_into().unwrap());
            }
            r
        })
        .collect())
}

fn rot_angle(a: [f32; 3], b: [f32; 3]) -> f64 {
    let (ga, gb) = (ipf::matrix(a), ipf::matrix(b));
    let mut tr = 0.0;
    for i in 0..3 {
        for j in 0..3 {
            tr += ga[i][j] * gb[i][j];
        }
    }
    ((tr - 1.0) / 2.0).clamp(-1.0, 1.0).acos()
}

pub fn compare(d: &Dataset, records: &[[f32; 15]]) -> Tally {
    let mut t = Tally::default();
    let page = &d.pages[0];
    let o = page.orientations.as_ref().expect("orientations");
    let (x, y) = (page.x.clone().unwrap(), page.y.clone().unwrap());
    let hex = page
        .meta
        .iter()
        .any(|(k, v)| k == "Grid" && v == "hexagonal");
    let planes: Vec<Vec<f32>> = (0..3).map(|k| page.read_plane(&[], k).unwrap().0).collect();
    let ci = d
        .pages
        .iter()
        .find(|p| p.title.as_deref() == Some("Maps"))
        .and_then(|p| {
            let k = p.planes.iter().position(|pl| pl.label == "CI")?;
            Some(p.read_plane(&[], k)?.0)
        });
    let pixel = |r: &[f32; 15]| {
        let row = ((f64::from(r[1]) - y.offset) / y.step).round();
        let u = (f64::from(r[0]) - x.offset) / x.step;
        let col = if hex && row as i64 % 2 == 1 {
            (u + 0.25).floor()
        } else {
            u.round()
        };
        (col, row)
    };
    let mut claims = std::collections::HashMap::new();
    for r in records {
        let (c, r) = pixel(r);
        *claims.entry((c as i64, r as i64)).or_insert(0usize) += 1;
    }
    for r in records {
        t.points += 1;
        let (col, row) = pixel(r);
        if claims[&(col as i64, row as i64)] > 1 {
            t.shared += 1;
            continue;
        }
        if row < 0.0 || col < 0.0 || row as usize >= page.height || col as usize >= page.width {
            t.placed += 1;
            continue;
        }
        let (col, row) = (col as usize, row as usize);
        let ours = o.at(col, row);
        let laue = r[2];
        if laue < 0.0 {
            if ours.is_some() {
                t.indexed += 1;
            }
            continue;
        }
        let Some((e, phase)) = ours else {
            if ci
                .as_ref()
                .is_some_and(|ci| ci[row * page.width + col] < 0.0)
            {
                t.ci_negative += 1;
                continue;
            }
            t.indexed += 1;
            continue;
        };
        let theirs = [r[3], r[4], r[5]];
        if rot_angle(e, theirs) > 1e-3 {
            t.orientation += 1;
            continue;
        }
        if laue < 1.0 {
            continue;
        }
        let name = LAUE[laue as usize - 1];
        let sym = phase.and_then(|p| Symmetry::from_name(&p.symmetry));
        if sym.map(Symmetry::laue) != Some(name) {
            t.laue += 1;
            continue;
        }
        let sym = sym.unwrap();
        let h = sym.reduce(ipf::crystal_direction(e, [0.0, 0.0, 1.0]));
        let h_orix = [f64::from(r[6]), f64::from(r[7]), f64::from(r[8])];
        if ipf::angle(h, h_orix) > 1e-3 {
            t.folded += 1;
            continue;
        }
        let i = row * page.width + col;
        let mut worst = 0f32;
        for c in 0..3 {
            worst = worst.max((planes[c][i] - r[9 + c]).abs());
            t.orix_key_sum += f64::from((planes[c][i] - r[12 + c]).abs());
        }
        t.orix_key_n += 3;
        t.colour_max = t.colour_max.max(worst);
        if worst > 2.0 / 255.0 {
            t.colour += 1;
        }
    }
    t
}
