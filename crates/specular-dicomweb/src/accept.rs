//! Transfer syntax negotiation: the `Accept` header lists what the in-tree decoders read.

/// Transfer syntaxes the in-tree decoders read, in the order offered: lossless compressed
/// first (less to transfer, nothing lost if the server transcodes), then lossy ones (only
/// useful when the stored data already is lossy), then uncompressed.
pub const DECODABLE: &[&str] = &[
    "1.2.840.10008.1.2.4.201", // HTJ2K lossless
    "1.2.840.10008.1.2.4.202", // HTJ2K RPCL lossless
    "1.2.840.10008.1.2.4.80",  // JPEG-LS lossless
    "1.2.840.10008.1.2.4.90",  // JPEG 2000 lossless
    "1.2.840.10008.1.2.4.92",  // JPEG 2000 Part 2 lossless
    "1.2.840.10008.1.2.4.70",  // JPEG lossless SV1
    "1.2.840.10008.1.2.4.57",  // JPEG lossless
    "1.2.840.10008.1.2.5",     // RLE
    "1.2.840.10008.1.2.8.1",   // Deflated image frame
    "1.2.840.10008.1.2.1.99",  // Deflated explicit VR LE
    "1.2.840.10008.1.2.4.203", // HTJ2K
    "1.2.840.10008.1.2.4.91",  // JPEG 2000
    "1.2.840.10008.1.2.4.93",  // JPEG 2000 Part 2
    "1.2.840.10008.1.2.4.81",  // JPEG-LS near-lossless
    "1.2.840.10008.1.2.4.50",  // JPEG baseline
    "1.2.840.10008.1.2.4.51",  // JPEG extended
    "1.2.840.10008.1.2.1.98",  // Encapsulated uncompressed
    "1.2.840.10008.1.2.1",     // Explicit VR LE (the WADO-RS default)
];

/// `Accept` for whole instances. `any`: also `transfer-syntax=*` (the server's choice), used
/// after a server refused the explicit list with 406.
pub fn instances(any: bool) -> String {
    media_ranges("application/dicom", any)
}

/// `Accept` for frames and pixel bulk data: compressed bitstreams by syntax, or uncompressed
/// octets.
pub fn frames(any: bool) -> String {
    let mut v = media_ranges("application/octet-stream", any);
    // Single-frame image media types some servers prefer for compressed frames.
    for (mt, ts) in [
        ("image/jls", "1.2.840.10008.1.2.4.80"),
        ("image/jp2", "1.2.840.10008.1.2.4.90"),
        ("image/jphc", "1.2.840.10008.1.2.4.201"),
    ] {
        v.push_str(&format!(
            ", multipart/related; type=\"{mt}\"; transfer-syntax={ts}"
        ));
    }
    v
}

fn media_ranges(ty: &str, any: bool) -> String {
    let mut v: Vec<String> = DECODABLE
        .iter()
        .map(|ts| format!("multipart/related; type=\"{ty}\"; transfer-syntax={ts}"))
        .collect();
    if any {
        v.push(format!(
            "multipart/related; type=\"{ty}\"; transfer-syntax=*"
        ));
    }
    v.join(", ")
}

/// Whether the in-tree decoders read pixel data in this syntax. The retired JPEG syntaxes
/// decode (all but arithmetic lossless hierarchical, .66) but are never offered: no server
/// transcodes into them.
pub fn decodable(uid: &str) -> bool {
    use specular_dicom::Compression::*;
    let ts = specular_dicom::TransferSyntax::from_uid(uid);
    !matches!(ts.compression, Video | Other) && uid != "1.2.840.10008.1.2.4.66"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_offered_syntax_is_decodable_and_vice_versa() {
        for ts in DECODABLE {
            assert!(decodable(ts), "{ts}");
        }
        // Every table syntax the decoders read is offered (implicit VR, big endian and Papyrus
        // are not WADO-RS transfer syntaxes).
        for ts in [
            "1.2.840.10008.1.2.4.50",
            "1.2.840.10008.1.2.4.57",
            "1.2.840.10008.1.2.5",
            "1.2.840.10008.1.2.4.203",
        ] {
            assert!(DECODABLE.contains(&ts));
        }
        assert!(!decodable("1.2.840.10008.1.2.4.102"));
        assert!(decodable("1.2.840.10008.1.2.4.55"));
        assert!(!decodable("1.2.840.10008.1.2.4.66"));
        let a = instances(false);
        assert!(a.starts_with(
            "multipart/related; type=\"application/dicom\"; transfer-syntax=1.2.840.10008.1.2.4.201"
        ));
        assert!(!a.contains('*'));
        assert!(instances(true).ends_with("transfer-syntax=*"));
    }
}
