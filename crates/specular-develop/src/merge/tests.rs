//! Synthetic brackets: a known radiance map photographed at several exposures with
//! Poisson–Gaussian noise, shifts, rotation and a moving object; the merge must give the map
//! back.

use std::path::PathBuf;

use specular_core::CancelToken;
use specular_raw::sensor::{BlackLevels, DngColour, Layout, SensorMeta};
use specular_raw::{MatrixSource, Rect, SensorData, WbSource};

use super::*;

const BLACK: f32 = 512.0;
const WHITE: f32 = 15000.0;
/// Noise in normalised units: variance `A·v + B`.
const A: f64 = 2.0e-4;
const B: f64 = 4.0e-7;

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

/// The scene's radiance (reference units: 1 = the reference's white) at reference position
/// (x, y), colour c: a wide range (about 2⁻⁷ … 6) with smooth detail to align on.
fn scene(x: f64, y: f64, c: usize) -> f64 {
    let (u, v) = (x / 40.0, y / 40.0);
    let base = 0.02 * (1.0 + 0.5 * (u * 1.7).sin() * (v * 1.3).cos());
    let blob = |cx: f64, cy: f64, r: f64, k: f64| {
        k * (-((x - cx).powi(2) + (y - cy).powi(2)) / (r * r)).exp()
    };
    let bright =
        blob(250.0, 90.0, 30.0, 5.5) + blob(90.0, 180.0, 22.0, 0.6) + blob(300.0, 200.0, 18.0, 1.4);
    let ramp = 0.004 + 0.3 * (x / 384.0).powi(3);
    let tint = [0.9, 1.0, 0.8][c];
    (base + bright + ramp) * tint
}

/// A moving dark square in frame `k` (its top-left in reference coordinates), if any.
type Object = Option<(f64, f64)>;

fn inverse(t: &Transform, q: (f64, f64)) -> (f64, f64) {
    let (s, c) = t.angle.sin_cos();
    let (dx, dy) = (q.0 - t.cx - t.tx, q.1 - t.cy - t.ty);
    (c * dx + s * dy + t.cx, -s * dx + c * dy + t.cy)
}

const W: u32 = 384;
const H: u32 = 256;

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
            noise_profile: vec![[A, B]],
            ..Default::default()
        }),
        profile: None,
        orientation: 1,
        truncated: false,
        lens: None,
        default_scale: None,
    }
}

/// Frame of exposure `e` seen through `t` (reference → frame), with noise when `seed` is
/// set and a dark square at `object`.
fn frame(e: f64, t: &Transform, seed: Option<u64>, object: Object, name: &str) -> Frame {
    frame_by(e, &|q| inverse(t, q), seed, object, name)
}

/// [`frame`] with any map from frame positions back to reference positions.
fn frame_by(
    e: f64,
    back: &dyn Fn((f64, f64)) -> (f64, f64),
    seed: Option<u64>,
    object: Object,
    name: &str,
) -> Frame {
    let mut rng = Rng(seed.unwrap_or(1) | 1);
    let m = meta();
    let mut samples = Vec::with_capacity((W * H) as usize);
    for y in 0..H {
        for x in 0..W {
            let c = m.layout.color(x, y) as usize;
            let (px, py) = back((f64::from(x), f64::from(y)));
            let mut r = scene(px, py, c);
            if let Some((ox, oy)) = object
                && (ox..ox + 40.0).contains(&px)
                && (oy..oy + 40.0).contains(&py)
            {
                r = 0.004 * [0.6, 1.0, 1.4][c];
            }
            let mut v = e * r;
            if seed.is_some() {
                v += (A * v.max(0.0) + B).sqrt() * rng.normal();
            }
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
            path: PathBuf::from(format!("/photos/{name}.CR2")),
            make: "Canon".into(),
            model: "EOS 5D Mark IV".into(),
            taken: Some(1000.0),
            exposure_time: Some(e / 100.0),
            f_number: Some(8.0),
            iso: Some(100.0),
            ..Shot::default()
        },
    }
}

fn shift(tx: f64, ty: f64, angle: f64) -> Transform {
    Transform {
        angle,
        tx,
        ty,
        cx: (f64::from(W) - 1.0) / 2.0,
        cy: (f64::from(H) - 1.0) / 2.0,
    }
}

fn run(frames: Vec<Frame>, o: &Options) -> Merged {
    merge(frames, o, &|_| {}, &CancelToken::new()).expect("merged")
}

/// Mean relative error of 8×8 same-colour block means against the scene, over the
/// interior; and the worst block.
fn block_error(m: &Merged, skip: &dyn Fn(u32, u32) -> bool) -> (f64, f64) {
    let (mut sum, mut worst, mut n) = (0.0, 0.0f64, 0);
    for by in (16..H - 16).step_by(16) {
        for bx in (16..W - 16).step_by(16) {
            if skip(bx, by) {
                continue;
            }
            for c in 0..4u32 {
                let (ox, oy) = (c & 1, c >> 1);
                let (mut got, mut want) = (0.0, 0.0);
                for y in (by + oy..by + 16).step_by(2) {
                    for x in (bx + ox..bx + 16).step_by(2) {
                        let col = m.mosaic.color(x, y) as usize;
                        got += f64::from(m.radiance[(y * W + x) as usize]);
                        want += scene(f64::from(x), f64::from(y), col);
                    }
                }
                let e = (got - want).abs() / want;
                sum += e;
                worst = worst.max(e);
                n += 1;
            }
        }
    }
    (sum / n as f64, worst)
}

/// The largest distance between where `a` and `b` put a corner of the picture.
fn misalignment(a: &Transform, b: &Transform) -> f64 {
    let (w, h) = (f64::from(W) - 1.0, f64::from(H) - 1.0);
    [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h), (w / 2.0, h / 2.0)]
        .iter()
        .map(|&(x, y)| {
            let (p, q) = (a.apply(x, y), b.apply(x, y));
            ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt()
        })
        .fold(0.0, f64::max)
}

