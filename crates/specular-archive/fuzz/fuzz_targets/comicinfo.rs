#![no_main]
//! The ComicInfo.xml parser on its own.

use libfuzzer_sys::fuzz_target;
use specular_archive::ComicInfo;

fuzz_target!(|data: &[u8]| {
    let _ = ComicInfo::parse(data);
});
