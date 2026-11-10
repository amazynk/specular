//! The macroblock layer of MPEG-4 Part 2 (ISO/IEC 14496-2 §6.2.6–§6.2.8, §7.4–§7.6) and of
//! H.263 baseline: macroblock types, motion vectors and their prediction, intra DC/AC
//! prediction, H.263 and MPEG inverse quantisation, the inverse DCT, and the prediction of
//! P-, S- (global motion) and B-VOPs (direct, forward, backward, interpolated), frame or field
//! based, at half or quarter sample accuracy. Video packets (resync markers) end the
//! prediction from macroblocks before them.

use specular_mpeg2::util::{Bits, idct};

use crate::Error;
use crate::headers::{Vol, Vop, VopType};
use crate::mc::{Frame, average, chroma_of, half, quarter, round_chroma_4};
use crate::tables::{ALT_HORIZONTAL, ALT_VERTICAL, ZIGZAG, dc_scaler};
use crate::vlc::{ESCAPE, Tables, tables};

/// What a decoded macroblock leaves for the prediction of later ones (and of B-VOPs that
/// take it as their co-located macroblock).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct MbInfo {
    pub intra: bool,
    /// Not coded (skipped) in a P- or S-VOP.
    pub skipped: bool,
    /// Four vectors (blocks 0..3); one vector repeated otherwise. Field vectors of a
    /// field-predicted macroblock are in `field_mv`.
    pub mv: [[i32; 2]; 4],
    pub field: bool,
    pub field_mv: [[i32; 2]; 2],
    pub field_sel: [u8; 2],
    pub four: bool,
}

/// Per 8×8 block: the reconstructed DC and the first row and column of quantised levels
/// (for intra prediction), the block's quantiser, and whether it is intra.
#[derive(Clone, Copy, Default)]
struct Pred {
    dc: i32,
    row: [i32; 7],
    col: [i32; 7],
    qp: i32,
    intra: bool,
}

pub(crate) struct Ctx<'a> {
    pub vol: &'a Vol,
    pub vop: &'a Vop,
    pub mb_w: usize,
    pub mb_h: usize,
    /// P/S: the reference; B: the past reference.
    pub fwd: Option<&'a Frame>,
    /// B: the future reference.
    pub bwd: Option<&'a Frame>,
    /// B: the macroblocks of the future reference.
    pub colocated: &'a [MbInfo],
    /// B: time from the past reference to this VOP, and between the references.
    pub trb: i32,
    pub trd: i32,
    /// The same in field periods (for field-predicted co-located macroblocks).
    pub trb_field: i32,
    pub trd_field: i32,
    pub gmc: Option<crate::gmc::Sprite>,
}

/// Decodes the macroblocks of a VOP into `out`; returns its macroblock info.
#[path = "dp.rs"]
mod dp;

pub(crate) fn decode_vop(ctx: &Ctx, data: &[u8], out: &mut Frame) -> Result<Vec<MbInfo>, Error> {
    let mut d = Dec::new(ctx);
    let mut r = Bits::new(data);
    r.skip_bits(ctx.vop.header_bits);
    let result = d.run(&mut r, out);
    // A damaged VOP keeps what was decoded; the rest comes from the reference.
    if result.is_err() {
        d.conceal(out);
    }
    result.map(|()| d.info)
}

struct Dec<'a, 'b> {
    ctx: &'b Ctx<'a>,
    t: &'static Tables,
    info: Vec<MbInfo>,
    /// Luma blocks on a (2·mb_w)×(2·mb_h) grid, then chroma blocks per macroblock.
    pred_y: Vec<Pred>,
    pred_c: [Vec<Pred>; 2],
    /// The video packet each macroblock is in (u32::MAX: not yet decoded).
    packet: Vec<u32>,
    qp: i32,
    /// B-VOPs: the last forward and backward vectors of the macroblock row (and field).
    b_pred: [[i32; 2]; 2],
    b_pred_field: [[[i32; 2]; 2]; 2],
    decoded_upto: usize,
}

const UNSET: u32 = u32::MAX;

impl<'a, 'b> Dec<'a, 'b> {
    fn new(ctx: &'b Ctx<'a>) -> Dec<'a, 'b> {
        let n = ctx.mb_w * ctx.mb_h;
        Dec {
            ctx,
            t: tables(),
            info: vec![MbInfo::default(); n],
            pred_y: vec![Pred::default(); 4 * n],
            pred_c: [vec![Pred::default(); n], vec![Pred::default(); n]],
            packet: vec![UNSET; n],
            qp: ctx.vop.quant,
            b_pred: [[0; 2]; 2],
            b_pred_field: [[[0; 2]; 2]; 2],
            decoded_upto: 0,
        }
    }

    fn malformed<T>(&self, what: &str) -> Result<T, Error> {
        Err(Error::Malformed(format!(
            "{what} at macroblock {}",
            self.decoded_upto
        )))
    }

