//! Real-camera samples from `target/agent-jpeg/raw-samples/` (never committed; skipped when
//! absent): previews, metadata and sensor decode.

mod common;

use specular_raw::{DecodeOptions, OutputSize, RawFormat};

#[test]
fn samples() {
    let Some(dir) = common::samples_dir() else {
        eprintln!("raw samples not present; skipping");
        return;
    };
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    paths.sort();
    for p in paths {
        let data = std::fs::read(&p).unwrap();
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let want = match ext.as_str() {
            "dng" => RawFormat::Dng,
            "cr2" => RawFormat::Cr2,
            "cr3" => RawFormat::Cr3,
            "nef" => RawFormat::Nef,
            "arw" => RawFormat::Arw,
            "rw2" => RawFormat::Rw2,
            "raf" => RawFormat::Raf,
            "orf" => RawFormat::Orf,
            "pef" => RawFormat::Pef,
            _ => continue,
        };
        assert_eq!(specular_raw::detect(&data), Some(want), "{}", p.display());
        let raw = specular_raw::parse(&data).unwrap();
        let info = raw.info();
        assert_eq!(info.format, Some(want));
        assert!(
            !info.make.is_empty() && !info.model.is_empty(),
            "{}",
            p.display()
        );
        assert!((1..=8).contains(&info.orientation));
        // DNGs need not carry a preview (e.g. converted cine frames); vendor RAWs always do.
        match info.largest_preview() {
            Some(prev) => {
                let img = specular_raw::decode_preview(&data, prev, 256).unwrap();
                assert!(img.width > 0 && img.rgba.len() == (img.width * img.height * 4) as usize);
                eprintln!(
                    "{}: {} previews, largest {}x{} ({})",
                    p.display(),
                    info.previews.len(),
                    prev.width,
                    prev.height,
                    prev.source
                );
            }
            None => assert_eq!(want, RawFormat::Dng, "{}: no preview", p.display()),
        }
        eprintln!(
            "    sensor {:?}",
            info.sensor
                .as_ref()
                .map(|s| (s.width, s.height, s.compression, s.decodable))
        );
        if raw.can_decode_sensor() {
            let opts = DecodeOptions {
                size: OutputSize::Half,
                ..Default::default()
            };
            let r = raw.decode_raw(&data, &opts).unwrap();
            assert!(!r.truncated, "{}: truncated sensor data", p.display());
            assert!(r.white > r.black.iter().cloned().fold(0.0, f32::max));
            let img = specular_raw::develop(&r, &opts).unwrap();
            let mean = img.data.iter().map(|&v| v as f64).sum::<f64>() / img.data.len() as f64;
            assert!(
                mean > 200.0 && mean < 60000.0,
                "{}: mean {mean}",
                p.display()
            );
        }
    }
}
