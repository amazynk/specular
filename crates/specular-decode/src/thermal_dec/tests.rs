use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::*;

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../formats/specular-fmt-thermal/tests/data")
}

fn open(path: &Path) -> Arc<dyn Session> {
    crate::Registry::new()
        .open(Arc::new(Source::open(path).unwrap()))
        .unwrap()
}

fn whole(s: &dyn Session) -> Raster {
    s.decode(
        &DecodeRequest {
            scale: 1,
            region: None,
            parallel: false,
            prepare_regions: false,
        },
        &CancelToken::default(),
    )
    .unwrap()
}

#[test]
fn formats_by_content() {
    for (name, f) in [
        ("rjpeg.jpg", Format::FlirJpeg),
        ("fff_png.fff", Format::FlirFff),
        ("fff_tiff_be.fff", Format::FlirFff),
        ("irg_c201.irg", Format::InfirayIrg),
        ("irg_p200.irg", Format::InfirayIrg),
        ("irb_c0.irb", Format::InfratecIrb),
        ("irb_c2.irb", Format::InfratecIrb),
        // A FLIR JPEG without raw data is an ordinary JPEG.
        ("flir_noraw.jpg", Format::Jpeg),
    ] {
        let s = open(&data_dir().join(name));
        assert_eq!(s.info().format, f, "{name}");
        let sci = s.science();
        assert_eq!(sci.is_some(), f != Format::Jpeg, "{name}");
        if let Some(sci) = sci {
            assert_eq!(sci.kind, ScienceKind::Thermal);
            assert_eq!(sci.luts.len(), 1);
            assert!(sci.thermal.is_some());
        }
    }
    // A plain JPEG stays with the JPEG decoder.
    let jpg = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/jpeg-calibration.jpg");
    assert_eq!(open(&jpg).info().format, Format::Jpeg);
}

#[test]
fn temperatures_in_each_unit() {
    let s = open(&data_dir().join("fff_raw_le.fff"));
    let t = s.science().unwrap().thermal.clone().unwrap();
    assert!(t.adjustable());
    let c = t.file_conditions();
    let cancel = CancelToken::default();
    let base = t.plane(&c, TemperatureUnit::Celsius, &cancel).unwrap();
    let f = t.plane(&c, TemperatureUnit::Fahrenheit, &cancel).unwrap();
    let k = t.plane(&c, TemperatureUnit::Kelvin, &cancel).unwrap();
    for i in [0, base.values.len() / 2, base.values.len() - 1] {
        let v = f64::from(base.values[i]);
        assert!((f64::from(f.values[i]) - (v * 1.8 + 32.0)).abs() < 1e-3);
        assert!((f64::from(k.values[i]) - (v + 273.15)).abs() < 1e-3);
    }
    // nd_plane is the recorded conditions in degrees Celsius, the same cached plane.
    let nd = s.nd_plane(&[], &cancel).unwrap().unwrap();
    assert!(Arc::ptr_eq(&nd, &base));
    // Lower emissivity: the hot pixels read hotter.
    let e = ThermalConditions {
        emissivity: Some(0.5),
        ..c
    };
    let hot = t.plane(&e, TemperatureUnit::Celsius, &cancel).unwrap();
    let (_, hi) = ThermalImage::range_of(&base).unwrap();
    let (_, hi2) = ThermalImage::range_of(&hot).unwrap();
    assert!(hi2 > hi + 10.0, "{hi} {hi2}");
    for u in TemperatureUnit::ALL {
        assert!((u.to_celsius(u.from_celsius(36.6)) - 36.6).abs() < 1e-9);
        assert_eq!(u.next().next().next(), u);
    }
    assert!(s.memory_bytes() > 0);
    assert!(s.trim_memory() > 0);
}

#[test]
fn thumbnail_is_the_palette_mapped_temperature() {
    let s = open(&data_dir().join("rjpeg.jpg"));
    let r = whole(s.as_ref());
    assert_eq!((r.width, r.height), (16, 12));
    let lut = s.science().unwrap().luts[0].clone();
    // The camera palette: coldest pixel its first colour, hottest its last.
    let t = s.science().unwrap().thermal.clone().unwrap();
    let p = s.nd_plane(&[], &CancelToken::default()).unwrap().unwrap();
    let (lo, hi) = ThermalImage::range_of(&p).unwrap();
    assert_eq!(r.layout, PixelLayout::Rgba);
    let px = &r.planes[0].data;
    let at = |i: usize| [px[i * 4], px[i * 4 + 1], px[i * 4 + 2]];
    let cold = p.values.iter().position(|&v| v == lo).unwrap();
    let hot = p.values.iter().position(|&v| v == hi).unwrap();
    assert_eq!(at(cold), lut[0]);
    assert_eq!(at(hot), lut[255]);
    assert_eq!(
        t.thermal().palette.as_ref().unwrap().colours[0],
        lut[0],
        "the camera palette, not the fallback"
    );
    // Without a palette: the iron ramp, dark to light.
    let g = open(&data_dir().join("irg_c201.irg"));
    let l = &g.science().unwrap().luts[0];
    let lum = |c: [u8; 3]| u32::from(c[0]) + u32::from(c[1]) + u32::from(c[2]);
    assert!(lum(l[0]) < 10 && lum(l[255]) > 700);
}

