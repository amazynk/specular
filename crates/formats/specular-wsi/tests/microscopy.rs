//! The stitched-scan corpus (tools/corpus/fetch-microscopy.sh: OpenSlide's MIRAX and Ventana
//! test slides, OME's cellSens VSI samples) against the oracle written beside it
//! (openslide-python for MIRAX and synthetic BIF files, Bio-Formats' bfconvert for VSI,
//! tifffile for the raw BIF levels). Reads SPECULAR_MICROSCOPY_CORPUS, else
//! tools/corpus/microscopy; skips when it has no expected.tsv.
//!
//! JPEG and JPEG 2000 tiles are decoded by different decoders, so colour regions match
//! within a tolerance (mean absolute difference, share of clearly different samples) over
//! the pixels the oracle paints; raw and lossless planes match exactly. OpenSlide resamples
//! MIRAX images placed at fractional positions on the coarser levels (we place them at whole
//! pixels): those rows carry a looser bound.

use std::path::{Path, PathBuf};

use specular_wsi::{AssociatedKind, Files, Position, ReadOptions, Region, Slide, Vendor};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_MICROSCOPY_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/microscopy")
        });
    dir.join("expected.tsv").is_file().then_some(dir)
}

/// A slide with the bytes of its files (the opened one first).
pub struct Open {
    pub slide: Slide,
    pub data: Vec<Vec<u8>>,
}

impl Open {
    pub fn files(&self) -> Vec<&[u8]> {
        self.data.iter().map(Vec::as_slice).collect()
    }
}

/// The frame files of a VSI file, by path from its folder, sorted as the readers do.
fn vsi_frames(path: &Path) -> Vec<String> {
    let stem = path.file_stem().unwrap().to_str().unwrap();
    let dir = path.parent().unwrap().join(format!("_{stem}_"));
    let mut stacks: Vec<String> = std::fs::read_dir(&dir)
        .map(|d| {
            d.flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    stacks.sort();
    let mut out = Vec::new();
    for s in stacks {
        let mut f: Vec<String> = std::fs::read_dir(dir.join(&s))
            .map(|d| {
                d.flatten()
                    .filter_map(|e| e.file_name().into_string().ok())
                    .collect()
            })
            .unwrap_or_default();
        f.sort();
        for name in f {
            if name.starts_with("frame_") && name.ends_with(".ets") {
                out.push(format!("_{stem}_/{s}/{name}"));
            }
        }
    }
    out
}

pub fn open(dir: &Path, rel: &str) -> Result<Open, String> {
    let path = dir.join(rel);
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let folder = path.parent().unwrap();
    let e = |e: specular_wsi::Error| e.to_string();
    if rel.ends_with(".mrxs") {
        let slide_dir = folder.join(path.file_stem().unwrap());
        let ini = std::fs::read(slide_dir.join("Slidedat.ini")).map_err(|e| e.to_string())?;
        let index = specular_wsi::mrxs::index_file(&ini).unwrap_or_else(|| "Index.dat".into());
        let mut data = vec![
            bytes,
            ini.clone(),
            std::fs::read(slide_dir.join(index)).map_err(|e| e.to_string())?,
        ];
        for f in specular_wsi::mrxs::data_files(&ini).map_err(e)? {
            data.push(std::fs::read(slide_dir.join(f)).unwrap_or_default());
        }
        let files: Vec<&[u8]> = data.iter().map(Vec::as_slice).collect();
        let slide = Slide::open_mrxs(Files { data: &files }).map_err(e)?;
        return Ok(Open { slide, data });
    }
    if rel.ends_with(".vsi") {
        let names = vsi_frames(&path);
        let mut data = vec![bytes];
        for n in &names {
            data.push(std::fs::read(folder.join(n)).unwrap_or_default());
        }
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        let mut members: Vec<(&str, &[u8])> = vec![(name.as_str(), data[0].as_slice())];
        members.extend(
            names
                .iter()
                .map(String::as_str)
                .zip(data[1..].iter().map(Vec::as_slice)),
        );
        let slide = Slide::open_vsi(&members).map_err(e)?;
        return Ok(Open { slide, data });
    }
    let slide = Slide::open_tiff(&bytes).map_err(e)?;
    Ok(Open {
        slide,
        data: vec![bytes],
    })
}

fn ro(scale: u32, region: Region, position: Position) -> ReadOptions<'static> {
    ReadOptions {
        scale,
        region: Some(region),
        position,
        cancel: None,
        parallel: true,
    }
}

/// Mean absolute difference over the samples the mask keeps, and the share of them that
/// differ by more than 24.
fn compare(ours: &[u8], want: &[u8], mask: Option<&[u8]>) -> (f64, f64, u64) {
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
        return (0.0, 0.0, 0);
    }
    (sum as f64 / n as f64, far as f64 / n as f64, n)
}

