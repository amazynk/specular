//! Inputs found by fuzzing (`fuzz/`), each built here as a small synthetic file.

use specular_video::wtv;

/// The WTV directory-entry GUID (9174B792-7059-4470-88DF-063B82CC213D) as stored.
const ENTRY: [u8; 16] = [
    0x92, 0xB7, 0x74, 0x91, 0x59, 0x70, 0x70, 0x44, 0x88, 0xDF, 0x06, 0x3B, 0x82, 0xCC, 0x21, 0x3D,
];

/// A directory entry whose length and name length point past the end of the file: the name
/// was sliced without a bounds check (an index panic).
#[test]
fn wtv_entry_name_past_the_end() {
    // The root directory at sector 0 (word at 0x38): the entry is at the start of the file.
    let mut d = ENTRY.to_vec();
    d.extend_from_slice(&1000u64.to_le_bytes()); // entry length
    d.extend_from_slice(&16u64.to_le_bytes()); // file size
    d.extend_from_slice(&100u32.to_le_bytes()); // name length (UTF-16 units)
    d.resize(0x40, 0);
    assert!(wtv::audio_tracks(&d).is_empty());
    assert!(wtv::parse_with(&d, true, None).is_err());
}
