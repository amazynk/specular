//! MPEG-4 AAC decoder (ISO/IEC 14496-3) for Specular's video playback.
//!
//! Decodes AAC-LC, HE-AAC (spectral band replication) and HE-AAC v2 (parametric stereo) raw
//! data blocks, given an AudioSpecificConfig (from MP4 `esds`, Matroska `CodecPrivate`),
//! ADTS framing ([`adts`]) or LOAS/LATM ([`latm`]), to interleaved `f32` samples in ±1.0;
//! [`StreamDecoder`] takes a container's samples in any of these framings.
//!
//! - AAC-LC: every coding tool (M/S and intensity stereo, TNS, PNS, pulse data, coupling
//!   channels, all window sequences and both window shapes), any channel layout from
//!   `channelConfiguration` or a program config element.
//! - AAC Main (backward-adaptive prediction, reset groups, order-20 TNS) and AAC-LTP
//!   (long-term prediction from the decoded output through a forward MDCT, with TNS on the
//!   estimate).
//! - 960-sample frames (`frameLengthFlag`, as DAB+ sends) for AAC-LC, Main and LTP; with SBR
//!   they are refused by name.
//! - SBR, signalled explicitly (object type 5 or a sync extension) or implicitly (settled
//!   from the first frames by [`resolve_implicit`]); downsampled SBR when signalled or when
//!   the core runs above 24 kHz.
//! - Parametric stereo with the 20- and 34-band hybrid filterbanks and the phase parameters
//!   (IPD/OPD); a mono stream that may carry it decodes to two channels.
//! - A frame that fails to decode can be concealed ([`Decoder::conceal`]).
//!
//! Mono stays mono and stereo stays stereo. More channels come as 5.1 in the WAVE order
//! (L R C LFE Ls Rs), other layouts folded into it, or downmixed to stereo with ITU-R BS.775
//! coefficients when [`Options::stereo_downmix`] is set.
//!
//! The ISO/IEC 14496-3 data tables (codebooks, band tables, SBR and PS tables) are copied
//! from other implementations under the user's licence exception; `src/data/PROVENANCE.md`
//! records where each came from. All decoding logic is this crate's own.
#![deny(unsafe_code)]

pub mod adts;
mod bits;
pub mod config;
mod coupling;
mod data;
mod fft;
mod filterbank;
mod ics;
pub mod latm;
mod ps;
mod sbr;
mod stream;
mod tables;
mod vlc;

use bits::Bits;
pub use config::{Config, Signal, make_asc, parse_asc};
use config::{Element, Position};
use filterbank::{Filterbank, Overlap, Sequence};
use ics::{AOT_LTP, AOT_MAIN, Channel, Core, IcsInfo, Lcg, Predictor};
pub use stream::{Framing, StreamDecoder};

/// Decoder errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// A valid stream using a coding tool or object type this decoder does not implement.
    #[error("unsupported: {0}")]
    Unsupported(String),
    /// The data does not follow the bitstream syntax.
    #[error("malformed AAC data: {0}")]
    Malformed(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Decoder options.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Options {
    /// Downmix more than two channels to stereo (instead of 5.1).
    pub stereo_downmix: bool,
}

/// Core samples per channel in one raw data block (960 when `frameLengthFlag` is set).
pub const FRAME_LEN: usize = 1024;

/// How SBR runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SbrMode {
    Off,
    /// Output at twice the core rate.
    Upsampled,
    /// Output at the core rate.
    Downsampled,
}

/// An AAC decoder for one stream.
pub struct Decoder {
    cfg: Config,
    opts: Options,
    books: &'static tables::Books,
    /// The core coder: band tables, frame length, object type.
    core: Core,
    fb: Filterbank,
    overlap: Vec<Overlap>,
    /// AAC Main: the predictor of each spectral line of each core channel.
    pred: Vec<Vec<Predictor>>,
    /// AAC-LTP: each core channel's last two output frames and the overlap after them.
    ltp_hist: Vec<Vec<f32>>,
    /// AAC-LTP scratch: the time estimate and its spectrum.
    ltp_time: Vec<f32>,
    ltp_spec: Vec<f32>,
    /// Decoded samples of each core channel for the current frame (16-bit scale).
    pcm: Vec<Vec<f32>>,
    /// The spectrum of each core channel for the current frame, and of the last good frame
    /// (for concealment) with the frames lost since.
    chans: Vec<Channel>,
    last: Vec<Channel>,
    lost: u8,
    /// Filterbank state of independently switched coupling channels, by tag.
    cce_overlap: Vec<(u8, Overlap)>,
    noise: Lcg,
    sbr_mode: SbrMode,
    /// A mono stream decodes to two channels (parametric stereo may come).
    ps_stereo: bool,
    /// SBR of each layout element.
    sbr: Vec<sbr::Sbr>,
    /// Output channels after SBR (16-bit scale).
    out: Vec<Vec<f32>>,
    /// Position of each output channel.
    positions: Vec<Position>,
    /// First core channel of each layout element.
    first_channel: Vec<usize>,
    /// The layout element (or, for a stereo layout of mono elements, the channel) each
    /// element instance tag took, by element kind (SCE, CPE, LFE).
    tags: [[Option<usize>; 16]; 3],
}

