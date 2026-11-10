//! DEC Sixel graphics (`.six`, `.sixel`): the escape sequence a terminal draws.
//!
//! A device control string (ESC `P`, or the 8-bit 0x90) with up to three numeric parameters
//! and `q`, then data until the string terminator (ESC `\` or 0x9C). Characters `?`‥`~`
//! each paint a column of six pixels (bit 0 the top) in the current colour; `!n` repeats the
//! next one n times, `$` returns to the left edge, `-` moves down six rows, `#c` selects
//! colour register c and `#c;u;x;y;z` defines it (u = 1: hue, lightness and saturation,
//! with DEC's hue circle where blue is 0°; u = 2: red, green and blue in percent), and
//! `"a;b;w;h` sets the raster size. Registers not defined take the VT340 defaults.
//!
//! The size is the raster attributes' when they give one, else the extent drawn. Pixels
//! never painted are transparent when the second parameter is 1, else colour 0.

use crate::*;

/// The VT340's default colour registers, in percent.
const VT340: [[u8; 3]; 16] = [
    [0, 0, 0],
    [20, 20, 80],
    [80, 13, 13],
    [20, 80, 20],
    [80, 20, 80],
    [20, 80, 80],
    [80, 80, 20],
    [53, 53, 53],
    [26, 26, 26],
    [33, 33, 60],
    [60, 26, 26],
    [33, 60, 33],
    [60, 33, 60],
    [33, 60, 60],
    [60, 60, 33],
    [80, 80, 80],
];

/// Colour registers kept (terminals offer 256 to 1024).
const REGISTERS: usize = 1024;

/// Where the sixel data starts (after `q`) and the introducer's parameters.
fn introducer(data: &[u8]) -> Option<(usize, [u32; 3])> {
    // Leading white space or a terminal reset some writers put first.
    let mut i = 0;
    while i < data.len().min(64) && data[i].is_ascii_whitespace() {
        i += 1;
    }
    let rest = data.get(i..)?;
    let start = if rest.starts_with(&[0x1b, b'P']) {
        i + 2
    } else if rest.first() == Some(&0x90) {
        i + 1
    } else {
        return None;
    };
    let mut params = [0u32; 3];
    let mut k = 0;
    let mut j = start;
    while j < data.len() && j < start + 32 {
        match data[j] {
            b'0'..=b'9' => {
                if k < 3 {
                    params[k] = params[k]
                        .saturating_mul(10)
                        .saturating_add(u32::from(data[j] - b'0'));
                }
            }
            b';' => k += 1,
            b'q' => return Some((j + 1, params)),
            _ => return None,
        }
        j += 1;
    }
    None
}

/// Whether `head` starts a sixel image.
pub fn probe(head: &[u8]) -> bool {
    introducer(head).is_some()
}

/// The raster attributes (`"a;b;w;h`) right after the introducer, if any.
fn raster_size(data: &[u8], at: usize) -> Option<(u32, u32)> {
    let mut i = at;
    while data.get(i).is_some_and(|b| b.is_ascii_whitespace()) {
        i += 1;
    }
    if data.get(i) != Some(&b'"') {
        return None;
    }
    let (vals, _) = numbers(data, i + 1);
    match vals[..] {
        [_, _, w, h, ..] if w > 0 && h > 0 => Some((w, h)),
        _ => None,
    }
}

/// Up to five `;`-separated numbers from `at`; the values and where they end.
fn numbers(data: &[u8], mut at: usize) -> (Vec<u32>, usize) {
    let mut v = Vec::new();
    let mut cur: Option<u32> = None;
    while let Some(&b) = data.get(at) {
        match b {
            b'0'..=b'9' => {
                cur = Some(
                    cur.unwrap_or(0)
                        .saturating_mul(10)
                        .saturating_add(u32::from(b - b'0')),
                )
            }
            b';' => {
                v.push(cur.take().unwrap_or(0));
            }
            _ => break,
        }
        at += 1;
    }
    if cur.is_some() || data.get(at.wrapping_sub(1)) == Some(&b';') {
        v.push(cur.unwrap_or(0));
    }
    (v, at)
}

