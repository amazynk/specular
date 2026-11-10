//! Arithmetic-coded frames (SOF9 sequential, SOF10 progressive), written from ITU-T T.81:
//! the binary decoder of Annex D, the DC/AC models of F.1.4.4 / F.2.4 and the progressive
//! procedures of G.1.3.3. Lossless frames (SOF11, Annex H) share the binary decoder and are
//! decoded in [`lossless`].
//!
//! Entropy decoding is serial within a restart interval (the coder carries adaptive statistics
//! from block to block). A scan without restart markers is decoded on one thread into
//! per-component coefficient buffers; with markers, bands of rows are decoded in parallel,
//! each from the marker that starts its first interval (statistics reset there). The scans of
//! a progressive frame without markers run concurrently, pipelined by MCU row behind the
//! earlier scans of the same coefficients ([`decode_pipelined`]). The final
//! dequantisation + (scaled) IDCT is the multi-threaded pass of the coefficient path
//! ([`crate::coef::output`]). At 1/8 scale only DC coefficients are kept and progressive AC
//! scans are not read; sequential AC still has to be decoded, as every decision adapts the
//! statistics.
//!
//! The binary decoder keeps `C` and `A` scaled to the top of 64-bit registers with up to 48
//! prefetched stream bits below `C`, so renormalisation is one shift by `A`'s leading zeros,
//! the decision is branch-free (conditional moves, the next state from one table word per
//! bin) and bytes are fetched four at a time; `FF 00` stuffing and markers take an
//! out-of-line byte-wise path.
//!
//! Region decodes without an index decode every scan from the top down to the region's last
//! MCU row (progressive refinement needs the history of every coefficient it touches; a
//! sequential scan with restart markers, decoded in bands, starts at the interval holding the
//! region's first row). A
//! [`crate::RowIndex`] keeps, per scan and scan row, the complete decoder state, including the
//! adaptive statistics of the tables the scan uses (a few hundred bytes per row), so indexed
//! region decodes start every scan at the region's first row; the scans of a progressive
//! frame then run pipelined from that row as in a full decode.
//!
//! Corrupt data never panics: a coefficient run or magnitude outside the spec stops the scan
//! (the rest keeps what earlier scans gave, i.e. grey for a sequential file) and marks the
//! decode truncated, as does data that ends before the image does.

use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};

use crate::coef::{Coefs, Kind, ScanPlan, Window, output, plan_scans};
use crate::error::{JpegError, Result};
use crate::geom::{OutGeom, alloc_i16};
use crate::idct::Simd;
use crate::marker::{Parsed, Scan, ZIGZAG};

pub(crate) mod lossless;

/// T.81 Table D.2: Qe, next state after an LPS, next state after an MPS, MPS switch. Entry
/// 113 is the fixed 0.5 estimate used for sign and refinement bits (it never adapts).
#[rustfmt::skip]
const QE: [(u16, u8, u8, bool); 114] = [
    (0x5A1D, 1, 1, true), (0x2586, 14, 2, false), (0x1114, 16, 3, false),
    (0x080B, 18, 4, false), (0x03D8, 20, 5, false), (0x01DA, 23, 6, false),
    (0x00E5, 25, 7, false), (0x006F, 28, 8, false), (0x0036, 30, 9, false),
    (0x001A, 33, 10, false), (0x000D, 35, 11, false), (0x0006, 9, 12, false),
    (0x0003, 10, 13, false), (0x0001, 12, 13, false), (0x5A7F, 15, 15, true),
    (0x3F25, 36, 16, false), (0x2CF2, 38, 17, false), (0x207C, 39, 18, false),
    (0x17B9, 40, 19, false), (0x1182, 42, 20, false), (0x0CEF, 43, 21, false),
    (0x09A1, 45, 22, false), (0x072F, 46, 23, false), (0x055C, 48, 24, false),
    (0x0406, 49, 25, false), (0x0303, 51, 26, false), (0x0240, 52, 27, false),
    (0x01B1, 54, 28, false), (0x0144, 56, 29, false), (0x00F5, 57, 30, false),
    (0x00B7, 59, 31, false), (0x008A, 60, 32, false), (0x0068, 62, 33, false),
    (0x004E, 63, 34, false), (0x003B, 32, 35, false), (0x002C, 33, 9, false),
    (0x5AE1, 37, 37, true), (0x484C, 64, 38, false), (0x3A0D, 65, 39, false),
    (0x2EF1, 67, 40, false), (0x261F, 68, 41, false), (0x1F33, 69, 42, false),
    (0x19A8, 70, 43, false), (0x1518, 72, 44, false), (0x1177, 73, 45, false),
    (0x0E74, 74, 46, false), (0x0BFB, 75, 47, false), (0x09F8, 77, 48, false),
    (0x0861, 78, 49, false), (0x0706, 79, 50, false), (0x05CD, 48, 51, false),
    (0x04DE, 50, 52, false), (0x040F, 50, 53, false), (0x0363, 51, 54, false),
    (0x02D4, 52, 55, false), (0x025C, 53, 56, false), (0x01F8, 54, 57, false),
    (0x01A4, 55, 58, false), (0x0160, 56, 59, false), (0x0125, 57, 60, false),
    (0x00F6, 58, 61, false), (0x00CB, 59, 62, false), (0x00AB, 61, 63, false),
    (0x008F, 61, 32, false), (0x5B12, 65, 65, true), (0x4D04, 80, 66, false),
    (0x412C, 81, 67, false), (0x37D8, 82, 68, false), (0x2FE8, 83, 69, false),
    (0x293C, 84, 70, false), (0x2379, 86, 71, false), (0x1EDF, 87, 72, false),
    (0x1AA9, 87, 73, false), (0x174E, 72, 74, false), (0x1424, 72, 75, false),
    (0x119C, 74, 76, false), (0x0F6B, 74, 77, false), (0x0D51, 75, 78, false),
    (0x0BB6, 77, 79, false), (0x0A40, 77, 48, false), (0x5832, 80, 81, true),
    (0x4D1C, 88, 82, false), (0x438E, 89, 83, false), (0x3BDD, 90, 84, false),
    (0x34EE, 91, 85, false), (0x2EAE, 92, 86, false), (0x299A, 93, 87, false),
    (0x2516, 86, 71, false), (0x5570, 88, 89, true), (0x4CA9, 95, 90, false),
    (0x44D9, 96, 91, false), (0x3E22, 97, 92, false), (0x3824, 99, 93, false),
    (0x32B4, 99, 94, false), (0x2E17, 93, 86, false), (0x56A8, 95, 96, true),
    (0x4F46, 101, 97, false), (0x47E5, 102, 98, false), (0x41CF, 103, 99, false),
    (0x3C3D, 104, 100, false), (0x375E, 99, 93, false), (0x5231, 105, 102, false),
    (0x4C0F, 106, 103, false), (0x4639, 107, 104, false), (0x415E, 103, 99, false),
    (0x5627, 105, 106, true), (0x50E7, 108, 107, false), (0x4B85, 109, 103, false),
    (0x5597, 110, 109, false), (0x504F, 111, 107, false), (0x5A10, 110, 111, true),
    (0x5522, 112, 109, false), (0x59EB, 112, 111, true), (0x5A1D, 113, 113, false),
];

