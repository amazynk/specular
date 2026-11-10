//! Pattern dictionaries (7.4.4, 6.7) and halftone regions (7.4.5, 6.6).

use crate::bitmap::{Bitmap, Budget, Op};
use crate::generic::{self, Generic};
use crate::mq::{Mq, code_len};
use crate::{Error, Result, be32, mmr};

/// Decodes a pattern dictionary segment into its patterns.
pub(crate) fn patterns(data: &[u8], budget: &Budget) -> Result<Vec<Bitmap>> {
    if data.len() < 7 {
        return Err(Error::Truncated("pattern dictionary header".into()));
    }
    let flags = data[0];
    let mmr_coded = flags & 1 != 0;
    let template = (flags >> 1) & 3;
    let (pw, ph) = (data[1] as u32, data[2] as u32);
    let gray_max = be32(data, 3).unwrap_or(0) as u64;
    if pw == 0 || ph == 0 {
        return Err(Error::Malformed("empty halftone pattern".into()));
    }
    let count = gray_max + 1;
    let total_w = count * pw as u64;
    if total_w > crate::bitmap::MAX_SIDE as u64 {
        return Err(Error::TooLarge);
    }
    let body = &data[7..];
    let coll = if mmr_coded {
        mmr::decode(body, total_w as u32, ph, budget)?.0
    } else {
        let mut at = generic::default_at(template);
        at[0] = (-(pw as i32), 0);
        if template == 0 {
            at[1] = (-3, -1);
            at[2] = (2, -2);
            at[3] = (-2, -2);
        }
        let mut cx = vec![0u8; generic::contexts(template)];
        let p = Generic {
            w: total_w as u32,
            h: ph,
            template,
            tpgdon: false,
            at,
            skip: None,
        };
        generic::decode(&mut Mq::new(body), &mut cx, &p, budget)?
    };
    (0..count)
        .map(|i| coll.crop(i as i64 * pw as i64, 0, pw, ph, budget))
        .collect()
}

/// A halftone region segment's data after the region segment information field.
pub(crate) fn region(
    data: &[u8],
    w: u32,
    h: u32,
    pats: &[Bitmap],
    budget: &Budget,
) -> Result<Bitmap> {
    if data.len() < 21 {
        return Err(Error::Truncated("halftone region header".into()));
    }
    let flags = data[0];
    let mmr_coded = flags & 1 != 0;
    let template = (flags >> 1) & 3;
    let enable_skip = flags & 8 != 0;
    let op = Op::from_bits((flags >> 4) & 7);
    let def = (flags >> 7) & 1;
    let gw = be32(data, 1).unwrap_or(0);
    let gh = be32(data, 5).unwrap_or(0);
    let gx = be32(data, 9).unwrap_or(0) as i32 as i64;
    let gy = be32(data, 13).unwrap_or(0) as i32 as i64;
    let rx = u16::from_be_bytes([data[17], data[18]]) as i64;
    let ry = u16::from_be_bytes([data[19], data[20]]) as i64;
    let body = &data[21..];
    let mut reg = Bitmap::filled(w, h, def, budget)?;
    if pats.is_empty() {
        return Err(Error::Malformed("halftone region without patterns".into()));
    }
    if gw as u64 * gh as u64 > crate::bitmap::MAX_PIXELS {
        return Err(Error::TooLarge);
    }
    let (pw, ph) = (pats[0].w as i64, pats[0].h as i64);
    let pos = |mg: i64, ng: i64| -> (i64, i64) {
        ((gx + mg * ry + ng * rx) >> 8, (gy + mg * rx - ng * ry) >> 8)
    };
    let skip = if enable_skip {
        let mut s = Bitmap::new(gw, gh, budget)?;
        for mg in 0..gh as i64 {
            for ng in 0..gw as i64 {
                let (x, y) = pos(mg, ng);
                if x + pw <= 0 || x >= w as i64 || y + ph <= 0 || y >= h as i64 {
                    s.d[(mg * gw as i64 + ng) as usize] = 1;
                }
            }
        }
        Some(s)
    } else {
        None
    };
    let bpp = code_len(pats.len() as u64);
    // Grey-scale image decoding (C.5): bit planes, most significant first, Gray-coded. The
    // grid's values (4 bytes each, up to 2^28 of them) come out of the page's budget too.
    budget.take(gw as u64 * gh as u64 * 4)?;
    let mut vals = vec![0u32; gw as usize * gh as usize];
    let mut prev: Option<Bitmap> = None;
    let mut cx = vec![0u8; generic::contexts(template)];
    let mut mq = Mq::new(body);
    let mut off = 0usize;
    let mut at = generic::default_at(template);
    if template <= 1 {
        at[0] = (3, -1);
    } else {
        at[0] = (2, -1);
    }
    at[1] = (-3, -1);
    at[2] = (2, -2);
    at[3] = (-2, -2);
    for j in (0..bpp).rev() {
        let mut plane = if mmr_coded {
            let (bm, used, _) = mmr::decode(body.get(off..).unwrap_or(&[]), gw, gh, budget)?;
            off += used;
            bm
        } else {
            let p = Generic {
                w: gw,
                h: gh,
                template,
                tpgdon: false,
                at,
                skip: skip.as_ref(),
            };
            generic::decode(&mut mq, &mut cx, &p, budget)?
        };
        if let Some(pr) = &prev {
            for (a, b) in plane.d.iter_mut().zip(&pr.d) {
                *a ^= b;
            }
        }
        for (v, &b) in vals.iter_mut().zip(&plane.d) {
            *v |= (b as u32) << j;
        }
        prev = Some(plane);
    }
    let max = pats.len() - 1;
    for mg in 0..gh as i64 {
        for ng in 0..gw as i64 {
            let (x, y) = pos(mg, ng);
            let v = (vals[(mg * gw as i64 + ng) as usize] as usize).min(max);
            reg.compose(&pats[v], x, y, op);
        }
    }
    Ok(reg)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 21-byte header asking for a 16384 × 16384 grid over a 1 × 1 region: its values
    /// (1 GiB) are charged to the page budget, which refuses them.
    #[test]
    fn huge_grid_charged() {
        let b = Budget::new();
        let pat = Bitmap::new(1, 1, &b).unwrap();
        let mut d = vec![0u8];
        d.extend(16384u32.to_be_bytes());
        d.extend(16384u32.to_be_bytes());
        d.extend([0u8; 12]);
        assert!(matches!(region(&d, 1, 1, &[pat], &b), Err(Error::TooLarge)));
    }
}
