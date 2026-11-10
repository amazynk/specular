//! Text region decoding (6.4): symbol instances placed in strips, optionally refined.

use std::borrow::Cow;
use std::rc::Rc;

use crate::bitmap::{Bitmap, Budget, Op};
use crate::generic::{self, Refine};
use crate::huff::{Bits, Table};
use crate::mq::{IdCx, IntCx, Mq};
use crate::{Error, Result};

/// The arithmetic contexts of text region decoding.
pub(crate) struct TextCx {
    pub dt: IntCx,
    pub fs: IntCx,
    pub ds: IntCx,
    pub it: IntCx,
    pub ri: IntCx,
    pub rdw: IntCx,
    pub rdh: IntCx,
    pub rdx: IntCx,
    pub rdy: IntCx,
    pub id: IdCx,
    /// Refinement contexts.
    pub gr: Vec<u8>,
}

impl TextCx {
    pub(crate) fn new(symcodelen: u32, gr: Vec<u8>) -> TextCx {
        TextCx {
            dt: IntCx::new(),
            fs: IntCx::new(),
            ds: IntCx::new(),
            it: IntCx::new(),
            ri: IntCx::new(),
            rdw: IntCx::new(),
            rdh: IntCx::new(),
            rdx: IntCx::new(),
            rdy: IntCx::new(),
            id: IdCx::new(symcodelen),
            gr,
        }
    }
}

/// The Huffman tables of a text region.
pub(crate) struct HuffText {
    pub fs: Table,
    pub ds: Table,
    pub dt: Table,
    pub rdw: Table,
    pub rdh: Table,
    pub rdx: Table,
    pub rdy: Table,
    pub rsize: Table,
    /// Symbol ID codes; `None`: fixed-length codes of `symcodelen` bits.
    pub ids: Option<Table>,
    pub symcodelen: u32,
}

/// Where the text region's numbers come from.
pub(crate) enum Coder<'a, 'b> {
    Arith {
        mq: &'b mut Mq<'a>,
        cx: &'b mut TextCx,
    },
    Huff {
        bits: &'b mut Bits<'a>,
        t: &'b HuffText,
        gr: &'b mut Vec<u8>,
    },
}

fn int(v: Option<i64>, what: &str) -> Result<i64> {
    v.ok_or_else(|| Error::Malformed(format!("unexpected out-of-band {what}")))
}

impl Coder<'_, '_> {
    fn exhausted(&self) -> bool {
        match self {
            Coder::Arith { mq, .. } => mq.exhausted(),
            Coder::Huff { bits, .. } => bits.exhausted(),
        }
    }

    fn dt(&mut self) -> Result<i64> {
        match self {
            Coder::Arith { mq, cx } => int(cx.dt.decode(mq), "strip delta"),
            Coder::Huff { bits, t, .. } => t.dt.value(bits),
        }
    }

    fn fs(&mut self) -> Result<i64> {
        match self {
            Coder::Arith { mq, cx } => int(cx.fs.decode(mq), "first S"),
            Coder::Huff { bits, t, .. } => t.fs.value(bits),
        }
    }

    fn ds(&mut self) -> Result<Option<i64>> {
        match self {
            Coder::Arith { mq, cx } => Ok(cx.ds.decode(mq)),
            Coder::Huff { bits, t, .. } => t.ds.decode(bits),
        }
    }

    fn it(&mut self, log_strips: u32) -> Result<i64> {
        match self {
            Coder::Arith { mq, cx } => int(cx.it.decode(mq), "T"),
            Coder::Huff { bits, .. } => Ok(bits.bits(log_strips) as i64),
        }
    }

    fn id(&mut self) -> Result<u32> {
        match self {
            Coder::Arith { mq, cx } => Ok(cx.id.decode(mq)),
            Coder::Huff { bits, t, .. } => match &t.ids {
                Some(tab) => Ok(tab.value(bits)? as u32),
                None => Ok(bits.bits(t.symcodelen) as u32),
            },
        }
    }

    fn ri(&mut self) -> Result<u32> {
        match self {
            Coder::Arith { mq, cx } => Ok(int(cx.ri.decode(mq), "refinement flag")? as u32),
            Coder::Huff { bits, .. } => Ok(bits.bit()),
        }
    }

    /// RDW, RDH, RDX, RDY, then the refined bitmap.
    fn refined(
        &mut self,
        sym: &Bitmap,
        template: u8,
        at: [(i8, i8); 2],
        budget: &Budget,
    ) -> Result<Bitmap> {
        let (rdw, rdh, rdx, rdy) = match self {
            Coder::Arith { mq, cx } => (
                int(cx.rdw.decode(mq), "RDW")?,
                int(cx.rdh.decode(mq), "RDH")?,
                int(cx.rdx.decode(mq), "RDX")?,
                int(cx.rdy.decode(mq), "RDY")?,
            ),
            Coder::Huff { bits, t, .. } => (
                t.rdw.value(bits)?,
                t.rdh.value(bits)?,
                t.rdx.value(bits)?,
                t.rdy.value(bits)?,
            ),
        };
        let w = sym.w as i64 + rdw;
        let h = sym.h as i64 + rdh;
        if !(0..=u32::MAX as i64).contains(&w) || !(0..=u32::MAX as i64).contains(&h) {
            return Err(Error::Malformed(
                "refined symbol has a negative size".into(),
            ));
        }
        let p = Refine {
            w: w as u32,
            h: h as u32,
            template,
            reference: sym,
            dx: (rdw >> 1) + rdx,
            dy: (rdh >> 1) + rdy,
            tpgron: false,
            at,
        };
        match self {
            Coder::Arith { mq, cx } => generic::refine(mq, &mut cx.gr, &p, budget),
            Coder::Huff { bits, t, gr } => {
                let size = t.rsize.value(bits)?;
                bits.align();
                let rest = bits.rest();
                let size = usize::try_from(size).unwrap_or(usize::MAX).min(rest.len());
                let mut mq = Mq::new(&rest[..size]);
                let bm = generic::refine(&mut mq, gr, &p, budget)?;
                bits.skip_bytes(size);
                Ok(bm)
            }
        }
    }
}

