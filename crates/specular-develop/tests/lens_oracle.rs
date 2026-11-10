//! Lens-model oracle on real raws: the cameras' own JPEGs were corrected in the camera, so
//! where scene features sit in the JPEG (against the raw) shows the correction the camera
//! applied. This test writes, per file, a binned linear raw, the largest embedded JPEG
//! and the model's back-map factors; `tools/quality/lens_oracle.py` matches features and
//! compares.
//!
//!     tools/corpus/fetch-lens.sh                      # Sony and Fujifilm raws (raw.pixls.us)
//!     SPECULAR_LENS_CORPUS=tools/corpus/lens SPECULAR_LENS_DUMP=target/lens-dump \
//!         cargo test -p specular-develop --test lens_oracle -- --ignored
//!     python3 -I tools/quality/lens_oracle.py target/lens-dump
//!
//! Without the corpus the tests skip.

use std::path::{Path, PathBuf};

use specular_develop::model::{Edit, Registry};
use specular_develop::ops::geometry::{Context, Plan};
use specular_develop::preview::Orient;
use specular_raw::sensor::LensMaker;

fn corpus() -> Option<Vec<PathBuf>> {
    let dir = PathBuf::from(std::env::var_os("SPECULAR_LENS_CORPUS")?);
    let mut v = Vec::new();
    walk(&dir, &mut v);
    v.sort();
    Some(v)
}

fn walk(p: &Path, v: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(p) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, v);
        } else if p
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "arw" | "raf"))
        {
            v.push(p);
        }
    }
}

