//! IW44 wavelet images (DjVu v3 specification, appendix 1): `BG44`, `FG44` and `TH44`
//! chunks, colour (Y, Cb, Cr) or grey.
//!
//! Coefficients arrive progressively: each slice refines one band of every 32×32 block, one
//! bit-plane at a time, across any number of chunks of the same layer. The image is the
//! inverse Dubuc–Deslauriers–Lemire (4, 4) lifting transform of the coefficients, applied
//! from the coarsest scale (16) to the finest, then YCbCr to RGB.

use crate::Error;
use crate::zp::Zp;

/// Most pixels of one IW44 layer.
pub const MAX_PIXELS: u64 = 1 << 28;

/// First bucket and bucket count of each band.
const BAND_BUCKETS: [(usize, usize); 10] = [
    (0, 1),
    (1, 1),
    (2, 1),
    (3, 1),
    (4, 4),
    (8, 4),
    (12, 4),
    (16, 16),
    (32, 16),
    (48, 16),
];

/// Initial step sizes.
const IW_QUANT: [i32; 16] = [
    0x004000, 0x008000, 0x008000, 0x010000, 0x010000, 0x010000, 0x020000, 0x020000, 0x020000,
    0x040000, 0x040000, 0x040000, 0x080000, 0x040000, 0x040000, 0x080000,
];

const ZERO: u8 = 1;
const ACTIVE: u8 = 2;
const NEW: u8 = 4;
const UNK: u8 = 8;

/// Position (row-major in the 32×32 block, row 0 at the bottom) of coefficient `i`.
fn zigzag(i: usize) -> usize {
    let b = |k: usize| (i >> k) & 1;
    let x = (b(0) << 4) | (b(2) << 3) | (b(4) << 2) | (b(6) << 1) | b(8);
    let y = (b(1) << 4) | (b(3) << 3) | (b(5) << 2) | (b(7) << 1) | b(9);
    y * 32 + x
}

/// The coefficients of one 32×32 block, by bucket; a bucket exists once something has been
/// decoded into it. The coefficients are allocated with the first one decoded: the header
/// alone (9 bytes) may declare 2^28 pixels, 1.5 GB of blocks for the three components.
#[derive(Clone)]
struct Block {
    coeff: Option<Box<[i16; 1024]>>,
    present: u64,
}

impl Block {
    fn new() -> Block {
        Block {
            coeff: None,
            present: 0,
        }
    }
    fn has(&self, bucket: usize) -> bool {
        self.present & (1 << bucket) != 0
    }
    fn bucket(&self, bucket: usize) -> &[i16] {
        match &self.coeff {
            Some(c) => &c[bucket * 16..bucket * 16 + 16],
            None => &[0; 16],
        }
    }
    fn get(&self, i: usize) -> i16 {
        self.coeff.as_ref().map_or(0, |c| c[i])
    }
    fn set(&mut self, i: usize, v: i16) {
        self.coeff.get_or_insert_with(|| Box::new([0; 1024]))[i] = v;
    }
}

/// The decoder state of one colour component.
struct Codec {
    blocks: Vec<Block>,
    quant_lo: [i32; 16],
    quant_hi: [i32; 10],
    band: usize,
    /// Bit-plane; `None` once every step size is zero.
    bit: Option<u32>,
    ctx_start: [u8; 32],
    ctx_bucket: [[u8; 8]; 10],
    ctx_mant: u8,
    ctx_root: u8,
    coeff_state: [u8; 256],
    bucket_state: [u8; 16],
}

// Coefficients, their states and step sizes are parallel arrays indexed together.
#[allow(clippy::needless_range_loop)]
impl Codec {
    fn new(nblocks: usize) -> Codec {
        let q = &IW_QUANT;
        let mut quant_lo = [0; 16];
        quant_lo[..4].copy_from_slice(&q[..4]);
        quant_lo[4..8].fill(q[4]);
        quant_lo[8..12].fill(q[5]);
        quant_lo[12..16].fill(q[6]);
        let mut quant_hi = [0; 10];
        quant_hi[1..].copy_from_slice(&q[7..16]);
        Codec {
            blocks: vec![Block::new(); nblocks],
            quant_lo,
            quant_hi,
            band: 0,
            bit: Some(0),
            ctx_start: [0; 32],
            ctx_bucket: [[0; 8]; 10],
            ctx_mant: 0,
            ctx_root: 0,
            coeff_state: [0; 256],
            bucket_state: [0; 16],
        }
    }

