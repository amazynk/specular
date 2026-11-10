//! Closed captions carried inside the video, gathered as subtitle streams for
//! `specular-subs` to decode (codecs `CEA-608` and `CEA-708`).
//!
//! Where they come from:
//! - H.264 and HEVC: SEI `user_data_registered_itu_t_t35` messages holding ATSC A/53
//!   `GA94` `cc_data` (any container: MP4, Matroska, transport and programme streams).
//! - MPEG-2 video: picture user data with `GA94` `cc_data`, or the DVD's `CC` user data (a
//!   GOP's worth of byte pairs, spread one frame apart).
//! - MP4/MOV closed caption tracks: `c608` samples (`cdat` and `cdt2` atoms) and `c708`
//!   samples (`ccdp` caption distribution packets).
//!
//! The data is put in presentation order, then each channel or service that carries text
//! becomes a stream: CC1..CC4 and T1..T4 (codec data 1..=8) for CEA-608, services 1..63
//! for CEA-708. Each packet holds one frame's `cc_data` triplets (`cc_type`, two bytes; only
//! valid ones, field 1 and 2 padding left out) at the frame's presentation time.
//!
//! The data is looked for in a sample of the frames first (the start and a few places
//! through the movie), so movies without captions are not read whole.

use crate::extras::{Packet, SubStream, TrackInfo};
use crate::mp4::{Codec, Movie, VideoTrack};

/// Codec names (as `specular_subs::CEA_608` and `CEA_708`).
pub const CEA_608: &str = "CEA-608";
pub const CEA_708: &str = "CEA-708";

/// Track ids given to caption streams (the container has none): this plus the channel
/// (608) or `ID_708` plus the service.
pub const ID_608: u64 = 0x6080_0000;
pub const ID_708: u64 = 0x7080_0000;

/// A caption service's language from a transport stream's caption service descriptor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceLang {
    /// A CEA-708 service, else a line 21 field.
    pub digital: bool,
    /// The 708 service number, or the 608 field (1 or 2).
    pub number: u8,
    pub lang: String,
}

/// A line 21 frame (30000/1001 a second) in microseconds.
const LINE21_FRAME_US: i64 = 33_367;

/// One frame's caption data: its presentation time and its triplets.
pub(crate) type Frame = (i64, Vec<u8>);

/// Frames probed before the whole movie is read.
const PROBE_START: usize = 300;
const PROBE_SPOTS: usize = 8;
const PROBE_SPOT_LEN: usize = 60;

/// The caption streams of a movie's video track, with the languages `langs` gives.
pub(crate) fn from_video(data: &[u8], movie: &Movie, langs: &[ServiceLang]) -> Vec<SubStream> {
    let Some(v) = movie.video.as_ref() else {
        return Vec::new();
    };
    if !matches!(
        v.codec,
        Some(Codec::H264 | Codec::Hevc | Codec::Mpeg2 | Codec::Mpeg1)
    ) {
        return Vec::new();
    }
    let n = v.samples.len();
    let len_size = nal_length_size(v);
    let mut probe: Vec<usize> = (0..n.min(PROBE_START)).collect();
    if n > PROBE_START {
        for k in 1..=PROBE_SPOTS {
            let at = n * k / (PROBE_SPOTS + 1);
            probe.extend(at..(at + PROBE_SPOT_LEN).min(n));
        }
    }
    let found = probe.iter().any(|&i| {
        let mut out = Vec::new();
        sample_captions(data, v, i, len_size, &mut out);
        !out.is_empty()
    });
    if !found {
        return Vec::new();
    }
    let mut frames = Vec::new();
    for i in 0..n {
        sample_captions(data, v, i, len_size, &mut frames);
    }
    streams(frames, langs)
}

