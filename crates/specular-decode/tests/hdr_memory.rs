//! Peak memory of the streamed HDR decode, measured: an 8192×4096 run-length Radiance file is
//! written to `TMPDIR` (a few MB; nothing large is committed) and each level and a full-size
//! region decoded, with the process's resident high-water mark (`VmHWM`, reset through
//! `/proc/self/clear_refs`) checked against the session's own estimate. The old path held the
//! whole `f32` image (512 MiB) plus the half raster. One test in its own binary, so nothing
//! else allocates meanwhile. Linux only; skipped where `clear_refs` cannot be written.

use std::io::Write;
use std::sync::Arc;

use specular_core::{CancelToken, Source};
use specular_decode::{DecodeRequest, Rect, Registry};

fn status(key: &str) -> Option<u64> {
    let s = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = s.lines().find(|l| l.starts_with(key))?;
    let kb: u64 = line[key.len()..]
        .trim()
        .trim_end_matches("kB")
        .trim()
        .parse()
        .ok()?;
    Some(kb << 10)
}

/// Resets the high-water mark to the current resident size; returns that size.
fn reset_peak() -> Option<u64> {
    std::fs::write("/proc/self/clear_refs", "5").ok()?;
    status("VmRSS:")
}

fn rgbe([r, g, b]: [f32; 3]) -> [u8; 4] {
    let m = r.max(g).max(b);
    if m < 1e-32 {
        return [0; 4];
    }
    let e = m.log2().floor() as i32 + 1;
    let s = 256.0 / 2f32.powi(e);
    [(r * s) as u8, (g * s) as u8, (b * s) as u8, (e + 128) as u8]
}

/// Writes a `w`×`h` run-length Radiance file to `path`, a scanline at a time: a sky-like
/// gradient in runs of 64 pixels, so the file stays small.
fn write_rle(path: &std::path::Path, w: u32, h: u32) {
    let mut f = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    write!(f, "#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y {h} +X {w}\n").unwrap();
    let mut line = Vec::new();
    for y in 0..h {
        line.clear();
        line.extend_from_slice(&[2, 2, (w >> 8) as u8, w as u8]);
        let px: Vec<[u8; 4]> = (0..w / 64)
            .map(|k| rgbe([k as f32 * 0.07 + 0.01, y as f32 / h as f32 * 40.0, 2.5]))
            .collect();
        for c in 0..4 {
            for p in &px {
                // One run of 64 (code 128 + 64).
                line.extend_from_slice(&[128 + 64, p[c]]);
            }
        }
        f.write_all(&line).unwrap();
    }
    f.flush().unwrap();
}

#[test]
fn radiance_8k_peak_memory_is_one_level() {
    let Some(_) = reset_peak() else {
        eprintln!("/proc/self/clear_refs not writable; skipped");
        return;
    };
    let (w, h) = (8192u32, 4096u32);
    let path = std::env::temp_dir().join(format!("hdr_memory_{}.hdr", std::process::id()));
    write_rle(&path, w, h);
    let file_len = std::fs::metadata(&path).unwrap().len();
    let s = Registry::new()
        .open(Arc::new(Source::open(&path).unwrap()))
        .unwrap();
    let cancel = CancelToken::new();
    let mut report = Vec::new();
    let mut check = |name: &str, req: DecodeRequest| {
        let est = s.decode_memory(&req);
        let base = reset_peak().unwrap();
        let t = std::time::Instant::now();
        let r = s.decode(&req, &cancel).unwrap();
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let peak = status("VmHWM:").unwrap().saturating_sub(base);
        let out = r.planes[0].data.len() as u64;
        drop(r);
        report.push(format!(
            "{name:>14}: output {:>4} MiB, peak +{:>4} MiB, estimate {:>4} MiB, {ms:.0} ms",
            out >> 20,
            peak >> 20,
            est >> 20
        ));
        // The mapped file is resident too, and the allocator and thread stacks add a little.
        assert!(
            peak <= est + file_len + (24 << 20),
            "{name}: peak {peak} over the estimate {est}"
        );
        assert!(
            peak <= out + file_len + (96 << 20),
            "{name}: peak {peak} for {out}"
        );
    };
    s.prepare_regions(&cancel).unwrap();
    for scale in [1, 2, 4, 8] {
        check(&format!("1/{scale}"), DecodeRequest::full(scale));
    }
    check(
        "512² region",
        DecodeRequest {
            region: Some(Rect {
                x: 3000,
                y: 1000,
                w: 512,
                h: 512,
            }),
            ..DecodeRequest::full(1)
        },
    );
    for line in &report {
        eprintln!("{line}");
    }
    let _ = std::fs::remove_file(&path);
}
