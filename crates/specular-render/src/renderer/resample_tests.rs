//! Resampling and pixel accuracy of the display path, measured on test charts drawn by the
//! renderer at many zooms into a half-float target (linear light, before the compositor's
//! encode and dither): pixel-exact placement at 1× and at nearest-neighbour zooms, impulse
//! centroids and DC gain, linear-light downscale means, zone-plate aliasing and pass-band
//! energy, slanted-edge MTF50 and the precision of shallow 16-bit and float ramps.
//! `tools/quality/README.md` lists the metrics and their tolerances. GPU tests skip without
//! an adapter.

use specular_core::hdr::{self, Primaries};
use specular_core::{PixelLayout, Raster, RasterPlane};
use specular_gfx::Gpu;
use specular_inspect::ChannelView;

use super::hdr_tests::{draw_scaled, gpu};
use super::*;

fn srgb_decode(e: f64) -> f64 {
    if e <= 0.040_45 {
        e / 12.92
    } else {
        ((e + 0.055) / 1.055).powf(2.4)
    }
}

fn srgb_encode(l: f64) -> f64 {
    let l = l.clamp(0.0, 1.0);
    if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    }
}

/// An 8-bit grey chart from sRGB codes.
fn gray8(w: u32, h: u32, px: impl Fn(u32, u32) -> u8) -> Raster {
    let data = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| px(x, y))
        .collect();
    plane_raster(w, h, PixelLayout::Gray, w as usize, data)
}

/// A 16-bit grey chart from linear-light values (sRGB-coded, like a 16-bit PNG).
fn gray16(w: u32, h: u32, lin: impl Fn(u32, u32) -> f64) -> Raster {
    let mut data = Vec::with_capacity((w * h * 2) as usize);
    for y in 0..h {
        for x in 0..w {
            let c = (srgb_encode(lin(x, y)) * 65535.0).round() as u16;
            data.extend_from_slice(&c.to_ne_bytes());
        }
    }
    plane_raster(w, h, PixelLayout::Gray16, w as usize * 2, data)
}

/// A linear half-float grey chart (values may exceed 1).
fn grey_f16(w: u32, h: u32, lin: impl Fn(u32, u32) -> f64) -> Raster {
    let mut data = Vec::with_capacity((w * h * 8) as usize);
    for y in 0..h {
        for x in 0..w {
            let v = hdr::f32_to_f16(lin(x, y) as f32);
            for c in [v, v, v, hdr::f32_to_f16(1.0)] {
                data.extend_from_slice(&c.to_ne_bytes());
            }
        }
    }
    plane_raster(
        w,
        h,
        PixelLayout::Rgba16F(Primaries::Bt709),
        w as usize * 8,
        data,
    )
}

fn plane_raster(w: u32, h: u32, layout: PixelLayout, stride: usize, data: Vec<u8>) -> Raster {
    Raster {
        width: w,
        height: h,
        origin: (0, 0),
        scale: 1,
        layout,
        planes: vec![RasterPlane {
            width: w,
            height: h,
            stride,
            data,
            h_div: 1,
            v_div: 1,
        }],
        truncated: false,
    }
}

/// A drawn chart: linear grey per output pixel and the output → source mapping.
struct Out {
    w: u32,
    h: u32,
    s: f64,
    ox: f64,
    oy: f64,
    v: Vec<f64>,
}

impl Out {
    fn at(&self, x: u32, y: u32) -> f64 {
        self.v[(y * self.w + x) as usize]
    }

    /// Source position of output pixel (x, y)'s centre.
    fn src(&self, x: u32, y: u32) -> (f64, f64) {
        (
            (x as f64 + 0.5 - self.ox) / self.s,
            (y as f64 + 0.5 - self.oy) / self.s,
        )
    }

