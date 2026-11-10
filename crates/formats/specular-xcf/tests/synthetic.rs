//! XCF files written here (a small writer covering what the tests need): uncompressed, RLE
//! and zlib tiles, 32- and 64-bit offsets, 8- and 16-bit samples, offsets, masks, groups,
//! the ICC parasite, missing and corrupt tiles, cancellation and the API around `render`.

use specular_xcf::{
    BaseType, Compression, Error, Options, PixelFormat, decompress, parse, probe, probe_file,
};

const RGB: u32 = 0;
const RGBA: u32 = 1;
const GREYA: u32 = 3;

const PROP_END: u32 = 0;
const PROP_OPACITY: u32 = 6;
const PROP_MODE: u32 = 7;
const PROP_VISIBLE: u32 = 8;
const PROP_OFFSETS: u32 = 15;
const PROP_COMPRESSION: u32 = 17;
const PROP_PARASITES: u32 = 21;
const PROP_GROUP_ITEM: u32 = 29;
const PROP_ITEM_PATH: u32 = 30;

#[derive(Clone)]
struct Lay {
    name: &'static str,
    ty: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    mode: u32,
    opacity: u32,
    visible: bool,
    /// Stored samples, `bpp` bytes per pixel, rows top first.
    px: Vec<u8>,
    /// Stored mask samples (one per pixel), if any.
    mask: Option<Vec<u8>>,
    group: bool,
    path: Option<Vec<u32>>,
}

impl Lay {
    fn new(name: &'static str, ty: u32, x: i32, y: i32, w: u32, h: u32, px: Vec<u8>) -> Lay {
        Lay {
            name,
            ty,
            x,
            y,
            w,
            h,
            mode: 28,
            opacity: 255,
            visible: true,
            px,
            mask: None,
            group: false,
            path: None,
        }
    }
}

struct Spec {
    version: u32,
    base: u32,
    /// Written for v4+.
    precision: u32,
    /// Bytes per sample.
    sample: usize,
    compression: u8,
    w: u32,
    h: u32,
    icc: Option<Vec<u8>>,
    /// Top first.
    layers: Vec<Lay>,
}

struct W {
    out: Vec<u8>,
    wide: bool,
}

impl W {
    fn u32(&mut self, v: u32) {
        self.out.extend(v.to_be_bytes());
    }
    fn ptr(&mut self, v: u64) {
        if self.wide {
            self.out.extend(v.to_be_bytes());
        } else {
            self.out.extend((v as u32).to_be_bytes());
        }
    }
    fn string(&mut self, s: &str) {
        self.u32(s.len() as u32 + 1);
        self.out.extend(s.as_bytes());
        self.out.push(0);
    }
    fn prop(&mut self, ty: u32, body: &[u8]) {
        self.u32(ty);
        self.u32(body.len() as u32);
        self.out.extend(body);
    }
    fn pos(&self) -> u64 {
        self.out.len() as u64
    }
    fn patch(&mut self, at: u64, v: u64) {
        let at = at as usize;
        if self.wide {
            self.out[at..at + 8].copy_from_slice(&v.to_be_bytes());
        } else {
            self.out[at..at + 4].copy_from_slice(&(v as u32).to_be_bytes());
        }
    }
}

fn rle(planes: &[u8], bpp: usize) -> Vec<u8> {
    let n = planes.len() / bpp;
    let mut out = Vec::new();
    for k in 0..bpp {
        let s: Vec<u8> = (0..n).map(|i| planes[i * bpp + k]).collect();
        let mut i = 0;
        while i < n {
            let mut run = 1;
            while i + run < n && s[i + run] == s[i] {
                run += 1;
            }
            if run >= 3 {
                if run <= 127 {
                    out.push((run - 1) as u8);
                } else {
                    out.push(127);
                    out.extend((run as u16).to_be_bytes());
                }
                out.push(s[i]);
                i += run;
            } else {
                let mut j = i;
                while j < n && !(j + 2 < n && s[j] == s[j + 1] && s[j] == s[j + 2]) {
                    j += 1;
                }
                let lit = j - i;
                if lit <= 127 {
                    out.push((256 - lit) as u8);
                } else {
                    out.push(128);
                    out.extend((lit as u16).to_be_bytes());
                }
                out.extend(&s[i..j]);
                i = j;
            }
        }
    }
    out
}

