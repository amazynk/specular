//! Checks against an independent decoder (ffmpeg, a test oracle only): the stream decodes
//! without a single complaint, the decoder's pictures equal the encoder's reconstruction bit
//! for bit (no drift), and the reconstruction is close to the source. Every profile.
//!
//! Skipped (with a note) when ffmpeg is not installed.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use specular_h264enc::{Config, Encoder, Frame, MotionHint, Packet, Profile, annex_b};

const PROFILES: [Profile; 3] = [Profile::Baseline, Profile::Main, Profile::High];

struct Yuv {
    w: usize,
    h: usize,
    y: Vec<u8>,
    u: Vec<u8>,
    v: Vec<u8>,
}

impl Yuv {
    fn frame(&self) -> Frame<'_> {
        Frame {
            y: &self.y,
            u: &self.u,
            v: &self.v,
            y_stride: self.w,
            c_stride: self.w / 2,
        }
    }
}

/// A photo-like test picture: smooth gradients, a sharp-edged disc, fine stripes and noise.
fn scene(seed: u32, x: f32, y: f32) -> [f32; 3] {
    let s = seed as f32;
    let mut r = 0.5 + 0.4 * ((x * 0.013 + s).sin() * (y * 0.021 - s * 0.7).cos());
    let mut g = 0.4 + 0.3 * ((x + y) * 0.008 + s * 1.3).sin();
    let mut b = 0.3 + 0.5 * (y / 400.0);
    let (cx, cy) = (180.0 + 40.0 * s.sin(), 110.0);
    if (x - cx).powi(2) + (y - cy).powi(2) < 50.0f32.powi(2) {
        r = 0.9;
        g = 0.85 - 0.1 * s.cos();
        b = 0.2;
    }
    if (40.0..90.0).contains(&x) && (30.0..160.0).contains(&y) {
        let k = ((x * 0.9).sin() > 0.0) as u8 as f32;
        r = 0.2 + 0.6 * k;
        g = r;
        b = r;
    }
    let n = (((x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663) ^ seed)
        % 1000) as f32
        / 1000.0
        - 0.5;
    [r + n * 0.03, g + n * 0.03, b + n * 0.03]
}

/// A view of a scene: seed, and the sample position `(x·zoom + ox, y·zoom + oy)`.
#[derive(Clone, Copy)]
struct View {
    seed: u32,
    zoom: f32,
    ox: f32,
    oy: f32,
}

impl View {
    fn still(seed: u32) -> View {
        View {
            seed,
            zoom: 1.0,
            ox: 0.0,
            oy: 0.0,
        }
    }

    /// Ken Burns step `t`: slow zoom in and pan.
    fn kb(seed: u32, t: f32, speed: f32) -> View {
        View {
            seed,
            zoom: 1.0 - t * 0.0015 * speed,
            ox: t * 0.6 * speed,
            oy: t * 0.35 * speed,
        }
    }

    /// The hint for this view following `prev` (same scene).
    fn hint_from(&self, prev: &View) -> MotionHint {
        MotionHint {
            scale: self.zoom / prev.zoom,
            dx: (self.ox - prev.ox) / prev.zoom,
            dy: (self.oy - prev.oy) / prev.zoom,
        }
    }

    fn at(&self, x: f32, y: f32) -> [f32; 3] {
        scene(self.seed, x * self.zoom + self.ox, y * self.zoom + self.oy)
    }
}

