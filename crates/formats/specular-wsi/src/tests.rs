//! Synthetic slides built in-test: SVS, NDPI, SCN, generic pyramids and OME-TIFF; reads
//! against their source pixels, and damage (truncation, bad offsets, cycles, huge sizes).

#![cfg_attr(not(test), allow(dead_code))]

use crate::*;

/// A TIFF tag value.
pub enum V {
    S(Vec<u16>),
    L(Vec<u32>),
    A(String),
    R(Vec<(u32, u32)>),
    F(Vec<f32>),
    /// Blobs written to the file; the tag holds their offsets (LONG).
    Data(Vec<Vec<u8>>),
    /// SubIFDs written to the file; the tag holds their offsets.
    Sub(Vec<Vec<(u16, V)>>),
}

fn align(out: &mut Vec<u8>) {
    if out.len() % 2 == 1 {
        out.push(0);
    }
}

fn write_ifd(out: &mut Vec<u8>, entries: &[(u16, V)]) -> u32 {
    let mut resolved: Vec<(u16, u16, u32, Vec<u8>)> = Vec::new();
    for (tag, v) in entries {
        let (typ, count, bytes) = match v {
            V::S(x) => (3, x.len(), x.iter().flat_map(|v| v.to_le_bytes()).collect()),
            V::L(x) => (4, x.len(), x.iter().flat_map(|v| v.to_le_bytes()).collect()),
            V::A(s) => {
                let mut b = s.as_bytes().to_vec();
                b.push(0);
                (2, b.len(), b)
            }
            V::R(x) => (
                5,
                x.len(),
                x.iter()
                    .flat_map(|(a, b)| a.to_le_bytes().into_iter().chain(b.to_le_bytes()))
                    .collect(),
            ),
            V::F(x) => (
                11,
                x.len(),
                x.iter().flat_map(|v| v.to_le_bytes()).collect(),
            ),
            V::Data(blobs) => {
                let mut offs = Vec::new();
                for b in blobs {
                    align(out);
                    offs.push(out.len() as u32);
                    out.extend_from_slice(b);
                }
                (
                    4,
                    offs.len(),
                    offs.iter().flat_map(|v| v.to_le_bytes()).collect(),
                )
            }
            V::Sub(ifds) => {
                let offs: Vec<u32> = ifds.iter().map(|i| write_ifd(out, i)).collect();
                (
                    4,
                    offs.len(),
                    offs.iter().flat_map(|v| v.to_le_bytes()).collect(),
                )
            }
        };
        resolved.push((*tag, typ, count as u32, bytes));
    }
    // Out-of-line values.
    let mut vals = Vec::new();
    for (_, _, _, b) in &resolved {
        if b.len() > 4 {
            align(out);
            vals.push(out.len() as u32);
            out.extend_from_slice(b);
        } else {
            vals.push(0);
        }
    }
    align(out);
    let at = out.len() as u32;
    let mut order: Vec<usize> = (0..resolved.len()).collect();
    order.sort_by_key(|&i| resolved[i].0);
    out.extend_from_slice(&(resolved.len() as u16).to_le_bytes());
    for i in order {
        let (tag, typ, count, b) = &resolved[i];
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(&typ.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        if b.len() > 4 {
            out.extend_from_slice(&vals[i].to_le_bytes());
        } else {
            let mut x = b.clone();
            x.resize(4, 0);
            out.extend_from_slice(&x);
        }
    }
    out.extend_from_slice(&0u32.to_le_bytes());
    at
}

/// A little-endian classic TIFF of a main chain of IFDs.
pub fn tiff(ifds: &[Vec<(u16, V)>]) -> Vec<u8> {
    let mut out = b"II*\0\0\0\0\0".to_vec();
    let mut next_at = 4usize;
    for ifd in ifds {
        let at = write_ifd(&mut out, ifd);
        out[next_at..next_at + 4].copy_from_slice(&at.to_le_bytes());
        let n = u16::from_le_bytes([out[at as usize], out[at as usize + 1]]) as usize;
        next_at = at as usize + 2 + 12 * n;
    }
    out
}

/// A smooth RGB test pattern.
pub fn pattern(w: u32, h: u32, seed: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            v.push(((x * 255) / w.max(1)) as u8);
            v.push(((y * 255) / h.max(1)) as u8);
            v.push(((x + y + seed * 40) % 256) as u8 / 2 + 64);
        }
    }
    v
}

fn jpeg(rgb: &[u8], w: u32, h: u32, restart: u16, s444: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let mut e = jpeg_encoder::Encoder::new(&mut out, 95);
    if restart > 0 {
        e.set_restart_interval(restart);
    }
    e.set_sampling_factor(if s444 {
        jpeg_encoder::SamplingFactor::R_4_4_4
    } else {
        jpeg_encoder::SamplingFactor::R_4_2_0
    });
    e.encode(rgb, w as u16, h as u16, jpeg_encoder::ColorType::Rgb)
        .unwrap();
    out
}

fn crop_rgb(rgb: &[u8], w: u32, x: u32, y: u32, cw: u32, ch: u32) -> Vec<u8> {
    let mut v = Vec::new();
    for yy in y..y + ch {
        for xx in x..x + cw {
            let i = ((yy * w + xx) * 3) as usize;
            v.extend_from_slice(rgb.get(i..i + 3).unwrap_or(&[0, 0, 0]));
        }
    }
    v
}

/// A tiled JPEG level's tags.
fn jpeg_level(rgb: &[u8], w: u32, h: u32, tile: u32, desc: Option<&str>) -> Vec<(u16, V)> {
    let mut tiles = Vec::new();
    for ty in 0..h.div_ceil(tile) {
        for tx in 0..w.div_ceil(tile) {
            let t = crop_rgb(rgb, w, tx * tile, ty * tile, tile, tile);
            tiles.push(jpeg(&t, tile, tile, 0, true));
        }
    }
    let lens = tiles.iter().map(|t| t.len() as u32).collect();
    let mut v = vec![
        (256, V::L(vec![w])),
        (257, V::L(vec![h])),
        (258, V::S(vec![8, 8, 8])),
        (259, V::S(vec![7])),
        (262, V::S(vec![6])),
        (277, V::S(vec![3])),
        (284, V::S(vec![1])),
        (322, V::L(vec![tile])),
        (323, V::L(vec![tile])),
        (324, V::Data(tiles)),
        (325, V::L(lens)),
        (530, V::S(vec![1, 1])),
    ];
    if let Some(d) = desc {
        v.push((270, V::A(d.to_owned())));
    }
    v
}

