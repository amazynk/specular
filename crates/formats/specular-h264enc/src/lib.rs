//! An H.264 (AVC) encoder for Specular's slideshow export.
//!
//! It is tuned for what a slideshow is: long runs of static or slowly panning and zooming
//! pictures, fades, cross-fades and cuts. Three profiles (see [`Profile`]):
//!
//! * **Constrained Baseline** (CAVLC, I and P pictures): plays on anything.
//! * **Main**: CABAC entropy coding, explicitly weighted prediction for fades (a weight and
//!   offset per plane, estimated from the pictures), and B pictures for cross-fades. A B
//!   picture here predicts from two *past* pictures — the previous one and a long-term
//!   *anchor*, the picture before the cross-fade began — as `α·prev + β·anchor`, so a
//!   cross-fade between two stills costs almost nothing. Pictures are never reordered.
//! * **High** (the default): Main plus the adaptive 8×8 transform for inter macroblocks.
//!
//! Common to all:
//!
//! * **Decisions**: by rate-distortion cost (SSD + λ·bits, the bits counted by a CABAC or
//!   CAVLC coder that writes nothing) between the macroblock types, Intra16x16, Intra8x8
//!   (High) and Intra4x4, and for inter residuals between none, the 4×4 and the 8×8
//!   transform. Prediction modes within a type are picked by SATD (SA8D for 8×8) + λ·bits;
//!   Intra8x8 and Intra4x4 are abandoned once that proxy is 25 % above the best so far.
//! * **Inter pictures**: 16×16 partitions with a quarter-sample motion search (predictor,
//!   neighbour, co-located and caller-hint candidates, coarse-to-fine diamond in full
//!   samples, then half and quarter sample refinement by SATD), skipped macroblocks whenever
//!   the skip prediction leaves nothing to code, and intra macroblocks where they predict
//!   better. A picture identical to the previous source is coded as one run of skips.
//!   [`Encoder::encode_hinted`] takes the caller's known global zoom and pan as a motion
//!   candidate for every macroblock.
//! * **Residuals**: the 4×4 (and 8×8) integer transforms with the DC Hadamard transforms,
//!   dead-zone quantisation (⅓ intra, ⅙ inter) or, with CABAC, trellis quantisation over
//!   the level coding contexts, and decimation of isolated ±1 levels in inter blocks.
//! * **Rate**: constant QP; the caller picks it from a quality preset.
//! * **Reconstruction** is the decoder's, bit for bit (the scaling and inverse transforms of
//!   8.5, weighted prediction, and the in-loop deblocking filter of 8.7 over the whole
//!   picture), so there is no drift; the tests compare against an independent decoder.
//! * **Parallelism**: a picture is split into slices of whole macroblock rows encoded on the
//!   rayon pool; the deblocking filter (which runs across slice edges) and the half-sample
//!   reference planes follow.
//!
//! Output pictures are access units in AVCC form (4-byte big-endian NAL lengths), ready for
//! an MP4 sample; [`Encoder::avcc`] is the matching `avcC` record. [`annex_b`] converts for
//! raw `.h264` files.

#![feature(portable_simd)]

mod bits;
mod cabac;
mod cabac_tables;
mod cavlc;
mod deblock;
mod dsp;
mod mb;
mod mc;
mod params;
mod pred;
mod tables;
mod transform;
mod trellis;
mod weights;

use std::sync::Arc;

use rayon::prelude::*;

pub use mb::MotionHint;
pub use params::{Profile, Signal};

use mb::{Band, MbInfo, Picture, SliceHeader, SliceKind, SliceStats};
use mc::RefFrame;
use params::Params;
use weights::{Thumb, Weights};

/// What the caller knows about a picture beyond its samples.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hints {
    /// The global motion from the previous picture (a Ken Burns zoom and pan).
    pub motion: Option<MotionHint>,
    /// Whether a fade or cross-fade runs: weighted prediction and B pictures are only
    /// considered then. `None`: unknown — fades looked for always, cross-fades (B pictures)
    /// unless `motion` moves the picture.
    pub fading: Option<bool>,
}

