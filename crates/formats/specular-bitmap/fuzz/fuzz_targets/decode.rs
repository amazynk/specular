//! Fuzz every format of the crate (BMP, ICO/CUR, TGA, QOI, PNM/PAM, GIF, ANI) through
//! `parse` and `Decode::decode` at several scales and regions. Run from this directory with
//! `cargo +nightly fuzz run decode corpus/decode -- -dict=fuzz.dict`, or all targets via
//! `tools/fuzz/run.sh crates/formats/specular-bitmap`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_bitmap::{Decode, Options, Region, ani, bmp, gif, ico, pnm, qoi, tga};

fn exercise(d: &dyn Decode, data: &[u8], parallel: bool) {
    let info = d.info();
    // Keep each run cheap: large images decode the same way, slower.
    if info.width as u64 * info.height as u64 > 1 << 22 {
        return;
    }
    let never = || false;
    for (scale, region) in [
        (1, None),
        (
            3,
            Some(Region {
                x: info.width / 9,
                y: info.height / 7,
                w: info.width / 5 + 1,
                h: info.height / 4 + 1,
            }),
        ),
        (8, None),
    ] {
        let opts = Options {
            scale,
            region,
            parallel,
        };
        if let Ok(img) = d.decode(data, &opts, &never) {
            assert_eq!(
                img.data.len(),
                img.width as usize * img.height as usize * img.format.bytes_per_pixel()
            );
            let (sw, sh) = (info.width.div_ceil(scale), info.height.div_ceil(scale));
            assert!(
                img.width <= sw && img.height <= sh,
                "output larger than the scaled image"
            );
        }
    }
}

fn ico_entry(data: &[u8], parallel: bool) {
    if let Ok(i) = ico::Ico::parse(data) {
        for p in &i.pages {
            match &p.entry {
                ico::Entry::Bmp(b) => exercise(b.as_ref(), data, parallel),
                ico::Entry::Png { range, .. } => {
                    assert!(range.end <= data.len(), "PNG entry range out of bounds")
                }
            }
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let parallel = data.len() % 2 == 1;
    let _ = (
        bmp::probe(data),
        ico::probe(data),
        tga::probe(data),
        qoi::probe(data),
        pnm::probe(data),
        gif::probe(data),
        ani::probe(data),
    );
    if let Ok(b) = bmp::Bmp::parse(data) {
        exercise(&b, data, parallel);
    }
    if let Ok(t) = tga::Tga::parse(data) {
        exercise(&t, data, parallel);
    }
    if let Ok(q) = qoi::Qoi::parse(data) {
        exercise(&q, data, parallel);
    }
    if let Ok(p) = pnm::Pnm::parse(data) {
        exercise(&p, data, parallel);
    }
    if let Ok(g) = gif::Gif::parse(data) {
        exercise(&g, data, parallel);
    }
    ico_entry(data, parallel);
    if let Ok(a) = ani::Ani::parse(data) {
        for i in 0..a.frames.len().min(4) {
            ico_entry(a.frame_data(data, i), parallel);
        }
        for s in &a.steps {
            assert!(
                s.frame < a.frames.len().max(1),
                "ANI step past the frame table"
            );
        }
    }
});
