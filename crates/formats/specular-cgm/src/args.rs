//! What the interpreter reads parameters through, so the binary and clear-text encodings share
//! one interpreter.

pub(crate) use crate::binary::CellColour;

/// A real precision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RealPrec {
    Float32,
    Float64,
    Fixed32,
    Fixed64,
}

impl RealPrec {
    /// From REAL PRECISION's (form, exponent or whole width, fraction width).
    pub fn from_parts(form: i64, a: i64, b: i64) -> Option<RealPrec> {
        match (form, a, b) {
            (0, 9, 23) => Some(RealPrec::Float32),
            (0, 12, 52) => Some(RealPrec::Float64),
            (1, 16, 16) => Some(RealPrec::Fixed32),
            (1, 32, 32) => Some(RealPrec::Fixed64),
            // Loosely written files: pick by the total width.
            (0, _, _) if a + b < 32 => Some(RealPrec::Float32),
            (0, _, _) => Some(RealPrec::Float64),
            (1, _, _) if a + b <= 32 => Some(RealPrec::Fixed32),
            (1, _, _) => Some(RealPrec::Fixed64),
            _ => None,
        }
    }
}

/// The precisions in force (metafile descriptor, control elements).
#[derive(Clone, Debug)]
pub(crate) struct Prec {
    /// Bits of an integer (I).
    pub int: u8,
    pub real: RealPrec,
    /// Bits of an index (IX).
    pub index: u8,
    /// Bits of a direct colour component (CD).
    pub colour: u8,
    /// Bits of a colour index (CI).
    pub colour_index: u8,
    /// VDC are reals (else integers).
    pub vdc_real: bool,
    pub vdc_int: u8,
    pub vdc_real_prec: RealPrec,
    /// Components of a direct colour: 3 (RGB, CIE) or 4 (CMYK).
    pub components: u8,
}

impl Default for Prec {
    fn default() -> Self {
        Prec {
            int: 16,
            real: RealPrec::Fixed32,
            index: 16,
            colour: 8,
            colour_index: 8,
            vdc_real: false,
            vdc_int: 16,
            vdc_real_prec: RealPrec::Fixed32,
            components: 3,
        }
    }
}

/// Parameter access of one element.
pub(crate) trait Args {
    /// Whether parameters remain.
    fn more(&self) -> bool;
    fn int(&mut self) -> Option<i64>;
    fn index(&mut self) -> Option<i64>;
    /// An enumerated value; `names` are the clear-text keywords for 0, 1, 2…
    fn enumv(&mut self, names: &[&str]) -> Option<i64>;
    fn real(&mut self) -> Option<f64>;
    fn vdc(&mut self) -> Option<f64>;
    fn point(&mut self) -> Option<(f64, f64)> {
        Some((self.vdc()?, self.vdc()?))
    }
    fn ci(&mut self) -> Option<u32>;
    /// A direct colour, components as stored (scaled by the colour value extent later).
    fn cd(&mut self) -> Option<[f64; 4]>;
    fn string(&mut self) -> Option<Vec<u8>>;
    /// A cell array's representation mode: 0 run-length, 1 packed.
    fn cell_mode(&mut self) -> i64;
    /// The `nx`×`ny` cells of a cell array or pattern table, row by row.
    fn cells(
        &mut self,
        nx: usize,
        ny: usize,
        local_prec: i64,
        mode: i64,
        indexed: bool,
        out: &mut dyn FnMut(CellColour),
    ) -> Option<()>;
}
