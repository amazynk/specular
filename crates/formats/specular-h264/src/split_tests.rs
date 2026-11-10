//! Separate colour planes against libavcodec. No encoder writes separate_colour_plane_flag,
//! so the streams are built here from three monochrome streams encoded alike
//! (tests/data/gen_fixtures.sh): their SPS becomes a High 4:4:4 Predictive one with
//! separate_colour_plane_flag, and each slice header gains its colour_plane_id. Each colour
//! plane must then decode exactly as its monochrome stream does in libavcodec.

#[path = "../tests/md5/mod.rs"]
mod md5;

use crate::bits::{BitReader, rbsp_end, unescape};
use crate::params::{parse_pps, parse_sps};
use crate::slice::parse_slice_header;
use crate::{Decoder, Options, OutputOrder, split_annexb};
use std::sync::Arc;

#[derive(Default)]
struct BitWriter {
    bytes: Vec<u8>,
    bits: usize,
}

impl BitWriter {
    fn bit(&mut self, b: bool) {
        if self.bits.is_multiple_of(8) {
            self.bytes.push(0);
        }
        if b {
            *self.bytes.last_mut().unwrap() |= 0x80 >> (self.bits % 8);
        }
        self.bits += 1;
    }

    fn u(&mut self, v: u32, n: u32) {
        for i in (0..n).rev() {
            self.bit(v >> i & 1 != 0);
        }
    }

    /// Bits `from..to` of `data`.
    fn copy(&mut self, data: &[u8], from: usize, to: usize) {
        for p in from..to {
            self.bit(data[p / 8] & (0x80 >> (p % 8)) != 0);
        }
    }

    /// rbsp_trailing_bits().
    fn trail(&mut self) {
        self.bit(true);
        while !self.bits.is_multiple_of(8) {
            self.bit(false);
        }
    }
}

/// A NAL unit: `header` byte and RBSP with emulation prevention added.
fn nal(header: u8, rbsp: &[u8]) -> Vec<u8> {
    let mut out = vec![0, 0, 0, 1, header];
    let mut zeros = 0;
    for &b in rbsp {
        if zeros >= 2 && b <= 3 {
            out.push(3);
            zeros = 0;
        }
        out.push(b);
        zeros = if b == 0 { zeros + 1 } else { 0 };
    }
    out
}

fn rbsp(n: &[u8]) -> (Vec<u8>, usize) {
    let mut b = Vec::new();
    let len = unescape(&n[1..], &mut b);
    (b, len)
}

/// The three monochrome streams of `kind` joined into one with separate colour planes.
fn join(kind: &str) -> Vec<u8> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data");
    let streams: Vec<Vec<u8>> = ["y", "u", "v"]
        .iter()
        .map(|p| std::fs::read(format!("{dir}/sep-{kind}-{p}.264")).unwrap())
        .collect();
    let nals: Vec<Vec<&[u8]>> = streams.iter().map(|s| split_annexb(s)).collect();
    let is_slice = |n: &&[u8]| matches!(n[0] & 31, 1 | 5);
    let slices: Vec<Vec<&[u8]>> = nals
        .iter()
        .map(|v| v.iter().copied().filter(is_slice).collect())
        .collect();
    let mut spss = vec![None; 32];
    let mut ppss = vec![None; 256];
    let mut out = Vec::new();
    let mut k = 0;
    for n in &nals[0] {
        match n[0] & 31 {
            7 => {
                let (b, len) = rbsp(n);
                let sps = parse_sps(&b, len).unwrap();
                let id = sps.id as usize;
                spss[id] = Some(Arc::new(sps));
                // profile_idc 244, then chroma_format_idc 3 with separate_colour_plane_flag
                // in place of chroma_format_idc 0.
                let mut r = BitReader::new(&b, len);
                r.skip(24);
                r.ue();
                let at = r.pos();
                assert!(r.ue() == 0, "monochrome SPS expected");
                let mut w = BitWriter::default();
                w.u(244, 8);
                w.copy(&b, 8, at);
                w.u(0b00100, 5);
                w.bit(true);
                w.copy(&b, at + 1, rbsp_end(&b, len));
                w.trail();
                out.extend(nal(n[0], &w.bytes));
            }
            8 => {
                let (b, len) = rbsp(n);
                let pps = parse_pps(&b, len, &spss).unwrap();
                let id = pps.id as usize;
                ppss[id] = Some(Arc::new(pps));
                out.extend_from_slice(&[0, 0, 0, 1]);
                out.extend_from_slice(n);
            }
            1 | 5 => {
                for (plane, s) in slices.iter().enumerate() {
                    let n = s[k];
                    let (b, len) = rbsp(n);
                    let mut r = BitReader::new(&b, len);
                    r.ue();
                    r.ue();
                    let pps = ppss[r.ue() as usize].clone().unwrap();
                    let at = r.pos();
                    let sps = spss[pps.sps_id as usize].clone().unwrap();
                    let h =
                        parse_slice_header(&b, len, n[0] & 31, n[0] >> 5 & 3, &sps, &pps).unwrap();
                    let mut w = BitWriter::default();
                    w.copy(&b, 0, at);
                    w.u(plane as u32, 2);
                    if pps.cabac {
                        // cabac_alignment_one_bit, then the byte-aligned slice data.
                        w.copy(&b, at, h.data_bit);
                        while !w.bits.is_multiple_of(8) {
                            w.bit(true);
                        }
                        w.bytes.extend_from_slice(&b[h.data_bit.div_ceil(8)..len]);
                    } else {
                        w.copy(&b, at, rbsp_end(&b, len));
                        w.trail();
                    }
                    out.extend(nal(n[0], &w.bytes));
                }
                k += 1;
            }
            _ => {}
        }
    }
    out
}