    fn run(&mut self, r: &mut Bits, out: &mut Frame) -> Result<(), Error> {
        let ctx = self.ctx;
        if ctx.vol.data_partitioned && ctx.vop.kind != VopType::B && !ctx.vop.short_header {
            return self.run_partitioned(r, out);
        }
        let n = ctx.mb_w * ctx.mb_h;
        let mut packet = 0u32;
        let mut a = 0usize;
        let mut gob_rows = 1usize;
        if ctx.vop.short_header {
            gob_rows = if ctx.vol.height <= 400 { 1 } else { 2 };
        }
        while a < n {
            let (mbx, mby) = (a % ctx.mb_w, a / ctx.mb_w);
            if mbx == 0 && ctx.vop.kind == VopType::B {
                self.b_pred = [[0; 2]; 2];
                self.b_pred_field = [[[0; 2]; 2]; 2];
            }
            // A new video packet (or H.263 GOB) at this macroblock.
            if a > 0 {
                if ctx.vop.short_header {
                    // Sorenson Spark has no GOB layer.
                    if ctx.vol.sorenson.is_none()
                        && mbx == 0
                        && mby.is_multiple_of(gob_rows)
                        && self.gob_header(r)?
                    {
                        packet += 1;
                    }
                } else if !ctx.vol.resync_disable
                    && let Some(at) = self.resync(r)?
                {
                    packet += 1;
                    if at < a || at >= n {
                        return self.malformed("video packet macroblock number");
                    }
                    if at > a {
                        // B-VOP macroblocks whose co-located macroblock was not coded carry no
                        // bits, so a video packet may start after them (FFmpeg's encoder puts
                        // one there): they belong to the packet before and are decoded as
                        // such. They were jumped over, left black.
                        let skipped = |k: usize| ctx.colocated.get(k).is_some_and(|c| c.skipped);
                        if ctx.vop.kind != VopType::B || !(a..at).all(skipped) {
                            return self.malformed("video packet macroblock number");
                        }
                        for k in a..at {
                            self.packet[k] = packet - 1;
                            self.b_mb(r, out, k, k % ctx.mb_w, k / ctx.mb_w)?;
                            self.decoded_upto = k + 1;
                        }
                    }
                    a = at;
                    if ctx.vop.kind == VopType::B {
                        self.b_pred = [[0; 2]; 2];
                        self.b_pred_field = [[[0; 2]; 2]; 2];
                    }
                    continue_at(&mut self.packet, a, packet);
                }
            }
            let (mbx, mby) = (a % ctx.mb_w, a / ctx.mb_w);
            self.packet[a] = packet;
            match ctx.vop.kind {
                VopType::I => {
                    if !self.intra_mb(r, out, a, mbx, mby, None)? {
                        continue;
                    }
                }
                VopType::P | VopType::S => {
                    if !self.p_mb(r, out, a, mbx, mby)? {
                        continue;
                    }
                }
                VopType::B => self.b_mb(r, out, a, mbx, mby)?,
            }
            if r.overrun() {
                return self.malformed("truncated VOP");
            }
            a += 1;
            self.decoded_upto = a;
        }
        Ok(())
    }

    /// Macroblocks never decoded (a damaged VOP): copied from the reference.
    fn conceal(&mut self, out: &mut Frame) {
        let ctx = self.ctx;
        for a in self.decoded_upto..ctx.mb_w * ctx.mb_h {
            let (mbx, mby) = (a % ctx.mb_w, a / ctx.mb_w);
            if let Some(f) = ctx.fwd.or(ctx.bwd) {
                for c in 0..3 {
                    let (bs, s) = if c == 0 {
                        (16, out.stride[0])
                    } else {
                        (8, out.stride[c])
                    };
                    for i in 0..bs {
                        let o = (mby * bs + i) * s + mbx * bs;
                        out.planes[c][o..o + bs].copy_from_slice(&f.planes[c][o..o + bs]);
                    }
                }
            }
            self.info[a].skipped = true;
        }
    }

    /// An H.263 GOB header here (at the start of a GOB row): consumed, its quantiser taken.
    fn gob_header(&mut self, r: &mut Bits) -> Result<bool, Error> {
        // Zero stuffing to the byte boundary may come before the GBSC (17 bits, 0..01).
        let pos = r.position();
        let k = ((8 - pos % 8) % 8) as u32;
        let at = [0u32, k].into_iter().find(|&s| {
            let mut p = r.clone();
            p.skip(s);
            (s == 0 || r.peek32() >> (32 - s) == 0) && p.peek32() >> 15 == 1
        });
        let Some(stuffing) = at else {
            return Ok(false);
        };
        r.skip(stuffing);
        r.skip(17);
        let _gn = r.u(5);
        // GFID; GQUANT.
        r.skip(2);
        self.qp = r.u(5).max(1) as i32;
        Ok(true)
    }

    /// A resync marker here: the video packet header is consumed and its first macroblock
    /// returned.
    fn resync(&mut self, r: &mut Bits) -> Result<Option<usize>, Error> {
        let ctx = self.ctx;
        let pos = r.position();
        // Stuffing: a 0 then 1s to the byte boundary (a whole byte when aligned).
        let k = 8 - (pos % 8) as u32;
        let stuffing = (1u32 << (k - 1)) - 1;
        let p = r.peek32();
        if p >> (32 - k) != stuffing {
            return Ok(None);
        }
        let zeros = match ctx.vop.kind {
            VopType::I => 16,
            VopType::P | VopType::S => 15 + u32::from(ctx.vop.fcode_f),
            VopType::B => 15 + u32::from(ctx.vop.fcode_f.max(ctx.vop.fcode_b).max(2)),
        };
        // Look past the stuffing for the marker.
        let mut probe = r.clone();
        probe.skip(k);
        if probe.u(zeros) != 0 || probe.u(1) != 1 {
            return Ok(None);
        }
        r.skip(k);
        r.skip(zeros);
        r.skip(1);
        let n = (ctx.mb_w * ctx.mb_h) as u32;
        let bits = 32 - (n - 1).leading_zeros();
        let mb = r.u(bits.max(1)) as usize;
        self.qp = r.u(5).max(1) as i32;
        if r.flag() {
            // The header extension: the VOP's time and coding repeated.
            while r.flag() {}
            r.skip(1);
            r.skip(ctx.vol.time_bits);
            r.skip(1);
            let _kind = r.u(2);
            let _thr = r.u(3);
            if ctx.vop.kind != VopType::I {
                r.skip(3);
            }
            if ctx.vop.kind == VopType::B {
                r.skip(3);
            }
        }
        Ok(Some(mb))
    }

    fn dquant(&mut self, r: &mut Bits) {
        const D: [i32; 4] = [-1, -2, 1, 2];
        self.qp = (self.qp + D[r.u(2) as usize]).clamp(1, 31);
    }

    /// Whether macroblock `b` (index) is available to predict from for macroblock `a`: in the
    /// VOP, decoded, and in the same video packet.
    fn usable(&self, a: usize, mbx: i32, mby: i32) -> bool {
        let ctx = self.ctx;
        if mbx < 0 || mby < 0 || mbx >= ctx.mb_w as i32 || mby >= ctx.mb_h as i32 {
            return false;
        }
        let b = mby as usize * ctx.mb_w + mbx as usize;
        b < a && self.packet[b] == self.packet[a]
    }

    // -----------------------------------------------------------------------------------
    // Intra macroblocks

