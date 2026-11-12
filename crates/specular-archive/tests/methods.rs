//! ZIP compression methods beyond Deflate (Deflate64, bzip2, LZMA, Zstandard, xz) and the
//! bzip2/xz/Zstandard-compressed tars, read through the public API. The streams come from the
//! reference libraries (libbz2, liblzma, libzstd) or, for the committed fixtures, from 7-Zip
//! and Python's `zipfile`.
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

use std::io::Write;

use specular_archive::{Archive, Error, Format, Method, Options};

fn sample(seed: u64, len: usize) -> Vec<u8> {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut v = Vec::with_capacity(len);
    while v.len() < len {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        if x.is_multiple_of(3) {
            v.extend_from_slice(format!("page {} of {seed} ", x % 97).as_bytes());
        } else {
            v.push(x as u8);
        }
    }
    v.truncate(len);
    v
}

fn bz2(data: &[u8]) -> Vec<u8> {
    let mut e = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::new(9));
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

fn xz(data: &[u8]) -> Vec<u8> {
    let mut e = xz2::write::XzEncoder::new(Vec::new(), 6);
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

fn zst(data: &[u8]) -> Vec<u8> {
    zstd::bulk::compress(data, 3).unwrap()
}

/// ZIP's LZMA layout (version, properties size, properties, raw stream with an end marker),
/// from liblzma's `.lzma` output (properties, dictionary, size, stream).
fn zip_lzma(data: &[u8]) -> Vec<u8> {
    let opts = xz2::stream::LzmaOptions::new_preset(6).unwrap();
    let s = xz2::stream::Stream::new_lzma_encoder(&opts).unwrap();
    let mut e = xz2::write::XzEncoder::new_stream(Vec::new(), s);
    e.write_all(data).unwrap();
    let alone = e.finish().unwrap();
    let mut v = vec![16, 2, 5, 0];
    v.extend_from_slice(&alone[..5]);
    v.extend_from_slice(&alone[13..]);
    v
}

struct ZipItem {
    name: &'static str,
    method: u16,
    data: Vec<u8>,
    packed: Vec<u8>,
}

/// A ZIP of already-compressed entries (with their CRCs, unless `bad_crc`).
fn make_zip(items: &[ZipItem], bad_crc: bool) -> Vec<u8> {
    let mut z = Vec::new();
    let mut cd = Vec::new();
    for it in items {
        let crc = crc32fast::hash(&it.data) ^ u32::from(bad_crc);
        let flags: u16 = if it.method == 14 { 2 } else { 0 };
        let offset = z.len() as u32;
        let mut common = Vec::new();
        common.extend_from_slice(&63u16.to_le_bytes());
        common.extend_from_slice(&flags.to_le_bytes());
        common.extend_from_slice(&it.method.to_le_bytes());
        common.extend_from_slice(&[0, 0, 0x21, 0]);
        common.extend_from_slice(&crc.to_le_bytes());
        common.extend_from_slice(&(it.packed.len() as u32).to_le_bytes());
        common.extend_from_slice(&(it.data.len() as u32).to_le_bytes());
        common.extend_from_slice(&(it.name.len() as u16).to_le_bytes());
        common.extend_from_slice(&0u16.to_le_bytes());
        z.extend_from_slice(b"PK\x03\x04");
        z.extend_from_slice(&common);
        z.extend_from_slice(it.name.as_bytes());
        z.extend_from_slice(&it.packed);
        cd.extend_from_slice(b"PK\x01\x02");
        cd.extend_from_slice(&63u16.to_le_bytes());
        cd.extend_from_slice(&common);
        cd.extend_from_slice(&[0; 6]);
        cd.extend_from_slice(&0u32.to_le_bytes());
        cd.extend_from_slice(&offset.to_le_bytes());
        cd.extend_from_slice(it.name.as_bytes());
    }
    let cd_start = z.len() as u32;
    z.extend_from_slice(&cd);
    z.extend_from_slice(b"PK\x05\x06\0\0\0\0");
    z.extend_from_slice(&(items.len() as u16).to_le_bytes());
    z.extend_from_slice(&(items.len() as u16).to_le_bytes());
    z.extend_from_slice(&(cd.len() as u32).to_le_bytes());
    z.extend_from_slice(&cd_start.to_le_bytes());
    z.extend_from_slice(&0u16.to_le_bytes());
    z
}

fn method_items() -> Vec<ZipItem> {
    let item = |name, method, seed, len, pack: fn(&[u8]) -> Vec<u8>| {
        let data = sample(seed, len);
        let packed = pack(&data);
        ZipItem {
            name,
            method,
            data,
            packed,
        }
    };
    vec![
        item("p1.jpg", 12, 1, 300_000, bz2),
        item("p2.jpg", 14, 2, 200_000, zip_lzma),
        item("p3.jpg", 93, 3, 250_000, zst),
        item("p4.jpg", 95, 4, 150_000, xz),
        item("p5.jpg", 12, 5, 0, bz2),
        item("p6.jpg", 93, 6, 1, zst),
    ]
}

#[test]
fn zip_methods_roundtrip() {
    let items = method_items();
    let a = Archive::from_bytes(make_zip(&items, false), Options::default()).unwrap();
    assert_eq!(a.format(), Format::Zip);
    for it in &items {
        let i = a.find(it.name).unwrap();
        let want = match it.method {
            12 => Method::Bzip2,
            14 => Method::Lzma,
            93 => Method::Zstd,
            _ => Method::Xz,
        };
        assert_eq!(a.entries()[i].method, want, "{}", it.name);
        assert_eq!(a.read(i).unwrap(), it.data, "{}", it.name);
    }
    // A wrong CRC is caught for every method.
    let a = Archive::from_bytes(make_zip(&items, true), Options::default()).unwrap();
    for it in &items {
        let i = a.find(it.name).unwrap();
        assert!(matches!(a.read(i), Err(Error::Checksum)), "{}", it.name);
    }
}

#[test]
fn zip_methods_hostile() {
    for it in method_items().into_iter().filter(|it| it.data.len() > 1) {
        // Declared smaller than the stream: stopped at the declared size, not decoded on.
        let short = ZipItem {
            data: it.data[..it.data.len() / 2].to_vec(),
            ..it
        };
        let a = Archive::from_bytes(make_zip(&[short], false), Options::default()).unwrap();
        assert!(
            matches!(a.read(0), Err(Error::Malformed(_))),
            "short {}",
            it.name
        );
        // Truncated compressed data.
        let full = sample(9, 100_000);
        let mut packed = match it.method {
            12 => bz2(&full),
            14 => zip_lzma(&full),
            93 => zst(&full),
            _ => xz(&full),
        };
        packed.truncate(packed.len() * 2 / 3);
        let cut = ZipItem {
            name: "cut.jpg",
            method: it.method,
            data: full,
            packed,
        };
        let a = Archive::from_bytes(make_zip(&[cut], false), Options::default()).unwrap();
        assert!(a.read(0).is_err(), "truncated {}", it.name);
    }
}

/// The 152 KiB page in the committed fixtures (also what `methods_7z.zip` holds twice).
fn fixture_page() -> Vec<u8> {
    let mut x: u64 = 1;
    let mut part1 = Vec::new();
    for _ in 0..20000 {
        x = (x * 1_103_515_245 + 12345) % (1 << 31);
        part1.push((x >> 16) as u8);
    }
    let mut v = part1.clone();
    for i in 0..1200 {
        v.extend_from_slice(format!("line {i:05} of the deflate64 fixture\n").as_bytes());
    }
    v.extend_from_slice(&part1);
    v.resize(v.len() + 70_000, b'a');
    v
}

#[test]
fn seven_zip_deflate64_and_xz() {
    // `7z a -tzip -mm=Deflate64 -mx=9` and `-mm=XZ` (7-Zip 26.00).
    let a = Archive::from_bytes(
        include_bytes!("data/methods_7z.zip").to_vec(),
        Options::default(),
    )
    .unwrap();
    let page = fixture_page();
    let i = a.find("page.bin").unwrap();
    assert_eq!(a.entries()[i].method, Method::Deflate64);
    assert_eq!(a.read(i).unwrap(), page);
    let i = a.find("page_xz.bin").unwrap();
    assert_eq!(a.entries()[i].method, Method::Xz);
    assert_eq!(a.read(i).unwrap(), page);
}

#[test]
fn python_zipfile_methods() {
    // Python 3.14 `zipfile`: ZIP_BZIP2, ZIP_LZMA (with its end marker) and ZIP_ZSTANDARD.
    let a = Archive::from_bytes(
        include_bytes!("data/methods_py.zip").to_vec(),
        Options::default(),
    )
    .unwrap();
    let mut text = Vec::new();
    for i in 0..600 {
        text.extend_from_slice(
            format!("page {i:04} of the method fixture, made by Python's zipfile\n").as_bytes(),
        );
    }
    for (name, method) in [
        ("bzip2.txt", Method::Bzip2),
        ("lzma.txt", Method::Lzma),
        ("zstd.txt", Method::Zstd),
    ] {
        let i = a.find(name).unwrap();
        assert_eq!(a.entries()[i].method, method);
        assert_eq!(a.read(i).unwrap(), text, "{name}");
    }
}

fn make_tar(files: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut b = tar::Builder::new(Vec::new());
    for (name, data) in files {
        let mut h = tar::Header::new_gnu();
        h.set_size(data.len() as u64);
        h.set_mode(0o644);
        h.set_cksum();
        b.append_data(&mut h, name, &data[..]).unwrap();
    }
    b.into_inner().unwrap()
}

#[test]
fn compressed_tars() {
    let files: Vec<(&str, Vec<u8>)> = vec![
        ("ch1/p2.jpg", sample(1, 1_200_000)),
        ("ch1/p10.jpg", sample(2, 3000)),
        ("ch1/p1.png", sample(3, 700_000)),
    ];
    let tar = make_tar(&files);
    for threads in [1, 4] {
        let options = Options {
            decode_threads: threads,
            ..Options::default()
        };
        for (format, packed) in [
            (Format::TarBz2, bz2(&tar)),
            (Format::TarXz, xz(&tar)),
            (Format::TarZst, zst(&tar)),
        ] {
            let a = Archive::from_bytes(packed.clone(), options.clone()).unwrap();
            assert_eq!(a.format(), format);
            let names: Vec<&str> = a.entries().iter().map(|e| e.name.as_str()).collect();
            assert_eq!(
                names,
                ["ch1/p1.png", "ch1/p2.jpg", "ch1/p10.jpg"],
                "{format}"
            );
            for (name, data) in &files {
                let i = a.find(name).unwrap();
                assert_eq!(a.stored_slice(i).unwrap(), &data[..], "{format} {name}");
            }
            // Truncated: an error, never a partial book.
            let cut = packed[..packed.len() - 100].to_vec();
            assert!(
                Archive::from_bytes(cut, options.clone()).is_err(),
                "{format}"
            );
        }
    }
    // A compressed file that is not a tar.
    assert!(matches!(
        Archive::from_bytes(zst(b"just text"), Options::default()),
        Err(Error::NotAnArchive)
    ));
}

#[test]
fn compressed_tar_bombs_stop_at_the_cap() {
    let tar = make_tar(&[("zeros.bmp", vec![0; 40 << 20])]);
    for packed in [bz2(&tar), xz(&tar), zst(&tar)] {
        let r = Archive::from_bytes(packed, Options::default());
        assert!(matches!(r, Err(Error::Limit(_))), "{r:?}");
    }
}

/// bzip2 and Zstandard tars do not declare their unpacked size: the output is reserved as it
/// grows, so a 16 MiB tar is refused by the decoder itself (the refusal names it) within an
/// 8 MiB budget, instead of after all of it was decoded. Within 64 MiB the archive holds
/// exactly the tar's size until it is dropped.
#[test]
fn compressed_tars_reserve_as_they_grow() {
    let tar = make_tar(&[("page.bmp", sample(7, 16 << 20))]);
    for (packed, codec) in [(bz2(&tar), "bzip2"), (zst(&tar), "Zstandard")] {
        let r = specular_core::budget::with_limit(8 << 20, || {
            Archive::from_bytes(packed.clone(), Options::default())
        });
        match r {
            Err(Error::Limit(m)) => assert!(m.starts_with(codec) && m.contains("free RAM"), "{m}"),
            r => panic!("{codec}: {:?}", r.map(|a| a.len())),
        }
        let b = std::sync::Arc::new(specular_core::budget::MemoryBudget::fixed(64 << 20));
        let a = specular_core::budget::with_budget(b.clone(), || {
            Archive::from_bytes(packed, Options::default())
        })
        .unwrap();
        assert_eq!(b.reserved(), tar.len() as u64, "{codec}");
        drop(a);
        assert_eq!(b.reserved(), 0, "{codec}");
    }
}

/// The budget a compressed tar holds while it grows is the output
/// growing (by doubling, so at most twice the tar), whichever thread decodes it, and after
/// opening exactly the tar. A budget 2 MiB short of the tar is refused by the decoder itself,
/// with 1 and 4 threads.
#[test]
fn compressed_tars_reserve_no_more_than_they_hold() {
    let tar = make_tar(&[("page.bmp", sample(9, 16 << 20))]);
    for threads in [1, 4] {
        let options = || Options {
            decode_threads: threads,
            ..Options::default()
        };
        for (packed, codec) in [(bz2(&tar), "bzip2"), (zst(&tar), "Zstandard")] {
            let short = (tar.len() as u64) - (2 << 20);
            let r = specular_core::budget::with_limit(short, || {
                Archive::from_bytes(packed.clone(), options())
            });
            match r {
                Err(Error::Limit(m)) => assert!(m.starts_with(codec), "{m}"),
                r => panic!("{codec}: {:?}", r.map(|a| a.len())),
            }
            // Room for the unpacked tar twice (growing, then held) and for what four decode
            // threads hold while blocks are in flight (~4.5 MiB each at level 9) beside the
            // output's next growth step: 8 MiB was too tight on a slower 4-core CI runner.
            let b = std::sync::Arc::new(specular_core::budget::MemoryBudget::fixed(
                2 * tar.len() as u64 + (32 << 20),
            ));
            let a = specular_core::budget::with_budget(b.clone(), || {
                Archive::from_bytes(packed, options())
            })
            .unwrap();
            assert_eq!(b.reserved(), tar.len() as u64, "{codec} {threads}");
            drop(a);
            assert_eq!(b.reserved(), 0, "{codec} {threads}");
        }
    }
}

#[test]
fn compressed_tar_names() {
    for (name, format) in [
        ("Book.tar.bz2", Format::TarBz2),
        ("b.TBZ2", Format::TarBz2),
        ("b.tbz", Format::TarBz2),
        ("b.cbt.bz2", Format::TarBz2),
        ("b.tar.xz", Format::TarXz),
        ("b.txz", Format::TarXz),
        ("b.CBT.XZ", Format::TarXz),
        ("b.tar.zst", Format::TarZst),
        ("b.tzst", Format::TarZst),
        ("b.cbt.zst", Format::TarZst),
        ("b.tar.gz", Format::TarGz),
        ("b.cbt", Format::Tar),
    ] {
        assert_eq!(Format::from_file_name(name), Some(format), "{name}");
    }
    for name in ["b.svg.xz", "b.bz2", "b.zst", "b.xz", "tar.xz", "b.tarxz"] {
        assert_eq!(Format::from_file_name(name), None, "{name}");
    }
}
