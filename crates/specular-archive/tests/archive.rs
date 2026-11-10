//! Archives generated with independent writers (`zip`, `tar`, `sevenz-rust2`), read back.
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

use std::io::{Cursor, Write};

use specular_archive::{
    Archive, ComicLookup, Error, Format, Manga, Method, Options, PageType, Password,
};

/// Deterministic pseudo-random bytes (incompressible "JPEG-like" page data).
fn noise(seed: u64, len: usize) -> Vec<u8> {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect()
}

/// Compressible page data.
fn text(seed: u64, len: usize) -> Vec<u8> {
    let s = format!("page {seed} lorem ipsum dolor sit amet ");
    s.bytes().cycle().take(len).collect()
}

const COMIC_INFO: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<ComicInfo xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <Title>The &amp; Test</Title>
  <Series>Specular</Series>
  <Number>3</Number>
  <Volume>2</Volume>
  <PageCount>4</PageCount>
  <LanguageISO>ja</LanguageISO>
  <Manga>YesAndRightToLeft</Manga>
  <Pages>
    <Page Image="0" Type="FrontCover" ImageWidth="1000" ImageHeight="1500" />
    <Page Image="1" />
    <Page Image="2" DoublePage="True" ImageWidth="2000" ImageHeight="1500" Type="Story" />
    <Page Image="3" Type="BackCover" Bookmark="End" />
  </Pages>
</ComicInfo>"#;

struct Item {
    name: &'static str,
    data: Vec<u8>,
    deflate: bool,
}

fn items() -> Vec<Item> {
    let mut v = vec![
        Item {
            name: "p10.jpg",
            data: noise(10, 5000),
            deflate: false,
        },
        Item {
            name: "p2.jpg",
            data: noise(2, 7000),
            deflate: false,
        },
        Item {
            name: "p1.png",
            data: text(1, 20000),
            deflate: true,
        },
        Item {
            name: "ch2/p1.jpg",
            data: noise(21, 300),
            deflate: false,
        },
        Item {
            name: "ch1/p10.jpg",
            data: text(110, 3000),
            deflate: true,
        },
        Item {
            name: "ch1/p9.jpg",
            data: noise(19, 0),
            deflate: false,
        },
        Item {
            name: "ComicInfo.xml",
            data: COMIC_INFO.as_bytes().to_vec(),
            deflate: true,
        },
        Item {
            name: "__MACOSX/._p1.png",
            data: noise(99, 100),
            deflate: false,
        },
        Item {
            name: "extras/bonus.cbz",
            data: noise(7, 64),
            deflate: false,
        },
    ];
    v.push(Item {
        name: "notes.txt",
        data: b"hello".to_vec(),
        deflate: true,
    });
    v
}

const SORTED: &[&str] = &[
    "ComicInfo.xml",
    "notes.txt",
    "p1.png",
    "p2.jpg",
    "p10.jpg",
    "__MACOSX/._p1.png",
    "ch1/p9.jpg",
    "ch1/p10.jpg",
    "ch2/p1.jpg",
    "extras/bonus.cbz",
];

