//! EDAX/TSL .ang: `#` header lines (phases, grid, steps), then one line per point:
//! φ1 Φ φ2 (radians) x y IQ CI phase [detector signal, fit, more], unless `COLUMN_HEADERS`
//! names the columns otherwise.

use specular_em::dataset::{Phase, Plane};

use crate::ipf::Symmetry;
use crate::{Error, Grid, Scan, num, orientations, place, text};

/// Euler angles at or above this mark a point the indexer gave up on (EDAX writes 4π).
const UNINDEXED_EULER: f32 = 12.5;

#[derive(Default)]
struct HeaderPhase {
    id: Option<i64>,
    name: String,
    formula: String,
    symmetry: String,
}

pub(crate) fn parse(file: &[u8]) -> Result<Scan, Error> {
    let mut phases: Vec<HeaderPhase> = Vec::new();
    let mut hint = Grid {
        width: 0,
        height: 0,
        x0: 0.0,
        y0: 0.0,
        xstep: 0.0,
        ystep: 0.0,
        hex: false,
    };
    let mut meta = Vec::new();
    let mut headers: Vec<String> = Vec::new();
    let mut count = 0usize;
    let mut scan = Scan::default();
    let mut pos = 0;
    while pos < file.len() {
        let end = file[pos..]
            .iter()
            .position(|&c| c == b'\n')
            .map_or(file.len(), |p| pos + p + 1);
        let line = file[pos..end].trim_ascii();
        if !line.is_empty() && line[0] != b'#' {
            break;
        }
        pos = end;
        let line = String::from_utf8_lossy(line.get(1..).unwrap_or_default());
        let line = line.trim();
        let (key, value) = match line.find([' ', '\t']) {
            Some(i) => (&line[..i], line[i..].trim()),
            None => (line, ""),
        };
        let key = key.trim_end_matches(':');
        match key {
            "Phase" => phases.push(HeaderPhase {
                id: value.parse().ok(),
                ..HeaderPhase::default()
            }),
            "MaterialName" | "Formula" | "Symmetry" => {
                if phases.is_empty() {
                    phases.push(HeaderPhase::default());
                }
                let p = phases.last_mut().expect("pushed");
                let v = value.to_owned();
                match key {
                    "MaterialName" => p.name = v,
                    "Formula" => p.formula = v,
                    _ => p.symmetry = v,
                }
            }
            "GRID" => hint.hex = value.eq_ignore_ascii_case("HexGrid"),
            "XSTEP" => hint.xstep = num(value).unwrap_or(0.0),
            "YSTEP" => hint.ystep = num(value).unwrap_or(0.0),
            "NCOLS_ODD" => hint.width = value.parse().unwrap_or(0),
            "NROWS" => hint.height = value.parse().unwrap_or(0),
            "COLUMN_COUNT" => count = value.parse().unwrap_or(0),
            "COLUMN_HEADERS" => {
                headers = value.split(',').map(|s| s.trim().to_owned()).collect();
            }
            "OPERATOR" => push(&mut meta, "Operator", value),
            "SAMPLEID" => push(&mut meta, "Sample", value),
            "SCANID" => {
                scan.title = text(value);
            }
            "TEM_PIXperUM" | "x-star" | "y-star" | "z-star" | "WorkingDistance" | "NCOLS_EVEN" => {
                push(&mut meta, key, value)
            }
            _ => {}
        }
    }
    let body = &file[pos..];
    if count == 0 {
        let first = body
            .split(|&c| c == b'\n')
            .find(|l| !l.trim_ascii().is_empty())
            .ok_or(Error::NoData)?;
        count = text::fields(first).count();
    }
    if count < 5 {
        return Err(Error::Malformed("fewer than five columns"));
    }
    // Every row is padded to the column count, so a header's count must stay sane.
    if count > 4096 {
        return Err(Error::Malformed("more than 4096 columns"));
    }
    let table = text::table(body, count);
    if table.rows == 0 {
        return Err(Error::NoData);
    }
    let t = &table;
    let col = |k: usize| (0..t.rows).map(move |r| t.get(r, k));
    let xs: Vec<f32> = col(3).collect();
    let ys: Vec<f32> = col(4).collect();
    if hint.hex && hint.ystep <= 0.0 {
        hint.ystep = hint.xstep * 3f64.sqrt() / 2.0;
    }
    meta.insert(
        0,
        (
            "Grid".into(),
            if hint.hex { "hexagonal" } else { "square" }.into(),
        ),
    );
    let grid = Grid::new(&xs, &ys, hint)?;
    let _res = grid.reserve()?;
    let at: Vec<Option<usize>> = xs
        .iter()
        .zip(&ys)
        .map(|(&x, &y)| grid.index(x, y))
        .collect();

    let roles = Roles::new(&headers, count);
    let lookup: Vec<(i64, u8)> = phases
        .iter()
        .enumerate()
        .map(|(i, p)| (p.id.unwrap_or(i as i64 + 1), i.min(254) as u8))
        .collect();
    let phase_of = |v: f32| -> u8 {
        let id = v as i64;
        if let Some(&(_, k)) = lookup.iter().find(|(pid, _)| *pid == id) {
            k
        } else if id == 0 && !lookup.is_empty() {
            // Single-phase files from older OIM versions number the phase 0.
            0
        } else {
            u8::MAX
        }
    };
    let points = (0..table.rows).map(|r| {
        let e = [table.get(r, 0), table.get(r, 1), table.get(r, 2)];
        let ci = roles.ci.map_or(0.0, |k| table.get(r, k));
        let ph = roles.phase.map_or(1.0, |k| table.get(r, k));
        if ci < 0.0 || e.iter().any(|v| !v.is_finite() || *v >= UNINDEXED_EULER) {
            ([f32::NAN; 3], phase_of(ph))
        } else {
            (e, phase_of(ph))
        }
    });
    let (euler, phase) = orientations(&grid, &at, points);
    for (k, (label, unit)) in roles.maps {
        scan.maps
            .push(Plane::values(label, unit, place(&grid, &at, col(k))));
    }
    scan.phases = phases
        .into_iter()
        .map(|p| {
            let sym = Symmetry::from_tsl(&p.symmetry);
            let name = if p.name.is_empty() { p.formula } else { p.name };
            let symmetry = sym.map_or(p.symmetry, |s| s.laue().to_owned());
            (Phase { name, symmetry }, sym)
        })
        .collect();
    let (x, y) = grid.lateral();
    scan.width = grid.width;
    scan.height = grid.height;
    scan.x = x;
    scan.y = y;
    scan.euler = euler;
    scan.phase = phase;
    meta.insert(1, ("Points".into(), table.rows.to_string()));
    scan.meta = meta;
    Ok(scan)
}

