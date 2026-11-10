//! Sub-band Speex: wideband is a narrowband low band plus a 4–8 kHz band; ultra-wideband is
//! a wideband low band plus an 8–16 kHz band. The upper band has its own LSPs (order 8) and
//! either a split-VQ innovation scaled from the low band's excitation, or spectral folding
//! of the low band's innovation; the two bands are joined by a 64-tap QMF bank.

use crate::nb::{self, Innov, LowBand, Nb};
use crate::tables::*;
use crate::{Bits, Error, Frame, Stereo};

const ORDER: usize = 8;
const SUBS: usize = 4;
const QMF: usize = 64;
const LSP_MARGIN: f32 = 0.05;

#[derive(Clone, Copy)]
struct Submode {
    double_codebook: bool,
    /// None: spectral folding.
    innov: Option<Innov>,
}

const WB_SUBMODES: [Option<Submode>; 8] = [
    None,
    Some(Submode {
        double_codebook: false,
        innov: None,
    }),
    Some(Submode {
        double_codebook: false,
        innov: Some(Innov::Split(10, 4, &HEXC_10_32, 5, false)),
    }),
    Some(Submode {
        double_codebook: false,
        innov: Some(Innov::Split(8, 5, &HEXC, 7, true)),
    }),
    Some(Submode {
        double_codebook: true,
        innov: Some(Innov::Split(8, 5, &HEXC, 7, true)),
    }),
    None,
    None,
    None,
];

const GC_BOUND: [f32; 16] = [
    0.97979, 1.28384, 1.68223, 2.20426, 2.88829, 3.78458, 4.95900, 6.49787, 8.51428, 11.15642,
    14.61846, 19.15484, 25.09895, 32.88761, 43.09325, 56.46588,
];

enum Low {
    Nb(Nb),
    Sb(Box<Sb>),
}

pub(crate) struct Sb {
    low: Low,
    /// The upper band's frame (the low band's full frame) and subframe.
    frame: usize,
    sub: usize,
    ultra: bool,
    folding_gain: f32,
    submode: usize,
    first: bool,
    seed: u32,
    old_qlsp: [f32; ORDER],
    interp_qlpc: [f32; ORDER],
    mem_sp: [f32; ORDER],
    g0: [f32; QMF],
    g1: [f32; QMF],
    /// The last subframe's excitation (the synthesis runs a subframe behind).
    exc_buf: Vec<f32>,
    pi_gain: [f32; SUBS],
    exc_rms: [f32; SUBS],
    /// The low band's output, then its innovation (for folding).
    low_out: Vec<f32>,
    low_innov: Vec<f32>,
}

impl Sb {
    fn new(low: Low, frame: usize, ultra: bool) -> Sb {
        let sub = frame / SUBS;
        Sb {
            low,
            frame,
            sub,
            ultra,
            folding_gain: if ultra { 0.7 } else { 0.9 },
            submode: if ultra { 1 } else { 3 },
            first: true,
            seed: 1000,
            old_qlsp: [0.0; ORDER],
            interp_qlpc: [0.0; ORDER],
            mem_sp: [0.0; ORDER],
            g0: [0.0; QMF],
            g1: [0.0; QMF],
            exc_buf: vec![0.0; sub],
            pi_gain: [0.0; SUBS],
            exc_rms: [0.0; SUBS],
            low_out: vec![0.0; frame],
            low_innov: vec![0.0; frame],
        }
    }

    pub(crate) fn wideband() -> Sb {
        Sb::new(Low::Nb(Nb::new(true)), nb::FRAME, false)
    }

    pub(crate) fn ultra() -> Sb {
        Sb::new(Low::Sb(Box::new(Sb::wideband())), 2 * nb::FRAME, true)
    }

    pub(crate) fn full_frame(&self) -> usize {
        2 * self.frame
    }

