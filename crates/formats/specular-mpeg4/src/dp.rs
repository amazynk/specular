//! Data partitioning (ISO/IEC 14496-2 6.2.5, the `data_partitioned` VOL flag): in I- and
//! P-VOPs each video packet carries its macroblocks' headers and motion first, then a marker,
//! then their remaining headers (and P-VOP intra DC), then all their texture. The packet is
//! parsed partition by partition into per-macroblock records, then its macroblocks are
//! reconstructed in order exactly as unpartitioned ones are. B-VOPs are never partitioned.

use super::*;

/// dc_marker (I-VOPs, 19 bits) and motion_marker (P-VOPs, 17 bits).
const DC_MARKER: (u32, u32) = (0b110_1011_0000_0000_0001, 19);
const MOTION_MARKER: (u32, u32) = (0b1_1111_0000_0000_0001, 17);

/// One macroblock of a partitioned packet.
#[derive(Clone, Copy, Default)]
struct Rec {
    a: usize,
    not_coded: bool,
    intra: bool,
    /// mb_type (3 or 4 intra, 1 inter with dquant, 2 four vectors) and cbpc.
    ty: u8,
    cbpc: u8,
    mcsel: bool,
    ac_pred: bool,
    cbpy: u32,
    /// The quantiser of its texture.
    qp: i32,
    /// Intra DC differentials, when coded apart from the AC coefficients.
    dc: [Option<i32>; 6],
    m: MbInfo,
}

impl Dec<'_, '_> {
    pub(super) fn run_partitioned(&mut self, r: &mut Bits, out: &mut Frame) -> Result<(), Error> {
        let ctx = self.ctx;
        if ctx.vol.reversible_vlc {
            return Err(crate::unsupported("MPEG-4 reversible VLC (RVLC)"));
        }
        if ctx.vol.interlaced {
            return Err(crate::unsupported(
                "interlaced MPEG-4 with data partitioning",
            ));
        }
        let n = ctx.mb_w * ctx.mb_h;
        let mut a = 0usize;
        let mut packet = 0u32;
        let mut recs = Vec::new();
        while a < n {
            recs.clear();
            self.first_partition(r, a, packet, &mut recs)?;
            self.second_partition(r, &mut recs)?;
            for rec in &recs {
                self.texture(r, out, rec)?;
                self.decoded_upto = rec.a + 1;
            }
            if r.overrun() {
                return self.malformed("truncated VOP");
            }
            a = recs.last().map_or(a, |l| l.a + 1);
            if a >= n {
                break;
            }
            match self.resync(r)? {
                Some(at) if at == a => {
                    packet += 1;
                    continue_at(&mut self.packet, a, packet);
                }
                _ => return self.malformed("video packet macroblock number"),
            }
        }
        Ok(())
    }

    /// Partition 1: up to the marker, from macroblock `a` on.
    fn first_partition(
        &mut self,
        r: &mut Bits,
        mut a: usize,
        packet: u32,
        recs: &mut Vec<Rec>,
    ) -> Result<(), Error> {
        let ctx = self.ctx;
        let n = ctx.mb_w * ctx.mb_h;
        let i_vop = ctx.vop.kind == VopType::I;
        let (marker, len) = if i_vop { DC_MARKER } else { MOTION_MARKER };
        let gmc_vop = ctx.vop.kind == VopType::S;
        let fcode = ctx.vop.fcode_f;
        loop {
            if r.peek32() >> (32 - len) == marker {
                r.skip(len);
                return Ok(());
            }
            if a >= n || r.overrun() {
                return self.malformed("no marker after the first partition");
            }
            let (mbx, mby) = (a % ctx.mb_w, a / ctx.mb_w);
            self.packet[a] = packet;
            let mut rec = Rec {
                a,
                ..Rec::default()
            };
            if i_vop {
                let v = self.t.mcbpc_i.read(r).ok_or_else(|| self.err("mcbpc"))?;
                let (ty, cbpc) = ((v >> 4) as u8, (v & 3) as u8);
                if ty == 0xF {
                    continue;
                }
                (rec.ty, rec.cbpc, rec.intra) = (ty, cbpc, true);
                if ty == 4 {
                    self.dquant(r);
                }
                rec.qp = self.qp;
                if self.use_dc_vlc() {
                    for b in 0..6 {
                        rec.dc[b] = Some(self.dc_diff(r, b < 4)?);
                    }
                }
                self.info[a] = MbInfo {
                    intra: true,
                    ..MbInfo::default()
                };
            } else if r.flag() {
                rec.not_coded = true;
                let mv = if gmc_vop {
                    self.gmc_mv(mbx, mby)
                } else {
                    [0, 0]
                };
                rec.m = MbInfo {
                    skipped: !gmc_vop,
                    mv: [mv; 4],
                    ..MbInfo::default()
                };
                self.info[a] = rec.m;
            } else {
                let v = self.t.mcbpc_p.read(r).ok_or_else(|| self.err("mcbpc"))?;
                let (ty, cbpc) = ((v >> 4) as u8, (v & 3) as u8);
                if ty == 0xF {
                    continue;
                }
                (rec.ty, rec.cbpc) = (ty, cbpc);
                if ty >= 3 {
                    rec.intra = true;
                    self.info[a] = MbInfo {
                        intra: true,
                        ..MbInfo::default()
                    };
                } else {
                    rec.mcsel = gmc_vop && ty <= 1 && r.flag();
                    let mut m = MbInfo::default();
                    if rec.mcsel {
                        m.mv = [self.gmc_mv(mbx, mby); 4];
                    } else if ty == 2 {
                        m.four = true;
                        for b in 0..4 {
                            self.info[a].mv = m.mv;
                            self.info[a].intra = false;
                            let pred = self.predictor(a, mbx, mby, b);
                            m.mv[b] = self.mv(r, fcode, pred)?;
                        }
                    } else {
                        let pred = self.predictor(a, mbx, mby, 0);
                        m.mv = [self.mv(r, fcode, pred)?; 4];
                    }
                    rec.m = m;
                    self.info[a] = m;
                }
            }
            recs.push(rec);
            a += 1;
        }
    }