/// The largest mean absolute difference of a colour region (JPEG decoders differ; where
/// the oracle blends overlapping images at fractional positions and we place them at whole
/// pixels, seams differ too).
const MAX_MAD: f64 = 3.5;
/// The largest share of samples further apart than 24.
const MAX_FAR: f64 = 0.01;

fn vendor_of(name: &str) -> Option<Vendor> {
    Some(match name {
        "mirax" => Vendor::Mirax,
        "ventana" => Vendor::Ventana,
        "olympus" => Vendor::Olympus,
        _ => return None,
    })
}

#[test]
fn corpus_matches_the_oracle() {
    // Starts the `ORACLE` lines on their own line (libtest prints `test NAME ... ` first).
    println!();
    let Some(dir) = corpus() else {
        eprintln!("microscopy corpus absent: run tools/corpus/fetch-microscopy.sh");
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
            if open(&dir, rel).is_ok() {
                mismatches.push(format!("{rel}: opens, the oracle refuses it"));
            }
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
                    bad(format!("vendor {:?}, oracle {}", s.props.vendor, f[2]));
                }
                if (s.width, s.height) != (n(3), n(4)) {
                    bad(format!(
                        "size {}x{}, oracle {}x{}",
                        s.width, s.height, f[3], f[4]
                    ));
                }
                if let Ok(m) = f[5].parse::<f64>()
                    && s.props.mpp.is_none_or(|(x, _)| (x - m).abs() > 1e-6 * m)
                {
                    bad(format!("MPP {:?}, oracle {m}", s.props.mpp));
                }
                let obj = f[6].parse::<f64>().ok();
                if obj.is_some() && s.props.objective != obj {
                    bad(format!("objective {:?}, oracle {obj:?}", s.props.objective));
                }
            }
            "levels" => {
                let want: Vec<(u32, u32)> = f[2]
                    .split(',')
                    .map(|p| {
                        let (w, h) = p.split_once('x').unwrap();
                        (w.parse().unwrap(), h.parse().unwrap())
                    })
                    .collect();
                let got: Vec<(u32, u32)> = s.levels.iter().map(|l| (l.width, l.height)).collect();
                if got != want {
                    bad(format!("levels {got:?}, oracle {want:?}"));
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
                // Where the oracle resamples images placed at fractional positions (MIRAX's
                // coarser levels), its pixels are blurred: a looser bound, which a misplaced
                // image still exceeds.
                let (max_mad, max_far) = match f.get(8).and_then(|v| v.parse::<f64>().ok()) {
                    Some(m) => (m, 0.2),
                    None => (MAX_MAD, MAX_FAR),
                };
                match s.read(files, &ro(scale, r, Position::default())) {
                    Ok(px) if (px.width, px.height) == (r.w, r.h) => {
                        let (mad, far, _) = compare(&px.data, &want, mask.as_deref());
                        worst = worst.max(mad);
                        if mad > max_mad || far > max_far {
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
                        let (mad, far, _) = compare(&px.data, &want, None);
                        if mad > MAX_MAD || far > MAX_FAR {
                            bad(format!(
                                "{}: mean difference {mad:.3}, {:.2}% far",
                                f[2],
                                far * 100.0
                            ));
                        }
                    }
                    Ok(px) => bad(format!(
                        "{}: {}x{}, oracle {}x{}",
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
                let tol: f32 = f.get(11).and_then(|v| v.parse().ok()).unwrap_or(0.0);
                match s.read_channel(files, c, &ro(scale, r, Position { z, t })) {
                    Ok(p) if (p.width, p.height) == (r.w, r.h) => {
                        let diff = p
                            .values
                            .iter()
                            .zip(&want)
                            .filter(|(a, b)| (*a - *b).abs() > tol)
                            .count();
                        if let Some((i, (a, b))) = p
                            .values
                            .iter()
                            .zip(&want)
                            .enumerate()
                            .find(|(_, (a, b))| (*a - *b).abs() > tol)
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
    // One line per file for tools/ci/oracle-pass-rates.sh: a file fails when any of its rows
    // does (every mismatch starts with the file's path).
    let rels: std::collections::BTreeSet<&str> = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .filter_map(|l| l.split('\t').nth(1))
        .collect();
    for rel in rels {
        let failed = mismatches
            .iter()
            .any(|m| m.starts_with(&format!("{rel}: ")));
        let ext = Path::new(rel)
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_uppercase())
            .unwrap_or_default();
        let status = if failed { "fail" } else { "pass" };
        println!("ORACLE\tWSI/{ext}\t{rel}\t{status}");
    }
    eprintln!(
        "microscopy corpus: {checked} checks, {} mismatches, worst region mean difference {worst:.3}",
        mismatches.len()
    );
    for m in &mismatches {
        eprintln!("  {m}");
    }
    assert!(mismatches.is_empty(), "{} mismatches", mismatches.len());
}

/// What each corpus slide opens as (`cargo test -- --ignored dump`).
#[test]
#[ignore]
fn dump() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/microscopy");
    for rel in [
        "mrxs/CMU-1-Saved-1_16.mrxs",
        "mrxs/CMU-1-Saved-1_2.mrxs",
        "mrxs/Mirax2-Fluorescence-2.mrxs",
        "bif/Ventana-1.bif",
        "bif/synthetic-right.bif",
        "vsi/metadataTest_01.vsi",
        "vsi/Image_V4.1_FL_2D.vsi",
        "vsi/Image_V4.1_BF.vsi",
    ] {
        match open(&dir, rel) {
            Ok(o) => {
                let s = &o.slide;
                eprintln!(
                    "{rel}: {:?} {}x{} levels {:?} C {} Z {} T {} assoc {:?} mpp {:?} obj {:?} sample {}",
                    s.props.vendor,
                    s.width,
                    s.height,
                    s.levels
                        .iter()
                        .map(|l| (l.width, l.height, l.downsample))
                        .collect::<Vec<_>>(),
                    s.channels.len(),
                    s.size_z,
                    s.size_t,
                    s.associated,
                    s.props.mpp,
                    s.props.objective,
                    s.props.sample
                );
                let files = o.files();
                let t0 = std::time::Instant::now();
                let sc = s.max_scale() / 8;
                let r = s.read(
                    Files { data: &files },
                    &ro(
                        sc,
                        Region {
                            x: 0,
                            y: 0,
                            w: s.width.div_ceil(sc),
                            h: s.height.div_ceil(sc),
                        },
                        Position::default(),
                    ),
                );
                eprintln!(
                    "   overview 1/{sc}: {:?} in {:?}",
                    r.as_ref()
                        .map(|r| (r.width, r.height, r.truncated))
                        .map_err(|e| e.to_string()),
                    t0.elapsed()
                );
            }
            Err(e) => eprintln!("{rel}: {e}"),
        }
    }
}

/// Write one region as raw RGB (`SPECULAR_REGION=file,scale,x,y,w,h,out`), for comparing by
/// eye or by script.
#[test]
#[ignore]
fn region_raw() {
    let Ok(spec) = std::env::var("SPECULAR_REGION") else {
        return;
    };
    let f: Vec<&str> = spec.split(',').collect();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/microscopy");
    let o = open(&dir, f[0]).unwrap();
    let n = |i: usize| f[i].parse::<u32>().unwrap();
    let files = o.files();
    let r = o
        .slide
        .read(
            Files { data: &files },
            &ro(
                n(1),
                Region {
                    x: n(2),
                    y: n(3),
                    w: n(4),
                    h: n(5),
                },
                Position::default(),
            ),
        )
        .unwrap();
    let rgb: Vec<u8> = r
        .data
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    std::fs::write(f[6], rgb).unwrap();
}
