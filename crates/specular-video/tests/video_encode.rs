//! The export encoder's engine glue ([`VideoEncoder`]): a hardware backend that fails
//! mid-export or fails its check hands over to the software encoder, and the movie still
//! decodes from start to end in our decoder and in ffmpeg (the oracle; skipped with a note
//! when it is not installed). The "hardware" here is our software encoder behind the backend
//! trait, so these run everywhere; with a VA-API encoder present the real one is run as well
//! (`SPECULAR_REQUIRE_VAAPI_ENC=1` makes it mandatory).
//!
//! `bench_1080p_engines` (ignored) times both engines on a 1080p slideshow-like clip.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use specular_core::paths::{Engine, Task, Trust, selector};
use specular_h264enc::{Config, Encoder, Frame, Profile};
use specular_hwdec::encode::{EncodeConfig, H264Encoder, pps_id, sps_id};
use specular_video::encode::{Cue, HwBackend, Tuning, Used, VideoEncoder, hw_config};
use specular_video::mp4::{self, Colour};
use specular_video::mp4write::{Mp4Writer, Track};

/// A planar 4:2:0 picture like a slideshow's: a smooth photo-like field with detail, panned
/// and slowly zoomed (Ken Burns), and a hard cut to a different picture at `cut`.
fn picture(w: usize, h: usize, t: usize) -> [Vec<u8>; 3] {
    let scene = t / 40;
    let t = t % 40;
    let zoom = 1.0 + t as f32 * 0.002;
    let mut y = vec![0u8; w * h];
    for r in 0..h {
        for c in 0..w {
            let x = (c as f32 + t as f32 * 2.0) / zoom;
            let yy = r as f32 / zoom + scene as f32 * 37.0;
            let smooth = 110.0 + 60.0 * ((x * 0.011).sin() * (yy * 0.007).cos());
            let detail = 18.0 * (((x * 0.31).sin() + (yy * 0.23).cos()) * 0.5);
            let grain = ((x as usize * 7 + yy as usize * 13) % 9) as f32;
            y[r * w + c] = (smooth + detail + grain).clamp(16.0, 235.0) as u8;
        }
    }
    let (cw, ch) = (w / 2, h / 2);
    let mut u = vec![0u8; cw * ch];
    let mut v = vec![0u8; cw * ch];
    for r in 0..ch {
        for c in 0..cw {
            let x = (c as f32 + t as f32) / zoom;
            u[r * cw + c] = (128.0 + 30.0 * (x * 0.013 + scene as f32).sin()) as u8;
            v[r * cw + c] = (128.0 + 25.0 * (r as f32 * 0.017).cos()) as u8;
        }
    }
    [y, u, v]
}

fn frame<'a>(p: &'a [Vec<u8>; 3], w: usize) -> Frame<'a> {
    Frame {
        y: &p[0],
        u: &p[1],
        v: &p[2],
        y_stride: w,
        c_stride: w / 2,
    }
}

/// "Hardware" for the glue: our software encoder (ids 0), failing at picture `fail_at` or
/// coding a different picture than it is given (`wrong`).
struct Mock {
    enc: Encoder,
    sps: Vec<u8>,
    pps: Vec<u8>,
    n: usize,
    fail_at: Option<usize>,
    wrong: bool,
    /// Codes the wrong picture from this one on.
    wrong_from: Option<usize>,
}

impl Mock {
    fn new(w: usize, h: usize, fail_at: Option<usize>, wrong: bool) -> Mock {
        let enc = Encoder::new(Config::new(w as u32, h as u32, 30, 1, 24)).unwrap();
        Mock {
            sps: enc.sps(),
            pps: enc.pps(),
            enc,
            n: 0,
            fail_at,
            wrong,
            wrong_from: None,
        }
    }
}

impl HwBackend for Mock {
    fn sps(&self) -> &[u8] {
        &self.sps
    }
    fn pps(&self) -> &[u8] {
        &self.pps
    }
    fn encode(
        &mut self,
        f: &Frame<'_>,
        force_idr: bool,
        _qp: u8,
    ) -> Result<(Vec<u8>, bool), String> {
        let n = self.n;
        self.n += 1;
        if Some(n) == self.fail_at {
            return Err("vaEndPicture failed: the operation failed (1)".into());
        }
        let grey;
        let f = if self.wrong || self.wrong_from.is_some_and(|k| n >= k) {
            grey = [
                vec![128u8; f.y.len()],
                vec![128u8; f.u.len()],
                vec![128u8; f.v.len()],
            ];
            frame(&grey, f.y_stride)
        } else {
            *f
        };
        let p = self.enc.encode(&f, force_idr);
        Ok((p.data, p.key))
    }
    fn describe(&self) -> String {
        "mock".into()
    }
}

