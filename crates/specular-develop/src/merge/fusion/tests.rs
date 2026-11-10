//! Synthetic brackets of developed pictures: a known radiance map, a known camera response
//! (a 2.2 power, 8-bit), shifts and a rotation; and the OpenCV oracle (ignored by default).

use std::path::PathBuf;

use super::*;

const W: u32 = 320;
const H: u32 = 240;
const TIMES: [f64; 3] = [0.25, 1.0, 4.0];

/// Scene radiance at (x, y), channel c: about 0.004 … 5.
fn scene(x: f64, y: f64, c: usize) -> f64 {
    let (u, v) = (x / 30.0, y / 30.0);
    let base = 0.05 * (1.0 + 0.6 * (u * 1.7).sin() * (v * 1.3).cos());
    let blob = |cx: f64, cy: f64, r: f64, k: f64| {
        k * (-((x - cx).powi(2) + (y - cy).powi(2)) / (r * r)).exp()
    };
    let bright = blob(220.0, 70.0, 28.0, 4.5) + blob(80.0, 160.0, 20.0, 0.8);
    let ramp = 0.004 + 0.25 * (x / 320.0).powi(3);
    let edges = if ((x / 17.0).floor() as i64 + (y / 23.0).floor() as i64) % 2 == 0 {
        1.0
    } else {
        0.7
    };
    // A band along the bottom runs through every exposure (2⁻¹¹ … 2³).
    // Fine texture: exposure fusion weighs by local contrast, which a smooth scene lacks.
    let v = if y >= 200.0 {
        (2f64).powf(-11.0 + 14.0 * x / 320.0)
    } else {
        let tex = 1.0 + 0.12 * (x * 1.37 + 0.5 * c as f64).sin() * (y * 1.11).sin();
        (base * edges + bright + ramp) * tex
    };
    v * [0.85, 1.0, 0.75][c]
}

/// The camera's code for exposure `e` (the known response: a 2.2 power, 8 bits).
fn camera(e: f64) -> f32 {
    let c = e.clamp(0.0, 1.0).powf(1.0 / 2.2);
    ((c * 255.0).round() / 255.0) as f32
}

/// The known response's linear value of code `z`, 1 at 128.
fn truth(z: usize) -> f64 {
    (z as f64 / 128.0).powf(2.2)
}

/// A frame of exposure time `t` seen through `map` (reference → frame).
fn frame(t: f64, map: &Homography) -> CodeImage {
    let inv = map.inverse().expect("invertible");
    let mut rgb = Vec::with_capacity((W * H * 3) as usize);
    for y in 0..H {
        for x in 0..W {
            let (px, py) = inv.apply(f64::from(x), f64::from(y));
            for c in 0..3 {
                rgb.push(camera(scene(px, py, c) * t));
            }
        }
    }
    CodeImage { w: W, h: H, rgb }
}

fn shot(name: &str, t: f64) -> Shot {
    Shot {
        path: PathBuf::from(format!("/photos/{name}.jpg")),
        exposure_time: Some(t),
        f_number: Some(8.0),
        iso: Some(100.0),
        ..Shot::default()
    }
}

fn aligned() -> Vec<CodeImage> {
    TIMES
        .iter()
        .map(|&t| frame(t, &Homography::IDENTITY))
        .collect()
}

#[test]
fn mtb_finds_an_integer_shift_across_exposures() {
    let a = grey_reduced(&frame(1.0, &Homography::IDENTITY), 1);
    let b = grey_reduced(&frame(4.0, &Homography::translation(7.0, -5.0)), 1);
    assert_eq!(mtb_shift(&a, &b, 6), (7, -5));
}

#[test]
fn bracket_alignment_recovers_shift_and_rotation() {
    let maps = [
        Homography::similarity(1.0, 0.006, 4.3, -2.6, 159.5, 119.5),
        Homography::IDENTITY,
        Homography::similarity(1.0, -0.004, -3.1, 5.4, 159.5, 119.5),
    ];
    let frames: Vec<(CodeImage, Shot)> = TIMES
        .iter()
        .zip(&maps)
        .enumerate()
        .map(|(i, (&t, m))| (frame(t, m), shot(&format!("f{i}"), t)))
        .collect();
    let f = fuse(
        frames,
        &FusionOptions::default(),
        &|_| {},
        &CancelToken::new(),
    )
    .expect("fused");
    assert_eq!(f.reference, 1);
    for (got, want) in f.warps.iter().zip(&maps) {
        // Away from the corners the clipped frames cannot see well.
        let d = [
            (40.0, 40.0),
            (280.0, 40.0),
            (40.0, 200.0),
            (280.0, 200.0),
            (160.0, 120.0),
        ]
        .iter()
        .map(|&(x, y)| {
            let (a, b) = (got.apply(x, y), want.apply(x, y));
            (a.0 - b.0).hypot(a.1 - b.1)
        })
        .fold(0.0, f64::max);
        eprintln!("warp {:?} want {:?}: {d:.3}", got.0, want.0);
        assert!(d < 0.3, "off by {d:.3} px");
    }
}