/// Encoder settings.
#[derive(Clone, Debug)]
pub struct Config {
    /// Picture size in samples; both must be even.
    pub width: u32,
    pub height: u32,
    /// Frame rate `fps_num / fps_den`.
    pub fps_num: u32,
    pub fps_den: u32,
    /// Quantisation parameter, 10..=51 (lower is better quality).
    pub qp: u8,
    /// How much finer than `qp` I pictures are quantised: what they gain is inherited by every
    /// picture predicted from them (most of a slideshow's are skipped). Default 3.
    pub i_qp_offset: u8,
    /// Longest distance between IDR pictures, in frames.
    pub keyint: u32,
    /// Slices per picture (0: one per worker thread, at least four macroblock rows each).
    pub slices: usize,
    /// Colour signalling for the VUI.
    pub signal: Signal,
    pub profile: Profile,
    /// `seq_parameter_set_id` and `pic_parameter_set_id` (0..=31). A stream that switches to
    /// this encoder from another one at an IDR keeps both sets apart with different ids.
    pub param_set_id: u8,
    /// Rate-distortion optimised quantisation (trellis; Main and High). Default on.
    pub trellis: bool,
}

impl Config {
    pub fn new(width: u32, height: u32, fps_num: u32, fps_den: u32, qp: u8) -> Config {
        Config {
            width,
            height,
            fps_num,
            fps_den,
            qp,
            i_qp_offset: 3,
            keyint: (fps_num / fps_den.max(1)).max(1) * 5,
            slices: 0,
            signal: Signal::BT709,
            profile: Profile::default(),
            param_set_id: 0,
            trellis: true,
        }
    }
}

/// Why an encoder could not be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Zero, odd or out-of-range size, frame rate or QP.
    Unsupported(&'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported(s) => write!(f, "unsupported H.264 encoder settings: {s}"),
        }
    }
}

impl std::error::Error for Error {}

/// A 4:2:0 8-bit picture: luma `width`×`height`, chroma half size each way.
#[derive(Clone, Copy, Debug)]
pub struct Frame<'a> {
    pub y: &'a [u8],
    pub u: &'a [u8],
    pub v: &'a [u8],
    pub y_stride: usize,
    pub c_stride: usize,
}

/// One coded picture.
#[derive(Clone, Debug, Default)]
pub struct Packet {
    /// NAL units with 4-byte big-endian lengths (an MP4 sample).
    pub data: Vec<u8>,
    /// An IDR picture (a sync sample).
    pub key: bool,
    pub stats: FrameStats,
}

/// Macroblock counts of a coded picture.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameStats {
    pub intra: u32,
    pub inter: u32,
    pub skip: u32,
    /// Coded as a B picture (a cross-fade step).
    pub b: bool,
    /// Coded with non-default explicit weights (a fade).
    pub weighted: bool,
}

/// A reference picture with the statistics weighted prediction is estimated from.
#[derive(Clone)]
struct Ref {
    frame: Arc<RefFrame>,
    thumb: Option<Arc<[Thumb; 3]>>,
}

/// The encoder: feed pictures in display order, get one [`Packet`] per picture (no
/// reordering, no delay).
pub struct Encoder {
    cfg: Config,
    params: Params,
    mb_w: usize,
    mb_h: usize,
    /// Macroblock-aligned source of the current and previous picture.
    src: [Vec<u8>; 3],
    prev_src: [Vec<u8>; 3],
    have_prev: bool,
    /// Reconstruction (deblocked after each picture).
    rec: [Vec<u8>; 3],
    /// The previous picture (RefPicList0[0]).
    prev: Option<Ref>,
    /// The long-term anchor (RefPicList1[0] of B pictures; Main and High).
    anchor: Option<Ref>,
    /// The previous picture is the long-term one (an IDR picture): no B picture yet.
    prev_is_lt: bool,
    info: Vec<MbInfo>,
    prev_info: Vec<MbInfo>,
    frame_num: u32,
    since_idr: u32,
    idr_count: u32,
}

