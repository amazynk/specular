//! Precinct decoding (ISO/IEC 21122-1 Annex C and D): precinct and packet headers,
//! significance flags, bit-plane counts (raw, unary or vertically predicted), magnitude and
//! sign bits, then inverse quantisation into the interleaved coefficient planes.

use crate::bits::Bits;
use crate::header::Header;
use crate::layout::Layout;
use crate::{Error, Result};

const SIGN: u32 = 1 << 31;
/// Largest bit-plane count (bounded codes and truncation).
const MAX_GCLI: i32 = 15;
/// Longest unary code accepted in a bit-plane count.
const MAX_UNARY: u32 = 32;

fn corrupt(what: &str) -> Error {
    Error::Malformed(format!("damaged precinct ({what})"))
}

/// Bit-plane counts and coefficients of one precinct of one column.
#[derive(Default)]
struct Precinct {
    /// Per band line: one count per code group.
    gcli: Vec<Vec<i8>>,
    /// Per band line: sign (bit 31) and magnitude per coefficient, whole code groups.
    coef: Vec<Vec<u32>>,
    /// Truncation per band.
    gtli: Vec<i32>,
    /// Precinct row.
    py: usize,
    /// Temporal coding: per band line, the mode of each TDC selection group (0 intra,
    /// 1 intra with refresh weights, 2 and 3 predicted from the frame buffer).
    tdc: Vec<Vec<u8>>,
}

impl Precinct {
    fn new(l: &Layout, px: usize, ng: usize, si: usize) -> Precinct {
        let n = l.lines.len();
        let mut gcli = Vec::with_capacity(n);
        let mut coef = Vec::with_capacity(n);
        let mut tdc = Vec::new();
        for i in 0..n {
            let groups = l.width(i, px).div_ceil(ng);
            gcli.push(vec![0i8; groups]);
            coef.push(vec![0u32; groups * ng]);
            if si > 0 {
                tdc.push(vec![0u8; groups.div_ceil(si)]);
            }
        }
        Precinct {
            gcli,
            coef,
            gtli: vec![0; l.bands.len()],
            py: 0,
            tdc,
        }
    }
}

/// One component's coefficient rows covered by a slice.
pub(crate) struct Plane<'a> {
    pub data: &'a mut [i32],
    /// Component row of `data`'s first row.
    pub row0: usize,
    pub width: usize,
}

/// Decodes the precincts of one slice.
pub(crate) struct SliceDecoder<'h> {
    h: &'h Header,
    l: &'h Layout,
    ng: usize,
    long: bool,
    /// TicoRAW: a vertically predicted line without one above has plain unary counts.
    tico: bool,
    cur: Vec<Precinct>,
    top: Vec<Precinct>,
    /// Scratch: significance (inclusion) per code group, and prediction residuals.
    sig: Vec<Vec<bool>>,
    resid: Vec<i32>,
    /// Scratch: the bit-plane counts of the line above.
    above: Vec<i8>,
}

impl<'h> SliceDecoder<'h> {
    pub(crate) fn new(h: &'h Header, l: &'h Layout) -> Self {
        let ng = h.ng as usize;
        let si = h.tdc.as_ref().map(|t| t.si as usize).unwrap_or(0);
        let cur = (0..l.npx).map(|px| Precinct::new(l, px, ng, si)).collect();
        let top = (0..l.npx).map(|px| Precinct::new(l, px, ng, si)).collect();
        let sig = (0..l.lines.len())
            .map(|i| vec![false; l.width(i, 0).div_ceil(ng)])
            .collect();
        let maxg = (0..l.lines.len())
            .map(|i| l.width(i, 0).div_ceil(ng))
            .max()
            .unwrap_or(0);
        SliceDecoder {
            h,
            l,
            ng,
            long: h.long_headers(),
            tico: h.ticoraw(),
            cur,
            top,
            sig,
            resid: vec![0; maxg],
            above: Vec::with_capacity(maxg),
        }
    }

