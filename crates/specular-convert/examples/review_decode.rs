//! The decode half of the visual review sheets (`tools/ci/review-sheets.py`): files go through
//! the viewer's own path (the registry's session, the source's ICC profile to sRGB, HDR
//! tone-mapped, EXIF orientation applied; movies through specular-video, first frame) and come
//! out as 8-bit PNG.
//!
//! ```text
//! cargo run -p specular-convert --example review_decode -- probe FILE...
//! cargo run -p specular-convert --example review_decode -- decode IN OUT.png LONG_EDGE
//! ```
//!
//! `probe` opens each file (headers only) and prints one line per file; `decode` does the
//! same for one file and writes it, its long side at most `LONG_EDGE`. Lines are tab-separated:
//! `path status format coding bits width height display_w display_h orientation alpha icc
//! pages animated ms` (status `ok`, or `err:` and the reason with the other fields empty).

use std::path::Path;
use std::time::Instant;

use specular_convert::{ColorMode, Depth, Format, HdrMode, MetaMode, Options, Resize};
use specular_core::CancelToken;
use specular_decode::{Registry, Session};

fn facts(s: &dyn Session) -> String {
    let i = s.info();
    let (dw, dh) = i.display_size();
    format!(
        "{}\t{}\t{}\t{}\t{}\t{dw}\t{dh}\t{}\t{}\t{}\t{}\t{}",
        i.format.name(),
        i.coding.replace(['\t', '\n'], " "),
        i.bit_depth,
        i.width,
        i.height,
        i.orientation.0,
        u8::from(i.has_alpha),
        u8::from(i.icc_profile.is_some() || s.embedded_icc().is_some()),
        s.page_count(),
        u8::from(s.animation().is_some()),
    )
}

fn err_line(path: &str, e: impl std::fmt::Display) -> String {
    let e = e.to_string().replace(['\t', '\n'], " ");
    format!("{path}\terr:{e}\t\t\t\t\t\t\t\t\t\t\t\t\t")
}

fn probe(reg: &Registry, path: &str) -> String {
    let t = Instant::now();
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        reg.open_path(Path::new(path)).map(|s| facts(s.as_ref()))
    }));
    match r {
        Ok(Ok(f)) => format!("{path}\tok\t{f}\t{}", t.elapsed().as_millis()),
        Ok(Err(e)) => err_line(path, e),
        Err(_) => err_line(path, "panic"),
    }
}

fn decode(reg: &Registry, path: &str, out: &str, long: u32) -> String {
    let t = Instant::now();
    let s = match reg.open_path(Path::new(path)) {
        Ok(s) => s,
        Err(e) => return err_line(path, e),
    };
    let opts = Options {
        format: Format::Png,
        png_effort: 1,
        depth: Depth::Eight,
        resize: Resize::LongEdge(long),
        upscale: false,
        color: ColorMode::Srgb,
        hdr: HdrMode::ToneMap,
        metadata: MetaMode::Strip,
        ..Options::default()
    };
    match specular_convert::convert_bytes(s.as_ref(), &opts, &CancelToken::new()) {
        Ok(Some(png)) => match std::fs::write(out, png) {
            Ok(()) => format!(
                "{path}\tok\t{}\t{}",
                facts(s.as_ref()),
                t.elapsed().as_millis()
            ),
            Err(e) => err_line(path, e),
        },
        Ok(None) => err_line(path, "cancelled"),
        Err(e) => err_line(path, e),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Movies open as images through specular-video, as in the viewer.
    specular_video::install();
    let reg = Registry::new();
    match args.first().map(String::as_str) {
        Some("probe") => {
            for p in &args[1..] {
                println!("{}", probe(&reg, p));
            }
        }
        Some("decode") if args.len() == 4 => {
            let long = args[3].parse().expect("LONG_EDGE");
            println!("{}", decode(&reg, &args[1], &args[2], long));
        }
        _ => {
            eprintln!("usage: review_decode probe FILE... | decode IN OUT.png LONG_EDGE");
            std::process::exit(2);
        }
    }
}
