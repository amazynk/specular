//! Per-camera oracle comparison with rawpy (LibRaw) of everything before the look: the
//! unpacked sensor values, black and white levels, the as-shot white balance, the colour
//! matrix, and the linear camera-space data (no white balance, no matrix) of the half-size
//! develop (no interpolation) and of the full-size demosaic. Corpus:
//! `tools/quality/fetch-raw-cameras.sh`.
//!
//!     SPECULAR_RAWPY=$CARGO_TARGET_DIR/venv/bin/python \
//!     SPECULAR_RAW_CAMERAS=$CARGO_TARGET_DIR/corpus/raw-cameras \
//!       cargo test -p specular-raw --test raw_cameras -- --ignored --nocapture
//!
//! `SPECULAR_QUALITY_ONLY` keeps the paths that contain it; `SPECULAR_QUALITY_OUT=<dir>`
//! also writes `<dir>/raw-cameras.tsv`. Prints one row per camera and fails on any value
//! outside the tolerances below (checks with a documented reason are reported, not failed).
//!
//! The colour matrix is compared as CIEDE2000 over the 24 ColorChecker patches: each patch
//! (linear sRGB) is taken back to white-balanced camera RGB through LibRaw's matrix and
//! forward through ours. The linear data is compared pixel by pixel for the half-size develop
//! and in 2x2 means of the full demosaic (where the two demosaics' edge handling averages
//! out), in black-subtracted raw units as a fraction of LibRaw's range, after a search
//! (within 24 pixels) for the crop offset.

use std::path::{Path, PathBuf};
use std::process::Command;

use specular_color::math::{self, Vec3};
use specular_raw::{DecodeOptions, DevelopStats, MatrixSource, OutputSize, RawImage};

/// Tolerances. Sensor values: bit-exact inside the crop. Levels: white exact, black within one
/// unit (LibRaw rounds a measured black; Canon's own per-channel levels sit one off LibRaw's on
/// some bodies, 0.01 % of the range). White balance: 0.5 % (both read the same tags;
/// LibRaw prints 6 digits). Linear data (in raw units): half size (no interpolation) channel
/// gain within 0.5 % and mean error 0.25 % of the range; full demosaic (2x2 means) gain within
/// 1 % and mean error 1 % (LibRaw's own AHD, VNG and bilinear differ by up to 1.0 % in gain
/// on the 300D file).
///
/// The matrix is reported, not gated: where both use the dcraw-era Adobe table it agrees to
/// ΔE00 0.00 (1300D, GH5, Minolta, Kodak, Hasselblad); the others use newer Adobe or camera
/// DNG data on purpose, or interpolate a DNG's two matrices by the shot's white balance (DNG
/// spec) where LibRaw takes one.
const WB_TOL: f64 = 0.005;
const HALF_GAIN_TOL: f64 = 0.005;
const FULL_GAIN_TOL: f64 = 0.01;
const HALF_MEAN_TOL: f64 = 0.0025;
const FULL_MEAN_TOL: f64 = 0.01;

