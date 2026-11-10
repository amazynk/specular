//! Real fonts (tools/corpus/fetch-fonts.sh) read against ttf-parser: every fact it reads
//! and every outline. Skipped when the corpus is not there.

mod common;

use std::path::Path;
use std::time::Instant;

use specular_font::{Limits, open};

#[test]
fn corpus_matches_the_oracle() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/fonts");
    let Ok(rd) = std::fs::read_dir(&dir) else {
        eprintln!("no font corpus: run tools/corpus/fetch-fonts.sh");
        return;
    };
    let mut files: Vec<_> = rd.filter_map(|e| Some(e.ok()?.path())).collect();
    files.sort();
    let mut total = common::Tally::default();
    for p in files {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let data = std::fs::read(&p).unwrap();
        let t0 = Instant::now();
        let f = match open(&data, Limits::default()) {
            Ok(f) => f,
            Err(e) => {
                total.mismatches += 1;
                total.notes.push(format!("{name}: {e}"));
                continue;
            }
        };
        let opened = t0.elapsed();
        let t1 = Instant::now();
        let mut glyphs = 0usize;
        for face in &f.faces {
            let o = face.outliner();
            for g in 0..face.glyph_count {
                let _ = o.outline(g);
            }
            glyphs += face.glyph_count as usize;
        }
        let outlined = t1.elapsed();
        let mut t = common::Tally::default();
        for (i, face) in f.faces.iter().enumerate() {
            common::compare(face, &format!("{name}#{i}"), &mut t);
        }
        // fontTools' decompression of a WOFF/WOFF2 (tools/corpus/font-oracle.py): every table
        // byte-equal but the rebuilt ones and the head fields fontTools rewrites.
        if let Some(o) = std::env::var_os("SPECULAR_FONT_ORACLE")
            && let Ok(sfnt) = std::fs::read(Path::new(&o).join(format!("{name}.sfnt")))
        {
            let theirs = open(&sfnt, Limits::default()).unwrap();
            let (a, b) = (&f.faces[0], &theirs.faces[0]);
            for r in &b.tables {
                let (x, y) = (a.table(&r.tag), b.table(&r.tag).unwrap());
                let same = match (&r.tag, x) {
                    (_, None) => false,
                    (b"glyf" | b"loca" | b"hmtx", Some(_)) => true,
                    (b"head", Some(x)) => {
                        x.len() == y.len()
                            && x[..8] == y[..8]
                            && x[12..28] == y[12..28]
                            && x[36..] == y[36..]
                    }
                    (_, Some(x)) => x == y,
                };
                t.check_pub(same, || format!("{name}: table {}", r.tag_str()));
            }
        }
        eprintln!(
            "{name}: {} faces, {glyphs} glyphs, open {:.1} ms, outlines {:.1} ms, {} checks, {} mismatches",
            f.faces.len(),
            opened.as_secs_f64() * 1e3,
            outlined.as_secs_f64() * 1e3,
            t.checked,
            t.mismatches
        );
        for n in t.notes.iter().take(5) {
            eprintln!("  {n}");
        }
        total.checked += t.checked;
        total.mismatches += t.mismatches;
        total.notes.extend(t.notes);
    }
    eprintln!(
        "corpus: {} checks, {} mismatches",
        total.checked, total.mismatches
    );
    assert_eq!(
        total.mismatches,
        0,
        "{:#?}",
        &total.notes[..total.notes.len().min(20)]
    );
}
