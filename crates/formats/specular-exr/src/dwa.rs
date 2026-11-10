//! DWAA / DWAB (OpenEXR `ImfDwaCompressor`), written from the format description.
//!
//! A chunk is an 88-byte header of eleven little-endian `u64` counters, (version 2) a table of
//! channel rules, then four sections:
//!
//! * **unknown**: zlib of the raw samples of channels no rule claims, channel after channel;
//! * **AC**: the quantised AC coefficients of the lossy channels, static Huffman (PIZ's coder)
//!   or zlib;
//! * **DC**: zlib + ZIP's predictor/interleave of the DC coefficients;
//! * **RLE**: zlib, then OpenEXR RLE, of the RLE channels' byte planes.
//!
//! Lossy channels are 8x8 DCT blocks of perceptually encoded halves; complete R/G/B sets were
//! converted to Y'CbCr before the transform.

use std::sync::OnceLock;

use crate::codec::{BlockGeom, inflate, rle, unpredict_interleave};
use crate::half::{f2h, h2f};
use crate::header::{Channel, PixelType};
use crate::piz::{self, PizScratch};

const HEADER: usize = 11 * 8;
const STATIC_HUFFMAN: u64 = 0;
const DEFLATE: u64 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scheme {
    Unknown,
    Lossy,
    Rle,
}

#[derive(Clone, Copy, Debug)]
struct Rule<'a> {
    suffix: &'a [u8],
    /// Component of a colour set (0..3 for R, G, B), or -1.
    csc: i8,
    scheme: Scheme,
    case_insensitive: bool,
    ty: PixelType,
}

const fn legacy(s: &'static str, csc: i8, scheme: Scheme, ty: PixelType) -> Rule<'static> {
    Rule {
        suffix: s.as_bytes(),
        csc,
        scheme,
        case_insensitive: true,
        ty,
    }
}

/// Rules of version-1 chunks (which carry no table).
const LEGACY_RULES: [Rule<'static>; 14] = {
    use PixelType::{Float, Half, Uint};
    use Scheme::{Lossy, Rle};
    [
        legacy("r", 0, Lossy, Half),
        legacy("red", 0, Lossy, Half),
        legacy("g", 1, Lossy, Half),
        legacy("grn", 1, Lossy, Half),
        legacy("green", 1, Lossy, Half),
        legacy("b", 2, Lossy, Half),
        legacy("blu", 2, Lossy, Half),
        legacy("blue", 2, Lossy, Half),
        legacy("y", -1, Lossy, Half),
        legacy("by", -1, Lossy, Half),
        legacy("ry", -1, Lossy, Half),
        legacy("a", -1, Rle, Uint),
        legacy("a", -1, Rle, Half),
        legacy("a", -1, Rle, Float),
    ]
};

impl Rule<'_> {
    fn matches(&self, suffix: &[u8], ty: PixelType) -> bool {
        self.ty == ty
            && if self.case_insensitive {
                self.suffix.eq_ignore_ascii_case(suffix)
            } else {
                self.suffix == suffix
            }
    }
}

/// Parse a version-2 rule table (`d` starts after the table's `u16` size).
fn parse_rules<'a>(mut d: &'a [u8], rules: &mut Vec<Rule<'a>>) -> Option<()> {
    rules.clear();
    while !d.is_empty() {
        let nul = d.iter().position(|&b| b == 0)?;
        let (suffix, rest) = d.split_at(nul);
        let &[_, flags, ty, ..] = rest else {
            return None;
        };
        let scheme = match (flags >> 2) & 3 {
            0 => Scheme::Unknown,
            1 => Scheme::Lossy,
            2 => Scheme::Rle,
            _ => return None,
        };
        let csc = (flags >> 4) as i8 - 1;
        if csc > 2 {
            return None;
        }
        let ty = match ty {
            0 => PixelType::Uint,
            1 => PixelType::Half,
            2 => PixelType::Float,
            _ => return None,
        };
        rules.push(Rule {
            suffix,
            csc,
            scheme,
            case_insensitive: flags & 1 != 0,
            ty,
        });
        d = &rest[3..];
    }
    Some(())
}

