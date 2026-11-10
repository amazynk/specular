//! NAL unit handling: parameter sets, picture boundaries, slices, and the per-picture flow
//! (reconstruction, inverse luma mapping, loop filters, cropping).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::bits::{BitReader, unescape_with_positions};
use crate::ctu::{self, PicState, SliceCtx};
use crate::error::{Error, Result};
use crate::filter;
use crate::frame::{ChromaFormat, Frame};
use crate::lmcs::Lmcs;
use crate::nal::{self, Header};
use crate::picture::Picture;
use crate::ps::alf::AlfAps;
use crate::ps::aps::{Aps, LmcsAps, ScalingAps, parse_aps};
use crate::ps::ph::{
    PictureHeader, SliceHeader, SliceType, parse_picture_header, parse_slice_header,
};
use crate::ps::pps::{Layout, Pps, parse_pps};
use crate::ps::sps::{Sps, parse_sps};
use crate::{Decoded, Info};

pub fn info(parameter_sets: &[&[u8]]) -> Result<Info> {
    let mut sps = None;
    let mut pps = None;
    for u in parameter_sets {
        let h = Header::parse(u)?;
        if h.layer_id != 0 {
            continue;
        }
        match h.nal_type {
            nal::SPS if sps.is_none() => {
                sps = Some(parse_sps(&unescape_with_positions(&u[2..]).0)?)
            }
            nal::PPS if pps.is_none() => {
                pps = Some(parse_pps(&unescape_with_positions(&u[2..]).0)?)
            }
            _ => {}
        }
    }
    let sps = sps.ok_or(Error::MissingParameterSet("SPS"))?;
    let pps = pps.ok_or(Error::MissingParameterSet("PPS"))?;
    check_supported(&sps)?;
    let (w, h, _) = cropped_size(&sps, &pps)?;
    Ok(Info {
        width: w,
        height: h,
        bit_depth: sps.bit_depth as u8,
        chroma: ChromaFormat::from_idc(sps.chroma_format_idc).unwrap_or_default(),
        profile_idc: sps.general_profile_idc,
    })
}

/// Conformance window: (width, height, (left, top) offset in luma samples).
fn cropped_size(sps: &Sps, pps: &Pps) -> Result<(u32, u32, (u32, u32))> {
    let win = match pps.conf_win {
        Some(w) => w,
        None if pps.width == sps.pic_width_max && pps.height == sps.pic_height_max => sps.conf_win,
        None => [0; 4],
    };
    let (sw, sh) = (sps.sub_width_c(), sps.sub_height_c());
    let l = win[0] * sw;
    let r = win[1] * sw;
    let t = win[2] * sh;
    let b = win[3] * sh;
    if l + r >= pps.width || t + b >= pps.height {
        return Err(Error::InvalidData(
            "conformance window crops the whole picture",
        ));
    }
    Ok((pps.width - l - r, pps.height - t - b, (l, t)))
}

/// Refuses what this decoder does not implement, in plain words.
fn check_supported(sps: &Sps) -> Result<()> {
    if sps.bit_depth > 10 {
        return Err(Error::Unsupported(
            "more than 10 bits per sample (the 12- and 16-bit VVC profiles)",
        ));
    }
    if sps.extended_precision
        || sps.ts_residual_coding_rice_present_in_sh
        || sps.rrc_rice_extension
        || sps.persistent_rice_adaptation
        || sps.reverse_last_sig_coeff
    {
        return Err(Error::Unsupported(
            "range-extension coding tools (used by the 12- and 16-bit VVC profiles)",
        ));
    }
    Ok(())
}

/// A picture being decoded.
struct Current {
    ph: PictureHeader,
    sps: Arc<Sps>,
    pps: Arc<Pps>,
    layout: Arc<Layout>,
    state: PicState,
    slices: Vec<filter::SliceFilter>,
    poc: i32,
    cvs: u32,
    inter: bool,
    alf_aps: [Option<Arc<AlfAps>>; 8],
    lmcs: Option<Arc<Lmcs>>,
    /// Slices that used LMCS, per slice index (inverse mapping is per slice).
    lmcs_used: Vec<bool>,
    damaged: usize,
    /// Memory held for the picture and its per-sample decoding state until it is output.
    _mem: specular_core::budget::Reservation,
}