/// An uncompressed RGB strip image's tags.
pub fn rgb_strip(rgb: &[u8], w: u32, h: u32, desc: Option<&str>) -> Vec<(u16, V)> {
    let mut v = vec![
        (256, V::L(vec![w])),
        (257, V::L(vec![h])),
        (258, V::S(vec![8, 8, 8])),
        (259, V::S(vec![1])),
        (262, V::S(vec![2])),
        (273, V::Data(vec![rgb.to_vec()])),
        (277, V::S(vec![3])),
        (278, V::L(vec![h])),
        (279, V::L(vec![rgb.len() as u32])),
    ];
    if let Some(d) = desc {
        v.push((270, V::A(d.to_owned())));
    }
    v
}

fn half(rgb: &[u8], w: u32, h: u32) -> Vec<u8> {
    let (hw, hh) = (w / 2, h / 2);
    let mut v = Vec::new();
    for y in 0..hh {
        for x in 0..hw {
            for c in 0..3 {
                let p = |xx: u32, yy: u32| u32::from(rgb[((yy * w + xx) * 3 + c) as usize]);
                v.push(
                    ((p(2 * x, 2 * y)
                        + p(2 * x + 1, 2 * y)
                        + p(2 * x, 2 * y + 1)
                        + p(2 * x + 1, 2 * y + 1)
                        + 2)
                        / 4) as u8,
                );
            }
        }
    }
    v
}

/// Mean absolute difference of an RGBA read against RGB source pixels.
fn mad(rgba: &Rgba, rgb: &[u8]) -> f64 {
    let n = (rgba.width * rgba.height) as usize;
    assert_eq!(rgb.len(), n * 3);
    let mut s = 0u64;
    for i in 0..n {
        for c in 0..3 {
            s += u64::from(rgba.data[i * 4 + c].abs_diff(rgb[i * 3 + c]));
        }
    }
    s as f64 / (n * 3) as f64
}

pub const SVS_DESC: &str = "Aperio Image Library v12.0.15\r\n512x512 [0,0 512x512] (256x256) JPEG/RGB Q=95|AppMag = 20|MPP = 0.4990|ScanScope ID = SS1234|Date = 01/02/20|Time = 10:11:12";

pub fn svs() -> (Vec<u8>, Vec<u8>) {
    let (w, h) = (512, 384);
    let rgb = pattern(w, h, 0);
    let l1 = half(&rgb, w, h);
    let l2 = half(&l1, w / 2, h / 2);
    let thumb = pattern(64, 48, 1);
    let label = pattern(40, 30, 2);
    let mac = pattern(80, 30, 3);
    let file = tiff(&[
        jpeg_level(&rgb, w, h, 256, Some(SVS_DESC)),
        rgb_strip(
            &thumb,
            64,
            48,
            Some("Aperio Image Library v12\r\n512x384 -> 64x48"),
        ),
        jpeg_level(
            &l1,
            w / 2,
            h / 2,
            128,
            Some("Aperio Image Library v12\r\n256x192"),
        ),
        jpeg_level(
            &l2,
            w / 4,
            h / 4,
            64,
            Some("Aperio Image Library v12\r\n128x96"),
        ),
        rgb_strip(
            &label,
            40,
            30,
            Some("Aperio Image Library v12\r\nlabel 40x30"),
        ),
        rgb_strip(
            &mac,
            80,
            30,
            Some("Aperio Image Library v12\r\nmacro 80x30"),
        ),
    ]);
    (file, rgb)
}

fn ro(scale: u32, region: Option<Region>) -> ReadOptions<'static> {
    ReadOptions {
        scale,
        region,
        position: Position::default(),
        cancel: None,
        parallel: true,
    }
}

#[test]
fn aperio_levels_props_and_regions() {
    let (file, rgb) = svs();
    assert_eq!(probe_tiff(&file), Some(Vendor::Aperio));
    let s = Slide::open_tiff(&file).unwrap();
    assert_eq!((s.width, s.height), (512, 384));
    assert_eq!(s.levels.len(), 3);
    assert_eq!(s.props.mpp, Some((0.499, 0.499)));
    assert_eq!(s.props.objective, Some(20.0));
    assert_eq!(s.props.scanner.as_deref(), Some("SS1234"));
    let kinds: Vec<_> = s.associated.iter().map(|a| a.0.clone()).collect();
    assert_eq!(
        kinds,
        vec![
            AssociatedKind::Thumbnail,
            AssociatedKind::Label,
            AssociatedKind::Macro
        ]
    );
    assert_eq!(s.associated[1].1, 40);
    let files = [file.as_slice()];
    let f = Files { data: &files };
    let r = Region {
        x: 200,
        y: 100,
        w: 150,
        h: 120,
    };
    let px = s.read(f, &ro(1, Some(r))).unwrap();
    assert_eq!((px.width, px.height), (150, 120));
    assert!(mad(&px, &crop_rgb(&rgb, 512, 200, 100, 150, 120)) < 2.0);
    // 1/4 comes from level 2, 1/16 from level 2 box-reduced.
    let px = s.read(f, &ro(4, None)).unwrap();
    assert_eq!((px.width, px.height), (128, 96));
    let px = s.read(f, &ro(16, None)).unwrap();
    assert_eq!((px.width, px.height), (32, 24));
    let lab = s.read_associated(f, 1, None).unwrap();
    assert_eq!((lab.width, lab.height), (40, 30));
    assert!(mad(&lab, &pattern(40, 30, 2)) < 0.01);
    assert!(s.max_scale() >= 32);
}

