//! The IPTC fields of photos on disk, and the templates in the config folder.
//!
//! A photo's fields are its XMP sidecar's where the sidecar has them (an empty one says
//! "none"), else its own (XMP, else IPTC-IIM). Writing changes the sidecar, found as the
//! gallery's marks find it, so marks, keywords and fields share one sidecar (every writer takes
//! [`marks::SIDECAR_WRITE`]). With writing into files on, a JPEG, TIFF or DNG gets the fields
//! itself (replaced atomically, nothing else in it changed) and a sidecar it already has is
//! brought up to date; other files still get a sidecar. Raw files are never written.

use std::io;
use std::path::{Path, PathBuf};

use specular_metadata::edit::{self, Edit, Record, embed, template::Template};
use specular_metadata::keywords::{self as kw, Vocabulary};

use crate::app::gallery::marks::{self, Sidecars};

/// The templates in the config folder.
pub(super) const TEMPLATES: &str = "metadata-templates.txt";

/// What a file holds of its own.
fn embedded(image: &Path) -> edit::Embedded {
    specular_core::open_regular(image)
        .ok()
        .and_then(|mut f| edit::read_embedded(&mut f).ok())
        .unwrap_or_default()
}

/// The fields of `image` now: its sidecar's (text given), then its own.
fn effective(image: &Path, side: Option<&str>) -> (edit::Embedded, Record) {
    let own = embedded(image);
    let rec = side
        .map(edit::from_xmp_text)
        .unwrap_or_default()
        .over(&own.record());
    (own, rec)
}

fn sidecar_text(path: &Path, exists: bool) -> io::Result<Option<String>> {
    if !exists {
        return Ok(None);
    }
    let mut v = Vec::new();
    io::Read::read_to_end(&mut specular_core::open_regular(path)?, &mut v)?;
    Ok(Some(String::from_utf8_lossy(&v).into_owned()))
}

/// The fields of each of `images` (one folder scan per folder).
pub(super) fn read_all(images: &[PathBuf]) -> Vec<(PathBuf, Record)> {
    let mut folders = std::collections::HashMap::<PathBuf, Sidecars>::new();
    images
        .iter()
        .map(|p| {
            let dir = p.parent().unwrap_or(Path::new("")).to_path_buf();
            let sc = folders.entry(dir).or_insert_with_key(|d| Sidecars::scan(d));
            let (side, exists) = sc.resolve(p);
            let text = sidecar_text(&side, exists).ok().flatten();
            (p.clone(), effective(p, text.as_deref()).1)
        })
        .collect()
}

/// Applies `e` to each of `images` (read afresh, so changes made elsewhere since are kept)
/// and writes them. Returns each image's fields after, or why it failed.
pub(super) fn write_all(
    images: &[PathBuf],
    e: &Edit,
    vocab: &Vocabulary,
    into_files: bool,
) -> Vec<(PathBuf, Result<Record, String>)> {
    let _one_at_a_time = marks::SIDECAR_WRITE
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut folders = std::collections::HashMap::<PathBuf, Sidecars>::new();
    let mut out = Vec::with_capacity(images.len());
    for image in images {
        let dir = image.parent().unwrap_or(Path::new("")).to_path_buf();
        let sc = folders.entry(dir).or_insert_with_key(|d| Sidecars::scan(d));
        let r = write_one(image, sc, e, vocab, into_files);
        out.push((image.clone(), r.map_err(|e| e.to_string())));
    }
    out
}

/// Whether `image` is written itself (with writing into files on).
fn takes_fields(image: &Path, head: &[u8]) -> bool {
    image
        .extension()
        .is_some_and(|x| embed::writable_ext(&x.to_string_lossy()))
        && embed::container(head).is_some()
}

