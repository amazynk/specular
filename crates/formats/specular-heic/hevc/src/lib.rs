//! The HEVC (ITU-T H.265) decoder of Specular.
//!
//! An in-tree fork of the HEVC core of heic-rs 0.1.1 (MIT OR Apache-2.0,
//! see `NOTICE` for provenance and the list of changes). [`decode_still`]
//! decodes one intra picture — 4:0:0, 4:2:0, 4:2:2 or 4:4:4 at 8 to 16
//! bits — from the parameter sets and coded slices of an IRAP access unit.
//! [`VideoDecoder`] decodes whole sequences with inter prediction (P and B
//! slices, weighted prediction, temporal motion vector prediction, long-term
//! references) up to 16 bits, and 4:4:4 with separate colour planes (one
//! monochrome decoder per colour_plane_id, joined; each plane uses the luma
//! scaling lists). Range-extension tools it does not implement are refused
//! with [`Error::Unsupported`], naming the tool. Not supported: the screen
//! content coding (SCC) and multi-layer (3D, scalable, multiview) extensions;
//! their base layer decodes on its own.
//!
//! # Contract
//!
//! `parameter_sets` are the VPS, SPS and PPS NAL units (from the `hvcC`
//! record), each without a length prefix or start code. `slices` are the
//! remaining NAL units of the item; non-VCL units and enhancement-layer
//! units (`nuh_layer_id > 0`) are ignored. [`hvcc`] splits the HEIF
//! framing.
//!
//! # Damage and cancellation
//!
//! A slice that fails to parse does not fail the picture: its remaining
//! CTUs are concealed from their neighbours and [`Decoded::damaged`]
//! counts them. Only a picture with no decodable CTU is an error.
//! [`Options::cancel`] is polled once per CTU row.

#![forbid(unsafe_code)]

extern crate alloc;

mod bits;
mod cabac;
mod decode;
mod error;
mod filter;
mod frame;
pub mod hvcc;
mod intra;
mod nal;
mod picture;
mod ps;
mod scan;
mod slice;
mod transform;

#[cfg(test)]
#[doc(hidden)]
pub mod synth;

#[cfg(test)]
mod golden;

#[cfg(test)]
mod hostile;
#[cfg(test)]
mod split_tests;

pub use decode::seq::Snapshot;
pub use error::{Error, Result};
pub use frame::{ChromaFormat, Frame, Info};
pub use ps::vui::Signal;

#[cfg(test)]
use nal::split_annexb;

use core::sync::atomic::AtomicBool;

/// How to decode.
#[derive(Clone, Copy, Debug, Default)]
pub struct Options<'a> {
    /// Worker threads to use; 0 and 1 both mean the calling thread only.
    /// More than one only helps pictures coded with wavefronts or several
    /// tiles, whose CTU rows or tiles are decoded on different threads.
    pub threads: usize,
    /// Polled at least once per CTU row; when it reads `true` the decode
    /// stops with [`Error::Cancelled`].
    pub cancel: Option<&'a AtomicBool>,
}

/// A decoded picture and how complete it is.
#[derive(Debug)]
pub struct Decoded {
    /// The picture, cropped to the conformance window.
    pub frame: Frame,
    /// CTUs that could not be decoded and were concealed from their
    /// neighbours; zero for an intact stream.
    pub damaged: usize,
    /// CTUs in the picture.
    pub total: usize,
    /// The VUI video signal type (range and colour description), when
    /// the SPS signals one.
    pub signal: Option<Signal>,
    /// The tag the access unit was given ([`VideoDecoder::decode_tagged`]);
    /// zero for stills and untagged access units.
    pub tag: i64,
}

/// Decode a single still picture.
///
/// See the module documentation for the shape of the inputs. A picture with
/// damaged CTUs is returned concealed; use [`decode_still_with`] to learn
/// how much was concealed.
pub fn decode_still(parameter_sets: &[&[u8]], slices: &[&[u8]]) -> Result<Frame> {
    decode_still_with(parameter_sets, slices, &Options::default()).map(|d| d.frame)
}

