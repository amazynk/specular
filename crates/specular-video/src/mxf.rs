//! MXF (SMPTE ST 377-1): OP1a and OP-Atom files, frame-wrapped and clip-wrapped essence,
//! into the same [`Movie`] the MP4 demuxer gives.
//!
//! - The file is a run of KLV triplets (a 16-byte SMPTE label, a BER length, the value):
//!   partition packs, header metadata (local sets with 2-byte tags), index table segments
//!   and essence elements. One walk over the triplets finds them all; header metadata
//!   repeated in later partitions (an open header closed in the footer) replaces the earlier.
//! - Frame wrapping: each content package holds one element per track, so every picture
//!   element is a sample. Clip wrapping: a track's essence is one element, cut into edit
//!   units by the index table (variable-size entries, or a constant edit unit byte count).
//! - Index tables give key frames and, for long-GOP MPEG-2 and H.264, the temporal offsets
//!   from coded to presented order. Without them a long-GOP track is timed by slots
//!   ([`VideoTrack::slot_timing`]).
//! - Picture essence: ProRes, MPEG-2 (D-10 / IMX, XDCAM, Canon XF), H.264 (XAVC, AVC-Intra
//!   with parameter sets in the stream: Annex B, see [`Layout::AnnexB`]), JPEG 2000, DV, and
//!   uncompressed 4:2:2 (SMPTE 384M: 8-bit UYVY, 10-bit v210). Others (DNxHD, other
//!   uncompressed packings, RAW codings) are named and refused.
//! - Sound: AES3 or BWF PCM (generic container), or D-10's AES3 elements
//!   ([`Layout::D10Sound`]). A camera's separate mono tracks play the first two as left and
//!   right; an OP-Atom picture file's sound is found in the clip's sound files beside it
//!   ([`sibling_sound`]).

use crate::mp4::{
    AudioCodec, AudioTrack, Codec, Colour, Edit, Error, Layout, MAX_SAMPLES, Movie, Result, Sample,
    VideoTrack,
};

fn bad(s: impl Into<String>) -> Error {
    Error::Malformed(s.into())
}

/// The SMPTE label prefix.
const UL: [u8; 4] = [0x06, 0x0E, 0x2B, 0x34];
/// Partition pack keys up to the kind byte (13: 2 header, 3 body, 4 footer).
const PARTITION: [u8; 13] = [
    0x06, 0x0E, 0x2B, 0x34, 0x02, 0x05, 0x01, 0x01, 0x0D, 0x01, 0x02, 0x01, 0x01,
];
/// The search for the header partition covers the run-in (at most 64 KiB, ST 377-1 §6.5).
const RUN_IN: usize = 65_536;

/// Whether `head` is an MXF file: a header partition pack within the run-in.
pub fn sniff(head: &[u8]) -> bool {
    find_header(head).is_some()
}

fn find_header(d: &[u8]) -> Option<usize> {
    let end = d.len().min(RUN_IN + 16);
    (0..end.saturating_sub(16))
        .find(|&i| d[i] == 0x06 && d[i..].starts_with(&PARTITION) && d.get(i + 13) == Some(&0x02))
}

/// One KLV triplet: its key, where its value starts and how long it is.
struct Klv<'a> {
    key: &'a [u8; 16],
    at: usize,
    len: usize,
}

/// The triplet at `at`, if it is one.
fn klv(d: &[u8], at: usize) -> Option<Klv<'_>> {
    let key: &[u8; 16] = d.get(at..at + 16)?.try_into().ok()?;
    if key[..4] != UL {
        return None;
    }
    let b = *d.get(at + 16)?;
    let (len, head) = if b < 0x80 {
        (usize::from(b), 17)
    } else {
        let n = usize::from(b & 0x7F);
        if n == 0 || n > 8 {
            return None;
        }
        let mut v = 0u64;
        for &x in d.get(at + 17..at + 17 + n)? {
            v = v << 8 | u64::from(x);
        }
        (usize::try_from(v).ok()?, 17 + n)
    };
    Some(Klv {
        key,
        at: at + head,
        len,
    })
}

fn be(b: &[u8]) -> u64 {
    b.iter().fold(0, |v, &x| v << 8 | u64::from(x))
}

/// The local tags of a set's value.
fn tags(v: &[u8]) -> impl Iterator<Item = (u16, &[u8])> {
    let mut at = 0;
    std::iter::from_fn(move || {
        let h = v.get(at..at + 4)?;
        let tag = u16::from_be_bytes([h[0], h[1]]);
        let n = usize::from(u16::from_be_bytes([h[2], h[3]]));
        let body = v.get(at + 4..at + 4 + n)?;
        at += 4 + n;
        Some((tag, body))
    })
}

/// A batch or array of 16-byte references (`count`, `size`, items).
fn refs(b: &[u8]) -> Vec<[u8; 16]> {
    if b.len() < 8 {
        return Vec::new();
    }
    let n = be(&b[..4]) as usize;
    let size = be(&b[4..8]) as usize;
    if size != 16 {
        return Vec::new();
    }
    b[8..]
        .chunks_exact(16)
        .take(n)
        .filter_map(|c| c.try_into().ok())
        .collect()
}

fn rational(b: &[u8]) -> Option<(u32, u32)> {
    let n = be(b.get(..4)?) as u32;
    let d = be(b.get(4..8)?) as u32;
    (n > 0 && d > 0).then_some((n, d))
}

/// UTF-16BE text without its terminator.
fn utf16(b: &[u8]) -> String {
    let units: Vec<u16> = b
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .take_while(|&u| u != 0)
        .collect();
    String::from_utf16_lossy(&units).trim().to_owned()
}

