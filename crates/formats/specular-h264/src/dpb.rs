//! Reference picture management: picture order count (8.2.1), reference list initialisation
//! and modification (8.2.4) and reference marking (8.2.5) for frames and fields, including
//! frame number gaps.
//!
//! The DPB holds reference entries: frames, complementary field pairs and non-paired fields.
//! Each field of an entry is marked on its own, so field pictures can reference (and unmark)
//! single fields, while frames only use entries with both fields marked.

use std::sync::Arc;

use crate::params::Sps;
use crate::picture::PicShared;
use crate::slice::{Mmco, Modification, SliceHeader, SliceType};
use crate::slicedec::RefPic;

/// A reference entry: a frame, a complementary field pair or a non-paired field.
#[derive(Clone)]
pub struct RefFrame {
    pub pic: Arc<PicShared>,
    /// Picture id; the fields and the frame have the [`RefPic::id`]s `id << 2 | 1 + parity`
    /// and `id << 2`.
    pub id: u32,
    pub frame_num: u32,
    /// Field order counts (top, bottom); `i32::MAX` for a field that was not decoded.
    pub poc: [i32; 2],
    /// Fields marked "used for short-term reference" (bit = parity).
    pub short: u8,
    /// Fields marked "used for long-term reference".
    pub long: u8,
    /// LongTermFrameIdx of the long-term fields.
    pub long_idx: u32,
}

impl RefFrame {
    fn marked(&self) -> u8 {
        self.short | self.long
    }

    /// PicOrderCnt of the entry: the smaller of its decoded fields' counts.
    fn entry_poc(&self) -> i32 {
        self.poc[0].min(self.poc[1])
    }

    /// The frame as a reference.
    pub fn frame_ref(&self) -> RefPic {
        RefPic {
            pic: self.pic.clone(),
            id: self.id << 2,
            poc: self.entry_poc(),
            pocs: self.poc,
            long: self.long == 3,
            parity: None,
        }
    }

    /// Field `p` as a reference.
    pub fn field_ref(&self, p: u8) -> RefPic {
        RefPic {
            pic: self.pic.clone(),
            id: self.id << 2 | (1 + p as u32),
            poc: self.poc[p as usize],
            pocs: self.poc,
            long: self.long >> p & 1 != 0,
            parity: Some(p),
        }
    }
}

/// Picture order count state carried between pictures.
#[derive(Clone, Default)]
pub struct PocState {
    prev_msb: i32,
    prev_lsb: i32,
    prev_frame_num_offset: i32,
    prev_frame_num: u32,
}

/// FrameNumWrap (8.2.4.1), which is also PicNum for frames.
fn wrap(f: &RefFrame, cur: u32, max: u32) -> i32 {
    if f.frame_num > cur {
        f.frame_num as i32 - max as i32
    } else {
        f.frame_num as i32
    }
}

#[derive(Clone, Default)]
pub struct Dpb {
    pub refs: Vec<RefFrame>,
    max_long_idx: Option<u32>,
    pub poc: PocState,
    /// frame_num of the previous reference picture (PrevRefFrameNum).
    pub prev_ref_frame_num: u32,
}

/// The FrameNumOffset and POCs of a picture, as computed before decoding it.
pub struct PocResult {
    /// PicOrderCnt of the picture: of the field, or the smaller of a frame's two.
    pub poc: i32,
    frame_num_offset: i32,
    /// TopFieldOrderCnt and BottomFieldOrderCnt (for a field, both are its own count).
    pub top: i32,
    pub bottom: i32,
    msb: i32,
}

/// The current picture as reference marking sees it.
pub struct CurPic {
    pub pic: Arc<PicShared>,
    pub id: u32,
    /// Field parity of a field picture.
    pub parity: Option<u8>,
    /// The second field of a pair whose first field is in the DPB already (as `id`).
    pub second: bool,
    /// Field order counts after picture order count decoding (and MMCO 5).
    pub poc: [i32; 2],
}

impl Dpb {
    pub fn clear(&mut self) {
        self.refs.clear();
        self.max_long_idx = None;
        self.poc = PocState::default();
        self.prev_ref_frame_num = 0;
    }

