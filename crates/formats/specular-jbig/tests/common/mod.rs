#![allow(dead_code)]

/// A binary PBM (P4) or PGM (P5) image: samples top-down, 1 per pixel; PBM as 0 (white) / 1
/// (black) with `maxval` 1.
pub struct Pnm {
    pub width: u32,
    pub height: u32,
    pub maxval: u32,
    pub samples: Vec<u32>,
    pub bitmap: bool,
}

impl Pnm {
    /// As grey levels 0..=`max` with 0 black (PBM black is 1).
    pub fn scaled(&self, max: u32) -> Vec<u16> {
        self.samples
            .iter()
            .map(|&s| {
                if self.bitmap {
                    if s == 1 { 0 } else { max as u16 }
                } else {
                    ((s * max + self.maxval / 2) / self.maxval) as u16
                }
            })
            .collect()
    }
}

pub fn read_pnm(d: &[u8]) -> Pnm {
    let mut pos = 2;
    let mut tok = || {
        loop {
            while d[pos].is_ascii_whitespace() {
                pos += 1;
            }
            if d[pos] == b'#' {
                while d[pos] != b'\n' {
                    pos += 1;
                }
            } else {
                break;
            }
        }
        let s = pos;
        while !d[pos].is_ascii_whitespace() {
            pos += 1;
        }
        std::str::from_utf8(&d[s..pos])
            .unwrap()
            .parse::<u32>()
            .unwrap()
    };
    let bitmap = &d[..2] == b"P4";
    assert!(bitmap || &d[..2] == b"P5", "not a binary PBM/PGM");
    let (w, h) = (tok(), tok());
    let maxval = if bitmap { 1 } else { tok() };
    pos += 1;
    let body = &d[pos..];
    let mut samples = Vec::with_capacity((w * h) as usize);
    if bitmap {
        let bpr = w.div_ceil(8) as usize;
        for y in 0..h as usize {
            for x in 0..w as usize {
                samples.push(u32::from(body[y * bpr + x / 8] >> (7 - x % 8) & 1));
            }
        }
    } else if maxval < 256 {
        samples.extend(body[..(w * h) as usize].iter().map(|&b| u32::from(b)));
    } else {
        samples.extend(
            body.chunks_exact(2)
                .take((w * h) as usize)
                .map(|c| u32::from(u16::from_be_bytes([c[0], c[1]]))),
        );
    }
    Pnm {
        width: w,
        height: h,
        maxval,
        samples,
        bitmap,
    }
}
