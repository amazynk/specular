//! GIMP XCF through the registry: a minimal hand-written file (one RGBA layer over a smaller
//! offset layer), plain and gzipped, by extension and by content. The compositing itself is
//! tested in specular-xcf against GIMP.

use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use specular_core::{CancelToken, PixelLayout, Source};
use specular_decode::{DecodeError, DecodeRequest, Format, PageLabel, Registry, Session};

#[path = "common/trim.rs"]
mod trim;

const PROP_END: u32 = 0;
const PROP_OPACITY: u32 = 6;
const PROP_VISIBLE: u32 = 8;
const PROP_OFFSETS: u32 = 15;
const PROP_COMPRESSION: u32 = 17;

struct W(Vec<u8>);

impl W {
    fn u32(&mut self, v: u32) {
        self.0.extend(v.to_be_bytes());
    }
    fn prop(&mut self, ty: u32, body: &[u8]) {
        self.u32(ty);
        self.u32(body.len() as u32);
        self.0.extend(body);
    }
    fn pos(&self) -> u32 {
        self.0.len() as u32
    }
    fn patch(&mut self, at: u32, v: u32) {
        self.0[at as usize..at as usize + 4].copy_from_slice(&v.to_be_bytes());
    }
}

/// One RGBA layer: name, x, y, width, height, pixels.
type Layer<'a> = (&'a str, i32, i32, u32, u32, Vec<u8>);

/// An uncompressed v1 XCF, `layers` top first.
fn xcf(width: u32, height: u32, layers: &[Layer]) -> Vec<u8> {
    xcf_hiding(width, height, layers, &[])
}

/// [`xcf`] with the layers at `hidden` (indices into `layers`) hidden.
fn xcf_hiding(width: u32, height: u32, layers: &[Layer], hidden: &[usize]) -> Vec<u8> {
    let mut w = W(b"gimp xcf v001\0".to_vec());
    w.u32(width);
    w.u32(height);
    w.u32(0); // RGB
    w.prop(PROP_COMPRESSION, &[0]);
    w.prop(PROP_END, &[]);
    let list = w.pos();
    for _ in 0..=layers.len() + 1 {
        w.u32(0); // layer pointers, 0, no channels, 0
    }
    for (i, (name, x, y, lw, lh, px)) in layers.iter().enumerate() {
        let at = w.pos();
        w.patch(list + 4 * i as u32, at);
        w.u32(*lw);
        w.u32(*lh);
        w.u32(1); // RGBA
        w.u32(name.len() as u32 + 1);
        w.0.extend(name.as_bytes());
        w.0.push(0);
        w.prop(PROP_OPACITY, &255u32.to_be_bytes());
        w.prop(PROP_VISIBLE, &u32::from(!hidden.contains(&i)).to_be_bytes());
        let mut off = x.to_be_bytes().to_vec();
        off.extend(y.to_be_bytes());
        w.prop(PROP_OFFSETS, &off);
        w.prop(PROP_END, &[]);
        let hier = w.pos() + 8;
        w.u32(hier);
        w.u32(0); // no mask
        w.u32(*lw);
        w.u32(*lh);
        w.u32(4);
        let level = w.pos() + 8;
        w.u32(level);
        w.u32(0);
        w.u32(*lw);
        w.u32(*lh);
        // One tile (layers here are at most 64×64).
        let tile = w.pos() + 8;
        w.u32(tile);
        w.u32(0);
        w.0.extend(px);
    }
    w.0
}

fn sample() -> Vec<u8> {
    let (bw, bh) = (12u32, 7u32);
    let back: Vec<u8> = (0..bw * bh)
        .flat_map(|i| [(i * 3) as u8, 100, 200, 255])
        .collect();
    let top: Vec<u8> = (0..4 * 3).flat_map(|_| [255, 0, 0, 255]).collect();
    xcf(
        bw,
        bh,
        &[
            ("Red patch", 2, 1, 4, 3, top),
            ("Background", 0, 0, bw, bh, back),
        ],
    )
}

fn open(name: &str, bytes: Vec<u8>) -> Arc<dyn Session> {
    Registry::default()
        .open(Arc::new(Source::from_vec(name, bytes)))
        .unwrap()
}

fn px(s: &dyn Session, x: usize, y: usize) -> [u8; 4] {
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert_eq!(r.layout, PixelLayout::Rgba);
    let p = &r.planes[0];
    p.data[y * p.stride + x * 4..][..4].try_into().unwrap()
}

#[test]
fn composites_and_describes() {
    for name in ["a.xcf", "noext"] {
        let s = open(name, sample());
        let i = s.info();
        assert_eq!(i.format, Format::Xcf);
        assert_eq!((i.width, i.height, i.bit_depth), (12, 7, 8));
        assert!(
            i.coding.starts_with(
                "XCF v1, 8-bit integer, uncompressed, 2 layers: Red patch, Background"
            ),
            "{}",
            i.coding
        );
        assert_eq!(s.colour_model(), Some("RGB"));
        assert_eq!(px(&*s, 0, 0), [0, 100, 200, 255]);
        assert_eq!(px(&*s, 2, 1), [255, 0, 0, 255]);
        assert_eq!(px(&*s, 6, 1), [54, 100, 200, 255]);
        let half = s
            .decode(&DecodeRequest::full(2), &CancelToken::new())
            .unwrap();
        assert_eq!((half.width, half.height), (6, 4));
        trim::assert_trims(&*open(name, sample()), &DecodeRequest::full(1));
    }
}