    /// Output pixels whose footprint (± `margin` source pixels) lies inside the image.
    fn interior(&self, iw: u32, ih: u32, margin: f64) -> impl Iterator<Item = (u32, u32)> + '_ {
        let (iw, ih) = (iw as f64, ih as f64);
        (0..self.h)
            .flat_map(move |y| (0..self.w).map(move |x| (x, y)))
            .filter(move |&(x, y)| {
                let (sx, sy) = self.src(x, y);
                sx > margin && sy > margin && sx < iw - margin && sy < ih - margin
            })
    }
}

fn render(gpu: &Gpu, r: &mut ImageRenderer, raster: &Raster, s: f32, opts: RenderOptions) -> Out {
    let mut img = r.create_image(gpu, 41, raster.width, raster.height, None);
    r.upload_overview(gpu, &mut img, raster);
    let px = draw_scaled(gpu, r, &img, opts, s);
    r.release_image(img.id);
    let (iw, ih) = (raster.width as f64, raster.height as f64);
    let w = (raster.width as f32 * s).ceil() as u32;
    let h = (raster.height as f32 * s).ceil() as u32;
    // View::fixed keeps log2(zoom): use the scale the shader sees.
    let s = (s.log2()).exp2() as f64;
    Out {
        w,
        h,
        s,
        ox: w as f64 * 0.5 - iw * 0.5 * s,
        oy: h as f64 * 0.5 - ih * 0.5 * s,
        v: px.iter().map(|p| p[1] as f64).collect(),
    }
}

fn hq() -> RenderOptions {
    RenderOptions::default()
}

/// Lanczos without the anti-ringing clamp: the kernel itself.
fn kernel() -> RenderOptions {
    RenderOptions {
        anti_ringing: false,
        ..hq()
    }
}

fn pseudo(x: u32, y: u32) -> u8 {
    let mut h = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    (h >> 24) as u8
}

#[test]
fn pixels_land_exactly_at_one_and_nearest_zooms() {
    let Some(gpu) = gpu() else { return };
    let mut r = ImageRenderer::new(&gpu, 32 << 20);
    let (iw, ih) = (61, 37);
    let chart = gray8(iw, ih, pseudo);
    for s in [1.0f32, 4.0, 5.0, 8.0] {
        let o = render(&gpu, &mut r, &chart, s, hq());
        let mut worst = 0f64;
        for y in 0..o.h {
            for x in 0..o.w {
                let (sx, sy) = o.src(x, y);
                let want = pseudo(sx.floor() as u32, sy.floor() as u32) as f64;
                // In 8-bit code space: a misplaced pixel lands on an unrelated pseudo-random
                // code. The shader decodes the stored codes exactly (the texture unit's own
                // sRGB decode kept about nine bits on AMD, 0.34 of a code off); what is left
                // is the half-float target's truncation (0.08). It must stay inside the final
                // pass's undithered zone (0.125) for 1:1 output to be bit-exact.
                let got = srgb_encode(o.at(x, y)) * 255.0;
                let err = (got - want).abs();
                worst = worst.max(err);
                assert!(
                    err < 0.125,
                    "zoom {s}: output ({x}, {y}) is code {got} want {want}"
                );
            }
        }
        eprintln!("placement zoom {s}: worst error {worst:.3} 8-bit codes");
    }
}

