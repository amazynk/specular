//! Layer III (MP3), ISO/IEC 11172-3 §2.4.3.4 and the lower sampling rates of ISO/IEC
//! 13818-3: side information, the bit reservoir, scalefactors (with MPEG-2's partitions),
//! Huffman-coded spectra, requantisation, middle/side and intensity stereo, reordering of
//! short blocks, alias reduction, the IMDCT with its four windows and overlap, and the
//! polyphase synthesis shared with Layers I and II.
//!
//! The Huffman code words are in `huffman_data.rs`, found by probing a reference decoder
//! (`tools/corpus/mp3-huffman-probe.py`) and checked word by word against ISO/IEC 11172-3
//! Annex B (`tools/corpus/mp3-tables-check.py`: 0 mismatches); the scalefactor bands,
//! `pretab`, the scalefactor lengths and MPEG-2's partition counts are the standard's
//! printed numbers; windows,
//! cosines and the alias-reduction butterflies are computed from their definitions.

use std::sync::OnceLock;

use crate::synth::Synth;
use crate::{Error, Header, Version};

fn bad(s: &str) -> Error {
    Error::Malformed(s.into())
}

/// Scalefactor band widths (long: 22 bands; short: 13 bands a window) by sampling rate:
/// 44.1, 48, 32 kHz (MPEG-1), 22.05, 24, 16 kHz (MPEG-2), 11.025, 12, 8 kHz (MPEG-2.5).
const LONG_WIDTHS: [[u16; 22]; 9] = [
    [
        4, 4, 4, 4, 4, 4, 6, 6, 8, 8, 10, 12, 16, 20, 24, 28, 34, 42, 50, 54, 76, 158,
    ],
    [
        4, 4, 4, 4, 4, 4, 6, 6, 6, 8, 10, 12, 16, 18, 22, 28, 34, 40, 46, 54, 54, 192,
    ],
    [
        4, 4, 4, 4, 4, 4, 6, 6, 8, 10, 12, 16, 20, 24, 30, 38, 46, 56, 68, 84, 102, 26,
    ],
    [
        6, 6, 6, 6, 6, 6, 8, 10, 12, 14, 16, 20, 24, 28, 32, 38, 46, 52, 60, 68, 58, 54,
    ],
    [
        6, 6, 6, 6, 6, 6, 8, 10, 12, 14, 16, 18, 22, 26, 32, 38, 46, 54, 62, 70, 76, 36,
    ],
    [
        6, 6, 6, 6, 6, 6, 8, 10, 12, 14, 16, 20, 24, 28, 32, 38, 46, 52, 60, 68, 58, 54,
    ],
    [
        6, 6, 6, 6, 6, 6, 8, 10, 12, 14, 16, 20, 24, 28, 32, 38, 46, 52, 60, 68, 58, 54,
    ],
    [
        6, 6, 6, 6, 6, 6, 8, 10, 12, 14, 16, 20, 24, 28, 32, 38, 46, 52, 60, 68, 58, 54,
    ],
    [
        12, 12, 12, 12, 12, 12, 16, 20, 24, 28, 32, 40, 48, 56, 64, 76, 90, 2, 2, 2, 2, 2,
    ],
];

const SHORT_WIDTHS: [[u16; 13]; 9] = [
    [4, 4, 4, 4, 6, 8, 10, 12, 14, 18, 22, 30, 56],
    [4, 4, 4, 4, 6, 6, 10, 12, 14, 16, 20, 26, 66],
    [4, 4, 4, 4, 6, 8, 12, 16, 20, 26, 34, 42, 12],
    [4, 4, 4, 6, 6, 8, 10, 14, 18, 26, 32, 42, 18],
    [4, 4, 4, 6, 8, 10, 12, 14, 18, 24, 32, 44, 12],
    [4, 4, 4, 6, 8, 10, 12, 14, 18, 24, 30, 40, 18],
    [4, 4, 4, 6, 8, 10, 12, 14, 18, 24, 30, 40, 18],
    [4, 4, 4, 6, 8, 10, 12, 14, 18, 24, 30, 40, 18],
    [8, 8, 8, 12, 16, 20, 24, 28, 36, 2, 2, 2, 26],
];

/// Added to the long scalefactors when `preflag` is set (Table B.6).
const PRETAB: [u8; 22] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 3, 3, 3, 2, 0,
];

