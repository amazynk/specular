use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::*;

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../formats/specular-imzml/tests/data")
}

fn open(path: &Path) -> Arc<dyn Session> {
    crate::Registry::new()
        .open(Arc::new(Source::open(path).unwrap()))
        .unwrap()
}

/// pyimzML's reading of the fixtures (gen/oracle.py): per file, the coordinates, TICs, the
/// ion windows and the window sums.
#[derive(Default)]
struct Expected {
    coords: Vec<(u32, u32, u32)>,
    tic: HashMap<usize, f64>,
    windows: Vec<(f64, f64)>,
    ion: HashMap<(usize, usize), f64>,
}

fn expected() -> HashMap<String, Expected> {
    let text = std::fs::read_to_string(data_dir().join("expected.tsv")).unwrap();
    let mut out: HashMap<String, Expected> = HashMap::new();
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let f: Vec<&str> = line.split('\t').collect();
        let e = out.entry(f[1].to_owned()).or_default();
        let n = |k: usize| f[k].parse::<f64>().unwrap();
        match f[0] {
            "coord" => e.coords.push((n(3) as u32, n(4) as u32, n(5) as u32)),
            "tic" => {
                e.tic.insert(n(2) as usize, n(3));
            }
            "win" => e.windows.push((n(3), n(4))),
            "ion" => {
                e.ion.insert((n(2) as usize, n(3) as usize), n(4));
            }
            _ => {}
        }
    }
    out
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-4 * b.abs().max(1.0)
}

