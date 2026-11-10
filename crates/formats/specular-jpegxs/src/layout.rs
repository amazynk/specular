//! The image decomposition (ISO/IEC 21122-1 Annex B): which wavelet bands each component has,
//! their sizes, how they are cut into precincts, and in which order their lines are packed.

use crate::header::{Header, MAX_NLX, MAX_NLY};
use crate::{Error, Result};

/// One component's sample grid.
#[derive(Clone, Debug)]
pub(crate) struct Comp {
    pub w: usize,
    pub h: usize,
    pub sx: usize,
    pub sy: usize,
    /// Horizontal and vertical decomposition depth (0 for components with the transform
    /// suppressed).
    pub nlx: u8,
    pub nly: u8,
}

/// One wavelet band of one component.
#[derive(Clone, Debug)]
pub(crate) struct Band {
    pub c: usize,
    /// Decomposition depth of the band in each direction.
    pub dx: u8,
    pub dy: u8,
    pub high_x: bool,
    pub high_y: bool,
    /// Width in coefficients inside a precinct, ordinary and last column.
    pub pw: [usize; 2],
    /// Lines inside a precinct, ordinary and last row (may be zero or less).
    pub lines: [isize; 2],
}

/// One band line of a precinct in packing order.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Line {
    pub band: usize,
    /// Its line inside the band's part of the precinct.
    pub y: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct Layout {
    pub comps: Vec<Comp>,
    pub bands: Vec<Band>,
    /// Band lines in packing order.
    pub lines: Vec<Line>,
    /// Packets: ranges of `lines`.
    pub packets: Vec<(usize, usize)>,
    /// `line_of[band][y]`: the index into `lines`.
    pub line_of: Vec<Vec<usize>>,
    /// `weight_of[band]`: the band's entry in the weights tables (WGT, WGR).
    pub weight_of: Vec<usize>,
    /// Precinct width (samples of the full grid), height (lines), columns and rows.
    pub cs: usize,
    pub ph: usize,
    pub npx: usize,
    pub npy: usize,
}

fn unsupported(s: impl Into<String>) -> Error {
    Error::Unsupported(s.into())
}