/// MPEG-1 `scalefac_compress` → (slen1, slen2).
const SLEN: [[u8; 2]; 16] = [
    [0, 0],
    [0, 1],
    [0, 2],
    [0, 3],
    [3, 0],
    [1, 1],
    [1, 2],
    [1, 3],
    [2, 1],
    [2, 2],
    [2, 3],
    [3, 1],
    [3, 2],
    [3, 3],
    [4, 2],
    [4, 3],
];

/// MPEG-2 scalefactors in each of four partitions, by the partition table (0–2 normal,
/// 3–5 for the intensity-coded right channel) and block kind (long, short, mixed).
const LSF_PARTS: [[[u8; 4]; 3]; 6] = [
    [[6, 5, 5, 5], [9, 9, 9, 9], [6, 9, 9, 9]],
    [[6, 5, 7, 3], [9, 9, 12, 6], [6, 9, 12, 6]],
    [[11, 10, 0, 0], [18, 18, 0, 0], [15, 18, 0, 0]],
    [[7, 7, 7, 0], [12, 12, 12, 0], [6, 15, 12, 0]],
    [[6, 6, 6, 3], [12, 9, 9, 6], [6, 12, 9, 6]],
    [[8, 8, 5, 0], [15, 12, 9, 0], [6, 18, 9, 0]],
];

/// The alias-reduction coefficients c_i (Table B.9).
const ALIAS_C: [f64; 8] = [
    -0.6, -0.535, -0.33, -0.185, -0.095, -0.041, -0.0142, -0.0037,
];

/// Band edges of one sampling rate.
struct Bands {
    long: [usize; 23],
    short: [usize; 14],
}

/// A Huffman tree: node pairs; a value ≥ 0x8000 is a leaf holding the symbol.
struct Tree {
    nodes: Vec<[u16; 2]>,
}

impl Tree {
    fn build(words: &[(u8, u32)]) -> Tree {
        let mut nodes = vec![[0u16; 2]];
        for (sym, &(len, code)) in words.iter().enumerate() {
            let mut at = 0usize;
            for i in (0..len).rev() {
                let bit = ((code >> i) & 1) as usize;
                if i == 0 {
                    nodes[at][bit] = 0x8000 | sym as u16;
                } else {
                    if nodes[at][bit] == 0 {
                        nodes.push([0, 0]);
                        nodes[at][bit] = (nodes.len() - 1) as u16;
                    }
                    at = usize::from(nodes[at][bit]);
                }
            }
        }
        Tree { nodes }
    }

    /// Decodes one symbol from the reader.
    #[inline]
    fn decode(&self, r: &mut Reader) -> usize {
        let w = r.peek32();
        let mut at = 0usize;
        for i in 0..32 {
            let bit = ((w >> (31 - i)) & 1) as usize;
            let n = self.nodes[at][bit];
            if n & 0x8000 != 0 {
                r.skip(i + 1);
                return usize::from(n & 0x7FFF);
            }
            if n == 0 {
                // Not a code word (the trees are complete, so only on bad tables).
                r.skip(i + 1);
                return 0;
            }
            at = usize::from(n);
        }
        r.skip(32);
        0
    }
}

