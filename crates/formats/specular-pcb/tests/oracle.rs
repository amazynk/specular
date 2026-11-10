//! specular-pcb against gerbonara (tests/oracle.py): for every Gerber file the bounds and the
//! image (dark minus clear) sampled on the oracle's pixel grid; for every drill file the
//! holes, slots and tool diameters. Ignored by default:
//!
//! ```sh
//! venv/bin/python -I crates/formats/specular-pcb/tests/oracle.py CORPUS EXPECT
//! SPECULAR_PCB_CORPUS=CORPUS SPECULAR_PCB_EXPECT=EXPECT \
//!     cargo test -p specular-pcb --test oracle -- --ignored --nocapture
//! ```
//!
//! A Gerber image matches when fewer than 0.5 % of the pixels set in either image (and at
//! most 4 more) have no pixel the other image touches within two pixels (one pixel is
//! reported too): a line thinner than a pixel can miss every pixel centre in one reading and
//! half cover one in the other. Bounds match within 1 µm (two raster pixels for pygerber).
//! The test fails on any mismatch except the files in `ORACLE_WRONG`, each with the evidence
//! that the oracle is wrong there.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;
use specular_pcb::{Limits, excellon, gerber, stack};
use specular_vector::draw::View;
use specular_vector::fonts::Fonts;
use specular_vector::kurbo::Affine;
use specular_vector::prep::{PrepLimits, prepare};
use specular_vector::{Color, Shape, raster};

fn read_pbm(p: &Path) -> Option<(usize, usize, Vec<bool>)> {
    let b = std::fs::read(p).ok()?;
    let text_end = {
        // "P4\nW H\n"
        let mut nl = 0;
        let mut i = 0;
        while i < b.len() && nl < 2 {
            if b[i] == b'\n' {
                nl += 1;
            }
            i += 1;
        }
        i
    };
    let head = std::str::from_utf8(&b[..text_end]).ok()?;
    let mut it = head.split_whitespace().skip(1);
    let w: usize = it.next()?.parse().ok()?;
    let h: usize = it.next()?.parse().ok()?;
    let row = w.div_ceil(8);
    let data = &b[text_end..];
    let mut out = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            out[y * w + x] = data
                .get(y * row + x / 8)
                .is_some_and(|v| v & (0x80 >> (x % 8)) != 0);
        }
    }
    Some((w, h, out))
}

/// Writes a mask as a binary PBM (for looking at mismatches: `SPECULAR_PCB_DUMP=DIR`).
fn write_pbm(p: &Path, m: &[bool], w: usize, h: usize) {
    let row = w.div_ceil(8);
    let mut out = format!("P4\n{w} {h}\n").into_bytes();
    let mut bits = vec![0u8; row * h];
    for y in 0..h {
        for x in 0..w {
            if m[y * w + x] {
                bits[y * row + x / 8] |= 0x80 >> (x % 8);
            }
        }
    }
    out.extend(bits);
    let _ = std::fs::write(p, out);
}

/// Our image of `bytes` on the grid `[ox, oy_top, px, w, h]`.
/// Our image on the oracle's grid: (pixels whose centre is at least half covered, pixels
/// whose centre has any ink at all).
fn our_mask(name: &str, bytes: &[u8], grid: &[f64]) -> Result<(Vec<bool>, Vec<bool>), String> {
    let mut scene =
        stack::single_scene(name, bytes, &Limits::default()).map_err(|e| e.to_string())?;
    for s in &mut scene.sheets {
        s.items.retain(|i| !matches!(i.shape, Shape::Stroke { .. }));
        for i in &mut s.items {
            if i.color != Color::Clear {
                i.color = Color::Rgb(0, 0, 0);
            }
        }
    }
    let (ox, oy, px) = (grid[0], grid[1], grid[2]);
    let (w, h) = (grid[3] as u32, grid[4] as u32);
    let prepared = prepare(
        Arc::new(scene),
        &Fonts::bundled_only(),
        PrepLimits::default(),
    );
    let sheet = &prepared.sheets[0];
    // Three samples per pixel each way; the middle one stands for the pixel's centre, as
    // the oracle samples (so lines thinner than a pixel are seen alike).
    const K: f64 = 3.0;
    let view = View {
        transform: Affine::new([K / px, 0.0, 0.0, -K / px, -K * ox / px, K * oy / px]),
        size: (w * 3, h * 3),
        dark: false,
        background: [255, 255, 255],
        lineweights: false,
        dpi_scale: 1.0,
        layers: &[],
    };
    let (rgba, _) = raster::render(sheet, &view).ok_or("render failed")?;
    let (w, h) = (w as usize, h as usize);
    let mut out = vec![false; w * h];
    let mut any = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = ((3 * y + 1) * 3 * w + 3 * x + 1) * 4;
            out[y * w + x] = rgba[i] < 128;
            any[y * w + x] = rgba[i] < 255;
        }
    }
    Ok((out, any))
}

