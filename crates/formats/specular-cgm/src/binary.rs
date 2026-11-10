//! The binary encoding (ISO/IEC 8632-3): element headers, partitions, and the parameter types
//! at the precisions the metafile sets.

use std::borrow::Cow;

use crate::args::{Args, Prec, RealPrec};

/// Largest parameter list of one element after joining its partitions (cell arrays of big
/// images are split into many partitions).
pub(crate) const MAX_ELEMENT_BYTES: usize = 256 << 20;

/// One element as stored: class, id and its parameter bytes (partitions joined).
pub(crate) struct Raw<'a> {
    pub class: u8,
    pub id: u8,
    pub data: Cow<'a, [u8]>,
}

/// The elements of a binary metafile, in order. Stops at the end of the data or at the first
/// element that does not fit (a truncated file draws what came before).
pub(crate) struct Elements<'a> {
    data: &'a [u8],
    pos: usize,
    /// The file ended inside an element.
    pub truncated: bool,
}

impl<'a> Elements<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            truncated: false,
        }
    }

    fn word(&mut self) -> Option<u16> {
        let b = self.data.get(self.pos..self.pos + 2)?;
        self.pos += 2;
        Some(u16::from_be_bytes([b[0], b[1]]))
    }

    fn take(&mut self, len: usize) -> Option<&'a [u8]> {
        let d = self.data.get(self.pos..self.pos.checked_add(len)?)?;
        // Parameter lists are padded to a 16-bit boundary.
        self.pos += len + (len & 1);
        Some(d)
    }
}

impl<'a> Iterator for Elements<'a> {
    type Item = Raw<'a>;

    fn next(&mut self) -> Option<Raw<'a>> {
        if self.pos + 2 > self.data.len() {
            return None;
        }
        let h = self.word()?;
        let class = (h >> 12) as u8;
        let id = ((h >> 5) & 0x7f) as u8;
        let short = usize::from(h & 0x1f);
        if short != 31 {
            let Some(d) = self.take(short) else {
                self.truncated = true;
                self.pos = self.data.len();
                return None;
            };
            return Some(Raw {
                class,
                id,
                data: Cow::Borrowed(d),
            });
        }
        // Long form: a length word with a "more partitions follow" flag.
        let mut joined: Option<Vec<u8>> = None;
        let mut first: &'a [u8] = &[];
        loop {
            let Some(lw) = self.word() else {
                self.truncated = true;
                self.pos = self.data.len();
                return None;
            };
            let more = lw & 0x8000 != 0;
            let len = usize::from(lw & 0x7fff);
            let Some(d) = self.take(len) else {
                self.truncated = true;
                self.pos = self.data.len();
                return None;
            };
            match &mut joined {
                Some(v) => {
                    if v.len() + d.len() > MAX_ELEMENT_BYTES {
                        self.truncated = true;
                        self.pos = self.data.len();
                        return None;
                    }
                    v.extend_from_slice(d);
                }
                None if more => {
                    let mut v = Vec::with_capacity(first.len() + d.len() * 2);
                    v.extend_from_slice(d);
                    joined = Some(v);
                }
                None => first = d,
            }
            if !more {
                break;
            }
        }
        Some(Raw {
            class,
            id,
            data: match joined {
                Some(v) => Cow::Owned(v),
                None => Cow::Borrowed(first),
            },
        })
    }
}

/// Reads parameters of one element at the current precisions.
pub(crate) struct Params<'a> {
    d: &'a [u8],
    /// Byte position.
    p: usize,
    prec: &'a Prec,
}

impl<'a> Params<'a> {
    pub fn new(d: &'a [u8], prec: &'a Prec) -> Self {
        Self { d, p: 0, prec }
    }

    fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let b = self.d.get(self.p..self.p.checked_add(n)?)?;
        self.p += n;
        Some(b)
    }

    fn signed(&mut self, bits: u8) -> Option<i64> {
        let n = usize::from(bits.clamp(8, 32) / 8);
        let b = self.bytes(n)?;
        let mut v: i64 = 0;
        for &x in b {
            v = (v << 8) | i64::from(x);
        }
        let shift = 64 - 8 * n as u32;
        Some((v << shift) >> shift)
    }

    fn unsigned(&mut self, bits: u8) -> Option<u64> {
        let n = usize::from(bits.clamp(8, 32) / 8);
        let b = self.bytes(n)?;
        Some(b.iter().fold(0u64, |v, &x| (v << 8) | u64::from(x)))
    }

    fn real_at(&mut self, r: RealPrec) -> Option<f64> {
        Some(match r {
            RealPrec::Float32 => {
                let b = self.bytes(4)?;
                f64::from(f32::from_be_bytes([b[0], b[1], b[2], b[3]]))
            }
            RealPrec::Float64 => {
                let b = self.bytes(8)?;
                f64::from_be_bytes(b.try_into().ok()?)
            }
            RealPrec::Fixed32 => {
                let whole = self.signed(16)?;
                let frac = self.unsigned(16)?;
                whole as f64 + frac as f64 / 65536.0
            }
            RealPrec::Fixed64 => {
                let whole = self.signed(32)?;
                let frac = self.unsigned(32)?;
                whole as f64 + frac as f64 / 4_294_967_296.0
            }
        })
    }

    /// Skips to the next 16-bit boundary of the parameter list (cell array rows).
    fn align(&mut self) {
        self.p += self.p & 1;
    }
}

