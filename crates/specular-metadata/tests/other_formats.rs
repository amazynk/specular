//! The info panel's path for formats other than JPEG: the Exif that specular-decode finds
//! without decoding (PNG, WebP, HEIC, AVIF, JPEG XL, PSD, CR3, TIFF) read into a report like a JPEG's.
//! Fixtures: crates/specular-thumbs/tests/data/gpsmeta/make_fixtures.sh.

use specular_metadata::*;
use std::path::PathBuf;

#[test]
fn exif_of_other_formats_reaches_the_report() {
    let dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../specular-thumbs/tests/data/gpsmeta");
    let l = Localizer::new("en".parse().unwrap());
    for name in [
        "gps.png",
        "gps_rawprofile.png",
        "gps.webp",
        "gps.heic",
        "gps.avif",
        "gps.jxl",
        "gps_brob.jxl",
        "gps.psd",
        "gps.cr3",
        "gps.tif",
    ] {
        let data = std::fs::read(dir.join(name)).unwrap();
        let exif = specular_decode::exif_tiff(&data);
        assert!(exif.is_some(), "{name}");
        let input = MetadataInput {
            exif_tiff: exif.as_deref(),
            ..Default::default()
        };
        let r = read(&input, &l);
        let gps = r
            .gps
            .as_ref()
            .unwrap_or_else(|| panic!("{name}: no position"));
        assert!(
            (gps.lat - 10.0).abs() < 1e-6 && (gps.lon - 20.0).abs() < 1e-6,
            "{name}"
        );
        assert_eq!(
            r.capture_time.as_deref(),
            Some("2021-07-14T09:26:53"),
            "{name}"
        );
        let camera = r.summary.iter().find(|g| g.key == "camera");
        assert!(camera.is_some(), "{name}: no camera group");
    }
}

/// gps.cr3's Canon maker note (CMT3): the lens (CMT2 names none), its focal length and
/// aperture, focus mode and image stabilisation reach the summary; its serial number shows
/// nowhere, not even among all fields.
#[test]
fn cr3_maker_note_reaches_the_report() {
    let dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../specular-thumbs/tests/data/gpsmeta");
    let data = std::fs::read(dir.join("gps.cr3")).unwrap();
    let exif = specular_decode::exif_tiff(&data);
    let l = Localizer::new("en".parse().unwrap());
    let r = read(
        &MetadataInput {
            exif_tiff: exif.as_deref(),
            ..Default::default()
        },
        &l,
    );
    let item = |group: &str, key: &str| {
        r.summary
            .iter()
            .find(|g| g.key == group)
            .and_then(|g| g.items.iter().find(|i| i.key == key))
            .unwrap_or_else(|| panic!("no {group}/{key}: {:?}", r.summary))
    };
    let lens = item("lens", "lens");
    assert_eq!(strip_isolates(&lens.value), "Fixture Lens 35mm F2");
    assert_eq!(
        lens.detail.as_deref().map(strip_isolates).as_deref(),
        Some("35 mm f/2")
    );
    assert_eq!(strip_isolates(&item("lens", "stabilisation").value), "On");
    assert_eq!(
        strip_isolates(&item("exposure", "focus_mode").value),
        "AI Servo AF"
    );
    // Only the location warning: the rebuilt maker note reads cleanly.
    assert_eq!(r.warnings.len(), 1, "{:?}", r.warnings);
    // 0x5E81A1 in any notation.
    let all = format!("{:?} {:?}", r.summary, r.all_fields);
    for s in ["5E81A1", "5e81a1", "6193569"] {
        assert!(!all.contains(s), "serial shown: {all}");
    }
}

/// A JPEG XL container whose Exif and XMP boxes are Brotli-compressed (`brob`): both reach the
/// report the way the app's metadata path passes them.
#[test]
fn brotli_compressed_jxl_boxes() {
    let dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../specular-thumbs/tests/data/gpsmeta");
    let data = std::fs::read(dir.join("gps_brob.jxl")).unwrap();
    assert!(data.windows(8).any(|w| w == b"brobExif"));
    let exif = specular_decode::exif_tiff(&data);
    let xmp = specular_decode::jxl_xmp(&data);
    assert!(xmp.as_deref().is_some_and(|x| x.starts_with(b"<?xpacket")));
    let l = Localizer::new("en".parse().unwrap());
    let r = read(
        &MetadataInput {
            exif_tiff: exif.as_deref(),
            xmp: xmp.as_deref(),
            ..Default::default()
        },
        &l,
    );
    let value = |key: &str| {
        r.summary
            .iter()
            .flat_map(|g| &g.items)
            .find(|i| i.key == key)
            .map(|i| i.value.clone())
    };
    assert!(value("title").is_some_and(|v| v.contains("Brotli box fixture")));
    assert!(value("rating").is_some(), "no rating");
    assert!(r.gps.is_some());
    // The plain container has no XMP.
    let plain = std::fs::read(dir.join("gps.jxl")).unwrap();
    assert!(specular_decode::jxl_xmp(&plain).is_none());
}

/// A TIFF's Exif is IFD0's descriptive tags with the Exif and GPS IFDs, listed like a JPEG's:
/// the image layout (size, strips, samples) is not, and no other IFD shows.
#[test]
fn tiff_fields_are_listed_like_a_jpegs() {
    let dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../specular-thumbs/tests/data/gpsmeta");
    let data = std::fs::read(dir.join("gps.tif")).unwrap();
    let exif = specular_decode::exif_tiff(&data).expect("exif");
    assert!(
        exif.len() < data.len() / 2,
        "{} of {}",
        exif.len(),
        data.len()
    );
    let l = Localizer::new("en".parse().unwrap());
    let r = read(
        &MetadataInput {
            exif_tiff: Some(&exif),
            ..Default::default()
        },
        &l,
    );
    // Only the location notice: nothing failed to parse.
    assert!(
        r.warnings.len() == 1 && r.warnings[0].contains("location"),
        "{:?}",
        r.warnings
    );
    let titles: Vec<&str> = r.all_fields.iter().map(|g| g.title.as_str()).collect();
    assert_eq!(titles, ["IFD0 (main image)", "Exif", "GPS"]);
    let tags: Vec<String> = r
        .all_fields
        .iter()
        .flat_map(|g| {
            g.fields
                .iter()
                .map(|f| f.tag.replace(' ', "").to_lowercase())
        })
        .collect();
    for want in [
        "make",
        "model",
        "orientation",
        "datetimeoriginal",
        "gpslatitude",
    ] {
        assert!(
            tags.iter().any(|t| t.contains(want)),
            "{want} missing: {tags:?}"
        );
    }
    for gone in ["stripoffsets", "imagewidth", "bitspersample", "compression"] {
        assert!(!tags.iter().any(|t| t.contains(gone)), "{gone}: {tags:?}");
    }
}

// ------------------------------------------------------------------ XMP and ICC, synthetic

/// An XMP packet with a title, a creator, a rating and a colour label.
const XMP: &str = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/"
 xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmp:Rating="4" xmp:Label="Green">
<dc:title><rdf:Alt><rdf:li xml:lang="x-default">Harbour at dusk</rdf:li></rdf:Alt></dc:title>
<dc:creator><rdf:Seq><rdf:li>A. Photographer</rdf:li></rdf:Seq></dc:creator>
</rdf:Description></rdf:RDF></x:xmpmeta>
<?xpacket end="w"?>"#;

const PROFILE: &str = "Synthetic Wide RGB";

/// A minimal ICC v2 display profile whose only tag is a `desc` of [`PROFILE`].
fn icc() -> Vec<u8> {
    let mut desc = b"desc\0\0\0\0".to_vec();
    desc.extend_from_slice(&(PROFILE.len() as u32 + 1).to_be_bytes());
    desc.extend_from_slice(PROFILE.as_bytes());
    desc.push(0);
    // Empty Unicode and ScriptCode descriptions.
    desc.extend_from_slice(&[0; 8 + 3 + 67]);
    let mut p = vec![0u8; 128];
    p[8] = 2;
    p[12..16].copy_from_slice(b"mntr");
    p[16..20].copy_from_slice(b"RGB ");
    p[20..24].copy_from_slice(b"XYZ ");
    p[36..40].copy_from_slice(b"acsp");
    p.extend_from_slice(&1u32.to_be_bytes());
    p.extend_from_slice(b"desc");
    p.extend_from_slice(&144u32.to_be_bytes());
    p.extend_from_slice(&(desc.len() as u32).to_be_bytes());
    p.extend(desc);
    let n = p.len() as u32;
    p[..4].copy_from_slice(&n.to_be_bytes());
    p
}

/// A stored (uncompressed) zlib stream.
fn zlib(data: &[u8]) -> Vec<u8> {
    assert!(data.len() < 65536);
    let mut z = vec![0x78, 0x01, 1];
    z.extend_from_slice(&(data.len() as u16).to_le_bytes());
    z.extend_from_slice(&(!(data.len() as u16)).to_le_bytes());
    z.extend_from_slice(data);
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&(b << 16 | a).to_be_bytes());
    z
}

