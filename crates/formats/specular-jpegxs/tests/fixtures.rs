//! Committed fixtures (64×32 synthetic images encoded by the ISO/IEC 21122-5 reference
//! software across profiles by `tools/corpus/jpegxs-oracle.py synth --small`; their
//! `manifest.tsv` holds the SHA-256 of the reference decoder's samples); header facts; the
//! JPEG XS file wrapper; damaged and cut codestreams.

use sha2::{Digest, Sha256};
use specular_jpegxs::file::{self, Cicp};
use specular_jpegxs::{
    Error, Model, Pixels, Sequence, codestream, decode, decode_samples, info, probe,
};
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn read(name: &str) -> Vec<u8> {
    std::fs::read(dir().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// (name without `.jxs`, sample count, SHA-256, encoder configuration).
fn expected() -> Vec<(String, usize, String, String)> {
    std::fs::read_to_string(dir().join("manifest.tsv"))
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            assert_eq!(c[1], "ok", "{l}");
            (
                c[0].trim_end_matches(".jxs").to_owned(),
                c[2].parse().unwrap(),
                c[3].to_owned(),
                c[4].to_owned(),
            )
        })
        .collect()
}

#[test]
fn every_fixture_matches_the_reference_decoder() {
    let all = expected();
    assert!(all.len() >= 40);
    let mut bad = Vec::new();
    for (name, n, hash, cfg) in &all {
        let data = read(&format!("{name}.jxs"));
        let s = decode_samples(&data).unwrap_or_else(|e| panic!("{name} ({cfg}): {e}"));
        let all: Vec<u8> = s
            .planes
            .iter()
            .flat_map(|p| p.data.iter().flat_map(|v| v.to_le_bytes()))
            .collect();
        assert!(!s.truncated, "{name}");
        if all.len() / 2 != *n || format!("{:x}", Sha256::digest(&all)) != *hash {
            bad.push(format!("{name} ({cfg})"));
        }
    }
    assert!(bad.is_empty(), "differ from the reference decoder: {bad:?}");
}

#[test]
fn every_fixture_has_facts_and_pixels() {
    for (name, ..) in expected() {
        let data = read(&format!("{name}.jxs"));
        assert!(probe(&data), "{name}");
        let i = info(&data, 0).unwrap_or_else(|e| panic!("{name}: {e}"));
        let img = decode(&data, 0).unwrap();
        assert_eq!((img.width, img.height), (i.width, i.height), "{name}");
        let n = (i.width * i.height) as usize;
        let len = match &img.pixels {
            Pixels::Gray8(v) => v.len(),
            Pixels::Gray16(v) => v.len(),
            Pixels::Rgba8(v) => v.len() / 4,
            Pixels::Rgba16(v) => v.len() / 4,
            Pixels::RgbaF32(v) => v.len() / 4,
        };
        assert_eq!(len, n, "{name}");
        assert!(i.coding.starts_with("JPEG XS "), "{name}: {}", i.coding);
        for key in [
            "profile",
            "level",
            "decomposition",
            "colour-transform",
            "compression",
        ] {
            assert!(i.summary.iter().any(|(k, _)| *k == key), "{name}: {key}");
        }
    }
}

