//! Thumbnails of HDF5, NetCDF and EMD arrays: big ones from every n-th sample of the first
//! plane (only the chunks holding samples are read), small ones decoded whole; and how long
//! typical files take.

use std::sync::Arc;
use std::time::{Duration, Instant};

use specular_core::{CancelToken, Source};
use specular_decode::{DecodeRequest, Registry, Session};
use specular_hdf5::writer::{AttrValue, ROOT, Storage, Ty, Writer};
use specular_thumbs::generate::{GenOptions, generate};
use specular_thumbs::pixels;

fn zlib(raw: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    e.write_all(raw).unwrap();
    e.finish().unwrap()
}

/// Deflated row-major chunks of a 2D row-major array of `esize`-byte elements.
fn chunks(w: u64, h: u64, cw: u64, ch: u64, esize: usize, raw: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for gy in 0..h.div_ceil(ch) {
        for gx in 0..w.div_ceil(cw) {
            let mut c = vec![0u8; (cw * ch) as usize * esize];
            for y in 0..ch.min(h - gy * ch) {
                let row = ((gy * ch + y) * w + gx * cw) as usize * esize;
                let n = cw.min(w - gx * cw) as usize * esize;
                let at = (y * cw) as usize * esize;
                c[at..at + n].copy_from_slice(&raw[row..row + n]);
            }
            out.push(zlib(&c));
        }
    }
    out
}

/// A smooth field with texture, as f32 or u16.
fn field(w: u64, h: u64) -> Vec<f32> {
    (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as f32, (i / w) as f32);
            (x * 0.004).sin() * 900.0 + (y * 0.006).cos() * 700.0 + ((i * 7919) % 211) as f32
        })
        .collect()
}