fn png() -> Vec<u8> {
    let chunk = |kind: &[u8], data: &[u8]| {
        let mut c = (data.len() as u32).to_be_bytes().to_vec();
        c.extend_from_slice(kind);
        c.extend_from_slice(data);
        c.extend_from_slice(&[0; 4]);
        c
    };
    let mut iccp = b"ICC\0\0".to_vec();
    iccp.extend(zlib(&icc()));
    // Compressed iTXt.
    let mut itxt = b"XML:com.adobe.xmp\0\x01\0\0\0".to_vec();
    itxt.extend(zlib(XMP.as_bytes()));
    let mut f = b"\x89PNG\r\n\x1a\n".to_vec();
    f.extend(chunk(b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0, 0, 0]));
    f.extend(chunk(b"iCCP", &iccp));
    f.extend(chunk(b"IDAT", &zlib(&[0, 0, 0, 0])));
    f.extend(chunk(b"iTXt", &itxt));
    f.extend(chunk(b"IEND", &[]));
    f
}

fn webp() -> Vec<u8> {
    let mut body = b"WEBP".to_vec();
    let icc = icc();
    for (k, d) in [
        (&b"VP8X"[..], &[0x24, 0, 0, 0, 0, 0, 0, 0, 0, 0][..]),
        (b"ICCP", &icc),
        (b"VP8L", &[0x2f, 0, 0, 0, 0]),
        (b"XMP ", XMP.as_bytes()),
    ] {
        body.extend_from_slice(k);
        body.extend_from_slice(&(d.len() as u32).to_le_bytes());
        body.extend_from_slice(d);
        if d.len() % 2 == 1 {
            body.push(0);
        }
    }
    let mut f = b"RIFF".to_vec();
    f.extend_from_slice(&(body.len() as u32).to_le_bytes());
    f.extend(body);
    f
}

fn bx(kind: &[u8], body: &[u8]) -> Vec<u8> {
    let mut b = ((body.len() + 8) as u32).to_be_bytes().to_vec();
    b.extend_from_slice(kind);
    b.extend_from_slice(body);
    b
}

/// A full box body: version 0, no flags, then `rest`.
fn full(rest: &[u8]) -> Vec<u8> {
    let mut b = vec![0; 4];
    b.extend_from_slice(rest);
    b
}

/// HEIF or AVIF (`brand`): primary item 1 (`coded`) with a `colr` `prof` property, and an XMP
/// `mime` item 2 describing it, stored in `mdat`.
fn heif(brand: &[u8; 4], coded: &[u8; 4]) -> Vec<u8> {
    let infe = |id: u8, kind: &[u8], extra: &[u8]| {
        let mut b = vec![2, 0, 0, 0, 0, id, 0, 0];
        b.extend_from_slice(kind);
        b.push(0);
        b.extend_from_slice(extra);
        bx(b"infe", &b)
    };
    let mut iinf = full(&[0, 2]);
    iinf.extend(infe(1, coded, b""));
    iinf.extend(infe(2, b"mime", b"application/rdf+xml\0"));
    let mut prof = b"prof".to_vec();
    prof.extend(icc());
    let mut ipco = bx(b"ispe", &full(&[0, 0, 0, 1, 0, 0, 0, 1]));
    ipco.extend(bx(b"colr", &prof));
    // Item 1: properties 1 and 2 (essential).
    let ipma = full(&[0, 0, 0, 1, 0, 1, 2, 0x81, 0x82]);
    let iprp = [bx(b"ipco", &ipco), bx(b"ipma", &ipma)].concat();
    let xmp = XMP.as_bytes();
    let meta = |at: u32| {
        // iloc v0: 4-byte offsets and lengths; item 2, one extent.
        let mut iloc = full(&[0x44, 0x00, 0, 1, 0, 2, 0, 0, 0, 1]);
        iloc.extend_from_slice(&at.to_be_bytes());
        iloc.extend_from_slice(&(xmp.len() as u32).to_be_bytes());
        let mut m = bx(b"hdlr", &full(&[0; 20]));
        m.extend(bx(b"pitm", &full(&[0, 1])));
        m.extend(bx(b"iinf", &iinf));
        m.extend(bx(b"iref", &full(&bx(b"cdsc", &[0, 2, 0, 1, 0, 1]))));
        m.extend(bx(b"iprp", &iprp));
        m.extend(bx(b"iloc", &iloc));
        bx(b"meta", &full(&m))
    };
    let mut ftyp = brand.to_vec();
    ftyp.extend_from_slice(&[0; 4]);
    ftyp.extend_from_slice(b"mif1");
    ftyp.extend_from_slice(brand);
    let ftyp = bx(b"ftyp", &ftyp);
    let at = (ftyp.len() + meta(0).len() + 8) as u32;
    [ftyp, meta(at), bx(b"mdat", xmp)].concat()
}

/// A big-endian TIFF whose IFD0 has an XMP packet (tag 700) and an ICC profile (34675).
fn tiff() -> Vec<u8> {
    let icc = icc();
    let xmp = XMP.as_bytes();
    let data_at = 8 + 2 + 2 * 12 + 4;
    let mut f = b"MM\0*\0\0\0\x08\0\x02".to_vec();
    for (tag, kind, len, at) in [
        (700u16, 1u16, xmp.len(), data_at),
        (34675, 7, icc.len(), data_at + xmp.len()),
    ] {
        f.extend_from_slice(&tag.to_be_bytes());
        f.extend_from_slice(&kind.to_be_bytes());
        f.extend_from_slice(&(len as u32).to_be_bytes());
        f.extend_from_slice(&(at as u32).to_be_bytes());
    }
    f.extend_from_slice(&[0; 4]);
    f.extend_from_slice(xmp);
    f.extend(icc);
    f
}

fn jxl() -> Vec<u8> {
    [
        bx(b"JXL ", b"\r\n\x87\n"),
        bx(b"ftyp", b"jxl \0\0\0\0jxl "),
        bx(b"xml ", XMP.as_bytes()),
        bx(b"jxlc", &[0xFF, 0x0A, 0, 0]),
    ]
    .concat()
}

/// PSD (`version` 1) or PSB (2) whose image resources hold the ICC profile (1039) and XMP
/// (1060), after an unrelated resource with a name.
fn psd(version: u16) -> Vec<u8> {
    let resource = |id: u16, name: &[u8], data: &[u8]| {
        let mut r = b"8BIM".to_vec();
        r.extend_from_slice(&id.to_be_bytes());
        r.push(name.len() as u8);
        r.extend_from_slice(name);
        if name.len().is_multiple_of(2) {
            r.push(0);
        }
        r.extend_from_slice(&(data.len() as u32).to_be_bytes());
        r.extend_from_slice(data);
        if data.len() % 2 == 1 {
            r.push(0);
        }
        r
    };
    let resources = [
        resource(1005, b"res", &[0; 16]),
        resource(1039, b"", &icc()),
        resource(1060, b"", XMP.as_bytes()),
    ]
    .concat();
    let mut f = b"8BPS".to_vec();
    f.extend_from_slice(&version.to_be_bytes());
    f.extend_from_slice(&[0; 6]);
    f.extend_from_slice(&[0, 3, 0, 0, 0, 1, 0, 0, 0, 1, 0, 8, 0, 3]);
    f.extend_from_slice(&[0; 4]);
    f.extend_from_slice(&(resources.len() as u32).to_be_bytes());
    f.extend(resources);
    f.extend_from_slice(&[0; 8]);
    f
}

/// A Canon CR3 skeleton whose XMP `uuid` box is NUL-padded like a camera's.
fn cr3() -> Vec<u8> {
    let mut xmp = b"\xbe\x7a\xcf\xcb\x97\xa9\x42\xe8\x9c\x71\x99\x94\x91\xe3\xaf\xac".to_vec();
    xmp.extend_from_slice(XMP.as_bytes());
    xmp.resize(xmp.len() + 300, 0);
    [
        bx(b"ftyp", b"crx \0\0\0\x01crx isom"),
        bx(b"moov", &bx(b"mvhd", &full(&[0; 96]))),
        bx(b"uuid", &xmp),
        bx(b"mdat", &[0; 16]),
    ]
    .concat()
}

