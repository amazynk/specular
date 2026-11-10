//! Label masks against shapes whose covered pixels are known exactly.

use super::*;
use crate::annot::{Class, Poly};

fn class(name: &str) -> Option<Class> {
    Some(Class {
        name: name.into(),
        colour: None,
    })
}

fn ann(name: &str, g: Geometry) -> Annotation {
    Annotation {
        name: String::new(),
        class: class(name),
        geometry: g,
    }
}

fn full(items: &[Annotation], region: Region, d: f64) -> (u32, u32, Vec<u16>) {
    let cls = classes(items);
    let (w, h) = region.size_at(d);
    let mut buf = vec![0u16; (w * h) as usize];
    paint(items, &cls, &region, d, 0, 0, w, h, &mut buf);
    (w, h, buf)
}

#[test]
fn rectangles_cover_their_pixels_exactly() {
    let items = [ann("A", Geometry::rect((2.0, 3.0), (7.0, 5.0)))];
    let (w, _, m) = full(
        &items,
        Region {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 8.0,
        },
        1.0,
    );
    for (i, v) in m.iter().enumerate() {
        let (x, y) = (i as u32 % w, i as u32 / w);
        let inside = (2..7).contains(&x) && (3..5).contains(&y);
        assert_eq!(*v, u16::from(inside), "{x},{y}");
    }
    assert_eq!(m.iter().filter(|v| **v == 1).count(), 10);
}

#[test]
fn holes_later_classes_and_points() {
    let mut p = Poly::new(vec![(0.0, 0.0), (8.0, 0.0), (8.0, 8.0), (0.0, 8.0)]);
    p.holes
        .push(vec![(2.0, 2.0), (6.0, 2.0), (6.0, 6.0), (2.0, 6.0)]);
    let items = [
        ann("Tumor", Geometry::Area(vec![p])),
        ann("Stroma", Geometry::rect((5.0, 5.0), (7.0, 7.0))),
        ann("Cell", Geometry::Point((3.5, 3.2))),
        ann("Line", Geometry::Line(vec![(0.0, 0.0), (8.0, 8.0)])),
    ];
    let (w, _, m) = full(
        &items,
        Region {
            x: 0.0,
            y: 0.0,
            w: 8.0,
            h: 8.0,
        },
        1.0,
    );
    let at = |x: u32, y: u32| m[(y * w + x) as usize];
    assert_eq!(at(0, 0), 1);
    assert_eq!(at(3, 4), 0, "the hole");
    assert_eq!(at(5, 5), 2, "painted over");
    assert_eq!(at(6, 6), 2);
    assert_eq!(at(7, 7), 1);
    assert_eq!(at(3, 3), 3, "the point's pixel");
    // Area: 64 − 16 = 48 pixels of class 1 before the others painted over 4 + 1.
    assert_eq!(m.iter().filter(|v| **v == 1).count(), 48 - 3);
}

#[test]
fn downsampled_and_offset_regions() {
    // A rectangle 100..300 × 50..150 at downsample 4 over a region starting at (20, 10).
    let items = [ann("A", Geometry::rect((100.0, 50.0), (300.0, 150.0)))];
    let region = Region {
        x: 20.0,
        y: 10.0,
        w: 400.0,
        h: 200.0,
    };
    let (w, h, m) = full(&items, region, 4.0);
    assert_eq!((w, h), (100, 50));
    // Covered: centres 20 + (i + 0.5)·4 in [100, 300) → i in 20..70; rows 10..35.
    for (k, v) in m.iter().enumerate() {
        let (i, j) = (k as u32 % w, k as u32 / w);
        let inside = (20..70).contains(&i) && (10..35).contains(&j);
        assert_eq!(*v, u16::from(inside), "{i},{j}");
    }
    // Tiles agree with the whole.
    let cls = classes(&items);
    let mut tile = vec![0u16; 32 * 16];
    paint(&items, &cls, &region, 4.0, 32, 16, 32, 16, &mut tile);
    for j in 0..16 {
        for i in 0..32 {
            assert_eq!(tile[j * 32 + i], m[(16 + j) * 100 + 32 + i]);
        }
    }
}