/// The real hardware encoder, failing at picture `fail_at`.
struct Failing {
    hw: H264Encoder,
    n: usize,
    fail_at: Option<usize>,
}

impl HwBackend for Failing {
    fn sps(&self) -> &[u8] {
        self.hw.sps()
    }
    fn pps(&self) -> &[u8] {
        self.hw.pps()
    }
    fn encode(
        &mut self,
        f: &Frame<'_>,
        force_idr: bool,
        qp: u8,
    ) -> Result<(Vec<u8>, bool), String> {
        let n = self.n;
        self.n += 1;
        if Some(n) == self.fail_at {
            return Err("injected failure".into());
        }
        HwBackend::encode(&mut self.hw, f, force_idr, qp)
    }
    fn describe(&self) -> String {
        HwBackend::describe(&self.hw)
    }
}

fn config(w: usize, h: usize) -> Config {
    let mut c = Config::new(w as u32, h as u32, 30, 1, 24);
    c.keyint = 30;
    c
}

/// Encodes `n` pictures into an MP4 the way the export does; returns the movie, the
/// sources and the encoder.
fn export(
    mut enc: VideoEncoder,
    w: usize,
    h: usize,
    n: usize,
) -> (Vec<u8>, Vec<[Vec<u8>; 3]>, VideoEncoder) {
    let track = Track {
        width: w as u32,
        height: h as u32,
        fps_num: 30,
        fps_den: 1,
        avcc: enc.avcc(),
        colour: Colour {
            primaries: 1,
            transfer: 1,
            matrix: 1,
            full_range: false,
        },
    };
    let mut out = Mp4Writer::new(Cursor::new(Vec::new()), track, n as u32).unwrap();
    let mut src = Vec::new();
    for t in 0..n {
        let p = picture(w, h, t);
        let s = enc.encode(&frame(&p, w), false);
        out.write_sample(&s.data, s.key).unwrap();
        src.push(p);
    }
    out.set_avcc(enc.avcc());
    (out.finish().unwrap().into_inner(), src, enc)
}

fn psnr(a: &[u8], b: &[u8]) -> f64 {
    let sum: u64 = a
        .iter()
        .zip(b)
        .map(|(x, y)| u64::from(x.abs_diff(*y)).pow(2))
        .sum();
    if sum == 0 {
        return 99.0;
    }
    10.0 * (255.0 * 255.0 * a.len() as f64 / sum as f64).log10()
}

/// Decodes the movie with our decoder; the luma PSNR of every picture against its source.
fn decode_ours(movie: &[u8], src: &[[Vec<u8>; 3]]) -> Vec<f64> {
    let m = mp4::parse(movie, true).unwrap();
    let v = m.video.unwrap();
    assert_eq!(v.samples.len(), src.len());
    let opts = specular_h264::Options {
        threads: 1,
        output: specular_h264::OutputOrder::Display,
    };
    let mut d = specular_h264::Decoder::with_avcc(&v.config, opts).unwrap();
    let mut out = Vec::new();
    for (i, s) in v.samples.iter().enumerate() {
        let bytes = &movie[s.offset as usize..][..s.size as usize];
        d.decode_sample(bytes, i as i64).unwrap();
        while let Some(p) = d.next_picture(false) {
            out.push(p);
        }
    }
    d.flush();
    while let Some(p) = d.next_picture(true) {
        out.push(p);
    }
    assert_eq!(out.len(), src.len(), "our decoder's picture count");
    out.iter()
        .map(|p| psnr(&src[p.tag as usize][0], &p.planes[0]))
        .collect()
}

fn have_ffmpeg() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("video-encode");
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

