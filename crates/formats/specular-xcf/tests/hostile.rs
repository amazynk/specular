//! Hostile inputs built in place: each must come back as an error or a sane result, without
//! a large allocation or a panic.

/// An xz stream whose block declares a 4 GiB LZMA2 dictionary but holds 5 bytes. The reader
/// allocates and zeroes its dictionary up front, so the dictionary is capped by what the
/// block can produce.
#[test]
fn xz_4gib_dictionary() {
    let mut f = vec![0xfd, b'7', b'z', b'X', b'Z', 0, 0, 0, 0, 0, 0, 0];
    // Block header: size byte (2 → 12 bytes), flags (one filter), LZMA2 with dictionary
    // bits 40, padding, CRC (not checked).
    f.extend([2, 0, 0x21, 1, 40, 0, 0, 0, 0, 0, 0, 0]);
    // LZMA2: an uncompressed chunk resetting the dictionary, then the end marker.
    f.extend([1, 0, 4]);
    f.extend(b"hello");
    f.push(0);
    while f.len() % 4 != 0 {
        f.push(0);
    }
    // Index: indicator, one record (unpadded size, uncompressed size), padding, CRC.
    f.extend([0, 1, 21, 5, 0, 0, 0, 0]);
    // Footer.
    f.extend([0; 12]);
    let out = specular_xcf::decompress(&f).unwrap().unwrap();
    assert_eq!(out, b"hello");
}
