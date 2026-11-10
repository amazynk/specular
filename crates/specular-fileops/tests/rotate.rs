//! Permanent rotation, per format: what is written, that readers (ours, and where installed
//! exiftool, jpegtran and ImageMagick/libheif as independent oracles) see the new orientation,
//! that nothing but the orientation changed, and that undo restores the file byte for byte.
//!
//! Fixtures come from tests/fixtures/make.sh. External tools are optional: a test that can't
//! find one says so and checks only what our own readers can.

use std::path::{Path, PathBuf};
use std::process::Command;

use specular_fileops::history;
use specular_fileops::orient::{self, Method, RotateOptions, TagReason, Transform};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("fileops-rotate")
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A copy of `fixture` in a fresh folder.
fn copy_of(fixture_name: &str, test: &str) -> PathBuf {
    let d = scratch(test);
    let p = d.join(fixture_name);
    std::fs::copy(fixture(fixture_name), &p).unwrap();
    p
}

const ALL: [Transform; 8] = [
    Transform::IDENTITY,
    Transform::ROTATE_90,
    Transform::ROTATE_180,
    Transform::ROTATE_270,
    Transform::FLIP_H,
    Transform::FLIP_V,
    Transform {
        transpose: true,
        flip_h: false,
        flip_v: false,
    },
    Transform {
        transpose: true,
        flip_h: true,
        flip_v: true,
    },
];

fn tool(name: &str) -> bool {
    let ok = Command::new(name)
        .arg(if name == "magick" { "-version" } else { "-ver" })
        .output()
        .is_ok();
    if !ok {
        eprintln!("{name} not installed: its oracle checks are skipped");
    }
    ok
}

