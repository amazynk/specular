//! Differential tests: specular-jxl against upstream jxl-oxide 0.12.6 (the oracle) on the tiny
//! fixtures in `tests/data`.

use specular_jxl::{JxlFile, Layout};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

/// Upstream render of `keyframe` as interleaved u8 (colour + alpha only), with its channel count.
fn upstream(data: &[u8], keyframe: usize) -> (u32, u32, usize, Vec<u8>) {
    let img = jxl_oxide::JxlImage::builder().read(data).unwrap();
    let r = img.render_frame(keyframe).unwrap();
    let mut s = r.stream();
    let (w, h, c) = (s.width(), s.height(), s.channels() as usize);
    let mut buf = vec![0u8; w as usize * h as usize * c];
    s.write_to_buffer(&mut buf);
    (w, h, c, buf)
}

fn upstream_frames(data: &[u8]) -> usize {
    jxl_oxide::JxlImage::builder()
        .read(data)
        .unwrap()
        .num_loaded_keyframes()
}

/// Box-downscale an interleaved u8 image the way specular-jxl does (average, round). With
/// alpha (2 or 4 channels, alpha last) colour is weighted by alpha, as straight alpha must be.
fn box_down(w: u32, h: u32, c: usize, px: &[u8], s: u32) -> Vec<u8> {
    let (w, h, s) = (w as usize, h as usize, s as usize);
    let (ow, oh) = (w.div_ceil(s), h.div_ceil(s));
    let alpha = c == 2 || c == 4;
    let mut out = vec![0u8; ow * oh * c];
    for oy in 0..oh {
        for ox in 0..ow {
            for k in 0..c {
                let weighted = alpha && k + 1 < c;
                let (mut t, mut n) = (0u64, 0u64);
                for y in oy * s..((oy + 1) * s).min(h) {
                    for x in ox * s..((ox + 1) * s).min(w) {
                        let i = (y * w + x) * c;
                        let wt = if weighted { px[i + c - 1] as u64 } else { 1 };
                        t += px[i + k] as u64 * wt;
                        n += wt;
                    }
                }
                out[(oy * ow + ox) * c + k] = (t + n / 2).checked_div(n).unwrap_or(0) as u8;
            }
        }
    }
    out
}

fn max_diff(a: &[u8], b: &[u8]) -> u8 {
    assert_eq!(a.len(), b.len(), "buffer sizes differ");
    a.iter()
        .zip(b)
        .map(|(x, y)| x.abs_diff(*y))
        .max()
        .unwrap_or(0)
}

fn check(name: &str, expect: Layout, tol_full: u8) {
    let data = fixture(name);
    let (uw, uh, uc, ubuf) = upstream(&data, 0);
    let f = JxlFile::open(&data[..]).unwrap();
    assert_eq!((f.info().width, f.info().height), (uw, uh), "{name}: size");
    for parallel in [false, true] {
        let d = f.decode(1, parallel).unwrap();
        assert!(d.complete);
        assert_eq!(d.layout, expect, "{name}: layout");
        assert_eq!(d.layout.channels(), uc, "{name}: channels");
        assert_eq!((d.width, d.height), (uw, uh));
        let m = max_diff(&d.data, &ubuf);
        assert!(m <= tol_full, "{name}: full-size max diff {m} > {tol_full}");
    }
    for s in [2u32, 4, 8] {
        let d = f.decode(s, false).unwrap();
        assert_eq!(
            (d.width, d.height),
            (uw.div_ceil(s), uh.div_ceil(s)),
            "{name}: 1/{s}"
        );
        let m = max_diff(&d.data, &box_down(uw, uh, uc, &ubuf, s));
        // Ours averages before quantising; the reference averages quantised samples.
        assert!(m <= tol_full + 2, "{name}: 1/{s} max diff {m}");
    }
    eprintln!("{name}: {uw}×{uh}×{uc} ok");
}

#[test]
fn vardct() {
    check("vardct.jxl", Layout::Rgb, 1);
}

