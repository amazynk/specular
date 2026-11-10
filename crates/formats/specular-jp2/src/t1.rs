//! EBCOT tier-1: code-block bit-plane decoding (Annex D).

use crate::markers::{CBS_BYPASS, CBS_CAUSAL, CBS_RESET, CBS_SEGSYM};
use crate::mq::{CTX_RL, CTX_UNI, Mq, Raw};

// Flags: one word per stripe column (4 rows of one column), as in OpenJPEG.
// Bits 0..17: significance of a 3 x 6 window: columns W, centre, E (c = 0..2) x rows
// "above the stripe", the 4 stripe rows, "below the stripe" (r = 0..5), at bit 3r + c.
// Bit 18: sign of the sample above the stripe; bit 31: sign of the sample below.
// Stripe row j (0..3): sign at 19 + 3j, refined at 20 + 3j, visited at 21 + 3j.
// The 3 x 3 neighbourhood of row j is `(word >> 3j) & NBHD`.
const NBHD: u32 = 0x1EF;
const CHI_ABOVE: u32 = 1 << 18;
const CHI_BELOW: u32 = 1 << 31;
/// Bits describing the row below the stripe (ignored in vertically causal mode).
const BELOW: u32 = 0x38000 | CHI_BELOW;
const SIG_ROWS: u32 = (1 << 4) | (1 << 7) | (1 << 10) | (1 << 13);
const PI_ALL: u32 = (1 << 21) | (1 << 24) | (1 << 27) | (1 << 30);
/// Sign bit of a decoded magnitude (see [`T1::row`]).
pub(crate) const NEG: u32 = 1 << 31;

/// Highest supported number of magnitude bit-planes (values are kept with one extra
/// half-bit of precision in a u32).
pub(crate) const MAX_BITPLANES: u8 = 30;

/// Zero-coding context LUT indexed by the 9-bit neighbourhood (bit 0 NW, 1 N, 2 NE, 3 W,
/// 4 the sample itself (ignored), 5 E, 6 SW, 7 S, 8 SE). `class`: 0 LL/LH, 1 HL, 2 HH.
const fn zc_lut(class: usize) -> [u8; 512] {
    let mut t = [0u8; 512];
    let mut f = 0;
    while f < 512 {
        let b = f as u32;
        let mut h = ((b >> 3) & 1) + ((b >> 5) & 1);
        let mut v = ((b >> 1) & 1) + ((b >> 7) & 1);
        let d = (b & 1) + ((b >> 2) & 1) + ((b >> 6) & 1) + ((b >> 8) & 1);
        if class == 1 {
            let t2 = h;
            h = v;
            v = t2;
        }
        let ctx = if class < 2 {
            if h == 2 {
                8
            } else if h == 1 {
                if v >= 1 {
                    7
                } else if d >= 1 {
                    6
                } else {
                    5
                }
            } else if v == 2 {
                4
            } else if v == 1 {
                3
            } else if d >= 2 {
                2
            } else if d == 1 {
                1
            } else {
                0
            }
        } else {
            let hv = h + v;
            if d >= 3 {
                8
            } else if d == 2 {
                if hv >= 1 { 7 } else { 6 }
            } else if d == 1 {
                if hv >= 2 {
                    5
                } else if hv == 1 {
                    4
                } else {
                    3
                }
            } else if hv >= 2 {
                2
            } else if hv == 1 {
                1
            } else {
                0
            }
        };
        t[f] = ctx;
        f += 1;
    }
    t
}

static ZC: [[u8; 512]; 3] = [zc_lut(0), zc_lut(1), zc_lut(2)];

const fn sc_contrib(sig: u32, neg: u32, bit: u32) -> i32 {
    if sig & bit == 0 {
        0
    } else if neg & bit != 0 {
        -1
    } else {
        1
    }
}

