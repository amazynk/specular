//! ETC1S with BasisLZ supercompression (KTX2 `supercompressionScheme` 1).
//!
//! Written from the KTX 2.0 specification (BasisLZ global data, BasisLZ/ETC1S bitstream) and
//! the Khronos Data Format specification (ETC1). The global data holds two Huffman+DPCM coded
//! codebooks (endpoints: 5:5:5 base colour + intensity table; selectors: 16 2-bit texel
//! selectors) and four Huffman tables shared by every slice. A slice codes one codebook index
//! pair per block: endpoint indices through 2x2-grouped left/up/up-left/delta predictors,
//! selector indices through an approximate move-to-front history buffer with run lengths.
//! P-frames (texture video) reuse the previous frame's pair per block.
//!
//! [`Etc1sGlobal::transcode`] turns a slice into standard ETC1 blocks (differential mode, zero
//! delta, no flip, both sub-blocks on the same intensity table), which is exactly ETC1S;
//! [`decode_block`] expands those to RGBA8.

use crate::{Error, Result};

/// Image descriptor flag: the image is a P-frame of an animation sequence.
const IS_P_FRAME: u32 = 0x02;

/// ETC1 intensity modifier tables (a, b): texel = base + {-b, -a, +a, +b}[selector].
const INTEN: [[i32; 2]; 8] = [
    [2, 8],
    [5, 17],
    [9, 29],
    [13, 42],
    [18, 60],
    [24, 80],
    [33, 106],
    [47, 183],
];

/// Order of the code-length code lengths in a compressed Huffman table.
const CODELENGTH_ORDER: [usize; 21] = [
    17, 18, 19, 20, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15, 16,
];

const ENDPOINT_PRED_REPEAT: u32 = 256;
const SELECTOR_RLE_LONG: u32 = 63;

// ------------------------------------------------------------------------------- bit reading

/// LSB-first bit reader. Reads past the end yield zeros; [`BitReader::overrun`] tells whether
/// any of them were consumed.
struct BitReader<'a> {
    data: &'a [u8],
    /// Next byte to load (may run past `data.len()` while feeding zeros).
    pos: usize,
    bits: u64,
    count: u32,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        BitReader {
            data,
            pos: 0,
            bits: 0,
            count: 0,
        }
    }

    /// Tops the buffer up to at least 56 bits.
    #[inline(always)]
    fn refill(&mut self) {
        if let Some(chunk) = self.data.get(self.pos..self.pos + 8) {
            let v = u64::from_le_bytes(chunk.try_into().unwrap());
            self.bits |= v << self.count;
            self.pos += ((63 - self.count) >> 3) as usize;
            self.count |= 56;
        } else {
            self.refill_slow();
        }
    }

    #[cold]
    fn refill_slow(&mut self) {
        while self.count <= 56 {
            let b = self.data.get(self.pos).copied().unwrap_or(0);
            self.bits |= u64::from(b) << self.count;
            self.pos += 1;
            self.count += 8;
        }
    }

    /// Reads `n` (<= 32) bits.
    #[inline(always)]
    fn bits(&mut self, n: u32) -> u32 {
        if self.count < n {
            self.refill();
        }
        let v = (self.bits & ((1u64 << n) - 1)) as u32;
        self.bits >>= n;
        self.count -= n;
        v
    }

    /// Whether more bits were consumed than the data holds.
    fn overrun(&self) -> bool {
        self.pos * 8 - self.count as usize > self.data.len() * 8
    }

    /// Variable-length integer in chunks of `chunk` bits whose top bit means "more follows".
    fn vlc(&mut self, chunk: u32) -> u32 {
        let payload = chunk - 1;
        let mut v = 0u32;
        let mut shift = 0;
        loop {
            let s = self.bits(chunk);
            v |= (s & ((1 << payload) - 1)) << shift;
            shift += payload;
            if s >> payload == 0 || shift >= 32 {
                return v;
            }
        }
    }

    /// Decodes one Huffman symbol.
    #[inline(always)]
    fn decode(&mut self, h: &Huffman) -> Result<u32> {
        if self.count < MAX_CODE_LEN {
            self.refill();
        }
        let mut e = h.fast[(self.bits as usize) & (FAST_SIZE - 1)];
        if e & SUB_FLAG != 0 {
            let sub_bits = e & 31;
            let i = ((e & !SUB_FLAG) >> 5) as usize
                + ((self.bits >> FAST_BITS) as usize & ((1 << sub_bits) - 1));
            e = h.sub[i];
        }
        let len = e & 31;
        if len == 0 {
            return Err(Error::Malformed("basis: invalid Huffman code"));
        }
        self.bits >>= len;
        self.count -= len;
        Ok(e >> 5)
    }
}

