//! Oxford HKL .ctf ("Channel Text File"): tab-separated header lines (project, cell counts and
//! steps, acquisition settings, phases with their Laue group), a column header line starting
//! with "Phase", then one line per point: Phase X Y Bands Error Euler1 Euler2 Euler3 (degrees)
//! MAD BC BS and possibly more. Phase 0 marks a point not indexed.

use specular_em::dataset::{Phase, Plane};

use crate::ipf::Symmetry;
use crate::{Error, Grid, Scan, num, orientations, place, text};

pub(crate) fn parse(file: &[u8]) -> Result<Scan, Error> {
    let mut scan = Scan::default();
    let mut hint = Grid {
        width: 0,
        height: 0,
        x0: 0.0,
        y0: 0.0,
        xstep: 0.0,
        ystep: 0.0,
        hex: false,
    };
    let mut meta: Vec<(String, String)> = Vec::new();
    let mut phases: Vec<(Phase, Option<Symmetry>)> = Vec::new();
    let mut pending_phases = 0usize;
    let mut columns: Vec<String> = Vec::new();
    let mut pos = 0;
    while pos < file.len() {
        let end = file[pos..]
            .iter()
            .position(|&c| c == b'\n')
            .map_or(file.len(), |p| pos + p + 1);
        let raw = String::from_utf8_lossy(&file[pos..end]);
        pos = end;
        let line = raw.trim_end_matches(['\r', '\n']);
        let cells: Vec<&str> = line.split('\t').collect();
        if pending_phases > 0 {
            pending_phases -= 1;
            phases.push(phase(&cells));
            continue;
        }
        let key = cells[0].trim();
        let value = cells.get(1).map_or("", |v| v.trim());
        if key == "Phase" && cells.len() > 2 {
            columns = cells.iter().map(|c| c.trim().to_owned()).collect();
            break;
        }
        match key {
            "Prj" => scan.title = text(value),
            "Author" => {
                if value != "[Unknown]" && !value.is_empty() {
                    meta.push(("Author".into(), value.into()));
                }
            }
            "JobMode" => {
                meta.push(("Job mode".into(), value.into()));
                hint.hex = value.eq_ignore_ascii_case("Hex");
            }
            "XCells" => hint.width = value.parse().unwrap_or(0),
            "YCells" => hint.height = value.parse().unwrap_or(0),
            "XStep" => hint.xstep = num(value).unwrap_or(0.0),
            "YStep" => hint.ystep = num(value).unwrap_or(0.0),
            "Phases" => pending_phases = value.parse().unwrap_or(0),
            _ if key.starts_with("Euler angles refer to") => {
                // Name/value pairs: Mag, Coverage, Device, KV, TiltAngle, TiltAxis…
                for pair in cells[1..].chunks(2) {
                    let (Some(k), Some(v)) = (pair.first(), pair.get(1)) else {
                        continue;
                    };
                    let (k, v) = (k.trim(), v.trim());
                    match k {
                        "Mag" => scan.magnification = num(v).filter(|m| *m > 0.0),
                        "KV" => scan.voltage = num(v).filter(|m| *m > 0.0).map(|kv| kv * 1e3),
                        "TiltAngle" => meta.push(("Tilt angle".into(), format!("{v}°"))),
                        "TiltAxis" => meta.push(("Tilt axis".into(), format!("{v}°"))),
                        "Coverage" => meta.push(("Coverage".into(), format!("{v} %"))),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    if columns.is_empty() {
        return Err(Error::Malformed("no column header line"));
    }
    let find = |name: &str| columns.iter().position(|c| c.eq_ignore_ascii_case(name));
    let (Some(cx), Some(cy), Some(e1), Some(e2), Some(e3)) = (
        find("X"),
        find("Y"),
        find("Euler1"),
        find("Euler2"),
        find("Euler3"),
    ) else {
        return Err(Error::Malformed("missing X, Y or Euler columns"));
    };
    let cphase = find("Phase").unwrap_or(0);
    let table = text::table(&file[pos..], columns.len());
    if table.rows == 0 {
        return Err(Error::NoData);
    }
    let t = &table;
    let col = |k: usize| (0..t.rows).map(move |r| t.get(r, k));
    let xs: Vec<f32> = col(cx).collect();
    let ys: Vec<f32> = col(cy).collect();
    let grid = Grid::new(&xs, &ys, hint)?;
    let _res = grid.reserve()?;
    let at: Vec<Option<usize>> = xs
        .iter()
        .zip(&ys)
        .map(|(&x, &y)| grid.index(x, y))
        .collect();
    let rad = std::f32::consts::PI / 180.0;
    let points = (0..table.rows).map(|r| {
        let id = table.get(r, cphase);
        let e = [table.get(r, e1), table.get(r, e2), table.get(r, e3)].map(|v| v * rad);
        // NaN ids are not indexed either.
        if id.is_nan() || id < 1.0 || id > phases.len() as f32 {
            ([f32::NAN; 3], u8::MAX)
        } else {
            (e, (id as usize - 1).min(254) as u8)
        }
    });
    let (euler, phase) = orientations(&grid, &at, points);
    let maps = [
        ("BC", "Band contrast", ""),
        ("BS", "Band slope", ""),
        ("MAD", "MAD", "°"),
        ("Bands", "Bands", ""),
        ("Error", "Error", ""),
        ("Phase", "Phase", ""),
    ];
    for (name, label, unit) in maps {
        if let Some(k) = find(name) {
            scan.maps
                .push(Plane::values(label, unit, place(&grid, &at, col(k))));
        }
    }
    let known = [
        "Phase", "X", "Y", "Bands", "Error", "Euler1", "Euler2", "Euler3", "MAD", "BC", "BS",
    ];
    for (k, c) in columns.iter().enumerate() {
        if !known.iter().any(|n| n.eq_ignore_ascii_case(c)) && !c.is_empty() {
            scan.maps
                .push(Plane::values(c.clone(), "", place(&grid, &at, col(k))));
        }
    }
    let (x, y) = grid.lateral();
    scan.width = grid.width;
    scan.height = grid.height;
    scan.x = x;
    scan.y = y;
    scan.euler = euler;
    scan.phase = phase;
    scan.phases = phases;
    meta.insert(0, ("Points".into(), table.rows.to_string()));
    scan.meta = meta;
    Ok(scan)
}

/// A phase line: "a;b;c  α;β;γ  name  Laue group  space group  …".
fn phase(cells: &[&str]) -> (Phase, Option<Symmetry>) {
    let name = cells.get(2).map_or("", |s| s.trim()).to_owned();
    let laue = cells.get(3).and_then(|s| s.trim().parse::<u32>().ok());
    let sym = laue.and_then(Symmetry::from_laue_id);
    let symmetry = match (sym, laue) {
        (Some(s), _) => s.laue().to_owned(),
        (None, Some(n)) => format!("Laue group {n}"),
        (None, None) => String::new(),
    };
    (Phase { name, symmetry }, sym)
}