#[test]
fn ecc_recovers_synthetic_shifts_within_a_tenth_of_a_pixel() {
    let id = shift(0.0, 0.0, 0.0);
    let reference = frame(1.0, &id, Some(3), None, "ref");
    let cases = [
        shift(0.37, -0.81, 0.0),
        shift(-5.62, 3.25, 0.0),
        shift(11.4, -7.7, 0.0),
        shift(2.3, 1.1, 0.004),
        shift(-3.8, 6.4, -0.008),
    ];
    let mut worst = 0.0f64;
    for (k, t) in cases.iter().enumerate() {
        for (e, motion) in [
            (1.0, Motion::Euclidean),
            (0.25, Motion::Euclidean),
            (4.0, Motion::Euclidean),
        ] {
            if t.angle != 0.0 && motion == Motion::Translation {
                continue;
            }
            let f = frame(e, t, Some(10 + k as u64), None, "f");
            let pr = normalise(&reference.sensor);
            let pf = normalise(&f.sensor);
            let tp = align::proxy(&pr, 2, 0.002);
            let fp = align::proxy(&pf, 2, 0.002);
            let found = align::ecc(&tp, &fp, motion).expect("aligned");
            let got = align::to_samples(&found.transform, 2, W, H);
            let d = misalignment(&got, t);
            eprintln!(
                "shift {k} exposure {e}: misalignment {d:.4} px (rho {:.4})",
                found.rho
            );
            worst = worst.max(d);
        }
    }
    eprintln!("worst misalignment {worst:.4} px");
    assert!(worst <= 0.1, "misalignment {worst} px");
}

#[test]
fn translation_only_alignment_finds_shifts() {
    let id = shift(0.0, 0.0, 0.0);
    let reference = frame(1.0, &id, None, None, "ref");
    let t = shift(-2.4, 4.6, 0.0);
    let f = frame(0.5, &t, None, None, "f");
    let tp = align::proxy(&normalise(&reference.sensor), 2, 0.002);
    let fp = align::proxy(&normalise(&f.sensor), 2, 0.002);
    let found = align::ecc(&tp, &fp, Motion::Translation).expect("aligned");
    let d = misalignment(&align::to_samples(&found.transform, 2, W, H), &t);
    assert!(d <= 0.05, "translation misalignment {d}");
}

#[test]
fn noise_free_merge_returns_the_radiance() {
    let id = shift(0.0, 0.0, 0.0);
    let frames = vec![
        frame(0.125, &id, None, None, "a"),
        frame(1.0, &id, None, None, "b"),
        frame(8.0, &id, None, None, "c"),
    ];
    let m = run(
        frames,
        &Options {
            motion: Motion::None,
            ..Options::default()
        },
    );
    assert_eq!(m.reference, 1);
    for (i, e) in m.exposures.iter().enumerate() {
        let want = [0.125, 1.0, 8.0][i];
        assert!(
            (e / want - 1.0).abs() < 0.002,
            "exposure {i}: {e} vs {want}"
        );
    }
    // Every sample, not just blocks: the quantisation of the longest usable frame only.
    let mut worst = 0.0f64;
    for y in 0..H {
        for x in 0..W {
            let col = m.mosaic.color(x, y) as usize;
            let want = scene(f64::from(x), f64::from(y), col);
            let got = f64::from(m.radiance[(y * W + x) as usize]);
            worst = worst.max((got - want).abs() / want);
        }
    }
    eprintln!("noise-free worst sample error {worst:.2e}");
    assert!(worst < 0.01, "worst sample error {worst}");
    // The brightest blob (5.5 × the reference's white) is there, unclipped.
    let peak = m.radiance.iter().copied().fold(0.0f32, f32::max);
    assert!(peak > 5.0, "peak {peak}");
}

#[test]
fn noisy_shifted_brackets_merge_within_one_percent() {
    let frames = vec![
        frame(0.25, &shift(3.4, -2.2, 0.003), Some(21), None, "a"),
        frame(1.0, &shift(0.0, 0.0, 0.0), Some(22), None, "b"),
        frame(4.0, &shift(-4.7, 1.9, -0.002), Some(23), None, "c"),
    ];
    let truths = [
        shift(3.4, -2.2, 0.003),
        shift(0.0, 0.0, 0.0),
        shift(-4.7, 1.9, -0.002),
    ];
    let t0 = std::time::Instant::now();
    let m = run(frames, &Options::default());
    eprintln!(
        "merge of 3 × {W}×{H}: {:.1} ms",
        t0.elapsed().as_secs_f64() * 1e3
    );
    for (i, t) in truths.iter().enumerate() {
        let d = misalignment(&m.transforms[i], t);
        eprintln!(
            "frame {i}: misalignment {d:.4} px, exposure {:.4}",
            m.exposures[i]
        );
        assert!(d <= 0.1, "frame {i} misalignment {d}");
    }
    let (mean, worst) = block_error(&m, &|_, _| false);
    eprintln!(
        "radiance error: mean {:.3} %, worst block {:.3} %",
        mean * 100.0,
        worst * 100.0
    );
    assert!(mean < 0.01, "mean radiance error {mean}");
    // No ghosts where nothing moved.
    assert!(
        m.ghosts.coverage() < 0.01,
        "ghost coverage {}",
        m.ghosts.coverage()
    );
}