// ------------------------------------------------------------------------------- Huffman

const MAX_CODE_LEN: u32 = 16;
const FAST_BITS: u32 = 10;
const FAST_SIZE: usize = 1 << FAST_BITS;
/// Fast-table entry flag: the entry points at a second-level table.
const SUB_FLAG: u32 = 1 << 31;

/// Canonical (Deflate-style) Huffman decoding table. Entries are `symbol << 5 | length`, 0 for
/// an unused code; codes longer than `FAST_BITS` go through a per-prefix second-level table
/// (fast entry `SUB_FLAG | offset << 5 | sub_bits`).
struct Huffman {
    fast: Box<[u32; FAST_SIZE]>,
    sub: Vec<u32>,
}

impl Huffman {
    fn from_lengths(lens: &[u8]) -> Result<Huffman> {
        let mut fast = Box::new([0u32; FAST_SIZE]);
        let mut count = [0u32; MAX_CODE_LEN as usize + 1];
        for &l in lens {
            count[l as usize] += 1;
        }
        count[0] = 0;
        let mut next = [0u32; MAX_CODE_LEN as usize + 1];
        let mut code = 0u32;
        for len in 1..=MAX_CODE_LEN as usize {
            code = (code + count[len - 1]) << 1;
            next[len] = code;
            if code + count[len] > 1 << len {
                return Err(Error::Malformed("basis: oversubscribed Huffman table"));
            }
        }
        let used: u32 = count.iter().sum();
        // Long codes: (symbol, length, bit-reversed code).
        let mut long = Vec::new();
        let mut sub_len = [0u8; FAST_SIZE];
        for (sym, &l) in lens.iter().enumerate() {
            if l == 0 {
                continue;
            }
            let len = u32::from(l);
            let rev = next[len as usize].reverse_bits() >> (32 - len);
            next[len as usize] += 1;
            let entry = (sym as u32) << 5 | len;
            if len <= FAST_BITS {
                for i in (rev as usize..FAST_SIZE).step_by(1 << len) {
                    fast[i] = entry;
                }
            } else {
                let p = rev as usize & (FAST_SIZE - 1);
                sub_len[p] = sub_len[p].max(l);
                long.push((entry, rev));
            }
        }
        let mut sub = Vec::new();
        if !long.is_empty() {
            for (p, &l) in sub_len.iter().enumerate() {
                if l != 0 {
                    let bits = u32::from(l) - FAST_BITS;
                    fast[p] = SUB_FLAG | (sub.len() as u32) << 5 | bits;
                    sub.resize(sub.len() + (1 << bits), 0);
                }
            }
            for (entry, rev) in long {
                let len = entry & 31;
                let e = fast[rev as usize & (FAST_SIZE - 1)];
                let base = ((e & !SUB_FLAG) >> 5) as usize;
                let size = 1usize << (e & 31);
                for i in ((rev >> FAST_BITS) as usize..size).step_by(1 << (len - FAST_BITS)) {
                    sub[base + i] = entry;
                }
            }
        } else if used == 1 {
            // A lone symbol: every bit pattern decodes to it.
            let e = fast.iter().copied().find(|&e| e != 0).unwrap_or(0);
            fast.fill(e);
        }
        Ok(Huffman { fast, sub })
    }

    /// Reads a compressed Huffman table (code lengths RLE + Huffman coded, like Deflate).
    fn read(r: &mut BitReader) -> Result<Huffman> {
        let total = r.bits(14) as usize;
        let n = r.bits(5) as usize;
        if n == 0 || n > 21 {
            return Err(Error::Malformed("basis: bad Huffman code length count"));
        }
        let mut cl_lens = [0u8; 21];
        for &sym in &CODELENGTH_ORDER[..n] {
            cl_lens[sym] = r.bits(3) as u8;
        }
        let cl = Huffman::from_lengths(&cl_lens)?;
        let mut lens = vec![0u8; total];
        let mut i = 0;
        while i < total {
            let s = r.decode(&cl)?;
            let (run, v) = match s {
                0..=16 => (1, s as u8),
                17 => (r.bits(3) as usize + 3, 0),
                18 => (r.bits(7) as usize + 11, 0),
                19 | 20 => {
                    let run = if s == 19 {
                        r.bits(2) as usize + 3
                    } else {
                        r.bits(7) as usize + 7
                    };
                    let prev = if i > 0 { lens[i - 1] } else { 0 };
                    if prev == 0 {
                        return Err(Error::Malformed("basis: Huffman repeat without a length"));
                    }
                    (run, prev)
                }
                _ => return Err(Error::Malformed("basis: bad Huffman code length symbol")),
            };
            if i + run > total {
                return Err(Error::Malformed("basis: Huffman code lengths overflow"));
            }
            lens[i..i + run].fill(v);
            i += run;
        }
        if r.overrun() {
            return Err(Error::Truncated("basis: Huffman table"));
        }
        Huffman::from_lengths(&lens)
    }
}

