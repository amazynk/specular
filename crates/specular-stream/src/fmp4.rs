//! Fragmented MP4 (CMAF: an initialisation segment, then `moof`+`mdat` fragments, as HLS
//! and DASH send them) rewritten as a plain MP4 with its sample tables in `moov`, which the
//! in-tree demuxer reads. Several inputs (DASH's picture and sound come apart) become the
//! tracks of one file.
//!
//! The tracks keep their sample descriptions (`stsd`) byte for byte; `stts`, `ctts`, `stss`,
//! `stsc`, `stsz` and `co64` are written from the fragments' `trun`s; an edit list keeps the
//! tracks in step when one starts later than another. Encrypted tracks (`encv`, `enca`) are
//! refused.

/// Why fragments could not be rewritten.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The initialisation segment has no `moov` with tracks.
    NoInit,
    /// A sample description is encrypted (Common Encryption).
    Encrypted,
    /// A fragment is cut short or inconsistent.
    Malformed(&'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NoInit => f.write_str("no initialisation segment"),
            Error::Encrypted => f.write_str("the stream is encrypted (DRM)"),
            Error::Malformed(w) => write!(f, "a broken fragment ({w})"),
        }
    }
}

type Result<T> = std::result::Result<T, Error>;

fn be32(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

fn be64(d: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(d.get(at..at + 8)?.try_into().ok()?))
}

/// The boxes in `d`: `(type, start, header length, end)`.
fn boxes(d: &[u8]) -> Vec<([u8; 4], usize, usize, usize)> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 8 <= d.len() {
        let size = be32(d, at).unwrap_or(0) as u64;
        let mut kind = [0u8; 4];
        kind.copy_from_slice(&d[at + 4..at + 8]);
        let (size, head) = match size {
            0 => ((d.len() - at) as u64, 8),
            1 => match be64(d, at + 8) {
                Some(s) => (s, 16),
                None => break,
            },
            s => (s, 8),
        };
        if size < head as u64 || at as u64 + size > d.len() as u64 {
            break;
        }
        let end = at + size as usize;
        out.push((kind, at, head, end));
        at = end;
    }
    out
}

fn find<'a>(d: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    boxes(d)
        .into_iter()
        .find(|b| &b.0 == kind)
        .map(|(_, s, h, e)| &d[s + h..e])
}

fn find_box<'a>(d: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    boxes(d)
        .into_iter()
        .find(|b| &b.0 == kind)
        .map(|(_, s, _, e)| &d[s..e])
}

#[derive(Clone, Copy, Debug, Default)]
struct Defaults {
    duration: u32,
    size: u32,
    flags: u32,
}

#[derive(Clone, Copy, Debug)]
struct Sample {
    size: u32,
    duration: u32,
    cts: i64,
    sync: bool,
    /// Where its bytes are in the output's `mdat` payload.
    at: u64,
}

#[derive(Clone, Debug)]
struct Track {
    input: usize,
    source_id: u32,
    /// The `trak` box as the init has it.
    trak: Vec<u8>,
    timescale: u32,
    defaults: Defaults,
    samples: Vec<Sample>,
    first_dts: Option<u64>,
    next_dts: u64,
    /// `(first sample, count)` per run (a chunk each).
    chunks: Vec<(usize, usize)>,
}

/// Collects fragments and writes them out as one plain MP4.
#[derive(Clone, Debug, Default)]
pub struct Defrag {
    tracks: Vec<Track>,
    data: Vec<u8>,
}

impl Defrag {
    pub fn new() -> Defrag {
        Defrag::default()
    }

