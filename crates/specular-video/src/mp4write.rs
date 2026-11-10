//! An ISO-BMFF (MP4) writer for one H.264 video track without audio. Its first user is the
//! slideshow export (DESIGN §6.5).
//!
//! The file is laid out for streaming playback ("faststart"): `ftyp`, then `moov`, then a
//! single `mdat`. Samples are written straight to the output as they arrive, so the whole
//! movie never sits in memory. To make that possible without a second pass, [`Mp4Writer::new`]
//! reserves room for the largest `moov` the announced sample count can need, assuming every
//! sample might be a sync sample. [`Mp4Writer::finish`] then writes the real `moov` into that
//! room and fills the rest with a `free` box.
//!
//! If more samples arrive than were announced, `moov` goes after `mdat` instead. The file is
//! still valid; only the faststart layout is lost.
//!
//! All samples form one chunk, so `stsc` and `stco` hold a single entry each. The chunk
//! offset is small because only `ftyp` and the reservation come before it, but `co64` is
//! still used when the offset needs it. `mdat` always carries a 64-bit size, so files over
//! 4 GiB need no special handling. Boxes written:
//! - `ftyp`
//! - `moov`: `mvhd` and one `trak`:
//!   - `tkhd`
//!   - `mdia`: `mdhd`, `hdlr`, and `minf` holding `vmhd`, `dinf/dref` and `stbl`
//!   - `stbl`: `stsd`, `stts`, `stss`, `stsz`, `stsc` and `stco`/`co64`
//!   - `stsd` holds `avc1` with `avcC`, `colr` `nclx` and `pasp`
//! - `free`
//! - `mdat`
//!
//! `stss` is left out when every sample is a sync sample, which the format defines to mean
//! the same thing.

use std::io::{self, Seek, SeekFrom, Write};

use crate::mp4::Colour;

/// The track being written.
#[derive(Clone, Debug)]
pub struct Track {
    pub width: u32,
    pub height: u32,
    /// Frame rate as a fraction: `fps_num / fps_den` frames per second.
    pub fps_num: u32,
    pub fps_den: u32,
    /// The `AVCDecoderConfigurationRecord` (body of `avcC`).
    pub avcc: Vec<u8>,
    pub colour: Colour,
}

impl Track {
    /// Media timescale and the duration of one frame in it. 90 kHz when the rate divides it
    /// exactly (24, 25, 30, 50 and 60 fps all do); otherwise the rate's own numerator.
    fn timing(&self) -> (u32, u32) {
        let (n, d) = (self.fps_num.max(1), self.fps_den.max(1));
        if 90_000 % n == 0 && (90_000 / n).checked_mul(d).is_some() {
            (90_000, 90_000 / n * d)
        } else {
            (n, d)
        }
    }
}

/// The movie timescale of `mvhd` / `tkhd` (milliseconds).
const MOVIE_TIMESCALE: u32 = 1000;

/// Writes an MP4 with one H.264 track to `W`, which is normally a file in the target directory.
pub struct Mp4Writer<W: Write + Seek> {
    out: W,
    track: Track,
    sizes: Vec<u32>,
    /// 1-based numbers of the sync samples.
    sync: Vec<u32>,
    /// Absolute offset where the reserved `moov` room begins (right after `ftyp`).
    moov_at: u64,
    /// Size of the reserved room (the `moov` plus a `free` box filling the rest).
    reserved: u64,
    /// Offset of the 16-byte `mdat` header.
    mdat_at: u64,
    data_len: u64,
}

fn bx(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(body.len() + 8);
    v.extend_from_slice(&((body.len() + 8) as u32).to_be_bytes());
    v.extend_from_slice(kind);
    v.extend_from_slice(body);
    v
}

fn full(kind: &[u8; 4], version: u8, flags: u32, body: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(body.len() + 4);
    b.extend_from_slice(&((u32::from(version) << 24) | (flags & 0xff_ffff)).to_be_bytes());
    b.extend_from_slice(body);
    bx(kind, &b)
}

fn be32(b: &mut Vec<u8>, v: u32) {
    b.extend_from_slice(&v.to_be_bytes());
}

