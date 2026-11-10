//! Specular's own VC-1 video decoder (SMPTE ST 421:2013): Windows Media Video 9 (WMV3, Simple
//! and Main profile) and VC-1 Advanced profile (WVC1), as found in ASF/WMV, Matroska, MP4,
//! AVI, transport streams and Blu-ray.
//!
//! - Simple and Main profile: I, P, B and BI pictures, skipped pictures, 1-MV and mixed 1/4-MV
//!   P pictures with hybrid prediction, quarter-pel bicubic and half-pel bilinear motion,
//!   intensity compensation, range reduction, variable-size transforms, macroblock
//!   quantization, overlap smoothing, the loop filter, sync markers.
//! - Advanced profile progressive pictures: the above plus slices, conditional overlap, range
//!   mapping, pan-scan and pull-down fields (skipped), in-band sequence and entry-point headers.
//! - Refused in words: interlaced field and frame pictures (until supported), multi-resolution
//!   pictures, X8 intra and pre-release WMV9 bitstreams.
//!
//! Every table is generated from the standard's text by `tools/corpus/specular-vc1-tables.py`.
//! Pictures come out in display order, each with the tag of its coded frame.
#![deny(unsafe_code)]
#![allow(clippy::needless_range_loop)]

mod bitplane;
mod bits;
mod coefs;
mod decode;
mod filter;
mod mc;
mod picture;
mod seq;
mod small;
// Generated; the interlaced scans wait for interlaced pictures.
#[rustfmt::skip]
#[allow(dead_code)]
mod tables;
mod transform;
mod vlc;

use std::collections::VecDeque;
use std::sync::Arc;

use bits::Bits;
use decode::{Mv, Pic, Refs, State};
use mc::{Edges, Frame};
use picture::{PicHeader, PicType};
use seq::{Profile, Seq};

/// Why a stream can't be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Unsupported(String),
    Malformed(String),
    NeedsMemory(specular_core::NeedsMemory),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported(s) => write!(f, "{s}"),
            Error::Malformed(s) => write!(f, "damaged VC-1 video: {s}"),
            Error::NeedsMemory(n) => write!(f, "this video {n}"),
        }
    }
}

impl std::error::Error for Error {}

pub(crate) fn malformed(s: &str) -> Error {
    Error::Malformed(s.into())
}

pub(crate) fn unsupported(s: &str) -> Error {
    Error::Unsupported(s.into())
}

/// How the stream's sequence parameters come.
#[derive(Clone, Copy, Debug)]
pub enum Config<'a> {
    /// WMV3 (Simple/Main): STRUCT_C (the 4-byte sequence header from ASF/AVI/Matroska
    /// extradata or RCV) and the frame size from the container.
    SimpleMain {
        struct_c: &'a [u8],
        width: u32,
        height: u32,
    },
    /// WVC1 / Advanced: the sequence header and entry point as start-code EBDUs (ASF/Matroska
    /// extradata, MP4 `dvc1` box payload after its fixed fields); may be empty if they come
    /// in-band.
    Advanced { headers: &'a [u8] },
}

/// What a stream's configuration says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeqInfo {
    /// "Simple", "Main" or "Advanced".
    pub profile: &'static str,
    pub level: Option<u8>,
    pub width: u32,
    pub height: u32,
    pub interlaced: bool,
}

/// The sequence parameters of a configuration.
pub fn probe(config: &Config) -> Result<SeqInfo, Error> {
    let s = parse_config(config)?.ok_or_else(|| malformed("no sequence header"))?;
    Ok(SeqInfo {
        profile: match s.profile {
            Profile::Simple => "Simple",
            Profile::Main => "Main",
            Profile::Advanced => "Advanced",
        },
        level: (s.profile == Profile::Advanced).then_some(s.level),
        width: s.width,
        height: s.height,
        interlaced: s.interlace,
    })
}

fn parse_config(config: &Config) -> Result<Option<Seq>, Error> {
    match *config {
        Config::SimpleMain {
            struct_c,
            width,
            height,
        } => Seq::struct_c(struct_c, width, height).map(Some),
        Config::Advanced { headers } => {
            let mut seq: Option<Seq> = None;
            for (kind, payload) in bdus(headers) {
                let rb = unescape(payload);
                match kind {
                    0x0F => seq = Some(Seq::advanced(&rb)?),
                    0x0E => {
                        if let Some(s) = seq.as_mut() {
                            s.entry_point(&rb)?;
                        }
                    }
                    _ => {}
                }
            }
            Ok(seq)
        }
    }
}