    /// 8.2.1.
    pub fn compute_poc(&self, sps: &Sps, h: &SliceHeader) -> PocResult {
        let st = &self.poc;
        let max_frame_num = 1i64 << sps.log2_max_frame_num;
        let frame_num_offset = if h.idr {
            0
        } else if st.prev_frame_num > h.frame_num {
            (st.prev_frame_num_offset as i64 + max_frame_num) as i32
        } else {
            st.prev_frame_num_offset
        };
        let mut msb = 0;
        let (top, bottom) = match sps.poc_type {
            0 => {
                let (pm, pl) = if h.idr {
                    (0, 0)
                } else {
                    (st.prev_msb, st.prev_lsb)
                };
                let max = 1i32 << sps.log2_max_poc_lsb;
                let lsb = h.poc_lsb as i32;
                msb = if lsb < pl && pl - lsb >= max / 2 {
                    pm.wrapping_add(max)
                } else if lsb > pl && lsb - pl > max / 2 {
                    pm.wrapping_sub(max)
                } else {
                    pm
                };
                let v = msb.wrapping_add(lsb);
                if h.field_pic {
                    (v, v)
                } else {
                    (v, v.wrapping_add(h.delta_poc_bottom))
                }
            }
            1 => {
                let n = sps.offset_for_ref_frame.len() as i64;
                let mut abs = if n != 0 {
                    frame_num_offset as i64 + h.frame_num as i64
                } else {
                    0
                };
                if h.nal_ref_idc == 0 && abs > 0 {
                    abs -= 1;
                }
                // No overflow: with AbsFrameNum <= 2^31 + 2^16 and |offsets| <= 2^31,
                // |cycle * per_cycle| <= AbsFrameNum * 2^31 < 2^63 (minus a margin for the rest).
                let mut expected: i64 = 0;
                if abs > 0 {
                    let per_cycle: i64 = sps.offset_for_ref_frame.iter().map(|&v| v as i64).sum();
                    let cycle = (abs - 1) / n;
                    let in_cycle = ((abs - 1) % n) as usize;
                    expected = cycle * per_cycle;
                    for &o in &sps.offset_for_ref_frame[..=in_cycle] {
                        expected += o as i64;
                    }
                }
                if h.nal_ref_idc == 0 {
                    expected += sps.offset_for_non_ref_pic as i64;
                }
                let t2b = sps.offset_for_top_to_bottom_field as i64;
                let (top, bottom) = match h.parity() {
                    None => {
                        let top = expected + h.delta_poc[0] as i64;
                        (top, top + t2b + h.delta_poc[1] as i64)
                    }
                    Some(0) => {
                        let v = expected + h.delta_poc[0] as i64;
                        (v, v)
                    }
                    Some(_) => {
                        let v = expected + t2b + h.delta_poc[0] as i64;
                        (v, v)
                    }
                };
                (top as i32, bottom as i32)
            }
            _ => {
                let v = if h.idr {
                    0
                } else {
                    let t = 2 * (frame_num_offset as i64 + h.frame_num as i64);
                    if h.nal_ref_idc == 0 { t - 1 } else { t }
                };
                (v as i32, v as i32)
            }
        };
        PocResult {
            poc: top.min(bottom),
            frame_num_offset,
            top,
            bottom,
            msb,
        }
    }

    /// Updates the POC state after a picture; returns its field order counts as later
    /// pictures see them (changed by memory_management_control_operation 5).
    pub fn update_poc(&mut self, sps: &Sps, h: &SliceHeader, r: &PocResult) -> [i32; 2] {
        let mmco5 = h.has_mmco5();
        let (mut top, mut bottom) = (r.top, r.bottom);
        if mmco5 {
            match h.parity() {
                None => {
                    // Saturating: a damaged stream's top and bottom may lie 2^32 apart.
                    let temp = top.min(bottom);
                    top = top.saturating_sub(temp);
                    bottom = bottom.saturating_sub(temp);
                }
                Some(_) => {
                    top = 0;
                    bottom = 0;
                }
            }
        }
        if h.nal_ref_idc != 0 && sps.poc_type == 0 {
            if mmco5 {
                self.poc.prev_msb = 0;
                self.poc.prev_lsb = if h.parity() == Some(1) { 0 } else { top };
            } else {
                self.poc.prev_msb = r.msb;
                self.poc.prev_lsb = h.poc_lsb as i32;
            }
        }
        self.poc.prev_frame_num_offset = if mmco5 { 0 } else { r.frame_num_offset };
        self.poc.prev_frame_num = if mmco5 { 0 } else { h.frame_num };
        [top, bottom]
    }

