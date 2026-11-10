//! The slice and macroblock layers (§6.2.4–§6.2.6) and their reconstruction (§7.2–§7.6):
//! coefficients, inverse quantisation with saturation and mismatch control (MPEG-1:
//! oddification), the inverse DCT, frame, field, 16×8 and dual-prime prediction, skipped
//! macroblocks, and concealment of what a damaged slice leaves out.

use crate::Chroma;
use crate::bits::Bits;
use crate::frame::{Frame, mc};
use crate::headers::{PictureHeader, PictureType, Sequence, Structure};
use crate::idct::idct;
use crate::tables::NONLINEAR_QSCALE;
use crate::tables::{ALTERNATE, MB_BACKWARD, MB_FORWARD, MB_INTRA, MB_PATTERN, MB_QUANT, ZIGZAG};
use crate::vlc::{ADDR_ESCAPE, ADDR_STUFFING, DCT_EOB, DCT_ESCAPE, Tables, tables};

/// Motion types (frame_motion_type / field_motion_type).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Motion {
    /// Frame pictures: frame prediction. Field pictures: field prediction (16×16).
    Whole,
    /// Frame pictures: field prediction. Field pictures: 16×8.
    Halves,
    DualPrime,
}

/// What a picture's slices decode against.
pub(crate) struct Ctx<'a> {
    pub seq: &'a Sequence,
    pub pic: &'a PictureHeader,
    pub quant: &'a [[u8; 64]; 4],
    pub mb_w: usize,
    /// Macroblock rows of this picture (a field has half the frame's).
    pub mb_rows: usize,
    /// Forward and backward reference frames.
    pub fwd: Option<&'a Frame>,
    pub bwd: Option<&'a Frame>,
    /// The second field of a frame: the first field (its parity, and the frame holding it).
    pub first_field: Option<(u8, &'a Frame)>,
}

impl Ctx<'_> {
    fn field_parity(&self) -> Option<u8> {
        match self.pic.structure {
            Structure::Top => Some(0),
            Structure::Bottom => Some(1),
            Structure::Frame => None,
        }
    }

    /// The reference to predict field `parity` from in direction `s` (field pictures, and
    /// field prediction in frame pictures).
    fn field_ref(&self, s: usize, parity: u8) -> Option<&Frame> {
        if s == 0 {
            if let Some((p, f)) = self.first_field
                && p == parity
                && self.pic.kind == PictureType::P
            {
                return Some(f);
            }
            self.fwd
        } else {
            self.bwd
        }
    }
}

/// One job: a run of macroblock rows and the slices that start in them, written into
/// `out` (the frame lines those rows cover).
pub(crate) struct Job<'a> {
    /// First macroblock row (of the picture).
    pub row0: usize,
    pub rows: usize,
    /// Slice units: (macroblock row, bytes after the start code).
    pub slices: Vec<(usize, &'a [u8])>,
    pub out: [&'a mut [u8]; 3],
}

/// The motion of a macroblock (kept for skipped B macroblocks, which repeat it).
#[derive(Clone, Copy, Debug)]
struct MbMotion {
    dirs: u8,
    motion: Motion,
    /// vector[r][s][t] in half-pels (field vectors in field lines).
    mv: [[[i32; 2]; 2]; 2],
    field_select: [[u8; 2]; 2],
    /// Dual prime: the derived opposite-parity vectors (for the top and bottom fields of a
    /// frame picture, or the one of a field picture).
    dmv: [[i32; 2]; 2],
}

impl MbMotion {
    const NONE: MbMotion = MbMotion {
        dirs: 0,
        motion: Motion::Whole,
        mv: [[[0; 2]; 2]; 2],
        field_select: [[0; 2]; 2],
        dmv: [[0; 2]; 2],
    };
}

/// Decodes every slice of `job`. Damaged slices lose their remaining macroblocks to
/// concealment; returns how many slices were damaged.
pub(crate) fn decode_job(ctx: &Ctx, job: &mut Job) -> usize {
    let mut damaged = 0;
    let t = tables();
    let Job {
        row0,
        rows,
        slices,
        out,
    } = job;
    let (row0, rows) = (*row0, *rows);
    let total = rows * ctx.mb_w;
    // Macroblock addresses (relative to the job) not yet reconstructed start at `next`.
    let mut next = 0usize;
    let mut w = Writer::new(ctx, row0, rows, out);
    for k in 0..slices.len() {
        let (row, data) = slices[k];
        let start = (row - row0) * ctx.mb_w;
        if start < next {
            // A slice overlapping one already decoded: skip it.
            damaged += 1;
            continue;
        }
        conceal(ctx, &mut w, next, start);
        match decode_slice(ctx, t, &mut w, row, data) {
            Ok(end) => next = end,
            Err(end) => {
                damaged += 1;
                next = end.max(start);
            }
        }
        let limit = slices.get(k + 1).map_or(total, |s| (s.0 - row0) * ctx.mb_w);
        if next > limit && ctx.seq.mpeg2 {
            next = limit.max(start);
        }
    }
    conceal(ctx, &mut w, next, total);
    damaged
}

/// Writes reconstructed macroblocks into a job's lines.
struct Writer<'a, 'b> {
    out: &'b mut [&'a mut [u8]; 3],
    row0: usize,
    rows: usize,
    stride: [usize; 3],
    /// Field pictures: the parity written (lines interleave).
    parity: Option<u8>,
    shift: (u32, u32),
}

