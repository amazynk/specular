//! An HLS sound rendition added to an MPEG-TS piece whose variant has no sound of its own.
//!
//! The rendition's segments are MPEG-TS (their sound packets are taken over under a new
//! PID) or packed sound (ADTS or MPEG audio frames with an ID3 timestamp in front, put into
//! PES packets here). The piece's PMT gets the new stream, its CRC made again; the sound's
//! packets follow the picture's. Both keep their own 90 kHz times, which HLS puts on one
//! clock.

use crate::frames;

const PACKET: usize = 188;

/// A frame reader: its length and seconds.
type FrameStep = fn(&[u8]) -> Option<(usize, f64)>;

/// The PID of a TS packet.
fn pid(p: &[u8]) -> u16 {
    (u16::from(p[1] & 0x1F) << 8) | u16::from(p[2])
}

/// The payload of a TS packet (after any adaptation field).
fn payload(p: &[u8]) -> &[u8] {
    let afc = (p[3] >> 4) & 3;
    let start = if afc & 2 != 0 {
        5 + usize::from(p[4])
    } else {
        4
    };
    if afc & 1 == 0 || start >= PACKET {
        return &[];
    }
    &p[start..]
}

fn packets(ts: &[u8]) -> impl Iterator<Item = &[u8]> {
    ts.chunks_exact(PACKET).filter(|p| p[0] == 0x47)
}

/// The first PSI section with table `table` on PID `on` (in one packet).
fn section(ts: &[u8], on: u16, table: u8) -> Option<Vec<u8>> {
    for p in packets(ts) {
        if pid(p) != on || p[1] & 0x40 == 0 {
            continue;
        }
        let pl = payload(p);
        let at = 1 + usize::from(*pl.first()?);
        let s = pl.get(at..)?;
        if s.len() < 3 || s[0] != table {
            continue;
        }
        let len = (usize::from(s[1] & 0x0F) << 8 | usize::from(s[2])) + 3;
        return s.get(..len).map(<[u8]>::to_vec);
    }
    None
}

/// The PMT PID of the first programme.
fn pmt_pid(ts: &[u8]) -> Option<u16> {
    let pat = section(ts, 0, 0)?;
    let end = pat.len().checked_sub(4)?;
    pat.get(8..end)?.chunks_exact(4).find_map(|e| {
        let number = u16::from_be_bytes([e[0], e[1]]);
        (number != 0).then(|| u16::from_be_bytes([e[2], e[3]]) & 0x1FFF)
    })
}

/// A PMT's streams: (type, PID, descriptors).
fn streams(pmt: &[u8]) -> Vec<(u8, u16, Vec<u8>)> {
    let mut out = Vec::new();
    if pmt.len() < 16 {
        return out;
    }
    let info = usize::from(pmt[10] & 0x0F) << 8 | usize::from(pmt[11]);
    let end = pmt.len() - 4;
    let mut p = 12 + info;
    while p + 5 <= end {
        let ty = pmt[p];
        let pid = u16::from_be_bytes([pmt[p + 1], pmt[p + 2]]) & 0x1FFF;
        let n = usize::from(pmt[p + 3] & 0x0F) << 8 | usize::from(pmt[p + 4]);
        let Some(d) = pmt.get(p + 5..p + 5 + n) else {
            break;
        };
        out.push((ty, pid, d.to_vec()));
        p += 5 + n;
    }
    out
}

/// Whether a PMT stream is sound (by type, or AC-3/E-AC-3/DTS/Opus descriptors under 0x06).
fn is_sound(ty: u8, desc: &[u8]) -> bool {
    match ty {
        0x03 | 0x04 | 0x0F | 0x11 | 0x81 | 0x82 | 0x83 | 0x84 | 0x87 => true,
        0x06 => {
            let mut p = 0;
            while p + 2 <= desc.len() {
                let (tag, n) = (desc[p], usize::from(desc[p + 1]));
                if matches!(tag, 0x6A | 0x7A | 0x7B)
                    || tag == 0x05
                        && desc
                            .get(p + 2..p + 6)
                            .is_some_and(|f| matches!(f, b"AC-3" | b"EAC3" | b"Opus"))
                {
                    return true;
                }
                p += 2 + n;
            }
            false
        }
        _ => false,
    }
}