fn make_zip(items: &[Item], large: bool) -> Vec<u8> {
    use zip::write::SimpleFileOptions;
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    w.add_directory("ch1/", SimpleFileOptions::default())
        .unwrap();
    for it in items {
        let method = if it.deflate {
            zip::CompressionMethod::Deflated
        } else {
            zip::CompressionMethod::Stored
        };
        let opts = SimpleFileOptions::default()
            .compression_method(method)
            .large_file(large);
        w.start_file(it.name, opts).unwrap();
        w.write_all(&it.data).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn make_tar(items: &[Item]) -> Vec<u8> {
    let mut b = tar::Builder::new(Vec::new());
    let mut dir = tar::Header::new_gnu();
    dir.set_entry_type(tar::EntryType::Directory);
    dir.set_size(0);
    dir.set_mode(0o755);
    dir.set_cksum();
    b.append_data(&mut dir, "ch1/", &[][..]).unwrap();
    for it in items {
        let mut h = tar::Header::new_gnu();
        h.set_size(it.data.len() as u64);
        h.set_mode(0o644);
        h.set_cksum();
        b.append_data(&mut h, it.name, &it.data[..]).unwrap();
    }
    b.into_inner().unwrap()
}

/// A gzip file of `data` in stored Deflate blocks (the inflater is zlib-rs; this keeps the
/// writer independent of it).
fn make_gz(data: &[u8]) -> Vec<u8> {
    let mut g = vec![0x1f, 0x8b, 8, 8, 0, 0, 0, 0, 0, 3];
    g.extend_from_slice(b"book.tar\0");
    let chunks: Vec<&[u8]> = data.chunks(65535).collect();
    for (k, c) in chunks.iter().enumerate() {
        g.push(u8::from(k + 1 == chunks.len()));
        g.extend_from_slice(&(c.len() as u16).to_le_bytes());
        g.extend_from_slice(&(!(c.len() as u16)).to_le_bytes());
        g.extend_from_slice(c);
    }
    g.extend_from_slice(&crc32fast::hash(data).to_le_bytes());
    g.extend_from_slice(&(data.len() as u32).to_le_bytes());
    g
}

/// An ISO 9660 directory record.
fn iso_record(lba: usize, size: usize, flags: u8, name: &[u8]) -> Vec<u8> {
    let len = 33 + name.len() + usize::from(name.len().is_multiple_of(2));
    let mut r = vec![0u8; len];
    r[0] = len as u8;
    r[2..6].copy_from_slice(&(lba as u32).to_le_bytes());
    r[6..10].copy_from_slice(&(lba as u32).to_be_bytes());
    r[10..14].copy_from_slice(&(size as u32).to_le_bytes());
    r[14..18].copy_from_slice(&(size as u32).to_be_bytes());
    r[25] = flags;
    r[28] = 1;
    r[31] = 1;
    r[32] = name.len() as u8;
    r[33..33 + name.len()].copy_from_slice(name);
    r
}

/// A minimal ISO 9660 image of `items` (folders one level deep): a primary volume with
/// upper-case ISO names and, with `joliet`, a Joliet volume with the real names, both trees
/// pointing at the same file extents.
fn make_iso(items: &[Item], joliet: bool) -> Vec<u8> {
    const S: usize = 2048;
    let mut dirs: std::collections::BTreeMap<&str, Vec<usize>> = Default::default();
    dirs.insert("", Vec::new());
    for (i, it) in items.iter().enumerate() {
        let parent = it.name.rsplit_once('/').map_or("", |p| p.0);
        dirs.entry(parent).or_default().push(i);
    }
    let trees = if joliet { 2 } else { 1 };
    let first_dir = 16 + trees + 1;
    let ndirs = dirs.len();
    let mut next = first_dir + trees * ndirs;
    let file_lba: Vec<usize> = items
        .iter()
        .map(|it| {
            let l = next;
            next += it.data.len().div_ceil(S).max(1);
            l
        })
        .collect();
    let mut img = vec![0u8; next * S];
    let paths: Vec<&str> = dirs.keys().copied().collect();
    for t in 0..trees {
        let j = t == 1;
        let name = |s: &str, file: bool| -> Vec<u8> {
            let s = if file {
                format!("{s};1")
            } else {
                s.to_string()
            };
            if j {
                s.encode_utf16().flat_map(u16::to_be_bytes).collect()
            } else {
                s.chars()
                    .map(|c| match c {
                        'a'..='z' => c.to_ascii_uppercase() as u8,
                        'A'..='Z' | '0'..='9' | '.' | ';' => c as u8,
                        _ => b'_',
                    })
                    .collect()
            }
        };
        let dir_lba = |k: usize| first_dir + t * ndirs + k;
        for (k, (path, files)) in dirs.iter().enumerate() {
            let mut d = iso_record(dir_lba(k), S, 2, &[0]);
            d.extend(iso_record(dir_lba(0), S, 2, &[1]));
            if path.is_empty() {
                for (k2, sub) in paths.iter().enumerate().skip(1) {
                    d.extend(iso_record(dir_lba(k2), S, 2, &name(sub, false)));
                }
            }
            for &i in files {
                let base = items[i].name.rsplit('/').next().unwrap();
                d.extend(iso_record(
                    file_lba[i],
                    items[i].data.len(),
                    0,
                    &name(base, true),
                ));
            }
            assert!(d.len() <= S);
            img[dir_lba(k) * S..][..d.len()].copy_from_slice(&d);
        }
        let root = iso_record(dir_lba(0), S, 2, &[0]);
        let vd = &mut img[(16 + t) * S..(17 + t) * S];
        vd[0] = if j { 2 } else { 1 };
        vd[1..6].copy_from_slice(b"CD001");
        vd[6] = 1;
        if j {
            vd[88..91].copy_from_slice(b"%/E");
        }
        vd[128..130].copy_from_slice(&(S as u16).to_le_bytes());
        vd[130..132].copy_from_slice(&(S as u16).to_be_bytes());
        vd[156..190].copy_from_slice(&root);
    }
    let term = &mut img[(16 + trees) * S..];
    term[0] = 255;
    term[1..6].copy_from_slice(b"CD001");
    term[6] = 1;
    for (i, it) in items.iter().enumerate() {
        img[file_lba[i] * S..][..it.data.len()].copy_from_slice(&it.data);
    }
    img
}

fn make_7z(items: &[Item], solid: bool) -> Vec<u8> {
    use sevenz_rust2::{ArchiveEntry, ArchiveWriter, SourceReader};
    let mut w = ArchiveWriter::new(Cursor::new(Vec::new())).unwrap();
    if solid {
        let entries = items
            .iter()
            .map(|i| ArchiveEntry::new_file(i.name))
            .collect();
        let readers = items
            .iter()
            .map(|i| SourceReader::new(Cursor::new(i.data.clone())))
            .collect();
        w.push_archive_entries(entries, readers).unwrap();
    } else {
        for i in items {
            w.push_archive_entry(
                ArchiveEntry::new_file(i.name),
                Some(Cursor::new(&i.data[..])),
            )
            .unwrap();
        }
    }
    w.finish().unwrap().into_inner()
}

fn open(bytes: Vec<u8>) -> Archive {
    Archive::from_bytes(bytes, Options::default()).unwrap()
}

fn check_contents(a: &Archive, items: &[Item]) {
    let names: Vec<&str> = a.entries().iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, SORTED);
    for (i, e) in a.entries().iter().enumerate() {
        let it = items.iter().find(|it| it.name == e.name).unwrap();
        assert_eq!(e.size, it.data.len() as u64, "{}", e.name);
        assert_eq!(a.read(i).unwrap(), it.data, "{}", e.name);
    }
    let pages = a.pages(|e| matches!(e.extension().as_str(), "jpg" | "png"));
    let page_names: Vec<&str> = pages
        .iter()
        .map(|&i| a.entries()[i].name.as_str())
        .collect();
    assert_eq!(
        page_names,
        [
            "p1.png",
            "p2.jpg",
            "p10.jpg",
            "ch1/p9.jpg",
            "ch1/p10.jpg",
            "ch2/p1.jpg"
        ]
    );
    let nested = a.nested_archives();
    assert_eq!(nested.len(), 1);
    assert_eq!(a.entries()[nested[0]].name, "extras/bonus.cbz");
    let ci = a.comic_info().expect("ComicInfo.xml");
    assert!(ci.right_to_left());
}

#[test]
fn zip_roundtrip() {
    let items = items();
    let a = open(make_zip(&items, false));
    assert_eq!(a.format(), Format::Zip);
    check_contents(&a, &items);
    let p2 = a.find("p2.jpg").unwrap();
    assert_eq!(a.entries()[p2].method, Method::Stored);
    assert_eq!(a.stored_slice(p2).unwrap(), &items[1].data[..]);
    let p1 = a.find("P1.PNG").unwrap();
    assert_eq!(a.entries()[p1].method, Method::Deflate);
    assert!(a.entries()[p1].compressed_size < a.entries()[p1].size);
    assert!(a.stored_slice(p1).is_none());
}

#[test]
fn zip64_roundtrip() {
    let items = items();
    let bytes = make_zip(&items, true);
    let a = open(bytes);
    check_contents(&a, &items);
}

#[test]
fn zip_with_prefix() {
    // A self-extractor: an executable stub before the archive; offsets are not adjusted.
    let items = items();
    let mut bytes = noise(1234, 4096);
    bytes.extend(make_zip(&items, false));
    let a = open(bytes);
    check_contents(&a, &items);
}

#[test]
fn zip_from_file_is_mapped() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("specular-archive-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("book.cbz");
    // Large enough to be mapped rather than read (64 KiB threshold in specular-core).
    let mut big = items();
    big.push(Item {
        name: "zz-large.jpg",
        data: noise(5, 200_000),
        deflate: false,
    });
    std::fs::write(&path, make_zip(&big, false)).unwrap();
    let a = Archive::open(&path).unwrap();
    let i = a.find("zz-large.jpg").unwrap();
    assert_eq!(a.read(i).unwrap(), big.last().unwrap().data);
    std::fs::remove_file(&path).ok();
}

#[test]
fn tar_roundtrip() {
    let items = items();
    let a = open(make_tar(&items));
    assert_eq!(a.format(), Format::Tar);
    check_contents(&a, &items);
    let i = a.find("p10.jpg").unwrap();
    assert_eq!(a.stored_slice(i).unwrap(), &items[0].data[..]);
}

#[test]
fn tar_gz_roundtrip() {
    let items = items();
    let a = open(make_gz(&make_tar(&items)));
    assert_eq!(a.format(), Format::TarGz);
    check_contents(&a, &items);
    let i = a.find("p10.jpg").unwrap();
    assert_eq!(a.stored_slice(i).unwrap(), &items[0].data[..]);
    // A gzip file that is not a tar, and one whose trailer lies about the size.
    assert!(matches!(
        Archive::from_bytes(make_gz(b"just text"), Options::default()),
        Err(Error::NotAnArchive)
    ));
    let mut lying = make_gz(&make_tar(&items));
    let n = lying.len();
    lying[n - 4..].copy_from_slice(&1000u32.to_le_bytes());
    assert!(Archive::from_bytes(lying, Options::default()).is_err());
    assert_eq!(Format::from_file_name("Book.TAR.GZ"), Some(Format::TarGz));
    assert_eq!(Format::from_file_name("b.tgz"), Some(Format::TarGz));
    assert_eq!(Format::from_file_name("b.svg.gz"), None);
}

#[test]
fn iso_joliet_roundtrip() {
    let items = items();
    let a = open(make_iso(&items, true));
    assert_eq!(a.format(), Format::Iso);
    check_contents(&a, &items);
    let i = a.find("p10.jpg").unwrap();
    assert_eq!(a.stored_slice(i).unwrap(), &items[0].data[..]);
    assert_eq!(Format::from_file_name("disc.iso"), Some(Format::Iso));
}

#[test]
fn iso_plain_names() {
    let items = items();
    let a = open(make_iso(&items, false));
    let names: Vec<&str> = a.entries().iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"P10.JPG"), "{names:?}");
    assert!(names.contains(&"CH1/P9.JPG"), "{names:?}");
    let i = a.find("p10.jpg").unwrap();
    assert_eq!(a.read(i).unwrap(), items[0].data);
}