#[test]
fn gzipped_by_extension_and_content() {
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(&sample()).unwrap();
    let data = gz.finish().unwrap();
    for name in ["a.xcf.gz", "a.XCFGZ", "noext"] {
        let s = open(name, data.clone());
        assert_eq!(s.info().format, Format::Xcf, "{name}");
        assert!(s.info().coding.contains(" in gzip"), "{}", s.info().coding);
        assert_eq!(px(&*s, 2, 1), [255, 0, 0, 255]);
    }
    // A gzipped SVG is still SVG.
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\"/>")
        .unwrap();
    let svgz = gz.finish().unwrap();
    assert_eq!(open("a.svgz", svgz).info().format, Format::Svg);
}

#[test]
fn extensions_and_garbage() {
    let reg = Registry::default();
    for name in [
        "a.xcf",
        "a.XCF",
        "a.xcf.gz",
        "a.xcf.bz2",
        "a.Xcf.Xz",
        "a.xcfgz",
        "a.xcfbz2",
        "a.xcfxz",
    ] {
        assert!(reg.supports_extension(Path::new(name)), "{name}");
    }
    for name in ["a.gz", "a.tar.gz", "a.bz2", "a.xz"] {
        assert!(!reg.supports_extension(Path::new(name)), "{name}");
    }
    for name in ["x.xcf", "x.xcf.gz"] {
        assert!(
            reg.open(Arc::new(Source::from_vec(name, vec![0x5a; 40])))
                .is_err(),
            "{name}"
        );
    }
    let mut cut = sample();
    cut.truncate(cut.len() - 40);
    let s = open("cut.xcf", cut);
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert!(r.truncated);
}

/// The sample with an unnamed hidden layer on top.
fn layered() -> Vec<u8> {
    let (bw, bh) = (12u32, 7u32);
    let back: Vec<u8> = (0..bw * bh)
        .flat_map(|i| [(i * 3) as u8, 100, 200, 255])
        .collect();
    let top: Vec<u8> = (0..4 * 3).flat_map(|_| [255, 0, 0, 255]).collect();
    let veil: Vec<u8> = (0..2 * 2).flat_map(|_| [0, 255, 0, 255]).collect();
    xcf_hiding(
        bw,
        bh,
        &[
            ("", 9, 4, 2, 2, veil),
            ("Red patch", 2, 1, 4, 3, top),
            ("Background", 0, 0, bw, bh, back),
        ],
        &[0],
    )
}

#[test]
fn layers_are_pages() {
    let s = open("layers.xcf", layered());
    assert_eq!(s.page_count(), 4);
    assert_eq!(
        (s.page_index(), s.page_label()),
        (0, Some(PageLabel::Merged))
    );
    // The hidden layer is not in the composite.
    assert_eq!(px(&*s, 9, 4), [171, 100, 200, 255]);

    let veil = s.open_page(1).unwrap();
    assert_eq!(
        (veil.page_index(), veil.page_label()),
        (
            1,
            Some(PageLabel::DocumentLayer {
                name: "".into(),
                index: 1,
                hidden: true
            })
        )
    );
    let i = veil.info();
    assert_eq!((i.width, i.height, i.format), (12, 7, Format::Xcf));
    assert!(i.has_alpha);
    assert!(i.coding.ends_with("; layer 1 of 3"), "{}", i.coding);
    assert!(veil.decode_memory(&DecodeRequest::full(1)) >= 12 * 7 * 4);
    assert_eq!(px(&*veil, 9, 4), [0, 255, 0, 255]);
    assert_eq!(px(&*veil, 8, 4)[3], 0);
    assert_eq!(px(&*veil, 0, 0)[3], 0);

    // Back and forth: from a layer page to another, and back to the composite.
    let red = veil.open_page(2).unwrap();
    assert_eq!(
        red.page_label(),
        Some(PageLabel::DocumentLayer {
            name: "Red patch".into(),
            index: 2,
            hidden: false
        })
    );
    assert_eq!(px(&*red, 2, 1), [255, 0, 0, 255]);
    assert_eq!(px(&*red, 0, 0)[3], 0);
    let back = red.open_page(3).unwrap();
    assert_eq!(px(&*back, 6, 1), [54, 100, 200, 255]);
    assert_eq!(px(&*back, 2, 1), [42, 100, 200, 255]);
    let merged = back.open_page(0).unwrap();
    assert_eq!(merged.page_label(), Some(PageLabel::Merged));
    assert_eq!(merged.info().coding, s.info().coding);
    assert_eq!(px(&*merged, 2, 1), [255, 0, 0, 255]);
    assert!(merged.open_page(4).is_err());
    trim::assert_trims(&*red, &DecodeRequest::full(1));

    // Cancelled before it starts.
    let c = CancelToken::new();
    c.cancel();
    assert!(matches!(
        red.open_page(1)
            .unwrap()
            .decode(&DecodeRequest::full(1), &c),
        Err(DecodeError::Cancelled)
    ));

    // Gzipped: the pages share the decompressed bytes.
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(&layered()).unwrap();
    let s = open("layers.xcf.gz", gz.finish().unwrap());
    assert_eq!(s.page_count(), 4);
    assert_eq!(px(&*s.open_page(2).unwrap(), 2, 1), [255, 0, 0, 255]);

    // A truncated layer page reads as truncated.
    let mut cut = layered();
    cut.truncate(cut.len() - 40);
    let s = open("cut.xcf", cut);
    let r = s
        .open_page(3)
        .unwrap()
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert!(r.truncated);
}
