//! IVF reading and per-frame MD5s for the VP8 tests.
#![allow(dead_code)]

use specular_vp8::{Decoder, Error};

#[path = "../md5/mod.rs"]
pub mod md5;

/// The frames of an IVF file (32-byte file header, 12-byte frame headers).
pub fn ivf_frames(data: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    if data.len() < 32 || &data[..4] != b"DKIF" {
        return out;
    }
    let hdr = u16::from_le_bytes([data[6], data[7]]) as usize;
    let mut at = hdr.max(32);
    while at + 12 <= data.len() {
        let n = u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]) as usize;
        at += 12;
        if at + n > data.len() {
            break;
        }
        out.push(&data[at..at + n]);
        at += n;
    }
    out
}

/// MD5 of the shown frames (planes concatenated), and the first error.
pub fn decode_md5s(frames: &[&[u8]], threads: usize) -> (Vec<String>, Option<(usize, Error)>) {
    let mut d = Decoder::new(threads);
    let mut out = Vec::new();
    for (i, f) in frames.iter().enumerate() {
        match d.decode(f) {
            Ok(Some(p)) => {
                let mut h = md5::Md5::new();
                for pl in &p.planes {
                    h.update(pl);
                }
                out.push(h.hex());
            }
            Ok(None) => {}
            Err(e) => return (out, Some((i, e))),
        }
    }
    (out, None)
}

/// The hashes of an `ffmpeg -f framemd5` file or a libvpx `.md5` list.
pub fn expected(text: &str) -> Vec<String> {
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| {
            l.split(|c: char| c == ',' || c.is_whitespace())
                .map(str::trim)
                .find(|t| t.len() == 32 && t.bytes().all(|b| b.is_ascii_hexdigit()))
                .map(str::to_string)
        })
        .collect()
}

/// Frames differing (counting missing ones).
pub fn mismatches(got: &[String], want: &[String]) -> usize {
    let n = got.len().max(want.len());
    (0..n).filter(|&i| got.get(i) != want.get(i)).count()
}
