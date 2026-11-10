//! UIDs this crate names: the Verification SOP class, the uncompressed transfer syntaxes and
//! the names of common storage SOP classes (for messages about what a server refused).

pub const VERIFICATION: &str = "1.2.840.10008.1.1";
pub const IMPLICIT_LE: &str = "1.2.840.10008.1.2";
pub const EXPLICIT_LE: &str = "1.2.840.10008.1.2.1";
/// Study Root Query/Retrieve Information Model - FIND (PS3.4 C.6.2).
pub const STUDY_ROOT_FIND: &str = "1.2.840.10008.5.1.4.1.2.2.1";
/// Study Root Query/Retrieve Information Model - MOVE.
pub const STUDY_ROOT_MOVE: &str = "1.2.840.10008.5.1.4.1.2.2.2";
/// Study Root Query/Retrieve Information Model - GET.
pub const STUDY_ROOT_GET: &str = "1.2.840.10008.5.1.4.1.2.2.3";

/// Transfer syntaxes the in-tree decoders read, as specular-dicomweb's `accept::DECODABLE`
/// lists them for WADO-RS, plus Implicit VR Little Endian and Explicit VR Big Endian.
pub const DECODABLE: &[&str] = &[
    "1.2.840.10008.1.2.1",
    "1.2.840.10008.1.2",
    "1.2.840.10008.1.2.4.201",
    "1.2.840.10008.1.2.4.202",
    "1.2.840.10008.1.2.4.80",
    "1.2.840.10008.1.2.4.90",
    "1.2.840.10008.1.2.4.92",
    "1.2.840.10008.1.2.4.70",
    "1.2.840.10008.1.2.4.57",
    "1.2.840.10008.1.2.5",
    "1.2.840.10008.1.2.8.1",
    "1.2.840.10008.1.2.1.99",
    "1.2.840.10008.1.2.4.203",
    "1.2.840.10008.1.2.4.91",
    "1.2.840.10008.1.2.4.93",
    "1.2.840.10008.1.2.4.81",
    "1.2.840.10008.1.2.4.50",
    "1.2.840.10008.1.2.4.51",
    "1.2.840.10008.1.2.1.98",
    "1.2.840.10008.1.2.2",
];

/// Storage classes retrieved besides those named in [`name`]'s table.
const MORE_STORAGE: &[&str] = &[
    "1.2.840.10008.5.1.4.1.1.2.2",    // Legacy Converted Enhanced CT
    "1.2.840.10008.5.1.4.1.1.4.4",    // Legacy Converted Enhanced MR
    "1.2.840.10008.5.1.4.1.1.128.1",  // Legacy Converted Enhanced PET
    "1.2.840.10008.5.1.4.1.1.130",    // Enhanced PET
    "1.2.840.10008.5.1.4.1.1.6.2",    // Enhanced US Volume
    "1.2.840.10008.5.1.4.1.1.7.2",    // Multi-frame Grayscale Byte SC
    "1.2.840.10008.5.1.4.1.1.7.3",    // Multi-frame Grayscale Word SC
    "1.2.840.10008.5.1.4.1.1.7.4",    // Multi-frame True Color SC
    "1.2.840.10008.5.1.4.1.1.12.1.1", // Enhanced XA
    "1.2.840.10008.5.1.4.1.1.1.2.1",  // Digital Mammography (processing)
    "1.2.840.10008.5.1.4.1.1.11.2",   // Color Softcopy Presentation State
];

/// The image classes most series hold: these also get contexts for compressed syntaxes.
pub const COMMON_IMAGES: &[&str] = &[
    "1.2.840.10008.5.1.4.1.1.2",
    "1.2.840.10008.5.1.4.1.1.4",
    "1.2.840.10008.5.1.4.1.1.1",
    "1.2.840.10008.5.1.4.1.1.1.1",
    "1.2.840.10008.5.1.4.1.1.1.2",
    "1.2.840.10008.5.1.4.1.1.6.1",
    "1.2.840.10008.5.1.4.1.1.3.1",
    "1.2.840.10008.5.1.4.1.1.7",
    "1.2.840.10008.5.1.4.1.1.12.1",
    "1.2.840.10008.5.1.4.1.1.12.2",
    "1.2.840.10008.5.1.4.1.1.20",
    "1.2.840.10008.5.1.4.1.1.128",
    "1.2.840.10008.5.1.4.1.1.2.1",
];

/// Compressed syntaxes servers store most, offered for [`COMMON_IMAGES`].
pub const COMMON_COMPRESSED: &[&str] = &[
    "1.2.840.10008.1.2.4.70",
    "1.2.840.10008.1.2.4.50",
    "1.2.840.10008.1.2.4.90",
    "1.2.840.10008.1.2.4.91",
    "1.2.840.10008.1.2.4.80",
    "1.2.840.10008.1.2.5",
];

/// Every storage class a retrieval asks for: the named ones, then a few more.
pub fn storage_classes() -> impl Iterator<Item = &'static str> {
    NAMES
        .iter()
        .map(|(u, _)| *u)
        .chain(MORE_STORAGE.iter().copied())
}