fn zlib(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; data.len() + 1024];
    let (c, rc) = zlib_rs::compress_slice(&mut out, data, zlib_rs::DeflateConfig::default());
    assert_eq!(rc, zlib_rs::ReturnCode::Ok);
    c.to_vec()
}

/// Writes a tiled buffer (hierarchy, level, tiles); returns its hierarchy offset.
fn buffer(w: &mut W, spec: &Spec, width: u32, height: u32, bpp: usize, px: &[u8]) -> u64 {
    let hier = w.pos();
    w.u32(width);
    w.u32(height);
    w.u32(bpp as u32);
    let level_ptr = w.pos();
    w.ptr(0);
    w.ptr(0);
    let level = w.pos();
    w.patch(level_ptr, level);
    w.u32(width);
    w.u32(height);
    let (tx, ty) = (width.div_ceil(64), height.div_ceil(64));
    let table = w.pos();
    for _ in 0..=tx * ty {
        w.ptr(0);
    }
    let step = if w.wide { 8 } else { 4 };
    for t in 0..ty {
        for s in 0..tx {
            let (x0, y0) = (s * 64, t * 64);
            let (tw, th) = (64.min(width - x0), 64.min(height - y0));
            let mut tile = Vec::new();
            for y in y0..y0 + th {
                let a = ((y * width + x0) as usize) * bpp;
                tile.extend(&px[a..a + tw as usize * bpp]);
            }
            let at = w.pos();
            w.patch(table + u64::from(t * tx + s) * step, at);
            let body = match spec.compression {
                0 => tile,
                1 => rle(&tile, bpp),
                _ => zlib(&tile),
            };
            w.out.extend(body);
        }
    }
    hier
}

fn write(spec: &Spec) -> Vec<u8> {
    let mut w = W {
        out: Vec::new(),
        wide: spec.version >= 11,
    };
    if spec.version == 0 {
        w.out.extend(b"gimp xcf file\0");
    } else {
        w.out
            .extend(format!("gimp xcf v{:03}\0", spec.version).as_bytes());
    }
    w.u32(spec.w);
    w.u32(spec.h);
    w.u32(spec.base);
    if spec.version >= 4 {
        w.u32(spec.precision);
    }
    w.prop(PROP_COMPRESSION, &[spec.compression]);
    if let Some(icc) = &spec.icc {
        let mut p = Vec::new();
        p.extend(12u32.to_be_bytes());
        p.extend(b"icc-profile\0");
        p.extend(1u32.to_be_bytes());
        p.extend((icc.len() as u32).to_be_bytes());
        p.extend(icc);
        w.prop(PROP_PARASITES, &p);
    }
    w.prop(PROP_END, &[]);
    let list = w.pos();
    for _ in 0..=spec.layers.len() {
        w.ptr(0);
    }
    w.ptr(0); // no channels
    let step = if w.wide { 8 } else { 4 };
    for (i, l) in spec.layers.iter().enumerate() {
        let at = w.pos();
        w.patch(list + i as u64 * step, at);
        w.u32(l.w);
        w.u32(l.h);
        w.u32(l.ty);
        w.string(l.name);
        w.prop(PROP_OPACITY, &l.opacity.to_be_bytes());
        w.prop(PROP_MODE, &l.mode.to_be_bytes());
        w.prop(PROP_VISIBLE, &u32::from(l.visible).to_be_bytes());
        let mut off = l.x.to_be_bytes().to_vec();
        off.extend(l.y.to_be_bytes());
        w.prop(PROP_OFFSETS, &off);
        if l.group {
            w.prop(PROP_GROUP_ITEM, &[]);
        }
        if let Some(path) = &l.path {
            let b: Vec<u8> = path.iter().flat_map(|v| v.to_be_bytes()).collect();
            w.prop(PROP_ITEM_PATH, &b);
        }
        w.prop(PROP_END, &[]);
        let ptrs = w.pos();
        w.ptr(0);
        w.ptr(0);
        let ch = match l.ty {
            0 => 3,
            1 => 4,
            2 => 1,
            3 => 2,
            _ => unreachable!(),
        };
        let bpp = ch * spec.sample;
        let px = if l.group {
            vec![0; (l.w * l.h) as usize * bpp]
        } else {
            l.px.clone()
        };
        let hier = buffer(&mut w, spec, l.w, l.h, bpp, &px);
        w.patch(ptrs, hier);
        if let Some(m) = &l.mask {
            let at = w.pos();
            w.patch(ptrs + step, at);
            w.u32(l.w);
            w.u32(l.h);
            w.string("mask");
            w.prop(PROP_END, &[]);
            let hp = w.pos();
            w.ptr(0);
            let hier = buffer(&mut w, spec, l.w, l.h, spec.sample, m);
            w.patch(hp, hier);
        }
    }
    w.out
}

