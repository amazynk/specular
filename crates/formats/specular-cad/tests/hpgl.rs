//! HP-GL and HP-GL/2: synthetic plots, one per command group, each against a golden text
//! dump of the scene (tests/data/hpgl/*.txt; `SPECULAR_BLESS=1` rewrites them), and
//! robustness (truncation, garbage, limits).

use std::fmt::Write as _;
use std::path::PathBuf;

use kurbo::{PathEl, Rect, Shape as _};
use specular_cad::hpgl::{extents, is_hpgl, parse};
use specular_cad::{CadError, Limits};
use specular_vector::{Color, Shape, VectorScene};

/// Every command group, as a plot of its own.
const FIXTURES: &[(&str, &[u8])] = &[
    (
        "lines",
        b"IN;SP1;PU0,0;PD400,0,400,400,0,400,0,0;PU;PR100,100;PD200,0,0,200;PU;PA1000,1000;\
          PD;PA1400,1000 1400 1400;DF;PU;PA2000,0;PD2400,0;",
    ),
    (
        "arcs",
        b"IN;SP1;PA400,400;CI200;PU600,400;PD;AA400,400,90;AR-200,0,-45;PU;PA1000,0;PD;\
          AT1200,200,1400,0;RT200,-200,400,0;PU;PA2000,0;PD;BZ2100,200,2300,200,2400,0;\
          BR100,-200,300,-200,400,0;",
    ),
    (
        "fills",
        b"IN;SP2;PA0,0;FT1;RA400,400;EA400,400;PA600,0;FT3,50,45;RR400,400;ER400,400;\
          PA1200,0;FT4,80,0;RA1600,400;PA2000,200;FT10,30;WG200,0,90;EW200,90,180;\
          FT1;PA0,800;PM0;PD400,800,400,1200,0,1200;PM1;PU100,900;PD300,900,300,1100,100,1100;\
          PM2;FP;EP;PU;PA600,800;PM0;PD1000,800,1000,1200,600,1200;PM2;FP1;AC5,5;\
          FT3,40,30;PU;PA1200,800;RR300,300;",
    ),
    (
        "pens",
        b"IN;NP16;PC2,0,0,255;CR0,100,0,100,0,100;PC3,50,0,100;WU0;PW0.7,2;PW0.18,3;SP2;\
          PU0,0;PD1000,0;SP3;PU0,200;PD1000,200;SP9;PU0,400;PD1000,400;WU1;PW1;SP4;\
          PU0,600;PD1000,600;TR0;LA1,4,2,4;SP1;LT2;PU0,800;PD1000,800;LT4,2,1;PU0,1000;\
          PD1000,1000;UL5,10,20,40,30;LT5;PU0,1200;PD1000,1200;LT;PU0,1400;PD1000,1400;\
          LT99;PU0,1600;PD1000,1600;",
    ),
    (
        "frame",
        b"IN;IP0,0,4000,4000;SC0,100,0,100;SP1;PU0,0;PD100,0,100,100,0,100,0,0;\
          SC0,100,0,50,1,0,0;PU0,0;PD100,50;SC0,2,0,4,2;PU10,10;PD20,20;SC;IR0,0,50,50;\
          SC0,10,0,10;PU0,0;PD10,10;IP;SC;RO90;PU0,0;PD400,0;CI100;RO0;PS20000,10000;\
          PU0,0;PD400,0;",
    ),
    (
        "labels",
        b"IN;SP1;PA0,0;LBHello\x03PA0,500;SI0.5,0.8;LBBig\x03SR1,2;PA0,1000;LBRelative\x03\
          SI;DI0,1;PA2000,0;LBUp\x03DR1,1;PA2500,0;LBDiag\x03DI;LO5;PA0,2000;LBCentre\x03\
          LO19;LBTopRight\x03LO1;PA0,3000;LBline1\r\nline2\x03CP2,0;LBafter\x03CP;\
          DT#,1;LBterm#SL0.5;LBslant#DT;SL;SD1,21,2,1,3,10,4,24,5,1,6,3;SS;PA0,4000;\
          LBfont\x03AD4,18;SA;LBalt\x03SS;CS0;SM*;PA4000,0,4100,100;SM;",
    ),
    (
        "pages",
        b"BP1,\"Two pages\";IN;SP1;PA0,0;PD100,0;PG;SP2;PA0,0;PD0,100;PG1;PG;",
    ),
    (
        "encoded",
        // PE: pen 1, relative (100,50), then absolute (7,9) pen up, 7-bit (10,0).
        b"IN;PA0,0;PE:\xc1G\xc2c\xc0<=\xcd\xd1;PE7s_;",
    ),
    (
        "pcl",
        b"\x1b%-12345X@PJL JOB NAME=\"t\"\r\n@PJL ENTER LANGUAGE=PCL\r\n\x1bE\x1b&l1O\
          \x1b*c7200x5040Y\x1b%1BIN;SP1;PA0,0;PD1000,0,1000,1000;\x1b%0A\x1b*b2W\x01\x02\
          \x1bE\x1b%1BIN;SP2;PA0,0;PD500,500;\x1b%0A\x1bE\x1b%-12345X",
    ),
    (
        "plotter",
        b"\x1b.(;\x1b.I81;;17:\x1b.N;19:IN;SP1;PU0,0;PD100,100;SP0;PD200,0;SP3;PD300,100;\
          XT;YT;TL2,2;XT;ZZ1;QQ;PU;",
    ),
];

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/hpgl")
}