/// The fixed-probability state (T.81 F.2.4.1: signs and refinement bits).
const FIXED: u8 = 113;
/// Statistics bins per DC / AC conditioning table (T.81 F.1.4.4: 5 contexts x 4 + 2 x 14,
/// and 63 x 3 + 2 x 28; rounded up to a power of two so bin indices are masked, not checked).
const DC_BINS: usize = 64;
const AC_BINS: usize = 256;
/// First magnitude bins of the DC model and of the AC model at k <= Kx / k > Kx.
const DC_X1: usize = 20;
const AC_X2_LOW: usize = 189;
const AC_X2_HIGH: usize = 217;
/// Zero bytes fed past the end of a scan whose data runs to the end of the file, after which
/// the data is taken to be cut short (an encoder's flush leaves a few implied zero bytes).
const CUT_FILL: u32 = 64;

/// Per bin byte (state in bits 0..7, MPS in bit 7): Qe in bits 0..16, the bin byte after an
/// MPS decision in bits 16..24 and after an LPS decision in bits 24..32 (the MPS switch
/// folded in). States past the table (only reachable through a foreign checkpoint) behave as
/// the fixed state.
static NEXT: [u32; 256] = {
    let mut t = [0u32; 256];
    let mut b = 0;
    while b < 256 {
        let s = b & 0x7F;
        let mps = (b >> 7) as u32;
        let i = if s > FIXED as usize {
            FIXED as usize
        } else {
            s
        };
        let (qe, nlps, nmps, switch) = QE[i];
        let lps_mps = mps ^ switch as u32;
        t[b] = qe as u32 | ((nmps as u32 | mps << 7) << 16) | ((nlps as u32 | lps_mps << 7) << 24);
        b += 1;
    }
    t
};

/// Bit position of `Cx` in the code register.
const CX: u32 = 48;

/// Binary arithmetic decoder (T.81 D.2) over one scan's entropy-coded data. A bin is a byte:
/// the state index in the low 7 bits, the MPS in the top bit.
///
/// The code register holds `Cx` (16 bits, always below `a`) in its top bits and up to 48
/// prefetched stream bits below it, so a renormalisation is one shift by `a`'s leading zeros
/// and bytes are fetched four at a time; stuffed `FF 00` pairs and markers take the slow
/// byte-wise path.
#[derive(Clone, Copy)]
struct Coder<'a> {
    /// The file up to the marker that ends the scan.
    data: &'a [u8],
    /// Next byte to read.
    pos: usize,
    /// Code register: `Cx` in bits 48..64, the next `bits` stream bits right below it.
    c: u64,
    /// Interval `A` (`0x8000..=0x10000` between decisions) aligned with `Cx`, i.e. `A << 48`
    /// modulo 2^64: 0 stands for 0x10000 (before the first decision only), so the top bit is
    /// set whenever no renormalisation is due.
    a: u64,
    /// Prefetched bits below `Cx` (at least 16 between decisions).
    bits: u32,
    /// Zero bytes fed since a marker or the end of the data was reached.
    fill: u32,
}

impl<'a> Coder<'a> {
    fn new(data: &'a [u8], pos: usize, end: usize) -> Coder<'a> {
        let mut d = Coder {
            data: &data[..end.min(data.len())],
            pos,
            c: 0,
            a: 0,
            bits: 0,
            fill: 0,
        };
        d.init();
        d
    }

    /// INITDEC: the first two bytes into `Cx`.
    fn init(&mut self) {
        self.a = 0;
        self.c = 0;
        self.bits = 0;
        self.refill();
        self.c <<= 16;
        self.bits -= 16;
        if self.bits < 16 {
            self.refill();
        }
    }

    /// Prefetch stream bits (`bits < 16` on entry): four bytes at once when none of them is
    /// 0xFF, else byte-wise.
    #[inline(always)]
    fn refill(&mut self) {
        if let Some(w) = self.data.get(self.pos..self.pos + 4) {
            let w = u32::from_be_bytes([w[0], w[1], w[2], w[3]]);
            let x = !w;
            if x.wrapping_sub(0x0101_0101) & !x & 0x8080_8080 == 0 {
                self.c |= (w as u64) << (CX - 32 - self.bits);
                self.bits += 32;
                self.pos += 4;
                return;
            }
        }
        *self = self.refilled_bytes();
    }

    /// BYTE_IN until the register is full: a stuffed `FF 00` is one 0xFF byte, a marker (or
    /// the end of the scan) reads as zeros and is not passed. By value, so that the hot
    /// loops' coder never has its address taken and stays in registers.
    #[inline(never)]
    fn refilled_bytes(mut self) -> Self {
        while self.bits <= CX - 8 {
            let b = if let Some(&b) = self.data.get(self.pos) {
                if b != 0xFF {
                    self.pos += 1;
                    b
                } else if self.data.get(self.pos + 1) == Some(&0) {
                    self.pos += 2;
                    0xFF
                } else {
                    self.fill += 1;
                    0
                }
            } else {
                self.fill += 1;
                0
            };
            self.c |= (b as u64) << (CX - 8 - self.bits);
            self.bits += 8;
        }
        self
    }

    /// Zero bytes consumed past the data (the prefetched ones not counted).
    fn filled(&self) -> u32 {
        self.fill.saturating_sub(self.bits / 8)
    }

    /// DECODE (D.2.2): one binary decision with the adaptive bin `st`, without branches but
    /// the refill: an MPS that needs no renormalisation shifts by 0 and keeps its state, and
    /// the conditional exchange is `(a < qe) ^ lps_half` (always false while `a >= 0x8000`,
    /// as every Qe is below 0x8000).
    #[inline(always)]
    fn decode(&mut self, st: &mut u8) -> bool {
        let s = *st;
        let e = NEXT[s as usize];
        let qe = e & 0xFFFF;
        let (lps, n) = self.decide((qe as u64) << CX);
        let next = (e >> (16 + 8 * lps as u32)) as u8;
        *st = if n != 0 { next } else { s };
        (s >= 0x80) ^ lps
    }

    /// The decision with Qe `qe` (aligned with `Cx`; state update left to the caller):
    /// whether the LPS was decoded, and the renormalisation shift (0 if none).
    #[inline(always)]
    fn decide(&mut self, qe: u64) -> (bool, u32) {
        self.a = self.a.wrapping_sub(qe);
        let lps_half = self.c >= self.a;
        let lps = (self.a < qe) ^ lps_half;
        self.c -= if lps_half { self.a } else { 0 };
        self.a = if lps_half { qe } else { self.a };
        // RENORMD in one step (`a` is never 0 here).
        let n = self.a.leading_zeros();
        self.a <<= n;
        self.c <<= n;
        self.bits -= n;
        if self.bits < 16 {
            self.refill();
        }
        (lps, n)
    }

    /// A decision with the fixed 0.5 estimate (MPS 0, never adapts).
    #[inline(always)]
    fn bit(&mut self) -> bool {
        self.decide((QE[FIXED as usize].0 as u64) << CX).0
    }

    /// Skip to just past the next RSTn marker (or to the end of the scan) and restart the
    /// coder there.
    #[inline(never)]
    fn restarted(mut self) -> Self {
        let mut p = self.pos;
        let mut next = self.data.len();
        while p + 1 < self.data.len() {
            if self.data[p] == 0xFF && (0xD0..=0xD7).contains(&self.data[p + 1]) {
                next = p + 2;
                break;
            }
            p += 1;
        }
        self.pos = next;
        self.fill = 0;
        self.init();
        self
    }
}

/// Decoder state at the start of one scan row (see [`crate::RowIndex`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ArithCp {
    pos: usize,
    c: u64,
    a: u64,
    bits: u32,
    fill: u32,
    /// Units (MCUs, or blocks of a non-interleaved scan) left before the next restart.
    restart_left: u32,
    dc_pred: [i32; 4],
    dc_ctx: [u8; 4],
    /// The statistics of the tables the scan uses ([`Model::used`] order).
    stats: Box<[u8]>,
}

