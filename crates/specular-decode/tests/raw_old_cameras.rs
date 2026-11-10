//! Old and rare camera RAW containers through the registry: Phase One EIP packages (the IIQ
//! inside a ZIP), the plain-language refusal of Rawzor, and the
//! corpus of `tools/corpus/fetch-raw400.sh` (ignored test).

#[path = "../../formats/specular-raw/tests/common/mod.rs"]
mod common;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::{DngCodec, DngSpec, write_dng};
use specular_core::{CancelToken, Source};
use specular_decode::{DecodeError, DecodeRequest, Registry};

fn zip_of(name: &str, bytes: &[u8], method: zip::CompressionMethod) -> Vec<u8> {
    let mut out = Vec::new();
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut out));
    let opts = zip::write::SimpleFileOptions::default().compression_method(method);
    w.start_file("settings/CaptureOne.cos", opts).unwrap();
    w.write_all(b"<settings/>").unwrap();
    w.start_file(name, opts).unwrap();
    w.write_all(bytes).unwrap();
    w.finish().unwrap();
    out
}

#[test]
fn eip_opens_the_raw_inside() {
    let s = DngSpec::new(32, 32, DngCodec::U16);
    let raw = write_dng(&s, &vec![6000u16; 32 * 32]);
    for method in [
        zip::CompressionMethod::Stored,
        zip::CompressionMethod::Deflated,
    ] {
        let eip = zip_of("Capture/IMG_0001.IIQ", &raw, method);
        let sess = Registry::default()
            .open(Arc::new(Source::from_vec("shot.eip", eip)))
            .unwrap();
        assert_eq!((sess.info().width, sess.info().height), (32, 32));
        let r = sess
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert_eq!(r.width, 32);
    }
    // A ZIP without an IIQ says what is missing.
    let eip = zip_of("notes.txt", b"hello", zip::CompressionMethod::Stored);
    let e = Registry::default()
        .open(Arc::new(Source::from_vec("shot.eip", eip)))
        .err()
        .unwrap();
    assert!(e.to_string().contains("no IIQ"), "{e}");
}

#[test]
fn unpublished_codecs_are_refused_by_name() {
    // Canon Cinema RAW Light is read now (`tests/raw_clips.rs`).
    // Rawzor by its signature; a `.rwz` without one is an Outlook rules file.
    let mut rawzor = b"rawzor\x01\x00\x01\x00".to_vec();
    rawzor.resize(64, 0);
    let mut outlook = vec![0u8; 64];
    outlook[..4].copy_from_slice(&[0xd0, 0xcf, 0x11, 0xe0]);
    for (name, what, head) in [
        ("photo.rwz", "Rawzor compression", Some(&rawzor)),
        ("PHOTO.RWZ", "Rawzor compression", Some(&rawzor)),
        ("rules.rwz", "Outlook", Some(&outlook)),
    ] {
        let mut bytes = vec![0u8; 64];
        bytes[4..12].copy_from_slice(b"ftypcrx ");
        if let Some(h) = head {
            bytes.clone_from(h);
        }
        let e = Registry::default()
            .open(Arc::new(Source::from_vec(name, bytes)))
            .err()
            .unwrap();
        assert!(
            matches!(&e, DecodeError::Unsupported(m) if m.contains(what)),
            "{name}: {e}"
        );
    }
}

#[test]
fn headerless_raw_needs_its_size() {
    // A `.raw` of no known camera size stays with the other `.raw` readers.
    let e = Registry::default().open(Arc::new(Source::from_vec("dump.raw", vec![7u8; 4096])));
    assert!(e.is_err() || e.unwrap().info().format != specular_decode::Format::Raw);
}

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_RAW400_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/raw400"));
    dir.is_dir().then_some(dir)
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if p.is_dir() {
            if name != "oracle" {
                files(&p, out);
            }
        } else if !name.starts_with('.')
            && !name.ends_with(".zip")
            && !name.ends_with(".txt")
            && !name.ends_with(".unpacked")
        {
            out.push(p);
        }
    }
    out.sort();
}