fn r(v: f64) -> String {
    let v = (v * 1000.0).round() / 1000.0;
    let v = if v == 0.0 { 0.0 } else { v };
    format!("{v}")
}

fn rect(e: Option<Rect>) -> String {
    e.map_or("-".into(), |e| {
        format!("{} {} {} {}", r(e.x0), r(e.y0), r(e.x1), r(e.y1))
    })
}

fn color(c: Color) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Foreground => "fg".into(),
        Color::Background => "bg".into(),
        Color::Clear => "clear".into(),
    }
}

fn path(p: &kurbo::BezPath) -> String {
    let mut s = String::new();
    for el in p.elements() {
        match el {
            PathEl::MoveTo(a) => write!(s, " M{},{}", r(a.x), r(a.y)),
            PathEl::LineTo(a) => write!(s, " L{},{}", r(a.x), r(a.y)),
            PathEl::QuadTo(_, a) => write!(s, " Q{},{}", r(a.x), r(a.y)),
            PathEl::CurveTo(_, _, a) => write!(s, " C{},{}", r(a.x), r(a.y)),
            PathEl::ClosePath => write!(s, " Z"),
        }
        .unwrap();
    }
    if s.len() > 600 {
        let mut cut = 600;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        s.truncate(cut);
        s.push_str(" ...");
    }
    s
}

/// The scene as text: what a golden holds.
fn dump(s: &VectorScene) -> String {
    let mut o = String::new();
    let p = s.plot.as_ref().unwrap();
    writeln!(
        o,
        "format {} units {} pcl {} pens {:?} paper {:?} name {:?} lwdisplay {}",
        s.format, s.units.code, p.pcl, p.pens, p.paper_mm, p.name, s.lineweight_display
    )
    .unwrap();
    for l in &s.layers {
        writeln!(
            o,
            "layer {:?} {} lw {} items {}",
            l.name,
            color(l.color),
            l.lineweight,
            l.items
        )
        .unwrap();
    }
    for l in &s.linetypes {
        let pat: Vec<String> = l.pattern.iter().map(|v| r(*v)).collect();
        writeln!(o, "linetype {:?} [{}]", l.name, pat.join(" ")).unwrap();
    }
    let counts: Vec<String> = s.counts.iter().map(|(k, v)| format!("{k}={v}")).collect();
    writeln!(o, "counts {}", counts.join(" ")).unwrap();
    for w in &s.warnings {
        writeln!(o, "warning {w}").unwrap();
    }
    for sh in &s.sheets {
        writeln!(o, "sheet {:?} extents {}", sh.name, rect(sh.extents)).unwrap();
        for it in &sh.items {
            write!(o, "  L{} {} ", it.layer, color(it.color)).unwrap();
            match &it.shape {
                Shape::Stroke {
                    path: p,
                    weight,
                    dash,
                } => {
                    write!(o, "stroke w{weight}").unwrap();
                    if let Some(d) = dash {
                        write!(o, " dash{}x{}", d.linetype, r(d.scale)).unwrap();
                    }
                    writeln!(o, "{}", path(p)).unwrap();
                }
                Shape::Fill { path: p, even_odd } => {
                    writeln!(o, "fill eo{}{}", u8::from(*even_odd), path(p)).unwrap();
                }
                Shape::Text(t) => {
                    let c = t.transform.as_coeffs();
                    let sp = &t.paragraphs[0].spans[0];
                    writeln!(
                        o,
                        "text {:?} h{} wf{} obl{} {:?} {:?} at {},{} x {},{} it{} b{}",
                        t.plain(),
                        r(sp.height),
                        r(sp.width_factor),
                        r(sp.oblique),
                        t.halign,
                        t.valign,
                        r(c[4]),
                        r(c[5]),
                        r(c[0]),
                        r(c[1]),
                        u8::from(sp.italic),
                        u8::from(sp.bold)
                    )
                    .unwrap();
                }
                Shape::Point(p) => writeln!(o, "point {},{}", r(p.x), r(p.y)).unwrap(),
            }
        }
    }
    o
}