impl ArithCp {
    pub fn heap_bytes(&self) -> usize {
        std::mem::size_of::<ArithCp>() + self.stats.len()
    }
}

/// Per scan: one checkpoint per scan row.
pub(crate) type ArithIndex = Vec<Vec<ArithCp>>;

/// The adaptive part of a scan's state: statistics bins and DC predictions/contexts.
struct Model {
    dc: [[u8; DC_BINS]; 4],
    ac: [[u8; AC_BINS]; 4],
    dc_pred: [i32; 4],
    dc_ctx: [u8; 4],
    /// DC is predicted from the previous block (false in differential frames, T.81 J.2.3.1).
    predict: bool,
    /// (is AC, table) pairs the scan uses.
    used: Vec<(bool, usize)>,
}

impl Model {
    fn new(s: &Scan, kind: Kind) -> Model {
        let mut used = Vec::new();
        for c in &s.comps {
            if matches!(kind, Kind::Seq | Kind::DcFirst) && !used.contains(&(false, c.td)) {
                used.push((false, c.td));
            }
            if matches!(kind, Kind::Seq | Kind::AcFirst | Kind::AcRefine)
                && !used.contains(&(true, c.ta))
            {
                used.push((true, c.ta));
            }
        }
        Model {
            dc: [[0; DC_BINS]; 4],
            ac: [[0; AC_BINS]; 4],
            dc_pred: [0; 4],
            dc_ctx: [0; 4],
            predict: true,
            used,
        }
    }

    /// The state at the start of a scan and after every restart marker.
    fn reset(&mut self) {
        for &(ac, t) in &self.used {
            if ac {
                self.ac[t] = [0; AC_BINS];
            } else {
                self.dc[t] = [0; DC_BINS];
            }
        }
        self.dc_pred = [0; 4];
        self.dc_ctx = [0; 4];
    }

    fn checkpoint(&self, d: Coder, restart_left: u32) -> ArithCp {
        let mut stats = Vec::new();
        for &(ac, t) in &self.used {
            stats.extend_from_slice(if ac { &self.ac[t][..] } else { &self.dc[t][..] });
        }
        ArithCp {
            pos: d.pos,
            c: d.c,
            a: d.a,
            bits: d.bits,
            fill: d.fill,
            restart_left,
            dc_pred: self.dc_pred,
            dc_ctx: self.dc_ctx,
            stats: stats.into_boxed_slice(),
        }
    }

    /// Restore `cp` (recorded for this scan); returns the coder and the restart count.
    fn restore<'a>(
        &mut self,
        cp: &ArithCp,
        data: &'a [u8],
        end: usize,
    ) -> Result<(Coder<'a>, u32)> {
        let want: usize = self
            .used
            .iter()
            .map(|&(ac, _)| if ac { AC_BINS } else { DC_BINS })
            .sum();
        if cp.stats.len() != want
            || (cp.a != 0 && (cp.a >> 63 == 0 || cp.c >= cp.a))
            || cp.a & ((1 << CX) - 1) != 0
            || !(16..=CX).contains(&cp.bits)
        {
            return Err(JpegError::IndexMismatch);
        }
        let mut o = 0;
        for &(ac, t) in &self.used {
            if ac {
                self.ac[t].copy_from_slice(&cp.stats[o..o + AC_BINS]);
                o += AC_BINS;
            } else {
                self.dc[t].copy_from_slice(&cp.stats[o..o + DC_BINS]);
                o += DC_BINS;
            }
        }
        self.dc_pred = cp.dc_pred;
        self.dc_ctx = cp.dc_ctx;
        let d = Coder {
            data: &data[..end.min(data.len())],
            pos: cp.pos,
            c: cp.c,
            a: cp.a,
            bits: cp.bits,
            fill: cp.fill,
        };
        Ok((d, cp.restart_left))
    }
}

/// A scan whose data is not decodable any further (T.81 bounds exceeded).
struct Broken;

/// Decode one DC difference (T.81 F.2.4.1) for component slot `slot` with table `t`, and
/// update its prediction context. Returns the difference.
#[inline(always)]
fn dc_diff(
    d: &mut Coder,
    m: &mut Model,
    slot: usize,
    t: usize,
    l: u8,
    u: u8,
) -> std::result::Result<i32, Broken> {
    let st = &mut m.dc[t];
    let ctx = m.dc_ctx[slot] as usize;
    if !d.decode(&mut st[ctx & (DC_BINS - 1)]) {
        m.dc_ctx[slot] = 0;
        return Ok(0);
    }
    let neg = d.decode(&mut st[(ctx + 1) & (DC_BINS - 1)]);
    let mut i = ctx + 2 + neg as usize;
    let mut mag: i32 = 0;
    if d.decode(&mut st[i & (DC_BINS - 1)]) {
        mag = 1;
        i = DC_X1;
        while d.decode(&mut st[i & (DC_BINS - 1)]) {
            mag <<= 1;
            i += 1;
            if mag >= 0x8000 || i >= DC_X1 + 15 {
                return Err(Broken);
            }
        }
    }
    // Conditioning (F.1.4.4.1.2) on the magnitude category of the difference.
    m.dc_ctx[slot] = if mag < (1i32 << l) >> 1 {
        0
    } else if mag > (1i32 << u) >> 1 {
        12 + 4 * neg as u8
    } else {
        4 + 4 * neg as u8
    };
    let mut v = mag;
    i += 14;
    let mut bitv = mag >> 1;
    while bitv != 0 {
        v |= bitv & (d.decode(&mut st[i & (DC_BINS - 1)]) as i32).wrapping_neg();
        bitv >>= 1;
    }
    v += 1;
    Ok(if neg { -v } else { v })
}