fn write_one(
    image: &Path,
    sc: &mut Sidecars,
    e: &Edit,
    vocab: &Vocabulary,
    into_files: bool,
) -> io::Result<Record> {
    if !image.is_file() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "the file is gone"));
    }
    let (side, exists) = sc.resolve(image);
    let is_xmp = |p: &Path| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("xmp"));
    // A sidecar is never anything but a sidecar.
    if !is_xmp(&side) || is_xmp(image) || side == image {
        return Err(io::Error::other("not a sidecar path"));
    }
    let side_text = sidecar_text(&side, exists)?;
    let (own, current) = effective(image, side_text.as_deref());
    let after = e.apply(&current);
    if after == current {
        return Ok(after);
    }
    let fields = e.fields();
    let mut head = [0u8; 4];
    let into = into_files && {
        use std::io::Read;
        let n = specular_core::open_regular(image)
            .and_then(|mut f| f.read(&mut head))
            .unwrap_or(0);
        takes_fields(image, &head[..n])
    };
    if into {
        write_into(image, &own, &after, &fields, vocab)?;
        // A sidecar it has would still say the old values over the file's.
        if side_text.is_some() {
            write_sidecar(&side, side_text.as_deref(), &after, &fields, vocab, sc)?;
        }
    } else {
        write_sidecar(&side, side_text.as_deref(), &after, &fields, vocab, sc)?;
    }
    Ok(after)
}

fn write_sidecar(
    path: &Path,
    existing: Option<&str>,
    after: &Record,
    fields: &[edit::Field],
    vocab: &Vocabulary,
    sc: &mut Sidecars,
) -> io::Result<()> {
    let text = edit::write_xmp(existing, after, fields, Some(vocab), true);
    if existing != Some(text.as_str()) {
        marks::write_atomic(path, text.as_bytes())?;
        sc.add(path);
    }
    Ok(())
}

/// The fields into the file itself: the whole file read, the packet (and a JPEG's IPTC
/// block) replaced, then written to a temporary beside it and renamed over it.
fn write_into(
    image: &Path,
    own: &edit::Embedded,
    after: &Record,
    fields: &[edit::Field],
    vocab: &Vocabulary,
) -> io::Result<()> {
    let bytes = std::fs::read(image)?;
    let xmp = edit::write_xmp(own.xmp.as_deref(), after, fields, Some(vocab), false);
    let out = match embed::container(&bytes) {
        Some(embed::Container::Jpeg) => {
            let subject = kw::subject(after.values(edit::Field::Keywords), Some(vocab));
            let iim = edit::write_iim(own.iim.as_deref(), after, fields, &subject);
            embed::jpeg(&bytes, &xmp, Some(iim.as_deref()))
        }
        Some(embed::Container::Tiff) => embed::tiff(&bytes, &xmp),
        None => Err(embed::EmbedError::Unsupported),
    }
    .map_err(io::Error::other)?;
    specular_fileops::atomic::replace_file(image, &out, false)
}

/// The templates saved in `dir`.
pub(super) fn load(dir: &Path) -> Vec<Template> {
    std::fs::read_to_string(dir.join(TEMPLATES))
        .map(|s| edit::template::parse(&s))
        .unwrap_or_default()
}

