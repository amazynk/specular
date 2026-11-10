#![no_main]
//! The `specular://` URL parser fed arbitrary bytes: nothing panics, and every plainly local
//! result is a list of absolute paths (see `url_scheme::fuzz_one`).
//!
//! Run: `cargo +nightly fuzz run url_scheme fuzz/seeds` in `crates/specular-platform`.

#[allow(dead_code)]
#[path = "../../src/url_scheme.rs"]
mod url_scheme;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    url_scheme::fuzz_one(data);
});
