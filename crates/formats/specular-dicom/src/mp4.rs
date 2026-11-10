//! Video pixel data that holds an MP4 (ISO-BMFF) file instead of an elementary stream.
//!
//! PS3.5 wants an Annex B stream in the fragments, but some writers store the whole `.mp4`.
//! Its first video track is read without copying: the parameter sets in its `avcC` / `hvcC`
//! record and the length-prefixed NAL units of its samples (located through `stsz`, `stsc`
//! and `stco` / `co64`) become byte ranges of the stream, in decoding order, which the video
//! index then treats like the units of an Annex B stream.

use std::ops::Range;

use crate::Error;
use crate::video::Codec;

/// Samples a track may have (bounds the index of a hostile file).
const MAX_SAMPLES: usize = 1 << 22;

/// Whether `data` starts like an ISO-BMFF file (a `ftyp` box).
pub(crate) fn is_mp4(data: &[u8]) -> bool {
    data.len() >= 16
        && &data[4..8] == b"ftyp"
        && u32::from_be_bytes([data[0], data[1], data[2], data[3]]) >= 8
}

fn be(d: &[u8], at: usize, n: usize) -> Option<u64> {
    let b = d.get(at..at.checked_add(n)?)?;
    Some(b.iter().fold(0u64, |a, &x| (a << 8) | u64::from(x)))
}

/// The boxes in `range` of `d`: (type, body range).
fn boxes(d: &[u8], range: Range<usize>) -> impl Iterator<Item = ([u8; 4], Range<usize>)> + '_ {
    let mut at = range.start;
    let end = range.end.min(d.len());
    std::iter::from_fn(move || {
        if at + 8 > end {
            return None;
        }
        let size = be(d, at, 4)? as usize;
        let kind: [u8; 4] = d[at + 4..at + 8].try_into().ok()?;
        let (head, size) = match size {
            0 => (8, end - at),
            1 => (16, usize::try_from(be(d, at + 8, 8)?).ok()?),
            s => (8, s),
        };
        if size < head || size > end - at {
            return None;
        }
        let body = at + head..at + size;
        at += size;
        Some((kind, body))
    })
}

fn child(d: &[u8], range: Range<usize>, kind: &[u8; 4]) -> Option<Range<usize>> {
    boxes(d, range).find(|(k, _)| k == kind).map(|(_, r)| r)
}

/// A video track's sample entry: its codec, the NAL length size and the parameter sets.
struct Entry {
    codec: Codec,
    length_size: usize,
    params: Vec<Range<usize>>,
}

/// Reads `n` parameter sets (16-bit length, bytes) from `at`; returns the position after.
fn sets(d: &[u8], mut at: usize, n: usize, out: &mut Vec<Range<usize>>) -> Option<usize> {
    for _ in 0..n {
        let len = be(d, at, 2)? as usize;
        let r = at + 2..at + 2 + len;
        d.get(r.clone())?;
        if len > 0 {
            out.push(r);
        }
        at += 2 + len;
    }
    Some(at)
}

fn entry(d: &[u8], stsd: Range<usize>) -> Option<Entry> {
    // Version and flags, entry count, then the entries (sample entry header 8 bytes and the
    // visual sample entry's 70 before its child boxes).
    let (kind, body) = boxes(d, stsd.start + 8..stsd.end).next()?;
    let children = body.start + 78..body.end;
    match &kind {
        b"avc1" | b"avc3" => {
            let c = child(d, children, b"avcC")?;
            let length_size = (be(d, c.start + 4, 1)? as usize & 3) + 1;
            let mut params = Vec::new();
            let n = be(d, c.start + 5, 1)? as usize & 31;
            let at = sets(d, c.start + 6, n, &mut params)?;
            let n = be(d, at, 1)? as usize;
            sets(d, at + 1, n, &mut params)?;
            Some(Entry {
                codec: Codec::H264,
                length_size,
                params,
            })
        }
        b"hvc1" | b"hev1" => {
            let c = child(d, children, b"hvcC")?;
            let length_size = (be(d, c.start + 21, 1)? as usize & 3) + 1;
            let mut params = Vec::new();
            let arrays = be(d, c.start + 22, 1)? as usize;
            let mut at = c.start + 23;
            for _ in 0..arrays {
                let n = be(d, at + 1, 2)? as usize;
                at = sets(d, at + 3, n, &mut params)?;
            }
            Some(Entry {
                codec: Codec::Hevc,
                length_size,
                params,
            })
        }
        _ => None,
    }
}

/// What the first sample entry of a track that is not H.264 / HEVC holds, for the refusal.
fn other_codec(d: &[u8], stbl: Range<usize>) -> Option<&'static str> {
    let stsd = child(d, stbl, b"stsd")?;
    let (kind, _) = boxes(d, stsd.start + 8..stsd.end).next()?;
    Some(match &kind {
        b"mp4v" => "MPEG-4 Part 2 video",
        b"av01" => "AV1 video",
        b"vp08" | b"vp09" => "VP8 / VP9 video",
        b"mjpg" | b"jpeg" => "Motion JPEG video",
        b"dvhe" | b"dvh1" | b"dva1" | b"dvav" => "Dolby Vision video",
        _ => return None,
    })
}