#[test]
fn merge_is_less_noisy_than_the_reference_alone() {
    // A flat dark patch: the long frame must bring the noise down.
    let id = shift(0.0, 0.0, 0.0);
    let frames = vec![
        frame(1.0, &id, Some(31), None, "a"),
        frame(4.0, &id, Some(32), None, "b"),
    ];
    let reference = normalise(&frames[0].sensor);
    let m = run(
        frames,
        &Options {
            motion: Motion::None,
            reference: Some(0),
            ..Options::default()
        },
    );
    let rms = |v: &dyn Fn(u32, u32) -> f64| {
        let mut s = 0.0;
        let mut n = 0;
        for y in (20..60).step_by(2) {
            for x in (20..60).step_by(2) {
                let want = scene(f64::from(x), f64::from(y), 0);
                s += ((v(x, y) - want) / want).powi(2);
                n += 1;
            }
        }
        (s / f64::from(n)).sqrt()
    };
    let merged = rms(&|x, y| f64::from(m.radiance[(y * W + x) as usize]));
    let single = rms(&|x, y| f64::from(reference.v[(y * W + x) as usize]));
    eprintln!("shadow noise: reference {single:.4}, merge {merged:.4}");
    // Four times the light: about half the noise.
    assert!(
        merged < 0.65 * single,
        "merge {merged} vs reference {single}"
    );
}

#[test]
fn moving_object_ghosts_are_suppressed() {
    let id = shift(0.0, 0.0, 0.0);
    // The object moves across the frames; the reference (middle) has it at (60, 60).
    let objects = [
        Some((150.0, 60.0)),
        Some((60.0, 60.0)),
        Some((240.0, 120.0)),
    ];
    let mk = || {
        vec![
            frame(0.25, &id, Some(41), objects[0], "a"),
            frame(1.0, &id, Some(42), objects[1], "b"),
            frame(4.0, &id, Some(43), objects[2], "c"),
        ]
    };
    // Blocks where the object is in another frame but not the reference: the scene's.
    let ghost_area = |x: u32, y: u32| {
        let inside = |o: (f64, f64)| {
            let (x, y) = (f64::from(x), f64::from(y));
            x + 16.0 > o.0 - 8.0 && x < o.0 + 48.0 && y + 16.0 > o.1 - 8.0 && y < o.1 + 48.0
        };
        let theirs =
            inside(objects[0].unwrap_or_default()) || inside(objects[2].unwrap_or_default());
        theirs && !inside(objects[1].unwrap_or_default())
    };
    let only = |x: u32, y: u32| !ghost_area(x, y);
    // The reference's object: its own radiance, kept.
    let on = run(
        mk(),
        &Options {
            motion: Motion::None,
            ..Options::default()
        },
    );
    let off = run(
        mk(),
        &Options {
            motion: Motion::None,
            ghosts: None,
            ..Options::default()
        },
    );
    let (g_on, w_on) = block_error(&on, &only);
    let (g_off, w_off) = block_error(&off, &only);
    eprintln!(
        "ghost areas: with ghost handling mean {:.2} % (worst {:.2} %), without {:.2} % (worst {:.2} %); mask {:.2} %",
        g_on * 100.0,
        w_on * 100.0,
        g_off * 100.0,
        w_off * 100.0,
        on.ghosts.coverage() * 100.0
    );
    assert!(g_on < 0.01, "ghost areas still off by {g_on}");
    assert!(
        g_off > 0.1,
        "the test's ghosts are too weak to show ({g_off})"
    );
    assert!(on.ghosts.coverage() > 0.01 && on.ghosts.coverage() < 0.2);
    // The mask covers the object's other places.
    let gm = &on.ghosts;
    let at = |x: u32, y: u32| gm.mask[((y / gm.block) * gm.width + x / gm.block) as usize];
    assert!(
        at(170, 80) > 128 && at(260, 140) > 128,
        "ghosts not in the mask"
    );
    assert!(at(330, 30) == 0, "a still area in the mask");
    let (pw, ph, px) = on.ghost_preview();
    assert_eq!(px.len(), (pw * ph * 4) as usize);
}

#[test]
fn float_dng_reads_back_as_the_merge() {
    let id = shift(0.0, 0.0, 0.0);
    let frames = vec![
        frame(0.25, &id, Some(51), None, "IMG_0001"),
        frame(1.0, &id, Some(52), None, "IMG_0002"),
        frame(4.0, &id, Some(53), None, "IMG_0003"),
    ];
    let m = run(
        frames,
        &Options {
            motion: Motion::None,
            ..Options::default()
        },
    );
    for (bits, tol) in [
        (dng::Bits::F32, 1e-6),
        (dng::Bits::F24, 2e-5),
        (dng::Bits::F16, 1.1e-3),
    ] {
        let bytes = dng::encode(&m, &[], &dng::DngOptions { bits, cfa: true });
        let f = specular_raw::parse(&bytes).expect("parses");
        assert!(f.can_decode_sensor());
        let s = f
            .decode_sensor(&bytes, &specular_raw::DecodeOptions::default())
            .expect("decodes");
        let s = apply_baseline(s);
        assert_eq!((s.width, s.height, s.cpp), (W, H, 1));
        assert_eq!(s.meta.layout, m.mosaic.layout());
        let fl = s.float.as_ref().expect("float samples");
        let mut worst = 0.0f64;
        let mut bad = 0;
        for (i, (&a, &b)) in fl.iter().zip(&m.radiance).enumerate() {
            let got = f64::from(a) / 65535.0;
            let want = f64::from(b);
            if want <= 1e-4 {
                continue;
            }
            let e = (got - want).abs() / want;
            if e > tol {
                bad += 1;
                if bad < 3 {
                    eprintln!("sample {i}: {got} vs {want}");
                }
            }
            worst = worst.max(e);
        }
        eprintln!(
            "{bits:?}: {} bytes, worst relative difference {worst:.2e}",
            bytes.len()
        );
        assert_eq!(bad, 0, "{bits:?}: {bad} samples off by more than {tol}");
        // The sources and the preview.
        let (sources, reference) = dng::sources_of(&bytes).expect("XMP");
        assert_eq!(sources, ["IMG_0001.CR2", "IMG_0002.CR2", "IMG_0003.CR2"]);
        assert_eq!(reference, "IMG_0002.CR2");
        assert!(f.info().largest_preview().is_some());
        assert_eq!(f.info().make, "Canon");
    }
    // Demosaiced: three samples per pixel, read back as linear data.
    let bytes = dng::encode(
        &m,
        &[],
        &dng::DngOptions {
            bits: dng::Bits::F32,
            cfa: false,
        },
    );
    let f = specular_raw::parse(&bytes).expect("parses");
    let s = apply_baseline(
        f.decode_sensor(&bytes, &specular_raw::DecodeOptions::default())
            .expect("decodes"),
    );
    assert_eq!((s.cpp, &s.meta.layout), (3, &Layout::Linear));
    // Green at a green site equals the mosaic's.
    let fl = s.float.as_ref().expect("float");
    let (x, y) = (101u32, 40u32);
    assert_eq!(m.mosaic.color(x, y), 1);
    let g = f64::from(fl[((y * W + x) * 3 + 1) as usize]) / 65535.0;
    let want = f64::from(m.radiance[(y * W + x) as usize]);
    assert!((g - want).abs() / want < 1e-4, "{g} vs {want}");
}