    /// Decodes precinct (`px`, `py`) at the reader's position into `planes`; `first` says it
    /// opens its slice (no precinct above to predict from). With temporal coding, `fb` is
    /// the frame buffer (laid out as `planes`).
    #[allow(clippy::needless_range_loop)]
    pub(crate) fn precinct(
        &mut self,
        r: &mut Bits,
        px: usize,
        py: usize,
        first: bool,
        planes: &mut [Plane],
        fb: Option<&mut [Plane]>,
    ) -> Result<()> {
        let (h, l) = (self.h, self.l);
        let nb = l.bands.len();
        let lprc = r.read(24) as usize * 8;
        let q = r.read(8) as i32;
        let rr = r.read(8) as i32;
        let (qf, rf) = if h.tdc.is_some() {
            (r.read(8) as i32, r.read(8) as i32)
        } else {
            (0, 0)
        };
        let mut mode = vec![0u8; nb];
        for m in mode.iter_mut() {
            *m = r.read(2) as u8;
        }
        let mut di = vec![0u8; nb];
        if h.tdc.is_some() {
            for m in di.iter_mut() {
                *m = r.read(2) as u8;
            }
        }
        r.align();
        let data_start = r.pos();
        if r.overrun() {
            return Err(Error::Truncated(
                "the codestream ends inside a precinct".into(),
            ));
        }
        let mut p = std::mem::take(&mut self.cur[px]);
        p.py = py;
        for b in 0..nb {
            let wi = l.weight_of[b];
            p.gtli[b] = if di[b] == 1 {
                match (h.refresh_gains.get(wi), h.refresh_priorities.get(wi)) {
                    (Some(&g), Some(&pr)) => truncation(q, rr, g, pr),
                    _ => {
                        self.cur[px] = p;
                        return Err(corrupt("refresh weights missing"));
                    }
                }
            } else {
                truncation(q, rr, h.gains[wi], h.priorities[wi])
            };
        }
        let above = std::mem::take(&mut self.top[px]);
        let top = if first { None } else { Some(&above) };
        let mut res = self.packets(r, &mut p, top, &mode, &di, py);
        if res.is_ok() && r.pos() - data_start > lprc {
            res = Err(corrupt("data longer than its precinct"));
        }
        if res.is_ok() {
            r.seek(data_start + lprc);
            dequantise(&mut p, l, h.qpih, self.ng, py);
            if let (Some(t), Some(fb)) = (&h.tdc, fb) {
                inverse_tdc(&mut p, l, h, t, (qf, rf), (px, py), fb);
            }
            place(&p, l, h.fq, px, py, planes);
        }
        // This precinct is the one above the next row's.
        self.top[px] = p;
        self.cur[px] = above;
        res
    }

    fn packets(
        &mut self,
        r: &mut Bits,
        p: &mut Precinct,
        top: Option<&Precinct>,
        mode: &[u8],
        di: &[u8],
        py: usize,
    ) -> Result<()> {
        let (h, l) = (self.h, self.l);
        let ng = self.ng;
        let (dl, gl, sl) = if self.long {
            (20, 20, 15)
        } else {
            (15, 13, 11)
        };
        for &(i0, i1) in &l.packets {
            if !l.included(i0, py) {
                continue;
            }
            let raw = r.bit() == 1;
            let data_len = r.read(dl) as usize * 8;
            let gcli_len = r.read(gl) as usize * 8;
            let sign_len = r.read(sl) as usize * 8;
            r.align();
            if h.tdc.is_some() {
                // TDC selection: per line, one flag per selection group (switched bands) or
                // the band's mode for every group.
                let mut flags = false;
                for idx in i0..i1 {
                    if !l.included(idx, py) {
                        continue;
                    }
                    let b = l.lines[idx].band;
                    let f = &mut p.tdc[idx];
                    if di[b] == 3 {
                        for v in f.iter_mut() {
                            *v = if r.bit() == 1 { 3 } else { 0 };
                        }
                        flags = true;
                    } else {
                        f.fill(di[b]);
                    }
                }
                if flags {
                    r.align();
                }
            }
            if !raw {
                for idx in i0..i1 {
                    let b = l.lines[idx].band;
                    if !l.included(idx, py) || mode[b] & 2 == 0 {
                        continue;
                    }
                    let groups = p.gcli[idx].len();
                    let sig = &mut self.sig[idx];
                    let ss = h.ss as usize;
                    for chunk in sig[..groups].chunks_mut(ss) {
                        let z = r.bit() == 1;
                        chunk.fill(!z);
                    }
                }
                r.align();
            }
            let at = r.pos();
            for idx in i0..i1 {
                if l.included(idx, py) {
                    let b = l.lines[idx].band;
                    self.counts(r, p, top, idx, mode[b] & 1 != 0, mode[b] & 2 != 0, raw)?;
                }
            }
            r.align();
            sub_end(r, at, gcli_len, "bit-plane counts")?;
            let at = r.pos();
            for idx in i0..i1 {
                if l.included(idx, py) {
                    let gtli = p.gtli[l.lines[idx].band];
                    magnitudes(r, &p.gcli[idx], &mut p.coef[idx], ng, gtli, h.fs == 0);
                }
            }
            r.align();
            sub_end(r, at, data_len, "coefficients")?;
            if h.fs == 1 {
                let at = r.pos();
                for idx in i0..i1 {
                    if l.included(idx, py) {
                        for c in p.coef[idx].iter_mut() {
                            if *c != 0 && r.bit() == 1 {
                                *c |= SIGN;
                            }
                        }
                    }
                }
                r.align();
                sub_end(r, at, sign_len, "signs")?;
            }
            if r.overrun() {
                return Err(Error::Truncated(
                    "the codestream ends inside a precinct".into(),
                ));
            }
        }
        Ok(())
    }