#[test]
fn vardct_multigroup() {
    check("vardct_multigroup.jxl", Layout::Rgb, 1);
}

#[test]
fn modular_lossless() {
    check("modular.jxl", Layout::Rgb, 1);
}

#[test]
fn modular_lossy() {
    check("modular_lossy.jxl", Layout::Rgb, 1);
}

#[test]
fn alpha() {
    check("alpha.jxl", Layout::Rgba, 1);
}

#[test]
fn alpha_lossless() {
    check("alpha_lossless.jxl", Layout::Rgba, 1);
}

#[test]
fn gray() {
    let f = JxlFile::open(fixture("gray.jxl")).unwrap();
    assert!(f.info().gray);
    check("gray.jxl", Layout::Gray, 1);
}

#[test]
fn rgb16() {
    let f = JxlFile::open(fixture("rgb16.jxl")).unwrap();
    assert_eq!(f.info().bits_per_sample, 16);
    let expect = if f.info().has_alpha {
        Layout::Rgba
    } else {
        Layout::Rgb
    };
    check("rgb16.jxl", expect, 1);
}

#[test]
fn animation() {
    let data = fixture("anim3.jxl");
    let n = upstream_frames(&data);
    assert_eq!(n, 3, "fixture has 3 keyframes");
    let f = JxlFile::open(&data[..]).unwrap();
    assert!(f.info().animated);
    for scale in [1u32, 2] {
        let frames = f.frames(scale, false).unwrap();
        assert_eq!(frames.len(), 3);
        let mut count = 0;
        for (i, d) in frames.enumerate() {
            let d = d.unwrap();
            assert!(d.complete);
            assert!(d.duration_ms > 0, "frame {i} has a duration");
            let (uw, uh, uc, ubuf) = upstream(&data, i);
            let reference = if scale == 1 {
                ubuf
            } else {
                box_down(uw, uh, uc, &ubuf, scale)
            };
            let m = max_diff(&d.data, &reference);
            assert!(m <= 3, "frame {i} at 1/{scale}: max diff {m}");
            count += 1;
        }
        assert_eq!(count, 3);
    }
}

#[test]
fn decode_dc_preview() {
    for name in [
        "vardct.jxl",
        "vardct_multigroup.jxl",
        "modular.jxl",
        "alpha.jxl",
        "gray.jxl",
    ] {
        let data = fixture(name);
        let f = JxlFile::open(&data[..]).unwrap();
        let d = f.decode_dc(false).unwrap();
        let (w, h) = (f.info().width, f.info().height);
        assert_eq!(d.scale, 8, "{name}");
        assert_eq!(
            (d.width, d.height),
            (w.div_ceil(8), h.div_ceil(8)),
            "{name}"
        );
        // The LF image is a low-pass of the full image: compare against the 1/8 box reference.
        let full = f.decode(8, false).unwrap();
        assert_eq!(d.layout, full.layout);
        let mean = d
            .data
            .iter()
            .zip(&full.data)
            .map(|(a, b)| a.abs_diff(*b) as u64)
            .sum::<u64>() as f64
            / d.data.len() as f64;
        // DC is the block mean in XYB, not in gamma-encoded sRGB, and saturated colours clip
        // differently: a few levels of mean difference are expected on these synthetic images.
        assert!(mean < 8.0, "{name}: DC preview mean abs diff {mean:.2}");
        eprintln!("{name}: dc mean diff {mean:.2}");
    }
}

#[test]
fn probe_and_info() {
    assert!(specular_jxl::probe(&fixture("vardct.jxl")));
    assert!(!specular_jxl::probe(b"\x89PNG\r\n\x1a\n"));
    let f = JxlFile::open(fixture("vardct.jxl")).unwrap();
    let i = f.info();
    assert!(i.xyb);
    assert!(!i.animated);
    assert_eq!(i.orientation, 1);
    assert!(i.output_icc.is_none());
    assert!(f.decode_memory(1) > 0);
}

