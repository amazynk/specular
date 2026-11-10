//! Every file of the OpenDocument corpus (tools/corpus/fetch-design2-odf.sh) opens, and its
//! pages render, without panicking; skipped when the corpus is absent.

use std::path::{Path, PathBuf};

use specular_drawpage::RenderRequest;

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_ODF_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/design2/odf")
        });
    dir.is_dir().then_some(dir)
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n != "oracle") {
                files(&p, out);
            }
        } else if p
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| ["odg", "fodg", "odp", "fodp", "ods", "odt"].contains(&e))
        {
            out.push(p);
        }
    }
}

#[test]
fn corpus_opens_and_renders() {
    let Some(dir) = corpus() else {
        eprintln!("OpenDocument corpus absent: skipped");
        return;
    };
    let mut all = Vec::new();
    files(&dir, &mut all);
    all.sort();
    let (mut drawn, mut thumbs) = (0, 0);
    for f in &all {
        let data = std::fs::read(f).unwrap();
        let kind = specular_odf::probe_file(&data)
            .unwrap_or_else(|| panic!("{}: not recognised", f.display()));
        if !matches!(
            kind,
            specular_odf::Kind::Graphics | specular_odf::Kind::Presentation
        ) {
            assert!(
                specular_odf::thumbnail(&data).is_ok(),
                "{}: no thumbnail",
                f.display()
            );
            thumbs += 1;
            continue;
        }
        let doc = specular_odf::open_graphics(&data, None)
            .unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        for (i, p) in doc.pages.iter().enumerate() {
            let s = 0.25;
            let w = (p.size.0 * s).ceil() as u32;
            let h = (p.size.1 * s).ceil() as u32;
            specular_drawpage::render(
                p,
                RenderRequest {
                    scale_x: s,
                    scale_y: s,
                    x: 0,
                    y: 0,
                    width: w,
                    height: h,
                },
                &|| false,
            )
            .unwrap_or_else(|e| panic!("{} page {i}: {e}", f.display()));
        }
        let _ = doc.describe(0);
        drawn += 1;
    }
    eprintln!("{drawn} drawings/presentations, {thumbs} thumbnails");
}
