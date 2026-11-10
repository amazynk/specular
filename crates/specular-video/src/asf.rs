//! ASF (`.wmv`, `.asf`; Windows Media) into the same [`Movie`] the MP4 demuxer gives,
//! from the published ASF specification (revision 01.20.05).
//!
//! - The header object: file properties (packet size, preroll, duration), stream properties
//!   (BITMAPINFOHEADER video, WAVEFORMATEX sound), stream properties inside extended stream
//!   properties of the header extension; content encryption objects mean DRM, refused.
//! - DVR-MS (Media Center's older recordings): binary-media streams carrying a DirectShow
//!   media type ([`crate::dshow`]: MPEG-2 video, MPEG audio, AC-3), and MPEG-2 video named
//!   `DVR `; encrypted recordings are refused as copy-protected TV.
//! - The data object: fixed-size packets of payloads. A payload is a piece of a media object
//!   (a coded frame) of one stream; compressed payloads carry several whole small objects.
//!   Media objects are the samples, timed by their presentation times (ms, minus preroll).
//! - A media object split over payloads is gathered when read ([`AsfLayout::read`]): the
//!   sample's offset is its first piece's, its packet is found from the fixed packet size.

use std::collections::HashMap;

use crate::mp4::{
    AudioTrack, Codec, Edit, Error, Layout, MAX_SAMPLES, Movie, Result, Sample, VideoTrack,
};

fn bad(s: impl Into<String>) -> Error {
    Error::Malformed(s.into())
}

/// A GUID as stored (little-endian first three fields), from its text form.
pub(crate) const fn guid(s: &[u8; 36]) -> [u8; 16] {
    const fn hex(c: u8) -> u8 {
        match c {
            b'0'..=b'9' => c - b'0',
            b'A'..=b'F' => c - b'A' + 10,
            _ => c - b'a' + 10,
        }
    }
    // Text digit pairs in stored byte order.
    const ORDER: [usize; 16] = [6, 4, 2, 0, 11, 9, 16, 14, 19, 21, 24, 26, 28, 30, 32, 34];
    let mut g = [0u8; 16];
    let mut i = 0;
    while i < 16 {
        g[i] = hex(s[ORDER[i]]) << 4 | hex(s[ORDER[i] + 1]);
        i += 1;
    }
    g
}

const HEADER: [u8; 16] = guid(b"75B22630-668E-11CF-A6D9-00AA0062CE6C");
const DATA: [u8; 16] = guid(b"75B22636-668E-11CF-A6D9-00AA0062CE6C");
const FILE_PROPERTIES: [u8; 16] = guid(b"8CABDCA1-A947-11CF-8EE4-00C00C205365");
const STREAM_PROPERTIES: [u8; 16] = guid(b"B7DC0791-A9B7-11CF-8EE6-00C00C205365");
const HEADER_EXTENSION: [u8; 16] = guid(b"5FBF03B5-A92E-11CF-8EE3-00C00C205365");
const EXT_STREAM_PROPERTIES: [u8; 16] = guid(b"14E6A5CB-C672-4332-8399-A96952065B5A");
const CONTENT_DESCRIPTION: [u8; 16] = guid(b"75B22633-668E-11CF-A6D9-00AA0062CE6C");
const CONTENT_ENCRYPTION: [u8; 16] = guid(b"2211B3FB-BD23-11D2-B4B7-00A0C955FC6E");
const EXT_CONTENT_ENCRYPTION: [u8; 16] = guid(b"298AE614-2622-4C17-B935-DAE07EE9289C");
const VIDEO_MEDIA: [u8; 16] = guid(b"BC19EFC0-5B4D-11CF-A8FD-00805F5C442B");
const AUDIO_MEDIA: [u8; 16] = guid(b"F8699E40-5B4D-11CF-A8FD-00805F5C442B");
/// DVR-MS streams: a DirectShow media type in the type-specific data (§9.4).
const BINARY_MEDIA: [u8; 16] = guid(b"3AFB65E2-47EF-40F2-AC2C-70A90D71D343");

fn u16le(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(o..o + 2)?.try_into().ok()?))
}

