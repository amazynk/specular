//! Photoshop layers as pages: page 0 the merged image, then the pixel layers top-most first,
//! opened from the session already open (synthetic files only).

use std::sync::Arc;

use specular_core::{CancelToken, PixelLayout, Raster, Source};
use specular_decode::{DecodeError, DecodeRequest, PageLabel, Registry, Session};

fn be16(v: &mut Vec<u8>, x: u16) {
    v.extend(x.to_be_bytes());
}
fn be32(v: &mut Vec<u8>, x: u32) {
    v.extend(x.to_be_bytes());
}

/// A layer: name, `[top, left, bottom, right]`, hidden, section type (0 none, 1 open group,
/// 3 divider) and its raw channels (id, rows over the rectangle).
struct L {
    name: &'static str,
    rect: [i32; 4],
    hidden: bool,
    section: u32,
    channels: Vec<(i16, Vec<u8>)>,
    /// More additional-layer-info blocks (key, body padded to 4 bytes).
    blocks: Vec<([u8; 4], Vec<u8>)>,
}

/// An 8-bit RGB PSD with a raw composite of `rgb` and `layers` (bottom-most first, raw).
fn psd(w: u32, h: u32, rgb: [u8; 3], layers: &[L]) -> Vec<u8> {
    let mut f = b"8BPS".to_vec();
    be16(&mut f, 1);
    f.extend([0; 6]);
    be16(&mut f, 3);
    be32(&mut f, h);
    be32(&mut f, w);
    be16(&mut f, 8);
    be16(&mut f, 3);
    be32(&mut f, 0); // colour mode data
    be32(&mut f, 0); // resources
    let mut info = Vec::new();
    be16(&mut info, layers.len() as u16);
    for l in layers {
        for v in l.rect {
            be32(&mut info, v as u32);
        }
        be16(&mut info, l.channels.len() as u16);
        for (id, rows) in &l.channels {
            be16(&mut info, *id as u16);
            be32(&mut info, 2 + rows.len() as u32);
        }
        info.extend(b"8BIMnorm");
        info.extend([255, 0, if l.hidden { 2 } else { 0 }, 0]);
        let mut extra = Vec::new();
        be32(&mut extra, 0); // mask
        be32(&mut extra, 0); // blending ranges
        let mut name = vec![l.name.len() as u8];
        name.extend(l.name.as_bytes());
        name.resize(name.len().div_ceil(4) * 4, 0);
        extra.extend(name);
        if l.section != 0 {
            extra.extend(b"8BIMlsct");
            be32(&mut extra, 4);
            be32(&mut extra, l.section);
        }
        for (k, body) in &l.blocks {
            extra.extend(b"8BIM");
            extra.extend(k);
            be32(&mut extra, body.len() as u32);
            extra.extend(body);
        }
        be32(&mut info, extra.len() as u32);
        info.extend(extra);
    }
    for l in layers {
        for (_, rows) in &l.channels {
            be16(&mut info, 0);
            info.extend(rows);
        }
    }
    if info.len() % 2 == 1 {
        info.push(0);
    }
    be32(&mut f, info.len() as u32 + 8);
    be32(&mut f, info.len() as u32);
    f.extend(info);
    be32(&mut f, 0); // global mask
    be16(&mut f, 0);
    for c in rgb {
        f.extend(std::iter::repeat_n(c, (w * h) as usize));
    }
    // The last pixel's blue differs, so the merged image is not a blank placeholder.
    if let Some(b) = f.last_mut() {
        *b ^= 1;
    }
    f
}

fn solid(rect: [i32; 4], v: u8) -> Vec<u8> {
    vec![v; ((rect[2] - rect[0]) * (rect[3] - rect[1])) as usize]
}

fn file() -> Vec<u8> {
    let back = [0, 0, 6, 8];
    let sky = [1, 2, 4, 6];
    psd(
        8,
        6,
        [10, 20, 30],
        &[
            L {
                blocks: Vec::new(),
                name: "Back",
                rect: back,
                hidden: false,
                section: 0,
                channels: (0..3)
                    .map(|c| (c, solid(back, [200, 0, 0][c as usize])))
                    .collect(),
            },
            L {
                blocks: Vec::new(),
                name: "</Layer group>",
                rect: [0; 4],
                hidden: false,
                section: 3,
                channels: Vec::new(),
            },
            L {
                blocks: Vec::new(),
                name: "Sky",
                rect: sky,
                hidden: true,
                section: 0,
                channels: vec![
                    (-1, solid(sky, 128)),
                    (0, solid(sky, 0)),
                    (1, solid(sky, 0)),
                    (2, solid(sky, 255)),
                ],
            },
            L {
                blocks: Vec::new(),
                name: "Group",
                rect: [0; 4],
                hidden: false,
                section: 1,
                channels: Vec::new(),
            },
        ],
    )
}

fn open() -> Arc<dyn Session> {
    Registry::default()
        .open(Arc::new(Source::from_vec("layers.psd", file())))
        .unwrap()
}

fn full(s: &dyn Session) -> Raster {
    s.decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap()
}

