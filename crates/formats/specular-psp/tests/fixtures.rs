//! Committed fixtures from Mike Lischke's GraphicEx test suite (MPL-1.1; see
//! tools/corpus/fetch-editors-psp.sh): PSP 5, 6 and 7 files. The pixel hashes are of renders
//! that match GIMP 3's importer exactly (the corpus test checks that on the full set).

use specular_psp as sp;

fn fnv(d: &[u8]) -> u64 {
    d.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    })
}

fn rgba(img: &sp::Image) -> Vec<u8> {
    match img.format {
        sp::PixelFormat::Gray8 => img.data.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        _ => img.data.clone(),
    }
}

/// (file, major, size, bits, layers, composite stored, hash of the layers composited)
type Fixture = (&'static str, u16, (u32, u32), u16, usize, bool, u64);
const FIXTURES: &[Fixture] = &[
    (
        "palette4-rle-v4.psp",
        4,
        (286, 191),
        4,
        1,
        false,
        0x8581030892b93685,
    ),
    (
        "bw1-lz77-v4.psp",
        4,
        (286, 191),
        1,
        1,
        false,
        0x50508eb9e0ff36ac,
    ),
    (
        "palette8-raw-v3.psp",
        3,
        (286, 191),
        8,
        1,
        false,
        0x88c3e6681048bc54,
    ),
    (
        "grey8-raw-v4.psp",
        4,
        (286, 191),
        8,
        1,
        false,
        0xfddcb47dacb2080d,
    ),
    ("gold-v5.tub", 5, (57, 57), 24, 1, true, 0x0da0200a4370ff0d),
    (
        "rgb-lz77-v5.psp",
        5,
        (20, 20),
        24,
        1,
        false,
        0x5f1460a7f01d57e5,
    ),
    (
        "layers3-rle-v3.psp",
        3,
        (180, 325),
        24,
        3,
        false,
        0xa952c1a9191efa90,
    ),
];

#[test]
fn fixtures_decode_as_gimp_does() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let mut wrong = Vec::new();
    for &(name, major, size, bits, layers, composite, hash) in FIXTURES {
        let data = std::fs::read(dir.join(name)).unwrap();
        assert!(sp::probe(&data));
        let p = sp::parse(&data).unwrap();
        let i = p.info();
        assert_eq!(
            (
                i.major,
                (i.width, i.height),
                i.bit_depth,
                i.layer_count,
                i.has_composite
            ),
            (major, size, bits, layers, composite),
            "{name}"
        );
        assert!(i.has_thumbnail, "{name}");
        let opts = sp::Options {
            prefer_composite: false,
            ..Default::default()
        };
        let img = p.render(&data, &opts).unwrap();
        assert!(!img.truncated, "{name}");
        let h = fnv(&rgba(&img));
        if h != hash {
            wrong.push(format!("(\"{name}\", {h:#x})"));
        }
        if composite {
            let stored = p.render(&data, &sp::Options::default()).unwrap();
            assert_eq!(rgba(&stored), rgba(&img), "{name}: own composite");
        }
        let t = p.thumbnail(&data).unwrap().unwrap();
        assert!(t.width > 0 && t.width <= i.width.max(200), "{name}");
    }
    assert!(wrong.is_empty(), "hashes: {}", wrong.join(", "));
}
