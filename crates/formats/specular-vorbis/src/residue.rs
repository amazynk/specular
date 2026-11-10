//! Residues (spec section 8): the fine spectral structure, VQ-coded in partitions.

use crate::Error;
use crate::bits::BitReader;
use crate::codebook::Codebook;

fn malformed(s: &str) -> Error {
    Error::Malformed(s.to_string())
}

pub(crate) struct Residue {
    kind: u8,
    begin: usize,
    end: usize,
    psize: usize,
    classifications: u32,
    classbook: u8,
    /// Per classification, the book of each of the 8 passes (-1: none).
    books: Vec<[i16; 8]>,
}

/// One output bundle: `nv` vectors of `n` values each, vector `chans[j]` at
/// `buf[chans[j] * stride..]`.
pub(crate) struct Bundle<'a> {
    pub(crate) buf: &'a mut [f32],
    pub(crate) stride: usize,
    pub(crate) chans: &'a [usize],
    pub(crate) skip: &'a [bool],
    pub(crate) n: usize,
}

impl Residue {
    pub(crate) fn parse(br: &mut BitReader, books: &[Codebook]) -> Result<Residue, Error> {
        let eop = || malformed("the setup header ends early");
        let kind = br.read(16).ok_or_else(eop)?;
        if kind > 2 {
            return Err(Error::Unsupported(
                "Vorbis with a reserved residue type".to_string(),
            ));
        }
        let begin = br.read(24).ok_or_else(eop)? as usize;
        let end = br.read(24).ok_or_else(eop)? as usize;
        let psize = br.read(24).ok_or_else(eop)? as usize + 1;
        let classifications = br.read(6).ok_or_else(eop)? + 1;
        let classbook = br.read(8).ok_or_else(eop)?;
        match books.get(classbook as usize) {
            Some(b) if b.dims > 0 => {}
            _ => return Err(malformed("a residue names a missing codebook")),
        }
        let mut cascade = Vec::with_capacity(classifications as usize);
        for _ in 0..classifications {
            let low = br.read(3).ok_or_else(eop)?;
            let high = if br.bit().ok_or_else(eop)? {
                br.read(5).ok_or_else(eop)?
            } else {
                0
            };
            cascade.push(high * 8 + low);
        }
        let mut list = Vec::with_capacity(classifications as usize);
        for c in cascade {
            let mut row = [-1i16; 8];
            for (j, r) in row.iter_mut().enumerate() {
                if c >> j & 1 != 0 {
                    let b = br.read(8).ok_or_else(eop)?;
                    match books.get(b as usize) {
                        Some(book) if book.values.is_some() && book.dims > 0 => {}
                        _ => return Err(malformed("a residue codebook has no values")),
                    }
                    *r = b as i16;
                }
            }
            list.push(row);
        }
        Ok(Residue {
            kind: kind as u8,
            begin,
            end,
            psize,
            classifications,
            classbook: classbook as u8,
            books: list,
        })
    }

    /// Decodes into the bundle's vectors, which the caller has zeroed. The end of the
    /// packet just stops decode.
    pub(crate) fn decode(
        &self,
        br: &mut BitReader,
        books: &[Codebook],
        b: &mut Bundle,
        classes: &mut Vec<u8>,
    ) {
        let _ = self.try_decode(br, books, b, classes);
    }