    /// Partition 2: ac_pred_flag and cbpy; in P-VOPs also dquant and the intra DC.
    fn second_partition(&mut self, r: &mut Bits, recs: &mut [Rec]) -> Result<(), Error> {
        let i_vop = self.ctx.vop.kind == VopType::I;
        for rec in recs.iter_mut() {
            if i_vop {
                rec.ac_pred = r.flag();
                rec.cbpy = self.t.cbpy.read(r).ok_or_else(|| self.err("cbpy"))? as u32;
                continue;
            }
            if rec.not_coded {
                rec.qp = self.qp;
                continue;
            }
            if rec.intra {
                rec.ac_pred = r.flag();
            }
            let cbpy = self.t.cbpy.read(r).ok_or_else(|| self.err("cbpy"))? as u32;
            rec.cbpy = if rec.intra { cbpy } else { 15 - cbpy };
            if rec.ty == 1 || rec.ty == 4 {
                self.dquant(r);
            }
            rec.qp = self.qp;
            if rec.intra && self.use_dc_vlc() {
                for b in 0..6 {
                    rec.dc[b] = Some(self.dc_diff(r, b < 4)?);
                }
            }
        }
        if r.overrun() {
            return self.malformed("truncated second partition");
        }
        Ok(())
    }

    /// Partition 3: the macroblock's texture, and its reconstruction.
    fn texture(&mut self, r: &mut Bits, out: &mut Frame, rec: &Rec) -> Result<(), Error> {
        let ctx = self.ctx;
        let a = rec.a;
        let (mbx, mby) = (a % ctx.mb_w, a / ctx.mb_w);
        self.qp = rec.qp;
        let mut y = [0u8; 256];
        let mut cb = [0u8; 64];
        let mut cr = [0u8; 64];
        let cbp = rec.cbpy << 2 | u32::from(rec.cbpc);
        if rec.intra {
            self.info[a] = MbInfo {
                intra: true,
                ..MbInfo::default()
            };
            for b in 0..6 {
                let mut blk = [0i32; 64];
                let coded = cbp & (1 << (5 - b)) != 0;
                self.intra_block(r, a, mbx, mby, b, coded, rec.ac_pred, rec.dc[b], &mut blk)?;
                idct(&mut blk);
                put_block(&mut y, &mut cb, &mut cr, b, false, &blk, true);
            }
        } else {
            self.info[a] = rec.m;
            self.clear_intra(mbx, mby);
            if rec.mcsel || (rec.not_coded && ctx.vop.kind == VopType::S) {
                self.gmc_predict(mbx, mby, &mut y, &mut cb, &mut cr);
            } else {
                self.predict_p(&rec.m, mbx, mby, &mut y, &mut cb, &mut cr);
            }
            if !rec.not_coded {
                self.residual(r, cbp, false, &mut y, &mut cb, &mut cr)?;
            }
        }
        store(out, mbx, mby, &y, &cb, &cr);
        Ok(())
    }
}
