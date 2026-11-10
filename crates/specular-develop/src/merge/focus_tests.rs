//! Synthetic focus stacks: a textured scene with a known depth per pixel (a tilted plane
//! and a near disc in front of it), each frame focused one step further and blurred by a
//! disc whose radius grows with the distance from focus, slightly scaled (focus
//! breathing), mosaiced (RGGB) with Poisson–Gaussian noise. The stack must give the
//! all-in-focus scene and the depth back.

use std::path::PathBuf;

use specular_core::CancelToken;
use specular_raw::sensor::{BlackLevels, DngColour, Layout, SensorMeta};
use specular_raw::{MatrixSource, Rect, SensorData, WbSource};

use super::*;

const W: u32 = 192;
const H: u32 = 144;
const FRAMES: usize = 7;
const BLACK: f32 = 512.0;
const WHITE: f32 = 15000.0;
const NA: f64 = 2.0e-4;
const NB: f64 = 4.0e-7;
/// Blur radius per depth step, pixels.
const BLUR: f64 = 1.6;
/// Padding of the texture grid.
const PAD: i64 = 16;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn normal(&mut self) -> f64 {
        let (u, v) = (self.next().max(1e-300), self.next());
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}

/// The scene's depth (in frame steps) at reference position (x, y).
fn depth(x: f64, y: f64) -> f64 {
    let (dx, dy) = (x - 128.0, y - 72.0);
    if dx * dx + dy * dy < 24.0 * 24.0 {
        1.0
    } else {
        (FRAMES - 1) as f64 * x / f64::from(W)
    }
}

/// The in-focus texture per colour on a padded grid.
struct Texture {
    w: usize,
    h: usize,
    v: [Vec<f64>; 3],
}

impl Texture {
    fn new() -> Texture {
        let (w, h) = ((W as i64 + 2 * PAD) as usize, (H as i64 + 2 * PAD) as usize);
        let mut rng = Rng(0x5eed);
        let blobs: Vec<(f64, f64, f64, f64)> = (0..140)
            .map(|_| {
                (
                    rng.next() * w as f64,
                    rng.next() * h as f64,
                    2.0 + rng.next() * 4.0,
                    (rng.next() - 0.5) * 0.5,
                )
            })
            .collect();
        let v = std::array::from_fn(|c| {
            let tint = [0.85, 1.0, 0.75][c];
            (0..w * h)
                .map(|i| {
                    let (x, y) = ((i % w) as f64, (i / w) as f64);
                    let mut t = 0.3 + 0.08 * (x * 0.7).sin() * (y * 0.55).cos();
                    for &(bx, by, r, k) in &blobs {
                        let d2 = ((x - bx).powi(2) + (y - by).powi(2)) / (r * r);
                        if d2 < 9.0 {
                            t += k * (-d2).exp();
                        }
                    }
                    (t * tint).clamp(0.02, 0.9)
                })
                .collect()
        });
        Texture { w, h, v }
    }

    /// Bilinear at reference position (x, y).
    fn at(&self, c: usize, x: f64, y: f64) -> f64 {
        let (gx, gy) = (x + PAD as f64, y + PAD as f64);
        let (x0, y0) = (gx.floor().max(0.0), gy.floor().max(0.0));
        let (fx, fy) = (gx - x0, gy - y0);
        let (x0, y0) = ((x0 as usize).min(self.w - 2), (y0 as usize).min(self.h - 2));
        let v = &self.v[c];
        let a = v[y0 * self.w + x0];
        let b = v[y0 * self.w + x0 + 1];
        let cc = v[(y0 + 1) * self.w + x0];
        let d = v[(y0 + 1) * self.w + x0 + 1];
        (a * (1.0 - fx) + b * fx) * (1.0 - fy) + (cc * (1.0 - fx) + d * fx) * fy
    }

    /// The mean over a disc of radius `r` around (x, y).
    fn blurred(&self, c: usize, x: f64, y: f64, r: f64) -> f64 {
        if r < 0.5 {
            return self.at(c, x, y);
        }
        let k = r.ceil() as i64;
        let (mut s, mut n) = (0.0, 0.0);
        for dy in -k..=k {
            for dx in -k..=k {
                let (fx, fy) = (dx as f64, dy as f64);
                if fx * fx + fy * fy <= r * r {
                    s += self.at(c, x + fx, y + fy);
                    n += 1.0;
                }
            }
        }
        s / n
    }
}