// ------------------------------------------------------------------------------- global data

/// A slice selector on [`ImageDesc`] and the channel's byte offset in each output block.
type Channel = (fn(&ImageDesc) -> (usize, usize), usize);

/// One image's slices: flags (bit 1: P-frame) and the (offset, length) of its colour and
/// alpha slices in the bytes handed to [`Etc1sGlobal::transcode`] (alpha length 0: none).
#[derive(Clone, Copy, Default)]
pub(crate) struct ImageDesc {
    pub(crate) flags: u32,
    pub(crate) rgb: (usize, usize),
    pub(crate) alpha: (usize, usize),
}

/// The BasisLZ supercompression global data of a KTX2 file: codebooks, slice Huffman tables
/// and image descriptors.
pub(crate) struct Etc1sGlobal {
    /// Per endpoint: ETC1 block bytes 0..4 (5-bit base colour in the top bits, zero delta,
    /// both intensity codewords, differential bit set, no flip).
    endpoints: Vec<[u8; 4]>,
    /// Per selector: ETC1 block bytes 4..8 (big-endian pixel index MSB / LSB planes).
    selectors: Vec<[u8; 4]>,
    endpoint_pred: Huffman,
    endpoint_delta: Huffman,
    selector: Huffman,
    selector_rle: Huffman,
    history_size: usize,
    descs: Vec<ImageDesc>,
    alpha: bool,
}

fn le32(b: &[u8], at: usize) -> usize {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap()) as usize
}

impl Etc1sGlobal {
    /// Parses the supercompression global data (`sgd`) for `image_count` images (levels x
    /// layers x faces x depth slices, level-major).
    pub(crate) fn parse(sgd: &[u8], image_count: usize) -> Result<Etc1sGlobal> {
        if sgd.len() < 20 {
            return Err(Error::Truncated("BasisLZ global data"));
        }
        let endpoint_count = usize::from(u16::from_le_bytes([sgd[0], sgd[1]]));
        let selector_count = usize::from(u16::from_le_bytes([sgd[2], sgd[3]]));
        let lens = [le32(sgd, 4), le32(sgd, 8), le32(sgd, 12), le32(sgd, 16)];
        let descs_len = image_count
            .checked_mul(20)
            .ok_or(Error::Malformed("BasisLZ image count"))?;
        let total = lens
            .iter()
            .try_fold(20 + descs_len, |a, &l| a.checked_add(l));
        if total.is_none_or(|t| t > sgd.len()) {
            return Err(Error::Truncated("BasisLZ global data"));
        }
        let descs = (0..image_count)
            .map(|i| {
                let d = &sgd[20 + i * 20..40 + i * 20];
                ImageDesc {
                    flags: le32(d, 0) as u32,
                    rgb: (le32(d, 4), le32(d, 8)),
                    alpha: (le32(d, 12), le32(d, 16)),
                }
            })
            .collect();
        let mut at = 20 + descs_len;
        let mut section = |len: usize| {
            let s = &sgd[at..at + len];
            at += len;
            s
        };
        let (ep_data, sel_data, tables) = (section(lens[0]), section(lens[1]), section(lens[2]));
        Self::from_parts(
            endpoint_count,
            selector_count,
            ep_data,
            sel_data,
            tables,
            descs,
        )
    }

    /// Builds the global data from its parts: the codebooks, the slice tables and one
    /// descriptor per image (the `.basis` file stores them apart; KTX2 in one block).
    pub(crate) fn from_parts(
        endpoint_count: usize,
        selector_count: usize,
        ep_data: &[u8],
        sel_data: &[u8],
        tables: &[u8],
        descs: Vec<ImageDesc>,
    ) -> Result<Etc1sGlobal> {
        if endpoint_count == 0 || selector_count == 0 {
            return Err(Error::Malformed("BasisLZ: empty codebook"));
        }
        let alpha = descs.iter().any(|d| d.alpha.1 != 0);
        let endpoints = decode_endpoints(ep_data, endpoint_count)?;
        let selectors = decode_selectors(sel_data, selector_count)?;

        let mut r = BitReader::new(tables);
        let endpoint_pred = Huffman::read(&mut r)?;
        let endpoint_delta = Huffman::read(&mut r)?;
        let selector = Huffman::read(&mut r)?;
        let selector_rle = Huffman::read(&mut r)?;
        let history_size = r.bits(13) as usize;
        if r.overrun() {
            return Err(Error::Truncated("BasisLZ slice tables"));
        }
        Ok(Etc1sGlobal {
            endpoints,
            selectors,
            endpoint_pred,
            endpoint_delta,
            selector,
            selector_rle,
            history_size,
            descs,
            alpha,
        })
    }