fn bytes_f32(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn bytes_u16(v: &[f32]) -> Vec<u8> {
    v.iter()
        .map(|x| ((x + 2000.0) * 10.0) as u16)
        .flat_map(u16::to_le_bytes)
        .collect()
}

/// An HDF5 file holding one 2D dataset, contiguous or in deflated chunks.
fn h5(w: u64, h: u64, ty: Ty, raw: Vec<u8>, chunk: Option<(u64, u64)>) -> Vec<u8> {
    let mut wr = Writer::new();
    let storage = match chunk {
        None => Storage::Contiguous,
        Some((cw, ch)) => Storage::Chunks {
            chunk: vec![ch, cw],
            filters: vec![(1, vec![1])],
            stored: chunks(w, h, cw, ch, ty.size(), &raw),
        },
    };
    wr.dataset(ROOT, "data", ty, &[h, w], raw, storage);
    wr.finish()
}

/// A Berkeley EMD file with one image in deflated chunks.
fn emd(w: u64, h: u64, raw: Vec<u8>) -> Vec<u8> {
    let mut wr = Writer::new();
    wr.attr(ROOT, "version_major", AttrValue::I32(vec![0]));
    wr.attr(ROOT, "version_minor", AttrValue::I32(vec![2]));
    let g = wr.groups("/data/image");
    wr.attr(g, "emd_group_type", AttrValue::I32(vec![1]));
    wr.dataset(
        g,
        "data",
        Ty::U16,
        &[h, w],
        raw.clone(),
        Storage::Chunks {
            chunk: vec![256, 256],
            filters: vec![(1, vec![1])],
            stored: chunks(w, h, 256, 256, 2, &raw),
        },
    );
    wr.finish()
}

/// A classic (CDF-1) NetCDF file with `t(time, lat, lon)` as big-endian f32.
fn netcdf(w: u32, h: u32, times: u32) -> Vec<u8> {
    fn name(b: &mut Vec<u8>, s: &str) {
        b.extend((s.len() as u32).to_be_bytes());
        b.extend(s.as_bytes());
        b.resize(b.len().next_multiple_of(4), 0);
    }
    let mut b = b"CDF\x01".to_vec();
    b.extend(0u32.to_be_bytes());
    b.extend(10u32.to_be_bytes());
    b.extend(3u32.to_be_bytes());
    for (n, l) in [("time", times), ("lat", h), ("lon", w)] {
        name(&mut b, n);
        b.extend(l.to_be_bytes());
    }
    b.extend([0; 8]);
    b.extend(11u32.to_be_bytes());
    b.extend(1u32.to_be_bytes());
    name(&mut b, "t");
    b.extend(3u32.to_be_bytes());
    for d in 0..3u32 {
        b.extend(d.to_be_bytes());
    }
    b.extend([0; 8]);
    b.extend(5u32.to_be_bytes());
    let size = u64::from(times) * u64::from(w) * u64::from(h) * 4;
    b.extend((size as u32).to_be_bytes());
    let begin = b.len() as u32 + 4;
    b.extend(begin.to_be_bytes());
    for _ in 0..times {
        for v in field(u64::from(w), u64::from(h)) {
            b.extend(v.to_be_bytes());
        }
    }
    b
}

fn open(name: &str, data: Vec<u8>) -> Arc<dyn Session> {
    Registry::new()
        .open(Arc::new(Source::from_vec(name, data)))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// A big array's thumbnail is the sampled rendition, filtered to the class; a small one's
/// is the usual full decode.
#[test]
fn big_arrays_are_sampled_small_ones_decoded() {
    let c = CancelToken::new();
    let (w, h) = (2048u64, 1536u64);
    let raw = bytes_f32(&field(w, h));
    for (name, data) in [
        ("big.h5", h5(w, h, Ty::F32, raw.clone(), Some((256, 256)))),
        ("big.emd", emd(w, h, bytes_u16(&field(w, h)))),
        ("big.nc", netcdf(2048, 1536, 1)),
    ] {
        let s = open(name, data);
        let px = generate(&*s, 256, GenOptions::default(), &c, &mut |_| {}).unwrap();
        assert_eq!((px.width, px.height), (256, 192), "{name}");
        let r = s.decode_sampled(256, &c).unwrap().unwrap();
        assert_eq!((r.width, r.height), (512, 384), "{name}");
        let want = pixels::thumbnail(&r, s.info().orientation, 256);
        assert_eq!(px.rgba, want.rgba, "{name}");
    }
    let (w, h) = (400u64, 300u64);
    let s = open(
        "small.h5",
        h5(w, h, Ty::F32, bytes_f32(&field(w, h)), Some((64, 64))),
    );
    assert!(s.decode_sampled(256, &c).is_none());
    let px = generate(&*s, 256, GenOptions::default(), &c, &mut |_| {}).unwrap();
    let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
    assert_eq!(
        px.rgba,
        pixels::thumbnail(&r, s.info().orientation, 256).rgba
    );
}

/// Planetary images: a big VICAR frame and a big three-band PDS3 product are sampled (every
/// n-th line and sample straight from the file), a small one is decoded; fpack files from the
/// corpus (when fetched) thumbnail through the FITS reader.
#[test]
fn planetary_images_thumbnail() {
    use specular_planetary::{pds3, vicar};
    let c = CancelToken::new();
    let (w, h) = (2048usize, 1536usize);
    let values: Vec<f64> = field(w as u64, h as u64)
        .iter()
        .map(|&v| f64::from(v))
        .collect();
    let big_vicar = vicar::write("HALF", "BSQ", "LOW", "RIEEE", (w, h, 1), 0, 0, "", &values);
    let mut three = values.clone();
    three.extend(values.iter().map(|v| v * 0.5));
    three.extend(values.iter().map(|v| v * 0.25));
    let big_pds = pds3::write_attached((w, h, 3), "LSB_INTEGER", 16, "BAND_SEQUENTIAL", "", &three);
    for (name, data) in [("big.vic", big_vicar), ("big.img", big_pds)] {
        let s = open(name, data);
        let px = generate(&*s, 256, GenOptions::default(), &c, &mut |_| {}).unwrap();
        assert_eq!((px.width, px.height), (256, 192), "{name}");
        let r = s.decode_sampled(256, &c).unwrap().unwrap();
        assert_eq!((r.width, r.height), (512, 384), "{name}");
    }
    let small = vicar::write(
        "BYTE",
        "BSQ",
        "LOW",
        "RIEEE",
        (40, 30, 1),
        0,
        0,
        "",
        &vec![9.0; 1200],
    );
    let s = open("small.vic", small);
    assert!(s.decode_sampled(256, &c).is_none());
    let px = generate(&*s, 256, GenOptions::default(), &c, &mut |_| {}).unwrap();
    assert_eq!((px.width, px.height), (40, 30));
    let corpus = std::env::var_os("SPECULAR_SPACE_CORPUS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/space")
        })
        .join("fpack");
    if let Ok(rd) = std::fs::read_dir(&corpus) {
        let mut n = 0;
        for e in rd.flatten() {
            let s = Registry::new().open_path(&e.path()).unwrap();
            assert_eq!(s.info().format, specular_decode::Format::Fpack);
            let px = generate(&*s, 128, GenOptions::default(), &c, &mut |_| {}).unwrap();
            assert!(px.width <= 128 && px.height <= 128 && px.width.max(px.height) == 128);
            n += 1;
        }
        eprintln!("{n} fpack thumbnails");
    }
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let t = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &t);
        } else {
            std::fs::copy(e.path(), t).unwrap();
        }
    }
}