/// Decode the AC coefficients `ss..=se` of a first (or sequential) scan into `blk` (natural
/// order), each value shifted left by `al`. `kx` is the conditioning parameter K.
#[inline(always)]
fn ac_first(
    d: &mut Coder,
    st: &mut [u8; AC_BINS],
    blk: &mut [i16],
    ss: usize,
    se: usize,
    al: u8,
    kx: usize,
) -> std::result::Result<(), Broken> {
    let mut k = ss;
    while k <= se {
        let mut i = 3 * (k - 1);
        if d.decode(&mut st[i & (AC_BINS - 1)]) {
            break; // EOB
        }
        while !d.decode(&mut st[(i + 1) & (AC_BINS - 1)]) {
            i += 3;
            k += 1;
            if k > se {
                return Err(Broken);
            }
        }
        let neg = d.bit();
        i += 2;
        let mut mag: i32 = 0;
        if d.decode(&mut st[i & (AC_BINS - 1)]) {
            mag = 1;
            if d.decode(&mut st[i & (AC_BINS - 1)]) {
                mag = 2;
                i = if k <= kx { AC_X2_LOW } else { AC_X2_HIGH };
                let base = i;
                while d.decode(&mut st[i & (AC_BINS - 1)]) {
                    mag <<= 1;
                    i += 1;
                    if mag >= 0x8000 || i >= base + 14 {
                        return Err(Broken);
                    }
                }
            }
        }
        let mut v = mag;
        i += 14;
        let mut bitv = mag >> 1;
        while bitv != 0 {
            v |= bitv & (d.decode(&mut st[i & (AC_BINS - 1)]) as i32).wrapping_neg();
            bitv >>= 1;
        }
        v += 1;
        let v = if neg { -v } else { v };
        blk[ZIGZAG[k] as usize] = (v << al) as i16;
        k += 1;
    }
    Ok(())
}

/// AC successive-approximation refinement (T.81 G.1.3.3.2) of `blk` (natural order).
#[inline(always)]
fn ac_refine(
    d: &mut Coder,
    st: &mut [u8; AC_BINS],
    blk: &mut [i16],
    ss: usize,
    se: usize,
    al: u8,
) -> std::result::Result<(), Broken> {
    let p1: i16 = (1i32 << al) as i16;
    let m1: i16 = (-1i32 << al) as i16;
    // The previous scans' end of block.
    let mut kex = se;
    while kex > 0 && blk[ZIGZAG[kex] as usize] == 0 {
        kex -= 1;
    }
    let mut k = ss;
    while k <= se {
        let mut i = 3 * (k - 1);
        if k > kex && d.decode(&mut st[i & (AC_BINS - 1)]) {
            break; // EOB
        }
        loop {
            let z = ZIGZAG[k] as usize;
            if blk[z] != 0 {
                if d.decode(&mut st[(i + 2) & (AC_BINS - 1)]) {
                    blk[z] = if blk[z] < 0 {
                        blk[z].wrapping_add(m1)
                    } else {
                        blk[z].wrapping_add(p1)
                    };
                }
                break;
            }
            if d.decode(&mut st[(i + 1) & (AC_BINS - 1)]) {
                blk[z] = if d.bit() { m1 } else { p1 };
                break;
            }
            i += 3;
            k += 1;
            if k > se {
                return Err(Broken);
            }
        }
        k += 1;
    }
    Ok(())
}

pub(crate) struct ArithParams<'c> {
    pub simd: Simd,
    pub threads: usize,
    pub cancel: Option<&'c AtomicBool>,
    pub index: Option<&'c ArithIndex>,
    pub want_index: bool,
    /// Skip the final IDCT (index building only).
    pub no_output: bool,
}

pub(crate) struct ArithOutcome {
    pub truncated: bool,
    pub index: Option<ArithIndex>,
}

/// Coefficient storage of one component.
struct Store {
    win: Window,
    dc: Vec<i16>,
    /// 64 coefficients per block (natural order, `[0]` unused); empty when only DC is kept.
    ac: Vec<i16>,
}

/// Decode an arithmetic-coded frame into `planes` (geometry `g`).
pub(crate) fn decode(
    p: &Parsed,
    data: &[u8],
    g: &OutGeom,
    planes: &mut [&mut [u8]],
    ap: &ArithParams,
) -> Result<ArithOutcome> {
    let f = &p.frame;
    // Recording a progressive index parses every scan, refinements included, so it keeps every
    // coefficient; a sequential scan parses blocks outside the kept window into scratch.
    let keep_all = ap.want_index && f.progressive;
    let dc_only = g.n == 1 && f.precision == 8 && !keep_all;
    let with_index = ap.index.is_some() && !ap.want_index;
    // Sequential scans need no history; progressive ones do, from the first row they parse.
    let store_from_top = f.progressive && !with_index;
    // An index-only build of a sequential file keeps no coefficients.
    let store = !(ap.no_output && !f.progressive);
    let plans = plan_scans(p, g, dc_only, store_from_top || keep_all);
    tracing::debug!(
        target: "jpeg",
        scans = plans.len(),
        dc_only,
        with_index,
        want_index = ap.want_index,
        "arithmetic: coefficient buffers"
    );
    if let Some(ix) = ap.index.filter(|_| with_index) {
        if ix.len() != p.scans.len() {
            return Err(JpegError::IndexMismatch);
        }
        for pl in &plans {
            if ix[pl.si].len() != pl.rows as usize {
                return Err(JpegError::IndexMismatch);
            }
        }
    }
    let mut stores = Vec::with_capacity(f.comps.len());
    for c in &f.comps {
        let (by0, by1) = if !store {
            (0, 0)
        } else if keep_all {
            (0, c.bh_pad)
        } else if store_from_top {
            (0, g.my1 * c.v)
        } else {
            (g.my0 * c.v, g.my1 * c.v)
        };
        let win = Window {
            bx: 0..c.bw_pad,
            by: by0..by1,
        };
        let n = win.len();
        stores.push(Store {
            win,
            dc: alloc_i16(n)?,
            ac: if dc_only {
                Vec::new()
            } else {
                alloc_i16(n * 64)?
            },
        });
    }

    let mut truncated = false;
    let mut index: ArithIndex = vec![Vec::new(); p.scans.len()];
    let pipelined = if f.progressive && ap.threads > 1 && !ap.want_index {
        decode_pipelined(p, data, &plans, &mut stores, dc_only, ap)?
    } else {
        None
    };
    if let Some(cut) = pipelined {
        truncated = cut;
    }
    for pl in plans.iter().filter(|_| pipelined.is_none()) {
        if pl.need.is_empty() && !ap.want_index {
            continue;
        }
        let rec = ap.want_index.then(|| Vec::with_capacity(pl.rows as usize));
        let (cut, rec) = decode_scan(p, data, pl, &mut stores, dc_only, ap, rec)?;
        truncated |= cut;
        if let Some(r) = rec {
            index[pl.si] = r;
        }
    }

    if !ap.no_output {
        let win: Vec<Window> = stores.iter().map(|s| s.win.clone()).collect();
        let dc: Vec<Vec<i16>> = stores
            .iter_mut()
            .map(|s| std::mem::take(&mut s.dc))
            .collect();
        let ac: Vec<Vec<i16>> = stores
            .iter_mut()
            .map(|s| std::mem::take(&mut s.ac))
            .collect();
        let coefs = Coefs {
            win: &win,
            dc: &dc,
            ac: &ac,
            dc_only,
        };
        output(p, g, &coefs, planes, ap.simd, ap.threads)?;
    }
    let index = (ap.want_index && !truncated).then_some(index);
    Ok(ArithOutcome { truncated, index })
}

