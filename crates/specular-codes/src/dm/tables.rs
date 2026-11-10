//! ECC 200 symbol sizes (ISO/IEC 16022, table 7): 24 square and 6 rectangular.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    /// Modules, finder and timing included.
    pub rows: usize,
    pub cols: usize,
    /// One data region's modules.
    pub region_rows: usize,
    pub region_cols: usize,
    pub data: usize,
    pub ecc: usize,
    /// Interleaved Reed–Solomon blocks.
    pub blocks: usize,
}

const fn s(
    rows: usize,
    cols: usize,
    region_rows: usize,
    region_cols: usize,
    data: usize,
    ecc: usize,
    blocks: usize,
) -> Size {
    Size {
        rows,
        cols,
        region_rows,
        region_cols,
        data,
        ecc,
        blocks,
    }
}

pub const SIZES: [Size; 30] = [
    s(10, 10, 8, 8, 3, 5, 1),
    s(12, 12, 10, 10, 5, 7, 1),
    s(14, 14, 12, 12, 8, 10, 1),
    s(16, 16, 14, 14, 12, 12, 1),
    s(18, 18, 16, 16, 18, 14, 1),
    s(20, 20, 18, 18, 22, 18, 1),
    s(22, 22, 20, 20, 30, 20, 1),
    s(24, 24, 22, 22, 36, 24, 1),
    s(26, 26, 24, 24, 44, 28, 1),
    s(32, 32, 14, 14, 62, 36, 1),
    s(36, 36, 16, 16, 86, 42, 1),
    s(40, 40, 18, 18, 114, 48, 1),
    s(44, 44, 20, 20, 144, 56, 1),
    s(48, 48, 22, 22, 174, 68, 1),
    s(52, 52, 24, 24, 204, 84, 2),
    s(64, 64, 14, 14, 280, 112, 2),
    s(72, 72, 16, 16, 368, 144, 4),
    s(80, 80, 18, 18, 456, 192, 4),
    s(88, 88, 20, 20, 576, 224, 4),
    s(96, 96, 22, 22, 696, 272, 4),
    s(104, 104, 24, 24, 816, 336, 6),
    s(120, 120, 18, 18, 1050, 408, 6),
    s(132, 132, 20, 20, 1304, 496, 8),
    s(144, 144, 22, 22, 1558, 620, 10),
    s(8, 18, 6, 16, 5, 7, 1),
    s(8, 32, 6, 14, 10, 11, 1),
    s(12, 26, 10, 24, 16, 14, 1),
    s(12, 36, 10, 16, 22, 18, 1),
    s(16, 36, 14, 16, 32, 24, 1),
    s(16, 48, 14, 22, 49, 28, 1),
];

impl Size {
    /// The mapping matrix (data regions side by side, without their borders).
    pub fn mapping(&self) -> (usize, usize) {
        let vr = self.rows / (self.region_rows + 2);
        let hr = self.cols / (self.region_cols + 2);
        (vr * self.region_rows, hr * self.region_cols)
    }

    /// What a finder or timing module must be, `None` for data.
    pub fn fixed(&self, r: usize, c: usize) -> Option<bool> {
        let (rh, rw) = (self.region_rows + 2, self.region_cols + 2);
        let (rr, cc) = (r % rh, c % rw);
        if cc == 0 || rr == rh - 1 {
            Some(true)
        } else if rr == 0 {
            Some(cc % 2 == 0)
        } else if cc == rw - 1 {
            Some(rr % 2 == 1)
        } else {
            None
        }
    }

    /// The symbol module of mapping-matrix module (`r`, `c`).
    pub fn module_of(&self, r: usize, c: usize) -> (usize, usize) {
        (
            1 + r + 2 * (r / self.region_rows),
            1 + c + 2 * (c / self.region_cols),
        )
    }

    pub fn name(&self) -> String {
        format!("{}×{}", self.rows, self.cols)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regions_tile_the_symbol() {
        for s in SIZES {
            let (mr, mc) = s.mapping();
            let (vr, hr) = (mr / s.region_rows, mc / s.region_cols);
            assert_eq!(vr * (s.region_rows + 2), s.rows, "{s:?}");
            assert_eq!(hr * (s.region_cols + 2), s.cols, "{s:?}");
            // Every codeword's 8 modules fit, give or take the 4 spare ones.
            let modules = mr * mc;
            let cw = s.data + s.ecc;
            assert!(modules >= cw * 8 && modules - cw * 8 <= 4, "{s:?}");
        }
    }
}
