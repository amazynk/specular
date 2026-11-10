//! Which MP4 samples carry the second field of a field pair, from the slice headers alone
//! (no decoding): a container that stores one field per sample has two composition times
//! per frame, and a player needs to know which of them a frame will come out under.

use std::sync::Arc;

use crate::bits::unescape;
use crate::decoder::{first_slice_of_new_picture, parse_avcc};
use crate::params::{Pps, Sps, parse_pps, parse_sps};
use crate::slice::{SliceHeader, peek_pps_id, peek_slice_header};
use crate::{Result, malformed};

/// How one sample's pictures make up frames.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SampleFields {
    /// The sample starts at least one frame of its own: a frame, or a first field (with or
    /// without its second field in the same sample). Its composition time labels a decoded
    /// picture.
    pub starts_frame: bool,
    /// The sample opens with the second field of a first field left open by an earlier
    /// sample: the frame it completes belongs to that sample.
    pub completes_open: bool,
    /// The sample ends on a first field still waiting for its second field.
    pub leaves_open: bool,
    /// The sample's first picture is an IDR picture.
    pub idr: bool,
}

/// A first field waiting for its second field (the decoder's pairing rule, 7.4.1.2.4 and
/// 8.2.1).
#[derive(Clone, Copy)]
struct Open {
    parity: bool,
    frame_num: u32,
    is_ref: bool,
    idr: bool,
    /// Size in macroblocks and sample format (chroma_format_idc 2 or not, bit depths).
    size: (u32, u32, bool, u32, u32),
}

/// Classifies a track's samples in decoding order with [`FieldScan::sample`]. Pairs fields
/// exactly as [`crate::Decoder`] does, so a field pair comes out of the decoder tagged with
/// the sample that starts it, and a field that never gets its pair as a frame of its own.
/// A sequence parameter set's VUI video signal type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VuiSignal {
    /// `colour_primaries`, `transfer_characteristics`, `matrix_coefficients`, when given.
    pub colour: Option<(u8, u8, u8)>,
    pub full_range: bool,
}

pub struct FieldScan {
    sps: Vec<Option<Arc<Sps>>>,
    pps: Vec<Option<Arc<Pps>>>,
    length_size: usize,
    open: Option<Open>,
    buf: Vec<u8>,
}

/// Slice header bytes unescaped for a peek: enough for every field up to redundant_pic_cnt.
const PEEK: usize = 64;

impl FieldScan {
    /// A scan for an MP4 track with this `avcC` record.
    pub fn with_avcc(avcc: &[u8]) -> Result<FieldScan> {
        let (length_size, nals) = parse_avcc(avcc)?;
        let mut s = FieldScan {
            sps: vec![None; 32],
            pps: vec![None; 256],
            length_size,
            open: None,
            buf: Vec::new(),
        };
        for n in nals {
            s.param_set(n);
        }
        Ok(s)
    }

    /// Whether a sequence parameter set seen so far allows field or MBAFF coding
    /// (`frame_mbs_only_flag` 0): the stream is interlaced.
    pub fn interlaced(&self) -> bool {
        self.sps.iter().flatten().any(|s| !s.frame_mbs_only)
    }

    /// The first sequence parameter set's VUI video signal type, when it has one.
    pub fn signal(&self) -> Option<VuiSignal> {
        let s = self.sps.iter().flatten().next()?;
        s.vui.signal.then_some(VuiSignal {
            colour: s.vui.colour,
            full_range: s.vui.full_range,
        })
    }

    /// The first sequence parameter set's VUI `chroma_sample_loc_type_top_field` (0..=5),
    /// when it says where subsampled chroma sits.
    pub fn chroma_loc(&self) -> Option<u8> {
        self.sps.iter().flatten().next()?.vui.chroma_loc
    }

    fn param_set(&mut self, nal: &[u8]) {
        let Some(&h) = nal.first() else { return };
        let mut b = std::mem::take(&mut self.buf);
        match h & 31 {
            7 => {
                let len = unescape(&nal[1..], &mut b);
                if let Ok(s) = parse_sps(&b, len) {
                    let id = s.id as usize;
                    self.sps[id] = Some(Arc::new(s));
                }
            }
            8 => {
                let len = unescape(&nal[1..], &mut b);
                if let Ok(p) = parse_pps(&b, len, &self.sps) {
                    let id = p.id as usize;
                    self.pps[id] = Some(Arc::new(p));
                }
            }
            _ => {}
        }
        self.buf = b;
    }

