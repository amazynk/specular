use wgpu::util::DeviceExt;

use super::*;
use crate::testgpu;

/// A picture with smooth ramps, saturated patches, values above the peak and below zero,
/// and noise (so every scope gets work).
pub(crate) fn picture(w: u32, h: u32, peak: f32) -> Vec<f32> {
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    let mut seed = 0x2545_f491u32;
    let mut rnd = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32
    };
    for y in 0..h {
        for x in 0..w {
            let fx = x as f32 / w as f32;
            let fy = y as f32 / h as f32;
            let p = match (x * 4 / w, y * 3 / h) {
                (0, _) => [fx * 4.0 * peak; 3],
                (1, 0) => [fy * peak, 0.05, 0.02],
                (1, 1) => [0.03, fy * peak, 0.6 * fy],
                (1, 2) => [0.6 * fx, 0.1, fy * peak],
                (2, _) => [rnd() * 1.2 * peak, rnd() * peak, rnd() * 0.8 * peak],
                _ => [(fx - 0.7) * 3.0, fy * 1.3 * peak, 0.5 * peak],
            };
            v.extend_from_slice(&[p[0], p[1], p[2], 1.0]);
        }
    }
    v
}

fn upload(device: &wgpu::Device, px: &[f32], prec: Precision) -> wgpu::Buffer {
    let bytes: Vec<u8> = match prec {
        Precision::F32 => bytemuck::cast_slice(px).to_vec(),
        Precision::F16 => px
            .iter()
            .flat_map(|v| half::f16::from_f32(*v).to_bits().to_le_bytes())
            .collect(),
    };
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("scope test"),
        contents: &bytes,
        usage: wgpu::BufferUsages::STORAGE,
    })
}

fn rounded(px: &[f32], prec: Precision) -> Vec<f32> {
    match prec {
        Precision::F32 => px.to_vec(),
        Precision::F16 => px
            .iter()
            .map(|v| half::f16::from_f32(*v).to_f32())
            .collect(),
    }
}

/// Bins that differ between `a` and `b` in `range`, as a fraction of the samples.
fn moved(a: &Bins, b: &Bins, range: std::ops::Range<usize>) -> f64 {
    let d: u64 = range
        .map(|i| u64::from(a.words[i].abs_diff(b.words[i])))
        .sum();
    d as f64 / f64::from(a.samples.max(1))
}

#[test]
fn stride_counts_about_a_megapixel() {
    assert_eq!(stride(1000, 1000), 1);
    assert_eq!(stride(1024, 1024), 1);
    assert_eq!(stride(4096, 2731), 4);
    for (w, h) in [(6000, 4000), (2560, 1600), (8192, 8192), (300, 20000)] {
        let (gw, gh) = grid(w, h);
        let n = u64::from(gw) * u64::from(gh);
        assert!(n <= SAMPLES && n > SAMPLES / 5, "{w}x{h}: {n}");
    }
}

#[test]
fn cpu_reference_counts_every_sample_once_per_scope() {
    let (w, h) = (640, 480);
    let px = picture(w, h, 1.0);
    let b = cpu(&px, w, h, 1.0);
    let n = u64::from(b.samples);
    assert_eq!(n, u64::from(w * h));
    for c in 0..4 {
        assert_eq!(b.hist(c).iter().map(|&v| u64::from(v)).sum::<u64>(), n);
    }
    for p in 0..4 {
        assert_eq!(b.wave(p).iter().map(|&v| u64::from(v)).sum::<u64>(), n);
    }
    assert_eq!(b.vector().iter().map(|&v| u64::from(v)).sum::<u64>(), n);
    // The grey ramp lands on the vectorscope's centre.
    let mid = VEC / 2;
    let centre: u32 = (mid - 1..=mid)
        .flat_map(|r| (mid - 1..=mid).map(move |c| (r, c)))
        .map(|(r, c)| b.vector()[r * VEC + c])
        .sum();
    assert!(centre >= w * h / 5, "{centre}");
}

#[test]
fn grey_is_central_and_primaries_where_marked() {
    let t = targets();
    // Rec.709 red in OkLab: a ≈ 0.225, b ≈ 0.126 (Ottosson's published sRGB red).
    assert!((t.ab[0][0] - 0.2249).abs() < 2e-3 && (t.ab[0][1] - 0.1258).abs() < 2e-3);
    let (lms, m2) = (rec709_to_lms(), m2_ab());
    let g = ab([0.18, 0.18, 0.18], &lms, &m2);
    assert!(g[0].abs() < 1e-4 && g[1].abs() < 1e-4, "{g:?}");
    // Skin sits between red and yellow.
    let s = skin_hue().to_degrees();
    assert!((30.0..80.0).contains(&s), "skin hue {s}");
}