/// The reference's DNG opcodes that still hold after merging reach the merged DNG, moved to
/// the merged area (gain maps, bad-pixel lists, warps and vignetting rescaled to its corners);
/// ones that don't (tables, bad-pixel constants) are left out.
#[test]
fn merged_dng_carries_the_opcodes_that_still_hold() {
    use specular_raw::Rect;
    use specular_raw::sensor::{OpcodeInfo, PointKind};
    let id = shift(0.0, 0.0, 0.0);
    let mut m = run(
        vec![
            frame(1.0, &id, None, None, "a"),
            frame(4.0, &id, None, None, "b"),
        ],
        &Options {
            motion: Motion::None,
            ghosts: None,
            ..Options::default()
        },
    );
    let (w, h) = (m.width, m.height);
    m.crop = Rect { x: 10, y: 6, w, h };
    let wide = Rect {
        x: 8,
        y: 4,
        w: w + 4,
        h: h + 4,
    };
    let centre = (10.0 + f64::from(w) / 2.0, 6.0 + f64::from(h) / 2.0);
    let far2 = |c: (f64, f64), b: Rect| {
        let (x0, x1) = (f64::from(b.x), f64::from(b.x + b.w));
        let (y0, y1) = (f64::from(b.y), f64::from(b.y + b.h));
        [(x0, y0), (x1, y0), (x0, y1), (x1, y1)]
            .iter()
            .map(|&(x, y)| (x - c.0).powi(2) + (y - c.1).powi(2))
            .fold(0.0, f64::max)
    };
    let point = |kind: PointKind, bounds: Rect| OpcodeInfo::Point {
        rect: wide,
        plane: 0,
        planes: 1,
        row_pitch: 1,
        col_pitch: 1,
        bounds,
        kind,
    };
    m.meta.opcodes = [
        vec![
            OpcodeInfo::FixBadPixelsConstant {
                value: 0,
                bounds: wide,
            },
            OpcodeInfo::FixBadPixelsList {
                points: vec![(6 + 5, 10 + 7), (1, 1)],
                rects: vec![],
                bounds: wide,
            },
        ],
        vec![
            point(PointKind::MapTable(vec![0, 65535]), wide),
            point(
                PointKind::GainMap {
                    points_v: 2,
                    points_h: 2,
                    spacing_v: 1.0,
                    spacing_h: 1.0,
                    origin_v: 0.0,
                    origin_h: 0.0,
                    planes: 1,
                    gains: vec![1.0, 2.0, 3.0, 4.0],
                },
                wide,
            ),
        ],
        vec![
            OpcodeInfo::Warp {
                fisheye: false,
                k: [[1.0, 0.02, -0.01, 0.0, 0.001, -0.002]; 3],
                centre: (0.5, 0.5),
                bounds: wide,
            },
            point(
                PointKind::Vignette {
                    k: [0.3, -0.1, 0.0, 0.0, 0.0],
                    centre,
                    inv_m2: 1.0 / far2(centre, wide),
                },
                wide,
            ),
        ],
    ];
    let bytes = dng::encode(&m, &[], &dng::DngOptions::default());
    let f = specular_raw::parse(&bytes).expect("parses");
    let s = f
        .decode_sensor(&bytes, &specular_raw::DecodeOptions::default())
        .expect("decodes");
    let [l1, l2, l3] = &s.meta.opcodes;
    // List 1: the bad pixel inside the merged area, moved; the constant left out.
    assert_eq!(l1.len(), 1, "{l1:?}");
    let OpcodeInfo::FixBadPixelsList { points, .. } = &l1[0] else {
        panic!("{l1:?}")
    };
    assert_eq!(points, &vec![(5, 7)]);
    // List 2: the gain map with the same gain at the same photo point; no table.
    assert_eq!(l2.len(), 1, "{l2:?}");
    let OpcodeInfo::Point {
        rect,
        bounds,
        kind:
            PointKind::GainMap {
                origin_v,
                spacing_v,
                origin_h,
                spacing_h,
                ..
            },
        ..
    } = &l2[0]
    else {
        panic!("{l2:?}")
    };
    assert_eq!((rect.x, rect.y, rect.w, rect.h), (0, 0, w, h));
    // Map row positions (fractions of the bounds) of a photo row, before and after.
    let y_abs = 6.0 + 20.5;
    let old_v = (y_abs - 4.0) / f64::from(h + 4);
    let new_v = (y_abs - 6.0 - f64::from(bounds.y)) / f64::from(bounds.h);
    assert!(((new_v - origin_v) / spacing_v - old_v).abs() < 1e-9);
    let x_abs = 10.0 + 31.5;
    let old_h = (x_abs - 8.0) / f64::from(w + 4);
    let new_h = (x_abs - 10.0 - f64::from(bounds.x)) / f64::from(bounds.w);
    assert!(((new_h - origin_h) / spacing_h - old_h).abs() < 1e-9);
    // List 3: the warp and the vignette act the same at the same photo point.
    assert_eq!(l3.len(), 2, "{l3:?}");
    let OpcodeInfo::Warp {
        k,
        centre: c,
        bounds: b,
        ..
    } = &l3[0]
    else {
        panic!("{l3:?}")
    };
    let c_new = (
        f64::from(b.x) + c.0 * f64::from(b.w),
        f64::from(b.y) + c.1 * f64::from(b.h),
    );
    assert!((c_new.0 + 10.0 - centre.0).abs() < 1e-9 && (c_new.1 + 6.0 - centre.1).abs() < 1e-9);
    let radial = |k: &[f64; 6], r2: f64| k[0] + r2 * (k[1] + r2 * (k[2] + r2 * k[3]));
    let r2 = 50.0f64 * 50.0;
    let before = radial(
        &[1.0, 0.02, -0.01, 0.0, 0.001, -0.002],
        r2 / far2(centre, wide),
    );
    let after = radial(&k[0], r2 / far2(c_new, *b));
    assert!((before - after).abs() < 1e-9, "{before} vs {after}");
    let OpcodeInfo::Point {
        kind:
            PointKind::Vignette {
                k,
                centre: vc,
                inv_m2,
            },
        ..
    } = &l3[1]
    else {
        panic!("{l3:?}")
    };
    let gain = |k: &[f64; 5], r2: f64| 1.0 + r2 * (k[0] + r2 * k[1]);
    let before = gain(&[0.3, -0.1, 0.0, 0.0, 0.0], r2 / far2(centre, wide));
    let after = gain(k, r2 * inv_m2);
    assert!((vc.0 + 10.0 - centre.0).abs() < 1e-9);
    assert!((before - after).abs() < 1e-9, "{before} vs {after}");
    // Opened straight into Develop, the merge has the lists the saved DNG has.
    assert_eq!(m.to_sensor().meta.opcodes, s.meta.opcodes);
}

