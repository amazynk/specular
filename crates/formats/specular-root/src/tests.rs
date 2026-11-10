//! Reading the fixtures, and refusals: unknown class versions, truncated keys and objects,
//! corrupt compression.

use super::*;

fn fixture(name: &str) -> Vec<u8> {
    let p = format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

fn key<'f>(f: &'f RootFile, path: &str) -> &'f Key {
    f.keys
        .iter()
        .rev()
        .find(|k| k.path() == path)
        .unwrap_or_else(|| panic!("no key {path}"))
}

fn hist(f: &RootFile, data: &[u8], path: &str) -> Hist {
    match f.read(data, key(f, path)) {
        Ok(Object::Hist(h)) => *h,
        other => panic!("{path}: {other:?}"),
    }
}

#[test]
fn header_and_keys() {
    let data = fixture("types.root");
    let f = RootFile::open(&data).expect("open");
    assert!(!f.large);
    assert_eq!(f.compress, 104);
    assert_eq!(f.compression(), "zlib level 4");
    assert!(
        f.version_string().starts_with("6."),
        "{}",
        f.version_string()
    );
    assert_eq!(f.keys.len(), 16);
    assert!(f.streamers.iter().any(|(c, v)| c == "TH1" && *v == 8));
    assert!(f.warnings.is_empty(), "{:?}", f.warnings);
    let h = hist(&f, &data, "h2f");
    assert_eq!(h.class, "TH2F");
    assert_eq!((h.axes[0].nbins, h.axes[1].nbins), (24, 16));
    assert_eq!(h.cells.len(), 26 * 18);
    assert_eq!(h.axes[0].unit(), Some("cm"));
    assert!(h.entries > 0.0);
}

#[test]
fn nested_directories_cycles_and_trees() {
    let data = fixture("nested.root");
    let f = RootFile::open(&data).expect("open");
    let paths: Vec<String> = f.keys.iter().map(Key::path).collect();
    for p in ["run1/tracker/inner/occ", "run2/hits", "events", "ntuple"] {
        assert!(paths.contains(&p.to_owned()), "{p} in {paths:?}");
    }
    let tops: Vec<i16> = f
        .keys
        .iter()
        .filter(|k| k.path() == "top")
        .map(|k| k.cycle)
        .collect();
    assert_eq!(tops.len(), 2);
    match f.read(&data, key(&f, "events")).expect("tree") {
        Object::Tree(t) => assert_eq!(t.entries, 1234),
        o => panic!("{o:?}"),
    }
    assert!(matches!(
        f.read(&data, key(&f, "ntuple")),
        Err(Error::Unsupported(_))
    ));
}

#[test]
fn every_compression() {
    let none = fixture("comp_none.root");
    let fnone = RootFile::open(&none).expect("open");
    let want = hist(&fnone, &none, "hpxpy");
    for (tag, alg) in [("zlib", 1), ("lz4", 4), ("zstd", 5), ("lzma", 2)] {
        let data = fixture(&format!("comp_{tag}.root"));
        let f = RootFile::open(&data).expect(tag);
        assert_eq!(f.compress / 100, alg, "{tag}");
        let k = key(&f, "hpxpy");
        assert!(k.compressed(), "{tag}");
        let h = hist(&f, &data, "hpxpy");
        assert_eq!(h.cells, want.cells, "{tag}");
        assert_eq!(h.sumw2, want.sumw2, "{tag}");
    }
}

/// The offset of a key's object bytes in the file.
fn object_at(f: &RootFile, path: &str) -> usize {
    let k = key(f, path);
    k.seek as usize + usize::from(k.keylen)
}

#[test]
fn refuses_unknown_class_versions() {
    let mut data = fixture("comp_none.root");
    let f = RootFile::open(&data).expect("open");
    let at = object_at(&f, "hpxpy");
    // TH2F's version follows its byte count.
    data[at + 4..at + 6].copy_from_slice(&99i16.to_be_bytes());
    match f.read(&data, key(&f, "hpxpy")) {
        Err(Error::Version { class, version, .. }) => {
            assert_eq!((class.as_str(), version), ("TH2F", 99));
        }
        o => panic!("{o:?}"),
    }
    // TH1 inside TH2 inside TH2F.
    let mut data = fixture("comp_none.root");
    data[at + 16..at + 18].copy_from_slice(&2i16.to_be_bytes());
    let e = f.read(&data, key(&f, "hpxpy")).expect_err("TH1 v2");
    assert!(e.to_string().contains("TH1 version 2"), "{e}");
}

#[test]
fn refuses_truncation() {
    let data = fixture("types.root");
    let f = RootFile::open(&data).expect("open");
    // The file cut inside the top key list.
    let cut = f.seek_keys as usize + 120;
    assert!(matches!(
        RootFile::open(&data[..cut]),
        Err(Error::Truncated(_))
    ));
    assert!(matches!(
        RootFile::open(&data[..40]),
        Err(Error::Truncated(_))
    ));
    assert!(matches!(
        RootFile::open(b"rootxxxx"),
        Err(Error::Truncated(_))
    ));
    assert!(matches!(RootFile::open(b"TFile..."), Err(Error::NotRoot)));
    // An object past the end of a cut file.
    let k = key(&f, "h2d");
    let short = &data[..(k.seek as usize + 100)];
    assert!(matches!(f.read(short, k), Err(Error::Truncated(_))));
    // A key that claims more than the file holds.
    let mut k2 = k.clone();
    k2.nbytes = u32::MAX / 2;
    assert!(matches!(f.read(&data, &k2), Err(Error::Truncated(_))));
}