impl<'a, 'b> Writer<'a, 'b> {
    fn new(ctx: &Ctx, row0: usize, rows: usize, out: &'b mut [&'a mut [u8]; 3]) -> Writer<'a, 'b> {
        let shift = ctx.seq.chroma.shift();
        let w = ctx.mb_w * 16;
        Writer {
            row0,
            rows,
            out,
            stride: [w, w >> shift.0, w >> shift.0],
            parity: ctx.field_parity(),
            shift,
        }
    }

    /// Stores a macroblock's samples (`y`: 16×16, `c`: chroma blocks) at (`x`, `y_rel`).
    fn put(&mut self, mbx: usize, mby: usize, y: &[u8; 256], cb: &[u8; 256], cr: &[u8; 256]) {
        let rel = mby - self.row0;
        debug_assert!(rel < self.rows);
        let cw = 16 >> self.shift.0;
        let ch = 16 >> self.shift.1;
        for (c, (src, bw, bh)) in [(&y[..], 16, 16), (&cb[..], cw, ch), (&cr[..], cw, ch)]
            .into_iter()
            .enumerate()
        {
            let stride = self.stride[c];
            let x0 = mbx * bw;
            for i in 0..bh {
                let line = match self.parity {
                    None => rel * bh + i,
                    Some(p) => 2 * (rel * bh + i) + usize::from(p),
                };
                let o = line * stride + x0;
                self.out[c][o..o + bw].copy_from_slice(&src[i * bw..i * bw + bw]);
            }
        }
    }
}

/// Macroblocks `from..to` (job-relative) that no slice gave: copied from the forward
/// reference where the picture predicts, else mid-grey.
fn conceal(ctx: &Ctx, w: &mut Writer, from: usize, to: usize) {
    let mut y = [128u8; 256];
    let mut cb = [128u8; 256];
    let mut cr = [128u8; 256];
    let parity = ctx.field_parity();
    for a in from..to {
        let mbx = a % ctx.mb_w;
        let mby = w.row0 + a / ctx.mb_w;
        let src = match parity {
            Some(p) => ctx.field_ref(0, p),
            None => ctx.fwd,
        };
        if let Some(f) = src {
            let mot = MbMotion {
                dirs: MB_FORWARD,
                ..MbMotion::NONE
            };
            predict(ctx, &mot, mbx, mby, &mut y, &mut cb, &mut cr, Some(f));
        }
        w.put(mbx, mby, &y, &cb, &cr);
    }
}

/// Per-slice decoding state.
struct State {
    qscale: i32,
    dc_pred: [i32; 3],
    /// PMV[r][s][t].
    pmv: [[[i32; 2]; 2]; 2],
    last: MbMotion,
}

fn quantiser_scale(ctx: &Ctx, code: u32) -> i32 {
    if !ctx.seq.mpeg2 {
        code as i32
    } else if ctx.pic.q_scale_type {
        i32::from(NONLINEAR_QSCALE[code as usize & 31])
    } else {
        2 * code as i32
    }
}

