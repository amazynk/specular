//! Ogg movies (RFC 3533: `.ogv`, `.ogg`, `.oga`) into the same [`Movie`] the MP4 demuxer
//! gives: the first Theora stream and the first Vorbis, Opus or FLAC stream.
//!
//! - Pages are walked by their headers (a damaged stretch is skipped to the next capture
//!   pattern); packets are joined across pages per stream. A sample's bytes are read back
//!   through [`OggLayout`], from the page where its packet starts.
//! - Theora frames are timed from their order and the frame rate, anchored to the first
//!   page's granule position; keyframes are marked from the frame type.
//! - Sound packets are timed by their own lengths (Vorbis block sizes, Opus TOCs, FLAC frame
//!   headers), anchored to the first granule position, so the start trim (Opus pre-skip, a
//!   stream starting late, a trim signalled by the second packet) is kept as the track's
//!   edit.
//! - Dirac / VC-2: the sequence header packet is the configuration, every later packet a
//!   picture, timed from the sequence header's frame rate.
//! - OGM video: the stream header names the codec (an AVI four-character code) and the time
//!   per frame; each data packet's leading byte and length bytes are skipped.
//! - OGM sound: the stream header names the codec by its WAVE format tag (MP3, AC-3, PCM and
//!   the others AVI plays); packets are timed by their length bytes, else by their size
//!   (PCM), else by the page's granule position.
//! - Skeleton, Kate and other logical streams are skipped.

use std::borrow::Cow;

use crate::mkv::opus_samples;
use crate::mp4::{
    AudioCodec, AudioTrack, Codec, Edit, Error, Layout, MAX_SAMPLES, Movie, Result, Sample,
    VideoTrack,
};

fn bad(s: impl Into<String>) -> Error {
    Error::Malformed(s.into())
}

/// The capture pattern every page starts with.
const CAPTURE: &[u8; 4] = b"OggS";
/// Where the payload offset sits in [`Sample::offset`]: page offset below, bytes into the
/// page's payload above.
const SKIP_SHIFT: u32 = 40;

/// Whether `data` is an Ogg stream.
pub fn sniff(data: &[u8]) -> bool {
    data.len() >= 27 && &data[..4] == CAPTURE && data[4] == 0 && data[5] & 2 != 0
}

/// One page's header.
#[derive(Clone, Copy, Debug)]
struct Page {
    at: usize,
    continued: bool,
    granule: i64,
    serial: u32,
    /// Page sequence number (counts each stream's pages; a jump means pages were lost).
    seq: u32,
    /// Payload start and end (file offsets).
    body: usize,
    end: usize,
    nsegs: usize,
}

fn page(data: &[u8], at: usize) -> Option<Page> {
    let h = data.get(at..at + 27)?;
    if &h[..4] != CAPTURE || h[4] != 0 {
        return None;
    }
    let nsegs = usize::from(h[26]);
    let lacing = data.get(at + 27..at + 27 + nsegs)?;
    let len: usize = lacing.iter().map(|&l| usize::from(l)).sum();
    let body = at + 27 + nsegs;
    let end = body + len;
    if end > data.len() {
        return None;
    }
    Some(Page {
        at,
        continued: h[5] & 1 != 0,
        granule: i64::from_le_bytes(h[6..14].try_into().ok()?),
        serial: u32::from_le_bytes(h[14..18].try_into().ok()?),
        seq: u32::from_le_bytes(h[18..22].try_into().ok()?),
        body,
        end,
        nsegs,
    })
}

/// The next page at or after `at` (skipping damage to the next capture pattern).
fn next_page(data: &[u8], mut at: usize) -> Option<Page> {
    loop {
        if let Some(p) = page(data, at) {
            return Some(p);
        }
        let from = at + 1;
        at = from + data.get(from..)?.windows(4).position(|w| w == CAPTURE)?;
    }
}

/// Where a logical stream's packets lie: the stream's serial number. A sample's offset is
/// the page where its packet starts with the bytes to skip into that page's payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OggLayout {
    pub serial: u32,
}

impl OggLayout {
    /// The packet bytes of sample `s`: borrowed when the packet lies in one page.
    pub fn read<'a>(&self, data: &'a [u8], s: &Sample) -> Option<Cow<'a, [u8]>> {
        let mut at = usize::try_from(s.offset & ((1 << SKIP_SHIFT) - 1)).ok()?;
        let mut skip = (s.offset >> SKIP_SHIFT) as usize;
        let size = s.size as usize;
        let first = page(data, at)?;
        let start = first.body + skip;
        if start + size <= first.end {
            return data.get(start..start + size).map(Cow::Borrowed);
        }
        let mut out = Vec::with_capacity(size.min(1 << 24));
        while out.len() < size {
            let p = next_page(data, at)?;
            at = p.end;
            if p.serial != self.serial {
                continue;
            }
            let pl = data.get(p.body + skip.min(p.end - p.body)..p.end)?;
            skip = 0;
            let n = pl.len().min(size - out.len());
            out.extend_from_slice(&pl[..n]);
        }
        Some(Cow::Owned(out))
    }
}