fn malformed(s: &str) -> Error {
    Error::Malformed(s.into())
}

impl Decoder {
    /// Creates a decoder from an AudioSpecificConfig.
    pub fn new(asc: &[u8], opts: Options) -> Result<Self> {
        let cfg = parse_asc(asc)?;
        Self::with_config(cfg, opts)
    }

    /// Creates a decoder from a parsed config (for example [`adts::Header::config`]).
    pub fn with_config(cfg: Config, opts: Options) -> Result<Self> {
        if !tables::ready() || sbr::books().is_none() {
            return Err(Error::Unsupported("AAC tables are malformed".into()));
        }
        let books = tables::books().ok_or_else(|| Error::Unsupported("AAC tables".into()))?;
        if !matches!(cfg.object_type, 1 | 2 | 4) {
            return Err(Error::Unsupported(config::object_type_name(
                cfg.object_type,
            )));
        }
        let frame_len = if cfg.frame_len == 960 { 960 } else { 1024 };
        if frame_len == 960 && cfg.sbr == Signal::Present {
            return Err(Error::Unsupported(config::SBR_960.into()));
        }
        let core = Core {
            rate_index: usize::from(cfg.rate_index),
            frame_len,
            object_type: cfg.object_type,
        };
        let sbr_mode = match cfg.sbr {
            // SBR with 960-sample frames is not decoded: only the core plays
            _ if frame_len == 960 => SbrMode::Off,
            Signal::Absent => SbrMode::Off,
            Signal::Present => match cfg.sbr_rate {
                Some(r) if r <= cfg.sample_rate => SbrMode::Downsampled,
                _ => SbrMode::Upsampled,
            },
            Signal::Implicit if cfg.sample_rate <= 24000 => SbrMode::Upsampled,
            Signal::Implicit => SbrMode::Off,
        };
        let mut d = Decoder {
            cfg,
            opts,
            books,
            core,
            fb: Filterbank::new(frame_len),
            overlap: Vec::new(),
            pred: Vec::new(),
            ltp_hist: Vec::new(),
            ltp_time: vec![0.0; 2 * frame_len],
            ltp_spec: vec![0.0; frame_len],
            pcm: Vec::new(),
            chans: Vec::new(),
            last: Vec::new(),
            lost: 0,
            cce_overlap: Vec::new(),
            noise: Lcg(0x1f2e_3d4c),
            sbr_mode,
            ps_stereo: false,
            sbr: Vec::new(),
            out: Vec::new(),
            positions: Vec::new(),
            first_channel: Vec::new(),
            tags: [[None; 16]; 3],
        };
        d.rebuild();
        Ok(d)
    }

    /// Output sampling rate in Hz.
    pub fn sample_rate(&self) -> u32 {
        if self.sbr_mode == SbrMode::Upsampled {
            2 * self.cfg.sample_rate
        } else {
            self.cfg.sample_rate
        }
    }

    /// Output channels. Before the first frame of a stream whose layout only an in-band
    /// program config element gives, this is 0.
    pub fn channels(&self) -> u16 {
        match self.positions.len() {
            n if n <= 2 => n as u16,
            _ if self.opts.stereo_downmix => 2,
            _ => self.surround_channels() as u16,
        }
    }

    /// Channels of the surround output: 7.1 (8) when the layout has both side and back
    /// pairs, else 5.1 (6).
    fn surround_channels(&self) -> usize {
        let has = |a: Position, b: Position| self.positions.iter().any(|p| *p == a || *p == b);
        if has(Position::SideLeft, Position::SideRight)
            && has(Position::BackLeft, Position::BackRight)
        {
            8
        } else {
            6
        }
    }

    /// The configuration being decoded.
    pub fn config(&self) -> &Config {
        &self.cfg
    }

    /// Whether SBR runs (signalled or assumed).
    pub fn sbr_active(&self) -> bool {
        self.sbr_mode != SbrMode::Off
    }

    /// Whether SBR data, and parametric stereo data, came in the frames decoded so far.
    pub fn found(&self) -> (bool, bool) {
        (
            self.sbr.iter().any(|s| s.seen),
            self.sbr.iter().any(|s| s.ps_seen()),
        )
    }

    /// Samples per channel each frame decodes to.
    pub fn frame_len(&self) -> usize {
        if self.sbr_mode == SbrMode::Upsampled {
            2 * self.core.frame_len
        } else {
            self.core.frame_len
        }
    }