/// ffmpeg decodes the whole movie without a complaint; the luma PSNR of every picture.
fn decode_ffmpeg(
    movie: &[u8],
    src: &[[Vec<u8>; 3]],
    w: usize,
    h: usize,
    name: &str,
) -> Option<Vec<f64>> {
    if !have_ffmpeg() {
        eprintln!("ffmpeg not installed; oracle decode skipped");
        return None;
    }
    let path = scratch(name);
    std::fs::write(&path, movie).unwrap();
    let dec = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-hide_banner",
            "-err_detect",
            "explode+bitstream+buffer",
            "-i",
        ])
        .arg(&path)
        .args([
            "-fps_mode",
            "passthrough",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "yuv420p",
            "-",
        ])
        .output()
        .expect("ffmpeg runs");
    std::fs::remove_file(&path).ok();
    let err = String::from_utf8_lossy(&dec.stderr);
    let fsize = w * h * 3 / 2;
    // One line per movie for tools/ci/oracle-pass-rates.sh: ffmpeg decodes all of it.
    let clean = err.trim().is_empty() && dec.stdout.len() == fsize * src.len();
    println!(
        "\nORACLE\tMP4/H264\t{name}\t{}",
        if clean { "pass" } else { "fail" }
    );
    assert!(err.trim().is_empty(), "ffmpeg reported: {err}");
    assert_eq!(
        dec.stdout.len(),
        fsize * src.len(),
        "ffmpeg's picture count"
    );
    Some(
        src.iter()
            .enumerate()
            .map(|(t, s)| psnr(&s[0], &dec.stdout[t * fsize..][..w * h]))
            .collect(),
    )
}

fn sets(avcc: &[u8]) -> (Vec<u32>, Vec<u32>) {
    let (_, nals) = specular_h264::parse_avcc(avcc).unwrap();
    let sps = nals
        .iter()
        .filter(|n| n[0] & 31 == 7)
        .filter_map(|n| sps_id(n))
        .collect();
    let pps = nals
        .iter()
        .filter(|n| n[0] & 31 == 8)
        .filter_map(|n| pps_id(n))
        .collect();
    (sps, pps)
}

#[test]
fn hardware_failing_mid_export_hands_over_to_software() {
    let (w, h, n) = (160, 96, 50);
    let mock = Mock::new(w, h, Some(17), false);
    let enc = VideoEncoder::with_backend(config(w, h), Some(Box::new(mock)), false).unwrap();
    assert_eq!(enc.engine(), Engine::HwEncoder);
    assert_eq!(sets(&enc.avcc()), (vec![0], vec![0]));
    let (movie, src, enc) = export(enc, w, h, n);
    assert_eq!(enc.used(), Used::Both);
    assert_eq!(enc.engine(), Engine::Cpu);
    assert!(enc.fell_back().is_some_and(|w| w.contains("vaEndPicture")));
    assert_eq!(
        selector().trust(Task::VideoEncode, 1),
        Trust::Blocked,
        "an error blocks the path"
    );
    // Both encoders' sets, apart by id.
    assert_eq!(sets(&enc.avcc()), (vec![0, 1], vec![0, 1]));
    let m = mp4::parse(&movie, true).unwrap();
    let sync: Vec<usize> = m
        .video
        .unwrap()
        .samples
        .iter()
        .enumerate()
        .filter(|(_, s)| s.sync)
        .map(|(i, _)| i)
        .collect();
    // The hardware's IDR, the software's first picture (the failed one) and its keyint.
    assert_eq!(sync, vec![0, 17, 47]);
    let ours = decode_ours(&movie, &src);
    assert!(ours.iter().all(|&d| d > 33.0), "{ours:?}");
    if let Some(ff) = decode_ffmpeg(&movie, &src, w, h, "fallback.mp4") {
        for (t, (a, b)) in ours.iter().zip(&ff).enumerate() {
            assert!(
                (a - b).abs() < 0.01,
                "picture {t}: ours {a:.2} dB, ffmpeg {b:.2} dB"
            );
        }
    }
}

#[test]
fn hardware_failing_its_check_is_not_used() {
    let (w, h, n) = (96, 64, 8);
    let mock = Mock::new(w, h, None, true);
    let enc = VideoEncoder::with_backend(config(w, h), Some(Box::new(mock)), true).unwrap();
    let (movie, src, enc) = export(enc, w, h, n);
    assert_eq!(enc.used(), Used::Software);
    assert!(enc.fell_back().is_some_and(|w| w.contains("dB")));
    assert_eq!(selector().trust(Task::VideoEncode, 1), Trust::Blocked);
    // No hardware sample was written: only the software's sets, with id 0.
    assert_eq!(sets(&enc.avcc()), (vec![0], vec![0]));
    let ours = decode_ours(&movie, &src);
    assert!(ours.iter().all(|&d| d > 33.0), "{ours:?}");
}