/// Deterministic opaque RGB backdrop.
fn backdrop(w: u32, h: u32) -> Vec<u8> {
    (0..w * h)
        .flat_map(|i| {
            let (x, y) = (i % w, i / w);
            [(x * 3) as u8, (y * 5) as u8, ((x ^ y) * 7) as u8]
        })
        .collect()
}

/// RGBA with alpha 0 or 255 only (so normal compositing is exact).
fn cutout(w: u32, h: u32) -> Vec<u8> {
    (0..w * h)
        .flat_map(|i| {
            let (x, y) = (i % w, i / w);
            let a = if (x / 7 + y / 5) % 3 == 0 { 0 } else { 255 };
            [200 - (x % 50) as u8, (y * 2) as u8, 90, a]
        })
        .collect()
}

fn basic(version: u32, compression: u8) -> Spec {
    let (cw, ch) = (90, 70);
    let top = Lay::new("top", RGBA, 7, -5, 100, 66, cutout(100, 66));
    Spec {
        version,
        base: 0,
        precision: 150,
        sample: 1,
        compression,
        w: cw,
        h: ch,
        icc: None,
        layers: vec![
            top,
            Lay::new(
                "hidden",
                RGB,
                0,
                0,
                cw,
                ch,
                vec![255; (cw * ch * 3) as usize],
            ),
            Lay::new("backdrop", RGB, 0, 0, cw, ch, backdrop(cw, ch)),
        ],
    }
    .hide(1)
}

impl Spec {
    fn hide(mut self, i: usize) -> Spec {
        self.layers[i].visible = false;
        self
    }
}

/// The expected composite of [`basic`]: the cut-out over the backdrop.
fn expected_basic() -> Vec<u8> {
    let (cw, ch) = (90u32, 70u32);
    let b = backdrop(cw, ch);
    let t = cutout(100, 66);
    let mut out = Vec::new();
    for y in 0..ch as i32 {
        for x in 0..cw as i32 {
            let (lx, ly) = (x - 7, y + 5);
            let i = (y as u32 * cw + x as u32) as usize * 3;
            let mut p = [b[i], b[i + 1], b[i + 2], 255];
            if (0..100).contains(&lx) && (0..66).contains(&ly) {
                let j = (ly as usize * 100 + lx as usize) * 4;
                if t[j + 3] == 255 {
                    p = [t[j], t[j + 1], t[j + 2], 255];
                }
            }
            out.extend(p);
        }
    }
    out
}

fn render(data: &[u8]) -> specular_xcf::Image {
    parse(data)
        .unwrap()
        .render(data, &Options::default())
        .unwrap()
}