/// Decodes one slice starting in macroblock row `row`. `Ok`/`Err` carry the job-relative
/// address after the last macroblock reconstructed.
fn decode_slice(
    ctx: &Ctx,
    t: &Tables,
    w: &mut Writer,
    row: usize,
    data: &[u8],
) -> Result<usize, usize> {
    let mut r = Bits::new(data);
    let mut row = row;
    if ctx.seq.height > 2800 {
        row += (r.u(3) as usize) << 7;
    }
    let base = (row - w.row0) * ctx.mb_w;
    let code = r.u(5);
    if code == 0 {
        return Err(base);
    }
    let dc_reset = 1 << (7 + ctx.pic.intra_dc_precision);
    let mut st = State {
        qscale: quantiser_scale(ctx, code),
        dc_pred: [dc_reset; 3],
        pmv: [[[0; 2]; 2]; 2],
        last: MbMotion::NONE,
    };
    if ctx.seq.mpeg2 && r.peek32() >> 31 == 1 {
        // intra_slice_flag, intra_slice, reserved_bits, then extra information.
        r.skip(9);
    }
    while r.flag() {
        r.skip(8);
    }
    let total = w.rows * ctx.mb_w;
    // The address before the first macroblock: the end of the previous row.
    let mut addr: isize = base as isize - 1;
    let mut first = true;
    let mut mbs = MbBufs::default();
    loop {
        // macroblock_address_increment, with escapes (and MPEG-1 stuffing).
        let mut inc = 0usize;
        loop {
            match t.addr_inc.read(&mut r) {
                Some(ADDR_ESCAPE) => inc += 33,
                Some(ADDR_STUFFING) => {}
                Some(v) => {
                    inc += usize::from(v);
                    break;
                }
                None => return Err((addr + 1).max(base as isize) as usize),
            }
        }
        let target = addr + inc as isize;
        if target as usize >= total || r.overrun() {
            return Err((addr + 1).max(base as isize) as usize);
        }
        if !first && inc > 1 {
            // Skipped macroblocks.
            for a in (addr + 1) as usize..target as usize {
                skipped(ctx, w, &mut st, a, &mut mbs).map_err(|_| a)?;
            }
        }
        first = false;
        addr = target;
        let a = addr as usize;
        if macroblock(ctx, t, w, &mut st, &mut r, a, &mut mbs).is_err() || r.overrun() {
            return Err(a);
        }
        // The slice ends where only zero bits (the next start code's) are left.
        if r.left() <= 0 || r.peek32() >> 9 == 0 {
            return Ok(a + 1);
        }
    }
}

#[derive(Default)]
struct MbBufs {
    blk: Blk,
}

struct Blk([i32; 64]);

impl Default for Blk {
    fn default() -> Blk {
        Blk([0; 64])
    }
}

/// A skipped macroblock at job-relative address `a`.
fn skipped(ctx: &Ctx, w: &mut Writer, st: &mut State, a: usize, _m: &mut MbBufs) -> Result<(), ()> {
    let dc_reset = 1 << (7 + ctx.pic.intra_dc_precision);
    st.dc_pred = [dc_reset; 3];
    let mot = match ctx.pic.kind {
        PictureType::P => {
            st.pmv = [[[0; 2]; 2]; 2];
            let parity = ctx.field_parity().unwrap_or(0);
            MbMotion {
                dirs: MB_FORWARD,
                motion: Motion::Whole,
                field_select: [[parity; 2]; 2],
                ..MbMotion::NONE
            }
        }
        PictureType::B => {
            // §7.6.6.4: the previous macroblock's directions, frame prediction (field
            // pictures: from the same parity) with the predicted vectors.
            if st.last.dirs == 0 {
                return Err(());
            }
            let parity = ctx.field_parity().unwrap_or(0);
            let mut mv = [[[0; 2]; 2]; 2];
            for s in 0..2 {
                let k = if !ctx.seq.mpeg2 && ctx.pic.full_pel[s] {
                    2
                } else {
                    1
                };
                mv[0][s] = [st.pmv[0][s][0] * k, st.pmv[0][s][1] * k];
            }
            MbMotion {
                dirs: st.last.dirs,
                motion: Motion::Whole,
                mv,
                field_select: [[parity; 2]; 2],
                dmv: [[0; 2]; 2],
            }
        }
        _ => return Err(()),
    };
    let mbx = a % ctx.mb_w;
    let mby = w.row0 + a / ctx.mb_w;
    let mut y = [0u8; 256];
    let mut cb = [0u8; 256];
    let mut cr = [0u8; 256];
    predict(ctx, &mot, mbx, mby, &mut y, &mut cb, &mut cr, None);
    w.put(mbx, mby, &y, &cb, &cr);
    Ok(())
}

