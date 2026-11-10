//! MOBI 6, joint MOBI/KF8, AZW3 and FB2 books (tests/data, made by gen_ebooks.sh with
//! Calibre's ebook-convert) rewritten as EPUB: their metadata against ebook-meta's reading,
//! their words against ebook-convert's plain text, their contents and cover, and pages.

// Only the picture stub is used here.
#[allow(unused_imports)]
mod common;

use std::path::PathBuf;
use std::sync::Arc;

use common::Stub;
use specular_epub::{Book, Options, PageSetup};

fn data(name: &str) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(name),
    )
    .unwrap()
}

fn open(name: &str) -> Book {
    let d = data(name);
    if specular_epub::is_mobi(&d) {
        Book::open_mobi(&d, &Options::default()).unwrap()
    } else {
        Book::open_fb2(&d, &Options::default()).unwrap()
    }
}

const BOOKS: &[(&str, &str)] = &[
    ("ebook-mobi6.mobi", "ebook-mobi6"),
    ("ebook-joint.mobi", "ebook-joint"),
    ("ebook-kf8.azw3", "ebook-kf8"),
    ("ebook.fb2", "ebook"),
    ("ebook-zip.fb2.zip", "ebook"),
];

/// The words of a text, lower-cased.
fn words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric() && c != '-')
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// The text of every page of the book, markup dropped.
fn book_text(b: &Book) -> String {
    let mut out = String::new();
    for p in b.spine() {
        let x = String::from_utf8_lossy(&b.read(p).unwrap().into_vec()).into_owned();
        let body = x.find("<body").map_or(&x[..], |i| &x[i..]);
        let mut in_tag = false;
        for c in body.chars() {
            match c {
                '<' => in_tag = true,
                '>' => {
                    in_tag = false;
                    out.push(' ');
                }
                c if !in_tag => out.push(c),
                _ => {}
            }
        }
        out.push('\n');
    }
    out.replace("&amp;", "&")
}

#[test]
fn metadata_matches_calibre() {
    let mut checked = 0;
    for &(file, stem) in BOOKS {
        let b = open(file);
        let m = b.metadata();
        let oracle = String::from_utf8(data(&format!("{stem}.meta"))).unwrap();
        for line in oracle.lines() {
            let (k, v) = line.split_once(": ").unwrap();
            checked += 1;
            match k {
                "Title" => assert_eq!(m.title.as_deref(), Some(v), "{file}"),
                "Author(s)" => {
                    let names: Vec<&str> = m.creators.iter().map(|c| c.name.as_str()).collect();
                    assert_eq!(names.join(" & "), v, "{file}");
                }
                "Publisher" => assert_eq!(m.publisher.as_deref(), Some(v), "{file}"),
                // Calibre names languages in ISO 639-2 ("eng"); the books say "en".
                "Languages" => {
                    let l = m.languages.first().map(String::as_str).unwrap_or("");
                    assert!(
                        v.starts_with(&l[..2.min(l.len())]) && !l.is_empty(),
                        "{file}: {l} vs {v}"
                    );
                }
                _ => {}
            }
        }
    }
    assert!(checked >= 3 * BOOKS.len(), "{checked}");
}

#[test]
fn words_match_calibre() {
    // Calibre's own pages (its title page, contents page) are not in the books.
    const CALIBRE: &[&str] = &["title", "page", "cover", "table", "of", "contents"];
    for &(file, stem) in BOOKS {
        let b = open(file);
        let ours = words(&book_text(&b));
        let want = words(&String::from_utf8(data(&format!("{stem}.txt"))).unwrap());
        let missing: Vec<&String> = want
            .iter()
            .filter(|w| !CALIBRE.contains(&w.as_str()) && !ours.contains(w))
            // Calibre's text runs some contents entries together ("StormThe Lamp").
            .filter(|w| {
                !(1..w.len()).any(|i| {
                    w.is_char_boundary(i)
                        && ours.contains(&w[..i].to_owned())
                        && ours.contains(&w[i..].to_owned())
                })
            })
            .collect();
        eprintln!(
            "{file}: {} of {} words found",
            want.len() - missing.len(),
            want.len()
        );
        assert!(missing.is_empty(), "{file}: missing {missing:?}");
        // In order: the chapters' first words come in reading order.
        let pos = |w: &str| ours.iter().position(|x| x == w).unwrap();
        assert!(
            pos("boat") < pos("wind") && pos("wind") < pos("gulls"),
            "{file}"
        );
    }
}

#[test]
fn contents_cover_and_pages() {
    for &(file, _) in BOOKS {
        let b = open(file);
        let toc: Vec<&str> = b.toc().iter().map(|t| t.title.as_str()).collect();
        for want in ["Arrival", "Storm", "The Lamp", "Morning"] {
            assert!(toc.contains(&want), "{file}: {toc:?}");
        }
        let (cover, mt) = b.cover().unwrap_or_else(|| panic!("{file}: no cover"));
        assert_eq!(mt, "image/jpeg", "{file}");
        assert!(cover.starts_with(&[0xFF, 0xD8]), "{file}");
        let pages = b
            .paginate(&PageSetup::default(), Arc::new(Stub), &|| false)
            .unwrap();
        assert!(pages.len() >= 4, "{file}: {} pages", pages.len());
        // Every contents entry lands on a page.
        for i in 0..b.toc().len() {
            assert!(
                pages.toc_page(i).is_some(),
                "{file}: entry {i} ({:?})",
                b.toc()[i].title
            );
        }
    }
}

#[test]
fn damaged_books_never_panic() {
    for &(file, _) in BOOKS {
        let d = data(file);
        for cut in (0..d.len()).step_by(97) {
            let part = &d[..cut];
            let _ = Book::open_mobi(part, &Options::default());
            let _ = Book::open_fb2(part, &Options::default());
        }
        let mut flipped = d.clone();
        for i in (0..flipped.len()).step_by(53) {
            flipped[i] ^= 0x5A;
        }
        let _ = Book::open_mobi(&flipped, &Options::default());
        let _ = Book::open_fb2(&flipped, &Options::default());
    }
}
