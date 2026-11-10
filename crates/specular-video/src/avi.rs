//! AVI (RIFF) movies, OpenDML (files over 1 GB, `AVIX` parts) included, into the same
//! [`Movie`] the MP4 demuxer gives.
//!
//! - The header list (`hdrl`): the main header, and per stream its header (`strh`) and format
//!   (`strf`: BITMAPINFOHEADER, WAVEFORMATEX).
//! - Samples from the OpenDML indexes (`indx` → `ix##`), else the legacy `idx1`, else a walk
//!   over the `movi` lists. Zero-sized video chunks (dropped frames) are left out: the frame
//!   before them lasts.
//! - Video times are chunk counts (`dwScale`/`dwRate`); AVI has no presentation times, so a
//!   codec with B-frames shows its pictures in display order in the chunks' time slots
//!   ([`VideoTrack::slot_timing`]).
//! - Sound: PCM, MPEG audio (Layers I–III), AC-3, AAC; times from the byte position for
//!   constant-rate streams, else from the chunk count.

use crate::mp4::{
    AdpcmKind, AudioCodec, AudioTrack, Codec, Edit, Error, Layout, MAX_SAMPLES, Movie, Result,
    Sample, VideoTrack,
};

fn bad(s: impl Into<String>) -> Error {
    Error::Malformed(s.into())
}

/// The video coding of a BITMAPINFOHEADER compression code (AVI `strh`/`strf`, Matroska
/// `V_MS/VFW/FOURCC`).
pub fn fourcc_codec(c: [u8; 4]) -> Codec {
    // Raw layouts and the old codecs first: their codes are case-sensitive (`yuvs`, `YUVS`).
    if let Some(codec) = crate::rawvid::codec_of(c) {
        return codec;
    }
    if let Some(k) = specular_hap::kind_of_fourcc(&c) {
        return Codec::Hap(k);
    }
    let up = c.map(|b| b.to_ascii_uppercase());
    match &up {
        b"FFV1" => Codec::Ffv1,
        b"CFHD" => Codec::CineForm,
        b"DRAC" => Codec::Vc2,
        b"DIVX" | b"DX50" | b"XVID" | b"FMP4" | b"MP4V" | b"3IV2" | b"M4S2" | b"MP4S" | b"RMP4"
        | b"SEDG" | b"WV1F" | b"BLZ0" | b"DXGM" | b"FVFW" | b"GEOV" | b"HDX4" | b"SMP4"
        | b"UMP4" | b"XVIX" | b"DM4V" | b"LMP4" | b"NDIG" | b"QMP4" | b"EM4A" | b"PVMM" => {
            Codec::Mpeg4
        }
        b"MPG1" => Codec::Mpeg1,
        b"MPG2" | b"MPEG" | b"MMES" | b"M2V1" | b"EM2V" | b"PIM2" => Codec::Mpeg2,
        b"H263" | b"S263" | b"U263" | b"I263" | b"X263" | b"M263" => Codec::H263,
        b"H264" | b"X264" | b"AVC1" | b"DAVC" | b"VSSH" => Codec::H264,
        b"HEVC" | b"H265" | b"HVC1" => Codec::Hevc,
        b"VP80" => Codec::Vp8,
        b"VP90" => Codec::Vp9,
        b"AV01" => Codec::Av1,
        // Microsoft's MPEG-4 variants (and DivX 3) are not MPEG-4 Part 2 bitstreams.
        b"MP42" | b"MP43" | b"DIV3" | b"DIV4" | b"MPG4" | b"AP41" | b"COL1" => {
            Codec::Other(*b"MP43")
        }
        b"MJPG" | b"AVRN" | b"AVDJ" | b"JPEG" | b"DMB1" | b"IJPG" | b"LJPG" | b"JPGL" => {
            Codec::Mjpeg
        }
        b"MJ2C" | b"MJP2" | b"LJ2C" | b"LJ2K" => Codec::Jpeg2000,
        b"DVSD" | b"DV25" | b"DV50" | b"DVHD" | b"CDVC" | b"CDVH" | b"DVCS" | b"DVC " | b"DVSL"
        | b"DVH1" | b"DVIS" | b"PDVC" | b"SL25" | b"SLDV" | b"AVD1" => Codec::Dv,
        b"WMV3" | b"WVC1" | b"WMVA" => Codec::Vc1,
        b"WMV1" | b"WMV2" => Codec::Other(*b"WMV2"),
        b"VP30" | b"VP31" | b"VP32" => Codec::Vp3,
        b"FLV1" => Codec::Flv1,
        _ => Codec::Other(c),
    }
}

