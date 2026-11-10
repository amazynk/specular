//! The array session against the oracles' values: every committed NumPy, MATLAB (levels 4
//! and 5 and v7.3) and Zarr fixture of specular-arrays is opened through the registry and
//! every plane of every page compared with what numpy, scipy.io, zarr-python and h5py read
//! (`specular-arrays/gen`); TDMS and VTK fixtures open and show.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::*;
use crate::Registry;

fn formats() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../formats")
}

#[derive(Default, Debug)]
struct Tally {
    files: usize,
    pages: usize,
    values: u64,
    mismatches: u64,
    failures: Vec<String>,
}

fn load_f32(p: &Path) -> Vec<f32> {
    std::fs::read(p)
        .map(|b| {
            b.chunks_exact(4)
                .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
                .collect()
        })
        .unwrap_or_default()
}

/// The page labelled `name` of the file's session.
fn page_named(s: &Arc<dyn Session>, name: &str) -> Option<Arc<dyn Session>> {
    for i in 0..s.page_count() {
        let p = if i == s.page_index() {
            s.clone()
        } else {
            s.open_page(i).ok()?
        };
        let label = p.science()?.dataset.clone().unwrap_or_default();
        if label == name {
            return Some(p);
        }
    }
    None
}

/// Compares every plane of `page` with `want` (C order of the MATLAB / NumPy shape), laid
/// out by the documented convention.
#[allow(clippy::too_many_arguments)]
fn compare(
    t: &mut Tally,
    what: &str,
    page: &Arc<dyn Session>,
    shape: &[u64],
    matlab: bool,
    want: &[f32],
    want_re: Option<&[f32]>,
) {
    let s = page.science().unwrap();
    let ns: Vec<usize> = (0..shape.len()).filter(|&d| shape[d] > 1).collect();
    let cancel = CancelToken::new();
    let strides = specular_arrays::strides(shape, false);
    let flat = |pos: &[u64]| -> usize {
        pos.iter().zip(&strides).map(|(a, b)| a * b).sum::<u64>() as usize
    };
    let part_axis = s.axes.iter().position(|a| a.kind == AxisKind::Part);
    if ns.len() <= 1 {
        let x = ns.first().copied().unwrap_or(0);
        let n = shape[x] as usize;
        // The plot is the whole array.
        let sp = s.spectra.as_ref().expect("1-D page plot");
        let mut idx = vec![0u32; s.axes.len()];
        for (part, w) in [(0u32, Some(want)), (1, want_re)] {
            let Some(w) = w else { continue };
            if let Some(k) = part_axis {
                idx[k] = part;
            }
            let got = sp.at(0, 0, &idx).expect("plot").y;
            assert_eq!(got.len(), n.min(1 << 16), "{what}");
            if n > 1 << 16 {
                continue;
            }
            let bad = got
                .iter()
                .zip(w)
                .filter(|(g, e)| {
                    g.to_bits() != e.to_bits()
                        && !(g.is_nan() && e.is_nan())
                        && ((**g - **e).abs() > 1e-6 * e.abs() || g.is_nan() != e.is_nan())
                })
                .count();
            t.values += got.len() as u64;
            t.mismatches += bad as u64;
            if bad > 0 {
                t.failures.push(format!("{what}: {bad} plot values differ"));
            }
        }
        t.pages += 1;
        return;
    }
    let colour = s.channel_axis.is_some();
    let (x, y, c, sliders): (usize, usize, Option<usize>, Vec<usize>) = if matlab {
        if colour {
            (ns[1], ns[0], Some(ns[2]), ns[3..].to_vec())
        } else {
            (ns[1], ns[0], None, ns[2..].to_vec())
        }
    } else {
        let k = ns.len();
        if colour {
            (ns[k - 2], ns[k - 3], Some(ns[k - 1]), ns[..k - 3].to_vec())
        } else {
            (ns[k - 1], ns[k - 2], None, ns[..k - 2].to_vec())
        }
    };
    let mut dims_of_axes: Vec<usize> = sliders.clone();
    if let Some(c) = c {
        dims_of_axes.push(c);
    }
    assert_eq!(
        s.axes.len(),
        dims_of_axes.len() + usize::from(part_axis.is_some()),
        "{what}: axes"
    );
    assert_eq!(
        (s.width as u64, s.height as u64),
        (shape[x], shape[y]),
        "{what}"
    );
    let planes: u64 = dims_of_axes.iter().map(|&d| shape[d]).product();
    let mut bad = 0u64;
    for pn in 0..planes {
        let mut pos = vec![0u64; shape.len()];
        let mut r = pn;
        let mut idx = vec![0u32; s.axes.len()];
        for (k, &d) in dims_of_axes.iter().enumerate().rev() {
            pos[d] = r % shape[d];
            idx[k] = pos[d] as u32;
            r /= shape[d];
        }
        for (part, w) in [(0u32, Some(want)), (1, want_re)] {
            let Some(w) = w else { continue };
            if let Some(k) = part_axis {
                idx[k] = part;
            }
            let plane = page.nd_plane(&idx, &cancel).unwrap().unwrap();
            for yy in 0..shape[y] {
                for xx in 0..shape[x] {
                    pos[x] = xx;
                    pos[y] = yy;
                    let e = w[flat(&pos)];
                    let g = plane.values[(yy * shape[x] + xx) as usize];
                    let same = g.to_bits() == e.to_bits()
                        || (g.is_nan() && e.is_nan())
                        || (part_axis.is_some() && (g - e).abs() <= 1e-6 * e.abs());
                    if !same {
                        if bad < 3 {
                            t.failures
                                .push(format!("{what}: {pos:?}: got {g}, want {e}"));
                        }
                        bad += 1;
                    }
                    t.values += 1;
                }
            }
        }
    }
    t.mismatches += bad;
    t.pages += 1;
}

