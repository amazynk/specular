//! Flash Video (`.flv`, `.f4v` is MP4) into the same [`Movie`] the MP4 demuxer gives (Adobe
//! "Video File Format Specification" version 10, and the enhanced RTMP/FLV extension for
//! HEVC, AV1 and VP9).
//!
//! - A 9-byte header (`FLV`, version, audio/video flags, header size), then tags: type
//!   (8 sound, 9 video, 18 script data), size, a millisecond timestamp, the body and the
//!   size of the tag before.
//! - Video: H.264 (its `AVCDecoderConfigurationRecord` in the first packet, then
//!   length-prefixed NAL units with a composition offset), Sorenson Spark (H.263), VP6, and
//!   the enhanced codes `avc1`, `hvc1`, `av01`, `vp09`. Frame type 1 marks keyframes.
//! - Sound: AAC (its AudioSpecificConfig in the first packet), MP3, PCM, G.711 and Flash
//!   ADPCM, Nellymoser and Speex.
//! - The `onMetaData` script tag gives the picture size when the coding doesn't.

use crate::mp4::{
    AudioCodec, AudioTrack, Codec, Edit, Error, Layout, MAX_SAMPLES, Movie, Result, Sample,
    VideoTrack,
};

fn bad(s: impl Into<String>) -> Error {
    Error::Malformed(s.into())
}

/// Whether `head` starts a Flash Video file.
pub fn sniff(head: &[u8]) -> bool {
    head.len() >= 9 && &head[..3] == b"FLV" && head[3] == 1 && be32(head, 5) >= 9
}

fn be24(d: &[u8], at: usize) -> u32 {
    u32::from(d[at]) << 16 | u32::from(d[at + 1]) << 8 | u32::from(d[at + 2])
}

