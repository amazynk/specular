//! Separate colour planes against libavcodec. No encoder writes separate_colour_plane_flag,
//! so the stream is built here from three monochrome x265 streams encoded alike
//! (tests/data/gen_fixtures.sh): the SPS becomes a 4:4:4 one with separate_colour_plane_flag
//! and each slice segment header gains its colour_plane_id. Each colour plane must then
//! decode exactly as its monochrome stream does in libavcodec.

#[path = "../tests/md5/mod.rs"]
mod md5;

use crate::decode::ParameterSets;
use crate::nal::{NalHeader, Rbsp, split_annexb};
use crate::{ChromaFormat, VideoDecoder};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::{format, vec};

/// Bits `from..to` of `data` appended to `out` (a bit count `n`).
fn copy(out: &mut Vec<u8>, n: &mut usize, data: &[u8], from: usize, to: usize) {
    for p in from..to {
        put(out, n, data[p / 8] & (0x80 >> (p % 8)) != 0);
    }
}

fn put(out: &mut Vec<u8>, n: &mut usize, b: bool) {
    if n.is_multiple_of(8) {
        out.push(0);
    }
    if b {
        *out.last_mut().unwrap() |= 0x80 >> (*n % 8);
    }
    *n += 1;
}

/// A reader of the few header fields the rewrite needs, with its bit position.
struct Bits<'a> {
    d: &'a [u8],
    p: usize,
}

impl Bits<'_> {
    fn bit(&mut self) -> u32 {
        let b = (self.d[self.p / 8] >> (7 - self.p % 8)) & 1;
        self.p += 1;
        u32::from(b)
    }
    fn u(&mut self, n: u32) -> u32 {
        (0..n).fold(0, |v, _| (v << 1) | self.bit())
    }
    fn ue(&mut self) -> u32 {
        let mut z = 0;
        while self.bit() == 0 {
            z += 1;
        }
        (1 << z) - 1 + self.u(z)
    }
}

/// The NAL unit with `header` (two bytes) and `rbsp`, emulation prevention added.
fn nal(header: &[u8], rbsp: &[u8]) -> Vec<u8> {
    let mut out = vec![0, 0, 0, 1, header[0], header[1]];
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

/// Position of the last set bit of the RBSP (its stop bit).
fn stop_bit(d: &[u8]) -> usize {
    let last = d.iter().rposition(|&b| b != 0).unwrap();
    last * 8 + 7 - d[last].trailing_zeros() as usize
}

/// The three monochrome streams joined into one with separate colour planes.
fn join() -> Vec<u8> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data");
    let streams: Vec<Vec<u8>> = ["y", "u", "v"]
        .iter()
        .map(|p| std::fs::read(format!("{dir}/sep-{p}.hevc")).unwrap())
        .collect();
    let nals: Vec<Vec<&[u8]>> = streams.iter().map(|s| split_annexb(s)).collect();
    let vcl = |n: &&[u8]| NalHeader::parse(n).unwrap().is_vcl();
    let slices: Vec<Vec<&[u8]>> = nals
        .iter()
        .map(|v| v.iter().copied().filter(vcl).collect())
        .collect();
    let sets: Vec<&[u8]> = nals[0]
        .iter()
        .copied()
        .filter(|n| matches!(NalHeader::parse(n).unwrap().nal_unit_type, 32..=34))
        .collect();
    let ps = ParameterSets::parse(&sets).unwrap();
    let mut out = Vec::new();
    let mut k = 0;
    for n in &nals[0] {
        let h = NalHeader::parse(n).unwrap();
        match h.nal_unit_type {
            33 => {
                // chroma_format_idc 3 and separate_colour_plane_flag in place of
                // chroma_format_idc 0.
                let r = Rbsp::from_nal(n).unwrap();
                let mut b = Bits { d: &r.data, p: 4 };
                assert_eq!(b.u(3), 0, "one sub-layer expected");
                b.p += 1 + 96;
                b.ue();
                let at = b.p;
                assert_eq!(b.ue(), 0, "monochrome SPS expected");
                let (mut w, mut bits) = (Vec::new(), 0);
                copy(&mut w, &mut bits, &r.data, 0, at);
                for x in [false, false, true, false, false, true] {
                    put(&mut w, &mut bits, x);
                }
                copy(&mut w, &mut bits, &r.data, at + 1, stop_bit(&r.data) + 1);
                while !bits.is_multiple_of(8) {
                    put(&mut w, &mut bits, false);
                }
                out.extend(nal(&n[..2], &w));
            }
            32 | 34 => {
                out.extend_from_slice(&[0, 0, 0, 1]);
                out.extend_from_slice(n);
            }
            _ if h.is_vcl() => {
                // One slice per picture in these streams.
                for (plane, s) in slices.iter().enumerate() {
                    let n = s[k];
                    let h = NalHeader::parse(n).unwrap();
                    let r = Rbsp::from_nal(n).unwrap();
                    let mut b = Bits { d: &r.data, p: 0 };
                    assert_eq!(b.bit(), 1, "one slice segment per picture expected");
                    if h.is_irap() {
                        b.bit();
                    }
                    let pps = ps.pps(b.ue()).unwrap();
                    let sps = ps.sps(pps.sps_id).unwrap();
                    b.p += pps.extra_slice_header_bits as usize;
                    b.ue();
                    if pps.output_flag_present {
                        b.bit();
                    }
                    let at = b.p;
                    let sh = crate::slice::parse(&h, &r.data, sps, pps, None).unwrap();
                    // The header ends before byte_alignment()'s one bit.
                    let end = (0..sh.data_offset * 8)
                        .rev()
                        .find(|&p| r.data[p / 8] & (0x80 >> (p % 8)) != 0)
                        .unwrap();
                    let (mut w, mut bits) = (Vec::new(), 0);
                    copy(&mut w, &mut bits, &r.data, 0, at);
                    put(&mut w, &mut bits, plane & 2 != 0);
                    put(&mut w, &mut bits, plane & 1 != 0);
                    copy(&mut w, &mut bits, &r.data, at, end);
                    put(&mut w, &mut bits, true);
                    while !bits.is_multiple_of(8) {
                        put(&mut w, &mut bits, false);
                    }
                    w.extend_from_slice(&r.data[sh.data_offset..]);
                    out.extend(nal(&n[..2], &w));
                }
                k += 1;
            }
            _ => {}
        }
    }
    out
}

