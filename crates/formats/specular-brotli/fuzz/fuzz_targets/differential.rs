//! Differential fuzzing against the upstream decoder: whenever it decodes a stream (within
//! the cap), ours must decode it to the same bytes, or reject it for a reason RFC 7932 gives
//! that brotli-decompressor does not check (Google's C decoder rejects those too). Run from
//! this directory with `cargo +nightly fuzz run differential corpus/differential`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_brotli::Error;

const CAP: usize = 1 << 20;

/// Rules brotli-decompressor does not enforce.
fn stricter(e: &Error) -> bool {
    matches!(
        e,
        Error::Invalid(
            "insert length past the meta-block"
                | "copy length past the meta-block"
                | "dictionary word past the meta-block"
                // 0010001: the large-window extension, not RFC 7932.
                | "window size"
        )
    )
}

fuzz_target!(|data: &[u8]| {
    let ours = specular_brotli::decompress(data, CAP);
    let mut reference = Vec::new();
    let mut input = data;
    if brotli_decompressor::BrotliDecompress(&mut input, &mut reference).is_ok()
        && reference.len() <= CAP
    {
        match ours {
            Ok(out) => assert!(out == reference, "different output"),
            Err(e) => assert!(stricter(&e), "reference decoded it, we said {e}"),
        }
    }
});
