//! PlayStation movies (`.str`): CD-ROM sectors of MDEC pictures with CD-ROM XA ADPCM sound
//! between them (psx-spx, "CDROM File Video Streaming STR" and "CDROM XA Audio").
//!
//! The file is raw 2352-byte sectors (sync, header, subheader), 2336-byte ones (subheader
//! first), the same in a RIFF `CDXA` wrapper, or 2048-byte user data only (no sound). A
//! video sector's data starts with the 32-byte STR header (0160h, 8001h, the sector's place
//! in its frame, the frame's sector count, frame number and size, picture width and height);
//! a frame is the 2016 bytes after the headers of its sectors in turn. A sound sector is a
//! Form 2 sector with the audio bit of its submode set; its coding byte gives the channels,
//! the rate (37800 or 18900 Hz) and the sample size.
//!
//! There is no frame rate: frames come at the disc's speed (75 or 150 sectors a second,
//! told by the sound's rate against the share of sound sectors) over the sectors a frame
//! takes on average.

use crate::mp4::{
    AdpcmKind, AudioCodec, AudioTrack, Classic, Codec, Edit, Error, Layout, MAX_SAMPLES, Movie,
    Result, Sample, SoundExtra, VideoTrack,
};

const SYNC: [u8; 12] = [
    0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0,
];
const STR_MAGIC: [u8; 4] = [0x60, 0x01, 0x01, 0x80];
/// Bytes of a frame in each video sector (2048 less the STR header).
const PER_SECTOR: usize = 0x7E0;
/// Sectors looked at for the first video sector.
const LEAD: usize = 32;

/// How the sectors are laid out in the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StrLayout {
    /// File offset of the first sector.
    pub base: u64,
    /// Bytes a sector (2352, 2336 or 2048).
    pub sector: u16,
    /// Offset of the subheader in a sector (none at 2048).
    pub sub: Option<u16>,
}

impl StrLayout {
    fn data_at(&self) -> usize {
        self.sub.map_or(0, |s| usize::from(s) + 8)
    }

    fn sectors(&self, len: usize) -> usize {
        len.saturating_sub(self.base as usize) / usize::from(self.sector)
    }

    fn sector<'a>(&self, d: &'a [u8], k: usize) -> Option<&'a [u8]> {
        let at = self.base as usize + k * usize::from(self.sector);
        d.get(at..at + usize::from(self.sector))
    }

    /// The subheader's submode and coding bytes, file and channel.
    fn subheader(&self, s: &[u8]) -> Option<[u8; 4]> {
        let o = usize::from(self.sub?);
        s.get(o..o + 4).map(|b| [b[0], b[1], b[2], b[3]])
    }

    /// The STR header of a video sector.
    fn video<'a>(&self, s: &'a [u8]) -> Option<&'a [u8]> {
        let d = s.get(self.data_at()..self.data_at() + 0x800)?;
        // A Form 2 sector holds sound or other data, never a picture.
        if let Some([_, _, mode, _]) = self.subheader(s)
            && mode & 0x20 != 0
        {
            return None;
        }
        d.starts_with(&STR_MAGIC).then_some(d)
    }

    /// Gathers the frame whose first sector is at file offset `offset`.
    pub fn read(&self, data: &[u8], s: &Sample) -> Option<Vec<u8>> {
        let first = (s.offset.checked_sub(self.base)? / u64::from(self.sector)) as usize;
        let head = self.video(self.sector(data, first)?)?;
        let frame = u32le(head, 8);
        let count = usize::from(u16le(head, 6)).clamp(1, 512);
        let mut out = vec![0u8; count * PER_SECTOR];
        let mut got = 0;
        for k in first..(first + count * 4 + 16).min(self.sectors(data.len())) {
            let Some(v) = self.sector(data, k).and_then(|x| self.video(x)) else {
                continue;
            };
            if u32le(v, 8) != frame {
                if got > 0 {
                    break;
                }
                continue;
            }
            let i = usize::from(u16le(v, 4));
            if i < count {
                out[i * PER_SECTOR..(i + 1) * PER_SECTOR].copy_from_slice(&v[0x20..0x800]);
                got += 1;
            }
            if got == count {
                break;
            }
        }
        (got > 0).then(|| {
            out.truncate((s.size as usize).clamp(8, out.len()));
            out
        })
    }
}