#[test]
fn goldens() {
    let bless = std::env::var_os("SPECULAR_BLESS").is_some_and(|v| v == "1");
    let mut failed = Vec::new();
    for (name, bytes) in FIXTURES {
        let s = parse(bytes, &Limits::default()).unwrap_or_else(|e| panic!("{name}: {e}"));
        let got = dump(&s);
        let file = golden_dir().join(format!("{name}.txt"));
        if bless {
            std::fs::create_dir_all(golden_dir()).unwrap();
            std::fs::write(&file, &got).unwrap();
            continue;
        }
        let want = std::fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("{}: {e} (bless with SPECULAR_BLESS=1)", file.display()));
        if got != want {
            eprintln!("--- {name}\n{got}");
            failed.push(*name);
        }
    }
    assert!(failed.is_empty(), "goldens differ: {failed:?}");
}

#[test]
fn probes() {
    for (name, bytes) in FIXTURES {
        assert!(is_hpgl(&bytes[..bytes.len().min(8192)]), "{name}");
    }
}

fn scene(b: &[u8]) -> VectorScene {
    parse(b, &Limits::default()).unwrap()
}

#[test]
fn true_millimetres() {
    // 40 plu per mm: a 4000 plu line is 100 mm.
    let s = scene(b"IN;SP1;PU0,0;PD4000,0;");
    assert_eq!(s.units.code, 4);
    let e = extents(&s).unwrap();
    assert!((e.width() - 100.0).abs() < 1e-9, "{e:?}");
    // User units: SC 0..10 over P1/P2 0..4000 plu, so 1 user unit = 10 mm.
    let s = scene(b"IN;IP0,0,4000,4000;SC0,10,0,10;SP1;PU0,0;PD5,0;");
    let e = extents(&s).unwrap();
    assert!((e.width() - 50.0).abs() < 1e-9, "{e:?}");
    // RO 90 turns the picture a quarter on the paper.
    let s = scene(b"IN;RO90;SP1;PU0,0;PD4000,0;");
    let e = extents(&s).unwrap();
    assert!(
        e.width().abs() < 1e-9 && (e.height() - 100.0).abs() < 1e-9,
        "{e:?}"
    );
}

#[test]
fn pens_become_layers() {
    let s = scene(b"IN;SP3;PD100,0;SP1;PU;PA0,100;PD100,100;SP3;PU0,200;PD100,200;");
    let names: Vec<&str> = s.layers.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["Pen 1", "Pen 3"]);
    // The names are made up, so they are shown translated.
    let generated: Vec<_> = s.layers.iter().map(|l| l.generated).collect();
    assert_eq!(
        generated,
        [
            Some(specular_vector::Generated::Pen(1)),
            Some(specular_vector::Generated::Pen(3))
        ]
    );
    assert_eq!(s.sheets[0].name, "Page 1");
    assert_eq!(
        s.sheets[0].generated,
        Some(specular_vector::Generated::Page(1))
    );
    assert_eq!(s.layers[1].items, 2);
    assert_eq!(s.plot.as_ref().unwrap().pens, [1, 3]);
}

#[test]
fn not_hpgl() {
    assert_eq!(
        parse(b"set terminal png\nplot sin(x)\n", &Limits::default()).err(),
        Some(CadError::NotHpgl)
    );
    assert!(parse(b"", &Limits::default()).is_err());
    assert!(parse(b"\x00\x01\x02", &Limits::default()).is_err());
}