fn pct(v: u32) -> u8 {
    scale_to_8(v.min(100), 100)
}

/// DEC HLS (hue 0° blue, 120° red, 240° green; lightness and saturation in percent) to RGB.
fn hls(h: u32, l: u32, s: u32) -> [u8; 3] {
    let h = ((h + 240) % 360) as f64;
    let l = f64::from(l.min(100)) / 100.0;
    let s = f64::from(s.min(100)) / 100.0;
    if s == 0.0 {
        let v = (l * 255.0).round() as u8;
        return [v; 3];
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let f = |t: f64| {
        let t = t.rem_euclid(1.0);
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (v * 255.0).round().clamp(0.0, 255.0) as u8
    };
    let h = h / 360.0;
    [f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0)]
}

/// One pass over the data: calls `paint(x, y, colour)` for every set pixel bit and returns
/// the extent drawn and whether the terminator was found.
fn walk(
    data: &[u8],
    start: usize,
    pal: &mut [[u8; 3]],
    mut paint: impl FnMut(u32, u32, u32, u8, [u8; 3]),
) -> (u32, u32, bool) {
    let (mut x, mut y) = (0u32, 0u32);
    let (mut max_x, mut max_y) = (0u32, 0u32);
    let mut colour: usize = 0;
    let mut i = start;
    while i < data.len() {
        let b = data[i];
        match b {
            0x1b | 0x9c => return (max_x, max_y, true),
            b'?'..=b'~' => {
                sixel(
                    b,
                    1,
                    &mut x,
                    y,
                    pal[colour],
                    &mut max_x,
                    &mut max_y,
                    &mut paint,
                );
                i += 1;
            }
            b'!' => {
                let (v, end) = numbers(data, i + 1);
                let n = v.first().copied().unwrap_or(1).max(1);
                i = end;
                if let Some(&c @ b'?'..=b'~') = data.get(i) {
                    sixel(
                        c,
                        n,
                        &mut x,
                        y,
                        pal[colour],
                        &mut max_x,
                        &mut max_y,
                        &mut paint,
                    );
                    i += 1;
                }
            }
            b'$' => {
                x = 0;
                i += 1;
            }
            b'-' => {
                x = 0;
                y = y.saturating_add(6);
                i += 1;
            }
            b'#' => {
                let (v, end) = numbers(data, i + 1);
                i = end;
                if let Some(&c) = v.first() {
                    let c = (c as usize).min(REGISTERS - 1);
                    colour = c;
                    if let [_, u, a, b2, c2, ..] = v[..] {
                        pal[c] = match u {
                            1 => hls(a, b2, c2),
                            _ => [pct(a), pct(b2), pct(c2)],
                        };
                    }
                }
            }
            b'"' => {
                i = numbers(data, i + 1).1;
            }
            _ => i += 1,
        }
    }
    (max_x, max_y, false)
}

/// Paints one sixel character `n` times: `paint(x, n, y, bits, colour)`.
#[allow(clippy::too_many_arguments)]
#[inline]
fn sixel(
    c: u8,
    n: u32,
    x: &mut u32,
    y: u32,
    colour: [u8; 3],
    max_x: &mut u32,
    max_y: &mut u32,
    paint: &mut impl FnMut(u32, u32, u32, u8, [u8; 3]),
) {
    let bits = c - b'?';
    let n = n.min(MAX_DIMENSION);
    if bits != 0 {
        paint(*x, n, y, bits, colour);
        *max_y = (*max_y).max(y.saturating_add(8 - bits.leading_zeros()));
    }
    *x = x.saturating_add(n);
    *max_x = (*max_x).max(*x);
}

fn default_palette() -> Vec<[u8; 3]> {
    let mut pal = vec![[0u8; 3]; REGISTERS];
    for (i, p) in pal.iter_mut().enumerate() {
        let [r, g, b] = VT340[i % 16];
        *p = [pct(u32::from(r)), pct(u32::from(g)), pct(u32::from(b))];
    }
    pal
}

struct Layout {
    start: usize,
    width: u32,
    height: u32,
    transparent: bool,
    terminated: bool,
}

