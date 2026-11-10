//! The slideshow export's H.264 encoder: the GPU's hardware encoder (VA-API on Linux, Media
//! Foundation on Windows, through `specular-hwdec`) or our software encoder
//! (`specular-h264enc`), as the path selector
//! decides (`Task::VideoEncode`: Automatic takes the hardware when it is there and faster,
//! "CPU only" never does).
//!
//! The hardware is checked once per driver: its first picture is decoded by our decoder and
//! must come within a few dB of the software encoder's PSNR at the same QP. After that, one P
//! picture every [`SPOT_EVERY_S`] seconds is checked the same way (its GOP decoded up to it),
//! which catches a reconstruction that drifts from what the bitstream says. A failed check or
//! any error, even mid-export, blocks the path and hands the rest of the clip to the software
//! encoder, starting at an IDR picture. The two encoders' parameter sets have different ids (hardware
//! 0, software 1), so one `avcC` holds both and the clip stays one track; [`VideoEncoder::avcc`]
//! is final only after the last picture.

use specular_core::paths::{Choice, Engine, Reason, Request, Task, Trust, selector};
use specular_h264enc::{Config, Encoder, Frame, Hints, MotionHint, Profile, Signal};
use specular_hwdec::encode as hw;
use specular_log::{info, warn};

/// The hardware's QP for the export's QP. At the same QP the hardware codes stills and Ken
/// Burns pans finer than the software encoder (0.4–2.5 dB on 1080p slideshows); one step up
/// matches the software's quality on cross-fade shows and stays above it on pans.
const HW_QP_OFFSET: i8 = 1;

/// Added to the hardware's QP while a cross-fade runs. It has no weighted prediction or B
/// pictures, so each fading picture re-codes most of the frame (on a cross-fade show the
/// fades were 90% of the bytes, and 20% more than the software's). Those pictures are on
/// screen for a few hundredths of a second each, so they are coded coarser. With this and
/// [`HW_QP_OFFSET`] the hardware matches the software's size and mean PSNR on cross-fade
/// shows (fading pictures about 1.2 dB lower, the rest 0.4 dB higher).
const HW_FADE_QP_OFFSET: i8 = 3;

/// The hardware may come this far below the software encoder's luma PSNR on the check
/// picture (dB).
const VERIFY_MARGIN_DB: f64 = 4.0;

/// Further allowance on a spot-checked picture per step its QP is above the stream's (dB).
const VERIFY_DB_PER_QP: f64 = 0.6;

/// A P picture of the hardware's is spot-checked about this often (seconds of video).
pub const SPOT_EVERY_S: u32 = 10;

/// The spot-checked picture is this many pictures after its GOP's IDR picture (the decode
/// it costs: about 0.1 s per check at 1080p).
const SPOT_DEPTH: u32 = 4;

/// A hardware encoder as [`VideoEncoder`] uses it (a trait so tests can fail on purpose).
pub trait HwBackend: Send {
    /// SPS and PPS NAL units (no start code); ids 0.
    fn sps(&self) -> &[u8];
    fn pps(&self) -> &[u8];
    /// One picture at quantiser `qp` as AVCC NAL units, and whether it is an IDR picture.
    fn encode(&mut self, f: &Frame<'_>, force_idr: bool, qp: u8)
    -> Result<(Vec<u8>, bool), String>;
    /// For logs.
    fn describe(&self) -> String;
}

impl HwBackend for hw::H264Encoder {
    fn sps(&self) -> &[u8] {
        hw::H264Encoder::sps(self)
    }
    fn pps(&self) -> &[u8] {
        hw::H264Encoder::pps(self)
    }
    fn encode(
        &mut self,
        f: &Frame<'_>,
        force_idr: bool,
        qp: u8,
    ) -> Result<(Vec<u8>, bool), String> {
        hw::H264Encoder::encode_at(self, f.y, f.u, f.v, f.y_stride, f.c_stride, force_idr, qp)
            .map(|p| (p.data, p.key))
            .map_err(|e| e.to_string())
    }
    fn describe(&self) -> String {
        format!("{:?} on {}", self.profile(), self.device())
    }
}

/// One coded picture: AVCC NAL units.
#[derive(Clone, Debug)]
pub struct Sample {
    pub data: Vec<u8>,
    pub key: bool,
}

/// What the caller knows about a picture.
#[derive(Clone, Copy, Debug, Default)]
pub struct Cue {
    /// An IDR picture (a cut).
    pub force_idr: bool,
    /// The motion the picture was made with (Ken Burns; the software encoder's
    /// [`Encoder::encode_hinted`], the hardware does its own search).
    pub hint: Option<MotionHint>,
    /// A fade or cross-fade runs.
    pub fading: bool,
}

/// The hardware's settings: [`Tuning::default`] in the export, others in benchmarks and tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tuning {
    /// Added to the export's QP.
    pub qp_offset: i8,
    /// Added on top while fading.
    pub fade_qp_offset: i8,
    /// Spot-check a P picture after this many pictures (0: never; `None`: [`SPOT_EVERY_S`]).
    pub spot_every: Option<u32>,
}

