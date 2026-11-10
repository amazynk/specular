//! Damaged fonts: every fixture cut short at many lengths and with bytes flipped opens or
//! fails with a reason, and whatever opens reads its facts and outlines without panicking.

use std::path::Path;

use specular_font::{Limits, open};

fn exercise(data: &[u8]) {
    let limits = Limits {
        max_sfnt_bytes: 8 << 20,
        max_table_bytes: 4 << 20,
    };
    if let Ok(f) = open(data, limits) {
        for face in &f.faces {
            let o = face.outliner();
            for g in 0..face.glyph_count.min(64) {
                let _ = o.outline(g);
                let _ = face.advance(g);
                let _ = face.glyph_name(g);
            }
            let _ = specular_font::preview(face, 16, 16);
        }
    }
}

#[test]
fn damaged_fixtures_never_panic() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut runs = 0;
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_none_or(|x| x == "py") || p.is_dir() {
            continue;
        }
        let data = std::fs::read(&p).unwrap();
        for cut in (0..data.len()).step_by(data.len() / 40 + 1) {
            exercise(&data[..cut]);
            runs += 1;
        }
        for _ in 0..60 {
            let mut d = data.clone();
            for _ in 0..1 + next() % 8 {
                let i = next() as usize % d.len();
                d[i] = next() as u8;
            }
            exercise(&d);
            runs += 1;
        }
    }
    assert!(runs > 1000);
    assert!(open(b"", Limits::default()).is_err());
    assert!(open(&[0u8; 64], Limits::default()).is_err());
}
