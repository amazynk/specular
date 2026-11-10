//! Synthetic Part 10 files for the SCU tests.
#![allow(dead_code)]

use std::borrow::Cow;

use specular_dicom::dict::Vr;
use specular_dicom::write::{self, FileMeta, Object, Value};

pub const CT: &str = "1.2.840.10008.5.1.4.1.1.2";
pub const MR: &str = "1.2.840.10008.5.1.4.1.1.4";
pub const RLE: &str = "1.2.840.10008.1.2.5";
pub const EXPLICIT: &str = "1.2.840.10008.1.2.1";
pub const IMPLICIT: &str = "1.2.840.10008.1.2";

/// 16-bit pixels of a `size`² CT slice `i` (a ramp with a bright square).
pub fn pixels(size: u16, i: u32) -> Vec<u16> {
    let n = size as usize;
    (0..n * n)
        .map(|k| {
            let (x, y) = (k % n, k / n);
            let base = (x * 7 + y * 3) as u16 + i as u16 * 11;
            if (n / 4..n / 2).contains(&x) && (n / 4..n / 2).contains(&y) {
                base.wrapping_add(2000)
            } else {
                base
            }
        })
        .collect()
}

pub fn object(class: &str, instance: &str, size: u16, i: u32) -> Object<'static> {
    let mut o = Object::default();
    o.put_text(0x0008_0016, Vr::UI, class);
    o.put_text(0x0008_0018, Vr::UI, instance);
    o.put_text(0x0008_0060, Vr::CS, "CT");
    o.put_text(0x0010_0010, Vr::PN, "Test^Patient");
    o.put_text(0x0010_0020, Vr::LO, "ID-0001");
    o.put_text(0x0020_000D, Vr::UI, "1.2.826.0.1.3680043.10.9.1");
    o.put_text(0x0020_000E, Vr::UI, "1.2.826.0.1.3680043.10.9.2");
    o.put_bytes(0x0020_0013, Vr::IS, format!("{}", i + 1).into_bytes());
    o.put_bytes(0x0028_0002, Vr::US, 1u16.to_le_bytes().to_vec());
    o.put_text(0x0028_0004, Vr::CS, "MONOCHROME2");
    o.put_bytes(0x0028_0010, Vr::US, size.to_le_bytes().to_vec());
    o.put_bytes(0x0028_0011, Vr::US, size.to_le_bytes().to_vec());
    o.put_bytes(0x0028_0100, Vr::US, 16u16.to_le_bytes().to_vec());
    o.put_bytes(0x0028_0101, Vr::US, 16u16.to_le_bytes().to_vec());
    o.put_bytes(0x0028_0102, Vr::US, 15u16.to_le_bytes().to_vec());
    o.put_bytes(0x0028_0103, Vr::US, 0u16.to_le_bytes().to_vec());
    // A sequence, to check items survive conversion.
    let mut item = Object::default();
    item.put_text(0x0008_0100, Vr::SH, "T-D0050");
    item.put_text(0x0008_0102, Vr::SH, "SRT");
    o.put(0x0008_2218, Vr::SQ, Value::Items(vec![item]));
    o
}

fn le(px: &[u16]) -> Vec<u8> {
    px.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// Native Explicit VR Little Endian file.
pub fn explicit(class: &str, instance: &str, size: u16, i: u32) -> Vec<u8> {
    let mut o = object(class, instance, size, i);
    o.put(
        0x7FE0_0010,
        Vr::OW,
        Value::Bytes(Cow::Owned(le(&pixels(size, i)))),
    );
    write::part10(&o, &FileMeta::of(&o, EXPLICIT)).unwrap()
}

/// The same with an Implicit VR Little Endian data set.
pub fn implicit(class: &str, instance: &str, size: u16, i: u32) -> Vec<u8> {
    let mut o = object(class, instance, size, i);
    o.put(
        0x7FE0_0010,
        Vr::OW,
        Value::Bytes(Cow::Owned(le(&pixels(size, i)))),
    );
    let meta_only = write::part10(&Object::default(), &FileMeta::of(&o, IMPLICIT)).unwrap();
    let mut out = meta_only;
    specular_dimse::store::implicit(&o, &mut out).unwrap();
    out
}

/// RLE Lossless (PS3.5 annex G): two byte planes (high, then low), literal runs only.
pub fn rle(class: &str, instance: &str, size: u16, i: u32) -> Vec<u8> {
    let px = pixels(size, i);
    let plane = |hi: bool| -> Vec<u8> {
        let bytes: Vec<u8> = px
            .iter()
            .map(|v| if hi { (v >> 8) as u8 } else { *v as u8 })
            .collect();
        let mut seg = Vec::new();
        for c in bytes.chunks(128) {
            seg.push((c.len() - 1) as u8);
            seg.extend_from_slice(c);
        }
        if seg.len() % 2 == 1 {
            seg.push(0x80); // no-op
        }
        seg
    };
    let (a, b) = (plane(true), plane(false));
    let mut frag = vec![0u8; 64];
    frag[0..4].copy_from_slice(&2u32.to_le_bytes());
    frag[4..8].copy_from_slice(&64u32.to_le_bytes());
    frag[8..12].copy_from_slice(&(64 + a.len() as u32).to_le_bytes());
    frag.extend_from_slice(&a);
    frag.extend_from_slice(&b);
    let mut o = object(class, instance, size, i);
    o.put(
        0x7FE0_0010,
        Vr::OB,
        Value::Fragments {
            offset_table: Cow::Owned(Vec::new()),
            fragments: vec![Cow::Owned(frag)],
        },
    );
    write::part10(&o, &FileMeta::of(&o, RLE)).unwrap()
}

/// A received data set back into a file, to parse it.
pub fn wrap(data: &[u8], class: &str, instance: &str, ts: &str) -> Vec<u8> {
    let meta = FileMeta {
        sop_class: class.into(),
        sop_instance: instance.into(),
        transfer_syntax: ts.into(),
    };
    let mut f = write::part10(&Object::default(), &meta).unwrap();
    f.extend_from_slice(data);
    f
}

/// The 16-bit pixels of a file, decoded.
pub fn decoded(file: &[u8]) -> Vec<u16> {
    let d = specular_dicom::Dicom::parse(file).unwrap();
    let f = d.decode_stored(file, 0, None).unwrap();
    match f.pixels {
        specular_dicom::Pixels::Grey(g) => (0..g.len()).map(|k| g.get(k).unwrap() as u16).collect(),
        _ => panic!("not grey"),
    }
}