/// Orientation as exiftool reads it (`None` when it reports none).
fn exiftool_orientation(p: &Path) -> Option<u8> {
    let out = Command::new("exiftool")
        .args(["-n", "-s3", "-Orientation"])
        .arg(p)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

/// exiftool's warnings about the file's structure.
fn exiftool_warnings(p: &Path) -> String {
    let out = Command::new("exiftool")
        .args(["-s3", "-Warning", "-a"])
        .arg(p)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// ImageMagick's decode (libheif applies `irot`/`imir`): width, height, RGBA8.
fn magick_rgba(p: &Path) -> Option<(u32, u32, Vec<u8>)> {
    let size = Command::new("magick")
        .arg(p)
        .args(["-format", "%w %h", "info:"])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&size.stdout);
    let mut it = s.split_whitespace().map(|v| v.parse::<u32>().ok());
    let (w, h) = (it.next()??, it.next()??);
    let px = Command::new("magick")
        .arg(p)
        .args(["-depth", "8", "rgba:-"])
        .output()
        .ok()?;
    (px.status.success() && px.stdout.len() == (w * h * 4) as usize).then_some((w, h, px.stdout))
}

/// `t` applied to an RGBA8 image.
fn apply(t: Transform, w: u32, h: u32, px: &[u8]) -> (u32, u32, Vec<u8>) {
    let (nw, nh) = if t.transpose { (h, w) } else { (w, h) };
    let mut out = vec![0u8; px.len()];
    for y in 0..h {
        for x in 0..w {
            let (mut u, mut v) = if t.transpose { (y, x) } else { (x, y) };
            if t.flip_h {
                u = nw - 1 - u;
            }
            if t.flip_v {
                v = nh - 1 - v;
            }
            let s = ((y * w + x) * 4) as usize;
            let d = ((v * nw + u) * 4) as usize;
            out[d..d + 4].copy_from_slice(&px[s..s + 4]);
        }
    }
    (nw, nh, out)
}

fn mismatches(a: &[u8], b: &[u8], tol: u8) -> usize {
    a.chunks(4)
        .zip(b.chunks(4))
        .filter(|(x, y)| x.iter().zip(y.iter()).any(|(p, q)| p.abs_diff(*q) > tol))
        .count()
}

fn opts() -> RotateOptions {
    RotateOptions::default()
}

/// Rotate, check the method and the recorded orientation, then undo and check the bytes.
fn rotate_and_undo(path: &Path, t: Transform, want: u8) -> orient::Rotated {
    let before = std::fs::read(path).unwrap();
    let r = orient::rotate(path, t, opts()).unwrap();
    assert_eq!(r.orientation, want, "{} {t:?}", path.display());
    let fmt = orient::detect(path, &std::fs::read(path).unwrap()).unwrap();
    let now = if fmt == orient::Format::Raw {
        orient::sidecar_orientation(path).unwrap()
    } else {
        orient::current(fmt, &std::fs::read(path).unwrap())
    };
    assert_eq!(now, want, "{} read back", path.display());
    struct NoTrash;
    impl specular_fileops::Trash for NoTrash {
        fn trash(&self, _: &Path) -> std::io::Result<()> {
            unreachable!()
        }
        fn restore(&self, _: &Path) -> std::io::Result<()> {
            unreachable!()
        }
    }
    let after = std::fs::read(path).unwrap();
    if !r.op.is_empty() {
        let back = history::undo(&r.op, &NoTrash).unwrap();
        assert_eq!(
            std::fs::read(path).unwrap(),
            before,
            "undo restores the bytes"
        );
        history::redo(&back, &NoTrash).unwrap();
        assert_eq!(
            std::fs::read(path).unwrap(),
            after,
            "redo writes them again"
        );
    }
    r
}

#[test]
fn tag_formats_every_orientation() {
    let exiftool = tool("exiftool");
    let magick = tool("magick");
    for name in [
        "plain.png",
        "odd.png",
        "lossy.webp",
        "lossless.webp",
        "little.tif",
        "big.tif",
    ] {
        for (k, &t) in ALL.iter().enumerate() {
            let p = copy_of(name, &format!("tag-{name}-{k}"));
            let orig = std::fs::read(&p).unwrap();
            let fmt = orient::detect(&p, &orig).unwrap();
            let old = orient::current(fmt, &orig);
            let want = Transform::from_exif(old).then(t).to_exif();
            let r = rotate_and_undo(&p, t, want);
            assert_eq!(r.method, Method::Tag { reason: None });
            if exiftool {
                let got = exiftool_orientation(&p).unwrap_or(1);
                assert_eq!(got, want, "exiftool reads {name} {t:?}");
                assert_eq!(
                    exiftool_warnings(&p),
                    "",
                    "exiftool warns about {name} {t:?}"
                );
            }
            if magick {
                // Same pixels: only metadata changed.
                let a = magick_rgba(&fixture(name)).unwrap();
                let b = magick_rgba(&p).expect("still decodes");
                assert_eq!((a.0, a.1), (b.0, b.1));
                assert_eq!(mismatches(&a.2, &b.2, 0), 0, "{name} pixels");
            }
        }
    }
}

#[test]
fn jpeg_lossless_matches_jpegtran_and_cycles() {
    let jpegtran = tool("jpegtran");
    let src = std::fs::read(fixture("aligned.jpg")).unwrap();
    let (_, _, orig_coefs) = specular_jpeg::read_coefficients(&src).unwrap();
    let mut coef_mismatches = 0usize;
    for (k, &t) in ALL.iter().enumerate().skip(1) {
        let p = copy_of("aligned.jpg", &format!("jpeg-{k}"));
        let r = rotate_and_undo(&p, t, 1);
        assert_eq!(r.method, Method::Lossless { trimmed: None }, "{t:?}");
        if jpegtran {
            let op: &[&str] = match k {
                1 => &["-rotate", "90"],
                2 => &["-rotate", "180"],
                3 => &["-rotate", "270"],
                4 => &["-flip", "horizontal"],
                5 => &["-flip", "vertical"],
                6 => &["-transpose"],
                _ => &["-transverse"],
            };
            let out = Command::new("jpegtran")
                .args(op)
                .arg(fixture("aligned.jpg"))
                .output()
                .unwrap();
            assert!(out.status.success());
            let (_, _, want) = specular_jpeg::read_coefficients(&out.stdout).unwrap();
            let (_, _, got) =
                specular_jpeg::read_coefficients(&std::fs::read(&p).unwrap()).unwrap();
            assert_eq!(want.len(), got.len());
            for (a, b) in want.iter().zip(&got) {
                assert_eq!((a.blocks_w, a.blocks_h), (b.blocks_w, b.blocks_h));
                coef_mismatches += a.coefs.iter().zip(&b.coefs).filter(|(x, y)| x != y).count();
            }
        }
    }
    assert_eq!(coef_mismatches, 0, "coefficients differing from jpegtran");
    eprintln!("jpegtran oracle: 7 transforms, {coef_mismatches} coefficient mismatches");

    // Four quarter turns: the coefficients come back exactly, and from then on every cycle
    // of four gives the same bytes.
    let p = copy_of("aligned.jpg", "jpeg-cycle");
    let turn = |n: usize| {
        for _ in 0..n {
            let r = orient::rotate(&p, Transform::ROTATE_90, opts()).unwrap();
            assert_eq!(r.method, Method::Lossless { trimmed: None });
        }
        std::fs::read(&p).unwrap()
    };
    let four = turn(4);
    let (_, _, back) = specular_jpeg::read_coefficients(&four).unwrap();
    for (a, b) in orig_coefs.iter().zip(&back) {
        assert_eq!(a.coefs, b.coefs, "coefficients after four turns");
    }
    let eight = turn(4);
    assert_eq!(four, eight, "a second cycle of four turns is bit-identical");
    // The Exif stays: camera model kept, orientation 1.
    if tool("exiftool") {
        assert_eq!(exiftool_orientation(&p), Some(1));
        assert_eq!(exiftool_warnings(&p), "");
    }
}

#[test]
fn jpeg_unaligned_tag_unless_trim_allowed() {
    // 27 x 19, 4:2:0, Exif orientation 6. A further quarter turn is 180 degrees in all,
    // which moves both partial edges.
    let p = copy_of("odd.jpg", "odd-tag");
    assert_eq!(
        orient::jpeg_plan(&p, Transform::ROTATE_90).unwrap(),
        Some(TagReason::NotAligned {
            width: 16,
            height: 16
        })
    );
    let r = rotate_and_undo(&p, Transform::ROTATE_90, 3);
    assert_eq!(
        r.method,
        Method::Tag {
            reason: Some(TagReason::NotAligned {
                width: 16,
                height: 16
            })
        }
    );
    if tool("exiftool") {
        assert_eq!(exiftool_orientation(&p), Some(3));
    }
    // With consent: trimmed and lossless.
    let p = copy_of("odd.jpg", "odd-trim");
    let r = orient::rotate(
        &p,
        Transform::ROTATE_90,
        RotateOptions {
            trim: true,
            ..opts()
        },
    )
    .unwrap();
    assert_eq!(
        r.method,
        Method::Lossless {
            trimmed: Some((16, 16))
        }
    );
    assert_eq!(r.orientation, 1);
    if tool("exiftool") {
        assert_eq!(exiftool_orientation(&p), Some(1));
    }
    // Undoing the current tag back to identity is pure metadata.
    let p = copy_of("odd.jpg", "odd-identity");
    let r = rotate_and_undo(&p, Transform::ROTATE_270, 1);
    assert_eq!(r.method, Method::Lossless { trimmed: None });
}

#[test]
fn jpeg_without_exif_and_keep_mtime() {
    let p = copy_of("noexif.jpg", "noexif");
    let old = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_500_000_000);
    specular_fileops::atomic::set_modified(&p, old).unwrap();
    let r = orient::rotate(
        &p,
        Transform::ROTATE_270,
        RotateOptions {
            keep_mtime: true,
            ..opts()
        },
    )
    .unwrap();
    assert_eq!(r.method, Method::Lossless { trimmed: None });
    assert_eq!(std::fs::metadata(&p).unwrap().modified().unwrap(), old);
    // A JPEG with a trailer gets the tag (its data is kept).
    let p = copy_of("noexif.jpg", "trailer");
    let mut d = std::fs::read(&p).unwrap();
    d.extend_from_slice(b"TRAILER DATA");
    std::fs::write(&p, &d).unwrap();
    let r = rotate_and_undo(&p, Transform::ROTATE_90, 6);
    assert_eq!(
        r.method,
        Method::Tag {
            reason: Some(TagReason::Trailer)
        }
    );
    assert!(std::fs::read(&p).unwrap().ends_with(b"TRAILER DATA"));
}

