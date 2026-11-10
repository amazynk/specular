//! Generated screens for every format: known pixels land where and in the colour the
//! machine shows them; wrong sizes are refused with a reason.

use specular_retro2::{Error, Hints, Image, Kind, decode, info, signature};

fn px(img: &Image, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * img.width + x) * 4) as usize;
    [img.rgba[i], img.rgba[i + 1], img.rgba[i + 2]]
}

fn hints(ext: &str) -> Hints<'_> {
    Hints { ext, palette: None }
}

fn amsdos(body: &[u8]) -> Vec<u8> {
    let mut h = vec![0u8; 128];
    h[1..9].copy_from_slice(b"PICTURE ");
    h[0x12] = 2;
    h[0x15..0x17].copy_from_slice(&0xC000u16.to_le_bytes());
    h[0x18..0x1A].copy_from_slice(&(body.len() as u16).to_le_bytes());
    h[0x40] = body.len() as u8;
    h[0x41] = (body.len() >> 8) as u8;
    let sum: u32 = h[..67].iter().map(|&b| b as u32).sum();
    h[67..69].copy_from_slice(&(sum as u16).to_le_bytes());
    h.extend_from_slice(body);
    h
}

#[test]
fn amstrad_cpc() {
    let mut scr = vec![0u8; 16384];
    scr[0] = 0x88; // mode 1: pixel 0 is pen 3 (bright red)
    scr[2048] = 0x80; // line 1: pixel 0 is pen 1 (bright yellow)
    let i = info(Kind::CpcScreen, &scr, &hints("scr"), 0).unwrap();
    assert_eq!((i.width, i.height, i.pages), (320, 200, 3));
    let img = decode(Kind::CpcScreen, &scr, &hints("scr"), 0).unwrap();
    assert_eq!(px(&img, 0, 0), [255, 0, 0]);
    assert_eq!(px(&img, 0, 1), [255, 255, 0]);
    assert_eq!(px(&img, 1, 0), [0, 0, 128]);
    assert!(!signature(Kind::CpcScreen, &scr));
    // With an AMSDOS header: the same picture, and a signature.
    let with = amsdos(&scr);
    assert!(signature(Kind::CpcScreen, &with));
    assert_eq!(
        decode(Kind::CpcScreen, &with, &hints("scr"), 0).unwrap(),
        img
    );
    // Pages 1 and 2 are modes 0 and 2.
    let m0 = decode(Kind::CpcScreen, &scr, &hints("scr"), 1).unwrap();
    assert_eq!((m0.width, m0.height), (320, 200));
    let m2 = decode(Kind::CpcScreen, &scr, &hints("scr"), 2).unwrap();
    assert_eq!((m2.width, m2.height), (640, 400));
    // An OCP .pal in mode 0 with pen 15 hardware &4C (bright red): mode 0 comes first.
    let mut pal = vec![0x54u8; 239];
    pal[0] = 0;
    pal[3 + 15 * 12] = 0x4C;
    let mut m0scr = vec![0u8; 16384];
    m0scr[0] = 0xAA; // pixel 0 is pen 15
    let h = Hints {
        ext: "scr",
        palette: Some(&pal),
    };
    let i = info(Kind::CpcScreen, &m0scr, &h, 0).unwrap();
    assert!(i.coding.contains("mode 0"), "{}", i.coding);
    let img = decode(Kind::CpcScreen, &m0scr, &h, 0).unwrap();
    assert_eq!(px(&img, 0, 0), [255, 0, 0]);
    assert_eq!(px(&img, 2, 0), [0, 0, 0]);
    // Refusals.
    assert!(matches!(
        info(Kind::CpcScreen, &[0u8; 6912], &hints("scr"), 0),
        Err(Error::Unsupported(_))
    ));
    let mut prog = amsdos(&[1u8; 300]);
    prog[0x15..0x17].copy_from_slice(&0x4000u16.to_le_bytes());
    let sum: u32 = prog[..67].iter().map(|&b| b as u32).sum();
    prog[67..69].copy_from_slice(&(sum as u16).to_le_bytes());
    match info(Kind::CpcScreen, &prog, &hints("scr"), 0) {
        Err(Error::Unsupported(s)) => assert!(s.contains("program")),
        r => panic!("{r:?}"),
    }
}

fn bsave(end: u16, body: &[u8]) -> Vec<u8> {
    let mut d = vec![0xFE, 0, 0];
    d.extend_from_slice(&end.to_le_bytes());
    d.extend_from_slice(&[0, 0]);
    d.extend_from_slice(body);
    d
}