/// Per component: its DC values and its AC slots.
pub(crate) type ComponentCoefs = (Vec<i16>, Vec<i16>);

/// Every quantised coefficient of an arithmetic-coded DCT frame, per frame component over its
/// MCU-padded block grid (`bw_pad * bh_pad` blocks, row-major): the DC values and 64 AC slots
/// per block in natural order (slot 0 unused). Serial; used by the lossless transform.
/// Returns whether the data ended early or broke off.
pub(crate) fn coefficients(p: &Parsed, data: &[u8]) -> Result<(Vec<ComponentCoefs>, bool)> {
    let f = &p.frame;
    let g = OutGeom::new(f, crate::Scale::Full, None)?;
    let plans = plan_scans(p, &g, false, true);
    let mut stores = Vec::with_capacity(f.comps.len());
    for c in &f.comps {
        let win = Window {
            bx: 0..c.bw_pad,
            by: 0..c.bh_pad,
        };
        let n = win.len();
        stores.push(Store {
            win,
            dc: alloc_i16(n)?,
            ac: alloc_i16(n * 64)?,
        });
    }
    let ap = ArithParams {
        simd: Simd::detect(),
        threads: 1,
        cancel: None,
        index: None,
        want_index: false,
        no_output: true,
    };
    let mut truncated = false;
    for pl in &plans {
        let (cut, _) = decode_scan(p, data, pl, &mut stores, false, &ap, None)?;
        truncated |= cut;
    }
    Ok((
        stores.into_iter().map(|s| (s.dc, s.ac)).collect(),
        truncated,
    ))
}

/// The coefficients a scan decode writes to, per frame component: the stored window (or the
/// part of it a restart band covers) and its DC / AC arrays.
struct View<'s> {
    win: Window,
    dc: &'s mut [i16],
    /// 64 per block; empty when only DC is kept.
    ac: &'s mut [i16],
}

/// What every unit of one scan decode needs.
struct ScanCx<'a> {
    p: &'a Parsed,
    pl: &'a ScanPlan<'a>,
    dc_only: bool,
    cancel: Option<&'a AtomicBool>,
    /// The scan's data runs to the end of the file (it has been cut short).
    open_end: bool,
    ss: usize,
    se: usize,
    al: u8,
}

/// Decode the rows of one scan (all rows when recording an index, else `pl.need`, from the
/// index checkpoint at its first row when there is one, or in parallel restart bands).
/// Returns whether the data ended early or broke off, and the recorded checkpoints.
fn decode_scan(
    p: &Parsed,
    data: &[u8],
    pl: &ScanPlan,
    stores: &mut [Store],
    dc_only: bool,
    ap: &ArithParams,
    mut rec: Option<Vec<ArithCp>>,
) -> Result<(bool, Option<Vec<ArithCp>>)> {
    let s = pl.scan;
    let ri = s.restart_interval;
    let end = s.data_end.min(data.len());
    let cx = ScanCx {
        p,
        pl,
        dc_only,
        cancel: ap.cancel,
        open_end: end >= data.len(),
        ss: s.ss.max(1) as usize,
        se: (s.se as usize).min(63),
        al: s.al,
    };
    if rec.is_none()
        && ap.threads > 1
        && ri > 0
        && let Some(cut) = decode_restart_bands(&cx, data, end, stores, ap.threads)?
    {
        return Ok((cut, None));
    }
    let mut m = Model::new(s, pl.kind);
    m.predict = !p.frame.differential;
    let (first, last) = if rec.is_some() {
        (0, pl.rows)
    } else if ap.index.is_some() {
        (pl.need.start, pl.need.end)
    } else {
        (0, pl.need.end)
    };
    let (d, restart_left) = match ap.index {
        Some(ix) if rec.is_none() && first > 0 => {
            m.restore(&ix[pl.si][first as usize], data, end)?
        }
        _ => {
            m.reset();
            (Coder::new(data, s.data_start, end), ri)
        }
    };
    let mut views: Vec<View> = stores
        .iter_mut()
        .map(|st| View {
            win: st.win.clone(),
            dc: &mut st.dc,
            ac: &mut st.ac,
        })
        .collect();
    let cut = decode_rows(
        &cx,
        d,
        &mut m,
        restart_left,
        first..last,
        &mut views,
        rec.as_mut(),
    )?;
    Ok((cut, if cut { None } else { rec }))
}

/// Decode scan rows `rows` from the coder state `d`/`m`/`restart_left` (the coder by value,
/// so it stays in registers) (recording a
/// checkpoint per row into `rec`). Returns whether the data ended early or broke off (the
/// scan stops there).
fn decode_rows(
    cx: &ScanCx,
    mut d: Coder,
    m: &mut Model,
    mut restart_left: u32,
    rows: Range<u32>,
    views: &mut [View],
    mut rec: Option<&mut Vec<ArithCp>>,
) -> Result<bool> {
    let pl = cx.pl;
    let mut scratch = [0i16; 64];
    for row in rows {
        if let Some(r) = rec.as_mut() {
            r.push(m.checkpoint(d, restart_left));
        }
        if let Some(c) = cx.cancel
            && c.load(Ordering::Relaxed)
        {
            return Err(JpegError::Cancelled);
        }
        for ux in 0..pl.units_x {
            if unit(
                cx,
                &mut d,
                m,
                &mut restart_left,
                Some((row, ux)),
                views,
                &mut scratch,
            )
            .is_err()
            {
                tracing::debug!(target: "jpeg", scan = pl.si, row, "arithmetic: corrupt data, scan stopped");
                return Ok(true);
            }
        }
        if cx.open_end && d.filled() > CUT_FILL {
            tracing::debug!(target: "jpeg", scan = pl.si, row, "arithmetic: data ends early");
            return Ok(true);
        }
    }
    Ok(false)
}

