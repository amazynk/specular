//! HEVC field coding: a sequence whose VUI sets `field_seq_flag` codes every picture as one
//! field. The decoder hands out fields; two consecutive ones in output order make a frame,
//! woven here (the top field on the even lines). Which field is the top one comes from the
//! picture timing SEI's `pic_struct` when it is there, else the first of a pair is taken
//! for the top field. A field left without its pair at the end is shown line-doubled.

use std::collections::HashMap;

use specular_heic::hevc;
use specular_hwdec::video::VideoFrame;

use specular_h264::SampleFields;

use crate::mp4::VideoTrack;

/// Whether an `hvcC` record's sequence codes fields (`field_seq_flag`).
pub fn field_seq(config: &[u8]) -> bool {
    hevc::hvcc::parse(config)
        .ok()
        .and_then(|c| hevc::probe(&c.nal_units).ok())
        .is_some_and(|i| i.field_seq)
}

/// Per sample (decoding order): whether it starts a frame. Fields pair up in presentation
/// order, so every other composition time starts one.
pub fn sample_fields(track: &VideoTrack) -> Vec<SampleFields> {
    let mut times: Vec<i64> = track.samples.iter().map(|s| s.cts).collect();
    times.sort_unstable();
    times.dedup();
    let rank: HashMap<i64, usize> = times.iter().enumerate().map(|(i, &c)| (c, i)).collect();
    track
        .samples
        .iter()
        .map(|s| SampleFields {
            starts_frame: rank.get(&s.cts).is_none_or(|r| r % 2 == 0),
            completes_open: false,
            leaves_open: false,
            idr: false,
        })
        .collect()
}

/// The parity a picture timing SEI among `nals` gives its field: `Some(true)` for a top
/// field, `Some(false)` for a bottom one (`pic_struct` 1, 9, 11 and 2, 10, 12).
pub fn sei_top_field(nals: &[&[u8]]) -> Option<bool> {
    for nal in nals {
        // Prefix SEI (type 39), nuh_layer_id 0.
        if nal.len() < 3 || (nal[0] >> 1) & 0x3f != 39 {
            continue;
        }
        let rbsp = unescape(&nal[2..]);
        let mut p = 0usize;
        while p < rbsp.len() && rbsp[p] != 0x80 {
            let mut ty = 0usize;
            while *rbsp.get(p)? == 0xff {
                ty += 255;
                p += 1;
            }
            ty += usize::from(rbsp[p]);
            p += 1;
            let mut size = 0usize;
            while *rbsp.get(p)? == 0xff {
                size += 255;
                p += 1;
            }
            size += usize::from(rbsp[p]);
            p += 1;
            if ty == 1 {
                // pic_timing: pic_struct u(4) first (frame_field_info_present_flag is 1
                // whenever field_seq_flag is).
                return match rbsp.get(p)? >> 4 {
                    1 | 9 | 11 => Some(true),
                    2 | 10 | 12 => Some(false),
                    _ => None,
                };
            }
            p += size;
        }
    }
    None
}

fn unescape(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(b.len());
    let mut zeros = 0;
    for &x in b {
        if zeros >= 2 && x == 3 {
            zeros = 0;
            continue;
        }
        zeros = if x == 0 { zeros + 1 } else { 0 };
        out.push(x);
    }
    out
}

/// Pairs fields coming out of the decoder (in output order) into frames.
///
/// With the track's composition times ([`Weaver::for_track`]) a pair is a frame-starting
/// field (an even rank, as [`sample_fields`] counts) and the field right after it; a field
/// whose partner never comes (dropped by the decoder, or not decodable) is shown alone,
/// line-doubled, under its frame's time, so one lost field never shifts the pairs after
/// it. Two fields an SEI gives the same parity are never paired either.
#[derive(Default)]
pub struct Weaver {
    /// The first field of a pair: its tag, picture and parity (when an SEI gave one).
    pending: Option<(i64, VideoFrame, Option<bool>)>,
    /// Parity of each sample's field by composition time, from its SEI.
    parity: HashMap<i64, bool>,
    /// The track's composition times, sorted (even ranks start frames); empty: pair
    /// fields as they come.
    times: Vec<i64>,
    /// A frame's first field the decoder dropped: its rank.
    lost_first: Option<usize>,
}

impl Weaver {
    /// A weaver that pairs by the track's composition times.
    pub fn for_track(track: &VideoTrack) -> Weaver {
        let mut times: Vec<i64> = track.samples.iter().map(|s| s.cts).collect();
        times.sort_unstable();
        times.dedup();
        Weaver {
            times,
            ..Weaver::default()
        }
    }

    /// Forgets the field waiting (after a seek); the times stay.
    pub fn reset(&mut self) {
        self.pending = None;
        self.parity.clear();
        self.lost_first = None;
    }

    fn rank(&self, tag: i64) -> Option<usize> {
        self.times.binary_search(&tag).ok()
    }

