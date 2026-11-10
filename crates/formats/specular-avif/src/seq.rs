//! AVIF image sequences (`avis`): the `moov` box, its `trak`s and their sample tables.
//!
//! Only what playback needs is kept: for the colour track (handler `pict`/`vide`) and an
//! optional alpha track (linked with a `tref`/`auxl` reference) the list of samples (file
//! offset, size, duration) plus the timescale, the `av1C` configuration, the canvas size and
//! the edit-list loop count.

use crate::Error;
use crate::bmff::{Boxes, Reader, find};
use crate::heif::{Av1Config, Nclx};

/// Most samples we accept in one track (bounds the sample table on hostile files).
const MAX_SAMPLES: usize = 1 << 20;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Sample {
    pub offset: u64,
    pub size: u32,
    /// Duration in media timescale units.
    pub duration: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct Track {
    pub id: u32,
    pub handler: [u8; 4],
    pub width: u32,
    pub height: u32,
    pub timescale: u32,
    pub samples: Vec<Sample>,
    pub config: Option<Av1Config>,
    pub nclx: Option<Nclx>,
    pub icc: Option<Vec<u8>>,
    /// `auxl` track references (this track is auxiliary to these track ids).
    pub auxl: Vec<u32>,
    /// Track duration in movie timescale units (`u64::MAX` = unknown/indefinite).
    pub duration: u64,
    /// Edit list: (repeat flag, first segment duration in movie timescale units).
    pub edit: Option<(bool, u64)>,
    /// Premultiplied alpha (`prem` reference).
    pub prem: Vec<u32>,
}

/// The parsed sequence: colour track, optional alpha track and loop count.
#[derive(Clone, Debug)]
pub(crate) struct Sequence {
    pub color: Track,
    pub alpha: Option<Track>,
    /// Total plays; 0 = infinite.
    pub loop_count: u32,
}

impl Sequence {
    pub(crate) fn parse(moov: &[u8]) -> Result<Sequence, Error> {
        let mut movie_timescale = 1u32;
        let mut tracks = Vec::new();
        for b in Boxes::new(moov) {
            let b = b?;
            match &b.typ {
                b"mvhd" => {
                    let mut r = Reader::new(b.payload);
                    let (v, _) = r.full()?;
                    if v == 1 {
                        r.skip(16)?;
                    } else {
                        r.skip(8)?;
                    }
                    movie_timescale = r.u32()?.max(1);
                }
                b"trak" => {
                    if tracks.len() >= 64 {
                        return Err(Error::Unsupported("too many tracks".into()));
                    }
                    if let Some(t) = parse_trak(b.payload)? {
                        tracks.push(t);
                    }
                }
                _ => {}
            }
        }
        let _ = movie_timescale;
        let is_aux = |t: &Track| &t.handler == b"auxv" || !t.auxl.is_empty();
        let color_idx = tracks
            .iter()
            .position(|t| !is_aux(t) && t.config.is_some() && !t.samples.is_empty())
            .ok_or_else(|| Error::Malformed("no AV1 video track".into()))?;
        let color = tracks.swap_remove(color_idx);
        let alpha = tracks
            .into_iter()
            .find(|t| t.auxl.contains(&color.id) && t.config.is_some() && !t.samples.is_empty());
        let loop_count = match color.edit {
            None => 0,
            Some((false, _)) => 1,
            Some((true, seg)) => {
                if color.duration == u64::MAX || seg == 0 {
                    0
                } else {
                    // The track duration covers all repetitions of the edit.
                    u32::try_from(color.duration.div_ceil(seg))
                        .unwrap_or(0)
                        .max(1)
                }
            }
        };
        Ok(Sequence {
            color,
            alpha,
            loop_count,
        })
    }
}

fn parse_trak(p: &[u8]) -> Result<Option<Track>, Error> {
    let mut t = Track {
        id: 0,
        handler: [0; 4],
        width: 0,
        height: 0,
        timescale: 1,
        samples: Vec::new(),
        config: None,
        nclx: None,
        icc: None,
        auxl: Vec::new(),
        duration: u64::MAX,
        edit: None,
        prem: Vec::new(),
    };
    let mut mdia = None;
    for b in Boxes::new(p) {
        let b = b?;
        match &b.typ {
            b"tkhd" => {
                let mut r = Reader::new(b.payload);
                let (v, _) = r.full()?;
                if v == 1 {
                    r.skip(16)?;
                    t.id = r.u32()?;
                    r.skip(4)?;
                    let d = r.u64()?;
                    t.duration = d;
                } else {
                    r.skip(8)?;
                    t.id = r.u32()?;
                    r.skip(4)?;
                    let d = r.u32()?;
                    t.duration = if d == u32::MAX {
                        u64::MAX
                    } else {
                        u64::from(d)
                    };
                }
                r.skip(8 + 2 + 2 + 2 + 2 + 36)?;
                t.width = r.u32()? >> 16;
                t.height = r.u32()? >> 16;
            }
            b"tref" => {
                for e in Boxes::new(b.payload) {
                    let e = e?;
                    let mut r = Reader::new(e.payload);
                    let mut ids = Vec::new();
                    while r.remaining() >= 4 && ids.len() < 64 {
                        ids.push(r.u32()?);
                    }
                    match &e.typ {
                        b"auxl" => t.auxl.extend(ids),
                        b"prem" => t.prem.extend(ids),
                        _ => {}
                    }
                }
            }
            b"edts" => {
                if let Some(elst) = find(b.payload, b"elst")? {
                    let mut r = Reader::new(elst);
                    let (v, flags) = r.full()?;
                    let n = r.u32()?;
                    if n >= 1 {
                        let seg = if v == 1 {
                            r.u64()?
                        } else {
                            u64::from(r.u32()?)
                        };
                        t.edit = Some((flags & 1 != 0, seg));
                    } else {
                        t.edit = Some((flags & 1 != 0, 0));
                    }
                }
            }
            b"mdia" => mdia = Some(b.payload),
            _ => {}
        }
    }
    let Some(mdia) = mdia else { return Ok(None) };
    let mut minf = None;
    for b in Boxes::new(mdia) {
        let b = b?;
        let mut r = Reader::new(b.payload);
        match &b.typ {
            b"mdhd" => {
                let (v, _) = r.full()?;
                r.skip(if v == 1 { 16 } else { 8 })?;
                t.timescale = r.u32()?.max(1);
            }
            b"hdlr" => {
                r.full()?;
                r.u32()?;
                t.handler = r.fourcc()?;
            }
            b"minf" => minf = Some(b.payload),
            _ => {}
        }
    }
    let Some(minf) = minf else { return Ok(None) };
    let Some(stbl) = find(minf, b"stbl")? else {
        return Ok(None);
    };
    parse_stbl(stbl, &mut t)?;
    Ok(Some(t))
}

fn parse_stbl(stbl: &[u8], t: &mut Track) -> Result<(), Error> {
    let mut durations: Vec<u32> = Vec::new();
    let mut sizes: Vec<u32> = Vec::new();
    let mut chunks: Vec<u64> = Vec::new();
    let mut stsc: Vec<(u32, u32)> = Vec::new();
    let cap = |n: usize, r: &Reader, per: usize| n.min(r.remaining() / per.max(1));
    for b in Boxes::new(stbl) {
        let b = b?;
        let mut r = Reader::new(b.payload);
        match &b.typ {
            b"stsd" => {
                r.full()?;
                let n = r.u32()?;
                if let Some(e) = Boxes::new(r.rest()).take(n as usize).next() {
                    let e = e?;
                    if &e.typ == b"av01" {
                        parse_av01(e.payload, t)?;
                    }
                }
            }
            b"stts" => {
                r.full()?;
                let n = r.u32()? as usize;
                for _ in 0..cap(n, &r, 8) {
                    let count = r.u32()? as usize;
                    let delta = r.u32()?;
                    if durations.len() + count > MAX_SAMPLES {
                        return Err(Error::Unsupported("too many samples".into()));
                    }
                    durations.resize(durations.len() + count, delta);
                }
            }
            b"stsz" => {
                r.full()?;
                let size = r.u32()?;
                let n = r.u32()? as usize;
                if n > MAX_SAMPLES {
                    return Err(Error::Unsupported("too many samples".into()));
                }
                if size != 0 {
                    sizes = vec![size; n];
                } else {
                    if n > r.remaining() / 4 {
                        return Err(Error::Malformed("stsz too short".into()));
                    }
                    sizes.reserve(n);
                    for _ in 0..n {
                        sizes.push(r.u32()?);
                    }
                }
            }
            b"stz2" => {
                r.full()?;
                r.skip(3)?;
                let field = r.u8()?;
                let n = r.u32()? as usize;
                if n > MAX_SAMPLES {
                    return Err(Error::Unsupported("too many samples".into()));
                }
                let bits = usize::from(field);
                if !matches!(bits, 4 | 8 | 16) || n.saturating_mul(bits).div_ceil(8) > r.remaining()
                {
                    return Err(Error::Malformed("bad stz2".into()));
                }
                let data = r.rest();
                sizes.reserve(n);
                for i in 0..n {
                    sizes.push(match bits {
                        4 => {
                            let byte = data[i / 2];
                            u32::from(if i % 2 == 0 { byte >> 4 } else { byte & 15 })
                        }
                        8 => u32::from(data[i]),
                        _ => u32::from(u16::from_be_bytes([data[2 * i], data[2 * i + 1]])),
                    });
                }
            }
            b"stco" | b"co64" => {
                r.full()?;
                let n = r.u32()? as usize;
                let wide = &b.typ == b"co64";
                let per = if wide { 8 } else { 4 };
                if n > r.remaining() / per {
                    return Err(Error::Malformed("chunk offset table too short".into()));
                }
                chunks.reserve(n);
                for _ in 0..n {
                    chunks.push(if wide { r.u64()? } else { u64::from(r.u32()?) });
                }
            }
            b"stsc" => {
                r.full()?;
                let n = r.u32()? as usize;
                for _ in 0..cap(n, &r, 12) {
                    let first = r.u32()?;
                    let per = r.u32()?;
                    r.u32()?;
                    stsc.push((first, per));
                }
            }
            _ => {}
        }
    }
    // Lay the samples out over the chunks.
    let total = sizes.len();
    let mut samples = Vec::with_capacity(total);
    let mut s = 0usize;
    // `stsc` runs are in increasing first-chunk order: walk them alongside the chunks (a
    // search per chunk is quadratic in the table sizes on hostile files).
    let mut run = 0usize;
    for (ci, &chunk_off) in chunks.iter().enumerate() {
        if s >= total {
            break;
        }
        let chunk_no = ci as u64 + 1;
        while stsc
            .get(run + 1)
            .is_some_and(|&(first, _)| u64::from(first) <= chunk_no)
        {
            run += 1;
        }
        let per = stsc
            .get(run)
            .filter(|&&(first, _)| u64::from(first) <= chunk_no)
            .map_or(1, |&(_, per)| per) as usize;
        let mut off = chunk_off;
        for _ in 0..per {
            if s >= total {
                break;
            }
            let size = sizes[s];
            samples.push(Sample {
                offset: off,
                size,
                duration: durations.get(s).copied().unwrap_or(0),
            });
            off = off.saturating_add(u64::from(size));
            s += 1;
        }
    }
    t.samples = samples;
    Ok(())
}

/// `av01` VisualSampleEntry: fixed 78-byte header, then child boxes (`av1C`, `colr`, ...).
fn parse_av01(p: &[u8], t: &mut Track) -> Result<(), Error> {
    let mut r = Reader::new(p);
    r.skip(6 + 2 + 16)?;
    let w = u32::from(r.u16()?);
    let h = u32::from(r.u16()?);
    if w > 0 && h > 0 {
        t.width = w;
        t.height = h;
    }
    r.skip(78 - 28)?;
    for b in Boxes::new(r.rest()) {
        let Ok(b) = b else { break };
        match &b.typ {
            b"av1C" => t.config = Some(Av1Config::parse(b.payload)?),
            b"colr" => {
                let mut r = Reader::new(b.payload);
                match &r.fourcc()? {
                    b"nclx" => {
                        let primaries = r.u16()?;
                        let transfer = r.u16()?;
                        let matrix = r.u16()?;
                        let full_range = r.u8()? & 0x80 != 0;
                        t.nclx = Some(Nclx {
                            primaries,
                            transfer,
                            matrix,
                            full_range,
                        });
                    }
                    b"prof" | b"rICC" => t.icc = Some(r.rest().to_vec()),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    Ok(())
}