    fn is_null_slice(&mut self) -> bool {
        if self.band == 0 {
            let mut null = true;
            for i in 0..16 {
                let t = self.quant_lo[i];
                self.coeff_state[i] = ZERO;
                if t > 0 && t < 0x8000 {
                    self.coeff_state[i] = UNK;
                    null = false;
                }
            }
            null
        } else {
            let t = self.quant_hi[self.band];
            !(t > 0 && t < 0x8000)
        }
    }

    /// Decodes one slice of this component; false once nothing is left to code.
    fn slice(&mut self, zp: &mut Zp<'_>) -> bool {
        if self.bit.is_none() {
            return false;
        }
        if !self.is_null_slice() {
            let (fbucket, nbucket) = BAND_BUCKETS[self.band];
            for b in 0..self.blocks.len() {
                self.buckets(zp, b, fbucket, nbucket);
            }
        }
        // Finish the slice: halve this band's step sizes, move to the next band.
        self.quant_hi[self.band] >>= 1;
        if self.band == 0 {
            for q in &mut self.quant_lo {
                *q >>= 1;
            }
        }
        self.band += 1;
        if self.band >= BAND_BUCKETS.len() {
            self.band = 0;
            self.bit = self.bit.map(|b| b + 1);
            if self.quant_hi[BAND_BUCKETS.len() - 1] == 0 {
                self.bit = None;
                return false;
            }
        }
        true
    }

    fn prepare(&mut self, b: usize, fbucket: usize, nbucket: usize) -> u8 {
        let blk = &self.blocks[b];
        let mut bbstate = 0;
        if fbucket != 0 {
            for buck in 0..nbucket {
                let mut bs = 0;
                if blk.has(fbucket + buck) {
                    let c = blk.bucket(fbucket + buck);
                    for i in 0..16 {
                        let cs = if c[i] != 0 { ACTIVE } else { UNK };
                        self.coeff_state[buck * 16 + i] = cs;
                        bs |= cs;
                    }
                } else {
                    bs = UNK;
                }
                self.bucket_state[buck] = bs;
                bbstate |= bs;
            }
        } else {
            if blk.has(0) {
                let c = blk.bucket(0);
                for i in 0..16 {
                    let mut cs = self.coeff_state[i];
                    if cs != ZERO {
                        cs = if c[i] != 0 { ACTIVE } else { UNK };
                    }
                    self.coeff_state[i] = cs;
                    bbstate |= cs;
                }
            } else {
                bbstate = UNK;
            }
            self.bucket_state[0] = bbstate;
        }
        bbstate
    }

    fn buckets(&mut self, zp: &mut Zp<'_>, b: usize, fbucket: usize, nbucket: usize) {
        let band = self.band;
        let mut bbstate = self.prepare(b, fbucket, nbucket);
        // The root decision is coded only for a full block band with nothing active yet.
        if nbucket < 16
            || bbstate & ACTIVE != 0
            || (bbstate & UNK != 0 && zp.decode(&mut self.ctx_root))
        {
            bbstate |= NEW;
        }
        // Bucket decisions.
        if bbstate & NEW != 0 {
            for buck in 0..nbucket {
                if self.bucket_state[buck] & UNK != 0 {
                    let mut ctx = 0;
                    if band > 0 {
                        let k = (fbucket + buck) << 2;
                        let blk = &self.blocks[b];
                        if blk.has(k >> 4) {
                            let c = blk.bucket(k >> 4);
                            let k = k & 0xf;
                            ctx += usize::from(c[k] != 0);
                            ctx += usize::from(c[k + 1] != 0);
                            ctx += usize::from(c[k + 2] != 0);
                            if ctx < 3 && c[k + 3] != 0 {
                                ctx += 1;
                            }
                        }
                    }
                    if bbstate & ACTIVE != 0 {
                        ctx |= 4;
                    }
                    if zp.decode(&mut self.ctx_bucket[band][ctx]) {
                        self.bucket_state[buck] |= NEW;
                    }
                }
            }
        }
        // Newly active coefficients and their signs.
        if bbstate & NEW != 0 {
            let mut thres = self.quant_hi[band];
            for buck in 0..nbucket {
                if self.bucket_state[buck] & NEW == 0 {
                    continue;
                }
                let cs = &mut self.coeff_state[buck * 16..buck * 16 + 16];
                let blk = &mut self.blocks[b];
                let bi = fbucket + buck;
                if !blk.has(bi) {
                    blk.present |= 1 << bi;
                    if fbucket == 0 {
                        for s in cs.iter_mut() {
                            if *s != ZERO {
                                *s = UNK;
                            }
                        }
                    } else {
                        cs.fill(UNK);
                    }
                }
                let mut gotcha = cs.iter().filter(|&&s| s & UNK != 0).count();
                for i in 0..16 {
                    if cs[i] & UNK == 0 {
                        continue;
                    }
                    if band == 0 {
                        thres = self.quant_lo[i];
                    }
                    let mut ctx = gotcha.min(7);
                    if self.bucket_state[buck] & ACTIVE != 0 {
                        ctx |= 8;
                    }
                    if zp.decode(&mut self.ctx_start[ctx]) {
                        cs[i] |= NEW;
                        let half = thres >> 1;
                        let coeff = thres + half - (half >> 2);
                        let v = if zp.decode_raw() { -coeff } else { coeff };
                        blk.set(bi * 16 + i, v as i16);
                    }
                    if cs[i] & NEW != 0 {
                        gotcha = 0;
                    } else {
                        gotcha = gotcha.saturating_sub(1);
                    }
                }
            }
        }
        // Refinement of already active coefficients.
        if bbstate & ACTIVE != 0 {
            let mut thres = self.quant_hi[band];
            for buck in 0..nbucket {
                if self.bucket_state[buck] & ACTIVE == 0 {
                    continue;
                }
                let bi = fbucket + buck;
                for i in 0..16 {
                    if self.coeff_state[buck * 16 + i] & ACTIVE == 0 {
                        continue;
                    }
                    let stored = i32::from(self.blocks[b].get(bi * 16 + i));
                    let mut coeff = stored.abs();
                    if band == 0 {
                        thres = self.quant_lo[i];
                    }
                    if coeff <= 3 * thres {
                        coeff += thres >> 2;
                        if zp.decode(&mut self.ctx_mant) {
                            coeff += thres >> 1;
                        } else {
                            coeff = coeff - thres + (thres >> 1);
                        }
                    } else if zp.decode_raw() {
                        coeff += thres >> 1;
                    } else {
                        coeff = coeff - thres + (thres >> 1);
                    }
                    let v = if stored > 0 { coeff } else { -coeff };
                    self.blocks[b].set(bi * 16 + i, v as i16);
                }
            }
        }
    }

