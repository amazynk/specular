//! Fuzz Brotli decompression: no panics, the output cap holds, and a stream that decodes
//! under one cap decodes the same under a larger one. Run from this directory with
//! `cargo +nightly fuzz run decompress corpus/decompress`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_brotli::{decompress, Error};

fuzz_target!(|data: &[u8]| {
    // The first byte picks a cap, so small caps get exercised too.
    let Some((&sel, stream)) = data.split_first() else {
        return;
    };
    let cap = [0, 1, 100, 4096, 1 << 16, 1 << 20][usize::from(sel) % 6];
    match decompress(stream, cap) {
        Ok(out) => {
            assert!(out.len() <= cap);
            assert_eq!(decompress(stream, 1 << 22).as_ref(), Ok(&out));
        }
        Err(Error::TooLarge(c)) => assert_eq!(c, cap),
        Err(_) => {}
    }
});
