//! The export's clock, without a GPU: the output sizes, frame rates and qualities, the order
//! of the slides, which slide and moment each frame shows, and the estimates the dialog
//! shows before anything is rendered.
//!
//! Frame `n` is at `n / fps` seconds. A slide is on for the show's interval (an animation
//! plays through once, up to [`ANIM_CAP_S`], as in the live show); it starts at the first
//! frame at or after the previous slide's end, so a slide of `d` seconds gets
//! `ceil(d · fps)` frames and the timeline never drifts.

use specular_h264enc::Profile;
use specular_settings::SlideshowTransition;

use super::super::slideshow::{ANIM_CAP_S, FADE_S, Playlist, fade_weight};

/// Below this a time counts as reached (float noise at frame boundaries).
const EPS: f64 = 1e-9;

/// The video's frame size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Resolution {
    P720,
    P1080,
    P2160,
    /// The viewer window's size (physical pixels).
    Window,
}

impl Resolution {
    pub const ALL: [Resolution; 4] = [
        Resolution::P720,
        Resolution::P1080,
        Resolution::P2160,
        Resolution::Window,
    ];

    /// The frame size: the preset, or the window rounded down to even sides (4:2:0).
    pub fn size(self, window: (u32, u32)) -> (u32, u32) {
        match self {
            Resolution::P720 => (1280, 720),
            Resolution::P1080 => (1920, 1080),
            Resolution::P2160 => (3840, 2160),
            Resolution::Window => (
                (window.0.clamp(64, 8192)) & !1,
                (window.1.clamp(64, 8192)) & !1,
            ),
        }
    }
}

pub(crate) const FPS: [u32; 3] = [24, 30, 60];

/// How hard the encoder squeezes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Quality {
    High,
    Standard,
    Compact,
}

impl Quality {
    pub const ALL: [Quality; 3] = [Quality::High, Quality::Standard, Quality::Compact];

    /// The encoder's QP.
    pub fn qp(self) -> u8 {
        specular_h264enc::qp_for_quality(match self {
            Quality::High => 1.0,
            Quality::Standard => 0.6,
            Quality::Compact => 0.3,
        })
    }
}

/// The dialog's compatibility choices: High (the default; current players) and Constrained
/// Baseline (every player, larger files).
pub(crate) const PROFILES: [Profile; 2] = [Profile::High, Profile::Baseline];

/// What the pictures of an export cost, for the size estimate.
struct Rates {
    /// Bits per pixel of an IDR picture.
    intra: f64,
    /// Of a picture while a cross-fade runs between stills. The incoming slide's content is
    /// coded during the fade, so these carry what an intra picture per slide would.
    fade: f64,
    /// Of a cross-fade picture with Ken Burns (both slides move).
    fade_kb: f64,
    /// Of a Ken Burns picture outside the fade.
    pan: f64,
    /// Bytes of a picture that repeats the last (all macroblocks skipped).
    held: f64,
}

/// The encoder's rates at each quality (QP 18, 24, 28) and profile. Measured with the
/// software encoder (`specular-h264enc` `tests/slideshow.rs` on shows written by the app's
/// `dump_slideshows_for_encoder_bench`, 2026-10) on 1080p30 shows of five camera photos of
/// 12–48 MP (detailed, scaled down), 2 s per slide, cross-fade and Ken Burns. The Kodak set
/// the bench uses by default (768×512 scaled up 2.1×, so soft) comes out at about 0.6× the
/// intra and 0.45× the pan rates, the fade rates within 10 %. The GPU's encoder (VA-API,
/// which Automatic picks for High where it can) writes files within about 10 % of these at
/// the same quality setting (more for held pictures, less for pans), so one table serves
/// both. Rates are per pixel at 1080p: larger frames of the same photos cost somewhat less
/// per pixel, smaller ones more.
fn rates(quality: Quality, profile: Profile) -> Rates {
    // (intra, fade, fade with Ken Burns, pan)
    let (intra, fade, fade_kb, pan) = match (profile, quality) {
        (Profile::Baseline, Quality::High) => (2.34, 0.74, 0.86, 0.46),
        (Profile::Baseline, Quality::Standard) => (1.52, 0.40, 0.46, 0.154),
        (Profile::Baseline, Quality::Compact) => (1.09, 0.25, 0.28, 0.064),
        (Profile::Main, Quality::High) => (2.22, 0.47, 0.70, 0.46),
        (Profile::Main, Quality::Standard) => (1.44, 0.23, 0.33, 0.157),
        (Profile::Main, Quality::Compact) => (1.02, 0.14, 0.19, 0.063),
        (Profile::High, Quality::High) => (2.19, 0.45, 0.69, 0.46),
        (Profile::High, Quality::Standard) => (1.42, 0.22, 0.32, 0.155),
        (Profile::High, Quality::Compact) => (1.00, 0.13, 0.19, 0.062),
    };
    Rates {
        intra,
        fade,
        fade_kb,
        pan,
        // CAVLC skips a held picture's macroblocks in a few run-length codes; CABAC codes a
        // skip flag for each.
        held: if profile == Profile::Baseline {
            10.0
        } else {
            68.0
        },
    }
}