/// Reads motion_code + residual for one component and returns the new vector component
/// from its prediction (§7.6.3.1).
fn motion_component(r: &mut Bits, t: &Tables, f_code: u8, pred: i32) -> Result<i32, ()> {
    let code = i32::from(t.motion.read(r).ok_or(())?);
    let code = if code != 0 && r.flag() { -code } else { code };
    let r_size = u32::from(f_code.max(1)) - 1;
    let f = 1i32 << r_size;
    let delta = if f == 1 || code == 0 {
        code
    } else {
        let residual = r.u(r_size) as i32;
        let d = ((code.abs() - 1) << r_size) + residual + 1;
        if code < 0 { -d } else { d }
    };
    let low = -16 * f;
    let high = 16 * f - 1;
    let range = 32 * f;
    let mut v = pred + delta;
    if v < low {
        v += range;
    }
    if v > high {
        v -= range;
    }
    Ok(v)
}

fn dmvector(r: &mut Bits) -> i32 {
    if !r.flag() {
        0
    } else if r.flag() {
        -1
    } else {
        1
    }
}

/// motion_vector(r, s): both components, with dual-prime differentials; field vectors in
/// frame pictures predict from and store PMV at frame scale.
#[allow(clippy::too_many_arguments)]
fn motion_vector(
    ctx: &Ctx,
    r: &mut Bits,
    t: &Tables,
    st: &mut State,
    rr: usize,
    s: usize,
    field_format: bool,
    dual: bool,
    dm: &mut [i32; 2],
) -> Result<[i32; 2], ()> {
    let frame_pic = ctx.pic.structure == Structure::Frame;
    let mut v = [0i32; 2];
    for tt in 0..2 {
        let f_code = ctx.pic.f_code[s][tt];
        if ctx.seq.mpeg2 && f_code == 15 {
            return Err(());
        }
        let halve = field_format && tt == 1 && frame_pic;
        let pred = if halve {
            st.pmv[rr][s][tt] >> 1
        } else {
            st.pmv[rr][s][tt]
        };
        v[tt] = motion_component(r, t, f_code, pred)?;
        st.pmv[rr][s][tt] = if halve { v[tt] * 2 } else { v[tt] };
        if dual {
            dm[tt] = dmvector(r);
        }
    }
    Ok(v)
}

/// The vectors of a macroblock in direction `s` (motion_vectors(s), §6.2.5.2).
fn motion_vectors(
    ctx: &Ctx,
    r: &mut Bits,
    t: &Tables,
    st: &mut State,
    mot: &mut MbMotion,
    s: usize,
) -> Result<(), ()> {
    let frame_pic = ctx.pic.structure == Structure::Frame;
    let (count, field_format) = match (frame_pic, mot.motion) {
        (true, Motion::Whole) => (1, false),
        (true, Motion::Halves) => (2, true),
        (true, Motion::DualPrime) => (1, true),
        (false, Motion::Whole) => (1, true),
        (false, Motion::Halves) => (2, true),
        (false, Motion::DualPrime) => (1, true),
    };
    let dual = mot.motion == Motion::DualPrime;
    let mut dm = [0i32; 2];
    if count == 1 {
        if field_format && !dual {
            mot.field_select[0][s] = r.u(1) as u8;
        }
        mot.mv[0][s] = motion_vector(ctx, r, t, st, 0, s, field_format, dual, &mut dm)?;
        st.pmv[1][s] = st.pmv[0][s];
    } else {
        for rr in 0..2 {
            mot.field_select[rr][s] = r.u(1) as u8;
            mot.mv[rr][s] = motion_vector(ctx, r, t, st, rr, s, field_format, false, &mut dm)?;
        }
    }
    if !ctx.seq.mpeg2 && ctx.pic.full_pel[s] {
        // MPEG-1 full-pel vectors predict in their own units; prediction uses half-pels.
        for v in &mut mot.mv[0][s] {
            *v *= 2;
        }
    }
    if dual {
        // §7.6.3.6: the opposite-parity vectors from the same-parity one.
        let v = mot.mv[0][0];
        let scale = |x: i32, m: i32| (x * m + i32::from(x > 0)) >> 1;
        if frame_pic {
            let (m0, m1) = if ctx.pic.top_field_first {
                (1, 3)
            } else {
                (3, 1)
            };
            // Top field from the bottom reference field, bottom from the top.
            mot.dmv[0] = [scale(v[0], m0) + dm[0], scale(v[1], m0) - 1 + dm[1]];
            mot.dmv[1] = [scale(v[0], m1) + dm[0], scale(v[1], m1) + 1 + dm[1]];
        } else {
            let e = if ctx.pic.structure == Structure::Top {
                -1
            } else {
                1
            };
            mot.dmv[0] = [scale(v[0], 1) + dm[0], scale(v[1], 1) + e + dm[1]];
        }
    }
    Ok(())
}

