//! IMAGE_HEADER, IMAGE_PLANE_HEADER and quantiser syntax (T.832 8.3, 8.4, 8.5.6).

use crate::Error;
use crate::bits::Bits;

/// INTERNAL_CLR_FMT.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClrFmt {
    YOnly,
    Yuv420,
    Yuv422,
    Yuv444,
    Yuvk,
    NComp,
}

/// OUTPUT_CLR_FMT.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutFmt {
    YOnly,
    Yuv420,
    Yuv422,
    Yuv444,
    Cmyk,
    CmykDirect,
    NComp,
    Rgb,
    Rgbe,
}

/// OUTPUT_BITDEPTH.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BitDepth {
    Bd1White1,
    Bd8,
    Bd16,
    Bd16S,
    Bd16F,
    Bd32S,
    Bd32F,
    Bd5,
    Bd10,
    Bd565,
    Bd1Black1,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bands {
    All,
    NoFlex,
    NoHp,
    DcOnly,
}

impl Bands {
    /// Bands carried per tile in a frequency-mode codestream.
    pub fn count(self) -> usize {
        match self {
            Bands::All => 4,
            Bands::NoFlex => 3,
            Bands::NoHp => 2,
            Bands::DcOnly => 1,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ImageHeader {
    pub hard_tiling: bool,
    pub frequency: bool,
    pub index_table: bool,
    pub overlap: u8,
    pub trim_flex: bool,
    pub rb_not_swapped: bool,
    pub premultiplied: bool,
    pub alpha_plane: bool,
    pub out_fmt: OutFmt,
    pub bit_depth: BitDepth,
    /// Output (cropped) size.
    pub width: u32,
    pub height: u32,
    /// Tile column / row boundaries in macroblocks, starting with 0 and ending with the MB count.
    pub tile_cols: Vec<u32>,
    pub tile_rows: Vec<u32>,
    pub top: u32,
    pub left: u32,
    pub bottom: u32,
    pub right: u32,
}

impl ImageHeader {
    pub fn ext_width(&self) -> u32 {
        self.width + self.left + self.right
    }

    pub fn ext_height(&self) -> u32 {
        self.height + self.top + self.bottom
    }

    pub fn mb_width(&self) -> u32 {
        self.ext_width() / 16
    }

    pub fn mb_height(&self) -> u32 {
        self.ext_height() / 16
    }

    pub fn num_tiles(&self) -> usize {
        (self.tile_cols.len() - 1) * (self.tile_rows.len() - 1)
    }
}

/// Upper bound on either dimension; keeps every derived size well inside `usize`/`i32` maths.
pub const MAX_DIM: u32 = 1 << 20;

pub(crate) fn image_header(b: &mut Bits) -> Result<ImageHeader, Error> {
    if b.u(32) != 0x574D_5048 || b.u(32) != 0x4F54_4F00 {
        return Err(Error::NotJxr);
    }
    let reserved_b = b.u(4);
    if reserved_b > 1 {
        return Err(Error::Unsupported(format!(
            "codestream version {reserved_b}"
        )));
    }
    let hard_tiling = b.bit();
    b.u(3);
    let tiling = b.bit();
    let frequency = b.bit();
    b.u(3);
    let index_table = b.bit();
    let overlap = b.u(2) as u8;
    let short = b.bit();
    b.bit(); // LONG_WORD_FLAG: only an encoder hint for VLW_ESC sizes.
    let windowing = b.bit();
    let trim_flex = b.bit();
    b.u(1);
    let rb_not_swapped = b.bit();
    let premultiplied = b.bit();
    let alpha_plane = b.bit();
    let out_fmt = match b.u(4) {
        0 => OutFmt::YOnly,
        1 => OutFmt::Yuv420,
        2 => OutFmt::Yuv422,
        3 => OutFmt::Yuv444,
        4 => OutFmt::Cmyk,
        5 => OutFmt::CmykDirect,
        6 => OutFmt::NComp,
        7 => OutFmt::Rgb,
        8 => OutFmt::Rgbe,
        v => return Err(Error::Malformed(format!("output colour format {v}"))),
    };
    let bit_depth = match b.u(4) {
        0 => BitDepth::Bd1White1,
        1 => BitDepth::Bd8,
        2 => BitDepth::Bd16,
        3 => BitDepth::Bd16S,
        4 => BitDepth::Bd16F,
        6 => BitDepth::Bd32S,
        7 => BitDepth::Bd32F,
        8 => BitDepth::Bd5,
        9 => BitDepth::Bd10,
        10 => BitDepth::Bd565,
        15 => BitDepth::Bd1Black1,
        v => return Err(Error::Malformed(format!("output bit depth {v}"))),
    };
    if overlap == 3 {
        return Err(Error::Malformed("overlap mode 3".into()));
    }
    let wbits = if short { 16 } else { 32 };
    let w1 = b.u(wbits) as u64;
    let h1 = b.u(wbits) as u64;
    if w1 >= MAX_DIM as u64 || h1 >= MAX_DIM as u64 {
        return Err(Error::TooLarge);
    }
    let (width, height) = (w1 as u32 + 1, h1 as u32 + 1);
    let (ncols, nrows) = if tiling {
        (b.u(12) + 1, b.u(12) + 1)
    } else {
        (1, 1)
    };
    let tbits = if short { 8 } else { 16 };
    let mut tw = Vec::with_capacity(ncols as usize);
    for _ in 1..ncols {
        tw.push(b.u(tbits));
    }
    let mut th = Vec::with_capacity(nrows as usize);
    for _ in 1..nrows {
        th.push(b.u(tbits));
    }
    let (top, left, bottom, right) = if windowing {
        (b.u(6), b.u(6), b.u(6), b.u(6))
    } else {
        (0, 0, (16 - height % 16) % 16, (16 - width % 16) % 16)
    };
    let mut h = ImageHeader {
        hard_tiling,
        frequency,
        index_table,
        overlap,
        trim_flex,
        rb_not_swapped,
        premultiplied,
        alpha_plane,
        out_fmt,
        bit_depth,
        width,
        height,
        tile_cols: Vec::new(),
        tile_rows: Vec::new(),
        top,
        left,
        bottom,
        right,
    };
    if !h.ext_width().is_multiple_of(16) || !h.ext_height().is_multiple_of(16) {
        return Err(Error::Malformed(
            "extended size is not a multiple of 16".into(),
        ));
    }
    h.tile_cols = boundaries(&tw, h.mb_width())?;
    h.tile_rows = boundaries(&th, h.mb_height())?;
    if b.overrun() {
        return Err(Error::Truncated("image header".into()));
    }
    Ok(h)
}

fn boundaries(sizes: &[u32], total: u32) -> Result<Vec<u32>, Error> {
    let mut v = Vec::with_capacity(sizes.len() + 2);
    v.push(0);
    let mut at = 0u32;
    for &s in sizes {
        at = at.saturating_add(s);
        if s == 0 || at >= total {
            return Err(Error::Malformed("tile layout".into()));
        }
        v.push(at);
    }
    v.push(total);
    Ok(v)
}

/// Quantiser parameters of one band: `qp[component][index]`.
#[derive(Clone, Debug, Default)]
pub struct Qp {
    pub qp: Vec<Vec<u8>>,
}

impl Qp {
    pub fn count(&self) -> usize {
        self.qp.first().map_or(1, |q| q.len())
    }
}

/// Reads `n` sets (DC_QP, LP_QP or HP_QP) for `nc` components.
pub(crate) fn read_qp(b: &mut Bits, nc: usize, n: usize) -> Qp {
    let mode = if nc != 1 { b.u(2) } else { 0 };
    let mut qp = vec![vec![0u8; n]; nc];
    for j in 0..n {
        match mode {
            1 => {
                let l = b.u(8) as u8;
                let c = b.u(8) as u8;
                qp[0][j] = l;
                for q in &mut qp[1..] {
                    q[j] = c;
                }
            }
            2 => {
                for q in &mut qp {
                    q[j] = b.u(8) as u8;
                }
            }
            _ => {
                let v = b.u(8) as u8;
                for q in &mut qp {
                    q[j] = v;
                }
            }
        }
    }
    Qp { qp }
}

#[derive(Clone, Debug)]
pub struct PlaneHeader {
    pub clr: ClrFmt,
    pub scaled: bool,
    pub bands: Bands,
    pub nc: usize,
    pub shift: u8,
    pub len_mantissa: u8,
    pub exp_bias: i8,
    /// Image-plane level quantisers, `None` when they are given per tile.
    pub dc: Option<Qp>,
    pub lp: Option<Qp>,
    pub hp: Option<Qp>,
}

pub(crate) fn plane_header(
    b: &mut Bits,
    ih: &ImageHeader,
    alpha: bool,
) -> Result<PlaneHeader, Error> {
    let clr = match b.u(3) {
        0 => ClrFmt::YOnly,
        1 => ClrFmt::Yuv420,
        2 => ClrFmt::Yuv422,
        3 => ClrFmt::Yuv444,
        4 => ClrFmt::Yuvk,
        6 => ClrFmt::NComp,
        v => return Err(Error::Malformed(format!("internal colour format {v}"))),
    };
    if alpha && clr != ClrFmt::YOnly {
        return Err(Error::Malformed(
            "alpha plane is not single-component".into(),
        ));
    }
    let scaled = b.bit();
    let bands = match b.u(4) {
        0 => Bands::All,
        1 => Bands::NoFlex,
        2 => Bands::NoHp,
        3 => Bands::DcOnly,
        v => return Err(Error::Malformed(format!("bands present {v}"))),
    };
    let mut nc = 1;
    match clr {
        ClrFmt::Yuv420 | ClrFmt::Yuv422 | ClrFmt::Yuv444 => {
            nc = 3;
            // CHROMA_CENTERING_X/Y: chroma is upsampled as the reference software does,
            // which ignores the siting.
            b.u(8);
        }
        ClrFmt::Yuvk => nc = 4,
        ClrFmt::NComp => {
            let n = b.u(4);
            nc = if n == 0xF {
                b.u(12) as usize + 16
            } else {
                b.u(4);
                n as usize + 1
            };
            if nc > 16 {
                return Err(Error::Unsupported(format!("{nc} components")));
            }
        }
        ClrFmt::YOnly => {}
    }
    let mut shift = 0;
    let (mut len_mantissa, mut exp_bias) = (0, 0);
    match ih.bit_depth {
        BitDepth::Bd16 | BitDepth::Bd16S | BitDepth::Bd32S => shift = b.u(8) as u8,
        BitDepth::Bd32F => {
            len_mantissa = b.u(8) as u8;
            exp_bias = b.u(8) as u8 as i8;
            if len_mantissa == 0 || len_mantissa > 23 {
                return Err(Error::Malformed(format!("mantissa length {len_mantissa}")));
            }
        }
        _ => {}
    }
    if shift > 31 {
        return Err(Error::Malformed(format!("shift {shift}")));
    }
    let dc = b.bit().then(|| read_qp(b, nc, 1));
    let (mut lp, mut hp) = (None, None);
    if bands != Bands::DcOnly {
        b.u(1);
        lp = b.bit().then(|| read_qp(b, nc, 1));
        if bands != Bands::NoHp {
            b.u(1);
            hp = b.bit().then(|| read_qp(b, nc, 1));
        }
    }
    b.align();
    if b.overrun() {
        return Err(Error::Truncated("image plane header".into()));
    }
    Ok(PlaneHeader {
        clr,
        scaled,
        bands,
        nc,
        shift,
        len_mantissa,
        exp_bias,
        dc,
        lp,
        hp,
    })
}

/// Quantisers in force for one tile of one plane, with the LP/HP index counts.
#[derive(Clone, Debug, Default)]
pub struct TileQp {
    pub dc: Qp,
    pub lp: Qp,
    pub hp: Qp,
    /// HP indices are the LP ones (USE_LP_QP_FLAG).
    pub hp_from_lp: bool,
}

pub(crate) fn tile_dc(b: &mut Bits, ph: &PlaneHeader, t: &mut TileQp) {
    t.dc = match &ph.dc {
        Some(q) => q.clone(),
        None => read_qp(b, ph.nc, 1),
    };
}

pub(crate) fn tile_lp(b: &mut Bits, ph: &PlaneHeader, t: &mut TileQp) {
    t.lp = match &ph.lp {
        Some(q) => q.clone(),
        None => {
            if b.bit() {
                t.dc.clone()
            } else {
                let n = b.u(4) as usize + 1;
                read_qp(b, ph.nc, n)
            }
        }
    };
}

pub(crate) fn tile_hp(b: &mut Bits, ph: &PlaneHeader, t: &mut TileQp) {
    t.hp_from_lp = false;
    t.hp = match &ph.hp {
        Some(q) => q.clone(),
        None => {
            if b.bit() {
                t.hp_from_lp = true;
                t.lp.clone()
            } else {
                let n = b.u(4) as usize + 1;
                read_qp(b, ph.nc, n)
            }
        }
    };
}