    /// Decodes a frame into `out` (2 × the band's frame), its upper-band innovation into
    /// `innov_save` at even places when a wider band asks.
    fn decode_with(
        &mut self,
        b: &mut Bits,
        stereo: &mut Stereo,
        out: &mut [f32],
        mut innov_save: Option<&mut [f32]>,
        pi_out: Option<(&mut [f32; SUBS], &mut [f32; SUBS])>,
        dtx_out: Option<&mut bool>,
    ) -> Result<Frame, Error> {
        let mut low_pi = [0f32; SUBS];
        let mut low_rms = [0f32; SUBS];
        let mut dtx = false;
        let n = self.frame;
        let r = match &mut self.low {
            Low::Nb(d) => d.decode(
                b,
                stereo,
                &mut self.low_out,
                Some(LowBand {
                    innov: &mut self.low_innov,
                    pi_gain: &mut low_pi,
                    exc_rms: &mut low_rms,
                    dtx: &mut dtx,
                }),
            )?,
            Low::Sb(d) => d.decode_with(
                b,
                stereo,
                &mut self.low_out,
                Some(&mut self.low_innov),
                Some((&mut low_pi, &mut low_rms)),
                Some(&mut dtx),
            )?,
        };
        if let Frame::End = r {
            return Ok(Frame::End);
        }
        if let Some(d) = dtx_out {
            *d = dtx;
        }
        // The upper band's sub-mode: a set first bit says one follows.
        self.submode = if b.remaining() > 0 && b.peek() == 1 {
            b.read(1);
            b.read(3) as usize
        } else {
            0
        };
        let table = if self.ultra {
            [None, WB_SUBMODES[1], None, None, None, None, None, None]
        } else {
            WB_SUBMODES
        };
        let mut high = vec![0f32; n];
        let Some(mode) = table[self.submode] else {
            if self.submode != 0 {
                return Err(Error::Malformed("an invalid wideband mode".into()));
            }
            // No upper band: the filter rings out on almost nothing.
            high.fill(1e-15);
            self.first = true;
            nb::iir(&mut high, &self.interp_qlpc, &mut self.mem_sp);
            self.qmf(&high, out);
            if let Some(s) = innov_save {
                s[..n].fill(0.0);
            }
            if let Some((p, e)) = pi_out {
                *p = self.pi_gain;
                *e = self.exc_rms;
            }
            return Ok(Frame::Decoded);
        };
        let mut qlsp = [0f32; ORDER];
        for (i, q) in qlsp.iter_mut().enumerate() {
            *q = 0.3125 * i as f32 + 0.75;
        }
        let id = b.read(6) as usize;
        for (i, q) in qlsp.iter_mut().enumerate() {
            *q += 0.0039062 * f32::from(HIGH_LSP[id * ORDER + i]);
        }
        let id = b.read(6) as usize;
        for (i, q) in qlsp.iter_mut().enumerate() {
            *q += 0.0019531 * f32::from(HIGH_LSP2[id * ORDER + i]);
        }
        if self.first {
            self.old_qlsp = qlsp;
        }
        let sub = self.sub;
        let mut interp = [0f32; ORDER];
        let mut ak = [0f32; ORDER];
        for s in 0..SUBS {
            let offset = s * sub;
            if let Some(save) = innov_save.as_deref_mut() {
                save[2 * offset..2 * offset + 2 * sub].fill(0.0);
            }
            nb::lsp_interpolate(&self.old_qlsp, &qlsp, &mut interp, s, SUBS, LSP_MARGIN);
            nb::lsp_to_lpc(&interp, &mut ak);
            let mut rh = 1.0f32;
            self.pi_gain[s] = 1.0;
            for i in (0..ORDER).step_by(2) {
                rh += ak[i + 1] - ak[i];
                self.pi_gain[s] += ak[i] + ak[i + 1];
            }
            let rl = low_pi[s];
            let filter_ratio = (rl + 0.01) / (rh + 0.01);
            let mut exc = vec![0f32; sub];
            match mode.innov {
                None => {
                    let quant = b.read(5) as f32;
                    let g = (0.125 * (quant - 10.0)).exp() / filter_ratio;
                    for i in (0..sub).step_by(2) {
                        exc[i] = self.folding_gain * self.low_innov[offset + i] * g;
                        exc[i + 1] = -(self.folding_gain * self.low_innov[offset + i + 1] * g);
                    }
                }
                Some(innov) => {
                    let qgc = b.read(4) as usize;
                    let el = low_rms[s];
                    let mut gc = 0.87360 * GC_BOUND[qgc];
                    if sub == 80 {
                        // libspeex's rounded √2.
                        #[allow(clippy::approx_constant)]
                        {
                            gc *= 1.4142;
                        }
                    }
                    let scale = gc * el / filter_ratio;
                    nb::innovation(innov, &mut exc, b, &mut self.seed);
                    for v in exc.iter_mut() {
                        *v *= scale;
                    }
                    if mode.double_codebook {
                        let mut innov2 = vec![0f32; sub];
                        nb::innovation(innov, &mut innov2, b, &mut self.seed);
                        for (v, w) in exc.iter_mut().zip(innov2) {
                            *v += w * (0.4 * scale);
                        }
                    }
                }
            }
            if let Some(save) = innov_save.as_deref_mut() {
                for i in 0..sub {
                    save[2 * (offset + i)] = exc[i];
                }
            }
            let sp = &mut high[offset..offset + sub];
            sp.copy_from_slice(&self.exc_buf);
            nb::iir(sp, &self.interp_qlpc, &mut self.mem_sp);
            self.exc_buf.copy_from_slice(&exc);
            self.interp_qlpc = ak;
            self.exc_rms[s] = nb::rms(&self.exc_buf);
        }
        self.qmf(&high, out);
        self.old_qlsp = qlsp;
        self.first = false;
        if let Some((p, e)) = pi_out {
            *p = self.pi_gain;
            *e = self.exc_rms;
        }
        Ok(Frame::Decoded)
    }