/// A packet of a logical stream: where it starts, its size, and the granule position of the
/// page it ends on when it is the last packet ending there.
#[derive(Clone, Copy, Debug)]
struct Packet {
    page: usize,
    skip: usize,
    size: usize,
    granule: Option<i64>,
}

/// One logical stream while walking the pages.
struct Stream {
    serial: u32,
    packets: Vec<Packet>,
    /// The packet being assembled.
    open: Option<Packet>,
    /// The first bytes of its first packets (headers), kept whole.
    headers: Vec<Vec<u8>>,
    kind: Kind,
    /// The last page's sequence number.
    seq: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Theora,
    Vorbis,
    Opus,
    Flac,
    Speex,
    /// Dirac / VC-2 (the first packet a sequence header).
    Dirac,
    /// An OGM video stream (the 0x01 stream header naming its codec).
    Ogm,
    /// An OGM sound stream (the 0x01 stream header naming its WAVE format tag).
    OgmAudio,
    Skip,
}

fn kind(first: &[u8]) -> Kind {
    if first.starts_with(b"\x80theora") {
        Kind::Theora
    } else if first.starts_with(b"\x01vorbis") {
        Kind::Vorbis
    } else if first.starts_with(b"OpusHead") {
        Kind::Opus
    } else if first.starts_with(b"\x7FFLAC") {
        Kind::Flac
    } else if first.starts_with(b"Speex   ") {
        Kind::Speex
    } else if first.starts_with(b"BBCD") {
        Kind::Dirac
    } else if first.starts_with(b"\x01video") {
        Kind::Ogm
    } else if first.starts_with(b"\x01audio") {
        Kind::OgmAudio
    } else {
        Kind::Skip
    }
}

/// Header packets each stream kind starts with.
fn header_count(k: Kind, first: &[u8]) -> usize {
    match k {
        Kind::Theora | Kind::Vorbis => 3,
        Kind::Opus => 2,
        // The header, the comments, then as many extra headers as it says.
        Kind::Speex => {
            2 + specular_speex::Header::parse(first).map_or(0, |h| h.extra_headers as usize)
        }
        // Ogg FLAC: the first packet, then as many metadata packets as it says
        Kind::Flac => {
            1 + first
                .get(7..9)
                .map_or(0, |b| usize::from(u16::from_be_bytes([b[0], b[1]])))
        }
        _ => 1,
    }
}

/// Walks every page, collecting the packets of each logical stream.
fn walk(data: &[u8]) -> Result<Vec<Stream>> {
    let mut streams: Vec<Stream> = Vec::new();
    let mut at = 0;
    let mut total = 0usize;
    while let Some(p) = next_page(data, at) {
        at = p.end;
        let idx = match streams.iter().position(|s| s.serial == p.serial) {
            Some(i) => i,
            None => {
                streams.push(Stream {
                    serial: p.serial,
                    packets: Vec::new(),
                    open: None,
                    headers: Vec::new(),
                    kind: Kind::Skip,
                    seq: None,
                });
                streams.len() - 1
            }
        };
        let s = &mut streams[idx];
        let lacing = &data[p.at + 27..p.at + 27 + p.nsegs];
        let mut off = 0usize;
        let mut last_done: Option<usize> = None;
        // Pages lost before this one (a gap in the sequence): the packet left open is
        // dropped, and so is the end of one this page continues.
        let lost = s.seq.is_some_and(|q| p.seq != q.wrapping_add(1));
        s.seq = Some(p.seq);
        let mut tail = lost && p.continued;
        if !p.continued || lost {
            // a packet left open by a lost page is dropped
            s.open = None;
        }
        for &l in lacing {
            let l = usize::from(l);
            if tail {
                off += l;
                tail = l == 255;
                continue;
            }
            let pk = s.open.get_or_insert(Packet {
                page: p.at,
                skip: off,
                size: 0,
                granule: None,
            });
            pk.size += l;
            off += l;
            if l < 255 {
                let done = s.open.take().unwrap_or(Packet {
                    page: p.at,
                    skip: off,
                    size: 0,
                    granule: None,
                });
                s.packets.push(done);
                last_done = Some(s.packets.len() - 1);
                total += 1;
            }
        }
        if let Some(i) = last_done
            && p.granule != -1
        {
            s.packets[i].granule = Some(p.granule);
        }
        // the first packets (headers) are read whole as they complete
        while s.headers.len() < s.packets.len().min(16) {
            let k = s.headers.len();
            let pk = s.packets[k];
            let bytes = read_packet(data, s.serial, &pk).unwrap_or_default();
            if k == 0 {
                s.kind = kind(&bytes);
            }
            let need = header_count(s.kind, s.headers.first().map_or(&bytes, |h| h));
            if k >= need {
                break;
            }
            s.headers.push(bytes);
        }
        if total > 4 * MAX_SAMPLES {
            return Err(Error::Unsupported(format!("{total} packets")));
        }
    }
    if streams.is_empty() {
        return Err(bad("no Ogg pages"));
    }
    Ok(streams)
}

