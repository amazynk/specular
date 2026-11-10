//! Decode page 0 of each TIFF (wide) and write the output raster for an external oracle:
//! `<out>/<name>.raw` (rows packed, no padding) and one line per file on stdout:
//! `<name>\t<width>\t<height>\t<format>\t<truncated>\t<milliseconds>` (or `<name>\tERR\t<why>`).
//! `cargo run --release -p specular-tiff --example dump -- <out-dir> <file.tif>...`

use specular_tiff::{DecodeOptions, Tiff};
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().expect("output directory");
    for path in args {
        let name = std::path::Path::new(&path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let data = std::fs::read(&path).expect("read");
        let t0 = Instant::now();
        let r = Tiff::open(&data).and_then(|t| {
            t.pages()[0].check_supported()?;
            t.decode(
                &data,
                0,
                &DecodeOptions {
                    wide: true,
                    ..Default::default()
                },
            )
        });
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        match r {
            Ok(d) => {
                let row = d.width as usize * d.format.bytes_per_pixel();
                let mut raw = Vec::with_capacity(row * d.height as usize);
                for y in 0..d.height as usize {
                    raw.extend_from_slice(&d.data[y * d.stride..y * d.stride + row]);
                }
                std::fs::write(format!("{out}/{name}.raw"), raw).expect("write");
                println!(
                    "{name}\t{}\t{}\t{:?}\t{}\t{ms:.2}",
                    d.width, d.height, d.format, d.truncated
                );
            }
            Err(e) => println!("{name}\tERR\t{e}"),
        }
    }
}
