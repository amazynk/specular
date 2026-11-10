//! Fast byte searches over large mapped files, eight bytes at a time.

const LO: u64 = 0x0101_0101_0101_0101;
const HI: u64 = 0x8080_8080_8080_8080;

/// Whether some byte of `x` is zero, as the high bit of that byte (exact for the lowest
/// such byte).
#[inline]
fn zero_bytes(x: u64) -> u64 {
    x.wrapping_sub(LO) & !x & HI
}

/// The first index of `a` or `b` in `hay` at or after `from`.
#[inline]
pub fn find2(hay: &[u8], from: usize, a: u8, b: u8) -> Option<usize> {
    let mut i = from;
    let (va, vb) = (LO * u64::from(a), LO * u64::from(b));
    while i + 8 <= hay.len() {
        let mut w = [0u8; 8];
        w.copy_from_slice(&hay[i..i + 8]);
        let x = u64::from_le_bytes(w);
        let m = zero_bytes(x ^ va) | zero_bytes(x ^ vb);
        if m != 0 {
            return Some(i + (m.trailing_zeros() / 8) as usize);
        }
        i += 8;
    }
    hay[i.min(hay.len())..]
        .iter()
        .position(|&c| c == a || c == b)
        .map(|p| i + p)
}

/// The first index of `a` in `hay` at or after `from`.
#[inline]
pub fn find(hay: &[u8], from: usize, a: u8) -> Option<usize> {
    find2(hay, from, a, a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_like_position() {
        let hay: Vec<u8> = (0..300u32).map(|i| (i * 37 % 251) as u8).collect();
        for from in [0, 1, 7, 8, 9, 100, 299, 300] {
            for (a, b) in [(10u8, 10u8), (0, 200), (250, 3), (255, 255)] {
                let want = hay
                    .get(from..)
                    .and_then(|h| h.iter().position(|&c| c == a || c == b))
                    .map(|p| p + from);
                assert_eq!(find2(&hay, from, a, b), want, "{from} {a} {b}");
            }
        }
        assert_eq!(find(b"\x80\x81\n", 0, b'\n'), Some(2));
    }
}
