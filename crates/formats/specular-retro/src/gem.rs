//! GEM bit images (`.img`, Atari ST and PC GEM), with the XIMG palette extension.
//!
//! Big-endian words: version, header length in words, planes, pattern length, pixel width
//! and height in microns, image width and height. An XIMG header follows with "XIMG", a
//! colour model (0 = RGB) and one red, green, blue triple per colour (0‥1000).
//!
//! Each scan line holds every plane's bytes in turn, coded as pattern runs (`00 n` and a
//! pattern repeated n times), literal strings (`80 n` and n bytes) and solid runs (bit 7 the
//! value, the low bits the byte count); `00 00 FF n` before a line repeats it n times. Plane 0
//! is the lowest bit of the colour index. Without a palette, one plane is black on white and
//! four planes take GEM's standard VDI colours.
//!
//! `.img` is also a disk image's extension: the header is checked strictly ([`probe`]).

use crate::*;

/// The header facts.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Header {
    words: usize,
    planes: u32,
    pattern: usize,
    width: u32,
    height: u32,
    palette: Option<Vec<[u8; 4]>>,
}

/// GEM's standard VDI colours 0‥15.
const VDI: [[u8; 3]; 16] = [
    [255, 255, 255],
    [0, 0, 0],
    [255, 0, 0],
    [0, 255, 0],
    [0, 0, 255],
    [0, 255, 255],
    [255, 255, 0],
    [255, 0, 255],
    [192, 192, 192],
    [128, 128, 128],
    [128, 0, 0],
    [0, 128, 0],
    [0, 0, 128],
    [0, 128, 128],
    [128, 128, 0],
    [128, 0, 128],
];

fn header(data: &[u8]) -> Result<Header> {
    let w = |i: usize| u16be(data, i * 2).map(u32::from);
    let (Some(version), Some(words), Some(planes), Some(pattern), Some(width), Some(height)) =
        (w(0), w(1), w(2), w(3), w(6), w(7))
    else {
        return Err(Error::NotRecognised);
    };
    if !(1..=3).contains(&version)
        || !(8..=4096).contains(&words)
        || !(1..=8).contains(&pattern)
        || planes == 0
        || data.len() < words as usize * 2
    {
        return Err(Error::NotRecognised);
    }
    if !matches!(planes, 1..=8) {
        return Err(Error::Unsupported(format!(
            "GEM images with {planes} planes (true colour)"
        )));
    }
    check_dims(width, height)?;
    let mut palette = None;
    if words >= 11 && data.get(16..20) == Some(b"XIMG") && w(10) == Some(0) {
        let n = 1usize << planes;
        if words as usize >= 11 + 3 * n {
            let c = |i: usize| scale_to_8(w(11 + i).unwrap_or(0), 1000);
            palette = Some(
                (0..n)
                    .map(|k| [c(3 * k), c(3 * k + 1), c(3 * k + 2), 255])
                    .collect(),
            );
        }
    }
    Ok(Header {
        words: words as usize,
        planes,
        pattern: pattern as usize,
        width,
        height,
        palette,
    })
}

/// Whether the file starts with a plausible GEM image header (strict: `.img` is shared).
pub fn probe(head: &[u8]) -> bool {
    header(head).is_ok_and(|h| {
        // Pixel sizes in microns are small positive numbers on real files.
        let pw = u16be(head, 8).unwrap_or(0);
        let ph = u16be(head, 10).unwrap_or(0);
        pw <= 10_000 && ph <= 10_000 && (h.words == 8 || h.words == 9 || h.palette.is_some())
    })
}

fn palette(h: &Header) -> Vec<[u8; 4]> {
    if let Some(p) = &h.palette {
        return p.clone();
    }
    let n = 1usize << h.planes;
    match h.planes {
        1 => vec![[255, 255, 255, 255], [0, 0, 0, 255]],
        2..=4 => (0..n)
            .map(|i| {
                let [r, g, b] = VDI[i];
                [r, g, b, 255]
            })
            .collect(),
        _ => (0..n)
            .map(|i| {
                let v = scale_to_8(i as u32, n as u32 - 1);
                [v, v, v, 255]
            })
            .collect(),
    }
}

pub fn info(data: &[u8]) -> Result<Info> {
    let h = header(data)?;
    let what = if h.palette.is_some() { "XIMG" } else { "GEM" };
    Ok(if h.planes == 1 && h.palette.is_none() {
        Info::new(
            h.width,
            h.height,
            PixelFormat::Gray8,
            Model::Grey,
            1,
            format!("{what}, 1 plane"),
        )
    } else {
        Info::new(
            h.width,
            h.height,
            PixelFormat::Rgba8,
            Model::Palette,
            h.planes as u8,
            format!("{what}, {} planes", h.planes),
        )
    })
}

/// Decodes one plane's line into `out`; `None` when the data ends.
fn plane_line(d: &[u8], i: &mut usize, pattern: usize, out: &mut [u8]) -> Option<()> {
    let mut o = 0;
    while o < out.len() {
        let b = *d.get(*i)?;
        *i += 1;
        match b {
            0 => {
                let n = *d.get(*i)? as usize;
                *i += 1;
                let pat = d.get(*i..*i + pattern)?;
                *i += pattern;
                for _ in 0..n {
                    for &p in pat {
                        if o < out.len() {
                            out[o] = p;
                            o += 1;
                        }
                    }
                }
            }
            0x80 => {
                let n = *d.get(*i)? as usize;
                *i += 1;
                let lit = d.get(*i..*i + n)?;
                *i += n;
                let k = n.min(out.len() - o);
                out[o..o + k].copy_from_slice(&lit[..k]);
                o += k;
            }
            _ => {
                let n = (b & 0x7f) as usize;
                let v = if b & 0x80 != 0 { 0xff } else { 0 };
                let k = n.min(out.len() - o);
                out[o..o + k].fill(v);
                o += k;
            }
        }
    }
    Some(())
}

