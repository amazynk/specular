//! Fuzz movie and sound-file parsing through `parse_movie` (every container it sniffs; seeded
//! for ASF/DVR-MS, QCP and 3GPP/3GPP2 speech, raw APV and elementary streams), the track
//! extras, sample reads and the sound decoder of the chosen track (QCELP-13 and EVRC among
//! them). Run from this directory with `cargo +nightly fuzz run movie corpus/movie --
//! -dict=fuzz.dict`.
#![no_main]

use std::sync::Arc;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = specular_video::extras::read(data);
    let Ok(m) = specular_video::parse_movie(data, true) else {
        return;
    };
    let _ = m.duration_us();
    if let Some(v) = &m.video {
        for s in v.samples.iter().take(64) {
            let _ = v.sample_data(data, s);
        }
    }
    if let Some(a) = &m.audio {
        for s in a.samples.iter().take(64) {
            let _ = a.sample_data(data, s);
        }
        if a.samples.len() <= 256 {
            let source = Arc::new(specular_core::Source::from_vec("fuzz", data.to_vec()));
            let _ = specular_video::audio::decode_track(a, &source);
        }
    }
});
