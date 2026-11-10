//! GIMP gradients (`.ggr`), shown as a strip from left to right over transparency.
//!
//! Text: `GIMP Gradient`, an optional `Name:` line, the number of segments, then one line
//! per segment: its left, middle and right positions (0–1), the left and right colours as
//! RGBA (0–1), the blending function (0 linear, 1 curved, 2 sine, 3 sphere increasing, 4
//! sphere decreasing, 5 step), the colouring (0 RGB, 1 HSV counter-clockwise, 2 HSV
//! clockwise) and, in newer files, the left and right colour sources (0 the stored colour,
//! 1 foreground, 2 foreground transparent, 3 background, 4 background transparent).
//! Foreground and background are taken as GIMP's defaults, black and white.

use crate::swatch::{hsv_to_rgb, rgb_to_hsv, short, unit8};
use crate::{Error, Image, Info, Model, Pixels, Result};

const MAGIC: &[u8] = b"GIMP Gradient";
/// The strip drawn.
pub const WIDTH: u32 = 512;
pub const HEIGHT: u32 = 64;
const EPS: f64 = 1e-10;
const MAX_SEGMENTS: usize = 1 << 16;

pub fn probe(head: &[u8]) -> bool {
    let head = head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head);
    head.starts_with(MAGIC)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    pub left: f64,
    pub middle: f64,
    pub right: f64,
    pub c0: [f64; 4],
    pub c1: [f64; 4],
    pub blend: u8,
    pub colouring: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Gradient {
    pub name: String,
    pub segments: Vec<Segment>,
}

fn source(kind: u32, stored: [f64; 4]) -> [f64; 4] {
    match kind {
        1 => [0.0, 0.0, 0.0, 1.0],
        2 => [0.0, 0.0, 0.0, 0.0],
        3 => [1.0, 1.0, 1.0, 1.0],
        4 => [1.0, 1.0, 1.0, 0.0],
        _ => stored,
    }
}

pub fn parse(data: &[u8]) -> Result<Gradient> {
    if !probe(data) {
        return Err(Error::NotRecognised);
    }
    let text = String::from_utf8_lossy(data);
    let mut lines = text
        .lines()
        .skip(1)
        .map(str::trim)
        .filter(|l| !l.is_empty());
    let mut name = String::new();
    let mut count_line = lines
        .next()
        .ok_or_else(|| Error::Truncated("no segment count".into()))?;
    if let Some(n) = count_line.strip_prefix("Name:") {
        name = short(n);
        count_line = lines
            .next()
            .ok_or_else(|| Error::Truncated("no segment count".into()))?;
    }
    let count: usize = count_line
        .parse()
        .map_err(|_| Error::Malformed(format!("segment count {count_line:?}")))?;
    if count == 0 || count > MAX_SEGMENTS {
        return Err(Error::Malformed(format!("{count} segments")));
    }
    let mut segments = Vec::with_capacity(count);
    for i in 0..count {
        let line = lines
            .next()
            .ok_or_else(|| Error::Truncated(format!("segment {} of {count}", i + 1)))?;
        let v: Vec<f64> = line
            .split_whitespace()
            .map_while(|t| t.parse::<f64>().ok())
            .collect();
        if v.len() < 13 || v.iter().any(|x| !x.is_finite()) {
            return Err(Error::Malformed(format!("segment {}", i + 1)));
        }
        let c = |k: usize| [v[k], v[k + 1], v[k + 2], v[k + 3]];
        let (k0, k1) = if v.len() >= 15 {
            (v[13] as u32, v[14] as u32)
        } else {
            (0, 0)
        };
        segments.push(Segment {
            left: v[0],
            middle: v[1],
            right: v[2],
            c0: source(k0, c(3)),
            c1: source(k1, c(7)),
            blend: v[11] as u8,
            colouring: v[12] as u8,
        });
    }
    Ok(Gradient { name, segments })
}

fn linear(middle: f64, pos: f64) -> f64 {
    if pos <= middle {
        if middle < EPS {
            0.0
        } else {
            0.5 * pos / middle
        }
    } else {
        let (pos, middle) = (pos - middle, 1.0 - middle);
        if middle < EPS {
            1.0
        } else {
            0.5 + 0.5 * pos / middle
        }
    }
}

fn factor(blend: u8, middle: f64, pos: f64) -> f64 {
    match blend {
        1 => pos.powf(0.5f64.ln() / middle.max(EPS).ln()),
        2 => {
            ((-std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * linear(middle, pos)).sin()
                + 1.0)
                / 2.0
        }
        3 => {
            let p = linear(middle, pos) - 1.0;
            (1.0 - p * p).max(0.0).sqrt()
        }
        4 => {
            let p = linear(middle, pos);
            1.0 - (1.0 - p * p).max(0.0).sqrt()
        }
        5 => {
            if pos >= middle {
                1.0
            } else {
                0.0
            }
        }
        _ => linear(middle, pos),
    }
}

