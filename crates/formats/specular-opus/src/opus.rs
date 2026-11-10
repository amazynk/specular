//! One Opus stream: mode switching, SILK and CELT mixing, redundancy (RFC 6716 section 4.5).

use crate::Error;
use crate::celt::CeltDecoder;
use crate::packet::{self, Bandwidth, Mode};
use crate::range::RangeDecoder;
use crate::silk::SilkDecoder;

const F20: usize = 960;
const F10: usize = 480;
const F5: usize = 240;
const F2_5: usize = 120;

/// Decoder of one (mono or stereo) Opus stream at 48 kHz.
pub(crate) struct StreamDecoder {
    channels: usize,
    celt: CeltDecoder,
    silk: SilkDecoder,
    mode: Option<Mode>,
    prev_mode: Option<Mode>,
    bandwidth: Bandwidth,
    frame_size: usize,
    stream_channels: usize,
    prev_redundancy: bool,
    pub(crate) range_final: u32,
    silk_buf: Vec<f32>,
    /// Decoding a packet's LBRR data for the lost packet before it.
    fec: bool,
}

impl StreamDecoder {
    pub(crate) fn new(channels: usize) -> StreamDecoder {
        StreamDecoder {
            channels,
            celt: CeltDecoder::new(channels),
            silk: SilkDecoder::new(channels),
            mode: None,
            prev_mode: None,
            bandwidth: Bandwidth::Full,
            frame_size: F20,
            stream_channels: channels,
            prev_redundancy: false,
            range_final: 0,
            silk_buf: Vec::new(),
            fec: false,
        }
    }

    pub(crate) fn reset(&mut self) {
        let ch = self.channels;
        let inv = self.celt.disable_inv;
        *self = StreamDecoder::new(ch);
        self.celt.disable_inv = inv;
    }

    #[cfg(test)]
    pub(crate) fn silk_mut(&mut self) -> &mut SilkDecoder {
        &mut self.silk
    }

    pub(crate) fn set_phase_inversion(&mut self, enabled: bool) {
        self.celt.disable_inv = !enabled;
    }

    /// Decodes a whole packet (or conceals `frame_size` lost samples when `data` is None)
    /// into `out` (cleared first); returns samples per channel.
    pub(crate) fn decode_packet(
        &mut self,
        data: Option<(&[u8], &packet::Packet)>,
        lost_samples: usize,
        out: &mut Vec<f32>,
    ) -> Result<usize, Error> {
        let ch = self.channels;
        out.clear();
        let Some((data, p)) = data else {
            // Conceal in steps the concealment can produce.
            let mut left = lost_samples;
            while left > 0 {
                let n = left.min(F20);
                let start = out.len();
                out.resize(start + n * ch, 0.0);
                let got = self.decode_frame(None, n, &mut out[start..])?;
                out.truncate(start + got * ch);
                left -= got.min(left);
                if got == 0 {
                    break;
                }
            }
            return Ok(out.len() / ch);
        };
        self.mode = Some(p.toc.mode);
        self.bandwidth = p.toc.bandwidth;
        self.frame_size = p.toc.frame_size;
        self.stream_channels = if p.toc.stereo { 2 } else { 1 };
        let total = p.frames.len() * p.toc.frame_size;
        out.resize(total * ch, 0.0);
        let mut pos = 0;
        for &(off, len) in &p.frames {
            let frame = &data[off..off + len];
            let got = self.decode_frame(Some(frame), p.toc.frame_size, &mut out[pos * ch..])?;
            pos += got;
        }
        out.truncate(pos * ch);
        Ok(pos)
    }

    /// Rebuilds the lost packet before `data` from the in-band FEC (SILK LBRR) data of its
    /// first frame, `lost_samples` long; conceals instead when that frame is CELT, of another
    /// length, or carries no LBRR data for a part. Fills `out` like [`Self::decode_packet`].
    pub(crate) fn decode_fec(
        &mut self,
        data: &[u8],
        p: &packet::Packet,
        lost_samples: usize,
        out: &mut Vec<f32>,
    ) -> Result<usize, Error> {
        let Some(&(off, len)) = p.frames.first() else {
            return self.decode_packet(None, lost_samples, out);
        };
        if p.toc.mode == Mode::Celt
            || p.toc.frame_size != lost_samples
            || len <= 1
            || self.prev_mode.is_none()
        {
            return self.decode_packet(None, lost_samples, out);
        }
        let ch = self.channels;
        out.clear();
        out.resize(lost_samples * ch, 0.0);
        // Decode as a SILK frame of this packet's layout, LBRR data only; a hybrid frame's
        // CELT part is concealed.
        let saved = (
            self.mode,
            self.bandwidth,
            self.frame_size,
            self.stream_channels,
        );
        self.mode = Some(Mode::Silk);
        self.bandwidth = if p.toc.mode == Mode::Hybrid {
            Bandwidth::Wide
        } else {
            p.toc.bandwidth
        };
        self.frame_size = p.toc.frame_size;
        self.stream_channels = if p.toc.stereo { 2 } else { 1 };
        self.fec = true;
        let prev_mode = self.prev_mode;
        if prev_mode == Some(Mode::Hybrid) {
            self.prev_mode = Some(Mode::Silk);
        }
        let r = self.decode_frame(Some(&data[off..off + len]), lost_samples, out);
        self.fec = false;
        // The CELT layer of a hybrid stream goes on with its concealment.
        if prev_mode == Some(Mode::Hybrid) {
            let mut celt = vec![0f32; lost_samples * ch];
            self.celt.decode(None, lost_samples.min(F20), &mut celt);
            for (o, c) in out.iter_mut().zip(celt) {
                *o += c;
            }
            self.prev_mode = prev_mode;
        }
        (
            self.mode,
            self.bandwidth,
            self.frame_size,
            self.stream_channels,
        ) = saved;
        r
    }

