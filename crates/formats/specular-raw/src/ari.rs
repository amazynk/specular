//! ARRI ARI (ARRIRAW) frames: 12-bit samples packed two in three bytes, low bits first
//! (`b0 | (b1 & 15) << 8`, `b1 >> 4 | b2 << 4`).

use rayon::prelude::*;

/// Decodes `w x h` samples (`w` even) from `src` into `out`; returns true if data ran out
/// (the missing samples are zero).
pub(crate) fn decode(src: &[u8], w: usize, h: usize, out: &mut [u16], parallel: bool) -> bool {
    let row_bytes = w * 3 / 2;
    let out = &mut out[..w * h];
    let row = |(y, dst): (usize, &mut [u16])| -> bool {
        let s = src.get(y * row_bytes..).unwrap_or(&[]);
        let s = &s[..s.len().min(row_bytes)];
        for (p, b) in dst.chunks_exact_mut(2).zip(s.chunks_exact(3)) {
            p[0] = b[0] as u16 | (b[1] as u16 & 15) << 8;
            p[1] = (b[1] >> 4) as u16 | (b[2] as u16) << 4;
        }
        let done = s.len() / 3 * 2;
        dst[done.min(w)..].fill(0);
        s.len() < row_bytes
    };
    if parallel {
        out.par_chunks_exact_mut(w)
            .enumerate()
            .map(row)
            .reduce(|| false, |a, b| a | b)
    } else {
        out.chunks_exact_mut(w)
            .enumerate()
            .map(row)
            .fold(false, |a, b| a | b)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Packs samples two in three bytes, low bits first.
    pub(crate) fn pack(v: &[u16]) -> Vec<u8> {
        v.chunks(2)
            .flat_map(|p| {
                let (a, b) = (p[0], p.get(1).copied().unwrap_or(0));
                [
                    a as u8,
                    ((a >> 8) & 15) as u8 | ((b & 15) << 4) as u8,
                    (b >> 4) as u8,
                ]
            })
            .collect()
    }

    #[test]
    fn unpack() {
        let (w, h) = (6usize, 3usize);
        let want: Vec<u16> = (0..w * h).map(|i| (i * 937 % 4096) as u16).collect();
        let src = pack(&want);
        for parallel in [false, true] {
            let mut out = vec![0u16; w * h];
            assert!(!decode(&src, w, h, &mut out, parallel));
            assert_eq!(out, want);
        }
        let mut out = vec![1u16; w * h];
        assert!(decode(&src[..12], w, h, &mut out, false));
        assert_eq!(&out[6..], &[want[6], want[7], 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }
}