/// Where we knowingly differ from LibRaw: (camera substring, check, reason). Those checks are
/// printed, not failed.
const KNOWN: &[(&str, &str, &str)] = &[
    (
        "Canon EOS 1300D",
        "white",
        "LibRaw's table has 14274 (dcraw's 13584); the Rebel T6 file shows the data runs \
         evenly past both and clips at 16383, ours",
    ),
    (
        "Canon EOS Rebel T6",
        "white",
        "LibRaw's table has 14274 (dcraw's 13584); this file's samples run evenly past both \
         (7075 greens above 14274) and pile up only at 16383, ours",
    ),
    (
        "Canon EOS 6D (",
        "wb",
        "LibRaw reads no as-shot white balance from this file (unity); ours is 1.68 / 2.04",
    ),
    (
        "Canon EOS 6D (",
        "half",
        "LibRaw's develop of this file is off (green 7x red in camera space); sensor values agree",
    ),
    (
        "Canon EOS 6D (",
        "full",
        "LibRaw's develop of this file is off (green 7x red in camera space); sensor values agree",
    ),
    (
        "iPhone",
        "half",
        "LibRaw's half size rescales linear (demosaiced) DNG data; its full size matches exactly",
    ),
    // The broad sample (fetch-raw-cameras.sh --broad), checked on 2026-10-24:
    (
        "5G4A9394-compressed-lossy",
        "half",
        "Adobe's lossy DNG is linear (demosaiced) data: LibRaw's half size rescales it, its \
         full size matches",
    ),
    (
        "iPhone 7 Plus",
        "full",
        "we apply the DNG FixVignetteRadial opcode (exiftool lists it); LibRaw applies no \
         opcodes, so the corners differ",
    ),
    (
        "DJI FC220",
        "half",
        "we apply the DNG FixVignetteRadial and FixBadPixelsList opcodes; LibRaw applies none",
    ),
    (
        "DJI FC220",
        "full",
        "we apply the DNG FixVignetteRadial and FixBadPixelsList opcodes; LibRaw applies none",
    ),
    (
        "Nikon D1 (",
        "wb",
        "LibRaw reads no as-shot white balance from this file (unity)",
    ),
    // Fidelity round 6: LibRaw's own demosaics (VNG, PPG, DCB, bilinear) differ from its AHD
    // on these files by more than the tolerance, the same way as ours (camera space, 2x2
    // means; half size and sensor values agree exactly).
    (
        "Pentax K-x (",
        "full",
        "LibRaw's VNG/PPG/DCB/bilinear give blue 0.979-0.986 of its AHD on this ISO 2000 file; \
         ours 0.988",
    ),
    // Fidelity round 8.
    (
        "Panasonic DMC-LX3 (",
        "full",
        "LibRaw's AHD gives blue 0.961 of its half size on this file; its bilinear 1.000, VNG \
         1.008, PPG 1.015, DCB 1.014; ours 1.010 (camera space, means)",
    ),
    (
        "Fujifilm X-T1 (",
        "half",
        "LibRaw's X-Trans half size keeps one sample per colour of each 2x2 block (dcraw's \
         shrink; red and blue from the neighbours where the block has none: 0.15 % mean from \
         an emulation of that); ours averages each colour over the 3x3 window around the \
         block (an emulation of ours: 0.62 %, as measured)",
    ),
    (
        "Nikon Coolpix P7000 (",
        "full",
        "LibRaw's VNG/PPG/DCB/bilinear give red 1.005-1.008, blue 0.982-0.989 of its AHD; ours \
         1.010 / 0.989",
    ),
];

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            files(&p, out);
        } else if !p
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        {
            out.push(p);
        }
    }
}

fn u32le(b: &[u8], i: usize) -> usize {
    u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap()) as usize
}

/// An RGB image of f64 samples (camera space, white = 65535).
struct Img {
    w: usize,
    h: usize,
    v: Vec<[f64; 3]>,
}

impl Img {
    fn read(p: &Path) -> Option<Img> {
        let b = std::fs::read(p).ok()?;
        let (h, w) = (u32le(&b, 0), u32le(&b, 1));
        let v = b[8..]
            .chunks_exact(12)
            .map(|c| {
                let f = |i: usize| f32::from_le_bytes(c[i * 4..i * 4 + 4].try_into().unwrap());
                [f(0) as f64, f(1) as f64, f(2) as f64]
            })
            .collect::<Vec<_>>();
        (v.len() == w * h).then_some(Img { w, h, v })
    }

    /// k x k block means of interleaved u16 RGB.
    fn blocks(w: usize, h: usize, d: &[u16], k: usize) -> Img {
        let (bw, bh) = (w / k, h / k);
        let mut v = vec![[0f64; 3]; bw * bh];
        for (i, o) in v.iter_mut().enumerate() {
            let (bx, by) = (i % bw, i / bw);
            for dy in 0..k {
                for dx in 0..k {
                    let s = ((by * k + dy) * w + bx * k + dx) * 3;
                    for c in 0..3 {
                        o[c] += d[s + c] as f64;
                    }
                }
            }
            for c in o.iter_mut() {
                *c /= (k * k) as f64;
            }
        }
        Img { w: bw, h: bh, v }
    }
}