impl Encoder {
    pub fn new(cfg: Config) -> Result<Encoder, Error> {
        if cfg.width == 0 || cfg.height == 0 || cfg.width % 2 == 1 || cfg.height % 2 == 1 {
            return Err(Error::Unsupported("size must be even and non-zero"));
        }
        if cfg.width > 8192 || cfg.height > 8192 {
            return Err(Error::Unsupported("size above 8192"));
        }
        if cfg.fps_num == 0 || cfg.fps_den == 0 {
            return Err(Error::Unsupported("frame rate"));
        }
        if !(10..=51).contains(&cfg.qp) {
            return Err(Error::Unsupported("qp outside 10..=51"));
        }
        if cfg.param_set_id > 31 {
            return Err(Error::Unsupported("parameter set id above 31"));
        }
        let params = Params::new(
            cfg.width,
            cfg.height,
            cfg.fps_num,
            cfg.fps_den,
            cfg.qp,
            cfg.signal,
            cfg.profile,
        );
        let params = Params {
            param_set_id: cfg.param_set_id,
            ..params
        };
        let mb_w = params.mb_w as usize;
        let mb_h = params.mb_h as usize;
        let ly = mb_w * 16 * mb_h * 16;
        let lc = ly / 4;
        let planes = || [vec![0u8; ly], vec![128u8; lc], vec![128u8; lc]];
        Ok(Encoder {
            cfg,
            params,
            mb_w,
            mb_h,
            src: planes(),
            prev_src: planes(),
            have_prev: false,
            rec: planes(),
            prev: None,
            anchor: None,
            prev_is_lt: false,
            info: vec![MbInfo::default(); mb_w * mb_h],
            prev_info: vec![MbInfo::default(); mb_w * mb_h],
            frame_num: 0,
            since_idr: 0,
            idr_count: 0,
        })
    }

    /// The `avcC` record (AVCDecoderConfigurationRecord).
    pub fn avcc(&self) -> Vec<u8> {
        self.params.avcc()
    }

    /// The SPS NAL unit.
    pub fn sps(&self) -> Vec<u8> {
        self.params.sps()
    }

    /// The PPS NAL unit.
    pub fn pps(&self) -> Vec<u8> {
        self.params.pps()
    }

    pub fn level_idc(&self) -> u8 {
        self.params.level_idc
    }

    pub fn config(&self) -> &Config {
        &self.cfg
    }

    /// The reconstruction of the last picture (cropped to the picture size): what a decoder
    /// outputs. Luma, then the two chroma planes, each tightly packed.
    pub fn reconstruction(&self) -> [Vec<u8>; 3] {
        let (w, h) = (self.cfg.width as usize, self.cfg.height as usize);
        let crop = |p: &[u8], pw: usize, cw: usize, ch: usize| -> Vec<u8> {
            let mut o = Vec::with_capacity(cw * ch);
            for r in 0..ch {
                o.extend_from_slice(&p[r * pw..r * pw + cw]);
            }
            o
        };
        [
            crop(&self.rec[0], self.mb_w * 16, w, h),
            crop(&self.rec[1], self.mb_w * 8, w / 2, h / 2),
            crop(&self.rec[2], self.mb_w * 8, w / 2, h / 2),
        ]
    }

