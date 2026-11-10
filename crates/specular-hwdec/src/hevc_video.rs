//! HEVC video (a motion clip's `hvcC` and its samples) → VA-API buffers, picture by picture:
//! the decoding process around the fixed-function engine. H.265 §8.3.1 picture order count,
//! §8.3.2 reference picture set and marking, §8.3.4 reference picture lists, and the pred-weight
//! table; the slice data itself is the engine's.
//!
//! The decoded picture buffer here holds surfaces, not samples: a picture is read back as soon
//! as it is decoded, so the DPB only keeps reference pictures. Which pictures are output at all
//! follows Annex C.5.2 all the same (output order itself is the container's composition times):
//! the pictures waiting for output are tracked by POC and bumped as the standard says, so a
//! picture with `pic_output_flag` 0, or one still waiting when an IRAP picture with
//! `NoOutputOfPriorPicsFlag` arrives, is never shown ([`HevcStream::take_output`]). A picture
//! whose reference is missing is refused (never decoded from a garbage surface); RASL pictures
//! after the first CRA are skipped as the standard says.

use crate::hevc::{self, Limits, ParamSets, Reject};
use crate::params::{self as ffi, IqMatrixHevc, PicParamHevc, PictureHevc, SliceParamHevc};
use specular_heic::h265::dpb::{LongTermEntry, build_rps_poc_lists};
use specular_heic::h265::hvcc::parse_hvcc;
use specular_heic::h265::nal::strip_emulation_prevention;
use specular_heic::h265::poc::{NalKind, PocState};
use specular_heic::h265::pps::PicParameterSet;
use specular_heic::h265::slice::{SliceLongTermRefPicSource, SliceSegmentHeader, SliceType};
use specular_heic::h265::sps::{
    MaterializedShortTermRefPicSet, SeqParameterSet, ShortTermRefPicSet,
};
use specular_heic::h265::vui::VideoSignalType;
use std::collections::HashMap;

fn bad(s: impl Into<String>) -> Reject {
    Reject::Malformed(s.into())
}

/// One reference picture in the decoded picture buffer.
#[derive(Clone, Copy, Debug)]
struct Ref {
    poc: i32,
    slot: usize,
    long_term: bool,
    /// The picture's serial number (as in [`HevcJob::id`]).
    id: u64,
}

/// A decoded picture waiting for output (C.5.2: "needed for output").
#[derive(Clone, Copy, Debug)]
struct Waiting {
    poc: i32,
    id: u64,
    /// `PicLatencyCount`.
    latency: u32,
}

/// One picture ready for the engine.
pub struct HevcJob<'a> {
    pub pic: PicParamHevc,
    pub iq: Option<IqMatrixHevc>,
    pub slices: Vec<(SliceParamHevc, &'a [u8])>,
    /// The surface slot to decode into (`CurrPic`'s id is filled in from it).
    pub slot: usize,
    /// The picture's serial number, which [`HevcStream::take_output`] names it by.
    pub id: u64,
    /// `PicOutputFlag`: whether the picture is ever output. When it is, it waits for
    /// [`HevcStream::take_output`] to say it is output or discarded.
    pub output: bool,
    /// The PPS `uniform_spacing_flag` (DXVA carries it; VA-API takes the tile sizes only).
    #[cfg_attr(not(windows), allow(dead_code))]
    pub uniform_spacing: bool,
    /// `NumDeltaPocs[RefRpsIdx]` of a slice RPS predicted from an SPS one (DXVA's
    /// `ucNumDeltaPocsOfRefRpsIdx`), else 0.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub num_delta_pocs_of_ref_rps_idx: u8,
}

/// The decoding state of one HEVC stream.
pub struct HevcStream {
    length_size: usize,
    sps: HashMap<u8, SeqParameterSet>,
    pps: HashMap<u8, PicParameterSet>,
    /// (sps id, pps id) pairs that passed [`hevc::check_param_sets`].
    checked: Vec<(u8, u8)>,
    lim: Limits,
    poc: PocState,
    dpb: Vec<Ref>,
    /// No picture decoded yet, or an end of sequence: the next IRAP starts afresh.
    first: bool,
    /// The last IRAP had `NoRaslOutputFlag`: its RASL pictures are not decodable.
    skip_rasl: bool,
    /// No picture placed since the stream (re)started: C.5.2.2's "picture 0".
    no_pic_yet: bool,
    /// Serial number of the next picture.
    next_id: u64,
    /// Pictures needed for output (C.5.2), in decoding order.
    waiting: Vec<Waiting>,
    /// Output decisions not yet taken: (picture id, output). `false`: discarded unseen.
    decided: Vec<(u64, bool)>,
    /// Surfaces in the pool.
    slots: usize,
    /// The coded size, bit depth and crop of the first SPS (the pool's).
    pub coded: (u32, u32),
    pub bit_depth: u8,
    pub crop: [u32; 4],
    pub signal: Option<VideoSignalType>,
    /// The DPB size the stream declares (`sps_max_dec_pic_buffering`).
    pub dpb_size: usize,
}

