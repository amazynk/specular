//! The keywords of photos on disk, and the keyword list and recents in the config folder.
//!
//! A photo's keywords are its XMP sidecar's when the sidecar names any (an empty list too),
//! else those embedded in the file (XMP, else a JPEG's IPTC Keywords). Writing changes only
//! the sidecar (the image is never touched), found as the gallery's marks find it, so marks
//! and keywords share one sidecar; both writers take [`marks::SIDECAR_WRITE`].

use std::io;
use std::path::{Path, PathBuf};

use specular_i18n::t;
use specular_metadata::keywords::{self as kw, Change, Recents, Vocabulary};

use crate::app::gallery::marks::{self, Sidecars};

/// The keyword list (Lightroom keyword-list text) in the config folder.
pub(super) const LIST: &str = "keywords.txt";
/// Recently used keywords, one per line, the latest first.
pub(super) const RECENT: &str = "keywords-recent.txt";

/// The keywords of `image` now: its own sidecar's, else other apps' sidecars' (darktable's
/// own tags left out), else the embedded ones.
pub(in crate::app) fn read(image: &Path, sc: &Sidecars, vocab: &Vocabulary) -> Vec<String> {
    // Offline with a local copy: what the catalogue (and the changes waiting) say.
    if let Some(k) = crate::app::local_copies::offline_keywords(image) {
        return k;
    }
    let (side, exists) = sc.resolve(image);
    if exists
        && let Ok(text) = std::fs::read_to_string(&side)
        && specular_metadata::sidecars::owner(&text).is_none()
        && let Some(k) = kw::from_xmp_text(&text, &|n| vocab.is_synonym(n))
    {
        return k;
    }
    foreign(image, sc).unwrap_or_else(|| embedded(image))
}

/// The keywords other apps' sidecars of `image` name, the weightiest first.
fn foreign(image: &Path, sc: &Sidecars) -> Option<Vec<String>> {
    use specular_metadata::sidecars as sides;
    sc.foreign(image).into_iter().find_map(|p| {
        let kind = p.file_name()?.to_str().and_then(sides::Kind::of_name)?;
        let b = sides::read_small(&p)?;
        sides::read(kind, &b).values.keywords
    })
}

/// The keywords embedded in `image`.
fn embedded(image: &Path) -> Vec<String> {
    use std::io::Read;
    let Ok(f) = std::fs::File::open(image) else {
        return Vec::new();
    };
    if !f.metadata().is_ok_and(|m| m.is_file()) {
        return Vec::new();
    }
    let mut head = Vec::new();
    if f.take(kw::HEAD as u64).read_to_end(&mut head).is_err() {
        return Vec::new();
    }
    kw::from_file_head(&head)
}

/// The keywords of each of `images` (one folder scan per folder).
pub(super) fn read_all(images: &[PathBuf], vocab: &Vocabulary) -> Vec<(PathBuf, Vec<String>)> {
    let mut folders = std::collections::HashMap::<PathBuf, Sidecars>::new();
    images
        .iter()
        .map(|p| {
            let dir = p.parent().unwrap_or(Path::new("")).to_path_buf();
            let sc = folders.entry(dir).or_insert_with_key(|d| Sidecars::scan(d));
            (p.clone(), read(p, sc, vocab))
        })
        .collect()
}

/// Each image's keywords before and after a write, or why it failed.
pub(in crate::app) type Written = Vec<(PathBuf, Result<(Vec<String>, Vec<String>), String>)>;

/// The function that makes an image's keywords from its current ones.
type Edit<'a> = &'a dyn Fn(&[String]) -> Vec<String>;

/// Applies `changes` to each of `images` (read afresh, so changes made elsewhere since are
/// kept) and writes their sidecars.
pub(super) fn write_all(images: &[PathBuf], changes: &[Change], vocab: &Vocabulary) -> Written {
    let f = |cur: &[String]| kw::apply(cur, changes);
    let each: Vec<(&Path, Edit)> = images.iter().map(|p| (p.as_path(), &f as Edit)).collect();
    write_each(&each, vocab, None)
}