fn u16le(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32le(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// The sector layout of a PlayStation movie, when `d` starts one.
fn layout(d: &[u8]) -> Option<StrLayout> {
    let cdxa = d.len() > 44 && d.starts_with(b"RIFF") && &d[8..12] == b"CDXA";
    let candidates: &[StrLayout] = if cdxa {
        &[StrLayout {
            base: 44,
            sector: 2352,
            sub: Some(16),
        }]
    } else if d.starts_with(&SYNC) {
        &[StrLayout {
            base: 0,
            sector: 2352,
            sub: Some(16),
        }]
    } else {
        &[
            StrLayout {
                base: 0,
                sector: 2336,
                sub: Some(0),
            },
            StrLayout {
                base: 0,
                sector: 2048,
                sub: None,
            },
        ]
    };
    candidates.iter().copied().find(|l| {
        (0..LEAD.min(l.sectors(d.len()))).any(|k| {
            let Some(s) = l.sector(d, k) else {
                return false;
            };
            if l.sub == Some(16) && !s.starts_with(&SYNC) {
                return false;
            }
            if l.sub == Some(0) && s[0..4] != s[4..8] {
                return false;
            }
            l.video(s).is_some_and(|v| {
                let (w, h) = (u16le(v, 16), u16le(v, 18));
                (8..=1024).contains(&w) && (8..=1024).contains(&h) && v[6] > 0
            })
        })
    })
}

pub fn sniff(d: &[u8]) -> bool {
    layout(d).is_some()
}

pub fn parse(d: &[u8], samples: bool) -> Result<Movie> {
    let l = layout(d).ok_or_else(|| Error::Malformed("not a PlayStation movie".into()))?;
    let n = l.sectors(d.len());
    let mut frames: Vec<Sample> = Vec::new();
    let mut last_frame = None;
    // The last frame's sectors seen and needed.
    let mut seen = (0u16, 0u16);
    let mut size = (0u32, 0u32);
    let mut sound: Vec<Sample> = Vec::new();
    // The first sound stream's file, channel and coding.
    let mut stream: Option<(u8, u8, u8)> = None;
    for k in 0..n {
        let Some(s) = l.sector(d, k) else { break };
        if let Some(v) = l.video(s) {
            let f = u32le(v, 8);
            if last_frame == Some(f) {
                seen.0 += 1;
            }
            if last_frame != Some(f) && frames.len() < MAX_SAMPLES {
                last_frame = Some(f);
                seen = (1, u16le(v, 6));
                if size == (0, 0) {
                    size = (u32::from(u16le(v, 16)), u32::from(u16le(v, 18)));
                }
                frames.push(Sample {
                    offset: l.base + (k * usize::from(l.sector)) as u64,
                    size: u32le(v, 12).saturating_add(8),
                    dts: 0,
                    cts: 0,
                    duration: 0,
                    sync: true,
                });
            }
            continue;
        }
        let Some([file, channel, mode, coding]) = l.subheader(s) else {
            continue;
        };
        if mode & 0x24 != 0x24 {
            continue;
        }
        let id = (file, channel, coding);
        if *stream.get_or_insert(id) != id || sound.len() >= MAX_SAMPLES {
            continue;
        }
        sound.push(Sample {
            offset: (l.base as usize + k * usize::from(l.sector) + l.data_at()) as u64,
            size: 18 * crate::wave_sound::XA_GROUP as u32,
            dts: 0,
            cts: 0,
            duration: 0,
            sync: true,
        });
    }
    // A file cut short ends in part of a frame: leave it out.
    if seen.0 < seen.1 && frames.len() > 1 {
        frames.pop();
    }
    if frames.is_empty() {
        return Err(Error::Malformed(
            "a PlayStation movie without pictures".into(),
        ));
    }
    let audio = stream.map(|(_, _, coding)| {
        let channels = if coding & 3 == 1 { 2 } else { 1 };
        let rate = if coding & 4 != 0 { 18_900 } else { 37_800 };
        let eight = coding & 0x30 == 0x10;
        let per = crate::wave_sound::block_frames(
            &AdpcmKind::Xa { eight },
            crate::wave_sound::XA_GROUP,
            usize::from(channels),
        ) as u64
            * 18;
        let count = sound.len();
        for (i, s) in sound.iter_mut().enumerate() {
            s.dts = i as u64 * per;
            s.cts = s.dts as i64;
            s.duration = per as u32;
        }
        AudioTrack {
            codec: AudioCodec::Adpcm {
                kind: AdpcmKind::Xa { eight },
                block: crate::wave_sound::XA_GROUP as u16,
            },
            channels,
            sample_rate: rate,
            timescale: rate,
            duration: count as u64 * per,
            sample_count: count as u32,
            samples: if samples {
                std::mem::take(&mut sound)
            } else {
                Vec::new()
            },
            edit: Edit::default(),
            layout: Layout::Contiguous,
            extra: SoundExtra::default(),
        }
    });
    // Sectors a second: 150 (double speed) unless the sound says single speed.
    let speed = match &audio {
        Some(a) if a.sample_count > 0 => {
            let per_sector = a.duration / u64::from(a.sample_count);
            let sound_per_s = f64::from(a.sample_rate) / per_sector as f64;
            let all_per_s = sound_per_s * n as f64 / f64::from(a.sample_count);
            if all_per_s < 112.0 { 75 } else { 150 }
        }
        _ => 150,
    };
    let per_frame = (n as u64).div_ceil(frames.len() as u64).max(1) as u32;
    let per_frame = ((n as f64 / frames.len() as f64).round() as u32).clamp(1, per_frame);
    let count = frames.len();
    for (i, f) in frames.iter_mut().enumerate() {
        f.dts = i as u64 * u64::from(per_frame);
        f.cts = f.dts as i64;
        f.duration = per_frame;
    }
    let (w, h) = size;
    let mut v = VideoTrack {
        codec: Some(Codec::Classic(Classic::Mdec)),
        config: crate::rawvid::PixDesc {
            fourcc: *b"MDEC",
            depth: 24,
            ..Default::default()
        }
        .to_config(),
        width: w,
        height: h,
        display: (w, h),
        timescale: speed,
        layout: Layout::Str(l),
        ..VideoTrack::default()
    };
    v.duration = count as u64 * u64::from(per_frame);
    v.sample_count = count as u32;
    if samples {
        v.samples = frames;
    }
    let duration_us = v.duration_us();
    Ok(Movie {
        brand: *b"STR ",
        timescale: speed,
        duration_us,
        video: Some(v),
        has_audio: audio.is_some(),
        audio,
        ..Movie::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A raw 2352-byte sector: video (frame `f`, sector `i` of `n`) or XA sound.
    pub(crate) fn sector(video: Option<(u32, u16, u16, &[u8])>) -> Vec<u8> {
        let mut s = vec![0u8; 2352];
        s[..12].copy_from_slice(&SYNC);
        s[15] = 2;
        match video {
            Some((f, i, n, payload)) => {
                s[16..24].copy_from_slice(&[1, 0, 0x08, 0, 1, 0, 0x08, 0]);
                let h = &mut s[24..];
                h[..4].copy_from_slice(&STR_MAGIC);
                h[4..6].copy_from_slice(&i.to_le_bytes());
                h[6..8].copy_from_slice(&n.to_le_bytes());
                h[8..12].copy_from_slice(&f.to_le_bytes());
                h[12..16].copy_from_slice(&(payload.len() as u32).to_le_bytes());
                h[16..18].copy_from_slice(&16u16.to_le_bytes());
                h[18..20].copy_from_slice(&16u16.to_le_bytes());
                let at = usize::from(i) * PER_SECTOR;
                let part = &payload[at.min(payload.len())..(at + PER_SECTOR).min(payload.len())];
                h[0x20..0x20 + part.len()].copy_from_slice(part);
            }
            None => {
                // Stereo 37800 Hz 4-bit.
                s[16..24].copy_from_slice(&[1, 1, 0x64, 1, 1, 1, 0x64, 1]);
            }
        }
        s
    }

    #[test]
    fn frames_and_sound_are_found() {
        let payload: Vec<u8> = (0..3000u32).map(|i| i as u8).collect();
        let mut d = Vec::new();
        for f in 1..=3u32 {
            d.extend(sector(Some((f, 0, 2, &payload))));
            d.extend(sector(None));
            d.extend(sector(Some((f, 1, 2, &payload))));
            d.extend(sector(None));
        }
        assert!(sniff(&d));
        let m = parse(&d, true).unwrap();
        let v = m.video.as_ref().unwrap();
        assert_eq!(v.samples.len(), 3);
        assert_eq!((v.width, v.height), (16, 16));
        let a = m.audio.as_ref().unwrap();
        assert_eq!((a.channels, a.sample_rate, a.samples.len()), (2, 37_800, 6));
        assert_eq!(a.samples[1].dts, 2016);
        let got = v.sample_data(&d, &v.samples[1]).unwrap();
        // The frame's size plus 8 bytes of slack for the bit reader.
        assert_eq!(got.len(), 3008);
        assert_eq!(&got[..3000], &payload[..3000]);
        assert!(!sniff(&vec![0u8; 2352 * 4]));
    }
}
