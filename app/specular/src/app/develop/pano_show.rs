//! `--show pano-timings` (with `--screenshot`): stitches six synthetic 24 MP raws (a
//! textured sphere seen by a camera turning about its centre, Bayer, with noise) before the
//! shot and prints how long each step took to stderr. For measuring, not for users.

use std::path::PathBuf;
use std::time::Instant;

use rayon::prelude::*;
use specular_core::CancelToken;
use specular_develop::merge::bundle::Camera;
use specular_develop::merge::homography::{mul3, rotation};
use specular_develop::merge::{Frame, Shot, dng, pano};
use specular_raw::sensor::{BlackLevels, DngColour, Layout, SensorMeta};
use specular_raw::{MatrixSource, Rect, SensorData, WbSource};

use crate::app::App;

const W: u32 = 6000;
const H: u32 = 4000;
const F: f64 = 6200.0;
const BLACK: f32 = 512.0;
const WHITE: f32 = 15000.0;

fn hash(a: i64, b: i64, k: u64) -> f64 {
    let mut h = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ k.wrapping_mul(0x1656_67B1_9E37_79F9);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 32;
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// The sphere's radiance in direction `d`: blobs on a longitude-latitude grid.
fn sphere(d: [f64; 3]) -> f64 {
    let th = d[0].atan2(d[2]);
    let ph = d[1].clamp(-1.0, 1.0).asin();
    let cell = 0.004;
    let (gi, gj) = ((th / cell).floor() as i64, (ph / cell).floor() as i64);
    let mut v = 0.12 * (1.0 + 0.5 * (th * 1.3).sin() * (ph * 2.1 + 0.4).cos());
    for i in gi - 1..=gi + 1 {
        for j in gj - 1..=gj + 1 {
            let bx = (i as f64 + hash(i, j, 1)) * cell;
            let by = (j as f64 + hash(i, j, 2)) * cell;
            let r = 0.0007 + 0.0006 * hash(i, j, 3);
            let amp = if hash(i, j, 4) > 0.5 { 0.25 } else { -0.07 };
            let dd = ((th - bx) * ph.cos()).powi(2) + (ph - by).powi(2);
            v += amp * (-dd / (r * r)).exp();
        }
    }
    v.max(0.005)
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
            noise_profile: vec![[2.0e-4, 4.0e-7]],
            ..Default::default()
        }),
        profile: None,
        orientation: 1,
        truncated: false,
        lens: None,
        default_scale: None,
    }
}

fn frame(i: usize, dir: &std::path::Path) -> Frame {
    let yaw = -0.5 + 0.2 * i as f64;
    let c = Camera {
        r: mul3(
            &rotation([0.01 * (i as f64).sin(), 0.0, 0.0]),
            &rotation([0.0, yaw, 0.0]),
        ),
        f: F,
        cx: f64::from(W) / 2.0,
        cy: f64::from(H) / 2.0,
        k1: 0.0,
    };
    let tint = [0.9, 1.0, 0.8];
    let pat = [0usize, 1, 1, 2];
    let samples: Vec<u16> = (0..H)
        .into_par_iter()
        .flat_map_iter(|y| {
            (0..W).map(move |x| {
                let col = pat[((y & 1) * 2 + (x & 1)) as usize];
                let v = sphere(c.unproject(f64::from(x), f64::from(y))) * tint[col];
                // A little deterministic noise.
                let n = (hash(i64::from(x), i64::from(y), i as u64) - 0.5) * 0.004;
                let raw = f64::from(BLACK) + (v + n) * f64::from(WHITE - BLACK);
                raw.round().clamp(0.0, f64::from(WHITE)) as u16
            })
        })
        .collect();
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
            meta: meta(),
        },
        shot: Shot {
            path: dir.join(format!("SYN_{i:04}.DNG")),
            make: "Specular".into(),
            model: "Synthetic".into(),
            taken: Some(1000.0 + i as f64),
            exposure_time: Some(0.01),
            f_number: Some(8.0),
            iso: Some(100.0),
            ..Shot::default()
        },
    }
}

impl App {
    /// `--show pano-timings` (see the module documentation); false for other routes.
    pub(in crate::app) fn develop_pano_show(&mut self, what: &str) -> bool {
        if what != "pano-timings" {
            return false;
        }
        // Screenshot mode applies `--show` and exits after the shot, so the stitch runs here,
        // before it.
        {
            let dir =
                std::env::temp_dir().join(format!("specular-pano-show-{}", std::process::id()));
            let _ = std::fs::create_dir_all(&dir);
            let t0 = Instant::now();
            let frames: Vec<Frame> = (0..6).map(|i| frame(i, &dir)).collect();
            let made = t0.elapsed().as_secs_f64();
            let t1 = Instant::now();
            let steps = std::sync::Mutex::new(Vec::<(f32, f64)>::new());
            let r = pano::stitch(
                frames,
                &[None; 6],
                &pano::PanoOptions::default(),
                &|f| {
                    if let Ok(mut s) = steps.lock() {
                        s.push((f, t1.elapsed().as_secs_f64()));
                    }
                },
                &CancelToken::new(),
            );
            let stitched = t1.elapsed().as_secs_f64();
            match r {
                Ok(p) => {
                    let t2 = Instant::now();
                    let saved = dng::save_streamed(
                        &p.merged,
                        &[],
                        &dng::DngOptions::default(),
                        dng::Kind::Pano,
                        Some(pano::xmp(&p)),
                    );
                    let write = t2.elapsed().as_secs_f64();
                    if let Ok(s) = steps.lock() {
                        // The time each tenth of the progress was reached.
                        let mut next = 0.1f32;
                        for &(f, t) in s.iter() {
                            if f >= next {
                                eprintln!("pano-timings:   {:>3.0} % at {t:.2} s", f * 100.0);
                                next = (f / 0.1).floor() * 0.1 + 0.1;
                            }
                        }
                    }
                    eprintln!(
                        "pano-timings: 6×24 MP frames made in {made:.2} s; stitched in {stitched:.2} s \
                         ({}×{}, {:?}, rms {:.3} px); written in {write:.2} s ({:?})",
                        p.merged.width,
                        p.merged.height,
                        p.frame.projection,
                        p.rms,
                        saved
                            .as_ref()
                            .map(PathBuf::as_path)
                            .map_err(ToString::to_string),
                    );
                }
                Err(e) => eprintln!("pano-timings: failed after {stitched:.2} s: {e}"),
            }
            let _ = std::fs::remove_dir_all(&dir);
        }
        true
    }
}