#[test]
fn offsets_and_compressions() {
    let want = expected_basic();
    for version in [1, 3, 8, 11] {
        for compression in [0, 1, 2] {
            if compression == 2 && version < 8 {
                continue;
            }
            let data = write(&basic(version, compression));
            let x = parse(&data).unwrap();
            let info = x.info();
            assert_eq!(info.version, version);
            assert_eq!((info.width, info.height), (90, 70));
            assert_eq!(info.base, BaseType::Rgb);
            assert_eq!(
                info.compression,
                [Compression::None, Compression::Rle, Compression::Zlib][compression as usize]
            );
            assert_eq!((info.layer_count, info.group_count), (3, 0));
            assert!(!info.has_alpha, "opaque full-canvas backdrop");
            let names: Vec<_> = x.layers().iter().map(|l| l.name.as_str()).collect();
            assert_eq!(names, ["top", "hidden", "backdrop"]);
            assert!(!x.layers()[1].visible);
            assert_eq!((x.layers()[0].x, x.layers()[0].y), (7, -5));
            assert_eq!(x.output_format(), PixelFormat::Rgba8);
            for parallel in [false, true] {
                let img = x
                    .render(
                        &data,
                        &Options {
                            parallel,
                            cancel: None,
                        },
                    )
                    .unwrap();
                assert_eq!(img.format, PixelFormat::Rgba8);
                assert!(!img.truncated);
                assert!(
                    img.data == want,
                    "v{version} compression {compression} parallel {parallel}"
                );
            }
        }
    }
}

#[test]
fn sixteen_bit_big_endian() {
    // v12: big-endian samples; u16 non-linear.
    let (w, h) = (70u32, 65u32);
    let px: Vec<u8> = (0..w * h)
        .flat_map(|i| {
            let v = (i * 977 % 65536) as u16;
            let mut p = Vec::new();
            for c in [v, v ^ 0x5555, 65535 - v] {
                p.extend(c.to_be_bytes());
            }
            p
        })
        .collect();
    for compression in [0, 1, 2] {
        let spec = Spec {
            version: 12,
            base: 0,
            precision: 250,
            sample: 2,
            compression,
            w,
            h,
            icc: Some(b"fake profile".to_vec()),
            layers: vec![Lay::new("only", RGB, 0, 0, w, h, px.clone())],
        };
        let data = write(&spec);
        let x = parse(&data).unwrap();
        assert_eq!(x.icc(&data), Some(&b"fake profile"[..]));
        let img = render(&data);
        assert_eq!(img.format, PixelFormat::Rgba16);
        for (i, p) in img.data.chunks_exact(8).enumerate() {
            let s = &px[i * 6..i * 6 + 6];
            let want = [
                u16::from_be_bytes([s[0], s[1]]),
                u16::from_be_bytes([s[2], s[3]]),
                u16::from_be_bytes([s[4], s[5]]),
                65535,
            ];
            let got: Vec<u16> = p
                .chunks_exact(2)
                .map(|b| u16::from_ne_bytes([b[0], b[1]]))
                .collect();
            assert_eq!(got, want, "pixel {i}");
        }
    }
}

#[test]
fn grey_alpha_masks_and_groups() {
    // A grey+alpha layer in a group, masked to nothing on its left half; nothing below.
    let (w, h) = (40u32, 30u32);
    let px: Vec<u8> = (0..w * h).flat_map(|i| [(i % 251) as u8, 255]).collect();
    let mask: Vec<u8> = (0..w * h)
        .map(|i| if i % w < w / 2 { 0 } else { 255 })
        .collect();
    let mut g = Lay::new("group", GREYA, 0, 0, w, h, Vec::new());
    g.group = true;
    g.path = Some(vec![0]);
    let mut l = Lay::new("leaf", GREYA, 0, 0, w, h, px.clone());
    l.mask = Some(mask);
    l.path = Some(vec![0, 0]);
    let spec = Spec {
        version: 11,
        base: 1,
        precision: 150,
        sample: 1,
        compression: 1,
        w,
        h,
        icc: None,
        layers: vec![g, l],
    };
    let data = write(&spec);
    let x = parse(&data).unwrap();
    let info = x.info();
    assert_eq!((info.layer_count, info.group_count), (1, 1));
    assert!(info.has_alpha);
    assert_eq!(x.layers()[1].depth, 1);
    assert!(x.layers()[1].has_mask);
    assert_eq!(x.output_format(), PixelFormat::GrayAlpha16);
    let img = render(&data);
    for (i, p) in img.data.chunks_exact(4).enumerate() {
        let g = u16::from_ne_bytes([p[0], p[1]]);
        let a = u16::from_ne_bytes([p[2], p[3]]);
        if i as u32 % w < w / 2 {
            assert_eq!((g, a), (0, 0), "masked pixel {i}");
        } else {
            assert_eq!(a, 65535);
            assert_eq!(g, u16::from(px[i * 2]) * 257, "pixel {i}");
        }
    }
}

