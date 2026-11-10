//! Tables, HEALPix maps, event lists and catalogues against the oracle values of
//! `gen/table_fixtures.py` (astropy.io.fits, astropy-healpix, astropy.wcs,
//! numpy.histogram2d, astropy.io.votable), on the fixtures in `tests/data/tables`.

use std::path::PathBuf;

use specular_fits::events::Events;
use specular_fits::healpix::{self, Map};
use specular_fits::table::{Datum, Table};
use specular_fits::wcs::Wcs;
use specular_fits::{Kind, catalogue, parse};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/tables")
}

fn read(name: &str) -> Vec<u8> {
    std::fs::read(dir().join(name)).unwrap()
}

fn oracle(name: &str) -> Vec<Vec<String>> {
    std::fs::read_to_string(dir().join(name))
        .unwrap()
        .lines()
        .map(|l| l.split('\t').map(str::to_owned).collect())
        .collect()
}

fn same_number(want: &str, got: f64) -> bool {
    match want {
        "nan" => got.is_nan(),
        _ => want.parse::<f64>().is_ok_and(|w| w == got),
    }
}

fn datum_matches(want: &str, got: &Datum) -> bool {
    match got {
        Datum::Null => want == "null",
        Datum::Bool(b) => want == if *b { "T" } else { "F" },
        Datum::Int(i) => want.parse::<i128>().is_ok_and(|w| w == i128::from(*i)),
        Datum::UInt(u) => want.parse::<i128>().is_ok_and(|w| w == i128::from(*u)),
        Datum::Float(f) => same_number(want, *f),
        Datum::Complex(a, b) => {
            let t = want.trim_start_matches('(').trim_end_matches(')');
            let mut p = t.split(", ");
            p.next().is_some_and(|x| same_number(x, *a))
                && p.next().is_some_and(|x| same_number(x, *b))
        }
        Datum::Str(s) => want == s,
    }
}