fn read_packet(data: &[u8], serial: u32, p: &Packet) -> Option<Vec<u8>> {
    prefix(data, serial, p, p.size)
}

/// The first `n` bytes (at most) of a packet.
fn prefix(data: &[u8], serial: u32, p: &Packet, n: usize) -> Option<Vec<u8>> {
    let s = Sample {
        offset: p.page as u64 | (p.skip as u64) << SKIP_SHIFT,
        size: p.size.min(n).min(u32::MAX as usize) as u32,
        dts: 0,
        cts: 0,
        duration: 0,
        sync: true,
    };
    OggLayout { serial }.read(data, &s).map(Cow::into_owned)
}

/// Xiph lacing of header packets (Matroska's `CodecPrivate` layout): the count less one,
/// the sizes of all but the last in 255-runs, then the packets.
fn xiph_lace(headers: &[Vec<u8>]) -> Vec<u8> {
    let mut v = vec![(headers.len().saturating_sub(1)) as u8];
    for h in &headers[..headers.len().saturating_sub(1)] {
        let mut n = h.len();
        while n >= 255 {
            v.push(255);
            n -= 255;
        }
        v.push(n as u8);
    }
    for h in headers {
        v.extend_from_slice(h);
    }
    v
}

/// The sample at packet `p`.
fn sample(p: &Packet, dts: i64, duration: u32, sync: bool) -> Sample {
    Sample {
        offset: p.page as u64 | (p.skip as u64) << SKIP_SHIFT,
        size: p.size.min(u32::MAX as usize) as u32,
        dts: dts.max(0) as u64,
        cts: dts,
        duration,
        sync,
    }
}

/// The Theora track: frames in order, timed from the frame rate.
fn theora_track(data: &[u8], s: &Stream, samples: bool) -> Option<VideoTrack> {
    if s.headers.len() < 3 {
        return None;
    }
    let config = xiph_lace(&s.headers);
    let info = specular_theora::Info::parse(&s.headers[0]).ok();
    let (num, den) = info
        .as_ref()
        .map_or((25, 1), |i| (i.fps.0.max(1), i.fps.1.max(1)));
    let shift = info.as_ref().map_or(6, |i| i.keyframe_granule_shift);
    // 3.2.1 and later number the first frame's granule 1
    let one_based = info.as_ref().is_some_and(|i| i.version >= (3, 2, 1));
    let frames = &s.packets[3..];
    // the frame number of the first frame, from the first page that says
    let mut first = 0i64;
    for (k, p) in frames.iter().enumerate() {
        if let Some(g) = p.granule.filter(|&g| g >= 0) {
            let shift = u32::from(shift.min(62));
            let n = (g >> shift) + (g & ((1 << shift) - 1)) - i64::from(one_based);
            first = n - k as i64;
            break;
        }
    }
    let frame_us = |n: i64| (n as i128 * 1_000_000 * i128::from(den) / i128::from(num)) as i64;
    let mut list = Vec::new();
    let mut last_key = None;
    for (k, p) in frames.iter().enumerate() {
        let n = first + k as i64;
        let t = frame_us(n);
        let d = (frame_us(n + 1) - t).clamp(0, i64::from(u32::MAX)) as u32;
        // a zero-length packet repeats the frame before
        let key = p.size > 0
            && prefix(data, s.serial, p, 1).is_some_and(|b| specular_theora::is_keyframe(&b));
        if key {
            last_key = Some(k);
        }
        list.push(sample(p, t, d, key));
    }
    if last_key.is_none()
        && let Some(f) = list.first_mut()
    {
        f.sync = true;
    }
    let (w, h, aspect) = info.as_ref().map_or((0, 0, None), |i| {
        let a = (i.aspect.0 > 0 && i.aspect.1 > 0 && i.aspect.0 != i.aspect.1).then_some(i.aspect);
        (i.pic_width, i.pic_height, a)
    });
    let mut v = VideoTrack {
        codec: Some(Codec::Theora),
        config,
        width: w,
        height: h,
        display: (w, h),
        timescale: 1_000_000,
        pixel_aspect: aspect,
        layout: Layout::Ogg(OggLayout { serial: s.serial }),
        ..VideoTrack::default()
    };
    v.display = v.shown_size();
    v.duration = list.last().map_or(0, |l| {
        (l.cts + i64::from(l.duration) - list[0].cts.min(0)).max(0) as u64
    });
    v.sample_count = list.len() as u32;
    if samples {
        v.samples = list;
    }
    Some(v)
}