struct Tables {
    bands: Vec<Bands>,
    /// |x|^(4/3) for the values a spectrum can hold.
    pow43: Vec<f32>,
    /// Trees by table number (`None` for 0, 4 and 14) and their sizes.
    trees: Vec<Option<(Tree, usize)>>,
    count1a: Tree,
    /// IMDCT cosines: long [36][18], short [12][6].
    cos_long: Vec<[f32; 18]>,
    cos_short: Vec<[f32; 6]>,
    /// Windows by block type (0 normal, 1 start, 3 stop; 2 is the short window in `win_short`).
    win: [[f32; 36]; 4],
    win_short: [f32; 12],
    /// Alias-reduction butterflies (cs, ca).
    alias: [(f32, f32); 8],
}

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        use crate::huffman_data as h;
        let bands = (0..9)
            .map(|r| {
                let mut long = [0usize; 23];
                for i in 0..22 {
                    long[i + 1] = long[i] + usize::from(LONG_WIDTHS[r][i]);
                }
                let mut short = [0usize; 14];
                for i in 0..13 {
                    short[i + 1] = short[i] + usize::from(SHORT_WIDTHS[r][i]);
                }
                Bands { long, short }
            })
            .collect();
        let pow43 = (0..8207 + 16)
            .map(|i| (f64::from(i)).powf(4.0 / 3.0) as f32)
            .collect();
        let sized = |w: &[(u8, u32)], n: usize| Some((Tree::build(w), n));
        let mut trees: Vec<Option<(Tree, usize)>> = (0..32).map(|_| None).collect();
        trees[1] = sized(&h::T1, 2);
        trees[2] = sized(&h::T2, 3);
        trees[3] = sized(&h::T3, 3);
        trees[5] = sized(&h::T5, 4);
        trees[6] = sized(&h::T6, 4);
        trees[7] = sized(&h::T7, 6);
        trees[8] = sized(&h::T8, 6);
        trees[9] = sized(&h::T9, 6);
        trees[10] = sized(&h::T10, 8);
        trees[11] = sized(&h::T11, 8);
        trees[12] = sized(&h::T12, 8);
        trees[13] = sized(&h::T13, 16);
        trees[15] = sized(&h::T15, 16);
        trees[16] = sized(&h::T16, 16);
        trees[24] = sized(&h::T24, 16);
        let pi = std::f64::consts::PI;
        let cos_long = (0..36)
            .map(|i| {
                let mut row = [0f32; 18];
                for (k, v) in row.iter_mut().enumerate() {
                    *v = (pi / 72.0 * (2 * i + 1 + 18) as f64 * (2 * k + 1) as f64).cos() as f32;
                }
                row
            })
            .collect();
        let cos_short = (0..12)
            .map(|i| {
                let mut row = [0f32; 6];
                for (k, v) in row.iter_mut().enumerate() {
                    *v = (pi / 24.0 * (2 * i + 1 + 6) as f64 * (2 * k + 1) as f64).cos() as f32;
                }
                row
            })
            .collect();
        let s36 = |i: usize| (pi / 36.0 * (i as f64 + 0.5)).sin() as f32;
        let s12 = |i: usize| (pi / 12.0 * (i as f64 + 0.5)).sin() as f32;
        let mut win = [[0f32; 36]; 4];
        for i in 0..36 {
            win[0][i] = s36(i);
            win[1][i] = match i {
                0..=17 => s36(i),
                18..=23 => 1.0,
                24..=29 => s12(i - 18),
                _ => 0.0,
            };
            win[3][i] = match i {
                0..=5 => 0.0,
                6..=11 => s12(i - 6),
                12..=17 => 1.0,
                _ => s36(i),
            };
        }
        let mut win_short = [0f32; 12];
        for (i, w) in win_short.iter_mut().enumerate() {
            *w = s12(i);
        }
        let alias = ALIAS_C.map(|c| {
            let d = (1.0 + c * c).sqrt();
            ((1.0 / d) as f32, (c / d) as f32)
        });
        Tables {
            bands,
            pow43,
            trees,
            count1a: Tree::build(&h::COUNT1_A),
            cos_long,
            cos_short,
            win,
            win_short,
            alias,
        }
    })
}

/// Big-endian bit reader over a byte slice; past the end it reads zeros.
struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    #[inline]
    fn peek32(&self) -> u32 {
        let byte = self.pos >> 3;
        let mut w = [0u8; 8];
        if let Some(s) = self.data.get(byte..byte + 8) {
            w.copy_from_slice(s);
        } else {
            for (i, v) in w.iter_mut().enumerate() {
                *v = self.data.get(byte + i).copied().unwrap_or(0);
            }
        }
        (u64::from_be_bytes(w) << (self.pos & 7) >> 32) as u32
    }

    #[inline]
    fn skip(&mut self, n: usize) {
        self.pos += n;
    }

    #[inline]
    fn read(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        let v = self.peek32() >> (32 - n);
        self.skip(n as usize);
        v
    }
}

/// One granule of one channel's side information.
#[derive(Clone, Copy, Default)]
struct Granule {
    part23: usize,
    big_values: usize,
    global_gain: i32,
    sfc: u32,
    block_type: u8,
    mixed: bool,
    table: [u8; 3],
    subblock_gain: [u8; 3],
    region1: usize,
    region2: usize,
    preflag: bool,
    sf_scale: bool,
    count1b: bool,
    /// The right channel of intensity stereo (MPEG-2 reads its scalefactors differently).
    right_is: bool,
}

/// What a channel keeps between frames: the IMDCT overlap.
#[derive(Clone)]
pub(crate) struct State {
    /// Main data of the frames before (the bit reservoir), newest last.
    reservoir: Vec<u8>,
    overlap: [[[f32; 18]; 32]; 2],
}

impl State {
    pub fn new() -> State {
        State {
            reservoir: Vec::new(),
            overlap: [[[0.0; 18]; 32]; 2],
        }
    }
}