/// An NDPI: level 0 one JPEG with restart markers, level 1 one without, and a macro.
pub fn ndpi(mcu_starts: bool, s444: bool) -> (Vec<u8>, Vec<u8>) {
    let (w, h) = (256u32, 96u32);
    let rgb = pattern(w, h, 4);
    let mcu = if s444 { 8 } else { 16 };
    let ri = (w / mcu / 4) as u16; // four intervals per row
    let j0 = jpeg(&rgb, w, h, ri, s444);
    let l1 = half(&half(&rgb, w, h), w / 2, h / 2);
    let j1 = jpeg(&l1, w / 4, h / 4, 0, s444);
    let mac = pattern(64, 24, 5);
    let jm = jpeg(&mac, 64, 24, 0, false);
    let level = |j: Vec<u8>, w: u32, h: u32, mag: f32, extra: Vec<(u16, V)>| {
        let mut v = vec![
            (256, V::L(vec![w])),
            (257, V::L(vec![h])),
            (258, V::S(vec![8, 8, 8])),
            (259, V::S(vec![7])),
            (262, V::S(vec![6])),
            (277, V::S(vec![3])),
            (278, V::L(vec![h])),
            (279, V::L(vec![j.len() as u32])),
            (273, V::Data(vec![j])),
            (282, V::R(vec![(20000, 1)])),
            (283, V::R(vec![(20000, 1)])),
            (296, V::S(vec![3])),
            (272, V::A("NanoZoomer".into())),
            (65420, V::L(vec![1])),
            (65421, V::F(vec![mag])),
            // The image centre 3 µm left of and 1.5 µm below the slide centre.
            (65422, V::L(vec![(-3000i32) as u32])),
            (65423, V::L(vec![1500])),
        ];
        v.extend(extra);
        v
    };
    let mut extra = Vec::new();
    if mcu_starts {
        // Restart interval starts within the strip, from a scan of the encoder's output.
        let hd = crate::ndpi::tests_header_end(&j0);
        let starts = crate::ndpi::tests_scan(&j0, hd);
        let n = (w / mcu / u32::from(ri)) * h.div_ceil(mcu);
        extra.push((
            65426,
            V::L(starts[..n as usize].iter().map(|&v| v as u32).collect()),
        ));
    }
    let file = tiff(&[
        level(j0, w, h, 20.0, extra),
        level(j1, w / 4, h / 4, 20.0, vec![]),
        level(jm, 64, 24, -1.0, vec![]),
    ]);
    (file, rgb)
}

#[test]
fn ndpi_regions_splice_restart_intervals() {
    for (starts, s444) in [(true, true), (false, true), (true, false)] {
        let (file, rgb) = ndpi(starts, s444);
        assert_eq!(probe_tiff(&file), Some(Vendor::Hamamatsu));
        let s = Slide::open_tiff(&file).unwrap();
        assert_eq!((s.width, s.height, s.levels.len()), (256, 96, 2));
        assert_eq!(s.props.objective, Some(20.0));
        let mpp = s.props.mpp.unwrap();
        assert!((mpp.0 - 0.5).abs() < 1e-9, "{mpp:?}");
        assert_eq!(s.props.centre_offset_nm, Some((-3000.0, 1500.0)));
        assert_eq!(s.associated[0].0, AssociatedKind::Macro);
        let files = [file.as_slice()];
        let f = Files { data: &files };
        for (sc, r) in [
            (
                1,
                Region {
                    x: 70,
                    y: 20,
                    w: 100,
                    h: 50,
                },
            ),
            (
                1,
                Region {
                    x: 0,
                    y: 0,
                    w: 256,
                    h: 96,
                },
            ),
            (
                2,
                Region {
                    x: 10,
                    y: 5,
                    w: 100,
                    h: 40,
                },
            ),
            (
                4,
                Region {
                    x: 0,
                    y: 0,
                    w: 64,
                    h: 24,
                },
            ),
            (
                8,
                Region {
                    x: 3,
                    y: 1,
                    w: 29,
                    h: 11,
                },
            ),
        ] {
            let px = s.read(f, &ro(sc, Some(r))).unwrap();
            assert_eq!((px.width, px.height), (r.w, r.h));
            let mut src = rgb.clone();
            let (mut sw, mut sh) = (256, 96);
            let mut k = sc;
            while k > 1 {
                src = half(&src, sw, sh);
                sw /= 2;
                sh /= 2;
                k /= 2;
            }
            let m = mad(&px, &crop_rgb(&src, sw, r.x, r.y, r.w, r.h));
            assert!(m < 3.0, "scale {sc} {r:?} starts {starts} 444 {s444}: {m}");
        }
        let mac = s.read_associated(f, 0, None).unwrap();
        assert_eq!((mac.width, mac.height), (64, 24));
    }
}

#[test]
fn ndpi_splice_is_exact_for_444() {
    // With 4:4:4 sampling there is no chroma upsampling across interval edges: a spliced
    // region decodes to exactly the pixels of the whole JPEG.
    let (file, _) = ndpi(true, true);
    let s = Slide::open_tiff(&file).unwrap();
    let files = [file.as_slice()];
    let f = Files { data: &files };
    let all = s.read(f, &ro(1, None)).unwrap();
    let r = Region {
        x: 64,
        y: 16,
        w: 128,
        h: 48,
    };
    let part = s.read(f, &ro(1, Some(r))).unwrap();
    for y in 0..r.h {
        let a = &all.data[(((y + r.y) * 256 + r.x) * 4) as usize..][..(r.w * 4) as usize];
        let b = &part.data[(y * r.w * 4) as usize..][..(r.w * 4) as usize];
        assert_eq!(a, b, "row {y}");
    }
}