    /// Whether the images have an alpha slice (transcode output is then 16 bytes per block).
    pub(crate) fn has_alpha(&self) -> bool {
        self.alpha
    }

    /// Transcodes image `desc_index` from its mip level's bytes `level` into 8 bytes per block
    /// (standard big-endian ETC1 block, differential mode, zero delta, flip 0), or 16 bytes per
    /// block (`[colour block][alpha block]`, alpha in the alpha block's green) when
    /// [`has_alpha`](Self::has_alpha). Blocks are in stored (memory) order.
    ///
    /// A P-frame is decoded by replaying the frames from the last I-frame before it.
    pub(crate) fn transcode(
        &self,
        level: &[u8],
        desc_index: usize,
        blocks_x: usize,
        blocks_y: usize,
    ) -> Result<Vec<u8>> {
        if desc_index >= self.descs.len() {
            return Err(Error::Malformed("BasisLZ image index"));
        }
        let mut first = desc_index;
        while self.descs[first].flags & IS_P_FRAME != 0 {
            if first == 0 {
                return Err(Error::Malformed("BasisLZ: P-frame without an I-frame"));
            }
            first -= 1;
        }
        let n = blocks_x
            .checked_mul(blocks_y)
            .ok_or(Error::Malformed("BasisLZ: image too large"))?;
        let stride = if self.alpha { 16 } else { 8 };
        let mut out = vec![
            0u8;
            n.checked_mul(stride)
                .ok_or(Error::Malformed("BasisLZ: image too large"))?
        ];
        let slice = |(off, len): (usize, usize)| {
            off.checked_add(len)
                .and_then(|end| level.get(off..end))
                .filter(|s| !s.is_empty())
                .ok_or(Error::Truncated("BasisLZ slice"))
        };
        let channels: &[Channel] = if self.alpha {
            &[(|d| d.rgb, 0), (|d| d.alpha, 8)]
        } else {
            &[(|d| d.rgb, 0)]
        };
        for &(get, offset) in channels {
            // Previous frame's (endpoint << 16 | selector) per block, for P-frames only.
            let mut prev = if first < desc_index {
                vec![0u32; n]
            } else {
                Vec::new()
            };
            for i in first..desc_index {
                let d = &self.descs[i];
                self.decode_slice(
                    slice(get(d))?,
                    blocks_x,
                    blocks_y,
                    i > first,
                    &mut prev,
                    |_, _, _| {},
                )?;
            }
            let d = &self.descs[desc_index];
            let blocks = &mut out[..];
            self.decode_slice(
                slice(get(d))?,
                blocks_x,
                blocks_y,
                desc_index > first,
                &mut prev,
                |i, e, s| {
                    let o = i * stride + offset;
                    let b = &mut blocks[o..o + 8];
                    b[..4].copy_from_slice(&self.endpoints[e as usize]);
                    b[4..].copy_from_slice(&self.selectors[s as usize]);
                },
            )?;
        }
        Ok(out)
    }