fn rate_index(h: &Header) -> usize {
    let base = match h.sample_rate {
        44_100 | 22_050 | 11_025 => 0,
        48_000 | 24_000 | 12_000 => 1,
        _ => 2,
    };
    match h.version {
        Version::Mpeg1 => base,
        Version::Mpeg2 => 3 + base,
        Version::Mpeg25 => 6 + base,
    }
}

/// Decodes one frame; `want` is the stream's channel count (a frame that differs is
/// spread or folded to it).
pub(crate) fn decode_frame(
    h: &Header,
    frame: &[u8],
    st: &mut State,
    synth: &mut [Synth; 2],
    want: usize,
    out: &mut Vec<f32>,
) -> Result<(), Error> {
    let t = tables();
    let lsf = h.version != Version::Mpeg1;
    let nch = h.channels();
    let granules = if lsf { 1 } else { 2 };
    let at = 4 + if h.crc { 2 } else { 0 };
    let si_len = match (lsf, nch) {
        (false, 1) => 17,
        (false, _) => 32,
        (true, 1) => 9,
        (true, _) => 17,
    };
    let side = frame
        .get(at..at + si_len)
        .ok_or_else(|| bad("a frame shorter than its side information"))?;
    let main = &frame[at + si_len..];
    let mut r = Reader { data: side, pos: 0 };
    let begin = r.read(if lsf { 8 } else { 9 }) as usize;
    r.skip(match (lsf, nch) {
        (false, 1) => 5,
        (false, _) => 3,
        (true, 1) => 1,
        (true, _) => 2,
    });
    let mut scfsi = [[false; 4]; 2];
    if !lsf {
        for s in scfsi.iter_mut().take(nch) {
            for b in s.iter_mut() {
                *b = r.read(1) != 0;
            }
        }
    }
    let rb = &t.bands[rate_index(h)];
    let mut gr = [[Granule::default(); 2]; 2];
    for g in gr.iter_mut().take(granules) {
        for (ch, c) in g.iter_mut().take(nch).enumerate() {
            c.part23 = r.read(12) as usize;
            c.big_values = (r.read(9) as usize).min(288);
            c.global_gain = r.read(8) as i32;
            c.sfc = r.read(if lsf { 9 } else { 4 });
            if r.read(1) != 0 {
                c.block_type = r.read(2) as u8;
                if c.block_type == 0 {
                    return Err(bad("window switching with a normal block"));
                }
                c.mixed = r.read(1) != 0;
                c.table = [r.read(5) as u8, r.read(5) as u8, 0];
                for s in c.subblock_gain.iter_mut() {
                    *s = r.read(3) as u8;
                }
                c.region1 = if c.block_type == 2 {
                    3 * rb.short[3]
                } else {
                    rb.long[8]
                };
                c.region2 = 576;
            } else {
                c.table = [r.read(5) as u8, r.read(5) as u8, r.read(5) as u8];
                let r0 = r.read(4) as usize;
                let r1 = r.read(3) as usize;
                c.region1 = rb.long[(r0 + 1).min(22)];
                c.region2 = rb.long[(r0 + r1 + 2).min(22)];
            }
            if !lsf {
                c.preflag = r.read(1) != 0;
            }
            c.right_is = h.mode == 1 && h.mode_extension & 1 != 0 && ch == 1;
            if lsf {
                // MPEG-2: the third partition table implies the preflag.
                c.preflag = c.sfc >= 500 && !c.right_is;
            }
            c.sf_scale = r.read(1) != 0;
            c.count1b = r.read(1) != 0;
        }
    }
    // The bit reservoir: this frame's main data begins `begin` bytes back.
    let have = st.reservoir.len();
    let mut data = Vec::with_capacity(begin + main.len());
    let complete = begin <= have;
    if complete {
        data.extend_from_slice(&st.reservoir[have - begin..]);
    }
    data.extend_from_slice(main);
    st.reservoir.extend_from_slice(main);
    if st.reservoir.len() > 4096 {
        let cut = st.reservoir.len() - 2048;
        st.reservoir.drain(..cut);
    }
    if !complete {
        // After a seek or a cut: the data this frame needs is gone.
        out.resize(out.len() + granules * 576 * want, 0.0);
        return Ok(());
    }
    let mut r = Reader {
        data: &data,
        pos: 0,
    };
    let joint = h.mode == 1;
    let ms = joint && h.mode_extension & 2 != 0;
    let intensity = joint && h.mode_extension & 1 != 0;
    // Scalefactors as transmitted (long: 22; short: 3 a band, 39), per channel; kept for
    // granule 1's scfsi.
    let mut sf = [[0u8; 40]; 2];
    let mut is_max = [[255u8; 40]; 2];
    for g in 0..granules {
        let mut xr = [[0f32; 576]; 2];
        let mut nonzero = [0usize; 2];
        let mut sfl = [[0u8; 40]; 2];
        for ch in 0..nch {
            let c = &gr[g][ch];
            let start = r.pos;
            if lsf {
                lsf_scalefactors(&mut r, c, c.right_is, &mut sf[ch], &mut is_max[ch]);
            } else {
                mpeg1_scalefactors(&mut r, c, g, &scfsi[ch], &mut sf[ch]);
            }
            sfl[ch] = sf[ch];
            let end = start + c.part23;
            if r.pos > end {
                return Err(bad("scalefactors past the granule"));
            }
            let mut ix = [0i32; 576];
            nonzero[ch] = huffman(&mut r, c, end, &mut ix)?;
            r.pos = end;
            requantise(c, rb, &sfl[ch], &ix, nonzero[ch], &mut xr[ch]);
        }
        if nch == 2 {
            stereo(
                lsf, &gr[g], rb, &sfl[1], &is_max[1], &nonzero, ms, intensity, &mut xr,
            );
        }
        let mut pcm = [[0f32; 576]; 2];
        for ch in 0..nch {
            let c = &gr[g][ch];
            let mut x = xr[ch];
            if c.block_type == 2 {
                reorder(c, rb, &mut x);
            }
            antialias(c, &mut x, t);
            let mut sub = [[0f32; 32]; 18];
            imdct(c, &x, &mut st.overlap[ch], &mut sub, t);
            for (s, row) in sub.iter().enumerate() {
                let mut o = [0f32; 32];
                synth[ch].run(row, &mut o);
                pcm[ch][s * 32..s * 32 + 32].copy_from_slice(&o);
            }
        }
        for i in 0..576 {
            match (nch, want) {
                (1, 1) | (2, 2) => {
                    for p in pcm.iter().take(nch) {
                        out.push(p[i]);
                    }
                }
                (1, _) => {
                    out.push(pcm[0][i]);
                    out.push(pcm[0][i]);
                }
                _ => out.push((pcm[0][i] + pcm[1][i]) * 0.5),
            }
        }
    }
    Ok(())
}

