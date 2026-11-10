//! Zeiss CZI through the registry: each corpus file (tools/corpus/fetch-czi.sh; skipped when
//! absent) opens with its scenes and associated images as pages, its facts (pixel size,
//! channels and their colours, Z) as the oracles give them (tools/corpus/czi-oracle.py), its
//! stacks as science planes equal to the oracle's pixels, and decodes at every level.
//!
//! libCZI runs in the decode host: these tests need `specular-decode-host` built (next to
//! the test binaries' folder, or `SPECULAR_DECODE_HOST`) with its libCZI bridge; they skip
//! with the reason otherwise.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use specular_core::{CancelToken, Source};
use specular_decode::science::ScienceKind;
use specular_decode::{DecodeError, DecodeRequest, Format, PageLabel, Rect, Registry, SlideImage};

fn corpus() -> Option<PathBuf> {
    let d = std::env::var_os("SPECULAR_CZI_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/czi"));
    d.join("expected.tsv").is_file().then_some(d)
}

fn sha(b: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(b)
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}

/// expected.tsv by file.
fn expected(dir: &Path) -> BTreeMap<String, Vec<Vec<String>>> {
    let mut m: BTreeMap<String, Vec<Vec<String>>> = BTreeMap::new();
    for l in std::fs::read_to_string(dir.join("expected.tsv"))
        .unwrap()
        .lines()
    {
        let f: Vec<String> = l.split('\t').map(str::to_owned).collect();
        m.entry(f[1].clone()).or_default().push(f);
    }
    m
}