/// The sample ranges of a sample table, in decoding order.
fn samples(d: &[u8], stbl: Range<usize>) -> Option<Vec<Range<usize>>> {
    let stsz = child(d, stbl.clone(), b"stsz")?;
    let fixed = be(d, stsz.start + 4, 4)? as usize;
    let count = be(d, stsz.start + 8, 4)? as usize;
    if count > MAX_SAMPLES {
        return None;
    }
    let size = |i: usize| {
        if fixed != 0 {
            Some(fixed)
        } else {
            be(d, stsz.start + 12 + 4 * i, 4).map(|v| v as usize)
        }
    };
    let chunks: Vec<u64> = if let Some(c) = child(d, stbl.clone(), b"stco") {
        let n = (be(d, c.start + 4, 4)? as usize).min((c.end - c.start) / 4);
        (0..n)
            .map(|i| be(d, c.start + 8 + 4 * i, 4))
            .collect::<Option<_>>()?
    } else {
        let c = child(d, stbl.clone(), b"co64")?;
        let n = (be(d, c.start + 4, 4)? as usize).min((c.end - c.start) / 8);
        (0..n)
            .map(|i| be(d, c.start + 8 + 8 * i, 8))
            .collect::<Option<_>>()?
    };
    let stsc = child(d, stbl, b"stsc")?;
    let runs = (be(d, stsc.start + 4, 4)? as usize).min((stsc.end - stsc.start) / 12);
    let run = |i: usize| -> Option<(usize, usize)> {
        let at = stsc.start + 8 + 12 * i;
        Some((be(d, at, 4)? as usize, be(d, at + 4, 4)? as usize))
    };
    let mut out = Vec::with_capacity(count);
    // Chunks are visited once, in order (runs that go backwards are ignored).
    let mut next = 1;
    for r in 0..runs {
        let (first, per) = run(r)?;
        let last = if r + 1 < runs {
            run(r + 1)?.0
        } else {
            chunks.len() + 1
        };
        let from = first.max(next);
        let to = last.min(chunks.len() + 1);
        next = next.max(to);
        for chunk in from..to {
            let mut at = usize::try_from(chunks[chunk - 1]).ok()?;
            for _ in 0..per {
                if out.len() == count {
                    return Some(out);
                }
                let len = size(out.len())?;
                let end = at.checked_add(len)?;
                out.push(at..end);
                at = end;
            }
        }
    }
    Some(out)
}

/// The NAL units of the first H.264 / HEVC track of the MP4 file `d`: its parameter sets,
/// then its samples' units, as ranges of `d`. `codec` is the transfer syntax's.
pub(crate) fn nals(d: &[u8], codec: Codec) -> Result<Vec<Range<usize>>, Error> {
    let bad = |m: &str| Error::Codec(format!("MP4 in the video pixel data: {m}"));
    let moov = boxes(d, 0..d.len())
        .find(|(k, _)| k == b"moov")
        .map(|(_, r)| r)
        .ok_or_else(|| bad("no movie box"))?;
    let track = boxes(d, moov.clone())
        .filter(|(k, _)| k == b"trak")
        .find_map(|(_, trak)| {
            let stbl = child(d, trak, b"mdia")
                .and_then(|m| child(d, m, b"minf"))
                .and_then(|m| child(d, m, b"stbl"))?;
            let e = entry(d, child(d, stbl.clone(), b"stsd")?)?;
            Some((e, stbl))
        });
    let Some((e, stbl)) = track else {
        let other = boxes(d, moov)
            .filter(|(k, _)| k == b"trak")
            .find_map(|(_, trak)| {
                let stbl = child(d, trak, b"mdia")
                    .and_then(|m| child(d, m, b"minf"))
                    .and_then(|m| child(d, m, b"stbl"))?;
                other_codec(d, stbl)
            });
        return Err(match other {
            Some(what) => Error::Unsupported(format!(
                "{what} in an MP4 file labelled as {} (not decoded)",
                codec.name()
            )),
            None => bad("no H.264 or HEVC track"),
        });
    };
    if e.codec != codec {
        return Err(Error::Unsupported(format!(
            "{} in an MP4 file labelled as {} (not decoded)",
            e.codec.name(),
            codec.name()
        )));
    }
    let samples = samples(d, stbl).ok_or_else(|| bad("damaged sample table"))?;
    let mut out = e.params;
    for s in samples {
        let mut at = s.start;
        let end = s.end.min(d.len());
        while at + e.length_size <= end {
            let Some(len) = be(d, at, e.length_size) else {
                break;
            };
            let r = at + e.length_size..(at + e.length_size).saturating_add(len as usize);
            if len == 0 || r.end > end {
                break;
            }
            at = r.end;
            out.push(r);
        }
    }
    Ok(out)
}

