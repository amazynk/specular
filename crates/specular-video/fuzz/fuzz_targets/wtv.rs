//! Fuzz the WTV demuxer: the sector file system (resident, sector and chunk files, one and two
//! levels of sector lists), the timeline's packets (stream descriptors and their DirectShow
//! media types, timestamps, data), the programme attributes, sample gathering across packets
//! and units, each sound track by id, and the sound decoders behind them. Run from this
//! directory with `cargo +nightly fuzz run wtv corpus/wtv -- -dict=fuzz.dict`.
#![no_main]

use std::sync::Arc;

use libfuzzer_sys::fuzz_target;
use specular_video::wtv;

fuzz_target!(|data: &[u8]| {
    let _ = wtv::sniff(data);
    let tracks = wtv::audio_tracks(data);
    let _ = specular_video::extras::read(data);
    let mut ids: Vec<Option<u64>> = vec![None];
    ids.extend(tracks.iter().take(4).map(|t| Some(t.id)));
    let source = Arc::new(specular_core::Source::from_vec("fuzz.wtv", data.to_vec()));
    for id in ids {
        let Ok(m) = wtv::parse_with(data, true, id) else {
            continue;
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
                let _ = specular_video::audio::decode_track(a, &source);
            }
        }
    }
});
