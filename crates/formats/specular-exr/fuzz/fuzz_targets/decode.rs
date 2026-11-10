//! Fuzz the OpenEXR header parser (single and multi-part, tiled, mip/rip levels), the offset
//! tables, every supported compression (RLE, ZIPS, ZIP, PIZ, PXR24, B44, B44A), channel
//! selection and luminance/chroma conversion. Run from this directory with
//! `cargo +nightly fuzz run decode corpus/decode -- -dict=fuzz.dict`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_exr::{
    decode_with_info, probe, read_info, ChannelSelect, DecodeOptions, LevelSelect, Limits, Pixels,
    SampleFormat,
};

fuzz_target!(|data: &[u8]| {
    let _ = probe(data);
    let Ok(info) = read_info(data) else { return };
    let limits = Limits {
        max_width: 1 << 12,
        max_height: 1 << 12,
        max_pixels: 1 << 18,
        ..Limits::default()
    };
    let parallel = data.len() % 2 == 1;
    for (pi, part) in info.parts.iter().enumerate().take(3) {
        let _ = (
            part.width(),
            part.height(),
            part.tile_desc(),
            part.is_deep(),
        );
        let last = part.levels.len().saturating_sub(1);
        let names: Vec<String> = part.channels.iter().map(|c| c.name.clone()).collect();
        let layer = part.channels.first().map(|c| c.layer().to_string());
        let mut runs = vec![
            (LevelSelect::Full, ChannelSelect::Auto, SampleFormat::F32),
            (
                LevelSelect::Index(last),
                ChannelSelect::Auto,
                SampleFormat::F16,
            ),
            (
                LevelSelect::MinSize {
                    width: 3,
                    height: 2,
                },
                ChannelSelect::Slots([
                    names.get(1).cloned(),
                    names.first().cloned(),
                    None,
                    names.last().cloned(),
                ]),
                SampleFormat::F32,
            ),
        ];
        if let Some(l) = layer {
            runs.push((
                LevelSelect::Index(1),
                ChannelSelect::Layer(l),
                SampleFormat::F16,
            ));
        }
        for (level, channels, format) in runs {
            let opts = DecodeOptions {
                part: pi,
                level,
                channels,
                format,
                limits,
                parallel,
            };
            if let Ok(img) = decode_with_info(data, &info, &opts) {
                let n = img.width as usize * img.height as usize * 4;
                let len = match &img.pixels {
                    Pixels::F32(v) => v.len(),
                    Pixels::F16(v) => v.len(),
                };
                assert_eq!(len, n, "pixel buffer is not width × height × 4");
            }
        }
    }
});