/// Pixels set in one image with no pixel within `r` pixels that the other image touches at
/// all, and pixels set in either. `a_any` and `b_any` are `a` and `b` with every pixel that
/// has any ink: a line thinner than a pixel can miss every pixel centre in one reading and
/// cover half of one in the other.
fn compare(
    (a, a_any): (&[bool], &[bool]),
    (b, b_any): (&[bool], &[bool]),
    w: usize,
    h: usize,
    r: i64,
) -> (usize, usize) {
    let near = |m: &[bool], x: usize, y: usize| {
        for dy in -r..=r {
            for dx in -r..=r {
                let (xx, yy) = (x as i64 + dx, y as i64 + dy);
                if xx >= 0
                    && yy >= 0
                    && (xx as usize) < w
                    && (yy as usize) < h
                    && m[yy as usize * w + xx as usize]
                {
                    return true;
                }
            }
        }
        false
    };
    let mut bad = 0;
    let mut union = 0;
    for y in 0..h {
        for x in 0..w {
            let (p, q) = (a[y * w + x], b[y * w + x]);
            if p || q {
                union += 1;
            }
            if (p && !near(b_any, x, y)) || (q && !near(a_any, x, y)) {
                bad += 1;
            }
        }
    }
    (bad, union)
}

#[derive(Default)]
struct Tally {
    files: usize,
    sniff: Vec<String>,
    errors: Vec<String>,
    oracle_errors: Vec<String>,
    /// Of the files the oracle could not read: ours read them, or refused them.
    ours_read_oracle_errors: usize,
    ours_refused: Vec<String>,
    bbox: Vec<String>,
    image: Vec<String>,
    image_ok: usize,
    /// Matching within one pixel (not two).
    image_strict: usize,
    drill: Vec<String>,
    drill_ok: usize,
    /// Files in [`ORACLE_WRONG`]: not compared.
    allowed: Vec<String>,
}

/// Files where the oracle is shown wrong (or the file has no defined reading), with the
/// evidence; they are not compared. Everything else must match.
const ORACLE_WRONG: &[(&str, &str)] = &[
    (
        "gerbv/test-image-offset-2.gbx",
        "gerbonara ignores IO (image offset) and reads the file in mm; the file says the \
         crosshairs end centred on 0,0, as ours do",
    ),
    (
        "gerbv/test-image-offset-1.gbx",
        "gerbonara ignores IO (image offset); the file says the crosshairs end centred on \
         0,0, as ours do",
    ),
    (
        "gerbv/test-aperture-circle-1.gbx",
        "undefined: an arc's start and end are 0.005 in off one circle; ours move the centre \
         so the arc ends at its end point, gerbonara keeps the start radius and stops short \
         (its own bounds include the end point)",
    ),
    (
        "gerbv/test-circular-interpolation-1.gbx",
        "undefined: single-quadrant arcs whose start and end are off one circle; ours end \
         at the end point, gerbonara's stop short of it",
    ),
    (
        "gerbv/test-layer-axis-select-1.gbx",
        "gerbonara stores AS (axis select) but never applies it; the file says the line ends \
         along the y axis, as ours does",
    ),
    (
        "gerbv/test-layer-mode-1.gbx",
        "gerbonara loses the unit changes between aperture definitions (its bounds reach \
         3175 mm: 125 mm read as inches); ours end at 125.6 mm as the coordinates say",
    ),
    (
        "gerbv/test-image-polarity-1.gbx",
        "pygerber ignores IPNEG; the file says the crosshairs are cut out of a dark \
         background, as ours are",
    ),
    (
        "gerbv/test-load-mirroring.gbx",
        "pygerber mirrors the coordinates; LM mirrors the aperture about its own origin \
         (Gerber spec 4.9, object transformations), so the L-shapes drawn with a round \
         aperture stay where they are",
    ),
    (
        "gerbv/test-layer-scale-factor-1.gbx",
        "undefined: the deprecated SF scales the coordinates; gerbonara keeps the apertures, \
         gerbv (which wrote the test) and ours scale them too. Both are 2 by 1 inches, as the \
         file asks",
    ),
    (
        "gerbv/test-aperture-rectangle-1.gbx",
        "undefined: arcs drawn with a rectangular aperture (the spec allows only circles); \
         gerbonara draws round caps, ours sweep the rectangle's width",
    ),
    (
        "allegro-2/MinnowMax_RevA1_DRILL/MinnowMax_RevA1_NCROUTE.rou",
        "gerbonara sizes the route bits from ncdrill.log (drill tools 8, 36, 38 mil); \
         ncroute.log beside it lists the bits, 15, 31 and 40 mil, which ours use",
    ),
    (
        "allegro/08_057494d.rou",
        "the route tape defines no tool and no ncroute.log is given; gerbonara takes drill \
         T01 (12 mil) from ncdrill.log, ours leave the bit unsized",
    ),
];