    /// The initialisation segment of input `input` (each input once).
    pub fn add_init(&mut self, input: usize, init: &[u8]) -> Result<()> {
        if self.tracks.iter().any(|t| t.input == input) {
            return Ok(());
        }
        let moov = find(init, b"moov").ok_or(Error::NoInit)?;
        let mut trex = Vec::new();
        if let Some(mvex) = find(moov, b"mvex") {
            for (k, s, h, e) in boxes(mvex) {
                if &k == b"trex" {
                    let b = &mvex[s + h..e];
                    trex.push((
                        be32(b, 4).unwrap_or(0),
                        Defaults {
                            duration: be32(b, 12).unwrap_or(0),
                            size: be32(b, 16).unwrap_or(0),
                            flags: be32(b, 20).unwrap_or(0),
                        },
                    ));
                }
            }
        }
        let mut added = 0;
        for (k, s, h, e) in boxes(moov) {
            if &k != b"trak" {
                continue;
            }
            let trak = &moov[s..e];
            let body = &moov[s + h..e];
            let tkhd = find(body, b"tkhd").ok_or(Error::NoInit)?;
            let id = if tkhd.first() == Some(&1) {
                be32(tkhd, 20)
            } else {
                be32(tkhd, 12)
            }
            .ok_or(Error::NoInit)?;
            let mdia = find(body, b"mdia").ok_or(Error::NoInit)?;
            let mdhd = find(mdia, b"mdhd").ok_or(Error::NoInit)?;
            let timescale = if mdhd.first() == Some(&1) {
                be32(mdhd, 20)
            } else {
                be32(mdhd, 12)
            }
            .ok_or(Error::NoInit)?
            .max(1);
            let stsd = find(mdia, b"minf")
                .and_then(|m| find(m, b"stbl"))
                .and_then(|s| find(s, b"stsd"))
                .ok_or(Error::NoInit)?;
            // Entries start after the full-box header and the count.
            if stsd.len() >= 16 && matches!(&stsd[12..16], b"encv" | b"enca" | b"encs" | b"enct") {
                return Err(Error::Encrypted);
            }
            self.tracks.push(Track {
                input,
                source_id: id,
                trak: trak.to_vec(),
                timescale,
                defaults: trex
                    .iter()
                    .find(|(i, _)| *i == id)
                    .map(|t| t.1)
                    .unwrap_or_default(),
                samples: Vec::new(),
                first_dts: None,
                next_dts: 0,
                chunks: Vec::new(),
            });
            added += 1;
        }
        if added == 0 {
            return Err(Error::NoInit);
        }
        Ok(())
    }

    /// A media segment of input `input`: one or more `moof`+`mdat` pairs (`styp`, `sidx`,
    /// `emsg` and the like are skipped).
    pub fn add_fragment(&mut self, input: usize, seg: &[u8]) -> Result<()> {
        for (k, s, h, e) in boxes(seg) {
            if &k == b"moof" {
                self.moof(input, seg, s, s + h, e)?;
            }
        }
        Ok(())
    }