impl HevcStream {
    /// The stream's configuration record (`hvcC`): its parameter sets, checked against `lim`.
    pub fn new(hvcc: &[u8], lim: Limits) -> Result<HevcStream, Reject> {
        let rec = parse_hvcc(hvcc).map_err(|e| bad(format!("hvcC: {e:?}")))?;
        let mut st = HevcStream {
            length_size: rec.length_size,
            sps: HashMap::new(),
            pps: HashMap::new(),
            checked: Vec::new(),
            lim,
            poc: PocState::new(),
            dpb: Vec::new(),
            first: true,
            skip_rasl: false,
            no_pic_yet: true,
            next_id: 0,
            waiting: Vec::new(),
            decided: Vec::new(),
            slots: 0,
            coded: (0, 0),
            bit_depth: 8,
            crop: [0; 4],
            signal: None,
            dpb_size: 1,
        };
        for n in rec.nal_units.iter().filter(|n| n.header.nuh_layer_id == 0) {
            st.param_set(n.header.nal_unit_type, &n.escaped)?;
        }
        if st.sps.is_empty() {
            return Err(bad("hvcC without an SPS"));
        }
        // With several SPSs, one some PPS refers to (the lowest PPS id: HashMap order is not
        // stable, and an SPS without a PPS can't be checked).
        let (p, s) = st
            .pps
            .values()
            .filter_map(|p| Some((p, st.sps.get(&p.sps_id)?)))
            .min_by_key(|(p, _)| p.pps_id)
            .ok_or_else(|| bad("hvcC without a PPS"))?;
        let ps = ParamSets {
            sps: s.clone(),
            pps: p.clone(),
        };
        hevc::check_param_sets(&ps, &st.lim)?;
        st.checked.push((s.sps_id, p.pps_id));
        st.coded = (s.pic_width_in_luma_samples, s.pic_height_in_luma_samples);
        st.bit_depth = 8 + s.bit_depth_luma_minus8;
        let cw = &s.conformance_window;
        st.crop = [
            cw.left_offset * 2,
            cw.right_offset * 2,
            cw.top_offset * 2,
            cw.bottom_offset * 2,
        ];
        st.signal = s.vui_parameters.as_ref().and_then(|v| v.video_signal_type);
        st.dpb_size = s
            .sub_layer_ordering_info
            .iter()
            .map(|x| x.max_dec_pic_buffering_minus1 as usize + 1)
            .max()
            .unwrap_or(16)
            .clamp(1, 16);
        Ok(st)
    }

    /// Surfaces the pool needs: every reference plus the picture being decoded.
    pub fn surfaces_needed(&self) -> usize {
        (self.dpb_size + 1).min(17)
    }

    pub fn set_slots(&mut self, n: usize) {
        self.slots = n;
    }

    /// Forgets the decoded pictures (before decoding from a sync sample again), and the ones
    /// waiting for output (discarded).
    pub fn reset(&mut self) {
        self.dpb.clear();
        self.poc = PocState::new();
        self.first = true;
        self.skip_rasl = false;
        self.no_pic_yet = true;
        self.decided
            .extend(self.waiting.drain(..).map(|w| (w.id, false)));
    }

    /// The end of the stream: every picture still waiting is output (C.5.2.2 at the end of
    /// the bitstream), in POC order.
    pub fn flush(&mut self) {
        while self.bump() {}
    }

    /// The output decisions made since the last call, in the order made: (picture id,
    /// output). Output pictures come in output order; `false` means the picture is discarded
    /// without being output. Every picture prepared with [`HevcJob::output`] set gets
    /// exactly one decision (at the latest at [`Self::flush`] or [`Self::reset`]).
    pub fn take_output(&mut self) -> Vec<(u64, bool)> {
        std::mem::take(&mut self.decided)
    }

    /// C.5.2.4 bumping: the waiting picture with the smallest POC is output. False when none
    /// waits.
    fn bump(&mut self) -> bool {
        let Some(i) = (0..self.waiting.len()).min_by_key(|&i| self.waiting[i].poc) else {
            return false;
        };
        let w = self.waiting.remove(i);
        self.decided.push((w.id, true));
        true
    }

