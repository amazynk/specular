//! The fetched corpus (tools/corpus/fetch-wsi.sh: OpenSlide's test slides, OME's OME-TIFF
//! samples, OME-Zarr stores written by zarr-python) against the oracle written beside it
//! (openslide-python, tifffile, zarr). Reads SPECULAR_WSI_CORPUS, else tools/corpus/wsi; skips
//! when neither has an expected.tsv.
//!
//! Slide regions are JPEG / JPEG 2000 decodes by different decoders, so they match within a
//! tolerance (mean absolute difference and the share of clearly different samples); metadata
//! and OME / Zarr plane values match exactly.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_wsi::{AssociatedKind, Files, Position, ReadOptions, Region, Slide, Vendor, zarr};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_WSI_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/wsi"));
    dir.join("expected.tsv").is_file().then_some(dir)
}

/// A slide with the bytes of its files (the opened one first).
struct Open {
    slide: Slide,
    data: Vec<Vec<u8>>,
}

impl Open {
    fn files(&self) -> Vec<&[u8]> {
        self.data.iter().map(Vec::as_slice).collect()
    }
}

fn open(dir: &Path, rel: &str) -> Result<Open, String> {
    let path = dir.join(rel);
    if path.is_dir() {
        let slide =
            Slide::open_zarr(Arc::new(zarr::DirStore::new(&path))).map_err(|e| e.to_string())?;
        return Ok(Open {
            slide,
            data: vec![Vec::new()],
        });
    }
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    if bytes.starts_with(b"PK") {
        let store = zarr::ZipStore::new(bytes).map_err(|e| e.to_string())?;
        let slide = Slide::open_zarr(Arc::new(store)).map_err(|e| e.to_string())?;
        return Ok(Open {
            slide,
            data: vec![Vec::new()],
        });
    }
    let name = path.file_name().unwrap().to_str().unwrap();
    let folder = path.parent().unwrap();
    let slide = match specular_wsi::ome_metadata_file(&bytes) {
        Some(m) => {
            let xml = std::fs::read_to_string(folder.join(m)).map_err(|e| e.to_string())?;
            Slide::open_tiff_with_metadata(&bytes, name, &xml)
        }
        None => Slide::open_tiff_named(&bytes, name),
    }
    .map_err(|e| e.to_string())?;
    let mut data = vec![bytes];
    for f in &slide.files {
        data.push(std::fs::read(folder.join(f)).unwrap_or_default());
    }
    Ok(Open { slide, data })
}

/// The largest mean absolute difference of a region. Two reference JPEG 2000 decoders differ
/// by 3.09 on the busiest tile of CMU's JP2K-33003-1 at 1/4 (imagecodecs' against
/// OpenSlide's); a misplaced or wrongly decoded region also shows as many far samples.
const MAX_MAD: f64 = 3.5;

fn ro(scale: u32, region: Region, position: Position) -> ReadOptions<'static> {
    ReadOptions {
        scale,
        region: Some(region),
        position,
        cancel: None,
        parallel: true,
    }
}

fn vendor_of(openslide: &str) -> Option<Vendor> {
    Some(match openslide {
        "aperio" => Vendor::Aperio,
        "hamamatsu" => Vendor::Hamamatsu,
        "leica" => Vendor::Leica,
        "generic-tiff" => Vendor::Generic,
        _ => return None,
    })
}

/// Mean absolute difference over the samples the mask keeps, and the share of them that
/// differ by more than 24.
fn compare(ours: &[u8], want: &[u8], mask: Option<&[u8]>) -> (f64, f64) {
    let (mut sum, mut far, mut n) = (0u64, 0u64, 0u64);
    for (i, (a, b)) in ours.chunks_exact(4).zip(want.chunks_exact(3)).enumerate() {
        if mask.is_some_and(|m| m[i] == 0) {
            continue;
        }
        for k in 0..3 {
            let d = u64::from(a[k].abs_diff(b[k]));
            sum += d;
            far += u64::from(d > 24);
            n += 1;
        }
    }
    if n == 0 {
        return (0.0, 0.0);
    }
    (sum as f64 / n as f64, far as f64 / n as f64)
}