/// Decode one unit (an MCU, or a block of a non-interleaved scan) at `at` = (row, column),
/// or with `at` None, parse it into scratch; restart-interval bookkeeping first.
#[inline(always)]
fn unit(
    cx: &ScanCx,
    d: &mut Coder,
    m: &mut Model,
    restart_left: &mut u32,
    at: Option<(u32, u32)>,
    views: &mut [View],
    scratch: &mut [i16; 64],
) -> std::result::Result<(), Broken> {
    let (s, pl, f) = (cx.pl.scan, cx.pl, &cx.p.frame);
    let ri = s.restart_interval;
    if ri > 0 {
        if *restart_left == 0 {
            *d = d.restarted();
            m.reset();
            *restart_left = ri;
        }
        *restart_left -= 1;
    }
    for (slot, sc) in s.comps.iter().enumerate() {
        let fc = &f.comps[sc.ci];
        let (nh, nv) = if pl.interleaved { (fc.h, fc.v) } else { (1, 1) };
        for bv in 0..nv {
            for bh in 0..nh {
                let v = &mut views[sc.ci];
                let idx = at.and_then(|(row, ux)| {
                    let (bx, by) = if pl.interleaved {
                        (ux * fc.h + bh, row * fc.v + bv)
                    } else {
                        (ux, row)
                    };
                    v.win.idx(bx, by)
                });
                block(
                    d, m, s, pl.kind, slot, sc.td, sc.ta, v, idx, cx.dc_only, scratch, cx.ss,
                    cx.se, cx.al,
                )?;
            }
        }
    }
    Ok(())
}

/// Restart intervals are independent (statistics, predictions and the coder restart at every
/// RSTn), so with restart markers the scan's rows are decoded in parallel bands, each from the
/// marker that starts the interval holding its first unit (the units before that are parsed
/// into scratch). Marker positions come from the Huffman path's parallel marker scan.
/// Returns None (decode serially) when that does not pay or the markers are out of sequence;
/// else whether the data ended early or broke off in any band.
fn decode_restart_bands(
    cx: &ScanCx,
    data: &[u8],
    end: usize,
    stores: &mut [Store],
    threads: usize,
) -> Result<Option<bool>> {
    use rayon::prelude::*;
    let (pl, s, f) = (cx.pl, cx.pl.scan, &cx.p.frame);
    let ri = s.restart_interval as u64;
    let ux = pl.units_x as u64;
    // `pl.need`, from the first stored row on (the window may start above the region).
    let rows_per = |ci: usize| if pl.interleaved { f.comps[ci].v } else { 1 };
    let first = s
        .comps
        .iter()
        .map(|sc| stores[sc.ci].win.by.start / rows_per(sc.ci))
        .fold(pl.need.start, u32::min);
    let rows = first..pl.need.end;
    if rows.is_empty() {
        return Ok(None);
    }
    let units = (rows.end - rows.start) as u64 * ux;
    // An AC refinement block's parse depends on its coefficients so far (which bins a nonzero
    // one uses), so those bands start on an interval and parse nothing they do not keep:
    // band rows in steps of `step`.
    let step = if pl.kind == Kind::AcRefine {
        (ri / gcd(ri, ux)) as u32
    } else {
        1
    };
    if rows.start % step != 0 {
        return Ok(None);
    }
    // Bands of whole rows, each at least two intervals long (bounded re-parsing at its start).
    let nb = (threads * 4)
        .min((rows.end - rows.start) as usize)
        .min((units / (2 * ri)) as usize) as u64;
    let nrows = (rows.end - rows.start) as u64;
    let mut bounds: Vec<u32> = (0..nb)
        .map(|b| rows.start + (nrows * b / nb.max(1)) as u32 / step * step)
        .chain([rows.end])
        .collect();
    bounds.dedup();
    let nb = bounds.len() - 1;
    if nb < 2 {
        return Ok(None);
    }
    let restarts = crate::sequential::find_restarts(&data[..end], s.data_start, true);
    // RSTn markers count modulo 8: one out of sequence means lost or corrupt markers, where
    // only the serial decode's resynchronisation is safe.
    if restarts
        .iter()
        .enumerate()
        .any(|(k, &q)| data[q - 1] != 0xD0 + (k & 7) as u8)
    {
        tracing::debug!(target: "jpeg", scan = pl.si, "arithmetic: restart markers out of sequence, serial decode");
        return Ok(None);
    }
    // Every band starts at the marker of the interval holding its first unit. Past the last
    // marker the serial decode reads zeros (the scan's data ended) and goes on decoding, or
    // stops where a truncated file runs out; a band cannot tell which from its own rows, so a
    // scan missing markers some band needs decodes serially.
    let last_interval = bounds[nb - 1] as u64 * ux / ri;
    if last_interval > restarts.len() as u64 {
        tracing::debug!(target: "jpeg", scan = pl.si, markers = restarts.len(), "arithmetic: restart markers missing, serial decode");
        return Ok(None);
    }
    tracing::debug!(
        target: "jpeg",
        scan = pl.si,
        bands = nb,
        restart_interval = ri,
        markers = restarts.len(),
        "arithmetic: restart-parallel"
    );
    // Split every scanned component's window into the bands' block rows.
    let mut band_views: Vec<Vec<View>> = (0..nb).map(|_| Vec::new()).collect();
    let in_scan: Vec<bool> = (0..f.comps.len())
        .map(|ci| s.comps.iter().any(|sc| sc.ci == ci))
        .collect();
    for (ci, st) in stores.iter_mut().enumerate() {
        let w = st.win.clone();
        let width = (w.bx.end - w.bx.start) as usize;
        let rows_per = rows_per(ci);
        let (mut dc, mut ac) = (&mut st.dc[..], &mut st.ac[..]);
        let mut cur = w.by.start;
        for (b, bv) in band_views.iter_mut().enumerate() {
            let (b0, b1) = if in_scan[ci] {
                let b0 = (bounds[b] * rows_per).clamp(w.by.start, w.by.end).max(cur);
                (b0, (bounds[b + 1] * rows_per).clamp(b0, w.by.end))
            } else {
                (cur, cur)
            };
            let skip = (b0 - cur) as usize * width;
            let take = (b1 - b0) as usize * width;
            let (d0, rest) = std::mem::take(&mut dc)[skip..].split_at_mut(take);
            dc = rest;
            let a0 = if ac.is_empty() {
                &mut [][..]
            } else {
                let (a0, rest) = std::mem::take(&mut ac)[skip * 64..].split_at_mut(take * 64);
                ac = rest;
                a0
            };
            cur = b1;
            bv.push(View {
                win: Window {
                    bx: w.bx.clone(),
                    by: b0..b1,
                },
                dc: d0,
                ac: a0,
            });
        }
    }
    let run = |(b, mut views): (usize, Vec<View>)| -> Result<bool> {
        let (r0, r1) = (bounds[b], bounds[b + 1]);
        let u0 = r0 as u64 * ux;
        let j0 = u0 / ri;
        // Present for every band (checked above).
        let start = match j0 {
            0 => s.data_start,
            j => restarts[j as usize - 1],
        };
        let mut d = Coder::new(data, start, end);
        let mut m = Model::new(s, pl.kind);
        m.reset();
        let mut restart_left = ri as u32;
        let mut scratch = [0i16; 64];
        for _ in j0 * ri..u0 {
            if unit(
                cx,
                &mut d,
                &mut m,
                &mut restart_left,
                None,
                &mut views,
                &mut scratch,
            )
            .is_err()
            {
                return Ok(true);
            }
        }
        decode_rows(cx, d, &mut m, restart_left, r0..r1, &mut views, None)
    };
    let cut = band_views
        .into_iter()
        .enumerate()
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(run)
        .collect::<Result<Vec<bool>>>()?;
    Ok(Some(cut.into_iter().any(|c| c)))
}

