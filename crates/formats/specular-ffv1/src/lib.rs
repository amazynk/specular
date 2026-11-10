//! FFV1 (RFC 9043), the lossless intra-frame video coding of archives: versions 0, 1 and 3,
//! the range coder (default or custom state transitions) and Golomb-Rice coding, 8 to 16
//! bits, YCbCr (greyscale, 4:4:4, 4:4:0, 4:2:2, 4:2:0, 4:1:1) or RGB through the JPEG 2000
//! reversible colour transform, each with an optional transparency plane.
//!
//! A frame is cut into slices, each with its own header, contexts and (version 3) CRC, so
//! the slices decode in parallel on the rayon pool. Frames that are not keyframes keep the
//! contexts their slice had in the frame before; their pictures still stand alone.
//!
//! The output is planar: 8-bit samples as bytes, deeper ones as little-endian `u16`. RGB comes
//! out as G, B, R planes (with alpha beside them).
//!
//! Refused by name: version 2 (experimental) and version 4 (in development: float samples,
//! remapping), and 4:1:0 chroma.

mod coder;
mod slice;

use std::fmt;

use rayon::prelude::*;

pub use coder::crc;
use coder::{RangeDecoder, States, Transitions};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// Not an FFV1 frame or configuration, or one that contradicts itself.
    Malformed(String),
    /// A valid stream this decoder does not read.
    Unsupported(String),
    /// The memory budget cannot hold the frame.
    NeedsMemory(specular_core::NeedsMemory),
}