/// Sign context LUT indexed by significance N=1, S=2, W=4, E=8 in the low nibble and the
/// matching signs in the high nibble; value = ctx | xor << 7.
const fn sc_lut() -> [u8; 256] {
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        let sig = i as u32 & 0xF;
        let neg = (i as u32 >> 4) & 0xF;
        let mut h = sc_contrib(sig, neg, 4) + sc_contrib(sig, neg, 8);
        let mut v = sc_contrib(sig, neg, 1) + sc_contrib(sig, neg, 2);
        h = if h > 1 {
            1
        } else if h < -1 {
            -1
        } else {
            h
        };
        v = if v > 1 {
            1
        } else if v < -1 {
            -1
        } else {
            v
        };
        let (ctx, x) = match (h, v) {
            (1, 1) => (13, 0),
            (1, 0) => (12, 0),
            (1, -1) => (11, 0),
            (0, 1) => (10, 0),
            (0, 0) => (9, 0),
            (0, -1) => (10, 1),
            (-1, 1) => (11, 1),
            (-1, 0) => (12, 1),
            _ => (13, 1),
        };
        t[i] = ctx | (x << 7);
        i += 1;
    }
    t
}

static SC: [u8; 256] = sc_lut();

/// Sign LUT index for stripe row `j` of the word `t` (already masked in causal mode), with
/// the west / east column words `tw` / `te`.
#[inline(always)]
fn sc_index(t: u32, tw: u32, te: u32, j: u32) -> usize {
    let s = t >> (3 * j);
    let sig = ((s >> 1) & 1) | ((s >> 6) & 2) | ((s >> 1) & 4) | ((s >> 2) & 8);
    let neg_n = if j == 0 { t >> 18 } else { t >> (16 + 3 * j) } & 1;
    let neg_s = (t >> (22 + 3 * j)) & 1;
    let neg_w = (tw >> (19 + 3 * j)) & 1;
    let neg_e = (te >> (19 + 3 * j)) & 1;
    (sig | (neg_n << 4) | (neg_s << 5) | (neg_w << 6) | (neg_e << 7)) as usize
}

trait Bits: Copy {
    fn bit(&mut self, cx: usize) -> u32;
    fn sign(&mut self, lut: u8) -> u32;
}

impl Bits for Mq<'_> {
    #[inline(always)]
    fn bit(&mut self, cx: usize) -> u32 {
        self.decode(cx)
    }
    #[inline(always)]
    fn sign(&mut self, lut: u8) -> u32 {
        self.decode((lut & 0x7F) as usize) ^ (lut >> 7) as u32
    }
}

impl Bits for Raw<'_> {
    #[inline(always)]
    fn bit(&mut self, _cx: usize) -> u32 {
        Raw::bit(self)
    }
    #[inline(always)]
    fn sign(&mut self, _lut: u8) -> u32 {
        Raw::bit(self)
    }
}

/// One codeword segment of a code-block: its bytes (possibly split over several packets) and
/// the number of coding passes they contain.
pub(crate) struct SegRef<'s, 'a> {
    pub chunks: &'s [&'a [u8]],
    pub passes: u8,
}

/// Flag grid capacity: code-blocks hold at most 4096 samples and are at most 1024 wide, so
/// `(w + 2) * (ceil(h / 4) + 2) <= 3284`. Indices are masked with `FMASK` / `MMASK` so the
/// compiler can drop bounds checks; in-range indices are unaffected by the mask.
const FLAGS_CAP: usize = 4096;
const FMASK: usize = FLAGS_CAP - 1;
const MAGS_CAP: usize = 4096;
const MMASK: usize = MAGS_CAP - 1;

/// Reusable per-thread tier-1 state.
pub(crate) struct T1 {
    pub w: usize,
    pub h: usize,
    flags: Box<[u32; FLAGS_CAP]>,
    mags: Box<[u32; MAGS_CAP]>,
    /// [`T1::decode_ht`]'s output for blocks deeper than 30 bit-planes (allocated on first
    /// use): sign in bit 63.
    mags64: Vec<u64>,
    /// The last block went to `mags64`.
    pub wide: bool,
    buf: Vec<u8>,
}