    /// Reference picture lists of one slice (8.2.4), `num_ref_idx` entries each; missing
    /// entries are filled with a stand-in. `cur_poc` is PicOrderCnt(CurrPic).
    pub fn lists(
        &self,
        sps: &Sps,
        h: &SliceHeader,
        cur_poc: i32,
        fallback: &RefPic,
    ) -> [Vec<RefPic>; 2] {
        let mut out: [Vec<RefPic>; 2] = [Vec::new(), Vec::new()];
        if h.slice_type == SliceType::I {
            return out;
        }
        let max = 1u32 << sps.log2_max_frame_num;
        let cur = h.frame_num;
        let mut init: [Vec<RefPic>; 2] = [Vec::new(), Vec::new()];
        match h.parity() {
            None => {
                let mut long: Vec<&RefFrame> = self.refs.iter().filter(|f| f.long == 3).collect();
                long.sort_by_key(|f| f.long_idx);
                let short = || self.refs.iter().filter(|f| f.short == 3);
                if h.slice_type == SliceType::P {
                    let mut s: Vec<&RefFrame> = short().collect();
                    s.sort_by_key(|f| std::cmp::Reverse(wrap(f, cur, max)));
                    init[0] = s
                        .into_iter()
                        .chain(long.iter().copied())
                        .map(RefFrame::frame_ref)
                        .collect();
                } else {
                    let (before, after) = split_by_poc(short(), cur_poc);
                    let l0 = before.iter().chain(after.iter()).chain(long.iter());
                    init[0] = l0.map(|f| f.frame_ref()).collect();
                    let l1 = after.iter().chain(before.iter()).chain(long.iter());
                    init[1] = l1.map(|f| f.frame_ref()).collect();
                }
            }
            Some(par) => {
                let mut long: Vec<&RefFrame> = self.refs.iter().filter(|f| f.long != 0).collect();
                long.sort_by_key(|f| f.long_idx);
                let short = || self.refs.iter().filter(|f| f.short != 0);
                let long_fields = alternate(&long, par, |f| f.long);
                if h.slice_type == SliceType::P {
                    let mut s: Vec<&RefFrame> = short().collect();
                    s.sort_by_key(|f| std::cmp::Reverse(wrap(f, cur, max)));
                    init[0] = alternate(&s, par, |f| f.short);
                    init[0].extend(long_fields);
                } else {
                    let (before, after) = split_by_poc(short(), cur_poc);
                    let l0: Vec<&RefFrame> = before.iter().chain(after.iter()).copied().collect();
                    let l1: Vec<&RefFrame> = after.iter().chain(before.iter()).copied().collect();
                    init[0] = alternate(&l0, par, |f| f.short);
                    init[0].extend(long_fields.iter().cloned());
                    init[1] = alternate(&l1, par, |f| f.short);
                    init[1].extend(long_fields);
                }
            }
        }
        if h.slice_type == SliceType::B
            && init[1].len() > 1
            && init[0]
                .iter()
                .map(|r| r.id)
                .eq(init[1].iter().map(|r| r.id))
        {
            init[1].swap(0, 1);
        }
        let lists = if h.slice_type == SliceType::B { 2 } else { 1 };
        // 8.2.4.1 / 8.2.4.3: PicNum and LongTermPicNum.
        let (cur_pic_num, max_pic_num) = match h.parity() {
            None => (cur as i64, max as i64),
            Some(_) => (2 * cur as i64 + 1, 2 * max as i64),
        };
        for l in 0..lists {
            let n = h.num_ref_idx[l] as usize;
            let mut list: Vec<Option<RefPic>> = init[l].drain(..).map(Some).collect();
            list.truncate(n);
            list.resize(n, None);
            let mut pred = cur_pic_num;
            for (idx, m) in h.modifications[l].iter().enumerate() {
                if idx >= n {
                    break;
                }
                let found = match *m {
                    Modification::ShortTerm(d) => {
                        let mut no_wrap = pred + d as i64;
                        if no_wrap < 0 {
                            no_wrap += max_pic_num;
                        } else if no_wrap >= max_pic_num {
                            no_wrap -= max_pic_num;
                        }
                        pred = no_wrap;
                        let pic_num = if no_wrap > cur_pic_num {
                            no_wrap - max_pic_num
                        } else {
                            no_wrap
                        };
                        self.find_short(h, max, pic_num)
                    }
                    Modification::LongTerm(n) => self.find_long(h, n as i64),
                };
                let key = found.as_ref().map(|e| e.id);
                // Insert at idx, then drop the later duplicate of the same picture.
                list.insert(idx, found);
                let mut k = idx + 1;
                while k < list.len() {
                    if key.is_some() && list[k].as_ref().map(|e| e.id) == key {
                        list.remove(k);
                    } else {
                        k += 1;
                    }
                }
                list.truncate(n);
                list.resize(n, None);
            }
            let first = list.iter().flatten().next().cloned();
            out[l] = list
                .into_iter()
                .map(|e| {
                    e.or_else(|| first.clone())
                        .unwrap_or_else(|| fallback.clone())
                })
                .collect();
        }
        out
    }