fn u32le(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

fn u64le(d: &[u8], o: usize) -> Option<u64> {
    Some(u64::from_le_bytes(d.get(o..o + 8)?.try_into().ok()?))
}

/// Whether `head` starts an ASF file.
pub fn sniff(head: &[u8]) -> bool {
    head.len() >= 30 && head[..16] == HEADER
}

/// The Content Description object's title, author, copyright and description (UTF-16LE,
/// five lengths first; the rating is left out), as (name, text).
fn content_description(b: &[u8]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut at = 10usize;
    for (i, name) in ["Title", "Author", "Copyright", "Description"]
        .iter()
        .enumerate()
    {
        let len = b
            .get(2 * i..2 * i + 2)
            .map_or(0, |l| usize::from(u16::from_le_bytes([l[0], l[1]])));
        let Some(t) = b.get(at..at + len) else { break };
        at += len;
        let units: Vec<u16> = t
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let s = String::from_utf16_lossy(&units)
            .trim_matches(char::from(0))
            .trim()
            .to_owned();
        if !s.is_empty() {
            out.push(((*name).to_owned(), s));
        }
    }
    out
}

#[cfg(test)]
#[test]
fn content_description_strings() {
    let w = |s: &str| -> Vec<u8> {
        s.encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect()
    };
    let (t, a) = (w("Trailer"), w("Hawks"));
    let mut b = Vec::new();
    for l in [t.len(), a.len(), 0, 0, 0] {
        b.extend((l as u16).to_le_bytes());
    }
    b.extend(t);
    b.extend(a);
    assert_eq!(
        content_description(&b),
        [
            ("Title".to_owned(), "Trailer".to_owned()),
            ("Author".to_owned(), "Hawks".to_owned())
        ]
    );
    for n in 0..b.len() {
        let _ = content_description(&b[..n]);
    }
}

/// Objects (GUID, body range) within `from..to`.
fn objects(d: &[u8], from: usize, to: usize) -> Vec<([u8; 16], usize, usize)> {
    let to = to.min(d.len());
    let mut out = Vec::new();
    let mut at = from;
    while at + 24 <= to && out.len() < 4096 {
        let id: [u8; 16] = d[at..at + 16].try_into().unwrap_or_default();
        let size = u64le(d, at + 16).unwrap_or(0) as usize;
        if size < 24 {
            break;
        }
        let end = at.saturating_add(size).min(to);
        out.push((id, at + 24, end));
        at = at.saturating_add(size);
    }
    out
}

/// Where an ASF track's samples are: fixed-size packets from `data_start`, the stream's
/// number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AsfLayout {
    pub data_start: u64,
    pub packet_size: u32,
    pub stream: u8,
    /// The stream is one elementary stream cut into pictures here (DVR-MS MPEG-2): a
    /// sample starts anywhere in a piece and runs on through the stream's next pieces,
    /// whatever their media objects.
    pub es: bool,
}

/// One payload (or one sub-payload of a compressed payload).
#[derive(Clone, Copy, Debug)]
struct Payload {
    stream: u8,
    key: bool,
    object: u32,
    /// Offset of these bytes within the media object.
    offset: u32,
    object_size: u32,
    pts: u32,
    data: usize,
    len: usize,
}

/// A field of `ty` (0 absent, 1 byte, 2 word, 3 dword) at `*p`.
fn field(d: &[u8], p: &mut usize, ty: u8) -> Option<u32> {
    let v = match ty {
        0 => 0,
        1 => u32::from(*d.get(*p)?),
        2 => u32::from(u16le(d, *p)?),
        _ => u32le(d, *p)?,
    };
    *p += [0, 1, 2, 4][usize::from(ty & 3)];
    Some(v)
}