/// Decode a single still picture with the given [`Options`].
pub fn decode_still_with(
    parameter_sets: &[&[u8]],
    slices: &[&[u8]],
    options: &Options<'_>,
) -> Result<Decoded> {
    let sets = decode::ParameterSets::parse(parameter_sets)?;
    if let Some(lists) = decode::seq::split_planes(&sets, slices) {
        // Separate colour planes: each plane is a monochrome picture of its own.
        let mut planes = alloc::vec::Vec::with_capacity(3);
        for l in &lists {
            let out = decode::decode(&sets, l, options.threads, options.cancel)?;
            planes.push(Decoded {
                frame: out.frame,
                damaged: out.damaged,
                total: out.total,
                signal: out.signal,
                tag: 0,
            });
        }
        let [y, cb, cr]: [Decoded; 3] = planes.try_into().map_err(|_| Error::NoSlices)?;
        return Ok(join_planes(y, cb, cr));
    }
    let out = decode::decode(&sets, slices, options.threads, options.cancel)?;
    Ok(Decoded {
        frame: out.frame,
        damaged: out.damaged,
        total: out.total,
        signal: out.signal,
        tag: 0,
    })
}

/// Decodes a coded video sequence, one access unit at a time, and hands the
/// pictures back in output order.
///
/// Feed access units in decoding order with [`VideoDecoder::decode`], take
/// what is ready with [`VideoDecoder::next_output`], and call
/// [`VideoDecoder::flush`] at the end of the stream for the pictures still
/// held for reordering. Memory is bounded by the decoded picture buffer the
/// stream declares, capped at 17 pictures.
pub struct VideoDecoder {
    seq: decode::seq::Seq,
    /// Separate colour planes: a monochrome decoder per colour_plane_id, whose pictures are
    /// joined into 4:4:4 ones (each plane is coded as a monochrome picture of its own).
    planes: Option<alloc::boxed::Box<[decode::seq::Seq; 3]>>,
    /// Joined pictures from before the stream left separate colour planes.
    joined: alloc::collections::VecDeque<Decoded>,
}

impl VideoDecoder {
    /// A decoder for the sequence described by `parameter_sets` (the VPS,
    /// SPS and PPS NAL units of the `hvcC` record; more may arrive in band).
    ///
    /// `threads` is how many pictures decode at once, each on a worker
    /// thread, a few CTU rows behind the pictures it predicts from: 0 for
    /// one per core (at most 16), 1 to decode on the calling thread inside
    /// [`VideoDecoder::decode`]. Workers left idle help decode a picture
    /// with wavefront rows or tiles from the inside. The pictures are the
    /// same either way.
    pub fn new(parameter_sets: &[&[u8]], threads: usize) -> Result<VideoDecoder> {
        let sets = decode::ParameterSets::parse(parameter_sets)?;
        Ok(VideoDecoder {
            seq: decode::seq::Seq::new(sets, threads),
            planes: None,
            joined: alloc::collections::VecDeque::new(),
        })
    }

    /// Decodes one access unit: its NAL units, each without a start code or
    /// length prefix. An error means the access unit produced no picture;
    /// [`Error::Cancelled`] and [`Error::Unsupported`] are worth stopping
    /// for, anything else only cost that picture (later pictures that
    /// reference it are predicted from a stand-in).
    ///
    /// With worker threads the picture may still be decoding when this
    /// returns; `cancel` is polled while this call waits for a worker, and
    /// when it reads `true` the pictures in flight are stopped too (they
    /// are then reported by [`VideoDecoder::next_dropped`], not output).
    /// Decoding may go on after a cancel: pictures predicted, directly or
    /// not, from a cancelled one are reported dropped too, never output,
    /// until one depends on nothing cancelled (at the latest the next
    /// random access point), whatever the thread count.
    pub fn decode(&mut self, nal_units: &[&[u8]], cancel: Option<&AtomicBool>) -> Result<()> {
        self.decode_tagged(nal_units, 0, cancel)
    }