    fn param_set(&mut self, ty: u8, payload: &[u8]) -> Result<(), Reject> {
        let rbsp = strip_emulation_prevention(payload);
        match ty {
            33 => {
                let s = SeqParameterSet::parse(&rbsp).map_err(|e| bad(format!("SPS: {e:?}")))?;
                if self.sps.get(&s.sps_id).is_none_or(|o| *o != s) {
                    self.checked.retain(|c| c.0 != s.sps_id);
                    self.sps.insert(s.sps_id, s);
                }
            }
            34 => {
                let v1 = |id| {
                    self.sps
                        .get(&id)
                        .is_some_and(|s| s.ptl.is_version1_profile())
                };
                let p = PicParameterSet::parse_with(&rbsp, v1)
                    .map_err(|e| bad(format!("PPS: {e:?}")))?;
                if self.pps.get(&p.pps_id).is_none_or(|o| *o != p) {
                    self.checked.retain(|c| c.1 != p.pps_id);
                    self.pps.insert(p.pps_id, p);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Prepares one sample (an access unit) for the engine. `Ok(None)`: nothing to decode (no
    /// picture, or a RASL picture of a CRA that started decoding). `surfaces` are the pool's
    /// surface ids, indexed by slot.
    pub fn prepare<'a>(
        &mut self,
        sample: &'a [u8],
        surfaces: &[ffi::Id],
    ) -> Result<Option<HevcJob<'a>>, Reject> {
        let nals = hevc::split_nals(sample, self.length_size)?;
        let mut vcl: Vec<&'a [u8]> = Vec::new();
        let mut end_of_seq = false;
        for n in nals {
            // An end of sequence NAL unit is its two-byte header alone.
            if n.len() < 2 || n.len() < 3 && !matches!(hevc::nal_type(n), 36 | 37) {
                continue;
            }
            if n[0] & 0x80 != 0 {
                return Err(bad("forbidden_zero_bit"));
            }
            let layer = ((n[0] & 1) << 5) | (n[1] >> 3);
            if layer != 0 {
                // Layered (MV-HEVC / alpha) NAL units: the base layer is the picture.
                continue;
            }
            match hevc::nal_type(n) {
                0..=9 | 16..=21 => vcl.push(n),
                t @ (32..=34) => self.param_set(t, &n[2..])?,
                // End of sequence or bitstream (after the sample's picture): the next picture
                // is an IRAP picture with NoRaslOutputFlag 1.
                36 | 37 => end_of_seq = true,
                _ => {}
            }
        }
        let res = if vcl.is_empty() {
            Ok(None)
        } else {
            self.picture(vcl, surfaces)
        };
        if end_of_seq {
            self.first = true;
        }
        res
    }

    /// Places one picture (its slice segment NAL units) and prepares it for the engine.
    fn picture<'a>(
        &mut self,
        vcl: Vec<&'a [u8]>,
        surfaces: &[ffi::Id],
    ) -> Result<Option<HevcJob<'a>>, Reject> {
        let t0 = hevc::nal_type(vcl[0]);
        if vcl.iter().any(|n| hevc::nal_type(n) != t0) {
            return Err(bad("slices of one picture with different NAL types"));
        }
        let tid = (vcl[0][1] & 7)
            .checked_sub(1)
            .ok_or_else(|| bad("TemporalId"))?;
        let kind = NalKind::new(t0);
        let irap = kind.is_irap();
        if self.first && !irap {
            // Decoding starts at a random access point.
            return Ok(None);
        }
        let no_rasl = irap && (kind.is_idr() || kind.is_bla() || self.first);
        if irap {
            self.skip_rasl = no_rasl;
        } else if kind.is_rasl() && self.skip_rasl {
            return Ok(None);
        }

        let pps_id = peek_pps_id(&vcl[0][2..], irap).ok_or_else(|| bad("slice PPS id"))?;
        let p = self
            .pps
            .get(&pps_id)
            .ok_or_else(|| bad(format!("PPS {pps_id} not sent")))?
            .clone();
        let s = self
            .sps
            .get(&p.sps_id)
            .ok_or_else(|| bad(format!("SPS {} not sent", p.sps_id)))?
            .clone();
        if (s.pic_width_in_luma_samples, s.pic_height_in_luma_samples) != self.coded
            || 8 + s.bit_depth_luma_minus8 != self.bit_depth
        {
            return Err(Reject::Unsupported(
                "the picture size or bit depth changes mid-stream".into(),
            ));
        }
        let ps = ParamSets { sps: s, pps: p };
        if !self.checked.contains(&(ps.sps.sps_id, ps.pps.pps_id)) {
            hevc::check_param_sets(&ps, &self.lim)?;
            self.checked.push((ps.sps.sps_id, ps.pps.pps_id));
        }
        let (s, p) = (&ps.sps, &ps.pps);

        // Slice headers (dependent segments take the independent one's fields).
        let mut headers: Vec<SliceSegmentHeader> = Vec::with_capacity(vcl.len());
        for (k, nal) in vcl.iter().enumerate() {
            let sh = hevc::slice_header(nal, t0, s, p)?;
            if (k == 0) != sh.first_slice_segment_in_pic_flag {
                return Err(bad("first_slice_segment_in_pic_flag"));
            }
            if sh.slice_pic_parameter_set_id != p.pps_id {
                return Err(Reject::Unsupported("slices use different PPSs".into()));
            }
            if sh.byte_offset_to_slice_data.is_none() {
                return Err(bad("slice header not fully parsed"));
            }
            headers.push(sh);
        }
        let first = &headers[0];
        if first.dependent_slice_segment_flag {
            return Err(bad("dependent slice first"));
        }
        let intra = headers
            .iter()
            .all(|h| h.dependent_slice_segment_flag || h.slice_type == Some(SliceType::I));

        // §8.3.1 POC.
        let max_lsb = 1u32 << (s.log2_max_pic_order_cnt_lsb_minus4 + 4);
        let poc = self
            .poc
            .decode_picture_poc(
                kind,
                tid,
                no_rasl,
                first.slice_pic_order_cnt_lsb.unwrap_or(0),
                max_lsb,
            )
            .val;

        // §8.3.2 RPS and marking.
        let (st_rps, st_rps_bits, num_delta_pocs_of_ref_rps_idx) = slice_rps(first, s)?;
        let lt = long_term_entries(first, s)?;
        let lists = build_rps_poc_lists(kind.is_idr(), poc, max_lsb, &st_rps, &lt);
        if irap && no_rasl {
            self.dpb.clear();
        }
        let find_lt = |dpb: &[Ref], target: i32, msb: bool| {
            dpb.iter().position(|r| {
                if msb {
                    r.poc == target
                } else {
                    (r.poc as u32) & (max_lsb - 1) == (target as u32) & (max_lsb - 1)
                }
            })
        };
        let lt_curr: Vec<Option<usize>> = lists
            .lt_curr
            .iter()
            .zip(&lists.curr_delta_poc_msb_present)
            .map(|(&t, &m)| find_lt(&self.dpb, t, m))
            .collect();
        let lt_foll: Vec<Option<usize>> = lists
            .lt_foll
            .iter()
            .zip(&lists.foll_delta_poc_msb_present)
            .map(|(&t, &m)| find_lt(&self.dpb, t, m))
            .collect();
        for &i in lt_curr.iter().chain(&lt_foll).flatten() {
            self.dpb[i].long_term = true;
        }
        let find_st = |dpb: &[Ref], poc: i32| dpb.iter().position(|r| !r.long_term && r.poc == poc);
        let before: Vec<Option<usize>> = lists
            .st_curr_before
            .iter()
            .map(|&x| find_st(&self.dpb, x))
            .collect();
        let after: Vec<Option<usize>> = lists
            .st_curr_after
            .iter()
            .map(|&x| find_st(&self.dpb, x))
            .collect();
        let foll: Vec<Option<usize>> = lists
            .st_foll
            .iter()
            .map(|&x| find_st(&self.dpb, x))
            .collect();
        if before
            .iter()
            .chain(&after)
            .chain(&lt_curr)
            .any(Option::is_none)
        {
            return Err(bad(format!(
                "picture {poc} refers to a picture not decoded"
            )));
        }
        // VA's ReferenceFrames: what the RPS names, in its order (8.3.2: StCurrBefore,
        // StCurrAfter, LtCurr, then the pictures kept for later ones), with the RPS
        // membership. VA-API gets the reference lists spelled out per slice; DXVA builds
        // them from the current subsets, in this order. Everything else is no longer a
        // reference.
        let mut order: Vec<usize> = Vec::new();
        for &i in before
            .iter()
            .chain(&after)
            .chain(&lt_curr)
            .chain(&foll)
            .chain(&lt_foll)
            .flatten()
        {
            if !order.contains(&i) {
                order.push(i);
            }
        }
        if order.len() > 15 {
            return Err(bad("more than 15 reference pictures"));
        }
        let mut keep = vec![false; self.dpb.len()];
        let mut refs = [PictureHevc::INVALID; 15];
        let mut index_of = vec![None; self.dpb.len()];
        for (n, &i) in order.iter().enumerate() {
            keep[i] = true;
            let r = &self.dpb[i];
            let mut flags = 0;
            if r.long_term {
                flags |= ffi::PICTURE_HEVC_LONG_TERM_REFERENCE;
            }
            if before.contains(&Some(i)) {
                flags |= ffi::PICTURE_HEVC_RPS_ST_CURR_BEFORE;
            } else if after.contains(&Some(i)) {
                flags |= ffi::PICTURE_HEVC_RPS_ST_CURR_AFTER;
            } else if lt_curr.contains(&Some(i)) {
                flags |= ffi::PICTURE_HEVC_RPS_LT_CURR;
            }
            refs[n] = PictureHevc {
                picture_id: surfaces[r.slot],
                pic_order_cnt: r.poc,
                flags,
                reserved: [0; 4],
            };
            index_of[i] = Some(n as u8);
        }
        let va_index = |i: Option<usize>| i.and_then(|i| index_of[i]).unwrap_or(0xff);
        let num_pic_total_curr = before.len() + after.len() + lt_curr.len();

        // A surface for the current picture.
        let slot = (0..self.slots.min(surfaces.len()))
            .find(|&k| {
                !self
                    .dpb
                    .iter()
                    .enumerate()
                    .any(|(i, r)| keep[i] && r.slot == k)
            })
            .ok_or_else(|| bad("the stream holds more references than its DPB size"))?;

        // Slices.
        let mut slices = Vec::with_capacity(vcl.len());
        let mut indep = 0;
        for (k, (nal, sh)) in vcl.iter().zip(&headers).enumerate() {
            if !sh.dependent_slice_segment_flag {
                indep = k;
            }
            let hdr = &headers[indep];
            let off = sh.byte_offset_to_slice_data.unwrap_or(0);
            let (esc, emu) = hevc::escaped_offset(&nal[2..], off);
            if 2 + esc >= nal.len() {
                return Err(bad("no slice data"));
            }
            let mut sp = hevc::slice_param(k + 1 == vcl.len(), nal, sh, hdr, p, (esc, emu));
            let ty = hdr.slice_type.ok_or_else(|| bad("slice type"))?;
            if ty != SliceType::I {
                if num_pic_total_curr == 0 {
                    return Err(bad("inter slice without references"));
                }
                let n0 = hdr.num_ref_idx_l0_active_minus1.unwrap_or(0) as usize + 1;
                let n1 = hdr.num_ref_idx_l1_active_minus1.unwrap_or(0) as usize + 1;
                if n0 > 15 || n1 > 15 {
                    return Err(bad("more than 15 active references"));
                }
                let m = hdr.ref_pic_lists_modification.as_ref();
                let l0 = ref_list(
                    &[&before, &after, &lt_curr],
                    n0,
                    num_pic_total_curr,
                    m.filter(|m| m.ref_pic_list_modification_flag_l0)
                        .map(|m| m.list_entry_l0.as_slice()),
                )?;
                for (d, i) in sp.ref_pic_list[0].iter_mut().zip(l0) {
                    *d = va_index(i);
                }
                sp.num_ref_idx_l0_active_minus1 = (n0 - 1) as u8;
                if ty == SliceType::B {
                    let l1 = ref_list(
                        &[&after, &before, &lt_curr],
                        n1,
                        num_pic_total_curr,
                        m.filter(|m| m.ref_pic_list_modification_flag_l1 == Some(true))
                            .map(|m| m.list_entry_l1.as_slice()),
                    )?;
                    for (d, i) in sp.ref_pic_list[1].iter_mut().zip(l1) {
                        *d = va_index(i);
                    }
                    sp.num_ref_idx_l1_active_minus1 = (n1 - 1) as u8;
                }
                let col_l0 = hdr.collocated_from_l0_flag.unwrap_or(true);
                sp.long_slice_flags |= (hdr.mvd_l1_zero_flag.unwrap_or(false) as u32) << 8
                    | (hdr.cabac_init_flag.unwrap_or(false) as u32) << 9
                    | (hdr.slice_temporal_mvp_enabled_flag as u32) << 10
                    | (col_l0 as u32) << 12;
                sp.collocated_ref_idx = if hdr.slice_temporal_mvp_enabled_flag {
                    hdr.collocated_ref_idx.unwrap_or(0).min(14) as u8
                } else {
                    0xff
                };
                sp.five_minus_max_num_merge_cand =
                    hdr.five_minus_max_num_merge_cand.unwrap_or(0).min(4) as u8;
                if let Some(w) = &hdr.pred_weight_table {
                    pred_weights(&mut sp, w);
                }
            }
            slices.push((sp, *nal));
        }

        let mut pic = hevc::pic_param(&ps, irap, kind.is_idr(), intra, poc);
        pic.curr_pic.picture_id = surfaces[slot];
        pic.reference_frames = refs;
        pic.st_rps_bits = st_rps_bits;

        // The current picture becomes a short-term reference; the rest of the DPB is what the
        // RPS kept.
        let mut k = 0;
        self.dpb.retain(|_| {
            k += 1;
            keep[k - 1]
        });
        let id = self.next_id;
        self.next_id += 1;
        let output = first.pic_output_flag;
        let no_output_of_prior =
            kind.is_cra() || first.no_output_of_prior_pics_flag.unwrap_or(false);
        self.output_process(s, irap && no_rasl, no_output_of_prior, poc, id, output);
        self.dpb.push(Ref {
            poc,
            slot,
            long_term: false,
            id,
        });
        self.first = false;
        self.no_pic_yet = false;
        Ok(Some(HevcJob {
            pic,
            iq: s.scaling_list_enabled_flag.then(|| hevc::iq_matrix(&ps)),
            slices,
            slot,
            id,
            output,
            uniform_spacing: ps.pps.tiles_enabled_flag && ps.pps.tiles.uniform_spacing_flag,
            num_delta_pocs_of_ref_rps_idx,
        }))
    }

    /// C.5.2.2 and C.5.2.3 around the current picture (`id`), the DPB already marked by its
    /// RPS: output or discard what an IRAP picture with NoRaslOutputFlag ends, bump what the
    /// buffer can't hold, then let the picture join the waiting ones (when it is output) and
    /// bump what is due. Mirrors the software decoder (specular-hevc), so that both output the
    /// same pictures.
    fn output_process(
        &mut self,
        s: &SeqParameterSet,
        irap_no_rasl: bool,
        no_output_of_prior: bool,
        poc: i32,
        id: u64,
        output: bool,
    ) {
        // The highest sub-layer's limits.
        let o = &s.sub_layer_ordering_info[(s.max_sub_layers_minus1 as usize).min(6)];
        let max_dec = o
            .max_dec_pic_buffering_minus1
            .saturating_add(1)
            .clamp(1, 16) as usize;
        let max_reorder = (o.max_num_reorder_pics as usize).min(max_dec - 1);
        let max_latency = (o.max_latency_increase_plus1 != 0).then(|| {
            o.max_num_reorder_pics
                .saturating_add(o.max_latency_increase_plus1 - 1)
        });
        let late = |w: &[Waiting]| max_latency.is_some_and(|l| w.iter().any(|p| p.latency >= l));
        if irap_no_rasl && !self.no_pic_yet {
            // A CRA picture here follows an end of sequence: NoOutputOfPriorPicsFlag is 1
            // whatever its slice header says.
            if no_output_of_prior {
                self.decided
                    .extend(self.waiting.drain(..).map(|w| (w.id, false)));
            } else {
                self.flush();
            }
        } else {
            loop {
                // The buffer: the references the RPS kept and the pictures waiting for output.
                let refs = &self.dpb;
                let held = refs.len()
                    + self
                        .waiting
                        .iter()
                        .filter(|w| !refs.iter().any(|r| r.id == w.id))
                        .count();
                let due =
                    self.waiting.len() > max_reorder || late(&self.waiting) || held >= max_dec;
                if !due || !self.bump() {
                    break;
                }
            }
        }
        for w in &mut self.waiting {
            w.latency = w.latency.saturating_add(1);
        }
        if output {
            self.waiting.push(Waiting {
                poc,
                id,
                latency: 0,
            });
        }
        while (self.waiting.len() > max_reorder || late(&self.waiting)) && self.bump() {}
    }

    /// Drops the picture `prepare` just added (its decode failed): no longer a reference, and
    /// never output.
    pub fn forget_last(&mut self) {
        if let Some(r) = self.dpb.pop() {
            self.waiting.retain(|w| w.id != r.id);
            self.decided.retain(|d| d.0 != r.id);
        }
    }
}

/// `slice_pic_parameter_set_id` of a slice segment, read from the start of its payload
/// (after the NAL header): `first_slice_segment_in_pic_flag`, `no_output_of_prior_pics_flag`
/// (IRAP), then the `ue(v)`.
fn peek_pps_id(payload: &[u8], irap: bool) -> Option<u8> {
    let rbsp = strip_emulation_prevention(&payload[..payload.len().min(16)]);
    let mut pos = 1 + irap as usize;
    let bit = |p: usize| rbsp.get(p / 8).map(|b| (b >> (7 - p % 8)) & 1);
    let mut zeros = 0;
    while bit(pos)? == 0 {
        zeros += 1;
        pos += 1;
        if zeros > 6 {
            return None;
        }
    }
    pos += 1;
    let mut v = 1u32;
    for _ in 0..zeros {
        v = v << 1 | bit(pos)? as u32;
        pos += 1;
    }
    u8::try_from(v - 1).ok().filter(|&v| v < 64)
}

/// Bits of a `ue(v)` coding of `v`.
fn ue_bits(v: u32) -> u32 {
    2 * (63 - (v as u64 + 1).leading_zeros()) + 1
}

/// The slice's short-term RPS (materialized) and, for one coded in the slice header, the bits
/// its `st_ref_pic_set()` took (`st_rps_bits`, which the engine needs to skip it) and the
/// `NumDeltaPocs` of the SPS set it is predicted from (0 when it is not).
fn slice_rps(
    h: &SliceSegmentHeader,
    s: &SeqParameterSet,
) -> Result<(MaterializedShortTermRefPicSet, u32, u8), Reject> {
    let empty = MaterializedShortTermRefPicSet {
        delta_poc_s0: vec![],
        used_by_curr_pic_s0: vec![],
        delta_poc_s1: vec![],
        used_by_curr_pic_s1: vec![],
    };
    let Some(sps_flag) = h.short_term_ref_pic_set_sps_flag else {
        return Ok((empty, 0, 0));
    };
    let chain = s
        .materialize_short_term_ref_pic_sets()
        .map_err(|e| bad(format!("SPS RPS: {e:?}")))?;
    if sps_flag {
        let i = h.short_term_ref_pic_set_idx.unwrap_or(0) as usize;
        let r = chain
            .get(i)
            .cloned()
            .ok_or_else(|| bad("short_term_ref_pic_set_idx"))?;
        return Ok((r, 0, 0));
    }
    let set = h
        .inline_short_term_ref_pic_set
        .as_ref()
        .ok_or_else(|| bad("inline RPS missing"))?;
    let source = if set.inter_ref_pic_set_prediction_flag {
        let i = chain
            .len()
            .checked_sub(set.delta_idx_minus1 as usize + 1)
            .ok_or_else(|| bad("RefRpsIdx"))?;
        Some(&chain[i])
    } else {
        None
    };
    let r = set
        .materialize(source)
        .map_err(|e| bad(format!("slice RPS: {e:?}")))?;
    let ref_delta_pocs = source.map_or(0, |s| s.num_delta_pocs().min(255) as u8);
    Ok((r, st_rps_bits(set, chain.len()), ref_delta_pocs))
}

/// Bits of the slice-header `st_ref_pic_set(num_short_term_ref_pic_sets)` syntax.
fn st_rps_bits(set: &ShortTermRefPicSet, idx: usize) -> u32 {
    let mut bits = (idx != 0) as u32;
    if set.inter_ref_pic_set_prediction_flag {
        bits += ue_bits(set.delta_idx_minus1) + 1 + ue_bits(set.abs_delta_rps_minus1);
        for &used in &set.used_by_curr_pic_flag {
            bits += 1 + (!used) as u32;
        }
    } else {
        bits += ue_bits(set.num_negative_pics) + ue_bits(set.num_positive_pics);
        for &d in set
            .delta_poc_s0_minus1
            .iter()
            .chain(&set.delta_poc_s1_minus1)
        {
            bits += ue_bits(d) + 1;
        }
    }
    bits
}

/// The slice's long-term entries with `PocLsbLt` resolved and `DeltaPocMsbCycleLt` accumulated
/// (H.265 eq. 7-52).
fn long_term_entries(
    h: &SliceSegmentHeader,
    s: &SeqParameterSet,
) -> Result<Vec<LongTermEntry>, Reject> {
    let num_sps = h.num_long_term_sps.unwrap_or(0) as usize;
    let mut out: Vec<LongTermEntry> = Vec::with_capacity(h.long_term_ref_pics.len());
    for (i, lt) in h.long_term_ref_pics.iter().enumerate() {
        let poc_lsb_lt = match lt.source {
            SliceLongTermRefPicSource::Sps { lt_idx_sps } => {
                s.long_term_ref_pics
                    .get(lt_idx_sps as usize)
                    .ok_or_else(|| bad("lt_idx_sps"))?
                    .poc_lsb
            }
            SliceLongTermRefPicSource::InSlice { poc_lsb_lt, .. } => poc_lsb_lt,
        };
        let used = lt.used_by_curr_pic_lt(s).ok_or_else(|| bad("lt_idx_sps"))?;
        let cycle = if i == 0 || i == num_sps {
            lt.delta_poc_msb_cycle_lt
        } else {
            lt.delta_poc_msb_cycle_lt
                .saturating_add(out[i - 1].delta_poc_msb_cycle_lt)
        };
        out.push(LongTermEntry {
            poc_lsb_lt,
            used_by_curr_pic_lt: used,
            delta_poc_msb_present: lt.delta_poc_msb_present_flag,
            delta_poc_msb_cycle_lt: cycle,
        });
    }
    Ok(out)
}

/// H.265 §8.3.4: the initial list cycles through `sets` until it holds
/// `max(active, NumPicTotalCurr)` entries; `entries` (the list modification) picks from it.
fn ref_list(
    sets: &[&Vec<Option<usize>>; 3],
    active: usize,
    total: usize,
    entries: Option<&[u32]>,
) -> Result<Vec<Option<usize>>, Reject> {
    let n = active.max(total);
    let mut temp = Vec::with_capacity(n);
    while temp.len() < n {
        for set in sets {
            for &e in set.iter() {
                if temp.len() < n {
                    temp.push(e);
                }
            }
        }
    }
    (0..active)
        .map(|i| {
            let k = match entries {
                Some(e) => *e.get(i).ok_or_else(|| bad("list_entry"))? as usize,
                None => i,
            };
            temp.get(k)
                .copied()
                .ok_or_else(|| bad("list_entry out of range"))
        })
        .collect()
}

/// The pred-weight table in VA's form: the coded deltas and luma offsets as sent, the chroma
/// offsets derived (H.265 eq. 7-56).
fn pred_weights(sp: &mut SliceParamHevc, w: &specular_heic::h265::slice::PredWeightTable) {
    let clamp8 = |v: i32| v.clamp(-128, 127) as i8;
    sp.luma_log2_weight_denom = w.luma_log2_weight_denom;
    sp.delta_chroma_log2_weight_denom = clamp8(w.delta_chroma_log2_weight_denom);
    // Without high-precision offsets (a range extension, rejected) the offsets are in 8-bit
    // units whatever the bit depth: WpOffsetHalfRangeC is 128.
    for (i, e) in w.entries_l0.iter().enumerate().take(15) {
        if e.luma_weight_flag {
            sp.delta_luma_weight_l0[i] = clamp8(e.delta_luma_weight);
            sp.luma_offset_l0[i] = clamp8(e.luma_offset);
        }
        if e.chroma_weight_flag {
            for j in 0..2 {
                sp.delta_chroma_weight_l0[i][j] = clamp8(e.delta_chroma_weight[j]);
                sp.chroma_offset_l0[i][j] = clamp8(w.chroma_offset_l0(i, j, 128).unwrap_or(0));
            }
        }
    }
    for (i, e) in w.entries_l1.iter().enumerate().take(15) {
        if e.luma_weight_flag {
            sp.delta_luma_weight_l1[i] = clamp8(e.delta_luma_weight);
            sp.luma_offset_l1[i] = clamp8(e.luma_offset);
        }
        if e.chroma_weight_flag {
            for j in 0..2 {
                sp.delta_chroma_weight_l1[i][j] = clamp8(e.delta_chroma_weight[j]);
                sp.chroma_offset_l1[i][j] = clamp8(w.chroma_offset_l1(i, j, 128).unwrap_or(0));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ue_widths() {
        assert_eq!(ue_bits(0), 1);
        assert_eq!(ue_bits(1), 3);
        assert_eq!(ue_bits(2), 3);
        assert_eq!(ue_bits(3), 5);
        assert_eq!(ue_bits(6), 5);
        assert_eq!(ue_bits(7), 7);
    }

    #[test]
    fn pps_id_peek() {
        // first_slice 1, (IRAP: no_output 0), pps id ue '1' = 0.
        assert_eq!(peek_pps_id(&[0b1010_0000, 0], true), Some(0));
        // first 1, pps id ue '010' = 1.
        assert_eq!(peek_pps_id(&[0b1010_0000, 0], false), Some(1));
        // first 0, pps id ue '00111' = 6.
        assert_eq!(peek_pps_id(&[0b0001_1100, 0], false), Some(6));
    }

    #[test]
    fn ref_lists_cycle_and_modify() {
        let (b, a, l) = (vec![Some(0), Some(1)], vec![Some(2)], vec![]);
        assert_eq!(
            ref_list(&[&b, &a, &l], 4, 3, None).unwrap(),
            vec![Some(0), Some(1), Some(2), Some(0)]
        );
        assert_eq!(
            ref_list(&[&a, &b, &l], 2, 3, Some(&[2, 0])).unwrap(),
            vec![Some(1), Some(2)]
        );
        assert!(ref_list(&[&b, &a, &l], 1, 3, Some(&[5])).is_err());
    }
}