/// A video track of the samples in `list`.
fn framed_track(
    codec: Codec,
    s: &Stream,
    list: Vec<Sample>,
    config: Vec<u8>,
    (w, h): (u32, u32),
    samples: bool,
) -> VideoTrack {
    let mut v = VideoTrack {
        codec: Some(codec),
        config,
        width: w,
        height: h,
        display: (w, h),
        timescale: 1_000_000,
        layout: Layout::Ogg(OggLayout { serial: s.serial }),
        ..VideoTrack::default()
    };
    v.duration = list
        .last()
        .map_or(0, |l| (l.cts + i64::from(l.duration)).max(0) as u64);
    v.sample_count = list.len() as u32;
    if samples {
        v.samples = list;
    }
    v
}

/// Dirac / VC-2 in Ogg: the sequence header packet is the configuration; every later packet
/// is a picture's data units, every picture intra-coded in VC-2. Timed from the sequence
/// header's frame rate.
fn dirac_track(data: &[u8], s: &Stream, samples: bool) -> Option<VideoTrack> {
    let config = s.headers.first()?.clone();
    let info = specular_vc2::probe(&config);
    let (num, den) = info
        .as_ref()
        .map_or((25, 1), |i| (i.frame_rate.0.max(1), i.frame_rate.1.max(1)));
    let frame_us = |n: i64| (n as i128 * 1_000_000 * i128::from(den) / i128::from(num)) as i64;
    let list: Vec<Sample> = s
        .packets
        .iter()
        .skip(1)
        .filter(|p| p.size > 0)
        .enumerate()
        .map(|(k, p)| {
            let t = frame_us(k as i64);
            let d = (frame_us(k as i64 + 1) - t).clamp(0, i64::from(u32::MAX)) as u32;
            sample(p, t, d, true)
        })
        .collect();
    let _ = data;
    let size = info.as_ref().map_or((0, 0), |i| (i.width, i.height));
    Some(framed_track(Codec::Vc2, s, list, config, size, samples))
}

/// OGM video (the DirectShow Ogg mapping): a 0x01 stream header with the codec's
/// four-character code, the time per frame (100 ns units) and the size; data packets start
/// with a byte (keyframe flag 0x08, the count of length bytes in bits 6, 7 and 1) and that
/// many bytes of duration in frames before the frame itself.
fn ogm_track(data: &[u8], s: &Stream, samples: bool) -> Option<VideoTrack> {
    let head = s.headers.first()?;
    let le32 = |at: usize| -> Option<u32> {
        Some(u32::from_le_bytes(head.get(at..at + 4)?.try_into().ok()?))
    };
    let le64 = |at: usize| -> Option<i64> {
        Some(i64::from_le_bytes(head.get(at..at + 8)?.try_into().ok()?))
    };
    let fourcc: [u8; 4] = head.get(9..13)?.try_into().ok()?;
    let codec = crate::avi::fourcc_codec(fourcc);
    let unit = le64(17).filter(|&u| u > 0).unwrap_or(400_000);
    let per = le64(25).filter(|&u| u > 0).unwrap_or(1);
    let (w, h) = (le32(45).unwrap_or(0), le32(49).unwrap_or(0));
    // 100 ns units per frame, as microseconds.
    let frame_us = |n: i64| (n as i128 * i128::from(unit) / i128::from(per) / 10) as i64;
    let mut list = Vec::new();
    let mut n = 0i64;
    for d in ogm_data(data, s) {
        let Some(q) = d.payload else {
            n += 1;
            continue;
        };
        // A packet lasts a frame or a few; a wild length (a muxer's junk) counts as one.
        let frames = d.length.filter(|&l| l <= 1000).unwrap_or(1).max(1);
        let t = frame_us(n);
        let dur = (frame_us(n + frames) - t).clamp(0, i64::from(u32::MAX)) as u32;
        list.push(sample(&q, t, dur, d.key));
        n += frames;
    }
    if let Some(f) = list.first_mut() {
        f.sync = true;
    }
    Some(framed_track(codec, s, list, Vec::new(), (w, h), samples))
}

