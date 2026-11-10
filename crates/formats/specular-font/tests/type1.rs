//! Type 1 fonts rebuilt as OpenType CFF, against fontTools' reading of the same fonts
//! (tools/corpus/type1-oracle.py): every glyph's advance and the box of its outline points.
//! Run with `SPECULAR_TYPE1_CORPUS=<folder> cargo test -p specular-font --test type1 --
//! --ignored --nocapture`.

use std::collections::HashMap;
use std::path::Path;

#[test]
#[ignore]
fn matches_fonttools() {
    let Some(root) = std::env::var_os("SPECULAR_TYPE1_CORPUS") else {
        eprintln!("SPECULAR_TYPE1_CORPUS not set");
        return;
    };
    let root = Path::new(&root);
    let mut fonts: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "pfb" || e == "pfa"))
        .collect();
    fonts.sort();
    let (mut total, mut bad) = (0, 0);
    for p in fonts {
        let bytes = std::fs::read(&p).unwrap();
        let t0 = std::time::Instant::now();
        let file = specular_font::open(&bytes, Default::default()).unwrap();
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        let face = &file.faces[0];
        let stem = p.file_stem().unwrap().to_string_lossy().into_owned();
        let oracle = std::fs::read_to_string(root.join(format!("{stem}.pfb.glyphs"))).unwrap();
        let mut ours = HashMap::new();
        for gid in 0..face.glyph_count {
            let name = face.glyph_name(gid).unwrap_or_default();
            let adv = face.advance(gid).unwrap_or(0);
            let bbox = face.outline(gid).map(|o| o.bbox);
            ours.insert(name, (adv, bbox));
        }
        let (mut n, mut wrong) = (0, 0);
        for line in oracle.lines() {
            let f: Vec<&str> = line.split(' ').collect();
            let name = f[0];
            n += 1;
            let Some((adv, bbox)) = ours.get(name) else {
                wrong += 1;
                eprintln!("  {stem}: no glyph {name}");
                continue;
            };
            let w: f32 = f[1].parse().unwrap();
            let ok_adv = (*adv as f32 - w.round()).abs() <= 1.0;
            let ok_box = match (f.get(2), bbox) {
                (Some(&"-"), None) => true,
                (Some(&"-"), Some(_)) | (_, None) => false,
                (_, Some(b)) => f[2..6]
                    .iter()
                    .zip(b)
                    .all(|(t, o)| (t.parse::<f32>().unwrap() - o).abs() <= 0.51),
            };
            if !(ok_adv && ok_box) {
                wrong += 1;
                if wrong <= 3 {
                    eprintln!("  {stem}: {name}: ours {adv} {bbox:?}, fontTools {line}");
                }
            }
        }
        eprintln!(
            "{stem}: {n} glyphs, {wrong} mismatches, {} mapped characters, {ms:.1} ms",
            face.chars().len()
        );
        total += n;
        bad += wrong;
    }
    eprintln!("Type 1: {bad} mismatches in {total} glyphs");
    assert_eq!(bad, 0);
}