/// The track configuration of a BITMAPINFOHEADER (AVI `strf`, ASF, Matroska VFW): the codec's
/// own bytes after the header, or for raw video and the old codecs a
/// [`crate::rawvid::PixDesc`] (depth, row order, colour table).
pub fn bitmap_config(codec: Codec, bih: &[u8]) -> Vec<u8> {
    if crate::rawvid::described(codec) {
        crate::rawvid::PixDesc::from_bitmap_info(bih).to_config()
    } else {
        bih.get(40..).unwrap_or_default().to_vec()
    }
}

fn u16le(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(o..o + 2)?.try_into().ok()?))
}

fn u32le(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

fn u64le(d: &[u8], o: usize) -> Option<u64> {
    Some(u64::from_le_bytes(d.get(o..o + 8)?.try_into().ok()?))
}

/// A RIFF chunk: its id, the data range, and for lists the list type.
#[derive(Clone, Copy, Debug)]
struct Chunk {
    id: [u8; 4],
    data: usize,
    end: usize,
}

/// The chunks within `from..to` (`to` capped at the data).
fn chunks(d: &[u8], from: usize, to: usize) -> impl Iterator<Item = Chunk> + '_ {
    let to = to.min(d.len());
    let mut at = from;
    std::iter::from_fn(move || {
        if at + 8 > to {
            return None;
        }
        let id: [u8; 4] = d[at..at + 4].try_into().ok()?;
        let size = u32le(d, at + 4)? as usize;
        let data = at + 8;
        let end = data.checked_add(size)?.min(to);
        at = data + size + (size & 1);
        Some(Chunk { id, data, end })
    })
}

/// Whether `head` starts an AVI file.
pub fn sniff(head: &[u8]) -> bool {
    head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"AVI "
}

/// One stream of the file.
#[derive(Default)]
struct Stream {
    kind: [u8; 4],
    handler: [u8; 4],
    scale: u32,
    rate: u32,
    start: u32,
    sample_size: u32,
    format: Vec<u8>,
    /// OpenDML super index: (offset, size) of each standard index chunk.
    indx: Vec<(u64, u32)>,
    samples: Vec<(u64, u32, bool)>,
}

/// The two-digit stream number of a chunk id (`00dc`, `01wb`, `ix00`).
fn stream_of(id: &[u8; 4], ix: bool) -> Option<usize> {
    let digits = if ix { &id[2..4] } else { &id[0..2] };
    let n = std::str::from_utf8(digits).ok()?;
    usize::from_str_radix(n, 16).ok()
}