/// One OGM data packet: its payload (None when empty), the duration its length bytes give
/// (frames or samples), its keyframe flag and its granule position.
struct OgmData {
    payload: Option<Packet>,
    length: Option<i64>,
    key: bool,
    granule: Option<i64>,
}

/// The data packets of an OGM stream. Each starts with a byte (keyframe flag 0x08, the count
/// of length bytes in bits 6, 7 and 1) and that many bytes of duration before the payload;
/// header (0x01), comment (0x03) and setup (0x05) packets have the low bit set and are left
/// out.
fn ogm_data(data: &[u8], s: &Stream) -> Vec<OgmData> {
    let mut out = Vec::new();
    for p in &s.packets {
        let Some(b0) = prefix(data, s.serial, p, 1).and_then(|b| b.first().copied()) else {
            continue;
        };
        if b0 & 1 != 0 || p.size == 0 {
            continue;
        }
        let lenbytes = usize::from((b0 & 0xC0) >> 6 | (b0 & 2) << 1);
        let skip = 1 + lenbytes;
        let length = prefix(data, s.serial, p, skip)
            .filter(|b| b.len() == skip && lenbytes > 0)
            .map(|b| {
                b[1..]
                    .iter()
                    .rev()
                    .fold(0i64, |a, &x| a << 8 | i64::from(x))
            });
        // The payload starts after the data byte and the length bytes; within the first page
        // when it fits (a packet continued on the next page has 255 bytes or more on this
        // one), else the packet stays whole (a damaged frame at worst).
        let payload = (p.size > skip).then(|| {
            let fits = page(data, p.page).is_some_and(|pg| pg.body + p.skip + skip <= pg.end);
            if fits {
                Packet {
                    skip: p.skip + skip,
                    size: p.size - skip,
                    ..*p
                }
            } else {
                *p
            }
        });
        out.push(OgmData {
            payload,
            length,
            key: b0 & 8 != 0,
            granule: p.granule,
        });
    }
    out
}

/// The WAVE format tag an OGM sound header names: ASCII hex digits (`"2000"`, `"55"`).
fn ogm_wave_tag(sub: &[u8]) -> Option<u16> {
    let hex: Vec<u8> = sub
        .iter()
        .copied()
        .take_while(|&b| b != 0 && b != b' ')
        .collect();
    u16::from_str_radix(std::str::from_utf8(&hex).ok()?, 16).ok()
}

/// OGM sound: the stream header (after its 0x01 byte and `audio` padded to 8 bytes) has the
/// format tag in ASCII hex at 9, the samples per second at 25, bits per sample at 41,
/// channels at 45, the block alignment at 47 and the bytes per second at 49 (little-endian).
/// Data packets are timed in samples by their length bytes, else (PCM) by their size, else
/// by the granule position of the page they end on.
fn ogm_audio_track(data: &[u8], s: &Stream, samples: bool) -> Option<AudioTrack> {
    let head = s.headers.first()?;
    let tag = ogm_wave_tag(head.get(9..13)?)?;
    let le = |at: usize, n: usize| -> Option<u64> {
        let b = head.get(at..at + n)?;
        Some(b.iter().rev().fold(0u64, |a, &x| a << 8 | u64::from(x)))
    };
    let rate = u32::try_from(le(25, 8)?).ok().filter(|&r| r > 0)?;
    // A WAVEFORMATEX for the tag table AVI uses.
    let mut wave = Vec::with_capacity(18);
    wave.extend(tag.to_le_bytes());
    wave.extend((le(45, 2)? as u16).to_le_bytes());
    wave.extend(rate.to_le_bytes());
    wave.extend((le(49, 4)? as u32).to_le_bytes());
    wave.extend((le(47, 2)? as u16).to_le_bytes());
    wave.extend((le(41, 2)? as u16).to_le_bytes());
    wave.extend(0u16.to_le_bytes());
    let (codec, channels, rate) = crate::avi::wave_codec(&wave)?;
    let frame_bytes = match codec {
        AudioCodec::Pcm { bits, .. } => usize::from(channels) * usize::from(bits).div_ceil(8),
        _ => 0,
    };
    let mut list = Vec::new();
    let mut at = 0i64;
    for d in ogm_data(data, s) {
        let Some(q) = d.payload else {
            continue;
        };
        let n = d
            .length
            .filter(|&n| n > 0)
            .or_else(|| (frame_bytes > 0).then(|| (q.size / frame_bytes) as i64))
            .or_else(|| d.granule.map(|g| g - at))
            .unwrap_or(0)
            .clamp(0, i64::from(u32::MAX));
        list.push(sample(&q, at, n as u32, true));
        at += n;
    }
    if list.len() > MAX_SAMPLES {
        return None;
    }
    Some(AudioTrack {
        codec,
        channels,
        sample_rate: rate,
        timescale: rate,
        duration: at.max(0) as u64,
        sample_count: list.len() as u32,
        samples: if samples { list } else { Vec::new() },
        edit: Edit::default(),
        layout: Layout::Ogg(OggLayout { serial: s.serial }),
        extra: Default::default(),
    })
}