fn be16(b: &mut Vec<u8>, v: u16) {
    b.extend_from_slice(&v.to_be_bytes());
}

/// The unity transform matrix of `mvhd` / `tkhd`.
fn matrix(b: &mut Vec<u8>) {
    for v in [0x1_0000, 0, 0, 0, 0x1_0000, 0, 0, 0, 0x4000_0000u32] {
        be32(b, v);
    }
}

/// `ftyp`: ISO base media with AVC, in the brands browsers and players expect.
fn ftyp() -> Vec<u8> {
    let mut b = b"isom".to_vec();
    be32(&mut b, 0x200);
    for brand in [b"isom", b"iso2", b"avc1", b"mp41"] {
        b.extend_from_slice(brand);
    }
    bx(b"ftyp", &b)
}

/// A `free` box of exactly `len` bytes (≥ 8).
fn free(len: u64) -> Vec<u8> {
    debug_assert!(len >= 8);
    let mut v = vec![0u8; len as usize];
    v[..4].copy_from_slice(&(len as u32).to_be_bytes());
    v[4..8].copy_from_slice(b"free");
    v
}

/// The `moov` box for `sizes` / `sync` with the single chunk at `chunk_offset`.
fn moov(track: &Track, sizes: &[u32], sync: &[u32], chunk_offset: u64) -> Vec<u8> {
    let n = sizes.len() as u32;
    let (timescale, delta) = track.timing();
    let media_duration = u64::from(n) * u64::from(delta);
    let movie_duration = media_duration * u64::from(MOVIE_TIMESCALE) / u64::from(timescale);
    // Version 1 headers only when a duration overflows 32 bits (never for a slideshow).
    let long = media_duration > u64::from(u32::MAX) || movie_duration > u64::from(u32::MAX);
    let times = |b: &mut Vec<u8>, scale: Option<u32>, duration: u64, id: Option<u32>| {
        // creation / modification time 0 ("unknown"): exports are reproducible byte for byte.
        if long {
            b.extend_from_slice(&[0; 16]);
        } else {
            b.extend_from_slice(&[0; 8]);
        }
        if let Some(id) = id {
            be32(b, id);
            be32(b, 0);
        }
        if let Some(s) = scale {
            be32(b, s);
        }
        if long {
            b.extend_from_slice(&duration.to_be_bytes());
        } else {
            be32(b, duration as u32);
        }
    };
    let version = u8::from(long);

    let mut mvhd = Vec::new();
    times(&mut mvhd, Some(MOVIE_TIMESCALE), movie_duration, None);
    be32(&mut mvhd, 0x1_0000); // rate 1.0
    be16(&mut mvhd, 0x100); // volume 1.0
    mvhd.extend_from_slice(&[0; 10]);
    matrix(&mut mvhd);
    mvhd.extend_from_slice(&[0; 24]);
    be32(&mut mvhd, 2); // next_track_ID

    let mut tkhd = Vec::new();
    times(&mut tkhd, None, movie_duration, Some(1));
    tkhd.extend_from_slice(&[0; 8]);
    be16(&mut tkhd, 0); // layer
    be16(&mut tkhd, 0); // alternate_group
    be16(&mut tkhd, 0); // volume (video)
    be16(&mut tkhd, 0);
    matrix(&mut tkhd);
    be32(&mut tkhd, track.width << 16);
    be32(&mut tkhd, track.height << 16);

    let mut mdhd = Vec::new();
    times(&mut mdhd, Some(timescale), media_duration, None);
    // Language "und" packed as three 5-bit letters.
    be16(&mut mdhd, 0x55c4);
    be16(&mut mdhd, 0);

    let mut hdlr = vec![0; 4];
    hdlr.extend_from_slice(b"vide");
    hdlr.extend_from_slice(&[0; 12]);
    hdlr.extend_from_slice(b"VideoHandler\0");

    // Visual sample entry.
    let mut avc1 = vec![0; 6];
    be16(&mut avc1, 1); // data_reference_index
    avc1.extend_from_slice(&[0; 16]);
    be16(&mut avc1, track.width as u16);
    be16(&mut avc1, track.height as u16);
    be32(&mut avc1, 0x48_0000); // 72 dpi
    be32(&mut avc1, 0x48_0000);
    be32(&mut avc1, 0);
    be16(&mut avc1, 1); // frame_count
    let mut name = [0u8; 32];
    let label = b"Specular H.264";
    name[0] = label.len() as u8;
    name[1..=label.len()].copy_from_slice(label);
    avc1.extend_from_slice(&name);
    be16(&mut avc1, 0x18); // depth
    be16(&mut avc1, 0xffff); // pre_defined −1
    avc1.extend(bx(b"avcC", &track.avcc));
    let c = track.colour;
    let mut colr = b"nclx".to_vec();
    be16(&mut colr, c.primaries);
    be16(&mut colr, c.transfer);
    be16(&mut colr, c.matrix);
    colr.push(if c.full_range { 0x80 } else { 0 });
    avc1.extend(bx(b"colr", &colr));
    let mut pasp = Vec::new();
    be32(&mut pasp, 1);
    be32(&mut pasp, 1);
    avc1.extend(bx(b"pasp", &pasp));

    let mut stsd = Vec::new();
    be32(&mut stsd, 1);
    stsd.extend(bx(b"avc1", &avc1));

    let mut stts = Vec::new();
    if n > 0 {
        be32(&mut stts, 1);
        be32(&mut stts, n);
        be32(&mut stts, delta);
    } else {
        be32(&mut stts, 0);
    }

    let mut stsz = Vec::with_capacity(8 + 4 * sizes.len());
    be32(&mut stsz, 0);
    be32(&mut stsz, n);
    for &s in sizes {
        be32(&mut stsz, s);
    }

    let mut stsc = Vec::new();
    if n > 0 {
        be32(&mut stsc, 1);
        for v in [1, n, 1] {
            be32(&mut stsc, v);
        }
    } else {
        be32(&mut stsc, 0);
    }

    let mut stbl = full(b"stsd", 0, 0, &stsd);
    stbl.extend(full(b"stts", 0, 0, &stts));
    if sync.len() < sizes.len() {
        let mut stss = Vec::with_capacity(4 + 4 * sync.len());
        be32(&mut stss, sync.len() as u32);
        for &s in sync {
            be32(&mut stss, s);
        }
        stbl.extend(full(b"stss", 0, 0, &stss));
    }
    stbl.extend(full(b"stsz", 0, 0, &stsz));
    stbl.extend(full(b"stsc", 0, 0, &stsc));
    let mut co = Vec::new();
    if n == 0 {
        be32(&mut co, 0);
        stbl.extend(full(b"stco", 0, 0, &co));
    } else if let Ok(off) = u32::try_from(chunk_offset) {
        be32(&mut co, 1);
        be32(&mut co, off);
        stbl.extend(full(b"stco", 0, 0, &co));
    } else {
        be32(&mut co, 1);
        co.extend_from_slice(&chunk_offset.to_be_bytes());
        stbl.extend(full(b"co64", 0, 0, &co));
    }

    let mut vmhd = Vec::new();
    vmhd.extend_from_slice(&[0; 8]); // graphicsmode copy, opcolor 0
    let mut dref = Vec::new();
    be32(&mut dref, 1);
    dref.extend(full(b"url ", 0, 1, &[])); // flag 1: media in this file
    let mut minf = full(b"vmhd", 0, 1, &vmhd);
    minf.extend(bx(b"dinf", &full(b"dref", 0, 0, &dref)));
    minf.extend(bx(b"stbl", &stbl));

    let mut mdia = full(b"mdhd", version, 0, &mdhd);
    mdia.extend(full(b"hdlr", 0, 0, &hdlr));
    mdia.extend(bx(b"minf", &minf));

    // tkhd flags: enabled | in movie | in preview.
    let mut trak = full(b"tkhd", version, 0x7, &tkhd);
    trak.extend(bx(b"mdia", &mdia));

    let mut body = full(b"mvhd", version, 0, &mvhd);
    body.extend(bx(b"trak", &trak));
    bx(b"moov", &body)
}