impl From<specular_core::NeedsMemory> for Error {
    fn from(n: specular_core::NeedsMemory) -> Self {
        Error::NeedsMemory(n)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Malformed(s) => write!(f, "malformed FFV1: {s}"),
            Error::Unsupported(s) => write!(f, "unsupported FFV1: {s}"),
            Error::NeedsMemory(n) => write!(f, "FFV1 frame: {n}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T, E = Error> = std::result::Result<T, E>;

fn bad(s: impl Into<String>) -> Error {
    Error::Malformed(s.into())
}

/// The colour space a stream codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColourSpace {
    Ycbcr,
    /// RGB through the JPEG 2000 reversible colour transform.
    Rgb,
}

/// The five quantisation tables of one set and how many contexts they make.
#[derive(Clone, Debug)]
pub(crate) struct QuantSet {
    pub tables: Box<[[i32; 256]; 5]>,
    pub context_count: usize,
    /// Tables 3 and 4 (the samples two away) take part.
    pub far: bool,
}

/// The stream's parameters (RFC 9043 §4.2): from the configuration record (version 3) or
/// from every keyframe (versions 0 and 1).
#[derive(Clone, Debug)]
pub struct Params {
    pub version: u32,
    pub micro_version: u32,
    /// 0 Golomb-Rice, 1 range coder, 2 range coder with a custom state table.
    pub coder_type: u32,
    pub colour_space: ColourSpace,
    pub bits: u32,
    pub chroma_planes: bool,
    pub log2_h: u32,
    pub log2_v: u32,
    pub extra_plane: bool,
    pub num_h_slices: u32,
    pub num_v_slices: u32,
    /// Slice CRCs (`ec`).
    pub ec: bool,
    /// Every frame is a keyframe.
    pub intra: bool,
    pub(crate) transitions: Transitions,
    pub(crate) quant: Vec<QuantSet>,
    pub(crate) initial: Vec<Option<Vec<States>>>,
}

impl Params {
    /// "version 3, range coder, 10-bit 4:2:2, alpha, 4 slices, CRC".
    pub fn describe(&self) -> String {
        let mut s = format!(
            "version {}, {}",
            self.version,
            if self.coder_type == 0 {
                "Golomb-Rice"
            } else {
                "range coder"
            }
        );
        let layout = match self.colour_space {
            ColourSpace::Rgb => "RGB",
            ColourSpace::Ycbcr if !self.chroma_planes => "greyscale",
            ColourSpace::Ycbcr => match (self.log2_h, self.log2_v) {
                (0, 0) => "4:4:4",
                (0, 1) => "4:4:0",
                (1, 0) => "4:2:2",
                (1, 1) => "4:2:0",
                (2, 0) => "4:1:1",
                _ => "4:1:0",
            },
        };
        s.push_str(&format!(", {}-bit {layout}", self.bits));
        if self.extra_plane {
            s.push_str(", alpha");
        }
        let n = self.num_h_slices * self.num_v_slices;
        if n > 1 {
            s.push_str(&format!(", {n} slices"));
        }
        if self.ec {
            s.push_str(", CRC");
        }
        s
    }

    /// Planes coded per slice (luma, two chroma, transparency).
    fn plane_count(&self) -> usize {
        1 + if self.chroma_planes { 2 } else { 0 } + usize::from(self.extra_plane)
    }

    /// Context sets per slice: luma, chroma (kept for versions up to 3 even in greyscale),
    /// transparency.
    fn context_sets(&self) -> usize {
        1 + usize::from(self.chroma_planes || self.version <= 3) + usize::from(self.extra_plane)
    }
}

/// Reads the parameters (RFC 9043 Figure 28). `record`: from a configuration record.
fn read_params(c: &mut RangeDecoder<'_>, record: bool) -> Result<Params> {
    let t = Transitions::default();
    let mut st: States = [128; 32];
    let version = c.unsigned(&mut st, &t);
    match version {
        0 | 1 if record => return Err(bad("a configuration record for version 0 or 1")),
        0 | 1 => {}
        2 => return Err(Error::Unsupported("FFV1 version 2 (experimental)".into())),
        3 if !record => return Err(bad("version 3 without a configuration record")),
        3 => {}
        v => return Err(Error::Unsupported(format!("FFV1 version {v}"))),
    }
    let micro_version = if version >= 3 {
        c.unsigned(&mut st, &t)
    } else {
        0
    };
    let coder_type = c.unsigned(&mut st, &t);
    let transitions = match coder_type {
        0 | 1 => Transitions::default(),
        2 => {
            let mut delta = [0i32; 256];
            for d in delta.iter_mut().skip(1) {
                *d = c.symbol(&mut st, &t, true);
            }
            Transitions::custom(&delta)
        }
        n => return Err(Error::Unsupported(format!("FFV1 coder type {n}"))),
    };
    let colour_space = match c.unsigned(&mut st, &t) {
        0 => ColourSpace::Ycbcr,
        1 => ColourSpace::Rgb,
        n => return Err(Error::Unsupported(format!("FFV1 colour space {n}"))),
    };
    let bits = if version >= 1 {
        match c.unsigned(&mut st, &t) {
            0 => 8,
            b => b,
        }
    } else {
        8
    };
    let chroma_planes = c.bit(&mut st[0], &t);
    let log2_h = c.unsigned(&mut st, &t);
    let log2_v = c.unsigned(&mut st, &t);
    let extra_plane = c.bit(&mut st[0], &t);
    let (mut num_h_slices, mut num_v_slices, mut sets) = (1, 1, 1);
    if version >= 3 {
        num_h_slices = c.unsigned(&mut st, &t).saturating_add(1);
        num_v_slices = c.unsigned(&mut st, &t).saturating_add(1);
        sets = c.unsigned(&mut st, &t);
    }
    if c.overflow {
        return Err(bad("parameters"));
    }
    if !(1..=16).contains(&bits) {
        return Err(Error::Unsupported(format!("{bits}-bit FFV1")));
    }
    if bits < 8 {
        return Err(Error::Unsupported(format!("{bits}-bit FFV1")));
    }
    match colour_space {
        ColourSpace::Rgb if !chroma_planes || log2_h != 0 || log2_v != 0 => {
            return Err(bad("RGB with subsampled or missing planes"));
        }
        _ => {}
    }
    if chroma_planes {
        match (log2_h, log2_v) {
            (0, 0) | (0, 1) | (1, 0) | (1, 1) | (2, 0) => {}
            (2, 2) => return Err(Error::Unsupported("FFV1 with 4:1:0 chroma".into())),
            _ => {
                return Err(Error::Unsupported(
                    "FFV1 with this chroma subsampling".into(),
                ));
            }
        }
    }
    if !(1..=8).contains(&sets) {
        return Err(bad(format!("{sets} quantisation table sets")));
    }
    if num_h_slices > 64 || num_v_slices > 64 {
        return Err(bad("slice raster"));
    }
    let mut quant = Vec::with_capacity(sets as usize);
    for _ in 0..sets {
        quant.push(read_quant_set(c, &t)?);
    }
    let mut initial = vec![None; sets as usize];
    let (mut ec, mut intra) = (false, false);
    if version >= 3 {
        let mut by_k = [[128u8; 32]; 32];
        for (i, q) in quant.iter().enumerate() {
            if c.bit(&mut st[0], &t) {
                let mut states: Vec<States> = vec![[128; 32]; q.context_count];
                for j in 0..q.context_count {
                    for k in 0..32 {
                        let pred = if j > 0 { states[j - 1][k] } else { 128 };
                        let d = c.symbol(&mut by_k[k], &t, true);
                        states[j][k] = (i32::from(pred) + d) as u8;
                    }
                }
                initial[i] = Some(states);
            }
        }
        ec = match c.unsigned(&mut st, &t) {
            0 => false,
            1 => true,
            n => {
                return Err(Error::Unsupported(format!(
                    "FFV1 error correction type {n}"
                )));
            }
        };
        if micro_version > 2 {
            intra = c.unsigned(&mut st, &t) == 1;
        }
    }
    if c.overflow {
        return Err(bad("parameters"));
    }
    Ok(Params {
        version,
        micro_version,
        coder_type,
        colour_space,
        bits,
        chroma_planes,
        log2_h,
        log2_v,
        extra_plane,
        num_h_slices,
        num_v_slices,
        ec,
        intra,
        transitions,
        quant,
        initial,
    })
}

/// One quantisation table set (RFC 9043 §4.1): five tables, each with its own states.
fn read_quant_set(c: &mut RangeDecoder<'_>, t: &Transitions) -> Result<QuantSet> {
    let mut tables = Box::new([[0i32; 256]; 5]);
    let mut scale: i64 = 1;
    for table in tables.iter_mut() {
        let mut st: States = [128; 32];
        let mut v: i64 = 0;
        let mut k = 0usize;
        while k < 128 {
            let len = c.unsigned(&mut st, t) as usize + 1;
            if len > 128 - k || c.overflow {
                return Err(bad("quantisation table"));
            }
            for e in &mut table[k..k + len] {
                *e = (scale * v) as i32;
            }
            k += len;
            v += 1;
        }
        for k in 1..128 {
            table[256 - k] = -table[k];
        }
        table[128] = -table[127];
        scale *= 2 * v - 1;
        if scale > 65_536 {
            return Err(bad("quantisation tables with too many contexts"));
        }
    }
    let context_count = ((scale + 1) / 2) as usize;
    let far = tables[3][127] != 0 || tables[4][127] != 0;
    Ok(QuantSet {
        tables,
        context_count,
        far,
    })
}

/// Reads a configuration record (version 3: Matroska `CodecPrivate`, MP4 `glbl`, the AVI
/// format's extra bytes), its CRC checked.
pub fn parse_config(record: &[u8]) -> Result<Params> {
    if record.len() < 4 {
        return Err(bad("configuration record"));
    }
    if crc(record) != 0 {
        return Err(bad("configuration record CRC"));
    }
    let mut c = RangeDecoder::new(&record[..record.len() - 4]);
    read_params(&mut c, true)
}

/// The parameters a stream's first keyframe states (versions 0 and 1 carry them in it).
pub fn probe(config: &[u8], first_frame: &[u8]) -> Result<Params> {
    if config.len() >= 4 {
        return parse_config(config);
    }
    let mut c = RangeDecoder::new(first_frame);
    let t = Transitions::default();
    let mut ks = 128u8;
    if !c.bit(&mut ks, &t) {
        return Err(bad("the first frame is not a keyframe"));
    }
    read_params(&mut c, false)
}

/// How the planes of a [`Picture`] are laid out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// Y, Cb, Cr with chroma `ceil(w >> h)` × `ceil(h >> v)`.
    Ycbcr { log2_h: u32, log2_v: u32 },
    /// Y only (the other two planes are empty).
    Grey,
    /// G, B, R at full size.
    Rgb,
}

/// A decoded frame.
#[derive(Debug)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub bits: u32,
    pub layout: Layout,
    /// Planes in the layout's order; samples as bytes up to 8 bits, else little-endian `u16`.
    pub planes: [Vec<u8>; 3],
    /// The transparency plane, at full size and the same depth.
    pub alpha: Option<Vec<u8>>,
    pub keyframe: bool,
    /// Slices lost to a bad CRC or damaged data (their area keeps the frame before's picture,
    /// or black).
    pub damaged_slices: u32,
    /// RFC 9043 `picture_structure` of the first slice: 0 unknown, 1 top field first,
    /// 2 bottom field first, 3 progressive.
    pub picture_structure: u32,
    /// The sample aspect ratio (0/0 when unknown).
    pub sar: (u32, u32),
}