/// Linear-data agreement of ours against the oracle, both in black-subtracted raw units (so a
/// different white level, a choice, does not read as a gain): the offset found, per-channel
/// gain of ours over theirs, mean |error|, 99th percentile and max as a fraction of LibRaw's
/// range, over the blocks below `clip` on both sides (clipping differs by design).
struct LinCmp {
    dx: i64,
    dy: i64,
    gain: [f64; 3],
    mean: f64,
    p99: f64,
    max: f64,
}

fn compare_lin(a: &Img, b: &Img, clip: f64, norm: f64) -> Option<LinCmp> {
    let pairs = |dx: i64, dy: i64, step: usize| {
        let mut out = Vec::new();
        for y in (0..a.h).step_by(step) {
            let oy = y as i64 + dy;
            if oy < 0 || oy as usize >= b.h {
                continue;
            }
            for x in (0..a.w).step_by(step) {
                let ox = x as i64 + dx;
                if ox < 0 || ox as usize >= b.w {
                    continue;
                }
                let p = a.v[y * a.w + x];
                let q = b.v[oy as usize * b.w + ox as usize];
                if p.iter().chain(&q).all(|&v| v < clip) {
                    out.push((p, q));
                }
            }
        }
        out
    };
    let mad = |v: &[([f64; 3], [f64; 3])]| {
        v.iter()
            .map(|(p, q)| (0..3).map(|c| (p[c] - q[c]).abs()).sum::<f64>())
            .sum::<f64>()
            / v.len().max(1) as f64
    };
    let r = (6i64
        + (a.w as i64 - b.w as i64)
            .abs()
            .max((a.h as i64 - b.h as i64).abs()))
    .min(24);
    let mut best = (f64::INFINITY, 0, 0);
    for dy in -r..=r {
        for dx in -r..=r {
            let v = pairs(dx, dy, 7);
            if v.len() > 100 {
                let m = mad(&v);
                if m < best.0 {
                    best = (m, dx, dy);
                }
            }
        }
    }
    if !best.0.is_finite() {
        return None;
    }
    let v = pairs(best.1, best.2, 1);
    let mut gain = [0f64; 3];
    for c in 0..3 {
        let sa: f64 = v.iter().map(|(p, _)| p[c]).sum();
        let sb: f64 = v.iter().map(|(_, q)| q[c]).sum();
        gain[c] = sa / sb.max(1e-9);
    }
    let mut errs: Vec<f64> = v
        .iter()
        .flat_map(|(p, q)| (0..3).map(move |c| (p[c] - q[c]).abs() / norm))
        .collect();
    errs.sort_by(f64::total_cmp);
    let n = errs.len();
    Some(LinCmp {
        dx: best.1,
        dy: best.2,
        gain,
        mean: errs.iter().sum::<f64>() / n as f64,
        p99: errs[(n * 99 / 100).min(n - 1)],
        max: errs[n - 1],
    })
}

