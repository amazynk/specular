//! Zeiss CZI through libCZI in the decode host, end to end through the viewer-side client.
//!
//! Without the libCZI bridge (libCZI not found when the host was built) the host must say so
//! in plain words; the other tests then skip. With it:
//! - the sandbox stays complete in a host that loaded libCZI;
//! - hostile files (noise behind the magic, truncated documents) are refused, never crash;
//! - the corpus (tools/corpus/fetch-czi.sh; skipped when absent) matches the oracles in its
//!   `expected.tsv` (tools/corpus/czi-oracle.py: pylibCZIrw, the metadata XML, CZIcmd):
//!   dimensions, bounding boxes, scenes, scaling, display settings, and planes and attachments
//!   bit for bit (SHA-256 of the pixels).
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

use sha2::{Digest, Sha256};
use specular_sandbox::czi::CziRect;
use specular_sandbox::protocol::{CziPlaneKind, CziRegion, ErrorKind, Response, TestOp};
use specular_sandbox::{Config, CziInfo, CziSource, Error, Host, SandboxLevel};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn config() -> Config {
    Config {
        exe: Some(env!("CARGO_BIN_EXE_specular-decode-host").into()),
        timeout: Duration::from_secs(120),
        pool_size: 1,
        test_hooks: true,
        czi: true,
        ..Config::default()
    }
}

/// A host with libCZI, or `None` (with the reason) when the bridge was not built.
fn czi_host() -> Option<Host> {
    let h = Host::new(config());
    let info = h.info().unwrap();
    match info.czi {
        Some(v) => {
            eprintln!("libCZI {v}");
            Some(h)
        }
        None => {
            eprintln!("SKIP: libCZI not available ({})", info.czi_error);
            None
        }
    }
}

fn corpus() -> Option<PathBuf> {
    let d = std::env::var_os("SPECULAR_CZI_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/czi"));
    d.join("expected.tsv").is_file().then_some(d)
}

fn hex(b: &[u8]) -> String {
    Sha256::digest(b)
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}

fn region(
    raw: CziRect,
    zoom: f32,
    scene: Option<u32>,
    coords: &[(u8, i32)],
    pixel: u8,
) -> CziRegion {
    let mut c = [(0u8, 0i32); 8];
    c[..coords.len()].copy_from_slice(coords);
    CziRegion {
        x: raw.x,
        y: raw.y,
        w: raw.w,
        h: raw.h,
        zoom,
        scene,
        coords: c,
        coord_count: coords.len() as u8,
        pixel,
        max_bytes: 1 << 30,
    }
}

fn info_of(h: &Host, src: &CziSource) -> Result<CziInfo, Error> {
    let raw = h.czi_info(src, 16 << 20, 64 << 20)?;
    Ok(CziInfo::decode(&raw, 16 << 20).expect("libCZI's facts decode"))
}

#[test]
fn missing_bridge_is_named() {
    let h = Host::new(Config {
        env: vec![(
            "SPECULAR_CZI_BRIDGE".into(),
            "/nonexistent/libspecular-czi.so".into(),
        )],
        ..config()
    });
    let info = h.info().unwrap();
    assert!(info.czi.is_none());
    assert!(info.czi_error.contains("libCZI"), "{}", info.czi_error);
    let src = CziSource::from_bytes(b"ZISRAWFILE").unwrap();
    match h.czi_info(&src, 0, 1 << 20) {
        Err(Error::Remote {
            kind: ErrorKind::NotInstalled,
            message,
        }) => assert!(message.contains("libCZI"), "{message}"),
        r => panic!("{r:?}"),
    }
    // A host started without --czi says it was not started for CZI files.
    let plain = Host::new(Config {
        czi: false,
        ..config()
    });
    assert!(plain.info().unwrap().czi.is_none());
}