fn meta() -> SensorMeta {
    SensorMeta {
        layout: Layout::Bayer([0, 1, 1, 2]),
        black: BlackLevels::simple([BLACK; 4], 1),
        white: WHITE,
        clip_white: WHITE,
        linearization: None,
        opcodes: Default::default(),
        opcodes_unknown: Vec::new(),
        wb: [1.9, 1.0, 1.4],
        wb_source: WbSource::AsShot,
        rgb_cam: [[1.6, -0.4, -0.2], [-0.2, 1.4, -0.2], [0.0, -0.5, 1.5]],
        matrix_source: MatrixSource::Generic,
        dng: Some(DngColour {
            noise_profile: vec![[NA, NB]],
            ..Default::default()
        }),
        profile: None,
        orientation: 1,
        truncated: false,
        lens: None,
        default_scale: None,
    }
}

/// Frame `i`: focused at depth `i`, scaled by `scale` about the centre.
fn frame(t: &Texture, i: usize, scale: f64, seed: u64) -> Frame {
    let m = meta();
    let mut rng = Rng(seed | 1);
    let (cx, cy) = ((f64::from(W) - 1.0) / 2.0, (f64::from(H) - 1.0) / 2.0);
    let mut samples = Vec::with_capacity((W * H) as usize);
    for y in 0..H {
        for x in 0..W {
            let c = m.layout.color(x, y) as usize;
            // The reference position this sample shows.
            let (px, py) = (
                cx + (f64::from(x) - cx) / scale,
                cy + (f64::from(y) - cy) / scale,
            );
            let r = BLUR * (depth(px, py) - i as f64).abs();
            let v = t.blurred(c, px, py, r);
            let v = v + (NA * v + NB).sqrt() * rng.normal();
            let raw = f64::from(BLACK) + v * f64::from(WHITE - BLACK);
            samples.push(raw.round().clamp(0.0, f64::from(WHITE)) as u16);
        }
    }
    Frame {
        sensor: SensorData {
            width: W,
            height: H,
            cpp: 1,
            samples,
            float: None,
            crop: Rect {
                x: 0,
                y: 0,
                w: W,
                h: H,
            },
            meta: m,
        },
        shot: Shot {
            path: PathBuf::from(format!("/photos/F{i:03}.ORF")),
            make: "OM Digital".into(),
            model: "OM-1".into(),
            taken: Some(1000.0 + 0.1 * i as f64),
            exposure_time: Some(0.01),
            f_number: Some(5.6),
            iso: Some(200.0),
            focal_length: Some(60.0),
            focus_distance: Some(0.2 + 0.01 * i as f64),
            ..Shot::default()
        },
    }
}

fn frames(t: &Texture, breathing: f64) -> Vec<Frame> {
    (0..FRAMES)
        .map(|i| {
            let s = 1.0 + breathing * (i as f64 - (FRAMES / 2) as f64);
            frame(t, i, s, 11 + i as u64)
        })
        .collect()
}

/// PSNR (peak 1) of a mosaic against the in-focus scene over the interior.
fn psnr(t: &Texture, v: &[f32]) -> f64 {
    let m = meta();
    let (mut se, mut n) = (0.0, 0.0);
    for y in 16..H - 16 {
        for x in 16..W - 16 {
            let c = m.layout.color(x, y) as usize;
            let want = t.at(c, f64::from(x), f64::from(y));
            se += (f64::from(v[(y * W + x) as usize]) - want).powi(2);
            n += 1.0;
        }
    }
    10.0 * (n / se).log10()
}

/// Shares of interior cells whose depth is within 0.5 and 1 step of the truth.
fn depth_accuracy(d: &DepthMap) -> (f64, f64) {
    let (mut a, mut b, mut n) = (0.0, 0.0, 0.0);
    for cy in 8..d.height as usize - 8 {
        for cx in 8..d.width as usize - 8 {
            let (x, y) = (2.0 * cx as f64 + 0.5, 2.0 * cy as f64 + 0.5);
            let e = (f64::from(d.depth[cy * d.width as usize + cx]) - depth(x, y)).abs();
            a += f64::from(u8::from(e <= 0.5));
            b += f64::from(u8::from(e <= 1.0));
            n += 1.0;
        }
    }
    (a / n, b / n)
}