fn mpeg1_scalefactors(r: &mut Reader, c: &Granule, g: usize, scfsi: &[bool; 4], sf: &mut [u8; 40]) {
    let [s1, s2] = SLEN[c.sfc as usize & 15];
    let (s1, s2) = (u32::from(s1), u32::from(s2));
    if c.block_type == 2 {
        // Short: 6 bands of slen1 and 6 of slen2, three windows each; mixed: 8 long bands of
        // slen1 first, then short bands 3..5 and 6..11.
        let mut i = 0;
        if c.mixed {
            for _ in 0..8 {
                sf[i] = r.read(s1) as u8;
                i += 1;
            }
            for _ in 0..9 {
                sf[i] = r.read(s1) as u8;
                i += 1;
            }
        } else {
            for _ in 0..18 {
                sf[i] = r.read(s1) as u8;
                i += 1;
            }
        }
        for _ in 0..18 {
            sf[i] = r.read(s2) as u8;
            i += 1;
        }
        sf[i..].fill(0);
        return;
    }
    const GROUPS: [(usize, usize); 4] = [(0, 6), (6, 11), (11, 16), (16, 21)];
    for (k, &(a, b)) in GROUPS.iter().enumerate() {
        // Granule 1 may reuse granule 0's factors, group by group.
        if g == 1 && scfsi[k] {
            continue;
        }
        let bits = if k < 2 { s1 } else { s2 };
        for v in &mut sf[a..b] {
            *v = r.read(bits) as u8;
        }
    }
    sf[21] = 0;
}