    fn moof(
        &mut self,
        input: usize,
        seg: &[u8],
        start: usize,
        body: usize,
        end: usize,
    ) -> Result<()> {
        let moof = &seg[body..end];
        for (k, s, h, e) in boxes(moof) {
            if &k != b"traf" {
                continue;
            }
            let traf = &moof[s + h..e];
            let tfhd = find(traf, b"tfhd").ok_or(Error::Malformed("no tfhd"))?;
            let flags = be32(tfhd, 0).ok_or(Error::Malformed("tfhd"))? & 0xFF_FFFF;
            let id = be32(tfhd, 4).ok_or(Error::Malformed("tfhd"))?;
            let Some(ti) = self
                .tracks
                .iter()
                .position(|t| t.input == input && t.source_id == id)
            else {
                continue;
            };
            let mut at = 8;
            let mut base = start as u64;
            if flags & 0x1 != 0 {
                base = be64(tfhd, at).ok_or(Error::Malformed("tfhd"))?;
                at += 8;
            }
            if flags & 0x2 != 0 {
                at += 4;
            }
            let mut d = self.tracks[ti].defaults;
            if flags & 0x8 != 0 {
                d.duration = be32(tfhd, at).ok_or(Error::Malformed("tfhd"))?;
                at += 4;
            }
            if flags & 0x10 != 0 {
                d.size = be32(tfhd, at).ok_or(Error::Malformed("tfhd"))?;
                at += 4;
            }
            if flags & 0x20 != 0 {
                d.flags = be32(tfhd, at).ok_or(Error::Malformed("tfhd"))?;
            }
            if let Some(tfdt) = find(traf, b"tfdt") {
                let dt = if tfdt.first() == Some(&1) {
                    be64(tfdt, 4)
                } else {
                    be32(tfdt, 4).map(u64::from)
                }
                .ok_or(Error::Malformed("tfdt"))?;
                let t = &mut self.tracks[ti];
                match t.first_dts {
                    None => {
                        t.first_dts = Some(dt);
                        t.next_dts = dt;
                    }
                    Some(_) if dt > t.next_dts => {
                        // A gap: the last sample lasts until this fragment starts.
                        let gap = dt - t.next_dts;
                        if let Some(last) = t.samples.last_mut() {
                            last.duration = last
                                .duration
                                .saturating_add(gap.min(u32::MAX as u64) as u32);
                        }
                        t.next_dts = dt;
                    }
                    Some(_) => {}
                }
            }
            let mut data_at = base;
            for (k2, s2, h2, e2) in boxes(traf) {
                if &k2 != b"trun" {
                    continue;
                }
                let tr = &traf[s2 + h2..e2];
                let vf = be32(tr, 0).ok_or(Error::Malformed("trun"))?;
                let (version, tf) = (vf >> 24, vf & 0xFF_FFFF);
                let count = be32(tr, 4).ok_or(Error::Malformed("trun"))? as usize;
                let mut p = 8;
                if tf & 0x1 != 0 {
                    let off = be32(tr, p).ok_or(Error::Malformed("trun"))? as i32;
                    data_at = (base as i64 + i64::from(off)) as u64;
                    p += 4;
                }
                let first_flags = if tf & 0x4 != 0 {
                    let f = be32(tr, p);
                    p += 4;
                    f
                } else {
                    None
                };
                let per = [0x100, 0x200, 0x400, 0x800]
                    .iter()
                    .filter(|&&b| tf & b != 0)
                    .count()
                    * 4;
                if per > 0 && count.saturating_mul(per) > tr.len().saturating_sub(p) {
                    return Err(Error::Malformed("trun count"));
                }
                let t = &mut self.tracks[ti];
                let first_sample = t.samples.len();
                for i in 0..count {
                    let mut f = |bit: u32| -> Option<u32> {
                        if tf & bit != 0 {
                            let v = be32(tr, p);
                            p += 4;
                            v
                        } else {
                            None
                        }
                    };
                    let duration = f(0x100).unwrap_or(d.duration);
                    let size = f(0x200).unwrap_or(d.size);
                    let sflags = f(0x400)
                        .or(if i == 0 { first_flags } else { None })
                        .unwrap_or(d.flags);
                    let cts = f(0x800).map_or(0, |c| {
                        if version == 0 {
                            i64::from(c)
                        } else {
                            i64::from(c as i32)
                        }
                    });
                    let lo = usize::try_from(data_at).map_err(|_| Error::Malformed("offset"))?;
                    let hi = lo
                        .checked_add(size as usize)
                        .filter(|&h| h <= seg.len())
                        .ok_or(Error::Malformed("sample past the segment"))?;
                    let out_at = self.data.len() as u64;
                    self.data.extend_from_slice(&seg[lo..hi]);
                    t.samples.push(Sample {
                        size,
                        duration,
                        cts,
                        sync: sflags & 0x0001_0000 == 0,
                        at: out_at,
                    });
                    t.next_dts += u64::from(duration);
                    data_at += u64::from(size);
                }
                if count > 0 {
                    t.chunks.push((first_sample, count));
                }
            }
        }
        Ok(())
    }

    /// Seconds of the longest track.
    pub fn duration(&self) -> f64 {
        self.tracks
            .iter()
            .map(|t| {
                t.samples.iter().map(|s| u64::from(s.duration)).sum::<u64>() as f64
                    / f64::from(t.timescale)
            })
            .fold(0.0, f64::max)
    }

    /// Whether any sample was added.
    pub fn is_empty(&self) -> bool {
        self.tracks.iter().all(|t| t.samples.is_empty())
    }