fn run(corpus: &Path, expect: &Path) -> Tally {
    // Starts the `ORACLE` lines on their own line (libtest prints `test NAME ... ` first).
    println!();
    let mut t = Tally::default();
    let lines = std::fs::read_to_string(expect.join("expect.jsonl")).expect("expect.jsonl");
    for line in lines.lines().filter(|l| !l.trim().is_empty()) {
        let rec: Value = serde_json::from_str(line).expect("json");
        let rel = rec["path"].as_str().unwrap().to_string();
        let path: PathBuf = corpus.join(&rel);
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        t.files += 1;
        let head = &bytes[..bytes.len().min(16 * 1024)];
        // One line per file for tools/ci/oracle-pass-rates.sh.
        let fmt = match rec["kind"].as_str() {
            Some("drill") => "Excellon",
            _ => "Gerber",
        };
        let verdict = |status: &str| println!("ORACLE\t{fmt}\t{rel}\t{status}");
        if let Some((_, why)) = ORACLE_WRONG.iter().find(|(f, _)| rel.ends_with(f)) {
            t.allowed.push(format!("{rel}: {why}"));
            verdict("skip");
            continue;
        }
        let problems = |t: &Tally| {
            t.sniff.len() + t.errors.len() + t.bbox.len() + t.image.len() + t.drill.len()
        };
        let before = problems(&t);
        match rec["kind"].as_str().unwrap() {
            "error" => {
                t.oracle_errors
                    .push(format!("{rel}: {}", rec["error"].as_str().unwrap_or("")));
                // Ours on the same file: read, refused (an error), or not ours at all.
                let ours = if gerber::is_gerber(head) {
                    gerber::parse(&bytes, &Limits::default()).map(|_| ())
                } else if excellon::is_excellon(head) {
                    excellon::parse(&bytes, &Limits::default()).map(|_| ())
                } else {
                    Err(specular_pcb::Error::NotThisFormat)
                };
                match ours {
                    Ok(()) => t.ours_read_oracle_errors += 1,
                    Err(e) => t.ours_refused.push(format!("{rel}: {e}")),
                }
                verdict("skip");
                continue;
            }
            "gerber" => {
                if !gerber::is_gerber(head) {
                    t.sniff.push(format!("{rel}: not sniffed as Gerber"));
                }
                let g = match gerber::parse(&bytes, &Limits::default()) {
                    Ok(g) => g,
                    Err(e) => {
                        t.errors.push(format!("{rel}: {e}"));
                        verdict("fail");
                        continue;
                    }
                };
                let want: Vec<f64> = rec["bbox"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_f64().unwrap())
                    .collect();
                match g.bounds {
                    Some(b) => {
                        let d = [
                            b.x0 - want[0],
                            b.y0 - want[1],
                            b.x1 - want[2],
                            b.y1 - want[3],
                        ];
                        // pygerber's bounds are its raster's: two of its pixels.
                        let tol = if rec["oracle"].as_str() == Some("pygerber") {
                            2.0 * rec["grid"][2].as_f64().unwrap_or(0.0)
                        } else {
                            1e-3
                        };
                        if d.iter().any(|v| v.abs() > tol.max(1e-3)) {
                            t.bbox.push(format!(
                                "{rel}: ours [{:.4} {:.4} {:.4} {:.4}] gerbonara [{:.4} {:.4} {:.4} {:.4}]",
                                b.x0, b.y0, b.x1, b.y1, want[0], want[1], want[2], want[3]
                            ));
                        }
                    }
                    None => t.bbox.push(format!("{rel}: no bounds of ours")),
                }
                let grid: Vec<f64> = rec["grid"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_f64().unwrap())
                    .collect();
                let Some((w, h, theirs)) =
                    read_pbm(&expect.join("masks").join(rec["mask"].as_str().unwrap()))
                else {
                    t.image.push(format!("{rel}: no mask"));
                    verdict("fail");
                    continue;
                };
                let theirs_any = rec["mask_any"]
                    .as_str()
                    .and_then(|m| read_pbm(&expect.join("masks").join(m)))
                    .map_or_else(|| theirs.clone(), |(_, _, m)| m);
                match our_mask(&name, &bytes, &grid) {
                    Ok((ours, ours_any)) => {
                        if let Some(dir) = std::env::var_os("SPECULAR_PCB_DUMP") {
                            let m = rec["mask"].as_str().unwrap();
                            write_pbm(&Path::new(&dir).join(m), &ours, w, h);
                            write_pbm(&Path::new(&dir).join(format!("any-{m}")), &ours_any, w, h);
                        }
                        let (bad1, union) =
                            compare((&ours, &ours_any), (&theirs, &theirs_any), w, h, 1);
                        if bad1 as f64 <= 0.005 * union as f64 + 4.0 {
                            t.image_strict += 1;
                        }
                        let (bad, _) = compare((&ours, &ours_any), (&theirs, &theirs_any), w, h, 2);
                        if bad as f64 > 0.005 * union as f64 + 4.0 {
                            t.image.push(format!(
                                "{rel}: {bad} of {union} pixels differ ({:.2} %)",
                                100.0 * bad as f64 / union.max(1) as f64
                            ));
                        } else {
                            t.image_ok += 1;
                        }
                    }
                    Err(e) => t.image.push(format!("{rel}: {e}")),
                }
            }
            "drill" => {
                if !excellon::is_excellon(head) {
                    t.sniff.push(format!("{rel}: not sniffed as Excellon"));
                }
                let mut d = match excellon::parse(&bytes, &Limits::default()) {
                    Ok(d) => d,
                    Err(e) => {
                        t.errors.push(format!("{rel}: {e}"));
                        verdict("fail");
                        continue;
                    }
                };
                // As a board opens: Allegro's NC logs beside the tape size its tools.
                let route = rel.to_ascii_lowercase().ends_with(".rou");
                let log = path.with_file_name(if route { "ncroute.log" } else { "ncdrill.log" });
                if let Ok(text) = std::fs::read(&log) {
                    d.size_undefined_tools(&excellon::allegro_log_tools(
                        &String::from_utf8_lossy(&text),
                        route,
                    ));
                }
                let holes = rec["holes"].as_u64().unwrap() as usize;
                let slots = rec["slots"].as_u64().unwrap() as usize;
                let mut tools: Vec<f64> = rec["tools"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_f64().unwrap())
                    .collect();
                tools.sort_by(f64::total_cmp);
                let mut ours: Vec<f64> = d
                    .holes
                    .iter()
                    .map(|h| h.tool)
                    .chain(d.slots.iter().map(|s| s.tool))
                    .map(|i| (d.tools[i].diameter * 1e4).round() / 1e4)
                    .collect();
                ours.sort_by(f64::total_cmp);
                ours.dedup();
                let same_tools = ours.len() == tools.len()
                    && ours.iter().zip(&tools).all(|(a, b)| (a - b).abs() <= 1e-3);
                if d.holes.len() != holes || d.slots.len() != slots || !same_tools {
                    t.drill.push(format!(
                        "{rel}: ours {} holes {} slots {:?}, gerbonara {holes} holes {slots} slots {tools:?}",
                        d.holes.len(),
                        d.slots.len(),
                        ours
                    ));
                } else {
                    t.drill_ok += 1;
                }
            }
            _ => {
                verdict("skip");
                continue;
            }
        }
        verdict(if problems(&t) > before {
            "fail"
        } else {
            "pass"
        });
    }
    t
}

