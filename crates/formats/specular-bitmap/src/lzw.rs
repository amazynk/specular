//! GIF-flavoured LZW decoding (variable code size 3..=12 bits, LSB-first, clear and end codes,
//! deferred clear when the table is full).
//!
//! Each code's string is written straight into the output, back to front along the prefix
//! chain, using a stored length; there is no intermediate stack.

const MAX: usize = 4096;

pub(crate) struct Table {
    prefix: [u16; MAX],
    suffix: [u8; MAX],
    first: [u8; MAX],
    len: [u16; MAX],
}

pub(crate) struct Outcome {
    /// Pixels written.
    pub written: usize,
    /// The stream ended (or broke) before filling the output or reaching the end code.
    pub truncated: bool,
}

impl Table {
    pub fn new() -> Box<Table> {
        Box::new(Table {
            prefix: [0; MAX],
            suffix: [0; MAX],
            first: [0; MAX],
            len: [0; MAX],
        })
    }

    /// Decode `input` (sub-blocks already joined) into `out`, stopping when it is full.
    pub fn decode(&mut self, min_size: u8, input: &[u8], out: &mut [u8]) -> Outcome {
        let min = min_size.clamp(2, 11) as u32;
        let clear = 1usize << min;
        let eoi = clear + 1;
        for c in 0..clear {
            self.suffix[c] = c as u8;
            self.first[c] = c as u8;
            self.len[c] = 1;
            self.prefix[c] = 0;
        }
        let mut next = clear + 2;
        let mut size = min + 1;
        let mut prev: Option<usize> = None;
        let mut bits: u64 = 0;
        let mut nbits: u32 = 0;
        let mut p = 0usize;
        let mut pos = 0usize;
        let n = out.len();
        while pos < n {
            if nbits < size {
                while nbits <= 56 {
                    let Some(&b) = input.get(p) else { break };
                    bits |= (b as u64) << nbits;
                    nbits += 8;
                    p += 1;
                }
                if nbits < size {
                    return Outcome {
                        written: pos,
                        truncated: true,
                    };
                }
            }
            let code = (bits & ((1u64 << size) - 1)) as usize;
            bits >>= size;
            nbits -= size;
            if code == clear {
                next = clear + 2;
                size = min + 1;
                prev = None;
                continue;
            }
            if code == eoi {
                return Outcome {
                    written: pos,
                    truncated: false,
                };
            }
            let Some(pv) = prev else {
                if code > clear {
                    // Invalid first code: treat as corrupt data.
                    return Outcome {
                        written: pos,
                        truncated: true,
                    };
                }
                out[pos] = code as u8;
                pos += 1;
                prev = Some(code);
                continue;
            };
            let (c, kwk) = if code < next {
                (code, false)
            } else if code == next && next < MAX {
                (pv, true)
            } else {
                return Outcome {
                    written: pos,
                    truncated: true,
                };
            };
            let l = self.len[c] as usize;
            let first = self.first[c];
            // Write the string of `c` back to front (clipped to the output).
            let end = pos + l;
            let mut k = c;
            let mut i = end;
            while i > pos {
                i -= 1;
                if i < n {
                    out[i] = self.suffix[k];
                }
                k = self.prefix[k] as usize;
            }
            pos = end;
            if kwk {
                if pos < n {
                    out[pos] = first;
                }
                pos += 1;
            }
            if next < MAX {
                self.prefix[next] = pv as u16;
                self.suffix[next] = first;
                self.first[next] = self.first[pv];
                self.len[next] = self.len[pv] + 1;
                next += 1;
                if next == (1 << size) && size < 12 {
                    size += 1;
                }
            }
            prev = Some(code);
        }
        Outcome {
            written: pos.min(n),
            truncated: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A straightforward LZW encoder (no clear codes except the first) for round trips.
    fn encode(min: u8, data: &[u8]) -> Vec<u8> {
        use std::collections::HashMap;
        let clear = 1u32 << min;
        let mut dict: HashMap<Vec<u8>, u32> = HashMap::new();
        let mut next = clear + 2;
        let mut size = min as u32 + 1;
        let mut out = Vec::new();
        let (mut acc, mut nb) = (0u64, 0u32);
        let mut emit = |c: u32, size: u32, out: &mut Vec<u8>| {
            acc |= (c as u64) << nb;
            nb += size;
            while nb >= 8 {
                out.push(acc as u8);
                acc >>= 8;
                nb -= 8;
            }
        };
        emit(clear, size, &mut out);
        let mut w: Vec<u8> = Vec::new();
        for &b in data {
            let mut wb = w.clone();
            wb.push(b);
            if wb.len() == 1 || dict.contains_key(&wb) {
                w = wb;
                continue;
            }
            let code = if w.len() == 1 { w[0] as u32 } else { dict[&w] };
            emit(code, size, &mut out);
            if next < 4096 {
                dict.insert(wb, next);
                next += 1;
                if next > (1 << size) && size < 12 {
                    size += 1;
                }
            }
            w = vec![b];
        }
        if !w.is_empty() {
            let code = if w.len() == 1 { w[0] as u32 } else { dict[&w] };
            emit(code, size, &mut out);
        }
        emit(clear + 1, size, &mut out);
        if nb > 0 {
            out.push(acc as u8);
        }
        out
    }

    #[test]
    fn round_trip() {
        let mut t = Table::new();
        for (min, len) in [(2u8, 1000usize), (8, 100_000), (4, 50_000)] {
            let mask = (1u32 << min) - 1;
            let mut s = 12345u32;
            let data: Vec<u8> = (0..len)
                .map(|i| {
                    s = s.wrapping_mul(1103515245).wrapping_add(12345);
                    (if i % 7 < 4 { (i / 50) as u32 } else { s >> 16 } & mask) as u8
                })
                .collect();
            let enc = encode(min, &data);
            let mut out = vec![0u8; len];
            let r = t.decode(min, &enc, &mut out);
            assert!(!r.truncated);
            assert_eq!(r.written, len);
            assert_eq!(out, data);
            // Truncated input never panics.
            for cut in [0, 1, enc.len() / 3, enc.len() - 1] {
                let mut out = vec![0u8; len];
                let r = t.decode(min, &enc[..cut], &mut out);
                assert!(r.written <= len);
            }
        }
    }
}