pub const SCN_DESC: &str = r#"<?xml version="1.0"?><scn xmlns="http://www.leica-microsystems.com/scn/2010/10/01">
<collection name="s" sizeX="1000000" sizeY="500000"><barcode>BC-1</barcode>
<image name="macro"><device model="Leica SCN400" version="1.5"/>
<pixels sizeX="64" sizeY="32"><dimension sizeX="64" sizeY="32" r="0" ifd="0"/></pixels>
<view sizeX="1000000" sizeY="500000" offsetX="0" offsetY="0"/></image>
<image name="main"><creationDate>2012-01-01T00:00:00Z</creationDate><device model="Leica SCN400" version="1.5"/>
<pixels sizeX="512" sizeY="256"><dimension sizeX="512" sizeY="256" r="0" ifd="1"/><dimension sizeX="128" sizeY="64" r="1" ifd="2"/></pixels>
<view sizeX="256000" sizeY="128000" offsetX="10" offsetY="10"/><scanSettings><objectiveSettings><objective>40</objective></objectiveSettings></scanSettings></image>
</collection></scn>"#;

#[test]
fn leica_scn_main_image_and_macro() {
    let rgb = pattern(512, 256, 6);
    let l1 = half(&half(&rgb, 512, 256), 256, 128);
    let file = tiff(&[
        rgb_strip(&pattern(64, 32, 7), 64, 32, Some(SCN_DESC)),
        jpeg_level(&rgb, 512, 256, 256, None),
        jpeg_level(&l1, 128, 64, 64, None),
    ]);
    assert_eq!(probe_tiff(&file), Some(Vendor::Leica));
    let s = Slide::open_tiff(&file).unwrap();
    assert_eq!((s.width, s.height, s.levels.len()), (512, 256, 2));
    assert_eq!(s.props.mpp, Some((0.5, 0.5)));
    assert_eq!(s.props.objective, Some(40.0));
    assert_eq!(s.associated[0].0, AssociatedKind::Macro);
    let files = [file.as_slice()];
    let px = s.read(Files { data: &files }, &ro(4, None)).unwrap();
    assert!(mad(&px, &l1) < 2.0);
}

/// A big generic pyramid: every tile shares one blob, so a 16384² level costs a few KiB.
pub fn generic() -> Vec<u8> {
    let tile = 256u32;
    let blob = jpeg(&pattern(tile, tile, 8), tile, tile, 0, true);
    let level = |w: u32, h: u32, sub: Option<Vec<Vec<(u16, V)>>>| {
        let n = (w.div_ceil(tile) * h.div_ceil(tile)) as usize;
        let mut v = vec![
            (256, V::L(vec![w])),
            (257, V::L(vec![h])),
            (258, V::S(vec![8, 8, 8])),
            (259, V::S(vec![7])),
            (262, V::S(vec![6])),
            (277, V::S(vec![3])),
            (322, V::L(vec![tile])),
            (323, V::L(vec![tile])),
            (325, V::L(vec![blob.len() as u32; n])),
            (530, V::S(vec![1, 1])),
            (282, V::R(vec![(40000, 1)])),
            (283, V::R(vec![(40000, 1)])),
            (296, V::S(vec![3])),
        ];
        // Placeholder offsets, patched below to the one blob.
        v.push((324, V::L(vec![0xDEAD_BEEF; n])));
        if let Some(s) = sub {
            v.push((330, V::Sub(s)));
        }
        v
    };
    let subs = vec![
        level(4096, 4096, None),
        level(1024, 1024, None),
        level(256, 256, None),
    ];
    let mut file = tiff(&[level(16384, 16384, Some(subs))]);
    let at = file.len() as u32;
    file.extend_from_slice(&blob);
    let mut i = 0;
    while i + 4 <= file.len() {
        if file[i..i + 4] == 0xDEAD_BEEFu32.to_le_bytes() {
            file[i..i + 4].copy_from_slice(&at.to_le_bytes());
            i += 4;
        } else {
            i += 2;
        }
    }
    file
}

#[test]
fn generic_pyramid_in_subifds() {
    let file = generic();
    assert!(file.len() < 100_000);
    assert_eq!(probe_tiff(&file), Some(Vendor::Generic));
    let s = Slide::open_tiff(&file).unwrap();
    assert_eq!((s.width, s.levels.len()), (16384, 4));
    assert_eq!(s.props.mpp, Some((0.25, 0.25)));
    let files = [file.as_slice()];
    let f = Files { data: &files };
    let px = s.read(f, &ro(64, None)).unwrap();
    assert_eq!((px.width, px.height), (256, 256));
    assert!(mad(&px, &pattern(256, 256, 8)) < 2.0);
    let px = s
        .read(
            f,
            &ro(
                1,
                Some(Region {
                    x: 8000,
                    y: 8000,
                    w: 300,
                    h: 300,
                }),
            ),
        )
        .unwrap();
    assert_eq!(px.width, 300);
    // Too large a read is refused before allocating.
    assert!(matches!(s.read(f, &ro(1, None)), Err(Error::TooLarge)));
}

pub fn ome_xml(own: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<OME xmlns="http://www.openmicroscopy.org/Schemas/OME/2016-06" Creator="test" UUID="urn:uuid:1">
<Image ID="Image:0" Name="cells"><Pixels ID="Pixels:0" DimensionOrder="XYCZT" Type="uint16" SizeX="64" SizeY="48" SizeZ="2" SizeC="2" SizeT="1"
 PhysicalSizeX="0.65" PhysicalSizeY="0.65" PhysicalSizeZ="1.5" PhysicalSizeZUnit="µm">
<Channel ID="Channel:0:0" Name="DAPI" Color="65535" SamplesPerPixel="1"/>
<Channel ID="Channel:0:1" Name="GFP" Color="16711935" SamplesPerPixel="1"/>
<TiffData IFD="0" PlaneCount="4"><UUID FileName="{own}">urn:uuid:1</UUID></TiffData>
</Pixels></Image>
<Image ID="Image:1" Name="label image"><Pixels ID="Pixels:1" DimensionOrder="XYCZT" Type="uint8" SizeX="16" SizeY="8" SizeZ="1" SizeC="3" SizeT="1">
<Channel ID="Channel:1:0" SamplesPerPixel="3"/><TiffData IFD="4" PlaneCount="1"/></Pixels></Image></OME>"#
    )
}