#[test]
fn heic_every_orientation_against_our_decoder_and_libheif() {
    let magick = tool("magick");
    let decode = |p: &Path| {
        let f = specular_heic::HeicFile::open(std::fs::read(p).unwrap()).unwrap();
        let img = f.decode(&Default::default()).unwrap();
        (img.width(), img.height(), img.to_rgba8().unwrap())
    };
    let (w, h, base) = decode(&fixture("plain.heic"));
    let lib_base = if magick {
        magick_rgba(&fixture("plain.heic"))
    } else {
        None
    };
    let mut ours = 0;
    let mut theirs = 0;
    for (k, &t) in ALL.iter().enumerate() {
        let p = copy_of("plain.heic", &format!("heic-{k}"));
        let r = rotate_and_undo(&p, t, t.to_exif());
        assert_eq!(r.method, Method::Tag { reason: None });
        let (gw, gh, got) = decode(&p);
        let (ww, wh, want) = apply(t, w, h, &base);
        assert_eq!((gw, gh), (ww, wh), "{t:?}");
        ours += mismatches(&got, &want, 0);
        if let Some((lw, lh, lb)) = &lib_base {
            let (gw, gh, got) = magick_rgba(&p).expect("libheif decodes it");
            let (ww, wh, want) = apply(t, *lw, *lh, lb);
            assert_eq!((gw, gh), (ww, wh), "libheif {t:?}");
            theirs += mismatches(&got, &want, 0);
        }
    }
    eprintln!("HEIC: {ours} pixel mismatches (our decoder), {theirs} (libheif)");
    assert_eq!(ours, 0);
    assert_eq!(theirs, 0);
    // Repeated turns reuse the same properties: the file stops growing.
    let p = copy_of("plain.heic", "heic-cycle");
    for _ in 0..4 {
        orient::rotate(&p, Transform::ROTATE_90, opts()).unwrap();
    }
    let n = std::fs::metadata(&p).unwrap().len();
    for _ in 0..4 {
        orient::rotate(&p, Transform::ROTATE_90, opts()).unwrap();
    }
    assert_eq!(std::fs::metadata(&p).unwrap().len(), n);
    assert_eq!(
        orient::heif::orientation(&std::fs::read(&p).unwrap()),
        Some(1)
    );
}

