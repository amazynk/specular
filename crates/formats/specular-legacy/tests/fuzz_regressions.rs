//! Inputs found by `fuzz/` that once panicked. Each must decode (or fail) without panicking.

/// An XPM colour `#` value holding a multi-byte UTF-8 character: splitting it into three
/// equal byte runs cut the character in half.
#[test]
fn xpm_non_ascii_hex_colour() {
    let data = "/* XPM */\nstatic char *a[] = {\n\"2 1 2 1\",\n\"a c #\u{20ac}\",\n\"b c #00ff00\",\n\"ab\"\n};\n";
    if let Ok(img) = specular_legacy::decode(data.as_bytes()) {
        assert_eq!((img.width, img.height), (2, 1));
    }
}
