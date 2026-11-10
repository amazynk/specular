//! Shared by the fixture and corpus tests: 16-bit PAM reading and the comparisons against
//! the oracles' readings stored beside the files (`<name>.magick.pam` raw code values,
//! `.oiio.pam` OpenImageIO, `.ffmpeg.pam` FFmpeg, `.magicksrgb.pam` / `.magicklin.pam`
//! ImageMagick's conversions, `.oiiof.pam` OpenImageIO's float reading of signed samples, and
//! `.truth.pam` / `.truthf.pam` the code values or floats that tools/corpus/raster-film-gen.py
//! wrote, for the variants no other reader reads).
#![allow(dead_code)]

use specular_film::{Kind, Model, Pixels, cineon, decode, detect, dpx, info};
use std::path::Path;
use std::time::{Duration, Instant};

pub type Pam = (usize, usize, usize, Vec<u16>);

/// A PAM: (width, height, depth, samples scaled to 16 bits).
pub fn pam(p: &Path) -> Option<Pam> {
    let d = match std::fs::read(p) {
        Ok(d) => d,
        Err(_) => {
            use std::io::Read;
            let gz = std::fs::read(format!("{}.gz", p.display())).ok()?;
            let mut d = Vec::new();
            flate2::read::GzDecoder::new(&gz[..])
                .read_to_end(&mut d)
                .ok()?;
            d
        }
    };
    let end = d.windows(7).position(|w| w == b"ENDHDR\n")? + 7;
    let head = std::str::from_utf8(&d[..end]).ok()?;
    let key = |k: &str| -> Option<usize> {
        head.lines()
            .find_map(|l| l.strip_prefix(k))
            .and_then(|v| v.trim().parse().ok())
    };
    let (w, h, depth, max) = (
        key("WIDTH ")?,
        key("HEIGHT ")?,
        key("DEPTH ")?,
        key("MAXVAL ")?,
    );
    let body = &d[end..];
    let s: Vec<u16> = if max > 255 {
        body.chunks_exact(2)
            .map(|b| u16::from_be_bytes([b[0], b[1]]))
            .collect()
    } else {
        body.iter().map(|&b| b as u16 * 257).collect()
    };
    (s.len() >= w * h * depth).then_some((w, h, depth, s))
}

#[derive(Default)]
pub struct Tally {
    pub files: usize,
    pub pass: usize,
    pub mismatched_samples: u64,
    pub max_err: u32,
    pub failed: Vec<String>,
}

impl Tally {
    pub fn add(&mut self, name: &str, mism: u64, err: u32) {
        self.files += 1;
        self.mismatched_samples += mism;
        self.max_err = self.max_err.max(err);
        if mism == 0 {
            self.pass += 1;
        } else {
            self.failed
                .push(format!("{name} ({mism} samples, max {err})"));
        }
    }
    pub fn print(&self, what: &str) {
        println!(
            "{what}: {}/{} files pass, {} mismatched samples, max error {}",
            self.pass, self.files, self.mismatched_samples, self.max_err
        );
        for f in &self.failed {
            println!("  mismatch: {f}");
        }
    }
}

/// Sample `c` of pixel `i` of `want`, with grey expanded to RGB and grey + alpha to RGBA.
fn want_at(want: &Pam, i: usize, c: usize, comps: usize) -> Option<u16> {
    let (_, _, depth, s) = want;
    let k = match (comps, *depth) {
        (a, b) if a == b => c,
        (1, 3) | (1, 4) => 0,
        _ => return None,
    };
    s.get(i * depth + k).copied()
}

/// Our code values scaled to 16 bits as the oracles scale them, against `want`. Only the
/// first `present` samples are compared: past them a truncated file holds nothing to agree
/// on (readers fill the rest as they like).
pub fn compare_codes(
    codes: &[u16],
    present: usize,
    comps: usize,
    bits: u8,
    want: &Pam,
    tol: u32,
) -> (u64, u32) {
    let max = ((1u32 << bits) - 1) as f64;
    let (w, h, _, _) = want;
    if codes.len() != w * h * comps {
        return ((w * h * comps) as u64, u32::MAX);
    }
    let mut mism = 0;
    let mut worst = 0;
    for (k, &c) in codes.iter().enumerate().take(present) {
        let Some(e) = want_at(want, k / comps, k % comps, comps) else {
            return ((w * h * comps) as u64, u32::MAX);
        };
        // Readers widen codes to 16 bits by scaling (rounded or not), by shifting, or by
        // shifting with the top bits repeated: agreeing with any of them is agreeing on the
        // code value.
        let c64 = c as i64;
        let sh = 16 - bits.min(16) as i64;
        let e = e as i64;
        let err = [
            (c as f64 * 65535.0 / max).round() as i64,
            (c as f64 * 65535.0 / max) as i64,
            c64 << sh,
            if bits >= 8 {
                (c64 << sh) | (c64 >> (bits as i64 - sh).max(0))
            } else {
                c64 << sh
            },
        ]
        .iter()
        .map(|&o| (o - e).unsigned_abs() as u32)
        .min()
        .unwrap();
        // A 12-bit sample filled into 16 bits may be read back as the whole word.
        let err = if e >> sh == c64 { 0 } else { err };
        worst = worst.max(err);
        if err > tol {
            mism += 1;
        }
    }
    (mism, worst)
}