pub fn gray16(
    w: u32,
    h: u32,
    f: &dyn Fn(u32, u32) -> u16,
    desc: Option<String>,
    sub: bool,
) -> Vec<(u16, V)> {
    let mut d = Vec::new();
    for y in 0..h {
        for x in 0..w {
            d.extend_from_slice(&f(x, y).to_le_bytes());
        }
    }
    let mut v = vec![
        (256, V::L(vec![w])),
        (257, V::L(vec![h])),
        (258, V::S(vec![16])),
        (259, V::S(vec![1])),
        (262, V::S(vec![1])),
        (277, V::S(vec![1])),
        (278, V::L(vec![h])),
        (279, V::L(vec![d.len() as u32])),
        (273, V::Data(vec![d])),
    ];
    if let Some(t) = desc {
        v.push((270, V::A(t)));
    }
    if sub {
        let f2 = |x: u32, y: u32| f(2 * x, 2 * y);
        v.push((330, V::Sub(vec![gray16(w / 2, h / 2, &f2, None, false)])));
    }
    v
}

pub fn ome_tiff() -> Vec<u8> {
    let val = |c: u32, z: u32| move |x: u32, y: u32| (c * 10000 + z * 1000 + y * 64 + x) as u16;
    tiff(&[
        gray16(64, 48, &val(0, 0), Some(ome_xml("a.ome.tif")), true),
        gray16(64, 48, &val(1, 0), None, true),
        gray16(64, 48, &val(0, 1), None, true),
        gray16(64, 48, &val(1, 1), None, true),
        rgb_strip(&pattern(16, 8, 9), 16, 8, None),
    ])
}

#[test]
fn ome_tiff_planes_channels_and_subifds() {
    let file = ome_tiff();
    assert_eq!(probe_tiff(&file), Some(Vendor::OmeTiff));
    let s = Slide::open_tiff(&file).unwrap();
    assert_eq!((s.width, s.height, s.size_z, s.size_t), (64, 48, 2, 1));
    assert_eq!(s.channels.len(), 2);
    assert_eq!(s.channels[0].name, "DAPI");
    assert_eq!(s.channels[0].colour, Some([0, 0, 255]));
    assert_eq!(s.props.mpp, Some((0.65, 0.65)));
    assert_eq!(s.props.z_step, Some(1.5));
    assert_eq!(s.levels.len(), 2);
    assert!(s.multichannel());
    assert_eq!(s.associated.len(), 1);
    assert_eq!(s.associated[0].0, AssociatedKind::Label);
    let files = [file.as_slice()];
    let f = Files { data: &files };
    let mut o = ro(
        1,
        Some(Region {
            x: 3,
            y: 2,
            w: 2,
            h: 1,
        }),
    );
    o.position.z = 1;
    let p = s.read_channel(f, 1, &o).unwrap();
    assert_eq!(p.values, vec![11000.0 + 128.0 + 3.0, 11000.0 + 128.0 + 4.0]);
    let p = s
        .read_channel(
            f,
            0,
            &ro(
                2,
                Some(Region {
                    x: 1,
                    y: 1,
                    w: 1,
                    h: 1,
                }),
            ),
        )
        .unwrap();
    assert_eq!(p.values, vec![(2 * 64 + 2) as f32]);
    let px = s.read(f, &ro(1, None)).unwrap();
    assert_eq!((px.width, px.height), (64, 48));
    let lab = s.read_associated(f, 0, None).unwrap();
    assert_eq!(lab.width, 16);
}

#[test]
fn ome_tiff_in_two_files() {
    // Planes of z = 1 live in a second file of the folder.
    let xml = r#"<?xml version="1.0"?><OME xmlns="http://www.openmicroscopy.org/Schemas/OME/2016-06" UUID="urn:uuid:a">
<Image ID="Image:0"><Pixels ID="Pixels:0" DimensionOrder="XYZCT" Type="uint16" SizeX="8" SizeY="4" SizeZ="2" SizeC="1" SizeT="1">
<Channel ID="Channel:0:0" SamplesPerPixel="1"/>
<TiffData IFD="0" FirstZ="0" PlaneCount="1"><UUID FileName="a.ome.tif">urn:uuid:a</UUID></TiffData>
<TiffData IFD="0" FirstZ="1" PlaneCount="1"><UUID FileName="b.ome.tif">urn:uuid:b</UUID></TiffData>
</Pixels></Image></OME>"#;
    let a = tiff(&[gray16(8, 4, &|x, _| x as u16, Some(xml.to_owned()), false)]);
    let b = tiff(&[gray16(8, 4, &|x, _| 100 + x as u16, None, false)]);
    let s = crate::tiffslide::open_named(&a, Some("a.ome.tif")).unwrap();
    assert_eq!(s.files, vec!["b.ome.tif".to_owned()]);
    let files = [a.as_slice(), b.as_slice()];
    let mut o = ro(
        1,
        Some(Region {
            x: 2,
            y: 0,
            w: 1,
            h: 1,
        }),
    );
    o.position.z = 1;
    assert_eq!(
        s.read_channel(Files { data: &files }, 0, &o)
            .unwrap()
            .values,
        vec![102.0]
    );
    // Without the second file the plane reports an I/O error, not a panic.
    let only = [a.as_slice()];
    assert!(s.read_channel(Files { data: &only }, 0, &o).is_err());
}

/// Open and read a damaged file every way; nothing may panic.
fn exercise(data: &[u8]) {
    let _ = probe_tiff(data);
    let Ok(s) = Slide::open_tiff(data) else {
        return;
    };
    let files = [data];
    let f = Files { data: &files };
    for sc in [1, 4, 64] {
        let _ = s.read(
            f,
            &ro(
                sc,
                Some(Region {
                    x: 0,
                    y: 0,
                    w: 300,
                    h: 300,
                }),
            ),
        );
    }
    let _ = s.read(f, &ro(64, None));
    for i in 0..s.associated.len() {
        let _ = s.read_associated(f, i, None);
    }
    if s.multichannel() {
        let _ = s.read_channel(f, 0, &ro(1, None));
    }
}