    /// [`VideoDecoder::decode`] with a tag (a sample's composition time, say)
    /// that comes back on the access unit's picture in [`Decoded::tag`], or
    /// from [`VideoDecoder::next_dropped`] when it will never be output: a
    /// RASL picture skipped after a random access point, `pic_output_flag`
    /// 0, pictures discarded by `no_output_of_prior_pics_flag`, and access
    /// units that produced no picture (the error cases).
    pub fn decode_tagged(
        &mut self,
        nal_units: &[&[u8]],
        tag: i64,
        cancel: Option<&AtomicBool>,
    ) -> Result<()> {
        if let Some(lists) = self.seq.split_planes(nal_units) {
            let seq = &self.seq;
            let planes = self.planes.get_or_insert_with(|| {
                alloc::boxed::Box::new(core::array::from_fn(|_| seq.plane_decoder()))
            });
            let mut res = Ok(());
            for (p, l) in planes.iter_mut().zip(&lists) {
                let r = p.decode(l, tag, cancel);
                if res.is_ok() {
                    res = r;
                }
            }
            return res;
        }
        if let Some(mut planes) = self.planes.take() {
            // Back to a stream without separate colour planes: the planes' pictures first.
            planes.iter_mut().for_each(decode::seq::Seq::flush);
            while let Some(d) = join(&mut planes) {
                self.joined.push_back(d);
            }
        }
        self.seq.decode(nal_units, tag, cancel)
    }

    /// The tag of an access unit known never to be output, oldest first.
    pub fn next_dropped(&mut self) -> Option<i64> {
        match &mut self.planes {
            Some(p) => {
                // The planes see the same access units and drop the same ones.
                let [a, b, c] = p.each_mut().map(decode::seq::Seq::next_dropped);
                a.or(b).or(c)
            }
            None => self.seq.next_dropped(),
        }
    }

    /// The next picture in output order, when one is ready: due for output
    /// and decoded.
    pub fn next_output(&mut self) -> Option<Decoded> {
        if let Some(d) = self.joined.pop_front() {
            return Some(d);
        }
        match &mut self.planes {
            Some(p) => join(p),
            None => self.seq.next_output(),
        }
    }

    /// The tag of the picture [`VideoDecoder::next_output`] would return
    /// now, if one is ready.
    pub fn ready_tag(&self) -> Option<i64> {
        if let Some(d) = self.joined.front() {
            return Some(d.tag);
        }
        match &self.planes {
            Some(p) => {
                let [a, b, c] = p.each_ref().map(decode::seq::Seq::ready_tag);
                b.and(c).and(a)
            }
            None => self.seq.ready_tag(),
        }
    }

    /// Takes the next ready picture in output order without making its
    /// frame (cheaper than [`VideoDecoder::next_output`] for a picture the
    /// caller would discard, e.g. one before a seek target); returns its tag.
    pub fn skip_output(&mut self) -> Option<i64> {
        if let Some(d) = self.joined.pop_front() {
            return Some(d.tag);
        }
        if self.planes.is_some() {
            self.ready_tag()?;
        }
        match &mut self.planes {
            Some(p) => {
                let [a, _, _] = p.each_mut().map(decode::seq::Seq::skip_output);
                a
            }
            None => self.seq.skip_output(),
        }
    }

    /// The decoding state between two access units, to come back to with
    /// [`VideoDecoder::restore`]: a seek can resume from the snapshot taken
    /// nearest before its target instead of from the random access point.
    /// Cheap (the pictures are shared, not copied), but it keeps the
    /// pictures it holds alive; [`Snapshot::pictures`] sizes them.
    pub fn snapshot(&self) -> Snapshot {
        let mut s = self.seq.snapshot();
        s.planes = self
            .planes
            .as_ref()
            .map(|p| alloc::sync::Arc::new(p.each_ref().map(decode::seq::Seq::snapshot)));
        s
    }

