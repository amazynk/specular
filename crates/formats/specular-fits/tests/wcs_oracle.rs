//! WCS pixel → world against astropy (wcslib), pinned in `tests/data/wcs_oracle.tsv`
//! (`gen/wcs_oracle.py`), plus the world → pixel round trip.

use specular_fits::Header;
use specular_fits::wcs::{Table, Wcs};

struct Case {
    name: String,
    cards: Vec<(String, String)>,
    points: Vec<(Vec<f64>, Vec<f64>)>,
    /// Lookup table extensions: (EXTNAME, EXTVER, table).
    tables: Vec<(String, i64, Table)>,
}

fn pair(s: &str) -> [f64; 2] {
    let v = nums(s);
    [v[0], v[1]]
}

fn nums(s: &str) -> Vec<f64> {
    s.split(',').map(|v| v.parse().unwrap()).collect()
}

fn cases(text: &str) -> Vec<Case> {
    let mut out: Vec<Case> = Vec::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let f: Vec<&str> = line.split('\t').collect();
        match f[0] {
            "case" => out.push(Case {
                name: f[1].into(),
                cards: Vec::new(),
                points: Vec::new(),
                tables: Vec::new(),
            }),
            "table" => out.last_mut().unwrap().tables.push((
                f[1].into(),
                f[2].parse().unwrap(),
                Table {
                    width: f[3].parse().unwrap(),
                    height: f[4].parse().unwrap(),
                    crpix: pair(f[5]),
                    crval: pair(f[6]),
                    cdelt: pair(f[7]),
                    values: nums(f[8]).into_iter().map(|v| v as f32).collect(),
                },
            )),
            "card" => out
                .last_mut()
                .unwrap()
                .cards
                .push((f[1].into(), f[2].into())),
            "pix" => out
                .last_mut()
                .unwrap()
                .points
                .push((nums(f[1]), nums(f[2]))),
            x => panic!("bad line kind {x}"),
        }
    }
    out
}

/// Angular distance between two (lng, lat) positions, degrees.
fn separation(a: (f64, f64), b: (f64, f64)) -> f64 {
    let r = std::f64::consts::PI / 180.0;
    let (l1, b1, l2, b2) = (a.0 * r, a.1 * r, b.0 * r, b.1 * r);
    let s = ((b2 - b1) / 2.0).sin().powi(2) + b1.cos() * b2.cos() * ((l2 - l1) / 2.0).sin().powi(2);
    2.0 * s.sqrt().min(1.0).asin() / r
}

#[test]
fn astropy_pixel_to_world() {
    let r = check(cases(include_str!("data/wcs_oracle.tsv")), true);
    assert!(r.refused.is_empty());
    assert!(r.matched > 1400);
}

/// astropy's own WCS test headers (every projection map, spectral cubes, SIP, TPV, HST
/// lookup tables, odd headers), from `tools/corpus/fetch-astrowcs.sh`: whatever is read
/// agrees with wcslib; what is refused is listed. Skips when the corpus is absent.
#[test]
fn astropy_corpus() {
    let dir = std::env::var_os("SPECULAR_ASTROWCS_CORPUS").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../tools/corpus/astrowcs")
        },
        std::path::PathBuf::from,
    );
    let Ok(text) = std::fs::read_to_string(dir.join("wcs/expected.tsv")) else {
        eprintln!("astrowcs corpus not found; skipping");
        return;
    };
    let all = cases(&text);
    let n = all.len();
    let r = check(all, false);
    for (name, why) in &r.refused {
        eprintln!("refused {name}: {why}");
    }
    eprintln!(
        "wcs corpus: {} of {n} files read, {} refused",
        n - r.refused.len(),
        r.refused.len()
    );
    assert!(r.matched > 1000);
}

struct Report {
    matched: usize,
    /// Cases not read: (name, why).
    refused: Vec<(String, String)>,
}