#[test]
fn avif_every_orientation() {
    let magick = tool("magick");
    let lib_base = if magick {
        magick_rgba(&fixture("plain.avif"))
    } else {
        None
    };
    let base = specular_avif::AvifFile::open(std::fs::read(fixture("plain.avif")).unwrap())
        .unwrap()
        .decode(1, false)
        .unwrap();
    let mut theirs = 0;
    for (k, &t) in ALL.iter().enumerate() {
        let p = copy_of("plain.avif", &format!("avif-{k}"));
        rotate_and_undo(&p, t, t.to_exif());
        let f = specular_avif::AvifFile::open(std::fs::read(&p).unwrap()).unwrap();
        assert_eq!(f.info().orientation, t.to_exif(), "{t:?}");
        // The coded image is untouched.
        assert_eq!(f.decode(1, false).unwrap().data, base.data);
        if let Some((lw, lh, lb)) = &lib_base {
            let (gw, gh, got) = magick_rgba(&p).expect("libheif decodes it");
            let (ww, wh, want) = apply(t, *lw, *lh, lb);
            assert_eq!((gw, gh), (ww, wh), "libheif {t:?}");
            theirs += mismatches(&got, &want, 0);
        }
    }
    eprintln!("AVIF: {theirs} pixel mismatches (libheif)");
    assert_eq!(theirs, 0);
}

#[test]
fn jxl_header_orientation_every_fixture() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../formats/specular-jxl/tests/data");
    let mut files = 0;
    // Header layouts seen: [neither, all_default, extra_fields, -].
    let mut kinds = [0usize; 4];
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "jxl") {
            continue;
        }
        let orig = std::fs::read(&p).unwrap();
        let Ok(f) = specular_jxl::JxlFile::open(orig.clone()) else {
            continue;
        };
        let Ok(base) = f.decode(1, false) else {
            continue;
        };
        files += 1;
        let cs_at = if orig.starts_with(&[0xFF, 0x0A]) {
            Some(0)
        } else {
            orig.windows(4).position(|w| w == b"jxlc").map(|i| i + 4)
        };
        if let Some(l) = cs_at.and_then(|i| specular_jxl::header_layout(&orig[i..])) {
            kinds[usize::from(l.extra_fields) * 2 + usize::from(l.all_default)] += 1;
        }
        for o in 1..=8u8 {
            let out = orient::jxl::set_orientation(&orig, o)
                .unwrap_or_else(|| panic!("{} to {o}", p.display()));
            assert_eq!(orient::jxl::orientation(&out), Some(o));
            let g = specular_jxl::JxlFile::open(out.clone()).unwrap();
            assert_eq!(g.info().orientation, u32::from(o), "{}", p.display());
            let d = g.decode(1, false).unwrap();
            assert_eq!(d.data, base.data, "{} pixels at {o}", p.display());
            // And back.
            let again = orient::jxl::set_orientation(&out, 1).unwrap();
            assert_eq!(
                specular_jxl::JxlFile::open(again)
                    .unwrap()
                    .info()
                    .orientation,
                1
            );
        }
    }
    assert!(files >= 10, "only {files} JPEG XL fixtures decoded");
    eprintln!(
        "JPEG XL: {files} fixtures, header layouts (plain, all-default, extra fields) {kinds:?}"
    );
}

