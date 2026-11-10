//! The fetched DICOM whole-slide series (tools/corpus/fetch-dicomwsi.sh: a few small IDC slides)
//! against the wsidicom oracle written beside them (oracle/<series>.tsv and the reference regions
//! oracle/<series>/L<i>.rgb). Reads SPECULAR_DICOMWSI_CORPUS, else tools/corpus/dicomwsi; skips
//! when neither has an oracle folder.
//!
//! The slides are JPEG and JPEG 2000, decoded here and by wsidicom's decoders (Pillow, libjpeg,
//! OpenJPEG), so regions match within a tolerance: the share of pixels with a channel more than
//! 8 levels apart and the mean absolute difference. Sizes, levels and MPP match exactly. Prints
//! open and first-paint times (a fit-to-1024 read), which are wall-clock and load dependent.

use std::path::{Path, PathBuf};
use std::time::Instant;

use specular_wsi::{Files, ReadOptions, Region, Slide};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_DICOMWSI_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/dicomwsi")
        });
    dir.join("oracle").is_dir().then_some(dir)
}

struct Open {
    slide: Slide,
    data: Vec<Vec<u8>>,
}

impl Open {
    fn read(&self, scale: u32, region: Option<Region>) -> specular_wsi::Rgba {
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
}

/// Opens a series folder from its first file (by name), as opening that file in the viewer
/// does; returns the slide and the open time in milliseconds (reading the files included).
fn open(dir: &Path) -> (Result<Open, String>, f64) {
    let t = Instant::now();
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.ends_with(".dcm"))
        .collect();
    names.sort();
    let m: Vec<(String, Vec<u8>)> = names
        .into_iter()
        .map(|n| {
            let d = std::fs::read(dir.join(&n)).unwrap();
            (n, d)
        })
        .collect();
    let refs: Vec<(&str, &[u8])> = m.iter().map(|(n, d)| (n.as_str(), d.as_slice())).collect();
    let r = Slide::open_dicom(&refs).map(|slide| {
        let mut data = vec![m[0].1.clone()];
        for name in &slide.files {
            data.push(m.iter().find(|(n, _)| n == name).unwrap().1.clone());
        }
        Open { slide, data }
    });
    (
        r.map_err(|e| e.to_string()),
        t.elapsed().as_secs_f64() * 1e3,
    )
}

#[derive(Default, Debug)]
struct Tally {
    levels: usize,
    level_mismatches: usize,
    regions: usize,
    pixels: usize,
    far_pixels: usize,
    abs_sum: u64,
}

