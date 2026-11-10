//! The composite built from the layers: fill layers (solid colour, gradient, pattern),
//! vector masks, mask densities and group masks.

mod common;

use common::*;
use specular_psd::*;

const SEQ: Options = Options { parallel: false };

fn be32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_be_bytes());
}

/// A descriptor key or class ID of four bytes.
fn key(v: &mut Vec<u8>, k: &[u8; 4]) {
    be32(v, 0);
    v.extend_from_slice(k);
}

/// A descriptor: no name, class `class`, then the items (key, type, value bytes).
fn descriptor(class: &[u8; 4], items: &[(&[u8; 4], &[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut v = Vec::new();
    be32(&mut v, 0);
    key(&mut v, class);
    be32(&mut v, items.len() as u32);
    for (k, t, body) in items {
        key(&mut v, k);
        v.extend_from_slice(*t);
        v.extend_from_slice(body);
    }
    v
}

fn doub(x: f64) -> Vec<u8> {
    x.to_bits().to_be_bytes().to_vec()
}

fn unit(u: &[u8; 4], x: f64) -> Vec<u8> {
    let mut v = u.to_vec();
    v.extend(doub(x));
    v
}

fn long(x: i32) -> Vec<u8> {
    x.to_be_bytes().to_vec()
}

fn rgb(r: f64, g: f64, b: f64) -> Vec<u8> {
    descriptor(
        b"RGBC",
        &[
            (b"Rd  ", b"doub", doub(r)),
            (b"Grn ", b"doub", doub(g)),
            (b"Bl  ", b"doub", doub(b)),
        ],
    )
}

/// A block body: version 16 and the descriptor.
fn versioned(d: Vec<u8>) -> Vec<u8> {
    let mut v = 16u32.to_be_bytes().to_vec();
    v.extend(d);
    v
}

fn solid(r: f64, g: f64, b: f64) -> ([u8; 4], Vec<u8>) {
    (
        *b"SoCo",
        versioned(descriptor(b"null", &[(b"Clr ", b"Objc", rgb(r, g, b))])),
    )
}

/// A `vmsk` body: one closed rectangle (canvas fractions), flags.
fn vmsk(l: f64, t: f64, r: f64, b: f64, flags: u32) -> ([u8; 4], Vec<u8>) {
    let mut v = Vec::new();
    be32(&mut v, 3);
    be32(&mut v, flags);
    let rec = |v: &mut Vec<u8>, sel: u16, body: Vec<u8>| {
        let mut r = sel.to_be_bytes().to_vec();
        r.extend(body);
        r.resize(26, 0);
        v.extend(r);
    };
    rec(&mut v, 6, vec![]);
    rec(&mut v, 0, vec![0, 4, 0, 1]);
    let fx = |f: f64| ((f * f64::from(1u32 << 24)) as i32).to_be_bytes();
    for (x, y) in [(l, t), (r, t), (r, b), (l, b)] {
        let mut k = Vec::new();
        for _ in 0..3 {
            k.extend(fx(y));
            k.extend(fx(x));
        }
        rec(&mut v, 2, k);
    }
    (*b"vmsk", v)
}

fn fill_layer(name: &str, blocks: Vec<([u8; 4], Vec<u8>)>) -> LayerSpec {
    LayerSpec {
        name: name.into(),
        opacity: 255,
        blocks,
        ..Default::default()
    }
}

/// A document without a merged image (Maximize compatibility off) from `layers`.
fn doc(w: u32, h: u32, layers: Vec<LayerSpec>) -> Vec<u8> {
    let planes = (0..3).map(|c| pattern(w, h, c + 1, 1)).collect();
    let mut s = Spec::new(w, h, 8, 3, planes);
    s.layer_count = layers.len() as i16;
    s.layers = layers;
    s.resources
        .push((1057, version_info(false, "Adobe Photoshop")));
    write(&s)
}

fn rgba(f: &[u8]) -> (Psd, Vec<u8>) {
    let psd = parse(f).unwrap();
    assert!(psd.composites_layers());
    let img = psd.decode_rgba8(f, &SEQ).unwrap();
    (psd, img.data)
}

#[test]
fn solid_fill_through_a_vector_mask() {
    // A red fill shown through a rectangle over x 1..3 of a 4×1 canvas.
    let l = fill_layer(
        "fill",
        vec![solid(255.0, 0.0, 0.0), vmsk(0.25, 0.0, 0.75, 1.0, 0)],
    );
    let f = doc(4, 1, vec![l]);
    let (psd, px) = rgba(&f);
    assert_eq!(psd.layers()[0].fill.as_ref().unwrap().0, FillKind::Solid);
    assert_eq!(psd.undrawn_layers(&f), 0);
    assert_eq!(px[3], 0);
    assert_eq!(&px[4..8], &[255, 0, 0, 255]);
    assert_eq!(&px[8..12], &[255, 0, 0, 255]);
    assert_eq!(px[15], 0);
    // Inverted, the mask shows the fill outside the rectangle.
    let l = fill_layer(
        "fill",
        vec![solid(255.0, 0.0, 0.0), vmsk(0.25, 0.0, 0.75, 1.0, 1)],
    );
    let (_, px) = rgba(&doc(4, 1, vec![l]));
    assert_eq!([px[3], px[7], px[11], px[15]], [255, 0, 0, 255]);
    // Disabled, it hides nothing.
    let l = fill_layer(
        "fill",
        vec![solid(0.0, 0.0, 255.0), vmsk(0.25, 0.0, 0.75, 1.0, 4)],
    );
    let (_, px) = rgba(&doc(4, 1, vec![l]));
    assert_eq!([px[2], px[3], px[15]], [255, 255, 255]);
}

#[test]
fn vector_mask_coverage_is_antialiased() {
    // A rectangle edge halfway through pixel 1 of 4: half covered.
    let l = fill_layer(
        "fill",
        vec![solid(0.0, 255.0, 0.0), vmsk(0.375, 0.0, 1.0, 1.0, 0)],
    );
    let (_, px) = rgba(&doc(4, 1, vec![l]));
    assert_eq!(px[3], 0);
    assert!(px[7].abs_diff(128) <= 1, "{}", px[7]);
    assert_eq!(px[11], 255);
}

#[test]
fn gradient_fill_ramps_across_the_canvas() {
    let stop = |at: i32, v: f64| {
        descriptor(
            b"Clrt",
            &[
                (b"Clr ", b"Objc", rgb(v, v, v)),
                (b"Lctn", b"long", long(at)),
                (b"Mdpn", b"long", long(50)),
            ],
        )
    };
    let mut clrs = 2u32.to_be_bytes().to_vec();
    for s in [stop(0, 0.0), stop(4096, 255.0)] {
        clrs.extend_from_slice(b"Objc");
        clrs.extend(s);
    }
    let mut form = Vec::new();
    key(&mut form, b"GrdF");
    key(&mut form, b"CstS");
    let grad = descriptor(
        b"Grdn",
        &[(b"GrdF", b"enum", form), (b"Clrs", b"VlLs", clrs)],
    );
    let mut lin = Vec::new();
    key(&mut lin, b"GrdT");
    key(&mut lin, b"Lnr ");
    let body = versioned(descriptor(
        b"null",
        &[
            (b"Angl", b"UntF", unit(b"#Ang", 0.0)),
            (b"Type", b"enum", lin),
            (b"Grad", b"Objc", grad),
        ],
    ));
    let l = fill_layer("gradient", vec![(*b"GdFl", body)]);
    let (_, px) = rgba(&doc(5, 1, vec![l]));
    let reds: Vec<u8> = px.chunks(4).map(|p| p[0]).collect();
    assert_eq!(reds[0], 0);
    assert_eq!(reds[4], 255);
    assert!(reds[2].abs_diff(128) <= 1, "{reds:?}");
    assert!(reds.windows(2).all(|w| w[0] < w[1]), "{reds:?}");
    assert!(px.chunks(4).all(|p| p[3] == 255));
}

#[test]
fn pattern_fill_tiles_from_the_origin() {
    // A 2×1 RGB pattern (red, blue), raw channels.
    let id = b"pat-1";
    let mut rec = Vec::new();
    be32(&mut rec, 1); // version
    be32(&mut rec, 3); // RGB
    rec.extend_from_slice(&1i16.to_be_bytes());
    rec.extend_from_slice(&2i16.to_be_bytes());
    be32(&mut rec, 0); // name
    rec.push(id.len() as u8);
    rec.extend_from_slice(id);
    let mut vma = Vec::new();
    vma.extend_from_slice(&[0; 16]);
    be32(&mut vma, 3);
    for ch in [[255u8, 0], [0, 0], [0, 255]] {
        be32(&mut vma, 1);
        let mut c = Vec::new();
        be32(&mut c, 8);
        for v in [0, 0, 1, 2] {
            be32(&mut c, v);
        }
        c.extend_from_slice(&8u16.to_be_bytes());
        c.push(0);
        c.extend_from_slice(&ch);
        be32(&mut vma, c.len() as u32);
        vma.extend(c);
    }
    be32(&mut vma, 0); // no user mask
    be32(&mut vma, 0); // no sheet mask
    be32(&mut rec, 3);
    be32(&mut rec, vma.len() as u32);
    rec.extend(vma);
    let mut patt = Vec::new();
    be32(&mut patt, rec.len() as u32);
    patt.extend(rec);
    let mut idnt = Vec::new();
    be32(&mut idnt, id.len() as u32);
    for &b in id {
        idnt.extend_from_slice(&[0, b]);
    }
    let body = versioned(descriptor(
        b"null",
        &[(
            b"Ptrn",
            b"Objc",
            descriptor(b"Ptrn", &[(b"Idnt", b"TEXT", idnt)]),
        )],
    ));
    let l = fill_layer("pattern", vec![(*b"PtFl", body)]);
    let planes = (0..3).map(|c| pattern(4, 1, c + 1, 1)).collect();
    let mut s = Spec::new(4, 1, 8, 3, planes);
    s.layer_count = 1;
    s.layers = vec![l];
    s.resources
        .push((1057, version_info(false, "Adobe Photoshop")));
    s.blocks.push((*b"Patt", patt));
    let (_, px) = rgba(&write(&s));
    assert_eq!(
        px,
        [
            255, 0, 0, 255, 0, 0, 255, 255, 255, 0, 0, 255, 0, 0, 255, 255
        ]
    );
}

#[test]
fn group_masks_and_mask_density() {
    let green = LayerSpec {
        name: "green".into(),
        bounds: [0, 0, 1, 4],
        opacity: 255,
        channels: vec![
            (-1, vec![255; 4]),
            (0, vec![0; 4]),
            (1, vec![255; 4]),
            (2, vec![0; 4]),
        ],
        ..Default::default()
    };
    let divider = LayerSpec {
        name: "</Layer group>".into(),
        section: 3,
        ..Default::default()
    };
    // The group's mask hides x 0 (stored over x 0..2), shows the rest by its default.
    let folder = |blend: [u8; 4]| LayerSpec {
        name: "group".into(),
        section: 1,
        opacity: 255,
        blend: Some(blend),
        channels: vec![(-2, vec![0, 255])],
        mask: Some(MaskSpec {
            bounds: [0, 0, 1, 2],
            default: 255,
            ..Default::default()
        }),
        ..Default::default()
    };
    for blend in [*b"norm", *b"pass"] {
        let f = doc(4, 1, vec![divider.clone(), green.clone(), folder(blend)]);
        let (_, px) = rgba(&f);
        let a: Vec<u8> = px.chunks(4).map(|p| p[3]).collect();
        assert_eq!(a, [0, 255, 255, 255], "{}", String::from_utf8_lossy(&blend));
    }
    // A user mask at density 25 % (64): what it hides shows at 75 %.
    let mut l = green.clone();
    l.channels.push((-2, vec![0; 4]));
    l.mask = Some(MaskSpec {
        bounds: [0, 0, 1, 4],
        default: 0,
        flags: 0x10,
        params: vec![1, 64],
        ..Default::default()
    });
    let f = doc(4, 1, vec![l]);
    let psd = parse(&f).unwrap();
    assert_eq!(psd.layers()[0].mask.unwrap().density, 64);
    let (_, px) = rgba(&f);
    assert!(px[3].abs_diff(191) <= 1, "{}", px[3]);
}
