//! The composite built from the layers: adjustment layers, layer effects, shape strokes and
//! artboards (fidelity round psd2).

mod common;

use common::*;
use specular_psd::*;

const SEQ: Options = Options { parallel: false };

fn be32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_be_bytes());
}

fn key(v: &mut Vec<u8>, k: &[u8]) {
    if k.len() == 4 {
        be32(v, 0);
    } else {
        be32(v, k.len() as u32);
    }
    v.extend_from_slice(k);
}

/// A descriptor: no name, class `class`, then the items (key, type, value bytes).
fn descriptor(class: &[u8], items: &[(&[u8], &[u8; 4], Vec<u8>)]) -> Vec<u8> {
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

fn boolean(b: bool) -> Vec<u8> {
    vec![u8::from(b)]
}

fn enumv(ty: &[u8], val: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    key(&mut v, ty);
    key(&mut v, val);
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

fn versioned(d: Vec<u8>) -> Vec<u8> {
    let mut v = 16u32.to_be_bytes().to_vec();
    v.extend(d);
    v
}

/// An `lfx2` body: versions 0 and 16, then the effects descriptor.
fn lfx2(items: &[(&[u8], &[u8; 4], Vec<u8>)]) -> ([u8; 4], Vec<u8>) {
    let mut v = vec![0, 0, 0, 0];
    v.extend(versioned(descriptor(b"null", items)));
    (*b"lfx2", v)
}

/// A pixel layer of one colour over `bounds` (left, top, right, bottom), opaque.
fn px_layer(name: &str, bounds: [i32; 4], c: [u8; 3]) -> LayerSpec {
    let [l, t, r, b] = bounds;
    let n = ((r - l) * (b - t)) as usize;
    let mut channels: Vec<(i16, Vec<u8>)> = (0..3).map(|k| (k as i16, vec![c[k]; n])).collect();
    channels.push((-1, vec![255; n]));
    LayerSpec {
        name: name.into(),
        bounds: [t, l, b, r],
        opacity: 255,
        channels,
        ..Default::default()
    }
}

fn adj_layer(name: &str, key: &[u8; 4], body: Vec<u8>) -> LayerSpec {
    LayerSpec {
        name: name.into(),
        opacity: 255,
        blocks: vec![(*key, body)],
        ..Default::default()
    }
}

/// A document without a merged image from `layers` (bottom first).
fn doc(w: u32, h: u32, layers: Vec<LayerSpec>) -> Vec<u8> {
    let planes = (0..3).map(|c| pattern(w, h, c + 1, 1)).collect();
    let mut s = Spec::new(w, h, 8, 3, planes);
    s.layer_count = layers.len() as i16;
    s.layers = layers;
    s.resources
        .push((1057, version_info(false, "Adobe Photoshop")));
    write(&s)
}

fn rgba(f: &[u8]) -> Vec<u8> {
    let psd = parse(f).unwrap();
    assert!(psd.composites_layers());
    psd.decode_rgba8(f, &SEQ).unwrap().data
}

fn px(img: &[u8], w: usize, x: usize, y: usize) -> [u8; 4] {
    let o = (y * w + x) * 4;
    img[o..o + 4].try_into().unwrap()
}

fn close(a: [u8; 4], b: [u8; 4], tol: u8) -> bool {
    a.iter().zip(&b).all(|(x, y)| x.abs_diff(*y) <= tol)
}

/// Levels with the composite record set to `rec` (in black, in white, out black, out white,
/// gamma × 100) and the channels left alone.
fn levels(rec: [u16; 5]) -> Vec<u8> {
    let mut v = 2u16.to_be_bytes().to_vec();
    let id = [0u16, 255, 0, 255, 100];
    for r in std::iter::once(rec).chain(std::iter::repeat_n(id, 28)) {
        for x in r {
            v.extend(x.to_be_bytes());
        }
    }
    v
}

#[test]
fn table_adjustments_change_the_colours_below() {
    let base = || px_layer("base", [0, 0, 4, 1], [200, 100, 50]);
    let run = |adj: LayerSpec| rgba(&doc(4, 1, vec![base(), adj]));
    // Invert.
    let p = run(adj_layer("invert", b"nvrt", vec![]));
    assert_eq!(px(&p, 4, 0, 0), [55, 155, 205, 255]);
    // Levels: output white 128 halves every level.
    let p = run(adj_layer("levels", b"levl", levels([0, 255, 0, 128, 100])));
    assert!(
        close(px(&p, 4, 1, 0), [100, 50, 25, 255], 1),
        "{:?}",
        px(&p, 4, 1, 0)
    );
    // Curves: a straight line from (0, 0) to (255, 128) does the same.
    let mut c = vec![0u8, 0, 1, 0, 0, 0, 1, 0, 2];
    for (o, i) in [(0u16, 0u16), (128, 255)] {
        c.extend(o.to_be_bytes());
        c.extend(i.to_be_bytes());
    }
    let p = run(adj_layer("curves", b"curv", c));
    assert!(
        close(px(&p, 4, 2, 0), [100, 50, 25, 255], 1),
        "{:?}",
        px(&p, 4, 2, 0)
    );
    // Threshold at 128: luma 117 is black.
    let p = run(adj_layer("threshold", b"thrs", vec![0, 128, 0, 0]));
    assert_eq!(px(&p, 4, 3, 0), [0, 0, 0, 255]);
    // Posterize to two levels.
    let p = run(adj_layer("posterize", b"post", vec![0, 2, 0, 0]));
    assert_eq!(px(&p, 4, 0, 0), [255, 0, 0, 255]);
}

#[test]
fn colour_adjustments() {
    let base = || px_layer("base", [0, 0, 2, 1], [255, 0, 0]);
    let run = |adj: LayerSpec| rgba(&doc(2, 1, vec![base(), adj]));
    // Hue/saturation: master saturation −100 leaves grey.
    let mut hs = vec![0u8, 2, 0, 0, 0, 0, 0, 0, 0, 0];
    hs.extend([0, 0]);
    hs.extend((-100i16).to_be_bytes());
    hs.extend([0, 0]);
    hs.extend(vec![0u8; 6 * 14]);
    let p = run(adj_layer("hue", b"hue2", hs));
    let q = px(&p, 2, 0, 0);
    assert!(q[0] == q[1] && q[1] == q[2], "{q:?}");
    // Channel mixer: red out of the blue channel, blue out of the red.
    let mut m = vec![0u8, 1, 0, 0];
    for row in [
        [0i16, 0, 100, 0, 0],
        [0, 100, 0, 0, 0],
        [100, 0, 0, 0, 0],
        [0; 5],
    ] {
        for x in row {
            m.extend(x.to_be_bytes());
        }
    }
    let p = run(adj_layer("mixer", b"mixr", m));
    assert_eq!(px(&p, 2, 0, 0), [0, 0, 255, 255]);
    // Black & white: pure red at the red weight (40 %).
    let bw = versioned(descriptor(
        b"null",
        &[
            (b"Rd  ", b"long", long(40)),
            (b"Yllw", b"long", long(60)),
            (b"Grn ", b"long", long(40)),
            (b"Cyn ", b"long", long(60)),
            (b"Bl  ", b"long", long(20)),
            (b"Mgnt", b"long", long(80)),
            (b"useTint", b"bool", boolean(false)),
        ],
    ));
    let p = run(adj_layer("bw", b"blwh", bw));
    assert!(
        close(px(&p, 2, 1, 0), [102, 102, 102, 255], 1),
        "{:?}",
        px(&p, 2, 1, 0)
    );
}

#[test]
fn adjustments_follow_their_mask_opacity_and_clipping() {
    // Half opacity: halfway to the inverse.
    let mut inv = adj_layer("invert", b"nvrt", vec![]);
    inv.opacity = 128;
    let p = rgba(&doc(
        2,
        1,
        vec![px_layer("base", [0, 0, 2, 1], [200, 200, 200]), inv],
    ));
    assert!(
        close(px(&p, 2, 0, 0), [127, 127, 127, 255], 2),
        "{:?}",
        px(&p, 2, 0, 0)
    );
    // Clipped to a layer over x 1 only: the layer below it is left alone.
    let mut inv = adj_layer("invert", b"nvrt", vec![]);
    inv.clipping = true;
    let p = rgba(&doc(
        2,
        1,
        vec![
            px_layer("bottom", [0, 0, 2, 1], [200, 200, 200]),
            px_layer("base", [1, 0, 2, 1], [10, 20, 30]),
            inv,
        ],
    ));
    assert_eq!(px(&p, 2, 0, 0), [200, 200, 200, 255]);
    assert_eq!(px(&p, 2, 1, 0), [245, 235, 225, 255]);
    // A colour lookup is not drawn, and is counted.
    let f = doc(
        2,
        1,
        vec![
            px_layer("base", [0, 0, 2, 1], [9, 9, 9]),
            adj_layer("lut", b"clrL", vec![]),
        ],
    );
    let psd = parse(&f).unwrap();
    assert_eq!(psd.undrawn_layers(&f), 1);
    assert_eq!(psd.composite_notes(&f).not_drawn, vec!["Color Lookup"]);
    // One that names no lookup (as Photoshop saves a new one: psd-tools'
    // `layers/color-lookup.psd`) changes nothing and is drawn.
    let empty = [
        &[0u8, 1, 0, 0, 0, 16, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0][..],
        b"null",
        &[0, 0, 0, 0],
    ]
    .concat();
    let f = doc(
        2,
        1,
        vec![
            px_layer("base", [0, 0, 2, 1], [9, 9, 9]),
            adj_layer("lut", b"clrL", empty),
        ],
    );
    let psd = parse(&f).unwrap();
    assert_eq!(psd.undrawn_layers(&f), 0);
    assert!(psd.composite_notes(&f).not_drawn.is_empty());
    assert_eq!(px(&rgba(&f), 2, 1, 0), [9, 9, 9, 255]);
}

fn effect(class: &[u8; 4], items: &[(&[u8], &[u8; 4], Vec<u8>)]) -> Vec<u8> {
    descriptor(class, items)
}

#[test]
fn colour_overlay_shows_through_fill_opacity() {
    let mut l = px_layer("square", [2, 2, 4, 4], [0, 0, 255]);
    l.fill_opacity = Some(0);
    l.blocks.push(lfx2(&[
        (b"masterFXSwitch", b"bool", boolean(true)),
        (
            b"SoFi",
            b"Objc",
            effect(
                b"SoFi",
                &[
                    (b"enab", b"bool", boolean(true)),
                    (b"Md  ", b"enum", enumv(b"BlnM", b"Nrml")),
                    (b"Clr ", b"Objc", rgb(0.0, 255.0, 0.0)),
                    (b"Opct", b"UntF", unit(b"#Prc", 100.0)),
                ],
            ),
        ),
    ]));
    let p = rgba(&doc(6, 6, vec![l]));
    assert_eq!(px(&p, 6, 2, 2), [0, 255, 0, 255]);
    assert_eq!(px(&p, 6, 1, 1)[3], 0);
}

#[test]
fn drop_shadow_and_stroke_around_the_shape() {
    let mut l = px_layer("square", [4, 4, 8, 8], [255, 255, 255]);
    l.blocks.push(lfx2(&[
        (b"masterFXSwitch", b"bool", boolean(true)),
        (
            b"DrSh",
            b"Objc",
            effect(
                b"DrSh",
                &[
                    (b"enab", b"bool", boolean(true)),
                    (b"Md  ", b"enum", enumv(b"BlnM", b"Nrml")),
                    (b"Clr ", b"Objc", rgb(255.0, 0.0, 0.0)),
                    (b"Opct", b"UntF", unit(b"#Prc", 50.0)),
                    (b"uglg", b"bool", boolean(false)),
                    (b"lagl", b"UntF", unit(b"#Ang", 90.0)),
                    (b"Dstn", b"UntF", unit(b"#Pxl", 3.0)),
                    (b"Ckmt", b"UntF", unit(b"#Prc", 0.0)),
                    (b"blur", b"UntF", unit(b"#Pxl", 0.0)),
                ],
            ),
        ),
        (
            b"FrFX",
            b"Objc",
            effect(
                b"FrFX",
                &[
                    (b"enab", b"bool", boolean(true)),
                    (b"Styl", b"enum", enumv(b"FStl", b"OutF")),
                    (b"PntT", b"enum", enumv(b"FrFl", b"SClr")),
                    (b"Md  ", b"enum", enumv(b"BlnM", b"Nrml")),
                    (b"Opct", b"UntF", unit(b"#Prc", 100.0)),
                    (b"Sz  ", b"UntF", unit(b"#Pxl", 1.0)),
                    (b"Clr ", b"Objc", rgb(0.0, 0.0, 255.0)),
                ],
            ),
        ),
    ]));
    let p = rgba(&doc(12, 14, vec![l]));
    // The light comes from above: the shadow falls 3 pixels down, at half opacity.
    assert_eq!(px(&p, 12, 5, 10), [255, 0, 0, 128]);
    assert_eq!(px(&p, 12, 5, 11)[3], 0);
    // A one-pixel stroke just outside, on top of the shadow.
    assert_eq!(px(&p, 12, 3, 5), [0, 0, 255, 255]);
    assert_eq!(px(&p, 12, 5, 8), [0, 0, 255, 255]);
    assert_eq!(px(&p, 12, 5, 5), [255, 255, 255, 255]);
    assert_eq!(px(&p, 12, 1, 1)[3], 0);
}

/// A `vmsk` body: one closed rectangle (canvas fractions).
fn vmsk(l: f64, t: f64, r: f64, b: f64) -> ([u8; 4], Vec<u8>) {
    let mut v = Vec::new();
    be32(&mut v, 3);
    be32(&mut v, 0);
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

fn vstk(width: f64, align: &[u8], fill: bool, dashes: &[f64]) -> ([u8; 4], Vec<u8>) {
    let mut dash = (dashes.len() as u32).to_be_bytes().to_vec();
    for d in dashes {
        dash.extend(b"UntF");
        dash.extend(unit(b"#Nne", *d));
    }
    (
        *b"vstk",
        versioned(descriptor(
            b"strokeStyle",
            &[
                (b"strokeEnabled", b"bool", boolean(true)),
                (b"fillEnabled", b"bool", boolean(fill)),
                (b"strokeStyleLineWidth", b"UntF", unit(b"#Pxl", width)),
                (
                    b"strokeStyleLineAlignment",
                    b"enum",
                    enumv(b"strokeStyleLineAlignment", align),
                ),
                (
                    b"strokeStyleLineJoinType",
                    b"enum",
                    enumv(b"strokeStyleLineJoinType", b"strokeStyleMiterJoin"),
                ),
                (b"strokeStyleLineDashSet", b"VlLs", dash),
                (b"strokeStyleOpacity", b"UntF", unit(b"#Prc", 100.0)),
                (
                    b"strokeStyleContent",
                    b"Objc",
                    descriptor(
                        b"solidColorLayer",
                        &[(b"Clr ", b"Objc", rgb(255.0, 0.0, 0.0))],
                    ),
                ),
            ],
        )),
    )
}

fn shape(stroke: ([u8; 4], Vec<u8>)) -> LayerSpec {
    let soco = versioned(descriptor(
        b"null",
        &[(b"Clr ", b"Objc", rgb(0.0, 0.0, 255.0))],
    ));
    LayerSpec {
        name: "shape".into(),
        opacity: 255,
        blocks: vec![(*b"SoCo", soco), vmsk(0.25, 0.25, 0.75, 0.75), stroke],
        ..Default::default()
    }
}

#[test]
fn shape_strokes_follow_width_alignment_and_fill() {
    // A 16×16 canvas, the square over 4..12; a 2-pixel stroke centred on its edge.
    let p = rgba(&doc(
        16,
        16,
        vec![shape(vstk(2.0, b"strokeStyleAlignCenter", true, &[]))],
    ));
    assert_eq!(px(&p, 16, 8, 3), [255, 0, 0, 255]);
    assert_eq!(px(&p, 16, 8, 4), [255, 0, 0, 255]);
    assert_eq!(px(&p, 16, 8, 5), [0, 0, 255, 255]);
    assert_eq!(px(&p, 16, 8, 2)[3], 0);
    // Miter joins keep the corner square.
    assert_eq!(px(&p, 16, 3, 3), [255, 0, 0, 255]);
    // Inside, without the fill: a ring within the square, hollow in the middle.
    let p = rgba(&doc(
        16,
        16,
        vec![shape(vstk(2.0, b"strokeStyleAlignInside", false, &[]))],
    ));
    assert_eq!(px(&p, 16, 8, 3)[3], 0);
    assert_eq!(px(&p, 16, 8, 5), [255, 0, 0, 255]);
    assert_eq!(px(&p, 16, 8, 8)[3], 0);
    // Outside: around the square only.
    let p = rgba(&doc(
        16,
        16,
        vec![shape(vstk(2.0, b"strokeStyleAlignOutside", true, &[]))],
    ));
    assert_eq!(px(&p, 16, 8, 2), [255, 0, 0, 255]);
    assert_eq!(px(&p, 16, 8, 4), [0, 0, 255, 255]);
    // Dashes of 1 width on, 1 off along the top edge.
    let p = rgba(&doc(
        16,
        16,
        vec![shape(vstk(
            2.0,
            b"strokeStyleAlignCenter",
            false,
            &[1.0, 1.0],
        ))],
    ));
    let top: Vec<u8> = (4..12).map(|x| px(&p, 16, x, 3)[3]).collect();
    assert!(top.contains(&255) && top.contains(&0), "{top:?}");
}

#[test]
fn artboards_are_listed_cropped_and_backed() {
    let rect = descriptor(
        b"classFloatRect",
        &[
            (b"Top ", b"doub", doub(1.0)),
            (b"Left", b"doub", doub(2.0)),
            (b"Btom", b"doub", doub(5.0)),
            (b"Rght", b"doub", doub(6.0)),
        ],
    );
    let artb = versioned(descriptor(
        b"artboard",
        &[
            (b"artboardRect", b"Objc", rect),
            (b"Clr ", b"Objc", rgb(0.0, 255.0, 0.0)),
            (b"artboardBackgroundType", b"long", long(4)),
        ],
    ));
    let end = LayerSpec {
        name: "</Layer group>".into(),
        opacity: 255,
        section: 3,
        ..Default::default()
    };
    let group = LayerSpec {
        name: "Phone".into(),
        opacity: 255,
        section: 1,
        blocks: vec![(*b"artb", artb)],
        ..Default::default()
    };
    let f = doc(
        8,
        8,
        vec![end, px_layer("dot", [3, 2, 4, 3], [255, 0, 0]), group],
    );
    let psd = parse(&f).unwrap();
    let boards = psd.artboards(&f);
    assert_eq!(boards.len(), 1);
    let a = &boards[0];
    assert_eq!(
        (a.name.as_str(), a.left, a.top, a.right, a.bottom),
        ("Phone", 2, 1, 6, 5)
    );
    assert_eq!(a.background, ArtboardBackground::Color([0, 255, 0]));
    // The whole composite: transparent outside the artboard, its colour inside.
    let p = rgba(&f);
    assert_eq!(px(&p, 8, 0, 0)[3], 0);
    assert_eq!(px(&p, 8, 2, 1), [0, 255, 0, 255]);
    assert_eq!(px(&p, 8, 3, 2), [255, 0, 0, 255]);
    // The artboard alone.
    let pl = psd.decode_artboard_planes(&f, a, &SEQ).unwrap();
    assert_eq!((pl.width, pl.height), (4, 4));
    let img = pl.to_rgba8();
    assert_eq!(px(&img.data, 4, 1, 1), [255, 0, 0, 255]);
    assert_eq!(px(&img.data, 4, 0, 0), [0, 255, 0, 255]);
}

#[test]
fn group_blend_mode_comes_from_its_section_block() {
    // The record says normal, the section block pass through.
    let end = LayerSpec {
        name: "</Layer group>".into(),
        opacity: 255,
        section: 3,
        ..Default::default()
    };
    let mut lsct = 1u32.to_be_bytes().to_vec();
    lsct.extend(b"8BIMpass");
    let group = LayerSpec {
        name: "g".into(),
        opacity: 255,
        blocks: vec![(*b"lsct", lsct)],
        ..Default::default()
    };
    let f = doc(1, 1, vec![end, group]);
    let psd = parse(&f).unwrap();
    assert_eq!(&psd.layers()[1].blend_mode, b"pass");
}

/// A group's records around `children`: its divider below, its folder record above.
fn group(
    name: &str,
    blend: &[u8; 4],
    extra: Vec<([u8; 4], Vec<u8>)>,
    fill: Option<u8>,
    children: Vec<LayerSpec>,
) -> Vec<LayerSpec> {
    let mut v = vec![LayerSpec {
        name: "</Layer group>".into(),
        opacity: 255,
        section: 3,
        ..Default::default()
    }];
    v.extend(children);
    v.push(LayerSpec {
        name: name.into(),
        opacity: 255,
        section: 1,
        blend: Some(*blend),
        blocks: extra,
        fill_opacity: fill,
        ..Default::default()
    });
    v
}

/// A background layer (no transparency channel) of one colour.
fn background(w: i32, h: i32, c: [u8; 3]) -> LayerSpec {
    let mut l = px_layer("Background", [0, 0, w, h], c);
    l.channels.retain(|(id, _)| *id != -1);
    l
}

fn knko(v: u8) -> ([u8; 4], Vec<u8>) {
    (*b"knko", vec![v, 0, 0, 0])
}

#[test]
fn knockout_shows_the_background_or_the_group_start() {
    // Deep: a half-filled blue group knocks the red layer out down to the white background.
    let mut layers = vec![
        background(4, 4, [255, 255, 255]),
        px_layer("red", [0, 0, 4, 4], [255, 0, 0]),
    ];
    layers.extend(group(
        "knock",
        b"norm",
        vec![knko(2)],
        Some(128),
        vec![px_layer("blue", [0, 0, 4, 4], [0, 0, 255])],
    ));
    let p = rgba(&doc(4, 4, layers.clone()));
    assert!(
        close(px(&p, 4, 1, 1), [127, 127, 255, 255], 2),
        "{:?}",
        px(&p, 4, 1, 1)
    );
    // No knockout: blue over red.
    let n = layers.len();
    layers[n - 1].blocks = vec![knko(0)];
    let p = rgba(&doc(4, 4, layers));
    assert!(
        close(px(&p, 4, 1, 1), [127, 0, 128, 255], 2),
        "{:?}",
        px(&p, 4, 1, 1)
    );
    // Shallow, inside a pass-through group over the red layer: green (in the outer group)
    // is knocked out, red stays.
    let inner = group(
        "inner",
        b"norm",
        vec![knko(1)],
        Some(128),
        vec![px_layer("blue", [0, 0, 4, 4], [0, 0, 255])],
    );
    let mut outer_children = vec![px_layer("green", [0, 0, 4, 4], [0, 255, 0])];
    outer_children.extend(inner);
    let mut layers = vec![
        background(4, 4, [255, 255, 255]),
        px_layer("red", [0, 0, 4, 4], [255, 0, 0]),
    ];
    layers.extend(group("outer", b"pass", vec![], None, outer_children));
    let p = rgba(&doc(4, 4, layers));
    assert!(
        close(px(&p, 4, 2, 2), [127, 0, 128, 255], 2),
        "{:?}",
        px(&p, 4, 2, 2)
    );
}

/// Blend If ranges: grey this layer, grey underlying, then R, G, B left open.
fn ranges(this: [u8; 4], under: [u8; 4]) -> Vec<u8> {
    let mut v = this.to_vec();
    v.extend(under);
    for _ in 0..3 {
        v.extend([0, 0, 255, 255, 0, 0, 255, 255]);
    }
    v
}

#[test]
fn blend_if_hides_by_this_layer_and_the_underlying_one() {
    // Underlying: black on the left half, mid grey on the right half.
    let mut left = px_layer("dark", [0, 0, 2, 2], [0, 0, 0]);
    left.name = "dark".into();
    let right = px_layer("grey", [2, 0, 4, 2], [128, 128, 128]);
    let mut top = px_layer("blue", [0, 0, 4, 2], [0, 0, 255]);
    // Shown only where what is underneath is at least 100 (grey).
    top.ranges = ranges([0, 0, 255, 255], [100, 100, 255, 255]);
    let p = rgba(&doc(4, 2, vec![left.clone(), right.clone(), top]));
    assert_eq!(px(&p, 4, 0, 0), [0, 0, 0, 255]);
    assert_eq!(px(&p, 4, 3, 0), [0, 0, 255, 255]);
    // This layer: a white layer whose white slider stops at 200 does not show.
    let mut white = px_layer("white", [0, 0, 4, 2], [255, 255, 255]);
    white.ranges = ranges([0, 0, 200, 200], [0, 0, 255, 255]);
    let p = rgba(&doc(4, 2, vec![left.clone(), right.clone(), white]));
    assert_eq!(px(&p, 4, 0, 0), [0, 0, 0, 255]);
    assert_eq!(px(&p, 4, 3, 0), [128, 128, 128, 255]);
    // A split slider: underlying grey 128 between 64 and 192 lets half through.
    let mut top = px_layer("blue", [0, 0, 4, 2], [0, 0, 255]);
    top.ranges = ranges([0, 0, 255, 255], [64, 192, 255, 255]);
    let f = doc(4, 2, vec![left, right, top]);
    let notes = parse(&f).unwrap().composite_notes(&f);
    assert_eq!(notes.approximate, ["Blend If"]);
    let p = rgba(&f);
    assert!(
        close(px(&p, 4, 3, 0), [64, 64, 192, 255], 2),
        "{:?}",
        px(&p, 4, 3, 0)
    );
}

#[test]
fn special_modes_take_fill_opacity_in_the_blend() {
    // Photoshop's merged image of this stack (ag-psd's fill-opacity sample): 63, 78, 66, 206.
    let mut blue = px_layer("blue", [0, 0, 1, 1], [0, 0, 255]);
    blue.opacity = 153;
    let mut green = px_layer("green", [0, 0, 1, 1], [0, 255, 0]);
    green.opacity = 102;
    green.blend = Some(*b"lLit");
    let mut red = px_layer("red", [0, 0, 1, 1], [255, 0, 0]);
    red.fill_opacity = Some(51);
    red.blend = Some(*b"lLit");
    let p = rgba(&doc(1, 1, vec![blue, green, red]));
    assert!(
        close(px(&p, 1, 0, 0), [63, 78, 66, 206], 2),
        "{:?}",
        px(&p, 1, 0, 0)
    );
}

#[test]
fn clipped_layers_blend_alone_when_not_as_a_group() {
    // A blue base at fill 0 with a red multiply layer clipped to it, over white.
    let base = |clbl: u8| {
        let mut l = px_layer("base", [0, 0, 2, 2], [0, 0, 255]);
        l.fill_opacity = Some(0);
        l.blocks.push((*b"clbl", vec![clbl, 0, 0, 0]));
        l
    };
    let clip = || {
        let mut l = px_layer("red", [0, 0, 4, 4], [255, 0, 0]);
        l.clipping = true;
        l.blend = Some(*b"mul ");
        l
    };
    let white = background(4, 4, [255, 255, 255]);
    // As a group: the red multiplies the faded base (nothing), white shows.
    let p = rgba(&doc(4, 4, vec![white.clone(), base(1), clip()]));
    assert_eq!(px(&p, 4, 1, 1), [255, 255, 255, 255]);
    // Alone: the red multiplies the white backdrop where the base is.
    let p = rgba(&doc(4, 4, vec![white, base(0), clip()]));
    assert_eq!(px(&p, 4, 1, 1), [255, 0, 0, 255]);
    assert_eq!(px(&p, 4, 3, 3), [255, 255, 255, 255]);
}

#[test]
fn interior_effects_as_a_group_fade_with_fill_opacity() {
    let mut l = px_layer("square", [2, 2, 4, 4], [0, 0, 255]);
    l.fill_opacity = Some(0);
    l.blocks.push((*b"infx", vec![1, 0, 0, 0]));
    l.blocks.push(lfx2(&[
        (b"masterFXSwitch", b"bool", boolean(true)),
        (
            b"SoFi",
            b"Objc",
            effect(
                b"SoFi",
                &[
                    (b"enab", b"bool", boolean(true)),
                    (b"Md  ", b"enum", enumv(b"BlnM", b"Nrml")),
                    (b"Clr ", b"Objc", rgb(0.0, 255.0, 0.0)),
                    (b"Opct", b"UntF", unit(b"#Prc", 100.0)),
                ],
            ),
        ),
    ]));
    let p = rgba(&doc(6, 6, vec![l]));
    assert_eq!(px(&p, 6, 2, 2)[3], 0);
}

#[test]
fn legacy_effects_are_drawn() {
    // The drop shadow of `drop_shadow_and_stroke_around_the_shape`, as an `lrFX` record.
    let mut ds = 2u32.to_be_bytes().to_vec();
    for v in [0.0f64, 0.0, 90.0, 3.0] {
        ds.extend(((v * 65536.0) as i32).to_be_bytes());
    }
    ds.extend([0, 0, 255, 255, 0, 0, 0, 0, 0, 0]);
    ds.extend(b"8BIMnorm");
    ds.extend([1, 0, 128]);
    let rec = |key: &[u8; 4], body: &[u8]| {
        let mut v = b"8BIM".to_vec();
        v.extend(key);
        v.extend((body.len() as u32).to_be_bytes());
        v.extend(body);
        v
    };
    let mut lrfx = vec![0, 0, 0, 2];
    lrfx.extend(rec(b"cmnS", &[0, 0, 0, 0, 1, 0, 0]));
    lrfx.extend(rec(b"dsdw", &ds));
    let mut l = px_layer("square", [4, 4, 8, 8], [255, 255, 255]);
    l.blocks.push((*b"lrFX", lrfx));
    let f = doc(12, 14, vec![l]);
    let p = rgba(&f);
    assert_eq!(px(&p, 12, 5, 10), [255, 0, 0, 128]);
    assert_eq!(px(&p, 12, 5, 11)[3], 0);
    let psd = parse(&f).unwrap();
    assert!(psd.composite_notes(&f).not_drawn.is_empty());
}

/// Files whose layers draw nothing (adjustment layers alone, an empty layer) are no longer
/// refused: their composite is a transparent canvas, as Photoshop shows them, and a solid
/// fill among them draws.
#[test]
fn adjustment_only_files_render() {
    let adjustments = [
        adj_layer("levels", b"levl", levels([0, 255, 0, 128, 100])),
        adj_layer("hue/saturation", b"hue2", vec![0; 4]),
        adj_layer("brightness", b"brit", vec![0; 8]),
        adj_layer("invert", b"nvrt", vec![]),
        adj_layer("threshold", b"thrs", vec![0, 128, 0, 0]),
    ];
    for a in adjustments {
        let name = a.name.clone();
        let f = doc(3, 2, vec![a]);
        let img = rgba(&f);
        assert!(img.chunks_exact(4).all(|p| p[3] == 0), "{name}");
    }
    let empty = LayerSpec {
        name: "empty".into(),
        opacity: 255,
        ..Default::default()
    };
    assert!(rgba(&doc(2, 2, vec![empty])).iter().all(|&v| v == 0));
    // A solid colour fill under an invert: the fill, inverted.
    let fill = adj_layer(
        "fill",
        b"SoCo",
        versioned(descriptor(
            b"null",
            &[(b"Clr ", b"Objc", rgb(255.0, 0.0, 64.0))],
        )),
    );
    let img = rgba(&doc(2, 2, vec![fill, adj_layer("invert", b"nvrt", vec![])]));
    assert_eq!(px(&img, 2, 1, 1), [0, 255, 191, 255]);
}

/// A stored merged image that is one colour throughout under layers that show something is a
/// placeholder: the composite is built from the layers. A real merged image is kept, and
/// the layers can still be composited beside it.
#[test]
fn blank_merged_images_are_rebuilt_from_the_layers() {
    let build = |planes: Vec<Vec<u8>>| {
        let mut s = Spec::new(4, 4, 8, 3, planes);
        s.compression = 1;
        s.layers = vec![px_layer("red", [1, 1, 3, 3], [255, 0, 0])];
        s.layer_count = 1;
        write(&s)
    };
    let f = build(vec![vec![255; 16]; 3]);
    let psd = parse(&f).unwrap();
    assert_eq!(psd.info().composite, Composite::Blank);
    assert!(psd.composites_layers());
    let img = psd.decode_rgba8(&f, &SEQ).unwrap().data;
    assert_eq!(px(&img, 4, 1, 1), [255, 0, 0, 255]);
    assert_eq!(px(&img, 4, 0, 0)[3], 0);
    // A merged image with any variation is real.
    let mut planes = vec![vec![255; 16]; 3];
    planes[1][5] = 7;
    let f = build(planes);
    let psd = parse(&f).unwrap();
    assert_eq!(psd.info().composite, Composite::Present);
    assert!(!psd.composites_layers() && psd.can_compose_layers());
    assert_eq!(psd.decode_rgba8(&f, &SEQ).unwrap().data[4 * 5 + 1], 7);
    let p = psd.compose_layers(&f, &SEQ).unwrap().to_rgba8().data;
    assert_eq!(px(&p, 4, 2, 2), [255, 0, 0, 255]);
    // Hidden layers do not make a one-colour merged image a placeholder.
    let mut s = Spec::new(4, 4, 8, 3, vec![vec![255; 16]; 3]);
    let mut hidden = px_layer("red", [1, 1, 3, 3], [255, 0, 0]);
    hidden.hidden = true;
    s.layers = vec![hidden];
    s.layer_count = 1;
    let f = write(&s);
    assert_eq!(parse(&f).unwrap().info().composite, Composite::Present);
}

fn bevel_layer(style: &[u8], size: f64) -> LayerSpec {
    let mut l = px_layer("bar", [4, 10, 60, 22], [128, 128, 128]);
    l.blocks.push(lfx2(&[
        (b"masterFXSwitch", b"bool", boolean(true)),
        (
            b"ebbl",
            b"Objc",
            effect(
                b"ebbl",
                &[
                    (b"enab", b"bool", boolean(true)),
                    (b"hglM", b"enum", enumv(b"BlnM", b"Scrn")),
                    (b"hglC", b"Objc", rgb(255.0, 255.0, 255.0)),
                    (b"hglO", b"UntF", unit(b"#Prc", 100.0)),
                    (b"sdwM", b"enum", enumv(b"BlnM", b"Mltp")),
                    (b"sdwC", b"Objc", rgb(0.0, 0.0, 0.0)),
                    (b"sdwO", b"UntF", unit(b"#Prc", 100.0)),
                    (b"bvlT", b"enum", enumv(b"bvlT", b"SfBL")),
                    (b"bvlS", b"enum", enumv(b"BESl", style)),
                    (b"uglg", b"bool", boolean(false)),
                    (b"lagl", b"UntF", unit(b"#Ang", 90.0)),
                    (b"Lald", b"UntF", unit(b"#Ang", 30.0)),
                    (b"srgR", b"UntF", unit(b"#Prc", 100.0)),
                    (b"blur", b"UntF", unit(b"#Pxl", size)),
                    (b"bvlD", b"enum", enumv(b"BESs", b"In  ")),
                ],
            ),
        ),
    ]));
    l
}

#[test]
fn smooth_bevel_domes_a_thin_shape() {
    // A bar 12 px tall under a bevel of 20: the blurred shape is a dome, lit from above
    // over its whole height, as in Photoshop's merged images.
    let p = rgba(&doc(64, 32, vec![bevel_layer(b"InrB", 20.0)]));
    let g = |y: usize| i32::from(px(&p, 64, 32, y)[0]);
    assert!(g(11) > 128 + 40, "top {}", g(11));
    assert!(g(20) < 128 - 40, "bottom {}", g(20));
    assert!((11..20).all(|y| g(y) >= g(y + 1)));
    // An inner bevel stays inside.
    assert_eq!(px(&p, 64, 32, 8)[3], 0);
}

#[test]
fn emboss_shows_on_both_sides_of_the_edge() {
    let p = rgba(&doc(64, 32, vec![bevel_layer(b"Embs", 8.0)]));
    // Outside, just above the bar: the slope facing the light is lit.
    let above = px(&p, 64, 32, 8);
    assert!(above[3] > 0, "{above:?}");
    // Inside the bar's lower edge the slope faces away: darker than the middle.
    assert!(px(&p, 64, 32, 20)[0] < px(&p, 64, 32, 16)[0]);
}

#[test]
fn satin_follows_its_own_angle_and_contour() {
    let satin = |crv: Option<Vec<u8>>| {
        let mut l = px_layer("sq", [8, 8, 40, 40], [255, 255, 255]);
        let mut items: Vec<(&[u8], &[u8; 4], Vec<u8>)> = vec![
            (b"enab", b"bool", boolean(true)),
            (b"Md  ", b"enum", enumv(b"BlnM", b"Nrml")),
            (b"Clr ", b"Objc", rgb(0.0, 0.0, 0.0)),
            (b"Invr", b"bool", boolean(false)),
            (b"Opct", b"UntF", unit(b"#Prc", 100.0)),
            (b"lagl", b"UntF", unit(b"#Ang", 0.0)),
            (b"Dstn", b"UntF", unit(b"#Pxl", 8.0)),
            (b"blur", b"UntF", unit(b"#Pxl", 4.0)),
        ];
        if let Some(c) = crv {
            items.push((b"MpgS", b"Objc", c));
        }
        l.blocks.push(lfx2(&[
            (b"masterFXSwitch", b"bool", boolean(true)),
            (b"ChFX", b"Objc", effect(b"ChFX", &items)),
        ]));
        rgba(&doc(48, 48, vec![l]))
    };
    let p = satin(None);
    // The angle is horizontal: the copies differ near the left and right edges only.
    let v = |p: &[u8], x: usize, y: usize| px(p, 48, x, y)[0];
    assert!(v(&p, 9, 24) < 100, "left edge {}", v(&p, 9, 24));
    assert_eq!(v(&p, 24, 9), 255);
    assert_eq!(v(&p, 24, 24), 255);
    // A contour that keeps only the strongest part leaves the edge lighter.
    let point = |x: f64, y: f64| {
        descriptor(
            b"CrPt",
            &[(b"Hrzn", b"doub", doub(x)), (b"Vrtc", b"doub", doub(y))],
        )
    };
    let mut list = Vec::new();
    let pts = [point(0.0, 0.0), point(200.0, 0.0), point(255.0, 255.0)];
    be32(&mut list, pts.len() as u32);
    for p in pts {
        list.extend_from_slice(b"Objc");
        list.extend(p);
    }
    let crv = descriptor(b"ShpC", &[(b"Crv ", b"VlLs", list)]);
    let q = satin(Some(crv));
    assert!(
        v(&q, 12, 24) > v(&p, 12, 24),
        "{} {}",
        v(&q, 12, 24),
        v(&p, 12, 24)
    );
}