#[test]
fn tar_long_names() {
    let long = format!("{}/page-001.jpg", "a-very-long-folder-name".repeat(8));
    let data = noise(3, 1000);
    // GNU long name ('L' entry).
    let mut b = tar::Builder::new(Vec::new());
    let mut h = tar::Header::new_gnu();
    h.set_size(data.len() as u64);
    h.set_cksum();
    b.append_data(&mut h, &long, &data[..]).unwrap();
    let a = open(b.into_inner().unwrap());
    assert_eq!(a.entries()[0].name, long);
    assert_eq!(a.read(0).unwrap(), data);

    // ustar prefix field (name split at a '/').
    let mut b = tar::Builder::new(Vec::new());
    let mut h = tar::Header::new_ustar();
    let short = format!("{}/p.jpg", "d".repeat(120));
    h.set_path(&short).unwrap();
    h.set_size(data.len() as u64);
    h.set_cksum();
    b.append(&h, &data[..]).unwrap();
    let a = open(b.into_inner().unwrap());
    assert_eq!(a.entries()[0].name, short);

    // pax extended header.
    let mut bytes = Vec::new();
    let rec = {
        let body = format!("path={long}\n");
        let mut n = body.len() + 3;
        while format!("{n} {body}").len() != n {
            n += 1;
        }
        format!("{n} {body}")
    };
    let mut b = tar::Builder::new(&mut bytes);
    let mut x = tar::Header::new_ustar();
    x.set_entry_type(tar::EntryType::XHeader);
    x.set_path("PaxHeaders/x").unwrap();
    x.set_size(rec.len() as u64);
    x.set_cksum();
    b.append(&x, rec.as_bytes()).unwrap();
    let mut h = tar::Header::new_ustar();
    h.set_path("short.jpg").unwrap();
    h.set_size(data.len() as u64);
    h.set_cksum();
    b.append(&h, &data[..]).unwrap();
    b.finish().unwrap();
    drop(b);
    let a = open(bytes);
    assert_eq!(a.len(), 1);
    assert_eq!(a.entries()[0].name, long);
    assert_eq!(a.read(0).unwrap(), data);
}

#[test]
fn sevenz_non_solid() {
    let items = items();
    let a = open(make_7z(&items, false));
    assert_eq!(a.format(), Format::SevenZ);
    check_contents(&a, &items);
    assert!(matches!(
        a.entries()[0].method,
        Method::Lzma2 | Method::Lzma
    ));
}

#[test]
fn sevenz_solid_any_order() {
    let items = items();
    let a = open(make_7z(&items, true));
    check_contents(&a, &items);
    // Backwards (restarts the streamer or hits the cache) and repeated reads.
    for i in (0..a.len()).rev() {
        let e = &a.entries()[i];
        let it = items.iter().find(|it| it.name == e.name).unwrap();
        assert_eq!(a.read(i).unwrap(), it.data);
        assert_eq!(a.read(i).unwrap(), it.data);
    }
    // Without a cache, every out-of-order read restarts the stream.
    let opts = Options {
        solid_cache_bytes: 0,
        ..Options::default()
    };
    let a = Archive::from_bytes(make_7z(&items, true), opts).unwrap();
    for i in [5, 2, 9, 0, 9, 3] {
        let e = &a.entries()[i];
        let it = items.iter().find(|it| it.name == e.name).unwrap();
        assert_eq!(a.read(i).unwrap(), it.data, "{}", e.name);
    }
}