    /// The bit-plane counts of band line `idx`.
    #[allow(clippy::too_many_arguments)]
    fn counts(
        &mut self,
        r: &mut Bits,
        p: &mut Precinct,
        top: Option<&Precinct>,
        idx: usize,
        vertical: bool,
        sig: bool,
        raw: bool,
    ) -> Result<()> {
        let (h, l) = (self.h, self.l);
        let line = l.lines[idx];
        let b = line.band;
        let gtli = p.gtli[b];
        let groups = p.gcli[idx].len();
        if raw {
            let br = h.br as u32;
            for g in p.gcli[idx].iter_mut() {
                *g = r.read(br) as i8;
            }
            return Ok(());
        }
        // The line above: the previous line of this band in this precinct, or the last
        // line of the band in the precinct above.
        let src: Option<(&[i8], i32)> = if line.y == 0 {
            match top {
                Some(t) => {
                    let n = l.band_lines(b, t.py);
                    if n <= 0 {
                        None
                    } else {
                        let i = l.line_of[b][n as usize - 1];
                        Some((&t.gcli[i][..], t.gtli[b]))
                    }
                }
                None => None,
            }
        } else {
            Some((&p.gcli[l.line_of[b][line.y - 1]][..], gtli))
        };
        let gtli_top = src.map(|s| s.1).unwrap_or(0);
        let have_above = src.is_some();
        self.above.clear();
        if let Some((a, _)) = src {
            self.above.extend_from_slice(a);
        }
        let above = have_above.then_some(&self.above[..]);
        let included = &self.sig[idx];
        let resid = &mut self.resid[..groups];
        let incl = |g: usize| !sig || included[g];
        match above {
            Some(a) if vertical => {
                for (g, v) in resid.iter_mut().enumerate() {
                    if !incl(g) {
                        *v = 0;
                        continue;
                    }
                    let pred = (a.get(g).copied().unwrap_or(0) as i32).max(gtli.max(gtli_top));
                    let trigger = (pred - gtli).max(0) as u32;
                    let t = r.ones(MAX_UNARY);
                    if t >= MAX_UNARY {
                        return Err(corrupt("bit-plane count code"));
                    }
                    *v = if t > 2 * trigger {
                        (t - trigger) as i32
                    } else {
                        let m = t.div_ceil(2) as i32;
                        if t % 2 == 1 { -m } else { m }
                    };
                }
                let rm = h.rm == 1;
                let out = &mut p.gcli[idx];
                for g in 0..groups {
                    if rm && !incl(g) {
                        out[g] = 0;
                        continue;
                    }
                    let t = (a.get(g).copied().unwrap_or(0) as i32).max(gtli.max(gtli_top));
                    let mut v = t + resid[g];
                    if v <= gtli {
                        v -= gtli;
                    }
                    out[g] = v.clamp(-128, 127) as i8;
                }
            }
            _ => {
                for (g, v) in resid.iter_mut().enumerate() {
                    if !incl(g) {
                        *v = 0;
                        continue;
                    }
                    if vertical && !self.tico {
                        // A vertically predicted line without one above: signed unary.
                        // At most sixteen bits: sixteen ones mean -15, fifteen and a zero
                        // 15; shorter non-zero runs are followed by a sign bit. TicoRAW
                        // codes such lines in plain unary, as unpredicted ones.
                        let n = r.ones(16);
                        *v = match n {
                            16 => -15,
                            0 | 15 => n as i32,
                            _ if r.bit() == 1 => -(n as i32),
                            _ => n as i32,
                        };
                    } else {
                        let t = r.ones(MAX_UNARY);
                        if t >= MAX_UNARY {
                            return Err(corrupt("bit-plane count code"));
                        }
                        *v = t as i32;
                    }
                }
                let out = &mut p.gcli[idx];
                for g in 0..groups {
                    out[g] = if resid[g] > 0 {
                        (resid[g] + gtli).min(127) as i8
                    } else {
                        0
                    };
                }
            }
        }
        Ok(())
    }
}

