//! Encoders: streaming `cjpeg`, `magick` for CMYK, `exiftool` for metadata, and a small built-in
//! 12-bit (SOF1) encoder because libjpeg-turbo 2.1.5's `cjpeg` is 8-bit only.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::sync_channel;

use anyhow::{Context, Result, bail};
use rayon::prelude::*;

use crate::content::Scene;

#[derive(Clone, Debug, Default)]
pub struct CjpegOpts {
    pub quality: u8,
    /// `-sample` argument, e.g. "2x2" (4:2:0), "2x1" (4:2:2), "1x1" (4:4:4).
    pub sample: Option<&'static str>,
    pub progressive: bool,
    /// Restart interval in MCU rows.
    pub restart_rows: Option<u32>,
    pub arithmetic: bool,
    pub grayscale: bool,
    pub rgb: bool,
    pub optimize: bool,
}

fn tmp_path(out: &Path) -> PathBuf {
    let mut s = out.as_os_str().to_owned();
    s.push(".part");
    PathBuf::from(s)
}

/// Stream a PPM produced by `render(y, row)` into `cjpeg`, writing `out` atomically.
pub fn cjpeg_stream<F>(out: &Path, w: u32, h: u32, o: &CjpegOpts, render: F) -> Result<()>
where
    F: Fn(u32, &mut [u8]) + Sync,
{
    let tmp = tmp_path(out);
    let mut cmd = Command::new("cjpeg");
    cmd.arg("-quality").arg(o.quality.to_string());
    if let Some(s) = o.sample {
        cmd.arg("-sample").arg(s);
    }
    if o.progressive {
        cmd.arg("-progressive");
    }
    if let Some(r) = o.restart_rows {
        cmd.arg("-restart").arg(r.to_string());
    }
    if o.arithmetic {
        cmd.arg("-arithmetic");
    }
    if o.grayscale {
        cmd.arg("-grayscale");
    }
    if o.rgb {
        cmd.arg("-rgb");
    }
    if o.optimize {
        cmd.arg("-optimize");
    }
    cmd.arg("-outfile").arg(&tmp);
    let mut child = cmd
        .stdin(Stdio::piped())
        .spawn()
        .context("spawning cjpeg")?;
    let mut stdin = child.stdin.take().unwrap();
    let row_bytes = w as usize * 3;
    let band_rows = ((48 << 20) / row_bytes).clamp(1, h as usize);
    let (tx, rx) = sync_channel::<Vec<u8>>(2);
    let write_res = std::thread::scope(|sc| -> Result<()> {
        let writer =
            specular_thread::spawn_scoped(sc, "corpus-write", move || -> std::io::Result<()> {
                stdin.write_all(format!("P6\n{w} {h}\n255\n").as_bytes())?;
                for band in rx {
                    stdin.write_all(&band)?;
                }
                Ok(())
            });
        let mut y0 = 0usize;
        while y0 < h as usize {
            let rows = band_rows.min(h as usize - y0);
            let mut band = vec![0u8; rows * row_bytes];
            band.par_chunks_mut(row_bytes)
                .enumerate()
                .for_each(|(i, row)| render((y0 + i) as u32, row));
            if tx.send(band).is_err() {
                break;
            }
            y0 += rows;
        }
        drop(tx);
        writer
            .join()
            .expect("writer thread panicked")
            .context("writing PPM to cjpeg")?;
        Ok(())
    });
    let status = child.wait()?;
    write_res?;
    if !status.success() {
        bail!("cjpeg failed for {}", out.display());
    }
    std::fs::rename(&tmp, out)?;
    Ok(())
}

/// Encode an in-memory RGB buffer with cjpeg.
pub fn cjpeg_buffer(out: &Path, w: u32, h: u32, rgb: &[u8], o: &CjpegOpts) -> Result<()> {
    let rb = w as usize * 3;
    cjpeg_stream(out, w, h, o, |y, row| {
        row.copy_from_slice(&rgb[y as usize * rb..(y as usize + 1) * rb])
    })
}

pub fn write_ppm(path: &Path, w: u32, h: u32, rgb: &[u8]) -> Result<()> {
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    f.write_all(format!("P6\n{w} {h}\n255\n").as_bytes())?;
    f.write_all(rgb)?;
    f.flush()?;
    Ok(())
}