/// `read_many` of every entry in reverse, collected by position.
fn read_all_many(a: &Archive) -> Vec<Option<Vec<u8>>> {
    let indices: Vec<usize> = (0..a.len()).rev().collect();
    let mut got = vec![None; indices.len()];
    let mut order = Vec::new();
    a.read_many(&indices, &mut |k, r| {
        order.push(indices[k]);
        got[k] = Some(r.unwrap());
        true
    });
    // Handed over in stream order, whatever order they were asked in.
    let places: Vec<_> = order.iter().map(|&i| a.stream_position(i)).collect();
    assert!(places.iter().all(Option::is_some) || places.iter().all(Option::is_none));
    if places.iter().all(Option::is_some) {
        assert!(places.is_sorted(), "{places:?}");
    }
    got.reverse();
    got
}

#[test]
fn read_many_in_stream_order() {
    let items = items();
    for (bytes, solid) in [
        (make_7z(&items, true), true),
        (make_7z(&items, false), false),
        (make_zip(&items, false), false),
        (make_tar(&items), false),
    ] {
        let a = open(bytes);
        let streams: Vec<_> = (0..a.len()).map(|i| a.stream_position(i)).collect();
        assert_eq!(streams.iter().all(Option::is_some), solid, "{streams:?}");
        if solid {
            // One block: one stream; places differ.
            assert!(
                streams
                    .iter()
                    .all(|s| s.unwrap().0 == streams[0].unwrap().0)
            );
            let mut at: Vec<_> = streams.iter().map(|s| s.unwrap().1).collect();
            at.sort();
            at.dedup();
            assert_eq!(at.len(), a.len());
        }
        for (i, data) in read_all_many(&a).into_iter().enumerate() {
            assert_eq!(data.unwrap(), a.read(i).unwrap(), "{}", a.entries()[i].name);
        }
        // Stopping early: nothing more is handed over.
        let mut n = 0;
        a.read_many(&[0, 1, 2], &mut |_, _| {
            n += 1;
            false
        });
        assert_eq!(n, 1);
        // Errors are per entry.
        let mut res = vec![];
        a.read_many(&[0, 999], &mut |k, r| {
            res.push((k, r.is_ok()));
            true
        });
        res.sort();
        assert_eq!(res, [(0, true), (1, false)]);
    }
}

#[test]
fn parallel_reads() {
    let items = items();
    for bytes in [
        make_zip(&items, false),
        make_tar(&items),
        make_7z(&items, true),
        make_7z(&items, false),
    ] {
        let a = open(bytes);
        std::thread::scope(|s| {
            for t in 0..6 {
                let a = &a;
                let items = &items;
                s.spawn(move || {
                    for k in 0..40 {
                        let i = (k * 7 + t * 3) % a.len();
                        let e = &a.entries()[i];
                        let it = items.iter().find(|it| it.name == e.name).unwrap();
                        assert_eq!(a.read(i).unwrap(), it.data);
                    }
                });
            }
        });
    }
}

/// RAR fixtures shared with the decode host's tests (made with RAR 7.20; it cannot write RAR 4).
fn rar_fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/../specular-decode-host/tests/data/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

/// Opens through the sandboxed host, or `None` (after checking the error) when no host with
/// RAR support is available, e.g. `specular-decode-host` not built or not Linux.
fn open_rar(bytes: Vec<u8>) -> Option<Result<Archive, Error>> {
    match Archive::from_bytes(bytes, Options::default()) {
        Err(Error::NeedsSandbox(Format::Rar)) => {
            eprintln!("SKIP: no RAR-capable decode host (cargo build -p specular-decode-host)");
            None
        }
        r => Some(r),
    }
}

#[test]
fn rar_detection_and_garbage() {
    for sig in [&b"Rar!\x1a\x07\x00"[..], &b"Rar!\x1a\x07\x01\x00"[..]] {
        let mut bytes = sig.to_vec();
        bytes.extend(noise(1, 100));
        assert_eq!(specular_archive::detect(&bytes), Some(Format::Rar));
        // UnRAR either rejects the noise or finds no entries in it.
        if let Some(r) = open_rar(bytes) {
            assert!(
                matches!(&r, Err(Error::Malformed(_))) || r.as_ref().is_ok_and(|a| a.is_empty()),
                "{r:?}"
            );
        }
    }
    assert_eq!(
        specular_archive::rar_version(b"Rar!\x1a\x07\x01\x00"),
        Some(5)
    );
}

#[test]
fn rar_through_the_host() {
    let random = rar_fixture("random.bin");
    let numbers: String = (1..=20000).map(|i| format!("{i}\n")).collect();
    for name in ["rar5.rar", "solid.rar"] {
        let Some(r) = open_rar(rar_fixture(name)) else {
            return;
        };
        let a = r.unwrap();
        assert_eq!(a.format(), Format::Rar);
        let names: Vec<_> = a.entries().iter().map(|e| e.name.as_str()).collect();
        // Folders are not entries; natural order, a folder's files before its subfolders'.
        assert_eq!(
            names,
            ["a.txt", "sub/grad.png", "sub/numbers.txt", "sub/random.bin"],
            "{name}"
        );
        assert!(matches!(&a.entries()[0].method, Method::Other(m) if m.starts_with("RAR")));
        assert_eq!(a.read(0).unwrap(), b"hello from rar\n");
        assert_eq!(a.read(a.find("sub/random.bin").unwrap()).unwrap(), random);
        assert_eq!(
            a.read(a.find("sub/numbers.txt").unwrap()).unwrap(),
            numbers.as_bytes()
        );
        assert!(
            a.read(a.find("sub/grad.png").unwrap())
                .unwrap()
                .starts_with(b"\x89PNG")
        );
        assert!(matches!(a.read(99), Err(Error::NoEntry(99))));
        // A solid archive is one stream (read in one pass); a plain one has none.
        let solid = name == "solid.rar";
        assert_eq!(a.stream_position(0).is_some(), solid, "{name}");
        for (i, data) in read_all_many(&a).into_iter().enumerate() {
            assert_eq!(data.unwrap(), a.read(i).unwrap(), "{name} {i}");
        }
        // Entries larger than the limit are refused before the host is asked.
        let mut o = Options::default();
        o.limits.max_entry_size = 1000;
        let small = Archive::from_bytes(rar_fixture(name), o).unwrap();
        let i = small.find("sub/random.bin").unwrap();
        assert!(matches!(small.read(i), Err(Error::Limit(_))));
        let mut o = Options::default();
        o.limits.max_entries = 2;
        assert!(matches!(
            Archive::from_bytes(rar_fixture(name), o),
            Err(Error::Limit(_))
        ));
    }
}