#[test]
fn truncation_never_panics() {
    for (_, bytes) in FIXTURES {
        for n in 0..bytes.len() {
            let _ = parse(&bytes[..n], &Limits::default());
            let _ = is_hpgl(&bytes[..n]);
        }
    }
}

#[test]
fn garbage_never_panics() {
    // A fixed xorshift stream: random bytes, and random bytes spliced into plots.
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    for round in 0..300 {
        let mut b: Vec<u8> = (0..(next() % 400)).map(|_| next() as u8).collect();
        if round % 2 == 0 {
            let (_, f) = FIXTURES[round / 2 % FIXTURES.len()];
            let at = (next() as usize) % (f.len() + 1);
            let mut g = f.to_vec();
            g.splice(at..at, b.iter().copied());
            b = g;
        }
        let _ = parse(&b, &Limits::default());
        let _ = is_hpgl(&b);
    }
    for b in [
        &b"IN;SC0,0,0,0;PD1,1;CI99999999999999999999;AA0,0,1e9;"[..],
        b"IN;PD99999999999999999999,-99999999999999999999;CI1;WG1,0,99999;RA9e9,9e9;",
        b"IN;SI0,0;LBx\x03SR0,0;LBx\x03DI0,0;LBy\x03LO99;LBz\x03FT3,0.0000001;RA10,10;",
        b"IN;PM0;PM0;PM1;PM2;PM2;FP;EP;UL;UL1;LT1,0;LT-3;LT1,-5,1;PW-1;NP0;PC-1,999,9,9;",
        b"PE",
        b"PE7",
        b"PE:",
        b"\x1b%-12345X",
        b"\x1b*b99999999999W",
        b"\x1b%",
    ] {
        let _ = parse(b, &Limits::default());
    }
}

#[test]
fn limits_hold() {
    // A million-point polyline is split at max_points, and items stop at max_items.
    let mut b = b"IN;SP1;PU0,0;PD".to_vec();
    for i in 0..100_000 {
        write!(b, "{},{},", i % 1000, i / 1000).unwrap();
    }
    b.extend_from_slice(b"0,0;");
    let lim = Limits {
        max_points: 1000,
        ..Limits::default()
    };
    let s = parse(&b, &lim).unwrap();
    let items = s.sheets[0].items.len();
    assert!((100..=101).contains(&items), "{items}");
    let lim = Limits {
        max_items: 10,
        ..Limits::default()
    };
    let mut b = b"IN;SP1;".to_vec();
    for _ in 0..100 {
        b.extend_from_slice(b"CI10;");
    }
    let s = parse(&b, &lim).unwrap();
    assert_eq!(s.item_count(), 10);
    assert!(s.warnings.iter().any(|w| w.contains("cut short")));
}