#[cfg(test)]
pub(crate) mod build {
    //! A minimal MP4 writer for tests: one video track, one sample per chunk.

    fn bx(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(kind);
        v.extend_from_slice(body);
        v
    }

    fn full(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut b = vec![0u8; 4];
        b.extend_from_slice(body);
        bx(kind, &b)
    }

    /// An MP4 file of `samples` (each a list of NAL units, written with 4-byte lengths) with
    /// the H.264 parameter sets `sps` and `pps`. `co64` selects 64-bit chunk offsets.
    pub fn h264(sps: &[u8], pps: &[u8], samples: &[Vec<&[u8]>], co64: bool) -> Vec<u8> {
        let ftyp = bx(b"ftyp", b"isom\0\0\x02\0isomavc1");
        let mut mdat_body = Vec::new();
        let mut sizes = Vec::new();
        let mut offs = Vec::new();
        for s in samples {
            offs.push(mdat_body.len());
            let start = mdat_body.len();
            for n in s {
                mdat_body.extend_from_slice(&(n.len() as u32).to_be_bytes());
                mdat_body.extend_from_slice(n);
            }
            sizes.push(mdat_body.len() - start);
        }
        let base = ftyp.len() + 8;
        let mdat = bx(b"mdat", &mdat_body);

        let mut avcc = vec![1, sps[1], sps[2], sps[3], 0xFF, 0xE1];
        avcc.extend_from_slice(&(sps.len() as u16).to_be_bytes());
        avcc.extend_from_slice(sps);
        avcc.push(1);
        avcc.extend_from_slice(&(pps.len() as u16).to_be_bytes());
        avcc.extend_from_slice(pps);
        let mut avc1 = vec![0u8; 78];
        avc1.extend(bx(b"avcC", &avcc));
        let mut stsd = 1u32.to_be_bytes().to_vec();
        stsd.extend(bx(b"avc1", &avc1));

        let mut stsz = 0u32.to_be_bytes().to_vec();
        stsz.extend_from_slice(&(sizes.len() as u32).to_be_bytes());
        for s in &sizes {
            stsz.extend_from_slice(&(*s as u32).to_be_bytes());
        }
        let mut stsc = 1u32.to_be_bytes().to_vec();
        for v in [1u32, 1, 1] {
            stsc.extend_from_slice(&v.to_be_bytes());
        }
        let mut stco = (offs.len() as u32).to_be_bytes().to_vec();
        for o in &offs {
            if co64 {
                stco.extend_from_slice(&((base + o) as u64).to_be_bytes());
            } else {
                stco.extend_from_slice(&((base + o) as u32).to_be_bytes());
            }
        }
        let mut stbl = full(b"stsd", &stsd);
        stbl.extend(full(b"stsz", &stsz));
        stbl.extend(full(b"stsc", &stsc));
        stbl.extend(full(if co64 { b"co64" } else { b"stco" }, &stco));
        let minf = bx(b"minf", &bx(b"stbl", &stbl));
        let mut mdia = full(b"hdlr", b"\0\0\0\0vide\0\0\0\0\0\0\0\0\0\0\0\0\0");
        mdia.extend(minf);
        let trak = bx(b"trak", &bx(b"mdia", &mdia));
        // An audio-like track first, which is skipped.
        let other = bx(
            b"trak",
            &bx(
                b"mdia",
                &bx(b"minf", &bx(b"stbl", &full(b"stsd", &[0, 0, 0, 0]))),
            ),
        );
        let mut moov = other;
        moov.extend(trak);
        let mut out = ftyp;
        out.extend(mdat);
        out.extend(bx(b"moov", &moov));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_of_an_mp4_track() {
        let sps = [0x67u8, 0x64, 0, 0x1F, 0xAC];
        let pps = [0x68u8, 0xEE, 0x3C];
        let a: &[u8] = &[0x65, 0x88, 1, 2];
        let b: &[u8] = &[0x41, 0x9A, 3];
        for co64 in [false, true] {
            let f = build::h264(&sps, &pps, &[vec![a], vec![b, b]], co64);
            assert!(is_mp4(&f));
            let n: Vec<&[u8]> = nals(&f, Codec::H264)
                .unwrap()
                .into_iter()
                .map(|r| &f[r])
                .collect();
            assert_eq!(n, [&sps[..], &pps, a, b, b]);
            assert!(matches!(nals(&f, Codec::Hevc), Err(Error::Unsupported(_))));
            for cut in 0..f.len() {
                let _ = nals(&f[..cut], Codec::H264);
            }
            for i in 0..f.len() {
                let mut g = f.clone();
                g[i] ^= 0xFF;
                let _ = nals(&g, Codec::H264);
            }
        }
    }
}