    /// An I-VOP macroblock (or, with `ptype`, a P-VOP's intra macroblock whose mcbpc was
    /// read). Returns false for stuffing.
    fn intra_mb(
        &mut self,
        r: &mut Bits,
        out: &mut Frame,
        a: usize,
        mbx: usize,
        mby: usize,
        ptype: Option<(u8, u8)>,
    ) -> Result<bool, Error> {
        let ctx = self.ctx;
        let (ty, cbpc) = match ptype {
            Some(p) => p,
            None => {
                let v = self.t.mcbpc_i.read(r).ok_or_else(|| self.err("mcbpc"))?;
                let (ty, cbpc) = ((v >> 4) as u8, (v & 3) as u8);
                if ty == 0xF {
                    return Ok(false);
                }
                (ty, cbpc)
            }
        };
        let ac_pred = !ctx.vop.short_header && r.flag();
        let cbpy = self.t.cbpy.read(r).ok_or_else(|| self.err("cbpy"))? as u32;
        if ty == 4 {
            self.dquant(r);
        }
        let field_dct = ctx.vol.interlaced && r.flag();
        let cbp = cbpy << 2 | u32::from(cbpc);
        self.info[a] = MbInfo {
            intra: true,
            ..MbInfo::default()
        };
        let mut y = [0u8; 256];
        let mut cb = [0u8; 64];
        let mut cr = [0u8; 64];
        for b in 0..6 {
            let mut blk = [0i32; 64];
            let coded = cbp & (1 << (5 - b)) != 0;
            self.intra_block(r, a, mbx, mby, b, coded, ac_pred, None, &mut blk)?;
            idct(&mut blk);
            put_block(&mut y, &mut cb, &mut cr, b, field_dct, &blk, true);
        }
        store(out, mbx, mby, &y, &cb, &cr);
        Ok(true)
    }

    fn err(&self, what: &str) -> Error {
        Error::Malformed(format!("{what} at macroblock {}", self.decoded_upto))
    }

    /// The prediction neighbours of block `b`: left (A), above-left (B), above (C).
    fn neighbours(&self, a: usize, mbx: usize, mby: usize, b: usize) -> [Option<Pred>; 3] {
        let ctx = self.ctx;
        let (mx, my) = (mbx as i32, mby as i32);
        let get = |dx: i32, dy: i32| -> Option<Pred> {
            if b < 4 {
                let bx = 2 * mx + (b & 1) as i32 + dx;
                let by = 2 * my + (b >> 1) as i32 + dy;
                // The macroblock holding that block.
                let (nmx, nmy) = (bx.div_euclid(2), by.div_euclid(2));
                let same = nmx == mx && nmy == my;
                if !same && !self.usable(a, nmx, nmy) {
                    return None;
                }
                if bx < 0 || by < 0 {
                    return None;
                }
                let p = self.pred_y[by as usize * 2 * ctx.mb_w + bx as usize];
                p.intra.then_some(p)
            } else {
                let (nmx, nmy) = (mx + dx, my + dy);
                if !self.usable(a, nmx, nmy) {
                    return None;
                }
                let p = self.pred_c[b - 4][nmy as usize * ctx.mb_w + nmx as usize];
                p.intra.then_some(p)
            }
        };
        [get(-1, 0), get(-1, -1), get(0, -1)]
    }

    /// An intra block: DC and AC coefficients, their prediction, inverse quantisation.
    #[allow(clippy::too_many_arguments)]
    fn intra_block(
        &mut self,
        r: &mut Bits,
        a: usize,
        mbx: usize,
        mby: usize,
        b: usize,
        coded: bool,
        ac_pred: bool,
        dc_diff: Option<i32>,
        blk: &mut [i32; 64],
    ) -> Result<(), Error> {
        let ctx = self.ctx;
        let qp = self.qp;
        let luma = b < 4;
        let mut qf = [0i32; 64];
        if ctx.vop.short_header {
            // INTRADC: 8 bits (255 means 128); the AC with the inter table.
            let dc = r.u(8) as i32;
            if dc == 0 || dc == 128 {
                return Err(self.err("INTRADC"));
            }
            blk[0] = if dc == 255 { 1024 } else { dc * 8 };
            if coded {
                self.coefficients(r, &mut qf, 1, &ZIGZAG, false, true)?;
            }
            for i in 1..64 {
                blk[i] = h263_dequant(qf[i], qp);
            }
            return Ok(());
        }
        let scaler = dc_scaler(qp, luma);
        let [na, nb, nc] = self.neighbours(a, mbx, mby, b);
        let dc_of = |p: Option<Pred>| p.map_or(1024, |p| p.dc);
        let (fa, fb, fc) = (dc_of(na), dc_of(nb), dc_of(nc));
        // Predict from above (C) when the horizontal gradient is the smaller.
        let vertical = (fa - fb).abs() < (fb - fc).abs();
        let scan: &[u8; 64] = if ac_pred {
            if vertical {
                &ALT_HORIZONTAL
            } else {
                &ALT_VERTICAL
            }
        } else if ctx.vop.alternate_scan {
            &ALT_VERTICAL
        } else {
            &ZIGZAG
        };
        let mut start = 0;
        if let Some(d) = dc_diff {
            // Data partitioning: read with the macroblock's header.
            qf[0] = d;
            start = 1;
        } else if self.use_dc_vlc() {
            qf[0] = self.dc_diff(r, luma)?;
            start = 1;
        }
        if coded {
            self.coefficients(r, &mut qf, start, scan, true, false)?;
        }
        // DC prediction: the neighbour's DC divided by this block's scaler, rounded.
        let pred_dc = if vertical { fc } else { fa };
        qf[0] += (pred_dc + scaler / 2) / scaler;
        let dc = qf[0] * scaler;
        // AC prediction: the neighbour's first row (above) or column (left), rescaled.
        let rescale = |v: i32, from: i32| {
            if from == qp || v == 0 {
                v
            } else {
                let p = v * from;
                if p > 0 {
                    (p + qp / 2) / qp
                } else {
                    (p - qp / 2) / qp
                }
            }
        };
        if ac_pred {
            if vertical {
                if let Some(c) = nc {
                    for i in 0..7 {
                        qf[i + 1] += rescale(c.row[i], c.qp);
                    }
                }
            } else if let Some(l) = na {
                for i in 0..7 {
                    qf[(i + 1) * 8] += rescale(l.col[i], l.qp);
                }
            }
        }
        let mut p = Pred {
            dc,
            qp,
            intra: true,
            ..Pred::default()
        };
        for i in 0..7 {
            p.row[i] = qf[i + 1];
            p.col[i] = qf[(i + 1) * 8];
        }
        if luma {
            let bx = 2 * mbx + (b & 1);
            let by = 2 * mby + (b >> 1);
            self.pred_y[by * 2 * ctx.mb_w + bx] = p;
        } else {
            self.pred_c[b - 4][mby * ctx.mb_w + mbx] = p;
        }
        blk[0] = dc.clamp(-2048, 2047);
        if ctx.vol.mpeg_quant {
            let m = &ctx.vol.intra_matrix;
            let mut sum = blk[0];
            for i in 1..64 {
                if qf[i] != 0 {
                    let v = (qf[i] * i32::from(m[i]) * qp / 8).clamp(-2048, 2047);
                    blk[i] = v;
                    sum += v;
                }
            }
            if sum & 1 == 0 {
                blk[63] ^= 1;
            }
        } else {
            for i in 1..64 {
                blk[i] = h263_dequant(qf[i], qp);
            }
        }
        Ok(())
    }