impl Default for Tuning {
    fn default() -> Tuning {
        Tuning {
            qp_offset: HW_QP_OFFSET,
            fade_qp_offset: HW_FADE_QP_OFFSET,
            spot_every: None,
        }
    }
}

/// The hardware's pictures since its last IDR picture while a spot check is due in this GOP.
#[derive(Default)]
struct Spot {
    /// Pictures between checks.
    every: u64,
    /// Pictures since the last check (or the first picture).
    since: u64,
    /// A check is due: from the next IDR picture, collecting its GOP.
    armed: bool,
    gop: Vec<Vec<u8>>,
}

/// Which encoder made the samples so far.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Used {
    Hardware,
    Software,
    /// The hardware, then the software after an error.
    Both,
}

/// The export's encoder (see the module docs).
pub struct VideoEncoder {
    cfg: Config,
    choice: Choice,
    hw: Option<Box<dyn HwBackend>>,
    /// The hardware still has to pass its check.
    verify: bool,
    /// The hardware's parameter sets, once it wrote a sample.
    hw_sets: Option<(Vec<u8>, Vec<u8>)>,
    sw: Option<Encoder>,
    sw_samples: u32,
    /// Why the hardware stopped, if it did.
    fell_back: Option<String>,
    tuning: Tuning,
    spot: Spot,
    /// Spot checks passed.
    spot_checks: u32,
}

/// Whether the hardware can encode what `cfg` asks for: its profiles are Main and High, its
/// VUI says BT.709 limited range.
fn hw_can_take(cfg: &Config) -> bool {
    cfg.profile != Profile::Baseline && cfg.signal == Signal::BT709 && cfg.param_set_id == 0
}

fn offset_qp(qp: u8, offset: i16) -> u8 {
    (i16::from(qp) + offset).clamp(10, 51) as u8
}

/// The hardware's stream for `cfg` (QP `tuning.qp_offset` from the export's).
pub fn hw_config(cfg: &Config, tuning: &Tuning) -> hw::EncodeConfig {
    hw::EncodeConfig {
        width: cfg.width,
        height: cfg.height,
        fps_num: cfg.fps_num,
        fps_den: cfg.fps_den,
        qp: offset_qp(cfg.qp, i16::from(tuning.qp_offset)),
        keyint: cfg.keyint,
    }
}

impl VideoEncoder {
    /// The selector's choice of encoder for `cfg` (the software encoder when the hardware
    /// can't open this stream).
    pub fn new(cfg: Config) -> Result<VideoEncoder, String> {
        let sel = selector();
        let mut choice = sel.choose(Task::VideoEncode, &request(&cfg));
        // Automatic leaves an unchecked path to background exploration, which an export
        // (one long request) would never reach; its check costs one picture here, so an
        // unchecked encoder predicted faster is tried right away.
        if choice.path == 0
            && !matches!(choice.reason, Reason::Forced | Reason::CpuOnly)
            && sel.trust(Task::VideoEncode, 1) == Trust::Unverified
            && sel
                .estimate(Task::VideoEncode, 1, choice.key, choice.mpx)
                .wall_us
                < sel
                    .estimate(Task::VideoEncode, 0, choice.key, choice.mpx)
                    .wall_us
        {
            choice = Choice {
                path: 1,
                engine: Engine::HwEncoder,
                reason: Reason::Faster,
                verify: true,
                ..choice
            };
        }
        let hw: Option<Box<dyn HwBackend>> =
            if choice.engine == Engine::HwEncoder && hw_can_take(&cfg) {
                match hw::H264Encoder::new(&hw_config(&cfg, &Tuning::default())) {
                    Ok(e) => Some(Box::new(e)),
                    Err(specular_hwdec::Error::Unsupported(why)) => {
                        info!("video export: the hardware encoder can't take this: {why}");
                        None
                    }
                    Err(e) => {
                        selector().block(Task::VideoEncode, Engine::HwEncoder, &e.to_string());
                        None
                    }
                }
            } else {
                None
            };
        Self::with(cfg, choice, hw, Tuning::default())
    }

