//! Palettes, gradients, brushes, patterns, Pillow's odd rasters, CALS, SPIDER, PKM and
//! Firaxis textures through the registry: recognised, opened as the right format, drawn,
//! with Info facts and plain refusals.

use specular_core::{CancelToken, Source};
use specular_decode::{DecodeError, DecodeRequest, Format, Registry, Session};
use std::sync::Arc;

fn open(name: &str, bytes: Vec<u8>) -> specular_decode::Result<Arc<dyn Session>> {
    Registry::new().open(Arc::new(Source::from_vec(name, bytes)))
}

fn decode(s: &dyn Session) -> specular_core::Raster {
    s.decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap()
}

fn field<'a>(s: &'a dyn Session, name: &str) -> Option<&'a str> {
    s.header()?
        .fields
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

/// Opens, checks the format and decodes at the size the info says.
fn opens_as(name: &str, bytes: Vec<u8>, format: Format) -> Arc<dyn Session> {
    let s = open(name, bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(s.info().format, format, "{name}");
    let r = decode(s.as_ref());
    assert_eq!(
        (r.width, r.height),
        (s.info().width, s.info().height),
        "{name}"
    );
    s
}

fn be32(v: &[u32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_be_bytes()).collect()
}

#[test]
fn aliases_are_claimed() {
    let claims = Registry::new().extension_claims();
    assert_eq!(claims.get("pdd"), Some(&vec![Format::Psd]));
    assert_eq!(claims.get("psdt"), Some(&vec![Format::Psd]));
    assert_eq!(claims.get("krz"), Some(&vec![Format::Kra]));
}

#[test]
fn palettes_and_gradients() {
    let s = opens_as(
        "p.gpl",
        b"GIMP Palette\nName: Sea\nColumns: 4\n# c\n0 0 255 Blue\n0 255 255\n".to_vec(),
        Format::GimpPalette,
    );
    assert_eq!(field(s.as_ref(), "Name"), Some("Sea"));
    assert_eq!(field(s.as_ref(), "Colours"), Some("2"));
    assert_eq!(field(s.as_ref(), "1 Blue"), Some("#0000FF"));

    // Photoshop swatches: no signature, by extension.
    let mut aco = vec![0, 1, 0, 2];
    aco.extend_from_slice(&[0, 0, 0xFF, 0xFF, 0, 0, 0, 0, 0, 0]);
    aco.extend_from_slice(&[0, 2, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF, 0, 0]);
    let s = opens_as("s.aco", aco.clone(), Format::PhotoshopSwatches);
    assert_eq!(field(s.as_ref(), "Colours"), Some("2"));
    assert!(open("s.bin", aco).is_err());

    let mut act: Vec<u8> = (0..768).map(|i| (i % 256) as u8).collect();
    act.extend_from_slice(&[0, 16, 0xFF, 0xFF]);
    let s = opens_as("t.act", act, Format::AdobeColorTable);
    assert_eq!(field(s.as_ref(), "Colours in use"), Some("16"));
    match open("voice.act", vec![1; 4000]) {
        Err(DecodeError::Unsupported(m)) => assert!(m.contains("768"), "{m}"),
        r => panic!("{:?}", r.map(|s| s.info().format)),
    }

    let mut ase = b"ASEF\x00\x01\x00\x00".to_vec();
    ase.extend_from_slice(&1u32.to_be_bytes());
    let mut block = vec![0, 2, 0, b'R', 0, 0];
    block.extend_from_slice(b"RGB ");
    for v in [1.0f32, 0.0, 0.0] {
        block.extend_from_slice(&v.to_be_bytes());
    }
    block.extend_from_slice(&[0, 2]);
    ase.extend_from_slice(&[0, 1]);
    ase.extend_from_slice(&(block.len() as u32).to_be_bytes());
    ase.extend_from_slice(&block);
    let s = opens_as("c.ase", ase, Format::SwatchExchange);
    assert_eq!(field(s.as_ref(), "1 R"), Some("#FF0000"));

    let s = opens_as(
        "g.ggr",
        b"GIMP Gradient\nName: Fade\n1\n0 0.5 1 0 0 0 1 1 1 1 0 0 0\n".to_vec(),
        Format::GimpGradient,
    );
    assert_eq!(field(s.as_ref(), "Segments"), Some("1"));
    assert!(s.info().has_alpha);
}

fn brush(w: u32, h: u32, bytes: u32) -> Vec<u8> {
    let mut d = be32(&[29, 2, w, h, bytes]);
    d.extend_from_slice(b"GIMP");
    d.extend_from_slice(&be32(&[10]));
    d.push(0);
    d.resize(d.len() + (w * h * bytes) as usize, 128);
    d
}

#[test]
fn brushes_patterns_and_gerber() {
    let s = opens_as("b.gbr", brush(4, 3, 1), Format::GimpBrush);
    assert_eq!(field(s.as_ref(), "Spacing"), Some("10 %"));
    let mut pipe = b"Pipe\n2 ncells:2\n".to_vec();
    pipe.extend(brush(2, 2, 4));
    pipe.extend(brush(3, 1, 1));
    let s = opens_as("p.gih", pipe, Format::GimpBrushPipe);
    assert_eq!(s.page_count(), 2);
    let p1 = s.open_page(1).unwrap();
    assert_eq!((p1.info().width, p1.info().height), (3, 1));
    let mut pat = be32(&[25, 1, 2, 1, 3]);
    pat.extend_from_slice(b"GPAT\0");
    pat.extend_from_slice(&[1, 2, 3, 4, 5, 6]);
    opens_as("t.pat", pat, Format::GimpPattern);
    match open(
        "p.pat",
        b"8BPT\x00\x01\x00\x00\x00\x01rest of a photoshop pattern".to_vec(),
    ) {
        Err(DecodeError::Unsupported(m)) => assert!(m.contains("Photoshop"), "{m}"),
        r => panic!("{:?}", r.map(|s| s.info().format)),
    }
}

#[test]
fn pillow_rasters() {
    // Microsoft Paint 1.
    let mut msp = b"DanM".to_vec();
    for v in [8u16, 1, 1, 1, 1, 1, 8, 1, 0, 0] {
        msp.extend_from_slice(&v.to_le_bytes());
    }
    msp.resize(32, 0);
    let x = (0..16).fold(0u16, |x, i| {
        x ^ u16::from_le_bytes([msp[i * 2], msp[i * 2 + 1]])
    });
    msp[24..26].copy_from_slice(&x.to_le_bytes());
    msp.push(0xF0);
    let s = opens_as("a.msp", msp, Format::MsPaint);
    assert_eq!(field(s.as_ref(), "Version"), Some("1"));

    // GD 2.x true colour, under any name.
    let mut gd = vec![0xFF, 0xFE, 0, 2, 0, 1, 1];
    gd.extend_from_slice(&u32::MAX.to_be_bytes());
    gd.extend_from_slice(&be32(&[0x00FF_0000, 0x7F00_FF00]));
    let s = opens_as("pic.bin", gd, Format::Gd);
    assert!(s.info().has_alpha);
    // A Godot script named .gd is refused in plain words.
    match open(
        "player.gd",
        b"extends Node\nfunc _ready():\n    pass\n".to_vec(),
    ) {
        Err(DecodeError::Unsupported(m)) => assert!(m.contains("GD"), "{m}"),
        r => panic!("{:?}", r.map(|s| s.info().format)),
    }

    // GD2, one raw palette chunk.
    let mut gd2 = b"gd2\0".to_vec();
    for v in [2u16, 2, 1, 64, 1, 1, 1] {
        gd2.extend_from_slice(&v.to_be_bytes());
    }
    gd2.extend_from_slice(&[0, 0, 2]);
    gd2.extend_from_slice(&u32::MAX.to_be_bytes());
    gd2.extend(vec![0u8; 1024]);
    gd2.extend_from_slice(&[0, 1]);
    let s = opens_as("a.gd2", gd2, Format::Gd2);
    assert_eq!(field(s.as_ref(), "Chunk size"), Some("64"));

    // Pixar.
    let mut pxr = vec![0u8; 1024];
    pxr[..4].copy_from_slice(&[0x80, 0xE8, 0, 0]);
    for (o, v) in [(416, 1u16), (418, 2), (424, 14), (426, 2)] {
        pxr[o..o + 2].copy_from_slice(&v.to_le_bytes());
    }
    pxr.extend_from_slice(&[9; 6]);
    opens_as("a.pxr", pxr, Format::PixarPicture);

    // LabEye IM, two pages.
    let mut im = b"Image type: L image\r\nName: x\r\nImage size (x*y): 2*2\r\nFile size (no of images): 2\r\n"
        .to_vec();
    im.resize(511, 0);
    im.push(0x1A);
    im.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
    let s = opens_as("a.im", im, Format::LabEyeIm);
    assert_eq!(s.page_count(), 2);
    assert_eq!(field(s.as_ref(), "Image type"), Some("L"));

    // IM Tools: by extension.
    let mut imt = b"width 2\nheight 1\npixel n8\n\x0c".to_vec();
    imt.extend_from_slice(&[0, 255]);
    opens_as("a.imt", imt, Format::ImTools);

    // An XV thumbnail, under the original's name.
    let mut p7 = b"P7 332\n#IMGINFO:2x1 GIF\n#END_OF_COMMENTS\n2 1 255\n".to_vec();
    p7.extend_from_slice(&[0xE0, 0x03]);
    let s = opens_as("photo.jpg", p7, Format::XvThumbnail);
    assert_eq!(field(s.as_ref(), "Original"), Some("2x1 GIF"));

    // McIDAS AREA: by extension.
    let mut area = vec![0u8; 256];
    for (i, v) in [
        (1, 4i32),
        (3, 98260),
        (4, 74500),
        (8, 1),
        (9, 2),
        (10, 2),
        (13, 1),
        (33, 256),
    ] {
        area[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    area.extend_from_slice(&[0, 10, 0, 200]);
    let s = opens_as("goes.area", area, Format::McIdasArea);
    assert_eq!(field(s.as_ref(), "Time"), Some("07:45:00 UTC"));
    assert_eq!(s.info().bit_depth, 16);

    // PGX.
    let mut pgx = b"PG ML -9 2 1\n".to_vec();
    pgx.extend_from_slice(&[0x01, 0x00, 0x00, 0xFF]);
    let s = opens_as("c.pgx", pgx, Format::Pgx);
    assert!(s.info().coding.contains("signed"));
}

/// A CALS Type 1 header for `w × h`, with `rorient`.
fn cals(w: u32, h: u32, orient: &str, rtype: &str) -> Vec<u8> {
    let mut t = String::new();
    for rec in [
        "srcdocid: NONE".to_string(),
        format!("rtype: {rtype}"),
        format!("rorient: {orient}"),
        format!("rpelcnt: {w:06},{h:06}"),
        "rdensty: 0300".into(),
    ] {
        t += &format!("{rec:<128}");
    }
    let mut d = t.into_bytes();
    d.resize(2048, b' ');
    d
}

#[test]
fn cals_rasters() {
    // Eight all-white rows: each is one vertical-mode-0 code (a single 1 bit).
    let mut d = cals(16, 8, "000,270", "1");
    d.push(0xFF);
    let s = opens_as("drawing.cal", d, Format::Cals);
    assert_eq!(field(s.as_ref(), "rdensty"), Some("0300"));
    let r = decode(s.as_ref());
    assert!(r.planes[0].data.iter().all(|&v| v == 255));
    // Rotated: the orientation says so.
    let mut d = cals(16, 8, "090,270", "1");
    d.push(0xFF);
    let s = open("r.cal", d).unwrap();
    assert_eq!(s.info().orientation.0, 6);
    match open("t.cal", cals(16, 8, "000,270", "2")) {
        Err(DecodeError::Unsupported(m)) => assert!(m.contains("Type 2"), "{m}"),
        r => panic!("{:?}", r.map(|s| s.info().format)),
    }
}

#[test]
fn spider_stack() {
    // Two 4×2 images in a little-endian stack.
    let header = |stack: f32| {
        let mut h = vec![0u8; 1024];
        for (n, v) in [
            (1, 1.0f32),
            (2, 2.0),
            (5, 1.0),
            (12, 4.0),
            (13, 64.0),
            (22, 1024.0),
            (23, 16.0),
            (24, if stack > 0.0 { 2.0 } else { 0.0 }),
            (26, stack),
            (38, 1.25),
        ] {
            h[(n - 1) * 4..n * 4].copy_from_slice(&v.to_le_bytes());
        }
        h
    };
    let mut d = header(2.0);
    for k in 0..2 {
        d.extend(header(0.0));
        d.extend((0..8).flat_map(|i| (i as f32 + k as f32 * 100.0).to_le_bytes()));
    }
    let s = opens_as("particles.spi", d.clone(), Format::Spider);
    let sci = s.science().expect("science summary");
    assert_eq!(sci.axes.len(), 1);
    assert_eq!(sci.axes[0].len, 2);
    // No signature: not under another name.
    assert!(open("particles.bin", d).is_err());
}

#[test]
fn pkm_and_firaxis_textures() {
    let mut pkm = b"PKM 10".to_vec();
    for v in [0u16, 8, 4, 5, 3] {
        pkm.extend_from_slice(&v.to_be_bytes());
    }
    pkm.extend_from_slice(&[0u8; 16]);
    let s = opens_as("t.pkm", pkm, Format::Pkm);
    assert_eq!((s.info().width, s.info().height), (5, 3));
    let mut ftu = b"FTEX".to_vec();
    for v in [1u32, 1, 1, 1, 1, 1, 32] {
        ftu.extend_from_slice(&v.to_le_bytes());
    }
    ftu.extend_from_slice(&3u32.to_le_bytes());
    ftu.extend_from_slice(&[10, 20, 30]);
    opens_as("t.ftu", ftu, Format::Ftex);
}

/// The generated and fetched corpus against Pillow, texture2ddecoder and numpy
/// (tools/corpus/fetch-fmt500d.sh, then tools/corpus/fmt500d-oracle.py). Run with
/// `SPECULAR_FMT500D_CORPUS=$CARGO_TARGET_DIR/corpus/fmt500d cargo test -p specular-decode
/// --test fmt500d -- --ignored --nocapture`.
#[test]
#[ignore]
fn corpus_matches_oracles() {
    let Some(root) = std::env::var_os("SPECULAR_FMT500D_CORPUS") else {
        eprintln!("SPECULAR_FMT500D_CORPUS not set");
        return;
    };
    let root = std::path::Path::new(&root);
    let mut bad = Vec::new();
    // Rasters: our RGBA (grey widened) against the oracle's RGBA.
    for (name, format) in [
        ("gen/hopper.cal", Format::Cals),
        ("gen/hopper_10_0.pkm", Format::Pkm),
        ("gen/hopper_20_1.pkm", Format::Pkm),
        ("gen/hopper_20_3.pkm", Format::Pkm),
        ("ftex_dxt1.ftc", Format::Ftex),
        ("ftex_uncompressed.ftu", Format::Ftex),
    ] {
        let data = std::fs::read(root.join(name)).unwrap();
        let s = open(name, data).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(s.info().format, format, "{name}");
        let r = decode(s.as_ref());
        let p = &r.planes[0];
        let bpp = p.bytes_per_pixel();
        let mut ours = Vec::new();
        for y in 0..r.height as usize {
            for x in 0..r.width as usize {
                let px = &p.data[y * p.stride + x * bpp..y * p.stride + (x + 1) * bpp];
                ours.extend_from_slice(&match bpp {
                    1 => [px[0], px[0], px[0], 255],
                    _ => [px[0], px[1], px[2], px[3]],
                });
            }
        }
        let want = std::fs::read(root.join(format!("{name}.rgba"))).unwrap();
        assert_eq!(ours.len(), want.len(), "{name}");
        let (n, m) = ours.iter().zip(&want).fold((0, 0u8), |(n, m), (a, b)| {
            (n + usize::from(a != b), m.max(a.abs_diff(*b)))
        });
        println!(
            "{name}: {}×{}, {n} of {} bytes differ (max {m})",
            r.width,
            r.height,
            want.len()
        );
        // BC1's interpolated colours: Pillow rounds the thirds differently.
        let tolerance = u8::from(name.ends_with(".ftc"));
        if m > tolerance {
            bad.push(name.to_owned());
        }
    }
    // SPIDER: the stored values against Pillow's.
    {
        let name = "hopper.spider";
        let data = std::fs::read(root.join(name)).unwrap();
        let s = open(name, data.clone()).unwrap();
        assert_eq!(s.info().format, Format::Spider);
        let sp = specular_em::spider::parse(&data).unwrap();
        let mut v = vec![0f32; sp.nx * sp.ny];
        sp.read_plane(&data, 0, &mut v).unwrap();
        let want: Vec<f64> = std::fs::read(root.join(format!("{name}.f64")))
            .unwrap()
            .chunks_exact(8)
            .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
            .collect();
        let miss = v
            .iter()
            .zip(&want)
            .filter(|(a, b)| f64::from(**a) != **b)
            .count();
        println!(
            "{name}: {}×{}, {miss} of {} values differ",
            sp.nx,
            sp.ny,
            want.len()
        );
        if miss > 0 || v.len() != want.len() {
            bad.push(name.into());
        }
    }
    assert!(bad.is_empty(), "mismatches: {bad:?}");
}

/// A two-frame 2×1 RGBA sprite: one layer, a red cel, then a link to it.
fn sprite() -> Vec<u8> {
    let chunk = |kind: u16, body: &[u8]| {
        let mut c = ((body.len() + 6) as u32).to_le_bytes().to_vec();
        c.extend_from_slice(&kind.to_le_bytes());
        c.extend_from_slice(body);
        c
    };
    let mut layer = Vec::new();
    for v in [1u16, 0, 0, 0, 0, 0] {
        layer.extend_from_slice(&v.to_le_bytes());
    }
    layer.extend_from_slice(&[255, 0, 0, 0, 2, 0, b'b', b'g']);
    let mut cel = vec![0, 0, 0, 0, 0, 0, 255, 0, 0];
    cel.extend_from_slice(&[0; 7]);
    cel.extend_from_slice(&[2, 0, 1, 0, 255, 0, 0, 255, 255, 0, 0, 255]);
    let mut link = vec![0, 0, 0, 0, 0, 0, 255, 1, 0];
    link.extend_from_slice(&[0; 7]);
    link.extend_from_slice(&[0, 0]);
    let frames = [
        vec![chunk(0x2004, &layer), chunk(0x2005, &cel)],
        vec![chunk(0x2005, &link)],
    ];
    let mut d = vec![0u8; 128];
    d[4..6].copy_from_slice(&0xA5E0u16.to_le_bytes());
    d[6] = 2;
    d[8] = 2;
    d[10] = 1;
    d[12] = 32;
    d[14] = 1;
    for (k, f) in frames.iter().enumerate() {
        let body: Vec<u8> = f.concat();
        d.extend_from_slice(&((body.len() + 16) as u32).to_le_bytes());
        d.extend_from_slice(&0xF1FAu16.to_le_bytes());
        d.extend_from_slice(&(f.len() as u16).to_le_bytes());
        d.extend_from_slice(&(100 + 50 * k as u16).to_le_bytes());
        d.extend_from_slice(&[0, 0]);
        d.extend_from_slice(&(f.len() as u32).to_le_bytes());
        d.extend(body);
    }
    let n = d.len() as u32;
    d[..4].copy_from_slice(&n.to_le_bytes());
    d
}

#[test]
fn aseprite_animates_and_shares_ase() {
    let s = opens_as("walk.aseprite", sprite(), Format::Aseprite);
    assert_eq!(field(s.as_ref(), "Frames"), Some("2"));
    assert_eq!(field(s.as_ref(), "Layer bg"), Some("image, normal"));
    let a = s.animation().expect("animated");
    assert_eq!(a.frame_count, Some(2));
    let mut f = s.frames(1).unwrap();
    let delays = f.delays().unwrap();
    assert_eq!(delays[1].as_millis(), 150);
    let cancel = CancelToken::new();
    let first = f.next_frame(&cancel).unwrap().unwrap();
    let second = f.next_frame(&cancel).unwrap().unwrap();
    assert_eq!(first.raster.planes[0].data, second.raster.planes[0].data);
    assert!(f.next_frame(&cancel).is_none());
    // `.ase`: Aseprite by its magic, Swatch Exchange by "ASEF".
    opens_as("walk.ase", sprite(), Format::Aseprite);
    let mut swatches = b"ASEF\x00\x01\x00\x00".to_vec();
    swatches.extend_from_slice(&0u32.to_be_bytes());
    match open("empty.ase", swatches) {
        Err(DecodeError::Unsupported(m)) => assert!(m.contains("no colours"), "{m}"),
        r => panic!("{:?}", r.map(|s| s.info().format)),
    }
}
