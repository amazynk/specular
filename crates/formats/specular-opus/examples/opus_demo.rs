//! Decodes an opus_demo `.bit` file (u32 BE length, u32 BE final range, packet) to 16-bit
//! stereo 48 kHz raw PCM and reports the speed.
//!
//! Usage: opus_demo <in.bit> <out.raw> [--no-phase-inversion]

use specular_opus::{Decoder, Head};
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: opus_demo <in.bit> <out.raw> [--no-phase-inversion]");
        std::process::exit(2);
    }
    let data = std::fs::read(&args[1]).expect("read input");
    let head = Head::parse(&[
        b'O', b'p', b'u', b's', b'H', b'e', b'a', b'd', 1, 2, 0, 0, 0x80, 0xbb, 0, 0, 0, 0, 0,
    ])
    .expect("head");
    let mut dec = Decoder::new(&head).expect("decoder");
    if args.iter().any(|a| a == "--no-phase-inversion") {
        dec.set_phase_inversion(false);
    }
    let mut pcm = Vec::new();
    let mut p = 0;
    let mut mismatches = 0;
    let t = Instant::now();
    while p + 8 <= data.len() {
        let len = u32::from_be_bytes(data[p..p + 4].try_into().unwrap()) as usize;
        let rng = u32::from_be_bytes(data[p + 4..p + 8].try_into().unwrap());
        p += 8;
        let pkt = &data[p..p + len];
        p += len;
        if dec.decode(pkt, &mut pcm).is_err() || (len > 0 && dec.final_range() != rng) {
            mismatches += 1;
        }
    }
    let secs = t.elapsed().as_secs_f64();
    let audio = pcm.len() as f64 / 2.0 / 48000.0;
    eprintln!(
        "{audio:.1} s of audio in {secs:.3} s ({:.0}x real time), {mismatches} range mismatches",
        audio / secs
    );
    let bytes: Vec<u8> = pcm
        .iter()
        .flat_map(|&v| ((v * 32768.0).round().clamp(-32768.0, 32767.0) as i16).to_le_bytes())
        .collect();
    std::fs::write(&args[2], bytes).expect("write output");
}