/// Bits per value of a precision: 8, 16, 24 or 32 for integers; colour may be 1, 2 or 4 too.
fn colour_bits(bits: u8) -> u8 {
    match bits {
        1 | 2 | 4 | 8 | 16 | 24 | 32 => bits,
        _ => 8,
    }
}

impl Args for Params<'_> {
    fn more(&self) -> bool {
        self.p < self.d.len()
    }

    fn int(&mut self) -> Option<i64> {
        self.signed(self.prec.int)
    }

    fn index(&mut self) -> Option<i64> {
        self.signed(self.prec.index)
    }

    fn enumv(&mut self, _names: &[&str]) -> Option<i64> {
        self.signed(16)
    }

    fn real(&mut self) -> Option<f64> {
        self.real_at(self.prec.real)
    }

    fn vdc(&mut self) -> Option<f64> {
        if self.prec.vdc_real {
            self.real_at(self.prec.vdc_real_prec)
        } else {
            self.signed(self.prec.vdc_int).map(|v| v as f64)
        }
    }

    fn ci(&mut self) -> Option<u32> {
        self.unsigned(self.prec.colour_index)
            .map(|v| u32::try_from(v).unwrap_or(u32::MAX))
    }

    fn cd(&mut self) -> Option<[f64; 4]> {
        let bits = self.prec.colour;
        let mut c = [0.0; 4];
        for v in c.iter_mut().take(usize::from(self.prec.components)) {
            *v = self.unsigned(bits)? as f64;
        }
        Some(c)
    }

    fn string(&mut self) -> Option<Vec<u8>> {
        let n = *self.bytes(1)?.first()?;
        if n < 255 {
            return self.bytes(usize::from(n)).map(<[u8]>::to_vec);
        }
        // Long string: partial strings, each with a 16-bit length and a continuation flag.
        let mut out = Vec::new();
        loop {
            let w = self.unsigned(16)? as u16;
            let len = usize::from(w & 0x7fff);
            let part = match self.bytes(len) {
                Some(b) => b,
                None => {
                    // Truncated: keep what is there.
                    let b = &self.d[self.p.min(self.d.len())..];
                    self.p = self.d.len();
                    b
                }
            };
            out.extend_from_slice(part);
            if w & 0x8000 == 0 || !self.more() {
                break;
            }
        }
        Some(out)
    }

    fn cell_mode(&mut self) -> i64 {
        self.signed(16).unwrap_or(1)
    }

    fn cells(
        &mut self,
        nx: usize,
        ny: usize,
        local_prec: i64,
        mode: i64,
        indexed: bool,
        out: &mut dyn FnMut(CellColour),
    ) -> Option<()> {
        let bits = if local_prec > 0 {
            colour_bits(u8::try_from(local_prec).unwrap_or(8))
        } else if indexed {
            self.prec.colour_index
        } else {
            self.prec.colour
        };
        let comps = if indexed {
            1
        } else {
            usize::from(self.prec.components)
        };
        // Cell rows start on a 16-bit boundary in both representations.
        for _ in 0..ny {
            let mut bitr = BitReader {
                d: self.d,
                byte: self.p,
                bit: 0,
            };
            let mut done = 0usize;
            if mode == 0 {
                // Run-length: (count, colour) pairs, bit-packed; counts at integer precision.
                let int_bits = self.prec.int.clamp(8, 32);
                while done < nx {
                    let count = usize::try_from(bitr.read(int_bits)?).unwrap_or(0);
                    let col = read_cell(&mut bitr, bits, comps, indexed)?;
                    if count == 0 {
                        // A zero run would never end the row.
                        return None;
                    }
                    let n = count.min(nx - done);
                    for _ in 0..n {
                        out(col);
                    }
                    done += n;
                }
            } else {
                while done < nx {
                    let col = read_cell(&mut bitr, bits, comps, indexed)?;
                    out(col);
                    done += 1;
                }
            }
            self.p = bitr.byte_aligned();
            self.align();
        }
        Some(())
    }
}

/// A cell array or pattern colour as stored: an index, or direct components.
#[derive(Clone, Copy, Debug)]
pub(crate) enum CellColour {
    Index(u32),
    Direct([f64; 4]),
}

fn read_cell(b: &mut BitReader<'_>, bits: u8, comps: usize, indexed: bool) -> Option<CellColour> {
    if indexed {
        return Some(CellColour::Index(b.read(bits)? as u32));
    }
    let mut c = [0.0; 4];
    for v in c.iter_mut().take(comps) {
        *v = b.read(bits)? as f64;
    }
    Some(CellColour::Direct(c))
}

struct BitReader<'a> {
    d: &'a [u8],
    byte: usize,
    bit: u8,
}

impl BitReader<'_> {
    fn read(&mut self, bits: u8) -> Option<u64> {
        if bits.is_multiple_of(8) && self.bit == 0 {
            let n = usize::from(bits / 8);
            let b = self.d.get(self.byte..self.byte + n)?;
            self.byte += n;
            return Some(b.iter().fold(0u64, |v, &x| (v << 8) | u64::from(x)));
        }
        let mut v = 0u64;
        for _ in 0..bits {
            let x = *self.d.get(self.byte)?;
            let bit = (x >> (7 - self.bit)) & 1;
            v = (v << 1) | u64::from(bit);
            self.bit += 1;
            if self.bit == 8 {
                self.bit = 0;
                self.byte += 1;
            }
        }
        Some(v)
    }

    fn byte_aligned(&self) -> usize {
        self.byte + usize::from(self.bit > 0)
    }
}