fn expected() -> Vec<[String; 3]> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/sep.md5");
    let text = std::fs::read_to_string(path).unwrap();
    let mut planes: [Vec<String>; 3] = Default::default();
    for l in text.lines() {
        let (p, h) = l.split_once(' ').unwrap();
        planes[["y", "u", "v"].iter().position(|&x| x == p).unwrap()].push(h.to_string());
    }
    (0..planes[0].len())
        .map(|i| core::array::from_fn(|p| planes[p][i].clone()))
        .collect()
}

/// Access units of the joined stream: the parameter sets, then three slices (one per colour
/// plane) per picture.
fn access_units(data: &[u8]) -> Vec<Vec<&[u8]>> {
    let mut aus: Vec<Vec<&[u8]>> = Vec::new();
    let mut cur: Vec<&[u8]> = Vec::new();
    let mut vcl = 0;
    for n in split_annexb(data) {
        cur.push(n);
        if NalHeader::parse(n).unwrap().is_vcl() {
            vcl += 1;
            if vcl % 3 == 0 {
                aus.push(core::mem::take(&mut cur));
            }
        }
    }
    aus
}

fn md5(p: &[u16]) -> String {
    let mut h = md5::Md5::new();
    let b: Vec<u8> = p.iter().map(|&s| s as u8).collect();
    h.update(&b);
    h.hex()
}

#[test]
fn separate_colour_planes_match_libavcodec() {
    let data = join();
    let want = expected();
    assert_eq!(want.len(), 12);
    let aus = access_units(&data);
    for threads in [1, 3] {
        let mut d = VideoDecoder::new(&[], threads).unwrap();
        let mut got = Vec::new();
        let mut take = |d: &mut VideoDecoder| {
            while let Some(p) = d.next_output() {
                let f = &p.frame;
                assert_eq!(f.chroma, ChromaFormat::Yuv444);
                assert_eq!((f.width, f.height), (96, 64));
                let plane = |s: &[u16]| -> Vec<u16> {
                    (0..64)
                        .flat_map(|y| s[y * f.y_stride as usize..][..96].to_vec())
                        .collect()
                };
                got.push([md5(&plane(&f.y)), md5(&plane(&f.cb)), md5(&plane(&f.cr))]);
            }
        };
        for (i, au) in aus.iter().enumerate() {
            d.decode_tagged(au, i as i64, None).unwrap();
            take(&mut d);
        }
        d.flush();
        take(&mut d);
        let bad = got.iter().zip(&want).filter(|(a, b)| a != b).count();
        assert_eq!(got.len(), want.len(), "{threads} threads");
        assert_eq!(bad, 0, "{threads} threads: frames differing");
    }
}

#[test]
fn separate_colour_planes_snapshot_restore() {
    let data = join();
    let aus = access_units(&data);
    let mut d = VideoDecoder::new(&[], 2).unwrap();
    let mut first = Vec::new();
    let mut snap = None;
    let mut before = 0;
    let at = 6;
    for (i, au) in aus.iter().enumerate() {
        if i == at {
            snap = Some(d.snapshot());
            before = first.len();
        }
        d.decode_tagged(au, i as i64, None).unwrap();
        while let Some(p) = d.next_output() {
            first.push(p.frame);
        }
    }
    d.flush();
    while let Some(p) = d.next_output() {
        first.push(p.frame);
    }
    let snap = snap.unwrap();
    assert!(!snap.pictures().is_empty());
    assert!(d.restore(&snap));
    let mut second = Vec::new();
    for (i, au) in aus.iter().enumerate().skip(at) {
        d.decode_tagged(au, i as i64, None).unwrap();
        while let Some(p) = d.next_output() {
            second.push(p.frame);
        }
    }
    d.flush();
    while let Some(p) = d.next_output() {
        second.push(p.frame);
    }
    assert_eq!(first.len(), 12);
    assert_eq!(second.len(), 12 - before);
    for (a, b) in first[before..].iter().zip(&second) {
        assert!(a.y == b.y && a.cb == b.cb && a.cr == b.cr);
    }
}

#[test]
fn separate_colour_planes_still() {
    let data = join();
    let aus = access_units(&data);
    let sets: Vec<&[u8]> = aus[0]
        .iter()
        .copied()
        .filter(|n| !NalHeader::parse(n).unwrap().is_vcl())
        .collect();
    let slices: Vec<&[u8]> = aus[0]
        .iter()
        .copied()
        .filter(|n| NalHeader::parse(n).unwrap().is_vcl())
        .collect();
    let f = crate::decode_still(&sets, &slices).unwrap();
    assert_eq!(f.chroma, ChromaFormat::Yuv444);
    let plane = |s: &[u16]| -> Vec<u16> {
        (0..64)
            .flat_map(|y| s[y * f.y_stride as usize..][..96].to_vec())
            .collect()
    };
    let got = [md5(&plane(&f.y)), md5(&plane(&f.cb)), md5(&plane(&f.cr))];
    assert_eq!(got, expected()[0]);
}
