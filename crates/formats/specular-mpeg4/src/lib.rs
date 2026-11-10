//! Specular's own MPEG-4 Part 2 video decoder (ISO/IEC 14496-2): DivX, Xvid and camera
//! MPEG-4, and H.263 baseline pictures (the short header mode, 3GPP `s263`).
//!
//! - Simple and Advanced Simple Profiles: I-, P- and B-VOPs (direct, forward, backward and
//!   interpolated macroblocks), four vectors per macroblock, intra DC/AC prediction, H.263 and
//!   MPEG quantisation with matrices, quarter-sample motion, global motion compensation
//!   (S-VOPs, one to three warping points), interlaced coding (field DCT, field prediction),
//!   video packets with resync markers.
//! - DivX's packed bitstream (a P- and a B-VOP in one chunk, then a placeholder) and not-coded
//!   VOPs.
//! - Data partitioning (video packets split into header, motion and texture partitions).
//! - Refused in words: shapes, static sprites, scalability, reversible VLC (RVLC), interlaced
//!   data partitioning, NEWPRED, reduced resolution, complexity estimation headers, more than
//!   8 bits, H.263 options (PLUSPTYPE). libavcodec's encoder writes none of these.
//!
//! Input is the stream in pieces (container samples, or any): VOPs are cut at start codes;
//! with [`Decoder::set_aligned`] a piece's last VOP ends with it. Pictures come out in
//! decoding or display order, tagged with the tag of the piece their VOP came in. The inverse
//! DCT is libavcodec's `-idct int` one, so output is bit-exact with it.
#![deny(unsafe_code)]
#![allow(clippy::needless_range_loop)]

mod gmc;
mod headers;
mod mb;
mod mc;
mod tables;
mod vlc;

use std::collections::VecDeque;
use std::sync::Arc;

pub use headers::{Vol, VopType};

use headers::Vop;
use mb::MbInfo;
use mc::Frame;

/// Why a stream can't be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Unsupported(String),
    Malformed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::Malformed(s) => write!(f, "damaged MPEG-4 video: {s}"),
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

/// The order pictures come out in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputOrder {
    Decode,
    Display,
}

/// A decoded picture (4:2:0, cropped to the VOP's size).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    /// Y, Cb, Cr, rows tightly packed (chroma `ceil(width / 2)` × `ceil(height / 2)`).
    pub planes: [Vec<u8>; 3],
    pub tag: i64,
    pub kind: VopType,
    /// A not-coded VOP: the previous picture again.
    pub repeated: bool,
    pub interlaced: bool,
    pub top_field_first: bool,
    pub colour: Option<(u8, u8, u8)>,
    pub full_range: bool,
}

struct Anchor {
    frame: Arc<Frame>,
    info: Arc<Vec<MbInfo>>,
}

/// An MPEG-4 Part 2 / H.263 decoder for one stream.
pub struct Decoder {
    order: OutputOrder,
    vol: Option<Vol>,
    vo: headers::VisualObject,
    profile_level: u8,
    packed: bool,
    buf: Vec<u8>,
    head: usize,
    base: u64,
    marks: VecDeque<(u64, i64)>,
    aligned: bool,
    /// Older and newer references.
    refs: [Option<Anchor>; 2],
    held: Option<Picture>,
    out: VecDeque<Picture>,
    dropped: VecDeque<i64>,
    /// Time bases (§6.3.5): of the last I/P/S VOP and the one before, in seconds.
    time_base: u64,
    last_time_base: u64,
    last_anchor_time: i64,
    /// Time between the last two anchors.
    pp_time: i64,
    /// The time of a frame period, from the first B-VOP (for field direct mode).
    t_frame: i64,
    need_intra: bool,
    refusal: Option<Error>,
    /// Sorenson Spark (FLV1) pictures, one per sample.
    sorenson: bool,
    /// DivX packed bitstream: a B-VOP that came behind another VOP in its piece, decoded when
    /// the next piece comes (whose placeholder slot it takes), never at the end.
    packed_b: Option<(Vec<u8>, i64)>,
    /// VOPs decoded from the current piece.
    vops_in_piece: u32,
}

