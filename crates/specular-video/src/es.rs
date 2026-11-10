//! Raw video elementary streams (`.h264`/`.264`/`.avc`, `.h265`/`.265`/`.hevc`, `.obu`,
//! `.mjpeg`/`.mjpg`)
//! into the same [`Movie`] the MP4 demuxer gives, for the decoders the containers use.
//!
//! * **H.264 and HEVC** (Annex B byte streams): NAL units after start codes, cut into access
//!   units where the standards say a new one begins (an access unit delimiter, parameter
//!   sets or SEI after a picture's slices, or the first slice of a new picture). The
//!   parameter sets seen become the `avcC` / `hvcC` record. Presentation order comes from
//!   the pictures' order counts (H.264 types 0 and 2, HEVC), counted again from every IDR
//!   (and HEVC BLA); a stream of H.264 order-count type 1 is shown in decoding order.
//! * **AV1** (the low-overhead OBU stream of `aomenc --obu` and the conformance suite):
//!   temporal units start at temporal delimiters; the sequence header gives the size and,
//!   when present, the frame rate.
//! * **Motion JPEG**: JPEG pictures one after another, each from its SOI to its EOI (the
//!   marker segments walked, the entropy-coded data scanned for markers). Opened by name
//!   only (`.mjpeg`, `.mjpg`): a file of two JPEG pictures could be anything else.
//!
//! These streams carry no timing (AV1's optional timing info aside): frames are shown at
//! [`DEFAULT_FPS`], and the facts say so. There is no sound.

use crate::mp4::{Codec, Error, Layout, MAX_SAMPLES, Movie, Result, Sample, VideoTrack};
use crate::ts::{ParamSets, unescape};
use std::collections::HashMap;

/// A picture size.
type Size = (u32, u32);
/// An AV1 sequence header's size and frame rate (time scale, ticks per frame).
type Sequence = (Size, Option<(u32, u32)>);

/// Frames per second when the stream does not say.
pub const DEFAULT_FPS: u32 = 25;
/// Bytes looked at to recognise a stream.
const SNIFF: usize = 64 << 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    H264,
    Hevc,
    Av1,
    Mjpeg,
}

fn bad(s: impl Into<String>) -> Error {
    Error::Malformed(s.into())
}

/// Where each NAL unit's header starts (after its start code), and where its start code
/// began.
fn nal_starts(d: &[u8]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 3 <= d.len() {
        if d[i] == 0 && d[i + 1] == 0 && d[i + 2] == 1 {
            let code = if i > 0 && d[i - 1] == 0 { i - 1 } else { i };
            out.push((i + 3, code));
            i += 3;
        } else if d[i + 2] > 1 {
            i += 3;
        } else {
            i += 1;
        }
    }
    out
}

/// The kind of raw stream `data` starts with: a start code at the very start, then a NAL
/// unit that opens H.264 (delimiter, SPS, SEI, IDR slice) or HEVC (delimiter, VPS, SPS,
/// SEI) streams, with a sequence parameter set early on; or an AV1 temporal delimiter
/// followed by a sequence header.
pub fn sniff(data: &[u8]) -> Option<Kind> {
    let head = &data[..data.len().min(SNIFF)];
    if head.starts_with(&[0x12, 0x00]) && head.get(2).is_some_and(|&b| (b >> 3) & 15 == 1) {
        return Some(Kind::Av1);
    }
    if head.starts_with(&[0xFF, 0xD8, 0xFF]) {
        // A JPEG picture followed by another.
        let end = jpeg_end(data, 0)?;
        return next_soi(data, end).is_some().then_some(Kind::Mjpeg);
    }
    let first = if head.starts_with(&[0, 0, 0, 1]) {
        4
    } else if head.starts_with(&[0, 0, 1]) {
        3
    } else {
        return None;
    };
    let (&b0, &b1) = (head.get(first)?, head.get(first + 1)?);
    if b0 & 0x80 != 0 {
        return None;
    }
    let nals: Vec<usize> = nal_starts(head).into_iter().map(|n| n.0).collect();
    let hevc_type = (b0 >> 1) & 0x3F;
    if b0 & 1 == 0 && (b1 & 7) >= 1 && matches!(hevc_type, 32..=35 | 39) {
        let has_sps = nals
            .iter()
            .any(|&p| head.get(p).is_some_and(|&b| (b >> 1) & 0x3F == 33));
        if has_sps {
            return Some(Kind::Hevc);
        }
    }
    let t = b0 & 0x1F;
    if matches!(t, 1 | 5 | 6 | 7 | 9) {
        let has_sps = nals
            .iter()
            .any(|&p| head.get(p).is_some_and(|&b| b & 0x1F == 7));
        if has_sps {
            return Some(Kind::H264);
        }
    }
    None
}

