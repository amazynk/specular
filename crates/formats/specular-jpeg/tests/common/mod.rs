//! Shared helpers: generate test JPEGs with ImageMagick + cjpeg (libjpeg-turbo) and decode
//! references with djpeg. Tests are skipped (with a message) if the tools are missing.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

pub fn tools_available() -> bool {
    ["cjpeg", "djpeg", "magick"]
        .iter()
        .all(|t| Command::new(t).arg("-version").output().is_ok())
}

pub fn tmp_dir() -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("specular-jpeg-tests");
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Photo-like PPM source (smooth plasma + noise + edges), cached by name.
pub fn source_ppm(w: u32, h: u32, seed: u32) -> PathBuf {
    let p = tmp_dir().join(format!("src_{w}x{h}_{seed}.ppm"));
    if !p.exists() {
        let tmp = p.with_extension(format!("{}.tmp.ppm", uniq()));
        let st = Command::new("magick")
            .args([
                "-seed",
                &seed.to_string(),
                "-size",
                &format!("{w}x{h}"),
                "plasma:fractal",
                "-blur",
                "0x0.7",
            ])
            .args(["-attenuate", "0.4", "+noise", "Gaussian"])
            .args([
                "-fill",
                "none",
                "-stroke",
                "black",
                "-strokewidth",
                "2",
                "-draw",
                &format!("line 0,0 {},{}", w / 2, h),
            ])
            .args([
                "-stroke",
                "white",
                "-draw",
                &format!("circle {},{} {},{}", w / 3, h / 3, w / 3 + w / 5 + 1, h / 3),
            ])
            .arg(format!("ppm:{}", tmp.display()))
            .status()
            .expect("magick");
        assert!(st.success());
        std::fs::rename(&tmp, &p).unwrap();
    }
    p
}

/// Encode `src` with cjpeg using `args`; returns the JPEG bytes (cached by name).
pub fn cjpeg(src: &Path, name: &str, args: &[&str]) -> Vec<u8> {
    let out = tmp_dir().join(format!("{name}.jpg"));
    if !out.exists() {
        let tmp = out.with_extension(format!("{}.tmp", uniq()));
        let st = Command::new("cjpeg")
            .args(args)
            .arg("-outfile")
            .arg(&tmp)
            .arg(src)
            .status()
            .expect("cjpeg");
        assert!(st.success(), "cjpeg {args:?} failed");
        std::fs::rename(&tmp, &out).unwrap();
    }
    std::fs::read(out).unwrap()
}

pub struct Pnm {
    pub w: usize,
    pub h: usize,
    pub channels: usize,
    pub data: Vec<u8>,
}

pub fn parse_pnm(b: &[u8]) -> Pnm {
    let mut fields = Vec::new();
    let mut i = 0;
    while fields.len() < 4 {
        while b[i].is_ascii_whitespace() {
            i += 1;
        }
        if b[i] == b'#' {
            while b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        let s = i;
        while !b[i].is_ascii_whitespace() {
            i += 1;
        }
        fields.push(String::from_utf8_lossy(&b[s..i]).to_string());
    }
    i += 1;
    let channels = match fields[0].as_str() {
        "P5" => 1,
        "P6" => 3,
        f => panic!("unsupported pnm {f}"),
    };
    let w: usize = fields[1].parse().unwrap();
    let h: usize = fields[2].parse().unwrap();
    Pnm {
        w,
        h,
        channels,
        data: b[i..i + w * h * channels].to_vec(),
    }
}

/// Run djpeg with `args` on `jpeg` (a file path), returning the decoded PNM.
pub fn djpeg_file(path: &Path, args: &[&str]) -> Pnm {
    let out = Command::new("djpeg")
        .args(args)
        .arg(path)
        .output()
        .expect("djpeg");
    assert!(
        out.status.success(),
        "djpeg failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    parse_pnm(&out.stdout)
}

pub fn djpeg(jpeg: &[u8], args: &[&str]) -> Pnm {
    let p = tmp_dir().join(format!("dj_{}.jpg", uniq()));
    std::fs::write(&p, jpeg).unwrap();
    let r = djpeg_file(&p, args);
    let _ = std::fs::remove_file(&p);
    r
}

pub fn fxhash(b: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &x in b.iter().step_by(((b.len() / 4096) + 1).max(1)) {
        h ^= x as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h ^ b.len() as u64
}

/// Max abs difference between RGBA (ours) and an RGB / gray PNM, plus mean abs difference.
pub fn diff_rgba_pnm(rgba: &[u8], w: usize, h: usize, pnm: &Pnm) -> (u8, f64) {
    assert_eq!((w, h), (pnm.w, pnm.h), "size mismatch");
    let mut max = 0u8;
    let mut sum = 0u64;
    for i in 0..w * h {
        for c in 0..3 {
            let a = rgba[i * 4 + c];
            let b = if pnm.channels == 1 {
                pnm.data[i]
            } else {
                pnm.data[i * 3 + c]
            };
            let d = a.abs_diff(b);
            max = max.max(d);
            sum += d as u64;
        }
    }
    (max, sum as f64 / (w * h * 3) as f64)
}

/// Max abs difference between a plane (with stride) and a gray PNM.
pub fn diff_plane_pgm(data: &[u8], stride: usize, w: usize, h: usize, pnm: &Pnm) -> (u8, f64) {
    assert_eq!((w, h), (pnm.w, pnm.h), "size mismatch");
    assert_eq!(pnm.channels, 1);
    let mut max = 0u8;
    let mut sum = 0u64;
    for y in 0..h {
        for x in 0..w {
            let d = data[y * stride + x].abs_diff(pnm.data[y * w + x]);
            max = max.max(d);
            sum += d as u64;
        }
    }
    (max, sum as f64 / (w * h) as f64)
}

pub fn uniq() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}_{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    )
}