/// The payloads of the packet at `at` (`size` bytes).
fn packet(d: &[u8], at: usize, size: usize, out: &mut Vec<Payload>) -> Option<()> {
    let end = at.checked_add(size)?.min(d.len());
    let pk = d.get(at..end)?;
    let mut p = 0usize;
    let mut flags = *pk.first()?;
    if flags & 0x80 != 0 {
        // Error correction data, then the real length type flags.
        if flags & 0x60 != 0 {
            return None;
        }
        p = 1 + usize::from(flags & 0x0F);
        flags = *pk.get(p)?;
    }
    p += 1;
    let props = *pk.get(p)?;
    p += 1;
    let multiple = flags & 1 != 0;
    let packet_len = field(pk, &mut p, (flags >> 5) & 3)? as usize;
    field(pk, &mut p, (flags >> 1) & 3)?;
    let padding = field(pk, &mut p, (flags >> 3) & 3)? as usize;
    p += 6; // send time, duration
    let len = if packet_len > 0 {
        packet_len.min(pk.len())
    } else {
        pk.len()
    };
    let (rep_ty, off_ty, obj_ty, _stream_ty) = (
        props & 3,
        (props >> 2) & 3,
        (props >> 4) & 3,
        (props >> 6) & 3,
    );
    let (count, len_ty) = if multiple {
        let b = *pk.get(p)?;
        p += 1;
        (usize::from(b & 0x3F), b >> 6)
    } else {
        (1, 0)
    };
    for _ in 0..count {
        let s = *pk.get(p)?;
        p += 1;
        let object = field(pk, &mut p, obj_ty)?;
        let offset = field(pk, &mut p, off_ty)?;
        let rep = field(pk, &mut p, rep_ty)? as usize;
        let rep_at = p;
        p = p.checked_add(rep)?;
        let body_len = if multiple {
            field(pk, &mut p, len_ty)? as usize
        } else {
            len.checked_sub(p)?.saturating_sub(padding)
        };
        let body_end = p.checked_add(body_len)?;
        if body_end > pk.len() {
            return None;
        }
        let stream = s & 0x7F;
        let key = s & 0x80 != 0;
        if rep == 1 {
            // Compressed: `offset` is the presentation time; sub-payloads of one-byte lengths.
            let mut q = p;
            let mut n = 0u32;
            while q < body_end {
                let l = usize::from(pk[q]);
                q += 1;
                if q + l > body_end {
                    break;
                }
                out.push(Payload {
                    stream,
                    key,
                    object: object.wrapping_add(n),
                    offset: 0,
                    object_size: l as u32,
                    pts: offset,
                    data: at + q,
                    len: l,
                });
                q += l;
                n += 1;
            }
        } else {
            let (object_size, pts) = if rep >= 8 {
                (u32le(pk, rep_at)?, u32le(pk, rep_at + 4)?)
            } else {
                (body_len as u32, 0)
            };
            out.push(Payload {
                stream,
                key,
                object,
                offset,
                object_size,
                pts,
                data: at + p,
                len: body_len,
            });
        }
        p = body_end;
    }
    Some(())
}

impl AsfLayout {
    fn packet_of(&self, offset: u64) -> Option<usize> {
        let ps = u64::from(self.packet_size);
        let rel = offset.checked_sub(self.data_start)?;
        usize::try_from(self.data_start + rel / ps * ps).ok()
    }

    /// The bytes of sample `s`: its first piece in place, or its pieces gathered.
    pub fn read<'a>(&self, data: &'a [u8], s: &Sample) -> Option<std::borrow::Cow<'a, [u8]>> {
        if self.es {
            return self.read_es(data, s);
        }
        let size = s.size as usize;
        let start = usize::try_from(s.offset).ok()?;
        let mut at = self.packet_of(s.offset)?;
        let ps = self.packet_size as usize;
        let mut pays = Vec::new();
        packet(data, at, ps, &mut pays)?;
        let first = *pays
            .iter()
            .find(|p| p.stream == self.stream && p.data == start)?;
        if first.len >= size {
            return data
                .get(start..start + size)
                .map(std::borrow::Cow::Borrowed);
        }
        let mut out = Vec::with_capacity(size);
        out.extend_from_slice(data.get(start..start + first.len)?);
        // The other pieces follow, in this packet and the next ones.
        let mut seen_first = false;
        for _ in 0..4096 {
            for p in &pays {
                if p.data == start {
                    seen_first = true;
                    continue;
                }
                if seen_first
                    && p.stream == self.stream
                    && p.object == first.object
                    && p.offset as usize == out.len()
                {
                    let take = p.len.min(size - out.len());
                    out.extend_from_slice(data.get(p.data..p.data + take)?);
                }
            }
            if out.len() >= size {
                return Some(std::borrow::Cow::Owned(out));
            }
            at += ps;
            pays.clear();
            if packet(data, at, ps, &mut pays).is_none() {
                break;
            }
        }
        None
    }
}