/// Where the JPEG picture starting at `at` (its SOI) ends, after its EOI.
fn jpeg_end(d: &[u8], at: usize) -> Option<usize> {
    let mut p = at + 2;
    loop {
        // Fill bytes before a marker.
        while d.get(p) == Some(&0xFF) && d.get(p + 1) == Some(&0xFF) {
            p += 1;
        }
        if d.get(p) != Some(&0xFF) {
            return None;
        }
        let m = *d.get(p + 1)?;
        match m {
            0xD9 => return Some(p + 2),
            0x01 | 0xD0..=0xD7 => p += 2,
            _ => {
                let len = usize::from(u16::from_be_bytes([*d.get(p + 2)?, *d.get(p + 3)?]));
                if len < 2 {
                    return None;
                }
                p += 2 + len;
                if m == 0xDA {
                    // Entropy-coded data up to the next marker other than a restart.
                    loop {
                        let k = p + d.get(p..)?.iter().position(|&b| b == 0xFF)?;
                        let n = *d.get(k + 1)?;
                        if n == 0 || (0xD0..=0xD7).contains(&n) || n == 0xFF {
                            p = k + 1 + usize::from(n != 0xFF);
                        } else {
                            p = k;
                            break;
                        }
                    }
                }
            }
        }
        if p >= d.len() {
            return None;
        }
    }
}

/// The next SOI at or after `at`, past padding (at most 64 bytes of it).
fn next_soi(d: &[u8], at: usize) -> Option<usize> {
    let end = d.len().min(at + 64);
    (at..end.saturating_sub(2)).find(|&i| d[i..].starts_with(&[0xFF, 0xD8, 0xFF]))
}

/// The pictures of a Motion JPEG stream, and the first's size (its SOF).
fn mjpeg_frames(d: &[u8]) -> (Vec<(usize, usize)>, Option<Size>) {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(s) = next_soi(d, at) {
        let Some(e) = jpeg_end(d, s) else {
            // A last picture cut short: kept, the decoder shows what it has.
            out.push((s, d.len()));
            break;
        };
        out.push((s, e));
        if out.len() > MAX_SAMPLES {
            break;
        }
        at = e;
    }
    let size = out.first().and_then(|&(s, e)| {
        let mut p = s + 2;
        while p + 9 < e {
            if d[p] != 0xFF {
                return None;
            }
            let m = d[p + 1];
            let len = usize::from(u16::from_be_bytes([d[p + 2], d[p + 3]]));
            if matches!(m, 0xC0..=0xCF) && !matches!(m, 0xC4 | 0xC8 | 0xCC) {
                let h = u16::from_be_bytes([d[p + 5], d[p + 6]]);
                let w = u16::from_be_bytes([d[p + 7], d[p + 8]]);
                return Some((u32::from(w), u32::from(h)));
            }
            p += 2 + len;
        }
        None
    });
    (out, size)
}

/// A bit reader for the header fields read here.
struct Bits<'a> {
    b: &'a [u8],
    p: usize,
}