#[test]
fn rar_encrypted() {
    let Some(r) = open_rar(rar_fixture("encrypted.rar")) else {
        return;
    };
    let a = r.unwrap();
    let i = a.find("a.txt").unwrap();
    assert!(a.entries()[i].encrypted);
    assert!(matches!(a.read(i), Err(Error::Encrypted)));
    // Encrypted headers: not even the listing is readable.
    let r = open_rar(rar_fixture("hdrenc.rar")).unwrap();
    assert!(matches!(r, Err(Error::Encrypted)), "{r:?}");
}

#[test]
fn rar_truncated() {
    let bytes = rar_fixture("rar5.rar");
    for n in [8, 30, 200, bytes.len() / 2, bytes.len() - 10] {
        let Some(r) = open_rar(bytes[..n].to_vec()) else {
            return;
        };
        if let Ok(a) = r {
            for i in 0..a.len() {
                let _ = a.read(i);
            }
        }
    }
}

#[test]
fn not_an_archive() {
    assert!(matches!(
        Archive::from_bytes(noise(1, 1000), Options::default()),
        Err(Error::NotAnArchive)
    ));
    assert!(matches!(
        Archive::from_bytes(Vec::new(), Options::default()),
        Err(Error::NotAnArchive)
    ));
}

#[test]
fn empty_zip() {
    let w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let a = open(w.finish().unwrap().into_inner());
    assert!(a.is_empty());
    assert!(a.comic_info().is_none());
    assert_eq!(a.comic_info_lookup(Some(0)), ComicLookup::Absent);
}

// ---- limits ------------------------------------------------------------------------------------

#[test]
fn ratio_limit() {
    let zeros = Item {
        name: "bomb.bmp",
        data: vec![0; 8 << 20],
        deflate: true,
    };
    let bytes = make_zip(std::slice::from_ref(&zeros), false);
    let opts = Options {
        limits: specular_archive::Limits {
            ratio_threshold: 1 << 20,
            ..Default::default()
        },
        ..Default::default()
    };
    let a = Archive::from_bytes(bytes.clone(), opts).unwrap();
    assert!(matches!(a.read(0), Err(Error::Limit(_))));
    // The defaults let a legitimately compressible 8 MiB page through.
    let a = open(bytes);
    assert_eq!(a.read(0).unwrap().len(), 8 << 20);
}

/// An entry's declared size is reserved from the memory budget before it is inflated: a
/// 12 MiB page (below the ratio check's threshold) is refused within an 8 MiB budget and
/// reads within a 64 MiB one.
#[test]
fn entry_reads_reserve_their_size() {
    let zeros = Item {
        name: "blank.bmp",
        data: vec![0; 12 << 20],
        deflate: true,
    };
    let bytes = make_zip(std::slice::from_ref(&zeros), false);
    assert!(bytes.len() < 1 << 20);
    for b in [bytes, make_7z(std::slice::from_ref(&zeros), false)] {
        let a = open(b);
        let r = specular_core::budget::with_limit(8 << 20, || a.read(0));
        assert!(
            matches!(&r, Err(Error::Limit(m)) if m.contains("free RAM")),
            "{:?} {:?}",
            a.format(),
            r.err()
        );
        let r = specular_core::budget::with_limit(64 << 20, || a.read(0));
        assert_eq!(r.unwrap().len(), 12 << 20);
        // Read with its reservation, the entry stays reserved while the caller keeps it.
        let b = std::sync::Arc::new(specular_core::budget::MemoryBudget::fixed(20 << 20));
        specular_core::budget::with_budget(b.clone(), || {
            let (page, kept) = a.read_reserved(0).unwrap();
            assert_eq!((page.len() as u64, kept.bytes()), (12 << 20, 12 << 20));
            assert_eq!(b.reserved(), 12 << 20);
            assert!(a.read_reserved(0).is_err());
            let mut n = 0;
            a.read_many_reserved(&[0], &mut |_, r| {
                n += 1;
                r.is_err()
            });
            assert_eq!(n, 1);
            drop((page, kept));
            assert_eq!(b.reserved(), 0);
        });
    }
}

#[test]
fn size_limit() {
    let items = items();
    let opts = Options {
        limits: specular_archive::Limits {
            max_entry_size: 4096,
            ..Default::default()
        },
        ..Default::default()
    };
    for bytes in [
        make_zip(&items, false),
        make_tar(&items),
        make_7z(&items, true),
        make_7z(&items, false),
    ] {
        let a = Archive::from_bytes(bytes, opts.clone()).unwrap();
        let big = a.find("p1.png").unwrap();
        assert!(
            matches!(a.read(big), Err(Error::Limit(_))),
            "{:?}",
            a.format()
        );
        // Small entries after a refused one in a solid block still read.
        let small = a.find("ch2/p1.jpg").unwrap();
        assert_eq!(a.read(small).unwrap().len(), 300);
    }
}

#[test]
fn entry_count_limit() {
    let items = items();
    let opts = Options {
        limits: specular_archive::Limits {
            max_entries: 3,
            ..Default::default()
        },
        ..Default::default()
    };
    for bytes in [
        make_zip(&items, false),
        make_tar(&items),
        make_7z(&items, false),
    ] {
        assert!(matches!(
            Archive::from_bytes(bytes, opts.clone()),
            Err(Error::Limit(_))
        ));
    }
}

/// Finds the central directory header of the first entry named `name`.
fn find_cdh(zip: &[u8], name: &str) -> usize {
    (0..zip.len() - 46)
        .find(|&p| {
            zip[p..p + 4] == [0x50, 0x4b, 0x01, 0x02]
                && zip.get(p + 46..p + 46 + name.len()) == Some(name.as_bytes())
        })
        .unwrap()
}