/// BT.709 limited range 4:2:0 of `a` blended into `b` by `mix`, then faded to black by
/// `1 − light`.
fn render_mix(w: usize, h: usize, a: View, b: View, mix: f32, light: f32) -> Yuv {
    let mut rgb = vec![[0f32; 3]; w * h];
    for yy in 0..h {
        for xx in 0..w {
            let (x, y) = (xx as f32, yy as f32);
            let pa = a.at(x, y);
            let p = if mix > 0.0 {
                let pb = b.at(x, y);
                [0, 1, 2].map(|i| pa[i] * (1.0 - mix) + pb[i] * mix)
            } else {
                pa
            };
            rgb[yy * w + xx] = p.map(|v| (v * light).clamp(0.0, 1.0));
        }
    }
    let luma = |p: [f32; 3]| 0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2];
    let y = rgb
        .iter()
        .map(|&p| (16.0 + 219.0 * luma(p)).round() as u8)
        .collect();
    let mut u = vec![0u8; w * h / 4];
    let mut v = vec![0u8; w * h / 4];
    for cy in 0..h / 2 {
        for cx in 0..w / 2 {
            let mut acc = [0f32; 3];
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let p = rgb[(cy * 2 + dy) * w + cx * 2 + dx];
                for i in 0..3 {
                    acc[i] += p[i] / 4.0;
                }
            }
            let l = luma(acc);
            u[cy * w / 2 + cx] = (128.0 + 224.0 * (acc[2] - l) / 1.8556).round() as u8;
            v[cy * w / 2 + cx] = (128.0 + 224.0 * (acc[0] - l) / 1.5748).round() as u8;
        }
    }
    Yuv { w, h, y, u, v }
}

/// `render_mix` of one scene sampled at (x·zoom + ox, y·zoom + oy), blending in a second,
/// static scene by `mix`.
fn render(w: usize, h: usize, seed: u32, zoom: f32, ox: f32, oy: f32, mix: f32) -> Yuv {
    let a = View { seed, zoom, ox, oy };
    render_mix(w, h, a, View::still(seed + 7), mix, 1.0)
}

fn ffmpeg() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("h264enc");
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

/// Decodes with ffmpeg; returns (stderr, raw yuv420p frames).
fn decode(path: &Path) -> (String, Vec<u8>) {
    let out = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-hide_banner",
            "-err_detect",
            "explode+crccheck+bitstream+buffer",
            "-i",
        ])
        .arg(path)
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
    (
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.stdout,
    )
}

fn psnr(a: &[u8], b: &[u8]) -> f64 {
    let se: f64 = a
        .iter()
        .zip(b)
        .map(|(&x, &y)| (f64::from(x) - f64::from(y)).powi(2))
        .sum();
    let mse = se / a.len() as f64;
    if mse == 0.0 {
        99.0
    } else {
        10.0 * (255.0f64 * 255.0 / mse).log10()
    }
}

/// One test picture: source, cut (force IDR), motion hint.
struct Pic {
    yuv: Yuv,
    cut: bool,
    hint: Option<MotionHint>,
}

fn pics(frames: Vec<(Yuv, bool)>) -> Vec<Pic> {
    frames
        .into_iter()
        .map(|(yuv, cut)| Pic {
            yuv,
            cut,
            hint: None,
        })
        .collect()
}

struct Outcome {
    bytes: usize,
    psnr: f64,
    encode: Duration,
    b: usize,
    weighted: usize,
}

/// Encodes `frames`, checks the oracle, returns the size, mean luma PSNR vs source, encode
/// time and picture-type counts.
fn run(name: &str, frames: &[Pic], cfg: Config) -> Outcome {
    let mut enc = Encoder::new(cfg).unwrap();
    let mut packets: Vec<Packet> = Vec::new();
    let mut recon = Vec::new();
    let mut encode = Duration::ZERO;
    for p in frames {
        let t0 = Instant::now();
        packets.push(enc.encode_hinted(&p.yuv.frame(), p.cut, p.hint));
        encode += t0.elapsed();
        let [y, u, v] = enc.reconstruction();
        recon.extend_from_slice(&y);
        recon.extend_from_slice(&u);
        recon.extend_from_slice(&v);
    }
    let stream = annex_b(&enc.sps(), &enc.pps(), &packets);
    let path = scratch(&format!("{name}.h264"));
    std::fs::write(&path, &stream).unwrap();
    let (err, decoded) = decode(&path);
    std::fs::remove_file(&path).ok();
    // One line per stream for tools/ci/oracle-pass-rates.sh: ffmpeg decodes it, bit for bit
    // as our reconstruction.
    let first = &frames[0].yuv;
    let fsz = first.y.len() * 3 / 2;
    let exact = err.trim().is_empty() && decoded == recon;
    println!(
        "\nORACLE\tH264\t{name}\t{}",
        if exact { "pass" } else { "fail" }
    );
    assert!(err.trim().is_empty(), "{name}: ffmpeg reported: {err}");
    assert_eq!(decoded.len(), recon.len(), "{name}: frame count/size");
    for (i, (d, r)) in decoded.chunks(fsz).zip(recon.chunks(fsz)).enumerate() {
        if d != r {
            let at = d.iter().zip(r).position(|(a, b)| a != b).unwrap();
            panic!(
                "{name}: frame {i} differs from the reconstruction at byte {at} \
                 (luma {}×{}; key {}; stats {:?})",
                first.w, first.h, packets[i].key, packets[i].stats
            );
        }
    }
    let mut p = 0.0;
    for (i, f) in frames.iter().enumerate() {
        p += psnr(&f.yuv.y, &decoded[i * fsz..i * fsz + f.yuv.y.len()]);
    }
    Outcome {
        bytes: stream.len(),
        psnr: p / frames.len() as f64,
        encode,
        b: packets.iter().filter(|p| p.stats.b).count(),
        weighted: packets.iter().filter(|p| p.stats.weighted).count(),
    }
}

