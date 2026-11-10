//! Symbol dictionary segments (7.4.2, 6.5).

use std::rc::Rc;

use crate::bitmap::{Bitmap, Budget, Op};
use crate::generic::{self, Generic, Refine};
use crate::huff::{Bits, Table};
use crate::mq::{IntCx, Mq, code_len};
use crate::text::{self, Coder, HuffText, Text, TextCx};
use crate::{Error, Result, be16, be32, mmr};

/// A decoded symbol dictionary.
pub(crate) struct Dict {
    pub exported: Vec<Rc<Bitmap>>,
    /// Generic and refinement contexts, kept when the segment asks to retain them.
    pub gb: Option<Vec<u8>>,
    pub gr: Option<Vec<u8>>,
}

fn at_pair(d: &[u8], o: usize) -> (i8, i8) {
    (d[o] as i8, d[o + 1] as i8)
}

fn at_wide(d: &[u8], o: usize) -> (i32, i32) {
    (d[o] as i8 as i32, d[o + 1] as i8 as i32)
}

/// Decodes a symbol dictionary segment. `inputs`: the symbols exported by the dictionaries
/// it refers to; `tables`: the referred table segments, in order; `reuse`: the dictionary
/// whose retained contexts are used, when the segment asks for that.
pub(crate) fn decode(
    data: &[u8],
    inputs: Vec<Rc<Bitmap>>,
    tables: &mut dyn Iterator<Item = &Table>,
    reuse: Option<&Dict>,
    budget: &Budget,
) -> Result<Dict> {
    let short = || Error::Truncated("symbol dictionary header".into());
    let flags = be16(data, 0).ok_or_else(short)?;
    let huff = flags & 1 != 0;
    let refagg = flags & 2 != 0;
    let sel_dh = (flags >> 2) & 3;
    let sel_dw = (flags >> 4) & 3;
    let sel_bmsize = (flags >> 6) & 1;
    let sel_agginst = (flags >> 7) & 1;
    let ctx_used = flags & 0x100 != 0;
    let ctx_retained = flags & 0x200 != 0;
    let template = ((flags >> 10) & 3) as u8;
    let rtemplate = ((flags >> 12) & 1) as u8;
    let mut o = 2;
    let mut at = generic::default_at(template);
    if !huff {
        let n = if template == 0 { 4 } else { 1 };
        if data.len() < o + 2 * n {
            return Err(short());
        }
        for (i, a) in at.iter_mut().take(n).enumerate() {
            *a = at_wide(data, o + 2 * i);
        }
        o += 2 * n;
    }
    let mut rat = [(-1, -1), (-1, -1)];
    if refagg && rtemplate == 0 {
        if data.len() < o + 4 {
            return Err(short());
        }
        rat = [at_pair(data, o), at_pair(data, o + 2)];
        o += 4;
    }
    let num_ex = be32(data, o).ok_or_else(short)? as u64;
    let num_new = be32(data, o + 4).ok_or_else(short)? as u64;
    o += 8;
    let body = &data[o.min(data.len())..];
    let num_in = inputs.len() as u64;
    let total = num_in + num_new;
    if total > 1 << 24 || num_ex > total {
        return Err(Error::Malformed(
            "symbol dictionary symbol counts are not plausible".into(),
        ));
    }
    let symcodelen = code_len(total);
    if symcodelen > 24 {
        return Err(Error::TooLarge);
    }

    let mut custom = |sel: bool, std: usize| -> Result<Table> {
        if sel {
            tables.next().cloned().ok_or_else(|| {
                Error::Malformed("symbol dictionary refers to a missing Huffman table".into())
            })
        } else {
            Ok(Table::standard(std))
        }
    };
    // Huffman tables (only read in Huffman mode, but the custom ones in their fixed order).
    let (dh_t, dw_t, bmsize_t, agginst_t) = if huff {
        if sel_dh == 2 || sel_dw == 2 {
            return Err(Error::Malformed(
                "symbol dictionary selects a reserved Huffman table".into(),
            ));
        }
        let dh = custom(sel_dh == 3, if sel_dh == 1 { 5 } else { 4 })?;
        let dw = custom(sel_dw == 3, if sel_dw == 1 { 3 } else { 2 })?;
        let bm = custom(sel_bmsize == 1, 1)?;
        let ag = custom(sel_agginst == 1, 1)?;
        (Some(dh), Some(dw), Some(bm), Some(ag))
    } else {
        (None, None, None, None)
    };

    let gb_init = reuse.filter(|_| ctx_used).and_then(|d| d.gb.clone());
    let gr_init = reuse.filter(|_| ctx_used).and_then(|d| d.gr.clone());
    let mut gb = gb_init.unwrap_or_else(|| vec![0; generic::contexts(template)]);
    let gr = gr_init.unwrap_or_else(|| vec![0; generic::refine_contexts(rtemplate)]);
    if gb.len() != generic::contexts(template) {
        gb = vec![0; generic::contexts(template)];
    }

    let mut mq = Mq::new(body);
    let mut bits = Bits::new(body);
    let (mut iadh, mut iadw, mut iaex, mut iaai) =
        (IntCx::new(), IntCx::new(), IntCx::new(), IntCx::new());
    let mut tcx = TextCx::new(symcodelen, gr);
    let huff_text = HuffText {
        fs: Table::standard(6),
        ds: Table::standard(8),
        dt: Table::standard(11),
        rdw: Table::standard(15),
        rdh: Table::standard(15),
        rdx: Table::standard(15),
        rdy: Table::standard(15),
        rsize: Table::standard(1),
        ids: None,
        symcodelen,
    };

    let mut all: Vec<Rc<Bitmap>> = inputs;
    let mut widths: Vec<u32> = Vec::new();
    let mut nsyms = 0u64;
    let mut height = 0i64;
    while nsyms < num_new {
        if mq.exhausted() || bits.exhausted() {
            return Err(Error::Truncated("symbol dictionary data ends early".into()));
        }
        let dh = match &dh_t {
            Some(t) => t.value(&mut bits)?,
            None => iadh
                .decode(&mut mq)
                .ok_or_else(|| Error::Malformed("out-of-band height class delta".into()))?,
        };
        height += dh;
        if !(0..=crate::bitmap::MAX_SIDE as i64).contains(&height) {
            return Err(Error::Malformed("symbol height out of range".into()));
        }
        let mut width = 0i64;
        let mut tot_width = 0i64;
        let first = nsyms;
        loop {
            let dw = match &dw_t {
                Some(t) => t.decode(&mut bits)?,
                None => iadw.decode(&mut mq),
            };
            let Some(dw) = dw else { break };
            if nsyms >= num_new {
                return Err(Error::Malformed(
                    "symbol dictionary holds more symbols than it declares".into(),
                ));
            }
            width += dw;
            tot_width += width;
            if !(0..=crate::bitmap::MAX_SIDE as i64).contains(&width)
                || tot_width > crate::bitmap::MAX_SIDE as i64 * 64
            {
                return Err(Error::Malformed("symbol width out of range".into()));
            }
            let (w, h) = (width as u32, height as u32);
            if !huff || refagg {
                let bm = if !refagg {
                    let p = Generic {
                        w,
                        h,
                        template,
                        tpgdon: false,
                        at,
                        skip: None,
                    };
                    generic::decode(&mut mq, &mut gb, &p, budget)?
                } else {
                    let ninst = match &agginst_t {
                        Some(t) => t.value(&mut bits)?,
                        None => iaai
                            .decode(&mut mq)
                            .ok_or_else(|| Error::Malformed("out-of-band instance count".into()))?,
                    };
                    if ninst == 1 {
                        refine_one(
                            &mut mq, &mut bits, huff, &mut tcx, &huff_text, &all, w, h, rtemplate,
                            rat, budget,
                        )?
                    } else {
                        let p = Text {
                            w,
                            h,
                            instances: u32::try_from(ninst).map_err(|_| {
                                Error::Malformed("aggregate instance count out of range".into())
                            })?,
                            log_strips: 0,
                            symbols: &all,
                            refine: true,
                            default_pixel: 0,
                            op: Op::Or,
                            transposed: false,
                            corner: 1,
                            ds_offset: 0,
                            rtemplate,
                            rat,
                        };
                        let mut c = if huff {
                            Coder::Huff {
                                bits: &mut bits,
                                t: &huff_text,
                                gr: &mut tcx.gr,
                            }
                        } else {
                            Coder::Arith {
                                mq: &mut mq,
                                cx: &mut tcx,
                            }
                        };
                        text::decode(&mut c, &p, budget)?.0
                    }
                };
                all.push(Rc::new(bm));
            } else {
                widths.push(w);
            }
            nsyms += 1;
        }
        if huff && !refagg {
            let size = bmsize_t
                .as_ref()
                .map(|t| t.value(&mut bits))
                .transpose()?
                .unwrap_or(0);
            bits.align();
            let tw = tot_width as u32;
            let h = height as u32;
            let rest = bits.rest();
            let coll = if size == 0 {
                let stride = (tw as usize).div_ceil(8);
                let mut bm = Bitmap::new(tw, h, budget)?;
                for y in 0..h as usize {
                    for x in 0..tw as usize {
                        let byte = rest.get(y * stride + x / 8).copied().unwrap_or(0);
                        bm.d[y * tw as usize + x] = (byte >> (7 - (x & 7))) & 1;
                    }
                }
                bits.skip_bytes(stride * h as usize);
                bm
            } else {
                let size = usize::try_from(size).unwrap_or(usize::MAX).min(rest.len());
                let (bm, _, _) = mmr::decode(&rest[..size], tw, h, budget)?;
                bits.skip_bytes(size);
                bm
            };
            let mut x0 = 0i64;
            for &w in &widths[first as usize..] {
                all.push(Rc::new(coll.crop(x0, 0, w, h, budget)?));
                x0 += w as i64;
            }
        }
    }

    // Export flags (6.5.10).
    let mut exported = Vec::new();
    let mut index = 0u64;
    let mut cur = false;
    let mut rounds = 0u64;
    let b1 = Table::standard(1);
    while index < total {
        rounds += 1;
        if rounds > 2 * total + 4 || mq.exhausted() || bits.exhausted() {
            return Err(Error::Malformed(
                "symbol dictionary export flags do not add up".into(),
            ));
        }
        let run = if huff {
            b1.value(&mut bits)?
        } else {
            iaex.decode(&mut mq)
                .ok_or_else(|| Error::Malformed("out-of-band export run".into()))?
        };
        if run < 0 || index as i64 + run > total as i64 {
            return Err(Error::Malformed(
                "symbol dictionary export run out of range".into(),
            ));
        }
        if cur {
            exported.extend(
                all[index as usize..(index as i64 + run) as usize]
                    .iter()
                    .cloned(),
            );
        }
        index += run as u64;
        cur = !cur;
    }
    Ok(Dict {
        exported,
        gb: ctx_retained.then_some(gb),
        gr: ctx_retained.then_some(tcx.gr),
    })
}