#[test]
fn hardware_passing_its_check_encodes_everything() {
    let (w, h, n) = (96, 64, 12);
    let mock = Mock::new(w, h, None, false);
    let enc = VideoEncoder::with_backend(config(w, h), Some(Box::new(mock)), true).unwrap();
    let (movie, src, enc) = export(enc, w, h, n);
    assert_eq!(enc.used(), Used::Hardware);
    assert!(enc.fell_back().is_none());
    assert_eq!(sets(&enc.avcc()), (vec![0], vec![0]));
    let ours = decode_ours(&movie, &src);
    assert!(ours.iter().all(|&d| d > 33.0), "{ours:?}");
}

#[test]
fn software_only() {
    let (w, h, n) = (96, 64, 6);
    let enc = VideoEncoder::with_backend(config(w, h), None, false).unwrap();
    assert_eq!(enc.engine(), Engine::Cpu);
    let (movie, src, enc) = export(enc, w, h, n);
    assert_eq!(enc.used(), Used::Software);
    assert_eq!(sets(&enc.avcc()), (vec![0], vec![0]));
    decode_ours(&movie, &src);
}

/// The software encoder's pictures decode bit for bit in our decoder, in every profile from
/// low to high QPs: Intra 8×8, the 8×8 transform, the rate-distortion decisions and trellis
/// quantisation included (ffmpeg checks the same in the encoder's oracle tests).
#[test]
fn software_encoder_matches_our_decoder() {
    let (w, h) = (160usize, 96usize);
    let mix = |a: &[Vec<u8>; 3], b: &[Vec<u8>; 3], t: f32| -> [Vec<u8>; 3] {
        std::array::from_fn(|k| {
            a[k].iter()
                .zip(&b[k])
                .map(|(&x, &y)| (f32::from(x) * (1.0 - t) + f32::from(y) * t).round() as u8)
                .collect()
        })
    };
    // A pan, a cut without an IDR (intra macroblocks in P pictures), a cross-fade.
    let mut pics: Vec<[Vec<u8>; 3]> = (30..46).map(|t| picture(w, h, t)).collect();
    let (a, b) = (picture(w, h, 45), picture(w, h, 85));
    pics.extend((1..8).map(|i| mix(&a, &b, i as f32 / 8.0)));
    for profile in [Profile::Baseline, Profile::Main, Profile::High] {
        for trellis in [true, false] {
            if profile == Profile::Baseline && !trellis {
                continue;
            }
            for qp in [14u8, 28, 44] {
                let mut cfg = Config::new(w as u32, h as u32, 30, 1, qp);
                cfg.profile = profile;
                cfg.trellis = trellis;
                let mut enc = Encoder::new(cfg).unwrap();
                let opts = specular_h264::Options {
                    threads: 1,
                    output: specular_h264::OutputOrder::Display,
                };
                let mut d = specular_h264::Decoder::with_avcc(&enc.avcc(), opts).unwrap();
                let mut recs = Vec::new();
                let mut out = Vec::new();
                for (i, p) in pics.iter().enumerate() {
                    let pk = enc.encode(&frame(p, w), i == 0);
                    recs.push(enc.reconstruction());
                    d.decode_sample(&pk.data, i as i64).unwrap();
                    while let Some(p) = d.next_picture(false) {
                        out.push(p);
                    }
                }
                d.flush();
                while let Some(p) = d.next_picture(true) {
                    out.push(p);
                }
                assert_eq!(out.len(), pics.len());
                for p in &out {
                    let t = p.tag as usize;
                    assert!(
                        p.planes == recs[t],
                        "{profile:?} trellis {trellis} qp {qp}: picture {t} differs"
                    );
                }
            }
        }
    }
}

fn required() -> bool {
    std::env::var("SPECULAR_REQUIRE_VAAPI_ENC").is_ok_and(|v| v == "1")
}