#[test]
fn corpus_opens_through_the_registry() {
    let Some(dir) = corpus() else {
        eprintln!("SKIP: CZI corpus absent: run tools/corpus/fetch-czi.sh");
        return;
    };
    let reg = Registry::new();
    let cancel = CancelToken::new();
    let mut checked = 0;
    for (file, lines) in expected(&dir) {
        let path = dir.join(&file);
        let t0 = Instant::now();
        let s = match reg.open(Arc::new(Source::open(&path).unwrap())) {
            Ok(s) => s,
            Err(DecodeError::Unsupported(m)) if m.contains("libCZI") || m.contains("host") => {
                eprintln!("SKIP: {m}");
                return;
            }
            Err(e) => panic!("{file}: {e}"),
        };
        eprintln!(
            "{file}: opened in {:.1} ms, {} pages",
            t0.elapsed().as_secs_f64() * 1e3,
            s.page_count()
        );
        assert_eq!(s.info().format, Format::Czi, "{file}");
        let scenes: Vec<&Vec<String>> = lines.iter().filter(|l| l[0] == "scene").collect();
        let attachments = lines.iter().filter(|l| l[0] == "attach").count() as u32;
        let images = if scenes.len() > 1 {
            scenes.len() as u32
        } else {
            1
        };
        assert_eq!(s.page_count(), images + attachments, "{file}");
        // Facts: microns per pixel.
        let sci = s.science().expect("science summary");
        let scaling = lines.iter().find(|l| l[0] == "scaling").unwrap();
        let x: Option<f64> = scaling[2].split(',').next().unwrap().parse().ok();
        match (sci.pixel.as_ref(), x) {
            (Some(p), Some(x)) => {
                assert!(
                    (p.x.step - x * 1e6).abs() < 1e-9 * x * 1e6,
                    "{file}: {}",
                    p.x.step
                )
            }
            (None, None) => {}
            (p, x) => panic!("{file}: pixel {p:?} vs {x:?}"),
        }
        assert_eq!(sci.vendor.as_deref(), Some("Zeiss"));
        for page in 0..s.page_count() {
            let p = s.open_page(page).unwrap();
            let (w, h) = (p.info().width, p.info().height);
            assert!(w > 0 && h > 0);
            if page >= images {
                assert!(
                    matches!(p.page_label(), Some(PageLabel::Slide(_))),
                    "{file}"
                );
                let r = p.decode(&DecodeRequest::full(1), &cancel).unwrap();
                assert_eq!((r.width, r.height), (w, h));
                continue;
            }
            // The page's scene as the oracle sees it.
            if scenes.len() > 1 {
                let want = &scenes[page as usize][3];
                let v: Vec<u32> = want
                    .split(',')
                    .skip(2)
                    .map(|x| x.parse().unwrap())
                    .collect();
                assert_eq!((w, h), (v[0], v[1]), "{file} page {page}");
                assert!(matches!(
                    p.page_label(),
                    Some(PageLabel::Slide(SlideImage::Other(_)))
                ));
            }
            // Every offered scale decodes to its size; a region at full resolution too.
            for &sc in p.info().scales.iter().take(6) {
                // Whole slides are never decoded at full size (the viewer reads tiles).
                if u64::from(w.div_ceil(sc)) * u64::from(h.div_ceil(sc)) > 1 << 25 {
                    continue;
                }
                let t0 = Instant::now();
                let r = p.decode(&DecodeRequest::full(sc), &cancel).unwrap();
                assert_eq!(
                    (r.width, r.height),
                    (w.div_ceil(sc), h.div_ceil(sc)),
                    "{file} 1/{sc}"
                );
                if sc == 1 || r.width <= 1024 {
                    eprintln!(
                        "  page {page}: 1/{sc} {}×{} in {:.1} ms",
                        r.width,
                        r.height,
                        t0.elapsed().as_secs_f64() * 1e3
                    );
                }
            }
            let req = DecodeRequest {
                region: Some(Rect {
                    x: w / 3,
                    y: h / 3,
                    w: (w / 3).max(1),
                    h: (h / 3).max(1),
                }),
                ..DecodeRequest::full(1)
            };
            let r = p.decode(&req, &cancel).unwrap();
            assert_eq!((r.width, r.height), ((w / 3).max(1), (h / 3).max(1)));
            // Science planes of a stack: the oracle's whole-scene plane at full resolution.
            let sci = p.science().unwrap();
            if sci.raster_only {
                assert_eq!(sci.kind, ScienceKind::Slide);
                continue;
            }
            let scene = if scenes.len() > 1 {
                scenes[page as usize][2].clone()
            } else {
                "-1".into()
            };
            for l in lines.iter().filter(|l| {
                l[0] == "plane"
                    && l[2] == scene
                    && l[7] == "1.0"
                    && l[5] == w.to_string()
                    && l[6] == h.to_string()
            }) {
                let coords: BTreeMap<&str, u32> = l[8]
                    .split(',')
                    .filter_map(|c| c.split_once('='))
                    .map(|(k, v)| (k, v.parse().unwrap()))
                    .collect();
                let pixel: u8 = l[9].parse().unwrap();
                if pixel != 1 {
                    continue;
                }
                let index: Vec<u32> = sci
                    .axes
                    .iter()
                    .map(|a| *coords.get(a.name.as_str()).unwrap_or(&0))
                    .collect();
                let plane = p.nd_plane(&index, &cancel).unwrap().unwrap();
                let bytes: Vec<u8> = plane
                    .values
                    .iter()
                    .flat_map(|v| (*v as u16).to_le_bytes())
                    .collect();
                assert_eq!(sha(&bytes), l[12], "{file} plane {index:?}");
                checked += 1;
            }
            // Channel colours and names from the display settings.
            let chans: Vec<&Vec<String>> = lines.iter().filter(|l| l[0] == "channel").collect();
            if let Some(ca) = sci.channel_axis {
                assert_eq!(sci.axes[ca].len as usize, chans.len(), "{file}");
                if !sci.channel_colours.is_empty() {
                    for (c, l) in sci.channel_colours.iter().zip(&chans) {
                        let tint = l[3].split('|').nth(1).unwrap();
                        let got = format!(
                            "#{:02X}{:02X}{:02X}",
                            (c[0] * 255.0).round() as u8,
                            (c[1] * 255.0).round() as u8,
                            (c[2] * 255.0).round() as u8
                        );
                        assert_eq!(got, tint, "{file}");
                    }
                }
            }
        }
    }
    eprintln!("{checked} science planes equal to the oracle's");
    assert!(checked >= 3, "{checked}");
}