/// The order pictures come out in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputOrder {
    /// Pictures in display order, each with the tag of its own coded frame.
    Decode,
    /// The same; the caller ignores tags (AVI time slots).
    Display,
}

/// A decoded picture: 4:2:0, cropped to the coded size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    /// Y, Cb, Cr, rows tightly packed (chroma `ceil(width / 2)` × `ceil(height / 2)`).
    pub planes: [Vec<u8>; 3],
    pub tag: i64,
    /// Coded as fields or an interlaced frame.
    pub interlaced: bool,
    pub top_field_first: bool,
    /// H.273 colour primaries, transfer and matrix from the sequence header.
    pub colour: Option<(u8, u8, u8)>,
    pub full_range: bool,
}

/// A reference picture kept for prediction.
#[derive(Clone)]
struct Anchor {
    frame: Arc<Frame>,
    /// Direct-mode vectors per macroblock (None: an I picture).
    direct: Option<Arc<Vec<Mv>>>,
    rangered: bool,
}

/// A VC-1 decoder for one stream.
pub struct Decoder {
    seq: Option<Seq>,
    st: State,
    /// The older and the newer anchor.
    prev: Option<Anchor>,
    last: Option<Anchor>,
    /// The newest anchor's picture, held until the next anchor in display order.
    held: Option<Picture>,
    out: VecDeque<Picture>,
    rnd: bool,
    /// Simple/Main: B pictures possible, so anchors are held.
    reorder: bool,
    _reservation: Option<specular_core::budget::Reservation>,
    reserved_for: (usize, usize),
    /// Recycled frame buffers.
    pool: Vec<Frame>,
}

impl Decoder {
    pub fn new(config: &Config, order: OutputOrder) -> Result<Decoder, Error> {
        let _ = order;
        let seq = parse_config(config)?;
        let reorder = seq
            .as_ref()
            .is_none_or(|s| s.profile == Profile::Advanced || s.max_b_frames > 0);
        let mut d = Decoder {
            seq,
            st: State::default(),
            prev: None,
            last: None,
            held: None,
            out: VecDeque::new(),
            rnd: false,
            reorder,
            _reservation: None,
            reserved_for: (0, 0),
            pool: Vec::new(),
        };
        if let Some(s) = d.seq.clone() {
            d.size_for(&s)?;
        }
        Ok(d)
    }

    /// Reserves memory and resets the per-picture state for the sequence's size.
    fn size_for(&mut self, s: &Seq) -> Result<(), Error> {
        let (w, h) = (s.mb_width(), s.mb_height());
        if (w, h) == self.reserved_for {
            return Ok(());
        }
        // Five frames (two anchors, the picture being decoded, a held output and a spare)
        // plus the macroblock state and the overlap buffers.
        let bytes = 5 * Frame::bytes(w, h) + State::bytes(w, h);
        self._reservation = None;
        let res = specular_core::budget::reserve(bytes).map_err(Error::NeedsMemory)?;
        self._reservation = Some(res);
        self.reserved_for = (w, h);
        self.st.reset(w, h);
        self.st.ovl = [
            vec![0; w * 16 * h * 16],
            vec![0; w * 8 * h * 8],
            vec![0; w * 8 * h * 8],
        ];
        self.pool.clear();
        self.prev = None;
        self.last = None;
        Ok(())
    }

    /// Decodes one coded frame (Simple/Main) or access unit (Advanced, with start codes).
    pub fn decode(&mut self, frame: &[u8], tag: i64) -> Result<(), Error> {
        let advanced = match &self.seq {
            Some(s) => s.profile == Profile::Advanced,
            None => true,
        };
        if !advanced {
            return self.decode_simple_main(frame, tag);
        }
        self.decode_advanced(frame, tag)
    }

    fn decode_simple_main(&mut self, frame: &[u8], tag: i64) -> Result<(), Error> {
        let seq = self
            .seq
            .clone()
            .ok_or_else(|| malformed("no sequence header"))?;
        if frame.len() <= 1 {
            return self.skipped(&seq, tag);
        }
        let mut r = Bits::new(frame);
        let h = picture::parse_simple_main(&mut r, &seq, seq.mb_width(), seq.mb_height())?;
        if h.respic != 0 {
            return Err(unsupported("VC-1 multi-resolution pictures"));
        }
        self.picture(&seq, &h, frame, &[], tag)
    }