/// Per frame, the MD5 of each plane.
fn decode(data: &[u8], threads: usize) -> Vec<[String; 3]> {
    let mut d = Decoder::new(Options {
        threads,
        output: OutputOrder::Display,
    });
    let mut out = Vec::new();
    let mut take = |d: &mut Decoder| {
        while let Some(p) = d.next_picture(true) {
            assert_eq!(p.chroma, crate::Chroma::Yuv444);
            assert_eq!((p.width, p.height), (72, 40));
            out.push(p.planes.each_ref().map(|pl| {
                assert_eq!(pl.len(), 72 * 40);
                let mut h = md5::Md5::new();
                h.update(pl);
                h.hex()
            }));
        }
    };
    for n in split_annexb(data) {
        d.decode_nal(n, 0).unwrap();
        take(&mut d);
    }
    d.flush();
    take(&mut d);
    out
}

fn expected(kind: &str) -> Vec<[String; 3]> {
    let path = format!("{}/tests/data/sep-{kind}.md5", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(path).unwrap();
    let mut planes: [Vec<String>; 3] = Default::default();
    for l in text.lines() {
        let (p, h) = l.split_once(' ').unwrap();
        planes[["y", "u", "v"].iter().position(|&x| x == p).unwrap()].push(h.to_string());
    }
    (0..planes[0].len())
        .map(|i| std::array::from_fn(|p| planes[p][i].clone()))
        .collect()
}

#[test]
fn separate_colour_planes_match_libavcodec() {
    for kind in ["cabac", "cavlc", "lossless"] {
        let data = join(kind);
        let want = expected(kind);
        assert_eq!(want.len(), 12);
        for threads in [1, 4] {
            let got = decode(&data, threads);
            let bad = got.iter().zip(&want).filter(|(a, b)| a != b).count();
            assert_eq!(got.len(), want.len(), "{kind} at {threads} threads");
            assert_eq!(bad, 0, "{kind} at {threads} threads: frames differing");
        }
    }
}

#[test]
fn separate_colour_planes_snapshot_restore() {
    let data = join("cabac");
    let mut d = Decoder::new(Options {
        threads: 2,
        output: OutputOrder::Decode,
    });
    let nals = split_annexb(&data);
    // Snapshot before the second I picture, decode on, restore, decode on: both runs give
    // the same pictures from there.
    let drain = |d: &mut Decoder, out: &mut Vec<[Vec<u8>; 3]>| {
        while let Some(p) = d.next_picture(true) {
            out.push(p.planes);
        }
    };
    let mut snap = None;
    let mut first = Vec::new();
    let mut slices = 0;
    let mut at = 0;
    let mut before = 0;
    for (i, n) in nals.iter().enumerate() {
        if matches!(n[0] & 31, 1 | 5) {
            if slices == 3 * 8 && snap.is_none() {
                // An access unit delimiter ends the picture before it.
                d.decode_nal(&[9, 0xF0], 0).unwrap();
                drain(&mut d, &mut first);
                snap = d.snapshot();
                at = i;
                before = first.len();
            }
            slices += 1;
        }
        d.decode_nal(n, 0).unwrap();
        drain(&mut d, &mut first);
    }
    d.flush();
    drain(&mut d, &mut first);
    let snap = snap.expect("snapshot between access units");
    assert!(!snap.pictures().is_empty());
    d.restore(&snap);
    let mut second = Vec::new();
    for n in &nals[at..] {
        d.decode_nal(n, 0).unwrap();
        drain(&mut d, &mut second);
    }
    d.flush();
    drain(&mut d, &mut second);
    assert_eq!(first.len(), 12);
    assert_eq!(second.len(), 12 - before);
    assert!(
        first[before..] == second[..],
        "pictures after the snapshot differ"
    );
}