/// The caption data of sample `i`, appended to `out`.
fn sample_captions(data: &[u8], v: &VideoTrack, i: usize, len_size: usize, out: &mut Vec<Frame>) {
    let s = &v.samples[i];
    let Some(bytes) = v.sample_data(data, s) else {
        return;
    };
    let t = crate::play::presented_at_us(v, s.cts);
    let mut cc = Vec::new();
    match v.codec {
        Some(Codec::H264 | Codec::Hevc) => {
            let hevc = v.codec == Some(Codec::Hevc);
            let mut p = 0;
            while p + len_size <= bytes.len() {
                let len = bytes[p..p + len_size]
                    .iter()
                    .fold(0usize, |a, &b| a << 8 | usize::from(b));
                p += len_size;
                let Some(nal) = bytes.get(p..p + len) else {
                    break;
                };
                p += len;
                let sei = if hevc {
                    nal.len() > 2 && matches!((nal[0] >> 1) & 0x3F, 39 | 40)
                } else {
                    nal.first().is_some_and(|b| b & 0x1F == 6)
                };
                if sei {
                    sei_captions(&unescape(&nal[if hevc { 2 } else { 1 }..]), &mut cc);
                }
            }
        }
        _ => {
            let frame_us = v.us(i64::from(s.duration)).clamp(1, 1_000_000);
            mpeg2_captions(&bytes, t, frame_us, &mut cc, out);
        }
    }
    if !cc.is_empty() {
        out.push((t, cc));
    }
}

/// The length of the NAL unit length fields (`avcC` / `hvcC`), 4 when unknown.
fn nal_length_size(v: &VideoTrack) -> usize {
    let b = match v.codec {
        Some(Codec::H264) => v.config.get(4),
        Some(Codec::Hevc) => v.config.get(21),
        _ => None,
    };
    b.map_or(4, |b| usize::from(b & 3) + 1)
}

/// Removes emulation prevention bytes (`00 00 03`).
fn unescape(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(b.len());
    let mut zeros = 0;
    for &x in b {
        if zeros >= 2 && x == 3 {
            zeros = 0;
            continue;
        }
        zeros = if x == 0 { zeros + 1 } else { 0 };
        out.push(x);
    }
    out
}

/// The `cc_data` of an SEI NAL unit's messages (its header removed, unescaped).
fn sei_captions(mut b: &[u8], cc: &mut Vec<u8>) {
    while b.len() >= 2 && b[0] != 0x80 {
        let mut kind = 0usize;
        while let Some((&x, rest)) = b.split_first() {
            b = rest;
            kind += usize::from(x);
            if x != 0xFF {
                break;
            }
        }
        let mut size = 0usize;
        while let Some((&x, rest)) = b.split_first() {
            b = rest;
            size += usize::from(x);
            if x != 0xFF {
                break;
            }
        }
        let Some(payload) = b.get(..size) else {
            return;
        };
        b = &b[size..];
        // user_data_registered_itu_t_t35: United States, ATSC, "GA94", cc_data.
        if kind == 4
            && payload.len() > 8
            && payload[0] == 0xB5
            && payload[1..3] == [0x00, 0x31]
            && &payload[3..7] == b"GA94"
            && payload[7] == 0x03
        {
            cc_data(&payload[8..], cc);
        }
    }
}

/// An A/53 `cc_data()` structure: flags and count, `em_data`, then the triplets.
fn cc_data(b: &[u8], cc: &mut Vec<u8>) {
    let Some(&flags) = b.first() else { return };
    if flags & 0x40 == 0 {
        return;
    }
    let count = usize::from(flags & 0x1F);
    triplets(b.get(2..).unwrap_or_default(), count, cc);
}

/// `count` triplets (`marker`, `cc_valid`, `cc_type`; two bytes): the valid ones that carry
/// something.
fn triplets(b: &[u8], count: usize, cc: &mut Vec<u8>) {
    for t in b.chunks_exact(3).take(count) {
        if t[0] & 0x04 == 0 {
            continue;
        }
        let kind = t[0] & 3;
        // Line 21 padding (nulls with parity).
        if kind < 2 && t[1] & 0x7F == 0 && t[2] & 0x7F == 0 {
            continue;
        }
        cc.extend_from_slice(&[kind, t[1], t[2]]);
    }
}

/// MPEG-2 picture user data: `GA94` `cc_data` into `cc` (this frame's), or DVD captions
/// (a GOP's worth, one frame apart from `t`) straight into `out`.
fn mpeg2_captions(b: &[u8], t: i64, frame_us: i64, cc: &mut Vec<u8>, out: &mut Vec<Frame>) {
    let mut p = 0;
    while let Some(at) = find_start(b, p) {
        p = at + 4;
        if b[at + 3] != 0xB2 {
            continue;
        }
        let end = find_start(b, p).unwrap_or(b.len());
        let u = &b[p..end];
        if u.len() > 5 && &u[..4] == b"GA94" && u[4] == 0x03 {
            cc_data(&u[5..], cc);
        } else if u.len() > 5 && u[..4] == [b'C', b'C', 0x01, 0xF8] {
            dvd_captions(&u[4..], t, frame_us, out);
        }
        p = end;
    }
}