    /// Notes the parity the SEI of the sample at `cts` gives its field.
    pub fn note(&mut self, cts: i64, nals: &[&[u8]]) {
        if let Some(top) = sei_top_field(nals) {
            self.parity.insert(cts, top);
        }
    }

    /// Takes the next field out of the decoder; a completed frame goes to `out` under its
    /// first field's tag.
    pub fn push(&mut self, tag: i64, f: VideoFrame, out: &mut Vec<(i64, VideoFrame)>) {
        let top = self.parity.remove(&tag);
        let rank = self.rank(tag);
        let Some((t0, f0, top0)) = self.pending.take() else {
            match rank {
                // a second field whose first was lost: alone, under its frame's time
                Some(i) if i % 2 == 1 => {
                    self.lost_first = None;
                    out.push((self.times[i - 1], line_double(&f)));
                }
                _ => self.pending = Some((tag, f, top)),
            }
            return;
        };
        let partner = match (rank, self.rank(t0)) {
            (Some(i), Some(i0)) => i == i0 + 1 && i % 2 == 1,
            _ => true,
        };
        let same_size = f0.width == f.width && f0.height == f.height && f0.chroma == f.chroma;
        let same_parity = matches!((top0, top), (Some(a), Some(b)) if a == b);
        if partner && same_size && !same_parity {
            // The first field is the top one unless an SEI says otherwise.
            let first_top = top0.or(top.map(|t| !t)).unwrap_or(true);
            out.push((t0, weave(&f0, &f, first_top)));
            return;
        }
        // Not a pair: the first stands alone; the new field starts or ends a frame.
        out.push((t0, line_double(&f0)));
        self.pending = None;
        self.push_back(tag, f, top, out);
    }

    /// [`Weaver::push`] of a field whose parity was taken already.
    fn push_back(
        &mut self,
        tag: i64,
        f: VideoFrame,
        top: Option<bool>,
        out: &mut Vec<(i64, VideoFrame)>,
    ) {
        if let Some(t) = top {
            self.parity.insert(tag, t);
        }
        self.push(tag, f, out);
    }

    /// The decoder dropped the picture at `tag`: a first field waiting for it comes out
    /// alone. Returns the time of a frame now lost whole (both fields dropped; without
    /// the track's times, `tag` itself), which the caller counts as gone.
    pub fn dropped(&mut self, tag: i64, out: &mut Vec<(i64, VideoFrame)>) -> Option<i64> {
        self.parity.remove(&tag);
        let Some(i) = self.rank(tag) else {
            return Some(tag);
        };
        if i % 2 == 0 {
            // a first field: its second may still come and stand for the frame
            if let Some((t0, f0, _)) = self.pending.take() {
                out.push((t0, line_double(&f0)));
            }
            self.lost_first = Some(i);
            return None;
        }
        if let Some((t0, f0, _)) = self
            .pending
            .take_if(|p| self.times.get(i - 1) == Some(&p.0))
        {
            out.push((t0, line_double(&f0)));
            return None;
        }
        (self.lost_first.take() == Some(i - 1)).then(|| self.times[i - 1])
    }

    /// The end of the stream: a field without its pair comes out line-doubled.
    pub fn finish(&mut self, out: &mut Vec<(i64, VideoFrame)>) {
        if let Some((t, f, _)) = self.pending.take() {
            out.push((t, line_double(&f)));
        }
    }
}

/// Bytes per sample and the chroma plane size of a picture.
fn geometry(f: &VideoFrame) -> (usize, [(usize, usize); 3]) {
    let bps = if f.bit_depth > 8 { 2 } else { 1 };
    let (w, h) = (f.width as usize, f.height as usize);
    let c = f.chroma.plane_size(f.width, f.height);
    let c = (c.0 as usize, c.1 as usize);
    (bps, [(w, h), c, c])
}

/// Two fields as one frame of twice the height: `first`'s lines on the even rows when
/// `first_top`, else on the odd ones.
pub fn weave(first: &VideoFrame, second: &VideoFrame, first_top: bool) -> VideoFrame {
    let (bps, dims) = geometry(first);
    let (top, bottom) = if first_top {
        (first, second)
    } else {
        (second, first)
    };
    let planes = std::array::from_fn(|i| {
        let (w, h) = dims[i];
        let row = w * bps;
        let mut v = vec![0u8; row * h * 2];
        for (y, out) in v.chunks_exact_mut(row).enumerate() {
            let src = if y % 2 == 0 { top } else { bottom };
            let start = (y / 2) * row;
            if let Some(s) = src.planes[i].get(start..start + row) {
                out.copy_from_slice(s);
            }
        }
        v
    });
    VideoFrame {
        width: first.width,
        height: first.height * 2,
        bit_depth: first.bit_depth,
        chroma: first.chroma,
        planes,
        signal: first.signal,
    }
}