/// Sensor samples that differ inside our crop (trying offsets when the stored sizes differ):
/// (mismatches, compared).
fn compare_sensor(
    raw: &RawImage,
    h: usize,
    w: usize,
    c: usize,
    data: &[u16],
) -> Option<(u64, u64, u64)> {
    let (rw, rh, rc) = (raw.width as usize, raw.height as usize, raw.cpp as usize);
    if rc != c || raw.window.w as usize != rw || raw.window.h as usize != rh {
        return None;
    }
    let cr = raw.crop;
    // LibRaw's raw_image holds DNG samples through their LinearizationTable (Leica M8/M9
    // store 8-bit codes); compare ours the same way.
    let lin = raw.linearization.as_ref().filter(|t| !t.is_empty());
    let ours = |i: usize| {
        let v = raw.data[i];
        lin.map_or(v, |t| t[usize::from(v).min(t.len() - 1)])
    };
    // Samples the DNG's OpcodeList1/2 bad-pixel opcodes replace (we apply them before the
    // samples are compared; LibRaw applies none): counted apart (fidelity round 8, DJI FC220).
    let fixed_at = |y: usize, x: usize, v: u16| {
        raw.opcodes
            .as_ref()
            .is_some_and(|o| o.fixes_pixel(y as u32, x as u32, v))
    };
    let cmp = |dx: i64, dy: i64, step: usize| {
        let (mut bad, mut n, mut fixed) = (0u64, 0u64, 0u64);
        for y in (cr.y as usize..(cr.y + cr.h) as usize).step_by(step) {
            let oy = y as i64 + dy;
            if oy < 0 || oy as usize >= h {
                continue;
            }
            for x in cr.x as usize..(cr.x + cr.w) as usize {
                let ox = x as i64 + dx;
                if ox < 0 || ox as usize >= w {
                    continue;
                }
                for k in 0..c {
                    n += 1;
                    let theirs = data[(oy as usize * w + ox as usize) * c + k];
                    if ours((y * rw + x) * c + k) != theirs {
                        if fixed_at(y, x, theirs) {
                            fixed += 1;
                        } else {
                            bad += 1;
                        }
                    }
                }
            }
        }
        (bad, n, fixed)
    };
    if rw == w && rh == h {
        return Some(cmp(0, 0, 1));
    }
    let (rx, ry) = (
        (rw as i64 - w as i64).abs().min(64),
        (rh as i64 - h as i64).abs().min(64),
    );
    let mut best = (u64::MAX, 0, 0);
    for dy in -ry..=ry {
        for dx in -rx..=rx {
            let (b, n, _) = cmp(dx, dy, 97);
            if n > 0 && b < best.0 {
                best = (b, dx, dy);
            }
        }
    }
    Some(cmp(best.1, best.2, 1))
}

/// The 24 ColorChecker patches (BabelColor averages, linear sRGB / BT.709).
fn colorchecker() -> Vec<[f64; 3]> {
    const SRGB8: [[u8; 3]; 24] = [
        [115, 82, 68],
        [194, 150, 130],
        [98, 122, 157],
        [87, 108, 67],
        [133, 128, 177],
        [103, 189, 170],
        [214, 126, 44],
        [80, 91, 166],
        [193, 90, 99],
        [94, 60, 108],
        [157, 188, 64],
        [224, 163, 46],
        [56, 61, 150],
        [70, 148, 73],
        [175, 54, 60],
        [231, 199, 31],
        [187, 86, 149],
        [8, 133, 161],
        [243, 243, 242],
        [200, 200, 200],
        [160, 160, 160],
        [122, 122, 121],
        [85, 85, 85],
        [52, 52, 52],
    ];
    SRGB8
        .iter()
        .map(|p| p.map(|v| math::srgb_to_linear(v as f32 / 255.0) as f64))
        .collect()
}

fn inv3(m: [[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let d = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if d.abs() < 1e-12 {
        return None;
    }
    let c =
        |r0: usize, c0: usize, r1: usize, c1: usize| m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0];
    Some([
        [c(1, 1, 2, 2) / d, -c(0, 1, 2, 2) / d, c(0, 1, 1, 2) / d],
        [-c(1, 0, 2, 2) / d, c(0, 0, 2, 2) / d, -c(0, 0, 1, 2) / d],
        [c(1, 0, 2, 1) / d, -c(0, 0, 2, 1) / d, c(0, 0, 1, 1) / d],
    ])
}

fn mul3(m: &[[f64; 3]; 3], v: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|r| m[r][0] * v[0] + m[r][1] * v[1] + m[r][2] * v[2])
}

fn lab(rgb: [f64; 3]) -> Vec3 {
    let xyz = math::bt709_to_pcs().apply64(rgb);
    math::xyz_to_lab([xyz[0] as f32, xyz[1] as f32, xyz[2] as f32])
}

