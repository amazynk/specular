//! HDR stills without a gain map, on the fixtures made by `tests/gen_hdr.py`: the PQ / HLG
//! signal ([`JxlFile::decode_hdr`]) and linear light ([`JxlFile::decode_linear`]), checked
//! against upstream jxl-oxide (the oracle) and against the values the script encoded.

use specular_jxl::{Decoded, Error, Hdr, JxlFile, Layout, Primaries};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

/// Upstream render of the first frame, interleaved f32, in the file's own colour encoding.
fn upstream(data: &[u8]) -> (usize, Vec<f32>) {
    let img = jxl_oxide::JxlImage::builder().read(data).unwrap();
    render_upstream(&img)
}

fn render_upstream(img: &jxl_oxide::JxlImage) -> (usize, Vec<f32>) {
    let r = img.render_frame(0).unwrap();
    let mut s = r.stream();
    let (w, h, c) = (s.width(), s.height(), s.channels() as usize);
    let mut buf = vec![0f32; w as usize * h as usize * c];
    s.write_to_buffer(&mut buf);
    (c, buf)
}

/// jxl-oxide's linear light of an XYB image in BT.2020, 1.0 = its intensity target, as
/// libjxl renders it. jxl-oxide tone maps non-HDR output of an image brighter than 255 cd/m²
/// (to 255 cd/m²) and gamut maps XYB to sRGB before any change of primaries, so it renders
/// PQ in sRGB primaries (sign-preserving, unclamped f32, 1.0 = the intensity target) and the
/// test converts back to linear and to BT.2020.
fn upstream_linear_2020(data: &[u8], intensity_target: f32) -> (usize, Vec<f32>) {
    use jxl_oxide::color::{ColourSpace, Primaries, TransferFunction, WhitePoint};
    use jxl_oxide::{EnumColourEncoding, RenderingIntent};
    let mut img = jxl_oxide::JxlImage::builder().read(data).unwrap();
    img.request_color_encoding(EnumColourEncoding {
        colour_space: ColourSpace::Rgb,
        white_point: WhitePoint::D65,
        primaries: Primaries::Srgb,
        tf: TransferFunction::Pq,
        rendering_intent: RenderingIntent::Relative,
    });
    let (c, mut buf) = render_upstream(&img);
    // BT.709 to BT.2020 (ITU-R BT.2087).
    const M: [[f64; 3]; 3] = [
        [0.627_403_896, 0.329_283_039, 0.043_313_065],
        [0.069_097_289, 0.919_540_395, 0.011_362_316],
        [0.016_391_439, 0.088_013_308, 0.895_595_253],
    ];
    for px in buf.chunks_exact_mut(c) {
        let l = [0, 1, 2].map(|k| {
            let v = px[k] as f64;
            (pq_eotf(v.abs()) / intensity_target as f64).copysign(v)
        });
        for (k, row) in M.iter().enumerate() {
            px[k] = (row[0] * l[0] + row[1] * l[1] + row[2] * l[2]) as f32;
        }
    }
    (c, buf)
}

/// gen_hdr.py `lin`: linear light, 1.0 = reference white.
fn lin(x: u32, y: u32) -> [f64; 3] {
    let (x, y) = (x as f64, y as f64);
    [
        0.05 + x / 31.0 * 3.95 * (1.0 - y / 30.0),
        0.02 + y / 15.0 * 2.0,
        0.5 + (x % 4.0) * 0.25,
    ]
}

fn pq_oetf(nits: f64) -> f64 {
    let (m1, m2) = (2610.0 / 16384.0, 2523.0 / 4096.0 * 128.0);
    let (c1, c2, c3) = (
        3424.0 / 4096.0,
        2413.0 / 4096.0 * 32.0,
        2392.0 / 4096.0 * 32.0,
    );
    let p = (nits.max(0.0) / 10000.0).powf(m1);
    ((c1 + c2 * p) / (1.0 + c3 * p)).powf(m2)
}