/// The next `00 00 01` start code at or after `from`.
fn find_start(b: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while i + 3 < b.len() {
        if b[i + 2] > 1 {
            i += 3;
        } else if b[i] == 0 && b[i + 1] == 0 && b[i + 2] == 1 {
            return Some(i);
        } else {
            i += 1;
        }
    }
    None
}

/// DVD captions: a header byte (block count in bits 5..1, one extra word in bit 0), then
/// words of three bytes (bit 0 of the first: field 1).
fn dvd_captions(b: &[u8], t: i64, frame_us: i64, out: &mut Vec<Frame>) {
    let Some(&h) = b.first() else { return };
    let words = usize::from((h >> 1) & 0x1F) * 2 + usize::from(h & 1);
    let mut frame: Vec<u8> = Vec::new();
    let mut k = 0;
    let mut seen_field1 = false;
    for w in b[1..].chunks_exact(3).take(words) {
        let field = if w[0] & 1 != 0 { 0 } else { 1 };
        if field == 0 && seen_field1 {
            // A new frame begins with its field 1 word.
            if !frame.is_empty() {
                out.push((t + k * frame_us, std::mem::take(&mut frame)));
            }
            k += 1;
        }
        seen_field1 |= field == 0;
        if w[1] & 0x7F != 0 || w[2] & 0x7F != 0 {
            frame.extend_from_slice(&[field, w[1], w[2]]);
        }
    }
    if !frame.is_empty() {
        out.push((t + k * frame_us, frame));
    }
}

/// The caption data of an MP4 closed caption track's sample (`c608` or `c708`), spread
/// over its duration when it holds several frames' worth.
pub(crate) fn from_mp4_sample(
    entry: &[u8; 4],
    sample: &[u8],
    t: i64,
    dur: i64,
    out: &mut Vec<Frame>,
) {
    let mut fields: [Vec<[u8; 2]>; 2] = [Vec::new(), Vec::new()];
    let mut cc = Vec::new();
    let mut p = 0;
    while p + 8 <= sample.len() {
        let size =
            u32::from_be_bytes([sample[p], sample[p + 1], sample[p + 2], sample[p + 3]]) as usize;
        if size < 8 || p + size > sample.len() {
            break;
        }
        let body = &sample[p + 8..p + size];
        match (&sample[p + 4..p + 8], entry) {
            (b"cdat", b"c608") => fields[0].extend(body.chunks_exact(2).map(|c| [c[0], c[1]])),
            (b"cdt2", b"c608") => fields[1].extend(body.chunks_exact(2).map(|c| [c[0], c[1]])),
            (b"ccdp", b"c708") => cdp(body, &mut cc),
            _ => {}
        }
        p += size;
    }
    if !cc.is_empty() {
        out.push((t, cc));
    }
    // Line 21 pairs: one a frame per field (at 29.97 frames a second, as line 21 carries
    // them), squeezed into the sample when it is shorter.
    let n = fields[0].len().max(fields[1].len());
    let step = if n > 0 {
        (dur / n as i64).min(LINE21_FRAME_US)
    } else {
        0
    };
    for k in 0..n {
        let mut f = Vec::new();
        for (field, pairs) in fields.iter().enumerate() {
            if let Some(pr) = pairs.get(k)
                && (pr[0] & 0x7F != 0 || pr[1] & 0x7F != 0)
            {
                f.extend_from_slice(&[field as u8, pr[0], pr[1]]);
            }
        }
        if !f.is_empty() {
            out.push((t + step * k as i64, f));
        }
    }
}

/// A caption distribution packet (SMPTE 334-2): its `ccdata_section` triplets.
fn cdp(b: &[u8], cc: &mut Vec<u8>) {
    if b.len() < 7 || b[0..2] != [0x96, 0x69] {
        return;
    }
    let flags = b[4];
    let mut p = 7;
    if flags & 0x80 != 0 {
        // time_code_section
        p += 5;
    }
    if flags & 0x40 == 0 || b.get(p) != Some(&0x72) {
        return;
    }
    let Some(&n) = b.get(p + 1) else { return };
    triplets(
        b.get(p + 2..).unwrap_or_default(),
        usize::from(n & 0x1F),
        cc,
    );
}