/// Every point of every case against astropy; panics on a mismatch. `strict`: every case
/// must be read (the pinned oracle); else refusals are reported.
fn check(all: Vec<Case>, strict: bool) -> Report {
    let mut matched = 0;
    let mut undefined = 0;
    let mut bad = Vec::new();
    let mut refused = Vec::new();
    let mut cases_seen = Vec::new();
    let mut worst = 0.0f64;
    for c in all {
        let h = Header::from_pairs(c.cards.iter().map(|(k, v)| (k.as_str(), v.as_str(), "")));
        let axes: Vec<usize> = (1..=3)
            .filter_map(|i| h.i64(&format!("NAXIS{i}")).map(|v| v as usize))
            .collect();
        let load = |ext: &str, ver: i64| {
            c.tables
                .iter()
                .find(|t| t.0 == ext && t.1 == ver)
                .map(|t| t.2.clone())
        };
        let Some(w) = Wcs::from_header_with(&h, &axes, &load) else {
            assert!(!strict, "{}: no WCS", c.name);
            refused.push((c.name.clone(), "no WCS".into()));
            continue;
        };
        if let Some(why) = &w.unsupported {
            assert!(!strict, "{}: {why:?}", c.name);
            refused.push((c.name.clone(), format!("{why:?}")));
            continue;
        }
        if strict && !c.tables.is_empty() {
            assert_eq!(w.distortions(), ["SIP", "D2IM", "CPDIS"]);
            // Without its tables the header is refused, not half-applied.
            let bare = Wcs::from_header(&h, &axes).unwrap();
            assert!(bare.celestial.is_none() && bare.unsupported.is_some());
        }
        let cel = w.celestial.clone();
        cases_seen.push(c.name.clone());
        // wcslib evaluates CSC's polynomials in single precision (as the COBE software),
        // this reader in double: they agree to f32's resolution, tens of mas on a 45° face.
        // The polynomial inverse only approximates the forward, so the round trip is
        // checked to its accuracy.
        let csc = w
            .celestial
            .as_ref()
            .is_some_and(|c| c.projection.code() == "CSC");
        let (cel_tol, trip_tol) = if csc {
            (50.0 / 3.6e6, 0.05)
        } else {
            (1e-9, 1e-5)
        };
        for (p, want) in &c.points {
            let got = w.pixel_to_world(p);
            if want.iter().any(|v| v.is_nan()) {
                if got.is_some() {
                    bad.push(format!("{} {p:?}: astropy undefined, got {got:?}", c.name));
                }
                undefined += 1;
                continue;
            }
            let Some(got) = got else {
                bad.push(format!("{} {p:?}: undefined, astropy {want:?}", c.name));
                continue;
            };
            for (i, (g, e)) in got.iter().zip(want).enumerate() {
                if cel.as_ref().is_some_and(|c| c.lng == i || c.lat == i) {
                    continue;
                }
                if (g - e).abs() > 1e-9 * e.abs().max(1.0) {
                    bad.push(format!("{} {p:?} axis {i}: got {g}, astropy {e}", c.name));
                }
            }
            if let Some(cc) = &cel {
                let d = separation((got[cc.lng], got[cc.lat]), (want[cc.lng], want[cc.lat]));
                if d > cel_tol {
                    bad.push(format!(
                        "{} {p:?}: got {:?}, astropy {:?} ({:.3} mas)",
                        c.name,
                        (got[cc.lng], got[cc.lat]),
                        (want[cc.lng], want[cc.lat]),
                        d * 3.6e6
                    ));
                }
                if !csc {
                    worst = worst.max(d);
                }
                // Round trip.
                let rest: Vec<f64> = p[2..].to_vec();
                // All-sky layouts show some points twice (a cube's or HEALPix's wrapped
                // edges): another pixel of the same sky is a round trip too.
                let same_sky = |x: f64, y: f64| {
                    let mut q = vec![x, y];
                    q.extend_from_slice(&rest);
                    w.pixel_to_world(&q).is_some_and(|b| {
                        separation((b[cc.lng], b[cc.lat]), (got[cc.lng], got[cc.lat]))
                            < if csc { 0.05 } else { 1e-8 }
                    })
                };
                match w.world_to_pixel(got[cc.lng], got[cc.lat], &rest) {
                    Some((x, y)) if (x - p[0]).abs() < trip_tol && (y - p[1]).abs() < trip_tol => {}
                    Some((x, y)) if same_sky(x, y) => {}
                    r => bad.push(format!("{} {p:?}: round trip {r:?}", c.name)),
                }
            }
            matched += 1;
        }
    }
    assert!(
        bad.is_empty(),
        "{} mismatches:\n{}",
        bad.len(),
        bad.join("\n")
    );
    eprintln!(
        "wcs oracle: {} cases, {matched} points matched (worst {:.2e} mas outside CSC), {undefined} \
         undefined in both",
        cases_seen.len(),
        worst * 3.6e6
    );
    Report { matched, refused }
}