#[test]
fn diamond_matches_brute_force() {
    // A rotated square: brute force over pixel centres with the even-odd test.
    let ring = vec![(10.0, 0.5), (19.5, 10.0), (10.0, 19.5), (0.5, 10.0)];
    let items = [ann("D", Geometry::polygon(ring.clone()))];
    let (w, _, m) = full(
        &items,
        Region {
            x: 0.0,
            y: 0.0,
            w: 20.0,
            h: 20.0,
        },
        1.0,
    );
    let inside = |cx: f64, cy: f64| {
        let mut c = false;
        for k in 0..ring.len() {
            let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
            if (a.1 <= cy) != (b.1 <= cy) {
                let x = a.0 + (cy - a.1) * (b.0 - a.0) / (b.1 - a.1);
                if cx >= x {
                    c = !c;
                }
            }
        }
        c
    };
    for (k, v) in m.iter().enumerate() {
        let (i, j) = (k as u32 % w, k as u32 / w);
        assert_eq!(
            *v == 1,
            inside(f64::from(i) + 0.5, f64::from(j) + 0.5),
            "{i},{j}"
        );
    }
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("specular-pathology-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn png_tiles_with_class_table() {
    let items = [
        ann("Tumor", Geometry::rect((0.0, 0.0), (40.0, 40.0))),
        ann("Stroma", Geometry::rect((60.0, 60.0), (100.0, 100.0))),
    ];
    let dir = tmp("png_tiles");
    let job = MaskJob {
        region: Region {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
        },
        downsample: 1.0,
        tile: 50,
        skip_empty: true,
    };
    let out = write_tiles(&items, &job, TileFormat::Png, &dir, "slide").unwrap();
    // Four tiles; the top-right and bottom-left are empty.
    assert_eq!(out.empty_skipped, 2);
    assert_eq!(out.files.len(), 3);
    let first = std::fs::read(dir.join("slide_x0_y0.png")).unwrap();
    let img = specular_png::decode(&first, &Default::default()).unwrap();
    assert_eq!((img.width, img.height), (50, 50));
    let px = img.to_rgba8();
    assert_eq!(px[0], 1);
    assert_eq!(px[(45 * 50 + 45) * 4], 0);
    let last = specular_png::decode(
        &std::fs::read(dir.join("slide_x50_y50.png")).unwrap(),
        &Default::default(),
    )
    .unwrap();
    assert_eq!(last.to_rgba8()[(20 * 50 + 20) * 4], 2);
    let table: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("slide-classes.json")).unwrap())
            .unwrap();
    assert_eq!(table["classes"][1]["name"], "Stroma");
    assert_eq!(table["classes"][1]["index"], 2);
    // TIFF tiles too.
    let out = write_tiles(&items, &job, TileFormat::Tiff, &dir, "t").unwrap();
    assert_eq!(out.files.len(), 3);
    let t = std::fs::read(dir.join("t_x0_y0.tif")).unwrap();
    let dirs = specular_tiff::Directories::read(&t).unwrap();
    let ifd = dirs.dirs[0].ifd.as_ref().unwrap();
    assert_eq!((ifd.width, ifd.height, ifd.bits), (50, 50, 8));
    assert!(!dirs.big_tiff);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The full-resolution samples of level `l` of a tiled grey TIFF.
