//! Value representations and the data-element dictionary (a generated table, see
//! `gen/dict.py`): the VR of a tag for implicit-VR data sets, and its keyword for the
//! tag browser.

/// A DICOM value representation (PS3.5 6.2).
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Vr {
    AE,
    AS,
    AT,
    CS,
    DA,
    DS,
    DT,
    FD,
    FL,
    IS,
    LO,
    LT,
    OB,
    OD,
    OF,
    OL,
    OV,
    OW,
    PN,
    SH,
    SL,
    SQ,
    SS,
    ST,
    SV,
    TM,
    UC,
    UI,
    UL,
    UN,
    UR,
    US,
    UT,
    UV,
}

impl Vr {
    /// The VR named by two bytes of an explicit-VR element header.
    pub fn from_bytes(b: [u8; 2]) -> Option<Vr> {
        use Vr::*;
        Some(match &b {
            b"AE" => AE,
            b"AS" => AS,
            b"AT" => AT,
            b"CS" => CS,
            b"DA" => DA,
            b"DS" => DS,
            b"DT" => DT,
            b"FD" => FD,
            b"FL" => FL,
            b"IS" => IS,
            b"LO" => LO,
            b"LT" => LT,
            b"OB" => OB,
            b"OD" => OD,
            b"OF" => OF,
            b"OL" => OL,
            b"OV" => OV,
            b"OW" => OW,
            b"PN" => PN,
            b"SH" => SH,
            b"SL" => SL,
            b"SQ" => SQ,
            b"SS" => SS,
            b"ST" => ST,
            b"SV" => SV,
            b"TM" => TM,
            b"UC" => UC,
            b"UI" => UI,
            b"UL" => UL,
            b"UN" => UN,
            b"UR" => UR,
            b"US" => US,
            b"UT" => UT,
            b"UV" => UV,
            _ => return None,
        })
    }

    /// Whether the explicit-VR header has a reserved 2 bytes and a 32-bit length
    /// (PS3.5 7.1.2).
    pub fn long_length(self) -> bool {
        use Vr::*;
        matches!(
            self,
            OB | OD | OF | OL | OV | OW | SQ | SV | UC | UN | UR | UT | UV
        )
    }

    /// Whether the value is text (trailing spaces / NULs are padding).
    pub fn is_text(self) -> bool {
        use Vr::*;
        matches!(
            self,
            AE | AS | CS | DA | DS | DT | IS | LO | LT | PN | SH | ST | TM | UC | UI | UR | UT
        )
    }
}

/// Dictionary entry for `tag`: its VR and keyword. Private tags (odd groups) and tags outside
/// the generated groups are absent.
pub fn lookup(tag: u32) -> Option<(Vr, &'static str)> {
    let e = if tag >> 16 == 0x0004 {
        DIRECTORY
    } else {
        crate::dict_table::ENTRIES
    };
    e.binary_search_by_key(&tag, |&(t, _, _)| t)
        .ok()
        .map(|i| (e[i].1, e[i].2))
}

/// The directory structuring elements of a DICOMDIR (PS3.6 group 0004, PS3.3 F.3), which the
/// generated table leaves out.
static DIRECTORY: &[(u32, Vr, &str)] = &[
    (0x0004_1130, Vr::CS, "FileSetID"),
    (0x0004_1141, Vr::CS, "FileSetDescriptorFileID"),
    (
        0x0004_1142,
        Vr::CS,
        "SpecificCharacterSetOfFileSetDescriptorFile",
    ),
    (
        0x0004_1200,
        Vr::UL,
        "OffsetOfTheFirstDirectoryRecordOfTheRootDirectoryEntity",
    ),
    (
        0x0004_1202,
        Vr::UL,
        "OffsetOfTheLastDirectoryRecordOfTheRootDirectoryEntity",
    ),
    (0x0004_1212, Vr::US, "FileSetConsistencyFlag"),
    (0x0004_1220, Vr::SQ, "DirectoryRecordSequence"),
    (0x0004_1400, Vr::UL, "OffsetOfTheNextDirectoryRecord"),
    (0x0004_1410, Vr::US, "RecordInUseFlag"),
    (
        0x0004_1420,
        Vr::UL,
        "OffsetOfReferencedLowerLevelDirectoryEntity",
    ),
    (0x0004_1430, Vr::CS, "DirectoryRecordType"),
    (0x0004_1432, Vr::UI, "PrivateRecordUID"),
    (0x0004_1500, Vr::CS, "ReferencedFileID"),
    (0x0004_1504, Vr::UL, "MRDRDirectoryRecordOffset"),
    (0x0004_1510, Vr::UI, "ReferencedSOPClassUIDInFile"),
    (0x0004_1511, Vr::UI, "ReferencedSOPInstanceUIDInFile"),
    (0x0004_1512, Vr::UI, "ReferencedTransferSyntaxUIDInFile"),
    (
        0x0004_151A,
        Vr::UI,
        "ReferencedRelatedGeneralSOPClassUIDInFile",
    ),
    (0x0004_1600, Vr::UL, "NumberOfReferences"),
];

/// The keyword of `tag` ("PatientName"), if the dictionary has it.
pub fn keyword(tag: u32) -> Option<&'static str> {
    lookup(tag).map(|(_, k)| k)
}

/// The VR an implicit-VR data set implies for `tag`: the dictionary's, else group lengths
/// (`gggg,0000`) are UL and everything else UN.
pub(crate) fn implicit_vr(tag: u32) -> Vr {
    if tag & 0xFFFF == 0 {
        return Vr::UL;
    }
    match lookup(tag) {
        Some((vr, _)) => vr,
        // Pixel data of unknown provenance (overlay data in old files): OW.
        None => Vr::UN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted_and_finds_common_tags() {
        let e = crate::dict_table::ENTRIES;
        assert!(e.windows(2).all(|w| w[0].0 < w[1].0));
        assert_eq!(lookup(0x0028_0010), Some((Vr::US, "Rows")));
        assert_eq!(lookup(0x7FE0_0010).map(|x| x.1), Some("PixelData"));
        assert_eq!(lookup(0x0088_0200), Some((Vr::SQ, "IconImageSequence")));
        assert_eq!(lookup(0x0028_1201).map(|x| x.0), Some(Vr::OW));
        assert_eq!(implicit_vr(0x0009_0010), Vr::UN);
        assert_eq!(implicit_vr(0x0011_0000), Vr::UL);
        assert!(DIRECTORY.windows(2).all(|w| w[0].0 < w[1].0));
        assert_eq!(
            lookup(0x0004_1220),
            Some((Vr::SQ, "DirectoryRecordSequence"))
        );
    }
}