fn run(fr: Vec<Frame>, o: &Options) -> Stacked {
    stack(&InMemory(fr), o, &|_| {}, &CancelToken::new()).expect("stacked")
}

#[test]
fn depth_map_stack_is_sharp_throughout_and_finds_the_depth() {
    let t = Texture::new();
    let fr = frames(&t, 0.004);
    let single = {
        let p = normalise(&fr[FRAMES / 2].sensor);
        psnr(&t, &p.v)
    };
    let start = std::time::Instant::now();
    let s = run(
        fr,
        &Options {
            depth_map: true,
            ..Options::default()
        },
    );
    let took = start.elapsed();
    let p = psnr(&t, &s.merged.radiance);
    let d = s.depth.as_ref().expect("depth");
    let (half, one) = depth_accuracy(d);
    eprintln!(
        "depth map: PSNR {p:.2} dB (middle frame {single:.2} dB), depth within 0.5: {:.1} %, within 1: {:.1} %, {took:?}",
        half * 100.0,
        one * 100.0
    );
    assert!(
        p > single + 6.0 && p > 39.0,
        "PSNR {p:.2} dB vs {single:.2}"
    );
    assert!(half > 0.8 && one > 0.92, "depth {half:.3} / {one:.3}");
    // The breathing was found: frame 0 is 1.2 % smaller than the reference.
    let (x, _) = s.merged.warps[0].apply(f64::from(W) - 1.0, 71.5);
    let want = 95.5 + (f64::from(W) - 1.0 - 95.5) * (1.0 - 0.012);
    assert!(
        (x - want).abs() < 0.3,
        "warp of frame 0: {x:.3} vs {want:.3}"
    );
    let png = d.to_png().expect("png");
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(s.merged.reference, FRAMES / 2);
    assert_eq!(s.merged.shots.len(), FRAMES);
}

#[test]
fn pyramid_stack_is_sharp_throughout() {
    let t = Texture::new();
    let fr = frames(&t, 0.0);
    let single = psnr(&t, &normalise(&fr[FRAMES / 2].sensor).v);
    let start = std::time::Instant::now();
    let s = run(
        fr,
        &Options {
            method: Method::Pyramid,
            align: false,
            depth_map: false,
        },
    );
    let took = start.elapsed();
    let p = psnr(&t, &s.merged.radiance);
    eprintln!("pyramid: PSNR {p:.2} dB (middle frame {single:.2} dB), {took:?}");
    assert!(
        p > single + 6.0 && p > 40.0,
        "PSNR {p:.2} dB vs {single:.2}"
    );
    assert!(s.depth.is_none());
}

#[test]
fn stacks_refuse_mixed_cameras_and_too_many_frames() {
    let t = Texture::new();
    let mut fr = frames(&t, 0.0);
    fr.truncate(3);
    fr[1].sensor.meta.layout = Layout::Bayer([2, 1, 1, 0]);
    let r = stack(
        &InMemory(fr),
        &Options::default(),
        &|_| {},
        &CancelToken::new(),
    );
    assert!(matches!(r, Err(Error::NotOneCamera)));
    struct Many;
    impl FrameSource for Many {
        fn len(&self) -> usize {
            MAX_FRAMES + 1
        }
        fn get(&self, _: usize) -> Result<Frame, Error> {
            Err(Error::TooFew)
        }
    }
    let r = stack(&Many, &Options::default(), &|_| {}, &CancelToken::new());
    assert!(matches!(r, Err(Error::TooMany)));
    assert_eq!(
        depth_file_name(std::path::Path::new("/a/IMG_7.ORF")),
        "IMG_7-Stack-depth.png"
    );
}

#[test]
fn guided_filter_keeps_an_edge_and_flattens_noise() {
    let (w, h) = (40usize, 20usize);
    let g: Vec<f64> = (0..w * h)
        .map(|i| if i % w < 20 { 0.0 } else { 1.0 })
        .collect();
    let mut rng = Rng(3);
    let p: Vec<f64> = g.iter().map(|v| v * 4.0 + 0.2 * rng.normal()).collect();
    let q = guided(&g, &p, w, h, 4, 1e-3);
    for y in 0..h {
        assert!(q[y * w + 17].abs() < 0.2, "{}", q[y * w + 17]);
        assert!((q[y * w + 22] - 4.0).abs() < 0.2, "{}", q[y * w + 22]);
    }
}