/// CRC-32/MPEG-2 of a PSI section.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 {
                (crc << 1) ^ 0x04C1_1DB7
            } else {
                crc << 1
            };
        }
    }
    crc
}

/// Packets carrying `data` (a PES packet or a section with its pointer field) on `pid`,
/// stuffed to whole packets.
fn packetize(out: &mut Vec<u8>, pid: u16, cc: &mut u8, data: &[u8]) {
    let mut rest = data;
    let mut first = true;
    while !rest.is_empty() || first {
        let n = rest.len().min(184);
        let mut p = vec![
            0x47,
            (if first { 0x40 } else { 0 }) | ((pid >> 8) as u8 & 0x1F),
            pid as u8,
            0,
        ];
        if n < 184 {
            // An adaptation field fills the packet.
            p[3] = 0x30 | (*cc & 0x0F);
            let afl = 183 - n;
            p.push(afl as u8);
            if afl > 0 {
                p.push(0);
                p.resize(p.len() + afl - 1, 0xFF);
            }
        } else {
            p[3] = 0x10 | (*cc & 0x0F);
        }
        p.extend_from_slice(&rest[..n]);
        debug_assert_eq!(p.len(), PACKET);
        out.extend_from_slice(&p);
        *cc = (*cc + 1) & 0x0F;
        rest = &rest[n..];
        first = false;
    }
}

/// A PES packet of sound `es` at `pts` (90 kHz).
fn pes(es: &[u8], pts: u64) -> Vec<u8> {
    let mut v = vec![0, 0, 1, 0xC0];
    let len = 8 + es.len();
    v.extend_from_slice(&(if len <= 0xFFFF { len as u16 } else { 0 }).to_be_bytes());
    v.extend_from_slice(&[0x80, 0x80, 5]);
    let t = pts & ((1 << 33) - 1);
    v.extend_from_slice(&[
        (((t >> 29) & 0x0E) | 0x21) as u8,
        (t >> 22) as u8,
        (((t >> 14) & 0xFE) | 1) as u8,
        (t >> 7) as u8,
        (((t << 1) & 0xFE) | 1) as u8,
    ]);
    v.extend_from_slice(es);
    v
}

/// The 33-bit timestamp of HLS packed sound's ID3 tag
/// (`PRIV` "com.apple.streaming.transportStreamTimestamp"), and where the sound starts.
fn id3_timestamp(b: &[u8]) -> (Option<u64>, usize) {
    if b.len() < 10 || &b[..3] != b"ID3" {
        return (None, 0);
    }
    let size = b[6..10]
        .iter()
        .fold(0usize, |a, &x| (a << 7) | usize::from(x & 0x7F));
    let end = (10 + size).min(b.len());
    const OWNER: &[u8] = b"com.apple.streaming.transportStreamTimestamp\0";
    let tag = &b[10..end];
    let ts = tag
        .windows(OWNER.len())
        .position(|w| w == OWNER)
        .and_then(|i| tag.get(i + OWNER.len()..i + OWNER.len() + 8))
        .map(|t| u64::from_be_bytes(t.try_into().unwrap_or([0; 8])) & ((1 << 33) - 1));
    (ts, end)
}

