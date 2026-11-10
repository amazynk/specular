//! Renders DjVu pages to PPM for oracle comparison and prints timings.
//!
//! `djvu-render OUT_DIR [--scale N] [--every K] FILE...`: every K-th page of each file
//! (default every page) at 1/N size, written as `OUT_DIR/<file stem>_p<page>.ppm` (1-based),
//! (`--page P` for one page), with the time to open the document, decode and render each page, and the page text
//! length. Also times a 2048² region at full size of the largest page rendered.

use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use specular_djvu::{Bytes, Document, Options, RenderRequest, Resolver};

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().expect("OUT_DIR");
    let mut scale = 1u32;
    let mut every = 1usize;
    let mut only: Option<usize> = None;
    let mut files = Vec::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--scale" => scale = args.next().and_then(|s| s.parse().ok()).expect("--scale N"),
            "--every" => every = args.next().and_then(|s| s.parse().ok()).expect("--every K"),
            "--page" => {
                only = args
                    .next()
                    .and_then(|s| s.parse::<usize>().ok())
                    .map(|p| p - 1)
            }
            _ => files.push(a),
        }
    }
    std::fs::create_dir_all(&out).unwrap();
    for f in files {
        let path = Path::new(&f);
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let resolver: Resolver = Arc::new(move |n: &str| {
            std::fs::read(dir.join(n))
                .ok()
                .map(|v| Arc::new(v) as Bytes)
        });
        let t0 = Instant::now();
        let data: Bytes = Arc::new(std::fs::read(path).unwrap());
        let doc = match Document::open(
            data,
            Options {
                resolver: Some(resolver),
                ..Options::default()
            },
        ) {
            Ok(d) => d,
            Err(e) => {
                println!("{stem}\topen\terror\t{e}");
                continue;
            }
        };
        println!(
            "{stem}\topen\t{:.1} ms\t{} pages",
            t0.elapsed().as_secs_f64() * 1e3,
            doc.page_count()
        );
        let mut largest = (0u64, 0usize);
        for p in (0..doc.page_count()).step_by(every.max(1)) {
            if only.is_some_and(|o| o != p) {
                continue;
            }
            let t = Instant::now();
            let info = match doc.page_info(p) {
                Ok(i) => i,
                Err(e) => {
                    println!("{stem}\tp{}\terror\t{e}", p + 1);
                    continue;
                }
            };
            let (w, h) = info.display_size();
            let (w, h) = (w.div_ceil(scale), h.div_ceil(scale));
            let px = match doc.render(
                RenderRequest {
                    page: p,
                    scale,
                    x: 0,
                    y: 0,
                    width: w,
                    height: h,
                },
                &|| false,
            ) {
                Ok(px) => px,
                Err(e) => {
                    println!("{stem}\tp{}\terror\t{e}", p + 1);
                    continue;
                }
            };
            let ms = t.elapsed().as_secs_f64() * 1e3;
            let text = doc.page_text(p).ok().flatten().map_or(0, |t| t.text.len());
            println!(
                "{stem}\tp{}\t{ms:.1} ms\t{w}x{h}\t{}\ttext {text}",
                p + 1,
                info.layers
            );
            let area = u64::from(info.width) * u64::from(info.height);
            if area > largest.0 {
                largest = (area, p);
            }
            let mut f = std::io::BufWriter::new(
                std::fs::File::create(Path::new(&out).join(format!("{stem}_p{}.ppm", p + 1)))
                    .unwrap(),
            );
            write!(f, "P6\n{w} {h}\n255\n").unwrap();
            let rgb: Vec<u8> = px
                .chunks_exact(4)
                .flat_map(|c| [c[0], c[1], c[2]])
                .collect();
            f.write_all(&rgb).unwrap();
        }
        // A tile at full size from the middle of the largest page (layers cached).
        let p = largest.1;
        if let Ok(info) = doc.page_info(p) {
            let (w, h) = info.display_size();
            let (tw, th) = (w.min(2048), h.min(2048));
            let t = Instant::now();
            let _ = doc.render(
                RenderRequest {
                    page: p,
                    scale: 1,
                    x: (w - tw) / 2,
                    y: (h - th) / 2,
                    width: tw,
                    height: th,
                },
                &|| false,
            );
            println!(
                "{stem}\ttile\t{:.1} ms\t{tw}x{th} of p{}",
                t.elapsed().as_secs_f64() * 1e3,
                p + 1
            );
        }
    }
}