/// Per-thread buffers.
#[derive(Default)]
pub(crate) struct DwaScratch {
    scheme: Vec<Scheme>,
    /// Complete colour sets: channel indices of R, G, B.
    sets: Vec<[usize; 3]>,
    in_set: Vec<bool>,
    unknown: Vec<u8>,
    ac: Vec<u16>,
    dc_bytes: Vec<u8>,
    dc: Vec<u8>,
    rle_packed: Vec<u8>,
    rle: Vec<u8>,
}

/// Decompress one DWAA/DWAB chunk into `out` (planar layout, `g.total` bytes).
pub(crate) fn decompress(
    input: &[u8],
    g: &BlockGeom,
    chans: &[Channel],
    s: &mut DwaScratch,
    piz: &mut PizScratch,
    out: &mut [u8],
) -> Option<()> {
    if chans.len() != g.chans.len() {
        return None;
    }
    let head = input.get(..HEADER)?;
    let c: [u64; 11] =
        std::array::from_fn(|i| u64::from_le_bytes(head[i * 8..i * 8 + 8].try_into().unwrap()));
    let [
        version,
        unknown_unc,
        unknown_size,
        ac_size,
        dc_size,
        rle_size,
        rle_unc,
        rle_raw,
        ac_count,
        dc_count,
        ac_comp,
    ] = c;
    let mut pos = HEADER;
    let mut table = Vec::new();
    let rules: &[Rule] = match version {
        0..=1 => &LEGACY_RULES,
        2 => {
            let n = u16::from_le_bytes(input.get(pos..pos + 2)?.try_into().ok()?) as usize;
            let body = input.get(pos + 2..pos.checked_add(n)?)?;
            pos += n.max(2);
            parse_rules(body, &mut table)?;
            &table
        }
        _ => return None,
    };

    // Classify channels; the last matching rule wins.
    s.scheme.clear();
    s.sets.clear();
    s.in_set.clear();
    s.in_set.resize(chans.len(), false);
    let mut partial: Vec<(&str, [Option<usize>; 3])> = Vec::new();
    for (i, ch) in chans.iter().enumerate() {
        let (prefix, suffix) = match ch.name.rfind('.') {
            Some(d) => (&ch.name[..=d], &ch.name[d + 1..]),
            None => ("", &ch.name[..]),
        };
        let mut scheme = Scheme::Unknown;
        let mut csc = -1;
        for r in rules {
            if r.matches(suffix.as_bytes(), ch.pixel_type) {
                scheme = r.scheme;
                csc = r.csc;
            }
        }
        if scheme == Scheme::Lossy && csc >= 0 {
            let k = match partial.iter().position(|(p, _)| *p == prefix) {
                Some(k) => k,
                None => {
                    partial.push((prefix, [None; 3]));
                    partial.len() - 1
                }
            };
            partial[k].1[csc as usize] = Some(i);
        }
        s.scheme.push(scheme);
    }
    for (_, set) in &partial {
        if let [Some(r), Some(g), Some(b)] = *set {
            s.sets.push([r, g, b]);
            for i in [r, g, b] {
                s.in_set[i] = true;
            }
        }
    }

    // Expected sizes.
    let mut unknown_bytes = 0usize;
    let mut rle_bytes = 0usize;
    let mut blocks = 0usize;
    for (ch, &sc) in g.chans.iter().zip(&s.scheme) {
        let n = ch.nx * ch.ny;
        match sc {
            Scheme::Unknown => unknown_bytes += n * ch.bytes,
            Scheme::Rle => rle_bytes += n * ch.bytes,
            Scheme::Lossy => blocks += ch.nx.div_ceil(8) * ch.ny.div_ceil(8),
        }
    }
    if unknown_unc != unknown_bytes as u64 || rle_raw != rle_bytes as u64 {
        return None;
    }
    // At most 63 AC symbols per 8x8 block (the u16 AC buffer is sized from the header).
    if dc_count < blocks as u64 || dc_count > g.total as u64 || ac_count > 63 * blocks as u64 {
        return None;
    }
    if rle_unc > 2 * rle_raw + 64 {
        return None;
    }
    let (dc_count, ac_count) = (dc_count as usize, ac_count as usize);

    let mut take = |n: u64| -> Option<&[u8]> {
        let n = usize::try_from(n).ok()?;
        let r = input.get(pos..pos.checked_add(n)?)?;
        pos += n;
        Some(r)
    };
    let unknown_in = take(unknown_size)?;
    let ac_in = take(ac_size)?;
    let dc_in = take(dc_size)?;
    let rle_in = take(rle_size)?;

    // Unknown: raw lines.
    s.unknown.clear();
    s.unknown.resize(unknown_bytes, 0);
    if unknown_bytes > 0 {
        inflate(unknown_in, &mut s.unknown)?;
    }
    // AC.
    s.ac.clear();
    s.ac.resize(ac_count, 0);
    if ac_count > 0 {
        match ac_comp {
            STATIC_HUFFMAN => piz::huf_decode(ac_in, &mut s.ac, piz)?,
            DEFLATE => {
                s.dc_bytes.clear();
                s.dc_bytes.resize(ac_count * 2, 0);
                inflate(ac_in, &mut s.dc_bytes)?;
                for (a, b) in s.ac.iter_mut().zip(s.dc_bytes.chunks_exact(2)) {
                    *a = u16::from_le_bytes([b[0], b[1]]);
                }
            }
            _ => return None,
        }
    }
    // DC: zlib + ZIP predictor / interleave.
    s.dc.clear();
    s.dc.resize(dc_count * 2, 0);
    if dc_count > 0 {
        s.dc_bytes.clear();
        s.dc_bytes.resize(dc_count * 2, 0);
        inflate(dc_in, &mut s.dc_bytes)?;
        unpredict_interleave(&mut s.dc_bytes, &mut s.dc);
    }
    // RLE.
    s.rle.clear();
    s.rle.resize(rle_bytes, 0);
    if rle_bytes > 0 {
        s.rle_packed.clear();
        s.rle_packed.resize(rle_unc as usize, 0);
        inflate(rle_in, &mut s.rle_packed)?;
        rle(&s.rle_packed, &mut s.rle)?;
    }

    // Planar output: start of each channel's plane.
    let mut starts = Vec::with_capacity(g.chans.len());
    let mut acc = 0usize;
    for ch in &g.chans {
        starts.push(acc);
        acc += ch.nx * ch.ny * ch.bytes;
    }
    if acc != out.len() {
        return None;
    }

    // Lossy: colour sets first, then the other lossy channels in channel order.
    let mut ac_pos = 0usize;
    let mut dc_pos = 0usize;
    let lut = to_linear();
    let k = Consts::new();
    for set in &s.sets {
        let ch: [Comp; 3] = set.map(|i| Comp {
            nx: g.chans[i].nx,
            ny: g.chans[i].ny,
            ty: g.chans[i].ty,
            start: starts[i],
            lut: Some(lut),
        });
        if ch[1..].iter().any(|c| c.nx != ch[0].nx || c.ny != ch[0].ny) {
            return None;
        }
        decode_lossy(&ch, &s.ac, &mut ac_pos, &s.dc, &mut dc_pos, &k, out)?;
    }
    for (i, gc) in g.chans.iter().enumerate() {
        if s.scheme[i] != Scheme::Lossy || s.in_set[i] {
            continue;
        }
        let ch = [Comp {
            nx: gc.nx,
            ny: gc.ny,
            ty: gc.ty,
            start: starts[i],
            lut: (!gc.p_linear).then_some(lut),
        }];
        decode_lossy(&ch, &s.ac, &mut ac_pos, &s.dc, &mut dc_pos, &k, out)?;
    }

    // RLE channels: per channel, one plane per byte of the sample.
    let mut r = 0usize;
    for (i, gc) in g.chans.iter().enumerate() {
        if s.scheme[i] != Scheme::Rle {
            continue;
        }
        let n = gc.nx * gc.ny;
        let dst = &mut out[starts[i]..starts[i] + n * gc.bytes];
        for b in 0..gc.bytes {
            let plane = &s.rle[r..r + n];
            r += n;
            for (d, &v) in dst.chunks_exact_mut(gc.bytes).zip(plane) {
                d[b] = v;
            }
        }
    }

    // Unknown channels: planar, as the output.
    let mut u = 0usize;
    for (i, gc) in g.chans.iter().enumerate() {
        if s.scheme[i] != Scheme::Unknown {
            continue;
        }
        let n = gc.nx * gc.ny * gc.bytes;
        out[starts[i]..starts[i] + n].copy_from_slice(&s.unknown[u..u + n]);
        u += n;
    }
    Some(())
}