fn px(r: &Raster, x: usize, y: usize) -> [u8; 4] {
    let p = &r.planes[0];
    p.data[y * p.stride + x * 4..][..4].try_into().unwrap()
}

#[test]
fn layers_are_pages_on_the_canvas() {
    let s = open();
    assert_eq!(s.page_count(), 4);
    assert_eq!(s.page_index(), 0);
    assert_eq!(s.page_label(), Some(PageLabel::Merged));
    assert_eq!(px(&full(&*s), 0, 0), [10, 20, 30, 255]);

    // Page 1: the layers composited beside the stored merged image (the hidden sky left
    // out), with transparency and no embedded preview.
    let built = s.open_page(1).unwrap();
    assert_eq!(built.page_label(), Some(PageLabel::FromLayers));
    let i = built.info();
    assert!(i.has_alpha && i.embedded_preview.is_none());
    assert!(
        i.coding.ends_with(", composited from its layers"),
        "{}",
        i.coding
    );
    assert!(built.decode_preview().is_none());
    assert_eq!(px(&full(&*built), 0, 0), [200, 0, 0, 255]);

    // Page 2: the top-most layer (inside the group), hidden in the file.
    let sky = s.open_page(2).unwrap();
    assert_eq!(sky.page_index(), 2);
    assert_eq!(
        sky.page_label(),
        Some(PageLabel::DocumentLayer {
            name: "Sky".into(),
            index: 1,
            hidden: true,
        })
    );
    let i = sky.info();
    assert_eq!((i.width, i.height), (8, 6));
    assert!(i.has_alpha && i.embedded_preview.is_none());
    assert!(i.coding.ends_with(", layer 1 of 2"), "{}", i.coding);
    assert!(sky.decode_preview().is_none());
    assert!(sky.decode_memory(&DecodeRequest::full(1)) >= 8 * 6 * 4);
    let r = full(&*sky);
    assert_eq!(r.layout, PixelLayout::Rgba);
    assert_eq!(px(&r, 2, 1), [0, 0, 255, 128]);
    assert_eq!(px(&r, 5, 3), [0, 0, 255, 128]);
    assert_eq!(px(&r, 1, 1)[3], 0);
    assert_eq!(px(&r, 6, 3)[3], 0);
    assert_eq!(px(&r, 2, 4)[3], 0);
    // Coarser levels come from the same decode.
    let half = sky
        .decode(&DecodeRequest::full(2), &CancelToken::new())
        .unwrap();
    assert_eq!((half.width, half.height), (4, 3));

    // Page 3: the background layer, opened from another page.
    let back = sky.open_page(3).unwrap();
    assert_eq!(
        back.page_label(),
        Some(PageLabel::DocumentLayer {
            name: "Back".into(),
            index: 2,
            hidden: false,
        })
    );
    let r = full(&*back);
    assert_eq!(px(&r, 0, 0), [200, 0, 0, 255]);
    assert_eq!(px(&r, 7, 5), [200, 0, 0, 255]);

    // Back to the merged image: the facts from open.
    let merged = back.open_page(0).unwrap();
    assert_eq!(merged.page_label(), Some(PageLabel::Merged));
    assert_eq!(merged.info().coding, s.info().coding);
    assert_eq!(merged.info().has_alpha, s.info().has_alpha);
    assert_eq!(px(&full(&*merged), 3, 3), [10, 20, 30, 255]);

    assert!(s.open_page(4).is_err());
}

#[test]
fn a_cancelled_layer_decode_stops() {
    let sky = open().open_page(2).unwrap();
    let c = CancelToken::new();
    c.cancel();
    assert!(matches!(
        sky.decode(&DecodeRequest::full(1), &c),
        Err(DecodeError::Cancelled)
    ));
    // The page decodes once asked again.
    assert_eq!(px(&full(&*sky), 3, 2), [0, 0, 255, 128]);
}

#[test]
fn a_file_without_pixel_layers_has_one_page() {
    let f = psd(4, 4, [1, 2, 3], &[]);
    let s = Registry::default()
        .open(Arc::new(Source::from_vec("flat.psd", f)))
        .unwrap();
    assert_eq!(s.page_count(), 1);
    assert_eq!(s.page_label(), None);
    assert!(s.open_page(1).is_err());
}

/// An `artb` block: version 16, a descriptor with the rectangle (top, left, bottom, right)
/// and a white background.
fn artb(rect: [f64; 4]) -> Vec<u8> {
    let mut v = Vec::new();
    be32(&mut v, 16);
    let id = |v: &mut Vec<u8>, k: &[u8]| {
        be32(v, if k.len() == 4 { 0 } else { k.len() as u32 });
        v.extend(k);
    };
    be32(&mut v, 0);
    id(&mut v, b"artboard");
    be32(&mut v, 2);
    id(&mut v, b"artboardRect");
    v.extend(b"Objc");
    be32(&mut v, 0);
    id(&mut v, b"classFloatRect");
    be32(&mut v, 4);
    for (k, x) in [b"Top ", b"Left", b"Btom", b"Rght"].iter().zip(rect) {
        id(&mut v, *k);
        v.extend(b"doub");
        v.extend(x.to_bits().to_be_bytes());
    }
    id(&mut v, b"artboardBackgroundType");
    v.extend(b"long");
    be32(&mut v, 1);
    v
}