fn be32(d: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

/// A signed 24-bit composition offset.
fn si24(d: &[u8], at: usize) -> i64 {
    let v = be24(d, at);
    i64::from(((v << 8) as i32) >> 8)
}

/// One coded frame or sound packet: where its bytes are and its times (ms).
struct Packet {
    offset: usize,
    size: usize,
    dts: i64,
    cts: i64,
    key: bool,
}

#[derive(Default)]
struct VideoState {
    codec: Option<Codec>,
    config: Vec<u8>,
    packets: Vec<Packet>,
}

#[derive(Default)]
struct SoundState {
    codec: Option<AudioCodec>,
    rate: u32,
    channels: u16,
    packets: Vec<Packet>,
}

/// The video codec of a classic FLV codec id.
fn classic_video(id: u8) -> Codec {
    match id {
        2 => Codec::Flv1,
        4 => Codec::Other(*b"VP6F"),
        5 => Codec::Other(*b"VP6A"),
        3 => Codec::Classic(crate::mp4::Classic::FlashSv1),
        6 => Codec::Classic(crate::mp4::Classic::FlashSv2),
        7 => Codec::H264,
        12 => Codec::Hevc,
        _ => Codec::Other(*b"????"),
    }
}

fn video_tag(body: &[u8], at: usize, ts: i64, v: &mut VideoState) {
    let Some(&b0) = body.first() else {
        return;
    };
    let ex = b0 & 0x80 != 0;
    let frame_type = (b0 >> 4) & if ex { 7 } else { 15 };
    if frame_type == 5 {
        // A command frame (seek marker): no picture.
        return;
    }
    let key = frame_type == 1;
    if ex {
        // Enhanced: packet type in the low nibble, then the codec's four-character code.
        let kind = b0 & 15;
        let Some(fourcc) = body.get(1..5) else {
            return;
        };
        let codec = match fourcc {
            b"avc1" => Codec::H264,
            b"hvc1" => Codec::Hevc,
            b"av01" => Codec::Av1,
            b"vp09" => Codec::Vp9,
            c => Codec::Other(c.try_into().unwrap_or(*b"????")),
        };
        v.codec.get_or_insert(codec);
        match kind {
            // Sequence start: the configuration record.
            0 => {
                if v.config.is_empty() {
                    v.config = body[5..].to_vec();
                }
            }
            // Coded frames, with a composition offset for AVC/HEVC.
            1 | 3 => {
                let with_cts = kind == 1 && matches!(codec, Codec::H264 | Codec::Hevc);
                let skip = 5 + if with_cts { 3 } else { 0 };
                if body.len() <= skip {
                    return;
                }
                let cto = if with_cts { si24(body, 5) } else { 0 };
                v.packets.push(Packet {
                    offset: at + skip,
                    size: body.len() - skip,
                    dts: ts,
                    cts: ts + cto,
                    key,
                });
            }
            _ => {}
        }
        return;
    }
    let codec = classic_video(b0 & 15);
    v.codec.get_or_insert(codec);
    match codec {
        Codec::H264 | Codec::Hevc => {
            let Some(&kind) = body.get(1) else {
                return;
            };
            if body.len() < 5 {
                return;
            }
            match kind {
                0 if v.config.is_empty() => v.config = body[5..].to_vec(),
                1 if body.len() > 5 => v.packets.push(Packet {
                    offset: at + 5,
                    size: body.len() - 5,
                    dts: ts,
                    cts: ts + si24(body, 2),
                    key,
                }),
                _ => {}
            }
        }
        _ => {
            // VP6 has a byte of crop adjustment (and VP6 alpha an offset) first.
            let skip = match &codec {
                Codec::Other(c) => match c {
                    b"VP6F" => 2,
                    b"VP6A" => 5,
                    _ => 1,
                },
                _ => 1,
            };
            if body.len() > skip {
                v.packets.push(Packet {
                    offset: at + skip,
                    size: body.len() - skip,
                    dts: ts,
                    cts: ts,
                    key,
                });
            }
        }
    }
}

fn sound_tag(body: &[u8], at: usize, ts: i64, a: &mut SoundState) {
    let Some(&b0) = body.first() else {
        return;
    };
    let format = b0 >> 4;
    let rate = [5512, 11025, 22050, 44100][usize::from((b0 >> 2) & 3)];
    let bits = if b0 & 2 != 0 { 16 } else { 8 };
    let channels = if b0 & 1 != 0 { 2 } else { 1 };
    if a.codec.is_none() {
        a.rate = rate;
        a.channels = channels;
        a.codec = Some(match format {
            0 | 3 => AudioCodec::Pcm {
                bits,
                float: false,
                little_endian: true,
            },
            2 => AudioCodec::Mpa(3),
            14 => {
                a.rate = 8000;
                AudioCodec::Mpa(3)
            }
            10 => AudioCodec::Aac(Vec::new()),
            1 => AudioCodec::Other(*b"swfa", 0),
            // Nellymoser is mono; formats 4 and 5 are its 16 and 8 kHz voice modes.
            4..=6 => {
                a.channels = 1;
                match format {
                    4 => a.rate = 16000,
                    5 => a.rate = 8000,
                    _ => {}
                }
                AudioCodec::Other(*b"nmos", 0)
            }
            // G.711 in FLV is always 8 kHz, whatever the flags say.
            7 | 8 => {
                a.rate = 8000;
                AudioCodec::G711 { alaw: format == 7 }
            }
            // Speex in FLV is wideband mono.
            11 => {
                a.rate = 16000;
                a.channels = 1;
                AudioCodec::Speex(Vec::new())
            }
            _ => AudioCodec::Other(*b"????", 0),
        });
    }
    if format == 10 {
        let Some(&kind) = body.get(1) else {
            return;
        };
        if kind == 0 {
            if let Some(AudioCodec::Aac(cfg)) = a.codec.as_mut()
                && cfg.is_empty()
            {
                *cfg = body[2..].to_vec();
            }
            return;
        }
        if body.len() > 2 {
            a.packets.push(Packet {
                offset: at + 2,
                size: body.len() - 2,
                dts: ts,
                cts: ts,
                key: true,
            });
        }
        return;
    }
    if body.len() > 1 {
        a.packets.push(Packet {
            offset: at + 1,
            size: body.len() - 1,
            dts: ts,
            cts: ts,
            key: true,
        });
    }
}

/// `width` and `height` of the `onMetaData` ECMA array (AMF0), when present.
fn metadata_size(body: &[u8]) -> (Option<u32>, Option<u32>) {
    let num = |key: &[u8]| -> Option<u32> {
        let at = body
            .windows(key.len() + 2)
            .position(|w| w[..2] == (key.len() as u16).to_be_bytes() && &w[2..] == key)?
            + key.len()
            + 2;
        // AMF0 number: marker 0, then a big-endian double.
        if *body.get(at)? != 0 {
            return None;
        }
        let v = f64::from_be_bytes(body.get(at + 1..at + 9)?.try_into().ok()?);
        (v.is_finite() && (1.0..=65_536.0).contains(&v)).then_some(v as u32)
    };
    (num(b"width"), num(b"height"))
}

/// H.264 without composition offsets (some old muxers write none, B-frames or not): the
/// pictures' order counts give the presentation order, as the decoder shows them.
fn poc_order(data: &[u8], config: &[u8], packets: &mut [Packet]) {
    if packets.len() < 2 || packets.iter().any(|p| p.cts != p.dts) {
        return;
    }
    let mut units = Vec::with_capacity(packets.len());
    for pk in packets.iter() {
        let Some(b) = data.get(pk.offset..pk.offset + pk.size) else {
            return;
        };
        units.push(b);
    }
    let Some(rank) = avc_poc_rank(config, &units) else {
        return;
    };
    let dts: Vec<i64> = packets.iter().map(|p| p.dts).collect();
    for (pk, &r) in packets.iter_mut().zip(&rank) {
        pk.cts = dts[r as usize];
    }
}

/// The presentation rank of each length-prefixed H.264 access unit (`avcC` record `config`,
/// units in decode order), from the pictures' order counts.
pub(crate) fn avc_poc_rank(config: &[u8], units: &[&[u8]]) -> Option<Vec<i64>> {
    if config.len() < 7 {
        return None;
    }
    let len_size = usize::from(config[4] & 3) + 1;
    // The parameter sets of the record lead the first access unit.
    let mut head = Vec::new();
    let mut p = 5;
    for k in 0..2 {
        let n = *config.get(p)?;
        p += 1;
        let n = if k == 0 { n & 31 } else { n };
        for _ in 0..n {
            let l = config.get(p..p + 2)?;
            let l = usize::from(u16::from_be_bytes([l[0], l[1]]));
            let nal = config.get(p + 2..p + 2 + l)?;
            head.extend_from_slice(&[0, 0, 1]);
            head.extend_from_slice(nal);
            p += 2 + l;
        }
    }
    let mut aus: Vec<Vec<u8>> = Vec::with_capacity(units.len());
    for (i, &unit) in units.iter().enumerate() {
        let mut b = unit;
        let mut au = if i == 0 { head.clone() } else { Vec::new() };
        // Enough of each NAL unit for its slice header.
        while b.len() > len_size {
            let l = b[..len_size]
                .iter()
                .fold(0usize, |a, &x| a << 8 | usize::from(x));
            let Some(nal) = b.get(len_size..len_size + l) else {
                break;
            };
            au.extend_from_slice(&[0, 0, 1]);
            let keep = if nal.first().is_some_and(|h| h & 31 == 7) {
                nal.len()
            } else {
                nal.len().min(64)
            };
            au.extend_from_slice(&nal[..keep]);
            b = &b[len_size + l..];
        }
        aus.push(au);
    }
    let refs: Vec<&[u8]> = aus.iter().map(Vec::as_slice).collect();
    crate::mxf::h264_presentation(&refs)
}

/// The samples of packets in ms, each lasting until the next in presentation order.
fn samples(packets: &[Packet]) -> Vec<Sample> {
    let base = packets
        .iter()
        .map(|p| p.dts.min(p.cts))
        .min()
        .unwrap_or(0)
        .min(0);
    let mut order: Vec<i64> = packets.iter().map(|p| p.cts - base).collect();
    order.sort_unstable();
    order.dedup();
    let typical = order
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|&d| d > 0)
        .min()
        .unwrap_or(40);
    packets
        .iter()
        .map(|p| {
            let cts = p.cts - base;
            let k = order.partition_point(|&c| c <= cts);
            Sample {
                offset: p.offset as u64,
                size: p.size as u32,
                dts: (p.dts - base).max(0) as u64,
                cts,
                duration: order
                    .get(k)
                    .map_or(typical, |n| n - cts)
                    .clamp(0, i64::from(u32::MAX)) as u32,
                sync: p.key,
            }
        })
        .collect()
}