/// One component of a lossy decode.
#[derive(Clone, Copy)]
struct Comp<'a> {
    nx: usize,
    ny: usize,
    ty: PixelType,
    /// Byte offset of the channel's plane in the output.
    start: usize,
    /// Perceptual-to-linear table (absent for `pLinear` channels).
    lut: Option<&'a [u16; 65536]>,
}

/// Natural (row-major) index of each zig-zag position.
const ZIGZAG: [u8; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// Inverse DCT constants, `0.5 cos(k pi / 16)` evaluated in single precision as OpenEXR does
/// (with `pi` = 3.14159).
struct Consts {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    e: f32,
    f: f32,
    g: f32,
}

impl Consts {
    fn new() -> Self {
        // The truncated value is the format's, not an approximation of ours.
        #[allow(clippy::approx_constant)]
        let pi = 3.14159f32;
        let k = |n: f32, d: f32| 0.5 * (n * pi / d).cos();
        Consts {
            a: k(1.0, 4.0),
            b: k(1.0, 16.0),
            c: k(1.0, 8.0),
            d: k(3.0, 16.0),
            e: k(5.0, 16.0),
            f: k(3.0, 8.0),
            g: k(7.0, 16.0),
        }
    }

    /// One 8-point inverse DCT of `v[0], v[s], .. v[7s]`, in place.
    #[inline(always)]
    fn idct8(&self, v: &mut [f32; 64], o: usize, s: usize) {
        let x: [f32; 8] = std::array::from_fn(|i| v[o + i * s]);
        let alpha = [self.c * x[2], self.f * x[2], self.c * x[6], self.f * x[6]];
        let beta = [
            self.b * x[1] + self.d * x[3] + self.e * x[5] + self.g * x[7],
            self.d * x[1] - self.g * x[3] - self.b * x[5] - self.e * x[7],
            self.e * x[1] - self.b * x[3] + self.g * x[5] + self.d * x[7],
            self.g * x[1] - self.e * x[3] + self.d * x[5] - self.b * x[7],
        ];
        let theta = [
            self.a * (x[0] + x[4]),
            alpha[0] + alpha[3],
            alpha[1] - alpha[2],
            self.a * (x[0] - x[4]),
        ];
        let gamma = [
            theta[0] + theta[1],
            theta[3] + theta[2],
            theta[3] - theta[2],
            theta[0] - theta[1],
        ];
        v[o] = gamma[0] + beta[0];
        v[o + s] = gamma[1] + beta[1];
        v[o + 2 * s] = gamma[2] + beta[2];
        v[o + 3 * s] = gamma[3] + beta[3];
        v[o + 4 * s] = gamma[3] - beta[3];
        v[o + 5 * s] = gamma[2] - beta[2];
        v[o + 6 * s] = gamma[1] - beta[1];
        v[o + 7 * s] = gamma[0] - beta[0];
    }

    /// 2-D inverse DCT: rows, then columns.
    fn idct(&self, v: &mut [f32; 64]) {
        for r in 0..8 {
            self.idct8(v, r * 8, 1);
        }
        for c in 0..8 {
            self.idct8(v, c, 8);
        }
    }
}

/// Decode the 8x8 blocks of one channel (or one Y'CbCr set of three) into `out`.
fn decode_lossy(
    ch: &[Comp],
    ac: &[u16],
    ac_pos: &mut usize,
    dc: &[u8],
    dc_pos: &mut usize,
    k: &Consts,
    out: &mut [u8],
) -> Option<()> {
    let (nx, ny) = (ch[0].nx, ch[0].ny);
    let (bx_n, by_n) = (nx.div_ceil(8), ny.div_ceil(8));
    let nb = bx_n * by_n;
    let n = ch.len();
    if dc.len() < (*dc_pos + n * nb) * 2 {
        return None;
    }
    let dc_at = |i: usize| u16::from_le_bytes([dc[i * 2], dc[i * 2 + 1]]);
    let mut blk = [[0f32; 64]; 3];
    let mut ap = *ac_pos;
    for by in 0..by_n {
        for bx in 0..bx_n {
            let bi = by * bx_n + bx;
            for (comp, v) in blk.iter_mut().enumerate().take(n) {
                let mut zig = [0u16; 64];
                zig[0] = dc_at(*dc_pos + comp * nb + bi);
                let mut i = 1usize;
                let mut last = 0usize;
                while i < 64 {
                    let a = *ac.get(ap)?;
                    ap += 1;
                    if a == 0xff00 {
                        break;
                    } else if a >> 8 == 0xff {
                        i += (a & 0xff) as usize;
                    } else {
                        zig[i] = a;
                        last = i;
                        i += 1;
                    }
                }
                if last == 0 {
                    let d = h2f(zig[0]) * 3.535536e-01 * 3.535536e-01;
                    v.fill(d);
                } else {
                    for (z, &h) in ZIGZAG.iter().zip(&zig) {
                        v[*z as usize] = h2f(h);
                    }
                    k.idct(v);
                }
            }
            if n == 3 {
                let [y, cb, cr] = &mut blk;
                for i in 0..64 {
                    let (l, b, r) = (y[i], cb[i], cr[i]);
                    y[i] = l + 1.5747 * r;
                    cb[i] = l - 0.1873 * b - 0.4682 * r;
                    cr[i] = l + 1.8556 * b;
                }
            }
            let rows = 8.min(ny - by * 8);
            let cols = 8.min(nx - bx * 8);
            for (c, v) in ch.iter().zip(&blk) {
                for r in 0..rows {
                    let y = by * 8 + r;
                    for x in 0..cols {
                        let mut h = f2h(v[r * 8 + x]);
                        if let Some(t) = c.lut {
                            h = t[h as usize];
                        }
                        let p = y * nx + bx * 8 + x;
                        match c.ty {
                            PixelType::Half => {
                                let o = c.start + p * 2;
                                out[o..o + 2].copy_from_slice(&h.to_le_bytes());
                            }
                            _ => {
                                let o = c.start + p * 4;
                                out[o..o + 4].copy_from_slice(&h2f(h).to_le_bytes());
                            }
                        }
                    }
                }
            }
        }
    }
    *ac_pos = ap;
    *dc_pos += n * nb;
    Some(())
}

/// OpenEXR's `dwaCompressorToLinear`: perceptual half to linear half (`|x|^2.2` up to one,
/// `e^(2.2 (|x|-1))` above, sign kept; infinities and NaNs to zero).
fn to_linear() -> &'static [u16; 65536] {
    static T: OnceLock<Box<[u16; 65536]>> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = Box::new([0u16; 65536]);
        for (i, v) in t.iter_mut().enumerate() {
            let f = h2f(i as u16);
            if !f.is_finite() {
                continue;
            }
            let a = f.abs();
            let l = if a <= 1.0 {
                a.powf(2.2)
            } else {
                (2.2 * (a - 1.0)).exp()
            };
            *v = f2h(l.copysign(f));
        }
        t
    })
}
