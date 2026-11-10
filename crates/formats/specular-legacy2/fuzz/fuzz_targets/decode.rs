//! Fuzz the specular-legacy2 decoders: Aseprite sprites (every frame, layers, cels, tilemaps,
//! palettes), GIMP brushes, brush pipes, patterns and gradients, Photoshop and Adobe
//! swatches and colour tables, GIMP palettes, Microsoft Paint, GD/GD2, Pixar, McIDAS AREA,
//! PGX, and the older XWD, TIM, MIFF, LabEye, IM Tools and XV thumbnails. The first byte
//! picks the decoder (a probe-only pass always runs), the rest is the file. Run from this
//! directory with `cargo +nightly fuzz run decode corpus/decode -- -dict=fuzz.dict`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_legacy2::{
    aco, act, ase, aseprite, gd, ggr, gimp, gpl, imt, labeye, mcidas, miff, msp, pgx, pxr, tim,
    xvthumb, xwd, Image, Info, Pixels, Result,
};

type Dec = (
    fn(&[u8]) -> bool,
    fn(&[u8], u32) -> Result<Info>,
    fn(&[u8], u32) -> Result<Image>,
);

const DECODERS: [Dec; 21] = [
    (aseprite::probe, aseprite::info, aseprite::decode),
    (gimp::gbr::probe, gimp::gbr::info, gimp::gbr::decode),
    (gimp::pat::probe, gimp::pat::info, gimp::pat::decode),
    (gimp::gih::probe, gimp::gih::info, gimp::gih::decode),
    (ggr::probe, ggr::info, ggr::decode),
    (aco::probe, aco::info, aco::decode),
    (act::probe, act::info, act::decode),
    (ase::probe, ase::info, ase::decode),
    (gpl::probe, gpl::info, gpl::decode),
    (msp::probe, msp::info, msp::decode),
    (gd::image::probe, gd::image::info, gd::image::decode),
    (gd::gd2::probe, gd::gd2::info, gd::gd2::decode),
    (pxr::probe, pxr::info, pxr::decode),
    (mcidas::probe, mcidas::info, mcidas::decode),
    (pgx::probe, pgx::info, pgx::decode),
    (xwd::probe, xwd::info, xwd::decode),
    (tim::probe, tim::info, tim::decode),
    (miff::probe, miff::info, miff::decode),
    (labeye::probe, labeye::info, labeye::decode),
    (imt::probe, imt::info, imt::decode),
    (xvthumb::probe, xvthumb::info, xvthumb::decode),
];

fn check(img: &Image) {
    let n = img.width as usize * img.height as usize;
    let len = match &img.pixels {
        Pixels::Rgba8(v) => v.len() / 4,
        Pixels::Rgba16(v) => v.len() / 4,
        Pixels::RgbaF32(v) => v.len() / 4,
        Pixels::Gray8(v) => v.len(),
        Pixels::Gray16(v) => v.len(),
    };
    assert!(len >= n, "pixel buffer shorter than width × height");
}

fuzz_target!(|data: &[u8]| {
    let Some((&pick, file)) = data.split_first() else {
        return;
    };
    for (probe, _, _) in DECODERS {
        let _ = probe(file);
    }
    let (_, info, decode) = DECODERS[usize::from(pick) % DECODERS.len()];
    let Ok(i) = info(file, 0) else {
        return;
    };
    // Keep each run cheap: large images decode the same way, slower.
    if u64::from(i.width) * u64::from(i.height) > 1 << 20 {
        return;
    }
    for page in 0..i.pages.min(3) {
        if let Ok(img) = decode(file, page) {
            check(&img);
        }
    }
    if pick >= 128 {
        if let Ok(s) = aseprite::parse(file) {
            let _ = s.info();
            if u64::from(s.width) * u64::from(s.height) <= 1 << 18 {
                for f in 0..s.frames.len().min(4) {
                    let px = s.render(f);
                    assert!(px.len() >= s.width as usize * s.height as usize * 4);
                }
            }
        }
    }
});
