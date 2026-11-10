//! Specific Character Set against pydicom: every SH / LO / ST / LT / UC / UT / PN value of
//! pydicom's character set files (`tools/corpus/fetch-dicom.sh src_pydicom_charset`, read from
//! `SPECULAR_DICOM_CORPUS`, else `tools/corpus/dicom`; skipped when absent) decodes to what
//! pydicom gives (tests/data/charset_oracle.tsv, written by gen/charset_oracle.py).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use specular_dicom::dict::Vr;
use specular_dicom::{Charset, DataSet, Dicom, Values};

fn corpus() -> Option<PathBuf> {
    let p = std::env::var_os("SPECULAR_DICOM_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/dicom")
        });
    let p = p.join("pydicom/src/pydicom/data/charset_files");
    p.is_dir().then_some(p)
}

fn norm(text: &str, vr: Vr) -> String {
    let parts: Vec<&str> = if matches!(vr, Vr::LT | Vr::ST | Vr::UT) {
        vec![text]
    } else {
        text.split('\\').collect()
    };
    let parts: Vec<&str> = parts
        .into_iter()
        .map(|p| p.trim_matches([' ', '\0']))
        .map(|p| {
            if vr == Vr::PN {
                p.trim_end_matches(['^', '='])
            } else {
                p
            }
        })
        .collect();
    parts.join("\\")
}

fn walk(ds: &DataSet, v: &Values, cs: Charset, prefix: &str, out: &mut BTreeMap<String, String>) {
    let cs = cs.within(ds, v);
    for e in &ds.elements {
        let path = format!("{prefix}{:08X}", e.tag);
        if e.vr == Vr::SQ {
            for (i, item) in e.items.iter().enumerate() {
                walk(item, v, cs, &format!("{path}[{i}]/"), out);
            }
            continue;
        }
        if !matches!(
            e.vr,
            Vr::SH | Vr::LO | Vr::ST | Vr::LT | Vr::UC | Vr::UT | Vr::PN
        ) {
            continue;
        }
        out.insert(path, norm(&cs.decode(v.text_bytes(e), e.vr), e.vr));
    }
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some(c) => out.push(c),
            None => {}
        }
    }
    out
}

#[test]
fn pydicom_charset_files() {
    let Some(dir) = corpus() else {
        eprintln!("charset corpus absent; skipped");
        return;
    };
    let oracle = include_str!("data/charset_oracle.tsv");
    let mut want: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for line in oracle.lines().filter(|l| !l.starts_with('#')) {
        let mut f = line.splitn(3, '\t');
        let (Some(file), Some(path), Some(value)) = (f.next(), f.next(), f.next()) else {
            panic!("bad oracle line {line:?}");
        };
        want.entry(file.to_owned())
            .or_default()
            .insert(path.to_owned(), unescape(value));
    }
    assert!(want.len() >= 17, "{} files in the oracle", want.len());
    let mut checked = 0;
    for (file, values) in &want {
        let data = std::fs::read(dir.join(file)).unwrap();
        let d = Dicom::parse(&data).unwrap();
        let v = d.values(&data);
        let mut got = BTreeMap::new();
        walk(&d.dataset, &v, Charset::default(), "", &mut got);
        for (path, value) in values {
            assert_eq!(got.get(path), Some(value), "{file} {path}");
            checked += 1;
        }
    }
    eprintln!("{checked} values match pydicom");
}