    /// Forgets all inter-frame state (for a seek). What the stream said about itself (SBR
    /// headers, a program config element) is kept, so decoding resumes at full quality.
    pub fn reset(&mut self) {
        let headers: Vec<_> = self.sbr.iter().map(sbr::Sbr::header).collect();
        self.rebuild();
        for (s, h) in self.sbr.iter_mut().zip(headers) {
            s.set_header(h);
        }
    }

    /// Sets up the state for the layout and modes.
    fn rebuild(&mut self) {
        let n = self.cfg.core_channels();
        let len = self.core.frame_len;
        self.overlap = vec![Overlap::new(len); n];
        self.pcm = vec![vec![0.0; len]; n];
        self.pred = match self.core.object_type {
            AOT_MAIN => vec![vec![Predictor::default(); len]; n],
            _ => Vec::new(),
        };
        self.ltp_hist = match self.core.object_type {
            AOT_LTP => vec![vec![0.0; 3 * len]; n],
            _ => Vec::new(),
        };
        self.chans = vec![Channel::default(); n];
        self.last = vec![Channel::default(); n];
        self.lost = 0;
        self.cce_overlap.clear();
        self.noise = Lcg(0x1f2e_3d4c);
        self.tags = [[None; 16]; 3];
        let els = &self.cfg.elements;
        self.first_channel = els
            .iter()
            .scan(0, |c, (e, _)| {
                let first = *c;
                *c += e.channels();
                Some(first)
            })
            .collect();
        self.ps_stereo = self.sbr_mode != SbrMode::Off
            && self.cfg.ps != Signal::Absent
            && els.len() == 1
            && els[0].0 == Element::Sce;
        self.positions.clear();
        for (e, pos) in els {
            if self.ps_stereo {
                self.positions
                    .extend([Position::FrontLeft, Position::FrontRight]);
            } else {
                self.positions.extend_from_slice(&pos[..e.channels()]);
            }
        }
        self.sbr = if self.sbr_mode == SbrMode::Off {
            Vec::new()
        } else {
            els.iter()
                .map(|(e, _)| {
                    sbr::Sbr::new(
                        self.cfg.sample_rate,
                        e.channels(),
                        self.sbr_mode == SbrMode::Downsampled,
                        self.ps_stereo,
                    )
                })
                .collect()
        };
        let len = self.frame_len();
        self.out = if self.sbr_mode == SbrMode::Off {
            Vec::new()
        } else {
            vec![vec![0.0; len]; self.positions.len()]
        };
    }

