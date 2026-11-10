//! PNG: Orientation in the `eXIf` chunk (patched, or a new chunk before the first `IDAT`).

use super::tiff;

const SIG: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Offsets of each chunk: (start of the length field, type, data range).
type ChunkAt = (usize, [u8; 4], std::ops::Range<usize>);

fn chunks(d: &[u8]) -> Option<Vec<ChunkAt>> {
    if !d.starts_with(SIG) {
        return None;
    }
    let mut out = Vec::new();
    let mut i = SIG.len();
    while i + 12 <= d.len() {
        let len = u32::from_be_bytes(d[i..i + 4].try_into().ok()?) as usize;
        let ty: [u8; 4] = d[i + 4..i + 8].try_into().ok()?;
        let data = i + 8..(i + 8).checked_add(len)?;
        if data.end + 4 > d.len() {
            return None;
        }
        out.push((i, ty, data.clone()));
        i = data.end + 4;
        if &ty == b"IEND" {
            break;
        }
    }
    Some(out)
}

pub fn orientation(d: &[u8]) -> Option<u8> {
    let c = chunks(d)?;
    let (_, _, r) = c.iter().find(|(_, t, _)| t == b"eXIf")?;
    tiff::orientation(&d[r.clone()])
}

fn chunk(ty: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(data.len() + 12);
    v.extend_from_slice(&(data.len() as u32).to_be_bytes());
    v.extend_from_slice(ty);
    v.extend_from_slice(data);
    let crc = crc32(&v[4..]);
    v.extend_from_slice(&crc.to_be_bytes());
    v
}

/// The file with Orientation `o`.
pub fn set_orientation(d: &[u8], o: u8) -> Option<Vec<u8>> {
    let c = chunks(d)?;
    if let Some((at, _, r)) = c.iter().find(|(_, t, _)| t == b"eXIf") {
        let mut t = d[r.clone()].to_vec();
        tiff::set_orientation(&mut t, o)?;
        let mut out = d[..*at].to_vec();
        out.extend_from_slice(&chunk(b"eXIf", &t));
        out.extend_from_slice(&d[r.end + 4..]);
        return Some(out);
    }
    let (at, _, _) = c.iter().find(|(_, t, _)| t == b"IDAT")?;
    let mut out = d[..*at].to_vec();
    out.extend_from_slice(&chunk(b"eXIf", &tiff::minimal(o)));
    out.extend_from_slice(&d[*at..]);
    Some(out)
}

/// CRC-32 (ISO 3309, as PNG uses), table driven.
pub(crate) fn crc32(b: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let t = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (n, e) in t.iter_mut().enumerate() {
            let mut c = n as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
            *e = c;
        }
        t
    });
    let mut c = 0xFFFF_FFFFu32;
    for &x in b {
        c = t[((c ^ u32::from(x)) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

#[cfg(test)]
mod tests {
    #[test]
    fn crc_check_value() {
        assert_eq!(super::crc32(b"123456789"), 0xCBF4_3926);
    }
}