/// The block size of a FLAC frame from its header (RFC 9639 §9.1.1), without decoding.
fn flac_block(frame: &[u8]) -> Option<u64> {
    if frame.len() < 6 || frame[0] != 0xFF || frame[1] & 0xFE != 0xF8 {
        return None;
    }
    let code = frame[2] >> 4;
    // the coded number (UTF-8-like) starts at byte 4
    let lead = frame[4].leading_ones() as usize;
    let at = 4 + lead.max(1);
    Some(match code {
        1 => 192,
        2..=5 => 576 << (code - 2),
        6 => u64::from(*frame.get(at)?) + 1,
        7 => u64::from(u16::from_be_bytes([*frame.get(at)?, *frame.get(at + 1)?])) + 1,
        8..=15 => 256 << (code - 8),
        _ => return None,
    })
}

/// The sound track: packets timed by their own lengths at the track's rate.
fn audio_track(data: &[u8], s: &Stream, samples: bool) -> Option<AudioTrack> {
    if s.kind == Kind::OgmAudio {
        return ogm_audio_track(data, s, samples);
    }
    let nheaders = header_count(s.kind, s.headers.first()?);
    if s.headers.len() < nheaders {
        return None;
    }
    let (codec, rate, channels, pre_skip) = match s.kind {
        Kind::Vorbis => {
            let i = specular_vorbis::Ident::parse(&s.headers[0]).ok()?;
            (
                AudioCodec::Vorbis(xiph_lace(&s.headers)),
                i.sample_rate,
                u16::from(i.channels),
                0,
            )
        }
        Kind::Opus => {
            let h = &s.headers[0];
            let pre = h
                .get(10..12)
                .map_or(0, |b| i64::from(u16::from_le_bytes([b[0], b[1]])));
            (
                AudioCodec::Opus(h.clone()),
                48_000,
                u16::from(*h.get(9)?),
                pre,
            )
        }
        Kind::Flac => {
            let i = specular_flac::StreamInfo::from_config(&s.headers[0]).ok()?;
            (
                AudioCodec::Flac(s.headers[0].clone()),
                i.sample_rate,
                u16::from(i.channels),
                0,
            )
        }
        Kind::Speex => {
            let h = specular_speex::Header::parse(&s.headers[0]).ok()?;
            let d = specular_speex::Decoder::new(h.mode, h.channels);
            (
                AudioCodec::Speex(s.headers[0].clone()),
                // The header's rate (an encoder may have resampled to the mode's).
                if h.rate > 0 { h.rate } else { d.sample_rate() },
                u16::from(h.channels),
                0,
            )
        }
        _ => return None,
    };
    // Speex packets hold a fixed number of frames.
    let speex_packet = match &codec {
        AudioCodec::Speex(h) => specular_speex::Header::parse(h).map_or(0, |p| {
            let d = specular_speex::Decoder::new(p.mode, p.channels);
            i64::from(p.frames_per_packet) * d.frame_size() as i64
        }),
        _ => 0,
    };
    let vorbis = match (&codec, s.headers.len()) {
        (AudioCodec::Vorbis(_), 3) => {
            specular_vorbis::Decoder::new(&s.headers[0], &s.headers[2]).ok()
        }
        _ => None,
    };
    let packets = &s.packets[nheaders.min(s.packets.len())..];
    let mut list = Vec::with_capacity(packets.len());
    let mut at = 0i64;
    let mut prev_block: Option<usize> = None;
    // granule position less our running end: the stream's offset from our clock
    let mut offset: Option<i64> = None;
    // The first sample after the last page's granule position.
    let mut run_from = 0usize;
    for p in packets {
        // the lengths are in the first bytes: Vorbis's mode, Opus's TOC, FLAC's frame header
        let bytes = prefix(data, s.serial, p, 16).unwrap_or_default();
        let n = match s.kind {
            Kind::Vorbis => vorbis
                .as_ref()
                .and_then(|d| d.packet_blocksize(&bytes))
                .map_or(0, |b| {
                    let n = prev_block.map_or(0, |pb| pb / 4 + b / 4);
                    prev_block = Some(b);
                    n as i64
                }),
            Kind::Opus => opus_samples(&bytes).unwrap_or(0) as i64,
            Kind::Flac => flac_block(&bytes).unwrap_or(0) as i64,
            Kind::Speex => speex_packet,
            _ => 0,
        };
        list.push(sample(p, at, n.clamp(0, i64::from(u32::MAX)) as u32, true));
        at += n;
        if let (Kind::Opus, Some(o), Some(g)) = (s.kind, offset, p.granule.filter(|&g| g >= 0)) {
            at += fill_lost(&mut list, run_from, g - (at + o));
        }
        if p.granule.is_some() {
            run_from = list.len();
        }
        if offset.is_none()
            && let Some(g) = p.granule.filter(|&g| g >= 0)
        {
            // A stream starting late is honoured from any page. One trimmed at its start
            // (negative) only when the granule comes by the second packet, where the
            // Vorbis I encapsulation puts it (ffmpeg's muxer writes granules on its first
            // audio page that imply a trim neither it nor the source has).
            let k = list.len();
            offset = Some((g - at).max(if k <= 2 { i64::MIN } else { 0 }));
        }
    }
    // The decoded sample at our time `t` has granule `t + offset`; presentation time zero
    // is granule `pre_skip`.
    let offset = offset.unwrap_or(0);
    let start = pre_skip - offset;
    let edit = if start >= 0 {
        Edit {
            start,
            delay_us: 0,
            length_us: None,
        }
    } else {
        Edit {
            start: 0,
            delay_us: ((-start) as u128 * 1_000_000 / u128::from(rate.max(1))) as u64,
            length_us: None,
        }
    };
    if list.len() > MAX_SAMPLES {
        return None;
    }
    Some(AudioTrack {
        codec,
        channels,
        sample_rate: rate,
        timescale: rate,
        duration: at.max(0) as u64,
        sample_count: list.len() as u32,
        samples: if samples { list } else { Vec::new() },
        edit,
        layout: Layout::Ogg(OggLayout { serial: s.serial }),
        extra: Default::default(),
    })
}