fn lsf_scalefactors(
    r: &mut Reader,
    c: &Granule,
    right_is: bool,
    sf: &mut [u8; 40],
    is_max: &mut [u8; 40],
) {
    let sfc = c.sfc;
    let (slen, table) = if right_is {
        let s = sfc >> 1;
        if s < 180 {
            ([s / 36, (s % 36) / 6, (s % 36) % 6, 0], 3)
        } else if s < 244 {
            let s = s - 180;
            ([(s % 64) >> 4, (s % 16) >> 2, s % 4, 0], 4)
        } else {
            let s = s - 244;
            ([s / 3, s % 3, 0, 0], 5)
        }
    } else if sfc < 400 {
        (
            [(sfc >> 4) / 5, (sfc >> 4) % 5, (sfc % 16) >> 2, sfc % 4],
            0,
        )
    } else if sfc < 500 {
        let s = sfc - 400;
        ([(s >> 2) / 5, (s >> 2) % 5, s % 4, 0], 1)
    } else {
        let s = sfc - 500;
        ([s / 3, s % 3, 0, 0], 2)
    };
    let kind = match (c.block_type, c.mixed) {
        (2, false) => 1,
        (2, true) => 2,
        _ => 0,
    };
    let mut i = 0;
    for (p, &n) in LSF_PARTS[table][kind].iter().enumerate() {
        for _ in 0..n {
            if i < 40 {
                sf[i] = r.read(slen[p]) as u8;
                is_max[i] = ((1u32 << slen[p]) - 1) as u8;
            }
            i += 1;
        }
    }
    for k in i.min(40)..40 {
        sf[k] = 0;
        is_max[k] = 0;
    }
}

/// The Huffman-coded spectrum; returns one past the last line that may be nonzero.
fn huffman(r: &mut Reader, c: &Granule, end: usize, ix: &mut [i32; 576]) -> Result<usize, Error> {
    let t = tables();
    let big = c.big_values * 2;
    let regions = [c.region1.min(big), c.region2.min(big), big];
    let mut i = 0;
    for (k, &stop) in regions.iter().enumerate() {
        let n = usize::from(c.table[k]);
        let (tree, linbits) = match n {
            16..=23 => (&t.trees[16], [1, 2, 3, 4, 6, 8, 10, 13][n - 16]),
            24..=31 => (&t.trees[24], [4, 5, 6, 7, 8, 9, 11, 13][n - 24]),
            _ => (&t.trees[n], 0),
        };
        let Some((tree, size)) = tree else {
            // Table 0 (and the unused 4 and 14): zeros.
            while i < stop {
                ix[i] = 0;
                ix[i + 1] = 0;
                i += 2;
            }
            continue;
        };
        let value = |r: &mut Reader, a: usize| {
            let mut a = a as i32;
            if linbits > 0 && a == 15 {
                a += r.read(linbits) as i32;
            }
            if a != 0 && r.read(1) != 0 {
                a = -a;
            }
            a
        };
        while i < stop {
            let sym = tree.decode(r);
            ix[i] = value(r, sym / size);
            ix[i + 1] = value(r, sym % size);
            i += 2;
        }
    }
    // Count1: quadruples of −1, 0 or 1 until the granule's bits run out.
    while i + 4 <= 576 && r.pos < end {
        let q = if c.count1b {
            15 - r.read(4) as usize
        } else {
            t.count1a.decode(r)
        };
        let mut v = [(q >> 3) & 1, (q >> 2) & 1, (q >> 1) & 1, q & 1].map(|b| b as i32);
        for x in v.iter_mut() {
            if *x != 0 && r.read(1) != 0 {
                *x = -1;
            }
        }
        if r.pos > end {
            // The last quadruple ran past the granule: it isn't one.
            break;
        }
        ix[i..i + 4].copy_from_slice(&v);
        i += 4;
    }
    if r.pos > end + 32 {
        return Err(bad("Huffman data past the granule"));
    }
    Ok(i)
}

#[inline]
fn pow2(e: f64) -> f32 {
    e.exp2() as f32
}

