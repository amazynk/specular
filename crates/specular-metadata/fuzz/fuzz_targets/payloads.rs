#![no_main]
//! The EXIF (TIFF), XMP and ICC parsers fed directly. Input layout:
//! `[flags u8][exif len u16 LE][xmp len u16 LE][exif][xmp][icc = the rest]`; flags bits 0-2
//! say which payloads are present, bit 3 sets C2PA, bits 4-5 pick the language.

use libfuzzer_sys::fuzz_target;
use specular_metadata::{FileFacts, MetadataInput};

#[path = "common.rs"]
mod common;

fuzz_target!(|data: &[u8]| {
    if data.len() < 5 {
        return;
    }
    let flags = data[0];
    let exif_len = usize::from(u16::from_le_bytes([data[1], data[2]]));
    let xmp_len = usize::from(u16::from_le_bytes([data[3], data[4]]));
    let rest = &data[5..];
    let (exif, rest) = rest.split_at(exif_len.min(rest.len()));
    let (xmp, icc) = rest.split_at(xmp_len.min(rest.len()));
    let input = MetadataInput {
        exif_tiff: (flags & 1 != 0).then_some(exif),
        xmp: (flags & 2 != 0).then_some(xmp),
        icc: (flags & 4 != 0).then_some(icc),
        has_c2pa: flags & 8 != 0,
        file: FileFacts {
            format_name: Some("JPEG".into()),
            width: Some(4000),
            height: Some(3000),
            ..FileFacts::default()
        },
        medical: None,
        science: None,
        drawing: None,
        mesh: None,
        points: None,
        splats: None,
        rti: None,
        structure: None,
        font: None,
        book: None,
        volume: None,
        geo: None,
    };
    common::report(&input, common::localizer(flags >> 4));
});