/// The order of the slides: indices into the listing, from `start`, every image once
/// (shuffled with `seed` when asked), as the live show plays one cycle.
pub(crate) fn order(n: usize, start: usize, shuffle: bool, seed: u64) -> Vec<usize> {
    if n == 0 {
        return Vec::new();
    }
    let mut p = Playlist::new(n, start.min(n - 1), shuffle, false, seed);
    let mut v: Vec<usize> = p.current().into_iter().collect();
    while let Some(i) = p.advance() {
        v.push(i);
    }
    v
}

/// The Ken Burns direction of the slide at `listing_index` (as the live show picks it).
pub(crate) fn ken_burns_seed(run_seed: u64, listing_index: usize) -> u64 {
    run_seed ^ (listing_index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// How long a slide is on: the interval, or for an animation one play-through of
/// `anim_total` seconds when that is longer, up to [`ANIM_CAP_S`] (or the interval when that
/// is longer).
pub(crate) fn slide_seconds(interval: f64, anim_total: Option<f64>) -> f64 {
    match anim_total {
        None => interval,
        Some(a) => interval.max(a.min(interval.max(ANIM_CAP_S))),
    }
}

/// Frames of a slide of `seconds`.
pub(crate) fn frames_for(seconds: f64, fps: u32) -> u64 {
    (seconds * f64::from(fps) - EPS).ceil().max(1.0) as u64
}

/// What one frame of a slide shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Moment {
    /// Seconds since the slide came on.
    pub tau: f64,
    /// Ken Burns progress (0–1), when Ken Burns is on.
    pub ken_burns: Option<f32>,
    /// The outgoing slide's weight while the cross-fade runs.
    pub fade: Option<f32>,
    /// The first frame of a slide after a hard cut: an IDR picture.
    pub cut: bool,
}

/// One slide's clock.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SlideClock {
    pub fps: u32,
    pub interval: f64,
    pub transition: SlideshowTransition,
    /// Whether a slide came before (the first slide does not fade in).
    pub after_another: bool,
}

impl SlideClock {
    /// Frame `k` of the slide.
    pub fn moment(&self, k: u64) -> Moment {
        let tau = k as f64 / f64::from(self.fps);
        let ken_burns = (self.transition == SlideshowTransition::KenBurns)
            .then(|| (tau / self.interval).clamp(0.0, 1.0) as f32);
        let fades = self.after_another && self.transition != SlideshowTransition::None;
        let fade = (fades && tau + EPS < FADE_S).then(|| fade_weight(tau / FADE_S));
        Moment {
            tau,
            ken_burns,
            fade,
            cut: k == 0 && self.after_another && self.transition == SlideshowTransition::None,
        }
    }
}