/// The decoder of one track: the stream's parameters and every slice's contexts.
pub struct Decoder {
    width: u32,
    height: u32,
    params: Option<Params>,
    from_record: bool,
    slices: Vec<slice::SliceCtx>,
    /// Slices in parallel.
    pub parallel: bool,
}

impl Decoder {
    /// A decoder for frames of `width`×`height` (the container says), with the track's
    /// configuration record (empty for versions 0 and 1).
    pub fn new(width: u32, height: u32, config: &[u8]) -> Result<Decoder> {
        if width == 0 || height == 0 || width > 32_768 || height > 32_768 {
            return Err(bad(format!("frame size {width}×{height}")));
        }
        let params = if config.len() >= 4 {
            Some(parse_config(config)?)
        } else {
            None
        };
        Ok(Decoder {
            width,
            height,
            from_record: params.is_some(),
            params,
            slices: Vec::new(),
            parallel: true,
        })
    }

    /// The stream's parameters, once known.
    pub fn params(&self) -> Option<&Params> {
        self.params.as_ref()
    }

    /// Forgets the contexts (decoding goes on from a keyframe).
    pub fn reset(&mut self) {
        self.slices.clear();
        if !self.from_record {
            self.params = None;
        }
    }

    /// Decodes one frame.
    pub fn decode(&mut self, frame: &[u8]) -> Result<Picture> {
        if frame.len() < 2 {
            return Err(bad("empty frame"));
        }
        let dt = Transitions::default();
        // Slices as (start, end of coded bytes, end with footer).
        let mut regions: Vec<(usize, usize, usize)> = Vec::new();
        let v3 = self.params.as_ref().is_some_and(|p| p.version >= 3);
        if v3 {
            let p = self.params.as_ref().unwrap_or_else(|| unreachable!());
            let trailer = 3 + if p.ec { 5 } else { 0 };
            let max = (p.num_h_slices * p.num_v_slices) as usize;
            let mut end = frame.len();
            while end > 0 && regions.len() < max {
                if end < trailer {
                    return Err(bad("slice footer"));
                }
                let f = &frame[end - trailer..];
                let size = (usize::from(f[0]) << 16) | (usize::from(f[1]) << 8) | usize::from(f[2]);
                let coded_end = end - trailer;
                if size > coded_end {
                    return Err(bad("slice size"));
                }
                let start = if regions.len() + 1 == max {
                    0
                } else {
                    coded_end - size
                };
                regions.push((start, coded_end, end));
                end = start;
            }
            if let Some(first) = regions.last_mut() {
                first.0 = 0;
            }
            regions.reverse();
        } else {
            regions.push((0, frame.len(), frame.len()));
        }
        // The keyframe flag (and for versions 0 and 1 the parameters) open the first slice.
        let first_end = regions[0].1;
        let mut c0 = RangeDecoder::new(&frame[..first_end]);
        let mut ks = 128u8;
        let keyframe = c0.bit(&mut ks, &dt);
        if !v3 {
            if keyframe {
                let p = read_params(&mut c0, false)?;
                if self
                    .params
                    .as_ref()
                    .is_some_and(|o| o.plane_count() != p.plane_count())
                {
                    self.slices.clear();
                }
                self.params = Some(p);
            } else if self.params.is_none() {
                return Err(bad(
                    "a frame that is not a keyframe, with no keyframe before",
                ));
            }
        }
        let p = self.params.clone().ok_or_else(|| bad("no parameters"))?;
        if !keyframe && (p.intra || self.slices.is_empty()) {
            return Err(bad(
                "a frame that is not a keyframe, with no keyframe before",
            ));
        }
        let (w, h) = (self.width as usize, self.height as usize);
        let plane_bytes = if p.bits > 8 { 2 } else { 1 };
        let planes = p.plane_count();
        // The output planes and every slice's working samples (32-bit, with borders).
        let _held = specular_core::budget::reserve(
            (w * h * plane_bytes * (planes + 1) + (w + 8) * (h + 8) * 4 * planes) as u64,
        )?;
        if self.slices.len() < regions.len() {
            self.slices
                .resize_with(regions.len(), slice::SliceCtx::default);
        }
        // Every slice's range decoder: the first goes on from the keyframe flag.
        let mut coders = Vec::with_capacity(regions.len());
        coders.push(c0);
        for &(start, end, _) in &regions[1..] {
            coders.push(RangeDecoder::new(&frame[start..end]));
        }
        let frame_size = (self.width, self.height);
        let one = |((c, &(start, end, full_end)), ctx): (
            (RangeDecoder<'_>, &(usize, usize, usize)),
            &mut slice::SliceCtx,
        )| {
            if p.ec && crc(&frame[start..full_end]) != 0 {
                return slice::SliceOut::damaged();
            }
            slice::decode(&p, frame_size, keyframe, c, &frame[start..end], ctx)
        };
        let ctxs = &mut self.slices[..regions.len()];
        let outs: Vec<slice::SliceOut> = if self.parallel && regions.len() > 1 {
            coders
                .into_par_iter()
                .zip(regions.par_iter())
                .zip(ctxs.par_iter_mut())
                .map(one)
                .collect()
        } else {
            coders
                .into_iter()
                .zip(regions.iter())
                .zip(ctxs.iter_mut())
                .map(one)
                .collect()
        };
        slice::assemble(&p, frame_size, keyframe, outs)
    }
}