impl Decoder {
    pub fn new(order: OutputOrder) -> Decoder {
        Decoder {
            order,
            vol: None,
            vo: headers::VisualObject::default(),
            profile_level: 0,
            packed: false,
            buf: Vec::new(),
            head: 0,
            base: 0,
            marks: VecDeque::new(),
            aligned: false,
            refs: [None, None],
            held: None,
            out: VecDeque::new(),
            dropped: VecDeque::new(),
            time_base: 0,
            last_time_base: 0,
            last_anchor_time: 0,
            pp_time: 0,
            t_frame: 0,
            need_intra: false,
            refusal: None,
            packed_b: None,
            vops_in_piece: 0,
            sorenson: false,
        }
    }

    /// Every sample is one Sorenson Spark picture (FLV codec 2), whose start code H.263's
    /// can't be told from: the container says so.
    pub fn set_sorenson(&mut self, on: bool) {
        self.sorenson = on;
    }

    /// Pieces end where VOPs (or H.263 pictures) end: container samples.
    pub fn set_aligned(&mut self, aligned: bool) {
        self.aligned = aligned;
    }

    /// The video object layer in force (after its header, or an H.263 picture).
    pub fn vol(&self) -> Option<&Vol> {
        self.vol.as_ref()
    }

    /// DivX's packed bitstream (B-VOPs in the chunk of the P-VOP before them).
    pub fn packed(&self) -> bool {
        self.packed
    }

    pub fn decode(&mut self, data: &[u8], tag: i64) -> Result<(), Error> {
        if let Some((b, t)) = self.packed_b.take() {
            self.vop(&b, t);
        }
        self.vops_in_piece = 0;
        if self.sorenson || self.aligned && is_short_header(data) {
            self.short_picture(data, tag);
        } else {
            let pos = self.base + self.buf.len() as u64;
            self.marks.push_back((pos, tag));
            self.buf.extend_from_slice(data);
            self.units(self.aligned);
            self.compact();
        }
        self.refusal.take().map_or(Ok(()), Err)
    }

    pub fn flush(&mut self) {
        self.units(true);
        self.compact();
        if let Some((_, t)) = self.packed_b.take() {
            self.dropped.push_back(t);
        }
        if let Some(p) = self.held.take() {
            self.out.push_back(p);
        }
    }

    /// Forgets references and pending input (the layer header is kept).
    pub fn reset(&mut self) {
        self.buf.clear();
        self.head = 0;
        self.marks.clear();
        self.refs = [None, None];
        self.held = None;
        self.out.clear();
        self.dropped.clear();
        self.need_intra = true;
        self.packed_b = None;
    }

    pub fn next_picture(&mut self) -> Option<Picture> {
        self.out.pop_front()
    }

    pub fn next_dropped(&mut self) -> Option<i64> {
        self.dropped.pop_front()
    }

    fn compact(&mut self) {
        if self.head > 0 {
            self.buf.drain(..self.head);
            self.base += self.head as u64;
            self.head = 0;
        }
        while self.marks.len() > 1 && self.marks[1].0 <= self.base {
            self.marks.pop_front();
        }
    }

    fn tag_at(&self, pos: u64) -> i64 {
        self.marks
            .iter()
            .rev()
            .find(|m| m.0 <= pos)
            .or(self.marks.front())
            .map_or(0, |m| m.1)
    }

    fn units(&mut self, all: bool) {
        loop {
            let b = &self.buf[self.head..];
            let Some(s) = find_start(b, 0) else {
                let keep = b.len().min(3);
                self.head = self.buf.len() - keep;
                return;
            };
            if s > 0 {
                self.head += s;
                continue;
            }
            let end = match find_start(b, 4) {
                Some(e) => e,
                None if all && b.len() >= 4 => b.len(),
                None => return,
            };
            let code = b[3];
            let tag = self.tag_at(self.base + self.head as u64);
            let (from, to) = (self.head + 4, self.head + end);
            let last = end == b.len();
            self.head += end;
            let unit = self.buf[from..to].to_vec();
            self.unit(code, &unit, tag);
            if last {
                return;
            }
        }
    }