/// Every Sony and Fujifilm file of the corpus has tables, and they parse to the shapes the
/// models expect.
#[test]
fn corpus_files_have_lens_tables() {
    let Some(files) = corpus() else {
        eprintln!("SPECULAR_LENS_CORPUS not set; skipping");
        return;
    };
    for f in files {
        let d = std::fs::read(&f).expect("read");
        let raw = specular_raw::parse(&d).expect("parse");
        let s = raw
            .decode_sensor(&d, &specular_raw::DecodeOptions::default())
            .expect("sensor");
        let l = s.meta.lens.as_ref().expect("lens tables");
        assert!(l.knots.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(*l.knots.last().expect("knots"), 1.0);
        if l.maker == LensMaker::Fujifilm {
            // The camera's half diagonal is the crop's.
            let c = s.crop;
            let hd = f64::from(c.w).hypot(f64::from(c.h)) / 2.0;
            let k = l.half_diagonal_px.expect("scale");
            assert!((k / hd - 1.0).abs() < 0.01, "{}: {k} vs {hd}", f.display());
        }
        eprintln!("{}: {:?}, {} knots", f.display(), l.maker, l.knots.len());
    }
}

/// Writes the oracle's inputs (see the module documentation).
#[test]
#[ignore = "oracle: needs SPECULAR_LENS_CORPUS and SPECULAR_LENS_DUMP, then lens_oracle.py"]
fn dump_for_the_jpeg_oracle() {
    let (Some(files), Some(out)) = (corpus(), std::env::var_os("SPECULAR_LENS_DUMP")) else {
        eprintln!("SPECULAR_LENS_CORPUS or SPECULAR_LENS_DUMP not set; skipping");
        return;
    };
    let out = PathBuf::from(out);
    std::fs::create_dir_all(&out).expect("dump folder");
    for f in files {
        let d = std::fs::read(&f).expect("read");
        let raw = specular_raw::parse(&d).expect("parse");
        let s = raw
            .decode_sensor(&d, &specular_raw::DecodeOptions::default())
            .expect("sensor");
        let name = f.file_name().expect("name").to_string_lossy().to_string();
        // Binned linear camera RGB, white balanced (6×6 keeps whole X-Trans periods).
        let n = s.normalised();
        let (w, h) = (s.crop.w as usize, s.crop.h as usize);
        let b = 6;
        let (bw, bh) = (w / b, h / b);
        let mut v = vec![0f32; bw * bh * 3];
        let mut cnt = vec![0f32; bw * bh * 3];
        for y in 0..bh * b {
            for x in 0..bw * b {
                let c = usize::from(
                    s.meta
                        .layout
                        .color(x as u32 + s.crop.x, y as u32 + s.crop.y),
                );
                let i = ((y / b) * bw + x / b) * 3 + c;
                v[i] += n[y * w + x] * s.meta.wb[c];
                cnt[i] += 1.0;
            }
        }
        for (a, c) in v.iter_mut().zip(&cnt) {
            *a /= c.max(1.0);
        }
        let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
        std::fs::write(out.join(format!("{name}.bin{bw}x{bh}")), bytes).expect("write");
        let p = raw.info().largest_preview().expect("a preview").clone();
        let pi = specular_raw::decode_preview(&d, &p, 0).expect("preview");
        std::fs::write(
            out.join(format!("{name}.prev{}x{}", pi.width, pi.height)),
            &pi.rgba,
        )
        .expect("write");
        // The model: back-map factors of red, green, blue at 201 radii 0..1, with the fill
        // zoom, and the uncorrected map for comparison.
        let edit = Edit::default_for(Registry::builtin(), Default::default(), true);
        let plan = Plan::new(&edit, &Context::raw(&s, Orient::default()), true);
        let mut txt = String::new();
        for i in 0..=200 {
            let rho = f64::from(i) / 200.0;
            let f: Vec<String> = (0..3)
                .map(|p| format!("{:.9}", plan.radial.factor(p, rho)))
                .collect();
            txt.push_str(&format!("{rho:.4} {}\n", f.join(" ")));
        }
        std::fs::write(out.join(format!("{name}.model")), txt).expect("write");
        eprintln!("{name}: zoom {:.5}", plan.radial.zoom);
    }
}

/// Sony's vignetting table against the camera's JPEG: the camera's tone curve is
/// fitted on the centre (raw green → JPEG green, linearised), then in the outer rings the
/// JPEG says how bright the raw should be; the error is in EV, with the table off and on.
/// Prints per file.
#[test]
#[ignore = "oracle: needs SPECULAR_LENS_CORPUS (Sony ARWs with their JPEGs)"]
fn sony_vignetting_against_the_jpeg() {
    use specular_develop::ops::geometry::lens::MakerModel;
    let Some(files) = corpus() else {
        eprintln!("SPECULAR_LENS_CORPUS not set; skipping");
        return;
    };
    let srgb_dec = |v: f64| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    for f in files {
        let d = std::fs::read(&f).expect("read");
        let raw = specular_raw::parse(&d).expect("parse");
        let s = raw
            .decode_sensor(&d, &specular_raw::DecodeOptions::default())
            .expect("sensor");
        let Some(l) = s.meta.lens.as_ref().filter(|l| l.maker == LensMaker::Sony) else {
            continue;
        };
        let name = f.file_name().expect("name").to_string_lossy().to_string();
        let Some(table) = l.vignetting.as_ref() else {
            eprintln!("{name}: no vignetting table");
            continue;
        };
        let p = raw.info().largest_preview().expect("a preview").clone();
        let pi = specular_raw::decode_preview(&d, &p, 0).expect("preview");
        let (pw, ph) = (pi.width as usize, pi.height as usize);
        let (w, h) = (s.crop.w as usize, s.crop.h as usize);
        if (pw > ph) != (w > h) {
            eprintln!("{name}: preview turned; skipped");
            continue;
        }
        let m = MakerModel::new(l, w as f64, h as f64);
        let n = s.normalised();
        // Green per 16×16 block, its spread, and the JPEG's linearised green there.
        let b = 16;
        let (bw, bh) = (w / b, h / b);
        let mut blocks = Vec::new();
        for by in 0..bh {
            for bx in 0..bw {
                let (mut sum, mut sq, mut cnt, mut hot) = (0f64, 0f64, 0f64, false);
                for y in by * b..(by + 1) * b {
                    for x in bx * b..(bx + 1) * b {
                        let c = s
                            .meta
                            .layout
                            .color(x as u32 + s.crop.x, y as u32 + s.crop.y);
                        if c == 1 {
                            let v = f64::from(n[y * w + x]);
                            sum += v;
                            sq += v * v;
                            cnt += 1.0;
                            hot |= v > 0.9;
                        }
                    }
                }
                let mean = sum / cnt;
                let cv = (sq / cnt - mean * mean).max(0.0).sqrt() / mean.max(1e-9);
                let px0 = bx * pw / bw;
                let px1 = ((bx + 1) * pw / bw).max(px0 + 1).min(pw);
                let py0 = by * ph / bh;
                let py1 = ((by + 1) * ph / bh).max(py0 + 1).min(ph);
                let (mut js, mut jc, mut jhot) = (0f64, 0f64, false);
                for y in py0..py1 {
                    for x in px0..px1 {
                        let g = pi.rgba[(y * pw + x) * 4 + 1];
                        jhot |= !(12..=243).contains(&g);
                        js += srgb_dec(f64::from(g) / 255.0);
                        jc += 1.0;
                    }
                }
                let cx = (bx as f64 + 0.5) * b as f64 - w as f64 / 2.0;
                let cy = (by as f64 + 0.5) * b as f64 - h as f64 / 2.0;
                let rho = cx.hypot(cy) / (w as f64).hypot(h as f64) * 2.0;
                if !hot && !jhot && cv < 0.2 && mean > 0.002 && jc > 0.0 {
                    blocks.push((rho, mean, js / jc));
                }
            }
        }
        // The camera curve from the centre: medians of JPEG per quarter-EV of raw.
        let mut bins: Vec<Vec<f64>> = vec![Vec::new(); 64];
        let bin = |r: f64| ((r.log2() + 12.0) * 4.0).clamp(0.0, 63.0) as usize;
        for &(rho, r, j) in &blocks {
            if rho < 0.3 {
                bins[bin(r)].push(j);
            }
        }
        let mut curve: Vec<(f64, f64)> = Vec::new();
        for (i, v) in bins.iter_mut().enumerate() {
            if v.len() >= 3 {
                v.sort_by(f64::total_cmp);
                let r = ((i as f64 + 0.5) / 4.0 - 12.0).exp2();
                let j = v[v.len() / 2].max(curve.last().map_or(0.0, |c| c.1));
                curve.push((r, j));
            }
        }
        if curve.len() < 3 {
            eprintln!("{name}: too little smooth centre to fit the camera curve");
            continue;
        }
        // JPEG → raw-equivalent, inside the fitted range only.
        let inv = |j: f64| -> Option<f64> {
            let k = curve.windows(2).find(|c| c[0].1 <= j && j <= c[1].1)?;
            let t = if k[1].1 > k[0].1 {
                (j - k[0].1) / (k[1].1 - k[0].1)
            } else {
                0.5
            };
            Some((k[0].0.log2() * (1.0 - t) + k[1].0.log2() * t).exp2())
        };
        for (lo, hi) in [(0.3, 0.55), (0.55, 0.8), (0.8, 1.01)] {
            let (mut off, mut on) = (Vec::new(), Vec::new());
            for &(rho, r, j) in &blocks {
                if !(lo..hi).contains(&rho) {
                    continue;
                }
                let Some(want) = inv(j) else { continue };
                off.push((want / r).log2());
                on.push((want / (r * m.vignette_gain(rho))).log2());
            }
            if off.len() < 5 {
                eprintln!("{name}: ring {lo}–{hi}: {} usable blocks", off.len());
                continue;
            }
            off.sort_by(f64::total_cmp);
            on.sort_by(f64::total_cmp);
            eprintln!(
                "{name}: ring ρ {lo}–{hi} ({} blocks): JPEG vs raw {:+.3} EV table off, {:+.3} EV on (table {:+.3} EV at ρ {:.2})",
                off.len(),
                off[off.len() / 2],
                on[on.len() / 2],
                m.vignette_gain((lo + hi) / 2.0).log2(),
                (lo + hi) / 2.0
            );
        }
        eprintln!("{name}: table {table:?}");
    }
}
