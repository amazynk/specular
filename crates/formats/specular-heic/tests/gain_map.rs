//! Gain-map discovery on synthetic HEIF containers: an ISO 21496-1 `tmap` item deriving from
//! the primary, and Apple's `hdrgainmap` auxiliary with its own XMP. Only the container is
//! read, so the coded items carry a bare `hvcC` and dummy bytes. The same containers check
//! that an overlay's canvas and a grid's tile payloads are held to the memory budget.

use specular_heic::{APPLE_GAIN_MAP_URN, DecodeOptions, Error, GainMapKind, HeicFile};

fn boxed(ty: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut b = ((body.len() + 8) as u32).to_be_bytes().to_vec();
    b.extend_from_slice(ty);
    b.extend_from_slice(body);
    b
}

fn full_boxed(ty: &[u8; 4], version: u8, body: &[u8]) -> Vec<u8> {
    let mut b = vec![version, 0, 0, 0];
    b.extend_from_slice(body);
    boxed(ty, &b)
}

/// An `infe` (version 2); `mime` items get a content type.
fn infe(id: u16, ty: &[u8; 4], content_type: Option<&str>) -> Vec<u8> {
    let mut b = id.to_be_bytes().to_vec();
    b.extend_from_slice(&[0, 0]);
    b.extend_from_slice(ty);
    b.push(0);
    if let Some(c) = content_type {
        b.extend_from_slice(c.as_bytes());
        b.push(0);
    }
    full_boxed(b"infe", 2, &b)
}

/// An 8-bit 4:2:0 `hvcC` without parameter-set arrays.
fn hvcc() -> Vec<u8> {
    boxed(
        b"hvcC",
        &[
            1, 0x01, 0x60, 0, 0, 0, 0x90, 0, 0, 0, 0, 0, 90, 0xf0, 0, 0xfc, 0xfd, 0xf8, 0xf8, 0, 0,
            0x0f, 0,
        ],
    )
}

fn ispe(w: u32, h: u32) -> Vec<u8> {
    let mut b = w.to_be_bytes().to_vec();
    b.extend_from_slice(&h.to_be_bytes());
    full_boxed(b"ispe", 0, &b)
}

fn nclx(primaries: u16, transfer: u16) -> Vec<u8> {
    let mut b = b"nclx".to_vec();
    for v in [primaries, transfer, 1] {
        b.extend_from_slice(&v.to_be_bytes());
    }
    b.push(0x80);
    boxed(b"colr", &b)
}

fn auxc(urn: &str) -> Vec<u8> {
    let mut b = urn.as_bytes().to_vec();
    b.push(0);
    full_boxed(b"auxC", 0, &b)
}

struct Item<'a> {
    id: u16,
    ty: &'a [u8; 4],
    content_type: Option<&'a str>,
    /// 1-based indices into the property list.
    props: Vec<u8>,
    data: Vec<u8>,
    /// Extents as (offset into the `mdat` payload, length) instead of `data`.
    extents: Vec<(u32, u32)>,
}

/// A HEIF file: `items` (the first is the primary), `props`, and `refs` as
/// (type, from, to).
fn heif(items: &[Item], props: &[Vec<u8>], refs: &[(&[u8; 4], u16, &[u16])]) -> Vec<u8> {
    let mut ftyp = b"heic".to_vec();
    ftyp.extend_from_slice(&0u32.to_be_bytes());
    ftyp.extend_from_slice(b"mif1heic");
    let ftyp = boxed(b"ftyp", &ftyp);
    let meta = |offsets: &[u32], start: u32| {
        let mut hdlr = vec![0; 4];
        hdlr.extend_from_slice(b"pict");
        hdlr.extend_from_slice(&[0; 13]);
        let mut iinf = (items.len() as u16).to_be_bytes().to_vec();
        for it in items {
            iinf.extend(infe(it.id, it.ty, it.content_type));
        }
        let mut iloc = vec![0x44, 0x00];
        iloc.extend_from_slice(&(items.len() as u16).to_be_bytes());
        for (it, off) in items.iter().zip(offsets) {
            iloc.extend_from_slice(&it.id.to_be_bytes());
            iloc.extend_from_slice(&[0, 0, 0, 0]);
            if it.extents.is_empty() {
                iloc.extend_from_slice(&[0, 1]);
                iloc.extend_from_slice(&off.to_be_bytes());
                iloc.extend_from_slice(&(it.data.len() as u32).to_be_bytes());
            } else {
                iloc.extend_from_slice(&(it.extents.len() as u16).to_be_bytes());
                for (o, n) in &it.extents {
                    iloc.extend_from_slice(&(start + o).to_be_bytes());
                    iloc.extend_from_slice(&n.to_be_bytes());
                }
            }
        }
        let mut iref = Vec::new();
        for (ty, from, to) in refs {
            let mut b = from.to_be_bytes().to_vec();
            b.extend_from_slice(&(to.len() as u16).to_be_bytes());
            for t in *to {
                b.extend_from_slice(&t.to_be_bytes());
            }
            iref.extend(boxed(ty, &b));
        }
        let mut ipma = (items.len() as u32).to_be_bytes().to_vec();
        for it in items {
            ipma.extend_from_slice(&it.id.to_be_bytes());
            ipma.push(it.props.len() as u8);
            ipma.extend_from_slice(&it.props);
        }
        let mut iprp = boxed(b"ipco", &props.concat());
        iprp.extend(full_boxed(b"ipma", 0, &ipma));
        let mut m = full_boxed(b"hdlr", 0, &hdlr);
        m.extend(full_boxed(b"pitm", 0, &items[0].id.to_be_bytes()));
        m.extend(full_boxed(b"iinf", 0, &iinf));
        m.extend(full_boxed(b"iloc", 1, &iloc));
        m.extend(full_boxed(b"iref", 0, &iref));
        m.extend(boxed(b"iprp", &iprp));
        full_boxed(b"meta", 0, &m)
    };
    // The meta box has the same size whatever the offsets: size it first, then place the data.
    let start = (ftyp.len() + meta(&vec![0; items.len()], 0).len() + 8) as u32;
    let mut offsets = Vec::new();
    let mut at = start;
    for it in items {
        offsets.push(at);
        at += it.data.len() as u32;
    }
    let mut f = ftyp;
    f.extend(meta(&offsets, start));
    f.extend(boxed(
        b"mdat",
        &items
            .iter()
            .flat_map(|i| i.data.clone())
            .collect::<Vec<_>>(),
    ));
    f
}

