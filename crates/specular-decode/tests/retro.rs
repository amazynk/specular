//! Retro formats through the registry: chosen by signature or extension, shared extensions
//! told apart by content, and plain refusals for look-alikes.

use specular_core::{CancelToken, Source};
use specular_decode::{DecodeRequest, Format, Registry};
use std::sync::Arc;

fn open(reg: &Registry, name: &str, bytes: Vec<u8>) -> specular_decode::Result<Format> {
    let s = reg.open(Arc::new(Source::from_vec(name, bytes)))?;
    let r = s.decode(&DecodeRequest::full(1), &CancelToken::new())?;
    assert_eq!((r.width, r.height), (s.info().width, s.info().height));
    Ok(s.info().format)
}

fn softimage(w: u16, h: u16) -> Vec<u8> {
    let mut f = vec![0x53, 0x80, 0xf6, 0x34];
    f.extend_from_slice(&3.71f32.to_be_bytes());
    f.extend_from_slice(&[0u8; 80]);
    f.extend_from_slice(b"PICT");
    f.extend_from_slice(&w.to_be_bytes());
    f.extend_from_slice(&h.to_be_bytes());
    f.extend_from_slice(&1.0f32.to_be_bytes());
    f.extend_from_slice(&[0, 3, 0, 0, 0, 8, 0, 0xe0]);
    f.extend(vec![200u8; w as usize * h as usize * 3]);
    f
}

#[test]
fn by_signature_and_extension() {
    let reg = Registry::new();
    // Sixel, by content under any name.
    let six = b"\x1bPq#1;2;100;0;0#1~~\x1b\\".to_vec();
    assert_eq!(open(&reg, "x.six", six.clone()).unwrap(), Format::Sixel);
    assert_eq!(open(&reg, "noext", six).unwrap(), Format::Sixel);
    // `.pic`: Softimage by its magic, Radiance by its own.
    assert_eq!(
        open(&reg, "a.pic", softimage(3, 2)).unwrap(),
        Format::SoftimagePic
    );
    // `.scr`: a ZX Spectrum screen only at its exact size; a screen saver is refused.
    assert_eq!(
        open(&reg, "a.scr", (0..6912).map(|i| (i % 251) as u8).collect()).unwrap(),
        Format::ZxScreen
    );
    let mut exe = b"MZ\x90\0".to_vec();
    exe.resize(9000, 0);
    assert!(open(&reg, "saver.scr", exe).is_err());
    // AVS: by extension, its size must match; an AviSynth script is refused.
    let mut avs = vec![0, 0, 0, 2, 0, 0, 0, 2];
    avs.extend(vec![255u8; 16]);
    assert_eq!(open(&reg, "a.avs", avs).unwrap(), Format::Avs);
    assert!(open(&reg, "script.avs", b"AviSource(\"a.avi\")\n".to_vec()).is_err());
    // Koala and ZX by size, DEGAS by size and resolution word.
    let mut koala = vec![0, 0x60];
    koala.resize(10003, 0x1b);
    assert_eq!(open(&reg, "a.koa", koala).unwrap(), Format::Koala);
    let mut pi1 = vec![0u8; 34];
    pi1.resize(32034, 0xaa);
    assert_eq!(open(&reg, "a.pi1", pi1).unwrap(), Format::Degas);
}

#[test]
fn gem_img_by_content() {
    let reg = Registry::new();
    // 16×2, 1 plane: two solid black lines.
    let mut gem: Vec<u8> = [1u16, 8, 1, 2, 85, 85, 16, 2]
        .iter()
        .flat_map(|w| w.to_be_bytes())
        .collect();
    gem.extend_from_slice(&[0x82, 0x82]);
    assert_eq!(open(&reg, "pic.img", gem).unwrap(), Format::GemImg);
    // An unknown `.img` still gets the "not one of" refusal.
    let e = open(&reg, "disk.img", vec![0xeb; 4096]).unwrap_err();
    assert!(e.to_string().contains("GEM"), "{e}");
}

#[test]
fn qtif_raw_and_refused_codec() {
    let reg = Registry::new();
    let atom = |kind: &[u8], body: &[u8]| {
        let mut a = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        a.extend_from_slice(kind);
        a.extend_from_slice(body);
        a
    };
    let mut desc = vec![0u8; 86];
    desc[4..8].copy_from_slice(b"raw ");
    desc[32..34].copy_from_slice(&2u16.to_be_bytes());
    desc[34..36].copy_from_slice(&1u16.to_be_bytes());
    desc[82..84].copy_from_slice(&24u16.to_be_bytes());
    let mut f = atom(b"idsc", &desc);
    f.extend(atom(b"idat", &[1, 2, 3, 4, 5, 6]));
    assert_eq!(open(&reg, "a.qtif", f).unwrap(), Format::Qtif);
    desc[4..8].copy_from_slice(b"rpza");
    let mut f = atom(b"idsc", &desc);
    f.extend(atom(b"idat", &[0; 16]));
    let e = open(&reg, "b.qtif", f).unwrap_err();
    assert!(e.to_string().contains("rpza"), "{e}");
}

#[test]
fn every_new_extension_is_claimed() {
    let reg = Registry::new();
    for ext in [
        "six", "sixel", "qtif", "qif", "avs", "pntg", "mac", "pi1", "pc3", "neo", "koa", "kla",
        "scr", "cut", "pic", "pix", "rla", "rpf", "rle", "wal", "blp", "mpo", "dem", "off",
    ] {
        assert!(
            reg.supports_extension(std::path::Path::new(&format!("a.{ext}"))),
            ".{ext}"
        );
    }
}
