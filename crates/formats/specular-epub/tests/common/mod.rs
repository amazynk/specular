//! A stand-in picture decoder for the tests: the size from a PNG's IHDR, a JPEG's SOF or an
//! SVG's width/height, drawn as a flat colour taken from the bytes.

#![allow(dead_code)]

mod build;
pub use build::book;

use std::path::PathBuf;

use specular_epub::{Image, ImageDecoder};

pub struct Stub;

pub fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

pub fn read_fixture(name: &str) -> Vec<u8> {
    std::fs::read(fixture(name)).unwrap()
}

fn jpeg_size(b: &[u8]) -> Option<(u32, u32)> {
    let mut i = 2;
    while i + 9 < b.len() {
        if b[i] != 0xFF {
            i += 1;
            continue;
        }
        let m = b[i + 1];
        let len = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
        if (0xC0..=0xCF).contains(&m) && m != 0xC4 && m != 0xC8 && m != 0xCC {
            let h = u16::from_be_bytes([b[i + 5], b[i + 6]]) as u32;
            let w = u16::from_be_bytes([b[i + 7], b[i + 8]]) as u32;
            return Some((w, h));
        }
        i += 2 + len;
    }
    None
}

fn svg_size(b: &[u8]) -> Option<(u32, u32)> {
    let s = String::from_utf8_lossy(&b[..b.len().min(4096)]).to_string();
    let get = |k: &str| {
        let p = s.find(&format!("{k}=\""))? + k.len() + 2;
        let e = s[p..].find('"')? + p;
        s[p..e].trim_end_matches("px").parse::<f32>().ok()
    };
    match (get("width"), get("height")) {
        (Some(w), Some(h)) => Some((w as u32, h as u32)),
        _ => Some((300, 150)),
    }
}

impl ImageDecoder for Stub {
    fn decode(&self, bytes: &[u8], media_type: &str, max_w: u32, max_h: u32) -> Option<Image> {
        let (w, h) = self.size(bytes, media_type)?;
        let k = (max_w as f32 / w as f32)
            .min(max_h as f32 / h as f32)
            .min(1.0);
        let (w, h) = (
            ((w as f32 * k) as u32).max(1),
            ((h as f32 * k) as u32).max(1),
        );
        let c = bytes
            .iter()
            .fold(7u32, |a, &b| a.wrapping_mul(31).wrapping_add(u32::from(b)));
        let px = [(c >> 16) as u8, (c >> 8) as u8, c as u8, 255];
        Some(Image {
            width: w,
            height: h,
            rgba: px.repeat((w * h) as usize),
        })
    }

    fn size(&self, b: &[u8], media_type: &str) -> Option<(u32, u32)> {
        if b.starts_with(b"\x89PNG") && b.len() >= 24 {
            return Some((
                u32::from_be_bytes(b[16..20].try_into().ok()?),
                u32::from_be_bytes(b[20..24].try_into().ok()?),
            ));
        }
        if b.starts_with(&[0xFF, 0xD8]) {
            return jpeg_size(b);
        }
        if media_type == "image/svg+xml" || b.starts_with(b"<") {
            return svg_size(b);
        }
        if b.starts_with(b"GIF8") && b.len() >= 10 {
            return Some((
                u32::from(u16::from_le_bytes([b[6], b[7]])),
                u32::from(u16::from_le_bytes([b[8], b[9]])),
            ));
        }
        None
    }
}
