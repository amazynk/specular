//! Fuzz the HEVC sequence decoder (inter prediction, the DPB, RPS and
//! output bumping). The input is an Annex B byte stream: parameter sets
//! before the first slice go to `VideoDecoder::new`, and every slice NAL
//! unit with `first_slice_segment_in_pic_flag` starts a new access unit.
//! Run from this directory with
//! `cargo +nightly fuzz run decode_video <corpus dir> -- -max_total_time=900`
//! (seed it with short clips turned into Annex B, e.g.
//! `ffmpeg -i clip.mp4 -c copy -bsf:v hevc_mp4toannexb -f hevc seed.hevc`).
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_heic::hevc;

/// NAL units of an Annex B stream, without their start codes.
fn nal_units(data: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    let mut out = Vec::new();
    for (k, &s) in starts.iter().enumerate() {
        let mut e = starts.get(k + 1).map_or(data.len(), |&n| n - 3);
        // Trailing zero bytes belong to the next start code.
        while e > s && data[e - 1] == 0 {
            e -= 1;
        }
        if e > s + 2 {
            out.push(&data[s..e]);
        }
    }
    out
}

fn nal_type(n: &[u8]) -> u8 {
    (n[0] >> 1) & 0x3f
}

fuzz_target!(|data: &[u8]| {
    let nals = nal_units(data);
    let first_vcl = nals
        .iter()
        .position(|n| nal_type(n) < 32)
        .unwrap_or(nals.len());
    let (sets, rest) = nals.split_at(first_vcl);
    let Ok(info) = hevc::probe(sets) else {
        return;
    };
    // Keep each run cheap: large pictures decode the same way, slower.
    if info.width as u64 * info.height as u64 > 1 << 18 {
        return;
    }
    let threads = 1 + data.len() % 3;
    let Ok(mut dec) = hevc::VideoDecoder::new(sets, threads) else {
        return;
    };
    let mut au: Vec<&[u8]> = Vec::new();
    let mut frames = 0usize;
    let mut take = |dec: &mut hevc::VideoDecoder| {
        while let Some(d) = dec.next_output() {
            d.frame
                .validate()
                .expect("a decoded frame fits its geometry");
            assert!(d.damaged <= d.total);
            frames += 1;
        }
    };
    for &n in rest {
        let starts_picture = nal_type(n) < 32 && n.get(2).is_some_and(|b| b & 0x80 != 0);
        if starts_picture && !au.is_empty() {
            let _ = dec.decode(&au, None);
            take(&mut dec);
            au.clear();
        }
        au.push(n);
    }
    if !au.is_empty() {
        let _ = dec.decode(&au, None);
    }
    dec.flush();
    take(&mut dec);
    let _ = frames;
});
