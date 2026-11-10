//! Layouts libtiff refuses or ignores: the horizontal predictor on depths that are not whole
//! bytes, palettes deeper than 16 bits and separated images with named inks (InkSet 2,
//! InkNames). Files are built here; the references are the same samples stored without the
//! predictor, the TIFF 6.0 palette lookup and the CMYK path.

mod common;

use common::{Comp, Spec, write};
use specular_tiff::{DecodeOptions, Decoded, PixelFormat, Tiff};

const W: u32 = 37;
const H: u32 = 23;

fn decode(data: &[u8], wide: bool) -> Decoded {
    let t = Tiff::open(data).expect("open");
    t.pages()[0].check_supported().expect("supported");
    let d = t
        .decode(
            data,
            0,
            &DecodeOptions {
                wide,
                ..Default::default()
            },
        )
        .expect("decode");
    assert!(!d.truncated);
    d
}

/// Random-ish `bits`-bit samples, `spp` per pixel, packed into rows as the file stores them,
/// and the values.
fn packed(w: u32, h: u32, spp: u16, bits: u16, le: bool, seed: u64) -> (Vec<u8>, Vec<u64>) {
    let mut s = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
    let mask = if bits >= 64 {
        u64::MAX
    } else {
        (1u64 << bits) - 1
    };
    let rb = common::row_bytes(w, spp, bits);
    let mut raw = vec![0u8; rb * h as usize];
    let mut vals = Vec::new();
    for (y, row) in raw.chunks_exact_mut(rb).enumerate() {
        for i in 0..(w * spp as u32) as usize {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            // Smooth along the row (what the predictor is for) plus noise.
            let ramp = (i as u64 * 3 + y as u64) << bits.saturating_sub(6);
            let v = ramp.wrapping_add(s % 5) & mask;
            common::put_sample(row, i, bits, le, v);
            vals.push(v);
        }
    }
    (raw, vals)
}

/// The horizontal predictor on depths other than 8, 16, 24, 32 and 64 bits (libtiff refuses
/// them): the decode equals the same samples stored without it.
#[test]
fn predictor_on_other_depths() {
    for bits in [1u16, 2, 4, 6, 10, 12, 20, 40, 48] {
        for (spp, photo) in [(1u16, 1u16), (3, 2)] {
            for (comp, le) in [
                (Comp::None, true),
                (Comp::Lzw, false),
                (Comp::Deflate, true),
            ] {
                let (raw, _) = packed(W, H, spp, bits, le, bits as u64 * 7 + spp as u64);
                let plain = Spec::new(W, H, spp, bits, photo, raw.clone());
                let mut pred = Spec::new(W, H, spp, bits, photo, raw);
                pred.comp = comp;
                pred.predictor = 2;
                let want = decode(&write(&[plain], false, le), true);
                let got = decode(&write(&[pred], false, le), true);
                assert!(got.data == want.data, "{bits}-bit, {spp} samples, {comp:?}");
            }
        }
    }
}

/// Palettes of 17 and 20 bits: the indices pick ColorMap entries as the TIFF 6.0 palette
/// definition says.
#[test]
fn palettes_over_16_bits() {
    for bits in [17u16, 20] {
        let n = 1usize << bits;
        let map: Vec<u64> = (0..3 * n)
            .map(|i| ((i as u64).wrapping_mul(2_654_435_761) >> 7) & 0xffff)
            .collect();
        let (raw, idx) = packed(W, H, 1, bits, true, bits as u64);
        let mut s = Spec::new(W, H, 1, bits, 3, raw);
        s.extra.push((320, 3, map.clone()));
        let d = decode(&write(&[s], false, true), false);
        assert_eq!(d.format, PixelFormat::Rgba8);
        for (p, &i) in d.data.chunks_exact(4).zip(&idx) {
            let i = i as usize;
            let want = [
                (map[i] >> 8) as u8,
                (map[n + i] >> 8) as u8,
                (map[2 * n + i] >> 8) as u8,
                255,
            ];
            assert_eq!(p, want, "{bits}-bit index {i}");
        }
    }
    // Deeper palettes are refused by name.
    let mut s = Spec::new(4, 4, 1, 24, 3, vec![0; 48]);
    s.extra.push((320, 3, vec![0; 3]));
    let f = write(&[s], false, true);
    let e = Tiff::open(&f).unwrap().pages()[0]
        .check_supported()
        .unwrap_err();
    assert!(e.to_string().contains("24-bit palette"), "{e}");
}

fn names(n: &[&str]) -> (u16, u16, Vec<u64>) {
    let mut v = Vec::new();
    for s in n {
        v.extend(s.bytes().map(u64::from));
        v.push(0);
    }
    (333, 2, v)
}

/// Separated images with named inks: process inks in another order read as CMYK, spot inks
/// by the colour word in their name, unnamed InkSet 2 inks as CMYK when there are four.
/// (ImageMagick and libtiff ignore InkNames: the references are the CMYK path and the ink
/// model.)
#[test]
fn named_inks() {
    let cmyk: Vec<u8> = (0..W * H * 4)
        .map(|i| ((i * 37 + i / 4 * 11) % 256) as u8)
        .collect();
    let base = decode(
        &write(&[Spec::new(W, H, 4, 8, 5, cmyk.clone())], false, true),
        false,
    );
    // Black, Cyan, Magenta, Yellow: the same picture with the samples in that order.
    let kcmy: Vec<u8> = cmyk
        .chunks_exact(4)
        .flat_map(|p| [p[3], p[0], p[1], p[2]])
        .collect();
    let mut s = Spec::new(W, H, 4, 8, 5, kcmy);
    s.extra.push((332, 3, vec![2]));
    s.extra.push(names(&["Black", "Cyan", "Magenta", "Yellow"]));
    assert!(decode(&write(&[s], false, true), false).data == base.data);
    // Unnamed InkSet 2 with four inks: CMYK.
    let mut s = Spec::new(W, H, 4, 8, 5, cmyk);
    s.extra.push((332, 3, vec![2]));
    assert!(decode(&write(&[s], false, true), false).data == base.data);
    // Spot inks: full orange alone, full "PANTONE 485 C" (no colour word: mid grey) alone,
    // half of each.
    let px: [[u8; 2]; 3] = [[255, 0], [0, 255], [128, 128]];
    let raw: Vec<u8> = (0..W * H).flat_map(|i| px[i as usize % 3]).collect();
    let mut s = Spec::new(W, H, 2, 8, 5, raw);
    s.extra.push((332, 3, vec![2]));
    s.extra.push((334, 3, vec![2]));
    s.extra.push(names(&["Orange", "PANTONE 485 C"]));
    let d = decode(&write(&[s], false, true), false);
    // Each ink keeps 1 − t·(1 − ink/255) of each channel; orange is (255, 128, 0).
    let mix = |t: f64, ink: [f64; 3], rgb: [f64; 3]| -> [f64; 3] {
        [0, 1, 2].map(|c| rgb[c] * (1.0 - t / 255.0 * (1.0 - ink[c] / 255.0)))
    };
    let orange = [255.0, 128.0, 0.0];
    let grey = [128.0; 3];
    for (i, p) in d.data.chunks_exact(4).enumerate().take(6) {
        let [a, b] = px[i % 3];
        let rgb = mix(b as f64, grey, mix(a as f64, orange, [255.0; 3]));
        for c in 0..3 {
            assert!(
                (p[c] as f64 - rgb[c]).abs() <= 1.0,
                "pixel {i}: {:?} vs {rgb:?}",
                &p[..3]
            );
        }
        assert_eq!(p[3], 255);
    }
}
