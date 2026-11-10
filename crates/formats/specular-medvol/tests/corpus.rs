//! The fetched corpus (tools/corpus/fetch-medvol.sh: nibabel's and pynrrd's test volumes)
//! against the oracle written beside it. Reads SPECULAR_MEDVOL_CORPUS, else
//! tools/corpus/medvol; skips when neither has an expected.tsv or expected-parrec.tsv.

use std::path::{Path, PathBuf};

use specular_medvol::open_path;

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_MEDVOL_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/medvol")
        });
    (dir.join("expected.tsv").is_file() || dir.join("expected-parrec.tsv").is_file()).then_some(dir)
}

fn close(a: f64, b: f64, what: &str) {
    if a.is_nan() && b.is_nan() {
        return;
    }
    let tol = 1e-5 * b.abs().max(1.0);
    assert!((a - b).abs() <= tol, "{what}: {a} vs {b}");
}

#[test]
fn corpus_matches_the_oracle() {
    let Some(dir) = corpus() else {
        eprintln!("medvol corpus absent: run tools/corpus/fetch-medvol.sh");
        return;
    };
    let mut text = std::fs::read_to_string(dir.join("expected.tsv")).unwrap_or_default();
    // nibabel's PAR/REC and ECAT samples (section src_parrec), when fetched.
    if let Ok(t) = std::fs::read_to_string(dir.join("expected-parrec.tsv")) {
        text += &t;
    }
    // The file's header, and its voxels unless only the header is checked (a PAR header
    // whose REC data is not published).
    let mut current: Option<(
        String,
        specular_medvol::Header,
        Option<specular_medvol::MedVolume>,
    )> = None;
    let (mut files, mut headers, mut values, mut points, mut refused) = (0, 0, 0, 0, 0);
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let f: Vec<&str> = line.split('\t').collect();
        let n = |i: usize| f[i].parse::<usize>().unwrap();
        let x = |i: usize| f[i].parse::<f64>().unwrap();
        match f[0] {
            "file" => {
                let v = open_path(&dir.join(f[1])).unwrap_or_else(|e| panic!("{}: {e}", f[1]));
                assert_eq!(v.header.kind.name(), f[2], "{}", f[1]);
                assert_eq!(v.header.dtype.name(), f[3], "{}", f[1]);
                assert_eq!(v.header.orientation.key(), f[4], "{}", f[1]);
                assert_eq!(v.volumes(), n(5), "{}", f[1]);
                current = Some((f[1].to_owned(), v.header.clone(), Some(v)));
                files += 1;
            }
            "head" => {
                let path = dir.join(f[1]);
                let data = std::fs::read(&path).unwrap();
                let h = specular_medvol::read_header(&data, &path)
                    .unwrap_or_else(|e| panic!("{}: {e}", f[1]));
                assert_eq!(h.kind.name(), f[2], "{}", f[1]);
                assert_eq!(h.orientation.key(), f[3], "{}", f[1]);
                let dims: Vec<String> = h.dims.iter().map(|d| d.to_string()).collect();
                assert_eq!(dims.join("x"), f[4], "{}", f[1]);
                current = Some((f[1].to_owned(), h, None));
                headers += 1;
            }
            "geom" | "val" => {
                let (name, h, v) = current.as_ref().expect("a file row first");
                assert_eq!(name, f[1]);
                if f[0] == "geom" {
                    let p = h.to_patient([x(2), x(3), x(4)]);
                    for (a, &got) in p.iter().enumerate() {
                        close(got, x(5 + a), &format!("{name}: geometry {line}"));
                    }
                    points += 1;
                } else {
                    let v = v.as_ref().expect("values of a file row");
                    let got = v.value(n(2), n(3), n(4), n(5), n(6));
                    close(got, x(7), &format!("{name}: {line}"));
                    values += 1;
                }
            }
            "error" => {
                let e = open_path(&dir.join(f[1])).expect_err(f[1]).to_string();
                let words = f.get(2).copied().unwrap_or("");
                assert!(e.contains(words), "{}: {e:?} lacks {words:?}", f[1]);
                refused += 1;
            }
            "skip" => {}
            t => panic!("unknown row {t}"),
        }
    }
    eprintln!(
        "medvol corpus: {files} volumes, {headers} headers, {values} values, {points} points, \
         {refused} refused"
    );
    assert!(files > 0);
}