#[test]
fn damage_never_panics() {
    let fixtures = [
        svs().0,
        ndpi(true, false).0,
        ndpi(false, true).0,
        generic(),
        ome_tiff(),
    ];
    for f in &fixtures {
        // Truncations.
        let step = (f.len() / 97).max(1);
        for n in (0..f.len()).step_by(step) {
            exercise(&f[..n]);
        }
        // Byte flips through the file (offsets, counts, sizes, markers).
        let step = (f.len() / 211).max(1);
        for i in (0..f.len()).step_by(step) {
            let mut g = f.clone();
            g[i] ^= 0xFF;
            exercise(&g);
            g[i] = 0xFF;
            if i + 1 < g.len() {
                g[i + 1] = 0x7F;
            }
            exercise(&g);
        }
    }
}

#[test]
fn ifd_cycle_and_huge_sizes() {
    // An IFD whose next pointer points at itself.
    let mut f = tiff(&[jpeg_level(&pattern(64, 64, 1), 64, 64, 64, Some(SVS_DESC))]);
    let at = u32::from_le_bytes([f[4], f[5], f[6], f[7]]) as usize;
    let n = u16::from_le_bytes([f[at], f[at + 1]]) as usize;
    let p = at + 2 + 12 * n;
    f[p..p + 4].copy_from_slice(&(at as u32).to_le_bytes());
    exercise(&f);
    // A level claiming 2^31 × 2^31 pixels with one tile.
    let mut lv = jpeg_level(&pattern(64, 64, 1), 64, 64, 64, Some(SVS_DESC));
    lv[0] = (256, V::L(vec![1 << 31]));
    lv[1] = (257, V::L(vec![1 << 31]));
    exercise(&tiff(&[lv]));
}

/// A VMS of 2 × 2 JPEG files (restart markers every 4 MCUs, the right column narrower),
/// with an optimisation file of each file's row starts, a macro and a map at 1/8: the
/// files in [`vms::files`] order after the index, and the whole slide's RGB.
pub fn vms_set(with_opt: bool) -> (Vec<Vec<u8>>, Vec<u8>, (u32, u32)) {
    let (cw, ch) = ([128u32, 64], [64u32, 48]);
    let (w, h) = (cw[0] + cw[1], ch[0] + ch[1]);
    let rgb = pattern(w, h, 3);
    let mut index = String::from(
        "[Virtual Microscope Specimen]\r\nNoLayers=1\r\nNoJpegColumns=2\r\nNoJpegRows=2\r\n",
    );
    let mut files = Vec::new();
    let mut opt = Vec::new();
    for r in 0..2u32 {
        for c in 0..2u32 {
            let (x, y) = (
                if c == 0 { 0 } else { cw[0] },
                if r == 0 { 0 } else { ch[0] },
            );
            let part = crop_rgb(&rgb, w, x, y, cw[c as usize], ch[r as usize]);
            let j = jpeg(&part, cw[c as usize], ch[r as usize], 4, true);
            // Row starts: every (width / 8 / 4)th interval.
            let starts = crate::ndpi::tests_scan(&j, crate::ndpi::tests_header_end(&j));
            let per_row = (cw[c as usize] / 8 / 4) as usize;
            for s in starts
                .iter()
                .step_by(per_row)
                .take((ch[r as usize] / 8) as usize)
            {
                let mut rec = [0u8; 40];
                rec[..8].copy_from_slice(&s.to_le_bytes());
                opt.extend_from_slice(&rec);
            }
            let name = if (c, r) == (0, 0) {
                "s.jpg".to_owned()
            } else {
                format!("s({c},{r}).jpg")
            };
            if (c, r) == (0, 0) {
                index += &format!("ImageFile={name}\r\n");
            } else {
                index += &format!("ImageFile({c},{r})={name}\r\n");
            }
            files.push(j);
        }
    }
    if with_opt {
        index += "OptimisationFile=s.opt\r\n";
        files.push(opt);
    }
    // The map: the slide at 1/8 (24 × 14).
    let (mw, mh) = (w.div_ceil(8), h.div_ceil(8));
    index += "MapFile=s_map.jpg\r\nMacroImage=s_macro.jpg\r\n";
    files.push(jpeg(&pattern(mw, mh, 3), mw, mh, 1, true));
    files.push(jpeg(&pattern(40, 20, 9), 40, 20, 0, false));
    index += "SourceLens=20\r\nPhysicalWidth=96000\r\nPhysicalHeight=56000\r\n";
    index += "XOffsetFromSlideCentre=1000\r\nYOffsetFromSlideCentre=-2000\r\n";
    let mut all = vec![index.into_bytes()];
    all.extend(files);
    (all, rgb, (w, h))
}