impl Default for T1 {
    fn default() -> T1 {
        T1 {
            w: 0,
            h: 0,
            flags: Box::new([0; FLAGS_CAP]),
            mags: Box::new([0; MAGS_CAP]),
            mags64: Vec::new(),
            wide: false,
            buf: Vec::new(),
        }
    }
}

struct Grid<'g> {
    w: usize,
    h: usize,
    /// Flag words per stripe row (`w + 2`).
    fs: usize,
    causal: bool,
    flags: &'g mut [u32; FLAGS_CAP],
    mags: &'g mut [u32; MAGS_CAP],
}

impl Grid<'_> {
    #[inline(always)]
    fn f(&self, i: usize) -> u32 {
        self.flags[i & FMASK]
    }

    #[inline(always)]
    fn or(&mut self, i: usize, v: u32) {
        self.flags[i & FMASK] |= v;
    }

    /// The word as seen by stripe row `j` (vertically causal mode hides the row below).
    #[inline(always)]
    fn view(&self, t: u32, j: u32) -> u32 {
        if self.causal && j == 3 { t & !BELOW } else { t }
    }

    /// Make row `j` of the stripe column with word `t` (index `i`) significant.
    #[inline(always)]
    fn set_sig(&mut self, t: &mut u32, i: usize, j: u32, neg: u32) {
        let fs = self.fs;
        *t |= (1 << (4 + 3 * j)) | (neg << (19 + 3 * j));
        self.or(i.wrapping_sub(1), 1 << (5 + 3 * j));
        self.or(i + 1, 1 << (3 + 3 * j));
        if j == 0 {
            let u = i.wrapping_sub(fs);
            self.or(u.wrapping_sub(1), 1 << 17);
            self.or(u, (1 << 16) | (neg * CHI_BELOW));
            self.or(u + 1, 1 << 15);
        } else if j == 3 {
            let d = i + fs;
            self.or(d - 1, 1 << 2);
            self.or(d, 2 | (neg * CHI_ABOVE));
            self.or(d + 1, 1);
        }
    }

    /// Decode the sign of row `j` and make it significant with magnitude `oph`.
    #[inline(always)]
    fn newsig<D: Bits>(&mut self, d: &mut D, t: &mut u32, i: usize, m: usize, j: u32, oph: u32) {
        let lut = SC[sc_index(
            self.view(*t, j),
            self.f(i.wrapping_sub(1)),
            self.f(i + 1),
            j,
        )];
        let s = d.sign(lut);
        self.mags[m & MMASK] = oph | (s << 31);
        self.set_sig(t, i, j, s);
    }

    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn spp_step<D: Bits>(
        &mut self,
        d: &mut D,
        t: &mut u32,
        i: usize,
        m: usize,
        j: u32,
        oph: u32,
        zc: &[u8; 512],
    ) {
        let v = self.view(*t, j);
        let nb = (v >> (3 * j)) & NBHD;
        if (*t >> (4 + 3 * j)) & 1 == 0 && nb != 0 {
            if d.bit(zc[nb as usize] as usize) != 0 {
                self.newsig(d, t, i, m, j, oph);
            }
            *t |= 1 << (21 + 3 * j);
        }
    }

    fn spp<D: Bits>(&mut self, dref: &mut D, bp: u32, zc: &[u8; 512]) {
        let mut d = *dref;
        let oph = 3u32 << bp;
        let (w, h, fs) = (self.w, self.h, self.fs);
        for s in 0..h.div_ceil(4) {
            let n = (h - 4 * s).min(4) as u32;
            let base = (s + 1) * fs + 1;
            for x in 0..w {
                let i = base + x;
                let mut t = self.f(i);
                if t == 0 {
                    continue;
                }
                let m = 4 * s * w + x;
                if n == 4 {
                    self.spp_step(&mut d, &mut t, i, m, 0, oph, zc);
                    self.spp_step(&mut d, &mut t, i, m + w, 1, oph, zc);
                    self.spp_step(&mut d, &mut t, i, m + 2 * w, 2, oph, zc);
                    self.spp_step(&mut d, &mut t, i, m + 3 * w, 3, oph, zc);
                } else {
                    for j in 0..n {
                        self.spp_step(&mut d, &mut t, i, m + j as usize * w, j, oph, zc);
                    }
                }
                self.flags[i & FMASK] = t;
            }
        }
        *dref = d;
    }

    #[inline(always)]
    fn mrp_step<D: Bits>(&mut self, d: &mut D, t: &mut u32, m: usize, j: u32, half: u32) {
        if (*t >> (4 + 3 * j)) & 0x20001 == 1 {
            // Significant (bit 4 + 3j) and not visited (bit 21 + 3j).
            let ctx = if (*t >> (20 + 3 * j)) & 1 != 0 {
                16
            } else if (self.view(*t, j) >> (3 * j)) & NBHD != 0 {
                15
            } else {
                14
            };
            let b = d.bit(ctx);
            let v = &mut self.mags[m & MMASK];
            *v = if b != 0 {
                v.wrapping_add(half)
            } else {
                v.wrapping_sub(half)
            };
            *t |= 1 << (20 + 3 * j);
        }
    }

    fn mrp<D: Bits>(&mut self, dref: &mut D, bp: u32) {
        let mut d = *dref;
        let half = 1u32 << bp;
        let (w, h, fs) = (self.w, self.h, self.fs);
        for s in 0..h.div_ceil(4) {
            let n = (h - 4 * s).min(4) as u32;
            let base = (s + 1) * fs + 1;
            for x in 0..w {
                let i = base + x;
                let mut t = self.f(i);
                if t & SIG_ROWS == 0 {
                    continue;
                }
                let m = 4 * s * w + x;
                if n == 4 {
                    self.mrp_step(&mut d, &mut t, m, 0, half);
                    self.mrp_step(&mut d, &mut t, m + w, 1, half);
                    self.mrp_step(&mut d, &mut t, m + 2 * w, 2, half);
                    self.mrp_step(&mut d, &mut t, m + 3 * w, 3, half);
                } else {
                    for j in 0..n {
                        self.mrp_step(&mut d, &mut t, m + j as usize * w, j, half);
                    }
                }
                self.flags[i & FMASK] = t;
            }
        }
        *dref = d;
    }

    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn cup_step(
        &mut self,
        d: &mut Mq,
        t: &mut u32,
        i: usize,
        m: usize,
        j: u32,
        oph: u32,
        zc: &[u8; 512],
    ) {
        // Neither significant (bit 4 + 3j) nor visited (bit 21 + 3j).
        if (*t >> (4 + 3 * j)) & 0x20001 == 0 {
            let nb = (self.view(*t, j) >> (3 * j)) & NBHD;
            if d.decode(zc[nb as usize] as usize) != 0 {
                self.newsig(d, t, i, m, j, oph);
            }
        }
    }

    fn cup(&mut self, dref: &mut Mq, bp: u32, zc: &[u8; 512]) {
        let mut d = *dref;
        let oph = 3u32 << bp;
        let (w, h, fs) = (self.w, self.h, self.fs);
        for s in 0..h.div_ceil(4) {
            let n = (h - 4 * s).min(4) as u32;
            let base = (s + 1) * fs + 1;
            for x in 0..w {
                let i = base + x;
                let mut t = self.f(i);
                let m = 4 * s * w + x;
                if n == 4 {
                    let mut j0 = 0;
                    if self.view(t, 3) == 0 {
                        if d.decode(CTX_RL) == 0 {
                            continue;
                        }
                        let r = (d.decode(CTX_UNI) << 1) | d.decode(CTX_UNI);
                        self.newsig(&mut d, &mut t, i, m + r as usize * w, r, oph);
                        j0 = r + 1;
                    }
                    // Dispatch on the first row still to be coded so each step is specialised.
                    match j0 {
                        0 => {
                            self.cup_step(&mut d, &mut t, i, m, 0, oph, zc);
                            self.cup_step(&mut d, &mut t, i, m + w, 1, oph, zc);
                            self.cup_step(&mut d, &mut t, i, m + 2 * w, 2, oph, zc);
                            self.cup_step(&mut d, &mut t, i, m + 3 * w, 3, oph, zc);
                        }
                        1 => {
                            self.cup_step(&mut d, &mut t, i, m + w, 1, oph, zc);
                            self.cup_step(&mut d, &mut t, i, m + 2 * w, 2, oph, zc);
                            self.cup_step(&mut d, &mut t, i, m + 3 * w, 3, oph, zc);
                        }
                        2 => {
                            self.cup_step(&mut d, &mut t, i, m + 2 * w, 2, oph, zc);
                            self.cup_step(&mut d, &mut t, i, m + 3 * w, 3, oph, zc);
                        }
                        3 => self.cup_step(&mut d, &mut t, i, m + 3 * w, 3, oph, zc),
                        _ => {}
                    }
                } else {
                    for j in 0..n {
                        self.cup_step(&mut d, &mut t, i, m + j as usize * w, j, oph, zc);
                    }
                }
                self.flags[i & FMASK] = t & !PI_ALL;
            }
        }
        *dref = d;
    }
}