impl Bits<'_> {
    fn u(&mut self, n: u32) -> Option<u32> {
        let mut v = 0u32;
        for _ in 0..n {
            let byte = *self.b.get(self.p / 8)?;
            v = (v << 1) | u32::from(byte >> (7 - self.p % 8) & 1);
            self.p += 1;
        }
        Some(v)
    }
    fn ue(&mut self) -> Option<u32> {
        let mut z = 0;
        while self.u(1)? == 0 {
            z += 1;
            if z > 31 {
                return None;
            }
        }
        Some(((1u64 << z) - 1 + u64::from(self.u(z)?)).min(u64::from(u32::MAX)) as u32)
    }
    fn se(&mut self) -> Option<i32> {
        let k = self.ue()?;
        Some(if k & 1 == 1 {
            (k / 2 + 1) as i32
        } else {
            -((k / 2) as i32)
        })
    }
}

/// What H.264 order counts need from a sequence parameter set.
#[derive(Clone, Copy, Debug, Default)]
struct AvcSps {
    log2_frame_num: u32,
    poc_type: u32,
    log2_poc_lsb: u32,
    frame_mbs_only: bool,
    separate_planes: bool,
}

fn avc_sps(nal: &[u8]) -> Option<(u32, AvcSps)> {
    let rbsp = unescape(nal.get(1..)?);
    let mut b = Bits { b: &rbsp, p: 0 };
    let profile = b.u(8)?;
    b.u(16)?;
    let id = b.ue()?;
    let mut s = AvcSps::default();
    if matches!(
        profile,
        100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 135
    ) {
        let chroma = b.ue()?;
        if chroma == 3 {
            s.separate_planes = b.u(1)? == 1;
        }
        b.ue()?;
        b.ue()?;
        b.u(1)?;
        if b.u(1)? == 1 {
            for i in 0..if chroma == 3 { 12 } else { 8 } {
                if b.u(1)? == 1 {
                    let n = if i < 6 { 16 } else { 64 };
                    let (mut last, mut next) = (8i32, 8i32);
                    for _ in 0..n {
                        if next != 0 {
                            next = (last + b.se()? + 256) % 256;
                        }
                        if next != 0 {
                            last = next;
                        }
                    }
                }
            }
        }
    }
    s.log2_frame_num = (b.ue()? + 4).min(16);
    s.poc_type = b.ue()?;
    match s.poc_type {
        0 => s.log2_poc_lsb = (b.ue()? + 4).min(16),
        1 => {
            b.u(1)?;
            b.se()?;
            b.se()?;
            for _ in 0..b.ue()?.min(255) {
                b.se()?;
            }
        }
        _ => {}
    }
    b.ue()?;
    b.u(1)?;
    b.ue()?;
    b.ue()?;
    s.frame_mbs_only = b.u(1)? == 1;
    Some((id, s))
}

/// (pps id, sps id, bottom_field_pic_order_in_frame_present).
fn avc_pps(nal: &[u8]) -> Option<(u32, u32, bool)> {
    let rbsp = unescape(nal.get(1..)?);
    let mut b = Bits { b: &rbsp, p: 0 };
    let (pps, sps) = (b.ue()?, b.ue()?);
    b.u(1)?;
    Some((pps, sps, b.u(1)? == 1))
}

/// A slice's start: (first_mb_in_slice, frame_num, field, bottom, poc lsb).
struct AvcSlice {
    first_mb: u32,
    frame_num: u32,
    poc_lsb: u32,
}

fn avc_slice(
    nal: &[u8],
    sps: &HashMap<u32, AvcSps>,
    pps: &HashMap<u32, (u32, bool)>,
) -> Option<(AvcSlice, AvcSps)> {
    let idr = nal.first()? & 0x1F == 5;
    let rbsp = unescape(&nal[1..nal.len().min(64)]);
    let mut b = Bits { b: &rbsp, p: 0 };
    let first_mb = b.ue()?;
    b.ue()?;
    let (sps_id, _) = *pps.get(&b.ue()?)?;
    let s = *sps.get(&sps_id)?;
    if s.separate_planes {
        b.u(2)?;
    }
    let frame_num = b.u(s.log2_frame_num)?;
    if !s.frame_mbs_only && b.u(1)? == 1 {
        b.u(1)?;
    }
    if idr {
        b.ue()?;
    }
    let poc_lsb = if s.poc_type == 0 {
        b.u(s.log2_poc_lsb)?
    } else {
        0
    };
    Some((
        AvcSlice {
            first_mb,
            frame_num,
            poc_lsb,
        },
        s,
    ))
}