/// The streams in caption data: each CEA-608 channel and CEA-708 service with text.
pub(crate) fn streams(mut frames: Vec<Frame>, langs: &[ServiceLang]) -> Vec<SubStream> {
    if frames.is_empty() {
        return Vec::new();
    }
    frames.sort_by_key(|f| f.0);
    let last_t = frames.last().map_or(0, |f| f.0);
    let mut out = Vec::new();
    let found608 = channels_608(&frames);
    for code in 1..=8u8 {
        if !found608[usize::from(code - 1)] {
            continue;
        }
        let field = u8::from(matches!(code, 3 | 4 | 7 | 8));
        let lang = langs
            .iter()
            .find(|l| !l.digital && l.number == field + 1 && code <= 4)
            .map(|l| l.lang.clone());
        out.push(stream(
            CEA_608,
            ID_608 + u64::from(code),
            code,
            lang,
            &frames,
            last_t,
            |k| k == field,
        ));
    }
    for svc in services_708(&frames) {
        let lang = langs
            .iter()
            .find(|l| l.digital && l.number == svc)
            .map(|l| l.lang.clone());
        out.push(stream(
            CEA_708,
            ID_708 + u64::from(svc),
            svc,
            lang,
            &frames,
            last_t,
            |k| k >= 2,
        ));
    }
    out
}

fn stream(
    codec: &str,
    id: u64,
    code: u8,
    lang: Option<String>,
    frames: &[Frame],
    last_t: i64,
    keep: impl Fn(u8) -> bool,
) -> SubStream {
    let mut packets: Vec<Packet> = frames
        .iter()
        .filter_map(|(t, cc)| {
            let d: Vec<u8> = cc
                .chunks_exact(3)
                .filter(|c| keep(c[0]))
                .flatten()
                .copied()
                .collect();
            (!d.is_empty()).then_some(Packet {
                start_us: *t,
                dur_us: None,
                data: d,
            })
        })
        .collect();
    // The end of the caption data: what still shows ends soon after.
    if packets.last().is_some_and(|p| p.start_us < last_t) {
        packets.push(Packet {
            start_us: last_t,
            dur_us: None,
            data: Vec::new(),
        });
    }
    SubStream {
        info: TrackInfo {
            id,
            codec: codec.to_owned(),
            lang: lang.filter(|l| !l.is_empty() && l != "und"),
            hearing_impaired: true,
            ..TrackInfo::default()
        },
        private: vec![code],
        packets,
    }
}

/// Which CEA-608 channels carry text: CC1..CC4 then T1..T4.
fn channels_608(frames: &[Frame]) -> [bool; 8] {
    let mut found = [false; 8];
    // Per field: the data channel and mode last named, the last control code.
    let mut chan = [0u8; 2];
    let mut text = [[false; 2]; 2];
    let mut last: [Option<(u8, u8)>; 2] = [None; 2];
    for (_, cc) in frames {
        for c in cc.chunks_exact(3) {
            let f = usize::from(c[0]);
            if f > 1 || c[1].count_ones().is_multiple_of(2) {
                continue;
            }
            let (hi, lo) = (c[1] & 0x7F, c[2] & 0x7F);
            let printable = if (0x10..=0x1F).contains(&hi) {
                if last[f] == Some((hi, lo)) {
                    last[f] = None;
                    continue;
                }
                last[f] = Some((hi, lo));
                let ch = u8::from(hi & 0x08 != 0);
                chan[f] = ch;
                let base = hi & !0x08;
                if matches!(base, 0x14 | 0x15) && (0x20..=0x2F).contains(&lo) {
                    match lo {
                        0x2A | 0x2B => text[f][usize::from(ch)] = true,
                        0x20 | 0x25..=0x27 | 0x29 => text[f][usize::from(ch)] = false,
                        _ => {}
                    }
                }
                matches!((base, lo), (0x11, 0x30..=0x3F) | (0x12 | 0x13, 0x20..=0x3F))
            } else {
                last[f] = None;
                hi >= 0x20
            };
            if printable {
                let ch = usize::from(chan[f]);
                let k = if text[f][ch] { 4 } else { 0 } + f * 2 + ch;
                found[k] = true;
            }
        }
    }
    found
}