#[test]
fn sandbox_holds_with_libczi_loaded() {
    let Some(h) = czi_host() else { return };
    let info = h.info().unwrap();
    #[cfg(target_os = "linux")]
    assert!(info.sandbox >= SandboxLevel::Partial, "{info:?}");
    #[cfg(not(target_os = "linux"))]
    assert_eq!(info.sandbox, SandboxLevel::Full, "{info:?}");
    let read_path = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml").to_string();
    let write_dir = env!("CARGO_TARGET_TMPDIR").to_string();
    let Ok(Response::Probe(results)) = h.test(TestOp::Probe {
        read_path,
        write_dir,
    }) else {
        panic!("probe failed")
    };
    for r in results {
        if matches!(r.name.as_str(), "read file" | "create file" | "run program") {
            assert!(!r.allowed, "sandbox allowed {}: {}", r.name, r.detail);
        }
    }
}

#[test]
fn hostile_files_are_refused() {
    let Some(h) = czi_host() else { return };
    // Noise behind the magic.
    let mut s = 0x2545_F491_4F6C_DD1Du64;
    let mut noise = b"ZISRAWFILE".to_vec();
    for _ in 0..(1 << 16) {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        noise.push(s as u8);
    }
    for bytes in [&b""[..], b"ZISRAWFILE", &noise] {
        let src = CziSource::from_bytes(bytes).unwrap();
        match info_of(&h, &src) {
            Err(Error::Remote { kind, message }) => {
                assert!(
                    matches!(
                        kind,
                        ErrorKind::Malformed | ErrorKind::Unsupported | ErrorKind::Io
                    ),
                    "{kind:?}: {message}"
                )
            }
            r => panic!("{r:?}"),
        }
    }
    // Truncated real documents: refused at open, or read with missing tiles, never a crash.
    if let Some(dir) = corpus() {
        let full = std::fs::read(dir.join("s_3_t_1_c_3_z_5.czi")).unwrap();
        for cut in [64usize, 4096, full.len() / 2, full.len() - 1000] {
            let src = CziSource::from_bytes(&full[..cut]).unwrap();
            if let Ok(info) = info_of(&h, &src) {
                let r = info.bbox_layer0;
                let _ = h.czi_read(
                    &src,
                    region(r, 0.25, None, &[(b'C', 0)], 1),
                    (r.w / 4, r.h / 4),
                );
            }
        }
    }
    // A region past the document, a zero-sized one and a far too large one.
    if let Some(dir) = corpus() {
        let src = CziSource::open(&dir.join("RGB-8bit.czi")).unwrap();
        info_of(&h, &src).unwrap();
        let far = CziRect {
            x: 1 << 30,
            y: 0,
            w: 64,
            h: 64,
        };
        let r = h
            .czi_read(&src, region(far, 1.0, None, &[], 3), (64, 64))
            .unwrap();
        assert!(r.2.iter().all(|&b| b == 0), "background outside the image");
        let zero = CziRect { w: 0, ..far };
        assert!(
            h.czi_read(&src, region(zero, 1.0, None, &[], 3), (0, 0))
                .is_err()
        );
        let huge = CziRect {
            x: 0,
            y: 0,
            w: 1 << 20,
            h: 1 << 20,
        };
        let mut q = region(huge, 1.0, None, &[], 3);
        q.max_bytes = 1 << 20;
        match h.czi_read(&src, q, (1 << 20, 1 << 20)) {
            Err(Error::Remote {
                kind: ErrorKind::TooLarge,
                ..
            }) => {}
            r => panic!("{:?}", r.map(|r| (r.0, r.1))),
        }
        // The host still serves after all that.
        info_of(&h, &src).unwrap();
    }
}

/// A line of expected.tsv, split.
fn lines(dir: &Path) -> Vec<Vec<String>> {
    std::fs::read_to_string(dir.join("expected.tsv"))
        .unwrap()
        .lines()
        .map(|l| l.split('\t').map(str::to_owned).collect())
        .collect()
}

fn parse_rect(s: &str) -> CziRect {
    let v: Vec<i64> = s.split(',').map(|x| x.parse().unwrap()).collect();
    CziRect {
        x: v[0] as i32,
        y: v[1] as i32,
        w: v[2] as u32,
        h: v[3] as u32,
    }
}