    fn try_decode(
        &self,
        br: &mut BitReader,
        books: &[Codebook],
        b: &mut Bundle,
        classes: &mut Vec<u8>,
    ) -> Option<()> {
        let nch = b.chans.len();
        // Format 2 codes one interleaved vector of all channels.
        let (nv, size) = if self.kind == 2 {
            if b.skip.iter().all(|&s| s) {
                return Some(());
            }
            (1, b.n * nch)
        } else {
            (nch, b.n)
        };
        let begin = self.begin.min(size);
        let end = self.end.min(size);
        let parts = end.saturating_sub(begin) / self.psize;
        if parts == 0 {
            return Some(());
        }
        let classbook = &books[usize::from(self.classbook)];
        let cw = classbook.dims;
        // Class words past the last partition are read but not kept.
        let row = parts;
        classes.clear();
        classes.resize(nv * row, 0);
        let skip = |j: usize| self.kind != 2 && b.skip[j];
        for pass in 0..8 {
            let mut pc = 0;
            while pc < parts {
                if pass == 0 {
                    for j in 0..nv {
                        if skip(j) {
                            continue;
                        }
                        let mut temp = classbook.decode(br)?;
                        for i in (0..cw).rev() {
                            if i + pc < parts {
                                classes[j * row + i + pc] = (temp % self.classifications) as u8;
                            }
                            temp /= self.classifications;
                        }
                    }
                }
                let mut i = 0;
                while i < cw && pc < parts {
                    for j in 0..nv {
                        if skip(j) {
                            continue;
                        }
                        let class = usize::from(classes[j * row + pc]);
                        let book = self.books[class][pass];
                        if book < 0 {
                            continue;
                        }
                        let book = &books[book as usize];
                        let offset = begin + pc * self.psize;
                        match self.kind {
                            0 => self.format0(br, book, vector(b, j), offset)?,
                            1 => format1(br, book, vector(b, j), offset, self.psize)?,
                            _ => self.format2(br, book, b, offset)?,
                        }
                    }
                    i += 1;
                    pc += 1;
                }
            }
        }
        Some(())
    }

    fn format0(
        &self,
        br: &mut BitReader,
        book: &Codebook,
        v: &mut [f32],
        offset: usize,
    ) -> Option<()> {
        let step = self.psize / book.dims;
        for i in 0..step {
            let vals = book.decode_vq(br)?;
            for (j, &x) in vals.iter().enumerate() {
                if let Some(s) = v.get_mut(offset + i + j * step) {
                    *s += x;
                }
            }
        }
        Some(())
    }

    fn format2(
        &self,
        br: &mut BitReader,
        book: &Codebook,
        b: &mut Bundle,
        offset: usize,
    ) -> Option<()> {
        let nch = b.chans.len();
        let dims = book.dims;
        if nch == 1 {
            return format1(br, book, vector(b, 0), offset, self.psize);
        }
        let mut i = 0;
        while i < self.psize {
            let vals = book.decode_vq(br)?;
            let idx = offset + i;
            let (mut pos, mut ch) = (idx / nch, idx % nch);
            for &x in vals {
                if pos >= b.n {
                    break;
                }
                b.buf[b.chans[ch] * b.stride + pos] += x;
                ch += 1;
                if ch == nch {
                    ch = 0;
                    pos += 1;
                }
            }
            i += dims;
        }
        Some(())
    }
}

/// Vector `j` of the bundle.
fn vector<'b>(b: &'b mut Bundle, j: usize) -> &'b mut [f32] {
    let start = b.chans[j] * b.stride;
    &mut b.buf[start..start + b.n]
}