pub struct Decoder {
    threads: usize,
    sps: Vec<Option<Arc<Sps>>>,
    pps: Vec<Option<Arc<Pps>>>,
    layouts: Vec<Option<Arc<Layout>>>,
    alf: [Option<Arc<AlfAps>>; 8],
    lmcs: [Option<Arc<LmcsAps>>; 4],
    scaling: [Option<Arc<ScalingAps>>; 8],
    ph: Option<PictureHeader>,
    cur: Option<Current>,
    prev_tid0_poc: Option<(u32, i32)>,
    skipped: usize,
    emitted: usize,
    stop: bool,
    cvs: u32,
}

impl Decoder {
    pub fn new(threads: usize) -> Decoder {
        Decoder {
            threads: threads.max(1),
            sps: vec![None; 16],
            pps: vec![None; 64],
            layouts: vec![None; 64],
            alf: Default::default(),
            lmcs: Default::default(),
            scaling: Default::default(),
            ph: None,
            cur: None,
            prev_tid0_poc: None,
            skipped: 0,
            emitted: 0,
            stop: false,
            cvs: 0,
        }
    }

    /// Takes one parameter set (or any other non-VCL) NAL unit.
    pub fn push_parameter_set(&mut self, unit: &[u8]) -> Result<()> {
        let h = Header::parse(unit)?;
        if h.layer_id != 0 {
            return Ok(());
        }
        self.non_vcl(h, unit)
    }

    fn non_vcl(&mut self, h: Header, unit: &[u8]) -> Result<()> {
        let rbsp = || unescape_with_positions(&unit[2..]).0;
        match h.nal_type {
            nal::SPS => {
                let s = parse_sps(&rbsp())?;
                let id = usize::from(s.id);
                self.sps[id] = Some(Arc::new(s));
                // Layouts of PPSs referring to it are rebuilt on use.
                for (i, p) in self.pps.iter().enumerate() {
                    if p.as_ref().is_some_and(|p| usize::from(p.sps_id) == id) {
                        self.layouts[i] = None;
                    }
                }
            }
            nal::PPS => {
                let p = parse_pps(&rbsp())?;
                let id = usize::from(p.id);
                self.pps[id] = Some(Arc::new(p));
                self.layouts[id] = None;
            }
            nal::PREFIX_APS | nal::SUFFIX_APS => match parse_aps(&rbsp())? {
                (_, id, Aps::Alf(a)) => self.alf[usize::from(id) & 7] = Some(Arc::from(a)),
                (_, id, Aps::Lmcs(l)) => self.lmcs[usize::from(id) & 3] = Some(Arc::new(l)),
                (_, id, Aps::Scaling(s)) => self.scaling[usize::from(id) & 7] = Some(Arc::from(s)),
            },
            _ => {}
        }
        Ok(())
    }

    fn layout(&mut self, pps_id: u8) -> Result<()> {
        let i = usize::from(pps_id);
        if self.layouts[i].is_some() {
            return Ok(());
        }
        let pps = self.pps[i]
            .as_ref()
            .ok_or(Error::MissingParameterSet("PPS"))?;
        let sps = self.sps[usize::from(pps.sps_id)]
            .as_ref()
            .ok_or(Error::MissingParameterSet("SPS"))?;
        self.layouts[i] = Some(Arc::new(Layout::new(sps, pps)?));
        Ok(())
    }

