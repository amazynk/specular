//! Decodes a native .flac file and reports speed as a multiple of real time.
//!
//! `cargo run -p specular-flac --example flac_decode -- FILE [OUT.raw]` (OUT gets s32le PCM).

use std::time::Instant;

use specular_flac::{Decoder, StreamInfo, frames};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(path) = args.get(1) else {
        eprintln!("usage: flac_decode FILE.flac [OUT.raw]");
        std::process::exit(2);
    };
    let file = std::fs::read(path).expect("read file");
    if file.get(..4) != Some(b"fLaC") {
        eprintln!("not a native FLAC file");
        std::process::exit(1);
    }
    // Skip the metadata blocks.
    let mut p = 4;
    loop {
        let Some(h) = file.get(p..p + 4) else {
            eprintln!("metadata cut short");
            std::process::exit(1);
        };
        p += 4 + ((usize::from(h[1]) << 16) | (usize::from(h[2]) << 8) | usize::from(h[3]));
        if h[0] & 0x80 != 0 {
            break;
        }
    }
    let info = StreamInfo::from_config(&file[..p.min(file.len())]).expect("STREAMINFO");
    let body = &file[p.min(file.len())..];
    let list: Vec<&[u8]> = frames(body).collect();

    let mut dec = Decoder::new(Some(info));
    let mut out = Vec::new();
    let mut ints = Vec::new();
    let t = Instant::now();
    let mut samples = 0;
    let mut errors = 0;
    for f in &list {
        out.clear();
        match dec.decode(f, &mut out) {
            Ok(n) => samples += n,
            Err(_) => errors += 1,
        }
    }
    let secs = t.elapsed().as_secs_f64();
    let audio = samples as f64 / f64::from(info.sample_rate.max(1));
    println!(
        "{} Hz, {} ch, {} bits: {} frames, {audio:.2} s of audio in {:.3} s = {:.0}x real time, {errors} bad frames",
        info.sample_rate,
        info.channels,
        info.bits,
        list.len(),
        secs,
        audio / secs
    );
    if let Some(out_path) = args.get(2) {
        let mut d = Decoder::new(Some(info));
        for f in &list {
            let _ = d.decode_i32(f, &mut ints);
        }
        let bytes: Vec<u8> = ints.iter().flat_map(|s| s.to_le_bytes()).collect();
        std::fs::write(out_path, bytes).expect("write output");
    }
}