/// Decodes one macroblock at job-relative address `a` and writes it.
fn macroblock(
    ctx: &Ctx,
    t: &Tables,
    w: &mut Writer,
    st: &mut State,
    r: &mut Bits,
    a: usize,
    m: &mut MbBufs,
) -> Result<(), ()> {
    let pic = ctx.pic;
    let frame_pic = pic.structure == Structure::Frame;
    let flags = match pic.kind {
        PictureType::I => t.mb_i.read(r),
        PictureType::P => t.mb_p.read(r),
        PictureType::B => t.mb_b.read(r),
        // MPEG-1 D-pictures (ISO/IEC 11172-2 2.4.3.6): macroblock_type is a single 1 (intra).
        PictureType::D => r.flag().then_some(u16::from(MB_INTRA)),
    }
    .ok_or(())? as u8;
    let intra = flags & MB_INTRA != 0;
    let mut mot = MbMotion {
        dirs: flags & (MB_FORWARD | MB_BACKWARD),
        ..MbMotion::NONE
    };
    if mot.dirs != 0 {
        let code = if frame_pic {
            if pic.frame_pred_frame_dct { 2 } else { r.u(2) }
        } else {
            r.u(2)
        };
        mot.motion = match code {
            1 => {
                if frame_pic {
                    Motion::Halves
                } else {
                    Motion::Whole
                }
            }
            2 => {
                if frame_pic {
                    Motion::Whole
                } else {
                    Motion::Halves
                }
            }
            3 => Motion::DualPrime,
            _ => return Err(()),
        };
    }
    let field_dct =
        frame_pic && !pic.frame_pred_frame_dct && (intra || flags & MB_PATTERN != 0) && r.flag();
    if flags & MB_QUANT != 0 {
        let code = r.u(5);
        if code == 0 {
            return Err(());
        }
        st.qscale = quantiser_scale(ctx, code);
    }
    let dc_reset = 1 << (7 + pic.intra_dc_precision);
    if intra {
        if pic.concealment {
            // Concealment vectors: parsed for their prediction, not used.
            let mut cm = MbMotion {
                motion: Motion::Whole,
                ..MbMotion::NONE
            };
            motion_vectors(ctx, r, t, st, &mut cm, 0)?;
            r.skip(1);
        } else {
            st.pmv = [[[0; 2]; 2]; 2];
        }
        st.last = MbMotion::NONE;
    } else {
        st.dc_pred = [dc_reset; 3];
        if flags & MB_FORWARD != 0 {
            motion_vectors(ctx, r, t, st, &mut mot, 0)?;
        }
        if flags & MB_BACKWARD != 0 {
            motion_vectors(ctx, r, t, st, &mut mot, 1)?;
        }
        if pic.kind == PictureType::P && flags & MB_FORWARD == 0 {
            // No motion: zero vector, frame prediction (field pictures: the same parity).
            st.pmv = [[[0; 2]; 2]; 2];
            let parity = ctx.field_parity().unwrap_or(0);
            mot = MbMotion {
                dirs: MB_FORWARD,
                motion: Motion::Whole,
                field_select: [[parity; 2]; 2],
                ..MbMotion::NONE
            };
        }
        st.last = mot;
    }
    let blocks = ctx.seq.chroma.blocks();
    let cbp: u32 = if intra {
        (1 << blocks) - 1
    } else if flags & MB_PATTERN != 0 {
        let c = u32::from(t.cbp.read(r).ok_or(())?);
        match ctx.seq.chroma {
            Chroma::Yuv420 => c,
            Chroma::Yuv422 => c << 2 | r.u(2),
            Chroma::Yuv444 => c << 6 | r.u(6),
        }
    } else {
        0
    };
    let mbx = a % ctx.mb_w;
    let mby = w.row0 + a / ctx.mb_w;
    let mut y = [0u8; 256];
    let mut cb = [0u8; 256];
    let mut cr = [0u8; 256];
    if !intra {
        predict(ctx, &mot, mbx, mby, &mut y, &mut cb, &mut cr, None);
    }
    let (sx, sy) = ctx.seq.chroma.shift();
    let cw = 16 >> sx;
    let scan = if pic.alternate_scan {
        &ALTERNATE
    } else {
        &ZIGZAG
    };
    for b in 0..blocks {
        if cbp & (1 << (blocks - 1 - b)) == 0 {
            continue;
        }
        let blk = &mut m.blk.0;
        *blk = [0; 64];
        let cc = if b < 4 { 0 } else { 1 + (b & 1) };
        if intra {
            intra_block(ctx, t, r, st, cc, scan, blk)?;
        } else {
            inter_block(ctx, t, r, st, cc, scan, blk)?;
        }
        idct(blk);
        // Where the block lies in the macroblock: offset and line step.
        let (dst, stride): (&mut [u8], usize) = if b < 4 {
            (&mut y, 16)
        } else if b & 1 == 0 {
            (&mut cb, cw)
        } else {
            (&mut cr, cw)
        };
        let (bx, by, step) = if b < 4 {
            if field_dct {
                ((b & 1) * 8, b >> 1, 2)
            } else {
                ((b & 1) * 8, (b >> 1) * 8, 1)
            }
        } else {
            // Chroma: k-th block of its component (4:2:0 one; 4:2:2 top, bottom; 4:4:4
            // top-left, bottom-left, top-right, bottom-right).
            let k = (b - 4) >> 1;
            let bx = (k >> 1) * 8;
            let lower = k & 1;
            if field_dct && sy == 0 {
                (bx, lower, 2)
            } else {
                (bx, lower * 8, 1)
            }
        };
        for i in 0..8 {
            let o = (by + i * step) * stride + bx;
            for j in 0..8 {
                let v = i32::from(dst[o + j]) + blk[i * 8 + j];
                dst[o + j] = v.clamp(0, 255) as u8;
            }
        }
    }
    // end_of_macroblock of a D-picture.
    if pic.kind == PictureType::D && !r.flag() {
        return Err(());
    }
    w.put(mbx, mby, &y, &cb, &cr);
    Ok(())
}