    /// Decodes NAL units in decoding order; every finished intra picture goes to `sink`
    /// (which returns `true` to stop). Returns how many pictures were skipped because they
    /// have inter slices.
    pub fn decode_units(
        &mut self,
        units: &[&[u8]],
        cancel: Option<&AtomicBool>,
        sink: &mut dyn FnMut(Decoded) -> bool,
    ) -> Result<usize> {
        let mut first_err = None;
        for u in units {
            if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
                return Err(Error::Cancelled);
            }
            let h = match Header::parse(u) {
                Ok(h) => h,
                Err(e) => {
                    first_err.get_or_insert(e);
                    continue;
                }
            };
            if h.layer_id != 0 {
                continue;
            }
            let r = match h.nal_type {
                nal::PH => {
                    if self.finish(sink)? {
                        return Ok(self.skipped);
                    }
                    self.picture_header(u)
                }
                nal::AUD | nal::EOS | nal::EOB => {
                    if self.finish(sink)? {
                        return Ok(self.skipped);
                    }
                    if h.nal_type != nal::AUD {
                        self.prev_tid0_poc = None;
                    }
                    Ok(())
                }
                t if t <= 11 => self.slice(h, u, cancel, sink),
                _ => self.non_vcl(h, u),
            };
            match r {
                Ok(()) => {}
                Err(Error::Cancelled) => return Err(Error::Cancelled),
                Err(Error::NeedsMemory(n)) => return Err(Error::NeedsMemory(n)),
                Err(Error::Unsupported(m)) if self.cur.as_ref().is_none_or(|c| !c.inter) => {
                    return Err(Error::Unsupported(m));
                }
                Err(e) => {
                    first_err.get_or_insert(e);
                }
            }
            if self.stop {
                return Ok(self.skipped);
            }
        }
        self.finish(sink)?;
        if self.emitted == 0 {
            if let Some(e) = first_err {
                return Err(e);
            }
            if self.skipped > 0 {
                return Err(Error::Unsupported(
                    "inter-coded pictures (P and B slices); only intra pictures decode",
                ));
            }
        }
        Ok(self.skipped)
    }

    fn picture_header(&mut self, unit: &[u8]) -> Result<()> {
        let rbsp = unescape_with_positions(&unit[2..]).0;
        let mut r = BitReader::new(&rbsp);
        let sps = &self.sps;
        let pps = &self.pps;
        let ph = parse_picture_header(&mut r, |id| lookup(sps, pps, id))?;
        self.ph = Some(ph);
        Ok(())
    }

    fn slice(
        &mut self,
        h: Header,
        unit: &[u8],
        cancel: Option<&AtomicBool>,
        sink: &mut dyn FnMut(Decoded) -> bool,
    ) -> Result<()> {
        let (rbsp, epb) = unescape_with_positions(&unit[2..]);
        // The PPS id is in the PH; make sure its layout exists before parsing the header.
        let peek_pps = peek_pps_id(&rbsp, self.ph.as_ref(), &self.sps, &self.pps)?;
        self.layout(peek_pps)?;
        let mut r = BitReader::new(&rbsp);
        let (sh, own_ph) = {
            let sps = &self.sps;
            let pps = &self.pps;
            let layouts = &self.layouts;
            parse_slice_header(
                &mut r,
                h.nal_type,
                self.ph.as_ref(),
                |id| lookup(sps, pps, id),
                |id| layouts[usize::from(id)].as_deref(),
            )?
        };
        if let Some(ph) = own_ph {
            if self.finish(sink)? {
                self.stop = true;
                return Ok(());
            }
            self.ph = Some(ph);
        }
        if self.cur.is_none() {
            self.start_picture(h)?;
        }
        let cur = self.cur.as_mut().ok_or(Error::NoSlices)?;
        if sh.slice_type != SliceType::I {
            cur.inter = true;
        }
        if cur.inter {
            return Err(Error::Unsupported(
                "inter-coded pictures (P and B slices); only intra pictures decode",
            ));
        }
        let slice_idx = cur.slices.len();
        cur.slices.push(filter::SliceFilter {
            dbf: sh.dbf,
            sao_luma: sh.sao_luma,
            sao_chroma: sh.sao_chroma,
            alf: sh.alf.clone(),
            cb_qp_offset: sh.cb_qp_offset,
            cr_qp_offset: sh.cr_qp_offset,
        });
        cur.lmcs_used.push(sh.lmcs_used);
        let scaling = if sh.explicit_scaling_list_used {
            Some(
                self.scaling[usize::from(cur.ph.scaling_list_aps_id) & 7]
                    .clone()
                    .ok_or(Error::MissingParameterSet("scaling list APS"))?,
            )
        } else {
            None
        };
        let data = slice_data(&rbsp, &epb, &sh)?;
        let ctx = SliceCtx {
            sps: &cur.sps,
            pps: &cur.pps,
            layout: &cur.layout,
            ph: &cur.ph,
            sh: &sh,
            slice_idx: slice_idx as u32,
            lmcs: if sh.lmcs_used {
                cur.lmcs.as_deref()
            } else {
                None
            },
            scaling: scaling.as_deref(),
            alf_aps: &cur.alf_aps,
            cancel,
        };
        match ctu::decode_slice(&ctx, &data, &mut cur.state) {
            Ok(()) => Ok(()),
            Err(e @ (Error::Cancelled | Error::Unsupported(_) | Error::NeedsMemory(_))) => Err(e),
            Err(e) => {
                cur.damaged += cur.state.undecoded_in(&sh.ctbs);
                Err(e)
            }
        }
    }

    fn start_picture(&mut self, h: Header) -> Result<()> {
        let ph = self
            .ph
            .clone()
            .ok_or(Error::InvalidData("slice without a picture header"))?;
        let pid = usize::from(ph.pps_id);
        self.layout(ph.pps_id)?;
        let pps = self.pps[pid]
            .clone()
            .ok_or(Error::MissingParameterSet("PPS"))?;
        let sps = self.sps[usize::from(pps.sps_id)]
            .clone()
            .ok_or(Error::MissingParameterSet("SPS"))?;
        check_supported(&sps)?;
        let layout = self.layouts[pid]
            .clone()
            .ok_or(Error::MissingParameterSet("PPS"))?;
        // POC (8.3.1).
        let max_lsb = 1i32 << sps.log2_max_poc_lsb;
        let lsb = ph.poc_lsb as i32;
        let clvss = h.is_irap() || h.nal_type == nal::GDR;
        if h.is_idr() || (clvss && self.prev_tid0_poc.is_none()) {
            self.cvs += 1;
        }
        let msb = if let Some(c) = ph.poc_msb_cycle {
            c as i32 * max_lsb
        } else if clvss && self.prev_tid0_poc.is_none() || h.is_idr() {
            0
        } else if let Some((plsb, pmsb)) = self.prev_tid0_poc {
            let plsb = plsb as i32;
            if lsb < plsb && plsb - lsb >= max_lsb / 2 {
                pmsb + max_lsb
            } else if lsb > plsb && lsb - plsb > max_lsb / 2 {
                pmsb - max_lsb
            } else {
                pmsb
            }
        } else {
            0
        };
        if h.temporal_id == 0
            && !ph.non_ref_pic
            && h.nal_type != nal::RASL
            && h.nal_type != nal::RADL
        {
            self.prev_tid0_poc = Some((ph.poc_lsb, msb));
        }
        let samples = u64::from(pps.width) * u64::from(pps.height);
        // Picture (up to 16-bit 4:4:4) plus per-sample CU, motion and filter state: ~16 bytes.
        let mem = specular_core::budget::reserve_n(samples, 16).map_err(Error::NeedsMemory)?;
        let pic = Picture::new(pps.width, pps.height, sps.chroma_format_idc, sps.bit_depth)?;
        let lmcs = if ph.lmcs_enabled {
            let aps = self.lmcs[usize::from(ph.lmcs_aps_id) & 3]
                .as_ref()
                .ok_or(Error::MissingParameterSet("LMCS APS"))?;
            Some(Arc::new(Lmcs::new(aps, sps.bit_depth)?))
        } else {
            None
        };
        let state = PicState::new(&sps, &pps, &layout, pic)?;
        self.cur = Some(Current {
            ph,
            sps,
            pps,
            layout,
            state,
            slices: Vec::new(),
            poc: msb + lsb,
            cvs: self.cvs,
            inter: false,
            alf_aps: self.alf.clone(),
            lmcs,
            lmcs_used: Vec::new(),
            damaged: 0,
            _mem: mem,
        });
        Ok(())
    }

    /// Finishes the picture in progress; true when the sink asked to stop.
    fn finish(&mut self, sink: &mut dyn FnMut(Decoded) -> bool) -> Result<bool> {
        let Some(mut cur) = self.cur.take() else {
            return Ok(false);
        };
        if cur.inter {
            self.skipped += 1;
            return Ok(false);
        }
        let total = (cur.layout.w_ctbs * cur.layout.h_ctbs) as usize;
        let missing = cur.state.undecoded();
        let damaged = cur.damaged.max(missing);
        if missing == total {
            return Ok(false);
        }
        // Inverse luma mapping (8.8.2), per slice.
        if let Some(l) = cur.lmcs.as_deref()
            && cur.lmcs_used.iter().any(|&u| u)
        {
            let state = &mut cur.state;
            l.inverse_picture(
                &mut state.pic,
                &state.ctb_slice,
                &cur.lmcs_used,
                &cur.layout,
            );
        }
        let ctx = filter::FilterCtx {
            sps: &cur.sps,
            pps: &cur.pps,
            layout: &cur.layout,
            ctb_slice: &cur.state.ctb_slice,
            slices: &cur.slices,
            sao: &cur.state.sao,
            alf: &cur.state.alf,
            alf_aps: &cur.alf_aps,
            vb_x: &cur.ph.vb_x,
            vb_y: &cur.ph.vb_y,
        };
        filter::run(&mut cur.state.pic, &cur.state.meta, &ctx, self.threads);
        let frame = crop(&cur.sps, &cur.pps, &cur.state.pic)?;
        self.emitted += 1;
        let d = Decoded {
            frame,
            damaged,
            total,
            signal: cur.sps.signal,
            poc: cur.poc,
            cvs: cur.cvs,
        };
        Ok(sink(d))
    }
}

