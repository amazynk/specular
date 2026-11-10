//! Keywords in XMP against exiftool: what we write it reads the same, and what it writes we
//! read the same. Skipped (with a note) when exiftool is not installed.

use std::path::{Path, PathBuf};
use std::process::Command;

use specular_metadata::keywords::{self as kw, Vocabulary};

fn exiftool() -> bool {
    let ok = Command::new("exiftool").arg("-ver").output().is_ok();
    if !ok {
        eprintln!("exiftool not installed: the keyword oracle is skipped");
    }
    ok
}

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("specular-kw-oracle-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A list tag as exiftool reads it.
fn read_list(p: &Path, tag: &str) -> Vec<String> {
    let out = Command::new("exiftool")
        .args(["-s3", "-sep", " ;; ", "-charset", "UTF8"])
        .arg(format!("-{tag}"))
        .arg(p)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .trim_end()
        .split(" ;; ")
        .map(str::to_owned)
        .filter(|l| !l.is_empty())
        .collect()
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| (*x).to_owned()).collect()
}

#[test]
fn exiftool_reads_what_we_write() {
    if !exiftool() {
        return;
    }
    let d = dir("write");
    let mut v = Vocabulary::new();
    v.add("Places|France|Paris");
    v.set_synonyms("Places|France|Paris", &s(&["Lutetia"]));
    v.add("People|Ann & Bob <twins>");
    v.set_private("People", true);
    v.add("Hidden");
    v.set_private("Hidden", true);
    let keywords = s(&[
        "Places|France|Paris",
        "People|Ann & Bob <twins>",
        "Hidden",
        "Café",
    ]);
    // A new sidecar, and one of another tool's with a rating and a title.
    let other = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmp:Rating="2"><dc:title><rdf:Alt><rdf:li xml:lang="x-default">T</rdf:li></rdf:Alt></dc:title><dc:subject><rdf:Bag><rdf:li>old</rdf:li></rdf:Bag></dc:subject></rdf:Description></rdf:RDF></x:xmpmeta>"#;
    let mut mismatches = 0;
    for (name, existing) in [("new.xmp", None), ("other.xmp", Some(other))] {
        let p = d.join(name);
        std::fs::write(&p, kw::write_xmp(existing, &keywords, Some(&v))).unwrap();
        let subject = read_list(&p, "XMP-dc:Subject");
        let hier = read_list(&p, "XMP-lr:HierarchicalSubject");
        let want_subject = s(&[
            "Paris",
            "France",
            "Places",
            "Lutetia",
            "Ann & Bob <twins>",
            "Café",
        ]);
        if subject != want_subject {
            mismatches += 1;
            eprintln!("{name}: dc:subject {subject:?}");
        }
        if hier != keywords {
            mismatches += 1;
            eprintln!("{name}: lr:hierarchicalSubject {hier:?}");
        }
        if existing.is_some() {
            assert_eq!(read_list(&p, "XMP-xmp:Rating"), ["2"]);
            assert_eq!(read_list(&p, "XMP-dc:Title"), ["T"]);
        }
    }
    assert_eq!(mismatches, 0);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn we_read_what_exiftool_writes() {
    if !exiftool() {
        return;
    }
    let d = dir("read");
    let p = d.join("k.xmp");
    let st = Command::new("exiftool")
        .args([
            "-q",
            "-charset",
            "UTF8",
            "-XMP-dc:Subject=Paris",
            "-XMP-dc:Subject=France",
            "-XMP-dc:Subject=Places",
            "-XMP-dc:Subject=flat één",
            "-XMP-lr:HierarchicalSubject=Places|France|Paris",
            "-o",
        ])
        .arg(&p)
        .status()
        .unwrap();
    assert!(st.success());
    let text = std::fs::read_to_string(&p).unwrap();
    assert_eq!(
        kw::from_xmp_text(&text, &|_| false).unwrap(),
        s(&["Places|France|Paris", "flat één"])
    );
    // And our rewrite of exiftool's packet stays readable to it.
    let out = kw::write_xmp(Some(&text), &s(&["A|B"]), None);
    std::fs::write(&p, out).unwrap();
    assert_eq!(read_list(&p, "XMP-dc:Subject"), ["B", "A"]);
    assert_eq!(read_list(&p, "XMP-lr:HierarchicalSubject"), ["A|B"]);
    let _ = std::fs::remove_dir_all(&d);
}