    /// Decodes one raw data block (an MP4 sample, or an ADTS frame without its header),
    /// appending interleaved samples to `out`. Returns the samples per channel appended.
    pub fn decode(&mut self, frame: &[u8], out: &mut Vec<f32>) -> Result<usize> {
        let mut b = Bits::new(frame);
        let mut used = vec![false; self.cfg.elements.len()];
        let mut decoded = vec![false; self.cfg.core_channels()];
        // the layout element the last SCE/CPE filled (SBR data follows it)
        let mut last: Option<usize> = None;
        let mut ccs: Vec<coupling::Cce> = Vec::new();
        loop {
            if b.left() < 3 {
                return Err(malformed("raw data block has no END element"));
            }
            match b.read(3) {
                id @ (0 | 3) => {
                    let tag = b.read(4) as usize; // element_instance_tag
                    let kind = if id == 3 { Element::Lfe } else { Element::Sce };
                    let (target, el) = self.place(kind, tag, &mut used, &decoded)?;
                    let core = self.core;
                    self.chans[target].read(&mut b, self.books, &core, None, &mut self.noise)?;
                    if let Some(p) = self.pred.get_mut(target) {
                        self.chans[target].predict(p, core.rate_index);
                    }
                    decoded[target] = true;
                    // SBR data of an SCE standing in for half a pair would not fit the pair
                    last = el;
                }
                1 => {
                    let tag = b.read(4) as usize;
                    let (target, el) = self.place(Element::Cpe, tag, &mut used, &decoded)?;
                    self.read_cpe(&mut b, target)?;
                    decoded[target] = true;
                    decoded[target + 1] = true;
                    last = el;
                }
                2 => {
                    let core = self.core;
                    ccs.push(coupling::Cce::read(
                        &mut b,
                        self.books,
                        &core,
                        &mut self.noise,
                    )?);
                }
                4 => {
                    b.read(4);
                    let align = b.bit();
                    let mut count = b.read(8) as usize;
                    if count == 255 {
                        count += b.read(8) as usize;
                    }
                    if align {
                        b.align();
                    }
                    b.skip(8 * count);
                }
                5 => {
                    let pce = config::parse_pce(&mut b, 0)?;
                    if self.cfg.channel_config == 0 && self.cfg.elements.is_empty() {
                        self.cfg.elements = pce.elements;
                        self.cfg.tags = pce.tags;
                        self.rebuild();
                        decoded = vec![false; self.cfg.core_channels()];
                        used = vec![false; self.cfg.elements.len()];
                    }
                }
                6 => {
                    let mut count = b.read(4) as usize;
                    if count == 15 {
                        count += b.read(8) as usize;
                        count -= 1;
                    }
                    if count > 0 {
                        let ext = b.read(4);
                        let bits = 8 * count - 4;
                        match (ext, last) {
                            (13 | 14, Some(i)) if i < self.sbr.len() => {
                                // SBR data this decoder can't use leaves the frame without it
                                let _ = self.sbr[i].read(&mut b, bits, ext == 14);
                            }
                            _ => b.skip(bits), // dynamic range control, fill
                        }
                    }
                }
                _ => break,
            }
            if b.overrun() {
                return Err(malformed("element runs past the end of the frame"));
            }
        }
        if self.cfg.elements.is_empty() {
            return Err(malformed(
                "no channel layout (missing program config element)",
            ));
        }
        // two mono elements for a stereo layout: the lower tag is the left channel
        if self.split_pair() {
            let tag_of = |c: usize| self.tags[0].iter().position(|&t| t == Some(c));
            if let (Some(t0), Some(t1)) = (tag_of(0), tag_of(1))
                && t0 > t1
            {
                self.tags[0].swap(t0, t1);
                self.chans.swap(0, 1);
                self.overlap.swap(0, 1);
                if self.pred.len() == 2 {
                    self.pred.swap(0, 1);
                }
                if self.ltp_hist.len() == 2 {
                    self.ltp_hist.swap(0, 1);
                }
            }
        }
        // one mono element for a stereo layout plays in both channels
        if self.split_pair() && decoded.iter().filter(|&&d| d).count() == 1 {
            let (a, b) = self.chans.split_at_mut(1);
            if decoded[0] {
                b[0].clone_from(&a[0]);
            } else {
                a[0].clone_from(&b[0]);
            }
            decoded = vec![true, true];
        }
        self.finish_core(&decoded, &mut ccs);
        self.last.clone_from(&self.chans);
        self.lost = 0;
        self.run_sbr();
        self.emit(out);
        Ok(self.frame_len())
    }

    /// Appends one frame of concealment in place of a frame that failed to decode: the last
    /// good frame's spectrum again, 6 dB quieter for each frame lost in a row, so the output
    /// keeps its timing and fades instead of clicking. Returns the samples per channel
    /// appended.
    pub fn conceal(&mut self, out: &mut Vec<f32>) -> usize {
        if self.positions.is_empty() {
            return 0;
        }
        self.lost = self.lost.saturating_add(1);
        let gain = 0.5f32.powi(i32::from(self.lost.min(30)));
        for c in 0..self.pcm.len() {
            let mut ch = self.last.get(c).cloned().unwrap_or_default();
            for v in &mut ch.spec {
                *v *= gain;
            }
            self.chans[c] = ch;
            self.synth(c);
        }
        self.run_sbr();
        self.emit(out);
        self.frame_len()
    }

    /// Runs SBR (and parametric stereo) on each element's core output.
    fn run_sbr(&mut self) {
        let (mut core, mut o) = (0, 0);
        for (i, (e, _)) in self.cfg.elements.iter().enumerate() {
            let Some(s) = self.sbr.get_mut(i) else {
                break;
            };
            let n_in = e.channels();
            let n_out = s.out_channels();
            let ins: Vec<&[f32]> = self.pcm[core..core + n_in].iter().map(|v| &v[..]).collect();
            let mut outs: Vec<&mut [f32]> = self.out[o..o + n_out]
                .iter_mut()
                .map(|v| &mut v[..])
                .collect();
            s.process(&ins, &mut outs);
            core += n_in;
            o += n_out;
        }
    }

    fn rate_index(&self) -> usize {
        usize::from(self.cfg.rate_index)
    }

    /// A stereo layout carried as two mono elements (or one, played in both channels).
    fn split_pair(&self) -> bool {
        let els = &self.cfg.elements;
        els.len() == 1 && els[0].0 == Element::Cpe
    }