/// PQ signal to cd/m².
fn pq_eotf(e: f64) -> f64 {
    let (m1, m2) = (2610.0 / 16384.0, 2523.0 / 4096.0 * 128.0);
    let (c1, c2, c3) = (
        3424.0 / 4096.0,
        2413.0 / 4096.0 * 32.0,
        2392.0 / 4096.0 * 32.0,
    );
    let p = e.max(0.0).powf(1.0 / m2);
    10000.0 * ((p - c1).max(0.0) / (c2 - c3 * p)).powf(1.0 / m1)
}

fn hlg_oetf(e: f64) -> f64 {
    let a: f64 = 0.178_832_77;
    let b = 1.0 - 4.0 * a;
    let c = 0.5 - a * (4.0 * a).ln();
    let e = e.max(0.0);
    if e <= 1.0 / 12.0 {
        (3.0 * e).sqrt()
    } else {
        a * (12.0 * e - b).ln() + c
    }
}

fn u16_at(d: &Decoded, n: usize, c: usize) -> u16 {
    u16::from_ne_bytes([d.data[n * 8 + 2 * c], d.data[n * 8 + 2 * c + 1]])
}

fn f16_to_f32(h: u16) -> f32 {
    let s = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = (h >> 10) & 0x1f;
    let m = (h & 0x3ff) as f32;
    s * match e {
        0 => m * 2f32.powi(-24),
        31 => f32::INFINITY,
        _ => (1.0 + m / 1024.0) * 2f32.powi(e as i32 - 15),
    }
}

fn half_at(d: &Decoded, n: usize, c: usize) -> f32 {
    f16_to_f32(u16_at(d, n, c))
}

/// The 16-bit signal against upstream, to within 1/65535, both ways (parallel or not).
fn signal_matches_upstream(f: &JxlFile<Vec<u8>>, alpha: bool) -> Decoded {
    let (c, up) = upstream(f.data());
    assert_eq!(c, if alpha { 4 } else { 3 });
    let mut out = None;
    for parallel in [false, true] {
        let d = f.decode_hdr(1, parallel).unwrap();
        assert_eq!((d.width, d.height, d.layout), (32, 16, Layout::Rgba16));
        assert_eq!(d.data.len(), 32 * 16 * 8);
        let mut worst = 0f32;
        for n in 0..32 * 16 {
            for k in 0..c {
                let oracle = (up[n * c + k].clamp(0.0, 1.0) * 65535.0).round();
                worst = worst.max((u16_at(&d, n, k) as f32 - oracle).abs());
            }
            if !alpha {
                assert_eq!(u16_at(&d, n, 3), 65535);
            }
        }
        assert!(worst <= 1.0, "max |Δ| {worst} (16-bit levels) vs upstream");
        out = Some(d);
    }
    out.unwrap()
}

/// A reduced wide decode is the box average of the full one (alpha-weighted when there is
/// alpha), in the stored domain: the signal for 16-bit, linear light for halves.
fn reduced_is_average(f: &JxlFile<Vec<u8>>, full: &Decoded, scale: u32) {
    let d = match full.layout {
        Layout::Rgba16 => f.decode_hdr(scale, true).unwrap(),
        _ => f.decode_linear(scale, true).unwrap(),
    };
    let s = scale as usize;
    assert_eq!(d.layout, full.layout);
    assert_eq!(
        (d.width, d.height),
        (32u32.div_ceil(scale), 16u32.div_ceil(scale))
    );
    assert_eq!(d.scale, scale);
    let get = |d: &Decoded, n: usize, c: usize| match d.layout {
        Layout::Rgba16 => u16_at(d, n, c) as f32 / 65535.0,
        _ => half_at(d, n, c),
    };
    let tol = match d.layout {
        Layout::Rgba16 => 1.0 / 65535.0,
        _ => 2e-3,
    };
    for oy in 0..d.height as usize {
        for ox in 0..d.width as usize {
            let (mut sum, mut asum) = ([0f32; 3], 0f32);
            for y in oy * s..(oy * s + s).min(16) {
                for x in ox * s..(ox * s + s).min(32) {
                    let n = y * 32 + x;
                    let a = get(full, n, 3);
                    asum += a;
                    for (c, v) in sum.iter_mut().enumerate() {
                        *v += get(full, n, c) * a;
                    }
                }
            }
            let n = oy * d.width as usize + ox;
            let cnt = ((oy * s + s).min(16) - oy * s) * ((ox * s + s).min(32) - ox * s);
            assert!((get(&d, n, 3) - asum / cnt as f32).abs() <= 1.5 * tol);
            for (c, v) in sum.iter().enumerate() {
                let want = v / asum;
                let got = get(&d, n, c);
                assert!(
                    (got - want).abs() <= 1.5 * tol * want.abs().max(1.0),
                    "1/{scale} ({ox}, {oy}) {c}: {got} vs {want}"
                );
            }
        }
    }
}