#[test]
fn gpu_matches_cpu() {
    let Some(d) = testgpu::dev() else { return };
    let _g = testgpu::lock();
    let mut e = ScopeEngine::new(&d.device, &d.queue);
    for (w, h, prec, peak) in [
        (1500, 1000, Precision::F32, 1.0),
        (1500, 1000, Precision::F16, 1.0),
        (2560, 1707, Precision::F16, 1.0),
        (777, 333, Precision::F32, 4.0),
    ] {
        let px = picture(w, h, peak);
        let buf = upload(&d.device, &px, prec);
        e.run(&buf, w, h, prec, peak);
        let g = e.wait().expect("bins read back");
        let c = cpu(&rounded(&px, prec), w, h, peak);
        assert_eq!(g.samples, c.samples);
        let hist = moved(&g, &c, Layout::HIST..Layout::WAVE);
        let wave = moved(&g, &c, Layout::WAVE..Layout::VECTOR);
        let vec = moved(&g, &c, Layout::VECTOR..Layout::WORDS);
        eprintln!(
            "scopes {w}x{h} {prec:?} peak {peak}: moved hist {hist:.2e} wave {wave:.2e} vector {vec:.2e}"
        );
        // Only samples on a bin edge may land in the next bin (the curve's rounding): at
        // most 0.1 % of the samples per scope.
        assert!(hist < 2e-3 && wave < 2e-3 && vec < 2e-3);
    }
}

#[test]
fn rasters_have_the_asked_size_and_show_traces() {
    let (w, h) = (640, 480);
    let px = picture(w, h, 1.0);
    let b = cpu(&px, w, h, 1.0);
    let ground = |img: &[u8]| {
        img.chunks_exact(4)
            .filter(|p| p[..3] != raster::GROUND)
            .count()
    };
    for (img, n) in [
        (raster::histogram(&b, 300, 120, false), 300 * 120),
        (raster::histogram(&b, 300, 120, true), 300 * 120),
        (raster::waveform(&b, 300, 160, false), 300 * 160),
        (raster::waveform(&b, 300, 160, true), 300 * 160),
        (raster::parade(&b, 450, 160), 450 * 160),
        (raster::vectorscope(&b, 200), 200 * 200),
    ] {
        assert_eq!(img.len(), n * 4);
        let lit = ground(&img);
        assert!(lit > n / 50 && lit < n, "{lit} of {n} lit");
        assert!(img.chunks_exact(4).all(|p| p[3] == 255));
    }
}

/// Scope pass time at about 1 MP (GPU, waiting for each pass): `cargo test -p
/// specular-develop scope_timing -- --ignored --nocapture`.
#[test]
#[ignore]
fn scope_timing() {
    let Some(d) = testgpu::dev() else { return };
    let _g = testgpu::lock();
    let mut e = ScopeEngine::new(&d.device, &d.queue);
    e.profile = true;
    for (w, h, prec) in [
        (1254, 836, Precision::F16),
        (1254, 836, Precision::F32),
        (2560, 1707, Precision::F16),
    ] {
        let px = picture(w, h, 1.0);
        let buf = upload(&d.device, &px, prec);
        for _ in 0..5 {
            e.run(&buf, w, h, prec, 1.0);
            e.wait();
        }
        let mut t = Vec::new();
        let mut gpu = Vec::new();
        for _ in 0..60 {
            let t0 = std::time::Instant::now();
            e.run(&buf, w, h, prec, 1.0);
            e.wait();
            t.push(t0.elapsed().as_secs_f64() * 1e3);
            gpu.extend(e.gpu_ms);
        }
        t.sort_by(f64::total_cmp);
        gpu.sort_by(f64::total_cmp);
        if !gpu.is_empty() {
            eprintln!(
                "scopes {w}x{h} {prec:?}: GPU pass p50 {:.2} ms p95 {:.2} ms",
                gpu[gpu.len() / 2],
                gpu[gpu.len() * 95 / 100]
            );
        }
        let load = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
        eprintln!(
            "scopes {w}x{h} ({} samples): p50 {:.2} ms p95 {:.2} ms (incl. read-back) load {}",
            grid(w, h).0 * grid(w, h).1,
            t[t.len() / 2],
            t[t.len() * 95 / 100],
            load.split_whitespace()
                .take(3)
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
}
