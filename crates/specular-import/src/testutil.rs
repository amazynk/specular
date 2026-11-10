//! Simulated card contents for tests: small JPEGs and TIFF "RAW" files with Exif, and MP4
//! headers, under the test temp folder (never a real device).

use std::path::{Path, PathBuf};

/// A TIFF (little-endian) whose IFD0 has Make and Model and whose Exif IFD has
/// DateTimeOriginal.
pub fn tiff(make: &str, model: &str, date: &str) -> Vec<u8> {
    let ascii = |s: &str| {
        let mut v = s.as_bytes().to_vec();
        v.push(0);
        v
    };
    let (make, model, date) = (ascii(make), ascii(model), ascii(date));
    // Header (8) + IFD0 (2 + 3·12 + 4 = 42) + Exif IFD (2 + 12 + 4 = 18) + data.
    let ifd0 = 8u32;
    let exif = ifd0 + 42;
    let data = exif + 18;
    let mut b = b"II*\0".to_vec();
    b.extend(ifd0.to_le_bytes());
    let entry = |b: &mut Vec<u8>, tag: u16, ty: u16, n: u32, v: u32| {
        b.extend(tag.to_le_bytes());
        b.extend(ty.to_le_bytes());
        b.extend(n.to_le_bytes());
        b.extend(v.to_le_bytes());
    };
    b.extend(3u16.to_le_bytes());
    entry(&mut b, 0x010F, 2, make.len() as u32, data);
    entry(
        &mut b,
        0x0110,
        2,
        model.len() as u32,
        data + make.len() as u32,
    );
    entry(&mut b, 0x8769, 4, 1, exif);
    b.extend(0u32.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    entry(
        &mut b,
        0x9003,
        2,
        date.len() as u32,
        data + (make.len() + model.len()) as u32,
    );
    b.extend(0u32.to_le_bytes());
    assert_eq!(b.len() as u32, data);
    b.extend(make);
    b.extend(model);
    b.extend(date);
    b
}

/// Deterministic filler bytes.
pub fn filler(n: usize, seed: u64) -> Vec<u8> {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect()
}

/// A JPEG with an Exif APP1 and `kb` KiB of comment segments filled from `seed`.
pub fn exif_jpeg_seeded(make: &str, model: &str, date: &str, kb: usize, seed: u64) -> Vec<u8> {
    let t = tiff(make, model, date);
    let mut j = vec![0xFF, 0xD8, 0xFF, 0xE1];
    j.extend(((t.len() + 8) as u16).to_be_bytes());
    j.extend(b"Exif\0\0");
    j.extend(&t);
    let fill = filler(kb * 1024, seed);
    for chunk in fill.chunks(60_000) {
        j.extend([0xFF, 0xFE]);
        j.extend(((chunk.len() + 2) as u16).to_be_bytes());
        j.extend(chunk);
    }
    j.extend([0xFF, 0xD9]);
    j
}

pub fn exif_jpeg(make: &str, model: &str, date: &str, kb: usize) -> Vec<u8> {
    exif_jpeg_seeded(make, model, date, kb, 1)
}

/// A TIFF-structured "RAW" file: the Exif TIFF followed by filler.
pub fn raw(make: &str, model: &str, date: &str, kb: usize, seed: u64) -> Vec<u8> {
    let mut t = tiff(make, model, date);
    t.extend(filler(kb * 1024, seed));
    t
}

/// An MP4 with `ftyp`, `moov/mvhd` (created at `unix` seconds, 0 for unset) and `kb` KiB of
/// `mdat`.
pub fn mp4(unix: u64, kb: usize) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend(16u32.to_be_bytes());
    b.extend(b"ftypisom\0\0\0\0");
    let mdat = filler(kb * 1024, unix | 7);
    b.extend(((mdat.len() + 8) as u32).to_be_bytes());
    b.extend(b"mdat");
    b.extend(mdat);
    let secs = if unix == 0 { 0 } else { unix + 2_082_844_800 } as u32;
    let mut mvhd = vec![0u8; 4];
    mvhd.extend(secs.to_be_bytes());
    mvhd.extend(secs.to_be_bytes());
    mvhd.extend([0u8; 92]);
    b.extend(((mvhd.len() + 16) as u32).to_be_bytes());
    b.extend(b"moov");
    b.extend(((mvhd.len() + 8) as u32).to_be_bytes());
    b.extend(b"mvhd");
    b.extend(mvhd);
    b
}

/// A fresh, empty folder for one test under the temp folder.
pub fn fresh(name: &str) -> PathBuf {
    let d = std::env::temp_dir()
        .join("specular-import-tests")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

pub fn write(p: &Path, b: &[u8]) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, b).unwrap();
}

/// A simulated card: `DCIM/100CANON` with a RAW+JPEG pair (and the RAW's XMP), a JPEG alone,
/// and a video with its THM and LRV; `DCIM/101CANON` with another JPEG. Returns the card root.
pub fn card(root: &Path) -> PathBuf {
    let card = root.join("EOS_DIGITAL");
    let d = card.join("DCIM/100CANON");
    write(
        &d.join("IMG_0001.JPG"),
        &exif_jpeg_seeded("Canon", "Canon EOS R5", "2024:03:09 14:05:59", 80, 1),
    );
    write(
        &d.join("IMG_0001.CR2"),
        &raw("Canon", "Canon EOS R5", "2024:03:09 14:05:59", 150, 2),
    );
    write(&d.join("IMG_0001.CR2.xmp"), b"<x:xmpmeta/>");
    write(
        &d.join("IMG_0002.JPG"),
        &exif_jpeg_seeded("Canon", "Canon EOS R5", "2024:03:10 09:00:00", 20, 3),
    );
    write(&d.join("MVI_0003.MP4"), &mp4(1_710_061_200, 40));
    write(&d.join("MVI_0003.THM"), b"thumb");
    write(&d.join("MVI_0003.LRV"), &filler(3000, 4));
    write(
        &card.join("DCIM/101CANON/IMG_0101.JPG"),
        &exif_jpeg_seeded("Canon", "Canon EOS R5", "2024:03:10 10:00:00", 5, 5),
    );
    // Not media: ignored.
    write(&card.join("DCIM/100CANON/NOTES.TXT"), b"hi");
    write(&card.join("MISC/AUTPRINT.MRK"), b"x");
    card
}
