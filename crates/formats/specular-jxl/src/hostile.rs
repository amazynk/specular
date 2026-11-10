//! Hostile-input regressions: tiny hand-built codestreams whose headers declare sizes or counts
//! far beyond what the file holds. None may panic (debug builds check overflow) or allocate in
//! proportion to the declared numbers.

use crate::bitstream::Bitstream;
use crate::common::Bundle;
use crate::frame::FrameHeader;
use crate::frame::data::{Patches, Splines};
use crate::grid::AllocTracker;
use crate::image::ImageHeader;

/// LSB-first bit writer, as the JPEG XL bitstream reads.
#[derive(Default)]
struct Bits {
    out: Vec<u8>,
    n: usize,
}

impl Bits {
    fn put(&mut self, v: u64, bits: usize) -> &mut Self {
        for i in 0..bits {
            if self.n.is_multiple_of(8) {
                self.out.push(0);
            }
            if (v >> i) & 1 != 0 {
                *self.out.last_mut().unwrap() |= 1 << (self.n % 8);
            }
            self.n += 1;
        }
        self
    }

    fn pad(&mut self) -> &mut Self {
        let n = self.n.next_multiple_of(8) - self.n;
        self.put(0, n)
    }

    /// Signature, an 8×8 or 1×1 size, and all-default metadata.
    fn image_header(&mut self, eight: bool) -> &mut Self {
        self.put(0x0aff, 16);
        if eight {
            // div8, h_div8 = 1, ratio 0, w_div8 = 1: 8×8.
            self.put(1, 1).put(0, 5).put(0, 3).put(0, 5);
        } else {
            // height 1 + u(9) = 1, ratio 1 (square): 1×1.
            self.put(0, 1).put(0, 2).put(0, 9).put(1, 3);
        }
        self.put(1, 1).put(1, 1) // metadata all_default, default_m
    }

    /// A modular regular frame, upsampled ×8 with 1024-pixel groups, cropped to `side × side`
    /// at the origin (`side` >= 18688).
    fn big_frame_header(&mut self, side: u32) -> &mut Self {
        self.put(0, 1) // all_default
            .put(0, 2) // frame_type: regular
            .put(1, 1) // encoding: modular
            .put(0, 2) // flags: 0
            .put(3, 2) // upsampling: 8
            .put(3, 2) // group_size_shift: 3
            .put(0, 2) // num_passes: 1
            .put(1, 1) // have_crop
            .put(0, 2)
            .put(0, 8) // x0
            .put(0, 2)
            .put(0, 8) // y0
            .put(3, 2)
            .put((side - 18688) as u64, 30) // width
            .put(3, 2)
            .put((side - 18688) as u64, 30) // height
            .put(0, 2) // blending: replace (covers the canvas, so no source)
            .put(1, 1) // is_last
            .put(0, 2) // name: empty
            .put(1, 1) // restoration_filter: default
            .put(0, 2) // extensions: none
    }

    /// An entropy code (prefix, no LZ77) mapping each of `ctx_cluster.len()` contexts to a
    /// cluster whose only symbol is `cluster_token[c]`; each cluster's integer config has
    /// split 1, so token `t >= 1` decodes as `1 << (t - 1)` plus `t - 1` raw bits.
    fn entropy(&mut self, ctx_cluster: &[u8], cluster_token: &[u32]) -> &mut Self {
        self.put(0, 1); // lz77 off
        self.put(1, 1).put(2, 2); // simple clustering, 2 bits each
        for &c in ctx_cluster {
            self.put(c as u64, 2);
        }
        self.put(1, 1); // prefix code (log alphabet 15)
        for _ in cluster_token {
            self.put(0, 4); // split_exponent 0; msb and lsb read no bits
        }
        for _ in cluster_token {
            self.put(1, 1).put(5, 4).put(0, 5); // alphabet 1 + 32 + 0 = 33
        }
        for &t in cluster_token {
            self.put(1, 2).put(0, 2).put(t as u64, 6); // simple, one symbol
        }
        self
    }
}

fn headers(eight: bool, big_frame: bool) -> (ImageHeader, FrameHeader) {
    let mut w = Bits::default();
    w.image_header(eight);
    if big_frame {
        w.big_frame_header(65536);
    } else {
        w.put(1, 1); // all-default frame: the 8×8 canvas
    }
    let n = w.n;
    w.put(0, 64);
    let mut bs = Bitstream::new(&w.out);
    let image = ImageHeader::parse(&mut bs, ()).unwrap();
    let frame = FrameHeader::parse(&mut bs, &image).unwrap();
    assert_eq!(bs.num_read_bits(), n);
    (image, frame)
}