/// Our output (as 16-bit RGBA, floats clipped to 0..1) against `want`'s channels.
pub fn compare_display(px: &Pixels, want: &Pam, tol: u32) -> (u64, u32) {
    let (w, h, depth, s) = want;
    let n = w * h;
    let ours: Vec<[u16; 4]> = match px {
        Pixels::Gray8(v) => v.iter().map(|&g| [g as u16 * 257; 4]).collect(),
        Pixels::Gray16(v) => v.iter().map(|&g| [g; 4]).collect(),
        Pixels::Rgba8(v) => v
            .chunks_exact(4)
            .map(|p| [p[0], p[1], p[2], p[3]].map(|x| x as u16 * 257))
            .collect(),
        Pixels::Rgba16(v) => v
            .chunks_exact(4)
            .map(|p| [p[0], p[1], p[2], p[3]])
            .collect(),
        Pixels::RgbaF32(v) => v
            .chunks_exact(4)
            .map(|p| [p[0], p[1], p[2], p[3]].map(|x| (x.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16))
            .collect(),
    };
    if ours.len() != n || s.len() < n * depth {
        return (n as u64, u32::MAX);
    }
    let mut mism = 0;
    let mut worst = 0;
    for (i, o) in ours.iter().enumerate() {
        for c in 0..*depth {
            let e = s[i * depth + c];
            let mine = match (*depth, c) {
                (1, _) | (2, 0) => o[0],
                (2, _) => o[3],
                _ => o[c],
            };
            let err = (mine as i64 - e as i64).unsigned_abs() as u32;
            worst = worst.max(err);
            if err > tol {
                mism += 1;
            }
        }
    }
    (mism, worst)
}

#[derive(Default)]
pub struct Report {
    pub raw_magick: Tally,
    pub raw_oiio: Tally,
    pub raw_ffmpeg: Tally,
    /// The best of the three per file: agreeing with at least one reader.
    pub raw_truth: Tally,
    pub raw_either: Tally,
    pub conv: Tally,
    pub lin: Tally,
    pub refused: Vec<String>,
    pub opened: usize,
    pub timings: Vec<(String, u64, Duration)>,
    /// Print one `ORACLE<TAB>FILM/<ext><TAB>file<TAB>pass|fail|skip` line per file (for
    /// tools/ci/oracle-pass-rates.sh): pass when every comparison made agrees, fail on a
    /// mismatch, skip when we refuse the file (the corpus keeps damaged files on purpose) or no
    /// oracle reading was there to compare.
    pub oracle_lines: bool,
}

impl Report {
    /// Files compared and files mismatched over the per-file verdicts.
    fn counts(&self) -> (usize, usize) {
        let t = [&self.raw_either, &self.conv, &self.lin];
        (
            t.iter().map(|t| t.files).sum(),
            t.iter().map(|t| t.failed.len()).sum(),
        )
    }

    /// Decodes `file` and compares it with the oracle readings in `expect`.
    pub fn check(&mut self, file: &Path, expect: &Path) {
        let (refused, opened, (seen, failed)) = (self.refused.len(), self.opened, self.counts());
        self.check_file(file, expect);
        if !self.oracle_lines || (self.opened == opened && self.refused.len() == refused) {
            return;
        }
        let (seen2, failed2) = self.counts();
        let status = if failed2 > failed {
            "fail"
        } else if seen2 > seen && self.refused.len() == refused {
            "pass"
        } else {
            "skip"
        };
        let name = file.file_name().unwrap_or_default().to_string_lossy();
        let ext = file
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_uppercase())
            .unwrap_or_default();
        println!("ORACLE\tFILM/{ext}\t{name}\t{status}");
    }

    fn check_file(&mut self, file: &Path, expect: &Path) {
        let n = file.file_name().unwrap().to_string_lossy().to_string();
        let d = std::fs::read(file).unwrap();
        let Some(kind) = detect(&d) else { return };
        let i = match info(&d, 0) {
            Ok(i) => i,
            Err(e) => {
                self.refused.push(format!("{n}: {e}"));
                return;
            }
        };
        let t0 = Instant::now();
        let img = match decode(&d, 0) {
            Ok(img) => img,
            Err(e) => {
                self.refused.push(format!("{n}: {e}"));
                return;
            }
        };
        self.timings
            .push((n.clone(), i.width as u64 * i.height as u64, t0.elapsed()));
        self.opened += 1;
        let codes = match kind {
            Kind::Dpx => dpx::decode_codes_present(&d, 0)
                .ok()
                .map(|(w, _, ns, c, k)| (ns / w as usize, c, k, i.bit_depth)),
            Kind::Cineon => cineon::decode_codes(&d).ok().map(|(_, _, c, s, bits)| {
                let k = s.len();
                (c, s, k, bits)
            }),
        };
        let ycc = i.model == Model::YCbCr;
        let float = i.bit_depth >= 32;
        let exp = |tag: &str| pam(&expect.join(format!("{n}.{tag}.pam")));
        // ImageMagick read the stored codes as we do (its conversions are compared only then).
        let mut magick_codes = true;
        // Raw code values (ImageMagick's with the colourspace relabelled, OpenImageIO's).
        if let (Some((comps, codes, present, bits)), false, false) = (&codes, ycc, float)
            && !i.coding.contains("ABGR")
        {
            let bits = *bits;
            let mut best: Option<(u64, u32)> = None;
            if let Some(m) = exp("magick") {
                let (mism, err) = compare_codes(codes, *present, *comps, bits, &m, 1);
                self.raw_magick.add(&n, mism, err);
                best = Some((mism, err));
                magick_codes = mism == 0;
            }
            if let Some(o) = exp("oiio") {
                let (mism, err) = compare_codes(codes, *present, *comps, bits, &o, 1);
                self.raw_oiio.add(&n, mism, err);
                best = Some(best.map_or((mism, err), |b| b.min((mism, err))));
            }
            if let Some(o) = exp("ffmpeg") {
                let (mism, err) = compare_codes(codes, *present, *comps, bits, &o, 1);
                self.raw_ffmpeg.add(&n, mism, err);
                best = Some(best.map_or((mism, err), |b| b.min((mism, err))));
            }
            if let Some(o) = exp("truth") {
                let (mism, err) = compare_codes(codes, *present, *comps, bits, &o, 1);
                self.raw_truth.add(&n, mism, err);
                best = Some(best.map_or((mism, err), |b| b.min((mism, err))));
            }
            if let Some((mism, err)) = best {
                self.raw_either.add(&n, mism, err);
            }
        }
        // Signed samples against OpenImageIO's floats (−1 to 1, clipped to 0..1).
        if i.coding.contains("signed")
            && let Some(o) = exp("oiiof")
        {
            let (mism, err) = compare_display(&img.pixels, &o, 400);
            self.conv.add(&format!("{n} vs oiiof"), mism, err);
        }
        // YCbCr to RGB, floats.
        if ycc || float {
            // OpenImageIO reads floats as stored; we turn a video transfer's into linear light,
            // so that is undone here. ImageMagick converts YCbCr to sRGB.
            let video = ["BT.709", "BT.601", "SMPTE 274M", "video"]
                .iter()
                .any(|t| i.coding.contains(t));
            let mut px = img.pixels.clone();
            if let (true, Pixels::RgbaF32(v)) = (float && video, &mut px) {
                for (k, x) in v.iter_mut().enumerate() {
                    if k % 4 != 3 {
                        *x = if *x <= 0.003_130_8 {
                            *x * 12.92
                        } else {
                            1.055 * x.powf(1.0 / 2.4) - 0.055
                        };
                    }
                }
            }
            // What the generator wrote first (OpenImageIO ignores run-length encoding).
            let tag = match (float, exp("truthf").is_some()) {
                (true, true) => "truthf",
                (true, false) => "oiio",
                _ => "magicksrgb",
            };
            if let Some(o) = exp(tag) {
                let (mism, err) = compare_display(&px, &o, 1200);
                self.conv.add(&format!("{n} vs {tag}"), mism, err);
            }
        }
        // Log to linear against ImageMagick's (clipped to 1; its curve scales codes by
        // 1024/1023, up to about 0.6 % from Kodak's near white).
        if matches!(img.pixels, Pixels::RgbaF32(_))
            && !float
            && !ycc
            && magick_codes
            && let Some(o) = exp("magicklin")
        {
            let (mism, err) = compare_display(&img.pixels, &o, 400);
            self.lin.add(&n, mism, err);
        }
    }

    pub fn print(&mut self) {
        println!(
            "{} files opened, {} refused",
            self.opened,
            self.refused.len()
        );
        for r in &self.refused {
            println!("  refused: {r}");
        }
        self.raw_magick
            .print("code values vs ImageMagick (±1 of 65535)");
        self.raw_oiio
            .print("code values vs OpenImageIO (±1 of 65535)");
        self.raw_ffmpeg.print("code values vs FFmpeg (±1 of 65535)");
        self.raw_truth
            .print("code values vs what the generator wrote (±1 of 65535)");
        self.raw_either
            .print("code values vs at least one of the three (±1 of 65535)");
        self.conv.print("YCbCr / float vs oracles (±1200 of 65535)");
        self.lin
            .print("log to linear vs ImageMagick (±400 of 65535)");
        self.timings.sort_by_key(|t| std::cmp::Reverse(t.1));
        for (n, px, t) in self.timings.iter().take(5) {
            println!(
                "  decode {n}: {:.2} MP in {:.1} ms",
                *px as f64 / 1e6,
                t.as_secs_f64() * 1e3
            );
        }
    }
}