    /// Reconstructs the component: `w`×`h` signed samples, rows bottom-up. `half` stops one
    /// scale early and replicates 2×2 (half-resolution chrominance).
    fn image(&self, w: usize, h: usize, half: bool) -> Vec<i8> {
        let bw = w.div_ceil(32) * 32;
        let bh = h.div_ceil(32) * 32;
        let mut data = vec![0i16; bw * bh];
        let zz: Vec<usize> = (0..1024).map(zigzag).collect();
        let nbx = bw / 32;
        for (n, blk) in self.blocks.iter().enumerate() {
            let Some(coeff) = &blk.coeff else {
                continue;
            };
            let (bx, by) = (n % nbx, n / nbx);
            let base = by * 32 * bw + bx * 32;
            for (i, &loc) in zz.iter().enumerate() {
                if blk.has(i >> 4) {
                    let v = coeff[i];
                    if v != 0 {
                        data[base + (loc >> 5) * bw + (loc & 31)] = v;
                    }
                }
            }
        }
        let last = if half { 2 } else { 1 };
        let mut scale = 16;
        let mut tmp = Vec::new();
        while scale >= last {
            // Vertical: every `scale`-th column, samples every `scale` rows.
            let n = (h - 1) / scale + 1;
            for x in (0..w).step_by(scale) {
                tmp.clear();
                tmp.extend((0..n).map(|k| data[k * scale * bw + x]));
                lift(&mut tmp);
                for (k, &v) in tmp.iter().enumerate() {
                    data[k * scale * bw + x] = v;
                }
            }
            // Horizontal: every `scale`-th row.
            let n = (w - 1) / scale + 1;
            for y in (0..h).step_by(scale) {
                let row = y * bw;
                tmp.clear();
                tmp.extend((0..n).map(|k| data[row + k * scale]));
                lift(&mut tmp);
                for (k, &v) in tmp.iter().enumerate() {
                    data[row + k * scale] = v;
                }
            }
            scale >>= 1;
        }
        let mut out = vec![0i8; w * h];
        for y in 0..h {
            let sy = if half { y & !1 } else { y };
            for x in 0..w {
                let sx = if half { x & !1 } else { x };
                let v = (i32::from(data[sy * bw + sx]) + 32) >> 6;
                out[y * w + x] = v.clamp(-128, 127) as i8;
            }
        }
        out
    }
}

