//! Stored layouts the converter does not read directly, and their codecs: integer samples of
//! 17–31, 24 or 33–64 bits (widened to 32), SGI LogL / LogLuv (to linear float), chroma-
//! subsampled YCbCr from any non-JPEG compression (to one YCbCr triple per pixel), PixarLog's
//! log tokens (to linear float), and the ThunderScan 4-bit and NeXT 2-bit codings.
//!
//! A unit is decompressed in its stored layout ([`Stored::need`] bytes), and [`expand`]
//! rewrites it into the layout the rest of the pipeline sees (the [`crate::Ifd`] fields).
//! Written from the TIFF 6.0 specification, the TIFF Technical Note on ThunderScan and Greg
//! Ward's LogLuv paper ("The LogLuv Encoding for Full Gamut, High Dynamic Range Images", 1998).

/// How a unit's samples are stored, when that differs from the [`crate::Ifd`] fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stored {
    /// Integers of `bits` bits (not 8, 16 or 32; at most 64): bit-packed MSB first, or in the
    /// file's byte order when a whole number of bytes. Read as 32-bit integers.
    WideInt { bits: u16 },
    /// SGI LogL: one signed 16-bit log-luminance per pixel (compression 34676, photometric
    /// 32844). Read as one 32-bit float (Y, linear).
    LogL,
    /// SGI LogLuv32: 16-bit log-luminance plus 8-bit u' and v' (compression 34676,
    /// photometric 32845). Read as three 32-bit floats (linear RGB).
    LogLuv,
    /// SGI LogLuv24: 10-bit log-luminance and a 14-bit u'v' cell index, three bytes per
    /// pixel, not run-length coded (compression 34677, photometric 32845). Read as three
    /// 32-bit floats (linear RGB).
    LogLuv24,
    /// YCbCr with `h`×`v` chroma subsampling: blocks of `h·v` luma samples then Cb and Cr.
    /// Read as one 8-bit (Y, Cb, Cr) per pixel, the chroma repeated over its block.
    Subsampled { h: u16, v: u16 },
    /// L*a*b* (and its extra samples) of `bits` bits other than 8 and 16, integer (bit-packed
    /// MSB first, or in the file's byte order when whole bytes) or float (`float`; 16, 24, 32
    /// or 64 bits). Read as 16-bit samples in the 16-bit coding of the photometric: integers
    /// scaled to 0–65535 (CIELab's signed a*/b* too, then read as 16-bit two's complement, as
    /// ImageMagick writes them). Floats hold L*, a* and b* themselves and are read as CIELab
    /// (libvips), or with `unit` the 16-bit coding over 0–1 (ImageMagick); float extra samples
    /// run 0–1.
    Lab { bits: u16, float: bool, unit: bool },
    /// PixarLog (compression 32909): after Deflate, one 16-bit token per sample (in the file's
    /// byte order), differenced along each row per sample (per plane when planar); the low
    /// 11 bits index a log / linear table. Read as 32-bit floats (linear light, up to ~25).
    /// The differences restart every `restart` samples of the unit's data, an image row (as
    /// libtiff codes it, also in tiles: a tile is coded as image-width rows).
    PixarLog { restart: u32 },
}

impl Stored {
    /// Bytes holding rows `0..rows` of a unit `w` pixels wide with `spp` samples per pixel in
    /// its plane, and the bytes of one stored row (of one block row when subsampled).
    pub(crate) fn need(self, w: u32, rows: u32, spp: u16) -> (usize, usize) {
        let w = w as usize;
        let row = match self {
            Stored::WideInt { bits } | Stored::Lab { bits, .. } => {
                crate::row_bytes(w as u32, bits, spp) as usize
            }
            Stored::LogL => 2 * w,
            Stored::PixarLog { .. } => 2 * w * spp as usize,
            Stored::LogLuv => 4 * w,
            Stored::LogLuv24 => 3 * w,
            Stored::Subsampled { h, v } => {
                let blocks = w.div_ceil(h as usize);
                let row = blocks * (h as usize * v as usize + 2);
                return (rows.div_ceil(v as u32) as usize * row, row);
            }
        };
        (rows as usize * row, row)
    }
}

