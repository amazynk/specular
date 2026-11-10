//! Velox / FEI EMD images against RosettaSciIO (`gen/oracle_emd.py`, `oracle_emd.json` in
//! `$SPECULAR_MATSCI_CORPUS/rsciio/emd`, fetched by `tools/corpus/fetch-matsci.sh`; skipped
//! when absent): which images are found, their titles, sizes and frame counts, pixel size and
//! origin, and the sums and a few pixels of the first, middle and last frames. Complex images
//! (compound datasets) are counted but not compared: they are not read yet.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::Value;
use specular_hdf5::{Dtype, File, PlaneSpec, emd};

fn corpus() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("SPECULAR_MATSCI_CORPUS")?).join("rsciio/emd");
    dir.join("oracle_emd.json").is_file().then_some(dir)
}

fn close(a: f64, b: f64) -> bool {
    (a.is_nan() && b.is_nan()) || (a - b).abs() <= 1e-6 * a.abs().max(b.abs()) + 1e-12
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or(f64::NAN)
}

/// Metres (or 1/m) per oracle unit.
fn per_unit(units: &str) -> Option<f64> {
    Some(match units.replace(' ', "").as_str() {
        "m" | "1/m" => 1.0,
        "mm" => 1e-3,
        "μm" | "µm" | "um" => 1e-6,
        "nm" => 1e-9,
        "pm" => 1e-12,
        "Å" => 1e-10,
        "1/mm" => 1e3,
        "1/μm" | "1/µm" | "1/um" => 1e6,
        "1/nm" => 1e9,
        "1/pm" => 1e12,
        _ => return None,
    })
}

fn check(im: &emd::Image, file: &File, data: &[u8], sig: &Value, errs: &mut Vec<String>) {
    let ds = file.objects[im.index].dataset().unwrap();
    let shape = ds.shape().to_vec();
    let frame_dims: Vec<usize> = (0..shape.len())
        .filter(|&d| d != im.x_dim && d != im.y_dim)
        .collect();
    let frames: u64 = frame_dims.iter().map(|&d| shape[d]).product();
    let (w, h) = (shape[im.x_dim] as usize, shape[im.y_dim] as usize);
    let want: Vec<u64> = sig["shape"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    if [frames, h as u64, w as u64] != want[..] {
        errs.push(format!("shape {frames}x{h}x{w} vs {want:?}"));
        return;
    }
    let axes = sig["axes"].as_array().unwrap();
    let (ay, ax) = (&axes[axes.len() - 2], &axes[axes.len() - 1]);
    match (&im.pixel_size, per_unit(ax["units"].as_str().unwrap_or(""))) {
        (Some((sx, sy, _)), Some(k)) => {
            let (ox, oy) = im.pixel_offset.unwrap_or((0.0, 0.0));
            if !close(*sx, f(&ax["scale"]) * k) || !close(*sy, f(&ay["scale"]) * k) {
                errs.push(format!(
                    "pixel {sx} x {sy} vs {} {}",
                    ax["scale"], ax["units"]
                ));
            }
            if !close(ox, f(&ax["offset"]) * k) || !close(oy, f(&ay["offset"]) * k) {
                errs.push(format!(
                    "origin {ox}, {oy} vs {}, {} {}",
                    ax["offset"], ay["offset"], ax["units"]
                ));
            }
        }
        (None, None) => {}
        (ours, _) => errs.push(format!("pixel {ours:?} vs {} {}", ax["scale"], ax["units"])),
    }
    for p in sig["planes"].as_array().unwrap() {
        let mut k = p["index"].as_u64().unwrap();
        let mut index = vec![0u64; shape.len()];
        for &d in frame_dims.iter().rev() {
            index[d] = k % shape[d];
            k /= shape[d];
        }
        let spec = PlaneSpec {
            x_dim: im.x_dim,
            y_dim: im.y_dim,
            index,
        };
        let plane = match file.read_plane::<f64>(data, im.index, &spec, &|| false) {
            Ok(p) => p.values,
            Err(e) => {
                errs.push(format!("frame {}: {e}", p["index"]));
                continue;
            }
        };
        let sum: f64 = plane.iter().sum();
        if !close(sum, f(&p["sum"])) {
            errs.push(format!("frame {} sum {sum} vs {}", p["index"], p["sum"]));
        }
        for px in p["pixels"].as_array().unwrap() {
            let (x, y) = (
                px[0].as_u64().unwrap() as usize,
                px[1].as_u64().unwrap() as usize,
            );
            if !close(plane[y * w + x], f(&px[2])) {
                errs.push(format!(
                    "frame {} ({x}, {y}) {} vs {}",
                    p["index"],
                    plane[y * w + x],
                    px[2]
                ));
            }
        }
    }
}

#[test]
fn corpus_matches_oracle() {
    let Some(dir) = corpus() else {
        eprintln!("SPECULAR_MATSCI_CORPUS not set or not fetched; skipped");
        return;
    };
    let text = std::fs::read_to_string(dir.join("oracle_emd.json")).unwrap();
    let oracle: BTreeMap<String, Value> = serde_json::from_str(&text).unwrap();
    let (mut files, mut bad_files, mut images, mut bad_images, mut complex) = (0, 0, 0, 0, 0);
    let mut failures = Vec::new();
    for (rel, sigs) in &oracle {
        let Some(sigs) = sigs.as_array() else {
            continue;
        };
        files += 1;
        let data = std::fs::read(dir.join(rel)).unwrap();
        let file = match File::open(&data) {
            Ok(f) => f,
            Err(e) => {
                bad_files += 1;
                failures.push(format!("{rel}: {e}"));
                continue;
            }
        };
        let ours = emd::images(&file, &data);
        let mut used = vec![false; ours.len()];
        let mut file_bad = false;
        for sig in sigs {
            images += 1;
            let title = sig["title"].as_str().unwrap_or("");
            let mut errs = Vec::new();
            match (0..ours.len()).find(|&k| !used[k] && ours[k].title == title) {
                // Complex images (Velox DPC and FFT, compound `realFloat`/`imagFloat`) are not
                // read yet.
                Some(k)
                    if matches!(
                        file.objects[ours[k].index].dataset().map(|d| &d.dtype),
                        Some(Dtype::Other { class: 6, .. })
                    ) =>
                {
                    used[k] = true;
                    complex += 1;
                    continue;
                }
                Some(k) => {
                    used[k] = true;
                    check(&ours[k], &file, &data, sig, &mut errs);
                }
                None => errs.push(format!(
                    "not found; ours: {:?}",
                    ours.iter().map(|i| &i.title).collect::<Vec<_>>()
                )),
            }
            if !errs.is_empty() {
                bad_images += 1;
                file_bad = true;
                for e in errs {
                    failures.push(format!("{rel} [{title}]: {e}"));
                }
            }
        }
        for (k, im) in ours.iter().enumerate().filter(|(k, _)| !used[*k]) {
            eprintln!("{rel}: extra image {k} {:?} ({})", im.title, im.path);
        }
        bad_files += usize::from(file_bad);
    }
    eprintln!(
        "EMD: {files} files, {bad_files} with mismatches; {images} images, {bad_images} with \
         mismatches, {complex} complex not compared"
    );
    for f in &failures {
        eprintln!("  {f}");
    }
    assert!(failures.is_empty());
}