/// The CEA-708 services that carry text, in order.
fn services_708(frames: &[Frame]) -> Vec<u8> {
    let mut found = [false; 64];
    let mut packet: Vec<u8> = Vec::new();
    let mut done = |p: &[u8]| {
        if let Some(&h) = p.first() {
            let len = packet_len(h).min(p.len());
            for (svc, block) in service_blocks(&p[1..len]) {
                if svc > 0 && has_text(block) {
                    found[usize::from(svc)] = true;
                }
            }
        }
    };
    for (_, cc) in frames {
        for c in cc.chunks_exact(3) {
            match c[0] {
                3 => {
                    done(&packet);
                    packet.clear();
                    packet.extend_from_slice(&c[1..3]);
                }
                2 if !packet.is_empty() => packet.extend_from_slice(&c[1..3]),
                _ => continue,
            }
            if packet.len() >= packet_len(packet[0]) {
                done(&packet);
                packet.clear();
            }
        }
    }
    done(&packet);
    (1..64u8).filter(|&s| found[usize::from(s)]).collect()
}

fn packet_len(header: u8) -> usize {
    match header & 0x3F {
        0 => 128,
        n => usize::from(n) * 2,
    }
}

/// A caption channel packet's service blocks: (service, block).
fn service_blocks(mut d: &[u8]) -> Vec<(u8, &[u8])> {
    let mut out = Vec::new();
    while let Some(&h) = d.first() {
        let (mut svc, size) = (h >> 5, usize::from(h & 0x1F));
        d = &d[1..];
        if svc == 0 && size == 0 {
            break;
        }
        if svc == 7 && size != 0 {
            let Some(&x) = d.first() else { break };
            svc = x & 0x3F;
            d = &d[1..];
        }
        let Some(block) = d.get(..size) else { break };
        out.push((svc, block));
        d = &d[size..];
    }
    out
}