#[test]
fn pq_p3() {
    let f = JxlFile::open(fixture("hdr_pq_p3.jxl")).unwrap();
    let i = f.info();
    assert!(!i.gain_map && !i.linear);
    assert_eq!(i.hdr, Some(Hdr::Pq));
    assert_eq!(i.primaries, Some(Primaries::DisplayP3));
    assert_eq!(i.intensity_target, 1000.0);
    let d = signal_matches_upstream(&f, false);
    for n in 0..32 * 16 {
        let l = lin(n as u32 % 32, n as u32 / 32);
        for (c, v) in l.iter().enumerate() {
            let want = (pq_oetf(v * 203.0) * 65535.0).round();
            assert_eq!(u16_at(&d, n, c) as f64, want, "{n} {c}");
        }
    }
    reduced_is_average(&f, &d, 2);
    reduced_is_average(&f, &d, 8);
    // Not linear, so no linear decode; the ordinary decode still tone maps.
    assert!(matches!(
        f.decode_linear(1, false),
        Err(Error::Unsupported(_))
    ));
    assert_eq!(f.decode(1, false).unwrap().layout, Layout::Rgb);
    // Wide output: 8 bytes per output pixel on top of the codec's planes.
    assert_eq!(
        f.decode_wide_memory(1) - f.decode_memory(1),
        32 * 16 * (8 - 3)
    );
}

#[test]
fn hlg_2020_alpha() {
    let f = JxlFile::open(fixture("hdr_hlg_2020_alpha.jxl")).unwrap();
    let i = f.info();
    assert_eq!(i.hdr, Some(Hdr::Hlg));
    assert_eq!(i.primaries, Some(Primaries::Bt2020));
    assert!(i.has_alpha);
    let d = signal_matches_upstream(&f, true);
    for n in 0..32 * 16 {
        let (x, y) = (n as u32 % 32, n as u32 / 32);
        for (c, v) in lin(x, y).iter().enumerate() {
            let want = (hlg_oetf(v / 4.0) * 65535.0).round();
            assert_eq!(u16_at(&d, n, c) as f64, want, "{n} {c}");
        }
        assert_eq!(u16_at(&d, n, 3) as u32, x * 2113 + y);
    }
    reduced_is_average(&f, &d, 2);
    reduced_is_average(&f, &d, 4);
}

#[test]
fn pq_709_lossy() {
    let f = JxlFile::open(fixture("hdr_pq_709_lossy.jxl")).unwrap();
    let i = f.info();
    assert!(i.xyb);
    assert_eq!(i.hdr, Some(Hdr::Pq));
    assert_eq!(i.primaries, Some(Primaries::Bt709));
    let d = signal_matches_upstream(&f, false);
    // Lossy, but close to what was encoded.
    let mut sum = 0.0;
    for n in 0..32 * 16 {
        for (c, v) in lin(n as u32 % 32, n as u32 / 32).iter().enumerate() {
            sum += (u16_at(&d, n, c) as f64 / 65535.0 - pq_oetf(v * 203.0)).abs();
        }
    }
    let mean = sum / (32.0 * 16.0 * 3.0);
    assert!(mean < 0.01, "mean |Δ| {mean}");
    reduced_is_average(&f, &d, 2);
}