fn format1(
    br: &mut BitReader,
    book: &Codebook,
    v: &mut [f32],
    offset: usize,
    psize: usize,
) -> Option<()> {
    let dims = book.dims;
    let mut i = 0;
    while i < psize {
        let vals = book.decode_vq(br)?;
        let at = offset + i;
        if let Some(dst) = v.get_mut(at..at + dims) {
            for (d, &x) in dst.iter_mut().zip(vals) {
                *d += x;
            }
        } else {
            for (k, &x) in vals.iter().enumerate() {
                if let Some(s) = v.get_mut(at + k) {
                    *s += x;
                }
            }
        }
        i += dims;
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes values least significant bit first, like the Vorbis packer.
    #[derive(Default)]
    struct Writer {
        bytes: Vec<u8>,
        n: usize,
    }

    impl Writer {
        fn put(&mut self, v: u32, bits: u32) {
            for i in 0..bits {
                if self.n.is_multiple_of(8) {
                    self.bytes.push(0);
                }
                if v >> i & 1 != 0 {
                    *self.bytes.last_mut().unwrap() |= 1 << (self.n % 8);
                }
                self.n += 1;
            }
        }

        /// A codeword, first bit first.
        fn code(&mut self, code: u32, len: u32) {
            for i in (0..len).rev() {
                self.put(code >> i & 1, 1);
            }
        }
    }

    /// Book 0: 2 dimensions, 4 entries of 2-bit codewords, lookup type 1 over {-1, 1}:
    /// entry e is (±1 by bit 0, ±1 by bit 1). Book 1: one entry, 1 dimension, lookup type 2.
    fn books() -> Vec<Codebook> {
        let mut w = Writer::default();
        w.put(0x56_4342, 24);
        w.put(2, 16);
        w.put(4, 24);
        w.put(0, 1); // not ordered
        w.put(0, 1); // not sparse
        for _ in 0..4 {
            w.put(1, 5); // length 2
        }
        w.put(1, 4);
        w.put(0x8000_0000 | 788 << 21 | 1, 32); // minimum -1
        w.put(789 << 21 | 1, 32); // delta 2
        w.put(0, 4); // 1-bit values
        w.put(0, 1);
        w.put(0, 1);
        w.put(1, 1);
        w.put(0x56_4342, 24);
        w.put(1, 16);
        w.put(1, 24);
        w.put(0, 1);
        w.put(0, 1);
        w.put(0, 5); // length 1
        w.put(2, 4);
        w.put(0, 32);
        w.put(788 << 21 | 1, 32); // delta 1
        w.put(7, 4);
        w.put(0, 1);
        w.put(5, 8);
        let mut br = BitReader::new(&w.bytes);
        let mut budget = 1000;
        let a = Codebook::parse(&mut br, &mut budget).unwrap();
        let b = Codebook::parse(&mut br, &mut budget).unwrap();
        assert_eq!(
            a.values.as_deref().unwrap(),
            [-1., -1., 1., -1., -1., 1., 1., 1.]
        );
        assert_eq!(b.values.as_deref().unwrap(), [5.0]);
        vec![a, b]
    }

    fn run(kind: u8, bits: &Writer) -> Vec<f32> {
        let books = books();
        let r = Residue {
            kind,
            begin: 0,
            end: 8,
            psize: 4,
            classifications: 1,
            classbook: 1,
            books: vec![[0, -1, -1, -1, -1, -1, -1, -1]],
        };
        let mut buf = vec![0.0; 8];
        let mut b = Bundle {
            buf: &mut buf,
            stride: 4,
            chans: &[0, 1],
            skip: &[false, false],
            n: 4,
        };
        r.decode(
            &mut BitReader::new(&bits.bytes),
            &books,
            &mut b,
            &mut Vec::new(),
        );
        buf
    }

    #[test]
    fn formats() {
        // Formats 0 and 1: class words for both channels, then 2 entries per channel.
        let mut w = Writer::default();
        w.put(0, 2);
        for e in [1, 2, 3, 0] {
            w.code(e, 2);
        }
        assert_eq!(run(1, &w), [1., -1., -1., 1., 1., 1., -1., -1.]);
        // Format 0 spreads each entry with a step of 2.
        assert_eq!(run(0, &w), [1., -1., -1., 1., 1., -1., 1., -1.]);
        // Format 2: one interleaved vector of 8 in 2 partitions, a class word before each.
        let mut w = Writer::default();
        w.put(0, 1);
        w.code(1, 2);
        w.code(2, 2);
        w.put(0, 1);
        w.code(3, 2);
        w.code(0, 2);
        // Interleaved 1 -1 -1 1 | 1 1 -1 -1: the even ones to channel 0.
        assert_eq!(run(2, &w), [1., -1., 1., -1., -1., 1., 1., -1.]);
        // Cut short (6 bits and 2 of padding): what was read stays.
        let mut w = Writer::default();
        w.put(0, 2);
        w.code(1, 2);
        w.code(2, 2);
        assert_eq!(run(1, &w), [1., -1., -1., 1., -1., -1., 0., 0.]);
    }
}