/// `video` (a piece of MPEG-TS without sound) with the rendition's segments `sound` added.
/// `None` when the piece has sound of its own or the rendition is not read here.
pub fn add_sound(video: &[u8], sound: &[Vec<u8>]) -> Option<Vec<u8>> {
    let pmt_on = pmt_pid(video)?;
    let pmt = section(video, pmt_on, 2)?;
    let have = streams(&pmt);
    if have.iter().any(|(t, _, d)| is_sound(*t, d)) || sound.is_empty() {
        return None;
    }
    let used: Vec<u16> = packets(video).map(pid).collect();
    let new_pid = (0x100u16..0x1FFE).find(|p| !used.contains(p) && *p != pmt_on)?;
    let mut extra = Vec::new();
    let mut cc = 0u8;
    let (stream_type, desc) = if sound[0].first() == Some(&0x47) {
        // MPEG-TS segments: their sound stream, under the new PID.
        let apmt = section(&sound[0], pmt_pid(&sound[0])?, 2)?;
        let (ty, apid, d) = streams(&apmt)
            .into_iter()
            .find(|(t, _, d)| is_sound(*t, d))?;
        for seg in sound {
            for p in packets(seg).filter(|p| pid(p) == apid) {
                let mut q = p.to_vec();
                q[1] = (q[1] & 0xE0) | ((new_pid >> 8) as u8 & 0x1F);
                q[2] = new_pid as u8;
                extra.extend_from_slice(&q);
            }
        }
        (ty, d)
    } else {
        // Packed sound: frames put into PES packets at the ID3 tag's time on.
        let (_, start) = id3_timestamp(&sound[0]);
        let framing = frames::Framing::sniff(&sound[0][start..])?;
        let (ty, step): (u8, FrameStep) = match framing {
            frames::Framing::Adts => (0x0F, frames::adts),
            frames::Framing::Mp3 => (0x03, frames::mp3),
            _ => return None,
        };
        let mut next: Option<f64> = None;
        for seg in sound {
            let (ts, start) = id3_timestamp(seg);
            let mut t = ts.map(|t| t as f64).or(next).unwrap_or(0.0);
            let mut at = start;
            while at < seg.len() {
                let Some((n, secs)) = step(&seg[at..]) else {
                    break;
                };
                let Some(frame) = seg.get(at..at + n) else {
                    break;
                };
                packetize(&mut extra, new_pid, &mut cc, &pes(frame, t.round() as u64));
                t += secs * 90_000.0;
                at += n;
            }
            next = Some(t);
        }
        (ty, Vec::new())
    };
    if extra.is_empty() {
        return None;
    }
    // The PMT with the sound stream added.
    let body_end = pmt.len() - 4;
    let mut s = pmt[..body_end].to_vec();
    s.push(stream_type);
    s.extend_from_slice(&(0xE000 | new_pid).to_be_bytes());
    s.extend_from_slice(&(0xF000 | desc.len() as u16).to_be_bytes());
    s.extend_from_slice(&desc);
    let len = s.len() + 4 - 3;
    if len > 1021 || s.len() + 5 > 184 {
        return None;
    }
    s[1] = (s[1] & 0xF0) | ((len >> 8) as u8 & 0x0F);
    s[2] = len as u8;
    let crc = crc32(&s);
    s.extend_from_slice(&crc.to_be_bytes());
    let mut out = Vec::with_capacity(video.len() + extra.len());
    for p in video.chunks_exact(PACKET) {
        if p[0] == 0x47 && pid(p) == pmt_on && p[1] & 0x40 != 0 {
            let mut sec = vec![0u8];
            sec.extend_from_slice(&s);
            // A section is stuffed with 0xFF, not an adaptation field.
            let mut q = vec![0x47, 0x40 | ((pmt_on >> 8) as u8 & 0x1F), pmt_on as u8];
            q.push(0x10 | (p[3] & 0x0F));
            q.extend_from_slice(&sec);
            q.resize(PACKET, 0xFF);
            out.extend_from_slice(&q);
        } else {
            out.extend_from_slice(p);
        }
    }
    out.extend_from_slice(&extra);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_of_a_known_pat() {
        // A PAT for programme 1 at PID 0x1000 (as ffmpeg writes it).
        let pat = [
            0x00, 0xB0, 0x0D, 0x00, 0x01, 0xC1, 0x00, 0x00, 0x00, 0x01, 0xF0, 0x00,
        ];
        assert_eq!(crc32(&pat), 0x2AB1_04B2);
    }

    #[test]
    fn pes_times_and_stuffing() {
        let p = pes(&[1, 2, 3], 0x1_2345_6789);
        assert_eq!(&p[..9], &[0, 0, 1, 0xC0, 0, 11, 0x80, 0x80, 5]);
        let mut out = Vec::new();
        let mut cc = 15;
        packetize(&mut out, 0x101, &mut cc, &p);
        assert_eq!(out.len(), PACKET);
        assert_eq!(cc, 0);
        assert_eq!(payload(&out), &p[..]);
        let big = vec![7u8; 400];
        let mut out = Vec::new();
        packetize(&mut out, 0x101, &mut cc, &big);
        assert_eq!(out.len(), 3 * PACKET);
        let back: Vec<u8> = out
            .chunks(PACKET)
            .flat_map(|p| payload(p).to_vec())
            .collect();
        assert_eq!(back, big);
    }
}