#[test]
fn visible_photo_is_page_two() {
    let s = open(&data_dir().join("rjpeg.jpg"));
    assert_eq!(s.page_count(), 2);
    assert_eq!(s.page_label(), Some(PageLabel::Thermal { visible: false }));
    let v = s.open_page(1).unwrap();
    assert_eq!((v.info().width, v.info().height), (24, 18));
    assert_eq!(v.page_index(), 1);
    assert_eq!(v.page_label(), Some(PageLabel::Thermal { visible: true }));
    assert!(v.science().is_none());
    let r = whole(v.as_ref());
    assert_eq!((r.width, r.height), (24, 18));
    let back = v.open_page(0).unwrap();
    assert_eq!(back.info().format, Format::FlirJpeg);
    assert!(back.science().is_some());
    assert!(s.open_page(2).is_err());
    // Without a visible photo: one page, no label.
    let p = open(&data_dir().join("irg_p200.irg"));
    assert_eq!(p.page_count(), 1);
    assert_eq!(p.page_label(), None);
}

#[test]
fn keywords_carry_the_calibration() {
    let s = open(&data_dir().join("rjpeg.jpg"));
    let k = &s.science().unwrap().keywords;
    let has = |n: &str| k.iter().any(|(key, _, _)| key == n);
    for n in [
        "Camera",
        "Lens",
        "Emissivity",
        "Planck R1",
        "Planck B",
        "Raw range",
        "Temperature range",
        "Palette",
        "Visible photo",
    ] {
        assert!(has(n), "{n}: {k:?}");
    }
}

fn median(mut v: Vec<Duration>) -> Duration {
    v.sort();
    v[v.len() / 2]
}

/// The R-JPEG check every JPEG pays (a walk over the marker segment headers before the scan)
/// against opening the JPEG: the check must be noise.
#[test]
fn plain_jpeg_open_is_unchanged() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/jpeg-calibration.jpg");
    let src = Arc::new(Source::open(&path).unwrap());
    let reg = crate::Registry::new();
    let n = 31;
    let mut sniff = Vec::new();
    let mut direct = Vec::new();
    let mut registry = Vec::new();
    for _ in 0..n {
        let t = Instant::now();
        let thermal = std::hint::black_box(th::is_kind(src.bytes(), Kind::FlirJpeg));
        sniff.push(t.elapsed());
        assert!(!thermal);
        let t = Instant::now();
        std::hint::black_box(jpeg::JpegDecoder.open(src.clone()).unwrap());
        direct.push(t.elapsed());
        let t = Instant::now();
        std::hint::black_box(reg.open(src.clone()).unwrap());
        registry.push(t.elapsed());
    }
    let (s, d, r) = (median(sniff), median(direct), median(registry));
    eprintln!(
        "plain JPEG ({} bytes): R-JPEG check {:.2} µs, JPEG open {:.1} µs, registry open {:.1} µs \
         (medians of {n})",
        src.len(),
        s.as_secs_f64() * 1e6,
        d.as_secs_f64() * 1e6,
        r.as_secs_f64() * 1e6
    );
    // Loose bounds: the machine may be busy.
    assert!(s < Duration::from_micros(50).max(d / 20), "{s:?} vs {d:?}");
}

