//! IVF reading and frame MD5s for the tests.

use specular_vp9::{Decoder, Error};

#[path = "../md5/mod.rs"]
mod md5;

/// The frames of an IVF file.
pub fn ivf_frames(data: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    if data.len() < 32 || &data[..4] != b"DKIF" {
        return out;
    }
    let hdr = u16::from_le_bytes([data[6], data[7]]) as usize;
    let mut p = hdr;
    while p + 12 <= data.len() {
        let n = u32::from_le_bytes(data[p..p + 4].try_into().unwrap()) as usize;
        p += 12;
        if p + n > data.len() {
            break;
        }
        out.push(&data[p..p + n]);
        p += n;
    }
    out
}

/// Decodes an IVF file: the MD5 of the frame each block shows (the last one of a superframe,
/// as libvpx and the player show), and the error that stopped it.
pub fn decode_md5s(data: &[u8], threads: usize) -> (Vec<String>, Option<Error>) {
    let mut d = Decoder::new(threads);
    let mut md5s = Vec::new();
    let mut out = Vec::new();
    for f in ivf_frames(data) {
        out.clear();
        let r = d.decode(f, &mut out);
        if let Some(p) = out.last() {
            let mut h = md5::Md5::new();
            for pl in &p.planes {
                h.update(pl);
            }
            md5s.push(h.hex());
        }
        if let Err(e) = r {
            return (md5s, Some(e));
        }
    }
    (md5s, None)
}
