//! Fuzz the QCELP-13 decoder: packets (rate octet + frame) as a QCP file stores them, bare
//! frames by length, truncated and padded packets, erasures and blanks between good frames,
//! and a reset part-way. The first byte picks how the rest is cut. Run from this directory
//! with `cargo +nightly fuzz run decode corpus/decode`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_qcelp::{packet_len, Decoder, Rate, FRAME_SAMPLES};

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let mut dec = Decoder::new();
    let mut out = Vec::new();
    let mut packets = 0;
    let mut at = 0;
    while at < rest.len() && packets < 2000 {
        let n = match mode % 4 {
            // As stored: the rate octet gives the length.
            0 => packet_len(rest[at]).unwrap_or(1),
            // Bare frames of the bytes' length, or odd lengths.
            1 => [34, 16, 7, 3, 35, 17][usize::from(rest[at]) % 6],
            2 => usize::from(rest[at] % 40) + 1,
            _ => 35,
        };
        let p = &rest[at..(at + n).min(rest.len())];
        if mode & 4 != 0 && packets % 7 == 3 {
            let (r, f) = specular_qcelp::split_packet(p);
            dec.decode_frame(r, f, &mut out);
        } else if mode & 8 != 0 && packets % 5 == 2 {
            dec.decode_frame(Rate::Erasure, &[], &mut out);
        } else {
            dec.decode(p, &mut out);
        }
        if mode & 16 != 0 && packets == 40 {
            dec.reset();
        }
        at += n;
        packets += 1;
    }
    assert_eq!(out.len(), packets * FRAME_SAMPLES);
    for &s in &out {
        assert!(s.is_finite(), "non-finite sample");
        assert!(s.abs() <= 1.0, "sample {s} out of range");
    }
});