/// What a file descriptor says.
#[derive(Clone, Debug, Default)]
struct Descriptor {
    kind: DescKind,
    linked_track: Option<u32>,
    sample_rate: Option<(u32, u32)>,
    container: Option<[u8; 16]>,
    coding: Option<[u8; 16]>,
    stored: (u32, u32),
    display: (u32, u32),
    aspect: Option<(u32, u32)>,
    frame_layout: Option<u8>,
    transfer: Option<[u8; 16]>,
    primaries: Option<[u8; 16]>,
    equations: Option<[u8; 16]>,
    audio_rate: Option<(u32, u32)>,
    channels: u32,
    bits: u32,
    block_align: u32,
    sound_coding: Option<[u8; 16]>,
    subs: Vec<[u8; 16]>,
    /// CDCI `ComponentDepth`, `HorizontalSubsampling`, `VerticalSubsampling`.
    component_depth: u32,
    h_sub: u32,
    v_sub: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum DescKind {
    #[default]
    Other,
    Picture,
    Sound,
    Multiple,
}

/// A track of a package.
#[derive(Clone, Copy, Debug, Default)]
struct Track {
    id: u32,
    number: u32,
    edit_rate: Option<(u32, u32)>,
}

/// An index table segment's entries.
#[derive(Clone, Debug, Default)]
struct IndexSegment {
    start: u64,
    duration: u64,
    edit_unit_bytes: u32,
    body_sid: u32,
    edit_rate: Option<(u32, u32)>,
    /// (temporal offset, flags, stream offset) per edit unit.
    entries: Vec<(i8, u8, u64)>,
}

/// An essence stream's elements, by track number.
#[derive(Clone, Debug)]
struct Stream {
    number: u32,
    key: [u8; 16],
    /// (value offset, length) per element.
    elements: Vec<(u64, u64)>,
}

#[derive(Default)]
struct Walk {
    sets: Vec<([u8; 16], u8, Vec<u8>)>,
    index: Vec<IndexSegment>,
    streams: Vec<Stream>,
    encrypted: bool,
}

fn walk(d: &[u8], from: usize) -> Walk {
    let mut w = Walk::default();
    let mut at = from;
    while let Some(k) = klv(d, at) {
        let end = k.at.saturating_add(k.len);
        let key = k.key;
        if key[5] == 0x53 && key[8..14] == [0x0D, 0x01, 0x01, 0x01, 0x01, 0x01] {
            // A header metadata local set.
            if let Some(v) = d.get(k.at..end) {
                let uid = tags(v)
                    .find(|t| t.0 == 0x3C0A)
                    .and_then(|t| <[u8; 16]>::try_from(t.1).ok())
                    .unwrap_or_default();
                w.sets.push((uid, key[14], v.to_vec()));
            }
        } else if key[5] == 0x53 && key[8..15] == [0x0D, 0x01, 0x02, 0x01, 0x01, 0x10, 0x01] {
            if let Some(v) = d.get(k.at..end) {
                w.index.push(index_segment(v));
            }
        } else if key[4] == 0x01 && key[8..12] == [0x0D, 0x01, 0x03, 0x01] {
            let number = be(&key[12..16]) as u32;
            let s = match w.streams.iter_mut().position(|s| s.number == number) {
                Some(i) => &mut w.streams[i],
                None => {
                    w.streams.push(Stream {
                        number,
                        key: *key,
                        elements: Vec::new(),
                    });
                    w.streams.last_mut().expect("just pushed")
                }
            };
            if s.elements.len() < MAX_SAMPLES {
                s.elements.push((k.at as u64, k.len as u64));
            }
        } else if key[4] == 0x02 && key[5] == 0x04 && key[8..12] == [0x0D, 0x01, 0x03, 0x01] {
            w.encrypted = true;
        }
        if end > d.len() {
            // A truncated last element: keep what is there.
            if let Some(s) = w.streams.last_mut()
                && let Some(e) = s.elements.last_mut()
                && e.0 == k.at as u64
            {
                e.1 = (d.len() - k.at) as u64;
            }
            break;
        }
        at = end;
    }
    w
}

fn index_segment(v: &[u8]) -> IndexSegment {
    let mut s = IndexSegment::default();
    let mut slices = 0usize;
    let mut pos_tables = 0usize;
    let mut raw_entries = None;
    for (tag, b) in tags(v) {
        match tag {
            0x3F0B => s.edit_rate = rational(b),
            0x3F0C => s.start = be(b),
            0x3F0D => s.duration = be(b),
            0x3F05 => s.edit_unit_bytes = be(b) as u32,
            0x3F07 => s.body_sid = be(b) as u32,
            0x3F08 => slices = usize::from(b.first().copied().unwrap_or(0)),
            0x3F0E => pos_tables = usize::from(b.first().copied().unwrap_or(0)),
            0x3F0A => raw_entries = Some(b),
            _ => {}
        }
    }
    if let Some(b) = raw_entries
        && b.len() >= 8
    {
        let n = be(&b[..4]) as usize;
        let size = be(&b[4..8]) as usize;
        if size >= 11 + 4 * slices + 8 * pos_tables {
            s.entries = b[8..]
                .chunks_exact(size)
                .take(n.min(MAX_SAMPLES))
                .map(|e| (e[0] as i8, e[2], be(&e[3..11])))
                .collect();
        }
    }
    s
}

fn descriptor(kind: u8, v: &[u8]) -> Descriptor {
    let mut d = Descriptor {
        kind: match kind {
            0x27..=0x29 | 0x51 => DescKind::Picture,
            0x42 | 0x47 | 0x48 => DescKind::Sound,
            0x44 => DescKind::Multiple,
            _ => DescKind::Other,
        },
        ..Descriptor::default()
    };
    let ul = |b: &[u8]| <[u8; 16]>::try_from(b).ok();
    let u32_of = |b: &[u8]| be(b) as u32;
    for (tag, b) in tags(v) {
        match tag {
            0x3006 => d.linked_track = Some(u32_of(b)),
            0x3001 => d.sample_rate = rational(b),
            0x3004 => d.container = ul(b),
            0x3201 => d.coding = ul(b),
            0x3202 => d.stored.1 = u32_of(b),
            0x3203 => d.stored.0 = u32_of(b),
            0x3208 => d.display.1 = u32_of(b),
            0x3209 => d.display.0 = u32_of(b),
            0x320E => d.aspect = rational(b),
            0x320C => d.frame_layout = b.first().copied(),
            0x3210 => d.transfer = ul(b),
            0x3219 => d.primaries = ul(b),
            0x321A => d.equations = ul(b),
            0x3D03 => d.audio_rate = rational(b),
            0x3D07 => d.channels = u32_of(b),
            0x3D01 => d.bits = u32_of(b),
            0x3D0A => d.block_align = u32_of(b),
            0x3D06 => d.sound_coding = ul(b),
            0x3F01 => d.subs = refs(b),
            0x3301 => d.component_depth = u32_of(b),
            0x3302 => d.h_sub = u32_of(b),
            0x3308 => d.v_sub = u32_of(b),
            _ => {}
        }
    }
    d
}

/// The header metadata, resolved.
#[derive(Default)]
struct Metadata {
    descriptors: Vec<([u8; 16], Descriptor)>,
    tracks: Vec<Track>,
    /// The material package's UMID: every OP-Atom file of a clip has the same one.
    material: Option<[u8; 32]>,
    company: Option<String>,
    product: Option<String>,
    modified: Option<i64>,
}

fn metadata(sets: &[([u8; 16], u8, Vec<u8>)]) -> Metadata {
    let mut m = Metadata::default();
    for (uid, kind, v) in sets {
        match kind {
            0x3B | 0x3A => {
                let mut t = Track::default();
                for (tag, b) in tags(v) {
                    match tag {
                        0x4801 => t.id = be(b) as u32,
                        0x4804 => t.number = be(b) as u32,
                        0x4B01 => t.edit_rate = rational(b),
                        _ => {}
                    }
                }
                if t.number != 0 {
                    m.tracks.retain(|x| x.number != t.number);
                    m.tracks.push(t);
                }
            }
            0x36 => {
                if let Some((_, b)) = tags(v).find(|t| t.0 == 0x4401)
                    && let Ok(u) = <[u8; 32]>::try_from(b)
                {
                    m.material = Some(u);
                }
            }
            0x30 => {
                for (tag, b) in tags(v) {
                    match tag {
                        0x3C01 if m.company.is_none() => m.company = Some(utf16(b)),
                        0x3C02 if m.product.is_none() => m.product = Some(utf16(b)),
                        0x3C06 if m.modified.is_none() => m.modified = timestamp(b),
                        _ => {}
                    }
                }
            }
            _ => {
                let d = descriptor(*kind, v);
                if d.kind != DescKind::Other {
                    m.descriptors.retain(|x| x.0 != *uid);
                    m.descriptors.push((*uid, d));
                }
            }
        }
    }
    m
}

/// An MXF timestamp (year, month, day, hour, minute, second, 1/250 s) as seconds since 1970.
fn timestamp(b: &[u8]) -> Option<i64> {
    if b.len() < 8 {
        return None;
    }
    let y = i64::from(u16::from_be_bytes([b[0], b[1]]));
    let s = format!(
        "{y:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        b[2], b[3], b[4], b[5], b[6]
    );
    if y < 1900 {
        return None;
    }
    crate::session::wall_clock(&s)
}

impl Metadata {
    /// The descriptor of the track whose essence elements carry `number`: the one linked to
    /// that track, else the first of the kind.
    fn descriptor_for(&self, number: u32, kind: DescKind) -> Option<&Descriptor> {
        let id = self
            .tracks
            .iter()
            .find(|t| t.number == number)
            .map(|t| t.id);
        let of_kind = || {
            self.descriptors
                .iter()
                .map(|d| &d.1)
                .filter(|d| d.kind == kind)
        };
        id.and_then(|id| of_kind().find(|d| d.linked_track == Some(id)))
            .or_else(|| of_kind().next())
    }