    /// The plain MP4: `ftyp`, `moov` (before the data, so it plays as it is read), `mdat`.
    pub fn build(&self) -> Vec<u8> {
        const MOVIE_TS: u32 = 1000;
        let tracks: Vec<&Track> = self
            .tracks
            .iter()
            .filter(|t| !t.samples.is_empty())
            .collect();
        let start = |t: &Track| t.first_dts.unwrap_or(0) as f64 / f64::from(t.timescale);
        let t0 = tracks
            .iter()
            .map(|t| start(t))
            .fold(f64::INFINITY, f64::min);
        let ftyp = boxed(
            b"ftyp",
            &[
                b"isom".as_slice(),
                &512u32.to_be_bytes(),
                b"isomiso2avc1mp41",
            ]
            .concat(),
        );
        // The moov's size does not depend on the chunk offsets (always 64-bit): build it
        // once with offset 0 to measure, then for real.
        let moov_for = |data_start: u64| {
            let mut traks = Vec::new();
            let mut longest = 0u64;
            for (i, t) in tracks.iter().enumerate() {
                let media: u64 = t.samples.iter().map(|s| u64::from(s.duration)).sum();
                let delay = ((start(t) - t0).max(0.0) * f64::from(MOVIE_TS)).round() as u64;
                let presented = media * u64::from(MOVIE_TS) / u64::from(t.timescale);
                longest = longest.max(delay + presented);
                traks.push(rewrite_trak(t, i as u32 + 1, delay, presented, data_start));
            }
            let mut body = mvhd(MOVIE_TS, longest, tracks.len() as u32 + 1);
            for t in traks {
                body.extend(t);
            }
            boxed(b"moov", &body)
        };
        let moov_len = moov_for(0).len() as u64;
        let data_start = ftyp.len() as u64 + moov_len + 16;
        let moov = moov_for(data_start);
        let mut out = Vec::with_capacity(ftyp.len() + moov.len() + 16 + self.data.len());
        out.extend(ftyp);
        out.extend(moov);
        out.extend_from_slice(&1u32.to_be_bytes());
        out.extend_from_slice(b"mdat");
        out.extend_from_slice(&(16 + self.data.len() as u64).to_be_bytes());
        out.extend_from_slice(&self.data);
        out
    }
}

fn boxed(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(body.len() + 8);
    b.extend_from_slice(&(body.len() as u32 + 8).to_be_bytes());
    b.extend_from_slice(kind);
    b.extend_from_slice(body);
    b
}

fn full(kind: &[u8; 4], version: u8, flags: u32, body: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(body.len() + 4);
    b.extend_from_slice(&((u32::from(version) << 24) | flags).to_be_bytes());
    b.extend_from_slice(body);
    boxed(kind, &b)
}

const MATRIX: [u32; 9] = [0x0001_0000, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000];

fn mvhd(timescale: u32, duration: u64, next_id: u32) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&[0; 16]); // creation, modification
    b.extend_from_slice(&timescale.to_be_bytes());
    b.extend_from_slice(&duration.to_be_bytes());
    b.extend_from_slice(&0x0001_0000u32.to_be_bytes()); // rate
    b.extend_from_slice(&0x0100u16.to_be_bytes()); // volume
    b.extend_from_slice(&[0; 10]);
    for m in MATRIX {
        b.extend_from_slice(&m.to_be_bytes());
    }
    b.extend_from_slice(&[0; 24]);
    b.extend_from_slice(&next_id.to_be_bytes());
    full(b"mvhd", 1, 0, &b)
}

/// Run-length pairs of `values`.
fn runs<T: PartialEq + Copy>(values: impl Iterator<Item = T>) -> Vec<(u32, T)> {
    let mut out: Vec<(u32, T)> = Vec::new();
    for v in values {
        match out.last_mut() {
            Some((n, last)) if *last == v => *n += 1,
            _ => out.push((1, v)),
        }
    }
    out
}

fn stbl(t: &Track, stsd: &[u8], data_start: u64) -> Vec<u8> {
    let s = &t.samples;
    let mut body = stsd.to_vec();
    let mut b = Vec::new();
    let r = runs(s.iter().map(|x| x.duration));
    b.extend_from_slice(&(r.len() as u32).to_be_bytes());
    for (n, d) in r {
        b.extend_from_slice(&n.to_be_bytes());
        b.extend_from_slice(&d.to_be_bytes());
    }
    body.extend(full(b"stts", 0, 0, &b));
    if s.iter().any(|x| x.cts != 0) {
        let neg = s.iter().any(|x| x.cts < 0);
        let r = runs(s.iter().map(|x| x.cts));
        let mut b = Vec::new();
        b.extend_from_slice(&(r.len() as u32).to_be_bytes());
        for (n, c) in r {
            b.extend_from_slice(&n.to_be_bytes());
            b.extend_from_slice(&(c as i32).to_be_bytes());
        }
        body.extend(full(b"ctts", u8::from(neg), 0, &b));
    }
    if s.iter().any(|x| !x.sync) {
        let sync: Vec<u32> = (0..s.len())
            .filter(|&i| s[i].sync)
            .map(|i| i as u32 + 1)
            .collect();
        let mut b = Vec::new();
        b.extend_from_slice(&(sync.len() as u32).to_be_bytes());
        for i in sync {
            b.extend_from_slice(&i.to_be_bytes());
        }
        body.extend(full(b"stss", 0, 0, &b));
    }
    // One chunk per run.
    let mut b = Vec::new();
    let per = runs(t.chunks.iter().map(|c| c.1));
    b.extend_from_slice(&(per.len() as u32).to_be_bytes());
    let mut chunk = 1u32;
    for (n, count) in per {
        b.extend_from_slice(&chunk.to_be_bytes());
        b.extend_from_slice(&(count as u32).to_be_bytes());
        b.extend_from_slice(&1u32.to_be_bytes());
        chunk += n;
    }
    body.extend(full(b"stsc", 0, 0, &b));
    let mut b = Vec::new();
    b.extend_from_slice(&0u32.to_be_bytes());
    b.extend_from_slice(&(s.len() as u32).to_be_bytes());
    for x in s {
        b.extend_from_slice(&x.size.to_be_bytes());
    }
    body.extend(full(b"stsz", 0, 0, &b));
    let mut b = Vec::new();
    b.extend_from_slice(&(t.chunks.len() as u32).to_be_bytes());
    for &(first, _) in &t.chunks {
        b.extend_from_slice(&(data_start + s[first].at).to_be_bytes());
    }
    body.extend(full(b"co64", 0, 0, &b));
    boxed(b"stbl", &body)
}

