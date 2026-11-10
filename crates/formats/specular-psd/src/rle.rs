//! PackBits (Apple / TIFF / PSD RLE).

/// Unpack `src` into `dst`; returns the number of bytes written (`dst.len()` when complete).
/// Extra input is ignored; malformed input never panics.
#[inline]
pub(crate) fn unpack(src: &[u8], dst: &mut [u8]) -> usize {
    let n = dst.len();
    let (mut i, mut o) = (0usize, 0usize);
    while o < n && i < src.len() {
        let h = src[i] as i8;
        i += 1;
        if h >= 0 {
            let cnt = h as usize + 1;
            let take = cnt.min(n - o).min(src.len() - i);
            dst[o..o + take].copy_from_slice(&src[i..i + take]);
            o += take;
            i += cnt;
        } else if h != -128 {
            let cnt = (1 - h as isize) as usize;
            let Some(&v) = src.get(i) else { break };
            i += 1;
            let take = cnt.min(n - o);
            dst[o..o + take].fill(v);
            o += take;
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::unpack;

    #[test]
    fn tiff_example() {
        let src = [
            0xFE, 0xAA, 0x02, 0x80, 0x00, 0x2A, 0xFD, 0xAA, 0x03, 0x80, 0x00, 0x2A, 0x22, 0xF7,
            0xAA,
        ];
        let want = [
            0xAA, 0xAA, 0xAA, 0x80, 0x00, 0x2A, 0xAA, 0xAA, 0xAA, 0xAA, 0x80, 0x00, 0x2A, 0x22,
            0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA,
        ];
        let mut out = [0u8; 24];
        assert_eq!(unpack(&src, &mut out), 24);
        assert_eq!(out, want);
    }

    #[test]
    fn short_and_overlong() {
        let mut out = [0u8; 4];
        assert_eq!(unpack(&[0x05, 1, 2], &mut out), 2);
        assert_eq!(unpack(&[0x80, 0xF0, 7], &mut out), 4);
        assert_eq!(out, [7; 4]);
        assert_eq!(unpack(&[0xFF], &mut out), 0);
    }
}