/// Renames keyword `from` (and those below it) to `to` in each of `images` that has it:
/// a rename, a move, or a merge onto a keyword that exists. `done` counts the photos gone
/// through. Images without it are left alone (nothing written).
pub(super) fn rename_all(
    images: &[PathBuf],
    from: &str,
    to: &str,
    vocab: &Vocabulary,
    done: &std::sync::atomic::AtomicUsize,
) -> Written {
    let f = |cur: &[String]| kw::rename_in(cur, from, to);
    let each: Vec<(&Path, Edit)> = images.iter().map(|p| (p.as_path(), &f as Edit)).collect();
    write_each(&each, vocab, Some(done))
}

/// Gives each image exactly its keywords of `lists` (Undo puts back what a write changed).
pub(in crate::app) fn restore_all(lists: &[(PathBuf, Vec<String>)], vocab: &Vocabulary) -> Written {
    let fs: Vec<_> = lists
        .iter()
        .map(|(_, l)| move |_: &[String]| l.clone())
        .collect();
    let each: Vec<(&Path, Edit)> = lists
        .iter()
        .zip(&fs)
        .map(|((p, _), f)| (p.as_path(), f as Edit))
        .collect();
    write_each(&each, vocab, None)
}

/// Writes each image's sidecar with its keywords made from the current ones by its function;
/// `done` counts the images gone through.
fn write_each(
    each: &[(&Path, Edit)],
    vocab: &Vocabulary,
    done: Option<&std::sync::atomic::AtomicUsize>,
) -> Written {
    let _one_at_a_time = marks::SIDECAR_WRITE
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut folders = std::collections::HashMap::<PathBuf, Sidecars>::new();
    let mut out = Vec::with_capacity(each.len());
    for (image, f) in each {
        let dir = image.parent().unwrap_or(Path::new("")).to_path_buf();
        let sc = folders.entry(dir).or_insert_with_key(|d| Sidecars::scan(d));
        let r = write_one(image, sc, *f, vocab);
        out.push((image.to_path_buf(), r.map_err(|e| e.to_string())));
        if let Some(d) = done {
            d.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
    out
}

fn write_one(
    image: &Path,
    sc: &mut Sidecars,
    f: Edit,
    vocab: &Vocabulary,
) -> io::Result<(Vec<String>, Vec<String>)> {
    // Offline: the change waits for the original (written to its sidecar when it is back).
    if let Some(current) = crate::app::local_copies::offline_keywords(image) {
        let after = f(&current);
        if after != current && !crate::app::local_copies::queue_keywords(image, &after) {
            return Err(io::Error::new(io::ErrorKind::NotFound, "the file is gone"));
        }
        return Ok((current, after));
    }
    if !image.is_file() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "the file is gone"));
    }
    // Never another app's sidecar: Specular's own, or a new one beside it.
    let Some((path, exists)) = sc.resolve_writable(image) else {
        return Err(io::Error::other(t!("sidecar-other-apps-only")));
    };
    let is_xmp = |p: &Path| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("xmp"));
    // Never anything but a sidecar, never the image itself.
    if !is_xmp(&path) || is_xmp(image) || path == image {
        return Err(io::Error::other("not a sidecar path"));
    }
    let existing = if exists {
        Some(std::fs::read_to_string(&path)?)
    } else {
        None
    };
    let current = existing
        .as_deref()
        .and_then(|t| kw::from_xmp_text(t, &|n| vocab.is_synonym(n)))
        .or_else(|| foreign(image, sc))
        .unwrap_or_else(|| embedded(image));
    let after = f(&current);
    if after == current {
        // Nothing to say that the file (or its sidecar) does not say already.
        return Ok((current, after));
    }
    let text = kw::write_xmp(existing.as_deref(), &after, Some(vocab));
    if existing.as_deref() != Some(text.as_str()) {
        marks::write_atomic(&path, text.as_bytes())?;
        sc.add(&path);
    }
    Ok((current, after))
}