/// One step of the inverse transform along a line of samples: undo the update of the even
/// samples, then predict the odd ones (4-tap inside, linear at the ends).
fn lift(v: &mut [i16]) {
    let n = v.len() as isize;
    let at = |v: &[i16], i: isize| -> i32 {
        if i >= 0 && i < n {
            i32::from(v[i as usize])
        } else {
            0
        }
    };
    let mut x = 0;
    while x < n {
        let a = at(v, x - 1) + at(v, x + 1);
        let b = at(v, x - 3) + at(v, x + 3);
        let d = ((a << 3) + a - b + 16) >> 5;
        v[x as usize] = (i32::from(v[x as usize]) - d) as i16;
        x += 2;
    }
    let mut o = 1;
    while o < n {
        let d = if o >= 3 && o + 3 < n {
            let a = at(v, o - 1) + at(v, o + 1);
            let b = at(v, o - 3) + at(v, o + 3);
            ((a << 3) + a - b + 8) >> 4
        } else {
            let a = at(v, o - 1)
                + if o + 1 < n {
                    at(v, o + 1)
                } else {
                    at(v, o - 1)
                };
            (a + 1) >> 1
        };
        v[o as usize] = (i32::from(v[o as usize]) + d) as i16;
        o += 2;
    }
}

/// A decoded IW44 picture: RGB or grey, rows top-down.
#[derive(Debug, Clone)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    /// 3 bytes per pixel (RGB) or 1 (grey).
    pub channels: u8,
    pub data: Vec<u8>,
}

impl Picture {
    /// The RGB value at `(x, y)` (top-down), clamped to the picture.
    #[inline]
    pub fn rgb(&self, x: u32, y: u32) -> [u8; 3] {
        let x = x.min(self.width - 1) as usize;
        let y = y.min(self.height - 1) as usize;
        let i = y * self.width as usize + x;
        if self.channels == 3 {
            [self.data[i * 3], self.data[i * 3 + 1], self.data[i * 3 + 2]]
        } else {
            let g = self.data[i];
            [g, g, g]
        }
    }
}

/// An IW44 layer being decoded, fed its chunks in order.
pub struct Layer {
    width: usize,
    height: usize,
    crcb_delay: usize,
    crcb_half: bool,
    y: Codec,
    cbcr: Option<(Codec, Codec)>,
    slice: usize,
    serial: usize,
    /// The memory budget's share of the layer: its coefficients at most, and the peak of
    /// [`Layer::picture`].
    _held: specular_core::Reservation,
}

/// Facts from the first chunk's header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub width: u32,
    pub height: u32,
    pub color: bool,
    pub slices: u8,
}

/// Reads the header of a layer's first chunk.
pub fn header(data: &[u8]) -> Option<Header> {
    if data.len() < 9 || data[0] != 0 {
        return None;
    }
    Some(Header {
        width: u32::from(u16::from_be_bytes([data[4], data[5]])),
        height: u32::from(u16::from_be_bytes([data[6], data[7]])),
        color: data[2] & 0x80 == 0,
        slices: data[1],
    })
}

impl Layer {
    /// Starts a layer from its first chunk.
    pub fn new(first: &[u8]) -> Result<Layer, Error> {
        if first.len() < 2 || first[0] != 0 {
            return Err(Error::Malformed(
                "IW44 layer does not start with its first chunk",
            ));
        }
        if first.len() < 8 {
            return Err(Error::Malformed("IW44 header"));
        }
        let major = first[2];
        let minor = first[3];
        if major & 0x7f != 1 {
            return Err(Error::Unsupported("IW44 version other than 1"));
        }
        let w = usize::from(u16::from_be_bytes([first[4], first[5]]));
        let h = usize::from(u16::from_be_bytes([first[6], first[7]]));
        if w == 0 || h == 0 {
            return Err(Error::Malformed("IW44 image without pixels"));
        }
        if (w as u64) * (h as u64) > MAX_PIXELS {
            return Err(Error::TooLarge);
        }
        let delay = if minor >= 2 {
            *first.get(8).ok_or(Error::Malformed("IW44 header"))?
        } else {
            0
        };
        let nblocks = w.div_ceil(32) * h.div_ceil(32);
        let color = major & 0x80 == 0;
        // Reserved before anything is decoded: a header of a few bytes declares the size.
        // Per component 2 bytes of coefficients per block pixel and 1 byte of samples; while
        // a picture is made, one 2-byte working plane and the 1- or 3-byte output.
        let comps: u64 = if color { 3 } else { 1 };
        let block_px = nblocks as u64 * 1024;
        let px = (w * h) as u64;
        let need = comps * (2 * block_px + px) + 2 * block_px + comps * px;
        let held = specular_core::budget::reserve(need)?;
        Ok(Layer {
            width: w,
            height: h,
            crcb_delay: usize::from(delay & 0x7f),
            crcb_half: delay & 0x80 == 0,
            y: Codec::new(nblocks),
            cbcr: color.then(|| (Codec::new(nblocks), Codec::new(nblocks))),
            slice: 0,
            serial: 0,
            _held: held,
        })
    }