/// Spectral values from the Huffman values, the gains and the scalefactors.
fn requantise(
    c: &Granule,
    rb: &Bands,
    sf: &[u8; 40],
    ix: &[i32; 576],
    nonzero: usize,
    xr: &mut [f32; 576],
) {
    let t = tables();
    let mult = if c.sf_scale { 1.0 } else { 0.5 };
    let gain = f64::from(c.global_gain - 210) / 4.0;
    let preflag = c.preflag;
    let value = |i: usize| {
        let a = ix[i].unsigned_abs() as usize;
        let m = t
            .pow43
            .get(a)
            .copied()
            .unwrap_or_else(|| (a as f32).powf(4.0 / 3.0));
        if ix[i] < 0 { -m } else { m }
    };
    let long = |sfb: usize, from: usize, to: usize, xr: &mut [f32; 576]| {
        let pre = if preflag && sfb < 22 {
            f64::from(PRETAB[sfb])
        } else {
            0.0
        };
        let s = f64::from(*sf.get(sfb).unwrap_or(&0));
        let g = pow2(gain - mult * (s + pre));
        for i in from..to.min(nonzero) {
            xr[i] = value(i) * g;
        }
    };
    if c.block_type != 2 {
        for sfb in 0..22 {
            long(sfb, rb.long[sfb], rb.long[sfb + 1], xr);
        }
        return;
    }
    // Short (or mixed) blocks: lines in transmitted order, band by band, window by window.
    let (long_end, short_start) = mixed_split(c, rb);
    for sfb in 0..long_end {
        long(sfb, rb.long[sfb], rb.long[sfb + 1], xr);
    }
    for sfb in short_start..13 {
        let width = rb.short[sfb + 1] - rb.short[sfb];
        for w in 0..3 {
            let k = long_end + 3 * (sfb - short_start) + w;
            let s = if sfb < 12 {
                f64::from(*sf.get(k).unwrap_or(&0))
            } else {
                0.0
            };
            let g = pow2(gain - 2.0 * f64::from(c.subblock_gain[w]) - mult * s);
            let base = 3 * rb.short[sfb] + w * width;
            for i in base..(base + width).min(nonzero) {
                xr[i] = value(i) * g;
            }
        }
    }
}

/// For short blocks: the long bands before the short ones (mixed blocks: the first 36
/// lines) and the first short band.
fn mixed_split(c: &Granule, rb: &Bands) -> (usize, usize) {
    if !c.mixed {
        return (0, 0);
    }
    let long_end = (0..22).find(|&s| rb.long[s] >= 36).unwrap_or(8);
    let short_start = (0..13).find(|&s| rb.short[s] * 3 >= 36).unwrap_or(3);
    (long_end, short_start)
}

#[allow(clippy::too_many_arguments)]
fn stereo(
    lsf: bool,
    gr: &[Granule; 2],
    rb: &Bands,
    sf_right: &[u8; 40],
    is_max: &[u8; 40],
    nonzero: &[usize; 2],
    ms: bool,
    intensity: bool,
    xr: &mut [[f32; 576]; 2],
) {
    const M: f32 = std::f32::consts::FRAC_1_SQRT_2;
    let ms_lines = |xr: &mut [[f32; 576]; 2], from: usize, to: usize| {
        if ms {
            for i in from..to {
                let (m, s) = (xr[0][i], xr[1][i]);
                xr[0][i] = (m + s) * M;
                xr[1][i] = (m - s) * M;
            }
        }
    };
    if !intensity {
        ms_lines(xr, 0, nonzero[0].max(nonzero[1]));
        return;
    }
    let right = &gr[1];
    // The intensity factors (kl, kr) of an intensity position, or None when the position
    // is illegal (the band is coded normally).
    let factors = |pos: u8, max: u8| -> Option<(f32, f32)> {
        if lsf {
            if pos == max {
                return None;
            }
            let io: f64 = if right.sfc & 1 != 0 {
                std::f64::consts::FRAC_1_SQRT_2
            } else {
                0.5f64.sqrt().sqrt()
            };
            let p = i32::from(pos);
            Some(if p == 0 {
                (1.0, 1.0)
            } else if p & 1 == 1 {
                (io.powi((p + 1) / 2) as f32, 1.0)
            } else {
                (1.0, io.powi(p / 2) as f32)
            })
        } else {
            if pos >= 7 {
                return None;
            }
            if pos == 6 {
                return Some((1.0, 0.0));
            }
            let k = (f64::from(pos) * std::f64::consts::PI / 12.0).tan();
            Some(((k / (1.0 + k)) as f32, (1.0 / (1.0 + k)) as f32))
        }
    };
    let apply = |xr: &mut [[f32; 576]; 2], from: usize, to: usize, pos: u8, max: u8| match factors(
        pos, max,
    ) {
        Some((kl, kr)) => {
            for i in from..to {
                let l = xr[0][i];
                xr[0][i] = l * kl;
                xr[1][i] = l * kr;
            }
        }
        None => ms_lines(xr, from, to),
    };
    let right_nz =
        |xr: &[[f32; 576]; 2], from: usize, to: usize| xr[1][from..to].iter().any(|&v| v != 0.0);
    if right.block_type == 2 {
        let (long_end, short_start) = mixed_split(right, rb);
        // Long part of a mixed block: middle/side only.
        ms_lines(xr, 0, rb.long[long_end]);
        for w in 0..3 {
            // The intensity bands of a window are those above its last nonzero band.
            let mut first_is = short_start;
            for sfb in (short_start..13).rev() {
                let width = rb.short[sfb + 1] - rb.short[sfb];
                let base = 3 * rb.short[sfb] + w * width;
                if right_nz(xr, base, base + width) {
                    first_is = sfb + 1;
                    break;
                }
            }
            for sfb in short_start..13 {
                let width = rb.short[sfb + 1] - rb.short[sfb];
                let base = 3 * rb.short[sfb] + w * width;
                if sfb < first_is {
                    ms_lines(xr, base, base + width);
                } else {
                    let s = sfb.min(11);
                    let k = long_end + 3 * (s - short_start) + w;
                    let pos = sf_right[k.min(39)];
                    let max = if lsf { is_max[k.min(39)] } else { 7 };
                    apply(xr, base, base + width, pos, max);
                }
            }
        }
        return;
    }
    // Long blocks: the bands above the right channel's last nonzero line.
    let last = xr[1][..nonzero[1]].iter().rposition(|&v| v != 0.0);
    let first_is = match last {
        Some(i) => (0..22).find(|&s| rb.long[s + 1] > i).map_or(22, |s| s + 1),
        None => 0,
    };
    for sfb in 0..22 {
        let (a, b) = (rb.long[sfb], rb.long[sfb + 1]);
        if sfb < first_is {
            ms_lines(xr, a, b);
        } else {
            let s = sfb.min(20);
            let max = if lsf { is_max[s] } else { 7 };
            apply(xr, a, b, sf_right[s], max);
        }
    }
}