    /// Decodes one slice's codebook indices, calling `emit(block, endpoint, selector)` for each
    /// block in memory order. `prev` (empty, or one entry per block) holds the previous frame's
    /// indices; it is read for P-frames and updated when non-empty.
    fn decode_slice(
        &self,
        data: &[u8],
        bx: usize,
        by: usize,
        p_frame: bool,
        prev: &mut [u32],
        mut emit: impl FnMut(usize, u32, u32),
    ) -> Result<()> {
        let endpoint_count = self.endpoints.len() as u32;
        let selector_count = self.selectors.len() as u32;
        let hist_n = self.history_size;
        let rle_sym = selector_count + hist_n as u32;
        let video = !prev.is_empty();
        if p_frame && !video {
            return Err(Error::Malformed("BasisLZ: P-frame outside a sequence"));
        }

        let mut r = BitReader::new(data);
        // Endpoint indices of the previous and current block row (row parity selects).
        let mut row_preds = vec![0u16; bx * 2];
        // Predictor bits for the odd row, saved on the even row (2 blocks per entry).
        let mut odd_bits = vec![0u8; bx.div_ceil(2)];
        let mut hist = vec![0u32; hist_n];
        let mut rover = hist_n / 2;

        let mut pred_bits = 0u32;
        let mut prev_pred_sym = 0u32;
        let mut pred_repeat = 0u32;
        let mut prev_endpoint = 0u32;
        let mut sel_rle = 0u32;

        for y in 0..by {
            let (cur_row, up_row) = if y & 1 == 0 { (0, bx) } else { (bx, 0) };
            for x in 0..bx {
                if x & 1 == 0 {
                    if y & 1 == 0 {
                        if pred_repeat > 0 {
                            pred_repeat -= 1;
                            pred_bits = prev_pred_sym;
                        } else {
                            pred_bits = r.decode(&self.endpoint_pred)?;
                            if pred_bits == ENDPOINT_PRED_REPEAT {
                                pred_repeat = r.vlc(5).saturating_add(3 - 1);
                                pred_bits = prev_pred_sym;
                            } else if pred_bits > ENDPOINT_PRED_REPEAT {
                                return Err(Error::Malformed("BasisLZ: bad endpoint predictor"));
                            } else {
                                prev_pred_sym = pred_bits;
                            }
                        }
                        odd_bits[x >> 1] = (pred_bits >> 4) as u8;
                    } else {
                        pred_bits = u32::from(odd_bits[x >> 1]);
                    }
                }
                let pred = pred_bits & 3;
                pred_bits >>= 2;

                let block = y * bx + x;
                let mut selector = None;
                let endpoint = match pred {
                    0 => prev_endpoint,
                    1 => u32::from(row_preds[up_row + x]),
                    2 if p_frame => {
                        let p = prev[block];
                        selector = Some(p & 0xffff);
                        p >> 16
                    }
                    2 => u32::from(row_preds[up_row + x.saturating_sub(1)]),
                    _ => {
                        let mut e = r.decode(&self.endpoint_delta)? + prev_endpoint;
                        if e >= endpoint_count {
                            e -= endpoint_count;
                        }
                        if e >= endpoint_count {
                            return Err(Error::Malformed("BasisLZ: endpoint index"));
                        }
                        e
                    }
                };
                row_preds[cur_row + x] = endpoint as u16;
                prev_endpoint = endpoint;

                let selector = match selector {
                    Some(s) => s,
                    None => {
                        let sym = if sel_rle > 0 {
                            sel_rle -= 1;
                            selector_count
                        } else {
                            let mut sym = r.decode(&self.selector)?;
                            if sym == rle_sym {
                                let run = r.decode(&self.selector_rle)?;
                                sel_rle = if run == SELECTOR_RLE_LONG {
                                    r.vlc(8)
                                } else {
                                    run
                                }
                                .saturating_add(3 - 1);
                                sym = selector_count;
                            }
                            sym
                        };
                        if sym >= selector_count {
                            let h = (sym - selector_count) as usize;
                            if h >= hist_n {
                                return Err(Error::Malformed("BasisLZ: selector history index"));
                            }
                            let s = hist[h];
                            if h != 0 {
                                hist.swap(h / 2, h);
                            }
                            s
                        } else {
                            if hist_n > 0 {
                                hist[rover] = sym;
                                rover += 1;
                                if rover == hist_n {
                                    rover = hist_n / 2;
                                }
                            }
                            sym
                        }
                    }
                };
                if video {
                    prev[block] = endpoint << 16 | selector;
                }
                emit(block, endpoint, selector);
            }
        }
        if r.overrun() {
            return Err(Error::Truncated("BasisLZ slice"));
        }
        Ok(())
    }
}

/// Decodes the endpoint codebook into ETC1 block header bytes.
fn decode_endpoints(data: &[u8], count: usize) -> Result<Vec<[u8; 4]>> {
    let mut r = BitReader::new(data);
    let models = [
        Huffman::read(&mut r)?,
        Huffman::read(&mut r)?,
        Huffman::read(&mut r)?,
    ];
    let inten_model = Huffman::read(&mut r)?;
    let grey = r.bits(1) != 0;
    let mut out = Vec::with_capacity(count);
    let mut inten = 0u32;
    let mut c = [16u32; 3];
    for _ in 0..count {
        inten = (inten + r.decode(&inten_model)?) & 7;
        for v in c.iter_mut().take(if grey { 1 } else { 3 }) {
            let model = match *v {
                0..=9 => &models[0],
                10..=21 => &models[1],
                _ => &models[2],
            };
            *v = (*v + r.decode(model)?) & 31;
        }
        if grey {
            c = [c[0]; 3];
        }
        out.push([
            (c[0] << 3) as u8,
            (c[1] << 3) as u8,
            (c[2] << 3) as u8,
            (inten << 5 | inten << 2 | 2) as u8,
        ]);
    }
    if r.overrun() {
        return Err(Error::Truncated("BasisLZ endpoint codebook"));
    }
    Ok(out)
}

