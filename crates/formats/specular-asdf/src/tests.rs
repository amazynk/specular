use super::*;

fn block(
    data: &[u8],
    compression: &[u8; 4],
    decoded: usize,
    pad: usize,
    checksum: [u8; 16],
) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(MAGIC);
    b.extend_from_slice(&48u16.to_be_bytes());
    b.extend_from_slice(&0u32.to_be_bytes());
    b.extend_from_slice(compression);
    b.extend_from_slice(&((data.len() + pad) as u64).to_be_bytes());
    b.extend_from_slice(&(data.len() as u64).to_be_bytes());
    b.extend_from_slice(&(decoded as u64).to_be_bytes());
    b.extend_from_slice(&checksum);
    b.extend_from_slice(data);
    b.extend(std::iter::repeat_n(0u8, pad));
    b
}

fn file(tree: &str, blocks: &[Vec<u8>], index: bool) -> Vec<u8> {
    let mut f = format!("#ASDF 1.0.0\n#ASDF_STANDARD 1.5.0\n%YAML 1.1\n%TAG ! tag:stsci.edu:asdf/\n--- !core/asdf-1.1.0\n{tree}...\n").into_bytes();
    let mut offsets = Vec::new();
    for b in blocks {
        offsets.push(f.len());
        f.extend_from_slice(b);
    }
    if index {
        f.extend_from_slice(b"#ASDF BLOCK INDEX\n%YAML 1.1\n---\n");
        for o in offsets {
            f.extend_from_slice(format!("- {o}\n").as_bytes());
        }
        f.extend_from_slice(b"...\n");
    }
    f
}

/// A zlib stream of stored deflate blocks.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut z = vec![0x78, 0x01];
    let chunks: Vec<&[u8]> = if data.is_empty() {
        vec![&[][..]]
    } else {
        data.chunks(65535).collect()
    };
    for (k, c) in chunks.iter().enumerate() {
        z.push(u8::from(k + 1 == chunks.len()));
        let n = c.len() as u16;
        z.extend_from_slice(&n.to_le_bytes());
        z.extend_from_slice(&(!n).to_le_bytes());
        z.extend_from_slice(c);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    z
}

/// One literal-only LZ4 block.
fn lz4_literal(data: &[u8]) -> Vec<u8> {
    let mut o = Vec::new();
    let n = data.len();
    if n < 15 {
        o.push((n as u8) << 4);
    } else {
        o.push(0xf0);
        let mut r = n - 15;
        while r >= 255 {
            o.push(255);
            r -= 255;
        }
        o.push(r as u8);
    }
    o.extend_from_slice(data);
    o
}

/// ASDF's LZ4 framing over chunks of `chunk` bytes.
fn lz4_asdf(data: &[u8], chunk: usize) -> Vec<u8> {
    let mut o = Vec::new();
    for c in data.chunks(chunk) {
        let body = lz4_literal(c);
        o.extend_from_slice(&((body.len() + 4) as u32).to_be_bytes());
        o.extend_from_slice(&(c.len() as u32).to_le_bytes());
        o.extend_from_slice(&body);
    }
    o
}

