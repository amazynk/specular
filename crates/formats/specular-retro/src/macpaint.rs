//! MacPaint (`.mac`, `.pntg`): a 512-byte header (version and 38 fill patterns), then 720
//! rows of 72 bytes (576 pixels, 1 = black), each row PackBits-compressed on its own. Files
//! copied from a Mac often keep a 128-byte MacBinary header in front (file type `PNTG`).
//! No signature of its own: chosen by extension and checked by unpacking.

use crate::*;

pub const WIDTH: u32 = 576;
pub const HEIGHT: u32 = 720;
const ROW: usize = 72;
const HEADER: usize = 512;
const MACBINARY: usize = 128;

/// Whether `d` starts with a MacBinary header naming a MacPaint document.
fn macbinary(d: &[u8]) -> bool {
    d.len() > MACBINARY + HEADER
        && d[0] == 0
        && (1..=63).contains(&d[1])
        && d[74] == 0
        && d[82] == 0
        && &d[65..69] == b"PNTG"
}

/// Unpacks every row from `at`; the rows filled.
fn unpack(data: &[u8], at: usize, out: &mut [u8]) -> usize {
    let mut i = at;
    for (y, row) in out.chunks_exact_mut(ROW).enumerate() {
        let (n, ok) = unpackbits(data.get(i..).unwrap_or(&[]), row);
        if !ok {
            return y;
        }
        i += n;
    }
    HEIGHT as usize
}

/// Where the image starts: after a MacBinary header when there is one, or when the rows
/// only unpack fully from there.
fn start(data: &[u8]) -> Result<usize> {
    if data.len() < HEADER + 2 {
        return Err(truncated("header"));
    }
    let version = u32be(data, 0).unwrap_or(0);
    if macbinary(data) || version > 3 {
        let mut scratch = vec![0; ROW * HEIGHT as usize];
        if unpack(data, MACBINARY + HEADER, &mut scratch) == HEIGHT as usize || macbinary(data) {
            return Ok(MACBINARY + HEADER);
        }
    }
    Ok(HEADER)
}

pub fn info(data: &[u8]) -> Result<Info> {
    let at = start(data)?;
    let mb = if at > HEADER { ", MacBinary" } else { "" };
    Ok(Info::new(
        WIDTH,
        HEIGHT,
        PixelFormat::Gray8,
        Model::Grey,
        1,
        format!("1-bit, PackBits{mb}"),
    ))
}

pub fn decode(data: &[u8]) -> Result<Image> {
    let at = start(data)?;
    let mut bits = vec![0u8; ROW * HEIGHT as usize];
    let rows = unpack(data, at, &mut bits);
    let mut img = Image::filled(WIDTH, HEIGHT, PixelFormat::Gray8, 255);
    img.truncated = rows < HEIGHT as usize;
    for (o, b) in img.data.chunks_exact_mut(8).zip(&bits[..rows * ROW]) {
        for (k, px) in o.iter_mut().enumerate() {
            *px = if b >> (7 - k) & 1 != 0 { 0 } else { 255 };
        }
    }
    for px in &mut img.data[rows * WIDTH as usize..] {
        *px = GREY;
    }
    Ok(img)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// PackBits-encodes one row (simple: runs of 3+ as repeats, the rest literal).
    pub fn packbits(row: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < row.len() {
            let mut run = 1;
            while i + run < row.len() && row[i + run] == row[i] && run < 128 {
                run += 1;
            }
            if run >= 3 {
                out.push((1 - run as i32) as i8 as u8);
                out.push(row[i]);
                i += run;
            } else {
                let s = i;
                while i < row.len()
                    && i - s < 128
                    && !(i + 2 < row.len() && row[i] == row[i + 1] && row[i] == row[i + 2])
                {
                    i += 1;
                }
                out.push((i - s - 1) as u8);
                out.extend_from_slice(&row[s..i]);
            }
        }
        out
    }

    pub fn build(bits: &[u8], macbin: bool) -> Vec<u8> {
        let mut f = Vec::new();
        if macbin {
            let mut mb = vec![0u8; MACBINARY];
            mb[1] = 4;
            mb[2..6].copy_from_slice(b"test");
            mb[65..69].copy_from_slice(b"PNTG");
            mb[69..73].copy_from_slice(b"MPNT");
            f.extend_from_slice(&mb);
        }
        f.extend_from_slice(&[0, 0, 0, 2]);
        f.resize(f.len() + HEADER - 4, 0);
        for row in bits.chunks(ROW) {
            f.extend_from_slice(&packbits(row));
        }
        f
    }

    #[test]
    fn round_trip() {
        let mut bits = vec![0u8; ROW * HEIGHT as usize];
        for (i, b) in bits.iter_mut().enumerate() {
            *b = if i % 7 == 0 {
                0xa5
            } else {
                (i / 72 % 3) as u8 * 0x0f
            };
        }
        for mb in [false, true] {
            let f = build(&bits, mb);
            let i = info(&f).unwrap();
            assert_eq!(i.coding.contains("MacBinary"), mb);
            let img = decode(&f).unwrap();
            assert!(!img.truncated);
            // 0xa5 = 10100101.
            assert_eq!(&img.data[..8], &[0, 255, 0, 255, 255, 0, 255, 0]);
        }
        let f = build(&bits, false);
        assert!(decode(&f[..2000]).unwrap().truncated);
    }
}
