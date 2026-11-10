//! Best-pick on a synthetic burst at camera-like size: blur a few pixels wide is invisible on
//! the 256 px signature thumbnail and smooths the noise there, so it must be judged on
//! full-resolution crops (`focus`), and noise must never be bought with blur.

use specular_core::{Orientation, PixelLayout, Raster, RasterPlane};
use specular_decode::Registry;
use specular_similar::Item;
use specular_similar::focus::{self, Focus};
use specular_similar::pick::{Pick, Reason, pick_similar};
use specular_similar::sig::{SIG_CLASS, Signature};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

const W: usize = 2000;
const H: usize = 1500;

fn hash(x: u64, y: u64, seed: u64) -> u64 {
    let mut h = x.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ y.wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ seed.wrapping_mul(0x1656_67B1_9E37_79F9);
    h ^= h >> 31;
    h = h.wrapping_mul(0xD6E8_FEB8_6659_FD93);
    h ^ (h >> 32)
}

/// The scene's luma, shifted by `dx` (a hand-held burst): smooth shading, hard edges and a
/// fine texture (foliage, fabric) that only full resolution resolves.
fn scene(dx: usize) -> Vec<f32> {
    let mut v = vec![0f32; W * H];
    for y in 0..H {
        for x in 0..W {
            let sx = x + dx;
            let (fx, fy) = (sx as f32, y as f32);
            let shade = 50.0 * (fx / 97.0).sin() * (fy / 131.0).cos();
            let checker = if (sx / 48 + y / 48).is_multiple_of(2) {
                25.0
            } else {
                -25.0
            };
            // Value noise on a 3 px lattice: texture, not (white) noise.
            let (gx, gy) = (sx / 3, y / 3);
            let (tx, ty) = ((sx % 3) as f32 / 3.0, (y % 3) as f32 / 3.0);
            let g = |x: usize, y: usize| (hash(x as u64, y as u64, 7) & 63) as f32 - 31.5;
            let grain = (g(gx, gy) * (1.0 - tx) + g(gx + 1, gy) * tx) * (1.0 - ty)
                + (g(gx, gy + 1) * (1.0 - tx) + g(gx + 1, gy + 1) * tx) * ty;
            v[y * W + x] = 128.0 + shade + checker + grain;
        }
    }
    v
}

/// Box blur of radius `r` both ways (defocus).
fn blur(v: &[f32], r: usize) -> Vec<f32> {
    let n = (2 * r + 1) as f32;
    let mut t = vec![0f32; W * H];
    for y in 0..H {
        for x in 0..W {
            let s: f32 = (x.saturating_sub(r)..=(x + r).min(W - 1))
                .map(|xx| v[y * W + xx])
                .sum();
            t[y * W + x] = s / n;
        }
    }
    let mut o = vec![0f32; W * H];
    for y in 0..H {
        for x in 0..W {
            let s: f32 = (y.saturating_sub(r)..=(y + r).min(H - 1))
                .map(|yy| t[yy * W + x])
                .sum();
            o[y * W + x] = s / n;
        }
    }
    o
}

/// RGBA with sensor noise of about `sigma` (after the optics, so blur does not smooth it).
fn rgba(v: &[f32], sigma: f32, seed: u64) -> Vec<u8> {
    let mut o = vec![255u8; v.len() * 4];
    for (i, &l) in v.iter().enumerate() {
        let h = hash(i as u64, 0, seed);
        // Sum of four uniforms: near Gaussian, unit variance.
        let g = (0..4)
            .map(|k| ((h >> (16 * k)) & 0xFFFF) as f32 / 65535.0 - 0.5)
            .sum::<f32>()
            * (12.0f32 / 4.0).sqrt();
        let l = l + sigma * g;
        let p = &mut o[i * 4..i * 4 + 3];
        p[0] = l.clamp(0.0, 255.0) as u8;
        p[1] = (l * 0.9 + 10.0).clamp(0.0, 255.0) as u8;
        p[2] = (250.0 - l * 0.8).clamp(0.0, 255.0) as u8;
    }
    o
}

struct Frame {
    name: &'static str,
    item: Item,
}

fn scratch() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let d = exe
        .ancestors()
        .nth(3)
        .unwrap()
        .join("tmp/specular-similar-bestpick");
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Encodes, measures and signs one frame as the scan would.
fn frame(name: &'static str, luma: &[f32], sigma: f32, seed: u64) -> Frame {
    frame_sized(name, luma, (W, H), sigma, seed)
}