#[test]
fn refuses_bad_compression() {
    for (tag, at_data) in [("zlib", 40usize), ("zstd", 40), ("lz4", 30), ("lzma", 60)] {
        let mut data = fixture(&format!("comp_{tag}.root"));
        let f = RootFile::open(&data).expect(tag);
        let at = object_at(&f, "hpxpy");
        for b in &mut data[at + 9 + at_data..at + 9 + at_data + 16] {
            *b ^= 0x5a;
        }
        let r = f.read(&data, key(&f, "hpxpy"));
        assert!(
            matches!(
                r,
                Err(Error::Compression(_) | Error::Corrupt(_) | Error::Truncated(_))
            ),
            "{tag}: {r:?}"
        );
    }
    let mut data = fixture("comp_zlib.root");
    let f = RootFile::open(&data).expect("zlib");
    let at = object_at(&f, "hpxpy");
    data[at..at + 2].copy_from_slice(b"QQ");
    let e = f.read(&data, key(&f, "hpxpy")).expect_err("QQ");
    assert!(e.to_string().contains("unknown algorithm"), "{e}");
    // A block size beyond the object.
    let mut data = fixture("comp_zlib.root");
    data[at + 6..at + 9].copy_from_slice(&[0xff, 0xff, 0xff]);
    assert!(matches!(
        f.read(&data, key(&f, "hpxpy")),
        Err(Error::Compression(_))
    ));
    // LZ4's checksum.
    let mut data = fixture("comp_lz4.root");
    let f = RootFile::open(&data).expect("lz4");
    let at = object_at(&f, "hpxpy");
    data[at + 9] ^= 1;
    let e = f.read(&data, key(&f, "hpxpy")).expect_err("checksum");
    assert!(e.to_string().contains("checksum"), "{e}");
}

#[test]
fn variable_bins_labels_profiles() {
    let data = fixture("variable.root");
    let f = RootFile::open(&data).expect("open");
    let h = hist(&f, &data, "v2xy");
    assert!(h.axes[0].is_variable() && h.axes[1].is_variable());
    assert_eq!(h.axes[0].edge(1), 0.0);
    assert_eq!(h.axes[0].edge(13), 100.0);
    assert_eq!(h.axes[0].width(3), 2.0);

    let data = fixture("labels.root");
    let f = RootFile::open(&data).expect("open");
    let h = hist(&f, &data, "cutflow");
    assert_eq!(h.axes[0].label(3), Some("2 muons"));
    let h = hist(&f, &data, "occupancy");
    assert_eq!(h.axes[1].label(2), Some("endcap A"));
    assert!(h.axes[0].labels.is_empty());

    let data = fixture("profiles.root");
    let f = RootFile::open(&data).expect("open");
    let p = hist(&f, &data, "prof");
    assert!(p.profile);
    assert_eq!(p.bin_entries.len(), p.cells.len());
    let p2 = hist(&f, &data, "prof2d");
    assert_eq!((p2.dims, p2.axes[1].nbins), (2, 15));
}

#[test]
fn empty_file() {
    let data = fixture("empty.root");
    let f = RootFile::open(&data).expect("open");
    assert!(f.keys.is_empty());
}

#[test]
fn datimes() {
    // 2026-10-12 08:30:15
    let d = ((2026 - 1995) << 26) | (10 << 22) | (12 << 17) | (8 << 12) | (30 << 6) | 15;
    assert_eq!(datime_string(d).as_deref(), Some("2026-10-12 08:30:15"));
    assert_eq!(datime_string(0), None);
    assert_eq!(describe_compress(505), "ZSTD level 5");
    assert_eq!(describe_compress(0), "none");
    assert_eq!(describe_compress(101), "zlib level 1");
}

/// A key list of 100 000 keys under a directory with a 1000-byte path: each key used to get
/// its own copy of the path (here 100 MB; terabytes with a longer name and more keys).
#[test]
fn directory_paths_are_bounded() {
    fn key(out: &mut Vec<u8>) {
        out.extend_from_slice(&100i32.to_be_bytes()); // nbytes
        out.extend_from_slice(&4i16.to_be_bytes()); // version
        out.extend_from_slice(&0i32.to_be_bytes()); // objlen
        out.extend_from_slice(&0u32.to_be_bytes()); // datime
        out.extend_from_slice(&40i16.to_be_bytes()); // keylen
        out.extend_from_slice(&1i16.to_be_bytes()); // cycle
        out.extend_from_slice(&0i32.to_be_bytes()); // seek
        out.extend_from_slice(&0i32.to_be_bytes()); // pdir
        for s in [&b"TH1F"[..], b"h", b""] {
            out.push(s.len() as u8);
            out.extend_from_slice(s);
        }
    }
    let n = 100_000i32;
    let mut list = vec![0u8; 8];
    key(&mut list);
    list.extend_from_slice(&n.to_be_bytes());
    for _ in 0..n {
        key(&mut list);
    }
    let fixture = fixture("types.root");
    let mut f = RootFile::open(&fixture).expect("open");
    let mut seen = HashSet::new();
    let r = f.read_dir(&list, 8, "d".repeat(1000), 1, &mut seen);
    assert!(matches!(r, Err(Error::Corrupt(_))), "{r:?}");
}
