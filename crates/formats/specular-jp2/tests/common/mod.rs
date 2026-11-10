//! Shared helpers for the integration tests.
#![allow(dead_code)]

use specular_jp2::{DecodeOptions, Decoder, Depth, Image, Jp2Info, Pixels, Region};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

pub fn fixtures() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(data_dir())
        .expect("tests/data")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("j2k" | "jp2")))
        .collect();
    v.sort();
    v
}

/// The committed HTJ2K vectors (`tests/data/htj2k`, see `tests/gen_htj2k.sh`).
pub fn ht_fixtures() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(data_dir().join("htj2k"))
        .expect("tests/data/htj2k")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            matches!(
                p.extension().and_then(|e| e.to_str()),
                Some("jph" | "j2c" | "jhc")
            )
        })
        .collect();
    v.sort();
    v
}

pub fn name(p: &Path) -> String {
    p.file_name().unwrap().to_string_lossy().into_owned()
}

/// A decoded PNM: samples as `u16` at their native maxval.
pub struct Pnm {
    pub w: usize,
    pub h: usize,
    pub ch: usize,
    pub maxval: u32,
    pub data: Vec<u16>,
}

/// Parse a PAM (`P7`) as written by OpenJPEG.
fn read_pam(bytes: &[u8]) -> Option<Pnm> {
    let end = bytes.windows(7).position(|w| w == b"ENDHDR\n")?;
    let hdr = std::str::from_utf8(&bytes[..end]).ok()?;
    let (mut w, mut h, mut ch, mut maxval) = (0, 0, 0, 0);
    for line in hdr.lines() {
        let mut it = line.split_whitespace();
        let (Some(k), Some(v)) = (it.next(), it.next()) else {
            continue;
        };
        let v: u32 = v.parse().unwrap_or(0);
        match k {
            "WIDTH" => w = v as usize,
            "HEIGHT" => h = v as usize,
            "DEPTH" => ch = v as usize,
            "MAXVAL" => maxval = v,
            _ => {}
        }
    }
    let n = w * h * ch;
    let body = bytes.get(end + 7..)?;
    let data: Vec<u16> = if maxval > 255 {
        body.chunks_exact(2)
            .take(n)
            .map(|b| u16::from_be_bytes([b[0], b[1]]))
            .collect()
    } else {
        body.iter().take(n).map(|&b| b as u16).collect()
    };
    (data.len() == n).then_some(Pnm {
        w,
        h,
        ch,
        maxval,
        data,
    })
}

/// Parse a binary PGM/PPM/PAM (comments allowed, 8- or 16-bit).
pub fn read_pnm(bytes: &[u8]) -> Option<Pnm> {
    if bytes.starts_with(b"P7") {
        return read_pam(bytes);
    }
    let ch = match bytes.get(..2)? {
        b"P5" => 1,
        b"P6" => 3,
        _ => return None,
    };
    let mut pos = 2;
    let mut fields = [0u32; 3];
    for f in &mut fields {
        loop {
            while bytes.get(pos)?.is_ascii_whitespace() {
                pos += 1;
            }
            if bytes[pos] == b'#' {
                while bytes.get(pos)? != &b'\n' {
                    pos += 1;
                }
            } else {
                break;
            }
        }
        let s = pos;
        while bytes.get(pos)?.is_ascii_digit() {
            pos += 1;
        }
        *f = std::str::from_utf8(&bytes[s..pos]).ok()?.parse().ok()?;
    }
    pos += 1;
    let [w, h, maxval] = fields;
    let n = w as usize * h as usize * ch;
    let body = bytes.get(pos..)?;
    let data = if maxval > 255 {
        body.chunks_exact(2)
            .take(n)
            .map(|b| u16::from_be_bytes([b[0], b[1]]))
            .collect::<Vec<_>>()
    } else {
        body.iter().take(n).map(|&b| b as u16).collect()
    };
    (data.len() == n).then_some(Pnm {
        w: w as usize,
        h: h as usize,
        ch,
        maxval,
        data,
    })
}

/// Location of `opj_decompress`: `$SPECULAR_OPJ_DECOMPRESS`, the locally built copy under
/// `target/agent-app/jp2-tools`, or `PATH`. `None` when absent (tests then skip).
pub fn opj_decompress() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("SPECULAR_OPJ_DECOMPRESS") {
        return Some(PathBuf::from(p));
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let local = root.join("target/agent-app/jp2-tools/build/bin/opj_decompress");
    if local.is_file() {
        return Some(local);
    }
    let ok = std::process::Command::new("opj_decompress")
        .arg("-h")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok();
    ok.then(|| PathBuf::from("opj_decompress"))
}

pub fn scratch_dir(sub: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("specular-jp2")
        .join(sub);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Our output channel holding reference channel `c` of a `ref_ch`-channel PNM/PAM.
pub fn our_channel(ref_ch: usize, c: usize) -> usize {
    match (ref_ch, c) {
        (2, 1) => 3,
        (2, _) => 0,
        _ => c,
    }
}

/// Our 16-bit output channel `c` of pixel `i`, converted back to a native maxval.
pub fn native(px16: &[u16], chans: usize, i: usize, c: usize, maxval: u32) -> u32 {
    let v = px16[i * chans + c] as u64;
    ((v * maxval as u64 + 32767) / 65535) as u32
}

pub fn decode16(data: &[u8], reduce: u8, region: Option<Region>) -> Image {
    Decoder::new(data)
        .unwrap()
        .decode(&DecodeOptions {
            reduce,
            region,
            depth: Depth::Sixteen,
            ..Default::default()
        })
        .unwrap()
}

/// Run `opj_decompress` with extra arguments; `None` if it fails (e.g. a reduce it rejects).
pub fn opj(bin: &Path, file: &Path, info: &Jp2Info, tag: &str, args: &[String]) -> Option<Pnm> {
    let ext = if info.has_alpha {
        "pnm"
    } else if info.gray {
        "pgm"
    } else {
        "ppm"
    };
    let out = scratch_dir("opj").join(format!("{}.{tag}.{ext}", name(file)));
    let st = Command::new(bin)
        .arg("-i")
        .arg(file)
        .arg("-o")
        .arg(&out)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()?;
    if !st.success() {
        return None;
    }
    read_pnm(&std::fs::read(&out).ok()?)
}

/// Max and mean absolute difference over rows `0..rows`, in native sample units.
pub fn diff(ours: &Image, r: &Pnm, rows: usize) -> (u32, f64) {
    assert_eq!(
        (ours.width as usize, ours.height as usize),
        (r.w, r.h),
        "size mismatch"
    );
    let Pixels::U16(px) = &ours.pixels else {
        panic!("expected 16-bit output")
    };
    let chans = ours.format.channels();
    let (mut max, mut sum, mut n) = (0u32, 0u64, 0u64);
    for y in 0..rows.min(r.h) {
        for x in 0..r.w {
            let i = y * r.w + x;
            for c in 0..r.ch {
                let a = native(px, chans, i, our_channel(r.ch, c), r.maxval);
                let b = r.data[i * r.ch + c] as u32;
                let d = a.abs_diff(b);
                max = max.max(d);
                sum += d as u64;
                n += 1;
            }
        }
    }
    (max, sum as f64 / n.max(1) as f64)
}