    fn decode_advanced(&mut self, data: &[u8], tag: i64) -> Result<(), Error> {
        let units: Vec<(u8, &[u8])> = if data.len() >= 4 && data[..3] == [0, 0, 1] {
            bdus(data).collect()
        } else {
            vec![(0x0D, data)]
        };
        let mut frame: Option<Vec<u8>> = None;
        let mut slices: Vec<Vec<u8>> = Vec::new();
        for (kind, payload) in units {
            match kind {
                0x0F => {
                    let s = Seq::advanced(&unescape(payload))?;
                    self.seq = Some(s);
                }
                0x0E => {
                    let s = self
                        .seq
                        .as_mut()
                        .ok_or_else(|| malformed("an entry point before the sequence header"))?;
                    s.entry_point(&unescape(payload))?;
                    let s = s.clone();
                    self.size_for(&s)?;
                }
                0x0D => frame = Some(unescape(payload)),
                0x0B => slices.push(unescape(payload)),
                0x0C => return Err(unsupported("VC-1 interlaced field pictures")),
                _ => {}
            }
        }
        let Some(frame) = frame else {
            return Ok(());
        };
        let seq = self
            .seq
            .clone()
            .ok_or_else(|| malformed("no sequence header"))?;
        if !seq.entry_seen {
            return Err(malformed("no entry-point header"));
        }
        let mut r = Bits::new(&frame);
        let h = picture::parse_advanced(&mut r, &seq, seq.mb_width(), seq.mb_height())?;
        if h.ptype == PicType::Skipped {
            return self.skipped(&seq, tag);
        }
        let slice_refs: Vec<&[u8]> = slices.iter().map(|s| s.as_slice()).collect();
        self.picture(&seq, &h, &frame, &slice_refs, tag)
    }

    /// A skipped picture: the last anchor again, with zero motion.
    fn skipped(&mut self, seq: &Seq, tag: i64) -> Result<(), Error> {
        let Some(last) = self.last.clone() else {
            return Ok(());
        };
        let n = seq.mb_width() * seq.mb_height();
        let anchor = Anchor {
            frame: last.frame.clone(),
            direct: Some(Arc::new(vec![Some([0, 0]); n])),
            rangered: last.rangered,
        };
        let pic = self.output_picture(seq, &last.frame, last.rangered, tag);
        self.push_anchor(anchor, pic);
        Ok(())
    }

    fn take_frame(&mut self, seq: &Seq) -> Frame {
        self.pool
            .pop()
            .unwrap_or_else(|| Frame::new(seq.mb_width(), seq.mb_height()))
    }

    fn recycle(&mut self, a: Option<Anchor>) {
        if let Some(a) = a
            && let Ok(f) = Arc::try_unwrap(a.frame)
            && self.pool.len() < 2
        {
            self.pool.push(f);
        }
    }