/// Short blocks' lines from band-window order to subband-window order for the IMDCT.
fn reorder(c: &Granule, rb: &Bands, xr: &mut [f32; 576]) {
    let (long_end, short_start) = mixed_split(c, rb);
    let start = rb.long[long_end];
    let mut tmp = [0f32; 576];
    for sfb in short_start..13 {
        let s = rb.short[sfb];
        let width = rb.short[sfb + 1] - s;
        for w in 0..3 {
            for i in 0..width {
                let f = s + i;
                let src = 3 * s + w * width + i;
                let dst = (f / 6) * 18 + w * 6 + f % 6;
                if src < 576 && dst < 576 {
                    tmp[dst] = xr[src];
                }
            }
        }
    }
    xr[start..].copy_from_slice(&tmp[start..]);
}

fn antialias(c: &Granule, xr: &mut [f32; 576], t: &Tables) {
    let bands = match (c.block_type, c.mixed) {
        (2, false) => return,
        (2, true) => 1,
        _ => 31,
    };
    for sb in 1..=bands {
        for (i, &(cs, ca)) in t.alias.iter().enumerate() {
            let a = xr[18 * sb - 1 - i];
            let b = xr[18 * sb + i];
            xr[18 * sb - 1 - i] = a * cs - b * ca;
            xr[18 * sb + i] = b * cs + a * ca;
        }
    }
}

fn imdct(
    c: &Granule,
    xr: &[f32; 576],
    overlap: &mut [[f32; 18]; 32],
    sub: &mut [[f32; 32]; 18],
    t: &Tables,
) {
    for sb in 0..32 {
        let x = &xr[18 * sb..18 * sb + 18];
        let bt = if c.mixed && sb < 2 { 0 } else { c.block_type };
        let mut y = [0f32; 36];
        if bt != 2 {
            if x.iter().any(|&v| v != 0.0) {
                let w = &t.win[usize::from(bt)];
                for i in 0..36 {
                    let row = &t.cos_long[i];
                    let mut s = 0f32;
                    for k in 0..18 {
                        s += x[k] * row[k];
                    }
                    y[i] = s * w[i];
                }
            }
        } else {
            for w in 0..3 {
                let xs = &x[6 * w..6 * w + 6];
                if xs.iter().all(|&v| v == 0.0) {
                    continue;
                }
                for i in 0..12 {
                    let row = &t.cos_short[i];
                    let mut s = 0f32;
                    for k in 0..6 {
                        s += xs[k] * row[k];
                    }
                    y[6 + 6 * w + i] += s * t.win_short[i];
                }
            }
        }
        let o = &mut overlap[sb];
        for i in 0..18 {
            let mut v = y[i] + o[i];
            // Frequency inversion of the odd subbands.
            if sb & 1 == 1 && i & 1 == 1 {
                v = -v;
            }
            sub[i][sb] = v;
            o[i] = y[18 + i];
        }
    }
}