fn real_hw(w: usize, h: usize, qp: u8) -> Option<H264Encoder> {
    if specular_hwdec::capabilities()
        .and_then(|c| c.h264_encode.clone())
        .is_none()
    {
        assert!(
            !required(),
            "SPECULAR_REQUIRE_VAAPI_ENC=1 but no hardware H.264 encoder"
        );
        eprintln!("no hardware H.264 encoder; skipped");
        return None;
    }
    let cfg = EncodeConfig {
        width: w as u32,
        height: h as u32,
        fps_num: 30,
        fps_den: 1,
        qp,
        keyint: 30,
    };
    Some(H264Encoder::new(&cfg).expect("the probe found an encoder"))
}

/// The real hardware encoder, failing mid-export: its High stream and the software's in one
/// track, which ffmpeg and our decoder play from start to end.
#[test]
fn real_hardware_with_mid_export_fallback() {
    let (w, h, n) = (328, 186, 45);
    let Some(hw) = real_hw(w, h, 24) else {
        return;
    };
    let failing = Failing {
        hw,
        n: 0,
        fail_at: Some(23),
    };
    let enc = VideoEncoder::with_backend(config(w, h), Some(Box::new(failing)), true).unwrap();
    let (movie, src, enc) = export(enc, w, h, n);
    assert_eq!(enc.used(), Used::Both, "{:?}", enc.fell_back());
    assert_eq!(sets(&enc.avcc()), (vec![0, 1], vec![0, 1]));
    let ours = decode_ours(&movie, &src);
    assert!(ours.iter().all(|&d| d > 33.0), "{ours:?}");
    if let Some(ff) = decode_ffmpeg(&movie, &src, w, h, "hw_fallback.mp4") {
        for (t, (a, b)) in ours.iter().zip(&ff).enumerate() {
            assert!(
                (a - b).abs() < 0.01,
                "picture {t}: ours {a:.2} dB, ffmpeg {b:.2} dB"
            );
        }
    }
}

/// 1080p export throughput of both engines, and their size and quality at the same QP (the
/// calibration of the hardware's QP offset). `cargo test --release -- --ignored`.
#[test]
#[ignore = "benchmark"]
fn bench_1080p_engines() {
    let (w, h, n) = (1920, 1080, 120);
    let pics: Vec<[Vec<u8>; 3]> = (0..n).map(|t| picture(w, h, t)).collect();
    for qp in [20u8, 24, 28, 32] {
        let mut cfg = config(w, h);
        cfg.qp = qp;
        cfg.keyint = 150;
        let mut runs: Vec<(&str, Option<Box<dyn HwBackend>>)> = vec![("CPU", None)];
        if let Some(hw) = real_hw(w, h, qp) {
            runs.push(("GPU", Some(Box::new(hw))));
        }
        for (name, backend) in runs {
            let mut enc = VideoEncoder::with_backend(cfg.clone(), backend, false).unwrap();
            let mut samples = Vec::new();
            let t0 = Instant::now();
            for p in &pics {
                samples.push(enc.encode(&frame(p, w), false));
            }
            let secs = t0.elapsed().as_secs_f64();
            let bytes: usize = samples.iter().map(|s| s.data.len()).sum();
            // Quality through our decoder.
            let opts = specular_h264::Options {
                threads: 0,
                output: specular_h264::OutputOrder::Display,
            };
            let mut d = specular_h264::Decoder::with_avcc(&enc.avcc(), opts).unwrap();
            let mut dbs = Vec::new();
            for (i, s) in samples.iter().enumerate() {
                d.decode_sample(&s.data, i as i64).unwrap();
                while let Some(p) = d.next_picture(false) {
                    dbs.push(psnr(&pics[p.tag as usize][0], &p.planes[0]));
                }
            }
            d.flush();
            while let Some(p) = d.next_picture(true) {
                dbs.push(psnr(&pics[p.tag as usize][0], &p.planes[0]));
            }
            let mean = dbs.iter().sum::<f64>() / dbs.len() as f64;
            eprintln!(
                "qp {qp} {name}: {:.1} fps, {} kB, luma PSNR {mean:.2} dB",
                n as f64 / secs,
                bytes / 1000
            );
        }
    }
}

