//! ECW and MrSID aerial images: recognised and refused by name (checked
//! 2026-10-24).
//!
//! Both formats are published only as their makers' SDKs, and neither SDK's licence lets a
//! program use a copy the user installed for something else:
//!
//! - **ECW** (Hexagon ERDAS ECW/JP2 SDK). The Desktop Read-Only Redistributable licence
//!   (v5.x) grants the SDK only for Desktop Applications its licensee develops and ships
//!   with it, under an end-user licence carrying Hexagon's terms (§2.1, §3.3, Appendix A);
//!   anything else is forbidden (§3.5). The Server Read-Only End-user licence, which current
//!   GDAL packages carry, forbids desktop distribution and use with another product (§2.2,
//!   §3.7) and is paid per CPU core. There is no open description of the ECW file, so a
//!   `.ecw` file is known by its extension only.
//! - **MrSID** (Extensis/LizardTech MrSID Decode SDK, "DSDK"). Its licence covers the
//!   licensee's own development and distribution with the licensee's products, never
//!   separately from them (§1.2, §1.3(d)), and keeps the SDK confidential (§1.4). The file
//!   starts with "msid" (as GDAL identifies it); nothing more of it is openly described.
//!
//! JPEG 2000 files written by the ECW SDK are ordinary JP2 files: `jp2_dec` opens them, with
//! their GeoJP2 or GMLJP2 georeference.

use std::sync::Arc;

use specular_core::Source;

use crate::{DecodeError, Decoder, Format, Result, Session};

/// Why an ECW file is refused.
pub const ECW_REASON: &str = "ECW images can’t be opened: the format is published only as \
                              Hexagon’s SDK, whose licence doesn’t let other programs use it. \
                              Convert the file to GeoTIFF or JPEG 2000 with software that reads ECW.";

/// Why a MrSID file is refused.
pub const MRSID_REASON: &str = "MrSID images can’t be opened: the format is published only as \
                                the MrSID SDK, whose licence doesn’t let other programs use it. \
                                Convert the file to GeoTIFF or JPEG 2000 with software that \
                                reads MrSID.";

/// Whether `head` starts with MrSID's "msid" signature.
pub fn is_mrsid(head: &[u8]) -> bool {
    head.len() >= 8 && head[..4].eq_ignore_ascii_case(b"msid")
}

/// ECW (`.ecw`): no signature, so chosen by extension only.
pub struct EcwDecoder;

impl Decoder for EcwDecoder {
    fn format(&self) -> Format {
        Format::Ecw
    }
    fn probe(&self, _head: &[u8]) -> bool {
        false
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["ecw"]
    }
    fn open(&self, _source: Arc<Source>) -> Result<Arc<dyn Session>> {
        Err(DecodeError::Unsupported(ECW_REASON.into()))
    }
}

/// MrSID (`.sid`, signature "msid"). A `.sid` file without it (a Commodore 64 tune) is not
/// one.
pub struct MrSidDecoder;

impl Decoder for MrSidDecoder {
    fn format(&self) -> Format {
        Format::MrSid
    }
    fn probe(&self, head: &[u8]) -> bool {
        is_mrsid(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["sid"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        if is_mrsid(source.bytes()) {
            Err(DecodeError::Unsupported(MRSID_REASON.into()))
        } else {
            Err(DecodeError::Unsupported(
                "This .sid file is not a MrSID image".into(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(name: &str, bytes: &[u8]) -> String {
        let src = Arc::new(Source::from_vec(
            std::path::PathBuf::from(name),
            bytes.to_vec(),
        ));
        match crate::Registry::default().open(src) {
            Err(DecodeError::Unsupported(s)) => s,
            Err(e) => panic!("{name}: {e}"),
            Ok(_) => panic!("{name}: opened"),
        }
    }

    #[test]
    fn ecw_and_mrsid_are_refused_by_name() {
        // Content unknown: the extension decides.
        let ecw = open("survey.ECW", &[0x01, 0x02, 0x03, 0x04, 0, 0, 0, 0]);
        assert_eq!(ecw, ECW_REASON);
        assert!(ecw.contains("licence") && ecw.contains("GeoTIFF"));
        // The signature decides, whatever the name.
        let mut sid = b"msid\x02\x00\x00\x00".to_vec();
        sid.resize(64, 0);
        assert_eq!(open("ortho.sid", &sid), MRSID_REASON);
        assert_eq!(open("ortho.bin", &sid), MRSID_REASON);
        assert!(is_mrsid(b"MSID\x03\0\0\0"));
        // A C64 tune named .sid is not MrSID.
        let psid = b"PSID\x00\x02\x00\x7c\x00\x00\x10\x00".to_vec();
        assert!(!is_mrsid(&psid));
        assert!(open("tune.sid", &psid).contains("not a MrSID image"));
    }

    #[test]
    fn optional_decoders_list_them_as_refused() {
        use crate::optional::{self, OptionalId, State};
        for id in [OptionalId::Ecw, OptionalId::MrSid] {
            assert!(!id.usable());
            assert_eq!(optional::state(id), State::Refused);
            assert!(id.download_url().is_none());
        }
        assert_eq!(
            OptionalId::for_path(std::path::Path::new("a.ecw")),
            Some(OptionalId::Ecw)
        );
        assert_eq!(
            OptionalId::for_path(std::path::Path::new("a.SID")),
            Some(OptionalId::MrSid)
        );
        // No card or "Needs decoder" badge: there is nothing to install.
        assert_eq!(optional::needs_decoder(std::path::Path::new("a.ecw")), None);
    }
}