    fn unit(&mut self, code: u8, data: &[u8], tag: i64) {
        match code {
            headers::VOS => self.profile_level = data.first().copied().unwrap_or(0),
            headers::VISUAL_OBJECT => self.vo = headers::visual_object(data),
            0x20..=0x2F => match headers::vol(data, self.vo, self.profile_level) {
                Ok(v) => {
                    if self
                        .vol
                        .as_ref()
                        .is_some_and(|o| o.width != v.width || o.height != v.height)
                    {
                        self.refs = [None, None];
                    }
                    self.vol = Some(v);
                }
                Err(e) => {
                    self.refusal.get_or_insert(e);
                }
            },
            headers::USER_DATA => {
                if let Some(p) = headers::divx_packed(data) {
                    self.packed = p;
                }
            }
            headers::GOV => {
                // A GOV header restarts the time base at its time code.
                let mut r = specular_mpeg2::util::Bits::new(data);
                let h = u64::from(r.u(5));
                let m = u64::from(r.u(6));
                r.skip(1);
                let s = u64::from(r.u(6));
                self.time_base = h * 3600 + m * 60 + s;
            }
            headers::VOP => {
                // A B-VOP packed behind another VOP waits for the next piece.
                if self.packed
                    && self.aligned
                    && self.vops_in_piece > 0
                    && data.first().is_some_and(|b| b >> 6 == 2)
                {
                    self.packed_b = Some((data.to_vec(), tag));
                } else {
                    self.vop(data, tag);
                }
                self.vops_in_piece += 1;
            }
            _ => {}
        }
    }

    fn vop(&mut self, data: &[u8], tag: i64) {
        let Some(vol) = self.vol.clone() else {
            if std::env::var_os("SPECULAR_MPEG4_DEBUG").is_some() {
                eprintln!("mpeg4: VOP without a layer");
            }
            self.dropped.push_back(tag);
            return;
        };
        if let Some(why) = vol.refusal() {
            self.dropped.push_back(tag);
            self.refusal.get_or_insert(Error::Unsupported(why.into()));
            return;
        }
        let dmv = &vlc::tables().sprite_dmv;
        let vop = match headers::vop(data, &vol, dmv) {
            Ok(v) => v,
            Err(e @ Error::Unsupported(_)) => {
                self.dropped.push_back(tag);
                self.refusal.get_or_insert(e);
                return;
            }
            Err(e) => {
                if std::env::var_os("SPECULAR_MPEG4_DEBUG").is_some() {
                    eprintln!("mpeg4: VOP header: {e}");
                }
                self.dropped.push_back(tag);
                return;
            }
        };
        if vop.time_bits != vol.time_bits
            && let Some(v) = self.vol.as_mut()
        {
            v.time_bits = vop.time_bits;
        }
        self.picture(&vol, &vop, data, tag);
    }

    /// One H.263 picture (a whole sample).
    fn short_picture(&mut self, data: &[u8], tag: i64) {
        let header = if self.sorenson {
            headers::sorenson_header(data)
        } else {
            headers::short_header(data)
        };
        match header {
            Ok((vol, vop)) => {
                if self
                    .vol
                    .as_ref()
                    .is_some_and(|o| o.width != vol.width || o.height != vol.height)
                {
                    self.refs = [None, None];
                }
                self.vol = Some(vol.clone());
                self.picture(&vol, &vop, data, tag);
            }
            Err(e @ Error::Unsupported(_)) => {
                self.dropped.push_back(tag);
                self.refusal.get_or_insert(e);
            }
            Err(_) => self.dropped.push_back(tag),
        }
    }