/// What HEVC order counts need: per SPS the order-count bits and separate colour planes;
/// per PPS its SPS, `output_flag_present` and the extra slice-header bits.
#[derive(Default)]
struct HevcSets {
    sps: HashMap<u32, (u32, bool)>,
    pps: HashMap<u32, (u32, bool, u32)>,
}

fn hevc_sps(nal: &[u8]) -> Option<(u32, u32, bool)> {
    let rbsp = unescape(nal.get(2..)?);
    let mut b = Bits { b: &rbsp, p: 0 };
    b.u(4)?;
    let sub_layers = b.u(3)?;
    b.u(1)?;
    b.u(8)?;
    b.u(32)?;
    b.u(32)?;
    b.u(16)?;
    b.u(8)?;
    let mut flags = Vec::new();
    for _ in 0..sub_layers {
        flags.push((b.u(1)?, b.u(1)?));
    }
    if sub_layers > 0 {
        for _ in sub_layers..8 {
            b.u(2)?;
        }
    }
    for (profile, level) in flags {
        if profile == 1 {
            b.u(32)?;
            b.u(32)?;
            b.u(24)?;
        }
        if level == 1 {
            b.u(8)?;
        }
    }
    let id = b.ue()?;
    let chroma = b.ue()?;
    let separate = chroma == 3 && b.u(1)? == 1;
    b.ue()?;
    b.ue()?;
    if b.u(1)? == 1 {
        for _ in 0..4 {
            b.ue()?;
        }
    }
    b.ue()?;
    b.ue()?;
    Some((id, (b.ue()? + 4).min(16), separate))
}

fn hevc_pps(nal: &[u8]) -> Option<(u32, (u32, bool, u32))> {
    let rbsp = unescape(nal.get(2..)?);
    let mut b = Bits { b: &rbsp, p: 0 };
    let (pps, sps) = (b.ue()?, b.ue()?);
    b.u(1)?;
    let output = b.u(1)? == 1;
    Some((pps, (sps, output, b.u(3)?)))
}

/// A first slice segment's order-count lsb (0 for IDR pictures); `None` when the segment
/// is not the first of its picture.
fn hevc_poc_lsb(nal: &[u8], sets: &HevcSets) -> Option<Option<(u32, u32)>> {
    let t = (nal.first()? >> 1) & 0x3F;
    let rbsp = unescape(&nal[2..nal.len().min(64)]);
    let mut b = Bits { b: &rbsp, p: 0 };
    if b.u(1)? == 0 {
        return Some(None);
    }
    if (16..=23).contains(&t) {
        b.u(1)?;
    }
    let &(sps_id, output, extra) = sets.pps.get(&b.ue()?)?;
    let &(bits, separate) = sets.sps.get(&sps_id)?;
    b.u(extra)?;
    b.ue()?;
    if output {
        b.u(1)?;
    }
    if separate {
        b.u(2)?;
    }
    if t == 19 || t == 20 {
        return Some(Some((0, bits)));
    }
    Some(Some((b.u(bits)?, bits)))
}

/// Order count from its lsb: the msb follows the previous reference picture's.
fn poc(lsb: u32, bits: u32, prev: &mut (i64, i64), update: bool) -> i64 {
    let max = 1i64 << bits;
    let (pmsb, plsb) = *prev;
    let lsb = i64::from(lsb);
    let msb = if lsb < plsb && plsb - lsb >= max / 2 {
        pmsb + max
    } else if lsb > plsb && lsb - plsb > max / 2 {
        pmsb - max
    } else {
        pmsb
    };
    if update {
        *prev = (msb, lsb);
    }
    msb + lsb
}

