//! Cinepak: a frame is cut into strips, each with a V1 and a V4 codebook of up to 256
//! entries (four luma samples in a 2×2 square, then Cb and Cr; or four luma or palette
//! samples). A 4×4 block is one V1 entry, each sample doubled, or four V4 entries, one per
//! 2×2 quarter. Inter strips update codebook entries selectively and skip blocks.

use crate::{Pixels, Reader, Result, bad};

/// One strip's codebooks: four luma (or palette) samples, then Cb and Cr.
#[derive(Clone)]
struct Book {
    v1: [[u8; 6]; 256],
    v4: [[u8; 6]; 256],
}

impl Default for Book {
    fn default() -> Self {
        Book {
            v1: [[0, 0, 0, 0, 128, 128]; 256],
            v4: [[0, 0, 0, 0, 128, 128]; 256],
        }
    }
}

/// The codebooks of every strip, kept from frame to frame.
#[derive(Default)]
pub struct Books {
    strips: Vec<Book>,
}

/// Bits read most significant first from 32-bit words in the stream.
struct Flags {
    word: u32,
    left: u32,
}

impl Flags {
    fn new() -> Flags {
        Flags { word: 0, left: 0 }
    }
    fn bit(&mut self, r: &mut Reader<'_>) -> Result<bool> {
        if self.left == 0 {
            self.word = r.u32be()?;
            self.left = 32;
        }
        self.left -= 1;
        Ok(self.word >> self.left & 1 != 0)
    }
}

/// Reads a codebook chunk into `book`: `six` entries of six bytes (else four, Cb and Cr
/// neutral), selectively under 32-bit masks when `selective`.
fn read_book(
    r: &mut Reader<'_>,
    book: &mut [[u8; 6]; 256],
    six: bool,
    selective: bool,
) -> Result<()> {
    let size = if six { 6 } else { 4 };
    let mut flags = Flags::new();
    let mut i = 0;
    while i < 256 && r.left() >= size {
        if selective && !flags.bit(r)? {
            i += 1;
            continue;
        }
        if r.left() < size {
            break;
        }
        let e = r.bytes(size)?;
        let mut v = [0u8, 0, 0, 0, 128, 128];
        v[..size].copy_from_slice(e);
        if six {
            // Cb and Cr are signed; kept offset by 128.
            v[4] = e[4] ^ 0x80;
            v[5] = e[5] ^ 0x80;
        }
        book[i] = v;
        i += 1;
    }
    Ok(())
}

/// The RGB of a luma sample with the entry's chroma (Cinepak's own transform).
#[inline(always)]
fn rgb(y: u8, e: &[u8; 6]) -> [u8; 3] {
    let y = i32::from(y);
    let u = i32::from(e[4]) - 128;
    let v = i32::from(e[5]) - 128;
    let c = |x: i32| x.clamp(0, 255) as u8;
    [c(y + 2 * v), c(y - u / 2 - v), c(y + 2 * u)]
}

struct Target<'a> {
    canvas: &'a mut Pixels,
    w: usize,
    h: usize,
}

impl Target<'_> {
    /// Paints the 2×2 square at (x, y) from an entry: its four samples, or one sample doubled
    /// (`one` = Some(index)).
    #[inline(always)]
    fn square(&mut self, x: usize, y: usize, e: &[u8; 6], one: Option<usize>) {
        for dy in 0..2 {
            for dx in 0..2 {
                let (px, py) = (x + dx, y + dy);
                if px >= self.w || py >= self.h {
                    continue;
                }
                let s = e[one.unwrap_or(dy * 2 + dx)];
                let i = py * self.w + px;
                match self.canvas {
                    Pixels::Rgb24(p) => p[3 * i..3 * i + 3].copy_from_slice(&rgb(s, e)),
                    Pixels::Pal8(p) => p[i] = s,
                    _ => {}
                }
            }
        }
    }

    fn v1(&mut self, x: usize, y: usize, e: &[u8; 6]) {
        for (k, (dx, dy)) in [(0, 0), (2, 0), (0, 2), (2, 2)].into_iter().enumerate() {
            self.square(x + dx, y + dy, e, Some(k));
        }
    }

    fn v4(&mut self, x: usize, y: usize, es: [&[u8; 6]; 4]) {
        for (k, (dx, dy)) in [(0, 0), (2, 0), (0, 2), (2, 2)].into_iter().enumerate() {
            self.square(x + dx, y + dy, es[k], None);
        }
    }
}

