//! Opus packet framing (RFC 6716 section 3 and appendix B).

use crate::Error;

/// Coding mode of a packet (from its TOC byte).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Silk,
    Hybrid,
    Celt,
}

/// Audio bandwidth of a packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Bandwidth {
    Narrow,
    Medium,
    Wide,
    SuperWide,
    Full,
}

/// What the TOC byte says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Toc {
    pub(crate) mode: Mode,
    pub(crate) bandwidth: Bandwidth,
    /// Samples per frame at 48 kHz.
    pub(crate) frame_size: usize,
    pub(crate) stereo: bool,
}

impl Toc {
    pub(crate) fn parse(b: u8) -> Toc {
        let config = b >> 3;
        let stereo = b & 4 != 0;
        let (mode, bandwidth, frame_size) = match config {
            0..=11 => {
                let bw = match config / 4 {
                    0 => Bandwidth::Narrow,
                    1 => Bandwidth::Medium,
                    _ => Bandwidth::Wide,
                };
                (
                    Mode::Silk,
                    bw,
                    [480, 960, 1920, 2880][usize::from(config % 4)],
                )
            }
            12..=15 => {
                let bw = if config < 14 {
                    Bandwidth::SuperWide
                } else {
                    Bandwidth::Full
                };
                (Mode::Hybrid, bw, [480, 960][usize::from(config % 2)])
            }
            _ => {
                let bw = match (config - 16) / 4 {
                    0 => Bandwidth::Narrow,
                    1 => Bandwidth::Wide,
                    2 => Bandwidth::SuperWide,
                    _ => Bandwidth::Full,
                };
                (
                    Mode::Celt,
                    bw,
                    [120, 240, 480, 960][usize::from(config % 4)],
                )
            }
        };
        Toc {
            mode,
            bandwidth,
            frame_size,
            stereo,
        }
    }
}

/// A parsed packet: its TOC and the byte ranges of its frames.
#[derive(Debug, Clone)]
pub(crate) struct Packet {
    pub(crate) toc: Toc,
    /// (offset, length) of each frame within the packet bytes.
    pub(crate) frames: Vec<(usize, usize)>,
    /// Bytes the packet took (differs from the input length only when self-delimited).
    pub(crate) len: usize,
}

fn malformed(what: &str) -> Error {
    Error::Malformed(format!("Opus packet {what}"))
}

/// One- or two-byte frame length (section 3.2.1); returns (length, bytes used).
fn frame_len(data: &[u8]) -> Result<(usize, usize), Error> {
    let b0 = *data.first().ok_or_else(|| malformed("is cut short"))?;
    if b0 < 252 {
        Ok((usize::from(b0), 1))
    } else {
        let b1 = *data.get(1).ok_or_else(|| malformed("is cut short"))?;
        Ok((usize::from(b1) * 4 + usize::from(b0), 2))
    }
}

