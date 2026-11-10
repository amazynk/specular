//! Decodes an ADTS (`.aac`) stream to interleaved f32le PCM and reports the speed.
//!
//! Usage: aac_decode <in.aac> <out.f32> [--downmix] [--asc HEX]
//!
//! The ADTS header's implicit SBR/PS signalling is settled from the first frames. With
//! `--asc`, the configuration comes from that AudioSpecificConfig (hex) instead, for streams
//! with explicit SBR/PS signalling remuxed to ADTS.

use std::time::Instant;

use specular_aac::{Decoder, Options, adts};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: aac_decode <in.aac> <out.f32> [--downmix] [--asc HEX]");
        std::process::exit(2);
    }
    let data = std::fs::read(&args[1]).expect("read input");
    let opts = Options {
        stereo_downmix: args.iter().any(|a| a == "--downmix"),
    };
    let asc: Option<Vec<u8>> = args.iter().position(|a| a == "--asc").map(|i| {
        let h = &args[i + 1];
        (0..h.len() / 2)
            .map(|j| u8::from_str_radix(&h[2 * j..2 * j + 2], 16).expect("hex"))
            .collect()
    });
    let probe: Vec<&[u8]> = adts::frames(&data)
        .take(8)
        .map(|(h, f)| &f[h.payload()])
        .collect();
    let mut dec: Option<Decoder> = None;
    let mut pcm = Vec::new();
    let (mut frames, mut errors) = (0usize, 0usize);
    let t = Instant::now();
    for (h, frame) in adts::frames(&data) {
        let d = match &mut dec {
            Some(d) => d,
            None => {
                let d = match &asc {
                    Some(a) => Decoder::new(a, opts),
                    None => h.config().and_then(|c| {
                        Decoder::with_config(
                            specular_aac::resolve_implicit(&c, probe.iter().copied()),
                            opts,
                        )
                    }),
                };
                match d {
                    Ok(d) => dec.insert(d),
                    Err(e) => {
                        eprintln!("refused: {e}");
                        std::process::exit(1);
                    }
                }
            }
        };
        if h.blocks != 1 {
            eprintln!(
                "frame {frames}: {} raw data blocks (unsupported here)",
                h.blocks
            );
        }
        if let Err(e) = d.decode(&frame[h.payload()], &mut pcm) {
            if errors < 10 {
                eprintln!("frame {frames}: {e}");
            }
            errors += 1;
        }
        frames += 1;
    }
    let secs = t.elapsed().as_secs_f64();
    let Some(d) = dec else {
        eprintln!("no ADTS frames");
        std::process::exit(1);
    };
    let ch = usize::from(d.channels().max(1));
    let audio = pcm.len() as f64 / ch as f64 / f64::from(d.sample_rate());
    eprintln!(
        "{frames} frames, {errors} errors, {ch} ch, {} Hz: {audio:.2} s of audio in {secs:.3} s ({:.0}x real time)",
        d.sample_rate(),
        audio / secs.max(1e-9)
    );
    let bytes: Vec<u8> = pcm.iter().flat_map(|v| v.to_le_bytes()).collect();
    std::fs::write(&args[2], bytes).expect("write output");
}