/// ColorChecker ΔE00 (mean, max) between our rgb_cam and LibRaw's, and the largest
/// element difference.
fn compare_matrix(ours: &[[f32; 3]; 3], theirs: &[f64]) -> Option<(f64, f64, f64)> {
    if theirs.len() != 9 {
        return None;
    }
    let t = [
        [theirs[0], theirs[1], theirs[2]],
        [theirs[3], theirs[4], theirs[5]],
        [theirs[6], theirs[7], theirs[8]],
    ];
    let o = ours.map(|r| r.map(|v| v as f64));
    let ti = inv3(t)?;
    let mut el = 0f64;
    for r in 0..3 {
        for c in 0..3 {
            el = el.max((o[r][c] - t[r][c]).abs());
        }
    }
    let (mut sum, mut max) = (0f64, 0f64);
    let cc = colorchecker();
    for p in &cc {
        let cam = mul3(&ti, *p);
        let a = mul3(&o, cam).map(|v| v.max(0.0));
        let de = math::delta_e2000(lab(a), lab(*p)) as f64;
        sum += de;
        max = max.max(de);
    }
    Some((sum / cc.len() as f64, max, el))
}

/// How a file's highlights develop where its data clips.
struct Clip {
    /// LibRaw's largest visible sample, and how many samples have it.
    data_max: f64,
    samples: u64,
    /// Half-size blocks whose two greens are clipped, and those of them that develop (as-shot
    /// white balance, no matrix) with green below full scale: magenta.
    blocks: u64,
    magenta: u64,
    /// Clipped green as a fraction of full scale with the stated white level instead.
    stated_green: f64,
}

/// `None` unless the data clips: LibRaw's largest visible sample is shared by at least four
/// samples and lies in the top quarter of the range (where LibRaw's default develop lowers its
/// maximum to it). A green sample counts as clipped at the largest value of its 2x2 position.
fn highlights(raw: &RawImage, o: &Oracle) -> Option<Clip> {
    let data_max = o.fact("data_max")?.parse::<f64>().ok()?;
    let samples = o.fact("data_max_n")?.parse::<u64>().ok()?;
    let black = raw.black.iter().map(|&v| v as f64).sum::<f64>() / 4.0;
    let white = raw.white as f64;
    if samples < 4 || data_max < black + 0.75 * (white - black) {
        return None;
    }
    let mut clip = Clip {
        data_max,
        samples,
        blocks: 0,
        magenta: 0,
        stated_green: ((data_max - black) / (white - black)).min(1.0),
    };
    let p = raw.cfa.as_ref().and_then(|c| c.bayer2x2());
    let (Some(p), 1) = (p, raw.cpp) else {
        return Some(clip);
    };
    let mut wb = raw.clone();
    wb.rgb_cam = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    wb.profile = None;
    wb.matrix_source = MatrixSource::Identity;
    let opts = DecodeOptions {
        size: OutputSize::Half,
        ..Default::default()
    };
    let img = specular_raw::develop(&wb, &opts).ok()?;
    let (w, c) = (raw.width as usize, raw.crop);
    let at = |x: usize, y: usize| raw.data[y * w + x];
    let mut top = [0u16; 4];
    for y in c.y as usize..(c.y + c.h) as usize {
        for x in c.x as usize..(c.x + c.w) as usize {
            let k = (y & 1) * 2 + (x & 1);
            top[k] = top[k].max(at(x, y));
        }
    }
    for by in 0..img.height as usize {
        for bx in 0..img.width as usize {
            let (x0, y0) = (c.x as usize + 2 * bx, c.y as usize + 2 * by);
            let greens = (0..4).all(|d| {
                let (x, y) = (x0 + (d & 1), y0 + (d >> 1));
                let k = (y & 1) * 2 + (x & 1);
                p[k] != 1 || at(x, y) >= top[k]
            });
            if !greens {
                continue;
            }
            clip.blocks += 1;
            let i = (by * img.width as usize + bx) * 3;
            if img.data[i + 1] < 65535 {
                clip.magenta += 1;
            }
        }
    }
    Some(clip)
}