    /// Copies `f` into the macroblock-aligned source, replicating the right and bottom edges.
    fn load(&mut self, f: &Frame<'_>) {
        let (w, h) = (self.cfg.width as usize, self.cfg.height as usize);
        let geoms = [
            (f.y, f.y_stride, w, h, self.mb_w * 16, self.mb_h * 16),
            (f.u, f.c_stride, w / 2, h / 2, self.mb_w * 8, self.mb_h * 8),
            (f.v, f.c_stride, w / 2, h / 2, self.mb_w * 8, self.mb_h * 8),
        ];
        for (dst, (s, ss, sw, sh, dw, dh)) in self.src.iter_mut().zip(geoms) {
            dst.par_chunks_mut(dw)
                .take(dh)
                .enumerate()
                .for_each(|(r, row)| {
                    let sr = r.min(sh - 1);
                    let line = &s[sr * ss..sr * ss + sw];
                    row[..sw].copy_from_slice(line);
                    let last = line[sw - 1];
                    row[sw..].fill(last);
                });
        }
    }

    /// Thumbnails of the visible part of macroblock-aligned planes.
    fn thumbs(&self, p: &[Vec<u8>; 3]) -> [Thumb; 3] {
        let (w, h) = (self.cfg.width as usize, self.cfg.height as usize);
        let (ls, cs) = (self.mb_w * 16, self.mb_w * 8);
        [
            Thumb::new(&p[0], ls, w, h),
            Thumb::new(&p[1], cs, w / 2, h / 2),
            Thumb::new(&p[2], cs, w / 2, h / 2),
        ]
    }

    /// Encodes the next picture; `force_idr` starts a new coded video sequence here (cuts).
    pub fn encode(&mut self, f: &Frame<'_>, force_idr: bool) -> Packet {
        self.encode_hinted(f, force_idr, None)
    }

    /// [`Encoder::encode`] with the global motion from the previous picture when the caller
    /// knows it (a Ken Burns zoom and pan): a motion candidate for every macroblock. Only a
    /// hint — a wrong one costs search time, never correctness.
    pub fn encode_hinted(
        &mut self,
        f: &Frame<'_>,
        force_idr: bool,
        hint: Option<MotionHint>,
    ) -> Packet {
        self.encode_with(
            f,
            force_idr,
            Hints {
                motion: hint,
                fading: None,
            },
        )
    }

    /// [`Encoder::encode`] with what the caller knows about the picture (see [`Hints`]).
    pub fn encode_with(&mut self, f: &Frame<'_>, force_idr: bool, hints: Hints) -> Packet {
        let hint = hints.motion;
        // Fades are looked for unless the caller rules them out. Cross-fades (B pictures) not
        // in a picture known to move: a pan's pictures fit `α·prev + β·older` as well as a
        // cross-fade's do (a linear extrapolation of the motion), and coding them as B
        // pictures more than doubles a Ken Burns show. Brightness changes (weighted P) do not
        // look like motion.
        let moving = hint.is_some_and(|h| {
            let (w, h_) = (self.mb_w - 1, self.mb_h - 1);
            [(0, 0), (w, 0), (0, h_), (w, h_)]
                .iter()
                .any(|&(x, y)| h.mv(x, y) != [0, 0])
        });
        let may_weight = hints.fading.unwrap_or(true);
        let may_cross = hints.fading.unwrap_or(!moving);
        self.load(f);
        let idr = force_idr || self.prev.is_none() || self.since_idr >= self.cfg.keyint;
        let same = self.have_prev && self.src == self.prev_src;
        std::mem::swap(&mut self.src, &mut self.prev_src);
        // prev_src now holds the current picture; src the one before (reused next time).
        self.have_prev = true;
        if idr {
            self.frame_num = 0;
            self.since_idr = 0;
        } else {
            self.frame_num = (self.frame_num + 1) % (1 << params::LOG2_MAX_FRAME_NUM);
        }
        let adv = self.cfg.profile.advanced();
        // Picture type and weights.
        let mut kind = if idr { SliceKind::I } else { SliceKind::P };
        let mut weights = None;
        if adv && !idr {
            // Main/High P and B slices always carry a weight table.
            weights = Some(Weights::identity(0, 0));
            if !same && may_weight {
                let cur = self.thumbs(&self.prev_src);
                let prev = self.prev.as_ref().and_then(|r| r.thumb.as_deref());
                let anchor = self.anchor.as_ref().and_then(|r| r.thumb.as_deref());
                let b = match (prev, anchor) {
                    (Some(p), Some(a)) if may_cross && !self.prev_is_lt => {
                        weights::estimate_b(&cur, p, a)
                    }
                    _ => None,
                };
                if let Some(w) = b {
                    kind = SliceKind::B;
                    weights = Some(w);
                } else if let Some(w) = prev.and_then(|p| weights::estimate_p(&cur, p)) {
                    weights = Some(w);
                }
            }
        }
        let hdr = SliceHeader {
            idr,
            idr_pic_id: self.idr_count % 2,
            frame_num: self.frame_num,
            init_qp: self.params.init_qp,
            pps_id: self.params.param_set_id,
            long_term: adv && idr,
            mmco_prev: adv && kind == SliceKind::P && !self.prev_is_lt,
        };
        if idr {
            self.idr_count += 1;
        }
        self.since_idr += 1;
        if same && !idr {
            return self.repeat_picture(hdr, weights);
        }
        let mut out = Vec::with_capacity(1 << 16);
        let mut stats = self.code_picture(hdr, kind, weights, hint, &mut out);
        stats.b = kind == SliceKind::B;
        stats.weighted = weights.is_some_and(|w| !(w.list_is_default(0) && w.list_is_default(1)));
        Packet {
            data: out,
            key: idr,
            stats,
        }
    }