impl AsfLayout {
    /// [`AsfLayout::read`] of an elementary-stream track.
    fn read_es<'a>(&self, data: &'a [u8], s: &Sample) -> Option<std::borrow::Cow<'a, [u8]>> {
        let size = s.size as usize;
        let start = usize::try_from(s.offset).ok()?;
        let mut at = self.packet_of(s.offset)?;
        let ps = self.packet_size as usize;
        let mut pays = Vec::new();
        let mut out: Vec<u8> = Vec::new();
        let mut begun = false;
        for _ in 0..1 << 16 {
            pays.clear();
            packet(data, at, ps, &mut pays)?;
            for p in pays.iter().filter(|p| p.stream == self.stream) {
                let from = if begun {
                    p.data
                } else if (p.data..p.data + p.len).contains(&start) {
                    begun = true;
                    start
                } else {
                    continue;
                };
                let take = (p.data + p.len - from).min(size - out.len());
                if out.is_empty() && take == size {
                    return data.get(from..from + take).map(std::borrow::Cow::Borrowed);
                }
                out.extend_from_slice(data.get(from..from + take)?);
                if out.len() >= size {
                    return Some(std::borrow::Cow::Owned(out));
                }
            }
            if !begun {
                return None;
            }
            at += ps;
        }
        None
    }
}

/// The pictures of an MPEG-1/2 elementary stream carried in `pieces` (file offset, length)
/// in order: a sample per picture (a field pair joined), from the first I picture with a
/// sequence header; timed from the frame rate and the temporal references, in 90 kHz ticks.
fn es_pictures(
    data: &[u8],
    pieces: &[(usize, usize)],
    mut config: Vec<u8>,
    origin: i64,
) -> (Vec<Sample>, Vec<u8>) {
    // Where each picture's headers begin, as positions in the stream.
    let mut starts: Vec<u64> = Vec::new();
    let mut have_picture = false;
    let mut pos = 0u64;
    let mut last = [0xFFu8; 3];
    let mut piece_at: Vec<u64> = Vec::with_capacity(pieces.len());
    for &(off, len) in pieces {
        piece_at.push(pos);
        let Some(b) = data.get(off..off + len) else {
            break;
        };
        for &x in b {
            if last == [0, 0, 1] && matches!(x, 0x00 | 0xB3 | 0xB8) {
                let code_at = pos - 3;
                if x == 0 {
                    if !have_picture && starts.is_empty() {
                        starts.push(code_at);
                    }
                    if have_picture {
                        starts.push(code_at);
                    }
                    have_picture = true;
                } else if have_picture || starts.is_empty() {
                    starts.push(code_at);
                    have_picture = false;
                }
            }
            last = [last[1], last[2], x];
            pos += 1;
        }
        if starts.len() > MAX_SAMPLES {
            break;
        }
    }
    let total = pos;
    starts.dedup();
    let file_at = |p: u64| -> Option<u64> {
        let k = piece_at.partition_point(|&s| s <= p).checked_sub(1)?;
        Some(pieces.get(k)?.0 as u64 + (p - piece_at[k]))
    };
    // The bytes from stream position p (up to n), for headers.
    let read = |p: u64, n: usize| -> Vec<u8> {
        let mut out = Vec::new();
        let Some(mut k) = piece_at.partition_point(|&s| s <= p).checked_sub(1) else {
            return out;
        };
        let mut within = (p - piece_at[k]) as usize;
        while out.len() < n && k < pieces.len() {
            let (off, len) = pieces[k];
            let b = data.get(off + within..off + len).unwrap_or_default();
            out.extend_from_slice(&b[..b.len().min(n - out.len())]);
            k += 1;
            within = 0;
        }
        out
    };
    struct Pic {
        at: u64,
        size: u64,
        kind: Option<specular_mpeg2::PictureType>,
        tr: u16,
        gop: bool,
    }
    let mut pics: Vec<Pic> = Vec::new();
    let mut started = false;
    let mut open_field: Option<specular_mpeg2::Structure> = None;
    for (i, &s) in starts.iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(total);
        let head = read(s, (end - s).min(4096) as usize);
        let sc = specular_mpeg2::scan(&head);
        if config.is_empty()
            && let Some(b) = specular_mpeg2::sequence_bytes(&head)
        {
            config = b.to_vec();
        }
        let Some(info) = sc.pictures.first().copied() else {
            continue;
        };
        let frame = specular_mpeg2::Structure::Frame;
        if let (Some(o), Some(last)) = (open_field, pics.last_mut())
            && info.structure != frame
            && info.structure != o
        {
            last.size = end - last.at;
            open_field = None;
            continue;
        }
        open_field = (info.structure != frame).then_some(info.structure);
        if !started {
            if info.kind == specular_mpeg2::PictureType::I && !config.is_empty() {
                started = true;
            } else {
                continue;
            }
        }
        pics.push(Pic {
            at: s,
            size: end - s,
            kind: Some(info.kind),
            tr: info.temporal_reference,
            gop: sc.gop || sc.sequence.is_some(),
        });
    }
    let frame_ticks = specular_mpeg2::probe(&config)
        .and_then(|s| s.frame_rate())
        .map_or(3600, |r| (90_000.0 / r).round() as i64);
    let mut list = Vec::with_capacity(pics.len());
    let (mut base, mut most) = (origin, -1i64);
    for (i, p) in pics.iter().enumerate() {
        if p.gop && i > 0 {
            base += (most + 1) * frame_ticks;
            most = -1;
        }
        most = most.max(i64::from(p.tr));
        let Some(off) = file_at(p.at) else { continue };
        list.push(Sample {
            offset: off,
            size: p.size.min(u64::from(u32::MAX)) as u32,
            dts: 0,
            cts: base + i64::from(p.tr) * frame_ticks,
            duration: frame_ticks as u32,
            sync: p.kind == Some(specular_mpeg2::PictureType::I),
        });
    }
    let mut order: Vec<i64> = list.iter().map(|s| s.cts).collect();
    order.sort_unstable();
    for (s, t) in list.iter_mut().zip(order) {
        s.dts = t.max(0) as u64;
    }
    (list, config)
}