/// Per-scan progress of a pipelined decode (in MCU rows) and the wake-up for scans waiting on it.
struct Progress {
    done: Vec<AtomicU32>,
    lock: Mutex<()>,
    cv: Condvar,
}

impl Progress {
    /// Block until scan `j` has finished MCU row `k`.
    fn wait(&self, j: usize, k: u32) {
        if self.done[j].load(Ordering::Acquire) > k {
            return;
        }
        let mut g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        while self.done[j].load(Ordering::Acquire) <= k {
            g = self.cv.wait(g).unwrap_or_else(|e| e.into_inner());
        }
    }

    fn publish(&self, j: usize, rows: u32) {
        self.done[j].store(rows, Ordering::Release);
        // Taken after the store so a waiter that saw the old value is already waiting.
        drop(self.lock.lock().unwrap_or_else(|e| e.into_inner()));
        self.cv.notify_all();
    }
}

/// Marks a scan finished however its worker ends (cut short, error, panic), so later scans
/// waiting on it never hang.
struct Finish<'a>(&'a Progress, usize);

impl Drop for Finish<'_> {
    fn drop(&mut self) {
        self.0.publish(self.1, u32::MAX);
    }
}

/// A progressive frame without an index: every scan is a separate entropy-coded segment
/// (its byte range known from the marker parse), so all scans decode at once, each on its own
/// worker, pipelined by MCU row. A scan touches MCU row `k` of a component's DC (DC scans) or
/// AC coefficients (AC scans) only once the previous scan touching the same coefficients of
/// that component has finished row `k`, so every block sees exactly the history the serial
/// order gives it: the output is bit-identical. Scans writing disjoint coefficients (DC vs AC,
/// different components) do not wait for each other at all. Coefficient rows are handed over
/// through per-row locks (uncontended in this order).
///
/// With an index (a region decode), every scan starts from its checkpoint at the region's first
/// MCU row (`need.start`) and the stored windows start there too; rows above it are never
/// touched, so the pipeline simply starts at that row.
///
/// Returns None (decode serially) when it does not apply: fewer than two scans to decode,
/// restart markers (their scans take the band-parallel path), an unexpected window.
fn decode_pipelined(
    p: &Parsed,
    data: &[u8],
    plans: &[ScanPlan],
    stores: &mut [Store],
    dc_only: bool,
    ap: &ArithParams,
) -> Result<Option<bool>> {
    let f = &p.frame;
    let plans: Vec<&ScanPlan> = plans.iter().filter(|pl| !pl.need.is_empty()).collect();
    if plans.len() < 2
        || plans.iter().any(|pl| {
            pl.scan.restart_interval > 0
                || pl
                    .scan
                    .comps
                    .iter()
                    .enumerate()
                    .any(|(i, a)| pl.scan.comps[..i].iter().any(|b| b.ci == a.ci))
        })
    {
        return Ok(None);
    }
    // The first stored MCU row: 0, or the region's first row when decoding from an index.
    let base = stores[0].win.by.start / f.comps[0].v;
    let first_row = |pl: &ScanPlan| {
        if pl.interleaved {
            base
        } else {
            base * f.comps[pl.scan.comps[0].ci].v
        }
    };
    if stores
        .iter()
        .zip(&f.comps)
        .any(|(st, c)| st.win.by.start != base * c.v || st.win.by.end % c.v != 0)
        || plans
            .iter()
            .any(|pl| pl.need.start != first_row(pl) || (base > 0 && ap.index.is_none()))
    {
        return Ok(None);
    }
    let is_dc = |pl: &ScanPlan| matches!(pl.kind, Kind::DcFirst | Kind::DcRefine);
    // Per plan: the earlier plans it waits for (the last one touching the same coefficients
    // of each of its components).
    let mut last = vec![[None::<usize>; 2]; f.comps.len()];
    let deps: Vec<Vec<usize>> = plans
        .iter()
        .enumerate()
        .map(|(j, pl)| {
            let mut d = Vec::new();
            for sc in &pl.scan.comps {
                let l = &mut last[sc.ci][is_dc(pl) as usize];
                if let Some(i) = *l
                    && !d.contains(&i)
                {
                    d.push(i);
                }
                *l = Some(j);
            }
            d
        })
        .collect();
    let order = claim_order(&plans, &deps);
    // Coefficient rows per component and MCU row.
    type Rows<'s> = Vec<Vec<Mutex<&'s mut [i16]>>>;
    let wins: Vec<Range<u32>> = stores.iter().map(|st| st.win.bx.clone()).collect();
    let mut dc_rows: Rows = Vec::new();
    let mut ac_rows: Rows = Vec::new();
    for (st, c) in stores.iter_mut().zip(&f.comps) {
        let w = (st.win.bx.end - st.win.bx.start) as usize * c.v as usize;
        dc_rows.push(st.dc.chunks_mut(w.max(1)).map(Mutex::new).collect());
        ac_rows.push(st.ac.chunks_mut(w.max(1) * 64).map(Mutex::new).collect());
    }
    let progress = Progress {
        done: plans.iter().map(|_| AtomicU32::new(base)).collect(),
        lock: Mutex::new(()),
        cv: Condvar::new(),
    };
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<(usize, Result<bool>)>> = Mutex::new(Vec::new());
    tracing::debug!(
        target: "jpeg",
        scans = plans.len(),
        workers = ap.threads.min(plans.len()),
        "arithmetic: scans pipelined"
    );

    let run = |j: usize| -> Result<bool> {
        let _finish = Finish(&progress, j);
        let pl = plans[j];
        let s = pl.scan;
        let end = s.data_end.min(data.len());
        let cx = ScanCx {
            p,
            pl,
            dc_only,
            cancel: ap.cancel,
            open_end: end >= data.len(),
            ss: s.ss.max(1) as usize,
            se: (s.se as usize).min(63),
            al: s.al,
        };
        let rows = if is_dc(pl) { &dc_rows } else { &ac_rows };
        let mut m = Model::new(s, pl.kind);
        let (mut d, mut restart_left) = match ap.index {
            Some(ix) if pl.need.start > 0 => {
                m.restore(&ix[pl.si][pl.need.start as usize], data, end)?
            }
            _ => {
                m.reset();
                (Coder::new(data, s.data_start, end), 0)
            }
        };
        let mut scratch = [0i16; 64];
        let cv0 = if pl.interleaved {
            1
        } else {
            f.comps[s.comps[0].ci].v
        };
        let mut published = base;
        for row in pl.need.clone() {
            if let Some(c) = cx.cancel
                && c.load(Ordering::Relaxed)
            {
                return Err(JpegError::Cancelled);
            }
            let k = row / cv0;
            for &i in &deps[j] {
                progress.wait(i, k);
            }
            let mut guards: Vec<_> = s
                .comps
                .iter()
                .map(|sc| {
                    rows[sc.ci]
                        .get((k - base) as usize)
                        .map(|r| (sc.ci, r.lock().unwrap_or_else(|e| e.into_inner())))
                })
                .collect();
            let mut views: Vec<View> = wins
                .iter()
                .map(|bx| View {
                    win: Window {
                        bx: bx.clone(),
                        by: 0..0,
                    },
                    dc: &mut [],
                    ac: &mut [],
                })
                .collect();
            for (ci, g) in guards.iter_mut().flatten() {
                let v = &mut views[*ci];
                let cv = f.comps[*ci].v;
                v.win.by = k * cv..(k + 1) * cv;
                if is_dc(pl) {
                    v.dc = &mut g[..];
                } else {
                    v.ac = &mut g[..];
                }
            }
            for ux in 0..pl.units_x {
                if unit(
                    &cx,
                    &mut d,
                    &mut m,
                    &mut restart_left,
                    Some((row, ux)),
                    &mut views,
                    &mut scratch,
                )
                .is_err()
                {
                    tracing::debug!(target: "jpeg", scan = pl.si, row, "arithmetic: corrupt data, scan stopped");
                    return Ok(true);
                }
            }
            drop(views);
            drop(guards);
            let finished = (row + 1) / cv0;
            if finished > published {
                published = finished;
                progress.publish(j, finished);
            }
            if cx.open_end && d.filled() > CUT_FILL {
                tracing::debug!(target: "jpeg", scan = pl.si, row, "arithmetic: data ends early");
                return Ok(true);
            }
        }
        Ok(false)
    };
    // Scans are claimed in a topological order, so every scan a claimed one waits for is
    // already running or done: no deadlock however few workers the pool runs.
    let worker = || {
        while let Some(&j) = order.get(next.fetch_add(1, Ordering::Relaxed)) {
            let r = run(j);
            results
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push((j, r));
        }
    };
    rayon::scope(|sc| {
        for _ in 1..ap.threads.min(plans.len()) {
            sc.spawn(|_| worker());
        }
        worker();
    });
    let mut results = results.into_inner().unwrap_or_else(|e| e.into_inner());
    results.sort_by_key(|r| r.0);
    let mut cut = false;
    for (_, r) in results {
        cut |= r?;
    }
    Ok(Some(cut))
}