/// The XMP and ICC that `specular_decode::embedded_metadata` finds in PNG, WebP, HEIC, AVIF,
/// TIFF, JPEG XL, PSD/PSB and CR3 reach the info panel's rows like a JPEG's: title, creator, rating, label
/// and the colour profile's description.
#[test]
fn xmp_and_icc_of_other_formats_reach_the_report() {
    let l = Localizer::new("en".parse().unwrap());
    for (name, file, has_icc) in [
        ("png", png(), true),
        ("webp", webp(), true),
        ("heic", heif(b"heic", b"hvc1"), true),
        ("avif", heif(b"avif", b"av01"), true),
        ("tiff", tiff(), true),
        ("jxl", jxl(), false),
        ("psd", psd(1), true),
        ("psb", psd(2), true),
        ("cr3", cr3(), false),
    ] {
        let found = specular_decode::embedded_metadata(&file);
        assert_eq!(found.xmp.as_deref(), Some(XMP.as_bytes()), "{name}");
        let r = read(
            &MetadataInput {
                xmp: found.xmp.as_deref(),
                icc: found.icc.as_deref(),
                ..Default::default()
            },
            &l,
        );
        let value = |key: &str| {
            r.summary
                .iter()
                .flat_map(|g| &g.items)
                .find(|i| i.key == key)
                .map(|i| strip_isolates(&i.value))
        };
        assert_eq!(value("title").as_deref(), Some("Harbour at dusk"), "{name}");
        assert_eq!(
            value("creator").as_deref(),
            Some("A. Photographer"),
            "{name}"
        );
        assert_eq!(r.rating, Some(4), "{name}");
        assert!(value("rating").is_some(), "{name}");
        assert_eq!(r.label.as_deref(), Some("Green"), "{name}");
        assert!(r.warnings.is_empty(), "{name}: {:?}", r.warnings);
        if has_icc {
            assert_eq!(r.icc_description.as_deref(), Some(PROFILE), "{name}");
            assert_eq!(value("colour_profile").as_deref(), Some(PROFILE), "{name}");
        } else {
            assert_eq!(found.icc, None, "{name}");
        }
    }
}

/// An IFD entry for [`ifd_at`]: tag, type, count, value in file order, and a fixed offset for
/// a value that lies elsewhere (the maker note).
type Ent = (u16, u16, u32, Vec<u8>, Option<u32>);

/// An IFD of byte order `le` laid at offset `at` of its file, its values after it.
fn ifd_at(le: bool, at: u32, mut entries: Vec<Ent>) -> Vec<u8> {
    let w16 = |v: u16| if le { v.to_le_bytes() } else { v.to_be_bytes() };
    let w32 = |v: u32| if le { v.to_le_bytes() } else { v.to_be_bytes() };
    entries.sort_by_key(|e| e.0);
    let mut data = at + 2 + 12 * entries.len() as u32 + 4;
    let (mut b, mut tail) = (w16(entries.len() as u16).to_vec(), Vec::new());
    for (tag, typ, count, mut v, fixed) in entries {
        b.extend(w16(tag));
        b.extend(w16(typ));
        b.extend(w32(count));
        if let Some(o) = fixed {
            b.extend(w32(o));
        } else if v.len() <= 4 {
            v.resize(4, 0);
            b.extend(v);
        } else {
            b.extend(w32(data));
            if v.len() % 2 == 1 {
                v.push(0);
            }
            data += v.len() as u32;
            tail.extend(v);
        }
    }
    b.extend([0; 4]);
    b.extend(tail);
    b
}

fn ascii_ent(tag: u16, s: &str) -> Ent {
    let mut v = s.as_bytes().to_vec();
    v.push(0);
    (tag, 2, v.len() as u32, v, None)
}

/// A TIFF of byte order `le` and magic `magic` whose Exif IFD holds `exif` and a Panasonic
/// note with `note` entries, and whose IFD0 holds Make "Panasonic" and `ifd0`.
fn panasonic_tiff(le: bool, magic: u16, note: Vec<Ent>, exif: Vec<Ent>, ifd0: Vec<Ent>) -> Vec<u8> {
    let w32 = |v: u32| if le { v.to_le_bytes() } else { v.to_be_bytes() };
    let mut f = if le { b"II".to_vec() } else { b"MM".to_vec() };
    f.extend(if le {
        magic.to_le_bytes()
    } else {
        magic.to_be_bytes()
    });
    f.extend([0; 4]);
    let mut n = b"Panasonic\0\0\0".to_vec();
    n.extend(ifd_at(le, 8 + 12, note));
    let note_len = n.len() as u32;
    f.extend(n);
    let exif_at = f.len() as u32;
    let mut x = exif;
    x.push((0x927C, 7, note_len, vec![], Some(8)));
    f.extend(ifd_at(le, exif_at, x));
    let ifd0_at = f.len() as u32;
    let mut e = ifd0;
    e.push(ascii_ent(0x010F, "Panasonic"));
    e.push(ascii_ent(0x0110, "DC-FIXTURE"));
    e.push((0x8769, 4, 1, w32(exif_at).to_vec(), None));
    f.extend(ifd_at(le, ifd0_at, e));
    f[4..8].copy_from_slice(&w32(ifd0_at));
    f
}

/// A Panasonic note's focus mode, stabilisation and lens reach the summary from a little- or
/// big-endian file (the latter swapped when rebuilt); its serials show nowhere.
#[test]
fn panasonic_maker_note_reaches_the_report() {
    let l = Localizer::new("en".parse().unwrap());
    for le in [true, false] {
        let s = |v: u16| if le { v.to_le_bytes() } else { v.to_be_bytes() }.to_vec();
        let note = vec![
            (0x0007, 3, 1, s(7), None),
            (0x001A, 3, 1, s(11), None),
            ascii_ent(0x0051, "LUMIX G VARIO 12-60/F3.5-5.6"),
            ascii_ent(0x0052, "LENS-SERIAL-77"),
        ];
        let f = panasonic_tiff(le, 42, note, vec![], vec![]);
        let exif = specular_decode::exif_tiff(&f).expect("exif");
        let r = read(
            &MetadataInput {
                exif_tiff: Some(&exif),
                ..Default::default()
            },
            &l,
        );
        let item = |group: &str, key: &str| {
            r.summary
                .iter()
                .find(|g| g.key == group)
                .and_then(|g| g.items.iter().find(|i| i.key == key))
                .map(|i| strip_isolates(&i.value))
        };
        assert_eq!(item("exposure", "focus_mode").as_deref(), Some("AF-C"));
        assert_eq!(item("lens", "stabilisation").as_deref(), Some("Dual IS 2"));
        assert_eq!(
            item("lens", "lens").as_deref(),
            Some("LUMIX G VARIO 12-60/F3.5-5.6")
        );
        let all = format!("{:?} {:?}", r.summary, r.all_fields);
        assert!(!all.contains("LENS-SERIAL"), "serial shown: {all}");
    }
}

/// An RW2 without a preview: the raw ISO and the CameraIFD's 35 mm focal length reach the
/// summary.
#[test]
fn rw2_raw_values_reach_the_report() {
    let mut cam = b"II*\0\x08\0\0\0".to_vec();
    cam.extend(ifd_at(
        true,
        8,
        vec![
            (0x1200, 0x101, 1, 1u32.to_le_bytes().to_vec(), None),
            (0x1203, 3, 1, 50u16.to_le_bytes().to_vec(), None),
        ],
    ));
    cam.resize(64, 0);
    let ifd0 = vec![
        (0x0017, 3, 1, 640u16.to_le_bytes().to_vec(), None),
        (0x0120, 7, cam.len() as u32, cam, None),
    ];
    let exposure = [1u32.to_le_bytes(), 250u32.to_le_bytes()].concat();
    let exif = vec![(0x829A, 5, 1, exposure, None)];
    let f = panasonic_tiff(true, 0x55, vec![], exif, ifd0);
    let exif = specular_decode::exif_tiff(&f).expect("exif");
    let l = Localizer::new("en".parse().unwrap());
    let r = read(
        &MetadataInput {
            exif_tiff: Some(&exif),
            ..Default::default()
        },
        &l,
    );
    let item = |key: &str| {
        r.summary
            .iter()
            .flat_map(|g| &g.items)
            .find(|i| i.key == key)
            .map(|i| strip_isolates(&i.value))
            .unwrap_or_else(|| panic!("no {key}: {:?}", r.summary))
    };
    assert!(item("iso").contains("640"), "{}", item("iso"));
    assert!(
        item("focal_length").contains("50"),
        "{}",
        item("focal_length")
    );
}

/// A Samsung NX note's LensType names the lens of a file whose Exif has no LensModel, and its
/// focal range and apertures follow from the name; the serials show nowhere.
#[test]
fn samsung_lens_type_reaches_the_report() {
    let l = Localizer::new("en".parse().unwrap());
    for le in [true, false] {
        let w16 = |v: u16| if le { v.to_le_bytes() } else { v.to_be_bytes() }.to_vec();
        let w32 = |v: u32| if le { v.to_le_bytes() } else { v.to_be_bytes() }.to_vec();
        let mut f = if le { b"II*\0" } else { b"MM\0*" }.to_vec();
        f.extend([0; 4]);
        let note = ifd_at(
            le,
            8,
            vec![
                (0x0001, 7, 4, b"0100".to_vec(), None),
                ascii_ent(0xA002, "BODY-SERIAL-77"),
                (0xA003, 3, 1, w16(13), None),
                ascii_ent(0xA005, "LENS-SERIAL-77"),
            ],
        );
        let note_len = note.len() as u32;
        f.extend(note);
        let exif_at = f.len() as u32;
        let focal = [w32(35), w32(1)].concat();
        f.extend(ifd_at(
            le,
            exif_at,
            vec![
                (0x920A, 5, 1, focal, None),
                (0x927C, 7, note_len, vec![], Some(8)),
            ],
        ));
        let ifd0_at = f.len() as u32;
        f.extend(ifd_at(
            le,
            ifd0_at,
            vec![
                ascii_ent(0x010F, "SAMSUNG"),
                ascii_ent(0x0110, "NX-FIXTURE"),
                (0x8769, 4, 1, w32(exif_at), None),
            ],
        ));
        f[4..8].copy_from_slice(&w32(ifd0_at));
        let exif = specular_decode::exif_tiff(&f).expect("exif");
        let r = read(
            &MetadataInput {
                exif_tiff: Some(&exif),
                ..Default::default()
            },
            &l,
        );
        let lens = r
            .summary
            .iter()
            .find(|g| g.key == "lens")
            .and_then(|g| g.items.iter().find(|i| i.key == "lens"))
            .unwrap_or_else(|| panic!("no lens: {:?}", r.summary));
        assert_eq!(
            strip_isolates(&lens.value),
            "Samsung NX 16-50mm F2-2.8 S ED OIS"
        );
        let detail = strip_isolates(lens.detail.as_deref().unwrap_or_default());
        assert_eq!(detail, "16–50 mm f/2–2.8");
        let all = format!("{:?} {:?}", r.summary, r.all_fields);
        assert!(!all.contains("SERIAL-77"), "serial shown: {all}");
    }
}