    /// An encoder with the given hardware backend (`None`: software only), checked as the
    /// selector's choice says. For tests.
    pub fn with_backend(
        cfg: Config,
        hw: Option<Box<dyn HwBackend>>,
        verify: bool,
    ) -> Result<VideoEncoder, String> {
        Self::with_tuning(cfg, hw, verify, Tuning::default())
    }

    /// [`VideoEncoder::with_backend`] with the hardware's settings (the backend made with
    /// [`hw_config`] of the same tuning). For benchmarks and tests.
    pub fn with_tuning(
        cfg: Config,
        hw: Option<Box<dyn HwBackend>>,
        verify: bool,
        tuning: Tuning,
    ) -> Result<VideoEncoder, String> {
        let mut choice = selector().calibration(
            Task::VideoEncode,
            usize::from(hw.is_some()),
            u64::from(cfg.width) * u64::from(cfg.height),
            0,
        );
        choice.verify = verify;
        Self::with(cfg, choice, hw, tuning)
    }

    fn with(
        cfg: Config,
        choice: Choice,
        hw: Option<Box<dyn HwBackend>>,
        tuning: Tuning,
    ) -> Result<VideoEncoder, String> {
        let sw = if hw.is_none() {
            Some(Encoder::new(cfg.clone()).map_err(|e| e.to_string())?)
        } else {
            // The fallback must be possible: check the settings now.
            Encoder::new(Config {
                width: 16,
                height: 16,
                ..cfg.clone()
            })
            .map_err(|e| e.to_string())?;
            None
        };
        if let Some(h) = &hw {
            info!("video export: hardware H.264 encoder ({})", h.describe());
        }
        let fps = f64::from(cfg.fps_num) / f64::from(cfg.fps_den.max(1));
        let every = tuning
            .spot_every
            .map_or((f64::from(SPOT_EVERY_S) * fps).round() as u64, u64::from);
        Ok(VideoEncoder {
            tuning,
            spot: Spot {
                every,
                ..Spot::default()
            },
            spot_checks: 0,
            verify: choice.verify,
            cfg,
            choice,
            hw,
            hw_sets: None,
            sw,
            sw_samples: 0,
            fell_back: None,
        })
    }

    /// The engine encoding the next picture.
    pub fn engine(&self) -> Engine {
        if self.hw.is_some() {
            Engine::HwEncoder
        } else {
            Engine::Cpu
        }
    }

    /// Which encoder made the samples so far.
    pub fn used(&self) -> Used {
        match (
            self.hw_sets.is_some(),
            self.sw_samples > 0 || self.hw.is_none(),
        ) {
            (true, true) => Used::Both,
            (true, false) => Used::Hardware,
            _ => Used::Software,
        }
    }

    /// Spot checks the hardware passed.
    pub fn spot_checks(&self) -> u32 {
        self.spot_checks
    }

    /// Why the hardware encoder was given up mid-export.
    pub fn fell_back(&self) -> Option<&str> {
        self.fell_back.as_deref()
    }

    /// The `avcC` record for the samples so far: the parameter sets of every encoder that
    /// made one (before the first picture, the current encoder's).
    pub fn avcc(&self) -> Vec<u8> {
        let mut sps: Vec<Vec<u8>> = Vec::new();
        let mut pps: Vec<Vec<u8>> = Vec::new();
        if let Some((s, p)) = &self.hw_sets {
            sps.push(s.clone());
            pps.push(p.clone());
        } else if let Some(h) = &self.hw {
            sps.push(h.sps().to_vec());
            pps.push(h.pps().to_vec());
        }
        if let Some(e) = self
            .sw
            .as_ref()
            .filter(|_| self.sw_samples > 0 || sps.is_empty())
        {
            sps.push(e.sps());
            pps.push(e.pps());
        }
        let sps: Vec<&[u8]> = sps.iter().map(Vec::as_slice).collect();
        let pps: Vec<&[u8]> = pps.iter().map(Vec::as_slice).collect();
        hw::avcc(&sps, &pps)
    }