/// Decodes the selector codebook into ETC1 pixel-index bytes.
fn decode_selectors(data: &[u8], count: usize) -> Result<Vec<[u8; 4]>> {
    let mut r = BitReader::new(data);
    if r.bits(2) != 0 {
        return Err(Error::Malformed("BasisLZ selector codebook header"));
    }
    let raw = r.bits(1) != 0;
    let mut out = Vec::with_capacity(count);
    let mut rows = [0u32; 4];
    if raw {
        for _ in 0..count {
            for row in &mut rows {
                *row = r.bits(8);
            }
            out.push(etc1_selector_bytes(rows));
        }
    } else {
        let model = Huffman::read(&mut r)?;
        for i in 0..count {
            for row in &mut rows {
                *row = if i == 0 {
                    r.bits(8)
                } else {
                    (r.decode(&model)? ^ *row) & 0xff
                };
            }
            out.push(etc1_selector_bytes(rows));
        }
    }
    if r.overrun() {
        return Err(Error::Truncated("BasisLZ selector codebook"));
    }
    Ok(out)
}

/// ETC1 pixel index bytes for four selector rows (2 bits per texel, x = 0 in the low bits).
/// Selector 0..3 is modifier -b, -a, +a, +b, i.e. ETC1 index (MSB, LSB) 11, 10, 00, 01.
fn etc1_selector_bytes(rows: [u32; 4]) -> [u8; 4] {
    let mut v = 0u32;
    for (y, &row) in rows.iter().enumerate() {
        for x in 0..4 {
            let s = (row >> (2 * x)) & 3;
            let i = x * 4 + y;
            let msb = u32::from(s < 2);
            let lsb = u32::from(s == 0 || s == 3);
            v |= msb << (16 + i) | lsb << i;
        }
    }
    v.to_be_bytes()
}

/// Decodes one ETC1S block written by [`Etc1sGlobal::transcode`] (8 bytes, or 16 with `alpha`:
/// alpha is the alpha block's decoded green, else 255) to 4x4 RGBA8 at `out` with row `pitch`.
pub(crate) fn decode_block(src: &[u8], alpha: bool, out: &mut [u8], pitch: usize) {
    let (rgb, sel) = palette(&src[..8]);
    let a = if alpha {
        let (p, s) = palette(&src[8..16]);
        let g = [p[0][1], p[1][1], p[2][1], p[3][1]];
        Some((g, s))
    } else {
        None
    };
    for y in 0..4 {
        let row = &mut out[y * pitch..y * pitch + 16];
        for x in 0..4 {
            let i = x * 4 + y;
            let c = rgb[pixel_index(sel, i)];
            let alpha = a.map_or(255, |(g, s)| g[pixel_index(s, i)]);
            row[x * 4..x * 4 + 4].copy_from_slice(&[c[0], c[1], c[2], alpha]);
        }
    }
}

#[inline(always)]
fn pixel_index(sel: u32, i: usize) -> usize {
    ((sel >> (16 + i) & 1) << 1 | (sel >> i & 1)) as usize
}