fn level(data: &[u8], ifd: &specular_tiff::Ifd, tile: u32) -> Vec<u8> {
    let (w, h) = (ifd.width, ifd.height);
    let nx = w.div_ceil(tile);
    let mut out = vec![0u8; (w * h) as usize];
    for (n, (&o, &c)) in ifd.unit_offsets().iter().zip(ifd.unit_counts()).enumerate() {
        let raw =
            specular_png::inflate_zlib(&data[o as usize..(o + c) as usize], (tile * tile) as usize)
                .unwrap();
        let (tx, ty) = (n as u32 % nx, n as u32 / nx);
        for y in 0..tile {
            for x in 0..tile {
                let (gx, gy) = (tx * tile + x, ty * tile + y);
                if gx < w && gy < h {
                    out[(gy * w + gx) as usize] = raw[(y * tile + x) as usize];
                }
            }
        }
    }
    out
}

#[test]
fn ome_tiff_pyramid_levels_are_rasterised_at_their_scale() {
    let items = [ann("A", Geometry::rect((64.0, 32.0), (320.0, 160.0)))];
    let job = MaskJob {
        region: Region {
            x: 0.0,
            y: 0.0,
            w: 600.0,
            h: 300.0,
        },
        downsample: 1.0,
        tile: 128,
        skip_empty: false,
    };
    let mut c = io::Cursor::new(Vec::new());
    write_ome_tiff(&items, &job, "slide labels", Some(0.25), &mut c).unwrap();
    let data = c.into_inner();
    let dirs = specular_tiff::Directories::read(&data).unwrap();
    assert!(dirs.big_tiff);
    // 600×300, 300×150, 150×75 (fits a 128 tile? no: 150 > 128), 75×38.
    let sizes: Vec<(u32, u32)> = dirs
        .dirs
        .iter()
        .map(|d| d.ifd.as_ref().map_or((0, 0), |i| (i.width, i.height)))
        .collect();
    assert_eq!(sizes, [(600, 300), (300, 150), (150, 75), (75, 38)]);
    assert!(dirs.dirs[1..].iter().all(|d| d.parent == Some(0)));
    let desc = String::from_utf8_lossy(&data);
    assert!(
        desc.contains("<OME")
            && desc.contains("PhysicalSizeX=\"0.25\"")
            && desc.contains("<M K=\"1\">A</M>")
    );
    for (l, d) in dirs.dirs.iter().enumerate() {
        let ifd = d.ifd.as_ref().unwrap();
        let px = level(&data, ifd, 128);
        let s = f64::from(1u32 << l);
        let cover = |lo: f64, hi: f64, i: u32| {
            let c = (f64::from(i) + 0.5) * s;
            c >= lo && c < hi
        };
        for (k, v) in px.iter().enumerate() {
            let (i, j) = (k as u32 % ifd.width, k as u32 / ifd.width);
            let want = cover(64.0, 320.0, i) && cover(32.0, 160.0, j);
            assert_eq!(*v == 1, want, "level {l} {i},{j}");
        }
    }
}

#[test]
fn refusals() {
    let lines = [ann("L", Geometry::Line(vec![(0.0, 0.0), (1.0, 1.0)]))];
    let job = MaskJob {
        region: Region {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
        },
        downsample: 1.0,
        tile: 8,
        skip_empty: false,
    };
    let dir = tmp("refusals");
    assert!(matches!(
        write_tiles(&lines, &job, TileFormat::Png, &dir, "x"),
        Err(MaskError::NoAreas)
    ));
    let items = [ann("A", Geometry::rect((0.0, 0.0), (1.0, 1.0)))];
    let bad = MaskJob {
        tile: 0,
        ..job.clone()
    };
    assert!(matches!(
        write_tiles(&items, &bad, TileFormat::Png, &dir, "x"),
        Err(MaskError::BadTile)
    ));
    let many = MaskJob {
        region: Region {
            x: 0.0,
            y: 0.0,
            w: 1e7,
            h: 1e7,
        },
        tile: 16,
        ..job
    };
    assert!(matches!(
        write_tiles(&items, &many, TileFormat::Png, &dir, "x"),
        Err(MaskError::TooManyTiles(_))
    ));
    let _ = std::fs::remove_dir_all(&dir);
}