    /// Finds where an element of `kind` with instance tag `tag` goes: its first core
    /// channel, and its layout element if it fills one. An element keeps the slot its tag
    /// first took (streams may send the elements of a layout in any order); new tags take
    /// the first free slot of their kind. Two mono elements may carry a stereo layout, the
    /// first tag seen on the left.
    fn place(
        &mut self,
        kind: Element,
        tag: usize,
        used: &mut [bool],
        decoded: &[bool],
    ) -> Result<(usize, Option<usize>)> {
        let k = match kind {
            Element::Sce => 0,
            Element::Cpe => 1,
            Element::Lfe => 2,
        };
        if kind != Element::Cpe && self.split_pair() {
            let c = match self.tags[k][tag] {
                Some(c) if !decoded[c] => c,
                _ => {
                    let c = (0..2)
                        .find(|&c| !decoded[c] && !self.tags[k].contains(&Some(c)))
                        .or_else(|| (0..2).find(|&c| !decoded[c]))
                        .ok_or_else(|| malformed("more channel elements than the layout has"))?;
                    self.tags[k][tag] = Some(c);
                    c
                }
            };
            return Ok((c, None));
        }
        let els = &self.cfg.elements;
        let fits = |e: Element, strict: bool| match kind {
            Element::Cpe => e == Element::Cpe,
            _ if strict => e == kind,
            _ => e != Element::Cpe,
        };
        // a program config element names each element's tag
        if self.cfg.tags.iter().any(Option::is_some) {
            let i = (0..els.len())
                .find(|&i| {
                    !used[i]
                        && self.cfg.tags.get(i) == Some(&Some(tag as u8))
                        && fits(els[i].0, false)
                })
                .ok_or_else(|| malformed("element tag not in the program config element"))?;
            used[i] = true;
            return Ok((self.first_channel[i], Some(i)));
        }
        let i = match self.tags[k][tag] {
            Some(i) if i < els.len() && !used[i] && fits(els[i].0, false) => i,
            _ => {
                let taken = |i: usize| self.tags[k].contains(&Some(i));
                let i = (0..els.len())
                    .find(|&i| !used[i] && !taken(i) && fits(els[i].0, true))
                    .or_else(|| (0..els.len()).find(|&i| !used[i] && fits(els[i].0, true)))
                    .or_else(|| (0..els.len()).find(|&i| !used[i] && fits(els[i].0, false)))
                    .ok_or_else(|| malformed("more channel elements than the layout has"))?;
                self.tags[k][tag] = Some(i);
                i
            }
        };
        used[i] = true;
        Ok((self.first_channel[i], Some(i)))
    }

    /// Runs the filterbank on core channel `c`'s spectrum.
    fn synth(&mut self, c: usize) {
        let (Some(ov), Some(pcm), Some(ch)) = (
            self.overlap.get_mut(c),
            self.pcm.get_mut(c),
            self.chans.get(c),
        ) else {
            return;
        };
        self.fb.synth(&ch.spec, ch.info.seq, ch.info.shape, ov, pcm);
        // long-term prediction looks back over the output and the overlap after it
        if let Some(h) = self.ltp_hist.get_mut(c) {
            let n = self.core.frame_len;
            h.copy_within(n..2 * n, 0);
            h[n..2 * n].copy_from_slice(&pcm[..n]);
            h[2 * n..3 * n].copy_from_slice(&ov.saved[..n]);
        }
    }

    /// Adds long-term prediction (§4.6.6) to core channel `c`'s spectrum, before TNS: the
    /// lagged and scaled past output, windowed as this frame and transformed, filtered by the
    /// channel's TNS, in the bands that use it.
    fn apply_ltp(&mut self, c: usize) {
        let (Some(ch), Some(h), Some(ov)) =
            (self.chans.get(c), self.ltp_hist.get(c), self.overlap.get(c))
        else {
            return;
        };
        let Some(ltp) = ch.info.ltp else {
            return;
        };
        if ch.info.seq == Sequence::EightShort || ltp.used == 0 {
            return;
        }
        let n = self.core.frame_len;
        // x_est[i] = coef · x_rec[i - lag], with x_rec[0] the start of this frame's output;
        // the newest part of the past is only known up to the end of the overlap
        let known = if ltp.lag < n { ltp.lag + n } else { 2 * n };
        for (i, v) in self.ltp_time.iter_mut().enumerate() {
            *v = match (i + 2 * n).checked_sub(ltp.lag) {
                Some(j) if i < known && j < 3 * n => ltp.coef * h[j],
                _ => 0.0,
            };
        }
        self.fb.analyse(
            &mut self.ltp_time,
            ch.info.seq,
            ch.info.shape,
            ov.prev_shape,
            &mut self.ltp_spec,
        );
        ch.tns_filter(&mut self.ltp_spec, self.core.rate_index, true);
        let offsets = ch.info.offsets;
        let bands = ch.info.max_sfb.min(tables::LTP_LONG_SFB);
        let ch = &mut self.chans[c];
        for s in 0..bands {
            if ltp.used & (1 << s) != 0 {
                let band = usize::from(offsets[s])..usize::from(offsets[s + 1]);
                for (x, p) in ch.spec[band.clone()].iter_mut().zip(&self.ltp_spec[band]) {
                    *x += p;
                }
            }
        }
    }