    /// Encodes one picture (display order; IDR when `force_idr`, at the first picture and
    /// every `keyint`).
    pub fn encode(&mut self, f: &Frame<'_>, force_idr: bool) -> Sample {
        self.encode_cued(
            f,
            Cue {
                force_idr,
                ..Cue::default()
            },
        )
    }

    /// [`VideoEncoder::encode`] with the motion the picture was made with (the software
    /// encoder's [`Encoder::encode_hinted`]; the hardware does its own search).
    pub fn encode_hinted(
        &mut self,
        f: &Frame<'_>,
        force_idr: bool,
        hint: Option<MotionHint>,
    ) -> Sample {
        self.encode_cued(
            f,
            Cue {
                force_idr,
                hint,
                fading: false,
            },
        )
    }

    /// One picture with what the caller knows about it.
    pub fn encode_cued(&mut self, f: &Frame<'_>, cue: Cue) -> Sample {
        let Cue {
            force_idr,
            hint,
            fading,
        } = cue;
        if let Some(mut h) = self.hw.take() {
            let run = selector().start(&self.choice);
            let base = i16::from(self.tuning.qp_offset);
            let fade = if fading {
                i16::from(self.tuning.fade_qp_offset)
            } else {
                0
            };
            let qp = offset_qp(self.cfg.qp, base + fade);
            match h.encode(f, force_idr, qp) {
                Ok((data, key)) => {
                    let checked = if self.verify {
                        drop(run);
                        self.check(h.as_ref(), f, &data)
                    } else {
                        run.finish();
                        self.spot_check(h.as_ref(), f, &data, key, qp)
                    };
                    match checked {
                        Ok(()) => {
                            if self.hw_sets.is_none() {
                                self.hw_sets = Some((h.sps().to_vec(), h.pps().to_vec()));
                            }
                            self.hw = Some(h);
                            return Sample { data, key };
                        }
                        Err(why) => {
                            warn!("video export: the hardware encoder failed its check: {why}");
                            if self.hw_sets.is_some() {
                                // A spot check mid-export: as an error.
                                selector().block(Task::VideoEncode, Engine::HwEncoder, &why);
                            }
                            self.fell_back = Some(why);
                        }
                    }
                }
                Err(why) => {
                    warn!(
                        "video export: the hardware encoder failed, continuing on the CPU: {why}"
                    );
                    run.fail(&why);
                    self.fell_back = Some(why);
                }
            }
        }
        let fresh = self.sw.is_none();
        let sw = match &mut self.sw {
            Some(e) => e,
            None => {
                // Taking over from the hardware: its sets keep id 0.
                let id = u8::from(self.hw_sets.is_some());
                let cfg = Config {
                    param_set_id: id,
                    ..self.cfg.clone()
                };
                self.sw
                    .insert(Encoder::new(cfg).expect("settings checked in new"))
            }
        };
        let run = selector().start(&self.choice.reference());
        // `fading` false may only mean the caller does not say (`encode`): the encoder then
        // looks for fades itself, unless the picture has known motion.
        let hints = Hints {
            motion: hint,
            fading: fading.then_some(true),
        };
        let p = sw.encode_with(f, force_idr || fresh, hints);
        run.finish();
        self.sw_samples += 1;
        Sample {
            data: p.data,
            key: p.key,
        }
    }

    /// The software encoder's luma PSNR on `f` as an intra picture at the export's QP.
    fn software_psnr(&self, f: &Frame<'_>) -> Result<f64, String> {
        let (w, ht) = (self.cfg.width as usize, self.cfg.height as usize);
        let mut sw = Encoder::new(self.cfg.clone()).map_err(|e| e.to_string())?;
        let _ = sw.encode(f, true);
        Ok(psnr(f.y, f.y_stride, &sw.reconstruction()[0], w, w, ht))
    }