/// A CASA image folder is an image tile (listed at any depth, never descended into), keyed
/// by its entries' stamps, with a thumbnail of its first plane; plain folders stay folders.
#[test]
fn casa_folders_are_image_tiles() {
    use specular_thumbs::engine::{FileOpener, Opener};
    use specular_thumbs::key::ThumbKey;
    use specular_thumbs::list::{Entry, EntryKind, ListOptions, list_dir};
    let root = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("casa-gallery-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../formats/specular-casa/tests/data/cube_le.image"
    );
    copy_dir(std::path::Path::new(fixture), &root.join("cube.image"));
    copy_dir(
        std::path::Path::new(fixture),
        &root.join("nested/deeper.image"),
    );
    std::fs::create_dir_all(root.join("plain")).unwrap();
    let reg = Registry::new();
    let classify = |p: &std::path::Path| {
        if reg.supports_file(p) {
            EntryKind::Image
        } else {
            EntryKind::Other
        }
    };
    let list = |depth| {
        let mut all: Vec<Entry> = Vec::new();
        let opts = ListOptions {
            depth,
            ..ListOptions::default()
        };
        list_dir(&root, opts, &classify, &CancelToken::new(), &mut |c| {
            all.extend(c)
        })
        .unwrap();
        all.sort_by(|a, b| a.name.cmp(&b.name));
        all.into_iter()
            .map(|e| (e.name, e.kind, e.len))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        list(0),
        [
            ("cube.image".to_owned(), EntryKind::Image, 0),
            ("nested".to_owned(), EntryKind::Folder, 0),
            ("plain".to_owned(), EntryKind::Folder, 0),
        ]
    );
    // Recursive: the images, none of the files inside them.
    assert_eq!(
        list(3),
        [
            ("cube.image".to_owned(), EntryKind::Image, 0),
            ("nested/deeper.image".to_owned(), EntryKind::Image, 0),
        ]
    );
    let dir = root.join("cube.image");
    let k = FileOpener.key(&dir).unwrap();
    assert_eq!(k, ThumbKey::of_folder(&dir).unwrap());
    assert!(FileOpener.key(&root.join("plain")).is_err());
    let s = Registry::new()
        .open(FileOpener.open(&dir).unwrap().source)
        .unwrap();
    let c = CancelToken::new();
    let px = generate(&*s, 64, GenOptions::default(), &c, &mut |_| {}).unwrap();
    let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
    assert_eq!((r.width, r.height), (13, 11));
    assert_eq!(
        px.rgba,
        pixels::thumbnail(&r, s.info().orientation, 64).rgba
    );
    // Rewriting a file of the image gives it a new key.
    let f = dir.join("table.f0_TSM0");
    let bytes = std::fs::read(&f).unwrap();
    std::fs::write(&f, &bytes[..bytes.len() - 1]).unwrap();
    std::fs::write(&f, &bytes).unwrap();
    let t = std::fs::File::options().write(true).open(&f).unwrap();
    t.set_modified(std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000))
        .unwrap();
    assert_ne!(FileOpener.key(&dir).unwrap().content, k.content);
    std::fs::remove_dir_all(&root).unwrap();
}