    /// The short-term reference picture with PicNum `pic_num`.
    fn find_short(&self, h: &SliceHeader, max: u32, pic_num: i64) -> Option<RefPic> {
        let cur = h.frame_num;
        match h.parity() {
            None => self
                .refs
                .iter()
                .find(|f| f.short == 3 && wrap(f, cur, max) as i64 == pic_num)
                .map(RefFrame::frame_ref),
            Some(par) => {
                let (p, wr) = field_num(pic_num, par);
                self.refs
                    .iter()
                    .find(|f| f.short >> p & 1 != 0 && wrap(f, cur, max) as i64 == wr)
                    .map(|f| f.field_ref(p))
            }
        }
    }

    /// The long-term reference picture with LongTermPicNum `n`.
    fn find_long(&self, h: &SliceHeader, n: i64) -> Option<RefPic> {
        match h.parity() {
            None => self
                .refs
                .iter()
                .find(|f| f.long == 3 && f.long_idx as i64 == n)
                .map(RefFrame::frame_ref),
            Some(par) => {
                let (p, idx) = field_num(n, par);
                self.refs
                    .iter()
                    .find(|f| f.long >> p & 1 != 0 && f.long_idx as i64 == idx)
                    .map(|f| f.field_ref(p))
            }
        }
    }

    /// Frame number gap filling (8.2.5.2): frames for the missing numbers, sharing the samples
    /// of the latest reference.
    pub fn fill_gap(&mut self, sps: &Sps, frame_num: u32, mut next_id: impl FnMut() -> u32) {
        let max = 1u32 << sps.log2_max_frame_num;
        let Some(last) = self
            .refs
            .iter()
            .filter(|f| f.short != 0)
            .max_by_key(|f| wrap(f, frame_num, max))
            .or(self.refs.last())
            .cloned()
        else {
            return;
        };
        let mut fnum = (self.prev_ref_frame_num + 1) % max;
        let mut guard = 0;
        while fnum != frame_num && guard < 2 * sps.max_num_ref_frames.max(1) as usize + 1 {
            let poc = last.entry_poc();
            let f = RefFrame {
                pic: last.pic.clone(),
                id: next_id(),
                frame_num: fnum,
                poc: [poc, poc],
                short: 3,
                long: 0,
                long_idx: 0,
            };
            self.sliding_window(sps, fnum, u32::MAX);
            self.refs.push(f);
            self.prev_ref_frame_num = fnum;
            fnum = (fnum + 1) % max;
            guard += 1;
        }
        // A long gap only needs the last frames: the rest would slide out anyway.
        self.prev_ref_frame_num = (frame_num + max - 1) % max;
        self.poc.prev_frame_num = self.prev_ref_frame_num;
    }

    /// 8.2.5.3, keeping the entry `keep` (the current frame's first field).
    fn sliding_window(&mut self, sps: &Sps, cur: u32, keep: u32) {
        let max_refs = sps.max_num_ref_frames.max(1) as usize;
        let max = 1u32 << sps.log2_max_frame_num;
        while self.refs.iter().filter(|f| f.id != keep).count() >= max_refs {
            let victim = self
                .refs
                .iter()
                .enumerate()
                .filter(|(_, f)| f.short != 0 && f.id != keep)
                .min_by_key(|(_, f)| wrap(f, cur, max))
                .map(|(i, _)| i);
            match victim {
                Some(i) => {
                    self.refs.remove(i);
                }
                // Only long-term frames: drop the oldest of them (a damaged stream).
                None => match self.refs.iter().position(|f| f.id != keep) {
                    Some(i) => {
                        self.refs.remove(i);
                    }
                    None => break,
                },
            }
        }
    }

