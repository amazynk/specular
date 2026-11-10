//! Lays a book out and writes pages as PPM, with timings.
//!
//! Usage: `epub-render [--system-fonts] [--only] [--scale S] BOOK OUT_DIR [PAGE...]` →
//! `OUT_DIR/<stem>-<page>.ppm` (pages from 0; the first three without PAGE). Prints one line:
//! `book pages open_ms paginate_ms render_ms_per_page` (every page is rendered once for the
//! timing; with `--only` just the PAGEs). PNG and JPEG pictures are decoded with the in-tree decoders; others are grey.

use std::io::Write;
use std::sync::Arc;
use std::time::Instant;

use specular_epub::{Book, Image, ImageDecoder, Options, PageSetup};

struct Decoders;

impl ImageDecoder for Decoders {
    fn decode(&self, d: &[u8], _mt: &str, _w: u32, _h: u32) -> Option<Image> {
        if d.starts_with(b"\xff\xd8") {
            let p = specular_jpeg::decode(d, &specular_jpeg::DecodeOptions::default()).ok()?;
            return Some(Image {
                width: p.width,
                height: p.height,
                rgba: specular_jpeg::planes_to_rgba(&p),
            });
        }
        if d.starts_with(b"\x89PNG") {
            let img = specular_png::decode(d, &specular_png::DecodeOptions::default()).ok()?;
            return Some(Image {
                width: img.width,
                height: img.height,
                rgba: img.to_rgba8(),
            });
        }
        None
    }

    fn size(&self, d: &[u8], mt: &str) -> Option<(u32, u32)> {
        if d.starts_with(b"\x89PNG") {
            let i = specular_png::read_info(d).ok()?;
            return Some((i.width, i.height));
        }
        if mt == "image/svg+xml" {
            return Some((600, 900));
        }
        self.decode(d, mt, 0, 0).map(|i| (i.width, i.height))
    }
}

fn write_ppm(path: &std::path::Path, img: &Image) {
    let mut f = std::io::BufWriter::new(std::fs::File::create(path).expect("create"));
    write!(f, "P6\n{} {}\n255\n", img.width, img.height).unwrap();
    for px in img.rgba.chunks_exact(4) {
        f.write_all(&px[..3]).unwrap();
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut setup = PageSetup::default();
    let mut scale = 1.0f32;
    if let Some(i) = args.iter().position(|a| a == "--system-fonts") {
        args.remove(i);
        setup.system_fonts = true;
    }
    let mut all = true;
    if let Some(i) = args.iter().position(|a| a == "--only") {
        args.remove(i);
        all = false;
    }
    if let Some(i) = args.iter().position(|a| a == "--scale") {
        args.remove(i);
        scale = args.remove(i).parse().expect("scale");
    }
    if args.len() < 2 {
        eprintln!(
            "usage: epub-render [--system-fonts] [--only] [--scale S] BOOK OUT_DIR [PAGE...]"
        );
        std::process::exit(2);
    }
    let book_path = std::path::Path::new(&args[0]);
    let out = std::path::Path::new(&args[1]);
    let stem = book_path.file_stem().unwrap().to_string_lossy().to_string();
    let t0 = Instant::now();
    let data = std::fs::read(book_path).expect("read");
    let book = match Book::open(data, &Options::default()) {
        Ok(b) => b,
        Err(e) => {
            println!("{} ERROR {e}", book_path.display());
            return;
        }
    };
    let open_ms = t0.elapsed().as_secs_f64() * 1e3;
    let t1 = Instant::now();
    let pages = match book.paginate(&setup, Arc::new(Decoders), &|| false) {
        Ok(p) => p,
        Err(e) => {
            println!("{} ERROR {e}", book_path.display());
            return;
        }
    };
    let pag_ms = t1.elapsed().as_secs_f64() * 1e3;
    let wanted: Vec<usize> = if args.len() > 2 {
        args[2..].iter().filter_map(|a| a.parse().ok()).collect()
    } else {
        (0..pages.len().min(3)).collect()
    };
    let t2 = Instant::now();
    for i in 0..pages.len() {
        if !all && !wanted.contains(&i) {
            continue;
        }
        let img = pages.render(i, scale, None).expect("render");
        if wanted.contains(&i) {
            write_ppm(&out.join(format!("{stem}-{i}.ppm")), &img);
        }
    }
    let per = t2.elapsed().as_secs_f64() * 1e3 / pages.len().max(1) as f64;
    println!(
        "{} {} {open_ms:.1} {pag_ms:.1} {per:.2}",
        book_path.display(),
        pages.len()
    );
}