/// The four colours of an ETC1S block, by ETC1 pixel index, and its pixel index bits.
fn palette(b: &[u8]) -> ([[u8; 3]; 4], u32) {
    let base = [b[0] >> 3, b[1] >> 3, b[2] >> 3].map(|c| i32::from(c << 3 | c >> 2));
    let [a, bb] = INTEN[usize::from(b[3] >> 5)];
    let modifiers = [a, bb, -a, -bb];
    let colours = modifiers.map(|m| base.map(|c| (c + m).clamp(0, 255) as u8));
    (colours, u32::from_be_bytes([b[4], b[5], b[6], b[7]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes bits LSB-first.
    #[derive(Default)]
    struct BitWriter {
        bytes: Vec<u8>,
        n: usize,
    }

    impl BitWriter {
        fn put(&mut self, v: u32, bits: u32) {
            for i in 0..bits {
                if self.n.is_multiple_of(8) {
                    self.bytes.push(0);
                }
                let last = self.bytes.len() - 1;
                self.bytes[last] |= (((v >> i) & 1) as u8) << (self.n % 8);
                self.n += 1;
            }
        }

        /// Writes a canonical code MSB first (the order a Huffman decoder consumes it).
        fn code(&mut self, code: u32, len: u32) {
            for i in (0..len).rev() {
                self.put((code >> i) & 1, 1);
            }
        }
    }

    fn canonical(lens: &[u8]) -> Vec<u32> {
        let mut codes = vec![0; lens.len()];
        let mut code = 0;
        for len in 1..=16 {
            for (s, &l) in lens.iter().enumerate() {
                if l == len {
                    codes[s] = code;
                    code += 1;
                }
            }
            code <<= 1;
        }
        codes
    }

    #[test]
    fn huffman_fast_and_long_codes() {
        // A skewed table with codes from 1 to 16 bits (complete: lengths 1..15, then two 16s).
        let mut lens: Vec<u8> = (1..=15).collect();
        lens.push(16);
        lens.push(16);
        let codes = canonical(&lens);
        let h = Huffman::from_lengths(&lens).unwrap();
        let mut w = BitWriter::default();
        let seq: Vec<usize> = (0..lens.len())
            .chain((0..lens.len()).rev())
            .chain([16, 0, 16])
            .collect();
        for &s in &seq {
            w.code(codes[s], u32::from(lens[s]));
        }
        let mut r = BitReader::new(&w.bytes);
        for &s in &seq {
            assert_eq!(r.decode(&h).unwrap(), s as u32);
        }
        assert!(!r.overrun());
        r.bits(32);
        assert!(r.overrun());
    }

    #[test]
    fn huffman_rejects_oversubscribed_and_unused_codes() {
        assert!(Huffman::from_lengths(&[1, 1, 1]).is_err());
        // Incomplete code: 0 -> sym 0, 10 -> sym 1, 11 unused.
        let h = Huffman::from_lengths(&[1, 2]).unwrap();
        let mut r = BitReader::new(&[0b11]);
        assert!(r.decode(&h).is_err());
        // Empty table.
        let h = Huffman::from_lengths(&[0, 0]).unwrap();
        assert!(BitReader::new(&[0]).decode(&h).is_err());
        // One symbol decodes whatever the bit.
        let h = Huffman::from_lengths(&[0, 1]).unwrap();
        let mut r = BitReader::new(&[0b10]);
        assert_eq!((r.decode(&h).unwrap(), r.decode(&h).unwrap()), (1, 1));
    }

    #[test]
    fn huffman_table_with_runs() {
        // 260 code lengths: 3, 3, 0 x 130 (sym 18), 4 x 4 (4, then sym 19 run of 3), 0 x 5
        // (sym 17), 5 x 8 (5, then sym 20 run of 7), 0 x 111 (sym 18). An incomplete code.
        let mut lens = vec![0u8; 260];
        lens[..2].fill(3);
        lens[132..136].fill(4);
        lens[141..149].fill(5);
        let mut cl = [0u8; 21];
        for s in [0, 3, 4, 5, 17, 18, 19, 20] {
            cl[s] = 3;
        }
        let cl_codes = canonical(&cl);
        let mut w = BitWriter::default();
        w.put(260, 14);
        w.put(21, 5);
        for &s in &CODELENGTH_ORDER {
            w.put(u32::from(cl[s]), 3);
        }
        let sym = |w: &mut BitWriter, s: usize| w.code(cl_codes[s], 3);
        sym(&mut w, 3);
        sym(&mut w, 3);
        sym(&mut w, 18);
        w.put(130 - 11, 7);
        sym(&mut w, 4);
        sym(&mut w, 19);
        w.put(0, 2);
        sym(&mut w, 17);
        w.put(5 - 3, 3);
        sym(&mut w, 5);
        sym(&mut w, 20);
        w.put(0, 7);
        sym(&mut w, 18);
        w.put(111 - 11, 7);
        let mut r = BitReader::new(&w.bytes);
        let h = Huffman::read(&mut r).unwrap();
        let codes = canonical(&lens);
        let mut w = BitWriter::default();
        for s in [0, 1, 132, 135, 141, 148] {
            w.code(codes[s], u32::from(lens[s]));
        }
        let mut r = BitReader::new(&w.bytes);
        for s in [0, 1, 132, 135, 141, 148] {
            assert_eq!(r.decode(&h).unwrap(), s);
        }
    }

    #[test]
    fn vlc() {
        let mut w = BitWriter::default();
        w.put(0b0_0111, 5);
        w.put(0b1_0011, 5);
        w.put(0b0_0001, 5);
        w.put(0x85, 8);
        w.put(0x02, 8);
        let mut r = BitReader::new(&w.bytes);
        assert_eq!(r.vlc(5), 7);
        assert_eq!(r.vlc(5), 0x13);
        assert_eq!(r.vlc(8), 5 | 2 << 7);
    }

    #[test]
    fn selector_bytes_and_decode() {
        // Rows: texel (x, y) gets selector (x + y) & 3.
        let rows = [0b11_10_01_00, 0b00_11_10_01, 0b01_00_11_10, 0b10_01_00_11];
        let sel = etc1_selector_bytes(rows);
        // Endpoint: base 16 (-> 132), intensity table 1 (5, 17).
        let block = [
            16 << 3,
            16 << 3,
            16 << 3,
            1 << 5 | 1 << 2 | 2,
            sel[0],
            sel[1],
            sel[2],
            sel[3],
        ];
        let mut out = [0u8; 64];
        decode_block(&block, false, &mut out, 16);
        let expect = [132 - 17, 132 - 5, 132 + 5, 132 + 17];
        for y in 0..4 {
            for x in 0..4 {
                let px = &out[y * 16 + x * 4..][..4];
                let v = expect[(x + y) & 3] as u8;
                assert_eq!(px, [v, v, v, 255], "{x},{y}");
            }
        }
    }

    /// The BasisLZ global data and level 0 of the example KTX2 file in the KTX 2.0
    /// specification (8x8, alpha); RGBA reference from the basisu transcoder.
    #[test]
    fn spec_example() {
        let sgd: &[u8] = &[
            0x02, 0x00, 0x02, 0x00, 0x2D, 0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00, 0x2E, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // header
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, 0x00,
            0x00, 0x00, 0x01, 0x00, 0x00, 0x00, // imageDesc[0]
            0x01, 0xC0, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x04, 0x98, 0x1B, 0x20,
            0x00, 0x00, 0x00, 0x08, 0xC3, 0x36, 0x91, 0x3E, 0x91, 0x00, 0x60, 0x02, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x81, 0x00, 0x4C, 0x01, 0x10, 0x00, 0x00, 0x00, 0x00, 0x20,
            0x59, 0xC0, 0x3D, // endpoints
            0x54, 0x55, 0x55, 0x55, 0xAD, 0xAA, 0xAA, 0xAA, 0x02, // selectors
            0x14, 0xC0, 0x44, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x12, 0x41, 0x00, 0x98, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x18, 0x02, 0xA2, 0x04, 0x0C, 0x00, 0x00, 0x00,
            0x83, 0x76, 0x7B, 0x49, 0x04, 0xA2, 0x20, 0x00, 0x4C, 0x00, 0x08, 0x00, 0x00, 0x00,
            0x00, 0x20, 0x02, 0x01, // tables
        ];
        let level = [0x4E, 0x0E, 0x04];
        let g = Etc1sGlobal::parse(sgd, 1).unwrap();
        assert!(g.has_alpha());
        let blocks = g.transcode(&level, 0, 2, 2).unwrap();
        assert_eq!(blocks.len(), 4 * 16);
        let mut rgba = [0u8; 8 * 8 * 4];
        for (i, b) in blocks.chunks_exact(16).enumerate() {
            let o = (i / 2) * 4 * 32 + (i % 2) * 16;
            decode_block(b, true, &mut rgba[o..], 32);
        }
        // basisu: every texel (0xab, 0xbb, 0xcc, 0x80).
        assert!(
            rgba.chunks_exact(4)
                .all(|px| px == [0xab, 0xbb, 0xcc, 0x80])
        );
    }
}

#[cfg(test)]
mod hostile {
    use super::*;

    /// One endpoint and one selector, with the given slice Huffman code lengths.
    fn global(pred: &[u8], selector: &[u8], rle: &[u8], history_size: usize) -> Etc1sGlobal {
        Etc1sGlobal {
            endpoints: vec![[0; 4]],
            selectors: vec![[0; 4]],
            endpoint_pred: Huffman::from_lengths(pred).unwrap(),
            endpoint_delta: Huffman::from_lengths(&[1]).unwrap(),
            selector: Huffman::from_lengths(selector).unwrap(),
            selector_rle: Huffman::from_lengths(rle).unwrap(),
            history_size,
            descs: Vec::new(),
            alpha: false,
        }
    }

    /// An endpoint-predictor repeat whose count VLC has every continuation bit set: the count
    /// is u32::MAX, and adding the bias overflowed.
    #[test]
    fn endpoint_repeat_count_saturates() {
        let mut pred = [0u8; 257];
        pred[0] = 1;
        pred[ENDPOINT_PRED_REPEAT as usize] = 1;
        let g = global(&pred, &[1], &[1], 0);
        let _ = g.decode_slice(&[0xff; 16], 1, 1, false, &mut [], |_, _, _| {});
    }

    /// A long selector run whose VLC has every continuation bit set.
    #[test]
    fn selector_run_saturates() {
        let mut rle = [0u8; 64];
        rle[0] = 1;
        rle[SELECTOR_RLE_LONG as usize] = 1;
        let g = global(&[1], &[1, 0, 1], &rle, 1);
        let _ = g.decode_slice(&[0xff; 16], 1, 1, false, &mut [], |_, _, _| {});
    }
}