/// The truncation of a band from the precinct's quantisation and refinement.
fn truncation(q: i32, r: i32, gain: u8, priority: u8) -> i32 {
    let mut t = q - gain as i32;
    if (priority as i32) < r {
        t -= 1;
    }
    t.clamp(0, MAX_GCLI)
}

/// Where band line `idx` of precinct (`px`, `py`) goes: component, row, first column,
/// column step and coefficient count.
fn geometry(l: &Layout, idx: usize, px: usize, py: usize) -> (usize, usize, usize, usize, usize) {
    let line = l.lines[idx];
    let band = &l.bands[line.band];
    let comp = &l.comps[band.c];
    let oy = if band.high_y {
        1usize << (band.dy - 1)
    } else {
        0
    };
    let ox = if band.high_x {
        1usize << (band.dx - 1)
    } else {
        0
    };
    let row = oy + (l.ph >> (comp.sy - 1)) * py + (line.y << band.dy);
    let x0 = ox + (l.cs >> (comp.sx - 1)) * px;
    (band.c, row, x0, 1usize << band.dx, l.width(idx, px))
}

/// Inverse temporal prediction (TDC): predicted groups add the frame buffer, intra and
/// refresh groups are rescaled; the frame buffer keeps the result, truncated.
fn inverse_tdc(
    p: &mut Precinct,
    l: &Layout,
    h: &Header,
    t: &crate::header::Tpc,
    (qf, rf): (i32, i32),
    (px, py): (usize, usize),
    fb: &mut [Plane],
) {
    let qad = t.qbi.max(t.qbr).max(0) as u32;
    let (si, qbi, qbr) = (t.si as usize, t.qbi as i32, t.qbr as i32);
    let per_group = h.ng as usize * si.max(1);
    for idx in 0..l.lines.len() {
        if !l.included(idx, py) {
            continue;
        }
        let b = l.lines[idx].band;
        let mask = !((1i32
            << truncation(
                qf,
                rf,
                h.gains[l.weight_of[b]],
                h.priorities[l.weight_of[b]],
            ))
            - 1);
        let sh = t.sh.get(b).copied().unwrap_or(0);
        let yh = t.yh.get(b).copied().unwrap_or(0);
        let period = if sh > 0 { (1u8 << sh.min(7)) - 1 } else { 0 };
        let (c, row, x0, step, n) = geometry(l, idx, px, py);
        let plane = &mut fb[c];
        let Some(local) = row.checked_sub(plane.row0) else {
            continue;
        };
        let w = plane.width;
        let Some(fbrow) = plane.data.get_mut(local * w..(local + 1) * w) else {
            continue;
        };
        for (i, v) in p.coef[idx].iter_mut().take(n).enumerate() {
            let k = i / per_group;
            let mode = p.tdc[idx].get(k).copied().unwrap_or(0);
            let refresh = period > 0 && (k as u8).wrapping_sub(yh) & period == 0;
            let mag = (*v & !SIGN) as i32;
            let c = if *v & SIGN != 0 {
                mag.wrapping_neg()
            } else {
                mag
            };
            let x = x0 + i * step;
            let prev = fbrow.get(x).copied().unwrap_or(0);
            let cp = if mode < 2 {
                c.wrapping_shl((qad as i32 - qbi) as u32)
            } else if refresh {
                c.wrapping_shl((qad as i32 - qbr) as u32)
            } else {
                c.wrapping_shl(qad).wrapping_add(prev)
            };
            if let Some(f) = fbrow.get_mut(x) {
                *f = if cp < 0 {
                    (cp.wrapping_neg() & mask).wrapping_neg()
                } else {
                    cp & mask
                };
            }
            *v = if cp < 0 {
                cp.wrapping_neg() as u32 | SIGN
            } else {
                cp as u32
            };
        }
    }
}