/// A DICOM image: the medical group comes first, the two spacings are told apart, a lossy
/// transfer syntax is marked, and the not-for-diagnosis warning leads the warnings.
#[test]
fn medical_group_and_warning() {
    let l = Localizer::new("en".parse().unwrap());
    let input = MetadataInput {
        medical: Some(MedicalFacts {
            modality: Some("CR".into()),
            body_part: Some("CHEST".into()),
            study_description: Some(" ".into()),
            series_description: Some("PA".into()),
            slice_thickness: Some(2.5),
            pixel_spacing: None,
            imager_pixel_spacing: Some((0.139, 0.139)),
            frames: Some(1),
            transfer_syntax: Some("JPEG Baseline (Process 1)".into()),
            lossy: true,
            document_title: None,
            document_type: None,
            nde: None,
        }),
        ..Default::default()
    };
    let r = read(&input, &l);
    let g = &r.summary[0];
    assert_eq!(g.key, "medical");
    let keys: Vec<_> = g.items.iter().map(|i| i.key).collect();
    assert_eq!(
        keys,
        [
            "modality",
            "body_part",
            "series",
            "slice_thickness",
            "imager_pixel_spacing",
            "transfer_syntax"
        ]
    );
    let item = |k: &str| strip_isolates(&g.items.iter().find(|i| i.key == k).unwrap().value);
    assert_eq!(item("slice_thickness"), "2.5 mm");
    assert_eq!(item("imager_pixel_spacing"), "0.139 × 0.139 mm");
    let detail = g.items.iter().find(|i| i.key == "imager_pixel_spacing");
    assert!(
        detail
            .unwrap()
            .detail
            .as_ref()
            .unwrap()
            .contains("detector")
    );
    assert!(item("transfer_syntax").contains("lossy"));
    assert!(r.warnings[0].contains("Not for diagnostic use"));

    let plain = read(&MetadataInput::default(), &l);
    assert!(plain.summary.iter().all(|g| g.key != "medical"));
    assert!(plain.warnings.iter().all(|w| !w.contains("diagnostic")));
}

/// A NIfTI volume: the medical group (with its warning) then the volume group, whose
/// orientation source carries a caution when the file's orientation is not reliable.
#[test]
fn volume_group() {
    let l = Localizer::new("en".parse().unwrap());
    let input = MetadataInput {
        medical: Some(MedicalFacts {
            pixel_spacing: Some((1.0, 1.0)),
            frames: Some(120),
            ..Default::default()
        }),
        volume: Some(VolumeFacts {
            format: "NIfTI-1".into(),
            dims: vec![64, 64, 30, 4],
            voxel_size: Some([1.0, 1.0, 2.5]),
            data_type: "int16".into(),
            volumes: 4,
            orientation: "sform".into(),
            reliable: true,
            description: Some("fMRI run".into()),
            intent: None,
            compression: Some("gzip".into()),
            params: Vec::new(),
        }),
        ..Default::default()
    };
    let r = read(&input, &l);
    assert_eq!(r.summary[0].key, "medical");
    let g = &r.summary[1];
    assert_eq!(g.key, "volume");
    let keys: Vec<_> = g.items.iter().map(|i| i.key).collect();
    assert_eq!(
        keys,
        [
            "format",
            "dims",
            "voxel_size",
            "data_type",
            "volumes",
            "orientation",
            "description",
            "compression"
        ]
    );
    let item = |k: &str| strip_isolates(&g.items.iter().find(|i| i.key == k).unwrap().value);
    assert_eq!(item("dims"), "64 × 64 × 30 × 4");
    assert_eq!(item("voxel_size"), "1 × 1 × 2.5 mm");
    assert_eq!(item("orientation"), "NIfTI sform");
    assert!(g.items.iter().all(|i| i.detail.is_none()));
    assert!(r.warnings[0].contains("Not for diagnostic use"));

    let analyze = MetadataInput {
        medical: Some(MedicalFacts::default()),
        volume: Some(VolumeFacts {
            format: "Analyze 7.5".into(),
            orientation: "analyze".into(),
            reliable: false,
            ..Default::default()
        }),
        ..Default::default()
    };
    let r = read(&analyze, &l);
    let g = r.summary.iter().find(|g| g.key == "volume").unwrap();
    let o = g.items.iter().find(|i| i.key == "orientation").unwrap();
    assert!(o.detail.as_ref().unwrap().contains("left and right"));
}

