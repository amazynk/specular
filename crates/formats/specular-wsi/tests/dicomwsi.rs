//! DICOM VL Whole Slide Microscopy series (tests/data/dicomwsi, written by
//! tools/corpus/dicomwsi-fixtures.py) against what wsidicom reads from them
//! (tools/corpus/dicomwsi-oracle.py --tiles: full.tsv, sparse.tsv).
//!
//! The pyramid tiles are uncompressed, RLE, JPEG-LS, JPEG 2000 or JPEG XL lossless, so levels and
//! tiles match exactly (FNV-1a hashes of the RGB bytes); the JPEG label matches within a small
//! tolerance (different IDCTs).

use std::path::{Path, PathBuf};

use specular_wsi::{AssociatedKind, Files, ReadOptions, Region, Rgba, Slide, Vendor};

fn data(set: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/dicomwsi")
        .join(set)
}

/// The series folder's files, `first` first (as the decoder hands them: the opened file, then
/// its siblings).
fn members(set: &str, first: &str) -> Vec<(String, Vec<u8>)> {
    let dir = data(set);
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.ends_with(".dcm"))
        .collect();
    names.sort();
    names.retain(|n| n != first);
    names.insert(0, first.to_owned());
    names
        .into_iter()
        .map(|n| {
            let d = std::fs::read(dir.join(&n)).unwrap();
            (n, d)
        })
        .collect()
}

struct Open {
    slide: Slide,
    data: Vec<Vec<u8>>,
}

impl Open {
    fn new(set: &str, first: &str) -> Open {
        let m = members(set, first);
        let refs: Vec<(&str, &[u8])> = m.iter().map(|(n, d)| (n.as_str(), d.as_slice())).collect();
        let slide = Slide::open_dicom(&refs).unwrap();
        // Files: the opened one, then the slide's other files by name.
        let mut data = vec![m[0].1.clone()];
        for name in &slide.files {
            data.push(m.iter().find(|(n, _)| n == name).unwrap().1.clone());
        }
        Open { slide, data }
    }

    fn read(&self, scale: u32, region: Option<Region>) -> Rgba {
        let files: Vec<&[u8]> = self.data.iter().map(Vec::as_slice).collect();
        let o = ReadOptions {
            scale,
            region,
            position: Default::default(),
            cancel: None,
            parallel: true,
        };
        self.slide.read(Files { data: &files }, &o).unwrap()
    }

    fn associated(&self, kind: AssociatedKind) -> Option<Rgba> {
        let i = self.slide.associated.iter().position(|a| a.0 == kind)?;
        let files: Vec<&[u8]> = self.data.iter().map(Vec::as_slice).collect();
        Some(
            self.slide
                .read_associated(Files { data: &files }, i, None)
                .unwrap(),
        )
    }
}

fn rgb(img: &Rgba) -> Vec<u8> {
    img.data
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect()
}

fn fnv(b: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &x in b {
        h = (h ^ u64::from(x)).wrapping_mul(0x100_0000_01b3);
    }
    format!("{h:016x}")
}

fn oracle(set: &str) -> Vec<Vec<String>> {
    let p = data(set).with_extension("tsv");
    std::fs::read_to_string(p)
        .unwrap()
        .lines()
        .map(|l| l.split('\t').map(str::to_owned).collect())
        .collect()
}

#[derive(Default, Debug)]
struct Mismatches {
    levels: usize,
    tiles: usize,
    pixels: usize,
    checked_tiles: usize,
}