fn push(meta: &mut Vec<(String, String)>, key: &str, value: &str) {
    if !value.is_empty() {
        meta.push((key.to_owned(), value.to_owned()));
    }
}

/// Which columns hold what.
struct Roles {
    ci: Option<usize>,
    phase: Option<usize>,
    /// Columns shown as maps: index, label, unit.
    maps: Vec<(usize, (String, &'static str))>,
}

impl Roles {
    fn new(headers: &[String], count: usize) -> Roles {
        let mut r = Roles {
            ci: None,
            phase: None,
            maps: Vec::new(),
        };
        let named = |label: &str| -> (String, &'static str) {
            let unit = if label == "Fit" { "°" } else { "" };
            (label.to_owned(), unit)
        };
        if headers.len() >= count.min(8) && headers.len() >= 5 {
            for (k, h) in headers
                .iter()
                .enumerate()
                .skip(5)
                .take(count.saturating_sub(5))
            {
                let l = h.to_ascii_lowercase();
                let label = if l == "iq" || l.contains("image quality") {
                    "IQ".to_owned()
                } else if l == "ci" || l.contains("confidence") {
                    r.ci = Some(k);
                    "CI".to_owned()
                } else if l.starts_with("phase") {
                    r.phase = Some(k);
                    "Phase".to_owned()
                } else if l == "fit" || l.contains("fit") {
                    "Fit".to_owned()
                } else if l.starts_with("sem") || l.contains("detector") {
                    "Detector signal".to_owned()
                } else {
                    h.clone()
                };
                r.maps.push((k, named(&label)));
            }
            return r;
        }
        let defaults = ["IQ", "CI", "Phase", "Detector signal", "Fit"];
        for k in 5..count {
            let label = match defaults.get(k - 5) {
                Some(l) => (*l).to_owned(),
                None => format!("Column {}", k + 1),
            };
            match k {
                6 => r.ci = Some(k),
                7 => r.phase = Some(k),
                _ => {}
            }
            r.maps.push((k, named(&label)));
        }
        r
    }
}
