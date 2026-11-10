//! MPEG programme streams (MPEG-1 system streams and MPEG-2 programme streams: `.mpg`,
//! `.mpeg`, `.vob`, `.mod`, `.tod`, `.vro`, `.m2p`) and bare MPEG-1/2 video elementary streams
//! (`.m1v`, `.m2v`, `.mpv`) into the same [`Movie`] the MP4 demuxer gives.
//!
//! - Pack headers of either version, system headers, padding and DVD navigation packs are
//!   walked by their lengths; the first video stream (0xE0–0xEF) and one sound stream are
//!   taken: AC-3 in private stream 1 first (a DVD's first audio stream), else MPEG audio
//!   (0xC0–0xDF), DVD LPCM or DTS.
//! - Video samples are pictures, cut at start codes across PES packets (a field pair is one
//!   sample): a PES packet's PTS belongs to the first picture that starts in it, and pictures
//!   without one are timed from their GOP's temporal references.
//! - Sound samples are PES packets.
//!
//! A sample is located by the file offset of the PES packet it starts in, with the offset into
//! that packet's payload in the top 16 bits of [`Sample::offset`] (files are far below 2^48
//! bytes); [`PsLayout::read`] gathers its bytes from that packet on.

use crate::mp4::{
    AudioCodec, AudioTrack, Codec, Edit, Error, Layout, MAX_SAMPLES, Movie, Result, Sample,
    VideoTrack,
};

const PACK: u8 = 0xBA;
const END: u8 = 0xB9;
const PRIVATE1: u8 = 0xBD;
/// Bytes of a file read at both ends to describe it without its sample table.
const EDGE: usize = 4 << 20;
const SKIP_SHIFT: u32 = 48;
/// The pieces an elementary stream is cut into (offsets in a piece fit 16 bits).
const ES_PIECE: usize = 1 << 16;

fn bad(s: impl Into<String>) -> Error {
    Error::Malformed(s.into())
}

/// Where a programme stream track's samples are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PsLayout {
    pub stream: u8,
    /// Private stream 1 substream (0 for none).
    pub sub: u8,
}

/// Whether `head` starts a programme stream (a pack header) or a bare MPEG-1/2 video
/// stream (a sequence header).
pub fn sniff(head: &[u8]) -> bool {
    head.starts_with(&[0, 0, 1, PACK]) && head.len() > 4 && matches!(head[4] >> 4, 2 | 4..=7)
        || sniff_es(head)
}

/// A bare MPEG-1/2 video stream: a sequence header with a plausible size first. Zero bytes
/// may come before it (stuffing before a start code, ISO/IEC 13818-2 5.3; Sony's
/// conformance stream sony-ct3.bs has ten).
fn sniff_es(head: &[u8]) -> bool {
    let zeros = head.iter().take(256).take_while(|&&b| b == 0).count();
    let Some(head) = zeros.checked_sub(2).and_then(|z| head.get(z..)) else {
        return false;
    };
    head.starts_with(&[0, 0, 1, 0xB3])
        && head.len() >= 8
        && (u16::from(head[4]) << 4 | u16::from(head[5] >> 4)) > 0
        && (u16::from(head[5] & 15) << 8 | u16::from(head[6])) > 0
}

/// One packet of the stream: its start code and where it ends.
struct Packet {
    code: u8,
    at: usize,
    end: usize,
}

/// The packet at `at` (which holds a start code), or the next start code after damage.
fn packet(data: &[u8], at: usize) -> Option<Packet> {
    let b = data.get(at..at + 6)?;
    if b[..3] != [0, 0, 1] {
        return None;
    }
    let code = b[3];
    let end = match code {
        PACK => {
            let m = *data.get(at + 4)?;
            if m >> 6 == 1 {
                at + 14 + usize::from(*data.get(at + 13)? & 7)
            } else {
                at + 12
            }
        }
        END => at + 4,
        0xBB..=0xFF => at + 6 + usize::from(u16::from_be_bytes([b[4], b[5]])),
        _ => return None,
    };
    Some(Packet { code, at, end })
}