fn config(w: usize, h: usize, qp: u8, profile: Profile, slices: usize) -> Config {
    let mut cfg = Config::new(w as u32, h as u32, 30, 1, qp);
    cfg.profile = profile;
    cfg.slices = slices;
    cfg
}

#[test]
fn intra_only_decodes_exactly() {
    if !ffmpeg() {
        eprintln!("ffmpeg not installed; oracle test skipped");
        return;
    }
    for profile in PROFILES {
        for (w, h, qp) in [(64, 48, 26), (320, 184, 20), (176, 144, 40), (96, 64, 12)] {
            let frames: Vec<_> = (0..3)
                .map(|i| (render(w, h, i, 1.0, 0.0, 0.0, 0.0), true))
                .collect();
            let o = run(
                &format!("intra_{profile:?}_{w}x{h}_{qp}"),
                &pics(frames),
                config(w, h, qp, profile, 2),
            );
            eprintln!(
                "{profile:?} intra {w}×{h} qp {qp}: {} bytes, luma PSNR {:.2} dB",
                o.bytes, o.psnr
            );
            if qp <= 26 {
                assert!(o.psnr > 36.0, "PSNR {}", o.psnr);
            }
        }
    }
}

/// High profile's tools across the QP range (Intra 8×8 and the 8×8 transform with
/// rate-distortion decisions, intra macroblocks in P pictures from a cut without an IDR, with
/// and without trellis quantisation) decode bit for bit; trellis buys quality per bit.
#[test]
fn high_tools_decode_exactly() {
    if !ffmpeg() {
        eprintln!("ffmpeg not installed; oracle test skipped");
        return;
    }
    let (w, h) = (176usize, 112usize);
    let mut frames = Vec::new();
    for i in 0..6 {
        let t = i as f32;
        frames.push((
            render(w, h, 5, 1.0 - t * 0.01, t * 1.3, t * 0.6, 0.0),
            i == 0,
        ));
    }
    // A cut coded as P pictures, then a cross-fade back.
    for i in 0..3 {
        frames.push((render(w, h, 6, 1.0, i as f32, 0.0, 0.0), false));
    }
    for i in 1..4 {
        frames.push((render(w, h, 6, 1.0, 3.0, 0.0, i as f32 / 4.0), false));
    }
    let frames = pics(frames);
    for qp in [10u8, 22, 34, 51] {
        let mut out = Vec::new();
        for trellis in [false, true] {
            let mut cfg = config(w, h, qp, Profile::High, 2);
            cfg.trellis = trellis;
            let o = run(&format!("high_{qp}_{trellis}"), &frames, cfg);
            eprintln!(
                "High qp {qp} trellis {trellis}: {} bytes, luma PSNR {:.2} dB",
                o.bytes, o.psnr
            );
            out.push(o);
        }
        if (22..=34).contains(&qp) {
            // Fewer bits for the quality, or more quality for the bits.
            let (off, on) = (&out[0], &out[1]);
            assert!(
                on.bytes as f64 <= off.bytes as f64 * 1.02 && on.psnr >= off.psnr - 0.15,
                "trellis {} B {:.2} dB vs {} B {:.2} dB",
                on.bytes,
                on.psnr,
                off.bytes,
                off.psnr
            );
        }
    }
}

