//! Decoding one bit plane: the lowest resolution layer (6.5, 6.7.1) and the differential
//! layers (6.4, 6.6, 6.7.2), stripe by stripe.
//!
//! Layers are kept one byte per pixel (0 or 1) in rows padded with background on both sides,
//! so template pixels left and right of the image read as background without tests.

use crate::arith::{Contexts, Decoder};
use crate::bie::{AtMove, Bid, Bih, DPON, LRLTWO, TPBON, TPDON};
use crate::{Error, Result};

/// Background columns each side of a row.
pub(crate) const PAD: usize = 8;

/// Context of the lowest-layer TP pseudo-pixel (Figures 11 and 12 in our bit order).
const TPB3CX: usize = 0x0e5;
const TPB2CX: usize = 0x195;
/// Context of the differential-layer TP pseudo-pixel (Figure 8: phase 3, every
/// high-resolution pixel foreground, every low-resolution pixel background).
const TPDCX: usize = 0xc3f;

/// One resolution layer of a plane.
pub(crate) struct Layer {
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub px: Vec<u8>,
    /// Rows decoded from the top.
    pub rows: usize,
}

impl Layer {
    pub(crate) fn new(width: usize, height: usize) -> Result<Layer> {
        let stride = width + 2 * PAD;
        let len = stride.checked_mul(height).ok_or(Error::TooLarge)?;
        let mut px = Vec::new();
        px.try_reserve_exact(len).map_err(|_| Error::TooLarge)?;
        px.resize(len, 0);
        Ok(Layer {
            width,
            height,
            stride,
            px,
            rows: 0,
        })
    }

    #[inline]
    fn row(&self, y: usize) -> &[u8] {
        &self.px[y * self.stride..(y + 1) * self.stride]
    }
}

/// The adaptive-template state of one plane and layer.
#[derive(Clone, Copy)]
struct At {
    tx: i8,
    ty: u8,
}

impl At {
    const DEFAULT: At = At { tx: 0, ty: 0 };
}

fn apply_moves(moves: &[AtMove], line: u32, at: &mut At, min_tx: i8) -> Result<()> {
    for m in moves.iter().filter(|m| m.yat == line) {
        if m.tx == 0 && m.ty == 0 {
            *at = At::DEFAULT;
            continue;
        }
        // The pixel must already be known: on a line above, or left of the current one and
        // clear of the template.
        if m.ty == 0 && m.tx < min_tx {
            return Err(Error::Malformed(format!(
                "adaptive template moved onto the template ({}, {})",
                m.tx, m.ty
            )));
        }
        *at = At { tx: m.tx, ty: m.ty };
    }
    Ok(())
}

/// Per-layer decoding state carried from stripe to stripe.
pub(crate) struct State {
    cx: Contexts,
    at: At,
    /// LNTP of the line above (lowest layer TP).
    lntp: u8,
    /// The previous stripe ended with SDRST.
    reset: bool,
}

impl State {
    pub(crate) fn new(contexts: usize) -> State {
        State {
            cx: vec![0; contexts],
            at: At::DEFAULT,
            lntp: 1,
            reset: false,
        }
    }

    /// Applies a pending SDRST reset; returns whether there was one (the stripe then starts
    /// as the image does: nothing above it is referenced, 6.2.5).
    fn start_stripe(&mut self) -> bool {
        let r = self.reset;
        if r {
            self.cx.fill(0);
            self.at = At::DEFAULT;
            self.lntp = 1;
            self.reset = false;
        }
        r
    }
}

/// Outcome of a stripe: whether its data ran out early.
pub(crate) struct StripeResult {
    pub cut: bool,
}