fn coded(id: u16, props: &[u8]) -> Item<'static> {
    Item {
        id,
        ty: b"hvc1",
        content_type: None,
        props: props.to_vec(),
        data: vec![0, 0, 0, 1, 0x26],
        extents: Vec::new(),
    }
}

/// An `iovl` of 30000 × 30000 (under the canvas cap, ~36 GB of working canvas) over one tiny
/// coded item: refused under a 64 MiB memory budget before any input is decoded.
#[test]
fn overlay_canvas_within_memory_budget() {
    let props = [hvcc(), ispe(64, 48), ispe(30_000, 30_000)];
    let mut iovl = vec![0, 0]; // version, flags (16-bit fields)
    iovl.extend_from_slice(&[0; 8]); // canvas fill
    iovl.extend_from_slice(&30_000u16.to_be_bytes());
    iovl.extend_from_slice(&30_000u16.to_be_bytes());
    iovl.extend_from_slice(&[0; 4]); // the input's offsets
    let items = [
        Item {
            id: 1,
            ty: b"iovl",
            content_type: None,
            props: vec![3],
            data: iovl,
            extents: Vec::new(),
        },
        coded(2, &[0x81, 2]),
    ];
    let f = HeicFile::open(heif(&items, &props, &[(b"dimg", 1, &[2])])).unwrap();
    assert_eq!(f.info().width, 30_000);
    let o = DecodeOptions::default();
    let e = specular_core::budget::with_limit(64 << 20, || f.decode(&o)).unwrap_err();
    let Error::NeedsMemory(n) = e else {
        panic!("{e:?}")
    };
    assert!(n.needed >= 36_000_000_000 && n.needed > n.free, "{n:?}");
}

/// A grid of 64 tiles, each naming the same 1 MiB stretch of the file in two extents: each
/// tile's payload is copied together and held until the tiles decode. 64 MiB of copies from
/// a 1 MiB file are refused under a 48 MiB budget before any tile decodes.
#[test]
fn grid_tile_payloads_within_memory_budget() {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../specular-decode-host/tests/data/grad.heic");
    let src = HeicFile::open(std::fs::read(p).unwrap()).unwrap();
    let node = src.graph();
    let node = node.inputs.first().unwrap_or(node);
    let (w, h) = node.ispe().unwrap();
    let hvcc = boxed(b"hvcC", &node.properties.hvcc().unwrap().raw);
    let mut blob = src
        .container()
        .item_data(node.item.id)
        .unwrap()
        .into_owned();
    let len = blob.len();
    blob.resize(1 << 20, 0);
    let (rows, cols) = (8u16, 8u16);
    let props = [
        hvcc,
        ispe(w, h),
        ispe(w * u32::from(cols), h * u32::from(rows)),
    ];
    let mut grid = vec![0, 0, (rows - 1) as u8, (cols - 1) as u8];
    grid.extend_from_slice(&(w as u16 * cols).to_be_bytes());
    grid.extend_from_slice(&(h as u16 * rows).to_be_bytes());
    let tiles: Vec<u16> = (2..2 + rows * cols).collect();
    let file = |stretch: u32| {
        let mut items = vec![Item {
            id: 1,
            ty: b"grid",
            content_type: None,
            props: vec![3],
            data: grid.clone(),
            extents: Vec::new(),
        }];
        items.extend(tiles.iter().map(|&id| Item {
            id,
            ty: b"hvc1",
            content_type: None,
            props: vec![0x81, 2],
            data: Vec::new(),
            extents: vec![(0, stretch / 2), (stretch / 2, stretch - stretch / 2)],
        }));
        items.push(Item {
            id: 100,
            ty: b"mime",
            content_type: Some("application/octet-stream"),
            props: vec![],
            data: blob.clone(),
            extents: Vec::new(),
        });
        HeicFile::open(heif(&items, &props, &[(b"dimg", 1, &tiles)])).unwrap()
    };
    let o = DecodeOptions {
        parallel: true,
        ..Default::default()
    };
    let f = file(1 << 20);
    let e = specular_core::budget::with_limit(48 << 20, || f.decode(&o)).unwrap_err();
    assert!(matches!(e, Error::NeedsMemory(_)), "{e:?}");
    // Each tile its own payload: decodes within the same budget.
    let f = file(len as u32);
    let r = specular_core::budget::with_limit(48 << 20, || f.decode(&o));
    assert!(!matches!(r, Err(Error::NeedsMemory(_))), "{:?}", r.err());
}