#[test]
fn lying_sizes_never_overrun() {
    let items = [Item {
        name: "a.png",
        data: text(1, 100_000),
        deflate: true,
    }];
    let mut bytes = make_zip(&items, false);
    // Declare a smaller uncompressed size than the stream produces.
    let p = find_cdh(&bytes, "a.png");
    bytes[p + 24..p + 28].copy_from_slice(&1000u32.to_le_bytes());
    let a = open(bytes.clone());
    assert_eq!(a.entries()[0].size, 1000);
    assert!(matches!(a.read(0), Err(Error::Malformed(_))));
    // And a larger one: the stream ends early.
    bytes[p + 24..p + 28].copy_from_slice(&200_000u32.to_le_bytes());
    let a = open(bytes);
    assert!(a.read(0).is_err());
}

#[test]
fn overlapping_entries_rejected() {
    // Two central-directory records pointing at the same local header (the "overlapped" zip bomb).
    let items = [
        Item {
            name: "a.jpg",
            data: noise(1, 1000),
            deflate: false,
        },
        Item {
            name: "b.jpg",
            data: noise(2, 1000),
            deflate: false,
        },
    ];
    let mut bytes = make_zip(&items, false);
    let p = find_cdh(&bytes, "b.jpg");
    bytes[p + 42..p + 46].copy_from_slice(&0u32.to_le_bytes());
    assert!(matches!(
        Archive::from_bytes(bytes, Options::default()),
        Err(Error::Limit(_))
    ));
}

#[test]
fn crc_mismatch_detected() {
    let items = [Item {
        name: "a.jpg",
        data: noise(1, 1000),
        deflate: false,
    }];
    let mut bytes = make_zip(&items, false);
    // Corrupt one data byte.
    let at = bytes
        .windows(16)
        .position(|w| w == &items[0].data[..16])
        .unwrap();
    bytes[at + 10] ^= 0xFF;
    let a = open(bytes.clone());
    assert!(matches!(a.read(0), Err(Error::Checksum)));
    let opts = Options {
        verify_crc: false,
        ..Options::default()
    };
    let a = Archive::from_bytes(bytes, opts).unwrap();
    assert_eq!(a.read(0).unwrap().len(), 1000);
}

#[test]
fn absurd_declared_counts() {
    // An end record claiming a million entries in a tiny file.
    let mut eocd = vec![0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0];
    eocd.extend(60000u16.to_le_bytes());
    eocd.extend(60000u16.to_le_bytes());
    eocd.extend(1_000_000u32.to_le_bytes());
    eocd.extend(0u32.to_le_bytes());
    eocd.extend(0u16.to_le_bytes());
    assert!(Archive::from_bytes(eocd, Options::default()).is_err());
}

// ---- robustness ----------------------------------------------------------------------------------

/// Opens, lists and reads everything; any error is fine, a panic is not.
fn exercise(bytes: Vec<u8>) {
    let opts = Options {
        limits: specular_archive::Limits {
            max_entry_size: 64 << 20,
            max_decoder_memory: 64 << 20,
            ..Default::default()
        },
        ..Default::default()
    };
    if let Ok(a) = Archive::from_bytes(bytes, opts) {
        for i in 0..a.len() {
            let _ = a.read(i);
            let _ = a.stored_slice(i);
        }
        let _ = a.comic_info();
        let _ = a.pages(|_| true);
    }
}

#[test]
fn malformed_inputs_never_panic() {
    let items = items();
    let samples = [
        make_zip(&items, false),
        make_zip(&items, true),
        make_tar(&items),
        make_7z(&items, false),
        make_7z(&items, true),
        make_gz(&make_tar(&items)),
        make_iso(&items, true),
    ];
    let mut rng = 0x1234_5678_9ABC_DEF1u64;
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    for s in &samples {
        // Every truncation point in steps (and all of the last 600 bytes, where the directories are).
        let n = s.len();
        let step = (n / 300).max(1);
        let mut cuts: Vec<usize> = (0..n).step_by(step).collect();
        cuts.extend(n.saturating_sub(600)..n);
        for cut in cuts {
            exercise(s[..cut].to_vec());
        }
        // Random byte flips, weighted towards headers at both ends.
        for _ in 0..1500 {
            let mut m = s.clone();
            for _ in 0..1 + next() % 4 {
                let r = next() as usize;
                let at = match r % 3 {
                    0 => r / 3 % n,
                    1 => r / 3 % 600.min(n),
                    _ => n - 1 - (r / 3 % 600.min(n)),
                };
                m[at] = (next() & 0xFF) as u8;
            }
            exercise(m);
        }
    }
    // Pure noise behind each magic number.
    for magic in [
        &b"PK\x03\x04"[..],
        b"7z\xBC\xAF\x27\x1C",
        b"Rar!\x1a\x07\x00",
        b"PK\x05\x06",
        b"\x1f\x8b\x08\x00\0\0\0\0\0\x03",
    ] {
        for seed in 0..200 {
            let mut m = magic.to_vec();
            m.extend(noise(seed, (seed as usize * 37) % 3000));
            exercise(m);
        }
    }
}

#[test]
fn iso_noise_directories() {
    // Valid descriptors over noisy directory sectors: bounded, never a panic.
    let items = items();
    let good = make_iso(&items, true);
    for seed in 0..200u64 {
        let mut m = good.clone();
        let noise = noise(seed, 18 * 2048);
        m[18 * 2048..36 * 2048].copy_from_slice(&noise);
        exercise(m);
    }
}

// ---- ComicInfo ---------------------------------------------------------------------------------

#[test]
fn comic_info_fields() {
    let ci = specular_archive::ComicInfo::parse(COMIC_INFO.as_bytes()).unwrap();
    assert_eq!(ci.title.as_deref(), Some("The & Test"));
    assert_eq!(ci.series.as_deref(), Some("Specular"));
    assert_eq!(ci.number.as_deref(), Some("3"));
    assert_eq!(ci.volume, Some(2));
    assert_eq!(ci.page_count, Some(4));
    assert_eq!(ci.language.as_deref(), Some("ja"));
    assert_eq!(ci.manga, Manga::YesAndRightToLeft);
    assert!(ci.right_to_left());
    assert_eq!(ci.pages.len(), 4);
    assert_eq!(ci.pages[0].kind, PageType::FrontCover);
    assert_eq!(ci.pages[0].width, Some(1000));
    assert_eq!(ci.pages[1].kind, PageType::Story);
    assert!(ci.pages[2].double_page);
    assert_eq!(ci.spreads().collect::<Vec<_>>(), [2]);
    assert_eq!(ci.page(3).unwrap().kind, PageType::BackCover);
    assert_eq!(ci.page(3).unwrap().bookmark.as_deref(), Some("End"));
}