/// A lone field as a frame: every line twice.
pub fn line_double(f: &VideoFrame) -> VideoFrame {
    weave(f, f, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(w: u32, h: u32, v: u8) -> VideoFrame {
        let n = (w * h) as usize;
        let c = (w.div_ceil(2) * h.div_ceil(2)) as usize;
        VideoFrame {
            width: w,
            height: h,
            bit_depth: 8,
            chroma: specular_hwdec::video::Chroma::Yuv420,
            planes: [vec![v; n], vec![v; c], vec![v; c]],
            signal: None,
        }
    }

    #[test]
    fn fields_weave_top_first_unless_the_sei_says_bottom() {
        let mut w = Weaver::default();
        let mut out = Vec::new();
        w.push(0, field(4, 2, 10), &mut out);
        w.push(1, field(4, 2, 20), &mut out);
        assert_eq!(out.len(), 1);
        let f = &out[0].1;
        assert_eq!((out[0].0, f.width, f.height), (0, 4, 4));
        assert_eq!(&f.planes[0][..8], &[10, 10, 10, 10, 20, 20, 20, 20]);
        // A bottom field first.
        w.parity.insert(2, false);
        w.push(2, field(4, 2, 30), &mut out);
        w.push(3, field(4, 2, 40), &mut out);
        assert_eq!(&out[1].1.planes[0][..8], &[40, 40, 40, 40, 30, 30, 30, 30]);
        // A lone field at the end is line-doubled.
        w.push(4, field(4, 2, 50), &mut out);
        w.finish(&mut out);
        assert_eq!(out[2].1.height, 4);
        assert!(out[2].1.planes[0].iter().all(|&v| v == 50));
    }

    /// Fields at times 0..12 (pairs 0+1, 2+3, …). A missing field never shifts the pairs
    /// after it: its partner is shown alone, line-doubled, under its frame's time; a frame
    /// whose two fields are both dropped is reported gone once.
    #[test]
    fn a_missing_field_leaves_its_partner_alone() {
        let mut w = Weaver {
            times: (0..12).collect(),
            ..Weaver::default()
        };
        let mut out = Vec::new();
        let mut gone = Vec::new();
        // Frame 0 whole.
        w.push(0, field(4, 2, 10), &mut out);
        w.push(1, field(4, 2, 11), &mut out);
        // Frame 1: the second field (3) never decodes; frame 2 follows.
        w.push(2, field(4, 2, 20), &mut out);
        w.push(4, field(4, 2, 30), &mut out);
        w.push(5, field(4, 2, 31), &mut out);
        // Frame 3: the first field (6) is dropped by the decoder; its second comes.
        gone.extend(w.dropped(6, &mut out));
        w.push(7, field(4, 2, 41), &mut out);
        // Frame 4: the second field (9) is dropped while the first waits.
        w.push(8, field(4, 2, 50), &mut out);
        gone.extend(w.dropped(9, &mut out));
        // Frame 5: both dropped.
        gone.extend(w.dropped(10, &mut out));
        gone.extend(w.dropped(11, &mut out));
        w.finish(&mut out);
        let tags: Vec<i64> = out.iter().map(|o| o.0).collect();
        assert_eq!(tags, [0, 2, 4, 6, 8]);
        assert_eq!(gone, [10]);
        let rows = |f: &VideoFrame| -> Vec<u8> { f.planes[0].iter().step_by(4).copied().collect() };
        assert_eq!(rows(&out[0].1), [10, 11, 10, 11]);
        assert_eq!(rows(&out[1].1), [20, 20, 20, 20]);
        assert_eq!(rows(&out[2].1), [30, 31, 30, 31]);
        assert_eq!(rows(&out[3].1), [41, 41, 41, 41]);
        assert_eq!(rows(&out[4].1), [50, 50, 50, 50]);
        assert!(out.iter().all(|o| o.1.height == 4));
        // Two fields an SEI gives the same parity are not a pair, even back to back.
        let mut w = Weaver::default();
        let mut out = Vec::new();
        w.parity.insert(0, true);
        w.parity.insert(1, true);
        w.push(0, field(4, 2, 1), &mut out);
        w.push(1, field(4, 2, 2), &mut out);
        w.finish(&mut out);
        assert_eq!(out.len(), 2);
        assert!(out[1].1.planes[0].iter().all(|&v| v == 2));
    }

    #[test]
    fn pic_struct_comes_from_the_picture_timing_sei() {
        // Prefix SEI NAL: header 0x4e 0x01, pic_timing (1), size 1, pic_struct 2 (bottom).
        let nal = [0x4e, 0x01, 0x01, 0x01, 0x20, 0x80];
        assert_eq!(sei_top_field(&[&nal]), Some(false));
        let nal = [0x4e, 0x01, 0x01, 0x01, 0x10, 0x80];
        assert_eq!(sei_top_field(&[&nal]), Some(true));
        // Another SEI first (user data, 2 bytes), then pic_timing.
        let nal = [0x4e, 0x01, 0x05, 0x02, 0xaa, 0xbb, 0x01, 0x01, 0x90, 0x80];
        assert_eq!(sei_top_field(&[&nal]), Some(true));
        assert_eq!(sei_top_field(&[&[0x40, 0x01, 0x0c][..]]), None);
    }
}