/// Linear output against upstream: the same values, as halves, times the white scale, within
/// `rel` of the value (or of `floor`, for an oracle that went through a matrix and PQ).
fn linear_matches_upstream(
    f: &JxlFile<Vec<u8>>,
    (c, up): (usize, Vec<f32>),
    mul: f32,
    (rel, floor): (f32, f32),
) -> Decoded {
    assert_eq!(c, 3);
    let mut out = None;
    for parallel in [false, true] {
        let d = f.decode_linear(1, parallel).unwrap();
        assert_eq!((d.width, d.height, d.layout), (32, 16, Layout::Rgba16F));
        for n in 0..32 * 16 {
            for k in 0..3 {
                let want = up[n * c + k] * mul;
                let got = half_at(&d, n, k);
                assert!(
                    (got - want).abs() <= want.abs().max(floor) * rel + 1e-7,
                    "{n} {k}: {got} vs upstream {want}"
                );
            }
            assert_eq!(half_at(&d, n, 3), 1.0);
        }
        out = Some(d);
    }
    out.unwrap()
}

#[test]
fn linear_float32() {
    let f = JxlFile::open(fixture("linear_f32.jxl")).unwrap();
    let i = f.info();
    assert!(i.linear && i.float_samples && !i.xyb);
    assert_eq!((i.hdr, i.primaries), (None, Some(Primaries::Bt709)));
    assert_eq!(i.bits_per_sample, 32);
    assert_eq!(i.intensity_target, 255.0);
    let d = linear_matches_upstream(&f, upstream(f.data()), 1.0, (1.0 / 2048.0, 0.0));
    let mut peak = 0f32;
    for n in 0..32 * 16 {
        for (c, v) in lin(n as u32 % 32, n as u32 / 32).iter().enumerate() {
            let got = half_at(&d, n, c);
            peak = peak.max(got);
            assert!(
                (got as f64 - v).abs() <= v / 2048.0,
                "{n} {c}: {got} vs {v}"
            );
        }
    }
    assert!(peak > 3.9, "values above reference white are kept: {peak}");
    reduced_is_average(&f, &d, 2);
    reduced_is_average(&f, &d, 8);
    assert!(matches!(f.decode_hdr(1, false), Err(Error::Unsupported(_))));
    // The 8-bit decode clips at 1.0 (and is what an SDR-only caller gets).
    let d8 = f.decode(1, false).unwrap();
    assert_eq!(d8.layout, Layout::Rgb);
}

/// Half-float input, lossy (XYB), BT.2020, intensity target 812 cd/m².
#[test]
fn linear_half_lossy_2020() {
    let f = JxlFile::open(fixture("linear_f16_2020_lossy.jxl")).unwrap();
    let i = f.info();
    assert!(i.linear && i.float_samples && i.xyb);
    assert_eq!(i.primaries, Some(Primaries::Bt2020));
    // Above 255 cd/m² the intensity target is what 1.0 means, and reference white is
    // 203 cd/m²: 812 cd/m² is four times reference white.
    assert_eq!(i.intensity_target, 812.0);
    // libjxl decodes this to lin / 4 (1.0 = 812 cd/m²); jxl-oxide's own linear output
    // would be tone mapped to 255 cd/m² instead, so the oracle goes through PQ.
    let d = linear_matches_upstream(
        &f,
        upstream_linear_2020(f.data(), 812.0),
        4.0,
        (1.0 / 1024.0, 0.1),
    );
    let mut sum = 0.0;
    for n in 0..32 * 16 {
        for (c, v) in lin(n as u32 % 32, n as u32 / 32).iter().enumerate() {
            sum += (half_at(&d, n, c) as f64 - v).abs() / v;
        }
    }
    let mean = sum / (32.0 * 16.0 * 3.0);
    assert!(mean < 0.05, "mean relative |Δ| {mean}");
    reduced_is_average(&f, &d, 2);
}
