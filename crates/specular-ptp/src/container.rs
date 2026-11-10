//! USB containers (PIMA 15740 USB Still Image Capture Device Definition, §7): a 12-byte
//! header (length, type, code, transaction id) and a payload of parameters or data.

use crate::codes::container;

/// The header's size.
pub const HEADER: usize = 12;

/// A decoded container header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// The whole container's length, header included.
    pub length: u32,
    pub kind: u16,
    pub code: u16,
    pub tid: u32,
}

impl Header {
    pub fn parse(b: &[u8]) -> Option<Header> {
        if b.len() < HEADER {
            return None;
        }
        let u32_at = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
        Some(Header {
            length: u32_at(0),
            kind: u16::from_le_bytes([b[4], b[5]]),
            code: u16::from_le_bytes([b[6], b[7]]),
            tid: u32_at(8),
        })
    }

    pub fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.length.to_le_bytes());
        out.extend_from_slice(&self.kind.to_le_bytes());
        out.extend_from_slice(&self.code.to_le_bytes());
        out.extend_from_slice(&self.tid.to_le_bytes());
    }
}

/// A command, response or event container: up to five 32-bit parameters.
pub fn params(kind: u16, code: u16, tid: u32, params: &[u32]) -> Vec<u8> {
    let n = params.len().min(5);
    let mut out = Vec::with_capacity(HEADER + 4 * n);
    Header {
        length: (HEADER + 4 * n) as u32,
        kind,
        code,
        tid,
    }
    .write(&mut out);
    for p in &params[..n] {
        out.extend_from_slice(&p.to_le_bytes());
    }
    out
}

/// A data container holding `payload`.
pub fn data(code: u16, tid: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER + payload.len());
    Header {
        length: (HEADER + payload.len()) as u32,
        kind: container::DATA,
        code,
        tid,
    }
    .write(&mut out);
    out.extend_from_slice(payload);
    out
}

/// The parameters after a header (as many whole ones as `b` holds, at most five).
pub fn read_params(b: &[u8], length: u32) -> Vec<u32> {
    let end = (length as usize).min(b.len());
    b.get(HEADER..end)
        .unwrap_or(&[])
        .chunks_exact(4)
        .take(5)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_layout() {
        let c = params(container::COMMAND, 0x1002, 0, &[1]);
        assert_eq!(c, [16, 0, 0, 0, 1, 0, 2, 0x10, 0, 0, 0, 0, 1, 0, 0, 0]);
        let h = Header::parse(&c).unwrap();
        assert_eq!((h.length, h.kind, h.code, h.tid), (16, 1, 0x1002, 0));
        assert_eq!(read_params(&c, h.length), [1]);
        assert_eq!(Header::parse(&c[..11]), None);
    }

    #[test]
    fn data_layout() {
        let d = data(0x1016, 7, &[0xAA, 0xBB]);
        assert_eq!(d.len(), 14);
        let h = Header::parse(&d).unwrap();
        assert_eq!((h.length, h.kind, h.tid), (14, container::DATA, 7));
    }
}
