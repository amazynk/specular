//! Olympus / OM System compressed ORF sensor data (12-bit).
//!
//! One MSB-first bit stream for the whole frame, starting 7 bytes into the strip. Each sample
//! codes a residual as 3 bits (sign and the 2 low bits), a unary-coded high part and an
//! adaptive number of raw bits; two adaptive states alternate by column parity. The residual is
//! added to a prediction from the same-colour neighbours two pixels to the left, above and
//! above-left (a median-like choice between them).
//!
//! The stream has no restart points, so decoding is sequential.

use crate::bits::BitReader;

/// Bytes of the strip before the bit stream.
const SKIP: usize = 7;

#[derive(Clone, Copy, Default)]
struct Carry {
    /// Magnitude of the last high/low code.
    mag: i32,
    /// Running residual average.
    avg: i32,
    /// Count of consecutive small magnitudes.
    run: i32,
}

/// Decode `width x height` samples into `out` (row-major). Returns whether the stream ran out
/// of data or produced out-of-range samples.
pub(crate) fn decode(src: &[u8], width: usize, height: usize, out: &mut [u16]) -> bool {
    let out = &mut out[..width * height];
    let mut br = BitReader::new(src.get(SKIP..).unwrap_or(&[]), false);
    let mut bad = false;
    for row in 0..height {
        let mut carry = [Carry::default(); 2];
        let (above, cur) = out.split_at_mut(row * width);
        let cur = &mut cur[..width];
        let up2 = row
            .checked_sub(2)
            .map(|r| &above[r * width..(r + 1) * width]);
        for col in 0..width {
            let c = &mut carry[col & 1];
            br.refill();
            let i: u32 = if c.run < 3 { 2 } else { 0 };
            // Smallest n >= 2 + i with (mag & 0xffff) >> (n + i) == 0.
            let len = 32 - (c.mag as u32 & 0xffff).leading_zeros();
            let nbits = (2 + i).max(len.saturating_sub(i));
            let s3 = br.peek(3);
            br.consume(3);
            let low = (s3 & 3) as i32;
            let sign = if s3 & 4 != 0 { -1 } else { 0 };
            let lz = (br.peek(12).leading_zeros() as i32 - 20).min(12);
            let high = if lz >= 12 {
                br.consume(12);
                (br.bits(16 - nbits) >> 1) as i32
            } else {
                br.consume(lz as u32 + 1);
                lz
            };
            c.mag = (high << nbits) | br.bits(nbits) as i32;
            let diff = (c.mag ^ sign) + c.avg;
            c.avg = (diff * 3 + c.avg) >> 5;
            c.run = if c.mag > 16 { 0 } else { c.run + 1 };
            let pred = match up2 {
                None if col < 2 => 0,
                None => cur[col - 2] as i32,
                Some(u) if col < 2 => u[col] as i32,
                Some(u) => {
                    let w = cur[col - 2] as i32;
                    let n = u[col] as i32;
                    let nw = u[col - 2] as i32;
                    if (w < nw && nw < n) || (n < nw && nw < w) {
                        if (w - nw).abs() > 32 || (n - nw).abs() > 32 {
                            w + n - nw
                        } else {
                            (w + n) >> 1
                        }
                    } else if (w - nw).abs() > (n - nw).abs() {
                        w
                    } else {
                        n
                    }
                }
            };
            let v = pred.wrapping_add(diff.wrapping_mul(4) | low);
            bad |= (v as u32 & 0xffff) >> 12 != 0;
            cur[col] = v as u16;
        }
        if br.overrun() {
            // Rest of the frame stays zero.
            return true;
        }
    }
    bad
}

#[cfg(test)]
mod tests {
    #[test]
    fn garbage_does_not_panic() {
        let mut s = 1u32;
        for len in [0usize, 5, 7, 8, 100, 5000] {
            let src: Vec<u8> = (0..len)
                .map(|_| {
                    s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
                    (s >> 16) as u8
                })
                .collect();
            let mut out = vec![0u16; 64 * 16];
            let _ = super::decode(&src, 64, 16, &mut out);
        }
    }
}