/// A hardware encoder that starts coding the wrong pictures mid-export (as a reconstruction
/// drifting from its bitstream would look): a spot check catches it, blocks the path and the
/// software encoder takes over at an IDR picture; an earlier spot check passed.
#[test]
fn hardware_drifting_fails_its_spot_check() {
    let (w, h, n) = (96, 64, 90);
    let mut mock = Mock::new(w, h, None, false);
    mock.wrong_from = Some(40);
    let tuning = Tuning {
        spot_every: Some(20),
        ..Tuning::default()
    };
    let mut enc =
        VideoEncoder::with_tuning(config(w, h), Some(Box::new(mock)), false, tuning).unwrap();
    let mut keys = Vec::new();
    for t in 0..n {
        let p = picture(w, h, t);
        if enc.encode(&frame(&p, w), t % 30 == 0).key {
            keys.push(t);
        }
        if t == 63 {
            assert_eq!(enc.engine(), Engine::HwEncoder, "{:?}", enc.fell_back());
        }
    }
    // Checked at 34 (GOP 30, fine), then at 64 (GOP 60, wrong since 40): the software's
    // IDR picture replaces picture 64.
    assert_eq!(enc.spot_checks(), 1);
    assert!(
        enc.fell_back().is_some_and(|w| w.contains("P picture")),
        "{:?}",
        enc.fell_back()
    );
    assert_eq!(enc.used(), Used::Both);
    assert_eq!(keys, vec![0, 30, 60, 64]);
    assert_eq!(selector().trust(Task::VideoEncode, 1), Trust::Blocked);
}