/// Open and first-frame times of the largest file of the fetched corpus
/// (tools/corpus/fetch-thermal.sh): `SPECULAR_THERMAL_DIR=tools/corpus/thermal`.
#[test]
fn thermal_corpus_timings() {
    let Ok(dir) = std::env::var("SPECULAR_THERMAL_DIR") else {
        return;
    };
    let mut files = Vec::new();
    let mut stack = vec![PathBuf::from(dir)];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
            } else if matches!(
                p.extension().and_then(|e| e.to_str()),
                Some("jpg" | "irb" | "irg" | "fff")
            ) {
                files.push((e.metadata().unwrap().len(), p));
            }
        }
    }
    files.sort();
    let reg = crate::Registry::new();
    let mut seen = std::collections::HashSet::new();
    // The largest file of each format.
    for (_, p) in files.iter().rev() {
        let t0 = Instant::now();
        let s = reg.open(Arc::new(Source::open(p).unwrap())).unwrap();
        let open = t0.elapsed();
        if !seen.insert(s.info().format) {
            continue;
        }
        let t1 = Instant::now();
        let r = whole(s.as_ref());
        let first = t1.elapsed();
        assert!(s.science().is_some(), "{}", p.display());
        eprintln!(
            "{}: {} {}×{}: open {:.2} ms, first frame {:.2} ms",
            p.display(),
            s.info().format.name(),
            r.width,
            r.height,
            open.as_secs_f64() * 1e3,
            first.as_secs_f64() * 1e3
        );
    }
}

#[test]
fn sequences_are_frames_on_a_frame_axis() {
    for (name, frames) in [("seq_raw.seq", 6u32), ("csq_jls.csq", 4)] {
        let s = open(&data_dir().join(name));
        assert_eq!(s.info().format, Format::FlirSeq, "{name}");
        assert!(s.animation().is_none(), "plays in science mode");
        assert_eq!(s.page_count(), 1);
        let sci = s.science().unwrap();
        assert_eq!(sci.axes.len(), 1);
        let axis = &sci.axes[0];
        assert_eq!((axis.kind, axis.len), (AxisKind::Frame, frames));
        assert_eq!(sci.video.as_ref().unwrap().frames, frames);
        let t = sci.thermal.clone().unwrap();
        assert_eq!(t.frame_count(), frames);
        assert!(t.frame_rate().unwrap() > 20.0);
        // The hot spot moves right frame by frame: each frame's own plane.
        let hot = |k: u32| {
            let p = s.nd_plane(&[k], &CancelToken::default()).unwrap().unwrap();
            let (i, _) = p
                .values
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .unwrap();
            i as u32 % p.width
        };
        assert!(hot(0) < hot(2), "{name}");
        // Unit and conditions follow, per frame.
        let c = t.file_conditions();
        let k = t
            .plane_at(1, &c, TemperatureUnit::Kelvin, &CancelToken::default())
            .unwrap();
        let celsius = t
            .plane_at(1, &c, TemperatureUnit::Celsius, &CancelToken::default())
            .unwrap();
        assert!((k.values[0] - celsius.values[0] - 273.15).abs() < 1e-3);
        assert!(t.frame(frames).is_err());
        // The thumbnail is the first frame.
        let r = whole(s.as_ref());
        assert_eq!((r.width, r.height), (16, 12));
    }
    let s = open(&data_dir().join("seq_raw.seq"));
    let axis = &s.science().unwrap().axes[0];
    assert_eq!(axis.labels[1], "0.04 s");
    let t = s.science().unwrap().thermal.clone().unwrap();
    assert!((t.frame_seconds(5) - 0.2).abs() < 1e-6);
    assert!((t.frame_duration(5) - 0.04).abs() < 1e-6);
}