fn check_dir(dir: &Path, t: &mut Tally) {
    let reg = Registry::new();
    let mut names: Vec<PathBuf> = std::fs::read_dir(dir.join("expected"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    names.sort();
    for m in names {
        let man: Value = serde_json::from_slice(&std::fs::read(&m).unwrap()).unwrap();
        let name = m.file_stem().unwrap().to_str().unwrap().to_owned();
        let path = dir.join(&name);
        if man.get("broken").is_some() {
            // The oracle could not read it either: opening must not panic, and whatever
            // opens must decode.
            if let Ok(s) = reg.open_path(&path) {
                let _ = s.decode(&DecodeRequest::full(1), &CancelToken::new());
            }
            continue;
        }
        let arrays = man["arrays"].as_array().unwrap();
        let kind = man["kind"].as_str().unwrap();
        let showable = arrays.iter().any(|a| {
            a["shape"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap())
                .product::<u64>()
                >= 2
        });
        let s = match reg.open_path(&path) {
            Ok(s) => s,
            Err(e) => {
                // Refused files, scalars and empty arrays must say why, plainly.
                if showable {
                    t.failures.push(format!("{name}: {e}"));
                } else {
                    assert!(matches!(e, DecodeError::Unsupported(_)), "{name}: {e:?}");
                }
                continue;
            }
        };
        t.files += 1;
        let format = s.info().format;
        let want_format = match kind {
            "npy" => Format::Npy,
            "npz" => Format::Npz,
            "mat" | "mat73" => Format::Matlab,
            _ => Format::Zarr,
        };
        assert_eq!(format, want_format, "{name}");
        let values = load_f32(&dir.join("expected").join(format!("{name}.f32")));
        for a in arrays {
            let an = a["name"].as_str().unwrap();
            let shape: Vec<u64> = a["shape"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap())
                .collect();
            let n = shape.iter().product::<u64>() as usize;
            if n < 2 {
                continue;
            }
            let off = a["offset"].as_u64().unwrap() as usize;
            let re = a
                .get("re_offset")
                .and_then(Value::as_u64)
                .map(|o| &values[o as usize..o as usize + n]);
            let Some(page) = page_named(&s, an) else {
                t.failures.push(format!("{name}: no page {an}"));
                continue;
            };
            let matlab = matches!(kind, "mat" | "mat73");
            compare(
                t,
                &format!("{name}:{an}"),
                &page,
                &shape,
                matlab,
                &values[off..off + n],
                re,
            );
        }
        // Char arrays are header keywords.
        if let Some(texts) = man["texts"].as_object() {
            let sci = s.science().unwrap();
            for (p, text) in texts {
                let want = text.as_str().unwrap().replace('\n', " / ");
                if !sci.keywords.iter().any(|(k, v, _)| k == p && *v == want) {
                    t.failures
                        .push(format!("{name}: text {p} not in the keywords"));
                }
            }
        }
    }
}

#[test]
fn arrays_match_the_oracles_through_the_registry() {
    let mut t = Tally::default();
    let base = formats().join("specular-arrays/tests/data");
    check_dir(&base, &mut t);
    check_dir(&base.join("v73"), &mut t);
    eprintln!("{t:?}");
    assert!(t.files > 40 && t.pages > 150, "{t:?}");
    assert!(
        t.failures.is_empty() && t.mismatches == 0,
        "{:#?}",
        t.failures
    );
}

#[test]
fn larger_corpus_through_the_registry() {
    let Some(dirs) = std::env::var_os("SPECULAR_ARRAYS_DECODE_CORPUS") else {
        return;
    };
    let mut t = Tally::default();
    let t0 = std::time::Instant::now();
    for d in std::env::split_paths(&dirs) {
        check_dir(&d, &mut t);
    }
    eprintln!(
        "{} files, {} pages, {} values, {} mismatches in {:.1} s",
        t.files,
        t.pages,
        t.values,
        t.mismatches,
        t0.elapsed().as_secs_f64()
    );
    assert!(
        t.failures.is_empty() && t.mismatches == 0,
        "{:#?}",
        t.failures
    );
}

/// A `.npy` file of `descr` and `shape` with `body`.
fn npy_file(descr: &str, shape: &str, fortran: bool, body: &[u8]) -> Vec<u8> {
    let mut h = format!(
        "{{'descr': '{descr}', 'fortran_order': {}, 'shape': {shape}, }}",
        if fortran { "True" } else { "False" }
    );
    while !(10 + h.len() + 1).is_multiple_of(64) {
        h.push(' ');
    }
    h.push('\n');
    let mut out = b"\x93NUMPY\x01\x00".to_vec();
    out.extend_from_slice(&(h.len() as u16).to_le_bytes());
    out.extend_from_slice(h.as_bytes());
    out.extend_from_slice(body);
    out
}

fn open_bytes(name: &str, bytes: Vec<u8>) -> Arc<dyn Session> {
    Registry::new()
        .open(Arc::new(Source::from_vec(PathBuf::from(name), bytes)))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn colour_complex_and_one_d_layouts() {
    let cancel = CancelToken::new();
    // 4 × 5 × 3 uint8: a colour image.
    let s = open_bytes("rgb.npy", npy_file("|u1", "(4, 5, 3)", false, &[7; 60]));
    let sci = s.science().unwrap();
    assert_eq!((sci.width, sci.height), (5, 4));
    assert_eq!(sci.channel_axis, Some(0));
    assert_eq!(s.colour_model(), Some("RGB"));
    s.decode(&DecodeRequest::full(1), &cancel).unwrap();
    // The same shape in float: a stack of three 4 × 5 planes? No: the last dimension is x.
    let s = open_bytes("f.npy", npy_file("<f4", "(4, 5, 3)", false, &[0; 240]));
    let sci = s.science().unwrap();
    assert_eq!((sci.width, sci.height), (3, 5));
    assert_eq!(sci.axes.len(), 1);
    // Complex: a parts slider.
    let body: Vec<u8> = [3f32, 4.0, 0.0, 1.0]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let s = open_bytes("z.npy", npy_file("<c8", "(1, 2)", false, &body));
    let sci = s.science().unwrap();
    assert_eq!(sci.axes.len(), 1);
    assert_eq!(sci.axes[0].kind, AxisKind::Part);
    assert_eq!(sci.axes[0].len, 4);
    // (1, 2) is 1-D: a strip whose plot is the array.
    let sp = sci.spectra.as_ref().unwrap();
    assert_eq!(sp.at(0, 0, &[0]).unwrap().y, [5.0, 1.0]);
    assert_eq!(
        sp.at(0, 0, &[3]).unwrap().y,
        [4f32.atan2(3.0), std::f32::consts::FRAC_PI_2]
    );
    // A long 1-D array is binned in the strip but plotted in full.
    let n = 10_000usize;
    let body: Vec<u8> = (0..n).flat_map(|i| (i as f64).to_le_bytes()).collect();
    let s = open_bytes("long.npy", npy_file("<f8", "(10000,)", false, &body));
    let sci = s.science().unwrap();
    assert_eq!(sci.width, 3334);
    let pl = s.nd_plane(&[], &cancel).unwrap().unwrap();
    assert_eq!(pl.values[0], 1.0);
    assert_eq!(
        sci.spectra.as_ref().unwrap().at(5, 0, &[]).unwrap().y.len(),
        n
    );
    // A scalar is a keyword, not a page: refused with a reason.
    match Registry::new().open(Arc::new(Source::from_vec(
        PathBuf::from("one.npy"),
        npy_file("<f8", "()", false, &2.5f64.to_le_bytes()),
    ))) {
        Err(DecodeError::Unsupported(m)) => assert!(m.contains("fewer than two")),
        r => panic!("{:?}", r.map(|_| ())),
    }
}

#[test]
fn matlab_pages_and_keywords() {
    let p = formats().join("specular-arrays/tests/data/m00_z.mat");
    let s = Registry::new().open_path(&p).unwrap();
    assert_eq!(s.info().format, Format::Matlab);
    let mut labels = Vec::new();
    for i in 0..s.page_count() {
        let pg = s.open_page(i).unwrap();
        labels.push(pg.science().unwrap().dataset.clone().unwrap());
        match pg.page_label() {
            Some(PageLabel::Object { class, .. }) => assert!(!class.is_empty()),
            l => panic!("{l:?}"),
        }
    }
    for want in [
        "img",
        "s.a",
        "s.inner.b",
        "c{1}",
        "c{3}",
        "sa(1).p",
        "sa(2).q",
        "sp",
        "z",
    ] {
        assert!(labels.iter().any(|l| l == want), "{want}: {labels:?}");
    }
    // The uint8 16 × 20 × 3 image is in colour; the sparse matrix dense.
    let img = page_named(&s, "img").unwrap();
    assert_eq!(img.science().unwrap().channel_axis, Some(0));
    let sp = page_named(&s, "sp").unwrap();
    assert!(sp.science().unwrap().sample.contains("sparse"));
    let kw = &s.science().unwrap().keywords;
    assert!(kw.iter().any(|(k, v, _)| k == "name" && v == "hello"));
    assert!(kw.iter().any(|(k, _, _)| k == "MAT-file"));
}

#[test]
fn zarr_folder_zip_and_metadata_file_open() {
    let base = formats().join("specular-arrays/tests/data");
    let reg = Registry::new();
    let folder = base.join("consolidated_v3.zarr");
    assert!(specular_core::is_image_folder(&folder));
    let s = reg.open_path(&folder).unwrap();
    assert_eq!(s.info().format, Format::Zarr);
    assert_eq!(s.page_count(), 2);
    // A metadata document opened on its own opens its folder.
    let meta = base.join("root_v2.zarr/.zarray");
    assert!(reg.supports_file(&meta));
    let s = reg.open_path(&meta).unwrap();
    assert_eq!(s.info().format, Format::Zarr);
    assert_eq!((s.info().width, s.info().height), (9, 7));
    let s = reg.open_path(&base.join("root_v3.zarr/zarr.json")).unwrap();
    assert_eq!(s.info().format, Format::Zarr);
    // A zipped store through the OME-Zarr decoder's extension.
    let s = reg.open_path(&base.join("v3_codecs.zarr.zip")).unwrap();
    assert_eq!(s.info().format, Format::Zarr);
    assert!(s.page_count() > 10);
}

#[test]
fn tdms_fixtures_open() {
    let dir = formats().join("specular-tdms/tests/data");
    let reg = Registry::new();
    let cancel = CancelToken::new();
    let mut n = 0;
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_none_or(|x| x != "tdms") {
            continue;
        }
        let s = reg
            .open_path(&p)
            .unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        assert_eq!(s.info().format, Format::Tdms);
        for i in 0..s.page_count() {
            let pg = s.open_page(i).unwrap();
            let sci = pg.science().unwrap();
            let idx = vec![0; sci.axes.len()];
            match pg.nd_plane(&idx, &cancel).unwrap() {
                Ok(pl) => assert_eq!(pl.values.len(), (pl.width * pl.height) as usize),
                // Channels whose scaling we refuse say so.
                Err(DecodeError::Unsupported(m)) => assert!(m.contains("TDMS"), "{m}"),
                Err(e) => panic!("{}: page {i}: {e}", p.display()),
            }
            assert!(sci.spectra.is_some(), "{}: page {i}", p.display());
        }
        n += 1;
    }
    assert!(n >= 10);
    // The writer fixture: a group image of its channels and a page per channel.
    let s = reg.open_path(&dir.join("writer.tdms")).unwrap();
    let labels: Vec<String> = (0..s.page_count())
        .map(|i| {
            s.open_page(i)
                .unwrap()
                .science()
                .unwrap()
                .dataset
                .clone()
                .unwrap()
        })
        .collect();
    assert!(labels.iter().any(|l| l == "Measured"), "{labels:?}");
    assert!(labels.iter().any(|l| l == "Measured/i8"), "{labels:?}");
    let ch = page_named(&s, "Measured/i8").unwrap();
    let sci = ch.science().unwrap();
    assert_eq!(sci.unit.as_deref(), Some("V"));
    // The plot runs against time from wf_start_offset in steps of wf_increment.
    let sp = sci.spectra.as_ref().unwrap().at(0, 0, &[]).unwrap();
    assert_eq!(sp.x_at(0), 0.5);
    assert!((sp.x_at(1) - 0.501).abs() < 1e-12);
    let direct = specular_tdms::parse(&std::fs::read(dir.join("writer.tdms")).unwrap()).unwrap();
    let c = &direct.groups[0].channels[0];
    let mut want = vec![0f64; c.len as usize];
    c.read(
        &std::fs::read(dir.join("writer.tdms")).unwrap(),
        0,
        &mut want,
    )
    .unwrap();
    assert_eq!(sp.y, want.iter().map(|&v| v as f32).collect::<Vec<_>>());
}

#[test]
fn vtk_fixtures_open_as_grids_or_meshes() {
    let dir = formats().join("specular-vtk/tests/data");
    let reg = Registry::new();
    let cancel = CancelToken::new();
    let (mut grids, mut meshes, mut refused) = (0, 0, 0);
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("");
        if !["vtk", "vti", "vtr", "vts", "vtu", "vtp"].contains(&ext) {
            continue;
        }
        match reg.open_path(&p) {
            Ok(s) => {
                assert_eq!(s.info().format, Format::Vtk, "{}", p.display());
                if s.mesh().is_some() {
                    meshes += 1;
                    s.decode(&DecodeRequest::full(4), &cancel).unwrap();
                } else {
                    let sci = s.science().unwrap();
                    let pl = s
                        .nd_plane(&vec![0; sci.axes.len()], &cancel)
                        .unwrap()
                        .unwrap();
                    assert_eq!(pl.values.len(), (pl.width * pl.height) as usize);
                    grids += 1;
                }
            }
            Err(DecodeError::Unsupported(m)) => {
                assert!(!m.is_empty());
                refused += 1;
            }
            Err(e) => panic!("{}: {e}", p.display()),
        }
    }
    assert!(
        grids >= 20 && meshes >= 20,
        "{grids} grids, {meshes} meshes"
    );
    assert!(refused <= 12, "{refused}");
}

/// Timings on large files (`specular-arrays/gen/make_bench.py`): open, first plane,
/// thumbnail.
#[test]
fn bench_large_arrays() {
    let Some(dir) = std::env::var_os("SPECULAR_ARRAYS_BENCH") else {
        return;
    };
    let reg = Registry::new();
    let mut names: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    names.sort();
    for p in names {
        let cancel = CancelToken::new();
        let t0 = std::time::Instant::now();
        let s = match reg.open_path(&p) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{}: {e}", p.display());
                continue;
            }
        };
        let t_open = t0.elapsed();
        for i in 0..s.page_count().min(3) {
            let pg = s.open_page(i).unwrap();
            let sci = pg.science().unwrap();
            let t1 = std::time::Instant::now();
            let pl = pg
                .nd_plane(&vec![0; sci.axes.len()], &cancel)
                .unwrap()
                .unwrap();
            let t_plane = t1.elapsed();
            let fresh = s.open_page(i).unwrap();
            let t2 = std::time::Instant::now();
            let thumb = fresh
                .decode_sampled(256, &cancel)
                .unwrap_or_else(|| fresh.decode(&DecodeRequest::full(8), &cancel))
                .unwrap();
            let t_thumb = t2.elapsed();
            eprintln!(
                "{} [{}] {}×{}: open {:.1} ms, plane {:.1} ms, thumbnail {}×{} {:.1} ms",
                p.file_name().unwrap().to_string_lossy(),
                sci.dataset.clone().unwrap_or_default(),
                pl.width,
                pl.height,
                t_open.as_secs_f64() * 1e3,
                t_plane.as_secs_f64() * 1e3,
                thumb.width,
                thumb.height,
                t_thumb.as_secs_f64() * 1e3
            );
        }
    }
}

#[test]
fn vtk_image_data_is_y_up_with_spacing() {
    // A 3 × 2 × 1 structured points file: values 0..6, x fastest, spacing 0.5.
    let text = "# vtk DataFile Version 3.0\ntest\nASCII\nDATASET STRUCTURED_POINTS\n\
                DIMENSIONS 3 2 1\nORIGIN 1 2 0\nSPACING 0.5 0.25 1\nPOINT_DATA 6\n\
                SCALARS v float 1\nLOOKUP_TABLE default\n0 1 2 3 4 5\n";
    let s = open_bytes("t.vtk", text.as_bytes().to_vec());
    let sci = s.science().unwrap();
    assert_eq!((sci.width, sci.height), (3, 2));
    let pl = s.nd_plane(&[], &CancelToken::new()).unwrap().unwrap();
    // The top row is the file's last (y up).
    assert_eq!(pl.values, [3.0, 4.0, 5.0, 0.0, 1.0, 2.0]);
    let px = sci.pixel.as_ref().unwrap();
    assert_eq!(px.x.at(0.0), 1.0);
    assert_eq!(px.x.step, 0.5);
    assert_eq!(px.y.at(0.0), 2.25);
    assert_eq!(px.y.at(1.0), 2.0);
}