/// Encoder size and quality on 1080p slideshows rendered by the export's own path (the
/// app's `dump_slideshows_for_encoder_bench`; `SPECULAR_FADE_BENCH_DIR`): per show, engine
/// and QP the bytes and mean luma PSNR of the fading pictures, of the rest, and in all; last
/// the bytes and count of the IDR pictures (for the export's size estimate).
/// `SPECULAR_FADE_BENCH_QPS` (default 18,24,28) and `SPECULAR_FADE_BENCH_HW` (hardware
/// tunings `qp_offset:fade_qp_offset`, default 0:0) pick the points;
/// `SPECULAR_FADE_BENCH_CPU=0` leaves the software encoder out;
/// `SPECULAR_FADE_BENCH_SPOT` spot-checks every that many pictures (0: never). CSV on stderr.
/// `cargo test --release -- --ignored`.
#[test]
#[ignore = "benchmark"]
fn bench_slideshow_fades() {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(dir) = std::env::var("SPECULAR_FADE_BENCH_DIR") else {
        eprintln!("SPECULAR_FADE_BENCH_DIR not set; skipped");
        return;
    };
    let env = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_owned());
    let qps: Vec<u8> = env("SPECULAR_FADE_BENCH_QPS", "18,24,28")
        .split(',')
        .map(|q| q.trim().parse().unwrap())
        .collect();
    let tunings: Vec<(i8, i8)> = env("SPECULAR_FADE_BENCH_HW", "0:0")
        .split(',')
        .filter(|t| !t.is_empty())
        .map(|t| {
            let (a, b) = t.split_once(':').unwrap();
            (a.trim().parse().unwrap(), b.trim().parse().unwrap())
        })
        .collect();
    let cpu = env("SPECULAR_FADE_BENCH_CPU", "1") == "1";
    let spot_every = std::env::var("SPECULAR_FADE_BENCH_SPOT")
        .ok()
        .map(|v| v.parse().unwrap());
    let shows = env("SPECULAR_FADE_BENCH_SHOWS", "crossfade,kenburns,black");
    eprintln!(
        "show,engine,qp,qp_offset,fade_offset,kB,fade_kB,rest_kB,dB,fade_dB,rest_dB,fps,spot,idr_kB,idrs"
    );
    for name in shows.split(',') {
        let dir = Path::new(&dir);
        let Ok(meta) = std::fs::read_to_string(dir.join(format!("{name}.txt"))) else {
            eprintln!("{name}: not dumped; skipped");
            continue;
        };
        let file = std::fs::File::open(dir.join(format!("{name}.yuv"))).unwrap();
        let mut lines = meta.lines();
        let head: Vec<u32> = lines
            .next()
            .unwrap()
            .split(' ')
            .map(|v| v.parse().unwrap())
            .collect();
        let (w, h, fps) = (head[0] as usize, head[1] as usize, head[2]);
        let cues: Vec<Cue> = lines
            .map(|l| {
                let v: Vec<&str> = l.split(' ').collect();
                Cue {
                    force_idr: v[0] == "1",
                    fading: v[1] == "1",
                    hint: (v[2] != "-").then(|| specular_h264enc::MotionHint {
                        scale: v[2].parse().unwrap(),
                        dx: v[3].parse().unwrap(),
                        dy: v[4].parse().unwrap(),
                    }),
                }
            })
            .collect();
        let n = cues.len();
        let (ys, cs) = (w * h, (w / 2) * (h / 2));
        let read = |i: usize| -> [Vec<u8>; 3] {
            let len = ys + 2 * cs;
            let mut buf = vec![0u8; len];
            let mut f = &file;
            f.seek(SeekFrom::Start((i * len) as u64)).unwrap();
            f.read_exact(&mut buf).unwrap();
            let v = buf.split_off(ys + cs);
            let u = buf.split_off(ys);
            [buf, u, v]
        };
        for &qp in &qps {
            let mut cfg = Config::new(w as u32, h as u32, fps, 1, qp);
            cfg.keyint = fps * 5;
            let mut runs: Vec<(&str, Tuning, bool)> = Vec::new();
            if cpu {
                runs.push(("CPU", Tuning::default(), false));
            }
            for &(qp_offset, fade_qp_offset) in &tunings {
                let t = Tuning {
                    qp_offset,
                    fade_qp_offset,
                    spot_every,
                };
                runs.push(("GPU", t, true));
            }
            for (engine, tuning, gpu) in runs {
                let backend: Option<Box<dyn HwBackend>> = if gpu {
                    match H264Encoder::new(&hw_config(&cfg, &tuning)) {
                        Ok(e) => Some(Box::new(e)),
                        Err(e) => {
                            eprintln!("no hardware encoder: {e}");
                            continue;
                        }
                    }
                } else {
                    None
                };
                let mut enc =
                    VideoEncoder::with_tuning(cfg.clone(), backend, false, tuning).unwrap();
                let mut samples = Vec::with_capacity(n);
                let mut secs = 0.0;
                for (i, cue) in cues.iter().enumerate() {
                    let p = read(i);
                    let t0 = Instant::now();
                    samples.push(enc.encode_cued(&frame(&p, w), *cue));
                    secs += t0.elapsed().as_secs_f64();
                }
                if let Some(why) = enc.fell_back() {
                    eprintln!("{name} {engine} qp {qp}: fell back: {why}");
                }
                let opts = specular_h264::Options {
                    threads: 0,
                    output: specular_h264::OutputOrder::Display,
                };
                let mut d = specular_h264::Decoder::with_avcc(&enc.avcc(), opts).unwrap();
                let mut dbs = vec![0.0; n];
                let mut got = 0;
                let mut take = |p: specular_h264::Picture| {
                    let t = p.tag as usize;
                    dbs[t] = psnr(&read(t)[0], &p.planes[0]);
                    got += 1;
                };
                for (i, s) in samples.iter().enumerate() {
                    d.decode_sample(&s.data, i as i64).unwrap();
                    while let Some(p) = d.next_picture(false) {
                        take(p);
                    }
                }
                d.flush();
                while let Some(p) = d.next_picture(true) {
                    take(p);
                }
                assert_eq!(got, n);
                let sum = |fade: Option<bool>| {
                    let (mut bytes, mut db, mut k) = (0usize, 0.0, 0usize);
                    for i in 0..n {
                        if fade.is_none_or(|f| f == cues[i].fading) {
                            bytes += samples[i].data.len();
                            db += dbs[i];
                            k += 1;
                        }
                    }
                    (bytes as f64 / 1000.0, db / k.max(1) as f64)
                };
                let (all, fade, rest) = (sum(None), sum(Some(true)), sum(Some(false)));
                let keys = samples.iter().filter(|s| s.key);
                let idrs = keys.clone().count();
                let idr_kb = keys.map(|s| s.data.len()).sum::<usize>() as f64 / 1000.0;
                eprintln!(
                    "{name},{engine},{qp},{},{},{:.0},{:.0},{:.0},{:.3},{:.3},{:.3},{:.1},{},{idr_kb:.0},{idrs}",
                    tuning.qp_offset,
                    tuning.fade_qp_offset,
                    all.0,
                    fade.0,
                    rest.0,
                    all.1,
                    fade.1,
                    rest.1,
                    n as f64 / secs,
                    enc.spot_checks()
                );
            }
        }
    }
}