    fn picture(
        &mut self,
        seq: &Seq,
        h: &PicHeader,
        frame: &[u8],
        slices: &[&[u8]],
        tag: i64,
    ) -> Result<(), Error> {
        let (mb_w, mb_h) = (seq.mb_width(), seq.mb_height());
        if (mb_w, mb_h) != self.reserved_for {
            self.size_for(seq)?;
        }
        // Rounding control (§8.3.7).
        let rnd = if seq.profile == Profile::Advanced {
            h.rnd
        } else {
            match h.ptype {
                PicType::I | PicType::Bi => {
                    self.rnd = true;
                    true
                }
                PicType::P => {
                    self.rnd = !self.rnd;
                    self.rnd
                }
                _ => self.rnd,
            }
        };
        let edges = if seq.profile == Profile::Advanced {
            let (w, hh) = (seq.width as usize, seq.height as usize);
            Edges {
                w: [w, w.div_ceil(2), w.div_ceil(2)],
                h: [hh, hh.div_ceil(2), hh.div_ceil(2)],
            }
        } else {
            Edges {
                w: [16 * mb_w, 8 * mb_w, 8 * mb_w],
                h: [16 * mb_h, 8 * mb_h, 8 * mb_h],
            }
        };
        // References, rescaled for range reduction and remapped for intensity compensation.
        let mut fwd_frame: Option<Arc<Frame>> = None;
        let mut bwd_frame: Option<Arc<Frame>> = None;
        let mut direct: Option<Arc<Vec<Mv>>> = None;
        match h.ptype {
            PicType::P => {
                let Some(last) = self.last.as_ref() else {
                    // No reference: the picture can't be decoded.
                    return Ok(());
                };
                let mut f = last.frame.clone();
                if seq.rangered && last.rangered != h.rangeredfrm {
                    f = Arc::new(rescale(&f, h.rangeredfrm));
                }
                if let Some((scale, shift)) = h.intensity {
                    let (ly, luv) = intensity_luts(scale, shift);
                    let m = Arc::new(f.mapped(&ly, &luv));
                    // B pictures after this P picture predict from the remapped anchor too.
                    if let Some(l) = self.last.as_mut() {
                        l.frame = Arc::new(last_mapped(&l.frame, &ly, &luv));
                    }
                    f = m;
                }
                fwd_frame = Some(f);
            }
            PicType::B => {
                let (Some(prev), Some(last)) = (self.prev.as_ref(), self.last.as_ref()) else {
                    return Ok(());
                };
                let mut f = prev.frame.clone();
                if seq.rangered && prev.rangered != h.rangeredfrm {
                    f = Arc::new(rescale(&f, h.rangeredfrm));
                }
                fwd_frame = Some(f);
                bwd_frame = Some(last.frame.clone());
                direct = last.direct.clone();
            }
            _ => {}
        }
        let mut cur = self.take_frame(seq);
        {
            let refs = Refs {
                fwd: fwd_frame.as_deref(),
                bwd: bwd_frame.as_deref(),
                direct: direct.as_deref().map(|v| v.as_slice()),
                edges,
            };
            if decode::needs_ovl(seq, h) {
                for v in self.st.ovl_intra.iter_mut() {
                    *v = [false; 6];
                }
            }
            let mut p = Pic::new(seq, h, &mut self.st, &mut cur, refs, rnd);
            if h.ptype == PicType::B && direct.is_none() {
                // The next anchor is an I picture: all direct vectors are zero.
            }
            let mut r = Bits::new(frame);
            r.set_position(h.mb_start);
            // Slices split the rows (Advanced).
            let mut starts: Vec<(usize, Vec<u8>, usize)> = Vec::new();
            for s in slices {
                let mut sr = Bits::new(s);
                let addr = sr.u(9) as usize;
                if sr.flag() {
                    // A repeated picture header: parse it to skip it.
                    picture::parse_advanced(&mut sr, seq, mb_w, mb_h)?;
                }
                if addr > 0 && addr < mb_h {
                    starts.push((addr, s.to_vec(), sr.position()));
                }
            }
            starts.sort_by_key(|s| s.0);
            let first_end = starts.first().map_or(mb_h, |s| s.0);
            let res = p.decode_rows(&mut r, 0..first_end, false);
            let mut res = res;
            for (k, (addr, data, pos)) in starts.iter().enumerate() {
                if res.is_err() {
                    break;
                }
                let end = starts.get(k + 1).map_or(mb_h, |s| s.0);
                let mut sr = Bits::new(data);
                sr.set_position(*pos);
                res = p.decode_rows(&mut sr, *addr..end, true);
            }
            if let Err(e) = res {
                // A damaged picture is still finished from what decoded.
                match e {
                    Error::Malformed(_) => {}
                    other => return Err(other),
                }
            }
            p.finish_overlap();
            p.loop_filter();
        }
        let cur = Arc::new(cur);
        let pic = self.output_picture(seq, &cur, h.rangeredfrm, tag);
        match h.ptype {
            PicType::I | PicType::P => {
                let anchor = Anchor {
                    frame: cur,
                    direct: (h.ptype == PicType::P).then(|| Arc::new(self.st.direct.clone())),
                    rangered: h.rangeredfrm,
                };
                self.push_anchor(anchor, pic);
            }
            _ => {
                // B and BI pictures are shown at once, between the anchors.
                self.out.push_back(pic);
                self.recycle(Some(Anchor {
                    frame: cur,
                    direct: None,
                    rangered: false,
                }));
            }
        }
        Ok(())
    }

    fn push_anchor(&mut self, a: Anchor, pic: Picture) {
        let old = self.prev.take();
        self.recycle(old);
        self.prev = self.last.take();
        self.last = Some(a);
        if self.reorder {
            if let Some(p) = self.held.replace(pic) {
                self.out.push_back(p);
            }
        } else {
            self.out.push_back(pic);
        }
    }