/// [`frame`] of a `w`×`h` image.
fn frame_sized(
    name: &'static str,
    luma: &[f32],
    (w, h): (usize, usize),
    sigma: f32,
    seed: u64,
) -> Frame {
    let px = rgba(luma, sigma, seed);
    let path = scratch().join(format!("{name}.jpg"));
    std::fs::write(&path, specular_thumbs::codec::encode_jpeg(&px, w, h, 92)).unwrap();
    let f = focus::measure(&Registry::new(), &path, &AtomicBool::new(false)).unwrap();
    assert!(f.ok(), "{name}: {f:?}");
    let raster = Raster {
        width: w as u32,
        height: h as u32,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::Rgba,
        planes: vec![RasterPlane {
            width: w as u32,
            height: h as u32,
            stride: w * 4,
            data: px,
            h_div: 1,
            v_div: 1,
        }],
        truncated: false,
    };
    let t = specular_thumbs::pixels::thumbnail(&raster, Orientation::NORMAL, SIG_CLASS);
    Frame {
        name,
        item: Item {
            name: format!("{name}.jpg"),
            width: w as u32,
            height: h as u32,
            bit_depth: 8,
            sig: Signature::of_rgba(t.width, t.height, &t.rgba),
            focus: Some(f),
            ..Item::default()
        },
    }
}

fn pick(frames: &[&Frame]) -> Vec<Pick> {
    let items: Vec<Item> = frames.iter().map(|f| f.item.clone()).collect();
    let group: Vec<u32> = (0..items.len() as u32).collect();
    let picks = pick_similar(&items, &group);
    for (f, p) in frames.iter().zip(&picks) {
        let fc: Focus = f.item.focus.unwrap_or_default();
        eprintln!(
            "{:>12}: keep {:5} {:?} score {:.3}  detail {:.2} noise {:.2} aniso {:.3} | thumb detail {:.2} noise {:.2}",
            f.name,
            p.keep,
            p.reason,
            p.score,
            fc.detail,
            fc.noise,
            fc.anisotropy,
            f.item.sig.quality.detail,
            f.item.sig.quality.noise,
        );
    }
    picks
}

fn kept_best(p: &Pick) -> bool {
    p.keep && p.reason != Reason::AlsoGood
}

#[test]
fn the_sharp_frame_wins_over_blur_and_noise() {
    let sharp: Vec<Frame> = (0..4)
        .map(|k| {
            let name = ["sharp0", "sharp1", "sharp2", "sharp3"][k];
            frame(name, &scene(k), 2.0, 10 + k as u64)
        })
        .collect();
    let blurred = frame("blurred", &blur(&scene(4), 2), 2.0, 20);
    let noisy = frame("noisy_sharp", &scene(5), 7.0, 30);
    // A 3 px defocus: next to invisible at 8:1.
    let soft = frame("soft", &blur(&scene(6), 1), 2.0, 40);

    // Four sharp frames and one blurred: a sharp one is kept, the blurred one is discarded
    // for blur (not kept for "least noise").
    let burst: Vec<&Frame> = sharp.iter().chain([&blurred]).collect();
    let p = pick(&burst);
    let best = p.iter().position(kept_best).unwrap();
    assert!(best < 4, "a sharp frame wins: {p:?}");
    assert_ne!(p[best].reason, Reason::LeastNoise, "{p:?}");
    assert!(!p[4].keep && p[4].reason == Reason::Blurrier, "{p:?}");
    for q in &p[..4] {
        assert!(q.score > p[4].score, "{p:?}");
    }
    // The blurred frame is less detailed at full resolution, though not on the thumbnail.
    let fd = |f: &Frame| f.item.focus.unwrap().detail;
    assert!(fd(&blurred) < 0.7 * fd(&sharp[0]));

    // The softly blurred one too.
    let burst: Vec<&Frame> = sharp.iter().chain([&soft]).collect();
    let p = pick(&burst);
    assert!(p.iter().position(kept_best).unwrap() < 4, "{p:?}");
    assert!(!p[4].keep && p[4].reason == Reason::Blurrier, "{p:?}");

    // A noisy but sharp frame against a blurred clean one: the sharp one, for sharpness.
    let p = pick(&[&noisy, &blurred]);
    assert!(kept_best(&p[0]) && p[0].reason == Reason::Sharpest, "{p:?}");
    assert!(!p[1].keep && p[1].reason == Reason::Blurrier, "{p:?}");

    // Equally sharp, one noisy: the clean one; the noisy one goes for noise (or is as good).
    let p = pick(&[&noisy, &sharp[0]]);
    assert!(kept_best(&p[1]), "{p:?}");
    assert!(
        p[0].reason == Reason::Noisier || p[0].reason == Reason::AlsoGood,
        "{p:?}"
    );
    assert!(noisy.item.focus.unwrap().noise > 2.0 * sharp[0].item.focus.unwrap().noise);

    // Without the crops (not measured yet) the thumbnail decides, but blur never buys the
    // keep on noise.
    let thumb: Vec<Frame> = sharp
        .iter()
        .chain([&blurred])
        .map(|f| Frame {
            name: f.name,
            item: Item {
                focus: None,
                ..f.item.clone()
            },
        })
        .collect();
    let refs: Vec<&Frame> = thumb.iter().collect();
    let p = pick(&refs);
    let best = p.iter().position(kept_best).unwrap();
    assert_ne!(p[best].reason, Reason::LeastNoise, "{p:?}");
}