    /// Whether intra DC coefficients have their own code (intra_dc_vlc_thr) at this
    /// quantiser.
    pub(crate) fn use_dc_vlc(&self) -> bool {
        self.qp < i32::from(DC_THR[usize::from(self.ctx.vop.intra_dc_thr)])
    }

    /// An intra DC differential (dct_dc_size, dct_dc_differential, a marker after 9+ bits).
    pub(crate) fn dc_diff(&self, r: &mut Bits, luma: bool) -> Result<i32, Error> {
        let t = if luma {
            &self.t.dc_luma
        } else {
            &self.t.dc_chroma
        };
        let size = u32::from(t.read(r).ok_or_else(|| self.err("dct_dc_size"))?);
        let diff = if size == 0 {
            0
        } else {
            let v = r.u(size) as i32;
            if v >> (size - 1) & 1 == 1 {
                v
            } else {
                v - (1 << size) + 1
            }
        };
        if size > 8 {
            r.skip(1);
        }
        Ok(diff)
    }

    /// Coefficients from scan position `start` into `qf` (raster order, quantised levels).
    fn coefficients(
        &self,
        r: &mut Bits,
        qf: &mut [i32; 64],
        start: usize,
        scan: &[u8; 64],
        intra: bool,
        h263: bool,
    ) -> Result<(), Error> {
        let t = self.t;
        let table = if intra && !h263 { &t.intra } else { &t.inter };
        let (lmax, rmax) = if intra && !h263 {
            (&t.intra_lmax, &t.intra_rmax)
        } else {
            (&t.inter_lmax, &t.inter_rmax)
        };
        let mut i = start;
        loop {
            let v = table.read(r).ok_or_else(|| self.err("TCOEF"))?;
            let (last, run, level);
            if v == ESCAPE {
                if self.ctx.vol.sorenson == Some(1) {
                    // Sorenson Spark version 1: a format bit, LAST, RUN and a 7- or 11-bit
                    // two's complement level.
                    let wide = r.flag();
                    last = r.flag();
                    run = r.u(6) as usize;
                    let n = if wide { 11 } else { 7 };
                    let l = r.u(n) as i32;
                    level = if l >= 1 << (n - 1) { l - (1 << n) } else { l };
                    if level == 0 || level == -(1 << (n - 1)) {
                        return Err(self.err("TCOEF escape level"));
                    }
                } else if h263 || self.ctx.vop.short_header {
                    last = r.flag();
                    run = r.u(6) as usize;
                    let l = r.u(8) as i32;
                    if l == 0 || l == 128 {
                        return Err(self.err("TCOEF escape level"));
                    }
                    level = if l > 128 { l - 256 } else { l };
                } else if !r.flag() {
                    // Type 1: the level beyond the table's largest for the run.
                    let w = table.read(r).ok_or_else(|| self.err("TCOEF"))?;
                    let (l, ru, lv) = unpack(w);
                    let mag = lv + i32::from(lmax[usize::from(l)][ru]);
                    last = l;
                    run = ru;
                    level = if r.flag() { -mag } else { mag };
                } else if !r.flag() {
                    // Type 2: the run beyond the table's longest for the level.
                    let w = table.read(r).ok_or_else(|| self.err("TCOEF"))?;
                    let (l, ru, lv) = unpack(w);
                    last = l;
                    run = ru + usize::from(rmax[usize::from(l)][lv as usize]) + 1;
                    level = if r.flag() { -lv } else { lv };
                } else {
                    // Type 3: fixed length.
                    last = r.flag();
                    run = r.u(6) as usize;
                    r.skip(1);
                    let l = r.u(12) as i32;
                    r.skip(1);
                    level = if l >= 2048 { l - 4096 } else { l };
                    if level == 0 {
                        return Err(self.err("TCOEF escape level"));
                    }
                }
            } else {
                let (l, ru, lv) = unpack(v);
                last = l;
                run = ru;
                level = if r.flag() { -lv } else { lv };
            }
            i += run;
            if i > 63 {
                return Err(self.err("TCOEF run"));
            }
            qf[usize::from(scan[i])] = level;
            i += 1;
            if last {
                return Ok(());
            }
        }
    }

    // -----------------------------------------------------------------------------------
    // Motion vectors

    /// The vector of block grid position (`bx`, `by`) for prediction, when usable.
    fn mv_at(&self, a: usize, bx: i32, by: i32) -> Option<[i32; 2]> {
        let ctx = self.ctx;
        let (mx, my) = (bx.div_euclid(2), by.div_euclid(2));
        let cur = (a % ctx.mb_w) as i32 == mx && (a / ctx.mb_w) as i32 == my;
        if !cur && !self.usable(a, mx, my) {
            return None;
        }
        if bx < 0 || by < 0 || bx >= 2 * ctx.mb_w as i32 {
            return None;
        }
        let m = &self.info[my as usize * ctx.mb_w + mx as usize];
        if m.intra && !cur {
            return Some([0, 0]);
        }
        Some(m.mv[((by & 1) * 2 + (bx & 1)) as usize])
    }

