//! The DICOM video / encapsulated document corpus (`tools/corpus/fetch-dcmobj.sh`): read in
//! place from `SPECULAR_DCMOBJ_CORPUS`, else `tools/corpus/dcmobj` of this checkout; skipped
//! when absent. Every file either decodes its first and last frames, holds a document, or is
//! refused as unsupported. Pixel comparisons against PyAV live in `gen/dcmobj_oracle.py`.

use std::path::{Path, PathBuf};

use specular_dicom::{Dicom, Error};

fn corpus() -> Option<PathBuf> {
    let p = std::env::var_os("SPECULAR_DCMOBJ_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/dcmobj")
        });
    p.is_dir().then_some(p)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|x| x == "dcm") {
            out.push(p);
        }
    }
}

#[test]
fn corpus_files_decode_or_are_refused() {
    let Some(dir) = corpus() else {
        eprintln!("dcmobj corpus absent; skipped");
        return;
    };
    let mut files = Vec::new();
    walk(&dir, &mut files);
    files.sort();
    let (mut videos, mut documents, mut refused) = (0, 0, 0);
    for f in &files {
        let name = f.file_name().unwrap().to_string_lossy().into_owned();
        let data = std::fs::read(f).unwrap();
        let d = Dicom::parse(&data).unwrap_or_else(|e| panic!("{name}: {e}"));
        if d.pixel.is_none() {
            let doc = d
                .document(&data)
                .unwrap_or_else(|| panic!("{name}: no document"));
            assert!(!doc.range.is_empty(), "{name}");
            documents += 1;
            continue;
        }
        match d.check_supported() {
            Err(Error::Unsupported(m)) => {
                assert!(
                    name.starts_with("refused") || name.starts_with("ETIAM"),
                    "{name}: {m}"
                );
                refused += 1;
                continue;
            }
            Err(e) => panic!("{name}: {e}"),
            Ok(()) => {}
        }
        assert!(!name.starts_with("refused"), "{name} decoded");
        let n = d.frame_count();
        assert!(n > 1, "{name}");
        for i in [0, n - 1] {
            let fr = d
                .decode_frame(&data, i, None)
                .unwrap_or_else(|e| panic!("{name} {i}: {e}"));
            assert!(fr.width > 0 && fr.height > 0);
        }
        videos += 1;
    }
    eprintln!("{videos} videos, {documents} documents, {refused} refused");
    assert!(videos + documents + refused == files.len());
}