#[test]
fn raw_goes_to_the_sidecar() {
    let d = scratch("raw");
    let raw = d.join("IMG_9.CR2");
    std::fs::write(&raw, b"not really a raw").unwrap();
    std::fs::write(d.join("IMG_9.JPG"), b"the pair").unwrap();
    let r = rotate_and_undo(&raw, Transform::ROTATE_90, 6);
    assert_eq!(r.method, Method::Sidecar);
    assert_eq!(r.written, d.join("IMG_9.xmp"));
    assert_eq!(std::fs::read(&raw).unwrap(), b"not really a raw");
    assert_eq!(orient::sidecar_orientation(&raw), Some(6));
    // A second turn reads the sidecar: 6 then 90 degrees is 180.
    let r = orient::rotate(&raw, Transform::ROTATE_90, opts()).unwrap();
    assert_eq!(r.orientation, 3);
    assert_eq!(orient::sidecar_orientation(&raw), Some(3));
    // An existing rating sidecar keeps its rating.
    let d = scratch("raw-rated");
    let raw = d.join("DSC_1.NEF");
    std::fs::write(&raw, b"raw").unwrap();
    let rated = "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\"><rdf:Description rdf:about=\"\" xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\" xmp:Rating=\"4\"/></rdf:RDF></x:xmpmeta>";
    std::fs::write(d.join("DSC_1.xmp"), rated).unwrap();
    let r = orient::rotate(
        &raw,
        Transform::ROTATE_270,
        RotateOptions {
            displayed: Some(Transform::from_exif(6)),
            ..opts()
        },
    )
    .unwrap();
    // The camera said 6; a quarter turn back is upright.
    assert_eq!(r.orientation, 1);
    let s = std::fs::read_to_string(d.join("DSC_1.xmp")).unwrap();
    assert!(s.contains("xmp:Rating=\"4\""));
    assert_eq!(orient::sidecar_orientation(&raw), Some(1));
}

#[test]
fn xmp_round_trip_through_tools() {
    // exiftool reads the sidecar we write.
    let d = scratch("xmp-tool");
    let raw = d.join("A.ARW");
    std::fs::write(&raw, b"raw").unwrap();
    orient::rotate(&raw, Transform::ROTATE_270, opts()).unwrap();
    if tool("exiftool") {
        assert_eq!(exiftool_orientation(&d.join("A.xmp")), Some(8));
    }
    // A sidecar exiftool wrote, read by us and updated in place.
    if tool("exiftool") {
        let x = d.join("B.xmp");
        let st = Command::new("exiftool")
            .args([
                "-q",
                "-n",
                "-XMP-tiff:Orientation=5",
                "-XMP-xmp:Rating=2",
                "-o",
            ])
            .arg(&x)
            .arg(d.join("A.xmp"))
            .status()
            .unwrap();
        assert!(st.success());
        let b = d.join("B.ARW");
        std::fs::write(&b, b"raw").unwrap();
        assert_eq!(orient::sidecar_orientation(&b), Some(5));
        let r = orient::rotate(&b, Transform::ROTATE_90, opts()).unwrap();
        let want = Transform::from_exif(5).then(Transform::ROTATE_90).to_exif();
        assert_eq!(r.orientation, want);
        assert_eq!(exiftool_orientation(&x), Some(want));
        let rating = Command::new("exiftool")
            .args(["-s3", "-n", "-Rating"])
            .arg(&x)
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&rating.stdout).trim(), "2");
    }
}

#[test]
fn unsupported_and_detection() {
    let d = scratch("unsupported");
    let p = d.join("a.gif");
    std::fs::write(&p, b"GIF89a....").unwrap();
    assert_eq!(
        orient::rotate(&p, Transform::ROTATE_90, opts()).unwrap_err(),
        orient::RotateError::Unsupported
    );
    // A DNG named .tif is still a RAW (DNGVersion in IFD0).
    let mut dng = b"II*\0\x08\0\0\0\x01\0\x12\xc6\x01\0\x04\0\0\0\x01\x04\0\0\0\0\0\0".to_vec();
    dng.resize(64, 0);
    assert_eq!(
        orient::detect(Path::new("x.tif"), &dng),
        Some(orient::Format::Raw)
    );
}
