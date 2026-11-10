//! Renders CGM pictures to PPM for oracle comparison and prints timings.
//!
//! `cgm-render OUT_DIR WIDTH HEIGHT FILE...`: every picture of each file fitted into
//! WIDTH×HEIGHT pixels at the bottom left, aspect kept, on white (as the WebCGM suites'
//! reference screenshots show it), written as
//! `OUT_DIR/<file stem>_p<picture>.ppm` (1-based), with the time to open the file and to
//! render each picture. `--tile` also times a 4096² region of the first picture at 16× the
//! fitted scale.

use std::io::Write;
use std::path::Path;
use std::time::Instant;

use specular_cgm::{Document, RenderRequest};

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().expect("OUT_DIR");
    let w: u32 = args.next().and_then(|s| s.parse().ok()).expect("WIDTH");
    let h: u32 = args.next().and_then(|s| s.parse().ok()).expect("HEIGHT");
    let mut tile = false;
    let mut files = Vec::new();
    for a in args {
        if a == "--tile" {
            tile = true;
        } else {
            files.push(a);
        }
    }
    std::fs::create_dir_all(&out).unwrap();
    for f in files {
        let path = Path::new(&f);
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        let data = std::fs::read(path).unwrap();
        let t0 = Instant::now();
        let doc = match Document::open(&data) {
            Ok(d) => d,
            Err(e) => {
                println!("{stem}\topen\terror\t{e}");
                continue;
            }
        };
        let open_ms = t0.elapsed().as_secs_f64() * 1e3;
        let skipped: Vec<String> = doc
            .facts()
            .skipped
            .iter()
            .map(|(k, n)| format!("{k} ×{n}"))
            .collect();
        println!(
            "{stem}\topen\t{open_ms:.2} ms\t{} pictures\tskipped [{}]",
            doc.page_count(),
            skipped.join(", ")
        );
        for p in 0..doc.page_count() {
            let (pw, ph) = doc.page_size(p).unwrap();
            let k = (f64::from(w) / pw).min(f64::from(h) / ph);
            let (rw, rh) = (
                ((pw * k).round() as u32).clamp(1, w),
                ((ph * k).round() as u32).clamp(1, h),
            );
            let t = Instant::now();
            let px = match doc.render(
                RenderRequest {
                    page: p,
                    scale_x: k,
                    scale_y: k,
                    x: 0,
                    y: 0,
                    width: rw,
                    height: rh,
                },
                &|| false,
            ) {
                Ok(px) => px,
                Err(e) => {
                    println!("{stem}\tp{}\terror\t{e}", p + 1);
                    continue;
                }
            };
            println!(
                "{stem}\tp{}\t{:.2} ms\t{} ops",
                p + 1,
                t.elapsed().as_secs_f64() * 1e3,
                doc.op_count(p)
            );
            let mut o = std::io::BufWriter::new(
                std::fs::File::create(Path::new(&out).join(format!("{stem}_p{}.ppm", p + 1)))
                    .unwrap(),
            );
            write!(o, "P6\n{w} {h}\n255\n").unwrap();
            let mut rgb = vec![255u8; (w * h * 3) as usize];
            for y in 0..rh as usize {
                for x in 0..rw as usize {
                    let s = (y * rw as usize + x) * 4;
                    let d = ((y + (h - rh) as usize) * w as usize + x) * 3;
                    rgb[d..d + 3].copy_from_slice(&px[s..s + 3]);
                }
            }
            o.write_all(&rgb).unwrap();
        }
        if tile {
            let (pw, ph) = doc.page_size(0).unwrap();
            let k = 16.0 * f64::from(w) / pw.max(ph);
            let (sw, sh) = ((pw * k) as u32, (ph * k) as u32);
            let (tw, th) = (sw.min(4096), sh.min(4096));
            let t = Instant::now();
            let r = doc.render(
                RenderRequest {
                    page: 0,
                    scale_x: k,
                    scale_y: k,
                    x: (sw - tw) / 2,
                    y: (sh - th) / 2,
                    width: tw,
                    height: th,
                },
                &|| false,
            );
            println!(
                "{stem}\ttile\t{:.1} ms\t{tw}x{th} of {sw}x{sh}\t{}",
                t.elapsed().as_secs_f64() * 1e3,
                if r.is_ok() { "ok" } else { "error" }
            );
        }
    }
}