fn show(what: &str, v: &[String]) {
    println!("{what}: {}", v.len());
    for s in v.iter().take(60) {
        println!("  {s}");
    }
}

#[test]
#[ignore]
fn corpus_matches_gerbonara() {
    let (Some(c), Some(e)) = (
        std::env::var_os("SPECULAR_PCB_CORPUS"),
        std::env::var_os("SPECULAR_PCB_EXPECT"),
    ) else {
        eprintln!("SPECULAR_PCB_CORPUS / SPECULAR_PCB_EXPECT not set");
        return;
    };
    let t0 = std::time::Instant::now();
    let t = run(Path::new(&c), Path::new(&e));
    println!("{} files in {:.1} s", t.files, t0.elapsed().as_secs_f64());
    show("oracle errors (not compared)", &t.oracle_errors);
    println!(
        "  of those, read by Specular: {}",
        t.ours_read_oracle_errors
    );
    show("  refused by Specular too", &t.ours_refused);
    show("sniff mismatches", &t.sniff);
    show("our errors", &t.errors);
    show("bounds mismatches", &t.bbox);
    println!(
        "images matching: {} (within two pixels), {} (within one)",
        t.image_ok, t.image_strict
    );
    show("image mismatches", &t.image);
    println!("drill files matching: {}", t.drill_ok);
    show("drill mismatches", &t.drill);
    show("not compared (oracle wrong, see ORACLE_WRONG)", &t.allowed);
    assert_eq!(
        t.allowed.len(),
        ORACLE_WRONG.len(),
        "an ORACLE_WRONG file is gone"
    );
    let bad = t.sniff.len() + t.errors.len() + t.bbox.len() + t.image.len() + t.drill.len();
    assert_eq!(
        bad, 0,
        "files differ from the oracle beyond tolerance (listed above)"
    );
}