/// Saturates a reconstructed coefficient (§7.4.3).
#[inline(always)]
fn sat(v: i32) -> i32 {
    v.clamp(-2048, 2047)
}

fn escape_level(ctx: &Ctx, r: &mut Bits) -> Result<(usize, i32), ()> {
    let run = r.u(6) as usize;
    if ctx.seq.mpeg2 {
        let l = r.u(12) as i32;
        let l = if l >= 2048 { l - 4096 } else { l };
        if l == 0 || l == -2048 {
            return Err(());
        }
        Ok((run, l))
    } else {
        let l = r.u(8) as i32;
        let l = match l {
            0 => r.u(8) as i32,
            128 => r.u(8) as i32 - 256,
            l if l > 127 => l - 256,
            l => l,
        };
        if l == 0 {
            return Err(());
        }
        Ok((run, l))
    }
}

/// The next (run, level) of a block: `None` at its end.
#[inline(always)]
fn coefficient(
    ctx: &Ctx,
    r: &mut Bits,
    table: &crate::vlc::Vlc,
) -> Result<Option<(usize, i32)>, ()> {
    let v = table.read(r).ok_or(())?;
    if v == DCT_EOB {
        return Ok(None);
    }
    if v == DCT_ESCAPE {
        return escape_level(ctx, r).map(Some);
    }
    let run = usize::from(v & 63);
    let level = i32::from((v >> 6) & 63);
    Ok(Some((run, if r.flag() { -level } else { level })))
}