/// The next start code at or after `from` that begins a packet.
fn resync(data: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while i + 4 <= data.len() {
        if data[i + 2] > 1 {
            i += 3;
            continue;
        }
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 && data[i + 3] >= 0xB9 {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// A PES packet's payload range (file offsets), its PTS and DTS, and for private stream 1
/// its substream (whose own header is skipped).
struct Pes {
    stream: u8,
    sub: u8,
    payload: std::ops::Range<usize>,
    pts: Option<i64>,
    dts: Option<i64>,
}

fn ts33(t: &[u8]) -> i64 {
    (i64::from(t[0] >> 1 & 7) << 30)
        | (i64::from(t[1]) << 22)
        | (i64::from(t[2] >> 1) << 15)
        | (i64::from(t[3]) << 7)
        | i64::from(t[4] >> 1)
}

fn pes(data: &[u8], p: &Packet) -> Option<Pes> {
    let stream = p.code;
    if !(stream == PRIVATE1 || (0xC0..=0xEF).contains(&stream)) {
        return None;
    }
    let end = p.end.min(data.len());
    let b = data.get(p.at..end)?;
    let (mut h, mut pts, mut dts) = (6usize, None, None);
    if b.get(6).is_some_and(|&x| x >> 6 == 2) {
        // MPEG-2 PES header.
        let flags = *b.get(7)? >> 6;
        let hl = usize::from(*b.get(8)?);
        if flags & 2 != 0 {
            pts = b.get(9..14).map(ts33);
        }
        if flags == 3 {
            dts = b.get(14..19).map(ts33);
        }
        h = 9 + hl;
    } else {
        // MPEG-1: stuffing, the STD buffer size, then the times.
        while b.get(h) == Some(&0xFF) && h < 6 + 16 {
            h += 1;
        }
        if b.get(h).is_some_and(|&x| x >> 6 == 1) {
            h += 2;
        }
        match b.get(h).map(|&x| x >> 4) {
            Some(2) => {
                pts = b.get(h..h + 5).map(ts33);
                h += 5;
            }
            Some(3) => {
                pts = b.get(h..h + 5).map(ts33);
                dts = b.get(h + 5..h + 10).map(ts33);
                h += 10;
            }
            _ => h += 1,
        }
    }
    let mut sub = 0;
    if stream == PRIVATE1 {
        sub = *b.get(h)?;
        h += match sub {
            0x80..=0x8F => 4,
            0xA0..=0xAF => 7,
            _ => 1,
        };
    }
    let start = (p.at + h).min(end);
    Some(Pes {
        stream,
        sub,
        payload: start..end,
        pts,
        dts,
    })
}

impl PsLayout {
    /// Gathers `size` bytes from the packet at `offset` (payload offset in the top bits).
    fn gather(&self, data: &[u8], offset: u64, size: usize) -> Option<Vec<u8>> {
        let mut at = usize::try_from(offset & ((1 << SKIP_SHIFT) - 1)).ok()?;
        let mut skip = (offset >> SKIP_SHIFT) as usize;
        let mut out = Vec::with_capacity(size.min(1 << 24));
        while out.len() < size {
            let Some(p) = packet(data, at) else {
                at = resync(data, at + 1)?;
                continue;
            };
            at = p.end;
            let Some(pes) = pes(data, &p) else {
                continue;
            };
            if pes.stream != self.stream || pes.sub != self.sub {
                continue;
            }
            let pl = data.get(pes.payload.clone())?;
            let pl = pl.get(skip.min(pl.len())..)?;
            skip = 0;
            let n = pl.len().min(size - out.len());
            out.extend_from_slice(&pl[..n]);
        }
        Some(out)
    }

    pub fn read(&self, data: &[u8], s: &Sample) -> Option<Vec<u8>> {
        self.gather(data, s.offset, s.size as usize)
    }
}

/// A PES packet of a chosen stream, as listed.
#[derive(Clone, Copy)]
struct Piece {
    /// The packet's start.
    at: usize,
    payload_start: usize,
    len: usize,
    pts: Option<i64>,
    dts: Option<i64>,
}

/// Every PES packet in `ranges`, by stream (id and substream).
fn walk(
    data: &[u8],
    ranges: &[std::ops::Range<usize>],
    mut f: impl FnMut(u8, u8, Piece) -> Result<()>,
) -> Result<()> {
    for r in ranges {
        let mut at = if r.start == 0 {
            0
        } else {
            match resync(data, r.start) {
                Some(a) => a,
                None => continue,
            }
        };
        while at < r.end {
            let Some(p) = packet(data, at) else {
                match resync(data, at + 1) {
                    Some(a) => {
                        at = a;
                        continue;
                    }
                    None => break,
                }
            };
            at = p.end.max(p.at + 4);
            if p.code == END {
                continue;
            }
            if let Some(pes) = pes(data, &p) {
                f(
                    pes.stream,
                    pes.sub,
                    Piece {
                        at: p.at,
                        payload_start: pes.payload.start,
                        len: pes.payload.len(),
                        pts: pes.pts,
                        dts: pes.dts,
                    },
                )?;
            }
        }
    }
    Ok(())
}

/// The sound codings a programme stream carries, best first.
fn audio_rank(stream: u8, sub: u8) -> Option<(u8, AudioCodec)> {
    Some(match (stream, sub) {
        (0xC0..=0xDF, _) => (1, AudioCodec::Mpa(0)),
        (PRIVATE1, 0xA0..=0xA7) => (
            2,
            AudioCodec::Pcm {
                bits: 16,
                float: false,
                little_endian: false,
            },
        ),
        (PRIVATE1, 0x80..=0x87) => (0, AudioCodec::Ac3),
        (PRIVATE1, 0x88..=0x8F) => (3, AudioCodec::Other(*b"dtsc", 0)),
        _ => return None,
    })
}

/// Parses a programme stream or a bare MPEG video stream. With `samples`, every picture and
/// sound packet is listed; without, only the ends of the file are read.
pub fn parse(data: &[u8], samples: bool) -> Result<Movie> {
    if sniff_es(data) {
        return parse_es(data, samples);
    }
    if !sniff(data) {
        return Err(bad("not an MPEG programme stream"));
    }
    let whole = 0..data.len();
    let ranges: Vec<std::ops::Range<usize>> = if samples || data.len() <= 2 * EDGE {
        vec![whole]
    } else {
        vec![0..EDGE, data.len() - EDGE..data.len()]
    };
    let mut video_id: Option<u8> = None;
    let mut audio: Option<(u8, u8, u8, AudioCodec)> = None;
    let mut vpieces: Vec<Piece> = Vec::new();
    let mut apieces: Vec<Piece> = Vec::new();
    // First pass over the head: which streams there are.
    let head = 0..data.len().min(EDGE);
    walk(data, std::slice::from_ref(&head), |stream, sub, _| {
        if (0xE0..=0xEF).contains(&stream) {
            video_id.get_or_insert(stream);
        }
        if let Some((rank, codec)) = audio_rank(stream, sub)
            && audio.as_ref().is_none_or(|a| rank < a.2)
        {
            audio = Some((stream, sub, rank, codec));
        }
        Ok(())
    })?;
    if video_id.is_none() && audio.is_none() {
        return Err(Error::Unsupported("no video or sound in the stream".into()));
    }
    walk(data, &ranges, |stream, sub, piece| {
        if Some(stream) == video_id {
            vpieces.push(piece);
        } else if audio.as_ref().is_some_and(|a| a.0 == stream && a.1 == sub) {
            apieces.push(piece);
        }
        if vpieces.len() + apieces.len() > 16 * MAX_SAMPLES {
            return Err(Error::Unsupported("too many packets".into()));
        }
        Ok(())
    })?;
    let mut origin = vpieces
        .iter()
        .chain(&apieces)
        .filter_map(|p| p.pts)
        .min()
        .unwrap_or(0);
    let mut movie = Movie {
        brand: *b"mpps",
        timescale: 90_000,
        has_audio: audio.is_some(),
        ..Movie::default()
    };
    if let Some(id) = video_id {
        let layout = PsLayout { stream: id, sub: 0 };
        let mut v = video_track(data, layout, &vpieces, origin, samples)?;
        // A picture without a PTS shown before the first one with a PTS (an I picture
        // whose packet only has a DTS, ahead of the B pictures after it): the movie starts
        // with it, not before it (it was cut as lying before the start).
        if let Some(early) = v.samples.iter().map(|s| s.cts).min().filter(|&c| c < 0) {
            origin += early;
            v = video_track(data, layout, &vpieces, origin, samples)?;
        }
        movie.video = Some(v);
    }
    if let Some((stream, sub, _, codec)) = audio {
        movie.audio = Some(audio_track(
            data,
            PsLayout { stream, sub },
            codec,
            &apieces,
            origin,
            samples,
        ));
    }
    let d = movie
        .video
        .as_ref()
        .map_or(0, |v| v.duration)
        .max(movie.audio.as_ref().map_or(0, |a| a.duration));
    movie.duration_us = (d > 0).then(|| d * 1_000_000 / 90_000);
    Ok(movie)
}

/// A picture found in the video stream: where it starts (the first header byte of its
/// sample), its kind and timing.
struct Pic {
    piece: usize,
    skip: usize,
    es: u64,
    pts: Option<i64>,
    dts: Option<i64>,
    tr: u16,
    gop: usize,
    intra: bool,
    bidir: bool,
    /// Where the sample ends when a left-out picture follows it.
    end: Option<u64>,
}

/// The video elementary stream of `pieces` read as one: bytes at stream position `pos`.
struct Es<'a> {
    data: &'a [u8],
    pieces: &'a [Piece],
    /// Stream position of each piece's first payload byte.
    starts: Vec<u64>,
}

impl Es<'_> {
    fn bytes(&self, pos: u64, n: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(n);
        let mut k = self.starts.partition_point(|&s| s <= pos).saturating_sub(1);
        let mut off = (pos - self.starts.get(k).copied().unwrap_or(0)) as usize;
        while out.len() < n && k < self.pieces.len() {
            let p = self.pieces[k];
            let pl = &self.data[p.payload_start..p.payload_start + p.len];
            if off < pl.len() {
                let m = (pl.len() - off).min(n - out.len());
                out.extend_from_slice(&pl[off..off + m]);
            }
            off = 0;
            k += 1;
        }
        out
    }
}

fn video_track(
    data: &[u8],
    layout: PsLayout,
    pieces: &[Piece],
    origin: i64,
    samples: bool,
) -> Result<VideoTrack> {
    let mut starts = Vec::with_capacity(pieces.len());
    let mut pos = 0u64;
    for p in pieces {
        starts.push(pos);
        pos += p.len as u64;
    }
    let es = Es {
        data,
        pieces,
        starts,
    };
    let total = pos;
    // Start codes across the pieces: (piece, offset in its payload, stream position, code).
    let mut codes: Vec<(usize, usize, u64, u8)> = Vec::new();
    let mut tail: Vec<u8> = Vec::new();
    for (k, p) in pieces.iter().enumerate() {
        let pl = &data[p.payload_start..p.payload_start + p.len];
        // A start code straddling the previous piece's end.
        if !tail.is_empty() {
            let mut joint = tail.clone();
            joint.extend_from_slice(&pl[..pl.len().min(3)]);
            specular_mpeg2::start_codes(&joint, |i, c| {
                if i < tail.len() && i + 3 >= tail.len() {
                    let back = tail.len() - i;
                    let pos = es.starts[k] - back as u64;
                    let (pk, off) = locate(&es, pos);
                    codes.push((pk, off, pos, c));
                }
            });
        }
        specular_mpeg2::start_codes(pl, |i, c| codes.push((k, i, es.starts[k] + i as u64, c)));
        tail = pl[pl.len().saturating_sub(3)..].to_vec();
        if codes.len() > 64 * MAX_SAMPLES {
            return Err(Error::Unsupported("too many pictures".into()));
        }
    }
    codes.sort_by_key(|c| c.2);
    // Cut into pictures: a sample starts at its sequence or GOP header, else at its picture
    // header; a second field stays with its first.
    let mut pics: Vec<Pic> = Vec::new();
    let mut header_start: Option<(usize, usize, u64)> = None;
    let mut in_picture = false;
    let mut open_field: Option<specular_mpeg2::Structure> = None;
    let mut gop = 0usize;
    let mut config: Vec<u8> = Vec::new();
    let mut used_pts = vec![false; pieces.len()];
    let (mut started, mut leading_b, mut closed) = (false, false, false);
    for &(k, off, pos, code) in &codes {
        match code {
            0xB3 | 0xB8 => {
                if in_picture {
                    in_picture = false;
                    open_field = None;
                }
                if header_start.is_none() {
                    header_start = Some((k, off, pos));
                }
                if code == 0xB8 {
                    gop += 1;
                    closed = es.bytes(pos, 12).get(7).is_some_and(|b| b & 0x40 != 0);
                }
                if code == 0xB3 && config.is_empty() {
                    let head = es.bytes(pos, 4096);
                    if let Some(b) = specular_mpeg2::sequence_bytes(&head) {
                        config = b.to_vec();
                    }
                }
            }
            0x00 => {
                let head = es.bytes(pos, 128);
                let info = specular_mpeg2::scan(&head).pictures.first().copied();
                let structure = info.map_or(specular_mpeg2::Structure::Frame, |i| i.structure);
                let second = matches!((open_field, structure), (Some(o), s) if s != specular_mpeg2::Structure::Frame && s != o);
                if second {
                    open_field = None;
                    continue;
                }
                let (sk, soff, spos) = header_start.take().unwrap_or((k, off, pos));
                // A recording that starts mid-stream: pictures before the first sequence
                // header and I picture, and the B pictures right after it that predict from
                // before it, can't be decoded and are left out.
                let kind = info.map(|i| i.kind);
                let skip = if !started {
                    if kind == Some(specular_mpeg2::PictureType::I) && !config.is_empty() {
                        started = true;
                        leading_b = !closed;
                        false
                    } else {
                        true
                    }
                } else if leading_b && kind == Some(specular_mpeg2::PictureType::B) {
                    true
                } else {
                    leading_b = false;
                    false
                };
                if skip {
                    if let Some(prev) = pics.last_mut() {
                        prev.end.get_or_insert(spos);
                    }
                    in_picture = true;
                    open_field =
                        (structure != specular_mpeg2::Structure::Frame).then_some(structure);
                    continue;
                }
                let pts = (!used_pts[k]).then_some(pieces[k].pts).flatten();
                let dts = pts.and(pieces[k].dts);
                used_pts[k] = true;
                pics.push(Pic {
                    piece: sk,
                    skip: soff,
                    es: spos,
                    pts,
                    dts,
                    tr: info.map_or(0, |i| i.temporal_reference),
                    gop,
                    intra: info.is_some_and(|i| i.kind == specular_mpeg2::PictureType::I),
                    bidir: info.is_some_and(|i| i.kind == specular_mpeg2::PictureType::B),
                    end: None,
                });
                in_picture = true;
                open_field = (structure != specular_mpeg2::Structure::Frame).then_some(structure);
            }
            0x01..=0xAF => {}
            _ => {}
        }
        if pics.len() > MAX_SAMPLES {
            return Err(Error::Unsupported(format!("{} pictures", pics.len())));
        }
    }
    let seq = specular_mpeg2::probe(&config);
    let codec = if seq.as_ref().is_some_and(|s| !s.mpeg2) {
        Codec::Mpeg1
    } else {
        Codec::Mpeg2
    };
    let frame_ticks = seq
        .as_ref()
        .and_then(|s| s.frame_rate())
        .map_or(3600, |r| (90_000.0 / r).round() as i64)
        .max(1);
    // Pictures without a PTS: from their GOP's temporal references.
    let mut base_of_gop: std::collections::HashMap<usize, i64> = std::collections::HashMap::new();
    for p in &pics {
        if let Some(t) = p.pts {
            base_of_gop
                .entry(p.gop)
                .or_insert(t - i64::from(p.tr) * frame_ticks);
        }
    }
    // GOPs without times ahead of the first one with: just before it, each as long as its
    // temporal references say (they were put at the start, on top of the timed ones; a
    // stream copied from a bare one gives only its B pictures a PTS).
    if let Some(first) = pics.iter().find(|p| p.pts.is_some()).map(|p| p.gop) {
        let mut frames: std::collections::BTreeMap<usize, i64> = Default::default();
        for p in pics.iter().filter(|p| p.gop < first) {
            let n = frames.entry(p.gop).or_insert(0);
            *n = (*n).max(i64::from(p.tr) + 1);
        }
        let mut next = base_of_gop[&first];
        for (&g, &n) in frames.iter().rev() {
            next -= n * frame_ticks;
            base_of_gop.insert(g, next);
        }
    }
    let mut last_base = None::<(usize, i64, i64)>;
    let mut times = Vec::with_capacity(pics.len());
    for p in &pics {
        let base = match base_of_gop.get(&p.gop) {
            Some(&b) => b,
            None => {
                // A GOP without times: after the previous one.
                let b = last_base.map_or(origin, |(_, b, n)| b + n * frame_ticks);
                base_of_gop.insert(p.gop, b);
                b
            }
        };
        let pts = p.pts.unwrap_or(base + i64::from(p.tr) * frame_ticks);
        let n = match last_base {
            Some((g, b, n)) if g == p.gop && b == base => n.max(i64::from(p.tr) + 1),
            _ => i64::from(p.tr) + 1,
        };
        last_base = Some((p.gop, base, n));
        times.push((pts, p.dts));
    }
    // Two pictures stamped with the same time (muxers do that) stay two frames: the later
    // in decoding order a tick after.
    let mut by_time: Vec<usize> = (0..times.len()).collect();
    // B pictures are shown before the reference decoded ahead of them.
    by_time.sort_by_key(|&i| (times[i].0, !pics[i].bidir, i));
    for w in 1..by_time.len() {
        let (a, b) = (by_time[w - 1], by_time[w]);
        if times[b].0 <= times[a].0 {
            times[b].0 = times[a].0 + 1;
        }
    }
    let mut list: Vec<Sample> = Vec::with_capacity(pics.len());
    for (i, p) in pics.iter().enumerate() {
        let next_es = p.end.unwrap_or(pics.get(i + 1).map_or(total, |n| n.es));
        let (pts, _dts) = times[i];
        list.push(Sample {
            offset: pieces[p.piece].at as u64 | (p.skip as u64) << SKIP_SHIFT,
            size: (next_es - p.es).min(u64::from(u32::MAX)) as u32,
            dts: 0,
            cts: pts - origin,
            duration: 0,
            sync: p.intra,
        });
    }
    // Decode times: in decoding order, each no later than its presentation.
    let mut sorted: Vec<i64> = list.iter().map(|s| s.cts).collect();
    sorted.sort_unstable();
    for (s, &t) in list.iter_mut().zip(&sorted) {
        s.dts = t.max(0) as u64;
    }
    let mut order = sorted.clone();
    order.dedup();
    for s in &mut list {
        let k = order.partition_point(|&c| c <= s.cts);
        s.duration = order
            .get(k)
            .map_or(frame_ticks, |n| n - s.cts)
            .clamp(0, i64::from(u32::MAX)) as u32;
    }
    if !list.iter().any(|s| s.sync)
        && let Some(s) = list.first_mut()
    {
        s.sync = true;
    }
    let mut v = VideoTrack {
        codec: Some(codec),
        config,
        timescale: 90_000,
        layout: Layout::Ps(layout),
        ..VideoTrack::default()
    };
    v.duration = order.last().map_or(0, |&l| (l + frame_ticks).max(0) as u64);
    v.sample_count = list.len() as u32;
    if samples {
        v.samples = list;
    }
    if let Some(s) = &seq {
        (v.width, v.height) = (s.width, s.height);
        v.display = (s.width, s.height);
        if let Some(par) = s.pixel_aspect() {
            v.pixel_aspect = Some(par);
            v.display = v.shown_size();
        }
        v.interlaced = s.mpeg2 && !s.progressive;
    }
    Ok(v)
}

/// The piece and payload offset of stream position `pos`.
fn locate(es: &Es, pos: u64) -> (usize, usize) {
    let k = es.starts.partition_point(|&s| s <= pos).saturating_sub(1);
    (k, (pos - es.starts[k]) as usize)
}

fn audio_track(
    data: &[u8],
    layout: PsLayout,
    mut codec: AudioCodec,
    pieces: &[Piece],
    origin: i64,
    samples: bool,
) -> AudioTrack {
    let mut channels = 2u16;
    let mut sample_rate = 48_000u32;
    let mut byte_ticks: Option<f64> = None;
    match &mut codec {
        AudioCodec::Mpa(layer) => {
            let head: Vec<u8> = pieces
                .iter()
                .take(8)
                .flat_map(|p| {
                    data[p.payload_start..p.payload_start + p.len]
                        .iter()
                        .copied()
                })
                .collect();
            if let Some(h) = specular_mpa::probe(&head) {
                *layer = h.layer;
                channels = h.channels() as u16;
                sample_rate = h.sample_rate;
                byte_ticks =
                    (h.kbps() > 0).then(|| 90_000.0 * 8.0 / (f64::from(h.kbps()) * 1000.0));
            }
        }
        AudioCodec::Pcm { bits, .. } => {
            // DVD LPCM: the header's last bytes (just before the samples).
            if let Some(p) = pieces.first()
                && p.payload_start >= 3
            {
                let h = &data[p.payload_start - 3..p.payload_start];
                *bits = match h[1] >> 6 {
                    0 => 16,
                    1 => 20,
                    _ => 24,
                };
                sample_rate = if (h[1] >> 4) & 3 == 1 { 96_000 } else { 48_000 };
                channels = u16::from(h[1] & 7) + 1;
                byte_ticks = Some(
                    90_000.0
                        / (f64::from(sample_rate) * f64::from(channels) * f64::from(*bits) / 8.0),
                );
            }
            if let AudioCodec::Pcm {
                bits: b @ (20 | 24),
                ..
            } = codec
            {
                // 20 and 24-bit DVD LPCM pack their samples in groups (`wave_sound::DvdLpcm`).
                codec = AudioCodec::Other(*b"DVDL", b as u8);
            }
        }
        _ => {}
    }
    let mut list: Vec<Sample> = Vec::with_capacity(pieces.len());
    let mut last: Option<(i64, usize)> = None;
    for p in pieces {
        let t = match p.pts {
            Some(t) => t - origin,
            None => match (last, byte_ticks) {
                (Some((t, len)), Some(bt)) => t + (len as f64 * bt) as i64,
                (Some((t, _)), None) => t,
                _ => 0,
            },
        };
        last = Some((t, p.len));
        list.push(Sample {
            offset: p.at as u64,
            size: p.len.min(u32::MAX as usize) as u32,
            dts: t.max(0) as u64,
            cts: t,
            duration: 0,
            sync: true,
        });
    }
    for i in 0..list.len() {
        let next = list.get(i + 1).map(|n| n.dts);
        let s = &mut list[i];
        s.duration = next
            .map_or(0, |n| n.saturating_sub(s.dts))
            .min(u64::from(u32::MAX)) as u32;
    }
    if let Some(l) = list.len().checked_sub(2) {
        let d = list[l].duration;
        list[l + 1].duration = d;
    }
    AudioTrack {
        codec,
        channels,
        sample_rate,
        timescale: 90_000,
        duration: list.last().map_or(0, |s| s.dts + u64::from(s.duration)),
        sample_count: list.len() as u32,
        samples: if samples { list } else { Vec::new() },
        edit: Edit::default(),
        layout: Layout::Ps(layout),
        extra: Default::default(),
    }
}

/// A bare MPEG-1/2 video stream: one sample per picture, timed by the frame rate.
fn parse_es(data: &[u8], samples: bool) -> Result<Movie> {
    // Pieces of at most 64 KiB, like PES payloads: a sample's offset in its piece has 16 bits
    // (one piece of the whole stream put every picture past 64 KiB at the wrong place).
    let pieces: Vec<Piece> = (0..data.len().max(1))
        .step_by(ES_PIECE)
        .map(|at| Piece {
            at,
            payload_start: at,
            len: ES_PIECE.min(data.len() - at),
            pts: (at == 0).then_some(0),
            dts: None,
        })
        .collect();
    let mut v = video_track(data, PsLayout { stream: 0, sub: 0 }, &pieces, 0, true)?;
    // Contiguous bytes: the samples are plain ranges.
    v.layout = Layout::Contiguous;
    for s in &mut v.samples {
        s.offset = (s.offset & ((1 << SKIP_SHIFT) - 1)) + (s.offset >> SKIP_SHIFT);
    }
    if !samples {
        v.samples.clear();
    }
    let d = v.duration;
    Ok(Movie {
        brand: *b"mpvs",
        timescale: 90_000,
        duration_us: (d > 0).then(|| d * 1_000_000 / 90_000),
        video: Some(v),
        ..Movie::default()
    })
}