    /// Size of the picture.
    pub fn size(&self) -> (u32, u32) {
        (self.width as u32, self.height as u32)
    }

    /// Blocks holding coefficients, all components.
    #[cfg(test)]
    pub(crate) fn allocated_blocks(&self) -> usize {
        let n = |c: &Codec| c.blocks.iter().filter(|b| b.coeff.is_some()).count();
        n(&self.y) + self.cbcr.as_ref().map_or(0, |(b, r)| n(b) + n(r))
    }

    /// Memory the coefficients take.
    pub fn memory_bytes(&self) -> usize {
        let n = self.y.blocks.len() * 2048;
        n * if self.cbcr.is_some() { 3 } else { 1 }
    }

    /// Decodes the next chunk of the layer.
    pub fn chunk(&mut self, data: &[u8], cancel: &dyn Fn() -> bool) -> Result<(), Error> {
        if data.len() < 2 {
            return Err(Error::Malformed("IW44 chunk"));
        }
        if usize::from(data[0]) != self.serial {
            return Err(Error::Malformed("IW44 chunks out of order"));
        }
        self.serial += 1;
        let nslices = usize::from(data[1]);
        let body = if data[0] == 0 {
            let minor = data.get(3).copied().unwrap_or(0);
            let n = if minor >= 2 { 9 } else { 8 };
            data.get(n..).ok_or(Error::Malformed("IW44 header"))?
        } else {
            &data[2..]
        };
        let mut zp = Zp::new(body);
        let end = self.slice + nslices;
        let mut more = true;
        while more && self.slice < end {
            if cancel() {
                return Err(Error::Cancelled);
            }
            more = self.y.slice(&mut zp);
            if let Some((cb, cr)) = &mut self.cbcr
                && self.crcb_delay <= self.slice
            {
                more |= cb.slice(&mut zp);
                more |= cr.slice(&mut zp);
            }
            self.slice += 1;
            if zp.overrun() > 4096 {
                // Truncated data: keep what was decoded.
                break;
            }
        }
        Ok(())
    }

    /// The picture as decoded so far.
    pub fn picture(&self) -> Picture {
        let (w, h) = (self.width, self.height);
        let y = self.y.image(w, h, false);
        let mut data;
        let channels;
        if let Some((cb, cr)) = &self.cbcr {
            let cb = cb.image(w, h, self.crcb_half);
            let cr = cr.image(w, h, self.crcb_half);
            channels = 3;
            data = vec![0u8; w * h * 3];
            for row in 0..h {
                let src = (h - 1 - row) * w;
                let dst = row * w * 3;
                for x in 0..w {
                    let (yy, b, r) = (
                        i32::from(y[src + x]),
                        i32::from(cb[src + x]),
                        i32::from(cr[src + x]),
                    );
                    let t1 = b >> 2;
                    let t2 = r + (r >> 1);
                    let t3 = yy + 128 - t1;
                    let tr = yy + 128 + t2;
                    let tg = t3 - (t2 >> 1);
                    let tb = t3 + (b << 1);
                    let o = dst + x * 3;
                    data[o] = tr.clamp(0, 255) as u8;
                    data[o + 1] = tg.clamp(0, 255) as u8;
                    data[o + 2] = tb.clamp(0, 255) as u8;
                }
            }
        } else {
            channels = 1;
            data = vec![0u8; w * h];
            for row in 0..h {
                let src = (h - 1 - row) * w;
                for x in 0..w {
                    data[row * w + x] = (127 - i32::from(y[src + x])) as u8;
                }
            }
        }
        Picture {
            width: w as u32,
            height: h as u32,
            channels,
            data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zigzag_matches_the_band_layout() {
        assert_eq!(zigzag(0), 0);
        assert_eq!(zigzag(1), 16);
        assert_eq!(zigzag(2), 512);
        assert_eq!(zigzag(3), 528);
        assert_eq!(zigzag(4), 8);
        assert_eq!(zigzag(12), 264);
        assert_eq!(zigzag(16), 4);
        assert_eq!(zigzag(32), 128);
        // A permutation of the block.
        let mut seen = vec![false; 1024];
        for i in 0..1024 {
            seen[zigzag(i)] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn lift_of_zero_is_zero_and_constant_stays_constant() {
        let mut v = vec![0i16; 9];
        lift(&mut v);
        assert!(v.iter().all(|&x| x == 0));
    }
}