#[test]
fn comic_info_when_cheap() {
    let items = items();
    // In the solid 7z, ComicInfo.xml lies behind 35 300 bytes of pages.
    let solid = open(make_7z(&items, true));
    assert!(solid.has_stream_state());
    assert!(solid.comic_info_within(35_299).is_none());
    // Beyond the budget is not read, which is not the same as absent.
    assert_eq!(solid.comic_info_lookup(Some(35_299)), ComicLookup::Unread);
    assert!(solid.comic_info_within(35_300).is_some());
    // Once a read has passed over it, it is known whatever the budget.
    assert!(matches!(
        solid.comic_info_lookup(Some(0)),
        ComicLookup::Found(c) if c.right_to_left()
    ));
    let solid = open(make_7z(&items, true));
    assert!(matches!(
        solid.comic_info_lookup(None),
        ComicLookup::Found(c) if c.right_to_left()
    ));
    // Aside from the streamer, and told to whoever asked to be.
    let solid = open(make_7z(&items, true));
    let (tx, rx) = std::sync::mpsc::channel();
    assert!(solid.on_comic_info(move |c| tx.send(c).unwrap()));
    assert!(solid.comic_info_aside().found().is_some());
    assert!(rx.try_recv().unwrap().found().is_some());
    // Read on its own: always.
    for bytes in [
        make_zip(&items, false),
        make_tar(&items),
        make_7z(&items, false),
    ] {
        let a = open(bytes);
        assert!(!a.has_stream_state());
        assert!(a.comic_info_within(0).unwrap().right_to_left());
    }
}