    /// Joins the low band (`low_out`) and `high` into `out` (QMF synthesis).
    fn qmf(&mut self, high: &[f32], out: &mut [f32]) {
        let n = 2 * self.frame;
        let (n2, m2) = (n / 2, QMF / 2);
        let mut xx1 = vec![0f32; m2 + n2];
        let mut xx2 = vec![0f32; m2 + n2];
        for i in 0..n2 {
            xx1[i] = self.low_out[n2 - 1 - i];
            xx2[i] = high[n2 - 1 - i];
        }
        for i in 0..m2 {
            xx1[n2 + i] = self.g0[2 * i + 1];
            xx2[n2 + i] = self.g1[2 * i + 1];
        }
        let a = &H0;
        for i in (0..n2).step_by(2) {
            let (mut y0, mut y1, mut y2, mut y3) = (0f32, 0f32, 0f32, 0f32);
            let mut x10 = xx1[n2 - 2 - i];
            let mut x20 = xx2[n2 - 2 - i];
            for j in (0..m2).step_by(2) {
                let (a0, a1) = (a[2 * j], a[2 * j + 1]);
                let x11 = xx1[n2 - 1 + j - i];
                let x21 = xx2[n2 - 1 + j - i];
                y0 += a0 * (x11 - x21);
                y1 += a1 * (x11 + x21);
                y2 += a0 * (x10 - x20);
                y3 += a1 * (x10 + x20);
                let (a0, a1) = (a[2 * j + 2], a[2 * j + 3]);
                x10 = xx1[n2 + j - i];
                x20 = xx2[n2 + j - i];
                y0 += a0 * (x10 - x20);
                y1 += a1 * (x10 + x20);
                y2 += a0 * (x11 - x21);
                y3 += a1 * (x11 + x21);
            }
            out[2 * i] = 2.0 * y0;
            out[2 * i + 1] = 2.0 * y1;
            out[2 * i + 2] = 2.0 * y2;
            out[2 * i + 3] = 2.0 * y3;
        }
        for i in 0..m2 {
            self.g0[2 * i + 1] = xx1[i];
            self.g1[2 * i + 1] = xx2[i];
        }
    }

    pub(crate) fn decode(
        &mut self,
        b: &mut Bits,
        stereo: &mut Stereo,
        out: &mut [f32],
    ) -> Result<Frame, Error> {
        self.decode_with(b, stereo, out, None, None, None)
    }
}