/// Our camera-space develop: no white balance, identity matrix, no profile.
fn camera_space(raw: &RawImage) -> RawImage {
    let mut r = raw.clone();
    r.wb = Some([1.0; 3]);
    r.rgb_cam = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    r.profile = None;
    r.matrix_source = MatrixSource::Identity;
    r
}

struct Oracle {
    facts: Vec<(String, String)>,
    sensor: Option<(usize, usize, usize, Vec<u16>)>,
}

impl Oracle {
    fn fact(&self, k: &str) -> Option<&str> {
        self.facts
            .iter()
            .find(|(a, _)| a == k)
            .map(|(_, v)| v.as_str())
    }
    fn nums(&self, k: &str) -> Vec<f64> {
        self.fact(k)
            .unwrap_or("")
            .split(',')
            .filter_map(|v| v.parse().ok())
            .collect()
    }
}

fn oracle(py: &str, file: &Path, prefix: &Path) -> Result<Oracle, String> {
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/quality/rawpy_linear.py");
    let out = Command::new(py)
        .arg("-I")
        .arg(&script)
        .arg(file)
        .arg(prefix)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(err.lines().last().unwrap_or("rawpy failed").to_string());
    }
    let facts = String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let sensor = std::fs::read(prefix.with_extension("raw")).ok().map(|b| {
        let (h, w, c) = (u32le(&b, 0), u32le(&b, 1), u32le(&b, 2));
        let d = b[12..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        (h, w, c, d)
    });
    Ok(Oracle { facts, sensor })
}

fn camera_of(root: &Path, p: &Path) -> String {
    let rel = p.strip_prefix(root).unwrap_or(p);
    let parts: Vec<_> = rel
        .iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    if parts.len() >= 3 {
        format!("{} {} ({})", parts[0], parts[1], parts[parts.len() - 1])
    } else {
        rel.display().to_string()
    }
}