    /// The median predictor of block `blk` of macroblock `a` (§7.6.5).
    fn predictor(&self, a: usize, mbx: usize, mby: usize, blk: usize) -> [i32; 2] {
        const C_OFF: [i32; 4] = [2, 1, 1, -1];
        let bx = 2 * mbx as i32 + (blk & 1) as i32;
        let by = 2 * mby as i32 + (blk >> 1) as i32;
        let cands = [
            self.mv_at(a, bx - 1, by),
            self.mv_at(a, bx, by - 1),
            self.mv_at(a, bx + C_OFF[blk], by - 1),
        ];
        let valid: Vec<[i32; 2]> = cands.iter().flatten().copied().collect();
        match valid.len() {
            0 => [0, 0],
            1 => valid[0],
            _ => {
                let c = cands.map(|c| c.unwrap_or([0, 0]));
                [
                    median(c[0][0], c[1][0], c[2][0]),
                    median(c[0][1], c[1][1], c[2][1]),
                ]
            }
        }
    }

    /// One motion vector component: motion_code and residual, added to `pred`, wrapped to
    /// the f_code's range.
    fn mv_component(&self, r: &mut Bits, fcode: u8, pred: i32) -> Result<i32, Error> {
        let code = i32::from(
            self.t
                .motion
                .read(r)
                .ok_or_else(|| self.err("motion_code"))?,
        );
        if code == 0 {
            return Ok(pred);
        }
        let neg = r.flag();
        let shift = u32::from(fcode) - 1;
        let mut v = code;
        if shift > 0 {
            v = ((v - 1) << shift | r.u(shift) as i32) + 1;
        }
        if neg {
            v = -v;
        }
        v += pred;
        // Wrap into [-16·2^fcode, 16·2^fcode) units of the vector.
        let bits = 5 + u32::from(fcode);
        let m = 1i32 << bits;
        Ok(((v + m / 2).rem_euclid(m)) - m / 2)
    }

    fn mv(&self, r: &mut Bits, fcode: u8, pred: [i32; 2]) -> Result<[i32; 2], Error> {
        Ok([
            self.mv_component(r, fcode, pred[0])?,
            self.mv_component(r, fcode, pred[1])?,
        ])
    }

    // -----------------------------------------------------------------------------------
    // P- and S-VOPs

    fn p_mb(
        &mut self,
        r: &mut Bits,
        out: &mut Frame,
        a: usize,
        mbx: usize,
        mby: usize,
    ) -> Result<bool, Error> {
        let ctx = self.ctx;
        let gmc_vop = ctx.vop.kind == VopType::S;
        if r.flag() {
            // Not coded: zero vector (S-VOP: the global motion), no residual.
            let mut y = [0u8; 256];
            let mut cb = [0u8; 64];
            let mut cr = [0u8; 64];
            let mv = if gmc_vop {
                self.gmc_predict(mbx, mby, &mut y, &mut cb, &mut cr);
                self.gmc_mv(mbx, mby)
            } else {
                let m = MbInfo::default();
                self.predict_p(&m, mbx, mby, &mut y, &mut cb, &mut cr);
                [0, 0]
            };
            // (A not-coded S-VOP macroblock follows the global motion: B-VOPs don't take it
            // for skipped.)
            self.info[a] = MbInfo {
                skipped: !gmc_vop,
                mv: [mv; 4],
                ..MbInfo::default()
            };
            self.clear_intra(mbx, mby);
            store(out, mbx, mby, &y, &cb, &cr);
            return Ok(true);
        }
        let v = self.t.mcbpc_p.read(r).ok_or_else(|| self.err("mcbpc"))?;
        let (ty, cbpc) = ((v >> 4) as u8, (v & 3) as u8);
        if ty == 0xF {
            return Ok(false);
        }
        if ty >= 3 {
            self.intra_mb(r, out, a, mbx, mby, Some((ty, cbpc)))?;
            return Ok(true);
        }
        let mcsel = gmc_vop && ty <= 1 && r.flag();
        let cbpy = 15 - self.t.cbpy.read(r).ok_or_else(|| self.err("cbpy"))? as u32;
        if ty == 1 {
            self.dquant(r);
        }
        let cbp = cbpy << 2 | u32::from(cbpc);
        let mut field_dct = false;
        let mut field_pred = false;
        let mut field_sel = [0u8; 2];
        if ctx.vol.interlaced {
            if cbp != 0 {
                field_dct = r.flag();
            }
            if ty <= 1 && !mcsel {
                field_pred = r.flag();
                if field_pred {
                    field_sel = [r.u(1) as u8, r.u(1) as u8];
                }
            }
        }
        let mut m = MbInfo {
            field: field_pred,
            field_sel,
            ..MbInfo::default()
        };
        let fcode = ctx.vop.fcode_f;
        if mcsel {
            let mv = self.gmc_mv(mbx, mby);
            m.mv = [mv; 4];
        } else if ty == 2 {
            m.four = true;
            for b in 0..4 {
                // Each block's predictor sees the blocks decoded before it.
                self.info[a].mv = m.mv;
                self.info[a].intra = false;
                let pred = self.predictor(a, mbx, mby, b);
                m.mv[b] = self.mv(r, fcode, pred)?;
            }
        } else if field_pred {
            let pred = self.predictor(a, mbx, mby, 0);
            for f in 0..2 {
                let mx = self.mv_component(r, fcode, pred[0])?;
                let my = self.mv_component(r, fcode, pred[1] / 2)?;
                m.field_mv[f] = [mx, my];
            }
            // For later prediction: the average of the two, at frame scale.
            let avg = |a: i32, b: i32| {
                let s = a + b;
                (s >> 1) | (s & 1)
            };
            let fx = avg(m.field_mv[0][0], m.field_mv[1][0]);
            let fy = avg(2 * m.field_mv[0][1], 2 * m.field_mv[1][1]);
            m.mv = [[fx, fy]; 4];
        } else {
            let pred = self.predictor(a, mbx, mby, 0);
            let mv = self.mv(r, fcode, pred)?;
            m.mv = [mv; 4];
        }
        self.info[a] = m;
        self.clear_intra(mbx, mby);
        let mut y = [0u8; 256];
        let mut cb = [0u8; 64];
        let mut cr = [0u8; 64];
        if mcsel {
            self.gmc_predict(mbx, mby, &mut y, &mut cb, &mut cr);
        } else {
            self.predict_p(&m, mbx, mby, &mut y, &mut cb, &mut cr);
        }
        self.residual(r, cbp, field_dct, &mut y, &mut cb, &mut cr)?;
        store(out, mbx, mby, &y, &cb, &cr);
        Ok(true)
    }