#[test]
fn table_values_match_astropy() {
    let data = read("alltypes.fits");
    let f = parse(&data).unwrap();
    let mut cells = 0;
    let mut bad = Vec::new();
    for l in oracle("alltypes.tsv") {
        let (h, r, c): (usize, usize, usize) = (
            l[1].parse().unwrap(),
            l[2].parse().unwrap(),
            l[3].parse().unwrap(),
        );
        let hdu = &f.hdus[h];
        assert!(matches!(hdu.kind, Kind::BinTable | Kind::Table));
        let t = Table::from_hdu(hdu).unwrap();
        let got = t.cell(&data, r, c);
        let want = &l[4..];
        // An empty string or an empty variable-length array.
        let want: Vec<&str> = if want.is_empty() && !got.is_empty() {
            vec![""]
        } else {
            want.iter().map(String::as_str).collect()
        };
        let ok = got.len() == want.len() && got.iter().zip(&want).all(|(g, w)| datum_matches(w, g));
        if !ok {
            bad.push(format!(
                "HDU {h} row {r} col {c} ({}): want {want:?}, got {got:?}",
                t.columns[c].name
            ));
        }
        cells += 1;
    }
    eprintln!("tables: {cells} cells, {} mismatches", bad.len());
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn healpix_pixels_match_astropy_healpix() {
    let mut n = 0;
    for l in oracle("healpix.tsv") {
        let nest = l.get(2).is_some_and(|s| s == "nest");
        match l[0].as_str() {
            "ang2pix" => {
                let nside: u64 = l[1].parse().unwrap();
                let (t, p): (f64, f64) = (l[3].parse().unwrap(), l[4].parse().unwrap());
                let want: u64 = l[5].parse().unwrap();
                let got = if nest {
                    healpix::ang2pix_nest(nside, t, p)
                } else {
                    healpix::ang2pix_ring(nside, t, p)
                };
                assert_eq!(got, want, "ang2pix {l:?}");
            }
            "pix2ang" => {
                let nside: u64 = l[1].parse().unwrap();
                let pix: u64 = l[3].parse().unwrap();
                let (t, p): (f64, f64) = (l[4].parse().unwrap(), l[5].parse().unwrap());
                let (gt, gp) = if nest {
                    healpix::pix2ang_nest(nside, pix)
                } else {
                    healpix::pix2ang_ring(nside, pix)
                };
                let dp = (gp - p).rem_euclid(std::f64::consts::TAU);
                assert!(
                    (gt - t).abs() < 1e-12 && dp.min(std::f64::consts::TAU - dp) < 1e-12,
                    "pix2ang {l:?}: {gt} {gp}"
                );
            }
            "uniq" => {
                let (o, p, u): (u32, u64, u64) = (
                    l[1].parse().unwrap(),
                    l[2].parse().unwrap(),
                    l[3].parse().unwrap(),
                );
                assert_eq!(healpix::to_uniq(o, p), u);
                assert_eq!(healpix::from_uniq(u), Some((o, p)));
            }
            _ => continue,
        }
        n += 1;
    }
    eprintln!("healpix pixels: {n} checks, 0 mismatches");
    assert!(n > 3000);
}

fn map_of(data: &[u8]) -> (Map, healpix::Index) {
    let f = parse(data).unwrap();
    let mut m = Map::from_hdu(&f.hdus[1]).unwrap().unwrap();
    let idx = m.index(data).unwrap();
    (m, idx)
}

#[test]
fn mollweide_and_credible_match_astropy() {
    let lines = oracle("healpix.tsv");
    let mut cache: Option<(String, usize, Vec<f32>)> = None;
    let (mut n, mut bad) = (0, Vec::new());
    for l in lines.iter().filter(|l| l[0] == "mol") {
        let (name, field, w) = (
            &l[1],
            l[2].parse::<usize>().unwrap(),
            l[3].parse::<usize>().unwrap(),
        );
        if cache.as_ref().is_none_or(|c| c.0 != *name || c.1 != field) {
            let data = read(name);
            let (m, idx) = map_of(&data);
            let r = m.mollweide(&data, &idx, field, w, &|| false).unwrap();
            cache = Some((name.clone(), field, r));
        }
        let r = &cache.as_ref().unwrap().2;
        let (i, j): (usize, usize) = (l[4].parse().unwrap(), l[5].parse().unwrap());
        let got = r[j * w + i];
        if !same_number(&l[6], f64::from(got)) {
            bad.push(format!(
                "{name} field {field} at {i},{j}: want {}, got {got}",
                l[6]
            ));
        }
        n += 1;
    }
    for l in lines.iter().filter(|l| l[0] == "credible") {
        let data = read(&l[1]);
        let (m, _) = map_of(&data);
        let p = m.probability_field().unwrap();
        let lv: f64 = l[2].parse().unwrap();
        let got = m.credible(&data, p, &[lv]).unwrap()[0];
        if !same_number(&l[3], got) {
            bad.push(format!("{} credible {lv}: want {}, got {got}", l[1], l[3]));
        }
        n += 1;
    }
    eprintln!("healpix maps: {n} values, {} mismatches", bad.len());
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn event_binning_matches_numpy() {
    let lines = oracle("events.tsv");
    let (mut n, mut bad) = (0, Vec::new());
    for name in ["chandra.fits", "xmm.fits"] {
        let data = read(name);
        let f = parse(&data).unwrap();
        let ev = Events::from_hdu(&data, &f.hdus[1]).unwrap().unwrap();
        let mine = lines.iter().filter(|l| l[1] == name);
        let mut planes = std::collections::HashMap::new();
        for l in mine {
            match l[0].as_str() {
                "grid" => {
                    let want = (
                        l[2].parse::<f64>().unwrap(),
                        l[3].parse::<f64>().unwrap(),
                        l[4].parse::<f64>().unwrap(),
                    );
                    assert_eq!((ev.base, ev.x0, ev.y0), want, "{name} grid");
                    assert_eq!(
                        (ev.width, ev.height),
                        (l[5].parse().unwrap(), l[6].parse().unwrap())
                    );
                }
                "bin" | "hist" => {
                    let (fi, band): (usize, usize) = (l[2].parse().unwrap(), l[3].parse().unwrap());
                    let k = specular_fits::events::FACTORS[fi] as usize;
                    let p = planes
                        .entry((fi, band))
                        .or_insert_with(|| ev.bin(&data, fi, band, &|| false).unwrap());
                    // Coarse bin (i, j) from the bottom: its value at its block's top-left.
                    let at = |i: usize, j: usize| {
                        let row = ev.height - 1 - (j * k);
                        p[row * ev.width + i * k]
                    };
                    if l[0] == "bin" {
                        let (i, j, c): (usize, usize, f32) = (
                            l[4].parse().unwrap(),
                            l[5].parse().unwrap(),
                            l[6].parse().unwrap(),
                        );
                        if at(i, j) != c {
                            bad.push(format!(
                                "{name} {fi}/{band} bin {i},{j}: want {c}, got {}",
                                at(i, j)
                            ));
                        }
                    } else {
                        let (cw, ch) = (ev.width.div_ceil(k), ev.height.div_ceil(k));
                        let (mut sum, mut fp) = (0i64, 0i64);
                        for i in 0..cw {
                            for j in 0..ch {
                                let c = at(i, j) as i64;
                                sum += c;
                                fp += c * (((i * 7919 + j * 104729) % 1000003) as i64);
                            }
                        }
                        let want: (i64, i64) = (l[4].parse().unwrap(), l[5].parse().unwrap());
                        if (sum, fp) != want {
                            bad.push(format!(
                                "{name} {fi}/{band}: want {want:?}, got {:?}",
                                (sum, fp)
                            ));
                        }
                    }
                }
                "wcs" => {
                    let h = ev.wcs_header(&f.hdus[1].header).unwrap();
                    let w = Wcs::from_header(&h, &[ev.width, ev.height]).unwrap();
                    let (i, j): (usize, usize) = (l[2].parse().unwrap(), l[3].parse().unwrap());
                    let (a, d) = w
                        .sky_at(i as f64 + 0.5, (ev.height - 1 - j) as f64 + 0.5, &[])
                        .unwrap();
                    let (wa, wd): (f64, f64) = (l[4].parse().unwrap(), l[5].parse().unwrap());
                    if (a - wa).abs() > 1e-9 || (d - wd).abs() > 1e-9 {
                        bad.push(format!("{name} wcs {i},{j}: want {wa},{wd}, got {a},{d}"));
                    }
                }
                _ => continue,
            }
            n += 1;
        }
    }
    eprintln!("events: {n} checks, {} mismatches", bad.len());
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn catalogues_match_astropy() {
    let lines = oracle("catalogue.tsv");
    let mut n = 0;
    for name in ["cat.vot", "cat2.vot", "cat.fits", "cat.csv"] {
        let c = catalogue::parse(&read(name)).unwrap();
        let want: Vec<(f64, f64)> = lines
            .iter()
            .filter(|l| l[1] == name)
            .map(|l| (l[2].parse().unwrap(), l[3].parse().unwrap()))
            .collect();
        assert_eq!(c.lng.len(), want.len(), "{name}");
        for (k, (a, d)) in want.iter().enumerate() {
            assert!(
                (c.lng[k] - a).abs() < 1e-9 && (c.lat[k] - d).abs() < 1e-9,
                "{name} row {k}: {a},{d} vs {},{}",
                c.lng[k],
                c.lat[k]
            );
            n += 1;
        }
        assert!(!c.galactic);
        assert_eq!(c.rows[0].len(), c.columns.len());
    }
    eprintln!("catalogues: {n} positions, 0 mismatches");
}