impl T1 {
    /// Decode one code-block. Afterwards `mags[y * w + x]` holds twice the reconstructed
    /// magnitude (midpoint reconstruction) in its low 31 bits and the sign in bit 31 ([`NEG`]).
    /// `orient`: 0 LL, 1 HL, 2 LH, 3 HH.
    #[allow(clippy::too_many_arguments)]
    pub fn decode(
        &mut self,
        w: usize,
        h: usize,
        orient: u8,
        style: u8,
        numbps: u8,
        segs: &[SegRef<'_, '_>],
    ) {
        self.w = w;
        self.h = h;
        self.wide = false;
        if w * h > MAGS_CAP || w > 1024 {
            // Not producible by a valid COD/COC; decode as all zero.
            self.w = 0;
            self.h = 0;
            return;
        }
        let fs = w + 2;
        let nf = fs * (h.div_ceil(4) + 2);
        self.flags[..nf].fill(0);
        self.mags[..w * h].fill(0);
        if numbps == 0 || numbps > MAX_BITPLANES || w == 0 || h == 0 {
            return;
        }
        let zc = &ZC[match orient {
            1 => 1,
            3 => 2,
            _ => 0,
        }];
        let bypass = style & CBS_BYPASS != 0;
        let mut grid = Grid {
            w,
            h,
            fs,
            causal: style & CBS_CAUSAL != 0,
            flags: &mut self.flags,
            mags: &mut self.mags,
        };
        let mut mq = Mq::new();
        let mut bp = numbps as i32 - 1;
        let mut ptype = 2u8; // 0 SPP, 1 MRP, 2 CUP
        let mut pass_idx = 0usize;
        // Concatenate multi-chunk segments up front so `mq` can borrow the bytes.
        let buf = &mut self.buf;
        buf.clear();
        let mut spans = Vec::with_capacity(segs.len());
        for seg in segs {
            if seg.chunks.len() > 1 {
                let s = buf.len();
                for c in seg.chunks {
                    buf.extend_from_slice(c);
                }
                spans.push(Some((s, buf.len())));
            } else {
                spans.push(None);
            }
        }
        let buf: &[u8] = buf;
        'segs: for (seg, span) in segs.iter().zip(&spans) {
            let data: &[u8] = match span {
                Some((s, e)) => &buf[*s..*e],
                None => seg.chunks.first().copied().unwrap_or(&[]),
            };
            let raw_seg = bypass && pass_idx >= 10 && ptype != 2;
            let mut raw = Raw::new(data);
            if !raw_seg {
                mq.init(data);
            }
            for _ in 0..seg.passes {
                if bp < 0 {
                    break 'segs;
                }
                let b = bp as u32;
                match ptype {
                    0 => {
                        if raw_seg {
                            grid.spp(&mut raw, b, zc)
                        } else {
                            grid.spp(&mut mq, b, zc)
                        }
                    }
                    1 => {
                        if raw_seg {
                            grid.mrp(&mut raw, b)
                        } else {
                            grid.mrp(&mut mq, b)
                        }
                    }
                    _ => {
                        grid.cup(&mut mq, b, zc);
                        if style & CBS_SEGSYM != 0 {
                            for _ in 0..4 {
                                mq.decode(CTX_UNI);
                            }
                        }
                    }
                }
                if style & CBS_RESET != 0 {
                    mq.reset_contexts();
                }
                ptype = (ptype + 1) % 3;
                if ptype == 0 {
                    bp -= 1;
                }
                pass_idx += 1;
            }
        }
    }

    /// Decode one HTJ2K code-block (same output format as [`T1::decode`]). `segs[0]` is the
    /// cleanup segment of the HT set to decode and `segs[1]` (optional) its SigProp + MagRef
    /// segment. `cup_plane` is the bit-plane of the cleanup pass (`None` when it lies below
    /// plane 0). A malformed block decodes as all zero. `wide`: decode into 64-bit words
    /// ([`T1::row64`]), for blocks with more than [`MAX_BITPLANES`] bit-planes.
    pub fn decode_ht(
        &mut self,
        w: usize,
        h: usize,
        style: u8,
        cup_plane: Option<u32>,
        segs: &[SegRef<'_, '_>],
        wide: bool,
    ) {
        self.w = w;
        self.h = h;
        self.wide = wide;
        if w * h > MAGS_CAP || w > 1024 {
            self.w = 0;
            self.h = 0;
            return;
        }
        if wide {
            self.mags64.resize(MAGS_CAP, 0);
            self.mags64[..w * h].fill(0);
        } else {
            self.mags[..w * h].fill(0);
        }
        let (Some(p), Some(cup)) = (cup_plane, segs.first()) else {
            return;
        };
        let buf = &mut self.buf;
        buf.clear();
        for c in cup.chunks {
            buf.extend_from_slice(c);
        }
        let len1 = buf.len();
        let mut ref_passes = 0;
        if let Some(r) = segs.get(1) {
            for c in r.chunks {
                buf.extend_from_slice(c);
            }
            ref_passes = r.passes as u32;
        }
        let (cup, refine) = buf.split_at(len1);
        let causal = style & CBS_CAUSAL != 0;
        if wide {
            let out = &mut self.mags64[..w * h];
            if !crate::ht::decode(out, w, h, cup, refine, ref_passes, p, causal) {
                out.fill(0);
            }
        } else {
            let out = &mut self.mags[..w * h];
            if !crate::ht::decode(out, w, h, cup, refine, ref_passes, p, causal) {
                out.fill(0);
            }
        }
    }

    /// [`T1::row`] of a block decoded `wide`: the sign in bit 63.
    #[inline]
    pub fn row64(&self, y: usize, x0: usize, n: usize) -> Option<&[u64]> {
        if !self.wide || y >= self.h || x0 + n > self.w {
            return None;
        }
        self.mags64.get(y * self.w + x0..y * self.w + x0 + n)
    }

    /// Row `y` of the block, columns `x0..x0 + n`: twice the reconstructed magnitudes, with
    /// the sign in bit 31 ([`NEG`]). `None` outside the block.
    #[inline]
    pub fn row(&self, y: usize, x0: usize, n: usize) -> Option<&[u32]> {
        if y >= self.h || x0 + n > self.w {
            return None;
        }
        self.mags.get(y * self.w + x0..y * self.w + x0 + n)
    }
}