pub fn decode(
    data: &[u8],
    w: usize,
    h: usize,
    books: &mut Books,
    canvas: &mut Pixels,
) -> Result<()> {
    let mut r = Reader::new(data);
    if data.len() < 10 {
        return Ok(());
    }
    let flags = r.u8()?;
    let _len = r.u24be()?;
    let _fw = r.u16be()?;
    let _fh = r.u16be()?;
    let strips = usize::from(r.u16be()?);
    if strips > 32 {
        return Err(bad("strip count"));
    }
    if books.strips.len() < strips {
        books.strips.resize_with(strips, Book::default);
    }
    let mut t = Target { canvas, w, h };
    let mut prev_y2 = 0usize;
    for s in 0..strips {
        if r.left() < 12 {
            break;
        }
        let id = r.u8()?;
        let size = r.u24be()? as usize;
        let y1 = usize::from(r.u16be()?);
        let x1 = usize::from(r.u16be()?);
        let y2 = usize::from(r.u16be()?);
        let x2 = usize::from(r.u16be()?);
        if !matches!(id, 0x10 | 0x11) || size < 12 {
            return Err(bad("strip header"));
        }
        // A strip whose top is 0 after the first gives its height and follows the one above.
        let (y1, y2) = if y1 == 0 && s > 0 {
            (prev_y2, prev_y2 + y2)
        } else {
            (y1, y2)
        };
        prev_y2 = y2;
        let body = r.bytes((size - 12).min(r.left()))?;
        if s > 0 && flags & 1 == 0 {
            let prev = books.strips[s - 1].clone();
            books.strips[s] = prev;
        }
        let book = &mut books.strips[s];
        let mut c = Reader::new(body);
        while c.left() >= 4 {
            let cid = c.u8()?;
            let csize = c.u24be()? as usize;
            if csize < 4 {
                return Err(bad("chunk size"));
            }
            let chunk = c.bytes((csize - 4).min(c.left()))?;
            let mut cr = Reader::new(chunk);
            match cid {
                0x20..=0x27 => {
                    let six = cid & 0x04 == 0;
                    let selective = cid & 0x01 != 0;
                    let b = if cid & 0x02 != 0 {
                        &mut book.v1
                    } else {
                        &mut book.v4
                    };
                    read_book(&mut cr, b, six, selective)?;
                }
                0x30..=0x32 => {
                    let mut f = Flags::new();
                    let mut y = y1;
                    'rows: while y < y2.min(h) {
                        let mut x = x1;
                        while x < x2.min(w) {
                            if cr.left() == 0 {
                                break 'rows;
                            }
                            let kind = match cid {
                                0x32 => 1,
                                0x30 => {
                                    if f.bit(&mut cr)? {
                                        4
                                    } else {
                                        1
                                    }
                                }
                                _ => {
                                    if !f.bit(&mut cr)? {
                                        0
                                    } else if f.bit(&mut cr)? {
                                        4
                                    } else {
                                        1
                                    }
                                }
                            };
                            match kind {
                                1 => {
                                    let e = book.v1[usize::from(cr.u8()?)];
                                    t.v1(x, y, &e);
                                }
                                4 => {
                                    let i = cr.bytes(4)?;
                                    let es = [
                                        &book.v4[usize::from(i[0])],
                                        &book.v4[usize::from(i[1])],
                                        &book.v4[usize::from(i[2])],
                                        &book.v4[usize::from(i[3])],
                                    ];
                                    t.v4(x, y, es);
                                }
                                _ => {}
                            }
                            x += 4;
                        }
                        y += 4;
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}