#[test]
fn opacity_zero_and_invisible() {
    let mut s = basic(11, 1);
    s.layers[0].opacity = 0;
    s.layers[2].visible = false;
    let data = write(&s);
    let x = parse(&data).unwrap();
    assert!(x.info().has_alpha);
    let img = render(&data);
    assert!(img.data.iter().all(|&v| v == 0));
}

#[test]
fn missing_and_corrupt_tiles() {
    let data = write(&basic(11, 2));
    // Cut inside the last tile of the bottom layer: earlier tiles still render.
    let cut = &data[..data.len() - 20];
    let x = parse(cut).unwrap();
    let img = x.render(cut, &Options::default()).unwrap();
    assert!(img.truncated);
    let want = expected_basic();
    assert_eq!(img.data[..4 * 90 * 10], want[..4 * 90 * 10]);

    // Garbage zlib data in every tile: transparent, flagged.
    let mut bad = data.clone();
    let mut s = basic(11, 2);
    s.layers.clear();
    let head = write(&s).len();
    for b in bad[head + 400..].iter_mut() {
        *b ^= 0xa5;
    }
    if let Ok(x) = parse(&bad)
        && let Ok(img) = x.render(&bad, &Options::default())
    {
        assert!(img.truncated);
    }

    // Every prefix parses or fails cleanly and never panics.
    for n in (0..data.len()).step_by(37) {
        if let Ok(x) = parse(&data[..n]) {
            let _ = x.render(&data[..n], &Options::default());
        }
    }
}

#[test]
fn cancel() {
    let data = write(&basic(11, 1));
    let x = parse(&data).unwrap();
    let stop = || true;
    let r = x.render(
        &data,
        &Options {
            parallel: true,
            cancel: Some(&stop),
        },
    );
    assert_eq!(r.unwrap_err(), Error::Cancelled);
}

#[test]
fn probing_and_containers() {
    assert!(!probe(&[]));
    assert!(!probe(b"gimp xcf "));
    assert!(probe(b"gimp xcf file\0"));
    assert!(probe(b"gimp xcf v011\0"));
    assert!(!probe(b"gimp xcf v01a\0"));
    assert!(!probe_file(&[]));
    assert_eq!(parse(&[]).unwrap_err(), Error::NotXcf);
    assert_eq!(parse(b"GIMP xcf file\0").unwrap_err(), Error::NotXcf);

    let data = write(&basic(3, 1));
    assert!(probe_file(&data));
    assert_eq!(decompress(&data).unwrap(), None);
    let mut out = vec![0u8; data.len() + 1024];
    let cfg = zlib_rs::DeflateConfig {
        window_bits: 31,
        ..Default::default()
    };
    let (gz, rc) = zlib_rs::compress_slice(&mut out, &data, cfg);
    assert_eq!(rc, zlib_rs::ReturnCode::Ok);
    assert!(probe_file(gz));
    assert!(!probe(gz));
    assert_eq!(decompress(gz).unwrap().unwrap(), data);
}

#[test]
fn oversized_header() {
    let mut s = basic(11, 1);
    s.w = 600_000;
    let data = write(&s);
    assert!(parse(&data).is_err());
}