/// The keyword list and recents saved in `dir`.
pub(super) fn load(dir: &Path) -> (Vocabulary, Recents) {
    let v = std::fs::read_to_string(dir.join(LIST))
        .map(|s| Vocabulary::from_lightroom(&s))
        .unwrap_or_default();
    let r = std::fs::read_to_string(dir.join(RECENT))
        .map(|s| Recents::from_text(&s))
        .unwrap_or_default();
    (v, r)
}

/// Saves the keyword list and recents into `dir`.
pub(super) fn save(dir: &Path, list: &str, recent: &str) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    marks::write_atomic(&dir.join(LIST), list.as_bytes())?;
    marks::write_atomic(&dir.join(RECENT), recent.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("specular-kw-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| (*x).to_owned()).collect()
    }

    /// A JPEG with keywords in its XMP; the sidecar takes over once written, keeps the marks
    /// written beside it, and a RAW+JPEG pair gets one sidecar each.
    #[test]
    fn sidecars_take_over_from_embedded_keywords() {
        let d = tmp("side");
        let xmp = "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\"><rdf:Description xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:subject><rdf:Bag><rdf:li>sea</rdf:li></rdf:Bag></dc:subject></rdf:Description></rdf:RDF></x:xmpmeta>";
        let mut app1 = b"http://ns.adobe.com/xap/1.0/\0".to_vec();
        app1.extend(xmp.as_bytes());
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE1];
        jpeg.extend(((app1.len() + 2) as u16).to_be_bytes());
        jpeg.extend(&app1);
        jpeg.extend([0xFF, 0xD9]);
        let a = d.join("A.jpg");
        std::fs::write(&a, &jpeg).unwrap();
        let v = Vocabulary::new();
        assert_eq!(read_all(std::slice::from_ref(&a), &v)[0].1, s(&["sea"]));
        // Marks first (as the gallery writes them), then keywords.
        let t = marks::sidecar_ticket();
        let m = marks::Mark {
            rating: 3,
            ..Default::default()
        };
        assert!(marks::write_sidecars(t, vec![(a.clone(), m)]).is_empty());
        let r = write_all(
            std::slice::from_ref(&a),
            &[Change::Add("Places|Lisbon".into())],
            &v,
        );
        assert_eq!(r[0].1.as_ref().unwrap().1, s(&["sea", "Places|Lisbon"]));
        assert_eq!(r[0].1.as_ref().unwrap().0, s(&["sea"]));
        let side = std::fs::read_to_string(d.join("A.xmp")).unwrap();
        assert!(side.contains("xmp:Rating=\"3\""), "{side}");
        assert_eq!(
            read_all(std::slice::from_ref(&a), &v)[0].1,
            s(&["sea", "Places|Lisbon"])
        );
        // All taken off: the sidecar says none over the file's own.
        write_all(
            std::slice::from_ref(&a),
            &[
                Change::Remove("sea".into()),
                Change::Remove("places|lisbon".into()),
            ],
            &v,
        );
        assert!(read_all(std::slice::from_ref(&a), &v)[0].1.is_empty());
        // A pair: the RAW owns the stem sidecar, the JPEG the long one.
        let raw = d.join("B.CR3");
        let jpg = d.join("B.JPG");
        std::fs::write(&raw, b"raw").unwrap();
        std::fs::write(&jpg, &jpeg).unwrap();
        write_all(
            &[raw.clone(), jpg.clone()],
            &[Change::Add("pair".into())],
            &v,
        );
        assert!(d.join("B.xmp").is_file() && d.join("B.JPG.xmp").is_file());
        assert_eq!(read_all(std::slice::from_ref(&raw), &v)[0].1, s(&["pair"]));
        assert_eq!(
            read_all(std::slice::from_ref(&jpg), &v)[0].1,
            s(&["sea", "pair"])
        );
        // Undo: each file gets back exactly what it had (the pair's JPEG keeps "sea").
        let before = |w: &Written| -> Vec<(PathBuf, Vec<String>)> {
            w.iter()
                .map(|(p, r)| (p.clone(), r.as_ref().unwrap().0.clone()))
                .collect()
        };
        let w = write_all(
            &[raw.clone(), jpg.clone()],
            &[Change::Add("more".into())],
            &v,
        );
        assert_eq!(
            read_all(std::slice::from_ref(&jpg), &v)[0].1,
            s(&["sea", "pair", "more"])
        );
        let back = restore_all(&before(&w), &v);
        assert!(back.iter().all(|(_, r)| r.is_ok()));
        assert_eq!(read_all(std::slice::from_ref(&raw), &v)[0].1, s(&["pair"]));
        assert_eq!(
            read_all(std::slice::from_ref(&jpg), &v)[0].1,
            s(&["sea", "pair"])
        );
        // A file that is gone is reported, nothing is written for it.
        let gone = write_all(&[d.join("C.jpg")], &[Change::Add("x".into())], &v);
        assert!(gone[0].1.is_err() && !d.join("C.xmp").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A rename writes both properties in the sidecars that have the keyword, leaves the
    /// others alone, merges onto a keyword a photo has, and Undo puts each back.
    #[test]
    fn rename_rewrites_both_properties_and_undoes() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let d = tmp("rename");
        let (a, b, c) = (d.join("A.jpg"), d.join("B.jpg"), d.join("C.jpg"));
        for p in [&a, &b, &c] {
            std::fs::write(p, b"\xFF\xD8\xFF\xD9").unwrap();
        }
        let v = Vocabulary::new();
        write_all(
            std::slice::from_ref(&a),
            &[
                Change::Add("Places|Pari|Louvre".into()),
                Change::Add("sea".into()),
            ],
            &v,
        );
        write_all(
            std::slice::from_ref(&b),
            &[
                Change::Add("Places|Paris".into()),
                Change::Add("Places|Pari".into()),
            ],
            &v,
        );
        write_all(std::slice::from_ref(&c), &[Change::Add("other".into())], &v);
        let c_side = std::fs::read(d.join("C.xmp")).unwrap();
        let done = AtomicUsize::new(0);
        let w = rename_all(
            &[a.clone(), b.clone(), c.clone()],
            "Places|Pari",
            "Places|Paris",
            &v,
            &done,
        );
        assert_eq!(done.load(Ordering::Relaxed), 3);
        assert!(w.iter().all(|(_, r)| r.is_ok()), "{w:?}");
        assert_eq!(
            read_all(std::slice::from_ref(&a), &v)[0].1,
            s(&["Places|Paris|Louvre", "sea"])
        );
        // Merged: once.
        assert_eq!(
            read_all(std::slice::from_ref(&b), &v)[0].1,
            s(&["Places|Paris"])
        );
        let side = std::fs::read_to_string(d.join("A.xmp")).unwrap();
        assert!(side.contains("Places|Paris|Louvre"), "{side}");
        assert!(!side.contains("Pari<") && !side.contains("Pari|"), "{side}");
        assert!(
            side.contains("<rdf:li>Louvre</rdf:li>"),
            "dc:subject: {side}"
        );
        // C was not touched.
        assert_eq!(std::fs::read(d.join("C.xmp")).unwrap(), c_side);
        // Undo: the lists before, back.
        let before: Vec<(PathBuf, Vec<String>)> = w
            .iter()
            .filter_map(|(p, r)| {
                let (was, now) = r.as_ref().ok()?;
                (was != now).then(|| (p.clone(), was.clone()))
            })
            .collect();
        assert_eq!(before.len(), 2);
        restore_all(&before, &v);
        assert_eq!(
            read_all(std::slice::from_ref(&a), &v)[0].1,
            s(&["Places|Pari|Louvre", "sea"])
        );
        assert_eq!(
            read_all(std::slice::from_ref(&b), &v)[0].1,
            s(&["Places|Paris", "Places|Pari"])
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn list_and_recents_are_saved() {
        let d = tmp("list");
        let mut v = Vocabulary::new();
        v.add("A|B");
        v.set_private("A|B", true);
        let mut r = Recents::default();
        r.used("A|B");
        save(&d, &v.to_lightroom(), &r.to_text()).unwrap();
        let (v2, r2) = load(&d);
        assert!(v2.is_private("a|b"));
        assert_eq!(r2, r);
        let _ = std::fs::remove_dir_all(&d);
    }
}