/// Decode stripe rows `y0..y1` of the lowest resolution layer.
pub(crate) fn lowest_stripe(
    bih: &Bih,
    st: &mut State,
    l: &mut Layer,
    y0: usize,
    y1: usize,
    sde: &crate::bie::Sde,
) -> Result<StripeResult> {
    let top = if st.start_stripe() { y0 } else { 0 };
    let two = bih.options & LRLTWO != 0;
    let tpbon = bih.options & TPBON != 0;
    let min_tx = if two { 5 } else { 3 };
    let mut dec = Decoder::new(&sde.scd);
    let w = l.width;
    let stride = l.stride;
    let zero = vec![0u8; stride];
    for y in y0..y1 {
        apply_moves(&sde.moves, (y - y0) as u32, &mut st.at, min_tx)?;
        let (above, rest) = l.px.split_at_mut(y * stride);
        let cur = &mut rest[..stride];
        let r1: &[u8] = if y > top {
            &above[(y - 1) * stride..y * stride]
        } else {
            &zero
        };
        if tpbon {
            let slntp = dec.decode(&mut st.cx[if two { TPB2CX } else { TPB3CX }]);
            st.lntp = (slntp ^ st.lntp) ^ 1;
            if st.lntp == 0 {
                cur.copy_from_slice(r1);
                continue;
            }
        }
        let at = st.at;
        let at_row: Option<&[u8]> = if at.tx == 0 && at.ty == 0 {
            None
        } else if (at.ty as usize) > y - top {
            Some(&zero)
        } else if at.ty == 0 {
            None
        } else {
            let ay = y - at.ty as usize;
            Some(&above[ay * stride..(ay + 1) * stride])
        };
        let at_moved = !(at.tx == 0 && at.ty == 0);
        // Fetch the moved AT pixel at column x.
        let at_pix = |x: usize, cur: &[u8]| -> u32 {
            let ax = x as isize - at.tx as isize;
            if ax < 0 || ax as usize >= w {
                return 0;
            }
            let i = PAD + ax as usize;
            u32::from(match at_row {
                Some(r) => r[i],
                None => cur[i],
            })
        };
        if two {
            // y-1: x-3..x+1 (bits 9..5), A (bit 4), y: x-4..x-1 (bits 3..0).
            let mut r1w: u32 = (u32::from(r1[PAD]) << 1) | u32::from(r1[PAD + 1]);
            let mut r0: u32 = 0;
            for x in 0..w {
                r1w = ((r1w << 1) | u32::from(r1[PAD + x + 2])) & 0x3f;
                let a = if at_moved { at_pix(x, cur) } else { r1w & 1 };
                let ctx = ((r1w >> 1) << 5) | (a << 4) | r0;
                let pix = dec.decode(&mut st.cx[ctx as usize]);
                cur[PAD + x] = pix;
                r0 = ((r0 << 1) | u32::from(pix)) & 0xf;
            }
        } else {
            let r2: &[u8] = if y >= top + 2 {
                &above[(y - 2) * stride..(y - 1) * stride]
            } else {
                &zero
            };
            // y-2: x-1, x, x+1 (bits 9..7); y-1: x-2..x+1, A (bits 6..2); y: x-2, x-1.
            let mut r2w: u32 = u32::from(r2[PAD]);
            let mut r1w: u32 = (u32::from(r1[PAD]) << 1) | u32::from(r1[PAD + 1]);
            let mut r0: u32 = 0;
            for x in 0..w {
                r2w = ((r2w << 1) | u32::from(r2[PAD + x + 1])) & 7;
                r1w = ((r1w << 1) | u32::from(r1[PAD + x + 2])) & 0x1f;
                let g1 = if at_moved {
                    (r1w & 0x1e) | at_pix(x, cur)
                } else {
                    r1w
                };
                let ctx = (r2w << 7) | (g1 << 2) | r0;
                let pix = dec.decode(&mut st.cx[ctx as usize]);
                cur[PAD + x] = pix;
                r0 = ((r0 << 1) | u32::from(pix)) & 3;
            }
        }
    }
    l.rows = y1;
    st.reset = sde.reset;
    Ok(StripeResult { cut: sde.cut })
}