#[test]
fn vms_grid_regions_and_optimisation_file() {
    for with_opt in [true, false] {
        let (set, _rgb, (w, h)) = vms_set(with_opt);
        assert_eq!(vms::probe(&set[0]), Some(vms::Kind::Vms));
        let names = vms::files(&set[0]).unwrap();
        assert_eq!(names.len(), set.len() - 1);
        let refs: Vec<&[u8]> = set.iter().map(Vec::as_slice).collect();
        let f = Files { data: &refs };
        let s = Slide::open_vms(f).unwrap();
        assert_eq!((s.width, s.height), (w, h));
        assert_eq!(s.props.objective, Some(20.0));
        assert!((s.props.mpp.unwrap().0 - 0.5).abs() < 1e-9);
        assert_eq!(s.associated.len(), 1);
        // Full and DCT scales against each file decoded whole by the same decoder.
        for scale in [1u32, 2, 4] {
            let full = s
                .read(
                    f,
                    &ReadOptions {
                        scale,
                        ..Default::default()
                    },
                )
                .unwrap();
            assert_eq!(full.width, w.div_ceil(scale));
            for (k, (x0, y0)) in [(0usize, (0u32, 0u32)), (3, (128, 64))] {
                let j = jpeg_decode_scaled(&set[1 + k], scale);
                let (ox, oy) = (x0 / scale, y0 / scale);
                for (y, x) in [(0u32, 0u32), (3, 5), (j.1 - 1, j.0 - 1)] {
                    let a = &full.data[(((oy + y) * full.width + ox + x) * 4) as usize..][..3];
                    let b = &j.2[((y * j.0 + x) * 4) as usize..][..3];
                    assert_eq!(a, b, "1/{scale} file {k} at {x},{y}");
                }
            }
            // A region across the four files equals the same part of the whole read.
            let r = Region {
                x: 100 / scale,
                y: 50 / scale,
                w: 60 / scale,
                h: 30 / scale,
            };
            let part = s
                .read(
                    f,
                    &ReadOptions {
                        scale,
                        region: Some(r),
                        ..Default::default()
                    },
                )
                .unwrap();
            for y in 0..r.h {
                let a = &part.data[(y * r.w * 4) as usize..][..(r.w * 4) as usize];
                let b = &full.data[(((r.y + y) * full.width + r.x) * 4) as usize..]
                    [..(r.w * 4) as usize];
                assert_eq!(a, b, "1/{scale} row {y}");
            }
        }
        // 1/8 and coarser come from the map.
        let m = s
            .read(
                f,
                &ReadOptions {
                    scale: 8,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!((m.width, m.height), (24, 14));
        assert_eq!(s.read_associated(f, 0, None).unwrap().width, 40);
        assert!(s.max_scale() >= 64);
    }
}

/// The files of a VMS set (index first), each focal layer's RGB, and the slide's size.
pub type VmsLayers = (Vec<Vec<u8>>, Vec<Vec<u8>>, (u32, u32));

/// A VMS of 2 × 1 JPEG files in three focal layers (`NoLayers=4`; the fourth layer's
/// second file is not named, so three count), each layer its own pattern, with layer 0's
/// map and a macro: the files in [`vms::files`] order after the index, and each layer's
/// RGB.
pub fn vms_layers_set() -> VmsLayers {
    let (cw, h) = (64u32, 48u32);
    let w = 2 * cw;
    let mut index = String::from(
        "[Virtual Microscope Specimen]\r\nNoLayers=4\r\nNoJpegColumns=2\r\nNoJpegRows=1\r\n",
    );
    let mut files = Vec::new();
    let mut layers = Vec::new();
    for z in 0..3u32 {
        let rgb = pattern(w, h, 2 + 3 * z);
        for c in 0..2u32 {
            let part = crop_rgb(&rgb, w, c * cw, 0, cw, h);
            files.push(jpeg(&part, cw, h, 4, true));
            let name = format!("s{z}-{c}.jpg");
            index += &match (z, c) {
                (0, 0) => format!("ImageFile={name}\r\n"),
                (0, _) => format!("ImageFile({c},0)={name}\r\n"),
                _ => format!("ImageFile({z},{c},0)={name}\r\n"),
            };
        }
        layers.push(rgb);
    }
    index += "ImageFile(3,0,0)=s3-0.jpg\r\n";
    let (mw, mh) = (w.div_ceil(8), h.div_ceil(8));
    index += "MapFile=s_map.jpg\r\nMacroImage=s_macro.jpg\r\n";
    files.push(jpeg(&pattern(mw, mh, 2), mw, mh, 1, true));
    files.push(jpeg(&pattern(40, 20, 9), 40, 20, 0, false));
    let mut all = vec![index.into_bytes()];
    all.extend(files);
    (all, layers, (w, h))
}

/// Focal layers are Z planes: each reads its own files (equal to them decoded whole, at
/// full size and 1/2), layer 0 alone uses the map, a layer the index does not name every
/// file of is left out, and Z past the layers is refused.
#[test]
fn vms_focal_layers_are_z_planes() {
    let (set, layers, (w, h)) = vms_layers_set();
    let names = vms::files(&set[0]).unwrap();
    assert_eq!(names.len(), set.len() - 1);
    assert_eq!(names[2], "s1-0.jpg");
    let refs: Vec<&[u8]> = set.iter().map(Vec::as_slice).collect();
    let f = Files { data: &refs };
    let s = Slide::open_vms(f).unwrap();
    assert_eq!((s.width, s.height, s.size_z), (w, h, 3));
    assert_eq!(s.rgb_layers(), 3);
    assert!(
        s.props
            .properties
            .iter()
            .any(|(k, v)| k == "Focal layers" && v == "3")
    );
    let read = |z: u32, scale: u32| {
        s.read(
            f,
            &ReadOptions {
                scale,
                position: crate::Position { z, t: 0 },
                ..Default::default()
            },
        )
    };
    for z in 0..3u32 {
        for scale in [1u32, 2] {
            let full = read(z, scale).unwrap();
            assert_eq!(full.width, w.div_ceil(scale));
            for c in 0..2usize {
                let j = jpeg_decode_scaled(&set[1 + 2 * z as usize + c], scale);
                let ox = c as u32 * 64 / scale;
                for (y, x) in [(0u32, 0u32), (5, 7), (j.1 - 1, j.0 - 1)] {
                    let a = &full.data[((y * full.width + ox + x) * 4) as usize..][..3];
                    let b = &j.2[((y * j.0 + x) * 4) as usize..][..3];
                    assert_eq!(a, b, "layer {z} 1/{scale} file {c} at {x},{y}");
                }
            }
        }
        // Close to the layer's own pattern (JPEG at quality 95).
        let full = read(z, 1).unwrap();
        let rgb = &layers[z as usize];
        let off = (0..(w * h) as usize)
            .map(|i| {
                (0..3)
                    .map(|k| i32::from(full.data[i * 4 + k]).abs_diff(i32::from(rgb[i * 3 + k])))
                    .max()
                    .unwrap_or(0)
            })
            .filter(|&d| d > 12)
            .count();
        assert!(off < (w * h / 100) as usize, "layer {z}: {off} pixels off");
    }
    // 1/8: layer 0 from the map, the others from their files.
    let m0 = read(0, 8).unwrap();
    let m1 = read(1, 8).unwrap();
    assert_eq!((m0.width, m0.height), (m1.width, m1.height));
    assert_ne!(m0.data, m1.data);
    assert!(read(3, 1).is_err());
    assert_eq!(s.read_associated(f, 0, None).unwrap().width, 40);
}

/// `j` decoded whole at 1/`scale` by the slide's JPEG decoder: (width, height, RGBA).
fn jpeg_decode_scaled(j: &[u8], scale: u32) -> (u32, u32, Vec<u8>) {
    let l = crate::ndpi::jpeg_level(j).unwrap();
    let px = l.decode_whole(j, scale, None).unwrap();
    (px.width, px.height, px.data)
}

#[test]
fn vmu_columns_of_twelve_bit_rgb() {
    // 10 × 4 pixels in columns of 4 (4, 4, 2), value = x * 16 + y in the red channel.
    let (w, h, cw) = (10u32, 4u32, 4u32);
    let mut ngr = vec![0u8; 32];
    ngr[..2].copy_from_slice(b"GN");
    ngr[4..8].copy_from_slice(&w.to_le_bytes());
    ngr[8..12].copy_from_slice(&h.to_le_bytes());
    ngr[12..16].copy_from_slice(&cw.to_le_bytes());
    ngr[24..28].copy_from_slice(&32u32.to_le_bytes());
    for c in 0..w.div_ceil(cw) {
        let sw = cw.min(w - c * cw);
        for y in 0..h {
            for xi in 0..sw {
                let x = c * cw + xi;
                for v in [(x * 16 + y) << 4, 0x0FF0, 0x0010] {
                    ngr.extend_from_slice(&(v as u16).to_le_bytes());
                }
            }
        }
    }
    let index = b"[Uncompressed Virtual Microscope Specimen]\nImageFile=a.ngr\nBitsPerPixel=36\n";
    assert_eq!(vms::probe(index), Some(vms::Kind::Vmu));
    let refs: Vec<&[u8]> = vec![index, &ngr];
    let f = Files { data: &refs };
    let s = Slide::open_vms(f).unwrap();
    assert_eq!((s.width, s.height), (w, h));
    let px = s.read(f, &ReadOptions::default()).unwrap();
    for (x, y) in [(0u32, 0u32), (5, 2), (9, 3)] {
        let p = &px.data[((y * w + x) * 4) as usize..][..4];
        assert_eq!(p, [(x * 16 + y) as u8, 255, 1, 255], "{x},{y}");
    }
    // Truncated: missing pixels are flagged.
    let cut = &ngr[..ngr.len() - 6];
    let refs: Vec<&[u8]> = vec![index, cut];
    let f = Files { data: &refs };
    assert!(s.read(f, &ReadOptions::default()).unwrap().truncated);
}

#[cfg(test)]
mod hostile {
    use super::*;

    #[test]
    fn ome_sizes_and_plane_indices_overflow() {
        // Sizes of 2^32 - 1 once overflowed the plane count and index arithmetic and asked
        // for four billion channels; IFD numbers near 2^32 overflowed adding plane offsets.
        let max = u32::MAX;
        let xml = format!(
            r#"<?xml version="1.0"?><OME xmlns="http://www.openmicroscopy.org/Schemas/OME/2016-06">
<Image ID="Image:0" Name="cells"><Pixels ID="Pixels:0" DimensionOrder="XYTZC" Type="uint16" SizeX="8" SizeY="4" SizeZ="{max}" SizeC="{max}" SizeT="{max}"/></Image>
<Image ID="Image:1" Name="next"><Pixels ID="Pixels:1" DimensionOrder="XYZCT" Type="uint16" SizeX="8" SizeY="4" SizeZ="2" SizeC="1" SizeT="1"/></Image>
<Image ID="Image:2" Name="more"><Pixels ID="Pixels:2" DimensionOrder="XYCZT" Type="uint16" SizeX="8" SizeY="4" SizeZ="{max}" SizeC="{max}" SizeT="{max}">
<TiffData IFD="0" FirstC="{max}" FirstZ="{max}" FirstT="{max}" PlaneCount="2"/>
<TiffData IFD="{max}" PlaneCount="2"/></Pixels></Image></OME>"#
        );
        let file = tiff(&[gray16(8, 4, &|x, _| x as u16, Some(xml), false)]);
        let s = Slide::open_tiff(&file).unwrap();
        assert!(s.channels.len() <= 1 << 20);
        exercise(&file);
    }

    #[test]
    fn auto_window_of_a_subnormal_span() {
        // A span of one subnormal step makes the bin scale infinite.
        let w = crate::composite::auto_window(&[0.0, f32::from_bits(1)]);
        assert!(w.0.is_finite() && w.1.is_finite());
    }
}

/// A JPEG 2000 tile refused by the memory budget stays "needs memory" (the viewer says how
/// much and retries), not "malformed slide"; cancel and caps keep their own kinds too.
#[test]
fn jpeg_2000_refusals_keep_their_kind() {
    use specular_jp2::Jp2Error;
    let n = specular_core::NeedsMemory {
        needed: 8 << 30,
        free: 1 << 30,
    };
    assert!(matches!(
        Error::from(Jp2Error::NeedsMemory(n)),
        Error::NeedsMemory(m) if m.needed == 8 << 30
    ));
    assert!(matches!(Error::from(Jp2Error::Cancelled), Error::Cancelled));
    assert!(matches!(
        Error::from(Jp2Error::TooLarge(1)),
        Error::TooLarge
    ));
    assert!(matches!(
        Error::from(Jp2Error::Malformed("x")),
        Error::Malformed(s) if s.starts_with("JPEG 2000")
    ));
    // Damaged data is still malformed.
    assert!(matches!(
        blob::jp2_values(b"\x00\x00\x00\x0cjP  \r\n\x87\n"),
        Err(Error::Malformed(_) | Error::Truncated(_))
    ));
}
