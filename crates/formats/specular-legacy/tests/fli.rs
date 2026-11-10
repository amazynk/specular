//! FLI/FLC animations built by the test encoder.

mod common;

use std::time::Duration;

use common::*;
use specular_legacy::fli::{self, FliFrames};
use specular_legacy::{Format, decode, detect};

/// Palette whose entries survive the 6-bit round trip of COLOR_64.
fn palette(seed: u64, six_bit: bool) -> [[u8; 3]; 256] {
    let mut r = rng(seed);
    let mut p = [[0u8; 3]; 256];
    for c in p.iter_mut() {
        for v in c.iter_mut() {
            let x = r() as u8;
            *v = if six_bit {
                let q = x >> 2;
                (q << 2) | (q >> 4)
            } else {
                x
            };
        }
    }
    p
}

/// A few frames that change partially (a moving block over a background).
fn frames(w: usize, h: usize, n: usize, six_bit: bool) -> Vec<FliFrameSpec> {
    let bg = pattern(w * h, 42, 255);
    let pal0 = palette(1, six_bit);
    (0..n)
        .map(|k| {
            let mut img = bg.clone();
            for y in k % h..(k % h + 3).min(h) {
                for x in k..(k + 4).min(w) {
                    img[y * w + x] = (k * 31 + x) as u8;
                }
            }
            // A run of identical values exercises run packets.
            for x in 0..w.min(8) {
                img[(h - 1) * w + x] = k as u8;
            }
            let pal = if k == 2 { palette(9, six_bit) } else { pal0 };
            (img, pal, 0, false)
        })
        .collect()
}

fn check_all(f: &[u8], spec: &FliSpec, delays: &[Duration]) {
    let mut it = FliFrames::new(f).unwrap();
    assert_eq!(it.info().frame_count as usize, spec.frames.len());
    assert_eq!(it.info().width as usize, spec.w);
    for pass in 0..2 {
        for (k, (img, pal, _, _)) in spec.frames.iter().enumerate() {
            let fr = it.next_frame().unwrap().unwrap();
            assert_eq!(fr.index as usize, k);
            assert!(!fr.image.truncated, "frame {k}");
            assert_eq!(fr.image.data, rgba(img, pal), "pass {pass} frame {k}");
            assert_eq!(fr.delay, delays[k], "frame {k}");
        }
        // The ring frame is not emitted.
        assert!(it.next_frame().is_none());
        assert!(it.next_frame().is_none());
        it.rewind().unwrap();
    }
}

#[test]
fn fli_brun_lc_color64() {
    let (w, h) = (23, 9);
    let spec = FliSpec {
        flc: false,
        w,
        h,
        speed: 5,
        frames: frames(w, h, 5, true),
        copy_first: false,
        extras: false,
    };
    let f = fli(&spec);
    assert_eq!(detect(&f), Some(Format::Fli));
    let d = Duration::from_micros(5 * 1_000_000 / 70);
    check_all(&f, &spec, &[d; 5]);
    let i = fli::fli_info(&f).unwrap();
    assert!(!i.flc);
    assert_eq!(i.loop_count, 0);
    // Still decode = first frame.
    assert_eq!(
        decode(&f).unwrap().data,
        rgba(&spec.frames[0].0, &spec.frames[0].1)
    );
}

#[test]
fn flc_copy_ss2_black_extras_delays() {
    // Odd width exercises the SS2 last-byte opcode.
    for w in [17, 16] {
        let h = 7;
        let mut fr = frames(w, h, 4, false);
        fr[1].2 = 250; // per-frame delay override
        fr[2].3 = true; // BLACK then delta from black
        let spec = FliSpec {
            flc: true,
            w,
            h,
            speed: 40,
            frames: fr,
            copy_first: true,
            extras: true,
        };
        let f = fli(&spec);
        let d = Duration::from_millis(40);
        check_all(&f, &spec, &[d, Duration::from_millis(250), d, d]);
        assert!(fli::fli_info(&f).unwrap().flc);
    }
}

#[test]
fn frames_is_send() {
    fn send<T: Send>() {}
    send::<FliFrames<Vec<u8>>>();
    send::<FliFrames<std::sync::Arc<[u8]>>>();
}

#[test]
fn fli_truncated_and_mutated() {
    let (w, h) = (13, 6);
    let spec = FliSpec {
        flc: true,
        w,
        h,
        speed: 10,
        frames: frames(w, h, 4, false),
        copy_first: false,
        extras: true,
    };
    let files = [fli(&spec), fli(&FliSpec { flc: false, ..spec })];
    let run = |d: &[u8]| {
        if let Ok(mut it) = FliFrames::new(d) {
            let mut n = 0;
            while let Some(r) = it.next_frame() {
                if r.is_err() {
                    break;
                }
                n += 1;
                assert!(n <= 65535);
            }
            let _ = it.rewind();
            let _ = it.next_frame();
        }
        let _ = decode(d);
    };
    for f in &files {
        for cut in 0..f.len() {
            run(&f[..cut]);
        }
        let mut r = rng(77);
        for _ in 0..3000 {
            let mut m = f.clone();
            for _ in 0..1 + r() % 4 {
                let i = (r() as usize) % m.len();
                m[i] = r() as u8;
            }
            run(&m);
        }
    }
    // A cut file still shows its complete leading frames.
    let f = &files[0];
    let mut it = FliFrames::new(&f[..f.len() * 2 / 3]).unwrap();
    let first = it.next_frame().unwrap().unwrap();
    assert!(!first.image.truncated);
    let (mut shown, mut flagged) = (1, false);
    while let Some(Ok(fr)) = it.next_frame() {
        shown += 1;
        flagged |= fr.image.truncated;
    }
    assert!(flagged || shown < 4);
}