fn intra_block(
    ctx: &Ctx,
    t: &Tables,
    r: &mut Bits,
    st: &mut State,
    cc: usize,
    scan: &[u8; 64],
    blk: &mut [i32; 64],
) -> Result<(), ()> {
    let dct_table = if cc == 0 { &t.dc_luma } else { &t.dc_chroma };
    let size = u32::from(dct_table.read(r).ok_or(())?);
    let diff = if size == 0 {
        0
    } else {
        let bits = r.u(size) as i32;
        if bits >= 1 << (size - 1) {
            bits
        } else {
            bits + 1 - (1 << size)
        }
    };
    st.dc_pred[cc] += diff;
    let mpeg2 = ctx.seq.mpeg2;
    let mul = 8 >> ctx.pic.intra_dc_precision;
    let dc = st.dc_pred[cc] * mul;
    blk[0] = if mpeg2 { sat(dc) } else { dc };
    let mut sum = blk[0];
    if ctx.pic.kind == PictureType::D {
        // D-pictures code the DC coefficient only.
        return Ok(());
    }
    let q = &ctx.quant[if cc == 0 { 0 } else { 2 }];
    let table = if ctx.pic.intra_vlc { &t.dct1 } else { &t.dct0 };
    let qs = st.qscale;
    let mut i = 0usize;
    while let Some((run, level)) = coefficient(ctx, r, table)? {
        i += run + 1;
        if i > 63 {
            return Err(());
        }
        let j = usize::from(scan[i]);
        let wq = i32::from(q[j]);
        let v = if mpeg2 {
            sat(level * wq * qs / 16)
        } else {
            let v = level * qs * wq / 8;
            sat(if v & 1 == 0 { v - v.signum() } else { v })
        };
        blk[j] = v;
        sum += v;
    }
    if mpeg2 && sum & 1 == 0 {
        blk[63] ^= 1;
    }
    Ok(())
}

fn inter_block(
    ctx: &Ctx,
    t: &Tables,
    r: &mut Bits,
    st: &mut State,
    cc: usize,
    scan: &[u8; 64],
    blk: &mut [i32; 64],
) -> Result<(), ()> {
    let mpeg2 = ctx.seq.mpeg2;
    let q = &ctx.quant[if cc == 0 { 1 } else { 3 }];
    let qs = st.qscale;
    let mut sum = 0;
    let mut i: isize = -1;
    // The first coefficient: "1s" is (0, ±1).
    let first = if r.peek32() >> 31 == 1 {
        r.skip(1);
        Some((0, if r.flag() { -1 } else { 1 }))
    } else {
        Some(coefficient(ctx, r, &t.dct0)?.ok_or(())?)
    };
    let mut next = first;
    while let Some((run, level)) = next {
        i += run as isize + 1;
        if i > 63 {
            return Err(());
        }
        let j = usize::from(scan[i as usize]);
        let wq = i32::from(q[j]);
        let v = if mpeg2 {
            sat((2 * level + level.signum()) * wq * qs / 32)
        } else {
            let v = (2 * level + level.signum()) * qs * wq / 16;
            sat(if v & 1 == 0 { v - v.signum() } else { v })
        };
        blk[j] = v;
        sum += v;
        next = coefficient(ctx, r, &t.dct0)?;
    }
    if mpeg2 && sum & 1 == 0 {
        blk[63] ^= 1;
    }
    Ok(())
}