    /// Frees LongTermFrameIdx `idx` for picture `id` (MMCO 3 and 6): entries holding it are
    /// unmarked as long-term, except the other field of `id` itself.
    fn free_long_idx(&mut self, idx: u32, id: u32) {
        for f in &mut self.refs {
            if f.long != 0 && f.long_idx == idx && f.id != id {
                f.long = 0;
            }
        }
    }

    /// Reference marking of the decoded picture (8.2.5).
    pub fn mark(&mut self, sps: &Sps, h: &SliceHeader, mut cur: CurPic) {
        if h.nal_ref_idc == 0 {
            return;
        }
        let max = 1u32 << sps.log2_max_frame_num;
        let fnum = h.frame_num;
        let bits = match cur.parity {
            None => 3u8,
            Some(p) => 1 << p,
        };
        let mut long_idx = None;
        if h.idr {
            let id = cur.id;
            self.refs.retain(|f| cur.second && f.id == id);
            if h.long_term_reference {
                long_idx = Some(0);
                self.max_long_idx = Some(0);
            } else {
                self.max_long_idx = None;
            }
        } else if let Some(ops) = &h.mmcos {
            let (cur_pic_num, par) = match cur.parity {
                None => (fnum as i64, None),
                Some(p) => (2 * fnum as i64 + 1, Some(p)),
            };
            for op in ops {
                match *op {
                    Mmco::ShortToUnused(d) => {
                        let pn = cur_pic_num - (d as i64 + 1);
                        if let Some((i, b)) = self.short_by_pic_num(pn, par, fnum, max) {
                            self.refs[i].short &= !b;
                        }
                    }
                    Mmco::LongToUnused(n) => {
                        if let Some((i, b)) = self.long_by_pic_num(n as i64, par) {
                            self.refs[i].long &= !b;
                        }
                    }
                    Mmco::ShortToLong(d, idx) => {
                        let pn = cur_pic_num - (d as i64 + 1);
                        if let Some((i, b)) = self.short_by_pic_num(pn, par, fnum, max) {
                            let id = self.refs[i].id;
                            // A long-term frame or pair, or a field of another frame, holding
                            // the index loses it.
                            for f in &mut self.refs {
                                if f.long != 0 && f.long_idx == idx && (f.id != id || par.is_none())
                                {
                                    f.long = 0;
                                }
                            }
                            let f = &mut self.refs[i];
                            if f.long != 0 && f.long_idx != idx {
                                f.long = 0;
                            }
                            f.short &= !b;
                            f.long |= b;
                            f.long_idx = idx;
                        }
                    }
                    Mmco::MaxLongTermIdx(plus1) => {
                        for f in &mut self.refs {
                            if f.long != 0 && (plus1 == 0 || f.long_idx >= plus1) {
                                f.long = 0;
                            }
                        }
                        self.max_long_idx = plus1.checked_sub(1);
                    }
                    Mmco::ClearAll => {
                        self.refs.clear();
                        self.max_long_idx = None;
                    }
                    Mmco::CurrentToLong(idx) => {
                        let id = if cur.parity.is_some() {
                            cur.id
                        } else {
                            u32::MAX
                        };
                        self.free_long_idx(idx, id);
                        long_idx = Some(idx);
                    }
                }
                self.refs
                    .retain(|f| f.marked() != 0 || (cur.second && f.id == cur.id));
            }
            if h.has_mmco5() {
                cur.second = false;
            }
        } else {
            let first_short =
                cur.second && self.refs.iter().any(|f| f.id == cur.id && f.short != 0);
            if !first_short {
                self.sliding_window(sps, fnum, if cur.second { cur.id } else { u32::MAX });
            }
        }
        let frame_num = if h.has_mmco5() { 0 } else { fnum };
        let pos = self.refs.iter().position(|f| f.id == cur.id);
        let i = match pos {
            Some(i) if cur.second => i,
            _ => {
                // Keep within the limit even for damaged streams.
                let max_refs = sps.max_num_ref_frames.max(1) as usize;
                if self.refs.len() >= max_refs {
                    self.sliding_window(sps, fnum, u32::MAX);
                }
                self.refs.push(RefFrame {
                    pic: cur.pic.clone(),
                    id: cur.id,
                    frame_num,
                    poc: [i32::MAX; 2],
                    short: 0,
                    long: 0,
                    long_idx: 0,
                });
                self.refs.len() - 1
            }
        };
        let f = &mut self.refs[i];
        for p in 0..2 {
            if bits >> p & 1 != 0 {
                f.poc[p] = cur.poc[p];
            }
        }
        match long_idx {
            Some(idx) => {
                if f.long != 0 && f.long_idx != idx {
                    f.long = 0;
                }
                f.long |= bits;
                f.long_idx = idx;
            }
            None => f.short |= bits,
        }
        self.prev_ref_frame_num = frame_num;
    }

