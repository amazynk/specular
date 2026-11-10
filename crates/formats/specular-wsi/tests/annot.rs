//! Slide annotation sidecars (tests/data/annot, written by tools/corpus/annot-fixtures.py)
//! against an independent reader (tools/corpus/annot-oracle.py: xml.etree and shapely,
//! expected.tsv): every shape's kind, name, group, colour, points, area and length, and the
//! shapes left out. The corpus test does the same for the fetched CAMELYON and ImageScope
//! files (tools/corpus/fetch-vendormicro.sh), when present.

use std::path::Path;
use std::time::Instant;

use specular_wsi::annot::{self, NdpaFrame};

struct Expected {
    shape: String,
    name: String,
    group: String,
    colour: String,
    points: Vec<(f64, f64)>,
    area: f64,
    length: f64,
}

/// Rows per file, and the shapes each file leaves out.
type Table = Vec<(String, Vec<Expected>, usize)>;

fn table(path: &Path) -> Table {
    let text = std::fs::read_to_string(path).unwrap();
    let mut out: Table = Vec::new();
    let mut rows = Vec::new();
    for line in text.lines().filter(|l| !l.is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        if f[0] == "skipped" {
            out.push((
                f[1].to_owned(),
                std::mem::take(&mut rows),
                f[2].parse().unwrap(),
            ));
            continue;
        }
        let n = |i: usize| f[i].parse::<f64>().unwrap();
        rows.push(Expected {
            shape: f[2].to_owned(),
            name: f[3].to_owned(),
            group: f[4].to_owned(),
            colour: f[5].to_owned(),
            points: f[13]
                .split(';')
                .map(|p| {
                    let (x, y) = p.split_once(',').unwrap();
                    (x.parse().unwrap(), y.parse().unwrap())
                })
                .collect(),
            area: n(11),
            length: n(12),
        });
    }
    out
}

fn frame(dir: &Path) -> Option<NdpaFrame> {
    let t = std::fs::read_to_string(dir.join("frame.txt")).ok()?;
    let v: Vec<f64> = t.split_whitespace().map(|s| s.parse().unwrap()).collect();
    Some(NdpaFrame {
        nm_per_px: (v[0], v[1]),
        offset_nm: (v[2], v[3]),
        width: v[4] as u32,
        height: v[5] as u32,
    })
}

/// Files, shapes and mismatches (one per differing shape or count).
fn run(dir: &Path) -> (usize, usize, usize) {
    let frame = frame(dir);
    let (mut files, mut shapes, mut bad) = (0, 0, 0);
    for (file, rows, skipped) in table(&dir.join("expected.tsv")) {
        let data = std::fs::read(dir.join(&file)).unwrap();
        let t = Instant::now();
        let got = annot::parse(&data, frame.as_ref()).unwrap_or_else(|e| panic!("{file}: {e}"));
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let mut notes = Vec::new();
        if got.items.len() != rows.len() || got.skipped != skipped {
            notes.push(format!(
                "{} shapes ({} left out), expected {} ({skipped})",
                got.items.len(),
                got.skipped,
                rows.len()
            ));
        }
        for (i, (a, e)) in got.items.iter().zip(&rows).enumerate() {
            let colour = a.colour.map_or("-".to_owned(), |c| {
                format!("{:02x}{:02x}{:02x}", c[0], c[1], c[2])
            });
            let close = |x: f64, y: f64| (x - y).abs() <= 1e-6 + 1e-5 * y.abs();
            let same_points = a.points.len() == e.points.len()
                && a.points
                    .iter()
                    .zip(&e.points)
                    .all(|(p, q)| (p.0 - q.0).abs() < 1e-5 && (p.1 - q.1).abs() < 1e-5);
            let length = if a.shape.closed()
                || !matches!(e.shape.as_str(), "polyline" | "ruler" | "arrow")
            {
                0.0
            } else {
                a.length(1.0, 1.0)
            };
            let ok = a.shape.id() == e.shape
                && a.name == e.name
                && a.group == e.group
                && colour == e.colour
                && same_points
                && close(a.area(), e.area)
                && close(length, e.length);
            if !ok {
                notes.push(format!(
                    "#{i}: {} {:?} {:?} {colour} {} points area {} length {length}; expected {} {:?} {:?} {} {} points area {} length {}",
                    a.shape.id(),
                    a.name,
                    a.group,
                    a.points.len(),
                    a.area(),
                    e.shape,
                    e.name,
                    e.group,
                    e.colour,
                    e.points.len(),
                    e.area,
                    e.length
                ));
            }
        }
        eprintln!(
            "{file:28} {:5} shapes {:3} mismatches  {ms:8.3} ms (debug build)",
            got.items.len(),
            notes.len()
        );
        for n in notes.iter().take(8) {
            eprintln!("    {n}");
        }
        files += 1;
        shapes += got.items.len();
        bad += notes.len();
    }
    (files, shapes, bad)
}

#[test]
fn fixtures_match_the_oracle() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/annot");
    let (files, shapes, bad) = run(&dir);
    eprintln!("annotation fixtures: {files} files, {shapes} shapes, {bad} mismatches");
    assert_eq!((files, shapes, bad), (4, 23, 0));
}

#[test]
fn corpus_matches_the_oracle() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/vendormicro/annot");
    if !dir.join("expected.tsv").is_file() {
        eprintln!("no annotation corpus (run tools/corpus/fetch-vendormicro.sh)");
        return;
    }
    let (files, shapes, bad) = run(&dir);
    eprintln!("annotation corpus: {files} files, {shapes} shapes, {bad} mismatches");
    assert_eq!(bad, 0);
}

#[test]
fn malformed_sidecars_never_panic() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/annot");
    let frame = frame(&dir);
    for name in ["aperio.xml", "aperio-utf16.xml", "asap.xml", "slide.ndpa"] {
        let data = std::fs::read(dir.join(name)).unwrap();
        for cut in (0..data.len()).step_by(data.len() / 61 + 1) {
            let _ = annot::parse(&data[..cut], frame.as_ref());
        }
        let mut d = data.clone();
        let mut x: u32 = 0x9e37_79b9;
        for _ in 0..300 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            let i = x as usize % d.len();
            d[i] = (x >> 8) as u8;
            let _ = annot::parse(&d, frame.as_ref());
        }
    }
}