/// Saves the templates into `dir`.
pub(super) fn save(dir: &Path, templates: &[Template]) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    marks::write_atomic(
        &dir.join(TEMPLATES),
        edit::template::to_text(templates).as_bytes(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_metadata::edit::{Field, KeywordEdit};

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("specular-meta-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| (*x).to_owned()).collect()
    }

    /// A JPEG with a title in its XMP and an IPTC block with a caption.
    fn jpeg() -> Vec<u8> {
        let mut r = Record::new();
        r.set(Field::Title, s(&["Own title"]));
        let x = edit::write_xmp(None, &r, &[Field::Title], None, false);
        let mut c = Record::new();
        c.set(Field::Description, s(&["Own caption"]));
        let iim = edit::write_iim(None, &c, &[Field::Description], &[]).unwrap();
        let base = [
            &[0xFF, 0xD8, 0xFF, 0xDB, 0, 4, 0, 0][..],
            &[0xFF, 0xDA, 0, 2, 9, 9, 0xFF, 0xD9],
        ]
        .concat();
        embed::jpeg(&base, &x, Some(Some(&iim))).unwrap()
    }

    /// Sidecar by default: the image is untouched, the sidecar wins, marks beside it stay;
    /// a batch writes only the field it names and keeps each photo's other values.
    #[test]
    fn sidecars_take_the_fields_and_images_stay() {
        let d = tmp("side");
        let a = d.join("A.jpg");
        let b = d.join("B.jpg");
        std::fs::write(&a, jpeg()).unwrap();
        std::fs::write(&b, jpeg()).unwrap();
        let before = std::fs::read(&a).unwrap();
        let r = read_all(std::slice::from_ref(&a));
        assert_eq!(r[0].1.values(Field::Title), s(&["Own title"]));
        assert_eq!(r[0].1.values(Field::Description), s(&["Own caption"]));
        // Marks first (as the gallery writes them).
        let t = marks::sidecar_ticket();
        let m = marks::Mark {
            rating: 4,
            ..Default::default()
        };
        assert!(marks::write_sidecars(t, vec![(a.clone(), m)]).is_empty());
        // B gets its own headline first.
        let one = Edit {
            set: vec![(Field::Headline, s(&["B's own"]))],
            keywords: None,
        };
        write_all(std::slice::from_ref(&b), &one, &Vocabulary::new(), false);
        let batch = Edit {
            set: vec![(Field::Copyright, s(&["© Ann"]))],
            keywords: Some(KeywordEdit::Replace(s(&["Places|Lisbon"]))),
        };
        let out = write_all(&[a.clone(), b.clone()], &batch, &Vocabulary::new(), false);
        assert!(out.iter().all(|(_, r)| r.is_ok()));
        assert_eq!(
            std::fs::read(&a).unwrap(),
            before,
            "the image is never written"
        );
        let side = std::fs::read_to_string(d.join("A.xmp")).unwrap();
        assert!(side.contains("xmp:Rating=\"4\""), "{side}");
        let r = read_all(&[a.clone(), b.clone()]);
        for (_, rec) in &r {
            assert_eq!(rec.values(Field::Copyright), s(&["© Ann"]));
            assert_eq!(rec.values(Field::Keywords), s(&["Places|Lisbon"]));
            assert_eq!(rec.values(Field::Title), s(&["Own title"]));
        }
        assert_eq!(r[1].1.values(Field::Headline), s(&["B's own"]));
        assert!(r[0].1.get(Field::Headline).is_none());
        // Cleared: the sidecar says none over the file's own caption.
        let clear = Edit {
            set: vec![(Field::Description, vec![])],
            keywords: None,
        };
        write_all(std::slice::from_ref(&a), &clear, &Vocabulary::new(), false);
        assert!(
            read_all(std::slice::from_ref(&a))[0]
                .1
                .values(Field::Description)
                .is_empty()
        );
        assert_eq!(std::fs::read(&a).unwrap(), before);
        // A raw file never takes the fields itself, even with writing into files on.
        let raw = d.join("C.CR2");
        std::fs::write(&raw, b"II*\0\x08\0\0\0\0\0\0\0\0\0").unwrap();
        let raw_before = std::fs::read(&raw).unwrap();
        write_all(std::slice::from_ref(&raw), &batch, &Vocabulary::new(), true);
        assert_eq!(std::fs::read(&raw).unwrap(), raw_before);
        assert!(d.join("C.xmp").is_file());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Import: the template's fields and the keywords typed go into each copied photo's
    /// sidecar, never into the file; a template that replaces keywords keeps the typed ones.
    #[test]
    fn import_writes_the_template_and_keywords_to_sidecars() {
        use specular_metadata::edit::template::KeywordMode;
        let d = tmp("import");
        let a = d.join("IMG_0001.JPG");
        let raw = d.join("IMG_0001.CR2");
        std::fs::write(&a, jpeg()).unwrap();
        std::fs::write(&raw, b"II*\0\x08\0\0\0\0\0\0\0\0\0").unwrap();
        let (before, raw_before) = (std::fs::read(&a).unwrap(), std::fs::read(&raw).unwrap());
        // Nothing chosen: nothing to write.
        assert!(super::super::import_edit(None, "  ").is_none());
        let mut t = Template::new("Studio");
        t.fields.push((Field::Copyright, s(&["© Ann"])));
        t.fields.push((Field::Keywords, s(&["Work"])));
        t.keyword_mode = KeywordMode::Replace;
        let e = super::super::import_edit(Some(&t), "Family; Places > Paris; work")
            .expect("something to write");
        let failed = super::super::write_import(&[a.clone(), raw.clone()], &e, &Vocabulary::new());
        assert!(failed.is_empty(), "{failed:?}");
        assert_eq!(
            std::fs::read(&a).unwrap(),
            before,
            "the photo is never written"
        );
        assert_eq!(std::fs::read(&raw).unwrap(), raw_before);
        for p in [&a, &raw] {
            let r = read_all(std::slice::from_ref(p));
            assert_eq!(r[0].1.values(Field::Copyright), s(&["© Ann"]), "{p:?}");
            assert_eq!(
                r[0].1.values(Field::Keywords),
                s(&["Work", "Family", "Places|Paris"]),
                "{p:?}"
            );
        }
        let side = std::fs::read_to_string(d.join("IMG_0001.xmp")).unwrap();
        assert!(side.contains("lr:hierarchicalSubject"), "{side}");
        // Keywords alone are added to what a photo has.
        let e = super::super::import_edit(None, "Beach").unwrap();
        assert!(
            super::super::write_import(std::slice::from_ref(&a), &e, &Vocabulary::new()).is_empty()
        );
        assert_eq!(
            read_all(std::slice::from_ref(&a))[0]
                .1
                .values(Field::Keywords),
            s(&["Work", "Family", "Places|Paris", "Beach"])
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Writing into files: the JPEG gets XMP and IPTC; its sidecar is kept in step; the
    /// image data is the same bytes.
    #[test]
    fn into_files_writes_the_jpeg_and_keeps_its_sidecar_in_step() {
        let d = tmp("into");
        let a = d.join("A.jpg");
        std::fs::write(&a, jpeg()).unwrap();
        let before = std::fs::read(&a).unwrap();
        let e = Edit {
            set: vec![(Field::Title, s(&["New title"]))],
            keywords: None,
        };
        // No sidecar: none is made.
        let r = write_all(std::slice::from_ref(&a), &e, &Vocabulary::new(), true);
        assert!(r[0].1.is_ok(), "{:?}", r[0].1);
        assert!(!d.join("A.xmp").exists());
        let after = std::fs::read(&a).unwrap();
        assert!(after.ends_with(&[0xFF, 0xDA, 0, 2, 9, 9, 0xFF, 0xD9]));
        assert_ne!(after, before);
        let own = embedded(&a);
        let i = edit::read_iim(own.iim.as_deref().unwrap());
        assert_eq!(i.values(Field::Title), s(&["New title"]));
        assert_eq!(i.values(Field::Description), s(&["Own caption"]));
        assert_eq!(own.record().values(Field::Title), s(&["New title"]));
        // With a sidecar that has the field, the sidecar follows.
        let side = edit::write_xmp(None, &own.record(), &[Field::Title], None, true);
        std::fs::write(d.join("A.xmp"), side).unwrap();
        let e2 = Edit {
            set: vec![(Field::Title, s(&["Third"]))],
            keywords: None,
        };
        write_all(std::slice::from_ref(&a), &e2, &Vocabulary::new(), true);
        assert_eq!(
            read_all(std::slice::from_ref(&a))[0].1.values(Field::Title),
            s(&["Third"])
        );
        assert_eq!(embedded(&a).record().values(Field::Title), s(&["Third"]));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn templates_are_saved_and_loaded() {
        let d = tmp("tpl");
        let mut t = Template::new("Mine");
        t.set(Field::Copyright, s(&["© Me"]));
        save(&d, std::slice::from_ref(&t)).unwrap();
        assert_eq!(load(&d), vec![t]);
        assert!(load(&d.join("none")).is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }
}