/// Forms a macroblock's prediction (§7.6.4–§7.6.7) into `y`, `cb`, `cr` (macroblock order:
/// 16 luma lines, chroma by the format; field pictures' lines are field lines). `only`:
/// predict from this frame with a zero vector (concealment).
#[allow(clippy::too_many_arguments)]
fn predict(
    ctx: &Ctx,
    mot: &MbMotion,
    mbx: usize,
    mby: usize,
    y: &mut [u8; 256],
    cb: &mut [u8; 256],
    cr: &mut [u8; 256],
    only: Option<&Frame>,
) {
    let (sx, sy) = ctx.seq.chroma.shift();
    let cw = 16usize >> sx;
    let parity = ctx.field_parity();
    let x0 = (mbx * 16) as i32;
    let mut done = false;
    for s in 0..2 {
        let dir = if s == 0 { MB_FORWARD } else { MB_BACKWARD };
        if mot.dirs & dir == 0 {
            continue;
        }
        let avg = done;
        done = true;
        // One rectangle: from `frame` (field `p`, or the whole frame), vector `v`, luma lines
        // `l0..l0+lh` of the macroblock arrays written every `lstep` lines starting at
        // `lfirst`, at reference position `ry` (in the view's lines).
        let mut part = |frame: Option<&Frame>,
                        p: Option<u8>,
                        v: [i32; 2],
                        ry: i32,
                        lh: usize,
                        lfirst: usize,
                        lstep: usize,
                        avg: bool| {
            let Some(f) = frame else {
                return;
            };
            let view = f.view(0, p);
            mc(
                &mut y[lfirst * 16..],
                16 * lstep,
                &view,
                x0 + (v[0] >> 1),
                ry + (v[1] >> 1),
                v[0] & 1 != 0,
                v[1] & 1 != 0,
                16,
                lh,
                avg,
            );
            // Chroma: vectors halved (toward zero) where chroma is subsampled.
            let cvx = if sx == 1 { v[0] / 2 } else { v[0] };
            let cvy = if sy == 1 { v[1] / 2 } else { v[1] };
            let cx0 = x0 >> sx;
            let cry = ry >> sy;
            let clh = lh >> sy;
            // Field lines of a frame macroblock start at their parity; halves at their line.
            let cfirst = if lstep == 2 { lfirst } else { lfirst >> sy };
            for (c, dst) in [(1usize, &mut *cb), (2, &mut *cr)] {
                let view = f.view(c, p);
                mc(
                    &mut dst[cfirst * cw..],
                    cw * lstep,
                    &view,
                    cx0 + (cvx >> 1),
                    cry + (cvy >> 1),
                    cvx & 1 != 0,
                    cvy & 1 != 0,
                    cw,
                    clh,
                    avg,
                );
            }
        };
        if let Some(f) = only {
            match parity {
                None => part(Some(f), None, [0, 0], (mby * 16) as i32, 16, 0, 1, false),
                Some(p) => part(Some(f), Some(p), [0, 0], (mby * 16) as i32, 16, 0, 1, false),
            }
            return;
        }
        match (parity, mot.motion) {
            (None, Motion::Whole) => {
                let f = if s == 0 { ctx.fwd } else { ctx.bwd };
                part(f, None, mot.mv[0][s], (mby * 16) as i32, 16, 0, 1, avg);
            }
            (None, Motion::Halves) => {
                for rr in 0..2 {
                    let sel = mot.field_select[rr][s];
                    let f = if s == 0 { ctx.fwd } else { ctx.bwd };
                    part(f, Some(sel), mot.mv[rr][s], (mby * 8) as i32, 8, rr, 2, avg);
                }
            }
            (None, Motion::DualPrime) => {
                for fp in 0..2u8 {
                    let l = usize::from(fp);
                    part(
                        ctx.fwd,
                        Some(fp),
                        mot.mv[0][0],
                        (mby * 8) as i32,
                        8,
                        l,
                        2,
                        false,
                    );
                    part(
                        ctx.fwd,
                        Some(1 - fp),
                        mot.dmv[l],
                        (mby * 8) as i32,
                        8,
                        l,
                        2,
                        true,
                    );
                }
            }
            (Some(cp), Motion::Whole) => {
                let sel = mot.field_select[0][s];
                let f = ctx.field_ref(s, sel);
                part(f, Some(sel), mot.mv[0][s], (mby * 16) as i32, 16, 0, 1, avg);
                let _ = cp;
            }
            (Some(_), Motion::Halves) => {
                for rr in 0..2 {
                    let sel = mot.field_select[rr][s];
                    let f = ctx.field_ref(s, sel);
                    part(
                        f,
                        Some(sel),
                        mot.mv[rr][s],
                        (mby * 16 + rr * 8) as i32,
                        8,
                        rr * 8,
                        1,
                        avg,
                    );
                }
            }
            (Some(cp), Motion::DualPrime) => {
                let same = ctx.field_ref(0, cp);
                let opp = ctx.field_ref(0, 1 - cp);
                part(
                    same,
                    Some(cp),
                    mot.mv[0][0],
                    (mby * 16) as i32,
                    16,
                    0,
                    1,
                    false,
                );
                part(
                    opp,
                    Some(1 - cp),
                    mot.dmv[0],
                    (mby * 16) as i32,
                    16,
                    0,
                    1,
                    true,
                );
            }
        }
    }
}