#[test]
fn msx() {
    // Screen 5 with its palette: colour 1 is pure red.
    let mut v = vec![0u8; 0x76A0];
    v[0] = 0x12; // pixels 0, 1: colours 1, 2
    v[0x7680 + 2] = 0x70; // R=7
    v[0x7680 + 3] = 0x00;
    v[0x7680 + 5] = 0x07; // colour 2: G=7
    let f = bsave(0x769F, &v);
    assert!(signature(Kind::MsxScreen, &f));
    let i = info(Kind::MsxScreen, &f, &hints("sc5"), 0).unwrap();
    assert_eq!((i.width, i.height), (256, 212));
    assert!(i.fields.iter().any(|(_, v)| v == "saved with the screen"));
    let img = decode(Kind::MsxScreen, &f, &hints("sc5"), 0).unwrap();
    assert_eq!(px(&img, 0, 0), [255, 0, 0]);
    assert_eq!(px(&img, 1, 0), [0, 255, 0]);
    // Screen 8: GGGRRRBB.
    let mut v = vec![0u8; 0xD400];
    v[1] = 0b1110_0000;
    let img = decode(Kind::MsxScreen, &bsave(0xD3FF, &v), &hints("sc8"), 0).unwrap();
    assert_eq!(px(&img, 1, 0), [0, 255, 0]);
    // Screen 2: pattern 0 row 0 = 0x80 in white on black.
    let mut v = vec![0u8; 0x3800];
    v[0] = 0x80;
    v[0x2000] = 0xF1;
    let img = decode(Kind::MsxScreen, &bsave(0x37FF, &v), &hints("sc2"), 0).unwrap();
    assert_eq!((img.width, img.height), (256, 192));
    assert_eq!(px(&img, 0, 0), [255, 255, 255]);
    assert_eq!(px(&img, 1, 0), [0, 0, 0]);
    // Screen 7 is drawn on doubled lines; screen 12 grey from YJK.
    let i = info(Kind::MsxScreen, &vec![0u8; 0xD400], &hints("sc7"), 0).unwrap();
    assert_eq!((i.width, i.height), (512, 424));
    let v = vec![0xF8u8; 0xD400];
    let img = decode(Kind::MsxScreen, &v, &hints("scc"), 0).unwrap();
    assert_eq!(px(&img, 5, 5), [255, 255, 255]);
    // Unknown mode and compressed files are refused.
    assert!(matches!(
        info(Kind::MsxScreen, &f, &hints("sc9"), 0),
        Err(Error::Unsupported(_))
    ));
    assert!(matches!(
        info(Kind::MsxScreen, &[1u8; 5000], &hints("sc5"), 0),
        Err(Error::Unsupported(_))
    ));
}

#[test]
fn apple_ii() {
    let mut d = vec![0u8; 8192];
    d[0] = 0b0000_0011; // dots 0 and 1: white
    d[0x400] = 0b0000_0001; // line 1, dot 0 alone, palette 0: violet
    d[0x400 + 1] = 0b1000_0001; // line 1, dot 7 alone, palette 1, odd column: orange
    let img = decode(Kind::AppleHgr, &d, &hints("hgr"), 0).unwrap();
    assert_eq!((img.width, img.height), (560, 384));
    assert_eq!(px(&img, 0, 0), [255, 255, 255]);
    assert_eq!(px(&img, 3, 1), [255, 255, 255]);
    assert_eq!(px(&img, 0, 2), [255, 68, 253]);
    assert_eq!(px(&img, 14, 2), [255, 106, 60]);
    assert_eq!(px(&img, 4, 0), [0, 0, 0]);
    // Double hi-res: all dots lit is white; 16 KB named .hgr is pointed to .dhr.
    let d = vec![0x7Fu8; 16384];
    let img = decode(Kind::AppleDhr, &d, &hints("dhr"), 0).unwrap();
    assert_eq!(px(&img, 100, 100), [255, 255, 255]);
    match info(Kind::AppleHgr, &d, &hints("hgr"), 0) {
        Err(Error::Unsupported(s)) => assert!(s.contains(".dhr")),
        r => panic!("{r:?}"),
    }
}