pub fn decode(data: &[u8]) -> Result<Image> {
    let h = header(data)?;
    let (w, ht) = (h.width as usize, h.height as usize);
    let bpl = w.div_ceil(8);
    let planes = h.planes as usize;
    check_payload(
        data.len(),
        (bpl * planes) as u64 * ht as u64,
        // A pattern run covers up to 255 × 8 bytes in 3 + 8.
        200,
    )?;
    let mut idx = vec![0u8; w * ht];
    let mut lines = vec![0u8; bpl * planes];
    let mut i = h.words * 2;
    let mut y = 0;
    let mut ok = true;
    while y < ht {
        let mut repeat = 1;
        if data.get(i..i + 3) == Some(&[0, 0, 0xff]) {
            repeat = data.get(i + 3).copied().unwrap_or(1).max(1) as usize;
            i += 4;
        }
        for p in 0..planes {
            if plane_line(data, &mut i, h.pattern, &mut lines[p * bpl..(p + 1) * bpl]).is_none() {
                ok = false;
                break;
            }
        }
        if !ok {
            break;
        }
        for _ in 0..repeat {
            if y >= ht {
                break;
            }
            let row = &mut idx[y * w..(y + 1) * w];
            for (x, v) in row.iter_mut().enumerate() {
                let mut c = 0u8;
                for p in 0..planes {
                    c |= (lines[p * bpl + x / 8] >> (7 - x % 8) & 1) << p;
                }
                *v = c;
            }
            y += 1;
        }
    }
    let rows = y;
    let pal = palette(&h);
    let mut img = if h.planes == 1 && h.palette.is_none() {
        let mut img = Image::filled(h.width, h.height, PixelFormat::Gray8, GREY);
        for (o, &c) in img.data.iter_mut().zip(&idx[..rows * w]) {
            *o = if c == 0 { 255 } else { 0 };
        }
        img
    } else {
        let mut img = Image::from_indices(h.width, h.height, &idx, &pal);
        for px in img.data[rows * w * 4..].chunks_exact_mut(4) {
            px.copy_from_slice(&[GREY, GREY, GREY, 255]);
        }
        img
    };
    img.truncated = rows < ht;
    Ok(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(v: &[u16]) -> Vec<u8> {
        v.iter().flat_map(|w| w.to_be_bytes()).collect()
    }

    #[test]
    fn mono_runs() {
        // 16×3: line 0 solid black then literal; line repeated twice; pattern line.
        let mut f = words(&[1, 8, 1, 2, 85, 85, 16, 3]);
        f.extend_from_slice(&[0, 0, 0xff, 2, 0x81, 0x80, 1, 0x0f]);
        f.extend_from_slice(&[0, 1, 0xaa, 0x55]);
        assert!(probe(&f));
        let i = info(&f).unwrap();
        assert_eq!((i.width, i.height), (16, 3));
        let img = decode(&f).unwrap();
        assert!(!img.truncated);
        assert!(img.data[..8].iter().all(|&v| v == 0));
        assert_eq!(&img.data[8..16], &[255, 255, 255, 255, 0, 0, 0, 0]);
        assert_eq!(&img.data[16..32], &img.data[..16]);
        assert_eq!(&img.data[32..36], &[0, 255, 0, 255]);
        assert_eq!(&img.data[40..44], &[255, 0, 255, 0]);
    }

    #[test]
    fn ximg_palette() {
        // 8×1, 2 planes, XIMG palette.
        let mut h = vec![1, 0, 2, 1, 85, 85, 8, 1];
        h.extend_from_slice(&[0x5849, 0x4d47, 0]);
        h.extend_from_slice(&[1000, 0, 0, 0, 1000, 0, 0, 0, 1000, 1000, 1000, 1000]);
        h[1] = h.len() as u16;
        let mut f = words(&h);
        // plane 0 = 0x0f, plane 1 = 0x33.
        f.extend_from_slice(&[0x80, 1, 0x0f, 0x80, 1, 0x33]);
        let i = info(&f).unwrap();
        assert!(i.coding.starts_with("XIMG"));
        let img = decode(&f).unwrap();
        // x=0: p0 0, p1 0 → 0 red; x=2: p0 0 p1 1 → 2 blue; x=4: 1,0 → 1 green; x=6: 3 white.
        assert_eq!(&img.data[..4], &[255, 0, 0, 255]);
        assert_eq!(&img.data[8..12], &[0, 0, 255, 255]);
        assert_eq!(&img.data[16..20], &[0, 255, 0, 255]);
        assert_eq!(&img.data[24..28], &[255, 255, 255, 255]);
    }

    #[test]
    fn not_gem() {
        assert!(!probe(&[0u8; 32]));
        assert!(!probe(b"\xeb\x3c\x90MSDOS5.0\0\x02\x01\x01\0\x02\xe0\0"));
    }
}