    fn read_cpe(&mut self, b: &mut Bits, target: usize) -> Result<()> {
        let core = self.core;
        let common_window = b.bit();
        let mut ms = 0;
        let mut ms_used = [false; 512];
        let mut right_ltp = None;
        let common = if common_window {
            let info = IcsInfo::read(b, &core)?;
            right_ltp = info.read_second_ltp(b, &core);
            ms = b.read(2);
            if ms == 3 {
                return Err(malformed("reserved ms_mask_present"));
            }
            if ms == 1 {
                for g in 0..info.group_len.len() {
                    for s in 0..info.max_sfb {
                        ms_used[g * 64 + s] = b.bit();
                    }
                }
            }
            Some(info)
        } else {
            None
        };
        let right = common.as_ref().map(|i| IcsInfo {
            ltp: right_ltp,
            ..i.clone()
        });
        let (a, rest) = self.chans.split_at_mut(target + 1);
        let (l, r) = (&mut a[target], &mut rest[0]);
        l.read(b, self.books, &core, common.as_ref(), &mut self.noise)?;
        r.read(b, self.books, &core, right.as_ref(), &mut self.noise)?;
        // M/S, then AAC Main prediction, then intensity stereo
        if common_window {
            ics::ms_stereo(l, r, ms, &ms_used);
        }
        if let [pl, pr] = self.pred.get_mut(target..target + 2).unwrap_or_default() {
            l.predict(pl, core.rate_index);
            r.predict(pr, core.rate_index);
        }
        if common_window {
            ics::intensity_stereo(l, r, ms, &ms_used);
        }
        Ok(())
    }