    /// opus_decode_frame: one frame into `pcm` (at least `frame_size * channels` long).
    fn decode_frame(
        &mut self,
        data: Option<&[u8]>,
        frame_size: usize,
        pcm: &mut [f32],
    ) -> Result<usize, Error> {
        let ch = self.channels;
        let mut data = data;
        if data.is_some_and(|d| d.len() <= 1) {
            data = None;
        }
        let mut frame_size = frame_size.min(5760);
        let (mut audiosize, mode, bandwidth) = match data {
            Some(_) => (
                self.frame_size,
                self.mode.unwrap_or(Mode::Celt),
                Some(self.bandwidth),
            ),
            None => {
                frame_size = frame_size.min(self.frame_size);
                let mut a = frame_size;
                let Some(m) = self.prev_mode else {
                    pcm[..a * ch].fill(0.0);
                    return Ok(a);
                };
                if a > F20 {
                    let mut done = 0;
                    while done < a {
                        let got =
                            self.decode_frame(None, (a - done).min(F20), &mut pcm[done * ch..])?;
                        if got == 0 {
                            break;
                        }
                        done += got;
                    }
                    return Ok(done);
                } else if a < F20 {
                    if a > F10 {
                        a = F10;
                    } else if m != Mode::Silk && a > F5 && a < F10 {
                        a = F5;
                    }
                }
                (a, m, None)
            }
        };
        let mut ec_store = data.map(RangeDecoder::new);
        let mut len = data.map_or(0, |d| d.len());

        let mut transition = false;
        let mut pcm_transition: Vec<f32> = Vec::new();
        if data.is_some()
            && self.prev_mode.is_some()
            && ((mode == Mode::Celt && self.prev_mode != Some(Mode::Celt) && !self.prev_redundancy)
                || (mode != Mode::Celt && self.prev_mode == Some(Mode::Celt)))
        {
            transition = true;
            if mode == Mode::Celt {
                pcm_transition = vec![0.0; F5 * ch];
                let n = F5.min(audiosize);
                self.decode_frame(None, n, &mut pcm_transition)?;
            }
        }
        if audiosize > frame_size {
            return Err(Error::Malformed(
                "Opus frame larger than the packet allows".into(),
            ));
        }
        frame_size = audiosize;

        // SILK.
        let mut silk_out = std::mem::take(&mut self.silk_buf);
        silk_out.clear();
        if mode != Mode::Celt {
            if self.prev_mode == Some(Mode::Celt) {
                self.silk.reset();
            }
            if let Some(ec) = ec_store.as_mut() {
                let fs_khz = if mode == Mode::Silk {
                    match bandwidth {
                        Some(Bandwidth::Narrow) => 8,
                        Some(Bandwidth::Medium) => 12,
                        _ => 16,
                    }
                } else {
                    16
                };
                let ms = (frame_size / 48).max(10);
                self.silk.decode(
                    ec,
                    fs_khz,
                    self.stream_channels,
                    ms,
                    self.fec,
                    &mut silk_out,
                );
            } else {
                self.silk.conceal((frame_size / 48).max(10), &mut silk_out);
            }
            silk_out.resize(frame_size * ch, 0.0);
        }

        let mut redundancy = false;
        let mut celt_to_silk = false;
        let mut redundancy_bytes = 0usize;
        if let Some(ec) = ec_store.as_mut()
            && !self.fec
            && mode != Mode::Celt
            && ec.tell() + 17 + 20 * i32::from(mode == Mode::Hybrid) <= 8 * len as i32
        {
            redundancy = if mode == Mode::Hybrid {
                ec.bit_logp(12)
            } else {
                true
            };
            if redundancy {
                celt_to_silk = ec.bit_logp(1);
                redundancy_bytes = if mode == Mode::Hybrid {
                    ec.uint(256) as usize + 2
                } else {
                    len.saturating_sub(((ec.tell() + 7) >> 3) as usize)
                };
                if redundancy_bytes > len || ((len - redundancy_bytes) * 8) < ec.tell() as usize {
                    len = 0;
                    redundancy_bytes = 0;
                    redundancy = false;
                } else {
                    len -= redundancy_bytes;
                }
                ec.shrink(redundancy_bytes);
            }
        }
        let start_band = if mode != Mode::Celt { 17 } else { 0 };
        if redundancy {
            transition = false;
        }
        if transition && mode != Mode::Celt {
            pcm_transition = vec![0.0; F5 * ch];
            let n = F5.min(audiosize);
            self.decode_frame(None, n, &mut pcm_transition)?;
        }
        if let Some(bw) = bandwidth {
            self.celt.end = match bw {
                Bandwidth::Narrow => 13,
                Bandwidth::Medium | Bandwidth::Wide => 17,
                Bandwidth::SuperWide => 19,
                Bandwidth::Full => 21,
            };
        }
        self.celt.stream_channels = self.stream_channels;

        let full = data.unwrap_or(&[]);
        let mut redundant_audio = vec![0f32; if redundancy { F5 * ch } else { 0 }];
        let mut redundant_rng = 0u32;
        if redundancy && celt_to_silk {
            self.celt.start = 0;
            let rd = &full[len..len + redundancy_bytes];
            let mut rec = RangeDecoder::new(rd);
            self.celt.decode(Some(&mut rec), F5, &mut redundant_audio);
            redundant_rng = self.celt.rng();
        }
        self.celt.start = start_band;

        let pcm = &mut pcm[..frame_size * ch];
        if mode != Mode::Silk {
            let celt_frame = F20.min(frame_size);
            if Some(mode) != self.prev_mode && self.prev_mode.is_some() && !self.prev_redundancy {
                self.celt.reset();
            }
            self.celt.decode(ec_store.as_mut(), celt_frame, pcm);
        } else {
            pcm.fill(0.0);
            if self.prev_mode == Some(Mode::Hybrid)
                && !(redundancy && celt_to_silk && self.prev_redundancy)
            {
                self.celt.start = 0;
                let silence = [0xffu8, 0xff];
                let mut sd = RangeDecoder::new(&silence);
                let mut tmp = vec![0f32; F2_5 * ch];
                self.celt.decode(Some(&mut sd), F2_5, &mut tmp);
                pcm[..F2_5 * ch].copy_from_slice(&tmp);
            }
        }
        if mode != Mode::Celt {
            for (p, s) in pcm.iter_mut().zip(silk_out.iter()) {
                *p += *s;
            }
        }
        self.silk_buf = silk_out;

        let window: Vec<f32> = self.celt_window();
        if redundancy && !celt_to_silk {
            self.celt.reset();
            self.celt.start = 0;
            let rd = &full[len..len + redundancy_bytes];
            let mut rec = RangeDecoder::new(rd);
            self.celt.decode(Some(&mut rec), F5, &mut redundant_audio);
            redundant_rng = self.celt.rng();
            let o = ch * (frame_size - F2_5);
            let a: Vec<f32> = pcm[o..o + F2_5 * ch].to_vec();
            smooth_fade(
                &a,
                &redundant_audio[ch * F2_5..],
                &mut pcm[o..],
                ch,
                &window,
            );
        }
        if redundancy && celt_to_silk {
            pcm[..F2_5 * ch].copy_from_slice(&redundant_audio[..F2_5 * ch]);
            let b: Vec<f32> = pcm[ch * F2_5..ch * F5].to_vec();
            smooth_fade(
                &redundant_audio[ch * F2_5..],
                &b,
                &mut pcm[ch * F2_5..],
                ch,
                &window,
            );
        }
        if transition {
            if audiosize >= F5 {
                pcm[..ch * F2_5].copy_from_slice(&pcm_transition[..ch * F2_5]);
                let b: Vec<f32> = pcm[ch * F2_5..ch * F5].to_vec();
                smooth_fade(
                    &pcm_transition[ch * F2_5..],
                    &b,
                    &mut pcm[ch * F2_5..],
                    ch,
                    &window,
                );
            } else {
                let b: Vec<f32> = pcm[..ch * F2_5].to_vec();
                smooth_fade(&pcm_transition, &b, pcm, ch, &window);
            }
        }
        self.range_final = match ec_store.as_ref() {
            Some(ec) if len > 1 => ec.rng() ^ redundant_rng,
            _ => 0,
        };
        self.prev_mode = Some(mode);
        self.prev_redundancy = redundancy && !celt_to_silk;
        audiosize = frame_size;
        Ok(audiosize)
    }

    fn celt_window(&self) -> Vec<f32> {
        crate::mdct::window()
    }
}

/// Cross-fade from `a` to `b` over 2.5 ms with the squared CELT window.
fn smooth_fade(a: &[f32], b: &[f32], out: &mut [f32], ch: usize, window: &[f32]) {
    for c in 0..ch {
        for i in 0..F2_5 {
            let w = window[i] * window[i];
            out[i * ch + c] = w * b[i * ch + c] + (1.0 - w) * a[i * ch + c];
        }
    }
}
