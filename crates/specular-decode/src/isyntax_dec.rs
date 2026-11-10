//! Philips iSyntax whole-slide images: recognised and refused by name.
//!
//! The format's coefficient coding (bitplanes, zero runs, prefix codes) is claimed by a
//! Philips patent in force (US 10,917,663 B2, decoding in claim 8, to about 2037; granted
//! in China, Japan and Russia too), and the Philips SDK's licence is not public. Until the
//! user decides, Specular neither decodes iSyntax itself nor links Philips' SDK: the file
//! is recognised by its XML header (`<DataObject ObjectType="DPUfsImport">`) and refused
//! with that reason.

use std::sync::Arc;

use specular_core::Source;

use crate::{DecodeError, Decoder, Format, Result, Session};

pub struct IsyntaxDecoder;

/// The reason shown.
pub const REASON: &str = "Philips iSyntax is not supported: its compression is patented \
                          by Philips and their reader's licence is not public. Export the \
                          slide to TIFF or DICOM with Philips' software to open it here.";

/// Whether `head` is an iSyntax header.
pub fn sniff(head: &[u8]) -> bool {
    let h = &head[..head.len().min(512)];
    let s = String::from_utf8_lossy(h);
    let s = s.trim_start_matches('\u{feff}').trim_start();
    s.starts_with("<DataObject") && s.contains("DPUfsImport")
}

impl Decoder for IsyntaxDecoder {
    fn format(&self) -> Format {
        Format::Isyntax
    }
    fn probe(&self, head: &[u8]) -> bool {
        sniff(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["isyntax"]
    }
    fn open(&self, _source: Arc<Source>) -> Result<Arc<dyn Session>> {
        Err(DecodeError::Unsupported(REASON.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognised_and_refused_by_name() {
        let head =
            b"<DataObject ObjectType=\"DPUfsImport\">\r\n<Attribute Name=\"DICOM_MANUFACTURER\"";
        assert!(sniff(head));
        assert!(!sniff(b"<?xml version=\"1.0\"?><svg/>"));
        let src = Arc::new(Source::from_vec(
            std::path::PathBuf::from("a.isyntax"),
            head.to_vec(),
        ));
        match crate::Registry::default().open(src) {
            Err(DecodeError::Unsupported(s)) => assert!(s.starts_with("Philips iSyntax")),
            Err(e) => panic!("{e}"),
            Ok(_) => panic!("opened"),
        }
    }
}