/// Frame sample `(fx, fy)` of the offset-frame fixtures (10-bit RGBA); see gen_offset.py.
fn offset_px(fx: usize, fy: usize) -> [u32; 4] {
    let (fx, fy) = (fx as u32, fy as u32);
    [
        (fx * 37 + fy * 11) % 1024,
        (fx * fy * 7 + 100) % 1024,
        1023 - (fx * 20 + fy * 3) % 1024,
        512 + (fx + fy) * 13 % 512,
    ]
}

/// The offset-frame fixtures: a 20×12 image whose only frame is 34×22 at (-9, -5), so image
/// pixel (x, y) is frame sample (x + 9, y + 5) (libjxl conformance `sunset_logo` in small).
const OFFSET: (usize, usize, usize, usize) = (20, 12, 9, 5);

/// Lossless 10-bit samples in a frame larger than the image, at a negative offset: the
/// rendered planes start outside the image, and the conversions must read them from the
/// image origin (a regression: they read from the plane origin).
#[test]
fn offset_frame() {
    let (w, h, dx, dy) = OFFSET;
    let data = fixture("offset_frame.jxl");
    let f = JxlFile::open(&data[..]).unwrap();
    assert_eq!((f.info().width, f.info().height), (w as u32, h as u32));
    let expect: Vec<u8> = (0..h)
        .flat_map(|y| (0..w).flat_map(move |x| offset_px(x + dx, y + dy)))
        // round(v * 255 / 1023): never a tie for 10-bit v.
        .map(|v| ((v * 255 * 2 + 1023) / (1023 * 2)) as u8)
        .collect();
    let (uw, uh, uc, ubuf) = upstream(&data, 0);
    assert_eq!((uw, uh, uc), (w as u32, h as u32, 4));
    assert_eq!(
        max_diff(&ubuf, &expect),
        0,
        "jxl-oxide against the generator"
    );
    for parallel in [false, true] {
        let d = f.decode(1, parallel).unwrap();
        assert_eq!(d.layout, Layout::Rgba);
        assert_eq!((d.width, d.height), (w as u32, h as u32));
        assert_eq!(
            max_diff(&d.data, &expect),
            0,
            "full size, parallel {parallel}"
        );
    }
    for s in [2u32, 3] {
        let d = f.decode(s, false).unwrap();
        let m = max_diff(&d.data, &box_down(uw, uh, uc, &expect, s));
        assert!(m <= 2, "1/{s}: max diff {m}");
    }
    let frames: Vec<_> = f.frames(1, false).unwrap().collect();
    assert_eq!(frames.len(), 1);
    assert_eq!(
        max_diff(&frames[0].as_ref().unwrap().data, &expect),
        0,
        "frames()"
    );
}

/// The 16-bit path ([`JxlFile::decode_hdr`]) on the same layout, tagged PQ.
#[test]
fn offset_frame_hdr() {
    let (w, h, dx, dy) = OFFSET;
    let f = JxlFile::open(fixture("offset_frame_pq.jxl")).unwrap();
    let expect: Vec<u16> = (0..h)
        .flat_map(|y| (0..w).flat_map(move |x| offset_px(x + dx, y + dy)))
        // round(v * 65535 / 1023): never a tie for 10-bit v.
        .map(|v| ((v * 65535 * 2 + 1023) / (1023 * 2)) as u16)
        .collect();
    for parallel in [false, true] {
        let d = f.decode_hdr(1, parallel).unwrap();
        assert_eq!(d.layout, Layout::Rgba16);
        assert_eq!((d.width, d.height), (w as u32, h as u32));
        let got: Vec<u16> = d
            .data
            .chunks_exact(2)
            .map(|b| u16::from_ne_bytes([b[0], b[1]]))
            .collect();
        let m = got
            .iter()
            .zip(&expect)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        assert!(m <= 1, "decode_hdr, parallel {parallel}: max diff {m}");
    }
}