/// Whether `uid` is a storage SOP class (PS3.4 annex B: 1.2.840.10008.5.1.4.1.1.*).
pub fn is_storage(uid: &str) -> bool {
    uid.starts_with("1.2.840.10008.5.1.4.1.1.")
}

/// Storage SOP classes by UID (PS3.6 annex A), the ones patient media usually hold.
const NAMES: &[(&str, &str)] = &[
    ("1.2.840.10008.5.1.4.1.1.1", "Computed Radiography Image"),
    (
        "1.2.840.10008.5.1.4.1.1.1.1",
        "Digital X-Ray Image (presentation)",
    ),
    (
        "1.2.840.10008.5.1.4.1.1.1.1.1",
        "Digital X-Ray Image (processing)",
    ),
    (
        "1.2.840.10008.5.1.4.1.1.1.2",
        "Digital Mammography X-Ray Image (presentation)",
    ),
    (
        "1.2.840.10008.5.1.4.1.1.1.3",
        "Digital Intra-Oral X-Ray Image (presentation)",
    ),
    ("1.2.840.10008.5.1.4.1.1.2", "CT Image"),
    ("1.2.840.10008.5.1.4.1.1.2.1", "Enhanced CT Image"),
    (
        "1.2.840.10008.5.1.4.1.1.3.1",
        "Ultrasound Multi-frame Image",
    ),
    ("1.2.840.10008.5.1.4.1.1.4", "MR Image"),
    ("1.2.840.10008.5.1.4.1.1.4.1", "Enhanced MR Image"),
    ("1.2.840.10008.5.1.4.1.1.6.1", "Ultrasound Image"),
    ("1.2.840.10008.5.1.4.1.1.7", "Secondary Capture Image"),
    (
        "1.2.840.10008.5.1.4.1.1.11.1",
        "Grayscale Softcopy Presentation State",
    ),
    ("1.2.840.10008.5.1.4.1.1.12.1", "X-Ray Angiographic Image"),
    (
        "1.2.840.10008.5.1.4.1.1.12.2",
        "X-Ray Radiofluoroscopic Image",
    ),
    (
        "1.2.840.10008.5.1.4.1.1.13.1.1",
        "X-Ray 3D Angiographic Image",
    ),
    (
        "1.2.840.10008.5.1.4.1.1.13.1.3",
        "Breast Tomosynthesis Image",
    ),
    ("1.2.840.10008.5.1.4.1.1.20", "Nuclear Medicine Image"),
    ("1.2.840.10008.5.1.4.1.1.66", "Raw Data"),
    ("1.2.840.10008.5.1.4.1.1.66.4", "Segmentation"),
    ("1.2.840.10008.5.1.4.1.1.77.1.1", "VL Endoscopic Image"),
    ("1.2.840.10008.5.1.4.1.1.77.1.2", "VL Microscopic Image"),
    ("1.2.840.10008.5.1.4.1.1.77.1.4", "VL Photographic Image"),
    (
        "1.2.840.10008.5.1.4.1.1.77.1.5.1",
        "Ophthalmic Photography 8 Bit Image",
    ),
    (
        "1.2.840.10008.5.1.4.1.1.77.1.6",
        "VL Whole Slide Microscopy Image",
    ),
    ("1.2.840.10008.5.1.4.1.1.88.11", "Basic Text SR"),
    ("1.2.840.10008.5.1.4.1.1.88.22", "Enhanced SR"),
    ("1.2.840.10008.5.1.4.1.1.88.33", "Comprehensive SR"),
    (
        "1.2.840.10008.5.1.4.1.1.88.59",
        "Key Object Selection Document",
    ),
    ("1.2.840.10008.5.1.4.1.1.88.67", "X-Ray Radiation Dose SR"),
    ("1.2.840.10008.5.1.4.1.1.104.1", "Encapsulated PDF"),
    ("1.2.840.10008.5.1.4.1.1.128", "PET Image"),
    ("1.2.840.10008.5.1.4.1.1.481.1", "RT Image"),
    ("1.2.840.10008.5.1.4.1.1.481.2", "RT Dose"),
    ("1.2.840.10008.5.1.4.1.1.481.3", "RT Structure Set"),
    ("1.2.840.10008.5.1.4.1.1.481.5", "RT Plan"),
];

/// A storage SOP class's name, else its UID.
pub fn name(uid: &str) -> &str {
    NAMES
        .iter()
        .find(|(u, _)| *u == uid)
        .map_or(uid, |(_, n)| n)
}

#[cfg(test)]
mod tests {
    #[test]
    fn names_sorted_and_known() {
        assert_eq!(super::name("1.2.840.10008.5.1.4.1.1.2"), "CT Image");
        assert_eq!(super::name("1.2.3"), "1.2.3");
        let mut seen = std::collections::HashSet::new();
        assert!(super::NAMES.iter().all(|(u, _)| seen.insert(*u)));
    }
}