#[test]
fn layers_alone_on_the_canvas() {
    // basic(): "top" (cut-out at 7,-5), "hidden" (white, hidden), "backdrop"; give "top" a
    // mask over its left half, half opacity, and a multiply mode that is not applied.
    let mut s = basic(11, 1);
    let (lw, lh) = (100u32, 66u32);
    s.layers[0].mask = Some(
        (0..lw * lh)
            .map(|i| if i % lw < lw / 2 { 255 } else { 0 })
            .collect(),
    );
    s.layers[0].opacity = 128;
    s.layers[0].mode = 30; // multiply
    let data = write(&s);
    let x = parse(&data).unwrap();
    assert_eq!(x.pixel_layers(), vec![0, 1, 2]);
    assert_eq!(x.layer_format(), PixelFormat::Rgba8);
    let opts = Options::default();

    let top = x.render_layer(&data, 0, &opts).unwrap();
    assert_eq!(
        (top.width, top.height, top.format),
        (90, 70, PixelFormat::Rgba8)
    );
    assert!(!top.truncated);
    let t = cutout(lw, lh);
    for y in 0..70i32 {
        for x in 0..90i32 {
            let p = &top.data[(y as usize * 90 + x as usize) * 4..][..4];
            let (lx, ly) = (x - 7, y + 5);
            let inside = (0..lw as i32).contains(&lx) && (0..lh as i32).contains(&ly);
            if !inside || lx >= lw as i32 / 2 {
                assert_eq!(p[3], 0, "({x},{y}) outside the layer or masked");
                continue;
            }
            let j = (ly as usize * lw as usize + lx as usize) * 4;
            if t[j + 3] == 0 {
                assert_eq!(p[3], 0, "({x},{y}) transparent in the layer");
            } else {
                assert_eq!(p, [t[j], t[j + 1], t[j + 2], 128], "({x},{y})");
            }
        }
    }

    // A hidden layer renders; an opaque RGB layer stays opaque.
    let hidden = x.render_layer(&data, 1, &opts).unwrap();
    assert!(hidden.data.chunks_exact(4).all(|p| p == [255; 4]));
    let back = x.render_layer(&data, 2, &opts).unwrap();
    let b = backdrop(90, 70);
    for (p, q) in back.data.chunks_exact(4).zip(b.chunks_exact(3)) {
        assert_eq!(p, [q[0], q[1], q[2], 255]);
    }

    // Indices past the end are not layers; cancellation stops it.
    assert!(x.render_layer(&data, 3, &opts).is_err());
    let stop = || true;
    let r = x.render_layer(
        &data,
        0,
        &Options {
            parallel: false,
            cancel: Some(&stop),
        },
    );
    assert_eq!(r.unwrap_err(), Error::Cancelled);
}

#[test]
fn layer_pages_skip_groups_and_take_grey() {
    let (w, h) = (40u32, 30u32);
    let px: Vec<u8> = (0..w * h).flat_map(|i| [(i % 251) as u8, 255]).collect();
    let mut g = Lay::new("group", GREYA, 0, 0, w, h, Vec::new());
    g.group = true;
    g.path = Some(vec![0]);
    g.opacity = 0; // a group's opacity does not reach its layers' pages
    let mut l = Lay::new("leaf", GREYA, 3, 2, w, h, px.clone());
    l.path = Some(vec![0, 0]);
    let spec = Spec {
        version: 11,
        base: 1,
        precision: 150,
        sample: 1,
        compression: 1,
        w,
        h,
        icc: None,
        layers: vec![g, l],
    };
    let data = write(&spec);
    let x = parse(&data).unwrap();
    assert_eq!(x.pixel_layers(), vec![1]);
    assert!(x.render_layer(&data, 0, &Options::default()).is_err());
    let img = x.render_layer(&data, 1, &Options::default()).unwrap();
    assert_eq!(img.format, PixelFormat::GrayAlpha16);
    let at = |x: u32, y: u32| {
        let p = &img.data[((y * w + x) * 4) as usize..][..4];
        (
            u16::from_ne_bytes([p[0], p[1]]),
            u16::from_ne_bytes([p[2], p[3]]),
        )
    };
    assert_eq!(at(2, 5).1, 0);
    assert_eq!(at(5, 1).1, 0);
    assert_eq!(at(3, 2), (u16::from(px[0]) * 257, 65535));
    assert_eq!(
        at(10, 4),
        (u16::from(px[((2 * w + 7) * 2) as usize]) * 257, 65535)
    );
}

#[test]
fn damaged_layer_pages() {
    let data = write(&basic(11, 2));
    // Cut inside the bottom layer's last tile: that page is truncated, never a panic.
    let cut = &data[..data.len() - 20];
    let x = parse(cut).unwrap();
    let img = x.render_layer(cut, 2, &Options::default()).unwrap();
    assert!(img.truncated);
    for n in (0..data.len()).step_by(41) {
        if let Ok(x) = parse(&data[..n]) {
            for i in x.pixel_layers() {
                let _ = x.render_layer(&data[..n], i, &Options::default());
            }
        }
    }
}