fn lookup<'a>(
    sps: &'a [Option<Arc<Sps>>],
    pps: &'a [Option<Arc<Pps>>],
    id: u8,
) -> Option<(&'a Sps, &'a Pps)> {
    let p = pps.get(usize::from(id))?.as_deref()?;
    let s = sps.get(usize::from(p.sps_id))?.as_deref()?;
    Some((s, p))
}

/// The PPS id a slice refers to: from its own picture header or the one in force.
fn peek_pps_id(
    rbsp: &[u8],
    ph: Option<&PictureHeader>,
    sps: &[Option<Arc<Sps>>],
    pps: &[Option<Arc<Pps>>],
) -> Result<u8> {
    let mut r = BitReader::new(rbsp);
    if r.flag()? {
        let ph = parse_picture_header(&mut r, |id| lookup(sps, pps, id))?;
        return Ok(ph.pps_id);
    }
    ph.map(|p| p.pps_id)
        .ok_or(Error::InvalidData("slice without a picture header"))
}

/// The slice data and its substreams (entry points are counted in NAL unit bytes, which
/// include emulation prevention bytes; `epb` are their positions in the escaped payload).
fn slice_data(rbsp: &[u8], epb: &[usize], sh: &SliceHeader) -> Result<ctu::SliceData> {
    let start = sh.data_offset;
    if start > rbsp.len() {
        return Err(Error::Truncated);
    }
    // Escaped position of the first slice data byte.
    let mut esc_start = start;
    for &p in epb {
        if p <= esc_start {
            esc_start += 1;
        } else {
            break;
        }
    }
    let to_rbsp = |esc: usize| -> usize { esc - epb.iter().filter(|&&p| p < esc).count() };
    let mut bounds = vec![start];
    for &e in &sh.entry_points {
        let b = to_rbsp(esc_start + e);
        if b <= *bounds.last().unwrap_or(&0) || b > rbsp.len() {
            return Err(Error::InvalidData("entry point outside the slice data"));
        }
        bounds.push(b);
    }
    bounds.push(rbsp.len());
    Ok(ctu::SliceData {
        rbsp: rbsp.to_vec(),
        bounds,
        has_entry_points: !sh.entry_points.is_empty(),
    })
}