#[test]
fn artboards_are_pages_after_the_merged_image() {
    let dot = [2, 3, 3, 4];
    let f = psd(
        8,
        6,
        [10, 20, 30],
        &[
            L {
                blocks: Vec::new(),
                name: "</Layer group>",
                rect: [0; 4],
                hidden: false,
                section: 3,
                channels: Vec::new(),
            },
            L {
                blocks: Vec::new(),
                name: "Dot",
                rect: dot,
                hidden: false,
                section: 0,
                channels: (0..3)
                    .map(|c| (c, solid(dot, [0, 200, 0][c as usize])))
                    .collect(),
            },
            L {
                blocks: vec![(*b"artb", artb([1.0, 2.0, 5.0, 6.0]))],
                name: "Phone",
                rect: [0; 4],
                hidden: false,
                section: 1,
                channels: Vec::new(),
            },
        ],
    );
    let s = Registry::default()
        .open(Arc::new(Source::from_vec("boards.psd", f)))
        .unwrap();
    // Merged image, the composite of the layers, the artboard, the layer.
    assert_eq!(s.page_count(), 4);
    let h = s.header().expect("header facts");
    assert_eq!(h.summary, vec![("artboards", "Phone (4 × 4)".to_string())]);
    assert_eq!(
        s.open_page(1).unwrap().page_label(),
        Some(PageLabel::FromLayers)
    );
    let board = s.open_page(2).unwrap();
    assert_eq!(
        board.page_label(),
        Some(PageLabel::Sheet {
            name: "Phone".into(),
            model: false,
        })
    );
    assert_eq!((board.info().width, board.info().height), (4, 4));
    assert!(board.info().coding.ends_with(", artboard 1 of 1"));
    // Cut from the merged image.
    let r = full(&*board);
    assert_eq!((r.width, r.height), (4, 4));
    assert_eq!(px(&r, 0, 0), [10, 20, 30, 255]);
    let layer = s.open_page(3).unwrap();
    assert_eq!(
        layer.page_label(),
        Some(PageLabel::DocumentLayer {
            name: "Dot".into(),
            index: 1,
            hidden: false,
        })
    );
    assert_eq!(px(&full(&*layer), 3, 2), [0, 200, 0, 255]);
}

/// A 2×2 32-bit RGB PSD (raw composite of `v`, linear) carrying `icc` (resource 1039).
fn psd32(v: f32, icc: &[u8]) -> Vec<u8> {
    let mut f = b"8BPS".to_vec();
    be16(&mut f, 1);
    f.extend([0; 6]);
    be16(&mut f, 3);
    be32(&mut f, 2);
    be32(&mut f, 2);
    be16(&mut f, 32);
    be16(&mut f, 3);
    be32(&mut f, 0);
    let mut res = b"8BIM".to_vec();
    be16(&mut res, 1039);
    res.extend([0, 0]);
    be32(&mut res, icc.len() as u32);
    res.extend(icc);
    if icc.len() % 2 == 1 {
        res.push(0);
    }
    be32(&mut f, res.len() as u32);
    f.extend(res);
    be32(&mut f, 0);
    be16(&mut f, 0);
    for _ in 0..12 {
        f.extend(v.to_be_bytes());
    }
    f
}

/// Fidelity round 5 (psd-tools' `32bit.psd`, `300dpi.psd`, `pen-text.psd`): a 32-bit
/// document's linear profile was handed on with the composite already put through the sRGB
/// curve, so the viewer applied the curve twice (mid-tones shown far too bright). A linear
/// sRGB profile is now dropped, and other colorants travel with the sRGB curve.
#[test]
fn linear_profile_of_32_bit_documents_is_not_applied_twice() {
    use specular_color::math::{D65_XY, bt709_to_pcs, rgb_to_pcs};
    use specular_color::profiles::matrix_shaper_icc;
    let open32 = |icc: &[u8]| {
        Registry::default()
            .open(Arc::new(Source::from_vec("lin.psd", psd32(0.2, icc))))
            .unwrap()
    };
    let s = open32(&matrix_shaper_icc(&bt709_to_pcs(), 1.0, "sRGB linear"));
    assert!(s.info().icc_profile.is_none(), "linear sRGB: no profile");
    // 0.2 linear is sRGB code 124.
    assert_eq!(px(&full(s.as_ref()), 1, 1)[..3], [124, 124, 124]);
    let wide = rgb_to_pcs([0.7347, 0.2653], [0.1596, 0.8404], [0.0366, 0.0001], D65_XY);
    let s = open32(&matrix_shaper_icc(&wide, 1.0, "ProPhoto linear"));
    let icc = s.info().icc_profile.clone().expect("colorants kept");
    let p = specular_color::icc::Profile::parse(&icc).unwrap();
    assert!(p.matrix.is_some_and(|m| m.max_diff(&wide) < 1e-4));
    let c = &p.trc.as_ref().unwrap()[0];
    assert!((c.eval(124.0 / 255.0) - 0.2).abs() < 2e-3, "sRGB curve");
}
