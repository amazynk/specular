//! Slope, aspect and contours against GDAL (`gdaldem slope/aspect -alg Horn -compute_edges`
//! and `gdal_contour`, lines running on to the data's edges as GDAL draws them), from the files `gen/terrain_oracle.py` writes. The outer ring of
//! cells is left out of slope and aspect: GDAL extrapolates past the edge (2a − b) where we
//! repeat the edge's value. Mismatch counts are printed (`cargo test -- --nocapture`).

use specular_geodesy::terrain::{Dem, Levels, contours};

fn data(name: &str) -> String {
    format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn dem(name: &str) -> Vec<f32> {
    std::fs::read(data(&format!("terrain-{name}.f32")))
        .unwrap()
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

fn adiff(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

fn slope_aspect(name: &str, with_aspect: bool) {
    let text = std::fs::read_to_string(data(&format!("terrain-{name}.tsv"))).unwrap();
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let head: Vec<f64> = lines
        .next()
        .unwrap()
        .split('\t')
        .map(|s| s.parse().unwrap())
        .collect();
    let (w, h) = (head[0] as usize, head[1] as usize);
    let values = dem(name);
    let d = Dem {
        values: &values,
        w,
        h,
        step: [head[2], head[3]],
        z: 1.0,
    };
    let (mut n, mut bad_s, mut bad_a, mut max_s, mut max_a) = (0, 0, 0, 0.0f64, 0.0f64);
    let mut rounded = 0;
    for l in lines {
        let v: Vec<&str> = l.split('\t').collect();
        let (x, y): (usize, usize) = (v[0].parse().unwrap(), v[1].parse().unwrap());
        if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
            continue;
        }
        n += 1;
        let gs: f64 = v[2].parse().unwrap();
        let ga: f64 = v[3].parse().unwrap();
        let s = d.slope(x, y);
        match s {
            Some(s) if gs.is_finite() => {
                let e = (s - gs).abs();
                max_s = max_s.max(e);
                if e > 1e-3 {
                    bad_s += 1;
                }
            }
            None if gs.is_nan() => {}
            _ => bad_s += 1,
        }
        if with_aspect {
            match d.aspect(x, y) {
                Some(a) if ga.is_finite() => {
                    let e = adiff(a, ga);
                    max_a = max_a.max(e);
                    // GDAL sums the 3×3 window in single precision: its aspect is good to
                    // about 2e-3 height units over the gradient's run.
                    let run = 8.0 * head[2].abs() * gs.to_radians().tan();
                    let tol = 1e-3 + (2e-3 / run.max(1e-9)).to_degrees();
                    if e > 1e-3 {
                        rounded += 1;
                    }
                    if e > tol {
                        bad_a += 1;
                    }
                }
                // GDAL gives no data for flat ground (without -zero_for_flat).
                None if ga.is_nan() => {}
                _ => bad_a += 1,
            }
        }
    }
    println!(
        "{name}: {n} cells, slope {bad_s} mismatches (max {max_s:.2e}°), aspect {bad_a} (max {max_a:.2e}°; {rounded} past 1e-3° within GDAL's single-precision rounding)"
    );
    // One line per case for tools/ci/oracle-pass-rates.sh.
    let ok = n > 1000 && bad_s == 0 && bad_a == 0;
    println!(
        "\nORACLE\tTerrain/slope\t{name}\t{}",
        if ok { "pass" } else { "fail" }
    );
    assert!(n > 1000);
    assert_eq!(bad_s, 0);
    assert_eq!(bad_a, 0);
}

#[test]
fn slope_and_aspect_match_gdaldem() {
    slope_aspect("square", true);
    // GDAL's aspect assumes square cells; only the slope is compared on oblong ones.
    slope_aspect("oblong", false);
}

/// Distance from `p` to the segment `a`–`b`.
fn seg(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
}

fn near(p: (f64, f64), lines: &[Vec<(f64, f64)>]) -> f64 {
    let mut best = f64::INFINITY;
    for l in lines {
        for w in l.windows(2) {
            best = best.min(seg(p, w[0], w[1]));
        }
        if l.len() == 1 {
            best = best.min((p.0 - l[0].0).hypot(p.1 - l[0].1));
        }
    }
    best
}

fn length(l: &[(f64, f64)]) -> f64 {
    l.windows(2)
        .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
        .sum()
}

fn contours_match(name: &str, w: usize, h: usize) {
    let text = std::fs::read_to_string(data(&format!("contours-{name}.tsv"))).unwrap();
    let mut gdal: Vec<(f64, Vec<(f64, f64)>)> = Vec::new();
    for l in text.lines().filter(|l| !l.starts_with('#')) {
        let mut it = l.split('\t');
        let level: f64 = it.next().unwrap().parse().unwrap();
        let pts = it
            .map(|p| {
                let (x, y) = p.split_once(',').unwrap();
                (x.parse().unwrap(), y.parse().unwrap())
            })
            .collect();
        gdal.push((level, pts));
    }
    let values = dem(name);
    let ours = contours(
        &values,
        w,
        h,
        &Levels {
            base: 0.0,
            interval: 25.0,
            index_every: 4,
        },
    );
    let mut levels: Vec<f64> = gdal.iter().map(|g| g.0).collect();
    levels.extend(ours.iter().map(|c| c.level));
    levels.sort_by(f64::total_cmp);
    levels.dedup();
    let (mut checked, mut far, mut max_d, mut max_len) = (0, 0, 0.0f64, 0.0f64);
    for level in levels {
        let g: Vec<Vec<(f64, f64)>> = gdal
            .iter()
            .filter(|x| x.0 == level)
            .map(|x| x.1.clone())
            .collect();
        let o: Vec<Vec<(f64, f64)>> = ours
            .iter()
            .filter(|c| c.level == level)
            .map(|c| {
                c.points
                    .iter()
                    .map(|&(x, y)| (f64::from(x), f64::from(y)))
                    .collect()
            })
            .collect();
        // Every vertex of each on the other's lines, both ways.
        for (a, b) in [(&g, &o), (&o, &g)] {
            for l in a.iter() {
                for &p in l {
                    checked += 1;
                    let d = near(p, b);
                    max_d = max_d.max(d);
                    if d > 1e-3 {
                        far += 1;
                    }
                }
            }
        }
        let (lg, lo): (f64, f64) = (
            g.iter().map(|l| length(l)).sum(),
            o.iter().map(|l| length(l)).sum(),
        );
        max_len = max_len.max((lg - lo).abs() / lg.max(1e-9));
    }
    println!(
        "{name}: {} GDAL lines, {} ours; {checked} vertices, {far} beyond 1e-3 px (max {max_d:.2e}), length within {max_len:.2e}",
        gdal.len(),
        ours.len()
    );
    let ok = checked > 500 && far == 0 && max_len < 1e-4;
    println!(
        "\nORACLE\tTerrain/contour\t{name}\t{}",
        if ok { "pass" } else { "fail" }
    );
    assert!(checked > 500);
    assert_eq!(far, 0);
    assert!(max_len < 1e-4);
}

#[test]
fn contours_match_gdal_contour() {
    contours_match("square", 64, 48);
    contours_match("oblong", 40, 56);
}

/// Timing on a large grid (run with `--ignored --nocapture`): contours of a 2000×2000
/// elevation model (the map view traces at most 4 million cells), and slope at every cell.
#[test]
#[ignore]
fn timing_large_grid() {
    let n = 2000usize;
    let mut v = Vec::with_capacity(n * n);
    for y in 0..n {
        for x in 0..n {
            let (u, t) = (x as f32 / n as f32, y as f32 / n as f32);
            v.push(
                800.0
                    + 600.0 * (u * 7.0).sin() * (t * 5.0).cos()
                    + 150.0 * (u * 31.0 + t * 17.0).sin(),
            );
        }
    }
    let t0 = std::time::Instant::now();
    let c = contours(
        &v,
        n,
        n,
        &Levels {
            base: 0.0,
            interval: 50.0,
            index_every: 5,
        },
    );
    let tc = t0.elapsed();
    let d = Dem {
        values: &v,
        w: n,
        h: n,
        step: [30.0, -30.0],
        z: 1.0,
    };
    let t1 = std::time::Instant::now();
    let mut s = 0.0;
    for y in 0..n {
        for x in 0..n {
            s += d.slope(x, y).unwrap_or(0.0);
        }
    }
    println!(
        "{n}×{n}: {} contour lines in {:.0} ms; slope of every cell in {:.0} ms ({s:.0})",
        c.len(),
        tc.as_secs_f64() * 1e3,
        t1.elapsed().as_secs_f64() * 1e3
    );
}