pub(crate) struct Text<'s> {
    pub w: u32,
    pub h: u32,
    pub instances: u32,
    pub log_strips: u32,
    pub symbols: &'s [Rc<Bitmap>],
    pub refine: bool,
    pub default_pixel: u8,
    pub op: Op,
    pub transposed: bool,
    /// 0 bottom-left, 1 top-left, 2 bottom-right, 3 top-right.
    pub corner: u8,
    pub ds_offset: i64,
    pub rtemplate: u8,
    pub rat: [(i8, i8); 2],
}

/// Decodes a text region; the flag tells that the data ran out early.
pub(crate) fn decode(c: &mut Coder, p: &Text, budget: &Budget) -> Result<(Bitmap, bool)> {
    let mut reg = Bitmap::filled(p.w, p.h, p.default_pixel, budget)?;
    let strips = 1i64 << p.log_strips;
    let mut strip_t = -c.dt()?.saturating_mul(strips);
    let mut first_s = 0i64;
    let mut n = 0u32;
    let mut truncated = false;
    let (top, left) = (p.corner & 1 == 1, p.corner & 2 == 0);
    'strips: while n < p.instances {
        if c.exhausted() {
            truncated = true;
            break;
        }
        strip_t = strip_t.saturating_add(c.dt()?.saturating_mul(strips));
        let mut cur_s = 0i64;
        let mut first = true;
        loop {
            if first {
                first_s = first_s.saturating_add(c.fs()?);
                cur_s = first_s;
                first = false;
            } else {
                match c.ds()? {
                    None => break,
                    Some(d) => cur_s = cur_s.saturating_add(d).saturating_add(p.ds_offset),
                }
            }
            if n >= p.instances || c.exhausted() {
                truncated |= c.exhausted();
                break 'strips;
            }
            let cur_t = if strips == 1 { 0 } else { c.it(p.log_strips)? };
            let t = strip_t.saturating_add(cur_t);
            let id = c.id()? as usize;
            let ri = if p.refine { c.ri()? } else { 0 };
            let sym = p.symbols.get(id).ok_or_else(|| {
                Error::Malformed(format!(
                    "text region uses symbol {id}, which does not exist"
                ))
            })?;
            let ib: Cow<Bitmap> = if ri == 0 {
                Cow::Borrowed(sym.as_ref())
            } else {
                Cow::Owned(c.refined(sym, p.rtemplate, p.rat, budget)?)
            };
            let (wi, hi) = (ib.w as i64, ib.h as i64);
            if !p.transposed && !left {
                cur_s += wi - 1;
            } else if p.transposed && !top {
                cur_s += hi - 1;
            }
            let s = cur_s;
            let (x, y) = if !p.transposed {
                (
                    if left { s } else { s - wi + 1 },
                    if top { t } else { t - hi + 1 },
                )
            } else {
                (
                    if left { t } else { t - wi + 1 },
                    if top { s } else { s - hi + 1 },
                )
            };
            reg.compose(&ib, x, y, p.op);
            if !p.transposed && left {
                cur_s += wi - 1;
            } else if p.transposed && top {
                cur_s += hi - 1;
            }
            n += 1;
        }
    }
    Ok((reg, truncated))
}

/// Reads the symbol ID Huffman table of a text region segment (7.4.3.1.7).
pub(crate) fn symbol_id_table(bits: &mut Bits, nsyms: usize) -> Result<Table> {
    let mut run_lens = [0u8; 35];
    for l in run_lens.iter_mut() {
        *l = bits.bits(4) as u8;
    }
    let runs = Table::from_lengths(&run_lens)?;
    let mut lens: Vec<u8> = Vec::with_capacity(nsyms);
    while lens.len() < nsyms {
        if bits.exhausted() {
            return Err(Error::Truncated("symbol ID table".into()));
        }
        let code = runs.value(bits)?;
        match code {
            0..=31 => lens.push(code as u8),
            32 => {
                let prev = *lens
                    .last()
                    .ok_or_else(|| Error::Malformed("symbol ID table repeats nothing".into()))?;
                let n = 3 + bits.bits(2) as usize;
                lens.extend(std::iter::repeat_n(prev, n));
            }
            33 => {
                let n = 3 + bits.bits(3) as usize;
                lens.extend(std::iter::repeat_n(0, n));
            }
            _ => {
                let n = 11 + bits.bits(7) as usize;
                lens.extend(std::iter::repeat_n(0, n));
            }
        }
    }
    lens.truncate(nsyms);
    bits.align();
    Table::from_lengths(&lens)
}