/// A file listing `k` layers: one layer `k` times, or (`shared`) `k` layer headers sharing
/// one pixel buffer; the buffer is a `size²` RGB backdrop.
fn repeated(k: usize, size: u32, shared: bool) -> Vec<u8> {
    let spec = Spec {
        version: 0,
        base: RGB,
        precision: 150,
        sample: 1,
        compression: 0,
        w: size,
        h: size,
        icc: None,
        layers: Vec::new(),
    };
    let mut w = W {
        out: b"gimp xcf file\0".to_vec(),
        wide: false,
    };
    w.u32(size);
    w.u32(size);
    w.u32(RGB);
    w.prop(PROP_COMPRESSION, &[0]);
    w.prop(PROP_END, &[]);
    let list = w.pos();
    for _ in 0..=k {
        w.ptr(0);
    }
    w.ptr(0); // no channels
    let mut hiers = Vec::new();
    let mut first = 0;
    for i in 0..k {
        if i > 0 && !shared {
            w.patch(list + i as u64 * 4, first);
            continue;
        }
        let at = w.pos();
        if i == 0 {
            first = at;
        }
        w.patch(list + i as u64 * 4, at);
        w.u32(size);
        w.u32(size);
        w.u32(RGB);
        w.string("layer");
        w.prop(PROP_END, &[]);
        hiers.push(w.pos());
        w.ptr(0);
        w.ptr(0);
    }
    let hier = buffer(&mut w, &spec, size, size, 3, &backdrop(size, size));
    for p in hiers {
        w.patch(p, hier);
    }
    w.out
}

#[test]
fn hostile_repeated_layers_are_read_once() {
    // 20,000 references (4 bytes each, or a 26-byte header) to one 2048² buffer: each was
    // parsed and composited again, 20,000 × 4M pixels.
    let (k, size) = (20_000, 2048);
    let one = render(&repeated(1, size, false));
    assert!(!one.truncated);
    for shared in [false, true] {
        let f = repeated(k, size, shared);
        let t = std::time::Instant::now();
        let x = parse(&f).unwrap();
        let img = x.render(&f, &Options::default()).unwrap();
        let el = t.elapsed();
        assert!(el < std::time::Duration::from_secs(5), "{shared} {el:?}");
        // The first reference reads; the stack ends at a repeated layer, and a layer
        // reusing another's buffer is damaged (both like an unreadable layer).
        assert_eq!(x.layers().len(), if shared { k } else { 1 }, "{shared}");
        assert_eq!(x.pixel_layers(), vec![0], "{shared}");
        assert!(img.truncated, "{shared}");
        assert_eq!(img.data, one.data, "{shared}");
    }
}

/// 40 nested groups over one layer: each level holds a band accumulator while its children
/// draw (a few KB of file asked for 64 levels of full-width bands); now reserved up front.
#[test]
fn hostile_nested_groups_reserve_their_bands() {
    let (w, h) = (4096u32, 16u32);
    let mut layers = Vec::new();
    for k in 0..40usize {
        let mut g = Lay::new("group", GREYA, 0, 0, w, h, Vec::new());
        g.group = true;
        g.path = Some(vec![0; k + 1]);
        layers.push(g);
    }
    let mut l = Lay::new("leaf", GREYA, 0, 0, w, h, vec![200; (w * h * 2) as usize]);
    l.path = Some(vec![0; 41]);
    layers.push(l);
    let spec = Spec {
        version: 11,
        base: 1,
        precision: 150,
        sample: 1,
        compression: 1,
        w,
        h,
        icc: None,
        layers,
    };
    let data = write(&spec);
    let x = parse(&data).unwrap();
    let small = specular_core::budget::with_limit(8 << 20, || x.render(&data, &Options::default()));
    assert!(
        matches!(small, Err(specular_xcf::Error::NeedsMemory(_))),
        "{:?}",
        small.err()
    );
    let ok = specular_core::budget::with_limit(512 << 20, || x.render(&data, &Options::default()));
    assert!(ok.is_ok());
}