/// Pillow's RGBA8 of a FlashPix file (`tools/corpus/fpx-oracle.py`).
fn pillow_oracle(dir: &Path, rel: &str) -> Option<(usize, usize, Vec<u8>)> {
    let base = dir.join("oracle").join(rel.replace('/', "__"));
    let name = base.file_name()?.to_str()?.to_string();
    let txt = std::fs::read_to_string(base.with_file_name(format!("{name}.rgba.txt"))).ok()?;
    let mut it = txt.split_whitespace();
    let w = it.next()?.parse().ok()?;
    let h = it.next()?.parse().ok()?;
    Some((
        w,
        h,
        std::fs::read(base.with_file_name(format!("{name}.rgba"))).ok()?,
    ))
}

/// A decoded raster as tightly packed RGBA8 (8-bit grey and RGBA layouts).
fn rgba_of(r: &specular_core::Raster) -> Vec<u8> {
    let p = &r.planes[0];
    let (w, h) = (r.width as usize, r.height as usize);
    let bpp = (p.stride / w.max(1)).clamp(1, 4);
    let mut out = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let s = &p.data[y * p.stride + x * bpp..][..bpp];
            if bpp == 4 {
                out.extend_from_slice(s);
            } else {
                out.extend_from_slice(&[s[0], s[0], s[0], 255]);
            }
        }
    }
    out
}

/// Every corpus file through the registry: what opens it, the coding line, a 1/4 decode and
/// its time. Fails when a file of a format this lane added does not open.
#[test]
#[ignore = "needs tools/corpus/fetch-raw400.sh"]
fn corpus_opens() {
    let Some(dir) = corpus() else {
        eprintln!("no corpus");
        return;
    };
    let mut all = Vec::new();
    files(&dir, &mut all);
    let reg = Registry::default();
    let mut failed = Vec::new();
    for p in all {
        let rel = p.strip_prefix(&dir).unwrap().display().to_string();
        let t = std::time::Instant::now();
        let r = Source::open(&p)
            .map_err(DecodeError::Io)
            .and_then(|s| reg.open(Arc::new(s)))
            .and_then(|s| {
                let r = s.decode(&DecodeRequest::full(4), &CancelToken::new())?;
                Ok((s, r))
            });
        let ms = t.elapsed().as_secs_f64() * 1e3;
        if let Some((ow, oh, ov)) = pillow_oracle(&dir, &rel) {
            let s = Source::open(&p)
                .map_err(DecodeError::Io)
                .and_then(|s| reg.open(Arc::new(s)))
                .unwrap();
            let full = s
                .decode(&DecodeRequest::full(1), &CancelToken::new())
                .unwrap();
            let px = rgba_of(&full);
            assert_eq!(
                (full.width as usize, full.height as usize),
                (ow, oh),
                "{rel}"
            );
            let (mut differ, mut max) = (0usize, 0u8);
            for (a, b) in px.iter().zip(&ov) {
                let e = a.abs_diff(*b);
                differ += (e > 0) as usize;
                max = max.max(e);
            }
            eprintln!(
                "{rel:58.58} Pillow: {differ} of {} samples differ, max {max}",
                ov.len()
            );
            if max > 3 {
                failed.push(format!("{rel}: Pillow max difference {max}"));
            }
        }
        match r {
            Ok((s, r)) => eprintln!(
                "{rel:58.58} {:?} {}x{} 1/4 {}x{} {ms:.0} ms: {}",
                s.info().format,
                s.info().width,
                s.info().height,
                r.width,
                r.height,
                s.info().coding
            ),
            Err(e) => {
                eprintln!("{rel:58.58} error: {e}");
                let ext = p
                    .extension()
                    .map(|e| e.to_string_lossy().to_ascii_lowercase())
                    .unwrap_or_default();
                if ["mdc", "k25", "sti", "ori", "j6i", "lfr", "lfp"].contains(&ext.as_str())
                    || rel.contains("QuickTake") && !rel.contains("ME & LESLIE")
                {
                    failed.push(rel);
                }
            }
        }
    }
    assert!(failed.is_empty(), "{failed:?}");
}