/// One access unit: where its bytes are, its order count and the epoch (IDR period) it
/// belongs to.
struct Unit {
    start: usize,
    end: usize,
    epoch: u32,
    poc: i64,
}

fn annex_b(data: &[u8], kind: Kind, movie: &mut Movie) -> Result<(Vec<Unit>, ParamSets, bool)> {
    let starts = nal_starts(data);
    let mut units: Vec<Unit> = Vec::new();
    let mut sets = ParamSets::default();
    let (mut avc_sps_map, mut avc_pps_map) = (HashMap::new(), HashMap::new());
    let mut hevc = HevcSets::default();
    let mut decode_order = false;
    // The current unit: its start, whether it has a picture yet, its epoch and order count.
    let mut cur: Option<(usize, bool, u32, i64)> = None;
    let mut epoch = 0u32;
    let mut prev = (0i64, 0i64);
    let (mut frame_off, mut prev_frame_num) = (0i64, 0u32);
    let mut count = 0usize;
    for (k, &(at, code)) in starts.iter().enumerate() {
        let end = starts.get(k + 1).map_or(data.len(), |n| n.1);
        let nal = &data[at..end.max(at)];
        let Some(&h0) = nal.first() else { continue };
        // Whether this unit begins an access unit, and its picture's order count.
        let (vcl, opens, pic) = match kind {
            Kind::H264 => {
                let t = h0 & 0x1F;
                match t {
                    7 => {
                        if let Some((id, s)) = avc_sps(nal) {
                            avc_sps_map.insert(id, s);
                            decode_order |= s.poc_type == 1;
                        }
                    }
                    8 => {
                        if let Some((p, s, bf)) = avc_pps(nal) {
                            avc_pps_map.insert(p, (s, bf));
                        }
                    }
                    _ => {}
                }
                if t == 1 || t == 5 {
                    match avc_slice(nal, &avc_sps_map, &avc_pps_map) {
                        Some((sl, s)) if sl.first_mb == 0 => {
                            let idr = t == 5;
                            let reference = h0 & 0x60 != 0;
                            if idr {
                                prev = (0, 0);
                                frame_off = 0;
                            }
                            let p = match s.poc_type {
                                0 => poc(sl.poc_lsb, s.log2_poc_lsb, &mut prev, reference),
                                2 => {
                                    if !idr && prev_frame_num > sl.frame_num {
                                        frame_off += 1i64 << s.log2_frame_num;
                                    }
                                    prev_frame_num = sl.frame_num;
                                    let v = 2 * (frame_off + i64::from(sl.frame_num));
                                    if reference { v } else { v - 1 }
                                }
                                _ => count as i64,
                            };
                            (true, true, Some((idr, p)))
                        }
                        Some(_) => (true, false, None),
                        // Parameter sets not seen yet: a picture of its own.
                        None => (true, true, Some((false, count as i64))),
                    }
                } else {
                    (false, matches!(t, 6..=9 | 14..=18), None)
                }
            }
            _ => {
                let t = (h0 >> 1) & 0x3F;
                match t {
                    33 => {
                        if let Some((id, bits, sep)) = hevc_sps(nal) {
                            hevc.sps.insert(id, (bits, sep));
                        }
                    }
                    34 => {
                        if let Some((id, p)) = hevc_pps(nal) {
                            hevc.pps.insert(id, p);
                        }
                    }
                    _ => {}
                }
                if t < 32 {
                    match hevc_poc_lsb(nal, &hevc) {
                        Some(Some((lsb, bits))) => {
                            let tid = nal.get(1).map_or(0, |b| (b & 7).saturating_sub(1));
                            // IDR, BLA: the count starts again.
                            let reset = (16..=20).contains(&t);
                            if reset {
                                prev = (0, 0);
                            }
                            let sub_layer_non_ref = t <= 14 && t % 2 == 0;
                            let update = tid == 0 && !(6..=9).contains(&t) && !sub_layer_non_ref;
                            (true, true, Some((reset, poc(lsb, bits, &mut prev, update))))
                        }
                        Some(None) => (true, false, None),
                        None => (true, true, Some((false, count as i64))),
                    }
                } else {
                    (false, matches!(t, 32..=35 | 39 | 41..=44 | 48..=55), None)
                }
            }
        };
        let had_picture = cur.is_some_and(|c| c.1);
        if opens && had_picture || cur.is_none() {
            if let Some((s, true, e, p)) = cur {
                units.push(Unit {
                    start: s,
                    end: code,
                    epoch: e,
                    poc: p,
                });
                count += 1;
                if units.len() > MAX_SAMPLES {
                    return Err(Error::Unsupported(format!("{} frames", units.len())));
                }
            }
            cur = Some((code, false, epoch, 0));
        }
        if let (Some(c), Some((reset, p))) = (cur.as_mut(), pic) {
            if reset && units.last().is_some_and(|u| u.epoch == epoch) {
                epoch += 1;
            }
            if !c.1 {
                *c = (c.0, true, epoch, p);
            }
        }
        if vcl && let Some(c) = cur.as_mut() {
            c.1 = true;
        }
    }
    if let Some((s, true, e, p)) = cur {
        units.push(Unit {
            start: s,
            end: data.len(),
            epoch: e,
            poc: p,
        });
    }
    // Parameter sets and keyframes, as the transport-stream demuxer reads them.
    for u in units.iter().take(64) {
        let au = &data[u.start..u.end];
        match kind {
            Kind::H264 => {
                crate::ts::h264_kind(au, &mut sets, movie);
            }
            _ => {
                crate::ts::hevc_kind(au, &mut sets);
            }
        }
    }
    Ok((units, sets, decode_order))
}

