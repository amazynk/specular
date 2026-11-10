//! Minolta RD-175 (`.mdc`): three 768 x 494 8-bit CCDs behind a beam splitter, two green
//! (offset diagonally by half a photosite) and one with red and blue in alternate columns.
//!
//! Layout, from our own analysis of a sample file: a 512-byte header, then 1482 rows of 768
//! bytes. The rows of each CCD are written in six interleaved passes: row `k < 492` of the
//! first green CCD is file row `(k % 6) * 164 + k / 6`, of the red/blue CCD that plus 82, of
//! the second green CCD `984 + (k % 6) * 82 + k / 6`; rows 492 and 493 of the three follow at
//! the end (second green, red/blue, first green). Column 0 holds no picture.
//!
//! The decode builds a 1534 x 986 GRBG mosaic on the doubled grid the two green CCDs span
//! (values doubled to 9 bits): green from the CCDs as they are, red and blue placed at their
//! own positions on that grid (red with the second green CCD's rows, blue with the first's)
//! and linearly interpolated along the row where a mosaic site falls between two of them.

/// File size: header plus three CCDs.
pub(crate) const FILE_SIZE: usize = HEADER + 3 * CCD_W * CCD_H;
pub(crate) const HEADER: usize = 512;
const CCD_W: usize = 768;
const CCD_H: usize = 494;
/// Output mosaic size.
pub(crate) const WIDTH: usize = 2 * (CCD_W - 1);
pub(crate) const HEIGHT: usize = 2 * (CCD_H - 1);

#[derive(Clone, Copy)]
enum Ccd {
    /// The first green CCD (odd mosaic rows).
    G1,
    /// Red and blue.
    Rb,
    /// The second green CCD (even mosaic rows).
    G2,
}

/// File row (after the header) of CCD row `k`.
fn file_row(ccd: Ccd, k: usize) -> usize {
    if k >= 492 {
        let base = match ccd {
            Ccd::G2 => 1476,
            Ccd::Rb => 1478,
            Ccd::G1 => 1480,
        };
        return base + (k - 492);
    }
    match ccd {
        Ccd::G1 => (k % 6) * 164 + k / 6,
        Ccd::Rb => (k % 6) * 164 + 82 + k / 6,
        Ccd::G2 => 984 + (k % 6) * 82 + k / 6,
    }
}

/// Decodes `src` (the file after the header) into `out` (`WIDTH x HEIGHT`). Returns true when
/// the data ran short (missing rows stay zero).
pub(crate) fn decode(src: &[u8], out: &mut [u16]) -> bool {
    if out.len() < WIDTH * HEIGHT {
        return true;
    }
    let row = |ccd: Ccd, k: usize| -> Option<&[u8]> {
        let r = file_row(ccd, k);
        src.get(r * CCD_W..(r + 1) * CCD_W)
    };
    let mut short = false;
    for i in 0..CCD_H - 1 {
        let (even, odd) = out[2 * i * WIDTH..(2 * i + 2) * WIDTH].split_at_mut(WIDTH);
        // Even mosaic row: G R G R ... (green from the second green CCD, red from the
        // red/blue CCD's even columns: its column 2m sits at mosaic column 4m - 1).
        match (row(Ccd::G2, i), row(Ccd::Rb, i)) {
            (Some(g), Some(rb)) => {
                for j in 0..CCD_W - 1 {
                    even[2 * j] = 2 * g[j + 1] as u16;
                    // Red site at mosaic column 2j + 1.
                    let x = 2 * j + 1;
                    even[x] = if x % 4 == 3 {
                        2 * rb[x.div_ceil(2)] as u16
                    } else {
                        // Between the red columns at mosaic x - 2 and x + 2; the edges
                        // repeat the nearest one.
                        let (lo, hi) = ((x - 1) / 2, (x + 3) / 2);
                        if lo < 2 {
                            2 * rb[2] as u16
                        } else if hi > CCD_W - 2 {
                            2 * rb[CCD_W - 2] as u16
                        } else {
                            rb[lo] as u16 + rb[hi] as u16
                        }
                    };
                }
            }
            _ => short = true,
        }
        // Odd mosaic row: B G B G ... (blue: column 2m + 1 sits at mosaic column 4m).
        match (row(Ccd::G1, i), row(Ccd::Rb, i)) {
            (Some(g), Some(rb)) => {
                for j in 0..CCD_W - 1 {
                    odd[2 * j + 1] = 2 * g[j + 1] as u16;
                    let x = 2 * j;
                    odd[x] = if x % 4 == 0 {
                        2 * rb[x / 2 + 1] as u16
                    } else {
                        let lo = x / 2; // blue columns x / 2 and x / 2 + 2 (odd)
                        let hi = (x / 2 + 2).min(CCD_W - 1);
                        rb[lo] as u16 + rb[hi] as u16
                    };
                }
            }
            _ => short = true,
        }
    }
    short
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_cover_the_file_once() {
        let mut seen = vec![false; 3 * CCD_H];
        for ccd in [Ccd::G1, Ccd::Rb, Ccd::G2] {
            for k in 0..CCD_H {
                let r = file_row(ccd, k);
                assert!(!seen[r], "row {r} twice");
                seen[r] = true;
            }
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn flat_field_stays_flat() {
        let src = vec![100u8; FILE_SIZE - HEADER];
        let mut out = vec![0u16; WIDTH * HEIGHT];
        assert!(!decode(&src, &mut out));
        assert!(out.iter().all(|&v| v == 200));
        assert!(decode(&src[..1000], &mut out));
    }
}