/// Wall-clock time of a 256 thumbnail of typical files (median of five, after a first run
/// that also builds the chunk index). Prints the table; run with `--ignored --nocapture`.
#[test]
#[ignore = "timing; run by hand on an idle machine"]
fn timings() {
    let mut rows: Vec<(&str, Vec<u8>)> = Vec::new();
    let f2k = field(2048, 2048);
    rows.push((
        "HDF5 2048² f32, deflate 256² chunks",
        h5(2048, 2048, Ty::F32, bytes_f32(&f2k), Some((256, 256))),
    ));
    let f4k = field(4096, 4096);
    rows.push((
        "HDF5 4096² f32, deflate 512² chunks",
        h5(4096, 4096, Ty::F32, bytes_f32(&f4k), Some((512, 512))),
    ));
    rows.push((
        "HDF5 4096² f32, contiguous",
        h5(4096, 4096, Ty::F32, bytes_f32(&f4k), None),
    ));
    rows.push((
        "HDF5 4096² u16, deflate 64² chunks",
        h5(4096, 4096, Ty::U16, bytes_u16(&f4k), Some((64, 64))),
    ));
    let f8k = field(8192, 8192);
    rows.push((
        "HDF5 8192² u16, deflate 256² chunks",
        h5(8192, 8192, Ty::U16, bytes_u16(&f8k), Some((256, 256))),
    ));
    rows.push((
        "EMD 4096² u16, deflate 256² chunks",
        emd(4096, 4096, bytes_u16(&f4k)),
    ));
    rows.push(("NetCDF classic 1440×721×4 f32", netcdf(1440, 721, 4)));
    rows.push(("NetCDF classic 7200×3600 f32", netcdf(7200, 3600, 1)));
    let c = CancelToken::new();
    let mut worst = Duration::ZERO;
    for (label, data) in rows {
        let ext = if label.starts_with("EMD") {
            "x.emd"
        } else if label.starts_with("NetCDF") {
            "x.nc"
        } else {
            "x.h5"
        };
        let mut times = Vec::new();
        for _ in 0..6 {
            let t = Instant::now();
            let s = open(ext, data.clone());
            generate(&*s, 256, GenOptions::default(), &c, &mut |_| {}).unwrap();
            times.push(t.elapsed());
        }
        let first = times.remove(0);
        times.sort();
        let median = times[times.len() / 2];
        worst = worst.max(median);
        println!(
            "{label:40} {:>6.1} ms (first {:>6.1} ms)",
            median.as_secs_f64() * 1e3,
            first.as_secs_f64() * 1e3
        );
    }
    assert!(worst < Duration::from_secs(2), "{worst:?}");
}

/// A folder image's key follows files in its subfolders too (a CASA subtable, a Zarr chunk),
/// not only its top level.
#[test]
fn folder_key_sees_nested_changes() {
    use specular_thumbs::ThumbKey;
    let root = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("folder-key-nested");
    let _ = std::fs::remove_dir_all(&root);
    let chunk = root.join("0").join("0").join("0");
    std::fs::create_dir_all(&chunk).unwrap();
    std::fs::write(root.join(".zarray"), b"{}").unwrap();
    std::fs::write(chunk.join("0"), b"abcd").unwrap();
    let k = ThumbKey::of_folder(&root).unwrap();
    assert_eq!(k, ThumbKey::of_folder(&root).unwrap(), "stable");
    // Rewritten in place: same size, a new time.
    let f = std::fs::File::options()
        .write(true)
        .open(chunk.join("0"))
        .unwrap();
    f.set_modified(std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(2_000_000))
        .unwrap();
    let k2 = ThumbKey::of_folder(&root).unwrap();
    assert_ne!(k2.content, k.content);
    // A new file deep down.
    std::fs::write(chunk.join("1"), b"efgh").unwrap();
    assert_ne!(ThumbKey::of_folder(&root).unwrap().content, k2.content);
    std::fs::remove_dir_all(&root).unwrap();
}