    fn edit_rate(&self, number: u32) -> Option<(u32, u32)> {
        self.tracks
            .iter()
            .find(|t| t.number == number)
            .and_then(|t| t.edit_rate)
    }
}

/// The picture coding a descriptor's labels (or the first element) name.
fn picture_codec(d: &Descriptor, first: &[u8]) -> (Codec, Vec<u8>) {
    if let Some(c) = d.coding.filter(|c| c[..4] == UL && c[8] == 0x04) {
        let k = &c[8..];
        match (k[1], k[2], k[3], k[4], k[5]) {
            // 04.01.02.02.03.06.xx: ProRes, the profile in the last byte.
            (0x01, 0x02, 0x02, 0x03, 0x06) => {
                let fourcc: &[u8; 4] = match k[6] {
                    0x01 => b"apco",
                    0x02 => b"apcs",
                    0x03 => b"apcn",
                    0x04 => b"apch",
                    0x05 => b"ap4h",
                    _ => b"ap4x",
                };
                return (Codec::ProRes, fourcc.to_vec());
            }
            (0x01, 0x02, 0x02, 0x71, _) => return (Codec::Other(*b"AVdn"), Vec::new()),
            (0x01, 0x02, 0x02, 0x03, 0x01) => return (Codec::Jpeg2000, Vec::new()),
            (0x01, 0x02, 0x02, 0x02, _) => return (Codec::Dv, Vec::new()),
            (0x01, 0x02, 0x01, _, _) => return (Codec::Raw, raw_config(d, first)),
            (0x01, 0x02, 0x02, 0x01, 0x20..=0x2F) => return (Codec::Mpeg4, Vec::new()),
            (0x01, 0x02, 0x02, 0x01, 0x30..=0x3F) => {
                return (h264_or_headerless(first), Vec::new());
            }
            (0x01, 0x02, 0x02, 0x01, 0x10) => return (Codec::Mpeg1, Vec::new()),
            (0x01, 0x02, 0x02, 0x01, _) => return (Codec::Mpeg2, Vec::new()),
            _ => {}
        }
    }
    // The essence container's mapping, else the bytes themselves.
    if let Some(c) = d
        .container
        .filter(|c| c[..4] == UL && c[8..12] == [0x0D, 0x01, 0x03, 0x01])
    {
        match c[13] {
            0x01 | 0x04 => return (Codec::Mpeg2, Vec::new()),
            0x10 => return (h264_or_headerless(first), Vec::new()),
            0x11 => return (Codec::Other(*b"AVdn"), Vec::new()),
            0x0C => return (Codec::Jpeg2000, Vec::new()),
            0x02 => return (Codec::Dv, Vec::new()),
            0x05 => return (Codec::Raw, raw_config(d, first)),
            0x1C => return (Codec::ProRes, Vec::new()),
            _ => {}
        }
    }
    if first.get(4..8) == Some(b"icpf") {
        return (Codec::ProRes, Vec::new());
    }
    if first.starts_with(&[0, 0, 1, 0xB3]) {
        return (Codec::Mpeg2, Vec::new());
    }
    (Codec::Other(*b"mxfv"), Vec::new())
}

/// The layout of uncompressed picture essence (SMPTE 384M) from the CDCI descriptor: 8-bit
/// 4:2:2 is UYVY; 10-bit 4:2:2 is v210 when the frame has v210's size. Other packings get a
/// code the player refuses by name.
fn raw_config(d: &Descriptor, first: &[u8]) -> Vec<u8> {
    let (w, h) = (d.stored.0 as usize, d.stored.1 as usize);
    let fourcc = match (d.component_depth, d.h_sub, d.v_sub.max(1)) {
        (8, 2, 1) => *b"UYVY",
        (10, 2, 1) if !first.is_empty() && first.len() == w.div_ceil(48) * 128 * h => *b"v210",
        _ => *crate::rawvid::MXF_OTHER,
    };
    crate::rawvid::PixDesc {
        fourcc,
        depth: d.component_depth as u16,
        ..Default::default()
    }
    .to_config()
}

/// H.264, or AVC-Intra that leaves its parameter sets out of the stream (decoders that
/// read it carry tables of them; this one does not).
fn h264_or_headerless(first: &[u8]) -> Codec {
    let sps = crate::ts::nal_units(first).any(|n| n.first().is_some_and(|h| h & 0x1F == 7));
    if sps {
        Codec::H264
    } else {
        Codec::Other(*b"AVCI")
    }
}

/// An `avcC` record of the parameter sets in Annex B access units.
pub(crate) fn avcc(units: &[&[u8]]) -> Vec<u8> {
    let mut sps: Vec<&[u8]> = Vec::new();
    let mut pps: Vec<&[u8]> = Vec::new();
    for u in units {
        for n in crate::ts::nal_units(u) {
            let list = match n.first().map(|h| h & 0x1F) {
                Some(7) => &mut sps,
                Some(8) => &mut pps,
                _ => continue,
            };
            if list.len() < 16 && !list.contains(&n) {
                list.push(n);
            }
        }
    }
    let Some(first) = sps.first().filter(|s| s.len() >= 4) else {
        return Vec::new();
    };
    let mut v = vec![
        1,
        first[1],
        first[2],
        first[3],
        0xFF,
        0xE0 | sps.len() as u8,
    ];
    for s in &sps {
        v.extend_from_slice(&(s.len() as u16).to_be_bytes());
        v.extend_from_slice(s);
    }
    v.push(pps.len() as u8);
    for p in &pps {
        v.extend_from_slice(&(p.len() as u16).to_be_bytes());
        v.extend_from_slice(p);
    }
    v
}

/// Whether an H.264 access unit (Annex B) holds an IDR picture or an I slice.
pub(crate) fn h264_key(au: &[u8]) -> bool {
    for n in crate::ts::nal_units(au) {
        match n.first().map(|h| h & 0x1F) {
            Some(5) => return true,
            Some(1) => {
                // first_mb_in_slice ue, then slice_type ue: I is 2 or 7.
                let mut b = ExpGolomb { b: n, p: 8 };
                return b.ue().is_some() && matches!(b.ue(), Some(2 | 4 | 7 | 9));
            }
            _ => {}
        }
    }
    false
}

/// What the picture order counts of H.264 slices need from the sequence parameter set.
#[derive(Clone, Copy)]
struct PocSps {
    frame_num_bits: u32,
    poc_type: u32,
    poc_lsb_bits: u32,
    frame_mbs_only: bool,
    separate_planes: bool,
}

fn poc_sps(nal: &[u8]) -> Option<PocSps> {
    let rbsp = crate::ts::unescape(nal.get(1..)?);
    let mut b = ExpGolomb { b: &rbsp, p: 0 };
    let profile = b.bits(8)?;
    b.bits(16)?;
    b.ue()?;
    let mut separate_planes = false;
    if matches!(
        profile,
        100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 135
    ) {
        let chroma = b.ue()?;
        if chroma == 3 {
            separate_planes = b.bit()? == 1;
        }
        b.ue()?;
        b.ue()?;
        b.bit()?;
        if b.bit()? == 1 {
            for i in 0..if chroma == 3 { 12 } else { 8 } {
                if b.bit()? == 1 {
                    // A scaling list: deltas until the next scale is 0.
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
    let frame_num_bits = b.ue()? + 4;
    let poc_type = b.ue()?;
    let mut poc_lsb_bits = 0;
    match poc_type {
        0 => poc_lsb_bits = b.ue()? + 4,
        1 => {
            b.bit()?;
            b.se()?;
            b.se()?;
            for _ in 0..b.ue()?.min(255) {
                b.se()?;
            }
        }
        _ => {}
    }
    b.ue()?;
    b.bit()?;
    b.ue()?;
    b.ue()?;
    let frame_mbs_only = b.bit()? == 1;
    Some(PocSps {
        frame_num_bits,
        poc_type,
        poc_lsb_bits,
        frame_mbs_only,
        separate_planes,
    })
}

/// Presentation ranks of H.264 access units (Annex B) in decode order, from their picture
/// order counts (type 0: the lsb and its wrap; type 2: decode order). `None` when the
/// stream doesn't say (type 1, unreadable headers).
pub(crate) fn h264_presentation(units: &[&[u8]]) -> Option<Vec<i64>> {
    let mut sps = None;
    let mut keyed = Vec::with_capacity(units.len());
    let (mut period, mut msb, mut prev_lsb) = (0i64, 0i64, 0i64);
    for (i, au) in units.iter().enumerate() {
        let mut poc = None;
        for n in crate::ts::nal_units(au) {
            let Some(&h) = n.first() else { continue };
            match h & 0x1F {
                7 => sps = poc_sps(n).or(sps),
                1 | 5 => {
                    let s: PocSps = sps?;
                    if s.poc_type == 2 {
                        poc = Some(i as i64);
                        break;
                    }
                    if s.poc_type != 0 {
                        return None;
                    }
                    let rbsp = crate::ts::unescape(&n[1..n.len().min(64)]);
                    let mut b = ExpGolomb { b: &rbsp, p: 0 };
                    b.ue()?;
                    b.ue()?;
                    b.ue()?;
                    if s.separate_planes {
                        b.bits(2)?;
                    }
                    b.bits(s.frame_num_bits)?;
                    if !s.frame_mbs_only && b.bit()? == 1 {
                        b.bit()?;
                    }
                    let idr = h & 0x1F == 5;
                    if idr {
                        b.ue()?;
                        period += 1;
                        msb = 0;
                        prev_lsb = 0;
                    }
                    let lsb = i64::from(b.bits(s.poc_lsb_bits)?);
                    let max = 1i64 << s.poc_lsb_bits;
                    let mut m = msb;
                    if lsb < prev_lsb && prev_lsb - lsb >= max / 2 {
                        m += max;
                    } else if lsb > prev_lsb && lsb - prev_lsb > max / 2 {
                        m -= max;
                    }
                    if h >> 5 != 0 {
                        // A reference picture: the next one's wrap is judged against it.
                        msb = m;
                        prev_lsb = lsb;
                    }
                    poc = Some(m + lsb);
                    break;
                }
                _ => {}
            }
        }
        keyed.push((period, poc?, i));
    }
    let mut order: Vec<usize> = (0..keyed.len()).collect();
    order.sort_by_key(|&i| (keyed[i].0, keyed[i].1));
    let mut rank = vec![0i64; keyed.len()];
    for (r, &i) in order.iter().enumerate() {
        rank[i] = r as i64;
    }
    Some(rank)
}

/// Whether an MPEG-2 sample starts a GOP or codes an I picture.
fn mpeg2_key(s: &[u8]) -> bool {
    let head = &s[..s.len().min(4096)];
    let mut i = 0;
    while i + 6 <= head.len() {
        if head[i] == 0 && head[i + 1] == 0 && head[i + 2] == 1 {
            match head[i + 3] {
                0xB8 => return true,
                0x00 => return (head[i + 5] >> 3) & 7 == 1,
                _ => {}
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    false
}

struct ExpGolomb<'a> {
    b: &'a [u8],
    p: usize,
}

impl ExpGolomb<'_> {
    fn bits(&mut self, n: u32) -> Option<u32> {
        let mut v = 0;
        for _ in 0..n {
            v = v << 1 | self.bit()?;
        }
        Some(v)
    }

    fn se(&mut self) -> Option<i32> {
        let k = self.ue()?;
        Some(if k & 1 == 1 {
            (k / 2 + 1) as i32
        } else {
            -((k / 2) as i32)
        })
    }

    fn bit(&mut self) -> Option<u32> {
        let byte = *self.b.get(self.p / 8)?;
        let v = (byte >> (7 - self.p % 8)) & 1;
        self.p += 1;
        Some(u32::from(v))
    }

    fn ue(&mut self) -> Option<u32> {
        let mut zeros = 0;
        while self.bit()? == 0 {
            zeros += 1;
            if zeros > 31 {
                return None;
            }
        }
        let mut v = 1u32;
        for _ in 0..zeros {
            v = v << 1 | self.bit()?;
        }
        Some(v - 1)
    }
}

/// H.273 code points from the descriptor's colour labels (SMPTE RP 224 registers).
fn colour(d: &Descriptor) -> Option<Colour> {
    let last = |ul: Option<[u8; 16]>, group: u8| {
        ul.filter(|u| u[..4] == UL && u[8..12] == [0x04, 0x01, 0x01, 0x01] && u[12] == group)
            .map(|u| u[13])
    };
    let transfer = last(d.transfer, 0x01).map(|t| match t {
        0x01 => 5,
        0x02 | 0x04 => 1,
        0x03 => 7,
        0x05 => 12,
        0x06 => 8,
        0x07 => 17,
        0x08 => 11,
        0x09 => 14,
        0x0A => 16,
        0x0B => 18,
        _ => 2,
    });
    let primaries = last(d.primaries, 0x03).map(|p| match p {
        0x01 => 6,
        0x02 => 5,
        0x03 => 1,
        0x04 => 9,
        0x05 => 10,
        0x06 => 11,
        _ => 2,
    });
    let matrix = last(d.equations, 0x02).map(|m| match m {
        0x01 => 6,
        0x02 => 1,
        0x03 => 7,
        0x06 => 9,
        _ => 2,
    });
    if transfer.is_none() && primaries.is_none() && matrix.is_none() {
        return None;
    }
    Some(Colour {
        primaries: primaries.unwrap_or(2),
        transfer: transfer.unwrap_or(2),
        matrix: matrix.unwrap_or(2),
        full_range: false,
    })
}

/// The edit units of a clip-wrapped element at `value` (`len` bytes) from the index table:
/// (offset, size, temporal offset, flags) each.
fn clip_units(
    d: &[u8],
    value: u64,
    len: u64,
    index: &[IndexSegment],
    codec: Codec,
) -> Vec<(u64, u64, i8, u8)> {
    let mut segs: Vec<&IndexSegment> = index.iter().collect();
    segs.sort_by_key(|s| s.start);
    // Constant edit unit size.
    if let Some(s) = segs.iter().find(|s| s.edit_unit_bytes > 0) {
        let n = u64::from(s.edit_unit_bytes);
        return (0..len / n)
            .take(MAX_SAMPLES)
            .map(|i| (value + i * n, n, 0, 0x80))
            .collect();
    }
    let mut entries: Vec<(i8, u8, u64)> = Vec::new();
    for s in segs {
        if s.start as usize != entries.len() {
            // A gap or an overlap: keep what lines up.
            if (s.start as usize) < entries.len() {
                entries.truncate(s.start as usize);
            } else {
                break;
            }
        }
        entries.extend_from_slice(&s.entries);
    }
    if entries.is_empty() {
        return Vec::new();
    }
    // Offsets count from the element's value, or (some writers) from its key: whichever puts
    // a picture at the first one.
    let looks_right = |base: u64| {
        let at = (base + entries[0].2) as usize;
        let b = d.get(at..at + 8).unwrap_or_default();
        match codec {
            Codec::ProRes => b.get(4..8) == Some(b"icpf"),
            Codec::Mpeg2 | Codec::Mpeg1 => b.starts_with(&[0, 0, 1]),
            Codec::H264 => b.starts_with(&[0, 0, 1]) || b.starts_with(&[0, 0, 0, 1]),
            _ => true,
        }
    };
    let base = if looks_right(value) {
        value
    } else {
        // Back over the length and the key.
        let key_at = (value.saturating_sub(25)..value.saturating_sub(16))
            .rev()
            .find(|&k| d.get(k as usize..k as usize + 4) == Some(&UL))
            .unwrap_or(value);
        if looks_right(key_at) { key_at } else { value }
    };
    let skip = value - base;
    let mut out = Vec::with_capacity(entries.len());
    for (i, &(t, flags, off)) in entries.iter().enumerate() {
        let next = entries.get(i + 1).map_or(len + skip, |e| e.2);
        if off < skip || next < off || next - skip > len {
            break;
        }
        out.push((base + off, next - off, t, flags));
    }
    out
}

/// Parses an MXF file. With `samples`, the sample tables too.
pub fn parse(data: &[u8], samples: bool) -> Result<Movie> {
    let start = find_header(data).ok_or_else(|| bad("no header partition"))?;
    let w = walk(data, start);
    if w.encrypted && w.streams.is_empty() {
        return Err(Error::Unsupported("encrypted MXF".into()));
    }
    let meta = metadata(&w.sets);
    let is_picture = |s: &&Stream| matches!(s.key[12], 0x15 | 0x05);
    let is_sound = |s: &&Stream| matches!(s.key[12], 0x16 | 0x06);
    let mut movie = Movie {
        brand: *b"mxf ",
        make: meta.company.clone().filter(|s| !s.is_empty()),
        model: meta.product.clone().filter(|s| !s.is_empty()),
        created: meta.modified,
        ..Movie::default()
    };
    if let Some(s) = w.streams.iter().find(is_picture) {
        movie.video = Some(video_track(data, s, &meta, &w.index, samples)?);
    } else if w.encrypted {
        return Err(Error::Unsupported("encrypted MXF".into()));
    }
    let fps = movie
        .video
        .as_ref()
        .map(|v| (v.timescale, v.samples.first().map_or(1, |s| s.duration)));
    // Sound: one track per channel is usual (a camera's channels 1 to 4 or 8); the first
    // two mono tracks play as left and right.
    let sounds: Vec<AudioTrack> = w
        .streams
        .iter()
        .filter(is_sound)
        .filter_map(|s| sound_track(data, s, &meta, fps, samples))
        .collect();
    let count = sounds.len();
    let mut sounds = sounds.into_iter();
    if let Some(mut first) = sounds.next() {
        if let Some(second) = sounds.next()
            && pairs(&first, &second)
        {
            first.extra.companions.push(second);
        }
        first.extra.tracks = count;
        movie.audio = Some(first);
        movie.has_audio = true;
    }
    movie.timescale = 1_000_000;
    movie.duration_us = movie
        .video
        .as_ref()
        .and_then(|v| v.duration_us())
        .or_else(|| {
            let a = movie.audio.as_ref()?;
            (a.timescale > 0).then(|| a.duration * 1_000_000 / u64::from(a.timescale))
        });
    Ok(movie)
}

fn video_track(
    data: &[u8],
    s: &Stream,
    meta: &Metadata,
    index: &[IndexSegment],
    samples: bool,
) -> Result<VideoTrack> {
    let d = meta
        .descriptor_for(s.number, DescKind::Picture)
        .cloned()
        .unwrap_or_default();
    let first_el = s.elements.first().copied().unwrap_or((0, 0));
    let first_bytes = data
        .get(first_el.0 as usize..(first_el.0 + first_el.1.min(1 << 20)) as usize)
        .unwrap_or_default();
    let (mut codec, mut config) = picture_codec(&d, first_bytes);
    let rate = d
        .sample_rate
        .or_else(|| meta.edit_rate(s.number))
        .or_else(|| index.iter().find_map(|i| i.edit_rate))
        .unwrap_or((25, 1));
    // Clip wrapping: one element with every edit unit; frame wrapping: an element each.
    let cut = if s.elements.len() == 1 {
        clip_units(data, first_el.0, first_el.1, index, codec)
    } else {
        Vec::new()
    };
    let clip = cut.len() > 1;
    let mut units: Vec<(u64, u64, i8, u8)> = if clip {
        cut
    } else {
        s.elements.iter().map(|&(o, l)| (o, l, 0, 0)).collect()
    };
    if codec == Codec::Other(*b"mxfv")
        && let Some(&(o, l, ..)) = units.first()
        && let Some(b) = data.get(o as usize..(o + l.min(1 << 20)) as usize)
    {
        // Unlabelled: the first edit unit's bytes, now that clip-wrapped essence is cut.
        codec = picture_codec(&Descriptor::default(), b).0;
    }
    // Index flags and temporal offsets for frame-wrapped essence (one entry per element).
    let mut entries: Vec<(i8, u8)> = Vec::new();
    if !clip {
        let mut segs: Vec<&IndexSegment> = index.iter().filter(|i| !i.entries.is_empty()).collect();
        segs.sort_by_key(|s| s.start);
        for seg in segs {
            if seg.start as usize == entries.len() {
                entries.extend(seg.entries.iter().map(|e| (e.0, e.1)));
            }
        }
        if entries.len() == units.len() {
            for (u, e) in units.iter_mut().zip(&entries) {
                u.2 = e.0;
                u.3 = e.1;
            }
        } else {
            entries.clear();
        }
    }
    let intra =
        matches!(codec, Codec::ProRes | Codec::Raw) || d.container.is_some_and(|c| c[13] == 0x01);
    let have_index = clip || !entries.is_empty();
    let mut list = Vec::with_capacity(units.len());
    let n = units.len();
    // Presentation position of each stored edit unit: entry x is shown at x, stored at
    // x + its temporal offset.
    let mut shown: Vec<i64> = (0..n as i64).collect();
    let reordered = units.iter().any(|u| u.2 != 0);
    if reordered {
        let mut ok = true;
        let mut seen = vec![false; n];
        for (x, u) in units.iter().enumerate() {
            let s = x as i64 + i64::from(u.2);
            if s < 0 || s as usize >= n || seen[s as usize] {
                ok = false;
                break;
            }
            seen[s as usize] = true;
            shown[s as usize] = x as i64;
        }
        if !ok {
            shown = (0..n as i64).collect();
        }
    }
    let long_gop = matches!(codec, Codec::Mpeg2 | Codec::H264 | Codec::Mpeg1) && !intra;
    if codec == Codec::H264 && !reordered {
        // H.264 orders itself by picture order count (FFmpeg writes no temporal offsets).
        let aus: Vec<&[u8]> = units
            .iter()
            .map(|&(o, l, ..)| data.get(o as usize..(o + l) as usize).unwrap_or_default())
            .collect();
        if let Some(rank) = h264_presentation(&aus) {
            shown = rank;
        }
    }
    // No temporal offsets (FFmpeg writes none for H.264): the decoder's output order fills
    // the slots.
    let slot_timing = long_gop && !reordered && codec != Codec::H264;
    let (ts, dur) = rate;
    for (i, &(off, size, _, flags)) in units.iter().enumerate() {
        let bytes = || {
            data.get(off as usize..(off + size) as usize)
                .unwrap_or_default()
        };
        let sync = match codec {
            Codec::Mpeg2 | Codec::Mpeg1 if !intra => {
                if have_index {
                    flags & 0x80 != 0
                } else {
                    mpeg2_key(bytes())
                }
            }
            Codec::H264 => {
                if have_index && flags != 0 {
                    flags & 0x80 != 0
                } else {
                    h264_key(bytes())
                }
            }
            _ => true,
        };
        list.push(Sample {
            offset: off,
            size: size.min(u64::from(u32::MAX)) as u32,
            dts: i as u64 * u64::from(dur),
            cts: shown[i] * i64::from(dur),
            duration: dur,
            sync,
        });
    }
    if !list.is_empty() && !list.iter().any(|s| s.sync) {
        list[0].sync = true;
    }
    let layout = if codec == Codec::H264 {
        let firsts: Vec<&[u8]> = list
            .iter()
            .take(4)
            .filter_map(|s| data.get(s.offset as usize..(s.offset + u64::from(s.size)) as usize))
            .collect();
        config = avcc(&firsts);
        Layout::AnnexB
    } else {
        Layout::Contiguous
    };
    let mut height = d.stored.1;
    if d.frame_layout == Some(1) {
        // Separate fields: the stored height is a field's.
        height *= 2;
    }
    let mut v = VideoTrack {
        codec: Some(codec),
        config,
        width: d.stored.0,
        height,
        display: (0, 0),
        rotation: 0,
        mirror: false,
        timescale: ts,
        duration: n as u64 * u64::from(dur),
        sample_count: n as u32,
        samples: Vec::new(),
        colour: colour(&d),
        pixel_aspect: None,
        interlaced: matches!(d.frame_layout, Some(1 | 3)),
        bottom_first: false,
        slot_timing,
        edit: Edit::default(),
        layout,
    };
    if codec == Codec::ProRes
        && let Some(h) = list
            .first()
            .and_then(|s| data.get(s.offset as usize..(s.offset + u64::from(s.size)) as usize))
            .and_then(|b| specular_prores::header(b).ok())
    {
        (v.width, v.height) = (h.width, h.height);
    }
    if codec == Codec::H264
        && let Some(sps) = list.first().and_then(|s| {
            let b = data.get(s.offset as usize..(s.offset + u64::from(s.size)) as usize)?;
            crate::ts::nal_units(b).find(|n| n.first().is_some_and(|h| h & 0x1F == 7))
        })
        && let Some((w, h)) = crate::ts::h264_size(sps)
    {
        (v.width, v.height) = (w, h);
    }
    let shown_w = if d.display.0 > 0 {
        d.display.0
    } else {
        v.width
    };
    let shown_h = if d.display.1 > 0 {
        d.display.1 * if d.frame_layout == Some(1) { 2 } else { 1 }
    } else {
        v.height
    };
    if let Some((an, ad)) = d.aspect
        && shown_w > 0
        && shown_h > 0
    {
        // Display aspect → pixel aspect: (an/ad) / (w/h).
        let h = u64::from(an) * u64::from(shown_h);
        let vv = u64::from(ad) * u64::from(shown_w);
        let g = gcd(h, vv).max(1);
        let (h, vv) = (h / g, vv / g);
        if h != vv && h <= u64::from(u32::MAX) && vv <= u64::from(u32::MAX) {
            v.pixel_aspect = Some((h as u32, vv as u32));
        }
    }
    v.display = v.shown_size();
    if samples {
        v.samples = list;
    }
    Ok(v)
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Whether two sound tracks play as a stereo pair: mono PCM side by side, the same rate,
/// sample size and number of pieces.
fn pairs(a: &AudioTrack, b: &AudioTrack) -> bool {
    a.channels == 1
        && b.channels == 1
        && matches!(a.codec, AudioCodec::Pcm { .. })
        && a.codec == b.codec
        && a.layout == Layout::Contiguous
        && b.layout == Layout::Contiguous
        && a.sample_rate == b.sample_rate
        && a.samples.len() == b.samples.len()
        && a.samples
            .iter()
            .zip(&b.samples)
            .all(|(x, y)| x.duration == y.duration)
}

/// The material package of an MXF file (every OP-Atom file of a clip shares it).
pub fn material_package(data: &[u8]) -> Option<[u8; 32]> {
    let start = find_header(data)?;
    metadata(&walk(data, start).sets).material
}

/// Most sibling files looked at for an OP-Atom clip's sound.
const MAX_SIBLINGS: usize = 256;

/// The sound of an OP-Atom picture file from the sound files of the same clip beside it
/// (Avid: the same folder; Panasonic P2: `../AUDIO`), found by their shared material
/// package: the first file's track, with the second's as its right channel when both
/// are mono. `None` when there are none.
pub fn sibling_sound(source: &std::sync::Arc<specular_core::Source>) -> Option<AudioTrack> {
    use crate::mp4::External;
    let me = material_package(source.bytes())?;
    let path = source.path();
    let dir = path.parent()?;
    let mut dirs = vec![dir.to_path_buf()];
    if dir
        .file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case("VIDEO"))
        && let Some(up) = dir.parent()
    {
        for name in ["AUDIO", "audio", "Audio"] {
            let d = up.join(name);
            if d.is_dir() {
                dirs.push(d);
                break;
            }
        }
    }
    let mut found: Vec<(std::path::PathBuf, AudioTrack)> = Vec::new();
    let mut looked = 0;
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        let mut names: Vec<std::path::PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("mxf")) && p != path)
            .collect();
        names.sort();
        for p in names {
            looked += 1;
            if looked > MAX_SIBLINGS {
                break;
            }
            let Ok(src) = specular_core::Source::open(&p) else {
                continue;
            };
            if material_package(src.bytes()) != Some(me) {
                continue;
            }
            let Ok(m) = parse(src.bytes(), true) else {
                continue;
            };
            if let Some(mut a) = m.audio {
                if a.samples.is_empty() {
                    continue;
                }
                let src = std::sync::Arc::new(src);
                a.extra.source = Some(External(std::sync::Arc::clone(&src)));
                for c in a.extra.companions.iter_mut() {
                    c.extra.source = Some(External(std::sync::Arc::clone(&src)));
                }
                found.push((p, a));
            }
        }
    }
    let count = found.len();
    let mut it = found.into_iter().map(|f| f.1);
    let mut first = it.next()?;
    if first.extra.companions.is_empty()
        && let Some(second) = it.next()
        && pairs(&first, &second)
    {
        first.extra.companions.push(second);
    }
    first.extra.tracks = count.max(first.extra.tracks);
    Some(first)
}

fn sound_track(
    data: &[u8],
    s: &Stream,
    meta: &Metadata,
    fps: Option<(u32, u32)>,
    samples: bool,
) -> Option<AudioTrack> {
    let d = meta
        .descriptor_for(s.number, DescKind::Sound)
        .cloned()
        .unwrap_or_default();
    let rate = d
        .audio_rate
        .map(|(n, dd)| n / dd.max(1))
        .filter(|&r| r > 0)
        .unwrap_or(48_000);
    let d10 = s.key[12] == 0x06 && s.key[14] == 0x10;
    let channels = d.channels.clamp(1, 16) as u16;
    let bits = if d.bits == 0 { 24 } else { d.bits } as u16;
    let compressed = d
        .sound_coding
        .is_some_and(|c| c[..4] == UL && c[8..12] == [0x04, 0x02, 0x02, 0x02]);
    let (codec, layout, frame_bytes) = if compressed {
        (AudioCodec::Other(*b"mxfa", 0), Layout::Contiguous, 1)
    } else if d10 {
        (
            AudioCodec::Pcm {
                bits: 32,
                float: false,
                little_endian: true,
            },
            Layout::D10Sound(channels.min(8) as u8),
            32,
        )
    } else {
        let block = if d.block_align > 0 {
            d.block_align
        } else {
            u32::from(channels) * u32::from(bits.div_ceil(8))
        };
        (
            AudioCodec::Pcm {
                bits,
                float: false,
                little_endian: true,
            },
            Layout::Contiguous,
            block.max(1),
        )
    };
    // Clip-wrapped sound is one element: cut it into pieces of about a frame.
    let mut pieces: Vec<(u64, u64)> = Vec::new();
    if s.elements.len() == 1 && !d10 {
        let (o, l) = s.elements[0];
        let per = fps.map_or(u64::from(rate) / 25, |(ts, dur)| {
            u64::from(rate) * u64::from(dur) / u64::from(ts.max(1))
        });
        let chunk = (per.max(1) * u64::from(frame_bytes)).max(1);
        let mut at = 0;
        while at < l && pieces.len() < MAX_SAMPLES {
            let n = chunk.min(l - at);
            pieces.push((o + at, n));
            at += n;
        }
    } else {
        pieces = s.elements.clone();
    }
    let mut list = Vec::with_capacity(pieces.len());
    let mut t = 0u64;
    for (o, l) in pieces {
        let (frames, off, size) = if d10 {
            // A 4-byte element header, then eight 4-byte channel slots per sample.
            let n = l.saturating_sub(4) / 32;
            (n, o, l)
        } else {
            (l / u64::from(frame_bytes), o, l)
        };
        if frames == 0 {
            continue;
        }
        list.push(Sample {
            offset: off,
            size: size.min(u64::from(u32::MAX)) as u32,
            dts: t,
            cts: t as i64,
            duration: frames.min(u64::from(u32::MAX)) as u32,
            sync: true,
        });
        t += frames;
    }
    let _ = data;
    Some(AudioTrack {
        codec,
        channels: if d10 { channels.min(8) } else { channels },
        sample_rate: rate,
        timescale: rate,
        duration: t,
        sample_count: list.len() as u32,
        samples: if samples { list } else { Vec::new() },
        edit: Edit::default(),
        layout,
        extra: Default::default(),
    })
}

/// D-10 sound: each element is a 4-byte header and eight 32-bit AES3 slots per sample
/// (bits 4–27 the sample); the first `channels` slots as 32-bit little-endian PCM.
pub fn d10_sound(data: &[u8], s: &Sample, channels: u8) -> Option<Vec<u8>> {
    let b = crate::mp4::sample_bytes(data, s)?;
    let body = b.get(4..)?;
    let ch = usize::from(channels.clamp(1, 8));
    let mut out = Vec::with_capacity(body.len() / 32 * ch * 4);
    for slot in body.chunks_exact(32) {
        for c in slot.chunks_exact(4).take(ch) {
            let w = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
            out.extend_from_slice(&((w << 4) & 0xFFFF_FF00).to_le_bytes());
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ber(n: usize) -> Vec<u8> {
        let b = (n as u32).to_be_bytes();
        vec![0x83, b[1], b[2], b[3]]
    }

    fn kl(key: &[u8; 16], value: &[u8]) -> Vec<u8> {
        let mut v = key.to_vec();
        v.extend(ber(value.len()));
        v.extend_from_slice(value);
        v
    }

    fn set(kind: u8, items: &[(u16, Vec<u8>)]) -> Vec<u8> {
        let mut key = [0u8; 16];
        key[..13].copy_from_slice(&[
            0x06, 0x0E, 0x2B, 0x34, 0x02, 0x53, 0x01, 0x01, 0x0D, 0x01, 0x01, 0x01, 0x01,
        ]);
        key[13] = 0x01;
        key[14] = kind;
        let mut v = Vec::new();
        for (t, b) in items {
            v.extend(t.to_be_bytes());
            v.extend((b.len() as u16).to_be_bytes());
            v.extend(b);
        }
        kl(&key, &v)
    }

    fn partition(kind: u8) -> Vec<u8> {
        let mut key = [0u8; 16];
        key[..13].copy_from_slice(&PARTITION);
        key[13] = kind;
        key[14] = 0x04;
        kl(&key, &[0; 88])
    }

    fn element(item: u8, number: u8, body: &[u8]) -> Vec<u8> {
        let key = [
            0x06, 0x0E, 0x2B, 0x34, 0x01, 0x02, 0x01, 0x01, 0x0D, 0x01, 0x03, 0x01, item, 0x01,
            0x17, number,
        ];
        kl(&key, body)
    }

    /// A tiny OP1a-like file: a ProRes-labelled picture descriptor, a PCM sound
    /// descriptor and `frames` content packages.
    fn file(frames: usize) -> Vec<u8> {
        let mut f = vec![0u8; 40]; // run-in
        f.extend(partition(0x02));
        let mut coding = vec![0x06, 0x0E, 0x2B, 0x34, 0x04, 0x01, 0x01, 0x0D];
        coding.extend([0x04, 0x01, 0x02, 0x02, 0x03, 0x06, 0x04, 0x00]);
        f.extend(set(
            0x28,
            &[
                (0x3C0A, vec![1; 16]),
                (0x3001, [25u32.to_be_bytes(), 1u32.to_be_bytes()].concat()),
                (0x3201, coding),
                (0x3203, 64u32.to_be_bytes().to_vec()),
                (0x3202, 32u32.to_be_bytes().to_vec()),
                (0x320E, [16u32.to_be_bytes(), 9u32.to_be_bytes()].concat()),
            ],
        ));
        f.extend(set(
            0x48,
            &[
                (0x3C0A, vec![2; 16]),
                (
                    0x3D03,
                    [48_000u32.to_be_bytes(), 1u32.to_be_bytes()].concat(),
                ),
                (0x3D07, 2u32.to_be_bytes().to_vec()),
                (0x3D01, 16u32.to_be_bytes().to_vec()),
                (0x3D0A, 4u16.to_be_bytes().to_vec()),
            ],
        ));
        for i in 0..frames {
            let mut pic = vec![0, 0, 0, 40];
            pic.extend(b"icpf");
            pic.extend([i as u8; 32]);
            f.extend(element(0x15, 1, &pic));
            f.extend(element(0x16, 2, &vec![0u8; 1920 * 4]));
        }
        f.extend(partition(0x04));
        f
    }

    #[test]
    fn frame_wrapped_picture_and_sound() {
        let f = file(5);
        assert!(sniff(&f));
        assert!(!sniff(&f[60..]));
        let m = parse(&f, true).unwrap();
        let v = m.video.as_ref().unwrap();
        assert_eq!(v.codec, Some(Codec::ProRes));
        assert_eq!(v.config, b"apch");
        assert_eq!((v.timescale, v.samples.len()), (25, 5));
        assert_eq!(v.samples[3].cts, 3);
        assert!(v.samples.iter().all(|s| s.sync));
        let b = v.sample_data(&f, &v.samples[2]).unwrap();
        assert_eq!(&b[4..8], b"icpf");
        assert_eq!(b[8], 2);
        // 16:9 over 64×32 (2:1): pixels 8:9.
        assert_eq!(v.pixel_aspect, Some((8, 9)));
        let a = m.audio.as_ref().unwrap();
        assert_eq!((a.sample_rate, a.channels), (48_000, 2));
        assert_eq!(a.samples.len(), 5);
        assert_eq!(a.samples[1].dts, 1920);
        assert_eq!(m.duration_us, Some(200_000));
    }

    /// Uncompressed picture essence (SMPTE 384M) of 64×32 at `depth` bits, 4:2:2, each frame
    /// `frame` bytes of value `i`.
    fn raw_file(depth: u32, frame: usize) -> Vec<u8> {
        let mut f = vec![0u8; 40];
        f.extend(partition(0x02));
        let mut coding = vec![0x06, 0x0E, 0x2B, 0x34, 0x04, 0x01, 0x01, 0x0A];
        coding.extend([0x04, 0x01, 0x02, 0x01, 0x01, 0x02, 0x01, 0x00]);
        f.extend(set(
            0x28,
            &[
                (0x3C0A, vec![1; 16]),
                (0x3001, [25u32.to_be_bytes(), 1u32.to_be_bytes()].concat()),
                (0x3201, coding),
                (0x3203, 64u32.to_be_bytes().to_vec()),
                (0x3202, 32u32.to_be_bytes().to_vec()),
                (0x3301, depth.to_be_bytes().to_vec()),
                (0x3302, 2u32.to_be_bytes().to_vec()),
                (0x3308, 1u32.to_be_bytes().to_vec()),
            ],
        ));
        for i in 0..3 {
            f.extend(element(0x15, 1, &vec![i as u8 + 16; frame]));
        }
        f.extend(partition(0x04));
        f
    }

    #[test]
    fn uncompressed_picture_essence() {
        use crate::rawvid::PixDesc;
        // 8-bit 4:2:2: UYVY.
        let f = raw_file(8, 64 * 32 * 2);
        let m = parse(&f, true).unwrap();
        let v = m.video.as_ref().unwrap();
        assert_eq!(v.codec, Some(Codec::Raw));
        let d = PixDesc::from_config(&v.config).unwrap();
        assert_eq!(&d.fourcc, b"UYVY");
        assert!(v.samples.iter().all(|s| s.sync));
        let b = v.sample_data(&f, &v.samples[1]).unwrap();
        let u = crate::rawvid::unpack(&d, 64, 32, &b).unwrap();
        assert_eq!(u.frame.planes[0][..4], [17; 4]);
        assert_eq!(u.frame.planes[1].len(), 32 * 32);
        // 10-bit at v210's size: v210; another packing is refused by name.
        let f = raw_file(10, 128 * 2 * 32);
        let v = parse(&f, true).unwrap().video.unwrap();
        assert_eq!(&PixDesc::from_config(&v.config).unwrap().fourcc, b"v210");
        let f = raw_file(10, 64 * 5 / 2 * 32);
        let v = parse(&f, true).unwrap().video.unwrap();
        let d = PixDesc::from_config(&v.config).unwrap();
        assert_eq!(
            crate::rawvid::refusal(&d).as_deref(),
            Some("10-bit uncompressed MXF video in this packing")
        );
    }

    #[test]
    fn mpeg2_keys_and_h264_keys() {
        assert!(mpeg2_key(&[0, 0, 1, 0xB3, 0, 0, 1, 0xB8, 0, 0]));
        // Picture header: temporal reference 0, type 2 (P).
        assert!(!mpeg2_key(&[0, 0, 1, 0, 0, 0x10, 0, 0]));
        assert!(mpeg2_key(&[0, 0, 1, 0, 0, 0x08, 0, 0]));
        assert!(h264_key(&[0, 0, 0, 1, 0x65, 0x88]));
        // Non-IDR slice: first_mb 0 (1), slice_type 7 (0001000) → I.
        assert!(h264_key(&[0, 0, 1, 0x41, 0b1000_1000, 0]));
        assert!(!h264_key(&[0, 0, 1, 0x41, 0b1010_0000]));
    }

    #[test]
    fn d10_slots_to_pcm() {
        let mut el = vec![0u8; 4];
        // One sample: channel 0 = 0x123456 in bits 4..27, flags above, channel id below.
        let w: u32 = (0x123456 << 4) | 0xF000_0000 | 0x3;
        el.extend(w.to_le_bytes());
        el.extend([0u8; 28]);
        let s = Sample {
            offset: 0,
            size: el.len() as u32,
            dts: 0,
            cts: 0,
            duration: 1,
            sync: true,
        };
        let out = d10_sound(&el, &s, 2).unwrap();
        assert_eq!(out.len(), 8);
        assert_eq!(
            u32::from_le_bytes(out[..4].try_into().unwrap()),
            0x1234_5600
        );
    }
}