/// Opus packets lost with their pages: a page's granule position `gap` samples past the end
/// of its packets means packets are missing before `from` (the page's first). Empty samples
/// of the packet length before them stand in for those (the player rebuilds them from the
/// next packet's FEC data or conceals them), keeping the timeline. Returns the time added.
fn fill_lost(list: &mut Vec<Sample>, from: usize, gap: i64) -> i64 {
    let Some(before) = from.checked_sub(1).and_then(|i| list.get(i)).copied() else {
        return 0;
    };
    let dur = i64::from(before.duration);
    // (More than ten seconds is a broken granule position, not a loss.)
    if dur == 0 || gap < dur / 2 || gap > 480_000 {
        return 0;
    }
    let count = ((gap + dur / 2) / dur).max(1);
    let start = list
        .get(from)
        .map_or(before.dts as i64 + dur, |s| s.dts as i64);
    let lost = (0..count).map(|i| Sample {
        offset: before.offset,
        size: 0,
        dts: (start + i * dur) as u64,
        cts: start + i * dur,
        duration: dur as u32,
        sync: true,
    });
    list.splice(from..from, lost);
    let added = count * dur;
    for s in &mut list[from + count as usize..] {
        s.dts += added as u64;
        s.cts += added;
    }
    added
}

/// Parses an Ogg movie. With `samples`, every frame and sound packet is listed.
pub fn parse(data: &[u8], samples: bool) -> Result<Movie> {
    if !sniff(data) {
        return Err(bad("not an Ogg stream"));
    }
    let streams = walk(data)?;
    let video_stream = streams
        .iter()
        .find(|s| matches!(s.kind, Kind::Theora | Kind::Dirac | Kind::Ogm));
    let audio_stream = streams
        .iter()
        .find(|s| matches!(s.kind, Kind::Vorbis | Kind::Opus | Kind::Flac | Kind::Speex))
        .or_else(|| streams.iter().find(|s| s.kind == Kind::OgmAudio));
    let video = match video_stream {
        Some(s) if s.kind == Kind::Theora => theora_track(data, s, samples),
        Some(s) if s.kind == Kind::Dirac => dirac_track(data, s, samples),
        Some(s) => ogm_track(data, s, samples),
        None => None,
    };
    let audio = audio_stream.and_then(|s| audio_track(data, s, samples));
    if video.is_none() && audio.is_none() {
        return Err(Error::Unsupported(
            "an Ogg file without video or sound".into(),
        ));
    }
    // Sound alone (Ogg Vorbis, Opus, FLAC): its own length.
    let duration_us = match &video {
        Some(v) => v.duration_us(),
        None => audio.as_ref().and_then(|a| {
            let d = a.us(a.duration as i64).max(0) as u64;
            (d > 0).then_some(d)
        }),
    };
    Ok(Movie {
        brand: *b"OggS",
        timescale: 1_000_000,
        duration_us,
        created: None,
        has_audio: audio.is_some(),
        video,
        audio,
        content_id: None,
        date: None,
        make: None,
        model: None,
        xmp: None,
        tags: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One page of `packets` for stream `serial`.
    fn page_bytes(serial: u32, seq: u32, granule: i64, bos: bool, packets: &[&[u8]]) -> Vec<u8> {
        let mut lacing = Vec::new();
        for p in packets {
            let mut n = p.len();
            while n >= 255 {
                lacing.push(255);
                n -= 255;
            }
            lacing.push(n as u8);
        }
        let mut v = b"OggS".to_vec();
        v.push(0);
        v.push(if bos { 2 } else { 0 });
        v.extend_from_slice(&granule.to_le_bytes());
        v.extend_from_slice(&serial.to_le_bytes());
        v.extend_from_slice(&seq.to_le_bytes());
        v.extend_from_slice(&[0; 4]);
        v.push(lacing.len() as u8);
        v.extend_from_slice(&lacing);
        for p in packets {
            v.extend_from_slice(p);
        }
        v
    }

    #[test]
    fn packets_are_joined_across_pages_and_streams() {
        // stream 1 packet of 300 bytes split over two pages, with stream 2 between them
        let big: Vec<u8> = (0..300u32).map(|i| i as u8).collect();
        let mut d = page_bytes(1, 0, 0, true, &[b"\x01head"]);
        d.extend(page_bytes(2, 0, 0, true, &[b"other"]));
        // first page: 255 bytes of the packet (one lacing value of 255, packet open)
        let mut p = page_bytes(1, 1, -1, false, &[&big[..255]]);
        // the lacing for a 255-byte piece is [255, 0]; drop the 0 to leave it open
        let nsegs_at = 26;
        p[nsegs_at] = 1;
        p.remove(28);
        d.extend(p);
        d.extend(page_bytes(2, 1, 5, false, &[b"x"]));
        let mut q = page_bytes(1, 2, 77, false, &[&big[255..]]);
        q[5] |= 1; // continued
        d.extend(q);
        assert!(sniff(&d));
        let streams = walk(&d).unwrap();
        let s = &streams[0];
        assert_eq!(s.packets.len(), 2);
        let pk = s.packets[1];
        assert_eq!(pk.size, 300);
        assert_eq!(pk.granule, Some(77));
        assert_eq!(read_packet(&d, 1, &pk).unwrap(), big);
        assert_eq!(read_packet(&d, 2, &streams[1].packets[1]).unwrap(), b"x");
    }

    #[test]
    fn flac_block_sizes() {
        assert_eq!(
            flac_block(&[0xFF, 0xF8, 0xC9, 0x18, 0x00, 0xC2]),
            Some(4096)
        );
        assert_eq!(
            flac_block(&[0xFF, 0xF8, 0x79, 0x18, 0x00, 0x01, 0x00]),
            Some(257)
        );
    }

    #[test]
    fn ogm_wave_tags() {
        assert_eq!(ogm_wave_tag(b"2000"), Some(0x2000));
        assert_eq!(ogm_wave_tag(b"55\0\0"), Some(0x55));
        assert_eq!(ogm_wave_tag(b"1\0\0\0"), Some(1));
        assert_eq!(ogm_wave_tag(b"\0\0\0\0"), None);
        assert_eq!(ogm_wave_tag(b"zz\0\0"), None);
    }

    #[test]
    fn xiph_lacing_round_trips() {
        let h = vec![vec![1u8; 300], vec![2u8; 3], vec![3u8; 10]];
        let l = xiph_lace(&h);
        let [a, b, c] = specular_vorbis::split_xiph_laced(&l).unwrap();
        assert_eq!((a.len(), b.len(), c.len()), (300, 3, 10));
    }
}