    /// Marks a non-intra macroblock's blocks as not intra (for DC/AC prediction).
    fn clear_intra(&mut self, mbx: usize, mby: usize) {
        let w = self.ctx.mb_w;
        for b in 0..4 {
            let (bx, by) = (2 * mbx + (b & 1), 2 * mby + (b >> 1));
            self.pred_y[by * 2 * w + bx] = Pred::default();
        }
        self.pred_c[0][mby * w + mbx] = Pred::default();
        self.pred_c[1][mby * w + mbx] = Pred::default();
    }

    /// Inter residuals of the coded blocks added to the prediction.
    fn residual(
        &self,
        r: &mut Bits,
        cbp: u32,
        field_dct: bool,
        y: &mut [u8; 256],
        cb: &mut [u8; 64],
        cr: &mut [u8; 64],
    ) -> Result<(), Error> {
        let ctx = self.ctx;
        let qp = self.qp;
        let scan: &[u8; 64] = if ctx.vop.alternate_scan {
            &ALT_VERTICAL
        } else {
            &ZIGZAG
        };
        for b in 0..6 {
            if cbp & (1 << (5 - b)) == 0 {
                continue;
            }
            let mut qf = [0i32; 64];
            self.coefficients(r, &mut qf, 0, scan, false, ctx.vop.short_header)?;
            let mut blk = [0i32; 64];
            if ctx.vol.mpeg_quant {
                let m = &ctx.vol.inter_matrix;
                let mut sum = 0;
                for i in 0..64 {
                    if qf[i] != 0 {
                        let v = ((2 * qf[i] + qf[i].signum()) * i32::from(m[i]) * qp / 16)
                            .clamp(-2048, 2047);
                        blk[i] = v;
                        sum += v;
                    }
                }
                if sum & 1 == 0 {
                    blk[63] ^= 1;
                }
            } else {
                for i in 0..64 {
                    blk[i] = h263_dequant(qf[i], qp);
                }
            }
            idct(&mut blk);
            put_block(y, cb, cr, b, field_dct, &blk, false);
        }
        Ok(())
    }

    /// The prediction of a P-VOP macroblock from the reference.
    fn predict_p(
        &self,
        m: &MbInfo,
        mbx: usize,
        mby: usize,
        y: &mut [u8; 256],
        cb: &mut [u8; 64],
        cr: &mut [u8; 64],
    ) {
        let Some(f) = self.ctx.fwd else {
            y.fill(128);
            cb.fill(128);
            cr.fill(128);
            return;
        };
        let rnd = self.ctx.vop.rounding;
        self.predict(f, m, mbx, mby, rnd, y, cb, cr);
    }

    /// Motion-compensated prediction of a macroblock with `m`'s vectors from `f`.
    #[allow(clippy::too_many_arguments)]
    fn predict(
        &self,
        f: &Frame,
        m: &MbInfo,
        mbx: usize,
        mby: usize,
        rnd: bool,
        y: &mut [u8; 256],
        cb: &mut [u8; 64],
        cr: &mut [u8; 64],
    ) {
        let qpel = self.ctx.vol.quarter_sample;
        let (x0, y0) = (16 * mbx as i32, 16 * mby as i32);
        let luma = |dst: &mut [u8],
                    ds: usize,
                    v: &crate::mc::View,
                    x: i32,
                    yy: i32,
                    mv: [i32; 2],
                    w: usize,
                    h: usize| {
            if qpel {
                quarter(dst, ds, v, x, yy, mv[0], mv[1], w, h, rnd);
            } else {
                half(dst, ds, v, x, yy, mv[0], mv[1], w, h, rnd);
            }
        };
        // Luma vectors in half samples, for chroma.
        let to_half = |v: i32| if qpel { v / 2 } else { v };
        if m.field {
            for fld in 0..2 {
                let sel = m.field_sel[fld];
                let mv = m.field_mv[fld];
                let v = f.view(0, Some(sel));
                luma(&mut y[fld * 16..], 32, &v, x0, y0 / 2, mv, 16, 8);
                // Chroma field vectors.
                let cmv = if qpel {
                    [chroma_of(mv[0] / 2), chroma_of(mv[1] >> 1)]
                } else {
                    [chroma_of(mv[0]), chroma_of(mv[1])]
                };
                for (c, dst) in [(1usize, &mut *cb), (2, &mut *cr)] {
                    let v = f.view(c, Some(sel));
                    half(
                        &mut dst[fld * 8..],
                        16,
                        &v,
                        x0 / 2,
                        y0 / 4,
                        cmv[0],
                        cmv[1],
                        8,
                        4,
                        rnd,
                    );
                }
            }
            return;
        }
        let v = f.view(0, None);
        if m.four {
            // 8×8 blocks: their position is held within 16 samples of the VOP (a vector
            // reaching its far edge loses its fraction), as libavcodec does.
            let (vw, vh) = (self.ctx.vol.width as i32, self.ctx.vol.height as i32);
            let hold = |base: i32, v: i32, shift: u32, lo: i32, hi: i32| -> i32 {
                let p = base + (v >> shift);
                let c = p.clamp(lo, hi);
                let frac = if c == hi { 0 } else { v & ((1 << shift) - 1) };
                ((c - base) << shift) | frac
            };
            let ls = if qpel { 2 } else { 1 };
            for b in 0..4 {
                let (bx, by) = (x0 + 8 * (b & 1) as i32, y0 + 8 * (b >> 1) as i32);
                let o = (b >> 1) * 8 * 16 + (b & 1) * 8;
                let mv = [
                    hold(bx, m.mv[b][0], ls, -16, vw),
                    hold(by, m.mv[b][1], ls, -16, vh),
                ];
                luma(&mut y[o..], 16, &v, bx, by, mv, 8, 8);
            }
            let sx: i32 = m.mv.iter().map(|v| to_half(v[0])).sum();
            let sy: i32 = m.mv.iter().map(|v| to_half(v[1])).sum();
            let cmv = [
                hold(x0 / 2, round_chroma_4(sx), 1, -8, vw >> 1),
                hold(y0 / 2, round_chroma_4(sy), 1, -8, vh >> 1),
            ];
            for (c, dst) in [(1usize, &mut *cb), (2, &mut *cr)] {
                let v = f.view(c, None);
                half(dst, 8, &v, x0 / 2, y0 / 2, cmv[0], cmv[1], 8, 8, rnd);
            }
            return;
        }
        let mv = m.mv[0];
        luma(y, 16, &v, x0, y0, mv, 16, 16);
        // Quarter samples: halved toward zero to half samples first.
        let cmv = [chroma_of(to_half(mv[0])), chroma_of(to_half(mv[1]))];
        for (c, dst) in [(1usize, &mut *cb), (2, &mut *cr)] {
            let v = f.view(c, None);
            half(dst, 8, &v, x0 / 2, y0 / 2, cmv[0], cmv[1], 8, 8, rnd);
        }
    }