/// Parses an AVI file. With `samples`, every chunk is listed (always the case: the index is
/// needed for the length).
pub fn parse(data: &[u8], samples: bool) -> Result<Movie> {
    if !sniff(data) {
        return Err(bad("not an AVI file"));
    }
    let mut streams: Vec<Stream> = Vec::new();
    let mut movi: Vec<(usize, usize)> = Vec::new();
    let mut idx1: Option<(usize, usize)> = None;
    let mut micro_per_frame = 0u32;
    // The RIFF parts: AVI, then AVIX ones (OpenDML).
    for part in chunks(data, 0, data.len()) {
        if &part.id != b"RIFF" || part.end < part.data + 4 {
            continue;
        }
        for c in chunks(data, part.data + 4, part.end) {
            match (&c.id, data.get(c.data..c.data + 4)) {
                (b"LIST", Some(b"hdrl")) => {
                    for h in chunks(data, c.data + 4, c.end) {
                        if &h.id == b"avih" {
                            micro_per_frame = u32le(data, h.data).unwrap_or(0);
                        }
                        if &h.id == b"LIST" && data.get(h.data..h.data + 4) == Some(b"strl") {
                            streams.push(stream_list(data, h.data + 4, h.end));
                        }
                    }
                }
                (b"LIST", Some(b"movi")) => movi.push((c.data + 4, c.end)),
                _ if &c.id == b"idx1" => idx1 = Some((c.data, c.end)),
                _ => {}
            }
        }
    }
    if streams.is_empty() {
        return Err(bad("no streams in the AVI header"));
    }
    if movi.is_empty() {
        return Err(bad("no movie data"));
    }
    // Samples: OpenDML indexes, else idx1, else the chunks themselves.
    let have_indx = streams.iter().any(|s| !s.indx.is_empty());
    if have_indx {
        // Each `ix##` chunk is read once (super-index entries may all name the same one),
        // and the chunk count is capped while reading, not after.
        let mut seen = std::collections::HashSet::new();
        let mut room = 4 * MAX_SAMPLES;
        for s in &mut streams {
            for &(off, _) in &s.indx.clone() {
                if seen.insert(off) && !read_ix(data, off as usize, &mut s.samples, &mut room) {
                    return Err(Error::Unsupported(format!(
                        "over {} chunks",
                        4 * MAX_SAMPLES
                    )));
                }
            }
        }
    }
    if !have_indx || streams.iter().all(|s| s.samples.is_empty()) {
        let listed =
            idx1.is_some_and(|(from, to)| read_idx1(data, from, to, movi[0].0, &mut streams));
        if !listed {
            for &(from, to) in &movi {
                walk_movi(data, from, to, &mut streams, 0);
            }
        }
    }
    let total: usize = streams.iter().map(|s| s.samples.len()).sum();
    if total > 4 * MAX_SAMPLES {
        return Err(Error::Unsupported(format!("{total} chunks")));
    }
    let mut movie = Movie {
        brand: *b"AVI ",
        ..Movie::default()
    };
    // Type-1 DV AVI: one interleaved `iavs` stream holds the DV frames, sound included.
    if let Some(s) = streams
        .iter()
        .find(|s| &s.kind == b"vids")
        .or_else(|| streams.iter().find(|s| &s.kind == b"iavs"))
    {
        movie.video = Some(video_track(data, s, micro_per_frame, samples));
    }
    if let Some(s) = streams.iter().find(|s| &s.kind == b"auds") {
        movie.audio = audio_track(s, samples);
        movie.has_audio = movie.audio.is_some();
    }
    let vd = movie.video.as_ref().and_then(|v| v.duration_us());
    let ad = movie
        .audio
        .as_ref()
        .and_then(|a| (a.timescale > 0).then(|| a.duration * 1_000_000 / u64::from(a.timescale)));
    movie.duration_us = vd.max(ad);
    if movie.video.is_none() && movie.audio.is_none() {
        return Err(Error::Unsupported("no video or sound in the stream".into()));
    }
    Ok(movie)
}