fn fuji_lens() -> specular_raw::sensor::LensCorrection {
    let knots: Vec<f64> = (1..=9).map(|k| (f64::from(k) / 8.0).sqrt()).collect();
    let last = knots[8];
    specular_raw::sensor::LensCorrection {
        maker: specular_raw::sensor::LensMaker::Fujifilm,
        knots: knots.iter().map(|k| k / last).collect(),
        distortion: Some(vec![
            -1.17, -2.35, -3.46, -4.43, -5.27, -5.92, -6.33, -6.43, -6.43,
        ]),
        vignetting: Some(vec![98.5, 96.9, 94.9, 91.0, 84.7, 78.4, 73.0, 67.5, 67.5]),
        ca_red: Some(vec![
            -3e-5, -9e-5, -1.5e-4, -2.1e-4, -2.7e-4, -3.4e-4, -4.6e-4, -6.1e-4, -6.1e-4,
        ]),
        ca_blue: Some(vec![
            7e-4, 7.9e-4, 8.9e-4, 9.5e-4, 1.04e-3, 1.13e-3, 1.22e-3, 1.43e-3, 1.43e-3,
        ]),
        half_diagonal_px: Some(3749.0),
    }
}

fn sony_lens() -> specular_raw::sensor::LensCorrection {
    specular_raw::sensor::LensCorrection {
        maker: specular_raw::sensor::LensMaker::Sony,
        knots: (0..11).map(|i| f64::from(i) / 10.0).collect(),
        distortion: Some(vec![
            29.0, 29.0, 58.0, 144.0, 259.0, 417.0, 633.0, 891.0, 1222.0, 1639.0, 2156.0,
        ]),
        vignetting: Some(vec![0.0; 11]),
        ca_red: Some(vec![-1280.0; 11]),
        ca_blue: Some(vec![-640.0; 11]),
        half_diagonal_px: None,
    }
}

#[test]
fn maker_lens_tables_fit_dng_opcodes_within_a_twentieth_of_a_pixel_at_the_corners() {
    for (name, lens) in [("Fujifilm", fuji_lens()), ("Sony", sony_lens())] {
        let f = lens_fit::fit_lens(&lens, 6000, 4000).expect("fits");
        eprintln!(
            "{name} 6000x4000: corner {:.4} px, worst {:.3} px, vignetting gain {:.2e}",
            f.corner_px, f.max_px, f.gain_err
        );
        assert!(f.warp.is_some());
        assert!(f.corner_px < 0.05, "{name}: corner {}", f.corner_px);
        assert!(f.max_px < 1.5, "{name}: worst {}", f.max_px);
        // Sony's vignetting is not applied by Develop, so it is not written either.
        assert_eq!(f.vignette.is_some(), name == "Fujifilm");
        assert!(f.gain_err < 0.01, "{name}: gain {}", f.gain_err);
    }
    // Nothing recorded: nothing written.
    let mut none = sony_lens();
    none.distortion = None;
    none.ca_red = None;
    none.ca_blue = None;
    assert_eq!(lens_fit::fit_lens(&none, 6000, 4000), None);
}