trait WriteBytes {
    fn write_fmt(&mut self, a: std::fmt::Arguments<'_>) -> std::fmt::Result;
}

impl WriteBytes for Vec<u8> {
    fn write_fmt(&mut self, a: std::fmt::Arguments<'_>) -> std::fmt::Result {
        self.extend_from_slice(a.to_string().as_bytes());
        Ok(())
    }
}

/// The corpus (`tools/corpus/fetch-hpgl.sh`) against ezdxf's HP-GL/2 reader
/// (`gen/hpgl_oracle.py`, its output in tests/data/hpgl_oracle.tsv): every file opens and
/// is probed as a plot, and where the oracle draws what we draw (no labels, rotation,
/// windows or symbols, which it skips), the bounds of the strokes and fills agree.
///
/// `cargo test -p specular-cad --release --test hpgl -- --ignored --nocapture`
#[test]
#[ignore]
fn corpus_matches_oracle() {
    let root = std::env::var_os("SPECULAR_HPGL_CORPUS").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/hpgl"),
        PathBuf::from,
    );
    if !root.is_dir() {
        eprintln!(
            "no corpus at {}; run tools/corpus/fetch-hpgl.sh",
            root.display()
        );
        return;
    }
    let tsv = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/hpgl_oracle.tsv"),
    )
    .unwrap();
    // Commands the oracle does not draw that change what is drawn.
    const SKIPPED: &[&str] = &[
        "RO", "IW", "LB", "SM", "XT", "YT", "RA", "RR", "EA", "ER", "EW", "WG", "CA", "CP", "DT",
        "PB", "DF",
    ];
    let (mut compared, mut agreed) = (0, 0);
    let mut failures = Vec::new();
    // ORACLE lines start their own line (the harness's "test ... " has none).
    println!();
    for line in tsv.lines().skip(1) {
        let f: Vec<&str> = line.split('\t').collect();
        let (name, strokes, polygons) = (f[0], f[2], f[4]);
        let bytes = std::fs::read(root.join(name)).unwrap();
        let before = failures.len();
        let head = &bytes[..bytes.len().min(specular_cad::hpgl::PROBE_LEN)];
        let t = std::time::Instant::now();
        let s = parse(&bytes, &Limits::default());
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let Ok(s) = s else {
            failures.push(format!("{name}: {s:?}"));
            println!("ORACLE\tHPGL\t{name}\tfail");
            continue;
        };
        let probed = is_hpgl(head);
        // Bounds of strokes and fills only (the oracle draws no text).
        let mut b: Option<Rect> = None;
        let mut n_strokes = 0;
        for sh in &s.sheets {
            for it in &sh.items {
                let r = match &it.shape {
                    Shape::Stroke { path, .. } => {
                        n_strokes += 1;
                        path.bounding_box()
                    }
                    Shape::Fill { path, .. } => path.bounding_box(),
                    _ => continue,
                };
                b = Some(b.map_or(r, |b| b.union(r)));
            }
        }
        let ignored: Vec<&str> = f[9].split(',').collect();
        let notes = f[10];
        let meaningful = f[5] != "-"
            && !notes.contains("scaled-arc")
            && !notes.contains("sc-without-ip")
            && !ignored.iter().any(|c| SKIPPED.contains(c));
        let mut verdict = String::from("-");
        if meaningful {
            compared += 1;
            let o: Vec<f64> = f[5..9]
                .iter()
                .map(|v| v.parse::<f64>().unwrap() / 40.0)
                .collect();
            let o = Rect::new(o[0], o[1], o[2], o[3]);
            let tol = 0.25_f64.max(0.005 * o.width().max(o.height()));
            let ok = b.is_some_and(|b| {
                (b.x0 - o.x0).abs() <= tol
                    && (b.y0 - o.y0).abs() <= tol
                    && (b.x1 - o.x1).abs() <= tol
                    && (b.y1 - o.y1).abs() <= tol
            });
            if ok {
                agreed += 1;
                verdict = "agrees".into();
            } else {
                verdict = format!("DIFFERS (oracle {o:?})");
                failures.push(format!("{name}: ours {b:?}, oracle {o:?}"));
            }
        }
        if !probed {
            failures.push(format!("{name}: not probed as a plot"));
        }
        println!(
            "{name}\t{ms:.1} ms\tpages {}\tstrokes {n_strokes} (oracle {strokes}, polygons {polygons})\t{verdict}",
            s.sheets.len()
        );
        // One line per file for tools/ci/oracle-pass-rates.sh: extents compared, or only
        // parsed and probed (skip).
        let status = match (failures.len() > before, meaningful) {
            (true, _) => "fail",
            (false, true) => "pass",
            (false, false) => "skip",
        };
        println!("ORACLE\tHPGL\t{name}\t{status}");
    }
    println!("extents compared {compared}, agreed {agreed}");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn hatch_fills_share_one_budget() {
    // Forty 10000-plu squares hatched 0.05 apart: about 200 000 lines each, 16 million
    // path elements before; the file's hatches share one budget, then fill solid.
    let elements = |s: &VectorScene| -> usize {
        s.sheets
            .iter()
            .flat_map(|sh| &sh.items)
            .map(|it| match &it.shape {
                Shape::Stroke { path, .. } | Shape::Fill { path, .. } => path.elements().len(),
                _ => 1,
            })
            .sum()
    };
    let one = elements(&scene(b"IN;SP1;PA0,0;FT3,0.05,0;RA10000,10000;"));
    assert!(one > 300_000, "{one}");
    let mut b = b"IN;SP1;PA0,0;FT3,0.05,0;".to_vec();
    for _ in 0..40 {
        b.extend_from_slice(b"RA10000,10000;");
    }
    let t = std::time::Instant::now();
    let n = elements(&scene(&b));
    assert!(n <= 8_400_000, "{n}");
    assert!(t.elapsed().as_secs() < 60);
}