/// The coded size from an `AVCDecoderConfigurationRecord`'s first SPS.
fn avcc_size(cfg: &[u8]) -> Option<(u32, u32)> {
    if cfg.len() < 8 || cfg[5] & 31 == 0 {
        return None;
    }
    let len = usize::from(u16::from_be_bytes([cfg[6], cfg[7]]));
    crate::ts::h264_size(cfg.get(8..8 + len)?)
}

/// Parses a Flash Video file. With `samples`, every packet is listed.
pub fn parse(data: &[u8], samples_wanted: bool) -> Result<Movie> {
    if !sniff(data) {
        return Err(bad("not an FLV file"));
    }
    let mut at = be32(data, 5) as usize + 4;
    let mut v = VideoState::default();
    let mut a = SoundState::default();
    let (mut meta_w, mut meta_h) = (None, None);
    // Timestamps may wrap past 2^32 ms only in theory; the extension byte gives bits 24-31.
    while at + 11 <= data.len() {
        let kind = data[at] & 0x1F;
        let filtered = data[at] & 0x20 != 0;
        let size = be24(data, at + 1) as usize;
        let ts = i64::from(be24(data, at + 4) | u32::from(data[at + 7]) << 24);
        let body_at = at + 11;
        let Some(body) = data.get(body_at..body_at + size) else {
            break;
        };
        if filtered {
            return Err(Error::Unsupported("encrypted Flash Video".into()));
        }
        match kind {
            8 => sound_tag(body, body_at, ts, &mut a),
            9 => video_tag(body, body_at, ts, &mut v),
            18 if meta_w.is_none() => (meta_w, meta_h) = metadata_size(body),
            _ => {}
        }
        if v.packets.len() + a.packets.len() > 2 * MAX_SAMPLES {
            return Err(Error::Unsupported("over a million packets".into()));
        }
        at = body_at + size + 4;
    }
    let mut movie = Movie {
        brand: *b"FLV ",
        ..Movie::default()
    };
    if let Some(codec) = v.codec
        && !v.packets.is_empty()
    {
        if codec == Codec::H264 {
            poc_order(data, &v.config, &mut v.packets);
        }
        let list = samples(&v.packets);
        let (w, h) = match codec {
            Codec::H264 => avcc_size(&v.config),
            // Screen video: every packet starts with the picture size (12 bits each).
            Codec::Classic(crate::mp4::Classic::FlashSv1 | crate::mp4::Classic::FlashSv2) => v
                .packets
                .first()
                .and_then(|p| data.get(p.offset..p.offset + 4))
                .map(|b| {
                    (
                        u32::from(b[0] & 15) << 8 | u32::from(b[1]),
                        u32::from(b[2] & 15) << 8 | u32::from(b[3]),
                    )
                }),
            _ => None,
        }
        .unwrap_or((meta_w.unwrap_or(0), meta_h.unwrap_or(0)));
        let mut list = list;
        if !list.iter().any(|s| s.sync)
            && let Some(s) = list.first_mut()
        {
            s.sync = true;
        }
        let duration = list
            .iter()
            .map(|s| s.cts.max(0) as u64 + u64::from(s.duration))
            .max()
            .unwrap_or(0);
        movie.video = Some(VideoTrack {
            codec: Some(codec),
            config: v.config,
            width: w,
            height: h,
            display: (w, h),
            timescale: 1000,
            duration,
            sample_count: list.len() as u32,
            samples: if samples_wanted { list } else { Vec::new() },
            layout: Layout::Contiguous,
            edit: Edit::default(),
            ..VideoTrack::default()
        });
    }
    if let Some(codec) = a.codec
        && !a.packets.is_empty()
    {
        let list = samples(&a.packets);
        let duration = list
            .iter()
            .map(|s| s.cts.max(0) as u64 + u64::from(s.duration))
            .max()
            .unwrap_or(0);
        movie.audio = Some(AudioTrack {
            codec,
            channels: a.channels,
            sample_rate: a.rate,
            timescale: 1000,
            duration,
            sample_count: list.len() as u32,
            samples: if samples_wanted { list } else { Vec::new() },
            edit: Edit::default(),
            layout: Layout::Contiguous,
            extra: Default::default(),
        });
        movie.has_audio = true;
    }
    if movie.video.is_none() && movie.audio.is_none() {
        return Err(Error::Unsupported("no video or sound in the stream".into()));
    }
    let vd = movie.video.as_ref().and_then(|t| t.duration_us());
    let ad = movie.audio.as_ref().map(|t| t.duration * 1000);
    movie.duration_us = vd.max(ad);
    Ok(movie)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(kind: u8, ts: u32, body: &[u8]) -> Vec<u8> {
        let mut t = vec![kind];
        t.extend_from_slice(&(body.len() as u32).to_be_bytes()[1..]);
        t.extend_from_slice(&ts.to_be_bytes()[1..]);
        t.push((ts >> 24) as u8);
        t.extend_from_slice(&[0, 0, 0]);
        t.extend_from_slice(body);
        t.extend_from_slice(&(11 + body.len() as u32).to_be_bytes());
        t
    }

    fn file(tags: &[Vec<u8>]) -> Vec<u8> {
        let mut f = b"FLV\x01\x05\x00\x00\x00\x09\x00\x00\x00\x00".to_vec();
        for t in tags {
            f.extend_from_slice(t);
        }
        f
    }

    #[test]
    fn spark_and_mp3_tags_become_tracks() {
        let f = file(&[
            tag(9, 0, &[0x12, 0, 0, 0x84, 1]),
            tag(8, 0, &[0x2F, 0xFF, 0xFB]),
            tag(9, 40, &[0x22, 0, 0, 0x86, 2]),
            tag(8, 26, &[0x2F, 0xFF, 0xFB]),
            tag(9, 80, &[0x22, 0, 0, 0x88, 3]),
        ]);
        assert!(sniff(&f));
        let m = parse(&f, true).unwrap();
        let v = m.video.unwrap();
        assert_eq!(v.codec, Some(Codec::Flv1));
        assert_eq!(v.samples.len(), 3);
        assert_eq!(v.samples[1].cts, 40);
        assert!(v.samples[0].sync && !v.samples[1].sync);
        assert_eq!(&f[v.samples[2].offset as usize..][..4], &[0, 0, 0x88, 3]);
        let a = m.audio.unwrap();
        assert_eq!(a.codec, AudioCodec::Mpa(3));
        assert_eq!((a.sample_rate, a.channels), (44100, 2));
    }

    #[test]
    fn avc_packets_carry_composition_offsets() {
        let f = file(&[
            tag(9, 0, &[0x17, 0, 0, 0, 0, 1, 0x64, 0, 0x1F, 0xFF, 0xE0, 0]),
            tag(9, 0, &[0x17, 1, 0, 0, 80, 0, 0, 0, 1, 0x65]),
            tag(9, 40, &[0x27, 1, 0xFF, 0xFF, 0xD8, 0, 0, 0, 1, 0x41]),
        ]);
        let m = parse(&f, true).unwrap();
        let v = m.video.unwrap();
        assert_eq!(v.codec, Some(Codec::H264));
        assert_eq!(v.config[0], 1);
        assert_eq!(v.samples[0].cts, 80);
        assert_eq!(v.samples[1].cts, 0);
    }

    #[test]
    fn damaged_files_do_not_panic() {
        let f = file(&[tag(9, 0, &[0x17, 1, 0])]);
        for n in 0..f.len() {
            let _ = parse(&f[..n], true);
        }
        let mut meta = vec![2, 0, 10];
        meta.extend_from_slice(b"onMetaData");
        meta.extend_from_slice(&[8, 0, 0, 0, 2, 0, 5]);
        meta.extend_from_slice(b"width");
        meta.push(0);
        meta.extend_from_slice(&320f64.to_be_bytes());
        assert_eq!(metadata_size(&meta).0, Some(320));
    }
}