#[test]
fn merged_dng_carries_maker_lens_corrections_as_develop_shows_them() {
    use crate::model::params::ModuleParams;
    use crate::model::{Edit, ModuleInstance, ModuleKind, Params, Registry};
    use crate::ops::geometry::{Context, LensParams, Plan};
    use crate::preview::Orient;
    use specular_raw::sensor::{OpcodeInfo, PointKind};
    let id = shift(0.0, 0.0, 0.0);
    let mut edit = Edit::default_for(Registry::builtin(), Default::default(), true);
    let mut mi = ModuleInstance::new(Registry::builtin(), ModuleKind::LENS, 0);
    mi.params = Params::Other(ModuleKind::LENS, LensParams::default().to_fields());
    edit.modules.push(mi);
    edit.sort(Registry::builtin());
    for (name, lens) in [("Fujifilm", fuji_lens()), ("Sony", sony_lens())] {
        let mut m = run(
            vec![
                frame(1.0, &id, None, None, "a"),
                frame(4.0, &id, None, None, "b"),
            ],
            &Options {
                motion: Motion::None,
                ghosts: None,
                ..Options::default()
            },
        );
        m.meta.lens = Some(std::sync::Arc::new(lens.clone()));
        let bytes = dng::encode(&m, &[], &dng::DngOptions::default());
        let f = specular_raw::parse(&bytes).expect("parses");
        let s = f
            .decode_sensor(&bytes, &specular_raw::DecodeOptions::default())
            .expect("decodes");
        let l3 = &s.meta.opcodes[2];
        assert!(matches!(l3[0], OpcodeInfo::Warp { .. }), "{name}: {l3:?}");
        let vig = l3.iter().any(|o| {
            matches!(
                o,
                OpcodeInfo::Point {
                    kind: PointKind::Vignette { .. },
                    ..
                }
            )
        });
        assert_eq!(vig, name == "Fujifilm");
        assert!(s.meta.lens.is_none());
        assert_eq!(m.to_sensor().meta.opcodes, s.meta.opcodes);
        // The reference raw as Develop shows it (maker model) and the merged DNG (opcodes).
        let (w, h) = (m.width, m.height);
        let mut maker = Context::image(w, h, Orient::default());
        maker.lens = Some(std::sync::Arc::new(lens));
        let mut opcodes = Context::image(w, h, Orient::default());
        opcodes.list3 = l3.clone();
        let a = Plan::new(&edit, &maker, true);
        let b = Plan::new(&edit, &opcodes, true);
        let (mut worst, mut corner, mut gain) = (0.0f64, 0.0f64, 0.0f64);
        let n = 16;
        for j in 0..=n {
            for i in 0..=n {
                let o = (
                    f64::from(w) * i as f64 / n as f64,
                    f64::from(h) * j as f64 / n as f64,
                );
                for p in 0..3 {
                    let (pa, ga) = a.to_source(p, o, 1.0);
                    let (pb, gb) = b.to_source(p, o, 1.0);
                    let e = ((pa.0 - pb.0).powi(2) + (pa.1 - pb.1).powi(2)).sqrt();
                    worst = worst.max(e);
                    if (i == 0 || i == n) && (j == 0 || j == n) {
                        corner = corner.max(e);
                    }
                    gain = gain.max(((ga - gb) / ga).abs());
                }
            }
        }
        // In pixels of a 6000 px wide frame.
        let to6k = 6000.0 / f64::from(w);
        eprintln!(
            "{name} merged {w}x{h}: corners {:.4} px, worst {:.3} px (at 6000 px wide), gain {gain:.2e}",
            corner * to6k,
            worst * to6k
        );
        assert!(corner * to6k < 0.05, "{name}: corner {corner}");
        assert!(worst * to6k < 1.5, "{name}: worst {worst}");
        assert!(gain < 0.01, "{name}: gain {gain}");
    }
}