    // -----------------------------------------------------------------------------------
    // Global motion (S-VOPs)

    /// The vector a GMC macroblock stands for in the prediction of later vectors.
    fn gmc_mv(&self, mbx: usize, mby: usize) -> [i32; 2] {
        self.ctx.gmc.map_or([0, 0], |g| {
            g.amv(mbx, mby, self.ctx.vol.quarter_sample, self.ctx.vop.fcode_f)
        })
    }

    fn gmc_predict(
        &self,
        mbx: usize,
        mby: usize,
        y: &mut [u8; 256],
        cb: &mut [u8; 64],
        cr: &mut [u8; 64],
    ) {
        let (Some(f), Some(g)) = (self.ctx.fwd, self.ctx.gmc) else {
            y.fill(128);
            cb.fill(128);
            cr.fill(128);
            return;
        };
        g.predict(f, mbx, mby, self.ctx.vop.rounding, y, cb, cr);
    }

    // -----------------------------------------------------------------------------------
    // B-VOPs

    fn b_mb(
        &mut self,
        r: &mut Bits,
        out: &mut Frame,
        a: usize,
        mbx: usize,
        mby: usize,
    ) -> Result<(), Error> {
        let ctx = self.ctx;
        let col = ctx.colocated.get(a).copied().unwrap_or_default();
        let mut y = [0u8; 256];
        let mut cb = [0u8; 64];
        let mut cr = [0u8; 64];
        self.clear_intra(mbx, mby);
        if col.skipped {
            // The co-located macroblock was not coded: forward, zero vector, no residual.
            if let Some(f) = ctx.fwd {
                self.predict(
                    f,
                    &MbInfo::default(),
                    mbx,
                    mby,
                    false,
                    &mut y,
                    &mut cb,
                    &mut cr,
                );
            }
            self.info[a] = MbInfo {
                skipped: true,
                ..MbInfo::default()
            };
            store(out, mbx, mby, &y, &cb, &cr);
            return Ok(());
        }
        let skip = r.flag();
        let (kind, cbp) = if skip {
            (0u8, 0u32)
        } else {
            let no_cbp = r.flag();
            let k = self
                .t
                .mb_type_b
                .read(r)
                .ok_or_else(|| self.err("mb_type"))? as u8;
            let cbp = if no_cbp { 0 } else { r.u(6) };
            if k != 0 && cbp != 0 && r.flag() {
                self.qp = (self.qp + if r.flag() { 2 } else { -2 }).clamp(1, 31);
            }
            (k, cbp)
        };
        let mut field_dct = false;
        let mut field = false;
        let mut sel = [[0u8; 2]; 2];
        if ctx.vol.interlaced {
            if cbp != 0 {
                field_dct = r.flag();
            }
            if kind != 0 && r.flag() {
                field = true;
                if kind == 1 || kind == 3 {
                    sel[0] = [r.u(1) as u8, r.u(1) as u8];
                }
                if kind == 1 || kind == 2 {
                    sel[1] = [r.u(1) as u8, r.u(1) as u8];
                }
            }
        }
        let mut fm = MbInfo::default();
        let mut bm = MbInfo::default();
        let (use_f, use_b);
        match kind {
            0 => {
                // Direct: vectors from the co-located macroblock, scaled by time, plus a delta.
                let d = if skip { [0, 0] } else { self.mv(r, 1, [0, 0])? };
                self.direct(&col, d, &mut fm, &mut bm);
                use_f = true;
                use_b = true;
            }
            _ => {
                use_f = kind == 1 || kind == 3;
                use_b = kind == 1 || kind == 2;
                for (dir, used, fcode, m) in [
                    (0usize, use_f, ctx.vop.fcode_f, &mut fm),
                    (1, use_b, ctx.vop.fcode_b, &mut bm),
                ] {
                    if !used {
                        continue;
                    }
                    if field {
                        m.field = true;
                        m.field_sel = sel[dir];
                        for f in 0..2 {
                            let p = self.b_pred_field[dir][f];
                            let mx = self.mv_component(r, fcode, p[0])?;
                            let my = self.mv_component(r, fcode, p[1] / 2)?;
                            m.field_mv[f] = [mx, my];
                            self.b_pred_field[dir][f] = [mx, my * 2];
                        }
                        // A frame vector predicts from the first field's.
                        self.b_pred[dir] = self.b_pred_field[dir][0];
                        m.mv = [self.b_pred[dir]; 4];
                    } else {
                        let mv = self.mv(r, fcode, self.b_pred_field[dir][0])?;
                        self.b_pred[dir] = mv;
                        self.b_pred_field[dir] = [mv, mv];
                        m.mv = [mv; 4];
                    }
                }
            }
        }
        self.info[a] = if use_f { fm } else { bm };
        match (use_f, use_b, ctx.fwd, ctx.bwd) {
            (true, true, Some(f), Some(b)) => {
                self.predict(f, &fm, mbx, mby, false, &mut y, &mut cb, &mut cr);
                let mut y2 = [0u8; 256];
                let mut cb2 = [0u8; 64];
                let mut cr2 = [0u8; 64];
                self.predict(b, &bm, mbx, mby, false, &mut y2, &mut cb2, &mut cr2);
                average(&mut y, 16, &y2, 16, 16, 16);
                average(&mut cb, 8, &cb2, 8, 8, 8);
                average(&mut cr, 8, &cr2, 8, 8, 8);
            }
            (true, _, Some(f), _) | (_, false, Some(f), None) => {
                self.predict(f, &fm, mbx, mby, false, &mut y, &mut cb, &mut cr)
            }
            (_, true, _, Some(b)) => {
                self.predict(b, &bm, mbx, mby, false, &mut y, &mut cb, &mut cr)
            }
            _ => {
                y.fill(128);
                cb.fill(128);
                cr.fill(128);
            }
        }
        self.residual(r, cbp, field_dct, &mut y, &mut cb, &mut cr)?;
        store(out, mbx, mby, &y, &cb, &cr);
        Ok(())
    }