/// The unpacked deterministic-prediction table: phase 0 at 0, 1 at 256, 2 at 768, 3 at 2816.
pub(crate) fn dp_entries(packed: &[u8; 1728]) -> Vec<u8> {
    let mut v = Vec::with_capacity(6912);
    for b in packed {
        for k in (0..4).rev() {
            v.push((b >> (2 * k)) & 3);
        }
    }
    v
}

const DP_OFFSET: [usize; 4] = [0, 256, 768, 2816];

/// Decode stripe rows `y0..y1` of a differential layer `h` from the layer below `lo`, whose
/// rows of this stripe are `ly0..ly1`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn differential_stripe(
    bih: &Bih,
    st: &mut State,
    dp: Option<&[u8]>,
    lo: &Layer,
    h: &mut Layer,
    (y0, y1): (usize, usize),
    ly1: usize,
    sde: &crate::bie::Sde,
) -> Result<StripeResult> {
    let (top, ltop) = if st.start_stripe() {
        (y0, (y0 / 2) as isize)
    } else {
        (0, 0)
    };
    let tpdon = bih.options & TPDON != 0;
    let dp = if bih.options & DPON != 0 { dp } else { None };
    let mut dec = Decoder::new(&sde.scd);
    let w = h.width;
    let stride = h.stride;
    let lstride = lo.stride;
    let zero = vec![0u8; stride.max(lstride)];
    // Low-resolution row Y with the edge rules: background above the image, the last line of
    // this stripe replicated below it.
    let lrow = |yy: isize| -> &[u8] {
        if yy < ltop {
            &zero[..lstride]
        } else {
            let yy = (yy as usize).min(ly1.saturating_sub(1));
            lo.row(yy)
        }
    };
    let mut lntp = 1u8;
    for y in y0..y1 {
        apply_moves(&sde.moves, (y - y0) as u32, &mut st.at, 3)?;
        let at = st.at;
        let yp = (y / 2) as isize;
        let (lm, l0, l1) = (lrow(yp - 1), lrow(yp), lrow(yp + 1));
        let (above, rest) = h.px.split_at_mut(y * stride);
        let cur = &mut rest[..stride];
        let r1: &[u8] = if y > top {
            &above[(y - 1) * stride..y * stride]
        } else {
            &zero[..stride]
        };
        let r2: &[u8] = if y >= top + 2 {
            &above[(y - 2) * stride..(y - 1) * stride]
        } else {
            &zero[..stride]
        };
        let odd_row = y & 1;
        if tpdon && odd_row == 0 {
            lntp = dec.decode(&mut st.cx[TPDCX]);
        }
        let at_moved = !(at.tx == 0 && at.ty == 0);
        let at_row: Option<&[u8]> = if !at_moved || at.ty == 0 {
            None
        } else if at.ty as usize > y - top {
            Some(&zero[..stride])
        } else {
            let ay = y - at.ty as usize;
            Some(&above[ay * stride..(ay + 1) * stride])
        };
        for x in 0..w {
            let xp = PAD + x;
            let lx = PAD + x / 2;
            if tpdon && lntp == 0 {
                let c = l0[lx];
                let typical = lm[lx - 1] == c
                    && lm[lx] == c
                    && lm[lx + 1] == c
                    && l0[lx - 1] == c
                    && l0[lx + 1] == c
                    && l1[lx - 1] == c
                    && l1[lx] == c
                    && l1[lx + 1] == c;
                if typical {
                    cur[xp] = c;
                    continue;
                }
            }
            let phase = (odd_row << 1) | (x & 1);
            if let Some(dp) = dp {
                // Figure 13 around the block's top-left pixel (x0, y0).
                let (xb, rb_top, rb_mid): (usize, &[u8], &[u8]) = if odd_row == 0 {
                    (xp & !1, r1, &*cur)
                } else {
                    (xp & !1, r2, r1)
                };
                // xp = PAD + x and PAD is even, so `xp & !1` is the block's left column.
                let mut idx = usize::from(lm[lx - 1])
                    | usize::from(lm[lx]) << 1
                    | usize::from(l0[lx - 1]) << 2
                    | usize::from(l0[lx]) << 3
                    | usize::from(rb_top[xb - 1]) << 4
                    | usize::from(rb_top[xb]) << 5
                    | usize::from(rb_top[xb + 1]) << 6
                    | usize::from(rb_mid[xb - 1]) << 7;
                if phase >= 1 {
                    idx |= usize::from(rb_mid[xb]) << 8;
                }
                if phase >= 2 {
                    idx |= usize::from(rb_mid[xb + 1]) << 9;
                    idx |= usize::from(cur[xb - 1]) << 10;
                }
                if phase == 3 {
                    idx |= usize::from(cur[xb]) << 11;
                }
                let v = dp[DP_OFFSET[phase] + idx];
                if v < 2 {
                    cur[xp] = v;
                    continue;
                }
            }
            let a = if at_moved {
                let ax = x as isize - at.tx as isize;
                if ax < 0 || ax as usize >= w {
                    0
                } else {
                    match at_row {
                        Some(r) => r[PAD + ax as usize],
                        None => cur[PAD + ax as usize],
                    }
                }
            } else {
                r1[xp - 1]
            };
            let (la, lb) = if x & 1 == 0 {
                (lx - 1, lx)
            } else {
                (lx, lx + 1)
            };
            let ctx = usize::from(cur[xp - 1])
                | usize::from(cur[xp - 2]) << 1
                | usize::from(r1[xp + 1]) << 2
                | usize::from(r1[xp]) << 3
                | usize::from(a) << 4
                | usize::from(r2[xp]) << 5
                | usize::from(l0[la]) << 6
                | usize::from(l0[lb]) << 7
                | usize::from(l1[la]) << 8
                | usize::from(l1[lb]) << 9
                | phase << 10;
            cur[xp] = dec.decode(&mut st.cx[ctx]);
        }
    }
    h.rows = y1;
    st.reset = sde.reset;
    Ok(StripeResult { cut: sde.cut })
}

