//! Inputs found by the fuzzer (`fuzz/`), checked against the invariant they broke.

/// A bzip2 stream whose Huffman code lengths are over-subscribed (`fuzz-oversubscribed.bz2`, a
/// fuzzer input). Building the lookup table for it ran past the table's end; it must be an
/// error instead.
#[test]
fn bzip2_oversubscribed_code_is_an_error() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/fuzz-oversubscribed.bz2"
    );
    let d = std::fs::read(path).unwrap();
    assert!(specular_xcf::decompress(&d).is_err());
}