impl Layout {
    // Bands are indexed by component and filter type together: index loops read best.
    #[allow(clippy::needless_range_loop)]
    pub(crate) fn new(h: &Header) -> Result<Layout> {
        // Per-component wavelet levels belong to TicoRAW only; mixed headers are not decoded.
        if h.components.iter().any(|c| c.levels.is_some()) && !h.ticoraw() {
            return Err(unsupported(
                "per-component wavelet levels without a TicoRAW picture header",
            ));
        }
        let nc = h.components.len();
        let (nlx, nly) = (h.nlx, h.nly);
        if nlx == 0 || nlx > MAX_NLX {
            return Err(unsupported(format!(
                "{nlx} horizontal wavelet levels (1 to {MAX_NLX} are supported)"
            )));
        }
        if nly > nlx || nly > MAX_NLY {
            return Err(unsupported(format!(
                "{nly} vertical wavelet levels with {nlx} horizontal ones"
            )));
        }
        let sd = h.sd as usize;
        if sd >= nc {
            return Err(Error::Malformed(format!(
                "{sd} of {nc} components without a wavelet transform"
            )));
        }
        if h.components[0].sy != 1 {
            return Err(unsupported("the first component is subsampled vertically"));
        }
        if h.ng == 0 || h.ng > 32 || h.ss == 0 {
            return Err(Error::Malformed(format!(
                "code groups of {} coefficients, {} per significance group",
                h.ng, h.ss
            )));
        }
        let (w, ht) = h.component_grid();
        let mut comps = Vec::with_capacity(nc);
        for (i, c) in h.components.iter().enumerate() {
            let (sx, sy) = (c.sx as usize, c.sy as usize);
            let (cx, cy) = match c.levels {
                // TicoRAW: the component's own depths, within the picture header's.
                Some((x, y)) => {
                    if x > nlx || y > nly || y > x || (x == 0) != (y == 0) || sx != 1 || sy != 1 {
                        return Err(unsupported(format!(
                            "a component with {x} horizontal and {y} vertical wavelet levels"
                        )));
                    }
                    (x, y)
                }
                None if i < nc - sd => {
                    if sy == 2 && nly == 0 {
                        return Err(Error::Malformed(
                            "vertical subsampling without a vertical wavelet level".into(),
                        ));
                    }
                    (nlx, nly - (sy as u8 >> 1))
                }
                None => {
                    if sy != 1 {
                        return Err(unsupported(
                            "a vertically subsampled component without a wavelet transform",
                        ));
                    }
                    (0, 0)
                }
            };
            comps.push(Comp {
                w: w / sx,
                h: ht / sy,
                sx,
                sy,
                nlx: cx,
                nly: cy,
            });
        }
        // Components with and without a wavelet transform, each in index order.
        let wav: Vec<usize> = (0..nc).filter(|&c| comps[c].nlx > 0).collect();
        let flat: Vec<usize> = (0..nc).filter(|&c| comps[c].nlx == 0).collect();
        if wav.is_empty() {
            return Err(unsupported("no component has a wavelet transform"));
        }
        // Filter types: 0 is the low band; 1 ..= nlx - nly the horizontal-only high bands;
        // the last 3·nly the HL, LH, HH triples, deepest first.
        let nb = 2 * nly as usize + nlx as usize + 1;
        let first_triple = (nlx - nly) as usize + 1;
        let high = |beta: usize| -> (bool, bool) {
            if beta == 0 {
                (false, false)
            } else if beta < first_triple {
                (true, false)
            } else {
                match (beta - first_triple) % 3 {
                    0 => (true, false),
                    1 => (false, true),
                    _ => (true, true),
                }
            }
        };
        // Per component and filter type: the band's depths, or None when it does not exist.
        let depth = |c: &Comp, beta: usize| -> Option<(u8, u8)> {
            let (x, y) = (c.nlx as usize, c.nly as usize);
            if beta == 0 {
                Some((x as u8, y as u8))
            } else if beta <= x - y {
                Some(((x + 1 - beta) as u8, y as u8))
            } else if beta >= nb - 3 * y {
                let d = (nb - beta).div_ceil(3) as u8;
                Some((d, d))
            } else {
                None
            }
        };
        let cs = if h.cw > 0 {
            let max_sx = comps.iter().map(|c| c.sx).max().unwrap_or(1);
            (8 * h.cw as usize * max_sx) << nlx
        } else {
            w
        };
        if cs == 0 {
            return Err(Error::Malformed("empty image".into()));
        }
        let ph = 1usize << nly;
        let npx = w.div_ceil(cs);
        let npy = ht.div_ceil(ph);
        let pw = [cs, (w - 1) % cs + 1];
        let mut bands = Vec::new();
        let mut band_of = vec![vec![None; nb]; nc];
        let mut add = |c: usize, beta: usize, d: (u8, u8), comps: &[Comp]| {
            let comp = &comps[c];
            let (hx, hy) = high(beta);
            let size = |n: usize, d: u8, hi: bool| -> usize {
                if hi {
                    n.div_ceil(1 << (d - 1)) / 2
                } else {
                    n.div_ceil(1 << d)
                }
            };
            let bh = size(comp.h, d.1, hy);
            let sxs = comp.sx - 1;
            let pwb = [
                size((pw[0] + sxs) >> sxs, d.0, hx),
                size((pw[1] + sxs) >> sxs, d.0, hx),
            ];
            let lines = if comp.nlx > 0 {
                let per = 1isize << (comp.nly as isize - d.1 as isize).max(0);
                [per, bh as isize - (npy as isize - 1) * per]
            } else {
                [ph as isize, ((bh + ph - 1) % ph + 1) as isize]
            };
            band_of[c][beta] = Some(bands.len());
            bands.push(Band {
                c,
                dx: d.0,
                dy: d.1,
                high_x: hx,
                high_y: hy,
                pw: pwb,
                lines,
            });
        };
        for beta in 0..nb {
            for &c in &wav {
                if let Some(d) = depth(&comps[c], beta) {
                    add(c, beta, d, &comps);
                }
            }
        }
        for &c in &flat {
            add(c, 0, (0, 0), &comps);
        }
        if h.gains.len() != bands.len() || h.priorities.len() != bands.len() {
            return Err(Error::Malformed(format!(
                "the weights table has {} entries for {} bands",
                h.gains.len(),
                bands.len()
            )));
        }
        // Packing order (B.7): the low and horizontal-only bands in one packet; then per
        // triple and line, a packet for each of HL, LH and HH across components; then the
        // untransformed components line by line.
        let mut lines = Vec::new();
        let mut packet_of = Vec::new();
        let mut s = 0usize;
        for beta in 0..first_triple {
            for &c in &wav {
                let b = band_of[c][beta].ok_or_else(|| Error::Malformed("missing band".into()))?;
                lines.push(Line { band: b, y: 0 });
                packet_of.push(s);
            }
        }
        let mut beta0 = first_triple;
        while beta0 < nb {
            let b0 =
                band_of[wav[0]][beta0].ok_or_else(|| Error::Malformed("missing band".into()))?;
            let nlines = 1usize << (nly - bands[b0].dy);
            for lambda in 0..nlines {
                for beta in beta0..beta0 + 3 {
                    let mut first = true;
                    for &c in &wav {
                        if let Some(b) = band_of[c][beta]
                            && (lambda as isize) < bands[b].lines[0]
                        {
                            if first {
                                s += 1;
                                first = false;
                            }
                            lines.push(Line { band: b, y: lambda });
                            packet_of.push(s);
                        }
                    }
                }
            }
            beta0 += 3;
        }
        for lambda in 0..ph {
            for &c in &flat {
                let b = band_of[c][0].ok_or_else(|| Error::Malformed("missing band".into()))?;
                if (lambda as isize) < bands[b].lines[0] {
                    s += 1;
                    lines.push(Line { band: b, y: lambda });
                    packet_of.push(s);
                }
            }
        }
        if h.ticoraw() {
            // TicoRAW packs a precinct row by row, and each row component by component: one
            // packet holds what a component has on that row, its bands in filter-type order
            // (the low row: the low and horizontal bands, then HL; the high row: LH, HH; an
            // untransformed component: its line).
            if nly > 1 {
                return Err(unsupported(format!(
                    "TicoRAW with {nly} vertical wavelet levels"
                )));
            }
            lines.clear();
            packet_of.clear();
            s = 0;
            for row in 0..ph {
                for c in 0..nc {
                    let mut any = false;
                    for beta in 0..nb {
                        let Some(b) = band_of[c][beta] else { continue };
                        let band = &bands[b];
                        let y = if comps[c].nly == 0 {
                            row
                        } else if usize::from(band.high_y) == row {
                            0
                        } else {
                            continue;
                        };
                        if (y as isize) < band.lines[0] {
                            lines.push(Line { band: b, y });
                            packet_of.push(s);
                            any = true;
                        }
                    }
                    if any {
                        s += 1;
                    }
                }
            }
        }
        // Weights table entries follow the band order, except in TicoRAW, where they follow
        // the order the bands first appear in a precinct.
        let mut weight_of: Vec<usize> = (0..bands.len()).collect();
        if h.ticoraw() {
            let mut next = 0;
            weight_of.fill(usize::MAX);
            for l in &lines {
                if weight_of[l.band] == usize::MAX {
                    weight_of[l.band] = next;
                    next += 1;
                }
            }
            if next != bands.len() {
                return Err(Error::Malformed("a band missing from the precinct".into()));
            }
        }
        let mut packets = Vec::new();
        let mut start = 0;
        for i in 1..=lines.len() {
            if i == lines.len() || packet_of[i] != packet_of[start] {
                packets.push((start, i));
                start = i;
            }
        }
        let mut line_of = vec![Vec::new(); bands.len()];
        for (i, l) in lines.iter().enumerate() {
            let v = &mut line_of[l.band];
            if v.len() <= l.y {
                v.resize(l.y + 1, usize::MAX);
            }
            v[l.y] = i;
        }
        Ok(Layout {
            comps,
            bands,
            lines,
            packets,
            line_of,
            weight_of,
            cs,
            ph,
            npx,
            npy,
        })
    }

    /// Lines of band `b` in precinct row `py`.
    pub(crate) fn band_lines(&self, b: usize, py: usize) -> isize {
        self.bands[b].lines[usize::from(py + 1 == self.npy)]
    }

    /// Whether band line `idx` exists in precinct row `py`.
    pub(crate) fn included(&self, idx: usize, py: usize) -> bool {
        let l = self.lines[idx];
        (l.y as isize) < self.band_lines(l.band, py)
    }

    /// Coefficients in band line `idx` of a precinct in column `px`.
    pub(crate) fn width(&self, idx: usize, px: usize) -> usize {
        self.bands[self.lines[idx].band].pw[usize::from(px + 1 == self.npx)]
    }
}