/// Parses a packet; `self_delimited` for all but the last stream of a multistream packet.
pub(crate) fn parse(data: &[u8], self_delimited: bool) -> Result<Packet, Error> {
    let toc_byte = *data.first().ok_or_else(|| malformed("is empty"))?;
    let toc = Toc::parse(toc_byte);
    let mut pos = 1usize;
    // Bytes of `data` that belong to this packet (shrinks by the padding).
    let mut end = data.len();
    let code = toc_byte & 3;
    let mut sizes: Vec<usize> = Vec::new();
    let mut cbr_count = 0usize;
    let mut last_explicit = false;
    match code {
        0 => cbr_count = 1,
        1 => cbr_count = 2,
        2 => {
            let (n, used) = frame_len(&data[pos..])?;
            pos += used;
            sizes.push(n);
            last_explicit = false;
        }
        _ => {
            let ch = *data.get(pos).ok_or_else(|| malformed("is cut short"))?;
            pos += 1;
            let m = usize::from(ch & 0x3f);
            if m == 0 || m * toc.frame_size > 5760 {
                return Err(malformed("has a bad frame count"));
            }
            if ch & 0x40 != 0 {
                // Padding length: 255 means 254 more and another length byte follows.
                loop {
                    let p = *data
                        .get(pos)
                        .ok_or_else(|| malformed("padding is cut short"))?;
                    pos += 1;
                    let add = if p == 255 { 254 } else { usize::from(p) };
                    end = end
                        .checked_sub(add)
                        .ok_or_else(|| malformed("padding is too long"))?;
                    if p != 255 {
                        break;
                    }
                }
                if end < pos {
                    return Err(malformed("padding is too long"));
                }
            }
            if ch & 0x80 != 0 {
                for _ in 0..m - 1 {
                    let (n, used) = frame_len(data.get(pos..end).unwrap_or(&[]))?;
                    pos += used;
                    sizes.push(n);
                }
            } else {
                cbr_count = m;
            }
            last_explicit = ch & 0x80 != 0;
        }
    }
    if end < pos {
        return Err(malformed("is cut short"));
    }
    let mut frames = Vec::new();
    if self_delimited {
        // One more length: of every CBR frame, or of the last VBR frame.
        let (n, used) = frame_len(data.get(pos..end).unwrap_or(&[]))?;
        pos += used;
        if cbr_count > 0 {
            let total = n * cbr_count;
            if pos + total > end {
                return Err(malformed("is cut short"));
            }
            for i in 0..cbr_count {
                frames.push((pos + i * n, n));
            }
            pos += total;
        } else {
            let mut p = pos;
            for &s in &sizes {
                frames.push((p, s));
                p += s;
            }
            frames.push((p, n));
            p += n;
            if p > end {
                return Err(malformed("is cut short"));
            }
            pos = p;
        }
        // A self-delimited packet ends after its frames plus its padding.
        let padding = data.len() - end;
        let total = pos + padding;
        if total > data.len() {
            return Err(malformed("is cut short"));
        }
        check_sizes(&frames)?;
        return Ok(Packet {
            toc,
            frames,
            len: total,
        });
    }
    let avail = end - pos;
    if cbr_count > 0 {
        if !avail.is_multiple_of(cbr_count) {
            return Err(malformed("has frames of unequal size"));
        }
        let n = avail / cbr_count;
        for i in 0..cbr_count {
            frames.push((pos + i * n, n));
        }
    } else {
        let explicit: usize = sizes.iter().sum();
        if explicit > avail {
            return Err(malformed("frame lengths overrun the packet"));
        }
        let mut p = pos;
        for &s in &sizes {
            frames.push((p, s));
            p += s;
        }
        frames.push((p, end - p));
        let _ = last_explicit;
    }
    check_sizes(&frames)?;
    Ok(Packet {
        toc,
        frames,
        len: data.len(),
    })
}

fn check_sizes(frames: &[(usize, usize)]) -> Result<(), Error> {
    if frames.iter().any(|&(_, n)| n > 1275) {
        return Err(malformed("has a frame longer than 1275 bytes"));
    }
    Ok(())
}

/// Samples per channel at 48 kHz of a packet from its TOC and frame count (None if malformed).
pub fn packet_samples(packet: &[u8]) -> Option<usize> {
    let toc = Toc::parse(*packet.first()?);
    let count = match packet[0] & 3 {
        0 => 1,
        1 | 2 => 2,
        _ => usize::from(*packet.get(1)? & 0x3f),
    };
    let n = count * toc.frame_size;
    if count == 0 || n > 5760 {
        None
    } else {
        Some(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes() {
        // Code 0, CELT FB 20 ms.
        let p = parse(&[31 << 3, 1, 2, 3], false).unwrap();
        assert_eq!(p.frames, vec![(1, 3)]);
        assert_eq!(p.toc.frame_size, 960);
        // Code 1 needs an even payload.
        assert!(parse(&[(31 << 3) | 1, 1, 2, 3], false).is_err());
        let p = parse(&[(31 << 3) | 1, 1, 2, 3, 4], false).unwrap();
        assert_eq!(p.frames, vec![(1, 2), (3, 2)]);
        // Code 2.
        let p = parse(&[(31 << 3) | 2, 1, 9, 8, 7], false).unwrap();
        assert_eq!(p.frames, vec![(2, 1), (3, 2)]);
        // Code 3 CBR with padding of 2.
        let p = parse(&[(31 << 3) | 3, 0x40 | 2, 2, 1, 1, 2, 2, 0, 0], false).unwrap();
        assert_eq!(p.frames, vec![(3, 2), (5, 2)]);
        // Code 3 VBR.
        let p = parse(&[(31 << 3) | 3, 0x80 | 2, 1, 5, 6, 6], false).unwrap();
        assert_eq!(p.frames, vec![(3, 1), (4, 2)]);
        // Two-byte length.
        assert_eq!(frame_len(&[253, 1]).unwrap(), (257, 2));
        assert_eq!(packet_samples(&[(31 << 3) | 3, 3]), Some(2880));
        assert_eq!(packet_samples(&[(31 << 3) | 3, 7]), None);
    }

    #[test]
    fn self_delimited() {
        let p = parse(&[31 << 3, 2, 7, 7, 0xff, 0xff], true).unwrap();
        assert_eq!(p.frames, vec![(2, 2)]);
        assert_eq!(p.len, 4);
    }
}