/// The leb128 number at `p`, and where it ends.
fn leb128(d: &[u8], mut p: usize) -> Option<(u64, usize)> {
    let mut v = 0u64;
    for i in 0..8 {
        let b = *d.get(p)?;
        p += 1;
        v |= u64::from(b & 0x7F) << (7 * i);
        if b & 0x80 == 0 {
            return Some((v, p));
        }
    }
    None
}

/// The size and frame rate an AV1 sequence header gives.
fn av1_sequence(obu: &[u8]) -> Option<Sequence> {
    let mut b = Bits { b: obu, p: 0 };
    b.u(3)?;
    b.u(1)?;
    let reduced = b.u(1)? == 1;
    let mut rate = None;
    if reduced {
        b.u(5)?;
    } else {
        let timing = b.u(1)? == 1;
        let mut decoder_model = false;
        let mut delay_len = 0;
        if timing {
            let (units, scale) = (b.u(32)?, b.u(32)?);
            let mut ticks = 1u32;
            if b.u(1)? == 1 {
                // uvlc
                let mut z = 0;
                while b.u(1)? == 0 {
                    z += 1;
                    if z >= 32 {
                        return None;
                    }
                }
                ticks = b.u(z)?.saturating_add((1u32 << z) - 1).saturating_add(1);
            }
            if units > 0 && scale > 0 {
                rate = Some((scale, units.saturating_mul(ticks)));
            }
            decoder_model = b.u(1)? == 1;
            if decoder_model {
                delay_len = b.u(5)? + 1;
                b.u(32)?;
                b.u(5)?;
                b.u(5)?;
            }
        }
        let initial_delay = b.u(1)? == 1;
        for _ in 0..=b.u(5)? {
            b.u(12)?;
            let level = b.u(5)?;
            if level > 7 {
                b.u(1)?;
            }
            if decoder_model && b.u(1)? == 1 {
                b.u(delay_len)?;
                b.u(delay_len)?;
                b.u(1)?;
            }
            if initial_delay && b.u(1)? == 1 {
                b.u(4)?;
            }
        }
    }
    let (wb, hb) = (b.u(4)? + 1, b.u(4)? + 1);
    let (w, h) = (b.u(wb)? + 1, b.u(hb)? + 1);
    Some(((w, h), rate))
}