fn layout(data: &[u8]) -> Result<Layout> {
    let (start, params) = introducer(data).ok_or(Error::NotRecognised)?;
    let mut pal = default_palette();
    let (ew, eh, terminated) = walk(data, start, &mut pal, |_, _, _, _, _| {});
    let (width, height) = raster_size(data, start).unwrap_or((ew, eh));
    check_dims(width, height)?;
    Ok(Layout {
        start,
        width,
        height,
        transparent: params[1] == 1,
        terminated,
    })
}

pub fn info(data: &[u8]) -> Result<Info> {
    let l = layout(data)?;
    Ok(Info::new(
        l.width,
        l.height,
        PixelFormat::Rgba8,
        Model::Palette,
        8,
        if l.transparent {
            "Sixel, transparent background"
        } else {
            "Sixel"
        },
    )
    .alpha(l.transparent))
}

pub fn decode(data: &[u8]) -> Result<Image> {
    let l = layout(data)?;
    let (w, h) = (l.width, l.height);
    // Unpainted pixels: transparent, or colour 0 as it stands at the end.
    let mut img = Image::filled(w, h, PixelFormat::Rgba8, 0);
    let mut painted = vec![false; w as usize * h as usize];
    let mut pal = default_palette();
    // A pixel keeps the colour its register had when it was painted.
    walk(data, l.start, &mut pal, |x0, n, y, bits, [r, g, b]| {
        let x1 = x0.saturating_add(n).min(w);
        for k in 0..6 {
            let yy = y.saturating_add(k);
            if bits >> k & 1 == 0 || yy >= h {
                continue;
            }
            for x in x0..x1 {
                let i = yy as usize * w as usize + x as usize;
                img.data[i * 4..i * 4 + 4].copy_from_slice(&[r, g, b, 255]);
                painted[i] = true;
            }
        }
    });
    if !l.transparent {
        let [r, g, b] = pal[0];
        for (o, _) in img
            .data
            .chunks_exact_mut(4)
            .zip(&painted)
            .filter(|(_, p)| !**p)
        {
            o.copy_from_slice(&[r, g, b, 255]);
        }
    }
    img.truncated = !l.terminated;
    Ok(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hls_hues() {
        assert_eq!(hls(0, 50, 100), [0, 0, 255]);
        assert_eq!(hls(120, 50, 100), [255, 0, 0]);
        assert_eq!(hls(240, 50, 100), [0, 255, 0]);
        assert_eq!(hls(0, 100, 0), [255, 255, 255]);
    }

    #[test]
    fn small() {
        // 3×6 red bar then a green pixel on the next band, transparent background.
        let s = b"\x1bP0;1;0q\"1;1;4;12#1;2;100;0;0#2;2;0;100;0#1!3~-#2@\x1b\\";
        let i = info(s).unwrap();
        assert_eq!((i.width, i.height), (4, 12));
        let img = decode(s).unwrap();
        assert!(!img.truncated);
        let px = |x: usize, y: usize| &img.data[(y * 4 + x) * 4..][..4];
        assert_eq!(px(0, 0), [255, 0, 0, 255]);
        assert_eq!(px(2, 5), [255, 0, 0, 255]);
        assert_eq!(px(3, 0), [0, 0, 0, 0]);
        assert_eq!(px(0, 6), [0, 255, 0, 255]);
        assert_eq!(px(1, 6), [0, 0, 0, 0]);
    }

    #[test]
    fn extent_without_raster_attributes() {
        let s = b"\x90q#0;2;0;0;100!5N\x9c";
        let i = info(s).unwrap();
        assert_eq!((i.width, i.height), (5, 4));
        let img = decode(s).unwrap();
        // 'N' = 0b001111: rows 0-3 painted blue.
        assert_eq!(&img.data[..4], &[0, 0, 255, 255]);
    }

    #[test]
    fn hostile() {
        // A huge repeat is capped at the largest width.
        assert_eq!(info(b"\x1bPq!99999999~").unwrap().width, MAX_DIMENSION);
        assert!(info(b"\x1bPq").is_err());
        assert!(!probe(b"hello"));
        let _ = decode(b"\x1bPq\"1;1;65536;65536~");
    }
}
