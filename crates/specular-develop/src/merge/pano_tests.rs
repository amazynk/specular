//! Synthetic panoramas: a textured sphere photographed by a camera turning about its
//! centre through known rotations, mosaiced and with noise; the stitch must find the
//! cameras and give the sphere's radiance back.

use std::path::PathBuf;

use specular_core::CancelToken;
use specular_raw::sensor::{BlackLevels, DngColour, Layout, SensorMeta};
use specular_raw::{MatrixSource, Rect, SensorData, WbSource};

use super::super::bundle::Camera;
use super::super::homography::{mul3, rotation, rotation_distance, transpose3};
use super::*;

const BLACK: f32 = 512.0;
const WHITE: f32 = 15000.0;
const A: f64 = 2.0e-4;
const B: f64 = 4.0e-7;
const W: u32 = 640;
const H: u32 = 448;
const F: f64 = 700.0;

fn hash(a: i64, b: i64, k: u64) -> f64 {
    let mut h = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ k.wrapping_mul(0x1656_67B1_9E37_79F9);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 32;
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// The sphere's radiance in direction `d` (unit), colour `c`: blobs on a longitude-latitude
/// grid over a slow brightness variation.
fn sphere(d: [f64; 3], c: usize) -> f64 {
    let th = d[0].atan2(d[2]);
    let ph = d[1].clamp(-1.0, 1.0).asin();
    let cell = 0.02;
    let (gi, gj) = ((th / cell).floor() as i64, (ph / cell).floor() as i64);
    let mut v = 0.12 * (1.0 + 0.5 * (th * 1.3).sin() * (ph * 2.1 + 0.4).cos());
    for i in gi - 1..=gi + 1 {
        for j in gj - 1..=gj + 1 {
            let bx = (i as f64 + hash(i, j, 1)) * cell;
            let by = (j as f64 + hash(i, j, 2)) * cell;
            let r = 0.0035 + 0.003 * hash(i, j, 3);
            let amp = if hash(i, j, 4) > 0.5 { 0.25 } else { -0.07 };
            let dd = ((th - bx) * ph.cos()).powi(2) + (ph - by).powi(2);
            v += amp * (-dd / (r * r)).exp();
        }
    }
    v.max(0.005) * [0.9, 1.0, 0.8][c]
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

fn camera(yaw: f64, pitch: f64, roll: f64) -> Camera {
    Camera {
        r: mul3(
            &rotation([0.0, 0.0, roll]),
            &mul3(&rotation([pitch, 0.0, 0.0]), &rotation([0.0, yaw, 0.0])),
        ),
        f: F,
        cx: f64::from(W) / 2.0,
        cy: f64::from(H) / 2.0,
        k1: 0.0,
    }
}

/// A frame of the sphere through camera `c` at brightness `gain`, with noise.
fn frame(c: &Camera, gain: f64, seed: u64, name: &str) -> Frame {
    let m = meta();
    let mut rng = seed | 1;
    let mut normal = move || {
        let mut u = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            ((rng >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        };
        let (a, b) = (u(), u());
        (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos()
    };
    let mut samples = Vec::with_capacity((W * H) as usize);
    for y in 0..H {
        for x in 0..W {
            let col = m.layout.color(x, y) as usize;
            let d = c.unproject(f64::from(x), f64::from(y));
            let mut v = gain * sphere(d, col);
            v += (A * v.max(0.0) + B).sqrt() * normal();
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
            model: "EOS R5".into(),
            taken: Some(1000.0),
            exposure_time: Some(0.01),
            f_number: Some(8.0),
            iso: Some(100.0),
            ..Shot::default()
        },
    }
}

fn truth() -> Vec<Camera> {
    vec![
        camera(-0.62, 0.02, 0.004),
        camera(-0.18, -0.015, -0.006),
        camera(0.26, 0.01, 0.002),
        camera(0.70, -0.02, 0.008),
    ]
}

#[test]
fn stitch_recovers_the_cameras_and_radiance_of_a_synthetic_sphere() {
    let cams = truth();
    // Uneven brightness between frames (auto exposure drift) for the gains to fix.
    let gains = [1.0, 1.06, 0.95, 1.03];
    let frames: Vec<Frame> = cams
        .iter()
        .zip(gains)
        .enumerate()
        .map(|(i, (c, g))| frame(c, g, 11 + i as u64, &format!("IMG_{i}")))
        .collect();
    let t = std::time::Instant::now();
    let p = stitch(
        frames,
        &[None; 4],
        &PanoOptions::default(),
        &|_| {},
        &CancelToken::new(),
    )
    .expect("stitched");
    let secs = t.elapsed().as_secs_f64();
    assert!(p.left_out.is_empty());
    let got: Vec<Camera> = p.cameras.iter().map(|c| c.expect("joined")).collect();
    eprintln!(
        "stitch {secs:.2} s; rms {:.4} px; focal {:?}; {}×{} {:?}",
        p.rms,
        got.iter().map(|c| c.f).collect::<Vec<_>>(),
        p.frame.w,
        p.frame.h,
        p.frame.projection
    );
    assert!(p.rms < 0.1, "reprojection rms {:.4} px", p.rms);
    for c in &got {
        assert!((c.f - F).abs() / F < 0.003, "focal {:.2}", c.f);
    }
    for i in 1..cams.len() {
        let want = mul3(&cams[i].r, &transpose3(&cams[0].r));
        let have = mul3(&got[i].r, &transpose3(&got[0].r));
        let d = rotation_distance(&want, &have).to_degrees();
        assert!(d < 0.01, "frame {i}: rotation off by {d:.4}°");
    }
    // About 115° across: cylindrical.
    assert_eq!(p.frame.projection, Projection::Cylindrical);
    // The radiance against the sphere, in the reference's units: block means of green.
    let m = &p.merged;
    let (w, h) = (m.width as usize, m.height as usize);
    // Frame i shows gains[i]·sphere and its gain makes it gains[i]·g_i·sphere ≈ one level.
    let g_ref = gains[m.reference] * m.exposures[m.reference];
    let mut blocks: Vec<(f64, f64)> = Vec::new();
    // The world of the stitch is straightened: directions through the reference camera.
    let to_truth = mul3(&transpose3(&cams[m.reference].r), &got[m.reference].r);
    for by in (16..h - 16).step_by(24) {
        for bx in (16..w - 16).step_by(24) {
            let (mut a, mut b) = (0.0, 0.0);
            for y in by..by + 8 {
                for x in bx..bx + 8 {
                    if m.mosaic.color(x as u32, y as u32) != 1 {
                        continue;
                    }
                    let d = p.frame.dir(x as f64, y as f64);
                    let dt = super::super::homography::mul3v(&to_truth, d);
                    let nrm = (dt[0] * dt[0] + dt[1] * dt[1] + dt[2] * dt[2]).sqrt();
                    a += f64::from(m.radiance[y * w + x]);
                    b += sphere(dt.map(|v| v / nrm), 1) * g_ref;
                }
            }
            blocks.push((a, b));
        }
    }
    // The level is relative (the gains' prior pulls them toward 1): one scale fitted.
    let k = blocks.iter().map(|x| x.0).sum::<f64>() / blocks.iter().map(|x| x.1).sum::<f64>();
    let mut errs: Vec<f64> = blocks
        .iter()
        .map(|(a, b)| (a - k * b).abs() / (k * b))
        .collect();
    errs.sort_by(f64::total_cmp);
    let mean = errs.iter().sum::<f64>() / errs.len() as f64;
    eprintln!(
        "level {k:.4}; block error mean {mean:.4}, median {:.4}, 95th {:.4} over {} blocks",
        errs[errs.len() / 2],
        errs[errs.len() * 95 / 100],
        errs.len()
    );
    assert!((k - 1.0).abs() < 0.03, "level {k:.4}");
    assert!(mean < 0.02, "mean relative block error {mean:.4}");
}

#[test]
fn a_frame_that_overlaps_nothing_is_left_out_by_name() {
    let mut cams = truth();
    cams.truncate(2);
    // Looking the other way.
    cams.push(camera(3.0, 0.0, 0.0));
    let frames: Vec<Frame> = cams
        .iter()
        .enumerate()
        .map(|(i, c)| frame(c, 1.0, 40 + i as u64, &format!("IMG_{i}")))
        .collect();
    let p = stitch(
        frames,
        &[None; 3],
        &PanoOptions::default(),
        &|_| {},
        &CancelToken::new(),
    )
    .expect("stitched");
    assert_eq!(p.left_out, vec![PathBuf::from("/photos/IMG_2.CR2")]);
    assert!(p.cameras[2].is_none());
    assert_eq!(p.frame.projection, Projection::Rectilinear);
}

#[test]
fn too_many_frames_or_none_overlapping_are_refused() {
    let f = |i: u64, yaw: f64| frame(&camera(yaw, 0.0, 0.0), 1.0, i, &format!("x{i}"));
    let r = stitch(
        vec![f(1, 0.0), f(2, 3.0)],
        &[None; 2],
        &PanoOptions::default(),
        &|_| {},
        &CancelToken::new(),
    );
    assert!(matches!(r, Err(Error::NoOverlap)), "{:?}", r.err());
    let many: Vec<Frame> = (0..=MAX_FRAMES as u64)
        .map(|_| Frame {
            sensor: SensorData {
                width: 4,
                height: 4,
                cpp: 1,
                samples: vec![0; 16],
                float: None,
                crop: Rect {
                    x: 0,
                    y: 0,
                    w: 4,
                    h: 4,
                },
                meta: meta(),
            },
            shot: Shot::default(),
        })
        .collect();
    let r = stitch(
        many,
        &[],
        &PanoOptions::default(),
        &|_| {},
        &CancelToken::new(),
    );
    assert!(matches!(r, Err(Error::Cap(MAX_FRAMES))));
}

#[test]
fn streamed_panorama_dng_equals_the_in_memory_file_and_reads_back() {
    let dir = std::env::temp_dir().join(format!("specular-pano-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cams = truth();
    let mut frames: Vec<Frame> = cams[..2]
        .iter()
        .enumerate()
        .map(|(i, c)| frame(c, 1.0, 70 + i as u64, &format!("P_{i}")))
        .collect();
    for f in &mut frames {
        f.shot.path = dir.join(f.shot.path.file_name().unwrap());
    }
    let p = stitch(
        frames,
        &[None; 2],
        &PanoOptions::default(),
        &|_| {},
        &CancelToken::new(),
    )
    .expect("stitched");
    // Tiles more than one row and column of 256² tiles.
    assert!(p.merged.width > 512 && p.merged.height > 256);
    let o = super::super::dng::DngOptions::default();
    let mem = super::super::dng::encode(&p.merged, &[], &o);
    let path =
        super::super::dng::save_streamed(&p.merged, &[], &o, super::super::dng::Kind::Pano, None)
            .unwrap();
    assert!(
        path.file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with("-Pano.dng")
    );
    let disk = std::fs::read(&path).unwrap();
    assert!(disk == mem, "streamed and in-memory files differ");
    // Our own decoder reads the samples back (24-bit floats: about 1e-4 relative).
    let f = specular_raw::parse(&disk).expect("parsed");
    let s = f
        .decode_sensor(&disk, &specular_raw::DecodeOptions::default())
        .expect("decoded");
    let s = super::super::apply_baseline(s);
    assert_eq!((s.width, s.height), (p.merged.width, p.merged.height));
    let fl = s.float.as_ref().expect("float samples");
    let worst = fl
        .iter()
        .zip(&p.merged.radiance)
        .filter(|(_, b)| **b > 1e-3)
        .map(|(a, b)| (f64::from(*a) / 65535.0 - f64::from(*b)).abs() / f64::from(*b))
        .fold(0.0f64, f64::max);
    assert!(worst < 1e-4, "read back off by {worst:e} (relative)");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The OpenCV stitcher (tools/corpus/pano-oracle.py, outputs only) on the same sphere:
/// its focal lengths and turns between neighbours against the truth and ours.
#[test]
#[ignore = "needs the OpenCV oracle venv (tools/corpus/pano-oracle.py)"]
fn oracle_opencv_stitcher_agrees_on_the_cameras() {
    let py = std::env::var("SPECULAR_PANO_ORACLE_PY")
        .map(PathBuf::from)
        .ok()
        .or_else(|| {
            let p =
                PathBuf::from(std::env::var("CARGO_TARGET_DIR").ok()?).join("venv-pano/bin/python");
            p.exists().then_some(p)
        })
        .expect("set SPECULAR_PANO_ORACLE_PY or make the venv");
    let dir = std::env::temp_dir().join(format!("specular-pano-oracle-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let cams = truth();
    // The frames as 8-bit sRGB pictures for OpenCV.
    let mut files = Vec::new();
    for (i, c) in cams.iter().enumerate() {
        let mut b = Vec::with_capacity(8 + (W * H * 3) as usize);
        b.extend_from_slice(&W.to_le_bytes());
        b.extend_from_slice(&H.to_le_bytes());
        for y in 0..H {
            for x in 0..W {
                let d = c.unproject(f64::from(x), f64::from(y));
                for ch in 0..3 {
                    let v = (sphere(d, ch) * 2.5).clamp(0.0, 1.0);
                    let e = if v <= 0.003_130_8 {
                        12.92 * v
                    } else {
                        1.055 * v.powf(1.0 / 2.4) - 0.055
                    };
                    b.push((e * 255.0).round() as u8);
                }
            }
        }
        let p = dir.join(format!("f{i}.rgb8"));
        std::fs::write(&p, b).unwrap();
        files.push(p.display().to_string());
    }
    let json = dir.join("stitch.json");
    let script =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/pano-oracle.py");
    let out = std::process::Command::new(py)
        .arg("-I")
        .arg(script)
        .arg("stitch")
        .arg(&json)
        .args(&files)
        .output()
        .expect("oracle ran");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&json).unwrap()).unwrap();
    assert_eq!(v["status"], 0, "OpenCV stitcher status {}", v["status"]);
    let cv: Vec<([f64; 9], f64)> = v["cameras"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let r: Vec<f64> = c["R"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|row| row.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()))
                .collect();
            (r.try_into().unwrap(), c["focal"].as_f64().unwrap())
        })
        .collect();
    let idx: Vec<usize> = v["indices"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_u64().unwrap() as usize)
        .collect();
    // Ours on the raw frames of the same cameras.
    let frames: Vec<Frame> = cams
        .iter()
        .enumerate()
        .map(|(i, c)| frame(c, 1.0, 90 + i as u64, &format!("O_{i}")))
        .collect();
    let p = stitch(
        frames,
        &[None; 4],
        &PanoOptions::default(),
        &|_| {},
        &CancelToken::new(),
    )
    .expect("stitched");
    let ours: Vec<Camera> = p.cameras.iter().map(|c| c.expect("joined")).collect();
    let angle = |a: &[f64; 9], b: &[f64; 9]| rotation_distance(a, b).to_degrees();
    // OpenCV's focal is at its registration scale: compare it as a ratio to the long side
    // of its first camera's focal against the truth's.
    let scale = cv[0].1 / F;
    let mut worst_cv: f64 = 0.0;
    let mut worst_ours: f64 = 0.0;
    for k in 1..idx.len() {
        let (a, b) = (idx[k - 1], idx[k]);
        let t = angle(&cams[a].r, &cams[b].r);
        let c = angle(&cv[k - 1].0, &cv[k].0);
        let o = angle(&ours[a].r, &ours[b].r);
        eprintln!("turn {a}→{b}: truth {t:.4}°, OpenCV {c:.4}°, ours {o:.4}°");
        worst_cv = worst_cv.max((c - t).abs());
        worst_ours = worst_ours.max((o - t).abs());
    }
    for (k, &i) in idx.iter().enumerate() {
        eprintln!(
            "focal {i}: truth {F:.1}, OpenCV {:.1} (at its scale {:.3}: {:.1}), ours {:.2}",
            cv[k].1,
            scale,
            cv[k].1 / scale,
            ours[i].f
        );
    }
    eprintln!("worst turn error: OpenCV {worst_cv:.4}°, ours {worst_ours:.4}°");
    assert_eq!(idx.len(), 4, "OpenCV kept {idx:?}");
    assert!(
        worst_ours <= worst_cv.max(0.01),
        "ours {worst_ours:.4}° vs OpenCV {worst_cv:.4}°"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn known_focal_lengths_use_the_rotation_model() {
    let cams = truth();
    let frames: Vec<Frame> = cams[..3]
        .iter()
        .enumerate()
        .map(|(i, c)| frame(c, 1.0, 120 + i as u64, &format!("K_{i}")))
        .collect();
    let p = stitch(
        frames,
        &[Some(F); 3],
        &PanoOptions::default(),
        &|_| {},
        &CancelToken::new(),
    )
    .expect("stitched");
    let got: Vec<Camera> = p.cameras.iter().map(|c| c.expect("joined")).collect();
    assert!(p.rms < 0.1, "rms {:.4}", p.rms);
    for i in 1..3 {
        let want = mul3(&cams[i].r, &transpose3(&cams[0].r));
        let have = mul3(&got[i].r, &transpose3(&got[0].r));
        let d = rotation_distance(&want, &have).to_degrees();
        assert!(d < 0.01, "frame {i}: {d:.4}°");
    }
}