/// A float impulse on a 0.25 grey (so negative lobes are not clipped), at several positions
/// on an odd-sized image whose mips are not exact halves. Its height grows as 1/zoom² when
/// reducing so that the half-float target's rounding (up to half a step on every pixel it
/// touches) stays small against its energy.
#[test]
fn impulses_stay_centred_and_keep_their_energy() {
    let Some(gpu) = gpu() else { return };
    let mut r = ImageRenderer::new(&gpu, 32 << 20);
    let (iw, ih) = (101, 97);
    let py = 47u32;
    let mut bad = Vec::new();
    // The kernel itself, then as displayed: the anti-ringing clamp cuts the negative lobes
    // around an isolated impulse, which adds their (negative) energy back and may shift the
    // centroid a little where the lobe window is asymmetric.
    for (opts, what) in [(kernel(), "kernel"), (hq(), "anti-ringing")] {
        for s in [0.25f32, 0.33, 0.5, 0.7, 1.5, 2.0, 3.0] {
            let (mut wdx, mut wdy, mut wgain) = (0f64, 0f64, 0f64);
            // The background as drawn, subtracted pixel by pixel: half-float storage leaves parts
            // of a flat 0.25 one step (1.2e-4) low, which summed over the window would bias the
            // centroid.
            let flat = render(&gpu, &mut r, &grey_f16(iw, ih, |_, _| 0.25), s, opts);
            let amp = (0.5 / (s as f64 * s as f64)).max(0.5);
            for px in 48..54u32 {
                let chart = grey_f16(iw, ih, |x, y| {
                    0.25 + if (x, y) == (px, py) { amp } else { 0.0 }
                });
                let o = render(&gpu, &mut r, &chart, s, opts);
                let (mut sum, mut cx, mut cy) = (0.0, 0.0, 0.0);
                for y in 0..o.h {
                    for x in 0..o.w {
                        let (sx, sy) = o.src(x, y);
                        // Window of ±24 source pixels: at 1/4 the mip and the final Lanczos reach ±18.
                        if (sx - px as f64 - 0.5).abs() > 24.0
                            || (sy - py as f64 - 0.5).abs() > 24.0
                        {
                            continue;
                        }
                        let v = o.at(x, y) - flat.at(x, y);
                        sum += v;
                        cx += v * (x as f64 + 0.5);
                        cy += v * (y as f64 + 0.5);
                    }
                }
                let dx = cx / sum - (o.ox + (px as f64 + 0.5) * o.s);
                let dy = cy / sum - (o.oy + (py as f64 + 0.5) * o.s);
                let gain = sum / (amp * o.s * o.s);
                for (w, v) in [(&mut wdx, dx), (&mut wdy, dy), (&mut wgain, gain - 1.0)] {
                    if v.abs() > w.abs() {
                        *w = v;
                    }
                }
            }
            eprintln!(
                "impulse {what} zoom {s}: worst centroid offset ({wdx:+.4}, {wdy:+.4}) output px, DC gain error {wgain:+.4}"
            );
            // Lanczos-3 sampled at a fractional phase moves the centroid and the summed gain a
            // little (simulated in float64 for this chart and mip chain: ±0.02 px, ±0.5% per axis).
            // Below 1/2 the reduced mip levels are drawn from, and those are built with a clamp
            // to the range of their taps (mip.wgsl), which also removes negative lobes: the gain
            // may rise there in both modes (measured +0.28 kernel, +0.69 with the clamp).
            let (dmax, gain_ok) = if s < 0.5 {
                (
                    if what == "kernel" { 0.03 } else { 0.1 },
                    (-0.02..=0.8).contains(&wgain),
                )
            } else if what == "kernel" {
                (0.03, wgain.abs() <= 0.02)
            } else {
                // The clamp only removes negative lobes: the gain may rise, never fall.
                (0.1, (-0.02..=0.6).contains(&wgain))
            };
            if wdx.abs() > dmax || wdy.abs() > dmax || !gain_ok {
                bad.push(format!(
                    "{what} zoom {s}: centroid ({wdx:+.4}, {wdy:+.4}), gain {wgain:+.4}"
                ));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn downscaling_averages_in_linear_light() {
    let Some(gpu) = gpu() else { return };
    let mut r = ImageRenderer::new(&gpu, 32 << 20);
    let (iw, ih) = (96, 96);
    let charts: [(&str, Raster, f64); 3] = [
        (
            "checker",
            gray8(iw, ih, |x, y| if (x + y) % 2 == 0 { 255 } else { 0 }),
            0.5,
        ),
        (
            "stripes",
            gray8(iw, ih, |x, _| if x % 2 == 0 { 255 } else { 0 }),
            0.5,
        ),
        (
            "grid 1 in 4",
            gray8(
                iw,
                ih,
                |x, y| if x % 4 == 0 || y % 4 == 0 { 255 } else { 0 },
            ),
            7.0 / 16.0,
        ),
    ];
    for (name, chart, want) in &charts {
        for s in [0.125f32, 0.2, 0.25, 0.37, 0.5, 0.75] {
            // The grid's period of 4 is only above the output Nyquist frequency below 1/2.
            if name.starts_with("grid") && s >= 0.5 {
                continue;
            }
            let o = render(&gpu, &mut r, chart, s, hq());
            let (mut sum, mut n, mut dev) = (0.0, 0.0, 0f64);
            for (x, y) in o.interior(iw, ih, 8.0) {
                sum += o.at(x, y);
                n += 1.0;
            }
            let mean = sum / n;
            for (x, y) in o.interior(iw, ih, 8.0) {
                dev = dev.max((o.at(x, y) - want).abs());
            }
            eprintln!("{name} zoom {s}: mean {mean:.4} (want {want:.4}), worst pixel {dev:.4}");
            // A gamma-space average would give 0.214 for the checker.
            assert!((mean - want).abs() < 0.01, "{name} zoom {s}: mean {mean}");
            // Above the output Nyquist frequency the pattern must not survive as moiré.
            assert!(dev < 0.06, "{name} zoom {s}: moiré of {dev}");
        }
    }
}

/// Zone plate `0.5 + 0.4 cos(π r² / n)`: local frequency r / n cycles per source pixel.
fn zone_plate(size: u32) -> (Raster, f64) {
    let n = size as f64;
    let c = size as f64 * 0.5;
    let chart = gray16(size, size, |x, y| {
        let (dx, dy) = (x as f64 + 0.5 - c, y as f64 + 0.5 - c);
        0.5 + 0.4 * (std::f64::consts::PI * (dx * dx + dy * dy) / n).cos()
    });
    (chart, n)
}

#[derive(Default)]
struct Band {
    sq: f64,
    n: f64,
}

impl Band {
    fn add(&mut self, d: f64) {
        self.sq += d * d;
        self.n += 1.0;
    }
    /// RMS deviation relative to the chart's (0.4 · √½).
    fn rel(&self) -> f64 {
        (self.sq / self.n.max(1.0)).sqrt() / (0.4 * std::f64::consts::FRAC_1_SQRT_2)
    }
}

/// (pass-band amplitude kept, stop-band amplitude left = aliasing) over the zone plate.
fn zone_metrics(o: &Out, size: u32, n: f64) -> (f64, f64) {
    let c = size as f64 * 0.5;
    let nyq = o.s * 0.5;
    let (mut pass, mut stop) = (Band::default(), Band::default());
    for (x, y) in o.interior(size, size, 6.0) {
        let (sx, sy) = o.src(x, y);
        let rad = ((sx - c).powi(2) + (sy - c).powi(2)).sqrt();
        if rad > c - 4.0 {
            continue;
        }
        // Local frequency (sx − c, sy − c) / n. The output grid's Nyquist region is a square,
        // so the band is decided by the larger component, not the radius.
        let f = (sx - c).abs().max((sy - c).abs()) / n;
        let d = o.at(x, y) - 0.5;
        if f < 0.4 * nyq {
            pass.add(d);
        } else if f > 1.3 * nyq && f < 0.5 {
            stop.add(d);
        }
    }
    (pass.rel(), stop.rel())
}

#[test]
fn zone_plate_aliasing() {
    let Some(gpu) = gpu() else { return };
    let mut r = ImageRenderer::new(&gpu, 32 << 20);
    let size = 256;
    let (chart, n) = zone_plate(size);
    for s in [0.25f32, 0.33, 0.5, 0.6, 0.8] {
        let o = render(&gpu, &mut r, &chart, s, hq());
        let (pass, alias) = zone_metrics(&o, size, n);
        let ob = render(&gpu, &mut r, &chart, s, RenderOptions { hq: false, ..hq() });
        let (bpass, balias) = zone_metrics(&ob, size, n);
        eprintln!(
            "zone plate zoom {s}: Lanczos pass {pass:.3} alias {alias:.3}; bilinear pass {bpass:.3} alias {balias:.3}"
        );
        // Lanczos-3 keeps the pass band within its ripple and leaves under 5% above 1.3 × Nyquist
        // (its continuous response there is 0.07 at most, 0.02 RMS).
        assert!(
            (0.9..1.1).contains(&pass),
            "zoom {s}: pass band kept {pass}"
        );
        assert!(alias < 0.05, "zoom {s}: aliasing {alias}");
    }
}

/// MTF50 in cycles per source pixel from a slanted edge (ISO 12233 style: oversampled edge
/// spread function, differentiated, Hann-windowed, DFT).
fn mtf50(o: &Out, angle: f64, centre: (f64, f64), lo: f64, hi: f64) -> f64 {
    let (sn, cs) = angle.sin_cos();
    const OS: f64 = 8.0; // bins per source pixel
    const HALF: f64 = 8.0; // source pixels each side of the edge
    let bins = (2.0 * HALF * OS) as usize;
    let mut acc = vec![0.0; bins];
    let mut cnt = vec![0.0; bins];
    for y in 0..o.h {
        for x in 0..o.w {
            let (sx, sy) = o.src(x, y);
            // Signed distance from the edge through `centre` (normal (cos, sin)).
            let d = (sx - centre.0) * cs + (sy - centre.1) * sn;
            let along = -(sx - centre.0) * sn + (sy - centre.1) * cs;
            if d.abs() >= HALF || along.abs() > 20.0 {
                continue;
            }
            let b = ((d + HALF) * OS) as usize;
            acc[b] += (o.at(x, y) - lo) / (hi - lo);
            cnt[b] += 1.0;
        }
    }
    let esf: Vec<f64> = acc
        .iter()
        .zip(&cnt)
        .map(|(a, c)| if *c > 0.0 { a / c } else { f64::NAN })
        .collect();
    // Fill empty bins (rare at low oversampling) linearly.
    let mut esf2 = esf.clone();
    for (i, v) in esf2.iter_mut().enumerate() {
        if v.is_nan() {
            let prev = (0..i).rev().find(|&j| !esf[j].is_nan());
            let next = (i + 1..bins).find(|&j| !esf[j].is_nan());
            *v = match (prev, next) {
                (Some(p), Some(n)) => esf[p] + (esf[n] - esf[p]) * (i - p) as f64 / (n - p) as f64,
                (Some(p), None) => esf[p],
                (None, Some(n)) => esf[n],
                _ => 0.0,
            };
        }
    }
    let m = bins - 1;
    let lsf: Vec<f64> = (0..m)
        .map(|i| {
            let wnd = 0.5 - 0.5 * (2.0 * std::f64::consts::PI * (i as f64 + 0.5) / m as f64).cos();
            (esf2[i + 1] - esf2[i]) * wnd
        })
        .collect();
    let dc: f64 = lsf.iter().sum();
    let mtf = |f: f64| {
        // f in cycles per source pixel; sample spacing 1 / OS source pixels.
        let (mut re, mut im) = (0.0, 0.0);
        for (i, v) in lsf.iter().enumerate() {
            let ph = 2.0 * std::f64::consts::PI * f * i as f64 / OS;
            re += v * ph.cos();
            im += v * ph.sin();
        }
        (re * re + im * im).sqrt() / dc
    };
    let mut prev = (0.0, 1.0);
    let mut f = 0.0;
    while f < 2.0 {
        f += 0.005;
        let v = mtf(f);
        if v <= 0.5 {
            return prev.0 + (f - prev.0) * (prev.1 - 0.5) / (prev.1 - v);
        }
        prev = (f, v);
    }
    f
}

#[test]
fn slanted_edge_mtf50() {
    let Some(gpu) = gpu() else { return };
    let mut r = ImageRenderer::new(&gpu, 32 << 20);
    let size = 64u32;
    let angle = 5f64.to_radians();
    let centre = (size as f64 * 0.5 + 0.3, size as f64 * 0.5);
    let (lo, hi) = (0.2, 0.8);
    let (sn, cs) = angle.sin_cos();
    // Area-sampled ideal edge (16 × 16 supersamples per pixel).
    let chart = gray16(size, size, |x, y| {
        let mut on = 0;
        for j in 0..16 {
            for i in 0..16 {
                let sx = x as f64 + (i as f64 + 0.5) / 16.0;
                let sy = y as f64 + (j as f64 + 0.5) / 16.0;
                if (sx - centre.0) * cs + (sy - centre.1) * sn > 0.0 {
                    on += 1;
                }
            }
        }
        lo + (hi - lo) * on as f64 / 256.0
    });
    let no_nearest = RenderOptions {
        nearest_from: f32::INFINITY,
        ..hq()
    };
    let at1 = mtf50(
        &render(&gpu, &mut r, &chart, 1.0, hq()),
        angle,
        centre,
        lo,
        hi,
    );
    eprintln!("slanted edge zoom 1: MTF50 {at1:.3} cycles/px (pixel aperture alone: 0.603)");
    assert!((at1 - 0.603).abs() < 0.03, "zoom 1 MTF50 {at1}");
    // Lanczos (with its anti-ringing clamp) runs up to the nearest-neighbour zoom.
    for s in [1.5f32, 2.0, 3.0, 4.0] {
        let l = mtf50(
            &render(&gpu, &mut r, &chart, s, no_nearest),
            angle,
            centre,
            lo,
            hi,
        );
        let b = mtf50(
            &render(
                &gpu,
                &mut r,
                &chart,
                s,
                RenderOptions {
                    hq: false,
                    ..no_nearest
                },
            ),
            angle,
            centre,
            lo,
            hi,
        );
        eprintln!("slanted edge zoom {s}: MTF50 Lanczos {l:.3}, bilinear {b:.3} cycles/source px");
        assert!(l > 0.38, "zoom {s}: Lanczos MTF50 {l}");
        assert!(l > b, "zoom {s}: Lanczos {l} not sharper than bilinear {b}");
    }
}

/// Hard edges must not ring: a step, a one-texel line and a one-texel gap (sRGB codes 40 and
/// 220, so undershoot is not hidden by the clamp at 0) drawn with Lanczos from the overview
/// and from tiles, in the normal view and in a channel view (the straight colour in
/// `hidden`), stay within the source range ± 1/255. Without the clamp Lanczos-3 overshoots
/// a step by about 10 % of its height.
/// Where a step edge lands, against where the view maps it, at fit-like odd zooms, 2:1, 1:2,
/// 1:1 and nearest zooms, for vertical and horizontal edges at nine source positions each.
/// The position is measured by area: over a window [a, b) of output pixels around the edge,
/// an ideal step at e leaves (e - a)(hi - lo) of missing signal, so e = a + sum (hi - v)/(hi - lo).
/// A symmetric blur leaves that sum unchanged; sampling the blurred edge on the output grid
/// moves it a little depending on its phase (measured: Lanczos 0.008 px, with the clamp 0.043,
/// bilinear 0.08). A
/// half-pixel shift, or any systematic one, shows in the mean over the nine phases.
#[test]
fn edges_land_on_the_right_pixel() {
    let Some(gpu) = gpu() else { return };
    let mut r = ImageRenderer::new(&gpu, 32 << 20);
    let (iw, ih) = (97u32, 61u32);
    let (lo, hi) = (0.1, 0.6);
    // Edge position (output px) from the samples `v` around the expected position `e0`.
    let area = |v: &[f64], e0: f64, half: f64| -> f64 {
        // Symmetric, and clear of the first and last output pixels (part canvas).
        let half = half.min(e0 - 1.0).min(v.len() as f64 - 1.0 - e0).floor();
        let a = (e0 - half).floor() as usize;
        let b = (e0 + half).ceil() as usize;
        // The plateaus as drawn: half-float storage at each mip level moves them by a few
        // 1e-4, which summed over the window would read as a shift.
        let (l, h) = (v[a], v[b - 1]);
        a as f64 + v[a..b].iter().map(|x| (h - x) / (h - l)).sum::<f64>()
    };
    let bilinear = RenderOptions { hq: false, ..hq() };
    let mut bad = Vec::new();
    for (opts, what, phase_tol) in [
        (kernel(), "kernel", 0.03),
        (hq(), "anti-ringing", 0.06),
        (bilinear, "bilinear", 0.1),
    ] {
        let (mut worst, mut worst_mean) = (0f64, 0f64);
        for s in [0.31f32, 0.5, 0.6213, 1.0, 1.37, 2.0, 3.0, 4.0, 5.0] {
            let (mut sum, mut n) = (0f64, 0f64);
            for k in 0..9u32 {
                let (kx, ky) = (38 + k, 25 + k);
                let vchart = grey_f16(iw, ih, |x, _| if x >= kx { hi } else { lo });
                let hchart = grey_f16(iw, ih, |_, y| if y >= ky { hi } else { lo });
                let ov = render(&gpu, &mut r, &vchart, s, opts);
                let oh = render(&gpu, &mut r, &hchart, s, opts);
                let (y0, x0) = (ov.h / 2, oh.w / 2);
                let row: Vec<f64> = (0..ov.w).map(|x| ov.at(x, y0)).collect();
                let col: Vec<f64> = (0..oh.h).map(|y| oh.at(x0, y)).collect();
                // Lanczos-3 reaches 3 output px when reducing, 3s when enlarging.
                let half = (4.0 * s as f64).max(8.0);
                let (ex, ey) = (ov.ox + kx as f64 * ov.s, oh.oy + ky as f64 * oh.s);
                for d in [area(&row, ex, half) - ex, area(&col, ey, half) - ey] {
                    worst = worst.max(d.abs());
                    sum += d;
                    n += 1.0;
                    if d.abs() > phase_tol {
                        bad.push(format!("{what} zoom {s}: edge {k} off by {d:+.3} px"));
                    }
                }
            }
            let mean = sum / n;
            worst_mean = worst_mean.max(mean.abs());
            if mean.abs() > 0.015 {
                bad.push(format!(
                    "{what} zoom {s}: edges shifted by {mean:+.3} px on average"
                ));
            }
        }
        eprintln!(
            "edge placement {what}: worst {worst:.4} output px, worst mean over phases {worst_mean:.4}"
        );
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn hard_edges_do_not_ring() {
    let Some(gpu) = gpu() else { return };
    let mut r = ImageRenderer::new(&gpu, 64 << 20);
    let n = 64u32;
    let (lo, hi) = (40u8, 220u8);
    let code = |x: u32, y: u32| {
        let v = x >= 20 || x == 9 || y == 50;
        if (x == 40 && y < 30) || (y == 12 && x > 24) {
            !v
        } else {
            v
        }
    };
    let rgba = |w: u32, h: u32, step: u32| {
        let data = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .flat_map(|(x, y)| {
                let v = if code(x * step, y * step) { hi } else { lo };
                [v, v, v, 255]
            })
            .collect();
        plane_raster(w, h, PixelLayout::Rgba, w as usize * 4, data)
    };
    let black = [0.0, 0.0, 0.0, 1.0];
    let mut worst = 0f64;
    for tiled in [false, true] {
        let mut img = r.create_image(&gpu, 50 + tiled as u64, n, n, None);
        if tiled {
            // A half-size overview, so tiles serve every zoom from 1 up.
            r.upload_overview(&gpu, &mut img, &rgba(n / 2, n / 2, 2));
            assert_eq!(
                r.upload_tiles(&gpu, &mut img, &rgba(n, n, 1), 0, &[(0, 0)]),
                1
            );
        } else {
            r.upload_overview(&gpu, &mut img, &rgba(n, n, 1));
        }
        let scales: &[f32] = if tiled {
            &[1.0, 1.5, 2.0, 2.5, 3.0, 4.0]
        } else {
            &[0.7, 1.0, 1.5, 2.0, 2.5, 3.0, 3.7, 4.0]
        };
        for &s in scales {
            for channel in [ChannelView::Normal, ChannelView::Red] {
                let opts = RenderOptions {
                    nearest_from: 100.0,
                    channel,
                    checker_a: black,
                    checker_b: black,
                    ..hq()
                };
                let px = draw_scaled(&gpu, &mut r, &img, opts, s);
                let w = (n as f32 * s).ceil() as usize;
                let h = px.len() / w;
                for (i, p) in px.iter().enumerate() {
                    // The outermost pixels blend with the canvas by their coverage.
                    if i % w < 2 || i / w < 2 || i % w + 2 >= w || i / w + 2 >= h {
                        continue;
                    }
                    let got = srgb_encode(p[0] as f64) * 255.0;
                    let over = (lo as f64 - got).max(got - hi as f64);
                    worst = worst.max(over);
                    assert!(
                        over <= 1.0,
                        "tiled {tiled} zoom {s} {channel:?}: output ({}, {}) is code {got:.2}, \
                         outside {lo}..={hi}",
                        i % w,
                        i / w
                    );
                }
            }
        }
        r.release_image(img.id);
    }
    eprintln!("hard edges: worst overshoot {worst:.3} 8-bit codes beyond the source range");
}

#[test]
fn shallow_wide_ramps_keep_their_precision() {
    let Some(gpu) = gpu() else { return };
    let mut r = ImageRenderer::new(&gpu, 32 << 20);
    let (w, h) = (512, 8);
    // Four 8-bit codes' worth of grey across 512 pixels.
    let (a, b) = (srgb_decode(100.0 / 255.0), srgb_decode(104.0 / 255.0));
    let linf = |x: f64| a + (b - a) * x.clamp(0.0, (w - 1) as f64) / (w - 1) as f64;
    let lin = |x: u32| linf(x as f64);
    let ramp16 = gray16(w, h, |x, _| lin(x));
    let mut data = Vec::with_capacity((w * h * 8) as usize);
    for _ in 0..h {
        for x in 0..w {
            let v = hdr::f32_to_f16(lin(x) as f32);
            for c in [v, v, v, hdr::f32_to_f16(1.0)] {
                data.extend_from_slice(&c.to_ne_bytes());
            }
        }
    }
    let ramp_f = plane_raster(
        w,
        h,
        PixelLayout::Rgba16F(Primaries::Bt709),
        w as usize * 8,
        data,
    );
    for (name, chart) in [("16-bit", &ramp16), ("half float", &ramp_f)] {
        for s in [1.0f32, 0.5] {
            let o = render(&gpu, &mut r, chart, s, hq());
            let y = o.h / 2;
            let mut levels: Vec<u64> = (0..o.w).map(|x| o.at(x, y).to_bits()).collect();
            levels.sort_unstable();
            levels.dedup();
            let mut worst = 0f64;
            for x in 2..o.w - 2 {
                let (sx, _) = o.src(x, y);
                worst = worst.max((o.at(x, y) - linf(sx - 0.5)).abs());
            }
            let lsb8 = b - srgb_decode(103.0 / 255.0);
            eprintln!(
                "{name} ramp zoom {s}: {} distinct levels over 4 8-bit codes, worst error {:.3} 8-bit steps",
                levels.len(),
                worst / lsb8
            );
            // Half floats near 0.13 step by 2^-13 ≈ 0.012 of an 8-bit code here: dozens of
            // levels, not four bands.
            assert!(
                levels.len() >= 25,
                "{name} zoom {s}: {} levels",
                levels.len()
            );
            assert!(worst < 0.5 * lsb8, "{name} zoom {s}: error {worst}");
        }
    }
}