#[test]
fn bbc_micro() {
    let mut d = vec![0u8; 20480];
    d[0] = 0x88; // mode 1: pixel 0 colour 3 (white)
    d[1] = 0x80; // line 1: pixel 0 colour 2 (yellow)
    d[8] = 0x08; // next cell, line 0: pixel 0 colour 1 (red)
    let img = decode(Kind::BbcScreen, &d, &hints("bb1"), 0).unwrap();
    assert_eq!((img.width, img.height), (640, 512));
    assert_eq!(px(&img, 0, 0), [255, 255, 255]);
    assert_eq!(px(&img, 0, 2), [255, 255, 0]);
    assert_eq!(px(&img, 8, 0), [255, 0, 0]);
    let mut d = vec![0u8; 20480];
    d[0] = 0x02 | 0x08; // mode 2: pixel 0 colour 3 (yellow)
    let img = decode(Kind::BbcScreen, &d, &hints("bb2"), 0).unwrap();
    assert_eq!(px(&img, 0, 0), [255, 255, 0]);
    assert!(matches!(
        info(Kind::BbcScreen, &d, &hints("bb4"), 0),
        Err(Error::Unsupported(_))
    ));
}

#[test]
fn atari_8bit() {
    let mut d = vec![0u8; 7684];
    d[0] = 0b0110_0000; // pixel 0: colour 1 (COLOR0), pixel 1: colour 2 (COLOR1)
    d[7680..7684].copy_from_slice(&[0x0F, 0x0A, 0x00, 0x00]);
    let i = info(Kind::AtariMic, &d, &hints("mic"), 0).unwrap();
    assert_eq!((i.width, i.height), (320, 192));
    let img = decode(Kind::AtariMic, &d, &hints("mic"), 0).unwrap();
    assert_eq!(px(&img, 0, 0), [255, 255, 255]);
    assert_eq!(px(&img, 2, 0), [170, 170, 170]);
    assert_eq!(px(&img, 4, 0), [0, 0, 0]);
    let mut g = vec![0u8; 7680];
    g[0] = 0xF0;
    let img = decode(Kind::AtariGr9, &g, &hints("gr9"), 0).unwrap();
    assert_eq!(px(&img, 0, 0), [255, 255, 255]);
    assert_eq!(px(&img, 4, 0), [0, 0, 0]);
    let img = decode(Kind::AtariGr8, &g, &hints("gr8"), 0).unwrap();
    assert_ne!(px(&img, 0, 0), px(&img, 4, 0));
    assert!(matches!(
        info(Kind::AtariGr8, &g[..4000], &hints("gr8"), 0),
        Err(Error::Unsupported(_))
    ));
}

fn photo_cd(y: u8, c1: u8, c2: u8, rotation: u8) -> Vec<u8> {
    let mut d = vec![0u8; 96 * 2048 + 768 * 512 * 3 / 2];
    d[2048..2055].copy_from_slice(b"PCD_IPI");
    d[0xE02] = rotation;
    let base = 96 * 2048;
    for py in 0..256 {
        let s = base + py * 768 * 3;
        d[s..s + 1536].fill(y);
        d[s + 1536..s + 1920].fill(c1);
        d[s + 1920..s + 2304].fill(c2);
    }
    d
}

#[test]
fn kodak_photo_cd() {
    let d = photo_cd(128, 156, 137, 0);
    assert!(signature(Kind::PhotoCd, &d));
    let i = info(Kind::PhotoCd, &d, &hints("pcd"), 0).unwrap();
    assert_eq!((i.width, i.height, i.orientation), (768, 512, 1));
    let img = decode(Kind::PhotoCd, &d, &hints("pcd"), 0).unwrap();
    assert_eq!(px(&img, 400, 300), [174, 174, 174]);
    assert!(!img.truncated);
    // Portrait scans turn; a cut-short file shows what it has.
    let d = photo_cd(10, 156, 137, 3);
    assert_eq!(
        info(Kind::PhotoCd, &d, &hints("pcd"), 0)
            .unwrap()
            .orientation,
        6
    );
    let img = decode(Kind::PhotoCd, &d[..400_000], &hints("pcd"), 0).unwrap();
    assert!(img.truncated);
    // Overview packs and point clouds are not image packs.
    let mut opa = vec![0u8; 10_000];
    opa[..7].copy_from_slice(b"PCD_OPA");
    assert!(matches!(
        info(Kind::PhotoCd, &opa, &hints("pcd"), 0),
        Err(Error::Unsupported(_))
    ));
    assert!(!signature(Kind::PhotoCd, b"# .PCD v0.7\nVERSION 0.7\n"));
}

#[test]
fn hostile_inputs_do_not_panic() {
    for kind in Kind::ALL {
        for ext in kind.extensions() {
            for len in [0usize, 1, 7, 128, 7680, 8192, 16384, 20480, 54_272] {
                let d: Vec<u8> = (0..len).map(|i| (i * 131 % 251) as u8).collect();
                let h = hints(ext);
                for page in 0..4 {
                    let _ = info(kind, &d, &h, page);
                    let _ = decode(kind, &d, &h, page);
                }
            }
        }
    }
}
