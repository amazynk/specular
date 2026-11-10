//! One-pass multi-scale decoding (`decode_scales`), deriving levels from a decoded image
//! (`downscale`) and the parallel (wavefront) unfilter: all must match plain decodes exactly.

mod common;

use common::*;
use specular_png::{
    DecodeOptions, Output, Parallelism, decode, decode_scales, downscale, set_wavefront_min,
};

fn opts(parallelism: Parallelism, scale: u32) -> DecodeOptions<'static> {
    DecodeOptions {
        output: Output::Auto8,
        scale,
        parallelism,
        ..Default::default()
    }
}

fn samples() -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for (ct, depth, interlace) in [
        (2u8, 8u8, false),
        (6, 8, false),
        (6, 16, false),
        (0, 8, false),
        (4, 8, false),
        (3, 4, false),
        (2, 8, true),
        (6, 8, true),
    ] {
        let s = make(173, 131, ct, depth, Content::Photo, 7);
        let extra = Extra {
            palette: (ct == 3).then(|| palette(depth, 3)),
            ..Default::default()
        };
        let data = encode(&s, interlace, Filters::Mixed, &extra, 9);
        out.push((format!("ct {ct} depth {depth} interlace {interlace}"), data));
    }
    out
}

#[test]
fn scales_match_single_decodes() {
    let sets: [&[u32]; 6] = [
        &[1, 2, 8],
        &[8, 4, 1],
        &[1, 8],
        &[3, 6, 1],
        &[2, 3],
        &[4, 4, 1],
    ];
    for (what, data) in samples() {
        // Whole files and two truncation points (inside the image data).
        for cut in [data.len(), data.len() * 2 / 3, data.len() / 3] {
            let data = &data[..cut];
            for par in [Parallelism::Single, Parallelism::Threads] {
                for set in sets {
                    let got = decode_scales(data, &opts(par, 1), set)
                        .unwrap_or_else(|e| panic!("{what} cut {cut} {set:?}: {e}"));
                    assert_eq!(got.len(), set.len());
                    for (img, &sc) in got.iter().zip(set) {
                        let want = decode(data, &opts(par, sc)).unwrap();
                        assert_eq!(*img, want, "{what} cut {cut} {par:?} {set:?} scale {sc}");
                    }
                }
            }
        }
    }
}

#[test]
fn downscale_matches_scaled_decode() {
    for (what, data) in samples() {
        let full = decode(&data, &opts(Parallelism::Single, 1)).unwrap();
        let planes: Vec<(&[u8], usize)> = full
            .planes
            .iter()
            .map(|p| (p.data.as_slice(), p.stride))
            .collect();
        for sc in [2u32, 4, 8, 3] {
            let want = decode(&data, &opts(Parallelism::Single, sc)).unwrap();
            for workers in [1, 4] {
                let got =
                    downscale(full.layout, full.width, full.height, &planes, sc, workers).unwrap();
                assert_eq!(got, want, "{what} scale {sc} workers {workers}");
            }
        }
    }
}

#[test]
fn wavefront_matches_serial() {
    // Rows wide enough for several column segments; every filter type, odd widths.
    set_wavefront_min(0, 0);
    for (w, h, ct, depth) in [
        (6001usize, 37usize, 2u8, 16u8),
        (4099, 29, 6, 16),
        (9001, 23, 2, 8),
        (20011, 17, 0, 8),
        (5003, 31, 6, 8),
    ] {
        let s = make(w, h, ct, depth, Content::Photo, 21);
        for filters in [
            Filters::Mixed,
            Filters::Fixed(1),
            Filters::Fixed(3),
            Filters::Fixed(4),
        ] {
            let data = encode(&s, false, filters, &Extra::default(), 4);
            let serial = decode(&data, &opts(Parallelism::Single, 1)).unwrap();
            let len = data.len();
            for cut in [len, len / 2] {
                let data = &data[..cut];
                let serial_cut = decode(data, &opts(Parallelism::Single, 1)).unwrap();
                if cut == len {
                    assert_eq!(serial_cut, serial);
                    assert!(!serial.truncated);
                }
                let par = decode(data, &opts(Parallelism::Threads, 1)).unwrap();
                assert_eq!(par, serial_cut, "{w}x{h} ct {ct} {filters:?} cut {cut}");
                let levels = decode_scales(data, &opts(Parallelism::Threads, 1), &[1, 4]).unwrap();
                assert_eq!(levels[0], serial_cut, "{w}x{h} ct {ct} levels");
                assert_eq!(
                    levels[1],
                    decode(data, &opts(Parallelism::Single, 4)).unwrap(),
                    "{w}x{h} ct {ct} levels 1/4"
                );
            }
        }
    }
}

/// PngSuite and the other conformance PNGs (fetched by `tools/corpus/fetch-conformance.sh`;
/// skipped when absent): the parallel unfilter and one-pass levels give the plain decode's
/// bytes, errors included.
#[test]
fn conformance_corpus() {
    let root = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tools/corpus/conformance"
    );
    let mut files = Vec::new();
    let mut dirs = vec![std::path::PathBuf::from(root)];
    while let Some(d) = dirs.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                dirs.push(p);
            } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("png")) {
                files.push(p);
            }
        }
    }
    if files.is_empty() {
        eprintln!("no conformance corpus at {root}; skipped");
        return;
    }
    set_wavefront_min(0, 0);
    let mut checked = 0;
    for p in &files {
        let data = std::fs::read(p).unwrap();
        // Keep the test quick: the corpus has a few very large files.
        if data.len() > 8 << 20 {
            continue;
        }
        let serial = decode(&data, &opts(Parallelism::Single, 1));
        let par = decode(&data, &opts(Parallelism::Threads, 1));
        assert_eq!(par.is_ok(), serial.is_ok(), "{}", p.display());
        let Ok(serial) = serial else { continue };
        assert_eq!(par.unwrap(), serial, "{}", p.display());
        let levels = decode_scales(&data, &opts(Parallelism::Threads, 1), &[1, 2, 8]).unwrap();
        assert_eq!(levels[0], serial, "{} levels", p.display());
        for (img, sc) in levels[1..].iter().zip([2, 8]) {
            let want = decode(&data, &opts(Parallelism::Single, sc)).unwrap();
            assert_eq!(*img, want, "{} 1/{sc}", p.display());
        }
        checked += 1;
    }
    eprintln!("{checked} conformance PNGs checked");
}