/// A symbol that refines one other (6.5.8.2.2).
#[allow(clippy::too_many_arguments)]
fn refine_one(
    mq: &mut Mq,
    bits: &mut Bits,
    huff: bool,
    tcx: &mut TextCx,
    ht: &HuffText,
    all: &[Rc<Bitmap>],
    w: u32,
    h: u32,
    rtemplate: u8,
    rat: [(i8, i8); 2],
    budget: &Budget,
) -> Result<Bitmap> {
    let oob = || Error::Malformed("out-of-band refinement offset".into());
    let (id, rdx, rdy) = if huff {
        let id = bits.bits(ht.symcodelen) as usize;
        let rdx = ht.rdx.value(bits)?;
        let rdy = ht.rdy.value(bits)?;
        (id, rdx, rdy)
    } else {
        let id = tcx.id.decode(mq) as usize;
        let rdx = tcx.rdx.decode(mq).ok_or_else(oob)?;
        let rdy = tcx.rdy.decode(mq).ok_or_else(oob)?;
        (id, rdx, rdy)
    };
    let sym = all.get(id).ok_or_else(|| {
        Error::Malformed(format!("symbol {id} refines a symbol that does not exist"))
    })?;
    let p = Refine {
        w,
        h,
        template: rtemplate,
        reference: sym,
        dx: rdx,
        dy: rdy,
        tpgron: false,
        at: rat,
    };
    if huff {
        let size = ht.rsize.value(bits)?;
        bits.align();
        let rest = bits.rest();
        let size = usize::try_from(size).unwrap_or(usize::MAX).min(rest.len());
        let mut m = Mq::new(&rest[..size]);
        let bm = generic::refine(&mut m, &mut tcx.gr, &p, budget)?;
        bits.skip_bytes(size);
        Ok(bm)
    } else {
        generic::refine(mq, &mut tcx.gr, &p, budget)
    }
}
