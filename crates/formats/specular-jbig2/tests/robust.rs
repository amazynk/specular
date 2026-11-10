//! Cut and damaged files never panic, and cut files still give a page.

use std::path::Path;

fn fixtures() -> Vec<(String, Vec<u8>)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "jbig2"))
        .map(|p| {
            (
                p.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read(p).unwrap(),
            )
        })
        .collect();
    v.sort();
    v
}

#[test]
fn truncations_do_not_panic() {
    for (name, d) in fixtures() {
        for cut in (0..d.len()).step_by((d.len() / 40).max(1)) {
            let part = &d[..cut];
            let _ = specular_jbig2::info(part, 0);
            if let Ok(img) = specular_jbig2::decode(part, 0) {
                assert!(img.width > 0 && img.height > 0, "{name} cut at {cut}");
            }
        }
    }
}

#[test]
fn byte_damage_does_not_panic() {
    // A fixed pseudo-random sequence (no fuzzing): flip bytes and decode.
    let mut s = 0x9E37_79B9u32;
    let mut next = || {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        s
    };
    for (name, d) in fixtures() {
        for _ in 0..60 {
            let mut c = d.clone();
            for _ in 0..3 {
                let i = next() as usize % c.len();
                c[i] ^= (next() & 0xFF) as u8 | 1;
            }
            let _ = specular_jbig2::info(&c, 0);
            if let Ok(img) = specular_jbig2::decode(&c, 0) {
                assert!(img.width > 0 && img.height > 0, "{name}");
            }
        }
    }
}

#[test]
fn info_facts() {
    let d =
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/annex-h.jbig2"))
            .unwrap();
    let i = specular_jbig2::info(&d, 0).unwrap();
    assert_eq!(i.model, specular_jbig2::Model::Bilevel);
    assert_eq!(i.bit_depth, 1);
    assert!(i.pages >= 1);
    assert!(i.coding.starts_with("JBIG2"), "{}", i.coding);
    assert!(i.fields.iter().any(|(k, _)| k == "Organisation"));
    // Annex H holds three pages; each decodes at its own size.
    assert_eq!(i.pages, 3);
    for p in 0..3 {
        let pi = specular_jbig2::info(&d, p).unwrap();
        let img = specular_jbig2::decode(&d, p).unwrap();
        assert_eq!((img.width, img.height), (pi.width, pi.height), "page {p}");
    }
    assert!(specular_jbig2::decode(&d, 3).is_err());
}
