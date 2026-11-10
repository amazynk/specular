//! Small Part 10 files built with [`crate::write`] for tests (here and in `specular-decode`):
//! video pixel data and encapsulated documents.

use std::borrow::Cow;

use crate::dict::Vr;
use crate::write::{self, FileMeta, Object, Value};

/// Video Endoscopic Image Storage.
pub const VIDEO_ENDOSCOPIC: &str = "1.2.840.10008.5.1.4.1.1.77.1.1.1";
pub const ENCAPSULATED_PDF: &str = "1.2.840.10008.5.1.4.1.1.104.1";
pub const ENCAPSULATED_CDA: &str = "1.2.840.10008.5.1.4.1.1.104.2";
pub const ENCAPSULATED_STL: &str = "1.2.840.10008.5.1.4.1.1.104.3";
pub const ENCAPSULATED_OBJ: &str = "1.2.840.10008.5.1.4.1.1.104.4";
pub const ENCAPSULATED_MTL: &str = "1.2.840.10008.5.1.4.1.1.104.5";
pub const H264_HIGH_41: &str = "1.2.840.10008.1.2.4.102";
pub const H264_HIGH_41_FRAGMENTABLE: &str = "1.2.840.10008.1.2.4.102.1";
pub const HEVC_MAIN: &str = "1.2.840.10008.1.2.4.107";
pub const HEVC_MAIN10: &str = "1.2.840.10008.1.2.4.108";
pub const MPEG2_MAIN: &str = "1.2.840.10008.1.2.4.100";

fn common(obj: &mut Object, class: &str, instance: &str) {
    obj.put_text(0x0008_0016, Vr::UI, class);
    obj.put_text(0x0008_0018, Vr::UI, instance);
    obj.put_text(0x0010_0010, Vr::PN, "Test^Patient");
    obj.put_text(0x0010_0020, Vr::LO, "ID-0001");
    obj.put_text(0x0008_1030, Vr::LO, "Synthetic study");
    obj.put_text(0x0020_000D, Vr::UI, "1.2.826.0.1.3680043.10.1.1");
    obj.put_text(0x0020_000E, Vr::UI, "1.2.826.0.1.3680043.10.1.2");
}

/// A video file: `stream` in fragments of at most `fragment` bytes (0: one fragment) after an
/// empty Basic Offset Table, YBR_PARTIAL_420 8-bit (or `bits`), `frames` frames
/// (`None`: Number of Frames left out), Frame Time `frame_ms`.
#[allow(clippy::too_many_arguments)]
pub fn video(
    ts: &str,
    stream: &[u8],
    fragment: usize,
    columns: u16,
    rows: u16,
    frames: Option<u32>,
    frame_ms: f64,
    bits: u16,
) -> Vec<u8> {
    let mut o = Object::default();
    common(&mut o, VIDEO_ENDOSCOPIC, "1.2.826.0.1.3680043.10.1.3");
    o.put_text(0x0008_0060, Vr::CS, "ES");
    o.put_text(0x0018_1063, Vr::DS, &format!("{frame_ms}"));
    o.put_bytes(0x0018_0040, Vr::IS, b"30".to_vec());
    o.put_bytes(0x0028_0002, Vr::US, 3u16.to_le_bytes());
    o.put_text(0x0028_0004, Vr::CS, "YBR_PARTIAL_420");
    o.put_bytes(0x0028_0006, Vr::US, 0u16.to_le_bytes());
    if let Some(n) = frames {
        o.put_text(0x0028_0008, Vr::IS, &n.to_string());
    }
    o.put_bytes(0x0028_0009, Vr::AT, [0x18u8, 0, 0x63, 0x10]);
    o.put_bytes(0x0028_0010, Vr::US, rows.to_le_bytes());
    o.put_bytes(0x0028_0011, Vr::US, columns.to_le_bytes());
    o.put_bytes(
        0x0028_0100,
        Vr::US,
        (if bits > 8 { 16u16 } else { 8 }).to_le_bytes(),
    );
    o.put_bytes(0x0028_0101, Vr::US, bits.to_le_bytes());
    o.put_bytes(0x0028_0102, Vr::US, (bits - 1).to_le_bytes());
    o.put_bytes(0x0028_0103, Vr::US, 0u16.to_le_bytes());
    o.put_text(0x0028_2110, Vr::CS, "01");
    let fragments = if fragment == 0 {
        vec![Cow::Borrowed(stream)]
    } else {
        stream.chunks(fragment).map(Cow::Borrowed).collect()
    };
    o.put(
        0x7FE0_0010,
        Vr::OB,
        Value::Fragments {
            offset_table: Cow::Borrowed(&[]),
            fragments,
        },
    );
    write::part10(&o, &FileMeta::of(&o, ts)).expect("video fixture")
}

/// An encapsulated document of SOP class `class`.
pub fn document(class: &str, mime: &str, title: Option<&str>, doc: &[u8]) -> Vec<u8> {
    let mut o = Object::default();
    common(&mut o, class, "1.2.826.0.1.3680043.10.1.4");
    o.put_text(0x0008_0060, Vr::CS, "DOC");
    if let Some(t) = title {
        o.put_text(0x0042_0010, Vr::ST, t);
    }
    o.put_bytes(0x0042_0011, Vr::OB, doc.to_vec());
    o.put_text(0x0042_0012, Vr::LO, mime);
    o.put_bytes(0x0042_0015, Vr::UL, (doc.len() as u32).to_le_bytes());
    let mut code = Object::default();
    code.put_text(0x0008_0100, Vr::SH, "18748-4");
    code.put_text(0x0008_0102, Vr::SH, "LN");
    code.put_text(0x0008_0104, Vr::LO, "Diagnostic imaging report");
    o.put(0x0040_A043, Vr::SQ, Value::Items(vec![code]));
    write::part10(&o, &FileMeta::of(&o, write::EXPLICIT_LE)).expect("document fixture")
}