/// Temporal units of an OBU stream: (start, end), and the sequence header's facts.
/// A temporal unit: start, end, and whether it holds a sequence header.
type TemporalUnit = (usize, usize, bool);

fn obus(data: &[u8]) -> Result<(Vec<TemporalUnit>, Option<Sequence>)> {
    let mut units: Vec<TemporalUnit> = Vec::new();
    let mut seq = None;
    let mut at = 0;
    while at < data.len() {
        let h = data[at];
        let kind = (h >> 3) & 15;
        if h & 0x80 != 0 || h & 2 == 0 {
            return Err(bad("an OBU without a size field"));
        }
        let ext = usize::from(h & 4 != 0);
        let Some((size, body)) = leb128(data, at + 1 + ext) else {
            break;
        };
        let end = body.saturating_add(usize::try_from(size).unwrap_or(usize::MAX));
        if end > data.len() {
            break;
        }
        match kind {
            2 => {
                if let Some(u) = units.last_mut() {
                    u.1 = at;
                }
                units.push((at, data.len(), false));
                if units.len() > MAX_SAMPLES {
                    return Err(Error::Unsupported(format!("{} frames", units.len())));
                }
            }
            1 => {
                if seq.is_none() {
                    seq = av1_sequence(&data[body..end]);
                }
                if let Some(u) = units.last_mut() {
                    u.2 = true;
                }
            }
            _ => {}
        }
        at = end;
    }
    if let Some(u) = units.last_mut() {
        u.1 = u.1.min(at.max(u.0));
    }
    Ok((units, seq))
}