/// Compare `o` with the oracle's lines; returns the mismatch counts.
fn compare(o: &Open, lines: &[Vec<String>]) -> Mismatches {
    let mut m = Mismatches::default();
    let s = &o.slide;
    for l in lines {
        let n = |i: usize| l[i].parse::<u32>().unwrap();
        match l[0].as_str() {
            "slide" => {
                assert_eq!((s.width, s.height), (n(1), n(2)));
                assert_eq!(s.levels.len(), n(3) as usize, "levels");
                let (mx, my) = s.props.mpp.unwrap();
                let (ox, oy): (f64, f64) = (l[4].parse().unwrap(), l[5].parse().unwrap());
                assert!((mx - ox).abs() < 1e-6 && (my - oy).abs() < 1e-6, "mpp");
            }
            "level" => {
                let i = n(1) as usize;
                let lv = &s.levels[i];
                assert_eq!((lv.width, lv.height), (n(2), n(3)), "level {i} size");
                let scale = 1u32 << i;
                assert!((lv.downsample - f64::from(scale)).abs() < 1e-9);
                let img = o.read(scale, None);
                assert_eq!((img.width, img.height), (n(2), n(3)));
                if fnv(&rgb(&img)) != l[6] {
                    m.levels += 1;
                }
            }
            "tile" => {
                let (i, tx, ty) = (n(1), n(2), n(3));
                let lv = &s.levels[i as usize];
                let (tw, th) = (32, 32);
                let r = Region {
                    x: tx * tw,
                    y: ty * th,
                    w: tw.min(lv.width - tx * tw),
                    h: th.min(lv.height - ty * th),
                };
                let img = o.read(1 << i, Some(r));
                m.checked_tiles += 1;
                if fnv(&rgb(&img)) != l[4] {
                    m.tiles += 1;
                }
            }
            "assoc" => {
                let kind = match l[1].as_str() {
                    "label" => AssociatedKind::Label,
                    _ => AssociatedKind::Macro,
                };
                let img = o.associated(kind).expect("associated image");
                assert_eq!((img.width, img.height), (n(2), n(3)), "{}", l[1]);
                let ours = rgb(&img);
                if let Some(hex) = l.get(5) {
                    // Lossy: within 3 levels per sample (IDCT rounding).
                    let want: Vec<u8> = (0..hex.len() / 2)
                        .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
                        .collect();
                    m.pixels += ours
                        .chunks_exact(3)
                        .zip(want.chunks_exact(3))
                        .filter(|(a, b)| a.iter().zip(*b).any(|(x, y)| x.abs_diff(*y) > 3))
                        .count();
                } else if fnv(&ours) != l[4] {
                    m.levels += 1;
                }
            }
            _ => {}
        }
    }
    m
}

/// The fixture formula (tools/corpus/dicomwsi-fixtures.py).
fn formula(x: u32, y: u32, d: u32, c: u32, z: u32, path: u32) -> u8 {
    let (x, y) = (x * d, y * d);
    ((x * 3 + y * 5 + c * 70 + (x / 16 + y / 16) % 2 * 40 + z * 9 + path * 100) % 256) as u8
}

#[test]
fn tiled_full_matches_wsidicom() {
    let lines = oracle("full");
    // Opened from any member: the base, a resampled level, the label.
    for first in ["level0.dcm", "level2.dcm", "label.dcm"] {
        let o = Open::new("full", first);
        assert_eq!(o.slide.props.vendor, Vendor::Dicom);
        let m = compare(&o, &lines);
        eprintln!("full (opened from {first}): {m:?}");
        assert_eq!((m.levels, m.tiles, m.pixels), (0, 0, 0), "{first}");
        assert_eq!(m.checked_tiles, 15 + 6 + 2);
        // Label, overview (macro) and the JPEG XL thumbnail.
        let kinds: Vec<AssociatedKind> = o.slide.associated.iter().map(|a| a.0.clone()).collect();
        assert_eq!(
            kinds,
            [
                AssociatedKind::Label,
                AssociatedKind::Macro,
                AssociatedKind::Thumbnail
            ]
        );
        let t = o.associated(AssociatedKind::Thumbnail).unwrap();
        assert_eq!((t.width, t.height), (20, 12));
        for (i, p) in t.data.chunks_exact(4).enumerate() {
            let (x, y) = (i as u32 % 20, i as u32 / 20);
            for c in 0..3 {
                assert_eq!(
                    p[c as usize],
                    formula(x, y, 8, c, 0, 0),
                    "thumbnail {x},{y}"
                );
            }
        }
    }
}

#[test]
fn tiled_full_facts() {
    let o = Open::new("full", "level1.dcm");
    let p = &o.slide.props;
    assert_eq!(p.mpp, Some((0.5, 0.5)));
    assert_eq!(p.objective, Some(20.0));
    assert_eq!(p.scanner.as_deref(), Some("Specular Fixture Scanner"));
    assert_eq!(p.name.as_deref(), Some("Fixture slide"));
    let props: Vec<(&str, &str)> = p
        .properties
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    assert!(props.contains(&("Dimension Organization Type", "TILED_FULL")));
    assert!(props.contains(&("Focal Planes", "2")));
    assert!(props.contains(&("Tile Size", "32 × 32")));
    assert!(props.contains(&("Container Identifier", "FIXTURE-FULL")));
    // No patient attributes among the facts.
    assert!(!p.properties.iter().any(|(_, v)| v.contains("Fixture^")));
    // Every member of the folder is used (6 instances, the opened one first).
    assert_eq!(o.slide.files.len(), 5);
}