impl Gradient {
    /// The colour (straight RGBA, 0–1) at `t` (0–1).
    pub fn sample(&self, t: f64) -> [f64; 4] {
        let t = t.clamp(0.0, 1.0);
        let s = self
            .segments
            .iter()
            .find(|s| t <= s.right)
            .or(self.segments.last())
            .copied()
            .unwrap_or(Segment {
                left: 0.0,
                middle: 0.5,
                right: 1.0,
                c0: [0.0; 4],
                c1: [0.0; 4],
                blend: 0,
                colouring: 0,
            });
        let len = s.right - s.left;
        let (middle, pos) = if len < EPS {
            (0.5, 0.5)
        } else {
            ((s.middle - s.left) / len, (t - s.left) / len)
        };
        let f = factor(s.blend, middle, pos.clamp(0.0, 1.0)).clamp(0.0, 1.0);
        let mix = |a: f64, b: f64| a + (b - a) * f;
        let alpha = mix(s.c0[3], s.c1[3]);
        let rgb = match s.colouring {
            1 | 2 => {
                let h0 = rgb_to_hsv([s.c0[0], s.c0[1], s.c0[2]]);
                let h1 = rgb_to_hsv([s.c1[0], s.c1[1], s.c1[2]]);
                let (a, b) = (h0[0] / 360.0, h1[0] / 360.0);
                let h = if s.colouring == 1 {
                    if a < b {
                        a + (b - a) * f
                    } else {
                        (a + (1.0 - (a - b)) * f).rem_euclid(1.0)
                    }
                } else if b < a {
                    a - (a - b) * f
                } else {
                    (a - (1.0 - (b - a)) * f).rem_euclid(1.0)
                };
                hsv_to_rgb(h * 360.0, mix(h0[1], h1[1]), mix(h0[2], h1[2]))
            }
            _ => [
                mix(s.c0[0], s.c1[0]),
                mix(s.c0[1], s.c1[1]),
                mix(s.c0[2], s.c1[2]),
            ],
        };
        [rgb[0], rgb[1], rgb[2], alpha]
    }
}

pub fn info(data: &[u8], _page: u32) -> Result<Info> {
    let g = parse(data)?;
    let mut fields = Vec::new();
    let mut summary = Vec::new();
    if !g.name.is_empty() {
        fields.push(("Name".into(), g.name.clone()));
        summary.push(("title", g.name.clone()));
    }
    fields.push(("Segments".into(), g.segments.len().to_string()));
    let has_alpha = g.segments.iter().any(|s| s.c0[3] < 1.0 || s.c1[3] < 1.0);
    let hsv = g.segments.iter().any(|s| s.colouring != 0);
    Ok(Info {
        width: WIDTH,
        height: HEIGHT,
        pages: 1,
        bit_depth: 8,
        model: Model::Rgb,
        has_alpha,
        orientation: 1,
        coding: if hsv {
            "GIMP gradient, HSV blending".into()
        } else {
            "GIMP gradient".into()
        },
        summary,
        fields,
    })
}

pub fn decode(data: &[u8], _page: u32) -> Result<Image> {
    let g = parse(data)?;
    let row: Vec<u8> = (0..WIDTH)
        .flat_map(|x| g.sample((f64::from(x) + 0.5) / f64::from(WIDTH)).map(unit8))
        .collect();
    let mut px = Vec::with_capacity(row.len() * HEIGHT as usize);
    for _ in 0..HEIGHT {
        px.extend_from_slice(&row);
    }
    Ok(Image {
        width: WIDTH,
        height: HEIGHT,
        pixels: Pixels::Rgba8(px),
        truncated: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_blends_and_sources() {
        let f = b"GIMP Gradient\nName: Two\n2\n0 0.25 0.5 0 0 0 1 1 1 1 1 0 0\n0.5 0.75 1 1 0 0 1 0 0 1 0 2 1 0 4\n";
        let g = parse(f).unwrap();
        assert_eq!(g.name, "Two");
        assert_eq!(g.segments.len(), 2);
        // The middle of the first segment is half way.
        let m = g.sample(0.25);
        assert!((m[0] - 0.5).abs() < 1e-9);
        // The second ends on the background, transparent.
        assert_eq!(g.segments[1].c1, [1.0, 1.0, 1.0, 0.0]);
        let e = g.sample(1.0);
        assert!(e[3].abs() < 1e-9);
        let img = decode(f, 0).unwrap();
        assert_eq!((img.width, img.height), (WIDTH, HEIGHT));
        assert!(parse(b"GIMP Gradient\n3\n0 0.5 1 0 0 0 1 1 1 1 1 0 0\n").is_err());
        assert!(linear(0.0, 0.0) == 0.0 && linear(1.0, 1.0) == 0.5);
        assert_eq!(factor(5, 0.5, 0.6), 1.0);
    }
}