/// Parses a raw elementary stream. With `samples`, every access unit is listed.
pub fn parse(data: &[u8], samples: bool) -> Result<Movie> {
    let kind = sniff(data).ok_or_else(|| bad("not a raw video stream"))?;
    let mut movie = Movie {
        brand: *b"raw ",
        timescale: DEFAULT_FPS,
        duration_us: None,
        created: None,
        video: None,
        has_audio: false,
        audio: None,
        content_id: None,
        date: None,
        make: None,
        model: None,
        xmp: None,
        tags: Vec::new(),
    };
    let mut v = VideoTrack {
        timescale: DEFAULT_FPS,
        ..VideoTrack::default()
    };
    let mut dur = 1u32;
    let mut list: Vec<Sample> = Vec::new();
    match kind {
        Kind::Mjpeg => {
            let (pics, size) = mjpeg_frames(data);
            let (w, h) = size.ok_or_else(|| bad("no JPEG frame header"))?;
            (v.width, v.height) = (w, h);
            v.codec = Some(Codec::Mjpeg);
            v.layout = Layout::Contiguous;
            for (i, (s, e)) in pics.into_iter().enumerate() {
                list.push(Sample {
                    offset: s as u64,
                    size: (e - s).min(u32::MAX as usize) as u32,
                    dts: i as u64,
                    cts: i as i64,
                    duration: 1,
                    sync: true,
                });
            }
        }
        Kind::Av1 => {
            let (units, seq) = obus(data)?;
            let ((w, h), rate) = seq.ok_or_else(|| bad("no AV1 sequence header"))?;
            if let Some((scale, units)) = rate {
                v.timescale = scale;
                dur = units;
                movie.timescale = scale;
                movie.brand = *b"rawt";
            }
            (v.width, v.height) = (w, h);
            v.codec = Some(Codec::Av1);
            v.layout = Layout::Contiguous;
            for (i, (s, e, sync)) in units.into_iter().enumerate() {
                let t = i as u64 * u64::from(dur);
                list.push(Sample {
                    offset: s as u64,
                    size: (e - s).min(u32::MAX as usize) as u32,
                    dts: t,
                    cts: t as i64,
                    duration: dur,
                    sync,
                });
            }
        }
        _ => {
            let (units, sets, decode_order) = annex_b(data, kind, &mut movie)?;
            if decode_order {
                movie.brand = *b"rawd";
            }
            v.codec = Some(if kind == Kind::H264 {
                Codec::H264
            } else {
                Codec::Hevc
            });
            v.config = if kind == Kind::H264 {
                sets.avcc()
            } else {
                sets.hvcc()
            };
            if v.config.is_empty() {
                return Err(bad("no parameter sets in the video stream"));
            }
            if let Some((w, h)) = sets.size {
                (v.width, v.height) = (w, h);
            }
            if let Some(par) = sets.aspect {
                v.pixel_aspect = Some(par);
            }
            v.interlaced = sets.interlaced;
            v.layout = Layout::AnnexB;
            // Presentation slots: epoch by epoch, by order count.
            let mut order: Vec<usize> = (0..units.len()).collect();
            order.sort_by_key(|&i| (units[i].epoch, units[i].poc, i));
            let mut slot = vec![0usize; units.len()];
            for (k, &i) in order.iter().enumerate() {
                slot[i] = k;
            }
            // Decoding may not run behind presentation: shift by the largest reordering.
            let lag = slot
                .iter()
                .enumerate()
                .map(|(i, &s)| i.saturating_sub(s))
                .max()
                .unwrap_or(0) as i64;
            for (i, u) in units.iter().enumerate() {
                let au = &data[u.start..u.end];
                let sync = match kind {
                    Kind::H264 => crate::ts::h264_kind(au, &mut ParamSets::default(), &mut movie),
                    _ => crate::ts::hevc_kind(au, &mut ParamSets::default()),
                };
                list.push(Sample {
                    offset: u.start as u64,
                    size: (u.end - u.start).min(u32::MAX as usize) as u32,
                    dts: i as u64,
                    cts: slot[i] as i64 + lag,
                    duration: 1,
                    sync,
                });
            }
            v.edit.start = lag;
        }
    }
    if list.is_empty() {
        return Err(bad("a video stream without pictures"));
    }
    if !list.iter().any(|s| s.sync) {
        list[0].sync = true;
    }
    v.display = (v.width, v.height);
    v.duration = list
        .iter()
        .map(|s| s.cts.max(0) as u64 + u64::from(s.duration))
        .max()
        .unwrap_or(0)
        .saturating_sub(v.edit.start.max(0) as u64);
    v.sample_count = list.len() as u32;
    if samples {
        v.samples = list;
    }
    movie.duration_us = v.duration_us();
    movie.video = Some(v);
    Ok(movie)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_streams() {
        // H.264: an SPS first.
        let mut h = vec![0, 0, 0, 1, 0x67, 0x42, 0, 0x1E];
        h.extend_from_slice(&[0, 0, 1, 0x68, 0xCE]);
        assert_eq!(sniff(&h), Some(Kind::H264));
        // HEVC: a VPS, then an SPS.
        let mut v = vec![0, 0, 0, 1, 0x40, 0x01, 0x0C];
        v.extend_from_slice(&[0, 0, 1, 0x42, 0x01, 0x01]);
        assert_eq!(sniff(&v), Some(Kind::Hevc));
        // AV1: a temporal delimiter, then a sequence header.
        assert_eq!(sniff(&[0x12, 0x00, 0x0A, 0x0B, 0]), Some(Kind::Av1));
        assert_eq!(sniff(b"\0\0\0\x01not video"), None);
        assert_eq!(sniff(b"plain text"), None);
    }

    #[test]
    fn order_counts_wrap() {
        let mut prev = (0, 0);
        assert_eq!(poc(4, 4, &mut prev, true), 4);
        assert_eq!(poc(14, 4, &mut prev, true), -2);
        let mut prev = (0, 12);
        assert_eq!(poc(2, 4, &mut prev, true), 18);
    }
}