    fn picture(&mut self, vol: &Vol, vop: &Vop, data: &[u8], tag: i64) {
        // Times (§6.3.5): anchors advance the time base; B-VOPs count from the one before.
        let res = i64::from(vol.time_resolution);
        let (time, trb, trd);
        let (mut trb_field, mut trd_field) = (0i64, 0i64);
        if vop.kind == VopType::B {
            time = (self.last_time_base + u64::from(vop.seconds)) as i64 * res
                + i64::from(vop.increment);
            trd = self.pp_time;
            trb = trd - (self.last_anchor_time - time);
            // Field distances: frame times in frame periods (the first B-VOP's distance),
            // doubled (§7.6.9.5.2).
            if self.t_frame == 0 {
                self.t_frame = trb.max(1);
            }
            let t = self.t_frame;
            let rdiv = |a: i64| {
                if a >= 0 {
                    (a + t / 2) / t
                } else {
                    (a - t / 2) / t
                }
            };
            let past = self.last_anchor_time - self.pp_time;
            trd_field = (rdiv(self.last_anchor_time) - rdiv(past)) * 2;
            trb_field = (rdiv(time) - rdiv(past)) * 2;
        } else {
            self.last_time_base = self.time_base;
            self.time_base += u64::from(vop.seconds);
            time = self.time_base as i64 * res + i64::from(vop.increment);
            trd = 0;
            trb = 0;
            if vop.coded {
                self.pp_time = time - self.last_anchor_time;
                self.last_anchor_time = time;
            }
        }
        if !vop.coded {
            // A not-coded VOP: in a packed bitstream the placeholder of a B-VOP shown already;
            // else the last picture again.
            if self.packed || vol.short_header {
                self.dropped.push_back(tag);
                return;
            }
            match &self.refs[1] {
                Some(a) => {
                    let mut p = output(vol, &a.frame, tag, VopType::P);
                    p.repeated = true;
                    self.emit(p, false);
                }
                None => self.dropped.push_back(tag),
            }
            return;
        }
        let anchor = vop.kind != VopType::B;
        if self.need_intra && vop.kind != VopType::I {
            self.dropped.push_back(tag);
            return;
        }
        let ok = match vop.kind {
            VopType::I => true,
            VopType::P | VopType::S => self.refs[1].is_some(),
            VopType::B => self.refs[0].is_some() && self.refs[1].is_some(),
        };
        if !ok {
            self.dropped.push_back(tag);
            return;
        }
        self.need_intra = false;
        let (mb_w, mb_h) = vol.mb_size();
        let mut frame = Frame::new(mb_w, mb_h, (mb_w * 16) as u32, (mb_h * 16) as u32);
        let empty: Vec<MbInfo> = Vec::new();
        let (fwd, bwd, colocated) = match vop.kind {
            VopType::B => (
                self.refs[0].as_ref().map(|a| &*a.frame),
                self.refs[1].as_ref().map(|a| &*a.frame),
                self.refs[1].as_ref().map_or(&empty[..], |a| &a.info[..]),
            ),
            VopType::I => (None, None, &empty[..]),
            _ => (self.refs[1].as_ref().map(|a| &*a.frame), None, &empty[..]),
        };
        let gmc = if vop.kind == VopType::S {
            gmc::Sprite::new(vol, vop.warp)
        } else {
            None
        };
        let ctx = mb::Ctx {
            vol,
            vop,
            mb_w,
            mb_h,
            fwd,
            bwd,
            colocated,
            trb: trb as i32,
            trd: trd as i32,
            trb_field: trb_field as i32,
            trd_field: trd_field as i32,
            gmc,
        };
        let info = match mb::decode_vop(&ctx, data, &mut frame) {
            Ok(i) => i,
            Err(e) => {
                if std::env::var_os("SPECULAR_MPEG4_DEBUG").is_some() {
                    eprintln!("mpeg4: {e}");
                }
                vec![MbInfo::default(); mb_w * mb_h]
            }
        };
        let pic = output(vol, &frame, tag, vop.kind);
        let mut pic = pic;
        pic.top_field_first = vop.top_field_first;
        if vop.disposable {
            // Nothing refers to a disposable frame: shown after the anchor before it.
            if let Some(h) = self.held.take() {
                self.out.push_back(h);
            }
            self.out.push_back(pic);
        } else if anchor {
            let a = Anchor {
                frame: Arc::new(frame),
                info: Arc::new(info),
            };
            self.refs[0] = self.refs[1].take();
            self.refs[1] = Some(a);
            self.emit(pic, true);
        } else {
            self.emit(pic, false);
        }
    }

    fn emit(&mut self, p: Picture, anchor: bool) {
        match self.order {
            OutputOrder::Decode => self.out.push_back(p),
            OutputOrder::Display => {
                if anchor {
                    if let Some(h) = self.held.take() {
                        self.out.push_back(h);
                    }
                    self.held = Some(p);
                } else {
                    self.out.push_back(p);
                }
            }
        }
    }
}