#[test]
fn mertens_of_identical_frames_is_the_frame() {
    let a = frame(1.0, &Homography::IDENTITY);
    let out = mertens(
        &[a.clone(), a.clone(), a.clone()],
        [1.0; 3],
        &CancelToken::new(),
    )
    .expect("fused");
    let err = a
        .rgb
        .iter()
        .zip(&out.rgb)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0f32, f32::max);
    assert!(err < 1e-5, "{err:e}");
}

#[test]
fn mertens_keeps_highlights_and_shadows() {
    let f = aligned();
    let out = mertens(&f, [1.0; 3], &CancelToken::new()).expect("fused");
    let clipped = |img: &CodeImage| img.rgb.iter().filter(|&&v| v >= 0.99).count();
    let crushed = |img: &CodeImage| img.rgb.iter().filter(|&&v| v <= 0.02).count();
    eprintln!(
        "clipped {} (frames {} {} {}), crushed {} (frames {} {} {})",
        clipped(&out),
        clipped(&f[0]),
        clipped(&f[1]),
        clipped(&f[2]),
        crushed(&out),
        crushed(&f[0]),
        crushed(&f[1]),
        crushed(&f[2])
    );
    assert!(clipped(&out) < clipped(&f[1]), "clipped {}", clipped(&out));
    assert!(crushed(&out) <= crushed(&f[1]), "crushed {}", crushed(&out));
    // Blended bands overshoot a little at strong edges (OpenCV's too, up to about 1.3).
    let (lo, hi) = out
        .rgb
        .iter()
        .fold((f32::MAX, f32::MIN), |(a, b), &v| (a.min(v), b.max(v)));
    assert!(lo > -0.5 && hi < 1.5, "range {lo} … {hi}");
}

fn curve_error(c: &Curves, lo: usize, hi: usize) -> f64 {
    let (mut worst, mut at) = (0.0, (0, 0));
    for ch in 0..3 {
        for z in lo..=hi {
            let e = (f64::from(c[ch][z]).ln() - truth(z).ln()).abs();
            if e > worst {
                worst = e;
                at = (ch, z);
            }
        }
    }
    let row: Vec<String> = [8usize, 16, 32, 64, 128, 192, 240, 250]
        .iter()
        .map(|&z| format!("{z}:{:.4}/{:.4}", c[1][z], truth(z)))
        .collect();
    eprintln!("worst {worst:.4} at {at:?}; green {}", row.join(" "));
    worst
}

#[test]
fn debevec_recovers_the_known_response() {
    let c = debevec(&aligned(), &TIMES, 150, 10.0);
    // The codes next to clipping (0 and 255) are poorly fixed by any method.
    let e = curve_error(&c, 16, 240);
    assert!(e < 0.06, "ln error {e:.4}");
}

#[test]
fn robertson_recovers_the_known_response() {
    let c = robertson(&aligned(), &TIMES, 30, 1e-4, 1);
    // The estimator's fixed point is biased by the 8-bit quantisation (about ±0.17 in ln
    // around code 190 on this scene); the test bounds that.
    let e = curve_error(&c, 16, 240);
    assert!(e < 0.25, "ln error {e:.4}");
}

#[test]
fn radiance_matches_the_scene() {
    let frames: Vec<(CodeImage, Shot)> = aligned()
        .into_iter()
        .zip(TIMES)
        .enumerate()
        .map(|(i, (f, t))| (f, shot(&format!("f{i}"), t)))
        .collect();
    let o = FusionOptions {
        motion: Motion::None,
        radiance: Some(Response::default()),
        ..FusionOptions::default()
    };
    let f = fuse(frames, &o, &|_| {}, &CancelToken::new()).expect("fused");
    let r = f.radiance.expect("radiance");
    // Back to sRGB primaries, compared on a grid (exposure 1 = the reference's).
    let k = crate::export::Space::Srgb.from_rec2020();
    // A known scale: code 128 ↔ exposure (128/255)^2.2.
    let scale = (128.0f64 / 255.0).powf(2.2);
    let (short, long) = (
        frame(TIMES[0], &Homography::IDENTITY),
        frame(TIMES[2], &Homography::IDENTITY),
    );
    let mut worst = 0.0f64;
    for y in (8..H - 8).step_by(13) {
        for x in (8..W - 8).step_by(11) {
            let i = ((y * W + x) * 3) as usize;
            // Clipped or crushed in every frame: only a bound is known there.
            if short.rgb[i..i + 3].iter().any(|&v| v >= 0.92)
                || long.rgb[i..i + 3].iter().any(|&v| v <= 0.1)
            {
                continue;
            }
            let p = &r.linear[i..i + 3];
            for c in 0..3 {
                let v: f64 = (0..3).map(|j| k[c][j] * f64::from(p[j])).sum();
                let want = scene(f64::from(x), f64::from(y), c) / scale;
                worst = worst.max((v / want).ln().abs());
            }
        }
    }
    // Bright pixels rest on the top of the recovered curve (about 0.04 off there).
    assert!(worst < 0.15, "ln error {worst:.4}");
}