/// The track's `trak` with its id, durations, edit list and sample tables rewritten.
fn rewrite_trak(t: &Track, id: u32, delay: u64, presented: u64, data_start: u64) -> Vec<u8> {
    let media: u64 = t.samples.iter().map(|s| u64::from(s.duration)).sum();
    // The first sample shown (the composition offset of B-frame streams) is where the
    // presentation starts.
    let first_cts = t
        .samples
        .iter()
        .scan(0i64, |dts, s| {
            let c = *dts + s.cts;
            *dts += i64::from(s.duration);
            Some(c)
        })
        .min()
        .unwrap_or(0)
        .max(0);
    let body = &t.trak[8..];
    let mut out = Vec::new();
    for (k, s, _, e) in boxes(body) {
        let b = &body[s..e];
        match &k {
            b"tkhd" => {
                let mut b = b.to_vec();
                if b.get(8) == Some(&1) {
                    b[28..32].copy_from_slice(&id.to_be_bytes());
                    b[36..44].copy_from_slice(&presented.to_be_bytes());
                } else if b.len() >= 32 {
                    b[20..24].copy_from_slice(&id.to_be_bytes());
                    b[28..32]
                        .copy_from_slice(&(presented.min(u32::MAX as u64) as u32).to_be_bytes());
                }
                out.extend(b);
                let mut el = Vec::new();
                let entries = if delay > 0 { 2u32 } else { 1 };
                el.extend_from_slice(&entries.to_be_bytes());
                if delay > 0 {
                    el.extend_from_slice(&delay.to_be_bytes());
                    el.extend_from_slice(&(-1i64).to_be_bytes());
                    el.extend_from_slice(&0x0001_0000u32.to_be_bytes());
                }
                el.extend_from_slice(&presented.to_be_bytes());
                el.extend_from_slice(&first_cts.to_be_bytes());
                el.extend_from_slice(&0x0001_0000u32.to_be_bytes());
                out.extend(boxed(b"edts", &full(b"elst", 1, 0, &el)));
            }
            b"edts" => {}
            b"mdia" => out.extend(rewrite_mdia(t, b, media, data_start)),
            _ => out.extend_from_slice(b),
        }
    }
    boxed(b"trak", &out)
}

fn rewrite_mdia(t: &Track, mdia: &[u8], media: u64, data_start: u64) -> Vec<u8> {
    let body = &mdia[8..];
    let mut out = Vec::new();
    for (k, s, _, e) in boxes(body) {
        let b = &body[s..e];
        match &k {
            b"mdhd" => {
                let mut b = b.to_vec();
                if b.get(8) == Some(&1) && b.len() >= 40 {
                    b[32..40].copy_from_slice(&media.to_be_bytes());
                } else if b.len() >= 28 {
                    b[24..28].copy_from_slice(&(media.min(u32::MAX as u64) as u32).to_be_bytes());
                }
                out.extend(b);
            }
            b"minf" => {
                let mb = &b[8..];
                let mut m = Vec::new();
                for (k2, s2, _, e2) in boxes(mb) {
                    let c = &mb[s2..e2];
                    if &k2 == b"stbl" {
                        let stsd = find_box(&c[8..], b"stsd").unwrap_or(&[]);
                        m.extend(stbl(t, stsd, data_start));
                    } else {
                        m.extend_from_slice(c);
                    }
                }
                out.extend(boxed(b"minf", &m));
            }
            _ => out.extend_from_slice(b),
        }
    }
    boxed(b"mdia", &out)
}