/// The order in which pipeline workers claim scans: a topological order of `deps` (so every
/// scan a claimed one waits for has been claimed before it) that starts the longest chains
/// first (cost estimated by entropy-coded bytes), so that with fewer workers than scans the
/// scan bounding the decode (typically the luma's last AC refinement) is not left until last.
fn claim_order(plans: &[&ScanPlan], deps: &[Vec<usize>]) -> Vec<usize> {
    let n = plans.len();
    let mut succ: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut indeg: Vec<usize> = vec![0; n];
    for (j, d) in deps.iter().enumerate() {
        indeg[j] = d.len();
        for &i in d {
            succ[i].push(j);
        }
    }
    // Successors come later in file order, so one backward pass gives every chain length.
    let mut chain: Vec<u64> = vec![0; n];
    for j in (0..n).rev() {
        let own = plans[j]
            .scan
            .data_end
            .saturating_sub(plans[j].scan.data_start) as u64;
        chain[j] = own + succ[j].iter().map(|&k| chain[k]).max().unwrap_or(0);
    }
    let mut ready: std::collections::BinaryHeap<(u64, std::cmp::Reverse<usize>)> = (0..n)
        .filter(|&j| indeg[j] == 0)
        .map(|j| (chain[j], std::cmp::Reverse(j)))
        .collect();
    let mut order = Vec::with_capacity(n);
    while let Some((_, std::cmp::Reverse(j))) = ready.pop() {
        order.push(j);
        for &k in &succ[j] {
            indeg[k] -= 1;
            if indeg[k] == 0 {
                ready.push((chain[k], std::cmp::Reverse(k)));
            }
        }
    }
    order
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// Decode one block of a scan into the view (index `idx`) or, outside the stored window,
/// into `scratch` (sequential scans only need the parse).
#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn block(
    d: &mut Coder,
    m: &mut Model,
    s: &Scan,
    kind: Kind,
    slot: usize,
    td: usize,
    ta: usize,
    st: &mut View,
    idx: Option<usize>,
    dc_only: bool,
    scratch: &mut [i16; 64],
    ss: usize,
    se: usize,
    al: u8,
) -> std::result::Result<(), Broken> {
    let (l, u, kx) = (
        s.arith.dc_l[td],
        s.arith.dc_u[td],
        s.arith.ac_k[ta] as usize,
    );
    match kind {
        Kind::Seq | Kind::DcFirst => {
            let diff = dc_diff(d, m, slot, td, l, u)?;
            let v = if m.predict {
                m.dc_pred[slot].wrapping_add(diff)
            } else {
                diff
            };
            m.dc_pred[slot] = v;
            let dcv = (v.wrapping_shl(al as u32 * (kind == Kind::DcFirst) as u32)) as i16;
            if let Some(i) = idx {
                st.dc[i] = dcv;
            }
            if kind == Kind::Seq {
                let ac = &mut m.ac[ta];
                match idx {
                    Some(i) if !dc_only => {
                        let b = &mut st.ac[i * 64..i * 64 + 64];
                        ac_first(d, ac, b, 1, 63, 0, kx)?;
                    }
                    _ => {
                        ac_first(d, ac, scratch, 1, 63, 0, kx)?;
                        *scratch = [0; 64];
                    }
                }
            }
        }
        Kind::DcRefine => {
            if d.bit()
                && let Some(i) = idx
            {
                st.dc[i] |= (1i32 << al) as i16;
            }
        }
        Kind::AcFirst | Kind::AcRefine => {
            let ac = &mut m.ac[ta];
            let b: &mut [i16] = match idx {
                Some(i) if !st.ac.is_empty() => &mut st.ac[i * 64..i * 64 + 64],
                _ => {
                    *scratch = [0; 64];
                    &mut scratch[..]
                }
            };
            if kind == Kind::AcFirst {
                ac_first(d, ac, b, ss, se, al, kx)?;
            } else {
                ac_refine(d, ac, b, ss, se, al)?;
            }
        }
    }
    Ok(())
}