fn stream_list(d: &[u8], from: usize, to: usize) -> Stream {
    let mut s = Stream::default();
    for c in chunks(d, from, to) {
        match &c.id {
            b"strh" => {
                let b = &d[c.data..c.end];
                s.kind = b
                    .get(0..4)
                    .and_then(|x| x.try_into().ok())
                    .unwrap_or_default();
                s.handler = b
                    .get(4..8)
                    .and_then(|x| x.try_into().ok())
                    .unwrap_or_default();
                s.scale = u32le(b, 20).unwrap_or(1).max(1);
                s.rate = u32le(b, 24).unwrap_or(1).max(1);
                s.start = u32le(b, 28).unwrap_or(0);
                s.sample_size = u32le(b, 44).unwrap_or(0);
            }
            b"strf" => s.format = d[c.data..c.end].to_vec(),
            b"indx" => {
                // AVISUPERINDEX: longs per entry 4, sub type 0, type 0 (index of indexes).
                let b = &d[c.data..c.end];
                if b.get(3) == Some(&0) {
                    let n = u32le(b, 4).unwrap_or(0) as usize;
                    for k in 0..n.min(1 << 16) {
                        let o = 24 + 16 * k;
                        if let (Some(off), Some(size)) = (u64le(b, o), u32le(b, o + 8)) {
                            s.indx.push((off, size));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    s
}

/// An OpenDML standard index chunk at `at` (the `ix##` header), taking at most `room`
/// entries in all (false once that is used up).
fn read_ix(d: &[u8], at: usize, out: &mut Vec<(u64, u32, bool)>, room: &mut usize) -> bool {
    let Some(size) = u32le(d, at + 4) else {
        return true;
    };
    let b = match d.get(at + 8..(at + 8).saturating_add(size as usize)) {
        Some(b) => b,
        None => return true,
    };
    // wLongsPerEntry 2, bIndexSubType 0, bIndexType 1 (index of chunks).
    if b.get(3) != Some(&1) {
        return true;
    }
    let n = u32le(b, 4).unwrap_or(0) as usize;
    let base = u64le(b, 12).unwrap_or(0);
    for k in 0..n {
        let o = 24 + 8 * k;
        let (Some(off), Some(sz)) = (u32le(b, o), u32le(b, o + 4)) else {
            break;
        };
        let Some(left) = room.checked_sub(1) else {
            return false;
        };
        *room = left;
        out.push((
            base + u64::from(off),
            sz & 0x7FFF_FFFF,
            sz & 0x8000_0000 == 0,
        ));
    }
    true
}

/// The legacy index: entries relative to the `movi` list (or, in some files, absolute).
fn read_idx1(d: &[u8], from: usize, to: usize, movi_data: usize, streams: &mut [Stream]) -> bool {
    let entries: Vec<&[u8]> = d[from..to].chunks_exact(16).collect();
    if entries.is_empty() {
        return false;
    }
    // Offsets count from the 'movi' fourcc, or from the file start.
    let first = u32le(entries[0], 8).unwrap_or(0) as usize;
    let movi_fourcc = movi_data - 4;
    let base = if d.get(first..first + 4) == Some(&entries[0][..4]) {
        0
    } else {
        movi_fourcc
    };
    for e in entries {
        let id: [u8; 4] = e[..4].try_into().unwrap_or_default();
        let Some(n) = stream_of(&id, false) else {
            continue;
        };
        let Some(s) = streams.get_mut(n) else {
            continue;
        };
        let flags = u32le(e, 4).unwrap_or(0);
        let off = base + u32le(e, 8).unwrap_or(0) as usize;
        let size = u32le(e, 12).unwrap_or(0);
        // The entry points at the chunk header; the data follows it.
        s.samples.push(((off + 8) as u64, size, flags & 0x10 != 0));
    }
    true
}

/// `rec ` lists inside `movi` nest one level in practice; deeper ones are ignored (each
/// level would cost a stack frame for 12 bytes of file).
const MAX_REC_DEPTH: u32 = 8;

/// Lists the chunks of a `movi` list (and of its `rec ` lists).
fn walk_movi(d: &[u8], from: usize, to: usize, streams: &mut [Stream], depth: u32) {
    for c in chunks(d, from, to) {
        if &c.id == b"LIST" {
            if d.get(c.data..c.data + 4) == Some(b"rec ") && depth < MAX_REC_DEPTH {
                walk_movi(d, c.data + 4, c.end, streams, depth + 1);
            }
            continue;
        }
        if c.id[0..2] == *b"ix" {
            continue;
        }
        let Some(n) = stream_of(&c.id, false) else {
            continue;
        };
        if let Some(s) = streams.get_mut(n) {
            s.samples
                .push((c.data as u64, (c.end - c.data) as u32, false));
        }
    }
}

fn video_track(data: &[u8], s: &Stream, micro_per_frame: u32, samples: bool) -> VideoTrack {
    let f = &s.format;
    let iavs = &s.kind == b"iavs";
    // An `iavs` format is DVINFO, not a bitmap header: the frames give the size.
    let width = if iavs { 0 } else { u32le(f, 4).unwrap_or(0) };
    let height = if iavs {
        0
    } else {
        (u32le(f, 8).unwrap_or(0) as i32).unsigned_abs()
    };
    let compression: [u8; 4] = f
        .get(16..20)
        .and_then(|c| c.try_into().ok())
        .unwrap_or(s.handler);
    let codec = if iavs {
        Codec::Dv
    } else {
        fourcc_codec(compression)
    };
    let (mut scale, mut rate) = (s.scale, s.rate);
    if (scale <= 1 && rate <= 1) && micro_per_frame > 0 {
        (scale, rate) = (micro_per_frame, 1_000_000);
    }
    let mut list = Vec::with_capacity(s.samples.len());
    let any_key = s.samples.iter().any(|x| x.2);
    for (k, &(off, size, key)) in s.samples.iter().enumerate() {
        if size == 0 {
            // A dropped frame: the one before lasts.
            continue;
        }
        let t = (u64::from(s.start) + k as u64) * u64::from(scale);
        list.push(Sample {
            offset: off,
            size,
            dts: t,
            cts: t as i64,
            duration: scale,
            sync: key || !any_key && k == 0,
        });
    }
    // Keyframes the index doesn't mark: the coded pictures say.
    if !any_key {
        for smp in &mut list {
            let Some(b) = data
                .get(smp.offset as usize..(smp.offset + u64::from(smp.size.min(4096))) as usize)
            else {
                continue;
            };
            smp.sync |= match codec {
                Codec::Mpeg4 | Codec::H263 => {
                    specular_mpeg4::first_vop_type(b) == Some(specular_mpeg4::VopType::I)
                }
                Codec::Mpeg2 | Codec::Mpeg1 => specular_mpeg2::scan(b)
                    .pictures
                    .first()
                    .is_some_and(|p| p.kind == specular_mpeg2::PictureType::I),
                // Every picture stands alone.
                Codec::Mjpeg | Codec::Dv | Codec::Jpeg2000 | Codec::Raw => true,
                Codec::Vp3 => specular_theora::is_vp3_keyframe(b),
                Codec::Flv1 => specular_mpeg4::is_sorenson_keyframe(b),
                _ => false,
            };
        }
    }
    let n = list.len();
    let duration = (u64::from(s.start) + s.samples.len() as u64) * u64::from(scale);
    let mut config = bitmap_config(codec, f);
    if matches!(codec, Codec::Mpeg4) && config.is_empty() {
        // The VOL is in the first chunk.
        if let Some(first) = list.first()
            && let Some(b) =
                data.get(first.offset as usize..(first.offset + u64::from(first.size)) as usize)
            && let Some(cfg) = specular_mpeg4::config_bytes(b)
        {
            config = cfg.to_vec();
        }
    }
    VideoTrack {
        codec: Some(codec),
        config,
        width,
        height,
        display: (width, height),
        timescale: rate,
        duration,
        sample_count: n as u32,
        samples: if samples { list } else { Vec::new() },
        slot_timing: true,
        layout: Layout::Contiguous,
        edit: Edit::default(),
        ..VideoTrack::default()
    }
}

/// The sound coding, channels and rate of a WAVEFORMATEX (AVI `strf`, ASF, Matroska
/// `A_MS/ACM`).
pub(crate) fn wave_codec(f: &[u8]) -> Option<(AudioCodec, u16, u32)> {
    let tag = u16le(f, 0)?;
    let channels = u16le(f, 2).unwrap_or(2).max(1);
    let rate = u32le(f, 4).unwrap_or(48_000).max(1);
    let bits = u16le(f, 14).unwrap_or(16);
    let extra = f.get(18..).unwrap_or_default();
    let codec = match tag {
        1 => AudioCodec::Pcm {
            bits,
            float: false,
            little_endian: true,
        },
        3 => AudioCodec::Pcm {
            bits,
            float: true,
            little_endian: true,
        },
        0x50 => AudioCodec::Mpa(0),
        0x55 => AudioCodec::Mpa(3),
        0x2000 => AudioCodec::Ac3,
        0xF1AC => AudioCodec::Flac(Vec::new()),
        0x2001 => AudioCodec::Other(*b"dtsc", 0),
        0x57 => AudioCodec::Other(*b"samr", 0),
        0x58 => AudioCodec::Other(*b"sawb", 0),
        0xFF | 0x1600 | 0x706D => AudioCodec::Aac(extra.to_vec()),
        0x11 => AudioCodec::Adpcm {
            kind: AdpcmKind::ImaWav,
            block: u16le(f, 12).unwrap_or(0),
        },
        2 => AudioCodec::Adpcm {
            kind: AdpcmKind::Ms(ms_coefficients(extra)),
            block: u16le(f, 12).unwrap_or(0),
        },
        0x31 => crate::speech_files::GSM_MS,
        6 => AudioCodec::G711 { alaw: true },
        7 => AudioCodec::G711 { alaw: false },
        0x160..=0x163 => AudioCodec::Other(*b"WMA ", (tag - 0x15F) as u8),
        0x0A => AudioCodec::Other(*b"WMAV", 0),
        _ => AudioCodec::Other(*b"????", 0),
    };
    Some((codec, channels, rate))
}

/// Microsoft ADPCM's coefficient pairs after the WAVEFORMATEX (samples per block, their
/// count, then the pairs); the standard seven when the extra bytes are missing.
fn ms_coefficients(extra: &[u8]) -> Vec<[i16; 2]> {
    let n = extra
        .get(2..4)
        .map_or(0, |b| usize::from(u16::from_le_bytes([b[0], b[1]])));
    let pairs: Vec<[i16; 2]> = extra
        .get(4..)
        .unwrap_or_default()
        .chunks_exact(4)
        .take(n.min(256))
        .map(|c| {
            [
                i16::from_le_bytes([c[0], c[1]]),
                i16::from_le_bytes([c[2], c[3]]),
            ]
        })
        .collect();
    if pairs.is_empty() {
        crate::mp4::MS_COEFFS.to_vec()
    } else {
        pairs
    }
}

fn audio_track(s: &Stream, samples: bool) -> Option<AudioTrack> {
    let f = &s.format;
    let (codec, channels, rate) = wave_codec(f)?;
    let avg_bytes = u32le(f, 8).unwrap_or(0);
    // Constant-rate streams are timed by their bytes, others by chunk count (in the
    // stream's own rate units).
    let cbr = s.sample_size > 0 && avg_bytes > 0;
    // ADPCM blocks hold a known number of samples: time them exactly (the average byte
    // rate is rounded).
    let blocks = match &codec {
        AudioCodec::Adpcm { kind, block } if *block > 0 => Some((
            u64::from(*block),
            crate::wave_sound::block_frames(kind, usize::from(*block), usize::from(channels))
                as u64,
        )),
        _ => None,
    };
    // Chunks counted in the stream's units are timed at the sampling rate too when its
    // units are coarser (MP3 written as 32/1225 s a frame): a seek then lands on the sample,
    // not on a unit of 0.8 ms.
    let fine = !cbr && blocks.is_none() && s.rate > 0 && s.rate < rate;
    let timescale = if cbr || blocks.is_some() || fine {
        rate
    } else {
        s.rate
    };
    let mut list = Vec::with_capacity(s.samples.len());
    let mut bytes = 0u64;
    for (k, &(off, size, _)) in s.samples.iter().enumerate() {
        let t = if let Some((block, frames)) = blocks {
            bytes / block * frames
        } else if cbr {
            bytes * u64::from(rate) / u64::from(avg_bytes)
        } else if fine {
            ((u128::from(s.start) + k as u128) * u128::from(s.scale) * u128::from(rate)
                / u128::from(s.rate)) as u64
        } else {
            (u64::from(s.start) + k as u64) * u64::from(s.scale)
        };
        bytes += u64::from(size);
        if size == 0 {
            continue;
        }
        list.push(Sample {
            offset: off,
            size,
            dts: t,
            cts: t as i64,
            duration: 0,
            sync: true,
        });
    }
    for i in 0..list.len() {
        let next = list.get(i + 1).map(|n| n.dts);
        let smp = &mut list[i];
        smp.duration = next
            .map_or(0, |n| n.saturating_sub(smp.dts))
            .min(u64::from(u32::MAX)) as u32;
    }
    if let Some(l) = list.len().checked_sub(2) {
        let d = list[l].duration;
        list[l + 1].duration = d;
    }
    Some(AudioTrack {
        codec,
        channels,
        sample_rate: rate,
        timescale,
        duration: list.last().map_or(0, |x| x.dts + u64::from(x.duration)),
        sample_count: list.len() as u32,
        samples: if samples { list } else { Vec::new() },
        edit: Edit::default(),
        layout: Layout::Contiguous,
        extra: Default::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = id.to_vec();
        v.extend((body.len() as u32).to_le_bytes());
        v.extend(body);
        if body.len() % 2 == 1 {
            v.push(0);
        }
        v
    }

    fn list(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut b = kind.to_vec();
        b.extend(body);
        chunk(b"LIST", &b)
    }

    /// A `hdrl` list with one video stream, and `extra` chunks in its `strl`.
    fn hdrl(extra: &[u8]) -> Vec<u8> {
        let mut strh = b"vidsXVID".to_vec();
        strh.extend([0u8; 12]);
        strh.extend(1u32.to_le_bytes());
        strh.extend(25u32.to_le_bytes());
        strh.extend([0u8; 28]);
        let mut strf = 40u32.to_le_bytes().to_vec();
        strf.extend(64u32.to_le_bytes());
        strf.extend(48u32.to_le_bytes());
        strf.extend([1, 0, 24, 0]);
        strf.extend(b"XVID");
        strf.extend([0u8; 20]);
        let strl = [chunk(b"strh", &strh), chunk(b"strf", &strf), extra.to_vec()].concat();
        list(
            b"hdrl",
            &[chunk(b"avih", &[0u8; 56]), list(b"strl", &strl)].concat(),
        )
    }

    /// `rec ` lists nested 100 000 deep in `movi` (12 bytes a level): read to a fixed
    /// depth, not until the stack overflows.
    #[test]
    fn deep_rec_lists() {
        const N: usize = 100_000;
        let payload = chunk(b"00dc", &[0, 0, 1, 0xB6]);
        let mut movi = Vec::with_capacity(12 * N + payload.len());
        for i in 0..N {
            movi.extend(b"LIST");
            movi.extend(((4 + 12 * (N - 1 - i) + payload.len()) as u32).to_le_bytes());
            movi.extend(b"rec ");
        }
        movi.extend(&payload);
        let body = [b"AVI ".to_vec(), hdrl(&[]), list(b"movi", &movi)].concat();
        let d = chunk(b"RIFF", &body);
        let m = specular_thread::on_stack(256 * 1024, || parse(&d, true).unwrap());
        // The frame is deeper than the cap: not listed.
        assert!(m.video.unwrap().samples.is_empty());
    }

    /// 65 536 super-index entries naming one `ix00` chunk of 1000 entries: read once
    /// (65 million samples before).
    #[test]
    fn super_index_repeats_read_once() {
        const N: u32 = 1 << 16;
        let movi = list(b"movi", &chunk(b"00dc", &[0, 0, 1, 0xB6]));
        let riff_len =
            |indx: &[u8]| 8 + [b"AVI ".to_vec(), hdrl(indx), movi.clone()].concat().len() as u64;
        let mut indx = vec![4, 0, 0, 0];
        indx.extend(N.to_le_bytes());
        indx.extend(b"00dc");
        indx.extend([0u8; 12]);
        let at = riff_len(&chunk(
            b"indx",
            &[indx.clone(), vec![0; 16 * N as usize]].concat(),
        ));
        for _ in 0..N {
            indx.extend(at.to_le_bytes());
            indx.extend(8024u32.to_le_bytes());
            indx.extend(1u32.to_le_bytes());
        }
        let body = [b"AVI ".to_vec(), hdrl(&chunk(b"indx", &indx)), movi].concat();
        let mut d = chunk(b"RIFF", &body);
        assert_eq!(d.len() as u64, at);
        let mut ix = vec![2, 0, 0, 1];
        ix.extend(1000u32.to_le_bytes());
        ix.extend(b"00dc");
        ix.extend(0u64.to_le_bytes());
        ix.extend([0u8; 4]);
        for k in 0..1000u32 {
            ix.extend((k * 8).to_le_bytes());
            ix.extend(4u32.to_le_bytes());
        }
        d.extend(chunk(b"ix00", &ix));
        let m = parse(&d, true).unwrap();
        assert_eq!(m.video.unwrap().samples.len(), 1000);
    }

    /// A minimal AVI: one video stream of three chunks (one dropped), an idx1.
    fn tiny() -> Vec<u8> {
        let mut strh = b"vidsXVID".to_vec();
        strh.extend([0u8; 12]);
        strh.extend(1u32.to_le_bytes()); // scale
        strh.extend(25u32.to_le_bytes()); // rate
        strh.extend([0u8; 28]);
        let mut strf = 40u32.to_le_bytes().to_vec();
        strf.extend(64u32.to_le_bytes());
        strf.extend(48u32.to_le_bytes());
        strf.extend([1, 0, 24, 0]);
        strf.extend(b"XVID");
        strf.extend([0u8; 20]);
        let hdrl = list(
            b"hdrl",
            &[
                chunk(b"avih", &[0u8; 56]),
                list(
                    b"strl",
                    &[chunk(b"strh", &strh), chunk(b"strf", &strf)].concat(),
                ),
            ]
            .concat(),
        );
        let frames: Vec<&[u8]> = vec![&[0, 0, 1, 0xB6, 0x00], &[], &[0, 0, 1, 0xB6, 0x40]];
        let movi_body: Vec<u8> = frames.iter().flat_map(|f| chunk(b"00dc", f)).collect();
        let movi = list(b"movi", &movi_body);
        let mut idx = Vec::new();
        let mut off = 4u32;
        for (k, f) in frames.iter().enumerate() {
            idx.extend(b"00dc");
            idx.extend(if k == 0 { 0x10u32 } else { 0 }.to_le_bytes());
            idx.extend(off.to_le_bytes());
            idx.extend((f.len() as u32).to_le_bytes());
            off += 8 + f.len() as u32 + (f.len() as u32 & 1);
        }
        let body = [b"AVI ".to_vec(), hdrl, movi, chunk(b"idx1", &idx)].concat();
        chunk(b"RIFF", &body)
    }

    #[test]
    fn index_samples_and_dropped_frames() {
        let d = tiny();
        assert!(sniff(&d));
        let m = parse(&d, true).unwrap();
        let v = m.video.unwrap();
        assert_eq!(v.codec, Some(Codec::Mpeg4));
        assert_eq!((v.width, v.height), (64, 48));
        assert_eq!(v.samples.len(), 2);
        assert_eq!(v.samples[1].cts, 2);
        assert!(v.samples[0].sync && !v.samples[1].sync);
        let b = v.sample_data(&d, &v.samples[1]).unwrap();
        assert_eq!(&b[..], &[0, 0, 1, 0xB6, 0x40]);
        // Cut and garbled files never panic.
        for cut in [10, 50, 100, d.len() - 7] {
            let _ = parse(&d[..cut], true);
        }
    }
}