    /// Updates the reference structure after a picture of `kind` reconstructed as `new`.
    fn push_ref(&mut self, kind: SliceKind, new: Ref) {
        match kind {
            SliceKind::I => {
                self.anchor = None;
                self.prev_is_lt = self.cfg.profile.advanced();
            }
            SliceKind::P => {
                // MMCO 3 (or the IDR already long-term): the previous picture is the anchor.
                self.anchor = self.prev.take();
                self.prev_is_lt = false;
            }
            SliceKind::B => {}
        }
        self.prev = Some(new);
    }

    /// A picture equal to its reference: one slice of skipped macroblocks.
    fn repeat_picture(&mut self, hdr: SliceHeader, weights: Option<Weights>) -> Packet {
        let n = (self.mb_w * self.mb_h) as u32;
        let rbsp = mb::skip_slice(
            0,
            n,
            weights.as_ref(),
            self.cfg.profile.advanced(),
            self.cfg.qp,
            &hdr,
        );
        let mut data = Vec::new();
        bits::push_nal_avcc(&mut data, 2, 1, &rbsp);
        std::mem::swap(&mut self.info, &mut self.prev_info);
        self.info.fill(MbInfo::default());
        let same = self.prev.clone().expect("a reference");
        self.push_ref(SliceKind::P, same);
        Packet {
            data,
            key: false,
            stats: FrameStats {
                skip: n,
                ..FrameStats::default()
            },
        }
    }

    fn slice_rows(&self) -> Vec<(usize, usize)> {
        let n = if self.cfg.slices > 0 {
            self.cfg.slices
        } else {
            rayon::current_num_threads()
        };
        let n = n.clamp(1, (self.mb_h / 4).max(1));
        (0..n)
            .map(|i| (i * self.mb_h / n, (i + 1) * self.mb_h / n))
            .filter(|(a, b)| b > a)
            .collect()
    }

    /// The quantiser of a picture of `kind`: I pictures finer by [`Config::i_qp_offset`].
    fn picture_qp(&self, kind: SliceKind) -> u8 {
        if kind == SliceKind::I {
            self.cfg.qp.saturating_sub(self.cfg.i_qp_offset).max(10)
        } else {
            self.cfg.qp
        }
    }