/// A stream's description.
struct Stream {
    number: u8,
    video: bool,
    /// BITMAPINFOHEADER or WAVEFORMATEX.
    format: Vec<u8>,
    width: u32,
    height: u32,
    /// A binary-media stream's DirectShow type (DVR-MS).
    media: Option<crate::dshow::Media>,
}

fn stream_properties(d: &[u8], from: usize, to: usize) -> Option<Stream> {
    let b = d.get(from..to)?;
    let kind: [u8; 16] = b.get(0..16)?.try_into().ok()?;
    let ts_len = u32le(b, 40)? as usize;
    let flags = u16le(b, 48)?;
    let ts = b.get(54..54 + ts_len)?;
    let number = (flags & 0x7F) as u8;
    if kind == VIDEO_MEDIA {
        let fmt_len = usize::from(u16le(ts, 9)?);
        Some(Stream {
            number,
            video: true,
            format: ts.get(11..11 + fmt_len).unwrap_or(ts.get(11..)?).to_vec(),
            width: u32le(ts, 0)?,
            height: u32le(ts, 4)?,
            media: None,
        })
    } else if kind == AUDIO_MEDIA {
        Some(Stream {
            number,
            video: false,
            format: ts.to_vec(),
            width: 0,
            height: 0,
            media: None,
        })
    } else if kind == BINARY_MEDIA {
        let m = crate::dshow::parse(ts)?;
        let (video, width, height) = match &m {
            crate::dshow::Media::Video { width, height, .. } => (true, *width, *height),
            crate::dshow::Media::Audio { .. } => (false, 0, 0),
            crate::dshow::Media::Other => return None,
        };
        Some(Stream {
            number,
            video,
            format: Vec::new(),
            width,
            height,
            media: Some(m),
        })
    } else {
        None
    }
}

/// The stream properties object an extended stream properties object may end with.
fn ext_stream(d: &[u8], from: usize, to: usize) -> Option<Stream> {
    let b = d.get(from..to)?;
    let names = usize::from(u16le(b, 60)?);
    let systems = usize::from(u16le(b, 62)?);
    let mut p = 64;
    for _ in 0..names {
        p += 4 + usize::from(u16le(b, p + 2)?);
    }
    for _ in 0..systems {
        p += 22 + u32le(b, p + 18)? as usize;
    }
    let o = objects(d, from + p, to);
    let (_, f, t) = o.into_iter().find(|o| o.0 == STREAM_PROPERTIES)?;
    stream_properties(d, f, t)
}