#[test]
fn iso_tmap_item() {
    // 1: hvcC, 2: base ispe, 3: map ispe, 4: base nclx (Display P3), 5: alternate nclx
    // (BT.2020 PQ).
    let props = [
        hvcc(),
        ispe(64, 48),
        ispe(32, 24),
        nclx(12, 13),
        nclx(9, 16),
    ];
    let metadata = vec![0, 0, 0, 0, 0, 0x40, 1, 2, 3];
    let items = [
        coded(1, &[0x81, 2, 4]),
        coded(2, &[0x81, 3]),
        Item {
            id: 3,
            ty: b"tmap",
            content_type: None,
            props: vec![2, 5],
            data: metadata.clone(),
            extents: Vec::new(),
        },
    ];
    let f = HeicFile::open(heif(&items, &props, &[(b"dimg", 3, &[1, 2])])).unwrap();
    assert!(f.info().gain_map);
    assert_eq!(f.info().width, 64);
    let g = f.gain_map().unwrap();
    assert_eq!(g.map, 2);
    match g.kind {
        GainMapKind::Iso {
            metadata: m,
            alternate,
        } => {
            assert_eq!(m, metadata);
            let n = alternate.nclx.unwrap();
            assert_eq!((n.primaries, n.transfer), (9, 16));
        }
        GainMapKind::Apple => panic!("expected a tmap"),
    }
    assert_eq!(f.info().colour.nclx.unwrap().primaries, 12);

    // A tmap that derives from another image is not the primary's gain map.
    let f = HeicFile::open(heif(&items, &props, &[(b"dimg", 3, &[2, 1])])).unwrap();
    assert!(!f.info().gain_map);
    assert!(f.gain_map().is_none());
    // Without its map input the tmap is malformed: no gain map, the base still opens.
    if let Ok(f) = HeicFile::open(heif(&items, &props, &[(b"dimg", 3, &[1])])) {
        assert!(f.gain_map().is_none());
    }
}

#[test]
fn apple_auxiliary() {
    let xmp = br#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description xmlns:HDRGainMap="http://ns.apple.com/HDRGainMap/1.0/" HDRGainMap:HDRGainMapVersion="131072" HDRGainMap:HDRGainMapHeadroom="4.5"/></rdf:RDF></x:xmpmeta>"#;
    let with_urn = |urn: &str| {
        let props = [hvcc(), ispe(64, 48), ispe(32, 24), auxc(urn)];
        let items = [
            coded(1, &[0x81, 2]),
            coded(2, &[0x81, 3, 0x84]),
            Item {
                id: 3,
                ty: b"mime",
                content_type: Some("application/rdf+xml"),
                props: vec![],
                data: xmp.to_vec(),
                extents: Vec::new(),
            },
        ];
        HeicFile::open(heif(
            &items,
            &props,
            &[(b"auxl", 2, &[1]), (b"cdsc", 3, &[2])],
        ))
        .unwrap()
    };
    let f = with_urn(APPLE_GAIN_MAP_URN);
    // Not an ISO tmap, so the container-level flag stays off.
    assert!(!f.info().gain_map);
    let g = f.gain_map().unwrap();
    assert_eq!(g.map, 2);
    assert!(matches!(g.kind, GainMapKind::Apple));
    // The map's XMP (where the headroom lives) belongs to the map, not to the primary.
    assert_eq!(f.item_xmp(2).as_deref(), Some(&xmp[..]));
    assert!(f.xmp().is_none());

    // Other auxiliaries (depth here) are not gain maps.
    let f = with_urn("urn:com:apple:photo:2020:aux:hdrdepth");
    assert!(f.gain_map().is_none());
}
