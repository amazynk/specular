//! The AV1 sequence header's colour description (AV1 spec §5.5): where its
//! `color_primaries` byte sits, so an AVIF ravif made can say the real primaries and
//! transfer in the bitstream as well as in the container's `nclx` (ravif 0.13 always writes
//! BT.709 / sRGB there). The header is patched in place: the fields are fixed-width, so no
//! size changes.

/// A bit reader (most significant bit first).
struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
}

impl Bits<'_> {
    fn f(&mut self, n: u32) -> Option<u64> {
        let mut v = 0u64;
        for _ in 0..n {
            let byte = *self.d.get(self.pos / 8)?;
            let bit = (byte >> (7 - self.pos % 8)) & 1;
            v = (v << 1) | u64::from(bit);
            self.pos += 1;
        }
        Some(v)
    }

    fn flag(&mut self) -> Option<bool> {
        self.f(1).map(|v| v == 1)
    }

    fn uvlc(&mut self) -> Option<u64> {
        let mut zeros = 0;
        while !self.flag()? {
            zeros += 1;
            if zeros >= 32 {
                return None;
            }
        }
        Some(self.f(zeros)? + (1u64 << zeros) - 1)
    }
}

/// The colour description of a sequence header OBU payload: the bit offset of
/// `color_primaries` (then `transfer_characteristics` and `matrix_coefficients`, 8 bits
/// each), when the header has one and is not monochrome.
pub fn colour_bits(seq: &[u8]) -> Option<usize> {
    let mut b = Bits { d: seq, pos: 0 };
    let profile = b.f(3)?;
    let _still = b.flag()?;
    let reduced = b.flag()?;
    if reduced {
        b.f(5)?;
    } else {
        let timing = b.flag()?;
        let mut decoder_model = false;
        let mut delay_len = 0;
        if timing {
            b.f(32)?;
            b.f(32)?;
            if b.flag()? {
                b.uvlc()?;
            }
            decoder_model = b.flag()?;
            if decoder_model {
                delay_len = b.f(5)? as u32 + 1;
                b.f(32)?;
                b.f(5)?;
                b.f(5)?;
            }
        }
        let initial_delay = b.flag()?;
        let ops = b.f(5)? + 1;
        for _ in 0..ops {
            b.f(12)?;
            let level = b.f(5)?;
            if level > 7 {
                b.f(1)?;
            }
            if decoder_model && b.flag()? {
                b.f(delay_len)?;
                b.f(delay_len)?;
                b.f(1)?;
            }
            if initial_delay && b.flag()? {
                b.f(4)?;
            }
        }
    }
    let wb = b.f(4)? as u32 + 1;
    let hb = b.f(4)? as u32 + 1;
    b.f(wb)?;
    b.f(hb)?;
    if !reduced && b.flag()? {
        b.f(4)?;
        b.f(3)?;
    }
    b.f(3)?; // 128x128 superblocks, filter intra, intra edge filter
    if !reduced {
        b.f(4)?; // interintra, masked compound, warped motion, dual filter
        let order_hint = b.flag()?;
        if order_hint {
            b.f(2)?; // jnt comp, ref frame mvs
        }
        let force_screen = if b.flag()? { 2 } else { b.f(1)? };
        if force_screen > 0 && !b.flag()? {
            b.f(1)?;
        }
        if order_hint {
            b.f(3)?;
        }
    }
    b.f(3)?; // superres, cdef, restoration
    let high = b.flag()?;
    if profile == 2 && high {
        b.f(1)?;
    }
    let mono = if profile == 1 { false } else { b.flag()? };
    let described = b.flag()?;
    (described && !mono).then_some(b.pos)
}

fn leb128(d: &[u8]) -> Option<(u64, usize)> {
    let mut v = 0u64;
    for i in 0..8 {
        let byte = *d.get(i)?;
        v |= u64::from(byte & 0x7f) << (7 * i);
        if byte & 0x80 == 0 {
            return Some((v, i + 1));
        }
    }
    None
}

/// Byte offsets (within `obus`) of the sequence headers' `color_primaries` bit positions:
/// `(payload start, bit offset)` for each sequence header OBU with a colour description,
/// walking OBUs that carry their size.
pub fn colour_fields(obus: &[u8]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < obus.len() {
        let h = obus[at];
        if h & 0x80 != 0 {
            break;
        }
        let typ = (h >> 3) & 15;
        let ext = h & 4 != 0;
        if h & 2 == 0 {
            break;
        }
        let start = at + 1 + usize::from(ext);
        let Some((len, n)) = obus.get(start..).and_then(leb128) else {
            break;
        };
        let payload = start + n;
        let Some(end) = usize::try_from(len)
            .ok()
            .and_then(|l| payload.checked_add(l))
        else {
            break;
        };
        if end > obus.len() {
            break;
        }
        if typ == 1
            && let Some(bit) = colour_bits(&obus[payload..end])
        {
            out.push((payload, bit));
        }
        at = end;
    }
    out
}

/// Writes `v` as `n` bits at bit `pos` of `d`.
fn put_bits(d: &mut [u8], pos: usize, n: usize, v: u64) {
    for i in 0..n {
        let bit = (v >> (n - 1 - i)) & 1;
        let p = pos + i;
        let mask = 1u8 << (7 - p % 8);
        if bit == 1 {
            d[p / 8] |= mask;
        } else {
            d[p / 8] &= !mask;
        }
    }
}

/// Sets the colour primaries and transfer of every described sequence header in `obus`;
/// how many it set.
pub fn set_colour(obus: &mut [u8], primaries: u8, transfer: u8) -> usize {
    let at = colour_fields(obus);
    for &(payload, bit) in &at {
        put_bits(&mut obus[payload..], bit, 8, u64::from(primaries));
        put_bits(&mut obus[payload..], bit + 8, 8, u64::from(transfer));
    }
    at.len()
}

/// The `(primaries, transfer, matrix)` of the first described sequence header in `obus`.
#[cfg(test)]
pub fn colour(obus: &[u8]) -> Option<(u8, u8, u8)> {
    let &(payload, bit) = colour_fields(obus).first()?;
    let mut b = Bits {
        d: &obus[payload..],
        pos: bit,
    };
    Some((b.f(8)? as u8, b.f(8)? as u8, b.f(8)? as u8))
}
