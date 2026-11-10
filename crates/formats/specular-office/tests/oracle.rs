//! The office corpus (tools/corpus/gen-office.py) against its oracle: core properties through
//! python-docx, openpyxl and python-pptx, meta.xml through odfpy, summary information through
//! olefile, extended properties and thumbnails through zipfile/ElementTree. Reads
//! SPECULAR_OFFICE_CORPUS, else tools/corpus/office; skips when absent.

use std::path::{Path, PathBuf};

use serde_json::Value;
use sha2::{Digest, Sha256};
use specular_office::{Error, Properties, app_of_extension, open};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_OFFICE_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/office")
        });
    dir.join("oracle.json").is_file().then_some(dir)
}

/// Oracle readings known to be wrong, with why.
const ORACLE_WRONG: &[(&str, &str, &str)] = &[(
    "real/SampleSS.strict.xlsx",
    "sheets",
    "openpyxl does not read Strict OOXML namespaces and lists no sheets",
)];

/// "2024-05-06T07:08:09Z", "2024-05-06T07:08:09.123" → "2024-05-06T07:08:09"; the FILETIME
/// zero olefile shows as 1601 is no date.
fn date(s: &str) -> Option<String> {
    let s = s.trim().trim_end_matches('Z');
    (!s.starts_with("1601")).then(|| s.chars().take(19).collect())
}

fn ours(p: &Properties, key: &str) -> Option<String> {
    let n = |v: Option<u64>| v.map(|v| v.to_string());
    match key {
        "title" => p.title.clone(),
        "author" => p.author.clone(),
        "subject" => p.subject.clone(),
        "keywords" => p.keywords.clone(),
        "last_modified_by" => p.last_modified_by.clone(),
        "company" => p.company.clone(),
        "created" => p.created.as_deref().and_then(date),
        "modified" => p.modified.as_deref().and_then(date),
        "pages" => n(p.pages),
        "words" => n(p.words),
        "characters" => n(p.characters),
        "slides" => n(p.slides),
        "notes" => n(p.notes),
        "hidden_slides" => n(p.hidden_slides),
        "sheets" => n(p.sheets),
        _ => None,
    }
}

#[test]
fn office_corpus_against_the_oracle() {
    let Some(dir) = corpus() else {
        eprintln!("no office corpus; run tools/corpus/gen-office.py");
        return;
    };
    let oracle: serde_json::Map<String, Value> =
        serde_json::from_slice(&std::fs::read(dir.join("oracle.json")).unwrap()).unwrap();
    let (mut files, mut checks, mut previews, mut refused) = (0, 0, 0, 0);
    let mut bad = Vec::new();
    for (rel, want) in &oracle {
        let path = dir.join(rel);
        let data = std::fs::read(&path).unwrap();
        let ext = rel.rsplit('.').next().unwrap_or("");
        let is_odf = matches!(ext, "odt" | "ods" | "odp" | "odg");
        files += 1;
        if want.get("oracle_error").is_some() {
            bad.push(format!(
                "{rel}: the oracle failed: {}",
                want["oracle_error"]
            ));
            continue;
        }
        let t0 = std::time::Instant::now();
        let (props, preview) = if is_odf {
            let zip = specular_ora::Zip::parse(&data).unwrap();
            let meta = zip.find("meta.xml").unwrap();
            let meta = zip.read(&data, meta, 1 << 20).unwrap();
            let thumb = zip
                .find("Thumbnails/thumbnail.png")
                .map(|e| zip.read(&data, e, 1 << 24).unwrap().into_owned());
            (specular_office::odf_properties(&meta), thumb)
        } else {
            match open(&data, app_of_extension(ext)) {
                Ok(d) => {
                    let pv = d.preview.as_ref().map(|p| p.bytes.get(&data).to_vec());
                    (d.props, pv)
                }
                Err(Error::Encrypted(what)) => {
                    refused += 1;
                    if want.get("encrypted") != Some(&Value::Bool(true)) {
                        bad.push(format!("{rel}: refused as password-protected {what}"));
                    }
                    continue;
                }
                Err(e) => {
                    bad.push(format!("{rel}: {e}"));
                    continue;
                }
            }
        };
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        if want.get("encrypted").is_some() {
            bad.push(format!("{rel}: opened, but it is password-protected"));
            continue;
        }
        for (key, v) in want.as_object().unwrap() {
            let expect = match v {
                Value::Null => None,
                Value::String(s) if key == "created" || key == "modified" => date(s),
                Value::String(s) => Some(s.clone()),
                Value::Number(n) => Some(n.to_string()),
                _ => continue,
            };
            match key.as_str() {
                "preview_sha256" => {
                    checks += 1;
                    let got = preview.as_ref().map(|b| format!("{:x}", Sha256::digest(b)));
                    previews += usize::from(got.is_some());
                    if got != expect {
                        bad.push(format!("{rel}: preview {got:?}, oracle {expect:?}"));
                    }
                }
                "clip" => {
                    // olefile's VT_CF value: the format tag, the clipboard format, the data.
                    checks += 1;
                    let clip: Option<Vec<u8>> = expect.map(|h| {
                        (0..h.len() / 2)
                            .map(|i| u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).unwrap())
                            .collect()
                    });
                    let ok = match (&clip, &preview) {
                        (None, None) => true,
                        (Some(c), Some(p)) => {
                            let cf = u32::from_le_bytes(c[4..8].try_into().unwrap());
                            let data = &c[8..];
                            match cf {
                                // CF_DIB: a BMP file header in front.
                                8 => p.starts_with(b"BM") && &p[14..] == data,
                                // CF_METAFILEPICT: the WMF after its 8-byte header.
                                3 => data.ends_with(p) && data.len() - p.len() == 8,
                                _ => p == data,
                            }
                        }
                        _ => false,
                    };
                    if !ok {
                        bad.push(format!(
                            "{rel}: thumbnail does not match the clipboard data"
                        ));
                    } else {
                        previews += usize::from(preview.is_some());
                    }
                }
                "slides_counted" => {}
                k => {
                    if ORACLE_WRONG.iter().any(|(f, kk, _)| *f == rel && *kk == k) {
                        continue;
                    }
                    checks += 1;
                    let got = ours(&props, k);
                    if got != expect {
                        bad.push(format!("{rel}: {k} {got:?}, oracle {expect:?}"));
                    }
                }
            }
        }
        eprintln!("{rel}: {ms:.2} ms");
    }
    eprintln!(
        "office oracle: {files} files, {checks} checks, {previews} previews, {refused} refused as \
         password-protected, {} mismatches",
        bad.len()
    );
    assert!(bad.is_empty(), "{bad:#?}");
    assert!(files > 30);
}