#[test]
fn too_many_or_mismatched_frames_are_refused() {
    let a = frame(1.0, &Homography::IDENTITY);
    let one = vec![(a.clone(), shot("a", 1.0))];
    let o = FusionOptions::default();
    let c = CancelToken::new();
    assert!(matches!(fuse(one, &o, &|_| {}, &c), Err(Error::TooFew)));
    let many: Vec<_> = (0..10)
        .map(|i| (a.clone(), shot(&format!("{i}"), 1.0)))
        .collect();
    assert!(matches!(fuse(many, &o, &|_| {}, &c), Err(Error::TooMany)));
    let small = CodeImage {
        w: 10,
        h: 10,
        rgb: vec![0.5; 300],
    };
    let mixed = vec![(a, shot("a", 1.0)), (small, shot("b", 2.0))];
    assert!(matches!(
        fuse(mixed, &o, &|_| {}, &c),
        Err(Error::NotOneCamera)
    ));
}

// The OpenCV oracle (tools/corpus/pano-oracle.py): outputs only, ignored by default.

fn oracle_python() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("SPECULAR_PANO_ORACLE_PY") {
        return Some(PathBuf::from(p));
    }
    let t = std::env::var("CARGO_TARGET_DIR").ok()?;
    let p = PathBuf::from(t).join("venv-pano/bin/python");
    p.exists().then_some(p)
}

fn oracle_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "specular-fusion-oracle-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("dir");
    d
}

fn write_rgb8(path: &std::path::Path, img: &CodeImage) {
    let mut b = Vec::with_capacity(8 + img.rgb.len());
    b.extend_from_slice(&img.w.to_le_bytes());
    b.extend_from_slice(&img.h.to_le_bytes());
    b.extend(img.rgb.iter().map(|&v| code(v) as u8));
    std::fs::write(path, b).expect("written");
}

fn read_f32(path: &std::path::Path) -> (u32, u32, Vec<f32>) {
    let b = std::fs::read(path).expect("oracle output");
    let w = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    let h = u32::from_le_bytes([b[4], b[5], b[6], b[7]]);
    let v = b[8..]
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    (w, h, v)
}