#[test]
#[ignore = "oracle runner: needs tools/quality/fetch-raw-cameras.sh and SPECULAR_RAWPY"]
fn raw_cameras_vs_rawpy() {
    let Ok(py) = std::env::var("SPECULAR_RAWPY") else {
        eprintln!("SPECULAR_RAWPY unset; skipping");
        return;
    };
    let root = std::env::var_os("SPECULAR_RAW_CAMERAS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/quality/corpus/raw-cameras")
        });
    let only = std::env::var("SPECULAR_QUALITY_ONLY").unwrap_or_default();
    let mut paths = Vec::new();
    files(&root, &mut paths);
    paths.sort();
    paths.retain(|p| p.to_string_lossy().contains(&only));
    assert!(!paths.is_empty(), "no RAW files under {}", root.display());
    let tmp = std::env::temp_dir().join(format!("raw-cameras-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let mut tsv = String::from(
        "camera\tsensor_mismatch\tblack_d\twhite_d\twb_err\tmatrix\tde_mean\tde_max\t\
         half_gain\thalf_mean\thalf_p99\thalf_max\tfull_gain\tfull_mean\tfull_p99\tfull_max\n",
    );
    let mut bad = Vec::new();
    for p in &paths {
        let cam = camera_of(&root, p);
        let ext = p
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_uppercase())
            .unwrap_or_default();
        let data = std::fs::read(p).unwrap();
        let raw = match specular_raw::parse(&data)
            .and_then(|r| r.decode_raw(&data, &DecodeOptions::default()))
        {
            Ok(r) => r,
            Err(e) => {
                println!("{cam}: we refuse: {e}");
                println!("ORACLE\tRAW/{ext}\t{cam}\tskip");
                continue;
            }
        };
        let o = match oracle(&py, p, &tmp.join("o")) {
            Ok(o) => o,
            Err(e) => {
                println!("{cam}: LibRaw refuses: {e}");
                println!("ORACLE\tRAW/{ext}\t{cam}\tskip");
                continue;
            }
        };
        let known = |check: &str| {
            KNOWN
                .iter()
                .find(|(k, c, _)| cam.contains(k) && *c == check)
                .map(|(_, _, r)| *r)
        };
        let mut fail = Vec::new();

        // Sensor values.
        let sensor = o
            .sensor
            .as_ref()
            .and_then(|(h, w, c, d)| compare_sensor(&raw, *h, *w, *c, d));
        let sensor_txt = match sensor {
            Some((b, n, fixed)) => {
                if b > 0 {
                    fail.push(format!("{b} of {n} sensor samples differ"));
                }
                if fixed > 0 {
                    format!("{b}/{n} (and {fixed} at the bad pixels the DNG's opcodes fix)")
                } else {
                    format!("{b}/{n}")
                }
            }
            None => "n/a".into(),
        };

        // Levels: LibRaw's per-colour blacks against ours per 2x2 position (sorted: the two
        // orders differ), and the white level.
        let mut ob: Vec<f64> = raw.black.iter().map(|&v| v as f64).collect();
        let mut lb = o.nums("black");
        ob.sort_by(f64::total_cmp);
        lb.sort_by(f64::total_cmp);
        let black_d = if raw.cpp == 1 && lb.len() == 4 {
            ob.iter()
                .zip(&lb)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f64::max)
        } else {
            f64::NAN
        };
        let white_d = o
            .fact("white")
            .and_then(|v| v.parse::<f64>().ok())
            .map_or(f64::NAN, |w| raw.white as f64 - w);
        if black_d > 1.0 {
            fail.push(format!("black differs by {black_d}"));
        }
        if white_d.abs() > 0.5 {
            match known("white") {
                Some(r) => println!("  white differs by {white_d}: known ({r})"),
                None => fail.push(format!("white differs by {white_d}")),
            }
        }

        // As-shot white balance (G = 1).
        let lw = o.nums("wb");
        let wb_err = match (raw.wb, lw.len() >= 3 && lw[1] > 0.0) {
            (Some(w), true) => {
                let e0 = (w[0] as f64 / w[1] as f64) / (lw[0] / lw[1]) - 1.0;
                let e2 = (w[2] as f64 / w[1] as f64) / (lw[2] / lw[1]) - 1.0;
                if e0.abs() > e2.abs() { e0 } else { e2 }
            }
            _ => f64::NAN,
        };
        if wb_err.abs() > WB_TOL {
            match known("wb") {
                Some(r) => println!("  white balance differs: known ({r})"),
                None => fail.push(format!("white balance differs by {:+.2} %", wb_err * 100.0)),
            }
        }

        // Colour matrix.
        let m = compare_matrix(&raw.rgb_cam, &o.nums("rgb_cam"));
        let (de_mean, de_max) = m.map_or((f64::NAN, f64::NAN), |(a, b, _)| (a, b));

        // Highlights.
        let dev_white = DevelopStats::of(&raw).map_or(raw.white, |s| s.white()) as f64;
        let hl = highlights(&raw, &o);
        if let Some(h) = &hl {
            if (dev_white - h.data_max).abs() > 0.5 {
                fail.push(format!(
                    "develop white {dev_white} where the data clips at {}",
                    h.data_max
                ));
            }
            if h.magenta > 0 {
                fail.push(format!(
                    "{} of {} clipped blocks below full-scale green (magenta)",
                    h.magenta, h.blocks
                ));
            }
        }

        // Linear camera-space data.
        let cs = camera_space(&raw);
        let mut lin = Vec::new();
        for (name, size, k, tol, gain_tol) in [
            ("half", OutputSize::Half, 1, HALF_MEAN_TOL, HALF_GAIN_TOL),
            ("full", OutputSize::Full, 2, FULL_MEAN_TOL, FULL_GAIN_TOL),
        ] {
            let theirs = Img::read(&tmp.join(format!("o.{name}")));
            let opts = DecodeOptions {
                size,
                ..Default::default()
            };
            let ours = specular_raw::develop(&cs, &opts)
                .ok()
                .map(|i| Img::blocks(i.width as usize, i.height as usize, &i.data, k));
            let c = match (ours, theirs) {
                (Some(mut a), Some(mut b)) => {
                    // Both to black-subtracted raw units (ours scaled by the develop's white,
                    // which is the data's clip where it shows one).
                    let ra = dev_white - raw.black.iter().map(|&v| v as f64).sum::<f64>() / 4.0;
                    let lb = o.nums("black");
                    let rb = o
                        .fact("white")
                        .and_then(|v| v.parse::<f64>().ok())
                        .unwrap_or(65535.0)
                        - lb.iter().sum::<f64>() / lb.len().max(1) as f64;
                    for (img, r) in [(&mut a, ra), (&mut b, rb)] {
                        for px in img.v.iter_mut() {
                            for v in px.iter_mut() {
                                *v *= r / 65535.0;
                            }
                        }
                    }
                    compare_lin(&a, &b, 0.9 * ra.min(rb), rb)
                }
                _ => None,
            };
            if let Some(c) = c.as_ref().filter(|_| {
                known(name)
                    .inspect(|r| println!("  {name}: known ({r})"))
                    .is_none()
            }) {
                let g = c.gain.iter().map(|g| (g - 1.0).abs()).fold(0.0, f64::max);
                if g > gain_tol {
                    fail.push(format!("{name}: channel gain {:.4?}", c.gain));
                }
                if c.mean > tol {
                    fail.push(format!("{name}: mean error {:.3} %", c.mean * 100.0));
                }
            }
            lin.push(c);
            let _ = std::fs::remove_file(tmp.join(format!("o.{name}")));
        }
        let _ = std::fs::remove_file(tmp.join("o.raw"));

        let fmt_lin = |c: &Option<LinCmp>| match c {
            Some(c) => format!(
                "gain {:.4}/{:.4}/{:.4} mean {:.3}% p99 {:.2}% max {:.1}% at ({}, {})",
                c.gain[0],
                c.gain[1],
                c.gain[2],
                c.mean * 100.0,
                c.p99 * 100.0,
                c.max * 100.0,
                c.dx,
                c.dy
            ),
            None => "n/a".into(),
        };
        let hl_txt = match &hl {
            Some(h) => format!(
                "clips at {} ({} samples): develop white {dev_white}, {} clipped blocks, {} magenta \
                 (the stated white alone gives clipped green {:.4})",
                h.data_max, h.samples, h.blocks, h.magenta, h.stated_green
            ),
            None => format!("no clip shown, develop white {dev_white}"),
        };
        println!(
            "{cam}: sensor {sensor_txt}; black {:?} vs {:?}, white {} vs {}; highlights: {hl_txt}; \
             wb {:?} vs {:?} ({:+.3}%); matrix {:?} ΔE00 mean {de_mean:.2} max {de_max:.2}; \
             half {}; full {}",
            raw.black,
            o.nums("black"),
            raw.white,
            o.fact("white").unwrap_or("?"),
            raw.wb,
            o.nums("wb"),
            wb_err * 100.0,
            raw.matrix_source,
            fmt_lin(&lin[0]),
            fmt_lin(&lin[1]),
        );
        let col = |c: &Option<LinCmp>| match c {
            Some(c) => format!(
                "{:.4},{:.4},{:.4}\t{:.5}\t{:.5}\t{:.5}",
                c.gain[0], c.gain[1], c.gain[2], c.mean, c.p99, c.max
            ),
            None => "\t\t\t".into(),
        };
        tsv += &format!(
            "{cam}\t{sensor_txt}\t{black_d}\t{white_d}\t{wb_err:.5}\t{:?}\t{de_mean:.3}\t{de_max:.3}\t{}\t{}\n",
            raw.matrix_source,
            col(&lin[0]),
            col(&lin[1])
        );
        let status = if fail.is_empty() { "pass" } else { "fail" };
        // One line per file for tools/ci/oracle-pass-rates.sh.
        println!("ORACLE\tRAW/{ext}\t{cam}\t{status}");
        if !fail.is_empty() {
            bad.push(format!("{cam}: {}", fail.join("; ")));
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    if let Some(out) = std::env::var_os("SPECULAR_QUALITY_OUT").map(PathBuf::from) {
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join("raw-cameras.tsv"), tsv).unwrap();
    }
    assert!(bad.is_empty(), "{bad:#?}");
}