/// A DICONDE file: an inspection image with its component and inspection group, and no
/// medical warning.
#[test]
fn inspection_group() {
    let l = Localizer::new("en".parse().unwrap());
    let input = MetadataInput {
        medical: Some(MedicalFacts {
            modality: Some("CT".into()),
            nde: Some(NdeFacts {
                component_shape: Some("PIPE".into()),
                material_grade: Some("Inconel 718".into()),
                material_thickness: Some(4.5),
                source: Some((225.0, "kV".into())),
                iqi: Some("WIRE (NICKEL)".into()),
                indications: 2,
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let r = read(&input, &l);
    assert_eq!(r.summary[0].key, "medical");
    assert_eq!(r.summary[0].title, "Inspection image (DICONDE)");
    let g = &r.summary[1];
    assert_eq!(g.key, "inspection");
    let keys: Vec<_> = g.items.iter().map(|i| i.key).collect();
    assert_eq!(
        keys,
        [
            "component_shape",
            "material",
            "thickness",
            "source",
            "iqi",
            "indications"
        ]
    );
    let item = |k: &str| strip_isolates(&g.items.iter().find(|i| i.key == k).unwrap().value);
    assert_eq!(item("thickness"), "4.5 mm");
    assert_eq!(item("source"), "225 kV");
    assert!(r.warnings.iter().all(|w| !w.contains("diagnostic")));

    let bare = MetadataInput {
        medical: Some(MedicalFacts {
            nde: Some(NdeFacts::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let r = read(&bare, &l);
    let g = r.summary.iter().find(|g| g.key == "inspection").unwrap();
    assert_eq!(g.items[0].key, "standard");
}

/// An industrial CT volume opened as a research volume (object coordinates) is an
/// inspection image: no medical title and no "not for diagnosis" warning; a NIfTI keeps both.
#[test]
fn industrial_volume_is_not_medical() {
    let l = Localizer::new("en".parse().unwrap());
    let input = |orientation: &str| MetadataInput {
        medical: Some(MedicalFacts {
            frames: Some(488),
            pixel_spacing: Some((0.02, 0.02)),
            ..Default::default()
        }),
        volume: Some(VolumeFacts {
            format: "Bruker SkyScan".into(),
            dims: vec![444, 444, 488],
            orientation: orientation.into(),
            reliable: false,
            ..Default::default()
        }),
        ..Default::default()
    };
    // Every summary help text, to check the patient is not mentioned for an object.
    let details = |r: &specular_metadata::MetadataReport| {
        r.summary
            .iter()
            .flat_map(|g| &g.items)
            .filter_map(|i| i.detail.as_deref().map(strip_isolates))
            .collect::<Vec<_>>()
    };
    let r = read(&input("object"), &l);
    let g = r.summary.iter().find(|g| g.key == "medical").unwrap();
    assert_eq!(g.title, "Inspection image");
    assert!(
        r.warnings.iter().all(|w| !w.contains("diagnos")),
        "{:?}",
        r.warnings
    );
    let d = details(&r);
    assert!(d.len() >= 2, "{d:?}");
    assert!(d.iter().all(|t| !t.contains("in the patient")), "{d:?}");
    assert!(d.iter().any(|t| t.contains("in the object")), "{d:?}");
    let r = read(&input("sform"), &l);
    let g = r.summary.iter().find(|g| g.key == "medical").unwrap();
    assert_eq!(g.title, "Medical image");
    assert!(
        r.warnings.iter().any(|w| w.contains("diagnos")),
        "{:?}",
        r.warnings
    );
    assert!(details(&r).iter().any(|t| t.contains("in the patient")));
}

/// An industrial CT scan: object coordinates, what the file holds and the scan settings,
/// in words and units.
#[test]
fn industrial_ct_volume_group() {
    let l = Localizer::new("en".parse().unwrap());
    let input = MetadataInput {
        volume: Some(VolumeFacts {
            format: "Zeiss Xradia TXM".into(),
            dims: vec![1024, 1024, 1601],
            voxel_size: Some([0.007, 0.007, 0.007]),
            data_type: "uint16".into(),
            volumes: 1,
            orientation: "object".into(),
            reliable: false,
            params: vec![
                ("content".into(), VolumeParam::Text("projections".into())),
                (
                    "projections".into(),
                    VolumeParam::Number(1601.0, String::new()),
                ),
                ("kv".into(), VolumeParam::Number(80.0, "kV".into())),
                ("angle-step".into(), VolumeParam::Number(0.25, "°".into())),
                ("magnification".into(), VolumeParam::Number(4.0, "×".into())),
                ("coords".into(), VolumeParam::Text("rectilinear".into())),
                ("unheard-of".into(), VolumeParam::Text("x".into())),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let r = read(&input, &l);
    let g = r.summary.iter().find(|g| g.key == "volume").unwrap();
    let item = |k: &str| strip_isolates(&g.items.iter().find(|i| i.key == k).unwrap().value);
    assert!(item("orientation").starts_with("Object coordinates"));
    assert_eq!(item("content"), "Projections (raw scan)");
    assert_eq!(item("projections"), "1,601");
    assert_eq!(item("kv"), "80 kV");
    assert_eq!(item("angle-step"), "0.25°");
    assert_eq!(item("magnification"), "4×");
    assert!(item("coords").starts_with("Rectilinear"));
    assert!(g.items.iter().all(|i| i.key != "unheard-of"));
}

/// A FITS image: the science group comes first with its pixel scale and centre, and the
/// header keywords become an all-fields group.
#[test]
fn science_group_and_header() {
    let l = Localizer::new("en".parse().unwrap());
    let input = MetadataInput {
        science: Some(ScienceFacts {
            format: "FITS".into(),
            object: Some("M 51".into()),
            telescope: Some(" ".into()),
            exposure: Some(120.0),
            dims: vec![100, 80, 3],
            pixel_scale: Some(1.25),
            centre: Some((
                "RA 13h 29m 52.7s  Dec +47° 11′ 43″".into(),
                "202.47°, +47.20°".into(),
            )),
            keywords: vec![
                ("SIMPLE".into(), "T".into(), String::new()),
                ("OBJECT".into(), "M 51".into(), "target".into()),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let r = read(&input, &l);
    let g = &r.summary[0];
    assert_eq!(g.key, "science");
    let keys: Vec<_> = g.items.iter().map(|i| i.key).collect();
    assert_eq!(
        keys,
        [
            "object",
            "exposure",
            "dims",
            "centre",
            "centre_deg",
            "pixel_scale"
        ]
    );
    let item = |k: &str| strip_isolates(&g.items.iter().find(|i| i.key == k).unwrap().value);
    assert_eq!(item("exposure"), "120 s");
    assert_eq!(item("dims"), "100 × 80 × 3");
    assert_eq!(item("pixel_scale"), "1.25″ per pixel");
    let h = r.all_fields.last().unwrap();
    assert_eq!(strip_isolates(&h.title), "FITS header");
    assert_eq!(h.fields[1].display, "M 51  / target");
}

/// The galactic and ecliptic frames are words the user's language says; ICRS and FK5 are
/// names and stay.
#[test]
fn sky_frame_words_are_localised() {
    let l = Localizer::new("en".parse().unwrap());
    let frame = |f: &str| {
        let input = MetadataInput {
            science: Some(ScienceFacts {
                format: "FITS".into(),
                frame: Some(f.into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let r = read(&input, &l);
        let g = &r.summary[0];
        strip_isolates(&g.items.iter().find(|i| i.key == "frame").unwrap().value)
    };
    assert_eq!(frame("Galactic"), "Galactic");
    assert_eq!(frame("Ecliptic"), "Ecliptic");
    assert_eq!(frame("FK5 J2000"), "FK5 J2000");
    assert_eq!(frame("ICRS"), "ICRS");
}

/// A slide's annotation sidecar: its own group with counts by kind and group, and every
/// annotation in an all-fields group.
#[test]
fn slide_annotations_group_and_list() {
    let l = Localizer::new("en".parse().unwrap());
    let item = |name: &str, group: &str, shape: &str, measure: Option<&str>| AnnotationItem {
        name: name.into(),
        group: group.into(),
        shape: shape.into(),
        measure: measure.map(Into::into),
    };
    let input = MetadataInput {
        science: Some(ScienceFacts {
            format: "NDPI".into(),
            domain: ScienceDomain::Slide,
            annotations: Some(AnnotationFacts {
                file: "slide.xml".into(),
                format: "ASAP".into(),
                items: vec![
                    item("Tumor 1", "Tumor", "polygon", Some("1,250 µm²")),
                    item("", "Tumor", "polygon", None),
                    item("r", "", "ruler", Some("12.5 µm")),
                ],
                skipped: 2,
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let r = read(&input, &l);
    let g = r.summary.iter().find(|g| g.key == "annotations").unwrap();
    assert_eq!(strip_isolates(&g.title), "Annotations");
    let value = |k: &str| strip_isolates(&g.items.iter().find(|i| i.key == k).unwrap().value);
    assert_eq!(value("annotation_file"), "slide.xml (ASAP)");
    assert_eq!(value("annotation_shapes"), "Polygon (2), Ruler (1)");
    assert_eq!(value("annotation_groups"), "Tumor (2)");
    assert_eq!(value("annotation_skipped"), "2");
    let f = r.all_fields.last().unwrap();
    assert_eq!(strip_isolates(&f.title), "Annotations in slide.xml");
    let rows: Vec<_> = f
        .fields
        .iter()
        .map(|f| (f.tag.as_str(), f.display.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            ("Tumor 1", "Polygon · Tumor · 1,250 µm²"),
            ("#2", "Polygon · Tumor"),
            ("r", "Ruler · 12.5 µm")
        ]
    );
}

/// Distortions are listed; world coordinates that cannot be followed say why.
#[test]
fn science_wcs_distortion_and_unsupported() {
    let l = Localizer::new("en".parse().unwrap());
    let facts = |distortion: Option<&str>, why: Option<&str>| MetadataInput {
        science: Some(ScienceFacts {
            format: "FITS".into(),
            projection: Some("TAN".into()),
            distortion: distortion.map(Into::into),
            wcs_unsupported: why.map(Into::into),
            ..Default::default()
        }),
        ..Default::default()
    };
    let r = read(&facts(Some("SIP, D2IM, CPDIS"), None), &l);
    let item = |r: &MetadataReport, k: &str| {
        r.summary[0]
            .items
            .iter()
            .find(|i| i.key == k)
            .map(|i| strip_isolates(&i.value))
    };
    assert_eq!(item(&r, "distortion").unwrap(), "SIP, D2IM, CPDIS");
    assert_eq!(item(&r, "wcs_unsupported"), None);
    let r = read(&facts(None, Some("projection TNX")), &l);
    assert_eq!(
        item(&r, "wcs_unsupported").unwrap(),
        "Not supported: projection TNX"
    );
}

/// A SER capture: observer, colour filter array, frames (a cut file shows what is
/// missing), capture times and the frame rate.
#[test]
fn science_ser_video() {
    let l = Localizer::new("en".parse().unwrap());
    let facts = |frames: u64, declared: u64| MetadataInput {
        science: Some(ScienceFacts {
            format: "SER".into(),
            observer: Some("A. Imager".into()),
            telescope: Some("C14".into()),
            bayer: Some("RGGB".into()),
            frames: Some(frames),
            frames_declared: Some(declared),
            capture_start: Some("2026-08-01 22:10:00.000".into()),
            capture_end: Some("2026-08-01 22:11:00.500".into()),
            frame_rate: Some(99.5867),
            dims: vec![640, 480],
            ..Default::default()
        }),
        ..Default::default()
    };
    let item = |r: &MetadataReport, k: &str| {
        r.summary[0]
            .items
            .iter()
            .find(|i| i.key == k)
            .map(|i| strip_isolates(&i.value))
    };
    let r = read(&facts(6000, 6000), &l);
    assert_eq!(item(&r, "observer").unwrap(), "A. Imager");
    assert_eq!(item(&r, "bayer").unwrap(), "RGGB");
    assert_eq!(item(&r, "frames").unwrap(), "6,000");
    assert_eq!(
        item(&r, "capture_start").unwrap(),
        "2026-08-01 22:10:00.000"
    );
    assert_eq!(item(&r, "capture_end").unwrap(), "2026-08-01 22:11:00.500");
    assert_eq!(item(&r, "frame_rate").unwrap(), "99.59 frames/s");
    let r = read(&facts(5123, 6000), &l);
    assert_eq!(item(&r, "frames").unwrap(), "5,123 of 6,000");
}

#[test]
fn science_dataset_and_pixel_size() {
    let l = Localizer::new("en".parse().unwrap());
    let row = |pixel_size: (f64, f64, &str)| {
        let input = MetadataInput {
            science: Some(ScienceFacts {
                format: "EMD".into(),
                dataset: Some("/data/haadf/data".into()),
                pixel_size: Some((pixel_size.0, pixel_size.1, pixel_size.2.into())),
                ..Default::default()
            }),
            ..Default::default()
        };
        let r = read(&input, &l);
        let g = &r.summary[0];
        let item = |k: &str| strip_isolates(&g.items.iter().find(|i| i.key == k).unwrap().value);
        assert_eq!(item("dataset"), "/data/haadf/data");
        item("pixel_size")
    };
    assert_eq!(row((2.5e-10, 2.5e-10, "m")), "0.25 nm per pixel");
    assert_eq!(row((0.25, 0.5, "nm")), "0.25 × 0.5 nm per pixel");
    assert_eq!(row((1.5e-6, 1.5e-6, "m")), "1.5 µm per pixel");
    assert_eq!(row((0.02, 0.02, "1/nm")), "0.02 1/nm per pixel");
}

/// A DigitalMicrograph image: the electron-microscopy group with voltage, magnification and
/// pixel size; a Gwyddion height map titles its group for scanning probe microscopy.
#[test]
fn microscopy_groups() {
    let l = Localizer::new("en".parse().unwrap());
    let input = MetadataInput {
        science: Some(ScienceFacts {
            format: "DigitalMicrograph".into(),
            domain: ScienceDomain::ElectronMicroscopy,
            title: Some("Gold particles".into()),
            instrument: Some("Titan".into()),
            voltage: Some(300_000.0),
            magnification: Some(245_000.0),
            mode: Some("TEM".into()),
            dims: vec![2048, 2048],
            pixel_size: Some((2.5e-10, 2.5e-10, "m".into())),
            keywords: vec![(
                "Microscope Info.Voltage".into(),
                "300000".into(),
                String::new(),
            )],
            ..Default::default()
        }),
        ..Default::default()
    };
    let r = read(&input, &l);
    let g = &r.summary[0];
    assert_eq!(g.key, "science");
    assert_eq!(strip_isolates(&g.title), "Electron microscopy");
    let item = |k: &str| strip_isolates(&g.items.iter().find(|i| i.key == k).unwrap().value);
    assert_eq!(item("voltage"), "300 kV");
    assert_eq!(item("instrument"), "Titan");
    assert_eq!(item("pixel_size"), "0.25 nm per pixel");
    assert!(item("magnification").ends_with('×'));
    let h = r.all_fields.last().unwrap();
    assert_eq!(strip_isolates(&h.title), "DigitalMicrograph header");

    let input = MetadataInput {
        science: Some(ScienceFacts {
            format: "Gwyddion".into(),
            domain: ScienceDomain::ScanningProbe,
            dims: vec![256, 256],
            ..Default::default()
        }),
        ..Default::default()
    };
    let r = read(&input, &l);
    assert_eq!(
        strip_isolates(&r.summary[0].title),
        "Scanning probe microscopy"
    );
}

/// A 3D model: counts, size in its unit, materials and textures with the missing ones.
#[test]
fn mesh_group() {
    let l = Localizer::new("en".parse().unwrap());
    let input = MetadataInput {
        mesh: Some(MeshFacts {
            format: "glTF".into(),
            triangles: 12_345,
            vertices: 6_000,
            size: [10.0, 2.5, 0.125],
            unit: Some("mm".into()),
            up: Some("Y".into()),
            materials: vec!["Steel".into(), "Glass".into()],
            textures: 3,
            textures_missing: 1,
            title: None,
            generator: Some("Blender".into()),
        }),
        ..Default::default()
    };
    let r = read(&input, &l);
    let g = &r.summary[0];
    assert_eq!(g.key, "mesh");
    let item = |k: &str| strip_isolates(&g.items.iter().find(|i| i.key == k).unwrap().value);
    assert_eq!(item("triangles"), "12,345");
    assert_eq!(item("size"), "10 × 2.5 × 0.125 mm");
    assert_eq!(item("materials"), "Steel, Glass");
    assert_eq!(item("textures"), "3 (1 missing)");
    assert_eq!(item("generator"), "Blender");
}

/// Gaussian splats: format, count shown of the file's, harmonics, box and up axis.
#[test]
fn splats_group() {
    let l = Localizer::new("en".parse().unwrap());
    let input = MetadataInput {
        splats: Some(SplatFacts {
            format: "SPZ".into(),
            total: 1_200_000,
            shown: 600_000,
            sh_degree: 3,
            size: Some([2.0, 1.5, 0.25]),
            up: Some("Y".into()),
            antialiased: true,
        }),
        ..Default::default()
    };
    let r = read(&input, &l);
    let g = &r.summary[0];
    assert_eq!(g.key, "splats");
    assert_eq!(strip_isolates(&g.title), "Gaussian splats");
    let item = |k: &str| strip_isolates(&g.items.iter().find(|i| i.key == k).unwrap().value);
    assert_eq!(item("format"), "SPZ, trained with anti-aliasing");
    assert_eq!(item("count"), "600,000 of 1,200,000");
    assert_eq!(item("harmonics"), "3");
    assert_eq!(item("size"), "2 × 1.5 × 0.25");
    assert_eq!(item("up"), "Y");
}

/// A reflectance transformation image: container, basis and order, planes and mapping.
#[test]
fn rti_group() {
    let l = Localizer::new("en".parse().unwrap());
    let input = MetadataInput {
        rti: Some(RtiFacts {
            format: "RTI (HSH)".into(),
            basis: "HSH".into(),
            order: Some(3),
            terms: 9,
            colour: "RGB".into(),
            planes: 27,
            width: 4933,
            height: 2203,
            compression: None,
            scale: vec![1.5, 0.25],
            bias: vec![-0.125, 0.0],
        }),
        ..Default::default()
    };
    let r = read(&input, &l);
    let g = &r.summary[0];
    assert_eq!(g.key, "rti");
    assert_eq!(strip_isolates(&g.title), "Reflectance transformation");
    let item = |k: &str| strip_isolates(&g.items.iter().find(|i| i.key == k).unwrap().value);
    assert_eq!(item("basis"), "HSH");
    assert_eq!(item("order"), "3");
    assert_eq!(item("terms"), "9");
    assert_eq!(item("planes"), "27");
    assert_eq!(item("size"), "4,933 × 2,203");
    assert_eq!(item("scale"), "1.5, 0.25");
    assert_eq!(item("bias"), "-0.125, 0");
    assert!(g.items.iter().all(|i| i.key != "compression"));
}

/// A molecular structure: entry, method and resolution, counts, ligands, cell and symmetry.
#[test]
fn structure_group() {
    let l = Localizer::new("en".parse().unwrap());
    let input = MetadataInput {
        structure: Some(StructureFacts {
            id: Some("1ABC".into()),
            title: Some("A protein with ligands".into()),
            method: Some("X-RAY DIFFRACTION".into()),
            resolution: Some(1.5),
            chains: 2,
            residues: 312,
            atoms: 96,
            asymmetric_atoms: 12,
            bonds: 100,
            models: 20,
            records: 1,
            ligands: (0..14).map(|i| format!("L{i:02}")).collect(),
            cell: Some([5.4307, 5.4307, 5.4307, 90.0, 90.0, 90.0]),
            space_group: Some("F d -3 m".into()),
            secondary: Some(StructureSecondary::Computed),
            ..Default::default()
        }),
        ..Default::default()
    };
    let r = read(&input, &l);
    let g = &r.summary[0];
    assert_eq!(g.key, "structure");
    assert_eq!(strip_isolates(&g.title), "Structure");
    let item = |k: &str| strip_isolates(&g.items.iter().find(|i| i.key == k).unwrap().value);
    assert_eq!(item("id"), "1ABC");
    assert_eq!(item("method"), "X-RAY DIFFRACTION");
    assert_eq!(item("resolution"), "1.5 Å");
    assert_eq!(item("chains"), "2");
    assert_eq!(item("residues"), "312");
    assert_eq!(item("atoms"), "96 (12 in the asymmetric unit)");
    assert_eq!(item("models"), "20 (the first shown)");
    assert_eq!(
        item("ligands"),
        "L00, L01, L02, L03, L04, L05, L06, L07, L08, L09, L10, L11 and 2 more"
    );
    assert_eq!(
        item("cell"),
        "a = 5.431 Å, b = 5.431 Å, c = 5.431 Å, α = 90°, β = 90°, γ = 90°"
    );
    assert_eq!(item("space_group"), "F d -3 m");
    assert_eq!(item("secondary"), "Computed from the backbone");
    // One record, no formula: not shown.
    assert!(
        g.items
            .iter()
            .all(|i| i.key != "records" && i.key != "formula")
    );
}

/// A point cloud: format, counts (thinned), CRS, attributes, classes by name and E57 scans.
#[test]
fn points_group() {
    let l = Localizer::new("en".parse().unwrap());
    let input = MetadataInput {
        points: Some(PointsFacts {
            format: "LAZ 1.4 (COPC)".into(),
            point_format: Some(7),
            coding: Some("LASzip 3.4 r3, layered".into()),
            total: 120_000_000,
            loaded: 32_000_000,
            size: Some([1000.0, 800.5, 75.25]),
            crs: Some("NAD83(HARN) / Oregon GIC Lambert (ft)".into()),
            channels: vec!["rgb", "intensity", "classification"],
            classes: vec![(2, 5_000), (6, 1_200), (64, 3)],
            scans: vec![(Some("Station 1".into()), 10), (None, 20)],
        }),
        ..Default::default()
    };
    let r = read(&input, &l);
    let g = &r.summary[0];
    assert_eq!(g.key, "points");
    assert_eq!(strip_isolates(&g.title), "Point cloud");
    let item = |k: &str| strip_isolates(&g.items.iter().find(|i| i.key == k).unwrap().value);
    assert_eq!(item("count"), "32,000,000 of 120,000,000");
    assert_eq!(item("point-format"), "7");
    assert_eq!(item("size"), "1,000 × 800.5 × 75.25");
    assert_eq!(item("crs"), "NAD83(HARN) / Oregon GIC Lambert (ft)");
    assert_eq!(item("channels"), "RGB, Intensity, Classification");
    assert_eq!(
        item("classes"),
        "Ground (5,000), Building (1,200), Class 64 (3)"
    );
    assert_eq!(item("scans"), "Station 1 (10), 2 (20)");
}

#[test]
fn diffraction_group() {
    let l = Localizer::new("en".parse().unwrap());
    let input = MetadataInput {
        science: Some(ScienceFacts {
            format: "CBF".into(),
            domain: ScienceDomain::Diffraction,
            detector: Some("PILATUS 6M".into()),
            dims: vec![2463, 2527],
            pixel_size: Some((172e-6, 172e-6, "m".into())),
            diffraction: Some(DiffractionFacts {
                wavelength: Some(0.97625),
                energy: Some(12.7),
                distance: Some(250.0),
                beam: Some((1231.5, 1263.5)),
                omega: Some(0.0),
                oscillation: Some((12.5, 0.1, Some("phi".into()))),
                overload: Some(1_048_500.0),
                resolution: Some((2.53, 1.79)),
                complete: true,
                binning: Some((2, 2)),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let r = read(&input, &l);
    let g = &r.summary[0];
    assert_eq!(strip_isolates(&g.title), "Diffraction");
    let item = |k: &str| strip_isolates(&g.items.iter().find(|i| i.key == k).unwrap().value);
    assert_eq!(item("detector"), "PILATUS 6M");
    assert_eq!(item("wavelength"), "0.97625 Å");
    assert_eq!(item("distance"), "250 mm");
    assert_eq!(item("beam"), "x 1,231.5, y 1,263.5 px");
    assert_eq!(item("omega"), "0°");
    assert_eq!(item("oscillation"), "12.5° + 0.1° about phi");
    assert_eq!(item("overload"), "1,048,500");
    assert_eq!(
        item("resolution"),
        "2.53 Å at the edge, 1.79 Å in the corners"
    );
    assert_eq!(item("pixel_size"), "0.172 mm per pixel");
    assert_eq!(item("binning"), "2 × 2");
    assert!(!g.items.iter().any(|i| i.key == "geometry"));

    let mut input = input;
    if let Some(s) = input.science.as_mut() {
        s.diffraction = Some(DiffractionFacts::default());
    }
    let r = read(&input, &l);
    let g = &r.summary[0];
    let geometry = g.items.iter().find(|i| i.key == "geometry").unwrap();
    assert!(strip_isolates(&geometry.value).starts_with("Incomplete"));
}

/// A FLIR thermal still: the Thermal group with camera, object parameters and ranges, the
/// temperatures in the unit given.
#[test]
fn thermal_group() {
    let l = Localizer::new("en".parse().unwrap());
    let input = MetadataInput {
        science: Some(ScienceFacts {
            format: "FLIR radiometric JPEG".into(),
            domain: ScienceDomain::Thermal,
            date_obs: Some("2023-11-14 23:13:20".into()),
            dims: vec![640, 480],
            thermal: Some(ThermalFacts {
                format: "FLIR radiometric JPEG".into(),
                camera: Some("FLIR T640".into()),
                lens: Some("FOL41".into()),
                emissivity: Some(0.95),
                distance: Some(2.5),
                reflected: Some(68.0),
                atmospheric: Some(71.6),
                humidity: Some(0.5),
                raw_range: Some((13020, 17850)),
                temperature_range: Some((64.4, 98.6)),
                unit: "°F".into(),
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let r = read(&input, &l);
    let g = &r.summary[0];
    assert_eq!(g.key, "science");
    assert_eq!(strip_isolates(&g.title), "Thermal image");
    let item = |k: &str| strip_isolates(&g.items.iter().find(|i| i.key == k).unwrap().value);
    assert_eq!(item("camera"), "FLIR T640");
    assert_eq!(item("lens"), "FOL41");
    assert_eq!(item("emissivity"), "0.95");
    assert_eq!(item("object_distance"), "2.5 m");
    assert_eq!(item("reflected"), "68 °F");
    assert_eq!(item("atmospheric"), "71.6 °F");
    assert_eq!(item("humidity"), "50%");
    assert_eq!(item("raw_range"), "13,020 – 17,850");
    assert_eq!(item("temperature_range"), "64.4 °F – 98.6 °F");
    assert_eq!(item("dims"), "640 × 480");
}

/// Spectroscopy camera frames (SPE, SIF) get a group of their own name, not "Science".
#[test]
fn spectroscopy_group_title() {
    let l = Localizer::new("en".parse().unwrap());
    let input = MetadataInput {
        science: Some(ScienceFacts {
            format: "Princeton Instruments SPE".into(),
            domain: ScienceDomain::Spectroscopy,
            dims: vec![1340, 100],
            ..Default::default()
        }),
        ..Default::default()
    };
    let r = read(&input, &l);
    let g = &r.summary[0];
    assert_eq!(g.key, "science");
    assert_eq!(strip_isolates(&g.title), "Spectroscopy");
}

#[test]
fn ugrid_group() {
    let l = Localizer::new("en".parse().unwrap());
    let input = MetadataInput {
        science: Some(ScienceFacts {
            format: "NetCDF".into(),
            ugrid: Some(UgridFacts {
                name: "mesh2d".into(),
                convention: "UGRID".into(),
                nodes: 12_345,
                faces: 20_000,
                edges: 32_000,
                triangles: 19_000,
                quads: 999,
                polygons: 1,
                max_face_nodes: 6,
                geographic: false,
                unit: Some("m".into()),
                on_faces: Some(false),
                split_faces: 0,
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let r = read(&input, &l);
    let g = r
        .summary
        .iter()
        .find(|g| strip_isolates(&g.title) == "Unstructured mesh")
        .expect("the mesh group");
    let item = |k: &str| strip_isolates(&g.items.iter().find(|i| i.key == k).unwrap().value);
    assert_eq!(item("ugrid_name"), "mesh2d");
    assert_eq!(item("ugrid_convention"), "UGRID");
    assert_eq!(item("ugrid_nodes"), "12,345");
    assert_eq!(item("ugrid_faces"), "20,000");
    assert_eq!(item("ugrid_edges"), "32,000");
    assert_eq!(
        item("ugrid_shapes"),
        "19,000 triangles, 999 quadrilaterals, 1 polygon of 6 nodes"
    );
    assert_eq!(item("ugrid_coordinates"), "Projected x and y (m)");
    assert_eq!(
        item("ugrid_values"),
        "Nodes (interpolated across the faces)"
    );
    assert!(!g.items.iter().any(|i| i.key == "ugrid_split"));

    let mut input = input;
    if let Some(u) = input.science.as_mut().and_then(|s| s.ugrid.as_mut()) {
        u.geographic = true;
        u.on_faces = None;
        u.split_faces = 1;
    }
    let r = read(&input, &l);
    let g = r
        .summary
        .iter()
        .find(|g| strip_isolates(&g.title) == "Unstructured mesh")
        .unwrap();
    let item = |k: &str| strip_isolates(&g.items.iter().find(|i| i.key == k).unwrap().value);
    assert_eq!(item("ugrid_coordinates"), "Longitude and latitude");
    assert_eq!(
        item("ugrid_values"),
        "None: the image shows the nodes per face"
    );
    assert_eq!(item("ugrid_split"), "1 face at the antimeridian or a pole");
}

/// Neuroimaging: a GIFTI data file on the surface found beside it, a CIFTI file missing a
/// surface, a subsampled tractogram.
#[test]
fn neuro_groups() {
    let l = Localizer::new("en".parse().unwrap());
    let input = MetadataInput {
        neuro: Some(NeuroFacts {
            gifti: Some(GiftiFacts {
                version: "1.0".into(),
                arrays: 3,
                role: "data".into(),
                vertices: Some(32492),
                encoding: "GZipBase64Binary".into(),
                array: Some("thickness".into()),
                intent: Some("NIFTI_INTENT_SHAPE".into()),
                data_type: Some("NIFTI_TYPE_FLOAT32".into()),
                transform: Some(("scanner".into(), "MNI 152".into(), true)),
                surface: Some("100307.L.midthickness.32k_fs_LR.surf.gii".into()),
                ..Default::default()
            }),
            cifti: Some(CiftiFacts {
                version: "2".into(),
                intent: "ConnDenseSeries".into(),
                shape: [1200, 91282],
                map_kind: "series".into(),
                maps: 1200,
                series: Some((0.0, 0.72, "SECOND".into())),
                models: 21,
                voxels: 31870,
                structures: vec!["CORTEX_LEFT".into(), "CORTEX_RIGHT".into()],
                surfaces: vec![
                    ("CORTEX_LEFT".into(), 32492, Some("L.surf.gii".into())),
                    ("CORTEX_RIGHT".into(), 32492, None),
                ],
                shown: "surfaces".into(),
                ..Default::default()
            }),
            tracts: Some(TractFacts {
                format: "TrackVis 2".into(),
                streamlines: 200_000,
                points: 12_000_000,
                shown_streamlines: 16_666,
                scalars: vec!["fa".into()],
                dims: Some([96, 96, 60]),
                voxel_size: Some([2.0, 2.0, 2.5]),
                voxel_order: Some("LAS".into()),
                space: "vox_to_ras".into(),
                mean_length: Some(61.25),
                ..Default::default()
            }),
        }),
        ..Default::default()
    };
    let r = read(&input, &l);
    let keys: Vec<&str> = r.summary.iter().map(|g| g.key).collect();
    assert_eq!(&keys[..3], ["gifti", "cifti", "tractography"]);
    let item = |g: usize, k: &str| {
        strip_isolates(
            &r.summary[g]
                .items
                .iter()
                .find(|i| i.key == k)
                .unwrap_or_else(|| panic!("{k}"))
                .value,
        )
    };
    assert_eq!(item(0, "gifti_role"), "Values per vertex");
    assert_eq!(item(0, "gifti_vertices"), "32,492");
    assert_eq!(item(0, "gifti_transform"), "scanner → MNI 152 (applied)");
    assert_eq!(
        item(0, "gifti_surface"),
        "100307.L.midthickness.32k_fs_LR.surf.gii"
    );
    assert_eq!(item(1, "cifti_matrix"), "1,200 × 91,282");
    assert_eq!(item(1, "cifti_maps"), "1,200 time points");
    assert_eq!(item(1, "cifti_series"), "from 0, every 0.72 (second)");
    let surfaces: Vec<String> = r.summary[1]
        .items
        .iter()
        .filter(|i| i.key == "cifti_surface")
        .map(|i| strip_isolates(&i.value))
        .collect();
    assert_eq!(
        surfaces,
        [
            "CORTEX_LEFT: 32,492 vertices, L.surf.gii",
            "CORTEX_RIGHT: needs a surface with 32,492 vertices (none beside the file)"
        ]
    );
    assert_eq!(item(2, "tract_streamlines"), "200,000 (16,666 shown)");
    assert_eq!(item(2, "tract_mean_length"), "61.3 mm");
    assert_eq!(item(2, "tract_voxel_size"), "2 × 2 × 2.5 mm");
    assert_eq!(item(2, "tract_space"), "From the voxel-to-RAS matrix");
}

/// A dense connectivity file says how many seeds it has and what a pick reads, in place of
/// the maps.
#[test]
fn cifti_dense_connectivity() {
    let l = Localizer::new("en".parse().unwrap());
    let input = MetadataInput {
        neuro: Some(NeuroFacts {
            cifti: Some(CiftiFacts {
                version: "2".into(),
                intent: "ConnDense".into(),
                shape: [91282, 91282],
                map_kind: "brain-models".into(),
                maps: 91282,
                models: 21,
                voxels: 31870,
                shown: "surfaces".into(),
                dense: Some((91282, 365_128)),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let r = read(&input, &l);
    let g = r.summary.iter().find(|g| g.key == "cifti").unwrap();
    let item = |k: &str| {
        g.items
            .iter()
            .find(|i| i.key == k)
            .map(|i| strip_isolates(&i.value))
    };
    assert_eq!(item("cifti_maps"), None);
    assert_eq!(
        item("cifti_dense").as_deref(),
        Some("91,282 seeds, 365.1 KB read per seed")
    );
    assert_eq!(
        item("cifti_shown").as_deref(),
        Some("Connectivity of the picked seed")
    );
}

/// A film scan's header facts: the known keys as a labelled group (unknown keys dropped),
/// every field under "DPX header".
#[test]
fn header_facts_make_a_group_and_a_field_list() {
    let l = Localizer::new("en".parse().unwrap());
    let input = MetadataInput {
        header: Some(HeaderFacts {
            format: "DPX".into(),
            summary: vec![
                ("timecode", "01:00:00:12".into()),
                ("edge-code", "KK 12 34 567890 +12".into()),
                ("not-a-key", "dropped".into()),
            ],
            fields: vec![("Creator".into(), "scanner".into())],
        }),
        ..Default::default()
    };
    let r = read(&input, &l);
    let g = r.summary.iter().find(|g| g.key == "header").unwrap();
    assert_eq!(g.title, "File header");
    let labels: Vec<_> = g
        .items
        .iter()
        .map(|i| (i.label.as_str(), i.value.as_str()))
        .collect();
    assert_eq!(
        labels,
        [
            ("Time code", "01:00:00:12"),
            ("Edge code", "KK 12 34 567890 +12")
        ]
    );
    let f = r
        .all_fields
        .iter()
        .find(|g| g.title.contains("DPX"))
        .unwrap();
    assert_eq!(f.fields[0].tag, "Creator");
}

/// A fact given twice (a film scan's format and slate) is one row, its values joined by the
/// caller's join (the app's, for its UI direction); plainly without one.
#[test]
fn header_values_join_through_the_callers_join() {
    let input = MetadataInput {
        header: Some(HeaderFacts {
            format: "DPX".into(),
            summary: vec![
                ("film", "Academy".into()),
                ("timecode", "01:00:00:12".into()),
                ("film", "Scene 4 take 2".into()),
            ],
            fields: Vec::new(),
        }),
        ..Default::default()
    };
    let rows = |l: &Localizer| {
        let r = read(&input, l);
        let g = r.summary.iter().find(|g| g.key == "header").unwrap();
        g.items.iter().map(|i| i.value.clone()).collect::<Vec<_>>()
    };
    let plain = Localizer::new("he".parse().unwrap());
    assert_eq!(rows(&plain), ["Academy · Scene 4 take 2", "01:00:00:12"]);
    let joined = Localizer::new("he".parse().unwrap()).with_join(std::sync::Arc::new(
        |parts: &[&str], sep: &str| format!("<{}>", parts.join(&format!("|{sep}|"))),
    ));
    assert_eq!(
        rows(&joined),
        ["<Academy| · |Scene 4 take 2>", "01:00:00:12"]
    );
}

/// A Photoshop file's artboards and what its composite leaves out, in the header group.
#[test]
fn photoshop_notes_in_the_header_group() {
    let l = Localizer::new("en".parse().unwrap());
    let input = MetadataInput {
        header: Some(HeaderFacts {
            format: "PSD".into(),
            summary: vec![
                ("artboards", "Phone (375 × 667)".into()),
                ("not-drawn", "Color Lookup".into()),
                ("approximated", "Bevel & Emboss".into()),
            ],
            fields: Vec::new(),
        }),
        ..Default::default()
    };
    let r = read(&input, &l);
    let g = r.summary.iter().find(|g| g.key == "header").unwrap();
    let labels: Vec<_> = g.items.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(labels, ["Artboards", "Not drawn", "Drawn approximately"]);
}

/// An office document's group: kind with its qualifiers, the preview (or its absence),
/// properties, dates and counts, in English.
#[test]
fn office_document_group() {
    let l = Localizer::new("en".parse().unwrap());
    let d = DocumentFacts {
        kind: "word".into(),
        template: true,
        macros: true,
        preview: None,
        title: Some("Plan".into()),
        author: Some("Ann".into()),
        created: Some("2024-01-02T03:04:05Z".into()),
        pages: Some(3),
        words: Some(1234),
        edit_minutes: Some(25),
        application: Some("Microsoft Office Word 16.0000".into()),
        ..Default::default()
    };
    let r = read(
        &MetadataInput {
            document: Some(d),
            ..Default::default()
        },
        &l,
    );
    let g = r
        .summary
        .iter()
        .find(|g| g.key == "document")
        .expect("group");
    assert_eq!(r.summary[0].key, "document");
    let value = |key: &str| {
        g.items
            .iter()
            .find(|i| i.key == key)
            .map(|i| strip_isolates(&i.value))
    };
    assert_eq!(
        value("kind").as_deref(),
        Some("Word document, template, with macros")
    );
    assert_eq!(value("preview").as_deref(), Some("None inside this file"));
    assert_eq!(value("title").as_deref(), Some("Plan"));
    assert_eq!(value("author").as_deref(), Some("Ann"));
    assert_eq!(value("pages").as_deref(), Some("3"));
    assert_eq!(value("words").as_deref(), Some("1,234"));
    assert_eq!(value("editing-time").as_deref(), Some("25 min"));
    assert!(value("created").is_some_and(|c| c.contains("2024")));
    assert_eq!(value("slides"), None);
}
