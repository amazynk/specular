//! specular-geodesy against reference implementations, from tables written by
//! `gen/oracle.py`: GeographicLib (geodesics, polygon areas), PROJ (UTM, UPS) and GeoTrans
//! (MGRS). The largest errors are printed (`cargo test -- --nocapture`).

use specular_geodesy::{coords, geodesic, mgrs, utm};

fn rows(name: &str) -> Vec<Vec<String>> {
    let path = format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{path}: {e}"))
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| l.split('\t').map(str::to_owned).collect())
        .collect()
}

fn f(s: &str) -> f64 {
    s.parse().unwrap()
}

/// Azimuth difference in degrees, wrapped.
fn dazi(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

#[test]
fn inverse_matches_geographiclib() {
    let mut max_s = 0.0f64;
    let mut max_az = 0.0f64;
    let mut max_lat = 0.0f64;
    let mut n = 0;
    for r in rows("inverse.tsv") {
        let v: Vec<f64> = r.iter().map(|s| f(s)).collect();
        let g = geodesic::inverse(v[0], v[1], v[2], v[3]);
        let ds = (g.s12 - v[4]).abs();
        max_s = max_s.max(ds);
        // Azimuths are undefined at a pole and for coincident points; antipodal points on
        // the equator or across a pole have several shortest lines.
        let at_pole = v[0].abs() == 90.0 || v[2].abs() == 90.0;
        let degenerate = v[4] == 0.0 || at_pole || (v[0] + v[2] == 0.0 && v[4] > 2.0e7);
        if !degenerate {
            // As a sideways offset at the far end: azimuths of very short lines are
            // ill-conditioned (GeographicLib's C and Python builds differ there by 1e-7°).
            let da = dazi(g.azi1, v[5]).max(dazi(g.azi2, v[6]));
            let lateral = da.to_radians() * v[4];
            // Nearly antipodal lines are ill-conditioned too: the C library's last bits
            // (Apple's against glibc's) move their azimuths by ~1e-11°, micrometres at 20 000 km.
            let limit = if v[4] > 1.99e7 { 1e-5 } else { 1e-6 };
            assert!(lateral < limit, "{r:?}: {g:?}");
            max_lat = max_lat.max(lateral);
            if v[4] > 1000.0 {
                max_az = max_az.max(da);
            }
        }
        assert!(ds < 1e-6, "{r:?}: {g:?}");
        n += 1;
    }
    println!(
        "inverse: {n} lines, max |Δs| {max_s:.3e} m, max |Δazimuth| {max_az:.3e}° (lines over 1 km), max azimuth error as an offset {max_lat:.3e} m"
    );
    assert!(max_s < 1e-3);
}

#[test]
fn direct_matches_geographiclib() {
    let mut max_d = 0.0f64;
    let mut max_az = 0.0f64;
    let mut n = 0;
    for r in rows("direct.tsv") {
        let v: Vec<f64> = r.iter().map(|s| f(s)).collect();
        let (la, lo, az) = geodesic::direct(v[0], v[1], v[2], v[3]);
        // The position error in metres, from the inverse between the two answers.
        let d = geodesic::inverse(la, lo, v[4], v[5]).s12;
        max_d = max_d.max(d);
        max_az = max_az.max(dazi(az, v[6]));
        n += 1;
    }
    println!("direct: {n} lines, max position error {max_d:.3e} m, max |Δazimuth| {max_az:.3e}°");
    assert!(max_d < 1e-6, "{max_d}");
    assert!(max_az < 1e-8, "{max_az}");
}

#[test]
fn area_matches_geographiclib() {
    let mut max_rel = 0.0f64;
    let mut max_abs = 0.0f64;
    let mut max_p = 0.0f64;
    let mut n = 0;
    for r in rows("area.tsv") {
        let v: Vec<f64> = r.iter().map(|s| f(s)).collect();
        let pts: Vec<(f64, f64)> = v[2..].chunks(2).map(|c| (c[0], c[1])).collect();
        let (a, p) = geodesic::polygon_area(&pts);
        let da = (a - v[0]).abs();
        // Round-off is about 1e-5 m² per edge whatever the size (GeographicLib's C and
        // Python builds differ by as much on small rings): relative for large areas.
        assert!(da < 1e-9 * v[0] + 1e-3, "{r:?}: {a} vs {}", v[0]);
        max_abs = max_abs.max(da);
        if v[0] > 1.0e6 {
            max_rel = max_rel.max(da / v[0]);
        }
        max_p = max_p.max((p - v[1]).abs());
        n += 1;
    }
    println!(
        "area: {n} polygons, max error {max_abs:.3e} m², max relative error {max_rel:.3e} (areas over 1 km²), max perimeter error {max_p:.3e} m"
    );
}

#[test]
fn utm_matches_proj() {
    let mut max_fwd = 0.0f64;
    let mut max_inv = 0.0f64;
    let mut n = 0;
    for r in rows("utm.tsv") {
        let (la, lo) = (f(&r[0]), f(&r[1]));
        let zone: u8 = r[2].parse().unwrap();
        let north = r[3] == "1";
        let (e, nn) = (f(&r[4]), f(&r[5]));
        let u = utm::to_utm_in(la, lo, zone, north).unwrap();
        let d = (u.easting - e).hypot(u.northing - nn);
        max_fwd = max_fwd.max(d);
        let (la2, lo2) = utm::from_utm(&utm::Utm {
            zone,
            north,
            easting: e,
            northing: nn,
        })
        .unwrap();
        // The inverse's error as a distance on the ground.
        let di = geodesic::inverse(la, lo, la2, lo2).s12;
        max_inv = max_inv.max(di);
        n += 1;
    }
    println!(
        "utm/ups: {n} points, max forward error {max_fwd:.3e} m, max inverse error {max_inv:.3e} m"
    );
    assert!(max_fwd < 1e-6, "{max_fwd}");
    assert!(max_inv < 1e-6, "{max_inv}");
}

/// GeoTrans writes zones with a leading zero and no spaces.
fn normalise(s: &str) -> String {
    let t: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    t.strip_prefix('0').map(str::to_owned).unwrap_or(t)
}

#[test]
fn mgrs_matches_geotrans() {
    let mut n = 0;
    let mut mismatches = Vec::new();
    let mut max_back = 0.0f64;
    for r in rows("mgrs.tsv") {
        let (la, lo) = (f(&r[0]), f(&r[1]));
        let ours = mgrs::to_mgrs(la, lo, 5).unwrap();
        if normalise(&ours) != normalise(&r[2]) {
            mismatches.push(format!("{la} {lo}: ours {ours}, GeoTrans {}", r[2]));
        }
        // Back to the cell's centre: within the 1 m cell.
        let (la2, lo2) = mgrs::from_mgrs(&r[2]).unwrap();
        let d = geodesic::inverse(la, lo, la2, lo2).s12;
        max_back = max_back.max(d);
        n += 1;
    }
    println!(
        "mgrs: {n} points, {} mismatches, max distance to the parsed cell centre {max_back:.3} m",
        mismatches.len()
    );
    for m in &mismatches {
        println!("  {m}");
    }
    assert!(max_back < 1.0, "{max_back}");
    assert!(mismatches.len() * 200 <= n, "{mismatches:#?}");
}

/// Every format reads back (random points, zone and band edges, Norway and Svalbard, the
/// poles, ±180°, the equator).
#[test]
fn formats_round_trip() {
    let mut pts: Vec<(f64, f64)> = Vec::new();
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut rnd = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    for _ in 0..3000 {
        pts.push((rnd() * 180.0 - 90.0, rnd() * 360.0 - 180.0));
    }
    for lat in [
        -89.99, -80.0, -79.99, -8.0, 0.0, 0.0001, 55.99, 56.0, 63.99, 72.0, 83.99, 84.0, 84.01,
        89.99,
    ] {
        for lon in [
            -180.0, -179.9999, -3.0, 0.0, 2.999, 3.0, 8.999, 9.0, 11.999, 21.0, 32.999, 33.0,
            41.999, 42.0, 179.9999,
        ] {
            pts.push((lat, lon));
        }
    }
    let mut worst = [0.0f64; 4];
    for &(lat, lon) in &pts {
        for (i, fmt) in coords::CoordFormat::ALL.into_iter().enumerate() {
            let s = coords::format(lat, lon, fmt).unwrap_or_else(|| panic!("{lat} {lon} {fmt:?}"));
            let (la, lo) = coords::parse(&s).unwrap_or_else(|e| panic!("{s}: {e:?}"));
            let d = geodesic::inverse(lat, lon, la, lo).s12;
            worst[i] = worst[i].max(d);
            // DD: 6 decimals (≤ 0.08 m); DMS: 0.01″ (≤ 0.22 m); UTM: 1 m rounding;
            // MGRS: the centre of a 1 m cell.
            let tol = [0.1, 0.25, 0.75, 0.75][i];
            assert!(d < tol, "{lat} {lon} {fmt:?}: {s} → {la} {lo} ({d} m)");
        }
    }
    println!(
        "round trips over {} points: max error dd {:.3} m, dms {:.3} m, utm {:.3} m, mgrs {:.3} m",
        pts.len(),
        worst[0],
        worst[1],
        worst[2],
        worst[3]
    );
}
