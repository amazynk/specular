//! Writes what `tools/corpus/pathology-oracle.py` checks against scikit-image, pycocotools,
//! shapely and tifffile. Ignored; run with
//! `SPECULAR_PATHOLOGY_ORACLE=<dir> cargo test -p specular-pathology --test oracle_dump -- --ignored`.

use std::path::{Path, PathBuf};

use specular_pathology::annot::{Annotation, Class, Geometry, Poly, classes};
use specular_pathology::mask::{self, MaskJob, Region};
use specular_pathology::stain::{self, Mode, StainSet, Stains};
use specular_pathology::{coco, qupath};

fn dir() -> Option<PathBuf> {
    std::env::var_os("SPECULAR_PATHOLOGY_ORACLE").map(PathBuf::from)
}

fn f64s(path: &Path, v: &[f64]) {
    let b: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
    std::fs::write(path, b).unwrap();
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// Pixels: every grey level, the cube's corners, then random colours.
fn pixels() -> Vec<[u8; 3]> {
    let mut v: Vec<[u8; 3]> = (0..=255).map(|g| [g, g, g]).collect();
    for c in 0..8u8 {
        v.push([0, 1, 2].map(|k| if c >> k & 1 == 1 { 255 } else { 0 }));
    }
    let mut r = Rng(0x2545_f491_4f6c_dd1d);
    while v.len() < 64 * 64 {
        let x = r.next();
        v.push([x as u8, (x >> 8) as u8, (x >> 16) as u8]);
    }
    v
}

#[test]
#[ignore]
fn dump() {
    let Some(out) = dir() else {
        eprintln!("SPECULAR_PATHOLOGY_ORACLE not set");
        return;
    };
    std::fs::create_dir_all(&out).unwrap();
    // Stains: the pixels, each set's matrix and our concentrations and displays.
    let px = pixels();
    std::fs::write(
        out.join("pixels.rgb"),
        px.iter().flatten().copied().collect::<Vec<u8>>(),
    )
    .unwrap();
    for set in StainSet::ALL {
        let s = Stains::published(set);
        f64s(
            &out.join(format!("{}-matrix.f64", set.id())),
            &s.vectors.concat(),
        );
        let u = s.unmix().unwrap();
        let c: Vec<f64> = px
            .iter()
            .flat_map(|p| stain::concentrations(p.map(|v| f64::from(v) / 255.0), &u))
            .collect();
        f64s(&out.join(format!("{}-conc.f64", set.id())), &c);
        let d = stain::display(&s, None).unwrap();
        let one: Vec<f64> = px
            .iter()
            .flat_map(|p| {
                stain::render(
                    p.map(|v| f64::from(v) / 255.0),
                    &d,
                    [true, false, false],
                    Mode::Intensity,
                )
            })
            .collect();
        f64s(&out.join(format!("{}-h-intensity.f64", set.id())), &one);
    }
    // Macenko on a synthetic slide of known vectors.
    let (a, b) = (stain::HEMATOXYLIN, stain::DAB);
    let mut r = Rng(7);
    let slide: Vec<[u8; 3]> = (0..20_000)
        .map(|_| {
            let mix = (r.next() % 1000) as f64 / 1000.0;
            let amount = 0.4 + (r.next() % 1000) as f64 / 1000.0 * 0.8;
            let od: Vec<f64> = (0..3)
                .map(|k| amount * (mix * a[k] + (1.0 - mix) * b[k]))
                .collect();
            [0, 1, 2].map(|k| (10f64.powf(-od[k]) * 255.0).round() as u8)
        })
        .collect();
    std::fs::write(
        out.join("macenko.rgb"),
        slide.iter().flatten().copied().collect::<Vec<u8>>(),
    )
    .unwrap();
    let e = stain::macenko(&slide).unwrap();
    f64s(
        &out.join("macenko.f64"),
        &[
            e.vectors[0].as_slice(),
            e.vectors[1].as_slice(),
            e.max_c.as_slice(),
        ]
        .concat(),
    );

    // Annotations: COCO, GeoJSON, per-annotation masks and an OME-TIFF pyramid.
    let class = |n: &str, c: [u8; 3]| {
        Some(Class {
            name: n.into(),
            colour: Some(c),
        })
    };
    let mut holed = Poly::new(vec![
        (40.0, 30.0),
        (210.0, 25.5),
        (230.0, 160.0),
        (35.0, 150.0),
    ]);
    holed.holes.push(vec![
        (80.0, 60.0),
        (150.0, 62.0),
        (140.0, 120.0),
        (90.0, 110.0),
    ]);
    let mut items = vec![
        Annotation {
            name: "tumour".into(),
            class: class("Tumor", [200, 0, 0]),
            geometry: Geometry::Area(vec![holed]),
        },
        Annotation {
            name: String::new(),
            class: class("Stroma", [0, 150, 0]),
            geometry: Geometry::rect((250.0, 10.0), (310.5, 90.25)),
        },
        Annotation {
            name: "two parts".into(),
            class: class("Necrosis", [90, 90, 90]),
            geometry: Geometry::Area(vec![
                Poly::new(vec![(20.0, 180.0), (60.0, 175.0), (45.0, 230.0)]),
                Poly::new(vec![
                    (100.0, 190.0),
                    (150.0, 185.0),
                    (160.0, 235.0),
                    (95.0, 228.0),
                ]),
            ]),
        },
        Annotation {
            name: "nucleus".into(),
            class: None,
            geometry: Geometry::Point((300.5, 200.25)),
        },
    ];
    // A star and a thin sliver, for the rasteriser's edges.
    let star: Vec<(f64, f64)> = (0..10)
        .map(|i| {
            let t = std::f64::consts::PI * f64::from(i) / 5.0;
            let rad = if i % 2 == 0 { 40.0 } else { 16.0 };
            (270.0 + rad * t.sin(), 160.0 - rad * t.cos())
        })
        .collect();
    items.push(Annotation {
        name: "star".into(),
        class: class("Tumor", [200, 0, 0]),
        geometry: Geometry::polygon(star),
    });
    items.push(Annotation {
        name: "sliver".into(),
        class: class("Stroma", [0, 150, 0]),
        geometry: Geometry::polygon(vec![(5.0, 5.0), (300.0, 7.3), (5.0, 8.1)]),
    });
    let image = coco::Image {
        file_name: "synthetic.svs".into(),
        width: 320,
        height: 240,
    };
    let (text, _) = coco::write(&items, &image, "Specular oracle", "2026-10-25T00:00:00Z");
    std::fs::write(out.join("coco.json"), text).unwrap();
    std::fs::write(out.join("qupath.geojson"), qupath::write(&items).0).unwrap();
    let cls = classes(&items);
    let region = Region {
        x: 0.0,
        y: 0.0,
        w: 320.0,
        h: 240.0,
    };
    for (i, a) in items.iter().enumerate() {
        let mut m = vec![0u16; 320 * 240];
        mask::paint(
            std::slice::from_ref(a),
            &cls,
            &region,
            1.0,
            0,
            0,
            320,
            240,
            &mut m,
        );
        let b: Vec<u8> = m.iter().map(|v| u8::from(*v > 0)).collect();
        std::fs::write(out.join(format!("mask-{i}.u8")), b).unwrap();
    }
    let job = MaskJob {
        region,
        downsample: 1.0,
        tile: 64,
        skip_empty: false,
    };
    let mut f = std::fs::File::create(out.join("labels.ome.tif")).unwrap();
    mask::write_ome_tiff(&items, &job, "synthetic labels", Some(0.5), &mut f).unwrap();
    let mut whole = vec![0u16; 320 * 240];
    mask::paint(&items, &cls, &region, 1.0, 0, 0, 320, 240, &mut whole);
    std::fs::write(
        out.join("labels.u8"),
        whole.iter().map(|v| *v as u8).collect::<Vec<u8>>(),
    )
    .unwrap();
}

/// Wall-clock timings of the heavier paths (dev builds; ignored).
#[test]
#[ignore]
fn timings() {
    use std::time::Instant;
    let mut r = Rng(11);
    let px: Vec<[u8; 3]> = (0..1 << 20)
        .map(|_| {
            let mix = (r.next() % 1000) as f64 / 1000.0;
            let od: Vec<f64> = (0..3)
                .map(|k| 0.8 * (mix * stain::HEMATOXYLIN[k] + (1.0 - mix) * stain::EOSIN[k]) + 0.1)
                .collect();
            [0, 1, 2].map(|k| (10f64.powf(-od[k]) * 255.0).round() as u8)
        })
        .collect();
    let t = Instant::now();
    let e = stain::macenko(&px).unwrap();
    eprintln!(
        "macenko 1 Mpx: {:.0} ms ({} stained)",
        t.elapsed().as_secs_f64() * 1e3,
        e.pixels
    );
    // 20 000 cell outlines (16-gons) over a 40 000 × 30 000 slide.
    let items: Vec<Annotation> = (0..20_000)
        .map(|i| {
            let (cx, cy) = ((r.next() % 40_000) as f64, (r.next() % 30_000) as f64);
            let ring = (0..16)
                .map(|k| {
                    let a = std::f64::consts::TAU * f64::from(k) / 16.0;
                    (cx + 12.0 * a.cos(), cy + 12.0 * a.sin())
                })
                .collect();
            Annotation {
                name: String::new(),
                class: Some(Class {
                    name: if i % 2 == 0 {
                        "Tumor".into()
                    } else {
                        "Stroma".into()
                    },
                    colour: None,
                }),
                geometry: Geometry::polygon(ring),
            }
        })
        .collect();
    let image = coco::Image {
        file_name: "s.svs".into(),
        width: 40_000,
        height: 30_000,
    };
    let t = Instant::now();
    let (text, _) = coco::write(&items, &image, "t", "2026-01-01T00:00:00Z");
    eprintln!(
        "COCO 20k polygons: {:.0} ms, {} MB",
        t.elapsed().as_secs_f64() * 1e3,
        text.len() >> 20
    );
    let t = Instant::now();
    let g = qupath::write(&items).0;
    eprintln!(
        "GeoJSON 20k polygons: {:.0} ms, {} MB",
        t.elapsed().as_secs_f64() * 1e3,
        g.len() >> 20
    );
    let job = MaskJob {
        region: Region {
            x: 0.0,
            y: 0.0,
            w: 40_000.0,
            h: 30_000.0,
        },
        downsample: 4.0,
        tile: 512,
        skip_empty: false,
    };
    let t = Instant::now();
    let mut c = std::io::Cursor::new(Vec::new());
    mask::write_ome_tiff(&items, &job, "t", None, &mut c).unwrap();
    eprintln!(
        "OME-TIFF label pyramid 10 000 × 7 500 (level 2), 20k polygons: {:.0} ms, {} KB",
        t.elapsed().as_secs_f64() * 1e3,
        c.get_ref().len() >> 10
    );
}
