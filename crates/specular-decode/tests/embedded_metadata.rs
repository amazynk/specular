//! XMP and ICC profiles found without decoding pixels.
//!
//! The `locate` fuzz target's synthetic seeds (tools/fuzz/seeds.py `embedded`) must each hold
//! what they are meant to exercise, so the fuzzer starts from every parse path.
//!
//! `Session::embedded_icc` of JPEG XL, which codes its ICC profile in the codestream where
//! `embedded_metadata` cannot see it: checked against the conformance suite's reference
//! profiles. The reference is the space the expected pixels are in, which for XYB-coded
//! cases is not the embedded profile, so every embedded profile must be well-formed and some
//! must match their reference exactly. Skipped when the (gitignored) corpus is absent;
//! `SPECULAR_CONFORMANCE_DIR` overrides its root (`tools/corpus/conformance`), e.g. to share
//! one copy between worktrees.

use std::path::{Path, PathBuf};

use specular_decode::Registry;

fn corpus() -> Option<PathBuf> {
    let root = std::env::var_os("SPECULAR_CONFORMANCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/conformance")
        });
    let dir = root.join("jxl-conformance/testcases");
    dir.is_dir().then_some(dir)
}

#[test]
fn jxl_embedded_icc_is_the_stored_profile() {
    let Some(dir) = corpus() else {
        eprintln!("skipped: no JPEG XL conformance corpus");
        return;
    };
    let reg = Registry::new();
    let (mut embedded, mut matching) = (0, 0);
    let mut cases: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    cases.sort();
    for case in cases {
        let (Ok(reference), true) = (
            std::fs::read(case.join("reference.icc")),
            case.join("input.jxl").is_file(),
        ) else {
            continue;
        };
        let Ok(session) = reg.open_path(&case.join("input.jxl")) else {
            continue;
        };
        // Found from headers alone it would be in embedded_metadata; JPEG XL never is.
        let bytes = std::fs::read(case.join("input.jxl")).unwrap();
        assert_eq!(
            specular_decode::embedded_metadata(&bytes).icc,
            None,
            "{case:?}"
        );
        if let Some(icc) = session.embedded_icc() {
            assert!(icc.len() >= 132, "{case:?}");
            assert_eq!(
                u32::from_be_bytes(icc[..4].try_into().unwrap()) as usize,
                icc.len(),
                "{case:?}"
            );
            assert_eq!(&icc[36..40], b"acsp", "{case:?}");
            embedded += 1;
            matching += usize::from(icc == &reference[..]);
        }
    }
    eprintln!("{embedded} cases with an embedded ICC profile, {matching} equal to the reference");
    assert!(
        matching > 0,
        "no conformance case's embedded ICC profile is its reference"
    );
}

#[test]
fn fuzz_seeds_reach_every_location() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fuzz/seeds");
    let mut names = Vec::new();
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        let name = e.file_name().into_string().unwrap();
        let bytes = std::fs::read(e.path()).unwrap();
        let found = specular_decode::embedded_metadata(&bytes);
        let xmp = found.xmp.expect(&name);
        assert!(
            xmp.starts_with(b"<?xpacket") && xmp.ends_with(b"?>"),
            "{name}"
        );
        // JPEG XL codes its profile in the codestream; the plain PNG and the CR3 have none.
        let has_icc =
            !name.ends_with(".jxl") && !name.ends_with(".cr3") && name != "meta_plain.png";
        let icc = found
            .icc
            .filter(|i| &i[36..40] == b"acsp" && i[..4] == (i.len() as u32).to_be_bytes());
        assert_eq!(icc.is_some(), has_icc, "{name}");
        // PSD/PSB carry an Exif resource; the CR3 its CMT blocks, CMT4 with a position.
        if name.ends_with(".psd") || name.ends_with(".psb") || name.ends_with(".cr3") {
            let exif = specular_decode::exif_tiff(&bytes).expect(&name);
            let (camera, _) = specular_decode::exif_facts(&exif).expect(&name);
            assert!(!camera.camera.is_empty(), "{name}");
            assert_eq!(
                specular_decode::exif_position(&exif).is_some(),
                name.ends_with(".cr3"),
                "{name}"
            );
        }
        // The CR3's lens is in its Canon maker note only; its serial number is dropped.
        if name.ends_with(".cr3") {
            let exif = specular_decode::exif_tiff(&bytes).unwrap();
            let (camera, _) = specular_decode::exif_facts(&exif).unwrap();
            assert_eq!(camera.lens.as_deref(), Some("Seed 24-105mm"));
            assert!(!exif.windows(4).any(|w| w == 1234u32.to_le_bytes()));
        }
        // A Canon maker note before the IFDs is rebuilt, its lens kept and its serial dropped.
        if name.ends_with(".cr2") {
            let exif = specular_decode::exif_tiff(&bytes).expect(&name);
            let (camera, _) = specular_decode::exif_facts(&exif).unwrap();
            assert_eq!(camera.lens.as_deref(), Some("EF-S17-85mm"));
            assert!(!exif.windows(4).any(|w| w == 1234u32.to_le_bytes()));
        }
        // Compressed `application/exif` items, and a BigTIFF's Exif.
        if name.starts_with("meta_exif_") {
            let exif = specular_decode::exif_tiff(&bytes).expect(&name);
            let (camera, _) = specular_decode::exif_facts(&exif).expect(&name);
            assert_eq!(camera.camera, "Seed Cam", "{name}");
        }
        names.push(name);
    }
    names.sort();
    assert_eq!(
        names,
        [
            "meta.cr3",
            "meta.heic",
            "meta.jxl",
            "meta.png",
            "meta.psb",
            "meta.psd",
            "meta.webp",
            "meta_be.tif",
            "meta_canon_note.cr2",
            "meta_exif_big.tif",
            "meta_exif_gzip.avif",
            "meta_exif_zlib.heic",
            "meta_gzip.heic",
            "meta_idat.avif",
            "meta_ident.heic",
            "meta_le.tif",
            "meta_plain.png",
            "meta_raw.avif",
            "meta_zlib.heic",
        ]
    );
}