/// Whether a service block writes a character (commands and their parameters skipped).
fn has_text(b: &[u8]) -> bool {
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        i += 1;
        match c {
            0x00..=0x0F => {}
            0x10 => {
                let e = b.get(i).copied().unwrap_or(0);
                i += 1;
                match e {
                    0x00..=0x07 => {}
                    0x08..=0x0F => i += 1,
                    0x10..=0x17 => i += 2,
                    0x18..=0x1F => i += 3,
                    0x80..=0x87 => i += 4,
                    0x88..=0x8F => i += 5,
                    0x90..=0x9F => i += 1 + usize::from(b.get(i).copied().unwrap_or(0) & 0x3F),
                    _ => return true,
                }
            }
            0x11..=0x17 => i += 1,
            0x18 => return true,
            0x19..=0x1F => i += 2,
            0x20..=0x7F | 0xA0..=0xFF => return true,
            0x88..=0x8D => i += 1,
            0x90 | 0x92 => i += 2,
            0x91 => i += 3,
            0x97 => i += 4,
            0x98..=0x9F => i += 6,
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parity(b: u8) -> u8 {
        if b.count_ones().is_multiple_of(2) {
            b | 0x80
        } else {
            b
        }
    }

    /// An H.264 SEI NAL unit with `cc_data` of these triplets (emulation prevention added).
    pub(crate) fn sei(trip: &[[u8; 3]]) -> Vec<u8> {
        let mut p = vec![0xB5, 0x00, 0x31, b'G', b'A', b'9', b'4', 0x03];
        p.push(0x40 | trip.len() as u8);
        p.push(0xFF);
        for t in trip {
            p.extend_from_slice(t);
        }
        p.push(0xFF);
        let mut rbsp = vec![4, p.len() as u8];
        rbsp.extend(p);
        rbsp.push(0x80);
        let mut nal = vec![0x06];
        let mut zeros = 0;
        for &b in &rbsp {
            if zeros >= 2 && b <= 3 {
                nal.push(3);
                zeros = 0;
            }
            zeros = if b == 0 { zeros + 1 } else { 0 };
            nal.push(b);
        }
        nal
    }

    #[test]
    fn sei_and_unescaping() {
        let n = sei(&[
            [0xFC, parity(0x14), parity(0x20)],
            [0xFD, 0x80, 0x80],
            [0xFA, 0, 0],
        ]);
        let mut cc = Vec::new();
        sei_captions(&unescape(&n[1..]), &mut cc);
        // Field 2 padding and the invalid triplet are left out.
        assert_eq!(cc, [0, parity(0x14), parity(0x20)]);
        assert_eq!(unescape(&[0, 0, 3, 1, 0, 0, 3]), [0, 0, 1, 0, 0]);
    }

    #[test]
    fn mpeg2_user_data_and_dvd() {
        let mut es = vec![0, 0, 1, 0x00, 0x11, 0x22];
        es.extend([0, 0, 1, 0xB2]);
        es.extend(b"GA94\x03");
        es.extend([0x42, 0xFF, 0xFC, 0xC1, 0xC2, 0xFA, 0x00, 0x00]);
        es.extend([0, 0, 1, 0x01, 0xAA]);
        let (mut cc, mut out) = (Vec::new(), Vec::new());
        mpeg2_captions(&es, 0, 33_000, &mut cc, &mut out);
        assert_eq!(cc, [0, 0xC1, 0xC2]);
        // DVD: two frames (field 1 and 2 words each).
        let mut dvd = vec![0, 0, 1, 0xB2, b'C', b'C', 1, 0xF8, 0x80 | (2 << 1)];
        for (f, a) in [(1, 0xC1), (0, 0x80), (1, 0xC2), (0, 0x80)] {
            dvd.extend([0xFE | f, a, 0x80]);
        }
        let (mut cc, mut out) = (Vec::new(), Vec::new());
        mpeg2_captions(&dvd, 1000, 33_000, &mut cc, &mut out);
        assert_eq!(
            out,
            [(1000, vec![0, 0xC1, 0x80]), (34_000, vec![0, 0xC2, 0x80])]
        );
    }

    #[test]
    fn mp4_caption_samples() {
        let mut s = Vec::new();
        s.extend(12u32.to_be_bytes());
        s.extend(b"cdat");
        s.extend([0xC1, 0xC2, 0xC3, 0x80]);
        let s608 = s.clone();
        let mut out = Vec::new();
        from_mp4_sample(b"c608", &s, 0, 66_000, &mut out);
        assert_eq!(
            out,
            [(0, vec![0, 0xC1, 0xC2]), (33_000, vec![0, 0xC3, 0x80])]
        );
        // A long sample: one pair a line 21 frame.
        let mut out = Vec::new();
        from_mp4_sample(b"c608", &s608, 0, 1_000_000, &mut out);
        assert_eq!(out[1].0, LINE21_FRAME_US);
        let mut c = vec![
            0x96, 0x69, 0, 0x4F, 0x43, 0, 0, 0x72, 0xE1, 0xFF, 0x05, 0x41,
        ];
        c.push(0x74);
        let mut s = Vec::new();
        s.extend((8 + c.len() as u32).to_be_bytes());
        s.extend(b"ccdp");
        s.extend(&c);
        let mut out = Vec::new();
        from_mp4_sample(b"c708", &s, 5, 33_000, &mut out);
        assert_eq!(out, [(5, vec![3, 0x05, 0x41])]);
    }

    #[test]
    fn channels_and_services_with_text() {
        let c = |f: u8, a: u8, b: u8| vec![f, parity(a), parity(b)];
        let frames = vec![
            // CC1 control, then text: CC1.
            (0, [c(0, 0x14, 0x29), c(0, 0x14, 0x29)].concat()),
            (1, c(0, b'H', b'I')),
            // CC2 text mode (TR): T2.
            (
                2,
                [c(0, 0x1C, 0x2A), c(0, 0x1C, 0x2A), c(0, b'T', b'X')].concat(),
            ),
            // Field 2: CC3.
            (3, [c(1, 0x15, 0x2C), c(1, b'A', 0)].concat()),
        ];
        let f = channels_608(&frames);
        assert_eq!(f, [true, false, true, false, false, true, false, false]);
        // 708: service 1 defines a window only, service 2 writes text (header filled below).
        let mut pkt = vec![0u8];
        pkt.extend([0x27, 0x98, 0x20, 0x80, 0, 0, 0x1F, 0x01, 0x42, 0x01, b'x']);
        let len = pkt.len().div_ceil(2) * 2;
        pkt.resize(len, 0);
        pkt[0] = (len / 2) as u8;
        let tr: Vec<u8> = pkt
            .chunks(2)
            .enumerate()
            .flat_map(|(k, p)| [if k == 0 { 3 } else { 2 }, p[0], p[1]])
            .collect();
        assert_eq!(services_708(&[(0, tr.clone())]), [2]);
        let s = streams(
            vec![(0, tr)],
            &[ServiceLang {
                digital: true,
                number: 2,
                lang: "spa".into(),
            }],
        );
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].info.codec.as_str(), s[0].private[0]), (CEA_708, 2));
        assert_eq!(s[0].info.lang.as_deref(), Some("spa"));
        assert!(s[0].info.hearing_impaired);
    }
}