fn output(vol: &Vol, f: &Frame, tag: i64, kind: VopType) -> Picture {
    let (w, h) = (vol.width as usize, vol.height as usize);
    let crop = |c: usize, pw: usize, ph: usize| {
        let mut v = Vec::with_capacity(pw * ph);
        for row in f.planes[c].chunks(f.stride[c]).take(ph) {
            v.extend_from_slice(&row[..pw]);
        }
        v
    };
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    Picture {
        width: w as u32,
        height: h as u32,
        planes: [crop(0, w, h), crop(1, cw, ch), crop(2, cw, ch)],
        tag,
        kind,
        repeated: false,
        interlaced: vol.interlaced,
        top_field_first: false,
        colour: vol.colour,
        full_range: vol.full_range,
    }
}

/// An H.263 picture start code (22 bits: 0000 0000 0000 0000 1000 00) at the start.
pub fn is_short_header(data: &[u8]) -> bool {
    data.len() >= 4 && data[0] == 0 && data[1] == 0 && data[2] & 0xFC == 0x80
}

fn find_start(b: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while i + 3 <= b.len() {
        match b[i + 2] {
            1 if b[i] == 0 && b[i + 1] == 0 => return Some(i),
            0 => i += 1,
            _ => i += 3,
        }
    }
    None
}

/// The video object layer of a configuration record or a stream's first bytes (MPEG-4) or of
/// an H.263 picture.
pub fn probe(data: &[u8]) -> Option<Vol> {
    if is_short_header(data) {
        return headers::short_header(data).ok().map(|(v, _)| v);
    }
    let mut vo = headers::VisualObject::default();
    let mut pl = 0;
    let mut i = 0;
    while let Some(s) = find_start(data, i) {
        let code = *data.get(s + 3)?;
        let end = find_start(data, s + 4).unwrap_or(data.len());
        let body = &data[s + 4..end];
        match code {
            headers::VOS => pl = body.first().copied().unwrap_or(0),
            headers::VISUAL_OBJECT => vo = headers::visual_object(body),
            0x20..=0x2F => return headers::vol(body, vo, pl).ok(),
            _ => {}
        }
        i = end;
    }
    None
}

/// The layer of a Sorenson Spark picture (FLV codec 2).
pub fn probe_sorenson(data: &[u8]) -> Option<Vol> {
    headers::sorenson_header(data).ok().map(|(v, _)| v)
}

/// Whether a Sorenson Spark picture is intra coded.
pub fn is_sorenson_keyframe(data: &[u8]) -> bool {
    headers::sorenson_header(data).is_ok_and(|(_, v)| v.kind == VopType::I)
}

/// Why a Sorenson Spark track can't be decoded, from its first picture.
pub fn sorenson_refusal(data: &[u8]) -> Option<String> {
    match headers::sorenson_header(data) {
        Err(Error::Unsupported(m)) => Some(m),
        _ => None,
    }
}

/// The coding type of the first VOP in `data` (for keyframe flags), if any.
pub fn first_vop_type(data: &[u8]) -> Option<VopType> {
    if is_short_header(data) {
        return headers::short_header(data).ok().map(|(_, v)| v.kind);
    }
    let mut i = 0;
    while let Some(s) = find_start(data, i) {
        if data.get(s + 3) == Some(&headers::VOP) {
            return Some(match data.get(s + 4)? >> 6 {
                0 => VopType::I,
                1 => VopType::P,
                2 => VopType::B,
                _ => VopType::S,
            });
        }
        i = s + 3;
    }
    None
}

/// The VOL header bytes (with the visual object sequence and object headers before it) at
/// the start of `data`, as a configuration record.
pub fn config_bytes(data: &[u8]) -> Option<&[u8]> {
    let mut i = 0;
    let mut start = None;
    while let Some(s) = find_start(data, i) {
        let code = *data.get(s + 3)?;
        match code {
            headers::VOS | headers::VISUAL_OBJECT | 0x00..=0x2F | headers::USER_DATA => {
                start.get_or_insert(s);
            }
            _ => return start.map(|st| &data[st..s]),
        }
        i = s + 3;
    }
    start.map(|st| &data[st..])
}