/// Crops `pic` to the conformance window.
fn crop(sps: &Sps, pps: &Pps, pic: &Picture) -> Result<Frame> {
    let (w, h, (l, t)) = cropped_size(sps, pps)?;
    let chroma = ChromaFormat::from_idc(sps.chroma_format_idc).unwrap_or_default();
    let take = |p: &crate::picture::Plane, x0: usize, y0: usize, w: usize, h: usize| {
        let mut v = Vec::with_capacity(w * h);
        for y in 0..h {
            let row = (y0 + y) * p.stride + x0;
            v.extend_from_slice(&p.data[row..row + w]);
        }
        v
    };
    let y = take(
        &pic.planes[0],
        l as usize,
        t as usize,
        w as usize,
        h as usize,
    );
    let (mut cb, mut cr, mut cs) = (Vec::new(), Vec::new(), 0);
    if chroma != ChromaFormat::Monochrome {
        let (sx, sy) = (chroma.x_shift(), chroma.y_shift());
        let cw = (w as usize).div_ceil(1 << sx);
        let ch = (h as usize).div_ceil(1 << sy);
        cb = take(
            &pic.planes[1],
            (l >> sx) as usize,
            (t >> sy) as usize,
            cw,
            ch,
        );
        cr = take(
            &pic.planes[2],
            (l >> sx) as usize,
            (t >> sy) as usize,
            cw,
            ch,
        );
        cs = cw as u32;
    }
    Ok(Frame {
        width: w,
        height: h,
        bit_depth: sps.bit_depth as u8,
        chroma,
        y,
        cb,
        cr,
        y_stride: w,
        c_stride: cs,
    })
}

#[cfg(test)]
mod debug_tests {
    /// Prints the SPS and PPS of a corpus stream: `SPECULAR_VVC_PS=NAME cargo test ps_dump`.
    #[test]
    fn ps_dump() {
        let Ok(name) = std::env::var("SPECULAR_VVC_PS") else {
            return;
        };
        let p = format!(
            "{}/../../../tools/corpus/conformance/vvc/{name}.bit",
            env!("CARGO_MANIFEST_DIR")
        );
        let data = std::fs::read(p).unwrap();
        for u in crate::nal::split_annexb(&data) {
            let h = crate::nal::Header::parse(u).unwrap();
            let rbsp = super::unescape_with_positions(&u[2..]).0;
            match h.nal_type {
                crate::nal::SPS => eprintln!("{:?}", super::parse_sps(&rbsp).unwrap()),
                crate::nal::PPS => eprintln!("{:?}", super::parse_pps(&rbsp).unwrap()),
                _ => {}
            }
        }
    }
}