/// Whether `data` starts like a fragmented MP4 segment or initialisation segment.
pub fn sniff(data: &[u8]) -> bool {
    matches!(
        data.get(4..8),
        Some(b"ftyp" | b"styp" | b"moof" | b"sidx" | b"moov" | b"emsg" | b"prft")
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A one-track init (`mp4a`-less: a made-up sample entry is enough for the tables).
    pub(crate) fn init(id: u32, timescale: u32, entry: &[u8; 4]) -> Vec<u8> {
        let tkhd = full(b"tkhd", 0, 3, &{
            let mut b = vec![0; 8];
            b.extend_from_slice(&id.to_be_bytes());
            b.extend_from_slice(&[0; 4 + 4 + 8 + 4 + 36 + 8]);
            b
        });
        let mdhd = full(b"mdhd", 0, 0, &{
            let mut b = vec![0; 8];
            b.extend_from_slice(&timescale.to_be_bytes());
            b.extend_from_slice(&[0; 8]);
            b
        });
        let mut stsd_body = 1u32.to_be_bytes().to_vec();
        stsd_body.extend(boxed(entry, &[0; 8]));
        let stbl = boxed(
            b"stbl",
            &[
                full(b"stsd", 0, 0, &stsd_body),
                full(b"stts", 0, 0, &[0; 4]),
                full(b"stsc", 0, 0, &[0; 4]),
                full(b"stsz", 0, 0, &[0; 8]),
                full(b"stco", 0, 0, &[0; 4]),
            ]
            .concat(),
        );
        let minf = boxed(b"minf", &stbl);
        let mdia = boxed(
            b"mdia",
            &[mdhd, full(b"hdlr", 0, 0, &[0; 20]), minf].concat(),
        );
        let trak = boxed(b"trak", &[tkhd, mdia].concat());
        let trex = full(b"trex", 0, 0, &{
            let mut b = id.to_be_bytes().to_vec();
            b.extend_from_slice(&1u32.to_be_bytes());
            b.extend_from_slice(&1024u32.to_be_bytes());
            b.extend_from_slice(&0u32.to_be_bytes());
            b.extend_from_slice(&0u32.to_be_bytes());
            b
        });
        let moov = boxed(
            b"moov",
            &[mvhd(1000, 0, 2), trak, boxed(b"mvex", &trex)].concat(),
        );
        [boxed(b"ftyp", b"iso6\0\0\0\0iso6"), moov].concat()
    }

    /// A fragment of `samples` (size, sync) starting at `dts`, default duration from trex.
    pub(crate) fn fragment(id: u32, dts: u64, samples: &[(u32, bool)], fill: u8) -> Vec<u8> {
        let tfhd = full(b"tfhd", 0, 0x02_0000, &id.to_be_bytes());
        let tfdt = full(b"tfdt", 1, 0, &dts.to_be_bytes());
        let trun_len = 8 + 4 + 4 + 4 + samples.len() * 8;
        let traf_len = 8 + tfhd.len() + tfdt.len() + trun_len;
        let moof_len = 8 + 16 + traf_len;
        let mut tr = (samples.len() as u32).to_be_bytes().to_vec();
        tr.extend_from_slice(&((moof_len + 8) as u32).to_be_bytes());
        for &(size, sync) in samples {
            tr.extend_from_slice(&size.to_be_bytes());
            tr.extend_from_slice(&(if sync { 0u32 } else { 0x0001_0000 }).to_be_bytes());
        }
        let trun = full(b"trun", 0, 0x1 | 0x200 | 0x400, &tr);
        let traf = boxed(b"traf", &[tfhd, tfdt, trun].concat());
        let moof = boxed(
            b"moof",
            &[full(b"mfhd", 0, 0, &1u32.to_be_bytes()), traf].concat(),
        );
        assert_eq!(moof.len(), moof_len);
        let total: u32 = samples.iter().map(|s| s.0).sum();
        let mut data = Vec::new();
        for (i, &(size, _)) in samples.iter().enumerate() {
            data.extend(std::iter::repeat_n(
                fill.wrapping_add(i as u8),
                size as usize,
            ));
        }
        assert_eq!(data.len() as u32, total);
        [
            boxed(b"styp", b"msdh\0\0\0\0msdh"),
            moof,
            boxed(b"mdat", &data),
        ]
        .concat()
    }

    /// Tables written from fragments point at the right bytes; a later track gets a delay.
    #[test]
    fn fragments_become_sample_tables() {
        let mut d = Defrag::new();
        d.add_init(0, &init(1, 48_000, b"mp4a")).unwrap();
        d.add_init(1, &init(1, 90_000, b"avc1")).unwrap();
        // Sound from 0, picture from 0.5 s (in its own timescale).
        let f0 = fragment(1, 0, &[(10, true), (11, true), (12, true)], 0x10);
        let f1 = fragment(1, 3 * 1024, &[(13, true)], 0x20);
        let v0 = fragment(1, 45_000, &[(100, true), (50, false)], 0x40);
        // Each fragment is offset in its segment (the styp first) and the data follow
        // the moof: the bytes must come out right.
        d.add_fragment(0, &f0).unwrap();
        d.add_fragment(1, &v0).unwrap();
        d.add_fragment(0, &f1).unwrap();
        assert!((d.duration() - 4.0 * 1024.0 / 48_000.0).abs() < 1e-9);
        let out = d.build();
        let moov = find(&out, b"moov").unwrap();
        let traks: Vec<&[u8]> = boxes(moov)
            .into_iter()
            .filter(|b| &b.0 == b"trak")
            .map(|(_, s, h, e)| &moov[s + h..e])
            .collect();
        assert_eq!(traks.len(), 2);
        let tables = |trak: &[u8]| {
            let stbl = find(
                find(find(trak, b"mdia").unwrap(), b"minf").unwrap(),
                b"stbl",
            )
            .unwrap();
            let stsz = find(stbl, b"stsz").unwrap();
            let n = be32(stsz, 8).unwrap() as usize;
            let sizes: Vec<u32> = (0..n).map(|i| be32(stsz, 12 + 4 * i).unwrap()).collect();
            let co64 = find(stbl, b"co64").unwrap();
            let chunks: Vec<u64> = (0..be32(co64, 4).unwrap() as usize)
                .map(|i| be64(co64, 8 + 8 * i).unwrap())
                .collect();
            (
                sizes,
                chunks,
                find(stbl, b"stss").is_some(),
                find(trak, b"edts").map(|e| e.to_vec()),
            )
        };
        let (sizes, chunks, stss, _) = tables(traks[0]);
        assert_eq!(sizes, vec![10, 11, 12, 13]);
        assert_eq!(chunks.len(), 2);
        assert!(!stss, "every sound sample is a sync sample");
        assert_eq!(out[chunks[0] as usize], 0x10);
        assert_eq!(out[chunks[0] as usize + 10], 0x11);
        assert_eq!(out[chunks[1] as usize], 0x20);
        let (sizes, chunks, stss, edts) = tables(traks[1]);
        assert_eq!(sizes, vec![100, 50]);
        assert!(stss);
        assert_eq!(out[chunks[0] as usize + 100], 0x41);
        // An empty edit of 500 ms (movie timescale 1000) first.
        let elst = find(&edts.unwrap(), b"elst").unwrap().to_vec();
        assert_eq!(be32(&elst, 4), Some(2));
        assert_eq!(be64(&elst, 8), Some(500));
        assert_eq!(be64(&elst, 16).map(|v| v as i64), Some(-1));
        // The rewritten file's boxes add up.
        let top: Vec<[u8; 4]> = boxes(&out).iter().map(|b| b.0).collect();
        assert_eq!(top, [*b"ftyp", *b"moov", *b"mdat"]);
    }

    #[test]
    fn encrypted_and_broken_input_is_refused() {
        let mut d = Defrag::new();
        assert_eq!(
            d.add_init(0, &init(1, 1000, b"encv")),
            Err(Error::Encrypted)
        );
        assert_eq!(d.add_init(0, b"junk"), Err(Error::NoInit));
        d.add_init(0, &init(1, 1000, b"mp4a")).unwrap();
        let mut f = fragment(1, 0, &[(10, true)], 1);
        f.truncate(f.len() - 4);
        assert!(d.add_fragment(0, &f).is_err());
        assert!(sniff(&init(1, 1, b"mp4a")));
        assert!(!sniff(&[0x47; 200]));
    }
}