    fn output_picture(&self, seq: &Seq, f: &Frame, rangered: bool, tag: i64) -> Picture {
        let (w, h) = (seq.width as usize, seq.height as usize);
        let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
        let crop = |p: &mc::Plane, pw: usize, ph: usize, map: &dyn Fn(u8) -> u8| -> Vec<u8> {
            let mut v = Vec::with_capacity(pw * ph);
            for y in 0..ph {
                v.extend(
                    p.data[y * p.stride..y * p.stride + pw]
                        .iter()
                        .map(|&s| map(s)),
                );
            }
            v
        };
        let id = |s: u8| s;
        let up = |s: u8| ((i32::from(s) - 128) * 2 + 128).clamp(0, 255) as u8;
        let mapy = seq.range_mapy;
        let mapuv = seq.range_mapuv;
        let rm = |m: Option<u8>| {
            move |s: u8| match m {
                Some(m) => ((((i32::from(s) - 128) * (i32::from(m) + 9) + 4) >> 3) + 128)
                    .clamp(0, 255) as u8,
                None => s,
            }
        };
        type Map = Box<dyn Fn(u8) -> u8>;
        let (fy, fuv): (Map, Map) = if rangered && seq.rangered {
            (Box::new(up), Box::new(up))
        } else if seq.profile == Profile::Advanced {
            (Box::new(rm(mapy)), Box::new(rm(mapuv)))
        } else {
            (Box::new(id), Box::new(id))
        };
        Picture {
            width: w as u32,
            height: h as u32,
            planes: [
                crop(&f.planes[0], w, h, &*fy),
                crop(&f.planes[1], cw, ch, &*fuv),
                crop(&f.planes[2], cw, ch, &*fuv),
            ],
            tag,
            interlaced: false,
            top_field_first: true,
            colour: seq.colour,
            full_range: false,
        }
    }

    /// Ends the stream: the held picture comes out.
    pub fn flush(&mut self) {
        if let Some(p) = self.held.take() {
            self.out.push_back(p);
        }
    }

    /// Forgets the references and pending pictures (decoding resumes at a key frame).
    pub fn reset(&mut self) {
        self.prev = None;
        self.last = None;
        self.held = None;
        self.out.clear();
        self.rnd = false;
    }

    pub fn next_picture(&mut self) -> Option<Picture> {
        self.out.pop_front()
    }
}

/// A range-reduced reference rescaled for a picture of the other range (§8.3.4.11): scaled
/// down when the current picture is reduced, up when it is not.
fn rescale(f: &Frame, down: bool) -> Frame {
    let lut: [u8; 256] = std::array::from_fn(|i| {
        let v = i as i32 - 128;
        if down {
            ((v >> 1) + 128) as u8
        } else {
            (v * 2 + 128).clamp(0, 255) as u8
        }
    });
    f.mapped(&lut, &lut)
}

fn last_mapped(f: &Frame, ly: &[u8; 256], luv: &[u8; 256]) -> Frame {
    f.mapped(ly, luv)
}

/// Figure 67: the intensity compensation lookup tables.
fn intensity_luts(lumscale: u8, lumshift: u8) -> ([u8; 256], [u8; 256]) {
    let (scale, shift) = if lumscale == 0 {
        let mut sh = 255 * 64 - i32::from(lumshift) * 2 * 64;
        if lumshift > 31 {
            sh += 128 * 64;
        }
        (-64, sh)
    } else {
        let sc = i32::from(lumscale) + 32;
        let sh = if lumshift > 31 {
            i32::from(lumshift) * 64 - 64 * 64
        } else {
            i32::from(lumshift) * 64
        };
        (sc, sh)
    };
    let mut ly = [0u8; 256];
    let mut luv = [0u8; 256];
    for i in 0..256 {
        ly[i] = ((scale * i as i32 + shift + 32) >> 6).clamp(0, 255) as u8;
        luv[i] = ((scale * (i as i32 - 128) + 128 * 64 + 32) >> 6).clamp(0, 255) as u8;
    }
    (ly, luv)
}

/// The BDUs of a byte stream: (start code suffix, the bytes after it up to the next start
/// code).
fn bdus(data: &[u8]) -> impl Iterator<Item = (u8, &[u8])> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 < data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            starts.push(i);
            i += 3;
        } else {
            i += 1;
        }
    }
    let n = starts.len();
    (0..n).map(move |k| {
        let s = starts[k];
        let e = if k + 1 < n { starts[k + 1] } else { data.len() };
        (data[s + 3], &data[(s + 4).min(e)..e])
    })
}

/// Removes emulation prevention bytes (Annex E.2).
fn unescape(p: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(p.len());
    let mut zeros = 0;
    for &b in p {
        if zeros >= 2 && b == 3 {
            zeros = 0;
            continue;
        }
        if b == 0 {
            zeros += 1;
        } else {
            zeros = 0;
        }
        out.push(b);
    }
    out
}