/// Rewrite `src` (rows `0..rows` in the stored layout) into `dst` (the same rows in the read
/// layout: `dst.len() / rows` bytes per row). `spp` is the samples per pixel in the plane,
/// `plane` the plane's index (planar data: its sample), `le` the file's byte order
/// (multi-byte values are written in it).
#[allow(clippy::too_many_arguments)]
pub(crate) fn expand(
    stored: Stored,
    src: &[u8],
    w: u32,
    rows: u32,
    spp: u16,
    plane: usize,
    le: bool,
    dst: &mut [u8],
) {
    let w = w as usize;
    let rows = rows as usize;
    if rows == 0 {
        return;
    }
    let drb = dst.len() / rows;
    match stored {
        Stored::WideInt { bits } => {
            let (_, srb) = stored.need(w as u32, 1, spp);
            let n = w * spp as usize;
            for (s, d) in src.chunks(srb).zip(dst.chunks_exact_mut(drb)).take(rows) {
                for (i, o) in d[..n * 4].chunks_exact_mut(4).enumerate() {
                    let v = widen(s, i, bits, le);
                    o.copy_from_slice(&if le { v.to_le_bytes() } else { v.to_be_bytes() });
                }
            }
        }
        Stored::Lab { bits, float, unit } => {
            let (_, srb) = stored.need(w as u32, 1, spp);
            let spp = spp as usize;
            let n = w * spp;
            for (s, d) in src.chunks(srb).zip(dst.chunks_exact_mut(drb)).take(rows) {
                if s.len() < srb {
                    break;
                }
                for (i, o) in d[..n * 2].chunks_exact_mut(2).enumerate() {
                    let c = if spp == 1 { plane } else { i % spp };
                    let v = raw_bits(s, i, bits, le);
                    let v = if unit {
                        (float_bits(v, bits).clamp(0.0, 1.0) * 65535.0).round() as u16
                    } else {
                        lab16(v, bits, float, c)
                    };
                    o.copy_from_slice(&if le { v.to_le_bytes() } else { v.to_be_bytes() });
                }
            }
        }
        Stored::PixarLog { restart } => {
            let lut = pixarlog_table();
            let stride = spp.max(1) as usize;
            let n = w * stride;
            let restart = (restart as usize).max(1);
            let mut acc = vec![0u16; stride];
            let total = (rows * n).min(src.len() / 2);
            for i in 0..total {
                let j = i % restart;
                if j == 0 {
                    // The first pixel of a coded row is absolute.
                    acc.fill(0);
                }
                let p = &src[2 * i..2 * i + 2];
                let t = if le {
                    u16::from_le_bytes([p[0], p[1]])
                } else {
                    u16::from_be_bytes([p[0], p[1]])
                };
                let a = &mut acc[j % stride];
                *a = a.wrapping_add(t);
                let (r, c) = (i / n, i % n);
                if let Some(o) = dst.get_mut(r * drb + 4 * c..r * drb + 4 * c + 4) {
                    o.copy_from_slice(&f32_bytes(lut[(*a & 0x7FF) as usize], le));
                }
            }
        }
        Stored::LogL => {
            for (s, d) in src.chunks(2 * w).zip(dst.chunks_exact_mut(drb)).take(rows) {
                for (p, o) in s.chunks_exact(2).zip(d.chunks_exact_mut(4)) {
                    let y = log_l16(i16::from_be_bytes([p[0], p[1]]));
                    o.copy_from_slice(&f32_bytes(y, le));
                }
            }
        }
        Stored::LogLuv => {
            for (s, d) in src.chunks(4 * w).zip(dst.chunks_exact_mut(drb)).take(rows) {
                for (p, o) in s.chunks_exact(4).zip(d.chunks_exact_mut(12)) {
                    let rgb = log_luv32(u32::from_be_bytes([p[0], p[1], p[2], p[3]]));
                    for (c, v) in o.chunks_exact_mut(4).zip(rgb) {
                        c.copy_from_slice(&f32_bytes(v, le));
                    }
                }
            }
        }
        Stored::LogLuv24 => {
            for (s, d) in src.chunks(3 * w).zip(dst.chunks_exact_mut(drb)).take(rows) {
                for (p, o) in s.chunks_exact(3).zip(d.chunks_exact_mut(12)) {
                    let rgb = log_luv24(u32::from_be_bytes([0, p[0], p[1], p[2]]));
                    for (c, v) in o.chunks_exact_mut(4).zip(rgb) {
                        c.copy_from_slice(&f32_bytes(v, le));
                    }
                }
            }
        }
        Stored::Subsampled { h, v } => {
            let (h, v) = (h as usize, v as usize);
            let block = h * v + 2;
            let (_, brow) = stored.need(w as u32, 1, spp);
            for (by, blocks) in src.chunks(brow).enumerate() {
                for (bx, b) in blocks.chunks_exact(block).enumerate() {
                    let (cb, cr) = (b[h * v], b[h * v + 1]);
                    for j in 0..v {
                        let y = by * v + j;
                        if y >= rows {
                            break;
                        }
                        let d = &mut dst[y * drb..(y + 1) * drb];
                        for i in 0..h {
                            let x = bx * h + i;
                            if x < w {
                                d[3 * x..3 * x + 3].copy_from_slice(&[b[j * h + i], cb, cr]);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[inline]
fn f32_bytes(v: f32, le: bool) -> [u8; 4] {
    if le { v.to_le_bytes() } else { v.to_be_bytes() }
}

/// Sample `i` of a stored row of `bits`-bit integers (more than 16) as a 32-bit integer:
/// shorter values are shifted to the top and their high bits repeated below, so 0 stays 0 and
/// the maximum becomes `u32::MAX`; longer ones keep their top 32 bits. Signed values keep
/// their sign (the repeated bits are below the stored ones and do not change it).
fn widen(row: &[u8], i: usize, bits: u16, le: bool) -> u32 {
    let b = bits as usize;
    let v = raw_bits(row, i, bits, le);
    if b >= 32 {
        (v >> (b - 32)) as u32
    } else {
        let top = (v << (32 - b)) as u32;
        top | (top >> b)
    }
}

/// One L*a*b* sample `v` of `bits` bits (channel `c`: 0 L*, 1–2 a*/b*, more: extra samples)
/// in the 16-bit coding [`Stored::Lab`] describes.
fn lab16(v: u64, bits: u16, float: bool, c: usize) -> u16 {
    let b = bits as u32;
    if float {
        let f = float_bits(v, bits);
        return match c {
            0 => (f.clamp(0.0, 100.0) * 655.35).round() as u16,
            1 | 2 => (f * 256.0).round().clamp(-32768.0, 32767.0) as i16 as u16,
            _ => (f.clamp(0.0, 1.0) * 65535.0).round() as u16,
        };
    }
    // Every integer sample scales to 16 bits, CIELab's signed a*/b* included (then read as the
    // 16-bit two's complement), as ImageMagick writes them.
    let max = if b >= 64 { u64::MAX } else { (1u64 << b) - 1 };
    ((v as u128 * 65535 + max as u128 / 2) / max as u128) as u16
}

/// A float of `bits` bits from its bit pattern (NaN as 0).
fn float_bits(v: u64, bits: u16) -> f64 {
    let f = match bits {
        16 => crate::convert::half_to_f32(v as u16) as f64,
        24 => crate::convert::f24_to_f32(v as u32) as f64,
        32 => f32::from_bits(v as u32) as f64,
        _ => f64::from_bits(v),
    };
    if f.is_nan() { 0.0 } else { f }
}

/// Sample `i` of a stored row of `bits`-bit values (at most 64): bit-packed MSB first, or in
/// the file's byte order when a whole number of bytes.
fn raw_bits(row: &[u8], i: usize, bits: u16, le: bool) -> u64 {
    let b = bits as usize;
    if b.is_multiple_of(8) {
        let n = b / 8;
        let s = &row[i * n..i * n + n];
        if le {
            s.iter().rev().fold(0u64, |a, &x| (a << 8) | x as u64)
        } else {
            s.iter().fold(0u64, |a, &x| (a << 8) | x as u64)
        }
    } else {
        // Bit-packed, MSB first.
        let p = i * b;
        let mut v = 0u64;
        let mut got = 0usize;
        while got < b {
            let byte = row[(p + got) / 8] as u64;
            let off = (p + got) % 8;
            let take = (8 - off).min(b - got);
            v = (v << take) | ((byte >> (8 - off - take)) & ((1 << take) - 1));
            got += take;
        }
        v
    }
}

/// The stored value of a widened sample ([`widen`]), as a number (signed if `signed`).
pub(crate) fn unwiden(v: u32, bits: u16, signed: bool) -> f64 {
    let b = bits as i32;
    if signed {
        let s = v as i32;
        if b >= 32 {
            s as f64 * 2f64.powi(b - 32)
        } else {
            (s >> (32 - b)) as f64
        }
    } else if b >= 32 {
        v as f64 * 2f64.powi(b - 32)
    } else {
        (v >> (32 - b)) as f64
    }
}

/// LogL16: sign and 15 bits of `256·(log2 Y + 64)`; 0 is black.
fn log_l16(p: i16) -> f32 {
    let le = (p as u16 & 0x7fff) as f64;
    if le == 0.0 {
        return 0.0;
    }
    let y = ((le + 0.5) / 256.0 - 64.0).exp2() as f32;
    if p < 0 { -y } else { y }
}

/// u'v' are stored in 1/410 steps.
const UV_SCALE: f64 = 410.0;

/// XYZ (equal-energy white at X = Y = Z) to linear RGB with the ITU-R BT.709 / sRGB
/// primaries, white-balanced to that white: the inverse of the primaries' XYZ matrix with each
/// column scaled so that RGB (1, 1, 1) is X = Y = Z = 1.
const XYZ_E_TO_RGB: [[f64; 3]; 3] = xyz_e_to_rgb();

const fn xyz_e_to_rgb() -> [[f64; 3]; 3] {
    // Chromaticities of the primaries.
    let p = [(0.64, 0.33), (0.30, 0.60), (0.15, 0.06)];
    // Columns: XYZ of each primary at Y = 1.
    let m = [
        [p[0].0 / p[0].1, p[1].0 / p[1].1, p[2].0 / p[2].1],
        [1.0, 1.0, 1.0],
        [
            (1.0 - p[0].0 - p[0].1) / p[0].1,
            (1.0 - p[1].0 - p[1].1) / p[1].1,
            (1.0 - p[2].0 - p[2].1) / p[2].1,
        ],
    ];
    let inv = inv3(m);
    // Scale of each primary so the sum is the white (1, 1, 1).
    let s = [
        inv[0][0] + inv[0][1] + inv[0][2],
        inv[1][0] + inv[1][1] + inv[1][2],
        inv[2][0] + inv[2][1] + inv[2][2],
    ];
    // RGB = diag(1/s) · inv · XYZ.
    [
        [inv[0][0] / s[0], inv[0][1] / s[0], inv[0][2] / s[0]],
        [inv[1][0] / s[1], inv[1][1] / s[1], inv[1][2] / s[1]],
        [inv[2][0] / s[2], inv[2][1] / s[2], inv[2][2] / s[2]],
    ]
}

const fn inv3(m: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let c00 = m[1][1] * m[2][2] - m[1][2] * m[2][1];
    let c01 = m[1][2] * m[2][0] - m[1][0] * m[2][2];
    let c02 = m[1][0] * m[2][1] - m[1][1] * m[2][0];
    let det = m[0][0] * c00 + m[0][1] * c01 + m[0][2] * c02;
    [
        [
            c00 / det,
            (m[0][2] * m[2][1] - m[0][1] * m[2][2]) / det,
            (m[0][1] * m[1][2] - m[0][2] * m[1][1]) / det,
        ],
        [
            c01 / det,
            (m[0][0] * m[2][2] - m[0][2] * m[2][0]) / det,
            (m[0][2] * m[1][0] - m[0][0] * m[1][2]) / det,
        ],
        [
            c02 / det,
            (m[0][1] * m[2][0] - m[0][0] * m[2][1]) / det,
            (m[0][0] * m[1][1] - m[0][1] * m[1][0]) / det,
        ],
    ]
}

/// XYZ of a LogLuv32 value (LogL16 in the top half, then u' and v' bytes).
pub(crate) fn log_luv32_xyz(p: u32) -> [f64; 3] {
    let y = log_l16((p >> 16) as u16 as i16) as f64;
    if y <= 0.0 {
        return [0.0; 3];
    }
    let u = ((p >> 8 & 0xff) as f64 + 0.5) / UV_SCALE;
    let v = ((p & 0xff) as f64 + 0.5) / UV_SCALE;
    uv_y_to_xyz(u, v, y)
}

/// XYZ of luminance `y` at CIE 1976 chromaticity (u', v').
fn uv_y_to_xyz(u: f64, v: f64, y: f64) -> [f64; 3] {
    let d = 6.0 * u - 16.0 * v + 12.0;
    let (x, yy) = (9.0 * u / d, 4.0 * v / d);
    [x / yy * y, y, (1.0 - x - yy) / yy * y]
}

fn xyz_to_rgb(xyz: [f64; 3]) -> [f32; 3] {
    XYZ_E_TO_RGB.map(|m| (m[0] * xyz[0] + m[1] * xyz[1] + m[2] * xyz[2]) as f32)
}

fn log_luv32(p: u32) -> [f32; 3] {
    xyz_to_rgb(log_luv32_xyz(p))
}

/// The centre (u', v') of LogLuv24 cell `c`: the row of v' whose cells hold it, then the
/// cell along u'. `None` outside the 16289 cells.
pub(crate) fn uv24(c: u32) -> Option<(f64, f64)> {
    use crate::logluv_uv::{UV_NDIVS, UV_ROW, UV_SQSIZ, UV_VSTART};
    if c >= UV_NDIVS {
        return None;
    }
    // The last row starting at or before `c`.
    let vi = UV_ROW.partition_point(|r| u32::from(r.2) <= c) - 1;
    let (ustart, _, ncum) = UV_ROW[vi];
    let ui = c - u32::from(ncum);
    Some((
        f64::from(ustart) + (f64::from(ui) + 0.5) * UV_SQSIZ,
        UV_VSTART + (vi as f64 + 0.5) * UV_SQSIZ,
    ))
}

/// XYZ of a LogLuv24 value (10-bit `64·(log2 Y + 12)` above a 14-bit chroma cell); an
/// unused cell reads as the equal-energy white.
pub(crate) fn log_luv24_xyz(p: u32) -> [f64; 3] {
    let le = (p >> 14) & 0x3ff;
    if le == 0 {
        return [0.0; 3];
    }
    let y = ((f64::from(le) + 0.5) / 64.0 - 12.0).exp2();
    let (u, v) = uv24(p & 0x3fff).unwrap_or((4.0 / 19.0, 9.0 / 19.0));
    uv_y_to_xyz(u, v, y)
}

fn log_luv24(p: u32) -> [f32; 3] {
    xyz_to_rgb(log_luv24_xyz(p))
}

/// SGI LogL / LogLuv32 run-length coding (compression 34676): each row holds its values as
/// `bytes` byte planes, most significant first; in a plane a control byte `n ≥ 128` repeats
/// the next byte `n − 126` times, and `n < 128` is followed by `n` literal bytes. The values
/// are written big-endian into `out` (`row` bytes per row). Returns the bytes written.
pub(crate) fn sgilog(src: &[u8], out: &mut [u8], row: usize, bytes: usize) -> usize {
    let w = row / bytes;
    if w == 0 {
        return 0;
    }
    let mut i = 0usize;
    let mut done = 0usize;
    for r in out.chunks_exact_mut(row) {
        for plane in 0..bytes {
            let mut x = 0usize;
            while x < w {
                let Some(&n) = src.get(i) else {
                    return done;
                };
                i += 1;
                if n >= 128 {
                    let Some(&b) = src.get(i) else {
                        return done;
                    };
                    i += 1;
                    for _ in 0..(n as usize - 126).min(w - x) {
                        r[x * bytes + plane] = b;
                        x += 1;
                    }
                } else {
                    let n = n as usize;
                    let Some(lit) = src.get(i..i + n) else {
                        return done;
                    };
                    i += n;
                    for &b in lit.iter().take(w - x) {
                        r[x * bytes + plane] = b;
                        x += 1;
                    }
                }
            }
        }
        done += row;
    }
    done
}

/// PixarLog's token table (11-bit tokens to linear light): a linear segment from 0 in
/// steps of `b·c·e`, then a constant ratio `b·e^(c·i)` (`c` = 1/250, about ln 1.004, and
/// `b` = e^(−1250c), so token 1250 is exactly 1.0); the two meet smoothly. Values in `f64`,
/// stored as `f32`.
fn pixarlog_table() -> &'static [f32; 2048] {
    static T: std::sync::OnceLock<[f32; 2048]> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let ratio = 1.004f64;
        let nlin = (1.0 / ratio.ln()) as usize;
        let c = 1.0 / nlin as f64;
        let b = (-c * 1250.0).exp();
        let linstep = b * c * std::f64::consts::E;
        std::array::from_fn(|i| {
            if i < nlin {
                (i as f64 * linstep) as f32
            } else {
                (b * (c * i as f64).exp()) as f32
            }
        })
    })
}

/// NeXT 2-bit coding (compression 32766): every row starts white (all bits set) and opens
/// with a byte: 0x00, the packed row follows; 0x40, a span (big-endian byte offset and byte
/// count, then the bytes) is copied into the white row; otherwise the row is runs, each byte
/// a 2-bit grey (high bits) and a 6-bit count of pixels, until `w` pixels are set. Rows go
/// into `out`, `row` bytes each. Returns the bytes written.
pub(crate) fn next(src: &[u8], out: &mut [u8], row: usize, w: usize) -> usize {
    let mut i = 0usize;
    let mut done = 0usize;
    if row == 0 {
        return 0;
    }
    for r in out.chunks_exact_mut(row) {
        r.fill(0xFF);
        let Some(&op) = src.get(i) else {
            return done;
        };
        i += 1;
        match op {
            0x00 => {
                let Some(s) = src.get(i..i + row) else {
                    return done;
                };
                r.copy_from_slice(s);
                i += row;
            }
            0x40 => {
                let Some(h) = src.get(i..i + 4) else {
                    return done;
                };
                let off = u16::from_be_bytes([h[0], h[1]]) as usize;
                let n = u16::from_be_bytes([h[2], h[3]]) as usize;
                let (Some(s), true) = (src.get(i + 4..i + 4 + n), off + n <= row) else {
                    return done;
                };
                r[off..off + n].copy_from_slice(s);
                i += 4 + n;
            }
            mut b => {
                let mut x = 0usize;
                loop {
                    let grey = b >> 6;
                    for _ in 0..(b & 0x3F) {
                        if x >= w {
                            break;
                        }
                        let shift = 6 - 2 * (x % 4);
                        r[x / 4] = (r[x / 4] & !(3 << shift)) | grey << shift;
                        x += 1;
                    }
                    if x >= w {
                        break;
                    }
                    let Some(&nb) = src.get(i) else {
                        return done;
                    };
                    i += 1;
                    b = nb;
                }
            }
        }
        done += row;
    }
    done
}

/// ThunderScan 4-bit coding (compression 32809): per row, from a last value of 0, each byte is
/// a 2-bit opcode and 6 bits of data: a run of the last value (`00`), three 2-bit deltas
/// (`01`: 0, +1, skip, −1), two 3-bit deltas (`10`: 0..3, skip, −3..−1) or a raw value (`11`).
/// Pixels go into `out` as packed 4-bit samples, `row` bytes for `w` pixels per row. Returns
/// the bytes written.
pub(crate) fn thunderscan(src: &[u8], out: &mut [u8], row: usize, w: usize) -> usize {
    const D2: [i8; 4] = [0, 1, 0, -1];
    const D3: [i8; 8] = [0, 1, 2, 3, 0, -3, -2, -1];
    let mut i = 0usize;
    let mut done = 0usize;
    if row == 0 {
        return 0;
    }
    for r in out.chunks_exact_mut(row) {
        r.fill(0);
        let mut last = 0u8;
        let mut x = 0usize;
        let put = |r: &mut [u8], x: &mut usize, v: u8| {
            if *x < w {
                r[*x / 2] |= if (*x).is_multiple_of(2) { v << 4 } else { v };
                *x += 1;
            }
        };
        while x < w {
            let Some(&n) = src.get(i) else {
                return done;
            };
            i += 1;
            match n >> 6 {
                0 => {
                    for _ in 0..(n & 0x3f) {
                        put(r, &mut x, last);
                    }
                }
                1 => {
                    for code in [(n >> 4) & 3, (n >> 2) & 3, n & 3] {
                        if code != 2 {
                            last = (last as i8 + D2[code as usize]) as u8 & 0xf;
                            put(r, &mut x, last);
                        }
                    }
                }
                2 => {
                    for code in [(n >> 3) & 7, n & 7] {
                        if code != 4 {
                            last = (last as i8 + D3[code as usize]) as u8 & 0xf;
                            put(r, &mut x, last);
                        }
                    }
                }
                _ => {
                    last = n & 0xf;
                    put(r, &mut x, last);
                }
            }
        }
        done += row;
    }
    done
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logluv24_cells() {
        use crate::logluv_uv::UV_ROW;
        // The rows tile the cells without gaps, and each cell lies on its own row.
        let mut c = 0u32;
        for (vi, &(_, n, ncum)) in UV_ROW.iter().enumerate() {
            assert_eq!(u32::from(ncum), c);
            for ui in 0..u32::from(n) {
                let (u, v) = uv24(c + ui).unwrap();
                assert!((0.0..0.65).contains(&u) && (0.0..0.65).contains(&v));
                let row = ((v - crate::logluv_uv::UV_VSTART) / 0.0035) as usize;
                assert_eq!(row, vi);
            }
            c += u32::from(n);
        }
        assert_eq!(uv24(c), None);
        // The first cell: u' = 0.247663 + 0.00175, v' = 0.01694 + 0.00175.
        let (u, v) = uv24(0).unwrap();
        assert!((u - 0.249413).abs() < 1e-6 && (v - 0.01869).abs() < 1e-9);
        // Le 768 is Y = 2^(768.5/64 − 12); an unused cell reads as the white point.
        let xyz = log_luv24_xyz(768 << 14 | 0x3fff);
        let y = (768.5f64 / 64.0 - 12.0).exp2();
        assert!((xyz[1] - y).abs() < 1e-12 && (xyz[0] - y).abs() < 1e-9);
        assert!((xyz[2] - y).abs() < 1e-9);
        assert_eq!(log_luv24_xyz(5), [0.0; 3]);
    }

    #[test]
    fn widen_keeps_range_and_sign() {
        // 20-bit MSB-packed: 0xFFFFF and 0x00000.
        let row = [0xFF, 0xFF, 0xF0, 0x00, 0x00];
        assert_eq!(widen(&row, 0, 20, false), u32::MAX);
        assert_eq!(widen(&row, 1, 20, false), 0);
        // 24-bit little-endian: 0x800000 (signed minimum).
        let row = [0x00, 0x00, 0x80];
        let v = widen(&row, 0, 24, true);
        assert_eq!(unwiden(v, 24, true), -8388608.0);
        assert_eq!(unwiden(v, 24, false), 8388608.0);
        // 17-bit: 1.
        let row = [0x00, 0x00, 0x80];
        assert_eq!(unwiden(widen(&row, 0, 17, false), 17, false), 1.0);
        // 64-bit big-endian keeps the top 32 bits.
        let row = 0x1234_5678_9abc_def0u64.to_be_bytes();
        assert_eq!(widen(&row, 0, 64, false), 0x1234_5678);
    }

    #[test]
    fn log_luv_white_is_grey() {
        // Equal-energy white: u' = 4/19, v' = 9/19; Y = 1 is Le = 64·256.
        let u = (4.0 / 19.0 * UV_SCALE - 0.5).round() as u32;
        let v = (9.0 / 19.0 * UV_SCALE - 0.5).round() as u32;
        let p = (64 * 256) << 16 | u << 8 | v;
        let rgb = log_luv32(p);
        for c in rgb {
            assert!((c - 1.0).abs() < 0.01, "{rgb:?}");
        }
        assert_eq!(log_l16(0), 0.0);
        assert!((log_l16(64 * 256) - 1.0).abs() < 0.003);
    }

    #[test]
    fn sgilog_runs_and_literals() {
        // Two pixels, two planes: plane 0 a run of 2 × 0x12, plane 1 literals 0x34 0x56.
        let src = [128, 0x12, 2, 0x34, 0x56];
        let mut out = [0u8; 4];
        assert_eq!(sgilog(&src, &mut out, 4, 2), 4);
        assert_eq!(out, [0x12, 0x34, 0x12, 0x56]);
        // Truncated input stops.
        assert_eq!(sgilog(&src[..3], &mut out, 4, 2), 0);
    }

    #[test]
    fn thunderscan_ops() {
        // Raw 5, two 2-bit deltas (+1, skip, −1), a run of 2, a 3-bit delta pair (+3, −2).
        let src = [0xC5, 0b01_01_10_11, 0x02, 0b10_011_110];
        let mut out = [0u8; 4];
        assert_eq!(thunderscan(&src, &mut out, 4, 7), 4);
        // 5, 6, 5, 5, 5, 8, 6
        assert_eq!(out, [0x56, 0x55, 0x58, 0x60]);
    }

    #[test]
    fn subsampled_blocks_spread() {
        // 3×2 pixels, 2×2 blocks: two blocks (the second half outside).
        let src = [1, 2, 3, 4, 100, 200, 5, 6, 7, 8, 110, 210];
        let mut dst = [0u8; 18];
        expand(
            Stored::Subsampled { h: 2, v: 2 },
            &src,
            3,
            2,
            3,
            0,
            true,
            &mut dst,
        );
        assert_eq!(
            dst,
            [
                1, 100, 200, 2, 100, 200, 5, 110, 210, 3, 100, 200, 4, 100, 200, 7, 110, 210
            ]
        );
    }
}
