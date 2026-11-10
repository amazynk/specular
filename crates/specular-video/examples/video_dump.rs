//! Opens a movie as the viewer does and decodes every frame (woven, as decoded), reporting
//! the facts and the decode rate; with an output path, writes the frames' planes there
//! (ffmpeg's rawvideo layout): `cargo run --example video_dump -- MOVIE [OUT.yuv]`.

#![allow(clippy::disallowed_methods)]

use std::io::Write;
use std::sync::Arc;

use specular_core::{CancelToken, Source};
use specular_decode::{Format, Frames, Session};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = std::path::Path::new(&args[1]);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let format = match ext.as_str() {
        "ts" | "mts" | "m2ts" | "m2t" | "tod" => Format::MpegTs,
        "mpg" | "mpeg" | "vob" | "mod" | "vro" | "m2p" => Format::MpegPs,
        "m1v" | "m2v" | "mpv" => Format::MpegVideo,
        "avi" | "divx" => Format::Avi,
        "ogv" | "ogg" => Format::Ogg,
        "ivf" => Format::Ivf,
        "mxf" => Format::Mxf,
        "mkv" => Format::Matroska,
        "webm" => Format::WebM,
        "mov" => Format::QuickTime,
        "3gp" | "3g2" => Format::ThreeGpp,
        _ => Format::Mp4,
    };
    let t = std::time::Instant::now();
    let src = Arc::new(Source::open(path).expect("open"));
    let s = specular_video::session::open(src, format).expect("movie");
    eprintln!(
        "opened in {:.1} ms: {:?}",
        t.elapsed().as_secs_f64() * 1e3,
        s.video()
    );
    let mut out = args
        .get(2)
        .filter(|p| p.as_str() != "-")
        .map(|p| std::io::BufWriter::new(std::fs::File::create(p).expect("output")));
    let mut frames = s.video_frames().expect("frames");
    frames.set_deinterlace(args.get(3).is_some_and(|a| a == "deint"));
    let cancel = CancelToken::new();
    let t = std::time::Instant::now();
    let mut n = 0;
    while let Some(f) = frames.next_frame(&cancel) {
        let f = f.expect("frame");
        if let Some(o) = out.as_mut() {
            for p in &f.raster.planes {
                let bps = p.stride / p.width as usize;
                for row in p.data.chunks(p.stride).take(p.height as usize) {
                    o.write_all(&row[..p.width as usize * bps]).expect("write");
                }
            }
        }
        n += 1;
    }
    let el = t.elapsed().as_secs_f64();
    eprintln!(
        "{n} frames in {el:.2} s: {:.1} fps ({})",
        f64::from(n) / el,
        frames.backend()
    );
}
