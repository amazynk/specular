//! Hostile headers built in place: each must come back as an error or a sane result, quickly,
//! without a huge allocation or an overflow panic.

use crate::dict::Vr;
use crate::write::{self, FileMeta, Object, Value};
use crate::*;

/// An image of `columns` × `rows` 32-bit RGB claiming a billion frames, with `extra` elements
/// and a few bytes of native pixel data.
fn billion_frames(
    class: &str,
    columns: u16,
    rows: u16,
    extra: impl FnOnce(&mut Object),
) -> Vec<u8> {
    let mut o = Object::default();
    o.put_text(0x0008_0016, Vr::UI, class);
    o.put_text(0x0008_0018, Vr::UI, "1.2.826.0.1.3680043.10.9.1");
    o.put_bytes(0x0028_0002, Vr::US, 3u16.to_le_bytes());
    o.put_text(0x0028_0004, Vr::CS, "RGB");
    o.put_text(0x0028_0008, Vr::IS, "1000000000");
    o.put_bytes(0x0028_0010, Vr::US, rows.to_le_bytes());
    o.put_bytes(0x0028_0011, Vr::US, columns.to_le_bytes());
    o.put_bytes(0x0028_0100, Vr::US, 32u16.to_le_bytes());
    o.put_bytes(0x0028_0101, Vr::US, 32u16.to_le_bytes());
    o.put_bytes(0x0028_0102, Vr::US, 31u16.to_le_bytes());
    o.put_bytes(0x0028_0103, Vr::US, 0u16.to_le_bytes());
    extra(&mut o);
    o.put_bytes(0x7FE0_0010, Vr::OB, vec![7u8; 64]);
    write::part10(&o, &FileMeta::of(&o, write::EXPLICIT_LE)).unwrap()
}

const SECONDARY_CAPTURE: &str = "1.2.840.10008.5.1.4.1.1.7";

/// Frame 999,999,999 of a gigapixel 96-bit frame starts past 2^64 bits.
#[test]
fn native_frame_offset_past_u64() {
    let f = billion_frames(SECONDARY_CAPTURE, 32768, 32768, |_| {});
    let d = Dicom::parse(&f).unwrap();
    assert_eq!(d.frame_count(), 1_000_000_000);
    assert!(d.decode_frame(&f, 999_999_999, None).is_err());
}

/// Masking walked every claimed frame, overflowing the bit offset on the way.
#[test]
fn mask_stops_at_the_data() {
    let f = billion_frames(SECONDARY_CAPTURE, 32768, 32768, |_| {});
    let d = Dicom::parse(&f).unwrap();
    let info = d.pixel.clone().unwrap();
    let mut data = vec![7u8; 64];
    mask::mask_native(&info, &mut data, &[mask::Rect::new(0, 0, 2, 1)]);
    assert_eq!(&data[..24], &[0; 24]);
    assert_eq!(&data[24..], &[7; 40]);
}

/// A Frame Time with a billion frames gave a billion durations (16 GB).
#[test]
fn frame_durations_bounded_by_the_file() {
    let f = billion_frames(SECONDARY_CAPTURE, 4, 4, |o| {
        o.put_text(0x0018_1063, Vr::DS, "40");
    });
    let d = Dicom::parse(&f).unwrap();
    let t = d.frame_durations(&f).unwrap();
    assert!(t.len() <= f.len());
}

/// A segmentation placing a billion frames through its shared groups.
#[test]
fn seg_frames_bounded_by_the_file() {
    let f = billion_frames(seg::SEG_SOP_CLASS, 4, 4, |o| {
        o.put_text(0x0062_0001, Vr::CS, "BINARY");
        let mut segment = Object::default();
        segment.put_bytes(0x0062_0004, Vr::US, 1u16.to_le_bytes());
        segment.put_text(0x0062_0005, Vr::LO, "s");
        o.put(0x0062_0002, Vr::SQ, Value::Items(vec![segment]));
        let item = |tag: u32, vr: Vr, text: &str| {
            let mut i = Object::default();
            i.put_text(tag, vr, text);
            i
        };
        let mut id = Object::default();
        id.put_bytes(0x0062_000B, Vr::US, 1u16.to_le_bytes());
        let mut shared = Object::default();
        shared.put(
            0x0020_9116,
            Vr::SQ,
            Value::Items(vec![item(0x0020_0037, Vr::DS, "1\\0\\0\\0\\1\\0")]),
        );
        shared.put(
            0x0028_9110,
            Vr::SQ,
            Value::Items(vec![item(0x0028_0030, Vr::DS, "1\\1")]),
        );
        shared.put(
            0x0020_9113,
            Vr::SQ,
            Value::Items(vec![item(0x0020_0032, Vr::DS, "0\\0\\0")]),
        );
        shared.put(0x0062_000A, Vr::SQ, Value::Items(vec![id]));
        o.put(0x5200_9229, Vr::SQ, Value::Items(vec![shared]));
    });
    let d = Dicom::parse(&f).unwrap();
    let h = seg::SegHeader::read(&d, &f).unwrap();
    assert!(h.frames.len() <= f.len() * 8);
}

/// Channel and sample counts near i64::MAX: the product overflowed, and a zero sample count
/// let 2^62 channels through.
#[test]
fn waveform_counts_near_i64_max() {
    for (channels, samples) in [("4294967296", "4294967296"), ("4611686018427387904", "0")] {
        let mut o = Object::default();
        o.put_text(0x0008_0016, Vr::UI, "1.2.840.10008.5.1.4.1.1.9.1.1");
        o.put_text(0x0008_0018, Vr::UI, "1.2.826.0.1.3680043.10.9.2");
        let mut w = Object::default();
        w.put_text(0x003A_0005, Vr::IS, channels);
        w.put_text(0x003A_0010, Vr::IS, samples);
        o.put(0x5400_0100, Vr::SQ, Value::Items(vec![w]));
        let f = write::part10(&o, &FileMeta::of(&o, write::EXPLICIT_LE)).unwrap();
        let d = Dicom::parse(&f).unwrap();
        assert!(waveform::Waveforms::read(&d, &f).is_err());
    }
}