#[test]
fn slideshow_motion_decodes_exactly() {
    if !ffmpeg() {
        eprintln!("ffmpeg not installed; oracle test skipped");
        return;
    }
    let (w, h) = (320usize, 184usize);
    let mut frames = Vec::new();
    // Ken Burns: slow zoom and pan.
    for i in 0..12 {
        let t = i as f32;
        frames.push((
            render(w, h, 1, 1.0 - t * 0.004, t * 0.7, t * 0.3, 0.0),
            false,
        ));
    }
    // Hold (identical pictures).
    for _ in 0..3 {
        frames.push((render(w, h, 1, 1.0 - 11.0 * 0.004, 7.7, 3.3, 0.0), false));
    }
    // Cross-fade into the next scene.
    for i in 1..6 {
        frames.push((
            render(w, h, 1, 1.0 - 11.0 * 0.004, 7.7, 3.3, i as f32 / 5.0),
            false,
        ));
    }
    // Hard cut.
    frames.push((render(w, h, 3, 1.0, 0.0, 0.0, 0.0), true));
    frames.push((render(w, h, 3, 1.0, 0.5, 0.25, 0.0), false));
    let frames = pics(frames);
    for profile in PROFILES {
        for (qp, min_psnr) in [(20, 38.0), (28, 32.0)] {
            let o = run(
                &format!("motion_{profile:?}_{qp}"),
                &frames,
                config(w, h, qp, profile, 3),
            );
            eprintln!(
                "{profile:?} slideshow {w}×{h} qp {qp}: {} frames, {} bytes, luma PSNR {:.2} dB, \
                 {} B, {} weighted",
                frames.len(),
                o.bytes,
                o.psnr,
                o.b,
                o.weighted
            );
            assert!(o.psnr > min_psnr, "PSNR {}", o.psnr);
        }
    }
}

/// A Ken Burns pan with each picture's hint (what the slideshow export passes) costs no more
/// than the same pan without, at the same quality, and still decodes bit for bit.
#[test]
fn hinted_pan_is_no_larger() {
    if !ffmpeg() {
        eprintln!("ffmpeg not installed; oracle test skipped");
        return;
    }
    let (w, h) = (640usize, 368usize);
    let views: Vec<View> = (0..16).map(|i| View::kb(4, i as f32, 2.0)).collect();
    let pic = |i: usize, hint: bool| Pic {
        yuv: render_mix(w, h, views[i], views[i], 0.0, 1.0),
        cut: i == 0,
        hint: (hint && i > 0).then(|| views[i].hint_from(&views[i - 1])),
    };
    let plain: Vec<Pic> = (0..views.len()).map(|i| pic(i, false)).collect();
    let hinted: Vec<Pic> = (0..views.len()).map(|i| pic(i, true)).collect();
    for profile in PROFILES {
        let a = run(
            &format!("pan_{profile:?}"),
            &plain,
            config(w, h, 22, profile, 0),
        );
        let b = run(
            &format!("pan_hint_{profile:?}"),
            &hinted,
            config(w, h, 22, profile, 0),
        );
        eprintln!(
            "{profile:?} pan: {} bytes, {:.2} dB; hinted {} bytes, {:.2} dB",
            a.bytes, a.psnr, b.bytes, b.psnr
        );
        assert!(
            b.bytes <= a.bytes,
            "{profile:?}: hinted {} > {}",
            b.bytes,
            a.bytes
        );
        assert!(
            b.psnr >= a.psnr - 0.05,
            "{profile:?}: PSNR {} vs {}",
            b.psnr,
            a.psnr
        );
    }
}