#[test]
fn comic_info_variants() {
    use specular_archive::ComicInfo;
    let ltr = ComicInfo::parse(b"<ComicInfo><Manga>Yes</Manga></ComicInfo>").unwrap();
    assert_eq!(ltr.manga, Manga::Yes);
    assert!(!ltr.right_to_left());
    assert_eq!(
        ComicInfo::parse(b"<ComicInfo/>").unwrap(),
        ComicInfo::default()
    );
    assert!(ComicInfo::parse(b"<html><Manga>YesAndRightToLeft</Manga></html>").is_none());
    assert!(ComicInfo::parse(b"").is_none());
    assert!(ComicInfo::parse(b"\xFF\xFE garbage").is_none());
    // Cut off: keeps what was complete.
    let cut = ComicInfo::parse(b"<ComicInfo><Manga>YesAndRightToLeft</Manga><Pages><Page Image=\"1\" DoublePage=\"true\"/><Pa").unwrap();
    assert!(cut.right_to_left());
    assert!(cut.pages[0].double_page);
    // Several types, unknown values.
    let multi = ComicInfo::parse(
        br#"<ComicInfo><Pages><Page Image="0" Type="FrontCover Story"/><Page Image="1" Type="Weird"/></Pages></ComicInfo>"#,
    )
    .unwrap();
    assert_eq!(multi.pages[0].kind, PageType::FrontCover);
    assert_eq!(multi.pages[1].kind, PageType::Unknown("Weird".into()));
    // Nested <Page> elements outside <Pages> are ignored.
    let odd = ComicInfo::parse(br#"<ComicInfo><Page Image="5"/></ComicInfo>"#).unwrap();
    assert!(odd.pages.is_empty());
}

#[test]
fn stored_entry_with_mismatched_sizes_is_not_borrowed() {
    // Found by fuzzing: a ZIP entry marked Stored whose compressed and declared sizes differ.
    // `read` rejects it, so the borrowed path must not hand out bytes of either length.
    let a = open(include_bytes!("data/stored_size_mismatch.zip").to_vec());
    for i in 0..a.entries().len() {
        if let Some(s) = a.stored_slice(i) {
            assert_eq!(s.len() as u64, a.entries()[i].size);
        }
        if let Ok(v) = a.read(i) {
            assert_eq!(v.len() as u64, a.entries()[i].size);
        }
    }
}

/// Encrypted fixtures in `tests/data`, all with the password `Secret` but `utf8pw.7z`
/// (`äß€`), holding `a.txt` ("hello from an encrypted archive\n") and `sub/numbers.txt`
/// (1 to 2000, one per line):
/// - `zipcrypto.zip`: Info-ZIP 3.0 `zip -P` (traditional encryption, Deflate, data
///   descriptors, so the check byte is the time's);
/// - `zipcrypto_crc.zip`: 7-Zip `-tzip -mem=ZipCrypto -mx=0` (a.txt only; CRC check byte);
/// - `aes256.zip`, `aes128.zip`: 7-Zip `-mem=AES256` / `-mem=AES128 -mx=0` (WinZip AE-2;
///   aes128.zip holds a.txt only);
/// - `enc.7z` (`-mhe=off`), `hdrenc.7z` (`-mhe=on`), `utf8pw.7z` (`-mhe=on`, a.txt only).
fn enc_fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn with_password(bytes: Vec<u8>, password: Option<&str>) -> Result<Archive, Error> {
    let o = Options {
        password: password.map(|p| Password::new(p.to_owned())),
        ..Options::default()
    };
    Archive::from_bytes(bytes, o)
}

const ENC_HELLO: &[u8] = b"hello from an encrypted archive\n";

fn enc_numbers() -> Vec<u8> {
    (1..=2000)
        .map(|i| format!("{i}\n"))
        .collect::<String>()
        .into_bytes()
}

/// Every entry of `a` reads back as the fixtures hold it.
fn check_enc_contents(a: &Archive, name: &str) {
    assert!(a.is_encrypted(), "{name}");
    a.check_password().unwrap();
    assert_eq!(
        a.read(a.find("a.txt").unwrap()).unwrap(),
        ENC_HELLO,
        "{name}"
    );
    if let Some(i) = a.find("sub/numbers.txt") {
        assert_eq!(a.read(i).unwrap(), enc_numbers(), "{name}");
    }
    for (i, data) in read_all_many(a).into_iter().enumerate() {
        assert_eq!(data.unwrap(), a.read(i).unwrap(), "{name} {i}");
    }
}

#[test]
fn encrypted_zip_and_7z_with_the_right_password() {
    for (name, pw) in [
        ("zipcrypto.zip", "Secret"),
        ("zipcrypto_crc.zip", "Secret"),
        ("aes256.zip", "Secret"),
        ("aes128.zip", "Secret"),
        ("enc.7z", "Secret"),
        ("hdrenc.7z", "Secret"),
        ("utf8pw.7z", "\u{e4}\u{df}\u{20ac}"),
    ] {
        let a = with_password(enc_fixture(name), Some(pw)).unwrap();
        check_enc_contents(&a, name);
        assert!(a.entries().iter().all(|e| e.encrypted), "{name}");
    }
}

#[test]
fn encrypted_zip_and_7z_without_a_password() {
    for name in [
        "zipcrypto.zip",
        "zipcrypto_crc.zip",
        "aes256.zip",
        "aes128.zip",
        "enc.7z",
    ] {
        // The list is readable; the entries are not.
        let a = with_password(enc_fixture(name), None).unwrap();
        assert!(a.is_encrypted(), "{name}");
        assert!(a.find("a.txt").is_some(), "{name}");
        assert!(
            matches!(a.check_password(), Err(Error::Encrypted)),
            "{name}"
        );
        for i in 0..a.len() {
            assert!(matches!(a.read(i), Err(Error::Encrypted)), "{name} {i}");
        }
    }
    // An encrypted 7z header hides the list itself.
    for name in ["hdrenc.7z", "utf8pw.7z"] {
        let r = with_password(enc_fixture(name), None);
        assert!(matches!(r, Err(Error::Encrypted)), "{name}: {r:?}");
    }
}

#[test]
fn encrypted_zip_and_7z_with_a_wrong_or_empty_password() {
    for name in [
        "zipcrypto.zip",
        "zipcrypto_crc.zip",
        "aes256.zip",
        "aes128.zip",
        "enc.7z",
    ] {
        for pw in ["secret", "", "Secret ", "S"] {
            let a = with_password(enc_fixture(name), Some(pw)).unwrap();
            let r = a.check_password();
            assert!(
                matches!(r, Err(Error::WrongPassword)),
                "{name} {pw:?}: {r:?}"
            );
            for i in 0..a.len() {
                let r = a.read(i);
                assert!(
                    matches!(r, Err(Error::WrongPassword)),
                    "{name} {pw:?} {i}: {r:?}"
                );
            }
        }
    }
    for name in ["hdrenc.7z", "utf8pw.7z"] {
        for pw in ["secret", "", "\u{e4}\u{df}"] {
            let r = with_password(enc_fixture(name), Some(pw));
            assert!(
                matches!(r, Err(Error::WrongPassword)),
                "{name} {pw:?}: {r:?}"
            );
        }
    }
}

#[test]
fn a_password_on_a_plain_archive_is_ignored() {
    let a = with_password(enc_fixture("stored_size_mismatch.zip"), Some("x")).unwrap();
    assert!(!a.is_encrypted());
    assert!(a.check_password().is_ok());
}

/// The 10-byte WinZip AES authentication code is checked before anything is decrypted: an
/// altered byte anywhere in the encrypted data is refused, with the right password.
#[test]
fn tampered_aes_zip_is_refused() {
    let bytes = enc_fixture("aes128.zip");
    // Local header: 30 bytes, the name, the extra field; then salt (8), verifier (2), data,
    // auth code (10).
    let n = u16::from_le_bytes([bytes[26], bytes[27]]) as usize;
    let x = u16::from_le_bytes([bytes[28], bytes[29]]) as usize;
    let packed = u32::from_le_bytes(bytes[18..22].try_into().unwrap()) as usize;
    let start = 30 + n + x;
    let end = start + packed;
    for at in [
        start + 10,
        start + 10 + ENC_HELLO.len() / 2,
        end - 11,
        end - 1,
    ] {
        let mut b = bytes.clone();
        b[at] ^= 0x20;
        let a = with_password(b, Some("Secret")).unwrap();
        let r = a.read(0);
        assert!(matches!(r, Err(Error::Malformed(_))), "byte {at}: {r:?}");
    }
}

#[test]
fn truncated_encrypted_archives_do_not_panic() {
    for name in [
        "zipcrypto.zip",
        "zipcrypto_crc.zip",
        "aes256.zip",
        "aes128.zip",
        "enc.7z",
        "hdrenc.7z",
    ] {
        let bytes = enc_fixture(name);
        let mut cuts: Vec<usize> = (0..bytes.len()).step_by(37).collect();
        cuts.extend(bytes.len().saturating_sub(40)..bytes.len());
        for n in cuts {
            for pw in [Some("Secret"), Some("x"), None] {
                if let Ok(a) = with_password(bytes[..n].to_vec(), pw) {
                    let _ = a.check_password();
                    for i in 0..a.len() {
                        let _ = a.read(i);
                    }
                    let _ = read_all_many(&a);
                }
            }
        }
    }
}

#[test]
fn rar_passwords() {
    let Some(r) = open_rar(rar_fixture("encrypted.rar")) else {
        return;
    };
    assert!(r.unwrap().is_encrypted());
    // encrypted.rar: RAR5, `-pSecret` (the list is plain); hdrenc.rar: `-hpSecret`.
    for name in ["encrypted.rar", "hdrenc.rar"] {
        let a = with_password(rar_fixture(name), Some("Secret")).unwrap();
        a.check_password().unwrap();
        let i = a.find("a.txt").unwrap();
        assert!(a.entries()[i].encrypted, "{name}");
        assert_eq!(a.read(i).unwrap(), b"hello from rar\n", "{name}");
        for (i, data) in read_all_many(&a).into_iter().enumerate() {
            assert_eq!(data.unwrap(), a.read(i).unwrap(), "{name} {i}");
        }
        for pw in ["secret", "", "Secret\u{0}", "p\u{e4}ss"] {
            let r = with_password(rar_fixture(name), Some(pw));
            let r = r.and_then(|a| a.check_password().map(|_| a));
            assert!(
                matches!(r, Err(Error::WrongPassword)),
                "{name} {pw:?}: {r:?}"
            );
        }
    }
}