    fn code_picture(
        &mut self,
        hdr: SliceHeader,
        kind: SliceKind,
        weights: Option<Weights>,
        hint: Option<MotionHint>,
        out: &mut Vec<u8>,
    ) -> FrameStats {
        std::mem::swap(&mut self.info, &mut self.prev_info);
        let rows = self.slice_rows();
        let refs = match kind {
            SliceKind::I => [None, None],
            SliceKind::P => [self.prev.as_ref().map(|r| &*r.frame), None],
            SliceKind::B => [
                self.prev.as_ref().map(|r| &*r.frame),
                self.anchor.as_ref().map(|r| &*r.frame),
            ],
        };
        let adv = self.cfg.profile.advanced();
        let qp = self.picture_qp(kind);
        let pic = Picture {
            mb_w: self.mb_w,
            src: [&self.prev_src[0], &self.prev_src[1], &self.prev_src[2]],
            kind,
            refs,
            weights,
            prev_info: (!hdr.idr).then_some(&self.prev_info[..]),
            qp,
            cabac: adv,
            t8: self.cfg.profile == Profile::High,
            trellis: self.cfg.trellis,
            hint,
        };
        let ls = self.mb_w * 16;
        let cs = self.mb_w * 8;
        // Split the reconstruction and info into the slices' bands.
        let mut bands = Vec::with_capacity(rows.len());
        {
            let [ry, ru, rv] = &mut self.rec;
            let (mut y, mut u, mut v) = (&mut ry[..], &mut ru[..], &mut rv[..]);
            let mut info = &mut self.info[..];
            for &(a, b) in &rows {
                let n = b - a;
                let (by, ty) = y.split_at_mut(n * 16 * ls);
                let (bu, tu) = u.split_at_mut(n * 8 * cs);
                let (bv, tv) = v.split_at_mut(n * 8 * cs);
                let (bi, ti) = info.split_at_mut(n * self.mb_w);
                bands.push(Band {
                    mb_y0: a,
                    mb_y1: b,
                    y: by,
                    u: bu,
                    v: bv,
                    info: bi,
                });
                y = ty;
                u = tu;
                v = tv;
                info = ti;
            }
        }
        let results: Vec<(Vec<u8>, SliceStats)> = bands
            .par_iter_mut()
            .map(|band| mb::encode_slice(&pic, band, hdr))
            .collect();
        drop(bands);
        let mut stats = FrameStats::default();
        let (nal_ref, nal_type) = if hdr.idr { (3, 5) } else { (2, 1) };
        for (rbsp, s) in &results {
            bits::push_nal_avcc(out, nal_ref, nal_type, rbsp);
            stats.intra += s.intra;
            stats.inter += s.inter;
            stats.skip += s.skip;
        }
        let [ry, ru, rv] = &mut self.rec;
        deblock::deblock(ry, ru, rv, self.mb_w, self.mb_h, &self.info, qp);
        let frame = Arc::new(RefFrame::build(ry, ru, rv, ls, self.mb_h * 16));
        let thumb = adv.then(|| Arc::new(self.thumbs(&self.rec)));
        self.push_ref(kind, Ref { frame, thumb });
        stats
    }
}

/// Converts AVCC access units to an Annex B byte stream (start codes), with the parameter
/// sets before every IDR picture.
pub fn annex_b(sps: &[u8], pps: &[u8], packets: &[Packet]) -> Vec<u8> {
    let mut out = Vec::new();
    for p in packets {
        if p.key {
            for ps in [sps, pps] {
                out.extend_from_slice(&[0, 0, 0, 1]);
                out.extend_from_slice(ps);
            }
        }
        let mut d = &p.data[..];
        while d.len() >= 4 {
            let n = u32::from_be_bytes([d[0], d[1], d[2], d[3]]) as usize;
            out.extend_from_slice(&[0, 0, 0, 1]);
            out.extend_from_slice(&d[4..4 + n]);
            d = &d[4 + n..];
        }
    }
    out
}

/// A QP for a quality preset in 0.0 (smallest) ..= 1.0 (best).
pub fn qp_for_quality(q: f32) -> u8 {
    (32.0 - q.clamp(0.0, 1.0) * 14.0).round() as u8
}