/// A media object being collected.
struct Object {
    offset: usize,
    size: u32,
    pts: u32,
    key: bool,
    got: u32,
}

/// Parses an ASF file. With `samples`, every media object is listed.
pub fn parse(data: &[u8], samples: bool) -> Result<Movie> {
    if !sniff(data) {
        return Err(bad("not an ASF file"));
    }
    let header_size = u64le(data, 16).unwrap_or(0) as usize;
    let mut streams = Vec::new();
    let (mut packet_size, mut preroll, mut duration_100ns) = (0u32, 0u64, 0u64);
    let mut drm = false;
    let mut tags = Vec::new();
    for (id, from, to) in objects(data, 30, header_size) {
        if id == FILE_PROPERTIES {
            let b = &data[from..to];
            duration_100ns = u64le(b, 40).unwrap_or(0);
            preroll = u64le(b, 56).unwrap_or(0);
            let (min, max) = (u32le(b, 68).unwrap_or(0), u32le(b, 72).unwrap_or(0));
            if min == max {
                packet_size = min;
            }
        } else if id == STREAM_PROPERTIES {
            streams.extend(stream_properties(data, from, to));
        } else if id == HEADER_EXTENSION {
            for (eid, ef, et) in objects(data, from + 22, to) {
                if eid == EXT_STREAM_PROPERTIES {
                    streams.extend(ext_stream(data, ef, et));
                }
            }
        } else if id == CONTENT_DESCRIPTION {
            tags = content_description(&data[from..to]);
        } else if id == CONTENT_ENCRYPTION || id == EXT_CONTENT_ENCRYPTION {
            drm = true;
        }
    }
    if drm && streams.iter().any(|s| s.media.is_some()) {
        return Err(Error::Unsupported("copy-protected TV recording".into()));
    }
    if drm {
        return Err(Error::Unsupported(
            "copy-protected (DRM) Windows Media".into(),
        ));
    }
    if packet_size < 16 {
        return Err(Error::Unsupported(
            "ASF with packets of varying size".into(),
        ));
    }
    let (_, data_from, data_to) = objects(data, header_size, data.len())
        .into_iter()
        .find(|o| o.0 == DATA)
        .ok_or_else(|| bad("no ASF data object"))?;
    let first_packet = data_from + 26;
    let video = streams.iter().find(|s| s.video);
    let audio = streams.iter().find(|s| !s.video);
    let wanted = [video.map(|s| s.number), audio.map(|s| s.number)];
    // Media objects per stream, in order of their first piece.
    let mut lists: [Vec<Object>; 2] = Default::default();
    let mut open: [HashMap<u32, usize>; 2] = Default::default();
    let mut pays = Vec::new();
    let ps = packet_size as usize;
    // DVR-MS MPEG-2 arrives in arbitrary pieces: kept as one stream to cut at pictures.
    let es_video = video.is_some_and(|s| {
        matches!(
            s.media,
            Some(crate::dshow::Media::Video {
                codec: Codec::Mpeg2 | Codec::Mpeg1,
                ..
            })
        ) || s.format.get(16..20) == Some(b"DVR ")
    });
    let mut es_pieces: Vec<(usize, usize)> = Vec::new();
    let mut es_first: Option<u32> = None;
    let mut at = first_packet;
    while at + 16 <= data_to.min(data.len()) {
        pays.clear();
        let _ = packet(data, at, ps, &mut pays);
        for p in &pays {
            let Some(k) = wanted.iter().position(|w| *w == Some(p.stream)) else {
                continue;
            };
            if es_video && k == 0 {
                es_first.get_or_insert(p.pts);
                es_pieces.push((p.data, p.len));
                continue;
            }
            if p.offset == 0 {
                // A new object (a repeated number starts one too: numbers wrap).
                open[k].insert(p.object, lists[k].len());
                lists[k].push(Object {
                    offset: p.data,
                    size: p.object_size,
                    pts: p.pts,
                    key: p.key,
                    got: p.len as u32,
                });
            } else if let Some(&i) = open[k].get(&p.object) {
                lists[k][i].got += p.len as u32;
            }
        }
        if lists[0].len() + lists[1].len() + es_pieces.len() / 16 > 2 * MAX_SAMPLES {
            return Err(Error::Unsupported("over a million media objects".into()));
        }
        at += ps;
    }
    let layout = |stream: u8| AsfLayout {
        data_start: first_packet as u64,
        packet_size,
        stream,
        es: false,
    };
    let to_samples = |objs: &[Object]| -> Vec<Sample> {
        let mut list: Vec<Sample> = objs
            .iter()
            .filter(|o| o.got >= o.size && o.size > 0)
            .map(|o| {
                let t = (i64::from(o.pts) - preroll as i64).max(0);
                Sample {
                    offset: o.offset as u64,
                    size: o.size,
                    dts: t as u64,
                    cts: t,
                    duration: 0,
                    sync: o.key,
                }
            })
            .collect();
        // Each lasts until the next in presentation order.
        let mut order: Vec<i64> = list.iter().map(|s| s.cts).collect();
        order.sort_unstable();
        order.dedup();
        let typical = order
            .windows(2)
            .map(|w| w[1] - w[0])
            .filter(|&d| d > 0)
            .min()
            .unwrap_or(40);
        for s in &mut list {
            let k = order.partition_point(|&c| c <= s.cts);
            s.duration = order
                .get(k)
                .map_or(typical, |n| n - s.cts)
                .clamp(0, i64::from(u32::MAX)) as u32;
        }
        // Decode order is file order; decode times must not fall behind presentation.
        let mut dts: Vec<u64> = list.iter().map(|s| s.dts).collect();
        dts.sort_unstable();
        for (s, d) in list.iter_mut().zip(dts) {
            s.dts = d;
        }
        list
    };
    let mut movie = Movie {
        brand: *b"ASF ",
        tags,
        ..Movie::default()
    };
    if let Some(s) = video {
        let mut list = to_samples(&lists[0]);
        if !list.iter().any(|x| x.sync)
            && let Some(x) = list.first_mut()
        {
            x.sync = true;
        }
        let f = &s.format;
        let compression: [u8; 4] = f
            .get(16..20)
            .and_then(|c| c.try_into().ok())
            .unwrap_or(*b"????");
        let (w, h) = (
            u32le(f, 4).unwrap_or(s.width),
            (u32le(f, 8).unwrap_or(s.height) as i32).unsigned_abs(),
        );
        let duration = list
            .iter()
            .map(|x| x.cts.max(0) as u64 + u64::from(x.duration))
            .max()
            .unwrap_or(0);
        let (codec, mut config, w, h) = match &s.media {
            Some(crate::dshow::Media::Video {
                codec,
                config,
                width,
                height,
            }) => (*codec, config.clone(), *width, *height),
            // DVR-MS names its MPEG-2 video "DVR ".
            _ if &compression == b"DVR " => (Codec::Mpeg2, Vec::new(), w, h),
            _ => {
                let codec = crate::avi::fourcc_codec(compression);
                (codec, crate::avi::bitmap_config(codec, f), w, h)
            }
        };
        let mut track = VideoTrack {
            codec: Some(codec),
            config: Vec::new(),
            width: w,
            height: h,
            display: (w, h),
            timescale: 1000,
            duration,
            sample_count: list.len() as u32,
            samples: Vec::new(),
            layout: Layout::Asf(layout(s.number)),
            edit: Edit::default(),
            // Media objects carry their decoding times: B-frames are shown in the slots, as
            // in AVI.
            slot_timing: true,
            ..VideoTrack::default()
        };
        if es_video {
            // On the sound's timeline: the stream's first presentation time (ms).
            let origin = es_first.map_or(0, |t| (i64::from(t) - preroll as i64).max(0) * 90);
            let (pics, cfg) = es_pictures(data, &es_pieces, std::mem::take(&mut config), origin);
            list = pics;
            config = cfg;
            track.timescale = 90_000;
            track.slot_timing = false;
            track.layout = Layout::Asf(AsfLayout {
                es: true,
                ..layout(s.number)
            });
            track.duration = list
                .iter()
                .map(|x| x.cts.max(0) as u64 + u64::from(x.duration))
                .max()
                .unwrap_or(0);
            track.sample_count = list.len() as u32;
        }
        track.config = config;
        if samples {
            track.samples = list;
        }
        movie.video = Some(track);
    }
    let sound = audio.and_then(|s| match &s.media {
        Some(crate::dshow::Media::Audio {
            codec,
            channels,
            rate,
        }) => Some((s, (codec.clone(), *channels, *rate))),
        Some(_) => None,
        None => crate::avi::wave_codec(&s.format).map(|c| (s, c)),
    });
    if let Some((s, (codec, channels, rate))) = sound {
        let list = to_samples(&lists[1]);
        let duration = list
            .iter()
            .map(|x| x.cts.max(0) as u64 + u64::from(x.duration))
            .max()
            .unwrap_or(0);
        movie.audio = Some(AudioTrack {
            codec,
            channels,
            sample_rate: rate,
            timescale: 1000,
            duration,
            sample_count: list.len() as u32,
            samples: if samples { list } else { Vec::new() },
            edit: Edit::default(),
            layout: Layout::Asf(layout(s.number)),
            extra: Default::default(),
        });
        movie.has_audio = true;
    }
    if movie.video.is_none() && movie.audio.is_none() {
        return Err(Error::Unsupported("no video or sound in the stream".into()));
    }
    let play_us = (duration_100ns / 10).saturating_sub(preroll * 1000);
    let vd = movie.video.as_ref().and_then(|t| t.duration_us());
    movie.duration_us = vd.or((play_us > 0).then_some(play_us));
    Ok(movie)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guids_are_stored_little_endian() {
        assert_eq!(
            HEADER,
            [
                0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62,
                0xCE, 0x6C
            ]
        );
    }

    /// A packet of `size` bytes with multiple payloads (a padding-length byte, byte-sized
    /// payload fields, 8 bytes of
    /// replicated data each): (stream, key, object, offset, object size, pts, bytes).
    type Pay<'a> = (u8, bool, u8, u8, u32, u32, &'a [u8]);

    fn pk(size: usize, pays: &[Pay]) -> Vec<u8> {
        let mut v = vec![0x82, 0, 0, 0x09, 0x55];
        // packet length absent (0), sequence absent, padding byte
        let pad_at = v.len();
        v.push(0);
        v.extend_from_slice(&[0; 6]);
        v.push(0x80 | pays.len() as u8); // payload length type 2 (word)
        for &(s, k, o, off, osz, pts, b) in pays {
            v.push(s | if k { 0x80 } else { 0 });
            v.push(o);
            v.push(off);
            v.push(8);
            v.extend_from_slice(&osz.to_le_bytes());
            v.extend_from_slice(&pts.to_le_bytes());
            v.extend_from_slice(&(b.len() as u16).to_le_bytes());
            v.extend_from_slice(b);
        }
        let pad = size - v.len();
        v[pad_at] = pad as u8;
        v.resize(size, 0);
        v
    }

    #[test]
    fn split_objects_are_gathered() {
        let p1 = pk(64, &[(1, true, 0, 0, 10, 100, b"abcdef")]);
        let p2 = pk(
            64,
            &[
                (1, true, 0, 6, 10, 100, b"ghij"),
                (1, false, 1, 0, 3, 140, b"xyz"),
            ],
        );
        let data = [p1, p2].concat();
        let lay = AsfLayout {
            data_start: 0,
            packet_size: 64,
            stream: 1,
            es: false,
        };
        let mut pays = Vec::new();
        packet(&data, 0, 64, &mut pays).unwrap();
        assert_eq!(pays.len(), 1);
        let s = Sample {
            offset: pays[0].data as u64,
            size: 10,
            dts: 0,
            cts: 0,
            duration: 0,
            sync: true,
        };
        assert_eq!(&*lay.read(&data, &s).unwrap(), b"abcdefghij");
        pays.clear();
        packet(&data, 64, 64, &mut pays).unwrap();
        let s2 = Sample {
            offset: pays[1].data as u64,
            size: 3,
            ..s
        };
        assert_eq!(&*lay.read(&data, &s2).unwrap(), b"xyz");
    }
}