    /// The slice header of a slice NAL unit and its SPS; `None` when it can't be read (the
    /// decoder drops such a slice too).
    fn header(&mut self, nal: &[u8]) -> Option<(SliceHeader, Arc<Sps>)> {
        let ty = nal[0] & 31;
        let ref_idc = (nal[0] >> 5) & 3;
        let mut b = std::mem::take(&mut self.buf);
        let len = unescape(&nal[1..nal.len().min(1 + PEEK)], &mut b);
        let r = (|| {
            let (_, _, pps_id) = peek_pps_id(&b, len)?;
            let pps = self.pps.get(pps_id as usize)?.clone()?;
            let sps = self.sps.get(pps.sps_id as usize)?.clone()?;
            if sps.unsupported().is_some() {
                return None;
            }
            let h = match peek_slice_header(&b, len, ty, ref_idc, &sps, &pps) {
                Ok(h) => h,
                // A header longer than the peek (rare: long POC fields).
                Err(_) if nal.len() > 1 + PEEK => {
                    let len = unescape(&nal[1..], &mut b);
                    peek_slice_header(&b, len, ty, ref_idc, &sps, &pps).ok()?
                }
                Err(_) => return None,
            };
            Some((h, sps))
        })();
        self.buf = b;
        r
    }

    /// Classifies the next sample (decoding order), an access unit of length-prefixed NAL
    /// units.
    pub fn sample(&mut self, sample: &[u8]) -> Result<SampleFields> {
        let ls = self.length_size;
        let mut out = SampleFields::default();
        let mut prev: Option<(SliceHeader, Arc<Sps>)> = None;
        let mut first = true;
        let mut p = 0;
        while p + ls <= sample.len() {
            let n = sample[p..p + ls]
                .iter()
                .fold(0usize, |a, &b| (a << 8) | b as usize);
            p += ls;
            let nal = sample
                .get(p..p + n)
                .ok_or_else(|| malformed("NAL unit length past the end of the sample"))?;
            p += n;
            let Some(&h) = nal.first() else { continue };
            match h & 31 {
                7 | 8 => self.param_set(nal),
                1 | 2 | 5 => {
                    let Some((hdr, sps)) = self.header(nal) else {
                        continue;
                    };
                    if hdr.redundant_pic_cnt > 0 {
                        continue;
                    }
                    let new = prev.as_ref().is_none_or(|(ph, ps)| {
                        !Arc::ptr_eq(ps, &sps) || first_slice_of_new_picture(ph, &hdr, &sps, true)
                    });
                    if new {
                        let second = self.picture(&hdr, &sps);
                        if first {
                            out.completes_open = second;
                            out.idr = hdr.idr;
                            first = false;
                        }
                        out.starts_frame |= !second;
                    }
                    prev = Some((hdr, sps));
                }
                _ => {}
            }
        }
        out.leaves_open = self.open.is_some();
        Ok(out)
    }

    /// A new picture: whether it is the second field of the open first field. Updates the
    /// open field.
    fn picture(&mut self, h: &SliceHeader, sps: &Sps) -> bool {
        let size = (
            sps.width_mbs,
            sps.height_mbs,
            sps.chroma_format_idc == 2,
            sps.bit_depth_luma,
            sps.bit_depth_chroma,
        );
        let second = match (self.open, h.field_pic) {
            (Some(o), true) => {
                h.bottom_field != o.parity
                    && h.frame_num == o.frame_num
                    && (h.nal_ref_idc != 0) == o.is_ref
                    && (!h.idr || o.idr)
                    && size == o.size
            }
            _ => false,
        };
        self.open = (h.field_pic && !second).then_some(Open {
            parity: h.bottom_field,
            frame_num: h.frame_num,
            is_ref: h.nal_ref_idc != 0,
            idr: h.idr,
            size,
        });
        second
    }
}