    /// Direct-mode vectors (§7.6.9.5): the co-located vectors scaled by TRB/TRD, plus `d`;
    /// for a field-predicted co-located macroblock, per field with field distances.
    fn direct(&self, col: &MbInfo, d: [i32; 2], fm: &mut MbInfo, bm: &mut MbInfo) {
        let (trb, trd) = (self.ctx.trb, self.ctx.trd.max(1));
        if col.field && !col.intra && !col.four {
            fm.field = true;
            bm.field = true;
            for i in 0..2 {
                let sel = i32::from(col.field_sel[i]);
                let ii = i as i32;
                let (pp, pb) = if self.ctx.vop.top_field_first {
                    (self.ctx.trd_field - sel + ii, self.ctx.trb_field - sel + ii)
                } else {
                    (self.ctx.trd_field + sel - ii, self.ctx.trb_field + sel - ii)
                };
                let pp = if pp == 0 { 1 } else { pp };
                let c = col.field_mv[i];
                let mut f = [0; 2];
                let mut b = [0; 2];
                for k in 0..2 {
                    f[k] = c[k] * pb / pp + d[k];
                    b[k] = if d[k] != 0 {
                        f[k] - c[k]
                    } else {
                        c[k] * (pb - pp) / pp
                    };
                }
                fm.field_mv[i] = f;
                bm.field_mv[i] = b;
                fm.field_sel[i] = col.field_sel[i];
                bm.field_sel[i] = i as u8;
            }
            fm.mv = [fm.field_mv[0]; 4];
            bm.mv = [bm.field_mv[0]; 4];
            return;
        }
        let colv = |b: usize| if col.intra { [0, 0] } else { col.mv[b] };
        let n = if col.four { 4 } else { 1 };
        for b in 0..4 {
            let c = colv(if n == 4 { b } else { 0 });
            let mut f = [0; 2];
            let mut bk = [0; 2];
            for k in 0..2 {
                f[k] = trb * c[k] / trd + d[k];
                bk[k] = if d[k] == 0 {
                    (trb - trd) * c[k] / trd
                } else {
                    f[k] - c[k]
                };
            }
            fm.mv[b] = f;
            bm.mv[b] = bk;
        }
        // Four vectors when the co-located macroblock had four, and with quarter samples.
        let four = col.four || self.ctx.vol.quarter_sample;
        fm.four = four;
        bm.four = four;
    }
}

/// Moves on to macroblock `a` of a new video packet (its earlier macroblocks belong to no
/// packet: they were lost).
fn continue_at(packet: &mut [u32], a: usize, id: u32) {
    if let Some(p) = packet.get_mut(a) {
        *p = id;
    }
}

/// intra_dc_vlc_thr: the DC is coded with its own VLC below this quantiser.
const DC_THR: [u8; 8] = [99, 13, 15, 17, 19, 21, 23, 0];

fn median(a: i32, b: i32, c: i32) -> i32 {
    a.max(b).min(a.min(b).max(c))
}

fn unpack(v: u16) -> (bool, usize, i32) {
    (
        v & 0x8000 != 0,
        usize::from((v >> 8) & 63),
        i32::from(v & 0xFF),
    )
}

/// H.263 inverse quantisation (§7.4.4.1).
#[inline(always)]
fn h263_dequant(qf: i32, qp: i32) -> i32 {
    if qf == 0 {
        return 0;
    }
    let add = if qp & 1 == 1 { qp } else { qp - 1 };
    let m = 2 * qp * qf.abs() + add;
    (if qf < 0 { -m } else { m }).clamp(-2048, 2047)
}

/// Adds (or for intra, puts) an inverse-transformed block into the macroblock's samples.
fn put_block(
    y: &mut [u8; 256],
    cb: &mut [u8; 64],
    cr: &mut [u8; 64],
    b: usize,
    field_dct: bool,
    blk: &[i32; 64],
    intra: bool,
) {
    let (dst, stride, x0, y0, step): (&mut [u8], usize, usize, usize, usize) = match b {
        0..=3 if field_dct => (&mut y[..], 16, (b & 1) * 8, b >> 1, 2),
        0..=3 => (&mut y[..], 16, (b & 1) * 8, (b >> 1) * 8, 1),
        4 => (&mut cb[..], 8, 0, 0, 1),
        _ => (&mut cr[..], 8, 0, 0, 1),
    };
    for i in 0..8 {
        let o = (y0 + i * step) * stride + x0;
        for j in 0..8 {
            let p = if intra { 0 } else { i32::from(dst[o + j]) };
            dst[o + j] = (p + blk[i * 8 + j]).clamp(0, 255) as u8;
        }
    }
}

fn store(out: &mut Frame, mbx: usize, mby: usize, y: &[u8; 256], cb: &[u8; 64], cr: &[u8; 64]) {
    let s = out.stride[0];
    for i in 0..16 {
        let o = (mby * 16 + i) * s + mbx * 16;
        out.planes[0][o..o + 16].copy_from_slice(&y[i * 16..i * 16 + 16]);
    }
    for (c, src) in [(1usize, cb), (2, cr)] {
        let s = out.stride[c];
        for i in 0..8 {
            let o = (mby * 8 + i) * s + mbx * 8;
            out.planes[c][o..o + 8].copy_from_slice(&src[i * 8..i * 8 + 8]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(median(1, 5, 3), 3);
        assert_eq!(median(-4, -9, 2), -4);
        assert_eq!(h263_dequant(1, 5), 15);
        assert_eq!(h263_dequant(-1, 4), -11);
        assert_eq!(h263_dequant(0, 4), 0);
    }
}