    /// The short-term frame (or field, for a field picture of parity `par`) with PicNum `pn`,
    /// as the entry index and the field bits it covers.
    fn short_by_pic_num(
        &self,
        pn: i64,
        par: Option<u8>,
        cur: u32,
        max: u32,
    ) -> Option<(usize, u8)> {
        match par {
            None => self
                .refs
                .iter()
                .position(|f| f.short == 3 && wrap(f, cur, max) as i64 == pn)
                .map(|i| (i, 3)),
            Some(par) => {
                let (p, wr) = field_num(pn, par);
                self.refs
                    .iter()
                    .position(|f| f.short >> p & 1 != 0 && wrap(f, cur, max) as i64 == wr)
                    .map(|i| (i, 1 << p))
            }
        }
    }

    fn long_by_pic_num(&self, n: i64, par: Option<u8>) -> Option<(usize, u8)> {
        match par {
            None => self
                .refs
                .iter()
                .position(|f| f.long == 3 && f.long_idx as i64 == n)
                .map(|i| (i, 3)),
            Some(par) => {
                let (p, idx) = field_num(n, par);
                self.refs
                    .iter()
                    .position(|f| f.long >> p & 1 != 0 && f.long_idx as i64 == idx)
                    .map(|i| (i, 1 << p))
            }
        }
    }
}

/// A field PicNum or LongTermPicNum seen from a field of parity `par`: the field's parity
/// and its FrameNumWrap or LongTermFrameIdx (8.2.4.1).
fn field_num(n: i64, par: u8) -> (u8, i64) {
    let same = n & 1 != 0;
    (if same { par } else { par ^ 1 }, n >> 1)
}

/// Short-term entries with POC at most `cur_poc` (descending), and the rest (ascending).
fn split_by_poc<'a>(
    short: impl Iterator<Item = &'a RefFrame>,
    cur_poc: i32,
) -> (Vec<&'a RefFrame>, Vec<&'a RefFrame>) {
    let (mut before, mut after): (Vec<&RefFrame>, Vec<&RefFrame>) =
        short.partition(|f| f.entry_poc() <= cur_poc);
    before.sort_by_key(|f| std::cmp::Reverse(f.entry_poc()));
    after.sort_by_key(|f| f.entry_poc());
    (before, after)
}

/// 8.2.4.2.5: the fields of `frames` marked in `mask`, alternating parity starting with
/// `par`, then the rest of the parity left.
fn alternate(frames: &[&RefFrame], par: u8, mask: impl Fn(&RefFrame) -> u8) -> Vec<RefPic> {
    let mut out = Vec::new();
    let mut next = [0usize; 2];
    let mut p = par as usize;
    let n = frames.len();
    loop {
        while next[p] < n && mask(frames[next[p]]) >> p & 1 == 0 {
            next[p] += 1;
        }
        if next[p] < n {
            out.push(frames[next[p]].field_ref(p as u8));
            next[p] += 1;
            p ^= 1;
            continue;
        }
        let q = p ^ 1;
        while next[q] < n {
            if mask(frames[next[q]]) >> q & 1 != 0 {
                out.push(frames[next[q]].field_ref(q as u8));
            }
            next[q] += 1;
        }
        return out;
    }
}