#[test]
fn dng_names_never_clash_and_are_recognised() {
    let p = PathBuf::from("/a/IMG_0002.CR2");
    assert_eq!(dng::file_name(&p), "IMG_0002-HDR.dng");
    assert!(dng::looks_merged(&PathBuf::from("/a/IMG_0002-HDR.dng")));
    assert!(dng::looks_merged(&PathBuf::from("/a/IMG_0002-HDR-3.DNG")));
    assert!(!dng::looks_merged(&PathBuf::from("/a/IMG_0002.dng")));
    assert!(!dng::looks_merged(&PathBuf::from("/a/HDR-x.dng")));
    // Saved twice: two files, the first untouched.
    let dir = std::env::temp_dir().join(format!("specular-merge-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let id = shift(0.0, 0.0, 0.0);
    let mut frames = vec![
        frame(1.0, &id, None, None, "x"),
        frame(4.0, &id, None, None, "y"),
    ];
    for f in &mut frames {
        f.shot.path = dir.join(f.shot.path.file_name().unwrap());
    }
    let m = run(
        frames,
        &Options {
            motion: Motion::None,
            ghosts: None,
            ..Options::default()
        },
    );
    let o = dng::DngOptions::default();
    let a = dng::save(&m, &[], &o).unwrap();
    let first = std::fs::read(&a).unwrap();
    let b = dng::save(&m, &[], &o).unwrap();
    assert_ne!(a, b);
    // The reference (the brighter of two) names it.
    assert_eq!(m.reference, 1);
    assert_eq!(a.file_name().unwrap(), "y-HDR.dng");
    assert_eq!(b.file_name().unwrap(), "y-HDR-2.dng");
    assert_eq!(std::fs::read(&a).unwrap(), first);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn front_end_develops_float_sensors_like_the_cpu_reference() {
    use crate::ops::raw::{FrontEnd, Options as FeOptions, Output, Quality};
    let Some(d) = crate::testgpu::dev() else {
        return;
    };
    let _l = crate::testgpu::lock();
    let id = shift(0.0, 0.0, 0.0);
    let frames = vec![
        frame(0.25, &id, Some(61), None, "a"),
        frame(2.0, &id, Some(62), None, "b"),
    ];
    let m = run(
        frames,
        &Options {
            motion: Motion::None,
            ..Options::default()
        },
    );
    let s = m.to_sensor();
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    for q in [Quality::Half, Quality::Fast] {
        let o = FeOptions {
            precision: crate::graph::Precision::F32,
            output: Output::CameraRgb,
            ..FeOptions::new(q)
        };
        let g = fe.upload(s.clone()).unwrap();
        let out = fe.run(&g, &o).unwrap();
        let gpu = fe.read(&out).unwrap();
        let (_, _, cpu) = crate::ops::raw::cpu_ref::develop(&s, &o);
        assert_eq!(gpu.len(), cpu.len());
        let worst = gpu
            .iter()
            .zip(&cpu)
            .map(|(a, b)| (a - b).abs() / b.abs().max(1.0))
            .fold(0.0f32, f32::max);
        // Highlights above the reference's white survive (unclipped).
        let peak = gpu.chunks_exact(4).map(|p| p[1]).fold(0.0f32, f32::max);
        eprintln!("{q:?}: GPU vs CPU worst {worst:.2e}, peak green {peak:.2}");
        assert!(worst < 1e-4, "{q:?}: {worst}");
        assert!(peak > 4.0, "{q:?}: highlights clipped ({peak})");
    }
}

#[test]
fn brackets_are_detected_by_time_camera_and_steps() {
    let shot = |name: &str, t: f64, exp: f64, auto: bool| Shot {
        path: PathBuf::from(name),
        make: "Nikon".into(),
        model: "Z 6".into(),
        taken: Some(t),
        exposure_time: Some(exp),
        f_number: Some(8.0),
        iso: Some(100.0),
        auto_bracket: auto,
        ..Shot::default()
    };
    let shots = vec![
        // Two brackets of three shot back to back (0, −2, +2 EV), then a lone frame.
        shot("a", 0.0, 1.0 / 125.0, true),
        shot("b", 0.3, 1.0 / 500.0, true),
        shot("c", 0.6, 1.0 / 30.0, true),
        shot("d", 1.0, 1.0 / 125.0, true),
        shot("e", 1.3, 1.0 / 500.0, true),
        shot("f", 1.6, 1.0 / 30.0, true),
        shot("g", 60.0, 1.0 / 125.0, false),
        // Same exposure twice: a burst, not a bracket.
        shot("h", 120.0, 1.0 / 125.0, false),
        shot("i", 120.2, 1.0 / 125.0, false),
        // Manual bracket (no flag), 1 EV steps, a long exposure in it.
        shot("j", 200.0, 1.0, false),
        shot("k", 203.0, 2.0, false),
        shot("l", 207.0, 4.0, false),
    ];
    let sets = bracket_sets(&shots);
    let names: Vec<Vec<&str>> = sets
        .iter()
        .map(|s| {
            s.frames
                .iter()
                .map(|&i| shots[i].path.to_str().unwrap())
                .collect()
        })
        .collect();
    assert_eq!(
        names,
        vec![
            vec!["a", "b", "c"],
            vec!["d", "e", "f"],
            vec!["j", "k", "l"]
        ]
    );
    assert!(sets[0].auto && !sets[2].auto);
    assert!((sets[0].span_ev - (500.0f64 / 30.0).log2()).abs() < 1e-9);
    // Another camera breaks a run.
    let mut other = shots[1].clone();
    other.model = "Z 7".into();
    let sets = bracket_sets(&[shots[0].clone(), other, shots[2].clone()]);
    assert!(sets.is_empty());
    assert_eq!(
        detect::parse_time("2024:05:06 07:08:09"),
        Some(1_714_979_289.0)
    );
}

#[test]
fn refusals_are_plain() {
    let id = shift(0.0, 0.0, 0.0);
    let one = vec![frame(1.0, &id, None, None, "a")];
    let e = merge(one, &Options::default(), &|_| {}, &CancelToken::new()).unwrap_err();
    assert_eq!(e, Error::TooFew);
    let mut two = vec![
        frame(1.0, &id, None, None, "a"),
        frame(2.0, &id, None, None, "b"),
    ];
    two[1].sensor.meta.layout = Layout::Bayer([1, 0, 2, 1]);
    let e = merge(two, &Options::default(), &|_| {}, &CancelToken::new()).unwrap_err();
    assert_eq!(e, Error::NotOneCamera);
    assert_eq!(e.to_string(), "the photos are not from one camera");
    // Cancelled before it starts.
    let c = CancelToken::new();
    c.cancel();
    let two = vec![
        frame(1.0, &id, None, None, "a"),
        frame(2.0, &id, None, None, "b"),
    ];
    assert_eq!(
        merge(two, &Options::default(), &|_| {}, &c).unwrap_err(),
        Error::Cancelled
    );
    // A memory budget too small says how much is needed.
    let two = vec![
        frame(1.0, &id, None, None, "a"),
        frame(2.0, &id, None, None, "b"),
    ];
    let e = specular_core::budget::with_limit(1 << 20, || {
        merge(two, &Options::default(), &|_| {}, &CancelToken::new())
    })
    .unwrap_err();
    assert!(matches!(e, Error::NeedsMemory(_)), "{e:?}");
}

#[test]
fn saved_merge_opens_in_develop_and_keeps_its_edits() {
    use crate::model::{Edit, Registry, SourceKey};
    let dir = std::env::temp_dir().join(format!("specular-merge-rt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let id = shift(0.0, 0.0, 0.0);
    let mut frames = vec![
        frame(0.5, &id, Some(71), None, "s1"),
        frame(2.0, &id, Some(72), None, "s2"),
    ];
    for f in &mut frames {
        f.shot.path = dir.join(f.shot.path.file_name().unwrap());
    }
    let m = run(
        frames,
        &Options {
            motion: Motion::None,
            ..Options::default()
        },
    );
    let path = dng::save(&m, &[], &dng::DngOptions::default()).unwrap();
    // Develop reads it as a float raw at the reference's brightness.
    let bytes = std::fs::read(&path).unwrap();
    let crate::preview::Input::Raw(s) = crate::preview::decode_raw(&bytes).expect("develops")
    else {
        panic!("not a raw input");
    };
    let f = s.float.as_ref().expect("float samples");
    let (x, y) = (200usize, 100usize);
    let got = f[y * W as usize + x] / 65535.0;
    let want = m.radiance[y * W as usize + x];
    assert!((got - want).abs() / want < 2e-5, "{got} vs {want}");
    // Its sidecar round-trips like any photo's.
    let reg = Registry::builtin();
    let edit = Edit::default_for(reg, SourceKey::default(), true);
    crate::sidecar::write(&path, &edit, None).unwrap();
    assert_eq!(
        crate::sidecar::read(&path, reg).unwrap().unwrap().edit,
        edit
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A log-luma proxy of the scene seen through `h` (reference → frame): frame pixel q shows
/// the scene at h⁻¹(q).
fn scene_proxy(w: usize, h: usize, map: &Homography) -> align::Proxy {
    let inv = map.inverse().expect("invertible");
    let v = (0..w * h)
        .map(|k| {
            let (x, y) = inv.apply((k % w) as f64, (k / w) as f64);
            scene(x * 2.0, y * 2.0, 1).log2() as f32
        })
        .collect();
    align::Proxy { w, h, v }
}

#[test]
fn ecc_recovers_a_known_homography_within_five_hundredths_of_a_pixel() {
    let (w, h) = (192, 128);
    let t = scene_proxy(w, h, &Homography::IDENTITY);
    for (motion, truth) in [
        (
            Motion::Homography,
            Homography([1.02, 0.015, -2.5, -0.01, 0.985, 1.75, 6e-5, -4e-5, 1.0]),
        ),
        (
            Motion::Similarity,
            Homography::similarity(1.015, 0.012, 1.3, -0.8, 95.5, 63.5),
        ),
        (
            Motion::Affine,
            Homography([1.01, 0.02, 0.6, -0.015, 0.99, -1.1, 0.0, 0.0, 1.0]),
        ),
    ] {
        let i = scene_proxy(w, h, &truth);
        let found = align::ecc(&t, &i, motion).expect("aligned");
        // Compare away from the borders the model cannot see.
        let d = [
            (20.0, 20.0),
            (170.0, 20.0),
            (20.0, 108.0),
            (170.0, 108.0),
            (96.0, 64.0),
        ]
        .iter()
        .map(|&(x, y)| {
            let (a, b) = (found.h.apply(x, y), truth.apply(x, y));
            (a.0 - b.0).hypot(a.1 - b.1)
        })
        .fold(0.0, f64::max);
        assert!(
            d < 0.05,
            "{motion:?}: off by {d:.4} px (rho {:.5})",
            found.rho
        );
    }
}

#[test]
fn homography_alignment_merges_a_bracket_as_euclidean_does() {
    let id = shift(0.0, 0.0, 0.0);
    let t = shift(-3.4, 2.1, 0.004);
    let frames = || {
        vec![
            frame(0.25, &t, Some(5), None, "a"),
            frame(1.0, &id, Some(6), None, "b"),
            frame(4.0, &shift(1.2, -0.7, -0.002), Some(7), None, "c"),
        ]
    };
    let want = t.homography();
    // Over the interior (the model's corners extrapolate past what both frames show).
    let off = |m: &Merged| {
        (1..8)
            .flat_map(|i| {
                (1..8).map(move |j| (f64::from(W) * i as f64 / 8.0, f64::from(H) * j as f64 / 8.0))
            })
            .map(|(x, y)| {
                let (a, b) = (m.warps[0].apply(x, y), want.apply(x, y));
                (a.0 - b.0).hypot(a.1 - b.1)
            })
            .fold(0.0, f64::max)
    };
    let mut got = Vec::new();
    for motion in [Motion::Euclidean, Motion::Homography] {
        let o = Options {
            motion,
            ghosts: None,
            reference: Some(1),
        };
        got.push(run(frames(), &o));
    }
    let (de, dh) = (off(&got[0]), off(&got[1]));
    eprintln!("bracket: Euclidean off by {de:.3}, homography by {dh:.3} samples inside");
    // A plain turn: the homography adds nothing and the rotation is kept.
    assert!(dh < 0.1, "homography off by {dh:.3} samples");
    let m = &got[1];
    let (mean, _) = block_error(m, &|_, _| false);
    assert!(mean < 0.02, "mean block error {mean:.4}");

    // A hand-held frame seen from a little aside (perspective): only a homography fits.
    let h = Homography([1.006, 0.004, -2.2, -0.003, 0.997, 1.4, 2.5e-5, -1.5e-5, 1.0]);
    let inv = h.inverse().expect("invertible");
    let frames = || {
        vec![
            frame_by(0.25, &|q| inv.apply(q.0, q.1), Some(15), None, "a"),
            frame(1.0, &id, Some(16), None, "b"),
            frame(4.0, &id, Some(17), None, "c"),
        ]
    };
    let mut got = Vec::new();
    for motion in [Motion::Euclidean, Motion::Homography] {
        let o = Options {
            motion,
            ghosts: None,
            reference: Some(1),
        };
        got.push(run(frames(), &o));
    }
    let off = |m: &Merged| {
        (1..8)
            .flat_map(|i| {
                (1..8).map(move |j| (f64::from(W) * i as f64 / 8.0, f64::from(H) * j as f64 / 8.0))
            })
            .map(|(x, y)| {
                let (a, b) = (m.warps[0].apply(x, y), h.apply(x, y));
                (a.0 - b.0).hypot(a.1 - b.1)
            })
            .fold(0.0, f64::max)
    };
    let (de, dh) = (off(&got[0]), off(&got[1]));
    eprintln!("perspective bracket: Euclidean off by {de:.3}, homography by {dh:.3} samples");
    assert!(
        dh < 0.25 && dh < de,
        "homography off by {dh:.3} (Euclidean {de:.3})"
    );
    let m = &got[1];
    let (mean, _) = block_error(m, &|_, _| false);
    assert!(mean < 0.02, "mean block error {mean:.4}");
}
