//! Tier-2: tile body streams and progression orders (B.12).

use std::ops::ControlFlow;

use crate::markers::{Poc, Progression};

/// A tile's packet data: the concatenation of its tile-parts' bodies. Packets never straddle
/// tile-parts in a valid stream; data that would is truncated at the part boundary.
pub(crate) struct Body<'a> {
    parts: Vec<&'a [u8]>,
    idx: usize,
    pub pos: usize,
}

impl<'a> Body<'a> {
    pub fn new(parts: Vec<&'a [u8]>) -> Body<'a> {
        Body {
            parts,
            idx: 0,
            pos: 0,
        }
    }

    fn settle(&mut self) {
        while self.idx < self.parts.len() && self.pos >= self.parts[self.idx].len() {
            self.idx += 1;
            self.pos = 0;
        }
    }

    /// The current tile-part slice (at a packet start), or `None` when exhausted.
    pub fn current(&mut self) -> Option<&'a [u8]> {
        self.settle();
        self.parts.get(self.idx).copied()
    }

    /// Skip an SOP marker segment if one starts here.
    pub fn skip_sop(&mut self) {
        if let Some(p) = self.current()
            && p.len() >= self.pos + 6
            && p[self.pos] == 0xFF
            && p[self.pos + 1] == 0x91
        {
            self.pos += 6;
        }
    }

    /// Take `n` bytes of code-block data; the flag is set if fewer were available.
    pub fn take(&mut self, n: usize) -> (&'a [u8], bool) {
        let Some(p) = self.current() else {
            return (&[], n > 0);
        };
        let s = self.pos.min(p.len());
        let e = s.saturating_add(n).min(p.len());
        self.pos = s + n;
        if self.pos > p.len() {
            self.pos = p.len();
        }
        (&p[s..e], e - s < n)
    }
}

/// Precinct grid of one (component, resolution), for progression iteration.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ResGrid {
    /// Number of precincts (`pw * ph`); 0 for an empty resolution.
    pub pw: u32,
    pub ph: u32,
    /// Index of the first precinct in the tile-wide precinct arrays.
    pub base: usize,
    /// Position keys of precinct (i, j): `x_key(i)` / `y_key(j)` in reference-grid units.
    pub x0_key: u64,
    pub x_step: u64,
    pub x_first: u64,
    pub y0_key: u64,
    pub y_step: u64,
    pub y_first: u64,
}

impl ResGrid {
    fn x_key(&self, i: u32) -> u64 {
        if i == 0 {
            self.x_first
        } else {
            self.x0_key + i as u64 * self.x_step
        }
    }
    fn y_key(&self, j: u32) -> u64 {
        if j == 0 {
            self.y_first
        } else {
            self.y0_key + j as u64 * self.y_step
        }
    }
    pub fn count(&self) -> usize {
        self.pw as usize * self.ph as usize
    }
}