/// A hinted pan is plain P pictures: its pictures fit `α·prev + β·older` (a linear
/// extrapolation of the motion) as well as a cross-fade's do, but B pictures and weights would
/// only cost bits there.
#[test]
fn hinted_pan_is_neither_b_nor_weighted() {
    let (w, h) = (320usize, 184usize);
    let views: Vec<View> = (0..12).map(|i| View::kb(4, i as f32, 2.0)).collect();
    let mut enc = Encoder::new(config(w, h, 22, Profile::High, 0)).unwrap();
    for i in 0..views.len() {
        let yuv = render_mix(w, h, views[i], views[i], 0.0, 1.0);
        let hint = (i > 0).then(|| views[i].hint_from(&views[i - 1]));
        let p = enc.encode_hinted(&yuv.frame(), i == 0, hint);
        assert!(
            !p.stats.b && !p.stats.weighted,
            "picture {i}: {:?}",
            p.stats
        );
    }
}

/// Fades to black and back, cross-fades between moving pictures, with hints: weighted P and
/// B pictures, B pictures after B pictures, holds inside a cross-fade.
#[test]
fn fades_decode_exactly() {
    if !ffmpeg() {
        eprintln!("ffmpeg not installed; oracle test skipped");
        return;
    }
    let (w, h) = (320usize, 184usize);
    let mut frames = Vec::new();
    let mut prev_view: Option<View> = None;
    let mut push = |a: View, b: View, mix: f32, light: f32, cut: bool, frames: &mut Vec<Pic>| {
        let hint = prev_view
            .filter(|p| p.seed == a.seed)
            .map(|p| a.hint_from(&p));
        prev_view = Some(a);
        frames.push(Pic {
            yuv: render_mix(w, h, a, b, mix, light),
            cut,
            hint,
        });
    };
    // Ken Burns fading to black, then up from black.
    for i in 0..6 {
        let a = View::kb(2, i as f32, 3.0);
        push(a, a, 0.0, 1.0 - i as f32 / 6.0, i == 0, &mut frames);
    }
    for i in 0..5 {
        let a = View::still(4);
        push(a, a, 0.0, (i + 1) as f32 / 5.0, false, &mut frames);
    }
    // A still cross-fading into a Ken Burns picture.
    for i in 1..8 {
        let b = View::kb(5, i as f32, 3.0);
        push(View::still(4), b, i as f32 / 8.0, 1.0, false, &mut frames);
        if i == 4 {
            push(View::still(4), b, i as f32 / 8.0, 1.0, false, &mut frames);
        }
    }
    // Ken Burns on its own, then a cross-fade between two moving pictures.
    for i in 8..12 {
        let a = View::kb(5, i as f32, 3.0);
        push(a, a, 0.0, 1.0, false, &mut frames);
    }
    for i in 1..6 {
        let a = View::kb(5, (11 + i) as f32, 3.0);
        let b = View::kb(6, i as f32, 2.0);
        push(a, b, i as f32 / 6.0, 1.0, false, &mut frames);
    }
    for profile in PROFILES {
        for (qp, min_psnr) in [(20, 38.0), (30, 30.0)] {
            let o = run(
                &format!("fades_{profile:?}_{qp}"),
                &frames,
                config(w, h, qp, profile, 2),
            );
            eprintln!(
                "{profile:?} fades {w}×{h} qp {qp}: {} frames, {} bytes, luma PSNR {:.2} dB, \
                 {} B, {} weighted",
                frames.len(),
                o.bytes,
                o.psnr,
                o.b,
                o.weighted
            );
            assert!(o.psnr > min_psnr, "PSNR {}", o.psnr);
            if profile != Profile::Baseline {
                assert!(o.b > 0 && o.weighted > 0, "no B or weighted pictures");
            }
        }
    }
}

