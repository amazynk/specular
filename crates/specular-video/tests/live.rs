//! Live Photo pairing on disk: the sibling movie of a still, its `moov` read by seeking, and the
//! content identifier check.

use std::path::PathBuf;

fn dir(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("live-{name}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A minimal QuickTime movie: `ftyp`, a large `mdat` before the `moov` (so finding the
/// `moov` needs the seek walk), the content identifier in `moov/meta`.
fn mov(content_id: Option<&str>) -> Vec<u8> {
    fn bx(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend(kind);
        v.extend(body);
        v
    }
    fn full(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut b = vec![0u8; 4];
        b.extend(body);
        bx(kind, &b)
    }
    let mut out = bx(b"ftyp", b"qt  \0\0\0\0qt  ");
    out.extend(bx(b"mdat", &vec![0u8; 100_000]));
    // mvhd v0: timescale 600, duration 1800 (3 s).
    let mut mvhd = vec![0u8; 96];
    mvhd[8..12].copy_from_slice(&600u32.to_be_bytes());
    mvhd[12..16].copy_from_slice(&1800u32.to_be_bytes());
    // A video trak with an hvc1 sample entry and no samples.
    let mut tkhd = vec![0u8; 80];
    // The display matrix {0, 1, 0; -1, 0, 0; 0, 0, 1}: 90° clockwise.
    tkhd[40..44].copy_from_slice(&0x0001_0000u32.to_be_bytes());
    tkhd[48..52].copy_from_slice(&0xffff_0000u32.to_be_bytes());
    tkhd[68..72].copy_from_slice(&0x4000_0000u32.to_be_bytes());
    tkhd[72..76].copy_from_slice(&(1920u32 << 16).to_be_bytes());
    tkhd[76..80].copy_from_slice(&(1440u32 << 16).to_be_bytes());
    let mut mdhd = vec![0u8; 20];
    mdhd[8..12].copy_from_slice(&600u32.to_be_bytes());
    mdhd[12..16].copy_from_slice(&1800u32.to_be_bytes());
    let mut hdlr = vec![0u8; 20];
    hdlr[4..8].copy_from_slice(b"vide");
    let mut entry = vec![0u8; 78];
    entry[6..8].copy_from_slice(&1u16.to_be_bytes());
    entry[24..26].copy_from_slice(&1920u16.to_be_bytes());
    entry[26..28].copy_from_slice(&1440u16.to_be_bytes());
    let mut hvcc = vec![0u8; 23];
    hvcc[0] = 1;
    entry.extend(bx(b"hvcC", &hvcc));
    let mut stsd = 1u32.to_be_bytes().to_vec();
    stsd.extend(bx(b"hvc1", &entry));
    let stbl = [
        full(b"stsd", &stsd),
        full(b"stts", &0u32.to_be_bytes()),
        full(b"stsc", &0u32.to_be_bytes()),
        full(b"stsz", &[0u8; 8]),
        full(b"stco", &0u32.to_be_bytes()),
    ]
    .concat();
    let minf = bx(b"minf", &bx(b"stbl", &stbl));
    let mdia = bx(
        b"mdia",
        &[full(b"mdhd", &mdhd), full(b"hdlr", &hdlr), minf].concat(),
    );
    let trak = bx(b"trak", &[full(b"tkhd", &tkhd), mdia].concat());
    let mut moov = [full(b"mvhd", &mvhd), trak].concat();
    if let Some(id) = content_id {
        let key = b"com.apple.quicktime.content.identifier";
        let mut keys = 1u32.to_be_bytes().to_vec();
        keys.extend(((key.len() + 8) as u32).to_be_bytes());
        keys.extend(b"mdta");
        keys.extend(key);
        let mut data = 1u32.to_be_bytes().to_vec();
        data.extend(0u32.to_be_bytes());
        data.extend(id.as_bytes());
        let item = bx(&1u32.to_be_bytes(), &bx(b"data", &data));
        let meta = [
            bx(b"hdlr", &[0u8; 25]),
            full(b"keys", &keys),
            bx(b"ilst", &item),
        ]
        .concat();
        moov.extend(bx(b"meta", &meta));
    }
    out.extend(bx(b"moov", &moov));
    out
}

/// A big-endian TIFF whose IFD0 points at an Apple MakerNote with tag 0x0011.
fn exif_with_content_id(id: &str) -> Vec<u8> {
    let mut mn = b"Apple iOS\0\0\x01MM".to_vec();
    let ifd_at = mn.len();
    let val_at = ifd_at + 2 + 12 + 4;
    mn.extend(1u16.to_be_bytes());
    mn.extend(0x0011u16.to_be_bytes());
    mn.extend(2u16.to_be_bytes());
    mn.extend(((id.len() + 1) as u32).to_be_bytes());
    mn.extend((val_at as u32).to_be_bytes());
    mn.extend(0u32.to_be_bytes());
    mn.extend(id.as_bytes());
    mn.push(0);
    // TIFF: IFD0 with ExifIFD pointer; Exif IFD with MakerNote (0x927C, UNDEFINED).
    let mut t = b"MM\0\x2a".to_vec();
    t.extend(8u32.to_be_bytes());
    t.extend(1u16.to_be_bytes());
    t.extend(0x8769u16.to_be_bytes());
    t.extend(4u16.to_be_bytes());
    t.extend(1u32.to_be_bytes());
    t.extend(26u32.to_be_bytes());
    t.extend(0u32.to_be_bytes());
    // Exif IFD at 26.
    t.extend(1u16.to_be_bytes());
    t.extend(0x927cu16.to_be_bytes());
    t.extend(7u16.to_be_bytes());
    t.extend((mn.len() as u32).to_be_bytes());
    t.extend(44u32.to_be_bytes());
    t.extend(0u32.to_be_bytes());
    assert_eq!(t.len(), 44);
    t.extend(mn);
    t
}

const ID: &str = "A1B2C3D4-0000-4000-8000-0123456789AB";

#[test]
fn a_sibling_movie_is_a_live_photo() {
    let d = dir("pair");
    std::fs::write(d.join("IMG_0001.HEIC"), b"still").unwrap();
    std::fs::write(d.join("IMG_0001.MOV"), mov(None)).unwrap();
    let info = specular_video::probe_live(&d.join("IMG_0001.HEIC"), None).unwrap();
    assert_eq!(info.kind, specular_video::Kind::LivePhoto);
    assert_eq!(
        info.source,
        specular_video::Source::Sibling(d.join("IMG_0001.MOV"))
    );
    assert_eq!(info.codec.as_deref(), Some("HEVC"));
    assert_eq!((info.width, info.height), (1440, 1920), "rotated 90°");
    assert_eq!(info.duration_us, Some(3_000_000));
    assert_eq!(info.paired, None);
    // No sibling, a different stem, or a directory named like one: nothing.
    std::fs::write(d.join("IMG_0002.JPG"), b"still").unwrap();
    assert_eq!(
        specular_video::probe_live(&d.join("IMG_0002.JPG"), None),
        None
    );
    std::fs::create_dir(d.join("IMG_0003.MOV")).unwrap();
    std::fs::write(d.join("IMG_0003.JPG"), b"still").unwrap();
    assert_eq!(
        specular_video::probe_live(&d.join("IMG_0003.JPG"), None),
        None
    );
    // A truncated movie is not one.
    std::fs::write(d.join("IMG_0004.JPG"), b"still").unwrap();
    std::fs::write(d.join("IMG_0004.mov"), &mov(None)[..5000]).unwrap();
    assert_eq!(
        specular_video::probe_live(&d.join("IMG_0004.JPG"), None),
        None
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn content_identifiers_confirm_or_refute_the_pair() {
    let d = dir("ids");
    let still = d.join("IMG_0010.jpg");
    std::fs::write(&still, b"still").unwrap();
    std::fs::write(d.join("IMG_0010.MOV"), mov(Some(ID))).unwrap();
    let exif = exif_with_content_id(ID);
    assert_eq!(
        specular_video::motion::apple_content_id(&exif).as_deref(),
        Some(ID)
    );
    let info = specular_video::probe_live(&still, Some(&exif)).unwrap();
    assert_eq!(info.paired, Some(true));
    // Case differences are the same identifier.
    let lower = exif_with_content_id(&ID.to_ascii_lowercase());
    assert_eq!(
        specular_video::probe_live(&still, Some(&lower))
            .unwrap()
            .paired,
        Some(true)
    );
    // A different identifier: another shot's movie, not this still's.
    let other = exif_with_content_id("FFFFFFFF-0000-4000-8000-0123456789AB");
    assert_eq!(specular_video::probe_live(&still, Some(&other)), None);
    let _ = std::fs::remove_dir_all(&d);
}