#[test]
fn idc_series_match_wsidicom() {
    let Some(dir) = corpus() else {
        eprintln!("no DICOM WSI corpus (tools/corpus/fetch-dicomwsi.sh); skipping");
        return;
    };
    let mut series: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir() && p.file_name().is_some_and(|n| n != "oracle"))
        .collect();
    series.sort();
    let mut total = Tally::default();
    let mut failures = Vec::new();
    for s in &series {
        let name = s.file_name().unwrap().to_str().unwrap().to_owned();
        let (o, open_ms) = open(s);
        let tsv = dir.join("oracle").join(format!("{name}.tsv"));
        let lines: Vec<Vec<String>> = std::fs::read_to_string(&tsv)
            .map(|t| {
                t.lines()
                    .map(|l| l.split('\t').map(str::to_owned).collect())
                    .collect()
            })
            .unwrap_or_default();
        let o = match o {
            Ok(o) => o,
            Err(e) => {
                eprintln!("{name}: not opened ({e}), oracle lines {}", lines.len());
                // A series without a VOLUME (wsidicom refuses it too) may be refused.
                if !lines.is_empty() {
                    failures.push(format!("{name}: {e}"));
                }
                continue;
            }
        };
        let sl = &o.slide;
        // First paint: the whole slide fitted to 1024 x 1024.
        let t = Instant::now();
        let mut fit = 1u32;
        while sl.width.div_ceil(fit) > 1024 || sl.height.div_ceil(fit) > 1024 {
            fit *= 2;
        }
        let first = o.read(fit, None);
        let paint_ms = t.elapsed().as_secs_f64() * 1e3;
        let mut tally = Tally::default();
        for l in &lines {
            let n = |i: usize| l[i].parse::<u32>().unwrap();
            match l[0].as_str() {
                "slide" => {
                    assert_eq!((sl.width, sl.height), (n(1), n(2)), "{name}: size");
                    let (mx, my) = sl.props.mpp.expect("mpp");
                    let (ox, oy): (f64, f64) = (l[4].parse().unwrap(), l[5].parse().unwrap());
                    assert!(
                        (mx - ox).abs() <= 1e-5 * ox && (my - oy).abs() <= 1e-5 * oy,
                        "{name}: mpp {mx} {my} vs {ox} {oy}"
                    );
                }
                "level" => {
                    tally.levels += 1;
                    if !sl
                        .levels
                        .iter()
                        .any(|v| (v.width, v.height) == (n(2), n(3)))
                    {
                        eprintln!("{name}: no level {}x{}", n(2), n(3));
                        tally.level_mismatches += 1;
                    }
                }
                "region" => {
                    let i = n(1) as usize;
                    let (lw, lh) = lines
                        .iter()
                        .find(|k| k[0] == "level" && k[1] == l[1])
                        .map(|k| (k[2].parse::<u32>().unwrap(), k[3].parse::<u32>().unwrap()))
                        .unwrap();
                    let Some(lv) = sl.levels.iter().find(|v| (v.width, v.height) == (lw, lh))
                    else {
                        continue;
                    };
                    // Levels are a floor or ceiling of a power-of-two division (the downsample
                    // from Pixel Spacing is not exactly one).
                    let scale = 1u32 << lv.downsample.log2().round() as u32;
                    assert!(
                        (lv.downsample / f64::from(scale) - 1.0).abs() < 0.02,
                        "{name}: level {i} downsample {}",
                        lv.downsample
                    );
                    let r = Region {
                        x: n(2),
                        y: n(3),
                        w: n(4),
                        h: n(5),
                    };
                    let img = o.read(scale, Some(r));
                    let want = std::fs::read(dir.join("oracle").join(&name).join(&l[6])).unwrap();
                    assert_eq!(want.len(), r.w as usize * r.h as usize * 3);
                    tally.regions += 1;
                    let grey = |p: &[u8]| p[0] == p[1] && p[1] == p[2];
                    if img.data.chunks_exact(4).all(grey) && want.chunks_exact(3).all(grey) {
                        // Grey over 8 bits is windowed for display here and cut to 8 bits by
                        // wsidicom: the same samples, up to a non-decreasing mapping.
                        let mut map = vec![[0u32; 256]; 256];
                        for (a, b) in img.data.chunks_exact(4).zip(want.chunks_exact(3)) {
                            map[usize::from(b[0])][usize::from(a[0])] += 1;
                        }
                        let mut prev = 0;
                        for row in &map {
                            let total: u32 = row.iter().sum();
                            if total == 0 {
                                continue;
                            }
                            let (mode, &n) = row.iter().enumerate().max_by_key(|e| e.1).unwrap();
                            tally.pixels += total as usize;
                            tally.far_pixels += (total - n) as usize;
                            if mode < prev {
                                tally.far_pixels += n as usize;
                            }
                            prev = prev.max(mode);
                        }
                        continue;
                    }
                    for (a, b) in img.data.chunks_exact(4).zip(want.chunks_exact(3)) {
                        let d: Vec<u8> = (0..3).map(|c| a[c].abs_diff(b[c])).collect();
                        tally.pixels += 1;
                        tally.abs_sum += d.iter().map(|&x| u64::from(x)).sum::<u64>();
                        if d.iter().any(|&x| x > 8) {
                            tally.far_pixels += 1;
                        }
                    }
                }
                _ => {}
            }
        }
        let mad = tally.abs_sum as f64 / (tally.pixels.max(1) * 3) as f64;
        let far = tally.far_pixels as f64 / tally.pixels.max(1) as f64;
        eprintln!(
            "{name}: {}x{}, {} levels (oracle {}), {} level mismatches, {} regions, {} of {} pixels \
             more than 8 apart ({:.4}%), mean abs diff {mad:.3}; open {open_ms:.1} ms, first paint \
             1/{fit} {}x{} {paint_ms:.1} ms",
            sl.width,
            sl.height,
            sl.levels.len(),
            tally.levels,
            tally.level_mismatches,
            tally.regions,
            tally.far_pixels,
            tally.pixels,
            far * 100.0,
            first.width,
            first.height,
        );
        if tally.level_mismatches > 0 || far > 0.01 || mad > 1.5 {
            failures.push(format!("{name}: {tally:?}"));
        }
        total.levels += tally.levels;
        total.level_mismatches += tally.level_mismatches;
        total.regions += tally.regions;
        total.pixels += tally.pixels;
        total.far_pixels += tally.far_pixels;
        total.abs_sum += tally.abs_sum;
    }
    eprintln!("total: {total:?}");
    assert!(failures.is_empty(), "{failures:#?}");
}