/// Decode plane `p` up to layer `bih.d`. Returns the layers of the plane (lowest first) and
/// whether data was missing.
pub(crate) fn decode_plane(
    bih: &Bih,
    bid: &Bid,
    yd: u32,
    p: u8,
    dp: Option<&[u8]>,
) -> Result<(Vec<Layer>, bool)> {
    let stripes = bih.stripes(yd);
    let mut layers: Vec<Layer> = Vec::new();
    let mut truncated = false;
    for d in bih.dl..=bih.d {
        let (w, hgt) = bih.layer_size(d, yd);
        let mut l = Layer::new(w as usize, hgt as usize)?;
        let ld = u64::from(bih.l0) << d;
        let mut st = State::new(if d == bih.dl { 1024 } else { 4096 });
        let mut ok = true;
        for s in 0..stripes {
            let y0 = (s * ld).min(hgt) as usize;
            let y1 = ((s + 1) * ld).min(hgt) as usize;
            if y0 >= y1 {
                break;
            }
            let k = bih.sde_index(s, d, p, stripes)?;
            let Some(sde) = bid.sdes.get(k as usize) else {
                ok = false;
                break;
            };
            let r = if d == bih.dl {
                lowest_stripe(bih, &mut st, &mut l, y0, y1, sde)?
            } else {
                let lo = layers.last().expect("lower layer decoded first");
                let lstride_rows = (u64::from(bih.l0) << (d - 1)) * (s + 1);
                let ly1 = (lstride_rows.min(lo.height as u64)) as usize;
                if lo.rows < ly1 {
                    ok = false;
                    break;
                }
                differential_stripe(bih, &mut st, dp, lo, &mut l, (y0, y1), ly1, sde)?
            };
            if r.cut {
                // What was decoded of this stripe is shown; the rest is not trusted.
                truncated = true;
                ok = false;
                break;
            }
        }
        if !ok {
            truncated = true;
        }
        layers.push(l);
        if !ok {
            break;
        }
    }
    Ok((layers, truncated))
}