fn summary(i: &specular_jpegxs::Info, key: &str) -> String {
    i.summary
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

#[test]
fn facts_name_profile_sampling_and_transforms() {
    let i = info(&read("main422-10.jxs"), 0).unwrap();
    assert_eq!(summary(&i, "profile"), "Main 422.10");
    assert_eq!(summary(&i, "decomposition"), "5 horizontal, 1 vertical");
    assert_eq!(summary(&i, "colour-transform"), "None");
    assert_eq!(i.model, Model::YCbCr);
    assert_eq!(i.bit_depth, 10);
    assert!(i.coding.contains("10-bit 4:2:2"), "{}", i.coding);

    let i = info(&read("main444-12-rct.jxs"), 0).unwrap();
    assert_eq!(
        summary(&i, "colour-transform"),
        "Reversible colour transform (RCT)"
    );
    assert_eq!(i.model, Model::Rgb);

    let i = info(&read("mainbayer-grbg.jxs"), 0).unwrap();
    assert_eq!(summary(&i, "profile"), "Main Bayer");
    assert_eq!(summary(&i, "colour-transform"), "Star-Tetrix, GRBG mosaic");
    // The mosaic is the picture: twice the component size.
    assert_eq!((i.width, i.height), (64, 32));
    let s = decode_samples(&read("mainbayer-grbg.jxs")).unwrap();
    assert_eq!(s.planes.len(), 1);
    assert_eq!((s.planes[0].width, s.planes[0].height), (64, 32));

    let i = info(&read("light-subline422-10.jxs"), 0).unwrap();
    assert_eq!(summary(&i, "decomposition"), "5 horizontal, 0 vertical");

    let i = info(&read("mls12.jxs"), 0).unwrap();
    assert!(summary(&i, "compression").contains("lossless"));

    let i = info(&read("high4444-12.jxs"), 0).unwrap();
    assert!(i.has_alpha);

    let i = info(&read("u444-quadratic.jxs"), 0).unwrap();
    assert!(
        i.fields
            .iter()
            .any(|(k, v)| k == "Non-linear transform" && v.starts_with("Quadratic")),
        "{:?}",
        i.fields
    );

    let i = info(&read("tdc444-12.jxs"), 0).unwrap();
    assert_eq!(summary(&i, "profile"), "TDC 444.12");
}

#[test]
fn lossless_profiles_round_trip_exactly() {
    // MLS: the reference software's decoded samples are the encoder's input; checked through
    // the hashes above, here only that nothing was cut.
    for name in ["mls12", "mls12-422", "mls12-bw16"] {
        let s = decode_samples(&read(&format!("{name}.jxs"))).unwrap();
        assert!(!s.truncated);
    }
}

#[test]
fn image_file_wraps_the_codestream_and_states_colour() {
    let f = read("main422-10-file.jxs");
    assert!(probe(&f));
    let (cs, fi) = codestream(&f).unwrap();
    assert_eq!(cs, read("main422-10.jxs").as_slice());
    let fi = fi.unwrap();
    assert_eq!(&fi.brand, b"jxs ");
    assert_eq!(fi.profile_level.map(|p| p.0), Some(0x3540));
    assert_eq!(
        fi.cicp,
        Some(Cicp {
            primaries: 1,
            transfer: 1,
            matrix: 1,
            full_range: false
        })
    );
    let i = info(&f, 0).unwrap();
    assert!(i.fields.iter().any(|(k, _)| k == "Colour (CICP)"));
    assert_eq!(
        decode(&f, 0).unwrap(),
        decode(&read("main422-10.jxs"), 0).unwrap()
    );

    // A full-range RGB statement makes a 4:4:4 codestream RGB, an identity matrix YCbCr.
    let cs = read("high444-12-none.jxs");
    let rgb = file::wrap(
        &cs,
        Cicp {
            primaries: 1,
            transfer: 13,
            matrix: 0,
            full_range: true,
        },
        None,
    );
    assert_eq!(info(&rgb, 0).unwrap().model, Model::Rgb);
    let ycc = file::wrap(
        &cs,
        Cicp {
            primaries: 1,
            transfer: 1,
            matrix: 1,
            full_range: false,
        },
        Some((0x4A40, 0)),
    );
    assert_eq!(info(&ycc, 0).unwrap().model, Model::YCbCr);
    assert_eq!(codestream(&ycc).unwrap().0, cs.as_slice());
}

/// The HEIF fixtures are written by `tools/corpus/jpegxs-wrap.py heif` (an independent writer
/// of the boxes): a plain `jxs1` primary item; one in two extents with `clap`, `irot` and
/// `imir`; one kept in `idat`.
#[test]
fn heif_jxs1_items_decode_like_their_codestreams() {
    let plain = read("main422-10.jxsi");
    assert!(probe(&plain));
    assert!(probe(&plain[..300.min(plain.len())]));
    let (cs, fi) = codestream(&plain).unwrap();
    assert_eq!(cs, read("main422-10.jxs").as_slice());
    assert!(matches!(cs, std::borrow::Cow::Borrowed(_)));
    let fi = fi.unwrap();
    assert_eq!(&fi.brand, b"mif1");
    assert_eq!(fi.profile_level.map(|p| p.0), Some(0x3540));
    assert_eq!(fi.cicp.map(|c| c.matrix), Some(1));
    assert_eq!(
        decode(&plain, 0).unwrap(),
        decode(&read("main422-10.jxs"), 0).unwrap()
    );
    let i = info(&plain, 0).unwrap();
    assert_eq!((i.width, i.height, i.orientation), (64, 32, 1));
    assert!(
        i.fields
            .iter()
            .any(|(k, v)| k == "Container" && v.contains("jxs1"))
    );

    let idat = read("main420-12-idat.jxsi");
    assert!(probe(&idat));
    assert_eq!(
        decode(&idat, 0).unwrap(),
        decode(&read("main420-12.jxs"), 0).unwrap()
    );

    // Two extents (copied together), full-range RGB, a centred 48×24 clean aperture, a
    // quarter turn anti-clockwise then a left-right mirror: EXIF 7, applied by the viewer.
    let turned = read("high444-12-turned.jxsi");
    let (cs, _) = codestream(&turned).unwrap();
    assert!(matches!(cs, std::borrow::Cow::Owned(_)));
    assert_eq!(cs, read("high444-12-none.jxs").as_slice());
    let i = info(&turned, 0).unwrap();
    assert_eq!((i.width, i.height, i.orientation), (48, 24, 7));
    let full = decode(&read("high444-12-none.jxs"), 0).unwrap();
    let cut = decode(&turned, 0).unwrap();
    assert_eq!((cut.width, cut.height), (48, 24));
    let (Pixels::Rgba16(a), Pixels::Rgba16(b)) = (&full.pixels, &cut.pixels) else {
        panic!("16-bit RGBA expected");
    };
    for y in 0..24 {
        assert_eq!(
            &b[y * 48 * 4..(y + 1) * 48 * 4],
            &a[((y + 4) * 64 + 8) * 4..((y + 4) * 64 + 56) * 4]
        );
    }

    // Not JPEG XS: another primary item type, or no meta box in sight.
    let mut other = plain.clone();
    let at = other.windows(4).position(|w| w == b"jxs1").unwrap();
    other[at..at + 4].copy_from_slice(b"hvc1");
    assert!(!probe(&other));
    assert!(matches!(codestream(&other), Err(Error::NotRecognised)));
    assert!(!probe(&plain[..40]));
    // Cut anywhere: an error or a truncated picture, never a panic.
    for n in (0..plain.len()).step_by(7) {
        let _ = decode(&plain[..n], 0);
        let _ = info(&plain[..n], 0);
    }
}

#[test]
fn files_without_the_brand_are_refused_in_words() {
    let mut f = read("main422-10-file.jxs");
    // ftyp brand and compatibility list → "abcd".
    for at in [20, 28] {
        f[at..at + 4].copy_from_slice(b"abcd");
    }
    assert!(matches!(codestream(&f), Err(Error::Unsupported(_))));
}

#[test]
fn cut_and_damaged_codestreams_never_panic() {
    for name in [
        "main422-10",
        "high420-12",
        "u444-signs",
        "mainbayer-grbg",
        "tdc444-12",
        "u422-cw1",
    ] {
        let data = read(&format!("{name}.jxs"));
        let full = decode_samples(&data).unwrap();
        // Cut inside the slices: what was read stays, the rest is flat.
        for cut in [data.len() * 3 / 4, data.len() / 2, 200, 60] {
            match decode_samples(&data[..cut]) {
                Ok(s) => {
                    assert!(s.truncated, "{name} cut at {cut}");
                    assert_eq!(s.planes.len(), full.planes.len());
                }
                Err(Error::Truncated(_) | Error::Malformed(_)) => {}
                Err(e) => panic!("{name} cut at {cut}: {e}"),
            }
        }
        // Flipped bytes past the header.
        let mut x = 0x9E37_79B9u32;
        for _ in 0..40 {
            let mut d = data.clone();
            for _ in 0..8 {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                let at = 60 + x as usize % (d.len() - 60);
                d[at] ^= (x >> 24) as u8 | 1;
            }
            let _ = decode_samples(&d);
            let _ = decode(&d, 0);
        }
    }
}

#[test]
fn not_jpeg_xs_is_not_recognised() {
    assert!(!probe(b"\xFF\xD8\xFF\xE0"));
    assert_eq!(info(b"hello", 0).unwrap_err(), Error::NotRecognised);
    assert!(info(&read("main422-10.jxs"), 1).is_err());
}

#[test]
fn a_sequence_decodes_frames_in_turn() {
    let data = read("tdc444-12.jxs");
    let first = decode_samples(&data).unwrap();
    let mut seq = Sequence::new();
    assert_eq!(seq.decode(&data).unwrap(), first);
    // The same intra-coded frame again predicts from the first: same picture.
    assert_eq!(seq.decode(&data).unwrap().planes.len(), first.planes.len());
    // An intra-only codestream through a sequence is the still.
    let s = read("main422-10.jxs");
    assert_eq!(seq.decode(&s).unwrap(), decode_samples(&s).unwrap());
}

#[test]
fn decode_reserves_from_the_budget() {
    let data = read("main422-10.jxs");
    let r = specular_core::budget::with_limit(4096, || decode_samples(&data));
    assert!(matches!(r, Err(Error::NeedsMemory(_))), "{:?}", r.err());
    let b = std::sync::Arc::new(specular_core::budget::MemoryBudget::fixed(1 << 30));
    specular_core::budget::with_budget(b.clone(), || decode_samples(&data)).unwrap();
    assert_eq!(b.reserved(), 0);
}