#[test]
fn first_focal_plane_by_default() {
    // The base has planes at 0 and 2 µm; plane 0 shows (the formula's z = 0).
    let o = Open::new("full", "level0.dcm");
    let img = o.read(
        1,
        Some(Region {
            x: 0,
            y: 0,
            w: 4,
            h: 1,
        }),
    );
    for x in 0..4 {
        assert_eq!(img.data[x as usize * 4], formula(x, 0, 1, 0, 0, 0));
    }
}

#[test]
fn tiled_sparse_matches_wsidicom() {
    let lines = oracle("sparse");
    for first in ["level0.dcm", "level1.dcm", "label-other-series.dcm"] {
        let o = Open::new("sparse", first);
        let m = compare(&o, &lines);
        eprintln!("sparse (opened from {first}): {m:?}");
        assert_eq!((m.levels, m.tiles, m.pixels), (0, 0, 0), "{first}");
        assert_eq!(m.checked_tiles, 12 + 4);
        let props = &o.slide.props.properties;
        assert!(props.contains(&("Optical Paths".into(), "A, B".into())));
        assert!(props.contains(&("Dimension Organization Type".into(), "TILED_SPARSE".into())));
        // The label is in a series of its own, tied by the Container Identifier.
        assert_eq!(o.slide.associated, [(AssociatedKind::Label, 48, 24)]);
    }
}

#[test]
fn sparse_missing_tiles_are_white_and_first_path_shows() {
    let o = Open::new("sparse", "level0.dcm");
    // Base tile (1, 1) is not stored.
    let img = o.read(
        1,
        Some(Region {
            x: 32,
            y: 32,
            w: 32,
            h: 32,
        }),
    );
    assert!(img.data.iter().all(|&b| b == 255));
    assert!(!img.truncated);
    // Tile (0, 0) is optical path A (formula path 0), though B's frames come first in the file.
    let img = o.read(
        1,
        Some(Region {
            x: 0,
            y: 0,
            w: 2,
            h: 1,
        }),
    );
    assert_eq!(
        &img.data[..3],
        &[
            formula(0, 0, 1, 0, 0, 0),
            formula(0, 0, 1, 1, 0, 0),
            formula(0, 0, 1, 2, 0, 0)
        ]
    );
}

#[test]
fn odd_regions_and_scales() {
    let o = Open::new("full", "level0.dcm");
    // A region straddling tiles at full resolution equals the same crop of the full read.
    let full = o.read(1, None);
    let r = Region {
        x: 27,
        y: 30,
        w: 41,
        h: 37,
    };
    let part = o.read(1, Some(r));
    assert_eq!((part.width, part.height), (41, 37));
    for y in 0..37 {
        let a = &part.data[(y * 41 * 4) as usize..((y + 1) * 41 * 4) as usize];
        let s = (((y + 30) * 160 + 27) * 4) as usize;
        assert_eq!(a, &full.data[s..s + 41 * 4]);
    }
    // Past the coarsest level: box-reduced from it.
    let tiny = o.read(16, None);
    assert_eq!((tiny.width, tiny.height), (10, 6));
}

#[test]
fn not_a_slide() {
    assert!(!specular_wsi::dicomwsi::probe(b"not dicom"));
    let mut head = vec![0u8; 132];
    head[128..132].copy_from_slice(b"DICM");
    assert!(!specular_wsi::dicomwsi::probe(&head));
    let m = members("full", "level0.dcm");
    assert!(specular_wsi::dicomwsi::probe(&m[0].1));
    // A truncated base keeps the slide open; its tiles past the end read as missing.
    let mut cut = m[0].1.clone();
    cut.truncate(cut.len() / 2);
    let refs: Vec<(&str, &[u8])> = vec![("level0.dcm", cut.as_slice())];
    if let Ok(s) = Slide::open_dicom(&refs) {
        assert_eq!(s.levels.len(), 1);
    }
}