/// The dialog's estimates: the length in seconds and the file size in bytes, with every
/// slide counted at the interval (animations may run longer).
pub(crate) fn estimate(
    slides: usize,
    interval: f64,
    transition: SlideshowTransition,
    fps: u32,
    size: (u32, u32),
    quality: Quality,
    profile: Profile,
) -> (f64, u64) {
    let seconds = slides as f64 * interval;
    let px = f64::from(size.0) * f64::from(size.1);
    let r = rates(quality, profile);
    let per_slide = frames_for(interval, fps) as f64;
    let frames = slides as f64 * per_slide;
    // An IDR picture every keyframe interval (5 s, as the export sets it). A hard cut starts
    // each slide with one; a cross-fade codes the incoming slide in the fade's pictures.
    let keyint = f64::from(fps) * 5.0;
    let idr = match transition {
        SlideshowTransition::None => slides as f64 * (per_slide / keyint).ceil(),
        _ => (frames / keyint).ceil(),
    };
    let fading = match transition {
        SlideshowTransition::None => 0.0,
        _ => slides.saturating_sub(1) as f64 * (FADE_S * f64::from(fps)).ceil().min(per_slide),
    };
    let (fade, moving) = match transition {
        SlideshowTransition::KenBurns => (r.fade_kb, (frames - fading - idr).max(0.0)),
        _ => (r.fade, 0.0),
    };
    let held = (frames - idr - fading - moving).max(0.0);
    let bits = (idr * r.intra + fading * fade + moving * r.pan) * px + held * r.held * 8.0;
    // The MP4's boxes: a few kB, and about 8 bytes per sample in the tables.
    (seconds, (bits / 8.0 + 4096.0 + frames * 8.0) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_h264enc::{Hints, MotionHint};

    #[test]
    fn sizes_are_even_and_presets_fixed() {
        assert_eq!(Resolution::P1080.size((10, 10)), (1920, 1080));
        assert_eq!(Resolution::P2160.size((10, 10)), (3840, 2160));
        assert_eq!(Resolution::Window.size((1733, 1011)), (1732, 1010));
        assert_eq!(Resolution::Window.size((10, 10)), (64, 64));
        for q in Quality::ALL {
            assert!((10..=51).contains(&q.qp()));
        }
        assert!(Quality::High.qp() < Quality::Standard.qp());
        assert!(Quality::Standard.qp() < Quality::Compact.qp());
    }

    #[test]
    fn order_is_one_cycle_and_deterministic() {
        assert_eq!(order(5, 2, false, 1), [2, 3, 4, 0, 1]);
        let a = order(40, 7, true, 42);
        assert_eq!(a, order(40, 7, true, 42));
        assert_ne!(a, order(40, 7, true, 43));
        assert_eq!(a[0], 7, "the show starts at the current image");
        let mut s = a.clone();
        s.sort_unstable();
        assert_eq!(s, (0..40).collect::<Vec<_>>());
        assert!(order(0, 0, true, 1).is_empty());
    }

    #[test]
    fn slides_last_the_interval_or_one_play_through() {
        assert_eq!(slide_seconds(5.0, None), 5.0);
        assert_eq!(slide_seconds(5.0, Some(2.0)), 5.0);
        assert_eq!(slide_seconds(5.0, Some(12.5)), 12.5);
        assert_eq!(slide_seconds(5.0, Some(500.0)), ANIM_CAP_S);
        assert_eq!(slide_seconds(90.0, Some(500.0)), 90.0);
        assert_eq!(frames_for(5.0, 30), 150);
        assert_eq!(frames_for(1.0 / 3.0, 30), 10);
        assert_eq!(frames_for(0.34, 30), 11);
        assert_eq!(frames_for(2.05, 24), 50);
    }

    #[test]
    fn moments_fade_drift_and_cut() {
        let c = SlideClock {
            fps: 30,
            interval: 4.0,
            transition: SlideshowTransition::KenBurns,
            after_another: true,
        };
        let m0 = c.moment(0);
        assert_eq!(m0.fade, Some(1.0));
        assert_eq!(m0.ken_burns, Some(0.0));
        assert!(!m0.cut);
        // The fade lasts FADE_S: 12 frames at 30 fps.
        assert!(c.moment(11).fade.is_some_and(|w| w > 0.0 && w < 0.05));
        assert_eq!(c.moment(12).fade, None);
        assert_eq!(c.moment(60).ken_burns, Some(0.5));
        assert_eq!(c.moment(400).ken_burns, Some(1.0));
        let first = SlideClock {
            after_another: false,
            ..c
        };
        assert_eq!(first.moment(0).fade, None);
        let cut = SlideClock {
            transition: SlideshowTransition::None,
            ..c
        };
        assert!(cut.moment(0).cut && !cut.moment(1).cut);
        assert_eq!(cut.moment(0).fade, None);
        assert_eq!(cut.moment(0).ken_burns, None);
        let fade = SlideClock {
            transition: SlideshowTransition::CrossFade,
            ..c
        };
        assert!(fade.moment(3).fade.is_some() && fade.moment(3).ken_burns.is_none());
    }

    #[test]
    fn estimates_grow_with_size_quality_and_motion() {
        use SlideshowTransition as T;
        let e = |t, fps, size, q| estimate(20, 5.0, t, fps, size, q, Profile::Baseline);
        let (s, base) = e(T::CrossFade, 30, (1920, 1080), Quality::Standard);
        assert_eq!(s, 100.0);
        assert!(e(T::CrossFade, 30, (3840, 2160), Quality::Standard).1 > base * 3);
        assert!(e(T::CrossFade, 30, (1920, 1080), Quality::High).1 > base);
        assert!(e(T::CrossFade, 30, (1920, 1080), Quality::Compact).1 < base);
        assert!(e(T::KenBurns, 30, (1920, 1080), Quality::Standard).1 > base * 2);
        assert!(e(T::None, 30, (1920, 1080), Quality::Standard).1 < base);
        assert!(
            e(T::KenBurns, 60, (1920, 1080), Quality::Standard).1
                > e(T::KenBurns, 30, (1920, 1080), Quality::Standard).1
        );
        // High against Baseline: about as large with Ken Burns (the pans cost the same; High
        // looks better at that size), a third smaller on cross-fades between stills.
        let hb = |t| {
            let at = |p| estimate(20, 5.0, t, 30, (1920, 1080), Quality::Standard, p).1 as f64;
            at(Profile::High) / at(Profile::Baseline)
        };
        let (kb, fade) = (hb(T::KenBurns), hb(T::CrossFade));
        assert!((0.85..1.0).contains(&kb), "{kb}");
        assert!((0.55..0.75).contains(&fade) && fade < kb, "{fade}");
    }

    /// The estimate against the encoder on a small synthetic show: three slides of 2 s at
    /// 384×224, 24 fps, Standard, each transition with High and Baseline. Within 40 %
    /// (measured 2026-10: High 1.08, 0.94 and 0.98 of the coded size for cuts, cross-fades
    /// and Ken Burns; Baseline 0.94, 0.69 and 0.88, its cross-fades between these slides
    /// costing relatively more than between photos).
    #[test]
    fn estimate_is_near_the_coded_size() {
        use SlideshowTransition as T;
        use specular_h264enc::{Config, Encoder, Frame};
        let (w, h, fps, interval) = (384, 224, 24, 2.0);
        let slides: Vec<_> = (1..=3)
            .map(|i| synthetic_slide(w * SS, h * SS, i))
            .collect();
        let q = Quality::Standard;
        for t in [T::None, T::CrossFade, T::KenBurns] {
            let pics = show_pictures(&slides, w, h, fps, interval, t);
            for profile in PROFILES {
                let mut cfg = Config::new(w as u32, h as u32, fps, 1, q.qp());
                cfg.profile = profile;
                cfg.keyint = fps * 5;
                let mut enc = Encoder::new(cfg).unwrap();
                let coded: usize = pics
                    .iter()
                    .map(|p| {
                        let f = Frame {
                            y: &p.planes[0],
                            u: &p.planes[1],
                            v: &p.planes[2],
                            y_stride: w,
                            c_stride: w / 2,
                        };
                        enc.encode_with(&f, p.idr, p.hints).data.len()
                    })
                    .sum();
                let (_, est) = estimate(3, interval, t, fps, (w as u32, h as u32), q, profile);
                // Without the MP4's boxes, which this doesn't write.
                let est = est as f64 - 4096.0 - 8.0 * pics.len() as f64;
                let r = est / coded as f64;
                assert!(
                    (0.6..1.4).contains(&r),
                    "{t:?} {profile:?}: estimate {est:.0}, coded {coded} ({r:.2})"
                );
            }
        }
    }

    /// Slides are made at this multiple of the video's size and scaled down, as photos are.
    const SS: usize = 2;

    /// A photo-like test slide: `w`×`h` 4:2:0 planes of shading with detail at every scale
    /// and thousands of hard-edged shapes of every size (edges are most of what moving
    /// pictures cost).
    fn synthetic_slide(w: usize, h: usize, seed: u64) -> [Vec<u8>; 3] {
        let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut rnd = move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 11) as f32 / (1u64 << 53) as f32
        };
        // Value noise: octaves of bilinear random grids, cells of 64 down to 1 sample.
        let mut noise = |w: usize, h: usize, amp0: f32, fall: f32| {
            let mut out = vec![0f32; w * h];
            let (mut cell, mut amp) = (64, amp0);
            while cell >= 1 {
                let gw = w / cell + 2;
                let g: Vec<f32> = (0..gw * (h / cell + 2)).map(|_| rnd() - 0.5).collect();
                let at = |i: usize, j: usize| g[j * gw + i];
                for y in 0..h {
                    let (fy, ty) = (y / cell, (y % cell) as f32 / cell as f32);
                    for x in 0..w {
                        let (fx, tx) = (x / cell, (x % cell) as f32 / cell as f32);
                        let top = at(fx, fy) * (1.0 - tx) + at(fx + 1, fy) * tx;
                        let bot = at(fx, fy + 1) * (1.0 - tx) + at(fx + 1, fy + 1) * tx;
                        out[y * w + x] += amp * (top * (1.0 - ty) + bot * ty);
                    }
                }
                cell /= 2;
                amp *= fall;
            }
            out
        };
        let mut y = noise(w, h, 50.0, 0.6);
        let u = noise(w / 2, h / 2, 50.0, 0.4);
        let v = noise(w / 2, h / 2, 50.0, 0.4);
        // Flat discs and boxes over the shading, most of them small.
        for _ in 0..4000 {
            let (cx, cy) = (rnd() * w as f32, rnd() * h as f32);
            let a = rnd();
            let r = 2.0 + a * a * w as f32 / 16.0;
            let level = rnd() * 120.0 - 60.0;
            let disc = rnd() < 0.5;
            let span =
                |c: f32, n: usize| (c - r).max(0.0) as usize..((c + r + 1.0) as usize).min(n);
            for yy in span(cy, h) {
                for xx in span(cx, w) {
                    let (dx, dy) = (xx as f32 - cx, yy as f32 - cy);
                    let inside = if disc {
                        dx * dx + dy * dy < r * r
                    } else {
                        dx.abs() < r && dy.abs() < r * 0.6
                    };
                    if inside {
                        y[yy * w + xx] = y[yy * w + xx] * 0.5 + level;
                    }
                }
            }
        }
        let q = |v: &[f32], mid: f32| -> Vec<u8> {
            v.iter()
                .map(|&v| (v + mid).round().clamp(16.0, 235.0) as u8)
                .collect()
        };
        [q(&y, 126.0), q(&u, 128.0), q(&v, 128.0)]
    }

    /// Where a test slide is shown: zoom `z` about point `c` (in video samples, the slide
    /// fitting the video at `z` 1).
    #[derive(Clone, Copy, PartialEq)]
    struct Shot {
        z: f32,
        c: (f32, f32),
    }

    impl Shot {
        /// Slide `i` at Ken Burns progress `p` (fitted when `None`): a zoom in or out by 8 %
        /// and a pan across what the zoom leaves, as the live show's.
        fn at(i: usize, p: Option<f32>, w: usize, h: usize) -> Shot {
            let mid = (w as f32 * 0.5, h as f32 * 0.5);
            let Some(p) = p else {
                return Shot { z: 1.0, c: mid };
            };
            let z = 1.0 + 0.08 * if i.is_multiple_of(2) { p } else { 1.0 - p };
            let (dx, dy) = [(0.8, -0.5), (-0.6, 0.9), (0.3, 0.7)][i % 3];
            let t = 2.0 * p - 1.0;
            let o = |m: f32, d: f32| m + d * t * m * (1.0 - 1.0 / z);
            Shot {
                z,
                c: (o(mid.0, dx), o(mid.1, dy)),
            }
        }

        /// The encoder's motion hint from `prev`.
        fn hint_after(&self, prev: &Shot, w: usize, h: usize) -> MotionHint {
            let k = prev.z / self.z;
            let d = |c: f32, pc: f32, n: usize| {
                let m = n as f32 * 0.5;
                (c - pc) * prev.z + (0.5 - m) * k + m - 0.5
            };
            MotionHint {
                scale: k,
                dx: d(self.c.0, prev.c.0, w),
                dy: d(self.c.1, prev.c.1, h),
            }
        }

        /// The `w`×`h` picture of slide `s` (planes `SS` times as large): `SS`×`SS`
        /// bilinear samples averaged per sample.
        fn render(&self, s: &[Vec<u8>; 3], w: usize, h: usize) -> [Vec<u8>; 3] {
            let ss = SS as f32;
            let plane = |src: &[u8], pw: usize, ph: usize, sub: f32| {
                let (sw, sh) = (pw * SS, ph * SS);
                // The slide sample at position `v` of this plane.
                let at = |v: f32, c: f32, n: usize, len: usize| {
                    ((c + (v * sub - n as f32 * 0.5) / self.z) * ss / sub - 0.5)
                        .clamp(0.0, (len - 1) as f32)
                };
                let mut out = vec![0u8; pw * ph];
                for y in 0..ph {
                    for x in 0..pw {
                        let mut sum = 0.0;
                        for j in 0..SS {
                            for i in 0..SS {
                                let o = |k: usize| (k as f32 + 0.5) / ss;
                                let sx = at(x as f32 + o(i), self.c.0, w, sw);
                                let sy = at(y as f32 + o(j), self.c.1, h, sh);
                                let (x0, y0) = (sx as usize, sy as usize);
                                let (x1, y1) = ((x0 + 1).min(sw - 1), (y0 + 1).min(sh - 1));
                                let (fx, fy) = (sx - x0 as f32, sy - y0 as f32);
                                let p = |i: usize, j: usize| f32::from(src[j * sw + i]);
                                let top = p(x0, y0) * (1.0 - fx) + p(x1, y0) * fx;
                                let bot = p(x0, y1) * (1.0 - fx) + p(x1, y1) * fx;
                                sum += top * (1.0 - fy) + bot * fy;
                            }
                        }
                        out[y * pw + x] = (sum / (ss * ss)).round() as u8;
                    }
                }
                out
            };
            [
                plane(&s[0], w, h, 1.0),
                plane(&s[1], w / 2, h / 2, 2.0),
                plane(&s[2], w / 2, h / 2, 2.0),
            ]
        }
    }

    /// One picture of a test show, as the export hands it to the encoder.
    struct Pic {
        planes: [Vec<u8>; 3],
        idr: bool,
        hints: Hints,
    }

    /// The pictures of a show of `slides` on the export's clock: hard cuts, cross-fades from
    /// the outgoing slide's last picture, Ken Burns with the motion hints.
    fn show_pictures(
        slides: &[[Vec<u8>; 3]],
        w: usize,
        h: usize,
        fps: u32,
        interval: f64,
        transition: SlideshowTransition,
    ) -> Vec<Pic> {
        let mut out: Vec<Pic> = Vec::new();
        let mut placed: Option<Shot> = None;
        for (i, s) in slides.iter().enumerate() {
            let clock = SlideClock {
                fps,
                interval,
                transition,
                after_another: i > 0,
            };
            let outgoing = out.last().map(|p| p.planes.clone());
            let mut still: Option<[Vec<u8>; 3]> = None;
            for k in 0..frames_for(interval, fps) {
                let m = clock.moment(k);
                let now = Shot::at(i, m.ken_burns, w, h);
                let mut planes = match (&still, m.ken_burns) {
                    (Some(p), None) => p.clone(),
                    _ => now.render(s, w, h),
                };
                if m.ken_burns.is_none() {
                    still = Some(planes.clone());
                }
                if let (Some(a), Some(o)) = (m.fade, &outgoing) {
                    for (p, o) in planes.iter_mut().zip(o) {
                        for (v, &ov) in p.iter_mut().zip(o) {
                            *v = (f32::from(ov) * a + f32::from(*v) * (1.0 - a)).round() as u8;
                        }
                    }
                }
                let motion = placed
                    .filter(|p| *p != now && m.fade.is_none())
                    .map(|p| now.hint_after(&p, w, h));
                placed = Some(now);
                out.push(Pic {
                    planes,
                    idr: m.cut,
                    hints: Hints {
                        motion,
                        fading: m.fade.is_some().then_some(true),
                    },
                });
            }
        }
        out
    }
}