/// A frame recorded with another emissivity is computed with it, unless the parameters
/// were edited.
#[test]
fn sequence_frames_keep_their_own_parameters() {
    let mut d = std::fs::read(data_dir().join("seq_raw.seq")).unwrap();
    // Frame 2's CameraInfo emissivity (0.95, little-endian; one per frame).
    let at: Vec<usize> = d
        .windows(4)
        .enumerate()
        .filter(|(_, w)| *w == 0.95f32.to_le_bytes())
        .map(|(i, _)| i)
        .collect();
    assert_eq!(at.len(), 6);
    d[at[2]..at[2] + 4].copy_from_slice(&0.5f32.to_le_bytes());
    let dir = std::env::temp_dir().join(format!("specular-seqcond-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("cond.seq");
    std::fs::write(&p, &d).unwrap();
    let s = open(&p);
    let t = s.science().unwrap().thermal.clone().unwrap();
    let file = t.file_conditions();
    assert!((file.emissivity.unwrap() - 0.95).abs() < 1e-6);
    assert!((t.frame_conditions(2).emissivity.unwrap() - 0.5).abs() < 1e-6);
    assert_eq!(t.effective_conditions(2, &file), t.frame_conditions(2));
    let edited = ThermalConditions {
        emissivity: Some(0.8),
        ..file
    };
    assert_eq!(t.effective_conditions(2, &edited), edited);
    // As recorded: frame 2 under 0.5, which reads hotter than under frame 0's 0.95.
    let own = t.frame_values(2, &file, TemperatureUnit::Celsius).unwrap();
    let frame = t.frame(2).unwrap();
    let as_first = frame.temperatures(&file);
    let as_own = frame.temperatures(&t.frame_conditions(2));
    assert_eq!(own, as_own);
    assert_ne!(own, as_first);
    let _ = std::fs::remove_dir_all(dir);
}

/// Index and frame timings of the fetched sequences (tools/corpus/fetch-thermal-seq.sh).
#[test]
fn thermal_seq_corpus_timings() {
    let Ok(dir) = std::env::var("SPECULAR_SEQ_DIR") else {
        return;
    };
    let reg = crate::Registry::new();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if !matches!(p.extension().and_then(|e| e.to_str()), Some("seq" | "csq")) {
            continue;
        }
        let t0 = Instant::now();
        let s = reg.open(Arc::new(Source::open(&p).unwrap())).unwrap();
        let open = t0.elapsed();
        let t = s.science().unwrap().thermal.clone().unwrap();
        let n = t.frame_count();
        let c = t.file_conditions();
        let t1 = Instant::now();
        for k in 0..n {
            t.frame_values(k, &c, TemperatureUnit::Celsius).unwrap();
        }
        let per = t1.elapsed() / n;
        eprintln!(
            "{}: {n} frames {}×{}: open {:.2} ms, {:.2} ms a frame",
            p.display(),
            s.info().width,
            s.info().height,
            open.as_secs_f64() * 1e3,
            per.as_secs_f64() * 1e3
        );
    }
}

fn open_err(path: &Path) -> DecodeError {
    match crate::Registry::new().open(Arc::new(Source::open(path).unwrap())) {
        Ok(s) => panic!("{} opened as {:?}", path.display(), s.info().format),
        Err(e) => e,
    }
}

/// A copy of fixture `name` under another file name (the extension decides for BMT).
fn renamed(name: &str, to: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("specular-thermal2-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(to);
    std::fs::copy(data_dir().join(name), &p).unwrap();
    p
}

#[test]
fn testo_and_fluke_by_name_and_content() {
    for (name, f) in [
        ("bmt_872.bmt", Format::TestoBmt),
        ("bmt_bare.bmt", Format::TestoBmt),
        ("is2_json.is2", Format::FlukeIs2),
        ("is2_bare.is2", Format::FlukeIs2),
    ] {
        let s = open(&data_dir().join(name));
        assert_eq!(s.info().format, f, "{name}");
        let sci = s.science().unwrap();
        assert_eq!(sci.kind, ScienceKind::Thermal);
        let t = sci.thermal.clone().unwrap();
        assert_eq!(t.adjustable(), f == Format::FlukeIs2, "{name}");
        let r = whole(s.as_ref());
        assert_eq!(
            (r.width as usize, r.height as usize),
            (t.thermal().width, t.thermal().height)
        );
        let keys: Vec<&str> = sci.keywords.iter().map(|k| k.0.as_str()).collect();
        assert!(
            keys.contains(&"Scale") || keys.contains(&"Calibration curve 1"),
            "{keys:?}"
        );
        assert!(keys.contains(&"Camera range"), "{name}");
    }
    let dir = renamed("is2_json.is2", "photo.zip");
    // Content alone: an IS2 under another name is still one; a BMT led by its JPEG preview
    // is one only with its extension (otherwise the JPEG decoder takes the preview).
    assert_eq!(open(&dir).info().format, Format::FlukeIs2);
    assert_ne!(
        crate::Registry::new()
            .open(Arc::new(
                Source::open(renamed("bmt_872.bmt", "preview.jpg")).unwrap()
            ))
            .map(|s| s.info().format)
            .ok(),
        Some(Format::TestoBmt)
    );
    assert_eq!(
        open(&renamed("bmt_bare.bmt", "bare.dat")).info().format,
        Format::TestoBmt
    );
    // The older layouts, and IS2 without IR.data, are refused with the reason.
    for (name, why) in [
        ("is2_old.is2", th::OLD_IS2_REASON),
        ("bmt_old.bmt", th::OLD_BMT_REASON),
        ("is2_noir.is2", th::NO_IR_DATA_REASON),
    ] {
        match open_err(&data_dir().join(name)) {
            DecodeError::Unsupported(s) => assert!(s.contains(why), "{name}: {s}"),
            e => panic!("{name}: {e:?}"),
        }
    }
    // Something else named .bmt is not claimed.
    let other = renamed("irg_p200.irg", "notes.bmt");
    assert_eq!(open(&other).info().format, Format::InfirayIrg);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}