impl<W: Write + Seek> Mp4Writer<W> {
    /// Starts a movie at the current position of `out` (normally 0). `expected` is the number
    /// of samples the caller intends to write. It sizes the `moov` reservation that keeps the
    /// file faststart.
    pub fn new(mut out: W, track: Track, expected: u32) -> io::Result<Self> {
        let start = out.stream_position()?;
        let head = ftyp();
        out.write_all(&head)?;
        let moov_at = start + head.len() as u64;
        // The largest moov `expected` samples can need: every one a sync sample (which drops
        // stss — so also count a full stss), and a co64 offset.
        let n = expected as usize;
        let worst = moov(&track, &vec![0; n], &[], u64::MAX).len() as u64 + 8 + 4 * n as u64;
        // Room for the moov plus at least a minimal free box after it.
        let reserved = worst + 8;
        out.write_all(&free(reserved))?;
        let mdat_at = moov_at + reserved;
        // 64-bit mdat header; the size is patched in `finish`.
        out.write_all(&1u32.to_be_bytes())?;
        out.write_all(b"mdat")?;
        out.write_all(&16u64.to_be_bytes())?;
        Ok(Mp4Writer {
            out,
            track,
            sizes: Vec::with_capacity(n),
            sync: Vec::new(),
            moov_at,
            reserved,
            mdat_at,
            data_len: 0,
        })
    }

