//! Hand-built DWAA / DWAB chunks (no corpus needed): unknown and RLE sections must decode
//! exactly, under version-2 rules and version-1 legacy rules; DC-only and AC blocks, alone and
//! as a Y'CbCr set, must decode to OpenEXR's formulas. The full comparison with OpenEXR is
//! `tests/conformance.rs`.

#![feature(f16)]

mod common;

use common::dwa::*;
use specular_exr::{ChannelSelect, DecodeOptions, Pixels, SampleFormat};

fn decode(file: &[u8], slots: [Option<&str>; 4]) -> Vec<f32> {
    let opts = DecodeOptions {
        channels: ChannelSelect::Slots(slots.map(|s| s.map(Into::into))),
        format: SampleFormat::F32,
        ..Default::default()
    };
    let img = specular_exr::decode(file, &opts).unwrap();
    assert_eq!(img.damaged_chunks, 0);
    let Pixels::F32(v) = img.pixels else {
        unreachable!()
    };
    v
}

/// OpenEXR's perceptual-to-linear curve on a half.
fn to_linear(x: f32) -> f32 {
    let h = x as f16 as f32;
    let a = h.abs();
    let l = if a <= 1.0 {
        a.powf(2.2)
    } else {
        (2.2 * (a - 1.0)).exp()
    };
    l.copysign(h) as f16 as f32
}

#[test]
fn unknown_and_rle_sections_are_exact() {
    let (w, h) = (13u32, 40u32);
    let n = (w * h) as usize;
    let a: Vec<f32> = (0..n).map(|i| (i % 7) as f32 * 0.125).collect();
    // Periodic, so that the chunks compress below their raw size.
    let z: Vec<f32> = (0..n).map(|i| (i % 50) as f32 * 0.37 - 20.0).collect();
    let id: Vec<f32> = (0..n).map(|i| (i % 50 * 3) as f32).collect();
    let chans = [("A", HALF), ("Z", FLOAT), ("id", UINT)];
    for (compression, lines) in [(DWAA, 32u32), (DWAB, 256)] {
        for version in [1, 2] {
            let mut chunks = Vec::new();
            for y0 in (0..h).step_by(lines as usize) {
                let r = (y0 * w) as usize..((y0 + lines).min(h) * w) as usize;
                let mut unknown = bytes(FLOAT, &z[r.clone()]);
                unknown.extend(bytes(UINT, &id[r.clone()]));
                chunks.push(chunk(&Sections {
                    rules: (version == 2).then(|| rule("A", -1, 2, HALF)),
                    unknown,
                    rle: planes(HALF, &a[r]),
                    ..Default::default()
                }));
            }
            let f = file(&chans, w, h, compression, &chunks, lines);
            let v = decode(&f, [Some("A"), Some("Z"), Some("id"), None]);
            for i in 0..n {
                let what = format!("compression {compression} v{version} pixel {i}");
                assert_eq!(v[i * 4], a[i], "{what}");
                assert_eq!(v[i * 4 + 1], z[i], "{what}");
                assert_eq!(v[i * 4 + 2], id[i], "{what}");
            }
        }
    }
}

#[test]
fn dc_only_and_ac_blocks() {
    // 24 x 8: three blocks of a lossy Y. Block 0 and 1 DC only, block 2 one AC coefficient.
    let dc = [half(8.0), half(4.0), 0];
    let ac = [0xff00, 0xff00, half(1.0), 0xff00];
    let c = chunk(&Sections {
        rules: Some(rule("Y", -1, 1, HALF)),
        ac: ac.to_vec(),
        dc: dc.to_vec(),
        ..Default::default()
    });
    let f = file(&[("Y", HALF)], 24, 8, DWAA, &[c], 32);
    let v = decode(&f, [Some("Y"), None, None, None]);
    let s = 3.535536e-01f32;
    for y in 0..8 {
        for x in 0..24 {
            let got = v[(y * 24 + x) * 4];
            let want = match x / 8 {
                0 => to_linear(8.0 * s * s),
                1 => to_linear(4.0 * s * s),
                // X[0][1] = 1: 0.5 cos(pi/4) * 0.5 cos((2x + 1) pi / 16).
                _ => {
                    let k = std::f32::consts::PI / 16.0;
                    to_linear(0.5 * (4.0 * k).cos() * 0.5 * ((2 * (x % 8) + 1) as f32 * k).cos())
                }
            };
            let tol = if x < 16 { 0.0 } else { want.abs() * 0.01 };
            assert!((got - want).abs() <= tol, "({x},{y}): {got} vs {want}");
        }
    }
}

#[test]
fn colour_set_is_converted() {
    // One 8 x 8 block of an R/G/B set, DC only: Y' = 1, Cb = 0, Cr = 0.125.
    let rules = [
        rule("R", 0, 1, HALF),
        rule("G", 1, 1, HALF),
        rule("B", 2, 1, HALF),
    ]
    .concat();
    let c = chunk(&Sections {
        rules: Some(rules),
        ac: vec![0xff00; 3],
        dc: vec![half(8.0), 0, half(1.0)],
        ..Default::default()
    });
    let f = file(
        &[("B", HALF), ("G", HALF), ("R", HALF)],
        8,
        8,
        DWAB,
        &[c],
        256,
    );
    let v = decode(&f, [Some("R"), Some("G"), Some("B"), None]);
    let s = 3.535536e-01f32;
    let (y, cr) = (8.0 * s * s, s * s);
    let want = [
        to_linear(y + 1.5747 * cr),
        to_linear(y - 0.4682 * cr),
        to_linear(y),
    ];
    for px in v.chunks_exact(4) {
        assert_eq!(&px[..3], &want);
    }
    assert!(want[0] > want[2] && want[1] < want[2]);
}

#[test]
fn malformed_chunks_are_damaged_not_fatal() {
    let c = chunk(&Sections {
        rules: Some(rule("Y", -1, 1, HALF)),
        ac: vec![0xff00],
        dc: vec![half(8.0)],
        ..Default::default()
    });
    let good = file(&[("Y", HALF)], 8, 8, DWAA, std::slice::from_ref(&c), 32);
    // Counters that disagree with the data: every one of the eleven, raised.
    for k in 0..11 {
        let mut bad = c.clone();
        let v = u64::from_le_bytes(bad[k * 8..k * 8 + 8].try_into().unwrap());
        bad[k * 8..k * 8 + 8].copy_from_slice(&(v + 1000).to_le_bytes());
        let f = file(&[("Y", HALF)], 8, 8, DWAA, &[bad], 32);
        let img = specular_exr::decode(&f, &Default::default()).unwrap();
        assert_eq!(img.damaged_chunks, 1, "counter {k}");
    }
    assert_eq!(
        specular_exr::decode(&good, &Default::default())
            .unwrap()
            .damaged_chunks,
        0
    );
}