    /// Finishes the core channels of a frame: coupling, TNS and the filterbank, in element
    /// order; channels the frame did not carry are silent (their overlap still drains).
    fn finish_core(&mut self, decoded: &[bool], ccs: &mut [coupling::Cce]) {
        let ri = self.rate_index();
        // coupling channels get their own TNS; independently switched ones also go through
        // their own filterbank
        for cce in ccs.iter_mut() {
            cce.ch.apply_tns(ri);
        }
        for cce in ccs
            .iter_mut()
            .filter(|c| c.point == coupling::Point::AfterFilterbank)
        {
            let pos = match self.cce_overlap.iter().position(|o| o.0 == cce.tag) {
                Some(p) => p,
                None => {
                    let len = self.core.frame_len;
                    self.cce_overlap.push((cce.tag, Overlap::new(len)));
                    self.cce_overlap.len() - 1
                }
            };
            cce.time = vec![0.0; self.core.frame_len];
            let ov = &mut self.cce_overlap[pos].1;
            self.fb.synth(
                &cce.ch.spec,
                cce.ch.info.seq,
                cce.ch.info.shape,
                ov,
                &mut cce.time,
            );
        }
        let n_el = self.cfg.elements.len();
        for i in 0..n_el {
            let (kind, _) = self.cfg.elements[i];
            let first = self.first_channel[i];
            let k = match kind {
                Element::Sce => 0,
                Element::Cpe => 1,
                Element::Lfe => 2,
            };
            let tag = match self.cfg.tags.get(i) {
                Some(&Some(t)) => Some(usize::from(t)),
                _ => (!self.split_pair())
                    .then(|| self.tags[k].iter().position(|&t| t == Some(i)))
                    .flatten(),
            };
            for c in first..first + kind.channels() {
                if !decoded.get(c).copied().unwrap_or(false) {
                    self.chans[c] = Channel::default();
                    self.synth(c);
                    continue;
                }
                let lists: Vec<(usize, usize, usize)> = match tag {
                    Some(t) if kind != Element::Lfe => ccs
                        .iter()
                        .enumerate()
                        .flat_map(|(n, cce)| {
                            cce.lists_for(kind == Element::Cpe, t as u8)
                                .into_iter()
                                .filter(|&(ch, _)| ch == c - first)
                                .map(move |(_, l)| (n, l, 0))
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                for &(n, l, _) in &lists {
                    if ccs[n].point == coupling::Point::BeforeTns {
                        ccs[n].add_spectrum(&mut self.chans[c], l);
                    }
                }
                self.apply_ltp(c);
                self.chans[c].apply_tns(ri);
                for &(n, l, _) in &lists {
                    if ccs[n].point == coupling::Point::AfterTns {
                        ccs[n].add_spectrum(&mut self.chans[c], l);
                    }
                }
                self.synth(c);
                for &(n, l, _) in &lists {
                    if ccs[n].point == coupling::Point::AfterFilterbank {
                        ccs[n].add_time(&mut self.pcm[c], l);
                    }
                }
            }
        }
    }

    /// Interleaves the frame's output channels into `out`, scaled to ±1.0: as they are for
    /// one or two channels, as 5.1 or 7.1 in the WAVE order or downmixed to stereo for more.
    fn emit(&self, out: &mut Vec<f32>) {
        const S: f32 = 1.0 / 32768.0;
        const H: f32 = std::f32::consts::FRAC_1_SQRT_2;
        let chans: &[Vec<f32>] = if self.sbr_mode == SbrMode::Off {
            &self.pcm
        } else {
            &self.out
        };
        let len = self.frame_len();
        let n = chans.len().min(self.positions.len());
        if n <= 2 {
            out.reserve(len * n);
            for i in 0..len {
                for ch in &chans[..n] {
                    out.push(ch[i] * S);
                }
            }
            return;
        }
        if self.opts.stereo_downmix {
            // ITU-R BS.775 downmix: centre and surrounds at -3 dB, LFE dropped, then
            // normalised so a full-scale signal in every channel cannot clip.
            let gains: Vec<(f32, f32)> = self.positions[..n]
                .iter()
                .map(|p| match p {
                    Position::FrontLeft => (1.0, 0.0),
                    Position::FrontRight => (0.0, 1.0),
                    Position::FrontCentre => (H, H),
                    Position::SideLeft | Position::BackLeft => (H, 0.0),
                    Position::SideRight | Position::BackRight => (0.0, H),
                    Position::BackCentre => (0.5, 0.5),
                    Position::LowFrequency => (0.0, 0.0),
                })
                .collect();
            let sum_l: f32 = gains.iter().map(|g| g.0).sum();
            let sum_r: f32 = gains.iter().map(|g| g.1).sum();
            let norm = S / sum_l.max(sum_r).max(1.0);
            out.reserve(len * 2);
            for i in 0..len {
                let (mut l, mut r) = (0.0, 0.0);
                for (ch, g) in chans.iter().zip(&gains) {
                    l += ch[i] * g.0;
                    r += ch[i] * g.1;
                }
                out.push(l * norm);
                out.push(r * norm);
            }
            return;
        }
        // 5.1 or 7.1: each position to its WAVE slot (7.1: L R C LFE Lb Rb Ls Rs); a slot
        // fed by more than one channel (a second front pair; sides and backs together in
        // 5.1) takes each at equal power
        let wide = self.surround_channels();
        let slot = |p: &Position| -> &'static [(usize, f32)] {
            match (p, wide) {
                (Position::FrontLeft, _) => &[(0, 1.0)],
                (Position::FrontRight, _) => &[(1, 1.0)],
                (Position::FrontCentre, _) => &[(2, 1.0)],
                (Position::LowFrequency, _) => &[(3, 1.0)],
                (Position::SideLeft, 8) => &[(6, 1.0)],
                (Position::SideRight, 8) => &[(7, 1.0)],
                (Position::SideLeft | Position::BackLeft, _) => &[(4, 1.0)],
                (Position::SideRight | Position::BackRight, _) => &[(5, 1.0)],
                (Position::BackCentre, _) => &[(4, H), (5, H)],
            }
        };
        let mut count = [0usize; 8];
        for p in &self.positions[..n] {
            for &(s, _) in slot(p) {
                count[s] += 1;
            }
        }
        let base = out.len();
        out.resize(base + len * wide, 0.0);
        for (ch, p) in chans.iter().zip(&self.positions[..n]) {
            for &(s, g) in slot(p) {
                let g = g * S / (count[s] as f32).sqrt();
                for (i, v) in ch[..len].iter().enumerate() {
                    out[base + wide * i + s] += v * g;
                }
            }
        }
    }
}

/// Settles implicit SBR and PS signalling (ADTS, LATM, or an AudioSpecificConfig without a
/// sync extension) by decoding the first raw data blocks (up to 64): the config comes back
/// with SBR and PS marked present or absent, so plain AAC-LC decodes at its own rate and
/// HE-AAC at twice it (or at the core rate above 24 kHz, as downsampled SBR).
pub fn resolve_implicit<'a>(cfg: &Config, frames: impl IntoIterator<Item = &'a [u8]>) -> Config {
    let mut out = cfg.clone();
    if cfg.sbr != Signal::Implicit && cfg.ps != Signal::Implicit {
        return out;
    }
    let mut probe = cfg.clone();
    if probe.sbr == Signal::Implicit {
        probe.sbr = Signal::Present;
        probe.sbr_rate = Some(if cfg.sample_rate <= 24000 {
            2 * cfg.sample_rate
        } else {
            cfg.sample_rate
        });
    }
    if probe.sbr == Signal::Absent {
        return out;
    }
    let Ok(mut d) = Decoder::with_config(probe.clone(), Options::default()) else {
        return out;
    };
    // SBR headers may come only every few frames; once one has, a few more frames tell
    // whether parametric stereo is in the SBR data
    let mut scratch = Vec::new();
    let mut after_header = 0;
    for f in frames.into_iter().take(64) {
        scratch.clear();
        let _ = d.decode(f, &mut scratch);
        if d.sbr.iter().any(sbr::Sbr::header_seen) {
            after_header += 1;
            if after_header >= 4 {
                break;
            }
        }
    }
    let (sbr, ps) = d.found();
    if out.sbr == Signal::Implicit {
        out.sbr = if sbr { Signal::Present } else { Signal::Absent };
        out.sbr_rate = if sbr { probe.sbr_rate } else { None };
    }
    // without an SBR header the PS question stays open (a mono stream then plays in both
    // channels, ready for PS)
    if out.ps == Signal::Implicit && (ps || after_header > 0) {
        out.ps = if ps { Signal::Present } else { Signal::Absent };
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_lc_and_he_decoders() {
        let d = Decoder::new(&[0x12, 0x10], Options::default()).unwrap();
        assert_eq!(
            (d.sample_rate(), d.channels(), d.frame_len()),
            (44100, 2, 1024)
        );
        // aot 5, 24 kHz core, stereo, 48 kHz SBR, core aot 2
        let d = Decoder::new(&[0x2b, 0x11, 0x88, 0x00], Options::default()).unwrap();
        assert_eq!(
            (d.sample_rate(), d.channels(), d.frame_len()),
            (48000, 2, 2048)
        );
        // implicit: LC at 22.05 kHz mono may carry SBR and PS
        let d = Decoder::new(&[0x13, 0x88], Options::default()).unwrap();
        assert_eq!((d.sample_rate(), d.channels()), (44100, 2));
        // 5.1 comes as six channels, or two when downmixed
        let d = Decoder::new(&[0x11, 0xb0], Options::default()).unwrap();
        assert_eq!(d.channels(), 6);
        let o = Options {
            stereo_downmix: true,
        };
        assert_eq!(Decoder::new(&[0x11, 0xb0], o).unwrap().channels(), 2);
    }

    /// An empty stereo frame (two silent channel streams, END) decodes to silence, and
    /// concealment keeps the frame length.
    #[test]
    fn silent_frame_and_concealment() {
        let mut d = Decoder::new(&[0x11, 0x90], Options::default()).unwrap();
        // CPE, tag 0, common_window 0; each ICS: global_gain 100, ics_info (long, max_sfb 0),
        // no pulse/TNS/gain control; END
        let mut bits = String::from("00100000");
        for _ in 0..2 {
            bits += "01100100"; // global_gain
            bits += "0000"; // reserved, window sequence 0, shape 0
            bits += "000000"; // max_sfb
            bits += "0"; // predictor
            bits += "000"; // pulse, tns, gain control
        }
        bits += "111";
        while bits.len() % 8 != 0 {
            bits.push('0');
        }
        let bytes: Vec<u8> = bits
            .as_bytes()
            .chunks(8)
            .map(|c| c.iter().fold(0u8, |a, &d| (a << 1) | (d - b'0')))
            .collect();
        let mut out = Vec::new();
        assert_eq!(d.decode(&bytes, &mut out), Ok(1024));
        assert_eq!(out.len(), 2048);
        assert!(out.iter().all(|&v| v == 0.0));
        assert_eq!(d.conceal(&mut out), 1024);
        assert_eq!(out.len(), 4096);
    }

    /// Every truncation and single-bit flip of valid configs and ADTS headers must return,
    /// not panic.
    #[test]
    fn corruption_sweep() {
        let seeds: [&[u8]; 4] = [
            &[0x12, 0x10],
            &[0x2b, 0x11, 0x88, 0x00],
            &[0x13, 0x10, 0x56, 0xe5, 0x9d, 0x48, 0x80],
            &[0xff, 0xf1, 0x50, 0x80, 0x2e, 0x7f, 0xfc, 0x21, 0x10, 0x05],
        ];
        for seed in seeds {
            for len in 0..=seed.len() {
                let _ = parse_asc(&seed[..len]);
                let _ = adts::parse(&seed[..len]);
                let _ = adts::frames(&seed[..len]).count();
            }
            for bit in 0..seed.len() * 8 {
                let mut v = seed.to_vec();
                v[bit / 8] ^= 0x80 >> (bit % 8);
                let _ = parse_asc(&v);
                if let Ok(mut d) = Decoder::new(
                    &v,
                    Options {
                        stereo_downmix: true,
                    },
                ) {
                    let mut out = Vec::new();
                    for frame in [&v[..], &[0xff; 64], &[0u8; 3]] {
                        let _ = d.decode(frame, &mut out);
                    }
                    d.conceal(&mut out);
                }
                if let Ok(h) = adts::parse(&v) {
                    let _ = h.config();
                }
            }
        }
    }
}