    /// Appends one coded frame: AVCC length-prefixed NAL units.
    pub fn write_sample(&mut self, data: &[u8], sync: bool) -> io::Result<()> {
        let size = u32::try_from(data.len())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "sample over 4 GiB"))?;
        self.out.write_all(data)?;
        self.sizes.push(size);
        if sync {
            self.sync.push(self.sizes.len() as u32);
        }
        self.data_len += u64::from(size);
        Ok(())
    }

    /// Replaces the track's `avcC` record (an encoder that switched mid-stream adds its
    /// parameter sets). A record too long for the reservation makes `moov` trail.
    pub fn set_avcc(&mut self, avcc: Vec<u8>) {
        self.track.avcc = avcc;
    }

    /// Samples written so far.
    pub fn samples(&self) -> u32 {
        self.sizes.len() as u32
    }

    /// Bytes of the movie so far (headers and samples).
    pub fn bytes(&self) -> u64 {
        self.mdat_at + 16 + self.data_len
    }

    /// Writes the sample tables, flushes and hands the output back.
    pub fn finish(mut self) -> io::Result<W> {
        let end = self.mdat_at + 16 + self.data_len;
        self.out.seek(SeekFrom::Start(self.mdat_at + 8))?;
        self.out.write_all(&(16 + self.data_len).to_be_bytes())?;
        let chunk = self.mdat_at + 16;
        let moov = moov(&self.track, &self.sizes, &self.sync, chunk);
        let moov_len = moov.len() as u64;
        if moov_len + 8 <= self.reserved {
            // The reservation is zero-filled already: moov, then a free header over the rest.
            let rest = (self.reserved - moov_len) as u32;
            self.out.seek(SeekFrom::Start(self.moov_at))?;
            self.out.write_all(&moov)?;
            self.out.write_all(&rest.to_be_bytes())?;
            self.out.write_all(b"free")?;
            self.out.seek(SeekFrom::Start(end))?;
        } else {
            // More samples than announced: the reservation stays a free box and moov trails.
            self.out.seek(SeekFrom::Start(end))?;
            self.out.write_all(&moov)?;
        }
        self.out.flush()?;
        Ok(self.out)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::mp4::{self, Codec};

    fn track() -> Track {
        Track {
            width: 1280,
            height: 720,
            fps_num: 30,
            fps_den: 1,
            avcc: vec![
                1, 0x42, 0xc0, 0x1f, 0xff, 0xe1, 0, 4, 0x67, 0x42, 0xc0, 0x1f, 1, 0, 2, 0x68, 0xce,
            ],
            colour: Colour {
                primaries: 1,
                transfer: 1,
                matrix: 1,
                full_range: false,
            },
        }
    }

    fn write(samples: &[(&[u8], bool)], expected: u32) -> Vec<u8> {
        let mut w = Mp4Writer::new(Cursor::new(Vec::new()), track(), expected).unwrap();
        for &(s, k) in samples {
            w.write_sample(s, k).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    fn top_level(data: &[u8]) -> Vec<[u8; 4]> {
        let mut out = Vec::new();
        let mut p = 0;
        while p + 8 <= data.len() {
            let mut size = u64::from(u32::from_be_bytes(data[p..p + 4].try_into().unwrap()));
            if size == 1 {
                size = u64::from_be_bytes(data[p + 8..p + 16].try_into().unwrap());
            }
            out.push(data[p + 4..p + 8].try_into().unwrap());
            p += size as usize;
        }
        assert_eq!(p, data.len(), "boxes tile the file");
        out
    }

    #[test]
    fn round_trips_through_the_demuxer() {
        let samples: Vec<(Vec<u8>, bool)> = (0..50u8)
            .map(|i| (vec![i; 10 + usize::from(i) * 3], i % 20 == 0))
            .collect();
        let refs: Vec<(&[u8], bool)> = samples.iter().map(|(s, k)| (&s[..], *k)).collect();
        let data = write(&refs, 50);
        assert_eq!(top_level(&data), [*b"ftyp", *b"moov", *b"free", *b"mdat"]);
        let m = mp4::parse(&data, true).unwrap();
        assert!(!m.has_audio);
        assert_eq!(m.duration_us(), Some(50 * 1_000_000 / 30));
        let v = m.video.unwrap();
        assert_eq!(v.codec, Some(Codec::H264));
        assert_eq!(v.config, track().avcc);
        assert_eq!((v.width, v.height), (1280, 720));
        assert_eq!(v.display, (1280, 720));
        assert_eq!(v.rotation, 0);
        assert_eq!(v.colour, Some(track().colour));
        assert_eq!(v.timescale, 90_000);
        assert_eq!(v.samples.len(), 50);
        for (i, (s, (bytes, key))) in v.samples.iter().zip(&samples).enumerate() {
            assert_eq!(mp4::sample_bytes(&data, s), Some(&bytes[..]), "sample {i}");
            assert_eq!(s.sync, *key, "sync {i}");
            assert_eq!(s.dts, i as u64 * 3000);
            assert_eq!(s.duration, 3000);
        }
    }

    #[test]
    fn all_sync_omits_stss_and_cancelled_runs_stay_valid() {
        // Fewer samples than announced (a cancelled export finalised anyway).
        let data = write(&[(b"abc", true), (b"defg", true)], 1000);
        let m = mp4::parse(&data, true).unwrap();
        let v = m.video.unwrap();
        assert!(v.samples.iter().all(|s| s.sync));
        assert_eq!(mp4::sample_bytes(&data, &v.samples[1]), Some(&b"defg"[..]));
        assert!(!data.windows(4).any(|w| w == b"stss"));
    }

    #[test]
    fn overflowing_the_reservation_moves_moov_to_the_end() {
        let samples: Vec<Vec<u8>> = (0..40u8).map(|i| vec![i; 7]).collect();
        let refs: Vec<(&[u8], bool)> = samples.iter().map(|s| (&s[..], false)).collect();
        let data = write(&refs, 2);
        assert_eq!(top_level(&data), [*b"ftyp", *b"free", *b"mdat", *b"moov"]);
        let v = mp4::parse(&data, true).unwrap().video.unwrap();
        assert_eq!(v.samples.len(), 40);
        assert_eq!(
            mp4::sample_bytes(&data, &v.samples[39]),
            Some(&samples[39][..])
        );
    }

    #[test]
    fn odd_rates_use_their_own_timescale() {
        let mut t = track();
        (t.fps_num, t.fps_den) = (30_000, 1001);
        assert_eq!(t.timing(), (90_000, 3003));
        (t.fps_num, t.fps_den) = (7, 1);
        assert_eq!(t.timing(), (7, 1));
        (t.fps_num, t.fps_den) = (24, 1);
        assert_eq!(t.timing(), (90_000, 3750));
    }
}
