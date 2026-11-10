//! APNG compositing against a reference compositor over the `png` crate's raw frames.

mod common;

use common::*;
use specular_png::{Animator, Output, read_info};

#[allow(clippy::too_many_arguments)]
fn frame(
    w: usize,
    h: usize,
    x: u32,
    y: u32,
    dispose: u8,
    blend: u8,
    ct: u8,
    depth: u8,
    seed: u64,
) -> ApngFrame {
    ApngFrame {
        s: make(w, h, ct, depth, Content::Noise, seed),
        x,
        y,
        delay: (seed as u16 % 7, [0, 10, 100, 1000][seed as usize % 4]),
        dispose,
        blend,
    }
}

fn check(data: &[u8], w: usize, h: usize, default_separate: bool, what: &str) {
    let mut o = oracle(data);
    if default_separate {
        o.remove(0);
    }
    let want = composite(w, h, &o);
    let info = read_info(data).unwrap();
    assert_eq!(info.frames.len(), want.len(), "{what}");
    assert_eq!(info.default_image_is_frame, !default_separate, "{what}");
    for (out, tol) in [(Output::Rgba16, 1u16), (Output::Rgba8, 2)] {
        let mut a = Animator::new(data, out, 1).unwrap();
        for pass in 0..2 {
            for (i, wf) in want.iter().enumerate() {
                let f = a
                    .next_frame(None)
                    .expect("frame")
                    .unwrap_or_else(|e| panic!("{what}: {e}"));
                assert_eq!(f.index, i);
                let fi = &info.frames[i];
                assert_eq!((f.delay_num, f.delay_den), (fi.delay_num, fi.delay_den));
                let oc = o[i].fc.unwrap();
                assert_eq!((f.delay_num, f.delay_den), (oc.delay_num, oc.delay_den));
                let got: Vec<u16> = match out {
                    Output::Rgba16 => f.image.planes[0]
                        .data
                        .chunks_exact(2)
                        .map(|b| u16::from_ne_bytes([b[0], b[1]]))
                        .collect(),
                    _ => f.image.planes[0]
                        .data
                        .iter()
                        .map(|&v| u16::from(v))
                        .collect(),
                };
                for (p, (g, wv)) in got.chunks(4).zip(wf.chunks(4)).enumerate() {
                    let wv: Vec<u16> = if out == Output::Rgba16 {
                        wv.to_vec()
                    } else {
                        wv.iter().map(|&v| u16::from(r8(v))).collect()
                    };
                    // Colour of fully transparent pixels is irrelevant.
                    if wv[3] == 0 && g[3] == 0 {
                        continue;
                    }
                    for c in 0..4 {
                        assert!(
                            g[c].abs_diff(wv[c]) <= tol,
                            "{what} {out:?} frame {i} pixel {p}: {g:?} vs {wv:?}"
                        );
                    }
                }
            }
            assert!(a.next_frame(None).is_none());
            if pass == 0 {
                a.rewind();
            }
        }
    }
}

#[test]
fn compositing() {
    let (w, h) = (23, 19);
    let mut n = 0u64;
    for (ct, depth) in [(6u8, 8u8), (6, 16), (4, 8), (2, 8), (3, 8), (0, 4)] {
        for interlace in [false, true] {
            for default_separate in [false, true] {
                for dispose_set in 0..3u8 {
                    n += 1;
                    let extra = Extra {
                        palette: (ct == 3).then(|| palette(depth, n)),
                        trns: (ct == 3).then(|| (0..256).map(|i| (i * 37) as u8).collect()),
                        ..Default::default()
                    };
                    let mut frames = vec![frame(w, h, 0, 0, dispose_set, 0, ct, depth, n * 10)];
                    let rects = [
                        (5, 4, 3, 2),
                        (10, 7, 13, 12),
                        (1, 1, 0, 0),
                        (23, 19, 0, 0),
                        (4, 9, 19, 10),
                    ];
                    for (k, &(fw, fh, x, y)) in rects.iter().enumerate() {
                        let k = k as u8;
                        frames.push(frame(
                            fw,
                            fh,
                            x,
                            y,
                            (k + dispose_set) % 3,
                            k % 2,
                            ct,
                            depth,
                            n * 10 + u64::from(k) + 1,
                        ));
                    }
                    let def = make(w, h, ct, depth, Content::Photo, n);
                    let data = encode_apng(
                        w,
                        h,
                        &frames,
                        default_separate.then_some(&def),
                        interlace,
                        3,
                        &extra,
                    );
                    check(
                        &data,
                        w,
                        h,
                        default_separate,
                        &format!(
                            "ct {ct}/{depth} interlace {interlace} separate {default_separate} dispose {dispose_set}"
                        ),
                    );
                    let info = read_info(&data).unwrap();
                    assert_eq!(info.animation.unwrap().num_plays, 3);
                    // The default image decodes as the still image.
                    let still = specular_png::decode(&data, &Default::default()).unwrap();
                    let o = &oracle(&data)[0];
                    assert_eq!(
                        still.to_rgba8(),
                        o.rgba16.iter().map(|&v| r8(v)).collect::<Vec<_>>()
                    );
                }
            }
        }
    }
}

#[test]
fn scaled_frames_and_still() {
    let frames: Vec<_> = (0..3)
        .map(|i| frame(40, 30, 0, 0, 0, 1, 6, 8, i + 1))
        .collect();
    let data = encode_apng(40, 30, &frames, None, false, 0, &Extra::default());
    let mut a = Animator::new(&data[..], Output::Rgba8, 4).unwrap();
    let mut k = 0;
    while let Some(f) = a.next_frame(None) {
        let f = f.unwrap();
        assert_eq!((f.image.width, f.image.height), (10, 8));
        k += 1;
    }
    assert_eq!(k, 3);
    // A still PNG is a one-frame animation.
    let s = make(9, 9, 2, 8, Content::Photo, 1);
    let still = encode(&s, false, Filters::Mixed, &Extra::default(), 1);
    let mut a = Animator::new(&still[..], Output::Rgba8, 1).unwrap();
    assert!(a.next_frame(None).unwrap().is_ok());
    assert!(a.next_frame(None).is_none());
}