/// Checks a sub-packet's length and skips its padding.
fn sub_end(r: &mut Bits, at: usize, len: usize, what: &str) -> Result<()> {
    let used = r.pos() - at;
    if used > len {
        return Err(corrupt(what));
    }
    r.seek(at + len);
    Ok(())
}

/// Magnitude bits (and with `signs`, sign bits) of the code groups above the truncation.
fn magnitudes(r: &mut Bits, gcli: &[i8], coef: &mut [u32], ng: usize, gtli: i32, signs: bool) {
    coef.fill(0);
    let n = ng as u32;
    for (g, &m) in gcli.iter().enumerate() {
        let m = m as i32;
        if m <= gtli {
            continue;
        }
        let c = &mut coef[g * ng..(g + 1) * ng];
        if signs {
            let s = r.read(n);
            for (i, v) in c.iter_mut().enumerate() {
                if (s >> (n - 1 - i as u32)) & 1 == 1 {
                    *v = SIGN;
                }
            }
        }
        let mut bp = m - 1;
        while bp >= gtli {
            let bits = r.read(n);
            if bits != 0 {
                for (i, v) in c.iter_mut().enumerate() {
                    *v |= ((bits >> (n - 1 - i as u32)) & 1) << bp;
                }
            }
            bp -= 1;
        }
    }
}

/// Inverse quantisation of every band line present in the precinct.
fn dequantise(p: &mut Precinct, l: &Layout, qpih: u8, ng: usize, py: usize) {
    for idx in 0..l.lines.len() {
        if !l.included(idx, py) {
            continue;
        }
        let gtli = p.gtli[l.lines[idx].band];
        if gtli <= 0 {
            continue;
        }
        let (gcli, coef) = (&p.gcli[idx], &mut p.coef[idx]);
        for (g, &m) in gcli.iter().enumerate() {
            let m = m as i32;
            if m <= gtli {
                continue;
            }
            let c = &mut coef[g * ng..(g + 1) * ng];
            if qpih == 1 {
                // Uniform: the kept bits repeated below themselves.
                let zeta = (m - gtli + 1) as u32;
                for v in c.iter_mut() {
                    let sign = *v & SIGN;
                    let mut phi = *v & !SIGN;
                    let mut rho = 0u32;
                    while phi > 0 {
                        rho = rho.wrapping_add(phi);
                        phi = phi.checked_shr(zeta).unwrap_or(0);
                    }
                    *v = sign | (rho & !SIGN);
                }
            } else {
                // Dead zone: mid-point reconstruction of non-zero magnitudes.
                let half = 1u32 << (gtli - 1);
                for v in c.iter_mut() {
                    if *v & !SIGN != 0 {
                        *v |= half;
                    }
                }
            }
        }
    }
}

/// Writes a precinct's coefficients (times 2^Fq) into the interleaved component planes.
fn place(p: &Precinct, l: &Layout, fq: u8, px: usize, py: usize, planes: &mut [Plane]) {
    for idx in 0..l.lines.len() {
        if !l.included(idx, py) {
            continue;
        }
        let (c, row, x0, step, n) = geometry(l, idx, px, py);
        let plane = &mut planes[c];
        let Some(local) = row.checked_sub(plane.row0) else {
            continue;
        };
        let Some(dst) = plane
            .data
            .get_mut(local * plane.width..(local + 1) * plane.width)
        else {
            continue;
        };
        let src = &p.coef[idx];
        let mut x = x0;
        for &v in src.iter().take(n) {
            if x >= dst.len() {
                break;
            }
            let mag = ((v & !SIGN) as i32).wrapping_shl(fq as u32);
            dst[x] = if v & SIGN != 0 {
                mag.wrapping_neg()
            } else {
                mag
            };
            x += step;
        }
    }
}