fn patches(raw: impl FnOnce(&mut Bits)) -> crate::frame::Result<Patches> {
    // Patch contexts: 0 ref count, 1 ref index, 2 width/height, 3 x0/y0, 4 first target,
    // 7 target count. Every test declares one reference (cluster 1 decodes 1).
    let (image, frame) = headers(true, false);
    let mut w = Bits::default();
    raw(&mut w);
    w.put(0, 64);
    Patches::parse(&mut Bitstream::new(&w.out), (&image, &frame, None))
}

#[track_caller]
fn assert_err(r: crate::frame::Result<()>, what: &str) {
    let e = r.expect_err(what).to_string();
    assert!(e.contains(what), "{e}");
}

#[test]
fn patch_ref_width_overflow() {
    // width = u32::MAX: its `+ 1` overflowed.
    let r = patches(|w| {
        w.entropy(&[1, 0, 2, 0, 0, 0, 0, 0, 0, 0], &[0, 1, 32]);
        w.put(u32::MAX as u64 >> 1, 31)
            .put(u32::MAX as u64 >> 1, 31);
    });
    assert_err(r.map(drop), "region out of bounds");
}

#[test]
fn patch_count_overflow() {
    // count = u32::MAX: its `+ 1` overflowed.
    let r = patches(|w| {
        w.entropy(&[1, 0, 0, 0, 0, 0, 0, 2, 0, 0], &[0, 1, 32]);
        w.put(u32::MAX as u64 >> 1, 31);
    });
    assert_err(r.map(drop), "too many patches");
}

#[test]
fn patch_target_out_of_range() {
    // First target at x = y = 2^31 (i32::MIN): blending's region arithmetic overflowed.
    let r = patches(|w| {
        w.entropy(&[1, 0, 0, 0, 2, 0, 0, 0, 0, 0], &[0, 1, 32]);
        w.put(0, 31).put(0, 31);
    });
    assert_err(r.map(drop), "target out of bounds");
}

#[test]
fn patch_valid_still_parses() {
    // One 1×1 reference at the origin, one target at the origin.
    let r = patches(|w| {
        w.entropy(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0], &[0, 1]);
    });
    let p = r.unwrap();
    assert_eq!(p.patches.len(), 1);
    assert_eq!(p.patches[0].patch_targets.len(), 1);
}

#[test]
fn spline_area_over_u32() {
    // A 65536×65536 frame: `width * height` in u32 overflowed (in splines and LfGlobal).
    let (_, frame) = headers(false, true);
    assert_eq!(frame.width as u64 * frame.height as u64, 1 << 32);
    let mut w = Bits::default();
    w.entropy(&[0, 0, 0, 0, 0, 0], &[0]);
    w.put(0, 64);
    let _ = Splines::parse(&mut Bitstream::new(&w.out), (&frame, None));
}

#[test]
fn spline_count_tracked() {
    // 2^24 splines declared by a few bytes: ~9 GB of spline records before the tracker.
    let (_, frame) = headers(false, true);
    let tracker = AllocTracker::with_limit(1 << 20);
    let mut w = Bits::default();
    // Context 2 (count) → 2^24 - 1: token 24, 23 one bits.
    w.entropy(&[0, 0, 1, 0, 0, 0], &[0, 24]);
    w.put((1 << 23) - 1, 23);
    w.put(0, 64);
    let r = Splines::parse(&mut Bitstream::new(&w.out), (&frame, Some(&tracker)));
    assert_err(r.map(drop), "failed to allocate");
}

#[test]
fn tracker_count_saturates() {
    let t = AllocTracker::with_limit(1 << 20);
    assert!(t.alloc::<u64>(usize::MAX).is_err());
}

/// A bare codestream: 1×1 canvas and one frame of `side × side` with all sections empty.
fn codestream(side: u32) -> Vec<u8> {
    let mut w = Bits::default();
    w.image_header(false).pad();
    let entries = if side >= 18688 {
        w.big_frame_header(side);
        let groups = (side / 8).div_ceil(1024);
        2 + (side / 8).div_ceil(8192).pow(2) + groups * groups
    } else {
        w.put(1, 1); // all-default frame: the 1×1 canvas
        1
    };
    w.put(0, 1).pad(); // TOC not permuted
    for _ in 0..entries {
        w.put(0, 2).put(0, 10);
    }
    w.pad();
    w.out
}

#[test]
fn frame_far_beyond_canvas() {
    // A 1×1 image whose only frame is 65536×65536 (2^32 pixels) cropped over it.
    let r = crate::JxlFile::open(codestream(65536));
    assert!(matches!(r, Err(crate::Error::TooLarge)), "{:?}", r.err());
}

#[test]
fn small_codestream_opens() {
    // The builder above makes a well-formed header: the 1×1 variant opens (and only fails, if
    // at all, when its empty sections are decoded).
    crate::JxlFile::open(codestream(1)).unwrap();
    let r = crate::JxlFile::open(codestream(20000));
    assert!(r.is_ok(), "{:?}", r.err());
}