/// Visit packets `(component, resolution, precinct index within the resolution, layer)` in
/// codestream order. `grids[c][r]`; `next` holds the next expected layer per precinct (tile-wide
/// index) and implements the "already included" rule of progression order changes.
pub(crate) fn for_each_packet(
    grids: &[Vec<ResGrid>],
    layers: u16,
    default_prog: Progression,
    pocs: &[Poc],
    next: &mut [u16],
    mut f: impl FnMut(usize, usize, usize, u16) -> ControlFlow<()>,
) -> ControlFlow<()> {
    let nc = grids.len();
    let max_res = grids.iter().map(|g| g.len()).max().unwrap_or(0);
    let default = [Poc {
        rs: 0,
        cs: 0,
        lye: layers,
        re: max_res.min(255) as u8,
        ce: nc.min(u16::MAX as usize) as u16,
        prog: default_prog,
    }];
    let list: &[Poc] = if pocs.is_empty() { &default } else { pocs };
    for poc in list {
        let lye = poc.lye.min(layers);
        let cs = poc.cs as usize;
        let ce = (poc.ce as usize).min(nc);
        let rs = poc.rs as usize;
        let re = (poc.re as usize).min(max_res);
        if cs >= ce || rs >= re || lye == 0 {
            continue;
        }
        let mut emit = |c: usize, r: usize, p: usize, l: u16| -> ControlFlow<()> {
            let g = &grids[c][r];
            let k = g.base + p;
            if next[k] == l {
                next[k] = l + 1;
                f(c, r, p, l)
            } else {
                ControlFlow::Continue(())
            }
        };
        match poc.prog {
            Progression::Lrcp => {
                for l in 0..lye {
                    for r in rs..re {
                        #[allow(clippy::needless_range_loop)]
                        // the index also feeds closures/other arrays
                        #[allow(clippy::needless_range_loop)]
                        // the index also feeds closures/other arrays
                        for c in cs..ce {
                            if let Some(g) = grids[c].get(r) {
                                for p in 0..g.count() {
                                    emit(c, r, p, l)?;
                                }
                            }
                        }
                    }
                }
            }
            Progression::Rlcp => {
                for r in rs..re {
                    for l in 0..lye {
                        #[allow(clippy::needless_range_loop)]
                        // the index also feeds closures/other arrays
                        for c in cs..ce {
                            if let Some(g) = grids[c].get(r) {
                                for p in 0..g.count() {
                                    emit(c, r, p, l)?;
                                }
                            }
                        }
                    }
                }
            }
            prog => {
                // Position-driven orders: sort all precincts by their keys.
                let mut items: Vec<(u64, u64, u64, u64, u32)> = Vec::new();
                for (c, gc) in grids.iter().enumerate().take(ce).skip(cs) {
                    for (r, g) in gc.iter().enumerate().take(re).skip(rs) {
                        for j in 0..g.ph {
                            let y = g.y_key(j);
                            for i in 0..g.pw {
                                let x = g.x_key(i);
                                let p = (j * g.pw + i) as u64;
                                let (c, r) = (c as u64, r as u64);
                                let key = match prog {
                                    Progression::Rpcl => (r, y, x, c),
                                    Progression::Pcrl => (y, x, c, r),
                                    _ => (c, y, x, r),
                                };
                                items.push((key.0, key.1, key.2, key.3, p as u32));
                            }
                        }
                    }
                }
                items.sort_unstable();
                for &(k0, k1, k2, k3, p) in &items {
                    let (c, r) = match prog {
                        Progression::Rpcl => (k3, k0),
                        Progression::Pcrl => (k2, k3),
                        _ => (k0, k3),
                    };
                    for l in 0..lye {
                        emit(c as usize, r as usize, p as usize, l)?;
                    }
                    let _ = (k1, k2);
                }
            }
        }
    }
    ControlFlow::Continue(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(pw: u32, ph: u32, base: usize) -> ResGrid {
        ResGrid {
            pw,
            ph,
            base,
            x0_key: 0,
            x_step: 10,
            x_first: 0,
            y0_key: 0,
            y_step: 10,
            y_first: 0,
        }
    }

    #[test]
    fn lrcp_and_rpcl_visit_all_once() {
        let grids = vec![
            vec![grid(1, 1, 0), grid(2, 1, 1)],
            vec![grid(1, 1, 3), grid(2, 1, 4)],
        ];
        for prog in [
            Progression::Lrcp,
            Progression::Rlcp,
            Progression::Rpcl,
            Progression::Pcrl,
            Progression::Cprl,
        ] {
            let mut next = vec![0u16; 6];
            let mut seen = Vec::new();
            let _ = for_each_packet(&grids, 2, prog, &[], &mut next, |c, r, p, l| {
                seen.push((c, r, p, l));
                ControlFlow::Continue(())
            });
            assert_eq!(seen.len(), 12, "{prog:?}");
            let mut s = seen.clone();
            s.sort();
            s.dedup();
            assert_eq!(s.len(), 12);
        }
    }
}
