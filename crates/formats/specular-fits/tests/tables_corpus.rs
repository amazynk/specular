//! The FITS table corpus (`tools/corpus/fetch-fitstables.sh`: a Planck HEALPix map, LIGO/Virgo
//! flat and multi-order sky maps, Chandra and XMM-Newton event lists, VOTables) against the
//! oracle values of `gen/fitstables_oracle.py` (astropy.io.fits, astropy-healpix,
//! astropy.wcs, numpy.histogram2d, astropy.io.votable). Skipped when the corpus is absent:
//! it is read from `SPECULAR_FITSTABLES_CORPUS`, else from `tools/corpus/fitstables`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use specular_fits::events::{Events, FACTORS};
use specular_fits::healpix::Map;
use specular_fits::table::{Datum, Table};
use specular_fits::wcs::Wcs;
use specular_fits::{catalogue, parse};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_FITSTABLES_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/fitstables")
        });
    if dir.join("expected.tsv").is_file() {
        Some(dir)
    } else {
        eprintln!("FITS table corpus absent ({}); skipped", dir.display());
        None
    }
}

fn oracle(dir: &std::path::Path, tag: &str) -> Vec<Vec<String>> {
    std::fs::read_to_string(dir.join("expected.tsv"))
        .unwrap()
        .lines()
        .map(|l| l.split('\t').map(str::to_owned).collect::<Vec<_>>())
        .filter(|l| l[0] == tag)
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
fn corpus_table_cells() {
    let Some(dir) = corpus() else { return };
    let mut files: HashMap<String, Vec<u8>> = HashMap::new();
    let (mut n, mut bad) = (0, Vec::new());
    for l in oracle(&dir, "cell") {
        let data = files
            .entry(l[1].clone())
            .or_insert_with(|| std::fs::read(dir.join(&l[1])).unwrap());
        let f = parse(data).unwrap();
        let (h, r, c): (usize, usize, usize) = (
            l[2].parse().unwrap(),
            l[3].parse().unwrap(),
            l[4].parse().unwrap(),
        );
        let t = Table::from_hdu(&f.hdus[h]).unwrap();
        let got = t.cell(data, r, c);
        let want = &l[5..];
        let want: Vec<&str> = if want.is_empty() && !got.is_empty() {
            vec![""]
        } else {
            want.iter().map(String::as_str).collect()
        };
        if !(got.len() == want.len() && got.iter().zip(&want).all(|(g, w)| datum_matches(w, g))) {
            bad.push(format!(
                "{} HDU {h} row {r} col {c} ({}): want {want:?}, got {got:?}",
                l[1], t.columns[c].name
            ));
        }
        n += 1;
    }
    eprintln!("corpus tables: {n} cells, {} mismatches", bad.len());
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn corpus_healpix_maps() {
    let Some(dir) = corpus() else { return };
    let (mut n, mut bad) = (0, Vec::new());
    let mol = oracle(&dir, "mol");
    let credible = oracle(&dir, "credible");
    let mut names: Vec<&String> = mol.iter().map(|l| &l[1]).collect();
    names.dedup();
    for name in names {
        let data = std::fs::read(dir.join(name)).unwrap();
        let t0 = Instant::now();
        let f = parse(&data).unwrap();
        let mut m = Map::from_hdu(&f.hdus[1]).unwrap().unwrap();
        let idx = m.index(&data).unwrap();
        let mut rasters: HashMap<usize, Vec<f32>> = HashMap::new();
        for l in mol.iter().filter(|l| l[1] == *name) {
            let (field, w): (usize, usize) = (l[2].parse().unwrap(), l[3].parse().unwrap());
            let r = rasters
                .entry(field)
                .or_insert_with(|| m.mollweide(&data, &idx, field, w, &|| false).unwrap());
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
        for l in credible.iter().filter(|l| l[1] == *name) {
            let lv: f64 = l[2].parse().unwrap();
            let got = m
                .credible(&data, m.probability_field().unwrap(), &[lv])
                .unwrap()[0];
            if !same_number(&l[3], got) {
                bad.push(format!("{name} credible {lv}: want {}, got {got}", l[3]));
            }
            n += 1;
        }
        eprintln!(
            "{name}: nside {}, {} fields, checked in {:?}",
            m.nside,
            m.fields.len(),
            t0.elapsed()
        );
    }
    eprintln!("corpus HEALPix: {n} values, {} mismatches", bad.len());
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn corpus_event_binning() {
    let Some(dir) = corpus() else { return };
    let (mut n, mut bad) = (0, Vec::new());
    for name in ["chandra-635-evt2.fits", "xmm-0123700101-m1.fits"] {
        let data = std::fs::read(dir.join(name)).unwrap();
        let t0 = Instant::now();
        let f = parse(&data).unwrap();
        let ev = Events::from_hdu(&data, &f.hdus[1]).unwrap().unwrap();
        let opened = t0.elapsed();
        let mut planes = HashMap::new();
        for l in oracle(&dir, "grid").iter().filter(|l| l[1] == name) {
            let want = (
                l[2].parse::<f64>().unwrap(),
                l[3].parse::<f64>().unwrap(),
                l[4].parse::<f64>().unwrap(),
            );
            assert_eq!((ev.base, ev.x0, ev.y0), want, "{name} grid");
            assert_eq!(
                (ev.width, ev.height),
                (l[5].parse().unwrap(), l[6].parse().unwrap()),
                "{name} size"
            );
            n += 1;
        }
        for l in oracle(&dir, "hist").iter().filter(|l| l[1] == name) {
            let (fi, band): (usize, usize) = (l[2].parse().unwrap(), l[3].parse().unwrap());
            let k = FACTORS[fi] as usize;
            let p = planes
                .entry((fi, band))
                .or_insert_with(|| ev.bin(&data, fi, band, &|| false).unwrap());
            let (cw, ch) = (ev.width.div_ceil(k), ev.height.div_ceil(k));
            let (mut sum, mut fp) = (0i64, 0i64);
            for i in 0..cw {
                for j in 0..ch {
                    let c = p[(ev.height - 1 - j * k) * ev.width + i * k] as i64;
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
            n += 1;
        }
        let h = ev.wcs_header(&f.hdus[1].header).unwrap();
        let w = Wcs::from_header(&h, &[ev.width, ev.height]).unwrap();
        for l in oracle(&dir, "wcs").iter().filter(|l| l[1] == name) {
            let (i, j): (usize, usize) = (l[2].parse().unwrap(), l[3].parse().unwrap());
            let (a, d) = w
                .sky_at(i as f64 + 0.5, (ev.height - 1 - j) as f64 + 0.5, &[])
                .unwrap();
            let (wa, wd): (f64, f64) = (l[4].parse().unwrap(), l[5].parse().unwrap());
            if (a - wa).abs() > 1e-9 || (d - wd).abs() > 1e-9 {
                bad.push(format!("{name} wcs {i},{j}: want {wa},{wd}, got {a},{d}"));
            }
            n += 1;
        }
        eprintln!(
            "{name}: {} events, {}×{} at {} px, opened in {opened:?}",
            ev.table.rows, ev.width, ev.height, ev.base
        );
    }
    eprintln!("corpus events: {n} checks, {} mismatches", bad.len());
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn corpus_votables() {
    let Some(dir) = corpus() else { return };
    let pos = oracle(&dir, "pos");
    let mut n = 0;
    for name in ["vizier.vot", "gaia-binary2.vot"] {
        let c = catalogue::parse(&std::fs::read(dir.join(name)).unwrap()).unwrap();
        let want: Vec<(f64, f64)> = pos
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
    }
    eprintln!("corpus VOTables: {n} positions, 0 mismatches");
}
