//! Renders each page of WPG files to binary PPM, for oracle comparisons.
//!
//! Usage: `wpg-render DPI OUT_DIR FILE...` → `OUT_DIR/<stem>_p<N>.ppm`, and one line per file
//! with the description and the render time.

use std::io::Write;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: wpg-render DPI OUT_DIR FILE...");
        std::process::exit(2);
    }
    let dpi: f64 = args[0].parse().expect("DPI");
    let out = std::path::Path::new(&args[1]);
    for f in &args[2..] {
        let path = std::path::Path::new(f);
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("out");
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(e) => {
                println!("{stem}\tread error: {e}");
                continue;
            }
        };
        let t0 = Instant::now();
        let doc = match specular_wpg::open(&data) {
            Ok(d) => d,
            Err(e) => {
                println!("{stem}\trefused: {e}");
                continue;
            }
        };
        let t_open = t0.elapsed();
        for (i, page) in doc.pages.iter().enumerate() {
            let k = dpi / 72.0;
            let w = (page.size.0 * k).round().max(1.0) as u32;
            let h = (page.size.1 * k).round().max(1.0) as u32;
            let t1 = Instant::now();
            let req = specular_drawpage::RenderRequest {
                scale_x: f64::from(w) / page.size.0,
                scale_y: f64::from(h) / page.size.1,
                x: 0,
                y: 0,
                width: w,
                height: h,
            };
            let px = match specular_drawpage::render(page, req, &|| false) {
                Ok(p) => p,
                Err(e) => {
                    println!("{stem}\trender error: {e}");
                    continue;
                }
            };
            let name = out.join(format!("{stem}_p{}.ppm", i + 1));
            let mut file = std::fs::File::create(&name).expect("create");
            write!(file, "P6\n{w} {h}\n255\n").expect("write");
            let rgb: Vec<u8> = px
                .chunks_exact(4)
                .flat_map(|p| [p[0], p[1], p[2]])
                .collect();
            file.write_all(&rgb).expect("write");
            println!(
                "{stem}\t{}\t{w}x{h}\topen {:.1} ms\trender {:.1} ms",
                doc.describe(i),
                t_open.as_secs_f64() * 1e3,
                t1.elapsed().as_secs_f64() * 1e3
            );
        }
    }
}