fn run_oracle(args: &[String]) {
    let py = oracle_python().expect("set SPECULAR_PANO_ORACLE_PY or make the venv");
    let script =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/pano-oracle.py");
    let out = std::process::Command::new(py)
        .arg("-I")
        .arg(script)
        .args(args)
        .output()
        .expect("oracle ran");
    assert!(
        out.status.success(),
        "oracle failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn prepare(name: &str) -> (PathBuf, Vec<CodeImage>, Vec<String>) {
    let d = oracle_dir(name);
    let frames = aligned();
    let mut files = Vec::new();
    for (i, f) in frames.iter().enumerate() {
        let p = d.join(format!("f{i}.rgb8"));
        write_rgb8(&p, f);
        files.push(p.display().to_string());
    }
    (d, frames, files)
}

/// Samples beyond `tol` and the largest difference.
fn mismatches(a: &[f32], b: &[f32], tol: f32) -> (usize, f32) {
    let mut n = 0;
    let mut worst = 0.0f32;
    for (x, y) in a.iter().zip(b) {
        let d = (x - y).abs();
        worst = worst.max(d);
        if d > tol {
            n += 1;
        }
    }
    (n, worst)
}

#[test]
#[ignore = "needs the OpenCV oracle venv (tools/corpus/pano-oracle.py)"]
fn oracle_mertens() {
    let (d, frames, files) = prepare("mertens");
    let t = std::time::Instant::now();
    let ours = mertens(&frames, [1.0; 3], &CancelToken::new()).expect("fused");
    let ms = t.elapsed().as_secs_f64() * 1e3;
    // Pixels whose weights are not all negligible (elsewhere float rounding decides).
    let sum: Vec<f32> = frames.iter().map(|f| weights_of(f, [1.0; 3])).fold(
        vec![0.0; (W * H) as usize],
        |mut a, w| {
            a.iter_mut().zip(w).for_each(|(x, y)| *x += y);
            a
        },
    );
    let pick = |inner: bool, weighted: bool| -> Vec<usize> {
        (0..(W * H * 3) as usize)
            .filter(|&i| {
                let p = i / 3;
                let (x, y) = (p as u32 % W, p as u32 / W);
                (!inner || ((4..W - 4).contains(&x) && (4..H - 4).contains(&y)))
                    && (!weighted || sum[p] > 1e-4)
            })
            .collect()
    };
    let mut worst_inner = 0.0f32;
    for (cmd, label) in [
        ("mertens", "MergeMertens"),
        ("mertens-paper", "cv2-primitive paper method"),
    ] {
        let out = d.join(format!("{cmd}.f32"));
        let mut args = vec![cmd.to_string(), out.display().to_string()];
        args.extend(files.iter().cloned());
        run_oracle(&args);
        let (w, h, theirs) = read_f32(&out);
        assert_eq!((w, h), (W, H));
        for (inner, weighted, what) in [
            (false, false, "all"),
            (true, false, "inside 4 px"),
            (true, true, "inside, weighted"),
        ] {
            let idx = pick(inner, weighted);
            let (n, worst) = mismatches(
                &idx.iter().map(|&i| ours.rgb[i]).collect::<Vec<_>>(),
                &idx.iter().map(|&i| theirs[i]).collect::<Vec<_>>(),
                1e-3,
            );
            eprintln!(
                "mertens vs {label}, {what}: {n} of {} beyond 1e-3 (max {worst:.2e})",
                idx.len()
            );
            if cmd == "mertens-paper" && inner && weighted {
                worst_inner = worst;
            }
        }
    }
    eprintln!("ours {ms:.1} ms for 3 × {W}×{H}");
    let _ = std::fs::remove_dir_all(&d);
    assert!(worst_inner < 0.05, "max {worst_inner}");
}

fn compare_curves(name: &str, ours: &Curves, theirs: &[f32]) -> (usize, f32) {
    // OpenCV: 256 × 3, BGR (the script turns it to RGB); both normalised at code 128.
    let mut n = 0;
    let mut worst = 0.0f32;
    for c in 0..3 {
        let (a0, b0) = (ours[c][128], theirs[128 * 3 + c]);
        for z in 0..256 {
            let (a, b) = (ours[c][z] / a0, theirs[z * 3 + c] / b0);
            let rel = (a - b).abs() / b.abs().max(1e-6);
            worst = worst.max(rel);
            if rel > 1e-3 {
                n += 1;
            }
        }
    }
    // Codes 16…240 (the ends are fixed by little data), and both against the known truth.
    let (mut nm, mut wm, mut to, mut tt) = (0, 0.0f32, 0.0f64, 0.0f64);
    for c in 0..3 {
        let (a0, b0) = (ours[c][128], theirs[128 * 3 + c]);
        for z in 16..=240 {
            let (a, b) = (ours[c][z] / a0, theirs[z * 3 + c] / b0);
            let rel = (a - b).abs() / b.abs().max(1e-6);
            wm = wm.max(rel);
            if rel > 1e-3 {
                nm += 1;
            }
            to = to.max((f64::from(a).ln() - truth(z).ln()).abs());
            tt = tt.max((f64::from(b).ln() - truth(z).ln()).abs());
        }
    }
    eprintln!(
        "{name}: {n} of 768 response values beyond 1e-3 relative (max {worst:.2e}); codes 16–240: \
         {nm} of 675 (max {wm:.2e}); ln error to the truth: ours {to:.4}, OpenCV {tt:.4}"
    );
    (n, worst)
}

#[test]
#[ignore = "needs the OpenCV oracle venv (tools/corpus/pano-oracle.py)"]
fn oracle_debevec() {
    let (d, frames, files) = prepare("debevec");
    let out = d.join("debevec.f32");
    let mut args = vec![
        "debevec".to_string(),
        out.display().to_string(),
        TIMES.map(|t| t.to_string()).join(","),
    ];
    args.extend(files);
    run_oracle(&args);
    let (_, _, theirs) = read_f32(&out);
    let ours = debevec(&frames, &TIMES, 70, 10.0);
    let (_, worst) = compare_curves("debevec", &ours, &theirs);
    let _ = std::fs::remove_dir_all(&d);
    assert!(worst.is_finite());
}

#[test]
#[ignore = "needs the OpenCV oracle venv (tools/corpus/pano-oracle.py)"]
fn oracle_robertson() {
    let (d, frames, files) = prepare("robertson");
    let out = d.join("robertson.f32");
    let mut args = vec![
        "robertson".to_string(),
        out.display().to_string(),
        TIMES.map(|t| t.to_string()).join(","),
    ];
    args.extend(files);
    run_oracle(&args);
    let (_, _, theirs) = read_f32(&out);
    let ours = robertson(&frames, &TIMES, 30, 0.01, 1);
    let (_, worst) = compare_curves("robertson", &ours, &theirs);
    let _ = std::fs::remove_dir_all(&d);
    assert!(worst.is_finite());
}