/// 2×2 box downscale of a `W`×`H` luma (a smaller export of the same shot).
fn half(v: &[f32]) -> Vec<f32> {
    let (w, h) = (W / 2, H / 2);
    let mut o = vec![0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let at = |dx: usize, dy: usize| v[(2 * y + dy) * W + 2 * x + dx];
            o[y * w + x] = (at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1)) / 4.0;
        }
    }
    o
}

/// A group of a full-size frame and a half-size one: measured at the smaller one's size, the
/// larger one compares at full resolution (no fallback to the thumbnails), and the sharp one
/// wins whichever size it is.
#[test]
fn mixed_sizes_compare_at_a_common_scale() {
    let small = (W / 2, H / 2);
    let side = (W / 2) as u32;
    let big_sharp = frame("big_sharp", &scene(0), 2.0, 50);
    let big_blurred = frame("big_blurred", &blur(&scene(1), 3), 2.0, 51);
    let small_sharp = frame_sized("small_sharp", &half(&scene(2)), small, 2.0, 52);
    let small_blurred = frame_sized("small_blurred", &half(&blur(&scene(3), 3)), small, 2.0, 53);
    let at = |f: &Frame| {
        let path = scratch().join(format!("{}.jpg", f.name));
        let g = focus::measure_at(&Registry::new(), &path, Some(side), &AtomicBool::new(false))
            .unwrap();
        assert!(g.ok(), "{}: {g:?}", f.name);
        Frame {
            name: f.name,
            item: Item {
                focus_at: vec![(side, g)],
                ..f.item.clone()
            },
        }
    };
    let (big_sharp, big_blurred) = (at(&big_sharp), at(&big_blurred));
    // The same scene, sharp, at the same sampling: about the same detail either way.
    let d_big = big_sharp.item.focus_at[0].1.detail;
    let d_small = small_sharp.item.focus.unwrap().detail;
    eprintln!(
        "detail: big at {side} {d_big:.2}, big own {:.2}, small {d_small:.2}",
        big_sharp.item.focus.unwrap().detail
    );
    assert!(
        d_big > 0.75 * d_small && d_big < 1.33 * d_small,
        "{d_big} vs {d_small}"
    );

    let p = pick(&[&big_sharp, &small_blurred]);
    assert!(kept_best(&p[0]), "{p:?}");
    assert!(!p[1].keep && p[1].reason == Reason::Blurrier, "{p:?}");
    let p = pick(&[&big_blurred, &small_sharp]);
    assert!(kept_best(&p[1]), "{p:?}");
    assert!(!p[0].keep && p[0].reason == Reason::Blurrier, "{p:?}");
    // The fine measures decide: without the common-scale one the thumbnails' do.
    let fine = pick(&[&big_sharp, &small_sharp]);
    let thumbs = Frame {
        name: "big_sharp",
        item: Item {
            focus_at: Vec::new(),
            ..big_sharp.item.clone()
        },
    };
    let coarse = pick(&[&thumbs, &small_sharp]);
    assert_ne!(
        fine.iter().map(|p| p.score).collect::<Vec<_>>(),
        coarse.iter().map(|p| p.score).collect::<Vec<_>>()
    );
}