#[test]
fn stream_signals_profile_and_bt709() {
    if !ffmpeg() {
        eprintln!("ffmpeg not installed; oracle test skipped");
        return;
    }
    let f = render(128, 96, 5, 1.0, 0.0, 0.0, 0.0);
    for (profile, name) in [
        (Profile::Baseline, "Constrained Baseline"),
        (Profile::Main, "Main"),
        (Profile::High, "High"),
    ] {
        let mut cfg = Config::new(128, 96, 24, 1, 24);
        cfg.profile = profile;
        let mut enc = Encoder::new(cfg).unwrap();
        let p = enc.encode(&f.frame(), false);
        let path = scratch("probe.h264");
        std::fs::write(&path, annex_b(&enc.sps(), &enc.pps(), &[p])).unwrap();
        let out = Command::new("ffprobe")
            .args(["-v", "error", "-show_streams", "-select_streams", "v:0"])
            .arg(&path)
            .output()
            .expect("ffprobe runs");
        std::fs::remove_file(&path).ok();
        let s = String::from_utf8_lossy(&out.stdout);
        for want in [
            &format!("profile={name}\n"),
            "color_range=tv",
            "color_space=bt709",
            "color_transfer=bt709",
            "color_primaries=bt709",
            "width=128",
            "height=96",
            // A raw elementary stream reports the VUI tick rate, time_scale /
            // num_units_in_tick, which is two ticks (fields) per frame: 2 × 24.
            "r_frame_rate=48/1",
        ] {
            assert!(s.contains(want), "ffprobe lacks {want}:\n{s}");
        }
        let avcc = enc.avcc();
        assert_eq!(avcc[1], [66, 77, 100][profile as usize]);
    }
}

/// Size, quality and speed per profile at 1080p on slideshow content, every stream checked
/// against the oracle (run with `--release --ignored --nocapture`).
#[test]
#[ignore]
fn modes_1080p() {
    let (w, h) = (1920usize, 1080usize);
    let n = 30;
    let qp = std::env::var("H264ENC_QP")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20u8);
    let only = std::env::var("H264ENC_SCENES").unwrap_or_default();
    type Scene = (&'static str, Box<dyn Fn(usize) -> (View, View, f32)>);
    let scenes: Vec<Scene> = vec![
        // A still cross-fading into another still (the app's snapshot over a still).
        (
            "crossfade",
            Box::new(move |i| (View::still(1), View::still(2), i as f32 / (n - 1) as f32)),
        ),
        // A still cross-fading into a Ken Burns picture.
        (
            "crossfade+kb",
            Box::new(move |i| {
                (
                    View::still(1),
                    View::kb(2, i as f32, 1.0),
                    i as f32 / (n - 1) as f32,
                )
            }),
        ),
        (
            "kenburns",
            Box::new(|i| (View::kb(1, i as f32, 1.0), View::still(1), 0.0)),
        ),
        ("held", Box::new(|_| (View::still(1), View::still(1), 0.0))),
    ];
    for (name, f) in &scenes {
        if !only.is_empty() && !only.split(',').any(|s| s == *name) {
            continue;
        }
        let mut frames = Vec::new();
        let mut prev: Option<View> = None;
        for i in 0..n {
            let (a, b, mix) = f(i);
            // Hint the moving picture: B (the incoming one) in a cross-fade.
            let moving = if mix > 0.0 { b } else { a };
            let hint = prev.map(|p| moving.hint_from(&p));
            prev = Some(moving);
            frames.push(Pic {
                yuv: render_mix(w, h, a, b, mix, 1.0),
                cut: false,
                hint,
            });
        }
        for profile in PROFILES {
            for hinted in [false, true] {
                if hinted && *name == "held" {
                    continue;
                }
                let fr: Vec<Pic> = frames
                    .iter()
                    .map(|p| Pic {
                        yuv: Yuv {
                            w: p.yuv.w,
                            h: p.yuv.h,
                            y: p.yuv.y.clone(),
                            u: p.yuv.u.clone(),
                            v: p.yuv.v.clone(),
                        },
                        cut: p.cut,
                        hint: if hinted { p.hint } else { None },
                    })
                    .collect();
                let o = run(
                    &format!("bench_{name}_{profile:?}"),
                    &fr,
                    config(w, h, qp, profile, 0),
                );
                eprintln!(
                    "{name:>13} {:>8} {:>6} qp {qp}: {:6.2} Mbit/s, PSNR {:.2} dB, {:5.1} fps, \
                     {} B, {} weighted, bit-exact",
                    format!("{profile:?}"),
                    if hinted { "hint" } else { "" },
                    o.bytes as f64 * 8.0 / (n as f64 / 30.0) / 1e6,
                    o.psnr,
                    n as f64 / o.encode.as_secs_f64(),
                    o.b,
                    o.weighted
                );
            }
        }
    }
}