/// The TIC image and the ion images of every fixture agree with pyimzML, pixel by pixel.
#[test]
fn images_match_pyimzml() {
    let cancel = CancelToken::new();
    let mut checked = 0;
    for (name, e) in expected() {
        let s = open(&data_dir().join(&name));
        assert_eq!(s.info().format, Format::Imzml);
        let sc = s.science().unwrap();
        assert_eq!(sc.kind, ScienceKind::MassSpec);
        let ms = sc.mass_spec.as_ref().unwrap();
        let ox = e.coords.iter().map(|c| c.0).min().unwrap();
        let oy = e.coords.iter().map(|c| c.1).min().unwrap();
        let mut zs: Vec<u32> = e.coords.iter().map(|c| c.2).collect();
        zs.sort_unstable();
        zs.dedup();
        assert_eq!(sc.axes.len(), usize::from(zs.len() > 1), "{name}");
        let px = |i: usize| {
            let (x, y, z) = e.coords[i];
            let zi = zs.iter().position(|&v| v == z).unwrap() as u32;
            (x - ox, y - oy, zi)
        };
        for (&i, &want) in &e.tic {
            let (x, y, zi) = px(i);
            let index: Vec<u32> = if zs.len() > 1 { vec![zi] } else { vec![] };
            let p = s.nd_plane(&index, &cancel).unwrap().unwrap();
            let got = f64::from(p.value(x, y).unwrap());
            assert!(close(got, want), "{name} TIC {i}: {got} vs {want}");
            assert_eq!(ms.position(x, y, &index), e.coords[i], "{name} {i}");
            checked += 1;
        }
        for (w, &(mz, tol)) in e.windows.iter().enumerate() {
            let win = IonWindow {
                mz,
                tolerance: IonTolerance::Da(tol),
            };
            for (&(wi, i), &want) in &e.ion {
                if wi != w {
                    continue;
                }
                let (x, y, zi) = px(i);
                let index: Vec<u32> = if zs.len() > 1 { vec![zi] } else { vec![] };
                let p = ms.ion_plane(win, &index, &cancel).unwrap();
                let got = f64::from(p.value(x, y).unwrap());
                assert!(
                    close(got, want),
                    "{name} window {w} at {i}: {got} vs {want}"
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 2000, "{checked}");
}

#[test]
fn summary_and_spectra() {
    let s = open(&data_dir().join("processed_3d.imzML"));
    let sc = s.science().unwrap();
    assert_eq!(sc.axes.len(), 1);
    assert_eq!(sc.axes[0].name, "z");
    assert_eq!(sc.axes[0].len, 2);
    assert_eq!(sc.luts.len(), 1);
    let kw = |k: &str| {
        sc.keywords
            .iter()
            .find(|(n, _, _)| n == k)
            .map(|(_, v, _)| v.clone())
    };
    assert_eq!(kw("Mode").as_deref(), Some("processed"));
    assert_eq!(kw("Spectra").as_deref(), Some("96"));
    assert_eq!(kw("Polarity").as_deref(), Some("positive"));
    assert_eq!(kw("Spectrum type").as_deref(), Some("centroid"));
    assert!(kw("m/z range").is_some());
    let sp = sc.spectra.as_ref().unwrap();
    assert_eq!(sp.x_axis().0, "m/z");
    assert_eq!(sp.channels(), 0);
    let ms = sc.mass_spec.as_ref().unwrap();
    let (lo, hi) = ms.mz_range().unwrap();
    let mut found = 0;
    for z in 0..2 {
        for y in 0..sc.height {
            for x in 0..sc.width {
                let Some(spec) = sp.at(x, y, &[z]) else {
                    assert!(!ms.has_spectrum(x, y, &[z]));
                    continue;
                };
                found += 1;
                assert!(spec.sticks);
                let SpectrumX::Values(v) = &spec.x else {
                    panic!("m/z values")
                };
                assert_eq!(v.len(), spec.y.len());
                assert!(v.windows(2).all(|p| p[0] <= p[1]));
                if let Some((a, b)) = spec.x_range() {
                    assert!(a >= lo && b <= hi);
                    let peak = ms.base_peak(x, y, &[z]).unwrap();
                    assert!(peak >= a && peak <= b);
                }
            }
        }
    }
    assert_eq!(found, 96);

    let c = open(&data_dir().join("continuous_f32.imzML"));
    let sc = c.science().unwrap();
    let sp = sc.spectra.as_ref().unwrap();
    assert!(sp.channels() > 0);
    let spec = sp.at(0, 0, &[]).unwrap();
    assert!(!spec.sticks);
    assert_eq!(spec.y.len(), sp.channels());
    // A decode is the TIC through viridis.
    let r = c
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert_eq!((r.width, r.height), (sc.width, sc.height));
}

#[test]
fn ion_window_in_ppm() {
    let s = open(&data_dir().join("continuous_f32.imzML"));
    let ms = s.science().unwrap().mass_spec.clone().unwrap();
    let cancel = CancelToken::new();
    let ppm = IonWindow {
        mz: 369.3516,
        tolerance: IonTolerance::Ppm(10.0),
    };
    let da = IonWindow {
        mz: 369.3516,
        tolerance: IonTolerance::Da(369.3516 * 10e-6),
    };
    let a = ms.ion_plane(ppm, &[], &cancel).unwrap();
    let b = ms.ion_plane(da, &[], &cancel).unwrap();
    assert_eq!(a.values.len(), b.values.len());
    for (x, y) in a.values.iter().zip(&b.values) {
        assert!(close(f64::from(*x), f64::from(*y)));
    }
    let bad = IonWindow {
        mz: f64::NAN,
        tolerance: IonTolerance::Da(0.1),
    };
    assert!(ms.ion_plane(bad, &[], &cancel).is_err());
    // A cancelled ion image is refused, and not cached.
    let gone = CancelToken::new();
    gone.cancel();
    let w = IonWindow {
        mz: 500.0,
        tolerance: IonTolerance::Da(1.0),
    };
    assert!(ms.ion_plane(w, &[], &gone).is_err());
    assert!(ms.ion_plane(w, &[], &cancel).is_ok());
}

#[test]
fn missing_ibd_is_refused() {
    let p = data_dir().join("continuous_f32.imzML");
    let bytes = std::fs::read(&p).unwrap();
    let src = Arc::new(Source::from_vec("/nonexistent-dir/orphan.imzML", bytes));
    let err = crate::Registry::new().open(src).err().unwrap();
    assert!(err.to_string().contains("orphan.ibd"), "{err}");
}

#[test]
fn sniffing() {
    let head = std::fs::read(data_dir().join("processed_f32_i32.imzML")).unwrap();
    assert!(sniff(&head[..head.len().min(SNIFF)]));
    assert!(!sniff(
        b"<?xml version=\"1.0\"?><mzML><cvList><cv id=\"MS\"/></cvList>"
    ));
    assert!(!sniff(b"<?xml version=\"1.0\"?><svg/>"));
    assert_eq!(num(1.5000, 4), "1.5");
    assert_eq!(num(3.0, 3), "3");
    assert_eq!(num(120.0, 0), "120");
}

/// Open, TIC and ion-image timings on the corpus (tools/corpus/fetch-imzml.sh):
/// `SPECULAR_IMZML_DIR=… cargo test -p specular-decode imzml_corpus_timings -- --nocapture`.
#[test]
fn imzml_corpus_timings() {
    let Some(dir) = std::env::var_os("SPECULAR_IMZML_DIR") else {
        return;
    };
    let mut files = Vec::new();
    for sub in std::fs::read_dir(&dir).unwrap() {
        let sub = sub.unwrap().path();
        if sub.is_dir() {
            for f in std::fs::read_dir(&sub).unwrap() {
                let f = f.unwrap().path();
                if f.extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("imzml"))
                {
                    files.push(f);
                }
            }
        }
    }
    files.sort();
    let cancel = CancelToken::new();
    for f in files {
        let t = std::time::Instant::now();
        let s = open(&f);
        let t_open = t.elapsed();
        let sc = s.science().unwrap();
        let ms = sc.mass_spec.clone().unwrap();
        let t = std::time::Instant::now();
        let tic = ms.tic_plane(&sc.start, &cancel).unwrap();
        let t_tic = t.elapsed();
        let (lo, hi) = ms.mz_range().unwrap();
        let mid = (lo + hi) / 2.0;
        let t = std::time::Instant::now();
        let ion = ms
            .ion_plane(
                IonWindow {
                    mz: mid,
                    tolerance: IonTolerance::Ppm(10.0),
                },
                &sc.start,
                &cancel,
            )
            .unwrap();
        let t_ion = t.elapsed();
        // An ion image at a real peak: the base peak of the spot nearest the centre.
        let (w, h) = (sc.width, sc.height);
        let spots: Vec<(u32, u32)> = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .filter(|&(x, y)| ms.has_spectrum(x, y, &sc.start))
            .collect();
        let &(cx, cy) = spots
            .iter()
            .min_by_key(|&&(x, y)| x.abs_diff(w / 2).pow(2) + y.abs_diff(h / 2).pow(2))
            .unwrap();
        let peak = IonWindow {
            mz: ms.base_peak(cx, cy, &sc.start).unwrap(),
            tolerance: IonTolerance::Da(0.1),
        };
        let t = std::time::Instant::now();
        let at_peak = ms.ion_plane(peak, &sc.start, &cancel).unwrap();
        let t_peak = t.elapsed();
        // The spectra of 16 spots spread over the image, as the pointer would visit them.
        let spectra = sc.spectra.as_ref().unwrap();
        let step = (spots.len() / 16).max(1);
        let t = std::time::Instant::now();
        let n = spots
            .iter()
            .step_by(step)
            .take(16)
            .filter(|&&(x, y)| spectra.at(x, y, &sc.start).is_some())
            .count();
        let t_spec = t.elapsed();
        assert_eq!(tic.values.len(), ion.values.len());
        assert_eq!(tic.values.len(), at_peak.values.len());
        println!(
            "{}: {} spectra, {w}×{h}, open {t_open:?}, TIC {t_tic:?}, ion mid ±10 ppm {t_ion:?}, \
             ion at base peak {:.4} ±0.1 Da {t_peak:?}, {n} pixel spectra {t_spec:?}",
            f.display(),
            ms.imzml.entries.len(),
            peak.mz,
        );
    }
}