/// Every folder of the corpus that looks like a board opens as a stack: layers found, roles,
/// time (parse and scene) and warnings. `SPECULAR_PCB_CORPUS=DIR cargo test -p specular-pcb
/// --test oracle corpus_boards -- --ignored --nocapture`
#[test]
#[ignore]
fn corpus_boards() {
    let Some(c) = std::env::var_os("SPECULAR_PCB_CORPUS") else {
        eprintln!("SPECULAR_PCB_CORPUS not set");
        return;
    };
    let mut dirs = vec![PathBuf::from(c)];
    let mut boards = 0;
    let mut slowest = (0.0, String::new());
    while let Some(d) = dirs.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        let mut files = Vec::new();
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                dirs.push(p);
            } else if let Ok(b) = std::fs::read(&p) {
                files.push((p.file_name().unwrap().to_string_lossy().to_string(), b));
            }
        }
        let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        if !stack::looks_like_board(&names) {
            continue;
        }
        boards += 1;
        let t0 = std::time::Instant::now();
        let r = stack::board_scene(&files, &Limits::default());
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        match r {
            Ok(s) => {
                let info = s.pcb.as_ref().unwrap();
                let items: usize = s.sheets.iter().map(|s| s.items.len()).sum();
                let size = info
                    .size_mm
                    .map_or("?".into(), |(w, h)| format!("{w:.1}×{h:.1} mm"));
                println!(
                    "{}: {} layers ({} copper), {} holes, {size}, {items} items, {ms:.0} ms, {} warnings",
                    d.display(),
                    info.layers.len(),
                    info.copper_layers,
                    info.holes,
                    s.warnings.len()
                );
                for w in s.warnings.iter().take(3) {
                    println!("    {w}");
                }
                if ms > slowest.0 {
                    slowest = (ms, d.display().to_string());
                }
            }
            Err(e) => println!("{}: ERROR {e}", d.display()),
        }
    }
    println!(
        "{boards} boards; slowest {:.0} ms: {}",
        slowest.0, slowest.1
    );
}