pub fn run(cmd: &mut Command) -> Result<()> {
    let out = cmd
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("spawning {cmd:?}"))?;
    if !out.status.success() {
        bail!("{cmd:?} failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    Ok(())
}

/// CMYK JPEG via ImageMagick (Adobe APP14, inverted CMYK as written by libjpeg).
pub fn magick_cmyk(out: &Path, w: u32, h: u32, rgb: &[u8], scratch: &Path) -> Result<()> {
    let ppm = scratch.join(format!(
        "{}.ppm",
        out.file_stem().unwrap().to_string_lossy()
    ));
    write_ppm(&ppm, w, h, rgb)?;
    let tmp = tmp_path(out);
    run(Command::new("magick")
        .arg(&ppm)
        .args([
            "-colorspace",
            "CMYK",
            "-quality",
            "92",
            "-sampling-factor",
            "1x1",
        ])
        .arg(format!("jpeg:{}", tmp.display())))?;
    std::fs::remove_file(&ppm).ok();
    std::fs::rename(&tmp, out)?;
    Ok(())
}

/// Plain (non-YCCK) Adobe CMYK JPEG via Python Pillow, which writes Adobe transform=0.
pub fn pillow_cmyk(out: &Path, w: u32, h: u32, rgb: &[u8], scratch: &Path) -> Result<()> {
    let ppm = scratch.join(format!(
        "{}.pil.ppm",
        out.file_stem().unwrap().to_string_lossy()
    ));
    write_ppm(&ppm, w, h, rgb)?;
    let tmp = tmp_path(out);
    run(Command::new("python3")
        .arg("-I")
        .arg("-c")
        .arg("import sys; from PIL import Image; Image.open(sys.argv[1]).convert('CMYK').save(sys.argv[2], format='JPEG', quality=92, subsampling=0)")
        .arg(&ppm)
        .arg(&tmp))?;
    std::fs::remove_file(&ppm).ok();
    std::fs::rename(&tmp, out)?;
    Ok(())
}

#[derive(Clone, Debug, Default)]
pub struct Meta {
    pub camera: Option<(String, String)>,
    pub orientation: Option<u8>,
    pub gps: Option<(f64, f64)>,
    pub icc: Option<PathBuf>,
    pub thumbnail: Option<PathBuf>,
    pub strip_jfif: bool,
}

impl Meta {
    pub fn is_empty(&self) -> bool {
        self.camera.is_none()
            && self.orientation.is_none()
            && self.gps.is_none()
            && self.icc.is_none()
            && self.thumbnail.is_none()
    }
}

pub fn exiftool(path: &Path, m: &Meta) -> Result<()> {
    if m.is_empty() {
        return Ok(());
    }
    let mut c = Command::new("exiftool");
    c.args(["-q", "-q", "-overwrite_original", "-n"]);
    if m.strip_jfif {
        c.arg("-JFIF:all=");
    }
    if let Some((make, model)) = &m.camera {
        c.arg(format!("-Make={make}"))
            .arg(format!("-Model={model}"))
            .arg("-DateTimeOriginal=2026:05:17 10:42:07")
            .arg("-CreateDate=2026:05:17 10:42:07")
            .arg("-ExposureTime=0.004")
            .arg("-FNumber=8")
            .arg("-ISO=100")
            .arg("-FocalLength=35")
            .arg("-Software=specular-corpus");
    }
    if let Some(o) = m.orientation {
        c.arg(format!("-Orientation={o}"));
    }
    if let Some((lat, lon)) = m.gps {
        c.arg(format!("-GPSLatitude={}", lat.abs()))
            .arg(format!(
                "-GPSLatitudeRef={}",
                if lat >= 0.0 { "N" } else { "S" }
            ))
            .arg(format!("-GPSLongitude={}", lon.abs()))
            .arg(format!(
                "-GPSLongitudeRef={}",
                if lon >= 0.0 { "E" } else { "W" }
            ))
            .arg("-GPSAltitude=12")
            .arg("-GPSAltitudeRef=0");
    }
    if let Some(icc) = &m.icc {
        c.arg(format!("-ICC_Profile<={}", icc.display()));
    }
    if let Some(t) = &m.thumbnail {
        c.arg(format!("-ThumbnailImage<={}", t.display()));
    }
    c.arg(path);
    run(&mut c)
}

/// Generate and attach an EXIF thumbnail rendered from the scene.
pub fn make_thumbnail(scene: &Scene, path: &Path) -> Result<()> {
    let (tw, th) = if scene.w >= scene.h {
        (160, (160 * scene.h / scene.w).max(8))
    } else {
        ((160 * scene.w / scene.h).max(8), 160)
    };
    let px = scene.thumbnail(tw, th);
    cjpeg_buffer(
        path,
        tw,
        th,
        &px,
        &CjpegOpts {
            quality: 85,
            optimize: true,
            ..Default::default()
        },
    )
}

// ---------------------------------------------------------------------------------------------
// Minimal 12-bit extended-sequential (SOF1, Huffman) encoder, 3 components, 4:4:4.

struct BitWriter {
    out: Vec<u8>,
    acc: u32,
    n: u32,
}

impl BitWriter {
    fn put(&mut self, code: u32, len: u32) {
        debug_assert!(len <= 16);
        self.acc = (self.acc << len) | (code & ((1 << len) - 1));
        self.n += len;
        while self.n >= 8 {
            let b = (self.acc >> (self.n - 8)) as u8;
            self.out.push(b);
            if b == 0xFF {
                self.out.push(0);
            }
            self.n -= 8;
        }
        self.acc &= (1 << self.n) - 1;
    }
    fn flush(&mut self) {
        if self.n > 0 {
            let pad = 8 - self.n;
            self.put((1 << pad) - 1, pad);
        }
    }
}

fn category(v: i32) -> u32 {
    32 - (v.unsigned_abs()).leading_zeros()
}

fn magnitude_bits(v: i32, cat: u32) -> u32 {
    if v >= 0 {
        v as u32
    } else {
        (v + (1 << cat) - 1) as u32
    }
}

const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

const STD_LUMA_Q: [u16; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56,
    14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113,
    92, 49, 64, 78, 87, 103, 121, 120, 101, 72, 92, 95, 98, 112, 100, 103, 99,
];

/// Write a 12-bit 4:4:4 YCbCr JPEG of the scene. Huffman tables use fixed-length codes covering
/// every symbol (DC: 16 symbols x 5 bits, AC: 226 symbols x 8 bits) so no statistics pass is needed.
pub fn encode_12bit(out: &Path, scene: &Scene) -> Result<()> {
    let (w, h) = (scene.w as usize, scene.h as usize);
    // Quant table in natural order: 12-bit data has 16x the range of 8-bit; this is roughly q90.
    let q: Vec<u16> = STD_LUMA_Q.iter().map(|&v| (v * 3).max(4)).collect();
    let mut bytes = vec![0xFF, 0xD8];
    // DQT, 16-bit precision (Pq=1), table 0.
    bytes.extend_from_slice(&[0xFF, 0xDB]);
    bytes.extend_from_slice(&((2 + 1 + 128) as u16).to_be_bytes());
    bytes.push(0x10);
    for &zz in ZIGZAG.iter() {
        bytes.extend_from_slice(&q[zz].to_be_bytes());
    }
    // SOF1
    bytes.extend_from_slice(&[0xFF, 0xC1]);
    bytes.extend_from_slice(&(8u16 + 3 * 3).to_be_bytes());
    bytes.push(12);
    bytes.extend_from_slice(&(h as u16).to_be_bytes());
    bytes.extend_from_slice(&(w as u16).to_be_bytes());
    bytes.push(3);
    for id in 1..=3u8 {
        bytes.extend_from_slice(&[id, 0x11, 0]);
    }
    // DHT: DC table 0 (symbols 0..=15, length 5) and AC table 0 (226 symbols, length 8).
    let dc_syms: Vec<u8> = (0..16).collect();
    let mut ac_syms: Vec<u8> = vec![0x00, 0xF0];
    for r in 0..16u8 {
        for s in 1..=14u8 {
            ac_syms.push((r << 4) | s);
        }
    }
    for (class, len, syms) in [(0x00u8, 5usize, &dc_syms), (0x10u8, 8usize, &ac_syms)] {
        bytes.extend_from_slice(&[0xFF, 0xC4]);
        bytes.extend_from_slice(&((2 + 1 + 16 + syms.len()) as u16).to_be_bytes());
        bytes.push(class);
        let mut counts = [0u8; 16];
        counts[len - 1] = syms.len() as u8;
        bytes.extend_from_slice(&counts);
        bytes.extend_from_slice(syms);
    }
    let mut ac_code = [0u32; 256];
    for (i, &s) in ac_syms.iter().enumerate() {
        ac_code[s as usize] = i as u32;
    }
    // SOS
    bytes.extend_from_slice(&[0xFF, 0xDA, 0x00, 12, 3, 1, 0x00, 2, 0x00, 3, 0x00, 0, 63, 0]);

    // Render image (small; fine to hold in memory) as YCbCr 12-bit.
    let mut planes = vec![vec![0f32; w * h]; 3];
    let mut row = vec![[0f32; 3]; w];
    for y in 0..h {
        scene.render_row_hp(y as u32, 4095.0, &mut row);
        for (x, p) in row.iter().enumerate() {
            let (r, g, b) = (p[0], p[1], p[2]);
            planes[0][y * w + x] = 0.299 * r + 0.587 * g + 0.114 * b;
            planes[1][y * w + x] = -0.168_736 * r - 0.331_264 * g + 0.5 * b + 2048.0;
            planes[2][y * w + x] = 0.5 * r - 0.418_688 * g - 0.081_312 * b + 2048.0;
        }
    }
    let mut cos = [[0f32; 8]; 8];
    for (x, row) in cos.iter_mut().enumerate() {
        for (u, c) in row.iter_mut().enumerate() {
            *c = (((2 * x + 1) as f32 * u as f32 * std::f32::consts::PI) / 16.0).cos();
        }
    }
    let mut bw = BitWriter {
        out: Vec::new(),
        acc: 0,
        n: 0,
    };
    let mut pred = [0i32; 3];
    let bw_blocks = w.div_ceil(8);
    let bh_blocks = h.div_ceil(8);
    for by in 0..bh_blocks {
        for bx in 0..bw_blocks {
            for (ci, plane) in planes.iter().enumerate() {
                let mut blk = [0f32; 64];
                for y in 0..8 {
                    for x in 0..8 {
                        let sx = (bx * 8 + x).min(w - 1);
                        let sy = (by * 8 + y).min(h - 1);
                        blk[y * 8 + x] = plane[sy * w + sx] - 2048.0;
                    }
                }
                let mut coef = [0i32; 64];
                for v in 0..8 {
                    for u in 0..8 {
                        let mut s = 0.0;
                        for y in 0..8 {
                            for x in 0..8 {
                                s += blk[y * 8 + x] * cos[x][u] * cos[y][v];
                            }
                        }
                        let cu = if u == 0 {
                            std::f32::consts::FRAC_1_SQRT_2
                        } else {
                            1.0
                        };
                        let cv = if v == 0 {
                            std::f32::consts::FRAC_1_SQRT_2
                        } else {
                            1.0
                        };
                        let f = 0.25 * cu * cv * s;
                        coef[v * 8 + u] = (f / q[v * 8 + u] as f32).round() as i32;
                    }
                }
                // DC
                let diff = coef[0] - pred[ci];
                pred[ci] = coef[0];
                let cat = category(diff);
                bw.put(cat, 5);
                if cat > 0 {
                    bw.put(magnitude_bits(diff, cat), cat);
                }
                // AC
                let mut run = 0;
                for &zz in ZIGZAG.iter().skip(1) {
                    let v = coef[zz];
                    if v == 0 {
                        run += 1;
                        continue;
                    }
                    while run >= 16 {
                        bw.put(ac_code[0xF0], 8);
                        run -= 16;
                    }
                    let cat = category(v).min(14);
                    let v = v.clamp(-(1 << cat) + 1, (1 << cat) - 1);
                    bw.put(ac_code[((run << 4) | cat) as usize], 8);
                    bw.put(magnitude_bits(v, cat), cat);
                    run = 0;
                }
                if run > 0 {
                    bw.put(ac_code[0x00], 8);
                }
            }
        }
    }
    bw.flush();
    bytes.extend_from_slice(&bw.out);
    bytes.extend_from_slice(&[0xFF, 0xD9]);
    let tmp = tmp_path(out);
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, out)?;
    Ok(())
}