    /// The periodic check of a P picture (see the module docs): every `spot.every` pictures,
    /// the next GOP is collected up to [`SPOT_DEPTH`] and that picture is decoded by our
    /// decoder; its luma PSNR must be within the margin of the software encoder's intra
    /// picture at the export's QP, plus [`VERIFY_DB_PER_QP`] per step `qp` is above it.
    fn spot_check(
        &mut self,
        h: &dyn HwBackend,
        f: &Frame<'_>,
        data: &[u8],
        key: bool,
        qp: u8,
    ) -> Result<(), String> {
        let s = &mut self.spot;
        if s.every == 0 {
            return Ok(());
        }
        s.since += 1;
        if key {
            s.gop.clear();
            s.armed |= s.since >= s.every;
        }
        if !s.armed || (s.gop.is_empty() && !key) {
            return Ok(());
        }
        s.gop.push(data.to_vec());
        if s.gop.len() <= SPOT_DEPTH as usize {
            return Ok(());
        }
        let gop = std::mem::take(&mut s.gop);
        s.armed = false;
        s.since = 0;
        let (w, ht) = (self.cfg.width as usize, self.cfg.height as usize);
        let samples: Vec<&[u8]> = gop.iter().map(Vec::as_slice).collect();
        let hw_db = decode_psnr(&hw::avcc(&[h.sps()], &[h.pps()]), &samples, f, w, ht)?;
        let sw_db = self.software_psnr(f)?;
        let raised = f64::from(qp.saturating_sub(self.cfg.qp));
        let floor = sw_db - VERIFY_MARGIN_DB - VERIFY_DB_PER_QP * raised;
        if hw_db >= floor {
            self.spot_checks += 1;
            info!(
                "video export: hardware P picture spot-checked: {hw_db:.1} dB, software intra \
                 {sw_db:.1} dB"
            );
            Ok(())
        } else {
            Err(format!(
                "a P picture at {hw_db:.1} dB against the software's {sw_db:.1} dB (QP {qp})"
            ))
        }
    }

    /// The hardware's first picture against the software encoder's at the same QP: decoded
    /// by our decoder, luma PSNR within [`VERIFY_MARGIN_DB`]. Records the outcome.
    fn check(&mut self, h: &dyn HwBackend, f: &Frame<'_>, data: &[u8]) -> Result<(), String> {
        self.verify = false;
        let (w, ht) = (self.cfg.width as usize, self.cfg.height as usize);
        let hw_db = decode_psnr(&hw::avcc(&[h.sps()], &[h.pps()]), &[data], f, w, ht);
        let sw_db = self.software_psnr(f)?;
        let ok = hw_db.as_ref().is_ok_and(|&d| d >= sw_db - VERIFY_MARGIN_DB);
        selector().verified(Task::VideoEncode, Engine::HwEncoder, ok);
        match hw_db {
            Ok(d) if ok => {
                info!("video export: hardware encoder checked: {d:.1} dB, software {sw_db:.1} dB");
                Ok(())
            }
            Ok(d) => Err(format!("{d:.1} dB against the software's {sw_db:.1} dB")),
            Err(e) => Err(e),
        }
    }
}

fn request(cfg: &Config) -> Request {
    Request::new(u64::from(cfg.width) * u64::from(cfg.height), 0).verifying()
}

/// Luma PSNR of the last of `samples` (an IDR picture and the pictures after it) decoded
/// with `avcc` against the source picture.
fn decode_psnr(
    avcc: &[u8],
    samples: &[&[u8]],
    f: &Frame<'_>,
    w: usize,
    h: usize,
) -> Result<f64, String> {
    let opts = specular_h264::Options {
        threads: if samples.len() > 1 { 4 } else { 1 },
        output: specular_h264::OutputOrder::Decode,
    };
    let mut d = specular_h264::Decoder::with_avcc(avcc, opts).map_err(|e| e.to_string())?;
    let mut last = None;
    for (i, s) in samples.iter().enumerate() {
        d.decode_sample(s, i as i64).map_err(|e| e.to_string())?;
        while let Some(p) = d.next_picture(false) {
            last = Some(p);
        }
    }
    d.flush();
    while let Some(p) = d.next_picture(true) {
        last = Some(p);
    }
    let p = last
        .filter(|p| p.tag == samples.len() as i64 - 1)
        .ok_or("our decoder made no picture of it")?;
    if (p.width as usize, p.height as usize) != (w, h) {
        return Err(format!("decoded {}×{}", p.width, p.height));
    }
    Ok(psnr(f.y, f.y_stride, &p.planes[0], w, w, h))
}

fn psnr(a: &[u8], a_stride: usize, b: &[u8], b_stride: usize, w: usize, h: usize) -> f64 {
    let mut sum = 0u64;
    for r in 0..h {
        for (x, y) in a[r * a_stride..][..w].iter().zip(&b[r * b_stride..][..w]) {
            let d = u64::from(x.abs_diff(*y));
            sum += d * d;
        }
    }
    if sum == 0 {
        return 99.0;
    }
    let mse = sum as f64 / (w * h) as f64;
    10.0 * (255.0 * 255.0 / mse).log10()
}