fn le_f32(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

#[test]
fn sniffs() {
    assert!(sniff(b"#ASDF 1.0.0\n"));
    assert!(!sniff(b"#ASDF_STANDARD 1.0.0"));
    assert!(!sniff(b"SIMPLE  ="));
    assert_eq!(Asdf::open(b"#ASDF x").unwrap_err(), Error::NotAsdf);
    assert!(matches!(
        Asdf::open(b"#ASDF 2.0.0\n"),
        Err(Error::Unsupported(_))
    ));
}

#[test]
fn plain_blocks_and_index() {
    let a: Vec<f32> = (0..12).map(|i| i as f32).collect();
    let b: Vec<u8> = (0..6).collect();
    let tree = "x: !core/ndarray-1.0.0 {source: 0, datatype: float32, byteorder: little, shape: [3, 4]}\ny: !core/ndarray-1.0.0 {source: -1, datatype: uint8, byteorder: big, shape: [2, 3]}\nasdf_library: {name: asdf, version: 5.4.0}\n";
    for index in [false, true] {
        let f = file(
            tree,
            &[
                block(&le_f32(&a), &[0; 4], 48, 7, [0; 16]),
                block(&b, &[0; 4], 6, 0, [0; 16]),
            ],
            index,
        );
        let d = Asdf::open(&f).unwrap();
        assert_eq!(d.version, "1.0.0");
        assert_eq!(d.standard.as_deref(), Some("1.5.0"));
        assert_eq!(d.indexed, index);
        assert_eq!(d.blocks.len(), 2);
        assert_eq!(d.library(), Some(("asdf".into(), "5.4.0".into())));
        assert_eq!(d.arrays.len(), 2);
        let x = &d.arrays[0];
        assert_eq!((x.path.as_str(), x.shape.as_slice()), ("x", &[3u64, 4][..]));
        let bytes = d.array_data(&f, x).unwrap();
        assert_eq!(x.plane(&bytes, &[]).unwrap(), a);
        let y = &d.arrays[1];
        assert_eq!(y.source, ArraySource::Block(1));
        let bytes = d.array_data(&f, y).unwrap();
        assert_eq!(y.plane(&bytes, &[]).unwrap(), vec![0., 1., 2., 3., 4., 5.]);
    }
}

#[test]
fn bad_index_falls_back_to_walking() {
    let tree = "x: !core/ndarray-1.0.0 {source: 0, datatype: uint8, shape: [4]}\n";
    let mut f = file(tree, &[block(&[1, 2, 3, 4], &[0; 4], 4, 0, [0; 16])], false);
    f.extend_from_slice(b"#ASDF BLOCK INDEX\n%YAML 1.1\n---\n- 3\n...\n");
    let d = Asdf::open(&f).unwrap();
    assert!(!d.indexed);
    assert_eq!(d.blocks.len(), 1);
}

#[test]
fn compressed_blocks() {
    let raw: Vec<u8> = (0..70_000u32).map(|i| (i * 7 % 251) as u8).collect();
    let z = zlib_stored(&raw);
    let l = lz4_asdf(&raw, 30_000);
    let tree = "a: !core/ndarray-1.0.0 {source: 0, datatype: uint8, shape: [70000]}\nb: !core/ndarray-1.0.0 {source: 1, datatype: uint8, shape: [70000]}\nc: !core/ndarray-1.0.0 {source: 2, datatype: uint8, shape: [4]}\n";
    let f = file(
        tree,
        &[
            block(&z, b"zlib", raw.len(), 0, [0; 16]),
            block(&l, b"lz4\0", raw.len(), 3, [0; 16]),
            block(&[0; 9], b"blsc", 4, 0, [0; 16]),
        ],
        true,
    );
    let d = Asdf::open(&f).unwrap();
    assert_eq!(d.blocks[0].compression_name().as_deref(), Some("zlib"));
    assert_eq!(&*d.block_data(&f, 0).unwrap(), &raw[..]);
    assert_eq!(&*d.block_data(&f, 1).unwrap(), &raw[..]);
    match d.block_data(&f, 2) {
        Err(Error::Unsupported(m)) => assert!(m.contains("blsc"), "{m}"),
        other => panic!("{other:?}"),
    }
    // A header that lies about the decoded size.
    let f = file(
        tree,
        &[block(&z, b"zlib", raw.len() + 1, 0, [0; 16])],
        false,
    );
    let d = Asdf::open(&f).unwrap();
    assert!(matches!(d.block_data(&f, 0), Err(Error::Malformed(_))));
}

#[test]
fn checksums() {
    use md5::Digest;
    let data = [9u8, 8, 7, 6];
    let sum: [u8; 16] = md5::Md5::digest(data).into();
    let mut wrong = sum;
    wrong[0] ^= 1;
    let f = file(
        "",
        &[
            block(&data, &[0; 4], 4, 0, sum),
            block(&data, &[0; 4], 4, 0, wrong),
            block(&data, &[0; 4], 4, 0, [0; 16]),
        ],
        false,
    );
    let d = Asdf::open(&f).unwrap();
    assert_eq!(d.verify_checksum(&f, 0).unwrap(), Some(true));
    assert_eq!(d.verify_checksum(&f, 1).unwrap(), Some(false));
    assert_eq!(d.verify_checksum(&f, 2).unwrap(), None);
}

#[test]
fn streamed_block() {
    let tree =
        "s: !core/ndarray-1.0.0 {source: -1, datatype: int16, byteorder: big, shape: ['*', 3]}\n";
    let mut b = block(&[], &[0; 4], 0, 0, [0; 16]);
    b[6..10].copy_from_slice(&1u32.to_be_bytes());
    for v in [1i16, -2, 3, 4, 5, -6, 7, 8] {
        b.extend_from_slice(&v.to_be_bytes());
    }
    let f = file(tree, &[b], false);
    let d = Asdf::open(&f).unwrap();
    assert!(d.blocks[0].streamed());
    let s = &d.arrays[0];
    // 16 bytes hold two whole rows of three.
    assert_eq!(s.shape, vec![2, 3]);
    let bytes = d.array_data(&f, s).unwrap();
    assert_eq!(
        s.plane(&bytes, &[]).unwrap(),
        vec![1., -2., 3., 4., 5., -6.]
    );
}

#[test]
fn strides_offset_and_planes() {
    // A 2 × 3 × 4 int32 cube, read transposed through strides and from an offset.
    let vals: Vec<i32> = (0..24).collect();
    let bytes: Vec<u8> = vals.iter().flat_map(|v| v.to_le_bytes()).collect();
    let tree = "c: !core/ndarray-1.0.0 {source: 0, datatype: int32, shape: [2, 3, 4]}\nt: !core/ndarray-1.0.0 {source: 0, datatype: int32, shape: [4, 3], strides: [4, 16], offset: 48}\nr: !core/ndarray-1.0.0 {source: 0, datatype: int32, shape: [2], strides: [-4], offset: 92}\nbad: !core/ndarray-1.0.0 {source: 0, datatype: int32, shape: [7, 4], offset: 4}\n";
    let f = file(tree, &[block(&bytes, &[0; 4], 96, 0, [0; 16])], false);
    let d = Asdf::open(&f).unwrap();
    let data = d.block_data(&f, 0).unwrap();
    let c = &d.arrays[0];
    assert_eq!(
        c.plane(&data, &[1]).unwrap(),
        (12..24).map(|v| v as f32).collect::<Vec<_>>()
    );
    assert!(c.plane(&data, &[2]).is_err());
    assert!(c.plane(&data, &[]).is_err());
    let t = &d.arrays[1];
    assert_eq!(
        t.plane(&data, &[]).unwrap(),
        vec![12., 16., 20., 13., 17., 21., 14., 18., 22., 15., 19., 23.]
    );
    let r = &d.arrays[2];
    assert_eq!(r.plane(&data, &[]).unwrap(), vec![23., 22.]);
    let bad = &d.arrays[3];
    assert!(matches!(bad.plane(&data, &[]), Err(Error::Malformed(_))));
}

#[test]
fn squeeze() {
    let tree = "c: !core/ndarray-1.0.0 {source: 0, datatype: uint8, shape: [2, 3, 1]}\n";
    let f = file(
        tree,
        &[block(&[1, 2, 3, 4, 5, 6], &[0; 4], 6, 0, [0; 16])],
        false,
    );
    let d = Asdf::open(&f).unwrap();
    let s = d.arrays[0].squeezed();
    assert_eq!(s.shape, vec![2, 3]);
    assert_eq!(
        s.plane(&d.block_data(&f, 0).unwrap(), &[]).unwrap(),
        vec![1., 2., 3., 4., 5., 6.]
    );
    let one = NdArray {
        shape: vec![1, 1, 5],
        strides: None,
        ..s.clone()
    };
    assert_eq!(one.squeezed().shape, vec![1, 5]);
}

#[test]
fn inline_quantity_mask_and_skips() {
    let tree = "i: !core/ndarray-1.0.0\n  data: [[1, 2], [3, 4]]\n  datatype: int8\n  shape: [2, 2]\nj: !core/ndarray-1.0.0 [[1.5, null], [true, '3+4j']]\nq: !unit/quantity-1.1.0\n  unit: !unit/unit-1.0.0 DN / s\n  value: !core/ndarray-1.0.0 {source: 0, datatype: uint16, shape: [2], mask: 7}\ns: !core/ndarray-1.0.0 {source: 0, datatype: [{name: a, datatype: int8}], shape: [1]}\nf: !core/ndarray-1.0.0 {source: 'fits:SCI,1', datatype: float32, shape: [2, 2]}\ne: !core/ndarray-1.0.0 {source: other0000.asdf, datatype: float64, shape: [3]}\nw: !<tag:stsci.edu:gwcs/wcs-1.2.0> {k: !core/ndarray-1.0.0 [1, 2]}\nm: !core/ndarray-1.0.0 {source: 5, datatype: uint8, shape: [1]}\n";
    let f = file(tree, &[block(&[7, 0, 9, 0], &[0; 4], 4, 0, [0; 16])], false);
    let d = Asdf::open(&f).unwrap();
    let paths: Vec<&str> = d.arrays.iter().map(|a| a.path.as_str()).collect();
    assert_eq!(paths, ["i", "j", "q", "e"]);
    let i = &d.arrays[0];
    assert_eq!(i.dtype, Dtype::I8);
    assert_eq!(i.plane(&[], &[]).unwrap(), vec![1., 2., 3., 4.]);
    let j = d.arrays[1].plane(&[], &[]).unwrap();
    assert_eq!(j[0], 1.5);
    assert!(j[1].is_nan());
    assert_eq!(&j[2..], &[1.0, 5.0]);
    let q = &d.arrays[2];
    assert_eq!(q.unit.as_deref(), Some("DN / s"));
    let p = q.plane(&d.array_data(&f, q).unwrap(), &[]).unwrap();
    assert!(p[0].is_nan());
    assert_eq!(p[1], 9.0);
    assert_eq!(
        d.arrays[3].source,
        ArraySource::External("other0000.asdf".into())
    );
    assert!(d.array_data(&f, &d.arrays[3]).is_err());
    let skipped: Vec<&str> = d.skipped.iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(skipped, ["s", "f", "m"]);
    assert!(d.skipped[1].1.contains("FITS"));
}

/// Encodes a value in a dtype, big-endian when asked.
type Encode = fn(f64, bool) -> Vec<u8>;

#[test]
fn every_dtype_both_orders() {
    let cases: [(&str, usize, Encode); 12] = [
        ("int8", 1, |v, _| vec![v as i8 as u8]),
        ("uint8", 1, |v, _| vec![v as u8]),
        ("int16", 2, |v, be| {
            if be {
                (v as i16).to_be_bytes().to_vec()
            } else {
                (v as i16).to_le_bytes().to_vec()
            }
        }),
        ("uint16", 2, |v, be| {
            if be {
                (v as u16).to_be_bytes().to_vec()
            } else {
                (v as u16).to_le_bytes().to_vec()
            }
        }),
        ("int32", 4, |v, be| {
            if be {
                (v as i32).to_be_bytes().to_vec()
            } else {
                (v as i32).to_le_bytes().to_vec()
            }
        }),
        ("uint32", 4, |v, be| {
            if be {
                (v as u32).to_be_bytes().to_vec()
            } else {
                (v as u32).to_le_bytes().to_vec()
            }
        }),
        ("int64", 8, |v, be| {
            if be {
                (v as i64).to_be_bytes().to_vec()
            } else {
                (v as i64).to_le_bytes().to_vec()
            }
        }),
        ("uint64", 8, |v, be| {
            if be {
                (v as u64).to_be_bytes().to_vec()
            } else {
                (v as u64).to_le_bytes().to_vec()
            }
        }),
        ("float32", 4, |v, be| {
            if be {
                (v as f32).to_be_bytes().to_vec()
            } else {
                (v as f32).to_le_bytes().to_vec()
            }
        }),
        ("float64", 8, |v, be| {
            if be {
                v.to_be_bytes().to_vec()
            } else {
                v.to_le_bytes().to_vec()
            }
        }),
        ("bool8", 1, |v, _| vec![u8::from(v != 0.0)]),
        ("complex128", 16, |v, be| {
            let mut o = if be {
                v.to_be_bytes().to_vec()
            } else {
                v.to_le_bytes().to_vec()
            };
            o.extend_from_slice(&[0; 8]);
            o
        }),
    ];
    for (name, size, enc) in cases {
        for be in [false, true] {
            let vals = if name == "bool8" {
                vec![0.0, 1.0, 1.0, 0.0]
            } else {
                vec![0.0, 1.0, 100.0, 7.0]
            };
            let bytes: Vec<u8> = vals.iter().flat_map(|&v| enc(v, be)).collect();
            assert_eq!(bytes.len(), 4 * size);
            let order = if be { "big" } else { "little" };
            let tree = format!(
                "a: !core/ndarray-1.0.0 {{source: 0, datatype: {name}, byteorder: {order}, shape: [2, 2]}}\n"
            );
            let f = file(
                &tree,
                &[block(&bytes, &[0; 4], bytes.len(), 0, [0; 16])],
                false,
            );
            let d = Asdf::open(&f).unwrap();
            let a = &d.arrays[0];
            assert_eq!(a.dtype.size(), size, "{name}");
            let p = a.plane(&d.array_data(&f, a).unwrap(), &[]).unwrap();
            assert_eq!(
                p,
                vals.iter().map(|&v| v as f32).collect::<Vec<_>>(),
                "{name} {order}"
            );
        }
    }
}

#[test]
fn float16() {
    // 1.0, -2.0, 0.5, inf.
    let h = [0x3c00u16, 0xc000, 0x3800, 0x7c00];
    let bytes: Vec<u8> = h.iter().flat_map(|v| v.to_be_bytes()).collect();
    let tree =
        "a: !core/ndarray-1.0.0 {source: 0, datatype: float16, byteorder: big, shape: [4]}\n";
    let f = file(tree, &[block(&bytes, &[0; 4], 8, 0, [0; 16])], false);
    let d = Asdf::open(&f).unwrap();
    let p = d.arrays[0]
        .plane(&d.block_data(&f, 0).unwrap(), &[])
        .unwrap();
    assert_eq!(p, vec![1.0, -2.0, 0.5, f32::INFINITY]);
}

#[test]
fn truncated_and_malformed() {
    let tree = "a: !core/ndarray-1.0.0 {source: 0, datatype: uint8, shape: [4]}\n";
    let f = file(tree, &[block(&[1, 2, 3, 4], &[0; 4], 4, 0, [0; 16])], false);
    for cut in [f.len() - 1, f.len() - 4, f.len() - 50] {
        assert!(Asdf::open(&f[..cut]).is_err(), "cut at {cut}");
    }
    // No blocks at all is fine.
    let d = Asdf::open(&file("a: 1\n", &[], false)).unwrap();
    assert!(d.blocks.is_empty() && d.arrays.is_empty());
    // A header-only file.
    let d = Asdf::open(b"#ASDF 1.0.0\n").unwrap();
    assert!(d.tree.is_null());
}

#[test]
fn external_file() {
    let ext = file("", &[block(&[5, 6, 7], &[0; 4], 3, 0, [0; 16])], false);
    assert_eq!(&*external_data(&ext).unwrap(), &[5, 6, 7]);
    assert!(external_data(&file("", &[], false)).is_err());
}

mod hostile {
    use super::*;

    /// A streamed shape whose trailing axes overflow u64 once multiplied by the item size.
    #[test]
    fn streamed_row_overflow() {
        let tree = "s: !core/ndarray-1.0.0 {source: -1, datatype: float64, shape: ['*', 4294967296, 4294967296]}\n";
        let mut b = block(&[], &[0; 4], 0, 0, [0; 16]);
        b[6..10].copy_from_slice(&1u32.to_be_bytes());
        b.extend_from_slice(&[0; 16]);
        let f = file(tree, &[b], false);
        let d = Asdf::open(&f).unwrap();
        assert!(d.arrays.is_empty());
        assert_eq!(d.skipped[0].1, "too many elements");
    }

    /// An LZ4 block claiming 8 GiB decoded from a few bytes: no 8 GiB reserve, a clean error.
    #[test]
    fn lz4_lying_size() {
        let l = lz4_asdf(&[1, 2, 3, 4], 4);
        let tree = "a: !core/ndarray-1.0.0 {source: 0, datatype: uint8, shape: [4]}\n";
        let f = file(tree, &[block(&l, b"lz4\0", 1 << 33, 0, [0; 16])], false);
        let d = Asdf::open(&f).unwrap();
        assert!(matches!(d.block_data(&f, 0), Err(Error::Malformed(_))));
    }
}

/// A 32768² plane over 4 bytes with zero strides, and an inline array claiming the same
/// shape: each used to reserve 4 GiB before anything was checked.
#[test]
fn planes_are_checked_before_allocating() {
    let tree = "z: !core/ndarray-1.0.0 {source: 0, datatype: int32, shape: [32768, 32768], strides: [0, 0]}\ni: !core/ndarray-1.0.0 {data: [1, 2], datatype: int32, shape: [32768, 32768]}\n";
    let f = file(tree, &[block(&[1, 0, 0, 0], &[0; 4], 4, 0, [0; 16])], false);
    let d = Asdf::open(&f).unwrap();
    let data = d.block_data(&f, 0).unwrap();
    for a in &d.arrays {
        assert!(a.plane(&data, &[]).is_err(), "{}", a.path);
    }
}

#[test]
fn compressed_blocks_reserve_from_the_budget() {
    // 8 MiB blocks from a few bytes each, refused by a 4 MiB budget.
    let raw = vec![0u8; 8 << 20];
    let z = zlib_stored(&raw);
    let l = lz4_asdf(&raw, 1 << 20);
    // bzip2 of 8 MiB of zeros (python3 bz2.compress, level 9).
    let bz: &[u8] = &[
        66, 90, 104, 57, 49, 65, 89, 38, 83, 89, 162, 61, 75, 88, 0, 64, 64, 64, 128, 192, 0, 0, 4,
        0, 8, 32, 0, 48, 204, 5, 41, 166, 1, 0, 216, 128, 128, 120, 187, 146, 41, 194, 132, 133,
        17, 234, 90, 192,
    ];
    let tree = "a: 1\n";
    let f = file(
        tree,
        &[
            block(&z, b"zlib", raw.len(), 0, [0; 16]),
            block(&l, b"lz4\0", raw.len(), 3, [0; 16]),
            block(bz, b"bzp2", raw.len(), 0, [0; 16]),
        ],
        true,
    );
    let d = Asdf::open(&f).unwrap();
    for i in 0..3 {
        let r = specular_core::budget::with_limit(4 << 20, || d.block_data(&f, i).map(|c| c.len()));
        assert!(matches!(r, Err(Error::NeedsMemory(_))), "block {i}: {r:?}");
        assert_eq!(d.block_data(&f, i).unwrap().len(), raw.len(), "block {i}");
    }
    // A returned block keeps its memory reserved until it is dropped: two of them do not fit
    // a 12 MiB budget together.
    let b = std::sync::Arc::new(specular_core::budget::MemoryBudget::fixed(12 << 20));
    specular_core::budget::with_budget(b.clone(), || {
        let first = d.block_data(&f, 0).unwrap();
        assert_eq!(b.reserved(), 8 << 20);
        assert_eq!(first.reserved(), 8 << 20);
        assert!(matches!(d.block_data(&f, 1), Err(Error::NeedsMemory(_))));
        let (bytes, kept) = first.into_parts();
        assert_eq!(kept.bytes(), bytes.len() as u64);
        drop((bytes, kept));
        assert_eq!(b.reserved(), 0);
        assert!(d.block_data(&f, 1).is_ok());
    });
}