#[test]
fn corpus_matches_the_oracle() {
    let Some(dir) = corpus() else {
        eprintln!("wsi corpus absent: run tools/corpus/fetch-wsi.sh");
        return;
    };
    let text = std::fs::read_to_string(dir.join("expected.tsv")).unwrap();
    let mut current: Option<(String, Result<Open, String>)> = None;
    let mut mismatches = Vec::new();
    let (mut checked, mut worst) = (0usize, 0f64);
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let f: Vec<&str> = line.split('\t').collect();
        let n = |i: usize| f[i].parse::<u32>().unwrap();
        let rel = f[1];
        if f[0] == "refuse" {
            // Neither oracle reads it; ours may or may not, but must not panic.
            let _ = open(&dir, rel);
            continue;
        }
        if current.as_ref().is_none_or(|c| c.0 != rel) {
            current = Some((rel.to_owned(), open(&dir, rel)));
        }
        let o = match &current.as_ref().unwrap().1 {
            Ok(o) => o,
            Err(e) => {
                mismatches.push(format!("{rel}: does not open: {e}"));
                continue;
            }
        };
        let s = &o.slide;
        let files = o.files();
        let files = Files { data: &files };
        checked += 1;
        let mut bad = |what: String| mismatches.push(format!("{rel}: {what}"));
        match f[0] {
            "slide" => {
                if vendor_of(f[2]) != Some(s.props.vendor) {
                    bad(format!("vendor {:?}, openslide {}", s.props.vendor, f[2]));
                }
                if (s.width, s.height) != (n(3), n(4)) {
                    bad(format!(
                        "size {}x{}, openslide {}x{}",
                        s.width, s.height, f[3], f[4]
                    ));
                }
                let mpp = f[5].parse::<f64>().ok();
                // OpenSlide's generic TIFF MPP is the resolution tags' (whatever their unit);
                // we only take a resolution in centimetres or inches.
                if f[2] != "generic-tiff"
                    && let Some(m) = mpp
                    && s.props.mpp.is_none_or(|(x, _)| (x - m).abs() > 1e-6 * m)
                {
                    bad(format!("MPP {:?}, openslide {m}", s.props.mpp));
                }
                let obj = f[6].parse::<f64>().ok();
                if obj.is_some() && s.props.objective != obj {
                    bad(format!(
                        "objective {:?}, openslide {:?}",
                        s.props.objective, obj
                    ));
                }
            }
            "region" => {
                let (scale, r) = (
                    n(2),
                    Region {
                        x: n(3),
                        y: n(4),
                        w: n(5),
                        h: n(6),
                    },
                );
                let want = std::fs::read(dir.join(f[7])).unwrap();
                let mask = std::fs::read(dir.join(format!("{}.mask", f[7]))).ok();
                match s.read(files, &ro(scale, r, Position::default())) {
                    Ok(px) if (px.width, px.height) == (r.w, r.h) => {
                        let (mad, far) = compare(&px.data, &want, mask.as_deref());
                        worst = worst.max(mad);
                        if mad > MAX_MAD || far > 0.01 {
                            bad(format!(
                                "1/{scale} {r:?}: mean difference {mad:.3}, {:.2}% far",
                                far * 100.0
                            ));
                        }
                    }
                    Ok(px) => bad(format!("1/{scale} {r:?}: read {}x{}", px.width, px.height)),
                    Err(e) => bad(format!("1/{scale} {r:?}: {e}")),
                }
            }
            "assoc" => {
                let kind = match f[2] {
                    "label" => AssociatedKind::Label,
                    "macro" => AssociatedKind::Macro,
                    "thumbnail" => AssociatedKind::Thumbnail,
                    k => AssociatedKind::Other(k.to_owned()),
                };
                let Some(i) = s.associated.iter().position(|a| a.0 == kind) else {
                    bad(format!("no {} image", f[2]));
                    continue;
                };
                let want = std::fs::read(dir.join(f[5])).unwrap();
                match s.read_associated(files, i, None) {
                    Ok(px) if (px.width, px.height) == (n(3), n(4)) => {
                        let (mad, far) = compare(&px.data, &want, None);
                        if mad > MAX_MAD || far > 0.01 {
                            bad(format!(
                                "{}: mean difference {mad:.3}, {:.2}% far",
                                f[2],
                                far * 100.0
                            ));
                        }
                    }
                    Ok(px) => bad(format!(
                        "{}: {}x{}, openslide {}x{}",
                        f[2], px.width, px.height, f[3], f[4]
                    )),
                    Err(e) => bad(format!("{}: {e}", f[2])),
                }
            }
            "image" => {
                let got = (
                    s.width,
                    s.height,
                    s.channels.len().max(1) as u32,
                    s.size_z,
                    s.size_t,
                );
                let want = (n(2), n(3), n(4), n(5), n(6));
                if got != want {
                    bad(format!("WxHxCxZxT {got:?}, oracle {want:?}"));
                }
            }
            "plane" => {
                let (c, z, t, scale) = (n(2), n(3), n(4), n(5));
                let r = Region {
                    x: n(6),
                    y: n(7),
                    w: n(8),
                    h: n(9),
                };
                let want: Vec<f32> = std::fs::read(dir.join(f[10]))
                    .unwrap()
                    .chunks_exact(4)
                    .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                    .collect();
                match s.read_channel(files, c, &ro(scale, r, Position { z, t })) {
                    Ok(p) if (p.width, p.height) == (r.w, r.h) => {
                        let diff = p.values.iter().zip(&want).filter(|(a, b)| a != b).count();
                        if let Some((i, (a, b))) = p
                            .values
                            .iter()
                            .zip(&want)
                            .enumerate()
                            .find(|(_, (a, b))| a != b)
                        {
                            bad(format!(
                                "c{c} z{z} t{t} 1/{scale} {r:?}: {diff} values differ, first at {i}: {a} vs {b}"
                            ));
                        }
                    }
                    Ok(p) => bad(format!(
                        "c{c} z{z} t{t} 1/{scale}: read {}x{}",
                        p.width, p.height
                    )),
                    Err(e) => bad(format!("c{c} z{z} t{t} 1/{scale} {r:?}: {e}")),
                }
            }
            k => panic!("unknown row {k}"),
        }
    }
    eprintln!(
        "wsi corpus: {checked} checks, {} mismatches, worst region mean difference {worst:.3}",
        mismatches.len()
    );
    for m in &mismatches {
        eprintln!("  {m}");
    }
    assert!(mismatches.is_empty(), "{} mismatches", mismatches.len());
}