    /// Stops the pictures in flight and continues as the decoder was when
    /// `snapshot` was taken: decoding the access units that followed then
    /// gives the same pictures. Returns false when a picture the snapshot
    /// holds was cancelled while decoding (or is tainted by one); the
    /// decoder is then empty and must restart at a random access point.
    /// The worker threads are kept, so this is also a cheap reset to a
    /// snapshot taken before the first access unit.
    pub fn restore(&mut self, snapshot: &Snapshot) -> bool {
        self.joined.clear();
        let mut ok = self.seq.restore(snapshot);
        match &snapshot.planes {
            Some(snaps) => {
                let seq = &self.seq;
                let planes = self.planes.get_or_insert_with(|| {
                    alloc::boxed::Box::new(core::array::from_fn(|_| seq.plane_decoder()))
                });
                for (p, s) in planes.iter_mut().zip(snaps.iter()) {
                    ok &= p.restore(s);
                }
            }
            None => self.planes = None,
        }
        ok
    }

    /// Releases every picture still held for reordering to
    /// [`VideoDecoder::next_output`] and waits for the pictures still
    /// decoding; call it at the end of the stream.
    pub fn flush(&mut self) {
        self.seq.flush();
        if let Some(p) = &mut self.planes {
            p.iter_mut().for_each(decode::seq::Seq::flush);
        }
    }
}

/// The next picture of the three colour plane decoders, joined into a 4:4:4 picture, once
/// each has its own.
fn join(p: &mut [decode::seq::Seq; 3]) -> Option<Decoded> {
    if p.iter().any(|s| s.ready_tag().is_none()) {
        return None;
    }
    let [y, cb, cr] = p.each_mut().map(decode::seq::Seq::next_output);
    Some(join_planes(y?, cb?, cr?))
}

/// Three monochrome pictures, one per colour plane, as one 4:4:4 picture.
fn join_planes(mut y: Decoded, cb: Decoded, cr: Decoded) -> Decoded {
    let f = &mut y.frame;
    f.chroma = ChromaFormat::Yuv444;
    f.bit_depth_chroma = f.bit_depth;
    f.c_stride = f.y_stride;
    f.cb = cb.frame.y;
    f.cr = cr.frame.y;
    y.damaged += cb.damaged + cr.damaged;
    y.total += cb.total + cr.total;
    y
}

/// Read the geometry out of a sequence parameter set without decoding.
pub fn probe(parameter_sets: &[&[u8]]) -> Result<Info> {
    let sets = decode::ParameterSets::parse(parameter_sets)?;
    let sps = sets.first_sps()?;
    let [l, r, t, b] = sps.crop;
    Ok(Info {
        width: sps.width.saturating_sub(l + r).max(1) as u32,
        height: sps.height.saturating_sub(t + b).max(1) as u32,
        bit_depth: sps.bit_depth_y,
        chroma: ChromaFormat::from_idc(if sps.separate_colour_plane {
            3
        } else {
            sps.chroma_array_type
        })
        .unwrap_or(ChromaFormat::Monochrome),
        signal: sps.signal,
        field_seq: sps.field_seq,
        chroma_loc: sps.chroma_loc,
    })
}

/// Decode one still picture from an Annex-B byte stream.
///
/// Start codes are located, the NAL units split out and sorted into parameter
/// sets and slices, and [`decode_still`] is then applied. HEIF stores NAL
/// units with length prefixes rather than start codes, so this is used only by
/// the decoder's own tests, which speak the codec's native framing.
#[cfg(test)]
pub(crate) fn decode_annexb(stream: &[u8]) -> Result<Frame> {
    use alloc::vec::Vec;
    let units = split_annexb(stream);
    let mut sets: Vec<&[u8]> = Vec::new();
    let mut slices: Vec<&[u8]> = Vec::new();
    for u in units {
        let Ok(h) = nal::NalHeader::parse(u) else {
            continue;
        };
        if h.is_vcl() {
            slices.push(u);
        } else if matches!(h.nal_unit_type, 32..=34) {
            sets.push(u);
        }
    }
    decode_still(&sets, &slices)
}