#[test]
fn corpus_matches_the_oracles() {
    let Some(h) = czi_host() else { return };
    let Some(dir) = corpus() else {
        eprintln!("SKIP: CZI corpus absent: run tools/corpus/fetch-czi.sh");
        return;
    };
    let mut infos: std::collections::BTreeMap<String, (CziSource, CziInfo)> = Default::default();
    let mut checked = 0;
    let mut mismatches = Vec::new();
    for l in lines(&dir) {
        let file = l[1].clone();
        if !infos.contains_key(&file) {
            let src = CziSource::open(&dir.join(&file)).unwrap();
            let t0 = Instant::now();
            let info = info_of(&h, &src).unwrap_or_else(|e| panic!("{file}: {e}"));
            eprintln!(
                "{file}: facts in {:.1} ms",
                t0.elapsed().as_secs_f64() * 1e3
            );
            infos.insert(file.clone(), (src, info));
        }
        let (src, info) = &infos[&file];
        let mut fail = |what: String| mismatches.push(format!("{file}: {what}"));
        checked += 1;
        match l[0].as_str() {
            "info" if l[2] == "dims" => {
                // pylibCZIrw adds the dimensions a document lacks as 0:1, and leaves out S.
                for d in l[3].split(',').filter(|d| *d != "-") {
                    let letter = d.as_bytes()[0];
                    let (start, size) = d[1..].split_once(':').unwrap();
                    let (start, size): (i32, u32) = (start.parse().unwrap(), size.parse().unwrap());
                    match info.dim(letter) {
                        Some(x) if (x.start, x.size) == (start, size) => {}
                        None if (start, size) == (0, 1) => {}
                        x => fail(format!("dimension {d}: {x:?}")),
                    }
                }
            }
            "info" if l[2] == "bbox" => {
                if info.bbox_layer0 != parse_rect(&l[3]) {
                    fail(format!("bbox {:?} vs {}", info.bbox_layer0, l[3]));
                }
            }
            "scene" => {
                let index: i32 = l[2].parse().unwrap();
                let want = parse_rect(&l[3]);
                match info.scenes.iter().find(|s| s.index == index) {
                    Some(s) if s.bbox_layer0 == want => {}
                    // A document with one scene may not mark its sub-blocks with S.
                    None if info.scenes.is_empty() && info.bbox_layer0 == want => {}
                    s => fail(format!("scene {index}: {s:?} vs {want:?}")),
                }
            }
            "scaling" => {
                for (k, v) in l[2].split(',').enumerate() {
                    let want: Option<f64> = v.parse().ok().filter(|v: &f64| *v > 0.0);
                    let got = info.scaling[k];
                    let ok = match (got, want) {
                        (Some(a), Some(b)) => (a - b).abs() <= b * 1e-9,
                        (None, None) => true,
                        _ => false,
                    };
                    if !ok {
                        fail(format!("scaling {k}: {got:?} vs {want:?}"));
                    }
                }
            }
            "channel" => {
                let i: usize = l[2].parse().unwrap();
                let parts: Vec<&str> = l[3].split('|').collect();
                let Some(d) = info.channels.get(i).and_then(|c| c.display) else {
                    fail(format!("channel {i}: no display settings"));
                    continue;
                };
                let tint = d.tint.map_or("-".to_owned(), |[r, g, b]| {
                    format!("#{r:02X}{g:02X}{b:02X}")
                });
                let (lo, hi): (f64, f64) = (parts[2].parse().unwrap(), parts[3].parse().unwrap());
                if tint != parts[1]
                    || (f64::from(d.black) - lo).abs() > 1e-6
                    || (f64::from(d.white) - hi).abs() > 1e-6
                {
                    fail(format!("channel {i}: {d:?} vs {}", l[3]));
                }
            }
            "plane" => {
                let scene: i64 = l[2].parse().unwrap();
                let raw = CziRect {
                    x: l[3].parse().unwrap(),
                    y: l[4].parse().unwrap(),
                    w: l[5].parse().unwrap(),
                    h: l[6].parse().unwrap(),
                };
                let zoom: f32 = l[7].parse().unwrap();
                let coords: Vec<(u8, i32)> = if l[8] == "-" {
                    Vec::new()
                } else {
                    l[8].split(',')
                        .map(|c| {
                            let (k, v) = c.split_once('=').unwrap();
                            (k.as_bytes()[0], v.parse().unwrap())
                        })
                        // pylibCZIrw names dimensions the document lacks; libCZI wants only
                        // its own.
                        .filter(|(k, _)| info.dim(*k).is_some())
                        .collect()
                };
                let pixel: u8 = l[9].parse().unwrap();
                let want: (u32, u32) = (l[10].parse().unwrap(), l[11].parse().unwrap());
                let t0 = Instant::now();
                let scene = u32::try_from(scene).ok();
                match h.czi_read(src, region(raw, zoom, scene, &coords, pixel), want) {
                    Ok((w, ht, data)) => {
                        eprintln!(
                            "{file}: {}×{} at {zoom} in {:.1} ms",
                            w,
                            ht,
                            t0.elapsed().as_secs_f64() * 1e3
                        );
                        if (w, ht) != want || hex(&data) != l[12] {
                            fail(format!(
                                "plane {raw:?} at {zoom}: {w}×{ht}, pixels differ ({})",
                                hex(&data)
                            ));
                        }
                    }
                    Err(e) => fail(format!("plane {raw:?} at {zoom}: {e}")),
                }
            }
            "plane8" => {
                let raw = CziRect {
                    x: l[2].parse().unwrap(),
                    y: l[3].parse().unwrap(),
                    w: l[4].parse().unwrap(),
                    h: l[5].parse().unwrap(),
                };
                let pixel = info.channels[0].pixel.unwrap();
                match h.czi_read(src, region(raw, 1.0, None, &[], pixel), (raw.w, raw.h)) {
                    Ok((w, ht, data)) => {
                        // OpenSlide's RGB at 8 bits: the top byte of 16-bit samples.
                        let bpp = if pixel == 4 { 6 } else { 3 };
                        let mut rgb = Vec::with_capacity(w as usize * ht as usize * 3);
                        for px in data.chunks_exact(bpp) {
                            for k in [2usize, 1, 0] {
                                rgb.push(if pixel == 4 { px[2 * k + 1] } else { px[k] });
                            }
                        }
                        if hex(&rgb) != l[6] {
                            fail(format!("OpenSlide plane {raw:?}: pixels differ"));
                        }
                    }
                    Err(e) => fail(format!("OpenSlide plane {raw:?}: {e}")),
                }
            }
            "attach" => {
                let Some(a) = info.attachments.iter().find(|a| a.name == l[2]) else {
                    fail(format!("attachment {} missing", l[2]));
                    continue;
                };
                match h.czi_attachment(src, a.index, 8192, 256 << 20) {
                    Ok((p, data)) => {
                        let ok = match l[3].as_str() {
                            "enc" => p.kind == CziPlaneKind::Encoded && hex(&data) == l[4],
                            _ => {
                                p.kind == CziPlaneKind::Pixels
                                    && p.width.to_string() == l[4]
                                    && p.height.to_string() == l[5]
                                    && p.pixel.to_string() == l[6]
                                    && hex(&data) == l[7]
                            }
                        };
                        if !ok {
                            fail(format!(
                                "attachment {}: {}×{} {:?}",
                                l[2], p.width, p.height, p.kind
                            ));
                        }
                    }
                    Err(e) => fail(format!("attachment {}: {e}", l[2])),
                }
            }
            _ => checked -= 1,
        }
    }
    eprintln!(
        "{checked} oracle values checked, {} mismatches",
        mismatches.len()
    );
    assert!(mismatches.is_empty(), "{mismatches:#?}");
    assert!(checked > 50, "{checked}");
}
